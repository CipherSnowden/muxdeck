# MuxDeck — Setup & Build Plan

Part 1 is the one-time setup you run by hand. Part 2 is the milestone sequence you drive with
Claude Code, one milestone per session.

---

# Part 1 — One-time setup (Windows, PowerShell 7)

## 1.1 Toolchain

```powershell
winget install --id Rustlang.Rustup -e
winget install --id Git.Git -e
winget install --id GitHub.cli -e
winget install --id Microsoft.VisualStudio.2022.BuildTools -e   # MSVC linker, needed by Rust
winget install --id Anthropic.ClaudeCode -e                      # or: npm i -g @anthropic-ai/claude-code

rustup default stable
rustup component add rustfmt clippy
dart pub global activate fvm      # if fvm isn't already installed
```

Verify:

```powershell
rustc --version ; cargo --version ; fvm --version ; git --version ; gh --version
```

MSVC Build Tools: select the **Desktop development with C++** workload. Rust needs the linker
even though we write no C.

## 1.2 Deal with the old repository

Your existing `github.com/CipherSnowden/muxdeck` holds the pre-migration state. Recommended:
rename and archive it, then create a clean `muxdeck`.

```powershell
gh repo rename muxdeck-legacy --repo CipherSnowden/muxdeck
gh repo archive CipherSnowden/muxdeck-legacy --yes
```

Before archiving, clone it locally as reference — the old `input_win.go` and `input_linux.go`
contain working keycode tables that are worth mining even though the engine is being rewritten
in Rust:

```powershell
git clone https://github.com/CipherSnowden/muxdeck-legacy.git F:\reference\muxdeck-legacy
```

Keep that clone **outside** `F:\projects\muxdeck` so Claude Code never treats it as part of the
new repo.

## 1.3 Create the new repository

```powershell
cd F:\projects
mkdir muxdeck
cd muxdeck

git init -b main
gh repo create CipherSnowden/muxdeck --public --source=. --remote=origin `
  --description "Open-source Stream Deck alternative — Flutter client, Rust engine, LAN only"
```

## 1.4 Drop in the documentation

Place the files you were given:

```
F:\projects\muxdeck\
├── CLAUDE.md
└── docs\
    ├── ARCHITECTURE.md
    ├── PROTOCOL.md
    ├── ENGINE.md
    ├── CLIENT.md
    ├── SERVER.md
    └── BUILD-PLAN.md      (this file)
```

```powershell
git add .
git commit -m "docs: initial architecture, protocol and build plan"
git push -u origin main
```

## 1.5 Start Claude Code

```powershell
cd F:\projects\muxdeck
claude
```

It reads `CLAUDE.md` automatically. Your first message should be:

> Read CLAUDE.md and every file in docs/. Summarise the architecture back to me in your own
> words, then list anything in the specs that is ambiguous, contradictory, or that you would
> need to guess at. Do not write any code yet.

**Do not skip this step.** The gaps it finds are cheaper to fix in a doc than in three
codebases.

---

# Part 2 — Milestones

One milestone per Claude Code session. Commit and push at the end of each. Tick them off in this
file as you go — the checkboxes are how future sessions know where things stand.

---

## [ ] M0 — Scaffold

Repository skeleton, tooling config, CI that runs and does nothing useful yet.

> Implement milestone M0 from docs/BUILD-PLAN.md.
>
> Create the monorepo skeleton exactly as laid out in CLAUDE.md: the directory tree, a root
> README.md, an MIT LICENSE (author: CipherSnowden, 2026), .gitignore covering Rust, Flutter,
> Dart, and OS junk, and .editorconfig.
>
> Scaffold the Rust workspace under engine/ with the four crates from docs/ENGINE.md §2, each
> compiling as an empty stub with its dependency direction wired correctly. Pin the toolchain in
> rust-toolchain.toml.
>
> Print the exact `fvm flutter create` and `fvm dart create` commands I should run for
> apps/client, apps/server and packages/muxdeck_protocol — do not run them yourself, I will run
> them so FVM resolves correctly on my machine.
>
> Add .github/workflows/ with three workflows (engine, client, server) using path filters so
> each only runs on changes to its own area. For now they should just check out, install the
> toolchain, and run fmt/analyze — no builds yet.
>
> Verify `cargo check --workspace` passes, then commit.

After this, run the printed Flutter/Dart create commands, then `fvm use stable` in each Flutter
project, and commit the result.

---

## [ ] M1 — Protocol types and fixtures

The foundation everything else is checked against.

> Implement milestone M1 from docs/BUILD-PLAN.md. docs/PROTOCOL.md is the source of truth —
> follow it exactly and do not invent fields or ops.
>
> 1. Write protocol/fixtures/ with one JSON file per message shape named `<op>.<t>.json`,
>    covering every op and event in docs/PROTOCOL.md §4, plus the data objects in §6. Use
>    obviously fake values for anything key-shaped.
> 2. In engine/crates/muxdeck-core, define the full envelope and all payload types with serde.
>    Use an enum over ops with `#[serde(rename_all = "snake_case")]` and explicit renames to
>    match the dotted op names. Unknown ops must deserialise to a rejectable variant, not panic.
> 3. In packages/muxdeck_protocol, define the equivalent Dart types with hand-written
>    fromJson/toJson (no build_runner — the protocol is small and codegen adds a build step for
>    little gain here).
> 4. Both test suites must load every fixture, deserialise, re-serialise, and assert semantic
>    equality. Add negative tests: wrong `v`, unknown op, missing required field.
>
> Run `cargo test --workspace` and `fvm dart test` in the package, and make both green.

