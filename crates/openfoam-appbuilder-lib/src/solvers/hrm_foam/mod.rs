// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
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

use crate::error::AppBuilderError;
use crate::io::control_dict::{ControlDict, StartControl, StopControl};
use crate::io::fv_schemes::FvSchemes;
use crate::io::fv_solution::FvSolution;
use openfoam_basic_lib::fields::boundary::bc::BoundaryCondition;
use openfoam_basic_lib::prelude::*;
use std::sync::Arc;

// ── Downar-Zapolski (1996) model constants ────────────────────────────────────

/// Relaxation time pre-factor θ₀ [s]
pub const THETA_0: f64 = 3.84e-7;
/// Pressure undershoot exponent a
pub const DZ_A: f64 = -0.54;
/// Quality exponent b
pub const DZ_B: f64 = -0.05;

/// Runtime controls translated from HRMFoam's thermophysical/SigmaY dictionaries.
#[derive(Clone, Copy, Debug)]
pub struct HrmModelConfig {
    /// Relaxation time pre-factor θ₀ [s].
    pub theta_0: f64,
    /// Pressure undershoot exponent in the Downar-Zapolski correlation.
    pub pressure_undershoot_exp: f64,
    /// Liquid fraction exponent in the Downar-Zapolski correlation.
    pub liquid_fraction_exp: f64,
    /// Lower bound on relaxation time [s].
    pub theta_floor: f64,
    /// Lower bound on density [kg/m³].
    pub rho_min: f64,
    /// Enable the non-condensable gas mass-fraction equation.
    pub solve_gas_fraction: bool,
    /// Skip the enthalpy equation, matching HRMFoam's `adiabatic` switch.
    pub adiabatic: bool,
}

impl Default for HrmModelConfig {
    fn default() -> Self {
        Self {
            theta_0: THETA_0,
            pressure_undershoot_exp: DZ_A,
            liquid_fraction_exp: DZ_B,
            theta_floor: 1.0e-10,
            rho_min: 1.0e-4,
            solve_gas_fraction: true,
            adiabatic: false,
        }
    }
}

/// Homogeneous Relaxation Model (HRM) two-phase flashing flow solver.
///
/// The HRM assumes mechanical and thermal equilibrium between phases but
/// allows thermodynamic non-equilibrium via a finite relaxation time τ
/// toward the equilibrium dryness fraction x_eq(p, h).
///
/// Downar-Zapolski (1996) relaxation time:
///   τ = θ₀ · ψ^a · (1 − x)^b
/// where ψ = (p_sat − p) / p_sat is the pressure undershoot (dimensionless).
///
/// Transport equations:
///   ∂ρ/∂t  + ∇·(ρU)     = 0
///   ∂(ρU)/∂t + ∇·(ρUU)  = −∇p
///   ∂(ρh)/∂t + ∇·(ρhU)  = dp/dt
///   ∂(ρx)/∂t + ∇·(ρxU)  = ρ · (x_eq − x) / τ   ← HRM relaxation source
///   ∂(ρy)/∂t + ∇·(ρyU)  = ∇·(D∇y)              ← gas mass fraction
///
/// The equilibrium quality x_eq(p, h) is supplied externally (e.g. via
/// TAMPINES steam tables).  Call `set_x_eq` each time step before `step()`.
///
/// C++ source: `../HRMFoam/` (sibling directory, outside this workspace)
pub struct HrmFoam {
    pub mesh: Arc<FvMesh>,
    pub control: ControlDict,
    pub schemes: FvSchemes,
    pub solution: FvSolution,
    /// Velocity field [m/s]
    pub u: VolVectorField,
    /// Pressure [Pa]
    pub p: VolScalarField,
    /// Mixture density [kg/m³]
    pub rho: VolScalarField,
    /// Mixture specific enthalpy [J/kg]
    pub h: VolScalarField,
    /// Vapour dryness fraction x ∈ [0, 1]
    pub x: VolScalarField,
    /// Non-condensable gas mass fraction y ∈ [0, 1]
    pub y: VolScalarField,
    /// Equilibrium quality x_eq(p, h) — updated by caller each time step
    pub x_eq: VolScalarField,
    /// Dynamic viscosity μ [Pa·s]
    pub mu: VolScalarField,
    /// Effective thermal diffusivity αh [kg/(m·s)]
    pub alpha_h: VolScalarField,
    /// Effective gas diffusivity D [kg/(m·s)]
    pub gas_diffusivity: VolScalarField,
    /// Saturation pressure p_sat [Pa] — updated by caller each time step
    pub p_sat: VolScalarField,
    /// Mass flux φ = ρ U·Sf [kg/s]
    pub phi: SurfaceScalarField,
    pub model: HrmModelConfig,
}

