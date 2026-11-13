// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/solvers/onePhase/**
//             (onePhase.{C,H}, include/equations/{UEqn,EEqn,pEqn}_1p.H)
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `solver::one_phase` — porous single-phase thermal-hydraulics driver
//!
//! Rust port of GeN-Foam's `solvers/onePhase` — the single-fluid, one-stationary-
//! structure porous-medium Eulerian solver. It marches, per time step, a
//! **porous momentum** predictor/corrector and a **porous energy** equation
//! coupled to the solid structure:
//!
//! - **`UEqn`** (`UEqn_1p.H`): `ddt(alpha rho, U) + div(alphaRhoPhi, U)
//!   - laplacian(alpha rho nuEff, U) + Kd . U = -grad(p) + body forces`, where
//!   the anisotropic drag `Kd` is assembled by [`super::porous_drag`] from the
//!   fluid-structure friction closure. Here only the **isotropic** `Kd` is
//!   wired (`Kd = Kd * I`), so the drag term is the implicit `fvm::sp(Kd, U)`
//!   in upstream's `Sp((1/3) tr(Kd), U) + (dev(Kd) & U)` (the deviatoric part
//!   vanishes for isotropic `Kd`).
//! - **`pEqn`** (`pEqn_1p.H`): an incompressible-porous PIMPLE pressure
//!   corrector (`rAU`/`HbyA`/flux reconstruction) built on basic-lib operators,
//!   reusing the proven structure of [`crate::solvers::rho_pimple_foam`].
//! - **`EEqn`** (`EEqn_1p.H`): `ddt(alpha rho, he) + div(alphaRhoPhi, he)
//!   - laplacian(alpha alphaEff, he) = q_struct + q'''`, with the structure
//!   heat exchange linearised semi-implicitly (a `Su`/`Sp` pair) exactly as
//!   `structure::linearizedSemiImplicitHeatSource`.
//!
//! ## Honest scope
//!
//! This driver runs with **constant fluid properties** supplied on the
//! [`Fluid`] fields (`rho`, `mu`, `Cp`, `kappa`) and treats the enthalpy as
//! `he = Cp * T` about `T = 0`. The reason is wiring, not absence: the bespoke
//! hydrogen thermophysical package in [`super::super::thermophysical`] *is*
//! ported (EOS, thermodynamics, viscosity, conductivity), but this driver does
//! not yet call it as its `he <-> T` / `rho(p,T)` package. That
//! is sufficient for the incompressible porous momentum/energy physics that is
//! the ported slice's purpose (friction pressure drop, structure heat coupling)
//! and is what the V&V exercises. The structure side is wired as a
//! fixed-surface-temperature convective coupling (a field-wide analogue of
//! GeN-Foam's `fixedTemperature` structure model); driving a full
//! [`super::super::structure::StructureCell`] fuel-pin conduction field is the
//! next slice (bead op-p6p.7.11 follow-up).
//!
//! **Extension, new work and not port (gh:#1149, 2026-11-03):** the optional
//! [`OnePhaseSolver::conductivity_model`] ([`PorousConductivity`]) replaces
//! the constant `alphaEff` by `lambda(T)/Cp` per region before each energy
//! solve. With it `None` (the default) the ported path is unchanged, bit for
//! bit (`solver::tests::constant_property_energy_path_is_bit_identical`).
//!
//! ~~**Two behaviours of the energy solve found while verifying that extension
//! (2026-11-03), filed as gh:#1152 and deliberately left as they are,
//! because the port path is pinned:** (1) `correct_energy` replaces `he` by
//! the matrix solution, whose boundary conditions are all zero-gradient, so a
//! fixed-value enthalpy boundary is applied on the first call only; (2) the
//! `EEqn` is solved by Gauss-Seidel cold-started from zero and capped at
//! the default 1000 sweeps (`SolverSettings::default`), which does not
//! converge a near-steady diffusion-dominated matrix (a 40-cell slab at
//! `dt = 1e7 s` was left 2 K from its solution).~~ **FIXED 2026-11-03
//! (gh:#1152):** `correct_energy` now mirrors upstream's in-place
//! `EEqn.solve()`: `he` keeps its boundary conditions (re-applied as the
//! momentum path does, `bc_util::correct_bcs`) and the solve is upstream's
//! `smoothSolver` + `symGaussSeidel` warm-started from the current `he`, with
//! tolerance [`OnePhaseSolver::energy_solver`]. The 40-cell slab at
//! `dt = 1e7 s` now lands 0.064 K from the Kirchhoff profile (its
//! discretisation error); the pin was re-recorded with the old bits kept.
//!
//! ## Wiring of the consumed models
//!
//! | Consumed | Where it enters |
//! |---|---|
//! | [`Fluid`] field state (`U`, `phi`, `rho`, `he`, `alpha`, `Dh`, `mu`) | owned by the solver, read/written each step |
//! | [`super::porous_drag::PorousDrag`] ([`super::super::closures::fs_drag`]) | `Kd` assembly → `UEqn` drag term |
//! | structure coupling fields (`wall_htc`, `wall_area`, `wall_temperature`) | `EEqn` linearised heat source |

