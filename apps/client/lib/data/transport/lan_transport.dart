/// The Wi-Fi/LAN transport: a TLS WebSocket with a pinned certificate.
library;

import 'dart:async';
import 'dart:io';

import 'package:crypto/crypto.dart';
import 'package:web_socket_channel/io.dart';
import 'package:web_socket_channel/web_socket_channel.dart';

import '../../core/errors.dart';
import 'transport.dart';

/// How long to wait for the socket before giving up.
///
/// Short on purpose: the host is on the same LAN, so a slow connect means it is not there
/// rather than that it is far away, and a deck that hangs for 30 seconds is worse than one that
/// says so in five.
const _connectTimeout = Duration(seconds: 5);

class LanTransport implements Transport, FingerprintReporting {
  LanTransport({
    required this.uri,
    required this.expectedFingerprint,
    this.hostName = 'the host',
  });

  /// `wss://<host>:<port>/ws`.
  final Uri uri;

  /// Lowercase hex, 64 characters. From the pairing QR, or from the stored host record.
  ///
  /// **Empty means trust-on-first-use** and is legal on exactly one path: manual pairing, where
  /// the user typed an address and nothing was carried out of band. Every other caller has a
  /// fingerprint and must pass it, or the pin is not a pin.
  final String expectedFingerprint;

  /// The certificate the host actually presented, once a connection has been attempted.
  ///
  /// Recorded so manual pairing can store what it saw. On the QR path this always equals
  /// [expectedFingerprint], because anything else would have been rejected.
  @override
  String? presentedFingerprint;

  /// For error messages only.
  final String hostName;

  IOWebSocketChannel? _channel;
  final _frames = StreamController<String>.broadcast();

  /// Set by the certificate callback when a certificate is rejected.
  ///
  /// The TLS failure surfaces as a generic `HandshakeException` buried inside a
  /// `WebSocketChannelException`, which is indistinguishable from any other connection problem.
  /// Recording the reason at the point it is actually known is the only way to tell the user
  /// their host's identity changed rather than that their Wi-Fi is flaky.
  bool _fingerprintRejected = false;

  @override
  Future<void> connect() async {
    _fingerprintRejected = false;

    final client = HttpClient(context: SecurityContext(withTrustedRoots: false))
      ..badCertificateCallback = _acceptOnlyPinned
      ..connectionTimeout = _connectTimeout;

    try {
      final channel = IOWebSocketChannel.connect(
        uri,
        customClient: client,
        connectTimeout: _connectTimeout,
      );
      await channel.ready;

      _channel = channel;
      channel.stream.listen(
        (frame) {
          if (frame is String) _frames.add(frame);
        },
        onError: _frames.addError,
        onDone: () {
          if (!_frames.isClosed) _frames.close();
        },
      );
    } catch (e) {
      // Covers WebSocketChannelException and everything under it; _describe unwraps the cause.
      client.close(force: true);
      throw _describe(e);
    }
  }

  /// Accepts exactly one certificate: the one whose SHA-256 matches [expectedFingerprint].
  ///
  /// The [SecurityContext] above has no trusted roots, so **every** certificate reaches this
  /// callback and this comparison is the only thing that decides. With the default trust store,
  /// a certificate chaining to a public CA would skip the callback entirely and be accepted —
  /// the pin would simply not be consulted. See `docs/CLIENT.md` §3.
  bool _acceptOnlyPinned(X509Certificate cert, String host, int port) {
    // Over the DER, not the PEM. `toString()` on a Digest is lowercase hex with no separators,
    // which is exactly the representation docs/PROTOCOL.md §1 specifies.
    final actual = sha256.convert(cert.der).toString();
    presentedFingerprint = actual;

    // Trust-on-first-use, permitted only when no fingerprint is known — see the field's doc.
    if (expectedFingerprint.isEmpty) return true;

    final matches = actual == expectedFingerprint.toLowerCase();
    if (!matches) _fingerprintRejected = true;
    return matches;
  }

  /// Turns a connection failure into something the user can act on.
  AppError _describe(Object error) {
    if (_fingerprintRejected) return const FingerprintMismatch();

    // web_socket_channel 3.x wraps the real cause; unwrap before matching on it.
    final cause = error is WebSocketChannelException ? (error.inner ?? error) : error;

    if (cause is HandshakeException) {
      // A TLS failure that was not our rejection is still an identity problem: the host is
      // presenting something we cannot verify.
      return const FingerprintMismatch();
    }
    if (cause is SocketException || cause is TimeoutException) {
      return HostUnreachable(hostName);
    }
    return TransportFailed('$cause');
  }

  @override
  void send(String frame) {
    final channel = _channel;
    if (channel == null) {
      throw const TransportFailed('Not connected.');
    }
    channel.sink.add(frame);
  }

  @override
  Stream<String> get frames => _frames.stream;

  @override
  Future<void> close() async {
    await _channel?.sink.close();
    _channel = null;
    if (!_frames.isClosed) await _frames.close();
  }
}
