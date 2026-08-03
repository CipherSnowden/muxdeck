/// Telemetry, ping and lifecycle events. `docs/PROTOCOL.md` §4.7, §4.8 and §4.9.
library;

import 'envelope.dart';
import 'pairing.dart';

/// `system.ping` request. Milliseconds since the Unix epoch.
class PingRequest implements Payload {
  const PingRequest(this.tClient);

  factory PingRequest.fromJson(Map<String, dynamic> json) =>
      PingRequest(json['t_client'] as int);

  final int tClient;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{'t_client': tClient};
}

/// `system.ping` response — there is no `pong` op, this *is* the pong.
///
/// [tClient] is echoed verbatim so a client can match a reply to a send. Compute RTT from
/// your own send and receive timestamps; [tEngine] is for one-way-delay estimation, not
/// clock sync.
class PingResponse implements Payload {
  const PingResponse({required this.tClient, required this.tEngine});

  factory PingResponse.fromJson(Map<String, dynamic> json) => PingResponse(
    tClient: json['t_client'] as int,
    tEngine: json['t_engine'] as int,
  );

  final int tClient;
  final int tEngine;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{
    't_client': tClient,
    't_engine': tEngine,
  };
}

/// `evt telemetry.update`, to sockets that called `telemetry.subscribe`.
class TelemetryUpdate implements Payload {
  const TelemetryUpdate({
    required this.ts,
    required this.cpuPct,
    required this.ramPct,
  });

  factory TelemetryUpdate.fromJson(Map<String, dynamic> json) =>
      TelemetryUpdate(
        ts: json['ts'] as int,
        cpuPct: (json['cpu_pct'] as num).toDouble(),
        ramPct: (json['ram_pct'] as num).toDouble(),
      );

  /// Unix timestamp, seconds.
  final int ts;
  final double cpuPct;
  final double ramPct;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{
    'ts': ts,
    'cpu_pct': cpuPct,
    'ram_pct': ramPct,
  };
}

/// `evt device.changed`, to `admin` sockets only.
class DeviceChangedEvent implements Payload {
  const DeviceChangedEvent(this.devices);

  factory DeviceChangedEvent.fromJson(Map<String, dynamic> json) =>
      DeviceChangedEvent(
        (json['devices'] as List<dynamic>)
            .map((d) => DeviceInfo.fromJson(d as Map<String, dynamic>))
            .toList(),
      );

  final List<DeviceInfo> devices;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{
    'devices': devices.map((d) => d.toJson()).toList(),
  };
}

/// `evt engine.shutdown`, to every authenticated socket.
class EngineShutdownEvent implements Payload {
  const EngineShutdownEvent(this.reason);

  factory EngineShutdownEvent.fromJson(Map<String, dynamic> json) =>
      EngineShutdownEvent(ShutdownReason.fromWire(json['reason'] as String));

  final ShutdownReason reason;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{'reason': reason.wire};
}

/// An enum, not free text.
enum ShutdownReason {
  userRequested('user_requested'),
  settingsChanged('settings_changed'),
  fatalError('fatal_error');

  const ShutdownReason(this.wire);

  final String wire;

  static ShutdownReason fromWire(String wire) => values.firstWhere(
    (v) => v.wire == wire,
    orElse: () => throw ProtocolException(
      ErrorCode.badRequest,
      'unknown shutdown reason "$wire"',
    ),
  );
}
