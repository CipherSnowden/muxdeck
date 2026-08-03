/// Named shell actions. `docs/SERVER.md` §6 (Actions), `docs/PROTOCOL.md` §4.4.
library;

import 'dart:async';

import 'package:flutter/material.dart' hide Action;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:muxdeck_protocol/muxdeck_protocol.dart';

import '../domain/admin_session.dart';
import '../providers.dart';

class ActionsPage extends ConsumerStatefulWidget {
  const ActionsPage({super.key});

  @override
  ConsumerState<ActionsPage> createState() => _ActionsPageState();
}

class _ActionsPageState extends ConsumerState<ActionsPage> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) unawaited(ref.read(actionsProvider.notifier).load());
    });
  }

  @override
  Widget build(BuildContext context) {
    final admin = ref.watch(adminSessionProvider);
    final state = ref.watch(actionsProvider);

    final enabled =
        admin is AdminReady && admin.ready.capabilities.shellActions;

    if (!enabled) return const _Disabled();

    return Scaffold(
      floatingActionButton: FloatingActionButton.extended(
        onPressed: () => _edit(context, ref, null),
        icon: const Icon(Icons.add),
        label: const Text('New action'),
      ),
      body: ListView(
        padding: const EdgeInsets.all(24),
        children: [
          if (state.error != null)
            Padding(
              padding: const EdgeInsets.only(bottom: 16),
              child: Text(
                state.error!,
                style: const TextStyle(color: Color(0xFFB3422F)),
              ),
            ),
          if (state.actions.isEmpty)
            const _Empty()
          else
            for (final action in state.actions)
              Card(
                child: ListTile(
                  title: Text(action.name),
                  subtitle: Text(
                    // Shown as command plus arguments, spaced for reading only. It is never
                    // reassembled into a string that anything executes — see the editor.
                    [action.command, ...action.args].join(' '),
                    style: const TextStyle(fontFamily: 'monospace'),
                  ),
                  trailing: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      IconButton(
                        icon: const Icon(Icons.play_arrow),
                        tooltip: 'Run it now',
                        onPressed: () =>
                            ref.read(actionsProvider.notifier).run(action.id),
                      ),
                      IconButton(
                        icon: const Icon(Icons.edit),
                        tooltip: 'Edit',
                        onPressed: () => _edit(context, ref, action),
                      ),
                      IconButton(
                        icon: const Icon(Icons.delete_outline),
                        tooltip: 'Delete',
                        onPressed: () => _delete(context, ref, action),
                      ),
                    ],
                  ),
                ),
              ),
        ],
      ),
    );
  }

  Future<void> _edit(
    BuildContext context,
    WidgetRef ref,
    Action? existing,
  ) async {
    final action = await showDialog<Action>(
      context: context,
      builder: (context) => _ActionEditor(existing: existing),
    );
    if (action != null) await ref.read(actionsProvider.notifier).save(action);
  }

  Future<void> _delete(
    BuildContext context,
    WidgetRef ref,
    Action action,
  ) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text('Delete “${action.name}”?'),
        // The engine deliberately does not rewrite profiles on delete: it re-checks at press
        // time, so a stale button fails with NOT_FOUND rather than the delete having to touch
        // every layout that might mention it (`docs/PROTOCOL.md` §4.4).
        content: const Text(
          'Any deck button pointing at this action will stop working and show an error when '
          'pressed. The button itself is left alone.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: const Text('Delete'),
          ),
        ],
      ),
    );

    if (confirmed ?? false) {
      await ref.read(actionsProvider.notifier).delete(action.id);
    }
  }
}

/// Command and arguments as **separate fields**.
///
/// Not a single command line, and not splittable from one. A text box that a panel splits on
/// spaces is a shell by another name: it would have to decide what quotes and backslashes mean,
/// and every such decision is a way for an argument to become a second command. Typing arguments
/// one per line is slightly more work once and removes the whole class of problem
/// (`docs/ARCHITECTURE.md` §5.5).
class _ActionEditor extends StatefulWidget {
  const _ActionEditor({this.existing});

