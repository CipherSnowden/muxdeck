import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:muxdeck_icons/muxdeck_icons.dart';

void main() {
  test('an unknown name falls back rather than failing', () {
    // A button that draws blank looks broken; a dot reads as "this works, its icon just did not
    // resolve". docs/PROTOCOL.md §6.
    expect(iconFor('no_such_icon_exists'), fallbackIcon);
    expect(iconFor(''), fallbackIcon);
  });

  test('known names resolve to their icon', () {
    expect(iconFor('content_copy'), Icons.content_copy);
    expect(iconFor('volume_up'), Icons.volume_up);
  });

  test('the picker is offered exactly what the deck can render', () {
    // The picker sources from iconNames and the deck renders from deckIcons. If those ever
    // diverge, the editor would offer a name that draws as a fallback dot.
    for (final name in iconNames) {
      expect(
        deckIcons.containsKey(name),
        isTrue,
        reason: '$name is offered by the picker but the deck cannot render it',
      );

      // `circle` is exempt: it is both a legitimate icon a user may choose and the shape an
      // unresolved name falls back to. Every other name resolving to the fallback would mean it
      // is missing from the map rather than deliberately chosen.
      if (name != 'circle') {
        expect(
          iconFor(name),
          isNot(fallbackIcon),
          reason:
              '$name is offered but resolves to the fallback, so it is missing',
        );
      }
    }
  });

  test('names are sorted, so the picker is browsable', () {
    final sorted = [...iconNames]..sort();
    expect(iconNames, sorted);
  });

  test('the set is large enough to be useful', () {
    // Not a precise count — this guards against the map being accidentally gutted, not against
    // it growing.
    expect(deckIcons.length, greaterThan(100));
  });

  test('the default profile\'s icons all resolve', () {
    // These names are hardcoded in the engine's default profile (store.rs). If one is renamed
    // here without being changed there, a fresh install shows fallback dots — which is exactly
    // the kind of drift a shared map is supposed to prevent.
    const usedByDefaultProfile = [
      'content_copy',
      'content_paste',
      'content_cut',
      'undo',
      'redo',
      'select_all',
      'save',
      'search',
      'swap_horiz',
      'desktop_windows',
      'photo_camera',
      'lock',
      'grid_view',
      'close',
      'keyboard_return',
    ];

    for (final name in usedByDefaultProfile) {
      expect(
        deckIcons.containsKey(name),
        isTrue,
        reason:
            "the engine's default profile uses '$name', which this map must define",
      );
    }
  });
}
