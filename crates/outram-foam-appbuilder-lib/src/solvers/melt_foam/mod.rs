// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com / openfoam.org)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
// Upstream project: OpenFOAM-dev (OpenFOAM Foundation), vendored read-only at
//   crates/outram-foam-turbulence-lib/upstream_source/OpenFOAM
//   (README.org dated 14th July 2026; etc/bashrc WM_PROJECT_VERSION=dev).
// Upstream sources consulted for this solver (all verified present in the
// vendored tree at the paths given):
//   applications/modules/incompressibleFluid/            (modern pimpleFoam:
//       incompressibleFluid.C, momentumPredictor.C, correctPressure.C)
//   applications/modules/fluid/thermophysicalPredictor.C (energy equation and
//       the `== fvModels().source(rho, he)` sign convention)
//   src/fvModels/general/solidificationMelting/          (the phase-change model)
// Upstream licence: GPL-3.0-or-later.
//
// PROVENANCE NOTE: the PISO/pressure-correction block below is NOT a fresh
// transcription of the C++ above. It is adapted from this workspace's own
// `solvers::pimple_foam`, which was ported earlier from the maintainer's
// separate OpenFOAM reference tree and is already verified against icoFoam
// reference fields. The temperature equation and the fvModels wiring are new
// here. Upstream's `applications/modules/` layout differs from the
// `applications/solvers/` layout `pimple_foam`'s header cites; both are real,
// they are simply different OpenFOAM releases.
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

//! # meltFoam — incompressible buoyant PIMPLE with phase change
//!
//! ## What belongs here, and what does not
//!
//! This module holds the **Layer-5 solver loop** that melting needs and that
//! [`pimple_foam`](super::pimple_foam) does not provide: a temperature
//! transport equation, and the wiring that lets an
//! [`FvModels`] collection contribute to *both* the momentum and the energy
//! equation of the same timestep.
//!
//! The phase-change physics itself does **not** belong here — it lives in
//! `outram_foam_basic_lib::fv_options::SolidificationMelting`, which owns the
//! liquid fraction, the latent heat, the Darcy drag and the Boussinesq buoyancy.
//! This module only assembles equations and calls that model at the right points
//! in the timestep. Adding a mushy-zone correlation or a new drag law here would
//! be a layering mistake.
//!
//! There is no upstream application called `meltFoam`, and this module does not
//! claim to be a port of one. Upstream runs this physics by attaching the
//! `solidificationMelting` **fvModel** to an existing buoyant solver through a
//! runtime dictionary; because this crate's solvers are Rust structs rather than
//! runtime-assembled dictionaries, the same composition has to be written out as
//! a named solver. The individual equations are upstream's — the PISO loop from
//! `applications/modules/incompressibleFluid/`, the energy equation from
//! `applications/modules/fluid/thermophysicalPredictor.C` — but their assembly
//! into one struct is this crate's, not a transcription of any single upstream
//! file.
//!
//! ## Governing equations
//!
//! With kinematic pressure `p = p/ρ` \[m²/s²\], exactly as pimpleFoam:
//!
//! `∂U/∂t + ∇·(UU) − ∇·(ν∇U) = −∇p + S_U`
//!
//! `∇·U = 0`
//!
//! `∂T/∂t + ∇·(φT) − ∇·(α_th ∇T) = S_T`
//!
//! `S_U` and `S_T` are supplied entirely by the attached [`FvModels`]. For a
//! melting problem `S_U` is the Carman-Kozeny Darcy drag plus the Boussinesq
//! buoyancy, and `S_T` is the latent heat of fusion.
//!
//! ## The kinematic-units trap (upstream dimensional quirk — reproduced, not fixed)
//!
//! Upstream's `solidificationMelting::addSup` has two momentum overloads, and
//! **the incompressible one simply calls the compressible one**:
//!
//! ```text
//! void addSup(const volVectorField& U, fvMatrix<vector>& eqn) const
//! {
//!     ...
//!     const scalar S  = -Cu_*sqr(1.0 - alpha1c)/(pow3(alpha1c) + q_);
//!     const vector Sb = rhoRef_*g*beta_*deltaT_[i];
//!     Sp[celli] += Vc*S;
//!     Su[celli] += Vc*Sb;
//! }
//! void addSup(const volScalarField& rho, const volVectorField& U,
//!             fvMatrix<vector>& eqn) const
//! {
//!     addSup(U, eqn);          // <-- identical coefficients, density ignored
//! }
//! ```
//!
//! Those coefficients are dimensionally consistent only with a **force-form**
//! (density-weighted) momentum equation: `Vc*Sb` carries \[N\] and `Vc*S`
//! carries \[kg/s\]. A kinematic momentum equation — the one pimpleFoam and this
//! solver assemble — needs \[m⁴/s²\] and \[m³/s\] respectively, i.e. both terms
//! divided by density.
//!
//! Upstream does not divide. It offers no separate kinematic form and no
//! dimension check on this path, so a user attaching the model to an
//! incompressible solver silently gets both terms scaled by ρ unless they
//! compensate through the coefficients.
//!
//! **This port reproduces the upstream behaviour rather than correcting it**,
//! per the workspace rule on upstream defects. The compensation is therefore the
//! caller's, and it is mechanical:
//!
//! - set `reference_density = 1.0` (not the material density), and
//! - give `darcy_coefficient` in **kinematic** units \[1/s\], i.e. the
//!   literature `C_u` \[kg/(m³·s)\] divided by ρ.
//!
//! [`MeltFoam::boussinesq_coefficients`] performs exactly that conversion from
//! physical inputs, so a caller never has to remember it. Reach for it rather
//! than filling
//! `SolidificationMeltingCoefficients` in by hand for this solver.
//!
//! ## Why `rho` is a field of ones
//!
//! The temperature equation above is per unit volume with no ρ, matching
//! upstream's `addSup(he, eqn)` overload, which passes `geometricOneField()`.
//! This solver therefore hands [`FvModels::add_source_scalar`] a uniform field
//! of 1.0 — not the material density. Passing the real density would multiply
//! the latent-heat source by ρ (a factor of ~6000 for gallium) and freeze the
//! melt front in place.

