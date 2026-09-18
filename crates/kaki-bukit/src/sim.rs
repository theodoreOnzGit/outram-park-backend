// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>
//   Upstream file:    src/context.h (SimInfo, the cyclusYear/cyclusMonth
//                     constants), src/timer.h, src/timer.cc
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! Simulation configuration and the time step clock.
//!
//! # The time step
//!
//! A Cyclus time step is **one month**, and "one month" is a fixed number of
//! seconds rather than a calendar month — [`SECONDS_PER_MONTH`]. Every month
//! in a simulation is therefore exactly the same length, which is what makes
//! a decay over `n` steps depend only on `n`.
//!
//! # The phases of a time step
//!
//! Each step runs five phases in a fixed order, and the order is the whole
//! protocol between agents. [`Phase`] names them; the loop that drives them is
//! [`Simulation::step`](crate::agents::Simulation::step).
//!
//! 1. [`Phase::Build`] — agents scheduled to be built enter the simulation.
//! 2. [`Phase::Tick`] — every agent updates its own state and decides what it
//!    will want. Nothing is traded yet.
//! 3. [`Phase::Exchange`] — the dynamic resource exchange runs: requests,
//!    bids, solve, trades. See [`exchange`](crate::exchange).
//! 4. [`Phase::Tock`] — agents process what they received.
//! 5. [`Phase::Decision`] — agents make end-of-step decisions (whether to
//!    decommission, whether to request a new build).
//!
//! Upstream then runs a decommission sweep and increments the clock. The split
//! matters: an agent that traded in phase 3 may only act on the result in
//! phase 4, which is what stops the outcome from depending on the order agents
//! happen to be stored in.

use alloc::string::{String, ToString};

use crate::error::{CyclusError, Result};

/// Seconds in a Cyclus year. Upstream `cyclusYear = 31558200`.
///
/// That is 365.2569 days, not the Julian year's 365.25 — upstream's value, and
/// preserved exactly, because a decay computed over a different year length
/// would not reproduce an upstream result.
pub const SECONDS_PER_YEAR: u64 = 31_558_200;

/// Months in a year. Upstream `kMonthsPerYear`.
pub const MONTHS_PER_YEAR: u64 = 12;

/// Seconds in one time step. Upstream `cyclusMonth = cyclusYear / 12`, which
/// is `2_629_850` seconds exactly (integer division).
pub const SECONDS_PER_MONTH: u64 = SECONDS_PER_YEAR / MONTHS_PER_YEAR;

/// The default duration of one time step, in seconds. Upstream
/// `kDefaultTimeStepDur`.
pub const DEFAULT_TIME_STEP_DUR: u64 = SECONDS_PER_MONTH;

/// Upstream `kDefaultSeed`.
pub const DEFAULT_SEED: u64 = 20_160_212;

/// Upstream `kDefaultStride`.
pub const DEFAULT_STRIDE: u64 = 10_000;

/// Whether materials may decay during the simulation. Upstream's
/// `SimInfo::decay`, which is a `std::string` holding `"never"`, `"manual"` or
/// `"lazy"`.
///
/// An enum rather than a string, because the set is closed and a typo in a
/// string is a silent no-decay simulation — a failure mode worth making
/// impossible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DecayMode {
    /// Decay is never applied. Upstream `"never"`.
    #[default]
    Never,
    /// Decay is applied only when an agent explicitly asks for it. Upstream
    /// `"manual"`. This is the mode this crate supports.
    Manual,
    /// Upstream `"lazy"`: decay is applied implicitly whenever a composition
    /// is read, memoised along a shared decay chain.
    ///
    /// **Not implemented here.** The memo is a `shared_ptr` graph of exactly
    /// the kind the workspace's Rust rules exclude, and "decay happens when
    /// you look at it" makes a result depend on which accessors a caller
    /// happened to invoke. Selecting it is accepted so a configuration can
    /// round-trip, but [`SimInfo::validate`] rejects it rather than silently
    /// behaving like [`DecayMode::Manual`].
    Lazy,
}

impl DecayMode {
    /// The upstream string form, for round-tripping a configuration.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Never => "never",
            Self::Manual => "manual",
            Self::Lazy => "lazy",
        }
    }

    /// Parses the upstream string form.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] for anything other than `"never"`, `"manual"` or
    /// `"lazy"`.
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "never" => Ok(Self::Never),
            "manual" => Ok(Self::Manual),
            "lazy" => Ok(Self::Lazy),
            _ => Err(CyclusError::Value("unknown decay mode")),
        }
    }
}

