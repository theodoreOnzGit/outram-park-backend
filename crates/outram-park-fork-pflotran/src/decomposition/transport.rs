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

/// A distributed 1-D implicit solute-transport stepper for **general (non-uniform)
/// flow**.
///
/// Holds, per owned cell, the retarded storage `(θ+ρ_bK_d)V` and the volumetric
/// flux `q` and dispersion coupling `d = D_face·θ_face·(A_f/d_f)` of its west and
/// east internal faces (`0` where there is no such face at a domain end). Each
/// [`step`](Self::step) advances the concentration one backward-Euler step via a
/// distributed BiCGStab solve. Build directly with [`new`](Self::new), for uniform
/// flow with [`uniform`](Self::uniform), or from a global flow field with
/// [`from_global_flow`](Self::from_global_flow).
pub struct DistributedTransport1D {
    decomp: Decomposition1D,
    storage_v: Vec<f64>,
    /// West/east face volumetric flux (m³/s, `+x`) per owned cell.
    west_q: Vec<f64>,
    east_q: Vec<f64>,
    /// West/east face dispersion coupling `D_face·θ_face·(A/d)` per owned cell.
    west_d: Vec<f64>,
    east_d: Vec<f64>,
    /// Boundary condition at the global left (`gi = 0`) / right (`gi = n_global-1`)
    /// domain end.
    left_end: EndCondition,
    right_end: EndCondition,
    dt: f64,
}

/// The condition on a global domain-end boundary face, matching the serial
/// `SoluteTransport` boundary assembly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EndCondition {
    /// No boundary flux and no concentration (a closed end).
    Closed,
    /// A boundary flux but no fixed concentration — the serial default: the flux
    /// is added to the diagonal (advective outflow / zero-concentration inflow).
    Flux(f64),
    /// A Dirichlet concentration: advection upwinded by the flux sign plus an
    /// always-on dispersive coupling to `concentration`.
    Dirichlet {
        /// Boundary-face volumetric flux (`+` = outflow, `-` = inflow), m³/s.
        flux: f64,
        /// Dispersive coupling `D_face·θ·(A_bnd/d_bnd)`.
        dispersion: f64,
        /// Boundary solute concentration `c_bc`.
        concentration: f64,
    },
}

impl DistributedTransport1D {
    /// Build from explicit per-owned-cell arrays (all length `decomp.local_len`).
    /// A domain-end cell must carry `0` for its missing-side flux and dispersion.
    pub fn new(
        decomp: Decomposition1D,
        storage_v: Vec<f64>,
        west_q: Vec<f64>,
        east_q: Vec<f64>,
        west_d: Vec<f64>,
        east_d: Vec<f64>,
        dt: f64,
    ) -> Self {
        DistributedTransport1D {
            decomp,
            storage_v,
            west_q,
            east_q,
            west_d,
            east_d,
            left_end: EndCondition::Closed,
            right_end: EndCondition::Closed,
            dt,
        }
    }

    /// Set the boundary conditions at the two global domain ends (default
    /// [`EndCondition::Closed`]). See [`EndCondition`].
    pub fn with_ends(mut self, left: EndCondition, right: EndCondition) -> Self {
        self.left_end = left;
        self.right_end = right;
        self
    }

    /// Convenience constructor for **uniform** flow: constant face flux `q`,
    /// constant dispersion coupling `d`, and constant per-cell storage.
    pub fn uniform(decomp: Decomposition1D, storage_cell: f64, q: f64, d: f64, dt: f64) -> Self {
        let l = decomp.local_len;
        let (mut west_q, mut east_q, mut west_d, mut east_d) =
            (vec![0.0; l], vec![0.0; l], vec![0.0; l], vec![0.0; l]);
        for i in 0..l {
            let gi = decomp.global_index(i);
            if gi > 0 {
                west_q[i] = q;
                west_d[i] = d;
            }
            if gi + 1 < decomp.n_global {
                east_q[i] = q;
                east_d[i] = d;
            }
        }
        Self::new(decomp, vec![storage_cell; l], west_q, east_q, west_d, east_d, dt)
    }

