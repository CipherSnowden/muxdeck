/// The provider graph.
///
/// Modern providers only — `Notifier`/`AsyncNotifier` and the read-only `Provider` family.
/// Never `StateProvider`, `StateNotifierProvider` or `ChangeNotifierProvider`: those live behind
/// a separate `legacy.dart` import that this project never reaches for. Hand-written, no
/// codegen, matching `docs/CLIENT.md` §2 — the same rules apply to both apps.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'domain/admin_session.dart';

final adminSessionProvider = NotifierProvider<AdminSession, AdminState>(
  AdminSession.new,
);
