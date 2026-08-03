/// Persisted list of paired hosts.
library;

import 'package:shared_preferences/shared_preferences.dart';

import 'host_record.dart';

const _hostsKey = 'muxdeck.hosts';
const _lastHostKey = 'muxdeck.hosts.last';

/// Stores host records in `shared_preferences`.
///
/// Deliberately not secure storage: a host record holds no secret. The fingerprint and host ID
/// are public, the address is on the local network, and the device *private* key — the only
/// secret involved — lives in [DeviceIdentityStore]. Putting these in the Keychain would buy
/// nothing and inherit its failure modes.
class HostStore {
  HostStore(this._prefs);

  final SharedPreferences _prefs;

  static Future<HostStore> open() async =>
      HostStore(await SharedPreferences.getInstance());

  List<HostRecord> all() => decodeHostRecords(_prefs.getString(_hostsKey));

  HostRecord? byId(String hostId) {
    for (final record in all()) {
      if (record.hostId == hostId) return record;
    }
    return null;
  }

  /// Adds or replaces a record, keyed by host ID.
  ///
  /// Replacing rather than appending matters on re-pair: the same host reached at a new address
  /// must update in place, not appear twice in the list.
  Future<void> save(HostRecord record) async {
    final records = all()..removeWhere((r) => r.hostId == record.hostId);
    records.add(record);
    await _prefs.setString(_hostsKey, encodeHostRecords(records));
  }

  Future<void> remove(String hostId) async {
    final records = all()..removeWhere((r) => r.hostId == hostId);
    await _prefs.setString(_hostsKey, encodeHostRecords(records));
    if (_prefs.getString(_lastHostKey) == hostId) {
      await _prefs.remove(_lastHostKey);
    }
  }

  /// The host to reconnect to on launch, so the deck comes back without a menu.
  String? get lastHostId => _prefs.getString(_lastHostKey);

  Future<void> setLastHostId(String hostId) =>
      _prefs.setString(_lastHostKey, hostId);
}
