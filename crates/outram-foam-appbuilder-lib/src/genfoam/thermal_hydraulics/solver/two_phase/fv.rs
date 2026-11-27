// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
// and from OpenFOAM v2506 (ESI), (C) 2011-2016 OpenFOAM Foundation,
// 2017-2025 OpenCFD Ltd., GPL-3.0: the finite-volume operators below follow
// `finiteVolume/{fvMatrices,interpolation,finiteVolume}` as cited per item.
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! Finite-volume field storage and operators for the two-phase driver.
//!
//! GeN-Foam's two-phase equations read and write **boundary-face values of
//! surface fields** (`phiHbyA`, `rAUf`, `alphaRhoPhi`) and of `calculated`
//! volume fields (`p = p_rgh + rho*gh` on a patch). The driver therefore keeps
//! its own light field storage: a [`Scalar`] holds cell values, boundary-face
//! values and one [`Bc`] per patch; a surface field is a plain `Vec<f64>` over
//! **all** faces (internal then boundary), zero on `empty` patches. Matrices
//! are assembled straight into basic-lib's [`LduMatrix`] layout plus a source
//! vector, following OpenFOAM's sign conventions so `A()`/`H()` mean what
//! upstream's `UEqn.A()`/`UEqn.H()` mean (per unit volume).
//!
//! Only the operators the ported equations call are here: Euler `ddt`, Gauss
//! upwind `div`, Gauss linear `uncorrected` `laplacian` and `snGrad`, linear and
//! upwind interpolation, `surfaceSum`, `reconstruct` and Gauss `grad`. Mesh
//! geometry and topology come from basic-lib's [`FvMesh`].

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use alloc::sync::Arc;

use outram_foam_basic_lib::prelude::*;

/// Boundary condition of one patch of a [`Scalar`] field.
#[derive(Debug, Clone, PartialEq)]
pub enum Bc {
    /// `zeroGradient`: the face value equals the owner-cell value.
    ZeroGradient,
    /// `fixedValue`: one value per face of the patch.
    Fixed(Vec<f64>),
    /// `calculated`: the face value is whatever the last field algebra set.
    Calculated,
    /// `empty`: the patch carries no values (reduced-dimension case).
    Empty,
    /// `fixedGradient` (here: `fixedFluxPressure`, whose gradient the solver resets every corrector
    /// from `setSnGradP_rgh_2p.H`): one `snGrad` per face. Face value `c + g/deltaCoeffs`
    /// (`fixedGradientFvPatchField::evaluate`).
    FixedGradient(Vec<f64>),
    /// `inletOutlet` (ESI `inletOutletFvPatchField`, a `mixed` patch): `valueFraction = neg(phi)`,
    /// `refValue = inletValue`, `refGrad = 0`, so the face value is `inletValue` where the NAMED flux
    /// (`phi_index`: `0` = fluid 1's `phi`, `1` = fluid 2's) is negative and the owner value
    /// otherwise. Evaluated only by [`Scalar::correct_bc_with_flux`] (it needs the flux).
    InletOutlet {
        /// `inletValue`, one per face.
        inlet_value: Vec<f64>,
        /// Which fluid's `phi` the patch names (`phi phi.<fluid>`).
        phi_index: usize,
    },
}

/// Mesh geometry the operators need, precomputed once.
#[derive(Debug, Clone)]
pub struct Geometry {
    /// The mesh.
    pub mesh: Arc<FvMesh>,
    /// Linear interpolation weight of the **owner** on each internal face:
    /// `w = (Sf . (Cn - Cf)) / (Sf . (Cn - Co))` (OpenFOAM
    /// `surfaceInterpolation::makeWeights`).
    pub weights: Vec<f64>,
    /// `nonOrthDeltaCoeffs` on every face: `1/max(n . d, 0.05 |d|)`, with `d`
    /// cell-to-cell (internal) or cell-to-face (boundary). OpenFOAM
    /// `surfaceInterpolation::makeNonOrthDeltaCoeffs`; this is what
    /// `laplacianSchemes Gauss linear uncorrected` and `snGradSchemes
    /// uncorrected` use.
    pub delta: Vec<f64>,
    /// `false` for faces of `empty` patches.
    pub active: Vec<bool>,
    /// Patch index of each boundary face (indexed by `face - n_internal`).
    pub patch_of: Vec<usize>,
}

impl Geometry {
    /// Precompute weights and delta coefficients for `mesh`.
    #[must_use]
    pub fn new(mesh: Arc<FvMesh>) -> Self {
        Self::build(mesh, false)
    }

    /// [`Self::new`] with the weights and non-orthogonal delta coefficients computed in OpenFOAM's own operation order
    /// (gh:#1894 round 46; ESI v2506 `basicFvGeometryScheme.C`): `w = |Sf . (C_N - Cf)| / (|Sf . (Cf - C_P)| + |Sf . (C_N - Cf)|)`
    /// (`:100-130`, `0.5` below `ROOTVSMALL`) instead of `Sf . (C_N - Cf) / Sf . (C_N - C_P)`, and `unitArea = Sf / magSf` (a division
    /// per component, `:262`) instead of `Sf (1 / magSf)`. The differences are an ulp per face and show in `deltaCoeffs` of
    /// wall faces and in `weights` of internal faces of a mesh whose cell centres carry round-off noise. Used by `one_phase_fc`.
    #[must_use]
    pub fn new_openfoam(mesh: Arc<FvMesh>) -> Self {
        Self::build(mesh, true)
    }

    fn build(mesh: Arc<FvMesh>, openfoam: bool) -> Self {
        let ni = mesh.n_internal_faces;
        let nf = mesh.n_faces;
        let mut weights = vec![0.5; ni];
        let mut delta = vec![0.0; nf];
        let mut active = vec![true; nf];
        let mut patch_of = vec![0usize; nf - ni];
        let unit_area = |f: usize| {
            if openfoam {
                let (sf, m) = (mesh.face_area_vectors[f], mesh.face_areas[f]);
                Vector3::new(sf.x / m, sf.y / m, sf.z / m)
            } else {
                mesh.face_area_vectors[f] * (1.0 / mesh.face_areas[f])
            }
        };
        for f in 0..ni {
            let o = mesh.owner[f];
            let n = mesh.neighbour[f];
            let sf = mesh.face_area_vectors[f];
            let co = mesh.cell_centres[o];
            let cn = mesh.cell_centres[n];
            let cf = mesh.face_centres[f];
            if openfoam {
                let sfd_own = sf.dot(cf - co).abs();
                let sfd_nei = sf.dot(cn - cf).abs();
                let sum = sfd_own + sfd_nei;
                weights[f] = if sum.abs() > 1e-150 { sfd_nei / sum } else { 0.5 };
            } else {
                let den = sf.dot(cn - co);
                weights[f] = if den.abs() > 0.0 { sf.dot(cn - cf) / den } else { 0.5 };
            }
            let d = cn - co;
            let nhat = unit_area(f);
            delta[f] = 1.0 / nhat.dot(d).max(0.05 * d.mag());
        }
        for (pi, p) in mesh.patches.iter().enumerate() {
            for k in 0..p.size {
                let f = p.start + k;
                patch_of[f - ni] = pi;
                if p.kind == PatchKind::Empty {
                    active[f] = false;
                    continue;
                }
                let o = mesh.owner[f];
                let d = mesh.face_centres[f] - mesh.cell_centres[o];
                let nhat = unit_area(f);
                delta[f] = 1.0 / nhat.dot(d).max(0.05 * d.mag());
            }
        }
        Self {
            mesh,
            weights,
            delta,
            active,
            patch_of,
        }
    }

