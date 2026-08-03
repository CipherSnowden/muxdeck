/// Engine settings, over the admin socket. `docs/PROTOCOL.md` §4.6.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:muxdeck_protocol/muxdeck_protocol.dart';

import '../providers.dart';

/// The settings screen's state.
///
/// `restartRequired` is sticky once set: only a port change needs a restart, and the notice has
/// to survive every later save until the user actually restarts the engine. Clearing it on the
/// next write would hide the one thing they still have to do.
class SettingsState {
  const SettingsState({
    this.settings,
    this.saving = false,
    this.restartRequired = false,
    this.error,
  });

  final Settings? settings;
  final bool saving;
  final bool restartRequired;
  final String? error;

  SettingsState copyWith({
    Settings? settings,
    bool? saving,
    bool? restartRequired,
    String? error,
    bool clearError = false,
  }) => SettingsState(
    settings: settings ?? this.settings,
    saving: saving ?? this.saving,
    restartRequired: restartRequired ?? this.restartRequired,
    error: clearError ? null : (error ?? this.error),
  );
}

class SettingsController extends Notifier<SettingsState> {
  @override
  SettingsState build() {
    // Rebuilt whenever the admin session changes, so reconnecting reloads rather than showing
    // whatever was on screen when the engine went away.
    ref.watch(adminSessionProvider);
    return const SettingsState();
  }

  ProtocolClient? get _client => ref.read(adminSessionProvider.notifier).client;

  Future<void> load() async {
    final client = _client;
    if (client == null) return;

    try {
      final response = await client.request(KnownOp.settingsGet, const {});
      state = state.copyWith(
        settings: Settings.fromJson(response),
        clearError: true,
      );
    } on AppError catch (error) {
      state = state.copyWith(error: error.message);
    }
  }

  /// Sends a patch of only what changed.
  ///
  /// A partial object, not the whole settings block: two panels open at once would otherwise
  /// each write back every field, and the second save would silently undo the first's unrelated
  /// change (`docs/PROTOCOL.md` §4.6).
  Future<void> save(SettingsPatch patch) async {
    final client = _client;
    if (client == null) return;

    state = state.copyWith(saving: true, clearError: true);
    try {
      final response = await client.request(
        KnownOp.settingsSet,
        patch.toJson(),
      );
      final result = SettingsSetResponse.fromJson(response);

      state = state.copyWith(
        saving: false,
        restartRequired: state.restartRequired || result.restartRequired,
      );
      await load();
    } on AppError catch (error) {
      state = state.copyWith(saving: false, error: error.message);
    }
  }
}
