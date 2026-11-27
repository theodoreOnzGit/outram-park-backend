// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! OpenFOAM-v2506 `GAMGSolver` with the `faceAreaPair` agglomeration and the `DIC`, `GaussSeidel`, `DICGaussSeidel` and `symGaussSeidel`
//! smoothers (the last three and `nPreSweeps` since gh:#1936, README "Round 72"), translated line by line
//! (gh:#1934, README "Round 67"; maintainer 2026-11-27: "Match all upstream" = follow upstream including its solvers).
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! # Upstream (ESI v2506, `src/OpenFOAM/matrices/lduMatrix/...`, `src/finiteVolume/fvMatrices/solvers/GAMGSymSolver/...`)
//!
//! | piece | upstream file | here |
//! |---|---|---|
//! | `faceAreaPair` weights | `faceAreaPairGAMGAgglomeration.C:55-76` (the `requiresUpdate()` branch, always true with the default `updateInterval 1`) | [`face_area_pair_weights`] |
//! | cell pairing, `forward_` toggle, `continueAgglomerating`, `nCellsInCoarsestLevel_` clamp | `pairGAMGAgglomerate.C` (`agglomerate` both overloads), `GAMGAgglomeration.C:continueAgglomerating`, ctor | [`pair_agglomerate`] |
//! | coarse addressing, face restriction, flip map | `GAMGAgglomerateLduAddressing.C` | `agglomerate_ldu_addressing` |
//! | coarse matrices | `GAMGSolverAgglomerateMatrix.C:agglomerateMatrix` (symmetric branch) | `coarsen_matrix` |
//! | V-cycle, residual loop, convergence | `GAMGSolverSolve.C:solve, Vcycle` | [`gamg_dic_solve`], `v_cycle` |
//! | correction scaling | `GAMGSolverScale.C` | `scale` |
//! | `DIC` smoother | `smoothers/DIC/DICSmoother.C` | `dic_smooth` |
//! | `GaussSeidel`, `symGaussSeidel`, `DICGaussSeidel` smoothers; pre-smoothing | `GaussSeidelSmoother.C:74-176`, `symGaussSeidelSmoother.C:74-199`, `DICGaussSeidelSmoother.C:79-100`; `GAMGSolverSolve.C:186-253` | `gauss_seidel_smooth`, `sym_gauss_seidel_smooth`, `smooth`, `v_cycle` |
//! | coarsest level | `GAMGSolver.C:320-330` (`PCG`/`DIC` at the outer `tolerance`/`relTol`), `GAMGSolverSolve.C:PCGsolverDict` | [`super::pcg::pcg_dic`] |
//!
//! Defaults (v2506 `GAMGSolver.C` ctor): `nPreSweeps 0`, `preSweepsLevelMultiplier 1`, `maxPreSweeps 4`, `nPostSweeps 2`,
//! `postSweepsLevelMultiplier 1`, `maxPostSweeps 4`, `nFinestSweeps 2`, `cacheAgglomeration true`, `interpolateCorrection false`,
//! `scaleCorrection = matrix.symmetric()`, `directSolveCoarsest false`; agglomeration `agglomerator faceAreaPair`, `mergeLevels 1`,
//! `nCellsInCoarsestLevel 10`; solver `tolerance 1e-6`, `relTol 0`, `maxIter 1000`, `minIter 0`.
//!
//! # NOT ported (refused by name, see [`GamgControls::check_supported`])
//!
//! processor agglomeration and every parallel path; coupled interfaces (cyclic, AMI, processor); DIC/DICGaussSeidel on an asymmetric matrix (upstream has none; a symmetric
//! matrix is one whose `lower` equals `upper` bit for bit; asymmetric ones run with `GaussSeidel`/`symGaussSeidel` since gh:#1936); `interpolateCorrection true`; `directSolveCoarsest
//! true`; `mergeLevels != 1`; `cacheAgglomeration false`; agglomerators other than `faceAreaPair`; smoothers other than `DIC`,
//! `GaussSeidel`, `DICGaussSeidel`, `symGaussSeidel` (`DILU`, `FDIC`, `DILUGaussSeidel`, `nonBlockingGaussSeidel`).
//!
//! # Deliberate differences of form (no effect on any arithmetic)
//!
//! The per-coarse-cell face list of `agglomerateLduAddressing` is a `Vec<Vec<usize>>` instead of OpenFOAM's packed `cCellFaces` with
//! doubling; the entries and their order are the same. The `forward_` static of `pairGAMGAgglomeration` is a value the caller owns
//! and passes in (upstream: one process-wide `bool`, `true` at start, toggled once per `agglomerate` call, rejected last level
//! included).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use super::pcg::{converged, dic_reciprocal_diag, g_sum_mag, norm_factor, pcg_dic};
use crate::genfoam::thermo_mechanics::legacy_fvm::pbicg::{dilu_precondition, dilu_reciprocal_diag};
use outram_foam_basic_lib::prelude::LduMatrix;
#[allow(unused_imports)]
use petir::mathf::RealMath;

/// `gSumProd`.
fn g_sum_prod(a: &[f64], b: &[f64]) -> f64 {
    let mut s = 0.0;
    for i in 0..a.len() {
        s += a[i] * b[i];
    }
    s
}

/// `GREAT` for doubles (`doubleScalarGreat`).
const GREAT: f64 = 1.0e15;
/// `pTraits<double>::vsmall` (`stabilise` in `GAMGSolverScale.C`).
const VSMALL: f64 = 1e-300;

/// The `smoother` entry of a `GAMG` solver for a SYMMETRIC matrix (gh:#1936, README "Round 72"): `lduMatrix::smoother::New` selects one
/// per level. `DILU`/`DILUGaussSeidel`/`FDIC`/`nonBlockingGaussSeidel` and the `smoothSolver`s are refused by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GamgSmoother {
    /// `DIC` (`DICSmoother.C`, round 67): the default of every shipped `p_rgh`.
    #[default]
    Dic,
    /// `GaussSeidel` (`GaussSeidelSmoother.C:74-176`): the shipped `TStruct` entry of the thermo-mechanics region.
    GaussSeidel,
    /// `DICGaussSeidel` (`DICGaussSeidelSmoother.C:79-100`): `DIC` for `nSweeps`, then `GaussSeidel` for `nSweeps` (the commented-out
    /// neutronics `flux` entry of every tutorial).
    DicGaussSeidel,
    /// `symGaussSeidel` (`symGaussSeidelSmoother.C:74-199`): a forward then a backward sweep.
    SymGaussSeidel,
}

