// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (openfoam.org)
//   Copyright (C) 2011-2020 OpenFOAM Foundation
// Upstream project: OpenFOAM-8 (OpenFOAM Foundation), github.com/OpenFOAM/OpenFOAM-8,
//   branch master, fetched 2026-11-03 to a scratch directory (not vendored).
// Upstream sources read for this file:
//   applications/solvers/heatTransfer/chtMultiRegionFoam/fluid/EEqn.H
//       (the fluid energy equation: ddt + div(phi, he) + divq(he) == fvOptions)
//   applications/solvers/heatTransfer/chtMultiRegionFoam/solid/solveSolid.H
//       (the solid energy equation: ddt(rho, e) - laplacian(alpha, e), with
//       the pimple.correctNonOrthogonal() loop)
// Upstream licence: GPL-3.0-or-later.
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

//! # The incompressible energy equation (gh:#1156, Foam Studio stage 2)
//!
//! For a fluid with constant density ρ, specific heat c_p and conductivity k,
//! and for a solid with the same three constants, the energy equation of
//! chtMultiRegionFoam's `fluid/EEqn.H` and `solid/solveSolid.H`, divided by
//! ρ c_p, is one equation for the temperature T \[K\]:
//!
//! `∂T/∂t + ∇·(φ T) − ∇·(α ∇T) = 0`, with `α = k / (ρ c_p)` \[m²/s\],
//!
//! where φ is the volumetric face flux \[m³/s\] in a fluid and zero in a solid.
//!
//! ## What upstream has that this does not, and why that is right here
//!
//! Upstream's fluid `EEqn` carries the kinetic energy `ddt(ρ,K) + div(φ,K)`,
//! the pressure work `−dpdt` (or `div(φ/ρ, p)` for the e-form), buoyancy work
//! `ρ U·g`, radiation and reactions. For the incompressible, constant-property
//! forced convection this module is for, the kinetic-energy and pressure-work
//! terms are the viscous-heating order (Eckert number ~1e-9 at the demo's
//! 5 mm/s and 50 K), there is no gravity in the PIMPLE loop and no radiation
//! or chemistry. They are therefore **not represented**, and this module does
//! not claim to carry them. Viscous dissipation is likewise absent (upstream's
//! `EEqn` has no explicit dissipation term either; it enters through `K`).
//!
//! ## Discretisation
//!
//! - `ddt`: `Euler` or `steadyState` ([`EnergyEquation::ddt`]); the steady
//!   form is chtMultiRegionSimpleFoam's.
//! - `div(φ, T)`: `Gauss upwind` (the only scalar convection scheme
//!   `outram-foam-basic-lib` has; bounded, first order).
//! - `laplacian(α, T)`: the same `Gauss linear corrected` / `orthogonal`
//!   choice as the momentum and pressure Laplacians
//!   ([`crate::solvers::non_orthogonal`], gh:#1161), with the explicit
//!   correction lagged and `nNonOrthogonalCorrectors` passes, as in
//!   upstream's `solveSolid.H` loop. **On by default**: an `EnergyEquation`
//!   attached to [`PimpleFoam`](crate::solvers::pimple_foam::PimpleFoam)
//!   follows the flow's `default_laplacian`, whose default is `corrected`.
//! - The linear solve is BiCGStab warm-started from the current T, at
//!   tolerance 1e-12 by default: `MeltFoam` measured a 0.92 % energy leak
//!   over 10 000 steps at 1e-7, and an energy-balance V&V needs the solve's
//!   residual well below what it checks.
//!
//! ## Units
//!
//! The matrix is per unit ρ c_p (m³/s·K per cell); [`IncompressibleThermo::rho_cp`]
//! converts a matrix flux to watts, which is what the boundary diagnostics
//! here return.

use crate::error::AppBuilderError;
use crate::io::fv_schemes::DdtScheme;
use crate::solvers::bc_util::capture_bcs;
use crate::solvers::energy_radiation::RadiationCoupling;
use outram_foam_basic_lib::radiation::P1Model;
use crate::solvers::non_orthogonal::{self, LaplacianCorrection};
use outram_foam_basic_lib::prelude::*;
use std::sync::Arc;
use uom::si::f64::{MassDensity, Power, SpecificHeatCapacity, ThermalConductivity, Time};
use uom::si::time::second;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::power::watt;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;

