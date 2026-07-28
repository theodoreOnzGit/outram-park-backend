//! Distributed solve of a **real assembled `LduMatrix`** (bead op-gj5).
//!
//! The previous op-gj5 slices ([`super::operator`]) drive the distributed CG from
//! a re-implemented stencil. This module closes the loop to pflotran's *actual*
//! linear-algebra type: it assembles the genuine
//! [`outram_foam_basic_lib::ldu_matrix::LduMatrix`] — the face-addressed sparse
//! matrix the RICHARDS/transport/energy solvers build and hand to the serial
//! Krylov backend — and then solves it **distributed** across MPI ranks.
//!
//! [`assemble_diffusion_ldu`] performs a real face-based diffusion assembly
//! (per-connection geometric transmissibility × harmonic-mean conductivity, plus a
//! diagonal Helmholtz shift for SPD-ness) into an `LduMatrix`.
//! [`DistributedLduMatrix1D`] extracts this rank's rows from that matrix over a
//! 1-D cell partition and provides a halo-exchanged distributed matrix-vector
//! product; [`DistributedLduMatrix1D::solve`] drives
//! [`super::krylov::distributed_cg_with`]. The module tests check the distributed
//! SpMV against [`LduMatrix::multiply`] (the real serial product) cell-for-cell,
//! and the distributed solution against a serial CG on the same matrix.
//!
//! # Scope / human-review flags
//!
//! Verification-only, untrusted AI draft. The distributed matrix is currently
//! *extracted* from a globally-assembled `LduMatrix` (each rank sees the whole
//! assembly) — a demo simplification; assembling only the local rows per rank, and
//! using this inside pflotran's real Newton loop on the non-symmetric transport
//! Jacobian (which needs a distributed BiCGStab, not CG), remain the op-gj5
//! follow-ups. 1-D structured partition only.

use outram_foam_basic_lib::ldu_matrix::LduMatrix;

use super::krylov::{distributed_bicgstab_with, distributed_cg_with};
use super::Decomposition1D;
use crate::grid::CartesianGrid;
use outram_park_mpi::{Communicator, MpiResult};

/// Harmonic mean of two conductivities (series face transmissibility factor).
fn harmonic(a: f64, b: f64) -> f64 {
    if a + b > 0.0 {
        2.0 * a * b / (a + b)
    } else {
        0.0
    }
}

/// Assemble the real face-addressed diffusion matrix for `grid` with per-cell
/// conductivity `k` and a diagonal Helmholtz `shift` (`> 0` ⇒ SPD).
///
/// This is a genuine pflotran-style assembly into
/// [`outram_foam_basic_lib::ldu_matrix::LduMatrix`]: each internal connection `f`
/// contributes a symmetric transmissibility `T_f = A_f/d_f · harmonic(k_o, k_n)` to
/// the diagonal of both cells and to `upper[f]`/`lower[f]`. Face index `f` matches
/// `grid.connections()[f]` by construction.
///
/// # Panics
/// Debug-asserts `k.len() == grid.n_cells()`.
pub fn assemble_diffusion_ldu(grid: &CartesianGrid, k: &[f64], shift: f64) -> LduMatrix {
    debug_assert_eq!(k.len(), grid.n_cells());
    let owner: Vec<usize> = grid.connections().iter().map(|c| c.owner).collect();
    let neighbour: Vec<usize> = grid.connections().iter().map(|c| c.neighbour).collect();
    let mut a = LduMatrix::new(grid.n_cells(), owner, neighbour);
    for d in a.diag.iter_mut() {
        *d += shift;
    }
    for (f, conn) in grid.connections().iter().enumerate() {
        let (o, n) = (conn.owner, conn.neighbour);
        let t = conn.geometric_transmissibility * harmonic(k[o], k[n]);
        a.diag[o] += t;
        a.diag[n] += t;
        a.upper[f] -= t;
        a.lower[f] -= t;
    }
    a
}