impl GamgSmoother {
    /// The `smoother` word of an `fvSolution` entry, or `None` for a smoother that is not ported.
    #[must_use]
    pub fn from_word(w: &str) -> Option<Self> {
        match w {
            "DIC" => Some(Self::Dic),
            "GaussSeidel" => Some(Self::GaussSeidel),
            "DICGaussSeidel" => Some(Self::DicGaussSeidel),
            "symGaussSeidel" => Some(Self::SymGaussSeidel),
            _ => None,
        }
    }

    /// `true` when the smoother needs `DICSmoother::rD_`.
    fn uses_dic(self) -> bool {
        matches!(self, Self::Dic | Self::DicGaussSeidel)
    }
}

/// The `solvers.p_rgh` entries of a `GAMG` solver (defaults of v2506).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GamgControls {
    /// `smoother` (default here `DIC`; upstream has no default and fails without the entry, which the readers refuse).
    pub smoother: GamgSmoother,
    /// `tolerance` (`lduMatrix::solver` default `1e-6`).
    pub tolerance: f64,
    /// `relTol` (default 0).
    pub rel_tol: f64,
    /// `maxIter` (default 1000).
    pub max_iter: usize,
    /// `minIter` (default 0).
    pub min_iter: usize,
    /// `nPreSweeps` (default 0; `> 0` since gh:#1936).
    pub n_pre_sweeps: usize,
    /// `nPostSweeps` (default 2).
    pub n_post_sweeps: usize,
    /// `preSweepsLevelMultiplier` (default 1).
    pub pre_sweeps_level_multiplier: usize,
    /// `maxPreSweeps` (default 4).
    pub max_pre_sweeps: usize,
    /// `postSweepsLevelMultiplier` (default 1).
    pub post_sweeps_level_multiplier: usize,
    /// `maxPostSweeps` (default 4).
    pub max_post_sweeps: usize,
    /// `nFinestSweeps` (default 2).
    pub n_finest_sweeps: usize,
    /// `nCellsInCoarsestLevel` (default 10).
    pub n_cells_in_coarsest_level: usize,
    /// `mergeLevels` (default 1; other values are refused).
    pub merge_levels: usize,
    /// `scaleCorrection` (`None`: the default, `matrix.symmetric()`).
    pub scale_correction: Option<bool>,
    /// `true` when upstream's matrix HAS a `lower` array although it equals `upper` value for value (`fvMatrix::operator*=` by a field
    /// allocates it: the row-scaled `TStruct` equation): the solve then takes the asymmetric path (gh:#1936).
    pub has_lower: bool,
}

impl Default for GamgControls {
    fn default() -> Self {
        Self {
            smoother: GamgSmoother::Dic,
            pre_sweeps_level_multiplier: 1,
            max_pre_sweeps: 4,
            tolerance: 1e-6,
            rel_tol: 0.0,
            max_iter: 1000,
            min_iter: 0,
            n_pre_sweeps: 0,
            n_post_sweeps: 2,
            post_sweeps_level_multiplier: 1,
            max_post_sweeps: 4,
            n_finest_sweeps: 2,
            n_cells_in_coarsest_level: 10,
            merge_levels: 1,
            scale_correction: None,
            has_lower: false,
        }
    }
}

impl GamgControls {
    /// Refuse, by name, the selections that are not ported.
    ///
    /// # Errors
    /// Names the first unsupported selection.
    pub fn check_supported(&self) -> Result<(), String> {
        if self.merge_levels != 1 {
            return Err("GAMG mergeLevels != 1 (combineLevels) is not ported".to_string());
        }
        Ok(())
    }
}

/// One agglomeration level (`GAMGAgglomeration` arrays of index `fineLevelIndex`, plus the coarse mesh's addressing).
#[derive(Debug, Clone, PartialEq)]
pub struct AggLevel {
    /// `nCells_[level]`: coarse cells.
    pub n_cells: usize,
    /// `nFaces_[level]`: coarse faces.
    pub n_faces: usize,
    /// `restrictAddressing_[level]`: fine cell -> coarse cell.
    pub restrict: Vec<usize>,
    /// `faceRestrictAddressing_[level]`: fine face -> coarse face, or `-(coarse cell + 1)` for a face inside a coarse cell.
    pub face_restrict: Vec<isize>,
    /// `faceFlipMap_[level]`.
    pub face_flip: Vec<bool>,
    /// The coarse mesh's `lowerAddr` (coarse owner).
    pub lower: Vec<usize>,
    /// The coarse mesh's `upperAddr` (coarse neighbour).
    pub upper: Vec<usize>,
}

/// The cached agglomeration (`GAMGAgglomeration`, levels `0 .. size()`).
#[derive(Debug, Clone, PartialEq)]
pub struct GamgAgglomeration {
    /// Cells of the finest level.
    pub n_fine_cells: usize,
    /// The created levels.
    pub levels: Vec<AggLevel>,
}

/// `faceAreaPairGAMGAgglomeration` face weights: `mag(cmptMultiply(Sf/sqrt(magSf), vector(1, 1.01, 1.02)))` for the internal
/// faces (`faceAreaPairGAMGAgglomeration.C:62-76`, the `requiresUpdate()` branch).
#[must_use]
pub fn face_area_pair_weights(sf: &[[f64; 3]]) -> Vec<f64> {
    sf.iter()
        .map(|v| {
            // magSf = mag(Sf) = sqrt(x*x + y*y + z*z); `Sf/sqrt(magSf)` divides each component; cmptMultiply; mag
            let mag_sf = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            let s = mag_sf.sqrt();
            let a = v[0] / s * 1.0;
            let b = v[1] / s * 1.01;
            let c = v[2] / s * 1.02;
            (a * a + b * b + c * c).sqrt()
        })
        .collect()
}

