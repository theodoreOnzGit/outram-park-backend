// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

//! **Headless crude-distillation plant** — the transient CDU model, with no
//! GUI attached.
//!
//! [`crate::petroleum::crude_distillation`] gives a *steady* crude column:
//! characterise a black oil, cut it into pseudo-components, solve the MESH
//! system once, read the cuts. This module is the same column run forward in
//! time — [`CrudePlant`] wraps
//! [`DynamicColumn`](crate::columns::dynamic::DynamicColumn) so an operator (or
//! a GUI, or a test) can move the reflux ratio and reboiler duty and watch the
//! cuts respond.
//!
//! The plant lives here, with the physics, rather than inside a simulator
//! example: a model that only exists in an example is reachable from nothing
//! else. The GUI counterpart is
//! `outram-park-digital-twin-engine`'s `components::DistillationColumnVisual`,
//! which draws a [`CrudeSnapshot`] and computes nothing itself.
//!
//! ```no_run
//! use outram_park_fork_dwsim_libs::petroleum::crude_distillation::{
//!     BlackOilCrude, CrudeColumnConfig,
//! };
//! use outram_park_fork_dwsim_libs::petroleum::crude_plant::{CrudePlant, CrudeCommands};
//!
//! let mut plant = CrudePlant::new(
//!     &BlackOilCrude::light_sweet(),
//!     &CrudeColumnConfig::atmospheric_default(),
//!     8,
//! )?;
//! for _ in 0..100 {
//!     plant.step(CrudeCommands::from_plant(&plant))?;
//! }
//! let snap = plant.snapshot()?;
//! println!("{:.1} K at the top", snap.stage_temperature_k[0]);
//! # Ok::<(), outram_park_fork_dwsim_libs::petroleum::crude_distillation::CrudeColumnError>(())
//! ```
//!
//! > **⚠️ Untrusted AI-assisted draft — no human V&V.** The transient model is
//! > the one validated for a benzene/toluene column under bead `op-6rhz`; it
//! > has *not* been validated against dynamic crude-unit data, and neither the
//! > cut yields nor their response to a control move should be read as
//! > quantitative. Not for operational, licensing, or safety use.

use crate::columns::dynamic::{
    DynamicColumn, DynamicColumnOperating, DynamicColumnState, TrayHydraulics,
};
use crate::columns::model::ColumnSolverOutput;
use crate::petroleum::crude_distillation::{
    crude_column_setup, BlackOilCrude, CrudeColumnConfig, CrudeColumnError, CrudeColumnSetup,
    CrudeCut,
};

/// RK4 step \[s\]. Matches the benzene plant's step: the same integrator on the
/// same model, so the two behave comparably per tick.
pub const RK4_DT_S: f64 = 0.25;

/// RK4 sub-steps per [`CrudePlant::step`]. At a ~100 ms GUI cadence this runs
/// the plant at roughly 20x real time.
pub const SUBSTEPS_PER_STEP: usize = 20;

/// Hydraulic residence time of an interior tray \[s\].
///
/// A crude unit's trays hold more than a laboratory splitter's; 90 s is a
/// plausible large-column value and is **not** fitted to anything.
pub const TRAY_TAU_S: f64 = 90.0;

/// Condenser-drum and reboiler-sump molar holdups \[mol\].
pub const VESSEL_HOLDUP_MOL: f64 = 400.0;

/// Operator-manipulated inputs to the crude column.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrudeCommands {
    /// Reflux ratio `R = L_0 / D` \[-\], `> 0`.
    pub reflux_ratio: f64,
    /// Reboiler heat duty \[W\], `> 0`.
    pub reboiler_duty_watts: f64,
}

impl CrudeCommands {
    /// The operating point the plant was built at — the configured reflux
    /// ratio and the duty its own steady solve requires.
    #[must_use]
    pub fn from_plant(plant: &CrudePlant) -> Self {
        let op = plant.operating();
        Self {
            reflux_ratio: op.reflux_ratio,
            reboiler_duty_watts: op.reboiler_duty_watts,
        }
    }
}

/// The reboiler duty this column runs at in **steady** state \[W\].
///
/// Derived, not chosen: solve the same column steady and read the duty its own
/// energy balance requires. Starting the transient anywhere else means the
/// plant's first act is to drift away from the configuration that was solved,
/// and an invented duty is exactly what left the startup state with a negative
/// energy-balance denominator when this module was first written.
///
/// # Errors
///
/// [`CrudeColumnError`] if the steady column does not solve.
pub fn steady_reboiler_duty_watts(
    crude: &BlackOilCrude,
    config: &CrudeColumnConfig,
    cut_count: usize,
) -> Result<f64, CrudeColumnError> {
    let setup = crude_column_setup(crude, config, cut_count)?;
    Ok(reboiler_duty_of(&steady_solve(&setup)?))
}

