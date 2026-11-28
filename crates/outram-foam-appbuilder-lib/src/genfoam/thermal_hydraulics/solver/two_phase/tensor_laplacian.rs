// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
// and from OpenFOAM v2506 (ESI), (C) 2011-2016 OpenFOAM Foundation, 2017-2025 OpenCFD Ltd., GPL-3.0:
//   src/finiteVolume/finiteVolume/laplacianSchemes/gaussLaplacianScheme/gaussLaplacianScheme.C
//   src/finiteVolume/finiteVolume/gradSchemes/gaussGrad/gaussGrad.C
//   src/finiteVolume/interpolation/surfaceInterpolation/surfaceInterpolationScheme/surfaceInterpolationScheme.C
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! The tensor-diffusivity Laplacian of the `onePhase` equations (gh:#1229, README round 64).
//!
//! `structureProperties.<zone>.localTortuosity (tx ty tz)` builds a `volTensorField tortuosity` (identity outside the zone,
//! `diag(tx, ty, tz)` inside; `structure.C:663-711`; the local-frame rotation `Rl2g & diag & Rg2l` of `structure.C:702` is not ported:
//! a `localReferenceFrame` is refused). `UEqn_1p.H:27-40` then solves
//! `- fvm::laplacian(alpha rho nuEff tortuosity, U) - fvc::div(alpha rho nuEff tortuosity & dev2(T(fvc::grad(U))), "div(alphaRhoPhiNu,U)")`
//! and `EEqn_1p.H:22-27` `- fvm::laplacian(interpolate(alpha) interpolate(tortuosity alphaEff), he)`, EVERYWHERE on the mesh (the
//! tortuosity is the identity outside the zone), through `gaussLaplacianScheme<Type, tensor>::fvmLaplacian` (`gaussLaplacianScheme.C`,
//! the `GType` overload, translated here line by line):
//!
//! ```text
//! Sn          = Sf / magSf
//! SfGamma     = Sf & gamma                (surfaceVectorField)
//! SfGammaSn   = SfGamma & Sn              (the "gammaMagSf" of fvmLaplacianUncorrected)
//! SfGammaCorr = SfGamma - SfGammaSn Sn
//! fvm         = fvmLaplacianUncorrected(SfGammaSn, deltaCoeffs, vf)
//! faceFluxCorrection[cmpt] = dotInterpolate(SfGammaCorr, fvc::grad(vf.component(cmpt)))     (+ SfGammaSn snGrad correction if `corrected`: refused, also after gh:#1991 ported the scalar-gamma `corrected`)
//! fvm.source() -= V fvc::div(faceFluxCorrection)
//! ```
//!
//! Only `Gauss linear` gradients and the `uncorrected` snGrad/Laplacian schemes are ported (the reader refuses the others). The
//! `component` field of a vector keeps the constraint patch types (`wedge`, `symmetry`: scalar `snGrad` 0) and `calculated` elsewhere
//! (`snGrad = deltaCoeffs (b - c)`).

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use outram_foam_basic_lib::prelude::*;

use super::fv::{interp_linear_openfoam, wedge_velocity_bc, Bc, Eqn, Geometry, Scalar, Vector, VectorBc};
use super::one_phase_cc::flow::UEqnCc;

/// A row-major tensor.
pub type Tensor3 = [[f64; 3]; 3];

/// The identity tensor.
pub const IDENTITY: Tensor3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// `diag(d)`.
#[must_use]
pub fn diag(d: [f64; 3]) -> Tensor3 {
    [[d[0], 0.0, 0.0], [0.0, d[1], 0.0], [0.0, 0.0, d[2]]]
}

/// `s * t`, componentwise (`scalar * tensor`).
#[must_use]
pub fn scale(s: f64, t: &Tensor3) -> Tensor3 {
    let mut r = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            r[i][j] = s * t[i][j];
        }
    }
    r
}

/// `a & b` for tensors, OpenFOAM's `Tensor operator&`: `xx*xx + xy*yx + xz*zx` summed left to right.
#[must_use]
pub fn tensor_and(a: &Tensor3, b: &Tensor3) -> Tensor3 {
    let mut r = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            r[i][j] = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
        }
    }
    r
}

fn transpose(t: &Tensor3) -> Tensor3 {
    [[t[0][0], t[1][0], t[2][0]], [t[0][1], t[1][1], t[2][1]], [t[0][2], t[1][2], t[2][2]]]
}

