# MuxDeck — Architecture

## 1. Goal

Turn an Android phone, iPhone, or iPad into a customisable macro deck for a Windows, macOS,
or Linux desktop, over the local Wi-Fi network, with input latency low enough that a button
press feels physical (target: **< 25 ms** end to end on a normal home network).

## 2. Components

```
        ┌──────────────────────────────────────────────────────────┐
        │  muxdeck-client        Flutter · Android, iOS, iPadOS     │
        │  · button grid, haptics, RTT readout                      │
        │  · mDNS discovery, QR pairing                             │
        │  · role: "deck"                                           │
        └───────────────────────────┬──────────────────────────────┘
                                    │  WSS over LAN
                                    │  wss://<host>:47654/ws
                                    ▼
        ┌──────────────────────────────────────────────────────────┐
        │  muxdeck-engine        Rust daemon (muxdeckd)             │
        │                                                           │
        │  · TLS WebSocket server            · profile/layout store │
        │  · mDNS advertise _muxdeck._tcp    · device registry      │
        │  · pairing + Ed25519 auth          · telemetry sampler    │
        │  · input injection ────────────────────────┐              │
        └───────────────────────────▲────────────────┼──────────────┘
                                    │                ▼
                                    │      ┌────────────────────────┐
                 wss://127.0.0.1:47654/ws  │  OS input APIs         │
                 role: "admin"             │  Win:   SendInput      │
                                    │      │  macOS: CGEventPost    │
        ┌───────────────────────────┴────┐ │  Linux: /dev/uinput    │
        │  muxdeck-server                │ └────────────────────────┘
        │  Flutter Desktop · Win/mac/Linux│
        │  · tray icon + status           │
        │  · QR pairing screen            │
        │  · visual layout editor         │
        │  · installs/starts the daemon   │
        └────────────────────────────────┘
```

## 3. The key structural decision: the engine owns everything

The desktop GUI is **not** a supervisor that spawns and babysits the engine. The engine is a
standalone background daemon that owns the WebSocket server, the device registry, the profile
store, and input injection. The desktop GUI is simply **another WebSocket client** connecting
to `127.0.0.1` with the `admin` role.

Why this matters:

- **The deck keeps working when the GUI is closed.** You configure once and quit the window;
  your buttons still fire. A supervisor model makes the GUI load-bearing for no reason.
- **The GUI reuses the client's protocol code verbatim.** Both are Dart, both depend on
  `packages/muxdeck_protocol`, both speak the same ops. Only the role differs.
- **No bespoke IPC.** No stdout parsing, no named pipes, no sidecar lifecycle bugs. There is
  exactly one interface into the engine and it is already specified and already tested.
- **Remote administration comes free** later if you ever want it, because `admin` is a role,
  not a transport.

The GUI's only privileged job is *installation*: registering the daemon to auto-start
(Windows scheduled task / launchd user agent / systemd user unit) and starting it if it is not
already running.

## 4. Why Wi-Fi only

USB and BLE were both considered and cut.

- **BLE** has a 7.5 ms minimum connection interval and real-world round trips of 20–100 ms with
  periodic stalls — worse than Wi-Fi for the one thing that matters. iOS additionally blocks
  Bluetooth Classic SPP without MFi certification.
- **USB** would mean bundling `usbmuxd`/`iproxy` and a pile of native DLLs for iOS, and
  requiring `adb` plus USB debugging for Android. That is a large amount of surface area and a
  rough end-user story for a marginal latency win over a modern LAN.

Dropping both removes native tooling from the repo entirely and lets the client be a pure
Dart application. If USB is ever revisited, the client's `Transport` abstraction (see
`docs/CLIENT.md`) is the seam to add it at.

**Consequence to design around:** a phone on a guest VLAN, on cellular, or on a network with
AP isolation will not reach the host. Discovery failure must produce a clear, specific error
message with a manual `host:port` entry fallback — not a spinner.

## 5. Security model

The threat model is a shared home or office Wi-Fi network. Anyone on that network can reach
the engine's port. The engine injects arbitrary keystrokes, so an unauthenticated connection
is a full host compromise.

### 5.1 Identity

On first run the engine generates and stores, in the OS config directory:

- an **Ed25519 host identity keypair**
- a **self-signed TLS certificate** for the host, valid 10 years
- a **local admin token** (see §5.4)

Each client device generates its own **Ed25519 device keypair** on first launch. Private keys
never leave the device that generated them.

The host is identified by two strings, both derived from the host public key and both with
exactly one representation everywhere they appear:

```
host_id     = "h_" + first 16 hex chars of SHA-256(host_public_key_bytes)
fingerprint = lowercase hex, no separators, SHA-256 over the leaf certificate DER (64 chars)
```

Device IDs follow the same rule: `device_id = "d_" + first 16 hex chars of
SHA-256(device_public_key_bytes)`.

