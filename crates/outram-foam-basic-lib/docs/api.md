# Crate Documentation

**Version:** 0.1.1

**Format Version:** 60

# Module `outram_foam_basic_lib`

**This is OUTRAM PARK's independent Rust translation of selected
OpenFOAM® primitive/finite-volume algorithms — it is not the official
OpenFOAM® software and is not affiliated with, endorsed by, or
sanctioned by OpenCFD Ltd. or the ESI Group.** OpenFOAM® is a registered
trademark of OpenCFD Limited. See `TRADEMARKS.md` (this crate's
directory, mirrored from the workspace root) for the full attribution
and non-affiliation notice.

## Modules

## Module `fields`

Layer 2 — field containers (`VolField`/`SurfaceField`), boundary
conditions, and field-level tensor algebra.
Field types: the discretised quantities carried on the mesh.

This module holds the data containers the FV operators read and write:

- [`Field`](crate::fields::field::Field) — a flat `Vec<T>` with element-wise arithmetic; the raw storage
  with no mesh or dimension bookkeeping (mirrors `Foam::Field<Type>`).
- [`boundary`](crate::fields::boundary) — boundary conditions
  ([`BoundaryCondition`](crate::fields::boundary::bc::BoundaryCondition)) and
  per-patch boundary values
  ([`PatchField`](crate::fields::boundary::bc::PatchField)).
- [`VolField`](crate::fields::vol_field::VolField) (and the `Vol*Field` aliases) — cell-centred volume fields:
  one value per cell plus one `PatchField` per boundary patch.
- [`SurfaceField`](crate::fields::surface_field::SurfaceField) (and the `Surface*Field` aliases) — face fields: one
  value per internal face plus one `PatchField` per boundary patch.
- [`vol_field_algebra`](crate::fields::vol_field_algebra) — pure per-element tensor algebra (`tr`, `symm`,
  `dev`, …) lifted to whole volume fields.

Physical units are not tracked at this layer; a field simply carries `f64`,
`Vector3`, `Tensor`, or `SymmTensor` values in whatever SI units the caller
assigns them.

```rust
pub mod fields { /* ... */ }
```

### Modules

## Module `boundary`

Boundary conditions and per-patch boundary field values.

Defines [`BoundaryCondition`] (the BC variant applied on a patch — fixed
value, zero gradient, symmetry, empty, calculated) and [`PatchField`] (the
BC together with the current face values it holds for one patch).

```rust
pub mod boundary { /* ... */ }
```

### Modules

## Module `bc`

```rust
pub mod bc { /* ... */ }
```

### Types

#### Enum `BoundaryCondition`

Boundary condition variant for a single patch.

Covers the patch-field boundary conditions ported from OpenFOAM's
`finiteVolume/fields/fvPatchFields`.  The set is closed and dispatched by
enum (no `dyn`), so adding a variant forces every exhaustive `match` site to
be updated — the compiler flags each one.

# Units

The variants are unit-agnostic in `T`: `T` is whatever the field stores
(`f64` for a scalar field, [`Vector3`] for a vector field, …).  Where a
variant stores a *gradient* (`FixedGradient`, `Mixed::ref_grad`) the value
is a normal gradient in units of *field-value per metre* (`[T]·m⁻¹`), because
the boundary face value is reconstructed as `cell_value + gradient · delta`
with `delta` the owner-cell-centre-to-face distance in metres.

# Status

The `FixedGradient`, `Mixed`, `InletOutlet`, `OutletInlet`, `Slip`,
`NoSlip`, and `Wedge` variants (added 2026-08-04) are an **untrusted
AI-assisted draft pending human V&V review** — verified against
analytic/limiting cases (see the `vv_*` tests) but not yet human-reviewed.
`Wedge` in particular is a zero-gradient stand-in, not the full rotation.

The flow-context variants `Freestream`, `PressureInletOutletVelocity`,
`FixedFluxPressure`, `TotalPressure`, and `FlowRateInletVelocity` (added
2026-08-04, Wave 4) are likewise an **untrusted AI-assisted draft pending
human V&V review** — verified against analytic/definition cases (the `vv_*`
tests) but not yet human-reviewed. `Freestream` is self-contained
(flux-switched by the convection operator like `InletOutlet`); the other
four are **solver-driven** — they depend on context the per-face BC update
cannot supply on its own (`PressureInletOutletVelocity` and
`FixedFluxPressure` on the face flux / momentum-predictor flux,
`TotalPressure` on the patch velocity and density, `FlowRateInletVelocity`
on the patch-area integral), so the solver must refresh their stored face
values / gradient each iteration through the documented `update_*` /
`*_value` hooks below rather than the BC hard-coding a wrong value.

```rust
pub enum BoundaryCondition<T: Clone> {
    FixedValue(T),
    FixedField(crate::fields::field::Field<T>),
    ZeroGradient,
    FixedGradient(T),
    Mixed {
        value_fraction: f64,
        ref_value: T,
        ref_grad: T,
    },
    InletOutlet {
        inlet_value: T,
    },
    OutletInlet {
        outlet_value: T,
    },
    Symmetry,
    Slip,
    NoSlip,
    Wedge,
    Empty,
    Calculated(crate::fields::field::Field<T>),
    Freestream {
        freestream_value: T,
    },
    PressureInletOutletVelocity,
    FixedFluxPressure {
        gradient: T,
    },
    TotalPressure {
        p0: T,
    },
    FlowRateInletVelocity {
        volumetric_flow_rate: f64,
    },
}
```

##### Variants

###### `FixedValue`

Dirichlet: fixed uniform value.

OpenFOAM: `fixedValueFvPatchField`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `T` |  |

###### `FixedField`

Dirichlet: fixed per-face values.

OpenFOAM: `fixedValueFvPatchField` (non-uniform list form).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::fields::field::Field<T>` |  |

###### `ZeroGradient`

Neumann: zero normal gradient — boundary face value = internal adjacent value.

OpenFOAM: `zeroGradientFvPatchField`.

###### `FixedGradient`

Neumann with a prescribed **non-zero** normal gradient `g` (`[T]·m⁻¹`).

The boundary face value is `φ_face = φ_cell + g · delta`, where `delta`
`[m]` is the owner-cell-centre-to-face-centre distance.  Reduces to
[`ZeroGradient`](Self::ZeroGradient) when `g = 0`.

OpenFOAM: `fixedGradientFvPatchField`
(`src/finiteVolume/fields/fvPatchFields/derived/fixedGradient/fixedGradientFvPatchField.H`).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `T` |  |

###### `Mixed`

Robin / mixed boundary condition — a per-face blend of a Dirichlet part
(`fixedValue`, weight `value_fraction`) and a Neumann part
(`fixedGradient`, weight `1 - value_fraction`).

With `w = value_fraction ∈ [0, 1]`, `delta` `[m]` the cell-to-face
distance, `φ_c` the owner cell value:

- face value: `φ_face = w·ref_value + (1 - w)·(φ_c + ref_grad·delta)`
- it reduces to [`FixedValue`](Self::FixedValue)`(ref_value)` at `w = 1`
  and to [`FixedGradient`](Self::FixedGradient)`(ref_grad)` at `w = 0`.

This is the general form underlying every value/gradient-blending BC,
including the albedo / Robin condition used in neutron diffusion.

- `value_fraction` — dimensionless weight in `[0, 1]`.
- `ref_value` — the Dirichlet reference value (`[T]`).
- `ref_grad` — the Neumann reference normal gradient (`[T]·m⁻¹`).

OpenFOAM: `mixedFvPatchField`
(`src/finiteVolume/fields/fvPatchFields/basic/mixed/mixedFvPatchField.H`).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `value_fraction` | `f64` | Dirichlet/Neumann blend weight, dimensionless, `∈ [0, 1]`. |
| `ref_value` | `T` | Dirichlet reference value (`[T]`). |
| `ref_grad` | `T` | Neumann reference normal gradient (`[T]·m⁻¹`). |

###### `InletOutlet`

Flux-switched inflow/outflow BC: behaves as
[`FixedValue`](Self::FixedValue)`(inlet_value)` on **inflow** faces and
[`ZeroGradient`](Self::ZeroGradient) on **outflow** faces.

The switch is decided per face by the sign of the outward face flux
`φ_f = U·S_f` `[m³·s⁻¹]`: `φ_f < 0` is inflow (fixed value), `φ_f ≥ 0` is
outflow (zero gradient).  Equivalent to a [`Mixed`](Self::Mixed) BC whose
`value_fraction` is set to `1` on inflow and `0` on outflow.  The flux is
supplied by the convection operator at assembly time, so this variant is
only flux-switched inside operators that carry `phi`.

OpenFOAM: `inletOutletFvPatchField`
(`src/finiteVolume/fields/fvPatchFields/derived/inletOutlet/inletOutletFvPatchField.H`).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `inlet_value` | `T` | Value imposed on inflow faces (`[T]`). |

###### `OutletInlet`

Flux-switched outflow/inflow BC — the opposite of
[`InletOutlet`](Self::InletOutlet): [`FixedValue`](Self::FixedValue)`(outlet_value)`
on **outflow** faces (`φ_f ≥ 0`) and [`ZeroGradient`](Self::ZeroGradient)
on **inflow** faces (`φ_f < 0`).

OpenFOAM: `outletInletFvPatchField`
(`src/finiteVolume/fields/fvPatchFields/derived/outletInlet/outletInletFvPatchField.H`).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `outlet_value` | `T` | Value imposed on outflow faces (`[T]`). |

###### `Symmetry`

Symmetry plane — normal component zeroed.

OpenFOAM: `symmetryFvPatchField`.

###### `Slip`

Free-slip wall: the wall-normal component of a vector field is removed
(as for [`Symmetry`](Self::Symmetry)) while the tangential component is
zero-gradient.  For a scalar field it is exactly zero-gradient.

See [`BoundaryCondition::<Vector3>::slip_face_value`] for the exact
vector reconstruction.

OpenFOAM: `slipFvPatchField`
(`src/finiteVolume/fields/fvPatchFields/derived/slip/slipFvPatchField.H`).

###### `NoSlip`

No-slip wall for velocity: a `fixedValue` of zero.  Semantically it is
[`FixedValue`](Self::FixedValue)`(T::zero)` specialised to walls; the
stored patch values are all zero.

OpenFOAM: `noSlipFvPatchField`
(`src/finiteVolume/fields/fvPatchFields/derived/noSlip/noSlipFvPatchField.H`).

###### `Wedge`

Axisymmetric wedge patch (`wedgeFvPatchField`).

**First-pass simplification (Layer-1):** treated as zero-gradient — the
patch face value equals the adjacent internal cell value.  A full wedge
BC rotates the patch-internal field onto the wedge face about the
geometric axis (pairing across the wedge like a cyclic); that rotation is
**not yet implemented** here.  The zero-gradient stand-in is exact only
for a field that is uniform in the wedge (azimuthal) direction.  Do not
treat wedge results as validated until the rotation transform lands.

OpenFOAM: `wedgeFvPatchField`
(`src/finiteVolume/fields/fvPatchFields/constraint/wedge/wedgeFvPatchField.H`).

###### `Empty`

2-D / empty — zero-area faces; value has no physical meaning.

OpenFOAM: `emptyFvPatchField`.

###### `Calculated`

Value computed by the solver and stored here (read-only from BC side).

OpenFOAM: `calculatedFvPatchField`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::fields::field::Field<T>` |  |

###### `Freestream`

Freestream (far-field) inflow/outflow BC — an [`InletOutlet`](Self::InletOutlet)
specialised to external / far-field flow: it imposes the uniform
freestream value on **inflow** faces and is [`ZeroGradient`](Self::ZeroGradient)
on **outflow** faces, switched per face by the sign of the outward face
flux `φ_f = U·S_f` `[m³·s⁻¹]` (`φ_f < 0` inflow, `φ_f ≥ 0` outflow).

For a velocity field `freestream_value` is the far-field velocity `U_∞`
`[m·s⁻¹]`; for a scalar it is the far-field scalar value (`[T]`). It is
**self-contained**: the flux is supplied by the convection operator at
assembly time, exactly like [`InletOutlet`](Self::InletOutlet), so no
solver hook is needed. See [`flux_value_fraction`](Self::flux_value_fraction)
/ [`flux_ref_value`](Self::flux_ref_value).

OpenFOAM: `freestreamFvPatchField`
(`src/finiteVolume/fields/fvPatchFields/derived/freestream/freestreamFvPatchField.H`),
which derives from `inletOutletFvPatchField` with `inletValue = freestreamValue`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `freestream_value` | `T` | Far-field freestream value imposed on inflow faces (`[T]`; for a<br>velocity field, m·s⁻¹). |

###### `PressureInletOutletVelocity`

Velocity BC for a pressure-driven inlet/outlet patch: the patch velocity
is reconstructed from the face flux `φ_f` `[m³·s⁻¹]` and the face area.