    /// Build this rank's stepper from a **global** uniform-grid flow field: the
    /// per-cell water content, the per-internal-face volumetric flux, the
    /// dispersion parameters, the uniform face area + geometric transmissibility,
    /// and the retarded storage constant `θ+ρ_bK_d → storage = value · V`.
    ///
    /// Computes each owned cell's west/east face flux and dispersion coupling by
    /// slicing the global arrays, matching the serial `SoluteTransport` assembly
    /// (`d = (D_mol + α_L·|q|/A)·θ_face·geom`, `θ_face` the face average).
    pub fn from_global_flow(
        decomp: &Decomposition1D,
        water_content: &[f64],
        face_flux: &[f64],
        molecular_diffusion: f64,
        longitudinal_dispersivity: f64,
        area: f64,
        geom: f64,
        storage_v: Vec<f64>,
        dt: f64,
    ) -> Self {
        let l = decomp.local_len;
        let (mut west_q, mut east_q, mut west_d, mut east_d) =
            (vec![0.0; l], vec![0.0; l], vec![0.0; l], vec![0.0; l]);
        let disp = |q: f64, theta_face: f64| -> f64 {
            let v = q.abs() / area;
            (molecular_diffusion + longitudinal_dispersivity * v) * theta_face * geom
        };
        for i in 0..l {
            let gi = decomp.global_index(i);
            if gi + 1 < decomp.n_global {
                let q = face_flux[gi]; // face gi connects gi and gi+1
                let theta_face = 0.5 * (water_content[gi] + water_content[gi + 1]);
                east_q[i] = q;
                east_d[i] = disp(q, theta_face);
            }
            if gi > 0 {
                let q = face_flux[gi - 1]; // face gi-1 connects gi-1 and gi
                let theta_face = 0.5 * (water_content[gi - 1] + water_content[gi]);
                west_q[i] = q;
                west_d[i] = disp(q, theta_face);
            }
        }
        Self::new(decomp.clone(), storage_v, west_q, east_q, west_d, east_d, dt)
    }

    /// Advance the concentration `c` (this rank's slab) one implicit timestep,
    /// returning the new slab. Assembles the distributed tridiagonal transport
    /// matrix and RHS per rank, then solves with distributed BiCGStab.
    ///
    /// The per-cell rows match the serial `SoluteTransport::step` upwind assembly
    /// for general face fluxes: for the east face `(gi,gi+1)` the diagonal gains
    /// the dispersion `d_e` plus the advective outflow `q_e` when `q_e ≥ 0` (else
    /// the east coupling gains `q_e`); symmetrically for the west face. Domain-end
    /// cells drop the missing-side face.
    ///
    /// # Errors
    /// Propagates any transport error from the distributed solve.
    pub fn step(&self, comm: &Communicator, c: &[f64], tol: f64, max_iter: usize) -> MpiResult<Vec<f64>> {
        let l = c.len();
        let inv_dt = 1.0 / self.dt;
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
                let (qe, de) = (self.east_q[i], self.east_d[i]);
                dg += de;
                east[i] = -de;
                if qe >= 0.0 {
                    dg += qe; // owner outflow carries this cell's concentration
                } else {
                    east[i] += qe; // inflow from the east carries neighbour's
                }
            }
            if has_west {
                let (qw, dw) = (self.west_q[i], self.west_d[i]);
                dg += dw;
                west[i] = -dw;
                if qw >= 0.0 {
                    west[i] -= qw; // inflow from the west carries neighbour's
                } else {
                    dg -= qw; // outflow to the west carries this cell's (q<0)
                }
            }
            // Boundary condition at a global domain end (matches the serial
            // SoluteTransport boundary-face assembly).
            let end = if gi == 0 {
                Some(self.left_end)
            } else if gi + 1 == self.decomp.n_global {
                Some(self.right_end)
            } else {
                None
            };
            match end {
                None | Some(EndCondition::Closed) => {}
                Some(EndCondition::Flux(q_b)) => dg += q_b,
                Some(EndCondition::Dirichlet {
                    flux,
                    dispersion,
                    concentration,
                }) => {
                    if flux < 0.0 {
                        b[i] -= flux * concentration; // inflow brings c_bc
                    } else {
                        dg += flux; // outflow carries interior concentration out
                    }
                    dg += dispersion;
                    b[i] += dispersion * concentration;
                }
            }

            diag[i] = dg;
            b[i] += acc * c[i];
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
    use crate::transport::{
        DispersionModel, FlowField, SoluteTransport, TransportBoundaryCondition,
        TransportBoundaryKind,
    };
    use crate::grid::{BoundaryLocation, CartesianGrid};
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
                let stepper = DistributedTransport1D::uniform(dd.clone(), storage_cell, q, d, dt);
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

