// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! # Non-orthogonal correction of the solver Laplacians (gh:#1161)
//!
//! The pieces of OpenFOAM's `Gauss linear corrected` Laplacian that the
//! incompressible solver needs, in the form upstream applies them inside
//! `pimpleFoam`.
//!
//! ## What upstream does (read 2026-11-03)
//!
//! Sources: OpenFOAM-dev `gaussLaplacianSchemes.C` (the `scalar`-Γ
//! specialisation used for both `fvm::laplacian(rAU, p)` and
//! `fvm::laplacian(nuEff, U)`), `correctedSnGrad.C`, `surfaceInterpolation.C`
//! (`makeNonOrthDeltaCoeffs`, `makeNonOrthCorrectionVectors`), and
//! OpenFOAM-10 `pimpleFoam/pEqn.H` and `nonOrthogonalSolutionControl.C`.
//!
//! 1. **Implicit part.** `fvmLaplacianUncorrected(gammaMagSf,
//!    snGradScheme.deltaCoeffs(vf), vf)`. For `corrected` (and `uncorrected`)
//!    `deltaCoeffs` is `nonOrthDeltaCoeffs = 1/max(n·d, 0.05|d|)` on internal
//!    faces and `1/(n·d_b)` on boundary faces (`fvPatch::delta()` is the
//!    patch-normal projection). Only `orthogonal` uses `1/|d|`. This port's
//!    [`fvm::laplacian_with_delta`] with [`DeltaCoeff::NonOrthogonal`] already
//!    implements exactly that, so it is reused.
//! 2. **Explicit part (`corrected` only).** `fvm.source() -= V·div(gammaMagSf ·
//!    k_f·(grad vf)_f)`, with `k_f = n − d·nonOrthDeltaCoeff` and
//!    `(grad vf)_f` the **linear** interpolate of the cell gradient from
//!    `gradSchemes` (`Gauss linear` in every tutorial). The correction vectors
//!    are **zero on non-coupled boundary patches**, so no boundary face gets an
//!    explicit term. For a vector field it is done per component: `k·∇U_j`.
//! 3. **The flux.** `p` is `fluxRequired`, so the same face correction is
//!    stored as `faceFluxCorrection` and is part of `pEqn.flux()`; `phi =
//!    phiHbyA − pEqn.flux()` therefore subtracts it, which is what keeps the
//!    corrected flux divergence-free.
//! 4. **The loop.** `while (pimple.correctNonOrthogonal())` assembles, solves
//!    and (on the final pass only) updates `phi`. `nNonOrthogonalCorrectors`
//!    defaults to **0**, i.e. one pass. **The explicit correction is still
//!    applied with 0 correctors**: it is evaluated from the `p` the pass
//!    starts with (the previous PISO corrector or time step), and it converges
//!    with the PISO/time iteration rather than within the pass.
//!
//! ## Why `fvm::laplacian_corrected` (basic-lib) is not used here
//!
//! Checked and rejected: it adds an explicit correction on Dirichlet boundary
//! faces (upstream sets the boundary correction vectors to zero), it averages
//! the two cell gradients with equal weights (upstream: `linear`), it does not
//! return the face correction that `pEqn.flux()` must subtract, and it has no
//! vector form for the momentum equation. Its geometry
//! ([`fvm::non_ortho_geometry`]) is reused.
use crate::error::AppBuilderError;
use crate::io::fv_schemes::LaplacianScheme;
use outram_foam_basic_lib::prelude::*;

/// Which part of OpenFOAM's non-orthogonal treatment a solver Laplacian gets,
/// decoded from [`LaplacianScheme`].
///
/// | `laplacianSchemes` | implicit `deltaCoeffs` | explicit correction |
/// |---|---|---|
/// | `Gauss linear orthogonal` | `1/|d|` | none |
/// | `Gauss linear uncorrected` | `1/(n·d)` | none |
/// | `Gauss linear corrected` (**default**) | `1/(n·d)` | full |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaplacianCorrection {
    /// `1/|d|`, no correction: the port before gh:#1161 (OpenFOAM `orthogonal`).
    Orthogonal,
    /// `1/(n·d)`, no explicit term (OpenFOAM `uncorrected`).
    Uncorrected,
    /// `1/(n·d)` plus the explicit `k·∇φ` term (OpenFOAM `corrected`).
    Corrected,
}