The certificate carries SANs for `localhost`, `127.0.0.1`, `::1`, and every non-loopback local IP
present when it is generated; it is regenerated if the host's addresses change.

> **Clients authenticate the host by certificate fingerprint pinning only.** Hostname and CA
> validation are deliberately bypassed, because the host has no DNS name and no CA. The SANs
> exist for tooling convenience, not for trust. Do not "fix" this by enabling hostname
> validation — it will break every installation.

### 5.2 Pairing (once per device)

1. In the desktop control panel the user clicks **Add device**. The engine enters pairing mode
   for 120 seconds and generates a random 6-digit one-time code.
2. The panel displays a QR code encoding:
   `muxdeck://pair?addr=192.168.1.42:47654&host=<host_id>&fp=<fingerprint>&code=<otp>`
   where `host` and `fp` are exactly the strings defined in §5.1.
3. The client scans it (or the user types `addr` + `code` manually).
4. The client opens a TLS connection and **verifies the presented certificate's SHA-256
   fingerprint equals `fp` from the QR** (lowercase hex, §5.1). Mismatch aborts. This is
   trust-on-first-use, but the fingerprint arrives out of band via the QR, so it is not blind
   TOFU.
5. The client sends `pair.request` with its device public key, device name, the OTP, and a
   **proof of possession** — an Ed25519 signature by the device private key over
   `b"muxdeck-pair-v1" || otp || device_pubkey`. Without it, anyone who read the QR could
   register a public key they do not hold the private half of.
6. The engine verifies the OTP and the proof, stores the device public key in its registry, and
   returns a device ID. Pairing mode closes.

### 5.3 Session authentication (every connect)

No shared secret is ever transmitted after pairing.

1. Client connects, verifies the pinned certificate fingerprint stored at pairing time.
2. Client sends `session.hello` with its device ID.
3. Engine responds `mode: "challenge"`, carrying a 32-byte random nonce.
4. Client signs `b"muxdeck-session-v1" || nonce || device_id || host_id` with its device private
   key and sends `session.auth`. The domain prefix and the `host_id` stop a signature captured
   against one host being replayed at another.
5. Engine verifies the signature against the stored public key and responds with a `Ready`
   payload.

The panel's loopback path (§5.4) sends the same `session.hello` op and gets `mode: "ready"` back
immediately, skipping steps 3–5.

There are only two session ops, `session.hello` and `session.auth`; `Challenge` and `Ready` are
payload shapes on their responses, not ops of their own. Responses echo the op of the request
they answer, and the `session.hello` response is an internally-tagged union on `mode` so the
branch is read from the tag rather than inferred from which fields are present — see
`docs/PROTOCOL.md` §2 and §4.1.

This is the `deck` path. The local control panel authenticates differently — see §5.4.

Unauthenticated sockets may send only `session.*` and `pair.*` ops. Anything else closes the
connection. A socket that has not authenticated within 10 seconds is closed.

### 5.4 Roles and capability gating

Two roles: `deck` for paired devices, `admin` for the local control panel.

#### How `admin` is obtained

`admin` cannot be requested. The engine grants it if and only if **the peer address is loopback
(`127.0.0.1`/`::1`) AND the connection presents the local admin token**.

On first run the engine writes `admin.token` into the config directory: 32 random bytes, base64,
file mode `0600` on Unix and a current-user-only DACL on Windows. A loopback client sends it in
`session.hello` instead of a `device_id`, and receives a `mode: "ready"` response directly —
there is no challenge round trip and no `session.auth`. The 10-second unauthenticated-socket
timeout still applies.

`session.hello` therefore carries either `device_id` (deck, leads to a challenge) or
`admin_token` (panel, immediately ready). Exactly one; both or neither is `BAD_REQUEST`.

**This is an intentional design decision, not an oversight — do not "harden" it into a challenge
handshake, which would break the panel.** The panel cannot pair itself, because opening a pairing
window is itself an admin operation; a token file is what breaks that chicken-and-egg. Loopback
alone would be insufficient: on a multi-user desktop a second logged-in user can also reach
`127.0.0.1`. Reading `admin.token` requires being the same OS user, which is the actual trust
boundary.

#### Capability matrix

Every op in `docs/PROTOCOL.md` §4 appears here exactly once. `docs/ENGINE.md` §8 tests against
this table, so it must stay exhaustive.

