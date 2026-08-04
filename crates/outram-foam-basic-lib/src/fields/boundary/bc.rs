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
///
/// The flow-context variants `Freestream`, `PressureInletOutletVelocity`,
/// `FixedFluxPressure`, `TotalPressure`, and `FlowRateInletVelocity` (added
/// 2026-08-04, Wave 4) are likewise an **untrusted AI-assisted draft pending
/// human V&V review** — verified against analytic/definition cases (the `vv_*`
/// tests) but not yet human-reviewed. `Freestream` is self-contained
/// (flux-switched by the convection operator like `InletOutlet`); the other
/// four are **solver-driven** — they depend on context the per-face BC update
/// cannot supply on its own (`PressureInletOutletVelocity` and
/// `FixedFluxPressure` on the face flux / momentum-predictor flux,
/// `TotalPressure` on the patch velocity and density, `FlowRateInletVelocity`
/// on the patch-area integral), so the solver must refresh their stored face
/// values / gradient each iteration through the documented `update_*` /
/// `*_value` hooks below rather than the BC hard-coding a wrong value.
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
    /// Freestream (far-field) inflow/outflow BC — an [`InletOutlet`](Self::InletOutlet)
    /// specialised to external / far-field flow: it imposes the uniform
    /// freestream value on **inflow** faces and is [`ZeroGradient`](Self::ZeroGradient)
    /// on **outflow** faces, switched per face by the sign of the outward face
    /// flux `φ_f = U·S_f` [m³·s⁻¹] (`φ_f < 0` inflow, `φ_f ≥ 0` outflow).
    ///
    /// For a velocity field `freestream_value` is the far-field velocity `U_∞`
    /// [m·s⁻¹]; for a scalar it is the far-field scalar value (`[T]`). It is
    /// **self-contained**: the flux is supplied by the convection operator at
    /// assembly time, exactly like [`InletOutlet`](Self::InletOutlet), so no
    /// solver hook is needed. See [`flux_value_fraction`](Self::flux_value_fraction)
    /// / [`flux_ref_value`](Self::flux_ref_value).
    ///
    /// OpenFOAM: `freestreamFvPatchField`
    /// (`src/finiteVolume/fields/fvPatchFields/derived/freestream/freestreamFvPatchField.H`),
    /// which derives from `inletOutletFvPatchField` with `inletValue = freestreamValue`.
    Freestream {
        /// Far-field freestream value imposed on inflow faces (`[T]`; for a
        /// velocity field, m·s⁻¹).
        freestream_value: T,
    },
    /// Velocity BC for a pressure-driven inlet/outlet patch: the patch velocity
    /// is reconstructed from the face flux `φ_f` [m³·s⁻¹] and the face area.
    ///
    /// On **outflow** (`φ_f ≥ 0`) it is [`ZeroGradient`](Self::ZeroGradient); on
    /// **inflow** (`φ_f < 0`) it imposes the flux-implied wall-normal velocity
    /// `U = (φ_f / |S_f|)·n̂` [m·s⁻¹], where `n̂ = S_f/|S_f|` is the unit outward
    /// face normal. (OpenFOAM sets `valueFraction = 1 − pos0(φ_f)`, i.e.
    /// `fixedValue` on inflow and `zeroGradient` on outflow; the imposed value is
    /// the normal velocity above, the tangential component taken as zero here —
    /// OpenFOAM's optional `tangentialVelocity` is not modelled.)
    ///
    /// The per-face imposed velocity depends on the flux and face geometry, which
    /// a value-only variant cannot carry, so this variant is **solver-driven**:
    /// the solver refreshes [`PatchField::values`] each iteration via
    /// [`PatchField::update_pressure_inlet_outlet_velocity`], and the convection
    /// operator additionally flux-switches it per face
    /// ([`flux_value_fraction`](Self::flux_value_fraction) returns the inflow/
    /// outflow weight). The pure per-face formula is
    /// [`pressure_inlet_outlet_velocity_value`](BoundaryCondition::<Vector3>::pressure_inlet_outlet_velocity_value).
    ///
    /// OpenFOAM: `pressureInletOutletVelocityFvPatchVectorField`.
    PressureInletOutletVelocity,
    /// Pressure BC that fixes the surface-normal pressure gradient `snGrad(p)`
    /// [Pa·m⁻¹] so the pressure-corrected face flux matches a target flux — the
    /// natural wall / outlet pressure condition in a PISO/PIMPLE pressure solve.
    ///
    /// It behaves as a [`FixedGradient`](Self::FixedGradient)`(gradient)` whose
    /// gradient the solver sets each pressure solve from the flux mismatch:
    ///
    /// `snGrad(p) = (φ_HbyA − φ_target) / (D_p · |S_f|)`
    ///
    /// where `φ_HbyA` [m³·s⁻¹] is the momentum-predictor (H/A) face flux,
    /// `φ_target` [m³·s⁻¹] the desired boundary flux, `D_p` [m³·s·kg⁻¹] the
    /// face-interpolated `rAU` (interpolated `1/A_p` from the momentum-matrix
    /// diagonal, which absorbs any body-force term folded into `H/A`), and `|S_f|`
    /// [m²] the face area. The gradient is **solver-set** because it needs the
    /// predictor flux and the `rAU` field, which this Layer does not own. See
    /// [`fixed_flux_pressure_sn_grad`](BoundaryCondition::<f64>::fixed_flux_pressure_sn_grad)
    /// for the pure formula. The stored `gradient` is uniform over the patch
    /// (matching [`FixedGradient`](Self::FixedGradient)); the diffusion and
    /// `snGrad` operators consume it as a prescribed normal gradient.
    ///
    /// OpenFOAM: `fixedFluxPressureFvPatchScalarField`.
    FixedFluxPressure {
        /// Currently-set surface-normal pressure gradient `snGrad(p)` [Pa·m⁻¹],
        /// uniform over the patch.
        gradient: T,
    },
    /// Total-pressure (stagnation-pressure) inlet/outlet BC: the static boundary
    /// pressure is set from a fixed total pressure `p0` and the local dynamic
    /// head. Incompressible form:
    ///
    /// `p = p0 − 0.5·ρ·|U|²`
    ///
    /// with `p`, `p0` in Pa, `ρ` in kg·m⁻³, `|U|` in m·s⁻¹. At rest (`|U| = 0`)
    /// it reduces to [`FixedValue`](Self::FixedValue)`(p0)`. The compressible
    /// (subsonic) form `p = p0 (1 + ((γ−1)/2)·M²)^(−γ/(γ−1))` is **deferred**.
    ///
    /// This needs the velocity magnitude and density **at the patch** — a
    /// cross-field dependency a per-face BC update cannot supply on its own — so
    /// this variant is **solver-driven**: the solver refreshes
    /// [`PatchField::values`] every iteration via
    /// [`PatchField::update_total_pressure`] (which reads `p0` from this variant
    /// and applies
    /// [`total_pressure_value`](BoundaryCondition::<f64>::total_pressure_value)).
    /// At assembly time it acts as a [`FixedValue`](Self::FixedValue) holding the
    /// last-computed face pressure.
    ///
    /// OpenFOAM: `totalPressureFvPatchScalarField`.
    TotalPressure {
        /// Fixed total (stagnation) pressure `p0` [Pa].
        p0: T,
    },
    /// Uniform inlet-velocity BC scaled to a prescribed volumetric flow rate:
    /// the whole patch is given a uniform velocity directed **into** the domain
    /// whose magnitude makes the patch-integral volumetric flux equal `Q`:
    ///
    /// `U = −(Q / A_patch)·n̂`,  `A_patch = Σ_f |S_f|`
    ///
    /// with `U` in m·s⁻¹, `Q` in m³·s⁻¹, `A_patch` in m², `n̂ = S_f/|S_f|` the
    /// unit outward face normal (so `−n̂` points into the domain). The patch-area
    /// integral comes from the mesh, so this variant is **geometry/solver-driven**:
    /// the per-face values are filled from the patch face-area vectors by
    /// [`PatchField::update_flow_rate_inlet_velocity`] (the fixed quantity is the
    /// rate `Q`; the resulting velocity depends on the patch area). At assembly it
    /// acts as a [`FixedValue`](Self::FixedValue) inlet. Per-face formula:
    /// [`flow_rate_inlet_velocity_value`](BoundaryCondition::<Vector3>::flow_rate_inlet_velocity_value).
    ///
    /// Only the volumetric form is modelled; the mass-flow form
    /// `U = −(ṁ / (ρ·A))·n̂` is obtained by passing `Q = ṁ/ρ`.
    ///
    /// OpenFOAM: `flowRateInletVelocityFvPatchVectorField`.
    FlowRateInletVelocity {
        /// Prescribed volumetric flow rate `Q` [m³·s⁻¹] (positive = into the
        /// domain).
        volumetric_flow_rate: f64,
    },
}

