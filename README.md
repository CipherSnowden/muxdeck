# MuxDeck

An open-source Stream Deck alternative. A phone or tablet runs a grid of buttons; a daemon on your
desktop receives the presses over your local network and injects real keyboard, mouse and media
input into the host OS.

**Status: alpha.** Everything in [`docs/BUILD-PLAN.md`](docs/BUILD-PLAN.md) M0–M9 is built and
tested. Windows and Linux input injection is verified; **macOS is compiled but has never been
observed injecting a keystroke** — there is no Mac on the development machine. See
[Known gaps](#known-gaps).

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

## Install

Grab the build for your platform from
[Releases](https://github.com/CipherSnowden/muxdeck/releases). The desktop download contains both
the control panel and the `muxdeckd` daemon; the panel starts the daemon for you.

Nothing here is code-signed. This is a personal open-source project with no Apple Developer
Program membership and no Windows code-signing certificate, so every operating system will warn
you at least once.

### Windows

Unzip anywhere and run `muxdeck_server.exe`.

SmartScreen will say the publisher is unknown — *More info* → *Run anyway*. No permission setup is
needed: `SendInput` works for any interactive process. The one limit is that a non-elevated daemon
cannot inject into an elevated window, so a button pressed while an administrator app has focus
reports a failure rather than silently doing nothing.

### macOS

Unzip and move `muxdeck_server.app` to Applications. It is unsigned and unnotarised, so Gatekeeper
blocks the first launch:

```sh
xattr -dr com.apple.quarantine /Applications/muxdeck_server.app
```

Then — and this is the step people miss — **grant Accessibility permission**:

> System Settings → Privacy & Security → Accessibility → **+** → add MuxDeck → turn it on

Without it `CGEventPost` returns success and does nothing, so your deck connects, the buttons
light up, and not a single keystroke arrives. The panel's dashboard says so in red when it detects
this. The permission is remembered per application binary, so moving or replacing the app means
granting it again.

### Linux

```sh
tar -xzf muxdeck-panel-linux-x64.tar.gz
./muxdeck_server
```

Two prerequisites:

```sh
# The tray icon. Without it the panel runs but has no tray presence.
sudo apt install libayatana-appindicator3-1

# Input injection. /dev/uinput is root-only by default.
sudo usermod -aG input $USER
```

**The group change needs a fresh login** — not a restart of the app, an actual log out and back
in. If `/dev/uinput` still is not writable afterwards, run `muxdeckd service install`; it prints a
udev rule and the exact `install`/`udevadm` commands to apply it, rather than escalating on its
own where you cannot audit it.

Input on Linux is keyboard and media keys only. Typing arbitrary text and mouse control are
reported as unavailable and the deck greys those buttons out — see [Known gaps](#known-gaps).

### Android

Install `muxdeck-client-android.apk`. It is signed with a debug key, so Android asks you to allow
installation from unknown sources.

### iOS and iPadOS

The IPA is **unsigned**. Sideload it with AltStore, Sideloadly, or your own developer certificate.
There is no App Store build and no TestFlight, and there will not be one without a paid developer
account.

## Pairing

1. Open the panel and choose **Pair a device**. A QR code and a six-digit code appear, with a
   visible countdown.
2. Open the app, tap **Pair new device**, and scan the code. Or type the address and the six
   digits by hand if the camera is not an option.
3. That is it. The device is remembered; the pairing window closes after one device or when it
   expires.

Both devices must be on the same network. MuxDeck is Wi-Fi only, deliberately —
[`docs/TRANSPORT.md`](docs/TRANSPORT.md) explains why, including why the project's earlier
USB-first design could not have worked.

## Screenshots

None yet. Capturing them needs a running panel and a real tablet, which is a manual step — see
[`docs/screenshots/README.md`](docs/screenshots/README.md) for the four that belong here.

## Why it is built this way

- **The daemon owns everything.** The desktop control panel is just another WebSocket client
  connecting over loopback with an `admin` role. Close the panel and your deck keeps working.
- **Wi-Fi/LAN only, deliberately.** No USB, no Bluetooth. See
  [`docs/TRANSPORT.md`](docs/TRANSPORT.md).
- **The wire protocol is the contract.** [`docs/PROTOCOL.md`](docs/PROTOCOL.md) is the single
  source of truth; the Rust and Dart types are implementations of it, and both test suites assert
  against the same JSON fixtures in `protocol/fixtures/`.
- **A LAN deck is a remote code execution surface**, so pairing is out-of-band via QR, the host is
  authenticated by certificate fingerprint pinning, every session is an Ed25519 challenge, and
  shell execution is off by default. See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) §5.

### Shell actions

A deck button can run a program on your desktop. This is off by default and has to be turned on in
the panel, behind a warning, because it is the largest footgun in a project like this.

When it is on, a device sends an action *name* and never a command string; the action itself is
defined on the desktop, with the program and its arguments as **separate fields** so nothing is
ever handed to a shell interpreter. An argument containing `; rm -rf ~` is an argument.

## Performance

Measured on the development machine, release build, 200 samples over a loopback TLS WebSocket:

| Segment | Budget | p50 | p95 |
| --- | --- | --- | --- |
| engine dispatch + OS injection | < 4 ms | 0.62 ms | 1.21 ms |

Against the 25 ms press-to-keystroke budget in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) §7,
that leaves the whole rest to the touch and the network. The client measures and displays real
round-trip time, so a regression is visible rather than a vague feeling that the deck got worse.

Reproduce it:

```powershell
cd engine
cargo test --release -p muxdeck-engine --test integration -- --ignored --nocapture measure_latency
```

## Known gaps

| | Compiles | Verified injecting |
| --- | :---: | :---: |
| Windows | ✅ | ✅ |
| Linux | ✅ | ✅ (events read back off the device) |
| macOS | ✅ CI | ❌ **never observed** |

- **macOS is unverified.** There is no Mac on the development machine and a CI runner has no
  desktop session, so `macos/backend.rs` is written against Apple's documentation and proved only
  to compile. If you have a Mac and something does not work, start with the media keys — the
  `NSSystemDefined` path is the most intricate part.
- **Linux has no mouse support and cannot type arbitrary text.** A uinput device declares its
  capabilities at creation and this one is a keyboard; and uinput speaks key codes, not characters,
  so what a code produces depends on a layout a daemon cannot read under Wayland. Both are reported
  through the `capabilities` block, so affected buttons are visibly greyed out rather than failing
  when pressed.
- **macOS cannot express Print Screen, Scroll Lock, Pause, Menu or F21–F24.** No Apple keyboard has
  ever had them and no virtual key code means them, so those buttons refuse with a message instead
  of firing something else.
- The layout editor does not yet support drag-to-move between cells, multiple pages per profile, or
  activating a profile from the grid. The protocol and the engine support all three.

## Layout

| Path | What |
| --- | --- |
| `engine/` | Rust workspace — `muxdeckd`, the daemon that does the actual work |
| `apps/client/` | Flutter mobile deck — Android, iOS, iPadOS |
| `apps/server/` | Flutter desktop control panel — Windows, macOS, Linux |
| `packages/muxdeck_protocol/` | Dart protocol types, shared by both Flutter apps |
| `protocol/fixtures/` | Canonical JSON message samples, parsed by both test suites |
| `docs/` | Architecture, protocol, per-component specs, build plan |

## Building from source

Requires Rust stable and [FVM](https://fvm.app/). All Flutter and Dart commands go through `fvm`.

```powershell
# Daemon
cd engine
cargo test --workspace
cargo run -p muxdeckd -- --log-level debug --foreground

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

Build prerequisites beyond the runtime ones above:

- **Windows** — Visual Studio with the *Desktop development with C++* workload, for the MSVC
  linker Rust needs and for the desktop app's build.
- **Linux** — `clang cmake ninja-build pkg-config libgtk-3-dev liblzma-dev
  libayatana-appindicator3-dev`.
- **macOS** — Xcode command line tools.

Releases are cut by pushing a `v*` tag; `.github/workflows/release.yml` builds all six artefacts
and opens a draft release.

## Contributing

Read [`CLAUDE.md`](CLAUDE.md) first — it holds the hard constraints, and they are hard for reasons
documented in `docs/`. In particular: no USB or Bluetooth transports, no web platform, and
`docs/PROTOCOL.md` changes before any code that implements them.

## License

MIT — see [`LICENSE`](LICENSE).