/// Simulation-wide configuration. Upstream `SimInfo`.
///
/// # Fields not carried over
///
/// Upstream also holds `parent_sim` (a `boost::uuids::uuid`), `parent_type`
/// and `branch_time`, which exist to record a simulation's provenance in the
/// output database when one run branches from another. There is no output
/// database in this `no_std` kernel, so they are omitted rather than kept as
/// dead weight. `explicit_inventory` and `explicit_inventory_compact` are
/// omitted for the same reason — both select output tables.
#[derive(Debug, Clone, PartialEq)]
pub struct SimInfo {
    /// A user-defined label for this simulation.
    pub handle: String,
    /// Whether and how materials decay.
    pub decay: DecayMode,
    /// Length of the simulation, in time steps (months). Must be positive.
    pub duration: i64,
    /// Start year, e.g. 1973.
    pub y0: i32,
    /// Start month, 1 (January) through 12 (December).
    pub m0: i32,
    /// Duration of one time step, in seconds. Defaults to
    /// [`DEFAULT_TIME_STEP_DUR`].
    pub dt: u64,
    /// Generic tolerance for this simulation. Defaults to
    /// [`crate::limits::EPS`].
    pub eps: f64,
    /// Resource tolerance, in kilograms. Defaults to
    /// [`crate::limits::EPS_RSRC`].
    pub eps_rsrc: f64,
    /// Seed for a random number generator, for agents that need one.
    pub seed: u64,
    /// Stride length for a random number generator. Unused by this crate;
    /// carried because upstream carries it.
    pub stride: u64,
}

impl Default for SimInfo {
    /// Upstream's `SimInfo()` default: one time step, starting January 2010,
    /// no decay.
    fn default() -> Self {
        Self {
            handle: String::new(),
            decay: DecayMode::Never,
            duration: 1,
            y0: 2010,
            m0: 1,
            dt: DEFAULT_TIME_STEP_DUR,
            eps: crate::limits::EPS,
            eps_rsrc: crate::limits::EPS_RSRC,
            seed: DEFAULT_SEED,
            stride: DEFAULT_STRIDE,
        }
    }
}

impl SimInfo {
    /// A simulation of `duration` time steps starting in January 2010.
    /// Upstream `SimInfo(int dur)`.
    #[must_use]
    pub fn new(duration: i64) -> Self {
        Self {
            duration,
            ..Self::default()
        }
    }

    /// A simulation of `duration` time steps starting at `y0`/`m0`.
    #[must_use]
    pub fn starting(duration: i64, y0: i32, m0: i32) -> Self {
        Self {
            duration,
            y0,
            m0,
            ..Self::default()
        }
    }

    /// Sets the user-defined handle, consuming and returning `self`.
    #[must_use]
    pub fn with_handle(mut self, handle: &str) -> Self {
        self.handle = handle.to_string();
        self
    }

    /// Sets the decay mode, consuming and returning `self`.
    #[must_use]
    pub fn with_decay(mut self, decay: DecayMode) -> Self {
        self.decay = decay;
        self
    }

    /// Checks that this configuration can actually be run.
    ///
    /// # Errors
    ///
    /// - [`CyclusError::Value`] if `duration` is not positive, `m0` is outside
    ///   1..=12, `dt` is zero, or either tolerance is negative.
    /// - [`CyclusError::Value`] if [`DecayMode::Lazy`] is selected — see that
    ///   variant's documentation for why it is rejected rather than silently
    ///   downgraded.
    pub fn validate(&self) -> Result<()> {
        if self.duration <= 0 {
            return Err(CyclusError::Value("simulation duration must be positive"));
        }
        if !(1..=12).contains(&self.m0) {
            return Err(CyclusError::Value("start month must be 1..=12"));
        }
        if self.dt == 0 {
            return Err(CyclusError::Value("time step duration must be positive"));
        }
        if self.eps < 0.0 || self.eps_rsrc < 0.0 {
            return Err(CyclusError::Value("tolerances cannot be negative"));
        }
        if self.decay == DecayMode::Lazy {
            return Err(CyclusError::Value(
                "lazy decay is not implemented in this port; use DecayMode::Manual",
            ));
        }
        Ok(())
    }

