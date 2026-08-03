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

- **The TLS certificate is generated once and never regenerated.** §5.1 previously said it was
  refreshed when the host's addresses changed, which contradicted §8's promise that an IP change
  needs no re-pair: a new certificate means a new fingerprint, and the fingerprint is the only
  thing authenticating the host, so a DHCP lease renewal would have unpaired every device. Since
  no client validates SANs, refreshing them buys nothing. Detail: `docs/ARCHITECTURE.md` §5.1.

- **mDNS advertisement belongs to M2.** It was specified in `docs/ARCHITECTURE.md` §6 and listed
  as an engine module in `docs/ENGINE.md` §5, but no milestone had claimed it, and M4's client
  discovery depends on it existing.

- **The icon map lives in `packages/muxdeck_icons`,** not in `muxdeck_protocol`. See the M1
  section below and `docs/CLIENT.md` §5.

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

## [x] M1 — Protocol types and fixtures

The foundation everything else is checked against.

**Done.** Outcome notes:

- **66 envelope fixtures**, one per message shape, plus two byte-layout fixtures under
  `protocol/fixtures/signing/`. Every op in the `docs/ARCHITECTURE.md` §5.4 matrix has both a
  request and a response; all five events are covered; two `err` fixtures exercise the error
  envelope. `input.mouse.req` has six variants and no bare file beside them, per §8.
- **Both suites fail correctly.** Verified by mutation, not by assumption: adding an unknown
  field to a fixture breaks round-trip equality in both languages, and renaming `host_name`
  fails the parse in both. Each names the offending file.
- The signing buffers are compared as **raw bytes** via hex fixtures, so neither suite needs a
  base64 codec to check the one thing that fails silently at runtime.
- Rust: 15 tests. Dart: 14 tests. `cargo fmt`/`clippy -D warnings` and
  `dart format`/`dart analyze` all clean.

Deviations from the spec, all recorded in `docs/PROTOCOL.md` in the same commit:

- **`signature` and `proof` examples were the wrong length.** Both were 44-character base64
  (32 bytes); an Ed25519 signature is 64 bytes, so the correct encoding is 88 characters. Left
  as they were, M2 would have built a length check against an impossible value. §2 now carries
  a table of every base64 field's fixed byte length, and states that a wrong length is
  `BAD_REQUEST` rather than `BAD_SIGNATURE` — it is a malformed message, not a failed
  verification.
- `hold_ms` and `delay_ms` are **optional** rather than defaulted to zero on both sides. A
  `key_sequence` step that omits `hold_ms` must not gain one on the way back out, and a
  defaulted field would break exact round-tripping.

### [x] Resolved — `icon_map.dart` moves to its own package

`docs/CLIENT.md` §5 places the curated icon map at
`packages/muxdeck_protocol/lib/src/icon_map.dart` as a `const Map<String, IconData>`.
`IconData` comes from `package:flutter`, but `muxdeck_protocol` is a **plain Dart package** —
which is exactly what lets `protocol.yml` run it with the Dart SDK alone, in seconds, without a
Flutter install. Adding a Flutter dependency would make `dart test` unusable there.

**Decision: a second package, `packages/muxdeck_icons`** — a Flutter package depending on
`muxdeck_protocol`, holding the curated `const Map<String, IconData>`. Both apps depend on it.
This keeps `muxdeck_protocol` plain Dart so `protocol.yml` keeps testing on the Dart SDK alone in
seconds. `docs/CLIENT.md` §5 has been updated to match. **No code until M4** — nothing before then
renders an icon.

Rejected: making `muxdeck_protocol` a Flutter package (the protocol tests would need a Flutter
toolchain, and the shared package would stop being usable by anything non-Flutter), and splitting
names from icons across two places (the desktop picker and the deck could then disagree about
which names exist, which is the specific failure §5 exists to prevent).

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

## [x] M2 — Engine core: TLS, handshake, pairing

No input injection yet. This is the security backbone.

**Done.** Outcome notes:

- 69 tests: 45 unit, 9 integration over a real TLS WebSocket on port 0, plus the M1 protocol
  suites. Verified end to end against a running daemon with `examples/probe.rs` — pair, then
  authenticate on a fresh socket, then five pings at 0.11–0.21 ms over loopback.
- **`docs/ENGINE.md` §6 was wrong about the Windows config path** and has been corrected. The
  `directories` crate ignores the qualifier on Windows, so the real path is
  `%APPDATA%\redoimagined\MuxDeck\config\` with no `in.` prefix. macOS and Linux are still
  predictions — check them with the new `--print-config-dir` flag when hardware exists.
- Secret files are owner-only, confirmed with `icacls`: inheritance stripped, a single
  `<user>:(F)` entry, nobody else named.
- `icacls` is shelled out to rather than calling `SetNamedSecurityInfoW`, so the whole engine
  keeps `#![forbid(unsafe_code)]` — the plan had allowed an exception for Win32 FFI and it turned
  out not to be needed. The trade-off is recorded in `secret_file.rs`.
