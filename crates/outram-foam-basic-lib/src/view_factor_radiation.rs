// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com), ESI v2506
// Copyright (C) 2011-2017 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
//! `viewFactor` surface-to-surface radiation (ESI v2506), serial direct-solver path
//! (gh:#1529, under gh:#1379 and epic gh:#1376).
//!
//! # What is ported
//!
//! | Upstream (ESI v2506 `src/thermophysicalModels/radiation/`) | Here |
//! |---|---|
//! | `radiationModels/viewFactor/viewFactor.C` `initialise` (dense `Fmatrix_`, smoothing), `insertMatrixElements` | [`ViewFactorModel::new`] |
//! | `viewFactor::calculate`, `useDirectSolver true`, variable and `constantEmissivity` branches | [`ViewFactorModel::solve`] |
//! | coarse-face area averaging of `T^4`, emissivity, `Hoi` and the back-distribution of `qr` | [`ViewFactorModel::solve`] |
//! | `viewFactor::Ru`, `viewFactor::Rp` (both identically zero) | [`ViewFactorModel::ru`], [`ViewFactorModel::rp`] |
//! | `greyDiffusiveViewFactorFixedValueFvPatchScalarField` (`qro`, no solar load) | [`GreyDiffusiveViewFactorPatch`] |
//! | `Foam::LUDecompose` / `LUBacksubstitute` (`LUsolve`) | `matrix::SquareMatrix::lu_decompose` / `lu_back_substitute` (existing, unchanged) |
//! | the ASCII `F`, `globalFaceFaces`, `finalAgglom` files written by `viewFactorsGen` / `faceAgglomerate` | [`parse_scalar_list_list`], [`parse_label_list_list`] |
//!
//! NOT ported (gh:#1529 ledger): ~~the view-factor GENERATOR (`viewFactorsGen`,
//! `faceAgglomerate`, `singleCellFvMesh`: ~1,555 + 600 lines on `distributedTriSurfaceMesh`;
//! this module reads their output)~~ **CORRECTED 2026-11-14 (gh:#1540)**: `viewFactorsGen` (serial, 3-D, no
//! agglomeration) IS ported, see [`crate::view_factor_generation`]; `faceAgglomerate`,
//! `singleCellFvMesh` and `pairPatchAgglomeration` are still not (this module reads their
//! output). Also not ported: the iterative `useDirectSolver false` branch
//! (needs `lduPrimitiveMesh`/processor interfaces; serial runs use the direct branch by
//! default), several bands (`nBands > 1`), `solarLoad`, and parallel (`mapDistribute`).
//! The Rust solver takes `nBands = 1`.
//!
//! # Model
//!
//! With `F` the view-factor matrix of the coarse faces (rows of the visible faces only; a
//! row sum below 1 is an open enclosure), `E` the face emissivity, `sigma` the ESI
//! Stefan-Boltzmann constant and `Ho` the external flux `qro` of the
//! `greyDiffusiveRadiationViewFactor` BC, upstream solves
//!
//! ```text
//!   sum_j [ delta_ij / E_j - (1/E_j - 1) F_ij ] q_j
//!        = -sigma T_i^4 + Ho_i + sum_{j != i} F_ij sigma T_j^4
//! ```
//!
//! (`F_ii = 0`), for the coarse-face flux `q`; it is then copied to every fine face of the
//! coarse face. `q` is the NET flux INTO the wall (negative = leaving the wall into the
//! enclosure); for `Ho = 0` it is minus the Modest (Radiative Heat Transfer, 3rd ed.,
//! Eq. 5.22) net outgoing flux. The radiosity is therefore
//! `J_i = sigma T_i^4 + (1 - E_i)/E_i q_i` ([`ViewFactorSolution::radiosity`]).
//!
//! # `constantEmissivity` caches the LU factors on the FIRST call (upstream behaviour)
//!
//! With `constantEmissivity true` upstream builds and factorises `C` on the first
//! `calculate()` and reuses it for every later call, whatever the emissivities are then.
//! This port does the same (`lu` is filled once) and says so here rather than "fixing" it;
//! `Ho` and `T` still enter every solve.
//!
//! # Defaults
//!
//! Opt-in: nothing in the solvers calls this by default (the model is a new capability).
//! Units: SI (T in K, areas in m2, fluxes in W/m2).