/// Solve a prepared crude column steady.
fn steady_solve(setup: &CrudeColumnSetup) -> Result<ColumnSolverOutput, CrudeColumnError> {
    use crate::columns::solver::ColumnSolverMethod;
    ColumnSolverMethod::default()
        .solve(&setup.input)
        .map_err(|e| CrudeColumnError::Solve(format!("steady solve: {e:?}")))
}

fn reboiler_duty_of(steady: &ColumnSolverOutput) -> f64 {
    use uom::si::power::watt;
    -steady.reboiler_duty().get::<watt>()
}

/// A GUI-facing readout of the plant. Plain data: everything here is read off
/// the model, nothing is computed by the consumer.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CrudeSnapshot {
    /// Number of stages, stage 0 = condenser.
    pub n_stages: usize,
    /// Per-stage temperature \[K\], top first.
    pub stage_temperature_k: Vec<f64>,
    /// Per-stage liquid molar holdup \[mol\].
    pub stage_holdup_mol: Vec<f64>,
    /// Per-stage liquid flow \[mol/s\].
    pub liquid_flow_mol_s: Vec<f64>,
    /// Per-stage vapour flow \[mol/s\].
    pub vapor_flow_mol_s: Vec<f64>,
    /// Mole fraction of the lightest pseudo-component in each stage's liquid
    /// \[-\] — the crude analogue of a light-key profile.
    pub lightest_liquid_fraction: Vec<f64>,
    /// Overhead distillate \[mol/s\].
    pub distillate_mol_s: f64,
    /// Column bottoms \[mol/s\], excluding the bypassed heavy end.
    pub bottoms_mol_s: f64,
    /// `(stage, rate [mol/s], cut)` for every side draw, plus the overhead and
    /// the residue, in top-to-bottom order. The cut label comes from the
    /// *current* draw temperature, so it moves as the column does.
    pub cuts: Vec<(usize, f64, CrudeCut)>,
    /// Accumulated simulation time \[s\].
    pub sim_time_s: f64,
}

/// Build the starting inventory from the converged steady profile: each
/// stage holds what the hydraulics say its own steady liquid flow implies, in
/// that stage's own steady composition.
///
/// The benzene plant deliberately starts from a *uniform* fill so the approach
/// to steady state is visible from the first frame. That does not transfer to
/// a crude unit: one composition repeated down a column spanning 200 K of
/// boiling range is not a physical liquid anywhere, and the energy balance
/// says so — a uniform feed-composition fill left stage 10 with a negative
/// `H_vap − H_liq`. A crude column is commissioned near its design point
/// anyway, so starting there is both the workable choice and the realistic one.
fn steady_startup_state(
    column: &DynamicColumn,
    steady: &ColumnSolverOutput,
    op: &DynamicColumnOperating,
) -> DynamicColumnState {
    let n = column.n_stages();
    let holdups = (0..n)
        .map(|j| {
            let total = match j {
                0 => op.drum_holdup_moles,
                _ if j == n - 1 => op.sump_holdup_moles,
                _ => op
                    .hydraulics
                    .holdup_for_flow(steady.liquid_flows.get(j).copied().unwrap_or(0.0)),
            };
            let x = &steady.liquid_compositions[j];
            let s: f64 = x.iter().sum();
            x.iter()
                .map(|xi| total * if s > 0.0 { xi / s } else { 0.0 })
                .collect()
        })
        .collect();
    DynamicColumnState { holdups }
}

/// The crude-distillation plant: a dynamic column, its differential state, and
/// the operating point it was last built with.
pub struct CrudePlant {
    setup: CrudeColumnSetup,
    column: DynamicColumn,
    state: DynamicColumnState,
    current_op: DynamicColumnOperating,
    /// Accumulated simulation time \[s\].
    pub sim_time_s: f64,
}

impl CrudePlant {
    /// Build the plant from a black-oil crude and a column configuration,
    /// starting from a uniform-fill startup state — deliberately far from
    /// steady, so the approach is visible from the first step.
    ///
    /// # Errors
    ///
    /// [`CrudeColumnError`] if the column cannot be assembled from the crude,
    /// or if the dynamic model rejects it.
    pub fn new(
        crude: &BlackOilCrude,
        config: &CrudeColumnConfig,
        cut_count: usize,
    ) -> Result<Self, CrudeColumnError> {
        Self::with_inventory(crude, config, cut_count, TRAY_TAU_S, VESSEL_HOLDUP_MOL)
    }