    /// Number of cells.
    #[must_use]
    pub fn n_cells(&self) -> usize {
        self.mesh.n_cells
    }

    /// Number of internal faces.
    #[must_use]
    pub fn n_internal(&self) -> usize {
        self.mesh.n_internal_faces
    }

    /// Number of faces.
    #[must_use]
    pub fn n_faces(&self) -> usize {
        self.mesh.n_faces
    }

    /// A zero surface field.
    #[must_use]
    pub fn surface_zeros(&self) -> Vec<f64> {
        vec![0.0; self.n_faces()]
    }

    /// Per-patch `empty`/`zeroGradient` BC list (the default for a field).
    #[must_use]
    pub fn default_bcs(&self) -> Vec<Bc> {
        self.mesh
            .patches
            .iter()
            .map(|p| {
                if p.kind == PatchKind::Empty {
                    Bc::Empty
                } else {
                    Bc::ZeroGradient
                }
            })
            .collect()
    }
}

/// A volume scalar field: cell values, boundary-face values, per-patch BCs.
#[derive(Debug, Clone, PartialEq)]
pub struct Scalar {
    /// Cell values.
    pub c: Vec<f64>,
    /// Boundary-face values, indexed by `face - n_internal`.
    pub b: Vec<f64>,
    /// One boundary condition per patch.
    pub bc: Vec<Bc>,
}

impl Scalar {
    /// A uniform field with the given BCs (boundary values set by
    /// [`Self::correct_bc`]; `calculated` patches take the uniform value).
    #[must_use]
    pub fn uniform(g: &Geometry, v: f64, bc: Vec<Bc>) -> Self {
        let mut s = Self {
            c: vec![v; g.n_cells()],
            b: vec![v; g.n_faces() - g.n_internal()],
            bc,
        };
        s.correct_bc(g);
        s
    }

    /// Apply the BCs: `zeroGradient` copies the owner value, `fixedValue`
    /// writes its values, `fixedGradient` writes `c + gradient/deltaCoeffs`, `calculated` and
    /// `empty` leave the face values.
    ///
    /// # Panics
    /// On an [`Bc::InletOutlet`] patch, which needs the flux it names: use
    /// [`Self::correct_bc_with_flux`] (a programming error, never case data).
    pub fn correct_bc(&mut self, g: &Geometry) {
        self.correct_bc_with_flux(g, [&[], &[]]);
    }

    /// As [`Self::correct_bc`], with the two fluids' `phi` available to a flux-dependent patch
    /// (`inletOutlet`): `valueFraction = neg(phi_b)` as ESI `inletOutletFvPatchField::updateCoeffs`.
    ///
    /// # Panics
    /// An [`Bc::InletOutlet`] patch whose named flux is not supplied.
    pub fn correct_bc_with_flux(&mut self, g: &Geometry, phi: [&[f64]; 2]) {
        let ni = g.n_internal();
        for (pi, p) in g.mesh.patches.iter().enumerate() {
            match &self.bc[pi] {
                Bc::ZeroGradient => {
                    for k in 0..p.size {
                        let f = p.start + k;
                        self.b[f - ni] = self.c[g.mesh.owner[f]];
                    }
                }
                Bc::Fixed(v) => {
                    for k in 0..p.size {
                        self.b[p.start + k - ni] = v[k];
                    }
                }
                Bc::FixedGradient(gr) => {
                    for k in 0..p.size {
                        let f = p.start + k;
                        self.b[f - ni] = self.c[g.mesh.owner[f]] + gr[k] / g.delta[f];
                    }
                }
                Bc::InletOutlet { inlet_value, phi_index } => {
                    let phi = phi[*phi_index];
                    assert!(
                        !phi.is_empty(),
                        "an inletOutlet patch is evaluated with its flux: use correct_bc_with_flux"
                    );
                    for k in 0..p.size {
                        let f = p.start + k;
                        // mixed::evaluate: vf*refValue + (1 - vf)*(patchInternalField + refGrad/deltaCoeffs)
                        let vf = if phi[f] < 0.0 { 1.0 } else { 0.0 };
                        self.b[f - ni] =
                            vf * inlet_value[k] + (1.0 - vf) * (self.c[g.mesh.owner[f]] + 0.0 / g.delta[f]);
                    }
                }
                Bc::Calculated | Bc::Empty => {}
            }
        }
    }

    /// `(valueInternalCoeffs, valueBoundaryCoeffs)` of boundary face `f` for a convection term
    /// `fvm::div(phi, psi)`: the diagonal gets `phi * internal`, the source `-phi * boundary`
    /// (`gaussConvectionScheme::fvmDiv`). `phi` are the two fluids' fluxes (an `inletOutlet` patch reads
    /// the one it names).
    ///
    /// # Panics
    /// An [`Bc::InletOutlet`] patch whose named flux is not supplied.
    #[must_use]
    pub fn convection_coeffs(&self, g: &Geometry, f: usize, phi: [&[f64]; 2]) -> (f64, f64) {
        let ni = g.n_internal();
        let pi = g.patch_of[f - ni];
        let k = f - g.mesh.patches[pi].start;
        match &self.bc[pi] {
            Bc::Fixed(v) => (0.0, v[k]),
            Bc::FixedGradient(gr) => (1.0, gr[k] / g.delta[f]),
            Bc::InletOutlet { inlet_value, phi_index } => {
                let named = phi[*phi_index];
                assert!(!named.is_empty(), "an inletOutlet patch needs its flux in the matrix assembly");
                let vf = if named[f] < 0.0 { 1.0 } else { 0.0 };
                (1.0 - vf, vf * inlet_value[k] + (1.0 - vf) * 0.0 / g.delta[f])
            }
            Bc::ZeroGradient | Bc::Calculated | Bc::Empty => (1.0, 0.0),
        }
    }

    /// Value on face `f` (internal faces are not defined for a vol field;
    /// callers use it for boundary faces only).
    #[must_use]
    pub fn face(&self, g: &Geometry, f: usize) -> f64 {
        self.b[f - g.n_internal()]
    }

    /// `true` when the patch of boundary face `f` is `fixedGradient`.
    #[must_use]
    pub fn is_fixed_gradient(&self, g: &Geometry, f: usize) -> bool {
        matches!(self.bc[g.patch_of[f - g.n_internal()]], Bc::FixedGradient(_))
    }

    /// `true` when the patch of boundary face `f` is `fixedValue`.
    #[must_use]
    pub fn is_fixed(&self, g: &Geometry, f: usize) -> bool {
        matches!(self.bc[g.patch_of[f - g.n_internal()]], Bc::Fixed(_))
    }

    /// Cell-and-face map `self = op(self)`, BCs kept.
    pub fn map(&mut self, op: impl Fn(f64) -> f64) {
        for v in &mut self.c {
            *v = op(*v);
        }
        for v in &mut self.b {
            *v = op(*v);
        }
    }
}

