/// Editing one button: label, icon, colour, haptic and action.
library;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:muxdeck_icons/muxdeck_icons.dart';
import 'package:muxdeck_protocol/muxdeck_protocol.dart';

import '../domain/key_capture.dart';

/// What the dialog returns: an edited button, or a request to clear the cell.
class ButtonEditResult {
  const ButtonEditResult.saved(this.button) : deleted = false;
  const ButtonEditResult.deleted() : button = null, deleted = true;

  final Button? button;
  final bool deleted;
}

const _palette = <String>[
  '#2D6CDF',
  '#6B4FBB',
  '#1F8A70',
  '#B3422F',
  '#B8860B',
  '#4A5568',
];

class ButtonEditorDialog extends StatefulWidget {
  const ButtonEditorDialog({
    required this.button,
    required this.col,
    required this.row,
    required this.shellActionsEnabled,
    super.key,
  });

  /// Null when the cell is empty and a button is being created.
  final Button? button;
  final int col;
  final int row;

  /// Shell execution is off by default, so `action.run` is offered but disabled with an
  /// explanation rather than hidden — hiding it would make the feature undiscoverable.
  /// `docs/ARCHITECTURE.md` §5.5.
  final bool shellActionsEnabled;

  @override
  State<ButtonEditorDialog> createState() => _ButtonEditorDialogState();
}

class _ButtonEditorDialogState extends State<ButtonEditorDialog> {
  late final TextEditingController _label = TextEditingController(
    text: widget.button?.label ?? '',
  );

  late String _icon = widget.button?.icon ?? 'circle';
  late String _colour = widget.button?.color ?? _palette.first;
  late Haptic _haptic = widget.button?.haptic ?? Haptic.light;

  late KnownOp _op = widget.button?.onTap?.op.known ?? KnownOp.inputKeyCombo;
  late List<String> _keys = _initialKeys();
  late MediaCommand _media = _initialMedia();
  late final TextEditingController _text = TextEditingController(
    text: widget.button?.onTap?.d['text'] as String? ?? '',
  );

  /// True while the key-capture field is listening.
  var _capturing = false;
  final _captureFocus = FocusNode();

  /// Whether this button has a long-press action, which changes how it fires.
  late bool _hasLongPress = widget.button?.onLongPress != null;

  List<String> _initialKeys() {
    final raw = widget.button?.onTap?.d['keys'];
    if (raw is List) return raw.cast<String>().toList();
    return const [];
  }

  MediaCommand _initialMedia() {
    final raw = widget.button?.onTap?.d['command'];
    if (raw is String) {
      try {
        return MediaCommand.fromWire(raw);
      } catch (_) {
        // Fall through to the default.
      }
    }
    return MediaCommand.playPause;
  }

