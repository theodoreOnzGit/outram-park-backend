// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Physics formulation derived from Moltres (MSR multiphysics on MOOSE)
//   Upstream: https://github.com/arfc/moltres (UIUC ARFC group)
//   Upstream commit: 3dd2ce7
//   Upstream sources consulted (formulation only, no code reused):
//     src/kernels/FissionHeatSource.C        (q''' from kappaSigma_f phi,
//       normalised to a target reactor power)
//     src/kernels/ConvectiveHeatExchanger.C  (volumetric sink htc (T - T_ref))
//     src/kernels/SigmaR.C / GroupDiffusion.C (d*_d_temp feedback hooks)
//   Upstream license: LGPL-2.1, incorporated into this GPL-3.0 crate under
//   the LGPL-2.1 section 3 GPL-conversion option.
//
// Finite-volume assembly built on outram-foam-basic-lib (fvm::div upwind
// advection, fvm::laplacian conduction, fvm::sp/su sources).
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

//! Reduced fuel-salt thermal model and the power/temperature-feedback
//! coupling loop.
//!
//! **Deliberately reduced first pass — not CFD.** The salt moves as a rigid
//! slug at the prescribed loop speed (same [`FaceFluxField`] the precursors
//! use); the steady temperature `T` (`K`) on the closed loop obeys
//!
//! ```text
//!   rho c_p div(u T) - div(k_T grad T) + h_v m_HX (T - T_HX) = q'''
//! ```
//!
//! where `q'''` (`W/m^3`) is the fission heat deposited in the core
//! (Moltres `FissionHeatSource`, normalised to a target total power),
//! `h_v` (`W/(m^3 K)`) is a volumetric heat-exchanger conductance active
//! only on the HX section mask `m_HX` (Moltres `ConvectiveHeatExchanger`),
//! and `T_HX` (`K`) is the secondary-side temperature. Momentum, buoyancy,
//! turbulence, and conjugate structures are all out of scope here — the
//! full-CFD path lives in `outram-foam-appbuilder-lib` / GeN-Foam.
//!
//! [`CoupledMsrSolver`] closes the multiphysics loop: eigenvalue → power
//! shape scaled to target power → temperature → linear cross-section
//! feedback → eigenvalue …, Picard-iterated with under-relaxation.

use std::sync::Arc;

use outram_foam_basic_lib::prelude::{
    fvm, Field, FvMesh, PatchField, SolverPerformance, SolverSettings, SurfaceScalarField,
    VolScalarField,
};

use crate::circulating::CirculatingFuelSolver;
use crate::diffusion::EigenReport;
use crate::error::MoltresError;
use crate::materials::{scalar_field, FaceFluxField, TemperatureField};

/// Configuration of the reduced salt thermal model. All properties uniform
/// over the loop (well-mixed salt, single phase).
#[derive(Debug, Clone)]
pub struct SaltThermalConfig {
    /// Volumetric heat capacity `rho c_p` in `J/(m^3 K)` (molten fluoride
    /// salts: ~4e6).
    pub rho_cp: f64,
    /// Thermal conductivity `k_T` in `W/(m K)` (salts: ~1; nearly
    /// irrelevant next to advection at loop Peclet numbers, kept for
    /// completeness and for the `u = 0` limit).
    pub conductivity: f64,
    /// Volumetric heat-exchanger conductance `h_v` in `W/(m^3 K)`,
    /// applied only on HX cells.
    pub hx_conductance: f64,
    /// Secondary-side (coolant) temperature `T_HX` in `K`.
    pub hx_temperature: f64,
    /// Per-cell HX mask: `true` where the heat exchanger removes heat.
    /// Length must equal `mesh.n_cells`.
    pub hx_mask: Vec<bool>,
    /// Linear-solver settings for the (asymmetric) temperature solve.
    pub linear: SolverSettings,
}

/// Steady fuel-salt temperature model on a loop mesh (see module docs).
#[derive(Debug, Clone)]
pub struct SaltThermalModel {
    mesh: Arc<FvMesh>,
    config: SaltThermalConfig,
}