use std::sync::Arc;

use outram_foam_basic_lib::prelude::*;

use super::porous_drag::PorousDrag;
use crate::genfoam::thermal_hydraulics::solid_conduction::{ConductivityModel, SolidConductionError};
use crate::error::AppBuilderError;
use crate::genfoam::thermal_hydraulics::phase::fluid::Fluid;

/// Porous single-phase thermal-hydraulics solver (`onePhase`).
///
/// Owns one [`Fluid`] phase, the pressure field, the porous-drag closure, and
/// the structure convective-coupling fields, and advances them with
/// [`OnePhaseSolver::step`]. Construct with [`OnePhaseSolver::new`] and set the
/// initial/boundary fields through the public accessors before stepping.
pub struct OnePhaseSolver {
    /// The finite-volume mesh (shared, read-only).
    mesh: Arc<FvMesh>,
    /// The single fluid phase (velocity, flux, thermo placeholders, `alpha`).
    pub fluid: Fluid,
    /// Static pressure `p` [Pa].
    pub p: VolScalarField,
    /// Fluid-structure porous drag closure (assembles `Kd`).
    pub drag: PorousDrag,

    /// Effective kinematic viscosity `nuEff` [m^2/s] for the momentum diffusion
    /// term (molecular + turbulent). Constant-property placeholder.
    pub nu_eff: VolScalarField,
    /// Effective thermal diffusivity `alphaEff = kappa/Cp` [kg/(m s)] for the
    /// energy diffusion term.
    pub alpha_eff: VolScalarField,
    /// Constant specific heat `Cp` [J/(kg K)] relating `he = Cp * T`.
    pub cp: VolScalarField,

    /// Structure surface heat-transfer coefficient `h` [W/(m^2 K)].
    pub wall_htc: VolScalarField,
    /// Structure interfacial area density `a_v` [1/m].
    pub wall_area: VolScalarField,
    /// Structure surface temperature `T_wall` [K] (fixed-temperature coupling).
    pub wall_temperature: VolScalarField,

    /// Explicit body force per unit volume `f_body` [N/m^3] (e.g. an imposed
    /// pressure gradient or a gravity-driven head), added to the momentum RHS.
    pub body_force: Vector3,
    /// Whether to run the pressure corrector. For a spatially uniform, gradient-
    /// free driving (the drag/energy V&V), the pressure field is inert and this
    /// can be `false` to isolate the momentum/energy physics.
    pub solve_pressure: bool,
    /// Reference cell for the (singular, incompressible) pressure system.
    pub p_ref_cell: usize,
    /// Reference pressure value [Pa] pinned at `p_ref_cell`.
    pub p_ref_value: f64,

    /// **New work, not port (gh:#1149).** Optional temperature-dependent
    /// effective conductivity of the porous energy equation. `None` (the
    /// default) runs the ported constant-property path unchanged, bit for bit
    /// (pinned by `solver::tests::constant_property_energy_path_is_bit_identical`).
    /// `Some` overwrites [`Self::alpha_eff`] with `lambda(T)/Cp` before every
    /// energy solve, so the outer correctors and time steps iterate the
    /// nonlinearity (Picard, lagged one solve).
    pub conductivity_model: Option<PorousConductivity>,

