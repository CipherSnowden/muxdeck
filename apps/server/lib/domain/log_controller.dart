/// The log tail the dashboard shows.
library;

import 'dart:async';
import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:path/path.dart' as p;

import '../data/engine_locator.dart';
import '../data/log_tail.dart';

/// How often to re-read the file.
///
/// Polling rather than watching. `File.watch` is unreliable across platforms for a file being
/// appended to by another process — on Windows it can miss appends entirely — and a log view
/// that silently stops updating is worse than one that is a second behind.
const logPollInterval = Duration(seconds: 1);

class LogState {
  const LogState({this.lines = const [], this.path, this.error});

  final List<String> lines;

  /// The file being read, for the "open config folder" affordance.
  final String? path;

  final String? error;
}

class LogController extends Notifier<LogState> {
  Timer? _timer;

  @override
  LogState build() {
    ref.onDispose(() => _timer?.cancel());
    return const LogState();
  }

  /// Starts polling. Idempotent.
  void start() {
    if (_timer != null) return;
    unawaited(refresh());
    _timer = Timer.periodic(logPollInterval, (_) => unawaited(refresh()));
  }

  void stop() {
    _timer?.cancel();
    _timer = null;
  }

  Future<void> refresh() async {
    final config = engineConfigDirectory();
    if (config == null) {
      state = const LogState(
        error: 'Could not work out where MuxDeck stores its files.',
      );
      return;
    }

    final file = newestLogFile(Directory(p.join(config.path, 'logs')));
    if (file == null) {
      // Not an error worth shouting about: a daemon started with `--foreground` logs to stdout
      // and writes no file at all, which is the normal state during development.
      state = LogState(
        error:
            'No log file yet. The engine writes one unless it was started with --foreground.',
      );
      return;
    }

    try {
      state = LogState(lines: await readTail(file), path: file.path);
    } on IOException catch (error) {
      state = LogState(
        path: file.path,
        error: 'Could not read the log: $error',
      );
    }
  }
}