impl SaltThermalModel {
    /// Build the model; validates the HX mask length and property signs.
    ///
    /// # Errors
    /// [`MoltresError::SizeMismatch`] for a wrong-length mask;
    /// [`MoltresError::InvalidMaterial`] for non-positive `rho_cp`,
    /// negative conductivity/conductance, or an all-false mask (a heated
    /// closed loop with no sink has no steady state).
    pub fn new(mesh: Arc<FvMesh>, config: SaltThermalConfig) -> Result<Self, MoltresError> {
        if config.hx_mask.len() != mesh.n_cells {
            return Err(MoltresError::SizeMismatch {
                what: "hx_mask",
                expected: mesh.n_cells,
                got: config.hx_mask.len(),
            });
        }
        if !(config.rho_cp > 0.0) || config.conductivity < 0.0 || config.hx_conductance < 0.0 {
            return Err(MoltresError::InvalidMaterial(
                "salt thermal model needs rho_cp > 0, conductivity >= 0, hx_conductance >= 0"
                    .into(),
            ));
        }
        if !config.hx_mask.iter().any(|m| *m) || config.hx_conductance == 0.0 {
            return Err(MoltresError::InvalidMaterial(
                "a closed heated loop needs an active heat exchanger (non-empty mask, \
                 hx_conductance > 0) to have a steady temperature"
                    .into(),
            ));
        }
        Ok(Self { mesh, config })
    }

    /// Solve the steady temperature for a given loop flow and heat source.
    ///
    /// - `flow` — face volumetric flux `u . A_f` (`m^3/s`).
    /// - `heat_source` — `q'''` in `W/m^3` (zero outside the core).
    ///
    /// Returns the temperature field (`K`) and the linear-solver
    /// performance record.
    ///
    /// # Errors
    /// [`MoltresError::SizeMismatch`] for off-mesh inputs;
    /// [`MoltresError::LinearSolveFailed`] if Gauss-Seidel misses its
    /// tolerance.
    pub fn solve_steady(
        &self,
        flow: &FaceFluxField,
        heat_source: &VolScalarField,
    ) -> Result<(TemperatureField, SolverPerformance), MoltresError> {
        if flow.internal.len() != self.mesh.n_internal_faces {
            return Err(MoltresError::SizeMismatch {
                what: "flow flux (one value per internal face)",
                expected: self.mesh.n_internal_faces,
                got: flow.internal.len(),
            });
        }
        if heat_source.internal.len() != self.mesh.n_cells {
            return Err(MoltresError::SizeMismatch {
                what: "heat source field",
                expected: self.mesh.n_cells,
                got: heat_source.internal.len(),
            });
        }
        let cfg = &self.config;
        let t = VolScalarField::zeros("T", self.mesh.clone());

        // rho c_p scaled face flux for the advection matrix.
        let phi_rho_cp = SurfaceScalarField::new(
            "phiRhoCp",
            self.mesh.clone(),
            Field::from_fn(self.mesh.n_internal_faces, |f| {
                flow.internal[f] * cfg.rho_cp
            }),
            flow.boundary
                .iter()
                .map(|p| PatchField {
                    bc: p.bc.clone(),
                    values: p.values.map(|v| v * cfg.rho_cp),
                })
                .collect(),
        );
        // Conductivity on faces (uniform).
        let k_face = SurfaceScalarField::new(
            "kT",
            self.mesh.clone(),
            Field::uniform(self.mesh.n_internal_faces, cfg.conductivity),
            self.mesh
                .patches
                .iter()
                .map(|p| PatchField::zero_gradient(p.size))
                .collect(),
        );
        // Volumetric HX conductance field and its sink offset.
        let h_vals: Vec<f64> = cfg
            .hx_mask
            .iter()
            .map(|m| if *m { cfg.hx_conductance } else { 0.0 })
            .collect();
        let h_field = scalar_field(&self.mesh, "hHX", h_vals.clone());
        let rhs_vals: Vec<f64> = heat_source
            .internal
            .as_slice()
            .iter()
            .zip(h_vals.iter())
            .map(|(q, h)| q + h * cfg.hx_temperature)
            .collect();
        let rhs = scalar_field(&self.mesh, "qPlusHX", rhs_vals);

        let eqn = fvm::div(&phi_rho_cp, &t) + fvm::laplacian(&k_face, &t) + fvm::sp(&h_field, &t)
            - fvm::su(&rhs, &t);
        let (sol, perf) = eqn.solve("T", cfg.linear);
        if !perf.converged {
            return Err(MoltresError::LinearSolveFailed {
                field: "T".into(),
                residual: perf.final_residual,
                iterations: perf.n_iterations,
            });
        }
        Ok((sol, perf))
    }
}

