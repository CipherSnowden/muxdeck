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

The fingerprint is **lowercase hex, no separators, SHA-256 over the leaf certificate DER** —
64 characters. The same string appears in `muxdeckd --print-fingerprint`, the mDNS TXT `fp`
record, the QR `fp` parameter, and the client's comparison. See `docs/ARCHITECTURE.md` §5.1.

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

**A `res` or `err` echoes the `op` of the `req` it answers.** There is no such thing as a
response with an op of its own — a response is identified by its `id` and carries the request's
op verbatim. Payload *shapes* have names in this document (`Challenge`, `Ready`, …); those names
are documentation labels, not values on the wire.

An `evt` carries its own op name, because there is no request to echo, and has no `id`.

**All binary fields are standard base64** (RFC 4648 §4, `+/` alphabet, `=` padded). No prefix,
no URL-safe variant. This applies to `nonce`, `signature`, `device_pubkey`, `proof`, and
`admin_token`. Note the deliberate contrast with fingerprints and IDs, which are hex because they
travel in URLs and DNS TXT records; base64 is used only inside JSON bodies.

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

### 2.2 Identifiers

```
host_id   = "h_" + first 16 hex chars of SHA-256(host_public_key_bytes)
device_id = "d_" + first 16 hex chars of SHA-256(device_public_key_bytes)
```

18 characters, lowercase hex after the prefix. There is exactly one representation of each —
the mDNS TXT `id` record carries this same string, so a client can match a stored host to a
discovery result by string equality.

## 3. Connection lifecycle

A remote deck authenticates by signing a challenge with its device key:

```
client                                   engine
  │  ── TLS connect, verify fingerprint ──▶
  │  ── req session.hello (device_id) ───▶
  │  ◀── res session.hello (Challenge) ───
  │  ── req session.auth (signature) ────▶
  │  ◀── res session.auth (Ready) ────────
  │        ... normal operation ...
```

A loopback admin client authenticates with the local admin token, in one round trip:

```
panel                                    engine
  │  ── TLS connect, verify fingerprint ──▶
  │  ── req session.hello (admin_token) ─▶
  │  ◀── res session.hello (Ready) ───────
  │        ... normal operation ...
```

The engine pushes nothing on its own after the handshake. A client that wants a layout calls
`profile.get`, and `profile.subscribe` if it wants live updates; a client that wants telemetry
calls `telemetry.subscribe`. Explicit beats implicit, and the extra round trip costs ~10 ms once
per connection.

An unauthenticated socket may send only `session.*` and `pair.*`. It is closed after
10 seconds without reaching the `Ready` state, by either path.

## 4. Operations

The authoritative op × role capability matrix is `docs/ARCHITECTURE.md` §5.4. Role notes in this
section are a convenience; where they disagree, the matrix wins.

### 4.1 `session.*`

**`session.hello`** — req. Two mutually exclusive forms.

Deck form — a paired device, local or remote:
```json
{ "device_id": "d_7f3a91c2b4e05d18", "client_version": "0.1.0", "platform": "ios" }
```

Admin form — the local control panel, loopback only:
```json
{ "admin_token": "a0Rk9vQ2xZ7pN4tJ1sB6wH3mD8fG5cV0yU2iA7eK4oM=",
  "client_version": "0.1.0", "platform": "windows" }
```

`platform` ∈ `ios` | `android` | `windows` | `macos` | `linux`. Exactly one of `device_id` and
`admin_token` must be present; both or neither is `BAD_REQUEST`.

**`session.hello`** — res, `Challenge` payload (deck form only):
```json
{ "nonce": "9pQ0Vv3xR7tK2mN5bC8jH1sD4gF6wZ0aY3eU7iL5oP4=",
  "host_id": "h_a91c4d2e8f019b37", "host_name": "ENIGMA-ENTROPY" }
```

**`session.hello`** — res, `Ready` payload (admin form only). Identical shape to the `Ready`
below; `session.auth` is not sent on this path.

**`session.auth`** — req. Valid only after a `Challenge`.
```json
{ "signature": "Xr8kT2vB5nM9qL1cJ7hF4dS0aG6wY3zP8eU5iO2tK7Y=" }
```

