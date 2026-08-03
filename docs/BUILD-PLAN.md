# MuxDeck — Setup & Build Plan

Part 1 is the one-time setup you run by hand. Part 2 is the milestone sequence you drive with
Claude Code, one milestone per session.

---

# Part 1 — One-time setup (Windows, PowerShell 7)

**Status: complete.** Everything in §1.1–§1.5 has been done and verified on the dev machine. This
part is kept as a record of what the environment actually is, not as a list of things to run. The
only outstanding item is the one flagged in §1.2.

## [x] 1.1 Toolchain — verified present

| Tool | Version | Location / note |
| --- | --- | --- |
| `rustc` / `cargo` | 1.97.1 | target `x86_64-pc-windows-msvc` |
| `rustfmt`, `clippy` | installed | `rustup component list --installed` confirms both |
| `fvm` | 4.1.2 | `C:\ProgramData\chocolatey\bin\fvm.exe` — installed via Chocolatey, **not** `dart pub global activate` |
| Flutter / Dart | 3.44.8 / 3.12.2 | cached by FVM as `stable`; bare `flutter` is deliberately absent from `PATH` |
| `git` | 2.55.0 | |
| `gh` | 2.97.0 | authed as `CipherSnowden`, git protocol `ssh` |
| Visual Studio | Community **2026**, 18.8.12023.21 | `Microsoft.VisualStudio.Component.VC.Tools.x86.x64` present |
| `cmake` | present | `F:\utilities\cmake` |

Note the deviation from the original plan: the machine has **Visual Studio 2026 Community**, not
the 2022 Build Tools. This was the one genuine unknown at M0 — Flutter's Windows desktop toolchain
check and CMake generator were written against VS 2022 (v17), and VS 2026 is v18.

**Resolved: both toolchains accept VS 2026.** Rust links against it without complaint, and
`fvm flutter run -d windows` builds and launches `apps/server`. No fallback was needed.

If a future Flutter upgrade ever regresses this, the fix is to install the 2022 Build Tools
alongside 2026 — they coexist, and only `apps/server` on Windows is affected:

```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools -e   # Desktop development with C++
```

Nothing else is required. No Go, no MinGW, no Python, no `ninja`, no `usbmuxd`, no iTunes — the
legacy quickstart's prerequisite list is obsolete and should not be followed.

## [x] 1.2 The old repository — renamed and archived

Done. `github.com/CipherSnowden/muxdeck-legacy` is private and archived, and is cloned locally at
`F:\reference\muxdeck-legacy` — outside `F:\projects\muxdeck`, so Claude Code never treats it as
part of the new repo. Its `input_win.go` (9 KB) and `input_linux.go` (13 KB) hold working keycode
tables worth mining in M3 and M7.

> **[x] Fixed.** That clone was made *before* the rename, so its `origin` was
> `git@github.com:CipherSnowden/muxdeck.git` — which now points at the **new** repo. A stray
> `git push` from `F:\reference\muxdeck-legacy` would have pushed legacy Go history into this
> project. Repointed and verified to resolve against the archived repository:
>
> ```powershell
> git -C F:\reference\muxdeck-legacy remote set-url origin git@github.com:CipherSnowden/muxdeck-legacy.git
> ```

## [x] 1.3 The new repository — created

`github.com/CipherSnowden/muxdeck` exists, public, described "Open-source Stream Deck alternative
— Flutter client, Rust engine, LAN only", with `origin` wired over SSH and `main` as the default
branch.

## [x] 1.4 Documentation — in place and pushed

`CLAUDE.md` plus `docs/{ARCHITECTURE,PROTOCOL,ENGINE,CLIENT,SERVER,BUILD-PLAN}.md`, pushed across
four commits. `docs/TRANSPORT.md` was added later — see §1.6.

## [x] 1.5 Spec review — done