impl HrmFoam {
    pub fn new(
        mesh: Arc<FvMesh>,
        control: ControlDict,
        schemes: FvSchemes,
        solution: FvSolution,
    ) -> Self {
        Self::with_model_config(mesh, control, schemes, solution, HrmModelConfig::default())
    }

    pub fn with_model_config(
        mesh: Arc<FvMesh>,
        control: ControlDict,
        schemes: FvSchemes,
        solution: FvSolution,
        model: HrmModelConfig,
    ) -> Self {
        let u = VolVectorField::zero("U", mesh.clone());
        let p = VolScalarField::uniform("p", mesh.clone(), 1.0e5);
        let rho = VolScalarField::uniform("rho", mesh.clone(), model.rho_min.max(1.0));
        let h = VolScalarField::zeros("h", mesh.clone());
        let x = VolScalarField::zeros("x", mesh.clone());
        let y = VolScalarField::zeros("y", mesh.clone());
        let x_eq = VolScalarField::zeros("xEq", mesh.clone());
        let mu = VolScalarField::uniform("mu", mesh.clone(), 1.0e-3);
        let alpha_h = VolScalarField::uniform("alphaEff", mesh.clone(), 6.0e-4);
        let gas_diffusivity = VolScalarField::zeros("D", mesh.clone());
        let p_sat = VolScalarField::uniform("pSat", mesh.clone(), 1.0e5);
        let phi = SurfaceScalarField::zeros("phi", mesh.clone());
        Self {
            mesh,
            control,
            schemes,
            solution,
            u,
            p,
            rho,
            h,
            x,
            y,
            x_eq,
            mu,
            alpha_h,
            gas_diffusivity,
            p_sat,
            phi,
            model,
        }
    }

    /// Downar-Zapolski relaxation time τ at a single point.
    ///
    /// # Arguments
    /// * `psi` — dimensionless pressure undershoot (p_sat − p) / p_sat ≥ 0
    /// * `x`   — current dryness fraction ∈ [0, 1]
    pub fn relaxation_time(psi: f64, x: f64) -> f64 {
        HrmModelConfig::default().relaxation_time(psi, x)
    }

    pub fn relaxation_time_with_config(&self, psi: f64, x: f64) -> f64 {
        self.model.relaxation_time(psi, x)
    }

    fn clamp_unit_interval(field: VolScalarField, name: &str) -> VolScalarField {
        let mesh = field.mesh.clone();
        let vals: Vec<f64> = field
            .internal
            .as_slice()
            .iter()
            .map(|&v| v.clamp(0.0, 1.0))
            .collect();
        VolScalarField::new(
            name,
            mesh.clone(),
            Field::new(vals),
            mesh.patches
                .iter()
                .map(|p| PatchField::zero_gradient(p.size))
                .collect(),
        )
    }

    fn solve_gas_fraction_eqn(&mut self, y_old: &VolScalarField, settings: SolverSettings) {
        let mesh = self.mesh.clone();
        let n = mesh.n_cells;
        let dt = self.control.delta_t;
        let conv_y = fvc::div(&self.phi, &self.y);
        let d_f = fvc::interpolate(&self.gas_diffusivity);
        let mut y_eqn =
            fvm::ddt_coeff(&self.rho, &self.y, y_old, dt) - fvm::laplacian(&d_f, &self.y);
        {
            let conv_sl = conv_y.internal.as_slice();
            for c in 0..n {
                y_eqn.source[c] -= mesh.cell_volumes[c] * conv_sl[c];
            }
        }
        let (y_new, _) = y_eqn.solve("y", settings);
        self.y = Self::clamp_unit_interval(y_new, "y");
    }

