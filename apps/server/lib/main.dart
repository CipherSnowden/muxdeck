import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:tray_manager/tray_manager.dart';
import 'package:window_manager/window_manager.dart';

import 'app.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await windowManager.ensureInitialized();

  await windowManager.waitUntilReadyToShow(
    const WindowOptions(
      // `docs/SERVER.md` §8. The minimum is what the layout editor needs in M6; setting it now
      // avoids a resize that breaks a screen that does not exist yet.
      size: Size(1100, 760),
      minimumSize: Size(900, 640),
      center: true,
      title: 'MuxDeck',
      titleBarStyle: TitleBarStyle.normal,
    ),
    () async {
      await windowManager.show();
      await windowManager.focus();
    },
  );

  // Intercept the close button so it hides to the tray rather than exiting. The panel is not
  // load-bearing — closing it must not stop the deck — but vanishing entirely on a stray click
  // is still the wrong behaviour. `docs/SERVER.md` §7.
  await windowManager.setPreventClose(true);

  await trayManager.setIcon(
    Platform.isWindows
        ? 'windows/runner/resources/app_icon.ico'
        : 'assets/tray_icon.png',
  );

  runApp(const ProviderScope(child: MuxDeckPanel()));
}
