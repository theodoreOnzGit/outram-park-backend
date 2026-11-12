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

//! # `rho_central_foam` — density-based compressible solver (rhoCentralFoam)
//!
//! Rust port of rhoCentralFoam: a density-based, explicit compressible solver
//! using the Kurganov central-upwind flux with 2nd-order van Leer
//! reconstruction. It advances the conserved compressible fields
//! (ρ, ρU, ρE) and is well suited to shock-dominated high-speed flow (the Sod
//! shock-tube validation case exercises this solver). See [`RhoCentralFoam`].
//!
//! # Upstream (gh:#1363)
//! [`RhoCentralFoam::step`] follows OpenFOAM-v2506 (ESI)
//! `applications/solvers/compressible/rhoCentralFoam/rhoCentralFoam.C` in the
//! inviscid case, operation by operation: directed reconstruction of `rho`,
//! `rhoU` (one vector limiter per face, `vanLeerV`), `rPsi = 1/psi`, `e` and
//! `c = sqrt(Cp/Cv rPsi)`; the Kurganov weights `a_pos`, `aSf`; the fluxes
//! `phi`, `phiUp`, `phiEp`; `solve(fvm::ddt(X) + fvc::div(flux))` with the
//! `diagonal` solver; `e = rhoE/rho - |U|^2/2`; the temperature from the
//! energy by upstream's Newton iteration (`species::thermo::T`); and
//! `p = rho/psi`. Boundary faces carry their own state (`rho`, `rhoU`, `T`,
//! `psi`, `e`) as upstream's boundary fields do. Before gh:#1363 this was a
//! different formulation (reconstructing `U` and `e`, deriving `p` and `c`
//! from them, a hard-coded γ = 1.4, positivity clamps); see
//! `verification_and_validation/rho_central_foam_vs_openfoam.md`.

use crate::error::AppBuilderError;
use crate::io::control_dict::{ControlDict, StartControl, StopControl};
use crate::io::fv_schemes::FvSchemes;
use crate::io::fv_solution::FvSolution;
use outram_foam_basic_lib::prelude::*;
use std::sync::Arc;

/// Standard temperature [K], upstream `Tstd`.
const T_STD: f64 = 298.15;
/// Universal gas constant [J/(kmol K)], upstream `RR = 1e3*physicoChemical::R`,
/// `R = NA*k` with the `etc/controlDict` values `NA = 6.0221417930e23` (CODATA
/// 2006, the DimensionedConstants default) and `k = 1.38065e-23`.
const RR: f64 = 1.0e3 * (6.0221417930e23 * 1.38065e-23);

/// Perfect gas with constant `Cp` and the sensible internal energy as the
/// energy variable: upstream's `perfectGas` + `hConstThermo` +
/// `sensibleInternalEnergy` (`thermoType` of the rhoCentralFoam shockTube
/// tutorial), with the same arithmetic.
///
/// `Es(p, T) = Hs - p/rho`, `Hs = Cp (T - Tref)`, `rho = p/(R T)`,
/// `psi = 1/(R T)`, `Cv = Cp - R`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerfectGas {
    /// Specific gas constant `R = RR/W` [J/(kg K)].
    pub r: f64,
    /// Heat capacity at constant pressure [J/(kg K)].
    pub cp: f64,
    /// Reference temperature of the sensible enthalpy [K] (upstream `Tref`,
    /// default `Tstd`).
    pub t_ref: f64,
}

impl PerfectGas {
    /// Upstream's `thermodynamics { Cp; }`, `specie { molWeight; }` with the
    /// default `Tref = Tstd = 298.15 K` and `Hsref = 0`.
    #[must_use]
    pub fn from_molar_weight_and_cp(molar_weight_kg_per_kmol: f64, cp: f64) -> Self {
        Self { r: RR / molar_weight_kg_per_kmol, cp, t_ref: T_STD }
    }

