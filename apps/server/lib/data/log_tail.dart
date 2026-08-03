/// Reading the engine's log file. `docs/SERVER.md` §6 (Dashboard).
library;

import 'dart:convert';
import 'dart:io';

import 'package:path/path.dart' as p;

/// Lines the dashboard keeps.
const logTailLines = 200;

/// How much of the end of the file to read.
///
/// The whole point of a tail is not to load a log that has been growing since Tuesday. 256 KiB
/// comfortably holds far more than [logTailLines] of `tracing` output, and reading a fixed
/// window from the end costs the same whether the file is a megabyte or a hundred.
const _tailWindowBytes = 256 * 1024;

/// The daily-rotated file the engine writes. Must match `muxdeckd`'s `LOG_FILE_PREFIX`.
const logFilePrefix = 'muxdeckd.log';

/// The newest log file in [logDirectory], or null when the engine has never written one.
///
/// Newest by name rather than by modification time: the suffix is `YYYY-MM-DD`, which sorts
/// chronologically as text, and file times can be rewritten by a backup tool in a way names
/// cannot.
File? newestLogFile(Directory logDirectory) {
  if (!logDirectory.existsSync()) return null;

  final files =
      logDirectory
          .listSync()
          .whereType<File>()
          .where((f) => p.basename(f.path).startsWith(logFilePrefix))
          .toList()
        ..sort((a, b) => p.basename(a.path).compareTo(p.basename(b.path)));

  return files.isEmpty ? null : files.last;
}

/// The last [maxLines] lines of [file].
///
/// Reads only the end of the file, and drops the first line of that window because a byte offset
/// almost never lands on a line boundary — keeping it would show half a message.
Future<List<String>> readTail(File file, {int maxLines = logTailLines}) async {
  final length = await file.length();
  final start = length > _tailWindowBytes ? length - _tailWindowBytes : 0;

  final bytes = await file.openRead(start).expand((chunk) => chunk).toList();
  // Malformed UTF-8 is possible at the cut point and is not worth failing over.
  var text = const Utf8Decoder(allowMalformed: true).convert(bytes);

  if (start > 0) {
    final firstBreak = text.indexOf('\n');
    text = firstBreak == -1 ? '' : text.substring(firstBreak + 1);
  }

  final lines = const LineSplitter()
      .convert(text)
      .where((line) => line.trim().isNotEmpty)
      .toList();

  return lines.length > maxLines
      ? lines.sublist(lines.length - maxLines)
      : lines;
}
