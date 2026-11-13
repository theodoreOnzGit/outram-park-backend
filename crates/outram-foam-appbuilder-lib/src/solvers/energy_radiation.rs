// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com), ESI v2506
//   Copyright (C) 2011-2017 OpenFOAM Foundation
//   Copyright (C) 2016-2023 OpenCFD Ltd.
// Upstream sources read for this file (ESI v2506):
//   applications/solvers/heatTransfer/chtMultiRegionFoam/fluid/EEqn.H
//       (`== ... + rad.Sh(thermo, he)`, then `EEqn.solve`, `thermo.correct()`,
//       `rad.correct()`, in that order)
//   src/thermophysicalModels/radiation/radiationModels/radiationModel/radiationModel.C
//       (`correct()`: `firstIter_ || timeIndex % solverFreq_ == 0`; `Sh`)
//   src/thermophysicalModels/specie/thermo/hConst/hConstThermoI.H
//       (`Hs = Cp (T - Tref) + Hsref + EquationOfState::H`, `Tref = Tstd` by default)
// Upstream licence: GPL-3.0-or-later.

//! # Opt-in P1 radiation in the incompressible energy equation (gh:#1531, epic gh:#1379)
//!
//! [`RadiationCoupling`] is attached to an [`EnergyEquation`](crate::solvers::energy::EnergyEquation)
//! with [`EnergyEquation::with_radiation`](crate::solvers::energy::EnergyEquation::with_radiation).
//! **Off by default**: an energy equation without it is bit for bit what it was before
//! this module (pinned by `radiation_is_off_by_default_and_changes_nothing` in the
//! `solvers::energy` tests and by every existing differential).
//!
//! ## What upstream does, and what is here
//!
//! ESI's fluid `EEqn.H` assembles `... - laplacian(alphaEff, he) == ... + rad.Sh(thermo, he)`,
//! solves it, calls `thermo.correct()` and then `rad.correct()`:
//!
//! 1. The source of the step is evaluated from the radiation state of the **previous**
//!    `correct()` (the incident radiation `G`, lagged) and the **current** `T`/`he`
//!    (`Sh = Ru - Sp(4 Rp T^3/Cpv, he) - Rp T^3 (T - 4 he/Cpv)`: implicit in `he`,
//!    linearised about the current `T`). Before the first `correct()` the `G` read from
//!    `0/G` is used ([`RadiationCoupling::new`]'s `g0`).
//! 2. After the solve, `P1::calculate` re-solves the `G` equation for the new `T`
//!    (and the new wall temperatures), warm-started from the old `G`
//!    ([`P1Model::solve_from`](outram_foam_basic_lib::radiation::P1Model::solve_from)), when
//!    `firstIter_ || timeIndex % solverFreq == 0`. `correct()` runs once per energy solve,
//!    so once per outer corrector of a multi-pass step, exactly where upstream's
//!    `EEqn.H` calls it.
//!
//! ## From the `he` equation to this crate's `T` equation
//!
//! [`EnergyEquation`](crate::solvers::energy::EnergyEquation) solves the energy equation
//! per unit `rho c_p` for `T`. Upstream's source acts on `he = c_p (T - T_ref)` (hConst,
//! constant `c_p`, `p` at the reference pressure so the `EquationOfState::H` term is 0).
//! Substituting that into the matrix of [`P1Solution::sh_matrix`] and dividing by `rho c_p`:
//! `diag_T = diag_h / rho` and `source_T = source_h / (rho c_p) + (diag_h / rho) T_ref`.
//! `T_ref` cancels analytically (it only moves a constant between the implicit and the
//! explicit part) and is kept at ESI's `Tstd = 298.15 K` so that the round-off follows
//! upstream's; [`RadiationCoupling::reference_temperature`] can change it.
//!
//! ## Not here
//!
//! The radiative wall flux `qr` is computed and stored (`state.qr`) but is **not** fed to
//! the coupled interface condition (upstream's `turbulentTemperatureRadCoupledMixed`
//! `qr`/`qrNbr` terms): [`crate::solvers::cht_multi_region_foam::CoupledTemperature`] has
//! none. Variable properties, `fvDOM`, `viewFactor`, scattering and a non-orthogonal
//! correction of the `G` Laplacian are not ported (gh:#1379).

