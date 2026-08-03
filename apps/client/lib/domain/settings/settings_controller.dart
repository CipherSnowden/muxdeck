/// Device-local settings, and the wakelock they drive.
library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../data/settings/app_settings.dart';
import '../../providers.dart';

/// Turns the screen lock off and on.
///
/// An interface with one real implementation, for the same reason `Transport` is: it is the seam
/// that keeps `wakelock_plus` — which needs a platform channel and therefore a real device — out
/// of every widget test that happens to build the deck.
abstract class ScreenLock {
  Future<void> setKeepAwake({required bool enabled});
}

class SettingsController extends Notifier<AppSettings> {
  AppSettingsStore get _store => ref.read(appSettingsStoreProvider);
  ScreenLock get _screenLock => ref.read(screenLockProvider);

  @override
  AppSettings build() {
    final settings = _store.load();
    // Applied at startup, not only when the toggle is touched: the setting is remembered across
    // launches, so the wakelock has to be re-established on every one.
    unawaited(_apply(settings));
    return settings;
  }

  Future<void> setKeepScreenAwake({required bool enabled}) =>
      _update(state.copyWith(keepScreenAwake: enabled));

  Future<void> setShowRoundTrip({required bool enabled}) =>
      _update(state.copyWith(showRoundTrip: enabled));

  Future<void> _update(AppSettings settings) async {
    state = settings;
    await _apply(settings);
    await _store.save(settings);
  }

  /// Failures are swallowed on purpose.
  ///
  /// A platform that refuses the wakelock — or a test host with no platform channel at all — is
  /// not a reason to fail the settings write or crash the app. The worst case is a screen that
  /// dims, which the user can see for themselves.
  Future<void> _apply(AppSettings settings) async {
    try {
      await _screenLock.setKeepAwake(enabled: settings.keepScreenAwake);
    } catch (_) {
      // Deliberately ignored; see above.
    }
  }
}