/// Constant thermophysical properties of one region (fluid or solid):
/// upstream's `constant` transport + `hConst` thermo, without the equation of
/// state (the flow here is incompressible).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IncompressibleThermo {
    /// Density ρ.
    pub density: MassDensity,
    /// Specific heat c_p (a solid's c).
    pub specific_heat: SpecificHeatCapacity,
    /// Thermal conductivity k (laminar: the molecular value, upstream's
    /// `kappaEff` with no turbulent part).
    pub conductivity: ThermalConductivity,
}

impl IncompressibleThermo {
    /// Properties from SI numbers: ρ \[kg/m³\], c_p \[J/(kg K)\], k \[W/(m K)\].
    #[must_use]
    pub fn new(density: MassDensity, specific_heat: SpecificHeatCapacity, conductivity: ThermalConductivity) -> Self {
        Self { density, specific_heat, conductivity }
    }

    /// Properties with the given conductivity and diffusivity-defining ρ c_p:
    /// `IncompressibleThermo::si(rho, cp, k)` with plain SI numbers.
    #[must_use]
    pub fn si(rho: f64, cp: f64, k: f64) -> Self {
        Self::new(
            MassDensity::new::<kilogram_per_cubic_meter>(rho),
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(cp),
            ThermalConductivity::new::<watt_per_meter_kelvin>(k),
        )
    }

    /// ρ c_p \[J/(m³ K)\].
    #[must_use]
    pub fn rho_cp(&self) -> f64 {
        self.density.get::<kilogram_per_cubic_meter>() * self.specific_heat.get::<joule_per_kilogram_kelvin>()
    }

    /// k \[W/(m K)\].
    #[must_use]
    pub fn kappa(&self) -> f64 {
        self.conductivity.get::<watt_per_meter_kelvin>()
    }

    /// Thermal diffusivity α = k / (ρ c_p) \[m²/s\].
    #[must_use]
    pub fn diffusivity(&self) -> f64 {
        self.kappa() / self.rho_cp()
    }
}

/// The temperature field of one region with its properties and solve
/// settings. Attach one to
/// [`PimpleFoam::energy`](crate::solvers::pimple_foam::PimpleFoam::energy)
/// for forced convection, or hold one in a
/// [`SolidRegion`](crate::solvers::cht_multi_region_foam::SolidRegion).
#[derive(Debug, Clone)]
pub struct EnergyEquation {
    /// Temperature \[K\], with its boundary conditions.
    pub t: VolScalarField,
    /// Constant properties of the region.
    pub thermo: IncompressibleThermo,
    /// Time scheme of the energy equation: `Euler` (default) or
    /// `SteadyState` (chtMultiRegionSimpleFoam's steady form). Any other is
    /// a hard error at the solve.
    pub ddt: DdtScheme,
    /// Linear-solver settings; default tolerance 1e-12 (module docs).
    pub solver: SolverSettings,
    /// Opt-in P1 thermal radiation (gh:#1531, [`crate::solvers::energy_radiation`]); `None` by
    /// default, which leaves the equation exactly as it was.
    pub radiation: Option<RadiationCoupling>,
}

/// What one energy solve did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnergySolve {
    /// Largest cell |T_new − T_before| \[K\] over the solve.
    pub max_change: f64,
    /// The linear solver's report from the last non-orthogonal pass.
    pub iterations: usize,
    /// Its final residual.
    pub residual: f64,
}

impl EnergyEquation {
    /// A uniform temperature `t0` \[K\] on `mesh`, every patch zeroGradient
    /// (adiabatic); set the boundary conditions on `t.boundary` afterwards.
    #[must_use]
    pub fn uniform(mesh: Arc<FvMesh>, thermo: IncompressibleThermo, t0: f64) -> Self {
        let bcs = mesh.patches.iter().map(|p| PatchField::zero_gradient(p.size)).collect();
        let n = mesh.n_cells;
        Self {
            t: VolScalarField::new("T", mesh, Field::new(vec![t0; n]), bcs),
            thermo,
            ddt: DdtScheme::Euler,
            solver: SolverSettings { tolerance: 1e-12, max_iter: 2_000 },
            radiation: None,
        }
    }

