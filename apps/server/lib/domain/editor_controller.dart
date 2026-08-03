/// The profile being edited, and pushing changes to the engine.
library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:muxdeck_protocol/muxdeck_protocol.dart';

import '../providers.dart';
import 'admin_session.dart';

/// The active profile, kept in step with the engine.
///
/// The panel subscribes like any other client rather than tracking its own edits: a second
/// panel, or `muxdeckd` itself, could change a profile, and the editor must show what is really
/// stored rather than what it last sent.
class EditorController extends Notifier<Profile?> {
  StreamSubscription<Envelope<RawPayload>>? _events;

  @override
  Profile? build() {
    ref.onDispose(() => _events?.cancel());

    ref.listen(adminSessionProvider, (previous, next) {
      if (next is AdminReady) {
        unawaited(_load(next));
      } else {
        _events?.cancel();
        _events = null;
      }
    });

    final current = ref.read(adminSessionProvider);
    if (current is AdminReady) unawaited(_load(current));
    return null;
  }

  Future<void> _load(AdminReady session) async {
    final client = ref.read(adminSessionProvider.notifier).client;
    if (client == null) return;

    try {
      final response = await client.request(KnownOp.profileGet, {
        'profile_id': session.ready.activeProfileId,
      });
      state = ProfileWrapper.fromJson(response).profile;

      await client.request(KnownOp.profileSubscribe, const {});
      _events?.cancel();
      _events = client.events.listen((event) {
        if (event.op.known != KnownOp.profileChanged) return;
        state = ProfileWrapper.fromJson(event.d.json).profile;
      });
    } catch (_) {
      // Leave whatever is on screen; the editor is not useful without a profile but an error
      // here is transient and the next connection will retry.
    }
  }

  /// Replaces a button and saves.
  Future<void> saveButton(Button button) async {
    final profile = state;
    if (profile == null) return;

    final page = profile.pages.first;
    final buttons = [...page.buttons.where((b) => b.id != button.id), button];
    await _save(profile, buttons);
  }

  /// Removes a button and saves.
  Future<void> clearCell(String buttonId) async {
    final profile = state;
    if (profile == null) return;

    final page = profile.pages.first;
    await _save(profile, page.buttons.where((b) => b.id != buttonId).toList());
  }

  /// Writes the profile back.
  ///
  /// The engine validates and rejects with a specific message rather than coercing, so a
  /// failure here is worth surfacing verbatim — it says exactly which rule was broken.
  Future<void> _save(Profile profile, List<Button> buttons) async {
    final client = ref.read(adminSessionProvider.notifier).client;
    if (client == null) {
      throw const TransportFailed('Not connected to the engine.');
    }

    final page = profile.pages.first;
    final updated = Profile(
      id: profile.id,
      name: profile.name,
      grid: profile.grid,
      pages: [
        Page(id: page.id, name: page.name, buttons: buttons),
        ...profile.pages.skip(1),
      ],
    );

    await client.request(KnownOp.profileSet, {'profile': updated.toJson()});
    // The engine echoes the change back as `profile.changed`, which the subscription above
    // applies — so state is not set here. One path in, one path out.
  }
}
