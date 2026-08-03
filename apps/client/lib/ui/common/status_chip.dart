/// Connection state and live round-trip time.
library;

import 'package:flutter/material.dart';

import '../../domain/session/session_state.dart';

/// A persistent, unobtrusive readout of whether the deck is actually connected.
///
/// The RTT is not decoration: `docs/ARCHITECTURE.md` §7 sets a 25 ms press-to-keystroke budget,
/// and a number on screen is what makes a regression against it noticeable rather than a vague
/// feeling that the deck got worse.
class StatusChip extends StatelessWidget {
  const StatusChip({required this.state, this.showRoundTrip = true, super.key});

  final SessionState state;

  /// From the device's own settings. Off hides the millisecond readout, not the state.
  final bool showRoundTrip;

  @override
  Widget build(BuildContext context) {
    final (colour, label) = switch (state) {
      SessionDisconnected() => (const Color(0xFF4A5568), 'Disconnected'),
      SessionConnecting(:final hostName) => (
        const Color(0xFFB8860B),
        'Connecting to $hostName…',
      ),
      SessionAuthenticating(:final hostName) => (
        const Color(0xFFB8860B),
        'Authenticating with $hostName…',
      ),
      SessionReady(:final hostName, :final roundTripMs) => (
        const Color(0xFF1F8A70),
        roundTripMs == null || !showRoundTrip
            ? hostName
            : '$hostName · ${roundTripMs}ms',
      ),
      // Amber rather than red while a retry is pending: the deck is coming back on its own, and
      // the colour is the difference between "wait" and "go and fix something".
      SessionFailed(:final error, :final willRetry) => (
        willRetry ? const Color(0xFFB8860B) : const Color(0xFFB3422F),
        willRetry ? '${error.message} Retrying…' : error.message,
      ),
    };

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
      decoration: BoxDecoration(
        color: colour.withValues(alpha: 0.18),
        borderRadius: BorderRadius.circular(20),
        border: Border.all(color: colour.withValues(alpha: 0.6)),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Container(
            width: 8,
            height: 8,
            decoration: BoxDecoration(color: colour, shape: BoxShape.circle),
          ),
          const SizedBox(width: 8),
          Flexible(
            child: Text(
              label,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: Theme.of(
                context,
              ).textTheme.labelMedium?.copyWith(color: Colors.white70),
            ),
          ),
        ],
      ),
    );
  }
}