use crate::error::AppBuilderError;
use crate::io::control_dict::{ControlDict, StartControl, StopControl};
use crate::io::fv_schemes::FvSchemes;
use crate::io::fv_solution::FvSolution;
use crate::solvers::bc_util::{capture_bcs, correct_bcs, correct_bcs_vec};
use crate::solvers::pimple_foam::PressureSolver;
use outram_foam_basic_lib::prelude::*;
use std::sync::Arc;

/// Incompressible transient buoyant PIMPLE solver with phase change.
///
/// Solves the equations in the module documentation: a kinematic-pressure
/// PISO/PIMPLE velocity-pressure coupling, plus a temperature transport
/// equation, with an [`FvModels`] collection contributing to both.
///
/// # Units
///
/// Strict SI. `u` \[m/s\], `p` **kinematic** \[m²/s²\] (not Pa), `t` \[K\],
/// `phi` \[m³/s\], `nu` \[m²/s\], `alpha_thermal` \[m²/s\].
///
/// # Typical use
///
/// Build with [`new`](Self::new), set the fields and their boundary conditions,
/// attach a `SolidificationMelting` model built from
/// [`boussinesq_coefficients`](Self::boussinesq_coefficients), then call
/// [`step`](Self::step) in a loop or [`run`](Self::run) once.
pub struct MeltFoam {
    /// The mesh, shared read-only.
    pub mesh: Arc<FvMesh>,
    /// Time control — start, stop and timestep.
    pub control: ControlDict,
    /// Discretisation schemes.
    pub schemes: FvSchemes,
    /// Linear-solver and PIMPLE-loop settings.
    pub solution: FvSolution,
    /// Velocity \[m/s\].
    pub u: VolVectorField,
    /// Kinematic pressure `p/ρ` \[m²/s²\].
    pub p: VolScalarField,
    /// Temperature \[K\].
    pub t: VolScalarField,
    /// Face volumetric flux `φ = U·Sf` \[m³/s\].
    pub phi: SurfaceScalarField,
    /// Kinematic viscosity `ν` \[m²/s\]. For gallium ~3.2e-7.
    pub nu: VolScalarField,
    /// Thermal diffusivity `α_th = k/(ρ·Cp)` \[m²/s\]. For liquid gallium
    /// ~1.3e-5, i.e. roughly 40x the momentum diffusivity — the low Prandtl
    /// number that makes this problem convection-dominated.
    pub alpha_thermal: VolScalarField,
    /// Optional equation sources. A melting case attaches exactly one
    /// `SolidificationMelting` model here.
    pub fv_models: FvModels,
    /// Linear solver for the pressure Poisson equation.
    pub pressure_solver: PressureSolver,
    /// Linear-solver settings for the **temperature** equation.
    ///
    /// # Why this is separate, and why the default is so tight
    ///
    /// Defaults to `tolerance = 1e-12`, far tighter than
    /// [`SolverSettings::default`]'s `1e-7`. That is not caution for its own
    /// sake — it is a measured requirement.
    ///
    /// The enthalpy-porosity scheme conserves energy *exactly* at the discrete
    /// level: summing the temperature equation over all cells and all steps
    /// telescopes to `Σ V·(Cp·ΔT + L·Δα) = Cp·Σ dt·(wall flux)`, with every
    /// internal-face term cancelling. The only leak is the linear solve's own
    /// residual, and a melting run is *long* — thousands to tens of thousands of
    /// steps — so a per-step residual that is negligible in a short run
    /// accumulates into a visible energy drift.
    ///
    /// Measured on the 1-D Stefan case in this crate's `melting_vv_cases`
    /// integration test (400 cells, dt = 0.01 s, 10 000 steps, 2026-08-05):
    ///
    /// | T-solve tolerance | Energy imbalance |
    /// |---|---|
    /// | `1e-7` (the generic default) | **-0.9221 %** of the wall heat input |
    /// | `1e-14` | **-1.96e-6 J/m²**, i.e. -0.0000 % |
    ///
    /// A 0.9 % energy loss would be indistinguishable from a physics error while
    /// being purely numerical, which is exactly the kind of drift that makes a
    /// melting result untrustworthy. Loosen this only with a re-run of that
    /// energy-balance check.
    pub temperature_solver: SolverSettings,
}