    /// A calorically perfect gas of ratio of specific heats `gamma` and gas
    /// constant `r`, with `Tref = 0` so that `e = Cv T` (the convention of the
    /// solver before gh:#1363, and of every caller that sets `e = p/((γ-1)ρ)`).
    #[must_use]
    pub fn from_gamma(gamma: f64, r: f64) -> Self {
        Self { r, cp: gamma * r / (gamma - 1.0), t_ref: 0.0 }
    }

    /// `Cv = Cp - R` [J/(kg K)].
    #[must_use]
    pub fn cv(&self) -> f64 {
        self.cp - self.r
    }

    /// Compressibility `psi = 1/(R T)` [s²/m²].
    #[must_use]
    pub fn psi(&self, t: f64) -> f64 {
        1.0 / (self.r * t)
    }

    /// Sensible internal energy `Es(p, T)` [J/kg].
    #[must_use]
    pub fn es(&self, p: f64, t: f64) -> f64 {
        let rho = p / (self.r * t);
        let hs = self.cp * (t - self.t_ref) + 0.0 + 0.0;
        hs - p / rho
    }

    /// Temperature from the sensible internal energy, upstream
    /// `species::thermo::T` (Newton iteration, `tol = 1e-4`, at most 100
    /// iterations) started from `t0`.
    ///
    /// # Errors
    /// [`AppBuilderError::Diverged`] if `t0 < 0` or the iteration does not
    /// converge (upstream aborts in both cases).
    pub fn the(&self, e: f64, p: f64, t0: f64) -> Result<f64, AppBuilderError> {
        if t0 < 0.0 {
            // upstream: "Negative initial temperature T0"; `residual` carries T0
            return Err(AppBuilderError::Diverged { iter: 0, residual: t0 });
        }
        let t_tol = t0 * 1.0e-4;
        let mut t_new = t0;
        let mut iter = 0;
        loop {
            let t_est = t_new;
            t_new = t_est - (self.es(p, t_est) - e) / self.cv();
            iter += 1;
            if iter > 100 + 1 {
                return Err(AppBuilderError::Diverged { iter, residual: (t_new - t_est).abs() });
            }
            if (t_new - t_est).abs() <= t_tol {
                return Ok(t_new);
            }
        }
    }
}

impl Default for PerfectGas {
    /// Calorically perfect air, γ = 1.4, R = 287 J/(kg K), `e = Cv T`: what the
    /// solver hard-coded before gh:#1363.
    fn default() -> Self {
        Self::from_gamma(1.4, 287.0)
    }
}

/// State upstream keeps as fields besides `rho`, `U`, `e`, `p`: the conserved
/// `rhoU`, `rhoE`, the temperature and compressibility, and the boundary-face
/// values of each (gh:#1363). Created from the public fields at the first step.
#[derive(Debug, Clone)]
struct Conserved {
    rho_u: Vec<Vector3>,
    rho_e: Vec<f64>,
    t: Vec<f64>,
    psi: Vec<f64>,
    // Boundary faces, indexed by `global face - n_internal_faces`.
    rho_b: Vec<f64>,
    rho_u_b: Vec<Vector3>,
    rho_e_b: Vec<f64>,
    e_b: Vec<f64>,
    t_b: Vec<f64>,
    psi_b: Vec<f64>,
    p_b: Vec<f64>,
    u_b: Vec<Vector3>,
}

