/// The deck — the main screen.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:muxdeck_protocol/muxdeck_protocol.dart';

import '../../providers.dart';
import '../common/status_chip.dart';
import 'deck_button.dart';
import 'deck_grid.dart';

/// The default 3×5 layout.
///
/// Hardcoded this milestone. M6 replaces it with `profile.get` and a live-updating layout; the
/// grid widget already takes its dimensions as parameters so that change touches this list and
/// nothing else.
const defaultDeckColumns = 5;
const defaultDeckRows = 3;

const defaultDeckActions = <DeckAction?>[
  DeckAction(label: 'Copy', icon: Icons.content_copy, keys: ['CONTROL', 'C']),
  DeckAction(label: 'Paste', icon: Icons.content_paste, keys: ['CONTROL', 'V']),
  DeckAction(label: 'Cut', icon: Icons.content_cut, keys: ['CONTROL', 'X']),
  DeckAction(label: 'Undo', icon: Icons.undo, keys: ['CONTROL', 'Z']),
  DeckAction(label: 'Redo', icon: Icons.redo, keys: ['CONTROL', 'Y']),

  DeckAction(label: 'Select all', icon: Icons.select_all, keys: ['CONTROL', 'A']),
  DeckAction(label: 'Save', icon: Icons.save, keys: ['CONTROL', 'S']),
  DeckAction(label: 'Find', icon: Icons.search, keys: ['CONTROL', 'F']),
  DeckAction(
    label: 'Switch app',
    icon: Icons.swap_horiz,
    keys: ['ALT', 'TAB'],
    colour: Color(0xFF6B4FBB),
  ),
  DeckAction(
    label: 'Desktop',
    icon: Icons.desktop_windows,
    keys: ['META', 'D'],
    colour: Color(0xFF6B4FBB),
  ),

  DeckAction(
    label: 'Screenshot',
    icon: Icons.photo_camera,
    keys: ['META', 'SHIFT', 'S'],
    colour: Color(0xFF1F8A70),
  ),
  DeckAction(
    label: 'Lock',
    icon: Icons.lock,
    keys: ['META', 'L'],
    colour: Color(0xFF1F8A70),
  ),
  DeckAction(
    label: 'Task view',
    icon: Icons.grid_view,
    keys: ['META', 'TAB'],
    colour: Color(0xFF1F8A70),
  ),
  DeckAction(
    label: 'Close',
    icon: Icons.close,
    keys: ['ALT', 'F4'],
    colour: Color(0xFFB3422F),
  ),
  DeckAction(
    label: 'Escape',
    icon: Icons.keyboard_return,
    keys: ['ESCAPE'],
    colour: Color(0xFF4A5568),
  ),
];

class DeckPage extends ConsumerWidget {
  const DeckPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final session = ref.watch(sessionProvider);

    return Scaffold(
      backgroundColor: const Color(0xFF12141A),
      body: SafeArea(
        child: Column(
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(12, 8, 12, 0),
              child: Row(
                children: [
                  Expanded(child: StatusChip(state: session)),
                  IconButton(
                    icon: const Icon(Icons.settings, color: Colors.white70),
                    tooltip: 'Hosts',
                    onPressed: () => Navigator.of(context).pop(),
                  ),
                ],
              ),
            ),
            Expanded(
              child: DeckGrid(
                columns: defaultDeckColumns,
                rows: defaultDeckRows,
                actions: defaultDeckActions,
                isEnabled: (_) => session.isReady,
                onPressed: (action) => _press(ref, action),
              ),
            ),
          ],
        ),
      ),
    );
  }

  /// Sends a press.
  ///
  /// Fire-and-forget by design: a press that cannot be sent is **dropped, never queued**.
  /// Replaying `CONTROL+W` five seconds after the user pressed it is worse than losing it
  /// (`docs/CLIENT.md` §7).
  void _press(WidgetRef ref, DeckAction action) {
    final client = ref.read(sessionProvider.notifier).client;
    if (client == null) return;

    client.fireAndForget(KnownOp.inputKeyCombo, {'keys': action.keys});
  }
}
