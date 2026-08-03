# MuxDeck — Wire Protocol v1

**This document is the single source of truth.** Rust types in `engine/crates/muxdeck-core`
and Dart types in `packages/muxdeck_protocol` are implementations of this spec, not the
other way around.

To change the protocol: edit this file → add/update `protocol/fixtures/*.json` → update Rust
types → update Dart types. One commit, that order.

---

## 1. Transport

- WebSocket over TLS: `wss://<host>:47654/ws`
- Default port **47654** (configurable).
- Text frames. UTF-8 JSON. One message per frame.
- Client verifies the server certificate by **SHA-256 fingerprint pinning**. The normal CA
  chain is not used and must not be relied on.

## 2. Envelope

Every message is a JSON object with this shape:

```json
{
  "v": 1,
  "t": "req",
  "id": "01J8Z9K3M2",
  "op": "input.key_combo",
  "d": {}
}
```

| Field | Type | Present on | Meaning |
| --- | --- | --- | --- |
| `v` | int | all | Protocol major version. Reject anything but `1`. |
| `t` | string | all | `req`, `res`, `err`, or `evt`. |
| `id` | string | `req`, `res`, `err` | Correlation ID. Client-generated, unique per connection. Absent on `evt`. |
| `op` | string | all | Operation name, `namespace.action`. |
| `d` | object | all | Payload. `{}` when empty, never `null`. |

An `err` response carries:

```json
{
  "v": 1, "t": "err", "id": "01J8Z9K3M2", "op": "input.key_combo",
  "d": { "code": "NOT_AUTHORIZED", "message": "role 'deck' may not call this op" }
}
```

### 2.1 Error codes

| Code | Meaning |
| --- | --- |
| `BAD_REQUEST` | Malformed envelope or payload |
| `UNSUPPORTED_VERSION` | `v` is not 1 |
| `UNKNOWN_OP` | No such op |
| `NOT_AUTHENTICATED` | Op requires a completed session handshake |
| `NOT_AUTHORIZED` | Role lacks capability for this op |
| `PAIRING_CLOSED` | Not in pairing mode, or window expired |
| `BAD_CODE` | Wrong one-time pairing code |
| `UNKNOWN_DEVICE` | Device ID not in registry |
| `BAD_SIGNATURE` | Challenge signature did not verify |
| `INJECTION_FAILED` | OS refused the input event; `message` explains |
| `NOT_FOUND` | Profile or action does not exist |
| `DISABLED` | Feature is switched off (e.g. shell execution) |
| `INTERNAL` | Engine bug; always logged with a trace ID |

## 3. Connection lifecycle

```
client                                   engine
  │  ── TLS connect, verify fingerprint ──▶
  │  ── req session.hello ───────────────▶
  │  ◀── res session.challenge ───────────
  │  ── req session.auth (signature) ────▶
  │  ◀── res session.ready ───────────────
  │  ◀── evt profile.changed (initial) ───
  │        ... normal operation ...
```

An unauthenticated socket may send only `session.*` and `pair.*`. It is closed after
10 seconds without a successful `session.auth`.

## 4. Operations

### 4.1 `session.*`

**`session.hello`** — req
```json
{ "device_id": "d_7f3a91c2", "client_version": "0.1.0", "platform": "ios" }
```
`platform` ∈ `ios` | `android` | `windows` | `macos` | `linux`.

**`session.challenge`** — res
```json
{ "nonce": "base64:32-bytes", "host_id": "h_a91c...", "host_name": "ENIGMA-ENTROPY" }
```

**`session.auth`** — req
```json
{ "signature": "base64:ed25519-sig-over-nonce" }
```

**`session.ready`** — res
```json
{ "role": "deck", "protocol": 1, "engine_version": "0.1.0", "active_profile_id": "p_default" }
```

### 4.2 `pair.*`

Callable only while the engine is in pairing mode.

**`pair.request`** — req
```json
{
  "code": "402913",
  "device_pubkey": "base64:32-bytes",
  "device_name": "Cipher's iPad",
  "platform": "ios"
}
```