/// `v & t` (`Vector operator&(Vector, Tensor)`): `v.x t.xj + v.y t.yj + v.z t.zj`.
#[must_use]
pub fn vec_and_tensor(v: [f64; 3], t: &Tensor3) -> [f64; 3] {
    [
        v[0] * t[0][0] + v[1] * t[1][0] + v[2] * t[2][0],
        v[0] * t[0][1] + v[1] * t[1][1] + v[2] * t[2][1],
        v[0] * t[0][2] + v[1] * t[1][2] + v[2] * t[2][2],
    ]
}

/// The cell and boundary-face values of the `tortuosity` volTensorField (`structure.C:666-711`, `zeroGradient` patches with the
/// constraint patches of the mesh: a `wedge` patch holds `faceT & t & faceT.T()`, `wedgeFvPatchField::evaluate` through `transform`).
///
/// `cell_diag` is the per-cell `diag(localTortuosity)` (identity outside the tortuous zones). Boundary values are indexed by
/// `face - n_internal`.
///
/// # Errors
/// A `symmetry` patch (the tensor transform of a `symmetryFvPatchField` is not ported).
pub fn tortuosity_fields(g: &Geometry, cell_diag: &[[f64; 3]]) -> Result<(Vec<Tensor3>, Vec<Tensor3>), String> {
    let ni = g.n_internal();
    let cells: Vec<Tensor3> = cell_diag.iter().map(|d| diag(*d)).collect();
    let mut b = vec![IDENTITY; g.n_faces() - ni];
    for (pi, p) in g.mesh.patches.iter().enumerate() {
        match p.kind {
            PatchKind::Empty => {}
            PatchKind::Symmetry => return Err(format!("localTortuosity with a `symmetry` patch (`{}`) is not ported", p.name)),
            PatchKind::Wedge => {
                let VectorBc::Wedge { face_t, .. } = wedge_velocity_bc(g, pi) else { unreachable!("wedge_velocity_bc returns a wedge") };
                for k in 0..p.size {
                    let f = p.start + k;
                    b[f - ni] = tensor_and(&tensor_and(&face_t, &cells[g.mesh.owner[f]]), &transpose(&face_t));
                }
            }
            _ => {
                for k in 0..p.size {
                    let f = p.start + k;
                    b[f - ni] = cells[g.mesh.owner[f]];
                }
            }
        }
    }
    Ok((cells, b))
}

/// `fvc::interpolate(volTensorField)` with `linear`: `lambda (P - N) + N` per component on internal faces
/// (`surfaceInterpolationScheme.C:270`), the boundary value on boundary faces; every face of an `empty` patch is the zero tensor.
#[must_use]
pub fn interpolate(g: &Geometry, cells: &[Tensor3], boundary: &[Tensor3]) -> Vec<Tensor3> {
    let ni = g.n_internal();
    let mut out = vec![[[0.0; 3]; 3]; g.n_faces()];
    for f in 0..ni {
        let w = g.weights[f];
        let (p, n) = (&cells[g.mesh.owner[f]], &cells[g.mesh.neighbour[f]]);
        for i in 0..3 {
            for j in 0..3 {
                out[f][i][j] = w * (p[i][j] - n[i][j]) + n[i][j];
            }
        }
    }
    for f in ni..g.n_faces() {
        if g.active[f] {
            out[f] = boundary[f - ni];
        }
    }
    out
}

/// `SfGammaSn` (the `gammaMagSf` of the uncorrected Laplacian) and `SfGammaCorr`, per face.
#[derive(Debug, Clone, PartialEq)]
pub struct TensorGamma {
    /// `(Sf & gamma) & Sn`, every face (0 on `empty` faces).
    pub gamma_mag_sf: Vec<f64>,
    /// `Sf & gamma - SfGammaSn Sn`, every face.
    pub corr: Vec<[f64; 3]>,
}

/// `gaussLaplacianScheme::fvmLaplacian(gamma, vf)` lines `SfGamma`, `SfGammaSn`, `SfGammaCorr` for a surface tensor `gamma`.
#[must_use]
pub fn decompose(g: &Geometry, gamma_f: &[Tensor3]) -> TensorGamma {
    let m = &g.mesh;
    let mut gms = vec![0.0; g.n_faces()];
    let mut corr = vec![[0.0; 3]; g.n_faces()];
    for f in 0..g.n_faces() {
        if !g.active[f] {
            continue;
        }
        let (s, a) = (m.face_area_vectors[f], m.face_areas[f]);
        let sf = [s.x, s.y, s.z];
        let sn = [s.x / a, s.y / a, s.z / a];
        let sg = vec_and_tensor(sf, &gamma_f[f]);
        let sgsn = sg[0] * sn[0] + sg[1] * sn[1] + sg[2] * sn[2];
        gms[f] = sgsn;
        corr[f] = [sg[0] - sgsn * sn[0], sg[1] - sgsn * sn[1], sg[2] - sgsn * sn[2]];
    }
    TensorGamma { gamma_mag_sf: gms, corr }
}

