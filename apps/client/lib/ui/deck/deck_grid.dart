/// The button grid.
library;

import 'package:flutter/material.dart';
import 'package:muxdeck_protocol/muxdeck_protocol.dart';

import 'deck_button.dart';

/// A fixed grid of deck keys.
///
/// **Never scrolls.** Dimensions come from the profile, not from the screen, and the buttons
/// scale to fit whatever space there is — a deck you have to scroll is not a deck
/// (`docs/CLIENT.md` §6).
class DeckGrid extends StatelessWidget {
  const DeckGrid({
    required this.columns,
    required this.rows,
    required this.buttons,
    required this.onPressed,
    this.isEnabled,
    super.key,
  });

  final int columns;
  final int rows;

  /// Sparse and unordered: each button carries its own position, and a cell with no button is
  /// empty (`docs/PROTOCOL.md` §6).
  final List<Button> buttons;

  final void Function(Button button) onPressed;

  /// Whether the host can currently perform a button's action. Null means everything is
  /// available.
  final bool Function(Button button)? isEnabled;

  @override
  Widget build(BuildContext context) {
    // Indexed by position so the grid can be laid out row by row without searching the list for
    // every cell.
    final byPosition = <String, Button>{
      for (final button in buttons)
        '${button.pos.col},${button.pos.row}': button,
    };

    return LayoutBuilder(
      builder: (context, constraints) {
        const gap = 8.0;
        const padding = 12.0;

        // Sizing is computed rather than delegated to GridView because the grid must fill the
        // space exactly and never overflow into a scroll.
        final availableWidth =
            constraints.maxWidth - padding * 2 - gap * (columns - 1);
        final availableHeight =
            constraints.maxHeight - padding * 2 - gap * (rows - 1);
        final cellWidth = availableWidth / columns;
        final cellHeight = availableHeight / rows;

        return Padding(
          padding: const EdgeInsets.all(padding),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              for (var row = 0; row < rows; row++) ...[
                if (row > 0) const SizedBox(height: gap),
                Row(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    for (var column = 0; column < columns; column++) ...[
                      if (column > 0) const SizedBox(width: gap),
                      SizedBox(
                        width: cellWidth,
                        height: cellHeight,
                        child: _cell(byPosition['$column,$row']),
                      ),
                    ],
                  ],
                ),
              ],
            ],
          ),
        );
      },
    );
  }

  Widget _cell(Button? button) {
    if (button == null) return const SizedBox.shrink();

    return DeckButton(
      button: button,
      enabled: isEnabled?.call(button) ?? true,
      onPressed: () => onPressed(button),
    );
  }
}