The signature is plain Ed25519 by the device private key over exactly these bytes, concatenated
in this order:

```
message = b"muxdeck-session-v1"        (18 ASCII bytes, no separator, no terminator)
        || nonce                       (32 raw bytes, as base64-decoded from the Challenge)
        || device_id                   (UTF-8 bytes of the 18-char string, e.g. "d_7f3a91c2b4e05d18")
        || host_id                     (UTF-8 bytes of the 18-char string, e.g. "h_a91c4d2e8f019b37")
```

Domain separation by the `muxdeck-session-v1` prefix and by `host_id` is what prevents a
signature captured against one host from being replayed against another. The Rust and Dart sides
must build a byte-identical buffer; a mismatch authenticates nothing and is miserable to
diagnose, so this layout is fixture-tested on both sides.

**`session.auth`** — res, `Ready` payload:
```json
{
  "role": "deck",
  "protocol": 1,
  "engine_version": "0.1.0",
  "host_platform": "linux",
  "active_profile_id": "p_default",
  "capabilities": {
    "text_unicode": false,
    "media_keys": true,
    "mouse": true,
    "shell_actions": false
  }
}
```

`role` ∈ `deck` | `admin`. `host_platform` ∈ `windows` | `macos` | `linux`.

`capabilities` reports what this host can actually do right now, so the client can grey out
buttons whose action is unavailable instead of letting them fail at press time:

| Key | False when |
| --- | --- |
| `text_unicode` | `input.text` cannot inject arbitrary Unicode — notably Linux/uinput, see `docs/ENGINE.md` §4.3 |
| `media_keys` | the backend cannot emit media keys |
| `mouse` | the backend cannot emit mouse events |
| `shell_actions` | shell execution is disabled (`docs/ARCHITECTURE.md` §5.5) |

A `deck` never needs `settings.get`; everything it must know is here.

### 4.2 `pair.*`

`pair.request` is callable only while the engine is in pairing mode, by an unauthenticated
socket. The other `pair.*` ops are admin only.

**`pair.request`** — req
```json
{
  "code": "402913",
  "device_pubkey": "7mK3nQ9vR2xT5bJ8cH1sD4gF6wY0aZ3eU7iL5oP4tM0=",
  "device_name": "Cipher's iPad",
  "platform": "ios",
  "proof": "B4hN7kR0vX2mQ5tJ8cF1sD6gW3yZ9aU4eL7iO2pK5T8="
}
```

`proof` is an Ed25519 signature by the device private key over:

```
message = b"muxdeck-pair-v1"           (15 ASCII bytes)
        || code                        (UTF-8 bytes of the 6-digit string, e.g. "402913")
        || device_pubkey               (32 raw bytes, as base64-decoded)
```

This proves the device holds the private half of the key it is registering. Without it, anyone
who reads the QR could register a public key they do not control. A `proof` that does not verify
is `BAD_SIGNATURE`.

**`pair.request`** — res
```json
{ "device_id": "d_7f3a91c2b4e05d18", "host_id": "h_a91c4d2e8f019b37",
  "host_name": "ENIGMA-ENTROPY" }
```

**`pair.begin`** — req, **admin only**. Opens a pairing window.
```json
{ "ttl_seconds": 120 }
```
`ttl_seconds` is clamped to `30..=300`; the default when omitted is `120`. A value outside that
range is `BAD_REQUEST`, not silently coerced.

res:
```json
{ "code": "402913", "expires_at": 1785312000, "qr_payload": "muxdeck://pair?addr=..." }
```

**`pair.cancel`** — req, admin only. `{}` → `{}`
**`pair.list_devices`** — req, admin only. `{}` →
```json
{ "devices": [
  { "device_id": "d_7f3a91c2b4e05d18", "name": "Cipher's iPad", "platform": "ios",
    "paired_at": 1785300000, "last_seen": 1785311900, "connected": true }
] }
```
**`pair.revoke`** — req, admin only. `{ "device_id": "d_7f3a91c2b4e05d18" }` → `{}`
Revoking closes any live socket for that device immediately.

