/// The handshake state machine. `docs/PROTOCOL.md` §3.
library;

import 'dart:async';
import 'dart:convert';
import 'dart:math';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:muxdeck_protocol/muxdeck_protocol.dart';

import '../../data/hosts/host_record.dart';
import '../../data/identity/device_identity.dart';
import '../../providers.dart';
import 'session_state.dart';

/// How often to ping while connected. `docs/CLIENT.md` §7.
const pingInterval = Duration(seconds: 5);

/// Consecutive missed pings that force a reconnect.
const missedPingsBeforeReconnect = 3;

/// The reconnect schedule from `docs/CLIENT.md` §7, in milliseconds. The last entry is the cap.
const reconnectBackoffMs = [500, 1000, 2000, 4000, 8000];

/// How far either side of the scheduled delay the jitter can land.
const reconnectJitter = 0.25;

/// The delay before retry number [attempt], counting from zero.
///
/// Jittered on purpose. Without it, a room of decks that lost the same access point comes back
/// in lockstep and hammers the engine in waves — and worse, each wave fails together, so they
/// stay synchronised for as long as the outage lasts.
Duration reconnectDelay(int attempt, {Random? random}) {
  final index = attempt.clamp(0, reconnectBackoffMs.length - 1);
  final base = reconnectBackoffMs[index];
  final spread =
      (random ?? _random).nextDouble() * 2 * reconnectJitter - reconnectJitter;
  return Duration(milliseconds: (base * (1 + spread)).round());
}

final _random = Random();

/// Builds the transport for a host. Provided so tests can substitute a fake.
typedef TransportFactory = Transport Function(HostRecord host);

/// Drives one connection from disconnected to ready, and keeps it alive.
class SessionController extends Notifier<SessionState> {
  TransportFactory get _transportFactory => ref.read(transportFactoryProvider);
  DeviceIdentityStore get _identityStore =>
      ref.read(deviceIdentityStoreProvider);

  Transport? _transport;
  ProtocolClient? _client;
  Timer? _pingTimer;
  var _missedPings = 0;

  /// The host to reconnect to. Kept after a failure precisely so there is something to retry.
  HostRecord? _host;

  /// Whether the user wants a connection. Cleared only by [disconnect], so a dropped socket
  /// retries but a deliberate exit stays exited.
  var _wantConnected = false;

  Timer? _reconnectTimer;
  var _attempt = 0;

  @override
  SessionState build() {
    ref.onDispose(() {
      _reconnectTimer?.cancel();
      unawaited(_teardown());
    });
    return const SessionDisconnected();
  }

  /// The live protocol client, or null when not ready.
  ///
  /// Exposed so the deck can send input ops without routing every one through this controller.
  ProtocolClient? get client => state.isReady ? _client : null;

  /// Connects and authenticates, retrying with backoff if the connection drops.
  ///
  /// Safe to call repeatedly: an in-flight or established connection is torn down first, so a
  /// retry after failure does not leak the previous socket.
  Future<void> connect(HostRecord host) async {
    _host = host;
    _wantConnected = true;
    _attempt = 0;
    _reconnectTimer?.cancel();
    _reconnectTimer = null;
    await _open(host);
  }

  /// Reconnects at once, skipping whatever backoff was pending.
  ///
  /// Called when the app returns from the background: iOS will have killed the socket while
  /// suspended, and making the user wait out an eight-second backoff to press a button they are
  /// already looking at is the wrong answer. `docs/CLIENT.md` §7.
  Future<void> reconnectNow() async {
    final host = _host;
    if (!_wantConnected || host == null || state.isReady) return;

    _reconnectTimer?.cancel();
    _reconnectTimer = null;
    _attempt = 0;
    await _open(host);
  }

  Future<void> _open(HostRecord host) async {
    await _teardown();
    state = SessionConnecting(host.hostName);

    try {
      final identity = await _identityStore.load();

      final transport = _transportFactory(host);
      _transport = transport;
      await transport.connect();

      final client = ProtocolClient(transport);
      _client = client;

      final ready = await _handshake(client, identity, host);

      state = SessionReady(hostName: host.hostName, ready: ready);
      _attempt = 0;
      _startPinging();
    } on AppError catch (error) {
      await _teardown();
      _fail(error);
    } catch (error) {
      await _teardown();
      _fail(TransportFailed('$error'));
    }
  }

