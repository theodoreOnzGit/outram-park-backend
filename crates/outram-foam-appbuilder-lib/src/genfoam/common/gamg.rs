// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! OpenFOAM-v2506 `GAMGSolver` with the `faceAreaPair` agglomeration and the `DIC` smoother, translated line by line
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
//! | coarsest level | `GAMGSolver.C:320-330` (`PCG`/`DIC` at the outer `tolerance`/`relTol`), `GAMGSolverSolve.C:PCGsolverDict` | [`super::pcg::pcg_dic`] |
//!
//! Defaults (v2506 `GAMGSolver.C` ctor): `nPreSweeps 0`, `preSweepsLevelMultiplier 1`, `maxPreSweeps 4`, `nPostSweeps 2`,
//! `postSweepsLevelMultiplier 1`, `maxPostSweeps 4`, `nFinestSweeps 2`, `cacheAgglomeration true`, `interpolateCorrection false`,
//! `scaleCorrection = matrix.symmetric()`, `directSolveCoarsest false`; agglomeration `agglomerator faceAreaPair`, `mergeLevels 1`,
//! `nCellsInCoarsestLevel 10`; solver `tolerance 1e-6`, `relTol 0`, `maxIter 1000`, `minIter 0`.
//!
//! # NOT ported (refused by name, see [`GamgControls::check_supported`])
//!
//! processor agglomeration and every parallel path; coupled interfaces (cyclic, AMI, processor); asymmetric matrices (a symmetric
//! matrix is one whose `lower` equals `upper` bit for bit); `nPreSweeps != 0`; `interpolateCorrection true`; `directSolveCoarsest
//! true`; `mergeLevels != 1`; `cacheAgglomeration false`; agglomerators other than `faceAreaPair`; smoothers other than `DIC`.
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
use outram_foam_basic_lib::prelude::LduMatrix;
#[allow(unused_imports)]
use petir::mathf::RealMath;

/// `GREAT` for doubles (`doubleScalarGreat`).
const GREAT: f64 = 1.0e15;
/// `pTraits<double>::vsmall` (`stabilise` in `GAMGSolverScale.C`).
const VSMALL: f64 = 1e-300;

/// The `solvers.p_rgh` entries of a `GAMG` solver (defaults of v2506).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GamgControls {
    /// `tolerance` (`lduMatrix::solver` default `1e-6`).
    pub tolerance: f64,
    /// `relTol` (default 0).
    pub rel_tol: f64,
    /// `maxIter` (default 1000).
    pub max_iter: usize,
    /// `minIter` (default 0).
    pub min_iter: usize,
    /// `nPreSweeps` (default 0; other values are refused).
    pub n_pre_sweeps: usize,
    /// `nPostSweeps` (default 2).
    pub n_post_sweeps: usize,
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
}

impl Default for GamgControls {
    fn default() -> Self {
        Self {
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
        }
    }
}

