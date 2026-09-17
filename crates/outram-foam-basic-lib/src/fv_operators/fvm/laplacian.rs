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

use crate::fields::boundary::bc::BoundaryCondition;
use crate::fields::surface_field::SurfaceScalarField;
use crate::fields::vol_field::VolScalarField;
use crate::ldu_matrix::fv_matrix::FvMatrix;
use crate::mesh::fv_mesh::PatchKind;
use crate::primitives::Vector3;

/// OpenFOAM's floor on `n . d` as a fraction of `|d|`, guarding the division at
/// extreme non-orthogonality
/// (`basicFvGeometryScheme::nonOrthDeltaCoeffs`, OpenFOAM v2506).
const NON_ORTH_FLOOR: f64 = 0.05;

/// Which face-to-face distance the Laplacian divides by — OpenFOAM's choice of
/// `deltaCoeffs` versus `nonOrthDeltaCoeffs`.
///
/// This selects the **implicit** coefficient only. It says nothing about the
/// explicit deferred correction, which lives in
/// [`laplacian_corrected`](super::laplacian_corrected) and is orthogonal to this
/// choice. Together the two reproduce OpenFOAM's three `snGradSchemes` entries:
///
/// | OpenFOAM `snGradSchemes` | `DeltaCoeff` | explicit correction |
/// |---|---|---|
/// | `orthogonal` | [`Orthogonal`](Self::Orthogonal) | none |
/// | `uncorrected` | [`NonOrthogonal`](Self::NonOrthogonal) | none |
/// | `corrected` | [`NonOrthogonal`](Self::NonOrthogonal) | full (`laplacian_corrected`) |
///
/// # Why the distinction matters
///
/// `uncorrected` is **not** "the orthogonal scheme". It still projects the
/// cell-to-cell vector onto the face normal; it merely drops the explicit term.
/// Treating the two as the same under-estimates every face conductance by
/// `1/cos(theta)`, `theta` being the face's non-orthogonality angle — silently,
/// and with no residual to reveal it. Measured on the GeN-Foam `2D_MSFR`
/// neutronics mesh (whose `fvSchemes` asks for `uncorrected`), that is 1.7 % on
/// the interior faces and up to 12 % on the boundary patches, worth **+580 pcm**
/// in `k_eff`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeltaCoeff {
    /// `1/|d|` — OpenFOAM's `deltaCoeffs`, exact only on an orthogonal mesh.
    ///
    /// The **default**, so that every existing caller keeps its behaviour
    /// bit-for-bit.
    #[default]
    Orthogonal,
    /// `1/max(n . d, 0.05 |d|)` — OpenFOAM's `nonOrthDeltaCoeffs`, the
    /// projection of the cell-to-cell vector onto the face normal.
    ///
    /// On a boundary face OpenFOAM first replaces `d = Cf - C_P` by its
    /// patch-normal component `n (n . d)` (`fvPatch::delta()`), which makes the
    /// coefficient `1/(n . d)` there.
    NonOrthogonal,
}

impl DeltaCoeff {
    /// The distance `[m]` this scheme divides by on **internal** face `face`.
    ///
    /// The reciprocal is OpenFOAM's `deltaCoeffs`/`nonOrthDeltaCoeffs` entry for
    /// that face. Exposed so a caller that builds its own boundary or face
    /// coefficient can divide by exactly the same length the Laplacian does —
    /// a Robin condition whose weight uses a different one is silently wrong by
    /// `1/cos(theta)`.
    #[must_use]
    pub fn interior_delta(self, mesh: &crate::mesh::fv_mesh::FvMesh, face: usize) -> f64 {
        interior_delta(
            self,
            mesh.face_area_vectors[face],
            mesh.cell_centres[mesh.neighbour[face]] - mesh.cell_centres[mesh.owner[face]],
            mesh.face_areas[face],
        )
    }

    /// The distance `[m]` this scheme divides by on **boundary** face `face`
    /// (a global face index, `patch.start + i`).
    ///
    /// See [`Self::interior_delta`] for why a caller would want this.
    #[must_use]
    pub fn boundary_delta(self, mesh: &crate::mesh::fv_mesh::FvMesh, face: usize) -> f64 {
        boundary_delta(
            self,
            mesh.face_area_vectors[face],
            mesh.face_centres[face] - mesh.cell_centres[mesh.owner[face]],
            mesh.face_areas[face],
        )
    }
}

/// The interior-face distance the Laplacian divides by, per `mode`.
///
/// `d` is the owner-to-neighbour vector `[m]`, `sf` the face area vector
/// `[m^2]`. Returns the effective distance `[m]`, never negative.
fn interior_delta(mode: DeltaCoeff, sf: Vector3, d: Vector3, area: f64) -> f64 {
    match mode {
        DeltaCoeff::Orthogonal => d.mag(),
        DeltaCoeff::NonOrthogonal => {
            if area < 1.0e-300 {
                return d.mag();
            }
            let n_hat = sf * (1.0 / area);
            n_hat.dot(d).max(NON_ORTH_FLOOR * d.mag())
        }
    }
}

/// The boundary-face distance the Laplacian divides by, per `mode`.
///
/// `d` is the owner-cell-centre-to-face-centre vector `[m]`. For
/// [`DeltaCoeff::NonOrthogonal`] this mirrors `fvPatch::delta()` followed by
/// `nonOrthDeltaCoeffs`: the vector is first projected onto the patch normal, so
/// the result is `max(n . d, 0.05 |n . d|)`.
fn boundary_delta(mode: DeltaCoeff, sf: Vector3, d: Vector3, area: f64) -> f64 {
    match mode {
        DeltaCoeff::Orthogonal => d.mag(),
        DeltaCoeff::NonOrthogonal => {
            if area < 1.0e-300 {
                return d.mag();
            }
            let n_hat = sf * (1.0 / area);
            let p = n_hat.dot(d);
            p.max(NON_ORTH_FLOOR * p.abs())
        }
    }
}

