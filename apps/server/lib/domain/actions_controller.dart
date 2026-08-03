/// Named shell actions, over the admin socket. `docs/PROTOCOL.md` §4.4.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:muxdeck_protocol/muxdeck_protocol.dart';

import '../providers.dart';

class ActionsState {
  const ActionsState({this.actions = const [], this.error});

  final List<Action> actions;
  final String? error;
}

class ActionsController extends Notifier<ActionsState> {
  @override
  ActionsState build() {
    ref.watch(adminSessionProvider);
    return const ActionsState();
  }

  ProtocolClient? get _client => ref.read(adminSessionProvider.notifier).client;

  /// Reloads the list.
  ///
  /// `action.list` never errors on the feature being off — it comes back empty — so this is safe
  /// to call before knowing whether shell actions are enabled.
  Future<void> load() async {
    final client = _client;
    if (client == null) return;

    try {
      final response = await client.request(KnownOp.actionList, const {});
      state = ActionsState(
        actions: ActionListResponse.fromJson(response).actions,
      );
    } on AppError catch (error) {
      state = ActionsState(actions: state.actions, error: error.message);
    }
  }

  Future<void> save(Action action) => _mutate(
    () async => _require().request(
      KnownOp.actionSet,
      ActionSetRequest(action).toJson(),
    ),
  );

  Future<void> delete(String actionId) => _mutate(
    () async => _require().request(
      KnownOp.actionDelete,
      ActionDeleteRequest(actionId).toJson(),
    ),
  );

  /// Runs an action from the panel, so a user can check it works before putting it on a button.
  ///
  /// Worth having: the alternative is assigning an untested command to a deck key and finding out
  /// from across the room that it does nothing.
  Future<void> run(String actionId) => _mutate(
    () async => _require().request(
      KnownOp.actionRun,
      ActionRunRequest(actionId).toJson(),
    ),
    reload: false,
  );

  Future<void> _mutate(
    Future<void> Function() operation, {
    bool reload = true,
  }) async {
    try {
      await operation();
      state = ActionsState(actions: state.actions);
      if (reload) await load();
    } on AppError catch (error) {
      state = ActionsState(actions: state.actions, error: error.message);
    }
  }

  ProtocolClient _require() {
    final client = _client;
    if (client == null) {
      throw const TransportFailed('Not connected to the engine.');
    }
    return client;
  }
}