impl MeltFoam {
    /// Build a solver with zeroed fields on `mesh`.
    ///
    /// Every field is created uniform and must be given its initial values and
    /// boundary conditions by the caller before stepping. `nu` defaults to
    /// 1e-5 m²/s and `alpha_thermal` to 1e-5 m²/s (Pr = 1); both are placeholders
    /// a real case overwrites. `t` starts at 300 K rather than 0 K, because a
    /// zero-kelvin initial temperature would put every cell below any physical
    /// solidus and is never what a caller wants.
    #[must_use]
    pub fn new(
        mesh: Arc<FvMesh>,
        control: ControlDict,
        schemes: FvSchemes,
        solution: FvSolution,
    ) -> Self {
        let u = VolVectorField::zero("U", mesh.clone());
        let p = VolScalarField::zeros("p", mesh.clone());
        let t = VolScalarField::uniform("T", mesh.clone(), 300.0);
        let phi = SurfaceScalarField::zeros("phi", mesh.clone());
        let nu = VolScalarField::uniform("nu", mesh.clone(), 1e-5);
        let alpha_thermal = VolScalarField::uniform("alphat", mesh.clone(), 1e-5);
        Self {
            mesh,
            control,
            schemes,
            solution,
            u,
            p,
            t,
            phi,
            nu,
            alpha_thermal,
            fv_models: FvModels::new(),
            pressure_solver: PressureSolver::default(),
            temperature_solver: SolverSettings {
                tolerance: 1e-12,
                max_iter: 2_000,
            },
        }
    }