use crate::error::AppBuilderError;
use crate::solvers::energy::IncompressibleThermo;
use outram_foam_basic_lib::prelude::*;
use outram_foam_basic_lib::radiation::{P1Model, P1Solution};

/// ESI's `Tstd` \[K\], the default `Tref` of `hConst`.
pub const T_STD_ESI: f64 = 298.15;

/// The P1 radiation state and settings of one fluid energy equation.
#[derive(Debug, Clone)]
pub struct RadiationCoupling {
    /// The P1 model: medium and one entry per patch of the region's mesh.
    pub model: P1Model,
    /// The `G` linear solver (upstream's `solvers { G { ... } }`). Default: PCG/DIC to
    /// 1e-10, as the V&V case of gh:#1379.
    pub settings: SolverSettings,
    /// Solve `G` every `solver_freq`-th time step (upstream `solverFreq`, default 1);
    /// the first call always solves (`firstIter_`).
    pub solver_freq: usize,
    /// `T_ref` of `he = c_p (T - T_ref)` \[K\]; default [`T_STD_ESI`]. Analytically irrelevant.
    pub reference_temperature: f64,
    /// The radiation state of the last `correct()`: `G`, `qr`, `a`, `e`, `E`.
    pub state: P1Solution,
    time_index: usize,
    first_iter: bool,
}

impl RadiationCoupling {
    /// A coupling for the temperature field `t` with uniform initial incident radiation
    /// `g0` \[W/m2\] (upstream's `0/G` internal field).
    ///
    /// # Errors
    /// [`AppBuilderError::Case`] if `model` does not have one patch entry per patch of `t`'s mesh.
    pub fn new(model: P1Model, t: &VolScalarField, g0: f64) -> Result<Self, AppBuilderError> {
        if model.patches.len() != t.mesh.patches.len() {
            return Err(AppBuilderError::Case {
                case: std::path::PathBuf::from("energy radiation"),
                msg: format!("P1 model has {} patch entries, the mesh has {} patches", model.patches.len(), t.mesh.patches.len()),
            });
        }
        let state = model.initial_solution(t, g0);
        Ok(Self {
            model,
            settings: SolverSettings { tolerance: 1e-10, max_iter: 1000 },
            solver_freq: 1,
            reference_temperature: T_STD_ESI,
            state,
            time_index: 0,
            first_iter: true,
        })
    }

    /// Count a new time step (upstream's `runTime.timeIndex()`), which `solver_freq` reads.
    pub fn begin_time_step(&mut self) {
        self.time_index += 1;
    }

    /// The time steps counted so far.
    #[must_use]
    pub fn time_index(&self) -> usize {
        self.time_index
    }

    /// `rad.Sh(thermo, he)` as a term of the per-`rho c_p` temperature equation (module docs),
    /// from the lagged radiation state and the current `t`.
    #[must_use]
    pub fn energy_source(&self, t: &VolScalarField, thermo: &IncompressibleThermo) -> FvMatrix {
        let n = t.mesh.n_cells;
        let cp = thermo.specific_heat.get::<uom::si::specific_heat_capacity::joule_per_kilogram_kelvin>();
        let rho = thermo.density.get::<uom::si::mass_density::kilogram_per_cubic_meter>();
        let mut sol = self.state.clone();
        sol.t = t.clone();
        let cpv = vec![cp; n];
        let he: Vec<f64> = (0..n).map(|c| cp * (t.internal[c] - self.reference_temperature) + 0.0).collect();
        let mut m = sol.sh_matrix(&cpv, &he);
        let tref = self.reference_temperature;
        for c in 0..n {
            let diag_t = m.ldu.diag[c] / rho;
            m.source[c] = m.source[c] / (rho * cp) + diag_t * tref;
            m.ldu.diag[c] = diag_t;
        }
        m
    }

    /// `rad.correct()`: re-solve `G` for `t` (cells and boundary values), warm-started from the
    /// current `G`, if this is the first call or the time index is a multiple of `solver_freq`.
    /// Returns whether it solved.
    pub fn correct(&mut self, t: &VolScalarField) -> bool {
        let due = self.first_iter || self.solver_freq == 0 || self.time_index % self.solver_freq == 0;
        if due {
            let g_prev = self.state.g.internal.as_slice().to_vec();
            self.state = self.model.solve_from(t, Some(&g_prev), self.settings);
            self.first_iter = false;
        }
        due
    }
}