---

## [ ] M2 — Engine core: TLS, handshake, pairing

No input injection yet. This is the security backbone.

> Implement milestone M2. Read docs/ENGINE.md and docs/ARCHITECTURE.md §5 first.
>
> In muxdeck-engine, build: identity (Ed25519 host key + rcgen self-signed cert + `admin.token`,
> generated on first run into the config dir from docs/ENGINE.md §6 with 0600 permissions), the
> axum + rustls WebSocket server on /ws, both session handshake paths from docs/PROTOCOL.md §3
> with the 10-second auth timeout, the pairing module with OTP, proof-of-possession check and the
> 30..=300 second window, the device registry persisted as JSON, and role assignment where
> `admin` requires loopback **and** a matching admin token.
>
> In muxdeckd, wire up clap for the CLI in docs/ENGINE.md §7 (service subcommands can be stubs
> that return "not implemented" for now), tracing setup, and config dir resolution. The
> `muxdeckd pair begin/list/revoke` subcommands are **in scope for this milestone, not stubs** —
> M4 needs them to pair a phone before the desktop panel exists. They connect over loopback with
> the admin token like any other admin client.
>
> While you are there, print the resolved config directory on this machine and correct the
> docs/ENGINE.md §6 table if it does not match what the `directories` crate actually emits.
>
> Tests as specified in docs/ENGINE.md §8 for session and pairing, plus an integration test that
> binds port 0 and runs pair → auth → system.ping over a real WebSocket connection.
>
> Then write a small `examples/probe.rs` I can run to pair a fake device and send a ping, so I
> can exercise this from the terminal before any UI exists.

---

## [ ] M3 — Windows input injection

> Implement milestone M3. Read docs/ENGINE.md §4 and §4.1 carefully — the notes on scancodes,
> KEYEVENTF_UNICODE, extended keys, and batching a combo into a single SendInput call are all
> load-bearing.
>
> Define the InputBackend trait in muxdeck-input with the MockBackend behind cfg(test).
> Implement the Windows backend using the `windows` crate: key_combo, key_sequence, text, media,
> mouse, and preflight.
>
> Build the full canonical key name → Windows scancode/VK table from docs/PROTOCOL.md §5.
> Reference F:\reference\muxdeck-legacy\muxdeck-engine\input_win.go for the previous mapping, but re-derive it
> against the docs rather than copying blindly.
>
> Wire input.* ops through dispatch with capability checks, running injection on
> spawn_blocking. Test the ordering guarantees against MockBackend: modifiers press in order,
> release in reverse, and release still happens if injection fails partway.
>
> Extend examples/probe.rs so I can send a key combo from the terminal and watch it type into
> Notepad.

**Stop here and confirm it actually types before moving on.**

---

## [ ] M4 — Client: discover, pair, connect, press

The first end-to-end moment. This is the milestone that proves the whole design.

> Implement milestone M4. Read docs/CLIENT.md fully first.
>
> Build apps/client: device identity (Ed25519 keypair in flutter_secure_storage), the Transport
> abstraction with its LAN implementation, certificate fingerprint pinning exactly as in
> docs/CLIENT.md §3, the session handshake controller, bonsoir-based discovery, the QR pairing
> flow with manual fallback, and a deck screen rendering a hardcoded 3x5 grid of buttons wired
> to input.key_combo.
>
> Apply all the iOS Info.plist and Android manifest entries from docs/CLIENT.md §4 — especially
> NSBonjourServices, without which iOS discovery silently returns nothing.
>
> Fire actions on pointer down, not tap up. Trigger haptics before the network send. Show a
> connection status chip with live RTT from ping/pong.
>
> Implement the three distinct discovery failure states from docs/CLIENT.md §6 — no silent
> spinners.
>
> Add the fake in-process engine and the widget tests from docs/CLIENT.md §8.