/// `pairGAMGAgglomeration::agglomerate(nCoarseCells, fineMatrixAddressing, faceWeights)` (`pairGAMGAgglomerate.C:144-300`): one level of
/// pair agglomeration. Toggles `forward`. Returns `(coarseCellMap, nCoarseCells)`.
fn agglomerate_level(n_fine: usize, lower: &[usize], upper: &[usize], face_weights: &[f64], forward: &mut bool) -> (Vec<usize>, usize) {
    // for each cell calculate faces
    let mut n_nbrs = vec![0usize; n_fine];
    for &u in upper {
        n_nbrs[u] += 1;
    }
    for &l in lower {
        n_nbrs[l] += 1;
    }
    let mut offsets = vec![0usize; n_fine + 1];
    for c in 0..n_fine {
        offsets[c + 1] = offsets[c] + n_nbrs[c];
    }
    let mut cell_faces = vec![0usize; upper.len() + lower.len()];
    for v in n_nbrs.iter_mut() {
        *v = 0;
    }
    for (facei, &u) in upper.iter().enumerate() {
        cell_faces[offsets[u] + n_nbrs[u]] = facei;
        n_nbrs[u] += 1;
    }
    for (facei, &l) in lower.iter().enumerate() {
        cell_faces[offsets[l] + n_nbrs[l]] = facei;
        n_nbrs[l] += 1;
    }
    // go through the faces and create clusters
    let mut map: Vec<isize> = vec![-1; n_fine];
    let mut n_coarse: isize = 0;
    for cellfi in 0..n_fine {
        let celli = if *forward { cellfi } else { n_fine - cellfi - 1 };
        if map[celli] < 0 {
            let mut match_face: isize = -1;
            let mut max_w = -GREAT;
            for faceos in offsets[celli]..offsets[celli + 1] {
                let facei = cell_faces[faceos];
                if map[upper[facei]] < 0 && map[lower[facei]] < 0 && face_weights[facei] > max_w {
                    match_face = facei as isize;
                    max_w = face_weights[facei];
                }
            }
            if match_face >= 0 {
                let mf = match_face as usize;
                map[upper[mf]] = n_coarse;
                map[lower[mf]] = n_coarse;
                n_coarse += 1;
            } else {
                // no match: the best neighbouring cluster
                let mut cl_face: isize = -1;
                let mut cl_max = -GREAT;
                for faceos in offsets[celli]..offsets[celli + 1] {
                    let facei = cell_faces[faceos];
                    if face_weights[facei] > cl_max {
                        cl_face = facei as isize;
                        cl_max = face_weights[facei];
                    }
                }
                if cl_face >= 0 {
                    let cf = cl_face as usize;
                    map[celli] = map[upper[cf]].max(map[lower[cf]]);
                }
            }
        }
    }
    // single-cell clusters for the cells that are in none
    for cellfi in 0..n_fine {
        let celli = if *forward { cellfi } else { n_fine - cellfi - 1 };
        if map[celli] < 0 {
            map[celli] = n_coarse;
            n_coarse += 1;
        }
    }
    if !*forward {
        n_coarse -= 1;
        for m in map.iter_mut() {
            *m = n_coarse - *m;
        }
        n_coarse += 1;
    }
    // reverse the map ordering for the next level
    *forward = !*forward;
    (map.into_iter().map(|m| m as usize).collect(), n_coarse as usize)
}

/// `GAMGAgglomeration::agglomerateLduAddressing(fineLevelIndex)` (`GAMGAgglomerateLduAddressing.C:40-205`), without interfaces.
fn agglomerate_ldu_addressing(n_coarse_cells: usize, lower: &[usize], upper: &[usize], restrict: &[usize]) -> AggLevel {
    let n_fine_faces = upper.len();
    let mut cc_faces: Vec<Vec<usize>> = vec![Vec::new(); n_coarse_cells];
    let mut face_restrict = vec![0isize; n_fine_faces];
    let mut init_coarse_nbr: Vec<usize> = Vec::new();
    let mut n_coarse_faces = 0usize;
    for fine in 0..n_fine_faces {
        let rm_upper = restrict[upper[fine]];
        let rm_lower = restrict[lower[fine]];
        if rm_upper == rm_lower {
            face_restrict[fine] = -(rm_upper as isize + 1);
        } else {
            let (c_own, c_nei) = if rm_upper > rm_lower { (rm_lower, rm_upper) } else { (rm_upper, rm_lower) };
            let mut found = false;
            for &cf in &cc_faces[c_own] {
                if init_coarse_nbr[cf] == c_nei {
                    found = true;
                    face_restrict[fine] = cf as isize;
                    break;
                }
            }
            if !found {
                cc_faces[c_own].push(n_coarse_faces);
                init_coarse_nbr.push(c_nei);
                face_restrict[fine] = n_coarse_faces as isize;
                n_coarse_faces += 1;
            }
        }
    }
    // renumber into upper-triangular order
    let mut coarse_owner = Vec::with_capacity(n_coarse_faces);
    let mut coarse_neighbour = Vec::with_capacity(n_coarse_faces);
    let mut coarse_face_map = vec![0usize; n_coarse_faces];
    for (cci, faces) in cc_faces.iter().enumerate() {
        for &cf in faces {
            coarse_face_map[cf] = coarse_owner.len();
            coarse_owner.push(cci);
            coarse_neighbour.push(init_coarse_nbr[cf]);
        }
    }
    for fr in face_restrict.iter_mut() {
        if *fr >= 0 {
            *fr = coarse_face_map[*fr as usize] as isize;
        }
    }
    // face-flip status
    let mut face_flip = vec![false; n_fine_faces];
    for fine in 0..n_fine_faces {
        let cf = face_restrict[fine];
        if cf >= 0 {
            let c_own = coarse_owner[cf as usize];
            let c_nei = coarse_neighbour[cf as usize];
            let rm_upper = restrict[upper[fine]];
            let rm_lower = restrict[lower[fine]];
            if c_own == rm_upper && c_nei == rm_lower {
                face_flip[fine] = true;
            }
        }
    }
    AggLevel {
        n_cells: n_coarse_cells,
        n_faces: n_coarse_faces,
        restrict: restrict.to_vec(),
        face_restrict,
        face_flip,
        lower: coarse_owner,
        upper: coarse_neighbour,
    }
}

