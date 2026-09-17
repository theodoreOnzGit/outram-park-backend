//! Simulated-clock instants — the port of DWSIM's `Date`-valued dynamics clock.
//!
//! # Attribution
//!
//! Pure-Rust port of part of **DWSIM** (<https://dwsim.org>), upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`), GPL-3.0.
//! Upstream copyright: 2020 Daniel Wagner O. de Medeiros and the DWSIM
//! contributors. This port is GPL-3.0-only. Independent OUTRAM PARK fork, **not**
//! the official DWSIM software.
//!
//! Sources:
//!
//! - `DWSIM.DynamicsManager/Integrator.vb:39` — `CurrentTime As Date = New Date()`,
//!   i.e. the simulated clock starts at `DateTime.MinValue`, not at a wall-clock
//!   date.
//! - `DWSIM.DynamicsManager/Event.vb:30` — `TimeStamp As Date = DateTime.MinValue`.
//! - `DWSIM/Forms/FlowsheetComponents/FormDynamicsIntegratorControl.vb:135`,
//!   `:150` — monitored-variable samples are keyed by `tstamp.Ticks` (a .NET
//!   tick is 100 ns).
//! - `DWSIM.DynamicsManager/Manager.vb:119` — `New TimeSpan(item.Key).TotalMilliseconds / 1000.0`,
//!   confirming the key is a tick count that converts back to seconds.
//! - `FormDynamicsIntegratorControl.vb:512-516` — the clock advances (or, for a
//!   backwards single step, retreats) by `interval` seconds each step.
//!
//! # Why a tick count and not a `uom` `Time`
//!
//! The simulated clock is used as a **map key** (historian entries and monitored
//! variable samples), so it must be `Ord` + `Eq` + `Hash`. `uom`'s `Time` wraps
//! `f64`, which is none of those. An integer tick count reproduces upstream's
//! `Date`/`Ticks` key exactly, orders identically, and converts losslessly
//! enough for the seconds-scale steps dynamics uses. Public accessors are
//! `uom`-typed ([`SimInstant::elapsed`], [`SimInstant::from_time`]).
//!
//! # Excluded DWSIM behavior
//!
//! - **Calendar semantics.** Upstream's `Date` carries a full Gregorian
//!   calendar; only the offset from `DateTime.MinValue` is meaningful for a
//!   dynamic simulation, so this port keeps the offset alone. Formatting
//!   (`ToString("c")`, `Integrator.vb`'s XML round-trip) is not ported.
//! - **`ArgumentOutOfRangeException` on underflow.** `.NET`'s
//!   `DateTime.AddSeconds` throws when the result falls below `DateTime.MinValue`;
//!   a backwards single step (`FormDynamicsIntegratorControl.vb:513`) can do
//!   exactly that at the start of a run. [`SimInstant::add_seconds`] **saturates
//!   at zero** instead — see its own doc comment.

use uom::si::f64::Time;
use uom::si::time::second;

/// .NET ticks per second — one tick is 100 ns (`TimeSpan.TicksPerSecond`).
pub const TICKS_PER_SECOND: i64 = 10_000_000;

/// A point on the **simulated** clock, measured in 100-ns ticks from the start
/// of the simulation.
///
/// Zero is DWSIM's `DateTime.MinValue`, the value a fresh
/// [`crate::dynamics::integrator::Integrator`] starts at (Integrator.vb:39) and
/// the value the run loop resets to at the beginning of a non-restarting run
/// (FormDynamicsIntegratorControl.vb:384).
///
/// This is **simulated** time, never wall-clock time. Wall-clock pacing for
/// real-time mode uses [`std::time::Instant`] and lives in
/// [`crate::dynamics::runner`].
///
/// Quantity: time \[s\], stored as an integer count of 100-ns ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SimInstant {
    ticks: i64,
}

impl SimInstant {
    /// The start of the simulation — upstream's `DateTime.MinValue`.
    pub const ZERO: SimInstant = SimInstant { ticks: 0 };

    /// Build an instant from a raw 100-ns tick count (upstream's
    /// `Date.Ticks`). Negative counts are representable but never produced by
    /// the run loop, which saturates at zero.
    #[must_use]
    pub const fn from_ticks(ticks: i64) -> Self {
        SimInstant { ticks }
    }