To pair before the desktop panel exists, run `muxdeckd pair begin` — it prints the 6-digit code
and the QR payload for the phone to scan or for manual entry.

---

## [ ] M5 — Desktop control panel

> Implement milestone M5. Read docs/SERVER.md fully first.
>
> Build apps/server: loopback connection with fingerprint pinning, the daemon lifecycle logic
> from docs/SERVER.md §5, tray integration with tray_manager and window_manager, the Dashboard
> with engine status and the preflight result surfaced prominently, the Devices list with revoke,
> and the Pair-a-device screen with a large QR code and countdown.
>
> Implement the `muxdeckd service install/uninstall/status` subcommands in the engine for all
> three platforms — Windows must use a Scheduled Task at logon, NOT a Windows Service, because
> session 0 cannot inject input into the desktop.
>
> Make sure Quit closes the panel without stopping the engine, with a separate explicit
> "Stop engine" menu item.

---

## [ ] M6 — Profiles and the live layout editor

> Implement milestone M6. Read docs/SERVER.md §6 (Layout editor) and docs/PROTOCOL.md §4.5 and §6.
>
> In the engine: the profile store with atomic writes, profile.* ops, and evt profile.changed
> broadcast to subscribers. Ship a sensible default profile on first run so a fresh install isn't
> an empty grid.
>
> In the client: replace the hardcoded grid with profile.get + profile.subscribe, cache the last
> known profile locally and render it greyed out at launch, support multiple pages with swipe.
>
> In the server: the full visual layout editor — grid, per-button editor, op-aware action editor
> including the key-capture field that records real keypresses and maps them to canonical names,
> drag to move, multiple profiles and pages.
>
> The live loop matters: editing on the desktop must update the tablet immediately. Verify that
> end to end before considering this done.

---

## [ ] M7 — macOS and Linux input backends

Needs CI or real machines. Do the platform you have access to first.

> Implement milestone M7. Read docs/ENGINE.md §4.2 and §4.3.
>
> Implement the macOS backend (CGEventPost, CGEventSetFlags for modifiers,
> CGEventKeyboardSetUnicodeString for text, and the NSSystemDefined path for media keys) with
> preflight checking AXIsProcessTrustedWithOptions.
>
> Implement the Linux backend via /dev/uinput: register the full keybit range at device creation,
> sleep ~100ms after creation before the first event, emit SYN_REPORT after each batch, and have
> preflight return a specific message naming the `input` group fix when /dev/uinput is not
> writable. Ship the udev rule and wire it into `service install`.
>
> Extend the CI matrix to build and test all three platforms.

---

## [ ] M8 — Polish

> Implement milestone M8: telemetry sampling and the evt telemetry.update broadcast; the
> reconnect and backoff behaviour from docs/CLIENT.md §7 including immediate reconnect on app
> resume; keep-screen-awake on the client; the named-action system with shell execution gated off
> by default per docs/ARCHITECTURE.md §5.5; the log tail view in the panel; and the settings
> screens on both apps.
>
> Then measure the latency budget in docs/ARCHITECTURE.md §7 and report actual numbers for
> press-to-keystroke on my LAN.

---

## [ ] M9 — Packaging and release

> Implement milestone M9. Set up release CI: a tag push builds muxdeckd for Windows, macOS and
> Linux; builds the desktop panel for all three, each bundled with the matching muxdeckd binary
> alongside it (not as a Flutter asset); builds an Android APK; and builds an unsigned iOS IPA
> via `fvm flutter build ios --release --no-codesign` zipped as Payload/Runner.app for
> sideloading.
>
> Attach everything to a GitHub Release. Write the README with screenshots, install
> instructions per platform, the Linux libayatana-appindicator3 and input-group prerequisites,
> and the macOS Accessibility permission step.

---

# Working notes

- **One milestone per session.** Start each with "Read CLAUDE.md and docs/<relevant>.md, then
  implement milestone MX." Context stays clean and the specs stay authoritative.
- **When Claude Code wants to change the protocol, make it edit `docs/PROTOCOL.md` first.** The
  moment implementation drifts ahead of the spec, the spec stops being useful and the monorepo
  loses its main advantage.
- **Use plan mode** (Shift+Tab twice) for M2, M4, M6 and M7. They are large enough that reviewing
  the approach before any code is written will save you a rewrite.
- **Tick the checkboxes in this file** at the end of each milestone and note anything that turned
  out differently from the spec. This file is how a future session orients itself.

