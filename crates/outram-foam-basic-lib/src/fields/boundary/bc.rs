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

use crate::fields::field::Field;
use crate::primitives::{SymmTensor, Tensor, Vector3};

/// Boundary condition variant for a single patch.
///
/// Covers the patch-field boundary conditions ported from OpenFOAM's
/// `finiteVolume/fields/fvPatchFields`.  The set is closed and dispatched by
/// enum (no `dyn`), so adding a variant forces every exhaustive `match` site to
/// be updated — the compiler flags each one.
///
/// # Units
///
/// The variants are unit-agnostic in `T`: `T` is whatever the field stores
/// (`f64` for a scalar field, [`Vector3`] for a vector field, …).  Where a
/// variant stores a *gradient* (`FixedGradient`, `Mixed::ref_grad`) the value
/// is a normal gradient in units of *field-value per metre* (`[T]·m⁻¹`), because
/// the boundary face value is reconstructed as `cell_value + gradient · delta`
/// with `delta` the owner-cell-centre-to-face distance in metres.
///
/// # Status
///
/// The `FixedGradient`, `Mixed`, `InletOutlet`, `OutletInlet`, `Slip`,
/// `NoSlip`, and `Wedge` variants (added 2026-08-04) are an **untrusted
/// AI-assisted draft pending human V&V review** — verified against
/// analytic/limiting cases (see the `vv_*` tests) but not yet human-reviewed.
/// `Wedge` in particular is a zero-gradient stand-in, not the full rotation.
#[derive(Debug, Clone)]
pub enum BoundaryCondition<T: Clone> {
    /// Dirichlet: fixed uniform value.
    ///
    /// OpenFOAM: `fixedValueFvPatchField`.
    FixedValue(T),
    /// Dirichlet: fixed per-face values.
    ///
    /// OpenFOAM: `fixedValueFvPatchField` (non-uniform list form).
    FixedField(Field<T>),
    /// Neumann: zero normal gradient — boundary face value = internal adjacent value.
    ///
    /// OpenFOAM: `zeroGradientFvPatchField`.
    ZeroGradient,
    /// Neumann with a prescribed **non-zero** normal gradient `g` (`[T]·m⁻¹`).
    ///
    /// The boundary face value is `φ_face = φ_cell + g · delta`, where `delta`
    /// [m] is the owner-cell-centre-to-face-centre distance.  Reduces to
    /// [`ZeroGradient`](Self::ZeroGradient) when `g = 0`.
    ///
    /// OpenFOAM: `fixedGradientFvPatchField`
    /// (`src/finiteVolume/fields/fvPatchFields/derived/fixedGradient/fixedGradientFvPatchField.H`).
    FixedGradient(T),
    /// Robin / mixed boundary condition — a per-face blend of a Dirichlet part
    /// (`fixedValue`, weight `value_fraction`) and a Neumann part
    /// (`fixedGradient`, weight `1 - value_fraction`).
    ///
    /// With `w = value_fraction ∈ [0, 1]`, `delta` [m] the cell-to-face
    /// distance, `φ_c` the owner cell value:
    ///
    /// - face value: `φ_face = w·ref_value + (1 - w)·(φ_c + ref_grad·delta)`
    /// - it reduces to [`FixedValue`](Self::FixedValue)`(ref_value)` at `w = 1`
    ///   and to [`FixedGradient`](Self::FixedGradient)`(ref_grad)` at `w = 0`.
    ///
    /// This is the general form underlying every value/gradient-blending BC,
    /// including the albedo / Robin condition used in neutron diffusion.
    ///
    /// - `value_fraction` — dimensionless weight in `[0, 1]`.
    /// - `ref_value` — the Dirichlet reference value (`[T]`).
    /// - `ref_grad` — the Neumann reference normal gradient (`[T]·m⁻¹`).
    ///
    /// OpenFOAM: `mixedFvPatchField`
    /// (`src/finiteVolume/fields/fvPatchFields/basic/mixed/mixedFvPatchField.H`).
    Mixed {
        /// Dirichlet/Neumann blend weight, dimensionless, `∈ [0, 1]`.
        value_fraction: f64,
        /// Dirichlet reference value (`[T]`).
        ref_value: T,
        /// Neumann reference normal gradient (`[T]·m⁻¹`).
        ref_grad: T,
    },
    /// Flux-switched inflow/outflow BC: behaves as
    /// [`FixedValue`](Self::FixedValue)`(inlet_value)` on **inflow** faces and
    /// [`ZeroGradient`](Self::ZeroGradient) on **outflow** faces.
    ///
    /// The switch is decided per face by the sign of the outward face flux
    /// `φ_f = U·S_f` [m³·s⁻¹]: `φ_f < 0` is inflow (fixed value), `φ_f ≥ 0` is
    /// outflow (zero gradient).  Equivalent to a [`Mixed`](Self::Mixed) BC whose
    /// `value_fraction` is set to `1` on inflow and `0` on outflow.  The flux is
    /// supplied by the convection operator at assembly time, so this variant is
    /// only flux-switched inside operators that carry `phi`.
    ///
    /// OpenFOAM: `inletOutletFvPatchField`
    /// (`src/finiteVolume/fields/fvPatchFields/derived/inletOutlet/inletOutletFvPatchField.H`).
    InletOutlet {
        /// Value imposed on inflow faces (`[T]`).
        inlet_value: T,
    },
    /// Flux-switched outflow/inflow BC — the opposite of
    /// [`InletOutlet`](Self::InletOutlet): [`FixedValue`](Self::FixedValue)`(outlet_value)`
    /// on **outflow** faces (`φ_f ≥ 0`) and [`ZeroGradient`](Self::ZeroGradient)
    /// on **inflow** faces (`φ_f < 0`).
    ///
    /// OpenFOAM: `outletInletFvPatchField`
    /// (`src/finiteVolume/fields/fvPatchFields/derived/outletInlet/outletInletFvPatchField.H`).
    OutletInlet {
        /// Value imposed on outflow faces (`[T]`).
        outlet_value: T,
    },
    /// Symmetry plane — normal component zeroed.
    ///
    /// OpenFOAM: `symmetryFvPatchField`.
    Symmetry,
    /// Free-slip wall: the wall-normal component of a vector field is removed
    /// (as for [`Symmetry`](Self::Symmetry)) while the tangential component is
    /// zero-gradient.  For a scalar field it is exactly zero-gradient.
    ///
    /// See [`BoundaryCondition::<Vector3>::slip_face_value`] for the exact
    /// vector reconstruction.
    ///
    /// OpenFOAM: `slipFvPatchField`
    /// (`src/finiteVolume/fields/fvPatchFields/derived/slip/slipFvPatchField.H`).
    Slip,
    /// No-slip wall for velocity: a `fixedValue` of zero.  Semantically it is
    /// [`FixedValue`](Self::FixedValue)`(T::zero)` specialised to walls; the
    /// stored patch values are all zero.
    ///
    /// OpenFOAM: `noSlipFvPatchField`
    /// (`src/finiteVolume/fields/fvPatchFields/derived/noSlip/noSlipFvPatchField.H`).
    NoSlip,
    /// Axisymmetric wedge patch (`wedgeFvPatchField`).
    ///
    /// **First-pass simplification (Layer-1):** treated as zero-gradient — the
    /// patch face value equals the adjacent internal cell value.  A full wedge
    /// BC rotates the patch-internal field onto the wedge face about the
    /// geometric axis (pairing across the wedge like a cyclic); that rotation is
    /// **not yet implemented** here.  The zero-gradient stand-in is exact only
    /// for a field that is uniform in the wedge (azimuthal) direction.  Do not
    /// treat wedge results as validated until the rotation transform lands.
    ///
    /// OpenFOAM: `wedgeFvPatchField`
    /// (`src/finiteVolume/fields/fvPatchFields/constraint/wedge/wedgeFvPatchField.H`).
    Wedge,
    /// 2-D / empty — zero-area faces; value has no physical meaning.
    ///
    /// OpenFOAM: `emptyFvPatchField`.
    Empty,
    /// Value computed by the solver and stored here (read-only from BC side).
    ///
    /// OpenFOAM: `calculatedFvPatchField`.
    Calculated(Field<T>),
}

