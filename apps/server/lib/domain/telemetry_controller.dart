/// Live CPU and memory from the engine. `docs/PROTOCOL.md` §4.7.
library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:muxdeck_protocol/muxdeck_protocol.dart';

import '../providers.dart';

/// How many samples the dashboard sparkline keeps.
///
/// At the default one-second interval this is a minute of history — long enough to see a spike
/// that has already passed, short enough that a panel left open overnight is not holding a day
/// of readings in memory.
const telemetryHistory = 60;

class TelemetryState {
  const TelemetryState({this.samples = const [], this.subscribed = false});

  /// Oldest first.
  final List<TelemetryUpdate> samples;

  final bool subscribed;

  TelemetryUpdate? get latest => samples.isEmpty ? null : samples.last;
}

class TelemetryController extends Notifier<TelemetryState> {
  StreamSubscription<Envelope<RawPayload>>? _events;

  @override
  TelemetryState build() {
    // A reconnect means a new socket, and a subscription does not survive one. Rebuilding here
    // drops the old history with it, which is right: the numbers came from a session that is
    // over.
    ref.watch(adminSessionProvider);
    ref.onDispose(() => unawaited(_events?.cancel()));
    return const TelemetryState();
  }

  /// Subscribes and starts collecting. Idempotent.
  Future<void> subscribe() async {
    if (state.subscribed) return;

    final client = ref.read(adminSessionProvider.notifier).client;
    if (client == null) return;

    await client.request(KnownOp.telemetrySubscribe, const {});

    _events = client.events.listen((event) {
      if (event.op.known != KnownOp.telemetryUpdate) return;

      final sample = TelemetryUpdate.fromJson(event.d.json);
      final samples = [...state.samples, sample];
      state = TelemetryState(
        samples: samples.length > telemetryHistory
            ? samples.sublist(samples.length - telemetryHistory)
            : samples,
        subscribed: true,
      );
    });

    state = TelemetryState(samples: state.samples, subscribed: true);
  }
}
