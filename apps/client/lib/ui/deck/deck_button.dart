/// One button on the deck.
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:muxdeck_icons/muxdeck_icons.dart';
import 'package:muxdeck_protocol/muxdeck_protocol.dart';

/// A deck key.
///
/// Fires on **pointer down**, not on tap-up. This is not a micro-optimisation — it is the whole
/// difference between feeling like hardware and feeling like a web page, and it is why
/// `docs/CLIENT.md` §6 calls it out explicitly.
class DeckButton extends StatefulWidget {
  const DeckButton({
    required this.button,
    required this.onPressed,
    this.enabled = true,
    super.key,
  });

  final Button button;

  /// Fires the action. The returned future completing with an error is what turns the button
  /// red — the press itself is never delayed by waiting on it.
  final Future<void> Function() onPressed;

  /// False when the host cannot perform this action, from the `capabilities` block of the
  /// `Ready` payload. A disabled button is visibly unavailable rather than failing at press
  /// time.
  final bool enabled;

  @override
  State<DeckButton> createState() => _DeckButtonState();
}

/// How long a refused press stays red.
const _failureFlash = Duration(milliseconds: 600);

class _DeckButtonState extends State<DeckButton> {
  var _pressed = false;
  var _failed = false;
  Timer? _flashTimer;

  @override
  void dispose() {
    _flashTimer?.cancel();
    super.dispose();
  }

  void _handleDown(PointerDownEvent _) {
    if (!widget.enabled) return;

    // Haptic first, before the network send. It confirms the touch registered, not that the
    // action landed — so it must not wait on a round trip. `docs/CLIENT.md` §6.
    unawaited(_haptic());

    setState(() {
      _pressed = true;
      _failed = false;
    });
    _flashTimer?.cancel();

    // Fired, then watched. The send has already happened by the time this future is awaited, so
    // nothing about the press waits on the network — but a refusal still has to be visible, or
    // a button whose action the host rejected looks identical to one that worked
    // (`docs/CLIENT.md` §6).
    unawaited(widget.onPressed().catchError((Object _) => _flashFailure()));
  }

  void _flashFailure() {
    if (!mounted) return;
    setState(() => _failed = true);
    _flashTimer?.cancel();
    _flashTimer = Timer(_failureFlash, () {
      if (mounted) setState(() => _failed = false);
    });
  }

  Future<void> _haptic() => switch (widget.button.haptic) {
    Haptic.none => Future<void>.value(),
    Haptic.light => HapticFeedback.lightImpact(),
    Haptic.medium => HapticFeedback.mediumImpact(),
    Haptic.heavy => HapticFeedback.heavyImpact(),
  };

  void _handleUp([PointerEvent? _]) {
    if (_pressed) setState(() => _pressed = false);
  }

  /// `#RRGGBB` from the profile, falling back rather than throwing on a malformed value.
  Color get _colour {
    final hex = widget.button.color.replaceFirst('#', '');
    final value = int.tryParse(hex, radix: 16);
    if (value == null || hex.length != 6) return const Color(0xFF2D6CDF);
    return Color(0xFF000000 | value);
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final button = widget.button;
    final base = switch ((widget.enabled, _failed)) {
      (_, true) => const Color(0xFFB3422F),
      (true, _) => _colour,
      (false, _) => theme.disabledColor.withValues(alpha: 0.25),
    };

    return Listener(
      onPointerDown: _handleDown,
      onPointerUp: _handleUp,
      onPointerCancel: _handleUp,
      child: AnimatedScale(
        // Optimistic: the press shows immediately rather than waiting to hear back.
        scale: _pressed ? 0.94 : 1.0,
        duration: const Duration(milliseconds: 70),
        child: Semantics(
          button: true,
          enabled: widget.enabled,
          label: button.label,
          child: DecoratedBox(
            decoration: BoxDecoration(
              color: base,
              borderRadius: BorderRadius.circular(14),
              border: Border.all(
                color: Colors.white.withValues(alpha: _pressed ? 0.45 : 0.12),
                width: _pressed ? 2 : 1,
              ),
            ),
            child: LayoutBuilder(
              builder: (context, constraints) {
                // Scale with the cell so a 5x8 grid on a phone stays legible and a 3x5 on a
                // tablet does not look sparse.
                final iconSize = (constraints.biggest.shortestSide * 0.34)
                    .clamp(16.0, 44.0);
                return Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    Icon(
                      // From the curated map, so an unknown name draws a dot rather than a
                      // blank square. See muxdeck_icons.
                      iconFor(button.icon),
                      size: iconSize,
                      color: Colors.white.withValues(
                        alpha: widget.enabled ? 0.95 : 0.5,
                      ),
                    ),
                    const SizedBox(height: 6),
                    Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 6),
                      child: Text(
                        button.label,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        textAlign: TextAlign.center,
                        style: theme.textTheme.labelMedium?.copyWith(
                          color: Colors.white.withValues(
                            alpha: widget.enabled ? 0.95 : 0.5,
                          ),
                        ),
                      ),
                    ),
                  ],
                );
              },
            ),
          ),
        ),
      ),
    );
  }
}
