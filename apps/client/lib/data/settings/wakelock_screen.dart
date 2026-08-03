/// The real [ScreenLock], over `wakelock_plus`.
///
/// Isolated in its own file so nothing that imports it comes along for the ride in a widget
/// test: `wakelock_plus` needs a platform channel, and a test host has none.
library;

import 'package:wakelock_plus/wakelock_plus.dart';

import '../../domain/settings/settings_controller.dart';

class WakelockScreen implements ScreenLock {
  const WakelockScreen();

  @override
  Future<void> setKeepAwake({required bool enabled}) =>
      WakelockPlus.toggle(enable: enabled);
}
