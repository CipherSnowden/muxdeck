//! The `sysinfo` sampler. `docs/PROTOCOL.md` §4.7, `docs/ENGINE.md` §5.
//!
//! One task samples CPU and memory on an interval and publishes `evt telemetry.update` to the
//! sockets that asked for it. There is no per-socket sampler: sampling is the expensive part and
//! the reading is the same for everybody, so ten subscribed panels cost exactly what one does.

use std::sync::Arc;
use std::time::Duration;

use muxdeck_core::{KnownOp, TelemetryUpdate};
use sysinfo::{MemoryRefreshKind, RefreshKind, System};
use tokio::time::{interval_at, Instant};
use tracing::debug;

use crate::server::EventBus;
use crate::Engine;

/// The floor on `telemetry_interval_ms`.
///
/// `sysinfo` computes CPU percentages by differencing two samples, and below roughly this gap
/// the difference is mostly noise — the documented minimum refresh interval. A client asking for
/// 50 ms would not get faster numbers, only wronger ones.
const MINIMUM_INTERVAL: Duration = Duration::from_millis(200);

/// Runs the sampler until the process ends.
///
/// The interval and the on/off switch are both read from settings on **every tick** rather than
/// captured at startup, because `settings.set` changes them live (`docs/PROTOCOL.md` §4.6) and
/// restarting this task on every settings write would be a lot of machinery for a value that is
/// cheap to re-read.
pub async fn run(engine: Arc<Engine>, events: EventBus) {
    let mut system = System::new_with_specifics(
        RefreshKind::nothing()
            .with_cpu(sysinfo::CpuRefreshKind::nothing().with_cpu_usage())
            .with_memory(MemoryRefreshKind::nothing().with_ram()),
    );

    // The first CPU reading of a fresh `System` is always 0.0 — there is no previous sample to
    // difference against. Priming here means the first event a panel receives is a real number
    // rather than a zero it would draw as an idle machine.
    system.refresh_cpu_usage();
    tokio::time::sleep(MINIMUM_INTERVAL).await;

    let mut tick = interval_at(Instant::now(), MINIMUM_INTERVAL);
    // Ticks are sampling instants, not a queue to catch up on: if the machine stalls, publishing
    // a burst of stale readings is worse than skipping to the current one.
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let mut elapsed = Duration::ZERO;

    loop {
        tick.tick().await;
        elapsed += MINIMUM_INTERVAL;

        let settings = engine.settings();
        if !settings.telemetry_enabled {
            continue;
        }

        let wanted =
            Duration::from_millis(u64::from(settings.telemetry_interval_ms)).max(MINIMUM_INTERVAL);
        if elapsed < wanted {
            continue;
        }
        elapsed = Duration::ZERO;

        // Skip the sample entirely when no socket is connected at all — the common state for a
        // daemon that sits in the tray all day. This checks connections rather than telemetry
        // subscriptions specifically, because that is what the broadcast channel can answer for
        // free; counting subscribers exactly would need bookkeeping on every disconnect path to
        // save a refresh that costs well under a millisecond.
        if !events.has_listeners() {
            continue;
        }

        system.refresh_cpu_usage();
        system.refresh_memory();

        let update = sample(&system);
        debug!(
            cpu_pct = update.cpu_pct,
            ram_pct = update.ram_pct,
            "telemetry"
        );
        events.publish(KnownOp::TelemetryUpdate, &update);
    }
}

/// Turns a refreshed `System` into the wire payload.
///
/// Split out so the arithmetic — particularly the divide-by-zero guard — is testable without a
/// running sampler.
fn sample(system: &System) -> TelemetryUpdate {
    let total = system.total_memory();
    let ram_pct = if total == 0 {
        // Containers and unusual kernels can report zero total memory. Reporting 0% is a
        // readable lie; NaN would render as "NaN%" on the dashboard.
        0.0
    } else {
        system.used_memory() as f64 / total as f64 * 100.0
    };

    TelemetryUpdate {
        ts: now_seconds(),
        cpu_pct: round_to_tenths(f64::from(system.global_cpu_usage())),
        ram_pct: round_to_tenths(ram_pct),
    }
}

/// One decimal place, matching the example payload in `docs/PROTOCOL.md` §4.9.
///
/// Not cosmetic: raw readings carry a dozen meaningless digits, and a dashboard redrawing
/// `14.499999999999998` every second is noise a human has to filter out.
fn round_to_tenths(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readings_are_rounded_to_one_decimal_place() {
        assert_eq!(round_to_tenths(14.499999999999998), 14.5);
        assert_eq!(round_to_tenths(0.0), 0.0);
        assert_eq!(round_to_tenths(100.0), 100.0);
    }

    #[test]
    fn a_real_sample_is_in_range() {
        let mut system = System::new_all();
        system.refresh_cpu_usage();
        std::thread::sleep(MINIMUM_INTERVAL);
        system.refresh_cpu_usage();
        system.refresh_memory();

        let update = sample(&system);
        assert!(
            (0.0..=100.0).contains(&update.cpu_pct),
            "cpu {} is not a percentage",
            update.cpu_pct
        );
        assert!(
            (0.0..=100.0).contains(&update.ram_pct),
            "ram {} is not a percentage",
            update.ram_pct
        );
        assert!(
            update.ts > 1_700_000_000,
            "the timestamp must be a real one"
        );
    }

    #[test]
    fn a_machine_reporting_no_memory_does_not_produce_nan() {
        // `NaN` would serialise as `null` and render as "NaN%"; 0.0 is at least readable.
        let system = System::new();
        let update = sample(&system);
        assert!(update.ram_pct.is_finite());
    }
}