impl GamgControls {
    /// Refuse, by name, the selections that are not ported.
    ///
    /// # Errors
    /// Names the first unsupported selection.
    pub fn check_supported(&self) -> Result<(), String> {
        if self.n_pre_sweeps != 0 {
            return Err("GAMG nPreSweeps != 0 is not ported (default 0)".to_string());
        }
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

/// The coarse matrices of `a` over `agg` (`matrixLevels_` of `GAMGSolver`: entry `i` is the matrix of coarse level `i + 1`).
#[must_use]
pub fn coarse_matrices(a: &LduMatrix, agg: &GamgAgglomeration) -> Vec<LduMatrix> {
    let mut mats: Vec<LduMatrix> = Vec::with_capacity(agg.levels.len());
    for (i, lvl) in agg.levels.iter().enumerate() {
        let coarse = coarsen_matrix(if i == 0 { a } else { &mats[i - 1] }, lvl);
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
    /// `DICSmoother::rD_` of the fine matrix and of each coarse one (`smoothers[0]`, `smoothers[i + 1]`).
    r_d: Vec<Vec<f64>>,
}

/// `GAMGSolver::Vcycle` (`GAMGSolverSolve.C:156-380`) with `nPreSweeps = 0` and `interpolateCorrection = false`.
fn v_cycle(h: &Hierarchy, agg: &GamgAgglomeration, a: &LduMatrix, ctl: &GamgControls, scale_corr: bool, psi: &mut [f64], source: &[f64], finest_residual: &[f64]) {
    let coarsest = h.mats.len() - 1;
    let mut sources: Vec<Vec<f64>> = Vec::with_capacity(h.mats.len());
    // restrict the finest-grid residual, then going to coarser levels
    sources.push(restrict_field(finest_residual, &agg.levels[0].restrict, agg.levels[0].n_cells));
    for leveli in 0..coarsest {
        let next = restrict_field(&sources[leveli], &agg.levels[leveli + 1].restrict, agg.levels[leveli + 1].n_cells);
        sources.push(next);
    }
    // coarsest level: PCG/DIC at the outer tolerance and relTol (`PCGsolverDict`), default maxIter 1000, minIter 0
    let mut corr: Vec<Vec<f64>> = vec![Vec::new(); h.mats.len()];
    let mut cc = vec![0.0; h.mats[coarsest].diag.len()];
    let _ = pcg_dic(&h.mats[coarsest], &sources[coarsest], &mut cc, ctl.tolerance, ctl.rel_tol, 1000, 0);
    corr[coarsest] = cc;
    // smoothing and prolongation going to finer levels
    for leveli in (0..coarsest).rev() {
        let map = &agg.levels[leveli + 1].restrict;
        let mut c: Vec<f64> = (0..map.len()).map(|i| corr[leveli + 1][map[i]]).collect();
        if scale_corr && leveli < coarsest - 1 {
            scale(&mut c, &h.mats[leveli], &sources[leveli]);
        }
        let n = (ctl.n_post_sweeps + ctl.post_sweeps_level_multiplier * leveli).min(ctl.max_post_sweeps);
        dic_smooth(&h.mats[leveli], &h.r_d[leveli + 1], &mut c, &sources[leveli], n);
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
    dic_smooth(a, &h.r_d[0], psi, source, ctl.n_finest_sweeps);
}

/// `GAMGSolver::solve` (`GAMGSolverSolve.C:36-153`) for a symmetric `a` (`lower == upper`), with the `DIC` smoother and the `faceAreaPair`
/// agglomeration `agg`. `psi` is the initial guess and receives the solution.
///
/// # Errors
/// An asymmetric matrix, an unsupported control, or no coarse level (upstream: `FatalError`, "No coarse levels created").
pub fn gamg_dic_solve(a: &LduMatrix, agg: &GamgAgglomeration, ctl: &GamgControls, source: &[f64], psi: &mut [f64]) -> Result<GamgPerformance, String> {
    ctl.check_supported()?;
    if a.lower.iter().zip(&a.upper).any(|(l, u)| l.to_bits() != u.to_bits()) {
        return Err("GAMG: an asymmetric matrix is not ported (lower != upper)".to_string());
    }
    if agg.levels.is_empty() {
        return Err("GAMG: no coarse levels created (matrix too small for GAMG or nCellsInCoarsestLevel too large)".to_string());
    }
    // the constructor: agglomerate the matrices, build the smoothers
    let mats = coarse_matrices(a, agg);
    let mut r_d = Vec::with_capacity(mats.len() + 1);
    r_d.push(dic_reciprocal_diag(a));
    for m in &mats {
        r_d.push(dic_reciprocal_diag(m));
    }
    let h = Hierarchy { mats, r_d };
    let scale_corr = ctl.scale_correction.unwrap_or(true);

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
        let bad = GamgControls { n_pre_sweeps: 1, ..GamgControls::default() };
        assert!(pair_agglomerate(m.n_cells, &m.owner, &m.neighbour, &w, &bad, &mut fwd).unwrap_err().contains("nPreSweeps"));
        let bad = GamgControls { merge_levels: 2, ..GamgControls::default() };
        assert!(pair_agglomerate(m.n_cells, &m.owner, &m.neighbour, &w, &bad, &mut fwd).unwrap_err().contains("mergeLevels"));
        let ctl = GamgControls::default();
        let agg = pair_agglomerate(m.n_cells, &m.owner, &m.neighbour, &w, &ctl, &mut fwd).expect("agg");
        let mut asym = m.clone();
        asym.lower[0] = -0.5;
        let b = vec![1.0; m.n_cells];
        let mut x = vec![0.0; m.n_cells];
        assert!(gamg_dic_solve(&asym, &agg, &ctl, &b, &mut x).unwrap_err().contains("asymmetric"));
    }
}
