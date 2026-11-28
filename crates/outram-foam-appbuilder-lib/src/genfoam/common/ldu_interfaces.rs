// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! Coupled (`cyclic`) `lduInterface`s of an LDU matrix, in OpenFOAM-v2506's form (gh:#1905, README "Round 80").
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! OpenFOAM keeps a coupled patch OUT of the LDU addressing: the matrix carries per coupled face an
//! `interfaceBouCoeffs` entry (`fvMatrix::boundaryCoeffs_` of that patch), applied after the face loop of
//!
//! * `lduMatrix::Amul` (`lduMatrixATmul.C`): `updateMatrixInterfaces(true, ...)` ->
//!   `cyclicFvPatchField::updateInterfaceMatrix` -> `addToInternalField(result, !add, faceCells, coeffs, pnf)`:
//!   `result[faceCells] -= coeffs psi[nbrFaceCells]` ([`LduInterfaces::amul_update`]);
//! * `lduMatrix::residual` and the Gauss-Seidel smoothers' `bPrime` (`updateMatrixInterfaces(false, ...)`):
//!   `rA[faceCells] += coeffs psi_nbr` ([`LduInterfaces::residual_update`]);
//! * `lduMatrix::sumA` (`lduMatrix.C`): `sumA[faceCells] -= coeffs` ([`LduInterfaces::sum_a_update`]),
//!
//! and NOT in the `DIC`/`DILU`/`diagonal` preconditioners. The `internalCoeffs` of a coupled patch are added to
//! the diagonal before the solve (`addBoundaryDiag`), as for any other patch: that is the caller's.
//!
//! GAMG (`GAMGAgglomerateLduAddressing.C`, `cyclicGAMGInterface.C:39-115`, `GAMGInterface::agglomerateCoeffs`): each coarse
//! level has one interface per fine one; its coarse faces are the distinct `(local coarse cell, neighbour coarse cell)` pairs
//! (swapped on the non-owner side so that both sides agree), numbered in first-appearance order, and the coarse coefficients are
//! the SUMS of the fine ones ([`LduInterfaces::coarsen`]).
//!
//! Only translational (parallel) cyclics are represented: no `transformCoupleField` rotation.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use outram_foam_basic_lib::prelude::LduMatrix;

/// One coupled patch (`cyclicLduInterface` / `cyclicGAMGInterface`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoupledPatch {
    /// `faceCells()`: the cell of each coupled face on this side.
    pub face_cells: Vec<usize>,
    /// Index (in [`LduInterfaces::patches`]) of the neighbour patch (`neighbPatchID()`); face `i` of this patch is coupled to face `i`
    /// of that one.
    pub nbr_patch: usize,
    /// `cyclicPolyPatch::owner()` (`index() < neighbPatchID()`).
    pub owner: bool,
}

/// The coupled interfaces of a matrix with their `interfaceBouCoeffs`. The default value has none (every caller of the
/// interface-free solvers): every loop below is then empty and the arithmetic is the plain LDU one.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LduInterfaces {
    /// The coupled patches, in patch order.
    pub patches: Vec<CoupledPatch>,
    /// `interfaceBouCoeffs` per patch, per face.
    pub bou: Vec<Vec<f64>>,
}

impl LduInterfaces {
    /// No interfaces.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// `true` when there is no coupled patch.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.patches.is_empty()
    }

    /// `psi` at the neighbour face cells of patch `p` (`patchNeighbourField` of the interface).
    fn nbr_value(&self, p: usize, i: usize, psi: &[f64]) -> f64 {
        psi[self.patches[self.patches[p].nbr_patch].face_cells[i]]
    }

    /// The interface part of `Amul`: `result[faceCells] -= bou psi_nbr`, patch by patch, face by face.
    pub fn amul_update(&self, psi: &[f64], result: &mut [f64]) {
        for (p, patch) in self.patches.iter().enumerate() {
            for (i, &c) in patch.face_cells.iter().enumerate() {
                result[c] -= self.bou[p][i] * self.nbr_value(p, i, psi);
            }
        }
    }

    /// The interface part of `residual` and of `bPrime` in the Gauss-Seidel smoothers (v2506: `updateMatrixInterfaces(false,
    /// interfaceBouCoeffs, ...)`, so `addToInternalField(result, true, ...)`): `rA[faceCells] += bou psi_nbr`.
    pub fn residual_update(&self, psi: &[f64], r: &mut [f64]) {
        for (p, patch) in self.patches.iter().enumerate() {
            for (i, &c) in patch.face_cells.iter().enumerate() {
                r[c] += self.bou[p][i] * self.nbr_value(p, i, psi);
            }
        }
    }

    /// The interface part of `sumA`: `sumA[faceCells] -= bou`.
    pub fn sum_a_update(&self, sum_a: &mut [f64]) {
        for (p, patch) in self.patches.iter().enumerate() {
            for (i, &c) in patch.face_cells.iter().enumerate() {
                sum_a[c] -= self.bou[p][i];
            }
        }
    }

    /// The coarse-level interfaces for the fine-to-coarse cell map `restrict` (`cyclicGAMGInterface` constructor) and their
    /// coefficients (`agglomerateCoeffs`: the sum of the fine coefficients of each coarse face).
    #[must_use]
    pub fn coarsen(&self, restrict: &[usize]) -> Self {
        let mut patches = Vec::with_capacity(self.patches.len());
        let mut bou = Vec::with_capacity(self.patches.len());
        for (p, patch) in self.patches.iter().enumerate() {
            let nbr = &self.patches[patch.nbr_patch];
            let mut pairs: Vec<(usize, usize)> = Vec::new();
            let mut face_cells = Vec::new();
            let mut coeffs: Vec<f64> = Vec::new();
            for (ffi, &c) in patch.face_cells.iter().enumerate() {
                let local = restrict[c];
                let neighbour = restrict[nbr.face_cells[ffi]];
                let pair = if patch.owner { (local, neighbour) } else { (neighbour, local) };
                let k = match pairs.iter().position(|q| *q == pair) {
                    Some(k) => k,
                    None => {
                        pairs.push(pair);
                        face_cells.push(local);
                        coeffs.push(0.0);
                        pairs.len() - 1
                    }
                };
                coeffs[k] += self.bou[p][ffi];
            }
            patches.push(CoupledPatch { face_cells, nbr_patch: patch.nbr_patch, owner: patch.owner });
            bou.push(coeffs);
        }
        Self { patches, bou }
    }
}

