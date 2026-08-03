/// Request/response correlation over a [Transport].
///
/// Everything above this layer speaks in payloads; this is the only place that knows about
/// envelopes, correlation IDs and the difference between a response and an event.
library;

import 'dart:async';
import 'dart:convert';

import '../envelope.dart';

import 'errors.dart';
import 'transport.dart';

/// How long to wait for a response before giving up on it.
///
/// The engine answers in single-digit milliseconds on a LAN, so this is not a latency budget —
/// it is the point at which a request is assumed lost so its future does not hang forever.
const _requestTimeout = Duration(seconds: 10);

/// Sends requests and matches responses to them.
class ProtocolClient {
  ProtocolClient(this._transport) {
    _subscription = _transport.frames.listen(
      _onFrame,
      onError: _failAllPending,
      onDone: () =>
          _failAllPending(const TransportFailed('The connection closed.')),
    );
  }

  final Transport _transport;
  late final StreamSubscription<String> _subscription;

  final _pending = <String, Completer<Map<String, dynamic>>>{};
  final _events = StreamController<Envelope<RawPayload>>.broadcast();

  var _nextId = 0;

  /// Unsolicited messages from the engine — `profile.changed`, `telemetry.update` and friends.
  Stream<Envelope<RawPayload>> get events => _events.stream;

  /// Sends a request and completes with its response payload.
  ///
  /// Throws [EngineRefused] when the engine answers with an `err`, so callers can branch on the
  /// code rather than parsing envelopes themselves.
  Future<Map<String, dynamic>> request(
    KnownOp op,
    Map<String, dynamic> payload,
  ) {
    final id = 'c${_nextId++}';
    final completer = Completer<Map<String, dynamic>>();
    _pending[id] = completer;

    final envelope = <String, dynamic>{
      'v': protocolVersion,
      't': MessageType.req.wire,
      'id': id,
      'op': op.wire,
      'd': payload,
    };

    try {
      _transport.send(jsonEncode(envelope));
    } catch (e) {
      _pending.remove(id);
      rethrow;
    }

    return completer.future.timeout(
      _requestTimeout,
      onTimeout: () {
        _pending.remove(id);
        throw TransportFailed(
          '${op.wire} timed out after ${_requestTimeout.inSeconds}s.',
        );
      },
    );
  }

  /// Sends a request and ignores the reply.
  ///
  /// Used for button presses: `docs/CLIENT.md` §7 requires a press that cannot be sent to be
  /// dropped rather than queued, and waiting on the response would put network latency in front
  /// of the next press.
  void fireAndForget(KnownOp op, Map<String, dynamic> payload) {
    unawaited(
      request(op, payload).catchError((Object _) => <String, dynamic>{}),
    );
  }

  void _onFrame(String frame) {
    final Map<String, dynamic> json;
    try {
      json = jsonDecode(frame) as Map<String, dynamic>;
    } catch (_) {
      // An unparseable frame is the engine's bug, not something to crash the deck over.
      return;
    }

    final type = json['t'] as String?;
    final id = json['id'] as String?;

    if (type == MessageType.evt.wire) {
      _events.add(
        Envelope<RawPayload>(
          v: json['v'] as int? ?? protocolVersion,
          t: MessageType.evt,
          op: Op.parse(json['op'] as String? ?? ''),
          d: RawPayload(json['d'] as Map<String, dynamic>? ?? const {}),
        ),
      );
      return;
    }

    if (id == null) return;
    final completer = _pending.remove(id);
    if (completer == null || completer.isCompleted) return;

    final payload = json['d'] as Map<String, dynamic>? ?? const {};
    if (type == MessageType.err.wire) {
      completer.completeError(
        EngineRefused(
          payload['code'] as String? ?? 'INTERNAL',
          payload['message'] as String? ?? 'The host refused the request.',
        ),
      );
    } else {
      completer.complete(payload);
    }
  }

  void _failAllPending(Object error, [StackTrace? stack]) {
    final pending = List.of(_pending.values);
    _pending.clear();
    for (final completer in pending) {
      if (!completer.isCompleted) completer.completeError(error, stack);
    }
  }

  Future<void> close() async {
    await _subscription.cancel();
    _failAllPending(const TransportFailed('The connection closed.'));
    if (!_events.isClosed) await _events.close();
  }
}

/// An already-decoded payload, for events whose type is chosen by the reader.
class RawPayload implements Payload {
  const RawPayload(this.json);

  final Map<String, dynamic> json;

  @override
  Map<String, dynamic> toJson() => json;
}
