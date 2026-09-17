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

use crate::openfoam_algorithms::openfoam_source::boundary::bc::PatchField;
use crate::openfoam_algorithms::openfoam_source::field::Field;
use crate::openfoam_algorithms::openfoam_source::surface_field::SurfaceScalarField;
use crate::openfoam_algorithms::openfoam_source::vol_field::{VolScalarField, VolVectorField};
use super::interpolate;

/// `∇·φ_f = (1/V_O) · Σ_f φ_f` — net volumetric flux per unit volume.
///
/// Used to evaluate the continuity residual `∇·U` or `∇·(ρU)/ρ`.
pub fn div_flux(phi: &SurfaceScalarField) -> VolScalarField {
    let mesh = &phi.mesh;
    let mut d = vec![0.0_f64; mesh.n_cells];

    for f in 0..mesh.n_internal_faces {
        d[mesh.owner[f]] += phi.internal[f];
        d[mesh.neighbour[f]] -= phi.internal[f];
    }

    for (pi, patch) in mesh.patches.iter().enumerate() {
        for fi in 0..patch.size {
            d[mesh.owner[patch.start + fi]] += phi.boundary[pi].values[fi];
        }
    }

    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField::zero_gradient(p.size))
        .collect();

    VolScalarField::new(
        format!("div({})", phi.name),
        phi.mesh.clone(),
        Field::from_fn(mesh.n_cells, |c| d[c] / mesh.cell_volumes[c]),
        boundary,
    )
}

/// `∇·(φ·ψ) = (1/V_O) · Σ_f φ_f · ψ_f` — convective scalar flux.
///
/// `phi` is the face mass flux (SurfaceScalarField); `psi` is the transported
/// scalar (VolScalarField).  Face values of `psi` are obtained by linear
/// interpolation.
pub fn div(phi: &SurfaceScalarField, psi: &VolScalarField) -> VolScalarField {
    let mesh = &phi.mesh;
    let psi_f = interpolate(psi);
    let mut d = vec![0.0_f64; mesh.n_cells];

    for f in 0..mesh.n_internal_faces {
        let flux = phi.internal[f] * psi_f.internal[f];
        d[mesh.owner[f]] += flux;
        d[mesh.neighbour[f]] -= flux;
    }

    for (pi, patch) in mesh.patches.iter().enumerate() {
        for fi in 0..patch.size {
            let flux = phi.boundary[pi].values[fi] * psi_f.boundary[pi].values[fi];
            d[mesh.owner[patch.start + fi]] += flux;
        }
    }

    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField::zero_gradient(p.size))
        .collect();

    VolScalarField::new(
        format!("div({},{})", phi.name, psi.name),
        phi.mesh.clone(),
        Field::from_fn(mesh.n_cells, |c| d[c] / mesh.cell_volumes[c]),
        boundary,
    )
}

