/// Reading the engine's log file.
library;

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:muxdeck_server/data/log_tail.dart';
import 'package:path/path.dart' as p;

void main() {
  late Directory dir;

  setUp(() => dir = Directory.systemTemp.createTempSync('muxdeck-log-test'));
  tearDown(() => dir.deleteSync(recursive: true));

  File write(String name, String content) {
    final file = File(p.join(dir.path, name))..writeAsStringSync(content);
    return file;
  }

  group('finding the file', () {
    test('an empty or missing directory yields nothing', () {
      expect(newestLogFile(dir), isNull);
      expect(newestLogFile(Directory(p.join(dir.path, 'nope'))), isNull);
    });

    test('picks the newest by date suffix, not by list order', () {
      // The suffix is YYYY-MM-DD, which sorts chronologically as text. Modification times are
      // deliberately not used: a backup tool can rewrite those, and names it cannot.
      write('$logFilePrefix.2026-08-01', 'old');
      write('$logFilePrefix.2026-08-03', 'new');
      write('$logFilePrefix.2026-08-02', 'middle');

      expect(p.basename(newestLogFile(dir)!.path), '$logFilePrefix.2026-08-03');
    });

    test('ignores files that are not the engine log', () {
      write('devices.json', '{}');
      expect(newestLogFile(dir), isNull);
    });
  });

  group('reading the tail', () {
    test('a short file comes back whole', () async {
      final file = write('$logFilePrefix.2026-08-03', 'one\ntwo\nthree\n');
      expect(await readTail(file), ['one', 'two', 'three']);
    });

    test('keeps only the last lines asked for', () async {
      final file = write(
        '$logFilePrefix.2026-08-03',
        [for (var i = 0; i < 500; i++) 'line $i'].join('\n'),
      );

      final lines = await readTail(file, maxLines: 10);
      expect(lines.length, 10);
      expect(lines.last, 'line 499', reason: 'the newest line must survive');
      expect(lines.first, 'line 490');
    });

    test('never returns a partial first line', () async {
      // The byte window almost never lands on a line boundary, and half a log message reads as
      // corruption rather than as a truncation.
      final file = write(
        '$logFilePrefix.2026-08-03',
        [
          for (var i = 0; i < 20000; i++) 'a fairly long log line number $i',
        ].join('\n'),
      );

      final lines = await readTail(file);
      expect(
        lines.first,
        startsWith('a fairly long log line number'),
        reason: 'the window must be trimmed back to a line start',
      );
    });

    test('blank lines are dropped rather than shown as gaps', () async {
      final file = write('$logFilePrefix.2026-08-03', 'one\n\n\ntwo\n');
      expect(await readTail(file), ['one', 'two']);
    });

    test('an empty file is empty, not an error', () async {
      final file = write('$logFilePrefix.2026-08-03', '');
      expect(await readTail(file), isEmpty);
    });
  });
}
