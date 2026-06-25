use std::sync::Arc;
use openfoam_basic_lib::prelude::*;
use crate::error::AppBuilderError;
use crate::io::control_dict::{ControlDict, StartControl, StopControl};
use crate::io::fv_schemes::FvSchemes;
use crate::io::fv_solution::FvSolution;

/// Heat capacity ratio γ for ideal diatomic gas (e.g. air).
const GAMMA: f64 = 1.4;

/// Density-based central-upwind compressible solver — rhoCentralFoam.
///
/// Implements the **Kurganov-Noelle-Petrova (KNP)** scheme for the Euler
/// equations.  All convective terms are treated **explicitly** — no matrix
/// solve for transport.  Only suitable for time-accurate problems with
/// CFL ≤ 1.
///
/// Governing equations (conservation form):
///   ∂W/∂t + ∇·F(W) = 0
///   W = [ρ, ρU, ρE]ᵀ,  E = e + ½|U|²,  p = (γ−1)ρe  (calorically perfect gas)
///
/// KNP flux at face f (Kurganov, Noelle & Petrova, SIAM J. Sci. Comp. 2001):
///   F_KNP = (a_R·F_L − a_L·F_R + a_L·a_R·(W_R − W_L)) / (a_R − a_L)
///   a_L = min(U_n,L − c_L,  U_n,R − c_R,  0)
///   a_R = max(U_n,L + c_L,  U_n,R + c_R,  0)
///
/// C++ solver: `applications/solvers/compressible/rhoCentralFoam/`
pub struct RhoCentralFoam {
    pub mesh:      Arc<FvMesh>,
    pub control:   ControlDict,
    pub schemes:   FvSchemes,
    pub solution:  FvSolution,
    /// Velocity field [m/s]
    pub u:         VolVectorField,
    /// Pressure [Pa]
    pub p:         VolScalarField,
    /// Density [kg/m³]
    pub rho:       VolScalarField,
    /// Specific internal energy e [J/kg]
    pub e:         VolScalarField,
    /// Co-volume limiter (unused for calorically-perfect gas; kept for API compatibility).
    pub psi_limit: f64,
    /// Mass flux output [kg/s]
    pub phi:       SurfaceScalarField,
}

impl RhoCentralFoam {
    pub fn new(
        mesh: Arc<FvMesh>,
        control: ControlDict,
        schemes: FvSchemes,
        solution: FvSolution,
    ) -> Self {
        let u   = VolVectorField::zero("U",   mesh.clone());
        let p   = VolScalarField::uniform("p",   mesh.clone(), 1.0e5);
        let rho = VolScalarField::uniform("rho", mesh.clone(), 1.0);
        let e   = VolScalarField::zeros("e",   mesh.clone());
        let phi = SurfaceScalarField::zeros("phi", mesh.clone());
        Self { mesh, control, schemes, solution, u, p, rho, e, psi_limit: 1.0, phi }
    }