    /// Opt in to P1 thermal radiation (gh:#1531): the equation then carries ESI's
    /// `rad.Sh(thermo, he)` source and calls `rad.correct()` after each solve, with the
    /// incident radiation starting at `g0` \[W/m2\] everywhere. Not for a solid region (upstream
    /// has no P1 there). See [`crate::solvers::energy_radiation`].
    ///
    /// # Errors
    /// [`AppBuilderError::Case`] if `model` has not one patch entry per mesh patch.
    pub fn with_radiation(mut self, model: P1Model, g0: f64) -> Result<Self, AppBuilderError> {
        self.radiation = Some(RadiationCoupling::new(model, &self.t, g0)?);
        Ok(self)
    }

    /// Count a new time step for the radiation's `solverFreq` (a no-op without radiation).
    pub fn begin_time_step(&mut self) {
        if let Some(r) = self.radiation.as_mut() {
            r.begin_time_step();
        }
    }

    /// Set patch `name`'s condition (no-op if there is no such patch).
    pub fn set_patch(&mut self, name: &str, bc: BoundaryCondition<f64>) {
        let mesh = self.t.mesh.clone();
        if let Some(pi) = mesh.patches.iter().position(|p| p.name == name) {
            let size = mesh.patches[pi].size;
            let v = match &bc {
                BoundaryCondition::FixedValue(v) => *v,
                _ => self.t.internal.as_slice().first().copied().unwrap_or(0.0),
            };
            self.t.boundary[pi] = PatchField { bc, values: Field::uniform(size, v) };
        }
    }

    /// Assemble and solve the energy equation once, with old-time field
    /// `t_old` and face flux `phi` (`None` in a solid), at time step `dt`
    /// (ignored by `SteadyState`),
    /// with Laplacian treatment `lap` and `n_non_orth` extra non-orthogonal
    /// passes (upstream `pimple.correctNonOrthogonal()`).
    ///
    /// # Errors
    /// [`AppBuilderError::UnsupportedScheme`] for a ddt scheme other than
    /// `Euler` / `SteadyState`.
    pub fn solve(
        &mut self,
        t_old: &VolScalarField,
        phi: Option<&SurfaceScalarField>,
        dt: Time,
        lap: LaplacianCorrection,
        n_non_orth: usize,
    ) -> Result<EnergySolve, AppBuilderError> {
        let mesh = self.t.mesh.clone();
        let bcs = capture_bcs(&self.t.boundary);
        let name = self.t.name.clone();
        let dt = dt.get::<second>();
        let before = self.t.internal.as_slice().to_vec();
        let alpha = VolScalarField::uniform("alphaT", mesh.clone(), self.thermo.diffusivity());
        let alpha_f = fvc::interpolate(&alpha);
        let mut perf = SolverPerformance { n_iterations: 0, final_residual: 0.0, converged: true };
        // `rad.Sh(thermo, he)`: assembled once, from the lagged G and the T at entry, as in EEqn.H.
        // FvMatrix is not Clone, so it is rebuilt per pass from the entry temperature.
        let t_entry = self.radiation.as_ref().map(|_| self.t.clone());
        for _ in 0..=n_non_orth {
            let mut eqn = fvm::laplacian_with_delta(&alpha_f, &self.t, lap.delta_coeff());
            if lap.is_explicit() {
                let mut c = non_orthogonal::scalar_face_correction(&alpha_f, &fvc::grad(&self.t), &mesh);
                lap.limit_scalar(&mut c, &alpha_f, &self.t, &mesh);
                non_orthogonal::add_to_source_scalar(eqn.source.as_mut_slice(), &c, &mesh);
            }
            match &self.ddt {
                DdtScheme::Euler => eqn = eqn + fvm::ddt(&self.t, t_old, dt),
                DdtScheme::SteadyState => {}
                other => {
                    return Err(AppBuilderError::UnsupportedScheme {
                        family: "ddtSchemes (energy)",
                        scheme: format!("{other:?}"),
                        reason: "the energy equation implements Euler and steadyState only",
                    })
                }
            }
            if let Some(phi) = phi {
                eqn = eqn + fvm::div(phi, &self.t);
            }
            if let (Some(r), Some(te)) = (self.radiation.as_ref(), t_entry.as_ref()) {
                eqn = eqn + r.energy_source(te, &self.thermo);
            }
            let (mut t_new, p) = eqn.solve_bicgstab_with_guess(name.clone(), &self.t, KrylovOptions::default(), self.solver);
            perf = p;
            refine(&eqn, &mut t_new, &mut perf, self.solver);
            t_new.name = name.clone();
            for (pf, bc) in t_new.boundary.iter_mut().zip(&bcs) {
                pf.bc = bc.clone();
            }
            evaluate_boundary_values(&mut t_new);
            self.t = t_new;
        }
        // `rad.correct()`, after `thermo.correct()`.
        if let Some(r) = self.radiation.as_mut() {
            r.correct(&self.t);
        }
        let max_change = before.iter().zip(self.t.internal.as_slice()).fold(0.0_f64, |a, (b, c)| a.max((c - b).abs()));
        Ok(EnergySolve { max_change, iterations: perf.n_iterations, residual: perf.final_residual })
    }