/// Density-based central-upwind compressible solver — rhoCentralFoam.
///
/// Implements the **Kurganov** central-upwind scheme for the Euler
/// equations (the viscous branch of upstream is not ported: `mu = 0`). All
/// convective terms are explicit — no matrix solve for transport, only
/// upstream's `diagonal` solver. Only suitable for time-accurate problems with
/// CFL ≤ 1.
///
/// Governing equations (conservation form):
///   ∂W/∂t + ∇·F(W) = 0
///   W = [ρ, ρU, ρE]ᵀ,  E = e + ½|U|²,  p = ρ/ψ,  ψ = 1/(R T),  e = Es(p, T)
///
/// C++ solver: `applications/solvers/compressible/rhoCentralFoam/`
#[derive(Debug)]
pub struct RhoCentralFoam {
    pub mesh: Arc<FvMesh>,
    pub control: ControlDict,
    pub schemes: FvSchemes,
    pub solution: FvSolution,
    /// Velocity field [m/s]
    pub u: VolVectorField,
    /// Pressure [Pa]
    pub p: VolScalarField,
    /// Density [kg/m³]
    pub rho: VolScalarField,
    /// Specific sensible internal energy e [J/kg], in the convention of
    /// [`Self::gas`] (`e = Cv T` for the default gas).
    pub e: VolScalarField,
    /// Unused; kept for API compatibility.
    pub psi_limit: f64,
    /// Mass flux output [kg/s]
    pub phi: SurfaceScalarField,
    /// The gas: default calorically perfect air (γ = 1.4). Set it with
    /// [`Self::with_gas`], e.g. from a `thermophysicalProperties`' `molWeight`
    /// and `Cp`.
    pub gas: PerfectGas,
    /// Optional initial temperature per cell, upstream's `T` file, from which
    /// the first energy-to-temperature iteration starts. `None`: `p/(ρ R)`.
    initial_t: Option<Vec<f64>>,
    state: Option<Conserved>,
}

/// Which velocity components are solved: upstream `polyMesh::calcDirections`,
/// a component is excluded (`solutionD = -1`) when the summed `cmptMag(Sf)` of
/// the `empty` patches, normalised, exceeds `1e-6` in it.
fn solution_directions(mesh: &FvMesh) -> [bool; 3] {
    let mut sum = [0.0_f64; 3];
    for patch in &mesh.patches {
        if patch.kind != PatchKind::Empty {
            continue;
        }
        for fi in 0..patch.size {
            let sf = mesh.face_area_vectors[patch.start + fi];
            sum[0] += sf.x.abs();
            sum[1] += sf.y.abs();
            sum[2] += sf.z.abs();
        }
    }
    let mag = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt();
    if mag <= 0.0 {
        return [true; 3];
    }
    [sum[0] / mag <= 1e-6, sum[1] / mag <= 1e-6, sum[2] / mag <= 1e-6]
}

fn is_empty_patch(solver: &RhoCentralFoam, pi: usize) -> bool {
    matches!(solver.u.boundary[pi].bc, BoundaryCondition::Empty)
        || solver.mesh.patches[pi].kind == PatchKind::Empty
}

impl RhoCentralFoam {
    /// Build a density-based central-upwind (Kurganov) compressible solver on
    /// `mesh`, with every field allocated at a placeholder initial state.
    ///
    /// This solver is **explicit**: it takes no pressure-correction iterations, so
    /// `solution`'s PIMPLE controls are unused and the time step must satisfy the
    /// acoustic CFL condition. The caller sets the initial discontinuity (for a
    /// shock tube, the left and right states) on the public members before
    /// stepping.
    ///
    /// | Field | Initial value |
    /// |---|---|
    /// | `u` — velocity [m/s] | zero |
    /// | `p` — static pressure, Pa | uniform 1.0e5 |
    /// | `rho` — density [kg/m³] | uniform 1.0 |
    /// | `e` — specific internal energy [J/kg] | zero |
    /// | `psi_limit` | 1.0 (unused) |
    /// | `phi` — mass face flux [kg/s] | zero |
    /// | `gas` | [`PerfectGas::default`] (γ = 1.4) |
    ///
    /// `p`, `rho` and `e` must be set consistently with [`Self::gas`]:
    /// `e = gas.es(p, T)`, `rho = p/(R T)`.
    ///
    /// # Arguments
    ///
    /// See [`crate::solvers::pimple_foam::PimpleFoam::new`] — the four arguments
    /// have the same meaning and the same honoured-field caveats.
    pub fn new(
        mesh: Arc<FvMesh>,
        control: ControlDict,
        schemes: FvSchemes,
        solution: FvSolution,
    ) -> Self {
        let u = VolVectorField::zero("U", mesh.clone());
        let p = VolScalarField::uniform("p", mesh.clone(), 1.0e5);
        let rho = VolScalarField::uniform("rho", mesh.clone(), 1.0);
        let e = VolScalarField::zeros("e", mesh.clone());
        let phi = SurfaceScalarField::zeros("phi", mesh.clone());
        Self {
            mesh,
            control,
            schemes,
            solution,
            u,
            p,
            rho,
            e,
            psi_limit: 1.0,
            phi,
            gas: PerfectGas::default(),
            initial_t: None,
            state: None,
        }
    }