/// `true` when some face has a non-zero `SfGammaCorr` (otherwise the correction flux is exactly zero and no gradient is needed).
#[must_use]
pub fn has_correction(tg: &TensorGamma) -> bool {
    tg.corr.iter().any(|c| c[0] != 0.0 || c[1] != 0.0 || c[2] != 0.0)
}

/// `fvc::grad(vf.component(cmpt))`, `Gauss linear` (`gaussGrad.C`: `calcGrad` then `correctBoundaryConditions`): cell gradients and
/// boundary-face gradients (`indexed by face - n_internal`). `comp` holds the cell values and boundary-face values of the component
/// (`calculated` patches, `wedge`/`symmetry` keep their constraint type: `snGrad` 0).
#[must_use]
pub fn grad_component(g: &Geometry, comp: &Scalar) -> (Vec<[f64; 3]>, Vec<[f64; 3]>) {
    let m = &g.mesh;
    let ni = g.n_internal();
    let sf = interp_linear_openfoam(g, comp);
    let mut gr = vec![[0.0; 3]; g.n_cells()];
    for f in 0..ni {
        let s = m.face_area_vectors[f];
        let v = [s.x * sf[f], s.y * sf[f], s.z * sf[f]];
        let (o, n) = (m.owner[f], m.neighbour[f]);
        for k in 0..3 {
            gr[o][k] += v[k];
            gr[n][k] -= v[k];
        }
    }
    for f in ni..g.n_faces() {
        if g.active[f] {
            let s = m.face_area_vectors[f];
            let o = m.owner[f];
            gr[o][0] += s.x * sf[f];
            gr[o][1] += s.y * sf[f];
            gr[o][2] += s.z * sf[f];
        }
    }
    for (c, v) in gr.iter_mut().enumerate() {
        for k in v.iter_mut() {
            *k /= m.cell_volumes[c];
        }
    }
    let mut gb = vec![[0.0; 3]; g.n_faces() - ni];
    for f in ni..g.n_faces() {
        if !g.active[f] {
            continue;
        }
        let o = m.owner[f];
        let s = m.face_area_vectors[f];
        let a = m.face_areas[f];
        let n = [s.x / a, s.y / a, s.z / a];
        let constraint = matches!(m.patches[g.patch_of[f - ni]].kind, PatchKind::Wedge | PatchKind::Symmetry);
        let sn = if constraint { 0.0 } else { (comp.b[f - ni] - comp.c[o]) * g.delta[f] };
        let ndg = n[0] * gr[o][0] + n[1] * gr[o][1] + n[2] * gr[o][2];
        let d = sn - ndg;
        gb[f - ni] = [gr[o][0] + n[0] * d, gr[o][1] + n[1] * d, gr[o][2] + n[2] * d];
    }
    (gr, gb)
}

/// `gammaSnGradCorr` for one component and `fvm.source() -= V fvc::div(faceFluxCorrection)`, returned as `V div(flux)` per cell:
/// `flux = dotInterpolate(SfGammaCorr, grad(component))` (`lambda (P - N) + N` of the gradient on internal faces, the boundary
/// gradient on boundary faces), `fvc::div` = `surfaceIntegrate` (sum, then `/ V`), times `V`.
#[must_use]
pub fn correction_source(g: &Geometry, tg: &TensorGamma, comp: &Scalar) -> Vec<f64> {
    let m = &g.mesh;
    let ni = g.n_internal();
    let (gr, gb) = grad_component(g, comp);
    let mut flux = vec![0.0; g.n_faces()];
    for f in 0..ni {
        let w = g.weights[f];
        let (p, n) = (&gr[m.owner[f]], &gr[m.neighbour[f]]);
        let gf = [w * (p[0] - n[0]) + n[0], w * (p[1] - n[1]) + n[1], w * (p[2] - n[2]) + n[2]];
        let c = tg.corr[f];
        flux[f] = c[0] * gf[0] + c[1] * gf[1] + c[2] * gf[2];
    }
    for f in ni..g.n_faces() {
        if g.active[f] {
            let (c, gf) = (tg.corr[f], gb[f - ni]);
            flux[f] = c[0] * gf[0] + c[1] * gf[1] + c[2] * gf[2];
        }
    }
    let mut sum = vec![0.0; g.n_cells()];
    for f in 0..ni {
        sum[m.owner[f]] += flux[f];
        sum[m.neighbour[f]] -= flux[f];
    }
    for f in ni..g.n_faces() {
        if g.active[f] {
            sum[m.owner[f]] += flux[f];
        }
    }
    (0..g.n_cells()).map(|c| m.cell_volumes[c] * (sum[c] / m.cell_volumes[c])).collect()
}

