//! Distributed solute-transport timestep (bead op-gj5).
//!
//! This is the orchestration slice: a **working distributed implicit transport
//! timestep** that reproduces the serial [`crate::transport::SoluteTransport`]
//! step, but with the linear system assembled per-rank and solved with the
//! distributed BiCGStab ([`super::ldu`]) over an MPI decomposition.
//!
//! Each backward-Euler step assembles the same operator the serial module does for
//! a uniform 1-D flow — accumulation `(θ+ρ_bK_d)V/Δt`, upwind advection, and
//! symmetric dispersion — as a distributed tridiagonal matrix, forms the RHS
//! `acc·cₒₗₐ`, and solves `A c = b` with distributed BiCGStab (the operator is
//! non-symmetric because of upwinding). The module test runs several timesteps
//! and checks the distributed concentration field against the real serial
//! `SoluteTransport` cell-for-cell.
//!
//! # Scope / human-review flags
//!
//! Verification-only, untrusted AI draft. Uniform 1-D flow (constant face flux,
//! water content, dispersion), Upwind advection (no deferred-correction TVD),
//! closed ends (zero boundary flux, no Dirichlet BC). It demonstrates the
//! distributed solver as the linear stage of a real transport timestep;
//! non-uniform flow, TVD, Dirichlet boundaries, and the RICHARDS/energy timesteps
//! are the remaining op-gj5 follow-ups.

use outram_park_mpi::{Communicator, MpiResult};

use super::ldu::DistributedLduMatrix1D;
use super::Decomposition1D;

/// A distributed 1-D implicit solute-transport stepper for uniform flow.
///
/// Holds the per-cell storage `(θ+ρ_bK_d)V`, the constant internal face flux `q`
/// (m³/s, `+x`), the constant per-face dispersion coupling `d = D_face·θ·(A/d)`,
/// and the timestep. Each [`step`](Self::step) advances the concentration one
/// backward-Euler step via a distributed BiCGStab solve.
pub struct DistributedTransport1D {
    decomp: Decomposition1D,
    /// Retarded storage `(θ_w + ρ_b K_d)·V` per owned cell.
    storage_v: Vec<f64>,
    /// Constant internal face volumetric flux (m³/s, `+x`).
    q: f64,
    /// Constant per-face dispersion coupling `D_face·θ_face·(A_f/d_f)`.
    d: f64,
    /// Timestep (s).
    dt: f64,
}

impl DistributedTransport1D {
    /// Build the stepper. `storage_v` has one entry per owned cell.
    pub fn new(decomp: Decomposition1D, storage_v: Vec<f64>, q: f64, d: f64, dt: f64) -> Self {
        DistributedTransport1D {
            decomp,
            storage_v,
            q,
            d,
            dt,
        }
    }