**`pair.request`** — res
```json
{ "device_id": "d_7f3a91c2", "host_id": "h_a91c...", "host_name": "ENIGMA-ENTROPY" }
```

**`pair.begin`** — req, **admin only**. Opens a 120 s pairing window.
```json
{ "ttl_seconds": 120 }
```
res:
```json
{ "code": "402913", "expires_at": 1785312000, "qr_payload": "muxdeck://pair?addr=..." }
```

**`pair.cancel`** — req, admin only. `{}` → `{}`
**`pair.list_devices`** — req, admin only. `{}` →
```json
{ "devices": [
  { "device_id": "d_7f3a91c2", "name": "Cipher's iPad", "platform": "ios",
    "paired_at": 1785300000, "last_seen": 1785311900, "connected": true }
] }
```
**`pair.revoke`** — req, admin only. `{ "device_id": "d_7f3a91c2" }` → `{}`
Revoking closes any live socket for that device immediately.

### 4.3 `input.*` — role `deck` and `admin`

**`input.key_combo`**
```json
{ "keys": ["CONTROL", "SHIFT", "ESCAPE"], "hold_ms": 0 }
```
Modifiers are pressed in listed order, the final non-modifier key is tapped, then all are
released in reverse order. `hold_ms` optionally holds before release. Key names use the
canonical table in §5.

**`input.key_sequence`** — several combos in order.
```json
{ "steps": [ { "keys": ["CONTROL","C"] }, { "delay_ms": 50 }, { "keys": ["CONTROL","V"] } ] }
```
Each step is either `{ "keys": [...] , "hold_ms": 0 }` or `{ "delay_ms": n }`.

**`input.text`** — type a literal string (Unicode-safe; uses scancode-free unicode injection).
```json
{ "text": "muxdeck", "wpm": 0 }
```
`wpm: 0` means as fast as the OS allows.

**`input.media`**
```json
{ "command": "PLAY_PAUSE" }
```
`command` ∈ `PLAY_PAUSE` | `NEXT` | `PREV` | `STOP` | `VOLUME_UP` | `VOLUME_DOWN` | `MUTE`.

**`input.mouse`**
```json
{ "action": "move_rel", "dx": 12, "dy": -4 }
```
`action` ∈ `move_rel` | `move_abs` | `click` | `down` | `up` | `scroll`.
`click`/`down`/`up` take `"button": "left" | "right" | "middle"`.
`scroll` takes `dx`, `dy` in notches.

All `input.*` responses are `{}` on success, or an `err` with `INJECTION_FAILED`.

### 4.4 `action.*`

**`action.run`** — run a **named**, pre-defined action. The client never sends a command string.
```json
{ "action_id": "a_obs_scene_gaming" }
```
Returns `err` `DISABLED` if the shell feature is off, `NOT_FOUND` if the ID is unknown.

**`action.list`** — admin only. Returns defined actions.
**`action.set`** / **`action.delete`** — admin only. Defining an action requires the shell
feature to be enabled.

### 4.5 `profile.*`

A **profile** is one deck layout: a grid of pages of buttons.

**`profile.get`** — `{ "profile_id": "p_default" }` → a Profile object (§6).
**`profile.list`** → `{ "profiles": [ { "id": "...", "name": "...", "active": true } ] }`
**`profile.subscribe`** — `{}` → `{}`. Thereafter the engine pushes `evt profile.changed`.
**`profile.set`** — admin only. `{ "profile": { ...Profile } }` → `{}`
**`profile.activate`** — admin only. `{ "profile_id": "p_stream" }` → `{}`
**`profile.delete`** — admin only. `{ "profile_id": "p_old" }` → `{}`

### 4.6 `settings.*` — admin only

**`settings.get`** → 
```json
{
  "port": 47654, "host_name": "ENIGMA-ENTROPY",
  "shell_actions_enabled": false, "telemetry_enabled": true,
  "telemetry_interval_ms": 1000, "autostart": true
}
```
**`settings.set`** — partial object of the same shape → `{}`.
Changing `port` requires a restart; the response includes `{ "restart_required": true }`.

