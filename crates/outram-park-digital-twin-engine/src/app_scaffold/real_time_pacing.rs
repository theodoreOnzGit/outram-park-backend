//! Real-time pacing for fixed-timestep simulation loops, in this workspace's
//! house pattern.
//!
//! # Where the pattern comes from
//!
//! This is the pacing scheme the maintainer wrote for the `fhr_sim_v2` example
//! (`examples/fhr_sim_v2/app/prke_backend/mod.rs` and
//! `app/thermal_hydraulics_backend/mod.rs`), lifted into the shared scaffold so
//! the simulators do not each carry a hand-rolled copy. Its shape:
//!
//! 1. Keep a **cumulative** comparison between the plant clock and wall clock,
//!    both measured from the start of the loop — not a per-tick one.
//! 2. If the plant clock is **at or ahead of** wall clock, sleep whatever is
//!    left of this tick's budget after the work.
//! 3. If the plant clock is **behind** wall clock, sleep only a cursory amount
//!    ([`CATCH_UP_SLEEP`]) and run the next tick straight away, so the loop
//!    works off the deficit.
//!
//! The cumulative comparison is the part worth keeping. A purely per-tick
//! deadline loses every millisecond an overrunning tick costs and never gets it
//! back; comparing totals means a slow patch is repaid by the ticks that follow
//! it, and the loop converges back on 1:1 instead of drifting.
//!
//! The one deliberate change from the original is the cursory sleep. The
//! original does not sleep at all on the behind-real-time branch, which spins a
//! physics thread flat out against the GUI thread; `fhr_sim_v2` already uses a
//! 5 microsecond token sleep on its fast-forward branch for exactly this
//! reason, so that value is reused here.
//!
//! # The defect this replaces
//!
//! A loop that advances a fixed slice of plant time and then sleeps a **fixed**
//! wall period cannot run in real time. Its wall period is `compute + sleep`,
//! so with `sleep` fixed at the plant slice the achieved ratio is
//!
//! ```text
//! simulated / (compute + simulated)
//! ```
//!
//! which is below 1.0 for any nonzero compute cost and can never reach it. No
//! choice of the sleep constant fixes it, because the compute cost is never
//! subtracted. That was `htgr_sim_v1`'s physics thread (kopi-beans `op-v5zb`).
//!
//! # The arithmetic is the bug-prone part
//!
//! Two mistakes recur in hand-rolled versions of this, both recorded as
//! kopi-beans `op-xvye`, and both are impossible here by construction (see
//! [`pace_tick`]):
//!
//! - **Sign collapse.** Computing the remaining budget as
//!   `(budget_us - compute_us).round().abs()` turns an *overrun* into a
//!   positive sleep, so a tick that blew its budget by 50 ms sleeps another
//!   50 ms on top of it. [`Duration`] subtraction here is checked, and an
//!   overrun yields [`Duration::ZERO`] plus a reported
//!   [`TickPacing::overrun`].
//! - **Unsigned underflow.** Computing `Duration::from_micros(remaining - 1)`
//!   on a `u64` *before* checking `remaining > 1` wraps to `u64::MAX` when the
//!   remainder is zero — a ~584 000-year sleep under the mandatory release
//!   profile, and a panic in debug. Nothing here converts a duration to an
//!   integer and subtracts from it.
//!
//! # Over-budget policy: fall behind, work it off, and say so
//!
//! When the work does not fit in the budget, the plant clock falls behind wall
//! clock and the loop stops sleeping until it has caught up. Simulated time is
//! never skipped and the timestep is never grown — the first would fabricate
//! plant state that was never computed, and the second would change the
//! integration of a stiffly coupled plant, which pacing has no business doing.
//! If the compute cost is *persistently* over budget the deficit grows without
//! bound; [`RealTimePacer::is_behind_real_time`] and
//! [`RealTimePacer::real_time_deficit`] exist so that shows on screen instead
//! of being absorbed silently.
//!
//! It will never run **faster** than real time: once the plant clock is ahead,
//! the loop sleeps out the rest of the budget.

use std::time::Duration;

use uom::si::f64::Time;
use uom::si::time::second;

/// Token sleep taken on a tick that is already behind real time.
///
/// Long enough to yield the CPU so the GUI thread is not starved by a physics
/// thread spinning to catch up, short enough not to be part of the pacing.
/// Matches the fast-forward token sleep in `fhr_sim_v2`.
pub const CATCH_UP_SLEEP: Duration = Duration::from_micros(5);

