//! wlrgb — control the RGB lighting on a Work Louder Creator Micro v2.
//!
//! Talks to the keyboard directly over HID using the device's JSON-RPC protocol
//! (`lights.preview`), reverse-engineered from the Work Louder "input" app.
//! See PROTOCOL.md for the wire format.
//!
//! Lighting is applied via `lights.preview`, which takes effect immediately but
//! is NOT persisted to the keyboard's flash — ideal for transient status. The
//! keyboard's saved profile is untouched.

use std::process::exit;
use std::time::{SystemTime, UNIX_EPOCH};

use hidapi::HidApi;

// --- Device / protocol constants (see PROTOCOL.md) ---
const WL_VID: u16 = 0x303A; // 12346, Espressif
const CREATOR_MICRO_V2_PID: u16 = 0x8298; // 33432
const USAGE_PAGE: u16 = 0xFF00; // vendor-defined HID interface
const REPORT_ID: u8 = 0x06;
const CHANNEL_RPC: u8 = 0x02;
const MAX_CHUNK: usize = 61; // payload bytes per 64-byte report

// --- Default colors for the named states ---
const WORKING_COLOR: u32 = 0xFF5500; // vivid Claude orange
const WAITING_COLOR: u32 = 0xFFFFFF; // white

/// One lighting segment's configuration.
struct LightConfig {
    effect: String,
    brightness: f32,
    speed: f32,
    magic: i32,
    color: u32,
}

impl LightConfig {
    /// Serialize to the JSON object the firmware expects, e.g.
    /// `{"effect":"snake","brightness":1,"speed":0.5,"magic":1,"color":16733952}`
    fn to_json(&self) -> String {
        format!(
            "{{\"effect\":\"{}\",\"brightness\":{},\"speed\":{},\"magic\":{},\"color\":{}}}",
            self.effect,
            fmt_num(self.brightness),
            fmt_num(self.speed),
            self.magic,
            self.color,
        )
    }
}

/// Format an f32 as a JSON number without a trailing ".0" (e.g. 1.0 -> "1").
fn fmt_num(v: f32) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}

fn print_usage() {
    eprintln!(
        "wlrgb — Work Louder Creator Micro v2 RGB control\n\
\n\
USAGE:\n\
    wlrgb working                 Claude is thinking/working  (snake, orange)\n\
    wlrgb waiting                 waiting for your input      (breath, white)\n\
    wlrgb normal                  back to normal              (rainbow)\n\
    wlrgb set <effect> <hex> [brightness] [speed]\n\
    wlrgb list                    list matching HID devices\n\
\n\
EFFECTS:  off | solid | snake | rainbow | breath | gradient\n\
HEX:      RRGGBB or #RRGGBB   (e.g. FF5500)\n\
BRIGHTNESS/SPEED: 0.0 - 1.0   (defaults: 1.0 / 0.5)\n"
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        print_usage();
        exit(2);
    }

    let result = match args[1].as_str() {
        "working" => set_both("snake", WORKING_COLOR, 1.0, 0.5),
        "waiting" => set_both("breath", WAITING_COLOR, 1.0, 0.5),
        "normal" => set_both("rainbow", 0xFFFFFF, 1.0, 0.5),
        "set" => run_set(&args[2..]),
        "list" => list_devices(),
        "-h" | "--help" | "help" => {
            print_usage();
            Ok(())
        }
        other => {
            eprintln!("error: unknown command '{other}'\n");
            print_usage();
            exit(2);
        }
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        exit(1);
    }
}

/// Parse args for the `set` subcommand and apply.
fn run_set(args: &[String]) -> Result<(), String> {
    if args.len() < 2 {
        return Err("usage: wlrgb set <effect> <hex> [brightness] [speed]".into());
    }
    let effect = args[0].clone();
    if !matches!(
        effect.as_str(),
        "off" | "solid" | "snake" | "rainbow" | "breath" | "gradient"
    ) {
        return Err(format!("unknown effect '{effect}'"));
    }
    let color = parse_hex(&args[1])?;
    let brightness = parse_unit(args.get(2), 1.0)?;
    let speed = parse_unit(args.get(3), 0.5)?;
    set_both(&effect, color, brightness, speed)
}