    /// Conduction into the region through patch `pi` \[W\], from the same
    /// boundary arms `fvm::laplacian` assembles (so it is the heat the
    /// matrix saw), with boundary distance from `lap`. Zero on zeroGradient,
    /// empty and cyclic patches.
    #[must_use]
    pub fn conduction_in(&self, pi: usize, lap: LaplacianCorrection) -> Power {
        let k = self.thermo.kappa();
        let q = (0..self.t.mesh.patches[pi].size).map(|fi| face_conduction_in(&self.t, pi, fi, k, lap.delta_coeff())).sum();
        Power::new::<watt>(q)
    }

    /// Conduction into the region through face `fi` of patch `pi` \[W\];
    /// the per-face term of [`Self::conduction_in`].
    #[must_use]
    pub fn face_conduction(&self, pi: usize, fi: usize, lap: LaplacianCorrection) -> Power {
        Power::new::<watt>(face_conduction_in(&self.t, pi, fi, self.thermo.kappa(), lap.delta_coeff()))
    }

    /// Enthalpy carried **out** through patch `pi` by flux `phi` \[W\],
    /// `ρ c_p Σ φ_f T_f` with the face value the upwind operator uses
    /// (owner cell on outflow, the boundary value on inflow), relative to
    /// 0 K. Negative for an inlet.
    #[must_use]
    pub fn enthalpy_out(&self, pi: usize, phi: &SurfaceScalarField) -> Power {
        let mesh = &self.t.mesh;
        let patch = &mesh.patches[pi];
        let mut h = 0.0;
        for fi in 0..patch.size {
            let f = phi.boundary[pi].values[fi];
            let tp = self.t.internal[mesh.owner[patch.start + fi]];
            let tf = if f >= 0.0 { tp } else { inflow_value(&self.t.boundary[pi].bc, fi, tp) };
            h += f * tf;
        }
        Power::new::<watt>(self.thermo.rho_cp() * h)
    }

    /// Total thermal energy `ρ c_p Σ V T` \[J\] relative to 0 K.
    #[must_use]
    pub fn stored_energy(&self) -> f64 {
        let mesh = &self.t.mesh;
        self.thermo.rho_cp() * (0..mesh.n_cells).map(|c| mesh.cell_volumes[c] * self.t.internal[c]).sum::<f64>()
    }