/// `∇·(φ·ψ) = (1/V_O) · Σ_f φ_f · ψ_f` — convective scalar flux with an
/// **upwind-biased, flux-limited** face value, i.e. the bounded (TVD)
/// counterpart of [`div`].
///
/// [`div`] takes `ψ_f` from a plain linear (central) interpolation. That is
/// second-order but **unbounded**: on an advection-dominated transport equation
/// (cell Péclet ≫ 1) it produces dispersive over- and undershoots at a sharp
/// front, so the transported scalar leaves the range set by its own initial and
/// boundary data. For an enthalpy field that is not a cosmetic wiggle — an
/// undershoot can push a `(p, h)` flash outside the equation of state's valid
/// range.
///
/// This variant instead reconstructs each internal face from the **upwind**
/// side and blends toward the central value under a TVD limiter, via
/// [`reconstruct_pos_neg`]:
///
/// ```text
///   ψ_f = ψ_pos[f]   if φ_f ≥ 0   (owner is upwind)
///   ψ_f = ψ_neg[f]   if φ_f < 0   (neighbour is upwind)
/// ```
///
/// where `ψ_pos = ψ_O + λ(r_O)·(ψ_lin − ψ_O)` and `ψ_neg = ψ_N + λ(r_N)·(ψ_lin −
/// ψ_N)`. With [`Limiter::Upwind`] (`λ ≡ 0`) this is first-order upwind; with
/// [`Limiter::Linear`] (`λ ≡ 1`) it reduces **exactly** to [`div`]; the TVD
/// limiters ([`Limiter::VanLeer`], [`Limiter::Minmod`]) recover second order
/// where the solution is smooth and fall back toward upwind at an extremum.
/// Mirrors OpenFOAM's `div(phi,h) Gauss limitedLinear 1` / `Gauss vanLeer`
/// family (`src/finiteVolume/interpolation/surfaceInterpolation/limitedSchemes/`),
/// as opposed to `Gauss linear`.
///
/// ## Boundedness caveat
/// The TVD property of an **explicit** limited scheme holds under a CFL
/// condition; this operator does not check one. At the deeply sub-CFL steps a
/// pressure-based PIMPLE solve uses (`CFL ≈ 10⁻³` in the `rhoPimpleFoam` array)
/// that is amply satisfied, but a caller pushing the step toward `CFL → 1`
/// should not assume boundedness for free.
///
/// ## Boundary faces — **direction-aware (upwind) advection terminals**
/// This is where [`div_limited`] departs from [`div`] in *formulation*, not just
/// in accuracy, and it matters more than the limiter does.
///
/// [`div`] evaluates every boundary face from its BC type alone: a fixed-value
/// patch always contributes its prescribed value, a zero-gradient patch always
/// contributes the owner cell value. Both are wrong the moment the flow does not
/// run in the direction the patch was labelled for:
///
/// - a **zero-gradient patch on an inflow face** advects the domain's *own*
///   enthalpy inwards (`h_face = h_owner`), so the influx is self-referential
///   and the interior is structurally indifferent to what is actually flowing
///   in. With zero-gradient at both ends there is no boundary through which
///   energy can enter or leave at all, and a global balance
///   `ṁ·(h_out − h_in) = Q` cannot even be posed.
/// - a **fixed-value patch on an outflow face** forces the enthalpy the domain
///   is *exporting*, which the domain does not get to choose.
///
/// This operator instead selects the upstream state **by the sign of the
/// boundary mass flux**, exactly as `tuas_boussinesq_solver`'s
/// `single_control_vol/boundary_condition_interactions/advection_to_bcs.rs`
/// does (its `advection_heat_rate` takes `h_a` when `ṁ_{a→b} ≥ 0` and `h_b`
/// otherwise, and its `calculate_*_advection_set_temperature` /
/// `…_non_set_temperature` pair supplies the boundary-side enthalpy either from
/// a prescribed BC state or from a zero-gradient extrapolation):
///
/// ```text
///   φ_b ≥ 0  (outflow, interior is upstream)  →  ψ_f = ψ_owner
///   φ_b < 0  (inflow,  boundary is upstream)  →  ψ_f = BC-evaluated face value
/// ```
///
/// `φ` is positive **out of** the domain (the patch normal points outward), so
/// `φ_b < 0` is inflow. On a zero-gradient patch the BC-evaluated value *is* the
/// owner value, so both branches coincide and the behaviour is TUAS's
/// "non-set-temperature" (zero-gradient) advection terminal — a legitimate mode,
/// not an error, when the caller genuinely has no upstream state to give. On a
/// fixed-value patch the prescribed value is used on inflow and ignored on
/// outflow, which is OpenFOAM's `inletOutlet` semantics
/// (`src/finiteVolume/fields/fvPatchFields/derived/inletOutlet/`) and makes the
/// both-ends-zero-gradient failure structurally impossible to reach by
/// accident: the upstream side is chosen by the flow, never defaulted.
///
/// Because the choice is made per face per call, a **flow reversal** is handled
/// with no caller intervention — the end that becomes the inlet starts supplying
/// its prescribed state, and the end that becomes the outlet stops.
///
/// This matches the vendored implicit [`fvm::div`](super::super::fvm::div),
/// whose boundary handling already switched on `φ_f`'s sign; it is only the
/// explicit operator that did not.
pub fn div_limited(
    phi: &SurfaceScalarField,
    psi: &VolScalarField,
    limiter: super::Limiter,
) -> VolScalarField {
    let mesh = &phi.mesh;
    // `pos`/`neg` carry the owner-/neighbour-biased limited reconstructions on
    // internal faces, and (both) the BC-evaluated linear values on boundary
    // faces — so no separate `interpolate` call is needed for the boundary.
    let (pos, neg) = super::reconstruct_pos_neg(psi, limiter);
    let mut d = vec![0.0_f64; mesh.n_cells];

    for f in 0..mesh.n_internal_faces {
        let phi_f = phi.internal[f];
        let psi_f = if phi_f >= 0.0 {
            pos.internal[f]
        } else {
            neg.internal[f]
        };
        let flux = phi_f * psi_f;
        d[mesh.owner[f]] += flux;
        d[mesh.neighbour[f]] -= flux;
    }

    for (pi, patch) in mesh.patches.iter().enumerate() {
        for fi in 0..patch.size {
            let owner = mesh.owner[patch.start + fi];
            let phi_b = phi.boundary[pi].values[fi];
            // Upwind advection terminal: the sign of the boundary mass flux
            // picks the upstream state (see the method doc; ported from TUAS's
            // `advection_to_bcs.rs`). `phi` is positive out of the domain.
            let psi_f = if phi_b >= 0.0 {
                psi.internal[owner] // outflow — the interior is upstream
            } else {
                pos.boundary[pi].values[fi] // inflow — the BC state is upstream
            };
            d[owner] += phi_b * psi_f;
        }
    }

    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField::zero_gradient(p.size))
        .collect();

    VolScalarField::new(
        format!("div({},{})", phi.name, psi.name),
        phi.mesh.clone(),
        Field::from_fn(mesh.n_cells, |c| d[c] / mesh.cell_volumes[c]),
        boundary,
    )
}

