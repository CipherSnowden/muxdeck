# MuxDeck

An open-source Stream Deck alternative. A phone or tablet runs a grid of buttons; a daemon on your
desktop receives the presses over your local network and injects real keyboard, mouse and media
input into the host OS.

**Status: pre-alpha.** Scaffolding only — see [`docs/BUILD-PLAN.md`](docs/BUILD-PLAN.md) for what
works and what does not.

```
  phone / tablet                       your desktop
 ┌────────────────┐                 ┌──────────────────────┐
 │ muxdeck client │ ── wss://LAN ──▶│ muxdeckd (Rust)      │──▶ SendInput
 │ Flutter        │                 │ TLS + Ed25519 auth   │    CGEventPost
 │ Android · iOS  │                 │ mDNS · pairing       │    /dev/uinput
 └────────────────┘                 └──────────▲───────────┘
                                               │ loopback, admin role
                                    ┌──────────┴───────────┐
                                    │ muxdeck control panel│
                                    │ Flutter Desktop      │
                                    └──────────────────────┘
```

## Why it is built this way

- **The daemon owns everything.** The desktop control panel is just another WebSocket client
  connecting over loopback with an `admin` role. Close the panel and your deck keeps working.
- **Wi-Fi/LAN only, deliberately.** No USB, no Bluetooth. The reasoning — including why the
  project's earlier USB-first design could not have worked — is in
  [`docs/TRANSPORT.md`](docs/TRANSPORT.md).
- **The wire protocol is the contract.** [`docs/PROTOCOL.md`](docs/PROTOCOL.md) is the single
  source of truth; the Rust and Dart types are implementations of it, and both test suites assert
  against the same JSON fixtures in `protocol/fixtures/`.
- **A LAN deck is a remote code execution surface**, so pairing is out-of-band via QR, the host is
  authenticated by certificate fingerprint pinning, every session is an Ed25519 challenge, and
  shell execution is off by default. See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) §5.

## Layout

| Path | What |
| --- | --- |
| `engine/` | Rust workspace — `muxdeckd`, the daemon that does the actual work |
| `apps/client/` | Flutter mobile deck — Android, iOS, iPadOS |
| `apps/server/` | Flutter desktop control panel — Windows, macOS, Linux |
| `packages/muxdeck_protocol/` | Dart protocol types, shared by both Flutter apps |
| `protocol/fixtures/` | Canonical JSON message samples, parsed by both test suites |
| `docs/` | Architecture, protocol, per-component specs, build plan |

## Building

Requires Rust stable and [FVM](https://fvm.app/). All Flutter and Dart commands go through `fvm`.

```powershell
# Daemon
cd engine
cargo test --workspace
cargo run -p muxdeckd -- --log-level debug

# Shared protocol package
cd packages/muxdeck_protocol
fvm dart test

# Mobile deck
cd apps/client
fvm flutter pub get && fvm flutter run -d <device_id>

# Desktop control panel
cd apps/server
fvm flutter pub get && fvm flutter run -d windows
```

### Platform prerequisites

- **Windows** — Visual Studio with the *Desktop development with C++* workload, for the MSVC
  linker Rust needs and for the desktop app's build.
- **Linux** — `libayatana-appindicator3-dev` for the control panel's tray icon, and your user must
  be in the `input` group for `/dev/uinput` access. The panel detects the latter and tells you.
- **macOS** — the daemon needs Accessibility permission before it can inject anything. The panel
  detects this and deep-links to the settings pane.

## Contributing

Read [`CLAUDE.md`](CLAUDE.md) first — it holds the hard constraints, and they are hard for
reasons documented in `docs/`. In particular: no USB or Bluetooth transports, no web platform, and
`docs/PROTOCOL.md` changes before any code that implements them.

## License

MIT — see [`LICENSE`](LICENSE).