    /// Mixing-cup (flux-weighted) temperature over patch `pi` \[K\], the face
    /// value as in [`Self::enthalpy_out`]; `None` if no flux crosses it.
    #[must_use]
    pub fn mixing_cup(&self, pi: usize, phi: &SurfaceScalarField) -> Option<f64> {
        let mesh = &self.t.mesh;
        let patch = &mesh.patches[pi];
        let (mut s, mut w) = (0.0, 0.0);
        for fi in 0..patch.size {
            let f = phi.boundary[pi].values[fi];
            let tp = self.t.internal[mesh.owner[patch.start + fi]];
            let tf = if f >= 0.0 { tp } else { inflow_value(&self.t.boundary[pi].bc, fi, tp) };
            s += f * tf;
            w += f;
        }
        (w.abs() > 0.0).then(|| s / w)
    }
}

/// Iterative refinement of a BiCGStab solve (gh:#1156).
///
/// `outram-foam-basic-lib`'s BiCGStab tests breakdown with an **absolute**
/// threshold (`|ρ| < 1e-30`, ρ ~ ‖r‖²). This equation's right-hand side is
/// small in matrix units (‖b‖ ≈ 2e-5 m³K/s on the Graetz pipe), so the solve
/// "breaks down" once ‖r‖ ≈ 1e-15, a *relative* residual of ≈ 4e-11, and
/// returns `converged = false` above the requested 1e-12. Measured
/// 2026-11-03 on the Graetz pipe (m = 3): true residuals 1e-11 to 5e-11,
/// leaving an energy imbalance of 3.7e-8 of the wall heat (Σ r over the
/// cells, which all carry the same sign). Up to four rounds of "solve
/// `A d = (b − A x)/‖b − A x‖`, then `x += ‖b − A x‖ d`" (the right-hand side
/// normalised, which is what gets past the absolute threshold) bring the
/// true residual to the requested tolerance; a round that does not reduce it
/// stops. No physics changes: the same linear system, solved to the
/// tolerance it was asked for. The threshold itself is gh:#1170.
fn refine(eqn: &FvMatrix, t: &mut VolScalarField, perf: &mut SolverPerformance, settings: SolverSettings) {
    let b = eqn.source.as_slice();
    let bnorm = b.iter().map(|v| v * v).sum::<f64>().sqrt();
    if bnorm == 0.0 {
        return;
    }
    let true_rel = |x: &[f64]| eqn.ldu.residual(x, b).iter().map(|v| v * v).sum::<f64>().sqrt() / bnorm;
    let mut rel = true_rel(t.internal.as_slice());
    for _ in 0..4 {
        if rel <= settings.tolerance {
            break;
        }
        // Normalise the correction's right-hand side: the BiCGStab breakdown
        // test is absolute (|ρ| < 1e-30 with ρ ~ ‖r‖²), so an unscaled r of
        // ~1e-15 reads as a breakdown before the first step.
        let r = eqn.ldu.residual(t.internal.as_slice(), b);
        let rnorm = r.iter().map(|v| v * v).sum::<f64>().sqrt();
        if rnorm == 0.0 {
            break;
        }
        let r_unit: Vec<f64> = r.iter().map(|v| v / rnorm).collect();
        let inner = SolverSettings { tolerance: 1e-6, max_iter: settings.max_iter };
        let (d, p) = krylov_solve(&eqn.ldu, &r_unit, None, KrylovMethod::BiCGStab, KrylovOptions::default(), &inner);
        let trial: Vec<f64> = t.internal.as_slice().iter().zip(&d).map(|(x, dx)| x + rnorm * dx).collect();
        let new_rel = true_rel(&trial);
        perf.n_iterations += p.n_iterations;
        if new_rel >= rel {
            break;
        }
        rel = new_rel;
        for (x, v) in t.internal.as_mut_slice().iter_mut().zip(trial) {
            *x = v;
        }
    }
    perf.final_residual = rel;
    perf.converged = rel <= settings.tolerance;
}

/// The face temperature an upwind operator uses on an inflow face.
fn inflow_value(bc: &BoundaryCondition<f64>, fi: usize, tp: f64) -> f64 {
    match bc {
        BoundaryCondition::FixedValue(v) => *v,
        BoundaryCondition::FixedField(ff) => ff[fi],
        BoundaryCondition::InletOutlet { inlet_value } => *inlet_value,
        _ => tp,
    }
}