/// Boundary condition of one patch of a [`Vector`] (velocity) field.
///
/// Ported from OpenFOAM-ESI v2506 `finiteVolume/fields/fvPatchFields/`: `basic/zeroGradient`,
/// `basic/fixedValue`, `basic/basicSymmetry` (`derived/slip`) and `derived/pressureInletOutletVelocity`
/// (a `directionMixed` patch). Nothing else is accepted by the case reader.
#[derive(Debug, Clone, PartialEq)]
pub enum VectorBc {
    /// `zeroGradient`.
    ZeroGradient,
    /// `fixedValue`: one vector per face. The matching flux patch is a `fixedValue` `fvsPatchField`
    /// (`fluid.C`: the `phi` of a fixedValue/slip velocity patch is fixed, not solved).
    Fixed(Vec<Vector3>),
    /// `slip` (`basicSymmetry`): the normal component is removed, `(U_c + (I - 2nn).U_c)/2`.
    Slip,
    /// `pressureInletOutletVelocity` (`directionMixed`): `valueFraction = neg(phi)*(I - nn)`,
    /// `refValue = 0`, `refGrad = 0` (no `tangentialVelocity`); `phi_index` names which fluid's flux.
    PressureInletOutletVelocity {
        /// `0`: fluid 1's `phi`, `1`: fluid 2's.
        phi_index: usize,
    },
    /// `wedge` (ESI `wedgeFvPatchField`, a `transformFvPatchField`): the face value is `faceT & U_c` and the
    /// matrix coefficients follow `snGradTransformDiag = 0.5 diag(I - cellT)` (gh:#1812). Build it with
    /// [`wedge_velocity_bc`].
    Wedge {
        /// `wedgePolyPatch::faceT()`, row-major.
        face_t: [[f64; 3]; 3],
        /// `wedgePolyPatch::cellT() = faceT & faceT`, row-major.
        cell_t: [[f64; 3]; 3],
    },
}

/// `tensor & vector`, row-major.
fn tensor_dot(t: &[[f64; 3]; 3], v: [f64; 3]) -> [f64; 3] {
    [
        t[0][0] * v[0] + t[0][1] * v[1] + t[0][2] * v[2],
        t[1][0] * v[0] + t[1][1] * v[1] + t[1][2] * v[2],
        t[2][0] * v[0] + t[2][1] * v[1] + t[2][2] * v[2],
    ]
}

/// OpenFOAM `sign(s)`: `+1` for `s >= 0`.
fn foam_sign(s: f64) -> f64 {
    if s >= 0.0 {
        1.0
    } else {
        -1.0
    }
}

/// `rotationTensor(n1, n2)` of ESI `transform.H` (the rotation of `n1` onto `n2`), row-major.
#[must_use]
pub fn rotation_tensor(n1: [f64; 3], n2: [f64; 3]) -> [[f64; 3]; 3] {
    const SMALL: f64 = 1e-15;
    let s = n1[0] * n2[0] + n1[1] * n2[1] + n1[2] * n2[2];
    let n3 = [
        n1[1] * n2[2] - n1[2] * n2[1],
        n1[2] * n2[0] - n1[0] * n2[2],
        n1[0] * n2[1] - n1[1] * n2[0],
    ];
    let mag_sqr = n3[0] * n3[0] + n3[1] * n3[1] + n3[2] * n3[2];
    let mut out = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            let id = if i == j { 1.0 } else { 0.0 };
            out[i][j] = if mag_sqr > SMALL {
                // s I + (1 - s) sqr(n3)/|n3|^2 + (n2 n1 - n1 n2)
                s * id + (1.0 - s) * (n3[i] * n3[j]) / mag_sqr + (n2[i] * n1[j] - n1[i] * n2[j])
            } else if s < 0.0 {
                id + 2.0 * (n1[i] * n2[j])
            } else {
                id
            };
        }
    }
    out
}

/// `wedgePolyPatch::calcGeometry` for patch `pi`: `n = gAverage(faceNormals)`, `centreNormal` the coordinate-plane
/// direction implied by `n`, `faceT = rotationTensor(centreNormal, n)`, `cellT = faceT & faceT`.
///
/// # Panics
/// A patch that is not a wedge plane (zero `centreNormal`).
#[must_use]
pub fn wedge_velocity_bc(g: &Geometry, pi: usize) -> VectorBc {
    let p = &g.mesh.patches[pi];
    let mut n = [0.0; 3];
    for k in 0..p.size {
        let u = unit_normal(g, p.start + k);
        for c in 0..3 {
            n[c] += u[c];
        }
    }
    let sz = p.size as f64;
    for c in &mut n {
        *c /= sz;
    }
    let mut cn = [0.0; 3];
    for c in 0..3 {
        cn[c] = foam_sign(n[c]) * (n[c].abs().max(0.5) - 0.5);
    }
    let m = (cn[0] * cn[0] + cn[1] * cn[1] + cn[2] * cn[2]).sqrt();
    assert!(m > 1e-15, "wedge patch {pi}: no centre plane");
    for c in &mut cn {
        *c /= m;
    }
    let face_t = rotation_tensor(cn, n);
    let mut cell_t = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            cell_t[i][j] = face_t[i][0] * face_t[0][j] + face_t[i][1] * face_t[1][j] + face_t[i][2] * face_t[2][j];
        }
    }
    VectorBc::Wedge { face_t, cell_t }
}

/// The four boundary coefficients of one velocity patch face (OpenFOAM `valueInternalCoeffs`,
/// `valueBoundaryCoeffs`, `gradientInternalCoeffs`, `gradientBoundaryCoeffs`), per component.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VectorPatchCoeffs {
    /// `valueInternalCoeffs` (the convection term's diagonal weight).
    pub value_internal: [f64; 3],
    /// `valueBoundaryCoeffs`.
    pub value_boundary: [f64; 3],
    /// The diagonal `d` of `gradientInternalCoeffs = -deltaCoeffs * d` (`1` for `fixedValue`, `0`
    /// for `zeroGradient`, `snGradTransformDiag` for a transform patch).
    pub gradient_internal_diag: [f64; 3],
    /// `gradientBoundaryCoeffs`.
    pub gradient_boundary: [f64; 3],
}

/// A volume vector field (velocity): cells and boundary faces and one [`VectorBc`] per patch
/// (`zeroGradient` unless the case says otherwise; gh:#1801).
#[derive(Debug, Clone, PartialEq)]
pub struct Vector {
    /// Cell values.
    pub c: Vec<Vector3>,
    /// Boundary-face values.
    pub b: Vec<Vector3>,
    /// One boundary condition per patch.
    pub bc: Vec<VectorBc>,
}

fn comp3(v: Vector3) -> [f64; 3] {
    [v.x, v.y, v.z]
}

/// `nf = Sf/magSf` component-wise (OpenFOAM divides, it does not multiply by the reciprocal).
fn unit_normal(g: &Geometry, f: usize) -> [f64; 3] {
    let s = g.mesh.face_area_vectors[f];
    let a = g.mesh.face_areas[f];
    [s.x / a, s.y / a, s.z / a]
}

/// `symmTensor & vector` for a symmetric tensor given as `[xx, xy, xz, yy, yz, zz]`.
fn symm_dot(t: [f64; 6], v: [f64; 3]) -> [f64; 3] {
    [
        t[0] * v[0] + t[1] * v[1] + t[2] * v[2],
        t[1] * v[0] + t[3] * v[1] + t[4] * v[2],
        t[2] * v[0] + t[4] * v[1] + t[5] * v[2],
    ]
}