impl LaplacianCorrection {
    /// Decode a [`LaplacianScheme`].
    ///
    /// # Errors
    ///
    /// [`AppBuilderError::UnsupportedScheme`] for `Gauss linear limited c` with
    /// `0 < c < 1`: OpenFOAM's `limitedSnGrad` limiter is a face-wise ratio of
    /// the correction to the uncorrected gradient, not a plain scale, and is not
    /// ported. `limited 0` decodes to [`Self::Uncorrected`] and `limited 1` to
    /// [`Self::Corrected`], as upstream's limiter reduces to those.
    pub fn from_scheme(scheme: &LaplacianScheme) -> Result<Self, AppBuilderError> {
        match scheme {
            LaplacianScheme::GaussLinearOrthogonal => Ok(Self::Orthogonal),
            LaplacianScheme::GaussLinearUncorrected => Ok(Self::Uncorrected),
            LaplacianScheme::GaussLinearCorrected => Ok(Self::Corrected),
            LaplacianScheme::GaussLinearLimited(c) if *c <= 0.0 => Ok(Self::Uncorrected),
            LaplacianScheme::GaussLinearLimited(c) if *c >= 1.0 => Ok(Self::Corrected),
            LaplacianScheme::GaussLinearLimited(c) => Err(AppBuilderError::UnsupportedScheme {
                family: "laplacianSchemes",
                scheme: format!("Gauss linear limited {c}"),
                reason: "limitedSnGrad's face-wise limiter is not ported; use corrected or uncorrected",
            }),
        }
    }

    /// The implicit delta-coefficient mode.
    pub fn delta_coeff(self) -> fvm::DeltaCoeff {
        match self {
            Self::Orthogonal => fvm::DeltaCoeff::Orthogonal,
            Self::Uncorrected | Self::Corrected => fvm::DeltaCoeff::NonOrthogonal,
        }
    }

    /// Whether the explicit `k·∇φ` term is applied.
    pub fn is_explicit(self) -> bool {
        matches!(self, Self::Corrected)
    }
}

/// OpenFOAM `linear` weight of the owner on internal face `f` (the same
/// distance form as `fvc::interpolate`).
fn owner_weight(mesh: &FvMesh, f: usize) -> f64 {
    let co = mesh.cell_centres[mesh.owner[f]];
    let cn = mesh.cell_centres[mesh.neighbour[f]];
    let denom = (cn - co).mag();
    if denom < 1e-300 {
        return 0.5;
    }
    (cn - mesh.face_centres[f]).mag() / denom
}

/// `k_f = n − d·nonOrthDeltaCoeff` on internal face `f` (dimensionless), zero
/// for a degenerate face.
fn correction_vector(mesh: &FvMesh, f: usize) -> Vector3 {
    let d = mesh.cell_centres[mesh.neighbour[f]] - mesh.cell_centres[mesh.owner[f]];
    fvm::non_ortho_geometry(mesh.face_area_vectors[f], d).map_or(Vector3::ZERO, |g| g.corr_vec)
}

/// The explicit face correction of a scalar Laplacian, `Γ_f |Sf| k_f·(∇φ)_f`,
/// per internal face (upstream's `faceFluxCorrection`; boundary faces carry
/// none). Units: `[Γ]·m²·[φ]/m`, a volumetric flux \[m³/s\] when Γ = rAU \[s\]
/// and φ = p \[m²/s²\].
pub fn scalar_face_correction(gamma: &SurfaceScalarField, grad: &VolVectorField, mesh: &FvMesh) -> Vec<f64> {
    (0..mesh.n_internal_faces)
        .map(|f| {
            let w = owner_weight(mesh, f);
            let g = grad.internal[mesh.owner[f]] * w + grad.internal[mesh.neighbour[f]] * (1.0 - w);
            gamma.internal[f] * mesh.face_areas[f] * correction_vector(mesh, f).dot(g)
        })
        .collect()
}