    /// Choose the gas (upstream `thermophysicalProperties`).
    #[must_use]
    pub fn with_gas(mut self, gas: PerfectGas) -> Self {
        self.gas = gas;
        self
    }

    /// Give the initial temperature per cell (upstream's `T` file), the start
    /// of the first energy-to-temperature iteration. Without it the start is
    /// `p/(ρ R)`, which differs from the file's `T` by round-off only.
    pub fn set_initial_temperature(&mut self, t: Vec<f64>) {
        self.initial_t = Some(t);
    }

    /// Cell temperatures [K] (`None` before the first step).
    #[must_use]
    pub fn temperature(&self) -> Option<&[f64]> {
        self.state.as_ref().map(|s| s.t.as_slice())
    }

    /// Upstream `createFields.H` + the thermo constructor: derive `T`, `psi`,
    /// `rhoU`, `rhoE` and the boundary values from the public fields.
    fn init_state(&self) -> Result<Conserved, AppBuilderError> {
        let mesh = &self.mesh;
        let n = mesh.n_cells;
        let n_int = mesh.n_internal_faces;
        let n_b = mesh.n_faces - n_int;
        let gas = self.gas;
        let (rho, p, e) = (self.rho.internal.as_slice(), self.p.internal.as_slice(), self.e.internal.as_slice());
        let u = self.u.internal.as_slice();

        let mut t = vec![0.0; n];
        let mut psi = vec![0.0; n];
        for c in 0..n {
            let t0 = match &self.initial_t {
                Some(v) => v[c],
                None => p[c] / (rho[c] * gas.r),
            };
            t[c] = gas.the(e[c], p[c], t0)?;
            psi[c] = gas.psi(t[c]);
        }
        let mag_sqr = |v: Vector3| v.x * v.x + v.y * v.y + v.z * v.z;
        let rho_u: Vec<Vector3> =
            (0..n).map(|c| Vector3::new(rho[c] * u[c].x, rho[c] * u[c].y, rho[c] * u[c].z)).collect();
        let rho_e: Vec<f64> = (0..n).map(|c| rho[c] * (e[c] + 0.5 * mag_sqr(u[c]))).collect();

        let mut s = Conserved {
            rho_u,
            rho_e,
            t,
            psi,
            rho_b: vec![0.0; n_b],
            rho_u_b: vec![Vector3::ZERO; n_b],
            rho_e_b: vec![0.0; n_b],
            e_b: vec![0.0; n_b],
            t_b: vec![0.0; n_b],
            psi_b: vec![0.0; n_b],
            p_b: vec![0.0; n_b],
            u_b: vec![Vector3::ZERO; n_b],
        };
        for gf in n_int..mesh.n_faces {
            let o = mesh.owner[gf];
            let b = gf - n_int;
            s.p_b[b] = p[o];
            s.e_b[b] = e[o];
            let t_b0 = match &self.initial_t {
                Some(v) => v[o],
                None => p[o] / (rho[o] * gas.r),
            };
            s.t_b[b] = gas.the(s.e_b[b], s.p_b[b], t_b0)?;
            s.psi_b[b] = gas.psi(s.t_b[b]);
            s.rho_b[b] = s.psi_b[b] * s.p_b[b];
            s.u_b[b] = u[o];
            let ub = s.u_b[b];
            s.rho_u_b[b] = Vector3::new(s.rho_b[b] * ub.x, s.rho_b[b] * ub.y, s.rho_b[b] * ub.z);
            s.rho_e_b[b] = s.rho_b[b] * (s.e_b[b] + 0.5 * mag_sqr(ub));
        }
        Ok(s)
    }