/// `I - 2 sqr(n)` of `basicSymmetryFvPatchField`.
fn reflection(n: [f64; 3]) -> [f64; 6] {
    [
        1.0 - 2.0 * (n[0] * n[0]),
        -(2.0 * (n[0] * n[1])),
        -(2.0 * (n[0] * n[2])),
        1.0 - 2.0 * (n[1] * n[1]),
        -(2.0 * (n[1] * n[2])),
        1.0 - 2.0 * (n[2] * n[2]),
    ]
}

/// `valueFraction = neg(phi) * (I - sqr(n))` of `pressureInletOutletVelocity`, `[xx, xy, xz, yy, yz, zz]`.
fn piov_value_fraction(n: [f64; 3], phi: f64) -> [f64; 6] {
    let neg = if phi < 0.0 { 1.0 } else { 0.0 };
    [
        neg * (1.0 - n[0] * n[0]),
        neg * (-(n[0] * n[1])),
        neg * (-(n[0] * n[2])),
        neg * (1.0 - n[1] * n[1]),
        neg * (-(n[1] * n[2])),
        neg * (1.0 - n[2] * n[2]),
    ]
}

impl Vector {
    /// A uniform vector field, `zeroGradient` on every patch.
    #[must_use]
    pub fn uniform(g: &Geometry, v: Vector3) -> Self {
        Self {
            c: vec![v; g.n_cells()],
            b: vec![v; g.n_faces() - g.n_internal()],
            bc: vec![VectorBc::ZeroGradient; g.mesh.patches.len()],
        }
    }

    /// A uniform vector field as OpenFOAM constructs a temporary one (`UEqn.H()`: `extrapolatedCalculated`): ordinary
    /// patches extrapolate (`zeroGradient`), but a mesh patch of a CONSTRAINT type keeps its constraint patch field whatever
    /// patch field type was requested (`fvPatchField::New`: a `symmetry` mesh patch always gets `symmetryFvPatchField`,
    /// i.e. `basicSymmetry`, `(U_c + (I - 2nn) U_c)/2`, whose flux through the patch is zero). gh:#1833.
    #[must_use]
    pub fn uniform_with_constraint_patches(g: &Geometry, v: Vector3) -> Self {
        let mut out = Self::uniform(g, v);
        for (pi, p) in g.mesh.patches.iter().enumerate() {
            if p.kind == PatchKind::Symmetry {
                out.bc[pi] = VectorBc::Slip;
            }
            // a `wedge` mesh patch always gets `wedgeFvPatchField` (value `faceT . U_c`; gh:#1812, #1837)
            if p.kind == PatchKind::Wedge {
                out.bc[pi] = wedge_velocity_bc(g, pi);
            }
        }
        out
    }

    /// `true` when the flux patch that belongs to boundary face `f` is a `fixedValue` `fvsPatchField`
    /// (`fluid.C`: the velocity patch is `fixedValue`, `slip` or `partialSlip`): such a flux is set
    /// to `Sf . U_b` by `correctBoundaryFlux_2p.H` and is not overwritten by `phi = phiHbyA + ...`.
    #[must_use]
    pub fn flux_is_fixed(&self, g: &Geometry, f: usize) -> bool {
        matches!(self.bc[g.patch_of[f - g.n_internal()]], VectorBc::Fixed(_) | VectorBc::Slip)
    }

    /// `true` when any patch is not `zeroGradient`.
    #[must_use]
    pub fn has_non_zero_gradient_patch(&self) -> bool {
        self.bc.iter().any(|b| !matches!(b, VectorBc::ZeroGradient))
    }

    /// Evaluate the BCs that need no flux (`zeroGradient`, `fixedValue`, `slip`).
    ///
    /// # Panics
    /// On a `pressureInletOutletVelocity` patch: use [`Self::correct_bc_with_flux`].
    pub fn correct_bc(&mut self, g: &Geometry) {
        self.correct_bc_with_flux(g, [&[], &[]]);
    }

    /// `correctBoundaryConditions()` with the two fluids' `phi` available to a
    /// `pressureInletOutletVelocity` patch (`updateCoeffs` reads `neg(phi_b)`).
    ///
    /// # Panics
    /// A `pressureInletOutletVelocity` patch whose named flux is not supplied.
    pub fn correct_bc_with_flux(&mut self, g: &Geometry, phi: [&[f64]; 2]) {
        let ni = g.n_internal();
        for (pi, p) in g.mesh.patches.iter().enumerate() {
            if p.kind == PatchKind::Empty {
                continue;
            }
            for k in 0..p.size {
                let f = p.start + k;
                let pif = self.c[g.mesh.owner[f]];
                self.b[f - ni] = match &self.bc[pi] {
                    VectorBc::ZeroGradient => pif,
                    VectorBc::Fixed(v) => v[k],
                    VectorBc::Slip => {
                        let r = symm_dot(reflection(unit_normal(g, f)), comp3(pif));
                        Vector3::new((pif.x + r[0]) / 2.0, (pif.y + r[1]) / 2.0, (pif.z + r[2]) / 2.0)
                    }
                    VectorBc::Wedge { face_t, .. } => {
                        let r = tensor_dot(face_t, comp3(pif));
                        Vector3::new(r[0], r[1], r[2])
                    }
                    VectorBc::PressureInletOutletVelocity { phi_index } => {
                        let phi = phi[*phi_index];
                        assert!(
                            !phi.is_empty(),
                            "a pressureInletOutletVelocity patch is evaluated with its flux: use correct_bc_with_flux"
                        );
                        Self::direction_mixed_value(g, f, pif, phi[f])
                    }
                };
            }
        }
    }

    /// `directionMixedFvPatchField::evaluate` for `pressureInletOutletVelocity`:
    /// `transform(vf, refValue) + transform(I - vf, patchInternalField + refGrad/deltaCoeffs)`,
    /// `refValue = refGrad = 0`.
    fn direction_mixed_value(g: &Geometry, f: usize, pif: Vector3, phi: f64) -> Vector3 {
        let vf = piov_value_fraction(unit_normal(g, f), phi);
        let normal = symm_dot(vf, [0.0, 0.0, 0.0]);
        let grad_value = [pif.x + 0.0 / g.delta[f], pif.y + 0.0 / g.delta[f], pif.z + 0.0 / g.delta[f]];
        let i_minus_vf = [1.0 - vf[0], -vf[1], -vf[2], 1.0 - vf[3], -vf[4], 1.0 - vf[5]];
        let t = symm_dot(i_minus_vf, grad_value);
        Vector3::new(normal[0] + t[0], normal[1] + t[1], normal[2] + t[2])
    }