use crate::matrix::square_matrix::SquareMatrix;
use crate::radiation::stefan_boltzmann_esi_v2506;

/// Error from reading a view-factor file or assembling a model.
#[derive(Debug, Clone, PartialEq)]
pub enum ViewFactorError {
    /// The text is not an OpenFOAM ASCII list-of-lists.
    Parse(String),
    /// Inputs of inconsistent sizes.
    Shape(String),
}

impl std::fmt::Display for ViewFactorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViewFactorError::Parse(m) => write!(f, "view-factor parse error: {m}"),
            ViewFactorError::Shape(m) => write!(f, "view-factor shape error: {m}"),
        }
    }
}

impl std::error::Error for ViewFactorError {}

/// Tokens of an OpenFOAM ASCII file with the `FoamFile` header, `//` and `/* */`
/// comments removed and `(` `)` split out.
fn foam_tokens(text: &str) -> Vec<String> {
    // strip comments
    let mut clean = String::with_capacity(text.len());
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i += 2;
        } else if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'/' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else {
            clean.push(b[i] as char);
            i += 1;
        }
    }
    // drop the FoamFile { ... } header
    let body: &str = match clean.find("FoamFile") {
        Some(p) => match clean[p..].find('}') {
            Some(q) => &clean[p + q + 1..],
            None => &clean,
        },
        None => &clean,
    };
    let spaced = body.replace('(', " ( ").replace(')', " ) ");
    spaced.split_whitespace().map(str::to_string).collect()
}

/// Parse an ASCII `List<List<scalar>>` (`scalarListIOList`, the `F` file) exactly: every
/// number goes through `str::parse::<f64>`, which is correctly rounded like glibc `strtod`,
/// so a file written with `writePrecision 17` round-trips bit for bit.
///
/// # Errors
/// [`ViewFactorError::Parse`] when the list structure is malformed.
pub fn parse_scalar_list_list(text: &str) -> Result<Vec<Vec<f64>>, ViewFactorError> {
    parse_list_list(text, |s| s.parse::<f64>().map_err(|e| e.to_string()))
}

/// Parse an ASCII `List<List<label>>` (`labelListIOList`: `globalFaceFaces`,
/// `finalAgglom`).
///
/// # Errors
/// [`ViewFactorError::Parse`] when the list structure is malformed.
pub fn parse_label_list_list(text: &str) -> Result<Vec<Vec<usize>>, ViewFactorError> {
    parse_list_list(text, |s| s.parse::<usize>().map_err(|e| e.to_string()))
}

fn parse_list_list<T>(
    text: &str,
    conv: impl Fn(&str) -> Result<T, String>,
) -> Result<Vec<Vec<T>>, ViewFactorError> {
    let tok = foam_tokens(text);
    let err = |m: &str| ViewFactorError::Parse(m.to_string());
    let mut p = 0usize;
    let n: usize = tok
        .get(p)
        .ok_or_else(|| err("empty"))?
        .parse()
        .map_err(|_| err("outer length"))?;
    p += 1;
    if tok.get(p).map(String::as_str) != Some("(") {
        return Err(err("expected ( after outer length"));
    }
    p += 1;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let m: usize = tok
            .get(p)
            .ok_or_else(|| err("truncated"))?
            .parse()
            .map_err(|_| err("inner length"))?;
        p += 1;
        if tok.get(p).map(String::as_str) != Some("(") {
            return Err(err("expected ( after inner length"));
        }
        p += 1;
        let mut row = Vec::with_capacity(m);
        for _ in 0..m {
            let s = tok.get(p).ok_or_else(|| err("truncated row"))?;
            row.push(conv(s).map_err(|e| ViewFactorError::Parse(format!("`{s}`: {e}")))?);
            p += 1;
        }
        if tok.get(p).map(String::as_str) != Some(")") {
            return Err(err("expected ) closing inner list"));
        }
        p += 1;
        out.push(row);
    }
    if tok.get(p).map(String::as_str) != Some(")") {
        return Err(err("expected ) closing outer list"));
    }
    Ok(out)
}

