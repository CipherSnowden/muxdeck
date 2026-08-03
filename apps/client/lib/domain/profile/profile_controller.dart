/// The layout the deck displays, live from the engine and cached across launches.
library;

import 'dart:async';
import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:muxdeck_protocol/muxdeck_protocol.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../../providers.dart';
import '../session/session_state.dart';

const _cacheKey = 'muxdeck.profile.cached';

/// What the deck screen renders.
class DeckLayout {
  const DeckLayout({required this.profile, required this.isLive});

  final Profile profile;

  /// False when this came from the cache and the engine has not confirmed it.
  ///
  /// The deck renders a stale layout immediately at launch, greyed out, rather than showing a
  /// spinner: the grid appearing instantly matters more than it being current
  /// (`docs/CLIENT.md` §7).
  final bool isLive;
}

/// Fetches the active profile, subscribes to changes, and caches the last one seen.
class ProfileController extends Notifier<DeckLayout?> {
  StreamSubscription<Envelope<RawPayload>>? _events;

  @override
  DeckLayout? build() {
    ref.onDispose(() => _events?.cancel());

    // Re-fetch whenever a session becomes ready, and drop to cached-only when it is lost.
    ref.listen(sessionProvider, (previous, next) {
      if (next is SessionReady) {
        unawaited(_load());
      } else if (next is! SessionAuthenticating) {
        _events?.cancel();
        _events = null;
        final current = state;
        if (current != null) {
          state = DeckLayout(profile: current.profile, isLive: false);
        }
      }
    });

    unawaited(_restoreFromCache());
    return null;
  }

  /// Renders whatever was showing last time, before the network is even reachable.
  Future<void> _restoreFromCache() async {
    if (state != null) return;

    final prefs = await SharedPreferences.getInstance();
    final cached = prefs.getString(_cacheKey);
    if (cached == null || state != null) return;

    try {
      final profile = Profile.fromJson(
        jsonDecode(cached) as Map<String, dynamic>,
      );
      state = DeckLayout(profile: profile, isLive: false);
    } catch (_) {
      // A corrupt cache is not worth surfacing: the engine is about to supply the real thing.
      await prefs.remove(_cacheKey);
    }
  }

  /// Fetches the active profile and subscribes to edits.
  Future<void> _load() async {
    final session = ref.read(sessionProvider);
    final client = ref.read(sessionProvider.notifier).client;
    if (client == null || session is! SessionReady) return;

    try {
      final response = await client.request(KnownOp.profileGet, {
        'profile_id': session.ready.activeProfileId,
      });
      final profile = ProfileWrapper.fromJson(response).profile;
      state = DeckLayout(profile: profile, isLive: true);
      unawaited(_cache(profile));

      // Explicit: the engine pushes nothing unasked. Without this the deck would show a layout
      // frozen at connection time and the editor's live loop would not reach it.
      await client.request(KnownOp.profileSubscribe, const {});

      _events?.cancel();
      _events = client.events.listen((event) {
        if (event.op.known != KnownOp.profileChanged) return;
        final updated = ProfileWrapper.fromJson(event.d.json).profile;
        state = DeckLayout(profile: updated, isLive: true);
        unawaited(_cache(updated));
      });
    } catch (_) {
      // Keep whatever is on screen. A deck showing a slightly stale grid beats one showing an
      // error where its buttons were.
    }
  }

  Future<void> _cache(Profile profile) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_cacheKey, jsonEncode(profile.toJson()));
  }
}