    /// The boundary coefficients of boundary face `f` for the matrix assembly (`fvMatrix`'s
    /// `internalCoeffs`/`boundaryCoeffs`), from the CURRENT patch values.
    ///
    /// # Panics
    /// A `pressureInletOutletVelocity` patch whose named flux is not supplied.
    #[must_use]
    pub fn patch_coeffs(&self, g: &Geometry, f: usize, phi: [&[f64]; 2]) -> VectorPatchCoeffs {
        let ni = g.n_internal();
        let pi = g.patch_of[f - ni];
        let ub = comp3(self.b[f - ni]);
        let pif = comp3(self.c[g.mesh.owner[f]]);
        let delta = g.delta[f];
        let transform = |d: [f64; 3], sn: [f64; 3]| {
            let vic = [1.0 - d[0], 1.0 - d[1], 1.0 - d[2]];
            let vbc = [ub[0] - vic[0] * pif[0], ub[1] - vic[1] * pif[1], ub[2] - vic[2] * pif[2]];
            let gic = [-delta * d[0], -delta * d[1], -delta * d[2]];
            let gbc = [sn[0] - gic[0] * pif[0], sn[1] - gic[1] * pif[1], sn[2] - gic[2] * pif[2]];
            VectorPatchCoeffs {
                value_internal: vic,
                value_boundary: vbc,
                gradient_internal_diag: d,
                gradient_boundary: gbc,
            }
        };
        match &self.bc[pi] {
            VectorBc::ZeroGradient => VectorPatchCoeffs {
                value_internal: [1.0; 3],
                value_boundary: [0.0; 3],
                gradient_internal_diag: [0.0; 3],
                gradient_boundary: [0.0; 3],
            },
            VectorBc::Fixed(_) => VectorPatchCoeffs {
                value_internal: [0.0; 3],
                value_boundary: ub,
                gradient_internal_diag: [1.0; 3],
                gradient_boundary: [delta * ub[0], delta * ub[1], delta * ub[2]],
            },
            VectorBc::Slip => {
                let n = unit_normal(g, f);
                let d = [n[0].abs(), n[1].abs(), n[2].abs()];
                let r = symm_dot(reflection(n), pif);
                let h = delta / 2.0;
                let sn = [(r[0] - pif[0]) * h, (r[1] - pif[1]) * h, (r[2] - pif[2]) * h];
                transform(d, sn)
            }
            VectorBc::Wedge { cell_t, .. } => {
                // `snGradTransformDiag = 0.5 (I - cellT).diag()`, `snGrad = (0.5 deltaCoeffs)(cellT.U_c - U_c)`
                let d = [0.5 * (1.0 - cell_t[0][0]), 0.5 * (1.0 - cell_t[1][1]), 0.5 * (1.0 - cell_t[2][2])];
                let r = tensor_dot(cell_t, pif);
                let h = 0.5 * delta;
                let sn = [h * (r[0] - pif[0]), h * (r[1] - pif[1]), h * (r[2] - pif[2])];
                transform(d, sn)
            }
            VectorBc::PressureInletOutletVelocity { phi_index } => {
                let phi = phi[*phi_index];
                assert!(
                    !phi.is_empty(),
                    "a pressureInletOutletVelocity patch needs its flux: patch_coeffs was given none"
                );
                let vf = piov_value_fraction(unit_normal(g, f), phi[f]);
                let d = [vf[0].abs().sqrt(), vf[3].abs().sqrt(), vf[5].abs().sqrt()];
                let sn = [(ub[0] - pif[0]) * delta, (ub[1] - pif[1]) * delta, (ub[2] - pif[2]) * delta];
                transform(d, sn)
            }
        }
    }
}

/// `fvc::interpolate` with the `linear` scheme; boundary faces take the
/// field's boundary value.
#[must_use]
pub fn interp_linear(g: &Geometry, s: &Scalar) -> Vec<f64> {
    let ni = g.n_internal();
    let mut out = g.surface_zeros();
    for f in 0..ni {
        let w = g.weights[f];
        out[f] = w * s.c[g.mesh.owner[f]] + (1.0 - w) * s.c[g.mesh.neighbour[f]];
    }
    for f in ni..g.n_faces() {
        if g.active[f] {
            out[f] = s.b[f - ni];
        }
    }
    out
}

/// [`interp_linear`] in OpenFOAM's own operation order (gh:#1894 round 46): ESI v2506 `surfaceInterpolationScheme::interpolate(vf,
/// lambdas)` forwards to `dotInterpolate(geometricOneField(), vf, lambdas)` (`surfaceInterpolationScheme.C:296-318`), whose
/// internal-face line is `lambda (P - N) + N` (`:270`), not `lambda P + (1 - lambda) N`: the two differ by an ulp (a uniform field
/// does not even reproduce itself with the second form). Boundary faces take the field's boundary value. Used by
/// `one_phase_fc`; the other callers of [`interp_linear`] are unchanged (their recorded numbers were measured with it).
#[must_use]
pub fn interp_linear_openfoam(g: &Geometry, s: &Scalar) -> Vec<f64> {
    let ni = g.n_internal();
    let mut out = g.surface_zeros();
    for f in 0..ni {
        let w = g.weights[f];
        let (p, n) = (s.c[g.mesh.owner[f]], s.c[g.mesh.neighbour[f]]);
        out[f] = w * (p - n) + n;
    }
    for f in ni..g.n_faces() {
        if g.active[f] {
            out[f] = s.b[f - ni];
        }
    }
    out
}

/// `upwind<scalar>(mesh, phi).interpolate(s)`: the upwind cell by the sign of
/// `phi` on internal faces (`pos0(phi)` picks the owner), the boundary value
/// on boundary faces (OpenFOAM `upwind::weights`, `surfaceInterpolationScheme`).
#[must_use]
pub fn interp_upwind(g: &Geometry, s: &Scalar, phi: &[f64]) -> Vec<f64> {
    let ni = g.n_internal();
    let mut out = g.surface_zeros();
    for f in 0..ni {
        out[f] = if phi[f] >= 0.0 {
            s.c[g.mesh.owner[f]]
        } else {
            s.c[g.mesh.neighbour[f]]
        };
    }
    for f in ni..g.n_faces() {
        if g.active[f] {
            out[f] = s.b[f - ni];
        }
    }
    out
}

/// `fvc::flux(U)` = `interpolate(U) & Sf`, linear.
#[must_use]
pub fn flux(g: &Geometry, u: &Vector) -> Vec<f64> {
    let ni = g.n_internal();
    let m = &g.mesh;
    let mut out = g.surface_zeros();
    for f in 0..ni {
        let w = g.weights[f];
        let uf = u.c[m.owner[f]] * w + u.c[m.neighbour[f]] * (1.0 - w);
        out[f] = uf.dot(m.face_area_vectors[f]);
    }
    for f in ni..g.n_faces() {
        if g.active[f] {
            out[f] = u.b[f - ni].dot(m.face_area_vectors[f]);
        }
    }
    out
}

/// [`flux`] in OpenFOAM's operation order (gh:#1894 round 46): `Sf & (lambda (P - N) + N)` componentwise
/// (`surfaceInterpolationScheme.C:270`, `dotInterpolate`), not `P lambda + N (1 - lambda)`.
#[must_use]
pub fn flux_openfoam(g: &Geometry, u: &Vector) -> Vec<f64> {
    let ni = g.n_internal();
    let m = &g.mesh;
    let mut out = g.surface_zeros();
    for f in 0..ni {
        let w = g.weights[f];
        let (p, n) = (u.c[m.owner[f]], u.c[m.neighbour[f]]);
        let uf = Vector3::new(w * (p.x - n.x) + n.x, w * (p.y - n.y) + n.y, w * (p.z - n.z) + n.z);
        out[f] = m.face_area_vectors[f].dot(uf);
    }
    for f in ni..g.n_faces() {
        if g.active[f] {
            out[f] = m.face_area_vectors[f].dot(u.b[f - ni]);
        }
    }
    out
}

