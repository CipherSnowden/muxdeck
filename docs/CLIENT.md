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
| state management | `flutter_riverpod` |
| WebSocket | `web_socket_channel` |
| mDNS discovery | `bonsoir` |
| QR scanning | `mobile_scanner` |
| secure key storage | `flutter_secure_storage` |
| Ed25519 signing | `cryptography` |
| SHA-256 (certificate fingerprints) | `crypto` |
| local settings | `shared_preferences` |
| keep screen awake | `wakelock_plus` |
| haptics | `flutter/services` (`HapticFeedback`) — no package needed |
| shared protocol types | `muxdeck_protocol` (path dep on `../../packages/muxdeck_protocol`) |

Riverpod over Provider because the connection lifecycle is genuinely a graph of dependent async
state (discovery → selected host → socket → session → profile) and Riverpod's `AsyncValue` and
auto-dispose model handle that without hand-rolled listeners.

**Providers are hand-written; there is no `riverpod_annotation` and no `build_runner`.** The
graph is about eight providers, each a few lines, and codegen would add a build step and a watch
process to save very little — the same reasoning that kept `build_runner` out of
`packages/muxdeck_protocol` in M1. There is no code generation anywhere in this repository.

**Modern providers only. No legacy providers, ever.** Riverpod 3 splits its API in two: the
current one, and a legacy set kept for migrating old codebases. Use:

| Need | Use |
| --- | --- |
| mutable state with methods | `Notifier<T>` + `NotifierProvider<N, T>(N.new)` |
| async state with methods | `AsyncNotifier<T>` + `AsyncNotifierProvider<N, T>(N.new)` |
| a stream with methods | `StreamNotifier<T>` + `StreamNotifierProvider<N, T>(N.new)` |
| read-only derived value | `Provider`, `FutureProvider`, `StreamProvider` |

Never `StateProvider`, `StateNotifierProvider` or `ChangeNotifierProvider`. They are the legacy
set, they carry `state_notifier` along with them, and they encourage putting mutation logic in
widgets rather than in a notifier.

This is self-enforcing rather than a rule to remember: Riverpod 3 does not export the legacy
providers from `package:flutter_riverpod/flutter_riverpod.dart` at all — they live behind a
separate `legacy.dart` import. **Do not import `legacy.dart`.** If a legacy provider appears in a
diff, it arrived with an import that should not be there.

The canonical shape, from the package's own documentation:

```dart
final counterProvider = NotifierProvider<Counter, int>(Counter.new);

class Counter extends Notifier<int> {
  @override
  int build() => 0;          // initial state

  void increment() => state++;
}
```

## 3. Certificate pinning — the load-bearing detail

The engine uses a self-signed certificate, so normal TLS validation will fail. Do **not**
blanket-accept bad certificates. Accept exactly one fingerprint:

```dart
// withTrustedRoots: false is load-bearing — see below.
final httpClient = HttpClient(context: SecurityContext(withTrustedRoots: false))
  ..badCertificateCallback = (X509Certificate cert, String host, int port) {
    final actual = sha256.convert(cert.der).toString();
    return actual == expectedFingerprint;   // stored at pairing time
  };

final channel = IOWebSocketChannel.connect(uri, customClient: httpClient);
```

`expectedFingerprint` comes from the pairing QR code and is persisted alongside the host record.
A mismatch is a hard failure with a clear "this host's identity changed" message — never a
silent retry. This is the entire reason the web platform is excluded: browsers cannot do this.

**`SecurityContext(withTrustedRoots: false)` is not optional.** `badCertificateCallback` fires
only for certificates that fail normal validation. A certificate that *does* chain to a trusted
root skips the callback entirely and is accepted — so without this, the fingerprint check is
simply not consulted on the one path where it would matter, and the pin is not a pin. Disabling
the trust store means nothing can ever chain, so the callback always runs and the fingerprint is
the only thing that decides. Losing CA validation costs nothing here: the host has no DNS name
and no CA, which is why it is self-signed in the first place.

Two more details worth knowing before debugging this:

- Hash the **DER**, not the PEM. `X509Certificate.der` is a `Uint8List`; `sha256.convert(...)`
  from `package:crypto` renders lowercase hex with no separators via `toString()`, which is
  exactly the representation `docs/PROTOCOL.md` §1 specifies. Hashing the PEM yields a value
  nothing else in the system agrees with.
- `web_socket_channel` 3.x wraps connection failures in `WebSocketChannelException`. The real
  cause — the `HandshakeException` a rejected pin produces — is in its `.inner`. Surface that as
  a fingerprint mismatch rather than a generic "could not connect", or the user is told to check
  their Wi-Fi when their host's identity has changed.

## 4. Platform configuration

### Version floors

Taken from the dependencies' own gradle files and podspecs, not guessed. Raising these is not
optional — the build fails, or worse, succeeds and misbehaves.

| Platform | Floor | Imposed by |
| --- | --- | --- |
| Android | `minSdk 23` | `flutter_secure_storage` 10.x, `mobile_scanner` 7.x |
| iOS | deployment target `13.0` | `bonsoir_darwin` (the highest of the three; `mobile_scanner` and `flutter_secure_storage_darwin` want 12.0) |

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

Note the form: `_muxdeck._tcp`, **without** a `.local.` suffix, matching the type string passed
to `bonsoir`. The domain is supplied separately by the platform, and the engine advertising
`_muxdeck._tcp.local.` on the wire is correct and unrelated. See §4.1 below — getting this wrong
in the other direction is just as silent.

Also set `UISupportedInterfaceOrientations` to allow landscape on iPad, and enable
`UIRequiresFullScreen = false` so Split View works.

