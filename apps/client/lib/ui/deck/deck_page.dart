/// The deck — the main screen.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:muxdeck_protocol/muxdeck_protocol.dart';

import '../../domain/profile/profile_controller.dart';
import '../../domain/session/session_state.dart';
import '../../providers.dart';
import '../common/status_chip.dart';
import 'deck_grid.dart';

class DeckPage extends ConsumerWidget {
  const DeckPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final session = ref.watch(sessionProvider);
    final layout = ref.watch(profileProvider);

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
            Expanded(child: _body(ref, session, layout)),
          ],
        ),
      ),
    );
  }

  Widget _body(WidgetRef ref, SessionState session, DeckLayout? layout) {
    if (layout == null) {
      // Only before the very first profile has ever been seen. After that the cache means there
      // is always something to draw.
      return const Center(child: CircularProgressIndicator());
    }

    final page = layout.profile.pages.first;
    final ready = session is SessionReady ? session.ready : null;

    return Opacity(
      // A cached layout renders immediately but dimmed, so it is visibly not yet live rather
      // than silently stale. `docs/CLIENT.md` §7.
      opacity: layout.isLive ? 1.0 : 0.45,
      child: DeckGrid(
        columns: layout.profile.grid.cols,
        rows: layout.profile.grid.rows,
        buttons: page.buttons,
        isEnabled: (button) => layout.isLive && _canPerform(button, ready),
        onPressed: (button) => _press(ref, button),
      ),
    );
  }

  /// Whether the host can actually carry out a button's action.
  ///
  /// Greying out beats letting the press fail: the `capabilities` block exists precisely so a
  /// deck can show what is unavailable instead of discovering it at press time
  /// (`docs/PROTOCOL.md` §4.1).
  bool _canPerform(Button button, Ready? ready) {
    if (ready == null) return false;
    final action = button.onTap;
    if (action == null) return false;

    return switch (action.op.known) {
      KnownOp.inputText => ready.capabilities.textUnicode,
      KnownOp.inputMedia => ready.capabilities.mediaKeys,
      KnownOp.inputMouse => ready.capabilities.mouse,
      KnownOp.actionRun => ready.capabilities.shellActions,
      // Key combos need no capability: a host that can inject at all can send them.
      _ => true,
    };
  }

  /// Sends a press.
  ///
  /// Fire-and-forget by design: a press that cannot be sent is **dropped, never queued**.
  /// Replaying `CONTROL+W` five seconds after the user pressed it is worse than losing it
  /// (`docs/CLIENT.md` §7).
  void _press(WidgetRef ref, Button button) {
    final client = ref.read(sessionProvider.notifier).client;
    final action = button.onTap;
    if (client == null || action == null) return;

    final op = action.op.known;
    if (op == null) return;

    client.fireAndForget(op, action.d);
  }
}