    /// One explicit KNP time step.
    ///
    /// Updates the conserved variables (ρ, ρU, ρE) by summing KNP face fluxes,
    /// then recovers primitive variables (ρ, U, e, p) from the updated state.
    pub fn step(&mut self) -> Result<(), AppBuilderError> {
        let mesh = self.mesh.clone();
        let n    = mesh.n_cells;
        let dt   = self.control.delta_t;

        // ── Snapshot current primitives ──────────────────────────────────────
        let rho_sl = self.rho.internal.as_slice();
        let e_sl   = self.e.internal.as_slice();
        let u_sl   = self.u.internal.as_slice();

        // Accumulate conservative-variable tendencies:
        //   d_rho[c]  = Σ_f −flux_cont * |Sf|
        //   d_rhou[c] = Σ_f −flux_mom  * |Sf|   (Vector3)
        //   d_rhoe[c] = Σ_f −flux_ener * |Sf|
        let mut d_rho  = vec![0.0_f64;  n];
        let mut d_rhou = vec![Vector3::ZERO; n];
        let mut d_rhoe = vec![0.0_f64;  n];

        // ── Internal faces ────────────────────────────────────────────────────
        for f in 0..mesh.n_internal_faces {
            let o  = mesh.owner[f];
            let nb = mesh.neighbour[f];
            let area = mesh.face_areas[f];

            // Face unit normal (pointing from owner → neighbour)
            let sf  = mesh.face_area_vectors[f];
            let n_f = Vector3::new(sf.x / area, sf.y / area, sf.z / area);

            // Left (owner) state
            let rho_l = rho_sl[o];
            let u_l   = u_sl[o];
            let e_l   = e_sl[o].max(0.0);
            let p_l   = ((GAMMA - 1.0) * rho_l * e_l).max(0.0);
            let c_l   = (GAMMA * p_l / rho_l.max(1e-10)).sqrt();
            let u_n_l = u_l.x * n_f.x + u_l.y * n_f.y + u_l.z * n_f.z;

            // Right (neighbour) state
            let rho_r = rho_sl[nb];
            let u_r   = u_sl[nb];
            let e_r   = e_sl[nb].max(0.0);
            let p_r   = ((GAMMA - 1.0) * rho_r * e_r).max(0.0);
            let c_r   = (GAMMA * p_r / rho_r.max(1e-10)).sqrt();
            let u_n_r = u_r.x * n_f.x + u_r.y * n_f.y + u_r.z * n_f.z;

            // KNP wave-speed estimates (clamp so a_R > a_L)
            let a_l = (u_n_l - c_l).min(u_n_r - c_r).min(0.0);
            let a_r = (u_n_l + c_l).max(u_n_r + c_r).max(0.0);
            let da  = (a_r - a_l).max(1e-10);

            // Conserved variables
            let e_tot_l = e_l + 0.5 * (u_l.x * u_l.x + u_l.y * u_l.y + u_l.z * u_l.z);
            let e_tot_r = e_r + 0.5 * (u_r.x * u_r.x + u_r.y * u_r.y + u_r.z * u_r.z);
            let w_rho_l  = rho_l;
            let w_rhou_l = Vector3::new(rho_l * u_l.x, rho_l * u_l.y, rho_l * u_l.z);
            let w_rhoe_l = rho_l * e_tot_l;
            let w_rho_r  = rho_r;
            let w_rhou_r = Vector3::new(rho_r * u_r.x, rho_r * u_r.y, rho_r * u_r.z);
            let w_rhoe_r = rho_r * e_tot_r;

            // Fluxes in face-normal direction
            let h_l = e_tot_l + p_l / rho_l.max(1e-10);  // specific total enthalpy
            let h_r = e_tot_r + p_r / rho_r.max(1e-10);
            let f_cont_l = rho_l * u_n_l;
            let f_cont_r = rho_r * u_n_r;
            let f_mom_l  = Vector3::new(
                rho_l * u_n_l * u_l.x + p_l * n_f.x,
                rho_l * u_n_l * u_l.y + p_l * n_f.y,
                rho_l * u_n_l * u_l.z + p_l * n_f.z,
            );
            let f_mom_r  = Vector3::new(
                rho_r * u_n_r * u_r.x + p_r * n_f.x,
                rho_r * u_n_r * u_r.y + p_r * n_f.y,
                rho_r * u_n_r * u_r.z + p_r * n_f.z,
            );
            let f_ener_l = rho_l * u_n_l * h_l;
            let f_ener_r = rho_r * u_n_r * h_r;

            // KNP numerical fluxes
            let flux_cont = (a_r * f_cont_l - a_l * f_cont_r
                             + a_l * a_r * (w_rho_r  - w_rho_l))  / da;
            let flux_mom  = Vector3::new(
                (a_r * f_mom_l.x - a_l * f_mom_r.x + a_l * a_r * (w_rhou_r.x - w_rhou_l.x)) / da,
                (a_r * f_mom_l.y - a_l * f_mom_r.y + a_l * a_r * (w_rhou_r.y - w_rhou_l.y)) / da,
                (a_r * f_mom_l.z - a_l * f_mom_r.z + a_l * a_r * (w_rhou_r.z - w_rhou_l.z)) / da,
            );
            let flux_ener = (a_r * f_ener_l - a_l * f_ener_r
                             + a_l * a_r * (w_rhoe_r - w_rhoe_l)) / da;

            // Owner receives flux in, neighbour receives flux out (sign convention)
            d_rho[o]  -= flux_cont * area;
            d_rho[nb] += flux_cont * area;
            d_rhou[o]  = Vector3::new(
                d_rhou[o].x - flux_mom.x * area,
                d_rhou[o].y - flux_mom.y * area,
                d_rhou[o].z - flux_mom.z * area,
            );
            d_rhou[nb] = Vector3::new(
                d_rhou[nb].x + flux_mom.x * area,
                d_rhou[nb].y + flux_mom.y * area,
                d_rhou[nb].z + flux_mom.z * area,
            );
            d_rhoe[o]  -= flux_ener * area;
            d_rhoe[nb] += flux_ener * area;

            // Store mass flux for phi output
            self.phi.internal[f] = flux_cont * area;
        }

        // ── Explicit Euler update ─────────────────────────────────────────────
        let rho_data = self.rho.internal.as_mut_slice();
        let u_data   = self.u.internal.as_mut_slice();
        let e_data   = self.e.internal.as_mut_slice();
        for c in 0..n {
            let v     = mesh.cell_volumes[c];
            let inv_v = 1.0 / v;

            // Update conserved variables
            let rho_old_c = rho_data[c];
            let u_old_c   = u_data[c];
            let e_tot_old = e_data[c] + 0.5 * (u_old_c.x * u_old_c.x
                                               + u_old_c.y * u_old_c.y
                                               + u_old_c.z * u_old_c.z);

            let rho_new  = (rho_old_c + dt * inv_v * d_rho[c]).max(1e-6);
            let rhou_new = Vector3::new(
                rho_old_c * u_old_c.x + dt * inv_v * d_rhou[c].x,
                rho_old_c * u_old_c.y + dt * inv_v * d_rhou[c].y,
                rho_old_c * u_old_c.z + dt * inv_v * d_rhou[c].z,
            );
            let rhoe_new = (rho_old_c * e_tot_old + dt * inv_v * d_rhoe[c]).max(0.0);

            // Recover primitives
            rho_data[c] = rho_new;
            u_data[c]   = Vector3::new(
                rhou_new.x / rho_new,
                rhou_new.y / rho_new,
                rhou_new.z / rho_new,
            );
            let e_tot_new = rhoe_new / rho_new;
            let u_mag_sq  = u_data[c].x * u_data[c].x
                          + u_data[c].y * u_data[c].y
                          + u_data[c].z * u_data[c].z;
            e_data[c] = (e_tot_new - 0.5 * u_mag_sq).max(0.0);
        }

        // Update pressure: p = (γ−1)·ρ·e
        {
            let rho_sl = self.rho.internal.as_slice();
            let e_sl   = self.e.internal.as_slice();
            let p_sl   = self.p.internal.as_mut_slice();
            for c in 0..n {
                p_sl[c] = ((GAMMA - 1.0) * rho_sl[c] * e_sl[c]).max(0.0);
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
