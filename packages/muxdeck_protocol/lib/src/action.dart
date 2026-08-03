/// Named shell actions. `docs/PROTOCOL.md` §4.4 and §6.
library;

import 'envelope.dart';

/// A named, pre-defined action.
///
/// [command] and [args] are separate fields, never a single string, so the engine can never
/// pass anything to a shell interpreter. Shell execution is disabled by default — see
/// `docs/ARCHITECTURE.md` §5.5.
class Action {
  const Action({
    required this.id,
    required this.name,
    required this.command,
    required this.args,
    this.cwd,
  });

  factory Action.fromJson(Map<String, dynamic> json) => Action(
    id: json['id'] as String,
    name: json['name'] as String,
    command: json['command'] as String,
    args: (json['args'] as List<dynamic>).cast<String>(),
    cwd: json['cwd'] as String?,
  );

  final String id;
  final String name;
  final String command;
  final List<String> args;

  /// Working directory, or null to inherit the engine's.
  final String? cwd;

  /// `cwd` is emitted even when null, because the wire carries an explicit `null` and
  /// dropping the key would change the message.
  Map<String, dynamic> toJson() => <String, dynamic>{
    'id': id,
    'name': name,
    'command': command,
    'args': args,
    'cwd': cwd,
  };
}

/// `action.run`. The client sends an action *name*, never a command string.
class ActionRunRequest implements Payload {
  const ActionRunRequest(this.actionId);

  factory ActionRunRequest.fromJson(Map<String, dynamic> json) =>
      ActionRunRequest(json['action_id'] as String);

  final String actionId;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{'action_id': actionId};
}

/// `action.list` response.
///
/// Full [Action] objects for both roles: a deck can already execute every defined action,
/// so withholding the command string it will run buys nothing. Empty rather than an error
/// when shell actions are disabled, so a client can call it unconditionally at startup.
class ActionListResponse implements Payload {
  const ActionListResponse(this.actions);

  factory ActionListResponse.fromJson(Map<String, dynamic> json) =>
      ActionListResponse(
        (json['actions'] as List<dynamic>)
            .map((a) => Action.fromJson(a as Map<String, dynamic>))
            .toList(),
      );

  final List<Action> actions;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{
    'actions': actions.map((a) => a.toJson()).toList(),
  };
}

/// `action.set` — admin only. Creates or replaces by `id`.
class ActionSetRequest implements Payload {
  const ActionSetRequest(this.action);

  factory ActionSetRequest.fromJson(Map<String, dynamic> json) =>
      ActionSetRequest(Action.fromJson(json['action'] as Map<String, dynamic>));

  final Action action;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{'action': action.toJson()};
}

/// `action.delete` — admin only.
class ActionDeleteRequest implements Payload {
  const ActionDeleteRequest(this.actionId);

  factory ActionDeleteRequest.fromJson(Map<String, dynamic> json) =>
      ActionDeleteRequest(json['action_id'] as String);

  final String actionId;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{'action_id': actionId};
}
