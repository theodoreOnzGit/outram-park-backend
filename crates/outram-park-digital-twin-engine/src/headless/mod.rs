//! Drive a simulator with no GUI, no window and no thread.
//!
//! **Required of every egui simulator in this workspace** — see the "Every egui
//! simulator ships a headless mode" hard rule in the root `CLAUDE.md`, and bead
//! `op-otiy` / gh #150.
//!
//! ## Why
//!
//! A GUI-only simulator cannot be tested and therefore cannot be trusted. An
//! agent or a CI job cannot open a window, so the model ends up checked only by
//! a human watching it, which in practice means not checked at all.
//!
//! The rule was written after the first headless run of `htgr_sim_v1`.
//! `PlantCommands::default()` was documented as starting *"near steady state
//! rather than on a prompt excursion"*; it actually overshoots to ~2.8x nominal
//! power before settling. The docstring had been wrong, unnoticed, because
//! nobody could run the thing without watching it.
//!
//! ## The contract
//!
//! Implement [`HeadlessModel`] and you get [`run`], [`run_csv`] and a
//! determinism check for free. The trait is deliberately narrow, and each
//! requirement rules out a specific way headless testing goes wrong:
//!
//! - [`submit`](HeadlessModel::submit) — controls **one way in**
//! - [`tick`](HeadlessModel::tick) — **exactly one** timestep, then return. No
//!   loop, no thread, no clock. A model that owns its own cadence cannot be
//!   scheduled by a test, a native pool, or a Web Worker.
//! - [`sample`](HeadlessModel::sample) — observations **one way out**
//!
//! Implementations must be **deterministic**: same controls in, byte-identical
//! trace out. No wall clock, no time-seeded RNG, no I/O inside `tick`.
//! [`assert_deterministic`] checks it, and every committed fixture depends on
//! that property holding.

use std::fmt::Write as _;

/// A simulator that can be advanced headlessly.
pub trait HeadlessModel {
    /// Operator input. One way in.
    type Controls: Clone;
    /// One row of observations. One way out.
    type Sample: Clone + PartialEq;

    /// Accept new controls. Called before stepping, not per step.
    fn submit(&mut self, controls: Self::Controls);

    /// Advance **exactly one** timestep and return.
    ///
    /// Bounded work. No loop, no sleep, no clock read.
    fn tick(&mut self);

    /// Observe the current state.
    fn sample(&self, step: usize) -> Self::Sample;

    /// Column names for CSV output, matching [`sample_csv`](Self::sample_csv).
    fn csv_header() -> &'static str;

    /// One CSV row for a sample.
    ///
    /// Use fixed precision so a committed fixture diffs cleanly — a trace whose
    /// column widths wander is a trace whose diffs are unreadable.
    fn sample_csv(sample: &Self::Sample) -> String;
}

/// How long to run and how often to record.
#[derive(Clone, Copy, Debug)]
pub struct HeadlessRun {
    /// Timesteps to advance.
    pub steps: usize,
    /// Record every `sample_every` steps. `1` records every step. The final
    /// step is always recorded, so a run never ends on an unobserved state.
    pub sample_every: usize,
}

impl Default for HeadlessRun {
    fn default() -> Self {
        Self {
            steps: 600,
            sample_every: 60,
        }
    }
}

/// Advance `model` and return the sampled trace.
pub fn run<M: HeadlessModel>(
    model: &mut M,
    controls: M::Controls,
    cfg: HeadlessRun,
) -> Vec<M::Sample> {
    model.submit(controls);
    let every = cfg.sample_every.max(1);
    let mut trace = Vec::with_capacity(cfg.steps / every + 1);
    for step in 0..cfg.steps {
        model.tick();
        if step % every == 0 || step + 1 == cfg.steps {
            trace.push(model.sample(step));
        }
    }
    trace
}

/// [`run`], rendered as CSV with a header.
pub fn run_csv<M: HeadlessModel>(model: &mut M, controls: M::Controls, cfg: HeadlessRun) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{}", M::csv_header());
    for s in run(model, controls, cfg) {
        let _ = writeln!(out, "{}", M::sample_csv(&s));
    }
    out
}

/// Assert that two fresh runs of the same configuration agree exactly.
///
/// **The property every committed reference fixture rests on.** If this fails,
/// no baseline means anything, and a failing regression test cannot be told
/// apart from noise.
///
/// `fresh` must build a *new* model each call — reusing one would test nothing.
pub fn assert_deterministic<M, F>(mut fresh: F, controls: M::Controls, cfg: HeadlessRun)
where
    M: HeadlessModel,
    F: FnMut() -> M,
    M::Sample: std::fmt::Debug,
{
    let a = run(&mut fresh(), controls.clone(), cfg);
    let b = run(&mut fresh(), controls, cfg);
    assert_eq!(
        a.len(),
        b.len(),
        "headless runs produced different row counts"
    );
    for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
        assert!(
            x == y,
            "headless run is not deterministic at row {i}\n  first:  {x:?}\n  second: {y:?}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A trivial counter, standing in for a plant.
    struct Counter {
        v: i64,
        rate: i64,
    }

    impl HeadlessModel for Counter {
        type Controls = i64;
        type Sample = (usize, i64);

        fn submit(&mut self, rate: i64) {
            self.rate = rate;
        }
        fn tick(&mut self) {
            self.v += self.rate;
        }
        fn sample(&self, step: usize) -> (usize, i64) {
            (step, self.v)
        }
        fn csv_header() -> &'static str {
            "step,value"
        }
        fn sample_csv(s: &(usize, i64)) -> String {
            format!("{},{}", s.0, s.1)
        }
    }

    #[test]
    fn samples_at_the_requested_interval_and_always_the_last_step() {
        let mut c = Counter { v: 0, rate: 1 };
        let t = run(
            &mut c,
            1,
            HeadlessRun {
                steps: 10,
                sample_every: 4,
            },
        );
        assert_eq!(t.iter().map(|s| s.0).collect::<Vec<_>>(), vec![0, 4, 8, 9]);
    }

    #[test]
    fn tick_advances_exactly_once_per_call() {
        let mut c = Counter { v: 0, rate: 3 };
        c.tick();
        assert_eq!(c.v, 3, "tick must advance exactly one step");
    }

    #[test]
    fn determinism_helper_passes_for_a_deterministic_model() {
        assert_deterministic(
            || Counter { v: 0, rate: 1 },
            2,
            HeadlessRun {
                steps: 8,
                sample_every: 2,
            },
        );
    }

    #[test]
    fn csv_has_a_header_and_one_row_per_sample() {
        let mut c = Counter { v: 0, rate: 1 };
        let csv = run_csv(
            &mut c,
            1,
            HeadlessRun {
                steps: 4,
                sample_every: 2,
            },
        );
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "step,value");
        assert_eq!(lines.len(), 1 + 3);
    }
}