    /// The calendar year and month at time step `t`.
    ///
    /// Returns `(year, month)` with month in 1..=12. Upstream's `Timer` does
    /// the same arithmetic inline when recording output.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaki_bukit::sim::SimInfo;
    ///
    /// let si = SimInfo::starting(24, 2010, 11);
    /// assert_eq!(si.calendar_at(0), (2010, 11));
    /// assert_eq!(si.calendar_at(1), (2010, 12));
    /// assert_eq!(si.calendar_at(2), (2011, 1));
    /// ```
    #[must_use]
    pub fn calendar_at(&self, t: i64) -> (i32, i32) {
        let months_from_zero = i64::from(self.m0 - 1) + t;
        let year = self.y0 + (months_from_zero.div_euclid(12)) as i32;
        let month = months_from_zero.rem_euclid(12) as i32 + 1;
        (year, month)
    }

    /// The number of time steps from the simulation start to `year`/`month`.
    ///
    /// Upstream `Timer::CalcTimeDiff`. Negative if the date precedes the
    /// simulation start.
    #[must_use]
    pub fn steps_until(&self, year: i32, month: i32) -> i64 {
        i64::from(year - self.y0) * 12 + i64::from(month - self.m0)
    }

    /// The elapsed seconds represented by `n` time steps.
    #[must_use]
    pub fn seconds_for(&self, n: i64) -> f64 {
        n as f64 * self.dt as f64
    }
}

/// The phases of a single time step, in the order they run.
///
/// Upstream has no such enum — the phases are five method calls in
/// `Timer::RunSim`. Naming them makes the protocol inspectable: a test can
/// assert that a step visited the phases in order, and an agent's
/// documentation can say which phase it acts in without describing a call
/// site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Phase {
    /// Scheduled agents enter the simulation. Upstream `Timer::DoBuild`.
    Build,
    /// Agents update their own state and decide what they want. Upstream
    /// `Timer::DoTick`.
    Tick,
    /// The dynamic resource exchange runs. Upstream `Timer::DoResEx`.
    Exchange,
    /// Agents process what they received. Upstream `Timer::DoTock`.
    Tock,
    /// End-of-step decisions. Upstream `Timer::DoDecision`.
    Decision,
    /// Agents scheduled for decommissioning leave. Upstream `Timer::DoDecom`.
    Decommission,
}

impl Phase {
    /// Every phase, in execution order.
    pub const ORDER: [Phase; 6] = [
        Phase::Build,
        Phase::Tick,
        Phase::Exchange,
        Phase::Tock,
        Phase::Decision,
        Phase::Decommission,
    ];
}

/// The simulation clock. Upstream `Timer`, minus the output-recording and
/// progress-bar machinery.
///
/// This type owns the time step counter and the early-termination flag, and
/// nothing else. Upstream's `Timer` also owns the agent registry and the
/// build/decommission queues; here those belong to
/// [`Context`](crate::context::Context), because an agent registry that lives
/// on the clock makes the clock impossible to test on its own.
#[derive(Debug, Clone, PartialEq)]
pub struct Timer {
    info: SimInfo,
    time: i64,
    killed: bool,
}

impl Timer {
    /// Creates a clock for `info`, starting at time step 0.
    ///
    /// # Errors
    ///
    /// Whatever [`SimInfo::validate`] reports.
    pub fn new(info: SimInfo) -> Result<Self> {
        info.validate()?;
        Ok(Self {
            info,
            time: 0,
            killed: false,
        })
    }

    /// The current time step. Upstream `Timer::time()`.
    #[must_use]
    pub fn time(&self) -> i64 {
        self.time
    }

    /// The simulation duration in time steps. Upstream `Timer::dur()`.
    #[must_use]
    pub fn duration(&self) -> i64 {
        self.info.duration
    }

    /// The simulation configuration.
    #[must_use]
    pub fn info(&self) -> &SimInfo {
        &self.info
    }

    /// `true` while there are steps left and the simulation has not been
    /// killed. Upstream's `while (time_ < si_.duration)` plus its `want_kill_`
    /// break.
    #[must_use]
    pub fn running(&self) -> bool {
        self.time < self.info.duration && !self.killed
    }

    /// Requests early termination. Upstream `Timer::KillSim`.
    ///
    /// The current step still completes; [`running`](Timer::running) reports
    /// `false` afterwards. That matches upstream, which breaks at the *end* of
    /// the loop body.
    pub fn kill(&mut self) {
        self.killed = true;
    }

    /// `true` if [`kill`](Timer::kill) was called.
    #[must_use]
    pub fn killed(&self) -> bool {
        self.killed
    }

    /// Advances the clock by one step. Upstream's `time_++`.
    pub fn advance(&mut self) {
        self.time += 1;
    }

    /// The calendar year and month of the current step.
    #[must_use]
    pub fn calendar(&self) -> (i32, i32) {
        self.info.calendar_at(self.time)
    }

