/// The panel's connection to the engine, and the daemon lifecycle around it.
library;

import 'dart:async';
// `Platform` is aliased because muxdeck_protocol exports its own enum of that name, which
// otherwise wins. `File`, `Process` and `Socket` are used unprefixed.
import 'dart:io' hide Platform;
import 'dart:io' as io show Platform;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:muxdeck_protocol/muxdeck_protocol.dart';

import '../data/engine_locator.dart';

/// The port the engine listens on. Configurable in the engine; the panel learns it from
/// `settings.get` once connected, and assumes the default to make the first connection.
const defaultPort = 47654;

/// How long to wait for the socket after starting the daemon. `docs/SERVER.md` §5.
const startupTimeout = Duration(seconds: 10);

sealed class AdminState {
  const AdminState();
}

/// Looking for the engine.
class AdminConnecting extends AdminState {
  const AdminConnecting();
}

/// The engine has never run here: no config directory, no credentials.
///
/// Distinct from [AdminEngineStopped] because the fix differs — this needs an install, that
/// needs a start.
class AdminNotInstalled extends AdminState {
  const AdminNotInstalled({required this.canInstall});

  /// Whether a `muxdeckd` binary was found to install from.
  final bool canInstall;
}

/// The engine is installed but not answering.
class AdminEngineStopped extends AdminState {
  const AdminEngineStopped(this.detail);

  final String detail;
}

class AdminReady extends AdminState {
  const AdminReady({required this.ready, required this.devices, this.pairing});

  /// The engine's `Ready` payload — version, platform, and what it can actually do.
  final Ready ready;

  final List<DeviceInfo> devices;

  /// The open pairing window, if there is one.
  final PairingState? pairing;

  AdminReady copyWith({
    List<DeviceInfo>? devices,
    PairingState? pairing,
    bool clearPairing = false,
  }) => AdminReady(
    ready: ready,
    devices: devices ?? this.devices,
    pairing: clearPairing ? null : (pairing ?? this.pairing),
  );

  /// True when the host cannot inject input at all.
  ///
  /// The dashboard surfaces this loudly: buttons that silently do nothing are the worst
  /// possible failure, and `docs/SERVER.md` §6 makes it the loudest thing on the screen.
  bool get inputUnavailable =>
      !ready.capabilities.mediaKeys &&
      !ready.capabilities.mouse &&
      !ready.capabilities.textUnicode;
}

class AdminFailed extends AdminState {
  const AdminFailed(this.message);

  final String message;
}

/// Connects to the engine over loopback with the `admin` role, and keeps that view fresh.
///
/// `admin` cannot be requested — the engine grants it only to a loopback peer presenting the
/// local admin token, and the panel therefore has no keypair and never pairs itself
/// (`docs/ARCHITECTURE.md` §5.4).
class AdminSession extends Notifier<AdminState> {
  Transport? _transport;
  ProtocolClient? _client;
  StreamSubscription<Envelope<RawPayload>>? _events;

  @override
  AdminState build() {
    ref.onDispose(_teardown);
    return const AdminConnecting();
  }

  ProtocolClient? get client => state is AdminReady ? _client : null;

  /// Connects, starting the daemon first if it is installed but not running.
  ///
  /// `docs/SERVER.md` §5: try to connect; if refused, look for the binary; if found, start it
  /// and poll; if not found, offer to install.
  Future<void> connect({bool autoStart = true}) async {
    await _teardown();
    state = const AdminConnecting();

    var credentials = await readEngineCredentials();

    if (credentials == null) {
      // No config directory means the engine has never run. Starting it once creates the
      // identity, the certificate and the token — so an install is only needed if there is no
      // binary to start.
      final executable = findEngineExecutable();
      if (executable == null || !autoStart) {
        state = AdminNotInstalled(canInstall: executable != null);
        return;
      }
      if (!await _startDaemon(executable)) {
        state = const AdminEngineStopped('The engine did not start.');
        return;
      }
      credentials = await readEngineCredentials();
      if (credentials == null) {
        state = const AdminEngineStopped(
          'The engine started but wrote no credentials. Check its log.',
        );
        return;
      }
    }

    if (await _attach(credentials)) return;

    if (!autoStart) {
      state = const AdminEngineStopped('The engine is not running.');
      return;
    }

    final executable = findEngineExecutable();
    if (executable == null) {
      state = const AdminNotInstalled(canInstall: false);
      return;
    }
    if (!await _startDaemon(executable)) {
      state = const AdminEngineStopped(
        'The engine did not come up within 10 seconds. Check its log.',
      );
      return;
    }

    // Credentials may have only just been written, so re-read before the second attempt.
    final refreshed = await readEngineCredentials() ?? credentials;
    if (!await _attach(refreshed)) {
      state = const AdminEngineStopped(
        'The engine is running but refused the connection.',
      );
    }
  }

