/// Turning real keypresses into the protocol's canonical key names.
///
/// `docs/SERVER.md` §9 singles this out: *"this one needs real coverage; layout-dependent bugs
/// live here"*. The mapping is a single table, tested directly, rather than logic scattered
/// through the widget — so a wrong entry is a failing test rather than a button that sends the
/// wrong key.
library;

import 'package:flutter/services.dart';

/// The canonical name for a pressed key, or null if the protocol has no name for it.
///
/// Null is a real answer, not a failure: a keyboard has keys the protocol does not model, and
/// the editor offers a searchable list alongside capture so an unmappable key is never a dead
/// end.
String? canonicalKeyName(LogicalKeyboardKey key) => _byKey[key];

/// True for the four keys the protocol treats as modifiers. `docs/PROTOCOL.md` §5.
bool isModifierName(String name) =>
    const {'CONTROL', 'SHIFT', 'ALT', 'META'}.contains(name);

/// Orders a captured combo the way the protocol expects: modifiers first, then the key.
///
/// `input.key_combo` presses in listed order and releases in reverse, so `["C", "CONTROL"]`
/// would tap C *before* holding Control — the shortcut would not fire. Sorting here means the
/// editor cannot store a combo that looks right and behaves wrong.
List<String> orderCombo(Iterable<String> names) {
  // Order among the modifiers themselves is cosmetic — they are all held before the final key
  // whichever way round they go — so this follows how shortcuts are conventionally written:
  // Win+Shift+S, Ctrl+Shift+Esc, Ctrl+Alt+Del, Alt+Tab. A stored combo then reads the way the
  // user would say it out loud.
  const order = ['META', 'CONTROL', 'SHIFT', 'ALT'];
  final modifiers = names.where(isModifierName).toList()
    ..sort((a, b) => order.indexOf(a).compareTo(order.indexOf(b)));
  final rest = names.where((n) => !isModifierName(n)).toList();
  return [...modifiers, ...rest];
}