    /// Resets the clock to time step 0 and clears the kill flag. Upstream
    /// `Timer::Reset`.
    pub fn reset(&mut self) {
        self.time = 0;
        self.killed = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn a_time_step_is_upstreams_month_in_seconds() {
        assert_eq!(SECONDS_PER_YEAR, 31_558_200);
        assert_eq!(SECONDS_PER_MONTH, 2_629_850);
        assert_eq!(SECONDS_PER_MONTH * 12, SECONDS_PER_YEAR);
    }

    #[test]
    fn calendar_rolls_over_the_year() {
        let si = SimInfo::starting(24, 2010, 11);
        assert_eq!(si.calendar_at(0), (2010, 11));
        assert_eq!(si.calendar_at(1), (2010, 12));
        assert_eq!(si.calendar_at(2), (2011, 1));
        assert_eq!(si.calendar_at(14), (2012, 1));
    }

    #[test]
    fn calendar_handles_a_january_start() {
        let si = SimInfo::starting(24, 2010, 1);
        assert_eq!(si.calendar_at(0), (2010, 1));
        assert_eq!(si.calendar_at(11), (2010, 12));
        assert_eq!(si.calendar_at(12), (2011, 1));
    }

    #[test]
    fn steps_until_inverts_calendar_at() {
        let si = SimInfo::starting(60, 2010, 11);
        for t in 0..60 {
            let (y, m) = si.calendar_at(t);
            assert_eq!(si.steps_until(y, m), t, "round trip failed at step {t}");
        }
    }

    #[test]
    fn steps_until_is_negative_before_the_start() {
        let si = SimInfo::starting(24, 2010, 6);
        assert_eq!(si.steps_until(2010, 5), -1);
        assert_eq!(si.steps_until(2009, 6), -12);
    }

    #[test]
    fn validate_rejects_a_nonsense_configuration() {
        let mut si = SimInfo::new(0);
        assert!(si.validate().is_err());

        si = SimInfo::starting(10, 2010, 13);
        assert!(si.validate().is_err());

        si = SimInfo::new(10);
        si.dt = 0;
        assert!(si.validate().is_err());

        si = SimInfo::new(10);
        si.eps = -1.0;
        assert!(si.validate().is_err());
    }

    #[test]
    fn lazy_decay_is_rejected_rather_than_silently_downgraded() {
        let si = SimInfo::new(10).with_decay(DecayMode::Lazy);
        assert_eq!(
            si.validate(),
            Err(CyclusError::Value(
                "lazy decay is not implemented in this port; use DecayMode::Manual"
            ))
        );
    }

    #[test]
    fn decay_mode_round_trips_through_its_upstream_string() {
        for m in [DecayMode::Never, DecayMode::Manual, DecayMode::Lazy] {
            assert_eq!(DecayMode::parse(m.as_str()).unwrap(), m);
        }
        assert!(DecayMode::parse("sometimes").is_err());
    }

    #[test]
    fn timer_runs_for_exactly_the_configured_duration() {
        let mut t = Timer::new(SimInfo::new(5)).unwrap();
        let mut steps = 0;
        while t.running() {
            steps += 1;
            t.advance();
        }
        assert_eq!(steps, 5);
        assert_eq!(t.time(), 5);
    }

    #[test]
    fn kill_stops_the_clock_after_the_current_step() {
        let mut t = Timer::new(SimInfo::new(100)).unwrap();
        let mut steps = 0;
        while t.running() {
            steps += 1;
            if steps == 3 {
                t.kill();
            }
            t.advance();
        }
        assert_eq!(steps, 3);
        assert!(t.killed());
    }

    #[test]
    fn reset_returns_the_clock_to_the_start() {
        let mut t = Timer::new(SimInfo::new(5)).unwrap();
        t.advance();
        t.advance();
        t.kill();
        t.reset();
        assert_eq!(t.time(), 0);
        assert!(!t.killed());
        assert!(t.running());
    }

    #[test]
    fn phase_order_matches_upstreams_run_loop() {
        let order: Vec<Phase> = Phase::ORDER.to_vec();
        assert_eq!(
            order,
            alloc::vec![
                Phase::Build,
                Phase::Tick,
                Phase::Exchange,
                Phase::Tock,
                Phase::Decision,
                Phase::Decommission
            ]
        );
    }

    #[test]
    fn seconds_for_scales_by_the_time_step() {
        let si = SimInfo::new(12);
        assert_eq!(si.seconds_for(12), SECONDS_PER_YEAR as f64);
    }
}