/// `faceAreaPairGAMGAgglomeration` on a mesh: `pairGAMGAgglomeration::agglomerate(nCellsInCoarsestLevel_, 0, weights, true)`
/// (`pairGAMGAgglomerate.C:34-141`) with `maxLevels_ = 50` and `mergeLevels 1`.
///
/// `lower`/`upper` are the mesh's `lowerAddr`/`upperAddr` (owner, neighbour of the internal faces); `face_weights` come from
/// [`face_area_pair_weights`]; `forward` is the process-wide `pairGAMGAgglomeration::forward_` (`true` before the first
/// agglomeration of a process) and is toggled once per level attempted.
///
/// # Errors
/// `merge_levels != 1`.
pub fn pair_agglomerate(
    n_cells: usize,
    lower: &[usize],
    upper: &[usize],
    face_weights: &[f64],
    ctl: &GamgControls,
    forward: &mut bool,
) -> Result<GamgAgglomeration, String> {
    ctl.check_supported()?;
    const MAX_LEVELS: usize = 50;
    // GAMGAgglomeration ctor: nCellsInCoarsestLevel_ = max(1, min(nCells/2, nCellsInCoarsestLevel))
    let n_cells_coarsest = (n_cells / 2).min(ctl.n_cells_in_coarsest_level).max(1);
    let mut weights = face_weights.to_vec();
    let mut levels: Vec<AggLevel> = Vec::new();
    let mut cur_lower = lower.to_vec();
    let mut cur_upper = upper.to_vec();
    let mut cur_cells = n_cells;
    while levels.len() < MAX_LEVELS - 1 {
        let (map, n_coarse) = agglomerate_level(cur_cells, &cur_lower, &cur_upper, &weights, forward);
        // continueAgglomerating (one process)
        let keep = if n_coarse < n_cells_coarsest { false } else { n_coarse < map.len() };
        if !keep {
            break;
        }
        let lvl = agglomerate_ldu_addressing(n_coarse, &cur_lower, &cur_upper, &map);
        // agglomerate the faceWeights for the next level (restrictFaceField)
        let mut agg = vec![0.0; lvl.n_faces];
        for (ffacei, &c) in lvl.face_restrict.iter().enumerate() {
            if c >= 0 {
                agg[c as usize] += weights[ffacei];
            }
        }
        weights = agg;
        cur_lower = lvl.lower.clone();
        cur_upper = lvl.upper.clone();
        cur_cells = n_coarse;
        levels.push(lvl);
    }
    Ok(GamgAgglomeration { n_fine_cells: n_cells, levels })
}

/// `GAMGAgglomeration::restrictField(cf, ff, fineToCoarse)`.
fn restrict_field(ff: &[f64], map: &[usize], n_coarse: usize) -> Vec<f64> {
    let mut cf = vec![0.0; n_coarse];
    for (i, &v) in ff.iter().enumerate() {
        cf[map[i]] += v;
    }
    cf
}

/// `GAMGSolver::agglomerateMatrix` for a symmetric matrix (`GAMGSolverAgglomerateMatrix.C:44-160`).
fn coarsen_matrix(fine: &LduMatrix, lvl: &AggLevel) -> LduMatrix {
    let mut coarse_diag = restrict_field(&fine.diag, &lvl.restrict, lvl.n_cells);
    let mut coarse_upper = vec![0.0; lvl.n_faces];
    for (fine_face, &c) in lvl.face_restrict.iter().enumerate() {
        if c >= 0 {
            coarse_upper[c as usize] += fine.upper[fine_face];
        } else {
            coarse_diag[(-1 - c) as usize] += 2.0 * fine.upper[fine_face];
        }
    }
    let mut m = LduMatrix::new(lvl.n_cells, lvl.lower.clone(), lvl.upper.clone());
    m.diag = core::mem::take(&mut coarse_diag);
    m.lower = coarse_upper.clone();
    m.upper = coarse_upper;
    m
}

/// `GAMGSolver::agglomerateMatrix` for a matrix with a `lower` array (`GAMGSolverAgglomerateMatrix.C:151-180`, gh:#1936): the coarse
/// `upper`/`lower` take the fine ones (swapped on a flipped face), a face inside a coarse cell adds `upper + lower` to the diagonal.
fn coarsen_matrix_asymmetric(fine: &LduMatrix, lvl: &AggLevel) -> LduMatrix {
    let mut coarse_diag = restrict_field(&fine.diag, &lvl.restrict, lvl.n_cells);
    let mut coarse_upper = vec![0.0; lvl.n_faces];
    let mut coarse_lower = vec![0.0; lvl.n_faces];
    for (fine_face, &c) in lvl.face_restrict.iter().enumerate() {
        if c >= 0 {
            let cf = c as usize;
            if !lvl.face_flip[fine_face] {
                coarse_upper[cf] += fine.upper[fine_face];
                coarse_lower[cf] += fine.lower[fine_face];
            } else {
                coarse_upper[cf] += fine.lower[fine_face];
                coarse_lower[cf] += fine.upper[fine_face];
            }
        } else {
            coarse_diag[(-1 - c) as usize] += fine.upper[fine_face] + fine.lower[fine_face];
        }
    }
    let mut m = LduMatrix::new(lvl.n_cells, lvl.lower.clone(), lvl.upper.clone());
    m.diag = core::mem::take(&mut coarse_diag);
    m.lower = coarse_lower;
    m.upper = coarse_upper;
    m
}

/// `true` when `a` has no separate `lower` (OpenFOAM `lduMatrix::symmetric()`; here: `lower == upper` bit for bit).
fn is_symmetric(a: &LduMatrix) -> bool {
    a.lower.iter().zip(&a.upper).all(|(l, u)| l.to_bits() == u.to_bits())
}

