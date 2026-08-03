/// Input injection payloads and the canonical key table. `docs/PROTOCOL.md` §4.3 and §5.
library;

import 'envelope.dart';

/// `input.key_combo`.
///
/// Modifiers are pressed in listed order, the final non-modifier key is tapped, then all
/// are released in reverse order.
class KeyCombo implements Payload {
  const KeyCombo(this.keys, {this.holdMs});

  factory KeyCombo.fromJson(Map<String, dynamic> json) => KeyCombo(
    (json['keys'] as List<dynamic>)
        .map((k) => Key.fromWire(k as String))
        .toList(),
    holdMs: json['hold_ms'] as int?,
  );

  final List<Key> keys;

  /// Holds the entire combo — every key down — before releasing in reverse order.
  ///
  /// Nullable rather than defaulted to zero so the field round-trips exactly: a
  /// `key_sequence` step that omits it must not gain one on the way back out.
  final int? holdMs;

  int get holdMsOrDefault => holdMs ?? 0;

  /// `docs/PROTOCOL.md` §4.3.
  ///
  /// Zero non-modifiers is valid — `["META"]` alone is a real macro. Two or more is almost
  /// always a mistake, and `input.key_sequence` exists for the deliberate case.
  void validate() {
    if (keys.isEmpty) {
      throw const ProtocolException(
        ErrorCode.badRequest,
        'input.key_combo requires at least one key',
      );
    }
    if (keys.where((k) => !k.isModifier).length > 1) {
      throw const ProtocolException(
        ErrorCode.badRequest,
        'input.key_combo accepts at most one non-modifier key; use input.key_sequence',
      );
    }
  }

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{
    'keys': keys.map((k) => k.wire).toList(),
    if (holdMs != null) 'hold_ms': holdMs,
  };
}

/// `input.key_sequence` — several combos in order.
class KeySequence implements Payload {
  const KeySequence(this.steps);

  factory KeySequence.fromJson(Map<String, dynamic> json) => KeySequence(
    (json['steps'] as List<dynamic>)
        .map((s) => SequenceStep.fromJson(s as Map<String, dynamic>))
        .toList(),
  );

  final List<SequenceStep> steps;

  void validate() {
    for (final step in steps) {
      if (step is ComboStep) step.combo.validate();
    }
  }

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{
    'steps': steps.map((s) => s.toJson()).toList(),
  };
}

/// One step of a sequence: either a combo or a pause.
///
/// The wire carries no discriminant here — unlike [HelloResponse], where a tag exists and
/// must be used. The two shapes are told apart by their required fields, which is safe
/// only because `keys` and `delay_ms` are each mandatory in their own variant. Do not make
/// either optional.
sealed class SequenceStep {
  const SequenceStep();

  factory SequenceStep.fromJson(Map<String, dynamic> json) {
    if (json.containsKey('keys')) return ComboStep(KeyCombo.fromJson(json));
    if (json.containsKey('delay_ms')) {
      return DelayStep(json['delay_ms'] as int);
    }
    throw const ProtocolException(
      ErrorCode.badRequest,
      'a key_sequence step must carry either `keys` or `delay_ms`',
    );
  }

  Map<String, dynamic> toJson();
}

class ComboStep extends SequenceStep {
  const ComboStep(this.combo);

  final KeyCombo combo;

  @override
  Map<String, dynamic> toJson() => combo.toJson();
}

class DelayStep extends SequenceStep {
  const DelayStep(this.delayMs);

  final int delayMs;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{'delay_ms': delayMs};
}

/// `input.text` — type a literal string.
class TextRequest implements Payload {
  const TextRequest(this.text, {this.delayMs});

  factory TextRequest.fromJson(Map<String, dynamic> json) =>
      TextRequest(json['text'] as String, delayMs: json['delay_ms'] as int?);

  final String text;

  /// Pause between characters, milliseconds. `0` means as fast as the OS allows.
  final int? delayMs;

