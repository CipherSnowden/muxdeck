/// The pairing window: a QR code, the code in large type, and a countdown.
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:muxdeck_protocol/muxdeck_protocol.dart';
import 'package:qr_flutter/qr_flutter.dart';

import '../domain/admin_session.dart';
import '../providers.dart';

class PairDialog extends ConsumerStatefulWidget {
  const PairDialog({super.key});

  @override
  ConsumerState<PairDialog> createState() => _PairDialogState();
}

class _PairDialogState extends ConsumerState<PairDialog> {
  PairBeginResponse? _window;
  String? _error;
  Timer? _tick;
  Duration _remaining = Duration.zero;

  /// Devices already paired when this opened, so a new arrival is detectable.
  late final int _deviceCountAtOpen =
      (ref.read(adminSessionProvider) is AdminReady)
      ? (ref.read(adminSessionProvider) as AdminReady).devices.length
      : 0;

  @override
  void initState() {
    super.initState();
    unawaited(_begin());
  }

  @override
  void dispose() {
    _tick?.cancel();
    super.dispose();
  }

  Future<void> _begin() async {
    try {
      final window = await ref
          .read(adminSessionProvider.notifier)
          .beginPairing();
      if (!mounted) return;
      setState(() {
        _window = window;
        _error = null;
      });
      _startCountdown(window);
    } catch (e) {
      if (mounted) setState(() => _error = '$e');
    }
  }

  void _startCountdown(PairBeginResponse window) {
    _tick?.cancel();
    final expiry = DateTime.fromMillisecondsSinceEpoch(window.expiresAt * 1000);

    void update() {
      final left = expiry.difference(DateTime.now());
      if (!mounted) return;
      setState(() => _remaining = left.isNegative ? Duration.zero : left);
      if (left.isNegative) _tick?.cancel();
    }

    update();
    _tick = Timer.periodic(const Duration(seconds: 1), (_) => update());
  }

  @override
  Widget build(BuildContext context) {
    // The engine pushes device.changed when a device pairs; the dialog closes on it rather than
    // making the user notice and dismiss it. `docs/SERVER.md` §6.
    ref.listen(adminSessionProvider, (previous, next) {
      if (next is AdminReady &&
          next.devices.length > _deviceCountAtOpen &&
          mounted) {
        Navigator.of(context).pop();
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(const SnackBar(content: Text('Device paired.')));
      }
    });

    return AlertDialog(
      title: const Text('Pair a device'),
      content: SizedBox(width: 420, child: _content()),
      actions: [
        TextButton(
          onPressed: () async {
            // Close the window on the engine too — leaving it open would let anyone who saw the
            // QR pair a device after this dialog is gone.
            try {
              await ref.read(adminSessionProvider.notifier).cancelPairing();
            } catch (_) {
              // The window expires on its own; failing to cancel is not worth an error.
            }
            if (context.mounted) Navigator.of(context).pop();
          },
          child: const Text('Cancel'),
        ),
      ],
    );
  }

  Widget _content() {
    final error = _error;
    if (error != null) {
      return Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          const Icon(Icons.error_outline, size: 40),
          const SizedBox(height: 12),
          Text(
            'Could not open a pairing window.\n$error',
            textAlign: TextAlign.center,
          ),
        ],
      );
    }

    final window = _window;
    if (window == null) {
      return const SizedBox(
        height: 300,
        child: Center(child: CircularProgressIndicator()),
      );
    }

    final expired = _remaining == Duration.zero;

    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        const Text('Open MuxDeck on your phone and scan this code.'),
        const SizedBox(height: 16),

        // High contrast and large: this is photographed by a phone camera, often at an angle.
        // A white quiet zone is part of the QR specification, not decoration — without it many
        // scanners fail to lock on.
        Container(
          padding: const EdgeInsets.all(16),
          decoration: BoxDecoration(
            color: Colors.white,
            borderRadius: BorderRadius.circular(12),
          ),
          child: Opacity(
            opacity: expired ? 0.25 : 1,
            child: QrImageView(
              data: window.qrPayload,
              size: 240,
              backgroundColor: Colors.white,
              // The payload carries a 64-character fingerprint, so the code is dense. Medium
              // correction keeps it scannable without inflating it further.
              errorCorrectionLevel: QrErrorCorrectLevel.M,
            ),
          ),
        ),
        const SizedBox(height: 16),

        const Text('or enter this code by hand'),
        const SizedBox(height: 4),
        Text(
          window.code,
          style: Theme.of(context).textTheme.displaySmall?.copyWith(
            letterSpacing: 10,
            fontFeatures: const [FontFeature.tabularFigures()],
          ),
        ),
        const SizedBox(height: 12),

        if (expired)
          Column(
            children: [
              const Text('This code has expired.'),
              const SizedBox(height: 8),
              FilledButton.icon(
                onPressed: _begin,
                icon: const Icon(Icons.refresh),
                label: const Text('New code'),
              ),
            ],
          )
        else
          Text(
            'Expires in ${_remaining.inMinutes}:'
            '${(_remaining.inSeconds % 60).toString().padLeft(2, '0')}',
            style: Theme.of(context).textTheme.bodySmall,
          ),
      ],
    );
  }
}
