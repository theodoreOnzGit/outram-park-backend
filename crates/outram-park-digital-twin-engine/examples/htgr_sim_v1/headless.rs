//! Headless execution of the HTGR plant model — no GUI, no threads, no clock.
//!
//! Exists so the plant can be **run and observed without `eframe`**, which the
//! interactive binary cannot do. Two things need that:
//!
//! 1. **Recording a reference baseline** before the execution refactor
//!    (bead `op-fbou`). A baseline captured through the GUI would be a
//!    baseline of the GUI's timing, not of the physics.
//! 2. **Regression testing.** A test can call [`run`] and compare the trace
//!    against a committed fixture.
//!
//! ## Why this is already almost a kernel driver
//!
//! [`crate::physics::HtgrPlant::step`] is already deterministic, bounded and
//! free of any thread of its own — the loop that drives it lives in
//! `app_scaffold::spawn_physics_thread`, not in the physics. This module is
//! therefore a *second* driver for the same step function, and the fact that
//! writing it required no change to `physics/` is the evidence that the kernel
//! extraction (bead `op-37ke`) is an extraction rather than a rewrite.
//!
//! ## Determinism
//!
//! No wall clock, no RNG, no I/O inside the loop. Given the same
//! `HeadlessConfig` this produces byte-identical output on the same build.
//! That is the property the reference baseline depends on, and
//! `determinism_same_config_same_trace` in this module asserts it.

use crate::app::state::HtgrSnapshot;
use crate::physics::{HtgrPlant, PlantCommands, PLANT_TIMESTEP_S};
use uom::si::f64::Time;
use uom::si::time::second;

/// How to run a headless simulation.
#[derive(Clone, Debug)]
pub struct HeadlessConfig {
    /// Number of plant timesteps to advance.
    pub steps: usize,
    /// Emit one trace row every `sample_every` steps. `1` records every step.
    pub sample_every: usize,
    /// Operator commands, held constant for the whole run.
    ///
    /// `PlantCommands::default()` is the published operating point with the
    /// rods at their critical insertion.
    ///
    /// Its docstring in `physics` claims this starts *"near steady state
    /// rather than on a prompt excursion"*. **Measured 2026-09-06, it does
    /// not:** power rises to ~27.8 MW near 100 s — roughly 2.8x nominal —
    /// before settling near 8.1 MW by 1200 s, with bed temperature still
    /// drifting downward at that point. It is a large startup transient.
    ///
    /// That makes it a *better* baseline, not a worse one — a transient
    /// exercises far more of the model than a hold would — but it must not be
    /// described as steady state.
    pub commands: PlantCommands,
}

impl Default for HeadlessConfig {
    fn default() -> Self {
        Self {
            steps: 600,
            sample_every: 60,
            commands: PlantCommands::default(),
        }
    }
}

/// One sampled row of a headless run.
///
/// A deliberately narrow projection of [`HtgrSnapshot`]'s 66 fields: enough to
/// detect a behavioural change across the whole plant, few enough that the
/// committed fixture stays readable and a diff is interpretable.
#[derive(Clone, Debug, PartialEq)]
pub struct TraceRow {
    pub step: usize,
    pub sim_time_s: f64,
    pub reactor_power_mw: f64,
    pub prompt_power_mw: f64,
    pub delayed_power_mw: f64,
    pub fuel_temperature_k: f64,
    pub bed_temperature_k: f64,
}

impl TraceRow {
    /// CSV header matching [`Self::to_csv`].
    pub fn csv_header() -> &'static str {
        "step,sim_time_s,reactor_power_mw,prompt_power_mw,delayed_power_mw,fuel_temperature_k,bed_temperature_k"
    }

    /// One CSV row.
    ///
    /// `{:.9}` throughout: enough digits that a real behavioural change is
    /// visible, and a fixed width so a committed fixture diffs cleanly.
    pub fn to_csv(&self) -> String {
        format!(
            "{},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9}",
            self.step,
            self.sim_time_s,
            self.reactor_power_mw,
            self.prompt_power_mw,
            self.delayed_power_mw,
            self.fuel_temperature_k,
            self.bed_temperature_k,
        )
    }
}