/// Result of a converged neutronics–thermal Picard iteration.
#[derive(Debug, Clone)]
pub struct CoupledReport {
    /// The final eigenvalue report (at the converged temperature field).
    pub eigen: EigenReport,
    /// Converged salt temperature (`K`).
    pub temperature: TemperatureField,
    /// Heat source scaled to the target power (`W/m^3`).
    pub heat_source: VolScalarField,
    /// Picard (outer multiphysics) iterations performed.
    pub picard_iterations: usize,
    /// Final max-norm temperature change between Picard iterates (`K`).
    pub temperature_residual: f64,
}

/// Picard-coupled circulating-fuel neutronics + salt temperature +
/// cross-section feedback (see module docs). The neutronics and thermal
/// models must share one mesh and one flow field.
#[derive(Debug)]
pub struct CoupledMsrSolver {
    /// Circulating-fuel neutronics (owned; query `neutronics.k_eff`,
    /// `.flux`, `.precursors` after solving).
    pub neutronics: CirculatingFuelSolver,
    /// Reduced salt thermal model.
    pub thermal: SaltThermalModel,
    /// Loop flow shared by both physics (`m^3/s` per face).
    pub flow: FaceFluxField,
    /// Target total fission power in `W` (the flux is rescaled so
    /// `int q''' dV` equals this).
    pub target_power: f64,
    /// Reference temperature `T_ref` (`K`) of the cross-section data.
    pub t_ref: f64,
    /// Picard under-relaxation factor in `(0, 1]` (0.7 is robust).
    pub relaxation: f64,
    /// Max Picard iterations.
    pub max_picard_iterations: usize,
    /// Convergence tolerance on the max temperature change (`K`).
    pub temperature_tolerance: f64,
}