/// Every key the protocol names, keyed by what Flutter reports.
///
/// Left and right modifiers both map to the single canonical name: the protocol has one
/// `CONTROL`, not two, and a macro deck has no reason to distinguish them.
final Map<LogicalKeyboardKey, String> _byKey = {
  // Modifiers
  LogicalKeyboardKey.control: 'CONTROL',
  LogicalKeyboardKey.controlLeft: 'CONTROL',
  LogicalKeyboardKey.controlRight: 'CONTROL',
  LogicalKeyboardKey.shift: 'SHIFT',
  LogicalKeyboardKey.shiftLeft: 'SHIFT',
  LogicalKeyboardKey.shiftRight: 'SHIFT',
  LogicalKeyboardKey.alt: 'ALT',
  LogicalKeyboardKey.altLeft: 'ALT',
  LogicalKeyboardKey.altRight: 'ALT',
  LogicalKeyboardKey.meta: 'META',
  LogicalKeyboardKey.metaLeft: 'META',
  LogicalKeyboardKey.metaRight: 'META',

  // Letters
  LogicalKeyboardKey.keyA: 'A',
  LogicalKeyboardKey.keyB: 'B',
  LogicalKeyboardKey.keyC: 'C',
  LogicalKeyboardKey.keyD: 'D',
  LogicalKeyboardKey.keyE: 'E',
  LogicalKeyboardKey.keyF: 'F',
  LogicalKeyboardKey.keyG: 'G',
  LogicalKeyboardKey.keyH: 'H',
  LogicalKeyboardKey.keyI: 'I',
  LogicalKeyboardKey.keyJ: 'J',
  LogicalKeyboardKey.keyK: 'K',
  LogicalKeyboardKey.keyL: 'L',
  LogicalKeyboardKey.keyM: 'M',
  LogicalKeyboardKey.keyN: 'N',
  LogicalKeyboardKey.keyO: 'O',
  LogicalKeyboardKey.keyP: 'P',
  LogicalKeyboardKey.keyQ: 'Q',
  LogicalKeyboardKey.keyR: 'R',
  LogicalKeyboardKey.keyS: 'S',
  LogicalKeyboardKey.keyT: 'T',
  LogicalKeyboardKey.keyU: 'U',
  LogicalKeyboardKey.keyV: 'V',
  LogicalKeyboardKey.keyW: 'W',
  LogicalKeyboardKey.keyX: 'X',
  LogicalKeyboardKey.keyY: 'Y',
  LogicalKeyboardKey.keyZ: 'Z',

  // Digits — the number row, not the numpad.
  LogicalKeyboardKey.digit0: 'DIGIT0',
  LogicalKeyboardKey.digit1: 'DIGIT1',
  LogicalKeyboardKey.digit2: 'DIGIT2',
  LogicalKeyboardKey.digit3: 'DIGIT3',
  LogicalKeyboardKey.digit4: 'DIGIT4',
  LogicalKeyboardKey.digit5: 'DIGIT5',
  LogicalKeyboardKey.digit6: 'DIGIT6',
  LogicalKeyboardKey.digit7: 'DIGIT7',
  LogicalKeyboardKey.digit8: 'DIGIT8',
  LogicalKeyboardKey.digit9: 'DIGIT9',

  // Function
  LogicalKeyboardKey.f1: 'F1',
  LogicalKeyboardKey.f2: 'F2',
  LogicalKeyboardKey.f3: 'F3',
  LogicalKeyboardKey.f4: 'F4',
  LogicalKeyboardKey.f5: 'F5',
  LogicalKeyboardKey.f6: 'F6',
  LogicalKeyboardKey.f7: 'F7',
  LogicalKeyboardKey.f8: 'F8',
  LogicalKeyboardKey.f9: 'F9',
  LogicalKeyboardKey.f10: 'F10',
  LogicalKeyboardKey.f11: 'F11',
  LogicalKeyboardKey.f12: 'F12',
  LogicalKeyboardKey.f13: 'F13',
  LogicalKeyboardKey.f14: 'F14',
  LogicalKeyboardKey.f15: 'F15',
  LogicalKeyboardKey.f16: 'F16',
  LogicalKeyboardKey.f17: 'F17',
  LogicalKeyboardKey.f18: 'F18',
  LogicalKeyboardKey.f19: 'F19',
  LogicalKeyboardKey.f20: 'F20',
  LogicalKeyboardKey.f21: 'F21',
  LogicalKeyboardKey.f22: 'F22',
  LogicalKeyboardKey.f23: 'F23',
  LogicalKeyboardKey.f24: 'F24',

  // Navigation
  LogicalKeyboardKey.escape: 'ESCAPE',
  LogicalKeyboardKey.tab: 'TAB',
  LogicalKeyboardKey.capsLock: 'CAPSLOCK',
  LogicalKeyboardKey.space: 'SPACE',
  LogicalKeyboardKey.enter: 'ENTER',
  LogicalKeyboardKey.backspace: 'BACKSPACE',
  LogicalKeyboardKey.delete: 'DELETE',
  LogicalKeyboardKey.insert: 'INSERT',
  LogicalKeyboardKey.home: 'HOME',
  LogicalKeyboardKey.end: 'END',
  LogicalKeyboardKey.pageUp: 'PAGEUP',
  LogicalKeyboardKey.pageDown: 'PAGEDOWN',
  LogicalKeyboardKey.arrowLeft: 'LEFT',
  LogicalKeyboardKey.arrowRight: 'RIGHT',
  LogicalKeyboardKey.arrowUp: 'UP',
  LogicalKeyboardKey.arrowDown: 'DOWN',

  // Numpad
  LogicalKeyboardKey.numpad0: 'NUMPAD0',
  LogicalKeyboardKey.numpad1: 'NUMPAD1',
  LogicalKeyboardKey.numpad2: 'NUMPAD2',
  LogicalKeyboardKey.numpad3: 'NUMPAD3',
  LogicalKeyboardKey.numpad4: 'NUMPAD4',
  LogicalKeyboardKey.numpad5: 'NUMPAD5',
  LogicalKeyboardKey.numpad6: 'NUMPAD6',
  LogicalKeyboardKey.numpad7: 'NUMPAD7',
  LogicalKeyboardKey.numpad8: 'NUMPAD8',
  LogicalKeyboardKey.numpad9: 'NUMPAD9',
  LogicalKeyboardKey.numpadAdd: 'NUMPAD_ADD',
  LogicalKeyboardKey.numpadSubtract: 'NUMPAD_SUB',
  LogicalKeyboardKey.numpadMultiply: 'NUMPAD_MUL',
  LogicalKeyboardKey.numpadDivide: 'NUMPAD_DIV',
  LogicalKeyboardKey.numpadDecimal: 'NUMPAD_DECIMAL',
  LogicalKeyboardKey.numpadEnter: 'NUMPAD_ENTER',

  // Symbols
  LogicalKeyboardKey.minus: 'MINUS',
  LogicalKeyboardKey.equal: 'EQUAL',
  LogicalKeyboardKey.bracketLeft: 'BRACKET_LEFT',
  LogicalKeyboardKey.bracketRight: 'BRACKET_RIGHT',
  LogicalKeyboardKey.backslash: 'BACKSLASH',
  LogicalKeyboardKey.semicolon: 'SEMICOLON',
  LogicalKeyboardKey.quoteSingle: 'QUOTE',
  LogicalKeyboardKey.backquote: 'BACKQUOTE',
  LogicalKeyboardKey.comma: 'COMMA',
  LogicalKeyboardKey.period: 'PERIOD',
  LogicalKeyboardKey.slash: 'SLASH',

  // System
  LogicalKeyboardKey.printScreen: 'PRINTSCREEN',
  LogicalKeyboardKey.scrollLock: 'SCROLLLOCK',
  LogicalKeyboardKey.pause: 'PAUSE',
  LogicalKeyboardKey.numLock: 'NUMLOCK',
  LogicalKeyboardKey.contextMenu: 'MENU',
};

/// Every canonical name, for the searchable fallback list. `docs/PROTOCOL.md` §5.
final List<String> allKeyNames = _byKey.values.toSet().toList()..sort();