/// How far the plant clock may fall behind wall clock before the loop is
/// reported as behind real time.
///
/// A quarter of a second: past ordinary scheduling jitter, and about the point
/// at which a person watching a transient would notice the plant lagging their
/// slider.
pub const BEHIND_REAL_TIME_DEFICIT: Duration = Duration::from_millis(250);

/// What one tick's pacing arithmetic decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickPacing {
    /// How long to sleep before starting the next tick.
    ///
    /// [`Duration::ZERO`] when the tick's work consumed the whole budget, and
    /// [`CATCH_UP_SLEEP`] when the loop is behind real time and working off a
    /// deficit.
    pub sleep_for: Duration,
    /// By how much the tick's work exceeded its budget.
    ///
    /// [`Duration::ZERO`] whenever the work fitted. This is the quantity a
    /// sign-collapsing implementation mistakes for extra sleep.
    pub overrun: Duration,
    /// Whether the plant clock was at or ahead of wall clock at the end of this
    /// tick — the condition that decides whether the loop sleeps out its budget
    /// or hurries on.
    pub ahead_of_real_time: bool,
}

impl TickPacing {
    /// Whether this tick's work exceeded its budget.
    pub fn is_over_budget(&self) -> bool {
        !self.overrun.is_zero()
    }
}

/// Sleep budget for one tick, given the whole-tick wall `budget`, how long the
/// tick's work took, and whether the plant clock is at or ahead of wall clock.
///
/// The whole of the pacing arithmetic, kept as a pure function of two durations
/// and a flag so it can be exercised with synthetic timings rather than a real
/// clock.
///
/// - Ahead of real time, work inside the budget: sleep the remainder.
/// - Ahead of real time, work exactly on or past the budget: sleep nothing, and
///   report the overrun — never a negative-turned-positive sleep, and never an
///   unsigned wrap.
/// - Behind real time: sleep [`CATCH_UP_SLEEP`] regardless, and press on.
pub fn pace_tick(budget: Duration, compute: Duration, ahead_of_real_time: bool) -> TickPacing {
    // `checked_sub` returns `None` exactly when `compute > budget`, so the
    // subtraction in the `None` arm is the safe direction and cannot wrap.
    let (remaining, overrun) = match budget.checked_sub(compute) {
        Some(remaining) => (remaining, Duration::ZERO),
        None => (Duration::ZERO, compute - budget),
    };
    TickPacing {
        sleep_for: if ahead_of_real_time {
            remaining
        } else {
            CATCH_UP_SLEEP
        },
        overrun,
        ahead_of_real_time,
    }
}

/// Paces a fixed-timestep simulation loop against wall clock, and measures how
/// well it is keeping up.
///
/// One instance per simulation thread. The loop shape it expects is:
///
/// ```no_run
/// # use std::thread;
/// # use std::time::{Duration, Instant};
/// # use uom::si::f64::Time;
/// # use uom::si::time::second;
/// # use outram_park_digital_twin_engine::app_scaffold::RealTimePacer;
/// // 10 ms of plant time per tick, paced to 10 ms of wall clock.
/// let mut pacer = RealTimePacer::new(Time::new::<second>(0.010), Duration::from_millis(10));
/// let loop_start = Instant::now();
/// loop {
///     let tick_start = Instant::now();
///
///     // ... advance the plant by one tick's worth of simulated time ...
///
///     let pacing = pacer.pace(tick_start.elapsed(), loop_start.elapsed());
///     thread::sleep(pacing.sleep_for);
///
///     if pacer.is_behind_real_time() {
///         // publish the shortfall so the GUI can say so
///     }
/// }
/// ```
///
/// Both elapsed times are the caller's to measure, which keeps this type free
/// of a clock and therefore testable with synthetic sequences: `tick_start`
/// gives the **work**, which the deadline is computed from, and `loop_start`
/// gives **wall clock since the loop began**, which the cumulative comparison
/// is made against.
#[derive(Debug, Clone)]
pub struct RealTimePacer {
    /// Simulated time advanced per tick.
    simulated_per_tick: Duration,
    /// Wall-clock budget for one tick, work and sleep together.
    budget: Duration,
    /// Simulated time advanced since the loop began.
    simulated_total: Duration,
    /// Wall clock since the loop began, as of the last [`Self::pace`] call.
    wall_total: Duration,
    /// Whether [`Self::pace`] has been called at all.
    started: bool,
}