On **outflow** (`φ_f ≥ 0`) it is [`ZeroGradient`](Self::ZeroGradient); on
**inflow** (`φ_f < 0`) it imposes the flux-implied wall-normal velocity
`U = (φ_f / |S_f|)·n̂` `[m·s⁻¹]`, where `n̂ = S_f/|S_f|` is the unit outward
face normal. (OpenFOAM sets `valueFraction = 1 − pos0(φ_f)`, i.e.
`fixedValue` on inflow and `zeroGradient` on outflow; the imposed value is
the normal velocity above, the tangential component taken as zero here —
OpenFOAM's optional `tangentialVelocity` is not modelled.)

The per-face imposed velocity depends on the flux and face geometry, which
a value-only variant cannot carry, so this variant is **solver-driven**:
the solver refreshes [`PatchField::values`] each iteration via
[`PatchField::update_pressure_inlet_outlet_velocity`], and the convection
operator additionally flux-switches it per face
([`flux_value_fraction`](Self::flux_value_fraction) returns the inflow/
outflow weight). The pure per-face formula is
[`pressure_inlet_outlet_velocity_value`](BoundaryCondition::<Vector3>::pressure_inlet_outlet_velocity_value).

OpenFOAM: `pressureInletOutletVelocityFvPatchVectorField`.

###### `FixedFluxPressure`

Pressure BC that fixes the surface-normal pressure gradient `snGrad(p)`
`[Pa·m⁻¹]` so the pressure-corrected face flux matches a target flux — the
natural wall / outlet pressure condition in a PISO/PIMPLE pressure solve.

It behaves as a [`FixedGradient`](Self::FixedGradient)`(gradient)` whose
gradient the solver sets each pressure solve from the flux mismatch:

`snGrad(p) = (φ_HbyA − φ_target) / (D_p · |S_f|)`

where `φ_HbyA` `[m³·s⁻¹]` is the momentum-predictor (H/A) face flux,
`φ_target` `[m³·s⁻¹]` the desired boundary flux, `D_p` `[m³·s·kg⁻¹]` the
face-interpolated `rAU` (interpolated `1/A_p` from the momentum-matrix
diagonal, which absorbs any body-force term folded into `H/A`), and `|S_f|`
`[m²]` the face area. The gradient is **solver-set** because it needs the
predictor flux and the `rAU` field, which this Layer does not own. See
[`fixed_flux_pressure_sn_grad`](BoundaryCondition::<f64>::fixed_flux_pressure_sn_grad)
for the pure formula. The stored `gradient` is uniform over the patch
(matching [`FixedGradient`](Self::FixedGradient)); the diffusion and
`snGrad` operators consume it as a prescribed normal gradient.

OpenFOAM: `fixedFluxPressureFvPatchScalarField`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `gradient` | `T` | Currently-set surface-normal pressure gradient `snGrad(p)` `[Pa·m⁻¹]`,<br>uniform over the patch. |

###### `TotalPressure`

Total-pressure (stagnation-pressure) inlet/outlet BC: the static boundary
pressure is set from a fixed total pressure `p0` and the local dynamic
head. Incompressible form:

`p = p0 − 0.5·ρ·|U|²`

with `p`, `p0` in Pa, `ρ` in kg·m⁻³, `|U|` in m·s⁻¹. At rest (`|U| = 0`)
it reduces to [`FixedValue`](Self::FixedValue)`(p0)`. The compressible
(subsonic) form `p = p0 (1 + ((γ−1)/2)·M²)^(−γ/(γ−1))` is **deferred**.

This needs the velocity magnitude and density **at the patch** — a
cross-field dependency a per-face BC update cannot supply on its own — so
this variant is **solver-driven**: the solver refreshes
[`PatchField::values`] every iteration via
[`PatchField::update_total_pressure`] (which reads `p0` from this variant
and applies
[`total_pressure_value`](BoundaryCondition::<f64>::total_pressure_value)).
At assembly time it acts as a [`FixedValue`](Self::FixedValue) holding the
last-computed face pressure.

OpenFOAM: `totalPressureFvPatchScalarField`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `p0` | `T` | Fixed total (stagnation) pressure `p0` `[Pa]`. |

###### `FlowRateInletVelocity`

Uniform inlet-velocity BC scaled to a prescribed volumetric flow rate:
the whole patch is given a uniform velocity directed **into** the domain
whose magnitude makes the patch-integral volumetric flux equal `Q`:

`U = −(Q / A_patch)·n̂`,  `A_patch = Σ_f |S_f|`

with `U` in m·s⁻¹, `Q` in m³·s⁻¹, `A_patch` in m², `n̂ = S_f/|S_f|` the
unit outward face normal (so `−n̂` points into the domain). The patch-area
integral comes from the mesh, so this variant is **geometry/solver-driven**:
the per-face values are filled from the patch face-area vectors by
[`PatchField::update_flow_rate_inlet_velocity`] (the fixed quantity is the
rate `Q`; the resulting velocity depends on the patch area). At assembly it
acts as a [`FixedValue`](Self::FixedValue) inlet. Per-face formula:
[`flow_rate_inlet_velocity_value`](BoundaryCondition::<Vector3>::flow_rate_inlet_velocity_value).

Only the volumetric form is modelled; the mass-flow form
`U = −(ṁ / (ρ·A))·n̂` is obtained by passing `Q = ṁ/ρ`.

OpenFOAM: `flowRateInletVelocityFvPatchVectorField`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `volumetric_flow_rate` | `f64` | Prescribed volumetric flow rate `Q` `[m³·s⁻¹]` (positive = into the<br>domain). |

##### Implementations

###### Methods

- ```rust
  pub fn is_fixed_value(self: &Self) -> bool { /* ... */ }
  ```
  True if the BC imposes a value (Dirichlet-like) unconditionally.

- ```rust
  pub fn flux_value_fraction(self: &Self, phi_f: f64) -> Option<f64> { /* ... */ }
  ```
  Value fraction (`1` ⇒ acts as `fixedValue`, `0` ⇒ acts as

- ```rust
  pub fn flux_ref_value(self: &Self) -> Option<&T> { /* ... */ }
  ```
  The reference (Dirichlet) value of a flux-switched BC, if it has one.

- ```rust
  pub fn total_pressure_value(p0: f64, rho: f64, u_mag: f64) -> f64 { /* ... */ }
  ```
  Incompressible total-pressure face value: `p = p0 − 0.5·ρ·|U|²`.

- ```rust
  pub fn fixed_flux_pressure_sn_grad(phi_hbya: f64, phi_target: f64, dp: f64, mag_sf: f64) -> f64 { /* ... */ }
  ```
  Surface-normal pressure gradient for a

- ```rust
  pub fn slip_face_value(internal: Vector3, unit_normal: Vector3) -> Vector3 { /* ... */ }
  ```
  Free-slip face value for a vector field: the wall-normal component is

- ```rust
  pub fn pressure_inlet_outlet_velocity_value(phi_f: f64, area_vector: Vector3) -> Vector3 { /* ... */ }
  ```
  Flux-implied wall-normal velocity for a

- ```rust
  pub fn flow_rate_inlet_velocity_value(q: f64, area_patch: f64, area_vector: Vector3) -> Vector3 { /* ... */ }
  ```
  Uniform inlet velocity for a

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> BoundaryCondition<T> { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `PatchField`

Boundary field for one patch: the BC type plus the current face values.

The `values` field always holds the latest face values (updated by
`update_coeffs` in Layer 3 operators).  For `FixedValue`/`FixedField` the
values are set at construction and never change.  For `ZeroGradient` and
`Calculated` they are written by the operator code.

```rust
pub struct PatchField<T: Clone> {
    pub bc: BoundaryCondition<T>,
    pub values: crate::fields::field::Field<T>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `bc` | `BoundaryCondition<T>` | The boundary condition applied on this patch. |
| `values` | `crate::fields::field::Field<T>` | Current face values for this patch (length == patch.size). |

##### Implementations

###### Methods

- ```rust
  pub fn fixed_value(size: usize, v: f64) -> Self { /* ... */ }
  ```
  Dirichlet patch holding a uniform scalar `v` on all `size` faces.

- ```rust
  pub fn zero_gradient(size: usize) -> Self { /* ... */ }
  ```
  Zero-gradient (Neumann) scalar patch of `size` faces; values default to

- ```rust
  pub fn empty() -> Self { /* ... */ }
  ```
  Empty (zero-area) scalar patch — no faces, no physical value.

- ```rust
  pub fn update_total_pressure(self: &mut Self, rho: &[f64], u_mag: &[f64]) { /* ... */ }
  ```
  Solver hook for a [`TotalPressure`](BoundaryCondition::TotalPressure)

- ```rust
  pub fn fixed_value_vec(size: usize, v: Vector3) -> Self { /* ... */ }
  ```
  Dirichlet patch holding a uniform `Vector3` value `v` on all `size` faces.

- ```rust
  pub fn zero_gradient_vec(size: usize) -> Self { /* ... */ }
  ```
  Zero-gradient (Neumann) vector patch of `size` faces; values default to

- ```rust
  pub fn empty_vec() -> Self { /* ... */ }
  ```
  Empty (zero-area) vector patch — no faces, no physical value.

- ```rust
  pub fn update_flow_rate_inlet_velocity(self: &mut Self, area_vectors: &[Vector3]) { /* ... */ }
  ```
  Solver hook for a

- ```rust
  pub fn update_pressure_inlet_outlet_velocity(self: &mut Self, phi: &[f64], area_vectors: &[Vector3]) { /* ... */ }
  ```
  Solver hook for a

- ```rust
  pub fn fixed_value_tensor(size: usize, v: Tensor) -> Self { /* ... */ }
  ```
  Dirichlet patch holding a uniform `Tensor` value.

- ```rust
  pub fn zero_gradient_tensor(size: usize) -> Self { /* ... */ }
  ```
  Zero-gradient (Neumann) patch for a `Tensor` field; values default to

- ```rust
  pub fn fixed_value_symm_tensor(size: usize, v: SymmTensor) -> Self { /* ... */ }
  ```
  Dirichlet patch holding a uniform `SymmTensor` value.

- ```rust
  pub fn zero_gradient_symm_tensor(size: usize) -> Self { /* ... */ }
  ```
  Zero-gradient (Neumann) patch for a `SymmTensor` field; values default to

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> PatchField<T> { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Re-exports

#### Re-export `bc::*`

```rust
pub use bc::*;
```

## Module `field`

```rust
pub mod field { /* ... */ }
```

### Types

#### Struct `Field`

A flat array over all cells or faces, with element-wise arithmetic.

Mirrors `Foam::Field<Type>` from `src/OpenFOAM/fields/Fields/Field/Field.H`.
The raw storage is `Vec<T>` with no dimension bookkeeping — that lives in
the wrapping `VolField`/`SurfaceField`.

```rust
pub struct Field<T> {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(data: Vec<T>) -> Self { /* ... */ }
  ```
  Wrap an existing `Vec<T>` as a field (no copy).

- ```rust
  pub fn uniform(n: usize, value: T) -> Self { /* ... */ }
  ```
  Field of `n` elements all equal to `value`.

- ```rust
  pub fn from_fn</* synthetic */ impl Fn(usize) -> T: Fn(usize) -> T>(n: usize, f: impl Fn(usize) -> T) -> Self { /* ... */ }
  ```
  Field of `n` elements, element `i` set to `f(i)`.

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Number of elements in the field.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  True if the field has no elements.

- ```rust
  pub fn as_slice(self: &Self) -> &[T] { /* ... */ }
  ```
  Borrow the underlying storage as a slice.

- ```rust
  pub fn as_mut_slice(self: &mut Self) -> &mut [T] { /* ... */ }
  ```
  Mutably borrow the underlying storage as a slice.

- ```rust
  pub fn into_vec(self: Self) -> Vec<T> { /* ... */ }
  ```
  Consume the field, returning its underlying `Vec<T>`.

- ```rust
  pub fn iter(self: &Self) -> std::slice::Iter<''_, T> { /* ... */ }
  ```
  Iterator over element references.

- ```rust
  pub fn iter_mut(self: &mut Self) -> std::slice::IterMut<''_, T> { /* ... */ }
  ```
  Iterator over mutable element references.

- ```rust
  pub fn map<U: Clone, /* synthetic */ impl Fn(&T) -> U: Fn(&T) -> U>(self: &Self, f: impl Fn(&T) -> U) -> Field<U> { /* ... */ }
  ```
  Map `f` element-wise, returning a new `Field<U>` of the same length.

- ```rust
  pub fn zeros(n: usize) -> Self { /* ... */ }
  ```
  Scalar field of `n` zeros.

- ```rust
  pub fn ones(n: usize) -> Self { /* ... */ }
  ```
  Scalar field of `n` ones.

- ```rust
  pub fn sum(self: &Self) -> f64 { /* ... */ }
  ```
  Sum of all elements.

- ```rust
  pub fn mean(self: &Self) -> f64 { /* ... */ }
  ```
  Arithmetic mean of all elements; returns `0.0` for an empty field.

- ```rust
  pub fn min(self: &Self) -> f64 { /* ... */ }
  ```
  Smallest element (`+∞` for an empty field).

- ```rust
  pub fn max(self: &Self) -> f64 { /* ... */ }
  ```
  Largest element (`−∞` for an empty field).

- ```rust
  pub fn l2_norm(self: &Self) -> f64 { /* ... */ }
  ```
  Euclidean (L2) norm: `sqrt(sum(x_i²))`.

- ```rust
  pub fn abs(self: &Self) -> Self { /* ... */ }
  ```
  Element-wise absolute value.

- ```rust
  pub fn clamp(self: &Self, lo: f64, hi: f64) -> Self { /* ... */ }
  ```
  Element-wise clamp.

- ```rust
  pub fn pointwise_mul(self: &Self, rhs: &Self) -> Self { /* ... */ }
  ```
  Element-wise product of two scalar fields.

- ```rust
  pub fn pointwise_div(self: &Self, rhs: &Self) -> Self { /* ... */ }
  ```
  Element-wise division of two scalar fields.

- ```rust
  pub fn weighted_sum(self: &Self, weights: &Field<f64>) -> f64 { /* ... */ }
  ```
  Weighted sum: `sum(w[i] * x[i])`.

- ```rust
  pub fn zero_vec(n: usize) -> Self { /* ... */ }
  ```
  Vector field of `n` zero vectors.

- ```rust
  pub fn dot_field(self: &Self, rhs: &Field<Vector3>) -> Field<f64> { /* ... */ }
  ```
  Element-wise dot product → scalar field.

- ```rust
  pub fn scale(self: &Self, s: &Field<f64>) -> Self { /* ... */ }
  ```
  Scale each element by the corresponding scalar field entry.

###### Trait Implementations

- **Add**
  - ```rust
    fn add(self: Self, rhs: Self) -> Self { /* ... */ }
    ```

- **AddAssign**
  - ```rust
    fn add_assign(self: &mut Self, rhs: Self) { /* ... */ }
    ```

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **AsRef**
  - ```rust
    fn as_ref(self: &Self) -> &[T] { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Field<T> { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

  - ```rust
    fn default() -> Self { /* ... */ }
    ```

  - ```rust
    fn default() -> Self { /* ... */ }
    ```

  - ```rust
    fn default() -> Self { /* ... */ }
    ```

  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **Div**
  - ```rust
    fn div(self: Self, rhs: f64) -> Self { /* ... */ }
    ```

  - ```rust
    fn div(self: Self, rhs: Self) -> Self { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(v: Vec<T>) -> Self { /* ... */ }
    ```

- **Index**
  - ```rust
    fn index(self: &Self, i: usize) -> &T { /* ... */ }
    ```

- **IndexMut**
  - ```rust
    fn index_mut(self: &mut Self, i: usize) -> &mut T { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoIterator**
  - ```rust
    fn into_iter(self: Self) -> <Self as >::IntoIter { /* ... */ }
    ```

  - ```rust
    fn into_iter(self: Self) -> <Self as >::IntoIter { /* ... */ }
    ```

- **Mul**
  - ```rust
    fn mul(self: Self, rhs: f64) -> Self { /* ... */ }
    ```

  - ```rust
    fn mul(self: Self, rhs: Field<f64>) -> Field<f64> { /* ... */ }
    ```

  - ```rust
    fn mul(self: Self, rhs: Field<Vector3>) -> Field<Vector3> { /* ... */ }
    ```

  - ```rust
    fn mul(self: Self, rhs: Self) -> Self { /* ... */ }
    ```

  - ```rust
    fn mul(self: Self, rhs: Field<f64>) -> Field<Vector3> { /* ... */ }
    ```

  - ```rust
    fn mul(self: Self, rhs: Field<Vector3>) -> Field<Vector3> { /* ... */ }
    ```

- **Neg**
  - ```rust
    fn neg(self: Self) -> Self { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Field<T>) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sub**
  - ```rust
    fn sub(self: Self, rhs: Self) -> Self { /* ... */ }
    ```

- **SubAssign**
  - ```rust
    fn sub_assign(self: &mut Self, rhs: Self) { /* ... */ }
    ```

- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `surface_field`

```rust
pub mod surface_field { /* ... */ }
```

### Types

#### Struct `SurfaceField`

A surface field: one value per *internal* face in the internal field, plus
one `PatchField` per boundary patch.

Mirrors `Foam::surfaceScalarField` / `Foam::SurfaceField<Type>`.

## Why `internal` has length `n_internal_faces`, not `n_faces`

In OpenFOAM, `surfaceScalarField.internalField()` only covers the internal
faces; boundary-face values live in `boundaryField()[patch]`.  This matches
the LDU matrix structure: `lower` and `upper` arrays have length
`n_internal_faces`.

```rust
pub struct SurfaceField<T: Clone> {
    pub name: String,
    pub mesh: std::sync::Arc<crate::mesh::fv_mesh::FvMesh>,
    pub internal: super::field::Field<T>,
    pub boundary: Vec<super::boundary::bc::PatchField<T>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Field name (diagnostic label, e.g. `"phi"`). |
| `mesh` | `std::sync::Arc<crate::mesh::fv_mesh::FvMesh>` | Mesh this field is defined on. |
| `internal` | `super::field::Field<T>` | Face values for all internal faces; length == `mesh.n_internal_faces`. |
| `boundary` | `Vec<super::boundary::bc::PatchField<T>>` | One entry per boundary patch; `boundary[i].values` has length<br>`mesh.patches[i].size`. |

##### Implementations

###### Methods

- ```rust
  pub fn new</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, mesh: Arc<FvMesh>, internal: Field<T>, boundary: Vec<PatchField<T>>) -> Self { /* ... */ }
  ```
  Assemble a surface field from its internal-face values and per-patch

- ```rust
  pub fn zeros</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, mesh: Arc<FvMesh>) -> Self { /* ... */ }
  ```
  Scalar surface field with all internal-face values zero and zero-gradient

- ```rust
  pub fn uniform</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, mesh: Arc<FvMesh>, value: f64) -> Self { /* ... */ }
  ```
  Scalar surface field with all internal-face values set to `value` and

- ```rust
  pub fn zero</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, mesh: Arc<FvMesh>) -> Self { /* ... */ }
  ```
  Vector surface field with all internal-face values zero and zero-gradient

- ```rust
  pub fn face_value(self: &Self, f: usize) -> T { /* ... */ }
  ```
  Value at any face: internal face → from `internal`; boundary face →

###### Trait Implementations

- **Add**
  - ```rust
    fn add(self: Self, rhs: Self) -> Self { /* ... */ }
    ```

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> SurfaceField<T> { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Div**
  - ```rust
    fn div(self: Self, s: f64) -> Self { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Mul**
  - ```rust
    fn mul(self: Self, s: f64) -> Self { /* ... */ }
    ```

  - ```rust
    fn mul(self: Self, rhs: SurfaceField<T>) -> SurfaceField<T> { /* ... */ }
    ```

- **Neg**
  - ```rust
    fn neg(self: Self) -> Self { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sub**
  - ```rust
    fn sub(self: Self, rhs: Self) -> Self { /* ... */ }
    ```

- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Type Alias `SurfaceScalarField`

Scalar surface field: one `f64` per face (e.g. face flux `phi` `[m³/s]`).

```rust
pub type SurfaceScalarField = SurfaceField<f64>;
```

#### Type Alias `SurfaceVectorField`

Vector surface field: one `Vector3` per face.

```rust
pub type SurfaceVectorField = SurfaceField<crate::primitives::Vector3>;
```

## Module `vol_field`

```rust
pub mod vol_field { /* ... */ }
```

### Types

#### Struct `VolField`

A volume field: one value per cell in the internal field, plus one
`PatchField` per boundary patch.

Mirrors `Foam::volScalarField` / `Foam::VolumeField<Type>`.
The internal field has length `mesh.n_cells`.

```rust
pub struct VolField<T: Clone> {
    pub name: String,
    pub mesh: std::sync::Arc<crate::mesh::fv_mesh::FvMesh>,
    pub internal: super::field::Field<T>,
    pub boundary: Vec<super::boundary::bc::PatchField<T>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Field name (diagnostic label, e.g. `"p"`, `"U"`, `"T"`). |
| `mesh` | `std::sync::Arc<crate::mesh::fv_mesh::FvMesh>` | Mesh this field is defined on. |
| `internal` | `super::field::Field<T>` | Cell-centred values; length == `mesh.n_cells`. |
| `boundary` | `Vec<super::boundary::bc::PatchField<T>>` | One entry per boundary patch; `boundary[i].values` has length<br>`mesh.patches[i].size`. |

##### Implementations

###### Methods

- ```rust
  pub fn new</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, mesh: Arc<FvMesh>, internal: Field<T>, boundary: Vec<PatchField<T>>) -> Self { /* ... */ }
  ```
  Assemble a volume field from its cell-centred values and per-patch

- ```rust
  pub fn uniform</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, mesh: Arc<FvMesh>, value: f64) -> Self { /* ... */ }
  ```
  Uniform scalar field over the entire domain.

- ```rust
  pub fn zeros</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, mesh: Arc<FvMesh>) -> Self { /* ... */ }
  ```
  Zero-valued scalar field over the entire domain (zero-gradient boundaries).

- ```rust
  pub fn uniform</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, mesh: Arc<FvMesh>, value: Vector3) -> Self { /* ... */ }
  ```
  Uniform vector field over the entire domain.

- ```rust
  pub fn zero</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, mesh: Arc<FvMesh>) -> Self { /* ... */ }
  ```
  Zero-valued vector field over the entire domain (zero-gradient boundaries).

###### Trait Implementations

- **Add**
  - ```rust
    fn add(self: Self, rhs: Self) -> <Self as >::Output { /* ... */ }
    ```

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> VolField<T> { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Div**
  - ```rust
    fn div(self: Self, s: f64) -> <Self as >::Output { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Mul**
  - ```rust
    fn mul(self: Self, s: f64) -> <Self as >::Output { /* ... */ }
    ```

  - ```rust
    fn mul(self: Self, rhs: VolVectorField) -> VolVectorField { /* ... */ }
    ```

  - ```rust
    fn mul(self: Self, rhs: VolField<T>) -> VolField<T> { /* ... */ }
    ```

- **Neg**
  - ```rust
    fn neg(self: Self) -> <Self as >::Output { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sub**
  - ```rust
    fn sub(self: Self, rhs: Self) -> <Self as >::Output { /* ... */ }
    ```

- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Type Alias `VolScalarField`

Scalar volume field: one `f64` per cell (e.g. pressure `[Pa]`, temperature `[K]`).

```rust
pub type VolScalarField = VolField<f64>;
```

#### Type Alias `VolVectorField`

Vector volume field: one `Vector3` per cell (e.g. velocity `[m/s]`).

```rust
pub type VolVectorField = VolField<crate::primitives::Vector3>;
```

#### Type Alias `VolTensorField`

General (rank-2) tensor volume field: one `Tensor` per cell.

```rust
pub type VolTensorField = VolField<crate::primitives::Tensor>;
```

#### Type Alias `VolSymmTensorField`

Symmetric (rank-2) tensor volume field: one `SymmTensor` per cell (e.g. stress).

```rust
pub type VolSymmTensorField = VolField<crate::primitives::SymmTensor>;
```

## Module `vol_field_algebra`

Field-level (`GeometricField`) tensor algebra.

Thin per-element wrappers that lift the primitive `Tensor` / `SymmTensor`
operations (`tr`, `symm`, `twoSymm`, `dev`, `dev2` — defined in
`crate::primitives`) to whole volume fields, applying the operation to the
internal field and every boundary patch and returning the correctly-ranked
output field.

These belong here (with the field types) rather than in the FV operator
layer because they are pure algebra — no mesh metrics, no interpolation.
They are the field-level counterparts a `solidDisplacementFoam`-style stress
update needs, e.g. `sigma = mu*twoSymm(grad(D)) + lambda*tr(grad(D))*I`.

Rank map (OpenFOAM convention):

- `tr`        : tensor / symmTensor → scalar
- `symm`      : tensor → symmTensor  (0.5·(T + Tᵀ))
- `two_symm`  : tensor / symmTensor → symmTensor  (T + Tᵀ)
- `dev`       : tensor / symmTensor → same rank   (T − (tr/3)·I)
- `dev2`      : tensor / symmTensor → same rank   (T − (2·tr/3)·I)

The output boundary patches are set zero-gradient (values carry the mapped
result); the operation is applied element-wise so no BC evaluation occurs.

```rust
pub mod vol_field_algebra { /* ... */ }
```

### Functions

#### Function `tr`

`tr(T)` of a tensor field → scalar field: `tr = T_xx + T_yy + T_zz`.

```rust
pub fn tr(vol: &crate::fields::vol_field::VolTensorField) -> crate::fields::vol_field::VolScalarField { /* ... */ }
```

#### Function `tr_of_symm`

`tr(S)` of a symmetric-tensor field → scalar field.

```rust
pub fn tr_of_symm(vol: &crate::fields::vol_field::VolSymmTensorField) -> crate::fields::vol_field::VolScalarField { /* ... */ }
```

#### Function `symm`

`symm(T) = 0.5·(T + Tᵀ)` of a tensor field → symmetric-tensor field.

```rust
pub fn symm(vol: &crate::fields::vol_field::VolTensorField) -> crate::fields::vol_field::VolSymmTensorField { /* ... */ }
```

#### Function `two_symm`

`twoSymm(T) = T + Tᵀ` of a tensor field → symmetric-tensor field.

```rust
pub fn two_symm(vol: &crate::fields::vol_field::VolTensorField) -> crate::fields::vol_field::VolSymmTensorField { /* ... */ }
```

#### Function `two_symm_of_symm`

`twoSymm(S) = 2·S` of a symmetric-tensor field → symmetric-tensor field.

```rust
pub fn two_symm_of_symm(vol: &crate::fields::vol_field::VolSymmTensorField) -> crate::fields::vol_field::VolSymmTensorField { /* ... */ }
```

#### Function `dev`

`dev(T) = T − (tr(T)/3)·I` of a tensor field → tensor field (trace-free).

```rust
pub fn dev(vol: &crate::fields::vol_field::VolTensorField) -> crate::fields::vol_field::VolTensorField { /* ... */ }
```

#### Function `dev2`

`dev2(T) = T − (2·tr(T)/3)·I` of a tensor field → tensor field.

```rust
pub fn dev2(vol: &crate::fields::vol_field::VolTensorField) -> crate::fields::vol_field::VolTensorField { /* ... */ }
```

#### Function `dev_of_symm`

`dev(S) = S − (tr(S)/3)·I` of a symmetric-tensor field → symmetric-tensor field.

```rust
pub fn dev_of_symm(vol: &crate::fields::vol_field::VolSymmTensorField) -> crate::fields::vol_field::VolSymmTensorField { /* ... */ }
```

#### Function `dev2_of_symm`

`dev2(S) = S − (2·tr(S)/3)·I` of a symmetric-tensor field → symmetric-tensor field.

```rust
pub fn dev2_of_symm(vol: &crate::fields::vol_field::VolSymmTensorField) -> crate::fields::vol_field::VolSymmTensorField { /* ... */ }
```

### Re-exports

#### Re-export `Field`

```rust
pub use field::Field;
```

#### Re-export `boundary::*`

```rust
pub use boundary::*;
```

#### Re-export `surface_field::*`

```rust
pub use surface_field::*;
```

#### Re-export `vol_field::*`

```rust
pub use vol_field::*;
```

## Module `fluid_thermo`

Layer 4 — field-level fluid and solid thermodynamics (`FluidThermo`,
`SolidThermo`, `PsiThermo`, `RhoThermo`).

```rust
pub mod fluid_thermo { /* ... */ }
```

### Modules

## Module `psi_thermo`

```rust
pub mod psi_thermo { /* ... */ }
```

### Types

#### Struct `PsiThermo`

Compressible thermo using ψ-based density: `ρ = ψ · p`.

This is the `psiThermo` closure used by **sonicFoam** and the transonic
branch of **rhoPimpleFoam**.  Storing ψ rather than recomputing it each
step lets the pressure equation access ψ directly without a thermo call.

`M` is any `TransportModel` (which supers `ThermoModel` and `EquationOfState`).

```rust
pub struct PsiThermo<M: TransportModel> {
    pub species: M,
    pub p: crate::fields::vol_field::VolScalarField,
    pub t: crate::fields::vol_field::VolScalarField,
    pub he: crate::fields::vol_field::VolScalarField,
    pub rho: crate::fields::vol_field::VolScalarField,
    pub psi: crate::fields::vol_field::VolScalarField,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `species` | `M` | Per-species transport/thermo/EOS kernel (mesh-independent). |
| `p` | `crate::fields::vol_field::VolScalarField` | Pressure field `[Pa]`. |
| `t` | `crate::fields::vol_field::VolScalarField` | Temperature field `[K]`. |
| `he` | `crate::fields::vol_field::VolScalarField` | Sensible enthalpy `hs` `[J/kg]`. |
| `rho` | `crate::fields::vol_field::VolScalarField` | Density field ρ `[kg/m³]`, stored as `ρ = ψ · p`. |
| `psi` | `crate::fields::vol_field::VolScalarField` | Compressibility field ψ = ρ/p `[s²/m²]`. |

##### Implementations

###### Methods

- ```rust
  pub fn new(species: M, mesh: Arc<FvMesh>, p_init: f64, t_init: f64) -> Self { /* ... */ }
  ```
  Construct a thermodynamically consistent initial state.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **FluidThermo**
  - ```rust
    fn mesh(self: &Self) -> &Arc<FvMesh> { /* ... */ }
    ```

  - ```rust
    fn p(self: &Self) -> &VolScalarField { /* ... */ }
    ```

  - ```rust
    fn p_mut(self: &mut Self) -> &mut VolScalarField { /* ... */ }
    ```

  - ```rust
    fn t(self: &Self) -> &VolScalarField { /* ... */ }
    ```

  - ```rust
    fn rho(self: &Self) -> &VolScalarField { /* ... */ }
    ```

  - ```rust
    fn he(self: &Self) -> &VolScalarField { /* ... */ }
    ```

  - ```rust
    fn he_mut(self: &mut Self) -> &mut VolScalarField { /* ... */ }
    ```

  - ```rust
    fn psi(self: &Self) -> &VolScalarField { /* ... */ }
    ```

  - ```rust
    fn mu(self: &Self) -> VolScalarField { /* ... */ }
    ```

  - ```rust
    fn kappa(self: &Self) -> VolScalarField { /* ... */ }
    ```

  - ```rust
    fn alpha_h(self: &Self) -> VolScalarField { /* ... */ }
    ```

  - ```rust
    fn correct(self: &mut Self) { /* ... */ }
    ```

  - ```rust
    fn correct_rho(self: &mut Self, delta_rho: &VolScalarField, rho_min: f64, rho_max: f64) { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `rho_thermo`

```rust
pub mod rho_thermo { /* ... */ }
```

### Types

#### Struct `RhoThermo`

Compressible thermo using explicit EOS density: `ρ = ρ(p, T)`.

This is the `rhoThermo` closure used by the subsonic branch of
**rhoPimpleFoam**.  Density is computed directly from the equation of
state, not from ψ·p, so it works for non-ideal gas models (e.g. real-gas
EOS or incompressible `RhoConst`).

`M` is any `TransportModel` (which supers `ThermoModel` and `EquationOfState`).

```rust
pub struct RhoThermo<M: TransportModel> {
    pub species: M,
    pub p: crate::fields::vol_field::VolScalarField,
    pub t: crate::fields::vol_field::VolScalarField,
    pub he: crate::fields::vol_field::VolScalarField,
    pub rho: crate::fields::vol_field::VolScalarField,
    pub psi: crate::fields::vol_field::VolScalarField,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `species` | `M` | Per-species transport/thermo/EOS kernel (mesh-independent). |
| `p` | `crate::fields::vol_field::VolScalarField` | Pressure field `[Pa]`. |
| `t` | `crate::fields::vol_field::VolScalarField` | Temperature field `[K]`. |
| `he` | `crate::fields::vol_field::VolScalarField` | Sensible enthalpy `hs` `[J/kg]`. |
| `rho` | `crate::fields::vol_field::VolScalarField` | Density field ρ `[kg/m³]`, computed directly from the EOS `ρ(p, T)`. |
| `psi` | `crate::fields::vol_field::VolScalarField` | Compressibility ψ = ∂ρ/∂p|_T `[s²/m²]` — stored for the pressure eqn. |

##### Implementations

###### Methods

- ```rust
  pub fn new(species: M, mesh: Arc<FvMesh>, p_init: f64, t_init: f64) -> Self { /* ... */ }
  ```
  Construct a thermodynamically consistent initial state.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **FluidThermo**
  - ```rust
    fn mesh(self: &Self) -> &Arc<FvMesh> { /* ... */ }
    ```

  - ```rust
    fn p(self: &Self) -> &VolScalarField { /* ... */ }
    ```

  - ```rust
    fn p_mut(self: &mut Self) -> &mut VolScalarField { /* ... */ }
    ```

  - ```rust
    fn t(self: &Self) -> &VolScalarField { /* ... */ }
    ```

  - ```rust
    fn rho(self: &Self) -> &VolScalarField { /* ... */ }
    ```

  - ```rust
    fn he(self: &Self) -> &VolScalarField { /* ... */ }
    ```

  - ```rust
    fn he_mut(self: &mut Self) -> &mut VolScalarField { /* ... */ }
    ```

  - ```rust
    fn psi(self: &Self) -> &VolScalarField { /* ... */ }
    ```

  - ```rust
    fn mu(self: &Self) -> VolScalarField { /* ... */ }
    ```

  - ```rust
    fn kappa(self: &Self) -> VolScalarField { /* ... */ }
    ```

  - ```rust
    fn alpha_h(self: &Self) -> VolScalarField { /* ... */ }
    ```

  - ```rust
    fn correct(self: &mut Self) { /* ... */ }
    ```

  - ```rust
    fn correct_rho(self: &mut Self, delta_rho: &VolScalarField, rho_min: f64, rho_max: f64) { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `solid_thermo`

```rust
pub mod solid_thermo { /* ... */ }
```

### Types

#### Struct `ConstSolidThermo`

Solid thermo with constant κ and ρ·Cp.

Corresponds to `Foam::constSolidThermo` — the standard first choice for
metals, ceramics, and PCB substrates where property variation with T is
small.

```rust
use outram_foam_basic_lib::prelude::*;
use outram_foam_basic_lib::fluid_thermo::{ConstSolidThermo, SolidThermo};
use std::sync::Arc;

let mesh = Arc::new(
    FvMeshBuilder::new()
        .n_cells(1).n_internal_faces(0)
        .owner(vec![0]).neighbour(vec![])
        .patches(vec![BoundaryPatch::new("wall", 0, 1, PatchKind::Wall)])
        .cell_volumes(vec![1.0])
        .cell_centres(vec![Vector3::ZERO])
        .face_area_vectors(vec![Vector3::new(1.0, 0.0, 0.0)])
        .face_centres(vec![Vector3::ZERO])
        .build().unwrap()
);
let solid = ConstSolidThermo::new(mesh, 300.0, 16.0, 3.96e6);
assert!((solid.kappa().internal[0] - 16.0).abs() < 1e-12);
```

```rust
pub struct ConstSolidThermo {
    pub t: crate::fields::vol_field::VolScalarField,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `t` | `crate::fields::vol_field::VolScalarField` | Temperature field `[K]`. |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: Arc<FvMesh>, t_init: f64, kappa: f64, rho_cp: f64) -> Self { /* ... */ }
  ```
  Create a uniform solid thermo.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ConstSolidThermo { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **SolidThermo**
  - ```rust
    fn mesh(self: &Self) -> &Arc<FvMesh> { /* ... */ }
    ```

  - ```rust
    fn t(self: &Self) -> &VolScalarField { /* ... */ }
    ```

  - ```rust
    fn t_mut(self: &mut Self) -> &mut VolScalarField { /* ... */ }
    ```

  - ```rust
    fn kappa(self: &Self) -> VolScalarField { /* ... */ }
    ```

  - ```rust
    fn rho_cp(self: &Self) -> VolScalarField { /* ... */ }
    ```

  - ```rust
    fn correct(self: &mut Self) { /* ... */ }
    ```

- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Traits

#### Trait `SolidThermo`

Field-level solid thermodynamic model.

Solid regions have no flow — the only governing equation is the heat
conduction equation:

```text
ρ·Cp·∂T/∂t = ∇·(κ∇T) + q̇
```

This trait provides the two coefficients the energy equation needs:
`kappa()` for the Laplacian and `rho_cp()` for the ddt term.

Mirrors the role of `Foam::solidThermo` from
`src/thermophysicalModels/solidThermo/`.

```rust
pub trait SolidThermo {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `mesh`: The finite-volume mesh this solid region is defined on.
- `t`: Temperature field `[K]`.
- `t_mut`: Mutable temperature field `[K]` — for the energy equation to update in place.
- `kappa`: Thermal conductivity κ `[W/(m·K)]` — used in `fvm::laplacian(kappa, T)`.
- `rho_cp`: Volumetric heat capacity ρ·Cp `[J/(m³·K)]` — used in `fvm::ddt(rho_cp, T)`.
- `correct`: Recompute temperature-dependent properties after T has been updated.

##### Implementations

This trait is implemented for the following types:

- `ConstSolidThermo`

## Module `traits`

Field-level fluid thermodynamic interface (Layer 4).

Mirrors `Foam::fluidThermo` / `Foam::psiThermo` / `Foam::rhoThermo` from
`src/thermophysicalModels/basic/`.

Each struct owns the primary thermodynamic fields (`p`, `T`, `he`, `rho`,
`psi`) and uses a per-species `TransportModel` (from Layer 1h) to evaluate
properties cell-by-cell.

```rust
pub mod traits { /* ... */ }
```

### Traits

#### Trait `FluidThermo`

Field-level fluid thermodynamic model.

Mirrors the `Foam::fluidThermo` / `Foam::psiThermo` / `Foam::rhoThermo`
abstract interface from `src/thermophysicalModels/basic/`.

Owns the primary thermodynamic fields (`p`, `T`, `he`, `rho`, `psi`) and
provides `correct()` to recompute derived quantities after `he` or `p`
have been updated by the solver.

Computed transport fields (`mu`, `kappa`, `alpha_h`) are returned by value
rather than stored, to keep the struct lean and avoid stale-field bugs.

```rust
pub trait FluidThermo {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `mesh`: The finite-volume mesh these thermodynamic fields are defined on.
- `p`: Pressure field `[Pa]`.
- `p_mut`: Mutable pressure field `[Pa]` — for the pressure equation to update in place.
- `t`: Temperature field `[K]`.
- `rho`: Density field `[kg/m³]`.
- `he`: Energy field — sensible enthalpy `hs` `[J/kg]` by default.
- `he_mut`: Mutable energy field `he` `[J/kg]` — for the energy equation to update in place.
- `psi`: Compressibility field ψ = ∂ρ/∂p|_T `[s²/m²]`.
- `mu`: Dynamic viscosity field μ `[Pa·s]` — computed on demand.
- `kappa`: Thermal conductivity field κ `[W/(m·K)]` — computed on demand.
- `alpha_h`: Thermal diffusivity αh = κ/Cp `[kg/(m·s)]` — computed on demand.
- `correct`: Recompute `T`, `ρ`, and `ψ` from `he` + `p`.
- `correct_rho`: Clamp density after the pressure equation:

##### Implementations

This trait is implemented for the following types:

- `PsiThermo<M>` with <M: TransportModel>
- `RhoThermo<M>` with <M: TransportModel>

### Re-exports

#### Re-export `PsiThermo`

```rust
pub use psi_thermo::PsiThermo;
```

#### Re-export `RhoThermo`

```rust
pub use rho_thermo::RhoThermo;
```

#### Re-export `ConstSolidThermo`

```rust
pub use solid_thermo::ConstSolidThermo;
```

#### Re-export `SolidThermo`

```rust
pub use solid_thermo::SolidThermo;
```

#### Re-export `FluidThermo`

```rust
pub use traits::FluidThermo;
```

## Module `fv_operators`

Layer 3 — finite-volume discretisation operators (`fvc` explicit, `fvm`
implicit, and `adjust_phi` continuity correction).

```rust
pub mod fv_operators { /* ... */ }
```

### Modules

## Module `fvc`

Explicit finite-volume operators — return a new field.

Usage mirrors `Foam::fvc::` from `src/finiteVolume/finiteVolume/fvc/`.
Explicit (`fvc`) finite-volume operators — each returns a **new field**
(a `VolField` / `SurfaceField`), never a matrix.

Mirrors `Foam::fvc::` (`src/finiteVolume/finiteVolume/fvc/`). Contents:
Gauss gradient (`grad`, `grad_vec`), Gauss divergence (`div`, `div_flux`,
`div_vec`, `div_tensor`, `div_symm_tensor`), surface-normal gradient
the Gauss cell gradient (`grad`, `grad_vec`) and the mesh-independent
least-squares cell gradient (`grad_least_squares` — exact for a linear field
on a non-orthogonal mesh, where the Gauss gradient is not), the
surface-normal gradient
(`sn_grad`), linear face interpolation (`interpolate`) and flux assembly
(`flux`, `buoyancy_flux`), least-squares velocity reconstruction
(`reconstruct`), the Rhie–Chow time-derivative flux correction
(`ddt_corr`), and MUSCL / TVD limited face reconstruction
(`reconstruct_pos_neg`, `Limiter`). Field values carry raw
`f64` / `Vector3` / `Tensor` element data (no `uom`), consistent with the
rest of the FV operator layer.

```rust
pub mod fvc { /* ... */ }
```

### Re-exports

#### Re-export `ddt_corr`

```rust
pub use ddt_corr::ddt_corr;
```

#### Re-export `div`

```rust
pub use div::div;
```

#### Re-export `div_flux`

```rust
pub use div::div_flux;
```

#### Re-export `div_vec`

```rust
pub use div::div_vec;
```

#### Re-export `div_symm_tensor`

```rust
pub use div_tensor::div_symm_tensor;
```

#### Re-export `div_tensor`

```rust
pub use div_tensor::div_tensor;
```

#### Re-export `buoyancy_flux`

```rust
pub use flux::buoyancy_flux;
```

#### Re-export `flux`

```rust
pub use flux::flux;
```

#### Re-export `grad`

```rust
pub use grad::grad;
```

#### Re-export `grad_least_squares`

```rust
pub use grad_least_squares::grad_least_squares;
```

#### Re-export `grad_vec`

```rust
pub use grad_vec::grad_vec;
```

#### Re-export `interpolate`

```rust
pub use interpolate::interpolate;
```

#### Re-export `reconstruct_pos_neg`

```rust
pub use muscl::reconstruct_pos_neg;
```

#### Re-export `Limiter`

```rust
pub use muscl::Limiter;
```

#### Re-export `reconstruct`

```rust
pub use reconstruct::reconstruct;
```

#### Re-export `sn_grad`

```rust
pub use sn_grad::sn_grad;
```

## Module `fvm`

Implicit finite-volume operators — assemble into a sparse `FvMatrix`.

Usage mirrors `Foam::fvm::` from `src/finiteVolume/finiteVolume/fvm/`.
Implicit (`fvm`) finite-volume operators — each **assembles into a sparse
matrix** (`FvMatrix` for scalar unknowns, `FvVectorMatrix` for vector
unknowns) whose solve advances the field, rather than returning an explicit
field.

Mirrors `Foam::fvm::` (`src/finiteVolume/finiteVolume/fvm/`). Contents:
implicit Euler time derivatives (`ddt`, `ddt_coeff`, `ddt_vec`,
`ddt_coeff_vec`) and the second time derivative (`d2dt2`, `d2dt2_coeff`),
first-order upwind convection (`div`, `div_vec`), the Gauss-orthogonal
Laplacian (`laplacian`, `laplacian_vec`), its **non-orthogonality-corrected**
counterpart (`laplacian_corrected`, `solve_laplacian_non_orthogonal`,
selected by the `NonOrthoScheme` enum — the orthogonal form is silently
first-order-wrong on any non-hex mesh), and implicit / explicit source
terms (`sp`, `su`, `su_sp` and their `_vec` forms). See each function's doc
and the `sup` module header for the LHS / RHS sign conventions that apply
when combining these matrices.

```rust
pub mod fvm { /* ... */ }
```

### Re-exports

#### Re-export `d2dt2`

```rust
pub use d2dt2::d2dt2;
```

#### Re-export `d2dt2_coeff`

```rust
pub use d2dt2::d2dt2_coeff;
```

#### Re-export `ddt`

```rust
pub use ddt::ddt;
```

#### Re-export `ddt_coeff`

```rust
pub use ddt::ddt_coeff;
```

#### Re-export `ddt_coeff_vec`

```rust
pub use ddt_vec::ddt_coeff_vec;
```

#### Re-export `ddt_vec`

```rust
pub use ddt_vec::ddt_vec;
```

#### Re-export `div`

```rust
pub use div::div;
```

#### Re-export `div_vec`

```rust
pub use div_vec::div_vec;
```

#### Re-export `laplacian`

```rust
pub use laplacian::laplacian;
```

#### Re-export `laplacian_corrected`

```rust
pub use laplacian_corrected::laplacian_corrected;
```

#### Re-export `max_non_orthogonality_deg`

```rust
pub use laplacian_corrected::max_non_orthogonality_deg;
```

#### Re-export `non_ortho_geometry`

```rust
pub use laplacian_corrected::non_ortho_geometry;
```

#### Re-export `solve_laplacian_non_orthogonal`

```rust
pub use laplacian_corrected::solve_laplacian_non_orthogonal;
```

#### Re-export `NonOrthoGeometry`

```rust
pub use laplacian_corrected::NonOrthoGeometry;
```

#### Re-export `NonOrthoScheme`

```rust
pub use laplacian_corrected::NonOrthoScheme;
```

#### Re-export `laplacian_vec`

```rust
pub use laplacian_vec::laplacian_vec;
```

#### Re-export `sp`

```rust
pub use sup::sp;
```

#### Re-export `sp_vec`

```rust
pub use sup::sp_vec;
```

#### Re-export `su`

```rust
pub use sup::su;
```

#### Re-export `su_sp`

```rust
pub use sup::su_sp;
```

#### Re-export `su_sp_vec`

```rust
pub use sup::su_sp_vec;
```

#### Re-export `su_vec`

```rust
pub use sup::su_vec;
```

### Re-exports

#### Re-export `adjust_phi`

```rust
pub use adjust_phi::adjust_phi;
```

## Module `fv_options`

Layer 3 — optional source terms attached to finite-volume equations,
OpenFOAM's `fvOptions` (Foundation `fvModels`) mechanism.
Optional source terms added to finite-volume equations — OpenFOAM's
`fvOptions` mechanism.

# What this is for

A solver assembles a fixed equation — momentum, energy, a transported
scalar. Real cases then need *extra* terms in it that the solver itself
knows nothing about: a heat source in one region, a porous drag in another,
a phase-change latent heat, a momentum sink representing a fan. Editing the
solver for each is unworkable, so OpenFOAM lets a case attach source terms
to named equations from the outside. That is `fvOptions`.

The pattern is the same everywhere it appears:

```text
solve( ddt(rho, U) + div(phi, U) - laplacian(mu, U) == fvModels.source(rho, U) );
```

The solver names the equation; the case decides what, if anything, is added
to it.

# A note on the name

What ESI OpenFOAM (openfoam.com) calls **`fvOptions`**, the OpenFOAM
Foundation (openfoam.org) split into **`fvModels`** — terms that add
*sources* to an equation — and **`fvConstraints`** — terms that *constrain*
a solution, such as fixing a value in a cell set. This port follows the
Foundation split, because the vendored reference tree is the Foundation
one, but the module is named `fv_options` because that is the name most
users will search for. [`FvModel`](crate::fv_options::FvModel) is the source half; constraints are not
yet ported.

# Why this lives in `outram-foam-basic-lib`

It operates directly on [`FvMatrix`](crate::ldu_matrix::FvMatrix), and every
solver crate needs it — the multiphase, turbulence and application layers
all assemble equations that a case may want to add to. Putting it in a
solver crate would make it unavailable to the others. This mirrors
OpenFOAM's own dependency position, where `fvOptions`/`fvModels` sits
directly on `finiteVolume`.

# Sign convention, and the trap in it

Sources are added to the **right-hand side**, i.e. the equation reads
`ddt(...) + div(...) == source`. A positive scalar source therefore
*increases* the solved quantity.

Internally an `FvMatrix` stores the system as `A·φ = b`, so a right-hand
side contribution goes into `source`, while an implicit contribution
proportional to `φ` goes onto the **diagonal with the opposite sign**. That
asymmetry is the classic way to get a source term backwards, so
[`FvModel`](crate::fv_options::FvModel) never asks a caller to place terms by hand:
[`add_source_scalar`](crate::fv_options::FvModels::add_source_scalar) and
[`add_source_vector`](crate::fv_options::FvModels::add_source_vector) do the
placement, and the
individual models express themselves as an explicit part and an implicit
coefficient.

# Cell selection

Every model applies over a
[`CellSelection`](crate::fv_options::CellSelection) — the whole mesh, or a named
subset. This is upstream's `cellSetOption`/`fvCellZone`. Selections hold
their cell list behind an `Arc`, per the workspace rule against lifetime
parameters, so sharing one selection between several models is free.

```rust
pub mod fv_options { /* ... */ }
```

### Types

#### Enum `FvModel`

One optional source term.

Enum dispatch rather than trait objects, per the workspace rule: the set of
source models is closed and known at compile time, adding one forces every
`match` to be revisited, and rust-analyzer can navigate to each variant —
none of which is true of upstream's runtime-selection table.

# Which equations a model contributes to

Not every model contributes to every equation.
[`contributes_to`](FvModel::contributes_to) reports what a model acts on,
mirroring upstream's `addSupFields`, so applying a whole collection to an
equation only invokes the models that have something to say about it.

```rust
pub enum FvModel {
    SemiImplicit(SemiImplicitSource),
    SolidificationMelting(SolidificationMelting),
    SolidificationPorosity(SolidificationPorosity),
    VofSolidificationMelting(VofSolidificationMelting),
}
```

##### Variants

###### `SemiImplicit`

A general explicit/implicit source, upstream `semiImplicitSource`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `SemiImplicitSource` |  |

###### `SolidificationMelting`

Solidification and melting by the enthalpy-porosity method, upstream
`solidificationMelting`. The physically complete phase-change model:
tracks a liquid fraction, absorbs latent heat, carries its own
Boussinesq buoyancy.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `SolidificationMelting` |  |

###### `SolidificationPorosity`

Solidification as a bare porous blockage, upstream
`porosityModels::solidification`. **No latent heat and no buoyancy** —
it only freezes the momentum out of cold cells.

Strictly, upstream files this under `porosityModel` rather than
`fvModel`; it is folded into this enum because from a solver's point of
view it is the same thing — a term added to the momentum equation over
a cell zone — and keeping it in a parallel mechanism would double the
wiring for no gain.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `SolidificationPorosity` |  |

###### `VofSolidificationMelting`

Solidification and melting of a VoF phase, upstream
`VoFSolidificationMelting`. Needs a VoF phase fraction supplied from
outside; see [`FvModels::correct`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `VofSolidificationMelting` |  |

##### Implementations

###### Methods

- ```rust
  pub fn name(self: &Self) -> &str { /* ... */ }
  ```
  The model's name, for diagnostics.

- ```rust
  pub fn contributes_to(self: &Self, field: &str) -> bool { /* ... */ }
  ```
  Whether this model contributes to the equation for `field`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> FvModel { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `EquationField`

Which equation a source term is being applied to.

Models are attached to equations *by the name of the solved field*, exactly
as upstream does. This is stringly-typed for the same reason upstream is:
the solver that assembles an equation and the case that adds a source to it
do not share a type, and the field name is the only stable identifier they
both know.

```rust
pub struct EquationField<''n>(pub &'n str);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `&'n str` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> EquationField<''n> { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &EquationField<''n>) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `FvModels`

A collection of source terms, applied together to an equation.

Upstream's `fvModels`. Held by a solver and handed each equation in turn.

```rust
pub struct FvModels {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```
  An empty collection — a solver with no optional sources.

- ```rust
  pub fn push(self: &mut Self, model: FvModel) { /* ... */ }
  ```
  Attach a model.

- ```rust
  pub fn models(self: &Self) -> &[FvModel] { /* ... */ }
  ```
  The attached models.

- ```rust
  pub fn models_mut(self: &mut Self) -> &mut [FvModel] { /* ... */ }
  ```
  Mutable access, needed because stateful models advance their internal

- ```rust
  pub fn contributes_to(self: &Self, field: &str) -> bool { /* ... */ }
  ```
  Whether any attached model contributes to `field`.

- ```rust
  pub fn correct(self: &mut Self, temperature: &VolScalarField, vof_phase_fraction: Option<&VolScalarField>) { /* ... */ }
  ```
  Advance every stateful model, once per timestep — upstream's

- ```rust
  pub fn advance_time(self: &mut Self) { /* ... */ }
  ```
  Roll every stateful model's history forward and re-arm its

- ```rust
  pub fn add_source_scalar(self: &mut Self, field: &str, rho: &VolScalarField, temperature: &VolScalarField, dt: f64, eqn: &mut FvMatrix) { /* ... */ }
  ```
  Add every applicable model's contribution to a scalar equation.

- ```rust
  pub fn add_source_vector(self: &mut Self, field: &str, rho: &VolScalarField, temperature: &VolScalarField, velocity: &VolVectorField, phase_fraction: Option<&VolScalarField>, dt: f64, eqn: &mut FvVectorMatrix) { /* ... */ }
  ```
  Add every applicable model's contribution to a vector equation.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> FvModels { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> FvModels { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SourceContribution`

The explicit and implicit halves of a source term, per cell.

Upstream splits a source into `Su` (explicit, independent of the solution)
and `Sp` (implicit, the coefficient of `φ`). Keeping them apart is not
bookkeeping: putting a stabilising negative coefficient on the diagonal
rather than in the right-hand side is what keeps the matrix diagonally
dominant and the solve stable, and it is why a Darcy drag is written
implicitly.

# Units

`explicit` is in the units of the equation's residual per unit volume;
`implicit` in those units divided by the solved variable. Both are
**per unit volume** — multiplication by the cell volume happens when the
contribution is placed into the matrix, matching upstream, which writes
`Sp[celli] += Vc*S`.

```rust
pub struct SourceContribution {
    pub explicit: f64,
    pub implicit: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `explicit` | `f64` | The part independent of the solved variable, per unit volume. |
| `implicit` | `f64` | The coefficient of the solved variable, per unit volume.<br><br>**Negative values stabilise.** A sink proportional to the solution has a<br>negative coefficient here, and it lands on the matrix diagonal with the<br>sign flipped, increasing diagonal dominance. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> SourceContribution { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> SourceContribution { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SourceContribution) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Re-exports

#### Re-export `CellSelection`

```rust
pub use selection::CellSelection;
```

#### Re-export `SemiImplicitSource`

```rust
pub use semi_implicit::SemiImplicitSource;
```

#### Re-export `SolidificationMelting`

```rust
pub use solidification_melting::SolidificationMelting;
```

#### Re-export `SolidificationMeltingCoefficients`

```rust
pub use solidification_melting::SolidificationMeltingCoefficients;
```

#### Re-export `MomentumEquationForm`

```rust
pub use solidification_porosity::MomentumEquationForm;
```

#### Re-export `SolidificationPorosity`

```rust
pub use solidification_porosity::SolidificationPorosity;
```

#### Re-export `TemperatureTable`

```rust
pub use temperature_table::TemperatureTable;
```

#### Re-export `VofSolidificationMelting`

```rust
pub use vof_solidification_melting::VofSolidificationMelting;
```

## Module `interpolation`

Layer 1f — one-dimensional data interpolation (linear and spline).
One-dimensional table interpolation over sorted `(xs, ys)` samples.

Ports the OpenFOAM `interpolateXY` / `interpolateSplineXY` helpers:
[`interpolate_xy`](crate::interpolation::interpolate_xy::interpolate_xy) (piecewise-linear) and
[`interpolate_spline_xy`](crate::interpolation::interpolate_spline_xy::interpolate_spline_xy)
(Catmull-Rom cubic). Both assume `xs` is sorted ascending and clamp to the
endpoint value outside the table range. Abscissae and ordinates are bare
`f64` in the caller's own units.

```rust
pub mod interpolation { /* ... */ }
```

### Modules

## Module `interpolate_spline_xy`

```rust
pub mod interpolate_spline_xy { /* ... */ }
```

### Functions

#### Function `interpolate_spline_xy`

Catmull-Rom cubic spline interpolation over a sorted table `(xs, ys)`.

At the boundary knots the missing neighbours are mirrored (ghost-point
extension), matching OpenFOAM's `Foam::interpolateSplineXY`.
Clamps to endpoint values outside the table range.
Assumes `xs` is sorted in ascending order.

```rust
pub fn interpolate_spline_xy(x: f64, xs: &[f64], ys: &[f64]) -> f64 { /* ... */ }
```

## Module `interpolate_xy`

```rust
pub mod interpolate_xy { /* ... */ }
```

### Functions

#### Function `interpolate_xy`

Linear 1-D interpolation over a sorted table `(xs, ys)`.

Clamps to the endpoint values outside the table range.
Assumes `xs` is sorted in ascending order.
Maps to `Foam::interpolateXY(scalar, UList<scalar>&, UList<Type>&)`.

```rust
pub fn interpolate_xy(x: f64, xs: &[f64], ys: &[f64]) -> f64 { /* ... */ }
```

### Re-exports

#### Re-export `interpolate_spline_xy`

```rust
pub use interpolate_spline_xy::interpolate_spline_xy;
```

#### Re-export `interpolate_xy`

```rust
pub use interpolate_xy::interpolate_xy;
```

## Module `io`

OpenFOAM ASCII case I/O — `FoamFile` dictionaries, `polyMesh` read/write,
time-directory field read/write, and whole-case reading.
OpenFOAM ASCII **case I/O** — read and write OpenFOAM cases the way the
upstream utilities do.

This module is the foundation the OUTRAM PARK CLI reads/writes OpenFOAM
cases with. The format and algorithms are OpenFOAM-derived (the `FoamFile`
dictionary grammar, the `polyMesh` list layout, the time-directory field
layout); this is an independent Rust re-implementation of the ASCII reader
and writer, not the official OpenFOAM software.

## What lives here

- [`dict`](crate::io::dict) — the `FoamFile` **dictionary** format: a tokeniser (strips
  `//` and `/* */` comments; treats `( ) { } ; [ ]` as delimiters), an
  in-memory AST ([`FoamDict`](crate::io::dict::FoamDict),
  [`FoamEntry`](crate::io::dict::FoamEntry),
  [`FoamValue`](crate::io::dict::FoamValue),
  [`Dimensioned`](crate::io::dict::Dimensioned)), the
  [`FoamHeader`](crate::io::dict::FoamHeader) block, and an exact-round-trip
  writer. Handles `system/controlDict`, `fvSchemes`, `fvSolution`-style
  dictionaries.
- [`poly_mesh`](crate::io::poly_mesh) — [`PolyMesh`](crate::io::poly_mesh::PolyMesh): read/write `constant/polyMesh/{points,
  faces, owner, neighbour, boundary}` and convert to the crate's
  geometry-carrying [`crate::mesh::FvMesh`] via
  [`PolyMesh::to_fv_mesh`](crate::io::poly_mesh::PolyMesh::to_fv_mesh).
- [`field`](crate::io::field) — read/write a time-directory field file
  (`0/p` volScalarField, `0/U` volVectorField): the `dimensions`,
  `internalField`, and `boundaryField` blocks ↔ the crate's
  [`crate::fields::VolScalarField`] / [`crate::fields::VolVectorField`].
- [`case`](crate::io::case) — [`FoamCase`](crate::io::case::FoamCase): read a whole case directory (`system/…`,
  `constant/polyMesh`, `0/…`) into memory, with a best-effort writer.

## Round-trip guarantee

The writer and parser are inverse at the **AST level**: constructing a
[`FoamDict`](crate::io::dict::FoamDict) /
[`PolyMesh`](crate::io::poly_mesh::PolyMesh) / field, serialising it, and parsing it back
reproduces an equal in-memory value. (It is not a byte-for-byte re-emitter
of an arbitrary pre-existing file — comment banners and incidental
whitespace are normalised — but every value round-trips.)

```rust
pub mod io { /* ... */ }
```

### Modules

## Module `case`

Whole-**case** reader (`system/`, `constant/polyMesh`, a time directory).

[`FoamCase::read`] loads an OpenFOAM case directory into memory:

- `system/controlDict`, `system/fvSchemes`, `system/fvSolution` (and any
  other `system/` dictionaries) as [`FoamFile`] dictionaries;
- `constant/polyMesh` as a [`PolyMesh`] (plus a derived [`FvMesh`]);
- the fields in a time directory (default `0/`) as [`CaseField`]s,
  dispatched by their `FoamFile` `class` (`volScalarField` /
  `volVectorField`).

Field classes other than scalar/vector volume fields are **skipped** and
their file names recorded in [`FoamCase::skipped_fields`] (honest partial
coverage — nothing is silently lost). [`FoamCase::write`] is a best-effort
counterpart that re-emits the system dictionaries, the mesh, and the
scalar/vector fields.

```rust
pub mod case { /* ... */ }
```

### Types

#### Enum `CaseField`

A field loaded from a time directory, tagged by its value type.

An enum (not a trait object) so callers get exhaustive `match` handling and
the field lives inline.

```rust
pub enum CaseField {
    Scalar(crate::fields::VolScalarField, super::field::Dimensions),
    Vector(crate::fields::VolVectorField, super::field::Dimensions),
}
```

##### Variants

###### `Scalar`

A `volScalarField` and its dimensions.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::fields::VolScalarField` |  |
| 1 | `super::field::Dimensions` |  |

###### `Vector`

A `volVectorField` and its dimensions.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::fields::VolVectorField` |  |
| 1 | `super::field::Dimensions` |  |

##### Implementations

###### Methods

- ```rust
  pub fn name(self: &Self) -> &str { /* ... */ }
  ```
  The field name (its `object`).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> CaseField { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `FoamCase`

An in-memory OpenFOAM case.

```rust
pub struct FoamCase {
    pub root: std::path::PathBuf,
    pub system: Vec<(String, super::dict::FoamFile)>,
    pub poly_mesh: Option<super::poly_mesh::PolyMesh>,
    pub mesh: Option<std::sync::Arc<crate::mesh::FvMesh>>,
    pub time_dir: String,
    pub fields: Vec<CaseField>,
    pub skipped_fields: Vec<(String, String)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `root` | `std::path::PathBuf` | The case root directory. |
| `system` | `Vec<(String, super::dict::FoamFile)>` | `system/` dictionaries, keyed by file name (`controlDict`, …). |
| `poly_mesh` | `Option<super::poly_mesh::PolyMesh>` | The connectivity-carrying mesh from `constant/polyMesh`, if present. |
| `mesh` | `Option<std::sync::Arc<crate::mesh::FvMesh>>` | The geometry-carrying FV mesh derived from `poly_mesh`, if present. |
| `time_dir` | `String` | The time directory that was read (e.g. `"0"`). |
| `fields` | `Vec<CaseField>` | Fields loaded from the time directory. |
| `skipped_fields` | `Vec<(String, String)>` | Field files skipped because their `class` is not a scalar/vector volume<br>field (`(file_name, class)`). |

##### Implementations

###### Methods

- ```rust
  pub fn read</* synthetic */ impl AsRef<Path>: AsRef<Path>>(root: impl AsRef<Path>) -> Result<Self, IoError> { /* ... */ }
  ```
  Read a case from `root`, using time directory `"0"`.

- ```rust
  pub fn read_time</* synthetic */ impl AsRef<Path>: AsRef<Path>>(root: impl AsRef<Path>, time_dir: &str) -> Result<Self, IoError> { /* ... */ }
  ```
  Read a case from `root`, using the given `time_dir` (e.g. `"0"`).

- ```rust
  pub fn system_dict(self: &Self, name: &str) -> Option<&FoamFile> { /* ... */ }
  ```
  Look up a `system/` dictionary by file name.

- ```rust
  pub fn write</* synthetic */ impl AsRef<Path>: AsRef<Path>>(self: &Self, root: impl AsRef<Path>) -> Result<(), IoError> { /* ... */ }
  ```
  Best-effort write of the case back to `root`: `system/` dictionaries,

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> FoamCase { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `dict`

The `FoamFile` ASCII **dictionary** format: tokeniser, AST, parser, writer.

An OpenFOAM dictionary file is a banner comment, an optional `FoamFile`
header block, and a body of `keyword value ;` / `keyword { subdict }`
entries. This module models that as:

- [`FoamHeader`] — the `FoamFile { … }` block (an ordered flat
  keyword→raw-value map; values are kept verbatim so `version 2.0`
  round-trips as `2.0`, not `2`).
- [`FoamDict`] — an **ordered** keyword→[`FoamEntry`] map (ordered so
  writes preserve the input order and round-trip).
- [`FoamEntry`] — the value bound to one keyword: a scalar, word, quoted
  string, bare token sequence, parenthesised list, dimensioned value, or a
  nested sub-dictionary.
- [`FoamValue`] — a leaf inside a list / token sequence.
- [`Dimensioned`] — a `[0 2 -2 0 0 0 0] value` dimensioned quantity.

## Grammar notes

The tokeniser records whether each token was **glued** to the previous one
(no intervening whitespace). This disambiguates a function-style word such
as `div(phi,U)` or `grad(U)` (parentheses glued to a word → part of the
word) from a genuine list `(0 0 1)` (parenthesis preceded by whitespace →
a [`FoamValue::List`]). It also lets `4(1 4 13 10)` count-prefixed lists
parse cleanly.

```rust
pub mod dict { /* ... */ }
```

### Types

#### Struct `Dimensioned`

A dimensioned quantity: seven SI dimension exponents plus zero or more
numeric components.

OpenFOAM writes these as `[mass length time temperature moles current
luminous]` optionally followed by a value, e.g. `dimensions [0 2 -2 0 0 0
0];` (no value) or `nu [0 2 -1 0 0 0 0] 1e-05;` (scalar value). The value
components are stored as raw [`FoamValue`]s to support scalar and vector
forms.

```rust
pub struct Dimensioned {
    pub dims: [f64; 7],
    pub value: Vec<FoamValue>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `dims` | `[f64; 7]` | The seven SI dimension exponents, OpenFOAM order:<br>`[kg, m, s, K, mol, A, cd]`. |
| `value` | `Vec<FoamValue>` | Trailing value component(s); empty for a bare `dimensions […]` entry,<br>one element for a dimensioned scalar, three for a dimensioned vector. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Dimensioned { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Dimensioned) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `FoamValue`

A leaf value inside a list or a bare token sequence.

```rust
pub enum FoamValue {
    Scalar(f64),
    Word(String),
    Str(String),
    List(Vec<FoamValue>),
}
```

##### Variants

###### `Scalar`

A numeric scalar.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `Word`

A bare identifier / keyword-like word (e.g. `ascii`, `Gauss`, `PCG`,
or a function-style `grad(U)`).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Str`

A `"…"` quoted string (stored without the surrounding quotes).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `List`

A parenthesised `( … )` list of values (may nest).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<FoamValue>` |  |

##### Implementations

###### Methods

- ```rust
  pub fn as_vector3(self: &Self) -> Option<crate::primitives::Vector3> { /* ... */ }
  ```
  Interpret a `List` of exactly three scalars as a [`Vector3`]; returns

- ```rust
  pub fn as_scalar(self: &Self) -> Option<f64> { /* ... */ }
  ```
  The scalar value, if this is a [`FoamValue::Scalar`].

- ```rust
  pub fn as_word(self: &Self) -> Option<&str> { /* ... */ }
  ```
  The word text, if this is a [`FoamValue::Word`].

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> FoamValue { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FoamValue) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `FoamEntry`

The value bound to one keyword in a [`FoamDict`].

```rust
pub enum FoamEntry {
    Scalar(f64),
    Word(String),
    Str(String),
    Tokens(Vec<FoamValue>),
    List(Vec<FoamValue>),
    Dimensioned(Dimensioned),
    SubDict(FoamDict),
}
```

##### Variants

###### `Scalar`

A single numeric scalar: `startTime 0;`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `Word`

A single word: `application icoFoam;` (or function-style `default
Gauss;`).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Str`

A single quoted string.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Tokens`

A bare, space-separated multi-token value that is **not** parenthesised:
`div(phi,U) Gauss linearUpwind grad(U);` → the keyword is `div(phi,U)`
and the entry is `Tokens([Gauss, linearUpwind, grad(U)])`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<FoamValue>` |  |

###### `List`

A single parenthesised list: `( … )`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<FoamValue>` |  |

###### `Dimensioned`

A dimensioned value: `[0 2 -2 0 0 0 0] …`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Dimensioned` |  |

###### `SubDict`

A nested sub-dictionary: `keyword { … }`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `FoamDict` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> FoamEntry { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FoamEntry) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `FoamDict`

An **ordered** keyword → [`FoamEntry`] map — the body of a dictionary or a
sub-dictionary.

Insertion order is preserved so that serialising then parsing round-trips
byte-order of the entries. Lookups are linear (dictionaries are small).

```rust
pub struct FoamDict {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```
  A new, empty dictionary.

- ```rust
  pub fn insert</* synthetic */ impl Into<String>: Into<String>>(self: &mut Self, keyword: impl Into<String>, entry: FoamEntry) -> &mut Self { /* ... */ }
  ```
  Append `keyword → entry`. If the keyword already exists, the new entry

- ```rust
  pub fn get(self: &Self, keyword: &str) -> Option<&FoamEntry> { /* ... */ }
  ```
  Borrow the entry bound to `keyword`, if present.

- ```rust
  pub fn get_dict(self: &Self, keyword: &str) -> Option<&FoamDict> { /* ... */ }
  ```
  Borrow the sub-dictionary bound to `keyword`, if the entry is one.

- ```rust
  pub fn get_scalar(self: &Self, keyword: &str) -> Option<f64> { /* ... */ }
  ```
  The scalar bound to `keyword`, if the entry is a scalar.

- ```rust
  pub fn get_word(self: &Self, keyword: &str) -> Option<&str> { /* ... */ }
  ```
  The word bound to `keyword`, if the entry is a word.

- ```rust
  pub fn iter(self: &Self) -> impl Iterator<Item = (&str, &FoamEntry)> { /* ... */ }
  ```
  Iterate over `(keyword, entry)` pairs in insertion order.

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Number of top-level entries.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  True if the dictionary has no entries.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> FoamDict { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> FoamDict { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FoamDict) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `FoamHeader`

The `FoamFile { … }` header block — an ordered flat keyword → raw-value map.

Values are stored verbatim (quotes preserved on the values that had them)
so that e.g. `version 2.0;` round-trips as `2.0` rather than being reparsed
to `2`, and `location "constant/polyMesh";` keeps its quotes.

```rust
pub struct FoamHeader {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```
  A new, empty header.

- ```rust
  pub fn standard(class: &str, object: &str) -> Self { /* ... */ }
  ```
  The standard header for a given `class` and `object`, with

- ```rust
  pub fn standard_with_location(class: &str, location: &str, object: &str) -> Self { /* ... */ }
  ```
  Like [`Self::standard`] but also records a `location "…"`.

- ```rust
  pub fn set(self: &mut Self, keyword: &str, value: &str) -> &mut Self { /* ... */ }
  ```
  Set `keyword → value` (replacing in place if present, else appending).

- ```rust
  pub fn get(self: &Self, keyword: &str) -> Option<&str> { /* ... */ }
  ```
  The raw value bound to `keyword` (quotes still present if it had them).

- ```rust
  pub fn class(self: &Self) -> Option<&str> { /* ... */ }
  ```
  The `class` field, if present.

- ```rust
  pub fn object(self: &Self) -> Option<&str> { /* ... */ }
  ```
  The `object` field, if present.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> FoamHeader { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> FoamHeader { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FoamHeader) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `FoamFile`

A parsed OpenFOAM dictionary file: its `FoamFile` header (if present) and
the body of entries.

```rust
pub struct FoamFile {
    pub header: Option<FoamHeader>,
    pub dict: FoamDict,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `header` | `Option<FoamHeader>` | The `FoamFile { … }` header, or `None` if the file had none. |
| `dict` | `FoamDict` | The dictionary body. |

##### Implementations

###### Methods

- ```rust
  pub fn parse(text: &str) -> Result<Self, IoError> { /* ... */ }
  ```
  Parse dictionary `text` into a header (if any) and body.

- ```rust
  pub fn parse_named</* synthetic */ impl Into<String>: Into<String>>(text: &str, context: impl Into<String>) -> Result<Self, IoError> { /* ... */ }
  ```
  Like [`Self::parse`] but labels parse errors with `context`.

- ```rust
  pub fn read</* synthetic */ impl AsRef<Path>: AsRef<Path>>(path: impl AsRef<Path>) -> Result<Self, IoError> { /* ... */ }
  ```
  Read and parse a dictionary file from `path`.

- ```rust
  pub fn to_foam_string(self: &Self) -> String { /* ... */ }
  ```
  Serialise to OpenFOAM ASCII text (banner + header + body).

- ```rust
  pub fn write</* synthetic */ impl AsRef<Path>: AsRef<Path>>(self: &Self, path: impl AsRef<Path>) -> Result<(), IoError> { /* ... */ }
  ```
  Write to `path` as OpenFOAM ASCII text.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> FoamFile { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FoamFile) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `field`

Time-directory **field** file read/write (`0/p`, `0/U`, …).

A field file carries a `dimensions [7]` block, an `internalField`
(`uniform <v>` or `nonuniform List<...>`), and a `boundaryField` sub-dict
of per-patch `{ type …; value …; }` entries. This module maps those to and
from the crate's [`VolScalarField`] / [`VolVectorField`].

Because [`VolField`] itself carries no `dimensions`, the read functions
**return** the parsed dimension exponents alongside the field and the write
functions **take** them as an argument.

## Supported coverage

- `internalField`: both `uniform` and `nonuniform List<scalar|vector>`.
- `boundaryField` types: `fixedValue` (uniform or nonuniform `value`),
  `zeroGradient`, `empty`, `symmetry` / `symmetryPlane`, and `calculated`.

Value-carrying and flow-context types (`fixedGradient`, `mixed`,
`inletOutlet`, `outletInlet`, `freestream`, `pressureInletOutletVelocity`,
`fixedFluxPressure`, `totalPressure`, `flowRateInletVelocity`) are **written**
by the write functions but not yet **read** — the parser captures only the
single `value` sub-entry, not `gradient`/`refValue`/`freestreamValue`/`p0`/
`volumetricFlowRate`, so reading one raises [`IoError::Unsupported`]. These
reads are **deferred**, not silently dropped.

[`VolField`]: crate::fields::VolField

```rust
pub mod field { /* ... */ }
```

### Types

#### Type Alias `Dimensions`

Seven SI dimension exponents in OpenFOAM order `[kg, m, s, K, mol, A, cd]`.

```rust
pub type Dimensions = [f64; 7];
```

### Functions

#### Function `read_vol_scalar_field`

Read a `volScalarField` file, returning the field and its dimensions.

The `mesh` supplies the cell count and the boundary-patch order/sizes the
field is defined on; the file's `boundaryField` is matched to it by patch
name.

```rust
pub fn read_vol_scalar_field</* synthetic */ impl AsRef<Path>: AsRef<std::path::Path>>(path: impl AsRef<std::path::Path>, mesh: std::sync::Arc<crate::mesh::FvMesh>) -> Result<(crate::fields::VolScalarField, Dimensions), super::IoError> { /* ... */ }
```

#### Function `read_vol_vector_field`

Read a `volVectorField` file, returning the field and its dimensions.

```rust
pub fn read_vol_vector_field</* synthetic */ impl AsRef<Path>: AsRef<std::path::Path>>(path: impl AsRef<std::path::Path>, mesh: std::sync::Arc<crate::mesh::FvMesh>) -> Result<(crate::fields::VolVectorField, Dimensions), super::IoError> { /* ... */ }
```

#### Function `write_vol_scalar_field`

Write a `volScalarField` file to `path` with the given `dimensions`.

```rust
pub fn write_vol_scalar_field</* synthetic */ impl AsRef<Path>: AsRef<std::path::Path>>(path: impl AsRef<std::path::Path>, field: &crate::fields::VolScalarField, dimensions: Dimensions) -> Result<(), super::IoError> { /* ... */ }
```

#### Function `write_vol_vector_field`

Write a `volVectorField` file to `path` with the given `dimensions`.

```rust
pub fn write_vol_vector_field</* synthetic */ impl AsRef<Path>: AsRef<std::path::Path>>(path: impl AsRef<std::path::Path>, field: &crate::fields::VolVectorField, dimensions: Dimensions) -> Result<(), super::IoError> { /* ... */ }
```

## Module `poly_mesh`

`constant/polyMesh` read/write.

The crate's [`FvMesh`] stores only flat finite-volume geometry
(cell/face centres, areas, owner/neighbour) with no point/face-vertex
connectivity. OpenFOAM's on-disk `polyMesh`, by contrast, is defined by its
**connectivity**: `points` (vertices), `faces` (vertex loops),
`owner`/`neighbour` (cell adjacency), and `boundary` (patches). This module
therefore defines a connectivity-carrying [`PolyMesh`] as the I/O
representation and computes full FV geometry from it via
[`PolyMesh::to_fv_mesh`] — the same divergence-theorem pyramid
decomposition OpenFOAM's `primitiveMesh` uses (mirrored from
`outram-foam-mesh`'s `block_mesh` / `poly_dual_mesh`).

## Files

| file | class | contents |
|---|---|---|
| `points`    | `vectorField`      | vertex coordinates `[m]` |
| `faces`     | `faceList`         | each face as a vertex-index loop |
| `owner`     | `labelList`        | owner cell per face |
| `neighbour` | `labelList`        | neighbour cell per internal face |
| `boundary`  | `polyBoundaryMesh` | patches (`type`, `nFaces`, `startFace`) |

Faces are ordered OpenFOAM-style: internal faces first
(`[0, n_internal_faces)`), then boundary faces grouped by patch.

```rust
pub mod poly_mesh { /* ... */ }
```

### Types

#### Struct `MeshFace`

A single mesh face: its point-index loop plus owner / neighbour cells.

`verts` is wound so the face normal points **from `owner` toward
`neighbour`** (outward from the owner cell). Boundary faces have
`neighbour == None`.

```rust
pub struct MeshFace {
    pub verts: Vec<usize>,
    pub owner: usize,
    pub neighbour: Option<usize>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `verts` | `Vec<usize>` | Ordered point indices (into [`PolyMesh::points`]) forming the face loop. |
| `owner` | `usize` | Owning cell index. |
| `neighbour` | `Option<usize>` | Neighbour cell index (internal faces only). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> MeshFace { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &MeshFace) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `PolyMesh`

A connectivity-carrying poly-mesh — the on-disk `polyMesh` representation.

Faces are ordered internal-first (`[0, n_internal_faces)`), then boundary
faces grouped by patch, matching OpenFOAM. Call [`PolyMesh::to_fv_mesh`]
to obtain the geometry-carrying [`FvMesh`].

```rust
pub struct PolyMesh {
    pub points: Vec<crate::primitives::Vector3>,
    pub faces: Vec<MeshFace>,
    pub n_internal_faces: usize,
    pub n_cells: usize,
    pub patches: Vec<crate::mesh::BoundaryPatch>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `points` | `Vec<crate::primitives::Vector3>` | Mesh points `[m]`. |
| `faces` | `Vec<MeshFace>` | All faces, internal first then boundary. |
| `n_internal_faces` | `usize` | Number of internal faces (leading internal entries in `faces`). |
| `n_cells` | `usize` | Number of cells. |
| `patches` | `Vec<crate::mesh::BoundaryPatch>` | Boundary patches, covering `[n_internal_faces, faces.len())`. |

##### Implementations

###### Methods

- ```rust
  pub fn n_points(self: &Self) -> usize { /* ... */ }
  ```
  Number of points.

- ```rust
  pub fn n_faces(self: &Self) -> usize { /* ... */ }
  ```
  Total number of faces (internal + boundary).

- ```rust
  pub fn n_boundary_faces(self: &Self) -> usize { /* ... */ }
  ```
  Number of boundary faces.

- ```rust
  pub fn total_volume(self: &Self) -> f64 { /* ... */ }
  ```
  Total mesh volume `[m^3]` — the sum of all cell volumes.

- ```rust
  pub fn to_fv_mesh(self: &Self) -> Result<FvMesh, IoError> { /* ... */ }
  ```
  Convert to the geometry-carrying [`FvMesh`], computing cell

- ```rust
  pub fn read</* synthetic */ impl AsRef<Path>: AsRef<Path>>(dir: impl AsRef<Path>) -> Result<Self, IoError> { /* ... */ }
  ```
  Read a `polyMesh` from a `constant/polyMesh` directory.

- ```rust
  pub fn write</* synthetic */ impl AsRef<Path>: AsRef<Path>>(self: &Self, dir: impl AsRef<Path>) -> Result<(), IoError> { /* ... */ }
  ```
  Write the `polyMesh` files into `dir`, creating it if necessary.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> PolyMesh { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PolyMesh) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Types

#### Enum `IoError`

Errors raised while reading or writing OpenFOAM ASCII case files.

```rust
pub enum IoError {
    Io {
        path: String,
        source: std::io::Error,
    },
    Parse {
        context: String,
        message: String,
    },
    Unsupported {
        kind: String,
        name: String,
        context: String,
    },
    Mesh(String),
}
```

##### Variants

###### `Io`

An underlying filesystem error (file missing, permission denied, …).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `path` | `String` | The path being read or written when the error occurred. |
| `source` | `std::io::Error` | The underlying `std::io` error. |

###### `Parse`

The token stream did not match the expected grammar.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `context` | `String` | What was being parsed (file name / entry / block). |
| `message` | `String` | Human-readable description of the mismatch. |

###### `Unsupported`

A boundary-condition or field type that this reader does not yet
support was encountered.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `kind` | `String` | Category of the unsupported item (e.g. `"boundaryField type"`). |
| `name` | `String` | The offending type/keyword. |
| `context` | `String` | Where it was found. |

###### `Mesh`

The parsed topology could not be assembled into a valid mesh.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **Error**
  - ```rust
    fn source(self: &Self) -> ::core::option::Option<&dyn ::thiserror::__private18::Error + ''static> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Re-exports

#### Re-export `FoamCase`

```rust
pub use case::FoamCase;
```

#### Re-export `Dimensioned`

```rust
pub use dict::Dimensioned;
```

#### Re-export `FoamDict`

```rust
pub use dict::FoamDict;
```

#### Re-export `FoamEntry`

```rust
pub use dict::FoamEntry;
```

#### Re-export `FoamHeader`

```rust
pub use dict::FoamHeader;
```

#### Re-export `FoamValue`

```rust
pub use dict::FoamValue;
```

#### Re-export `read_vol_scalar_field`

```rust
pub use field::read_vol_scalar_field;
```

#### Re-export `read_vol_vector_field`

```rust
pub use field::read_vol_vector_field;
```

#### Re-export `write_vol_scalar_field`

```rust
pub use field::write_vol_scalar_field;
```

#### Re-export `write_vol_vector_field`

```rust
pub use field::write_vol_vector_field;
```

#### Re-export `MeshFace`

```rust
pub use poly_mesh::MeshFace;
```

#### Re-export `PolyMesh`

```rust
pub use poly_mesh::PolyMesh;
```

## Module `krylov`

Layer 2 — asymmetric Krylov iterative solvers (BiCGStab, restarted GMRES)
and preconditioners (Jacobi, ILU(0)) for the sparse `LduMatrix`.
Asymmetric Krylov iterative solvers and preconditioners for sparse `A x = b`.

This module complements the crate's existing SPD-only machinery (DIC-PCG and
GAMG in [`crate::ldu_matrix::solvers`]) with the **nonsymmetric** iterative
solvers a Newton–Krylov subsurface-flow solver needs, where the Jacobian is
not symmetric:

- [`bicgstab`](crate::krylov::bicgstab()) — preconditioned BiCGStab: fixed work/storage per iteration,
  breakdown-guarded.
- [`gmres`](crate::krylov::gmres()) — restarted, right-preconditioned GMRES(m): residual-minimising,
  robust, `O(m)` storage.

and three preconditioners dispatched by the [`Preconditioner`](crate::krylov::Preconditioner) enum (never
trait objects, per the workspace design rules):

- [`Preconditioner::identity`](crate::krylov::Preconditioner::identity) — no preconditioning (`M = I`).
- [`Preconditioner::jacobi`](crate::krylov::Preconditioner::jacobi) — diagonal scaling; always applicable.
- [`Preconditioner::ilu0`](crate::krylov::Preconditioner::ilu0) — genuine ILU(0) incomplete factorisation.

# Matrix representation and conventions

All solvers act on [`crate::ldu_matrix::LduMatrix`], the crate's face-addressed
sparse matrix, and use only its `multiply` (SpMV) and `residual` kernels. The
system size `n` is `LduMatrix::n_cells`; all right-hand-side, guess, and
solution slices have length `n`. Every quantity here is a dimensionless `f64`:
a Krylov subspace mixes residuals, search directions and increments that share
no single physical dimension, so no `uom` typing is applied — apply units at
the field/equation layer that assembles the matrix.

# Convergence

The stopping test for both solvers is the **relative** residual
`||b − A x||₂ / ||b||₂ ≤ tolerance`. The reported `final_residual` is always the
*true* residual of the returned iterate (recomputed from `A` and `b`), not an
internal estimate. A right-hand side that is exactly zero returns `x = 0`,
`converged = true`, `0` iterations.

# Example

```rust
use outram_foam_basic_lib::ldu_matrix::LduMatrix;
use outram_foam_basic_lib::krylov::{bicgstab, Preconditioner, KrylovSettings};

// 3-cell chain: cells 0-1 and 1-2 share a face each.
let mut a = LduMatrix::new(3, vec![0, 1], vec![1, 2]);
a.diag = vec![4.0, 4.0, 4.0];
a.lower = vec![-1.0, -1.0];
a.upper = vec![-1.0, -1.0];
let b = vec![1.0, 2.0, 3.0];

let precond = Preconditioner::jacobi(&a);
let settings = KrylovSettings::default();
let (x, result) = bicgstab(&a, &b, None, &precond, &settings);

assert!(result.converged);
// Verify: A x ≈ b.
let ax = a.multiply(&x);
for i in 0..3 {
    assert!((ax[i] - b[i]).abs() < 1e-6);
}
```

```rust
pub mod krylov { /* ... */ }
```

### Modules

## Module `vecops`

Dense BLAS-1 vector primitives for the Krylov solvers.

Pure-Rust replacements for the handful of level-1 BLAS operations the
iterative solvers need. All operands are dimensionless `&[f64]` slices whose
length equals the number of unknowns (mesh cells); no `uom` typing is applied
here because a Krylov subspace mixes residuals, search directions, and
solution increments that share no single physical dimension.

Every function is `O(n)` in the slice length and allocation-free (results are
either scalars or written in place), so they are safe to call inside the
innermost solver loops.

```rust
pub mod vecops { /* ... */ }
```

### Functions

#### Function `dot`

Euclidean inner product `Σ_i a_i · b_i` (dimensionless).

Both slices must have the same length; a mismatch panics via
`debug_assert`. Valid for any finite inputs; returns `0.0` for empty slices.

```rust
pub fn dot(a: &[f64], b: &[f64]) -> f64 { /* ... */ }
```

#### Function `nrm2`

Euclidean 2-norm `sqrt(Σ_i x_i²)` (dimensionless, always `>= 0`).

Computed as `sqrt(dot(x, x))`. For very large magnitudes this can overflow to
`+inf`; inputs are expected to be within normal `f64` range, which holds for
well-scaled linear systems.

```rust
pub fn nrm2(x: &[f64]) -> f64 { /* ... */ }
```

#### Function `axpy`

AXPY update `y := alpha · x + y`, in place.

`alpha` is a dimensionless scalar; `x` and `y` must have equal length (a
mismatch panics via `debug_assert`). `y` is overwritten with the result.

```rust
pub fn axpy(alpha: f64, x: &[f64], y: &mut [f64]) { /* ... */ }
```

#### Function `scal`

Scale `x := alpha · x`, in place.

`alpha` is a dimensionless scalar. Every element of `x` is multiplied by
`alpha`.

```rust
pub fn scal(alpha: f64, x: &mut [f64]) { /* ... */ }
```

### Types

#### Struct `KrylovSettings`

Iteration controls shared by [`bicgstab`] and [`gmres`].

All fields are plain scalars with no units.

```rust
pub struct KrylovSettings {
    pub tolerance: f64,
    pub max_iter: usize,
    pub restart: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `tolerance` | `f64` | Relative convergence tolerance on `||b − A x||₂ / ||b||₂`. Must be `> 0`;<br>typical range `1e-12 .. 1e-4`. Default `1e-8`. |
| `max_iter` | `usize` | Maximum total number of iterations (matrix–vector products) before the<br>solver returns unconverged. Default `1000`. |
| `restart` | `usize` | GMRES restart length `m` — the Krylov subspace dimension per outer cycle,<br>trading memory (`O(m·n)`) against convergence robustness. Ignored by<br>BiCGStab. `0` means "no restart" (`m = max_iter`). Default `30`. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> KrylovSettings { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```
    Defaults: `tolerance = 1e-8`, `max_iter = 1000`, `restart = 30`.

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `KrylovResult`

Outcome of a Krylov solve.

All fields are plain scalars with no units.

```rust
pub struct KrylovResult {
    pub n_iterations: usize,
    pub final_residual: f64,
    pub converged: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `n_iterations` | `usize` | Number of iterations (matrix–vector products) actually performed. |
| `final_residual` | `f64` | The **true** relative residual `||b − A x||₂ / ||b||₂` of the returned<br>iterate, recomputed from `A` and `b` (dimensionless, `>= 0`). |
| `converged` | `bool` | `true` iff `final_residual <= settings.tolerance`. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> KrylovResult { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `Preconditioner`

Preconditioner `M^{-1} ≈ A^{-1}`, dispatched by enum (no trait objects).

A preconditioner turns a residual `r` into `z = M^{-1} r`, an approximate
error, which the Krylov solvers use to accelerate convergence. Construct one
from the system matrix with [`Preconditioner::jacobi`] or
[`Preconditioner::ilu0`], or use [`Preconditioner::identity`] for none.

```rust
pub enum Preconditioner {
    Identity,
    Jacobi(JacobiPreconditioner),
    Ilu0(Ilu0Preconditioner),
}
```

##### Variants

###### `Identity`

No preconditioning: `M = I`, so `z = r`.

###### `Jacobi`

Diagonal (Jacobi) scaling: `z = r / diag(A)`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `JacobiPreconditioner` |  |

###### `Ilu0`

ILU(0) incomplete factorisation: `z = (LU)^{-1} r`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Ilu0Preconditioner` |  |

##### Implementations

###### Methods

- ```rust
  pub fn identity() -> Self { /* ... */ }
  ```
  Identity preconditioner (`M = I`) — equivalent to no preconditioning.

- ```rust
  pub fn jacobi(a: &LduMatrix) -> Self { /* ... */ }
  ```
  Build a Jacobi (reciprocal-diagonal) preconditioner from `a`.

- ```rust
  pub fn ilu0(a: &LduMatrix) -> Self { /* ... */ }
  ```
  Build an ILU(0) preconditioner from `a` (same sparsity pattern as `A`).

- ```rust
  pub fn apply(self: &Self, r: &[f64], z: &mut [f64]) { /* ... */ }
  ```
  Apply the preconditioner: write `z = M^{-1} r`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Re-exports

#### Re-export `bicgstab`

```rust
pub use bicgstab::bicgstab;
```

#### Re-export `gmres`

```rust
pub use gmres::gmres;
```

#### Re-export `Ilu0Preconditioner`

```rust
pub use preconditioner::Ilu0Preconditioner;
```

#### Re-export `JacobiPreconditioner`

```rust
pub use preconditioner::JacobiPreconditioner;
```

## Module `ldu_matrix`

Layer 2 — sparse LDU (lower/diagonal/upper) matrices, the assembled
`FvMatrix`, and iterative linear solvers (CG, Gauss–Seidel, GAMG).
Sparse LDU (lower-diagonal-upper) linear algebra for implicit FV solves.

Holds the face-addressed sparse matrix representation used by the
finite-volume implicit operators and the iterative solvers that invert it:

- [`ldu_matrix::LduMatrix`] — the raw sparse coefficients (diagonal + per-face
  lower/upper off-diagonals) and matrix–vector / residual kernels.
- [`FvMatrix`](crate::ldu_matrix::fv_matrix::FvMatrix) — a scalar equation `A·φ = b` for a `VolScalarField`,
  assembled by the Layer-3 `fvm::` operators.
- [`FvVectorMatrix`](crate::ldu_matrix::fv_vector_matrix::FvVectorMatrix) — the vector counterpart `A·U = b` with
  scalar LDU coefficients and a `Field<Vector3>` source.
- [`solvers`](crate::ldu_matrix::solvers) — Gauss-Seidel, DIC-preconditioned conjugate gradient, GAMG
  (algebraic multigrid), and the [`krylov_solve`](crate::ldu_matrix::solvers::krylov_solve()) adapter onto the asymmetric
  BiCGStab / GMRES kernels in [`crate::krylov`].

Belongs here: the sparse-matrix storage, its arithmetic, and the linear
solvers. Field types, meshes, and the differential operators that build these
matrices live in their own modules.

```rust
pub mod ldu_matrix { /* ... */ }
```

### Modules

## Module `fv_matrix`

```rust
pub mod fv_matrix { /* ... */ }
```

### Types

#### Struct `FvMatrix`

Sparse implicit matrix equation `A·φ = b` for a scalar field φ.

Mirrors `Foam::fvMatrix<scalar>` from
`src/finiteVolume/fvMatrices/fvMatrix/fvMatrix.H`.

Assembled incrementally by `fvm::` operators in Layer 3; solved via
`self.solve()`.

```rust
pub struct FvMatrix {
    pub mesh: std::sync::Arc<crate::mesh::fv_mesh::FvMesh>,
    pub ldu: super::ldu_matrix::LduMatrix,
    pub source: crate::fields::field::Field<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `std::sync::Arc<crate::mesh::fv_mesh::FvMesh>` | Mesh the equation is defined on (shares the face addressing). |
| `ldu` | `super::ldu_matrix::LduMatrix` | Sparse LDU coefficients of the operator `A`. |
| `source` | `crate::fields::field::Field<f64>` | Right-hand-side source term, length `n_cells`. |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: Arc<FvMesh>) -> Self { /* ... */ }
  ```
  Create a new zero-initialised FvMatrix for the given mesh.

- ```rust
  pub fn solve</* synthetic */ impl Into<String>: Into<String>>(self: &Self, name: impl Into<String>, settings: SolverSettings) -> (VolScalarField, SolverPerformance) { /* ... */ }
  ```
  Solve `A·φ = source` and return the solution as a `VolScalarField`.

- ```rust
  pub fn solve_cg</* synthetic */ impl Into<String>: Into<String>>(self: &Self, name: impl Into<String>, settings: SolverSettings) -> (VolScalarField, SolverPerformance) { /* ... */ }
  ```
  Solve the system with preconditioned conjugate gradient (cold start).

- ```rust
  pub fn solve_cg_with_guess</* synthetic */ impl Into<String>: Into<String>>(self: &Self, name: impl Into<String>, initial: &VolScalarField, settings: SolverSettings) -> (VolScalarField, SolverPerformance) { /* ... */ }
  ```
  Solve with PCG, **warm-started** from `initial` (typically the previous

- ```rust
  pub fn solve_gamg</* synthetic */ impl Into<String>: Into<String>>(self: &Self, name: impl Into<String>, settings: SolverSettings) -> (VolScalarField, SolverPerformance) { /* ... */ }
  ```
  Solve the system with GAMG (algebraic multigrid), cold-started from

- ```rust
  pub fn solve_gamg_with_guess</* synthetic */ impl Into<String>: Into<String>>(self: &Self, name: impl Into<String>, initial: &VolScalarField, settings: SolverSettings) -> (VolScalarField, SolverPerformance) { /* ... */ }
  ```
  Solve with GAMG, **warm-started** from `initial` (typically the previous

- ```rust
  pub fn solve_bicgstab</* synthetic */ impl Into<String>: Into<String>>(self: &Self, name: impl Into<String>, options: KrylovOptions, settings: SolverSettings) -> (VolScalarField, SolverPerformance) { /* ... */ }
  ```
  Solve the system with **preconditioned BiCGStab**, cold-started from

- ```rust
  pub fn solve_bicgstab_with_guess</* synthetic */ impl Into<String>: Into<String>>(self: &Self, name: impl Into<String>, initial: &VolScalarField, options: KrylovOptions, settings: SolverSettings) -> (VolScalarField, SolverPerformance) { /* ... */ }
  ```
  Solve with preconditioned BiCGStab, **warm-started** from `initial`

- ```rust
  pub fn solve_gmres</* synthetic */ impl Into<String>: Into<String>>(self: &Self, name: impl Into<String>, options: KrylovOptions, settings: SolverSettings) -> (VolScalarField, SolverPerformance) { /* ... */ }
  ```
  Solve the system with **restarted, right-preconditioned GMRES(m)**,

- ```rust
  pub fn solve_gmres_with_guess</* synthetic */ impl Into<String>: Into<String>>(self: &Self, name: impl Into<String>, initial: &VolScalarField, options: KrylovOptions, settings: SolverSettings) -> (VolScalarField, SolverPerformance) { /* ... */ }
  ```
  Solve with restarted GMRES(m), **warm-started** from `initial`.

- ```rust
  pub fn solve_krylov</* synthetic */ impl Into<String>: Into<String>>(self: &Self, name: impl Into<String>, initial: Option<&VolScalarField>, method: KrylovMethod, options: KrylovOptions, settings: SolverSettings) -> (VolScalarField, SolverPerformance) { /* ... */ }
  ```
  Solve with the Krylov method named by `method`, optionally warm-started.

- ```rust
  pub fn add_to_diag(self: &mut Self, coeff: &Field<f64>) { /* ... */ }
  ```
  Add `coeff * I` to the diagonal (e.g. from a time derivative term).

- ```rust
  pub fn add_to_source(self: &mut Self, term: &Field<f64>) { /* ... */ }
  ```
  Add `coeff[c]` to the source at cell `c`.

- ```rust
  pub fn add_face_coeff(self: &mut Self, face: usize, coeff: f64) { /* ... */ }
  ```
  Add upper/lower contributions from a face (used by fvm::laplacian etc.).

- ```rust
  pub fn set_reference(self: &mut Self, cell: usize, value: f64) { /* ... */ }
  ```
  Pin one cell to a reference value — fixes the singular pressure matrix

- ```rust
  pub fn a_field(self: &Self) -> VolScalarField { /* ... */ }
  ```
  Diagonal coefficient per cell: `A[c] = diag[c]`.

- ```rust
  pub fn h_field(self: &Self, x: &VolScalarField) -> VolScalarField { /* ... */ }
  ```
  Off-diagonal residual: `H[c] = source[c] − Σ off-diag · x`.

###### Trait Implementations

- **Add**
  - ```rust
    fn add(self: Self, rhs: Self) -> Self { /* ... */ }
    ```

- **AddAssign**
  - ```rust
    fn add_assign(self: &mut Self, rhs: Self) { /* ... */ }
    ```

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Neg**
  - ```rust
    fn neg(self: Self) -> Self { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sub**
  - ```rust
    fn sub(self: Self, rhs: Self) -> Self { /* ... */ }
    ```

- **SubAssign**
  - ```rust
    fn sub_assign(self: &mut Self, rhs: Self) { /* ... */ }
    ```

- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SolverSettings`

Solver settings passed to `FvMatrix::solve`.

```rust
pub struct SolverSettings {
    pub tolerance: f64,
    pub max_iter: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `tolerance` | `f64` | Convergence tolerance on the normalised residual (dimensionless). |
| `max_iter` | `usize` | Maximum iteration/sweep count before giving up. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> SolverSettings { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SolverPerformance`

Summary of a linear solve.

```rust
pub struct SolverPerformance {
    pub n_iterations: usize,
    pub final_residual: f64,
    pub converged: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `n_iterations` | `usize` | Number of iterations/sweeps actually performed. |
| `final_residual` | `f64` | Normalised residual at exit (dimensionless). |
| `converged` | `bool` | `true` if `final_residual` dropped below the requested tolerance. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> SolverPerformance { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `fv_vector_matrix`

```rust
pub mod fv_vector_matrix { /* ... */ }
```

### Types

#### Struct `FvVectorMatrix`

Implicit vector equation `A·U = b` for a `VolVectorField`.

Mirrors `Foam::fvVectorMatrix` (`fvMatrix<vector>`).

The LDU coefficients are **scalar** — they multiply the entire velocity
vector equally in all three directions.  The source vector is a
`Field<Vector3>`.  Solving decomposes into three independent scalar
Gauss-Seidel solves (one per component).

```rust
pub struct FvVectorMatrix {
    pub mesh: std::sync::Arc<crate::mesh::fv_mesh::FvMesh>,
    pub ldu: super::ldu_matrix::LduMatrix,
    pub source: crate::fields::field::Field<crate::primitives::Vector3>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `std::sync::Arc<crate::mesh::fv_mesh::FvMesh>` | Mesh the equation is defined on (shares the face addressing). |
| `ldu` | `super::ldu_matrix::LduMatrix` | Scalar LDU coefficients of the operator `A` (shared by all 3 components). |
| `source` | `crate::fields::field::Field<crate::primitives::Vector3>` | Right-hand-side vector source per cell, length `n_cells`. |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: Arc<FvMesh>) -> Self { /* ... */ }
  ```
  Allocate a zero-initialised vector matrix for `mesh` (zero coefficients,

- ```rust
  pub fn add_to_diag(self: &mut Self, coeff: &Field<f64>) { /* ... */ }
  ```
  Add `coeff[c]` to the diagonal of cell `c` (e.g. a time-derivative term).

- ```rust
  pub fn add_to_source(self: &mut Self, term: &Field<Vector3>) { /* ... */ }
  ```
  Add `term[c]` to the vector source of cell `c`.

- ```rust
  pub fn set_reference(self: &mut Self, cell: usize, value: Vector3) { /* ... */ }
  ```
  Pin one cell's velocity to a fixed value (reference cell for closed domains).

- ```rust
  pub fn a_field(self: &Self) -> VolScalarField { /* ... */ }
  ```
  Diagonal coefficient per cell: `A[c] = diag[c]`.

- ```rust
  pub fn h_field(self: &Self, u: &VolVectorField) -> VolVectorField { /* ... */ }
  ```
  Off-diagonal + source residual: `H[c] = source[c] − Σ off-diag · U`.

- ```rust
  pub fn solve(self: &Self, name: &str, settings: SolverSettings) -> (VolVectorField, SolverPerformance) { /* ... */ }
  ```
  Solve each component (x, y, z) as an independent scalar Gauss-Seidel problem.

- ```rust
  pub fn solve_bicgstab(self: &Self, name: &str, options: KrylovOptions, settings: SolverSettings) -> (VolVectorField, SolverPerformance) { /* ... */ }
  ```
  Solve each velocity component with **preconditioned BiCGStab**,

- ```rust
  pub fn solve_gmres(self: &Self, name: &str, options: KrylovOptions, settings: SolverSettings) -> (VolVectorField, SolverPerformance) { /* ... */ }
  ```
  Solve each velocity component with **restarted GMRES(m)**, cold-started

- ```rust
  pub fn solve_krylov(self: &Self, name: &str, initial: Option<&VolVectorField>, method: KrylovMethod, options: KrylovOptions, settings: SolverSettings) -> (VolVectorField, SolverPerformance) { /* ... */ }
  ```
  Solve each velocity component with the Krylov method named by `method`,

###### Trait Implementations

- **Add**
  - ```rust
    fn add(self: Self, rhs: Self) -> Self { /* ... */ }
    ```

- **AddAssign**
  - ```rust
    fn add_assign(self: &mut Self, rhs: Self) { /* ... */ }
    ```

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> FvVectorMatrix { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Neg**
  - ```rust
    fn neg(self: Self) -> Self { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sub**
  - ```rust
    fn sub(self: Self, rhs: Self) -> Self { /* ... */ }
    ```

- **SubAssign**
  - ```rust
    fn sub_assign(self: &mut Self, rhs: Self) { /* ... */ }
    ```

- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `ldu_matrix`

```rust
pub mod ldu_matrix { /* ... */ }
```

### Types

#### Struct `LduMatrix`

Sparse LDU (lower-diagonal-upper) matrix for FV implicit operators.

Mirrors `Foam::lduMatrix` from
`src/OpenFOAM/matrices/lduMatrix/lduMatrix/lduMatrix.H`.

Storage follows OpenFOAM's face-addressing convention:
```text
A·x[c] = diag[c]·x[c]
         + Σ_{f: owner[f]=c} upper[f]·x[neighbour[f]]
         + Σ_{f: neighbour[f]=c} lower[f]·x[owner[f]]
```
For a symmetric matrix (e.g. Laplacian), `lower[f] == upper[f]`.

```rust
pub struct LduMatrix {
    pub n_cells: usize,
    pub n_internal_faces: usize,
    pub diag: Vec<f64>,
    pub lower: Vec<f64>,
    pub upper: Vec<f64>,
    pub owner: Vec<usize>,
    pub neighbour: Vec<usize>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `n_cells` | `usize` | Number of cells (matrix rows/columns; length of `diag`). |
| `n_internal_faces` | `usize` | Number of internal faces (length of `lower`/`upper`). |
| `diag` | `Vec<f64>` | Diagonal coefficients, length `n_cells`. |
| `lower` | `Vec<f64>` | Lower off-diagonal (neighbour → owner contribution), length `n_internal_faces`. |
| `upper` | `Vec<f64>` | Upper off-diagonal (owner → neighbour contribution), length `n_internal_faces`. |
| `owner` | `Vec<usize>` | Owner cell index per internal face (shared with `FvMesh`). |
| `neighbour` | `Vec<usize>` | Neighbour cell index per internal face (shared with `FvMesh`). |

##### Implementations

###### Methods

- ```rust
  pub fn new(n_cells: usize, owner: Vec<usize>, neighbour: Vec<usize>) -> Self { /* ... */ }
  ```
  Allocate a zero-filled LDU matrix for `n_cells` cells connected by the

- ```rust
  pub fn multiply(self: &Self, x: &[f64]) -> Vec<f64> { /* ... */ }
  ```
  Matrix–vector product `y = A·x` (used for residual calculation).

- ```rust
  pub fn residual(self: &Self, x: &[f64], b: &[f64]) -> Vec<f64> { /* ... */ }
  ```
  Residual `r = b - A·x`.

- ```rust
  pub fn normalised_residual(self: &Self, x: &[f64], b: &[f64]) -> f64 { /* ... */ }
  ```
  L1-scaled norm of residual: `||r||₁ / (||A·x||₁ + ε)`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> LduMatrix { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `solvers`

Iterative linear solvers for the sparse LDU systems `A·x = b`.

Each solver takes an [`LduMatrix`](super::ldu_matrix::LduMatrix) and a
right-hand side and returns the solution together with the iteration count
and final normalised residual:

- [`gauss_seidel`](crate::ldu_matrix::solvers::gauss_seidel()) — a robust smoother that also handles the asymmetric
  (convection-bearing) momentum matrix.
- [`conjugate_gradient`](crate::ldu_matrix::solvers::conjugate_gradient()) — DIC-preconditioned CG for symmetric SPD systems
  (the pressure Poisson equation).
- [`gamg`](crate::ldu_matrix::solvers::gamg()) — algebraic multigrid for the same symmetric SPD systems, with
  near mesh-independent convergence on fine grids.
- [`krylov_solve`](fn@crate::ldu_matrix::solvers::krylov_solve) — the adapter onto the **asymmetric** Krylov kernels in
  [`crate::krylov`] (BiCGStab / restarted GMRES with identity, Jacobi or
  ILU(0) preconditioning), for the convection-bearing matrices where PCG and
  GAMG do not apply and Gauss-Seidel is slow.

Belongs here: the linear-solver kernels only. The matrix assembly and the
`FvMatrix`/`FvVectorMatrix` wrappers that call them live one level up.

```rust
pub mod solvers { /* ... */ }
```

### Modules

## Module `conjugate_gradient`

```rust
pub mod conjugate_gradient { /* ... */ }
```

### Functions

#### Function `conjugate_gradient`

Preconditioned Conjugate Gradient solver for **symmetric** LDU matrices.

## Preconditioner — DIC (Diagonal-based Incomplete Cholesky)

Uses OpenFOAM's default symmetric preconditioner, `DIC`
(`Foam::DICPreconditioner`): an incomplete Cholesky factorisation that keeps
only the existing matrix sparsity (no fill-in). It is a forward/backward
sweep over the faces using a precomputed reciprocal diagonal `rD`, and is
far more effective than the plain Jacobi (`M = diag(A)`) preconditioner this
function used previously — Jacobi-PCG iteration count grows with the mesh
(∝ √κ ≈ O(Nₓ)), whereas DIC dramatically flattens it.

DIC requires the faces to be in **upper-triangular order**
(`owner[f] < neighbour[f]`, sorted), which is how OpenFOAM `polyMesh` writes
internal faces and how `read_poly_mesh` loads them.

## Warm start

`x0` is the initial guess. Pass `Some(previous_solution)` to **warm-start**
the solve from the last time step's field — for a transient run approaching
steady state the solution barely changes between steps, so the initial
residual is tiny and the solver converges in a handful of iterations (often
zero) instead of paying full convergence from `x = 0` every step. Pass
`None` for a cold start (`x = 0`).

## When to use vs Gauss-Seidel

| Solver | Good for |
|---|---|
| Gauss-Seidel | Convection-dominated (asymmetric upper ≠ lower) |
| PCG (this) | Symmetric SPD systems — pressure Poisson (`fvm::laplacian`) |

The pressure equation assembled by `fvm::laplacian` is symmetric
(`upper[f] == lower[f]`), so PCG converges in O(√κ) iterations vs
O(κ) for Gauss-Seidel, where κ is the condition number.

```rust
pub fn conjugate_gradient(ldu: &crate::ldu_matrix::ldu_matrix::LduMatrix, b: &[f64], x0: Option<&[f64]>, settings: &crate::ldu_matrix::fv_matrix::SolverSettings) -> (Vec<f64>, crate::ldu_matrix::fv_matrix::SolverPerformance) { /* ... */ }
```

## Module `gamg`

GAMG — Geometric-Agglomerated Multi-Grid solver for symmetric LDU systems.

This is a **serial, algebraic** port of OpenFOAM's `Foam::GAMGSolver` with
`algebraicPairGAMGAgglomeration`. "Algebraic" means the coarse grids are
built purely from the matrix coefficients (the face weights are `|upper|`),
with no mesh geometry — so it works on any symmetric [`LduMatrix`], not just
one with a backing mesh.

## Why multigrid

A DIC-preconditioned CG ([`conjugate_gradient`](crate::ldu_matrix::solvers::conjugate_gradient()))
needs O(√κ) ≈ O(Nₓ) iterations on the pressure Poisson equation — the count
grows as the mesh is refined. Multigrid eliminates error at every length
scale by recursing onto coarser grids, so it converges in a handful of
V-cycles almost independently of mesh size. It is OpenFOAM's default
pressure solver for this reason.

## The algorithm (recursive correction-scheme V-cycle)

Each V-cycle is the textbook correction scheme with pre- and post-smoothing
(`GamgCycle::solve_level`):

1. **Pre-smooth** the current level with Gauss-Seidel (`N_PRE_SWEEPS`).
2. Form the residual `r = b − A·x` and **restrict** it to the next coarser
   level (additive, `restrict_field`).
3. **Recurse** to compute the coarse correction; the coarsest level is
   solved directly by dense LU (`solve_coarsest`).
4. **Prolong** the correction back (injection, `prolong_field`) and add it.
5. **Post-smooth** the current level (`N_POST_SWEEPS`).

Pre- *and* post-smoothing makes this a symmetric V-cycle, which converges far
faster than a post-smoothing-only sawtooth. OpenFOAM's `GAMGSolver::Vcycle`
reaches similar robustness with `nPreSweeps = 0` plus correction *scaling*;
the symmetric form is the cleaner equivalent here.

The outer loop ([`gamg`]) repeats V-cycles until the relative residual
`‖r‖₂ / ‖b‖₂` falls below `settings.tolerance` — the same convergence metric
[`conjugate_gradient`](crate::ldu_matrix::solvers::conjugate_gradient()) uses, so the two solvers
are interchangeable under one `SolverSettings`.

## Restrictions

Symmetric matrices only (`lower == upper`), which is exactly the pressure
Poisson case. The coarse matrices inherit symmetry, so the whole hierarchy
stays symmetric and the Gauss-Seidel smoother / dense coarsest solve need no
special face ordering.

```rust
pub mod gamg { /* ... */ }
```

### Functions

#### Function `gamg`

Solve a symmetric SPD LDU system with GAMG (algebraic multigrid).

Drop-in counterpart of
[`conjugate_gradient`](crate::ldu_matrix::solvers::conjugate_gradient()):
same signature, same `‖r‖₂ / ‖b‖₂` convergence metric, and the same warm
start — pass `Some(previous_solution)` as `x0` to start from the last time
step's field. The GAMG hierarchy is rebuilt each call (agglomeration is O(n)
and cheap next to the V-cycles).

Requires `ldu` to be **symmetric** (`lower == upper`); this holds for the
pressure Poisson equation assembled by `fvm::laplacian`.

# Example

```
use outram_foam_basic_lib::prelude::*;

// 1-D Poisson −∇²φ = 1 on [0,1], φ(0)=φ(1)=0, 63 interior points.
let n = 63;
let h = 1.0 / (n + 1) as f64;
let owner: Vec<usize> = (0..n - 1).collect();
let neighbour: Vec<usize> = (1..n).collect();
let mut m = LduMatrix::new(n, owner, neighbour);
let c = 1.0 / (h * h);
m.diag = vec![2.0 * c; n];
m.upper = vec![-c; n - 1];
m.lower = vec![-c; n - 1];
let b = vec![1.0; n];

let settings = SolverSettings { tolerance: 1e-8, max_iter: 100 };
let (x, perf) = gamg(&m, &b, None, &settings);
assert!(perf.converged);
// Exact solution is φ = x(1−x)/2; check the midpoint.
let mid = (n / 2) as f64 * h;
assert!((x[n / 2] - mid * (1.0 - mid) / 2.0).abs() < 1e-3);
```

```rust
pub fn gamg(ldu: &crate::ldu_matrix::ldu_matrix::LduMatrix, b: &[f64], x0: Option<&[f64]>, settings: &crate::ldu_matrix::fv_matrix::SolverSettings) -> (Vec<f64>, crate::ldu_matrix::fv_matrix::SolverPerformance) { /* ... */ }
```

## Module `gauss_seidel`

```rust
pub mod gauss_seidel { /* ... */ }
```

### Functions

#### Function `gauss_seidel`

Gauss-Seidel iterative solver for `A·x = b`.

Performs at most `max_iter` sweeps; stops early when the normalised
residual drops below `tol`.  Returns `(iters, final_residual)`.

Mirrors `Foam::GaussSeidelSmoother` in
`src/OpenFOAM/matrices/lduMatrix/smoothers/GaussSeidel/`.

```rust
pub fn gauss_seidel(mat: &crate::ldu_matrix::ldu_matrix::LduMatrix, b: &[f64], x: &mut Vec<f64>, tol: f64, max_iter: usize) -> (usize, f64) { /* ... */ }
```

## Module `krylov_solve`

Bridge from the finite-volume solver settings to the asymmetric Krylov
solvers in [`crate::krylov`].

[`crate::krylov`] holds solver *kernels* (BiCGStab, restarted GMRES) that
speak plain `LduMatrix` + `&[f64]`. The finite-volume layer
([`FvMatrix`](crate::ldu_matrix::FvMatrix),
[`FvVectorMatrix`](crate::ldu_matrix::FvVectorMatrix)) speaks
[`SolverSettings`] / [`SolverPerformance`]. This module is the thin adapter
between the two, plus the two small selection enums that let a caller choose
the method and the preconditioner **by value, never by trait object**
(workspace design rule).

# Why this matters physically

Any equation carrying a convection term — momentum, energy, or a transported
scalar — assembles an **asymmetric** matrix (`lower[f] != upper[f]`), because
upwinding puts the flux on the donor side only. The crate's symmetric
machinery (DIC-PCG, GAMG) is therefore inapplicable to those systems, and
before this module the only fallback was plain Gauss-Seidel, whose iteration
count grows like the condition number `O(kappa)`. BiCGStab/GMRES with an
ILU(0) preconditioner is the direct analogue of OpenFOAM's `PBiCGStab` with
`DILU`, and converges far faster on the same systems. See
`tests/krylov_convection_diffusion.rs` for measured iteration counts.

# Units

The linear algebra is dimensionless. `A`, `b` and `x` carry whatever units the
assembling operator gave them (e.g. for `fvm::laplacian(gamma, T)` the source
is `[gamma]·[T]·m`); the solver only ever forms ratios, so no `uom` typing is
applied here. Apply units at the field/equation layer.

```rust
pub mod krylov_solve { /* ... */ }
```

### Types

#### Enum `PreconditionerKind`

Which preconditioner `M^{-1} ~ A^{-1}` a Krylov solve should build from the
matrix.

This is a *selection* enum: it carries no data, so it is `Copy` and can sit
in a settings struct. The built preconditioner itself is
[`crate::krylov::Preconditioner`]. Dispatch is by enum, never a trait object.

```rust
pub enum PreconditionerKind {
    None,
    Jacobi,
    Ilu0,
}
```

##### Variants

###### `None`

No preconditioning (`M = I`). Cheapest per iteration, most iterations.
Use only as a baseline or when the matrix is already well conditioned.

###### `Jacobi`

Diagonal (Jacobi) scaling, `z = r / diag(A)`. Cannot break down; costs one
divide per cell per iteration. A good default for a strongly
diagonally-dominant matrix (small time step, low Peclet number).

###### `Ilu0`

ILU(0) incomplete factorisation on the matrix's own sparsity pattern —
the analogue of OpenFOAM's `DILU`. Typically several times fewer
iterations than Jacobi on convection-dominated systems, at the cost of one
forward/backward sweep per iteration. **Default.**

##### Implementations

###### Methods

- ```rust
  pub fn build(self: Self, a: &LduMatrix) -> Preconditioner { /* ... */ }
  ```
  Build the concrete preconditioner for `a`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> PreconditionerKind { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> PreconditionerKind { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PreconditionerKind) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `KrylovMethod`

Which asymmetric Krylov method to run.

Both handle `lower[f] != upper[f]`; neither requires symmetry or positive
definiteness.

```rust
pub enum KrylovMethod {
    BiCGStab,
    Gmres,
}
```

##### Variants

###### `BiCGStab`

Preconditioned BiCGStab — constant work and storage per iteration.
The default, and the right first choice for a finite-volume momentum or
scalar-transport matrix. Can break down on strongly nonnormal systems, in
which case the solve returns `converged = false` with the best iterate
found rather than garbage.

###### `Gmres`

Restarted, right-preconditioned GMRES(m) — minimises the residual over
the Krylov subspace, so its residual history is monotone and it cannot
break down, but it stores `m` basis vectors (`O(m·n_cells)` memory).
Prefer it when BiCGStab stalls or breaks down.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> KrylovMethod { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> KrylovMethod { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &KrylovMethod) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `KrylovOptions`

Extra controls a Krylov solve needs beyond [`SolverSettings`].

Kept separate from `SolverSettings` deliberately: `SolverSettings` is shared
with Gauss-Seidel, PCG and GAMG, and adding fields to it would break every
existing struct-literal construction.

All fields are dimensionless.

```rust
pub struct KrylovOptions {
    pub preconditioner: PreconditionerKind,
    pub restart: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `preconditioner` | `PreconditionerKind` | Preconditioner to build from the matrix. Default<br>[`PreconditionerKind::Ilu0`]. |
| `restart` | `usize` | GMRES restart length `m` — the Krylov subspace dimension per outer cycle.<br>Larger `m` converges in fewer total inner iterations but costs `O(m·n)`<br>memory. Ignored by [`KrylovMethod::BiCGStab`]. `0` means "no restart"<br>(`m = max_iter`). Default `30`. |

##### Implementations

###### Methods

- ```rust
  pub fn with_preconditioner(preconditioner: PreconditionerKind) -> Self { /* ... */ }
  ```
  Options using the given preconditioner and the default restart (`30`).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> KrylovOptions { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```
    Defaults: ILU(0) preconditioning, GMRES restart `m = 30`.

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `krylov_solve`

Solve `A·x = b` with an asymmetric Krylov method, reporting in the
finite-volume layer's [`SolverPerformance`] form.

This is the single entry point that
[`FvMatrix::solve_krylov`](crate::ldu_matrix::FvMatrix::solve_krylov) and
[`FvVectorMatrix::solve_krylov`](crate::ldu_matrix::FvVectorMatrix::solve_krylov)
call; use it directly when you already hold a raw [`LduMatrix`].

# Arguments
- `a` — the sparse system matrix. May be asymmetric (`lower != upper`);
  symmetric matrices also work but PCG/GAMG are cheaper for those.
- `b` — right-hand side, length `a.n_cells`.
- `x0` — optional initial guess (e.g. the previous time step's field);
  `None` starts from zero.
- `method` — BiCGStab or GMRES(m).
- `options` — preconditioner choice and GMRES restart length.
- `settings` — `tolerance` and `max_iter`, shared with the other solvers.

# Convergence measure

`SolverPerformance::final_residual` is the **true relative 2-norm residual**
`||b − A·x||₂ / ||b||₂` of the returned iterate, recomputed from `a` and `b`
— the same measure
[`conjugate_gradient`](crate::ldu_matrix::solvers::conjugate_gradient()) reports, and
**not** the L1-scaled [`LduMatrix::normalised_residual`] that
[`gauss_seidel`](crate::ldu_matrix::solvers::gauss_seidel()) reports. When comparing
against Gauss-Seidel, recompute one common measure rather than comparing the
two reported numbers directly.

# Example

```rust
use outram_foam_basic_lib::ldu_matrix::LduMatrix;
use outram_foam_basic_lib::ldu_matrix::{
    krylov_solve, KrylovMethod, KrylovOptions, PreconditionerKind, SolverSettings,
};

// Asymmetric 3-cell chain: upper != lower (an upwinded convection stencil).
let mut a = LduMatrix::new(3, vec![0, 1], vec![1, 2]);
a.diag = vec![4.0, 4.0, 4.0];
a.lower = vec![-2.0, -2.0];
a.upper = vec![-1.0, -1.0];
let b = vec![1.0, 2.0, 3.0];

let (x, perf) = krylov_solve(
    &a,
    &b,
    None,
    KrylovMethod::BiCGStab,
    KrylovOptions::with_preconditioner(PreconditionerKind::Ilu0),
    &SolverSettings::default(),
);
assert!(perf.converged);

let ax = a.multiply(&x);
for i in 0..3 {
    assert!((ax[i] - b[i]).abs() < 1e-6);
}
```

```rust
pub fn krylov_solve(a: &crate::ldu_matrix::ldu_matrix::LduMatrix, b: &[f64], x0: Option<&[f64]>, method: KrylovMethod, options: KrylovOptions, settings: &crate::ldu_matrix::fv_matrix::SolverSettings) -> (Vec<f64>, crate::ldu_matrix::fv_matrix::SolverPerformance) { /* ... */ }
```

### Re-exports

#### Re-export `conjugate_gradient`

```rust
pub use conjugate_gradient::conjugate_gradient;
```

#### Re-export `gamg`

```rust
pub use gamg::gamg;
```

#### Re-export `gauss_seidel`

```rust
pub use gauss_seidel::gauss_seidel;
```

#### Re-export `krylov_solve`

```rust
pub use krylov_solve::krylov_solve;
```

#### Re-export `KrylovMethod`

```rust
pub use krylov_solve::KrylovMethod;
```

#### Re-export `KrylovOptions`

```rust
pub use krylov_solve::KrylovOptions;
```

#### Re-export `PreconditionerKind`

```rust
pub use krylov_solve::PreconditionerKind;
```

### Re-exports

#### Re-export `FvMatrix`

```rust
pub use fv_matrix::FvMatrix;
```

#### Re-export `SolverPerformance`

```rust
pub use fv_matrix::SolverPerformance;
```

#### Re-export `SolverSettings`

```rust
pub use fv_matrix::SolverSettings;
```

#### Re-export `FvVectorMatrix`

```rust
pub use fv_vector_matrix::FvVectorMatrix;
```

#### Re-export `LduMatrix`

```rust
pub use ldu_matrix::LduMatrix;
```

#### Re-export `conjugate_gradient`

```rust
pub use solvers::conjugate_gradient;
```

#### Re-export `conjugate_gradient`

```rust
pub use solvers::conjugate_gradient;
```

#### Re-export `gamg`

```rust
pub use solvers::gamg;
```

#### Re-export `gamg`

```rust
pub use solvers::gamg;
```

#### Re-export `gauss_seidel`

```rust
pub use solvers::gauss_seidel;
```

#### Re-export `gauss_seidel`

```rust
pub use solvers::gauss_seidel;
```

#### Re-export `krylov_solve`

```rust
pub use solvers::krylov_solve::krylov_solve;
```

#### Re-export `KrylovMethod`

```rust
pub use solvers::krylov_solve::KrylovMethod;
```

#### Re-export `KrylovOptions`

```rust
pub use solvers::krylov_solve::KrylovOptions;
```

#### Re-export `PreconditionerKind`

```rust
pub use solvers::krylov_solve::PreconditionerKind;
```

## Module `limiters`

TVD flux limiters — field-agnostic `psi(r)` functions on plain `f64`.

A **flux limiter** `psi(r)` blends a high-order (linear/central) face flux
with first-order upwind to suppress spurious oscillations near sharp
gradients, where `r` is the ratio of successive solution gradients. `psi = 0`
is first-order upwind; `psi = 1` recovers second-order (linear) differencing.

This is a **pure-`f64`, mesh-free** API so any finite-volume code (e.g. the
`outram-park-fork-pflotran` solute/energy transport) can build higher-order
TVD advection without depending on this crate's field/mesh types. A separate,
field-tied limiter for rhoCentralFoam reconstruction lives at
[`crate::fv_operators::fvc::Limiter`]; this module is the reusable,
general one, and the two should be consolidated eventually.

# Provenance (translated from OpenFOAM upstream source)

Each limiter here is a Rust translation of the corresponding `limiter()`
method in OpenFOAM's
`src/finiteVolume/interpolation/surfaceInterpolation/limitedSchemes/<name>/<name>.H`,
**Copyright (C) 2011-2022 OpenFOAM Foundation**, GNU General Public License
version 3 or later (this crate is GPL-3.0). Source read from
`github.com/OpenFOAM/OpenFOAM-dev` (master) on 2026-07-22. OpenFOAM® is a
registered trademark of OpenCFD Ltd (ESI Group); this is an independent
translation, not an official OpenFOAM product (see the workspace
`TRADEMARKS.md`).

The exact upstream expression is quoted in each variant's doc comment. Only
OpenFOAM's **r-based** limiters are ported: OpenFOAM's NVD-based schemes
(`QUICK`, `Gamma`, `SFCD`, `Phi`) use actual cell values rather than a pure
`psi(r)` and are **not** representable here, so they are deliberately omitted
rather than approximated.

```rust
pub mod limiters { /* ... */ }
```

### Types

#### Enum `FluxLimiter`

A TVD flux limiter `psi(r)`, translated from OpenFOAM's r-based
`limitedSchemes`. `psi = 0` is first-order upwind, `psi = 1` is second-order
linear; the TVD variants clip extrema (`psi(r) = 0` for `r <= 0`).

```rust
pub enum FluxLimiter {
    Upwind,
    Linear,
    VanLeer,
    VanAlbada,
    Minmod,
    SuperBee,
    Muscl,
    Umist,
    Ospre,
    LimitedLinear(f64),
}
```

##### Variants

###### `Upwind`

First-order `upwind`: `psi = 0`.

###### `Linear`

Unlimited `linear` (central) differencing: `psi = 1` (2nd order, not TVD).

###### `VanLeer`

`vanLeer`: upstream `(r + mag(r))/(1 + mag(r))`. Smooth, symmetric.

###### `VanAlbada`

`vanAlbada`: upstream `r*(r + 1)/(sqr(r) + 1)` with `r = max(0, r)`. Symmetric.

###### `Minmod`

`Minmod`: upstream `max(min(r, 1), 0)`. Most diffusive TVD limiter. Symmetric.

###### `SuperBee`

`SuperBee`: upstream `max(max(min(2r, 1), min(r, 2)), 0)`. Most compressive. Symmetric.

###### `Muscl`

`MUSCL`: upstream `max(min(min(2r, 0.5r + 0.5), 2), 0)`. Symmetric.

###### `Umist`

`UMIST`: upstream `max(min(min(min(2r, 0.75r + 0.25), 0.25r + 0.75), 2), 0)`.
Third-order biased (not symmetric).

###### `Ospre`

`OSPRE`: upstream `1.5 r (r + 1)/(r (r + 1) + 1)` with `r = max(0, r)`. Symmetric.

###### `LimitedLinear`

`limitedLinear(k)`: upstream `max(min((2/k) r, 1), 0)`, coefficient
`k` in `[0, 1]` (`k -> 0` approaches unlimited linear, `k = 1` most
limited). A k-blended bounded scheme — not strictly within the classic
`psi <= 2r` Sweby envelope for small `k`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn psi(self: &Self, r: f64) -> f64 { /* ... */ }
  ```
  The flux-limiter function `psi(r)`. `r` may be any `f64`; a non-finite `r`

- ```rust
  pub fn is_tvd(self: &Self) -> bool { /* ... */ }
  ```
  True if this is a second-order **TVD** limiter (everything except

- ```rust
  pub fn is_symmetric(self: &Self) -> bool { /* ... */ }
  ```
  True if the limiter is **symmetric** (`psi(r)/r == psi(1/r)`), the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> FluxLimiter { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FluxLimiter) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `math`

Layer 1g — mathematical special functions (inverse error function,
incomplete gamma functions and their inverse).
Special mathematical functions used by the thermophysics and statistics
kernels.

Ports the OpenFOAM `primitives/functions/Math` helpers: the inverse error
function ([`erf_inv`](crate::math::erf_inv::erf_inv)), the regularised lower/upper incomplete gamma
functions and their unnormalised forms
([`inc_gamma_ratio_p`](crate::math::inc_gamma::inc_gamma_ratio_p),
[`inc_gamma_ratio_q`](crate::math::inc_gamma::inc_gamma_ratio_q),
[`inc_gamma_p`](crate::math::inc_gamma::inc_gamma_p),
[`inc_gamma_q`](crate::math::inc_gamma::inc_gamma_q)), and the inverse of
the regularised lower incomplete gamma
([`inv_inc_gamma`](crate::math::inv_inc_gamma::inv_inc_gamma)). All arguments and
results are dimensionless `f64`.

```rust
pub mod math { /* ... */ }
```

### Modules

## Module `erf_inv`

```rust
pub mod erf_inv { /* ... */ }
```

### Functions

#### Function `erf_inv`

Inverse error function.

Returns `x` such that `erf(x) = y`.  Valid domain: `y ∈ (−1, 1)`.
Behaviour outside that domain is undefined.

Algorithm: Winitzki (2008) approximation with `a = 0.147`, which limits
the maximum relative error to O(10⁻⁴). Matches `Foam::Math::erfInv`.

Reference: S. Winitzki, "A handy approximation for the error function and
its inverse", preprint 2008.

```rust
pub fn erf_inv(y: f64) -> f64 { /* ... */ }
```

## Module `inc_gamma`

```rust
pub mod inc_gamma { /* ... */ }
```

### Functions

#### Function `inc_gamma_ratio_q`

Regularised upper incomplete gamma: `Q(a, x) = Γ(a, x) / Γ(a)`.

Selects from several branch formulas depending on `a` and `x` ranges,
exactly as in `Foam::Math::incGammaRatio_Q`.

```rust
pub fn inc_gamma_ratio_q(a: f64, x: f64) -> f64 { /* ... */ }
```

#### Function `inc_gamma_ratio_p`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Regularised lower incomplete gamma: `P(a, x) = γ(a, x) / Γ(a) = 1 − Q(a, x)`.

```rust
pub fn inc_gamma_ratio_p(a: f64, x: f64) -> f64 { /* ... */ }
```

#### Function `inc_gamma_q`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Upper incomplete gamma: `Γ(a, x) = Q(a, x) · Γ(a)`.

```rust
pub fn inc_gamma_q(a: f64, x: f64) -> f64 { /* ... */ }
```

#### Function `inc_gamma_p`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Lower incomplete gamma: `γ(a, x) = P(a, x) · Γ(a)`.

```rust
pub fn inc_gamma_p(a: f64, x: f64) -> f64 { /* ... */ }
```

## Module `inv_inc_gamma`

```rust
pub mod inv_inc_gamma { /* ... */ }
```

### Functions

#### Function `inv_inc_gamma`

Inverse regularised lower incomplete gamma: find `x` such that `P(a, x) = p`.

```rust
pub fn inv_inc_gamma(a: f64, p: f64) -> f64 { /* ... */ }
```

### Re-exports

#### Re-export `erf_inv`

```rust
pub use erf_inv::erf_inv;
```

#### Re-export `inc_gamma_p`

```rust
pub use inc_gamma::inc_gamma_p;
```

#### Re-export `inc_gamma_q`

```rust
pub use inc_gamma::inc_gamma_q;
```

#### Re-export `inc_gamma_ratio_p`

```rust
pub use inc_gamma::inc_gamma_ratio_p;
```

#### Re-export `inc_gamma_ratio_q`

```rust
pub use inc_gamma::inc_gamma_ratio_q;
```

#### Re-export `inv_inc_gamma`

```rust
pub use inv_inc_gamma::inv_inc_gamma;
```

## Module `matrix`

Layer 1b — dense `SquareMatrix` with direct (LU) solve.
Dense square-matrix linear algebra.

Provides [`SquareMatrix`](crate::matrix::square_matrix::SquareMatrix), a row-major `n×n` matrix of `f64` with in-place
LU decomposition (Crout, scaled partial pivoting) and back-substitution — the
direct linear solver used by the stiff ODE solver and other kernels. Failure
to solve is reported through [`MatrixError`](crate::matrix::square_matrix::MatrixError). Entries are bare `f64`; the
matrix carries no unit information.

```rust
pub mod matrix { /* ... */ }
```

### Modules

## Module `square_matrix`

```rust
pub mod square_matrix { /* ... */ }
```

### Types

#### Enum `MatrixError`

Error type for `SquareMatrix::solve`.

```rust
pub enum MatrixError {
    Singular {
        col: usize,
    },
}
```

##### Variants

###### `Singular`

The matrix is exactly singular: the LU decomposition found a zero pivot
at the given column (the entire remaining column was zero).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `col` | `usize` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> MatrixError { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, f: &mut std::fmt::Formatter<''_>) -> std::fmt::Result { /* ... */ }
    ```

- **Error**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &MatrixError) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `SquareMatrix`

Row-major n×n dense matrix of `f64`. Maps to `Foam::scalarSquareMatrix`.

LU decomposition uses Crout's algorithm with scaled partial pivoting,
matching `Foam::LUDecompose(scalarSquareMatrix&, labelList&)`.

```rust
pub struct SquareMatrix {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(n: usize) -> Self { /* ... */ }
  ```
  Allocate an `n×n` matrix initialised to all zeros.

- ```rust
  pub fn n(self: &Self) -> usize { /* ... */ }
  ```
  The matrix order `n` (number of rows = number of columns).

- ```rust
  pub fn get(self: &Self, i: usize, j: usize) -> f64 { /* ... */ }
  ```
  Element in row `i`, column `j` (both 0-based, must be `< n`).

- ```rust
  pub fn set(self: &mut Self, i: usize, j: usize, v: f64) { /* ... */ }
  ```
  Set the element in row `i`, column `j` to `v` (0-based indices `< n`).

- ```rust
  pub fn add(self: &mut Self, i: usize, j: usize, v: f64) { /* ... */ }
  ```
  Add `v` to the element in row `i`, column `j` (0-based indices `< n`).

- ```rust
  pub fn fill_zero(self: &mut Self) { /* ... */ }
  ```
  Reset every entry to zero, keeping the same order `n`.

- ```rust
  pub fn lu_decompose(self: &mut Self) -> Vec<usize> { /* ... */ }
  ```
  In-place LU decomposition with scaled partial pivoting.

- ```rust
  pub fn lu_back_substitute(self: &Self, pivot: &[usize], b: &mut Vec<f64>) { /* ... */ }
  ```
  Solve `LU·x = b` in-place (`b` is overwritten with the solution).

- ```rust
  pub fn solve(self: &Self, rhs: &[f64]) -> Result<Vec<f64>, MatrixError> { /* ... */ }
  ```
  Convenience: decompose a copy and solve `A·x = b`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> SquareMatrix { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Re-exports

#### Re-export `MatrixError`

```rust
pub use square_matrix::MatrixError;
```

#### Re-export `SquareMatrix`

```rust
pub use square_matrix::SquareMatrix;
```

## Module `mesh`

Layer 2 — the finite-volume mesh: cells, faces, boundary patches, and
geometric metrics.
Finite-volume mesh layer: topology and geometry.

This module holds the flat, cache-friendly mesh representation the FV
operators run on. It contains:

- [`FvMesh`](crate::mesh::fv_mesh::FvMesh) — the mesh itself (cells, faces, owner/neighbour connectivity,
  cell volumes `[m³]`, face-area vectors `[m²]`, and cell/face centres `[m]`),
  plus [`FvMeshBuilder`](crate::mesh::fv_mesh::FvMeshBuilder) to assemble one incrementally.
- [`BoundaryPatch`](crate::mesh::fv_mesh::BoundaryPatch) /
  [`PatchKind`](crate::mesh::fv_mesh::PatchKind) — boundary-patch descriptors.
- [`ami`](crate::mesh::ami) — arbitrary-mesh-interface (non-conformal periodic / `cyclicAMI`)
  face-overlap weighting and [`AmiCoupling`](crate::mesh::ami::AmiCoupling)
  representation.
- [`RegionInterface`](crate::mesh::region_interface::RegionInterface) — a face-to-face coupling map between two regions'
  patches (used by conjugate-heat-transfer solvers).
- [`MeshError`](crate::mesh::error::MeshError) — the errors raised during mesh construction and validation.

It stores only the data required by the operators; the OpenFOAM
`polyMesh → primitiveMesh → lduMesh` inheritance chain is not reproduced.

```rust
pub mod mesh { /* ... */ }
```

### Modules

## Module `ami`

Arbitrary Mesh Interface (AMI) weight computation and non-conformal periodic
(cyclicAMI) coupling representation.

Mirrors OpenFOAM's
`src/meshTools/AMIInterpolation/AMIInterpolation/AMIInterpolation.H`
(the geometric face-overlap weighting) and
`src/finiteVolume/fields/fvPatchFields/constraint/cyclicAMI/cyclicAMIFvPatchField.H`
(the coupled-interface contribution), together with the
`cyclicAMIPolyPatch` topology.

## What AMI is (and why it differs from plain cyclic)

A plain [`PatchKind::Cyclic`] patch pair is
**conformal**: local face `i` of one half matches local face `i` of the
other exactly one-to-one, so the seam is discretised like an ordinary
internal face (see [`CyclicCoupling`](crate::mesh::CyclicCoupling)).

A [`PatchKind::CyclicAmi`] pair is
**non-conformal**: the two halves' faces do *not* line up, so each *target*
face overlaps several *source* faces. The coupling for one target face is
therefore a **weighted set** of source cells, the weight of each being the
geometric overlap-area fraction
`w_k = overlap_area(target, source_k) / target_area`.
When a target is fully covered by sources these weights sum to `1`
(conservative interpolation), so the value seen across the seam is the
area-weighted average of the overlapping source cells.

## Overlap method implemented here (first pass — planar / 1-D structured)

[`overlap_weights_1d`] projects both patch halves onto a common seam plane
and treats each face as an **interval along a single transverse axis** of
constant out-of-plane depth (a structured 2-D seam). The overlap of a target
interval `[t0, t1]` with a source interval `[s0, s1]` is the 1-D segment
overlap `max(0, min(t1, s1) - max(t0, s0))`, multiplied by the constant
`depth` to give an overlap **area** `[m²]`. This is exact for axis-aligned,
coplanar, structured seams (e.g. a translational-periodic channel meshed with
differing transverse resolutions on the two halves) — the case this first
pass targets.

### Deferred (documented limitations)

- **General 3-D polygon clipping.** True `AMIInterpolation` clips arbitrary
  source polygons against each target polygon (Sutherland-Hodgman /
  greatest-area walk). That is *not* implemented here; only the 1-D interval
  overlap above is. Non-axis-aligned faces, skewed seams, and unstructured
  transverse tilings are out of scope for this pass.
- **Two transverse axes.** Only one transverse coordinate is overlapped; a
  fully 2-D tiled seam (subdivided in both in-plane directions) is not
  handled.
- **Non-planar / curved seams and per-face normal rotation** (`cyclicAMI`
  with a rotational transform) are not handled.

These limits are acceptable for the verification cases this module ships
(matching-mesh limit reproduces plain cyclic; a 2:1 non-conformal case is
conservative). This code is an **untrusted AI-assisted draft pending human
V&V review** (2026-08-04).

```rust
pub mod ami { /* ... */ }
```

### Types

#### Struct `AmiOverlap`

One overlap between a target face and a source face on an AMI seam.

Produced by [`overlap_weights_1d`]; purely geometric (carries the *local*
source-face index within the source patch, not a global face or cell index —
the mesh constructor attaches those when it builds an [`AmiWeight`]).

```rust
pub struct AmiOverlap {
    pub source: usize,
    pub overlap_area: f64,
    pub weight: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `source` | `usize` | Local index of the overlapping source face within the source patch. |
| `overlap_area` | `f64` | Geometric overlap area between the two faces `[m²]`. |
| `weight` | `f64` | Overlap fraction of the **target** face:<br>`overlap_area / target_area` (dimensionless). Summed over all sources of<br>one target this is `1` when the target is fully covered. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> AmiOverlap { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AmiOverlap) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `AmiWeight`

One weighted source-cell contribution to a single AMI target seam face.

The finite-volume operators treat each [`AmiWeight`] as one "partial internal
face" of area [`overlap_area`](Self::overlap_area) joining the target cell to
[`source_cell`](Self::source_cell): the off-diagonal seam coefficient is
scaled by this pair's overlap so the whole target face's flux is distributed
conservatively across its overlapping sources.

```rust
pub struct AmiWeight {
    pub source_face: usize,
    pub source_cell: usize,
    pub weight: f64,
    pub overlap_area: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `source_face` | `usize` | Global face index of the overlapped source face. |
| `source_cell` | `usize` | Owner cell of the source face — the "neighbour" across this partial seam. |
| `weight` | `f64` | Overlap fraction of the target face (`overlap_area / target_area`,<br>dimensionless). Per target these sum to `≈ 1` (conservative). |
| `overlap_area` | `f64` | Geometric overlap area of this target/source pair `[m²]`. Used as the<br>effective face area of the partial seam face in the diffusion/advection<br>coefficient. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> AmiWeight { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AmiWeight) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `AmiCoupling`

One target seam face of a
[`PatchKind::CyclicAmi`]
patch pair, together with the weighted set of source cells it couples to.

Mirrors the coupled-interface contribution of `Foam::cyclicAMIFvPatchField`
whose `patchNeighbourField()` supplies the *interpolated* partner value
`Σ_k w_k · φ(source_cell_k)`.

The couplings are appended to the LDU face addressing *after* the internal
faces and the [`CyclicCoupling`](crate::mesh::CyclicCoupling)s: one LDU face
per [`AmiWeight`], laid out in `ami_couplings` order (see
[`FvMesh::ami_ldu_start`](crate::mesh::FvMesh::ami_ldu_start) and
[`FvMesh::n_ami_faces`](crate::mesh::FvMesh::n_ami_faces)).

```rust
pub struct AmiCoupling {
    pub target_face: usize,
    pub target_cell: usize,
    pub target_patch: usize,
    pub source_patch: usize,
    pub local: usize,
    pub weights: Vec<AmiWeight>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `target_face` | `usize` | Global face index of this target seam face. |
| `target_cell` | `usize` | Owner cell of the target face — the "owner" side of every partial seam<br>face in [`weights`](Self::weights). |
| `target_patch` | `usize` | Patch index of the target half of the AMI pair. |
| `source_patch` | `usize` | Patch index of the source half of the AMI pair. |
| `local` | `usize` | Local face index of the target face within its patch<br>(`target_face - patches[target_patch].start`). |
| `weights` | `Vec<AmiWeight>` | Weighted source contributions; per-target weights sum to `≈ 1`. |

##### Implementations

###### Methods

- ```rust
  pub fn weight_sum(self: &Self) -> f64 { /* ... */ }
  ```
  Sum of this target's overlap weights. Equals `1` (to rounding) when the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> AmiCoupling { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AmiCoupling) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `overlap_weights_1d`

Planar / 1-D-structured AMI overlap weights.

Given a target patch and a source patch each described as a list of
**transverse intervals** `(lo, hi)` (the projection of each face onto a
single in-plane axis of the shared seam plane) plus the constant out-of-plane
`depth` `[m]`, return for every target face the list of [`AmiOverlap`]s with
the source faces it geometrically overlaps.

- `target_spans[i] = (t_lo, t_hi)` — transverse extent of target face `i` `[m]`.
- `source_spans[j] = (s_lo, s_hi)` — transverse extent of source face `j` `[m]`.
- `depth` — constant out-of-plane face depth `[m]` (`> 0`).

The overlap **area** of target `i` with source `j` is
`interval_overlap · depth` `[m²]`; the **weight** is that area divided by the
target face's own area `(t_hi - t_lo)·depth`, i.e. simply the fraction of the
target interval covered by the source interval. Sources with zero overlap are
omitted. When the target intervals are fully tiled by the source intervals
(full coverage) each target's weights sum to `1`.

# Panics
Panics if `depth <= 0` or if any target span is degenerate (`hi <= lo`).

# Example
```
use outram_foam_basic_lib::mesh::ami::overlap_weights_1d;
// One coarse target [0,1] over two fine sources [0,0.5], [0.5,1], depth 1.
let w = overlap_weights_1d(&[(0.0, 1.0)], &[(0.0, 0.5), (0.5, 1.0)], 1.0);
assert_eq!(w[0].len(), 2);
assert!((w[0][0].weight - 0.5).abs() < 1e-15);
assert!((w[0][1].weight - 0.5).abs() < 1e-15);
// Conservative: weights sum to 1.
let s: f64 = w[0].iter().map(|o| o.weight).sum();
assert!((s - 1.0).abs() < 1e-15);
```

```rust
pub fn overlap_weights_1d(target_spans: &[(f64, f64)], source_spans: &[(f64, f64)], depth: f64) -> Vec<Vec<AmiOverlap>> { /* ... */ }
```

## Module `error`

```rust
pub mod error { /* ... */ }
```

### Types

#### Enum `MeshError`

Errors produced by the mesh layer (mesh construction and validation).

```rust
pub enum MeshError {
    ArrayLengthMismatch {
        array: &'static str,
        expected: usize,
        got: usize,
    },
    PatchStartMismatch {
        name: String,
        expected: usize,
        got: usize,
    },
    PatchCoverageMismatch {
        covered: usize,
        n_faces: usize,
    },
    NonPositiveCellCount {
        got: i64,
    },
    CyclicPairMismatch {
        name: String,
        reason: &'static str,
    },
    AmiCouplingInvalid {
        target_face: usize,
        reason: &'static str,
    },
}
```

##### Variants

###### `ArrayLengthMismatch`

An array field in the mesh has the wrong length.

For example, `owner` must have length `n_faces`; `neighbour` must have
length `n_internal_faces`; `cell_volumes` must have length `n_cells`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `array` | `&'static str` | Name of the offending array (e.g. `"owner"`, `"cell_volumes"`). |
| `expected` | `usize` |  |
| `got` | `usize` |  |

###### `PatchStartMismatch`

A boundary patch does not start immediately after the previous one,
leaving a gap or overlap in face coverage.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Name of the offending patch. |
| `expected` | `usize` |  |
| `got` | `usize` |  |

###### `PatchCoverageMismatch`

The sum of all patch sizes does not equal the number of boundary faces.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `covered` | `usize` | Total face count covered by all patches. |
| `n_faces` | `usize` | Total face count in the mesh. |

###### `NonPositiveCellCount`

`number_of_cells` was zero or negative when building a 1-D mesh.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `got` | `i64` |  |

###### `CyclicPairMismatch`

A [`PatchKind::Cyclic`](crate::mesh::PatchKind::Cyclic) patch pair is
inconsistent — e.g. the partner index is out of range, the partner does
not name this patch back, or the two halves have different face counts.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Name of the offending cyclic patch. |
| `reason` | `&'static str` | Why the pair is invalid. |

###### `AmiCouplingInvalid`

A [`PatchKind::CyclicAmi`](crate::mesh::PatchKind::CyclicAmi)
(non-conformal periodic) coupling is inconsistent — e.g. a target/source
cell or face index is out of range, or a target face has no overlapping
source faces.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `target_face` | `usize` | Global face index of the offending AMI target face. |
| `reason` | `&'static str` | Why the coupling is invalid. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> MeshError { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, f: &mut std::fmt::Formatter<''_>) -> std::fmt::Result { /* ... */ }
    ```

- **Error**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &MeshError) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `fv_mesh`

```rust
pub mod fv_mesh { /* ... */ }
```

### Types

#### Struct `BoundaryPatch`

Boundary patch descriptor: topology + kind.

Face indices in [start, start + size) within the global face array.
All boundary faces appear after the internal faces in OpenFOAM ordering:
`start >= n_internal_faces` for every patch.

```rust
pub struct BoundaryPatch {
    pub name: String,
    pub start: usize,
    pub size: usize,
    pub kind: PatchKind,
    pub cyclic_partner: Option<usize>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Patch name (e.g. `"left"`, `"wall"`, `"inlet"`). |
| `start` | `usize` | Index of the first face of this patch in the global face list. |
| `size` | `usize` | Number of faces in this patch. |
| `kind` | `PatchKind` | Topological type of the patch (wall, symmetry, empty, …). |
| `cyclic_partner` | `Option<usize>` | For a [`PatchKind::Cyclic`] patch, the **patch index** of its matching<br>partner (the other half of the periodic pair); `None` for every<br>non-cyclic patch, and `None` for a cyclic patch whose partner has not yet<br>been resolved (e.g. one read from a `polyMesh` whose `neighbourPatch`<br>ordering is not parsed yet).<br><br>Mirrors `Foam::cyclicPolyPatch::neighbPatchID()`<br>(`src/meshTools/.../cyclic/cyclicPolyPatch.H`). Local face `i` of this<br>patch corresponds to local face `i` of the partner patch (OpenFOAM<br>half0/half1 ordering), so the two halves must have equal `size`. |

##### Implementations

###### Methods

- ```rust
  pub fn new</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, start: usize, size: usize, kind: PatchKind) -> Self { /* ... */ }
  ```
  Construct a patch spanning faces `[start, start + size)` of the global

- ```rust
  pub fn new_cyclic</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, start: usize, size: usize, partner_patch: usize) -> Self { /* ... */ }
  ```
  Construct a [`PatchKind::Cyclic`] (periodic) patch spanning faces

- ```rust
  pub fn end(self: &Self) -> usize { /* ... */ }
  ```
  Last+1 face index (exclusive upper bound).

- ```rust
  pub fn contains_face(self: &Self, f: usize) -> bool { /* ... */ }
  ```
  True if global face index `f` belongs to this patch.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> BoundaryPatch { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &BoundaryPatch) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `PatchKind`

Topological type of a boundary patch.

```rust
pub enum PatchKind {
    Patch,
    Wall,
    Symmetry,
    Empty,
    Wedge,
    Cyclic,
    CyclicAmi,
    Processor,
}
```

##### Variants

###### `Patch`

Generic boundary patch.

###### `Wall`

No-slip wall.

###### `Symmetry`

Symmetry plane.

###### `Empty`

2-D reduced case (zero-area faces).

###### `Wedge`

Axisymmetric wedge.

###### `Cyclic`

Periodic / matching pair (conformal — faces line up one-to-one).

###### `CyclicAmi`

Non-conformal periodic pair — arbitrary mesh interface (AMI). The two
halves' faces do not match one-to-one; each target face couples to a
weighted set of source faces (see [`AmiCoupling`]).
Mirrors OpenFOAM `cyclicAMIPolyPatch`.

###### `Processor`

Inter-processor decomposition seam.

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> PatchKind { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PatchKind) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `CyclicCoupling`

One across-the-seam cell coupling introduced by a [`PatchKind::Cyclic`]
(periodic) patch pair.

A cyclic patch pair makes the domain periodic: a boundary face on one half
of the pair is physically the *same* interface as the matching face on the
other half. This struct records, for one such matched face pair, the two
cells it joins so the FV operators can couple them **exactly like an internal
face** — the owner cell of the half0 face (`owner`) is coupled to the owner
cell of the half1 face (`neighbour`), contributing an off-diagonal matrix
entry across the periodic seam.

The couplings are appended to the LDU face addressing *after* the internal
faces (see [`FvMatrix::new`](crate::ldu_matrix::FvMatrix::new)), so coupling
index `i` in [`FvMesh::cyclic_couplings`] occupies LDU face
`n_internal_faces + i`.

Mirrors the coupled-interface contribution of `Foam::cyclicFvPatchField`
(`src/finiteVolume/.../cyclic/cyclicFvPatchField.H`), whose
`patchNeighbourField()` supplies the value from the partner cell.

```rust
pub struct CyclicCoupling {
    pub owner: usize,
    pub neighbour: usize,
    pub face_a: usize,
    pub face_b: usize,
    pub patch_a: usize,
    pub patch_b: usize,
    pub local: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `owner` | `usize` | Owner cell of the half0 (lower-patch-index) face — the "owner" side of<br>the coupling. |
| `neighbour` | `usize` | Owner cell of the matched half1 face — the "neighbour" across the seam. |
| `face_a` | `usize` | Global face index of the half0 face (on `patch_a`). |
| `face_b` | `usize` | Global face index of the matched half1 face (on `patch_b`). |
| `patch_a` | `usize` | Patch index of half0 (the lower of the pair's two indices). |
| `patch_b` | `usize` | Patch index of half1 (the partner of `patch_a`). |
| `local` | `usize` | Local face index within each half (`face_a - patches[patch_a].start ==<br>face_b - patches[patch_b].start`). |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> CyclicCoupling { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CyclicCoupling) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `FvMesh`

Finite-volume mesh — topology and geometry in a flat data structure.

Mirrors `Foam::fvMesh` (`src/finiteVolume/fvMesh/fvMesh.H`) but without
the C++ inheritance chain (`polyMesh → primitiveMesh → lduMesh`).
Only the data required by the FV operators is stored.

## Face ordering (OpenFOAM convention)
```text
[0 .. n_internal_faces)         ← internal faces (have both owner & neighbour)
[n_internal_faces .. n_faces)   ← boundary faces (owner only)
```
The `neighbour` array has length `n_internal_faces`; boundary faces have no
entry in `neighbour`.

```rust
pub struct FvMesh {
    pub n_cells: usize,
    pub n_internal_faces: usize,
    pub n_faces: usize,
    pub owner: Vec<usize>,
    pub neighbour: Vec<usize>,
    pub patches: Vec<BoundaryPatch>,
    pub cyclic_couplings: Vec<CyclicCoupling>,
    pub ami_couplings: Vec<crate::mesh::ami::AmiCoupling>,
    pub cell_volumes: Vec<f64>,
    pub cell_centres: Vec<crate::primitives::Vector3>,
    pub face_area_vectors: Vec<crate::primitives::Vector3>,
    pub face_areas: Vec<f64>,
    pub face_centres: Vec<crate::primitives::Vector3>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `n_cells` | `usize` | Number of cells. |
| `n_internal_faces` | `usize` | Number of internal faces (both owner and neighbour defined). |
| `n_faces` | `usize` | Total number of faces (internal + boundary). |
| `owner` | `Vec<usize>` | `owner[f]` — cell that owns face `f` (for all faces). |
| `neighbour` | `Vec<usize>` | `neighbour[f]` — cell on the other side of internal face `f`.<br>Length == `n_internal_faces`; boundary faces have no neighbour. |
| `patches` | `Vec<BoundaryPatch>` | Boundary patch descriptors (one per patch, in face-index order). |
| `cyclic_couplings` | `Vec<CyclicCoupling>` | Across-seam cell couplings from [`PatchKind::Cyclic`] (periodic) patch<br>pairs, one entry per matched boundary-face pair. Empty for a mesh with no<br>(resolved) cyclic pairs. Each entry is treated by the FV operators and<br>the LDU matrix exactly like an internal face joining<br>[`CyclicCoupling::owner`] and [`CyclicCoupling::neighbour`], appended to<br>the LDU face addressing after the `n_internal_faces` internal faces. |
| `ami_couplings` | `Vec<crate::mesh::ami::AmiCoupling>` | Across-seam couplings from [`PatchKind::CyclicAmi`] (non-conformal<br>periodic) patch pairs, one entry per **target** seam face. Empty for a<br>mesh with no AMI pairs. Each entry couples its target cell to a weighted<br>set of source cells (the geometric face overlaps); the FV operators and<br>the LDU matrix append one LDU face per [`AmiWeight`](crate::mesh::AmiWeight)<br>after the internal faces and the [`cyclic_couplings`](Self::cyclic_couplings)<br>(see [`ami_ldu_start`](Self::ami_ldu_start)). Mirrors OpenFOAM<br>`cyclicAMIFvPatchField`. |
| `cell_volumes` | `Vec<f64>` | Cell volumes `V[c]` `[m³]`. |
| `cell_centres` | `Vec<crate::primitives::Vector3>` | Cell centres `C[c]` `[m]`. |
| `face_area_vectors` | `Vec<crate::primitives::Vector3>` | Face area vectors `Sf[f]` `[m²]`, pointing from owner toward neighbour<br>(or outward for boundary faces). |
| `face_areas` | `Vec<f64>` | Face area magnitudes `|Sf[f]|` `[m²]`. |
| `face_centres` | `Vec<crate::primitives::Vector3>` | Face centres `Cf[f]` `[m]`. |

##### Implementations

###### Methods

- ```rust
  pub fn ami_ldu_start(self: &Self) -> usize { /* ... */ }
  ```
  First LDU face index occupied by AMI seam couplings.

- ```rust
  pub fn n_ami_faces(self: &Self) -> usize { /* ... */ }
  ```
  Total number of AMI partial-seam LDU faces — the sum of each AMI target

- ```rust
  pub fn periodic_ring_ami(n_a: usize, n_b: usize, lx: f64, ly: f64, depth: f64) -> FvMesh { /* ... */ }
  ```
  Build a **non-conformal periodic ring** with two `cyclicAMI` seams,

- ```rust
  pub fn n_boundary_faces(self: &Self) -> usize { /* ... */ }
  ```
  Total number of boundary faces.

- ```rust
  pub fn n_patches(self: &Self) -> usize { /* ... */ }
  ```
  Number of patches.

- ```rust
  pub fn is_internal_face(self: &Self, f: usize) -> bool { /* ... */ }
  ```
  True if face `f` is an internal face (has a neighbour cell).

- ```rust
  pub fn patch_for_face(self: &Self, f: usize) -> Option<(usize, usize)> { /* ... */ }
  ```
  Given a global face index `f` that is a boundary face, return

- ```rust
  pub fn n_cyclic_couplings(self: &Self) -> usize { /* ... */ }
  ```
  Number of across-seam cyclic couplings (length of

- ```rust
  pub fn cyclic_coupling_face(self: &Self, i: usize) -> usize { /* ... */ }
  ```
  LDU face index of cyclic coupling `i`.

- ```rust
  pub fn cyclic_partner_face(self: &Self, global_face: usize) -> Option<usize> { /* ... */ }
  ```
  Given a global boundary face index on a resolved [`PatchKind::Cyclic`]

- ```rust
  pub fn periodic_1d(n: usize, length: f64, area: f64) -> FvMesh { /* ... */ }
  ```
  Build a uniform 1-D **periodic** (cyclic) mesh: `n` equal cells along the

- ```rust
  pub fn validate(self: &Self) -> Result<(), MeshError> { /* ... */ }
  ```
  Validate basic mesh consistency.  Returns `Err` on the first problem found.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> FvMesh { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `FvMeshBuilder`

Builder for `FvMesh` — lets tests and I/O code assemble a mesh incrementally.

```rust
pub struct FvMeshBuilder {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```
  New empty builder (all arrays empty, all counts zero).

- ```rust
  pub fn n_cells(self: Self, n: usize) -> Self { /* ... */ }
  ```
  Set the number of cells.

- ```rust
  pub fn n_internal_faces(self: Self, n: usize) -> Self { /* ... */ }
  ```
  Set the number of internal faces (faces with both owner and neighbour).

- ```rust
  pub fn owner(self: Self, v: Vec<usize>) -> Self { /* ... */ }
  ```
  Set the `owner` array (owning cell per face; length == `n_faces`).

- ```rust
  pub fn neighbour(self: Self, v: Vec<usize>) -> Self { /* ... */ }
  ```
  Set the `neighbour` array (neighbour cell per internal face; length ==

- ```rust
  pub fn patches(self: Self, v: Vec<BoundaryPatch>) -> Self { /* ... */ }
  ```
  Set the boundary patch descriptors.

- ```rust
  pub fn ami_couplings(self: Self, v: Vec<AmiCoupling>) -> Self { /* ... */ }
  ```
  Set the non-conformal-periodic (AMI) seam couplings (one entry per target

- ```rust
  pub fn cell_volumes(self: Self, v: Vec<f64>) -> Self { /* ... */ }
  ```
  Set the cell volumes `V[c]` `[m³]` (length == `n_cells`).

- ```rust
  pub fn cell_centres(self: Self, v: Vec<Vector3>) -> Self { /* ... */ }
  ```
  Set the cell centres `C[c]` `[m]` (length == `n_cells`).

- ```rust
  pub fn face_area_vectors(self: Self, v: Vec<Vector3>) -> Self { /* ... */ }
  ```
  Set the face area vectors `Sf[f]` `[m²]` (length == `n_faces`).

- ```rust
  pub fn face_areas(self: Self, v: Vec<f64>) -> Self { /* ... */ }
  ```
  Set the face area magnitudes `|Sf[f]|` `[m²]`. If left unset, they are

- ```rust
  pub fn face_centres(self: Self, v: Vec<Vector3>) -> Self { /* ... */ }
  ```
  Set the face centres `Cf[f]` `[m]` (length == `n_faces`).

- ```rust
  pub fn build(self: Self) -> Result<FvMesh, MeshError> { /* ... */ }
  ```
  Finalise the mesh: derive `face_areas` if needed, resolve any cyclic

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> FvMeshBuilder { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `region_interface`

```rust
pub mod region_interface { /* ... */ }
```

### Types

#### Struct `RegionInterface`

Face-to-face mapping between two mesh patches at a shared interface.

Used by `chtMultiRegionFoam`-style solvers where a fluid region and a
solid region share an interface.  Each side has a patch (identified by
mesh + patch index); the `face_map` gives the paired face index on side B
for each face on side A.

For matching meshes (same layout, same face count) `face_map[i] = i`.
For non-matching meshes (different refinements) the map is built by
nearest-face-centre search (see `from_face_centres`).

```rust
pub struct RegionInterface {
    pub mesh_a: std::sync::Arc<crate::mesh::fv_mesh::FvMesh>,
    pub patch_a: usize,
    pub mesh_b: std::sync::Arc<crate::mesh::fv_mesh::FvMesh>,
    pub patch_b: usize,
    pub face_map: Vec<usize>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh_a` | `std::sync::Arc<crate::mesh::fv_mesh::FvMesh>` | Mesh on side A of the interface. |
| `patch_a` | `usize` | Index of the coupled patch within `mesh_a.patches`. |
| `mesh_b` | `std::sync::Arc<crate::mesh::fv_mesh::FvMesh>` | Mesh on side B of the interface. |
| `patch_b` | `usize` | Index of the coupled patch within `mesh_b.patches`. |
| `face_map` | `Vec<usize>` | `face_map[fi_a]` = `fi_b` on the B-side patch. |

##### Implementations

###### Methods

- ```rust
  pub fn matching(mesh_a: Arc<FvMesh>, patch_a: usize, mesh_b: Arc<FvMesh>, patch_b: usize) -> Self { /* ... */ }
  ```
  Construct a matching interface: face `i` on A is coupled to face `i` on B.

- ```rust
  pub fn from_face_centres(mesh_a: Arc<FvMesh>, patch_a: usize, mesh_b: Arc<FvMesh>, patch_b: usize) -> Self { /* ... */ }
  ```
  Construct a non-matching interface via nearest-face-centre search.

- ```rust
  pub fn exchange_temperature(self: &Self, t_a: &VolScalarField, t_b: &VolScalarField) -> (PatchField<f64>, PatchField<f64>) { /* ... */ }
  ```
  Exchange temperature boundary values at the interface.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> RegionInterface { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Re-exports

#### Re-export `AmiCoupling`

```rust
pub use ami::AmiCoupling;
```

#### Re-export `AmiOverlap`

```rust
pub use ami::AmiOverlap;
```

#### Re-export `AmiWeight`

```rust
pub use ami::AmiWeight;
```

#### Re-export `MeshError`

```rust
pub use error::MeshError;
```

#### Re-export `RegionInterface`

```rust
pub use region_interface::RegionInterface;
```

#### Re-export `fv_mesh::*`

```rust
pub use fv_mesh::*;
```

## Module `ode`

Layer 1e — ordinary-differential-equation solvers (Euler, RKF45,
Rosenbrock23).
Ordinary differential equation solvers for systems `dy/dx = f(x, y)`.

Ports the OpenFOAM `ODE` layer: user systems implement the [`OdeSystem`](crate::ode::OdeSystem)
trait, and one of the concrete steppers integrates them with adaptive step
control — [`Euler`](crate::ode::euler::Euler) (explicit 1st order),
[`Rkf45`](crate::ode::rkf45::Rkf45) (explicit Runge-Kutta-Fehlberg 4(5)), and
[`Rosenbrock23`](crate::ode::rosenbrock23::Rosenbrock23) (semi-implicit, for stiff systems,
requiring a Jacobian). The independent variable `x`, state `y`, and step
size are bare `f64` in the caller's own units; tolerances are set through
[`OdeSolverConfig`](crate::ode::OdeSolverConfig).

# Storing an integrator: [`OdeIntegrator`](crate::ode::integrator::OdeIntegrator)

The three steppers above take the system by reference on every call, which
is awkward for any caller that wants to *keep* "the integrator for this
material point" as a struct field — storing a borrow would force a lifetime
parameter, which the workspace design rules forbid.

[`integrator`](crate::ode::integrator) solves that with two enums that own
what they integrate: [`OdeSolver`](crate::ode::integrator::OdeSolver) selects
the stepper, and
[`OdeIntegrator`](crate::ode::integrator::OdeIntegrator) selects how the
system is supplied —
[`OdeIntegrator::TypedState`](crate::ode::integrator::OdeIntegrator::TypedState)
(a concrete system owned
by value, statically dispatched, **preferred**) or
[`OdeIntegrator::DynSystem`](crate::ode::integrator::OdeIntegrator::DynSystem)
(an `Arc<dyn OdeSystem + Send + Sync>`, kept by
maintainer decision for flexibility). Neither borrows, so neither needs a
lifetime.

```rust
pub mod ode { /* ... */ }
```

### Modules

## Module `euler`

```rust
pub mod euler { /* ... */ }
```

### Types

#### Struct `Euler`

Explicit first-order Euler solver with adaptive step size.
Maps to `Foam::Euler` (which inherits from `adaptiveSolver`).

```rust
pub struct Euler {
    pub config: super::OdeSolverConfig,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `config` | `super::OdeSolverConfig` | Adaptive step-size controller settings (tolerances, scale limits). |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(n: usize, abs_tol: f64, rel_tol: f64) -> Self { /* ... */ }
  ```
  Create a solver for an `n`-equation system with the given absolute and

- ```rust
  pub fn solve_step<Sys: OdeSystem + ?Sized>(self: &mut Self, ode: &Sys, x: &mut f64, y: &mut Vec<f64>, dx_try: &mut f64) -> Result<(), OdeError> { /* ... */ }
  ```
  Take one adaptive step. On return `x` and `y` are updated and

- ```rust
  pub fn integrate<Sys: OdeSystem + ?Sized>(self: &mut Self, ode: &Sys, x_start: f64, x_end: f64, y: &mut Vec<f64>, dx_est: &mut f64) -> Result<(), OdeError> { /* ... */ }
  ```
  Integrate from `x_start` to `x_end`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Euler { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `integrator`

Enum-dispatched ODE integration — solver choice *and* system ownership.

# What this adds over the bare steppers

[`Euler`], [`Rkf45`] and [`Rosenbrock23`] each integrate a system you hand
them by reference on every call. That is fine inside one function, but a
constitutive law or a solver loop usually wants to *store* "the integrator
for this material point" — solver plus system together — and step it later.
Storing a borrow would require a lifetime parameter on the storing struct,
which the workspace design rules forbid outright.

This module removes the need for one. Two enums, no lifetimes anywhere:

- [`OdeSolver`] — *which* stepper. A closed set (`Euler`, `Rkf45`,
  `Rosenbrock23`), so the scheme can be chosen at run time without a trait
  object and without heap allocation. Adding a stepper forces every `match`
  site to be updated.
- [`OdeIntegrator`] — *how the system is supplied*. Two variants, and they
  are the whole point of this module:
  - [`OdeIntegrator::TypedState`] owns a concrete, statically-known system
    **by value**. Derivative calls are statically dispatched and inlinable.
    **This is the preferred variant.**
  - [`OdeIntegrator::DynSystem`] holds the system behind
    [`SharedOdeSystem`] (`Arc<dyn OdeSystem + Send + Sync>`). It exists **by
    maintainer decision, for flexibility**, so a caller that genuinely does
    not know the system type at the call site — a registry, a case-file
    reader, a test harness sweeping several systems — has a path that does
    not require inventing an enum of its own.

# Why there is no lifetime parameter

Both variants **own** their system: `S` by value, or shared ownership
through `Arc`. Nothing here borrows the system, so nothing here needs to
name the region the borrow is valid for. An [`OdeIntegrator`] can therefore
be stored in a struct, moved between threads, or held in a `Vec` for one
integrator per material point, with no lifetime plumbing at any call site.

# On the `dyn` in [`OdeIntegrator::DynSystem`]

The workspace rule is *"no trait objects for dispatch — use enums"*, and the
dispatch here **is** the enum: [`OdeIntegrator`] chooses between two owning
strategies by `match`, exhaustively. The `Arc<dyn OdeSystem + Send + Sync>`
inside one of its variants is a boundary coercion for callers who ask for
it, kept deliberately, not an accident to be cleaned up later. Prefer
[`OdeIntegrator::TypedState`]; reach for [`OdeIntegrator::DynSystem`] when
the type genuinely is not known statically.

# Example — the preferred typed path

```rust
use outram_foam_basic_lib::ode::{OdeIntegrator, OdeSolver, OdeSystem};

struct Decay;
impl OdeSystem for Decay {
    fn n_eqns(&self) -> usize { 1 }
    fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
        dydx[0] = -y[0];
    }
}

// Owned by value — no borrow, no lifetime, storable in any struct.
let mut integrator = OdeIntegrator::typed(Decay, OdeSolver::rkf45(1, 1e-10, 1e-8));
let mut y = vec![1.0_f64];
let mut dx = 0.1;
integrator.integrate(0.0, 1.0, &mut y, &mut dx).unwrap();
assert!((y[0] - (-1.0_f64).exp()).abs() < 1e-8);
```

# Example — the shared `dyn` path

```rust
use std::sync::Arc;
use outram_foam_basic_lib::ode::{OdeIntegrator, OdeSolver, OdeSystem, SharedOdeSystem};

struct Decay;
impl OdeSystem for Decay {
    fn n_eqns(&self) -> usize { 1 }
    fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
        dydx[0] = -y[0];
    }
}

let shared: SharedOdeSystem = Arc::new(Decay);
let mut integrator = OdeIntegrator::shared(shared, OdeSolver::rkf45(1, 1e-10, 1e-8));
let mut y = vec![1.0_f64];
let mut dx = 0.1;
integrator.integrate(0.0, 1.0, &mut y, &mut dx).unwrap();
assert!((y[0] - (-1.0_f64).exp()).abs() < 1e-8);
```

```rust
pub mod integrator { /* ... */ }
```

### Types

#### Type Alias `SharedOdeSystem`

A shared, runtime-typed ODE system.

`Send + Sync` is required because the only reason to reach for `Arc` over
owning the system by value is to share it — including across threads, which
is how this workspace shares simulation state (`Arc<T>` for read-only data).
A system that cannot cross a thread boundary should use
[`OdeIntegrator::TypedState`] instead.

```rust
pub type SharedOdeSystem = std::sync::Arc<dyn OdeSystem + Send + Sync>;
```

#### Enum `OdeSolver`

Which stepper integrates the system — enum dispatch over the closed set of
solvers this crate ports.

The steppers carry per-equation scratch buffers sized at construction, so a
solver built for `n` equations must only be used with an `n`-equation
system. All variants integrate a bare `f64` state vector in the caller's own
units; only [`OdeSolverConfig`] tolerances are interpreted here.

Cloning a solver clones its scratch buffers; the clone integrates
independently of the original.

```rust
pub enum OdeSolver {
    Euler(super::Euler),
    Rkf45(super::Rkf45),
    Rosenbrock23(super::Rosenbrock23),
}
```

##### Variants

###### `Euler`

Explicit first-order Euler with adaptive step size. Cheapest per step,
but the global error falls only linearly in the step size — use it for
smooth, non-stiff systems where accuracy is not critical.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `super::Euler` |  |

###### `Rkf45`

Explicit Runge-Kutta-Fehlberg 4(5), the general-purpose default for
non-stiff systems. Six derivative evaluations per step, fifth-order
propagation with an embedded fourth-order error estimate.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `super::Rkf45` |  |

###### `Rosenbrock23`

Semi-implicit W-method Rosenbrock23 for **stiff** systems. Requires the
system to implement [`OdeSystem::jacobian`]; the default `jacobian`
panics, so check [`OdeSolver::requires_jacobian`] before selecting it
for a system you did not write.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `super::Rosenbrock23` |  |

##### Implementations

###### Methods

- ```rust
  pub fn euler(n: usize, abs_tol: f64, rel_tol: f64) -> Self { /* ... */ }
  ```
  Explicit Euler for an `n`-equation system.

- ```rust
  pub fn rkf45(n: usize, abs_tol: f64, rel_tol: f64) -> Self { /* ... */ }
  ```
  Runge-Kutta-Fehlberg 4(5) for an `n`-equation system. See

- ```rust
  pub fn rosenbrock23(n: usize, abs_tol: f64, rel_tol: f64) -> Self { /* ... */ }
  ```
  Stiff Rosenbrock23 for an `n`-equation system. See [`OdeSolver::euler`]

- ```rust
  pub const fn name(self: &Self) -> &'static str { /* ... */ }
  ```
  The stepper's name, for diagnostics and log lines.

- ```rust
  pub const fn requires_jacobian(self: &Self) -> bool { /* ... */ }
  ```
  Whether this stepper calls [`OdeSystem::jacobian`], whose default

- ```rust
  pub fn config(self: &Self) -> &OdeSolverConfig { /* ... */ }
  ```
  The adaptive step-size controller settings in force.

- ```rust
  pub fn config_mut(self: &mut Self) -> &mut OdeSolverConfig { /* ... */ }
  ```
  Mutable access to the controller settings, e.g. to lower `max_steps`.

- ```rust
  pub fn solve_step<Sys: OdeSystem + ?Sized>(self: &mut Self, ode: &Sys, x: &mut f64, y: &mut Vec<f64>, dx_try: &mut f64) -> Result<(), OdeError> { /* ... */ }
  ```
  Take one adaptive step of `ode`. On return `x` and `y` are advanced and

- ```rust
  pub fn integrate<Sys: OdeSystem + ?Sized>(self: &mut Self, ode: &Sys, x_start: f64, x_end: f64, y: &mut Vec<f64>, dx_est: &mut f64) -> Result<(), OdeError> { /* ... */ }
  ```
  Integrate `ode` from `x_start` to `x_end`, updating `y` in place and

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> OdeSolver { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `NoTypedSystem`

The zero-equation system, and the default type argument of
[`OdeIntegrator`].

[`OdeIntegrator::DynSystem`] does not use the `S` type parameter, but Rust
still requires one to be named. `NoTypedSystem` is that name: writing
`OdeIntegrator` with no type argument means "the `dyn` variant is the only
one I intend to use".

It is a genuine, well-defined system rather than a panicking stub — a system
of zero equations, whose derivative vector is empty — so nothing goes wrong
if one is integrated by accident. Integrating it is simply a no-op on an
empty state.

```rust
pub struct NoTypedSystem;
```

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> NoTypedSystem { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> NoTypedSystem { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **OdeSystem**
  - ```rust
    fn n_eqns(self: &Self) -> usize { /* ... */ }
    ```

  - ```rust
    fn derivatives(self: &Self, _x: f64, _y: &[f64], _dydx: &mut Vec<f64>) { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &NoTypedSystem) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `TypedStateIntegrator`

Solver plus a **statically-typed, owned** system — the preferred variant of
[`OdeIntegrator`].

The system is stored by value, so derivative evaluation is a direct,
inlinable call with no vtable and no borrow to outlive. `S` may be any
concrete type implementing [`OdeSystem`], including the caller's own enum
over several systems.

```rust
pub struct TypedStateIntegrator<S: OdeSystem> {
    pub solver: OdeSolver,
    pub system: S,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `solver` | `OdeSolver` | Which stepper advances the state. |
| `system` | `S` | The system being integrated, owned outright. |

##### Implementations

###### Methods

- ```rust
  pub fn new(system: S, solver: OdeSolver) -> Self { /* ... */ }
  ```
  Pair an owned system with a solver.

- ```rust
  pub fn solve_step(self: &mut Self, x: &mut f64, y: &mut Vec<f64>, dx_try: &mut f64) -> Result<(), OdeError> { /* ... */ }
  ```
  Take one adaptive step. See [`OdeSolver::solve_step`].

- ```rust
  pub fn integrate(self: &mut Self, x_start: f64, x_end: f64, y: &mut Vec<f64>, dx_est: &mut f64) -> Result<(), OdeError> { /* ... */ }
  ```
  Integrate from `x_start` to `x_end`. See [`OdeSolver::integrate`].

- ```rust
  pub fn into_system(self: Self) -> S { /* ... */ }
  ```
  Consume the integrator and return the system, e.g. to read state the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `DynSystemIntegrator`

Solver plus a **shared, runtime-typed** system.

The flexibility variant, kept by maintainer decision: the system type need
not be known where the integrator is built, and the same system can be
shared by several integrators. Prefer [`TypedStateIntegrator`] when the type
*is* known — it dispatches statically.

```rust
pub struct DynSystemIntegrator {
    pub solver: OdeSolver,
    pub system: SharedOdeSystem,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `solver` | `OdeSolver` | Which stepper advances the state. |
| `system` | `SharedOdeSystem` | The system being integrated, shared by `Arc`. Cloning the integrator<br>shares the system rather than duplicating it. |

##### Implementations

###### Methods

- ```rust
  pub fn new(system: SharedOdeSystem, solver: OdeSolver) -> Self { /* ... */ }
  ```
  Pair a shared system with a solver.

- ```rust
  pub fn solve_step(self: &mut Self, x: &mut f64, y: &mut Vec<f64>, dx_try: &mut f64) -> Result<(), OdeError> { /* ... */ }
  ```
  Take one adaptive step. See [`OdeSolver::solve_step`].

- ```rust
  pub fn integrate(self: &mut Self, x_start: f64, x_end: f64, y: &mut Vec<f64>, dx_est: &mut f64) -> Result<(), OdeError> { /* ... */ }
  ```
  Integrate from `x_start` to `x_end`. See [`OdeSolver::integrate`].

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> DynSystemIntegrator { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut std::fmt::Formatter<''_>) -> std::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `OdeIntegrator`

Enum-dispatch wrapper over the two ways of owning an ODE system.

This is the type to store when a struct needs "an integrator" as a field.
Neither variant borrows, so no lifetime parameter propagates outward — the
reason this wrapper exists.

The type argument `S` names the concrete system used by
[`TypedState`](Self::TypedState). It defaults to [`NoTypedSystem`], so an
integrator that only ever uses [`DynSystem`](Self::DynSystem) can be written
as a plain `OdeIntegrator`.

# Choosing a variant

| | [`TypedState`](Self::TypedState) | [`DynSystem`](Self::DynSystem) |
|---|---|---|
| System known at compile time | yes | no |
| Dispatch | static, inlinable | vtable |
| Ownership | by value | shared, `Arc` |
| Use when | normal case — **prefer this** | the type is chosen at run time |

```rust
pub enum OdeIntegrator<S: OdeSystem = NoTypedSystem> {
    TypedState(TypedStateIntegrator<S>),
    DynSystem(DynSystemIntegrator),
}
```

##### Variants

###### `TypedState`

The typed-state integrator: a concrete system owned by value, with
static dispatch. **Preferred.**

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `TypedStateIntegrator<S>` |  |

###### `DynSystem`

The `dyn`-system integrator: `Arc<dyn OdeSystem + Send + Sync>`. Kept by
maintainer decision, for flexibility where the system type is not
statically known.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `DynSystemIntegrator` |  |

##### Implementations

###### Methods

- ```rust
  pub fn typed(system: S, solver: OdeSolver) -> Self { /* ... */ }
  ```
  Build the preferred, statically-typed integrator from an owned system.

- ```rust
  pub fn n_eqns(self: &Self) -> usize { /* ... */ }
  ```
  Number of coupled equations the stored system reports.

- ```rust
  pub fn solver(self: &Self) -> &OdeSolver { /* ... */ }
  ```
  The stepper in use.

- ```rust
  pub fn solver_mut(self: &mut Self) -> &mut OdeSolver { /* ... */ }
  ```
  Mutable access to the stepper, e.g. to adjust tolerances between steps.

- ```rust
  pub const fn is_typed_state(self: &Self) -> bool { /* ... */ }
  ```
  `true` for the preferred, statically-dispatched variant.

- ```rust
  pub fn typed_system(self: &Self) -> Option<&S> { /* ... */ }
  ```
  The owned system, when this is the typed variant; `None` otherwise.

- ```rust
  pub fn shared_system(self: &Self) -> Option<&SharedOdeSystem> { /* ... */ }
  ```
  The shared system, when this is the `dyn` variant; `None` otherwise.

- ```rust
  pub fn solve_step(self: &mut Self, x: &mut f64, y: &mut Vec<f64>, dx_try: &mut f64) -> Result<(), OdeError> { /* ... */ }
  ```
  Take one adaptive step. `x` is the independent variable, `y` the state

- ```rust
  pub fn integrate(self: &mut Self, x_start: f64, x_end: f64, y: &mut Vec<f64>, dx_est: &mut f64) -> Result<(), OdeError> { /* ... */ }
  ```
  Integrate from `x_start` to `x_end`, updating `y` in place and leaving

- ```rust
  pub fn shared(system: SharedOdeSystem, solver: OdeSolver) -> Self { /* ... */ }
  ```
  Build the shared, runtime-typed integrator.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `rkf45`

```rust
pub mod rkf45 { /* ... */ }
```

### Types

#### Struct `Rkf45`

Runge-Kutta-Fehlberg 4(5) explicit solver with adaptive step size.
Maps to `Foam::RKF45`.

```rust
pub struct Rkf45 {
    pub config: super::OdeSolverConfig,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `config` | `super::OdeSolverConfig` | Adaptive step-size controller settings (tolerances, scale limits). |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(n: usize, abs_tol: f64, rel_tol: f64) -> Self { /* ... */ }
  ```
  Create a solver for an `n`-equation system with the given absolute and

- ```rust
  pub fn solve_step<Sys: OdeSystem + ?Sized>(self: &mut Self, ode: &Sys, x: &mut f64, y: &mut Vec<f64>, dx_try: &mut f64) -> Result<(), OdeError> { /* ... */ }
  ```
  Take one adaptive step. On return `x` and `y` are updated and `dx_try`

- ```rust
  pub fn integrate<Sys: OdeSystem + ?Sized>(self: &mut Self, ode: &Sys, x_start: f64, x_end: f64, y: &mut Vec<f64>, dx_est: &mut f64) -> Result<(), OdeError> { /* ... */ }
  ```
  Integrate from `x_start` to `x_end`, updating `y` in place and leaving

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Rkf45 { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `rosenbrock23`

```rust
pub mod rosenbrock23 { /* ... */ }
```

### Types

#### Struct `Rosenbrock23`

W-method Rosenbrock23 stiff solver with adaptive step size.

Requires the user's `OdeSystem::jacobian` to be implemented.
Maps to `Foam::Rosenbrock23`.

```rust
pub struct Rosenbrock23 {
    pub config: super::OdeSolverConfig,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `config` | `super::OdeSolverConfig` | Adaptive step-size controller settings (tolerances, scale limits). |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(n: usize, abs_tol: f64, rel_tol: f64) -> Self { /* ... */ }
  ```
  Create a stiff solver for an `n`-equation system with the given absolute

- ```rust
  pub fn solve_step<Sys: OdeSystem + ?Sized>(self: &mut Self, ode: &Sys, x: &mut f64, y: &mut Vec<f64>, dx_try: &mut f64) -> Result<(), OdeError> { /* ... */ }
  ```
  One adaptive step (retries with smaller dx if error > 1).

- ```rust
  pub fn integrate<Sys: OdeSystem + ?Sized>(self: &mut Self, ode: &Sys, x_start: f64, x_end: f64, y: &mut Vec<f64>, dx_est: &mut f64) -> Result<(), OdeError> { /* ... */ }
  ```
  Integrate from `x_start` to `x_end`, updating `y` in place and leaving

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Rosenbrock23 { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Types

#### Struct `OdeSolverConfig`

Common parameters for the adaptive step-size controller.
Defaults match `Foam::adaptiveSolver` and `Foam::ODESolver`.

```rust
pub struct OdeSolverConfig {
    pub abs_tol: f64,
    pub rel_tol: f64,
    pub safe_scale: f64,
    pub alpha_inc: f64,
    pub alpha_dec: f64,
    pub min_scale: f64,
    pub max_scale: f64,
    pub max_steps: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `abs_tol` | `f64` | Absolute per-equation tolerance. |
| `rel_tol` | `f64` | Relative per-equation tolerance. |
| `safe_scale` | `f64` | Safety factor on the step-size scaling (0 < safeScale < 1). |
| `alpha_inc` | `f64` | Exponent for step *increase*. |
| `alpha_dec` | `f64` | Exponent for step *decrease*. |
| `min_scale` | `f64` | Minimum scale factor applied per step. |
| `max_scale` | `f64` | Maximum scale factor applied per step. |
| `max_steps` | `usize` | Maximum sub-steps for one `integrate()` call. |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> OdeSolverConfig { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `OdeError`

Failure modes of an adaptive integration.

```rust
pub enum OdeError {
    StepSizeUnderflow,
    MaxStepsExceeded(usize),
}
```

##### Variants

###### `StepSizeUnderflow`

The step size shrank below machine epsilon while trying to meet the
error tolerance — the system is too stiff for the chosen solver, or the
tolerances are unattainable.

###### `MaxStepsExceeded`

The interval could not be spanned within `max_steps` sub-steps; carries
the number of steps taken.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> OdeError { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, f: &mut std::fmt::Formatter<''_>) -> std::fmt::Result { /* ... */ }
    ```

- **Error**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &OdeError) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Traits

#### Trait `OdeSystem`

Abstract ODE system `dy/dx = f(x, y)`. Maps to `Foam::ODESystem`.

```rust
pub trait OdeSystem {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `n_eqns`: Number of coupled equations (the length of the state vector `y`).
- `derivatives`: Fill `dydx` with the derivatives at `(x, y)`.

##### Provided Methods

- ```rust
  fn jacobian(self: &Self, _x: f64, _y: &[f64], _dfdx: &mut Vec<f64>, _dfdy: &mut SquareMatrix) { /* ... */ }
  ```
  Fill `dfdx` and `dfdy` with the Jacobian at `(x, y)`.

##### Implementations

This trait is implemented for the following types:

- `NoTypedSystem`

### Re-exports

#### Re-export `Euler`

```rust
pub use euler::Euler;
```

#### Re-export `DynSystemIntegrator`

```rust
pub use integrator::DynSystemIntegrator;
```

#### Re-export `NoTypedSystem`

```rust
pub use integrator::NoTypedSystem;
```

#### Re-export `OdeIntegrator`

```rust
pub use integrator::OdeIntegrator;
```

#### Re-export `OdeSolver`

```rust
pub use integrator::OdeSolver;
```

#### Re-export `SharedOdeSystem`

```rust
pub use integrator::SharedOdeSystem;
```

#### Re-export `TypedStateIntegrator`

```rust
pub use integrator::TypedStateIntegrator;
```

#### Re-export `Rkf45`

```rust
pub use rkf45::Rkf45;
```

#### Re-export `Rosenbrock23`

```rust
pub use rosenbrock23::Rosenbrock23;
```

## Module `polynomial`

Layers 1c/1d — polynomial evaluation and closed-form equation solvers
(linear, quadratic, cubic).
Closed-form polynomial equation solvers and a fixed-degree polynomial type.

Ports the OpenFOAM `primitives/polynomialEqns` layer: the linear, quadratic,
and cubic root finders (`LinearEqn`, `QuadraticEqn`, `CubicEqn`) that return a
tagged [`Roots`](crate::polynomial::roots::Roots) container distinguishing real, complex, infinite, and NaN
roots, plus the general [`Polynomial<N>`](polynomial::Polynomial) value /
derivative / integral type. All coefficients and results are bare `f64` in SI
(dimensionless) form — these are numerical building blocks, not dimensioned
physical quantities.

```rust
pub mod polynomial { /* ... */ }
```

### Modules

## Module `cubic_eqn`

```rust
pub mod cubic_eqn { /* ... */ }
```

### Types

#### Struct `CubicEqn`

Solves `a·x³ + b·x² + c·x + d = 0`. Maps to `Foam::cubicEqn`.

The root-finding algorithm uses the depressed-cubic Cardano method with
Kahan-compensated intermediate discriminants for numerical robustness.
Reference: JLM = Numerical Recipes §3, with adjustments from the OpenFOAM
implementation.

```rust
pub struct CubicEqn {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `f64` |  |
| `b` | `f64` |  |
| `c` | `f64` |  |
| `d` | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn new(a: f64, b: f64, c: f64, d: f64) -> Self { /* ... */ }
  ```
  Construct `a·x³ + b·x² + c·x + d` from its four `f64` coefficients.

- ```rust
  pub fn value(self: &Self, x: f64) -> f64 { /* ... */ }
  ```
  Evaluate `a·x³ + b·x² + c·x + d` (Horner form).

- ```rust
  pub fn derivative(self: &Self, x: f64) -> f64 { /* ... */ }
  ```
  Derivative `3a·x² + 2b·x + c` (Horner form).

- ```rust
  pub fn error(self: &Self, x: f64) -> f64 { /* ... */ }
  ```
  Floating-point error estimate at `x`.

- ```rust
  pub fn roots(self: &Self) -> Roots<3> { /* ... */ }
  ```
  Roots of `a·x³ + b·x² + c·x + d = 0`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> CubicEqn { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CubicEqn) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `linear_eqn`

```rust
pub mod linear_eqn { /* ... */ }
```

### Types

#### Struct `LinearEqn`

Solves `a·x + b = 0`. Maps to `Foam::linearEqn`.

```rust
pub struct LinearEqn {
    pub a: f64,
    pub b: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `f64` |  |
| `b` | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn new(a: f64, b: f64) -> Self { /* ... */ }
  ```
  Construct `a·x + b` from its two `f64` coefficients.

- ```rust
  pub fn value(self: &Self, x: f64) -> f64 { /* ... */ }
  ```
  Evaluate `a·x + b`.

- ```rust
  pub fn derivative(self: &Self, _x: f64) -> f64 { /* ... */ }
  ```
  Derivative = `a` (constant).

- ```rust
  pub fn error(self: &Self, x: f64) -> f64 { /* ... */ }
  ```
  Floating-point error estimate at `x`.

- ```rust
  pub fn roots(self: &Self) -> Roots<1> { /* ... */ }
  ```
  Return the single root of `a·x + b = 0`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> LinearEqn { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &LinearEqn) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `polynomial`

```rust
pub mod polynomial { /* ... */ }
```

### Types

#### Struct `Polynomial`

Fixed-degree polynomial with an optional log term.

Represents `sum(coeffs[i] · xⁱ, i=0..N-1) + log_coeff · ln(x)`.

Maps to `Foam::Polynomial<N>` (`Polynomial.H`, `Polynomial.C`).

The log term is activated only via `integral_minus1`, which models
integrals of polynomials whose lowest-order term is `coeffs[0] · x⁻¹`.
The `integral() -> Polynomial<{N+1}>` form (returning one higher degree)
is not implemented because it requires `generic_const_exprs` (nightly);
use the scalar `integral(x1, x2) -> f64` form instead.

```rust
pub struct Polynomial<const N: usize> {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(coeffs: [f64; N]) -> Self { /* ... */ }
  ```
  Construct from coefficient array (constant term first).

- ```rust
  pub fn coeffs(self: &Self) -> &[f64; N] { /* ... */ }
  ```
  The `N` polynomial coefficients, constant term (`x⁰`) first.

- ```rust
  pub fn log_coeff(self: &Self) -> f64 { /* ... */ }
  ```
  Coefficient of the `ln(x)` term (zero unless the log term is active).

- ```rust
  pub fn log_active(self: &Self) -> bool { /* ... */ }
  ```
  Whether the `log_coeff · ln(x)` term contributes to `value`/`derivative`.

- ```rust
  pub fn value(self: &Self, x: f64) -> f64 { /* ... */ }
  ```
  Evaluate the polynomial at `x` (Horner-like accumulation, matching C++).

- ```rust
  pub fn derivative(self: &Self, x: f64) -> f64 { /* ... */ }
  ```
  Derivative of the polynomial at `x`.

- ```rust
  pub fn integral(self: &Self, x1: f64, x2: f64) -> f64 { /* ... */ }
  ```
  Definite integral from `x1` to `x2`.

- ```rust
  pub fn integral_minus1(self: &Self, int_constant: f64) -> Self { /* ... */ }
  ```
  Integrate a polynomial whose base starts at order −1.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Polynomial<N> { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `quadratic_eqn`

```rust
pub mod quadratic_eqn { /* ... */ }
```

### Types

#### Struct `QuadraticEqn`

Solves `a·x² + b·x + c = 0`. Maps to `Foam::quadraticEqn`.

```rust
pub struct QuadraticEqn {
    pub a: f64,
    pub b: f64,
    pub c: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `f64` |  |
| `b` | `f64` |  |
| `c` | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn new(a: f64, b: f64, c: f64) -> Self { /* ... */ }
  ```
  Construct `a·x² + b·x + c` from its three `f64` coefficients.

- ```rust
  pub fn value(self: &Self, x: f64) -> f64 { /* ... */ }
  ```
  Evaluate `a·x² + b·x + c` (Horner form).

- ```rust
  pub fn derivative(self: &Self, x: f64) -> f64 { /* ... */ }
  ```
  Derivative `2a·x + b`.

- ```rust
  pub fn error(self: &Self, x: f64) -> f64 { /* ... */ }
  ```
  Floating-point error estimate at `x`.

- ```rust
  pub fn roots(self: &Self) -> Roots<2> { /* ... */ }
  ```
  Roots of `a·x² + b·x + c = 0`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> QuadraticEqn { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &QuadraticEqn) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `roots`

```rust
pub mod roots { /* ... */ }
```

### Types

#### Enum `RootType`

**Attributes:**

- `Repr(AttributeRepr { kind: Rust, align: None, packed: None, int: Some("u64") })`

Root classification tag, matching `Foam::roots::type`.

```rust
pub enum RootType {
    Real = 0,
    Complex = 1,
    PosInf = 2,
    NegInf = 3,
    Nan = 4,
}
```

##### Variants

###### `Real`

Discriminant: `0`

Discriminant value: `0`

###### `Complex`

Discriminant: `1`

Discriminant value: `1`

###### `PosInf`

Discriminant: `2`

Discriminant value: `2`

###### `NegInf`

Discriminant: `3`

Discriminant value: `3`

###### `Nan`

Discriminant: `4`

Discriminant value: `4`

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> RootType { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Eq**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &RootType) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Roots`

Tagged root container for N roots.
Types are packed 3 bits per root into a u64, identical to C++ `Roots<N>`.
For complex conjugate pairs, slot i holds the real part and slot i+1 holds
the imaginary part; both slots are tagged `Complex`.

```rust
pub struct Roots<const N: usize> {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn get(self: &Self, i: usize) -> f64 { /* ... */ }
  ```
  Value stored at slot `i`.

- ```rust
  pub fn root_type(self: &Self, i: usize) -> RootType { /* ... */ }
  ```
  Root type at slot `i`.

- ```rust
  pub fn set_type(self: &mut Self, i: usize, t: RootType) { /* ... */ }
  ```
  Overwrite the type at slot `i`.

- ```rust
  pub fn new(t: RootType, x: f64) -> Self { /* ... */ }
  ```
  Single root with the given type and value.

- ```rust
  pub fn from_pair(a: Roots<1>, b: Roots<1>) -> Self { /* ... */ }
  ```
  Concatenate two single roots.  C++ `Roots<2>(Roots<1>, Roots<1>)`.

- ```rust
  pub fn with_tail(head: Roots<1>, t: RootType, x: f64) -> Self { /* ... */ }
  ```
  `Roots<1>` followed by one additional root.

- ```rust
  pub fn both(r: Roots<1>) -> Self { /* ... */ }
  ```
  Duplicate a single root into both slots.

- ```rust
  pub fn uniform(t: RootType, x: f64) -> Self { /* ... */ }
  ```
  All three slots get the same type and value.

- ```rust
  pub fn concat_1_2(a: Roots<1>, b: Roots<2>) -> Self { /* ... */ }
  ```
  Concatenate `Roots<1>` then `Roots<2>`.

- ```rust
  pub fn concat_2_1(a: Roots<2>, b: Roots<1>) -> Self { /* ... */ }
  ```
  Concatenate `Roots<2>` then `Roots<1>`.

- ```rust
  pub fn with_tail(head: Roots<2>, t: RootType, x: f64) -> Self { /* ... */ }
  ```
  `Roots<2>` followed by one additional root.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Roots<N> { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Index**
  - ```rust
    fn index(self: &Self, i: usize) -> &f64 { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Re-exports

#### Re-export `CubicEqn`

```rust
pub use cubic_eqn::CubicEqn;
```

#### Re-export `LinearEqn`

```rust
pub use linear_eqn::LinearEqn;
```

#### Re-export `Polynomial`

```rust
pub use polynomial::Polynomial;
```

#### Re-export `QuadraticEqn`

```rust
pub use quadratic_eqn::QuadraticEqn;
```

#### Re-export `RootType`

```rust
pub use roots::RootType;
```

#### Re-export `Roots`

```rust
pub use roots::Roots;
```

## Module `prelude`

Convenience re-exports of the most commonly used types and functions.

```rust
pub mod prelude { /* ... */ }
```

### Re-exports

#### Re-export `GREAT`

Convenience re-export of the most commonly used types and functions.

```rust
use outram_foam_basic_lib::prelude::*;
```

# What's included

**Primitives** (Layer 1a)
- Scalar constants: `SMALL`, `VSMALL`, `GREAT`, `VGREAT`, `ROOT_SMALL`, `ROOT_VSMALL`
- Types: `Vector3`, `Tensor`, `SymmTensor`, `SphericalTensor`

**Polynomial algebra** (Layers 1c + 1d)
- Root types: `RootType`, `Roots`
- Equation solvers: `LinearEqn`, `QuadraticEqn`, `CubicEqn`
- Function evaluation: `Polynomial`

**Math special functions** (Layer 1g)
- `erf_inv`, `inc_gamma_ratio_p`, `inc_gamma_ratio_q`, `inc_gamma_p`, `inc_gamma_q`, `inv_inc_gamma`

**Specie-level thermophysics** (Layer 1h)
- Custom quantity: `Compressibility` (ψ = ∂ρ/∂p|T, s²/m²)
- EOS traits/types: `EquationOfState`, `PerfectGas`, `RhoConst`
- Thermo traits/types: `ThermoModel`, `HConstThermo`, `JanafThermo`
- Transport traits/types: `TransportModel`, `ConstTransport`, `SutherlandTransport`

```rust
pub use crate::primitives::GREAT;
```

#### Re-export `ROOT_GREAT`

Convenience re-export of the most commonly used types and functions.

```rust
use outram_foam_basic_lib::prelude::*;
```

# What's included

**Primitives** (Layer 1a)
- Scalar constants: `SMALL`, `VSMALL`, `GREAT`, `VGREAT`, `ROOT_SMALL`, `ROOT_VSMALL`
- Types: `Vector3`, `Tensor`, `SymmTensor`, `SphericalTensor`

**Polynomial algebra** (Layers 1c + 1d)
- Root types: `RootType`, `Roots`
- Equation solvers: `LinearEqn`, `QuadraticEqn`, `CubicEqn`
- Function evaluation: `Polynomial`

**Math special functions** (Layer 1g)
- `erf_inv`, `inc_gamma_ratio_p`, `inc_gamma_ratio_q`, `inc_gamma_p`, `inc_gamma_q`, `inv_inc_gamma`

**Specie-level thermophysics** (Layer 1h)
- Custom quantity: `Compressibility` (ψ = ∂ρ/∂p|T, s²/m²)
- EOS traits/types: `EquationOfState`, `PerfectGas`, `RhoConst`
- Thermo traits/types: `ThermoModel`, `HConstThermo`, `JanafThermo`
- Transport traits/types: `TransportModel`, `ConstTransport`, `SutherlandTransport`

```rust
pub use crate::primitives::ROOT_GREAT;
```

#### Re-export `ROOT_SMALL`

Convenience re-export of the most commonly used types and functions.

```rust
use outram_foam_basic_lib::prelude::*;
```

# What's included

**Primitives** (Layer 1a)
- Scalar constants: `SMALL`, `VSMALL`, `GREAT`, `VGREAT`, `ROOT_SMALL`, `ROOT_VSMALL`
- Types: `Vector3`, `Tensor`, `SymmTensor`, `SphericalTensor`

**Polynomial algebra** (Layers 1c + 1d)
- Root types: `RootType`, `Roots`
- Equation solvers: `LinearEqn`, `QuadraticEqn`, `CubicEqn`
- Function evaluation: `Polynomial`

**Math special functions** (Layer 1g)
- `erf_inv`, `inc_gamma_ratio_p`, `inc_gamma_ratio_q`, `inc_gamma_p`, `inc_gamma_q`, `inv_inc_gamma`

**Specie-level thermophysics** (Layer 1h)
- Custom quantity: `Compressibility` (ψ = ∂ρ/∂p|T, s²/m²)
- EOS traits/types: `EquationOfState`, `PerfectGas`, `RhoConst`
- Thermo traits/types: `ThermoModel`, `HConstThermo`, `JanafThermo`
- Transport traits/types: `TransportModel`, `ConstTransport`, `SutherlandTransport`

```rust
pub use crate::primitives::ROOT_SMALL;
```

#### Re-export `ROOT_VSMALL`

Convenience re-export of the most commonly used types and functions.

```rust
use outram_foam_basic_lib::prelude::*;
```

# What's included

**Primitives** (Layer 1a)
- Scalar constants: `SMALL`, `VSMALL`, `GREAT`, `VGREAT`, `ROOT_SMALL`, `ROOT_VSMALL`
- Types: `Vector3`, `Tensor`, `SymmTensor`, `SphericalTensor`

**Polynomial algebra** (Layers 1c + 1d)
- Root types: `RootType`, `Roots`
- Equation solvers: `LinearEqn`, `QuadraticEqn`, `CubicEqn`
- Function evaluation: `Polynomial`

**Math special functions** (Layer 1g)
- `erf_inv`, `inc_gamma_ratio_p`, `inc_gamma_ratio_q`, `inc_gamma_p`, `inc_gamma_q`, `inv_inc_gamma`

**Specie-level thermophysics** (Layer 1h)
- Custom quantity: `Compressibility` (ψ = ∂ρ/∂p|T, s²/m²)
- EOS traits/types: `EquationOfState`, `PerfectGas`, `RhoConst`
- Thermo traits/types: `ThermoModel`, `HConstThermo`, `JanafThermo`
- Transport traits/types: `TransportModel`, `ConstTransport`, `SutherlandTransport`

```rust
pub use crate::primitives::ROOT_VSMALL;
```

#### Re-export `SMALL`

Convenience re-export of the most commonly used types and functions.

```rust
use outram_foam_basic_lib::prelude::*;
```

# What's included

**Primitives** (Layer 1a)
- Scalar constants: `SMALL`, `VSMALL`, `GREAT`, `VGREAT`, `ROOT_SMALL`, `ROOT_VSMALL`
- Types: `Vector3`, `Tensor`, `SymmTensor`, `SphericalTensor`

**Polynomial algebra** (Layers 1c + 1d)
- Root types: `RootType`, `Roots`
- Equation solvers: `LinearEqn`, `QuadraticEqn`, `CubicEqn`
- Function evaluation: `Polynomial`

**Math special functions** (Layer 1g)
- `erf_inv`, `inc_gamma_ratio_p`, `inc_gamma_ratio_q`, `inc_gamma_p`, `inc_gamma_q`, `inv_inc_gamma`

**Specie-level thermophysics** (Layer 1h)
- Custom quantity: `Compressibility` (ψ = ∂ρ/∂p|T, s²/m²)
- EOS traits/types: `EquationOfState`, `PerfectGas`, `RhoConst`
- Thermo traits/types: `ThermoModel`, `HConstThermo`, `JanafThermo`
- Transport traits/types: `TransportModel`, `ConstTransport`, `SutherlandTransport`

```rust
pub use crate::primitives::SMALL;
```

#### Re-export `VGREAT`

Convenience re-export of the most commonly used types and functions.

```rust
use outram_foam_basic_lib::prelude::*;
```

# What's included

**Primitives** (Layer 1a)
- Scalar constants: `SMALL`, `VSMALL`, `GREAT`, `VGREAT`, `ROOT_SMALL`, `ROOT_VSMALL`
- Types: `Vector3`, `Tensor`, `SymmTensor`, `SphericalTensor`

**Polynomial algebra** (Layers 1c + 1d)
- Root types: `RootType`, `Roots`
- Equation solvers: `LinearEqn`, `QuadraticEqn`, `CubicEqn`
- Function evaluation: `Polynomial`

**Math special functions** (Layer 1g)
- `erf_inv`, `inc_gamma_ratio_p`, `inc_gamma_ratio_q`, `inc_gamma_p`, `inc_gamma_q`, `inv_inc_gamma`

**Specie-level thermophysics** (Layer 1h)
- Custom quantity: `Compressibility` (ψ = ∂ρ/∂p|T, s²/m²)
- EOS traits/types: `EquationOfState`, `PerfectGas`, `RhoConst`
- Thermo traits/types: `ThermoModel`, `HConstThermo`, `JanafThermo`
- Transport traits/types: `TransportModel`, `ConstTransport`, `SutherlandTransport`

```rust
pub use crate::primitives::VGREAT;
```

#### Re-export `VSMALL`

Convenience re-export of the most commonly used types and functions.

```rust
use outram_foam_basic_lib::prelude::*;
```

# What's included

**Primitives** (Layer 1a)
- Scalar constants: `SMALL`, `VSMALL`, `GREAT`, `VGREAT`, `ROOT_SMALL`, `ROOT_VSMALL`
- Types: `Vector3`, `Tensor`, `SymmTensor`, `SphericalTensor`

**Polynomial algebra** (Layers 1c + 1d)
- Root types: `RootType`, `Roots`
- Equation solvers: `LinearEqn`, `QuadraticEqn`, `CubicEqn`
- Function evaluation: `Polynomial`

**Math special functions** (Layer 1g)
- `erf_inv`, `inc_gamma_ratio_p`, `inc_gamma_ratio_q`, `inc_gamma_p`, `inc_gamma_q`, `inv_inc_gamma`

**Specie-level thermophysics** (Layer 1h)
- Custom quantity: `Compressibility` (ψ = ∂ρ/∂p|T, s²/m²)
- EOS traits/types: `EquationOfState`, `PerfectGas`, `RhoConst`
- Thermo traits/types: `ThermoModel`, `HConstThermo`, `JanafThermo`
- Transport traits/types: `TransportModel`, `ConstTransport`, `SutherlandTransport`

```rust
pub use crate::primitives::VSMALL;
```

#### Re-export `SphericalTensor`

```rust
pub use crate::primitives::SphericalTensor;
```

#### Re-export `SymmTensor`

```rust
pub use crate::primitives::SymmTensor;
```

#### Re-export `Tensor`

```rust
pub use crate::primitives::Tensor;
```

#### Re-export `Vector3`

```rust
pub use crate::primitives::Vector3;
```

#### Re-export `eigen_values`

```rust
pub use crate::primitives::eigen_values;
```

#### Re-export `eigen_values_checked`

```rust
pub use crate::primitives::eigen_values_checked;
```

#### Re-export `eigen_values_symm`

```rust
pub use crate::primitives::eigen_values_symm;
```

#### Re-export `eigen_vectors`

```rust
pub use crate::primitives::eigen_vectors;
```

#### Re-export `eigen_vectors_symm`

```rust
pub use crate::primitives::eigen_vectors_symm;
```

#### Re-export `eigen_vectors_symm_with`

```rust
pub use crate::primitives::eigen_vectors_symm_with;
```

#### Re-export `eigen_vectors_with`

```rust
pub use crate::primitives::eigen_vectors_with;
```

#### Re-export `CubicEqn`

```rust
pub use crate::polynomial::CubicEqn;
```

#### Re-export `LinearEqn`

```rust
pub use crate::polynomial::LinearEqn;
```

#### Re-export `Polynomial`

```rust
pub use crate::polynomial::Polynomial;
```

#### Re-export `QuadraticEqn`

```rust
pub use crate::polynomial::QuadraticEqn;
```

#### Re-export `RootType`

```rust
pub use crate::polynomial::RootType;
```

#### Re-export `Roots`

```rust
pub use crate::polynomial::Roots;
```

#### Re-export `erf_inv`

```rust
pub use crate::math::erf_inv;
```

#### Re-export `erf_inv`

```rust
pub use crate::math::erf_inv;
```

#### Re-export `inc_gamma_p`

```rust
pub use crate::math::inc_gamma_p;
```

#### Re-export `inc_gamma_q`

```rust
pub use crate::math::inc_gamma_q;
```

#### Re-export `inc_gamma_ratio_p`

```rust
pub use crate::math::inc_gamma_ratio_p;
```

#### Re-export `inc_gamma_ratio_q`

```rust
pub use crate::math::inc_gamma_ratio_q;
```

#### Re-export `inv_inc_gamma`

```rust
pub use crate::math::inv_inc_gamma;
```

#### Re-export `inv_inc_gamma`

```rust
pub use crate::math::inv_inc_gamma;
```

#### Re-export `MatrixError`

```rust
pub use crate::matrix::MatrixError;
```

#### Re-export `SquareMatrix`

```rust
pub use crate::matrix::SquareMatrix;
```

#### Re-export `DynSystemIntegrator`

```rust
pub use crate::ode::DynSystemIntegrator;
```

#### Re-export `Euler`

```rust
pub use crate::ode::Euler;
```

#### Re-export `NoTypedSystem`

```rust
pub use crate::ode::NoTypedSystem;
```

#### Re-export `OdeError`

```rust
pub use crate::ode::OdeError;
```

#### Re-export `OdeIntegrator`

```rust
pub use crate::ode::OdeIntegrator;
```

#### Re-export `OdeSolver`

```rust
pub use crate::ode::OdeSolver;
```

#### Re-export `OdeSolverConfig`

```rust
pub use crate::ode::OdeSolverConfig;
```

#### Re-export `OdeSystem`

```rust
pub use crate::ode::OdeSystem;
```

#### Re-export `Rkf45`

```rust
pub use crate::ode::Rkf45;
```

#### Re-export `Rosenbrock23`

```rust
pub use crate::ode::Rosenbrock23;
```

#### Re-export `SharedOdeSystem`

```rust
pub use crate::ode::SharedOdeSystem;
```

#### Re-export `TypedStateIntegrator`

```rust
pub use crate::ode::TypedStateIntegrator;
```

#### Re-export `interpolate_spline_xy`

```rust
pub use crate::interpolation::interpolate_spline_xy;
```

#### Re-export `interpolate_spline_xy`

```rust
pub use crate::interpolation::interpolate_spline_xy;
```

#### Re-export `interpolate_xy`

```rust
pub use crate::interpolation::interpolate_xy;
```

#### Re-export `interpolate_xy`

```rust
pub use crate::interpolation::interpolate_xy;
```

#### Re-export `ThermoError`

```rust
pub use crate::thermophysics::error::ThermoError;
```

#### Re-export `Compressibility`

```rust
pub use crate::thermophysics::quantities::Compressibility;
```

#### Re-export `BoundaryCondition`

```rust
pub use crate::fields::BoundaryCondition;
```

#### Re-export `Field`

```rust
pub use crate::fields::Field;
```

#### Re-export `PatchField`

```rust
pub use crate::fields::PatchField;
```

#### Re-export `SurfaceField`

```rust
pub use crate::fields::SurfaceField;
```

#### Re-export `SurfaceScalarField`

```rust
pub use crate::fields::SurfaceScalarField;
```

#### Re-export `SurfaceVectorField`

```rust
pub use crate::fields::SurfaceVectorField;
```

#### Re-export `VolField`

```rust
pub use crate::fields::VolField;
```

#### Re-export `VolScalarField`

```rust
pub use crate::fields::VolScalarField;
```

#### Re-export `VolSymmTensorField`

```rust
pub use crate::fields::VolSymmTensorField;
```

#### Re-export `VolTensorField`

```rust
pub use crate::fields::VolTensorField;
```

#### Re-export `VolVectorField`

```rust
pub use crate::fields::VolVectorField;
```

#### Re-export `AmiCoupling`

```rust
pub use crate::mesh::AmiCoupling;
```

#### Re-export `AmiOverlap`

```rust
pub use crate::mesh::AmiOverlap;
```

#### Re-export `AmiWeight`

```rust
pub use crate::mesh::AmiWeight;
```

#### Re-export `BoundaryPatch`

```rust
pub use crate::mesh::BoundaryPatch;
```

#### Re-export `CyclicCoupling`

```rust
pub use crate::mesh::CyclicCoupling;
```

#### Re-export `FvMesh`

```rust
pub use crate::mesh::FvMesh;
```

#### Re-export `FvMeshBuilder`

```rust
pub use crate::mesh::FvMeshBuilder;
```

#### Re-export `MeshError`

```rust
pub use crate::mesh::MeshError;
```

#### Re-export `PatchKind`

```rust
pub use crate::mesh::PatchKind;
```

#### Re-export `RegionInterface`

```rust
pub use crate::mesh::RegionInterface;
```

#### Re-export `overlap_weights_1d`

```rust
pub use crate::mesh::ami::overlap_weights_1d;
```

#### Re-export `FvMatrix`

```rust
pub use crate::ldu_matrix::FvMatrix;
```

#### Re-export `FvVectorMatrix`

```rust
pub use crate::ldu_matrix::FvVectorMatrix;
```

#### Re-export `LduMatrix`

```rust
pub use crate::ldu_matrix::LduMatrix;
```

#### Re-export `SolverPerformance`

```rust
pub use crate::ldu_matrix::SolverPerformance;
```

#### Re-export `SolverSettings`

```rust
pub use crate::ldu_matrix::SolverSettings;
```

#### Re-export `adjust_phi`

```rust
pub use crate::fv_operators::adjust_phi;
```

#### Re-export `fvc`

```rust
pub use crate::fv_operators::fvc;
```

#### Re-export `fvm`

```rust
pub use crate::fv_operators::fvm;
```

#### Re-export `grad_least_squares`

```rust
pub use crate::fv_operators::fvc::grad_least_squares;
```

#### Re-export `laplacian_corrected`

```rust
pub use crate::fv_operators::fvm::laplacian_corrected;
```

#### Re-export `max_non_orthogonality_deg`

```rust
pub use crate::fv_operators::fvm::max_non_orthogonality_deg;
```

#### Re-export `non_ortho_geometry`

```rust
pub use crate::fv_operators::fvm::non_ortho_geometry;
```

#### Re-export `solve_laplacian_non_orthogonal`

```rust
pub use crate::fv_operators::fvm::solve_laplacian_non_orthogonal;
```

#### Re-export `NonOrthoGeometry`

```rust
pub use crate::fv_operators::fvm::NonOrthoGeometry;
```

#### Re-export `NonOrthoScheme`

```rust
pub use crate::fv_operators::fvm::NonOrthoScheme;
```

#### Re-export `CellSelection`

```rust
pub use crate::fv_options::CellSelection;
```

#### Re-export `EquationField`

```rust
pub use crate::fv_options::EquationField;
```

#### Re-export `FvModel`

```rust
pub use crate::fv_options::FvModel;
```

#### Re-export `FvModels`

```rust
pub use crate::fv_options::FvModels;
```

#### Re-export `MomentumEquationForm`

```rust
pub use crate::fv_options::MomentumEquationForm;
```

#### Re-export `SemiImplicitSource`

```rust
pub use crate::fv_options::SemiImplicitSource;
```

#### Re-export `SolidificationMelting`

```rust
pub use crate::fv_options::SolidificationMelting;
```

#### Re-export `SolidificationMeltingCoefficients`

```rust
pub use crate::fv_options::SolidificationMeltingCoefficients;
```

#### Re-export `SolidificationPorosity`

```rust
pub use crate::fv_options::SolidificationPorosity;
```

#### Re-export `SourceContribution`

```rust
pub use crate::fv_options::SourceContribution;
```

#### Re-export `TemperatureTable`

```rust
pub use crate::fv_options::TemperatureTable;
```

#### Re-export `VofSolidificationMelting`

```rust
pub use crate::fv_options::VofSolidificationMelting;
```

#### Re-export `vol_field_algebra`

```rust
pub use crate::fields::vol_field_algebra;
```

#### Re-export `ConstSolidThermo`

```rust
pub use crate::fluid_thermo::ConstSolidThermo;
```

#### Re-export `FluidThermo`

```rust
pub use crate::fluid_thermo::FluidThermo;
```

#### Re-export `PsiThermo`

```rust
pub use crate::fluid_thermo::PsiThermo;
```

#### Re-export `RhoThermo`

```rust
pub use crate::fluid_thermo::RhoThermo;
```

#### Re-export `SolidThermo`

```rust
pub use crate::fluid_thermo::SolidThermo;
```

#### Re-export `conjugate_gradient`

```rust
pub use crate::ldu_matrix::conjugate_gradient;
```

#### Re-export `conjugate_gradient`

```rust
pub use crate::ldu_matrix::conjugate_gradient;
```

#### Re-export `gamg`

```rust
pub use crate::ldu_matrix::gamg;
```

#### Re-export `gamg`

```rust
pub use crate::ldu_matrix::gamg;
```

#### Re-export `gauss_seidel`

```rust
pub use crate::ldu_matrix::gauss_seidel;
```

#### Re-export `gauss_seidel`

```rust
pub use crate::ldu_matrix::gauss_seidel;
```

#### Re-export `krylov_solve`

```rust
pub use crate::ldu_matrix::krylov_solve;
```

#### Re-export `KrylovMethod`

```rust
pub use crate::ldu_matrix::KrylovMethod;
```

#### Re-export `KrylovOptions`

```rust
pub use crate::ldu_matrix::KrylovOptions;
```

#### Re-export `PreconditionerKind`

```rust
pub use crate::ldu_matrix::PreconditionerKind;
```

#### Re-export `bicgstab`

```rust
pub use crate::krylov::bicgstab;
```

#### Re-export `gmres`

```rust
pub use crate::krylov::gmres;
```

#### Re-export `Ilu0Preconditioner`

```rust
pub use crate::krylov::Ilu0Preconditioner;
```

#### Re-export `JacobiPreconditioner`

```rust
pub use crate::krylov::JacobiPreconditioner;
```

#### Re-export `KrylovResult`

```rust
pub use crate::krylov::KrylovResult;
```

#### Re-export `KrylovSettings`

```rust
pub use crate::krylov::KrylovSettings;
```

#### Re-export `Preconditioner`

```rust
pub use crate::krylov::Preconditioner;
```

#### Re-export `interface`

```rust
pub use crate::interface;
```

#### Re-export `FluxLimiter`

```rust
pub use crate::limiters::FluxLimiter;
```

#### Re-export `crate::thermophysics::eos::*`

```rust
pub use crate::thermophysics::eos::*;
```

#### Re-export `crate::thermophysics::thermo::*`

```rust
pub use crate::thermophysics::thermo::*;
```

#### Re-export `crate::thermophysics::transport::*`

```rust
pub use crate::thermophysics::transport::*;
```

## Module `primitives`

Layer 1a — dimensionless scalar constants and the tensor-algebra
primitives (`Vector3`, `Tensor`, `SymmTensor`, `SphericalTensor`).
Layer 1a — the primitive numeric types OpenFOAM builds everything on.

This module holds the dimensionless scalar type and small-/large-number
constants (`scalar`) together with the fixed-size 3-D tensor-algebra
primitives: a 3-vector (`Vector3`), a full 3×3 tensor (`Tensor`), a
symmetric 3×3 tensor (`SymmTensor`), and an isotropic diagonal tensor
(`SphericalTensor`). All components are plain `f64` (dimensionless SI);
`uom`-dimensioned quantities are layered on top elsewhere in the crate.
Each type mirrors its `Foam::` counterpart, including component storage
order and the OpenFOAM operator conventions (`&`, `&&`, `^`, `*`).

```rust
pub mod primitives { /* ... */ }
```

### Modules

## Module `eigen`

The scalar floating-point type and the small/large numeric constants.
Spectral decomposition of 3x3 tensors -- eigenvalues, eigenvectors, and the
basis every isotropic tensor function (logarithm, exponential, square root)
is built on.
Eigenvalues and eigenvectors of 3x3 tensors.

# What this is for

A spectral decomposition turns a tensor into "three directions, each with a
stretch". That is exactly what several continuum-mechanics operations need:

- **Principal stresses and strains** — the eigenvalues of the stress or
  strain tensor, and the directions they act along.
- **Isotropic tensor functions** — any function of a symmetric tensor
  (logarithm, exponential, square root) is defined by applying the scalar
  function to the eigenvalues and rebuilding in the same eigenbasis. The
  logarithmic (Hencky) strain measure used by finite-strain plasticity is
  the motivating case.
- **Polar decomposition** — separating rotation from stretch.

# Method

Both routines solve the characteristic cubic
`det(T - λI) = 0` directly with [`CubicEqn`](crate::polynomial::cubic_eqn::CubicEqn), rather than iterating a Jacobi
or QR sweep. That is upstream OpenFOAM's approach and it is the right one at
3x3: the closed-form cubic is exact up to round-off, has no iteration count
to tune, and reuses the polynomial solver this crate already carries.

Eigenvectors then come from the sub-determinants of `T - λI`, choosing the
largest sub-determinant for conditioning, with dedicated fallbacks for
repeated and triple eigenvalues.

# Ordering and normalisation

Eigenvalues are returned in **ascending** order, matching upstream. The
eigenvector rows of the returned [`Tensor`] correspond to the eigenvalues in
that same order, and each is normalised to unit length.

# Degeneracy

Repeated eigenvalues do not have unique eigenvectors — any vector in the
degenerate subspace will do. The symmetric routines return *an* orthonormal
set spanning the right subspaces, which is what an isotropic tensor function
needs; do not read meaning into which particular basis of a degenerate
subspace comes back.

**Accuracy near a degeneracy is limited to `√(machine epsilon)`, about
1.5e-8, and this is inherent to the method rather than a defect.** A
repeated root of a polynomial is ill-conditioned: perturbing the
coefficients by `δ` moves a double root by `√δ`. Since both routines get
their eigenvalues from the characteristic cubic, a tensor with a repeated
eigenvalue yields that eigenvalue to roughly eight digits, not sixteen — so
`T v - λ v` for such a pair sits near 1e-8, not near 1e-16.

Two consequences worth knowing before relying on this:

- Do not set a residual tolerance tighter than about 1e-7 on a spectrum that
  may be degenerate.
- A *computed* tensor (`C = FᵀF`, say) splits an exactly-repeated eigenvalue
  into two numerically distinct ones. The symmetric routines handle that —
  [`eigen_vectors_symm_with`] orthonormalises for exactly this reason — but
  the general [`eigen_vectors_with`] does not, because a non-symmetric
  tensor has no orthogonal eigenbasis to restore.

```rust
pub mod eigen { /* ... */ }
```

### Functions

#### Function `eigen_values`

**Attributes:**

- `MustUse { reason: None }`

Eigenvalues of a general (possibly non-symmetric) 3x3 tensor, ascending.

A general tensor may have complex eigenvalues. Since this returns three real
numbers, a complex pair is reported as zero in those slots — matching
upstream OpenFOAM, which warns and does the same. If you need to know
whether that happened, use [`eigen_values_checked`].

Infinite roots are clamped to `±VGREAT` rather than returning an infinity
that would poison downstream arithmetic silently.

```rust
pub fn eigen_values(t: crate::primitives::Tensor) -> crate::primitives::Vector3 { /* ... */ }
```

#### Function `eigen_values_checked`

**Attributes:**

- `MustUse { reason: None }`

As [`eigen_values`], but also reports whether any root was complex.

The flag matters because a complex pair is *not* an error in a general
tensor — a rotation has complex eigenvalues — but it does mean the three
returned reals are not a complete description. A caller building an
isotropic tensor function must not proceed on a complex spectrum.

```rust
pub fn eigen_values_checked(t: crate::primitives::Tensor) -> (crate::primitives::Vector3, bool) { /* ... */ }
```

#### Function `eigen_values_symm`

**Attributes:**

- `MustUse { reason: None }`

Eigenvalues of a symmetric 3x3 tensor, ascending.

A real symmetric tensor is guaranteed a real spectrum, so unlike
[`eigen_values`] there is no complex case to report — any complex root here
would be round-off in the cubic solve, not physics.

```rust
pub fn eigen_values_symm(t: crate::primitives::SymmTensor) -> crate::primitives::Vector3 { /* ... */ }
```

#### Function `eigen_vectors_with`

**Attributes:**

- `MustUse { reason: None }`

Eigenvectors of a general tensor for given eigenvalues, as tensor **rows**.

Row `i` is the unit eigenvector belonging to `lambdas[i]`. Pass the
eigenvalues from [`eigen_values`] on the same tensor; passing values from a
different tensor produces a meaningless result rather than an error.

```rust
pub fn eigen_vectors_with(t: crate::primitives::Tensor, lambdas: crate::primitives::Vector3) -> crate::primitives::Tensor { /* ... */ }
```

#### Function `eigen_vectors`

**Attributes:**

- `MustUse { reason: None }`

Eigenvectors of a general tensor, as tensor rows, ordered by ascending
eigenvalue.

```rust
pub fn eigen_vectors(t: crate::primitives::Tensor) -> crate::primitives::Tensor { /* ... */ }
```

#### Function `eigen_vectors_symm_with`

**Attributes:**

- `MustUse { reason: None }`

Eigenvectors of a symmetric tensor for given eigenvalues, as tensor rows.

The rows are guaranteed **orthonormal**, which the general
[`eigen_vectors_with`] does not guarantee and cannot: a non-symmetric tensor
has no orthogonal eigenbasis in general. See the note on near-degeneracy
below for why this needs its own code path rather than deferring entirely to
the general routine.

```rust
pub fn eigen_vectors_symm_with(t: crate::primitives::SymmTensor, lambdas: crate::primitives::Vector3) -> crate::primitives::Tensor { /* ... */ }
```

#### Function `eigen_vectors_symm`

**Attributes:**

- `MustUse { reason: None }`

Eigenvectors of a symmetric tensor, as tensor rows, ordered by ascending
eigenvalue.

For a symmetric tensor the returned rows are orthonormal, so the tensor is a
rotation (or a reflection) and its transpose is its inverse — which is what
makes rebuilding an isotropic function cheap.

```rust
pub fn eigen_vectors_symm(t: crate::primitives::SymmTensor) -> crate::primitives::Tensor { /* ... */ }
```

## Module `scalar`

The scalar floating-point type and OpenFOAM's small/large numeric
guard constants.

`Scalar` is OpenFOAM's `scalar` (double-precision, dimensionless) and
`Label` is its `label` (signed integer index/count). The constants are
the fixed thresholds OpenFOAM uses to guard against divide-by-zero and
overflow; they are dimensionless and identical in value to the upstream
`doubleScalar` definitions.

```rust
pub mod scalar { /* ... */ }
```

### Types

#### Type Alias `Scalar`

OpenFOAM `scalar` — a dimensionless double-precision floating-point value.

```rust
pub type Scalar = f64;
```

#### Type Alias `Label`

OpenFOAM `label` — a signed integer used for indices and counts.

```rust
pub type Label = i64;
```

### Constants and Statics

#### Constant `SMALL`

Small number used to guard against division by (near-)zero (1e-15).

```rust
pub const SMALL: Scalar = 1e-15;
```

#### Constant `VSMALL`

Very small number near the underflow floor (1e-300).

```rust
pub const VSMALL: Scalar = 1e-300;
```

#### Constant `ROOT_SMALL`

Square root of `SMALL` (≈ 3.162e-8).

```rust
pub const ROOT_SMALL: Scalar = 3.162_277_660_168_379_5e-8;
```

#### Constant `ROOT_VSMALL`

Square root of `VSMALL` (1e-150).

```rust
pub const ROOT_VSMALL: Scalar = 1e-150;
```

#### Constant `GREAT`

Large number used as a finite stand-in for "infinity" (1e15).

```rust
pub const GREAT: Scalar = 1e15;
```

#### Constant `VGREAT`

Very large number near the overflow ceiling (1e300).

```rust
pub const VGREAT: Scalar = 1e300;
```

#### Constant `ROOT_GREAT`

Square root of `GREAT` (≈ 3.162e7).

```rust
pub const ROOT_GREAT: Scalar = 3.162_277_660_168_379_5e7;
```

## Module `spherical_tensor`

Isotropic diagonal tensor `ii * I` (`SphericalTensor`).
Isotropic diagonal tensor (`SphericalTensor`) — an OpenFOAM primitive that
stores only the single scalar `ii` of `ii * I`.

`ii` is a dimensionless `f64`. Because the tensor is a scalar multiple of
the identity, its operations reduce to scalar arithmetic (trace `3*ii`,
determinant `ii³`, inverse `1/ii`).

```rust
pub mod spherical_tensor { /* ... */ }
```

### Types

#### Struct `SphericalTensor`

Isotropic diagonal tensor: represents `ii * I` where `I` is the 3×3 identity.
Maps to `Foam::SphericalTensor<scalar>` (`SphericalTensorI.H`).

```rust
pub struct SphericalTensor {
    pub ii: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ii` | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn new(ii: f64) -> Self { /* ... */ }
  ```
  Construct from the single isotropic component `ii` (the tensor is `ii*I`).

- ```rust
  pub fn tr(self: Self) -> f64 { /* ... */ }
  ```
  Trace = 3 * ii

- ```rust
  pub fn mag_sqr(self: Self) -> f64 { /* ... */ }
  ```
  Frobenius norm squared = 3 * ii²

- ```rust
  pub fn mag(self: Self) -> f64 { /* ... */ }
  ```

- ```rust
  pub fn diag_sqr(self: Self) -> f64 { /* ... */ }
  ```
  Diagonal norm squared (sum of squared diagonal entries = 3*ii²)

- ```rust
  pub fn det(self: Self) -> f64 { /* ... */ }
  ```
  Determinant = ii³

- ```rust
  pub fn inv(self: Self) -> Self { /* ... */ }
  ```
  Inverse: SphericalTensor(1/ii)

- ```rust
  pub fn double_inner(self: Self, rhs: Self) -> f64 { /* ... */ }
  ```
  Double inner-product with itself: 3 * ii²

- ```rust
  pub fn lerp(a: Self, b: Self, t: f64) -> Self { /* ... */ }
  ```
  Linear interpolation

###### Trait Implementations

- **Add**
  - ```rust
    fn add(self: Self, rhs: Self) -> Self { /* ... */ }
    ```

  - ```rust
    fn add(self: Self, st: SymmTensor) -> SymmTensor { /* ... */ }
    ```

  - ```rust
    fn add(self: Self, spt: SphericalTensor) -> Self { /* ... */ }
    ```

  - ```rust
    fn add(self: Self, st: SphericalTensor) -> Self { /* ... */ }
    ```

  - ```rust
    fn add(self: Self, t: Tensor) -> Tensor { /* ... */ }
    ```

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> SphericalTensor { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> SphericalTensor { /* ... */ }
    ```

- **Div**
  - ```rust
    fn div(self: Self, s: f64) -> Self { /* ... */ }
    ```

  - ```rust
    fn div(self: Self, st: SphericalTensor) -> SphericalTensor { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(st: SphericalTensor) -> Self { /* ... */ }
    ```

  - ```rust
    fn from(st: SphericalTensor) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Mul**
  - ```rust
    fn mul(self: Self, s: f64) -> Self { /* ... */ }
    ```

  - ```rust
    fn mul(self: Self, st: SphericalTensor) -> SphericalTensor { /* ... */ }
    ```

  - ```rust
    fn mul(self: Self, st: SymmTensor) -> SymmTensor { /* ... */ }
    ```

  - ```rust
    fn mul(self: Self, spt: SphericalTensor) -> Self { /* ... */ }
    ```

  - ```rust
    fn mul(self: Self, t: Tensor) -> Tensor { /* ... */ }
    ```

  - ```rust
    fn mul(self: Self, st: SphericalTensor) -> Self { /* ... */ }
    ```

- **Neg**
  - ```rust
    fn neg(self: Self) -> Self { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SphericalTensor) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sub**
  - ```rust
    fn sub(self: Self, rhs: Self) -> Self { /* ... */ }
    ```

  - ```rust
    fn sub(self: Self, st: SymmTensor) -> SymmTensor { /* ... */ }
    ```

  - ```rust
    fn sub(self: Self, spt: SphericalTensor) -> Self { /* ... */ }
    ```

  - ```rust
    fn sub(self: Self, st: SphericalTensor) -> Self { /* ... */ }
    ```

  - ```rust
    fn sub(self: Self, t: Tensor) -> Tensor { /* ... */ }
    ```

- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `tr`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Trace tr = 3*ii.

```rust
pub fn tr(st: SphericalTensor) -> f64 { /* ... */ }
```

#### Function `det`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Determinant = ii³.

```rust
pub fn det(st: SphericalTensor) -> f64 { /* ... */ }
```

#### Function `inv`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Inverse = SphericalTensor(1/ii).

```rust
pub fn inv(st: SphericalTensor) -> SphericalTensor { /* ... */ }
```

#### Function `mag_sqr`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Frobenius norm squared = 3*ii².

```rust
pub fn mag_sqr(st: SphericalTensor) -> f64 { /* ... */ }
```

#### Function `lerp`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Linear interpolation `(1-t)*a + t*b` between two spherical tensors.

```rust
pub fn lerp(a: SphericalTensor, b: SphericalTensor, t: f64) -> SphericalTensor { /* ... */ }
```

## Module `symm_tensor`

Symmetric 3×3 tensor (`SymmTensor`).
Symmetric 3×3 tensor (`SymmTensor`) and its OpenFOAM-style operators.

Only the six upper-triangle components are stored (xx, xy, xz, yy, yz, zz),
all dimensionless `f64`; the lower triangle is implied by symmetry. Norms
and the double contraction count the off-diagonal terms twice, matching
OpenFOAM.

```rust
pub mod symm_tensor { /* ... */ }
```

### Types

#### Struct `SymmTensor`

Symmetric 3×3 tensor stored in upper-triangle order: xx, xy, xz, yy, yz, zz.
Maps to `Foam::symmTensor` (`Foam::SymmTensor<scalar>`).

```rust
pub struct SymmTensor {
    pub xx: f64,
    pub xy: f64,
    pub xz: f64,
    pub yy: f64,
    pub yz: f64,
    pub zz: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `xx` | `f64` |  |
| `xy` | `f64` |  |
| `xz` | `f64` |  |
| `yy` | `f64` |  |
| `yz` | `f64` |  |
| `zz` | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn new(xx: f64, xy: f64, xz: f64, yy: f64, yz: f64, zz: f64) -> Self { /* ... */ }
  ```
  Construct from the six upper-triangle components (xx, xy, xz, yy, yz, zz).

- ```rust
  pub fn from_diag(xx: f64, yy: f64, zz: f64) -> Self { /* ... */ }
  ```
  Construct from diagonal only (off-diagonal = 0).

- ```rust
  pub fn row_x(self: Self) -> Vector3 { /* ... */ }
  ```
  Row vectors (yx = xy, zx = xz, zy = yz because symmetric)

- ```rust
  pub fn row_y(self: Self) -> Vector3 { /* ... */ }
  ```

- ```rust
  pub fn row_z(self: Self) -> Vector3 { /* ... */ }
  ```

- ```rust
  pub fn diag(self: Self) -> Vector3 { /* ... */ }
  ```
  Diagonal as a vector

- ```rust
  pub fn tr(self: Self) -> f64 { /* ... */ }
  ```
  Trace: xx + yy + zz

- ```rust
  pub fn sph(self: Self) -> SphericalTensor { /* ... */ }
  ```
  Spherical (isotropic) part: (tr/3) * I

- ```rust
  pub fn dev(self: Self) -> Self { /* ... */ }
  ```
  Deviatoric part: self - (tr/3)*I

- ```rust
  pub fn dev2(self: Self) -> Self { /* ... */ }
  ```
  Two-thirds deviatoric part: self - (2*tr/3)*I

- ```rust
  pub fn det(self: Self) -> f64 { /* ... */ }
  ```
  Determinant

- ```rust
  pub fn adjunct(self: Self) -> Self { /* ... */ }
  ```
  Adjunct (= cofactor matrix, same as adjunct because symmetric)

- ```rust
  pub fn inv(self: Self) -> Self { /* ... */ }
  ```
  Inverse = adjunct / det. Panics if singular in debug builds.

- ```rust
  pub fn safe_inv(self: Self) -> Self { /* ... */ }
  ```
  Inverse with fallback: returns ZERO if nearly singular.

- ```rust
  pub fn mag_sqr(self: Self) -> f64 { /* ... */ }
  ```
  Frobenius norm squared (off-diagonal counted twice, matching OpenFOAM)

- ```rust
  pub fn mag(self: Self) -> f64 { /* ... */ }
  ```

- ```rust
  pub fn diag_sqr(self: Self) -> f64 { /* ... */ }
  ```
  Sum of squared diagonal entries (not Frobenius)

- ```rust
  pub fn inner_sqr(self: Self) -> Self { /* ... */ }
  ```
  Self² as a SymmTensor (S·S where both factors are symmetric)

- ```rust
  pub fn double_inner(self: Self, rhs: Self) -> f64 { /* ... */ }
  ```
  Double contraction (Frobenius inner product). C++ `operator&&`.

- ```rust
  pub fn mat_vec(self: Self, v: Vector3) -> Vector3 { /* ... */ }
  ```
  Matrix multiply SymmTensor·Vector → Vector. C++ `operator&(SymmTensor, Vector)`.

- ```rust
  pub fn hodge_dual(self: Self) -> Vector3 { /* ... */ }
  ```
  Hodge dual: returns the axial vector. C++ `operator*(SymmTensor)`.

- ```rust
  pub fn from_outer(v: Vector3) -> Self { /* ... */ }
  ```
  Outer (dyadic) product of a vector with itself: v ⊗ v → SymmTensor.

- ```rust
  pub fn invariant_i(self: Self) -> f64 { /* ... */ }
  ```
  First invariant: trace

- ```rust
  pub fn invariant_ii(self: Self) -> f64 { /* ... */ }
  ```
  Second invariant: (xx*yy + yy*zz + xx*zz) - (xy² + yz² + xz²)

- ```rust
  pub fn invariant_iii(self: Self) -> f64 { /* ... */ }
  ```
  Third invariant: determinant

- ```rust
  pub fn lerp(a: Self, b: Self, t: f64) -> Self { /* ... */ }
  ```
  Linear interpolation

- ```rust
  pub fn is_identity(self: Self, tol: f64) -> bool { /* ... */ }
  ```
  True if the tensor is (approximately) the identity.

###### Trait Implementations

- **Add**
  - ```rust
    fn add(self: Self, r: Self) -> Self { /* ... */ }
    ```

  - ```rust
    fn add(self: Self, st: SymmTensor) -> SymmTensor { /* ... */ }
    ```

  - ```rust
    fn add(self: Self, spt: SphericalTensor) -> Self { /* ... */ }
    ```

  - ```rust
    fn add(self: Self, st: SymmTensor) -> Self { /* ... */ }
    ```

  - ```rust
    fn add(self: Self, t: Tensor) -> Tensor { /* ... */ }
    ```

- **AddAssign**
  - ```rust
    fn add_assign(self: &mut Self, r: Self) { /* ... */ }
    ```

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> SymmTensor { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> SymmTensor { /* ... */ }
    ```

- **Div**
  - ```rust
    fn div(self: Self, s: f64) -> Self { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(st: SphericalTensor) -> Self { /* ... */ }
    ```

  - ```rust
    fn from(st: SymmTensor) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Mul**
  - ```rust
    fn mul(self: Self, s: f64) -> Self { /* ... */ }
    ```

  - ```rust
    fn mul(self: Self, st: SymmTensor) -> SymmTensor { /* ... */ }
    ```

  - ```rust
    fn mul(self: Self, st: SymmTensor) -> SymmTensor { /* ... */ }
    ```

  - ```rust
    fn mul(self: Self, spt: SphericalTensor) -> Self { /* ... */ }
    ```

  - ```rust
    fn mul(self: Self, t: Tensor) -> Tensor { /* ... */ }
    ```

  - ```rust
    fn mul(self: Self, st: SymmTensor) -> Self { /* ... */ }
    ```

  - ```rust
    fn mul(self: Self, st: SymmTensor) -> Tensor { /* ... */ }
    ```

- **MulAssign**
  - ```rust
    fn mul_assign(self: &mut Self, s: f64) { /* ... */ }
    ```

- **Neg**
  - ```rust
    fn neg(self: Self) -> Self { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SymmTensor) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sub**
  - ```rust
    fn sub(self: Self, r: Self) -> Self { /* ... */ }
    ```

  - ```rust
    fn sub(self: Self, st: SymmTensor) -> SymmTensor { /* ... */ }
    ```

  - ```rust
    fn sub(self: Self, spt: SphericalTensor) -> Self { /* ... */ }
    ```

  - ```rust
    fn sub(self: Self, st: SymmTensor) -> Self { /* ... */ }
    ```

  - ```rust
    fn sub(self: Self, t: Tensor) -> Tensor { /* ... */ }
    ```

- **SubAssign**
  - ```rust
    fn sub_assign(self: &mut Self, r: Self) { /* ... */ }
    ```

- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `tr`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Trace tr(S) = xx + yy + zz.

```rust
pub fn tr(st: SymmTensor) -> f64 { /* ... */ }
```

#### Function `det`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Determinant det(S).

```rust
pub fn det(st: SymmTensor) -> f64 { /* ... */ }
```

#### Function `inv`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Inverse S⁻¹ (panics in debug builds if singular).

```rust
pub fn inv(st: SymmTensor) -> SymmTensor { /* ... */ }
```

#### Function `dev`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Deviatoric part `S - (tr/3)*I`.

```rust
pub fn dev(st: SymmTensor) -> SymmTensor { /* ... */ }
```

#### Function `dev2`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Two-thirds deviatoric `S - (2*tr/3)*I`.

```rust
pub fn dev2(st: SymmTensor) -> SymmTensor { /* ... */ }
```

#### Function `symm`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Symmetric part of a SymmTensor is itself.

```rust
pub fn symm(st: SymmTensor) -> SymmTensor { /* ... */ }
```

#### Function `two_symm`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Twice the symmetric part of a SymmTensor.

```rust
pub fn two_symm(st: SymmTensor) -> SymmTensor { /* ... */ }
```

#### Function `dev_symm`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

dev(symm(st)) — deviatoric of symmetric part (same as dev for SymmTensor).

```rust
pub fn dev_symm(st: SymmTensor) -> SymmTensor { /* ... */ }
```

#### Function `dev_two_symm`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

dev(2*symm(st))

```rust
pub fn dev_two_symm(st: SymmTensor) -> SymmTensor { /* ... */ }
```

#### Function `sqr`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Outer (dyadic) product v ⊗ v as a SymmTensor. C++ `sqr(Vector)`.

```rust
pub fn sqr(v: super::vector::Vector3) -> SymmTensor { /* ... */ }
```

#### Function `mag_sqr`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Frobenius norm squared (off-diagonal terms counted twice, per OpenFOAM).

```rust
pub fn mag_sqr(st: SymmTensor) -> f64 { /* ... */ }
```

#### Function `lerp`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Linear interpolation `(1-t)*a + t*b` between two symmetric tensors.

```rust
pub fn lerp(a: SymmTensor, b: SymmTensor, t: f64) -> SymmTensor { /* ... */ }
```

## Module `tensor`

Full (non-symmetric) 3×3 tensor (`Tensor`).
Full (non-symmetric) 3×3 tensor (`Tensor`) and its OpenFOAM-style
operators, invariants, and decompositions.

Components are dimensionless `f64` stored row-major
(xx, xy, xz, yx, yy, yz, zx, zy, zz). Operator names follow OpenFOAM:
`mat_mul`/`mat_vec` are the single inner product (`&`), `double_inner`
the double contraction (`&&`), and the dyadic/outer product is `Mul`
(`*`) of two vectors.

```rust
pub mod tensor { /* ... */ }
```

### Types

#### Struct `Tensor`

Full (non-symmetric) 3×3 tensor stored row-major.
Component order: xx, xy, xz, yx, yy, yz, zx, zy, zz.
Maps to `Foam::tensor` (`Foam::Tensor<scalar>`).

```rust
pub struct Tensor {
    pub xx: f64,
    pub xy: f64,
    pub xz: f64,
    pub yx: f64,
    pub yy: f64,
    pub yz: f64,
    pub zx: f64,
    pub zy: f64,
    pub zz: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `xx` | `f64` |  |
| `xy` | `f64` |  |
| `xz` | `f64` |  |
| `yx` | `f64` |  |
| `yy` | `f64` |  |
| `yz` | `f64` |  |
| `zx` | `f64` |  |
| `zy` | `f64` |  |
| `zz` | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn new(xx: f64, xy: f64, xz: f64, yx: f64, yy: f64, yz: f64, zx: f64, zy: f64, zz: f64) -> Self { /* ... */ }
  ```
  Construct a tensor from its nine components in row-major order.

- ```rust
  pub fn from_rows(x: Vector3, y: Vector3, z: Vector3) -> Self { /* ... */ }
  ```
  Construct from three row vectors.

- ```rust
  pub fn from_cols(x: Vector3, y: Vector3, z: Vector3) -> Self { /* ... */ }
  ```
  Construct from three column vectors.

- ```rust
  pub fn row_x(self: Self) -> Vector3 { /* ... */ }
  ```
  First (x) row as a vector: (xx, xy, xz).

- ```rust
  pub fn row_y(self: Self) -> Vector3 { /* ... */ }
  ```
  Second (y) row as a vector: (yx, yy, yz).

- ```rust
  pub fn row_z(self: Self) -> Vector3 { /* ... */ }
  ```
  Third (z) row as a vector: (zx, zy, zz).

- ```rust
  pub fn col_x(self: Self) -> Vector3 { /* ... */ }
  ```
  First (x) column as a vector: (xx, yx, zx).

- ```rust
  pub fn col_y(self: Self) -> Vector3 { /* ... */ }
  ```
  Second (y) column as a vector: (xy, yy, zy).

- ```rust
  pub fn col_z(self: Self) -> Vector3 { /* ... */ }
  ```
  Third (z) column as a vector: (xz, yz, zz).

- ```rust
  pub fn diag(self: Self) -> Vector3 { /* ... */ }
  ```
  Diagonal as a vector

- ```rust
  pub fn tr(self: Self) -> f64 { /* ... */ }
  ```
  Trace

- ```rust
  pub fn diag_sqr(self: Self) -> f64 { /* ... */ }
  ```
  Sum of squared diagonal entries (not Frobenius)

- ```rust
  pub fn transpose(self: Self) -> Self { /* ... */ }
  ```
  Transpose. C++ `.T()`.

- ```rust
  pub fn det(self: Self) -> f64 { /* ... */ }
  ```
  Determinant

- ```rust
  pub fn adjunct(self: Self) -> Self { /* ... */ }
  ```
  Adjunct (transpose of cofactor matrix)

- ```rust
  pub fn cof(self: Self) -> Self { /* ... */ }
  ```
  Cofactor matrix = adjunct().T()

- ```rust
  pub fn inv(self: Self) -> Self { /* ... */ }
  ```
  Inverse = adjunct / det. Panics (debug) if singular.

- ```rust
  pub fn safe_inv(self: Self) -> Self { /* ... */ }
  ```
  Inverse with 2-D fallback: returns ZERO if nearly singular.

- ```rust
  pub fn mat_mul(self: Self, t: Self) -> Self { /* ... */ }
  ```
  Matrix multiply: `self & rhs`. C++ `operator&(Tensor, Tensor)` / `.inner(t2)`.

- ```rust
  pub fn schur(self: Self, t: Self) -> Self { /* ... */ }
  ```
  Element-wise product (Schur/Hadamard product).

- ```rust
  pub fn mat_vec(self: Self, v: Vector3) -> Vector3 { /* ... */ }
  ```
  Matrix-vector multiply: `T · v`. C++ `operator&(Tensor, Vector)`.

- ```rust
  pub fn vec_mat(v: Vector3, t: Self) -> Vector3 { /* ... */ }
  ```
  Vector-matrix multiply: `v · T`. C++ `operator&(Vector, Tensor)`.

- ```rust
  pub fn double_inner(self: Self, t: Self) -> f64 { /* ... */ }
  ```
  Double contraction (full Frobenius inner product). C++ `operator&&(Tensor, Tensor)`.

- ```rust
  pub fn symm(self: Self) -> SymmTensor { /* ... */ }
  ```
  Symmetric part: `0.5*(T + T^T)`. Returns `SymmTensor`.

- ```rust
  pub fn two_symm(self: Self) -> SymmTensor { /* ... */ }
  ```
  Twice the symmetric part: `T + T^T`. Returns `SymmTensor`.

- ```rust
  pub fn skew(self: Self) -> Self { /* ... */ }
  ```
  Skew-symmetric (antisymmetric) part: `0.5*(T - T^T)`.

- ```rust
  pub fn dev(self: Self) -> Self { /* ... */ }
  ```
  Deviatoric part: `T - (tr/3)*I`.

- ```rust
  pub fn dev2(self: Self) -> Self { /* ... */ }
  ```
  Two-thirds deviatoric: `T - (2*tr/3)*I`.

- ```rust
  pub fn dev_symm(self: Self) -> SymmTensor { /* ... */ }
  ```
  Deviatoric of symmetric part: `symm(T) - (tr/3)*I`. Returns `SymmTensor`.

- ```rust
  pub fn dev_two_symm(self: Self) -> SymmTensor { /* ... */ }
  ```
  Deviatoric of twice the symmetric part: `twoSymm(T) - (2*tr/3)*I`. Returns `SymmTensor`.

- ```rust
  pub fn hodge_dual(self: Self) -> Vector3 { /* ... */ }
  ```
  Hodge dual as a Vector. C++ `operator*(Tensor)`.

- ```rust
  pub fn invariant_i(self: Self) -> f64 { /* ... */ }
  ```
  First invariant: trace

- ```rust
  pub fn invariant_ii(self: Self) -> f64 { /* ... */ }
  ```
  Second invariant: (xx*yy + yy*zz + xx*zz) - (xy*yx + yz*zy + xz*zx)

- ```rust
  pub fn invariant_iii(self: Self) -> f64 { /* ... */ }
  ```
  Third invariant: determinant

- ```rust
  pub fn is_identity(self: Self, tol: f64) -> bool { /* ... */ }
  ```
  True if approximately the identity.

- ```rust
  pub fn lerp(a: Self, b: Self, t: f64) -> Self { /* ... */ }
  ```
  Linear interpolation

###### Trait Implementations

- **Add**
  - ```rust
    fn add(self: Self, r: Self) -> Self { /* ... */ }
    ```

  - ```rust
    fn add(self: Self, st: SphericalTensor) -> Self { /* ... */ }
    ```

  - ```rust
    fn add(self: Self, t: Tensor) -> Tensor { /* ... */ }
    ```

  - ```rust
    fn add(self: Self, st: SymmTensor) -> Self { /* ... */ }
    ```

  - ```rust
    fn add(self: Self, t: Tensor) -> Tensor { /* ... */ }
    ```

- **AddAssign**
  - ```rust
    fn add_assign(self: &mut Self, r: Self) { /* ... */ }
    ```

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Tensor { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Tensor { /* ... */ }
    ```

- **Div**
  - ```rust
    fn div(self: Self, s: f64) -> Self { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(st: SphericalTensor) -> Self { /* ... */ }
    ```

  - ```rust
    fn from(st: SymmTensor) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Mul**
  - ```rust
    fn mul(self: Self, s: f64) -> Self { /* ... */ }
    ```

  - ```rust
    fn mul(self: Self, t: Tensor) -> Tensor { /* ... */ }
    ```

  - ```rust
    fn mul(self: Self, t: Tensor) -> Tensor { /* ... */ }
    ```

  - ```rust
    fn mul(self: Self, st: SphericalTensor) -> Self { /* ... */ }
    ```

  - ```rust
    fn mul(self: Self, t: Tensor) -> Tensor { /* ... */ }
    ```

  - ```rust
    fn mul(self: Self, st: SymmTensor) -> Self { /* ... */ }
    ```

- **MulAssign**
  - ```rust
    fn mul_assign(self: &mut Self, s: f64) { /* ... */ }
    ```

- **Neg**
  - ```rust
    fn neg(self: Self) -> Self { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Tensor) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sub**
  - ```rust
    fn sub(self: Self, r: Self) -> Self { /* ... */ }
    ```

  - ```rust
    fn sub(self: Self, st: SphericalTensor) -> Self { /* ... */ }
    ```

  - ```rust
    fn sub(self: Self, t: Tensor) -> Tensor { /* ... */ }
    ```

  - ```rust
    fn sub(self: Self, st: SymmTensor) -> Self { /* ... */ }
    ```

  - ```rust
    fn sub(self: Self, t: Tensor) -> Tensor { /* ... */ }
    ```

- **SubAssign**
  - ```rust
    fn sub_assign(self: &mut Self, r: Self) { /* ... */ }
    ```

- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `hodge_dual_of_vec`

Hodge dual of a Vector as a skew-symmetric Tensor. C++ `operator*(Vector)`.

```rust
pub fn hodge_dual_of_vec(v: super::vector::Vector3) -> Tensor { /* ... */ }
```

#### Function `tr`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Trace tr(T) = xx + yy + zz.

```rust
pub fn tr(t: Tensor) -> f64 { /* ... */ }
```

#### Function `det`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Determinant det(T).

```rust
pub fn det(t: Tensor) -> f64 { /* ... */ }
```

#### Function `inv`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Inverse T⁻¹ (panics in debug builds if singular).

```rust
pub fn inv(t: Tensor) -> Tensor { /* ... */ }
```

#### Function `symm`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Symmetric part `0.5*(T + Tᵀ)`.

```rust
pub fn symm(t: Tensor) -> super::symm_tensor::SymmTensor { /* ... */ }
```

#### Function `two_symm`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Twice the symmetric part `T + Tᵀ`.

```rust
pub fn two_symm(t: Tensor) -> super::symm_tensor::SymmTensor { /* ... */ }
```

#### Function `skew`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Skew-symmetric part `0.5*(T - Tᵀ)`.

```rust
pub fn skew(t: Tensor) -> Tensor { /* ... */ }
```

#### Function `dev`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Deviatoric part `T - (tr/3)*I`.

```rust
pub fn dev(t: Tensor) -> Tensor { /* ... */ }
```

#### Function `dev2`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Two-thirds deviatoric `T - (2*tr/3)*I`.

```rust
pub fn dev2(t: Tensor) -> Tensor { /* ... */ }
```

#### Function `dev_symm`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Deviatoric of the symmetric part `symm(T) - (tr/3)*I`.

```rust
pub fn dev_symm(t: Tensor) -> super::symm_tensor::SymmTensor { /* ... */ }
```

#### Function `dev_two_symm`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Deviatoric of twice the symmetric part `twoSymm(T) - (2*tr/3)*I`.

```rust
pub fn dev_two_symm(t: Tensor) -> super::symm_tensor::SymmTensor { /* ... */ }
```

#### Function `lerp`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Linear interpolation `(1-t)*a + t*b` between two tensors.

```rust
pub fn lerp(a: Tensor, b: Tensor, t: f64) -> Tensor { /* ... */ }
```

#### Function `outer`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Outer product v ⊗ w. Same as `v * w` but as a named function.

```rust
pub fn outer(v: super::vector::Vector3, w: super::vector::Vector3) -> Tensor { /* ... */ }
```

## Module `vector`

3-component vector (`Vector3`).
3-component vector (`Vector3`) and its OpenFOAM-style operators.

Components are dimensionless `f64`; the same type is reused for positions,
velocities, forces, etc. with the physical dimension carried by the caller.

```rust
pub mod vector { /* ... */ }
```

### Types

#### Struct `Vector3`

3-component vector. Maps to `Foam::vector` (`Foam::Vector<scalar>`).
Component layout: x, y, z.

```rust
pub struct Vector3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `f64` |  |
| `y` | `f64` |  |
| `z` | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn new(x: f64, y: f64, z: f64) -> Self { /* ... */ }
  ```
  Construct a vector from its x, y, z components.

- ```rust
  pub fn mag_sqr(self: Self) -> f64 { /* ... */ }
  ```
  Squared magnitude: |v|² = x² + y² + z²

- ```rust
  pub fn mag(self: Self) -> f64 { /* ... */ }
  ```
  Magnitude: |v|

- ```rust
  pub fn dist_sqr(self: Self, other: Self) -> f64 { /* ... */ }
  ```
  Squared distance to another vector

- ```rust
  pub fn dist(self: Self, other: Self) -> f64 { /* ... */ }
  ```
  Distance to another vector

- ```rust
  pub fn dot(self: Self, other: Self) -> f64 { /* ... */ }
  ```
  Dot (inner) product. C++ `operator&(Vector, Vector)`.

- ```rust
  pub fn cross(self: Self, other: Self) -> Self { /* ... */ }
  ```
  Cross product. C++ `operator^(Vector, Vector)`.

- ```rust
  pub fn normalise(self: Self, tol: f64) -> Self { /* ... */ }
  ```
  Normalise to unit vector; returns zero if `|v| < tol`.

- ```rust
  pub fn remove_collinear(self: Self, unit_vec: Self) -> Self { /* ... */ }
  ```
  Remove the component collinear with `unit_vec`: `self - (self·unit) * unit`.

- ```rust
  pub fn lerp(a: Self, b: Self, t: f64) -> Self { /* ... */ }
  ```
  Linear interpolation: `(1-t)*a + t*b`.

###### Trait Implementations

- **Add**
  - ```rust
    fn add(self: Self, r: Self) -> Self { /* ... */ }
    ```

- **AddAssign**
  - ```rust
    fn add_assign(self: &mut Self, r: Self) { /* ... */ }
    ```

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Vector3 { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Vector3 { /* ... */ }
    ```

- **Div**
  - ```rust
    fn div(self: Self, s: f64) -> Self { /* ... */ }
    ```

- **DivAssign**
  - ```rust
    fn div_assign(self: &mut Self, s: f64) { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Mul**
  - ```rust
    fn mul(self: Self, v: Vector3) -> Tensor { /* ... */ }
    ```

  - ```rust
    fn mul(self: Self, s: f64) -> Self { /* ... */ }
    ```

  - ```rust
    fn mul(self: Self, v: Vector3) -> Vector3 { /* ... */ }
    ```

- **MulAssign**
  - ```rust
    fn mul_assign(self: &mut Self, s: f64) { /* ... */ }
    ```

- **Neg**
  - ```rust
    fn neg(self: Self) -> Self { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Vector3) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sub**
  - ```rust
    fn sub(self: Self, r: Self) -> Self { /* ... */ }
    ```

- **SubAssign**
  - ```rust
    fn sub_assign(self: &mut Self, r: Self) { /* ... */ }
    ```

- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Functions

#### Function `mag_sqr`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Squared magnitude |v|² of a vector.

```rust
pub fn mag_sqr(v: Vector3) -> f64 { /* ... */ }
```

#### Function `mag`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Magnitude |v| of a vector.

```rust
pub fn mag(v: Vector3) -> f64 { /* ... */ }
```

#### Function `dot`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Dot product. C++ `operator&`.

```rust
pub fn dot(a: Vector3, b: Vector3) -> f64 { /* ... */ }
```

#### Function `cross`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Cross product. C++ `operator^`.

```rust
pub fn cross(a: Vector3, b: Vector3) -> Vector3 { /* ... */ }
```

#### Function `lerp`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Linear interpolation `(1-t)*a + t*b` between two vectors.

```rust
pub fn lerp(a: Vector3, b: Vector3, t: f64) -> Vector3 { /* ... */ }
```

### Re-exports

#### Re-export `eigen_values`

```rust
pub use eigen::eigen_values;
```

#### Re-export `eigen_values_checked`

```rust
pub use eigen::eigen_values_checked;
```

#### Re-export `eigen_values_symm`

```rust
pub use eigen::eigen_values_symm;
```

#### Re-export `eigen_vectors`

```rust
pub use eigen::eigen_vectors;
```

#### Re-export `eigen_vectors_symm`

```rust
pub use eigen::eigen_vectors_symm;
```

#### Re-export `eigen_vectors_symm_with`

```rust
pub use eigen::eigen_vectors_symm_with;
```

#### Re-export `eigen_vectors_with`

```rust
pub use eigen::eigen_vectors_with;
```

#### Re-export `Label`

```rust
pub use scalar::Label;
```

#### Re-export `Scalar`

```rust
pub use scalar::Scalar;
```

#### Re-export `GREAT`

```rust
pub use scalar::GREAT;
```

#### Re-export `ROOT_GREAT`

```rust
pub use scalar::ROOT_GREAT;
```

#### Re-export `ROOT_SMALL`

```rust
pub use scalar::ROOT_SMALL;
```

#### Re-export `ROOT_VSMALL`

```rust
pub use scalar::ROOT_VSMALL;
```

#### Re-export `SMALL`

```rust
pub use scalar::SMALL;
```

#### Re-export `VGREAT`

```rust
pub use scalar::VGREAT;
```

#### Re-export `VSMALL`

```rust
pub use scalar::VSMALL;
```

#### Re-export `SphericalTensor`

```rust
pub use spherical_tensor::SphericalTensor;
```

#### Re-export `SymmTensor`

```rust
pub use symm_tensor::SymmTensor;
```

#### Re-export `Tensor`

```rust
pub use tensor::Tensor;
```

#### Re-export `Vector3`

```rust
pub use vector::Vector3;
```

## Module `thermophysics`

Layer 1h — specie-level thermophysics: equations of state, thermo, and
transport models.
Specie-level thermophysics: mesh-independent per-species property kernels.

Ports the OpenFOAM `thermophysicalModels/specie` layer. Properties are built
in three stacked layers, each wrapping the one below:
- [`eos`](crate::thermophysics::eos) — equation of state: density ρ, compressibility ψ, compressibility
  factor Z, and enthalpy/entropy/internal-energy departures from `(p, T)`.
- [`thermo`](crate::thermophysics::thermo) — specific heat Cp, enthalpy, entropy, and Newton `T`-inversion.
- [`transport`](crate::thermophysics::transport) — dynamic viscosity μ and thermal conductivity κ.

Supporting modules: [`constants`](crate::thermophysics::constants) (physical
constants), [`error`](crate::thermophysics::error) (the
[`ThermoError`](crate::thermophysics::error::ThermoError) type),
[`quantities`](crate::thermophysics::quantities) (uom type aliases), and
[`imports`](crate::thermophysics::imports) (shared uom re-exports used by
every implementation file).

```rust
pub mod thermophysics { /* ... */ }
```

### Modules

## Module `constants`

```rust
pub mod constants { /* ... */ }
```

### Constants and Statics

#### Constant `R_UNIVERSAL`

Universal gas constant in J/(mol·K).
Using this value with `MolarMass` in kg/mol gives `r = R_UNIVERSAL / W` in J/(kg·K).

```rust
pub const R_UNIVERSAL: f64 = 8.314_462_618_153_24;
```

#### Constant `T_STD`

Standard thermodynamic temperature (used as entropy reference in S = Cp·ln(T/Tstd)).

```rust
pub const T_STD: f64 = 298.15;
```

#### Constant `T_MIN`

Minimum temperature floor used in Newton T-iteration to prevent log(0).

```rust
pub const T_MIN: f64 = 100.0;
```

#### Constant `T_MAX`

Upper JANAF coefficient range limit.

```rust
pub const T_MAX: f64 = 6000.0;
```

#### Constant `P_REF`

Standard-state reference pressure for entropy calculations.

```rust
pub const P_REF: f64 = 101_325.0;
```

## Module `eos`

Per-species equations of state — `(p, T)` → density ρ `[kg/m³]`,
compressibility ψ = ∂ρ/∂p|_T `[s²/m²]`, compressibility factor Z `[-]`, and the
enthalpy / entropy / internal-energy departures from the ideal-gas value.

Each model implements [`EquationOfState`]. Available models: ideal
[`PerfectGas`], constant-density [`RhoConst`], incompressible specific-volume
polynomial [`IcoPolynomial`], and real-gas [`PengRobinsonGas`].

```rust
pub mod eos { /* ... */ }
```

### Modules

## Module `ico_polynomial`

```rust
pub mod ico_polynomial { /* ... */ }
```

### Types

#### Struct `IcoPolynomial`

Incompressible polynomial EOS: `v(T) = poly(T)`, so `ρ = 1 / poly(T)`.

Mirrors `Foam::icoPolynomial<Specie, PolySize>` from
`src/thermophysicalModels/specie/equationOfState/icoPolynomial/`.

The polynomial gives specific volume as a function of T.  ψ = 0 (incompressible).
h_eos = p·v = p/ρ  (enthalpy departure for incompressible EOS).

```rust
pub struct IcoPolynomial<const N: usize> {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(mol_weight: MolarMass, poly: Polynomial<N>) -> Self { /* ... */ }
  ```
  `poly` coefficients give specific volume `[m³/kg]` as a polynomial in T `[K]`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> IcoPolynomial<N> { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **EquationOfState**
  - ```rust
    fn mol_weight(self: &Self) -> MolarMass { /* ... */ }
    ```

  - ```rust
    fn r(self: &Self) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn rho(self: &Self, _p: Pressure, t: ThermodynamicTemperature) -> MassDensity { /* ... */ }
    ```

  - ```rust
    fn psi(self: &Self, _p: Pressure, _t: ThermodynamicTemperature) -> Compressibility { /* ... */ }
    ```

  - ```rust
    fn z(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> Ratio { /* ... */ }
    ```

  - ```rust
    fn cp_m_cv(self: &Self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn cp_eos(self: &Self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn h_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn e_eos(self: &Self, _p: Pressure, _t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn s_eos(self: &Self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `peng_robinson`

```rust
pub mod peng_robinson { /* ... */ }
```

### Types

#### Struct `PengRobinsonGas`

Peng-Robinson (1976) equation of state.

Mirrors `Foam::PengRobinsonGas<Specie>` from
`src/thermophysicalModels/specie/equationOfState/PengRobinsonGas/`.

EOS: `p = R·T/(v−b) − a(T)/(v(v+b)+b(v−b))`

Acentric-factor correlation for κ (valid for ω < 0.49):
```text
κ = 0.37464 + 1.54226·ω − 0.26992·ω²
a(T) = 0.45724·(R·Tc)²/Pc · α(T)
α(T) = (1 + κ·(1 − √(T/Tc)))²
b    = 0.07780·R·Tc/Pc
```

All methods select the **largest real root** of the Z-cubic, which corresponds
to the vapour phase.  For liquid-phase properties use a different root selector.

Formulas match `PengRobinsonGasI.H` with `R = R_specific = R_universal / W`.

```rust
pub struct PengRobinsonGas {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(mol_weight: MolarMass, tc: ThermodynamicTemperature, pc: Pressure, omega: f64) -> Self { /* ... */ }
  ```
  Construct a Peng-Robinson EOS from molar mass W `[kg/mol]`, critical

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> PengRobinsonGas { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **EquationOfState**
  - ```rust
    fn mol_weight(self: &Self) -> MolarMass { /* ... */ }
    ```

  - ```rust
    fn r(self: &Self) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn rho(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> MassDensity { /* ... */ }
    ```

  - ```rust
    fn psi(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> Compressibility { /* ... */ }
    ```
    ψ ≈ 1/(Z·R·T) — OpenFOAM's approximation treating Z as locally constant in p.

  - ```rust
    fn z(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> Ratio { /* ... */ }
    ```

  - ```rust
    fn cp_m_cv(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```
    Cp − Cv for the PR EOS via the Maxwell relation.

  - ```rust
    fn cp_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```
    EOS correction to Cp (departure from ideal-gas Cp).

  - ```rust
    fn h_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```
    Enthalpy departure from ideal gas.

  - ```rust
    fn e_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```
    Internal energy departure: `e_eos = h_eos − R·T·(Z−1)`.

  - ```rust
    fn s_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```
    Entropy departure (includes ideal-gas pressure term `−R·ln(p/p_ref)`).

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `perfect_gas`

```rust
pub mod perfect_gas { /* ... */ }
```

### Types

#### Struct `PerfectGas`

Ideal perfect gas: p = ρ·R·T.

Mirrors `Foam::perfectGas<Specie>` from
`src/thermophysicalModels/specie/equationOfState/perfectGas/`.

```rust
pub struct PerfectGas {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(mol_weight: MolarMass) -> Self { /* ... */ }
  ```
  Construct an ideal perfect-gas EOS from the species molar mass W `[kg/mol]`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> PerfectGas { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **EquationOfState**
  - ```rust
    fn mol_weight(self: &Self) -> MolarMass { /* ... */ }
    ```

  - ```rust
    fn r(self: &Self) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn rho(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> MassDensity { /* ... */ }
    ```

  - ```rust
    fn psi(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> Compressibility { /* ... */ }
    ```

  - ```rust
    fn z(self: &Self, _p: Pressure, _t: ThermodynamicTemperature) -> Ratio { /* ... */ }
    ```

  - ```rust
    fn cp_m_cv(self: &Self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn cp_eos(self: &Self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn h_eos(self: &Self, _p: Pressure, _t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn e_eos(self: &Self, _p: Pressure, _t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn s_eos(self: &Self, p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `rho_const`

```rust
pub mod rho_const { /* ... */ }
```

### Types

#### Struct `RhoConst`

Constant-density (incompressible) equation of state: ρ = const.

Mirrors `Foam::rhoConst<Specie>` from
`src/thermophysicalModels/specie/equationOfState/rhoConst/`.

```rust
pub struct RhoConst {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(mol_weight: MolarMass, rho0: MassDensity) -> Self { /* ... */ }
  ```
  Construct a constant-density EOS from molar mass W `[kg/mol]` and the fixed

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> RhoConst { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **EquationOfState**
  - ```rust
    fn mol_weight(self: &Self) -> MolarMass { /* ... */ }
    ```

  - ```rust
    fn r(self: &Self) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn rho(self: &Self, _p: Pressure, _t: ThermodynamicTemperature) -> MassDensity { /* ... */ }
    ```

  - ```rust
    fn psi(self: &Self, _p: Pressure, _t: ThermodynamicTemperature) -> Compressibility { /* ... */ }
    ```

  - ```rust
    fn z(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> Ratio { /* ... */ }
    ```

  - ```rust
    fn cp_m_cv(self: &Self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn cp_eos(self: &Self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn h_eos(self: &Self, _p: Pressure, _t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn e_eos(self: &Self, _p: Pressure, _t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn s_eos(self: &Self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Re-exports

#### Re-export `ico_polynomial::*`

```rust
pub use ico_polynomial::*;
```

#### Re-export `peng_robinson::*`

```rust
pub use peng_robinson::*;
```

#### Re-export `perfect_gas::*`

```rust
pub use perfect_gas::*;
```

#### Re-export `rho_const::*`

```rust
pub use rho_const::*;
```

#### Re-export `traits::*`

```rust
pub use traits::*;
```

## Module `error`

```rust
pub mod error { /* ... */ }
```

### Types

#### Enum `ThermoError`

Errors produced by the specie-level thermophysics layer.

```rust
pub enum ThermoError {
    NonConvergent {
        max_iter: usize,
        last_t: f64,
    },
}
```

##### Variants

###### `NonConvergent`

Newton T-inversion exhausted all iterations without meeting the
convergence tolerance (|ΔT/T| < 1e-6). Carries the last iterate.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `max_iter` | `usize` |  |
| `last_t` | `f64` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ThermoError { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, f: &mut std::fmt::Formatter<''_>) -> std::fmt::Result { /* ... */ }
    ```

- **Error**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ThermoError) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **ToString**
  - ```rust
    fn to_string(self: &Self) -> String { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `imports`

```rust
pub mod imports { /* ... */ }
```

### Re-exports

#### Re-export `AvailableEnergy`

Common uom re-exports for thermophysics implementation files.

Every EOS / thermo / transport source file starts with
`use crate::thermophysics::imports::*;` instead of repeating the full
type/unit import block.  From outside the crate the same symbols are
reachable as:

```rust
use outram_foam_basic_lib::thermophysics::imports::*;
let p = Pressure::new::<pascal>(101325.0);
assert!(p.get::<pascal>() > 0.0);
```

```rust
pub use uom::si::f64::AvailableEnergy;
```

#### Re-export `DynamicViscosity`

Common uom re-exports for thermophysics implementation files.

Every EOS / thermo / transport source file starts with
`use crate::thermophysics::imports::*;` instead of repeating the full
type/unit import block.  From outside the crate the same symbols are
reachable as:

```rust
use outram_foam_basic_lib::thermophysics::imports::*;
let p = Pressure::new::<pascal>(101325.0);
assert!(p.get::<pascal>() > 0.0);
```

```rust
pub use uom::si::f64::DynamicViscosity;
```

#### Re-export `MassDensity`

Common uom re-exports for thermophysics implementation files.

Every EOS / thermo / transport source file starts with
`use crate::thermophysics::imports::*;` instead of repeating the full
type/unit import block.  From outside the crate the same symbols are
reachable as:

```rust
use outram_foam_basic_lib::thermophysics::imports::*;
let p = Pressure::new::<pascal>(101325.0);
assert!(p.get::<pascal>() > 0.0);
```

```rust
pub use uom::si::f64::MassDensity;
```

#### Re-export `MolarMass`

Common uom re-exports for thermophysics implementation files.

Every EOS / thermo / transport source file starts with
`use crate::thermophysics::imports::*;` instead of repeating the full
type/unit import block.  From outside the crate the same symbols are
reachable as:

```rust
use outram_foam_basic_lib::thermophysics::imports::*;
let p = Pressure::new::<pascal>(101325.0);
assert!(p.get::<pascal>() > 0.0);
```

```rust
pub use uom::si::f64::MolarMass;
```

#### Re-export `Pressure`

Common uom re-exports for thermophysics implementation files.

Every EOS / thermo / transport source file starts with
`use crate::thermophysics::imports::*;` instead of repeating the full
type/unit import block.  From outside the crate the same symbols are
reachable as:

```rust
use outram_foam_basic_lib::thermophysics::imports::*;
let p = Pressure::new::<pascal>(101325.0);
assert!(p.get::<pascal>() > 0.0);
```

```rust
pub use uom::si::f64::Pressure;
```

#### Re-export `Ratio`

Common uom re-exports for thermophysics implementation files.

Every EOS / thermo / transport source file starts with
`use crate::thermophysics::imports::*;` instead of repeating the full
type/unit import block.  From outside the crate the same symbols are
reachable as:

```rust
use outram_foam_basic_lib::thermophysics::imports::*;
let p = Pressure::new::<pascal>(101325.0);
assert!(p.get::<pascal>() > 0.0);
```

```rust
pub use uom::si::f64::Ratio;
```

#### Re-export `SpecificHeatCapacity`

Common uom re-exports for thermophysics implementation files.

Every EOS / thermo / transport source file starts with
`use crate::thermophysics::imports::*;` instead of repeating the full
type/unit import block.  From outside the crate the same symbols are
reachable as:

```rust
use outram_foam_basic_lib::thermophysics::imports::*;
let p = Pressure::new::<pascal>(101325.0);
assert!(p.get::<pascal>() > 0.0);
```

```rust
pub use uom::si::f64::SpecificHeatCapacity;
```

#### Re-export `ThermalConductivity`

Common uom re-exports for thermophysics implementation files.

Every EOS / thermo / transport source file starts with
`use crate::thermophysics::imports::*;` instead of repeating the full
type/unit import block.  From outside the crate the same symbols are
reachable as:

```rust
use outram_foam_basic_lib::thermophysics::imports::*;
let p = Pressure::new::<pascal>(101325.0);
assert!(p.get::<pascal>() > 0.0);
```

```rust
pub use uom::si::f64::ThermalConductivity;
```

#### Re-export `ThermodynamicTemperature`

Common uom re-exports for thermophysics implementation files.

Every EOS / thermo / transport source file starts with
`use crate::thermophysics::imports::*;` instead of repeating the full
type/unit import block.  From outside the crate the same symbols are
reachable as:

```rust
use outram_foam_basic_lib::thermophysics::imports::*;
let p = Pressure::new::<pascal>(101325.0);
assert!(p.get::<pascal>() > 0.0);
```

```rust
pub use uom::si::f64::ThermodynamicTemperature;
```

#### Re-export `joule_per_kilogram`

```rust
pub use uom::si::available_energy::joule_per_kilogram;
```

#### Re-export `pascal_second`

```rust
pub use uom::si::dynamic_viscosity::pascal_second;
```

#### Re-export `kilogram_per_cubic_meter`

```rust
pub use uom::si::mass_density::kilogram_per_cubic_meter;
```

#### Re-export `gram_per_mole`

```rust
pub use uom::si::molar_mass::gram_per_mole;
```

#### Re-export `kilogram_per_mole`

```rust
pub use uom::si::molar_mass::kilogram_per_mole;
```

#### Re-export `pascal`

```rust
pub use uom::si::pressure::pascal;
```

#### Re-export `ratio`

```rust
pub use uom::si::ratio::ratio;
```

#### Re-export `joule_per_kilogram_kelvin`

```rust
pub use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
```

#### Re-export `watt_per_meter_kelvin`

```rust
pub use uom::si::thermal_conductivity::watt_per_meter_kelvin;
```

#### Re-export `kelvin`

```rust
pub use uom::si::thermodynamic_temperature::kelvin;
```

#### Re-export `Compressibility`

```rust
pub use crate::thermophysics::quantities::Compressibility;
```

## Module `quantities`

```rust
pub mod quantities { /* ... */ }
```

### Types

#### Type Alias `Compressibility`

Compressibility ψ = ∂ρ/∂p|_T  —  SI units: s²/m²  (L⁻²·T²)

Computed as `MassDensity / Pressure` via uom operator arithmetic; this type
alias names the resulting quantity so trait signatures are readable.

```rust
pub type Compressibility = uom::si::Quantity<uom::si::ISQ<uom::typenum::N2, uom::typenum::Z0, uom::typenum::P2, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0, uom::typenum::Z0>, uom::si::SI<f64>, f64>;
```

## Module `thermo`

Per-species thermodynamic models — specific heat Cp `[J/(kg·K)]`,
sensible/absolute specific enthalpy `[J/kg]`, specific entropy `[J/(kg·K)]`, and
Newton `T`-inversion, layered on top of an
[`EquationOfState`](crate::thermophysics::eos::EquationOfState).

Each model implements [`ThermoModel`]. Available models: constant-Cp
[`HConstThermo`], polynomial-Cp [`HPolynomialThermo`], tabulated
[`HTabulatedThermo`], and NASA-7 (JANAF) [`JanafThermo`].

```rust
pub mod thermo { /* ... */ }
```

### Modules

## Module `h_const`

```rust
pub mod h_const { /* ... */ }
```

### Types

#### Struct `HConstThermo`

Constant-Cp thermodynamic model.

Mirrors `Foam::hConstThermo<EOS>` from
`src/thermophysicalModels/specie/thermo/hConst/`.

Formulas (following OpenFOAM `hConstThermoI.H`):
```text
Cp(p,T)  = cp_ + EOS::Cp(p,T)
Hs(p,T)  = cp_·(T − tref_) + hsref_ + EOS::H(p,T)
Ha(p,T)  = Hs(p,T) + Hf_
S(p,T)   = cp_·ln(T / T_std) + EOS::S(p,T)
```

```rust
pub struct HConstThermo<E: EquationOfState> {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(eos: E, cp: SpecificHeatCapacity, hf: AvailableEnergy, tref: ThermodynamicTemperature, hsref: AvailableEnergy) -> Self { /* ... */ }
  ```
  Construct a constant-Cp thermo model wrapping `eos`, with heat capacity

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> HConstThermo<E> { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **EquationOfState**
  - ```rust
    fn mol_weight(self: &Self) -> MolarMass { /* ... */ }
    ```

  - ```rust
    fn r(self: &Self) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn rho(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> MassDensity { /* ... */ }
    ```

  - ```rust
    fn psi(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> Compressibility { /* ... */ }
    ```

  - ```rust
    fn z(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> Ratio { /* ... */ }
    ```

  - ```rust
    fn cp_m_cv(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn cp_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn h_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn e_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn s_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ThermoModel**
  - ```rust
    fn cp(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn hs(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn ha(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn hc(self: &Self) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn s(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `h_polynomial`

```rust
pub mod h_polynomial { /* ... */ }
```

### Types

#### Struct `HPolynomialThermo`

Polynomial Cp thermodynamic model.

Mirrors `Foam::hPolynomialThermo<EOS, PolySize>` from
`src/thermophysicalModels/specie/thermo/hPolynomial/`.

Formulas (matching `hPolynomialThermoI.H`):
```text
Cp(p,T) = cps.value(T) + EOS::Cp(p,T)
Ha(p,T) = hf + cps.integral(T_std, T) + EOS::H(p,T)
Hc()    = hf
Hs(p,T) = Ha(p,T) − Hc()
S(p,T)  = sf + cps.integral_minus1(0).value(T)
              − cps.integral_minus1(0).value(T_std)
              + EOS::S(p,T)
```
where `T_std = 298.15 K` and `cps.integral_minus1(0)` is the antiderivative
of `Cp/T` (activating the `log_coeff·ln(T)` term).

```rust
pub struct HPolynomialThermo<E: EquationOfState, const N: usize> {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(eos: E, cps: Polynomial<N>, hf: AvailableEnergy, sf: SpecificHeatCapacity) -> Self { /* ... */ }
  ```
  Construct a polynomial-Cp thermo model wrapping `eos`, with the Cp(T)

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> HPolynomialThermo<E, N> { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **EquationOfState**
  - ```rust
    fn mol_weight(self: &Self) -> MolarMass { /* ... */ }
    ```

  - ```rust
    fn r(self: &Self) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn rho(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> MassDensity { /* ... */ }
    ```

  - ```rust
    fn psi(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> Compressibility { /* ... */ }
    ```

  - ```rust
    fn z(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> Ratio { /* ... */ }
    ```

  - ```rust
    fn cp_m_cv(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn cp_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn h_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn e_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn s_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ThermoModel**
  - ```rust
    fn cp(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn ha(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn hs(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn hc(self: &Self) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn s(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `h_tabulated`

```rust
pub mod h_tabulated { /* ... */ }
```

### Types

#### Struct `HTabulatedThermo`

Tabulated thermodynamic model: Cp, Ha, and S stored as (T, value) lookup tables.

Mirrors `Foam::hTabulatedThermo<EOS>` from
`src/thermophysicalModels/specie/thermo/hTabulated/`.

All three tables use `interpolate_xy` (piecewise-linear, clamped at endpoints).
Separate temperature grids may be provided for each property.

`ha_table` should contain **absolute** enthalpy values (sensible + formation)
at each temperature.  `hc()` returns `hf` separately so that `hs = ha - hf`.

```rust
pub struct HTabulatedThermo<E: EquationOfState> {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(eos: E, cp_table: (Vec<f64>, Vec<f64>), ha_table: (Vec<f64>, Vec<f64>), s_table: (Vec<f64>, Vec<f64>), hf: AvailableEnergy) -> Self { /* ... */ }
  ```
  Construct with separate (T, value) tables for Cp, Ha, and S.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> HTabulatedThermo<E> { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **EquationOfState**
  - ```rust
    fn mol_weight(self: &Self) -> MolarMass { /* ... */ }
    ```

  - ```rust
    fn r(self: &Self) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn rho(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> MassDensity { /* ... */ }
    ```

  - ```rust
    fn psi(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> Compressibility { /* ... */ }
    ```

  - ```rust
    fn z(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> Ratio { /* ... */ }
    ```

  - ```rust
    fn cp_m_cv(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn cp_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn h_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn e_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn s_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ThermoModel**
  - ```rust
    fn cp(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn ha(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn hs(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn hc(self: &Self) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn s(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `janaf`

```rust
pub mod janaf { /* ... */ }
```

### Types

#### Struct `JanafThermo`

NASA 7-coefficient (JANAF) thermodynamic polynomial.

Mirrors `Foam::janafThermo<EOS>` from
`src/thermophysicalModels/specie/thermo/janaf/`.

Coefficients are stored **pre-scaled by R** (i.e. stored as R·a_i), so
polynomials directly return J/(kg·K) or J/kg without an extra R factor.

Dual temperature range: `low` coefficients apply for T < tcommon,
`high` for T >= tcommon.

Polynomial formulas (matching `janafThermoI.H`):
```text
Cp  = (((a[4]·T + a[3])·T + a[2])·T + a[1])·T + a[0]  + EOS::Cp
Ha  = ((((a[4]/5·T + a[3]/4)·T + a[2]/3)·T + a[1]/2)·T + a[0])·T + a[5]  + EOS::H
S   = (((a[4]/4·T + a[3]/3)·T + a[2]/2)·T + a[1])·T + a[0]·ln(T) + a[6]  + EOS::S
Hc  = Ha evaluated at T_std using low coefficients
Hs  = Ha − Hc
```

```rust
pub struct JanafThermo<E: EquationOfState> {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(eos: E, tlow: f64, thigh: f64, tcommon: f64, low: [f64; 7], high: [f64; 7]) -> Self { /* ... */ }
  ```
  Construct a NASA-7 (JANAF) thermo model wrapping `eos`, valid over

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> JanafThermo<E> { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **EquationOfState**
  - ```rust
    fn mol_weight(self: &Self) -> MolarMass { /* ... */ }
    ```

  - ```rust
    fn r(self: &Self) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn rho(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> MassDensity { /* ... */ }
    ```

  - ```rust
    fn psi(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> Compressibility { /* ... */ }
    ```

  - ```rust
    fn z(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> Ratio { /* ... */ }
    ```

  - ```rust
    fn cp_m_cv(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn cp_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn h_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn e_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn s_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ThermoModel**
  - ```rust
    fn cp(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn ha(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn hs(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn hc(self: &Self) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn s(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Re-exports

#### Re-export `h_const::*`

```rust
pub use h_const::*;
```

#### Re-export `h_polynomial::*`

```rust
pub use h_polynomial::*;
```

#### Re-export `h_tabulated::*`

```rust
pub use h_tabulated::*;
```

#### Re-export `janaf::*`

```rust
pub use janaf::*;
```

#### Re-export `traits::*`

```rust
pub use traits::*;
```

## Module `transport`

Per-species transport models — dynamic viscosity μ `[Pa·s]` and thermal
conductivity κ `[W/(m·K)]`, layered on top of a
[`ThermoModel`](crate::thermophysics::thermo::ThermoModel).

Each model implements [`TransportModel`]. Available models: constant-μ /
constant-Prandtl [`ConstTransport`], polynomial [`PolynomialTransport`],
Sutherland's-law [`SutherlandTransport`], and tabulated
[`TabulatedTransport`].

```rust
pub mod transport { /* ... */ }
```

### Modules

## Module `const_transport`

```rust
pub mod const_transport { /* ... */ }
```

### Types

#### Struct `ConstTransport`

Constant-viscosity / constant-Prandtl-number transport model.

Mirrors `Foam::constTransport<Thermo>` from
`src/thermophysicalModels/specie/transport/const/`.

Fields: `mu_` (constant dynamic viscosity), `rPr_` (1/Pr, reciprocal Prandtl).
```text
mu(p,T)    = mu_
kappa(p,T) = Cp(p,T) · mu_ / Pr  = Cp · mu_ · rPr_
alphah     = kappa / Cp = mu_ · rPr_       (default from TransportModel)
```

```rust
pub struct ConstTransport<T: ThermoModel> {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(thermo: T, mu: DynamicViscosity, pr: Ratio) -> Self { /* ... */ }
  ```
  Construct a constant-viscosity / constant-Prandtl transport model wrapping

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> ConstTransport<T> { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **EquationOfState**
  - ```rust
    fn mol_weight(self: &Self) -> MolarMass { /* ... */ }
    ```

  - ```rust
    fn r(self: &Self) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn rho(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> MassDensity { /* ... */ }
    ```

  - ```rust
    fn psi(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> Compressibility { /* ... */ }
    ```

  - ```rust
    fn z(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> Ratio { /* ... */ }
    ```

  - ```rust
    fn cp_m_cv(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn cp_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn h_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn e_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn s_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ThermoModel**
  - ```rust
    fn cp(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn ha(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn hs(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn hc(self: &Self) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn s(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TransportModel**
  - ```rust
    fn mu(self: &Self, _p: Pressure, _t: ThermodynamicTemperature) -> DynamicViscosity { /* ... */ }
    ```

  - ```rust
    fn kappa(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> ThermalConductivity { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `polynomial`

```rust
pub mod polynomial { /* ... */ }
```

### Types

#### Struct `PolynomialTransport`

Polynomial transport model: μ(T) and κ(T) evaluated from `Polynomial<N>`.

Mirrors `Foam::polynomialTransport<Thermo, PolySize>` from
`src/thermophysicalModels/specie/transport/polynomial/`.

Both mu and kappa are independent polynomials in T `[K]`, returning Pa·s and
W/(m·K) respectively.  The same degree N is used for both.

```rust
pub struct PolynomialTransport<T: ThermoModel, const N: usize> {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(thermo: T, mu_poly: Polynomial<N>, kappa_poly: Polynomial<N>) -> Self { /* ... */ }
  ```
  Construct a polynomial transport model wrapping `thermo`, with μ(T)

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> PolynomialTransport<T, N> { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **EquationOfState**
  - ```rust
    fn mol_weight(self: &Self) -> MolarMass { /* ... */ }
    ```

  - ```rust
    fn r(self: &Self) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn rho(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> MassDensity { /* ... */ }
    ```

  - ```rust
    fn psi(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> Compressibility { /* ... */ }
    ```

  - ```rust
    fn z(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> Ratio { /* ... */ }
    ```

  - ```rust
    fn cp_m_cv(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn cp_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn h_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn e_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn s_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ThermoModel**
  - ```rust
    fn cp(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn ha(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn hs(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn hc(self: &Self) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn s(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TransportModel**
  - ```rust
    fn mu(self: &Self, _p: Pressure, t: ThermodynamicTemperature) -> DynamicViscosity { /* ... */ }
    ```

  - ```rust
    fn kappa(self: &Self, _p: Pressure, t: ThermodynamicTemperature) -> ThermalConductivity { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `sutherland`

```rust
pub mod sutherland { /* ... */ }
```

### Types

#### Struct `SutherlandTransport`

Sutherland's law viscosity model.

Mirrors `Foam::sutherlandTransport<Thermo>` from
`src/thermophysicalModels/specie/transport/sutherland/`.

```text
μ(T)  = As · √T / (1 + Ts/T)
κ(p,T) = μ(T) · Cv(p,T) · (1.32 + 1.77 · R/Cv(p,T))    [Eucken relation]
```

`As` has implicit SI units kg/(m·s·K^½) and `Ts` is in K.
Both are stored as raw f64 rather than custom uom quantities.

```rust
pub struct SutherlandTransport<T: ThermoModel> {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(thermo: T, as_: f64, ts: f64) -> Self { /* ... */ }
  ```
  Construct directly from Sutherland coefficients As `[kg/(m·s·K^0.5)]` and Ts `[K]`.

- ```rust
  pub fn from_two_points(thermo: T, mu1: DynamicViscosity, t1: ThermodynamicTemperature, mu2: DynamicViscosity, t2: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  Construct from two viscosity reference points (μ₁, T₁) and (μ₂, T₂).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> SutherlandTransport<T> { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **EquationOfState**
  - ```rust
    fn mol_weight(self: &Self) -> MolarMass { /* ... */ }
    ```

  - ```rust
    fn r(self: &Self) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn rho(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> MassDensity { /* ... */ }
    ```

  - ```rust
    fn psi(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> Compressibility { /* ... */ }
    ```

  - ```rust
    fn z(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> Ratio { /* ... */ }
    ```

  - ```rust
    fn cp_m_cv(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn cp_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn h_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn e_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn s_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ThermoModel**
  - ```rust
    fn cp(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn ha(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn hs(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn hc(self: &Self) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn s(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TransportModel**
  - ```rust
    fn mu(self: &Self, _p: Pressure, t: ThermodynamicTemperature) -> DynamicViscosity { /* ... */ }
    ```

  - ```rust
    fn kappa(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> ThermalConductivity { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
## Module `tabulated`

```rust
pub mod tabulated { /* ... */ }
```

### Types

#### Struct `TabulatedTransport`

Tabulated transport model: μ(T) and κ(T) stored as (T, value) lookup tables.

Mirrors `Foam::tabulatedTransport<Thermo>` from
`src/thermophysicalModels/specie/transport/tabulated/`.

Both tables use `interpolate_xy` (piecewise-linear, clamped at endpoints).
Separate temperature grids may be provided for μ and κ.

```rust
pub struct TabulatedTransport<T: ThermoModel> {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(thermo: T, mu_table: (Vec<f64>, Vec<f64>), kappa_table: (Vec<f64>, Vec<f64>)) -> Self { /* ... */ }
  ```
  `mu_table` = `(temperatures_K, viscosities_Pa_s)`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> TabulatedTransport<T> { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **EquationOfState**
  - ```rust
    fn mol_weight(self: &Self) -> MolarMass { /* ... */ }
    ```

  - ```rust
    fn r(self: &Self) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn rho(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> MassDensity { /* ... */ }
    ```

  - ```rust
    fn psi(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> Compressibility { /* ... */ }
    ```

  - ```rust
    fn z(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> Ratio { /* ... */ }
    ```

  - ```rust
    fn cp_m_cv(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn cp_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn h_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn e_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn s_eos(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Same**
- **Send**
- **Sync**
- **ThermoModel**
  - ```rust
    fn cp(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

  - ```rust
    fn ha(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn hs(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn hc(self: &Self) -> AvailableEnergy { /* ... */ }
    ```

  - ```rust
    fn s(self: &Self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity { /* ... */ }
    ```

- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TransportModel**
  - ```rust
    fn mu(self: &Self, _p: Pressure, t: ThermodynamicTemperature) -> DynamicViscosity { /* ... */ }
    ```

  - ```rust
    fn kappa(self: &Self, _p: Pressure, t: ThermodynamicTemperature) -> ThermalConductivity { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
### Re-exports

#### Re-export `const_transport::*`

```rust
pub use const_transport::*;
```

#### Re-export `polynomial::*`

```rust
pub use polynomial::*;
```

#### Re-export `sutherland::*`

```rust
pub use sutherland::*;
```

#### Re-export `tabulated::*`

```rust
pub use tabulated::*;
```

#### Re-export `traits::*`

```rust
pub use traits::*;
```

## Module `interface`

this part is extension in Rust
Now under here, I want to expose the openfoam primitives to something
that can be human readable

Also useful add-ons for the underlying libraries are put here,
eg. generating one dimensional meshes for system code type simulations
in TAMPINES
User-facing helpers for building meshes and fields without hand-assembling
the low-level [`crate::mesh`] and [`crate::fields`] data structures.

Currently this provides
[`one_dimensional_meshing`](crate::interface::one_dimensional_meshing), a generator for the
uniform 1-D pipe meshes used by pipe-flow and steam-table (e.g. Marviken)
simulations.

```rust
pub mod interface { /* ... */ }
```

### Modules

## Module `one_dimensional_meshing`

now, for the TAMPINES steam tables Marviken test,
and other pipe simulations, I will often need to make
one dimensional meshes straight off the bat,



```rust
pub mod one_dimensional_meshing { /* ... */ }
```

### Functions

#### Function `create_one_d_mesh`

Creates a uniform 1-D finite-volume mesh along the x-axis.

Produces `number_of_cells` equal-width cells spanning x ∈ \[0, `l`\] with a
constant cross-sectional area of `xs_area`.  All geometry is aligned with
the x-axis; y and z components are zero everywhere.

## Layout
```text
|  cell 0  |  cell 1  |  …  |  cell n-1  |
^          ^          ^     ^             ^
left       i-face 0   …   i-face n-2    right
(patch)                                 (patch)
```

Face ordering follows the OpenFOAM convention:
- `[0, n-1)` — internal faces (face `i` separates cell `i` from cell `i+1`)
- face `n-1` — `"right"` boundary at x = `l`  (outward normal = +x)
- face `n`   — `"left"`  boundary at x = 0   (outward normal = −x)

Both patches are typed [`PatchKind::Patch`] (generic).  Replace them via
[`FvMesh::patches`] if you need `Wall`, `Cyclic`, etc.

## Parameters
- `l`               — total pipe length \[m\]
- `xs_area`         — constant cross-sectional area \[m²\]
- `number_of_cells` — number of cells; must be ≥ 1

## Errors
Returns `Err` if `number_of_cells < 1`.

## Example
```rust
use uom::si::f64::*;
use uom::si::length::meter;
use uom::si::area::square_meter;
use outram_foam_basic_lib::interface::one_dimensional_meshing::create_one_d_mesh;

let mesh = create_one_d_mesh(
    Length::new::<meter>(1.0),
    Area::new::<square_meter>(0.01),
    10,
).unwrap();

assert_eq!(mesh.n_cells, 10);
assert_eq!(mesh.n_internal_faces, 9);
assert_eq!(mesh.n_faces, 11);
```

```rust
pub fn create_one_d_mesh(l: Length, xs_area: Area, number_of_cells: i64) -> Result<crate::mesh::FvMesh, crate::mesh::MeshError> { /* ... */ }
```

