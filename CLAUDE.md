# CLAUDE.md — MuxDeck

This file is loaded automatically by Claude Code on every session in this repo.
Read it fully before acting. Keep it accurate: when a decision here changes, update
this file in the same commit as the code.

---

## What MuxDeck is

An open-source Stream Deck alternative. A phone or tablet runs a grid of buttons; a
daemon on the desktop receives button presses over the local network and injects real
keyboard, mouse, and media input into the host OS.

**Author org identifier:** `in.redoimagined`
**License:** MIT

---

## Hard constraints — do not violate without being asked

1. **Wi-Fi / LAN only.** There is no USB transport and no Bluetooth transport. Do not
   add `usbmuxd`, `iproxy`, `libimobiledevice`, `adb reverse`, BLE, or any USB tooling.
   If a task seems to need it, stop and ask. The rationale, the cost of revisiting, and why the
   project's earlier USB-first design could not have worked are in `docs/TRANSPORT.md` — read it
   before proposing a transport change.
2. **No web platform.** The Flutter client targets Android + iOS/iPadOS only. The Flutter
   desktop app targets Windows + macOS + Linux only. Never run
   `flutter create --platforms=...web` or add web support. (Reason: browsers cannot pin a
   self-signed TLS certificate, which the security model depends on.)
3. **All Flutter/Dart commands are prefixed with `fvm`.** `fvm flutter ...`, `fvm dart ...`.
   Never call bare `flutter` or `dart`.
4. **`docs/PROTOCOL.md` is the single source of truth for the wire format.** Never invent,
   rename, or silently change a message op. To change the protocol: edit `docs/PROTOCOL.md`
   first, then `protocol/fixtures/`, then the Rust types, then the Dart types — in that order,
   in one commit.
5. **Never commit secrets, keys, certificates, or device tokens.** These live in the OS
   config directory at runtime, never in the repo. `protocol/fixtures/` may contain only
   obviously-fake test values.
6. **No `unwrap()` / `expect()` in Rust outside tests and `main()` startup.** Use `anyhow`
   at the binary boundary and `thiserror` for library error types.

---

## Repository map

```
muxdeck/
├── CLAUDE.md                  <- you are here
├── README.md
├── LICENSE                    (MIT)
├── docs/
│   ├── ARCHITECTURE.md        system design, component boundaries, rationale
│   ├── PROTOCOL.md            wire protocol — SOURCE OF TRUTH
│   ├── ENGINE.md              Rust daemon spec
│   ├── CLIENT.md              Flutter mobile client spec
│   ├── SERVER.md              Flutter desktop control panel spec
│   ├── TRANSPORT.md           why Wi-Fi only; what USB would cost
│   └── BUILD-PLAN.md          milestones M0–M9 and per-milestone prompts
├── protocol/
│   └── fixtures/              canonical JSON message samples, parsed by BOTH test suites
├── engine/                    Rust cargo workspace  (VS Code profile: Rust)
│   ├── Cargo.toml
│   └── crates/
│       ├── muxdeck-core/      protocol types + serde, zero I/O
│       ├── muxdeck-input/     input injection trait + win/mac/linux backends
│       ├── muxdeck-engine/    library: TLS WS server, pairing, auth, store, mDNS
│       └── muxdeckd/          binary: the daemon
├── apps/
│   ├── client/                Flutter, android+ios   (VS Code profile: Flutter Mobile)
│   └── server/                Flutter, win+mac+linux (VS Code profile: Flutter Desktop)
├── packages/
│   └── muxdeck_protocol/      Dart package — protocol types, shared by client + server
├── tools/
└── .github/workflows/
```

**Open each of `engine/`, `apps/client/`, `apps/server/` as its own VS Code window** with its
matching profile. The monorepo exists so the protocol has one home, not so everything is
edited at once.

---

## Which doc to read before which task

