/// The connection lifecycle, as the UI sees it.
library;

import 'package:muxdeck_protocol/muxdeck_protocol.dart';

sealed class SessionState {
  const SessionState();

  /// True only when ops may actually be sent.
  bool get isReady => this is SessionReady;
}

class SessionDisconnected extends SessionState {
  const SessionDisconnected();
}

class SessionConnecting extends SessionState {
  const SessionConnecting(this.hostName);

  final String hostName;
}

/// TLS is up and the challenge is being signed.
///
/// A separate state from [SessionConnecting] because it means something different to the user:
/// the host was reached and its identity checked out, so a failure from here is about *this
/// device's* credentials, not the network.
class SessionAuthenticating extends SessionState {
  const SessionAuthenticating(this.hostName);

  final String hostName;
}

class SessionReady extends SessionState {
  const SessionReady({
    required this.hostName,
    required this.ready,
    this.roundTripMs,
  });

  final String hostName;

  /// The engine's `Ready` payload. Its `capabilities` block decides which buttons render
  /// enabled, so a deck never offers an action the host cannot perform.
  final Ready ready;

  /// Most recent round trip, milliseconds. Null until the first ping completes.
  final int? roundTripMs;

  SessionReady withRoundTrip(int ms) =>
      SessionReady(hostName: hostName, ready: ready, roundTripMs: ms);
}

class SessionFailed extends SessionState {
  const SessionFailed(this.error, {this.willRetry = false});

  final AppError error;

  /// Whether a reconnect is already scheduled.
  ///
  /// The distinction is the whole difference between "give up and go to the host list" and "sit
  /// tight, it is coming back" — a deck that reconnects silently but reads as broken is worse
  /// than one that says so.
  final bool willRetry;
}