/// Parse "RRGGBB" / "#RRGGBB" / "RGB" into a 0xRRGGBB integer.
fn parse_hex(s: &str) -> Result<u32, String> {
    let h = s.trim_start_matches('#');
    let h = if h.len() == 3 {
        h.chars().flat_map(|c| [c, c]).collect::<String>()
    } else {
        h.to_string()
    };
    if h.len() != 6 {
        return Err(format!("invalid hex color '{s}'"));
    }
    u32::from_str_radix(&h, 16).map_err(|_| format!("invalid hex color '{s}'"))
}

/// Parse an optional 0.0–1.0 value, defaulting if absent.
fn parse_unit(s: Option<&String>, default: f32) -> Result<f32, String> {
    match s {
        None => Ok(default),
        Some(v) => {
            let n: f32 = v.parse().map_err(|_| format!("invalid number '{v}'"))?;
            if !(0.0..=1.0).contains(&n) {
                return Err(format!("value '{v}' out of range 0.0-1.0"));
            }
            Ok(n)
        }
    }
}

/// Apply the same config to both backlight and underglow.
fn set_both(effect: &str, color: u32, brightness: f32, speed: f32) -> Result<(), String> {
    let cfg = LightConfig {
        effect: effect.to_string(),
        brightness,
        speed,
        magic: 1,
        color,
    };
    let params = format!(
        "{{\"backlight\":{},\"underglow\":{}}}",
        cfg.to_json(),
        cfg.to_json()
    );
    send_rpc("lights.preview", &params)
}

/// Open the keyboard's HID RPC interface and send a JSON-RPC call.
fn send_rpc(method: &str, params_json: &str) -> Result<(), String> {
    let api = HidApi::new().map_err(|e| format!("failed to init HID: {e}"))?;
    // macos-shared-device feature already opens non-exclusive; be explicit too.
    #[cfg(target_os = "macos")]
    api.set_open_exclusive(false);

    let path = find_device(&api).ok_or(
        "Work Louder Creator Micro v2 not found (is it connected / paired?)".to_string(),
    )?;
    let dev = api
        .open_path(&path)
        .map_err(|e| format!("failed to open device: {e}"))?;

    let id = rpc_id();
    let msg = format!(r#"{{"method":"{method}","params":{params_json},"id":{id}}}"#);
    let bytes = msg.as_bytes();

    let mut offset = 0;
    while offset < bytes.len() {
        let chunk = MAX_CHUNK.min(bytes.len() - offset);
        let mut report = [0u8; 64];
        report[0] = REPORT_ID;
        report[1] = CHANNEL_RPC;
        report[2] = chunk as u8;
        report[3..3 + chunk].copy_from_slice(&bytes[offset..offset + chunk]);
        dev.write(&report)
            .map_err(|e| format!("HID write failed: {e}"))?;
        offset += chunk;
    }
    Ok(())
}

/// Find the vendor RPC interface path of the keyboard.
fn find_device(api: &HidApi) -> Option<std::ffi::CString> {
    // Prefer exact PID match; fall back to any matching vendor interface.
    let mut fallback: Option<std::ffi::CString> = None;
    for d in api.device_list() {
        if d.vendor_id() == WL_VID && d.usage_page() == USAGE_PAGE {
            if d.product_id() == CREATOR_MICRO_V2_PID {
                return Some(d.path().to_owned());
            }
            fallback.get_or_insert_with(|| d.path().to_owned());
        }
    }
    fallback
}

/// Random-ish JSON-RPC id in [0, 999) (firmware rejects larger ids).
fn rpc_id() -> u32 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    nanos % 999
}

/// Diagnostic: print every HID interface that matches the vendor id.
fn list_devices() -> Result<(), String> {
    let api = HidApi::new().map_err(|e| format!("failed to init HID: {e}"))?;
    let mut found = false;
    for d in api.device_list() {
        if d.vendor_id() == WL_VID {
            found = true;
            println!(
                "vid={:#06x} pid={:#06x} usage_page={:#06x} usage={} product={:?} path={:?}",
                d.vendor_id(),
                d.product_id(),
                d.usage_page(),
                d.usage(),
                d.product_string().unwrap_or("?"),
                d.path()
            );
        }
    }
    if !found {
        println!("no Work Louder (vid {WL_VID:#06x}) devices found");
    }
    Ok(())
}