/// `PBiCGStab::scalarSolve` with `preconditioner DILU` (`PBiCGStab.C:63-290`, `DILUPreconditioner`), gh:#1936: the coarsest-level solver
/// of an asymmetric `GAMG` (`GAMGSolver::PBiCGStabSolverDict`). `psi` is the initial guess; returns `(iterations)`.
fn pbicgstab_dilu(a: &LduMatrix, source: &[f64], psi: &mut [f64], tolerance: f64, rel_tol: f64, max_iter: usize, min_iter: usize) -> usize {
    let n = psi.len();
    let y_a0 = a.multiply(psi);
    let mut r_a: Vec<f64> = (0..n).map(|c| source[c] - y_a0[c]).collect();
    let norm = norm_factor(a, psi, source, &y_a0);
    let initial = g_sum_mag(&r_a) / norm;
    let mut final_res = initial;
    let mut iterations = 0usize;
    if min_iter == 0 && converged(final_res, initial, tolerance, rel_tol) {
        return 0;
    }
    let r_a0 = r_a.clone();
    let r_d = dilu_reciprocal_diag(a);
    let mut p_a = vec![0.0; n];
    let mut y_a = vec![0.0; n];
    let mut ay_a = vec![0.0; n];
    let mut z_a = vec![0.0; n];
    let mut r_a0_r_a = 0.0_f64;
    let mut alpha = 0.0_f64;
    let mut omega = 0.0_f64;
    loop {
        let r_a0_r_a_old = r_a0_r_a;
        r_a0_r_a = g_sum_prod(&r_a0, &r_a);
        if r_a0_r_a.abs() < VSMALL {
            break;
        }
        if iterations == 0 {
            p_a.copy_from_slice(&r_a);
        } else {
            if omega.abs() < VSMALL {
                break;
            }
            let beta = (r_a0_r_a / r_a0_r_a_old) * (alpha / omega);
            for c in 0..n {
                p_a[c] = r_a[c] + beta * (p_a[c] - omega * ay_a[c]);
            }
        }
        dilu_precondition(a, &r_d, &p_a, &mut y_a);
        ay_a = a.multiply(&y_a);
        let r_a0_ay_a = g_sum_prod(&r_a0, &ay_a);
        alpha = r_a0_r_a / r_a0_ay_a;
        let s_a: Vec<f64> = (0..n).map(|c| r_a[c] - alpha * ay_a[c]).collect();
        final_res = g_sum_mag(&s_a) / norm;
        if iterations >= min_iter && converged(final_res, initial, tolerance, rel_tol) {
            for c in 0..n {
                psi[c] += alpha * y_a[c];
            }
            return iterations + 1;
        }
        dilu_precondition(a, &r_d, &s_a, &mut z_a);
        let t_a = a.multiply(&z_a);
        let mut t_a_t_a = 0.0;
        for v in &t_a {
            t_a_t_a += v * v;
        }
        omega = g_sum_prod(&t_a, &s_a) / t_a_t_a;
        for c in 0..n {
            psi[c] += alpha * y_a[c] + omega * z_a[c];
            r_a[c] = s_a[c] - omega * t_a[c];
        }
        final_res = g_sum_mag(&r_a) / norm;
        iterations += 1;
        if !((iterations < max_iter && !converged(final_res, initial, tolerance, rel_tol)) || iterations < min_iter) {
            break;
        }
    }
    iterations
}

/// The coarse matrices of `a` over `agg` (`matrixLevels_` of `GAMGSolver`: entry `i` is the matrix of coarse level `i + 1`).
#[must_use]
pub fn coarse_matrices(a: &LduMatrix, agg: &GamgAgglomeration) -> Vec<LduMatrix> {
    coarse_matrices_for(a, agg, is_symmetric(a))
}

/// [`coarse_matrices`] with the symmetry decided by the caller (`lduMatrix::symmetric()` is whether `lower` is allocated).
fn coarse_matrices_for(a: &LduMatrix, agg: &GamgAgglomeration, sym: bool) -> Vec<LduMatrix> {
    let mut mats: Vec<LduMatrix> = Vec::with_capacity(agg.levels.len());
    for (i, lvl) in agg.levels.iter().enumerate() {
        let fine = if i == 0 { a } else { &mats[i - 1] };
        let coarse = if sym { coarsen_matrix(fine, lvl) } else { coarsen_matrix_asymmetric(fine, lvl) };
        mats.push(coarse);
    }
    mats
}

/// `lduMatrix::residual(rA, psi, source)` (`lduMatrixATmul.C:268-340`): note `source - diag psi` first, then the faces
/// subtracted, which is NOT `source - Amul(psi)` in floating point.
fn ldu_residual(a: &LduMatrix, psi: &[f64], source: &[f64]) -> Vec<f64> {
    let mut r: Vec<f64> = (0..psi.len()).map(|c| source[c] - a.diag[c] * psi[c]).collect();
    for f in 0..a.n_internal_faces {
        let (l, u) = (a.owner[f], a.neighbour[f]);
        r[u] -= a.lower[f] * psi[l];
        r[l] -= a.upper[f] * psi[u];
    }
    r
}

/// `DICSmoother::smooth` (`DICSmoother.C:73-123`) with the reciprocal diagonal `r_d` of `DICSmoother`'s constructor.
fn dic_smooth(a: &LduMatrix, r_d: &[f64], psi: &mut [f64], source: &[f64], n_sweeps: usize) {
    for _ in 0..n_sweeps {
        let mut r_a = ldu_residual(a, psi, source);
        for i in 0..r_a.len() {
            r_a[i] *= r_d[i];
        }
        let n_faces = a.n_internal_faces;
        for facei in 0..n_faces {
            let u = a.neighbour[facei];
            r_a[u] -= r_d[u] * a.upper[facei] * r_a[a.owner[facei]];
        }
        for facei in (0..n_faces).rev() {
            let l = a.owner[facei];
            r_a[l] -= r_d[l] * a.upper[facei] * r_a[a.neighbour[facei]];
        }
        for i in 0..psi.len() {
            psi[i] += r_a[i];
        }
    }
}

/// `lduAddressing::ownerStartAddr()`: the first face of each owner cell (faces are in upper-triangular order).
fn owner_start(a: &LduMatrix) -> Vec<usize> {
    let n = a.n_cells;
    let mut start = vec![0usize; n + 1];
    for &o in &a.owner[..a.n_internal_faces] {
        start[o + 1] += 1;
    }
    for c in 0..n {
        start[c + 1] += start[c];
    }
    start
}

/// `GaussSeidelSmoother::smooth` (`GaussSeidelSmoother.C:74-176`, gh:#1936), no interfaces: `bPrime = source`, then per cell the owner side
/// `psii = bPrime - sum upper psi[u]`, `psii /= diag`, the neighbour side `bPrime[u] -= lower psii`.
fn gauss_seidel_smooth(a: &LduMatrix, psi: &mut [f64], source: &[f64], n_sweeps: usize) {
    let own_start = owner_start(a);
    let n = psi.len();
    for _ in 0..n_sweeps {
        let mut b_prime: Vec<f64> = source.to_vec();
        for celli in 0..n {
            let mut psii = b_prime[celli];
            for facei in own_start[celli]..own_start[celli + 1] {
                psii -= a.upper[facei] * psi[a.neighbour[facei]];
            }
            psii /= a.diag[celli];
            for facei in own_start[celli]..own_start[celli + 1] {
                b_prime[a.neighbour[facei]] -= a.lower[facei] * psii;
            }
            psi[celli] = psii;
        }
    }
}

