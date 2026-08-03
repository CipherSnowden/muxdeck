/// The button grid.
library;

import 'package:flutter/material.dart';

import 'deck_button.dart';

/// A fixed grid of deck keys.
///
/// **Never scrolls.** Dimensions come from the layout, not from the screen, and the buttons
/// scale to fit whatever space there is — a deck you have to scroll is not a deck
/// (`docs/CLIENT.md` §6).
class DeckGrid extends StatelessWidget {
  const DeckGrid({
    required this.columns,
    required this.rows,
    required this.actions,
    required this.onPressed,
    this.isEnabled,
    super.key,
  });

  final int columns;
  final int rows;

  /// Sparse: a cell with no action is empty. Indexed row-major.
  final List<DeckAction?> actions;

  final void Function(DeckAction action) onPressed;

  /// Whether the host can currently perform an action. Null means everything is available.
  final bool Function(DeckAction action)? isEnabled;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        const gap = 8.0;
        const padding = 12.0;

        // Sizing is computed rather than delegated to GridView because the grid must fill the
        // space exactly and never overflow into a scroll.
        final availableWidth = constraints.maxWidth - padding * 2 - gap * (columns - 1);
        final availableHeight = constraints.maxHeight - padding * 2 - gap * (rows - 1);
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
                        child: _cell(row * columns + column),
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

  Widget _cell(int index) {
    final action = index < actions.length ? actions[index] : null;
    if (action == null) return const SizedBox.shrink();

    return DeckButton(
      action: action,
      enabled: isEnabled?.call(action) ?? true,
      onPressed: () => onPressed(action),
    );
  }
}
