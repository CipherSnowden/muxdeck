# MuxDeck Engine — Rust Daemon Spec

Binary name: **`muxdeckd`**. Read `docs/PROTOCOL.md` before touching anything on the wire.

## 1. Why Rust

The deciding factor is macOS. Reaching `CGEventPost` from Go requires cgo, which drags a C
toolchain into macOS CI and gives up the pure-static-binary property; Rust reaches the same API
through the `core-graphics` crate with no C compiler. Beyond that: a ~4 MB single binary per
platform, no GC pauses on a latency-sensitive path, and `#[cfg(target_os)]` giving compile-time
platform dispatch rather than runtime branching.

## 2. Cargo workspace layout

```
engine/
├── Cargo.toml                  # [workspace] members
├── rust-toolchain.toml         # pin the toolchain
└── crates/
    ├── muxdeck-core/           # protocol types + serde. ZERO I/O, zero platform code.
    ├── muxdeck-input/          # InputBackend trait + windows/macos/linux impls
    ├── muxdeck-engine/         # library: server, auth, registry, store, mdns, telemetry
    └── muxdeckd/               # binary: CLI parsing, config paths, wiring, tracing setup
```

Dependency direction is strictly downward: `muxdeckd` → `muxdeck-engine` → {`muxdeck-core`,
`muxdeck-input`}. `muxdeck-core` depends on nothing in this workspace.

Keeping `muxdeck-core` I/O-free means the protocol types are trivially testable and could be
published or reused later without pulling in tokio.

## 3. Dependencies

| Purpose | Crate |
| --- | --- |
| async runtime | `tokio` (rt-multi-thread, macros, net, signal, time, sync) |
| HTTP/WS server | `axum` with `ws` feature |
| TLS | `axum-server` + `rustls` |
| cert generation | `rcgen` |
| serialisation | `serde`, `serde_json` |
| signatures | `ed25519-dalek`, `rand` |
| hashing | `sha2` |
| mDNS | `mdns-sd` |
| config paths | `directories` |
| errors | `thiserror` (libs), `anyhow` (binary) |
| logging | `tracing`, `tracing-subscriber`, `tracing-appender` |
| CLI | `clap` (derive) |
| system metrics | `sysinfo` |
| Windows input | `windows` (Win32_UI_Input_KeyboardAndMouse, Win32_Foundation) |
| macOS input | `core-graphics`, `core-foundation` |
| Linux input | `evdev` / direct `/dev/uinput` ioctls via `nix` |

Do not add `enigo` or `rdev`. They are convenient but abstract away exactly the parts we need
control over (unicode injection, modifier ordering, media keys, scancode vs virtual-key), and
their platform coverage of media keys is incomplete.

## 4. `muxdeck-input` — the platform seam

```rust
pub trait InputBackend: Send + Sync {
    fn key_combo(&self, keys: &[Key], hold: Duration) -> Result<(), InputError>;
    fn text(&self, s: &str) -> Result<(), InputError>;
    fn media(&self, cmd: MediaCommand) -> Result<(), InputError>;
    fn mouse(&self, ev: MouseEvent) -> Result<(), InputError>;
    /// Returns Ok(()) if this backend can actually inject right now.
    /// e.g. macOS Accessibility not granted, Linux /dev/uinput not writable.
    fn preflight(&self) -> Result<(), InputError>;
}
```

`preflight()` is important: it is what lets the control panel say *"grant Accessibility
permission"* instead of buttons silently doing nothing. Call it at startup and expose the
result through `settings.get`, and its per-feature outcome through the `capabilities` block of
the `Ready` payload (`docs/PROTOCOL.md` §4.1) so clients can grey out unavailable actions rather
than failing at press time.

**`input.key_sequence` is not a trait method.** The trait stays synchronous and dumb: no
sequences, no delays, no timers. The `dispatch` module walks the steps itself, calling
`key_combo` on `spawn_blocking` and `tokio::time::sleep` for `delay_ms` steps. That keeps the
platform surface as small as possible — three backends implement four methods, not five — and
delays belong in async code, not blocked on a worker thread.