impl RealTimePacer {
    /// A pacer that advances `simulated_per_tick` of plant time per tick and
    /// gives each tick `budget` of wall clock to do it in.
    ///
    /// For 1:1 real time, pass the same interval twice. A `budget` longer than
    /// `simulated_per_tick` deliberately runs slow motion; a shorter one asks
    /// for fast-forward and is honoured only as far as the compute allows.
    ///
    /// A negative or non-finite `simulated_per_tick` is treated as zero rather
    /// than panicking ([`Duration::from_secs_f64`] would panic), because it can
    /// only come from a mis-set configuration constant and killing a simulator
    /// thread over it helps nobody — it shows up instead as a real-time ratio
    /// of zero.
    pub fn new(simulated_per_tick: Time, budget: Duration) -> Self {
        let seconds = simulated_per_tick.get::<second>();
        let seconds = if seconds.is_finite() && seconds > 0.0 {
            seconds
        } else {
            0.0
        };
        Self {
            simulated_per_tick: Duration::from_secs_f64(seconds),
            budget,
            simulated_total: Duration::ZERO,
            wall_total: Duration::ZERO,
            started: false,
        }
    }

    /// The wall-clock budget one tick is given.
    pub fn budget(&self) -> Duration {
        self.budget
    }

    /// The simulated time one tick advances.
    pub fn simulated_per_tick(&self) -> Duration {
        self.simulated_per_tick
    }

    /// Advance the plant clock by one tick and decide how long to sleep.
    ///
    /// Call once per tick, after the tick's work. `compute` is how long that
    /// work took; `wall_elapsed` is wall clock since the loop began.
    pub fn pace(&mut self, compute: Duration, wall_elapsed: Duration) -> TickPacing {
        self.simulated_total += self.simulated_per_tick;
        self.wall_total = wall_elapsed;
        self.started = true;
        pace_tick(self.budget, compute, self.simulated_total >= wall_elapsed)
    }

    /// Simulated time advanced since the loop began.
    pub fn simulated_total(&self) -> Duration {
        self.simulated_total
    }

    /// How far the plant clock is behind wall clock.
    ///
    /// [`Duration::ZERO`] when the plant clock is at or ahead of wall clock;
    /// this is a deficit, never a credit.
    pub fn real_time_deficit(&self) -> Duration {
        self.wall_total.saturating_sub(self.simulated_total)
    }

    /// Simulated seconds advanced per wall-clock second since the loop began.
    ///
    /// `1.0` is real time, `0.5` is half speed. `None` before the first tick
    /// and while no wall time has elapsed — reporting a ratio before one has
    /// been measured is the silent misinformation this type exists to prevent.
    ///
    /// Cumulative, so it is steady but slow to react: a slowdown that has just
    /// started shows more sharply in [`Self::real_time_deficit`], which is why
    /// both are exposed.
    pub fn measured_real_time_ratio(&self) -> Option<f64> {
        if !self.started || self.wall_total.is_zero() {
            return None;
        }
        Some(self.simulated_total.as_secs_f64() / self.wall_total.as_secs_f64())
    }