    /// Advance the concentration `c` (this rank's slab) one implicit timestep,
    /// returning the new slab. Assembles the distributed tridiagonal transport
    /// matrix and RHS locally, then solves with distributed BiCGStab.
    ///
    /// The per-cell rows match the serial `SoluteTransport::step` assembly for
    /// uniform `+x` flow with `q > 0`: an east face contributes the advective
    /// outflow `q` to the diagonal and `-d` to the east coupling; a west face
    /// contributes `d` to the diagonal and `-(q+d)` to the west coupling
    /// (upwind advection + symmetric dispersion). Domain-end cells drop the
    /// missing-side face.
    ///
    /// # Errors
    /// Propagates any transport error from the distributed solve.
    pub fn step(&self, comm: &Communicator, c: &[f64], tol: f64, max_iter: usize) -> MpiResult<Vec<f64>> {
        let l = c.len();
        let inv_dt = 1.0 / self.dt;
        let q = self.q;
        let d = self.d;
        let mut diag = vec![0.0; l];
        let mut west = vec![0.0; l];
        let mut east = vec![0.0; l];
        let mut b = vec![0.0; l];
        for i in 0..l {
            let gi = self.decomp.global_index(i);
            let has_west = gi > 0;
            let has_east = gi + 1 < self.decomp.n_global;
            let acc = self.storage_v[i] * inv_dt;
            let mut dg = acc;
            if has_east {
                // East internal face (gi, gi+1): advective outflow + dispersion.
                dg += q + d;
                east[i] = -d;
            }
            if has_west {
                // West internal face (gi-1, gi): dispersion into gi; the advective
                // outflow was charged to gi-1, so gi only gets the dispersion here.
                dg += d;
                west[i] = -(q + d);
            }
            diag[i] = dg;
            b[i] = acc * c[i];
        }
        let matrix = DistributedLduMatrix1D::from_rows(&self.decomp, diag, west, east)
            .map_err(|e| outram_park_mpi::MpiError::Transport(format!("assembly: {e}")))?;
        let (x, _iters) = matrix.solve_bicgstab(comm, &b, tol, max_iter)?;
        Ok(x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::{DispersionModel, FlowField, SoluteTransport};
    use crate::grid::CartesianGrid;
    use outram_park_mpi::run;
    use uom::si::f64::Length;
    use uom::si::length::meter;

    fn m(v: f64) -> Length {
        Length::new::<meter>(v)
    }

    #[test]
    fn distributed_transport_step_matches_serial_solute_transport() {
        let n = 24;
        let q = 0.5_f64; // constant +x volumetric flux per internal face
        let theta = 0.3_f64; // uniform water content
        let dt = 0.5_f64;
        let nsteps = 6;
        let mol_diff = 0.02_f64;
        let alpha_l = 0.1_f64;

        // --- Serial reference: the real SoluteTransport module. ---
        let grid = CartesianGrid::uniform(n, 1, 1, m(1.0), m(1.0), m(1.0)).unwrap();
        let n_int = grid.connections().len();
        let n_bnd = grid.boundary_faces().len();
        let area = grid.connections()[0].area;
        let geom = grid.connections()[0].geometric_transmissibility;
        let flow = FlowField {
            face_flux: vec![q; n_int],
            boundary_flux: vec![0.0; n_bnd], // closed ends
            water_content: vec![theta; n as usize],
        };
        let disp = DispersionModel::new(mol_diff, alpha_l).unwrap();
        // Per-face dispersion coupling d = D_face * theta_face * geom, matching the
        // serial assembly (uniform flow -> constant).
        let v_darcy = q.abs() / area;
        // D_face = molecular_diffusion + alpha_L * |v_darcy| (public fields).
        let d_face = disp.molecular_diffusion + disp.longitudinal_dispersivity * v_darcy;
        let d = d_face * theta * geom;

        let mut serial = SoluteTransport::new(grid, flow, disp, Vec::new()).unwrap();
        let mut c_serial: Vec<f64> = (0..n).map(|i| if i < 3 { 1.0 } else { 0.0 }).collect();
        for _ in 0..nsteps {
            serial.set_timestep(dt);
            serial.set_previous(&c_serial);
            serial.step(&mut c_serial).unwrap();
        }

        // --- Distributed: same operator, per-rank BiCGStab. ---
        let storage_cell = theta * 1.0; // (theta + 0) * V, V = 1
        let c0: Vec<f64> = (0..n).map(|i| if i < 3 { 1.0 } else { 0.0 }).collect();
        for p in [1, 2, 3, 4] {
            let c_serial = c_serial.clone();
            let c0 = c0.clone();
            let ok = run(p, move |comm| {
                let dd = Decomposition1D::new(n, comm);
                let storage_v = vec![storage_cell; dd.local_len];
                let stepper = DistributedTransport1D::new(dd.clone(), storage_v, q, d, dt);
                let mut c: Vec<f64> = c0[dd.start..dd.start + dd.local_len].to_vec();
                for _ in 0..nsteps {
                    c = stepper.step(comm, &c, 1e-12, 5000).unwrap();
                }
                let expected = &c_serial[dd.start..dd.start + dd.local_len];
                c.iter().zip(expected).all(|(a, e)| (a - e).abs() < 1e-6)
            })
            .unwrap();
            assert!(
                ok.iter().all(|&b| b),
                "distributed transport != serial SoluteTransport for p={p}"
            );
        }
    }
}