/// `fvc::div(phi)`: net outflow per unit volume.
#[must_use]
pub fn div(g: &Geometry, phi: &[f64]) -> Vec<f64> {
    let m = &g.mesh;
    let mut out = vec![0.0; g.n_cells()];
    for f in 0..g.n_internal() {
        out[m.owner[f]] += phi[f];
        out[m.neighbour[f]] -= phi[f];
    }
    for f in g.n_internal()..g.n_faces() {
        if g.active[f] {
            out[m.owner[f]] += phi[f];
        }
    }
    for (c, v) in out.iter_mut().enumerate() {
        *v /= m.cell_volumes[c];
    }
    out
}

/// `fvc::surfaceSum(mag(phi))` per cell (not divided by volume).
#[must_use]
pub fn surface_sum_mag(g: &Geometry, phi: &[f64]) -> Vec<f64> {
    let m = &g.mesh;
    let mut out = vec![0.0; g.n_cells()];
    for f in 0..g.n_internal() {
        out[m.owner[f]] += phi[f].abs();
        out[m.neighbour[f]] += phi[f].abs();
    }
    for f in g.n_internal()..g.n_faces() {
        if g.active[f] {
            out[m.owner[f]] += phi[f].abs();
        }
    }
    out
}

/// `fvc::snGrad(s)` with the `uncorrected` scheme.
#[must_use]
pub fn sn_grad(g: &Geometry, s: &Scalar) -> Vec<f64> {
    let m = &g.mesh;
    let ni = g.n_internal();
    let mut out = g.surface_zeros();
    for f in 0..ni {
        out[f] = (s.c[m.neighbour[f]] - s.c[m.owner[f]]) * g.delta[f];
    }
    for f in ni..g.n_faces() {
        if g.active[f] {
            out[f] = (s.b[f - ni] - s.c[m.owner[f]]) * g.delta[f];
        }
    }
    out
}

/// Gauss linear gradient of a scalar, `sum(Sf s_f)/V`.
#[must_use]
pub fn grad(g: &Geometry, s: &Scalar) -> Vec<Vector3> {
    let m = &g.mesh;
    let sf = interp_linear(g, s);
    let mut out = vec![Vector3::ZERO; g.n_cells()];
    for f in 0..g.n_internal() {
        let a = m.face_area_vectors[f] * sf[f];
        out[m.owner[f]] = out[m.owner[f]] + a;
        out[m.neighbour[f]] = out[m.neighbour[f]] - a;
    }
    for f in g.n_internal()..g.n_faces() {
        if g.active[f] {
            let o = m.owner[f];
            out[o] = out[o] + m.face_area_vectors[f] * sf[f];
        }
    }
    for (c, v) in out.iter_mut().enumerate() {
        *v = *v * (1.0 / m.cell_volumes[c]);
    }
    out
}

/// `fvc::reconstruct(phi)`: `inv(sum(Sf Sf/|Sf|)) & sum(Sf/|Sf| phi)` over
/// the non-empty faces (OpenFOAM `fvcReconstruct.C`). Directions with no
/// face area (the empty directions of a 1-D/2-D case) are removed before the
/// inverse and get a zero component, as OpenFOAM's `inv` of a reduced tensor
/// does.
#[must_use]
pub fn reconstruct(g: &Geometry, phi: &[f64]) -> Vec<Vector3> {
    let m = &g.mesh;
    let n = g.n_cells();
    let mut tens = vec![[[0.0_f64; 3]; 3]; n];
    let mut rhs = vec![[0.0_f64; 3]; n];
    let mut add = |c: usize, f: usize, phif: f64| {
        let s = m.face_area_vectors[f];
        let s = [s.x, s.y, s.z];
        let a = m.face_areas[f];
        for i in 0..3 {
            for j in 0..3 {
                tens[c][i][j] += s[i] * s[j] / a;
            }
            rhs[c][i] += s[i] / a * phif;
        }
    };
    for f in 0..g.n_internal() {
        add(m.owner[f], f, phi[f]);
        add(m.neighbour[f], f, phi[f]);
    }
    for f in g.n_internal()..g.n_faces() {
        if g.active[f] {
            add(m.owner[f], f, phi[f]);
        }
    }
    (0..n)
        .map(|c| {
            let t = tens[c];
            let tr = t[0][0] + t[1][1] + t[2][2];
            let mut keep = [true; 3];
            let mut tm = t;
            let mut r = rhs[c];
            for i in 0..3 {
                if t[i][i] <= 1e-12 * tr {
                    keep[i] = false;
                    for j in 0..3 {
                        tm[i][j] = 0.0;
                        tm[j][i] = 0.0;
                    }
                    tm[i][i] = 1.0;
                    r[i] = 0.0;
                }
            }
            let x = solve3(tm, r);
            Vector3::new(
                if keep[0] { x[0] } else { 0.0 },
                if keep[1] { x[1] } else { 0.0 },
                if keep[2] { x[2] } else { 0.0 },
            )
        })
        .collect()
}