    /// A scalar field with the given cell values and EXPLICIT boundary-face
    /// values (upstream's boundary field of a derived `volScalarField`).
    fn scalar_field(&self, name: &str, cells: &[f64], boundary: &[f64]) -> VolScalarField {
        let n_int = self.mesh.n_internal_faces;
        let patches = self
            .mesh
            .patches
            .iter()
            .map(|patch| {
                let vals = Field::from_fn(patch.size, |fi| boundary[patch.start + fi - n_int]);
                PatchField { bc: BoundaryCondition::Calculated(vals.clone()), values: vals }
            })
            .collect();
        VolScalarField::new(name, self.mesh.clone(), Field::new(cells.to_vec()), patches)
    }

    /// As [`Self::scalar_field`], for a vector field.
    fn vector_field(&self, name: &str, cells: &[Vector3], boundary: &[Vector3]) -> VolVectorField {
        let n_int = self.mesh.n_internal_faces;
        let patches = self
            .mesh
            .patches
            .iter()
            .map(|patch| {
                let vals = Field::from_fn(patch.size, |fi| boundary[patch.start + fi - n_int]);
                PatchField { bc: BoundaryCondition::Calculated(vals.clone()), values: vals }
            })
            .collect();
        VolVectorField::new(name, self.mesh.clone(), Field::new(cells.to_vec()), patches)
    }

    /// One explicit Kurganov time step, operation for operation as upstream's
    /// `rhoCentralFoam.C` (inviscid branch).
    ///
    /// # Errors
    /// [`AppBuilderError::Diverged`] if the energy-to-temperature iteration
    /// fails (negative temperature, no convergence), as upstream aborts.
    pub fn step(&mut self) -> Result<(), AppBuilderError> {
        if self.state.is_none() {
            self.state = Some(self.init_state()?);
        }
        let mut st = self.state.take().expect("state initialised above");
        let result = self.step_with(&mut st);
        self.state = Some(st);
        result
    }

