// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da0cc55
//   Upstream source: GeN-Foam/GeN-Foam.C (time loop),
//                    GeN-Foam/include/setDeltaT.C (setDeltaT, adjustDeltaT),
//                    src/classes/common/solver/solver.C (deltaTFactor = 1.2),
//                    src/classes/neutronics/diffusion/diffusionNeutronics.C
//                    (maxDeltaT), src/classes/neutronics/include/calcKeff.H
//   OpenFOAM-ESI v2506: src/OpenFOAM/db/Time/Time.C (run, operator++,
//                    adjustDeltaT, setDeltaT)
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal authors: Carlo Fiorina, Nordine Kerkar, Konstantin Mikityuk (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! GeN-Foam's pseudo-time eigenvalue driver (gh:#1361, S4).
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! GeN-Foam does not converge an eigenvalue in one call. With
//! `eigenvalueNeutronics true` every **time step** of `GeN-Foam.C` does one flux
//! solve per group (the time-derivative coefficient is `1 - eigenvalueNeutronics
//! = 0`), then `calcKeff.H` updates `keff_ *= pTot / pTarget_` (clamped to
//! `[0.1, 5]`) and `normFluxes.H` rescales the flux to `pTarget_`. The
//! converged `keff_` is the eigenvalue of the discrete operator; the
//! "iteration history" is `keff_` per pseudo-time step. Existing
//! `solve_eigenvalue` methods are a different algorithm (a static power
//! iteration to a tolerance) and are kept untouched.
//!
//! This module holds what is model-independent: the `Time` object of
//! OpenFOAM-ESI as far as the loop uses it ([`PseudoTimeClock`]:
//! `adjustableRunTime` write alignment, `run()`, `operator++`), GeN-Foam's
//! `setDeltaT`/`adjustDeltaT`, the neutronics `maxDeltaT`, and the loop
//! ([`run_pseudo_time_eigenvalue`]). The per-step body lives with each model
//! (`diffusion/pseudo_time.rs`, `sp3/pseudo_time.rs`) behind
//! [`PseudoTimeEigenvalue`]. A generic bound, not a trait object.
//!
//! Scope: the discontinuity factors are 1 (the guided slabs; `dfAdjust` is
//! gh:#1474), no `integralPredictor`/`implicitPredictor`/Aitken (they are
//! transient-only upstream: `!eigenvalueNeutronics_`), and only the neutronics
//! region (`regionSolvers { neutroRegion ... }`) is stepped.

use outram_foam_basic_lib::prelude::{Field, FvMatrix, SolverSettings, VolScalarField};
use thiserror::Error;

/// OpenFOAM `SMALL` (double precision).
pub const SMALL: f64 = 1e-15;
/// OpenFOAM `GREAT` (double precision).
pub const GREAT: f64 = 1e15;
/// OpenFOAM `VSMALL` (double precision).
pub const VSMALL: f64 = 1e-300;
/// OpenFOAM `VGREAT` (double precision).
pub const VGREAT: f64 = 1e300;
/// OpenFOAM `ROOTVGREAT` (double precision).
pub const ROOTVGREAT: f64 = 1e150;
/// `solver::deltaTFactor` (`src/classes/common/solver/solver.C:52`).
pub const DELTA_T_FACTOR: f64 = 1.2;

/// Why a pseudo-time step could not be taken.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PseudoTimeError {
    /// The model was built without cross sections (state-only scaffold).
    #[error("pseudo-time step needs a model built with cross sections")]
    NoCrossSections,
    /// Circulating fuel (`liquidFuel true`): `precEq.H`'s transported branch is
    /// not part of this driver.
    #[error("pseudo-time eigenvalue stepping is implemented for stationary fuel only")]
    LiquidFuel,
}

/// The `controlDict` entries the loop reads (GeN-Foam `GeN-Foam.C`,
/// `setDeltaT.C`, `diffusionNeutronics::maxDeltaT`) and `pTarget` from
/// `0/uniform/reactorState`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PseudoTimeControls {
    /// `deltaT`.
    pub delta_t: f64,
    /// `endTime`.
    pub end_time: f64,
    /// `writeInterval` when `writeControl adjustableRunTime`, else `None`.
    pub write_interval: Option<f64>,
    /// `adjustTimeStep`.
    pub adjust_time_step: bool,
    /// `maxDeltaT` (default `GREAT`).
    pub max_delta_t: f64,
    /// `minDeltaT` (default `VSMALL`).
    pub min_delta_t: f64,
    /// `maxPowerVariation` (default 0.025).
    pub max_power_variation: f64,
    /// `pTarget` of `reactorState`.
    pub p_target: f64,
}