/// `lduMatrix::Amul` with interfaces.
#[must_use]
pub fn amul(a: &LduMatrix, ifs: &LduInterfaces, psi: &[f64]) -> Vec<f64> {
    let mut y = a.multiply(psi);
    ifs.amul_update(psi, &mut y);
    y
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A periodic chain of `n` cells: internal faces `(i, i + 1)`, and the seam `n - 1 -> 0` as two coupled patches.
    fn ring(n: usize) -> (LduMatrix, LduInterfaces) {
        let owner: Vec<usize> = (0..n - 1).collect();
        let neighbour: Vec<usize> = (1..n).collect();
        let mut a = LduMatrix::new(n, owner, neighbour);
        a.diag = vec![2.0; n];
        a.upper = vec![-1.0; n - 1];
        a.lower = vec![-1.0; n - 1];
        let ifs = LduInterfaces {
            patches: vec![
                CoupledPatch { face_cells: vec![n - 1], nbr_patch: 1, owner: true },
                CoupledPatch { face_cells: vec![0], nbr_patch: 0, owner: false },
            ],
            bou: vec![vec![1.0], vec![1.0]],
        };
        (a, ifs)
    }

    #[test]
    fn a_constant_field_is_in_the_null_space_of_the_periodic_laplacian() {
        let (a, ifs) = ring(8);
        let y = amul(&a, &ifs, &[3.0; 8]);
        assert!(y.iter().all(|v| *v == 0.0), "{y:?}");
        let mut s = a.diag.clone();
        for f in 0..7 {
            s[f + 1] += a.lower[f];
            s[f] += a.upper[f];
        }
        ifs.sum_a_update(&mut s);
        assert!(s.iter().all(|v| *v == 0.0));
        // residual = source - Amul
        let mut r = vec![0.0; 8];
        ifs.residual_update(&[1.0; 8], &mut r);
        assert_eq!(r[0], 1.0);
        assert_eq!(r[7], 1.0);
    }

    /// A periodic Poisson problem with a reference cell: `GAMG` with the interfaces on every level, `PCG`/`DIC` with interfaces and the
    /// interface-free solver all reach the tolerance, the first two to the same solution; without the seam the solution is different.
    #[test]
    fn gamg_and_pcg_with_interfaces_solve_the_periodic_ring() {
        use crate::genfoam::common::gamg::{gamg_dic_solve_if, pair_agglomerate, GamgControls};
        use crate::genfoam::common::pcg::pcg_dic_if;
        let n = 64;
        let (mut a, ifs) = ring(n);
        a.diag[0] += 2.0; // a reference cell
        let b: Vec<f64> = (0..n).map(|i| ((i * 7) % 11) as f64 - 5.0).collect();
        let ctl = GamgControls { tolerance: 1e-12, ..GamgControls::default() };
        let mut forward = true;
        let agg = pair_agglomerate(n, &a.owner, &a.neighbour, &vec![1.0; n - 1], &ctl, &mut forward).expect("agglomerate");
        assert!(!agg.levels.is_empty());
        let mut x_g = vec![0.0; n];
        let pg = gamg_dic_solve_if(&a, &ifs, &agg, &ctl, &b, &mut x_g).expect("gamg");
        let mut x_p = vec![0.0; n];
        let pp = pcg_dic_if(&a, &ifs, &b, &mut x_p, 1e-12, 0.0, 1000, 0);
        assert!(pg.final_residual < 1e-12 && pp.final_residual < 1e-12, "{pg:?} {pp:?}");
        let scale = x_p.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
        assert!(x_g.iter().zip(&x_p).all(|(g, p)| (g - p).abs() <= 1e-9 * scale));
        let r = amul(&a, &ifs, &x_g);
        assert!(r.iter().zip(&b).all(|(y, s)| (y - s).abs() <= 1e-9 * scale));
        let mut x_n = vec![0.0; n];
        let _ = pcg_dic_if(&a, &LduInterfaces::none(), &b, &mut x_n, 1e-12, 0.0, 1000, 0);
        assert!(x_n.iter().zip(&x_p).any(|(u, p)| (u - p).abs() > 1e-3 * scale));
    }

    #[test]
    fn coarsening_keeps_one_face_per_pair_and_sums_the_coefficients() {
        let (_, mut ifs) = ring(4);
        ifs.patches[0].face_cells = vec![3, 2];
        ifs.patches[1].face_cells = vec![0, 1];
        ifs.bou = vec![vec![0.5, 0.25], vec![0.5, 0.25]];
        // cells (0, 1) -> 0 and (2, 3) -> 1: both fine faces map to the coarse pair (1, 0)
        let c = ifs.coarsen(&[0, 0, 1, 1]);
        assert_eq!(c.patches[0].face_cells, vec![1]);
        assert_eq!(c.patches[1].face_cells, vec![0]);
        assert_eq!(c.bou, vec![vec![0.75], vec![0.75]]);
        assert!(LduInterfaces::none().is_empty());
    }
}
