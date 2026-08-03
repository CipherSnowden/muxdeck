/// MuxDeck wire protocol types.
///
/// An implementation of `docs/PROTOCOL.md`, which is the single source of truth — not the
/// other way around. To change the protocol, edit that document first, then
/// `protocol/fixtures/`, then the Rust types in `engine/crates/muxdeck-core`, then this
/// package, in that order and in one commit.
///
/// Hand-written `fromJson`/`toJson` throughout: the protocol is small, and code generation
/// would add a build step and a `build_runner` dependency to save very little.
library;

export 'src/action.dart';
export 'src/envelope.dart';
export 'src/input.dart';
export 'src/pairing.dart';
export 'src/profile.dart';
export 'src/session.dart';
export 'src/settings.dart';
export 'src/signing.dart';
export 'src/telemetry.dart';

// Speaking the protocol, not just describing it. Both apps connect the same way — the mobile
// deck over the LAN and the desktop panel over loopback — and certificate pinning in particular
// must exist exactly once: two copies is how one of them gets a security fix and the other
// quietly does not. `docs/SERVER.md` §1.
//
// All of this is plain Dart, so the package stays Flutter-free and its CI job keeps running on
// the Dart SDK alone in seconds.
export 'src/net/errors.dart';
export 'src/net/lan_transport.dart';
export 'src/net/protocol_client.dart';
export 'src/net/transport.dart';