  @override
  void dispose() {
    _label.dispose();
    _text.dispose();
    _captureFocus.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: Text(
        widget.button == null
            ? 'New button (${widget.col}, ${widget.row})'
            : 'Edit "${widget.button!.label}"',
      ),
      content: SizedBox(
        width: 520,
        child: SingleChildScrollView(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              TextField(
                controller: _label,
                decoration: const InputDecoration(
                  labelText: 'Label',
                  helperText: 'Shown on the deck button',
                ),
                autofocus: true,
              ),
              const SizedBox(height: 20),

              _section('Icon'),
              _IconPicker(
                selected: _icon,
                onSelected: (name) => setState(() => _icon = name),
              ),
              const SizedBox(height: 20),

              _section('Colour'),
              Row(
                children: [
                  for (final colour in _palette)
                    Padding(
                      padding: const EdgeInsets.only(right: 8),
                      child: _Swatch(
                        colour: colour,
                        selected: colour == _colour,
                        onTap: () => setState(() => _colour = colour),
                      ),
                    ),
                ],
              ),
              const SizedBox(height: 20),

              _section('Haptic'),
              SegmentedButton<Haptic>(
                segments: const [
                  ButtonSegment(value: Haptic.none, label: Text('None')),
                  ButtonSegment(value: Haptic.light, label: Text('Light')),
                  ButtonSegment(value: Haptic.medium, label: Text('Medium')),
                  ButtonSegment(value: Haptic.heavy, label: Text('Heavy')),
                ],
                selected: {_haptic},
                onSelectionChanged: (s) => setState(() => _haptic = s.first),
              ),
              const SizedBox(height: 20),

              _section('What it does'),
              DropdownButtonFormField<KnownOp>(
                initialValue: _op,
                items: [
                  const DropdownMenuItem(
                    value: KnownOp.inputKeyCombo,
                    child: Text('Press a keyboard shortcut'),
                  ),
                  const DropdownMenuItem(
                    value: KnownOp.inputText,
                    child: Text('Type some text'),
                  ),
                  const DropdownMenuItem(
                    value: KnownOp.inputMedia,
                    child: Text('Media control'),
                  ),
                  DropdownMenuItem(
                    value: KnownOp.actionRun,
                    enabled: widget.shellActionsEnabled,
                    child: Text(
                      widget.shellActionsEnabled
                          ? 'Run a named action'
                          : 'Run a named action — shell actions are off',
                      style: widget.shellActionsEnabled
                          ? null
                          : TextStyle(color: Theme.of(context).disabledColor),
                    ),
                  ),
                ],
                onChanged: (op) {
                  if (op == null) return;
                  if (op == KnownOp.actionRun && !widget.shellActionsEnabled) {
                    return;
                  }
                  setState(() => _op = op);
                },
              ),
              const SizedBox(height: 12),
              _actionEditor(),

              const SizedBox(height: 20),
              CheckboxListTile(
                contentPadding: EdgeInsets.zero,
                value: _hasLongPress,
                onChanged: (v) => setState(() => _hasLongPress = v ?? false),
                title: const Text('Has a separate long-press action'),
              ),
              if (_hasLongPress) const _LongPressWarning(),
            ],
          ),
        ),
      ),
      actions: [
        if (widget.button != null)
          TextButton(
            onPressed: () =>
                Navigator.of(context).pop(const ButtonEditResult.deleted()),
            child: const Text('Remove'),
          ),
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Cancel'),
        ),
        FilledButton(onPressed: _save, child: const Text('Save')),
      ],
    );
  }

  Widget _section(String title) => Padding(
    padding: const EdgeInsets.only(bottom: 8),
    child: Text(title, style: Theme.of(context).textTheme.titleSmall),
  );

  /// The action editor changes shape with the chosen op. `docs/SERVER.md` §6.
  Widget _actionEditor() => switch (_op) {
    KnownOp.inputKeyCombo => _keyCapture(),
    KnownOp.inputText => TextField(
      controller: _text,
      decoration: const InputDecoration(
        labelText: 'Text to type',
        helperText: 'Typed exactly, including spaces',
      ),
    ),
    KnownOp.inputMedia => DropdownButtonFormField<MediaCommand>(
      initialValue: _media,
      decoration: const InputDecoration(labelText: 'Media command'),
      items: [
        for (final command in MediaCommand.values)
          DropdownMenuItem(value: command, child: Text(_mediaLabel(command))),
      ],
      onChanged: (c) => setState(() => _media = c ?? MediaCommand.playPause),
    ),
    _ => const Text('Choose a defined action once shell actions are enabled.'),
  };

  /// "Press the combo you want", plus a list for keys a keyboard cannot easily produce.
  Widget _keyCapture() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Focus(
          focusNode: _captureFocus,
          onKeyEvent: _onKey,
          child: GestureDetector(
            onTap: () {
              setState(() {
                _capturing = true;
                _keys = [];
              });
              _captureFocus.requestFocus();
            },
            child: Container(
              width: double.infinity,
              padding: const EdgeInsets.all(16),
              decoration: BoxDecoration(
                border: Border.all(
                  color: _capturing
                      ? Theme.of(context).colorScheme.primary
                      : Theme.of(context).colorScheme.outline,
                  width: _capturing ? 2 : 1,
                ),
                borderRadius: BorderRadius.circular(8),
              ),
              child: Text(
                _capturing
                    ? 'Press the combination now…'
                    : _keys.isEmpty
                    ? 'Click here, then press the combination you want'
                    : _keys.join(' + '),
                style: TextStyle(
                  fontSize: 16,
                  color: _keys.isEmpty && !_capturing
                      ? Theme.of(context).colorScheme.outline
                      : null,
                ),
              ),
            ),
          ),
        ),
        const SizedBox(height: 8),
        Row(
          children: [
            // The fallback the spec asks for: a keyboard cannot always produce every key —
            // F13-F24 for instance — so capture is never the only way in.
            TextButton.icon(
              onPressed: _pickFromList,
              icon: const Icon(Icons.list),
              label: const Text('Choose from a list'),
            ),
            if (_keys.isNotEmpty)
              TextButton(
                onPressed: () => setState(() => _keys = []),
                child: const Text('Clear'),
              ),
          ],
        ),
      ],
    );
  }

  KeyEventResult _onKey(FocusNode node, KeyEvent event) {
    if (!_capturing || event is! KeyDownEvent) return KeyEventResult.ignored;

    final pressed = <String>{};
    for (final key in HardwareKeyboard.instance.logicalKeysPressed) {
      final name = canonicalKeyName(key);
      if (name != null) pressed.add(name);
    }
    if (pressed.isEmpty) return KeyEventResult.ignored;

    setState(() {
      // Ordered so modifiers precede the key they modify — `input.key_combo` presses in listed
      // order, so the wrong order would tap the key before the modifier is held and the
      // shortcut would never fire.
      _keys = orderCombo(pressed);

      // Stop as soon as a non-modifier lands, which is what "the combination" means. Holding
      // only modifiers keeps capturing so `META` alone can still be recorded via the list.
      if (_keys.any((k) => !isModifierName(k))) _capturing = false;
    });
    return KeyEventResult.handled;
  }

  Future<void> _pickFromList() async {
    final chosen = await showDialog<List<String>>(
      context: context,
      builder: (_) => _KeyListDialog(initial: _keys),
    );
    if (chosen != null) setState(() => _keys = orderCombo(chosen));
  }

  String _mediaLabel(MediaCommand command) => switch (command) {
    MediaCommand.playPause => 'Play / pause',
    MediaCommand.next => 'Next track',
    MediaCommand.prev => 'Previous track',
    MediaCommand.stop => 'Stop',
    MediaCommand.volumeUp => 'Volume up',
    MediaCommand.volumeDown => 'Volume down',
    MediaCommand.mute => 'Mute',
  };

  void _save() {
    final label = _label.text.trim();
    if (label.isEmpty) {
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(const SnackBar(content: Text('Give the button a label.')));
      return;
    }

    final Map<String, dynamic> payload = switch (_op) {
      KnownOp.inputKeyCombo => {'keys': _keys},
      KnownOp.inputText => {'text': _text.text},
      KnownOp.inputMedia => {'command': _media.wire},
      _ => const {},
    };

    if (_op == KnownOp.inputKeyCombo && _keys.isEmpty) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Capture a key combination first.')),
      );
      return;
    }

    final action = ButtonAction(Op.of(_op), payload);

    Navigator.of(context).pop(
      ButtonEditResult.saved(
        Button(
          // A stable ID across edits, and a fresh one for a new button. Position is part of it
          // so two new buttons cannot collide, which the engine would reject.
          id: widget.button?.id ?? 'b_${widget.col}_${widget.row}',
          pos: Position(col: widget.col, row: widget.row),
          label: label,
          icon: _icon,
          color: _colour,
          haptic: _haptic,
          onTap: action,
          onLongPress: _hasLongPress
              ? widget.button?.onLongPress ?? action
              : null,
        ),
      ),
    );
  }
}

