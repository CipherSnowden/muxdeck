/// Finding the engine: its config directory, its credentials, and its binary.
///
/// The panel is not the server (`docs/SERVER.md` §1). It reads what the daemon wrote and talks
/// to it over an ordinary socket — the same one a phone uses, differing only in the role it is
/// granted.
library;

import 'dart:convert';
import 'dart:io';

import 'package:crypto/crypto.dart';
import 'package:path/path.dart' as p;

/// Where `muxdeckd` keeps its state, per platform.
///
/// These mirror `docs/ENGINE.md` §6. The Windows path was verified against what the
/// `directories` crate actually emits — note there is **no** `in.` qualifier on Windows, which
/// the docs originally got wrong; the crate uses `{Organization}\{Application}` only there.
Directory? engineConfigDirectory() {
  final env = Platform.environment;

  if (Platform.isWindows) {
    final appData = env['APPDATA'];
    if (appData == null) return null;
    return Directory(p.join(appData, 'redoimagined', 'MuxDeck', 'config'));
  }

  final home = env['HOME'];
  if (home == null) return null;

  if (Platform.isMacOS) {
    return Directory(
      p.join(home, 'Library', 'Application Support', 'in.redoimagined.MuxDeck'),
    );
  }
  return Directory(p.join(home, '.config', 'muxdeck'));
}

/// The credentials the panel needs to reach the engine.
class EngineCredentials {
  const EngineCredentials({required this.adminToken, required this.fingerprint});

  /// Read from `admin.token`. **Never log this** — its file permissions are the entire boundary
  /// between this desktop user and any other local user (`docs/ARCHITECTURE.md` §5.4).
  final String adminToken;

  /// SHA-256 over the leaf certificate DER, lowercase hex.
  final String fingerprint;
}

/// Reads the admin token and derives the certificate fingerprint from the config directory.
///
/// Reading the certificate directly rather than shelling out to `muxdeckd --print-fingerprint`:
/// it is the same value by construction, and it avoids spawning a process on every launch just
/// to learn something already sitting on disk. `docs/SERVER.md` §4 permits either.
///
/// Returns null when the engine has never run — there is nothing to read yet, which is a
/// first-run state rather than an error.
Future<EngineCredentials?> readEngineCredentials() async {
  final dir = engineConfigDirectory();
  if (dir == null || !dir.existsSync()) return null;

  final tokenFile = File(p.join(dir.path, 'admin.token'));
  final certFile = File(p.join(dir.path, 'tls.pem'));
  if (!tokenFile.existsSync() || !certFile.existsSync()) return null;

  final token = (await tokenFile.readAsString()).trim();
  final fingerprint = fingerprintFromPem(await certFile.readAsString());
  if (token.isEmpty || fingerprint == null) return null;

  return EngineCredentials(adminToken: token, fingerprint: fingerprint);
}

/// SHA-256 over the certificate's DER bytes, lowercase hex.
///
/// Over the **DER**, not the PEM text: the PEM is base64 with a header and line breaks, and
/// hashing that produces a value nothing else in the system agrees with. `docs/PROTOCOL.md` §1.
String? fingerprintFromPem(String pem) {
  // Each line is trimmed rather than just the joined result: splitting CRLF text on `\n` leaves
  // a `\r` at the end of every line, and base64 with embedded carriage returns decodes to
  // rubbish — producing a plausible-looking fingerprint that matches nothing. This is a Windows
  // file, so CRLF is not a hypothetical.
  final body = pem
      .split('\n')
      .map((line) => line.trim())
      .where((line) => line.isNotEmpty && !line.startsWith('-----'))
      .join();
  if (body.isEmpty) return null;

  try {
    return sha256.convert(base64Decode(body)).toString();
  } catch (_) {
    return null;
  }
}

/// Locates the `muxdeckd` executable.
///
/// Beside the panel first, then on `PATH` — matching the order in `docs/SERVER.md` §5. The
/// binary ships **alongside** the app rather than as a Flutter asset, because assets are
/// extracted to temp directories, which breaks code signing on macOS and trips antivirus on
/// Windows.
File? findEngineExecutable() {
  final name = Platform.isWindows ? 'muxdeckd.exe' : 'muxdeckd';

  final candidates = <String>[
    p.join(p.dirname(Platform.resolvedExecutable), name),
    // A development tree: the panel runs from build/, the daemon from engine/target/debug.
    p.join(p.dirname(Platform.resolvedExecutable), '..', '..', '..', '..', '..', 'engine',
        'target', 'debug', name),
  ];

  for (final candidate in candidates) {
    final file = File(p.normalize(candidate));
    if (file.existsSync()) return file;
  }

  for (final entry in (Platform.environment['PATH'] ?? '').split(Platform.isWindows ? ';' : ':')) {
    if (entry.isEmpty) continue;
    final file = File(p.join(entry, name));
    if (file.existsSync()) return file;
  }

  return null;
}