| Task touches | Read first |
| --- | --- |
| any message, op, or field on the wire | `docs/PROTOCOL.md` |
| `engine/**` | `docs/ENGINE.md`, then `docs/PROTOCOL.md` |
| `apps/client/**` | `docs/CLIENT.md`, then `docs/PROTOCOL.md` |
| `apps/server/**` | `docs/SERVER.md`, then `docs/PROTOCOL.md` |
| pairing, tokens, TLS, trust | `docs/ARCHITECTURE.md` §Security |
| USB, BLE, or anything below `Transport` | `docs/TRANSPORT.md` |
| "what do I build next" | `docs/BUILD-PLAN.md` |

---

## Toolchain

Pinned versions this repo is developed against. Verified on the dev machine, not assumed.

| Tool | Version | Notes |
| --- | --- | --- |
| Rust | 1.97.1 stable | target `x86_64-pc-windows-msvc`; `rustfmt` + `clippy` installed |
| Flutter | 3.44.8 (`stable`) | always via `fvm`; bare `flutter` is deliberately not on `PATH` |
| Dart | 3.12.2 | ships with the above |
| FVM | 4.1.2 | installed via Chocolatey |
| Visual Studio | Community **2026** (18.x) | `Desktop development with C++`; supplies the MSVC linker Rust needs and the toolchain `apps/server` builds against |
| GitHub CLI | 2.97.0 | authed as `CipherSnowden`, git protocol `ssh` |

There is no Go, no MinGW/GCC, no Python and no `ninja` in this project's toolchain, and none is
needed. If something appears to require one, that is a signal to re-read the spec, not to install it.

## Command cheat sheet

Development machine is **Windows** (PowerShell 7). macOS and Linux builds happen in CI.

**iOS/iPadOS is CI-build only.** There is no Mac on the dev machine, so `apps/client` can never be
built or run locally for iOS — only analyzed and unit/widget tested. iOS artefacts come from the
`macos-latest` runner, and releases ship an unsigned IPA for sideloading. Keep the iOS
configuration (`Info.plist`, `NSBonjourServices`) correct from the moment it is written; a mistake
there costs a CI round trip per attempt to find.

```powershell
# Engine
cd engine
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo run -p muxdeckd -- --log-level debug

# Shared Dart protocol package
cd packages/muxdeck_protocol
fvm dart test

# Mobile client
cd apps/client
fvm flutter pub get
fvm flutter analyze
fvm flutter test
fvm flutter run -d <device_id>

# Desktop control panel
cd apps/server
fvm flutter pub get
fvm flutter run -d windows
```

---

## Conventions

- **Commits:** Conventional Commits, scoped by area —
  `feat(engine): ...`, `fix(client): ...`, `docs(protocol): ...`, `chore(ci): ...`
- **Branches:** `main` is always green. Work on `feat/<short-name>`.
- **Rust:** edition 2021, `rustfmt` defaults, `clippy` clean at `-D warnings`.
- **Dart:** `package:lints/recommended.yaml` plus the repo's `analysis_options.yaml`;
  `fvm flutter analyze` must be clean.
- **Logging:** `tracing` in Rust, structured fields, never log tokens or key material.
- **Tests:** every protocol change adds or updates a fixture in `protocol/fixtures/` and is
  asserted by both the Rust and Dart test suites. Input-injection backends are behind a trait
  so engine logic is testable without touching the real OS.

---

## Working style for Claude Code in this repo

- Work milestone by milestone from `docs/BUILD-PLAN.md`. Do not skip ahead.
- Prefer small, reviewable commits over one large change.
- When a spec is ambiguous, **ask rather than guess** — especially about protocol shape,
  security behaviour, or anything touching OS permissions.
- Platform-specific code goes behind the `InputBackend` trait or `#[cfg(target_os)]`, never
  inline branching in business logic.
- After finishing a milestone, update `docs/BUILD-PLAN.md` to check it off and note anything
  that turned out differently from the spec.