/// The internal-face part of `-fvm::laplacian(gamma, psi)` with `gammaMagSf = SfGammaSn`: `fvm.upper() = deltaCoeffs gammaMagSf;
/// negSumDiag()`, negated.
pub fn neg_laplacian_internal(e: &mut Eqn, g: &Geometry, gms: &[f64]) {
    let m = &g.mesh;
    for f in 0..g.n_internal() {
        let c = gms[f] * g.delta[f];
        e.ldu.diag[m.owner[f]] += c;
        e.ldu.diag[m.neighbour[f]] += c;
        e.ldu.upper[f] -= c;
        e.ldu.lower[f] -= c;
    }
}

/// `internalCoeffs` and `boundaryCoeffs` per boundary face of `-fvm::laplacian(gamma, psi)` with `pGamma = SfGammaSn` (as
/// `Eqn::neg_laplacian_boundary_coeffs`, which takes `gamma magSf`).
#[must_use]
pub fn neg_laplacian_boundary_coeffs(g: &Geometry, gms: &[f64], psi: &Scalar) -> (Vec<f64>, Vec<f64>) {
    let (ni, nf) = (g.n_internal(), g.n_faces());
    let mut ic = vec![0.0; nf - ni];
    let mut bc = vec![0.0; nf - ni];
    for f in ni..nf {
        if !g.active[f] {
            continue;
        }
        let pg = gms[f];
        if psi.is_fixed(g, f) {
            ic[f - ni] = pg * g.delta[f];
            bc[f - ni] = pg * (g.delta[f] * psi.face(g, f));
        } else if let Bc::FixedGradient(gr) = &psi.bc[g.patch_of[f - ni]] {
            let k = f - g.mesh.patches[g.patch_of[f - ni]].start;
            bc[f - ni] = pg * gr[k];
        }
    }
    (ic, bc)
}

/// `- fvm::laplacian(gamma tensor, U)` of `UEqn_1p.H` added to `ue`: the internal coefficients, the boundary coefficients from the
/// patch values of `u` and the `SfGammaCorr` source of each of the three components (`src += V div(flux)` for the negated matrix).
pub(in crate::genfoam::thermal_hydraulics::solver::two_phase) fn add_neg_laplacian_vector(ue: &mut UEqnCc, g: &Geometry, tg: &TensorGamma, u: &Vector) {
    neg_laplacian_internal(&mut ue.eqn, g, &tg.gamma_mag_sf);
    let ni = g.n_internal();
    for f in ni..g.n_faces() {
        if !g.active[f] {
            continue;
        }
        let c = u.patch_coeffs(g, f, [&[], &[]]);
        let pg = tg.gamma_mag_sf[f];
        let d = g.delta[f];
        for k in 0..3 {
            ue.ic[f - ni][k] += -(pg * (-d * c.gradient_internal_diag[k]));
            ue.bcoef[f - ni][k] += pg * c.gradient_boundary[k];
        }
    }
    if !has_correction(tg) {
        return;
    }
    for k in 0..3 {
        let comp = Scalar {
            c: u.c.iter().map(|v| [v.x, v.y, v.z][k]).collect(),
            b: u.b.iter().map(|v| [v.x, v.y, v.z][k]).collect(),
            bc: vec![Bc::Calculated; g.mesh.patches.len()],
        };
        let s = correction_source(g, tg, &comp);
        for c in 0..g.n_cells() {
            ue.src[k][c] += s[c];
        }
    }
}

