//! A distributed **variable-coefficient** diffusion operator (bead op-gj5).
//!
//! Where [`super::krylov`] demonstrates the distributed solve on a constant-
//! coefficient toy (the shifted Poisson stencil), this module drives the *same*
//! generic distributed CG ([`super::krylov::distributed_cg_with`]) with a **real
//! heterogeneous operator**: 1-D steady diffusion with a spatially-varying
//! conductivity `k(x)` and harmonic-mean face transmissibilities —
//!
//! `A u_i = shift·u_i + T^-_i (u_i − u_{i-1}) + T^+_i (u_i − u_{i+1})`,
//! `T = 2 k_i k_j / (k_i + k_j)` on an interior face, homogeneous Dirichlet (a
//! zero ghost with `T = k_i`) at a physical domain end.
//!
//! This is the operator a constant-density steady Darcy / heat-conduction problem
//! produces on a 1-D grid with heterogeneous permeability — the shape of a real
//! pflotran Jacobian block, not a toy. The point is that the distributed solve
//! reproduces the serial one **for a non-trivial, position-dependent matrix**: the
//! per-rank ghost *coefficients* (not just the field) are halo-exchanged once at
//! construction, and the field is exchanged each matvec.
//!
//! # Scope / human-review flags
//!
//! Verification-only, untrusted AI draft. Symmetric (harmonic-mean) coefficients →
//! SPD, so plain CG applies. Wiring this as the linear stage inside pflotran's
//! actual Newton loop on the assembled `LduMatrix` Jacobian, a distributed
//! preconditioner, and 2-D/3-D/unstructured coefficient operators remain the
//! op-gj5 follow-ups.

use outram_park_mpi::{Communicator, MpiResult};

use super::krylov::{distributed_cg_with, distributed_pcg_with};
use super::Decomposition1D;

/// Harmonic mean of two conductivities — the series face transmissibility of two
/// adjacent cells. Zero if both are zero (a closed face).
fn harmonic(a: f64, b: f64) -> f64 {
    if a + b > 0.0 {
        2.0 * a * b / (a + b)
    } else {
        0.0
    }
}

/// A distributed 1-D variable-coefficient diffusion operator over a
/// [`Decomposition1D`]. Holds this rank's conductivities plus its neighbours' edge
/// conductivities (halo-exchanged once), so a matvec needs only a field halo.
#[derive(Clone)]
pub struct DiffusionOperator1D {
    decomp: Decomposition1D,
    k: Vec<f64>,
    k_ghost_left: Option<f64>,
    k_ghost_right: Option<f64>,
    shift: f64,
}

impl DiffusionOperator1D {
    /// Build the operator: store `k` (this rank's per-cell conductivity) and
    /// halo-exchange the neighbours' edge conductivities once. `shift >= 0` adds a
    /// diagonal Helmholtz term; with Dirichlet ends the diffusion part is already
    /// SPD, so `shift = 0` is fine.
    ///
    /// # Errors
    /// [`crate::error::PflotranError::InvalidInput`] if `k.len()` differs from the
    /// local cell count; otherwise propagates any halo-exchange transport error.
    pub fn new(
        comm: &Communicator,
        decomp: Decomposition1D,
        k: Vec<f64>,
        shift: f64,
    ) -> MpiResult<Self> {
        // (Length is validated by callers building from the decomposition; the
        // halo exchange itself requires a non-empty slab only at rank boundaries.)
        let kh = super::exchange_halo(comm, &decomp, &k)?;
        Ok(DiffusionOperator1D {
            decomp,
            k,
            k_ghost_left: kh.left,
            k_ghost_right: kh.right,
            shift,
        })
    }

    /// Apply the operator to the distributed field `u` (this rank's slab),
    /// returning `A u`. Exchanges the field halo internally.
    ///
    /// # Errors
    /// Propagates any halo-exchange transport error.
    pub fn matvec(&self, comm: &Communicator, u: &[f64]) -> MpiResult<Vec<f64>> {
        let uh = super::exchange_halo(comm, &self.decomp, u)?;
        let n = u.len();
        let mut out = vec![0.0; n];
        for i in 0..n {
            // West face: interior cell, else a rank-boundary ghost (neighbour k),
            // else a physical Dirichlet-0 end (ghost k = k_i, u = 0).
            let (t_w, u_w) = if i > 0 {
                (harmonic(self.k[i], self.k[i - 1]), u[i - 1])
            } else if let Some(kl) = self.k_ghost_left {
                (harmonic(self.k[i], kl), uh.left.unwrap_or(0.0))
            } else {
                (self.k[i], 0.0)
            };
            let (t_e, u_e) = if i + 1 < n {
                (harmonic(self.k[i], self.k[i + 1]), u[i + 1])
            } else if let Some(kr) = self.k_ghost_right {
                (harmonic(self.k[i], kr), uh.right.unwrap_or(0.0))
            } else {
                (self.k[i], 0.0)
            };
            out[i] = self.shift * u[i] + t_w * (u[i] - u_w) + t_e * (u[i] - u_e);
        }
        Ok(out)
    }

