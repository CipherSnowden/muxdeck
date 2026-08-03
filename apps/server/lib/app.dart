/// The panel shell: navigation, tray, and the window's close behaviour.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:tray_manager/tray_manager.dart';
import 'package:window_manager/window_manager.dart';

import 'domain/admin_session.dart';
import 'providers.dart';
import 'ui/actions_page.dart';
import 'ui/dashboard_page.dart';
import 'ui/devices_page.dart';
import 'ui/editor_page.dart';
import 'ui/pair_page.dart';
import 'ui/settings_page.dart';

class MuxDeckPanel extends StatelessWidget {
  const MuxDeckPanel({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'MuxDeck',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        useMaterial3: true,
        colorScheme: ColorScheme.fromSeed(seedColor: const Color(0xFF2D6CDF)),
      ),
      darkTheme: ThemeData(
        useMaterial3: true,
        colorScheme: ColorScheme.fromSeed(
          seedColor: const Color(0xFF2D6CDF),
          brightness: Brightness.dark,
        ),
      ),
      // Follows the system theme, as `docs/SERVER.md` §8 asks.
      themeMode: ThemeMode.system,
      home: const PanelHome(),
    );
  }
}

class PanelHome extends ConsumerStatefulWidget {
  const PanelHome({super.key});

  @override
  ConsumerState<PanelHome> createState() => _PanelHomeState();
}

class _PanelHomeState extends ConsumerState<PanelHome>
    with WindowListener, TrayListener {
  var _index = 0;

  /// Shown once, the first time the window hides rather than closes.
  ///
  /// Without it the app appears to have quit while still running, which reads as a bug.
  var _explainedHideToTray = false;

  @override
  void initState() {
    super.initState();
    windowManager.addListener(this);
    trayManager.addListener(this);

    WidgetsBinding.instance.addPostFrameCallback((_) {
      ref.read(adminSessionProvider.notifier).connect();
      _refreshTray();
    });
  }

  @override
  void dispose() {
    windowManager.removeListener(this);
    trayManager.removeListener(this);
    super.dispose();
  }

  // --- window ------------------------------------------------------------

  @override
  void onWindowClose() {
    // Hide rather than exit. The panel is not load-bearing — the deck keeps working with this
    // window shut — but quitting it entirely on a stray close is still surprising.
    // `docs/SERVER.md` §7.
    windowManager.hide();
    if (!_explainedHideToTray) {
      _explainedHideToTray = true;
      // A tray balloon is platform-specific; the tooltip carries the same information and works
      // everywhere, so the first hide simply updates it.
      _refreshTray();
    }
  }

  // --- tray --------------------------------------------------------------

  @override
  void onTrayIconMouseDown() => windowManager.show();

  @override
  void onTrayIconRightMouseDown() => trayManager.popUpContextMenu();

  @override
  void onTrayMenuItemClick(MenuItem item) {
    switch (item.key) {
      case 'show':
        windowManager.show();
        windowManager.focus();
      case 'pair':
        windowManager.show();
        windowManager.focus();
        showDialog<void>(context: context, builder: (_) => const PairDialog());
      case 'quit':
        // Quits the panel only. The engine keeps running and the deck keeps working — this is
        // the single most important behaviour in the tray menu, because a user who quits here
        // expecting to close a window must not silently kill their deck.
        // `docs/SERVER.md` §7.
        windowManager.destroy();
    }
  }

  Future<void> _refreshTray() async {
    final state = ref.read(adminSessionProvider);
    final status = switch (state) {
      AdminReady(:final devices) when devices.any((d) => d.connected) =>
        'a device is connected',
      AdminReady() => 'running, no devices connected',
      AdminConnecting() => 'starting…',
      _ => 'not running',
    };

    await trayManager.setToolTip('MuxDeck — $status');
    await trayManager.setContextMenu(
      Menu(
        items: [
          MenuItem(key: 'show', label: 'Open MuxDeck'),
          MenuItem(key: 'pair', label: 'Pair a device'),
          MenuItem.separator(),
          // Deliberately labelled to say what it does *not* do. "Quit" alone reads as "stop
          // MuxDeck", and stopping the engine is a separate, explicit action.
          MenuItem(key: 'quit', label: 'Quit panel (deck keeps working)'),
        ],
      ),
    );
  }

  // --- ui ----------------------------------------------------------------

  @override
  Widget build(BuildContext context) {
    // Keep the tray text in step with the connection without polling.
    ref.listen(adminSessionProvider, (_, _) => _refreshTray());

    return Scaffold(
      body: Row(
        children: [
          NavigationRail(
            selectedIndex: _index,
            onDestinationSelected: (index) => setState(() => _index = index),
            labelType: NavigationRailLabelType.all,
            leading: const Padding(
              padding: EdgeInsets.symmetric(vertical: 16),
              child: Icon(Icons.dashboard_customize, size: 28),
            ),
            destinations: const [
              NavigationRailDestination(
                icon: Icon(Icons.speed_outlined),
                selectedIcon: Icon(Icons.speed),
                label: Text('Status'),
              ),
              NavigationRailDestination(
                icon: Icon(Icons.devices_outlined),
                selectedIcon: Icon(Icons.devices),
                label: Text('Devices'),
              ),
              NavigationRailDestination(
                icon: Icon(Icons.dashboard_outlined),
                selectedIcon: Icon(Icons.dashboard),
                label: Text('Layout'),
              ),
              NavigationRailDestination(
                icon: Icon(Icons.terminal_outlined),
                selectedIcon: Icon(Icons.terminal),
                label: Text('Actions'),
              ),
              NavigationRailDestination(
                icon: Icon(Icons.settings_outlined),
                selectedIcon: Icon(Icons.settings),
                label: Text('Settings'),
              ),
            ],
          ),
          const VerticalDivider(width: 1),
          Expanded(
            child: switch (_index) {
              1 => const DevicesPage(),
              2 => const EditorPage(),
              3 => const ActionsPage(),
              4 => const SettingsPage(),
              _ => const DashboardPage(),
            },
          ),
        ],
      ),
    );
  }
}