    /// As [`Self::new`], with the tray residence time \[s\] and end-vessel
    /// holdup \[mol\] given explicitly.
    ///
    /// These set the column's inventory and therefore its dynamics; the
    /// defaults are plausible large-column values and are not fitted.
    ///
    /// # Errors
    ///
    /// [`CrudeColumnError`] if the column cannot be assembled or solved.
    pub fn with_inventory(
        crude: &BlackOilCrude,
        config: &CrudeColumnConfig,
        cut_count: usize,
        tray_tau_s: f64,
        vessel_holdup_mol: f64,
    ) -> Result<Self, CrudeColumnError> {
        let setup = crude_column_setup(crude, config, cut_count)?;
        let steady = steady_solve(&setup)?;
        let duty = reboiler_duty_of(&steady);
        let op = DynamicColumnOperating {
            reflux_ratio: config.reflux_ratio,
            reboiler_duty_watts: duty,
            hydraulics: TrayHydraulics::HoldupTimeConstant {
                tau_seconds: tray_tau_s,
            },
            drum_holdup_moles: vessel_holdup_mol,
            sump_holdup_moles: vessel_holdup_mol,
        };
        let column = DynamicColumn::from_solver_input(&setup.input, op)
            .map_err(|e| CrudeColumnError::Solve(format!("dynamic model: {e:?}")))?;
        let state = steady_startup_state(&column, &steady, &op);
        Ok(Self {
            setup,
            column,
            state,
            current_op: op,
            sim_time_s: 0.0,
        })
    }

    /// Advance the plant by [`SUBSTEPS_PER_STEP`] RK4 steps under `commands`.
    ///
    /// A command change rebuilds the model at the new operating point, which
    /// is what makes reflux and duty genuinely manipulable rather than fixed
    /// at construction. An out-of-range command leaves the plant untouched and
    /// returns the error, so a GUI slider cannot crash the simulation.
    ///
    /// # Errors
    ///
    /// [`CrudeColumnError`] if the commands are outside the model's valid
    /// range, or the integrator fails on the current state.
    pub fn step(&mut self, commands: CrudeCommands) -> Result<(), CrudeColumnError> {
        let wanted = DynamicColumnOperating {
            reflux_ratio: commands.reflux_ratio,
            reboiler_duty_watts: commands.reboiler_duty_watts,
            ..self.current_op
        };
        if wanted != self.current_op {
            let rebuilt = DynamicColumn::from_solver_input(&self.setup.input, wanted)
                .map_err(|e| CrudeColumnError::Solve(format!("operating point: {e:?}")))?;
            self.column = rebuilt;
            self.current_op = wanted;
        }
        for _ in 0..SUBSTEPS_PER_STEP {
            self.state = self
                .column
                .step_rk4(&self.state, RK4_DT_S)
                .map_err(|e| CrudeColumnError::Solve(format!("rk4: {e:?}")))?;
            self.sim_time_s += RK4_DT_S;
        }
        Ok(())
    }

    /// Read the current state out as plain data.
    ///
    /// # Errors
    ///
    /// [`CrudeColumnError`] if the profiles cannot be resolved from the
    /// current state.
    pub fn snapshot(&self) -> Result<CrudeSnapshot, CrudeColumnError> {
        let p = self
            .column
            .profiles(&self.state)
            .map_err(|e| CrudeColumnError::Solve(format!("profiles: {e:?}")))?;
        let last = self.column.n_stages().saturating_sub(1);
        let at = |s: usize| p.stage_temperature.get(s).copied().unwrap_or(f64::NAN);

        let mut cuts = Vec::with_capacity(self.setup.draw_rates.len() + 2);
        cuts.push((
            0,
            p.distillate,
            CrudeCut::from_normal_boiling_point_k(at(0)),
        ));
        for &(stage, rate) in &self.setup.draw_rates {
            cuts.push((
                stage,
                rate,
                CrudeCut::from_normal_boiling_point_k(at(stage)),
            ));
        }
        // As in the steady solve, the bypassed heavy end is added back so the
        // residue closes on the whole crude rather than only on what entered
        // the column.
        cuts.push((last, p.bottoms + self.setup.bypass_mol_s, CrudeCut::Residue));

        Ok(CrudeSnapshot {
            n_stages: self.column.n_stages(),
            stage_temperature_k: p.stage_temperature,
            stage_holdup_mol: p.stage_holdup,
            liquid_flow_mol_s: p.liquid_flow,
            vapor_flow_mol_s: p.vapor_flow,
            lightest_liquid_fraction: p.liquid_composition.iter().map(|x| x[0]).collect(),
            distillate_mol_s: p.distillate,
            bottoms_mol_s: p.bottoms,
            cuts,
            sim_time_s: self.sim_time_s,
        })
    }