/// Assemble a **non-symmetric** advection–diffusion `LduMatrix`: the diffusion
/// assembly of [`assemble_diffusion_ldu`] plus an upwind advection term for a
/// constant `velocity` (m/s, `+x`).
///
/// Upwinding makes `upper[f] ≠ lower[f]` (the matrix is non-symmetric), so the
/// resulting system must be solved with BiCGStab, not CG — this is the shape of
/// the real transport Jacobian. The upwind stencil mirrors the `transport`
/// module's assembly.
pub fn assemble_advection_diffusion_ldu(
    grid: &CartesianGrid,
    k: &[f64],
    velocity: f64,
    shift: f64,
) -> LduMatrix {
    let mut a = assemble_diffusion_ldu(grid, k, shift);
    for (f, conn) in grid.connections().iter().enumerate() {
        let q = velocity * conn.area; // volumetric flux owner -> neighbour (+x)
        if q >= 0.0 {
            a.diag[conn.owner] += q;
            a.lower[f] -= q;
        } else {
            a.upper[f] += q;
            a.diag[conn.neighbour] -= q;
        }
    }
    a
}

/// A row-distributed view of a real [`LduMatrix`] over a 1-D cell partition.
///
/// Holds this rank's owned rows as a diagonal plus the two tridiagonal
/// off-diagonals (`west` = coupling to cell `c-1`, `east` = coupling to `c+1`),
/// extracted from the assembled matrix. Cross-rank couplings at the slab edges are
/// resolved by a halo exchange during the matvec.
#[derive(Clone)]
pub struct DistributedLduMatrix1D {
    decomp: Decomposition1D,
    diag: Vec<f64>,
    west: Vec<f64>,
    east: Vec<f64>,
}

impl DistributedLduMatrix1D {
    /// Build directly from this rank's tridiagonal rows: `diag[i]` = `A_ii`,
    /// `west[i]` = coupling to cell `i-1` (`A_{i,i-1}`), `east[i]` = coupling to
    /// cell `i+1` (`A_{i,i+1}`), all length `decomp.local_len`. Off-domain
    /// couplings must be `0`. Used by higher-level distributed assemblers (e.g. the
    /// transport driver).
    ///
    /// # Errors
    /// [`crate::error::PflotranError::InvalidInput`] if any array length differs
    /// from the local cell count.
    pub fn from_rows(
        decomp: &Decomposition1D,
        diag: Vec<f64>,
        west: Vec<f64>,
        east: Vec<f64>,
    ) -> Result<Self, crate::error::PflotranError> {
        let l = decomp.local_len;
        if diag.len() != l || west.len() != l || east.len() != l {
            return Err(crate::error::PflotranError::InvalidInput(format!(
                "from_rows: diag/west/east must all be length {l}"
            )));
        }
        Ok(DistributedLduMatrix1D {
            decomp: decomp.clone(),
            diag,
            west,
            east,
        })
    }

    /// Extract this rank's rows (per `decomp`) from a globally-assembled 1-D
    /// `LduMatrix`. The matrix must be tridiagonal in the natural 1-D ordering
    /// (`neighbour = owner + 1`), as produced by [`assemble_diffusion_ldu`] on a
    /// 1-D grid.
    pub fn from_global(decomp: &Decomposition1D, ldu: &LduMatrix) -> Self {
        let n = ldu.diag.len();
        let mut west = vec![0.0; n];
        let mut east = vec![0.0; n];
        // y[o] += upper[f]·x[n]  (east coupling of cell o);
        // y[n] += lower[f]·x[o]  (west coupling of cell n).
        for f in 0..ldu.upper.len() {
            east[ldu.owner[f]] = ldu.upper[f];
            west[ldu.neighbour[f]] = ldu.lower[f];
        }
        let (s, l) = (decomp.start, decomp.local_len);
        DistributedLduMatrix1D {
            decomp: decomp.clone(),
            diag: ldu.diag[s..s + l].to_vec(),
            west: west[s..s + l].to_vec(),
            east: east[s..s + l].to_vec(),
        }
    }

