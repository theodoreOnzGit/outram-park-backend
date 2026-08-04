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

/// Implicit first-order upwind convection: assembles the matrix for `∇·(φ·ψ)`.
///
/// `phi` is the face flux field (SurfaceScalarField); `psi` is the transported
/// scalar (VolScalarField). The upwind scheme selects the donor cell:
///
/// - `φ_f ≥ 0`: flux comes from the **owner** cell → coefficient on `diag[O]`.
/// - `φ_f < 0`: flux comes from the **neighbour** cell → coefficient on `upper[f]`.
///
/// ## Boundary conditions
///
/// - `ZeroGradient` / `Symmetry`: boundary value equals owner cell → flux goes
///   entirely to `diag[owner]` regardless of sign.
/// - `FixedValue(v)`: inflow (`φ_f < 0`) uses the fixed value → explicit source;
///   outflow (`φ_f ≥ 0`) remains on the diagonal (upwind from owner).
///
/// ## Cyclic (periodic) patches
///
/// [`PatchKind::Cyclic`] patches are assembled as internal-like faces across the
/// periodic seam (see the cyclic-coupling loop): the seam flux `φ_seam` is the
/// outward face flux on the half0 patch, and first-order upwind selects the
/// owner cell on outflow (`φ_seam ≥ 0`) or the partner cell across the seam on
/// inflow (`φ_seam < 0`). This makes a periodic loop conservative — a uniform
/// field advects around it unchanged. Mirrors `Foam::cyclicFvPatchField`.
///
/// ## Non-conformal periodic (cyclicAMI) patches
///
/// [`PatchKind::CyclicAmi`](crate::mesh::PatchKind::CyclicAmi) patches split each
/// target seam face's outward flux `φ_target` across its overlapping sources by
/// the overlap weight: pair `(target, source_k)` carries the sub-flux
/// `φ_target · weight_k`, upwinded like an internal face. Because the weights
/// sum to 1 the full face flux is preserved, so a uniform field advects around a
/// periodic loop unchanged (`A·uniform = 0`); the matching limit reduces exactly
/// to the plain cyclic seam. Mirrors `Foam::cyclicAMIFvPatchField`.
pub fn div(phi: &SurfaceScalarField, psi: &VolScalarField) -> FvMatrix {
    let mesh = psi.mesh.clone();
    let mut mat = FvMatrix::new(mesh.clone());

    // Internal faces: upwind
    for f in 0..mesh.n_internal_faces {
        let o = mesh.owner[f];
        let n = mesh.neighbour[f];
        let phi_f = phi.internal[f];

        // Owner row O: outflow contributes diag, inflow contributes upper (N column)
        mat.ldu.diag[o] += phi_f.max(0.0);
        mat.ldu.upper[f] += phi_f.min(0.0);

        // Neighbour row N: inflow from O contributes diag, outflow contributes lower (O column)
        mat.ldu.diag[n] -= phi_f.min(0.0);
        mat.ldu.lower[f] -= phi_f.max(0.0);
    }

    // Boundary faces
    for (pi, patch) in mesh.patches.iter().enumerate() {
        // Cyclic / cyclicAMI patches are handled as internal-like seam couplings
        // below.
        if patch.kind == PatchKind::Cyclic || patch.kind == PatchKind::CyclicAmi {
            continue;
        }
        for fi in 0..patch.size {
            let owner = mesh.owner[patch.start + fi];
            let phi_f = phi.boundary[pi].values[fi];
            match &psi.boundary[pi].bc {
                BoundaryCondition::ZeroGradient | BoundaryCondition::Symmetry => {
                    // psi_face = psi_owner (zero gradient) → always on diagonal
                    mat.ldu.diag[owner] += phi_f;
                }
                BoundaryCondition::FixedValue(v) => {
                    if phi_f >= 0.0 {
                        // Outflow: upwind donor is owner cell
                        mat.ldu.diag[owner] += phi_f;
                    } else {
                        // Inflow: known boundary value → explicit
                        mat.source[owner] -= phi_f * v;
                    }
                }
                BoundaryCondition::FixedField(ff) => {
                    if phi_f >= 0.0 {
                        mat.ldu.diag[owner] += phi_f;
                    } else {
                        mat.source[owner] -= phi_f * ff[fi];
                    }
                }
                // Flux-switched: inletOutlet = fixedValue(inletValue) on inflow
                // (phi_f < 0), zeroGradient on outflow (phi_f ≥ 0). outletInlet
                // is the mirror. The switch uses the sign of the outward face
                // flux phi_f = U·Sf, decided here where the flux is available.
                BoundaryCondition::InletOutlet { inlet_value } => {
                    if phi_f >= 0.0 {
                        mat.ldu.diag[owner] += phi_f; // outflow: zeroGradient
                    } else {
                        mat.source[owner] -= phi_f * inlet_value; // inflow: fixedValue
                    }
                }
                BoundaryCondition::OutletInlet { outlet_value } => {
                    if phi_f >= 0.0 {
                        mat.source[owner] -= phi_f * outlet_value; // outflow: fixedValue
                    } else {
                        mat.ldu.diag[owner] += phi_f; // inflow: zeroGradient
                    }
                }
                // Zero-gradient-like for convection (Slip/Wedge/NoSlip/Empty/
                // FixedGradient/Mixed): upwind donor is the owner cell on
                // outflow; inflow carries no known explicit value here.
                _ => {
                    if phi_f >= 0.0 {
                        mat.ldu.diag[owner] += phi_f;
                    }
                }
            }
        }
    }

    // Cyclic (periodic) seam couplings: first-order upwind across the seam, the
    // partner owner cell acting as the neighbour.
    for (i, cc) in mesh.cyclic_couplings.iter().enumerate() {
        let cf = mesh.cyclic_coupling_face(i);
        let o = cc.owner;
        let nb = cc.neighbour;
        // Outward seam flux on the half0 face points owner→neighbour across the
        // seam, matching the internal owner→neighbour convention.
        let phi_f = phi.boundary[cc.patch_a].values[cc.local];
        mat.ldu.diag[o] += phi_f.max(0.0);
        mat.ldu.upper[cf] += phi_f.min(0.0);
        mat.ldu.diag[nb] -= phi_f.min(0.0);
        mat.ldu.lower[cf] -= phi_f.max(0.0);
    }

    // Non-conformal (cyclicAMI) seam couplings: first-order upwind across each
    // partial (target, source) overlap. The target seam face's outward flux
    // φ_target is split by the overlap weight — the pair carries the sub-flux
    // `φ_target · weight` (the flux through that overlap patch) — and upwind
    // selects the target owner on outflow or the source cell on inflow. Because
    // the weights sum to 1, the split preserves the full face flux, so a uniform
    // field advects around a periodic loop unchanged (`A·uniform = 0`). Matching
    // limit (weight 1) reduces exactly to the plain cyclic seam.
    let mut cf = mesh.ami_ldu_start();
    for coupling in &mesh.ami_couplings {
        let o = coupling.target_cell;
        let phi_target = phi.boundary[coupling.target_patch].values[coupling.local];
        for w in &coupling.weights {
            let nb = w.source_cell;
            let phi_f = phi_target * w.weight;
            mat.ldu.diag[o] += phi_f.max(0.0);
            mat.ldu.upper[cf] += phi_f.min(0.0);
            mat.ldu.diag[nb] -= phi_f.min(0.0);
            mat.ldu.lower[cf] -= phi_f.max(0.0);
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

    fn make_phi(
        m: Arc<crate::mesh::fv_mesh::FvMesh>,
        internal: f64,
        bnd: f64,
    ) -> SurfaceScalarField {
        let n_int = m.n_internal_faces;
        let bnd_vals: Vec<_> = m
            .patches
            .iter()
            .map(|p| PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: Field::uniform(p.size, bnd),
            })
            .collect();
        SurfaceScalarField::new("phi", m, Field::uniform(n_int, internal), bnd_vals)
    }

    #[test]
    fn upwind_positive_flux_on_diagonal() {
        // phi_f > 0: donor is owner → only diag[O] += phi, upper unchanged
        let m = unit_mesh();
        let phi = make_phi(m.clone(), 1.0, 0.0);
        let psi = VolScalarField::uniform("psi", m.clone(), 0.0);
        let mat = div(&phi, &psi);
        // internal face: diag[0] += 1, lower[0] -= 1; diag[1] -= 0, upper[0] += 0
        assert!(
            (mat.ldu.diag[0] - 1.0).abs() < 1e-12,
            "diag[0]={}",
            mat.ldu.diag[0]
        );
        assert!((mat.ldu.upper[0] - 0.0).abs() < 1e-12);
        assert!((mat.ldu.diag[1] - 0.0).abs() < 1e-12);
        assert!(
            (mat.ldu.lower[0] - (-1.0)).abs() < 1e-12,
            "lower[0]={}",
            mat.ldu.lower[0]
        );
    }

    #[test]
    fn upwind_negative_flux_on_upper() {
        // phi_f < 0: donor is neighbour → only upper[f] += phi (negative)
        let m = unit_mesh();
        let phi = make_phi(m.clone(), -1.0, 0.0);
        let psi = VolScalarField::uniform("psi", m.clone(), 0.0);
        let mat = div(&phi, &psi);
        assert!((mat.ldu.upper[0] - (-1.0)).abs() < 1e-12);
        assert!((mat.ldu.diag[0] - 0.0).abs() < 1e-12);
        assert!((mat.ldu.diag[1] - 1.0).abs() < 1e-12);
    }

    /// Build phi with a per-patch boundary flux (right patch = `right_phi`,
    /// left patch = 0) and zero internal flux, to isolate the right patch.
    fn phi_right_only(m: Arc<crate::mesh::fv_mesh::FvMesh>, right_phi: f64) -> SurfaceScalarField {
        let bnd = vec![
            PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: Field::new(vec![right_phi]), // patch 0 = "right"
            },
            PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: Field::new(vec![0.0]), // patch 1 = "left"
            },
        ];
        SurfaceScalarField::new("phi", m.clone(), Field::uniform(m.n_internal_faces, 0.0), bnd)
    }

    /// V&V (verification, 2026-08-04). inletOutlet flux switch. Methodology:
    /// isolate the right patch (owner = cell 1) of the 2-cell unit mesh, zero
    /// internal and left-patch flux, and set the scalar field's right patch to
    /// `InletOutlet { inlet_value: 5.0 }`. Assemble `fvm::div` twice:
    /// - outflow (phi_f = +2 ≥ 0): must act as zeroGradient → `diag[1] += 2`,
    ///   `source[1]` unchanged (0);
    /// - inflow  (phi_f = −2 < 0): must act as fixedValue → `source[1] -=
    ///   phi_f·inlet_value = 10`, `diag[1]` unchanged (0).
    /// Pass criterion: both assembled entries match to < 1e-12.
    /// Result: outflow diag[1] = 2.000000, source[1] = 0; inflow diag[1] = 0,
    /// source[1] = 10.000000. PASS — the switch flips with the flux sign.
    #[test]
    fn vv_inlet_outlet_flux_switch() {
        let m = unit_mesh();
        let make_psi = || {
            let bc = vec![
                PatchField {
                    bc: BoundaryCondition::InletOutlet { inlet_value: 5.0 },
                    values: Field::new(vec![0.0]),
                },
                PatchField {
                    bc: BoundaryCondition::ZeroGradient,
                    values: Field::new(vec![0.0]),
                },
            ];
            VolScalarField::new("psi", m.clone(), Field::zeros(2), bc)
        };

        // Outflow: zeroGradient behaviour.
        let out = div(&phi_right_only(m.clone(), 2.0), &make_psi());
        assert!((out.ldu.diag[1] - 2.0).abs() < 1e-12, "diag[1]={}", out.ldu.diag[1]);
        assert!(out.source[1].abs() < 1e-12, "source[1]={}", out.source[1]);

        // Inflow: fixedValue behaviour, source[1] -= phi_f * inlet_value = 10.
        let inn = div(&phi_right_only(m.clone(), -2.0), &make_psi());
        assert!(inn.ldu.diag[1].abs() < 1e-12, "diag[1]={}", inn.ldu.diag[1]);
        assert!((inn.source[1] - 10.0).abs() < 1e-12, "source[1]={}", inn.source[1]);
    }

    // ── Cyclic (periodic) V&V — verification, not validation ─────────────────
    // UNTRUSTED AI-ASSISTED DRAFT pending human V&V review. Date: 2026-08-04.

    /// All-internal regular-n-gon ring reference mesh (see the identical helper
    /// in `laplacian.rs` for the rationale): `n` cells, `n` internal faces, face
    /// `f` couples cell `f` and cell `(f+1) mod n`, no boundary faces.
    fn ring_mesh(n: usize, h: f64, area: f64) -> Arc<crate::mesh::fv_mesh::FvMesh> {
        use std::f64::consts::PI;
        let r = h / (2.0 * (PI / n as f64).sin());
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
        let face_area_vectors: Vec<Vector3> = (0..n).map(|_| Vector3::new(area, 0.0, 0.0)).collect();
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(n)
                .n_internal_faces(n)
                .owner(owner)
                .neighbour(neighbour)
                .patches(vec![])
                .cell_volumes(vec![h * area; n])
                .cell_centres(cell_centres)
                .face_area_vectors(face_area_vectors)
                .face_centres(face_centres)
                .build()
                .expect("ring mesh valid"),
        )
    }

    /// V&V (verification, 2026-08-04). **Periodic advection: conservation + the
    /// uniform field advects unchanged, and cyclic == ring-mesh.**
    ///
    /// Methodology: a uniform circulating flux Φ = 1 (mass flux `U·Sf`) around a
    /// 4-cell periodic loop, h = 0.25, A = 1. The cyclic mesh
    /// `periodic_1d(4, 1.0, 1.0)` carries Φ on its 3 internal faces and the seam
    /// flux on the left/right cyclic patches (`φ_left = −Φ`, `φ_right = +Φ`, the
    /// outward `U·Sf`). The all-internal `ring_mesh` carries Φ on all 4 faces.
    /// First-order-upwind `fvm::div` is assembled on both.
    ///
    /// Pass criteria: (i) **conservation** — a uniform field is unchanged by the
    /// convection operator, `A·[1,1,1,1] = 0` (net seam flux balances so the
    /// loop closes); (ii) **cyclic == ring** — the assembled operators are
    /// identical, `A·x` matching for probe vectors to < 1e-12.
    ///
    /// Result (measured 2026-08-04): max |A·1| = 0.0 (exact — inflow Φ balances
    /// outflow Φ in every cell, including across the seam); max |ΔA·x| over
    /// probes = 0.0 (identical circulant upwind stencil). PASS.
    #[test]
    fn vv_cyclic_advection_conserves_and_matches_ring() {
        let n = 4;
        let h = 0.25;
        let area = 1.0;
        let phi_mag = 1.0;
        let cyc = Arc::new(crate::mesh::fv_mesh::FvMesh::periodic_1d(n, h * n as f64, area));
        let ring = ring_mesh(n, h, area);

        // Cyclic flux: uniform Φ on internal faces; seam flux is the outward
        // U·Sf on each half (left normal −x → −Φ, right normal +x → +Φ).
        let phi_cyc = SurfaceScalarField::new(
            "phi",
            cyc.clone(),
            Field::uniform(cyc.n_internal_faces, phi_mag),
            vec![
                PatchField {
                    bc: BoundaryCondition::ZeroGradient,
                    values: Field::new(vec![-phi_mag]), // left patch, Sf = −x
                },
                PatchField {
                    bc: BoundaryCondition::ZeroGradient,
                    values: Field::new(vec![phi_mag]), // right patch, Sf = +x
                },
            ],
        );
        // Ring flux: uniform Φ on all internal faces (consistent circulation).
        let phi_ring = SurfaceScalarField::new(
            "phi",
            ring.clone(),
            Field::uniform(ring.n_internal_faces, phi_mag),
            vec![],
        );

        let psi_cyc = VolScalarField::uniform("psi", cyc.clone(), 0.0);
        let psi_ring = VolScalarField::uniform("psi", ring.clone(), 0.0);
        let m_cyc = div(&phi_cyc, &psi_cyc);
        let m_ring = div(&phi_ring, &psi_ring);

        // (i) Conservation: uniform field unchanged around the loop.
        let ones = vec![1.0; n];
        let ay = m_cyc.ldu.multiply(&ones);
        for c in 0..n {
            assert!(ay[c].abs() < 1e-12, "A·1 not conserved at cell {c}: {}", ay[c]);
        }

        // (ii) cyclic == ring: identical operators.
        let probes = [
            vec![1.0, 0.0, 0.0, 0.0],
            vec![0.0, 0.0, 1.0, 0.0],
            vec![1.0, 2.0, 3.0, 4.0],
        ];
        for x in &probes {
            let yc = m_cyc.ldu.multiply(x);
            let yr = m_ring.ldu.multiply(x);
            for c in 0..n {
                assert!(
                    (yc[c] - yr[c]).abs() < 1e-12,
                    "div A·x mismatch at cell {c}: cyc={} ring={}",
                    yc[c],
                    yr[c]
                );
            }
        }
    }

    // ── cyclicAMI (non-conformal periodic) advection V&V — verification ────────
    // UNTRUSTED AI-ASSISTED DRAFT pending human V&V review. Date: 2026-08-04.

    /// Build the outward face-flux field `φ = U·Sf` for a **uniform** velocity
    /// `U = (u, 0, 0)` on a `periodic_ring_ami` mesh: every boundary face carries
    /// `u · Sf_x`. Internal-face flux is empty (the ring has none).
    fn ring_uniform_phi(m: Arc<crate::mesh::fv_mesh::FvMesh>, u: f64) -> SurfaceScalarField {
        let boundary = m
            .patches
            .iter()
            .map(|p| {
                let vals: Vec<f64> = (0..p.size)
                    .map(|fi| u * m.face_area_vectors[p.start + fi].x)
                    .collect();
                PatchField {
                    bc: BoundaryCondition::ZeroGradient,
                    values: Field::new(vals),
                }
            })
            .collect();
        SurfaceScalarField::new("phi", m.clone(), Field::uniform(m.n_internal_faces, 0.0), boundary)
    }

    /// V&V (verification, 2026-08-04). **Non-conformal (2:1) AMI advection is
    /// conservative** — a uniform field advects around the periodic loop
    /// unchanged, `A·uniform = 0`.
    ///
    /// Methodology: `FvMesh::periodic_ring_ami(2, 4, 1.0, 1.0, 1.0)` (2 coarse A
    /// cells vs 4 fine B cells; mid seam 2:1, wrap seam 1:2). Impose a uniform
    /// velocity `U = (1, 0, 0)` as the face flux `φ = U·Sf` on every seam face,
    /// then assemble first-order-upwind `fvm::div`. Each cell has equal in-/out-
    /// seam face area, so the discrete flux is divergence-free per cell; because
    /// each target face's flux is split by weights that sum to 1, the full flux
    /// is preserved across the non-conformal seam and the loop closes.
    ///
    /// Pass criterion: `‖A·1‖∞ < 1e-12`.
    ///
    /// Result (measured 2026-08-04): max |A·1| = 0.0 (exact) — inflow balances
    /// outflow in every cell across the 2:1 non-conformal seam. PASS.
    #[test]
    fn vv_ami_nonconformal_advection_conserves() {
        let ring = Arc::new(crate::mesh::fv_mesh::FvMesh::periodic_ring_ami(2, 4, 1.0, 1.0, 1.0));
        let phi = ring_uniform_phi(ring.clone(), 1.0);
        let psi = VolScalarField::uniform("psi", ring.clone(), 0.0);
        let mat = div(&phi, &psi);

        let ones = vec![1.0; ring.n_cells];
        for (c, &y) in mat.ldu.multiply(&ones).iter().enumerate() {
            assert!(y.abs() < 1e-12, "advection A·1 not conserved at cell {c}: {y}");
        }
    }

    /// V&V (verification, 2026-08-04). **Matching-mesh AMI advection is
    /// conservative** (sanity companion to the diffusion matching==cyclic check).
    ///
    /// Methodology: `FvMesh::periodic_ring_ami(3, 3, 1.0, 1.0, 1.0)` — conformal
    /// halves, so every AMI weight is 1 and each lane is a plain 2-cell periodic
    /// loop. Uniform `U = (1,0,0)` face flux, `fvm::div` assembled. A uniform
    /// field must be unchanged.
    /// Pass criterion: `‖A·1‖∞ < 1e-12`.
    /// Result (measured 2026-08-04): max |A·1| = 0.0 (exact). PASS.
    #[test]
    fn vv_ami_matching_advection_conserves() {
        let ring = Arc::new(crate::mesh::fv_mesh::FvMesh::periodic_ring_ami(3, 3, 1.0, 1.0, 1.0));
        let phi = ring_uniform_phi(ring.clone(), 1.0);
        let psi = VolScalarField::uniform("psi", ring.clone(), 0.0);
        let mat = div(&phi, &psi);
        let ones = vec![1.0; ring.n_cells];
        for (c, &y) in mat.ldu.multiply(&ones).iter().enumerate() {
            assert!(y.abs() < 1e-12, "advection A·1 not conserved at cell {c}: {y}");
        }
    }
}
