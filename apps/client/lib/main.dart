import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:shared_preferences/shared_preferences.dart';

import 'app.dart';
import 'data/hosts/host_store.dart';
import 'data/settings/app_settings.dart';
import 'providers.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();

  // Landscape-first, but portrait stays allowed: a phone in a pocket is used portrait, and
  // `docs/CLIENT.md` §6 requires the deck to work either way.
  await SystemChrome.setPreferredOrientations([
    DeviceOrientation.landscapeLeft,
    DeviceOrientation.landscapeRight,
    DeviceOrientation.portraitUp,
  ]);

  // Loaded before the app starts so no screen has to handle a store that is not ready yet.
  final prefs = await SharedPreferences.getInstance();

  runApp(
    ProviderScope(
      overrides: [
        hostStoreProvider.overrideWithValue(HostStore(prefs)),
        appSettingsStoreProvider.overrideWithValue(AppSettingsStore(prefs)),
      ],
      child: const MuxDeckApp(),
    ),
  );
}