  /// Reports a failure and schedules a retry, unless retrying cannot possibly help.
  void _fail(AppError error) {
    // A device the host does not recognise will be refused just as fast on every attempt.
    // Retrying would spin forever and bury the one message that tells the user what to do:
    // pair again.
    final worthRetrying = _wantConnected && error is! NotPaired;

    state = SessionFailed(error, willRetry: worthRetrying);
    if (!worthRetrying) return;

    final delay = reconnectDelay(_attempt);
    _attempt++;
    _reconnectTimer?.cancel();
    _reconnectTimer = Timer(delay, () {
      final host = _host;
      if (_wantConnected && host != null) unawaited(_open(host));
    });
  }

  /// `session.hello` → challenge → `session.auth` → `Ready`.
  Future<Ready> _handshake(
    ProtocolClient client,
    DeviceIdentity identity,
    HostRecord host,
  ) async {
    final helloResponse = await client
        .request(KnownOp.sessionHello, {
          'device_id': identity.deviceId,
          'client_version': clientVersion,
          'platform': currentPlatform.wire,
        })
        .catchError(_asAppError);

    final hello = HelloResponse.fromJson(helloResponse);

    // A deck must always be challenged. `mode: "ready"` here would mean the engine granted a
    // role without verifying anything, which is a bug worth failing loudly on rather than
    // quietly accepting.
    if (hello is! Challenge) {
      throw const TransportFailed(
        'The host completed the handshake without a challenge, which should never happen.',
      );
    }

    state = SessionAuthenticating(host.hostName);

    // Built by muxdeck_protocol, fixture-tested byte-for-byte against the Rust engine in M1.
    // Never assemble this buffer inline: a mismatch authenticates nothing and produces no
    // symptom worth reading.
    final message = sessionAuthMessage(
      nonce: base64Decode(hello.nonce),
      deviceId: identity.deviceId,
      hostId: hello.hostId,
    );
    final signature = await identity.sign(message);

    final authResponse = await client
        .request(KnownOp.sessionAuth, {'signature': base64Encode(signature)})
        .catchError(_asAppError);

    return Ready.fromJson(authResponse);
  }

  /// Translates wire errors into the failures the UI distinguishes.
  Never _asAppError(Object error) {
    if (error is EngineRefused) {
      throw switch (error.code) {
        'UNKNOWN_DEVICE' => const NotPaired(),
        'BAD_SIGNATURE' => const NotPaired(),
        _ => EngineRefused(error.code, error.message),
      };
    }
    throw error is AppError ? error : TransportFailed('$error');
  }

  void _startPinging() {
    _missedPings = 0;
    _pingTimer?.cancel();
    _pingTimer = Timer.periodic(pingInterval, (_) => unawaited(_ping()));
    unawaited(_ping());
  }

  Future<void> _ping() async {
    final client = _client;
    final current = state;
    if (client == null || current is! SessionReady) return;

    final sentAt = DateTime.now();
    try {
      await client.request(KnownOp.systemPing, {
        't_client': sentAt.millisecondsSinceEpoch,
      });
    } catch (_) {
      _missedPings++;
      if (_missedPings >= missedPingsBeforeReconnect) {
        _pingTimer?.cancel();
        unawaited(_teardown());
        _fail(const TransportFailed('The host stopped responding.'));
      }
      return;
    }

    _missedPings = 0;

    // RTT is measured locally from send to receive. `t_engine` is deliberately not trusted for
    // this — the two clocks are unrelated. `docs/PROTOCOL.md` §4.8.
    final elapsed = DateTime.now().difference(sentAt).inMilliseconds;
    final latest = state;
    if (latest is SessionReady) state = latest.withRoundTrip(elapsed);
  }

  Future<void> disconnect() async {
    _wantConnected = false;
    _host = null;
    _reconnectTimer?.cancel();
    _reconnectTimer = null;
    await _teardown();
    state = const SessionDisconnected();
  }

  Future<void> _teardown() async {
    _pingTimer?.cancel();
    _pingTimer = null;
    await _client?.close();
    _client = null;
    await _transport?.close();
    _transport = null;
  }
}

/// Reported to the engine in `session.hello`.
const clientVersion = '0.1.0';

/// This build's platform, as the protocol names it.
///
/// `defaultTargetPlatform` rather than `dart:io`'s `Platform`, because it is settable in widget
/// tests and does not drag `dart:io` into code that widget tests exercise. The client ships to
/// Android and iOS only; anything else means a test host, and `android` is the honest answer
/// there since the engine only uses this field for display.
Platform get currentPlatform => switch (defaultTargetPlatform) {
  TargetPlatform.iOS => Platform.ios,
  _ => Platform.android,
};