  final Action? existing;

  @override
  State<_ActionEditor> createState() => _ActionEditorState();
}

class _ActionEditorState extends State<_ActionEditor> {
  late final _name = TextEditingController(text: widget.existing?.name ?? '');
  late final _command = TextEditingController(
    text: widget.existing?.command ?? '',
  );
  late final _args = TextEditingController(
    text: widget.existing?.args.join('\n') ?? '',
  );
  late final _cwd = TextEditingController(text: widget.existing?.cwd ?? '');

  @override
  void dispose() {
    _name.dispose();
    _command.dispose();
    _args.dispose();
    _cwd.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: Text(widget.existing == null ? 'New action' : 'Edit action'),
      content: SizedBox(
        width: 520,
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              TextField(
                controller: _name,
                autofocus: true,
                decoration: const InputDecoration(
                  labelText: 'Name',
                  helperText:
                      'What you will see when assigning it to a button.',
                ),
              ),
              const SizedBox(height: 16),
              TextField(
                controller: _command,
                decoration: const InputDecoration(
                  labelText: 'Program',
                  helperText: 'The executable, on PATH or as a full path.',
                ),
              ),
              const SizedBox(height: 16),
              TextField(
                controller: _args,
                minLines: 3,
                maxLines: 6,
                decoration: const InputDecoration(
                  labelText: 'Arguments',
                  helperText:
                      'One per line. Spaces and quotes are part of the argument, not '
                      'separators — nothing here is interpreted by a shell.',
                  alignLabelWithHint: true,
                ),
              ),
              const SizedBox(height: 16),
              TextField(
                controller: _cwd,
                decoration: const InputDecoration(
                  labelText: 'Working directory (optional)',
                  helperText: "Leave blank to use the engine's.",
                ),
              ),
            ],
          ),
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Cancel'),
        ),
        FilledButton(onPressed: _submit, child: const Text('Save')),
      ],
    );
  }

  void _submit() {
    final command = _command.text.trim();
    if (command.isEmpty) return;

    final cwd = _cwd.text.trim();
    Navigator.of(context).pop(
      Action(
        id: widget.existing?.id ?? _newId(_name.text),
        name: _name.text.trim().isEmpty ? command : _name.text.trim(),
        command: command,
        // Only blank lines are dropped. Leading and trailing spaces are kept, because an
        // argument that is deliberately padded is the user's business.
        args: _args.text
            .split('\n')
            .where((line) => line.trim().isNotEmpty)
            .toList(),
        cwd: cwd.isEmpty ? null : cwd,
      ),
    );
  }
}

/// `a_` plus a slug of the name, which keeps the ID readable in a profile's JSON.
///
/// Uniqueness comes from the timestamp suffix rather than from the slug: two actions called
/// "Build" must not silently replace one another, and `action.set` keys on the ID.
String _newId(String name) {
  final slug = name
      .toLowerCase()
      .replaceAll(RegExp('[^a-z0-9]+'), '_')
      .replaceAll(RegExp(r'^_+|_+$'), '');
  final stamp = DateTime.now().millisecondsSinceEpoch.toRadixString(36);
  return 'a_${slug.isEmpty ? 'action' : slug}_$stamp';
}

class _Disabled extends ConsumerWidget {
  const _Disabled();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Center(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 460),
        child: Padding(
          padding: const EdgeInsets.all(32),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              const Icon(Icons.lock_outline, size: 48),
              const SizedBox(height: 20),
              Text(
                'Shell actions are switched off',
                style: Theme.of(context).textTheme.titleLarge,
              ),
              const SizedBox(height: 10),
              const Text(
                'Actions let a deck button run a program on this computer. They are off by '
                'default because any paired device could then run them.',
                textAlign: TextAlign.center,
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _Empty extends StatelessWidget {
  const _Empty();

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 48),
      child: Column(
        children: [
          const Icon(Icons.terminal, size: 40),
          const SizedBox(height: 12),
          Text(
            'No actions defined yet',
            style: Theme.of(context).textTheme.titleMedium,
          ),
        ],
      ),
    );
  }
}
