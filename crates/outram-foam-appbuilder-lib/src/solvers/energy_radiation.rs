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
//! ~~The radiative wall flux `qr` is computed and stored (`state.qr`) but is **not** fed to
//! the coupled interface condition (upstream's `turbulentTemperatureRadCoupledMixed`
//! `qr`/`qrNbr` terms): [`crate::solvers::cht_multi_region_foam::CoupledTemperature`] has
//! none.~~ **CORRECTED 2026-11-14 (gh:#1531, qr slice):** `qr` (`state.qr`) now enters the coupled
//! interface condition as `refGrad = (qr + qrNbr)/kappa` on both sides, by default whenever a
//! fluid region has radiation attached
//! ([`crate::solvers::cht_multi_region_foam::InterfaceRadiation`]); ablate with
//! `without_radiative_flux`. Bit-identical to ESI v2506 on the gh:#1531 case with `qr`/`qrNbr`
//! (`verification_and_validation/upstream_radiation_gh1531/README.md`, "qr coupling").
//! Not here: variable properties, `fvDOM`, `viewFactor`, scattering and a non-orthogonal
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solvers::energy::EnergyEquation;
    use outram_foam_basic_lib::radiation::{stefan_boltzmann_esi_v2506, ConstantAbsorptionEmission, P1PatchModel};
    use std::sync::Arc;
    use uom::si::f64::{Length, Time};
    use uom::si::length::meter;
    use uom::si::time::second;

    fn slab(n: usize) -> Arc<FvMesh> {
        Arc::new(crate::solvers::cht_multi_region_foam::line_mesh(n, Length::new::<meter>(0.1), Length::new::<meter>(0.0), ["left", "right"]))
    }

    fn model(a: f64, walls: P1PatchModel) -> P1Model {
        P1Model::new(
            ConstantAbsorptionEmission { absorptivity: a, emissivity: a, emission_source: 0.0 },
            vec![walls.clone(), walls, P1PatchModel::ZeroGradient],
        )
    }

    fn step(e: &mut EnergyEquation, dt: f64) {
        let old = e.t.clone();
        e.begin_time_step();
        e.solve(&old, None, Time::new::<second>(dt), crate::solvers::non_orthogonal::LaplacianCorrection::Corrected, 0).unwrap();
    }

    /// Off by default (the new field is `None`), and a model with the wrong patch count is refused.
    #[test]
    fn radiation_is_off_by_default_and_a_bad_model_is_refused() {
        let mesh = slab(4);
        let e = EnergyEquation::uniform(mesh.clone(), IncompressibleThermo::si(1000.0, 4000.0, 0.6), 300.0);
        assert!(e.radiation.is_none());
        let bad = P1Model::new(ConstantAbsorptionEmission { absorptivity: 1.0, emissivity: 1.0, emission_source: 0.0 }, vec![]);
        assert!(e.with_radiation(bad, 0.0).is_err());
    }

    /// Radiative equilibrium: a medium at T between walls at T, with G = 4 sigma T^4, has Ru = Rp T^4
    /// and no net source, so T does not move.
    #[test]
    fn radiative_equilibrium_leaves_t_unchanged() {
        let mesh = slab(10);
        let sigma = stefan_boltzmann_esi_v2506();
        let t0 = 800.0;
        let mut e = EnergyEquation::uniform(mesh.clone(), IncompressibleThermo::si(1.0, 1000.0, 1.0), t0)
            .with_radiation(model(2.0, P1PatchModel::MarshakFixedTemperature { emissivity: 1.0, t_rad: t0 }), 4.0 * sigma * t0.powi(4))
            .unwrap();
        e.set_patch("left", BoundaryCondition::FixedValue(t0));
        e.set_patch("right", BoundaryCondition::FixedValue(t0));
        for _ in 0..5 {
            step(&mut e, 1.0);
        }
        for c in 0..10 {
            assert!((e.t.internal[c] - t0).abs() < 1e-9, "cell {c}: {}", e.t.internal[c] - t0);
        }
        assert_eq!(e.radiation.as_ref().unwrap().time_index(), 5);
    }

    /// First step from G = 0 in an adiabatic, uniform medium: dT = -Rp T^4 dt/(rho cp) / (1 + 4 Rp T^3 dt/(rho cp))
    /// in every cell (the implicit linearisation of -Rp T^4; derived in PREDICTIONS.md item 1).
    #[test]
    fn first_step_cooling_from_zero_g_matches_the_linearised_formula() {
        let mesh = slab(6);
        let (rho, cp, a, t0, dt) = (1000.0, 4181.0, 50.0, 300.0, 0.1);
        let mut e = EnergyEquation::uniform(mesh.clone(), IncompressibleThermo::si(rho, cp, 0.6), t0)
            .with_radiation(model(a, P1PatchModel::Marshak { emissivity: 1.0 }), 0.0)
            .unwrap();
        step(&mut e, dt);
        let rp = 4.0 * a * stefan_boltzmann_esi_v2506();
        let k = rho * cp;
        let want = -(rp * t0.powi(4) * dt / k) / (1.0 + 4.0 * rp * t0.powi(3) * dt / k);
        for c in 0..6 {
            assert!(((e.t.internal[c] - t0) / want - 1.0).abs() < 1e-9, "cell {c}: {} vs {want}", e.t.internal[c] - t0);
        }
        // The G solve ran after the energy solve: G is now near the emission level, no longer 0.
        let g = e.radiation.as_ref().unwrap().state.g.internal[3];
        assert!(g > 1000.0, "{g}");
    }

    /// `solverFreq`: the first call always solves, then every `solver_freq`-th time index.
    #[test]
    fn solver_freq_follows_the_time_index() {
        let mesh = slab(4);
        let e = EnergyEquation::uniform(mesh.clone(), IncompressibleThermo::si(1.0, 1.0, 1.0), 500.0);
        let mut r = RadiationCoupling::new(model(1.0, P1PatchModel::Marshak { emissivity: 1.0 }), &e.t, 0.0).unwrap();
        r.solver_freq = 3;
        let mut due = Vec::new();
        for _ in 0..7 {
            r.begin_time_step();
            due.push(r.correct(&e.t));
        }
        assert_eq!(due, vec![true, false, true, false, false, true, false]);
        r.solver_freq = 0;
        assert!(r.correct(&e.t));
    }
}