impl PseudoTimeControls {
    /// The `controlDict` of the guided slab tutorials: `adjustTimeStep true`,
    /// `writeControl adjustableRunTime`, `maxDeltaT` and `maxPowerVariation` as
    /// given, `minDeltaT` at its default.
    #[must_use]
    pub fn guided(
        delta_t: f64,
        end_time: f64,
        write_interval: f64,
        max_delta_t: f64,
        max_power_variation: f64,
        p_target: f64,
    ) -> Self {
        Self {
            delta_t,
            end_time,
            write_interval: Some(write_interval),
            adjust_time_step: true,
            max_delta_t,
            min_delta_t: VSMALL,
            max_power_variation,
            p_target,
        }
    }
}

/// OpenFOAM's `Time`, as far as GeN-Foam's neutronics loop uses it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PseudoTimeClock {
    value: f64,
    delta_t: f64,
    time_index: usize,
    write_time_index: i64,
    end_time: f64,
    write_interval: Option<f64>,
}

impl PseudoTimeClock {
    /// `Time` at `startTime 0`, then GeN-Foam's `setDeltaT(runTime, solvers)`
    /// (`min(deltaTValue, controlDict deltaT)` with `adjust = true`).
    #[must_use]
    pub fn new(controls: &PseudoTimeControls) -> Self {
        let mut clock = Self {
            value: 0.0,
            delta_t: controls.delta_t,
            time_index: 0,
            write_time_index: 0,
            end_time: controls.end_time,
            write_interval: controls.write_interval,
        };
        let dt = clock.delta_t.min(controls.delta_t);
        clock.set_delta_t(dt, true);
        clock
    }

    /// Current time value.
    #[must_use]
    pub fn value(&self) -> f64 {
        self.value
    }

    /// Current time step.
    #[must_use]
    pub fn delta_t(&self) -> f64 {
        self.delta_t
    }

    /// Steps taken so far (`timeIndex`).
    #[must_use]
    pub fn time_index(&self) -> usize {
        self.time_index
    }

    /// `Time::run()`: `value() < endTime - 0.5 deltaT`.
    #[must_use]
    pub fn run(&self) -> bool {
        self.value < (self.end_time - 0.5 * self.delta_t)
    }

    /// `Time::setDeltaT(deltaT, adjust)`.
    pub fn set_delta_t(&mut self, delta_t: f64, adjust: bool) {
        self.delta_t = delta_t;
        if adjust {
            self.adjust_delta_t();
        }
    }

    /// `Time::adjustDeltaT()` for `writeControl adjustableRunTime`; a no-op
    /// for any other write control.
    pub fn adjust_delta_t(&mut self) {
        let Some(write_interval) = self.write_interval else {
            return;
        };
        let time_to_next_write =
            (((self.write_time_index + 1) as f64) * write_interval - (self.value - 0.0)).max(0.0);
        let n_steps = time_to_next_write / self.delta_t;
        if n_steps < (i32::MAX as f64) {
            // `max(1, round(nSteps))`, `round` being half away from zero.
            let n_steps_to_next_write = n_steps.round().max(1.0);
            let new_delta_t = time_to_next_write / n_steps_to_next_write;
            if new_delta_t >= self.delta_t {
                self.delta_t = new_delta_t.min(2.0 * self.delta_t);
            } else {
                self.delta_t = new_delta_t.max(0.2 * self.delta_t);
            }
        }
    }

    /// `Time::operator++()`: advance by `deltaT`, reset a time within
    /// `10 SMALL deltaT` of zero, and move the write index.
    pub fn advance(&mut self) {
        self.value += self.delta_t;
        self.time_index += 1;
        if self.value.abs() < 10.0 * SMALL * self.delta_t {
            self.value = 0.0;
        }
        if let Some(write_interval) = self.write_interval {
            let write_index = (((self.value - 0.0) + 0.5 * self.delta_t) / write_interval) as i64;
            if write_index > self.write_time_index {
                self.write_time_index = write_index;
            }
        }
    }
}

/// The neutronics `maxDeltaT()` (`diffusionNeutronics.C:220`, identical in SP3
/// and SN): the largest time step the power variation of the step allows.
///
/// `p_tot` is `power()` after the step, `p_tot_old` the `pTotOld_` taken at the
/// start of `correctPhysics()`.
#[must_use]
pub fn neutronics_max_delta_t(
    controls: &PseudoTimeControls,
    clock: &PseudoTimeClock,
    p_tot: f64,
    p_tot_old: f64,
) -> f64 {
    let new_delta_t = controls.max_delta_t;
    if clock.value() > controls.delta_t {
        let power_variation = ((p_tot - p_tot_old) / (p_tot_old + SMALL)).abs();
        let max_delta_t_neutro_fact = (controls.max_power_variation / (power_variation + SMALL)).abs();
        let delta_t_neutro_fact = (max_delta_t_neutro_fact.min(1.0 + 0.1 * max_delta_t_neutro_fact)).min(1.2);
        return (delta_t_neutro_fact * clock.delta_t()).min(new_delta_t);
    }
    new_delta_t
}

