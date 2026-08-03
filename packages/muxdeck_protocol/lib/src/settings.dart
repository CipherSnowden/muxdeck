/// Engine settings. `docs/PROTOCOL.md` §4.6. Admin only.
library;

import 'envelope.dart';

/// The full settings object, as returned by `settings.get`.
class Settings implements Payload {
  const Settings({
    required this.port,
    required this.hostName,
    required this.shellActionsEnabled,
    required this.telemetryEnabled,
    required this.telemetryIntervalMs,
    required this.autostart,
  });

  factory Settings.fromJson(Map<String, dynamic> json) => Settings(
    port: json['port'] as int,
    hostName: json['host_name'] as String,
    shellActionsEnabled: json['shell_actions_enabled'] as bool,
    telemetryEnabled: json['telemetry_enabled'] as bool,
    telemetryIntervalMs: json['telemetry_interval_ms'] as int,
    autostart: json['autostart'] as bool,
  );

  final int port;
  final String hostName;
  final bool shellActionsEnabled;
  final bool telemetryEnabled;
  final int telemetryIntervalMs;
  final bool autostart;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{
    'port': port,
    'host_name': hostName,
    'shell_actions_enabled': shellActionsEnabled,
    'telemetry_enabled': telemetryEnabled,
    'telemetry_interval_ms': telemetryIntervalMs,
    'autostart': autostart,
  };
}

/// `settings.set` request: a **partial** settings object.
///
/// Only the keys present are changed; an absent key is left alone rather than reset to its
/// default. An empty patch is valid.
class SettingsPatch implements Payload {
  const SettingsPatch({
    this.port,
    this.hostName,
    this.shellActionsEnabled,
    this.telemetryEnabled,
    this.telemetryIntervalMs,
    this.autostart,
  });

  factory SettingsPatch.fromJson(Map<String, dynamic> json) => SettingsPatch(
    port: json['port'] as int?,
    hostName: json['host_name'] as String?,
    shellActionsEnabled: json['shell_actions_enabled'] as bool?,
    telemetryEnabled: json['telemetry_enabled'] as bool?,
    telemetryIntervalMs: json['telemetry_interval_ms'] as int?,
    autostart: json['autostart'] as bool?,
  );

  final int? port;
  final String? hostName;
  final bool? shellActionsEnabled;
  final bool? telemetryEnabled;
  final int? telemetryIntervalMs;
  final bool? autostart;

  /// True when applying this patch needs a daemon restart to take effect.
  ///
  /// Only `port` does. `host_name` triggers an mDNS re-advertise; everything else is live.
  bool get requiresRestart => port != null;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{
    if (port != null) 'port': port,
    if (hostName != null) 'host_name': hostName,
    if (shellActionsEnabled != null)
      'shell_actions_enabled': shellActionsEnabled,
    if (telemetryEnabled != null) 'telemetry_enabled': telemetryEnabled,
    if (telemetryIntervalMs != null)
      'telemetry_interval_ms': telemetryIntervalMs,
    if (autostart != null) 'autostart': autostart,
  };
}

class SettingsSetResponse implements Payload {
  const SettingsSetResponse(this.restartRequired);

  factory SettingsSetResponse.fromJson(Map<String, dynamic> json) =>
      SettingsSetResponse(json['restart_required'] as bool);

  final bool restartRequired;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{
    'restart_required': restartRequired,
  };
}
