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
