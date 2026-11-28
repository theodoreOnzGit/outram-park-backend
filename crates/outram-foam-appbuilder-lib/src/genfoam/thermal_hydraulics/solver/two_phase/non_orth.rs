// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OpenFOAM v2506 (ESI), (C) 2011-2016 OpenFOAM Foundation, 2017-2025 OpenCFD Ltd., GPL-3.0:
//   src/finiteVolume/finiteVolume/snGradSchemes/correctedSnGrad/{correctedSnGrad.C, correctedSnGrads.C}
//   src/finiteVolume/finiteVolume/laplacianSchemes/gaussLaplacianScheme/gaussLaplacianSchemes.C (declareFvmLaplacianScalarGamma)
//   src/finiteVolume/finiteVolume/snGradSchemes/snGradScheme/snGradScheme.C (sndGrad: snGrad(vf, deltaCoeffs) + correction(vf))
//   src/finiteVolume/fvMesh/fvGeometryScheme/basic/basicFvGeometryScheme.C (nonOrthCorrectionVectors)
//   src/finiteVolume/interpolation/surfaceInterpolation/surfaceInterpolationScheme/surfaceInterpolationScheme.C (dotInterpolate)
//   src/finiteVolume/finiteVolume/fvc/fvcSurfaceIntegrate.C (fvc::div of a surface field)
// and GeN-Foam (Generalized Nuclear Foam), https://gitlab.com/foam-for-nuclear/GeN-Foam, commit 652b3da, (C) 2015-2022 EPFL,
// GPL-3.0: src/classes/thermalHydraulics/solvers/onePhase/include/equations/{UEqn_1p.H, EEqn_1p.H, pEqnf_1p.H,
// pEqnRoutines/solveP_rgh_1p.H, pEqnRoutines/calcPhiSbyA_1p.H}, src/classes/thermalHydraulics/turbulenceModels/porousKEpsilon.C.
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! The explicit non-orthogonal correction of `Gauss linear corrected` / `snGradSchemes corrected` on the GeN-Foam regions'
//! [`Geometry`] (gh:#1991, README round 88).
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ESI v2506 `gaussLaplacianScheme<Type, scalar>::fvmLaplacian` with a `corrected` snGrad builds the same implicit matrix as
//! `uncorrected` (`nonOrthDeltaCoeffs`) and then
//!
//! ```text
//! gammaMagSf         = gamma magSf
//! correction(vf)     = linear.dotInterpolate(nonOrthCorrectionVectors, gaussGrad(vf))   (internal faces: k & (w (g_P - g_N) + g_N))
//! faceFluxCorrection = gammaMagSf correction(vf)                                        (kept by the matrix when vf is fluxRequired)
//! source            -= V fvc::div(faceFluxCorrection)                                   (fvc::div = surfaceIntegrate: sum, then / V)
//! ```
//!
//! `nonOrthCorrectionVectors` are zero on every non-coupled patch, so the correction lives on internal faces only; a mesh with
//! coupled (cyclic) patches is refused by the reader. A vector field is corrected component by component (`correctedSnGrads.C`
//! specialises the vector `correction` to `fullGradCorrection` of the vector field, whose tensor gradient is per component the
//! scalar one). `fvc::snGrad(vf)` with `corrected` is `nonOrthDeltaCoeffs (N - P)` PLUS `correction(vf)` (`snGradScheme::sndGrad`).
//!
//! What is reused: the correction vectors are `outram_foam_basic_lib::fv_operators::fvc::non_orth_correction_vectors` (upstream's
//! `unitArea - delta nonOrthDeltaCoeffs`, gh:#1362), the gradient is [`super::tensor_laplacian::grad_component`] (upstream's
//! `gaussGrad`: `interp_linear_openfoam`, `/= V`; round 64). Only the `dotInterpolate`, the `gammaMagSf` product and the `V div` are
//! here. The gradient reads the field's patch values AS STORED: the caller passes the values upstream holds when it builds the
//! matrix (not re-evaluated).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use alloc::vec;
use alloc::vec::Vec;

use outram_foam_basic_lib::fv_operators::fvc::non_orth_correction_vectors;
use outram_foam_basic_lib::primitives::Vector3;

use super::fv::{Bc, Geometry, Scalar};
use super::tensor_laplacian::grad_component;

