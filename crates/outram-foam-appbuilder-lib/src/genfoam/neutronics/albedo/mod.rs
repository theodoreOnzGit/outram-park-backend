// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/albedoSP3/albedoSP3FvPatchField.{C,H}
//                    (the `gamma` definition and its per-group `Dalbedo` field,
//                    set to `D[energyI]` in diffusion/include/fluxEq.H)
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal author: Carlo Fiorina (EPFL)
//   Upstream license: GPL-3.0
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
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! # Albedo (partial-current) boundary condition for neutron diffusion
//!
//! GeN-Foam's `albedoSP3` patch condition, in the form the **diffusion** solver
//! uses it. Reactor cases lean on it heavily: a bare `fixedValue 0` treats the
//! domain edge as a perfect absorber, which is wrong wherever a reflector,
//! a vessel wall or a mirror-symmetry plane sits just outside the mesh.
//!
//! ## The condition
//!
//! A partially reflecting surface returns a fraction `alpha` of the neutrons
//! reaching it. In diffusion theory that is the Robin condition
//!
//! ```text
//!   -D dphi/dn = gamma phi          (n the OUTWARD normal)
//! ```
//!
//! with the albedo coefficient
//!
//! ```text
//!   gamma = (1 - alpha) / (1 + alpha) / 2
//! ```
//!
//! — GeN-Foam's own definition, stated in its tutorial dictionaries. Two limits
//! fix the convention and are worth holding on to:
//!
//! | surface | `alpha` | `gamma` | reduces to |
//! |---|---|---|---|
//! | perfect reflector | 1 | 0 | zero gradient (no leakage) |
//! | vacuum (Marshak) | 0 | 1/2 | `-D dphi/dn = phi/2` |
//!
//! `gamma = 1/2` being the vacuum value is why upstream's patch class defaults
//! `gamma` to `0.5`.
//!
//! ## Discretisation
//!
//! With `delta` the owner-cell-centre-to-face distance and `phi_c` the owner
//! cell value, the one-sided gradient `(phi_f - phi_c)/delta` gives
//!
//! ```text
//!   -D (phi_f - phi_c)/delta = gamma phi_f
//!   =>  phi_f = phi_c / (1 + gamma delta / D)
//! ```
//!
//! which is exactly a `mixed` (Robin) patch with zero references and weight
//!
//! ```text
//!   w = (gamma delta / D) / (1 + gamma delta / D),   refValue = 0, refGrad = 0
//! ```
//!
//! since `mixed` evaluates `phi_f = w refValue + (1-w)(phi_c + refGrad delta)`
//! `= (1-w) phi_c`. Check the limits again on this form: `gamma = 0` gives
//! `w = 0`, zero gradient; `gamma -> infinity` gives `w -> 1`, `phi_f = 0`.
//!
//! ## Why the weight has to vary along the patch
//!
//! `w` carries **both** the local mesh spacing `delta` and the local diffusion
//! coefficient `D`, and neither is constant: `delta` varies on a graded mesh and
//! `D` varies by material zone and, strongly, by energy group. Upstream handles
//! the group dependence by resetting its `Dalbedo` field to `D[energyI]` before
//! each group's solve (`diffusion/include/fluxEq.H`), and the spatial dependence
//! by looking `Dalbedo` up per face. This port matches that: the weight is built
//! per face, per group, as a
//! [`BoundaryCondition::MixedField`](outram_foam_basic_lib::fields::boundary::bc::BoundaryCondition::MixedField).
//!
//! For the MSFR tutorial `D` spans 1.10e-2 to 2.37e-2 m across its six groups —
//! better than a factor of two — so collapsing the weight to one number per
//! patch is not a rounding detail.
//!
//! ## What is deliberately not ported
//!
//! Upstream's patch class also carries a `fluxStarAlbedo` term and a
//! `forSecondMoment` branch, both of which belong to **SP3**: they couple the
//! first and second flux moments. In the diffusion solver upstream zeroes that
//! field (`fluxStarAlbedo_ *= 0.0`, `diffusion/include/fluxEq.H`), so it drops
//! out entirely and the condition reduces to the Robin form above. Porting the
//! SP3 branch belongs with the SP3 solver.

use outram_foam_basic_lib::fields::boundary::bc::{BoundaryCondition, PatchField};
use outram_foam_basic_lib::fields::field::Field;
use outram_foam_basic_lib::fields::VolScalarField;
use outram_foam_basic_lib::mesh::FvMesh;