    fn step_with(&mut self, st: &mut Conserved) -> Result<(), AppBuilderError> {
        let mesh = self.mesh.clone();
        let n = mesh.n_cells;
        let n_int = mesh.n_internal_faces;
        let gas = self.gas;
        let dt = self.control.delta_t;
        let mag_sqr = |v: Vector3| v.x * v.x + v.y * v.y + v.z * v.z;

        // ── Directed interpolation of the primitive fields onto faces ───────
        let rho_cells: Vec<f64> = self.rho.internal.as_slice().to_vec();
        let rho_old = rho_cells.clone();
        let rho_u_old = st.rho_u.clone();
        let rho_e_old = st.rho_e.clone();
        let lim = fvc::Limiter::VanLeer;

        let rho_f = self.scalar_field("rho", &rho_cells, &st.rho_b);
        let (rho_pos, rho_neg) = fvc::reconstruct_pos_neg(&rho_f, lim);
        let rho_u_f = self.vector_field("rhoU", &st.rho_u, &st.rho_u_b);
        let (rho_u_pos, rho_u_neg) = fvc::reconstruct_pos_neg_vector(&rho_u_f, lim);

        let r_psi: Vec<f64> = st.psi.iter().map(|&p| 1.0 / p).collect();
        let r_psi_b: Vec<f64> = st.psi_b.iter().map(|&p| 1.0 / p).collect();
        let r_psi_f = self.scalar_field("rPsi", &r_psi, &r_psi_b);
        let (r_psi_pos, r_psi_neg) = fvc::reconstruct_pos_neg(&r_psi_f, lim);

        let e_cells: Vec<f64> = self.e.internal.as_slice().to_vec();
        let e_f = self.scalar_field("e", &e_cells, &st.e_b);
        let (e_pos, e_neg) = fvc::reconstruct_pos_neg(&e_f, lim);

        // c = sqrt(Cp/Cv * rPsi), cells and boundary faces
        let cp_by_cv = gas.cp / gas.cv();
        let c_cells: Vec<f64> = r_psi.iter().map(|&r| (cp_by_cv * r).sqrt()).collect();
        let c_b: Vec<f64> = r_psi_b.iter().map(|&r| (cp_by_cv * r).sqrt()).collect();
        let c_f = self.scalar_field("c", &c_cells, &c_b);
        let (c_pos, c_neg) = fvc::reconstruct_pos_neg(&c_f, lim);

        // ── Kurganov fluxes: one closure for an internal or boundary face ───
        // Inputs are the pos/neg face values; the order of operations is the
        // upstream surface-field expressions.
        struct Face {
            phi: f64,
            phi_up: Vector3,
            phi_ep: f64,
        }
        let face_flux = |rho_p: f64,
                         rho_n: f64,
                         ru_p: Vector3,
                         ru_n: Vector3,
                         rpsi_p: f64,
                         rpsi_n: f64,
                         e_p: f64,
                         e_n: f64,
                         c_p: f64,
                         c_n: f64,
                         sf: Vector3,
                         mag_sf: f64| {
            let u_p = Vector3::new(ru_p.x / rho_p, ru_p.y / rho_p, ru_p.z / rho_p);
            let u_n = Vector3::new(ru_n.x / rho_n, ru_n.y / rho_n, ru_n.z / rho_n);
            let p_p = rho_p * rpsi_p;
            let p_n = rho_n * rpsi_n;
            let mut phiv_pos = u_p.x * sf.x + u_p.y * sf.y + u_p.z * sf.z;
            let mut phiv_neg = u_n.x * sf.x + u_n.y * sf.y + u_n.z * sf.z;
            let c_sf_pos = c_p * mag_sf;
            let c_sf_neg = c_n * mag_sf;
            let ap = (phiv_pos + c_sf_pos).max(phiv_neg + c_sf_neg).max(0.0);
            let am = (phiv_pos - c_sf_pos).min(phiv_neg - c_sf_neg).min(0.0);
            let a_pos = ap / (ap - am);
            let a_sf = am * a_pos;
            let a_neg = 1.0 - a_pos;
            phiv_pos *= a_pos;
            phiv_neg *= a_neg;
            let aphiv_pos = phiv_pos - a_sf;
            let aphiv_neg = phiv_neg + a_sf;
            let phi = aphiv_pos * rho_p + aphiv_neg * rho_n;
            let phi_u = Vector3::new(
                aphiv_pos * ru_p.x + aphiv_neg * ru_n.x,
                aphiv_pos * ru_p.y + aphiv_neg * ru_n.y,
                aphiv_pos * ru_p.z + aphiv_neg * ru_n.z,
            );
            let pr = a_pos * p_p + a_neg * p_n;
            let phi_up = Vector3::new(phi_u.x + pr * sf.x, phi_u.y + pr * sf.y, phi_u.z + pr * sf.z);
            let phi_ep = aphiv_pos * (rho_p * (e_p + 0.5 * mag_sqr(u_p)) + p_p)
                + aphiv_neg * (rho_n * (e_n + 0.5 * mag_sqr(u_n)) + p_n)
                + a_sf * p_p
                - a_sf * p_n;
            Face { phi, phi_up, phi_ep }
        };

        // fvc::div = surfaceIntegrate: owner += , neighbour -= , boundary
        // += (faces in order), then divide by the cell volume.
        let mut div_phi = vec![0.0_f64; n];
        let mut div_phi_up = vec![Vector3::ZERO; n];
        let mut div_phi_ep = vec![0.0_f64; n];

        for f in 0..n_int {
            let (o, nb) = (mesh.owner[f], mesh.neighbour[f]);
            let sf = mesh.face_area_vectors[f];
            let fl = face_flux(
                rho_pos.internal[f],
                rho_neg.internal[f],
                rho_u_pos[f],
                rho_u_neg[f],
                r_psi_pos.internal[f],
                r_psi_neg.internal[f],
                e_pos.internal[f],
                e_neg.internal[f],
                c_pos.internal[f],
                c_neg.internal[f],
                sf,
                mesh.face_areas[f],
            );
            self.phi.internal[f] = fl.phi;
            div_phi[o] += fl.phi;
            div_phi[nb] -= fl.phi;
            div_phi_up[o] = Vector3::new(div_phi_up[o].x + fl.phi_up.x, div_phi_up[o].y + fl.phi_up.y, div_phi_up[o].z + fl.phi_up.z);
            div_phi_up[nb] = Vector3::new(div_phi_up[nb].x - fl.phi_up.x, div_phi_up[nb].y - fl.phi_up.y, div_phi_up[nb].z - fl.phi_up.z);
            div_phi_ep[o] += fl.phi_ep;
            div_phi_ep[nb] -= fl.phi_ep;
        }
        for pi in 0..mesh.patches.len() {
            if is_empty_patch(self, pi) {
                continue;
            }
            let patch = mesh.patches[pi].clone();
            for fi in 0..patch.size {
                let gf = patch.start + fi;
                let b = gf - n_int;
                let o = mesh.owner[gf];
                // pos = neg = the boundary-face value
                let fl = face_flux(
                    st.rho_b[b],
                    st.rho_b[b],
                    st.rho_u_b[b],
                    st.rho_u_b[b],
                    r_psi_b[b],
                    r_psi_b[b],
                    st.e_b[b],
                    st.e_b[b],
                    c_b[b],
                    c_b[b],
                    mesh.face_area_vectors[gf],
                    mesh.face_areas[gf],
                );
                self.phi.boundary[pi].values[fi] = fl.phi;
                div_phi[o] += fl.phi;
                div_phi_up[o] = Vector3::new(div_phi_up[o].x + fl.phi_up.x, div_phi_up[o].y + fl.phi_up.y, div_phi_up[o].z + fl.phi_up.z);
                div_phi_ep[o] += fl.phi_ep;
            }
        }

        // solve(fvm::ddt(X) + fvc::div(flux)) with the diagonal solver:
        //   diag = rDeltaT*V, source = rDeltaT*X0*V - V*div, X = source/diag
        let r_delta_t = 1.0 / dt;
        let mut rho_new = vec![0.0_f64; n];
        let mut rho_u_new = vec![Vector3::ZERO; n];
        let mut rho_e_new = vec![0.0_f64; n];
        for c in 0..n {
            let v = mesh.cell_volumes[c];
            let div_c = div_phi[c] / v;
            let diag = r_delta_t * v;
            rho_new[c] = (r_delta_t * rho_old[c] * v - v * div_c) / diag;
            let d = div_phi_up[c];
            let (dx, dy, dz) = (d.x / v, d.y / v, d.z / v);
            let o = rho_u_old[c];
            rho_u_new[c] = Vector3::new(
                (r_delta_t * o.x * v - v * dx) / diag,
                (r_delta_t * o.y * v - v * dy) / diag,
                (r_delta_t * o.z * v - v * dz) / diag,
            );
            let de = div_phi_ep[c] / v;
            rho_e_new[c] = (r_delta_t * rho_e_old[c] * v - v * de) / diag;
        }

        // upstream `fvMatrix<vector>::solveSegregated` does not solve a
        // component along the normal of an `empty` patch (`polyMesh::
        // solutionD() == -1`): it keeps its old value. Without this the tiny
        // `Sf_y`, `Sf_z` of the internal faces seed round-off momentum there
        // that upstream does not have (and that then enters the vector limiter).
        let solve_dir = solution_directions(&mesh);
        for c in 0..n {
            let o = rho_u_old[c];
            let m = &mut rho_u_new[c];
            if !solve_dir[0] {
                m.x = o.x;
            }
            if !solve_dir[1] {
                m.y = o.y;
            }
            if !solve_dir[2] {
                m.z = o.z;
            }
        }

        // U = rhoU/rho; U boundary = zeroGradient; rhoU_b = rho_b(old)*U_b
        let mut u_new = vec![Vector3::ZERO; n];
        for c in 0..n {
            u_new[c] = Vector3::new(rho_u_new[c].x / rho_new[c], rho_u_new[c].y / rho_new[c], rho_u_new[c].z / rho_new[c]);
        }
        let n_b = st.rho_b.len();
        for gf in n_int..mesh.n_faces {
            let b = gf - n_int;
            let ub = u_new[mesh.owner[gf]];
            st.u_b[b] = ub;
            st.rho_u_b[b] = Vector3::new(st.rho_b[b] * ub.x, st.rho_b[b] * ub.y, st.rho_b[b] * ub.z);
        }

        // e = rhoE/rho - 0.5|U|^2 (cells); e boundary = zeroGradient
        let mut e_new = vec![0.0_f64; n];
        for c in 0..n {
            e_new[c] = rho_e_new[c] / rho_new[c] - 0.5 * mag_sqr(u_new[c]);
        }
        for gf in n_int..mesh.n_faces {
            st.e_b[gf - n_int] = e_new[mesh.owner[gf]];
        }

        // thermo.correct(): T from e with the OLD p, then psi
        let p_old: Vec<f64> = self.p.internal.as_slice().to_vec();
        for c in 0..n {
            st.t[c] = gas.the(e_new[c], p_old[c], st.t[c])?;
            st.psi[c] = gas.psi(st.t[c]);
        }
        for b in 0..n_b {
            st.t_b[b] = gas.the(st.e_b[b], st.p_b[b], st.t_b[b])?;
            st.psi_b[b] = gas.psi(st.t_b[b]);
        }
        // rhoE_b = rho_b(old)*(e_b + 0.5|U_b|^2)
        for b in 0..n_b {
            st.rho_e_b[b] = st.rho_b[b] * (st.e_b[b] + 0.5 * mag_sqr(st.u_b[b]));
        }

        // p = rho/psi (cells), p_b = zeroGradient, rho_b = psi_b*p_b
        let mut p_new = vec![0.0_f64; n];
        for c in 0..n {
            p_new[c] = rho_new[c] / st.psi[c];
        }
        for gf in n_int..mesh.n_faces {
            let b = gf - n_int;
            st.p_b[b] = p_new[mesh.owner[gf]];
            st.rho_b[b] = st.psi_b[b] * st.p_b[b];
        }

        st.rho_u = rho_u_new;
        st.rho_e = rho_e_new;
        for c in 0..n {
            self.rho.internal[c] = rho_new[c];
            self.u.internal[c] = u_new[c];
            self.e.internal[c] = e_new[c];
            self.p.internal[c] = p_new[c];
        }
        Ok(())
    }