  /// Opens a socket and completes the admin handshake. Returns false if the engine is not there.
  Future<bool> _attach(EngineCredentials credentials) async {
    final transport = LanTransport(
      uri: Uri.parse('wss://127.0.0.1:$defaultPort/ws'),
      expectedFingerprint: credentials.fingerprint,
      hostName: 'this computer',
    );

    try {
      await transport.connect();
    } on FingerprintMismatch {
      // Loopback still pins: the certificate on disk must match the one presented. A mismatch
      // here means something other than this engine is on the port.
      await transport.close();
      state = const AdminFailed(
        'Something other than MuxDeck is listening on port 47654. '
        'The certificate does not match the one in the config directory.',
      );
      return true; // Handled — a retry would not help.
    } catch (_) {
      await transport.close();
      return false;
    }

    _transport = transport;
    final client = ProtocolClient(transport);
    _client = client;

    try {
      // The admin form: a token instead of a device ID, answered `mode: "ready"` with no
      // challenge round trip. `docs/PROTOCOL.md` §4.1.
      final response = await client.request(KnownOp.sessionHello, {
        'admin_token': credentials.adminToken,
        'client_version': panelVersion,
        'platform': _platformName,
      });

      final hello = HelloResponse.fromJson(response);
      if (hello is! ReadyResponse) {
        state = const AdminFailed(
          'The engine issued a challenge to the local panel, which should never happen.',
        );
        return true;
      }

      final devices = await _fetchDevices(client);
      state = AdminReady(ready: hello.ready, devices: devices);
      _listenForEvents(client);
      return true;
    } on EngineRefused catch (e) {
      state = AdminFailed(
        e.code == 'NOT_AUTHORIZED'
            ? 'The engine refused the admin token. Try restarting the engine.'
            : e.message,
      );
      return true;
    } catch (_) {
      await _teardown();
      return false;
    }
  }

  Future<List<DeviceInfo>> _fetchDevices(ProtocolClient client) async {
    final response = await client.request(KnownOp.pairListDevices, const {});
    return PairListDevicesResponse.fromJson(response).devices;
  }

  /// Applies engine-pushed events, so the panel reflects reality without polling.
  void _listenForEvents(ProtocolClient client) {
    _events = client.events.listen((event) {
      final current = state;
      if (current is! AdminReady) return;

      switch (event.op.known) {
        case KnownOp.deviceChanged:
          state = current.copyWith(
            devices: DeviceChangedEvent.fromJson(event.d.json).devices,
          );
        case KnownOp.pairingState:
          final pairing = PairingState.fromJson(event.d.json);
          state = pairing.active
              ? current.copyWith(pairing: pairing)
              : current.copyWith(clearPairing: true);
        case KnownOp.engineShutdown:
          state = const AdminEngineStopped('The engine shut down.');
        default:
          break;
      }
    });
  }

  /// Spawns the daemon and waits for its socket, up to [startupTimeout].
  Future<bool> _startDaemon(File executable) async {
    try {
      // Detached: the engine outlives the panel by design. Closing this window must not stop
      // the deck — `docs/ARCHITECTURE.md` §3.
      await Process.start(
        executable.path,
        const [],
        mode: ProcessStartMode.detached,
      );
    } catch (_) {
      return false;
    }

    final deadline = DateTime.now().add(startupTimeout);
    while (DateTime.now().isBefore(deadline)) {
      await Future<void>.delayed(const Duration(milliseconds: 300));
      if (await _portIsOpen()) return true;
    }
    return false;
  }

  /// A plain TCP probe. Cheaper than a TLS handshake for "is anything listening".
  Future<bool> _portIsOpen() async {
    try {
      final socket = await Socket.connect(
        InternetAddress.loopbackIPv4,
        defaultPort,
        timeout: const Duration(milliseconds: 500),
      );
      socket.destroy();
      return true;
    } catch (_) {
      return false;
    }
  }

  /// Opens a pairing window and returns the payload for the QR screen.
  Future<PairBeginResponse> beginPairing({int ttlSeconds = 120}) async {
    final client = _requireClient();
    final response = await client.request(KnownOp.pairBegin, {
      'ttl_seconds': ttlSeconds,
    });
    return PairBeginResponse.fromJson(response);
  }

  Future<void> cancelPairing() async {
    await _requireClient().request(KnownOp.pairCancel, const {});
  }

  /// Removes a device. The engine drops its live socket immediately.
  Future<void> revokeDevice(String deviceId) async {
    await _requireClient().request(KnownOp.pairRevoke, {'device_id': deviceId});
  }

  ProtocolClient _requireClient() {
    final client = _client;
    if (client == null) {
      throw const TransportFailed('Not connected to the engine.');
    }
    return client;
  }

  /// Disconnects the panel. **Does not stop the engine** — see [stopEngine].
  Future<void> disconnect() async {
    await _teardown();
    state = const AdminEngineStopped('Disconnected.');
  }

  Future<void> _teardown() async {
    await _events?.cancel();
    _events = null;
    await _client?.close();
    _client = null;
    await _transport?.close();
    _transport = null;
  }
}

/// Reported to the engine in `session.hello`.
const panelVersion = '0.1.0';

String get _platformName {
  if (io.Platform.isWindows) return 'windows';
  if (io.Platform.isMacOS) return 'macos';
  return 'linux';
}
