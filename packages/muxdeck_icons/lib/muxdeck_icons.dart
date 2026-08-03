/// The curated icon set shared by the deck and the layout editor.
///
/// Separate from `muxdeck_protocol` deliberately: `IconData` comes from `package:flutter`, and
/// keeping the protocol package plain Dart is what lets its CI job run on the Dart SDK alone in
/// seconds. This package depends on Flutter; the protocol package must never.
///
/// See `docs/CLIENT.md` §5.
library;

export 'src/icon_map.dart';
