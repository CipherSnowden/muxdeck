/// The deck grid: layout at several sizes, and press behaviour.
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:muxdeck_client/ui/deck/deck_button.dart';
import 'package:muxdeck_client/ui/deck/deck_grid.dart';

/// Fills a grid so every cell is occupied.
List<DeckAction?> _actions(int count) => List.generate(
  count,
  (i) =>
      DeckAction(label: 'Key $i', icon: Icons.circle, keys: ['DIGIT${i % 10}']),
);

/// Wraps the grid in a fixed-size surface, standing in for a device screen.
Widget _harness({
  required int columns,
  required int rows,
  required Size screen,
  List<DeckAction?>? actions,
  void Function(DeckAction)? onPressed,
  bool Function(DeckAction)? isEnabled,
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
            actions: actions ?? _actions(columns * rows),
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

    testWidgets('empty cells leave a gap rather than shifting the layout', (
      tester,
    ) async {
      await tester.binding.setSurfaceSize(phone);
      addTearDown(() => tester.binding.setSurfaceSize(null));

      final sparse = <DeckAction?>[
        const DeckAction(label: 'One', icon: Icons.looks_one, keys: ['DIGIT1']),
        null,
        const DeckAction(label: 'Three', icon: Icons.looks_3, keys: ['DIGIT3']),
      ];

      await tester.pumpWidget(
        _harness(columns: 3, rows: 1, screen: phone, actions: sparse),
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
          onPressed: (action) => pressed.add(action.label),
        ),
      );

      final gesture = await tester.startGesture(
        tester.getCenter(find.byType(DeckButton).first),
      );
      await tester.pump();

      expect(pressed, [
        'Key 0',
      ], reason: 'the action must fire before the finger lifts');

      await gesture.up();
      await tester.pumpAndSettle();

      expect(pressed, [
        'Key 0',
      ], reason: 'releasing must not fire a second time');
    });

    testWidgets('sends the canonical key names for the button', (tester) async {
      await tester.binding.setSurfaceSize(phone);
      addTearDown(() => tester.binding.setSurfaceSize(null));

      List<String>? sent;
      await tester.pumpWidget(
        _harness(
          columns: 1,
          rows: 1,
          screen: phone,
          actions: const [
            DeckAction(label: 'Copy', icon: Icons.copy, keys: ['CONTROL', 'C']),
          ],
          onPressed: (action) => sent = action.keys,
        ),
      );

      await tester.tap(find.byType(DeckButton));
      await tester.pump();

      expect(sent, ['CONTROL', 'C']);
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
  });
}