/// The `snGrad` scheme of a `laplacianSchemes` or `snGradSchemes` entry (`Gauss linear <scheme>` / `<scheme>`).
///
/// The case selects it; [`Self::Corrected`] is upstream's correct form on a non-orthogonal mesh and what the shipped
/// `3D_HTR-10` selects. [`Self::Uncorrected`] is upstream's `uncorrected` (the only scheme the port read before round 88), kept as
/// the named variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnGradScheme {
    /// `corrected`: `nonOrthDeltaCoeffs` implicit part plus the explicit `fullGradCorrection`.
    Corrected,
    /// `uncorrected`: the `nonOrthDeltaCoeffs` part alone.
    Uncorrected,
}

impl SnGradScheme {
    /// The scheme named by the last word of a `default` entry (`Gauss linear corrected` or `corrected`), `None` when it is
    /// neither `corrected` nor `uncorrected`.
    #[must_use]
    pub fn from_word(w: &str) -> Option<Self> {
        match w {
            "corrected" => Some(Self::Corrected),
            "uncorrected" => Some(Self::Uncorrected),
            _ => None,
        }
    }
}

/// The words of a dictionary's `default` entry (`Gauss linear corrected` -> three words), empty when absent.
#[cfg(feature = "std")]
fn default_words(d: &outram_foam_basic_lib::io::FoamDict) -> Vec<alloc::string::String> {
    use outram_foam_basic_lib::io::{FoamEntry, FoamValue};
    match d.get("default") {
        Some(FoamEntry::Word(w)) => vec![w.clone()],
        Some(FoamEntry::Tokens(vs)) => vs
            .iter()
            .filter_map(|v| match v {
                FoamValue::Word(w) => Some(w.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// `(laplacianSchemes, snGradSchemes)` of a region's `fvSchemes` from their `default` entries: `Gauss linear corrected|uncorrected`
/// and `corrected|uncorrected` (README round 88). With either `corrected`, `gradSchemes.default` must be `Gauss linear` (the
/// gradient the correction is built from; [`super::tensor_laplacian::grad_component`] is that scheme).
///
/// # Errors
/// A message naming the entry when a block is missing or selects a scheme that is not ported (`limited`, `orthogonal`, another
/// interpolation than `linear`, another gradient than `Gauss linear` under `corrected`).
#[cfg(feature = "std")]
pub fn read_schemes(schemes: &outram_foam_basic_lib::io::FoamDict) -> Result<(SnGradScheme, SnGradScheme), alloc::string::String> {
    use alloc::format;
    let lap = match schemes.get_dict("laplacianSchemes") {
        Some(d) => default_words(d),
        None => return Err("fvSchemes: missing sub-dictionary `laplacianSchemes`".into()),
    };
    let laplacian = match lap.as_slice() {
        [a, b, c] if a == "Gauss" && b == "linear" => match SnGradScheme::from_word(c) {
            Some(s) => s,
            None => return Err(format!("fvSchemes.laplacianSchemes.default = {lap:?} is not ported (`Gauss linear corrected` and `Gauss linear uncorrected`)")),
        },
        _ => return Err(format!("fvSchemes.laplacianSchemes.default = {lap:?} is not ported (`Gauss linear corrected` and `Gauss linear uncorrected`)")),
    };
    let sng = match schemes.get_dict("snGradSchemes") {
        Some(d) => default_words(d),
        None => return Err("fvSchemes: missing sub-dictionary `snGradSchemes`".into()),
    };
    let sn_grad = match sng.as_slice() {
        [w] => match SnGradScheme::from_word(w) {
            Some(s) => s,
            None => return Err(format!("fvSchemes.snGradSchemes.default = {sng:?} is not ported (`corrected` and `uncorrected`)")),
        },
        _ => return Err(format!("fvSchemes.snGradSchemes.default = {sng:?} is not ported (`corrected` and `uncorrected`)")),
    };
    if laplacian == SnGradScheme::Corrected || sn_grad == SnGradScheme::Corrected {
        let gr = match schemes.get_dict("gradSchemes") {
            Some(d) => default_words(d),
            None => Vec::new(),
        };
        if gr.len() != 2 || gr[0] != "Gauss" || gr[1] != "linear" {
            return Err(format!("fvSchemes.gradSchemes.default = {gr:?} with a `corrected` scheme is not ported (only `Gauss linear`)"));
        }
    }
    Ok((laplacian, sn_grad))
}

/// `nonOrthCorrectionVectors` of a mesh and the operations of the `corrected` scheme on [`Geometry`] fields.
#[derive(Debug, Clone)]
pub struct NonOrthCorrection {
    /// The correction vector of every INTERNAL face (dimensionless); the non-coupled boundary vectors are zero upstream.
    pub vectors: Vec<Vector3>,
}

impl NonOrthCorrection {
    /// The correction vectors of `g`'s mesh (`basicFvGeometryScheme::nonOrthCorrectionVectors`).
    #[must_use]
    pub fn new(g: &Geometry) -> Self {
        Self { vectors: non_orth_correction_vectors(&g.mesh) }
    }

    /// `correctedSnGrad::fullGradCorrection(psi)` on every face (boundary faces 0): `k & (w (g_P - g_N) + g_N)` with
    /// `g = gaussGrad(psi)` from `psi`'s cell values and stored patch values.
    #[must_use]
    pub fn sn_grad_correction(&self, g: &Geometry, psi: &Scalar) -> Vec<f64> {
        let m = &g.mesh;
        let ni = g.n_internal();
        // `grad_component` reads `bc` only for its boundary-face gradients (constraint patches), which no internal face uses
        let comp = Scalar { c: psi.c.clone(), b: psi.b.clone(), bc: vec![Bc::Calculated; m.patches.len()] };
        let (gr, _gb) = grad_component(g, &comp);
        let mut out = vec![0.0; g.n_faces()];
        for f in 0..ni {
            let w = g.weights[f];
            let (p, n) = (&gr[m.owner[f]], &gr[m.neighbour[f]]);
            let gf = [w * (p[0] - n[0]) + n[0], w * (p[1] - n[1]) + n[1], w * (p[2] - n[2]) + n[2]];
            let k = self.vectors[f];
            out[f] = k.x * gf[0] + k.y * gf[1] + k.z * gf[2];
        }
        out
    }

    /// `faceFluxCorrection = (gamma magSf) correction(psi)` on every face (boundary faces 0), `gamma` per face as the Laplacian
    /// takes it.
    #[must_use]
    pub fn face_flux_correction(&self, g: &Geometry, gamma: &[f64], psi: &Scalar) -> Vec<f64> {
        let corr = self.sn_grad_correction(g, psi);
        let mut out = vec![0.0; g.n_faces()];
        for f in 0..g.n_internal() {
            out[f] = (gamma[f] * g.mesh.face_areas[f]) * corr[f];
        }
        out
    }

    /// The source of `-fvm::laplacian(gamma, psi)` from a face flux correction: `fvm::laplacian`'s `source -= V div(ffc)` negated,
    /// `V (sum / V)` per cell (`surfaceIntegrate`: owner `+=`, neighbour `-=`, then `/ V`; a non-coupled boundary face adds 0).
    #[must_use]
    pub fn neg_laplacian_source(g: &Geometry, ffc: &[f64]) -> Vec<f64> {
        let m = &g.mesh;
        let mut sum = vec![0.0; g.n_cells()];
        for f in 0..g.n_internal() {
            sum[m.owner[f]] += ffc[f];
            sum[m.neighbour[f]] -= ffc[f];
        }
        (0..g.n_cells()).map(|c| m.cell_volumes[c] * (sum[c] / m.cell_volumes[c])).collect()
    }

    /// [`Self::face_flux_correction`] then [`Self::neg_laplacian_source`]: the explicit source of `-fvm::laplacian(gamma, psi)`
    /// with `corrected`, for a field that is not `fluxRequired` (its correction flux is not kept).
    #[must_use]
    pub fn neg_laplacian_correction_source(&self, g: &Geometry, gamma: &[f64], psi: &Scalar) -> Vec<f64> {
        let ffc = self.face_flux_correction(g, gamma, psi);
        Self::neg_laplacian_source(g, &ffc)
    }

    /// The three component sources of `-fvm::laplacian(gamma, U)` with `corrected` (`correctedSnGrads.C`, per component), from the
    /// cell and stored patch values of `U`.
    #[must_use]
    pub fn neg_laplacian_vector_correction_source(&self, g: &Geometry, gamma: &[f64], uc: &[Vector3], ub: &[Vector3]) -> [Vec<f64>; 3] {
        let npatch = g.mesh.patches.len();
        let comp = |k: usize| -> Scalar {
            let pick = |v: &Vector3| match k {
                0 => v.x,
                1 => v.y,
                _ => v.z,
            };
            Scalar { c: uc.iter().map(pick).collect(), b: ub.iter().map(pick).collect(), bc: vec![Bc::Calculated; npatch] }
        };
        [
            self.neg_laplacian_correction_source(g, gamma, &comp(0)),
            self.neg_laplacian_correction_source(g, gamma, &comp(1)),
            self.neg_laplacian_correction_source(g, gamma, &comp(2)),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::sync::Arc;
    use outram_foam_basic_lib::prelude::{BoundaryPatch, FvMeshBuilder, PatchKind};

    /// A 3 x 2 x 1 block of hexahedra whose two interior vertical grid lines are sheared in x (non-orthogonal internal faces), every
    /// patch a plain `patch`; geometry by the polyMesh reader's own `compute_face_geometry`/`compute_cell_geometry`.
    fn sheared_mesh() -> Geometry {
        let (nx, ny) = (3usize, 2usize);
        let mut points: Vec<[f64; 3]> = Vec::new();
        for k in 0..2 {
            for j in 0..=ny {
                for i in 0..=nx {
                    let shear = if i > 0 && i < nx { 0.15 * (j as f64) + 0.05 * (i as f64) } else { 0.0 };
                    points.push([i as f64 + shear, j as f64 * 0.8, k as f64 * 0.5]);
                }
            }
        }
        let pid = |i: usize, j: usize, k: usize| k * (nx + 1) * (ny + 1) + j * (nx + 1) + i;
        let cid = |i: usize, j: usize| j * nx + i;
        let mut faces: Vec<Vec<usize>> = Vec::new();
        let mut owner = Vec::new();
        let mut neighbour = Vec::new();
        // internal faces in upper-triangular order (sorted by owner, then neighbour); each face's normal points owner -> neighbour
        let mut internal: Vec<(usize, usize, Vec<usize>)> = Vec::new();
        for j in 0..ny {
            for i in 0..nx {
                if i + 1 < nx {
                    internal.push((cid(i, j), cid(i + 1, j), vec![pid(i + 1, j, 0), pid(i + 1, j + 1, 0), pid(i + 1, j + 1, 1), pid(i + 1, j, 1)]));
                }
                if j + 1 < ny {
                    internal.push((cid(i, j), cid(i, j + 1), vec![pid(i, j + 1, 0), pid(i, j + 1, 1), pid(i + 1, j + 1, 1), pid(i + 1, j + 1, 0)]));
                }
            }
        }
        internal.sort_by_key(|t| (t.0, t.1));
        for (o, n, f) in internal {
            owner.push(o);
            neighbour.push(n);
            faces.push(f);
        }
        let left: Vec<_> = (0..ny).map(|j| (cid(0, j), vec![pid(0, j, 0), pid(0, j, 1), pid(0, j + 1, 1), pid(0, j + 1, 0)])).collect();
        let right: Vec<_> = (0..ny).map(|j| (cid(nx - 1, j), vec![pid(nx, j, 0), pid(nx, j + 1, 0), pid(nx, j + 1, 1), pid(nx, j, 1)])).collect();
        let bottom: Vec<_> = (0..nx).map(|i| (cid(i, 0), vec![pid(i, 0, 0), pid(i + 1, 0, 0), pid(i + 1, 0, 1), pid(i, 0, 1)])).collect();
        let top: Vec<_> = (0..nx).map(|i| (cid(i, ny - 1), vec![pid(i, ny, 0), pid(i, ny, 1), pid(i + 1, ny, 1), pid(i + 1, ny, 0)])).collect();
        let mut front = Vec::new();
        let mut back = Vec::new();
        for j in 0..ny {
            for i in 0..nx {
                front.push((cid(i, j), vec![pid(i, j, 0), pid(i, j + 1, 0), pid(i + 1, j + 1, 0), pid(i + 1, j, 0)]));
                back.push((cid(i, j), vec![pid(i, j, 1), pid(i + 1, j, 1), pid(i + 1, j + 1, 1), pid(i, j + 1, 1)]));
            }
        }
        let mut patches = Vec::new();
        for (name, list) in [("left", left), ("right", right), ("bottom", bottom), ("top", top), ("front", front), ("back", back)] {
            patches.push(BoundaryPatch::new(name, faces.len(), list.len(), PatchKind::Patch));
            for (o, f) in list {
                owner.push(o);
                faces.push(f);
            }
        }
        let n_cells = nx * ny;
        let (fc, sf, fa) = match crate::io::poly_mesh::compute_face_geometry(&points, &faces, "faces") {
            Ok(v) => v,
            Err(e) => panic!("face geometry: {e}"),
        };
        let (vol, cc) = crate::io::poly_mesh::compute_cell_geometry(&fc, &sf, &owner, &neighbour, n_cells);
        let mesh = match FvMeshBuilder::new()
            .n_cells(n_cells)
            .n_internal_faces(neighbour.len())
            .owner(owner)
            .neighbour(neighbour)
            .patches(patches)
            .cell_volumes(vol)
            .cell_centres(cc)
            .face_area_vectors(sf)
            .face_areas(fa)
            .face_centres(fc)
            .build()
        {
            Ok(m) => m,
            Err(e) => panic!("sheared test mesh: {e:?}"),
        };
        Geometry::new_openfoam(Arc::new(mesh))
    }

    /// The correction vectors are upstream's `unitArea - delta nonOrthDeltaCoeffs` with the SAME `nonOrthDeltaCoeffs` the
    /// implicit part uses (`Geometry::delta`, bit for bit), not zero on the sheared faces; a uniform field has no correction; on a
    /// linear field the corrected face-normal gradient is closer to the exact one than the uncorrected; the source is conservative;
    /// the vector form is the scalar form per component.
    #[test]
    fn correction_vectors_match_the_implicit_delta_and_a_linear_field_is_recovered() {
        let g = sheared_mesh();
        let nc = NonOrthCorrection::new(&g);
        let m = &g.mesh;
        let mut sheared = 0;
        for f in 0..g.n_internal() {
            let sf = m.face_area_vectors[f];
            let ua = Vector3::new(sf.x / m.face_areas[f], sf.y / m.face_areas[f], sf.z / m.face_areas[f]);
            let d = m.cell_centres[m.neighbour[f]] - m.cell_centres[m.owner[f]];
            let k = ua - Vector3::new(d.x * g.delta[f], d.y * g.delta[f], d.z * g.delta[f]);
            assert_eq!(nc.vectors[f], k, "face {f}: the correction vector must use Geometry::delta bit for bit");
            if k.mag() > 1e-3 {
                sheared += 1;
            }
        }
        assert!(sheared > 0, "the test mesh must have non-orthogonal faces");
        // uniform field: zero correction, zero source
        let u = Scalar::uniform(&g, 3.0, g.default_bcs());
        assert!(nc.sn_grad_correction(&g, &u).iter().all(|v| v.abs() < 1e-14));
        // linear field a . x with exact boundary values: the corrected face-normal gradient `delta (N - P) + correction` is much
        // closer to the exact `n . a` than the uncorrected `delta (N - P)` (not exact: the Gauss gradient carries the mesh's skewness)
        let a = Vector3::new(1.3, -0.7, 0.4);
        let mut lin = Scalar::uniform(&g, 0.0, g.default_bcs());
        for c in 0..g.n_cells() {
            lin.c[c] = a.dot(m.cell_centres[c]);
        }
        for f in g.n_internal()..g.n_faces() {
            lin.b[f - g.n_internal()] = a.dot(m.face_centres[f]);
        }
        lin.bc = vec![Bc::Calculated; m.patches.len()];
        let corr = nc.sn_grad_correction(&g, &lin);
        let (mut e_corr, mut e_unc) = (0.0_f64, 0.0_f64);
        for f in 0..g.n_internal() {
            let n = m.face_area_vectors[f] * (1.0 / m.face_areas[f]);
            let unc = g.delta[f] * (lin.c[m.neighbour[f]] - lin.c[m.owner[f]]);
            e_unc += (unc - n.dot(a)).abs();
            e_corr += (unc + corr[f] - n.dot(a)).abs();
        }
        assert!(e_corr < 0.25 * e_unc, "corrected error {e_corr} against uncorrected {e_unc}");
        assert!(corr[g.n_internal()..].iter().all(|v| *v == 0.0), "no correction on a non-coupled boundary face");
        // the source of -laplacian: divergence of the face flux correction, conservative (sums to zero over the mesh)
        let gamma = vec![2.0; g.n_faces()];
        let s = nc.neg_laplacian_correction_source(&g, &gamma, &lin);
        let total: f64 = s.iter().sum();
        assert!(total.abs() < 1e-12, "{total}");
        assert!(s.iter().any(|v| v.abs() > 1e-6), "a sheared mesh gives a non-zero correction source");
        // the vector form is the scalar form per component
        let uc: Vec<Vector3> = (0..g.n_cells()).map(|c| Vector3::new(lin.c[c], 2.0 * lin.c[c], 0.0)).collect();
        let ub: Vec<Vector3> = lin.b.iter().map(|&v| Vector3::new(v, 2.0 * v, 0.0)).collect();
        let sv = nc.neg_laplacian_vector_correction_source(&g, &gamma, &uc, &ub);
        assert_eq!(sv[0], s);
        assert!(sv[2].iter().all(|v| *v == 0.0));
        assert_eq!(SnGradScheme::from_word("corrected"), Some(SnGradScheme::Corrected));
        assert_eq!(SnGradScheme::from_word("uncorrected"), Some(SnGradScheme::Uncorrected));
        assert_eq!(SnGradScheme::from_word("limited"), None);
    }
}