    /// Build phase-change coefficients already converted to the **kinematic**
    /// convention this solver requires.
    ///
    /// # Why this exists
    ///
    /// See the module documentation: upstream's momentum `addSup` applies
    /// force-form coefficients to a kinematic equation without dividing by
    /// density. Rather than silently correcting upstream inside the model, this
    /// constructor does the division at the point where physical inputs are
    /// supplied, so the model itself stays a faithful transcription.
    ///
    /// # Parameters and units
    ///
    /// - `solidus`, `liquidus` — \[K\]. `liquidus` must exceed `solidus`; a pure
    ///   metal needs an artificial mushy interval of order 0.1–1 K, since a true
    ///   step function makes the liquid fraction non-differentiable.
    /// - `latent_heat` — latent heat of fusion `L` \[J/kg\].
    /// - `specific_heat` — `Cp` \[J/(kg·K)\].
    /// - `density` — the material density ρ \[kg/m³\], used **only** to convert
    ///   the Darcy coefficient. It is deliberately *not* stored as
    ///   `reference_density`, which is set to 1.0.
    /// - `thermal_expansion` — `β` \[1/K\].
    /// - `darcy_coefficient_force` — the literature mushy-zone constant `C_u`
    ///   \[kg/(m³·s)\], as tabulated for the force-form equation. Values of
    ///   1e5–1e8 are usual.
    ///
    /// # Returns
    ///
    /// Coefficients with `reference_density = 1.0` and
    /// `darcy_coefficient = darcy_coefficient_force / density` \[1/s\]. All
    /// other fields carry upstream's defaults (`relaxation = 0.9`,
    /// `darcy_regularisation = 1e-3`, `eutectic_fraction = 0`).
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn boussinesq_coefficients(
        solidus: f64,
        liquidus: f64,
        latent_heat: f64,
        specific_heat: f64,
        density: f64,
        thermal_expansion: f64,
        darcy_coefficient_force: f64,
    ) -> SolidificationMeltingCoefficients {
        let mut c = SolidificationMeltingCoefficients::new(
            solidus,
            liquidus,
            latent_heat,
            specific_heat,
            // Reference density 1.0: the kinematic momentum equation already
            // carries the division by rho, so re-applying it here would scale
            // the buoyancy by rho a second time.
            1.0,
            thermal_expansion,
        );
        c.darcy_coefficient = darcy_coefficient_force / density;
        c
    }

