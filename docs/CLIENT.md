# MuxDeck Client — Flutter Mobile Spec

Location: `apps/client/`. Targets **Android, iOS, iPadOS**. No web, no desktop.
Read `docs/PROTOCOL.md` before touching anything on the wire.

## 1. Create command

```powershell
cd apps
fvm flutter create --platforms=android,ios --org in.redoimagined --project-name=muxdeck_client client
cd client
fvm use stable
```

Note there is no `web` in `--platforms`. Do not add it later.

## 2. Packages

| Purpose | Package |
| --- | --- |
| state management | `flutter_riverpod` + `riverpod_annotation` |
| WebSocket | `web_socket_channel` |
| mDNS discovery | `bonsoir` |
| QR scanning | `mobile_scanner` |
| secure key storage | `flutter_secure_storage` |
| Ed25519 signing | `cryptography` |
| local settings | `shared_preferences` |
| haptics | `flutter/services` (`HapticFeedback`) — no package needed |
| shared protocol types | `muxdeck_protocol` (path dep on `../../packages/muxdeck_protocol`) |

Riverpod over Provider because the connection lifecycle is genuinely a graph of dependent async
state (discovery → selected host → socket → session → profile) and Riverpod's `AsyncValue` and
auto-dispose model handle that without hand-rolled listeners.

## 3. Certificate pinning — the load-bearing detail

The engine uses a self-signed certificate, so normal TLS validation will fail. Do **not**
blanket-accept bad certificates. Accept exactly one fingerprint:

```dart
final httpClient = HttpClient()
  ..badCertificateCallback = (X509Certificate cert, String host, int port) {
    final actual = sha256.convert(cert.der).toString();
    return actual == expectedFingerprint;   // stored at pairing time
  };

final channel = IOWebSocketChannel.connect(uri, customClient: httpClient);
```

`expectedFingerprint` comes from the pairing QR code and is persisted alongside the host record.
A mismatch is a hard failure with a clear "this host's identity changed" message — never a
silent retry. This is the entire reason the web platform is excluded: browsers cannot do this.

## 4. Platform configuration

### iOS — `ios/Runner/Info.plist`

```xml
<key>NSLocalNetworkUsageDescription</key>
<string>MuxDeck finds your computer on the local network to send button presses.</string>
<key>NSBonjourServices</key>
<array><string>_muxdeck._tcp</string></array>
<key>NSCameraUsageDescription</key>
<string>MuxDeck uses the camera to scan the pairing QR code.</string>
```

**Without `NSBonjourServices` listing the exact service type, mDNS on iOS 14+ returns nothing
and throws no error.** This is the single most common failure in this kind of app. If discovery
"doesn't work" on iOS, check this first.

Also set `UISupportedInterfaceOrientations` to allow landscape on iPad, and enable
`UIRequiresFullScreen = false` so Split View works.

### Android — `android/app/src/main/AndroidManifest.xml`

```xml
<uses-permission android:name="android.permission.INTERNET"/>
<uses-permission android:name="android.permission.ACCESS_NETWORK_STATE"/>
<uses-permission android:name="android.permission.CHANGE_WIFI_MULTICAST_STATE"/>
<uses-permission android:name="android.permission.CAMERA"/>
```

`bonsoir` handles the `MulticastLock` internally, but verify it is released when discovery stops
or you will drain the battery.

## 5. Directory structure

```
lib/
├── main.dart
├── app.dart                       root widget, theme, router
├── core/
│   ├── result.dart                Result<T, AppError>
│   ├── errors.dart
│   └── logging.dart
├── data/
│   ├── identity/
│   │   └── device_identity.dart   Ed25519 keypair, generate once, secure storage
│   ├── hosts/
│   │   ├── host_record.dart       id, name, addr, fingerprint, deviceId
│   │   └── host_store.dart        persisted list of paired hosts
│   └── transport/
│       ├── transport.dart         abstract Transport { connect, send, stream, close }
│       ├── lan_transport.dart     the only implementation today
│       └── connection.dart        reconnect/backoff wrapper around Transport
├── domain/
│   ├── session/
│   │   ├── session_controller.dart   handshake state machine
│   │   └── session_state.dart        disconnected|connecting|authing|ready|error
│   ├── discovery/
│   │   └── discovery_controller.dart bonsoir browse → List<DiscoveredHost>
│   ├── pairing/
│   │   └── pairing_controller.dart   QR parse → pair.request → persist host
│   └── profile/
│       └── profile_controller.dart   profile.get + subscribe, cache last known
└── ui/
    ├── deck/                      the grid — the main screen
    │   ├── deck_page.dart
    │   ├── deck_grid.dart
    │   └── deck_button.dart
    ├── connect/                   host list, discovery, manual entry
    ├── pairing/                   QR scanner, manual code entry
    ├── settings/
    └── common/
```

**`Transport` is an interface even though there is one implementation.** It costs almost nothing
now and it is the seam where USB or BLE would be added if that decision is ever revisited.

## 6. Screens

### Connect
Lists discovered hosts and previously paired hosts, merged and de-duplicated by host ID. Each
row shows name, IP, and paired/unpaired state. Actions: tap to connect, "Pair new device"
(QR), and "Enter address manually".

Discovery must distinguish three states in the UI, because they need different fixes:
- *No hosts found* → "Is the daemon running? Are you on the same Wi-Fi?"
- *Host found, connection refused* → "Found ENIGMA-ENTROPY but it isn't accepting connections."
- *Host found, fingerprint mismatch* → "This host's identity has changed. Re-pair to continue."

A spinner that never resolves is a bug.

### Pairing
Camera scanner via `mobile_scanner`, parsing `muxdeck://pair?addr=&host=&fp=&code=`. Manual
fallback: two text fields (address, 6-digit code). On success, persist a `HostRecord` and go
straight to the deck.

### Deck
The main screen. Landscape-first, but must work in portrait on phones.

- Grid dimensions come from the profile, not from the screen. Buttons scale to fit; the grid
  never scrolls — a deck you have to scroll is not a deck.
- Page indicator + horizontal swipe when a profile has multiple pages.
- Fire the action on **pointer down**, not on tap-up. This is a real perceptual difference and
  it is what makes it feel like hardware.
- Trigger the button's configured haptic on pointer down, before the network send.
- Show optimistic pressed-state immediately; if the engine returns an error, flash the button
  red and toast the message.
- Persistent, unobtrusive status chip: connection state + RTT in ms.

### Settings
Device name, current host, RTT display toggle, unpair, keep-screen-awake toggle
(`WakelockPlus` or platform channel — a deck that sleeps is useless).

## 7. Connection behaviour

- Reconnect with exponential backoff: 500 ms → 1 s → 2 s → 4 s → 8 s cap, with jitter.
- On app resume from background, reconnect immediately (skip the backoff) — iOS will have
  killed the socket.
- On reconnect, re-run the full handshake. Never cache a session.
- Cache the last known profile in `shared_preferences` and render it immediately at launch,
  greyed out, while connecting. The deck appearing instantly matters more than it being live.
- Send `ping` every 5 s while connected; three consecutive missed pongs force a reconnect.

## 8. Testing

- Unit: envelope round-trip against `protocol/fixtures/`, QR payload parsing (including
  malformed input), backoff schedule, fingerprint comparison.
- Widget: deck grid layout at 3×5 / 4×6 / 5×8 on phone and iPad sizes; button press dispatches
  the right op.
- Integration: a fake in-process engine implementing the handshake, so the whole
  discovery → pair → connect → press flow is testable without hardware.