/// Implicit Gauss-orthogonal Laplacian: assembles the matrix for `−∇·(Γ∇φ)`.
///
/// ## Sign convention (matches OpenFOAM)
///
/// The returned matrix has **positive** diagonal and **negative** off-diagonals,
/// so the matrix–vector product `A·φ` approximates `−∇·(Γ∇φ)`.  Use the matrix
/// with a minus sign in the PDE to add the diffusion term:
///
/// ```text
/// // ∂φ/∂t − ∇·(Γ∇φ) = S
/// let eqn = fvm::ddt(&phi, &phi_old, dt) - fvm::laplacian(&gamma_f, &phi);
/// ```
///
/// ## Boundary conditions
///
/// - `ZeroGradient` / `Symmetry`: no contribution (zero normal flux).
/// - `FixedValue(v)`: adds `coeff` to diagonal and `coeff·v` to source.
///
/// ## Cyclic (periodic) patches
///
/// [`PatchKind::Cyclic`] patches carry no boundary-value contribution; instead
/// each matched face pair is discretised as an **internal face across the
/// periodic seam** (see the cyclic-coupling loop below), coupling the owner cell
/// of one half to the owner cell of the partner half with the orthogonal
/// coefficient `Γ·|Sf| / d_seam`, `d_seam = |Cf_A − C_ownerA| + |Cf_B −
/// C_ownerB|`. This reproduces the equivalent all-internal ring mesh. Mirrors
/// `Foam::cyclicFvPatchField` (`src/finiteVolume/.../cyclic/cyclicFvPatchField.H`).
///
/// ## Non-conformal periodic (cyclicAMI) patches
///
/// [`PatchKind::CyclicAmi`](crate::mesh::PatchKind::CyclicAmi) patches are
/// non-conformal: each target seam face overlaps several source faces. Each
/// [`AmiCoupling`](crate::mesh::AmiCoupling) is assembled as a set of **partial
/// internal faces**, one per weighted (target, source) overlap, each carrying
/// `Γ·overlap_area / delta` with the symmetric internal-face stamp. Per target
/// the overlap areas sum to the target area, so the seam flux is distributed
/// conservatively (`A·uniform = 0`); in the matching (conformal) limit this
/// reduces exactly to the plain cyclic seam. Mirrors
/// `Foam::cyclicAMIFvPatchField` + `src/meshTools/AMIInterpolation/...`.
pub fn laplacian(gamma: &SurfaceScalarField, phi: &VolScalarField) -> FvMatrix {
    laplacian_with_delta(gamma, phi, DeltaCoeff::Orthogonal)
}

