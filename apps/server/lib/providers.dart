/// The provider graph.
///
/// Modern providers only — `Notifier`/`AsyncNotifier` and the read-only `Provider` family.
/// Never `StateProvider`, `StateNotifierProvider` or `ChangeNotifierProvider`: those live behind
/// a separate `legacy.dart` import that this project never reaches for. Hand-written, no
/// codegen, matching `docs/CLIENT.md` §2 — the same rules apply to both apps.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:muxdeck_protocol/muxdeck_protocol.dart';

import 'domain/actions_controller.dart';
import 'domain/admin_session.dart';
import 'domain/editor_controller.dart';
import 'domain/log_controller.dart';
import 'domain/settings_controller.dart';
import 'domain/telemetry_controller.dart';

final adminSessionProvider = NotifierProvider<AdminSession, AdminState>(
  AdminSession.new,
);

final editorProvider = NotifierProvider<EditorController, Profile?>(
  EditorController.new,
);

final settingsProvider = NotifierProvider<SettingsController, SettingsState>(
  SettingsController.new,
);

final actionsProvider = NotifierProvider<ActionsController, ActionsState>(
  ActionsController.new,
);

final telemetryProvider = NotifierProvider<TelemetryController, TelemetryState>(
  TelemetryController.new,
);

/// Reads the engine's log file directly rather than over the socket. The engine has no
/// `log.tail` op, and adding one would put a file read on the protocol for something the panel
/// can only ever do when it is on the same machine anyway.
final logProvider = NotifierProvider<LogController, LogState>(
  LogController.new,
);
