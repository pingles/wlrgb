# wlrgb — Work Louder Creator Micro v2 RGB control for Claude Code

A small Rust CLI that drives the RGB lighting on a [Work Louder Creator Micro v2]
keyboard to reflect what Claude Code is doing, via Claude Code hooks.

| Claude state        | command         | effect            |
| ------------------- | --------------- | ----------------- |
| thinking / working  | `wlrgb working` | snake, orange     |
| waiting for input   | `wlrgb waiting` | breath, white     |
| idle / done         | `wlrgb normal`  | rainbow           |

It talks to the keyboard **directly over HID** using the keyboard's JSON-RPC
protocol (`lights.preview`) — no dependency on the Work Louder "input" app being
the one in control. Because lighting is sent as a *preview*, it is applied
immediately but never written to the keyboard's flash, so your saved profile is
left untouched. The HID device is opened non-exclusively, so the tool coexists
with the input app if it happens to be running.

See [PROTOCOL.md](PROTOCOL.md) for the reverse-engineered wire format.

## Build & install

```bash
cargo install --path .        # installs to ~/.cargo/bin/wlrgb
```

## Usage

```bash
wlrgb working                          # snake, orange  (#FF5500)
wlrgb waiting                          # breath, white
wlrgb normal                           # rainbow
wlrgb set <effect> <hex> [bri] [spd]   # e.g. wlrgb set snake FF6A00 1 0.5
wlrgb list                             # list matching HID interfaces
```

Effects: `off | solid | snake | rainbow | breath | gradient`.
Brightness / speed are `0.0`–`1.0` (defaults `1.0` / `0.5`).

## Claude Code hooks

Wired into `~/.claude/settings.json`:

- `UserPromptSubmit` → `wlrgb working`
- `PostToolUse` → `wlrgb working` (re-asserts the working state after each tool, so a
  transient `waiting` from a mid-turn permission prompt self-heals back to snake)
- `Notification`, `PermissionRequest` → `wlrgb waiting`
- `Stop`, `StopFailure` → `wlrgb normal`

The hooks run `async` so they never add latency to a turn. After editing
settings, open `/hooks` once or restart Claude Code so the new config loads.

[Work Louder Creator Micro v2]: https://www.worklouder.cc/