- Logs were grepped for the admin token and the host private key at DEBUG level: neither appears.
  Only `host_id` and the certificate fingerprint are logged, both public by design.
- mDNS advertises `ENIGMA-ENTROPY._muxdeck._tcp.local.` with `v`, `id`, `name` and `fp`.

Two things worth knowing for later:

- **`std::net::TcpListener` must be set non-blocking before tokio adopts it.** Without it every
  connection hangs at the TLS handshake with no error at all — the tests simply never returned.
  Costly to diagnose, trivial to fix, easy to reintroduce.
- **`capabilities` is currently all-false** because there is no input backend yet. M3 replaces the
  placeholder in `Engine::capabilities` with real `InputBackend::preflight` results.

Not in scope and deliberately left: `input.*`, `profile.*`, `action.*`, `settings.*` and
`telemetry.*` are refused with `UNKNOWN_OP` until their milestones. The capability matrix is
already enforced for all of them, so those milestones add handlers, not permissions.

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
> Also build the discovery module: advertise `_muxdeck._tcp.local.` with the TXT records from
> docs/ARCHITECTURE.md §6 (`v`, `id`, `name`, `fp`), and de-advertise on shutdown. This is in M2
> rather than later because `id` and `fp` are exactly what identity generation already computes,
> and M4's client discovery has nothing to find without it.
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

## [x] M3 — Windows input injection

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

**Done, and confirmed typing.** Outcome notes:

- 92 tests. Verified end to end by injecting into a text box and reading the buffer back:
  the captured string matched `unicode: é ü ß 日本語 🎛` byte for byte, including `U+1F39B`,
  which is outside the BMP and therefore proves the surrogate-pair path.
- The same capture proves the two things that fail *silently* when wrong. A first line was
  typed, then `CONTROL+A` selected it and `DELETE` removed it, leaving only the unicode line —
  so the modifier really was held across `A`, and `DELETE` really did carry
  `KEYEVENTF_EXTENDEDKEY`. Without that flag it would have arrived as numpad `.` and appended
  a dot instead of deleting.
- **`docs/ENGINE.md` §4.1 was wrong about scancodes** and has been corrected. It said to use
  `KEYEVENTF_SCANCODE`; that serves games but sends the wrong letter on a non-US layout. The
  backend now sends both `wVk` and `wScan` with neither flag, which serves both audiences.
- `capabilities` in the `Ready` payload is now real: it is the backend's own report ANDed with
  `preflight()`, so a host that cannot inject advertises nothing rather than lying.
- Platforms without a backend get `NullBackend`, so the daemon still starts on macOS and Linux
  and says why input is unavailable instead of refusing to run. M7 replaces it.

Worth knowing:

- **`MockBackend` sits behind a `mock` feature, not `#[cfg(test)]`.** A `cfg(test)` module is
  visible only to its own crate, so `muxdeck-engine`'s tests could not reach it. `cargo build`
  never enables the feature, so it stays out of the shipped daemon.
- Two safety valves that were not in the spec: `input.text` is capped at 4096 bytes and a
  whole `input.key_sequence` at 30 seconds. Each character costs four `SendInput` events, and
  a sequence runs detached from the socket that asked for it, so neither had a natural ceiling.

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

**Code complete; awaiting on-device verification.** Outcome notes:

- 40 client tests, clean analyze. The integration suite runs against a `FakeEngine` that performs
  **real `Ed25519().verify()`** against `sessionAuthMessage` and `pairProofMessage`, so a green
  run proves the Dart client builds byte-identical signing input to the Rust engine — the one
  disagreement that authenticates nothing and leaves no diagnosable symptom.
- Package APIs were read from source in the pub cache rather than assumed. Five findings changed
  the implementation; they are recorded in `docs/CLIENT.md` §3, §4 and §4.1 because each of them
  fails *silently*.
- **A security hole in the spec's own pinning snippet was closed.** `badCertificateCallback`
  fires only for certificates that fail normal validation, so one chaining to a public CA would
  skip the check entirely and be accepted — the pin simply would not be consulted.
  `SecurityContext(withTrustedRoots: false)` makes the callback the only decider. §3.
- **A real bug was caught by the new tests**: `decodeHostRecords` returned `const []` on the
  empty path, and `HostStore.save` mutates that list — so the very first pairing on a fresh
  install failed with "Cannot remove from an unmodifiable list". Fixed at the source so every
  caller benefits.