/// Advance a fresh plant for `cfg.steps` and return the sampled trace.
///
/// Single-threaded, no clock, no I/O. See the module docs on determinism.
pub fn run(cfg: &HeadlessConfig) -> Vec<TraceRow> {
    let dt: Time = Time::new::<second>(PLANT_TIMESTEP_S);
    let mut plant = HtgrPlant::new();
    let mut snap = HtgrSnapshot::default();
    let sample_every = cfg.sample_every.max(1);

    let mut trace = Vec::with_capacity(cfg.steps / sample_every + 1);

    for step in 0..cfg.steps {
        plant.step(dt, cfg.commands.clone());

        if step % sample_every == 0 || step + 1 == cfg.steps {
            plant.write_snapshot(&mut snap);
            trace.push(TraceRow {
                step,
                sim_time_s: snap.sim_time_s,
                reactor_power_mw: snap.reactor_power_mw,
                prompt_power_mw: snap.prompt_power_mw,
                delayed_power_mw: snap.delayed_power_mw,
                fuel_temperature_k: snap.fuel_temperature_k,
                bed_temperature_k: snap.bed_temperature_k,
            });
        }
    }

    trace
}

/// Run and print the trace as CSV on stdout. The `--headless` entry point.
pub fn run_and_print(cfg: &HeadlessConfig) {
    println!("{}", TraceRow::csv_header());
    for row in run(cfg) {
        println!("{}", row.to_csv());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The property the reference baseline rests on. If this fails, no
    /// committed fixture means anything.
    #[test]
    fn determinism_same_config_same_trace() {
        let cfg = HeadlessConfig {
            steps: 120,
            sample_every: 20,
            ..Default::default()
        };
        assert_eq!(run(&cfg), run(&cfg), "headless run is not deterministic");
    }

    /// **The reference baseline for the execution refactor (bead `op-fbou`).**
    ///
    /// Compares a headless run against `reference/baseline_default_commands.csv`,
    /// captured 2026-09-06 **before** any execution change.
    ///
    /// Its job is to prove that extracting kernels and changing who schedules
    /// them **changed nothing about the physics**. Serial extraction should keep
    /// this bit-exact; that is why the comparison is exact rather than
    /// tolerance-based. When parallel execution lands and reduction order
    /// legitimately changes, add a *separate* tolerance-based comparison rather
    /// than loosening this one.
    ///
    /// **This is a baseline of the current PRISMATIC model, and of what it does
    /// rather than what it should do.** It is not an HTR-10 reference and must
    /// never be cited as one — see `op-jyyp.11` for that. It becomes
    /// intentionally obsolete when `op-jyyp` rewrites the physics.
    #[test]
    fn matches_the_recorded_reference_baseline() {
        let fixture = include_str!("reference/baseline_default_commands.csv");
        let cfg = HeadlessConfig {
            steps: 3000,
            sample_every: 25,
            ..Default::default()
        };

        let produced: Vec<String> = std::iter::once(TraceRow::csv_header().to_string())
            .chain(run(&cfg).iter().map(|r| r.to_csv()))
            .collect();
        let expected: Vec<&str> = fixture.lines().filter(|l| !l.is_empty()).collect();

        assert_eq!(
            produced.len(),
            expected.len(),
            "row count changed: produced {} vs reference {}",
            produced.len(),
            expected.len()
        );

        for (i, (got, want)) in produced.iter().zip(expected.iter()).enumerate() {
            assert_eq!(
                got, want,
                "reference baseline diverged at row {i}\n  produced: {got}\n  reference: {want}\n\
                 If this is an intended physics change, regenerate with:\n  \
                 cargo run --release --example htgr_sim_v1 -- --headless 3000 25 \
                 > examples/htgr_sim_v1/reference/baseline_default_commands.csv\n  \
                 If it is NOT intended, the execution refactor changed the physics."
            );
        }
    }

    /// A plant stepped from `PlantCommands::default()` should hold near its
    /// operating point, not run away. This is a sanity check on the harness,
    /// deliberately loose -- it is NOT a physics validation.
    #[test]
    fn default_commands_hold_the_plant_bounded() {
        let trace = run(&HeadlessConfig {
            steps: 600,
            sample_every: 100,
            ..Default::default()
        });
        assert!(!trace.is_empty());
        for row in &trace {
            assert!(
                row.reactor_power_mw.is_finite() && row.fuel_temperature_k.is_finite(),
                "non-finite state at step {}: {:?}",
                row.step,
                row
            );
            assert!(
                row.fuel_temperature_k > 250.0 && row.fuel_temperature_k < 3000.0,
                "fuel temperature left any physical range at step {}: {} K",
                row.step,
                row.fuel_temperature_k
            );
        }
    }
}