/// Solve a 3x3 system by Cramer's rule (the matrix is symmetric positive
/// definite after the empty-direction fix in [`reconstruct`]).
/// `fvc::reconstruct(phi)` in OpenFOAM's own operations (gh:#1894 round 46; ESI v2506 `fvcReconstruct.C`,
/// `inv(surfaceSum(SfHat*Sf)) & surfaceSum(SfHat*ssf)` with `SfHat = Sf/magSf`, a division per component): the tensor field is inverted
/// by `Tensor::safeInv` (`TensorI.H:608`: a component with `xx^2 < SMALL (xx^2 + yy^2 + zz^2)` is shifted by one, `adjunct()/det`,
/// shifted back), then dotted `t.xx v.x + t.xy v.y + t.xz v.z`. [`reconstruct`] divides by Cramer's rule instead and differs by an ulp.
#[must_use]
pub fn reconstruct_openfoam(g: &Geometry, phi: &[f64]) -> Vec<Vector3> {
    let m = &g.mesh;
    let n = g.n_cells();
    let mut tens = vec![[[0.0_f64; 3]; 3]; n];
    let mut rhs = vec![[0.0_f64; 3]; n];
    let mut add = |c: usize, f: usize, phif: f64| {
        let sv = m.face_area_vectors[f];
        let a = m.face_areas[f];
        let s = [sv.x, sv.y, sv.z];
        let hat = [sv.x / a, sv.y / a, sv.z / a];
        for i in 0..3 {
            for j in 0..3 {
                tens[c][i][j] += hat[i] * s[j];
            }
            rhs[c][i] += hat[i] * phif;
        }
    };
    for f in 0..g.n_internal() {
        add(m.owner[f], f, phi[f]);
        add(m.neighbour[f], f, phi[f]);
    }
    for f in g.n_internal()..g.n_faces() {
        if g.active[f] {
            add(m.owner[f], f, phi[f]);
        }
    }
    let det = |t: &[[f64; 3]; 3]| {
        t[0][0] * t[1][1] * t[2][2] + t[0][1] * t[1][2] * t[2][0] + t[0][2] * t[1][0] * t[2][1]
            - t[0][0] * t[1][2] * t[2][1]
            - t[0][1] * t[1][0] * t[2][2]
            - t[0][2] * t[1][1] * t[2][0]
    };
    let adjunct = |t: &[[f64; 3]; 3]| {
        [
            [
                t[1][1] * t[2][2] - t[2][1] * t[1][2],
                t[0][2] * t[2][1] - t[0][1] * t[2][2],
                t[0][1] * t[1][2] - t[0][2] * t[1][1],
            ],
            [
                t[2][0] * t[1][2] - t[1][0] * t[2][2],
                t[0][0] * t[2][2] - t[0][2] * t[2][0],
                t[1][0] * t[0][2] - t[0][0] * t[1][2],
            ],
            [
                t[1][0] * t[2][1] - t[1][1] * t[2][0],
                t[0][1] * t[2][0] - t[0][0] * t[2][1],
                t[0][0] * t[1][1] - t[1][0] * t[0][1],
            ],
        ]
    };
    (0..n)
        .map(|c| {
            let t = tens[c];
            let sq = [t[0][0] * t[0][0], t[1][1] * t[1][1], t[2][2] * t[2][2]];
            let threshold = 1e-15 * (sq[0] + sq[1] + sq[2]);
            let small = [sq[0] < threshold, sq[1] < threshold, sq[2] < threshold];
            let mut work = t;
            for k in 0..3 {
                if small[k] {
                    work[k][k] += 1.0;
                }
            }
            let d = det(&work);
            let inv = if d.abs() < 1e-150 {
                [[0.0; 3]; 3]
            } else {
                let adj = adjunct(&work);
                let mut w = [[0.0; 3]; 3];
                for i in 0..3 {
                    for j in 0..3 {
                        w[i][j] = adj[i][j] / d;
                    }
                }
                for k in 0..3 {
                    if small[k] {
                        w[k][k] -= 1.0;
                    }
                }
                w
            };
            let r = rhs[c];
            Vector3::new(
                inv[0][0] * r[0] + inv[0][1] * r[1] + inv[0][2] * r[2],
                inv[1][0] * r[0] + inv[1][1] * r[1] + inv[1][2] * r[2],
                inv[2][0] * r[0] + inv[2][1] * r[1] + inv[2][2] * r[2],
            )
        })
        .collect()
}

fn solve3(a: [[f64; 3]; 3], b: [f64; 3]) -> [f64; 3] {
    let det = |m: [[f64; 3]; 3]| {
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };
    let d = det(a);
    let mut x = [0.0; 3];
    for k in 0..3 {
        let mut ak = a;
        for i in 0..3 {
            ak[i][k] = b[i];
        }
        x[k] = det(ak) / d;
    }
    x
}

/// An assembled scalar equation `diag x + offdiag x = source`, volume
/// integrated, in OpenFOAM's convention (`fvMatrix` with `psi` on the left).
#[derive(Debug, Clone)]
pub struct Eqn {
    /// LDU coefficients (basic-lib layout).
    pub ldu: LduMatrix,
    /// Right-hand side.
    pub source: Vec<f64>,
}

impl Eqn {
    /// An empty equation on the mesh.
    #[must_use]
    pub fn new(g: &Geometry) -> Self {
        let m = &g.mesh;
        Self {
            ldu: LduMatrix::new(m.n_cells, m.owner[..m.n_internal_faces].to_vec(), m.neighbour.clone()),
            source: vec![0.0; m.n_cells],
        }
    }

    /// Euler `fvm::ddt(rho, psi)` with `rho` the new and `rho0` the old
    /// coefficient (`EulerDdtScheme::fvmDdt`).
    pub fn ddt(&mut self, g: &Geometry, rho: &[f64], rho0: &[f64], psi0: &[f64], dt: f64) {
        for c in 0..g.n_cells() {
            let v = g.mesh.cell_volumes[c] / dt;
            self.ldu.diag[c] += v * rho[c];
            self.source[c] += v * rho0[c] * psi0[c];
        }
    }

    /// Gauss upwind `fvm::div(phi, psi)` (`gaussConvectionScheme::fvmDiv`
    /// with `upwind` weights). Boundary faces: a `fixedValue` patch puts
    /// `phi psi_b` in the source whatever the flux sign; any other patch puts
    /// `phi` on the diagonal (`zeroGradient` `valueInternalCoeffs = 1`).
    pub fn div_upwind(&mut self, g: &Geometry, phi: &[f64], psi: &Scalar) {
        let m = &g.mesh;
        self.div_upwind_internal(g, phi);
        for f in g.n_internal()..g.n_faces() {
            if !g.active[f] {
                continue;
            }
            let o = m.owner[f];
            if psi.is_fixed(g, f) {
                self.source[o] -= phi[f] * psi.face(g, f);
            } else {
                self.ldu.diag[o] += phi[f];
            }
        }
    }

    /// The internal-face part of [`Self::div_upwind`] (the boundary part depends on the patch types).
    pub fn div_upwind_internal(&mut self, g: &Geometry, phi: &[f64]) {
        let m = &g.mesh;
        for f in 0..g.n_internal() {
            let (o, n) = (m.owner[f], m.neighbour[f]);
            let fl = phi[f];
            if fl >= 0.0 {
                self.ldu.diag[o] += fl;
                self.ldu.lower[f] -= fl;
            } else {
                self.ldu.upper[f] += fl;
                self.ldu.diag[n] -= fl;
            }
        }
    }

    /// The INTERNAL-face part of [`Self::neg_laplacian`] (`fvm.upper() = deltaCoeffs gammaMagSf; negSumDiag()`, negated): the boundary
    /// coefficients are NOT folded into the diagonal and the source but returned per boundary face by
    /// [`Self::neg_laplacian_boundary_coeffs`], as `fvMatrix` keeps `internalCoeffs_` and `boundaryCoeffs_` (gh:#1894 round 46).
    pub fn neg_laplacian_internal(&mut self, g: &Geometry, gamma: &[f64]) {
        let m = &g.mesh;
        for f in 0..g.n_internal() {
            let c = gamma[f] * m.face_areas[f] * g.delta[f];
            self.ldu.diag[m.owner[f]] += c;
            self.ldu.diag[m.neighbour[f]] += c;
            self.ldu.upper[f] -= c;
            self.ldu.lower[f] -= c;
        }
    }