**Keychain entitlement — required, and silent when missing.** Add an empty
`keychain-access-groups` array to **both** `ios/Runner/DebugProfile.entitlements` and
`ios/Runner/Release.entitlements`:

```xml
<key>keychain-access-groups</key>
<array/>
```

Without it, `flutter_secure_storage` writes appear to succeed and persist nothing — no
exception, no log. The device identity keypair silently fails to survive a restart, and the
device unpairs itself with no diagnosis available on the client.

### Android — `android/app/src/main/AndroidManifest.xml`

```xml
<uses-permission android:name="android.permission.INTERNET"/>
<uses-permission android:name="android.permission.ACCESS_NETWORK_STATE"/>
<uses-permission android:name="android.permission.CHANGE_WIFI_MULTICAST_STATE"/>
<uses-permission android:name="android.permission.CAMERA"/>
```

`bonsoir` and `mobile_scanner` declare the multicast and camera permissions in their own
manifests, so these merge automatically; listing them here documents the dependency rather than
adding anything. `bonsoir` also creates the `MulticastLock` itself, reference-counted, acquiring
it on discovery start and releasing it on stop — but verify that release actually happens on your
own teardown path, or you will drain the battery.

**`android:allowBackup="false"` on `<application>` — required.** With Android's automatic backup
enabled, Google Drive restores `flutter_secure_storage`'s encrypted blob onto a device whose
KeyStore does not hold the matching key, and every subsequent read throws
`InvalidKeyException: Failed to unwrap key`. It is the most common production failure for that
plugin, it only appears after a device migration, and it is unrecoverable in the field. The
alternative — excluding just the plugin's shared-preferences file via `android:fullBackupContent`
— is more precise but more to get wrong; a deck has nothing else worth backing up.

### 4.1 The service type string, on both sides

Three places name the service, and they do **not** all use the same form:

| Where | Value |
| --- | --- |
| Engine advertisement (`discovery.rs`) | `_muxdeck._tcp.local.` |
| `BonsoirDiscovery(type: …)` in the client | `_muxdeck._tcp` |
| iOS `NSBonjourServices` | `_muxdeck._tcp` |

The client and the plist take the bare type because the platform appends the `local.` domain
itself. Passing the fully-qualified form to `bonsoir` is **silently rewritten** to a default
service type by its normalizer, and discovery then finds nothing while reporting no error.

## 5. Directory structure

```
lib/
├── main.dart
├── app.dart                       root widget, theme, router
├── core/
│   ├── errors.dart                sealed AppError
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
now and it is the seam where USB or BLE would be added if that decision is ever revisited. It
also earns its keep immediately: a `FakeTransport` is what lets the whole client — handshake,
pairing, dispatch — be tested without a socket, a certificate or a running engine.

**There is no `Result<T, AppError>`.** Riverpod's `AsyncValue` already models loading, data and
error, so a parallel result type would be a second error channel to keep in sync with the first.
Failures are thrown as sealed `AppError` subclasses and surface through `AsyncValue.error`, which
is what the UI already switches on.

### Icons

A button's `icon` field is a string, and Flutter **tree-shakes icons** — a runtime
`String → IconData` lookup silently renders blanks in release builds while working perfectly in
debug. Do not work around this with `--no-tree-shake-icons`; it bloats every build to fix one
lookup.

Instead, a curated map of roughly 200 deck-appropriate Material icons lives in
`packages/muxdeck_icons/lib/src/icon_map.dart` — a `const Map<String, IconData>`. Because the
constant references each `IconData` directly, the tree-shaker keeps exactly those glyphs. The
client renders from this map and the desktop icon picker offers from the same map, so the two
cannot disagree about what a name means or which names exist. Unknown names fall back to a filled
dot.

**It is a separate package from `muxdeck_protocol`, deliberately.** `IconData` comes from
`package:flutter`, and `muxdeck_protocol` is a plain Dart package — which is what lets CI test the
protocol on the Dart SDK alone, in seconds, with no Flutter install. `muxdeck_icons` depends on
Flutter and on `muxdeck_protocol`; both apps depend on `muxdeck_icons`. Do not solve this by
adding Flutter to the protocol package.

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
- **Exception: buttons that have an `on_long_press` action fire on tap-up**, with long-press
  detection at 500 ms. A button cannot both fire instantly and wait to find out whether the press
  was long. Buttons with `on_long_press == null` — which should be most of them — keep the
  fire-on-down behaviour. The layout editor warns about this when a long-press action is assigned
  (`docs/SERVER.md` §6).
- Trigger the button's configured haptic on pointer down, before the network send — on both
  paths, since the haptic is feedback that the touch registered, not that the action fired.
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
- Send `system.ping` every 5 s while connected; three consecutive missed responses to
  `system.ping` force a reconnect. There is no `pong` op — the `res` is the pong.
- **Presses that cannot be sent are dropped, never queued.** Replaying a `CONTROL+W` five seconds
  after the user pressed it is worse than losing it. On send failure: drop the press, flash the
  button red, no retry.
- Grey out buttons whose action needs a capability the host does not have, using the
  `capabilities` block of the `Ready` payload (`docs/PROTOCOL.md` §4.1) — a Linux host reports
  `text_unicode: false`, so `input.text` buttons are visibly unavailable rather than failing at
  press time.

## 8. Testing

- Unit: envelope round-trip against `protocol/fixtures/`, QR payload parsing (including
  malformed input), backoff schedule, fingerprint comparison.
- Widget: deck grid layout at 3×5 / 4×6 / 5×8 on phone and iPad sizes; button press dispatches
  the right op.
- Integration: a fake in-process engine implementing the handshake, so the whole
  discovery → pair → connect → press flow is testable without hardware.