/// The `SfGammaCorr` source of `- fvm::laplacian(gamma tensor, he)` of `EEqn_1p.H`, added to `e.source`.
pub fn add_neg_laplacian_scalar_correction(e: &mut Eqn, g: &Geometry, tg: &TensorGamma, psi: &Scalar) {
    if !has_correction(tg) {
        return;
    }
    let comp = Scalar { c: psi.c.clone(), b: psi.b.clone(), bc: vec![Bc::Calculated; g.mesh.patches.len()] };
    let s = correction_source(g, tg, &comp);
    for c in 0..g.n_cells() {
        e.source[c] += s[c];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::interface::one_dimensional_meshing::create_one_d_mesh;
    use uom::si::area::square_meter;
    use uom::si::f64::{Area, Length};
    use uom::si::length::meter;

    fn line(n: i64) -> Geometry {
        let mesh = create_one_d_mesh(Length::new::<meter>(1.0), Area::new::<square_meter>(1e-2), n).expect("1-D mesh");
        Geometry::new(alloc::sync::Arc::new(mesh))
    }

    #[test]
    fn tensor_products_follow_the_openfoam_index_order() {
        let a = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]];
        let b = [[0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 2.0]];
        let c = tensor_and(&a, &b);
        assert_eq!(c, [[2.0, 1.0, 6.0], [5.0, 4.0, 12.0], [8.0, 7.0, 18.0]]);
        // v & t: row vector times tensor
        assert_eq!(vec_and_tensor([1.0, 0.0, 0.0], &a), [1.0, 2.0, 3.0]);
        assert_eq!(vec_and_tensor([0.0, 1.0, 0.0], &a), [4.0, 5.0, 6.0]);
        assert_eq!(scale(2.0, &IDENTITY)[1][1], 2.0);
        assert_eq!(diag([1.0, 2.0, 3.0])[2][2], 3.0);
    }

    /// Criterion 4 of README round 64: `gamma = g I` gives the scalar-gamma coefficients bit for bit, and a tensor with an off-axis
    /// entry gives the hand-computed `SfGammaSn` and `SfGammaCorr` (`Sf = (A, 0, 0)`: `Sf & gamma = (2A, 3A, 0)`).
    #[test]
    fn tensor_gamma_reduces_to_the_scalar_gamma_and_splits_an_off_axis_tensor() {
        let g = line(8);
        let nf = g.n_faces();
        let gi: Vec<Tensor3> = vec![scale(2.5, &IDENTITY); nf];
        let tg = decompose(&g, &gi);
        let mut e1 = Eqn::new(&g);
        let mut e2 = Eqn::new(&g);
        neg_laplacian_internal(&mut e1, &g, &tg.gamma_mag_sf);
        e2.neg_laplacian_internal(&g, &vec![2.5; nf]);
        assert_eq!(e1.ldu.diag, e2.ldu.diag);
        assert_eq!(e1.ldu.upper, e2.ldu.upper);
        assert!(!has_correction(&tg), "an isotropic gamma on an axis-aligned mesh has no correction");
        let a = g.mesh.face_areas[0];
        let off = [[2.0, 3.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let tg = decompose(&g, &vec![off; nf]);
        assert!((tg.gamma_mag_sf[0] - 2.0 * a).abs() < 1e-14 * a);
        assert!(tg.corr[0][0].abs() < 1e-14 && (tg.corr[0][1] - 3.0 * a).abs() < 1e-14 * a && tg.corr[0][2].abs() < 1e-14);
        assert!(has_correction(&tg));
        // the correction source of a uniform field is zero (its gradient is zero)
        let psi = Scalar::uniform(&g, 4.0, g.default_bcs());
        let s = correction_source(&g, &tg, &psi);
        assert!(s.iter().all(|v| v.abs() < 1e-14), "{s:?}");
        // and of a linear field it is the divergence of corr . grad: constant flux per face except at the ends
        let mut lin = Scalar::uniform(&g, 0.0, g.default_bcs());
        for c in 0..g.n_cells() {
            lin.c[c] = g.mesh.cell_centres[c].y;
        }
        lin.correct_bc(&g);
        let (gr, _gb) = grad_component(&g, &lin);
        assert!(gr.iter().all(|v| v[0].abs() < 1e-12 && v[1].abs() < 1e-12), "a field varying only in the empty direction");
    }

    /// The tortuosity tensor of a mesh without wedge patches is `diag(t)` in the cells and on every non-empty boundary face.
    #[test]
    fn tortuosity_fields_hold_the_diagonal_on_the_cells_and_the_boundary() {
        let g = line(4);
        let d = vec![[0.7, 1.3, 0.01389]; g.n_cells()];
        let (c, b) = tortuosity_fields(&g, &d).expect("no symmetry patch");
        assert_eq!(c[2], diag([0.7, 1.3, 0.01389]));
        let f = g.n_internal();
        assert_eq!(b[0], c[g.mesh.owner[f]]);
    }
}