/// One participating boundary patch (a `viewFactorWall` patch): its fine-face areas
/// `|Sf|` and the fine-to-coarse agglomeration labels (`finalAgglom[patch]`, identity for
/// no agglomeration).
#[derive(Debug, Clone)]
pub struct ViewFactorPatch {
    /// Patch name (informational).
    pub name: String,
    /// `|Sf|` of every fine face of the patch [m2].
    pub fine_areas: Vec<f64>,
    /// Agglomeration label of every fine face (`finalAgglom[patch]`).
    pub agglom: Vec<usize>,
}

/// `greyDiffusiveRadiationViewFactor` patch field: a fixed-value `qr` carrying the external
/// incident flux `qro` [W/m2] (no solar load).
#[derive(Debug, Clone)]
pub struct GreyDiffusiveViewFactorPatch {
    /// External flux `qro` per fine face [W/m2] (`qro uniform ...` in `0/qr`).
    pub qro: Vec<f64>,
    /// The patch value (`qr`), overwritten by [`ViewFactorModel::solve`].
    pub value: Vec<f64>,
}

impl GreyDiffusiveViewFactorPatch {
    /// A patch with the given `qro` and `qr = 0` (upstream: `value` default Zero).
    #[must_use]
    pub fn new(qro: Vec<f64>) -> Self {
        let n = qro.len();
        Self {
            qro,
            value: vec![0.0; n],
        }
    }

    /// `qro(bandI)` without solar load: a copy of the stored `qro`.
    #[must_use]
    pub fn qro(&self) -> Vec<f64> {
        self.qro.clone()
    }
}

/// Coarse-face bookkeeping of one patch, in `singleCellFvMesh` order: coarse face `k`
/// (the order in which agglomeration labels first appear over the fine faces) owns the
/// fine faces of label `label_of[k]`, ascending.
#[derive(Debug, Clone)]
struct CoarsePatch {
    label_of: Vec<usize>,
    coarse_to_fine: Vec<Vec<usize>>,
}

fn coarse_patch(agglom: &[usize]) -> CoarsePatch {
    let n_agglom = agglom.iter().copied().max().map_or(0, |m| m + 1);
    let mut by_label: Vec<Vec<usize>> = vec![Vec::new(); n_agglom];
    for (i, &a) in agglom.iter().enumerate() {
        by_label[a].push(i);
    }
    let mut seen = vec![false; n_agglom];
    let mut label_of = Vec::new();
    for &a in agglom {
        if !seen[a] {
            seen[a] = true;
            label_of.push(a);
        }
    }
    let coarse_to_fine = label_of.iter().map(|&a| by_label[a].clone()).collect();
    CoarsePatch {
        label_of,
        coarse_to_fine,
    }
}

/// The `viewFactor` radiation model, direct dense solver.
#[derive(Debug, Clone)]
pub struct ViewFactorModel {
    patches: Vec<ViewFactorPatch>,
    coarse: Vec<CoarsePatch>,
    n_coarse: usize,
    f: SquareMatrix,
    const_emissivity: bool,
    lu: Option<(SquareMatrix, Vec<usize>)>,
    sigma: f64,
}

/// Result of [`ViewFactorModel::solve`].
#[derive(Debug, Clone)]
pub struct ViewFactorSolution {
    /// Coarse-face net flux `q` (upstream `qNet`), in global coarse-face order [W/m2].
    pub q_coarse: Vec<f64>,
    /// Coarse-face `sigma T^4` as used (area-averaged `T^4` times `sigma`).
    pub sigma_t4_coarse: Vec<f64>,
    /// Coarse-face emissivity as used (area average).
    pub emissivity_coarse: Vec<f64>,
}