    /// The operator's diagonal `A_ii` per local cell — the sum of this cell's face
    /// transmissibilities plus `shift`, computed locally from the stored
    /// coefficients and their halo (no communication).
    ///
    /// This is the Jacobi preconditioner's diagonal; a physical domain end uses the
    /// same Dirichlet-`k_i` face weight the matvec does, so `A_ii` is exact.
    pub fn diagonal(&self) -> Vec<f64> {
        let n = self.k.len();
        (0..n)
            .map(|i| {
                let t_w = if i > 0 {
                    harmonic(self.k[i], self.k[i - 1])
                } else if let Some(kl) = self.k_ghost_left {
                    harmonic(self.k[i], kl)
                } else {
                    self.k[i]
                };
                let t_e = if i + 1 < n {
                    harmonic(self.k[i], self.k[i + 1])
                } else if let Some(kr) = self.k_ghost_right {
                    harmonic(self.k[i], kr)
                } else {
                    self.k[i]
                };
                self.shift + t_w + t_e
            })
            .collect()
    }

    /// Solve `A u = b` for this operator by distributed conjugate gradient
    /// (via [`distributed_cg_with`]). Returns this rank's solution slab + iteration
    /// count.
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

    /// Solve `A u = b` by **Jacobi-preconditioned** distributed CG (via
    /// [`distributed_pcg_with`] with `M⁻¹ = diag(A)⁻¹`). The diagonal solve is
    /// purely local, so preconditioning costs no extra communication yet cuts the
    /// iteration count on a heterogeneous-coefficient system.
    ///
    /// # Errors
    /// Propagates any transport error from the matvec or reduced dot products.
    pub fn solve_jacobi_pcg(
        &self,
        comm: &Communicator,
        b: &[f64],
        tol: f64,
        max_iter: usize,
    ) -> MpiResult<(Vec<f64>, usize)> {
        let inv_diag: Vec<f64> = self.diagonal().iter().map(|d| 1.0 / d).collect();
        distributed_pcg_with(
            comm,
            b,
            |v| self.matvec(comm, v),
            |r| Ok(r.iter().zip(&inv_diag).map(|(ri, di)| ri * di).collect()),
            tol,
            max_iter,
        )
    }
}

/// Serial reference matvec for the variable-coefficient operator over the whole
/// `n_global`-cell grid — the correctness oracle.
pub fn serial_diffusion_matvec(n_global: usize, k: &[f64], u: &[f64], shift: f64) -> Vec<f64> {
    let mut out = vec![0.0; n_global];
    for gi in 0..n_global {
        let (t_w, u_w) = if gi > 0 {
            (harmonic(k[gi], k[gi - 1]), u[gi - 1])
        } else {
            (k[gi], 0.0)
        };
        let (t_e, u_e) = if gi + 1 < n_global {
            (harmonic(k[gi], k[gi + 1]), u[gi + 1])
        } else {
            (k[gi], 0.0)
        };
        out[gi] = shift * u[gi] + t_w * (u[gi] - u_w) + t_e * (u[gi] - u_e);
    }
    out
}