  int get delayMsOrDefault => delayMs ?? 0;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{
    'text': text,
    if (delayMs != null) 'delay_ms': delayMs,
  };
}

/// `input.media`.
class MediaRequest implements Payload {
  const MediaRequest(this.command);

  factory MediaRequest.fromJson(Map<String, dynamic> json) =>
      MediaRequest(MediaCommand.fromWire(json['command'] as String));

  final MediaCommand command;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{'command': command.wire};
}

enum MediaCommand {
  playPause('PLAY_PAUSE'),
  next('NEXT'),
  prev('PREV'),
  stop('STOP'),
  volumeUp('VOLUME_UP'),
  volumeDown('VOLUME_DOWN'),
  mute('MUTE');

  const MediaCommand(this.wire);

  final String wire;

  static MediaCommand fromWire(String wire) => values.firstWhere(
    (v) => v.wire == wire,
    orElse: () => throw ProtocolException(
      ErrorCode.badRequest,
      'unknown media command "$wire"',
    ),
  );
}

/// `input.mouse`, tagged on `action`.
sealed class MouseRequest implements Payload {
  const MouseRequest();

  factory MouseRequest.fromJson(Map<String, dynamic> json) {
    final action = json['action'];
    double asDouble(String key) => (json[key] as num).toDouble();
    MouseButton button() => MouseButton.fromWire(json['button'] as String);

    return switch (action) {
      'move_rel' => MouseMoveRel(json['dx'] as int, json['dy'] as int),
      'move_abs' => MouseMoveAbs(asDouble('x'), asDouble('y')),
      'click' => MouseClick(button()),
      'down' => MouseDown(button()),
      'up' => MouseUp(button()),
      'scroll' => MouseScroll(asDouble('dx'), asDouble('dy')),
      _ => throw ProtocolException(
        ErrorCode.badRequest,
        'unknown mouse action "$action"',
      ),
    };
  }
}

/// Physical pixels, relative to the current cursor position.
class MouseMoveRel extends MouseRequest {
  const MouseMoveRel(this.dx, this.dy);

  final int dx;
  final int dy;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{
    'action': 'move_rel',
    'dx': dx,
    'dy': dy,
  };
}

/// Normalised `0.0..1.0` across the primary monitor, origin top-left — the client has no
/// idea what resolution the host runs.
class MouseMoveAbs extends MouseRequest {
  const MouseMoveAbs(this.x, this.y);

  final double x;
  final double y;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{
    'action': 'move_abs',
    'x': x,
    'y': y,
  };
}

class MouseClick extends MouseRequest {
  const MouseClick(this.button);

  final MouseButton button;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{
    'action': 'click',
    'button': button.wire,
  };
}

class MouseDown extends MouseRequest {
  const MouseDown(this.button);

  final MouseButton button;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{
    'action': 'down',
    'button': button.wire,
  };
}

class MouseUp extends MouseRequest {
  const MouseUp(this.button);

  final MouseButton button;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{
    'action': 'up',
    'button': button.wire,
  };
}

/// Notches; `1.0` is one detent. The engine converts per platform.
class MouseScroll extends MouseRequest {
  const MouseScroll(this.dx, this.dy);

  final double dx;
  final double dy;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{
    'action': 'scroll',
    'dx': dx,
    'dy': dy,
  };
}

enum MouseButton {
  left('left'),
  right('right'),
  middle('middle');

  const MouseButton(this.wire);

  final String wire;

  static MouseButton fromWire(String wire) => values.firstWhere(
    (v) => v.wire == wire,
    orElse: () => throw ProtocolException(
      ErrorCode.badRequest,
      'unknown mouse button "$wire"',
    ),
  );
}

/// A canonical key name. `docs/PROTOCOL.md` §5.
///
/// Uppercase, ASCII, no aliases. `META` is the Windows key on Windows and Linux, and
/// Command on macOS; the engine does **not** auto-swap `CONTROL`/`META` on macOS, because
/// profiles are per-host and the user maps what they want.
enum Key {
  // Modifiers
  control('CONTROL'),
  shift('SHIFT'),
  alt('ALT'),
  meta('META'),