    #[test]
    fn distributed_transport_matches_serial_for_nonuniform_flow() {
        // A spatially-varying face flux (accelerating +x flow) makes both the
        // advection and the velocity-dependent dispersion position-dependent.
        let n = 30;
        let theta = 0.35_f64;
        let dt = 0.4_f64;
        let nsteps = 5;
        let mol_diff = 0.03_f64;
        let alpha_l = 0.15_f64;

        let grid = CartesianGrid::uniform(n, 1, 1, m(1.0), m(1.0), m(1.0)).unwrap();
        let n_int = grid.connections().len();
        let n_bnd = grid.boundary_faces().len();
        let area = grid.connections()[0].area;
        let geom = grid.connections()[0].geometric_transmissibility;
        // Non-uniform face flux: increases along +x.
        let face_flux: Vec<f64> = (0..n_int).map(|f| 0.2 + 0.03 * f as f64).collect();
        let water_content = vec![theta; n as usize];

        let flow = FlowField {
            face_flux: face_flux.clone(),
            boundary_flux: vec![0.0; n_bnd],
            water_content: water_content.clone(),
        };
        let disp = DispersionModel::new(mol_diff, alpha_l).unwrap();
        let mut serial = SoluteTransport::new(grid, flow, disp, Vec::new()).unwrap();
        let c0: Vec<f64> = (0..n).map(|i| if i < 4 { 1.0 } else { 0.0 }).collect();
        let mut c_serial = c0.clone();
        for _ in 0..nsteps {
            serial.set_timestep(dt);
            serial.set_previous(&c_serial);
            serial.step(&mut c_serial).unwrap();
        }

        let storage_v_cell = theta * 1.0;
        for p in [1, 2, 3, 4, 6] {
            let c_serial = c_serial.clone();
            let c0 = c0.clone();
            let face_flux = face_flux.clone();
            let water_content = water_content.clone();
            let ok = run(p, move |comm| {
                let dd = Decomposition1D::new(n, comm);
                let storage_v = vec![storage_v_cell; dd.local_len];
                let stepper = DistributedTransport1D::from_global_flow(
                    &dd,
                    &water_content,
                    &face_flux,
                    mol_diff,
                    alpha_l,
                    area,
                    geom,
                    storage_v,
                    dt,
                );
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
                "non-uniform distributed transport != serial for p={p}"
            );
        }
    }

    #[test]
    fn distributed_transport_matches_serial_with_dirichlet_inflow() {
        // Open domain: uniform +x flow with a fixed inflow concentration at XMin
        // and advective outflow at XMax -- the realistic transport scenario.
        let n = 28;
        let q = 0.6_f64;
        let theta = 0.3_f64;
        let dt = 0.5_f64;
        let nsteps = 6;
        let mol = 0.02_f64;
        let alpha = 0.1_f64;
        let c_in = 1.0_f64;

        let grid = CartesianGrid::uniform(n, 1, 1, m(1.0), m(1.0), m(1.0)).unwrap();
        let n_int = grid.connections().len();
        let area = grid.connections()[0].area;
        let geom_int = grid.connections()[0].geometric_transmissibility;
        let mut boundary_flux = vec![0.0; grid.boundary_faces().len()];
        let (mut xmin_area, mut xmin_dist) = (0.0_f64, 0.0_f64);
        for (b, face) in grid.boundary_faces().iter().enumerate() {
            match face.location {
                BoundaryLocation::XMin => {
                    boundary_flux[b] = -q; // inflow
                    xmin_area = face.area;
                    xmin_dist = face.distance;
                }
                BoundaryLocation::XMax => boundary_flux[b] = q, // outflow
                _ => {}
            }
        }
        let flow = FlowField {
            face_flux: vec![q; n_int],
            boundary_flux,
            water_content: vec![theta; n as usize],
        };
        let disp = DispersionModel::new(mol, alpha).unwrap();
        let bcs = vec![TransportBoundaryCondition {
            location: BoundaryLocation::XMin,
            kind: TransportBoundaryKind::InflowConcentration(c_in),
        }];
        let mut serial = SoluteTransport::new(grid, flow, disp, bcs).unwrap();
        let c0 = vec![0.0_f64; n as usize];
        let mut c_serial = c0.clone();
        for _ in 0..nsteps {
            serial.set_timestep(dt);
            serial.set_previous(&c_serial);
            serial.step(&mut c_serial).unwrap();
        }

        // XMin Dirichlet dispersive coupling d_b = (D_mol + alpha*|q|/A_bnd)*theta*(A_bnd/d_bnd).
        let geom_bc = xmin_area / xmin_dist;
        let d_face_bc = mol + alpha * (q.abs() / xmin_area);
        let d_b_left = d_face_bc * theta * geom_bc;
        let storage_cell = theta * 1.0;
        for p in [1, 2, 3, 4] {
            let c_serial = c_serial.clone();
            let c0 = c0.clone();
            let ok = run(p, move |comm| {
                let dd = Decomposition1D::new(n, comm);
                let storage_v = vec![storage_cell; dd.local_len];
                let stepper = DistributedTransport1D::from_global_flow(
                    &dd,
                    &vec![theta; n as usize],
                    &vec![q; n_int],
                    mol,
                    alpha,
                    area,
                    geom_int,
                    storage_v,
                    dt,
                )
                .with_ends(
                    EndCondition::Dirichlet {
                        flux: -q,
                        dispersion: d_b_left,
                        concentration: c_in,
                    },
                    EndCondition::Flux(q), // XMax: advective outflow, no fixed concentration
                );
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
                "Dirichlet-inflow distributed transport != serial for p={p}"
            );
        }
    }
}