    /// `internalCoeffs` and `boundaryCoeffs` per boundary face (index `f - n_internal`) of `-fvm::laplacian(gamma, psi)`
    /// (`gaussLaplacianScheme::fvmLaplacianUncorrected`): `fixedValue`: `ic = pGamma deltaCoeffs`, `bc = pGamma (deltaCoeffs value)`;
    /// `fixedGradient`: `ic = 0`, `bc = pGamma gradient`; any other patch contributes nothing here; `pGamma = gamma magSf`.
    #[must_use]
    pub fn neg_laplacian_boundary_coeffs(g: &Geometry, gamma: &[f64], psi: &Scalar) -> (Vec<f64>, Vec<f64>) {
        let (ni, nf) = (g.n_internal(), g.n_faces());
        let mut ic = vec![0.0; nf - ni];
        let mut bc = vec![0.0; nf - ni];
        for f in ni..nf {
            if !g.active[f] {
                continue;
            }
            let pg = gamma[f] * g.mesh.face_areas[f];
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

    /// `fvMatrix::flux()` (ESI v2506 `fvMatrix.C:1422`) of a matrix with only a Laplacian's coefficients: internal faces
    /// `lduMatrix::faceH`, `Upper psi_N - Lower psi_P`; boundary faces `internalCoeffs psi_P - boundaryCoeffs`. `ic` and `bc` are
    /// per boundary face as [`Self::neg_laplacian_boundary_coeffs`] returns them.
    #[must_use]
    pub fn flux_of_coeffs(&self, g: &Geometry, p: &[f64], ic: &[f64], bc: &[f64]) -> Vec<f64> {
        let m = &g.mesh;
        let ni = g.n_internal();
        let mut out = g.surface_zeros();
        for f in 0..ni {
            out[f] = self.ldu.upper[f] * p[m.neighbour[f]] - self.ldu.lower[f] * p[m.owner[f]];
        }
        for f in ni..g.n_faces() {
            if g.active[f] {
                out[f] = ic[f - ni] * p[m.owner[f]] - bc[f - ni];
            }
        }
        out
    }

    /// `-fvm::laplacian(gamma, psi)` (positive definite), Gauss linear
    /// uncorrected, `gamma` given on faces.
    pub fn neg_laplacian(&mut self, g: &Geometry, gamma: &[f64], psi: &Scalar) {
        let m = &g.mesh;
        for f in 0..g.n_internal() {
            let c = gamma[f] * m.face_areas[f] * g.delta[f];
            self.ldu.diag[m.owner[f]] += c;
            self.ldu.diag[m.neighbour[f]] += c;
            self.ldu.upper[f] -= c;
            self.ldu.lower[f] -= c;
        }
        for f in g.n_internal()..g.n_faces() {
            if !g.active[f] {
                continue;
            }
            if psi.is_fixed(g, f) {
                let c = gamma[f] * m.face_areas[f] * g.delta[f];
                self.ldu.diag[m.owner[f]] += c;
                self.source[m.owner[f]] += c * psi.face(g, f);
            } else if let Bc::FixedGradient(gr) = &psi.bc[g.patch_of[f - g.n_internal()]] {
                // fvm::laplacian: `boundaryCoeffs = -pGamma * gradientBoundaryCoeffs` (= -pGamma * gradient
                // for `fixedGradient`, `internalCoeffs = 0`); negated for `-laplacian`.
                let k = f - g.mesh.patches[g.patch_of[f - g.n_internal()]].start;
                self.source[m.owner[f]] += gamma[f] * m.face_areas[f] * gr[k];
            }
        }
    }

    /// Add the implicit term `coeff psi` to the left-hand side (`fvm::Sp`).
    pub fn sp(&mut self, g: &Geometry, coeff: &[f64]) {
        for c in 0..g.n_cells() {
            self.ldu.diag[c] += g.mesh.cell_volumes[c] * coeff[c];
        }
    }

    /// `fvm::SuSp(coeff, psi)` on the left-hand side: implicit where
    /// `coeff > 0`, explicit (on the current `psi`) where `coeff < 0`.
    pub fn su_sp(&mut self, g: &Geometry, coeff: &[f64], psi: &[f64]) {
        for c in 0..g.n_cells() {
            let v = g.mesh.cell_volumes[c];
            self.ldu.diag[c] += v * coeff[c].max(0.0);
            self.source[c] -= v * coeff[c].min(0.0) * psi[c];
        }
    }

    /// Add an explicit per-volume term `s` to the **right-hand side**.
    pub fn su(&mut self, g: &Geometry, s: &[f64]) {
        for c in 0..g.n_cells() {
            self.source[c] += g.mesh.cell_volumes[c] * s[c];
        }
    }

    /// Scale row `c` by `s[c]` (OpenFOAM `fvMatrix::operator*=(volField)`):
    /// diagonal, source, the owner row's `upper` and the neighbour row's
    /// `lower`.
    pub fn scale_rows(&mut self, g: &Geometry, s: &[f64]) {
        let m = &g.mesh;
        for c in 0..g.n_cells() {
            self.ldu.diag[c] *= s[c];
            self.source[c] *= s[c];
        }
        for f in 0..g.n_internal() {
            self.ldu.upper[f] *= s[m.owner[f]];
            self.ldu.lower[f] *= s[m.neighbour[f]];
        }
    }

    /// Add another equation's coefficients.
    pub fn add(&mut self, o: &Eqn) {
        for (a, b) in self.ldu.diag.iter_mut().zip(&o.ldu.diag) {
            *a += b;
        }
        for (a, b) in self.ldu.upper.iter_mut().zip(&o.ldu.upper) {
            *a += b;
        }
        for (a, b) in self.ldu.lower.iter_mut().zip(&o.ldu.lower) {
            *a += b;
        }
        for (a, b) in self.source.iter_mut().zip(&o.source) {
            *a += b;
        }
    }

    /// `A()` per unit volume.
    #[must_use]
    pub fn a(&self, g: &Geometry) -> Vec<f64> {
        (0..g.n_cells())
            .map(|c| self.ldu.diag[c] / g.mesh.cell_volumes[c])
            .collect()
    }

    /// `H()` per unit volume for one component, `(source - offdiag x)/V`.
    /// For a vector equation the coefficients are shared and `source` is
    /// passed per component.
    #[must_use]
    pub fn h(&self, g: &Geometry, source: &[f64], x: &[f64]) -> Vec<f64> {
        let m = &g.mesh;
        let mut h = source.to_vec();
        for f in 0..g.n_internal() {
            let (o, n) = (m.owner[f], m.neighbour[f]);
            h[o] -= self.ldu.upper[f] * x[n];
            h[n] -= self.ldu.lower[f] * x[o];
        }
        for (c, v) in h.iter_mut().enumerate() {
            *v /= m.cell_volumes[c];
        }
        h
    }

    /// The face flux of this equation's implicit off-diagonal part,
    /// `fvMatrix::flux()` for a pure Laplacian: `upper (x_n - x_o)` on
    /// internal faces, and on a fixed boundary `c (x_b - x_o)` where `c` is
    /// the boundary coefficient. Used for `pEqnIncomp.flux()`, so it is only
    /// called on an equation holding `-laplacian(gamma, p)` alone, and the
    /// result is `-gamma |Sf| snGrad(p)` with sign as in OpenFOAM.
    #[must_use]
    pub fn laplacian_flux(g: &Geometry, gamma: &[f64], p: &Scalar) -> Vec<f64> {
        let sng = sn_grad(g, p);
        let mut out = g.surface_zeros();
        for f in 0..g.n_faces() {
            if g.active[f] {
                out[f] = -gamma[f] * g.mesh.face_areas[f] * sng[f];
            }
        }
        // `fixedGradient` faces: the flux is `-(pGamma) * gradient` straight from the boundary coefficient
        // (`fvMatrix::flux`: `internalCoeffs*psi_c - boundaryCoeffs`), not through the evaluated face value.
        for (pi, patch) in g.mesh.patches.iter().enumerate() {
            if let Bc::FixedGradient(gr) = &p.bc[pi] {
                for k in 0..patch.size {
                    let f = patch.start + k;
                    if g.active[f] {
                        out[f] = -(gamma[f] * g.mesh.face_areas[f]) * gr[k];
                    }
                }
            }
        }
        out
    }
}