  // Letters
  a('A'),
  b('B'),
  c('C'),
  d('D'),
  e('E'),
  f('F'),
  g('G'),
  h('H'),
  i('I'),
  j('J'),
  k('K'),
  l('L'),
  m('M'),
  n('N'),
  o('O'),
  p('P'),
  q('Q'),
  r('R'),
  s('S'),
  t('T'),
  u('U'),
  v('V'),
  w('W'),
  x('X'),
  y('Y'),
  z('Z'),

  // Digits
  digit0('DIGIT0'),
  digit1('DIGIT1'),
  digit2('DIGIT2'),
  digit3('DIGIT3'),
  digit4('DIGIT4'),
  digit5('DIGIT5'),
  digit6('DIGIT6'),
  digit7('DIGIT7'),
  digit8('DIGIT8'),
  digit9('DIGIT9'),

  // Function
  f1('F1'),
  f2('F2'),
  f3('F3'),
  f4('F4'),
  f5('F5'),
  f6('F6'),
  f7('F7'),
  f8('F8'),
  f9('F9'),
  f10('F10'),
  f11('F11'),
  f12('F12'),
  f13('F13'),
  f14('F14'),
  f15('F15'),
  f16('F16'),
  f17('F17'),
  f18('F18'),
  f19('F19'),
  f20('F20'),
  f21('F21'),
  f22('F22'),
  f23('F23'),
  f24('F24'),

  // Navigation
  escape('ESCAPE'),
  tab('TAB'),
  capsLock('CAPSLOCK'),
  space('SPACE'),
  enter('ENTER'),
  backspace('BACKSPACE'),
  delete('DELETE'),
  insert('INSERT'),
  home('HOME'),
  end('END'),
  pageUp('PAGEUP'),
  pageDown('PAGEDOWN'),
  left('LEFT'),
  right('RIGHT'),
  up('UP'),
  down('DOWN'),

  // Numpad
  numpad0('NUMPAD0'),
  numpad1('NUMPAD1'),
  numpad2('NUMPAD2'),
  numpad3('NUMPAD3'),
  numpad4('NUMPAD4'),
  numpad5('NUMPAD5'),
  numpad6('NUMPAD6'),
  numpad7('NUMPAD7'),
  numpad8('NUMPAD8'),
  numpad9('NUMPAD9'),
  numpadAdd('NUMPAD_ADD'),
  numpadSub('NUMPAD_SUB'),
  numpadMul('NUMPAD_MUL'),
  numpadDiv('NUMPAD_DIV'),
  numpadDecimal('NUMPAD_DECIMAL'),
  numpadEnter('NUMPAD_ENTER'),

  // Symbols
  minus('MINUS'),
  equal('EQUAL'),
  bracketLeft('BRACKET_LEFT'),
  bracketRight('BRACKET_RIGHT'),
  backslash('BACKSLASH'),
  semicolon('SEMICOLON'),
  quote('QUOTE'),
  backquote('BACKQUOTE'),
  comma('COMMA'),
  period('PERIOD'),
  slash('SLASH'),

  // System
  printScreen('PRINTSCREEN'),
  scrollLock('SCROLLLOCK'),
  pause('PAUSE'),
  numLock('NUMLOCK'),
  menu('MENU');

  const Key(this.wire);

  final String wire;

  static final Map<String, Key> _byWire = {
    for (final key in values) key.wire: key,
  };

  static Key fromWire(String wire) =>
      _byWire[wire] ??
      (throw ProtocolException(ErrorCode.badRequest, 'unknown key "$wire"'));

  /// `CONTROL`, `SHIFT`, `ALT` and `META` are the modifiers for the purposes of the combo
  /// rules in `docs/PROTOCOL.md` §4.3; everything else is a non-modifier.
  bool get isModifier =>
      this == Key.control ||
      this == Key.shift ||
      this == Key.alt ||
      this == Key.meta;
}