/// `k·T` for OpenFOAM's gradient-tensor convention `T_ij = ∂u_j/∂x_i`: the
/// vector whose component `j` is `k·∇u_j`.
fn k_dot_grad(k: Vector3, t: &Tensor) -> Vector3 {
    t.row_x() * k.x + t.row_y() * k.y + t.row_z() * k.z
}

/// Vector counterpart of [`scalar_face_correction`]: component `j` is
/// `Γ_f |Sf| k_f·(∇u_j)_f` \[m⁴/s²\] for Γ = ν \[m²/s\] and u \[m/s\].
pub fn vector_face_correction(gamma: &SurfaceScalarField, grad: &VolTensorField, mesh: &FvMesh) -> Vec<Vector3> {
    (0..mesh.n_internal_faces)
        .map(|f| {
            let w = owner_weight(mesh, f);
            let t = grad.internal[mesh.owner[f]] * w + grad.internal[mesh.neighbour[f]] * (1.0 - w);
            k_dot_grad(correction_vector(mesh, f), &t) * (gamma.internal[f] * mesh.face_areas[f])
        })
        .collect()
}

/// Add a face correction to a positive-definite (`−∇·Γ∇`) matrix source:
/// `+c` on the owner, `−c` on the neighbour. This is upstream's
/// `fvm.source() -= V·div(correction)` in this port's sign convention (the
/// matrix is the negative of OpenFOAM's; see the `pimple_foam` module doc).
pub fn add_to_source_scalar(source: &mut [f64], corr: &[f64], mesh: &FvMesh) {
    for (f, c) in corr.iter().enumerate() {
        source[mesh.owner[f]] += c;
        source[mesh.neighbour[f]] -= c;
    }
}

/// Vector counterpart of [`add_to_source_scalar`].
pub fn add_to_source_vector(source: &mut [Vector3], corr: &[Vector3], mesh: &FvMesh) {
    for (f, c) in corr.iter().enumerate() {
        source[mesh.owner[f]] = source[mesh.owner[f]] + *c;
        source[mesh.neighbour[f]] = source[mesh.neighbour[f]] - *c;
    }
}