/// `symGaussSeidelSmoother::smooth` (`symGaussSeidelSmoother.C:74-199`, gh:#1936): the forward sweep of [`gauss_seidel_smooth`], then a
/// backward sweep that finishes each cell from the same `bPrime` without distributing.
fn sym_gauss_seidel_smooth(a: &LduMatrix, psi: &mut [f64], source: &[f64], n_sweeps: usize) {
    let own_start = owner_start(a);
    let n = psi.len();
    for _ in 0..n_sweeps {
        let mut b_prime: Vec<f64> = source.to_vec();
        for celli in 0..n {
            let mut psii = b_prime[celli];
            for facei in own_start[celli]..own_start[celli + 1] {
                psii -= a.upper[facei] * psi[a.neighbour[facei]];
            }
            psii /= a.diag[celli];
            for facei in own_start[celli]..own_start[celli + 1] {
                b_prime[a.neighbour[facei]] -= a.lower[facei] * psii;
            }
            psi[celli] = psii;
        }
        for celli in (0..n).rev() {
            let mut psii = b_prime[celli];
            for facei in own_start[celli]..own_start[celli + 1] {
                psii -= a.upper[facei] * psi[a.neighbour[facei]];
            }
            psii /= a.diag[celli];
            psi[celli] = psii;
        }
    }
}

/// `lduMatrix::smoother::smooth` of the selected smoother (`r_d` is `DICSmoother::rD_`, empty when the smoother does not use it).
fn smooth(kind: GamgSmoother, a: &LduMatrix, r_d: &[f64], psi: &mut [f64], source: &[f64], n_sweeps: usize) {
    match kind {
        GamgSmoother::Dic => dic_smooth(a, r_d, psi, source, n_sweeps),
        GamgSmoother::GaussSeidel => gauss_seidel_smooth(a, psi, source, n_sweeps),
        GamgSmoother::SymGaussSeidel => sym_gauss_seidel_smooth(a, psi, source, n_sweeps),
        GamgSmoother::DicGaussSeidel => {
            dic_smooth(a, r_d, psi, source, n_sweeps);
            gauss_seidel_smooth(a, psi, source, n_sweeps);
        }
    }
}

/// `GAMGSolver::scale` (`GAMGSolverScale.C:36-86`): `Acf` is scratch.
fn scale(field: &mut [f64], a: &LduMatrix, source: &[f64]) {
    let acf = a.multiply(field);
    let mut sf0 = 0.0;
    let mut sf1 = 0.0;
    for i in 0..field.len() {
        sf0 += field[i] * source[i];
        sf1 += field[i] * acf[i];
    }
    // stabilise(sf1, vsmall)
    let stab = if sf1 >= 0.0 { sf1 + VSMALL } else { sf1 - VSMALL };
    let sf = sf0 / stab;
    for i in 0..field.len() {
        field[i] = sf * field[i] + (source[i] - sf * acf[i]) / a.diag[i];
    }
}

/// `solverPerformance` of a GAMG solve, plus the residual after every V-cycle (for the comparison with upstream's per-cycle dump).
#[derive(Debug, Clone, PartialEq)]
pub struct GamgPerformance {
    /// Normalised residual before the first cycle.
    pub initial_residual: f64,
    /// Normalised residual at exit.
    pub final_residual: f64,
    /// V-cycles performed.
    pub iterations: usize,
    /// Normalised residual after each V-cycle.
    pub cycles: Vec<f64>,
}

/// The smoothers and matrices of one solve (`matrixLevels_`, `smoothers`).
struct Hierarchy {
    /// `matrixLevels_[i]`: the matrix of coarse level `i + 1`.
    mats: Vec<LduMatrix>,
    /// `lduMatrix::symmetric()` of the fine matrix (every level has a `lower` array when it has none).
    symmetric: bool,
    /// `DICSmoother::rD_` of the fine matrix and of each coarse one (`smoothers[0]`, `smoothers[i + 1]`).
    r_d: Vec<Vec<f64>>,
}

/// `GAMGSolver::Vcycle` (`GAMGSolverSolve.C:156-380`) with `interpolateCorrection = false`; `nPreSweeps > 0` since gh:#1936.
fn v_cycle(h: &Hierarchy, agg: &GamgAgglomeration, a: &LduMatrix, ctl: &GamgControls, scale_corr: bool, psi: &mut [f64], source: &[f64], finest_residual: &[f64]) {
    let coarsest = h.mats.len() - 1;
    let mut sources: Vec<Vec<f64>> = Vec::with_capacity(h.mats.len());
    // restrict the finest-grid residual, then going to coarser levels
    sources.push(restrict_field(finest_residual, &agg.levels[0].restrict, agg.levels[0].n_cells));
    let mut pre_smoothed: Vec<Vec<f64>> = vec![Vec::new(); h.mats.len()];
    for leveli in 0..coarsest {
        if ctl.n_pre_sweeps > 0 {
            // optional pre-smoothing of the restricted source (`GAMGSolverSolve.C:200-250`)
            let mut cc = vec![0.0; sources[leveli].len()];
            let n = (ctl.n_pre_sweeps + ctl.pre_sweeps_level_multiplier * leveli).min(ctl.max_pre_sweeps);
            smooth(ctl.smoother, &h.mats[leveli], &h.r_d[leveli + 1], &mut cc, &sources[leveli], n);
            if scale_corr && leveli < coarsest - 1 {
                scale(&mut cc, &h.mats[leveli], &sources[leveli]);
            }
            // `matrixLevels_[leveli].residual(coarseSources[leveli], coarseCorrFields[leveli], coarseSources[leveli])`
            let resid = ldu_residual(&h.mats[leveli], &cc, &sources[leveli]);
            sources[leveli] = resid;
            pre_smoothed[leveli] = cc;
        }
        let next = restrict_field(&sources[leveli], &agg.levels[leveli + 1].restrict, agg.levels[leveli + 1].n_cells);
        sources.push(next);
    }
    // coarsest level: PCG/DIC at the outer tolerance and relTol (`PCGsolverDict`), default maxIter 1000, minIter 0
    let mut corr: Vec<Vec<f64>> = vec![Vec::new(); h.mats.len()];
    let mut cc = vec![0.0; h.mats[coarsest].diag.len()];
    if h.symmetric {
        let _ = pcg_dic(&h.mats[coarsest], &sources[coarsest], &mut cc, ctl.tolerance, ctl.rel_tol, 1000, 0);
    } else {
        // `GAMGSolver.C:298-312`: an asymmetric coarsest matrix is solved by `PBiCGStab`/`DILU` (`PBiCGStabSolverDict`)
        let _ = pbicgstab_dilu(&h.mats[coarsest], &sources[coarsest], &mut cc, ctl.tolerance, ctl.rel_tol, 1000, 0);
    }
    corr[coarsest] = cc;
    // smoothing and prolongation going to finer levels
    for leveli in (0..coarsest).rev() {
        let map = &agg.levels[leveli + 1].restrict;
        let mut c: Vec<f64> = (0..map.len()).map(|i| corr[leveli + 1][map[i]]).collect();
        if scale_corr && leveli < coarsest - 1 {
            scale(&mut c, &h.mats[leveli], &sources[leveli]);
        }
        if ctl.n_pre_sweeps > 0 {
            for i in 0..c.len() {
                c[i] += pre_smoothed[leveli][i];
            }
        }
        let n = (ctl.n_post_sweeps + ctl.post_sweeps_level_multiplier * leveli).min(ctl.max_post_sweeps);
        smooth(ctl.smoother, &h.mats[leveli], &h.r_d[leveli + 1], &mut c, &sources[leveli], n);
        corr[leveli] = c;
    }
    // prolong the finest level correction
    let map0 = &agg.levels[0].restrict;
    let mut fc: Vec<f64> = (0..map0.len()).map(|i| corr[0][map0[i]]).collect();
    if scale_corr {
        scale(&mut fc, a, finest_residual);
    }
    for i in 0..psi.len() {
        psi[i] += fc[i];
    }
    smooth(ctl.smoother, a, &h.r_d[0], psi, source, ctl.n_finest_sweeps);
}