impl CoupledMsrSolver {
    /// Run the Picard loop: eigenvalue → power → temperature → feedback →
    /// … until the temperature field settles.
    ///
    /// # Errors
    /// Any neutronics/thermal error, plus [`MoltresError::NoFissionSource`]
    /// if the power shape integrates to zero, or
    /// [`MoltresError::NotConverged`] if the Picard loop exhausts its
    /// budget.
    pub fn solve(&mut self) -> Result<CoupledReport, MoltresError> {
        let mesh = self.flow.mesh.clone();
        let mut temperature = VolScalarField::uniform("T", mesh.clone(), self.t_ref);
        let mut last_eigen: Option<EigenReport> = None;
        let mut heat = VolScalarField::zeros("q", mesh.clone());
        let mut t_residual = f64::INFINITY;

        for picard in 1..=self.max_picard_iterations {
            self.neutronics.set_temperature(&temperature, self.t_ref)?;
            let eigen = self.neutronics.solve()?;

            // Scale the power shape to the target total power.
            let shape = self.neutronics.power_density_shape();
            let total: f64 = shape
                .internal
                .as_slice()
                .iter()
                .zip(mesh.cell_volumes.iter())
                .map(|(q, v)| q * v)
                .sum();
            if total <= 0.0 {
                return Err(MoltresError::NoFissionSource);
            }
            let scale = self.target_power / total;
            let q_vals: Vec<f64> = shape
                .internal
                .as_slice()
                .iter()
                .map(|q| q * scale)
                .collect();
            heat = scalar_field(&mesh, "q", q_vals);

            let (t_new, _perf) = self.thermal.solve_steady(&self.flow, &heat)?;

            // Under-relaxed update and convergence check.
            t_residual = 0.0f64;
            {
                let told = temperature.internal.as_mut_slice();
                let tnew = t_new.internal.as_slice();
                for (o, n) in told.iter_mut().zip(tnew.iter()) {
                    let delta = n - *o;
                    t_residual = t_residual.max(delta.abs());
                    *o += self.relaxation * delta;
                }
            }
            last_eigen = Some(eigen);
            if t_residual < self.temperature_tolerance {
                return Ok(CoupledReport {
                    eigen,
                    temperature,
                    heat_source: heat,
                    picard_iterations: picard,
                    temperature_residual: t_residual,
                });
            }
        }
        let _ = (last_eigen, heat);
        Err(MoltresError::NotConverged {
            outer_iterations: self.max_picard_iterations,
            k_residual: f64::NAN,
            flux_residual: t_residual,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diffusion::EigenSettings;
    use crate::materials::{DelayedFamily, MsrMaterial, XsFields};
    use crate::ring_mesh::RingMesh;

    fn ring() -> RingMesh {
        RingMesh::new(15.0, 0.1, 300).unwrap()
    }

    fn thermal_config(ring: &RingMesh) -> SaltThermalConfig {
        // HX section between s = 7 m and s = 10 m.
        let mask: Vec<bool> = (0..ring.n_cells)
            .map(|i| {
                let s = ring.arc_centre(i);
                (7.0..10.0).contains(&s)
            })
            .collect();
        SaltThermalConfig {
            rho_cp: 4.0e6,
            conductivity: 1.0,
            hx_conductance: 5.0e5,
            hx_temperature: 900.0,
            hx_mask: mask,
            linear: SolverSettings {
                tolerance: 1e-10,
                max_iter: 200_000,
            },
        }
    }

    /// Core-localised heat source scaled to `total` watts.
    fn core_heat(ring: &RingMesh, total: f64) -> VolScalarField {
        let core_cells: Vec<usize> = (0..ring.n_cells)
            .filter(|i| ring.arc_centre(*i) < 5.6)
            .collect();
        let core_vol: f64 = core_cells.iter().map(|i| ring.mesh.cell_volumes[*i]).sum();
        let q = total / core_vol;
        let vals: Vec<f64> = (0..ring.n_cells)
            .map(|i| if ring.arc_centre(i) < 5.6 { q } else { 0.0 })
            .collect();
        scalar_field(&ring.mesh, "q", vals)
    }

    /// V&V (verification, conservation) — steady loop energy balance.
    ///
    /// **Methodology.** Closed ring (15 m, 0.1 m^2, 300 cells), slug flow
    /// at `u = 0.6 m/s`, `rho c_p = 4e6 J/(m^3 K)`, `k_T = 1 W/(m K)`,
    /// 1 MW deposited uniformly over the 5.6 m core, HX
    /// (`h_v = 5e5 W/(m^3 K)`, `T_HX = 900 K`) over `s in [7, 10) m`.
    /// Integrating the steady equation over the closed loop, advection and
    /// conduction telescope away, so the HX must remove exactly the
    /// deposited power: `int h_v (T - T_HX) dV = 1 MW`, and every
    /// temperature must sit above `T_HX` (the sink cannot cool the loop
    /// below the secondary side, and heat only enters). Pass criteria:
    /// relative energy imbalance `< 1e-6`; `min T > T_HX`.
    ///
    /// **Result (measured 2026-08-04, release build):** relative imbalance
    /// `1.1e-8`; `min T = 904.833 K`, `max T = 908.999 K`. The loop
    /// temperature span `max - min = 4.166 K` reproduces the analytic
    /// slug-flow heat-up `P/(rho c_p u A) = 1e6/(4e6 * 0.6 * 0.1)
    /// = 4.167 K` to 0.03 % — an independent advective-transport check.
    /// Untrusted AI-assisted draft pending human V&V.
    #[test]
    fn steady_loop_energy_balance() {
        let ring = ring();
        let cfg = thermal_config(&ring);
        let hx_temperature = cfg.hx_temperature;
        let hx_conductance = cfg.hx_conductance;
        let mask = cfg.hx_mask.clone();
        let model = SaltThermalModel::new(ring.mesh.clone(), cfg).unwrap();
        let flow = ring.uniform_flux(0.6);
        let power = 1.0e6;
        let q = core_heat(&ring, power);
        let (t, perf) = model.solve_steady(&flow, &q).unwrap();
        assert!(perf.converged);

        let removed: f64 = (0..ring.n_cells)
            .filter(|i| mask[*i])
            .map(|i| hx_conductance * (t.internal[i] - hx_temperature) * ring.mesh.cell_volumes[i])
            .sum();
        let rel = ((removed - power) / power).abs();
        println!(
            "[V&V energy balance] removed = {removed:.6e} W, rel imbalance = {rel:.3e}, \
             min T = {:.3} K, max T = {:.3} K",
            t.internal.min(),
            t.internal.max()
        );
        assert!(rel < 1e-6, "energy imbalance = {rel:.3e}");
        assert!(
            t.internal.min() > hx_temperature,
            "min T = {} must exceed T_HX = {hx_temperature}",
            t.internal.min()
        );
    }

    /// V&V (verification, feedback sign) — negative temperature feedback
    /// lowers k with power.
    ///
    /// **Methodology.** MSRE-like one-group ring as in
    /// `crate::circulating`, with `d Sigma_r/dT = +2e-4 1/(m K)` in the
    /// core (heating adds absorption: a negative reactivity coefficient of
    /// roughly `-dSigma_r/dT / Sigma_r = -25 pcm/K`), Keepin families,
    /// `u = 0.6 m/s`, thermal model as in `steady_loop_energy_balance`,
    /// Picard relaxation 0.7, `T` tolerance 0.01 K. Solve the coupled
    /// system at target powers 0.5, 4, and 8 MW. Pass criteria: Picard
    /// converges at every power; `k(0.5 MW) > k(4 MW) > k(8 MW)`; the
    /// converged salt temperature rises with power.
    ///
    /// **Result (measured 2026-08-04, release build):**
    /// `P = 0.5 MW: k = 1.00540376, max T = 904.50 K (7 Picard);`
    /// `P = 4 MW:   k = 0.99946179, max T = 935.99 K (8 Picard);`
    /// `P = 8 MW:   k = 0.99268115, max T = 971.99 K (9 Picard)` —
    /// monotone negative power-reactivity feedback, ~170 pcm/MW, i.e.
    /// ~-19 pcm/K of peak salt heat-up, consistent with the configured
    /// `-dSigma_r/dT / Sigma_r = -25 pcm/K` acting on the (cooler-than-
    /// peak) core average. Untrusted AI-assisted draft pending human V&V.
    #[test]
    fn temperature_feedback_reduces_k_with_power() {
        let ring = ring();
        let core = MsrMaterial {
            name: "core-salt".into(),
            diffusion: vec![0.01],
            sigma_removal: vec![0.8],
            nu_sigma_f: vec![0.81],
            chi_prompt: vec![1.0],
            chi_delayed: vec![1.0],
            scattering: vec![vec![0.0]],
            sigma_power: vec![1e-11],
            d_sigma_removal_d_temp: vec![2e-4],
        };
        let external = MsrMaterial::non_fuel("loop-salt", vec![0.01], vec![0.15]);
        let zones = ring.two_zone_map(5.6);
        let xs = XsFields::materialize(ring.mesh.clone(), &zones, &[core, external]).unwrap();
        let families = DelayedFamily::keepin_u235();

        let mut ks = Vec::new();
        let mut t_maxes = Vec::new();
        for power in [0.5e6, 4.0e6, 8.0e6] {
            let neutronics = CirculatingFuelSolver::new(
                xs.clone(),
                families.clone(),
                ring.uniform_flux(0.6),
                1e-4,
                &[],
                EigenSettings::default(),
            )
            .unwrap();
            let thermal = SaltThermalModel::new(ring.mesh.clone(), thermal_config(&ring)).unwrap();
            let mut coupled = CoupledMsrSolver {
                neutronics,
                thermal,
                flow: ring.uniform_flux(0.6),
                target_power: power,
                t_ref: 900.0,
                relaxation: 0.7,
                max_picard_iterations: 60,
                temperature_tolerance: 0.01,
            };
            let report = coupled.solve().unwrap();
            assert!(report.eigen.converged);
            println!(
                "[V&V feedback] P = {:.1} MW -> k = {:.8}, max T = {:.2} K, \
                 picard = {}",
                power / 1e6,
                report.eigen.k_eff,
                report.temperature.internal.max(),
                report.picard_iterations
            );
            ks.push(report.eigen.k_eff);
            t_maxes.push(report.temperature.internal.max());
        }
        assert!(
            ks[0] > ks[1] && ks[1] > ks[2],
            "k must fall with power: {ks:?}"
        );
        assert!(
            t_maxes[0] < t_maxes[1] && t_maxes[1] < t_maxes[2],
            "T must rise with power: {t_maxes:?}"
        );
    }
}