The review step ("summarise the architecture back to me, then list anything ambiguous or
contradictory") has been run. It produced the decisions in §1.6 and the protocol gap-fills now
merged into `docs/PROTOCOL.md`. Do not re-run it; read §1.6 instead.

## 1.6 Decisions taken after the spec review

Four decisions are settled. Each is recorded where the detail belongs; this section exists so a
future session finds them without archaeology.

- **No USB, no BLE — Wi-Fi/LAN only.** The project's earlier USB-first design could not have
  worked: `usbmuxd`/`iproxy` forwards host→device only and has no reverse tunnel, so iOS USB would
  require inverting the client/server roles, the pinning direction and the challenge direction for
  one platform. Android USB via `adb reverse` does work and is the only option worth revisiting.
  Full reasoning: **`docs/TRANSPORT.md`**. Rule: `CLAUDE.md` constraint #1.

- **iOS/iPadOS is CI-build only.** No Mac on the dev machine. `apps/client` keeps `ios` in
  `--platforms` and CI gets a `macos-latest` job from M0 onward, but iOS is never built locally.
  Releases ship an unsigned IPA (`fvm flutter build ios --release --no-codesign`, zipped as
  `Payload/Runner.app`) for sideloading via Sideloadly or AltStore. iPad layout is verified by
  widget tests at iPad dimensions plus an Android tablet as the physical proxy.

- **Milestone order is M0 → M9 as written.** No vertical spike, no reordering. M2 is the security
  backbone and is worth reviewing before code exists, which is why it is not deferred behind M3.

- **No web platform — reaffirmed at M0.** A reference `flutter create` command carrying
  `--platforms=android,ios,web` came up during M0 and was declined. A browser has no equivalent of
  `HttpClient.badCertificateCallback`, so it cannot pin the engine's self-signed certificate
  (`docs/CLIENT.md` §3); a web client could only connect over plaintext `ws://` or against a
  CA-issued certificate the engine has no way to obtain. Either one removes host authentication
  from the threat model in `docs/ARCHITECTURE.md` §5. Rule: `CLAUDE.md` constraint #2. Also
  declined at the same time: renaming `apps/client`/`apps/server` to `apps/muxdeck-client`/
  `apps/muxdeck-server` — under `apps/` the prefix is redundant, and the Dart package names are
  already `muxdeck_client` and `muxdeck_server`.

- **`docs/PROTOCOL.md` gaps were filled before M1.** `action.list`/`set`/`delete`, `settings.get`
  request shape, `settings.set` example, the literal `qr_payload` construction, `system.ping`
  units, and the `profile.get` response wrapper all now have documented payloads. M1 could not
  write one fixture per message shape without them. No existing shape was changed.

---

# Part 2 — Milestones

One milestone per Claude Code session. Commit and push at the end of each. Tick them off in this
file as you go — the checkboxes are how future sessions know where things stand.

---

## [x] M0 — Scaffold

Repository skeleton, tooling config, CI that runs and does nothing useful yet.

**Done.** Outcome notes:

- Rust workspace builds clean on 1.97.1 — `cargo check`, `cargo fmt --check`,
  `cargo clippy -D warnings` and `cargo test --workspace` all pass. `rust-toolchain.toml` pins
  1.97.1, so `rustup` pulled that exact toolchain rather than reusing `stable`.
- `muxdeck-engine` re-exports `muxdeck_core` and `muxdeck_input`, and `muxdeckd` has a
  `use muxdeck_engine as _;`, so the dependency direction is compiled rather than merely declared.
- `muxdeck-core` and `muxdeck-engine` carry `#![forbid(unsafe_code)]`. `muxdeck-input` deliberately
  does not — `SendInput`, `CGEventPost` and uinput ioctls are all raw FFI.
- All three Dart/Flutter projects created and pinned with `fvm use stable` (`.fvmrc` tracked,
  `.fvm/` ignored). No `web` directory anywhere. Both apps analyze clean and their default tests
  pass; `packages/muxdeck_protocol` analyzes, formats and tests clean.
- CI workflows use `subosito/flutter-action` with `channel: stable` rather than a pinned version,
  deliberately matching `fvm use stable`. Pinning one side and not the other would let CI drift
  away from what the dev machine builds.
- `engine.yml` installs no toolchain action; `rust-toolchain.toml` is left to do the pinning so a
  drift between CI and local shows up instead of being masked.

Deviation from the prompt below: `.editorconfig`, `README.md` and `LICENSE` landed as specified,
but `protocol/fixtures/` and `tools/` are empty directories and therefore not tracked by git —
they appear at M1 and M9 respectively.

> Implement milestone M0 from docs/BUILD-PLAN.md.
>
> Create the monorepo skeleton exactly as laid out in CLAUDE.md: the directory tree, a root
> README.md, an MIT LICENSE (author: CipherSnowden, 2026), and .editorconfig.
>
> Expand .gitignore. The current one is eight lines and will let Flutter ephemerals into the
> repo. It must also cover `**/ios/Pods/`, `**/ios/Flutter/Flutter.framework`,
> `**/windows/flutter/ephemeral/`, `**/macos/Flutter/ephemeral/`,
> `**/linux/flutter/ephemeral/`, `.flutter-plugins`, `.flutter-plugins-dependencies`,
> `pubspec_overrides.yaml`, `**/android/local.properties`, and `.vscode/`.
> **Cargo.lock must be committed, not ignored** — muxdeckd is a binary, not a library.
>
> Scaffold the Rust workspace under engine/ with the four crates from docs/ENGINE.md §2, each
> compiling as an empty stub with its dependency direction wired correctly. Pin the toolchain in
> rust-toolchain.toml to 1.97.1.
>
> Run the `fvm flutter create` / `fvm dart create` commands from docs/CLIENT.md §1 and
> docs/SERVER.md §2 yourself — FVM 4.1.2 is installed and resolves `stable` to Flutter 3.44.8, so
> there is no reason to hand them back. Follow each with `fvm use stable` in the project
> directory. Do not add web to any --platforms list.
>
> Add .github/workflows/ with four workflows using path filters so each only runs on changes to
> its own area:
>   - engine.yml   — windows/macos/ubuntu matrix; cargo fmt --check, clippy -D warnings, cargo test
>   - protocol.yml — packages/muxdeck_protocol; fvm dart analyze + fvm dart test
>   - client.yml   — apps/client; analyze + test on ubuntu, PLUS a macos-latest job that runs
>                    `fvm flutter build ios --release --no-codesign`. That iOS job is the only
>                    thing standing between iPad support and silent bitrot — it goes in now, not
>                    at M9.
>   - server.yml   — apps/server; analyze + test
> For now everything beyond the iOS job is fmt/analyze only — no release builds.
>
> Verify `cargo check --workspace` passes and `fvm flutter analyze` is clean in both apps, then
> commit.

---

## [ ] M1 — Protocol types and fixtures

The foundation everything else is checked against.

> Implement milestone M1 from docs/BUILD-PLAN.md. docs/PROTOCOL.md is the source of truth —
> follow it exactly and do not invent fields or ops.
>
> 1. Write protocol/fixtures/ with one JSON file per message shape, named per
>    docs/PROTOCOL.md §8 (`<op>.<t>[.<variant>].json` — where an op has multiple shapes every
>    file is suffixed, none left bare), covering every op and event in docs/PROTOCOL.md §4, plus
>    the data objects in §6. Use obviously fake values for anything key-shaped.
> 2. In engine/crates/muxdeck-core, define the full envelope and all payload types with serde.
>    Use an enum over ops with `#[serde(rename_all = "snake_case")]` and explicit renames to
>    match the dotted op names. Unknown ops must deserialise to a rejectable variant, not panic.
>    The `session.hello` response is an internally-tagged union — `#[serde(tag = "mode")]`, never
>    `untagged`. The fixture loader picks its type from `op` and `t` only; the filename variant
>    is not an input to that decision.
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

**Verify on a physical Android device on the same Wi-Fi.** An emulator will not exercise mDNS
realistically, and mDNS is the part most likely to break. iOS cannot be verified locally (§1.6) —
get `NSBonjourServices` right the first time, because finding that mistake later costs a CI round
trip per attempt.

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
> Implement the Linux backend via /dev/uinput, mining the keycode table in
> F:\reference\muxdeck-legacy\muxdeck-engine\input_linux.go but re-deriving it against
> docs/PROTOCOL.md §5: register the full keybit range at device creation,
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