- **The iOS build broke once, and the cause was an added `ios/Podfile`.** Every plugin here ships
  as a Swift Package, so Flutter uses SPM and CocoaPods is not involved; a Podfile forced
  CocoaPods integration into a project with none and the sandbox went out of sync with
  `Podfile.lock`. Removed. The deployment target it was meant to enforce is already set in the
  Xcode project, which is what SPM reads. `docs/CLIENT.md` §4 now says so, because the reflex to
  add one is natural and wrong.

- **Dart-signs-Rust-verifies is now covered, without a phone.**
  `apps/client/test/live_engine_test.dart` runs the real `LanTransport` against a real running
  `muxdeckd`. It confirmed three things nothing else could: the pin accepts the engine's actual
  `rcgen` certificate, a wrong pin is *rejected* against that same certificate (so the pin is
  load-bearing rather than incidentally passing), and `ed25519-dalek`'s `verify_strict` accepts
  signatures made by `package:cryptography` for both `pairProofMessage` and
  `sessionAuthMessage`. The engine's registry shows the Dart client paired:
  `d_62fdc013fe5fe059 live_engine_test android`.

  M1's fixtures proved the signing *buffers* match byte-for-byte; that says both sides build the
  same bytes, not that a signature over them verifies. This closes the gap.

  It skips unless `MUXDECK_LIVE_ADDR` and `MUXDECK_LIVE_FP` are set, so `flutter test` stays
  hermetic and CI is unaffected — a runner has no engine, and standing one up there would test
  the runner's network stack rather than the protocol. Run instructions are in the file's own
  header. Note `flutter_test` installs a mock `HttpClient`; the test clears
  `HttpOverrides.global` or every socket answers "Unsupported operation: Mocked response".

Still to do, and it needs hardware: run through the seven-step checklist in the plan on a
physical Android device. **Discovery is now the only step that cannot be verified any other
way** — pairing, the handshake, pinning and input dispatch are all covered above.

Two deviations from `docs/CLIENT.md` §5, both recorded there: no `core/result.dart` (Riverpod's
`AsyncValue` already models error state, so a parallel `Result` would be a second channel to keep
in sync), and no `packages/muxdeck_icons` yet — the M4 grid is hardcoded, so there is no
`String → IconData` lookup to do and the package would have no caller until M6.

---

## [x] M5 — Desktop control panel

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

**Done, and verified running against a live engine.** Outcome notes:

- The panel builds and runs on Windows with VS 2026, connects over loopback with the admin
  token, and renders real engine data — screenshots confirmed 2 paired devices matching the
  engine's actual registry, real capabilities, and a live QR pairing window with a working
  countdown. Pairing no longer needs a terminal, which is what this milestone was for.
- 109 engine tests (14 new), 5 panel tests, 40 client tests. `cargo fmt`, `clippy -D warnings`
  and `flutter analyze` all clean.

- **`docs/ENGINE.md` §7 was wrong about Windows auto-start, and it has been corrected.** It said
  a Scheduled Task at logon needs no admin. `schtasks /SC ONLOGON` in fact fails with
  `ERROR: Access is denied.` unelevated — confirmed by hand — because that flag emits a trigger
  with no `UserId`, meaning "any user", which is a machine-wide change. The fix is
  `/Create /XML` naming the current user in both the trigger and the principal. New §7.1 records
  this, along with three Task Scheduler defaults that would each kill the daemon silently
  (a 72-hour execution limit, and two battery restrictions).
- Verified end to end by hand: install unelevated, inspect the registered task
  (`InteractiveToken`, per-user SID, `PT0S`, battery limits off), uninstall twice for
  idempotence, confirm the machine is left clean.

- **The networking layer moved into `packages/muxdeck_protocol`.** The panel and the mobile
  client both need certificate pinning, and two copies is how one gets a security fix and the
  other quietly does not. It is all plain Dart, so the package stays Flutter-free and its CI job
  still runs on the Dart SDK alone. The client's 40 tests passed unchanged across the move.
- A bug the new tests caught: reading the certificate broke on CRLF line endings — a stray `\r`
  per line corrupted the base64 and produced a plausible-looking fingerprint matching nothing.
  It is a Windows text file, so that was not hypothetical.

Known ceilings, both commented in the code: the Windows task status word is localised, so on a
non-English Windows a running task reads as stopped — harmless, since a second daemon just fails
to bind and exits; and `launchctl load -w` is the deprecated spelling of `bootstrap gui/<uid>`,
kept because it needs no uid lookup.

Deferred to M8 as planned: the Settings screen, the log tail, and telemetry. The tray has Open,
Pair and Quit; "Stop engine" arrives with the settings work that gives it somewhere to report
failure.

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

