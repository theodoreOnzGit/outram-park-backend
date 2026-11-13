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

use std::sync::Arc;

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
        let ni = mesh.n_internal_faces;
        let nf = mesh.n_faces;
        let mut weights = vec![0.5; ni];
        let mut delta = vec![0.0; nf];
        let mut active = vec![true; nf];
        let mut patch_of = vec![0usize; nf - ni];
        for f in 0..ni {
            let o = mesh.owner[f];
            let n = mesh.neighbour[f];
            let sf = mesh.face_area_vectors[f];
            let co = mesh.cell_centres[o];
            let cn = mesh.cell_centres[n];
            let cf = mesh.face_centres[f];
            let den = sf.dot(cn - co);
            weights[f] = if den.abs() > 0.0 { sf.dot(cn - cf) / den } else { 0.5 };
            let d = cn - co;
            let nhat = sf * (1.0 / mesh.face_areas[f]);
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
                let nhat = mesh.face_area_vectors[f] * (1.0 / mesh.face_areas[f]);
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
    /// writes its values, `calculated` and `empty` leave the face values.
    pub fn correct_bc(&mut self, g: &Geometry) {
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
                Bc::Calculated | Bc::Empty => {}
            }
        }
    }

    /// Value on face `f` (internal faces are not defined for a vol field;
    /// callers use it for boundary faces only).
    #[must_use]
    pub fn face(&self, g: &Geometry, f: usize) -> f64 {
        self.b[f - g.n_internal()]
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

/// A volume vector field (velocity): cells and boundary faces, all patches
/// `zeroGradient` (the only vector BC the two-phase tutorials use for `U`).
#[derive(Debug, Clone, PartialEq)]
pub struct Vector {
    /// Cell values.
    pub c: Vec<Vector3>,
    /// Boundary-face values.
    pub b: Vec<Vector3>,
}

impl Vector {
    /// A uniform vector field.
    #[must_use]
    pub fn uniform(g: &Geometry, v: Vector3) -> Self {
        Self {
            c: vec![v; g.n_cells()],
            b: vec![v; g.n_faces() - g.n_internal()],
        }
    }

    /// `zeroGradient` on every non-empty patch.
    pub fn correct_bc(&mut self, g: &Geometry) {
        let ni = g.n_internal();
        for f in ni..g.n_faces() {
            if g.active[f] {
                self.b[f - ni] = self.c[g.mesh.owner[f]];
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
            if g.active[f] && psi.is_fixed(g, f) {
                let c = gamma[f] * m.face_areas[f] * g.delta[f];
                self.ldu.diag[m.owner[f]] += c;
                self.source[m.owner[f]] += c * psi.face(g, f);
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
        out
    }
}