/// GeN-Foam `adjustDeltaT(runTime, solvers)` (`setDeltaT.C`) with the one
/// neutronics solver's `maxDeltaT` already evaluated.
pub fn adjust_delta_t(controls: &PseudoTimeControls, clock: &mut PseudoTimeClock, solver_max_delta_t: f64) {
    if !controls.adjust_time_step {
        return;
    }
    let mut delta_t = VGREAT;
    delta_t = delta_t.min(solver_max_delta_t);
    delta_t = delta_t.max(controls.min_delta_t);
    if delta_t < ROOTVGREAT {
        let next = (DELTA_T_FACTOR * clock.delta_t()).min(delta_t);
        clock.set_delta_t(next, true);
    }
}

/// `neutroEq.solve()` called twice on the same matrix, as `fluxEq.H` and
/// `fluxEqSP3.H` do (`initRes = neutroEq.solve()...` and then
/// `neutroEq.solve()` again inside `residual_ = max(...)`): returns the field
/// after the second solve and the **first** solve's initial residual in
/// OpenFOAM's normalisation (`LduMatrix::openfoam_initial_residual`).
pub(crate) fn solve_twice(
    eqn: &FvMatrix,
    name: &str,
    start: &VolScalarField,
    settings: SolverSettings,
) -> (VolScalarField, f64) {
    let b: Vec<f64> = eqn.source.iter().copied().collect();
    let initial = eqn
        .ldu
        .openfoam_initial_residual(start.internal.as_slice(), &b);
    let (first, _) = eqn.solve_cg_with_guess(name.to_string(), start, settings);
    let (second, _) = eqn.solve_cg_with_guess(name.to_string(), &first, settings);
    (second, initial)
}

/// A cell field of `values` with the boundary of `template`.
pub(crate) fn field_like(template: &VolScalarField, values: Vec<f64>) -> VolScalarField {
    let mut f = template.clone();
    f.internal = Field::new(values);
    f
}

/// `calcKeff.H`: `keff_ *= pTot / pTarget_`, limited to `[0.1, 5]`.
#[must_use]
pub fn calc_keff(keff: f64, p_tot: f64, p_target: f64) -> f64 {
    let mut k = keff;
    k *= p_tot / p_target;
    if k > 5.0 {
        k = 5.0;
    } else if k < 0.1 {
        k = 0.1;
    }
    k
}

/// What one eigenvalue time step reports (the lines the upstream log prints).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PseudoTimeStep {
    /// `power: ...`, `power()` after the flux solves, before `calcKeff.H`.
    pub power: f64,
    /// `keff_ = ...` after `calcKeff.H`.
    pub k_eff: f64,
    /// `neutroResidual (neutroIter: 1)`: the largest initial residual of the
    /// first solve of every equation of the step.
    pub first_residual: f64,
}

/// One row of the pseudo-time history.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PseudoTimeRecord {
    /// `Time = ` of the step (value after `runTime++`).
    pub time: f64,
    /// The `deltaT` the step was taken with.
    pub delta_t: f64,
    /// The step's report.
    pub step: PseudoTimeStep,
}

/// A neutronics model that can take GeN-Foam's eigenvalue time step.
pub trait PseudoTimeEigenvalue {
    /// `power()`: the domain integral of `sum_g flux_g sigmaPow_g`.
    fn pseudo_time_power(&self) -> f64;

    /// `correctPhysics()` for `eigenvalueNeutronics true`, after `pTotOld_` was
    /// taken. `first_step` is `timeIndex - startTimeIndex == 1`.
    ///
    /// # Errors
    ///
    /// [`PseudoTimeError`] if the model cannot be stepped this way.
    fn eigenvalue_time_step(
        &mut self,
        p_target: f64,
        first_step: bool,
    ) -> Result<PseudoTimeStep, PseudoTimeError>;
}

