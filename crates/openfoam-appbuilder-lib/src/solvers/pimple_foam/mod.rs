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

use std::sync::Arc;
use openfoam_basic_lib::prelude::*;
use crate::error::AppBuilderError;
use crate::io::control_dict::{ControlDict, StartControl, StopControl};
use crate::io::fv_schemes::FvSchemes;
use crate::io::fv_solution::FvSolution;

/// Incompressible transient PIMPLE/PISO solver.
///
/// Solves:
///   ∂U/∂t + ∇·(UU) − ν∇²U = −∇p    (p here is kinematic: p/ρ, units m²/s²)
///   ∇·U = 0
///
/// Outer PIMPLE loop → momentum predictor → inner PISO pressure correctors.
///
/// C++ solver: `applications/solvers/incompressible/pimpleFoam/`
pub struct PimpleFoam {
    pub mesh:     Arc<FvMesh>,
    pub control:  ControlDict,
    pub schemes:  FvSchemes,
    pub solution: FvSolution,
    /// Velocity field [m/s]
    pub u:   VolVectorField,
    /// Kinematic pressure field p/ρ [m²/s²]
    pub p:   VolScalarField,
    /// Face volumetric flux φ = U·Sf [m³/s]
    pub phi: SurfaceScalarField,
    /// Kinematic viscosity ν [m²/s]
    pub nu:  VolScalarField,
}

impl PimpleFoam {
    pub fn new(
        mesh: Arc<FvMesh>,
        control: ControlDict,
        schemes: FvSchemes,
        solution: FvSolution,
    ) -> Self {
        let u   = VolVectorField::zero("U",   mesh.clone());
        let p   = VolScalarField::zeros("p",  mesh.clone());
        let phi = SurfaceScalarField::zeros("phi", mesh.clone());
        let nu  = VolScalarField::uniform("nu", mesh.clone(), 1e-5);
        Self { mesh, control, schemes, solution, u, p, phi, nu }
    }

    /// Advance the solution by one time step using the PIMPLE algorithm.
    ///
    /// Dimensional analysis (integrated FV system):
    ///   A has units m³/s  →  rAU = V/A has units s
    ///   H has units m⁴/s²  →  HbyA = H/A has units m/s
    ///   phi_hbya = flux(HbyA) has units m³/s
    ///   Pressure source = Σ_f phi_hbya_f (NOT divided by V) ∈ m³/s
    ///   Laplacian coeff = rAUf · area/delta ∈ s·m  →  coeff·p (m²/s²) = m³/s ✓
    pub fn step(&mut self) -> Result<(), AppBuilderError> {
        let mesh = self.mesh.clone();
        let n    = mesh.n_cells;
        let dt   = self.control.delta_t;
        let settings = SolverSettings::default();
        let n_outer = self.solution.pimple.n_outer_correctors.max(1);
        let n_inner = self.solution.pimple.n_correctors.max(1);

        let u_old = self.u.clone();

        for _ in 0..n_outer {
            // ── Assemble implicit momentum equation (no pressure source yet) ───
            let mut u_eqn = fvm::ddt_vec(&self.u, &u_old, dt, mesh.clone())
                + fvm::div_vec(&self.phi, &self.u, mesh.clone())
                - fvm::laplacian_vec(&self.nu, &self.u, mesh.clone());

            // A = diagonal [m³/s];  rAU = V/A [s]
            let a = u_eqn.a_field();
            let rau = {
                let a_data = a.internal.as_slice();
                let rau_vals: Vec<f64> = (0..n)
                    .map(|c| mesh.cell_volumes[c] / a_data[c].max(1e-30))
                    .collect();
                VolScalarField::new(
                    "rAU", mesh.clone(),
                    Field::new(rau_vals),
                    mesh.patches.iter().map(|p| PatchField::zero_gradient(p.size)).collect(),
                )
            };

            // Momentum predictor: temporarily add −V·∇p to the source
            let gp = fvc::grad(&self.p);
            for c in 0..n {
                u_eqn.source[c] = u_eqn.source[c] - gp.internal[c] * mesh.cell_volumes[c];
            }
            let (u_pred, _) = u_eqn.solve("U", settings);
            // Restore source (remove pressure contribution) for H(U) computation
            for c in 0..n {
                u_eqn.source[c] = u_eqn.source[c] + gp.internal[c] * mesh.cell_volumes[c];
            }
            self.u = u_pred;

            // H(U_pred) from the clean (no-pressure) source [m⁴/s²]
            let h = u_eqn.h_field(&self.u);

            // HbyA = H/A [m/s]
            let hbya = {
                let h_data  = h.internal.as_slice();
                let a_data  = a.internal.as_slice();
                let vals: Vec<Vector3> = (0..n)
                    .map(|c| h_data[c] * (1.0 / a_data[c].max(1e-30)))
                    .collect();
                VolVectorField::new(
                    "HbyA", mesh.clone(),
                    Field::new(vals),
                    mesh.patches.iter().map(|p| PatchField::zero_gradient_vec(p.size)).collect(),
                )
            };

            // rAUf = interpolate(rAU) at faces [s]
            let rauf = fvc::interpolate(&rau);

            // phi_hbya = flux(HbyA) [m³/s]
            let phi_hbya = fvc::flux(&hbya);

            // Raw face-flux divergence: Σ_f phi_hbya_f (NOT ÷ V) [m³/s]
            let source_p = {
                let mut s = vec![0.0_f64; n];
                let phi_int = phi_hbya.internal.as_slice();
                for f in 0..mesh.n_internal_faces {
                    s[mesh.owner[f]]     += phi_int[f];
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

            // ── PISO inner pressure correctors ───────────────────────────────
            for _ in 0..n_inner {
                // Pressure equation: ∇·(rAUf ∇p) = ∇·HbyA
                let mut p_eqn = fvm::laplacian(&rauf, &self.p);
                p_eqn.source = Field::new(source_p.clone());
                // Fix singular system (no fixed-pressure BC in closed domain)
                p_eqn.set_reference(0, 0.0);
                let (p_new, _) = p_eqn.solve("p", settings);
                self.p = p_new;
            }

            // ── Final flux and velocity correction ───────────────────────────
            let sng = fvc::sn_grad(&self.p);
            {
                let sng_int  = sng.internal.as_slice();
                let rauf_int = rauf.internal.as_slice();
                let mut phi_corr = phi_hbya;
                for f in 0..mesh.n_internal_faces {
                    phi_corr.internal[f] -= rauf_int[f] * sng_int[f] * mesh.face_areas[f];
                }
                self.phi = phi_corr;
            }
            // U = HbyA − rAU · ∇p
            self.u = hbya - rau * fvc::grad(&self.p);
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