    /// Linear-solver controls of the `EEqn` (upstream `fvSolution`
    /// `solvers { "h.*" { solver smoothSolver; smoother symGaussSeidel;
    /// tolerance ...; relTol 0; } }`). `tolerance` is OpenFOAM's normalised
    /// residual; `max_iter` is upstream's `maxIter` (default 1000,
    /// `lduMatrix.C:38`). Default `1e-7`, the value of GeN-Foam 652b3da's
    /// onePhase `3D_gFHR` and `2D_MSFR` tutorials; others use 1e-5 (HTR-10)
    /// to 1e-8.
    pub energy_solver: SolverSettings,

    /// PIMPLE outer correctors.
    pub n_outer: usize,
    /// PISO inner (pressure) correctors.
    pub n_inner: usize,
}

impl OnePhaseSolver {
    /// Allocate a one-phase solver on `mesh` with the given fluid and drag
    /// closure, unit constant properties, no structure coupling, and no body
    /// force. Overwrite the public fields to configure a case.
    #[must_use]
    pub fn new(mesh: Arc<FvMesh>, fluid: Fluid, drag: PorousDrag) -> Self {
        Self {
            p: VolScalarField::zeros("p", mesh.clone()),
            nu_eff: VolScalarField::zeros("nuEff", mesh.clone()),
            alpha_eff: VolScalarField::zeros("alphaEff", mesh.clone()),
            cp: VolScalarField::uniform("Cp", mesh.clone(), 1.0),
            wall_htc: VolScalarField::zeros("wallHtc", mesh.clone()),
            wall_area: VolScalarField::zeros("wallArea", mesh.clone()),
            wall_temperature: VolScalarField::zeros("Twall", mesh.clone()),
            body_force: Vector3::ZERO,
            solve_pressure: true,
            p_ref_cell: 0,
            p_ref_value: 0.0,
            conductivity_model: None,
            energy_solver: SolverSettings {
                tolerance: 1e-7,
                max_iter: 1000,
            },
            n_outer: 1,
            n_inner: 2,
            drag,
            fluid,
            mesh,
        }
    }

    /// The mesh this solver runs on.
    #[must_use]
    pub fn mesh(&self) -> &Arc<FvMesh> {
        &self.mesh
    }

    /// Fluid temperature field `T = he / Cp` [K], recomputed from the current
    /// enthalpy and constant `Cp`.
    #[must_use]
    pub fn temperature(&self) -> VolScalarField {
        let mut t = VolScalarField::zeros("T", self.mesh.clone());
        let he = self.fluid.enthalpy().internal.as_slice();
        let cp = self.cp.internal.as_slice();
        let out = t.internal.as_mut_slice();
        for i in 0..out.len() {
            out[i] = he[i] / cp[i];
        }
        t
    }

    /// Advance the coupled porous momentum + energy system one time step `dt`
    /// (seconds).
    ///
    /// Order (per PIMPLE outer corrector): [`Self::correct_fluid_mechanics`]
    /// then [`Self::correct_energy`], mirroring upstream `onePhase::correctPhysics`
    /// (`UEqn`/`pEqn` then `EEqn`).
    pub fn step(&mut self, dt: f64) -> Result<(), AppBuilderError> {
        let n_outer = self.n_outer.max(1);
        for _ in 0..n_outer {
            self.correct_fluid_mechanics(dt)?;
            self.correct_energy(dt)?;
        }
        Ok(())
    }