    /// Advance the solution by one timestep.
    ///
    /// Order of operations within the step, and why:
    ///
    /// 1. The momentum equation is assembled and the attached models add the
    ///    Darcy drag and buoyancy. The first such call also triggers the
    ///    phase-change model's once-per-step liquid-fraction update, so the drag
    ///    and the latent heat in step 2 are consistent with each other.
    /// 2. The PISO correctors enforce continuity.
    /// 3. The temperature equation is solved **after** the velocity, using the
    ///    corrected (divergence-free) flux — advecting temperature with a flux
    ///    that does not satisfy continuity is a standard way to lose energy
    ///    conservation.
    /// 4. [`FvModels::advance_time`] rolls the liquid fraction forward and
    ///    re-arms the once-per-step guard.
    ///
    /// # Errors
    ///
    /// Returns [`AppBuilderError`] if a linear solve fails.
    pub fn step(&mut self) -> Result<(), AppBuilderError> {
        let mesh = self.mesh.clone();
        let n = mesh.n_cells;
        let dt = self.control.delta_t;
        let settings = SolverSettings::default();
        let p_settings = SolverSettings {
            tolerance: 1e-8,
            max_iter: 2_000,
        };
        let n_outer = self.solution.pimple.n_outer_correctors.max(1);
        let n_inner = self.solution.pimple.n_correctors.max(1);

        let u_old = self.u.clone();
        let t_old = self.t.clone();
        let phi_old = self.phi.clone();

        let u_bcs = capture_bcs(&self.u.boundary);
        let p_bcs = capture_bcs(&self.p.boundary);
        let t_bcs = capture_bcs(&self.t.boundary);

        // The registered names of the solved fields, captured before any
        // arithmetic can overwrite them. `FvModels` dispatches on these, so they
        // must stay stable for the whole step.
        let velocity_name = self.u.name.clone();
        let temperature_name = self.t.name.clone();

        // `geometricOneField` stand-in. Both the momentum and the temperature
        // equation here are per unit volume with no density, so every model that
        // asks for a density gets 1.0. See the module docs.
        let ones = VolScalarField::uniform("one", mesh.clone(), 1.0);

        for _ in 0..n_outer {
            // ── Momentum predictor ───────────────────────────────────────────
            let mut u_eqn = fvm::ddt_vec(&self.u, &u_old, dt, mesh.clone())
                + fvm::div_vec(&self.phi, &self.u, mesh.clone())
                + fvm::laplacian_vec(&self.nu, &self.u, mesh.clone());

            // Darcy drag (implicit, onto the diagonal) and Boussinesq buoyancy
            // (explicit, into the source). This is also what triggers the
            // liquid-fraction update for the whole timestep.
            self.fv_models.add_source_vector(
                velocity_name.as_str(),
                &ones,
                &self.t,
                &u_old,
                None,
                dt,
                &mut u_eqn,
            );

            let a = u_eqn.a_field();
            let rau = {
                let a_data = a.internal.as_slice();
                let rau_vals: Vec<f64> = (0..n)
                    .map(|c| mesh.cell_volumes[c] / a_data[c].max(1e-30))
                    .collect();
                VolScalarField::new(
                    "rAU",
                    mesh.clone(),
                    Field::new(rau_vals),
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
            let (mut u_pred, _) = u_eqn.solve("U", settings);
            correct_bcs_vec(&mut u_pred, &u_bcs);
            for c in 0..n {
                u_eqn.source[c] = u_eqn.source[c] + gp.internal[c] * mesh.cell_volumes[c];
            }
            self.u = u_pred;

            let rauf = fvc::interpolate(&rau);
            let ddt_corr = fvc::ddt_corr(&u_old, &phi_old, dt);

            // ── PISO pressure-correction loop ────────────────────────────────
            for _ in 0..n_inner {
                let h = u_eqn.h_field(&self.u);
                let hbya = {
                    let h_data = h.internal.as_slice();
                    let a_data = a.internal.as_slice();
                    let vals: Vec<Vector3> = (0..n)
                        .map(|c| h_data[c] * (1.0 / a_data[c].max(1e-30)))
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

                let mut phi_hbya = fvc::flux(&hbya);
                {
                    let rauf_int = rauf.internal.as_slice();
                    let dc_int = ddt_corr.internal.as_slice();
                    for f in 0..mesh.n_internal_faces {
                        phi_hbya.internal[f] += rauf_int[f] * dc_int[f];
                    }
                }

                let source_p = {
                    let mut s = vec![0.0_f64; n];
                    let phi_int = phi_hbya.internal.as_slice();
                    for f in 0..mesh.n_internal_faces {
                        s[mesh.owner[f]] -= phi_int[f];
                        s[mesh.neighbour[f]] += phi_int[f];
                    }
                    // Prescribed wall flux, not the zero-gradient HbyA
                    // extrapolation — OpenFOAM's `constrainHbyA`.
                    for (pi, patch) in mesh.patches.iter().enumerate() {
                        if matches!(self.u.boundary[pi].bc, BoundaryCondition::Empty) {
                            continue;
                        }
                        for fi in 0..patch.size {
                            let gf = patch.start + fi;
                            let u_bc = self.u.boundary[pi].values[fi];
                            s[mesh.owner[gf]] -= u_bc.dot(mesh.face_area_vectors[gf]);
                        }
                    }
                    s
                };

                let mut p_eqn = fvm::laplacian(&rauf, &self.p);
                for (s, &sp) in p_eqn.source.iter_mut().zip(source_p.iter()) {
                    *s += sp;
                }
                p_eqn.set_reference(0, 0.0);
                let (mut p_new, _) = match self.pressure_solver {
                    PressureSolver::Pcg => p_eqn.solve_cg_with_guess("p", &self.p, p_settings),
                    PressureSolver::Gamg => p_eqn.solve_gamg_with_guess("p", &self.p, p_settings),
                };
                correct_bcs(&mut p_new, &p_bcs);
                self.p = p_new;

                let sng = fvc::sn_grad(&self.p);
                {
                    let sng_int = sng.internal.as_slice();
                    let rauf_int = rauf.internal.as_slice();
                    for f in 0..mesh.n_internal_faces {
                        phi_hbya.internal[f] -= rauf_int[f] * sng_int[f] * mesh.face_areas[f];
                    }
                    self.phi = phi_hbya;
                }

                self.u = hbya - rau.clone() * fvc::grad(&self.p);
                // Restore the field's registered name. Field arithmetic keeps
                // the LEFT operand's name (deliberately — see the crate
                // `CLAUDE.md` note on unbounded name growth), so the line above
                // would leave the velocity field called "HbyA". That is fatal
                // here and silent: `FvModels` selects models by field name, so a
                // renamed velocity makes `contributes_to("U")` false and the
                // Darcy drag and buoyancy are dropped from every step after the
                // first, with no error — the melt simply stops convecting. See
                // `melt_foam::tests::velocity_field_keeps_its_name_after_correction`.
                self.u.name = velocity_name.clone();
                correct_bcs_vec(&mut self.u, &u_bcs);
            }

            // ── Temperature equation ─────────────────────────────────────────
            //
            // Solved with the corrected flux, per buoyantBoussinesqPimpleFoam's
            // `TEqn.H`. `alpha_thermal` is interpolated to faces because
            // `fvm::laplacian` takes a surface diffusivity.
            let alpha_f = fvc::interpolate(&self.alpha_thermal);
            let mut t_eqn = fvm::ddt(&self.t, &t_old, dt)
                + fvm::div(&self.phi, &self.t)
                + fvm::laplacian(&alpha_f, &self.t);

            // Latent heat. `ones` for the density, per the module docs.
            self.fv_models.add_source_scalar(
                temperature_name.as_str(),
                &ones,
                &self.t,
                dt,
                &mut t_eqn,
            );

            let (mut t_new, _) = t_eqn.solve("T", self.temperature_solver);
            t_new.name = temperature_name.clone();
            correct_bcs(&mut t_new, &t_bcs);
            self.t = t_new;
        }

        // Roll the liquid fraction forward and re-arm the once-per-step guard.
        // Skipping this freezes the phase change entirely — see
        // `FvModels::advance_time`.
        self.fv_models.advance_time();

        Ok(())
    }

    /// Run from the start time to the end time in `control`.
    ///
    /// # Errors
    ///
    /// Returns [`AppBuilderError`] if any timestep's linear solve fails.
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

    /// The liquid fraction \[-\] of the first attached
    /// `SolidificationMelting` model, one value per cell it acts on, in
    /// selection order.
    ///
    /// Returns `None` if no such model is attached. This is the field a melting
    /// case measures its melt front from.
    #[must_use]
    pub fn liquid_fraction(&self) -> Option<&[f64]> {
        self.fv_models.models().iter().find_map(|m| match m {
            FvModel::SolidificationMelting(s) => Some(s.liquid_fraction()),
            _ => None,
        })
    }
}

#[cfg(test)]
mod tests;
