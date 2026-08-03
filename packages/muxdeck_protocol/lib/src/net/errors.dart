/// Failures the user interface has to tell apart.
///
/// These are thrown, not returned. Riverpod's `AsyncValue` already models loading, data and
/// error, so a parallel `Result` type would be a second error channel to keep in sync with the
/// first — see `docs/CLIENT.md` §5.
///
/// Every case carries a [message] written for a person rather than a log, because each of these
/// reaches the screen verbatim. The distinctions are not academic: `docs/CLIENT.md` §6 requires
/// three discovery failures to read differently, precisely because they need different fixes.
library;

sealed class AppError implements Exception {
  const AppError(this.message);

  /// Shown to the user as-is.
  final String message;

  @override
  String toString() => '$runtimeType: $message';
}

/// Discovery ran and turned up nothing.
class NoHostsFound extends AppError {
  const NoHostsFound()
    : super(
        'No MuxDeck hosts found. Check that the desktop app is running and '
        'that this device is on the same Wi-Fi network.',
      );
}

/// A host is there, but the socket was refused or timed out.
///
/// Distinct from [NoHostsFound] on purpose: the daemon being stopped and the phone being on the
/// wrong network look identical in a spinner and need opposite fixes.
class HostUnreachable extends AppError {
  const HostUnreachable(this.hostName)
    : super('Found $hostName, but it is not accepting connections.');

  final String hostName;
}

/// The certificate did not match the fingerprint stored at pairing time.
///
/// **Never retried and never downgraded to a generic connection error.** Either the host
/// regenerated its identity, or something is impersonating it; the user has to decide which.
class FingerprintMismatch extends AppError {
  const FingerprintMismatch()
    : super(
        "This host's identity has changed. It may have been reset, or this "
        'may not be the computer you paired with. Re-pair to continue.',
      );
}

/// The engine refused the pairing attempt — wrong code, expired window, or bad proof.
class PairingRejected extends AppError {
  const PairingRejected(super.message);
}

/// This device is not in the host's registry, so its key cannot authenticate.
///
/// Usually means the pairing was revoked from the desktop panel while the client still held a
/// host record.
class NotPaired extends AppError {
  const NotPaired()
    : super(
        'This device is no longer paired with that host. Pair it again to continue.',
      );
}

/// The socket failed for a reason that is not one of the above.
class TransportFailed extends AppError {
  const TransportFailed(super.message);
}

/// The engine answered with a protocol-level error.
class EngineRefused extends AppError {
  const EngineRefused(this.code, super.message);

  /// The wire error code, e.g. `NOT_AUTHORIZED`. Kept for logs and tests; the user sees
  /// [message].
  final String code;
}