impl<T: Clone + Default> BoundaryCondition<T> {
    /// True if the BC imposes a value (Dirichlet-like) unconditionally.
    ///
    /// Flux-switched BCs ([`InletOutlet`](Self::InletOutlet) /
    /// [`OutletInlet`](Self::OutletInlet)) are **not** counted here because
    /// whether they impose a value depends on the per-face flux; use
    /// [`flux_value_fraction`](Self::flux_value_fraction) for those.
    pub fn is_fixed_value(&self) -> bool {
        matches!(
            self,
            Self::FixedValue(_)
                | Self::FixedField(_)
                | Self::NoSlip
                | Self::TotalPressure { .. }
                | Self::FlowRateInletVelocity { .. }
        )
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
            // Inflow → fixedValue (1), outflow → zeroGradient (0). `Freestream`
            // and `PressureInletOutletVelocity` share this inletOutlet switch.
            Self::InletOutlet { .. }
            | Self::Freestream { .. }
            | Self::PressureInletOutletVelocity => Some(if phi_f < 0.0 { 1.0 } else { 0.0 }),
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
            Self::Freestream { freestream_value } => Some(freestream_value),
            _ => None,
        }
    }
}

impl BoundaryCondition<f64> {
    /// Incompressible total-pressure face value: `p = p0 − 0.5·ρ·|U|²`.
    ///
    /// Physical quantity: the static pressure [Pa] imposed on a
    /// [`TotalPressure`](Self::TotalPressure) patch given a fixed total
    /// (stagnation) pressure and the local dynamic head.
    ///
    /// - `p0` — fixed total (stagnation) pressure [Pa].
    /// - `rho` — density at the patch face [kg·m⁻³] (`ρ ≥ 0`).
    /// - `u_mag` — velocity magnitude `|U|` at the patch face [m·s⁻¹] (`≥ 0`).
    ///
    /// Returns the static pressure [Pa]. Reduces to `p0` at `u_mag = 0`. The
    /// compressible (subsonic) form is deferred — see the
    /// [`TotalPressure`](Self::TotalPressure) docs.
    ///
    /// ```
    /// use outram_foam_basic_lib::fields::boundary::bc::BoundaryCondition;
    /// // At rest the static pressure equals the total pressure.
    /// assert!((BoundaryCondition::total_pressure_value(1.0e5, 1.2, 0.0) - 1.0e5).abs() < 1e-6);
    /// // Moving: p = 1e5 − 0.5·1.2·10² = 1e5 − 60.
    /// assert!((BoundaryCondition::total_pressure_value(1.0e5, 1.2, 10.0) - (1.0e5 - 60.0)).abs() < 1e-6);
    /// ```
    pub fn total_pressure_value(p0: f64, rho: f64, u_mag: f64) -> f64 {
        p0 - 0.5 * rho * u_mag * u_mag
    }