    /// Solve the porous momentum equation (and, if enabled, the pressure
    /// corrector), updating the fluid velocity and flux.
    ///
    /// Port of `UEqn_1p.H` + `pEqn_1p.H` (isotropic-`Kd`, incompressible,
    /// constant-property). The assembled momentum matrix is
    ///
    /// ```text
    /// ddt(alpha rho, U) + div(alphaRhoPhi, U) + laplacian(alpha rho nuEff, U)
    ///   + Kd . U  =  f_body  (- grad p, in the corrector)
    /// ```
    pub fn correct_fluid_mechanics(&mut self, dt: f64) -> Result<(), AppBuilderError> {
        let mesh = self.mesh.clone();
        let n = mesh.n_cells;

        // alpha*rho and its face flux alphaRhoPhi = alpha*rho * phi.
        let alpha_rho = self.alpha_rho();
        let alpha_rho_f = fvc::interpolate(&alpha_rho);
        let mut alpha_rho_phi = self.fluid.phi().clone();
        {
            let arf = alpha_rho_f.internal.as_slice();
            let arp = alpha_rho_phi.internal.as_mut_slice();
            for f in 0..arp.len() {
                arp[f] *= arf[f];
            }
        }

        // Momentum diffusivity alpha*rho*nuEff (interpolated to faces).
        let mut arnu = VolScalarField::zeros("alphaRhoNuEff", mesh.clone());
        {
            let a = self.fluid.alpha().internal.as_slice();
            let r = self.fluid.density().internal.as_slice();
            let nu = self.nu_eff.internal.as_slice();
            let o = arnu.internal.as_mut_slice();
            for i in 0..o.len() {
                o[i] = a[i] * r[i] * nu[i];
            }
        }

        // Drag coefficient Kd (isotropic) from the friction closure.
        self.fluid.correct_mag_velocity();
        let kd = self.drag.assemble_kd(&self.fluid);

        let u_old = self.fluid.velocity().clone();
        let u_bcs = crate::solvers::bc_util::capture_bcs(&self.fluid.velocity().boundary);

        // UEqn assembly. `+ laplacian_vec` uses the positive-definite
        // (= -div(gamma grad)) operator convention of basic-lib.
        let mut u_eqn =
            fvm::ddt_coeff_vec(&alpha_rho, self.fluid.velocity(), &u_old, dt, mesh.clone())
                + fvm::div_vec(&alpha_rho_phi, self.fluid.velocity(), mesh.clone())
                + fvm::laplacian_vec(&arnu, self.fluid.velocity(), mesh.clone())
                + fvm::sp_vec(&kd, self.fluid.velocity());

        // Explicit body force f_body (imposed pressure gradient / head).
        for c in 0..n {
            u_eqn.source[c] += self.body_force * mesh.cell_volumes[c];
        }

        let a = u_eqn.a_field();

        if !self.solve_pressure {
            // Drag/energy V&V path: no pressure gradient, solve momentum alone.
            let (mut u_new, _) = u_eqn.solve("U", SolverSettings::default());
            crate::solvers::bc_util::correct_bcs_vec(&mut u_new, &u_bcs);
            *self.fluid.velocity_mut() = u_new;
            self.fluid.correct_mag_velocity();
            return Ok(());
        }

        // ── Incompressible porous PIMPLE pressure corrector ──────────────────
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
        // Momentum predictor with explicit -V*grad(p).
        let gp = fvc::grad(&self.p);
        for c in 0..n {
            u_eqn.source[c] -= gp.internal[c] * mesh.cell_volumes[c];
        }
        let (mut u_pred, _) = u_eqn.solve("U", SolverSettings::default());
        crate::solvers::bc_util::correct_bcs_vec(&mut u_pred, &u_bcs);
        for c in 0..n {
            u_eqn.source[c] += gp.internal[c] * mesh.cell_volumes[c];
        }
        *self.fluid.velocity_mut() = u_pred;

        // alpha_f * rAU_f interpolated for the flux/laplacian.
        let alpha_f = fvc::interpolate(self.fluid.alpha());
        let rauf = fvc::interpolate(&rau);
        let mut alpha_rauf = rauf.clone();
        {
            let af = alpha_f.internal.as_slice();
            let ar = alpha_rauf.internal.as_mut_slice();
            for f in 0..ar.len() {
                ar[f] *= af[f];
            }
        }

        let p_bcs = crate::solvers::bc_util::capture_bcs(&self.p.boundary);
        let p_settings = SolverSettings {
            tolerance: 1e-9,
            max_iter: 2_000,
        };

        for _ in 0..self.n_inner.max(1) {
            let h = u_eqn.h_field(self.fluid.velocity());
            let hbya = {
                let h_sl = h.internal.as_slice();
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
            // phiHbyA = alpha_f * flux(HbyA)  [volumetric flux].
            let mut phi_hbya = fvc::flux(&hbya);
            {
                let af = alpha_f.internal.as_slice();
                let ph = phi_hbya.internal.as_mut_slice();
                for f in 0..ph.len() {
                    ph[f] *= af[f];
                }
            }

            // pEqn: laplacian(alpha_rAU_f, p) = div(phiHbyA).
            let mut p_eqn = fvm::laplacian(&alpha_rauf, &self.p);
            {
                let mut s = vec![0.0_f64; n];
                let phi_int = phi_hbya.internal.as_slice();
                for f in 0..mesh.n_internal_faces {
                    s[mesh.owner[f]] -= phi_int[f];
                    s[mesh.neighbour[f]] += phi_int[f];
                }
                for (s_c, sp) in p_eqn.source.iter_mut().zip(s.iter()) {
                    *s_c += sp;
                }
            }
            p_eqn.set_reference(self.p_ref_cell, self.p_ref_value);
            let (mut p_new, _) = p_eqn.solve_cg("p", p_settings);
            crate::solvers::bc_util::correct_bcs(&mut p_new, &p_bcs);
            self.p = p_new;

            // Correct the flux and reconstruct U.
            let sng = fvc::sn_grad(&self.p);
            {
                let sng_sl = sng.internal.as_slice();
                let ar_sl = alpha_rauf.internal.as_slice();
                for f in 0..mesh.n_internal_faces {
                    phi_hbya.internal[f] -= ar_sl[f] * sng_sl[f] * mesh.face_areas[f];
                }
            }
            *self.fluid.phi_mut() = phi_hbya;

            let mut u_new = hbya - rau.clone() * fvc::grad(&self.p);
            crate::solvers::bc_util::correct_bcs_vec(&mut u_new, &u_bcs);
            *self.fluid.velocity_mut() = u_new;
        }
        self.fluid.correct_mag_velocity();
        Ok(())
    }

    /// Solve the porous energy equation, updating the fluid enthalpy.
    ///
    /// Port of `EEqn_1p.H`, constant-`Cp` form (`he = Cp T`). The structure
    /// exchange is linearised semi-implicitly like
    /// `structure::linearizedSemiImplicitHeatSource`: the wall source per unit
    /// volume `a_v h (T_wall - T)` contributes an explicit `Su(a_v h T_wall)`
    /// and an implicit `Sp(a_v h / Cp)` (on `he`), which is unconditionally
    /// stable and drives `T -> T_wall` at equilibrium with no flow. The
    /// neutronics power density `q'''` deposited directly in the fluid
    /// (`fluid.power_density()`) is added explicitly.
    pub fn correct_energy(&mut self, dt: f64) -> Result<(), AppBuilderError> {
        let mesh = self.mesh.clone();
        let n = mesh.n_cells;

        let alpha_rho = self.alpha_rho();
        let he_old = self.fluid.enthalpy().clone();

        // New work (gh:#1149), skipped entirely when no model is set:
        // alphaEff = lambda(T)/Cp at the current temperature T = he/Cp.
        if let Some(model) = &self.conductivity_model {
            model.update_alpha_eff(
                self.fluid.enthalpy().internal.as_slice(),
                self.cp.internal.as_slice(),
                self.alpha_eff.internal.as_mut_slice(),
            );
        }

        // alpha*alphaEff diffusivity, interpolated to faces.
        let mut aae = VolScalarField::zeros("alphaAlphaEff", mesh.clone());
        {
            let a = self.fluid.alpha().internal.as_slice();
            let ae = self.alpha_eff.internal.as_slice();
            let o = aae.internal.as_mut_slice();
            for i in 0..o.len() {
                o[i] = a[i] * ae[i];
            }
        }
        let aae_f = fvc::interpolate(&aae);

        // alphaRhoPhi for the convection term.
        let alpha_rho_f = fvc::interpolate(&alpha_rho);
        let mut alpha_rho_phi = self.fluid.phi().clone();
        {
            let arf = alpha_rho_f.internal.as_slice();
            let arp = alpha_rho_phi.internal.as_mut_slice();
            for f in 0..arp.len() {
                arp[f] *= arf[f];
            }
        }

        let mut e_eqn = fvm::ddt_coeff(&alpha_rho, self.fluid.enthalpy(), &he_old, dt)
            + fvm::div(&alpha_rho_phi, self.fluid.enthalpy())
            + fvm::laplacian(&aae_f, self.fluid.enthalpy());

        // Structure linearised heat source + fluid power density.
        {
            let av = self.wall_area.internal.as_slice();
            let hh = self.wall_htc.internal.as_slice();
            let tw = self.wall_temperature.internal.as_slice();
            let cp = self.cp.internal.as_slice();
            let alpha = self.fluid.alpha().internal.as_slice();
            let q = self.fluid.power_density().internal.as_slice();
            for c in 0..n {
                let v = mesh.cell_volumes[c];
                let coeff = av[c] * hh[c]; // a_v h  [W/(m^3 K)]
                                           // Su(a_v h T_wall):
                e_eqn.source[c] += v * coeff * tw[c];
                // Sp(a_v h / Cp) on he  (implicit sink toward T_wall):
                e_eqn.ldu.diag[c] += v * coeff / cp[c];
                // Direct fluid power q''' * alpha:
                e_eqn.source[c] += v * alpha[c] * q[c];
            }
        }

        // `EEqn.solve()` (EEqn_1p.H:94) solves IN PLACE: OpenFOAM's
        // `fvMatrix<scalar>::solveSegregated` hands `psi.primitiveFieldRef()`
        // to the linear solver as its starting iterate (fvScalarMatrix.C:170)
        // and then calls `psi.correctBoundaryConditions()` (:179), so `he`
        // keeps its boundary conditions and warm-starts from its current
        // value (gh:#1152). The solver is upstream's `smoothSolver` +
        // `symGaussSeidel` ([`smooth_solve_sym_gauss_seidel`]).
        let he_bcs = crate::solvers::bc_util::capture_bcs(&self.fluid.enthalpy().boundary);
        let mut he_new = self.fluid.enthalpy().clone();
        let b: Vec<f64> = e_eqn.source.iter().copied().collect();
        smooth_solve_sym_gauss_seidel(
            &e_eqn.ldu,
            &b,
            he_new.internal.as_mut_slice(),
            self.energy_solver,
        );
        crate::solvers::bc_util::correct_bcs(&mut he_new, &he_bcs);
        *self.fluid.enthalpy_mut() = he_new;
        // Update the temperature placeholder T = he/Cp.
        let t = self.temperature();
        *self.fluid.temperature_mut() = t;
        Ok(())
    }

    /// The `alpha * rho` field (fluid volume fraction times density).
    fn alpha_rho(&self) -> VolScalarField {
        let mut ar = VolScalarField::zeros("alphaRho", self.mesh.clone());
        let a = self.fluid.alpha().internal.as_slice();
        let r = self.fluid.density().internal.as_slice();
        let o = ar.internal.as_mut_slice();
        for i in 0..o.len() {
            o[i] = a[i] * r[i];
        }
        ar
    }
}

/// OpenFOAM's `smoothSolver` with the `symGaussSeidel` smoother, solving
/// `A x = b` in place from the iterate already in `x`, with `relTol 0`.
///
/// Port of `smoothSolver::solve` (OpenFOAM-10
/// `src/OpenFOAM/matrices/lduMatrix/solvers/smoothSolver/smoothSolver.C:112-189`,
/// `nSweeps 1`, `minIter 0`), `symGaussSeidelSmoother::smooth`
/// (`smoothers/symGaussSeidel/symGaussSeidelSmoother.C:125-206`) and the
/// residual normalisation `lduMatrix::solver::normFactor`
/// (`lduMatrix/lduMatrixSolver.C:174-193`, `small_ = 1e-20`), which is fixed
/// from the starting iterate and measured about its mean, unlike basic-lib's
/// [`LduMatrix::normalised_residual`]. The convergence check runs before the
/// first sweep, so an already-converged field is left untouched. Coupled
/// (processor/cyclic) interfaces are not handled: the seams basic-lib adds
/// to the LDU addressing are swept as ordinary faces. Upstream sweeps faces
/// in owner order; this sorts them by owner (stable) to the same effect.
/// Returns `(sweeps, initial residual, final residual)`.
pub(crate) fn smooth_solve_sym_gauss_seidel(
    a: &LduMatrix,
    b: &[f64],
    x: &mut [f64],
    settings: SolverSettings,
) -> (usize, f64, f64) {
    let n = a.n_cells;
    let nf = a.n_internal_faces;
    // ownerStart addressing (lduAddressing::ownerStartAddr).
    let mut faces: Vec<usize> = (0..nf).collect();
    faces.sort_by_key(|&f| a.owner[f]);
    let mut own_start = vec![0usize; n + 1];
    for &f in &faces {
        own_start[a.owner[f] + 1] += 1;
    }
    for c in 0..n {
        own_start[c + 1] += own_start[c];
    }

    let ax = a.multiply(x);
    // normFactor: sum(|A x - sumA xRef| + |b - sumA xRef|) + small.
    let x_ref = x.iter().sum::<f64>() / n.max(1) as f64;
    let mut sum_a = a.diag.clone();
    for f in 0..nf {
        sum_a[a.neighbour[f]] += a.lower[f];
        sum_a[a.owner[f]] += a.upper[f];
    }
    let norm_factor = (0..n)
        .map(|c| {
            let t = sum_a[c] * x_ref;
            (ax[c] - t).abs() + (b[c] - t).abs()
        })
        .sum::<f64>()
        + 1e-20;
    let initial = (0..n).map(|c| (b[c] - ax[c]).abs()).sum::<f64>() / norm_factor;
    let mut residual = initial;
    let mut sweeps = 0;
    if residual < settings.tolerance {
        return (sweeps, initial, residual);
    }
    let mut b_prime = vec![0.0_f64; n];
    loop {
        b_prime.copy_from_slice(b);
        for c in 0..n {
            let mut psi = b_prime[c];
            for &f in &faces[own_start[c]..own_start[c + 1]] {
                psi -= a.upper[f] * x[a.neighbour[f]];
            }
            psi /= a.diag[c];
            for &f in &faces[own_start[c]..own_start[c + 1]] {
                b_prime[a.neighbour[f]] -= a.lower[f] * psi;
            }
            x[c] = psi;
        }
        for c in (0..n).rev() {
            let mut psi = b_prime[c];
            for &f in &faces[own_start[c]..own_start[c + 1]] {
                psi -= a.upper[f] * x[a.neighbour[f]];
            }
            psi /= a.diag[c];
            for &f in &faces[own_start[c]..own_start[c + 1]] {
                b_prime[a.neighbour[f]] -= a.lower[f] * psi;
            }
            x[c] = psi;
        }
        sweeps += 1;
        let ax = a.multiply(x);
        residual = (0..n).map(|c| (b[c] - ax[c]).abs()).sum::<f64>() / norm_factor;
        if sweeps >= settings.max_iter || residual < settings.tolerance {
            return (sweeps, initial, residual);
        }
    }
}

/// **New work, not port (gh:#1149).** A temperature-dependent effective
/// conductivity for the porous energy equation of [`OnePhaseSolver`]: one
/// [`ConductivityModel`] per region and the region of every cell.
///
/// GeN-Foam's `onePhase` carries a constant `alphaEff`; this lets a porous
/// region (e.g. a stagnant pebble bed's effective conductivity, once
/// gh:#1148's closures exist) vary with temperature. The models are the same
/// enum the solid conduction field uses
/// ([`crate::genfoam::thermal_hydraulics::solid_conduction`]). The models
/// must return a positive conductivity over the temperatures reached; unlike
/// the solid field, the porous driver does not check it.
#[derive(Debug, Clone)]
pub struct PorousConductivity {
    cell_region: Vec<usize>,
    models: Vec<ConductivityModel>,
}

impl PorousConductivity {
    /// `cell_region[c]` indexes `models` for each of the mesh's `n_cells`.
    ///
    /// # Errors
    /// [`SolidConductionError::InvalidInput`] for the wrong length or a
    /// region index with no model.
    pub fn new(
        n_cells: usize,
        cell_region: Vec<usize>,
        models: Vec<ConductivityModel>,
    ) -> Result<Self, SolidConductionError> {
        if cell_region.len() != n_cells {
            return Err(SolidConductionError::InvalidInput(format!(
                "{} cell regions for {n_cells} cells",
                cell_region.len()
            )));
        }
        if let Some(r) = cell_region.iter().find(|r| **r >= models.len()) {
            return Err(SolidConductionError::InvalidInput(format!(
                "region {r} has no conductivity model ({} given)",
                models.len()
            )));
        }
        Ok(Self {
            cell_region,
            models,
        })
    }

    /// Write `alphaEff = lambda(he/Cp)/Cp` per cell.
    fn update_alpha_eff(&self, he: &[f64], cp: &[f64], alpha_eff: &mut [f64]) {
        for c in 0..alpha_eff.len() {
            let t = he[c] / cp[c];
            alpha_eff[c] = self.models[self.cell_region[c]].0.value(t) / cp[c];
        }
    }
}