    /// Assemble this rank's rows **locally** — without ever forming the global
    /// matrix — for the 1-D diffusion operator with per-cell conductivity
    /// `k_local`, a uniform face geometric transmissibility `geom` (`= A_f/d_f`,
    /// constant on a uniform grid), and diagonal Helmholtz `shift`.
    ///
    /// Each rank halo-exchanges its neighbours' edge conductivities once, then
    /// builds its `(diag, west, east)` rows from local + ghost `k` — the genuinely
    /// distributed assembly (no rank sees the whole matrix). It produces the
    /// *identical* distributed matrix as [`from_global`](Self::from_global) applied
    /// to [`assemble_diffusion_ldu`] (checked in the module tests).
    ///
    /// # Errors
    /// Propagates any halo-exchange transport error.
    pub fn assemble_diffusion_local(
        comm: &Communicator,
        decomp: &Decomposition1D,
        k_local: &[f64],
        geom: f64,
        shift: f64,
    ) -> MpiResult<Self> {
        let kh = super::exchange_halo(comm, decomp, k_local)?;
        let l = k_local.len();
        let mut diag = vec![0.0; l];
        let mut west = vec![0.0; l];
        let mut east = vec![0.0; l];
        for i in 0..l {
            let gi = decomp.global_index(i);
            // West face exists iff this is not the global-left cell.
            if gi > 0 {
                let k_w = if i > 0 { k_local[i - 1] } else { kh.left.unwrap_or(0.0) };
                let t = geom * harmonic(k_local[i], k_w);
                west[i] = -t;
                diag[i] += t;
            }
            // East face exists iff this is not the global-right cell.
            if gi + 1 < decomp.n_global {
                let k_e = if i + 1 < l { k_local[i + 1] } else { kh.right.unwrap_or(0.0) };
                let t = geom * harmonic(k_local[i], k_e);
                east[i] = -t;
                diag[i] += t;
            }
            diag[i] += shift;
        }
        Ok(DistributedLduMatrix1D {
            decomp: decomp.clone(),
            diag,
            west,
            east,
        })
    }

    /// Distributed matrix-vector product `A x` on this rank's slab, exchanging the
    /// halo for the cross-rank tridiagonal couplings.
    ///
    /// # Errors
    /// Propagates any halo-exchange transport error.
    pub fn matvec(&self, comm: &Communicator, x: &[f64]) -> MpiResult<Vec<f64>> {
        let halo = super::exchange_halo(comm, &self.decomp, x)?;
        let l = x.len();
        let mut y = vec![0.0; l];
        for i in 0..l {
            let xw = if i > 0 {
                x[i - 1]
            } else {
                halo.left.unwrap_or(0.0)
            };
            let xe = if i + 1 < l {
                x[i + 1]
            } else {
                halo.right.unwrap_or(0.0)
            };
            y[i] = self.diag[i] * x[i] + self.west[i] * xw + self.east[i] * xe;
        }
        Ok(y)
    }

    /// Solve `A u = b` for the assembled matrix by distributed conjugate gradient.
    ///
    /// # Errors
    /// Propagates any transport error from the matvec or reduced dot products.
    pub fn solve(
        &self,
        comm: &Communicator,
        b: &[f64],
        tol: f64,
        max_iter: usize,
    ) -> MpiResult<(Vec<f64>, usize)> {
        distributed_cg_with(comm, b, |v| self.matvec(comm, v), tol, max_iter)
    }

    /// Solve `A u = b` by distributed **BiCGStab** — for a **non-symmetric**
    /// assembled matrix (e.g. advection–diffusion) where CG does not apply.
    ///
    /// # Errors
    /// Propagates any transport error from the matvec or reduced dot products.
    pub fn solve_bicgstab(
        &self,
        comm: &Communicator,
        b: &[f64],
        tol: f64,
        max_iter: usize,
    ) -> MpiResult<(Vec<f64>, usize)> {
        distributed_bicgstab_with(comm, b, |v| self.matvec(comm, v), tol, max_iter)
    }
}

