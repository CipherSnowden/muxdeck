/// The layout editor — the reason this app exists.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:muxdeck_icons/muxdeck_icons.dart';
import 'package:muxdeck_protocol/muxdeck_protocol.dart';

import '../domain/admin_session.dart';
import '../providers.dart';
import 'button_editor.dart';

class EditorPage extends ConsumerWidget {
  const EditorPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final profile = ref.watch(editorProvider);

    if (profile == null) {
      return const Center(child: CircularProgressIndicator());
    }

    final page = profile.pages.first;
    final byPosition = <String, Button>{
      for (final button in page.buttons)
        '${button.pos.col},${button.pos.row}': button,
    };

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(24, 24, 24, 8),
          child: Row(
            children: [
              Text(profile.name, style: Theme.of(context).textTheme.titleLarge),
              const SizedBox(width: 12),
              Text(
                '${profile.grid.cols} × ${profile.grid.rows}',
                style: Theme.of(context).textTheme.bodySmall,
              ),
              const Spacer(),
              const Text('Changes appear on your deck immediately'),
            ],
          ),
        ),
        Expanded(
          child: Padding(
            padding: const EdgeInsets.all(24),
            child: LayoutBuilder(
              builder: (context, constraints) {
                const gap = 10.0;
                // The editor grid mirrors the deck's own proportions so what is designed here
                // is what appears there. `docs/SERVER.md` §8.
                final cell =
                    ((constraints.maxWidth - gap * (profile.grid.cols - 1)) /
                            profile.grid.cols)
                        .clamp(60.0, 150.0);

                return SingleChildScrollView(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      for (var row = 0; row < profile.grid.rows; row++)
                        Padding(
                          padding: const EdgeInsets.only(bottom: gap),
                          child: Row(
                            children: [
                              for (var col = 0; col < profile.grid.cols; col++)
                                Padding(
                                  padding: const EdgeInsets.only(right: gap),
                                  child: _Cell(
                                    size: cell,
                                    button: byPosition['$col,$row'],
                                    onTap: () => _edit(
                                      context,
                                      ref,
                                      profile,
                                      byPosition['$col,$row'],
                                      col,
                                      row,
                                    ),
                                  ),
                                ),
                            ],
                          ),
                        ),
                    ],
                  ),
                );
              },
            ),
          ),
        ),
      ],
    );
  }

  Future<void> _edit(
    BuildContext context,
    WidgetRef ref,
    Profile profile,
    Button? existing,
    int col,
    int row,
  ) async {
    final result = await showDialog<ButtonEditResult>(
      context: context,
      builder: (_) => ButtonEditorDialog(
        button: existing,
        col: col,
        row: row,
        shellActionsEnabled: _shellActionsEnabled(ref),
      ),
    );
    if (result == null || !context.mounted) return;

    try {
      if (result.deleted) {
        if (existing != null) {
          await ref.read(editorProvider.notifier).clearCell(existing.id);
        }
      } else {
        await ref.read(editorProvider.notifier).saveButton(result.button!);
      }
    } catch (e) {
      if (context.mounted) {
        // The engine rejects with a message naming the exact rule broken, so it is shown
        // verbatim rather than replaced with something vaguer.
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text('$e')));
      }
    }
  }

  bool _shellActionsEnabled(WidgetRef ref) {
    final session = ref.read(adminSessionProvider);
    return session is AdminReady && session.ready.capabilities.shellActions;
  }
}

class _Cell extends StatelessWidget {
  const _Cell({required this.size, required this.button, required this.onTap});

  final double size;
  final Button? button;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final button = this.button;

    return SizedBox(
      width: size,
      height: size,
      child: Material(
        color: button == null
            ? Theme.of(context).colorScheme.surfaceContainerHighest
            : _colourOf(button),
        borderRadius: BorderRadius.circular(12),
        child: InkWell(
          borderRadius: BorderRadius.circular(12),
          onTap: onTap,
          child: button == null
              ? Icon(Icons.add, color: Theme.of(context).colorScheme.outline)
              : Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    Icon(
                      iconFor(button.icon),
                      color: Colors.white,
                      size: size * 0.3,
                    ),
                    const SizedBox(height: 4),
                    Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 4),
                      child: Text(
                        button.label,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        textAlign: TextAlign.center,
                        style: const TextStyle(
                          color: Colors.white,
                          fontSize: 11,
                        ),
                      ),
                    ),
                  ],
                ),
        ),
      ),
    );
  }

  Color _colourOf(Button button) {
    final hex = button.color.replaceFirst('#', '');
    final value = int.tryParse(hex, radix: 16);
    if (value == null || hex.length != 6) return const Color(0xFF2D6CDF);
    return Color(0xFF000000 | value);
  }
}