/// Switch the implicit coefficients of a vector Laplacian that was assembled
/// with `1/|d|` (as `fvm::laplacian_vec` and the turbulence closures'
/// `divDevReff` do) to `mode`'s delta coefficients, face by face, on the
/// internal faces and on the Dirichlet boundary arms `laplacian_vec` stamps
/// (`FixedValue`, `FixedField`, `Calculated`, `FlowRateInletVelocity`,
/// `NoSlip`). Robin/gradient arms keep the orthogonal treatment.
///
/// `gamma` must be the face diffusivity the matrix was built with. A no-op for
/// [`fvm::DeltaCoeff::Orthogonal`].
pub fn restamp_vector_delta(mat: &mut FvVectorMatrix, gamma: &SurfaceScalarField, u: &VolVectorField, mode: fvm::DeltaCoeff) {
    if mode == fvm::DeltaCoeff::Orthogonal {
        return;
    }
    let mesh = u.mesh.clone();
    for f in 0..mesh.n_internal_faces {
        let d_orth = fvm::DeltaCoeff::Orthogonal.interior_delta(&mesh, f);
        let d_new = mode.interior_delta(&mesh, f);
        if d_orth < 1e-300 || d_new < 1e-300 {
            continue;
        }
        let ga = gamma.internal[f] * mesh.face_areas[f];
        let delta = ga / d_new - ga / d_orth;
        mat.ldu.diag[mesh.owner[f]] += delta;
        mat.ldu.diag[mesh.neighbour[f]] += delta;
        mat.ldu.upper[f] -= delta;
        mat.ldu.lower[f] -= delta;
    }
    for (pi, patch) in mesh.patches.iter().enumerate() {
        if matches!(patch.kind, PatchKind::Cyclic | PatchKind::CyclicAmi) {
            continue;
        }
        for fi in 0..patch.size {
            let gf = patch.start + fi;
            let value = match &u.boundary[pi].bc {
                BoundaryCondition::FixedValue(_)
                | BoundaryCondition::FixedField(_)
                | BoundaryCondition::Calculated(_)
                | BoundaryCondition::FlowRateInletVelocity { .. } => u.boundary[pi].values[fi],
                BoundaryCondition::NoSlip => Vector3::ZERO,
                _ => continue,
            };
            let d_orth = fvm::DeltaCoeff::Orthogonal.boundary_delta(&mesh, gf).max(1e-30);
            let d_new = mode.boundary_delta(&mesh, gf);
            if d_new < 1e-300 {
                continue;
            }
            let ga = gamma.boundary[pi].values[fi] * mesh.face_areas[gf];
            let delta = ga / d_new - ga / d_orth;
            let o = mesh.owner[gf];
            mat.ldu.diag[o] += delta;
            mat.source[o] = mat.source[o] + value * delta;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// Two cells whose centres are offset in y, so the shared face (normal +x)
    /// is 45° non-orthogonal.
    fn sheared_pair() -> Arc<FvMesh> {
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(2)
                .n_internal_faces(1)
                .owner(vec![0, 1, 0])
                .neighbour(vec![1])
                .patches(vec![
                    BoundaryPatch::new("right", 1, 1, PatchKind::Wall),
                    BoundaryPatch::new("left", 2, 1, PatchKind::Wall),
                ])
                .cell_volumes(vec![1.0, 1.0])
                .cell_centres(vec![Vector3::new(-0.5, -0.5, 0.0), Vector3::new(0.5, 0.5, 0.0)])
                .face_area_vectors(vec![
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(-1.0, 0.0, 0.0),
                ])
                .face_centres(vec![
                    Vector3::new(0.0, 0.0, 0.0),
                    Vector3::new(1.0, 0.5, 0.0),
                    Vector3::new(-1.0, -0.5, 0.0),
                ])
                .build()
                .unwrap(),
        )
    }

    #[test]
    fn scheme_decoding_matches_upstream_table() {
        use LaplacianCorrection as C;
        assert_eq!(C::from_scheme(&LaplacianScheme::GaussLinearCorrected).unwrap(), C::Corrected);
        assert_eq!(C::from_scheme(&LaplacianScheme::GaussLinearUncorrected).unwrap(), C::Uncorrected);
        assert_eq!(C::from_scheme(&LaplacianScheme::GaussLinearOrthogonal).unwrap(), C::Orthogonal);
        assert_eq!(C::from_scheme(&LaplacianScheme::GaussLinearLimited(1.0)).unwrap(), C::Corrected);
        assert_eq!(C::from_scheme(&LaplacianScheme::GaussLinearLimited(0.0)).unwrap(), C::Uncorrected);
        assert!(C::from_scheme(&LaplacianScheme::GaussLinearLimited(0.33)).is_err());
        assert!(C::Corrected.is_explicit() && !C::Uncorrected.is_explicit());
        assert_eq!(C::Orthogonal.delta_coeff(), fvm::DeltaCoeff::Orthogonal);
        assert_eq!(C::Uncorrected.delta_coeff(), fvm::DeltaCoeff::NonOrthogonal);
    }

    /// For a linear field φ = a·x the corrected snGrad, Δ(φ_N − φ_P) + k·∇φ,
    /// equals n·a exactly on any face: the property upstream's construction
    /// exists for. Checked here through the two pieces this module provides.
    #[test]
    fn corrected_sngrad_is_exact_for_a_linear_field() {
        let mesh = sheared_pair();
        let a = Vector3::new(2.0, -3.0, 0.5);
        let phi: Vec<f64> = mesh.cell_centres.iter().map(|c| a.dot(*c)).collect();
        let grad = VolVectorField::new(
            "g",
            mesh.clone(),
            Field::new(vec![a; 2]),
            mesh.patches.iter().map(|p| PatchField::zero_gradient_vec(p.size)).collect(),
        );
        let gamma = SurfaceScalarField::uniform("G", mesh.clone(), 1.0);
        let corr = scalar_face_correction(&gamma, &grad, &mesh);
        let delta = fvm::DeltaCoeff::NonOrthogonal.interior_delta(&mesh, 0);
        let sngrad = (phi[1] - phi[0]) / delta + corr[0];
        assert!((sngrad - a.x).abs() < 1e-12, "snGrad {sngrad} vs n·a {}", a.x);

        // Vector form, component by component (T_ij = ∂u_j/∂x_i = A_ji for u = A·x).
        let t = Tensor::new(1.0, 4.0, 0.0, -2.0, 0.5, 0.0, 0.0, 0.0, 1.0);
        let gv = VolTensorField::new(
            "gU",
            mesh.clone(),
            Field::new(vec![t; 2]),
            mesh.patches.iter().map(|p| PatchField::zero_gradient_tensor(p.size)).collect(),
        );
        let cv = vector_face_correction(&gamma, &gv, &mesh);
        // k = n − d/(n·d) = (1,0,0) − (1,1,0) = (0,−1,0): k·∇u_j = −T_yj.
        assert!((cv[0] - Vector3::new(2.0, -0.5, 0.0)).mag() < 1e-12, "{:?}", cv[0]);

        let mut s = vec![0.0; 2];
        add_to_source_scalar(&mut s, &corr, &mesh);
        assert_eq!(s[0], -s[1]);
        let mut sv = vec![Vector3::ZERO; 2];
        add_to_source_vector(&mut sv, &cv, &mesh);
        assert!((sv[0] + sv[1]).mag() < 1e-15);
    }

    /// Re-stamping an orthogonal vector Laplacian to `NonOrthogonal` gives the
    /// same matrix as the scalar `laplacian_with_delta(NonOrthogonal)`.
    #[test]
    fn restamped_vector_laplacian_matches_the_scalar_non_orthogonal_one() {
        let mesh = sheared_pair();
        let nu = VolScalarField::uniform("nu", mesh.clone(), 0.7);
        let u = VolVectorField::new(
            "U",
            mesh.clone(),
            Field::new(vec![Vector3::ZERO; 2]),
            vec![
                PatchField::fixed_value_vec(1, Vector3::new(1.0, 0.0, 0.0)),
                PatchField { bc: BoundaryCondition::NoSlip, values: Field::new(vec![Vector3::ZERO]) },
            ],
        );
        let p = VolScalarField::new(
            "p",
            mesh.clone(),
            Field::new(vec![0.0; 2]),
            vec![PatchField::fixed_value(1, 1.0), PatchField::fixed_value(1, 0.0)],
        );
        let gamma = fvc::interpolate(&nu);
        let mut mv = fvm::laplacian_vec(&nu, &u, mesh.clone());
        restamp_vector_delta(&mut mv, &gamma, &u, fvm::DeltaCoeff::NonOrthogonal);
        let ms = fvm::laplacian_with_delta(&gamma, &p, fvm::DeltaCoeff::NonOrthogonal);
        for c in 0..2 {
            assert!((mv.ldu.diag[c] - ms.ldu.diag[c]).abs() < 1e-12, "diag {c}");
        }
        assert!((mv.ldu.upper[0] - ms.ldu.upper[0]).abs() < 1e-12);
        assert!((mv.source[0].x - ms.source[0]).abs() < 1e-12);

        // Orthogonal mode leaves the matrix untouched.
        let before = fvm::laplacian_vec(&nu, &u, mesh.clone());
        let mut same = fvm::laplacian_vec(&nu, &u, mesh.clone());
        restamp_vector_delta(&mut same, &gamma, &u, fvm::DeltaCoeff::Orthogonal);
        assert_eq!(before.ldu.diag, same.ldu.diag);
    }
}