    /// The operating point currently in force.
    #[must_use]
    pub fn operating(&self) -> DynamicColumnOperating {
        self.current_op
    }

    /// Stages the column draws side products from, with a label for each —
    /// the shape `DistillationColumnVisual::with_side_draws` wants.
    #[must_use]
    pub fn side_draw_stages(&self) -> Vec<(usize, f64)> {
        self.setup.draw_rates.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plant() -> CrudePlant {
        CrudePlant::new(
            &BlackOilCrude::light_sweet(),
            &CrudeColumnConfig::atmospheric_default(),
            8,
        )
        .expect("the reference crude column must build")
    }

    /// Seeded from the steady profile, the plant must *stay* there when
    /// commanded at the operating point it was built at. Drift would mean the
    /// dynamic model and the steady solve disagree about the same column.
    #[test]
    fn the_plant_holds_its_steady_seed() {
        let mut p = plant();
        let cmd = CrudeCommands::from_plant(&p);
        let before = p.snapshot().expect("snapshot");
        for _ in 0..50 {
            p.step(cmd)
                .expect("a step at the steady point must not fail");
        }
        let after = p.snapshot().expect("snapshot");
        for (j, (a, b)) in before
            .stage_temperature_k
            .iter()
            .zip(&after.stage_temperature_k)
            .enumerate()
        {
            assert!(
                (a - b).abs() < 5.0,
                "stage {j} drifted {:.1} K from its steady seed ({a:.1} -> {b:.1})",
                (a - b).abs()
            );
        }
        assert!(
            after.distillate_mol_s > 0.0,
            "distillate went non-positive: {}",
            after.distillate_mol_s
        );
    }

    /// The column must run hotter going down, at every point of the transient.
    #[test]
    fn the_temperature_profile_stays_ordered() {
        let mut p = plant();
        let cmd = CrudeCommands::from_plant(&p);
        for k in 0..40 {
            p.step(cmd).expect("step");
            let s = p.snapshot().expect("snapshot");
            for j in 1..s.n_stages {
                assert!(
                    s.stage_temperature_k[j] >= s.stage_temperature_k[j - 1] - 1.0,
                    "step {k}: stage {j} is colder than the one above it \
                     ({:.1} K under {:.1} K)",
                    s.stage_temperature_k[j],
                    s.stage_temperature_k[j - 1]
                );
            }
        }
    }

    /// The inventory constants set how fast the column responds, not where it
    /// settles. A result that moves with them is the signature of the
    /// reference-dependent energy sweep this module's fix removed: before it,
    /// distillate came out negative and the divergence rate tracked the holdup.
    #[test]
    fn the_steady_answer_does_not_depend_on_the_inventory() {
        let crude = BlackOilCrude::light_sweet();
        let cfg = CrudeColumnConfig::atmospheric_default();
        let mut seen: Vec<f64> = Vec::new();
        for &(tau, hold) in &[(30.0, 50.0), (90.0, 400.0), (300.0, 2000.0)] {
            let mut p = CrudePlant::with_inventory(&crude, &cfg, 8, tau, hold).expect("build");
            let cmd = CrudeCommands::from_plant(&p);
            for _ in 0..30 {
                p.step(cmd).expect("step");
            }
            seen.push(p.snapshot().expect("snapshot").distillate_mol_s);
        }
        let lo = seen.iter().cloned().fold(f64::INFINITY, f64::min);
        let hi = seen.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        assert!(
            lo > 0.0 && (hi - lo) < 1e-3,
            "distillate moved with the inventory: {seen:?}"
        );
    }

    /// Every cut leaves at a positive rate and the residue carries the
    /// bypassed heavy end, so the balance closes on the whole crude.
    #[test]
    fn every_cut_has_a_positive_rate() {
        let p = plant();
        let s = p.snapshot().expect("snapshot");
        assert_eq!(s.cuts.len(), 5, "overhead + three side draws + residue");
        for (stage, rate, cut) in &s.cuts {
            assert!(
                *rate > 0.0 && rate.is_finite(),
                "{cut:?} at stage {stage} draws {rate} mol/s"
            );
        }
    }
}