    /// The raw 100-ns tick count — the key upstream uses for monitored-variable
    /// samples (`FormDynamicsIntegratorControl.vb:135`).
    #[must_use]
    pub const fn ticks(self) -> i64 {
        self.ticks
    }

    /// Build an instant from a number of seconds since simulation start.
    ///
    /// Rounds to the nearest tick (100 ns). Valid range is roughly
    /// ±29 000 years; values beyond that saturate.
    #[must_use]
    pub fn from_seconds(seconds: f64) -> Self {
        let ticks = (seconds * TICKS_PER_SECOND as f64).round();
        if ticks >= i64::MAX as f64 {
            SimInstant { ticks: i64::MAX }
        } else if ticks <= i64::MIN as f64 {
            SimInstant { ticks: i64::MIN }
        } else {
            SimInstant {
                ticks: ticks as i64,
            }
        }
    }

    /// Seconds since simulation start \[s\], as a plain `f64`.
    #[must_use]
    pub fn seconds(self) -> f64 {
        self.ticks as f64 / TICKS_PER_SECOND as f64
    }

    /// Milliseconds since simulation start \[ms\] — the unit upstream uses when
    /// it computes event-transition spans (`Manager.vb:291-297`,
    /// `TotalMilliseconds`).
    #[must_use]
    pub fn milliseconds(self) -> f64 {
        self.ticks as f64 / 10_000.0
    }

    /// Build an instant from a `uom` [`Time`] measured from simulation start.
    #[must_use]
    pub fn from_time(t: Time) -> Self {
        Self::from_seconds(t.get::<second>())
    }

    /// Elapsed simulated time since simulation start, `uom`-typed \[s\].
    #[must_use]
    pub fn elapsed(self) -> Time {
        Time::new::<second>(self.seconds())
    }

    /// Advance (or, with a negative argument, retreat) the clock by `seconds`.
    ///
    /// Ports `integrator.CurrentTime.AddSeconds(±interval)`
    /// (FormDynamicsIntegratorControl.vb:512-516).
    ///
    /// **Divergence:** .NET throws `ArgumentOutOfRangeException` if the result
    /// falls below `DateTime.MinValue`; a backwards step taken near the start of
    /// a run does exactly that. This port **saturates at zero**, so a backwards
    /// step from the start of the simulation stays at the start rather than
    /// aborting the run.
    #[must_use]
    pub fn add_seconds(self, seconds: f64) -> Self {
        let next = self.seconds() + seconds;
        if next <= 0.0 {
            SimInstant::ZERO
        } else {
            Self::from_seconds(next)
        }
    }

    /// Signed difference `self - other`, in milliseconds \[ms\] — the quantity
    /// upstream calls `span` / `dt` when interpolating an event transition
    /// (`Manager.vb:291-299`).
    #[must_use]
    pub fn millis_since(self, other: SimInstant) -> f64 {
        (self.ticks - other.ticks) as f64 / 10_000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_is_the_start_of_the_simulation() {
        assert_eq!(SimInstant::ZERO.ticks(), 0);
        assert_eq!(SimInstant::default(), SimInstant::ZERO);
        assert!((SimInstant::ZERO.seconds() - 0.0).abs() < 1e-12);
    }

    #[test]
    fn seconds_round_trip_through_ticks() {
        let t = SimInstant::from_seconds(5.0);
        assert_eq!(t.ticks(), 5 * TICKS_PER_SECOND);
        assert!((t.seconds() - 5.0).abs() < 1e-12);
        assert!((t.milliseconds() - 5000.0).abs() < 1e-9);
    }

    #[test]
    fn add_seconds_saturates_at_zero_instead_of_throwing() {
        // Upstream would raise ArgumentOutOfRangeException here.
        let t = SimInstant::from_seconds(1.0).add_seconds(-10.0);
        assert_eq!(t, SimInstant::ZERO);
    }

    #[test]
    fn instants_order_like_dotnet_dates() {
        let a = SimInstant::from_seconds(1.0);
        let b = SimInstant::from_seconds(2.0);
        assert!(a < b);
        assert!((b.millis_since(a) - 1000.0).abs() < 1e-9);
        assert!((a.millis_since(b) + 1000.0).abs() < 1e-9);
    }

    #[test]
    fn uom_round_trip() {
        let t = SimInstant::from_time(Time::new::<second>(12.5));
        assert!((t.elapsed().get::<second>() - 12.5).abs() < 1e-12);
    }
}