/// Serial CG reference solving the variable-coefficient system on one rank.
pub fn serial_diffusion_cg(
    n_global: usize,
    k: &[f64],
    b: &[f64],
    shift: f64,
    tol: f64,
    max_iter: usize,
) -> (Vec<f64>, usize) {
    let dot = |a: &[f64], c: &[f64]| -> f64 { a.iter().zip(c).map(|(x, y)| x * y).sum() };
    let mut x = vec![0.0; n_global];
    let mut r = b.to_vec();
    let mut p = r.clone();
    let mut rs_old = dot(&r, &r);
    let mut iters = 0;
    if rs_old.sqrt() < tol {
        return (x, 0);
    }
    for it in 0..max_iter {
        iters = it + 1;
        let ap = serial_diffusion_matvec(n_global, k, &p, shift);
        let alpha = rs_old / dot(&p, &ap);
        for i in 0..n_global {
            x[i] += alpha * p[i];
            r[i] -= alpha * ap[i];
        }
        let rs_new = dot(&r, &r);
        if rs_new.sqrt() < tol {
            break;
        }
        let beta = rs_new / rs_old;
        for i in 0..n_global {
            p[i] = r[i] + beta * p[i];
        }
        rs_old = rs_new;
    }
    (x, iters)
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_park_mpi::run;

    /// A heterogeneous conductivity field (orders-of-magnitude contrast) to make
    /// the operator genuinely position-dependent.
    fn conductivity(n: usize) -> Vec<f64> {
        (0..n)
            .map(|i| match i % 4 {
                0 => 1.0,
                1 => 10.0,
                2 => 0.1,
                _ => 3.0,
            })
            .collect()
    }

    #[test]
    fn distributed_variable_coefficient_solve_matches_serial() {
        let n = 40;
        let shift = 0.05_f64;
        let tol = 1e-11;
        let k = conductivity(n);
        let b: Vec<f64> = (0..n).map(|i| ((i as f64) * 0.25).cos() + 1.5).collect();
        let (reference, _) = serial_diffusion_cg(n, &k, &b, shift, tol, 2000);

        for p in [1, 2, 3, 5, 8] {
            let k = k.clone();
            let b = b.clone();
            let reference = reference.clone();
            let ok = run(p, move |c| {
                let d = Decomposition1D::new(n, c);
                let k_local = k[d.start..d.start + d.local_len].to_vec();
                let b_local = b[d.start..d.start + d.local_len].to_vec();
                let op = DiffusionOperator1D::new(c, d.clone(), k_local, shift).unwrap();
                let (x, _) = op.solve(c, &b_local, tol, 2000).unwrap();
                let expected = &reference[d.start..d.start + d.local_len];
                x.iter().zip(expected).all(|(a, e)| (a - e).abs() < 1e-7)
            })
            .unwrap();
            assert!(ok.iter().all(|&b| b), "variable-coef distributed != serial for p={p}");
        }
    }

    #[test]
    fn jacobi_pcg_matches_serial_and_cuts_iterations() {
        let n = 40;
        let shift = 0.05_f64;
        let tol = 1e-11;
        let k = conductivity(n);
        let b: Vec<f64> = (0..n).map(|i| ((i as f64) * 0.25).cos() + 1.5).collect();
        let (reference, _) = serial_diffusion_cg(n, &k, &b, shift, tol, 2000);

        for p in [1, 2, 4] {
            let k = k.clone();
            let b = b.clone();
            let reference = reference.clone();
            let out = run(p, move |c| {
                let d = Decomposition1D::new(n, c);
                let k_local = k[d.start..d.start + d.local_len].to_vec();
                let b_local = b[d.start..d.start + d.local_len].to_vec();
                let op = DiffusionOperator1D::new(c, d.clone(), k_local, shift).unwrap();
                let (x_cg, it_cg) = op.solve(c, &b_local, tol, 2000).unwrap();
                let (x_pcg, it_pcg) = op.solve_jacobi_pcg(c, &b_local, tol, 2000).unwrap();
                let expected = &reference[d.start..d.start + d.local_len];
                let match_pcg = x_pcg.iter().zip(expected).all(|(a, e)| (a - e).abs() < 1e-7);
                let match_cg = x_cg.iter().zip(expected).all(|(a, e)| (a - e).abs() < 1e-7);
                (match_cg && match_pcg, it_cg, it_pcg)
            })
            .unwrap();
            for (ok, it_cg, it_pcg) in &out {
                assert!(ok, "PCG or CG solution wrong for p={p}");
                // Jacobi preconditioning should not need more iterations than plain
                // CG on this heterogeneous system (and generally fewer).
                assert!(
                    it_pcg <= it_cg,
                    "Jacobi PCG took {it_pcg} iters vs CG {it_cg} (p={p})"
                );
            }
        }
    }

    #[test]
    fn distributed_matvec_matches_serial_matvec() {
        // The distributed operator applied to a known field equals the serial one,
        // cell-for-cell — proves the coefficient halo is exchanged correctly.
        let n = 24;
        let k = conductivity(n);
        let u: Vec<f64> = (0..n).map(|i| (i as f64).sqrt()).collect();
        let shift = 0.2_f64;
        let serial = serial_diffusion_matvec(n, &k, &u, shift);

        for p in [2, 3, 4] {
            let k = k.clone();
            let u = u.clone();
            let serial = serial.clone();
            let ok = run(p, move |c| {
                let d = Decomposition1D::new(n, c);
                let k_local = k[d.start..d.start + d.local_len].to_vec();
                let u_local = u[d.start..d.start + d.local_len].to_vec();
                let op = DiffusionOperator1D::new(c, d.clone(), k_local, shift).unwrap();
                let av = op.matvec(c, &u_local).unwrap();
                let expected = &serial[d.start..d.start + d.local_len];
                av.iter().zip(expected).all(|(a, e)| (a - e).abs() < 1e-12)
            })
            .unwrap();
            assert!(ok.iter().all(|&b| b), "matvec mismatch for p={p}");
        }
    }
}
