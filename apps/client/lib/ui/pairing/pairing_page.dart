/// QR scanning, with manual entry beside it.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:mobile_scanner/mobile_scanner.dart';

import '../../domain/pairing/pairing_controller.dart';
import '../../providers.dart';

class PairingPage extends ConsumerStatefulWidget {
  const PairingPage({super.key});

  @override
  ConsumerState<PairingPage> createState() => _PairingPageState();
}

class _PairingPageState extends ConsumerState<PairingPage> {
  late final MobileScannerController _scanner = MobileScannerController(
    formats: const [BarcodeFormat.qrCode],
    // The camera sees the same code many times a second; without this every frame would start
    // another pairing attempt.
    detectionSpeed: DetectionSpeed.noDuplicates,
  );

  var _manual = false;
  var _handled = false;

  final _addressController = TextEditingController();
  final _codeController = TextEditingController();

  @override
  void dispose() {
    // This widget owns the controller, so this widget disposes it. A controller passed to
    // MobileScanner is started and stopped by the widget but never disposed by it.
    _scanner.dispose();
    _addressController.dispose();
    _codeController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final pairing = ref.watch(pairingProvider);

    ref.listen(pairingProvider, (previous, next) {
      if (next is PairingSucceeded && mounted) {
        Navigator.of(context).pop();
      }
      if (next is PairingFailed && mounted) {
        // Re-arm: a rejected code is usually a typo, and the user's next move is to try again.
        _handled = false;
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(next.error.message),
            backgroundColor: const Color(0xFFB3422F),
          ),
        );
      }
    });

    return Scaffold(
      backgroundColor: const Color(0xFF12141A),
      appBar: AppBar(
        backgroundColor: const Color(0xFF1A1D26),
        foregroundColor: Colors.white,
        title: Text(_manual ? 'Enter address' : 'Scan pairing code'),
        actions: [
          TextButton(
            onPressed: () => setState(() => _manual = !_manual),
            child: Text(
              _manual ? 'Scan QR' : 'Enter manually',
              style: const TextStyle(color: Colors.white70),
            ),
          ),
        ],
      ),
      body: pairing is PairingInProgress
          ? const Center(child: CircularProgressIndicator())
          : _manual
          ? _manualEntry()
          : _scannerView(),
    );
  }

  Widget _scannerView() {
    return Column(
      children: [
        Expanded(
          child: MobileScanner(
            controller: _scanner,
            onDetect: _onDetect,
            errorBuilder: (context, error) => _ScannerUnavailable(
              onManual: () => setState(() => _manual = true),
            ),
          ),
        ),
        const Padding(
          padding: EdgeInsets.all(20),
          child: Text(
            'On your computer, open MuxDeck and choose "Pair a device".',
            textAlign: TextAlign.center,
            style: TextStyle(color: Colors.white54),
          ),
        ),
      ],
    );
  }

  void _onDetect(BarcodeCapture capture) {
    if (_handled) return;

    final raw = capture.barcodes.firstOrNull?.rawValue;
    if (raw == null) return;

    _handled = true;
    ref.read(pairingProvider.notifier).pairFromQr(raw);
  }

  Widget _manualEntry() {
    return SingleChildScrollView(
      padding: const EdgeInsets.all(20),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          const Text(
            'Enter the address and 6-digit code shown on your computer.',
            style: TextStyle(color: Colors.white70),
          ),
          const SizedBox(height: 20),
          TextField(
            controller: _addressController,
            style: const TextStyle(color: Colors.white),
            keyboardType: TextInputType.url,
            autocorrect: false,
            decoration: _fieldDecoration('Address', '192.168.1.42:47654'),
          ),
          const SizedBox(height: 12),
          TextField(
            controller: _codeController,
            style: const TextStyle(color: Colors.white, letterSpacing: 8),
            keyboardType: TextInputType.number,
            maxLength: 6,
            decoration: _fieldDecoration('Pairing code', '402913'),
          ),
          const SizedBox(height: 20),
          FilledButton(onPressed: _submitManual, child: const Text('Pair')),
          const SizedBox(height: 24),
          const Text(
            'Scanning the QR code is more secure: it carries the host\'s certificate '
            'fingerprint, so this device can verify the computer it connects to. Entering '
            'an address trusts whatever answers at that address the first time.',
            style: TextStyle(color: Colors.white38, fontSize: 12),
          ),
        ],
      ),
    );
  }

  InputDecoration _fieldDecoration(String label, String hint) => InputDecoration(
    labelText: label,
    hintText: hint,
    labelStyle: const TextStyle(color: Colors.white54),
    hintStyle: const TextStyle(color: Colors.white24),
    counterStyle: const TextStyle(color: Colors.white38),
    enabledBorder: const OutlineInputBorder(
      borderSide: BorderSide(color: Colors.white24),
    ),
    focusedBorder: const OutlineInputBorder(
      borderSide: BorderSide(color: Color(0xFF2D6CDF)),
    ),
  );

  void _submitManual() {
    final address = _addressController.text.trim();
    final code = _codeController.text.trim();

    if (!address.contains(':') || code.length != 6) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text('Enter an address as host:port, and the full 6-digit code.'),
        ),
      );
      return;
    }

    ref.read(pairingProvider.notifier).pairManually(address: address, code: code);
  }
}

class _ScannerUnavailable extends StatelessWidget {
  const _ScannerUnavailable({required this.onManual});

  final VoidCallback onManual;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(32),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Icon(Icons.no_photography, size: 48, color: Colors.white38),
            const SizedBox(height: 16),
            const Text(
              'The camera is unavailable',
              style: TextStyle(color: Colors.white, fontSize: 18),
            ),
            const SizedBox(height: 8),
            const Text(
              'Grant camera permission, or enter the address manually.',
              textAlign: TextAlign.center,
              style: TextStyle(color: Colors.white54),
            ),
            const SizedBox(height: 20),
            FilledButton(onPressed: onManual, child: const Text('Enter manually')),
          ],
        ),
      ),
    );
  }
}