### 4.7 `ping`

**`ping`** — req `{ "t_client": 1785311999123 }` → res `{ "t_client": 1785311999123, "t_engine": 1785311999130 }`
The client computes RTT locally; it does not trust `t_engine` for clock sync, only for
one-way-delay estimation.

### 4.8 Events (`t: "evt"`, no `id`)

| `op` | `d` |
| --- | --- |
| `profile.changed` | `{ "profile": { ...Profile } }` |
| `telemetry.update` | `{ "ts": 1785312000, "cpu_pct": 14.5, "ram_pct": 58.2, "active_window": "Code" }` |
| `device.changed` | `{ "devices": [ ...as pair.list_devices ] }` (admin only) |
| `pairing.state` | `{ "active": true, "expires_at": 1785312120 }` (admin only) |
| `engine.shutdown` | `{ "reason": "user_requested" }` |

`telemetry.update` is only sent to sockets that have called `profile.subscribe` and only when
`telemetry_enabled` is true.

## 5. Canonical key names

Uppercase, ASCII, no aliases. The engine maps these to platform scancodes.

```
Modifiers : CONTROL SHIFT ALT META
Letters   : A .. Z
Digits    : DIGIT0 .. DIGIT9
Function  : F1 .. F24
Nav       : ESCAPE TAB CAPSLOCK SPACE ENTER BACKSPACE DELETE INSERT
            HOME END PAGEUP PAGEDOWN LEFT RIGHT UP DOWN
Numpad    : NUMPAD0 .. NUMPAD9 NUMPAD_ADD NUMPAD_SUB NUMPAD_MUL NUMPAD_DIV
            NUMPAD_DECIMAL NUMPAD_ENTER
Symbols   : MINUS EQUAL BRACKET_LEFT BRACKET_RIGHT BACKSLASH SEMICOLON
            QUOTE BACKQUOTE COMMA PERIOD SLASH
System    : PRINTSCREEN SCROLLLOCK PAUSE NUMLOCK MENU
```

`META` is the Windows key on Windows/Linux and Command on macOS. The engine does **not**
auto-swap `CONTROL`/`META` on macOS; profiles are per-host, so the user maps what they want.

## 6. Data objects

### Profile
```json
{
  "id": "p_default",
  "name": "Default",
  "grid": { "cols": 5, "rows": 3 },
  "pages": [
    {
      "id": "pg_1",
      "name": "Main",
      "buttons": [
        {
          "id": "b_1",
          "pos": { "col": 0, "row": 0 },
          "label": "Copy",
          "icon": "content_copy",
          "color": "#2D6CDF",
          "haptic": "light",
          "on_tap":       { "op": "input.key_combo", "d": { "keys": ["CONTROL","C"] } },
          "on_long_press": null
        }
      ]
    }
  ]
}
```

- `icon` is a Material icon name; the client resolves it, unknown names fall back to a dot.
- `color` is `#RRGGBB`.
- `haptic` ∈ `none` | `light` | `medium` | `heavy`.
- `on_tap` / `on_long_press` are an embedded `{ op, d }` pair. The op must be one the sender's
  role is permitted to call — the engine re-checks at execution time, it does not trust the
  profile.
- Buttons are sparse: a grid cell with no button is empty.

### Action
```json
{ "id": "a_obs_scene_gaming", "name": "OBS: Gaming scene",
  "command": "obs-cli", "args": ["scene","switch","--name","Gaming"], "cwd": null }
```
`command` and `args` are separate — the engine never passes a string to a shell interpreter.

## 7. Versioning

`v` is the **major** version and appears in every message. A breaking change increments it and
the engine must then accept both for at least one release, advertising the highest supported
version in the mDNS TXT `v` record. Additive, optional fields do **not** bump `v`.

## 8. Fixtures

`protocol/fixtures/` holds one JSON file per message shape, named `<op>.<t>.json`, e.g.
`input.key_combo.req.json`. Both the Rust and the Dart test suites must deserialise every
fixture, re-serialise it, and assert semantic equality. A protocol change without a fixture
change is incomplete.
