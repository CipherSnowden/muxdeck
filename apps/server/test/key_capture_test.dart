/// Key capture: the layout-dependent part `docs/SERVER.md` §9 asks for real coverage on.
library;

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:muxdeck_server/domain/key_capture.dart';

void main() {
  group('canonicalKeyName', () {
    test('letters and digits map to their protocol names', () {
      expect(canonicalKeyName(LogicalKeyboardKey.keyC), 'C');
      expect(canonicalKeyName(LogicalKeyboardKey.keyZ), 'Z');
      // The number row is DIGIT0..9; the numpad is NUMPAD0..9. Conflating them would send the
      // wrong key to anything that tells them apart.
      expect(canonicalKeyName(LogicalKeyboardKey.digit1), 'DIGIT1');
      expect(canonicalKeyName(LogicalKeyboardKey.numpad1), 'NUMPAD1');
    });

    test('left and right modifiers collapse to one name', () {
      // The protocol has one CONTROL, not two. A deck has no reason to distinguish them, and
      // producing 'CONTROL_LEFT' would be a name the engine does not know.
      for (final key in [
        LogicalKeyboardKey.control,
        LogicalKeyboardKey.controlLeft,
        LogicalKeyboardKey.controlRight,
      ]) {
        expect(canonicalKeyName(key), 'CONTROL');
      }
      expect(canonicalKeyName(LogicalKeyboardKey.metaRight), 'META');
    });

    test('navigation keys use the protocol spelling, not Flutter\'s', () {
      // PAGEUP, not PAGE_UP; LEFT, not ARROW_LEFT. These are the exact strings docs/PROTOCOL.md
      // §5 defines, and the engine rejects anything else.
      expect(canonicalKeyName(LogicalKeyboardKey.pageUp), 'PAGEUP');
      expect(canonicalKeyName(LogicalKeyboardKey.pageDown), 'PAGEDOWN');
      expect(canonicalKeyName(LogicalKeyboardKey.arrowLeft), 'LEFT');
      expect(canonicalKeyName(LogicalKeyboardKey.arrowDown), 'DOWN');
      expect(canonicalKeyName(LogicalKeyboardKey.capsLock), 'CAPSLOCK');
      expect(canonicalKeyName(LogicalKeyboardKey.contextMenu), 'MENU');
    });

    test('symbols use the protocol spelling', () {
      expect(canonicalKeyName(LogicalKeyboardKey.bracketLeft), 'BRACKET_LEFT');
      expect(canonicalKeyName(LogicalKeyboardKey.backquote), 'BACKQUOTE');
      expect(canonicalKeyName(LogicalKeyboardKey.quoteSingle), 'QUOTE');
      expect(canonicalKeyName(LogicalKeyboardKey.numpadSubtract), 'NUMPAD_SUB');
    });

    test('a key the protocol does not model returns null', () {
      // Null is a real answer, not a failure — the editor falls back to a searchable list so an
      // unmappable key is never a dead end.
      expect(canonicalKeyName(LogicalKeyboardKey.browserBack), isNull);
      expect(canonicalKeyName(LogicalKeyboardKey.mediaPlay), isNull);
      // Media keys in particular: they reach the deck through `input.media`, not as key names,
      // so capturing one as a combo would be wrong rather than merely unsupported.
      expect(canonicalKeyName(LogicalKeyboardKey.audioVolumeUp), isNull);
    });

    test('every produced name is one the protocol defines', () {
      // Guards against a typo in the table producing a name the engine will reject at press
      // time — which would look like a broken button, not a broken editor.
      const canonical = {
        'CONTROL',
        'SHIFT',
        'ALT',
        'META',
        'A',
        'B',
        'C',
        'D',
        'E',
        'F',
        'G',
        'H',
        'I',
        'J',
        'K',
        'L',
        'M',
        'N',
        'O',
        'P',
        'Q',
        'R',
        'S',
        'T',
        'U',
        'V',
        'W',
        'X',
        'Y',
        'Z',
        'DIGIT0',
        'DIGIT1',
        'DIGIT2',
        'DIGIT3',
        'DIGIT4',
        'DIGIT5',
        'DIGIT6',
        'DIGIT7',
        'DIGIT8',
        'DIGIT9',
        'F1',
        'F2',
        'F3',
        'F4',
        'F5',
        'F6',
        'F7',
        'F8',
        'F9',
        'F10',
        'F11',
        'F12',
        'F13',
        'F14',
        'F15',
        'F16',
        'F17',
        'F18',
        'F19',
        'F20',
        'F21',
        'F22',
        'F23',
        'F24',
        'ESCAPE',
        'TAB',
        'CAPSLOCK',
        'SPACE',
        'ENTER',
        'BACKSPACE',
        'DELETE',
        'INSERT',
        'HOME',
        'END',
        'PAGEUP',
        'PAGEDOWN',
        'LEFT',
        'RIGHT',
        'UP',
        'DOWN',
        'NUMPAD0',
        'NUMPAD1',
        'NUMPAD2',
        'NUMPAD3',
        'NUMPAD4',
        'NUMPAD5',
        'NUMPAD6',
        'NUMPAD7',
        'NUMPAD8',
        'NUMPAD9',
        'NUMPAD_ADD',
        'NUMPAD_SUB',
        'NUMPAD_MUL',
        'NUMPAD_DIV',
        'NUMPAD_DECIMAL',
        'NUMPAD_ENTER',
        'MINUS',
        'EQUAL',
        'BRACKET_LEFT',
        'BRACKET_RIGHT',
        'BACKSLASH',
        'SEMICOLON',
        'QUOTE',
        'BACKQUOTE',
        'COMMA',
        'PERIOD',
        'SLASH',
        'PRINTSCREEN',
        'SCROLLLOCK',
        'PAUSE',
        'NUMLOCK',
        'MENU',
      };

      for (final name in allKeyNames) {
        expect(
          canonical.contains(name),
          isTrue,
          reason: "'$name' is not a key name docs/PROTOCOL.md §5 defines",
        );
      }
    });
  });

  group('orderCombo', () {
    test('modifiers come before the key they modify', () {
      // input.key_combo presses in listed order, so ["C", "CONTROL"] taps C *before* holding
      // Control and the shortcut never fires. The editor must not be able to store that.
      expect(orderCombo(['C', 'CONTROL']), ['CONTROL', 'C']);
      expect(orderCombo(['S', 'SHIFT', 'META']), ['META', 'SHIFT', 'S']);
    });

    test('modifiers are ordered consistently', () {
      // Same set, same output, whatever order the user happened to press them in — so two
      // identical combos do not read as different in the stored profile.
      expect(
        orderCombo(['SHIFT', 'CONTROL', 'A']),
        orderCombo(['CONTROL', 'SHIFT', 'A']),
      );
      expect(orderCombo(['ALT', 'CONTROL']), ['CONTROL', 'ALT']);
    });

    test('a lone modifier survives', () {
      // ["META"] alone is a real macro — it opens the Start menu. docs/PROTOCOL.md §4.3.
      expect(orderCombo(['META']), ['META']);
    });

    test('a lone key survives', () {
      expect(orderCombo(['ESCAPE']), ['ESCAPE']);
    });
  });

  group('isModifierName', () {
    test('exactly the four the protocol calls modifiers', () {
      for (final name in ['CONTROL', 'SHIFT', 'ALT', 'META']) {
        expect(isModifierName(name), isTrue);
      }
      for (final name in ['A', 'ESCAPE', 'F1', 'CAPSLOCK', 'NUMLOCK']) {
        expect(
          isModifierName(name),
          isFalse,
          reason: '$name is not a modifier for combo purposes',
        );
      }
    });
  });
}