/// Which linearisation of the albedo condition to assemble.
///
/// The condition `-D dphi/dn = gamma phi` is exact; turning it into a matrix
/// coefficient requires deciding **which** `phi` the right-hand side means, and
/// GeN-Foam and the textbook one-sided derivation answer differently. The
/// difference is first order in the cell size, vanishes under refinement, and is
/// worth several thousand pcm on a real reactor mesh — so it is a choice, made
/// explicitly, rather than a detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlbedoLinearisation {
    /// **GeN-Foam's**: the leakage current is evaluated at the *owner cell*
    /// value, `J_out = gamma phi_cell`.
    ///
    /// This is what upstream's `albedoSP3FvPatchField` assembles. Its
    /// `snGrad()` returns `-phi_c gamma / D` and its `gradientInternalCoeffs()`
    /// returns `-gamma/D`, both in terms of `patchInternalField()` — the cell
    /// value — with no face-value closure. Multiplied by the laplacian's own
    /// `D |Sf|`, that contributes exactly `gamma |Sf|` to the diagonal,
    /// independent of `D` and of the cell size.
    ///
    /// In this port's `mixed` form that is `w = gamma delta / D`, with zero
    /// references. Note `w` may exceed 1 on a coarse mesh, which a `mixed`
    /// weight nominally may not; that is a faithful consequence of upstream's
    /// linearisation, not a bug here, and it is why the weight is not clamped.
    ///
    /// **Use this to compare against GeN-Foam.** It is the default for that
    /// reason.
    #[default]
    CellValue,
    /// The exact one-sided closure: the leakage current is evaluated at the
    /// *face* value, `J_out = gamma phi_face`.
    ///
    /// Eliminating `phi_face` between `-D (phi_f - phi_c)/delta = gamma phi_f`
    /// gives `phi_f = phi_c/(1 + gamma delta/D)`, i.e. `w = r/(1+r)` with
    /// `r = gamma delta/D`. This is the form a textbook derivation reaches and
    /// the one that stays bounded (`w < 1` always, so it never over-drains a
    /// cell).
    ///
    /// It is strictly **less** leaky than [`CellValue`](Self::CellValue), since
    /// `r/(1+r) < r`. On the MSFR tutorial the two differ by about 2100 pcm.
    FaceValue,
}

impl AlbedoLinearisation {
    /// The `mixed` weight for a face with `r = gamma delta / D`.
    #[must_use]
    fn weight(self, r: f64) -> f64 {
        match self {
            Self::CellValue => r,
            Self::FaceValue => r / (1.0 + r),
        }
    }
}

/// The albedo coefficient `gamma` for a surface returning a fraction `alpha` of
/// incident neutrons: `gamma = (1 - alpha)/(1 + alpha)/2`.
///
/// `alpha` is dimensionless in `[0, 1]`; `alpha = 0` is a vacuum
/// (`gamma = 1/2`) and `alpha = 1` a perfect reflector (`gamma = 0`).
///
/// ```
/// use outram_foam_appbuilder_lib::genfoam::neutronics::albedo::gamma_from_albedo;
///
/// assert!((gamma_from_albedo(0.0) - 0.5).abs() < 1e-15); // vacuum
/// assert!(gamma_from_albedo(1.0).abs() < 1e-15);         // perfect reflector
/// ```
///
/// # Panics
///
/// Panics if `alpha` is outside `[0, 1]`, which is not a physical albedo. A
/// caller that has a `gamma` already — as GeN-Foam case dictionaries do, since
/// they state `gamma` directly — should pass it to
/// [`albedo_patch_field`] without going through this function.
#[must_use]
pub fn gamma_from_albedo(alpha: f64) -> f64 {
    assert!(
        (0.0..=1.0).contains(&alpha),
        "albedo must lie in [0, 1], got {alpha}"
    );
    (1.0 - alpha) / (1.0 + alpha) / 2.0
}

/// Build the albedo (Robin) patch condition for one patch and one energy group.
///
/// # Parameters
///
/// - `mesh` — the neutronics mesh.
/// - `patch_index` — index into `mesh.patches`.
/// - `gamma` — the albedo coefficient, dimensionless and non-negative. This is
///   the quantity GeN-Foam case dictionaries state directly; convert from a
///   reflection fraction with [`gamma_from_albedo`].
/// - `diffusion_coefficient` — `D` **for this energy group** (m), the field
///   upstream copies into `Dalbedo` before each group's solve.
///
/// # Returns
///
/// A [`PatchField`] carrying a per-face
/// [`MixedField`](outram_foam_basic_lib::fields::boundary::bc::BoundaryCondition::MixedField)
/// with `refValue = 0`, `refGrad = 0` and
/// `w = (gamma delta/D)/(1 + gamma delta/D)`. The stored face `values` are left
/// at zero; they are recomputed from the weight whenever the flux is
/// interpolated.
///
/// # Panics
///
/// Panics if `patch_index` is out of range, if `gamma` is negative or
/// non-finite, or if `diffusion_coefficient` is not defined on `mesh`.
///
/// A non-positive `D` on a boundary-adjacent cell is treated as **vacuum**
/// (`w = 1`) rather than producing an infinity: `D <= 0` is unphysical, and the
/// most absorbing interpretation is the one that cannot silently inflate
/// `k_eff`.
#[must_use]
pub fn albedo_patch_field(
    mesh: &FvMesh,
    patch_index: usize,
    gamma: f64,
    diffusion_coefficient: &VolScalarField,
    linearisation: AlbedoLinearisation,
) -> PatchField<f64> {
    assert!(
        gamma.is_finite() && gamma >= 0.0,
        "albedo gamma must be finite and non-negative, got {gamma}"
    );
    let patch = &mesh.patches[patch_index];
    assert_eq!(
        diffusion_coefficient.internal.len(),
        mesh.n_cells,
        "the diffusion coefficient field is not defined on this mesh"
    );

    let weights: Vec<f64> = (0..patch.size)
        .map(|fi| {
            let gf = patch.start + fi;
            let owner = mesh.owner[gf];
            let delta = (mesh.face_centres[gf] - mesh.cell_centres[owner]).mag();
            let d = diffusion_coefficient.internal[owner];
            if !(d > 0.0) {
                return 1.0;
            }
            linearisation.weight(gamma * delta / d)
        })
        .collect();

    PatchField {
        bc: BoundaryCondition::MixedField {
            value_fraction: Field::new(weights),
            ref_value: Field::new(vec![0.0; patch.size]),
            ref_grad: Field::new(vec![0.0; patch.size]),
        },
        values: Field::new(vec![0.0; patch.size]),
    }
}

#[cfg(test)]
mod tests;