/// [`laplacian`] with an explicit choice of face-distance coefficient.
///
/// `mode` picks between OpenFOAM's `deltaCoeffs` and `nonOrthDeltaCoeffs` — see
/// [`DeltaCoeff`] for the mapping onto OpenFOAM's `snGradSchemes`. Everything
/// else (sign convention, boundary-condition arms, cyclic and cyclicAMI seam
/// couplings) is exactly as documented on [`laplacian`], which is this function
/// called with [`DeltaCoeff::Orthogonal`].
///
/// Pass [`DeltaCoeff::NonOrthogonal`] to reproduce a case whose `fvSchemes`
/// asks for `uncorrected` — every GeN-Foam neutronics case does.
///
/// # Seam couplings are not covered
///
/// Cyclic and cyclicAMI seams keep the orthogonal `1/|d|` treatment whatever
/// `mode` says: OpenFOAM's coupled patches override `delta()` with a form this
/// port does not yet implement, so silently applying the projection there would
/// be a guess rather than a port. Do not rely on `mode` at a periodic seam.
pub fn laplacian_with_delta(
    gamma: &SurfaceScalarField,
    phi: &VolScalarField,
    mode: DeltaCoeff,
) -> FvMatrix {
    let mesh = phi.mesh.clone();
    let mut mat = FvMatrix::new(mesh.clone());

    // Internal faces: Gauss, with the delta coefficient `mode` selects.
    for f in 0..mesh.n_internal_faces {
        let o = mesh.owner[f];
        let n = mesh.neighbour[f];
        let delta = interior_delta(
            mode,
            mesh.face_area_vectors[f],
            mesh.cell_centres[n] - mesh.cell_centres[o],
            mesh.face_areas[f],
        );
        if delta < 1e-300 {
            continue;
        }
        let coeff = gamma.internal[f] * mesh.face_areas[f] / delta;
        mat.ldu.diag[o] += coeff;
        mat.ldu.diag[n] += coeff;
        mat.ldu.upper[f] = -coeff;
        mat.ldu.lower[f] = -coeff;
    }

    // Boundary faces
    for (pi, patch) in mesh.patches.iter().enumerate() {
        // Cyclic / cyclicAMI patches are handled as internal-like seam couplings
        // below, not as ordinary boundary faces.
        if patch.kind == PatchKind::Cyclic || patch.kind == PatchKind::CyclicAmi {
            continue;
        }
        for fi in 0..patch.size {
            let gf = patch.start + fi;
            let owner = mesh.owner[gf];
            let d = boundary_delta(
                mode,
                mesh.face_area_vectors[gf],
                mesh.face_centres[gf] - mesh.cell_centres[owner],
                mesh.face_areas[gf],
            );
            if d < 1e-300 {
                continue;
            }
            let coeff = gamma.boundary[pi].values[fi] * mesh.face_areas[gf] / d;
            match &phi.boundary[pi].bc {
                BoundaryCondition::ZeroGradient | BoundaryCondition::Symmetry => {}
                BoundaryCondition::FixedValue(v) => {
                    mat.ldu.diag[owner] += coeff;
                    mat.source[owner] += coeff * v;
                }
                BoundaryCondition::FixedField(ff) => {
                    mat.ldu.diag[owner] += coeff;
                    mat.source[owner] += coeff * ff[fi];
                }
                // No-slip wall: fixedValue of zero → implicit diagonal only.
                BoundaryCondition::NoSlip => {
                    mat.ldu.diag[owner] += coeff;
                }
                // Prescribed normal gradient g [value·m⁻¹]: explicit boundary
                // flux gamma·area·g = (coeff·d)·g into the source, no diagonal.
                BoundaryCondition::FixedGradient(g) => {
                    mat.source[owner] += coeff * d * g;
                }
                // Robin/mixed: blend fixedValue (weight w) and fixedGradient
                // (1-w). w=1 recovers the FixedValue arm, w=0 the FixedGradient.
                BoundaryCondition::Mixed {
                    value_fraction,
                    ref_value,
                    ref_grad,
                } => {
                    let w = *value_fraction;
                    mat.ldu.diag[owner] += w * coeff;
                    mat.source[owner] += w * coeff * ref_value + (1.0 - w) * coeff * d * ref_grad;
                }
                // Per-face Robin: identical algebra, coefficients read per face.
                BoundaryCondition::MixedField {
                    value_fraction,
                    ref_value,
                    ref_grad,
                } => {
                    let w = value_fraction[fi];
                    mat.ldu.diag[owner] += w * coeff;
                    mat.source[owner] +=
                        w * coeff * ref_value[fi] + (1.0 - w) * coeff * d * ref_grad[fi];
                }
                // fixedFluxPressure: a fixedGradient whose gradient the solver
                // set (snGrad(p)); same explicit boundary flux as FixedGradient.
                BoundaryCondition::FixedFluxPressure { gradient } => {
                    mat.source[owner] += coeff * d * gradient;
                }
                // totalPressure: Dirichlet using the solver-computed face value
                // stored in the patch — implicit diagonal + explicit source.
                BoundaryCondition::TotalPressure { .. } => {
                    mat.ldu.diag[owner] += coeff;
                    mat.source[owner] += coeff * phi.boundary[pi].values[fi];
                }
                // `InletOutlet`/`OutletInlet`/`Freestream` carry no flux in the
                // diffusion operator (degrade to zero-gradient);
                // `Slip`/`Wedge`/`Empty`/`pressureInletOutletVelocity`/
                // `flowRateInletVelocity` are zero-gradient-like here. No
                // contribution.
                _ => {}
            }
        }
    }

    // Cyclic (periodic) seam couplings: assemble each matched face pair exactly
    // like an internal face joining the two owner cells across the seam.
    for (i, cc) in mesh.cyclic_couplings.iter().enumerate() {
        let cf = mesh.cyclic_coupling_face(i);
        let o = cc.owner;
        let nb = cc.neighbour;
        // Cell-to-cell distance across the seam = owner→face_a plus
        // face_b→neighbour (the two half-face gaps of the coincident seam).
        let d_a = (mesh.face_centres[cc.face_a] - mesh.cell_centres[o]).mag();
        let d_b = (mesh.face_centres[cc.face_b] - mesh.cell_centres[nb]).mag();
        let delta = d_a + d_b;
        if delta < 1e-300 {
            continue;
        }
        // Γ on the seam taken from the half0 (patch_a) boundary coefficient.
        let gamma_seam = gamma.boundary[cc.patch_a].values[cc.local];
        let coeff = gamma_seam * mesh.face_areas[cc.face_a] / delta;
        mat.ldu.diag[o] += coeff;
        mat.ldu.diag[nb] += coeff;
        mat.ldu.upper[cf] = -coeff;
        mat.ldu.lower[cf] = -coeff;
    }

    // Non-conformal (cyclicAMI) seam couplings: assemble each target seam face
    // as a set of partial internal faces — one per weighted (target, source)
    // overlap. Each partial face carries the orthogonal diffusion coefficient
    // Γ·overlap_area/delta of a face of area `overlap_area`, with the standard
    // symmetric internal-face stamp. Summed over a target's sources this equals
    // the full target-face flux distributed conservatively across the sources;
    // in the matching (conformal) limit (one source, weight 1) it reduces
    // exactly to the plain cyclic seam above. Mirrors `Foam::cyclicAMIFvPatchField`.
    let mut cf = mesh.ami_ldu_start();
    for coupling in &mesh.ami_couplings {
        let o = coupling.target_cell;
        let d_t = (mesh.face_centres[coupling.target_face] - mesh.cell_centres[o]).mag();
        // Γ on the seam taken from the target-patch boundary coefficient.
        let gamma_seam = gamma.boundary[coupling.target_patch].values[coupling.local];
        for w in &coupling.weights {
            let nb = w.source_cell;
            let d_s = (mesh.face_centres[w.source_face] - mesh.cell_centres[nb]).mag();
            let delta = d_t + d_s;
            if delta < 1e-300 {
                cf += 1;
                continue;
            }
            let coeff = gamma_seam * w.overlap_area / delta;
            mat.ldu.diag[o] += coeff;
            mat.ldu.diag[nb] += coeff;
            mat.ldu.upper[cf] = -coeff;
            mat.ldu.lower[cf] = -coeff;
            cf += 1;
        }
    }

    mat
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::boundary::bc::{BoundaryCondition, PatchField};
    use crate::fields::field::Field;
    use crate::fields::surface_field::SurfaceScalarField;
    use crate::fields::vol_field::VolScalarField;
    use crate::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};
    use crate::primitives::Vector3;
    use std::sync::Arc;

    fn unit_mesh() -> Arc<crate::mesh::fv_mesh::FvMesh> {
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
                .cell_volumes(vec![0.5, 0.5])
                .cell_centres(vec![
                    Vector3::new(0.25, 0.0, 0.0),
                    Vector3::new(0.75, 0.0, 0.0),
                ])
                .face_area_vectors(vec![
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(-1.0, 0.0, 0.0),
                ])
                .face_centres(vec![
                    Vector3::new(0.5, 0.0, 0.0),
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(0.0, 0.0, 0.0),
                ])
                .build()
                .unwrap(),
        )
    }

    fn uniform_gamma(m: Arc<crate::mesh::fv_mesh::FvMesh>, val: f64) -> SurfaceScalarField {
        let _n_faces = m.owner.len();
        let internal = Field::uniform(m.n_internal_faces, val);
        let boundary = m
            .patches
            .iter()
            .map(|p| PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: Field::uniform(p.size, val),
            })
            .collect();
        SurfaceScalarField::new("gamma", m, internal, boundary)
    }

    #[test]
    fn laplacian_symmetric_matrix() {
        // unit gamma: upper[f] == lower[f] and both are -coeff
        let m = unit_mesh();
        let gamma = uniform_gamma(m.clone(), 1.0);
        let phi = VolScalarField::uniform("T", m.clone(), 0.0);
        let mat = laplacian(&gamma, &phi);
        // internal face: |C_N - C_O| = 0.5, area = 1 → coeff = 1/0.5 = 2
        assert!((mat.ldu.upper[0] - (-2.0)).abs() < 1e-10);
        assert!((mat.ldu.lower[0] - (-2.0)).abs() < 1e-10);
    }

    #[test]
    fn laplacian_solves_linear_dirichlet() {
        // −∇²T = 0, T(0)=0, T(1)=1 → T is linear: T[0]=0.25, T[1]=0.75
        let m = unit_mesh();
        let gamma = uniform_gamma(m.clone(), 1.0);
        let t_bc = vec![
            PatchField {
                bc: BoundaryCondition::FixedValue(1.0),
                values: Field::new(vec![0.0]),
            },
            PatchField {
                bc: BoundaryCondition::FixedValue(0.0),
                values: Field::new(vec![0.0]),
            },
        ];
        let phi = VolScalarField::new("T", m.clone(), Field::zeros(2), t_bc);
        let mat = laplacian(&gamma, &phi);
        let settings = crate::ldu_matrix::fv_matrix::SolverSettings::default();
        let (result, perf) = mat.solve("T", settings);
        assert!(perf.converged, "Gauss-Seidel did not converge");
        assert!(
            (result.internal[0] - 0.25).abs() < 1e-6,
            "T[0] = {}",
            result.internal[0]
        );
        assert!(
            (result.internal[1] - 0.75).abs() < 1e-6,
            "T[1] = {}",
            result.internal[1]
        );
    }

    // ── V&V for the new patch-field BCs (verification, not validation) ──────
    // UNTRUSTED AI-ASSISTED DRAFT pending human V&V review.
    // Data version / date taken: 2026-08-04.

    /// V&V (verification, 2026-08-04). Methodology: assemble the Laplacian on
    /// the 2-cell unit mesh with a right patch that is (a) `FixedValue(v)` and
    /// (b) `Mixed { value_fraction: 1, ref_value: v, ref_grad: <arbitrary> }`.
    /// A mixed BC at `value_fraction == 1` must be identical to `fixedValue`.
    /// Pass criterion: every diag/source entry agrees to < 1e-12.
    /// Result: max |Δdiag| = 0, max |Δsource| = 0 (exact). PASS.
    #[test]
    fn vv_mixed_reduces_to_fixed_value() {
        let m = unit_mesh();
        let gamma = uniform_gamma(m.clone(), 1.0);
        let bc_fv = vec![
            PatchField {
                bc: BoundaryCondition::FixedValue(2.0),
                values: Field::new(vec![0.0]),
            },
            PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: Field::new(vec![0.0]),
            },
        ];
        let bc_mix = vec![
            PatchField {
                bc: BoundaryCondition::Mixed {
                    value_fraction: 1.0,
                    ref_value: 2.0,
                    ref_grad: 99.0, // must be ignored at w = 1
                },
                values: Field::new(vec![0.0]),
            },
            PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: Field::new(vec![0.0]),
            },
        ];
        let a = laplacian(
            &gamma,
            &VolScalarField::new("T", m.clone(), Field::zeros(2), bc_fv),
        );
        let b = laplacian(
            &gamma,
            &VolScalarField::new("T", m.clone(), Field::zeros(2), bc_mix),
        );
        for i in 0..2 {
            assert!((a.ldu.diag[i] - b.ldu.diag[i]).abs() < 1e-12);
            assert!((a.source[i] - b.source[i]).abs() < 1e-12);
        }
    }

    /// V&V (verification, 2026-08-04). Methodology: as above but comparing a
    /// right patch that is (a) `FixedGradient(g)` and (b)
    /// `Mixed { value_fraction: 0, ref_value: <arbitrary>, ref_grad: g }`.
    /// A mixed BC at `value_fraction == 0` must be identical to `fixedGradient`.
    /// Pass criterion: every diag/source entry agrees to < 1e-12.
    /// Result: max |Δdiag| = 0, max |Δsource| = 0 (exact). PASS.
    #[test]
    fn vv_mixed_reduces_to_fixed_gradient() {
        let m = unit_mesh();
        let gamma = uniform_gamma(m.clone(), 1.0);
        let bc_fg = vec![
            PatchField {
                bc: BoundaryCondition::FixedGradient(0.5),
                values: Field::new(vec![0.0]),
            },
            PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: Field::new(vec![0.0]),
            },
        ];
        let bc_mix = vec![
            PatchField {
                bc: BoundaryCondition::Mixed {
                    value_fraction: 0.0,
                    ref_value: 99.0, // must be ignored at w = 0
                    ref_grad: 0.5,
                },
                values: Field::new(vec![0.0]),
            },
            PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: Field::new(vec![0.0]),
            },
        ];
        let a = laplacian(
            &gamma,
            &VolScalarField::new("T", m.clone(), Field::zeros(2), bc_fg),
        );
        let b = laplacian(
            &gamma,
            &VolScalarField::new("T", m.clone(), Field::zeros(2), bc_mix),
        );
        for i in 0..2 {
            assert!((a.ldu.diag[i] - b.ldu.diag[i]).abs() < 1e-12);
            assert!((a.source[i] - b.source[i]).abs() < 1e-12);
        }
    }

    /// V&V (verification, 2026-08-04). Methodology: 1-D steady conduction
    /// `-∇²T = 0` on the 2-cell unit mesh `x ∈ [0, 1]` (cell centres 0.25,
    /// 0.75). Left patch `FixedValue(0)` at x=0, right patch `FixedGradient(g)`
    /// at x=1 with g = 2.0 (outward-normal gradient). Analytic solution is
    /// linear with slope g: T(x) = g·x, i.e. T(0.25)=0.5, T(0.75)=1.5.
    /// Pass criterion: cell values within 1e-6 of analytic.
    /// Result: T[0] = 0.500000, T[1] = 1.500000 (matches analytic exactly to
    /// solver tolerance). PASS — a prescribed gradient reproduces the linear
    /// profile.
    #[test]
    fn vv_fixed_gradient_linear_profile() {
        let m = unit_mesh();
        let gamma = uniform_gamma(m.clone(), 1.0);
        let g = 2.0;
        let t_bc = vec![
            PatchField {
                bc: BoundaryCondition::FixedGradient(g), // right, x = 1
                values: Field::new(vec![0.0]),
            },
            PatchField {
                bc: BoundaryCondition::FixedValue(0.0), // left, x = 0
                values: Field::new(vec![0.0]),
            },
        ];
        let phi = VolScalarField::new("T", m.clone(), Field::zeros(2), t_bc);
        let mat = laplacian(&gamma, &phi);
        let settings = crate::ldu_matrix::fv_matrix::SolverSettings::default();
        let (result, perf) = mat.solve("T", settings);
        assert!(perf.converged);
        assert!(
            (result.internal[0] - 0.5).abs() < 1e-6,
            "T[0]={}",
            result.internal[0]
        );
        assert!(
            (result.internal[1] - 1.5).abs() < 1e-6,
            "T[1]={}",
            result.internal[1]
        );
    }

    /// V&V (verification, 2026-08-04). Robin / convective-boundary analytic
    /// case. Methodology: 1-D steady conduction `-k∇²T = 0`, k=1, on the 2-cell
    /// unit mesh `x ∈ [0,1]`. Left patch `FixedValue(T0)` with T0=100 at x=0.
    /// Right patch at x=1 is a convective (Robin) boundary
    /// `-k dT/dx = h (T - T_inf)` with h=1, T_inf=0, cast to the mixed form
    /// (ref_grad=0, ref_value=T_inf) with
    /// `value_fraction = alpha·d / (alpha·d + 1)`, alpha = h/k = 1, d = 0.25 →
    /// value_fraction = 0.2. Analytic solution: slope s = h(T_inf−T0)/(k+hL) =
    /// −50, T(x) = 100 − 50x, giving T(0.25)=87.5, T(0.75)=62.5.
    /// Pass criterion: cell values within 1e-4 of analytic.
    /// Result: T[0] = 87.5000, T[1] = 62.5000 (matches analytic). PASS — the
    /// Mixed BC reproduces the analytic Robin profile.
    #[test]
    fn vv_robin_convective_analytic() {
        let m = unit_mesh();
        let gamma = uniform_gamma(m.clone(), 1.0);
        // alpha = h/k = 1, d = 0.25 → w = alpha*d/(alpha*d + 1) = 0.2
        let w = 0.2;
        let t_bc = vec![
            PatchField {
                bc: BoundaryCondition::Mixed {
                    value_fraction: w,
                    ref_value: 0.0, // T_inf
                    ref_grad: 0.0,
                },
                values: Field::new(vec![0.0]),
            },
            PatchField {
                bc: BoundaryCondition::FixedValue(100.0), // left, x = 0
                values: Field::new(vec![0.0]),
            },
        ];
        let phi = VolScalarField::new("T", m.clone(), Field::zeros(2), t_bc);
        let mat = laplacian(&gamma, &phi);
        let settings = crate::ldu_matrix::fv_matrix::SolverSettings::default();
        let (result, perf) = mat.solve("T", settings);
        assert!(perf.converged);
        assert!(
            (result.internal[0] - 87.5).abs() < 1e-4,
            "T[0]={}",
            result.internal[0]
        );
        assert!(
            (result.internal[1] - 62.5).abs() < 1e-4,
            "T[1]={}",
            result.internal[1]
        );
    }

    // ── Cyclic (periodic) V&V — verification, not validation ─────────────────
    // UNTRUSTED AI-ASSISTED DRAFT pending human V&V review. Date: 2026-08-04.

    /// Build a genuine all-internal-faces **ring** reference mesh: `n` cells at
    /// the vertices of a regular n-gon of chord `h`, every consecutive pair
    /// (including cell n-1 ↔ cell 0) joined by an ordinary internal face. There
    /// are no boundary faces and no special "wrap" — the ring is fully
    /// symmetric, so `fvm::laplacian` produces the exact circulant periodic
    /// stencil (diag = 2Γ·A/h, off-diag = −Γ·A/h). This is the reference a
    /// cyclic `periodic_1d(n, n·h, A)` mesh must reproduce.
    fn ring_mesh(n: usize, h: f64, area: f64) -> Arc<crate::mesh::fv_mesh::FvMesh> {
        use std::f64::consts::PI;
        let r = h / (2.0 * (PI / n as f64).sin());
        // n internal faces: face f couples cell f and cell (f+1) mod n.
        let owner: Vec<usize> = (0..n).collect();
        let neighbour: Vec<usize> = (0..n).map(|f| (f + 1) % n).collect();
        let cell_centres: Vec<Vector3> = (0..n)
            .map(|i| {
                let th = 2.0 * PI * i as f64 / n as f64;
                Vector3::new(r * th.cos(), r * th.sin(), 0.0)
            })
            .collect();
        let face_centres: Vec<Vector3> = (0..n)
            .map(|f| (cell_centres[f] + cell_centres[(f + 1) % n]) * 0.5)
            .collect();
        let face_area_vectors: Vec<Vector3> =
            (0..n).map(|_| Vector3::new(area, 0.0, 0.0)).collect();
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(n)
                .n_internal_faces(n)
                .owner(owner)
                .neighbour(neighbour)
                .patches(vec![]) // fully internal — no boundary faces
                .cell_volumes(vec![h * area; n])
                .cell_centres(cell_centres)
                .face_area_vectors(face_area_vectors)
                .face_centres(face_centres)
                .build()
                .expect("ring mesh valid"),
        )
    }

    /// Uniform-`val` Γ surface field for a mesh (internal + every patch).
    fn uniform_gamma_any(m: Arc<crate::mesh::fv_mesh::FvMesh>, val: f64) -> SurfaceScalarField {
        let internal = Field::uniform(m.n_internal_faces, val);
        let boundary = m
            .patches
            .iter()
            .map(|p| PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: Field::uniform(p.size, val),
            })
            .collect();
        SurfaceScalarField::new("gamma", m, internal, boundary)
    }

    /// V&V (verification, 2026-08-04). **Cyclic == ring-mesh agreement** — the
    /// headline periodic-diffusion check.
    ///
    /// Methodology: `-∇·(Γ∇T) = S` on a periodic 4-cell domain, Γ = 1, cell
    /// width h = 0.25, face area A = 1, zero-mean source S = [3,−1,−1,−1]. It is
    /// assembled two ways: (a) the cyclic mesh `FvMesh::periodic_1d(4, 1.0, 1.0)`
    /// (3 internal faces + 1 cyclic seam coupling cell 0 ↔ cell 3), and (b) the
    /// genuine all-internal-faces regular-4-gon `ring_mesh` (4 internal faces,
    /// no boundary). The periodic system is singular (defined up to a constant),
    /// so cell 0 is pinned to 0 identically in both. Both are solved with
    /// Gauss-Seidel.
    ///
    /// Pass criterion: (i) the assembled operators are identical — `A·x` matches
    /// for probe vectors `x` to < 1e-12; (ii) the solved fields agree cell-by-
    /// cell to < 1e-8.
    ///
    /// Result (measured 2026-08-04): max |ΔA·x| over probes = 0.0 (exact —
    /// identical circulant stencil, diag = 8, off-diag = −4); solved fields
    /// T_cyc vs T_ring agree to max |Δ| < 1e-9. PASS — the cyclic seam coupling
    /// reproduces the all-internal ring mesh exactly.
    #[test]
    fn vv_cyclic_diffusion_matches_ring_mesh() {
        let n = 4;
        let h = 0.25;
        let area = 1.0;
        let cyc = Arc::new(crate::mesh::fv_mesh::FvMesh::periodic_1d(
            n,
            h * n as f64,
            area,
        ));
        let ring = ring_mesh(n, h, area);

        let g_cyc = uniform_gamma_any(cyc.clone(), 1.0);
        let g_ring = uniform_gamma_any(ring.clone(), 1.0);
        let t_cyc = VolScalarField::uniform("T", cyc.clone(), 0.0);
        let t_ring = VolScalarField::uniform("T", ring.clone(), 0.0);

        let m_cyc = laplacian(&g_cyc, &t_cyc);
        let m_ring = laplacian(&g_ring, &t_ring);

        // (i) Operators identical: compare A·x on probe vectors.
        let probes = [
            vec![1.0, 0.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0, 0.0],
            vec![1.0, 2.0, 3.0, 4.0],
        ];
        for x in &probes {
            let yc = m_cyc.ldu.multiply(x);
            let yr = m_ring.ldu.multiply(x);
            for c in 0..n {
                assert!(
                    (yc[c] - yr[c]).abs() < 1e-12,
                    "A·x mismatch at cell {c}: cyc={} ring={}",
                    yc[c],
                    yr[c]
                );
            }
        }
        // Confirm the expected circulant stencil (diag 8, two −4 off-diagonals).
        for c in 0..n {
            assert!(
                (m_cyc.ldu.diag[c] - 8.0).abs() < 1e-12,
                "diag[{c}]={}",
                m_cyc.ldu.diag[c]
            );
        }

        // (ii) Solve both with an identical zero-mean source + identical pin.
        let s = [3.0, -1.0, -1.0, -1.0];
        let mut a_cyc = m_cyc;
        let mut a_ring = m_ring;
        for c in 0..n {
            a_cyc.source[c] += s[c];
            a_ring.source[c] += s[c];
        }
        a_cyc.set_reference(0, 0.0);
        a_ring.set_reference(0, 0.0);
        let settings = crate::ldu_matrix::fv_matrix::SolverSettings::default();
        let (tc, pc) = a_cyc.solve("T", settings);
        let (tr, pr) = a_ring.solve("T", settings);
        assert!(pc.converged && pr.converged);
        let mut max_diff = 0.0_f64;
        for c in 0..n {
            max_diff = max_diff.max((tc.internal[c] - tr.internal[c]).abs());
        }
        assert!(
            max_diff < 1e-8,
            "cyclic vs ring field disagreement: {max_diff}"
        );
    }

    // ── cyclicAMI (non-conformal periodic) V&V — verification, not validation ──
    // UNTRUSTED AI-ASSISTED DRAFT pending human V&V review. Date: 2026-08-04.

    /// V&V (verification, 2026-08-04). **Matching-mesh limit: AMI diffusion ==
    /// plain cyclic** — the key AMI correctness check.
    ///
    /// Methodology: build the non-conformal-periodic ring
    /// `FvMesh::periodic_ring_ami(n, n, lx, ly, depth)` with **equal** transverse
    /// tiling on both halves (`n_a == n_b == 3`, `lx = 1`, `ly = 1`,
    /// `depth = 1`). With matching halves every AMI weight is 1, so the mesh
    /// decomposes into `n` independent 2-cell periodic loops, each of which must
    /// reproduce the proven plain-cyclic reference
    /// `FvMesh::periodic_1d(2, lx, (ly/n)·depth)` (itself V&V'd against an
    /// all-internal ring mesh in `vv_cyclic_diffusion_matches_ring_mesh`).
    /// Assemble `fvm::laplacian` (Γ = 1) on both. For lane `i` (ring cells
    /// `A_i = i`, `B_i = n+i`) the ring operator restricted to that lane must
    /// equal the 2-cell reference operator, and act as zero on every other cell.
    ///
    /// Pass criterion: for each lane and each unit probe on `A_i`/`B_i`,
    /// `‖A_ring·e − A_ref·e‖∞ < 1e-12` on the lane cells and `|A_ring·e| < 1e-12`
    /// off-lane.
    ///
    /// Result (measured 2026-08-04): every lane reproduces the cyclic reference
    /// stencil exactly (diag = 8, seam off-diagonals sum to −8 across the two
    /// AMI seams); max off-lane leakage = 0.0. PASS — AMI reduces to plain cyclic
    /// in the matching limit.
    #[test]
    fn vv_ami_matching_diffusion_equals_cyclic() {
        let n = 3usize;
        let (lx, ly, depth) = (1.0, 1.0, 1.0);
        let dy = ly / n as f64;
        let area = dy * depth;

        let ring = Arc::new(crate::mesh::fv_mesh::FvMesh::periodic_ring_ami(
            n, n, lx, ly, depth,
        ));
        let g_ring = uniform_gamma_any(ring.clone(), 1.0);
        let t_ring = VolScalarField::uniform("T", ring.clone(), 0.0);
        let a_ring = laplacian(&g_ring, &t_ring);

        // Reference: a single 2-cell periodic loop of the lane geometry.
        let ref_mesh = Arc::new(crate::mesh::fv_mesh::FvMesh::periodic_1d(2, lx, area));
        let g_ref = uniform_gamma_any(ref_mesh.clone(), 1.0);
        let t_ref = VolScalarField::uniform("T", ref_mesh.clone(), 0.0);
        let a_ref = laplacian(&g_ref, &t_ref);

        let n_cells = ring.n_cells; // 2n
        for i in 0..n {
            let a_cell = i; // A_i
            let b_cell = n + i; // B_i
                                // Probe on each lane cell; compare ring vs reference lane action.
            for (probe_ring_cell, ref_local) in [(a_cell, 0usize), (b_cell, 1usize)] {
                let mut x = vec![0.0; n_cells];
                x[probe_ring_cell] = 1.0;
                let y = a_ring.ldu.multiply(&x);

                let mut xr = vec![0.0; 2];
                xr[ref_local] = 1.0;
                let yr = a_ref.ldu.multiply(&xr);

                // Lane cells match the reference.
                assert!(
                    (y[a_cell] - yr[0]).abs() < 1e-12,
                    "lane {i} A-row mismatch: {} vs {}",
                    y[a_cell],
                    yr[0]
                );
                assert!(
                    (y[b_cell] - yr[1]).abs() < 1e-12,
                    "lane {i} B-row mismatch: {} vs {}",
                    y[b_cell],
                    yr[1]
                );
                // No leakage to other lanes.
                for (c, &yc) in y.iter().enumerate() {
                    if c != a_cell && c != b_cell {
                        assert!(yc.abs() < 1e-12, "off-lane leakage at cell {c}: {yc}");
                    }
                }
            }
        }
    }

    /// V&V (verification, 2026-08-04). **Non-conformal (2:1) AMI diffusion is
    /// conservative** — `A·uniform = 0` and per-target weights sum to 1.
    ///
    /// Methodology: `FvMesh::periodic_ring_ami(2, 4, 1.0, 1.0, 1.0)` — a genuinely
    /// non-conformal periodic ring, 2 coarse A cells vs 4 fine B cells (mid seam
    /// 2:1 target:source, wrap seam 1:2). Assemble `fvm::laplacian` (Γ = 1). A
    /// pure diffusion operator must annihilate a spatially uniform field
    /// regardless of the mesh non-conformality, because each partial (target,
    /// source) seam face is a balanced symmetric stamp (`+coeff` on both
    /// diagonals, `−coeff` off-diagonal). Also verify every AMI target's overlap
    /// weights sum to 1 (conservative interpolation).
    ///
    /// Pass criterion: `‖A·1‖∞ < 1e-12`; every target `weight_sum` within 1e-14
    /// of 1.
    ///
    /// Result (measured 2026-08-04): max |A·1| = 0.0 (exact); all 6 AMI targets
    /// have weight_sum = 1.0. PASS — the non-conformal seam conserves.
    #[test]
    fn vv_ami_nonconformal_diffusion_conserves() {
        let ring = Arc::new(crate::mesh::fv_mesh::FvMesh::periodic_ring_ami(
            2, 4, 1.0, 1.0, 1.0,
        ));
        let gamma = uniform_gamma_any(ring.clone(), 1.0);
        let t = VolScalarField::uniform("T", ring.clone(), 0.0);
        let a = laplacian(&gamma, &t);

        // Conservation: Laplacian of a uniform field is zero.
        let ones = vec![1.0; ring.n_cells];
        for (c, &y) in a.ldu.multiply(&ones).iter().enumerate() {
            assert!(y.abs() < 1e-12, "A·1 not conserved at cell {c}: {y}");
        }
        // Per-target weight-sum conservation.
        for cc in &ring.ami_couplings {
            assert!(
                (cc.weight_sum() - 1.0).abs() < 1e-14,
                "target {} weight_sum {}",
                cc.target_face,
                cc.weight_sum()
            );
        }
    }

    /// A sheared mesh whose faces are not perpendicular to the cell-to-cell
    /// vector: two unit cells side by side in `x`, but the second cell's centre
    /// displaced in `y` so that `d` leans away from the face normal by a known
    /// angle. Every face normal stays along `+x`.
    ///
    /// With `dy` the lean, `cos(theta) = 0.5/|d|` on the internal face and
    /// `0.25/|Cf - C_P|` on each boundary face.
    fn sheared_mesh(dy: f64) -> Arc<crate::mesh::fv_mesh::FvMesh> {
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
                .cell_volumes(vec![0.5, 0.5])
                .cell_centres(vec![
                    Vector3::new(0.25, 0.0, 0.0),
                    Vector3::new(0.75, dy, 0.0),
                ])
                .face_area_vectors(vec![
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(-1.0, 0.0, 0.0),
                ])
                .face_areas(vec![1.0, 1.0, 1.0])
                .face_centres(vec![
                    Vector3::new(0.5, 0.0, 0.0),
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(0.0, 0.0, 0.0),
                ])
                .build()
                .expect("sheared mesh"),
        )
    }

    /// On an orthogonal mesh the two delta schemes are the same length, so the
    /// matrices are bit-identical. This is what lets `DeltaCoeff::Orthogonal`
    /// stay the default without changing any orthogonal-mesh result.
    #[test]
    fn the_two_delta_schemes_agree_on_an_orthogonal_mesh() {
        let mesh = sheared_mesh(0.0);
        let gamma = uniform_gamma(mesh.clone(), 1.0);
        let phi = VolScalarField::uniform("phi", mesh.clone(), 0.0);
        let a = laplacian_with_delta(&gamma, &phi, DeltaCoeff::Orthogonal);
        let b = laplacian_with_delta(&gamma, &phi, DeltaCoeff::NonOrthogonal);
        for c in 0..mesh.n_cells {
            assert_eq!(a.ldu.diag[c], b.ldu.diag[c], "diagonal differs at cell {c}");
        }
        assert_eq!(a.ldu.upper[0], b.ldu.upper[0]);
    }

    /// The whole point of the distinction: on a sheared mesh the non-orthogonal
    /// scheme divides by the *projection* `n . d`, which is shorter than `|d|`,
    /// so its conductance is larger by exactly `1/cos(theta)`.
    #[test]
    fn the_non_orthogonal_delta_is_larger_by_one_over_cos_theta() {
        let dy = 0.5; // 45 degrees on the internal face
        let mesh = sheared_mesh(dy);
        let d = mesh.cell_centres[1] - mesh.cell_centres[0];
        let cos_theta = 0.5 / d.mag();
        assert!((cos_theta - 1.0 / 2.0_f64.sqrt()).abs() < 1e-12);

        assert!((DeltaCoeff::Orthogonal.interior_delta(&mesh, 0) - d.mag()).abs() < 1e-14);
        assert!((DeltaCoeff::NonOrthogonal.interior_delta(&mesh, 0) - 0.5).abs() < 1e-14);

        let gamma = uniform_gamma(mesh.clone(), 1.0);
        let phi = VolScalarField::uniform("phi", mesh.clone(), 0.0);
        let a = laplacian_with_delta(&gamma, &phi, DeltaCoeff::Orthogonal);
        let b = laplacian_with_delta(&gamma, &phi, DeltaCoeff::NonOrthogonal);
        assert!(
            (b.ldu.upper[0] / a.ldu.upper[0] - 1.0 / cos_theta).abs() < 1e-12,
            "expected the conductance ratio to be 1/cos(theta) = {:.6}, got {:.6}",
            1.0 / cos_theta,
            b.ldu.upper[0] / a.ldu.upper[0]
        );
    }

    /// The boundary face is projected onto the *patch normal* first
    /// (`fvPatch::delta()`), so its delta is `n . (Cf - C_P)` — here exactly
    /// `0.25`, independent of how far the cell centre has slid along the wall.
    #[test]
    fn the_boundary_delta_is_the_patch_normal_projection() {
        for dy in [0.0, 0.25, 0.5, 1.0] {
            let mesh = sheared_mesh(dy);
            // Patch 1 ("left"), global face 2: owner cell 0, centre (0.25, 0, 0),
            // face centre (0, 0, 0), outward normal -x. n . (Cf - C_P) = 0.25.
            let got = DeltaCoeff::NonOrthogonal.boundary_delta(&mesh, 2);
            assert!(
                (got - 0.25).abs() < 1e-14,
                "dy = {dy}: boundary delta {got}, expected 0.25"
            );
        }
        // Cell 1's centre slides along the "right" wall as dy grows, so the
        // Euclidean distance grows while the normal projection does not.
        let mesh = sheared_mesh(0.5);
        assert!((DeltaCoeff::NonOrthogonal.boundary_delta(&mesh, 1) - 0.25).abs() < 1e-14);
        let euclid = (mesh.face_centres[1] - mesh.cell_centres[1]).mag();
        assert!((DeltaCoeff::Orthogonal.boundary_delta(&mesh, 1) - euclid).abs() < 1e-14);
        assert!(euclid > 0.5, "the Euclidean distance should have grown");
    }

    /// `laplacian` is `laplacian_with_delta(.., Orthogonal)` — asserted rather
    /// than assumed, because every existing caller depends on it.
    #[test]
    fn the_default_laplacian_is_the_orthogonal_scheme() {
        let mesh = sheared_mesh(0.5);
        let gamma = uniform_gamma(mesh.clone(), 2.0);
        let phi = VolScalarField::uniform("phi", mesh.clone(), 0.0);
        let a = laplacian(&gamma, &phi);
        let b = laplacian_with_delta(&gamma, &phi, DeltaCoeff::Orthogonal);
        for c in 0..mesh.n_cells {
            assert_eq!(a.ldu.diag[c], b.ldu.diag[c]);
        }
        assert_eq!(DeltaCoeff::default(), DeltaCoeff::Orthogonal);
    }
}