/// The conduction into the owner cell of boundary face `fi` of patch `pi`
/// \[W\], `k |Sf| / δ` times the arm `fvm::laplacian` assembles (fixed value,
/// fixed gradient, mixed); zero for any other condition.
pub(crate) fn face_conduction_in(t: &VolScalarField, pi: usize, fi: usize, kappa: f64, mode: fvm::DeltaCoeff) -> f64 {
    let mesh = &t.mesh;
    let patch = &mesh.patches[pi];
    if matches!(patch.kind, PatchKind::Cyclic | PatchKind::CyclicAmi | PatchKind::Empty) {
        return 0.0;
    }
    let gf = patch.start + fi;
    let d = mode.boundary_delta(mesh, gf);
    if d < 1e-300 {
        return 0.0;
    }
    let g = kappa * mesh.face_areas[gf] / d;
    let tp = t.internal[mesh.owner[gf]];
    match &t.boundary[pi].bc {
        BoundaryCondition::FixedValue(v) => g * (v - tp),
        BoundaryCondition::FixedField(ff) => g * (ff[fi] - tp),
        BoundaryCondition::FixedGradient(gr) => g * d * gr,
        BoundaryCondition::Mixed { value_fraction, ref_value, ref_grad } => {
            value_fraction * g * (ref_value - tp) + (1.0 - value_fraction) * g * d * ref_grad
        }
        BoundaryCondition::MixedField { value_fraction, ref_value, ref_grad } => {
            let w = value_fraction[fi];
            w * g * (ref_value[fi] - tp) + (1.0 - w) * g * d * ref_grad[fi]
        }
        _ => 0.0,
    }
}