/// Serial CG on a real [`LduMatrix`] using its own [`LduMatrix::multiply`] — the
/// correctness oracle for the distributed solve.
pub fn serial_ldu_cg(ldu: &LduMatrix, b: &[f64], tol: f64, max_iter: usize) -> (Vec<f64>, usize) {
    let n = b.len();
    let dot = |a: &[f64], c: &[f64]| -> f64 { a.iter().zip(c).map(|(x, y)| x * y).sum() };
    let mut x = vec![0.0; n];
    let mut r = b.to_vec();
    let mut p = r.clone();
    let mut rs_old = dot(&r, &r);
    let mut iters = 0;
    if rs_old.sqrt() < tol {
        return (x, 0);
    }
    for it in 0..max_iter {
        iters = it + 1;
        let ap = ldu.multiply(&p);
        let alpha = rs_old / dot(&p, &ap);
        for i in 0..n {
            x[i] += alpha * p[i];
            r[i] -= alpha * ap[i];
        }
        let rs_new = dot(&r, &r);
        if rs_new.sqrt() < tol {
            break;
        }
        let beta = rs_new / rs_old;
        for i in 0..n {
            p[i] = r[i] + beta * p[i];
        }
        rs_old = rs_new;
    }
    (x, iters)
}

/// Serial BiCGStab on a real [`LduMatrix`] via [`LduMatrix::multiply`] — the
/// oracle for the distributed non-symmetric solve.
pub fn serial_ldu_bicgstab(
    ldu: &LduMatrix,
    b: &[f64],
    tol: f64,
    max_iter: usize,
) -> (Vec<f64>, usize) {
    let n = b.len();
    let dot = |a: &[f64], c: &[f64]| -> f64 { a.iter().zip(c).map(|(x, y)| x * y).sum() };
    let mut x = vec![0.0; n];
    let mut r = b.to_vec();
    let r_hat = r.clone();
    let (mut rho, mut alpha, mut omega) = (1.0_f64, 1.0_f64, 1.0_f64);
    let mut v = vec![0.0; n];
    let mut p = vec![0.0; n];
    let b_norm = dot(b, b).sqrt().max(1.0);
    let mut iters = 0;
    for k in 0..max_iter {
        iters = k + 1;
        let rho_new = dot(&r_hat, &r);
        if rho_new.abs() < 1e-30 {
            break;
        }
        let beta = (rho_new / rho) * (alpha / omega);
        for i in 0..n {
            p[i] = r[i] + beta * (p[i] - omega * v[i]);
        }
        v = ldu.multiply(&p);
        alpha = rho_new / dot(&r_hat, &v);
        let s: Vec<f64> = (0..n).map(|i| r[i] - alpha * v[i]).collect();
        if dot(&s, &s).sqrt() / b_norm < tol {
            for i in 0..n {
                x[i] += alpha * p[i];
            }
            break;
        }
        let t = ldu.multiply(&s);
        let tt = dot(&t, &t);
        if tt < 1e-30 {
            break;
        }
        omega = dot(&t, &s) / tt;
        for i in 0..n {
            x[i] += alpha * p[i] + omega * s[i];
            r[i] = s[i] - omega * t[i];
        }
        if dot(&r, &r).sqrt() / b_norm < tol {
            break;
        }
        rho = rho_new;
    }
    (x, iters)
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_park_mpi::run;
    use uom::si::f64::Length;
    use uom::si::length::meter;

    fn m(v: f64) -> Length {
        Length::new::<meter>(v)
    }

    fn one_d_grid(n: usize) -> CartesianGrid {
        CartesianGrid::uniform(n, 1, 1, m(1.0), m(1.0), m(1.0)).unwrap()
    }

    fn conductivity(n: usize) -> Vec<f64> {
        (0..n)
            .map(|i| match i % 4 {
                0 => 1.0,
                1 => 8.0,
                2 => 0.2,
                _ => 2.5,
            })
            .collect()
    }

    #[test]
    fn distributed_spmv_matches_real_ldu_multiply() {
        // The distributed matvec of the assembled LduMatrix must equal the crate's
        // own LduMatrix::multiply, cell-for-cell, on this rank's slab.
        let n = 24;
        let shift = 0.3_f64;
        let k = conductivity(n);
        let grid = one_d_grid(n);
        let ldu = assemble_diffusion_ldu(&grid, &k, shift);
        let x: Vec<f64> = (0..n).map(|i| (i as f64 * 0.4).sin() + 1.0).collect();
        let serial_y = ldu.multiply(&x);

        for p in [1, 2, 3, 4] {
            let ldu = ldu.clone();
            let x = x.clone();
            let serial_y = serial_y.clone();
            let ok = run(p, move |c| {
                let d = Decomposition1D::new(n, c);
                let dm = DistributedLduMatrix1D::from_global(&d, &ldu);
                let x_local = x[d.start..d.start + d.local_len].to_vec();
                let y_local = dm.matvec(c, &x_local).unwrap();
                let expected = &serial_y[d.start..d.start + d.local_len];
                y_local.iter().zip(expected).all(|(a, e)| (a - e).abs() < 1e-12)
            })
            .unwrap();
            assert!(ok.iter().all(|&b| b), "distributed SpMV != LduMatrix::multiply for p={p}");
        }
    }

    #[test]
    fn distributed_solve_matches_serial_ldu_cg() {
        let n = 32;
        let shift = 0.2_f64;
        let tol = 1e-11;
        let k = conductivity(n);
        let grid = one_d_grid(n);
        let ldu = assemble_diffusion_ldu(&grid, &k, shift);
        let b: Vec<f64> = (0..n).map(|i| ((i % 6) as f64) - 2.0).collect();
        let (reference, _) = serial_ldu_cg(&ldu, &b, tol, 2000);

        for p in [1, 2, 3, 4, 8] {
            let ldu = ldu.clone();
            let b = b.clone();
            let reference = reference.clone();
            let ok = run(p, move |c| {
                let d = Decomposition1D::new(n, c);
                let dm = DistributedLduMatrix1D::from_global(&d, &ldu);
                let b_local = b[d.start..d.start + d.local_len].to_vec();
                let (x, _) = dm.solve(c, &b_local, tol, 2000).unwrap();
                let expected = &reference[d.start..d.start + d.local_len];
                x.iter().zip(expected).all(|(a, e)| (a - e).abs() < 1e-7)
            })
            .unwrap();
            assert!(ok.iter().all(|&b| b), "distributed LDU solve != serial for p={p}");
        }
    }

    #[test]
    fn distributed_bicgstab_solves_nonsymmetric_advection_diffusion() {
        // Advection makes the matrix non-symmetric (upper != lower); CG would fail,
        // BiCGStab must match the serial BiCGStab across rank counts.
        let n = 32;
        let k = conductivity(n);
        let grid = one_d_grid(n);
        let ldu = assemble_advection_diffusion_ldu(&grid, &k, 2.0, 0.3);
        // Sanity: the matrix really is non-symmetric.
        assert!(
            ldu.upper.iter().zip(&ldu.lower).any(|(u, l)| (u - l).abs() > 1e-9),
            "advection-diffusion matrix should be non-symmetric"
        );
        let b: Vec<f64> = (0..n).map(|i| ((i % 5) as f64) + 0.5).collect();
        let tol = 1e-10;
        let (reference, _) = serial_ldu_bicgstab(&ldu, &b, tol, 4000);

        for p in [1, 2, 3, 4, 8] {
            let ldu = ldu.clone();
            let b = b.clone();
            let reference = reference.clone();
            let ok = run(p, move |c| {
                let d = Decomposition1D::new(n, c);
                let dm = DistributedLduMatrix1D::from_global(&d, &ldu);
                let b_local = b[d.start..d.start + d.local_len].to_vec();
                let (x, _) = dm.solve_bicgstab(c, &b_local, tol, 4000).unwrap();
                let expected = &reference[d.start..d.start + d.local_len];
                // Also check the local residual is small.
                let ax = dm.matvec(c, &x).unwrap();
                let res_ok = b_local
                    .iter()
                    .zip(&ax)
                    .all(|(bi, ai)| (bi - ai).abs() < 1e-6);
                let match_ok = x.iter().zip(expected).all(|(a, e)| (a - e).abs() < 1e-6);
                res_ok && match_ok
            })
            .unwrap();
            assert!(ok.iter().all(|&b| b), "distributed BiCGStab != serial for p={p}");
        }
    }

    #[test]
    fn local_assembly_equals_global_extract_and_solves() {
        // Per-rank local assembly must produce the SAME distributed matrix as
        // extracting from the globally-assembled LduMatrix, and solve identically.
        let n = 30;
        let shift = 0.25_f64;
        let tol = 1e-11;
        let k = conductivity(n);
        let grid = one_d_grid(n); // unit cells -> geometric transmissibility = 1.0
        let geom = grid.connections()[0].geometric_transmissibility;
        assert!((geom - 1.0).abs() < 1e-12);
        let ldu = assemble_diffusion_ldu(&grid, &k, shift);
        let b: Vec<f64> = (0..n).map(|i| ((i % 4) as f64) + 1.0).collect();
        let (reference, _) = serial_ldu_cg(&ldu, &b, tol, 2000);

        for p in [1, 2, 3, 5] {
            let ldu = ldu.clone();
            let k = k.clone();
            let b = b.clone();
            let reference = reference.clone();
            let ok = run(p, move |c| {
                let d = Decomposition1D::new(n, c);
                let from_global = DistributedLduMatrix1D::from_global(&d, &ldu);
                let k_local = k[d.start..d.start + d.local_len].to_vec();
                let local =
                    DistributedLduMatrix1D::assemble_diffusion_local(c, &d, &k_local, geom, shift)
                        .unwrap();
                // Identical matrix rows (diag/west/east) — no global matrix needed.
                let same_matrix = local
                    .diag
                    .iter()
                    .zip(&from_global.diag)
                    .all(|(a, b)| (a - b).abs() < 1e-12)
                    && local
                        .west
                        .iter()
                        .zip(&from_global.west)
                        .all(|(a, b)| (a - b).abs() < 1e-12)
                    && local
                        .east
                        .iter()
                        .zip(&from_global.east)
                        .all(|(a, b)| (a - b).abs() < 1e-12);
                // And solving with the locally-assembled matrix matches serial.
                let b_local = b[d.start..d.start + d.local_len].to_vec();
                let (x, _) = local.solve(c, &b_local, tol, 2000).unwrap();
                let expected = &reference[d.start..d.start + d.local_len];
                let solve_ok = x.iter().zip(expected).all(|(a, e)| (a - e).abs() < 1e-7);
                same_matrix && solve_ok
            })
            .unwrap();
            assert!(ok.iter().all(|&b| b), "local assembly != global / wrong solve for p={p}");
        }
    }

    #[test]
    fn distributed_solve_residual_is_near_zero() {
        let n = 20;
        let k = conductivity(n);
        let grid = one_d_grid(n);
        let ldu = assemble_diffusion_ldu(&grid, &k, 0.5);
        let b: Vec<f64> = (0..n).map(|i| (i as f64) * 0.1).collect();
        // Global residual ||b - A x|| via the distributed solve (p=4), checked with
        // the real serial multiply.
        let sol = run(4, {
            let ldu = ldu.clone();
            let b = b.clone();
            move |c| {
                let d = Decomposition1D::new(n, c);
                let dm = DistributedLduMatrix1D::from_global(&d, &ldu);
                let b_local = b[d.start..d.start + d.local_len].to_vec();
                let (x, _) = dm.solve(c, &b_local, 1e-12, 2000).unwrap();
                // Return the residual 2-norm contribution via a reduced dot.
                let ax = dm.matvec(c, &x).unwrap();
                let diff: Vec<f64> = b_local.iter().zip(&ax).map(|(bi, ai)| bi - ai).collect();
                distributed_dot(c, &diff, &diff).unwrap().sqrt()
            }
        })
        .unwrap();
        assert!(sol[0] < 1e-8, "distributed LDU residual too large: {}", sol[0]);
    }
}