| Op | pre-auth | `deck` | `admin` |
| --- | :---: | :---: | :---: |
| `session.hello` | ✅ | — | — |
| `session.auth` | ✅ | — | — |
| `pair.request` | ✅ (pairing window only) | — | — |
| `pair.begin` | ❌ | ❌ | ✅ |
| `pair.cancel` | ❌ | ❌ | ✅ |
| `pair.list_devices` | ❌ | ❌ | ✅ |
| `pair.revoke` | ❌ | ❌ | ✅ |
| `system.ping` | ❌ | ✅ | ✅ |
| `input.key_combo` | ❌ | ✅ | ✅ |
| `input.key_sequence` | ❌ | ✅ | ✅ |
| `input.text` | ❌ | ✅ | ✅ |
| `input.media` | ❌ | ✅ | ✅ |
| `input.mouse` | ❌ | ✅ | ✅ |
| `action.run` | ❌ | ✅ | ✅ |
| `action.list` | ❌ | ✅ | ✅ |
| `action.set` | ❌ | ❌ | ✅ |
| `action.delete` | ❌ | ❌ | ✅ |
| `profile.get` | ❌ | ✅ | ✅ |
| `profile.list` | ❌ | ✅ | ✅ |
| `profile.subscribe` | ❌ | ✅ | ✅ |
| `profile.activate` | ❌ | ✅ | ✅ |
| `profile.set` | ❌ | ❌ | ✅ |
| `profile.delete` | ❌ | ❌ | ✅ |
| `telemetry.subscribe` | ❌ | ✅ | ✅ |
| `settings.get` | ❌ | ❌ | ✅ |
| `settings.set` | ❌ | ❌ | ✅ |

Events by role: `profile.changed`, `telemetry.update` and `engine.shutdown` go to any subscribed
socket. `device.changed` and `pairing.state` go to `admin` sockets only.

`profile.activate` is deck-callable on purpose: a device that can already inject arbitrary
keystrokes is not escalating by choosing which grid it displays, and "switch to my streaming
profile" is a table-stakes deck button that must work without the panel running.

### 5.5 Shell execution

`action.run` (execution of user-defined shell commands) is **disabled by default** and can only be enabled
from the control panel, with an explicit warning. When enabled, commands must match an
allow-list of user-defined named actions — the client sends an action *name*, never a raw
command string. This is the single largest footgun in a project like this and is deliberately
locked down.

## 6. Discovery

The engine advertises `_muxdeck._tcp.local.` with TXT records:

| Key | Meaning |
| --- | --- |
| `v` | comma-separated list of supported protocol majors, e.g. `1` or `1,2` |
| `id` | host ID — the full `h_…` string from §5.1, e.g. `h_a91c4d2e8f019b37` |
| `name` | friendly host name, e.g. `ENIGMA-ENTROPY` |
| `fp` | TLS certificate fingerprint, lowercase hex per §5.1 |

`v` is a list, not a single value, because a single value cannot express "this host speaks both
majors" during a transition release. Clients pick the highest major they also support.

`id` carries the same string the protocol uses, so a client matches a stored host to a discovery
result by plain string equality — there is no second representation to normalise.

Clients browse for this service to populate the "hosts found" list. Because `fp` is in the TXT
record, a previously-paired client can confirm it is talking to the same host even if the IP
changed. Manual `host:port` entry must always remain available as a fallback.

## 7. Latency budget

| Segment | Target |
| --- | --- |
| touch → client frame handled | < 4 ms |
| client serialise + send | < 1 ms |
| LAN round trip | 3–12 ms |
| engine parse + dispatch | < 1 ms |
| OS input injection | 1–3 ms |
| **total, press to keystroke** | **< 25 ms** |

The client measures true RTT with `system.ping` and its response, and displays it. Any regression above budget
is a bug, not a tuning opportunity.

JSON is used on the wire deliberately: at a few messages per second it costs nothing measurable
and it makes the whole system debuggable with `websocat`. Do not "optimise" to a binary codec
without a measurement showing JSON is the bottleneck.

## 8. Failure behaviour

- **Client loses connection:** exponential backoff reconnect (0.5 s → 8 s cap), grey out the
  grid, keep the last known layout on screen so it snaps back instantly.
- **Engine not running:** control panel offers a one-click start; mobile client shows
  "host found but not responding" distinctly from "host not found".
- **Host IP changed:** client re-resolves via mDNS using the stored host ID, re-pins by
  fingerprint, reconnects without re-pairing.
- **Injection failure** (e.g. Linux `/dev/uinput` permission denied): engine returns a
  structured error the client surfaces as a toast, and logs the remediation step.

## 9. Known platform hazards

- **iOS 14+** requires `NSLocalNetworkUsageDescription` and `NSBonjourServices` in `Info.plist`
  or mDNS silently returns nothing. This is the number one cause of "discovery doesn't work".
- **Android** needs `INTERNET` and `CHANGE_WIFI_MULTICAST_STATE`; multicast must be held via a
  `MulticastLock` while browsing.
- **Linux/Wayland** does not permit global synthetic input from an ordinary client. `/dev/uinput`
  works because it is kernel-level, but requires the user to be in the `input` group or a udev
  rule to be installed. The control panel must detect and guide through this.
- **macOS** requires the user to grant Accessibility permission to the daemon before
  `CGEventPost` does anything. Detect it and deep-link to the settings pane.
- **Flutter Linux desktop** tray support depends on `libayatana-appindicator3`; document it as
  a package dependency.