impl ViewFactorSolution {
    /// Radiosity `J_i = sigma T_i^4 + (1 - E_i)/E_i q_i` per coarse face, the Modest
    /// radiosity for `Ho = 0` (see the module docs for the sign of `q`). A black face
    /// (`E = 1`) gives `J = sigma T^4`.
    #[must_use]
    pub fn radiosity(&self) -> Vec<f64> {
        (0..self.q_coarse.len())
            .map(|i| {
                self.sigma_t4_coarse[i]
                    + (1.0 - self.emissivity_coarse[i]) / self.emissivity_coarse[i] * self.q_coarse[i]
            })
            .collect()
    }
}

impl ViewFactorModel {
    /// Assemble the dense `F` (`insertMatrixElements`, one processor) and, with
    /// `smoothing`, apply upstream's row smoothing `F_ij *= 1 - delta/(sumF + 0.001)`,
    /// `delta = sumF - 1`, rows summed in ascending `j`.
    ///
    /// `global_face_faces[i]` lists the coarse columns visible from coarse face `i` and
    /// `f_rows[i]` the matching view factors (the `globalFaceFaces` and `F` files).
    ///
    /// # Errors
    /// [`ViewFactorError::Shape`] when the row counts or lengths disagree, or a column is
    /// out of range.
    pub fn new(
        patches: Vec<ViewFactorPatch>,
        global_face_faces: &[Vec<usize>],
        f_rows: &[Vec<f64>],
        smoothing: bool,
        const_emissivity: bool,
    ) -> Result<Self, ViewFactorError> {
        let coarse: Vec<CoarsePatch> = patches.iter().map(|p| coarse_patch(&p.agglom)).collect();
        let n_coarse: usize = coarse.iter().map(|c| c.label_of.len()).sum();
        if global_face_faces.len() != n_coarse || f_rows.len() != n_coarse {
            return Err(ViewFactorError::Shape(format!(
                "{} coarse faces but {} globalFaceFaces rows and {} F rows",
                n_coarse,
                global_face_faces.len(),
                f_rows.len()
            )));
        }
        let mut f = SquareMatrix::new(n_coarse);
        for i in 0..n_coarse {
            if global_face_faces[i].len() != f_rows[i].len() {
                return Err(ViewFactorError::Shape(format!("row {i} length mismatch")));
            }
            for (k, &j) in global_face_faces[i].iter().enumerate() {
                if j >= n_coarse {
                    return Err(ViewFactorError::Shape(format!("row {i}: column {j} out of range")));
                }
                f.set(i, j, f_rows[i][k]);
            }
        }
        if smoothing {
            for i in 0..n_coarse {
                let mut sum_f = 0.0;
                for j in 0..n_coarse {
                    sum_f += f.get(i, j);
                }
                let delta = sum_f - 1.0;
                for j in 0..n_coarse {
                    let v = f.get(i, j) * (1.0 - delta / (sum_f + 0.001));
                    f.set(i, j, v);
                }
            }
        }
        Ok(Self {
            patches,
            coarse,
            n_coarse,
            f,
            const_emissivity,
            lu: None,
            sigma: stefan_boltzmann_esi_v2506(),
        })
    }

    /// Number of coarse faces (order of the dense system).
    #[must_use]
    pub fn n_coarse(&self) -> usize {
        self.n_coarse
    }

    /// The (possibly smoothed) dense view-factor matrix `F`.
    #[must_use]
    pub fn f_matrix(&self) -> &SquareMatrix {
        &self.f
    }

    /// `viewFactor::Ru`: identically zero (upstream returns a zero field).
    #[must_use]
    pub fn ru(&self, n_cells: usize) -> Vec<f64> {
        vec![0.0; n_cells]
    }

    /// `viewFactor::Rp`: identically zero.
    #[must_use]
    pub fn rp(&self, n_cells: usize) -> Vec<f64> {
        vec![0.0; n_cells]
    }