/// `GAMGSolver::solve` (`GAMGSolverSolve.C:36-153`) for a symmetric `a` (`lower == upper`), with the `DIC` smoother and the `faceAreaPair`
/// agglomeration `agg`. `psi` is the initial guess and receives the solution.
///
/// # Errors
/// An asymmetric matrix, an unsupported control, or no coarse level (upstream: `FatalError`, "No coarse levels created").
pub fn gamg_dic_solve(a: &LduMatrix, agg: &GamgAgglomeration, ctl: &GamgControls, source: &[f64], psi: &mut [f64]) -> Result<GamgPerformance, String> {
    ctl.check_supported()?;
    let sym = is_symmetric(a) && !ctl.has_lower;
    if !sym && ctl.smoother.uses_dic() {
        return Err("GAMG: the DIC and DICGaussSeidel smoothers need a symmetric matrix (upstream has no asymmetric `DIC`)".to_string());
    }
    if agg.levels.is_empty() {
        return Err("GAMG: no coarse levels created (matrix too small for GAMG or nCellsInCoarsestLevel too large)".to_string());
    }
    // the constructor: agglomerate the matrices, build the smoothers
    let mats = coarse_matrices_for(a, agg, sym);
    let mut r_d = Vec::with_capacity(mats.len() + 1);
    if ctl.smoother.uses_dic() {
        r_d.push(dic_reciprocal_diag(a));
        for m in &mats {
            r_d.push(dic_reciprocal_diag(m));
        }
    } else {
        r_d.resize(mats.len() + 1, Vec::new());
    }
    let h = Hierarchy { mats, symmetric: sym, r_d };
    let scale_corr = ctl.scale_correction.unwrap_or(sym);

    let apsi = a.multiply(psi);
    let norm = norm_factor(a, psi, source, &apsi);
    let mut finest_residual: Vec<f64> = (0..psi.len()).map(|i| source[i] - apsi[i]).collect();
    let initial = g_sum_mag(&finest_residual) / norm;
    let mut perf = GamgPerformance { initial_residual: initial, final_residual: initial, iterations: 0, cycles: Vec::new() };
    if ctl.min_iter > 0 || !converged(perf.final_residual, initial, ctl.tolerance, ctl.rel_tol) {
        loop {
            v_cycle(&h, agg, a, ctl, scale_corr, psi, source, &finest_residual);
            let apsi = a.multiply(psi);
            finest_residual = (0..psi.len()).map(|i| source[i] - apsi[i]).collect();
            perf.final_residual = g_sum_mag(&finest_residual) / norm;
            perf.cycles.push(perf.final_residual);
            perf.iterations += 1;
            let go_on = (perf.iterations < ctl.max_iter && !converged(perf.final_residual, initial, ctl.tolerance, ctl.rel_tol)) || perf.iterations < ctl.min_iter;
            if !go_on {
                break;
            }
        }
    }
    Ok(perf)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `nx` by `ny` 5-point Laplacian on an OpenFOAM-ordered grid (faces in upper-triangular order), Dirichlet-like diagonal.
    fn grid(nx: usize, ny: usize) -> (LduMatrix, Vec<[f64; 3]>) {
        let n = nx * ny;
        let mut owner = Vec::new();
        let mut nbr = Vec::new();
        let mut sf = Vec::new();
        for c in 0..n {
            let (i, j) = (c % nx, c / nx);
            if i + 1 < nx {
                owner.push(c);
                nbr.push(c + 1);
                sf.push([1.0, 0.0, 0.0]);
            }
            if j + 1 < ny {
                owner.push(c);
                nbr.push(c + nx);
                sf.push([0.0, 1.0, 0.0]);
            }
        }
        let mut m = LduMatrix::new(n, owner, nbr);
        for f in 0..m.n_internal_faces {
            m.upper[f] = -1.0;
            m.lower[f] = -1.0;
            m.diag[m.owner[f]] += 1.0;
            m.diag[m.neighbour[f]] += 1.0;
        }
        for c in 0..n {
            m.diag[c] += 0.1;
        }
        (m, sf)
    }

    #[test]
    fn agglomeration_covers_every_cell_and_coarse_matrix_conserves_the_row_sums() {
        let (m, sf) = grid(12, 10);
        let w = face_area_pair_weights(&sf);
        let mut fwd = true;
        let ctl = GamgControls::default();
        let agg = pair_agglomerate(m.n_cells, &m.owner, &m.neighbour, &w, &ctl, &mut fwd).expect("agglomerate");
        assert!(agg.levels.len() >= 2, "levels {}", agg.levels.len());
        // `forward_` toggled once per attempted level (created + the rejected one)
        assert_eq!(fwd, (agg.levels.len() + 1) % 2 == 0, "one toggle per attempted level");
        let mut prev = m.clone();
        for lvl in &agg.levels {
            assert!(lvl.restrict.iter().all(|&c| c < lvl.n_cells));
            let c = coarsen_matrix(&prev, lvl);
            let s = |x: &LduMatrix| x.multiply(&vec![1.0; x.n_cells]).iter().sum::<f64>();
            assert!((s(&c) - s(&prev)).abs() < 1e-9 * s(&prev).abs(), "row sums {} {}", s(&c), s(&prev));
            assert!(c.owner.iter().zip(&c.neighbour).all(|(o, n)| o < n));
            prev = c;
        }
    }

    #[test]
    fn gamg_solves_the_grid_and_the_second_agglomeration_starts_backwards() {
        let (m, sf) = grid(16, 16);
        let w = face_area_pair_weights(&sf);
        let ctl = GamgControls { tolerance: 1e-10, ..GamgControls::default() };
        let mut fwd = true;
        let agg = pair_agglomerate(m.n_cells, &m.owner, &m.neighbour, &w, &ctl, &mut fwd).expect("agg");
        let b: Vec<f64> = (0..m.n_cells).map(|c| 1.0 + (c % 7) as f64).collect();
        let mut x = vec![0.0; m.n_cells];
        let perf = gamg_dic_solve(&m, &agg, &ctl, &b, &mut x).expect("solve");
        assert!(perf.final_residual < 1e-10 && perf.iterations > 0 && perf.iterations < 60, "{perf:?}");
        assert_eq!(perf.cycles.len(), perf.iterations);
        let r = m.residual(&x, &b);
        assert!(r.iter().all(|v| v.abs() < 1e-6));
        // a converged guess exits with zero cycles
        let again = gamg_dic_solve(&m, &agg, &ctl, &b, &mut x).expect("solve");
        assert_eq!(again.iterations, 0);
        // the toggle state is the caller's: a second agglomeration from the left-over state is a different one
        let mut fwd2 = fwd;
        let agg2 = pair_agglomerate(m.n_cells, &m.owner, &m.neighbour, &w, &ctl, &mut fwd2).expect("agg2");
        let mut fwd3 = true;
        let agg3 = pair_agglomerate(m.n_cells, &m.owner, &m.neighbour, &w, &ctl, &mut fwd3).expect("agg3");
        assert_eq!(agg3, agg);
        assert_eq!(agg2.n_fine_cells, agg.n_fine_cells);
        assert!(!agg2.levels.is_empty());
    }

    #[test]
    fn unsupported_selections_are_refused_by_name() {
        let (m, sf) = grid(8, 8);
        let w = face_area_pair_weights(&sf);
        let mut fwd = true;
        let bad = GamgControls { merge_levels: 2, ..GamgControls::default() };
        assert!(pair_agglomerate(m.n_cells, &m.owner, &m.neighbour, &w, &bad, &mut fwd).unwrap_err().contains("mergeLevels"));
        let ctl = GamgControls::default();
        let agg = pair_agglomerate(m.n_cells, &m.owner, &m.neighbour, &w, &ctl, &mut fwd).expect("agg");
        let mut asym = m.clone();
        asym.lower[0] = -0.5;
        let b = vec![1.0; m.n_cells];
        let mut x = vec![0.0; m.n_cells];
        assert!(gamg_dic_solve(&asym, &agg, &ctl, &b, &mut x).unwrap_err().contains("symmetric matrix"));
        // gh:#1936: an asymmetric matrix runs with GaussSeidel (coarse matrices with `lower`, PBiCGStab/DILU coarsest, no scaling)
        let mut asym = m.clone();
        for f in 0..asym.n_internal_faces {
            asym.lower[f] *= 1.3;
            let u = asym.neighbour[f];
            asym.diag[u] += 0.3;
        }
        let gs = GamgControls { smoother: GamgSmoother::GaussSeidel, tolerance: 1e-9, ..GamgControls::default() };
        let agg = pair_agglomerate(m.n_cells, &m.owner, &m.neighbour, &w, &gs, &mut true).expect("agg");
        let perf = gamg_dic_solve(&asym, &agg, &gs, &b, &mut x).expect("asymmetric solve");
        assert!(perf.final_residual < 1e-9 && perf.iterations < 100, "{perf:?}");
        let r = asym.residual(&x, &b);
        assert!(r.iter().all(|v| v.abs() < 1e-5));
        let mut y = vec![0.0; m.n_cells];
        assert!(pbicgstab_dilu(&asym, &b, &mut y, 1e-12, 0.0, 1000, 0) > 0);
        assert!(asym.residual(&y, &b).iter().all(|v| v.abs() < 1e-8));
    }

    /// gh:#1936: every ported smoother, with and without pre-sweeps, converges the grid problem; the smoothers differ from one another
    /// (a wrong smoother changes the cycle); `GaussSeidel` of a diagonal matrix is the exact solve; `from_word` refuses the rest.
    #[test]
    fn every_ported_smoother_converges_and_they_differ() {
        let (m, sf) = grid(16, 16);
        let w = face_area_pair_weights(&sf);
        let b: Vec<f64> = (0..m.n_cells).map(|c| 1.0 + (c % 7) as f64).collect();
        let mut first_cycle = Vec::new();
        for kind in [GamgSmoother::Dic, GamgSmoother::GaussSeidel, GamgSmoother::DicGaussSeidel, GamgSmoother::SymGaussSeidel] {
            for n_pre in [0usize, 1] {
                let ctl = GamgControls { tolerance: 1e-10, smoother: kind, n_pre_sweeps: n_pre, ..GamgControls::default() };
                let mut fwd = true;
                let agg = pair_agglomerate(m.n_cells, &m.owner, &m.neighbour, &w, &ctl, &mut fwd).expect("agg");
                let mut x = vec![0.0; m.n_cells];
                let perf = gamg_dic_solve(&m, &agg, &ctl, &b, &mut x).expect("solve");
                assert!(perf.final_residual < 1e-10 && perf.iterations < 80, "{kind:?} pre {n_pre}: {perf:?}");
                first_cycle.push(perf.cycles[0]);
            }
        }
        for i in 0..first_cycle.len() {
            for j in 0..i {
                assert!(first_cycle[i] != first_cycle[j], "smoothers/pre-sweeps {i} and {j} give the same first cycle");
            }
        }
        let mut d = LduMatrix::new(3, vec![0], vec![1]);
        d.diag = vec![2.0, 4.0, 5.0];
        d.upper = vec![0.0];
        d.lower = vec![0.0];
        let mut x = vec![0.0; 3];
        gauss_seidel_smooth(&d, &mut x, &[2.0, 8.0, 5.0], 1);
        assert_eq!(x, vec![1.0, 2.0, 1.0]);
        let mut y = vec![0.0; 3];
        sym_gauss_seidel_smooth(&d, &mut y, &[2.0, 8.0, 0.0], 1);
        assert_eq!(y[..2], [1.0, 2.0]);
        assert_eq!(GamgSmoother::from_word("DICGaussSeidel"), Some(GamgSmoother::DicGaussSeidel));
        assert_eq!(GamgSmoother::from_word("DILU"), None);
    }
}
