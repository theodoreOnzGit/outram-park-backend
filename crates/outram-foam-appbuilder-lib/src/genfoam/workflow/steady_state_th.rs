// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: GeN-Foam/GeN-Foam.C (the time loop), GeN-Foam/include/setDeltaT.C (`setDeltaT`, `adjustDeltaT`),
//                    Tutorials/reactorCases/2D_MSFR/Allrun (`setSteadyStateTH`), OpenFOAM-ESI v2506 `Time::adjustDeltaT`,
//                    `Time::operator++`, `Time::run`
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK. OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free Software Foundation, either
// version 3 of the License, or (at your option) any later version. See <https://www.gnu.org/licenses/>.
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! The `steadyStateTH` stage of `2D_MSFR`: the fluid region alone, flow solved (gh:#1841).
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! `regionSolvers { fluidRegion onePhase; }`, `solveFluidMechanics true`, adjustable time step. One step of
//! `GeN-Foam.C` with a single region and the (empty) `multiRegionCouplingDict_steadyStateTH` mapping:
//! `runTime++`, `correctPhysics`, `adjustDeltaT`. The clock is the workspace's `Time` port
//! ([`PseudoTimeClock`]: the `adjustableRunTime` write alignment of `Time::adjustDeltaT`, `Time::run`'s
//! `endTime - 0.5 deltaT`), so a step that would pass an integer second is shortened to land on it, as upstream's is.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::WorkflowError;
use crate::genfoam::neutronics::pseudo_time::{adjust_delta_t, PseudoTimeClock, PseudoTimeControls, VSMALL};
use crate::genfoam::thermal_hydraulics::solver::two_phase::one_phase_cc::PressureSolve;
use crate::genfoam::thermal_hydraulics::solver::two_phase::one_phase_cc::OnePhaseCcSolver;

/// A deliberate difference from upstream that a stage applied, named so a result cannot carry it silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deviation {
    /// `solvers.p_rgh` selects `GAMG`; `PCG`/`DIC` at the same `tolerance`/`relTol` ran in its place (README round 21:
    /// `outram_foam_basic_lib`'s GAMG is not a translation of `GAMGSolver`; a faithful port is a separate issue).
    PressureGamgSubstitutedByPcgDic,
}

impl Deviation {
    /// One line for a log or a report.
    #[must_use]
    pub fn describe(&self) -> &'static str {
        match self {
            Self::PressureGamgSubstitutedByPcgDic => {
                "p_rgh: GAMG/DIC replaced by PCG/DIC at the entry's tolerance and relTol (no faithful GAMG in the port)"
            }
        }
    }
}

/// The `controlDict` entries of the `steadyStateTH` stage after `Allrun`'s edits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThRunControls {
    /// `endTime` (`endTimeTH`, 15 in `2D_MSFR`). The stage starts at `startTime 0`.
    pub end_time: f64,
    /// `deltaT` (1e-3 in `2D_MSFR`).
    pub delta_t: f64,
    /// `adjustTimeStep` (true).
    pub adjust_time_step: bool,
    /// `writeInterval` of `writeControl adjustableRunTime` (1 in `2D_MSFR`); `None` for any other write control, which
    /// aligns no step to a write time.
    pub write_interval: Option<f64>,
}

/// One step of the stage.
#[derive(Debug, Clone)]
pub struct ThRecord {
    /// `runTime.value()` after the step.
    pub time: f64,
    /// `deltaT` of the step.
    pub delta_t: f64,
    /// `deltaT` after `adjustDeltaT` (the next step's).
    pub next_delta_t: f64,
    /// `true` when `Time::operator++` made this a write time.
    pub write_time: bool,
    /// Initial residual of each of the `h` solves.
    pub h_residuals: Vec<f64>,
    /// `T (avg min max)` after each outer iteration.
    pub t_stats: Vec<(f64, f64, f64)>,
    /// Every `p_rgh` solve of the step.
    pub pressure_solves: Vec<PressureSolve>,
    /// Courant number (max) with this step's `deltaT`.
    pub courant_max: f64,
}

/// The fluid region of the `steadyStateTH` stage.
#[derive(Debug, Clone)]
pub struct SteadyStateTh {
    /// The `onePhase` region, flow solved.
    pub fluid: OnePhaseCcSolver,
}

impl SteadyStateTh {
    /// Wrap a fluid region built at `startTime 0` with `solveFluidMechanics true`.
    ///
    /// # Errors
    ///
    /// [`WorkflowError::Case`] if the region does not solve the flow or does not start at 0.
    pub fn new(fluid: OnePhaseCcSolver) -> Result<Self, WorkflowError> {
        if fluid.case().flow.is_none() {
            return Err(WorkflowError::Case("steadyStateTH needs solveFluidMechanics true".into()));
        }
        if fluid.time() != 0.0 {
            return Err(WorkflowError::Case("steadyStateTH starts at startTime 0 (the write-time clock is anchored there)".into()));
        }
        Ok(Self { fluid })
    }

    /// The deviations this stage carries.
    #[must_use]
    pub fn deviations(&self) -> Vec<Deviation> {
        let mut out = Vec::new();
        if self.fluid.case().flow.as_ref().is_some_and(|f| f.p_rgh_gamg_substituted) {
            out.push(Deviation::PressureGamgSubstitutedByPcgDic);
        }
        out
    }

    /// Run to `endTime`: `while (runTime.run()) { runTime++; correctPhysics; adjustDeltaT }`. `on_step` sees each record
    /// and the stage (to sample fields at write times).
    ///
    /// # Errors
    ///
    /// None at present (the step cannot fail); the `Result` is the stage interface.
    pub fn run<F: FnMut(&ThRecord, &Self)>(
        &mut self,
        controls: &ThRunControls,
        mut on_step: F,
    ) -> Result<Vec<ThRecord>, WorkflowError> {
        let clock_controls = PseudoTimeControls {
            delta_t: controls.delta_t,
            end_time: controls.end_time,
            write_interval: controls.write_interval,
            adjust_time_step: controls.adjust_time_step,
            max_delta_t: self.fluid.case().controls.max_delta_t,
            min_delta_t: VSMALL,
            max_power_variation: 0.025,
            p_target: 1.0,
        };
        let mut clock = PseudoTimeClock::new(&clock_controls);
        let mut records = Vec::new();
        let mut write_index = 0_i64;
        while clock.run() {
            let dt = clock.delta_t();
            clock.advance();
            let write_time = match controls.write_interval {
                Some(wi) => {
                    let idx = ((clock.value() + 0.5 * dt) / wi) as i64;
                    let w = idx > write_index;
                    if w {
                        write_index = idx;
                    }
                    w
                }
                None => false,
            };
            self.fluid.begin_time_step(dt);
            let (_residual, report) = self.fluid.correct_physics();
            let (courant_max, _) = self.fluid.courant();
            adjust_delta_t(&clock_controls, &mut clock, self.fluid.max_delta_t());
            let rec = ThRecord {
                time: self.fluid.time(),
                delta_t: dt,
                next_delta_t: clock.delta_t(),
                write_time,
                h_residuals: report.h_residuals,
                t_stats: report.t_stats,
                pressure_solves: self.fluid.pressure_solves().to_vec(),
                courant_max,
            };
            on_step(&rec, self);
            records.push(rec);
        }
        Ok(records)
    }
}