### 4.3 `input.*` — role `deck` and `admin`

**`input.key_combo`**
```json
{ "keys": ["CONTROL", "SHIFT", "ESCAPE"], "hold_ms": 0 }
```
Modifiers are pressed in listed order, the final non-modifier key is tapped, then all are
released in reverse order. `hold_ms` holds **the entire combo** — every key down — before
releasing in reverse order. Key names use the canonical table in §5.

Edge cases:

- **Zero non-modifiers is valid.** `["META"]` alone presses and releases META; this is a real
  macro.
- **Two or more non-modifiers is `BAD_REQUEST`.** `["A","B"]` is almost always a mistake, and
  `input.key_sequence` exists for the deliberate case.
- **An empty `keys` array is `BAD_REQUEST`.**

**`input.key_sequence`** — several combos in order.
```json
{ "steps": [ { "keys": ["CONTROL","C"] }, { "delay_ms": 50 }, { "keys": ["CONTROL","V"] } ] }
```
Each step is either `{ "keys": [...] , "hold_ms": 0 }` or `{ "delay_ms": n }`. Each `keys` step
obeys the `input.key_combo` rules above.

**`input.text`** — type a literal string (Unicode-safe; uses scancode-free unicode injection).
```json
{ "text": "muxdeck", "delay_ms": 0 }
```
`delay_ms` is the pause between characters in milliseconds; `0` means as fast as the OS allows.
Hosts reporting `capabilities.text_unicode == false` reject non-representable characters with
`INJECTION_FAILED`.

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

| Action | Fields | Units |
| --- | --- | --- |
| `move_rel` | `dx`, `dy` (int) | physical pixels, relative to the current cursor position |
| `move_abs` | `x`, `y` (float) | normalised `0.0..1.0` across the **primary monitor**, origin top-left |
| `click` / `down` / `up` | `button` | `left` \| `right` \| `middle` |
| `scroll` | `dx`, `dy` (float) | notches; `1.0` is one detent |

`move_abs` is normalised because the client has no idea what resolution the host runs. The engine
converts notches per platform — ×120 on Windows, one line on macOS and Linux.

All `input.*` responses are `{}` on success, or an `err` with `INJECTION_FAILED`.

### 4.4 `action.*`

**`action.run`** — role `deck` and `admin`. Runs a **named**, pre-defined action. The client
never sends a command string.
```json
{ "action_id": "a_obs_scene_gaming" }
```
Returns `err` `DISABLED` if the shell feature is off, `NOT_FOUND` if the ID is unknown.

**`action.list`** — role `deck` and `admin`. Returns defined actions, so a deck can label and
gate its own buttons.
**`action.set`** / **`action.delete`** — admin only. Defining an action requires the shell
feature to be enabled.

### 4.5 `profile.*`

A **profile** is one deck layout: a grid of pages of buttons.

**`profile.get`** — `{ "profile_id": "p_default" }` → a Profile object (§6).
**`profile.list`** → `{ "profiles": [ { "id": "...", "name": "...", "active": true } ] }`
**`profile.subscribe`** — `{}` → `{}`. Thereafter the engine pushes `evt profile.changed`.
**`profile.activate`** — role `deck` and `admin`. `{ "profile_id": "p_stream" }` → `{}`
**`profile.set`** — admin only. `{ "profile": { ...Profile } }` → `{}`
**`profile.delete`** — admin only. `{ "profile_id": "p_old" }` → `{}`

`profile.activate` is deck-callable deliberately: a device that can already inject arbitrary
keystrokes gains nothing by choosing which grid it displays, and "switch to my streaming profile"
is a table-stakes deck button.

**`profile.set` validation.** The engine rejects with `BAD_REQUEST` and a specific `message` —
never last-write-wins, never silent coercion — on any of:

- two buttons at the same `pos`
- a `pos` outside the profile's `grid`
- an empty `pages` array
- a duplicate button `id` or page `id`
- an `on_tap` / `on_long_press` op the calling role may not invoke
- an unknown op in `on_tap` / `on_long_press`

### 4.6 `settings.*` — admin only