/// GeN-Foam's main time loop for a single neutronics region.
///
/// ```text
/// setDeltaT; while (runTime.run()) { runTime++; correctPhysics(); adjustDeltaT; }
/// ```
///
/// # Errors
///
/// The first [`PseudoTimeError`] a step reports.
pub fn run_pseudo_time_eigenvalue<S: PseudoTimeEigenvalue>(
    solver: &mut S,
    controls: &PseudoTimeControls,
) -> Result<Vec<PseudoTimeRecord>, PseudoTimeError> {
    let mut clock = PseudoTimeClock::new(controls);
    let mut history = Vec::new();
    while clock.run() {
        clock.advance();
        let delta_t = clock.delta_t();
        // `pTotOld_ = power();` first in `correctPhysics()`.
        let p_tot_old = solver.pseudo_time_power();
        let first_step = clock.time_index() == 1;
        let step = solver.eigenvalue_time_step(controls.p_target, first_step)?;
        history.push(PseudoTimeRecord {
            time: clock.value(),
            delta_t,
            step,
        });
        let p_tot = solver.pseudo_time_power();
        let solver_max = neutronics_max_delta_t(controls, &clock, p_tot, p_tot_old);
        adjust_delta_t(controls, &mut clock, solver_max);
    }
    Ok(history)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn controls() -> PseudoTimeControls {
        PseudoTimeControls::guided(1e-9, 60.0, 1.0, 1.0, 0.01, 1e6)
    }

    #[test]
    fn calc_keff_scales_and_clamps_like_calc_keff_h() {
        assert_eq!(calc_keff(1.0, 2.0, 4.0), 0.5);
        assert_eq!(calc_keff(1.0, -3.0, 1.0), 0.1);
        assert_eq!(calc_keff(2.0, 9.0, 1.0), 5.0);
    }

    #[test]
    fn the_clock_starts_at_the_dictionary_step_and_stops_before_end_time() {
        let c = controls();
        let clock = PseudoTimeClock::new(&c);
        // adjustDeltaT: 1 write interval / round(1e9 steps) = 1e-9.
        assert_eq!(clock.delta_t(), 1e-9);
        assert_eq!(clock.value(), 0.0);
        assert!(clock.run());
        let mut late = clock;
        late.value = 60.0 - 0.4 * late.delta_t;
        assert!(!late.run());
    }

    #[test]
    fn the_step_grows_by_the_factor_until_a_write_time_is_near() {
        let c = controls();
        let mut clock = PseudoTimeClock::new(&c);
        clock.advance();
        // first step: value 1e-9 is not > deltaT, so maxDeltaT is the dictionary's
        let m = neutronics_max_delta_t(&c, &clock, 1e6, 1e6);
        assert_eq!(m, 1.0);
        adjust_delta_t(&c, &mut clock, m);
        assert!((clock.delta_t() - 1.2e-9).abs() < 1e-24);
    }

    #[test]
    fn power_variation_limits_the_next_step() {
        let c = controls();
        let mut clock = PseudoTimeClock::new(&c);
        clock.advance();
        clock.advance();
        // 50 % power change: factor = 0.01/0.5 = 0.02 -> min(0.02, 1.002, 1.2)
        let m = neutronics_max_delta_t(&c, &clock, 1.5e6, 1e6);
        assert!((m / clock.delta_t() - 0.02).abs() < 1e-3);
    }

    #[test]
    fn a_non_adjustable_clock_keeps_its_step() {
        let mut c = controls();
        c.write_interval = None;
        let mut clock = PseudoTimeClock::new(&c);
        clock.set_delta_t(0.3, true);
        assert_eq!(clock.delta_t(), 0.3);
        clock.advance();
        assert_eq!(clock.time_index(), 1);
        assert_eq!(clock.value(), 0.3);
    }

    #[test]
    fn adjust_time_step_false_leaves_the_clock_alone() {
        let mut c = controls();
        c.adjust_time_step = false;
        let mut clock = PseudoTimeClock::new(&c);
        let dt = clock.delta_t();
        adjust_delta_t(&c, &mut clock, 5.0);
        assert_eq!(clock.delta_t(), dt);
    }

    /// A model whose power is `k` times the target: `calcKeff` is a fixed
    /// point iteration `k <- k * k0 / 1` that the loop must run to `endTime`.
    struct Fixed {
        k: f64,
        calls: usize,
    }
    impl PseudoTimeEigenvalue for Fixed {
        fn pseudo_time_power(&self) -> f64 {
            1e6
        }
        fn eigenvalue_time_step(
            &mut self,
            p_target: f64,
            _first_step: bool,
        ) -> Result<PseudoTimeStep, PseudoTimeError> {
            self.calls += 1;
            self.k = calc_keff(self.k, 1e6, p_target);
            Ok(PseudoTimeStep { power: 1e6, k_eff: self.k, first_residual: 0.0 })
        }
    }

    #[test]
    fn the_loop_runs_to_the_end_time_and_reports_every_step() {
        let mut m = Fixed { k: 1.0, calls: 0 };
        let h = run_pseudo_time_eigenvalue(&mut m, &controls()).expect("loop");
        assert_eq!(h.len(), m.calls);
        let last = h.last().expect("history");
        assert!((last.time - 60.0).abs() < 1e-9);
        assert!(h.windows(2).all(|w| w[1].time > w[0].time));
    }
}