/// Re-evaluate the boundary face values from each patch's condition
/// (OpenFOAM `T.correctBoundaryConditions()`), through `fvc::interpolate`'s
/// boundary arms, so the stored face values match what the next gradient and
/// the diagnostics read.
pub(crate) fn evaluate_boundary_values(t: &mut VolScalarField) {
    let tf = fvc::interpolate(t);
    for (pf, sf) in t.boundary.iter_mut().zip(tf.boundary.iter()) {
        if !matches!(pf.bc, BoundaryCondition::Empty) {
            pf.values = sf.values.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::f64::Length;
    use uom::si::length::meter;

    /// A 1-D line of `n` cells of length `len` along x, unit cross-section,
    /// patches `left`, `right` and `sides` (adiabatic).
    fn line(n: usize, len: f64) -> Arc<FvMesh> {
        Arc::new(crate::solvers::cht_multi_region_foam::line_mesh(n, Length::new::<meter>(len), Length::new::<meter>(0.0), ["left", "right"]))
    }

    #[test]
    fn thermo_diffusivity_and_units() {
        let w = IncompressibleThermo::si(1000.0, 4000.0, 0.6);
        assert!((w.rho_cp() - 4.0e6).abs() < 1e-6);
        assert!((w.diffusivity() - 1.5e-7).abs() < 1e-20);
        assert!((w.kappa() - 0.6).abs() < 1e-15);
    }

    /// Steady 1-D conduction between two fixed temperatures is linear; the
    /// FV answer is exact for it and the boundary conduction balances.
    #[test]
    fn steady_slab_is_linear_and_balances() {
        let mesh = line(10, 0.1);
        let mut e = EnergyEquation::uniform(mesh.clone(), IncompressibleThermo::si(1.0, 1.0, 2.0), 300.0);
        e.ddt = DdtScheme::SteadyState;
        e.set_patch("left", BoundaryCondition::FixedValue(400.0));
        e.set_patch("right", BoundaryCondition::FixedValue(300.0));
        e.set_patch("nope", BoundaryCondition::FixedValue(1.0));
        let old = e.t.clone();
        let r = e.solve(&old, None, Time::new::<second>(1.0), LaplacianCorrection::Corrected, 0).unwrap();
        assert!(r.max_change > 1.0);
        for c in 0..mesh.n_cells {
            let x = mesh.cell_centres[c].x;
            let want = 400.0 - 1000.0 * x;
            assert!((e.t.internal[c] - want).abs() < 1e-8, "{c} {} {want}", e.t.internal[c]);
        }
        let l = mesh.patches.iter().position(|p| p.name == "left").unwrap();
        let rgt = mesh.patches.iter().position(|p| p.name == "right").unwrap();
        let ql = e.conduction_in(l, LaplacianCorrection::Corrected).get::<watt>();
        let qr = e.conduction_in(rgt, LaplacianCorrection::Corrected).get::<watt>();
        // q = k ΔT / L = 2 * 100 / 0.1 = 2000 W through 1 m².
        assert!((ql - 2000.0).abs() < 1e-6 && (qr + 2000.0).abs() < 1e-6, "{ql} {qr}");
        // Face values were re-evaluated: the fixed ends carry their values.
        assert!((e.t.boundary[l].values[0] - 400.0).abs() < 1e-12);
        assert!(e.stored_energy() > 0.0);
    }

    /// Transient Euler heating conserves energy to the solve tolerance: the
    /// stored-energy change equals the boundary heat times dt.
    #[test]
    fn euler_step_conserves_energy() {
        let mesh = line(8, 0.08);
        let mut e = EnergyEquation::uniform(mesh.clone(), IncompressibleThermo::si(8000.0, 500.0, 15.0), 300.0);
        e.set_patch("left", BoundaryCondition::FixedGradient(1.0e4));
        let l = mesh.patches.iter().position(|p| p.name == "left").unwrap();
        let e0 = e.stored_energy();
        let dt = 10.0;
        let mut q_dt = 0.0;
        for _ in 0..5 {
            let old = e.t.clone();
            e.solve(&old, None, Time::new::<second>(dt), LaplacianCorrection::Corrected, 1).unwrap();
            q_dt += e.conduction_in(l, LaplacianCorrection::Corrected).get::<watt>() * dt;
        }
        // Fixed gradient 1e4 K/m into the slab: q = k g = 1.5e5 W/m².
        assert!((q_dt - 1.5e5 * 50.0).abs() < 1e-6 * q_dt);
        let gain = e.stored_energy() - e0;
        assert!((gain / q_dt - 1.0).abs() < 1e-9, "gain {gain} heat {q_dt}");
        let bad = DdtScheme::Backward;
        e.ddt = bad;
        let old = e.t.clone();
        assert!(e.solve(&old, None, Time::new::<second>(dt), LaplacianCorrection::Corrected, 0).is_err());
    }

    #[test]
    fn upwind_inflow_values_and_mixing_cup() {
        let mesh = line(4, 0.04);
        let mut e = EnergyEquation::uniform(mesh.clone(), IncompressibleThermo::si(1.0, 1.0, 1.0), 300.0);
        e.set_patch("left", BoundaryCondition::FixedValue(350.0));
        let l = mesh.patches.iter().position(|p| p.name == "left").unwrap();
        let r = mesh.patches.iter().position(|p| p.name == "right").unwrap();
        let mut phi = SurfaceScalarField::zeros("phi", mesh.clone());
        phi.boundary[l].values[0] = -2.0;
        phi.boundary[r].values[0] = 2.0;
        assert_eq!(e.mixing_cup(l, &phi), Some(350.0));
        assert_eq!(e.mixing_cup(r, &phi), Some(300.0));
        assert!((e.enthalpy_out(l, &phi).get::<watt>() + 700.0).abs() < 1e-12);
        assert!((e.enthalpy_out(r, &phi).get::<watt>() - 600.0).abs() < 1e-12);
        let s = mesh.patches.iter().position(|p| p.name == "sides").unwrap();
        assert_eq!(e.mixing_cup(s, &phi), None);
        assert_eq!(inflow_value(&BoundaryCondition::FixedField(Field::new(vec![1.0, 2.0])), 1, 0.0), 2.0);
        assert_eq!(inflow_value(&BoundaryCondition::InletOutlet { inlet_value: 5.0 }, 0, 0.0), 5.0);
        assert_eq!(inflow_value(&BoundaryCondition::ZeroGradient, 0, 7.0), 7.0);
    }
}
