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
//! The fluid **thermophysical package** (`he <-> T`, `rho(p,T)`) is still a
//! scaffold in [`super::super::thermophysical`], so this driver runs with
//! **constant fluid properties** supplied on the [`Fluid`] fields (`rho`, `mu`,
//! `Cp`, `kappa`) and treats the enthalpy as `he = Cp * T` about `T = 0`. That
//! is sufficient for the incompressible porous momentum/energy physics that is
//! the ported slice's purpose (friction pressure drop, structure heat coupling)
//! and is what the V&V exercises. The structure side is wired as a
//! fixed-surface-temperature convective coupling (a field-wide analogue of
//! GeN-Foam's `fixedTemperature` structure model); driving a full
//! [`super::super::structure::StructureCell`] fuel-pin conduction field is the
//! next slice (bead op-p6p.7.11 follow-up).
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

        let (he_new, _) = e_eqn.solve("he", SolverSettings::default());
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
