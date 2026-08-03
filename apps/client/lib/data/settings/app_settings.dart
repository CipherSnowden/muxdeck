/// Device-local preferences. `docs/CLIENT.md` §6 (Settings).
///
/// These are **this phone's** settings and never travel: engine settings are a separate,
/// admin-only thing the panel edits (`docs/PROTOCOL.md` §4.6). A deck has no business changing
/// the host's port.
library;

import 'package:shared_preferences/shared_preferences.dart';

const _keepAwakeKey = 'muxdeck.settings.keepScreenAwake';
const _showRoundTripKey = 'muxdeck.settings.showRoundTrip';

/// What the settings screen edits.
class AppSettings {
  const AppSettings({this.keepScreenAwake = true, this.showRoundTrip = true});

  /// Hold the screen on while the deck is showing.
  ///
  /// Defaults to **on**: a deck that sleeps after thirty seconds is useless, and a user who
  /// props a tablet beside their keyboard has already told you what they want it for.
  final bool keepScreenAwake;

  /// Show the round trip in the status chip.
  final bool showRoundTrip;

  AppSettings copyWith({bool? keepScreenAwake, bool? showRoundTrip}) =>
      AppSettings(
        keepScreenAwake: keepScreenAwake ?? this.keepScreenAwake,
        showRoundTrip: showRoundTrip ?? this.showRoundTrip,
      );
}

/// Reads and writes [AppSettings] through `shared_preferences`.
class AppSettingsStore {
  AppSettingsStore(this._prefs);

  final SharedPreferences _prefs;

  AppSettings load() => AppSettings(
    keepScreenAwake: _prefs.getBool(_keepAwakeKey) ?? true,
    showRoundTrip: _prefs.getBool(_showRoundTripKey) ?? true,
  );

  Future<void> save(AppSettings settings) async {
    await _prefs.setBool(_keepAwakeKey, settings.keepScreenAwake);
    await _prefs.setBool(_showRoundTripKey, settings.showRoundTrip);
  }
}