/// Why assigning a long press makes a button feel slower.
class _LongPressWarning extends StatelessWidget {
  const _LongPressWarning();

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: const Color(0xFFB8860B).withValues(alpha: 0.15),
        border: Border.all(color: const Color(0xFFB8860B)),
        borderRadius: BorderRadius.circular(8),
      ),
      child: const Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(Icons.timer, size: 18, color: Color(0xFFB8860B)),
          SizedBox(width: 10),
          Expanded(
            // The user should learn this here rather than by wondering why one key feels
            // wrong. `docs/SERVER.md` §6 and `docs/CLIENT.md` §6.
            child: Text(
              'This button will feel slower than the others. Buttons fire the moment you touch '
              'them; one with a long-press action has to wait for you to lift your finger to '
              'tell a tap from a hold.',
            ),
          ),
        ],
      ),
    );
  }
}

class _Swatch extends StatelessWidget {
  const _Swatch({
    required this.colour,
    required this.selected,
    required this.onTap,
  });

  final String colour;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final value = int.parse(colour.replaceFirst('#', ''), radix: 16);
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(20),
      child: Container(
        width: 32,
        height: 32,
        decoration: BoxDecoration(
          color: Color(0xFF000000 | value),
          shape: BoxShape.circle,
          border: Border.all(
            color: selected
                ? Theme.of(context).colorScheme.onSurface
                : Colors.transparent,
            width: 3,
          ),
        ),
      ),
    );
  }
}

