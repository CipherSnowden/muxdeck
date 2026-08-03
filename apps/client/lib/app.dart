/// Root widget, theme, and the app-lifecycle hook.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'providers.dart';
import 'ui/connect/connect_page.dart';

class MuxDeckApp extends ConsumerStatefulWidget {
  const MuxDeckApp({super.key});

  @override
  ConsumerState<MuxDeckApp> createState() => _MuxDeckAppState();
}

class _MuxDeckAppState extends ConsumerState<MuxDeckApp> {
  late final AppLifecycleListener _lifecycle;

  @override
  void initState() {
    super.initState();

    // iOS tears down the socket while the app is suspended, and Android will too under memory
    // pressure. Waiting out an eight-second backoff after the user has already unlocked their
    // phone and is looking at the deck is the wrong answer, so resume skips the schedule
    // entirely (`docs/CLIENT.md` §7).
    _lifecycle = AppLifecycleListener(
      onResume: () => ref.read(sessionProvider.notifier).reconnectNow(),
    );

    // Reads the stored value and applies the wakelock. Built here rather than lazily on the
    // settings screen, which the user may never open.
    ref.read(settingsProvider);
  }

  @override
  void dispose() {
    _lifecycle.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'MuxDeck',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        useMaterial3: true,
        brightness: Brightness.dark,
        colorScheme: ColorScheme.fromSeed(
          seedColor: const Color(0xFF2D6CDF),
          brightness: Brightness.dark,
        ),
        scaffoldBackgroundColor: const Color(0xFF12141A),
      ),
      home: const ConnectPage(),
    );
  }
}
