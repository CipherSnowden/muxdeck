/// The deck grid: layout at several sizes, and press behaviour.
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:muxdeck_client/ui/deck/deck_button.dart';
import 'package:muxdeck_client/ui/deck/deck_grid.dart';
import 'package:muxdeck_protocol/muxdeck_protocol.dart';

/// A button at a position, of the shape the engine actually sends.
Button _button({
  required int col,
  required int row,
  String? label,
  String icon = 'circle',
  List<String> keys = const ['A'],
  KnownOp op = KnownOp.inputKeyCombo,
}) => Button(
  id: 'b_${col}_$row',
  pos: Position(col: col, row: row),
  label: label ?? 'Key $col$row',
  icon: icon,
  color: '#2D6CDF',
  haptic: Haptic.light,
  onTap: ButtonAction(Op.of(op), {'keys': keys}),
);

/// Fills every cell of a grid.
List<Button> _fill(int columns, int rows) => [
  for (var row = 0; row < rows; row++)
    for (var column = 0; column < columns; column++)
      _button(col: column, row: row),
];

/// Wraps the grid in a fixed-size surface, standing in for a device screen.
Widget _harness({
  required int columns,
  required int rows,
  required Size screen,
  List<Button>? buttons,
  void Function(Button)? onPressed,
  bool Function(Button)? isEnabled,
}) {
  return MaterialApp(
    home: MediaQuery(
      data: MediaQueryData(size: screen),
      child: Scaffold(
        body: SizedBox(
          width: screen.width,
          height: screen.height,
          child: DeckGrid(
            columns: columns,
            rows: rows,
            buttons: buttons ?? _fill(columns, rows),
            isEnabled: isEnabled,
            onPressed: onPressed ?? (_) {},
          ),
        ),
      ),
    ),
  );
}

void main() {
  // Landscape, because the deck is landscape-first.
  const phone = Size(844, 390); // iPhone-ish
  const tablet = Size(1180, 820); // iPad-ish

  group('layout', () {
    for (final (columns, rows) in const [(5, 3), (6, 4), (8, 5)]) {
      for (final (name, screen) in const [
        ('phone', phone),
        ('tablet', tablet),
      ]) {
        testWidgets('$columns×$rows renders every button on $name', (
          tester,
        ) async {
          await tester.binding.setSurfaceSize(screen);
          addTearDown(() => tester.binding.setSurfaceSize(null));

          await tester.pumpWidget(
            _harness(columns: columns, rows: rows, screen: screen),
          );

          expect(find.byType(DeckButton), findsNWidgets(columns * rows));
          // A grid that overflows would paint a yellow-and-black stripe and fail the frame.
          expect(tester.takeException(), isNull);
        });
      }
    }

    testWidgets('the grid never scrolls', (tester) async {
      // "A deck you have to scroll is not a deck" — docs/CLIENT.md §6. Buttons shrink to fit
      // instead, which is why DeckGrid computes cell sizes rather than using GridView.
      await tester.binding.setSurfaceSize(phone);
      addTearDown(() => tester.binding.setSurfaceSize(null));

      await tester.pumpWidget(_harness(columns: 8, rows: 5, screen: phone));

      expect(find.byType(Scrollable), findsNothing);
    });

    testWidgets('buttons land in their own cells regardless of list order', (
      tester,
    ) async {
      // Buttons are sparse and unordered — each carries its own position, so the grid must
      // place by `pos` rather than by index. docs/PROTOCOL.md §6.
      await tester.binding.setSurfaceSize(phone);
      addTearDown(() => tester.binding.setSurfaceSize(null));

      await tester.pumpWidget(
        _harness(
          columns: 3,
          rows: 1,
          screen: phone,
          buttons: [
            _button(col: 2, row: 0, label: 'Third'),
            _button(col: 0, row: 0, label: 'First'),
          ],
        ),
      );

      expect(find.byType(DeckButton), findsNWidgets(2));
      final first = tester.getCenter(find.text('First'));
      final third = tester.getCenter(find.text('Third'));
      expect(
        first.dx,
        lessThan(third.dx),
        reason: 'the button at col 0 must render left of the one at col 2',
      );
    });

    testWidgets('an empty cell leaves a gap rather than shifting the layout', (
      tester,
    ) async {
      await tester.binding.setSurfaceSize(phone);
      addTearDown(() => tester.binding.setSurfaceSize(null));

      await tester.pumpWidget(
        _harness(
          columns: 3,
          rows: 1,
          screen: phone,
          buttons: [
            _button(col: 0, row: 0, label: 'One'),
            _button(col: 2, row: 0, label: 'Three'),
          ],
        ),
      );

      expect(find.byType(DeckButton), findsNWidgets(2));
      expect(find.text('One'), findsOneWidget);
      expect(find.text('Three'), findsOneWidget);
    });
  });

  group('press', () {
    testWidgets('fires on pointer down, not on release', (tester) async {
      // The whole difference between feeling like hardware and feeling like a web page.
      // docs/CLIENT.md §6.
      await tester.binding.setSurfaceSize(phone);
      addTearDown(() => tester.binding.setSurfaceSize(null));

      final pressed = <String>[];
      await tester.pumpWidget(
        _harness(
          columns: 2,
          rows: 1,
          screen: phone,
          onPressed: (button) => pressed.add(button.label),
        ),
      );

      final gesture = await tester.startGesture(
        tester.getCenter(find.byType(DeckButton).first),
      );
      await tester.pump();

      expect(pressed, [
        'Key 00',
      ], reason: 'the action must fire before the finger lifts');

      await gesture.up();
      await tester.pumpAndSettle();

      expect(pressed, [
        'Key 00',
      ], reason: 'releasing must not fire a second time');
    });

    testWidgets('carries the button action through unchanged', (tester) async {
      await tester.binding.setSurfaceSize(phone);
      addTearDown(() => tester.binding.setSurfaceSize(null));

      Button? sent;
      await tester.pumpWidget(
        _harness(
          columns: 1,
          rows: 1,
          screen: phone,
          buttons: [
            _button(col: 0, row: 0, label: 'Copy', keys: ['CONTROL', 'C']),
          ],
          onPressed: (button) => sent = button,
        ),
      );

      await tester.tap(find.byType(DeckButton));
      await tester.pump();

      expect(sent?.onTap?.op.known, KnownOp.inputKeyCombo);
      expect(sent?.onTap?.d['keys'], ['CONTROL', 'C']);
    });

    testWidgets('a disabled button does not fire', (tester) async {
      // Capabilities from the Ready payload grey out actions the host cannot perform, so they
      // are visibly unavailable rather than failing at press time. docs/PROTOCOL.md §4.1.
      await tester.binding.setSurfaceSize(phone);
      addTearDown(() => tester.binding.setSurfaceSize(null));

      var fired = 0;
      await tester.pumpWidget(
        _harness(
          columns: 1,
          rows: 1,
          screen: phone,
          onPressed: (_) => fired++,
          isEnabled: (_) => false,
        ),
      );

      await tester.tap(find.byType(DeckButton));
      await tester.pump();

      expect(fired, 0);
    });

    testWidgets('an unknown icon renders the fallback rather than a blank', (
      tester,
    ) async {
      await tester.binding.setSurfaceSize(phone);
      addTearDown(() => tester.binding.setSurfaceSize(null));

      await tester.pumpWidget(
        _harness(
          columns: 1,
          rows: 1,
          screen: phone,
          buttons: [_button(col: 0, row: 0, icon: 'no_such_icon')],
        ),
      );

      expect(find.byIcon(Icons.circle), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  });
}