    /// Surface-normal pressure gradient for a
    /// [`FixedFluxPressure`](Self::FixedFluxPressure) face:
    ///
    /// `snGrad(p) = (φ_HbyA − φ_target) / (D_p · |S_f|)`   [Pa·m⁻¹]
    ///
    /// chosen so the pressure-corrected face flux
    /// `φ = φ_HbyA − D_p·snGrad(p)·|S_f|` equals the target flux `φ_target`.
    ///
    /// - `phi_hbya` — momentum-predictor (H/A) face flux `φ_HbyA` [m³·s⁻¹]
    ///   (already includes any body-force term folded into `H/A`).
    /// - `phi_target` — desired boundary face flux `φ_target` [m³·s⁻¹].
    /// - `dp` — face-interpolated `rAU` coefficient `D_p` [m³·s·kg⁻¹].
    /// - `mag_sf` — face area `|S_f|` [m²].
    ///
    /// Returns `0.0` for a degenerate `D_p·|S_f| ≈ 0`.
    ///
    /// ```
    /// use outram_foam_basic_lib::fields::boundary::bc::BoundaryCondition;
    /// let (phi_hbya, phi_target, dp, mag_sf) = (0.30_f64, 0.20, 0.5, 2.0);
    /// let g = BoundaryCondition::fixed_flux_pressure_sn_grad(phi_hbya, phi_target, dp, mag_sf);
    /// // Reconstructed corrected flux reproduces the target.
    /// let phi = phi_hbya - dp * g * mag_sf;
    /// assert!((phi - phi_target).abs() < 1e-12);
    /// ```
    pub fn fixed_flux_pressure_sn_grad(phi_hbya: f64, phi_target: f64, dp: f64, mag_sf: f64) -> f64 {
        let denom = dp * mag_sf;
        if denom.abs() < 1e-300 {
            0.0
        } else {
            (phi_hbya - phi_target) / denom
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

    /// Flux-implied wall-normal velocity for a
    /// [`PressureInletOutletVelocity`](Self::PressureInletOutletVelocity) face:
    /// `U = (φ_f / |S_f|)·n̂` with `n̂ = S_f/|S_f|`.
    ///
    /// Physical quantity: a velocity [m·s⁻¹] parallel to the face normal whose
    /// signed magnitude is `φ_f/|S_f|` — pointing **into** the domain when
    /// `φ_f < 0` (inflow), **out** when `φ_f > 0`. The tangential component is
    /// zero (OpenFOAM's optional `tangentialVelocity` is not modelled).
    ///
    /// - `phi_f` — outward face flux `φ_f = U·S_f` [m³·s⁻¹].
    /// - `area_vector` — the face area vector `S_f = |S_f| n̂` [m²].
    ///
    /// Returns [`Vector3::ZERO`] for a degenerate zero-area face.
    ///
    /// ```
    /// use outram_foam_basic_lib::fields::boundary::bc::BoundaryCondition;
    /// use outram_foam_basic_lib::primitives::Vector3;
    /// let sf = Vector3::new(2.0, 0.0, 0.0);            // area 2 m², normal +x
    /// let u = BoundaryCondition::pressure_inlet_outlet_velocity_value(3.0, sf);
    /// assert!((u.x - 1.5).abs() < 1e-15);              // |U| = φ_f/|S_f| = 3/2
    /// assert!(u.y.abs() < 1e-15 && u.z.abs() < 1e-15); // purely normal
    /// ```
    pub fn pressure_inlet_outlet_velocity_value(phi_f: f64, area_vector: Vector3) -> Vector3 {
        let mag = area_vector.mag();
        if mag < 1e-300 {
            Vector3::ZERO
        } else {
            // S_f · φ_f / |S_f|² = n̂ · (φ_f/|S_f|).
            area_vector * (phi_f / (mag * mag))
        }
    }

    /// Uniform inlet velocity for a
    /// [`FlowRateInletVelocity`](Self::FlowRateInletVelocity) face given the
    /// prescribed volumetric flow rate and the patch-area integral:
    /// `U = −(Q / A_patch)·n̂`, `n̂ = S_f/|S_f|`.
    ///
    /// Physical quantity: an inlet velocity [m·s⁻¹] of magnitude `Q/A_patch`
    /// directed **into** the domain (opposite the outward normal).
    ///
    /// - `q` — prescribed volumetric flow rate `Q` [m³·s⁻¹] (positive = inflow).
    /// - `area_patch` — patch-area integral `A_patch = Σ_f |S_f|` [m²].
    /// - `area_vector` — this face's area vector `S_f` [m²].
    ///
    /// Returns [`Vector3::ZERO`] for a degenerate zero-area face or patch.
    ///
    /// ```
    /// use outram_foam_basic_lib::fields::boundary::bc::BoundaryCondition;
    /// use outram_foam_basic_lib::primitives::Vector3;
    /// // Q = 4 m³/s through a patch of area 2 m², face normal +x.
    /// let sf = Vector3::new(1.0, 0.0, 0.0);
    /// let u = BoundaryCondition::flow_rate_inlet_velocity_value(4.0, 2.0, sf);
    /// assert!((u.x + 2.0).abs() < 1e-15);              // U = −(4/2) x̂ = −2 x̂ (inward)
    /// ```
    pub fn flow_rate_inlet_velocity_value(q: f64, area_patch: f64, area_vector: Vector3) -> Vector3 {
        let mag = area_vector.mag();
        if mag < 1e-300 || area_patch.abs() < 1e-300 {
            Vector3::ZERO
        } else {
            let n = area_vector * (1.0 / mag);
            n * (-(q / area_patch))
        }
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

    /// Solver hook for a [`TotalPressure`](BoundaryCondition::TotalPressure)
    /// patch: recompute the per-face static pressures `p = p0 − 0.5·ρ·|U|²`
    /// [Pa] from the current patch density and velocity magnitude, storing them
    /// in [`values`](Self::values).
    ///
    /// The solver **must** call this each outer iteration, because the boundary
    /// pressure depends on the patch velocity/density (a cross-field dependency
    /// the BC cannot read on its own). No-op if the patch is not a
    /// `TotalPressure` BC.
    ///
    /// - `rho` — density per face [kg·m⁻³], length == patch size.
    /// - `u_mag` — velocity magnitude `|U|` per face [m·s⁻¹], length == patch
    ///   size.
    pub fn update_total_pressure(&mut self, rho: &[f64], u_mag: &[f64]) {
        if let BoundaryCondition::TotalPressure { p0 } = self.bc {
            let n = self.values.len();
            let vals: Vec<f64> = (0..n)
                .map(|i| BoundaryCondition::<f64>::total_pressure_value(p0, rho[i], u_mag[i]))
                .collect();
            self.values = Field::new(vals);
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

    /// Solver hook for a
    /// [`FlowRateInletVelocity`](BoundaryCondition::FlowRateInletVelocity) patch:
    /// fill the per-face uniform inlet velocities `U = −(Q / A_patch)·n̂`
    /// [m·s⁻¹] from the patch face-area vectors, where `A_patch = Σ_f |S_f|`.
    ///
    /// The rate `Q` is read from the BC variant; the velocity magnitude depends
    /// on the patch area, which comes from the mesh, so the solver (or setup
    /// code) calls this once the patch geometry is known. No-op if the patch is
    /// not a `FlowRateInletVelocity` BC.
    ///
    /// - `area_vectors` — this patch's face area vectors `S_f` [m²], length ==
    ///   patch size.
    pub fn update_flow_rate_inlet_velocity(&mut self, area_vectors: &[Vector3]) {
        if let BoundaryCondition::FlowRateInletVelocity {
            volumetric_flow_rate,
        } = self.bc
        {
            let area_patch: f64 = area_vectors.iter().map(|s| s.mag()).sum();
            let vals: Vec<Vector3> = area_vectors
                .iter()
                .map(|s| {
                    BoundaryCondition::flow_rate_inlet_velocity_value(
                        volumetric_flow_rate,
                        area_patch,
                        *s,
                    )
                })
                .collect();
            self.values = Field::new(vals);
        }
    }

    /// Solver hook for a
    /// [`PressureInletOutletVelocity`](BoundaryCondition::PressureInletOutletVelocity)
    /// patch: fill the per-face flux-implied wall-normal velocities
    /// `U = (φ_f / |S_f|)·n̂` [m·s⁻¹] (OpenFOAM's `refValue`).
    ///
    /// The solver **must** call this each iteration with the current face flux,
    /// because the imposed velocity is reconstructed from the flux and face
    /// geometry. The convection operator additionally flux-switches the patch per
    /// face (`fixedValue` on inflow, `zeroGradient` on outflow). No-op if the
    /// patch is not a `PressureInletOutletVelocity` BC.
    ///
    /// - `phi` — outward face flux `φ_f = U·S_f` [m³·s⁻¹] per face, length ==
    ///   patch size.
    /// - `area_vectors` — this patch's face area vectors `S_f` [m²], length ==
    ///   patch size.
    pub fn update_pressure_inlet_outlet_velocity(&mut self, phi: &[f64], area_vectors: &[Vector3]) {
        if matches!(self.bc, BoundaryCondition::PressureInletOutletVelocity) {
            let vals: Vec<Vector3> = area_vectors
                .iter()
                .zip(phi.iter())
                .map(|(s, p)| BoundaryCondition::pressure_inlet_outlet_velocity_value(*p, *s))
                .collect();
            self.values = Field::new(vals);
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

    /// V&V (verification, 2026-08-04). Freestream flux switch: it must behave as
    /// `inletOutlet` — `fixedValue(freestreamValue)` on inflow (`φ_f < 0`),
    /// `zeroGradient` on outflow (`φ_f ≥ 0`). Methodology: check
    /// `flux_value_fraction` and `flux_ref_value` for a scalar Freestream.
    /// Pass criterion: exact enum/float equality.
    /// Result: inflow → Some(1.0); outflow → Some(0.0); ref value → Some(3.5).
    /// PASS. (Untrusted AI-assisted draft pending human V&V.)
    #[test]
    fn vv_freestream_switches_like_inlet_outlet() {
        let fs = BoundaryCondition::Freestream { freestream_value: 3.5_f64 };
        assert_eq!(fs.flux_value_fraction(-1.0), Some(1.0)); // inflow  → fixedValue
        assert_eq!(fs.flux_value_fraction(1.0), Some(0.0)); // outflow → zeroGradient
        assert_eq!(fs.flux_ref_value(), Some(&3.5));
    }

    /// V&V (verification, 2026-08-04). pressureInletOutletVelocity reconstructs
    /// the wall-normal velocity from the face flux: `U = (φ_f/|S_f|)·n̂`.
    /// Methodology: for `S_f = (2,0,0)` (area 2 m², normal +x) and `φ_f = 3`,
    /// the analytic result is `U = 1.5 x̂` (|U| = φ_f/|S_f| = 1.5), purely
    /// normal. Also check the flux switch (`1 − pos0`). Pass criterion:
    /// |U−1.5x̂| < 1e-15; switch exact.
    /// Result: U = (1.5, 0, 0); inflow fraction 1.0, outflow 0.0. PASS.
    /// (Untrusted AI-assisted draft pending human V&V.)
    #[test]
    fn vv_pressure_inlet_outlet_velocity_from_flux() {
        let sf = Vector3::new(2.0, 0.0, 0.0);
        let u = BoundaryCondition::pressure_inlet_outlet_velocity_value(3.0, sf);
        assert!((u.x - 1.5).abs() < 1e-15, "|U| should be φ_f/|S_f| = 1.5, got {}", u.x);
        assert!(u.y.abs() < 1e-15 && u.z.abs() < 1e-15, "velocity must be purely normal");
        let bc = BoundaryCondition::<Vector3>::PressureInletOutletVelocity;
        assert_eq!(bc.flux_value_fraction(-1.0), Some(1.0)); // inflow  → fixedValue
        assert_eq!(bc.flux_value_fraction(1.0), Some(0.0)); // outflow → zeroGradient
        // Sign: outflow flux → outward velocity; inflow flux → inward.
        let out = BoundaryCondition::pressure_inlet_outlet_velocity_value(4.0, sf);
        let inn = BoundaryCondition::pressure_inlet_outlet_velocity_value(-4.0, sf);
        assert!(out.x > 0.0 && inn.x < 0.0);
    }

    /// V&V (verification, 2026-08-04). fixedFluxPressure yields the `snGrad(p)`
    /// that reproduces a prescribed corrected face flux. Methodology: with
    /// `φ_HbyA = 0.30`, `φ_target = 0.20`, `D_p = 0.5`, `|S_f| = 2.0`, compute
    /// `snGrad = (φ_HbyA−φ_target)/(D_p·|S_f|)` and reconstruct the corrected
    /// flux `φ = φ_HbyA − D_p·snGrad·|S_f|`. Pass criterion: reconstructed
    /// `φ = φ_target` to < 1e-12; expected snGrad = 0.1/1.0 = 0.1 Pa/m.
    /// Result: snGrad = 0.1; reconstructed φ = 0.20. PASS.
    /// (Untrusted AI-assisted draft pending human V&V.)
    #[test]
    fn vv_fixed_flux_pressure_sn_grad_reproduces_flux() {
        let (phi_hbya, phi_target, dp, mag_sf) = (0.30_f64, 0.20, 0.5, 2.0);
        let g = BoundaryCondition::fixed_flux_pressure_sn_grad(phi_hbya, phi_target, dp, mag_sf);
        assert!((g - 0.1).abs() < 1e-12, "snGrad should be 0.1 Pa/m, got {g}");
        let phi = phi_hbya - dp * g * mag_sf;
        assert!((phi - phi_target).abs() < 1e-12, "corrected flux must equal target");
        // Degenerate coefficient → zero gradient, not NaN/inf.
        assert_eq!(
            BoundaryCondition::fixed_flux_pressure_sn_grad(0.3, 0.2, 0.0, 2.0),
            0.0
        );
    }

    /// V&V (verification, 2026-08-04). totalPressure reduces to fixedValue(p0)
    /// at rest and gives `p0 − 0.5·ρ·|U|²` when moving. Methodology: p0 = 1e5 Pa,
    /// ρ = 1.2 kg/m³. At |U| = 0 → p = p0; at |U| = 10 → p = 1e5 − 0.5·1.2·100 =
    /// 1e5 − 60. Also exercise the PatchField solver hook over 2 faces. Pass
    /// criterion: |Δ| < 1e-6.
    /// Result: p(0) = 100000.0; p(10) = 99940.0; hook writes [100000, 99940].
    /// PASS. (Untrusted AI-assisted draft pending human V&V.)
    #[test]
    fn vv_total_pressure_dynamic_head() {
        let p0 = 1.0e5_f64;
        let rho = 1.2;
        assert!((BoundaryCondition::total_pressure_value(p0, rho, 0.0) - p0).abs() < 1e-6);
        assert!(
            (BoundaryCondition::total_pressure_value(p0, rho, 10.0) - (p0 - 60.0)).abs() < 1e-6
        );
        // Solver hook: two faces, one at rest, one at |U| = 10.
        let mut pf = PatchField {
            bc: BoundaryCondition::TotalPressure { p0 },
            values: Field::new(vec![0.0, 0.0]),
        };
        pf.update_total_pressure(&[rho, rho], &[0.0, 10.0]);
        assert!((pf.values[0] - p0).abs() < 1e-6);
        assert!((pf.values[1] - (p0 - 60.0)).abs() < 1e-6);
    }

    /// V&V (verification, 2026-08-04). flowRateInletVelocity gives a patch-flux
    /// integral equal to the specified `Q`. Methodology: a 3-face patch with
    /// area vectors summing to `A_patch = 4 m²` (faces of area 1, 1, 2, all
    /// normal +x). Prescribe `Q = 8 m³/s`. Fill per-face `U = −(Q/A)·n̂`, then
    /// measure the inward volumetric flux `−Σ_f U·S_f`. Pass criterion:
    /// measured flux = Q to < 1e-12, and every face velocity magnitude = Q/A = 2.
    /// Result: |U| = 2 m/s inward; measured inward flux = 8.0 m³/s = Q. PASS.
    /// (Untrusted AI-assisted draft pending human V&V.)
    #[test]
    fn vv_flow_rate_inlet_velocity_integral_matches_q() {
        let q = 8.0_f64;
        let sfs = vec![
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(2.0, 0.0, 0.0),
        ];
        let mut pf = PatchField {
            bc: BoundaryCondition::FlowRateInletVelocity {
                volumetric_flow_rate: q,
            },
            values: Field::new(vec![Vector3::ZERO; 3]),
        };
        pf.update_flow_rate_inlet_velocity(&sfs);
        let area: f64 = sfs.iter().map(|s| s.mag()).sum();
        // Measured inward volumetric flux = −Σ U·S_f.
        let measured: f64 = pf
            .values
            .as_slice()
            .iter()
            .zip(&sfs)
            .map(|(u, s)| -u.dot(*s))
            .sum();
        assert!((measured - q).abs() < 1e-12, "patch flux {measured} must equal Q = {q}");
        for u in pf.values.as_slice() {
            assert!((u.mag() - q / area).abs() < 1e-12); // |U| = Q/A = 2 m/s
            assert!(u.x < 0.0); // directed into the domain (−x̂)
        }
    }
}