A `MockBackend` recording calls into a `Vec` lives behind `#[cfg(test)]` so every engine test
runs without touching the real OS.

### 4.1 Windows

`SendInput` with `INPUT_KEYBOARD`. Notes:

- Use **scancodes** with `KEYEVENTF_SCANCODE` where possible; virtual-key codes are
  keyboard-layout dependent and will produce wrong characters on non-US layouts.
- For `input.text`, use `KEYEVENTF_UNICODE` with the UTF-16 code unit — this bypasses layout
  entirely and handles non-ASCII correctly. Surrogate pairs need two events.
- Extended keys (arrows, Insert/Delete/Home/End/PageUp/PageDown, right Ctrl/Alt, numpad Enter)
  require `KEYEVENTF_EXTENDEDKEY` or they will be interpreted as their numpad twins.
- Media keys use `VK_MEDIA_PLAY_PAUSE`, `VK_VOLUME_UP`, etc. — these are virtual-key only.
- Batch a combo into **one** `SendInput` call with an array of events; separate calls can be
  interleaved by other input and drop modifiers.
- UIPI: a non-elevated daemon cannot inject into an elevated window. Detect and report rather
  than requiring admin — most users do not need it.

### 4.2 macOS

`CGEventCreateKeyboardEvent` + `CGEventPost(kCGHIDEventTap, ...)`.

- Requires **Accessibility** permission. Check with `AXIsProcessTrustedWithOptions`; surface
  the result via `preflight()`.
- Set modifier flags with `CGEventSetFlags` on the key event rather than posting separate
  modifier key events — more reliable across apps.
- For `input.text`, use `CGEventKeyboardSetUnicodeString`.
- Media keys are **not** normal key events on macOS: they need `NSEvent` of type
  `NSSystemDefined` subtype 8, or `CGEventPost` with the special media keycodes
  (`NX_KEYTYPE_PLAY` = 16, `NX_KEYTYPE_SOUND_UP` = 0, etc.). This is the fiddliest part of the
  macOS backend — budget time for it.

### 4.3 Linux

Create a virtual device via `/dev/uinput`.

- Kernel-level, so it works under **both X11 and Wayland**. This is why uinput is used rather
  than XTEST.
- Requires write access to `/dev/uinput`. Ship a udev rule:
  `KERNEL=="uinput", GROUP="input", MODE="0660"` and have the installer add the user to
  `input`. `preflight()` must detect the permission error and return a message naming the exact
  fix.
- After creating the device, **sleep ~100 ms** before the first event — udev needs time to
  settle or the first keystroke is dropped. This is a classic uinput bug.
- Emit `EV_SYN`/`SYN_REPORT` after each event batch or nothing is delivered.
- Register the full key range at device creation time (`UI_SET_KEYBIT` for every keycode you
  may ever send), including `KEY_PLAYPAUSE`, `KEY_VOLUMEUP`, etc.
- Unicode text injection has no clean uinput path. Implement `input.text` by mapping to the
  active layout where possible, and return `INJECTION_FAILED` with a clear message for
  characters that cannot be produced. Document this as a known Linux limitation.

## 5. `muxdeck-engine` — modules

| Module | Responsibility |
| --- | --- |
| `server` | axum router, `/ws` upgrade, TLS via rustls, per-socket task |
| `session` | handshake state machine, challenge/response, role assignment, 10 s auth timeout |
| `registry` | paired devices: pubkeys, names, last-seen. Persisted as JSON. |
| `pairing` | pairing window, OTP generation, QR payload construction |
| `identity` | host Ed25519 keypair, TLS cert, and `admin.token`: generate on first run, load thereafter |
| `store` | profiles and actions, persisted; atomic write (temp file + rename) |
| `dispatch` | op → handler routing, capability check, payload validation |
| `discovery` | mDNS advertise / de-advertise on shutdown |
| `telemetry` | `sysinfo` sampler on an interval, broadcast to subscribers |
| `config` | settings load/save, defaults |

Broadcast to subscribers uses `tokio::sync::broadcast`; each socket task holds a receiver.

## 6. Config directory

Via the `directories` crate: `ProjectDirs::from("in", "redoimagined", "MuxDeck")`.