impl<T: Clone + Default> BoundaryCondition<T> {
    /// True if the BC imposes a value (Dirichlet-like) unconditionally.
    ///
    /// Flux-switched BCs ([`InletOutlet`](Self::InletOutlet) /
    /// [`OutletInlet`](Self::OutletInlet)) are **not** counted here because
    /// whether they impose a value depends on the per-face flux; use
    /// [`flux_value_fraction`](Self::flux_value_fraction) for those.
    pub fn is_fixed_value(&self) -> bool {
        matches!(self, Self::FixedValue(_) | Self::FixedField(_) | Self::NoSlip)
    }
}

impl<T: Clone> BoundaryCondition<T> {
    /// Value fraction (`1` ⇒ acts as `fixedValue`, `0` ⇒ acts as
    /// `zeroGradient`) for a **flux-switched** BC, given the outward face flux
    /// `phi_f = U·S_f` [m³·s⁻¹].
    ///
    /// Returns `None` for BCs that are not flux-switched.  The sign convention
    /// matches OpenFOAM's `inletOutlet`/`outletInlet`: `phi_f < 0` is inflow,
    /// `phi_f ≥ 0` is outflow.
    ///
    /// ```
    /// use outram_foam_basic_lib::fields::boundary::bc::BoundaryCondition;
    /// let io = BoundaryCondition::InletOutlet { inlet_value: 5.0_f64 };
    /// assert_eq!(io.flux_value_fraction(-1.0), Some(1.0)); // inflow  -> fixedValue
    /// assert_eq!(io.flux_value_fraction( 1.0), Some(0.0)); // outflow -> zeroGradient
    /// assert_eq!(BoundaryCondition::ZeroGradient::<f64>.flux_value_fraction(1.0), None);
    /// ```
    pub fn flux_value_fraction(&self, phi_f: f64) -> Option<f64> {
        match self {
            Self::InletOutlet { .. } => Some(if phi_f < 0.0 { 1.0 } else { 0.0 }),
            Self::OutletInlet { .. } => Some(if phi_f >= 0.0 { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    /// The reference (Dirichlet) value of a flux-switched BC, if it has one.
    ///
    /// Returns the inlet value for [`InletOutlet`](Self::InletOutlet), the
    /// outlet value for [`OutletInlet`](Self::OutletInlet), and `None`
    /// otherwise.
    pub fn flux_ref_value(&self) -> Option<&T> {
        match self {
            Self::InletOutlet { inlet_value } => Some(inlet_value),
            Self::OutletInlet { outlet_value } => Some(outlet_value),
            _ => None,
        }
    }
}

impl BoundaryCondition<Vector3> {
    /// Free-slip face value for a vector field: the wall-normal component is
    /// removed and the tangential component is kept (a zero-gradient
    /// projection onto the wall plane).
    ///
    /// `φ_face = φ_internal − n̂ (φ_internal · n̂)`, where `n̂` is the **unit**
    /// outward face normal (dimensionless) and `φ_internal` is the adjacent
    /// cell value.  After this projection `φ_face · n̂ = 0` to rounding.
    ///
    /// ```
    /// use outram_foam_basic_lib::fields::boundary::bc::BoundaryCondition;
    /// use outram_foam_basic_lib::primitives::Vector3;
    /// let internal = Vector3::new(3.0, 4.0, 0.0);
    /// let n = Vector3::new(1.0, 0.0, 0.0); // wall normal along +x
    /// let v = BoundaryCondition::slip_face_value(internal, n);
    /// assert!(v.x.abs() < 1e-15);          // normal component removed
    /// assert!((v.y - 4.0).abs() < 1e-15);  // tangential component preserved
    /// ```
    pub fn slip_face_value(internal: Vector3, unit_normal: Vector3) -> Vector3 {
        internal - unit_normal * internal.dot(unit_normal)
    }
}

/// Boundary field for one patch: the BC type plus the current face values.
///
/// The `values` field always holds the latest face values (updated by
/// `update_coeffs` in Layer 3 operators).  For `FixedValue`/`FixedField` the
/// values are set at construction and never change.  For `ZeroGradient` and
/// `Calculated` they are written by the operator code.
#[derive(Debug, Clone)]
pub struct PatchField<T: Clone> {
    /// The boundary condition applied on this patch.
    pub bc: BoundaryCondition<T>,
    /// Current face values for this patch (length == patch.size).
    pub values: Field<T>,
}

impl PatchField<f64> {
    /// Dirichlet patch holding a uniform scalar `v` on all `size` faces.
    pub fn fixed_value(size: usize, v: f64) -> Self {
        Self {
            bc: BoundaryCondition::FixedValue(v),
            values: Field::uniform(size, v),
        }
    }

    /// Zero-gradient (Neumann) scalar patch of `size` faces; values default to
    /// `0.0` and are overwritten by the operator that owns them.
    pub fn zero_gradient(size: usize) -> Self {
        Self {
            bc: BoundaryCondition::ZeroGradient,
            values: Field::zeros(size),
        }
    }

    /// Empty (zero-area) scalar patch — no faces, no physical value.
    pub fn empty() -> Self {
        Self {
            bc: BoundaryCondition::Empty,
            values: Field::new(vec![]),
        }
    }
}

impl PatchField<Vector3> {
    /// Dirichlet patch holding a uniform `Vector3` value `v` on all `size` faces.
    pub fn fixed_value_vec(size: usize, v: Vector3) -> Self {
        Self {
            bc: BoundaryCondition::FixedValue(v),
            values: Field::uniform(size, v),
        }
    }

    /// Zero-gradient (Neumann) vector patch of `size` faces; values default to
    /// `Vector3::ZERO` and are overwritten by the operator that owns them.
    pub fn zero_gradient_vec(size: usize) -> Self {
        Self {
            bc: BoundaryCondition::ZeroGradient,
            values: Field::zero_vec(size),
        }
    }

    /// Empty (zero-area) vector patch — no faces, no physical value.
    pub fn empty_vec() -> Self {
        Self {
            bc: BoundaryCondition::Empty,
            values: Field::new(vec![]),
        }
    }
}

impl PatchField<Tensor> {
    /// Dirichlet patch holding a uniform `Tensor` value.
    pub fn fixed_value_tensor(size: usize, v: Tensor) -> Self {
        Self {
            bc: BoundaryCondition::FixedValue(v),
            values: Field::uniform(size, v),
        }
    }

    /// Zero-gradient (Neumann) patch for a `Tensor` field; values default to
    /// `Tensor::ZERO` and are overwritten by the operator that owns them.
    pub fn zero_gradient_tensor(size: usize) -> Self {
        Self {
            bc: BoundaryCondition::ZeroGradient,
            values: Field::uniform(size, Tensor::ZERO),
        }
    }
}

impl PatchField<SymmTensor> {
    /// Dirichlet patch holding a uniform `SymmTensor` value.
    pub fn fixed_value_symm_tensor(size: usize, v: SymmTensor) -> Self {
        Self {
            bc: BoundaryCondition::FixedValue(v),
            values: Field::uniform(size, v),
        }
    }

    /// Zero-gradient (Neumann) patch for a `SymmTensor` field; values default to
    /// `SymmTensor::ZERO` and are overwritten by the operator that owns them.
    pub fn zero_gradient_symm_tensor(size: usize) -> Self {
        Self {
            bc: BoundaryCondition::ZeroGradient,
            values: Field::uniform(size, SymmTensor::ZERO),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// V&V (verification, 2026-08-04). Free-slip semantics: for a vector field
    /// the wall-normal component is removed and the tangential component is
    /// preserved. Methodology: project (3,4,0) at a wall whose unit normal is
    /// +x. Analytic result: (0,4,0). Pass criterion: normal component < 1e-15,
    /// tangential preserved to < 1e-15.
    /// Result: v = (0, 4, 0); v·n̂ = 0 exactly. PASS.
    #[test]
    fn vv_slip_zeroes_normal_preserves_tangential() {
        let internal = Vector3::new(3.0, 4.0, 0.0);
        let n = Vector3::new(1.0, 0.0, 0.0);
        let v = BoundaryCondition::slip_face_value(internal, n);
        assert!(v.dot(n).abs() < 1e-15, "normal component not removed: {}", v.dot(n));
        assert!((v.y - 4.0).abs() < 1e-15);
        assert!(v.z.abs() < 1e-15);
        // Oblique normal check: (1,1,0)/√2 removes the (1,1,0) part of (2,0,0).
        let n2 = Vector3::new(1.0, 1.0, 0.0) * (1.0 / 2.0_f64.sqrt());
        let v2 = BoundaryCondition::slip_face_value(Vector3::new(2.0, 0.0, 0.0), n2);
        assert!(v2.dot(n2).abs() < 1e-15, "oblique normal not removed: {}", v2.dot(n2));
    }

    /// V&V (verification, 2026-08-04). Flux switch of inletOutlet/outletInlet.
    /// Methodology: check `flux_value_fraction` returns 1 (fixedValue) / 0
    /// (zeroGradient) with the correct flux sign, and `None` for non-switched
    /// BCs. Pass criterion: exact enum/float equality.
    /// Result: inletOutlet → (inflow 1, outflow 0); outletInlet → (inflow 0,
    /// outflow 1); zeroGradient → None. PASS.
    #[test]
    fn vv_flux_value_fraction_switch() {
        let io = BoundaryCondition::InletOutlet { inlet_value: 5.0_f64 };
        assert_eq!(io.flux_value_fraction(-1.0), Some(1.0)); // inflow
        assert_eq!(io.flux_value_fraction(1.0), Some(0.0)); // outflow
        assert_eq!(io.flux_ref_value(), Some(&5.0));
        let oi = BoundaryCondition::OutletInlet { outlet_value: 7.0_f64 };
        assert_eq!(oi.flux_value_fraction(-1.0), Some(0.0)); // inflow
        assert_eq!(oi.flux_value_fraction(1.0), Some(1.0)); // outflow
        assert_eq!(oi.flux_ref_value(), Some(&7.0));
        assert_eq!(BoundaryCondition::<f64>::ZeroGradient.flux_value_fraction(1.0), None);
    }
}