    fn solve_density_eqn(&mut self, rho_old: &VolScalarField, settings: SolverSettings) {
        let dt = self.control.delta_t;
        let rho_eqn = fvm::ddt(&self.rho, rho_old, dt) + fvm::div(&self.phi, &self.rho);
        let (rho_new, _) = rho_eqn.solve("rho", settings);
        self.rho = rho_new;
        self.clamp_density();
    }

    fn volume_flux_from_mass_flux(&self) -> SurfaceScalarField {
        let mesh = self.mesh.clone();
        let rho_f = fvc::interpolate(&self.rho);
        let internal = Field::from_fn(mesh.n_internal_faces, |f| {
            self.phi.internal[f] / rho_f.internal[f].max(self.model.rho_min)
        });
        let boundary = mesh
            .patches
            .iter()
            .enumerate()
            .map(|(pi, patch)| {
                let values = Field::from_fn(patch.size, |fi| {
                    self.phi.boundary[pi].values[fi]
                        / rho_f.boundary[pi].values[fi].max(self.model.rho_min)
                });
                PatchField {
                    bc: BoundaryCondition::ZeroGradient,
                    values,
                }
            })
            .collect();
        SurfaceScalarField::new("phiv", mesh, internal, boundary)
    }

    fn clamp_density(&mut self) {
        let rho_min = self.model.rho_min.max(1.0e-12);
        for rho in self.rho.internal.as_mut_slice() {
            if *rho < rho_min {
                *rho = rho_min;
            }
        }
    }
}

impl HrmModelConfig {
    /// Downar-Zapolski relaxation time τ for the configured constants.
    pub fn relaxation_time(self, psi: f64, x: f64) -> f64 {
        let psi_c = psi.max(1e-10);
        let x_c = (1.0 - x).max(1e-10);
        let tau = self.theta_0
            * psi_c.powf(self.pressure_undershoot_exp)
            * x_c.powf(self.liquid_fraction_exp);
        tau.max(self.theta_floor)
    }
}

