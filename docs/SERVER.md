# MuxDeck Server — Flutter Desktop Control Panel Spec

Location: `apps/server/`. Targets **Windows, macOS, Linux**. No web, no mobile.

## 1. What this app is and is not

It **is** a control panel: a WebSocket client that connects to `wss://127.0.0.1:47654/ws`
with the `admin` role, plus an installer that registers the daemon to auto-start.

It **is not** the server. It does not host the WebSocket server, does not inject input, does not
own configuration, and is not required to be running for the deck to work. If this app is
closed, MuxDeck keeps working. Any design that makes the GUI load-bearing is wrong — see
`docs/ARCHITECTURE.md` §3.

Practically, this means the app shares almost all of its networking code with the mobile client,
via `packages/muxdeck_protocol`. Only the role and the UI differ.

## 2. Create command

```powershell
cd apps
fvm flutter create --platforms=windows,macos,linux --org in.redoimagined --project-name=muxdeck_server server
cd server
fvm use stable
```

## 3. Packages

| Purpose | Package |
| --- | --- |
| state management | `flutter_riverpod` |
| WebSocket | `web_socket_channel` |
| window control | `window_manager` |
| system tray | `tray_manager` |
| auto-start (fallback) | `launch_at_startup` |
| QR generation | `qr_flutter` |
| local settings | `shared_preferences` |
| shared protocol types | `muxdeck_protocol` (path dep) |

Linux tray requires `libayatana-appindicator3-dev` at build time and
`libayatana-appindicator3-1` at runtime. Document it in the README and check for it in CI.

## 4. Loopback connection

Connecting to `127.0.0.1` still uses TLS with the self-signed certificate, so the same
fingerprint-pinning logic from `docs/CLIENT.md` §3 applies. The panel reads the fingerprint by
shelling out to `muxdeckd --print-fingerprint`, or by reading the certificate directly from the
config directory — it does not need a QR code, because it is already on the machine.

`admin` is granted by the engine only when the connection is **both** from loopback **and**
presenting the local admin token. The panel reads `admin.token` from the same config directory
and sends it in `session.hello` in place of a `device_id`; the response is a `Ready` payload
directly, with no challenge round trip and no `session.auth`. See `docs/ARCHITECTURE.md` §5.4 —
the token exists because loopback alone would also admit a second logged-in user on a
multi-user desktop.

The panel therefore has no device identity, no keypair, and never pairs itself. It cannot obtain
`admin` remotely, and it cannot request the role at all.

## 5. Daemon lifecycle

The panel's only privileged responsibility.

On launch:
1. Try to connect. If it succeeds, done — nothing else to do.
2. If connection is refused, check whether `muxdeckd` is installed (look for the binary next to
   the app bundle, then on `PATH`).
3. If installed but not running, start it. If not installed, show a first-run screen with an
   **Install** button that runs `muxdeckd service install`.
4. Poll for the socket to come up, with a 10 s timeout and a real error message on failure.

The `muxdeckd` binary ships **alongside** the desktop app in the same installer, not embedded as
a Flutter asset. Assets get extracted to temp directories, which breaks code-signing on macOS
and trips antivirus on Windows.

Platform auto-start mechanisms (implemented in the daemon, invoked from here):

- **Windows:** Scheduled Task at logon. Not a Windows Service — services run in session 0 and
  cannot inject input into the desktop.
- **macOS:** `launchd` LaunchAgent in `~/Library/LaunchAgents/`.
- **Linux:** systemd **user** unit in `~/.config/systemd/user/`.

## 6. Screens

### Dashboard (home)
- Engine status: running / stopped / unreachable, with version.
- Listening address and port, plus the host name shown to clients.
- **Input backend preflight result.** If `preflight()` failed, this is the loudest thing on the
  screen, with the specific remediation:
  - macOS → "Grant Accessibility permission" + a button that opens the settings pane.
  - Linux → "Add your user to the `input` group" + the exact command, and a note that a logout
    is required.
  - Windows → generally fine; note the elevated-window limitation if relevant.
- Connected devices with live RTT.
- Recent log tail (last ~200 lines, streamed from the log file).

### Devices
Table of paired devices from `pair.list_devices`: name, platform, paired date, last seen,
connected indicator. Actions: rename, revoke. Revoking prompts for confirmation and kills any
live socket immediately.

### Pair a device
Calls `pair.begin`, then displays the QR code from `qr_payload` large and high-contrast, the
6-digit code in large type underneath for manual entry, and a visible countdown to expiry.
Auto-closes and returns to Devices on success (`evt device.changed`).

### Layout editor
The most valuable screen and the reason this app exists.

- Visual grid matching the profile's dimensions; click a cell to edit.
- Per-button editor: label, icon picker, colour, haptic strength, tap action, long-press action.
- The icon picker offers **only** the names in `packages/muxdeck_protocol`'s
  `lib/src/icon_map.dart`, the same curated map the client renders from — so the picker can never
  offer a name the deck would draw as a blank. See `docs/CLIENT.md` §5.
- **Assigning a long-press action shows a small latency warning.** Buttons without one fire on
  pointer down; buttons with one must wait for tap-up so a long press can be distinguished
  (`docs/CLIENT.md` §6). That button will feel slower, and the user should find that out here
  rather than by wondering why one key feels wrong.
- Action editor is op-aware: choosing `input.key_combo` gives a key-capture field ("press the
  combo you want") that records real keys and maps them to the canonical names in
  `docs/PROTOCOL.md` §5. Choosing `input.media` gives a dropdown. Choosing `action.run` gives a
  list of defined actions — and is disabled with an explanation if shell actions are off.
- Drag to move buttons between cells; drag off the grid to clear.
- Multiple profiles and multiple pages per profile; activate a profile from here.
- Every edit calls `profile.set`, and the engine pushes `evt profile.changed` to connected
  decks — **the layout updates live on the tablet while you edit it.** Build this feedback loop
  early, it is what makes the editor pleasant.

### Actions
Only reachable when shell actions are enabled. Enabling shows an unambiguous warning that any
paired device will be able to run these commands. Each action is a name plus a command and an
argument list as separate fields — never a single string, so nothing is passed to a shell
interpreter.

### Settings
Port (with a restart-required notice), host display name, telemetry on/off and interval,
auto-start on/off, shell actions on/off, log level, open-config-folder button,
reset-identity button behind a double confirmation (it unpairs every device).

## 7. Tray behaviour

- Tray icon reflects state: connected / running-but-no-devices / stopped.
- Menu: Open MuxDeck, Pair a device, Start/Stop engine, Open logs, Quit.
- **Quit means quit the panel, not the engine.** Add a separate, explicit "Stop engine" item.
  Users will otherwise kill their own deck by closing a window.
- Closing the window hides to tray by default; make it a setting.
- On first hide-to-tray, show a one-time notification explaining where the app went.

## 8. Window

- `window_manager`: minimum size 900×640, sensible default 1100×760, remember size and position.
- Follow the system light/dark theme; allow an override.
- Keep the visual language consistent with the mobile client — same accent colour, same button
  rendering in the editor preview as the client uses on the deck, so what you design is what you
  get.

## 9. Testing

- Unit: fingerprint reading, daemon-detection logic, key-capture → canonical key name mapping
  (this one needs real coverage; layout-dependent bugs live here).
- Widget: layout editor renders and edits a profile correctly; the action editor gates
  `action.run` when shell actions are disabled.
- Integration: against a fake in-process engine, exercise pair.begin → QR display →
  device.changed, and profile.set → profile.changed round trip.