**`settings.get`** → 
```json
{
  "port": 47654, "host_name": "ENIGMA-ENTROPY",
  "shell_actions_enabled": false, "telemetry_enabled": true,
  "telemetry_interval_ms": 1000, "autostart": true
}
```
**`settings.set`** — partial object of the same shape → `{ "restart_required": bool }`.
The response always carries `restart_required`; it is `true` when a changed field (such as
`port`) needs a daemon restart to take effect.

### 4.7 `telemetry.*` — role `deck` and `admin`

**`telemetry.subscribe`** — `{}` → `{}`. Thereafter the engine pushes `evt telemetry.update` on
the interval in `settings.telemetry_interval_ms`, for as long as `settings.telemetry_enabled` is
true. Telemetry has its own subscription; it is not implied by `profile.subscribe`.

### 4.8 `system.ping`

**`system.ping`** — req `{ "t_client": 1785311999123 }` → res `{ "t_client": 1785311999123, "t_engine": 1785311999130 }`
The client computes RTT locally from its own send and receive timestamps; it does not trust
`t_engine` for clock sync, only for one-way-delay estimation. There is no `pong` op — the `res`
to `system.ping` is the pong.

### 4.9 Events (`t: "evt"`, no `id`)

| `op` | `d` | Delivered to |
| --- | --- | --- |
| `profile.changed` | `{ "profile": { ...Profile } }` | sockets that called `profile.subscribe` |
| `telemetry.update` | `{ "ts": 1785312000, "cpu_pct": 14.5, "ram_pct": 58.2 }` | sockets that called `telemetry.subscribe` |
| `device.changed` | `{ "devices": [ ...as pair.list_devices ] }` | `admin` sockets only |
| `pairing.state` | `{ "active": true, "expires_at": 1785312120 }` | `admin` sockets only |
| `engine.shutdown` | `{ "reason": "user_requested" }` | every authenticated socket |

`engine.shutdown` `reason` is an enum, not free text:
`user_requested` | `settings_changed` | `fatal_error`.

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

`CONTROL`, `SHIFT`, `ALT` and `META` are the modifiers for the purposes of §4.3's combo rules;
everything else is a non-modifier.

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

- `icon` is a name from the curated icon map shipped in `packages/muxdeck_protocol`; unknown
  names fall back to a filled dot. See `docs/CLIENT.md` §5.
- `color` is `#RRGGBB`.
- `haptic` ∈ `none` | `light` | `medium` | `heavy`.
- `on_tap` / `on_long_press` are an embedded `{ op, d }` pair. The op must be one the sender's
  role is permitted to call — the engine re-checks at execution time, it does not trust the
  profile — and `profile.set` rejects one that the caller could not invoke (§4.5).
- Buttons are sparse: a grid cell with no button is empty.

### Action
```json
{ "id": "a_obs_scene_gaming", "name": "OBS: Gaming scene",
  "command": "obs-cli", "args": ["scene","switch","--name","Gaming"], "cwd": null }
```
`command` and `args` are separate — the engine never passes a string to a shell interpreter.

## 7. Versioning

`v` is the **major** version and appears in every message. A breaking change increments it and
the engine must then accept both for at least one release. Additive, optional fields do **not**
bump `v`.

The mDNS TXT `v` record is a **comma-separated list of supported majors** — `v=1` today,
`v=1,2` during a transition — because a single value cannot express "this host speaks both".
Clients pick the highest major they also support.

## 8. Fixtures

`protocol/fixtures/` holds one JSON file per message shape, named `<op>.<t>.json`, e.g.
`input.key_combo.req.json`, `session.hello.req.json`, `session.hello.res.json`. Where a single
op has more than one payload shape — `session.hello` has a deck form and an admin form — a
variant suffix disambiguates: `session.hello.req.admin.json`, `session.hello.res.admin.json`.

Both the Rust and the Dart test suites must deserialise every fixture, re-serialise it, and
assert semantic equality. A protocol change without a fixture change is incomplete.

The `session.auth` and `pair.request` signing layouts (§4.1, §4.2) are fixture-tested on both
sides as raw byte buffers, not just as JSON — an encoding disagreement there fails silently at
runtime.