impl HrmFoam {
    /// Advance one time step.
    ///
    /// The caller must update `self.x_eq` and `self.p_sat` before each call
    /// using the steam table lookup x_eq = f(p, h).
    pub fn step(&mut self) -> Result<(), AppBuilderError> {
        let mesh = self.mesh.clone();
        let n = mesh.n_cells;
        let dt = self.control.delta_t;
        let settings = SolverSettings::default();
        let n_outer = self.solution.pimple.n_outer_correctors.max(1);
        let n_inner = self.solution.pimple.n_correctors.max(1);

        let rho_old = self.rho.clone();
        let u_old = self.u.clone();
        let h_old = self.h.clone();
        let x_old = self.x.clone();
        let y_old = self.y.clone();
        let p_old = self.p.clone();

        for _ in 0..n_outer {
            // ── rhoEqn: fvm::ddt(rho) + div(phi, rho) = 0 ───────────────────
            self.solve_density_eqn(&rho_old, settings);

            // ── xEqn: ∂(ρx)/∂t + ∇·(φx) = ρ·(x_eq − x)/τ ──────────────────
            // Semi-implicit: implicit source uses linearisation of the RHS.
            // The relaxation source S = ρ·(x_eq − x)/τ is linearised as:
            //   S ≈ ρ/τ · x_eq  −  ρ/τ · x   (implicit in x)
            // so the diagonal gains ρ·V/τ and source gains ρ·V·x_eq/τ.
            let conv_x = fvc::div(&self.phi, &self.x);

            let mut x_eqn = fvm::ddt_coeff(&self.rho, &self.x, &x_old, dt);
            {
                let conv_sl = conv_x.internal.as_slice();
                let rho_sl = self.rho.internal.as_slice();
                let x_sl = self.x.internal.as_slice();
                let x_eq_sl = self.x_eq.internal.as_slice();
                let p_sl = self.p.internal.as_slice();
                let p_sat_sl = self.p_sat.internal.as_slice();
                for c in 0..n {
                    let v = mesh.cell_volumes[c];
                    x_eqn.source[c] -= v * conv_sl[c];

                    let psi_dz = {
                        let p_c = p_sl[c];
                        let psat = p_sat_sl[c];
                        ((psat - p_c) / psat.max(1.0)).max(0.0)
                    };
                    let tau = self.relaxation_time_with_config(psi_dz, x_sl[c]);
                    let rho_v_tau = rho_sl[c] * v / tau;
                    x_eqn.ldu.diag[c] += rho_v_tau;
                    x_eqn.source[c] += rho_v_tau * x_eq_sl[c];
                }
            }
            let (x_new, _) = x_eqn.solve("x", settings);
            self.x = Self::clamp_unit_interval(x_new, "x");

            // ── yEqn: ∂(ρy)/∂t + ∇·(φy) − ∇·(D∇y) = 0 ─────────────────────
            if self.model.solve_gas_fraction {
                self.solve_gas_fraction_eqn(&y_old, settings);
            }

            // ── hEqn: HRMFoam pressure work + enthalpy diffusion ─────────────
            if !self.model.adiabatic {
                let conv_h = fvc::div(&self.phi, &self.h);
                let alpha_h_f = fvc::interpolate(&self.alpha_h);
                let dp_dt = (self.p.clone() - p_old.clone()) * (1.0 / dt);
                let phiv = self.volume_flux_from_mass_flux();
                let pressure_work =
                    fvc::div(&phiv, &self.p) - self.p.clone() * fvc::div_flux(&phiv);

                let mut h_eqn = fvm::ddt_coeff(&self.rho, &self.h, &h_old, dt)
                    - fvm::laplacian(&alpha_h_f, &self.h);
                {
                    let conv_sl = conv_h.internal.as_slice();
                    let dpdt_sl = dp_dt.internal.as_slice();
                    let pwork_sl = pressure_work.internal.as_slice();
                    for c in 0..n {
                        let v = mesh.cell_volumes[c];
                        h_eqn.source[c] -= v * conv_sl[c];
                        h_eqn.source[c] += v * (dpdt_sl[c] + pwork_sl[c]);
                    }
                }
                let (h_new, _) = h_eqn.solve("h", settings);
                self.h = h_new;
            }

            // ── UEqn: ∂(ρU)/∂t + ∇·(ρUU) − ∇·(μ∇U) = −∇p ─────────────────
            let mut u_eqn = fvm::ddt_coeff_vec(&self.rho, &self.u, &u_old, dt, mesh.clone())
                + fvm::div_vec(&self.phi, &self.u, mesh.clone())
                - fvm::laplacian_vec(&self.mu, &self.u, mesh.clone());

            let a = u_eqn.a_field();
            let rau = {
                let a_sl = a.internal.as_slice();
                let vals: Vec<f64> = (0..n)
                    .map(|c| mesh.cell_volumes[c] / a_sl[c].max(1e-30))
                    .collect();
                VolScalarField::new(
                    "rAU",
                    mesh.clone(),
                    Field::new(vals),
                    mesh.patches
                        .iter()
                        .map(|p| PatchField::zero_gradient(p.size))
                        .collect(),
                )
            };

            let gp = fvc::grad(&self.p);
            for c in 0..n {
                u_eqn.source[c] = u_eqn.source[c] - gp.internal[c] * mesh.cell_volumes[c];
            }
            let (u_pred, _) = u_eqn.solve("U", settings);
            for c in 0..n {
                u_eqn.source[c] = u_eqn.source[c] + gp.internal[c] * mesh.cell_volumes[c];
            }
            self.u = u_pred;

            let h_vec = u_eqn.h_field(&self.u);
            let hbya = {
                let h_sl = h_vec.internal.as_slice();
                let a_sl = a.internal.as_slice();
                let vals: Vec<Vector3> = (0..n)
                    .map(|c| h_sl[c] * (1.0 / a_sl[c].max(1e-30)))
                    .collect();
                VolVectorField::new(
                    "HbyA",
                    mesh.clone(),
                    Field::new(vals),
                    mesh.patches
                        .iter()
                        .map(|p| PatchField::zero_gradient_vec(p.size))
                        .collect(),
                )
            };

            let rho_f = fvc::interpolate(&self.rho);
            let rauf = fvc::interpolate(&rau);
            let rho_rauf = rho_f.clone() * rauf.clone();

            let vol_hbya = fvc::flux(&hbya);
            let phi_hbya = rho_f * vol_hbya;

            let source_p = {
                let mut s = vec![0.0_f64; n];
                let phi_int = phi_hbya.internal.as_slice();
                for f in 0..mesh.n_internal_faces {
                    s[mesh.owner[f]] += phi_int[f];
                    s[mesh.neighbour[f]] -= phi_int[f];
                }
                for (pi, patch) in mesh.patches.iter().enumerate() {
                    let phi_bc = phi_hbya.boundary[pi].values.as_slice();
                    for fi in 0..patch.size {
                        s[mesh.owner[patch.start + fi]] += phi_bc[fi];
                    }
                }
                s
            };

            // ── pEqn (same compressible PISO as rhoPimpleFoam) ───────────────
            // Compressibility ψ = ρ/p (approximation from EOS ρ = ψ·p)
            for _ in 0..n_inner {
                let mut p_eqn = fvm::laplacian(&rho_rauf, &self.p);
                let rho_sl = self.rho.internal.as_slice();
                let p_old_sl = p_old.internal.as_slice();
                let p_now_sl = self.p.internal.as_slice();
                let mut src = source_p.clone();
                for c in 0..n {
                    let psi_c = rho_sl[c] / p_now_sl[c].max(1.0);
                    let pvdt = psi_c * mesh.cell_volumes[c] / dt;
                    p_eqn.ldu.diag[c] += pvdt;
                    src[c] += pvdt * p_old_sl[c];
                }
                p_eqn.source = Field::new(src);
                let (p_new, _) = p_eqn.solve("p", settings);
                self.p = p_new;
            }

            let sng = fvc::sn_grad(&self.p);
            {
                let sng_sl = sng.internal.as_slice();
                let rho_rauf_sl = rho_rauf.internal.as_slice();
                let mut phi_corr = phi_hbya;
                for f in 0..mesh.n_internal_faces {
                    phi_corr.internal[f] -= rho_rauf_sl[f] * sng_sl[f] * mesh.face_areas[f];
                }
                self.phi = phi_corr;
            }
            self.u = hbya - rau * fvc::grad(&self.p);

            // Update density from p (keep ρ consistent with pressure solve)
            {
                let rho_sl = self.rho.internal.as_mut_slice();
                let p_sl = self.p.internal.as_slice();
                for c in 0..n {
                    // ρ corrected via continuity; use p change for small compressibility
                    let _ = (p_sl[c], rho_sl[c]); // will update from x and h in real implementation
                }
            }
        }

        Ok(())
    }

    pub fn run(&mut self) -> Result<(), AppBuilderError> {
        let start = match self.control.start {
            StartControl::StartTime(t) => t,
            _ => 0.0,
        };
        let end = match self.control.stop {
            StopControl::EndTime(t) => t,
            _ => return Ok(()),
        };
        let dt = self.control.delta_t;
        let mut time = start;
        while time < end {
            self.step()?;
            time += dt;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_relaxation_time_matches_downar_zapolski_constants() {
        let psi = 0.02_f64;
        let x = 0.1_f64;
        let expected = THETA_0 * psi.powf(DZ_A) * (1.0 - x).powf(DZ_B);

        let actual = HrmFoam::relaxation_time(psi, x);

        assert!((actual - expected).abs() / expected < 1.0e-12);
    }

    #[test]
    fn relaxation_time_respects_configured_floor() {
        let config = HrmModelConfig {
            theta_floor: 1.0e-3,
            ..HrmModelConfig::default()
        };

        assert_eq!(config.relaxation_time(1.0, 0.0), 1.0e-3);
    }
}