    /// One `viewFactor::calculate()` (direct solver, one band).
    ///
    /// `t` and `emissivity` hold one value per fine face of each patch (patch order as in
    /// `new`); `bcs` supplies `qro` and receives the new `qr` in `value`.
    ///
    /// # Errors
    /// [`ViewFactorError::Shape`] when the per-patch vectors do not match the patches.
    pub fn solve(
        &mut self,
        t: &[Vec<f64>],
        emissivity: &[Vec<f64>],
        bcs: &mut [GreyDiffusiveViewFactorPatch],
    ) -> Result<ViewFactorSolution, ViewFactorError> {
        let np = self.patches.len();
        if t.len() != np || emissivity.len() != np || bcs.len() != np {
            return Err(ViewFactorError::Shape("per-patch input count".to_string()));
        }
        let n = self.n_coarse;
        let mut t4 = Vec::with_capacity(n);
        let mut e = Vec::with_capacity(n);
        let mut qr_ext = Vec::with_capacity(n);
        for (pi, patch) in self.patches.iter().enumerate() {
            let nf = patch.fine_areas.len();
            if t[pi].len() != nf || emissivity[pi].len() != nf || bcs[pi].qro.len() != nf {
                return Err(ViewFactorError::Shape(format!("patch {} field size", patch.name)));
            }
            let hoi = bcs[pi].qro();
            for fines in &self.coarse[pi].coarse_to_fine {
                // `area = sum(fineSf)`, then `+= (pow4(T) sf)/area` in fine-face order
                let mut area = 0.0;
                for &fi in fines {
                    area += patch.fine_areas[fi];
                }
                let (mut a4, mut ae, mut ah) = (0.0, 0.0, 0.0);
                for &fi in fines {
                    let tt = t[pi][fi];
                    let p4 = (tt * tt) * (tt * tt);
                    a4 += (p4 * patch.fine_areas[fi]) / area;
                    ae += (emissivity[pi][fi] * patch.fine_areas[fi]) / area;
                    ah += (hoi[fi] * patch.fine_areas[fi]) / area;
                }
                t4.push(a4);
                e.push(ae);
                qr_ext.push(ah);
            }
        }

        let sigma = self.sigma;
        let mut q = vec![0.0; n];
        if !self.const_emissivity {
            let mut c = SquareMatrix::new(n);
            for i in 0..n {
                for j in 0..n {
                    let inv_ej = 1.0 / e[j];
                    let sigma_t4 = sigma * t4[j];
                    if i == j {
                        c.set(i, j, inv_ej);
                        q[i] += -sigma_t4 + qr_ext[j];
                    } else {
                        c.set(i, j, (1.0 - inv_ej) * self.f.get(i, j));
                        q[i] += self.f.get(i, j) * sigma_t4;
                    }
                }
            }
            let pivot = c.lu_decompose();
            c.lu_back_substitute(&pivot, &mut q);
        } else {
            if self.lu.is_none() {
                let mut c = SquareMatrix::new(n);
                for i in 0..n {
                    for j in 0..n {
                        let inv_ej = 1.0 / e[j];
                        if i == j {
                            c.set(i, j, inv_ej);
                        } else {
                            c.set(i, j, (1.0 - inv_ej) * self.f.get(i, j));
                        }
                    }
                }
                let pivot = c.lu_decompose();
                self.lu = Some((c, pivot));
            }
            for i in 0..n {
                for j in 0..n {
                    let sigma_t4 = sigma * t4[j];
                    if i == j {
                        q[i] += -sigma_t4 + qr_ext[j];
                    } else {
                        q[i] += self.f.get(i, j) * sigma_t4;
                    }
                }
            }
            if let Some((lu, pivot)) = &self.lu {
                lu.lu_back_substitute(pivot, &mut q);
            }
        }

        // back-distribute to the fine faces
        let mut glob = 0usize;
        for (pi, cp) in self.coarse.iter().enumerate() {
            for fines in &cp.coarse_to_fine {
                for &fi in fines {
                    bcs[pi].value[fi] = q[glob];
                }
                glob += 1;
            }
        }
        let sigma_t4_coarse = t4.iter().map(|x| sigma * x).collect();
        Ok(ViewFactorSolution {
            q_coarse: q,
            sigma_t4_coarse,
            emissivity_coarse: e,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_patch(n: usize) -> Vec<ViewFactorPatch> {
        vec![ViewFactorPatch {
            name: "p".into(),
            fine_areas: vec![1.0; n],
            agglom: (0..n).collect(),
        }]
    }

    #[test]
    fn parse_round_trip() {
        let t = "FoamFile { version 2.0; format ascii; class scalarListList; object F; }\n// c\n2\n(\n2\n(\n0.5\n1e-3\n)\n1\n(\n0.25\n)\n)\n";
        let v = parse_scalar_list_list(t).expect("parse");
        assert_eq!(v, vec![vec![0.5, 1e-3], vec![0.25]]);
        let l = parse_label_list_list("2\n(\n2\n(\n0\n1\n)\n1\n(\n7\n)\n)\n").expect("parse");
        assert_eq!(l, vec![vec![0, 1], vec![7]]);
        assert!(parse_scalar_list_list("2\n(\n").is_err());
    }

    #[test]
    fn two_black_plates_exchange() {
        // two facing black faces, F12 = F21 = 1: q1 = -sigma (T1^4 - T2^4)
        let p = vec![
            ViewFactorPatch { name: "a".into(), fine_areas: vec![1.0], agglom: vec![0] },
            ViewFactorPatch { name: "b".into(), fine_areas: vec![1.0], agglom: vec![0] },
        ];
        let mut m = ViewFactorModel::new(p, &[vec![1], vec![0]], &[vec![1.0], vec![1.0]], false, false)
            .expect("model");
        let mut bcs = vec![
            GreyDiffusiveViewFactorPatch::new(vec![0.0]),
            GreyDiffusiveViewFactorPatch::new(vec![0.0]),
        ];
        let s = m
            .solve(&[vec![1000.0], vec![500.0]], &[vec![1.0], vec![1.0]], &mut bcs)
            .expect("solve");
        let sg = stefan_boltzmann_esi_v2506();
        let want = -sg * (1000f64.powi(4) - 500f64.powi(4));
        assert!((bcs[0].value[0] - want).abs() < 1e-9 * want.abs());
        assert!((s.q_coarse[0] + s.q_coarse[1]).abs() < 1e-9 * want.abs());
        assert_eq!(m.ru(3), vec![0.0; 3]);
        assert_eq!(m.rp(2), vec![0.0; 2]);
        assert_eq!(m.n_coarse(), 2);
        assert_eq!(m.f_matrix().get(0, 1), 1.0);
        let j = s.radiosity();
        assert!((j[0] - sg * 1000f64.powi(4)).abs() < 1e-9 * sg * 1e12);
    }

    #[test]
    fn shape_errors_and_cache() {
        assert!(ViewFactorModel::new(one_patch(2), &[vec![]], &[vec![]], false, false).is_err());
        let mut m = ViewFactorModel::new(one_patch(1), &[vec![]], &[vec![]], true, true).expect("m");
        let mut bcs = vec![GreyDiffusiveViewFactorPatch::new(vec![0.0])];
        assert!(m.solve(&[], &[], &mut bcs).is_err());
        // single self-only face: q = -sigma T^4 (nothing to exchange with)
        let s = m.solve(&[vec![300.0]], &[vec![0.5]], &mut bcs).expect("s");
        let s2 = m.solve(&[vec![600.0]], &[vec![0.9]], &mut bcs).expect("s2");
        // emissivity of the second call is ignored by the cached LU (diag 1/0.5 = 2)
        let sg = stefan_boltzmann_esi_v2506();
        assert!((s.q_coarse[0] + 0.5 * sg * 300f64.powi(4)).abs() < 1e-9);
        assert!((s2.q_coarse[0] + 0.5 * sg * 600f64.powi(4)).abs() < 1e-6);
    }

    #[test]
    fn agglomeration_order_is_first_appearance() {
        let cp = coarse_patch(&[1, 1, 0, 2, 0]);
        assert_eq!(cp.label_of, vec![1, 0, 2]);
        assert_eq!(cp.coarse_to_fine, vec![vec![0, 1], vec![2, 4], vec![3]]);
    }
}