/// Offers only names the deck can actually render.
class _IconPicker extends StatelessWidget {
  const _IconPicker({required this.selected, required this.onSelected});

  final String selected;
  final void Function(String) onSelected;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: 120,
      child: Container(
        decoration: BoxDecoration(
          border: Border.all(
            color: Theme.of(context).colorScheme.outlineVariant,
          ),
          borderRadius: BorderRadius.circular(8),
        ),
        child: GridView.builder(
          padding: const EdgeInsets.all(6),
          gridDelegate: const SliverGridDelegateWithMaxCrossAxisExtent(
            maxCrossAxisExtent: 40,
          ),
          // Sourced from muxdeck_icons, the same map the deck renders from — so the picker can
          // never offer a name that would draw as a fallback dot. `docs/CLIENT.md` §5.
          itemCount: iconNames.length,
          itemBuilder: (context, index) {
            final name = iconNames[index];
            return IconButton(
              tooltip: name,
              isSelected: name == selected,
              onPressed: () => onSelected(name),
              icon: Icon(iconFor(name), size: 18),
              style: name == selected
                  ? IconButton.styleFrom(
                      backgroundColor: Theme.of(
                        context,
                      ).colorScheme.primaryContainer,
                    )
                  : null,
            );
          },
        ),
      ),
    );
  }
}

/// The searchable fallback for keys a keyboard cannot easily produce.
class _KeyListDialog extends StatefulWidget {
  const _KeyListDialog({required this.initial});

  final List<String> initial;

  @override
  State<_KeyListDialog> createState() => _KeyListDialogState();
}

class _KeyListDialogState extends State<_KeyListDialog> {
  late final Set<String> _selected = widget.initial.toSet();
  var _filter = '';

  @override
  Widget build(BuildContext context) {
    final matches = allKeyNames
        .where((n) => n.toLowerCase().contains(_filter.toLowerCase()))
        .toList();

    return AlertDialog(
      title: const Text('Choose keys'),
      content: SizedBox(
        width: 420,
        height: 420,
        child: Column(
          children: [
            TextField(
              decoration: const InputDecoration(
                labelText: 'Search',
                prefixIcon: Icon(Icons.search),
              ),
              onChanged: (v) => setState(() => _filter = v),
            ),
            const SizedBox(height: 8),
            Expanded(
              child: ListView.builder(
                itemCount: matches.length,
                itemBuilder: (context, index) {
                  final name = matches[index];
                  return CheckboxListTile(
                    dense: true,
                    value: _selected.contains(name),
                    title: Text(name),
                    onChanged: (on) => setState(() {
                      if (on ?? false) {
                        _selected.add(name);
                      } else {
                        _selected.remove(name);
                      }
                    }),
                  );
                },
              ),
            ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Cancel'),
        ),
        FilledButton(
          onPressed: () => Navigator.of(context).pop(_selected.toList()),
          child: const Text('Use these'),
        ),
      ],
    );
  }
}