    /// Advance the solver from the `controlDict` start time to its end time,
    /// calling [`Self::step`] repeatedly at the fixed step `control.delta_t`.
    ///
    /// This is a convenience wrapper over [`Self::step`]. Set the initial field
    /// state on the public members *before* calling it, and read the results off
    /// those same members afterwards — **`run` writes nothing to disk**, because
    /// every writer in [`crate::io::output`] is still `todo!()`.
    ///
    /// # Time control
    ///
    /// * Starts at `t` for [`StartControl::StartTime(t)`](StartControl::StartTime);
    ///   any other `startFrom` selection is treated as `t = 0`.
    /// * Runs while `t < end` for
    ///   [`StopControl::EndTime(end)`](StopControl::EndTime). **For every other
    ///   `stopAt` selection this returns `Ok(())` immediately without taking a
    ///   single step**, since those selections are defined in terms of a write
    ///   this crate cannot perform.
    /// * The step is fixed at `control.delta_t`. `adjustTimeStep`, `maxCo` and
    ///   `maxDeltaT` are **not implemented** — choose a Δt that respects your own
    ///   Courant limit.
    ///
    /// # Errors
    ///
    /// Propagates the first error from [`Self::step`] (typically
    /// [`AppBuilderError::Diverged`]); the run stops at that point with the
    /// fields left in their last state.
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