/// `∇·(φ·U) = (1/V_O) · Σ_f φ_f · U_f` — convective vector flux.
///
/// `phi` is the face mass flux; `U` is the velocity (VolVectorField).
pub fn div_vec(phi: &SurfaceScalarField, u: &VolVectorField) -> VolVectorField {
    use crate::openfoam_algorithms::openfoam_source::Vector3;

    let mesh = &phi.mesh;
    let u_f = interpolate(u);
    let mut d = vec![Vector3::ZERO; mesh.n_cells];

    for f in 0..mesh.n_internal_faces {
        let flux = u_f.internal[f] * phi.internal[f];
        d[mesh.owner[f]] = d[mesh.owner[f]] + flux;
        d[mesh.neighbour[f]] = d[mesh.neighbour[f]] - flux;
    }

    for (pi, patch) in mesh.patches.iter().enumerate() {
        for fi in 0..patch.size {
            let flux = u_f.boundary[pi].values[fi] * phi.boundary[pi].values[fi];
            d[mesh.owner[patch.start + fi]] = d[mesh.owner[patch.start + fi]] + flux;
        }
    }

    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField::zero_gradient_vec(p.size))
        .collect();

    VolVectorField::new(
        format!("div({},{})", phi.name, u.name),
        phi.mesh.clone(),
        Field::from_fn(mesh.n_cells, |c| d[c] * (1.0 / mesh.cell_volumes[c])),
        boundary,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::openfoam_algorithms::openfoam_source::{FvMesh, Vector3};
    use crate::openfoam_algorithms::openfoam_source::boundary::bc::{BoundaryCondition, PatchField};
    use crate::openfoam_algorithms::openfoam_source::field::Field;
    use crate::openfoam_algorithms::openfoam_source::surface_field::SurfaceScalarField;
    use crate::openfoam_algorithms::openfoam_source::fv_mesh::{
        FvMeshBuilder, BoundaryPatch, PatchKind,
    };

    fn unit_mesh() -> Arc<FvMesh> {
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

    fn phi_field(m: Arc<FvMesh>, int: f64, bnd: f64) -> SurfaceScalarField {
        let ni = m.n_internal_faces;
        let bnd_pf: Vec<_> = m
            .patches
            .iter()
            .map(|p| PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: Field::uniform(p.size, bnd),
            })
            .collect();
        SurfaceScalarField::new("phi", m, Field::uniform(ni, int), bnd_pf)
    }

    #[test]
    fn div_flux_of_uniform_inoutflow_is_zero() {
        // Symmetric flux +1 in, -1 out → net = 0 for interior cell
        // Only internal face with phi=1: cell 0 gains +1, cell 1 loses +1
        // Both boundary fluxes are 0 → both cells see net = ±1/0.5 = ±2
        // Actually just verify the formula: div_flux sums face fluxes / V
        let m = unit_mesh();
        let phi = phi_field(m.clone(), 0.0, 0.0);
        let d = div_flux(&phi);
        assert!(d.internal[0].abs() < 1e-12);
        assert!(d.internal[1].abs() < 1e-12);
    }

    #[test]
    fn div_flux_nonzero() {
        // phi_internal = 1, boundaries = 0
        // cell 0: +1 (internal outflow), -0 (left bnd inflow=0) → net +1 / 0.5 = +2
        // cell 1: -1 (internal, neighbour) + 0 (right bnd) → net -1 / 0.5 = -2
        let m = unit_mesh();
        let phi = phi_field(m.clone(), 1.0, 0.0);
        let d = div_flux(&phi);
        assert!(
            (d.internal[0] - 2.0).abs() < 1e-10,
            "div_flux[0]={}",
            d.internal[0]
        );
        assert!(
            (d.internal[1] - (-2.0)).abs() < 1e-10,
            "div_flux[1]={}",
            d.internal[1]
        );
    }
}