| Platform | Path |
| --- | --- |
| Windows | `%APPDATA%\in.redoimagined\MuxDeck\config\` |
| macOS | `~/Library/Application Support/in.redoimagined.MuxDeck/` |
| Linux | `~/.config/muxdeck/` |

**These paths are an assumption, not a verified fact.** During M2, print the resolved
`config_dir()` on each platform and correct this table to match what the crate actually emits.
Do not trust what is written here over what the crate returns.

```
identity.key       host Ed25519 private key   (0600)
tls.pem / tls.key  self-signed certificate    (0600)
admin.token        32 random bytes, base64    (0600)
devices.json       paired device registry
profiles.json      layouts
actions.json       named shell actions
settings.json      engine settings
logs/muxdeckd.log  rolling, 7 days
```

All secret files are written with mode `0600` on Unix and a current-user-only DACL on Windows.
`admin.token` is a secret file: its file permissions are the entire boundary between "the user
who owns this desktop session" and "any other local user", per `docs/ARCHITECTURE.md` §5.4.

On first run, generate everything and log the fingerprint at INFO so the user can verify it.
Never log `admin.token` or any key material.

## 7. CLI

```
muxdeckd [OPTIONS]
  --port <PORT>              override listen port (default 47654)
  --config-dir <PATH>        override config directory
  --log-level <LEVEL>        trace|debug|info|warn|error  (default info)
  --foreground               log to stdout instead of the log file
  --print-fingerprint        print the TLS cert fingerprint and exit
  --reset-identity           regenerate host key + cert (unpairs all devices), requires --yes
  --yes                      confirm a destructive operation without prompting
```

Subcommands for the installer, invoked by the control panel:

```
muxdeckd service install     register auto-start for the current user
muxdeckd service uninstall
muxdeckd service status
```

Pairing subcommands, so a device can be paired before the desktop panel exists:

```
muxdeckd pair begin [--ttl <SECONDS>]    open a pairing window, print the code and QR payload
muxdeckd pair list                       list paired devices
muxdeckd pair revoke <DEVICE_ID>
```

These are not a second control path into the engine: they read `admin.token` from the config
directory and connect over loopback as an ordinary admin WebSocket client, exactly like the
panel. `--ttl` obeys the `30..=300` clamp from `docs/PROTOCOL.md` §4.2.

- Windows: a Scheduled Task at logon (no admin required), not a Windows Service — a Service
  runs in session 0 and **cannot inject input into the user's desktop**. This is a real trap.
- macOS: a `launchd` LaunchAgent in `~/Library/LaunchAgents/`.
- Linux: a systemd **user** unit in `~/.config/systemd/user/`.

## 8. Testing

- `muxdeck-core`: round-trip every file in `protocol/fixtures/` — deserialise, serialise,
  compare semantically. Reject unknown `v`. Reject unknown ops.
- `session`: full handshake happy path; wrong signature; unknown device; timeout; op sent
  before auth; `session.hello` carrying both `device_id` and `admin_token`, and neither; a valid
  `admin_token` from a non-loopback address (must not grant `admin`); a loopback connection with
  a wrong or absent token (must not grant `admin`).
- `pairing`: correct OTP; wrong OTP; invalid proof-of-possession; expired window; `ttl_seconds`
  out of the `30..=300` range; pairing op outside the window.
- `dispatch`: every op × every role, asserting the capability matrix in
  `docs/ARCHITECTURE.md` §5.4.
- `input`: against `MockBackend`, asserting modifier press/release ordering and that a combo
  releases in reverse order even when injection fails midway.
- Integration: spin the server on port 0, connect a real WS client, run a full
  pair → auth → key_combo flow against `MockBackend`.

Platform backends get smoke tests gated behind `#[ignore]` — they are run manually, since CI
runners have no desktop session to inject into.

## 9. Performance rules

- One `tokio` task per socket; never block the runtime. Input injection is a syscall that can
  take a millisecond or two — run it on `spawn_blocking`.
- Do not allocate per keystroke in the hot path; reuse buffers where the profiler says it
  matters and nowhere else.
- Log at DEBUG or below in the hot path. INFO-per-keypress will dominate the latency budget.
