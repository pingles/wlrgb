# Work Louder Creator Micro v2 — Lighting Protocol (reverse-engineered)

Reverse-engineered from the `input.app` (`it.focusense.input-app`, v0.16.0) bundle and its
`@worklouder/wl-device-kit` (v0.1.18) dependency. This documents how to control the keyboard
RGB directly over HID, without the input app.

## Device identification

The keyboard (USB **and** Bluetooth) presents as a USB-HID device with a vendor-specific
interface:

| field      | value                                        |
| ---------- | -------------------------------------------- |
| vendorId   | `12346` (`0x303A`, Espressif — it's ESP32)   |
| productId  | `33432` (`0x8298`) for `creator_micro_v2`    |
| usagePage  | `65280` (`0xFF00`, vendor-defined)           |
| manufacturer | "Work Louder" / "Work_Louder" (best-effort match) |

Selection logic (from `WLDeviceDiscovery.filterWLDevices`):
1. Prefer devices where `vendorId === 12346` **and** manufacturer contains "Work Louder".
2. Fall back to `vendorId === 12346` only.
3. Of those, keep the one with `usagePage === 0xFF00` and a known `productId`.

On macOS the device path looks like `DevSrvsID:4295064739`. The app opens it with
node-hid `HIDAsync.open(path, { nonExclusive: true })` — **non-exclusive**, so our tool can
hold the device at the same time as the input app.

## Transport framing (host → device)

The device speaks **JSON-RPC** over 64-byte HID output reports. Each report:

```
byte 0 : 0x06            report id (constant)
byte 1 : 0x02            channel (1 = debug, 2 = RPC)
byte 2 : chunkSize       number of payload bytes in THIS report (max 61)
byte 3.. : payload       UTF-8 bytes of the JSON-RPC string (zero-padded to 64)
```

Messages longer than 61 bytes are split across multiple reports, each with the same
`[0x06, 0x02, chunkSize]` header. (node-hid treats byte 0 of the write buffer as the HID
report id, so the on-the-wire report id is `6`.)

Responses come back on channel 2, newline-terminated, and are also JSON-RPC.

## JSON-RPC request shape

```json
{ "method": "lights.preview", "params": { ... }, "id": 123 }
```

- No `jsonrpc` version field.
- `id` is a random integer in **[0, 999)** (firmware rejects larger ids).
- The JSON string is unicode-escaped (`\uXXXX` for non-ASCII) before sending.

## Lighting RPC

`lights.preview` applies lighting **immediately but does NOT persist to flash**. Perfect for
transient status indication — when we stop sending, nothing is permanently changed; the
device keeps whatever was last previewed until power cycle / a new preview / a persisted write.

```json
{
  "method": "lights.preview",
  "params": {
    "backlight":  { "effect": "snake", "brightness": 1, "speed": 0.5, "magic": 1, "color": 16733952 },
    "underglow":  { "effect": "snake", "brightness": 1, "speed": 0.5, "magic": 1, "color": 16733952 }
  },
  "id": 42
}
```

Lighting config fields (per `backlight` and `underglow`):

| field      | type    | notes                                                        |
| ---------- | ------- | ------------------------------------------------------------ |
| effect     | string  | `off`, `solid`, `snake`, `rainbow`, `breath`, `gradient`     |
| brightness | float   | 0.0–1.0 ("Light" slider)                                     |
| speed      | float   | 0.0–1.0                                                      |
| magic      | int     | effect-specific parameter (disabled in UI for some effects)  |
| color      | int     | `0xRRGGBB` as a decimal integer, e.g. `#FF5500` → `16733952` |

Other discovered RPC methods (not needed for lighting): `sys.version`, `device.status`,
`fs.list`/`fs.read`/`fs.write`/`fs.readbin`/`fs.writebin`/`fs.delete`, `sys.bootloader`,
`sys.selftest`, `ui.home_accent_color`, `ui.active_screen`. Persisted config is written to
`keymap.json` on the device via the `fs.*` methods.

## Status → effect mapping (this project)

| Claude state          | effect   | color           |
| --------------------- | -------- | --------------- |
| working / thinking    | `snake`  | Claude orange   |
| waiting for input     | `breath` | (TBD)           |
| idle / back to normal | `rainbow`| device default  |