    /// Whether the plant clock has fallen measurably behind wall clock.
    ///
    /// `false` for ordinary jitter — see [`BEHIND_REAL_TIME_DEFICIT`].
    pub fn is_behind_real_time(&self) -> bool {
        self.real_time_deficit() > BEHIND_REAL_TIME_DEFICIT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(v: u64) -> Duration {
        Duration::from_millis(v)
    }

    fn pacer_10ms() -> RealTimePacer {
        RealTimePacer::new(Time::new::<second>(0.010), ms(10))
    }

    /// V&V: the deadline arithmetic subtracts the compute cost, and does not
    /// collapse the sign of an overrun.
    ///
    /// **Methodology.** Feed [`pace_tick`] a synthetic sequence spanning the
    /// three regimes against a 10 ms budget, with the loop ahead of real time
    /// so the sleep branch is the deadline one: work comfortably inside the
    /// budget, exactly on it, and past it. The overrun case is the regression
    /// test for the sign-collapse defect in kopi-beans `op-xvye`, where
    /// `(budget - compute).round().abs()` turned an overrun into extra sleep.
    ///
    /// **Results (2026-08-13).** 3 ms of work in a 10 ms budget slept 7 ms with
    /// 0 ms overrun; 10 ms of work slept 0 ms with 0 ms overrun; 25 ms of work
    /// slept **0 ms** and reported a 15 ms overrun. The defective formula would
    /// have produced `|10 - 25| = 15` and slept 14 ms after an already-blown
    /// budget, turning a 25 ms tick into a 39 ms one. Interpretation: an
    /// overrun now costs only the overrun.
    #[test]
    fn an_overrun_sleeps_nothing_and_is_reported_as_an_overrun() {
        let inside = pace_tick(ms(10), ms(3), true);
        assert_eq!(inside.sleep_for, ms(7));
        assert_eq!(inside.overrun, Duration::ZERO);

        let exact = pace_tick(ms(10), ms(10), true);
        assert_eq!(exact.sleep_for, Duration::ZERO);
        assert_eq!(exact.overrun, Duration::ZERO);

        let over = pace_tick(ms(10), ms(25), true);
        assert_eq!(over.sleep_for, Duration::ZERO);
        assert_eq!(over.overrun, ms(15));
        assert!(over.is_over_budget());
    }

    /// V&V: no unsigned underflow anywhere near the budget boundary.
    ///
    /// **Methodology.** The second defect in `op-xvye` is
    /// `Duration::from_micros(remaining - 1)` evaluated **before** the
    /// `remaining > 1` guard, which wraps `0u64 - 1` to `u64::MAX` under the
    /// mandatory release profile and panics in debug. Sweep the compute cost
    /// across the boundary in 1 ms steps, from 3 ms under the budget to 3 ms
    /// over, on both the ahead-of-real-time and behind-real-time branches, and
    /// require every sleep to stay bounded and every overrun to equal the true
    /// shortfall.
    ///
    /// **Results (2026-08-13).** Ahead of real time, computes of 7, 8, 9, 10,
    /// 11, 12, 13 ms against a 10 ms budget gave sleeps of 3, 2, 1, 0, 0, 0,
    /// 0 ms and overruns of 0, 0, 0, 0, 1, 2, 3 ms. Behind real time, all seven
    /// gave the 5 microsecond catch-up sleep with the same overruns. No sleep
    /// exceeded the 10 ms budget; the wrapped value would have been
    /// 18 446 744 073 709 551 615 microseconds, about 584 000 years.
    /// Interpretation: the boundary is safe in both directions and on both
    /// branches.
    #[test]
    fn the_budget_boundary_does_not_wrap() {
        let budget = ms(10);
        for ahead in [true, false] {
            for compute_ms in 7..=13u64 {
                let pacing = pace_tick(budget, ms(compute_ms), ahead);
                let expected_overrun = ms(compute_ms).saturating_sub(budget);
                assert_eq!(pacing.overrun, expected_overrun);
                if ahead {
                    assert_eq!(pacing.sleep_for, budget.saturating_sub(ms(compute_ms)));
                } else {
                    assert_eq!(pacing.sleep_for, CATCH_UP_SLEEP);
                }
                assert!(
                    pacing.sleep_for <= budget,
                    "compute {compute_ms} ms, ahead {ahead}: slept {:?}",
                    pacing.sleep_for
                );
            }
        }
    }

    /// V&V: a loop that keeps up reads 1.0 and sleeps out its budget; a loop
    /// that cannot keep up reads the shortfall and stops sleeping.
    ///
    /// **Methodology.** Drive a [`RealTimePacer`] configured for 10 ms of plant
    /// time per 10 ms tick with two synthetic sequences, supplying the wall
    /// clock by hand so the result is deterministic. First, 200 ticks whose
    /// wall clock advances exactly 10 ms per tick with 3 ms of compute — a loop
    /// that keeps up. Second, 200 ticks whose wall clock advances 25 ms per
    /// tick with 25 ms of compute — compute overrunning the budget by 15 ms
    /// every tick. Require the sleep decision, the cumulative ratio and the
    /// deficit in each.
    ///
    /// **Results (2026-08-13).** Keeping up: every tick was ahead of real time
    /// and slept 7 ms; after 200 ticks the ratio was 1.000000, the deficit
    /// 0 ms, and the loop was not flagged behind. Overrunning: every tick after
    /// the first was behind real time and slept only the 5 microsecond
    /// catch-up sleep; after 200 ticks the plant clock read 2.000 s against
    /// 5.000 s of wall clock — a ratio of 0.400000 and a 3.000 s deficit — and
    /// the loop was flagged behind. Interpretation: the shortfall is measured
    /// and reported rather than silently absorbed, which is the complaint in
    /// `op-v5zb`.
    #[test]
    fn keeping_up_sleeps_out_the_budget_and_falling_behind_does_not() {
        let mut keeping_up = pacer_10ms();
        assert_eq!(keeping_up.measured_real_time_ratio(), None);
        let mut wall = Duration::ZERO;
        for _ in 0..200 {
            wall += ms(10);
            let pacing = keeping_up.pace(ms(3), wall);
            assert!(pacing.ahead_of_real_time);
            assert_eq!(pacing.sleep_for, ms(7));
        }
        assert_eq!(keeping_up.real_time_deficit(), Duration::ZERO);
        let ratio = keeping_up.measured_real_time_ratio().expect("measured");
        assert!((ratio - 1.0).abs() < 1e-9, "ratio was {ratio}");
        assert!(!keeping_up.is_behind_real_time());

        let mut behind = pacer_10ms();
        let mut wall = Duration::ZERO;
        for _ in 0..200 {
            wall += ms(25);
            let pacing = behind.pace(ms(25), wall);
            assert!(!pacing.ahead_of_real_time);
            assert_eq!(pacing.sleep_for, CATCH_UP_SLEEP);
            assert_eq!(pacing.overrun, ms(15));
        }
        assert_eq!(behind.simulated_total(), ms(2000));
        assert_eq!(behind.real_time_deficit(), ms(3000));
        let ratio = behind.measured_real_time_ratio().expect("measured");
        assert!((ratio - 0.4).abs() < 1e-9, "ratio was {ratio}");
        assert!(behind.is_behind_real_time());
    }

    /// V&V: a burst of slow ticks is worked off, not lost.
    ///
    /// **Methodology.** This is the property the cumulative comparison buys
    /// over a purely per-tick deadline, and the reason for copying
    /// `fhr_sim_v2`'s structure rather than inventing one. Run 10 ticks that
    /// each take 60 ms of wall clock against a 10 ms budget (a 500 ms stall,
    /// as a backgrounded window or a slow patch of physics would produce),
    /// then feed cheap 2 ms ticks whose wall clock advances only 2 ms each —
    /// the loop running flat out — and require the deficit to be worked back
    /// off and the pacer to resume sleeping once it is.
    ///
    /// **Results (2026-08-13).** After the stall the plant clock read 0.100 s
    /// against 0.600 s of wall clock: a 500 ms deficit, flagged behind, with
    /// every tick taking the 5 microsecond catch-up sleep. The deficit then
    /// fell by 8 ms per catch-up tick and reached zero after 63 of them, at
    /// which point the tick was ahead of real time again and slept out its
    /// 8 ms remainder. Interpretation: lost time is repaid rather than
    /// abandoned, and the loop converges back on 1:1.
    #[test]
    fn a_stall_is_worked_off_rather_than_lost() {
        let mut pacer = pacer_10ms();
        let mut wall = Duration::ZERO;

        for _ in 0..10 {
            wall += ms(60);
            let pacing = pacer.pace(ms(60), wall);
            assert!(!pacing.ahead_of_real_time);
            assert_eq!(pacing.sleep_for, CATCH_UP_SLEEP);
        }
        assert_eq!(pacer.simulated_total(), ms(100));
        assert_eq!(pacer.real_time_deficit(), ms(500));
        assert!(pacer.is_behind_real_time());

        let mut catch_up_ticks = 0;
        loop {
            wall += ms(2);
            let pacing = pacer.pace(ms(2), wall);
            if pacing.ahead_of_real_time {
                assert_eq!(pacing.sleep_for, ms(8));
                break;
            }
            catch_up_ticks += 1;
            assert!(catch_up_ticks < 1000, "the deficit never cleared");
        }
        assert_eq!(catch_up_ticks, 62);
        assert_eq!(pacer.real_time_deficit(), Duration::ZERO);
        assert!(!pacer.is_behind_real_time());
    }

    /// V&V: a mis-set simulated timestep degrades rather than panicking.
    ///
    /// **Methodology.** [`Duration::from_secs_f64`] panics on a negative or
    /// non-finite argument, and a physics thread is the wrong place to discover
    /// a bad configuration constant. Construct pacers from a negative and a NaN
    /// timestep and require a zero plant advance instead of a panic.
    ///
    /// **Results (2026-08-13).** Both gave `simulated_per_tick == 0 s`; after
    /// 200 ticks against 2.000 s of wall clock the ratio read 0.000000 with a
    /// 2.000 s deficit and the loop was flagged behind, which is the correct
    /// reading of "the plant clock is not advancing". Interpretation: the fault
    /// shows in the readout rather than as a dead thread.
    #[test]
    fn a_bad_timestep_degrades_instead_of_panicking() {
        for bad in [-0.010_f64, f64::NAN] {
            let mut pacer = RealTimePacer::new(Time::new::<second>(bad), ms(10));
            assert_eq!(pacer.simulated_per_tick(), Duration::ZERO);
            let mut wall = Duration::ZERO;
            for _ in 0..200 {
                wall += ms(10);
                pacer.pace(ms(1), wall);
            }
            assert_eq!(pacer.measured_real_time_ratio(), Some(0.0));
            assert_eq!(pacer.real_time_deficit(), ms(2000));
            assert!(pacer.is_behind_real_time());
        }
    }
}
