# Crate Documentation

**Version:** 0.0.0

**Format Version:** 61

# Module `outram_park_fork_offbeat`

# OUTRAM PARK fork of OFFBEAT — nuclear fuel-performance

A pure-Rust translation of [OFFBEAT](https://gitlab.com/foam-for-nuclear/offbeat),
the multi-dimensional finite-volume fuel-behaviour solver, onto the
OUTRAM-FOAM finite-volume substrate in `outram-foam-basic-lib`.

## What a fuel-performance code computes

Given a fuel rod (or a TRISO particle) and its irradiation history — linear
power against time, coolant conditions, fast-neutron flux — it predicts the
**thermal and mechanical state of the fuel through life**: temperature
distribution, fuel and cladding deformation, the closure of the fuel/cladding
gap, the stress in the cladding, the pressure of released fission gas, and
ultimately whether the cladding (or a TRISO coating layer) fails.

The physics is a strongly coupled loop. Power deposits heat; heat sets the
temperature field; temperature drives thermal expansion, creep and swelling;
those deformations close the gap; gap closure changes the gap conductance,
which changes the temperature field again.

## What belongs in this crate, and what does not

**Belongs here:** the fuel-performance physics — solid mechanics
([`mechanics`]), constitutive laws ([`rheology`]), material property
correlations ([`materials`]), gap and contact ([`gap`]), burnup accumulation
([`burnup`]), fission-gas release ([`fgr`]) and cladding corrosion
([`corrosion`]).

**Does not belong here:** the finite-volume machinery itself. Meshes, fields,
`fvc`/`fvm` operators, the LDU matrix and the Krylov solvers all come from
`outram-foam-basic-lib` and are used, never re-implemented. Neutron transport
and cross-section data live in `outram-mc-libs` and `njoy-outram-park-fork`.

## Units

Public constructors and results are typed with `uom` where a caller supplies
or consumes a physical quantity. The inner numerical loops carry **raw `f64`
in strict SI** — temperature in kelvin, stress in pascal, length in metre,
burnup in MWd/kgHM unless a specific item documents otherwise — because the
correlations are dense and per-cell, and `uom` round-trips inside a cell loop
cost more than they buy. Every such raw-`f64` boundary says so in its doc
comment.

## Status

**Scaffold / early.** This crate has had no human verification or validation.
Per `RESPONSIBLE_USE.md`, AI-assisted output is untrusted draft material until
a human reviews it. Nothing here may be described as validated.

## Provenance

OFFBEAT is GPL-3.0, the same licence as this workspace. Each ported module
keeps an attribution header naming the upstream file it derives from. The
upstream tree is **not** vendored into this repository — porting is done from
a read-only clone kept outside the working tree.

## Modules

## Module `error`

Error type for the fuel-performance crate.

```rust
pub mod error { /* ... */ }
```

### Types

#### Enum `OffbeatError`

Everything that can go wrong in a fuel-performance calculation.

Fuel-performance runs fail in a small number of characteristic ways, and
each variant below names one of them so a caller can react rather than
merely log. Correlation ranges matter here: material correlations are fits
to experimental data over a stated range, and silently extrapolating one
(say, a UO2 conductivity fit far above its melting point) produces a number
that looks plausible and is meaningless. This crate reports that as
[`OffbeatError::OutOfRange`] rather than returning the extrapolated value.

```rust
pub enum OffbeatError {
    OutOfRange {
        quantity: &'static str,
        value: f64,
        low: f64,
        high: f64,
        unit: &'static str,
    },
    Unphysical {
        quantity: &'static str,
        value: f64,
        unit: &'static str,
        reason: &'static str,
    },
    MechanicsNotConverged {
        residual: f64,
        tolerance: f64,
        iterations: usize,
    },
    ConstitutiveNotConverged {
        cell: usize,
        residual: f64,
        iterations: usize,
    },
    UnknownModel {
        category: &'static str,
        name: String,
    },
    Mesh(String),
    NotImplemented(&'static str),
}
```

##### Variants

###### `OutOfRange`

A material correlation was evaluated outside the range it was fitted
over.

`quantity` names what was being evaluated (e.g. `"UO2 conductivity"`),
`value` is the offending input, `low`/`high` the validity bounds, and
`unit` the SI unit the three numbers are expressed in.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `quantity` | `&'static str` | What was being evaluated. |
| `value` | `f64` | The offending input value. |
| `low` | `f64` | Lower bound of validity. |
| `high` | `f64` | Upper bound of validity. |
| `unit` | `&'static str` | SI unit of `value`, `low` and `high`. |

###### `Unphysical`

A physically impossible input was supplied — a negative absolute
temperature, a porosity outside `[0, 1]`, a negative density.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `quantity` | `&'static str` | What was being evaluated. |
| `value` | `f64` | The offending input value. |
| `unit` | `&'static str` | SI unit of `value`. |
| `reason` | `&'static str` | Why the value is impossible. |

###### `MechanicsNotConverged`

The mechanics solve did not converge within the iteration budget.

`residual` is the final scaled residual and `tolerance` the target it
failed to reach, after `iterations` outer iterations.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `residual` | `f64` | Final scaled residual. |
| `tolerance` | `f64` | Convergence target. |
| `iterations` | `usize` | Outer iterations performed. |

###### `ConstitutiveNotConverged`

The constitutive-law (stress) integration did not converge.

Rate-dependent laws — creep especially — are integrated with a local
Newton iteration per cell; this reports that iteration failing.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `cell` | `usize` | Index of the cell whose local iteration failed. |
| `residual` | `f64` | Final local residual. |
| `iterations` | `usize` | Local iterations performed. |

###### `UnknownModel`

A model was requested by name and no such model is registered.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `category` | `&'static str` | The family the model was looked up in (e.g. `"conductivity"`). |
| `name` | `String` | The name that was not found. |

###### `Mesh`

A mesh or field precondition was violated — mismatched sizes, a missing
region, a zero-volume cell.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `NotImplemented`

A model is declared but not yet implemented in this port.

This is an honest placeholder, not a silent fallback: a caller reaching
this has asked for physics the port does not yet contain.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `&'static str` |  |

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
    fn clone(self: &Self) -> OffbeatError { /* ... */ }
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
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
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
    fn eq(self: &Self, other: &OffbeatError) -> bool { /* ... */ }
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
#### Type Alias `Result`

Convenience alias for fallible fuel-performance operations.

```rust
pub type Result<T> = core::result::Result<T, OffbeatError>;
```

## Module `materials`

Material property correlations for nuclear fuel, cladding and structure.

# What belongs in this module

Correlations that answer "what is property P of material M in state S?" —
thermal conductivity, heat capacity, density, emissivity, Young's modulus,
Poisson's ratio, thermal expansion ([`properties`]), and the behavioural
models for how the material's volume and integrity evolve under irradiation
— swelling, densification, relocation, phase transition, failure
([`behavioral`]).

# What does not belong here

Anything that solves a field equation. Constitutive laws that integrate
stress from strain live in [`crate::rheology`]; the momentum solve lives in
[`crate::mechanics`]. A material model here is a *pure function* of
[`MaterialState`] — it has no memory, no mesh and no timestep.

# The shape every property family takes

Each family is an **enum** whose variants are the published correlations for
it, dispatched by `match` rather than by a trait object. The set of
correlations is closed and known at compile time, so an enum gives
exhaustiveness (adding a correlation forces every call site to consider it)
and lets rust-analyzer's go-to-definition work on the variants, which it
does not on `dyn Trait`. See the workspace `CLAUDE.md` "No trait objects"
rule.

Every variant is named for the **author or data source of the fit** and the
material it applies to — `MatproUo2`, `RelapZircaloy`, `SneadSiC` — because
that is how the fuel-performance literature identifies a correlation, and
because two correlations for "UO2 conductivity" can differ by 20% and a
reader must be able to tell which one is in use.

# Validity ranges are enforced, not assumed

Each correlation is a fit over a stated range of temperature, burnup or
composition. Evaluated outside it, a fit returns a number that looks
reasonable and means nothing. The `*_checked` methods return
[`OutOfRange`](crate::error::OffbeatError::OutOfRange) instead; the plain
methods clamp to the range endpoints, matching upstream behaviour, and say
so in their doc comments.

```rust
pub mod materials { /* ... */ }
```

### Modules

## Module `behavioral`

Behavioural models — how the material's volume and integrity evolve.

Distinct from [`properties`](super::properties): a property answers "what is
the conductivity of this material right now?", whereas a behavioural model
answers "how much has this material swollen, densified, cracked or
relocated?". Both are pure functions of
[`MaterialState`](crate::materials::MaterialState) — the accumulation of
state over time is the caller's job, not theirs.

| Module | Answers |
|---|---|
| [`swelling`] | volume growth from solid and gaseous fission products |
| [`densification`] | early-life shrinkage as fabrication porosity sinters out |
| [`relocation`] | outward movement of cracked fuel fragments, closing the gap |
| [`phase_transition`] | solid-phase changes (e.g. Zircaloy alpha to beta) |
| [`failure`] | failure criteria, including Weibull statistics for ceramics |

```rust
pub mod behavioral { /* ... */ }
```

### Modules

## Module `densification`

Densification models — early-life **shrinkage**, volumetric strain \[-\].

# What densification is

A fresh UO2 pellet is not fully dense: it is pressed and sintered to
typically 95% of theoretical density, and the missing 5% is fine porosity
left over from fabrication. Under irradiation, fission fragments knock atoms
across the small pores and the pores disappear — in-pile sintering. The
pellet therefore **shrinks** over roughly the first 5–10 MWd/kgHM, then
stops: the process saturates once the fine porosity is gone, and from then
on [swelling](super::swelling) takes over and the pellet grows again.

Densification matters because it happens *first*. It opens the fuel/cladding
gap at the beginning of life, which raises the gap's thermal resistance and
so raises fuel temperature — exactly when the rod is at its highest power.

# SIGN CONVENTION — densification is NEGATIVE

**Every model in this module returns a negative number**, or zero.
[`SwellingModel`](super::swelling::SwellingModel) returns a **positive**
number for the same fuel growing. The caller sums the two, so a sign error
here does not crash anything — it silently cancels part of the swelling and
gives a gap that closes at the wrong time. The unit tests at the bottom of
this file assert the sign explicitly, for exactly that reason.

# Units, and the volumetric/linear factor of three

- [`DensificationModel::value`] returns the **volumetric** strain `ΔV/V`
  \[-\], matching [`MaterialState::densification`].
- [`DensificationModel::linear`] returns the one-dimensional strain
  `ΔL/L = value / 3`, which is **exactly upstream's `epsilonDensification`
  field**. Upstream's variable is literally named `dLOverL_max`. If you are
  comparing this port against an OFFBEAT run, compare
  [`linear`](DensificationModel::linear).

Burnup arrives as **MWd/kgHM** ([`MaterialState::burnup`]); temperature as
**K**. Upstream reads burnup off the mesh in MWd/tUO2 and converts locally
(`/1000/0.881` for the FRAPCON model, `/0.881` for the empirical one); this
port receives heavy-metal burnup directly, so those conversions collapse to
a single factor of 1000 where the correlation wants MWd/tHM.

# Validity ranges: `value` clamps, `value_checked` refuses

[`value`](DensificationModel::value) **clamps** burnup and temperature to
the endpoints of the variant's stated validity range before evaluating.
[`value_checked`](DensificationModel::value_checked) returns
[`OffbeatError::OutOfRange`] instead, and additionally rejects a degenerate
parameter set with [`OffbeatError::Unphysical`]. Upstream clamps nothing —
it extrapolates — so this port and upstream deliberately disagree outside
the range.

The ranges are **this port's stated applicability**, not upstream constants:
upstream declares none. Each variant says what its range is.

# Status

AI-assisted translation, reviewed by no human yet. Per `RESPONSIBLE_USE.md`
this is untrusted draft material: the tests below establish internal
consistency with upstream's algebra, **not** validation against measured
densification data.

[`MaterialState::burnup`]: crate::materials::MaterialState::burnup
[`MaterialState::densification`]: crate::materials::MaterialState::densification
[`OffbeatError::OutOfRange`]: crate::error::OffbeatError::OutOfRange
[`OffbeatError::Unphysical`]: crate::error::OffbeatError::Unphysical

```rust
pub mod densification { /* ... */ }
```

### Types

#### Enum `DensificationModel`

Fuel densification correlations — **negative** volumetric strain \[-\] for
shrinking fuel.

One variant per model compiled by upstream OFFBEAT's
`behavioralModels/densification/`; each variant's documentation names the
upstream class and its `TypeName` (the string a user writes in
`solverDict`), so a case file can be translated variant by variant.

Dispatch is by `match`, never by a trait object, per the workspace
`CLAUDE.md` "No trait objects" rule.

# Sign convention

**Negative is shrinkage, and every variant is negative or zero.** See the
[module documentation](self) — this matters because swelling returns a
positive number for the same fuel and the two are summed.

# Densification does not recover

Upstream keeps `min(new, old)` each timestep, so the field never becomes
less negative even if the correlation would allow it. These variants are
pure functions and hold no history; because all three are monotonically
decreasing in burnup at fixed temperature, evaluating them afresh gives the
same answer as upstream's ratchet on a monotonically increasing burnup
history. On a history where the *temperature* falls (which can make the
FRAPCON asymptote less negative) they differ, and the caller wanting
upstream's exact behaviour must apply the `min` itself.

```rust
pub enum DensificationModel {
    Zero,
    Empirical {
        density_fraction: f64,
        density_change: f64,
        saturation_burnup: f64,
    },
    Uo2Frapcon {
        density_fraction: f64,
        resintering_density_change: f64,
        sintering_temperature: f64,
    },
}
```

##### Variants

###### `Zero`

No densification at all — upstream `densificationModel`,
`TypeName("none")`.

Selecting this upstream still creates the `epsilonDensification` field
and leaves it at zero. Returns exactly `0.0` at every state; no validity
range.

###### `Empirical`

Exponential approach to a user-specified final density —
upstream `densificationEmpirical`, `TypeName("empirical")`.

The simplest defensible form: the fuel relaxes exponentially in burnup
towards a final density the user states from their own resintering
measurement.

`ΔV/V(Bu) = ΔV/V_max · (1 − exp(−Bu·1000 / saturation_burnup))`

with `Bu` in MWd/kgHM (so `Bu·1000` is MWd/tHM, the unit
`saturation_burnup` is quoted in), and

`ΔV/V_max = −density_change / (density_fraction·100 + density_change)`

which is minus the fractional density increase, i.e. the volume the fuel
loses once all the sinterable porosity has gone. Upstream computes this
divided by three, because it stores the linear strain.

With the upstream example values (`density_fraction = 0.95`,
`density_change = 0.5` %TD, `saturation_burnup = 1000` MWd/tHM) the
asymptote is `−5.236e-3` volumetric (`−1.745e-3` linear) and it is 63%
complete at 1 MWd/kgHM.

Valid range: burnup `0` to `120` MWd/kgHM. No temperature dependence at
all — this variant is temperature-blind, which is its main limitation
against [`Uo2Frapcon`](Self::Uo2Frapcon).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `density_fraction` | `f64` | As-fabricated fuel density as a fraction of theoretical \[-\];<br>0.95 for typical LWR UO2. Upstream's `densityFraction`.<br><br>This is a fabrication parameter, deliberately **not** taken from<br>[`MaterialState::porosity`](crate::materials::MaterialState::porosity):<br>the correlation is anchored on the as-fabricated density, whereas<br>the state's porosity evolves in service. |
| `density_change` | `f64` | Final density increase \[%TD\] once densification is complete —<br>upstream's `densificationDensityChange`. **Positive** for a<br>densifying fuel (0.5 means the density rises by 0.5 percentage<br>points of theoretical); the sign flip to a negative strain happens<br>inside the correlation. |
| `saturation_burnup` | `f64` | Burnup constant \[MWd/tHM\] of the exponential —<br>upstream's `densificationTimeConstant`. Densification is 63%<br>complete at this burnup and 95% complete at three times it. Must be<br>strictly positive. |

###### `Uo2Frapcon`

UO2 densification, FRAPCON form — upstream `densificationFRAPCON`,
`TypeName("UO2FRAPCON")`.

A two-exponential decay in burnup towards a temperature-dependent
asymptote:

`ΔL/L(Bu) = ΔL/L_max + exp(−3(Bu + B)) + 2·exp(−35(Bu + B))`

with `Bu` in MWd/kgHM. The fast term (decay constant 35) is the rapid
removal of the finest pores in the first fraction of a MWd/kgHM; the
slow term (decay constant 3) is the tail. `B` is not a free parameter —
it is solved for so that `ΔL/L(0) = 0` exactly, i.e. so the correlation
starts from the as-fabricated state. This port solves it with upstream's
own fixed-point iteration,
`B ← −ln(−2·exp(−35B) − ΔL/L_max) / 3`, to a tolerance of 1e-6 in at
most 10 iterations. `ΔV/V` is three times `ΔL/L`.

# The asymptote, and the resintering switch

`ΔL/L_max` depends on temperature, and on whether the user has supplied
a measured resintering density change:

- **`resintering_density_change > 0`** — the measured route:
  `ΔL/L_max = −0.0015 · r · 109.6 / 100` below 950 K and
  `−0.00285 · r · 109.6 / 100` above 1050 K, linearly interpolated in
  between (`r` is the resintering density change in %TD; `109.6` is
  upstream's `10960/100`, the UO2 theoretical density folded into the
  percentage). Hotter fuel densifies about 1.9 times as much, because
  thermal sintering assists the irradiation-driven process.
- **`resintering_density_change == 0`** — the fallback route, from the
  as-fabricated porosity alone:
  `ΔL/L_max = −22.2 · (1 − density_fraction) / (T_sinter − 1453)` below
  950 K and `−66.6 · (1 − density_fraction) / (T_sinter − 1453)` above
  1050 K — a factor of exactly three between the two.

# Two upstream defects, reproduced deliberately

1. **The 950–1050 K interpolation is a no-op on the fallback route.**
   Upstream computes both interpolation endpoints with `par1` (22.2),
   so the "smooth transition" returns the *low*-temperature value across
   the whole window and then jumps by a factor of three at 1050 K. This
   port reproduces that exactly, because changing it would silently
   disagree with any OFFBEAT result being compared against. It is
   checked by a unit test below, which exists to document the defect
   rather than to endorse it.
2. **A dead assignment.** Upstream computes
   `−r/(density_fraction·100 + r)/3` on the measured route and then
   overwrites it in all three temperature branches without using it.
   Not ported — it has no effect.

Valid range: burnup `0` to `120` MWd/kgHM; temperature `300` to
`2000` K.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `density_fraction` | `f64` | As-fabricated fuel density as a fraction of theoretical \[-\];<br>0.95 for typical LWR UO2. Upstream's `densityFraction`. Used only<br>on the fallback route (`resintering_density_change == 0`). |
| `resintering_density_change` | `f64` | Resintering density change \[%TD\] — upstream's<br>`resinteringDensityChange`, the density increase measured on an<br>as-fabricated pellet held at the resintering temperature in a<br>furnace. **Positive**, or exactly `0.0` to select the fallback<br>route. |
| `sintering_temperature` | `f64` | Resintering test temperature \[K\] — upstream's `Tsintering`,<br>default 1800. Used only on the fallback route, where it enters as<br>`T_sinter − 1453`; it must not equal 1453 K. |

##### Implementations

###### Methods

- ```rust
  pub fn value(self: &Self, state: &MaterialState) -> f64 { /* ... */ }
  ```
  Volumetric densification strain `ΔV/V` \[-\], **negative for shrinking

- ```rust
  pub fn linear(self: &Self, state: &MaterialState) -> f64 { /* ... */ }
  ```
  One-dimensional densification strain `ΔL/L` \[-\], **negative for

- ```rust
  pub fn value_checked(self: &Self, state: &MaterialState) -> Result<f64> { /* ... */ }
  ```
  [`value`](Self::value), but returning an error instead of clamping or

- ```rust
  pub fn linear_checked(self: &Self, state: &MaterialState) -> Result<f64> { /* ... */ }
  ```
  [`linear`](Self::linear), but returning an error instead of clamping or

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
    fn clone(self: &Self) -> DensificationModel { /* ... */ }
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
    fn eq(self: &Self, other: &DensificationModel) -> bool { /* ... */ }
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
## Module `failure`

Material failure models (Weibull, strain limits).

**Not yet ported.** This module is a placeholder; see bead op-6sl.

```rust
pub mod failure { /* ... */ }
```

## Module `phase_transition`

Solid-phase transition models.

**Not yet ported.** This module is a placeholder; see bead op-6sl.

```rust
pub mod phase_transition { /* ... */ }
```

## Module `relocation`

Relocation models — outward movement of cracked fuel fragments,
**radial strain** \[-\].

# What relocation is

A UO2 pellet at power has a very steep radial temperature profile: hundreds
of kelvin between its centre and its rim across a few millimetres. The
resulting thermal stress cracks it, radially, within the first hours of
operation. The fragments then slide outward into the fuel/cladding gap.

Nothing has swollen — the fuel's *volume* is essentially unchanged — but the
pellet's effective outer radius has grown and part of the gap has closed.
That matters a great deal, because the gap dominates the fuel's thermal
resistance: a pellet that has relocated runs cooler than one that has not,
at the same power.

# UNITS AND SIGN CONVENTION — read this before using the number

Upstream mixes two conventions in one model — a **percentage gap closure**
internally and a **dimensionless radial strain** in the field it stores — so
this port names all three quantities separately and never returns the
ambiguous one:

| Method | Quantity | Unit | Sign |
|---|---|---|---|
| [`value`](RelocationModel::value) | radial relocation **strain** `ε` — upstream's `epsilonRelocation` | \[-\] | **positive = outward**, closing the gap |
| [`gap_closure_fraction`](RelocationModel::gap_closure_fraction) | fraction `f` of the as-fabricated cold gap closed | \[-\], in `[0, 1]` | positive |
| [`radial_displacement`](RelocationModel::radial_displacement) | outward movement of the pellet surface `Δr` | \[m\] | positive = outward |

They are related by upstream's algebra exactly as

`ε = f · (G_cold / D_cold)` and `Δr = ε · D_cold / 2 = f · G_cold / 2`

where `G_cold` is [`cold_gap`](RelocationModel::Uo2Frapcon::cold_gap) and
`D_cold` is
[`cold_pellet_diameter`](RelocationModel::Uo2Frapcon::cold_pellet_diameter).

**`cold_gap` is read here as the DIAMETRAL gap.** Upstream's header
documents it only as "Cold Gap Reference Thickness \[m\]", which is
ambiguous. Taking it as diametral — consistent with `DiamCold` being a
diameter — makes the algebra self-consistent: `Δr = f · G_cold/2` is then
`f` times the *radial* gap, so `f` is exactly the fraction of the radial gap
that relocation closes, and `f = 1` closes it completely. Under the other
reading, full closure would move the pellet only half way across the gap,
which no relocation model means. If your input deck quotes a radial gap,
double it.

# Relocation is NOT a volumetric strain

[`SwellingModel`](super::swelling::SwellingModel) and
[`DensificationModel`](super::densification::DensificationModel) return
volumetric strains that are summed together. **Relocation is not one of
them.** It is a radial displacement of a cracked, essentially
constant-volume body, and it acts only on the fuel's outer surface and only
in the radial direction. Adding it to a volumetric strain sum is a modelling
error, not a unit error, and nothing will complain.

# What is not ported

Upstream's `relocationFRAPCON::correct` does four things around the
correlation, none of which is a material property, and none of which is here:

- **Slice averaging.** It averages power and burnup over an axial slice of
  the mesh and takes the minimum gap width on the slice. That is mesh
  machinery; a caller passes the slice-averaged values in.
- **The ratchet.** `relocation = max(old, new)` — relocation is not allowed
  to decrease. These variants are pure functions with no history; because
  the correlation is monotonically non-decreasing in burnup at fixed power,
  evaluating it afresh matches the ratchet on a rising-burnup, constant-power
  history. On a **power ramp down** it does not, and the caller must apply
  the `max` itself.
- **Relocation recovery.** When hard pellet/cladding contact would make the
  relocated fuel penetrate the cladding, upstream recovers part of the
  relocation (`recoveryFraction`, `relaxRecovery`) using the gap width and
  the previous timestep's recovered strain. That is a contact-mechanics
  feedback loop over history and gap state, not a correlation; it belongs
  with [`crate::gap`] and is deferred.
- **Sensitivity-analysis scaling.** Upstream's `F_epsilonRelocation` and
  `delta_epsilonRelocation` multiply and offset the result for uncertainty
  studies. A caller wanting that applies it to the returned number.

# Validity ranges: `value` clamps, `value_checked` refuses

[`value`](RelocationModel::value) **clamps** burnup to the endpoints of the
stated validity range before evaluating.
[`value_checked`](RelocationModel::value_checked) returns
[`OffbeatError::OutOfRange`] instead, and additionally rejects an
unphysical geometry with [`OffbeatError::Unphysical`]. Upstream clamps
nothing. The range is **this port's stated applicability**, not an upstream
constant.

# Status

AI-assisted translation, reviewed by no human yet. Per `RESPONSIBLE_USE.md`
this is untrusted draft material: the tests below establish internal
consistency with upstream's algebra, **not** validation against measured
gap-closure data.

[`OffbeatError::OutOfRange`]: crate::error::OffbeatError::OutOfRange
[`OffbeatError::Unphysical`]: crate::error::OffbeatError::Unphysical

```rust
pub mod relocation { /* ... */ }
```

### Types

#### Enum `FrapconRelocationForm`

Which of the two FRAPCON relocation formulations to evaluate — upstream's
`modifiedRelocationModel` switch.

The two are genuinely different fits, not a refinement of one another: at
beginning of life they differ by roughly a factor of six in gap closure. The
switch defaults to [`Modified`](Self::Modified) upstream.

```rust
pub enum FrapconRelocationForm {
    Modified,
    Legacy,
}
```

##### Variants

###### `Modified`

The modified relocation model used from FRAPCON 3.5 onwards —
upstream's `modifiedRelocationModel true`, the default.

Gap closure `f = 0.055 + min(R, R·(0.5795 + 0.2447·ln Bu))` for burnup
above 0.0937 MWd/kgHM, and `f = 0.055` below it, with the power-dependent
amplitude

- `R = 0.345` for `q' < 20` kW/m,
- `R = 0.345 + (q' − 20)/200` for `20 ≤ q' ≤ 40` kW/m,
- `R = 0.445` for `q' > 40` kW/m.

The logarithmic burnup term reaches 1 at `Bu = 5.576` MWd/kgHM, above
which the `min` freezes `f` at `0.055 + R` — relocation is complete
early in life and does not evolve further. The 0.0937 MWd/kgHM cut-off
is not arbitrary: it is the burnup at which the logarithmic term crosses
zero, so the two branches meet (a residual step of about 5e-5 in `f`
remains — measured in a unit test below).

###### `Legacy`

The earlier FRAPCON relocation model (the GT2R2-derived form used before
FRAPCON 3.5) — upstream's `modifiedRelocationModel false`.

Gap closure as a percentage, with `F_Bu = min(Bu/5, 1)` and
`P = (q' − 20)·5/20`:

- `q' < 20` kW/m: `100·f = 30 + 10·F_Bu`
- `20 ≤ q' < 40` kW/m: `100·f = 28 + P + (12 + P)·F_Bu`
- `q' ≥ 40` kW/m: `100·f = 32 + 18·F_Bu`

so `f` runs from 0.28–0.32 fresh to 0.40–0.50 by 5 MWd/kgHM, and is
frozen thereafter. **This form is discontinuous in power at the branch
boundaries** — at exactly 20 kW/m the first branch gives `0.30 + 0.10
F_Bu` and the second `0.28 + 0.12 F_Bu`. Upstream is discontinuous
there too; it is reproduced rather than smoothed, so that a comparison
against an OFFBEAT run is not silently shifted.

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
    fn clone(self: &Self) -> FrapconRelocationForm { /* ... */ }
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
    fn default() -> FrapconRelocationForm { /* ... */ }
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
    fn eq(self: &Self, other: &FrapconRelocationForm) -> bool { /* ... */ }
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
#### Enum `RelocationModel`

Fuel-fragment relocation correlations — **positive** radial strain \[-\] for
fuel moving outward into the gap.

One variant per model compiled by upstream OFFBEAT's
`behavioralModels/relocation/`. Dispatch is by `match`, never by a trait
object, per the workspace `CLAUDE.md` "No trait objects" rule.

Read the [module documentation](self) for the unit and sign convention
before using [`value`](Self::value) — upstream mixes a percentage gap
closure with a radial strain, and this port separates them into three named
methods.

```rust
pub enum RelocationModel {
    Zero,
    Uo2Frapcon {
        cold_gap: f64,
        cold_pellet_diameter: f64,
        linear_power: f64,
        form: FrapconRelocationForm,
    },
}
```

##### Variants

###### `Zero`

No relocation — upstream `relocationModel`, `TypeName("none")`.

Selecting this upstream still creates the `epsilonRelocation` and
`epsilonRecoveredRelocation` fields and leaves them at zero. Returns
exactly `0.0` at every state; no validity range.

Choosing this is a real modelling decision, not a null one: without
relocation the gap stays open, the fuel runs several hundred kelvin
hotter at beginning of life, and a fuel-temperature comparison against
measured data will be visibly wrong.

###### `Uo2Frapcon`

UO2 relocation, FRAPCON form — upstream `relocationFRAPCON`,
`TypeName("UO2FRAPCON")`.

Empirical gap closure as a function of linear power and burnup, in
either of the two formulations of [`FrapconRelocationForm`], converted
to a radial strain by `ε = f · (G_cold / D_cold)`.

# Why the geometry and the power live on the variant

[`MaterialState`] carries the local
thermodynamic and irradiation state, not rod geometry or rod power.
Relocation needs all three, so the cold gap, the cold pellet diameter
and the linear power sit on this variant. The first two are fixed for a
given rod design; **`linear_power` is not** — it changes through life,
and this variant must be reconstructed when it does.

Valid range: burnup `0` to `120` MWd/kgHM. Linear power is not clamped —
both formulations saturate outside 20–40 kW/m by construction — but a
negative power is rejected by
[`value_checked`](Self::value_checked).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `cold_gap` | `f64` | As-fabricated **diametral** fuel/cladding gap \[m\] at cold<br>conditions — upstream's `GapCold`. See the [module<br>documentation](self) on why it is read as diametral; if your input<br>deck quotes a radial gap, double it. Typical LWR value: 1.7e-4 m. |
| `cold_pellet_diameter` | `f64` | As-fabricated pellet diameter \[m\] at cold conditions — upstream's<br>`DiamCold`. Typical LWR value: 8.2e-3 m. |
| `linear_power` | `f64` | Rod linear power \[W/m\] — the slice-averaged `q'` the correlation<br>branches on, converted to kW/m internally.<br><br>Upstream derives it from the volumetric heat source `Q` \[W/m³\] as<br>`q' = Q · π · (D_cold/2)²`, so a caller holding a volumetric source<br>must apply that conversion. Typical LWR value: 2.0e4 W/m<br>(20 kW/m). |
| `form` | `FrapconRelocationForm` | Which of the two FRAPCON formulations to evaluate. |

##### Implementations

###### Methods

- ```rust
  pub fn value(self: &Self, state: &MaterialState) -> f64 { /* ... */ }
  ```
  Radial relocation **strain** `ε` \[-\] — upstream's `epsilonRelocation`.

- ```rust
  pub fn gap_closure_fraction(self: &Self, state: &MaterialState) -> f64 { /* ... */ }
  ```
  Fraction `f` \[-\] of the as-fabricated cold gap that relocation has

- ```rust
  pub fn radial_displacement(self: &Self, state: &MaterialState) -> f64 { /* ... */ }
  ```
  Outward movement `Δr` \[m\] of the pellet outer surface caused by

- ```rust
  pub fn value_checked(self: &Self, state: &MaterialState) -> Result<f64> { /* ... */ }
  ```
  [`value`](Self::value), but returning an error instead of clamping or

- ```rust
  pub fn gap_closure_fraction_checked(self: &Self, state: &MaterialState) -> Result<f64> { /* ... */ }
  ```
  [`gap_closure_fraction`](Self::gap_closure_fraction), but returning an

- ```rust
  pub fn radial_displacement_checked(self: &Self, state: &MaterialState) -> Result<f64> { /* ... */ }
  ```
  [`radial_displacement`](Self::radial_displacement), but returning an

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
    fn clone(self: &Self) -> RelocationModel { /* ... */ }
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
    fn eq(self: &Self, other: &RelocationModel) -> bool { /* ... */ }
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
## Module `swelling`

Swelling models — irradiation-driven volume **growth**, volumetric strain \[-\].

# What swelling is

Fission destroys one heavy-metal atom and creates two fission-product atoms
that, together, occupy more space than the atom they replaced. The solid
fission products dissolve in the fuel lattice and make it grow roughly
**linearly with burnup**; the gaseous ones (xenon, krypton) precipitate into
bubbles whose growth is **strongly temperature-dependent** and saturates.
In metals — cladding and structural steels — the same word covers two
different phenomena: **void swelling**, the growth of vacancy voids under
fast-neutron damage (isotropic, threshold-like in dose), and **irradiation
growth**, a volume-*conserving* change of shape in anisotropic hexagonal
metals such as Zircaloy (elongation along the rod axis, contraction across
it).

# SIGN CONVENTION — swelling is POSITIVE

**Every model in this module returns a positive number for material that is
growing.** [`DensificationModel`] returns a **negative** number for the same
material shrinking. The two are summed by the caller, so a sign error here
does not blow up — it silently cancels part of the densification and
produces a fuel/cladding gap that closes at the wrong time. Be certain which
one you are holding.

The one documented exception is
[`WrightShamHastelloyN`](SwellingModel::WrightShamHastelloyN), whose
upstream correlation is *negative below its incubation dose* (about 0.99
dpa). That is upstream's behaviour, reproduced faithfully; it is a defect of
the fit, not a sign-convention change. See that variant's documentation.

# Units, and the volumetric/linear factor of three

- [`SwellingModel::value`] returns the **volumetric** strain `ΔV/V` \[-\],
  matching [`MaterialState::swelling`].
- [`SwellingModel::strain`] returns the three **linear** components \[-\]
  separately, matching upstream's `epsilonSwelling` symmetric-tensor
  diagonal. For an isotropic model each component is `value() / 3`.

**Upstream stores the linear components, not the volumetric strain.** Every
upstream `correct()` in this directory ends with `swellingI[cellI] =
nominalValue * I`, i.e. it writes one third of the volume change into each
diagonal component. If you are comparing this port against an OFFBEAT run,
compare [`strain`](SwellingModel::strain), not [`value`](SwellingModel::value).

Burnup arrives as **MWd/kgHM** ([`MaterialState::burnup`]); fast fluence as
**n/m²** ([`MaterialState::fast_fluence`]). Several upstream correlations
are written against fluence in **n/cm²** and burnup in MWd/tU or in %FIMA;
every conversion is done once, here, and is stated in the variant's
documentation.

# Validity ranges: `value` clamps, `value_checked` refuses

[`value`](SwellingModel::value) and [`strain`](SwellingModel::strain)
**clamp** burnup, fluence and temperature to the endpoints of the variant's
stated validity range before evaluating, so they always return a finite,
bounded number. [`value_checked`](SwellingModel::value_checked) and
[`strain_checked`](SwellingModel::strain_checked) instead return
[`OffbeatError::OutOfRange`]. Upstream OFFBEAT clamps *nothing* in this
directory — it extrapolates freely — so outside the stated range this port
and upstream deliberately disagree, and the clamped answer is the more
defensible of the two.

The ranges themselves are **this port's stated applicability**, not upstream
constants: upstream declares no ranges at all. They are set to the operating
window the correlation's material actually sees, wide enough that a normal
case never touches an endpoint. Each variant says what its range is.

# What is deliberately not ported

- **`swellingPARFUMEBuffer` / `swellingPARFUMEPyC`** (TRISO buffer and
  pyrolytic-carbon layers, PARFUME correlations). These are not pure
  functions of [`MaterialState`]: they need the fast **flux** and the
  **timestep** (they integrate a strain *rate* explicitly, `ε += ε̇ φ̇ Δt`),
  plus a two-dimensional interpolation table in temperature and
  Bacon-Anisotropy-Factor and a second table in coating density. Porting
  them needs a state object this crate does not yet have and a table
  interpolator this crate does not yet have. Use
  [`PyroCarbonCorrelation`](SwellingModel::PyroCarbonCorrelation) — a
  closed-form polynomial in fluence — for pyrolytic carbon in the meantime.
- **`swellingGrowthMatproZy`** (MATPRO Zircaloy irradiation growth). It is
  **absent from upstream's `Make/files`** and, as written, does not compile:
  it assigns to a `const scalar`, and binds a `symmTensorField` to a
  `const scalarField&`. It is dead code upstream, so there is no compiled
  behaviour to port and nothing to verify a port against.

# Status

AI-assisted translation, reviewed by no human yet. Per `RESPONSIBLE_USE.md`
this is untrusted draft material: the unit tests below establish internal
consistency and agreement with upstream's own algorithms, **not** validation
against measured swelling data.

[`DensificationModel`]: super::densification::DensificationModel
[`MaterialState`]: crate::materials::MaterialState
[`MaterialState::burnup`]: crate::materials::MaterialState::burnup
[`MaterialState::fast_fluence`]: crate::materials::MaterialState::fast_fluence
[`MaterialState::swelling`]: crate::materials::MaterialState::swelling
[`OffbeatError::OutOfRange`]: crate::error::OffbeatError::OutOfRange

```rust
pub mod swelling { /* ... */ }
```

### Types

#### Struct `SwellingStrain`

The three linear swelling strain components \[-\] in the material's local
radial / hoop / axial frame.

# Why three numbers and not one

Isotropic swelling (fission-product swelling in an oxide pellet, void
swelling in a steel) puts one third of the volume change into each
direction, and a single scalar would do. **Irradiation growth in Zircaloy
does not**: it elongates the cladding along the rod axis and contracts it
across, at essentially constant volume, so its volumetric strain is nearly
zero while its axial strain is the whole engineering point. A scalar-only
interface would report "no swelling" for the model whose entire purpose is
axial elongation. Upstream stores a symmetric tensor for exactly this
reason; this struct is that tensor's diagonal.

# Frame

- `radial` — upstream's `xx`. For a spherical TRISO coating layer this is
  the through-thickness direction.
- `hoop` — upstream's `yy`. For a TRISO layer, one of the two tangential
  directions.
- `axial` — upstream's `zz`. Along the fuel rod. For a TRISO layer, the
  second tangential direction (equal to `hoop` by symmetry).

# Sign and units

Dimensionless engineering strain, **positive for growth** in that
direction. Zircaloy growth legitimately gives a negative `radial`/`hoop`
alongside a positive `axial`.

```rust
pub struct SwellingStrain {
    pub radial: f64,
    pub hoop: f64,
    pub axial: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `radial` | `f64` | Linear strain \[-\] along the local radial direction (upstream `xx`). |
| `hoop` | `f64` | Linear strain \[-\] along the local hoop direction (upstream `yy`). |
| `axial` | `f64` | Linear strain \[-\] along the axial direction (upstream `zz`). |

##### Implementations

###### Methods

- ```rust
  pub const fn new(radial: f64, hoop: f64, axial: f64) -> Self { /* ... */ }
  ```
  Construct from the three linear components \[-\], each positive for

- ```rust
  pub fn isotropic(volumetric: f64) -> Self { /* ... */ }
  ```
  Construct an isotropic strain from a **volumetric** strain \[-\], i.e.

- ```rust
  pub fn volumetric(self: &Self) -> f64 { /* ... */ }
  ```
  Volumetric swelling strain `ΔV/V` \[-\], **positive for growth**.

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
    fn clone(self: &Self) -> SwellingStrain { /* ... */ }
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
    fn eq(self: &Self, other: &SwellingStrain) -> bool { /* ... */ }
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
#### Enum `BisonZircaloyCladType`

Zircaloy alloy and metallurgical condition selecting the BISON irradiation
growth coefficients.

Upstream's `swellingGrowthBISONZy` reads this as the `cladType` keyword and
overwrites its `A` and `n` coefficients accordingly. Growth is
`ε_axial = A φ^n` with `φ` the fast fluence in **n/cm²** (E > 1 MeV); the
coefficient pairs below are the ones hard-coded upstream.

```rust
pub enum BisonZircaloyCladType {
    Sra,
    Rxa,
    Pra,
    Zirlo,
    Escore,
    M5,
}
```

##### Variants

###### `Sra`

Stress-relief annealed Zircaloy-2 or Zircaloy-4. `A = 2.18e-21`,
`n = 0.845`.

###### `Rxa`

Recrystallisation annealed Zircaloy-2 or M5. `A = 1.09e-21`,
`n = 0.845`.

###### `Pra`

Partially recrystallised Zircaloy-2. `A = 1.09e-21`, `n = 0.845` —
upstream gives it the same coefficients as [`Rxa`](Self::Rxa).

###### `Zirlo`

Stress-relief annealed ZIRLO. `A = 9.7893e-25`, `n = 0.98239`.

###### `Escore`

The ESCORE growth model (Rashid). `A = 3.0e-20`, `n = 0.794`. This is
upstream's default when no `cladType` is given.

###### `M5`

M5 (Gilbon). `A = 7.013e-21`, `n = 0.81787`.

##### Implementations

###### Methods

- ```rust
  pub fn coefficients(self: &Self) -> (f64, f64) { /* ... */ }
  ```
  The `(A, n)` pair of the growth law `ε_axial = A φ^n`, with `φ` the fast

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
    fn clone(self: &Self) -> BisonZircaloyCladType { /* ... */ }
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
    fn default() -> BisonZircaloyCladType { /* ... */ }
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
    fn eq(self: &Self, other: &BisonZircaloyCladType) -> bool { /* ... */ }
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
#### Enum `SwellingModel`

Irradiation swelling and growth correlations — **positive** volumetric
strain \[-\] for growing material.

One variant per model compiled by upstream OFFBEAT's
`behavioralModels/swelling/`; each variant's documentation names the
upstream class and its `TypeName` (the string a user writes in
`solverDict`), so a case file can be translated variant by variant. Two
upstream models are deliberately absent — see the [module
documentation](self).

Dispatch is by `match` on the enum, never by a trait object, per the
workspace `CLAUDE.md` "No trait objects" rule: the set of published
correlations is closed and known at compile time, so adding one must be a
compile error at every call site rather than a runtime surprise.

# Sign convention

**Positive is growth.** See the [module documentation](self) — this matters
because densification returns a negative number for the same fuel and the
two are summed.

```rust
pub enum SwellingModel {
    Zero,
    Constant {
        swelling_rate: f64,
    },
    Uo2Frapcon {
        theoretical_density: f64,
    },
    Uo2Matpro {
        theoretical_density: f64,
    },
    FbrMox {
        gap_open: bool,
    },
    FeCrAl {
        rate: f64,
    },
    GrowthBisonZircaloy {
        clad_type: BisonZircaloyCladType,
    },
    GrowthAim11515Ti,
    GrowthGeneralized1515Ti,
    WrightShamHastelloyN,
    PyroCarbonCorrelation {
        radial_coefficients: [f64; 6],
        tangential_coefficients: [f64; 6],
        flux_conversion_factor: f64,
    },
}
```

##### Variants

###### `Zero`

No swelling at all — upstream `swellingModel`, `TypeName("none")`.

Selecting this upstream still creates the `epsilonSwelling` field and
leaves it at zero, so the mechanics solve runs with a swelling term that
is identically zero. Use it to isolate other effects, never as a
physical model of irradiated fuel.

Returns exactly `0.0` at every state; no validity range.

###### `Constant`

Swelling proportional to fast fluence with a user-supplied rate —
upstream `constantSwelling`, `TypeName("constant")`.

`ε_linear = swelling_rate · φ / 1e25`, with `φ` the fast fluence in
n/m². Isotropic, so the volumetric strain is three times that.

**Not a correlation** — a placeholder for a material whose swelling has
been measured but not fitted, and a convenient way to impose a known
swelling in a verification case. Its "validity range" below is a sanity
guard only.

Valid range: fast fluence `0` to `1e27` n/m².

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `swelling_rate` | `f64` | Upstream's `swellingRate` \[-\]: the **linear** strain in each<br>direction per `1e25` n/m² of fast fluence. The volumetric strain per<br>`1e25` n/m² is three times this. |

###### `Uo2Frapcon`

UO2 fission-product swelling, FRAPCON form — upstream `swellingFRAPCON`,
`TypeName("UO2FRAPCON")`.

Piecewise-linear in burnup, with the swelling rate stepping up at high
burnup as fission gas starts to contribute:

- `Bu < 6` MWd/kgHM: zero. The as-fabricated porosity accommodates the
  early fission products, so no net growth is seen.
- `6 ≤ Bu < 80` MWd/kgHM: `ΔV/V = (Bu − 6)·1000 · b1 · ρ`
- `Bu ≥ 80` MWd/kgHM:
  `ΔV/V = (80 − 6)·1000·b1·ρ + (Bu − 80)·1000·b2·ρ`

with `b1 = 2.974e10 · 2.315e-23 · 86.4` and
`b2 = 2.974e10 · 3.211e-23 · 86.4` (upstream's `par3·par4·par5` and
`par3·par6·par5`, fixed here at their upstream defaults), and `ρ` the
porous fuel density in kg/m³, taken as
`theoretical_density · state.density_fraction()`. `2.315e-23` and
`3.211e-23` are FRAPCON's `ΔV/V` per fission per m³ below and above
80 MWd/kgHM.

At `ρ = 10 400` kg/m³ this is **6.19e-4 volumetric strain per
MWd/kgHM** in the first regime, i.e. 0.62% per 10 MWd/kgHM.

**Gaseous swelling is not included.** Upstream adds the
`intragranularGasSwelling` and `intergranularGasSwelling` fields on top
of this when the fission-gas model has created them. Those come from
[`crate::fgr`], not from here; the caller adds them.

Valid range: burnup `0` to `120` MWd/kgHM.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `theoretical_density` | `f64` | Theoretical (pore-free) density of the fuel \[kg/m³\]; 10 960 for<br>UO2. The porous density used by the correlation is this times<br>`MaterialState::density_fraction`. |

###### `Uo2Matpro`

UO2 fission-product swelling, MATPRO form — upstream `swellingMATPRO`,
`TypeName("UO2MATPRO")`.

Solid plus gaseous fission-product swelling, both quoted per unit
**FIMA** (fissions per initial metal atom). Burnup is converted with
`FIMA = Bu / 937.06` for `Bu` in MWd/kgHM.

- solid: `ΔV/V = 5.577e-5 · ρ · FIMA`
- gaseous rate: `d(ΔV/V)/dFIMA = 1.96e-31 · ρ · (2800 − T)^11.73 ·
  exp(−0.0162 (2800 − T)) · exp(−0.0178 ρ · FIMA)`

with `ρ` in kg/m³ and `T` in K. The gaseous term peaks in the
intermediate-temperature bubble-growth window and dies away at high
burnup, where the `exp(−0.0178 ρ FIMA)` factor saturates it.

# This port integrates; upstream accumulates

Upstream evaluates the *rate* and adds `rate · ΔBu` each timestep — a
forward-Euler accumulation whose answer depends on the timestep. A pure
function of `MaterialState` cannot do that, so this port integrates the
rate analytically from zero burnup:

`ΔV/V_gas = C · (exp(k·FIMA) − 1) / k`, with
`C = 1.96e-31 ρ (2800−T)^11.73 exp(−0.0162(2800−T))` and
`k = −0.0178 ρ`, holding `T` at its current value over the whole
history. The two agree in the limit of small burnup steps at constant
temperature — that convergence is a unit test in this module. They
differ, legitimately, on a history where the temperature changed.

The solid term is exactly linear, so it is unaffected.

Valid range: burnup `0` to `120` MWd/kgHM; temperature `300` to
`2800` K. The upper temperature bound is not decorative:
`(2800 − T)^11.73` is `NaN` for `T > 2800` K, and upstream would
propagate that `NaN` into the mechanics solve.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `theoretical_density` | `f64` | Theoretical (pore-free) density of the fuel \[kg/m³\]; 10 960 for<br>UO2, about 11 000 for LWR MOX. Multiplied by<br>`MaterialState::density_fraction` to get the porous density the<br>correlation wants. |

###### `FbrMox`

Fast-reactor MOX swelling — upstream `swellingFBRMOX`,
`TypeName("FBRMOX")`.

A three-rate empirical law in burnup expressed as **%FIMA**, converted
here as `%FIMA = Bu / 9.5` for `Bu` in MWd/kgHM. Which rate applies
depends on whether the fuel/cladding gap is still open:

- gap open, `%FIMA ≤ 1`: `d(ΔV/V)/d(%FIMA) = 0.020` — free swelling,
  fission gas retained in bubbles.
- gap open, `%FIMA > 1`: `0.012`.
- gap closed: `0.0065` — only the solid fission products contribute;
  with the pellet in contact with the cladding, gas bubbles are
  suppressed and the gas is released instead.

This port integrates those rates from zero burnup, so with the gap open
throughout, `ΔV/V = 0.020·min(%FIMA, 1) + 0.012·max(%FIMA − 1, 0)`, and
with the gap closed throughout, `ΔV/V = 0.0065·%FIMA`.

# `gap_open` describes the whole history, not this instant

Upstream re-reads the slice gap width every timestep and can switch
rates mid-life; a pure function cannot. The value returned here is the
swelling that would have accumulated had `gap_open` held for the entire
irradiation. For a rod whose gap closes part-way through, that is an
upper bound (`gap_open = true`) or a lower bound (`gap_open = false`),
and a caller needing the mixed history must integrate the rates itself.

Valid range: burnup `0` to `250` MWd/kgHM (about 26 %FIMA).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `gap_open` | `bool` | Whether the fuel/cladding gap is open (`true`) for the whole<br>irradiation history. See the variant note above — this is a history<br>flag, not an instantaneous one. |

###### `FeCrAl`

FeCrAl cladding swelling, linear in fast fluence — upstream
`swellingFrCrAl` (spelt "Fr" upstream), `TypeName("FrCrAl")`.

`ε_linear = rate · φ`, isotropic, with `φ` the fast fluence in **n/m²**
— this is one of the few upstream models that works in SI fluence
directly. The upstream default `rate = 4.5e-29` per n/m² gives 0.45%
linear (1.35% volumetric) at `1e26` n/m².

FeCrAl is an accident-tolerant-fuel cladding candidate; the linear form
is a first-order fit with no incubation dose and no temperature
dependence, so it will over-predict at low dose.

Valid range: fast fluence `0` to `2e26` n/m².

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `rate` | `f64` | Upstream's `par1` \[1/(n/m²)\]: the **linear** strain per unit fast<br>fluence in n/m². Upstream default `4.5e-29`. |

###### `GrowthBisonZircaloy`

Zircaloy irradiation **growth** (not swelling) — upstream
`swellingGrowthBISONZy`, `TypeName("growthBISONZy")`.

Anisotropic and volume-conserving. Zircaloy's hexagonal grains grow
along the rod axis and contract across it under fast-neutron damage, at
essentially constant volume:

- `ε_axial = A φ^n`, with `φ` the fast fluence in **n/cm²** — the SI
  fluence in `MaterialState` is divided by 1e4 here.
- `ε_radial = ε_hoop = −(1 − (1 + ε_axial)^(−1/2))`, the transverse
  contraction that keeps the volume fixed.

`(A, n)` come from [`BisonZircaloyCladType`].

**[`value`](Self::value) is near zero for this variant, by design.** Its
volumetric strain is `O(ε_axial²)` — `+5.78e-5` against an axial strain
of `+8.81e-3` at 1e26 n/m², i.e. 0.66% of it — because that is what
volume conservation means. Use
[`strain`](Self::strain) and read `axial`. A caller that only ever calls
`value()` will conclude the cladding is not moving, and will be wrong.

# Closed form versus upstream's accumulation

Upstream applies the transverse mapping to each timestep's *increment*
and sums; this port applies it once to the total. The mapping is
nonlinear, so the two differ at **second order in the strain**: a unit
test in this module measures `−4.377561e-3` (closed form) against
`−4.406474e-3` (accumulated in 100 000 steps) at 1e26 n/m², a relative
difference of 6.6e-3 and an absolute one of 2.9e-5 in strain. The
accumulated form is the one that omits the `3ε²/8` term, so the closed
form is the more nearly exact of the two; either way the gap is far
below any measurement uncertainty on cladding growth.

Valid range: fast fluence `0` to `1.5e26` n/m² (1.5e22 n/cm²), which
covers LWR cladding to end of life.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `clad_type` | `BisonZircaloyCladType` | Alloy and metallurgical condition, selecting `(A, n)`. |

###### `GrowthAim11515Ti`

AIM1 15-15Ti austenitic steel **void swelling** — upstream
`swellingGrowthAIM11515Ti`, `TypeName("growthAIM11515Ti")`.

Despite upstream's "growth" name this is isotropic void swelling, not
anisotropic growth: upstream adds the same increment to `xx`, `yy` and
`zz`.

`ΔV/V [%] = 1.3e-5 · exp(−2.5·((T_C − 490)/100)²) · φ22^3.9`

with `T_C` the temperature in **°C** and `φ22` the fast fluence in units
of `1e22` n/cm² (the SI fluence divided by 1e26). The result is a
percentage; this port divides by 100 to return a strain. The Gaussian in
temperature peaks at 490 °C — the classic austenitic-steel void-swelling
peak, where vacancy mobility and void stability overlap — and the
`φ^3.9` dependence is the steep post-incubation regime.

AIM1 (Austenitic Improved Material #1) is the titanium-stabilised
15Cr-15Ni cladding developed for sodium-cooled fast reactors.

Valid range: fast fluence `0` to `3e27` n/m²; temperature `573.15` to
`1023.15` K (300–750 °C).

###### `GrowthGeneralized1515Ti`

Generalised 15-15Ti austenitic steel **void swelling** — upstream
`swellingGrowthGeneralized1515Ti`,
`TypeName("growthGeneralized1515Ti")`.

Same functional form as [`GrowthAim11515Ti`](Self::GrowthAim11515Ti),
different fit — a generic 15-15Ti rather than the AIM1 heat:

`ΔV/V [%] = 1.5e-3 · exp(−2.5·((T_C − 450)/100)²) · φ22^2.75`

The swelling peak sits 40 °C lower and the dose exponent is milder
(2.75 against 3.9), so this fit predicts more swelling at low dose and
less at high dose than the AIM1 one. At `φ22 = 10` and the peak
temperature it gives 0.84% against AIM1's 0.10%. They are genuinely
different materials — do not treat the pair as an uncertainty band.

Valid range: fast fluence `0` to `3e27` n/m²; temperature `573.15` to
`1023.15` K (300–750 °C).

###### `WrightShamHastelloyN`

Hastelloy N **void swelling**, Wright-Sham correlation — upstream
`swellingWrightShamHastelloyN`, `TypeName("WrightShamHastelloyN")`.

Isotropic. Hastelloy N is the nickel-molybdenum alloy developed for
molten-salt reactors, where the relevant damage measure is displacement
damage rather than raw fluence:

- `dpa = φ / 1e26 · 5` — upstream's conversion, 5 dpa per `1e22` n/cm².
- `f(dpa) = 0.9845 · dpa^0.4385 − 0.981`
- `g(T) = exp(−((T_C − 490)/100)²)`
- `ΔV/V [%] = g(T) · f(dpa)`, divided by 100 here to give a strain.

# This variant can return a NEGATIVE value — and that is upstream

`f(dpa)` is negative below `dpa ≈ 0.992` (a fast fluence of about
`1.98e25` n/m²), so the correlation reports
*shrinkage* below its incubation dose. That is an artefact of fitting a
power law with an offset to data that has an incubation period; it is
not a sign-convention change and not densification. Upstream does not
clamp it and neither does this port, because clamping would silently
change the numbers an OFFBEAT comparison is judged against. **If you are
summing this with a densification model, be aware you may be adding two
negative numbers below 1 dpa.**

Valid range: fast fluence `0` to `4e26` n/m² (about 20 dpa);
temperature `573.15` to `1073.15` K (300–800 °C).

###### `PyroCarbonCorrelation`

Pyrolytic-carbon TRISO coating dimensional change, polynomial
correlation — upstream `swellingCorrelationPyC`,
`TypeName("PyCCorrelation")`.

Anisotropic and user-parameterised. Pyrolytic carbon under fast-neutron
damage first *densifies* (negative strain in both directions) and then
turns around and swells, with the radial and tangential responses
differing because the deposited layer is texturally anisotropic. The
user supplies the coefficients of the strain **rate** with respect to
fluence; this port integrates them, as upstream does:

`ε_r(φ) = Σ_{i=0}^{5} A_r[i] · φ^(i+1) / (i+1)`, and likewise for `ε_t`
with `A_t`,

with `φ` the fast fluence in units of `1e25` n/m², scaled by
`flux_conversion_factor`. The radial component goes to
[`SwellingStrain::radial`]; the tangential component goes to **both**
[`hoop`](SwellingStrain::hoop) and [`axial`](SwellingStrain::axial),
matching upstream's `yy = zz = ε_t` for a spherical layer.

**Not ported:** upstream can rotate this spherical-frame tensor into
Cartesian mesh coordinates when `sphereCoordinate` is false. That is a
mesh operation, not a material correlation, so it belongs to the caller;
this variant always returns the spherical-frame components.

Because the coefficients are user input, the sign of the *result* is
whatever the supplied fit says — a negative value here means the coating
is densifying, which for pyrolytic carbon is real physics and not an
error.

Valid range: fast fluence `0` to `4e25` n/m², the range over which the
PARFUME-family pyrolytic-carbon fits these coefficients come from are
defined (upstream's PARFUME models hard-bound it at `3.96e25`).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `radial_coefficients` | `[f64; 6]` | `A_r` — the six polynomial coefficients \[1/(1e25 n/m²)^(i+1)\] of<br>the **radial** strain rate with respect to fluence, upstream's<br>`radialCoefficients`. |
| `tangential_coefficients` | `[f64; 6]` | `A_t` — the six polynomial coefficients of the **tangential** strain<br>rate, upstream's `tangentialCoefficients`. |
| `flux_conversion_factor` | `f64` | Upstream's `fluxConversionFactor` \[-\], default `1.0`. Rescales the<br>fluence when the fit's fast-neutron energy cut-off differs from the<br>one the fluence field was accumulated with (e.g. an "equivalent DIDO<br>nickel dose" against E > 1 MeV). |

##### Implementations

###### Methods

- ```rust
  pub fn strain(self: &Self, state: &MaterialState) -> SwellingStrain { /* ... */ }
  ```
  The three linear swelling strain components \[-\], in the local

- ```rust
  pub fn value(self: &Self, state: &MaterialState) -> f64 { /* ... */ }
  ```
  Volumetric swelling strain `ΔV/V` \[-\], **positive for growth**.

- ```rust
  pub fn strain_checked(self: &Self, state: &MaterialState) -> Result<SwellingStrain> { /* ... */ }
  ```
  [`strain`](Self::strain), but returning [`OffbeatError::OutOfRange`]

- ```rust
  pub fn value_checked(self: &Self, state: &MaterialState) -> Result<f64> { /* ... */ }
  ```
  [`value`](Self::value), but returning [`OffbeatError::OutOfRange`]

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
    fn clone(self: &Self) -> SwellingModel { /* ... */ }
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
    fn eq(self: &Self, other: &SwellingModel) -> bool { /* ... */ }
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
## Module `properties`

Thermo-mechanical property correlations.

Seven families, one module each, mirroring upstream's
`thermoMechanicalPropertiesModels/` directory layout:

| Module | Property | SI unit |
|---|---|---|
| [`conductivity`] | thermal conductivity | W/(m K) |
| [`heat_capacity`] | specific heat capacity | J/(kg K) |
| [`density`] | density | kg/m^3 |
| [`emissivity`] | surface emissivity | - |
| [`young_modulus`] | Young's modulus | Pa |
| [`poisson_ratio`] | Poisson's ratio | - |
| [`thermal_expansion`] | thermal expansion strain / coefficient | - and 1/K |

Every family is an enum over the published correlations for it, evaluated
against a [`MaterialState`](crate::materials::MaterialState). See the
[module-level documentation](crate::materials) for why enums rather than
trait objects, and for the validity-range convention.

```rust
pub mod properties { /* ... */ }
```

### Modules

## Module `conductivity`

Thermal conductivity correlations \[W/(m K)\].

# What this module computes

The **thermal conductivity** `k` of fuel, cladding and structural materials,
in W/(m K), as a function of the local [`MaterialState`] — temperature,
burnup, porosity, stoichiometry, plutonium and gadolinia content.

Conductivity is the single most influential material property in a
fuel-performance calculation: it sets the fuel centreline temperature, which
then drives thermal expansion, creep, fission-gas release and, at the limit,
melting. Two published UO2 correlations can differ by 20% at the same
temperature, so each variant of [`ConductivityModel`] is named for the
**author or data source of the fit**, never for the material alone.

# Units — raw `f64`, strict SI

Inputs come from [`MaterialState`] (temperature in K, burnup in MWd/kgHM —
see that type for the full list); the returned value is always in
**W/(m K)**. Several of the underlying published fits are written in degrees
Celsius or in MWd/tHM; every such conversion is done inside this module, so
a caller never has to know which convention a given fit used.

# Validity ranges: `value` clamps, `value_checked` reports

Every variant declares a temperature validity window via
[`ConductivityModel::temperature_range`].

- [`ConductivityModel::value`] **clamps the temperature into that window**
  before evaluating, so it always returns a finite number in the range the
  fit was built for.
- [`ConductivityModel::value_checked`] instead returns
  [`OffbeatError::OutOfRange`] when the temperature falls outside the window,
  and [`OffbeatError::Unphysical`] for a non-positive absolute temperature.

**Honest note on "matching upstream".** Upstream OFFBEAT does not clamp:
`conductivityRelapZy` prints a warning and extrapolates, and most of the
other models perform no range check at all. Clamping in [`value`] is
therefore this port's deliberate, documented choice — it removes the silent
extrapolation without removing the information, because
[`value_checked`] still reports it. Where a correlation *does* clamp
internally (porosity cut-offs in the MOX models, the 95%-porosity floor in
[`MaterialState::density_fraction`]) that clamp is part of the correlation
and is applied by both methods.

[`value`]: ConductivityModel::value
[`value_checked`]: ConductivityModel::value_checked
[`OffbeatError::OutOfRange`]: crate::error::OffbeatError::OutOfRange
[`OffbeatError::Unphysical`]: crate::error::OffbeatError::Unphysical

# Where the validity windows come from

Only four of the fifteen upstream models state a validity range in their own
source (`conductivityRelapZy`: 290–1800 K, and — indirectly, through the
matching heat-capacity fits — the Zircaloy, 15-15 Ti and SiC windows).
For every other variant the window in [`temperature_range`] is **this
port's engineering choice**, and its doc comment says so explicitly. A
port-chosen window is a guard rail, not a statement about the underlying
experiment.

[`temperature_range`]: ConductivityModel::temperature_range

# Known deviations from upstream, and known upstream defects

- **Burnup units.** Upstream is internally inconsistent about the unit of
  its `Bu` field: `conductivityMatproUO2` converts with `Bu/1000/0.881`
  (MWd/tUO2 → MWd/kgU), while `conductivityUO2IFA601`,
  `conductivityUPuO2LanningBeyer` and `conductivityUPuO2Philipponneau`
  convert with `Bu*1e-3` (MWd/tHM → MWd/kgHM) — i.e. the same field is read
  with and without the 0.881 heavy-metal fraction. This port takes
  [`MaterialState::burnup`] in **MWd/kgHM** and uses it directly in every
  correlation, so the inconsistency cannot survive here.
- **Dictionary-tunable coefficients are not exposed.** Upstream lets a case
  dictionary override every fit coefficient (`par1` … `par13`, `perturb`,
  `kappaInf`, …), which exists for uncertainty quantification. This port
  hard-codes the published values; a UQ surface can be added later without
  changing the physics.
- **`conductivityNfirUO2` reads its gadolinia content from a dictionary key
  named `GdContent_`** (with a trailing underscore) while
  `conductivityMatproUO2` reads `GdContent`. The gadolinia branch of the
  NFIR model is therefore almost certainly never exercised upstream, and it
  is not continuous as the gadolinia content goes to zero. See
  [`ConductivityModel::NfirUo2`].

```rust
pub mod conductivity { /* ... */ }
```

### Types

#### Enum `ConductivityModel`

Thermal conductivity \[W/(m K)\] of a fuel, cladding or structural material.

# What this represents

One published thermal-conductivity correlation, selected at construction and
evaluated per cell against a [`MaterialState`]. Variants are named
`<author-or-source><material>`, following the convention set out in
[`crate::materials`]: `MatproUo2` is the MATPRO-v11 UO2 fit, `RelapZircaloy`
the RELAP Zircaloy fit, and so on.

# Dispatch

This is an enum, not a trait object, per the workspace "no trait objects"
rule: the set of correlations is closed and known at compile time, adding
one is then a compile error at every `match`, and rust-analyzer's
go-to-definition works on the variants.

# Example

```
use outram_park_fork_offbeat::materials::MaterialState;
use outram_park_fork_offbeat::materials::properties::conductivity::ConductivityModel;

// Fresh, 95%-dense UO2 at 1000 K.
let mut state = MaterialState::fresh(1000.0);
state.porosity = 0.05;

let k = ConductivityModel::MatproUo2.value(&state);
assert!((3.0..4.0).contains(&k), "UO2 near 1000 K is a few W/(m K), got {k}");
```

```rust
pub enum ConductivityModel {
    Constant(f64),
    MatproUo2,
    NfirUo2,
    Ifa601Uo2,
    BrancheriaMox,
    KatoMox {
        americium_atom_fraction: f64,
        neptunium_atom_fraction: f64,
    },
    LanningBeyerMox,
    MagniMamox {
        americium_atom_fraction: f64,
        neptunium_atom_fraction: f64,
    },
    PhilipponneauMox,
    RelapZircaloy,
    Molybdenum,
    SwindemanHastelloyN,
    Tobbe1515Ti,
    ParfumeBuffer {
        initial_density: f64,
        theoretical_density: f64,
        initial_conductivity: f64,
        theoretical_conductivity: f64,
    },
    ParfumeSiC,
}
```

##### Variants

###### `Constant`

Temperature-independent conductivity \[W/(m K)\], the payload value.

Upstream: `conductivityConstant` (reads `k` from the case dictionary).

Valid over any temperature — [`temperature_range`] returns
`(0, +inf)` — because a constant is not a fit to anything. Use it for
scoping calculations and for verification cases with an analytical
solution, not for a real material.

[`temperature_range`]: ConductivityModel::temperature_range

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `MatproUo2`

UO2, MATPRO-v11 / modified-NFI fit \[W/(m K)\].

Upstream: `conductivityMatproUO2`. This is the "modified NFI" form used
by MATPRO-v11 and FRAPCON, a phonon term plus an electronic term:

```text
k95 = 1 / (A + B*T + C*Bu + D*gad + (1 - 0.9*exp(-0.04*Bu)) * E*Bu^0.28 * h(T))
      + F/T^2 * exp(-G/T)
h(T) = 1 / (1 + 396*exp(-6380/T))
```

with `A = 0.0452`, `B = 2.46e-4` 1/K, `C = 1.87e-3` kgHM/MWd,
`D = 1.1599`, `E = 0.038`, `F = 3.5e9` W K/m, `G = 16360` K, and `k95`
the conductivity at 95% of theoretical density. It is then rescaled to
the actual density fraction `d` by the Maxwell-Euken-type factor
`1.0789 * d / (1 + 0.5*(1 - d))`, which is unity at `d = 0.95` by
construction.

# Inputs used

[`temperature`](MaterialState::temperature) \[K\],
[`burnup`](MaterialState::burnup) \[MWd/kgHM\],
[`gadolinia_fraction`](MaterialState::gadolinia_fraction) \[weight
fraction\], and the porosity/swelling/densification group through the
evolving density fraction described below.

# Porosity evolution

Upstream corrects the as-fabricated density fraction by a cell-volume
factor `1 + swelling(intragranular + intergranular) + 3*densification`.
The factor of three is there because upstream's `epsilonDensification`
field is a **linear** strain while its swelling fields are volumetric;
[`MaterialState::densification`] is documented as volumetric, so this
port uses `1 + swelling + densification` with no factor. The result is
floored at 0.05 so a pathological input cannot divide by zero.

# Validity range

Upstream states none. This port uses **300–3113 K**, the upper bound
being the melting temperature of unirradiated UO2 as used by MATPRO;
the window is this port's choice, not a bound from the report. No
burnup or gadolinia bound is enforced.

# Source

MATPRO-v11 (SCDAP/RELAP5 material properties library) UO2 thermal
conductivity, in the "modified NFI" form of Ohira & Itagaki, as
implemented in upstream `conductivityMatproUO2.C`.

###### `NfirUo2`

UO2, NFIR (EPRI Nuclear Fuel Industry Research) fit \[W/(m K)\].

Upstream: `conductivityNfirUO2`. Blends a low-temperature and a
high-temperature phonon branch with a `tanh` ramp centred on 900 °C, and
adds an electronic term:

```text
RF   = 0.5*(1 + tanh((t - 900)/150))                 [t in degC]
kLo  = 1/(a + 6.14e-3*Bu - 1.4e-5*Bu^2 + (2.5e-4 - 1.81e-6*Bu)*t)
kHi  = 1/(a + 2.6e-3*Bu + (2.5e-4 - 2.7e-7*Bu)*t)
kEl  = 1.32e-2*exp(1.88e-3*t)
k95  = (1 - RF)*kLo + RF*kHi + kEl
```

with `a = 9.592e-2` for un-poisoned UO2. The porosity rescaling is the
temperature-dependent form
`k95 * (1 - (2.58 - 5.8e-4*t)*(1 - d)) / (1 - 0.05*(2.58 - 5.8e-4*t))`,
again unity at `d = 0.95` by construction.

# Gadolinia branch — do not trust without checking the NFIR report

When [`gadolinia_fraction`](MaterialState::gadolinia_fraction) is
non-zero, `a` is replaced by a polynomial-times-factor expression in the
gadolinia content. Two problems, both inherited from upstream and both
reproduced here so the port stays faithful:

1. The expression contains `tanh(Gd)^0.1`, which tends to **zero** as
   `Gd -> 0`, so `a -> 0` and the conductivity *jumps upward* as an
   infinitesimal amount of burnable poison is added. The branch is
   discontinuous at the origin and does not reduce to the un-poisoned
   `a = 9.592e-2`.
2. Upstream reads the content from a dictionary key named `GdContent_`
   — with a trailing underscore — while every other model reads
   `GdContent`, so this branch is likely dead code upstream and the
   intended unit (weight fraction or weight per cent) cannot be
   determined from the source. This port applies it as a **weight
   fraction**, matching [`MatproUo2`](Self::MatproUo2).

The un-poisoned branch (`gadolinia_fraction == 0`) is unaffected by any
of this.

# Validity range

Upstream states none; this port uses **300–3113 K**, as for
[`MatproUo2`](Self::MatproUo2). Port's choice, not from the report.

# Source

EPRI NFIR UO2 thermal conductivity, as implemented in upstream
`conductivityNfirUO2.C`.

###### `Ifa601Uo2`

UO2, fitted to the Halden IFA-601 instrumented rod \[W/(m K)\].

Upstream: `conductivityUO2IFA601`.

```text
k = 1000 / (40 + 5*Bu + c(Bu)*T) + 1.32e-2*exp(1.88e-3*T)
c(Bu) = 0.24                    for Bu <= 50 MWd/kgHM
      = 0.457 - 0.00433*Bu      for 50 < Bu <= 80
      = 0.11                    for Bu > 80
```

The piecewise coefficient is very nearly continuous at both breakpoints
(0.240 vs 0.2405 at 50 MWd/kgHM; 0.1106 vs 0.110 at 80 MWd/kgHM), which
the unit tests check.

# Inputs used

[`temperature`](MaterialState::temperature) \[K\] and
[`burnup`](MaterialState::burnup) \[MWd/kgHM\]. Note that unlike
[`MatproUo2`](Self::MatproUo2) this correlation carries **no porosity
rescaling** — the density dependence is baked into the rod-specific fit.

# Validity range

Upstream states none; this port uses **300–3113 K**. Port's choice. The
fit is a rod-specific one and should not be used outside the burnup
range of the IFA-601 experiment it was fitted to.

# Source

Fit to the Halden IFA-601 instrumented fuel rod, as implemented in
upstream `conductivityUO2IFA601.C`.

###### `BrancheriaMox`

MOX (U,Pu)O2, Brancheria fit for Pu/HM = 0.25 \[W/(m K)\].

Upstream: `conductivityUPuO2Brancheria`.

```text
k = 100 * D1(p) * ( 1/(2.88 + 0.0252*T) + 5.83e-13*T^3 )
D1 = 1 - 1.5*p                          for p <= 0.05
   = 1 - pc - 10*pc^2, pc = min(p,0.15) for p >  0.05
```

`D1` is continuous at `p = 0.05` (both branches give 0.925) and its
porosity argument is cut off at 15%.

# Inputs used

[`temperature`](MaterialState::temperature) \[K\] and
[`porosity`](MaterialState::porosity) \[-\]. The fit is for 25% Pu
content and takes no Pu argument; using it for another Pu fraction is
an extrapolation the correlation cannot express.

# Validity range

Upstream states none; this port uses **300–3000 K**. Port's choice.

# Source

Brancheria et al., *Calibration of a fuel-to-cladding gap conductance
model for fast reactor fuel pins*, Hanford Engineering Development
Laboratory, <https://www.osti.gov/servlets/purl/6863540>; cited by
upstream `conductivityUPuO2Brancheria.H`.

Upstream also multiplies the result by a dictionary-set `perturb`
factor (default 1.0) for uncertainty quantification; that knob is not
exposed here.

###### `KatoMox`

MOX (U,Pu)O2, Kato fit with minor actinides \[W/(m K)\].

Upstream: `conductivityUPuO2Kato`, as used by Novascone et al. (2018).

```text
den = 1.595e-2 + 2.713*x + 3.583e-1*cAm + 6.317e-2*cNp
      + (-2.625*x + 2.493)*1e-4*T
k   = 1/den + 1.541e11 * T^-2.5 * exp(-1.522e4/T)
```

where `x = 2 - O/M` is the **hypo**stoichiometry (positive for
oxygen-deficient fuel), and `cAm`, `cNp` are americium and neptunium
concentrations in mass per mass of fuel. The porosity correction is
Barani's Maxwell-Eucken form with a helium-filled pore conductivity of
0.69 W/(m K):

```text
k_eff = k * (kHe + 2k - 2p(k - kHe)) / (kHe + 2k + p(k - kHe))
```

which returns `k` exactly at `p = 0` and `kHe` exactly at `p = 1`.

# Payload — minor actinide content

[`MaterialState`] carries no americium or neptunium field, so the two
minor-actinide **atom fractions of the heavy metal** (atoms of Am, resp.
Np, per atom of U + Pu + Am + Np) are carried on the variant. Pass
`0.0` for both for ordinary MOX. They are converted to mass fractions
internally with upstream's approximate factors `k_Am = 1.12` and
`k_Np = 1.14`.

# Inputs used

[`temperature`](MaterialState::temperature) \[K\],
[`porosity`](MaterialState::porosity) \[-\] and
[`oxygen_deviation`](MaterialState::oxygen_deviation) \[-\] (this
crate's `x` in `(U,Pu)O_{2+x}`, which is the **negative** of upstream's
`x = 2 - O/M`). Kato's fit is independent of the Pu content, so
[`pu_fraction`](MaterialState::pu_fraction) is not used — upstream
computes a Pu concentration in this model and then never reads it.

# Validity range

Upstream states none; this port uses **300–3000 K**. Port's choice.

# Source

S. Novascone et al., *Modeling porosity migration in LWR and fast
reactor MOX fuel using the finite element method*, Journal of Nuclear
Materials 508 (2018) 226–236,
<https://doi.org/10.1016/j.jnucmat.2018.05.041>; cited by upstream
`conductivityUPuO2Kato.H`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `americium_atom_fraction` | `f64` | Americium content \[atoms Am per atom of heavy metal\]. Zero for<br>ordinary MOX. |
| `neptunium_atom_fraction` | `f64` | Neptunium content \[atoms Np per atom of heavy metal\]. Zero for<br>ordinary MOX. |

###### `LanningBeyerMox`

MOX (U,Pu)O2, Lanning & Beyer fit \[W/(m K)\].

Upstream: `conductivityUPuO2LanningBeyer`. Structurally the MATPRO UO2
form with stoichiometry-dependent `A` and `B` coefficients:

```text
A   = 0.035 + 2.8*x                     [x = 2 - O/M]
B   = (2.86 - 7.15*x)*1e-4
k95 = 1/(A + B*T + 1.87e-3*Bu + (1 - 0.9*exp(-0.04*Bu))*0.038*Bu^0.28*h(T))
      + 1.5e9/T^2 * exp(-13520/T)
h(T) = 1/(1 + 396*exp(-6380/T))
```

rescaled to the actual porosity by `1.0789*(1 - p)/(1 + 0.5*p)`, unity
at `p = 0.05` by construction.

# Inputs used

[`temperature`](MaterialState::temperature) \[K\],
[`burnup`](MaterialState::burnup) \[MWd/kgHM\],
[`porosity`](MaterialState::porosity) \[-\] and
[`oxygen_deviation`](MaterialState::oxygen_deviation) \[-\]. Upstream
takes the porosity from `1 - densityFraction` in the case dictionary,
i.e. it is fixed for the run; this port takes it from the state, so it
can evolve.

# Validity range

Upstream states none — the header's `Description` block is empty. This
port uses **300–3000 K**. Port's choice.

# Source

Lanning & Beyer MOX thermal conductivity, as implemented in upstream
`conductivityUPuO2LanningBeyer.C`. Upstream cites no report for it;
the coefficients are reproduced from that source file.

###### `MagniMamox`

Minor-actinide-bearing MOX, Magni MAMOX correlation \[W/(m K)\].

Upstream: `conductivityUPuO2MagniMAMOX`. A fresh-fuel conductivity `k0`
with composition-dependent phonon coefficients, a porosity factor
`(1 - p)^2.5`, and an exponential relaxation towards an irradiated
asymptote:

```text
k0 = [ 1/(A(x, cPu, cAm, cNp) + B(cPu, cAm, cNp)*T) + 5.27e9/T^2*exp(-17109.5/T) ]
     * (1 - p)^2.5
k  = kInf + (k0 - kInf)*exp(-Bu/phi),  kInf = 1.755 W/(m K)
```

with `phi = 128.75` MWd/kgHM (upstream writes it as `128.75e3` in its
own MWd/tHM burnup unit). At zero burnup `k = k0` exactly; as burnup
grows without bound `k -> kInf`.

# Payload — minor actinide content

As for [`KatoMox`](Self::KatoMox): the Am and Np **atom fractions of the
heavy metal**, converted internally to mass fractions with upstream's
factors `k_Pu = 1.13`, `k_Am = 1.12`, `k_Np = 1.14`. Pass `0.0` for
both for ordinary MOX — in which case only the plutonium term of `A`
and `B` is active.

# Inputs used

[`temperature`](MaterialState::temperature) \[K\],
[`burnup`](MaterialState::burnup) \[MWd/kgHM\],
[`porosity`](MaterialState::porosity) \[-\] (cut off at 0.95),
[`pu_fraction`](MaterialState::pu_fraction) \[atoms Pu per atom HM\] and
[`oxygen_deviation`](MaterialState::oxygen_deviation) \[-\].

# Validity range

Upstream states none; this port uses **300–3000 K**. Port's choice.

# Source

A. Magni et al., *Modelling of thermal conductivity and melting
behaviour of minor actinide-MOX fuels and assessment against
experimental and molecular dynamics data*, Journal of Nuclear Materials
(2021), <https://doi.org/10.1016/j.jnucmat.2021.153312>; cited by
upstream `conductivityUPuO2MagniMAMOX.H`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `americium_atom_fraction` | `f64` | Americium content \[atoms Am per atom of heavy metal\]. Zero for<br>ordinary MOX. |
| `neptunium_atom_fraction` | `f64` | Neptunium content \[atoms Np per atom of heavy metal\]. Zero for<br>ordinary MOX. |

###### `PhilipponneauMox`

MOX (U,Pu)O2-x, Philipponneau (1992) fit \[W/(m K)\].

Upstream: `conductivityUPuO2Philipponneau`.

```text
A     = 1.528*sqrt(x + 0.00931) - 0.1055 + 0.44*tau
alpha = (1/0.864) * (1 - p)/(1 + 2p)
k     = ( 1/(A + 2.885e-4*T) + 76.38e-12*T^3 ) * alpha
```

where `x = 2 - O/M` is the hypostoichiometry and `tau` the fractional
burnup in FIMA. Upstream converts burnup to FIMA with the MOX-specific
factor of 9.5 GWd/tHM per %FIMA; with this crate's MWd/kgHM burnup that
becomes `tau = Bu/950`. `alpha` is unity at `p = 0.05` by construction.

The square root is guarded with a floor of zero: for
**hyper**stoichiometric fuel with `oxygen_deviation > 0.00931` the
argument goes negative and upstream would produce a NaN.

# Inputs used

[`temperature`](MaterialState::temperature) \[K\],
[`burnup`](MaterialState::burnup) \[MWd/kgHM\],
[`porosity`](MaterialState::porosity) \[-\] (cut off at 0.95) and
[`oxygen_deviation`](MaterialState::oxygen_deviation) \[-\]. As
upstream's header notes, the MOX conductivity in this correlation is
**independent of the Pu content**.

# Validity range

Upstream's header states *500 K < T < melting T*. This port uses
**500–3000 K**: the lower bound is the stated one, the upper bound is
the port's stand-in for the (composition-dependent) melting
temperature, which the reference does not give as a number.

# Source

Y. Philipponneau, *Thermal conductivity of (U,Pu)O2-x mixed oxide
fuel*, Journal of Nuclear Materials (1992),
<https://doi.org/10.1016/0022-3115(92)90470-6>; FIMA conversion factor
from <https://doi.org/10.1016/j.pnucene.2017.03.016>. Both cited by
upstream `conductivityUPuO2Philipponneau.H`.

###### `RelapZircaloy`

Zircaloy cladding, RELAP fit \[W/(m K)\].

Upstream: `conductivityRelapZy`.

```text
k = 7.51 + 2.09e-2*T - 1.45e-5*T^2 + 7.67e-9*T^3
```

# Inputs used

[`temperature`](MaterialState::temperature) \[K\] only.

# Validity range

**290–1800 K**, stated by upstream, which prints a warning and then
extrapolates anyway. This port clamps in [`value`] and reports
[`OutOfRange`] in [`value_checked`].

[`value`]: ConductivityModel::value
[`value_checked`]: ConductivityModel::value_checked
[`OutOfRange`]: crate::error::OffbeatError::OutOfRange

# Source

RELAP Zircaloy thermal conductivity, as implemented in upstream
`conductivityRelapZy.C`.

###### `Molybdenum`

Molybdenum \[W/(m K)\].

Upstream: `conductivityMolybdenum`.

```text
k = 9.128e-6*T^2 - 4.945e-2*T + 152
```

Note the *negative* linear term: molybdenum's conductivity falls with
temperature from about 138 W/(m K) at room temperature, the quadratic
term turning it back up only above roughly 2700 K.

# Inputs used

[`temperature`](MaterialState::temperature) \[K\] only.

# Validity range

Upstream states none; this port uses **300–2800 K**, the upper bound
being below molybdenum's melting point (about 2896 K). Port's choice.

# Source

Upstream `conductivityMolybdenum.C`; upstream cites no report.

###### `SwindemanHastelloyN`

Hastelloy N, Swindeman fit \[W/(m K)\].

Upstream: `conductivitySwindemanHastelloyN`.

```text
k = 8.431 + 0.0205*(T - 273.15)     [T in K]
```

# Inputs used

[`temperature`](MaterialState::temperature) \[K\] only.

# Validity range

Upstream states none; this port uses **300–1200 K**, a conservative
engineering window that covers the alloy's service range and stays well
below its melting range. Port's choice, not a bound from Swindeman.

# Source

R. W. Swindeman's Hastelloy-N property work, as implemented in upstream
`conductivitySwindemanHastelloyN.C` (upstream gives no full citation).

###### `Tobbe1515Ti`

15-15 Ti austenitic stainless cladding, Tobbe (1975) fit \[W/(m K)\].

Upstream: `conductivityTobbe1515Ti`.

```text
k = 13.95 + 1.163e-2*(T - 273.15)   [T in K]
```

# Inputs used

[`temperature`](MaterialState::temperature) \[K\] only.

# Validity range

Upstream states none for the conductivity. This port adopts
**293–1273 K**, the window upstream *does* state for the Banerjee
15-15 Ti heat-capacity fit of the same alloy. Port's choice, not
Tobbe's stated range.

# Source

Tobbe correlation (1975) for 15-15 Ti, as implemented in upstream
`conductivityTobbe1515Ti.C`.

###### `ParfumeBuffer`

TRISO buffer layer (porous pyrolytic carbon), PARFUME model
\[W/(m K)\].

Upstream: `conductivityPARFUMEBuffer`. A density-interpolating form
that passes exactly through two anchor points — the as-fabricated
buffer (`rho_init`, `k_init`) and fully dense pyrocarbon
(`rho_theo`, `k_theo`):

```text
k = k_init*k_theo*rho_theo*(rho_theo - rho_init)
    / ( k_theo*rho_theo*(rho_theo - rho) + k_init*rho*(rho - rho_init) )
```

# Inputs used

[`porosity`](MaterialState::porosity) \[-\] only — **this model is
temperature-independent**. Upstream reads the current density from a
`rho` field on the mesh; this port reconstructs it from the state as
`rho = theoretical_density * (1 - porosity)`, which is the definition
of porosity for a single-phase solid and is the only density
information [`MaterialState`] carries.

# Validity range

None — [`temperature_range`] returns `(0, +inf)` because there is no
temperature dependence to invalidate.

[`temperature_range`]: ConductivityModel::temperature_range

# Source

PARFUME (INL TRISO fuel-performance code) buffer conductivity model, as
implemented in upstream `conductivityPARFUMEBuffer.C`. Use
[`ConductivityModel::parfume_buffer_default`] for upstream's default
parameters.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `initial_density` | `f64` | As-fabricated buffer density \[kg/m^3\] (upstream default 1000). |
| `theoretical_density` | `f64` | Theoretical (fully dense) pyrocarbon density \[kg/m^3\] (upstream<br>default 2250). |
| `initial_conductivity` | `f64` | Conductivity at the as-fabricated density \[W/(m K)\] (upstream<br>default 0.5). |
| `theoretical_conductivity` | `f64` | Conductivity at theoretical density \[W/(m K)\] (upstream default<br>4.0). |

###### `ParfumeSiC`

TRISO silicon-carbide layer, PARFUME model \[W/(m K)\].

Upstream: `conductivityPARFUMESiC`.

```text
k = 17885/T + 2
```

The `1/T` form is the irradiated-SiC behaviour PARFUME uses: about
62 W/(m K) at 300 K falling to about 16 W/(m K) at 1273 K.

# Inputs used

[`temperature`](MaterialState::temperature) \[K\] only.

# Validity range

Upstream states none for the conductivity. This port adopts
**200–2400 K**, the window upstream states for the Snead SiC
heat-capacity fit of the same material. Port's choice.

# Source

PARFUME manual, SiC conductivity, as implemented in upstream
`conductivityPARFUMESiC.C`. (Upstream's header calls it "PyC material
from PARFUME manual" while the class and the parameters are the SiC
ones — the header text appears to be a copy-paste slip.)

##### Implementations

###### Methods

- ```rust
  pub const fn parfume_buffer_default() -> Self { /* ... */ }
  ```
  [`ParfumeBuffer`](Self::ParfumeBuffer) with upstream's default

- ```rust
  pub const fn name(self: &Self) -> &'static str { /* ... */ }
  ```
  Short human-readable name of the correlation, for error messages and

- ```rust
  pub const fn temperature_range(self: &Self) -> (f64, f64) { /* ... */ }
  ```
  Temperature validity window `(low, high)` \[K\] of this correlation.

- ```rust
  pub fn value(self: &Self, state: &MaterialState) -> f64 { /* ... */ }
  ```
  Thermal conductivity \[W/(m K)\], **clamping** the temperature into

- ```rust
  pub fn value_checked(self: &Self, state: &MaterialState) -> Result<f64> { /* ... */ }
  ```
  Thermal conductivity \[W/(m K)\], **reporting** an out-of-range

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
    fn clone(self: &Self) -> ConductivityModel { /* ... */ }
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
    fn eq(self: &Self, other: &ConductivityModel) -> bool { /* ... */ }
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
## Module `density`

Density correlations \[kg/m^3\].

# Porosity is the thing to get right here

"Density" means two different numbers in fuel-performance work, and mixing
them up misstates the fuel mass in a rod by five per cent:

- the **theoretical density** of the fully dense crystal (10 960 kg/m^3 for
  UO2), and
- the **smeared density** of the as-fabricated pellet, which contains a few
  per cent of fabrication porosity and is therefore lower.

Every variant in this module states explicitly, in its own doc comment,
whether the number it returns **already includes** the porosity correction
or whether it is a fully dense value. The rule for this port:

- [`UO2`](DensityModel::UO2) and [`UPuO2`](DensityModel::UPuO2) apply
  [`MaterialState::density_fraction`] internally. **Do not apply it again.**
- [`Constant`](DensityModel::Constant),
  [`Molybdenum`](DensityModel::Molybdenum),
  [`IAEAZy`](DensityModel::IAEAZy) and
  [`Schumann1515Ti`](DensityModel::Schumann1515Ti) return the density of the
  dense material and ignore [`porosity`](MaterialState::porosity) entirely —
  which is right for cladding and structure, and is a caller responsibility
  if one of them is ever pointed at a porous body.

# Temperature dependence

Only two of the six vary with temperature. That is faithful to upstream:
OFFBEAT holds the *fuel* density fixed and carries fuel volume change
through the strain field (thermal expansion, swelling, densification) rather
than through `rho`, so making the fuel density temperature-dependent as well
would double-count. [`IAEAZy`](DensityModel::IAEAZy) and
[`Schumann1515Ti`](DensityModel::Schumann1515Ti) are cladding correlations
published directly as `rho(T)` and are ported as such.

# Units

Raw `f64` in strict SI: temperature in kelvin, density in kg/m^3.

```rust
pub mod density { /* ... */ }
```

### Types

#### Enum `DensityModel`

A published correlation for the mass density of a fuel, cladding or
structural material.

Evaluate with [`value`](Self::value) for kg/m^3, or
[`value_checked`](Self::value_checked) to be told when the correlation is
being pushed outside the range it was fitted over.

Read the [module documentation](self) on porosity before choosing a variant:
two of the six apply [`MaterialState::density_fraction`] internally and four
do not.

# Example

```
use outram_park_fork_offbeat::materials::MaterialState;
use outram_park_fork_offbeat::materials::properties::density::{
    DensityModel, UO2_THEORETICAL_DENSITY,
};

// A 95 %-dense UO2 pellet: 5 % fabrication porosity.
let model = DensityModel::UO2 {
    theoretical_density: UO2_THEORETICAL_DENSITY,
};
let mut state = MaterialState::fresh(900.0);
state.porosity = 0.05;

// The porosity is already in the answer — do not multiply by 0.95 again.
assert!((model.value(&state) - 0.95 * 10960.0).abs() < 1e-9);
```

```rust
pub enum DensityModel {
    Constant {
        density: f64,
    },
    UO2 {
        theoretical_density: f64,
    },
    UPuO2 {
        theoretical_density: f64,
    },
    Molybdenum {
        density: f64,
    },
    IAEAZy,
    Schumann1515Ti {
        reference_density: f64,
    },
}
```

##### Variants

###### `Constant`

A single density supplied by the case \[kg/m^3\].

Upstream `densityConstant`.

# Porosity

**Not applied.** The number is returned exactly as given, so whatever
porosity correction is wanted must already be baked into it.

# Validity

None — a user-supplied constant carries no fitted range. `*_checked`
only rejects a non-positive temperature or a non-positive density.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `density` | `f64` | Density \[kg/m^3\]. Must be strictly positive. |

###### `UO2`

UO2 fuel: `rho = density_fraction * theoretical_density`.

Upstream `constantDensityUO2`, which multiplies a dictionary
`densityFraction` (default 0.95) by `theoreticalDensity` (default
10 960 kg/m^3). This port takes the fraction from the material state
instead, so that as-fabricated porosity and its evolution live in one
place.

# Porosity

**Applied internally**, via [`MaterialState::density_fraction`] — which
is `1 - porosity`, floored at 0.05. Do not multiply by it again.

# Temperature

Independent of temperature by design: fuel volume change is carried by
the strain field (see the [module documentation](self)), not by `rho`.

# Validity

**Upstream states no validity range and this port enforces none.**

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `theoretical_density` | `f64` | Fully dense, pore-free density \[kg/m^3\]; upstream default<br>[`UO2_THEORETICAL_DENSITY`]. |

###### `UPuO2`

(U,Pu)O2 MOX fuel: `rho = density_fraction * theoretical_density`.

Upstream `constantDensityUPuO2`, which is explicit about the intent: it
looks up the live `porosity` field where one exists so that pore
migration and central-void formation change the local density, falling
back to `1 - densityFraction` (default fraction 0.945) otherwise. In
this port [`MaterialState::porosity`] *is* the live value, so the two
paths collapse into one.

# Porosity

**Applied internally**, via [`MaterialState::density_fraction`]. Do not
multiply by it again.

# Validity

**Upstream states no validity range and this port enforces none.**

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `theoretical_density` | `f64` | Fully dense, pore-free density \[kg/m^3\]; upstream default<br>[`UPUO2_THEORETICAL_DENSITY`]. |

###### `Molybdenum`

Molybdenum structural material, constant `10 280 kg/m^3`.

Upstream `constantDensityMolybdenum`.

# Porosity

**Not applied** — the value is that of the dense metal.

# Validity

**Upstream states no validity range and this port enforces none.**

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `density` | `f64` | Density \[kg/m^3\]; upstream default [`MOLYBDENUM_DENSITY`]. |

###### `IAEAZy`

Zircaloy cladding, IAEA correlation, with the alpha → beta phase
transition.

Two linear branches and a blend across the transformation:

- `T < 1083 K` (alpha): `rho = 6595.2 - 0.1477*T`
- `1083 <= T < 1144 K`: linear blend between the two branches
- `1144 <= T <= 1800 K` (beta): `rho = 6690.0 - 0.1855*T`

The density **rises** across the blend (about `6435` to `6478 kg/m^3`)
because the hexagonal-to-cubic transformation contracts the metal. That
is the same physics as the negative expansion coefficient reported by
[`ThermalExpansionModel::MatproZy`](super::thermal_expansion::ThermalExpansionModel::MatproZy)
over its own 1073-1273 K transition window, and the two are
cross-checked against each other in this module's tests.

Upstream `densityIAEAZy`.

# Porosity

**Not applied** — cladding is treated as fully dense.

# Validity — stated upstream, and enforced

Upper bound **1800 K**, from upstream's warning "*Supplied temperature
… above maximum of 1800 K*". No lower bound is stated, so none is
enforced.

# Deviation from upstream

Above 1800 K upstream warns and then returns a density of **zero**,
which is not a density and would divide by zero in any mass or
heat-capacity term downstream. This port clamps to the 1800 K value in
[`value`](Self::value) and returns [`OffbeatError::OutOfRange`] from
[`value_checked`](Self::value_checked) instead.

###### `Schumann1515Ti`

15-15Ti austenitic stainless cladding, Schumann (1970).

`rho(T) = rho_0 / (1 + eps(T))^3` with the linear thermal strain
`eps(T) = -3.101e-4 + 1.545e-5*T_C + 2.75e-9*T_C^2`, `T_C = T - 273.15`,
and `rho_0 = 7900 kg/m^3` at 20 °C. The cube converts the linear strain
to a volumetric one, so this is mass conservation applied to the Gehr
thermal-expansion fit and nothing more — the same three coefficients
appear in
[`ThermalExpansionModel::Gehr1515Ti`](super::thermal_expansion::ThermalExpansionModel::Gehr1515Ti).

Upstream `densitySchumann1515Ti`.

# Porosity

**Not applied** — cladding is treated as fully dense.

# Validity

**Upstream states no validity range and this port enforces none.** Note
that unlike the Gehr expansion model this correlation has no 293 K
cut-off: below 20 °C it returns a density slightly above `rho_0`, which
is the physically correct direction.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `reference_density` | `f64` | Density at 20 °C \[kg/m^3\]; upstream default<br>[`SCHUMANN_1515TI_REFERENCE_DENSITY`]. |

##### Implementations

###### Methods

- ```rust
  pub fn name(self: &Self) -> &'static str { /* ... */ }
  ```
  Human-readable name of the correlation, used in error messages.

- ```rust
  pub fn validity_range(self: &Self) -> (f64, f64) { /* ... */ }
  ```
  Temperature range \[K\] over which this port *enforces* the correlation,

- ```rust
  pub fn value(self: &Self, state: &MaterialState) -> f64 { /* ... */ }
  ```
  Mass density \[**kg/m^3**\].

- ```rust
  pub fn value_checked(self: &Self, state: &MaterialState) -> Result<f64> { /* ... */ }
  ```
  Mass density \[kg/m^3\], or [`OffbeatError`] if the correlation is being

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
    fn clone(self: &Self) -> DensityModel { /* ... */ }
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
    fn eq(self: &Self, other: &DensityModel) -> bool { /* ... */ }
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
### Constants and Statics

#### Constant `UO2_THEORETICAL_DENSITY`

Theoretical (fully dense, pore-free) density of UO2 at room temperature
\[kg/m^3\]: `10960.0`.

Upstream default of `theoreticalDensity` in `constantDensityUO2.C`.

```rust
pub const UO2_THEORETICAL_DENSITY: f64 = 10960.0;
```

#### Constant `UO2_DEFAULT_DENSITY_FRACTION`

Upstream default fraction of theoretical density for UO2 pellets \[-\]:
`0.95`.

Provided for reference only — this port takes the density fraction from
[`MaterialState::density_fraction`] (i.e. from
[`porosity`](MaterialState::porosity)) so that fabrication porosity and its
in-service evolution are described in one place. A caller reproducing an
upstream case with the default should set `porosity = 0.05`.

```rust
pub const UO2_DEFAULT_DENSITY_FRACTION: f64 = 0.95;
```

#### Constant `UPUO2_THEORETICAL_DENSITY`

Theoretical (fully dense, pore-free) density of (U,Pu)O2 MOX at room
temperature \[kg/m^3\]: `10430.0`.

Upstream default of `theoreticalDensity` in `constantDensityUPuO2.C`. Lower
than UO2 because PuO2 is lighter per unit cell than UO2.

```rust
pub const UPUO2_THEORETICAL_DENSITY: f64 = 10430.0;
```

#### Constant `UPUO2_DEFAULT_DENSITY_FRACTION`

Upstream default fraction of theoretical density for MOX pellets \[-\]:
`0.945`. See [`UO2_DEFAULT_DENSITY_FRACTION`] for how this port uses it.

```rust
pub const UPUO2_DEFAULT_DENSITY_FRACTION: f64 = 0.945;
```

#### Constant `MOLYBDENUM_DENSITY`

Density of molybdenum \[kg/m^3\]: `10280.0`, the upstream default of
`densityValue` in `constantDensityMolybdenum.C`.

```rust
pub const MOLYBDENUM_DENSITY: f64 = 10280.0;
```

#### Constant `SCHUMANN_1515TI_REFERENCE_DENSITY`

Reference density of 15-15Ti austenitic stainless steel at 20 °C
\[kg/m^3\]: `7900.0`, the upstream default `rho0_` in
`densitySchumann1515Ti.C`.

```rust
pub const SCHUMANN_1515TI_REFERENCE_DENSITY: f64 = 7900.0;
```

## Module `emissivity`

Surface emissivity correlations \[-\].

# What emissivity is used for here

Emissivity is the **hemispherical total emissivity** of a surface: the
fraction of black-body radiation it actually emits, between 0 (perfect
mirror) and 1 (black body). In a fuel rod it appears in exactly one place
that matters — the radiative term of the **fuel-cladding gap conductance**,

`h_rad = sigma * F * (T_f^2 + T_c^2) * (T_f + T_c)`

with the exchange factor `F = 1 / (1/eps_fuel + 1/eps_clad - 1)` for
concentric cylinders. Because `F` divides by the emissivities, an emissivity
of zero is not merely inaccurate — it is a division by zero. Every method
here therefore guarantees a result strictly inside `[0, 1]`, and the type
system cannot do that for you, so the guarantee is enforced at runtime.

Radiation is a small part of gap conductance while the gap is open and gas
filled, and grows in importance as the gas thins (high fission-gas release,
helium replaced by xenon) and as temperatures rise in a transient. It is not
a term to leave crudely estimated in a LOCA calculation.

# Upstream's model set is small, and mostly constant

Only one of the four correlations varies with temperature. Upstream carries
the other three as named constants rather than fits, and this port keeps
that shape — the value of `constantEmissivityZy` is a *number a case
reproduces*, so it deserves a named variant and a documented provenance,
not to be flattened into [`Constant`](EmissivityModel::Constant).

# Units

Raw `f64`: temperature in kelvin, emissivity dimensionless in `[0, 1]`.

```rust
pub mod emissivity { /* ... */ }
```

### Types

#### Enum `EmissivityModel`

A published correlation for the hemispherical total emissivity of a fuel or
cladding surface.

Evaluate with [`value`](Self::value) for a dimensionless emissivity in
`[0, 1]`, or [`value_checked`](Self::value_checked) to be told when the
correlation is being pushed outside the range upstream states.

# Example

```
use outram_park_fork_offbeat::materials::MaterialState;
use outram_park_fork_offbeat::materials::properties::emissivity::{
    EmissivityModel, ZY_EMISSIVITY,
};

let fuel = EmissivityModel::RelapUO2;
let clad = EmissivityModel::Zy { emissivity: ZY_EMISSIVITY };

let eps_f = fuel.value(&MaterialState::fresh(1200.0));
let eps_c = clad.value(&MaterialState::fresh(600.0));

// Concentric-cylinder exchange factor for the gap radiation term.
let exchange_factor = 1.0 / (1.0 / eps_f + 1.0 / eps_c - 1.0);
assert!(exchange_factor > 0.0 && exchange_factor < 1.0);
```

```rust
pub enum EmissivityModel {
    Constant {
        emissivity: f64,
    },
    RelapUO2,
    Zy {
        emissivity: f64,
    },
    Molybdenum {
        emissivity: f64,
    },
}
```

##### Variants

###### `Constant`

A single emissivity supplied by the case \[-\].

Upstream `emissivityConstant`.

# Validity

None — a user-supplied constant carries no fitted range. `*_checked`
rejects a non-positive temperature, and an emissivity outside `[0, 1]`
as [`OffbeatError::Unphysical`]; [`value`](Self::value) clamps such a
value into range instead.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `emissivity` | `f64` | Hemispherical total emissivity \[-\], in `[0, 1]`. |

###### `RelapUO2`

UO2 fuel surface, RELAP-derived linear fit:
`eps = 0.7856 + 1.5263e-5 * T`.

The only temperature-dependent correlation in this module. It rises
gently — `0.790` at 300 K, `0.804` at 1200 K, `0.833` at 3120 K — so
UO2 is close to, but never quite, a black body.

Upstream `emissivityRelapUO2`.

# Validity

**Upstream states no validity range and this port enforces none.** Note
that the linear form crosses `eps = 1` at about 14 000 K; that is far
outside any physical application, but [`value`](Self::value) clamps the
result into `[0, 1]` regardless, because a caller must never receive an
emissivity above unity.

###### `Zy`

Zircaloy cladding surface, constant `0.808642`.

Upstream `constantEmissivityZy`.

# Validity — stated upstream, and enforced

Upper bound **1500 K**, from upstream's warning "*Supplied temperature
… out of range T < 1500 K*". No lower bound is stated, so none is
enforced. Since the value is constant, clamping above 1500 K changes
nothing numerically — but
[`value_checked`](Self::value_checked) still reports the excursion,
which is the point: beyond 1500 K a Zircaloy surface is oxidising
rapidly and its emissivity is no longer a constant at all.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `emissivity` | `f64` | Hemispherical total emissivity \[-\]; upstream default<br>[`ZY_EMISSIVITY`]. |

###### `Molybdenum`

Molybdenum surface, constant `0.2`.

Upstream `constantEmissivityMolybdenum`.

# Validity

**Upstream states no validity range and this port enforces none.**

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `emissivity` | `f64` | Hemispherical total emissivity \[-\]; upstream default<br>[`MOLYBDENUM_EMISSIVITY`]. |

##### Implementations

###### Methods

- ```rust
  pub fn name(self: &Self) -> &'static str { /* ... */ }
  ```
  Human-readable name of the correlation, used in error messages.

- ```rust
  pub fn validity_range(self: &Self) -> (f64, f64) { /* ... */ }
  ```
  Temperature range \[K\] over which this port *enforces* the correlation,

- ```rust
  pub fn value(self: &Self, state: &MaterialState) -> f64 { /* ... */ }
  ```
  Hemispherical total emissivity \[**dimensionless**\], guaranteed to lie

- ```rust
  pub fn value_checked(self: &Self, state: &MaterialState) -> Result<f64> { /* ... */ }
  ```
  Hemispherical total emissivity \[-\], or [`OffbeatError`] if the

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
    fn clone(self: &Self) -> EmissivityModel { /* ... */ }
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
    fn eq(self: &Self, other: &EmissivityModel) -> bool { /* ... */ }
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
### Constants and Statics

#### Constant `ZY_EMISSIVITY`

Emissivity of Zircaloy cladding \[-\]: `0.808642`, the upstream default of
`emissivityValue` in `constantEmissivityZy.C`.

The six-figure precision is upstream's, not a claim about the measurement:
oxidised Zircaloy emissivity in the literature scatters over roughly
0.3-0.9 depending on oxide thickness, and this is one number from that
range.

```rust
pub const ZY_EMISSIVITY: f64 = 0.808642;
```

#### Constant `MOLYBDENUM_EMISSIVITY`

Emissivity of molybdenum \[-\]: `0.2`, the upstream default of
`emissivityValue` in `constantEmissivityMolybdenum.C`.

Low, as expected for a clean refractory metal surface — an order of
magnitude less radiative coupling than oxidised Zircaloy at the same
temperature.

```rust
pub const MOLYBDENUM_EMISSIVITY: f64 = 0.2;
```

## Module `heat_capacity`

Specific heat capacity correlations \[J/(kg K)\].

# What this module computes

The **specific heat capacity at constant pressure** `Cp` of fuel, cladding
and structural materials, in J/(kg K), as a function of the local
[`MaterialState`].

Heat capacity does not appear in a steady-state temperature solution at all
— it is the property that sets the **thermal inertia** of a transient. It
decides how fast the fuel centreline responds to a power ramp, how much
stored energy a rod holds at the start of an accident, and how sharply the
cladding heats up during a temperature excursion. The Zircaloy fits below
carry a large peak near 1150 K precisely because the alpha-to-beta phase
transformation absorbs latent heat there, and that peak is what slows a
LOCA heat-up.

# Units — raw `f64`, strict SI

Inputs come from [`MaterialState`] (temperature in K, and for the oxide
fuels the plutonium fraction and the deviation from stoichiometry); the
returned value is always in **J/(kg K)**.

# Validity ranges: `value` clamps, `value_checked` reports

Identical convention to [`conductivity`](super::conductivity):

- [`HeatCapacityModel::value`] **clamps the temperature** into
  [`HeatCapacityModel::temperature_range`] before evaluating.
- [`HeatCapacityModel::value_checked`] returns
  [`OffbeatError::OutOfRange`] outside that window and
  [`OffbeatError::Unphysical`] for a non-positive absolute temperature.

Upstream warns and extrapolates rather than clamping (`heatCapacityMatproZy`,
`heatCapacityIAEAZy`, `heatCapacityBanerjee1515Ti` and
`heatCapacitySneadSiC` all print a `WarningInFunction` and carry on), so the
clamp in [`value`] is this port's deliberate, documented choice. Four of the
nine models state their window in their own source; the rest are
port-chosen and say so in their doc comments.

[`value`]: HeatCapacityModel::value
[`OffbeatError::OutOfRange`]: crate::error::OffbeatError::OutOfRange
[`OffbeatError::Unphysical`]: crate::error::OffbeatError::Unphysical

# Known upstream defects reproduced here

Both are reproduced faithfully — a port that silently repairs its upstream
stops being comparable to it — and both are characterised by a unit test:

- **`heatCapacityMatproZy` has an off-by-one-interval bug.** Its
  1093–1113 K branch interpolates from `Tlow = 1090` instead of
  `Tlow = 1093`, which puts a spurious 11.5 J/(kg K) step at 1093 K. See
  [`HeatCapacityModel::MatproZircaloy`].
- **`heatCapacityMatproUPuO2` and `heatCapacitySneadSiC` each declare a
  coefficient twice, with different values.** The constructor initialiser
  list and the dictionary `lookupOrDefault` default disagree
  (`K2 = 3.95e-4` vs `3.95e4`; `par4 = -3.1946e7` vs `-3.19446e7`). This
  port uses the initialiser value in both cases — that is the value
  upstream actually uses when the optional `heatCapacity` sub-dictionary is
  absent, and for SiC it is also the value published by Snead et al.

```rust
pub mod heat_capacity { /* ... */ }
```

### Types

#### Enum `HeatCapacityModel`

Specific heat capacity \[J/(kg K)\] of a fuel, cladding or structural
material.

# What this represents

One published heat-capacity correlation, selected at construction and
evaluated per cell against a [`MaterialState`]. Variants are named
`<author-or-source><material>` following the convention in
[`crate::materials`].

# Dispatch

An enum rather than a trait object, per the workspace "no trait objects"
rule — see [`crate::materials`] for the reasoning.

# Example

```
use outram_park_fork_offbeat::materials::MaterialState;
use outram_park_fork_offbeat::materials::properties::heat_capacity::HeatCapacityModel;

// UO2 at room temperature.
let cp = HeatCapacityModel::MatproUo2.value(&MaterialState::fresh(300.0));
assert!((230.0..245.0).contains(&cp), "UO2 Cp near 300 K is ~235 J/(kg K), got {cp}");
```

```rust
pub enum HeatCapacityModel {
    Constant(f64),
    MatproUo2,
    MatproMox,
    FinkMox,
    MatproZircaloy,
    IaeaZircaloy,
    Banerjee1515Ti,
    Molybdenum,
    SneadSiC,
}
```

##### Variants

###### `Constant`

Temperature-independent heat capacity \[J/(kg K)\], the payload value.

Upstream: `heatCapacityConstant` (reads `Cp` from the case dictionary).

[`temperature_range`](Self::temperature_range) returns `(0, +inf)`: a
constant is not a fit to anything, so there is nothing to invalidate.
Appropriate for scoping calculations and for verification cases with an
analytical transient solution, not for a real material.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `MatproUo2`

UO2, MATPRO-v11 \[J/(kg K)\].

Upstream: `heatCapacityMatproUO2`. The standard three-term oxide form —
an Einstein (lattice-vibration) term, a linear
anharmonic/thermal-expansion term, and a Schottky-defect term that
switches on above roughly 2000 K:

```text
Cp = K1*theta^2*exp(theta/T) / (T*(exp(theta/T) - 1))^2
     + K2*T
     + (O/M / 2) * K3*ED / (R*T^2) * exp(-ED/(R*T))
```

with `K1 = 296.7` J/(kg K), `K2 = 2.43e-2` J/(kg K^2),
`K3 = 8.745e7` J/kg, Einstein temperature `theta = 535.285` K,
Schottky activation energy `ED = 1.577e5` J/mol, and `R` the molar gas
constant.

The Einstein term tends to `K1` as `T -> infinity`, so the whole
expression is asymptotically `K1 + K2*T` plus the (bounded) defect term
— a useful analytic handle, and one the unit tests use.

# Inputs used

[`temperature`](MaterialState::temperature) \[K\] and
[`oxygen_deviation`](MaterialState::oxygen_deviation) \[-\], the latter
entering as `O/M = 2 + oxygen_deviation`. Upstream instead reads a fixed
`OM` from the case dictionary (default 2.0), so a stoichiometric state
reproduces upstream exactly.

# Validity range

Upstream states none. This port uses **300–3113 K**, the upper bound
being the melting temperature of unirradiated UO2 as used by MATPRO.
Port's choice, not a bound from the report.

# Source

MATPRO-v11 UO2 specific heat, as implemented in upstream
`heatCapacityMatproUO2.C`.

###### `MatproMox`

MOX (U,Pu)O2, MATPRO-v11 \[J/(kg K)\].

Upstream: `heatCapacityMatproUPuO2`. The mass-weighted mean of the
MATPRO UO2 and PuO2 heat capacities, each in the same three-term form as
[`MatproUo2`](Self::MatproUo2):

```text
Cp = w_UO2 * Cp_UO2(T) + w_PuO2 * Cp_PuO2(T)
```

The PuO2 coefficients are `K1 = 347.4` J/(kg K), `K2 = 3.95e-4`
J/(kg K^2), `K3 = 3.86e7` J/kg, `theta = 571` K, `ED = 1.967e5` J/mol.

# Coefficient discrepancy in upstream

Upstream declares `K2_` twice with different values: `3.95e-4` in the
constructor initialiser list and `3.95e4` as the dictionary
`lookupOrDefault` fallback — an eight-order-of-magnitude difference.
`3.95e-4` J/(kg K^2) is the MATPRO PuO2 value and is what upstream
actually uses unless a case supplies a `heatCapacity` sub-dictionary, so
that is what this port uses.

# Inputs used

[`temperature`](MaterialState::temperature) \[K\],
[`pu_fraction`](MaterialState::pu_fraction) \[-\] as the PuO2 weight
fraction (upstream reads it from an `isotopes/Pu/ratioOverMetal` entry
in the case dictionary — the same quantity to within the PuO2/UO2 molar
mass ratio of about 1.004), and
[`oxygen_deviation`](MaterialState::oxygen_deviation) \[-\] as
`O/M = 2 + oxygen_deviation`.

At `pu_fraction = 0` this reduces **exactly** to
[`MatproUo2`](Self::MatproUo2), which the unit tests check.

# Validity range

Upstream states none; this port uses **300–3113 K**. Port's choice.

# Source

MATPRO-v11 (U,Pu)O2 specific heat, as implemented in upstream
`heatCapacityMatproUPuO2.C`.

###### `FinkMox`

MOX (U,Pu)O2, Fink correlation \[J/(kg K)\].

Upstream: `heatCapacityFinkUPuO2`. An Einstein term plus a linear term,
with the defect term disabled by upstream's default `C3 = 0`:

```text
Cp = C1*(theta/T)^2*exp(theta/T)/(exp(theta/T) - 1)^2
     + 2*C2*T
     + C3*Ea*exp(-Ea/T)/T^2
```

with `C1 = 322.49` J/(kg K), `C2 = 1.4679e-2` J/(kg K^2), `C3 = 0`,
`theta = 587.41` K, `Ea = 18531.7` K. Note that the Einstein term is
written here in the algebraically equivalent `(theta/T)^2` form rather
than MATPRO's `theta^2/T^2` form, and that the linear term carries an
explicit factor of two — both as upstream has them.

Because `C3 = 0`, this correlation has **no Schottky-defect upturn** and
so falls increasingly below [`MatproMox`](Self::MatproMox) above about
2000 K. That is a property of the fit, not of the port.

# Inputs used

[`temperature`](MaterialState::temperature) \[K\] only.

# Validity range

Upstream states none; this port uses **300–3000 K**. Port's choice.

# Source

J. K. Fink's (U,Pu)O2 heat capacity,
<https://info.ornl.gov/sites/publications/Files/Pub57523.pdf>, as cited
by upstream `heatCapacityFinkUPuO2.H`.

###### `MatproZircaloy`

Zircaloy cladding, MATPRO piecewise-linear table \[J/(kg K)\].

Upstream: `heatCapacityMatproZy`. A table of thirteen `(T, Cp)` anchor
points interpolated linearly, resolving the alpha-to-beta phase
transformation between 1090 K and 1248 K in 20 K steps. The peak is
816 J/(kg K) at 1173 K, more than double the 356 J/(kg K) plateau of the
beta phase above 1248 K.

# Known upstream bug, reproduced

The branch covering 1093 < T <= 1113 K interpolates from `Tlow = 1090`
rather than `Tlow = 1093`, while its `CpLow = 502` is the value belonging
to 1093 K. The result is a spurious upward step of about
11.5 J/(kg K) at exactly 1093 K, where the table is otherwise
continuous. This port reproduces it so results stay comparable with
upstream; the unit test
`matpro_zircaloy_has_the_upstream_discontinuity_at_1093_k` characterises
it, and it should be reported upstream rather than silently patched
here.

# Inputs used

[`temperature`](MaterialState::temperature) \[K\] only.

# Validity range

**273–2099 K**, stated by upstream (which notes it extended the
literature lower bound of 300 K down to 273 K to allow simulation below
290 K, so the 273–300 K stretch is itself a linear extrapolation of the
first table interval).

# Source

MATPRO Zircaloy specific heat,
<https://www.nrc.gov/docs/ML1429/ML14296A063.pdf> page 60, as cited by
upstream `heatCapacityMatproZy.H`.

###### `IaeaZircaloy`

Zircaloy cladding, IAEA correlation \[J/(kg K)\].

Upstream: `heatCapacityIAEAZy`. A smooth low-temperature line and a
high-temperature parabola, with a Gaussian phase-transformation peak
added over the transition window:

```text
Cp1 = 255.66 + 0.1024*T
Cp2 = 597.1 - 0.4088*T + 1.565e-4*T^2
f   = 1058.4 * exp(-(T - 1213.8)^2 / 719.61)

Cp  = Cp1       for T <  1100 K
    = Cp1 + f   for 1100 <= T < 1213.8 K
    = Cp2 + f   for 1213.8 <= T < 1320 K
    = Cp2       for T >= 1320 K
```

The Gaussian is negligible (about 1.6e-5 J/(kg K)) at both 1100 K and
1320 K, so those two branch changes are continuous to round-off. The
switch from `Cp1` to `Cp2` at 1213.8 K is **not**: the two differ by
about 48 J/(kg K) there, on top of a peak of 1058 J/(kg K), a step of
roughly 3.4%. That is a property of the published piecewise fit; see the
unit tests.

# Inputs used

[`temperature`](MaterialState::temperature) \[K\] only.

# Validity range

**273–2000 K**, stated by upstream.

# Source

IAEA-TECDOC-1496,
<https://www-pub.iaea.org/MTCD/publications/PDF/te_1496_web.pdf>, as
cited by upstream `heatCapacityIAEAZy.H`.

###### `Banerjee1515Ti`

15-15 Ti austenitic stainless cladding, Banerjee (2007)
\[J/(kg K)\].

Upstream: `heatCapacityBanerjee1515Ti`.

```text
Cp = 431 + 0.177*T + 8.72e-5*T^-2
```

# Port note on the third term

Upstream writes the last term as `par3 * pow(Ti, -2)` with
`par3 = 8.72e-5`, which makes it utterly negligible — of order
1e-10 J/(kg K) at any temperature of interest, i.e. the correlation is
effectively the straight line `431 + 0.177*T`. A `+8.72e-5*T^2` term
(positive exponent) would instead contribute about 87 J/(kg K) at
1000 K, which is the sort of magnitude an inverse-square Debye
correction normally has in these fits. This port **reproduces upstream's
negative exponent verbatim** because it cannot check Banerjee (2007)
offline, and flags the ambiguity here rather than guessing. Treat the
third term as unverified.

# Inputs used

[`temperature`](MaterialState::temperature) \[K\] only.

# Validity range

**293–1273 K**, stated by upstream.

# Source

Banerjee et al. (2007) 15-15 Ti heat capacity, as cited by upstream
`heatCapacityBanerjee1515Ti.H` (no DOI given upstream).

###### `Molybdenum`

Molybdenum \[J/(kg K)\].

Upstream: `heatCapacityMolybdenum`.

```text
Cp = 9.74e-6*T^2 + 5.37e-2*T + 235
```

# Inputs used

[`temperature`](MaterialState::temperature) \[K\] only.

# Validity range

Upstream states none; this port uses **300–2800 K**, below
molybdenum's melting point of about 2896 K. Port's choice.

# Source

Upstream `heatCapacityMolybdenum.C`; upstream's header `Description`
block is empty and cites no report.

###### `SneadSiC`

Silicon carbide, Snead et al. (2007) \[J/(kg K)\].

Upstream: `heatCapacitySneadSiC`.

```text
Cp = 925.65 + 0.3772*T - 7.9259e-5*T^2 - 3.1946e7*T^-2
```

The large negative `T^-2` term is what pulls the curve down at low
temperature — it dominates below about 400 K and is negligible above
1000 K.

# Coefficient discrepancy in upstream

As with [`MatproMox`](Self::MatproMox), upstream declares the last
coefficient twice with different values: `-3.1946e7` in the constructor
initialiser list and `-3.19446e7` as the dictionary fallback. This port
uses `-3.1946e7`, which is both the value upstream uses by default and
the value published by Snead et al.

# Inputs used

[`temperature`](MaterialState::temperature) \[K\] only.

# Validity range

**200–2400 K**, stated by upstream.

# Source

L. L. Snead et al., *Handbook of SiC properties for fuel performance
modeling*, Journal of Nuclear Materials (2007), as cited by upstream
`heatCapacitySneadSiC.H`.

##### Implementations

###### Methods

- ```rust
  pub const fn name(self: &Self) -> &'static str { /* ... */ }
  ```
  Short human-readable name of the correlation, for error messages and

- ```rust
  pub const fn temperature_range(self: &Self) -> (f64, f64) { /* ... */ }
  ```
  Temperature validity window `(low, high)` \[K\] of this correlation.

- ```rust
  pub fn value(self: &Self, state: &MaterialState) -> f64 { /* ... */ }
  ```
  Specific heat capacity \[J/(kg K)\], **clamping** the temperature into

- ```rust
  pub fn value_checked(self: &Self, state: &MaterialState) -> Result<f64> { /* ... */ }
  ```
  Specific heat capacity \[J/(kg K)\], **reporting** an out-of-range

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
    fn clone(self: &Self) -> HeatCapacityModel { /* ... */ }
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
    fn eq(self: &Self, other: &HeatCapacityModel) -> bool { /* ... */ }
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
## Module `poisson_ratio`

Poisson's ratio correlations \[-\].

# What this module computes

Poisson's ratio `nu` — the negative ratio of transverse to axial strain
under uniaxial load — for fuel, cladding and structural materials, as a pure
function of the local [`MaterialState`]. The quantity is **dimensionless**;
every value returned here is a bare number, not a percentage.

Most of the upstream models are simply the constant a given material is
conventionally assigned (0.316 for UO2, 0.276 for MOX, 0.3 for Zircaloy);
only two vary with state, and one of those does so by dividing a Young's
modulus by a shear modulus.

# Why the mechanics solve needs it

With Young's modulus `E` from
[`young_modulus`](crate::materials::properties::young_modulus), Poisson's
ratio gives the two Lame parameters an isotropic-elasticity momentum
equation consumes:

$$ \mu = \frac{E}{2(1 + \nu)} $$

$$ \lambda = \frac{E \nu}{(1 + \nu)(1 - 2\nu)} $$

**This module does not build them and does not solve anything** — that is
[`crate::mechanics`]. What lives here is only the property lookup.

The `1 - 2*nu` in `lambda` is the reason this module takes admissibility
seriously. As `nu` approaches 0.5 the material becomes incompressible and
`lambda` diverges; at `nu = 0.5` exactly it is a division by zero, and above
0.5 `lambda` changes sign and the elasticity tensor stops being positive
definite.

# Thermodynamic admissibility

For an isotropic linear-elastic solid, positive-definiteness of the strain
energy requires

$$ -1 < \nu < 0.5 $$

This is a real physical constraint, not a modelling convention: `nu <= -1`
makes the bulk modulus negative and `nu >= 0.5` makes it infinite or
negative. The unit tests below check it for every variant across its valid
range, and **one variant fails it in part of that range** — see
[`MatproZircaloy`](PoissonRatioModel::MatproZircaloy). That failure is
reported here rather than papered over, because it is a genuine property of
the upstream correlation pair. Use
[`is_admissible`](PoissonRatioModel::is_admissible) to test a result.

# Validation case

[`MatproZircaloy`](PoissonRatioModel::MatproZircaloy) is tracked as a
**formal validation case** (bead `op-6sl.7`). The write-up lives in the
repository at `docs/validation/poisson_ratio_zircaloy.md`, with the source
provenance in `docs/validation/References.md`. (Both are repo-only —
`docs/` is excluded from the packaged crate.)

In summary, as of 2026-08-05:

- **Admissibility check against the `-1 < nu < 0.5` constraint: FAILS**, for
  the reasons documented on the variant below. This needs no experimental
  data, which is why it is the one criterion that could be evaluated.
- **Comparison against measured Zircaloy Poisson's ratio: NOT PERFORMED.**
  Candidate benchmark datasets are identified with full provenance in
  `References.md` (chiefly Schwenk & Wheeler 1978, a direct `nu` measurement
  on Zircaloy-4 over 297-589 K, and Northwood, London & Bahen 1975 for
  Zircaloy-2 over 293-773 K), but **no measured value has been obtained or
  transcribed**, so no accuracy claim is made here or anywhere in this
  crate. Gaps are listed explicitly rather than filled with estimates.

Nothing in this module has been validated against experiment. Treat the
numbers as a faithful reproduction of a published correlation, not as a
statement about Zircaloy.

# Validity ranges, clamping and checking

Same contract as the companion Young's-modulus module:
[`value`](PoissonRatioModel::value) clamps out-of-range temperatures to the
endpoints and always returns a number;
[`value_checked`](PoissonRatioModel::value_checked) returns
[`OffbeatError::OutOfRange`] instead of extrapolating.

# Known divergences from upstream

1. **Isotropic cracking is not implemented here.** Upstream's UO2 and MOX
   Poisson models optionally rescale `nu` by a crack factor driven by a
   `nCracks` field. That is damage-model state, not a pure function of
   [`MaterialState`]. All variants return the uncracked value.
2. **`MatproZircaloy` composes its own Young's modulus.** Upstream looks the
   Young's-modulus *field* `E` up on the mesh registry, whatever model
   produced it, and divides it by the MATPRO shear modulus. This port pairs
   the MATPRO shear modulus with the MATPRO Young's modulus
   ([`YoungModulusModel::MatproZircaloy`]) — the internally consistent
   combination MATPRO itself defines. Mixing a MATPRO shear modulus with,
   say, a constant Young's modulus (which upstream permits) is not
   reproducible here, and should not be wanted.
3. **Fast fluence is in n/m^2.** Upstream's Young's-modulus Zircaloy model
   scales the fluence field by `1e4` and its Poisson counterpart does not —
   an internal inconsistency. This port uses
   [`MaterialState::fast_fluence`] in n/m^2 in both. In the alpha phase the
   choice is immaterial to `nu` anyway: the fluence factor cancels exactly
   between `E` and `G` (there is a test for this).
4. **`ConstantMolybdenum` returns 0.31.** Upstream's
   `constantPoissonRatioMolybdenum` initialises the member to `0.31` but
   declares the *dictionary* default as `0.316`, so a case with no
   `PoissonRatio` sub-dictionary gets 0.31 and one with an empty
   sub-dictionary gets 0.316. This port takes the no-dictionary value, 0.31.

[`MaterialState::fast_fluence`]: crate::materials::MaterialState::fast_fluence
[`YoungModulusModel::MatproZircaloy`]: crate::materials::properties::young_modulus::YoungModulusModel::MatproZircaloy

```rust
pub mod poisson_ratio { /* ... */ }
```

### Types

#### Enum `PoissonRatioModel`

Poisson's ratio `nu` \[-\] of a fuel, cladding or structural material.

# What it is

The negative ratio of transverse to axial strain in uniaxial loading: how
much a bar thins when you stretch it. Dimensionless, and for a
thermodynamically admissible isotropic solid strictly between
[`POISSON_RATIO_MIN`] and [`POISSON_RATIO_MAX`].

# Dispatch

An enum, not a trait object — see the workspace `CLAUDE.md` "No trait
objects" rule and the [module documentation](crate::materials).

# Example

```
use outram_park_fork_offbeat::materials::MaterialState;
use outram_park_fork_offbeat::materials::properties::poisson_ratio::PoissonRatioModel;

let state = MaterialState::fresh(900.0);
assert_eq!(PoissonRatioModel::MatproUo2.value(&state), 0.316);
assert!(PoissonRatioModel::MatproUo2.is_admissible(&state));
```

```rust
pub enum PoissonRatioModel {
    Constant(f64),
    MatproUo2,
    MatproMox,
    ConstantZircaloy,
    ConstantMolybdenum,
    MatproZircaloy,
    Tobbe1515Ti,
}
```

##### Variants

###### `Constant`

A user-supplied constant Poisson's ratio \[-\], independent of state.

Upstream: `PoissonRatioConstant`, which reads `nu` from the material
dictionary.

**Valid range:** none in temperature. The payload should lie strictly
between [`POISSON_RATIO_MIN`] and [`POISSON_RATIO_MAX`];
[`value_checked`](Self::value_checked) reports it as
[`OffbeatError::Unphysical`] otherwise.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `MatproUo2`

UO2 fuel: the constant 0.316 from MATPRO-11.

Upstream: `constantPoissonRatioUO2`, whose hard-coded default is
`0.316`. The same number appears inside upstream's UO2
Young's-modulus model as the `nui` used by its crack-softening factor.

**Inputs used:** none — the value is a constant.

**Valid range:** 300 K to 3113 K (room temperature to the UO2 melting
point), **port-imposed**. The value itself does not vary with
temperature; the range records where the fuel model is meant to be used
and keeps the two fuel-property families consistent with each other.

**Source:** MATPRO-11 (Rev. 2), as transcribed in OFFBEAT.

###### `MatproMox`

(U,Pu)O2 MOX fuel: the constant 0.276 from MATPRO-11.

Upstream: `constantPoissonRatioUPuO2`, whose hard-coded default is
`0.276` — the same `nui` used inside upstream's MOX Young's-modulus
models.

**Inputs used:** none — the value is a constant.

**Valid range:** 300 K to 3023 K (room temperature to the approximate
MOX melting point), **port-imposed**, for the same reason as
[`MatproUo2`](Self::MatproUo2).

**Source:** MATPRO-11 (Rev. 2), as transcribed in OFFBEAT.

###### `ConstantZircaloy`

Zircaloy cladding: the conventional constant 0.3.

Upstream: `constantPoissonRatioZy`, hard-coded default `0.3`. Use this
when a temperature-independent cladding Poisson's ratio is wanted;
[`MatproZircaloy`](Self::MatproZircaloy) is the state-dependent
alternative, and the two differ by more than 0.6 at the top of the
range.

**Inputs used:** none — the value is a constant.

**Valid range:** 290 K to 1800 K, **port-imposed** to match the MATPRO
Zircaloy models' stated range.

**Source:** the conventional value for Zircaloy, as transcribed in
OFFBEAT.

###### `ConstantMolybdenum`

Molybdenum structure: the constant 0.31.

Upstream: `constantPoissonRatioMolybdenum`. **Note the upstream
inconsistency:** the member initialiser is `0.31` while the dictionary
default it advertises is `0.316`, so upstream returns 0.31 for a case
with no `PoissonRatio` sub-dictionary and 0.316 for one with an empty
sub-dictionary. This port takes 0.31, the no-dictionary value. Use
[`Constant`](Self::Constant) if you specifically want 0.316.

**Inputs used:** none — the value is a constant.

**Valid range:** 300 K to 2896 K (room temperature to the melting point
of molybdenum), **port-imposed**.

**Source:** as transcribed in OFFBEAT.

###### `MatproZircaloy`

Zircaloy cladding, MATPRO-11: derived from the MATPRO Young's and shear
moduli.

Upstream: `PoissonRatioMatproZy`. Rather than tabulating `nu`, MATPRO
fits `E` and the shear modulus `G` independently and forms

```text
nu = E / (2 * G) - 1
```

with `G` from the same three-branch phase structure as the Young's
modulus — see [`matpro_zircaloy_shear_modulus`]:

```text
K1 = (7.07e11 - 2.315e8 * T) * C_ox        oxygen effect
K2 = -2.6e10 * C_cw                        cold-work effect
K3 = 0.88 + 0.12 * exp(-phi / 1e25)        fast-fluence effect

alpha (T < 1073 K):  G = (4.04e10 - 2.168e7 * T + K1 + K2) / K3
beta  (T >= 1273 K): G = 3.49e10 - 1.66e7 * T
1073 <= T < 1273 K:  linear interpolation, alpha value at 1073 K and
                     beta value at 1273 K
```

Note the sign difference from the Young's-modulus oxygen term:
`+par2*T` there, `-par2*T` here. That asymmetry is upstream's (and
MATPRO's) form, not a transcription slip.

# This variant can leave the admissible range

Because `E` and `G` are two *independently fitted* lines, their ratio is
not constrained to keep `nu < 0.5`, and it does not stay there. Two
independent failure regimes exist:

- **Temperature.** Unirradiated, uncold-worked cladding crosses
  `nu = 0.5` at **T = 1354.838709677 K** and reaches `nu = 0.912351` at
  the top of the range (1800 K). That leaves 445.16 K of the 1510 K
  validity interval — 29.5% of it — beyond the crossover. `nu = 0.5` is
  exactly the
  condition `E = 3G`; solving it on the beta lines gives
  `T = 1.26e10 / 9.3e6`, and bisection agrees to nine decimals.
- **Cold work,** and this one reaches down to ordinary operating
  temperature. `K2` subtracts the same absolute amount from both
  numerators, and since `G` is roughly a third of `E` the subtraction
  costs `G` proportionally three times as much, driving `nu` up. The
  threshold **falls as temperature rises**: **0.179096 at 300 K**,
  **0.119731 at 600 K**, and only **0.026131 at 1073 K**. Cold-worked
  stress-relief-annealed cladding retains cold work by definition, so
  2.6% at the top of the alpha branch is a routine condition, not a
  corner case.

Fast fluence cannot mitigate either regime: `K3` divides both `E` and
`G`, so it cancels exactly in `nu`. Oxygen content moves `nu` *down*,
away from the bound.

Neither regime is a port error — both are properties of the upstream
correlation pair, verified against the transcribed coefficients and
pinned down by the unit tests. Upstream neither detects nor guards
them; notably, upstream's own default Zircaloy material selects the
*constant* Poisson model rather than this one. Call
[`is_admissible`](Self::is_admissible) before handing the result to a
mechanics solve, or use [`ConstantZircaloy`](Self::ConstantZircaloy) in
the beta phase.

Every number above was printed by this crate's code and transcribed;
the full tables, the provenance and the gating-policy discussion are in
the repository at `docs/validation/poisson_ratio_zircaloy.md`.

**Inputs used:** [`temperature`](MaterialState::temperature),
[`fast_fluence`](MaterialState::fast_fluence) \[n/m^2\],
[`cold_work`](MaterialState::cold_work),
[`oxygen_content`](MaterialState::oxygen_content) \[weight fraction\].

**Valid range:** 290 K to 1800 K — **stated by upstream**, which warns
outside it.

**Source:** MATPRO-11 (Rev. 2), as transcribed in OFFBEAT.

###### `Tobbe1515Ti`

15-15 Ti austenitic stainless-steel cladding, Tobbe correlation (1975).

Upstream: `PoissonRatioTobbe1515Ti`.

```text
nu = 0.277 + 6e-5 * T_C
```

with `T_C` the temperature in **degrees Celsius** (`T - 273.15`). The
only variant here that rises smoothly with temperature, and it stays
comfortably admissible: at the top of its range (1273 K, i.e. 999.85 C)
it reaches 0.337.

**Inputs used:** [`temperature`](MaterialState::temperature).

**Valid range:** 293 K to 1273 K, **port-imposed** — upstream's Poisson
model performs no check, but its Young's-modulus counterpart
(`YoungModulusTobbe1515Ti`) warns outside exactly this range, and the
two come from the same 1975 correlation set.

**Source:** Tobbe (1975), as named in upstream's
`PoissonRatioTobbe1515Ti.H`.

##### Implementations

###### Methods

- ```rust
  pub fn name(self: &Self) -> &'static str { /* ... */ }
  ```
  Human-readable name of the correlation, used in error messages.

- ```rust
  pub fn temperature_range(self: &Self) -> (f64, f64) { /* ... */ }
  ```
  Temperature validity range `(low, high)` \[K\] of this correlation.

- ```rust
  pub fn value(self: &Self, state: &MaterialState) -> f64 { /* ... */ }
  ```
  Poisson's ratio \[-\] at the given state, **clamping** an out-of-range

- ```rust
  pub fn value_checked(self: &Self, state: &MaterialState) -> Result<f64> { /* ... */ }
  ```
  Poisson's ratio \[-\] at the given state, or

- ```rust
  pub fn is_admissible(self: &Self, state: &MaterialState) -> bool { /* ... */ }
  ```
  Whether [`value`](Self::value) at this state is thermodynamically

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
    fn clone(self: &Self) -> PoissonRatioModel { /* ... */ }
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
    fn eq(self: &Self, other: &PoissonRatioModel) -> bool { /* ... */ }
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

#### Function `matpro_zircaloy_shear_modulus`

**Attributes:**

- `MustUse { reason: None }`

MATPRO-11 shear modulus `G` \[Pa\] of Zircaloy at temperature `t` \[K\].

# What it is

The elastic resistance to shear — the companion fit to
[`YoungModulusModel::MatproZircaloy`]. MATPRO fits `E` and `G` separately
and derives Poisson's ratio from them as `nu = E/(2G) - 1`, which is what
[`PoissonRatioModel::MatproZircaloy`] does.

Exposed publicly because a mechanics layer that wants the shear modulus
should not have to rebuild it from `E` and `nu`: that round trip loses
precision and hides which quantity was actually fitted.

# Structure

Alpha phase below 1073 K with oxygen, cold-work and fast-fluence
corrections; beta phase at and above 1273 K as a bare line in temperature;
linear interpolation between, with the alpha endpoint taken at 1073 K and
the beta endpoint at 1273 K.

# Inputs

- `t` — temperature \[K\]. Valid 290 K to 1800 K (upstream's stated range).
  **Not clamped here**: callers are expected to have clamped or checked
  already, as [`PoissonRatioModel::value`] does.
- `state` — supplies [`fast_fluence`](MaterialState::fast_fluence)
  \[n/m^2\], [`cold_work`](MaterialState::cold_work) \[-\] and
  [`oxygen_content`](MaterialState::oxygen_content) \[weight fraction\].

# Source

MATPRO-11 (Rev. 2), as transcribed in upstream's `PoissonRatioMatproZy.C`.

# Example

```
use outram_park_fork_offbeat::materials::MaterialState;
use outram_park_fork_offbeat::materials::properties::poisson_ratio::matpro_zircaloy_shear_modulus;

// Unirradiated Zircaloy at 300 K: G = 4.04e10 - 2.168e7 * 300 Pa.
let g = matpro_zircaloy_shear_modulus(300.0, &MaterialState::fresh(300.0));
assert!((g - 3.3896e10).abs() < 1.0);
```

[`YoungModulusModel::MatproZircaloy`]: crate::materials::properties::young_modulus::YoungModulusModel::MatproZircaloy

```rust
pub fn matpro_zircaloy_shear_modulus(t: f64, state: &crate::materials::MaterialState) -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `POISSON_RATIO_MIN`

Lower bound \[-\] of the thermodynamically admissible range of Poisson's
ratio for an isotropic linear-elastic solid, exclusive.

At `nu = -1` the elasticity tensor stops being positive definite: the bulk
modulus `E/(3(1-2nu))` and the shear modulus can no longer both be
positive.

```rust
pub const POISSON_RATIO_MIN: f64 = -1.0;
```

#### Constant `POISSON_RATIO_MAX`

Upper bound \[-\] of the thermodynamically admissible range of Poisson's
ratio for an isotropic linear-elastic solid, exclusive.

At `nu = 0.5` the material is incompressible: the bulk modulus and the Lame
parameter `lambda = E*nu/((1+nu)(1-2nu))` both diverge. Above 0.5 `lambda`
changes sign.

```rust
pub const POISSON_RATIO_MAX: f64 = 0.5;
```

## Module `thermal_expansion`

Thermal expansion correlations — **strain** \[-\] and **coefficient**
\[1/K\].

# Read this before using the module: strain is not the coefficient

Two different quantities are called "thermal expansion" in the
fuel-performance literature, they differ by three or more orders of
magnitude, and interchanging them is *the* classic error in this corner of
the physics. This module exposes both, separately and by name:

| Method | Symbol | Unit | Meaning |
|---|---|---|---|
| [`strain`](ThermalExpansionModel::strain) | `eps_th` | \[-\] | the dimensionless linear thermal strain `dL/L0`, i.e. how much longer the material *is* at the current temperature than at the correlation's reference temperature |
| [`coefficient`](ThermalExpansionModel::coefficient) | `alpha` | \[1/K\] | the **instantaneous** coefficient of linear thermal expansion, `d(eps_th)/dT` — how fast the strain is *changing* with temperature right now |

A typical oxide fuel at 1000 K has `eps_th` of order `1e-2` and `alpha` of
order `1e-5 1/K`. If a stress calculation is silently a thousand times too
large or too small, this is the first thing to check.

Two further traps this module makes explicit:

- **`alpha` is not `eps_th / (T - Tref)`.** That ratio is the *mean*
  coefficient over the interval, which equals the instantaneous coefficient
  only for a strictly linear fit. Several correlations here
  ([`SneadSiC`](ThermalExpansionModel::SneadSiC),
  [`MartinUPuO2`](ThermalExpansionModel::MartinUPuO2)) are published as
  *mean* coefficients and are converted internally; the mean form never
  escapes this module.
- **Every correlation has its own reference temperature.** Some take it as
  a parameter (the stress-free temperature of the case, upstream's `Tref`);
  some have it baked into the fit and it cannot be changed. Each variant
  documents which, and
  [`reference_temperature`](ThermalExpansionModel::reference_temperature)
  reports it at runtime. Subtracting the strain of one correlation from
  that of another with a different reference is meaningless.

# Anisotropy

All but one of the ported correlations are isotropic. Pyrolytic carbon in a
TRISO coating is not: [`PARFUMEPyC`](ThermalExpansionModel::PARFUMEPyC)
expands differently along the radius than tangentially, controlled by the
Bacon anisotropy factor. [`strain`](ThermalExpansionModel::strain) returns
the isotropic-equivalent linear strain (the mean of the three principal
components, i.e. one third of the volumetric strain);
[`principal_strains`](ThermalExpansionModel::principal_strains) returns the
three components `[radial, tangential, tangential]` for callers that need
the tensor. For every isotropic variant the three are equal.

# Validity ranges, clamping and honesty about what upstream states

Where the upstream OFFBEAT source states or encodes a validity range — a
warning, a hard cut-off, a documented composition window — this port
enforces exactly that range. Where upstream states **no** range, this port
enforces **none** and says so, rather than inventing a plausible-looking
bound. [`validity_range`](ThermalExpansionModel::validity_range) returns
`(0.0, f64::INFINITY)` in that case, and the doc comment on the variant says
that the caller carries the extrapolation risk.

The plain [`strain`](ThermalExpansionModel::strain) /
[`coefficient`](ThermalExpansionModel::coefficient) methods **clamp** the
temperature to the enforced range endpoints before evaluating; the
`*_checked` variants return
[`OffbeatError::OutOfRange`]
instead. Note that clamping is a *deviation* from upstream for the two
variants that do have a range: upstream
[`MatproZy`](ThermalExpansionModel::MatproZy) prints a warning and then
extrapolates anyway. Clamping was chosen because an extrapolated fit
feeding a mechanics solve produces a plausible, wrong answer with no trace
in the log.

# Units

Raw `f64` in strict SI, per the crate-level units policy: temperature in
kelvin, strain dimensionless, coefficient in `1/K`. Correlations published
in degrees Celsius convert internally and never expose °C.

```rust
pub mod thermal_expansion { /* ... */ }
```

### Types

#### Enum `ThermalExpansionModel`

A published correlation for the thermal expansion of a fuel, cladding or
structural material.

Evaluate it with [`strain`](Self::strain) for the dimensionless thermal
strain `eps_th = dL/L0` \[-\] or [`coefficient`](Self::coefficient) for the
instantaneous linear expansion coefficient `alpha = d(eps_th)/dT` \[1/K\].
Read the [module documentation](self) first — the two are not
interchangeable.

# Choosing a variant

The variant names the **author or data source of the fit** and the material,
as the fuel-performance literature does. Two correlations for "MOX thermal
expansion" can differ by several per cent in strain, which is several
hundred MPa of cladding stress after gap closure, so the provenance is part
of the model, not a footnote.

| Variant | Material | Reference temperature |
|---|---|---|
| [`Constant`](Self::Constant) | any | caller-supplied |
| [`RelapUO2`](Self::RelapUO2) | UO2 | caller-supplied |
| [`MatproUPuO2`](Self::MatproUPuO2) | (U,Pu)O2 | fixed by the fit (~341 K) |
| [`MartinUPuO2`](Self::MartinUPuO2) | (U,Pu)O2 | caller-supplied |
| [`LemehovUPuO2`](Self::LemehovUPuO2) | (U,Pu)O2 | fixed by the fit |
| [`MAMOX`](Self::MAMOX) | minor-actinide MOX | caller-supplied |
| [`MatproZy`](Self::MatproZy) | Zircaloy | caller-supplied |
| [`Gehr1515Ti`](Self::Gehr1515Ti) | 15-15Ti steel | 293.15 K, fixed |
| [`Molybdenum`](Self::Molybdenum) | Mo | 273.15 K, fixed |
| [`SneadSiC`](Self::SneadSiC) | SiC | caller-supplied stress-free T |
| [`SwindemanHastelloyN`](Self::SwindemanHastelloyN) | Hastelloy N | caller-supplied |
| [`PARFUMEBuffer`](Self::PARFUMEBuffer) | TRISO buffer | caller-supplied |
| [`PARFUMEPyC`](Self::PARFUMEPyC) | TRISO pyrolytic carbon | caller-supplied |
| [`PARFUMESiC`](Self::PARFUMESiC) | TRISO SiC | caller-supplied |

# Example

```
use outram_park_fork_offbeat::materials::MaterialState;
use outram_park_fork_offbeat::materials::properties::thermal_expansion::
    ThermalExpansionModel;

// Zircaloy cladding, stress-free at 300 K, now sitting at 600 K.
let model = ThermalExpansionModel::MatproZy { t_ref: 300.0 };
let state = MaterialState::fresh(600.0);

let eps = model.strain(&state);       // dimensionless, ~2e-3
let alpha = model.coefficient(&state); // 1/K, ~6.7e-6

assert!(eps > 1e-3 && eps < 3e-3);
assert!(alpha > 5e-6 && alpha < 8e-6);
// The strain is ~300x the coefficient here; they are different quantities.
assert!(eps / alpha > 100.0);
```

```rust
pub enum ThermalExpansionModel {
    Constant {
        alpha: f64,
        t_ref: f64,
    },
    RelapUO2 {
        t_ref: f64,
    },
    MatproUPuO2,
    MartinUPuO2 {
        t_ref: f64,
    },
    LemehovUPuO2,
    MAMOX {
        t_ref: f64,
    },
    MatproZy {
        t_ref: f64,
    },
    Gehr1515Ti,
    Molybdenum,
    SneadSiC {
        t_stress_free: f64,
    },
    SwindemanHastelloyN {
        t_ref: f64,
    },
    PARFUMEBuffer {
        t_ref: f64,
    },
    PARFUMEPyC {
        t_ref: f64,
        anisotropy_factor: f64,
    },
    PARFUMESiC {
        alpha: f64,
        t_ref: f64,
    },
}
```

##### Variants

###### `Constant`

Temperature-independent expansion coefficient — `eps_th = alpha *
(T - t_ref)`.

Upstream `thermalExpansionConstant`. Use when a case supplies a single
engineering `alpha` and no fit is wanted, or as the null model in a
sensitivity study.

# Validity

None. A user-supplied constant carries no fitted range, so
[`validity_range`](Self::validity_range) reports an unbounded range and
`*_checked` only rejects a non-positive temperature. The caller owns the
question of where this `alpha` is meaningful.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `alpha` | `f64` | Instantaneous linear expansion coefficient \[1/K\]. Physically<br>positive for essentially all solids; typically `1e-6` to `2e-5`. |
| `t_ref` | `f64` | Reference (stress-free) temperature \[K\] at which the strain is<br>zero. |

###### `RelapUO2`

UO2 fuel, RELAP-derived fit with a Frenkel-defect term.

`f(T) = K1*T - K2 + K3*exp(-E_D / (k*T))`, and `eps_th = f(T) -
f(t_ref)`, with `K1 = 9.8e-6 1/K`, `K2 = 2.61e-3`, `K3 = 3.16e-1`,
`E_D = 1.32e-19 J`, `k = 1.38e-23 J/K` (upstream's rounded Boltzmann
constant is retained so results match upstream exactly).

The exponential term is negligible below about 1500 K and turns upward
steeply above 2000 K, which is the physical signature of Frenkel-pair
formation in the oxygen sub-lattice near melting.

Upstream `thermalExpansionRelapUO2`.

# Validity

**Upstream states no validity range and this port enforces none.** The
fit describes solid UO2 and has no meaning above the melting point
(~3120 K), but no bound is imposed here because none could be sourced
from upstream. The caller carries the extrapolation risk.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `t_ref` | `f64` | Reference (stress-free) temperature \[K\]. |

###### `MatproUPuO2`

(U,Pu)O2 MOX fuel, MATPRO-v11 — cubic polynomials in °C for the PuO2 and
UO2 end members, blended by Pu **mass** fraction.

`eps_th = c_Pu * P_PuO2(T_C) + (1 - c_Pu) * P_UO2(T_C)` with
`T_C = T - 273.15` and `c_Pu = pu_fraction / 1.13` (see
[`PU_ATOM_TO_MASS_FRACTION`]).

# The reference temperature is baked into the fit

Unlike most variants here, upstream subtracts **nothing**: the
polynomial is returned as the strain directly, so the reference
temperature is wherever the polynomial happens to vanish — about
**341 K** for pure UO2, and composition-dependent in general.
[`reference_temperature`](Self::reference_temperature) therefore returns
`None` for this variant. Do not mix its strain with another
correlation's without accounting for that.

Upstream `thermalExpansionMatproUPuO2`. Note that upstream's dictionary
reader has a copy-paste defect (it reads `par5..par8` from the keys
`par1..par4`); this port hard-codes the intended MATPRO coefficients and
does not reproduce the defect.

# Inputs used from [`MaterialState`]

[`temperature`](MaterialState::temperature),
[`pu_fraction`](MaterialState::pu_fraction). Upstream additionally
discounts the Pu atom fraction by any Am and Np present; this port has
no minor-actinide fields in [`MaterialState`] and so assumes none, which
makes `pu_fraction` the Pu/(U+Pu) atom ratio exactly as documented on
the field.

# Validity

**Upstream states no validity range and this port enforces none.**

###### `MartinUPuO2`

(U,Pu)O2 MOX fuel, Martin (1988) review.

D. G. Martin, *"The thermal expansion of solid UO2 and (U,Pu) mixed
oxides — a review and recommendations"*, J. Nucl. Mater. 152 (1988),
[doi:10.1016/0022-3115(88)90315-7](https://doi.org/10.1016/0022-3115(88)90315-7).

Martin publishes a **mean** coefficient `alpha_m(T)` as a cubic in `T`,
with separate coefficient sets below and above 923 K, scaled by
`(1 + 3.98*(2 - O/M))` for hypostoichiometry. Upstream forms the strain
as `alpha_m(T)*T - alpha_m(t_ref)*t_ref`, and this port reproduces that
— including upstream's detail that the reference term always uses the
**low-temperature** coefficient set regardless of `t_ref`. The
instantaneous coefficient returned by [`coefficient`](Self::coefficient)
is the correct derivative `alpha_m(T) + T * d(alpha_m)/dT`, which is
**not** `alpha_m(T)`.

The two coefficient sets agree to six significant figures at the 923 K
branch point, so the strain is continuous there; the derivative is not.

# Inputs used from [`MaterialState`]

[`temperature`](MaterialState::temperature),
[`oxygen_deviation`](MaterialState::oxygen_deviation) (as
`2 - O/M = -oxygen_deviation`).

# Validity

**Upstream states no validity range and this port enforces none.**

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `t_ref` | `f64` | Reference (stress-free) temperature \[K\]. |

###### `LemehovUPuO2`

(U,Pu)O2 MOX fuel, Lemehov (2020) — strain as a cubic in the homologous
temperature `T/T_melt`, with a burnup-dependent melting point.

INSPYRE technical report WP7-D7.2 (2020),
<https://re.public.polimi.it/handle/11311/1172415>. The melting
temperature follows Magni et al.,
[doi:10.1016/j.jnucmat.2021.153312](https://doi.org/10.1016/j.jnucmat.2021.153312):
`T_m0 = 3147 - 364.85*c_Pu - 1014.15*(2 - O/M) - 329.5*c_Am` and
`T_m = 2964.94 + (T_m0 - 2964.94)*exp(-Bu/24.25)` with burnup in
GWd/tHM, which is numerically identical to
[`MaterialState::burnup`] in MWd/kgHM.

Then `eps_th = 0.01*(b0 + b1*r + b2*r^2 + b3*r^3) * (1 + 3.98*(2 - O/M))`
with `r = T/T_m`. The leading `0.01` converts upstream's per-cent fit.

# The reference temperature is baked into the fit

As with [`MatproUPuO2`](Self::MatproUPuO2), upstream subtracts nothing,
so the strain is zero wherever the cubic vanishes and
[`reference_temperature`](Self::reference_temperature) returns `None`.

# Inputs used from [`MaterialState`]

[`temperature`](MaterialState::temperature),
[`burnup`](MaterialState::burnup),
[`pu_fraction`](MaterialState::pu_fraction),
[`oxygen_deviation`](MaterialState::oxygen_deviation). The Am term of
the melting-point correlation is evaluated with zero americium, because
[`MaterialState`] carries no minor-actinide inventory; for MA-bearing
fuel this over-predicts `T_m` and therefore under-predicts the strain.

# Validity — stated upstream, and enforced

The upstream class description gives the composition window explicitly:
**O/M between 1.94 and 2.0** (i.e.
[`oxygen_deviation`](MaterialState::oxygen_deviation) in `[-0.06, 0.0]`)
and **Pu/HM below 60 %**. Both are checked by
[`strain_checked`](Self::strain_checked). No temperature range is
stated, so none is enforced.

###### `MAMOX`

Minor-actinide-bearing MOX (MA-MOX), isotropic.

J. Nucl. Mater. 469 (2016) 223-227,
[doi:10.1016/j.jnucmat.2015.11.048](https://doi.org/10.1016/j.jnucmat.2015.11.048).

A cubic in `T`, `P(T) = a0 + a1*T + a2*T^2 + a3*T^3`, where each `a_i`
is itself a quadratic response surface in the Pu content `c_Pu` and the
hypostoichiometry `x = 2 - O/M`. The strain is `P(T) - P(t_ref)`.

# Inputs used from [`MaterialState`]

[`temperature`](MaterialState::temperature),
[`pu_fraction`](MaterialState::pu_fraction) (used directly as the atom
fraction `c_Pu`, matching upstream's `ratioPuMetal` dictionary entry),
[`oxygen_deviation`](MaterialState::oxygen_deviation).

# Validity

The upstream class description says the fit is *"for Pu = 0.3"*, yet the
implementation retains the full `c_Pu` dependence of the response
surface. No numerical composition window and no temperature range are
stated, so **this port enforces none**; treat compositions far from
`pu_fraction = 0.3` with suspicion.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `t_ref` | `f64` | Reference (stress-free) temperature \[K\]. |

###### `MatproZy`

Zircaloy cladding, MATPRO-v11, with the alpha → beta phase transition.

Three regimes, all isotropic (upstream carries a separate axial fit but
has it commented out and assumes isotropy):

- `T < 1073 K` (alpha phase): `s(T) = 6.721e-6*T - 2.073e-3`
- `1073 <= T < 1273 K`: linear interpolation between the two branch
  values, giving a **negative** apparent coefficient of about
  `-1.1e-5 1/K` — the material genuinely contracts through the
  alpha → beta transformation
- `T >= 1273 K` (beta phase): `s(T) = 9.7e-6*T - 9.4e-3`

and `eps_th = s(T) - s(t_ref)`.

Upstream `thermalExpansionMatproZy`. (Upstream's class description says
"UO2 fuel"; the code is unambiguously Zircaloy, and the variant is named
for what the code does.)

# Validity — stated upstream, and enforced

**273 K to 1800 K.** Upstream warns outside `273 < T < 1800 K` (checking
`T < 272.9 || T > 1801` to absorb rounding) and notes that the
literature lower bound of 290 K was relaxed to 273 K so that contraction
below room temperature can be modelled. Upstream then extrapolates
anyway; this port clamps in [`strain`](Self::strain) and errors in
[`strain_checked`](Self::strain_checked).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `t_ref` | `f64` | Reference (stress-free) temperature \[K\]. |

###### `Gehr1515Ti`

15-15Ti austenitic stainless cladding, Gehr (1973).

`eps_th = -3.101e-4 + 1.545e-5*T_C + 2.75e-9*T_C^2` with
`T_C = T - 273.15`, referenced to **20 °C = 293.15 K** (the quadratic
vanishes there to within `1e-8`). The reference is fixed by the fit and
cannot be moved.

Upstream additionally forces the strain to exactly zero at or below
293 K, which this port reproduces; the strain is therefore discontinuous
in its derivative at that point.

Upstream `thermalExpansionGehr1515Ti`.

# Validity

Lower bound **293 K**, taken from upstream's own hard cut-off. Upstream
states no upper bound and this port enforces none.

###### `Molybdenum`

Molybdenum structural material.

`eps_th = (4.985e-6 + 6.667e-10*T) * (T - 273.15)` — a mean coefficient
linear in `T`, multiplied by the rise above 273.15 K, so the reference
temperature is fixed at **273.15 K**. The instantaneous coefficient is
`4.985e-6 + 6.667e-10*(2T - 273.15)`, not the bracketed term.

Upstream `thermalExpansionMolybdenum`.

# Validity

**Upstream states no validity range and this port enforces none.**

###### `SneadSiC`

Silicon carbide, Snead handbook fit converted to instantaneous form.

- L. L. Snead, T. Nozawa, Y. Katoh, T.-S. Byun, S. Kondo, D. A. Petti,
  *"Handbook of SiC properties for fuel performance modeling"*,
  J. Nucl. Mater. 371 (2007) 329-377.
- M. Niffenegger, K. Reichlin, *"The proper use of thermal expansion
  coefficients in finite element calculations"*, Nucl. Eng. Des. 243
  (2012) 356-359 — the mean → instantaneous conversion.
- B. P. Collin, J. Nucl. Mater. 451 (2014) 65-77 — use of the Snead fit
  for UN TRISO.

The **mean** coefficient is `alpha_m(T) = 1e-6*(-1.8276 + 0.0178*T -
1.5544e-5*T^2 + 4.5246e-9*T^3)` below 1273.15 K and a constant `5e-6
1/K` above (the two agree to 0.5 % at the branch). Niffenegger's
conversion to a strain referenced to the stress-free temperature `T_sf`
is

`eps_th = [alpha_m(T)*(T - T_r) - alpha_m(T_sf)*(T_sf - T_r)] /
[1 + alpha_m(T_sf)*(T_sf - T_r)]`

where `T_r = 298.15 K` is the reference of the **mean** coefficient
itself — a different thing from `T_sf`, and the reason this variant is
the easiest one in the module to get wrong.

Upstream `thermalExpansionSneadSiC`.

# Validity

**Upstream states no validity range and this port enforces none.** The
1273.15 K branch point is a change of functional form, not a validity
bound.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `t_stress_free` | `f64` | Stress-free temperature `T_sf` \[K\] of the case — upstream's<br>`Tref`, i.e. the temperature at which the component carries no<br>thermal strain. Not to be confused with the fit's own 298.15 K mean<br>reference. |

###### `SwindemanHastelloyN`

Hastelloy N, Swindeman correlation.

`f(T) = 1e-6*(0.005291*T_C^2 + 9.682*T_C + 107.8)` with
`T_C = T - 273.15`, and `eps_th = f(T) - f(t_ref)`. The mean coefficient
is about `1.35e-5 1/K` at 970 K, in the expected range for a
nickel-based alloy.

Upstream `thermalExpansionSwindemanHastelloyN`.

# Validity

**Upstream states no validity range and this port enforces none.**

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `t_ref` | `f64` | Reference (stress-free) temperature \[K\]. |

###### `PARFUMEBuffer`

TRISO buffer layer (porous pyrolytic carbon), PARFUME correlation.

`alpha(T) = 5e-6 * (1 + 0.11*(T_C - 400)/700)` with `T_C = T - 273.15`,
applied as a **mean** coefficient: `eps_th = alpha(T)*(T - t_ref)`. The
instantaneous coefficient adds the `(T - t_ref) * d(alpha)/dT` term.

Upstream `thermalExpansionPARFUMEBuffer`.

# Deviation from upstream

Upstream's numerical dead zone here tests `strain < 1e-7` rather than
`|strain| < 1e-7`, so it silently discards **all** contraction below the
reference temperature. This port applies the magnitude test used
everywhere else in the family, treating the unsigned comparison as an
upstream defect. A case cooling below `t_ref` will therefore differ from
upstream — deliberately.

# Validity

**Upstream states no validity range and this port enforces none.**

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `t_ref` | `f64` | Reference (stress-free) temperature \[K\]. |

###### `PARFUMEPyC`

TRISO pyrolytic carbon layer (IPyC/OPyC), PARFUME correlation —
**transversely isotropic**.

Deposited PyC has a preferred crystallite orientation measured by the
Bacon anisotropy factor (BAF). With `R_r = 2/(2 + BAF)` and
`R_t = (1 + BAF)/(2 + BAF)`, the mean coefficients are

- radial: `alpha_r = (30 - 37.5*R_r) * (1 + 0.11*(T - 673)/700) * 1e-6`
- tangential: `alpha_t = (36*(R_t - 1)^2 + 1) * (1 + 0.11*(T - 673)/700) * 1e-6`

and `eps_r = alpha_r*(T - t_ref)`, `eps_t = alpha_t*(T - t_ref)`.

At `BAF = 1` (isotropic as-fabricated PyC) both reduce to the same
`5e-6`-scaled expression — a useful self-check, and the reason
[`PARFUME_PYC_DEFAULT_ANISOTROPY`] is 1.0.

[`strain`](Self::strain) returns the isotropic-equivalent
`(eps_r + 2*eps_t)/3`; use
[`principal_strains`](Self::principal_strains) to get
`[eps_r, eps_t, eps_t]` separately. Upstream also offers a rotation of
the spherical-coordinate tensor into Cartesian components for
non-1D cases; that is a *mesh* operation and belongs with the mechanics
assembly, not with the correlation, so it is not ported here.

Upstream `thermalExpansionPARFUMEPyC`.

# Deviation from upstream

The same unsigned dead-zone defect described under
[`PARFUMEBuffer`](Self::PARFUMEBuffer) applies, and is likewise not
reproduced.

# Validity

**Upstream states no validity range and this port enforces none.**

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `t_ref` | `f64` | Reference (stress-free) temperature \[K\]. |
| `anisotropy_factor` | `f64` | As-fabricated Bacon anisotropy factor (BAF) \[-\]. `1.0` is<br>isotropic; deposited PyC in TRISO particles is typically 1.0-1.1.<br>Must be greater than `-2` for the orientation factors to be finite. |

###### `PARFUMESiC`

TRISO silicon-carbide layer, PARFUME constant coefficient.

`eps_th = alpha * (T - t_ref)` with upstream's default
`alpha = 4.9e-6 1/K` ([`PARFUME_SIC_ALPHA`]), quoted directly in the
upstream class description.

Kept distinct from [`Constant`](Self::Constant) despite the identical
algebra, because the provenance of the number is part of the model.

Upstream `thermalExpansionPARFUMESiC`.

# Validity

**Upstream states no validity range and this port enforces none.**

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `alpha` | `f64` | Instantaneous linear expansion coefficient \[1/K\]; upstream default<br>[`PARFUME_SIC_ALPHA`]. |
| `t_ref` | `f64` | Reference (stress-free) temperature \[K\]. |

##### Implementations

###### Methods

- ```rust
  pub fn name(self: &Self) -> &'static str { /* ... */ }
  ```
  Human-readable name of the correlation, used in error messages.

- ```rust
  pub fn validity_range(self: &Self) -> (f64, f64) { /* ... */ }
  ```
  Temperature range \[K\] over which this port *enforces* the correlation,

- ```rust
  pub fn reference_temperature(self: &Self) -> Option<f64> { /* ... */ }
  ```
  Reference temperature \[K\] at which this correlation's strain is zero,

- ```rust
  pub fn strain(self: &Self, state: &MaterialState) -> f64 { /* ... */ }
  ```
  Linear thermal strain `eps_th = dL/L0` \[**dimensionless**\].

- ```rust
  pub fn strain_checked(self: &Self, state: &MaterialState) -> Result<f64> { /* ... */ }
  ```
  Linear thermal strain \[-\], or [`OffbeatError`] if the correlation is

- ```rust
  pub fn principal_strains(self: &Self, state: &MaterialState) -> [f64; 3] { /* ... */ }
  ```
  The three principal linear thermal strains \[-\], as

- ```rust
  pub fn coefficient(self: &Self, state: &MaterialState) -> f64 { /* ... */ }
  ```
  **Instantaneous** coefficient of linear thermal expansion

- ```rust
  pub fn coefficient_checked(self: &Self, state: &MaterialState) -> Result<f64> { /* ... */ }
  ```
  Instantaneous linear expansion coefficient \[1/K\], or [`OffbeatError`]

- ```rust
  pub fn principal_coefficients(self: &Self, state: &MaterialState) -> [f64; 3] { /* ... */ }
  ```
  The three principal instantaneous expansion coefficients \[1/K\], as

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
    fn clone(self: &Self) -> ThermalExpansionModel { /* ... */ }
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
    fn eq(self: &Self, other: &ThermalExpansionModel) -> bool { /* ... */ }
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
### Constants and Statics

#### Constant `PARFUME_SIC_ALPHA`

Upstream default instantaneous linear expansion coefficient of SiC from the
PARFUME code \[1/K\]: `4.9e-6`, quoted directly in the upstream class
description of `thermalExpansionPARFUMESiC.H`.

```rust
pub const PARFUME_SIC_ALPHA: f64 = 4.9e-6;
```

#### Constant `PARFUME_PYC_DEFAULT_ANISOTROPY`

Upstream default as-fabricated Bacon anisotropy factor (BAF) of pyrolytic
carbon \[-\]: `1.0`, i.e. fully isotropic PyC.

See [`ThermalExpansionModel::PARFUMEPyC`] for what the factor does.

```rust
pub const PARFUME_PYC_DEFAULT_ANISOTROPY: f64 = 1.0;
```

#### Constant `PU_ATOM_TO_MASS_FRACTION`

Approximate conversion factor from Pu atom fraction of the heavy metal to
Pu **mass** fraction of the fuel \[-\]: `1.13`.

Upstream computes it as `(MM_HM + 2*MM_O) / MM_Pu` with `MM_Pu ~ 239`,
`MM_HM ~ 238.5` and `MM_O = 16` g/mol, and applies it as
`c_mass = c_atom / 1.13` in both the MATPRO and the Lemehov MOX
correlations. It is approximate by upstream's own admission.

```rust
pub const PU_ATOM_TO_MASS_FRACTION: f64 = 1.13;
```

## Module `young_modulus`

Young's modulus correlations \[Pa\].

# What this module computes

Young's modulus `E` — the elastic (uniaxial) stiffness of a fuel, cladding,
structural or TRISO-coating material — as a pure function of the local
[`MaterialState`]: temperature, porosity, plutonium content, deviation from
stoichiometry, fast-neutron fluence and cold work. The returned quantity is
**always in pascals (Pa)**, never GPa or MPa, even though most of the
published fits are quoted in GPa or MPa; the unit conversion is done inside
each variant.

# Why the mechanics solve needs it

`E` alone is not what an isotropic-elasticity momentum equation consumes.
Together with Poisson's ratio `nu` from
[`poisson_ratio`](crate::materials::properties::poisson_ratio) it forms the
two Lame parameters:

$$ \mu = \frac{E}{2(1 + \nu)} $$

$$ \lambda = \frac{E \nu}{(1 + \nu)(1 - 2\nu)} $$

`mu` is the shear modulus and `lambda` the first Lame parameter. **This
module does not build them and does not solve anything** — assembling the
Lame parameters and the momentum equation belongs to
[`crate::mechanics`]. What lives here is only the property lookup.

Note the `1 - 2*nu` in the denominator of `lambda`: it is why the companion
module cares whether a correlation can return `nu >= 0.5`, and why the
thermodynamic admissibility of `nu` is tested there rather than assumed.

# Units — raw `f64`, strict SI

Like [`MaterialState`], this module carries raw `f64` in strict SI rather
than `uom` quantities, because it is evaluated once per cell per property
per timestep inside the numerical loops. Inputs are kelvin, n/m^2 and
dimensionless fractions; the output is pascals. Correlations whose published
form is in degrees Celsius (the PARFUME set, Hofman D9, Tobbe 15-15 Ti,
Watrous Hastelloy N) convert internally — a caller never passes Celsius.

# Validity ranges, clamping and checking

Every variant declares a temperature range with
[`YoungModulusModel::temperature_range`]. Two evaluation entry points:

- [`value`](YoungModulusModel::value) **clamps** the inputs to the range
  endpoints and always returns a number. This is the one the solver loop
  calls, and it matches the spirit of upstream, which prints a warning and
  carries on.
- [`value_checked`](YoungModulusModel::value_checked) returns
  [`OffbeatError::OutOfRange`] instead of extrapolating. Use it when setting
  a case up, to learn that the correlation does not cover the conditions
  asked of it.

Some ranges are stated by upstream (it emits an explicit warning outside
them); the rest are **port-imposed** and are labelled as such on the variant
that owns them. A port-imposed bound is a convention of this crate, not a
number taken from the cited report.

# Known divergences from upstream

Recorded here rather than buried, because a port that silently "improves"
its source is not a port:

1. **Isotropic cracking is not implemented here.** Upstream's UO2, MOX and
   SCK-CEN variants optionally multiply `E` by a crack-softening factor
   driven by a `nCracks` field, a `sliceMapper` and the linear heat rate.
   That is damage-model state, not a pure function of [`MaterialState`], so
   it belongs with the damage model, not here. All variants below return the
   upstream **nominal** (uncracked) value.
2. **`WatrousHastelloyN` returns a finite modulus above 1273.15 K**, where
   upstream leaves the field at its initialised `0.0`. A zero Young's
   modulus makes the stiffness matrix singular; this port clamps to the
   range endpoint instead. See the variant docs.
3. **Fast fluence is in n/m^2 throughout.** Upstream's `MatproZy` variant
   multiplies the stored fluence field by `1e4` (i.e. reads n/cm^2) while
   its companion Poisson-ratio model does not — an internal inconsistency.
   This port takes [`MaterialState::fast_fluence`] in n/m^2 in both places.

[`MaterialState::fast_fluence`]: crate::materials::MaterialState::fast_fluence

```rust
pub mod young_modulus { /* ... */ }
```

### Types

#### Enum `YoungModulusModel`

Young's modulus `E` \[Pa\] of a fuel, cladding, structural or TRISO-coating
material.

# What it is

The elastic stiffness in uniaxial tension: the slope of the stress-strain
curve at zero strain. Every variant is one published correlation, named for
the **author or data source of the fit plus the material** — that is how the
fuel-performance literature identifies these, and two "UO2 Young's modulus"
correlations can differ by tens of percent.

# Dispatch

An enum, not a trait object: the set of correlations is closed and known at
compile time, so adding one is a compile error at every `match` site rather
than a runtime surprise, and go-to-definition works on the variants. See the
workspace `CLAUDE.md` "No trait objects" rule.

# Example

```
use outram_park_fork_offbeat::materials::MaterialState;
use outram_park_fork_offbeat::materials::properties::young_modulus::YoungModulusModel;

// Fully dense UO2 at room temperature, MATPRO-11 correlation.
let state = MaterialState::fresh(300.0);
let e = YoungModulusModel::MatproUo2.value(&state);
assert!((e - 2.25757317e11).abs() < 1.0e4); // ~225.8 GPa
```

```rust
pub enum YoungModulusModel {
    Constant(f64),
    MatproUo2,
    MatproMox,
    SckCenMox,
    MatproZircaloy,
    HofmanD9,
    BisonMolybdenum,
    Tobbe1515Ti,
    WatrousHastelloyN,
    SneadSiC {
        porosity_coefficient: f64,
    },
    ParfumeBuffer {
        density: f64,
    },
    ParfumePyC {
        density: f64,
        bacon_anisotropy_factor: f64,
        crystallite_diameter: f64,
    },
    ParfumeSiC,
}
```

##### Variants

###### `Constant`

A user-supplied constant Young's modulus \[Pa\], independent of state.

Upstream: `YoungModulusConstant`, which reads the value from the
material dictionary as `E`. Use it for a material this port has no
correlation for, or to isolate a mechanics test from property
variation.

**Valid range:** none — the value is returned unchanged at any
temperature. The payload should be positive; a non-positive modulus is
reported by [`value_checked`](Self::value_checked) as
[`OffbeatError::Unphysical`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `MatproUo2`

UO2 fuel, MATPRO-11 correlation.

Upstream: `YoungModulusMatproUO2`.

```text
E = 2.334e11 * (1 - 2.752 * P) * (1 - 1.0915e-4 * T)     [Pa]
```

with `P` the porosity \[-\] and `T` the temperature \[K\]. The first
factor is the porosity knock-down, the second the linear thermal
softening.

**Inputs used:** [`temperature`](MaterialState::temperature),
[`porosity`](MaterialState::porosity).

**Valid range:** temperature 300 K to 3113 K (room temperature to the
UO2 melting point) and porosity 0 to [`MATPRO_MAX_POROSITY`]. Both are
**port-imposed**: upstream performs no range check on this correlation.

**Source:** MATPRO-11 (Rev. 2), as transcribed in OFFBEAT.

###### `MatproMox`

(U,Pu)O2 MOX fuel, MATPRO-11 correlation with plutonium and
stoichiometry corrections.

Upstream: `YoungModulusMatproUPuO2`.

```text
E = E_UO2(T, P) * exp(-B * x) * (1 + 0.05 * c_Pu)        [Pa]
```

where `E_UO2(T, P)` is exactly [`MatproUo2`](Self::MatproUo2),
`x = 2 - O/M` is upstream's deviation-from-stoichiometry variable, and
`c_Pu` is the plutonium **mass** fraction of the fuel, obtained from the
atom fraction by the approximate conversion `c_Pu = at_Pu / 1.13` that
upstream derives for `MM_Pu ~ 239 g/mol`, `MM_HM ~ 238.5 g/mol`,
`O/M = 2`.

`B = 1.35` for `x >= 0` and `B = 1.75` for `x < 0`. Because this port
stores the deviation as the `x` of `(U,Pu)O_{2+x}`
([`oxygen_deviation`](MaterialState::oxygen_deviation)), which is the
**negative** of upstream's variable, hypostoichiometric fuel
(`O/M < 2`, `oxygen_deviation < 0`) takes `B = 1.35` and softens, which
is the normal fast-reactor MOX case.

**Upstream quirk, ported faithfully:** for hyperstoichiometric fuel
(`O/M > 2`) upstream's `exp(-B*x)` has a positive exponent and therefore
*stiffens* the fuel; and upstream's comments label the two branches
"hypostoichiometric"/"hyperstoichiometric" the opposite way round from
what its own algebra does. The algebra is reproduced here, not the
comments.

**Inputs used:** [`temperature`](MaterialState::temperature),
[`porosity`](MaterialState::porosity),
[`pu_fraction`](MaterialState::pu_fraction),
[`oxygen_deviation`](MaterialState::oxygen_deviation).

**Valid range:** temperature 300 K to 3023 K (room temperature to the
approximate MOX melting point), porosity 0 to [`MATPRO_MAX_POROSITY`].
Both **port-imposed**; upstream performs no range check.

**Source:** MATPRO-11 (Rev. 2), as transcribed in OFFBEAT.

###### `SckCenMox`

(U,Pu)O2 MOX fuel, SCK-CEN correlation developed for TRANSURANUS.

Upstream: `YoungModulusSckCenUPuO2`.

A mixture rule between the two end-member moduli, a stoichiometry
correction with two slopes, a quadratic temperature shape normalised at
273 K, and a porosity knock-down:

```text
E_mix = (1 - c_Pu) * 218.74 + c_Pu * 249.45              [GPa]
E_y   = E_mix - 586 * y                       for 0 <= y <= 0.037
E_y   = E_mix - 586 * 0.037 - 126.59*(y - 0.037)  for y > 0.037
E_y   = E_mix                                 for y < 0
f(T)  = 219.12 - 0.0154 * T - 9.0e-6 * T^2
E     = E_y * f(T)/f(273) * (1 - P)^2 / (1 + 1.1 * P) * 1e9   [Pa]
```

with `y = 2 - O/M` (again the negative of
[`oxygen_deviation`](MaterialState::oxygen_deviation)) and `P` the
porosity, clamped at 0.3 by upstream itself "to prevent YM decreasing
too much".

**Inputs used:** [`temperature`](MaterialState::temperature),
[`porosity`](MaterialState::porosity),
[`pu_fraction`](MaterialState::pu_fraction),
[`oxygen_deviation`](MaterialState::oxygen_deviation).

**Valid range:** temperature 273 K to 3023 K, **port-imposed** — the
correlation is normalised at 273 K and upstream checks nothing.

**Source:** INSPYRE project deliverable D7.2 (2020), SCK-CEN correlation
for TRANSURANUS; the URL is given in upstream's
`YoungModulusSckCenUPuO2.H`.

###### `MatproZircaloy`

Zircaloy cladding, MATPRO-11 correlation with alpha/beta phase branches.

Upstream: `YoungModulusMatproZy`.

Three temperature regimes, with oxygen, cold-work and fast-fluence
corrections in the alpha phase:

```text
K1 = (6.61e11 + 5.912e8 * T) * C_ox        oxygen effect
K2 = -2.6e10 * C_cw                        cold-work effect
K3 = 0.88 + 0.12 * exp(-phi / 1e25)        fast-fluence effect

alpha (T < 1073 K):  E = (1.088e11 - 5.475e7 * T + K1 + K2) / K3
beta  (T >= 1273 K): E = 9.21e10 - 4.05e7 * T
1073 <= T < 1273 K:  linear interpolation between the two, with the
                     alpha value taken at 1073 K and the beta value at
                     1273 K
```

Note the sign of the fluence term: `K3` **decreases** towards 0.88 with
accumulated fluence and divides the numerator, so irradiation
*stiffens* the cladding by up to a factor `1/0.88 = 1.136`. That is
irradiation hardening, and it is the correct direction for this fit.

**Inputs used:** [`temperature`](MaterialState::temperature),
[`fast_fluence`](MaterialState::fast_fluence) \[n/m^2\],
[`cold_work`](MaterialState::cold_work),
[`oxygen_content`](MaterialState::oxygen_content) \[weight fraction\].

**Valid range:** 290 K to 1800 K — **stated by upstream**, which emits
a warning outside it (with a one-degree slack on each side for rounding).

**Source:** MATPRO-11 (Rev. 2), as transcribed in OFFBEAT.

###### `HofmanD9`

D9 austenitic stainless-steel cladding, Hofman correlation.

Upstream: `YoungModulusHofmanD9`.

```text
E = (2.01e5 - 79.29 * T_C) * 1e6                         [Pa]
```

with `T_C` the temperature in **degrees Celsius** (`T - 273.15`); the
bracket is in MPa.

**Inputs used:** [`temperature`](MaterialState::temperature).

**Valid range:** 293 K to 1273 K — **stated by upstream**, which warns
outside it.

**Source:** Hofman correlation for D9, as transcribed in OFFBEAT.

###### `BisonMolybdenum`

Molybdenum structural material, correlation from the BISON manual.

Upstream: `YoungModulusMolybdenum`.

```text
E = 3.349e11 - 5.101e7 * T                               [Pa]
```

**Inputs used:** [`temperature`](MaterialState::temperature).

**Valid range:** 300 K to 2896 K (room temperature to the melting point
of molybdenum) — **port-imposed**; upstream checks nothing.

**Source:** BISON manual, as named in upstream's
`YoungModulusMolybdenum.H`.

###### `Tobbe1515Ti`

15-15 Ti austenitic stainless-steel cladding, Tobbe correlation (1975).

Upstream: `YoungModulusTobbe1515Ti`.

```text
E = (202.7 - 0.08167 * T_C) * 1e9                        [Pa]
```

with `T_C` in **degrees Celsius**; the bracket is in GPa.

**Inputs used:** [`temperature`](MaterialState::temperature).

**Valid range:** 293 K to 1273 K — **stated by upstream**, which warns
outside it.

**Source:** Tobbe (1975), as named in upstream's
`YoungModulusTobbe1515Ti.H`.

###### `WatrousHastelloyN`

Hastelloy N structural alloy, Watrous correlation.

Upstream: `YoungModulusWatrousHastelloyN`.

A cubic in degrees Celsius:

```text
E = (-9.944e-8 * T_C^3 + 1.178e-4 * T_C^2
     - 0.1033 * T_C + 220.9) * 1e9                       [Pa]
```

**Inputs used:** [`temperature`](MaterialState::temperature).

**Valid range:** upper bound 1273.15 K (1000 C) — **stated by
upstream**. The lower bound of 293.15 K is **port-imposed**.

**Divergence from upstream:** above 1273.15 K upstream warns and leaves
the modulus at its initialised value of **zero**, which would make the
mechanics stiffness matrix singular. This port clamps to the 1273.15 K
endpoint instead, and [`value_checked`](Self::value_checked) reports
[`OffbeatError::OutOfRange`] there.

**Source:** Watrous, as named in upstream's
`YoungModulusWatrousHastelloyN.H`.

###### `SneadSiC`

CVD silicon carbide, Snead et al. (2007) handbook correlation.

Upstream: `YoungModulusSneadSiC`.

```text
E = 460e9 * exp(-C * P) - 0.04e9 * T * exp(-962 / T)     [Pa]
```

The second term is the thermal softening (about -0.5 GPa at 300 K,
growing with temperature); the first is the room-temperature modulus of
fully dense CVD SiC with an exponential porosity knock-down.

**Inputs used:** [`temperature`](MaterialState::temperature),
[`porosity`](MaterialState::porosity).

**Valid range:** 300 K to 1873 K — **port-imposed**; upstream states no
range. Treat the bound as a convention of this crate, not a number taken
from Snead et al.

**Source:** L. L. Snead, T. Nozawa, Y. Katoh, T.-S. Byun, S. Kondo,
D. A. Petti, "Handbook of SiC properties for fuel performance
modeling", *Journal of Nuclear Materials* **371** (2007) 329-377.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `porosity_coefficient` | `f64` | Exponential porosity-knock-down coefficient `C` \[-\] in<br>`exp(-C * P)`.<br><br>Upstream's dictionary default is **0.0**, i.e. no porosity<br>dependence at all; pass 0.0 to reproduce upstream's default<br>behaviour exactly. Values around 3-4 are typical when an<br>exponential porosity correction is wanted for CVD SiC. |

###### `ParfumeBuffer`

TRISO buffer layer (porous pyrolytic carbon), PARFUME correlation.

Upstream: `YoungModulusPARFUMEBuffer`.

```text
E = 25.5 * (0.384 + 0.324e-3 * rho)
         * (1 + 0.23 * phi25)
         * (1 + 1.5e-4 * (T_C - 20)) * 1e9               [Pa]
```

with `rho` the buffer density \[kg/m^3\], `T_C` the temperature in
degrees Celsius, and `phi25` the fast fluence in units of `1e25` n/m^2
(E > 0.18 MeV), saturated at 3.96 — see
[`PARFUME_SATURATION_FLUENCE`].

The irradiation term *increases* the modulus: pyrolytic carbon
densifies and stiffens under fast-neutron damage.

**Inputs used:** [`temperature`](MaterialState::temperature),
[`fast_fluence`](MaterialState::fast_fluence); the density is carried on
the variant because [`MaterialState`] has no density field (upstream
looks up the `rho` mesh field).

**Valid range:** 293.15 K to 2273.15 K — **port-imposed**, covering
normal TRISO operation through accident temperatures; upstream states no
temperature range. Fluence is saturated rather than rejected, matching
upstream.

**Source:** PARFUME (INL TRISO fuel-performance code) material models,
as transcribed in OFFBEAT.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `density` | `f64` | As-fabricated buffer density \[kg/m^3\]. Typically about<br>1000 kg/m^3, i.e. roughly half the density of dense pyrolytic<br>carbon. |

###### `ParfumePyC`

TRISO IPyC/OPyC dense pyrolytic-carbon layer, PARFUME correlation.

Upstream: `YoungModulusPARFUMEPyC`.

Radial and tangential components, then the isotropic average upstream
actually returns:

```text
c   = 25.5 * (0.384 + 0.324e-3 * rho) * (2.985 - 0.0662 * Lc)
          * (1 + 0.23 * phi25) * (1 + 1.5e-4 * (T_C - 20))
E_r = c * (1.463 - 0.463 * BAF)
E_t = c * (0.481 + 0.519 * BAF)
E   = (E_r + 2 * E_t) / 3 * 1e9                          [Pa]
```

**Upstream's own note, kept:** PyC is properly transversely isotropic,
with different radial and tangential moduli. Upstream returns the
isotropic average as a temporary measure, observing that for the usual
TRISO defaults `BAF = 1.0` and `Lc = 30` the two components are
identical anyway (`1.463 - 0.463 = 0.481 + 0.519 = 1`). This port
reproduces that; a transversely isotropic PyC would need the mechanics
layer to accept a direction-dependent modulus.

**Inputs used:** [`temperature`](MaterialState::temperature),
[`fast_fluence`](MaterialState::fast_fluence); density, anisotropy and
crystallite size are carried on the variant because [`MaterialState`]
has no field for them.

**Valid range:** temperature 293.15 K to 2273.15 K (**port-imposed**);
density 1800 to 2000 kg/m^3 and `BAF >= 1.0` (**stated by upstream**,
which warns outside them). Fluence saturates at
[`PARFUME_SATURATION_FLUENCE`].

**Source:** PARFUME (INL TRISO fuel-performance code) material models,
as transcribed in OFFBEAT.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `density` | `f64` | As-fabricated PyC density \[kg/m^3\]. The fit's data range is<br>1800-2000 kg/m^3. |
| `bacon_anisotropy_factor` | `f64` | As-fabricated Bacon Anisotropy Factor (BAF) \[-\], a measure of<br>preferred crystallite orientation. `1.0` is fully isotropic; the fit<br>requires `BAF >= 1.0`. |
| `crystallite_diameter` | `f64` | Crystallite diameter `Lc` \[nm\]. The usual TRISO default is 30.<br><br>Note that the factor `2.985 - 0.0662 * Lc` reaches zero at<br>`Lc = 45.1` nm and turns negative beyond, so this parameter is not<br>meaningfully extrapolable. |

###### `ParfumeSiC`

TRISO SiC layer, PARFUME piecewise-linear interpolation.

Upstream: `YoungModulusPARFUMESiC`.

Linear interpolation in **degrees Celsius** between four tabulated
points, clamped to the end values outside:

| `T_C` \[C\] | `E` \[GPa\] |
|---|---|
| 25 | 428 |
| 940 | 375 |
| 1215 | 340 |
| 1600 | 198 |

The steep drop over the last interval (375 to 198 GPa between 940 C and
1600 C) is the high-temperature softening of the SiC layer that governs
TRISO particle failure in accident conditions.

**Inputs used:** [`temperature`](MaterialState::temperature).

**Valid range:** 298.15 K to 1873.15 K (25 C to 1600 C), the span of the
table — **stated by the table itself**; upstream clamps to the end
values outside it, and so does [`value`](Self::value).

**Source:** PARFUME (INL TRISO fuel-performance code) material models,
as transcribed in OFFBEAT.

##### Implementations

###### Methods

- ```rust
  pub fn name(self: &Self) -> &'static str { /* ... */ }
  ```
  Human-readable name of the correlation, used in error messages.

- ```rust
  pub fn temperature_range(self: &Self) -> (f64, f64) { /* ... */ }
  ```
  Temperature validity range `(low, high)` \[K\] of this correlation.

- ```rust
  pub fn value(self: &Self, state: &MaterialState) -> f64 { /* ... */ }
  ```
  Young's modulus \[Pa\] at the given state, **clamping** out-of-range

- ```rust
  pub fn value_checked(self: &Self, state: &MaterialState) -> Result<f64> { /* ... */ }
  ```
  Young's modulus \[Pa\] at the given state, or

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
    fn clone(self: &Self) -> YoungModulusModel { /* ... */ }
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
    fn eq(self: &Self, other: &YoungModulusModel) -> bool { /* ... */ }
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
### Constants and Statics

#### Constant `MATPRO_MAX_POROSITY`

Upper porosity bound \[-\] for the MATPRO-family fuel fits.

The MATPRO porosity correction is linear, `1 - 2.752 * P`, so it predicts
**zero** stiffness at `P = 1/2.752 = 0.3634` and negative stiffness above
that. Upstream's SCK-CEN variant already guards against this by clamping
porosity with `min(porosity, 0.3)`; this port applies the same 0.3 bound to
the MATPRO UO2 and MOX variants, which upstream leaves unguarded.

```rust
pub const MATPRO_MAX_POROSITY: f64 = 0.3;
```

#### Constant `PARFUME_SATURATION_FLUENCE`

Saturation fast fluence \[n/m^2\] of the PARFUME coating-layer fits.

PARFUME's buffer and PyC modulus correlations are evaluated at this fluence
for any larger fluence — the irradiation term `1 + 0.23 * phi` saturates
rather than growing without bound. Upstream applies the same cutoff
(`min(phi, 3.96)` with `phi` in units of `1e25` n/m^2), for fast neutrons
with E > 0.18 MeV.

```rust
pub const PARFUME_SATURATION_FLUENCE: f64 = 3.96e25;
```

## Module `state`

The per-cell state every material correlation is evaluated against.

```rust
pub mod state { /* ... */ }
```

### Types

#### Struct `MaterialState`

Everything a material property correlation may depend on, for **one cell**.

# Why this type exists

Upstream OFFBEAT correlations reach into the OpenFOAM mesh registry and look
up whatever fields they need by name — `"burnup"`, `"porosity"`,
`"Intragranular_gas_swelling"` and so on — so the dependencies of a given
correlation are invisible until you read its body, and a missing field is a
runtime failure. This port inverts that: every correlation takes a
`MaterialState`, so its inputs are visible in the signature and the compiler
checks that they exist.

# Units — raw `f64`, strict SI unless stated

This struct is evaluated once per cell per property per timestep, deep inside
the numerical loops, so it carries raw `f64` rather than `uom` quantities.
Every field documents its unit; the two that are **not** plain SI are called
out explicitly because getting them wrong is the classic fuel-performance
error:

- [`burnup`](Self::burnup) is in **MWd/kgHM**, not J/kg.
- [`fast_fluence`](Self::fast_fluence) is in **n/m²** with E > 1 MeV.

# Defaults

[`MaterialState::fresh`] gives unirradiated material at a chosen temperature:
zero burnup, zero fluence, zero swelling and densification, fully dense,
stoichiometric. Build a state with that and override what the case needs,
rather than filling twelve fields by hand.

```rust
pub struct MaterialState {
    pub temperature: f64,
    pub burnup: f64,
    pub fast_fluence: f64,
    pub porosity: f64,
    pub swelling: f64,
    pub densification: f64,
    pub pu_fraction: f64,
    pub oxygen_deviation: f64,
    pub gadolinia_fraction: f64,
    pub cold_work: f64,
    pub oxygen_content: f64,
    pub hydrogen_content: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `temperature` | `f64` | Temperature \[K\]. Absolute; must be > 0. |
| `burnup` | `f64` | Burnup \[**MWd/kgHM**\] — megawatt-days per kilogram of initial heavy<br>metal.<br><br>Note the unit, and note that it differs from upstream's. OFFBEAT stores<br>burnup on the mesh in **MWd/t(oxide)** — `burnupFromPower.H` states<br>"MWd/MT_oxide" and `burnupFromPower.C` accumulates<br>`Bu += Q·Δt/ρ/1000` against the *bulk fuel* density. Each correlation<br>then converts locally: the `Bu/1000/0.881` that appears throughout is<br>MWd/t(oxide) → MWd/kg(oxide) → MWd/kgHM, using a UO2 heavy-metal mass<br>fraction of 0.881.<br><br>This port does that conversion **once**, at the crate boundary<br>([`crate::burnup`]), so correlations receive MWd/kgHM directly and cannot<br>each get it slightly wrong. Be aware that the two bases differ by about<br>13%, which is large enough to matter and small enough to look plausible<br>if confused — hence naming the basis in the field's unit rather than<br>leaving "burnup" to mean whichever the reader assumes. |
| `fast_fluence` | `f64` | Fast-neutron fluence \[n/m²\], conventionally for E > 1 MeV.<br><br>Drives irradiation hardening, irradiation creep and — for TRISO coating<br>layers — anisotropic dimensional change in pyrolytic carbon. |
| `porosity` | `f64` | Porosity \[-\], the void volume fraction in `[0, 1)`.<br><br>As-fabricated porosity plus its in-service evolution. Fuel conductivity<br>falls steeply with porosity, so this is a first-order input, not a<br>correction. |
| `swelling` | `f64` | Volumetric swelling strain \[-\] from solid and gaseous fission products.<br><br>Volumetric, not linear: divide by three for the linear equivalent. |
| `densification` | `f64` | Volumetric densification strain \[-\], **negative** for the usual<br>early-life sintering of as-fabricated porosity. |
| `pu_fraction` | `f64` | Plutonium fraction \[-\] of the heavy metal, `Pu/(U+Pu)`.<br><br>Zero for UO2, ~0.05–0.1 for LWR MOX, higher for fast-reactor fuel. |
| `oxygen_deviation` | `f64` | Deviation from stoichiometry \[-\], the `x` in `(U,Pu)O_{2+x}`.<br><br>Negative is hypostoichiometric (oxygen-deficient), which is the normal<br>condition for fast-reactor MOX. Strongly affects conductivity and<br>melting temperature. |
| `gadolinia_fraction` | `f64` | Gadolinia (Gd2O3) weight fraction \[-\] in the fuel, for burnable-poison<br>fuel. Zero for unpoisoned UO2. |
| `cold_work` | `f64` | Cold work \[-\] in the cladding, the retained cold-work fraction from<br>fabrication. Zero for fully recrystallised material. |
| `oxygen_content` | `f64` | Oxygen concentration \[-\] in Zircaloy as a weight fraction, raised by<br>waterside oxidation and by oxygen uptake at high temperature. |
| `hydrogen_content` | `f64` | Hydrogen concentration \[wt-ppm\] in the cladding, from corrosion-driven<br>hydrogen pickup. Drives hydride embrittlement. |

##### Implementations

###### Methods

- ```rust
  pub fn fresh(temperature: f64) -> Self { /* ... */ }
  ```
  Unirradiated, as-fabricated, fully dense, stoichiometric material at

- ```rust
  pub fn density_fraction(self: &Self) -> f64 { /* ... */ }
  ```
  Fraction of theoretical density \[-\], i.e. `1 - porosity`.

- ```rust
  pub fn linear_swelling(self: &Self) -> f64 { /* ... */ }
  ```
  Linear (one-dimensional) swelling strain \[-\], i.e. `swelling / 3`.

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
    fn clone(self: &Self) -> MaterialState { /* ... */ }
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
    fn eq(self: &Self, other: &MaterialState) -> bool { /* ... */ }
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

#### Re-export `MaterialState`

```rust
pub use state::MaterialState;
```

## Module `mechanics`

Solid mechanics — the displacement solve at the heart of fuel performance.

# What this module computes

Given a temperature field and the material's accumulated irradiation state,
it solves for the **displacement field** `D` of the fuel and cladding, and
from it the **stress** and **strain**. That is what tells you whether the
fuel/cladding gap has closed, how hard the pellet is pushing on the cladding,
and whether the cladding is close to failing.

# Why a fuel rod is not just a thermo-elastic body

Ordinary thermo-elasticity has one source of stress-free deformation: thermal
expansion. Irradiated fuel has four, and they are the whole subject:

| Source | Sign | Rough end-of-life magnitude |
|---|---|---|
| Thermal expansion | + | ~1% linear |
| Fission-product swelling | + | ~0.1% linear per 10 MWd/kgHM |
| Densification | − | ~0.5% linear, saturating early |
| Crack relocation | + | comparable to the as-built gap width |

All four are **eigenstrains** — deformation the material would undergo freely
if nothing constrained it, generating no stress on its own. Stress appears
only where geometry, or a neighbouring body, prevents that free deformation.
[`Eigenstrain`] carries all four together, because they enter the momentum
balance identically and separating them buys nothing.

That unification is why this module is small. Swelling is not a special case
bolted onto a thermo-elastic solver; it is one more term in `ε*`.

# The equation actually solved

Quasi-static equilibrium — no inertia, because a fuel rod evolves over months
while elastic waves cross it in microseconds:

`∇·σ = 0`

with the small-strain isotropic constitutive law, `ε*` the isotropic
(linear) eigenstrain and `I` the identity tensor:

`σ = 2μ(ε − ε* I) + λ(tr(ε) − 3ε*) I`,   `ε = ½(∇D + ∇Dᵀ)`

Substituting and splitting off an implicit Laplacian gives the segregated
form upstream's `smallStrain.C` assembles, and which
[`MechanicsSolver::solve_quasi_static`] assembles here:

`∇·[(2μ+λ)∇D] + ∇·[σ_e − (2μ+λ)∇D] − ∇(3K ε*) = 0`

The first term is implicit (a vector Laplacian on the LDU matrix), the second
is the explicit stress correction that makes the split exact at convergence,
and the third is the eigenstrain load. Because the second term depends on the
solution, the system is iterated — the outer corrector loop.

Splitting this way, rather than assembling the full anisotropic operator, is
what lets a segregated finite-volume code reuse the ordinary Laplacian
machinery per component. The price is that outer iteration.

# Inelastic deformation: creep and plasticity

[`crate::rheology`] owns the constitutive laws; this module drives them.
Attach one with [`MechanicsSolver::set_rheology`] and step the solve with
[`MechanicsSolver::solve_creep_step`] instead of
[`MechanicsSolver::solve_quasi_static`]. The coupling adds two things to the
equation above:

- the strain handed to the constitutive law is the **mechanical** strain
  `ε − ε* I`, with the eigenstrain already removed, so an unconstrained
  freely expanding body is correctly stress-free and does not creep;
- the accumulated inelastic strain `ε_in = ε_p + ε_c` re-enters the momentum
  balance as an additional (tensor) eigenstrain through the extra explicit
  term `−∇·[2μ ε_in + λ tr(ε_in) I]`, which restores equilibrium after the
  corrected stress comes back softer than the elastic one. This is the
  finite-volume analogue of upstream's `correctAdditionalStrain`, and it
  rides on exactly the same explicit-remainder hook as the segregated split.

The per-cell [`RheologyState`](crate::rheology::RheologyState) is advanced
**once** per completed step, after the corrector loop, never inside it.

# Scope of this port

**Implemented:** small-strain isotropic linear elasticity with arbitrary
isotropic eigenstrain, quasi-static and transient (inertial) forms, and the
inelastic coupling described above (creep and plasticity through
[`crate::rheology`], with [`CreepTimeStepControl`](crate::rheology::CreepTimeStepControl)
bounding the step), on a single mesh region.

**Not implemented here:** contact and gap closure ([`crate::gap`]),
large-strain updated/total Lagrangian kinematics, traction boundary
conditions, and multi-material interface correction — the last of which
matters for the stress *recovery* across a sharp material interface; see the
measured limitation recorded on
`solver::rheology_tests::spatially_varying_creep_keeps_the_axial_stress_uniform`.
Each is tracked separately under bead `op-6sl`.

```rust
pub mod mechanics { /* ... */ }
```

### Re-exports

#### Re-export `CreepStepReport`

```rust
pub use solver::CreepStepReport;
```

#### Re-export `Eigenstrain`

```rust
pub use solver::Eigenstrain;
```

#### Re-export `LinearElastic`

```rust
pub use solver::LinearElastic;
```

#### Re-export `MechanicsReport`

```rust
pub use solver::MechanicsReport;
```

#### Re-export `MechanicsSolver`

```rust
pub use solver::MechanicsSolver;
```

## Module `burnup`

Burnup accumulation and fast-neutron fluence accumulation — the
**irradiation-history bookkeeping** of a fuel-performance run.

# What this module is for

A fuel rod's material properties do not depend only on where it is and how
hot it is; they depend on *how much irradiation it has already seen*. Two
scalars carry almost all of that memory:

- **Burnup** — the thermal energy extracted from the fuel per unit mass of
  the heavy metal (uranium + plutonium) it started life with. It is the
  standard measure of "how far through its life" a piece of fuel is, and it
  drives fuel conductivity degradation, solid and gaseous swelling,
  densification, relocation, and fission-gas release.
- **Fast fluence** — the time integral of the fast-neutron flux
  (conventionally neutrons with energy above 1 MeV). Fast neutrons displace
  atoms from lattice sites; the accumulated damage drives irradiation
  hardening, irradiation creep and irradiation growth in the cladding, and
  anisotropic dimensional change in TRISO pyrolytic-carbon layers.

Both are *monotonically accumulating* quantities: this module owns the small
amount of state needed to advance them through a timestep, and nothing else.

# Units — read this before using anything here

Burnup is the classic unit trap in fuel performance, because four different
quantities are all called "burnup" and differ by factors of 1000 and by
whether the denominator is the *heavy metal* or the whole *oxide*:

| Unit | Meaning | Typical LWR discharge |
|---|---|---|
| MWd/kgHM | MW-days per kg of **initial heavy metal** | ~40-60 |
| MWd/tHM (= GWd/tHM x 1000) | same, per **tonne** of heavy metal | ~40 000-60 000 |
| MWd/t(oxide) | per tonne of **UO2**, i.e. HM *and* its oxygen | ~35 000-53 000 |
| %FIMA | percent of initial heavy-metal atoms fissioned | ~4-6 |

**This crate's canonical unit is MWd/kgHM**, matching
[`MaterialState::burnup`](crate::materials::MaterialState::burnup).

Upstream OFFBEAT is different, and the difference is worth stating precisely
because it is easy to mis-port. Upstream's `Bu` field is stored in
**MWd/t(oxide)** (see `burnupFromPower.C`, whose update is
`Bu += Q*dt_days/rho/1000` with `rho` the *fuel* density and the class
documentation reading "burnup in MWd/MT_oxide"), and every use site converts
locally — `Bu/1000/0.881` appears throughout the material correlations, which
is MWd/t(oxide) -> MWd/kg(oxide) -> MWd/kgHM. Because that conversion is
repeated at a dozen call sites upstream, it is exactly the kind of thing that
drifts. **This port converts once, here, at the boundary**, and everything
downstream receives MWd/kgHM.

The heavy-metal mass fraction that appears in that conversion is *not*
hard-coded silently: it lives in [`HeavyMetalBasis`], which the caller
constructs explicitly. See [`UO2_HEAVY_METAL_MASS_FRACTION`] for why upstream
uses two slightly different numbers (`0.881` and `0.8815`) for it.

Fluence is in **n/m²** here (matching
[`MaterialState::fast_fluence`](crate::materials::MaterialState::fast_fluence)),
whereas upstream's `fastFlux`/`fastFluence` fields are in n/cm²/s and n/cm².
Convert with [`FLUENCE_PER_CM2_TO_PER_M2`].

# What is *not* here

Upstream also ships `burnupLassmann` and `burnupLassmannFBR`: a TUBRNP-style
radial depletion module that solves a reduced Bateman chain for ~14 nuclides
and rebuilds the radial power profile from the resulting fissile-nuclide
distribution. That is a neutronics model, not bookkeeping; it needs a flux
solution and an axial slice mapper, and it is **not ported here**. Callers
needing a radial burnup profile must supply the profile themselves and drive
one [`BurnupAccumulator`] per radial ring.

Likewise the *axial shape* machinery of upstream's
`timeDependentAxialProfile` (which needs a mesh, a pin direction and the
`profiles/` library) is out of scope: [`FastFluxModel`] gives the rod-average
flux history `phi(t)`, and the caller multiplies by the normalised axial
shape `g(z, t)` themselves — exactly the product upstream forms, just with
the mesh half left to the mesh layer.

# Status

Scaffold. No human verification or validation. The tests below are
self-consistency and code-equivalence checks against the upstream C++
expressions; none of them is a validation against experiment.

```rust
pub mod burnup { /* ... */ }
```

### Types

#### Struct `HeavyMetalBasis`

The mass basis burnup is measured against: how many kilograms of **initial
heavy metal** sit in a cubic metre of fuel.

# Why this is a separate type

Burnup is energy per unit *initial heavy-metal* mass, but a thermal solver
only knows energy per unit *volume*. Converting between them needs two
numbers — the fuel's bulk density and the heavy-metal fraction of that
density — and getting either wrong scales every burnup-dependent correlation
in the run by a constant factor that is easy to miss and hard to find.
Upstream spreads these two numbers across a density-field lookup and a
literal `0.881` (or `0.8815`) repeated at a dozen call sites. Here they are
one explicitly-constructed value.

# Fields, units and ranges

- bulk fuel density \[kg/m³\] — the density of the fuel **including** its
  as-fabricated porosity, i.e. theoretical density x fraction of theoretical
  density. Must be > 0. Typical LWR UO2: ~10 400 kg/m³.
- heavy-metal mass fraction \[-\] — kg of U + Pu per kg of fuel. Must be in
  `(0, 1]`. UO2: 0.8815 ([`UO2_HEAVY_METAL_MASS_FRACTION`]); metal fuel: 1.0
  for pure U, ~0.9 for U-10Zr.

# Assumptions

The basis is the **as-fabricated** one and never changes during irradiation.
That is correct by definition — burnup is per *initial* heavy metal, so the
denominator is deliberately frozen even though the fuel's actual heavy-metal
content falls as it is fissioned, and its bulk density changes with
densification, swelling and thermal expansion.

```rust
pub struct HeavyMetalBasis {
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
  pub fn new(fuel_density: f64, heavy_metal_fraction: f64) -> Result<Self> { /* ... */ }
  ```
  Build a basis from an explicit bulk fuel density \[kg/m³\] and

- ```rust
  pub fn uo2(density_fraction: f64) -> Result<Self> { /* ... */ }
  ```
  Basis for stoichiometric UO2 at a given fraction of theoretical density.

- ```rust
  pub fn fuel_density(self: &Self) -> f64 { /* ... */ }
  ```
  Bulk fuel density \[kg/m³\], including as-fabricated porosity.

- ```rust
  pub fn heavy_metal_fraction(self: &Self) -> f64 { /* ... */ }
  ```
  Heavy-metal mass fraction \[-\], kg of U + Pu per kg of fuel.

- ```rust
  pub fn heavy_metal_density(self: &Self) -> f64 { /* ... */ }
  ```
  Initial heavy-metal density \[kg-HM/m³\] — the product of the two fields.

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
    fn clone(self: &Self) -> HeavyMetalBasis { /* ... */ }
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
    fn eq(self: &Self, other: &HeavyMetalBasis) -> bool { /* ... */ }
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
#### Struct `BurnupAccumulator`

Accumulates local burnup and fast-neutron fluence through an irradiation
history.

# What it represents

One **cell's worth** of irradiation memory: the total energy extracted per kg
of initial heavy metal, and the total fast-neutron fluence, since beginning
of life. It is deliberately tiny and owns its data by value, so a mesh-wide
field of them is a plain `Vec<BurnupAccumulator>` with no lifetimes and no
shared borrows.

# The two updates

Per timestep of length `dt` \[s\]:

- burnup: `Bu += Q · dt / (rho_fuel · f_HM) / 8.64e10` \[MWd/kgHM\], with `Q`
  the volumetric heat generation \[W/m³\]. This is upstream
  `burnupFromPower.C`'s `Bu += Q*dt_days/rho/1000`, with the
  oxide-to-heavy-metal conversion (which upstream defers to each use site)
  folded in here.
- fluence: `Phi += phi · dt` \[n/m²\], upstream
  `constantFastFlux::advanceFluence`.

Both are explicit (forward-Euler) integrations using the *end-of-step* power
and flux, exactly as upstream does. That is first-order accurate in `dt`;
over a slow irradiation with smoothly varying power the error is negligible,
but during a fast ramp the timestep must be small enough that power is nearly
constant across it — which is what [`Self::next_time_step`] is for.

# Units

State is carried in the crate's canonical units — burnup MWd/kgHM, fluence
n/m² — so [`Self::apply_to`] can write straight into a
[`MaterialState`]. Every accessor names its
unit; nothing here is "just a number".

# Example

```
use outram_park_fork_offbeat::burnup::{BurnupAccumulator, HeavyMetalBasis};

let basis = HeavyMetalBasis::uo2(0.95).unwrap();
let mut acc = BurnupAccumulator::new(basis);

// 379 MW/m^3 (about 20 kW/m in an 8.2 mm pellet) for 1000 days,
// with a fast flux of 1e18 n/m^2/s (= 1e14 n/cm^2/s).
acc.advance(3.79e8, 1.0e18, 1000.0 * 86_400.0).unwrap();

assert!((acc.burnup_mwd_per_kg_hm() - 41.3).abs() < 0.1);
assert!((acc.fast_fluence_per_m2() - 8.64e25).abs() < 1e20);
```

```rust
pub struct BurnupAccumulator {
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
  pub fn new(basis: HeavyMetalBasis) -> Self { /* ... */ }
  ```
  A fresh, unirradiated accumulator: zero burnup, zero fluence, zero

- ```rust
  pub fn restart(basis: HeavyMetalBasis, burnup: f64, fast_fluence: f64) -> Result<Self> { /* ... */ }
  ```
  An accumulator restarted from a known irradiation state.

- ```rust
  pub fn advance(self: &mut Self, power_density: f64, fast_flux: f64, dt: f64) -> Result<()> { /* ... */ }
  ```
  Advance both burnup and fluence over one timestep.

- ```rust
  pub fn advance_burnup(self: &mut Self, power_density: f64, dt: f64) -> Result<()> { /* ... */ }
  ```
  Advance burnup only, from local volumetric power over `dt`.

- ```rust
  pub fn advance_fluence(self: &mut Self, fast_flux: f64, dt: f64) -> Result<()> { /* ... */ }
  ```
  Advance fast fluence only, from local fast flux over `dt`.

- ```rust
  pub fn burnup_mwd_per_kg_hm(self: &Self) -> f64 { /* ... */ }
  ```
  Burnup \[MWd/kgHM\] — the crate's canonical unit, and the one

- ```rust
  pub fn burnup_mwd_per_tonne_hm(self: &Self) -> f64 { /* ... */ }
  ```
  Burnup \[MWd/tHM\], i.e. per *tonne* of initial heavy metal.

- ```rust
  pub fn burnup_mwd_per_tonne_oxide(self: &Self) -> f64 { /* ... */ }
  ```
  Burnup \[MWd/t(oxide)\] — **upstream OFFBEAT's own `Bu` field unit**.

- ```rust
  pub fn burnup_joules_per_kg_hm(self: &Self) -> f64 { /* ... */ }
  ```
  Burnup \[J/kgHM\] — energy per unit initial heavy-metal mass, in strict

- ```rust
  pub fn burnup_percent_fima(self: &Self) -> f64 { /* ... */ }
  ```
  Burnup as \[%FIMA\] — percent of the initial heavy-metal atoms fissioned.

- ```rust
  pub fn fast_fluence_per_m2(self: &Self) -> f64 { /* ... */ }
  ```
  Fast fluence \[n/m²\], E > 1 MeV — the crate's canonical unit.

- ```rust
  pub fn fast_fluence_per_cm2(self: &Self) -> f64 { /* ... */ }
  ```
  Fast fluence \[n/cm²\], E > 1 MeV — **upstream OFFBEAT's field unit**,

- ```rust
  pub fn basis(self: &Self) -> HeavyMetalBasis { /* ... */ }
  ```
  The mass basis this accumulator measures burnup against.

- ```rust
  pub fn elapsed_time(self: &Self) -> f64 { /* ... */ }
  ```
  Total irradiation time integrated so far \[s\].

- ```rust
  pub fn last_burnup_increment(self: &Self) -> f64 { /* ... */ }
  ```
  Burnup added by the most recent [`Self::advance_burnup`] call

- ```rust
  pub fn apply_to(self: &Self, state: &mut MaterialState) { /* ... */ }
  ```
  Write this accumulator's burnup and fluence into a

- ```rust
  pub fn next_time_step(self: &Self, current_dt: f64, max_increment: f64) -> Result<f64> { /* ... */ }
  ```
  Timestep \[s\] the burnup model would like to take next, so that the

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
    fn clone(self: &Self) -> BurnupAccumulator { /* ... */ }
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
    fn eq(self: &Self, other: &BurnupAccumulator) -> bool { /* ... */ }
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
#### Enum `TimeInterpolation`

How to interpolate a tabulated history between its time points.

Port of the two options upstream's `interpolateTableBase` exposes and that
`timeDependentAxialProfile` accepts under the `timeInterpolationMethod`
keyword (upstream default `linear`).

```rust
pub enum TimeInterpolation {
    Linear,
    Step,
}
```

##### Variants

###### `Linear`

Piecewise linear between bracketing points. Upstream's default, and the
right choice for a power or flux ramp.

###### `Step`

Piecewise constant: hold the value of the preceding time point until the
next one. The right choice when the table represents discrete operating
states rather than a continuous ramp.

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
    fn clone(self: &Self) -> TimeInterpolation { /* ... */ }
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
    fn default() -> TimeInterpolation { /* ... */ }
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
    fn eq(self: &Self, other: &TimeInterpolation) -> bool { /* ... */ }
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
#### Struct `FastFluxHistory`

A tabulated rod-average fast-flux history `phi(t)`.

# What it represents

The fast-neutron flux (E > 1 MeV) averaged over the rod, as a function of
time — an irradiation history read from a reactor-physics calculation or from
an experiment's operating record. Port of the `timePoints` / `fastFlux`
tables in upstream's `timeDependentAxialProfile` fast-flux model.

# Units and ranges

- times \[s\], strictly increasing, at least one point.
- fluxes \[n/(m²·s)\], non-negative. **Note the unit differs from upstream**,
  whose tables are in n/(cm²·s); multiply an upstream table by
  [`FLUENCE_PER_CM2_TO_PER_M2`] before passing it here.

# Behaviour outside the table

Clamped: a query before the first time point returns the first value, after
the last returns the last. It never extrapolates, because extrapolating a
measured irradiation history is meaningless.

# What is deliberately missing

Upstream forms `fastFlux(t, z) = phi(t) · g(z, t)` where `g` is a normalised
axial shape supplied by the `axialProfile` class hierarchy, which needs a
mesh, a pin direction and the axial extent of the fuel. None of that is
ported here — this type gives `phi(t)` and the caller multiplies by their own
`g(z, t)`.

```rust
pub struct FastFluxHistory {
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
  pub fn new(times: Vec<f64>, fluxes: Vec<f64>, method: TimeInterpolation) -> Result<Self> { /* ... */ }
  ```
  Build a history from paired time \[s\] and flux \[n/(m²·s)\] tables.

- ```rust
  pub fn flux_at(self: &Self, t: f64) -> f64 { /* ... */ }
  ```
  Rod-average fast flux \[n/(m²·s)\] at time `t` \[s\].

- ```rust
  pub fn times(self: &Self) -> &[f64] { /* ... */ }
  ```
  The tabulated time points \[s\].

- ```rust
  pub fn fluxes(self: &Self) -> &[f64] { /* ... */ }
  ```
  The tabulated flux values \[n/(m²·s)\].

- ```rust
  pub fn interpolation(self: &Self) -> TimeInterpolation { /* ... */ }
  ```
  The interpolation method this history was built with.

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
    fn clone(self: &Self) -> FastFluxHistory { /* ... */ }
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
    fn eq(self: &Self, other: &FastFluxHistory) -> bool { /* ... */ }
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
#### Enum `FastFluxModel`

Which fast-flux model supplies `phi(t)` to the fluence accumulation.

# Why an enum and not a trait object

The set of fast-flux models is closed and known at compile time, so a `match`
here is exhaustive: adding a variant makes every dispatch site a compile
error rather than a runtime surprise. This is the workspace rule (root
`CLAUDE.md`, "No trait objects"), and it also means go-to-definition works on
each variant, which it does not on a `dyn` implementation.

# Variants map one-to-one onto upstream's runtime-selectable models

| Here | Upstream `fastFlux` typename | Upstream file |
|---|---|---|
| [`Self::Disabled`] | `none` | `fastFlux.C` |
| [`Self::Constant`] | `constant` | `constantFastFlux.C` |
| [`Self::Tabulated`] | `timeDependentAxialProfile` | `timeDependentAxialProfile.C` |

The axial-shape half of `timeDependentAxialProfile` is not ported; see
[`FastFluxHistory`].

```rust
pub enum FastFluxModel {
    Disabled,
    Constant(f64),
    Tabulated(FastFluxHistory),
}
```

##### Variants

###### `Disabled`

No fast flux is modelled; `phi(t) = 0` always, so fluence never grows.

Upstream's `none` model does not create the `fastFlux`/`fastFluence`
fields at all, so any correlation that needs them fails loudly. This port
cannot do that — it returns zero — so **be aware that selecting this
variant silently gives every fluence-dependent correlation an
unirradiated input.** For a frozen non-zero fluence use [`Self::Constant`]
with a zero flux together with [`BurnupAccumulator::restart`], which is
the analogue of upstream's own advice to prefer `constant` over `none`.

###### `Constant`

A time-invariant flux \[n/(m²·s)\]; fluence grows linearly.

Upstream's `constant` model: the flux field is read once from the start
time directory (or defaults to zero) and never changes, but the fluence
*is* still integrated every timestep.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `Tabulated`

A tabulated flux history `phi(t)`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `FastFluxHistory` |  |

##### Implementations

###### Methods

- ```rust
  pub fn flux_at(self: &Self, t: f64) -> f64 { /* ... */ }
  ```
  Rod-average fast flux \[n/(m²·s)\] at time `t` \[s\].

- ```rust
  pub fn upstream_name(self: &Self) -> &'static str { /* ... */ }
  ```
  Upstream's runtime-selection typename for this model, for logging and

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
    fn clone(self: &Self) -> FastFluxModel { /* ... */ }
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
    fn default() -> FastFluxModel { /* ... */ }
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
    fn eq(self: &Self, other: &FastFluxModel) -> bool { /* ... */ }
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

#### Function `fission_rate_density`

Fission-rate density \[fissions/(m³·s)\] from volumetric heat generation
\[W/m³\].

Divides by [`ENERGY_PER_FISSION_J`]. This is the bridge between the thermal
solve (which knows power) and every fission-product model (which needs a
fission rate) — fission-gas production in [`crate::fgr`] is driven from it,
as is upstream's MATPRO irradiation-creep law.

# Inputs and range

`power_density` in W/m³, must be finite and non-negative. A representative
LWR pellet-average value is 3–4 x 10⁸ W/m³ (20 kW/m over an 8.2 mm pellet),
giving about 1.2 x 10¹⁹ fissions/(m³·s) = 1.2 x 10¹³ fissions/(cm³·s).

# Errors

[`OffbeatError::Unphysical`] if `power_density` is negative or not finite.

```
use outram_park_fork_offbeat::burnup::fission_rate_density;

let f = fission_rate_density(3.79e8).unwrap();
assert!(f > 1.0e19 && f < 1.3e19);
```

```rust
pub fn fission_rate_density(power_density: f64) -> crate::error::Result<f64> { /* ... */ }
```

### Constants and Statics

#### Constant `SECONDS_PER_DAY`

Seconds in a day \[s/d\] — exactly 86 400.

Named because the burnup update is the one place in a fuel-performance code
where a seconds/days mix-up produces a plausible-looking wrong answer rather
than an obvious blow-up.

```rust
pub const SECONDS_PER_DAY: f64 = 86_400.0;
```

#### Constant `JOULES_PER_MEGAWATT_DAY`

Joules in one megawatt-day \[J/(MW·d)\] — exactly `1e6 * 86400 = 8.64e10`.

This is the whole of the burnup unit conversion: energy per unit heavy-metal
mass in J/kgHM, divided by this constant, is burnup in MWd/kgHM.

```rust
pub const JOULES_PER_MEGAWATT_DAY: f64 = 8.64e10;
```

#### Constant `UO2_THEORETICAL_DENSITY`

Theoretical (pore-free) density of stoichiometric UO2 at room temperature
\[kg/m³\].

Value 10 960 kg/m³, taken from upstream `burnupLassmann.C` (line ~625,
`rho_hm = 10960*densityFractionAverage*0.8815`). Real fabricated pellets are
94–97 % of this; multiply by the fraction of theoretical density — which is
what [`HeavyMetalBasis::uo2`] does.

```rust
pub const UO2_THEORETICAL_DENSITY: f64 = 10_960.0;
```

#### Constant `UO2_HEAVY_METAL_MASS_FRACTION`

Heavy-metal mass fraction of stoichiometric UO2 \[kg-HM / kg-UO2\].

# Where the number comes from

`M(U) / (M(U) + 2 M(O)) = 238.029 / (238.029 + 2 x 15.999) = 0.88150`.

# Why upstream has two of them

OFFBEAT is not internally consistent about this constant, and a port that
silently picks one hides a real (small) inconsistency in the original:

- `0.8815` in `burnupLassmann.C` (lines 436–437, 625, 645, 800–812) and in
  the bundled SCIANTIX (`GlobalVariables.C`, `U_UO2 = 0.8815`);
- `0.881` in every material correlation that converts burnup, e.g.
  `conductivityMatproUO2.C:175`, `swellingFRAPCON.C:132`,
  `densificationFRAPCON.C:128`, `relocationFRAPCON.C:338`.

The two differ by 0.057 %, which is far inside the scatter of any of those
correlations, so neither is "wrong" — but they *are* different, so this port
exposes the fraction as data rather than baking it in. This constant is the
more accurate `0.8815`; pass `0.881` to [`HeavyMetalBasis::new`] if you are
reproducing an upstream correlation number exactly.

# For anything that is not UO2

MOX, U-Pu-Zr metal fuel, UN, UC and TRISO kernels all have different
heavy-metal fractions. Do not reuse this value for them — compute the
fraction from the actual stoichiometry and pass it to
[`HeavyMetalBasis::new`].

```rust
pub const UO2_HEAVY_METAL_MASS_FRACTION: f64 = 0.8815;
```

#### Constant `MWD_PER_KGHM_PER_PERCENT_FIMA`

Burnup, in MWd/kgHM, corresponding to 1 % FIMA \[MWd/kgHM per %FIMA\].

FIMA = "fissions per initial metal atom". Value 9.3706 is upstream's, from
`fissionProductsDiffusionSolver.C:119`
(`b = Bu*1e-3/0.881/9.3706`, which takes MWd/t-oxide to %FIMA).

Physically it is the energy released by fissioning 1 % of the initial
heavy-metal atoms in a kilogram of uranium. It depends weakly on the isotopic
mix and on the energy released per fission, so treat it as a ~1 %-accurate
engineering conversion, not an exact identity.

```rust
pub const MWD_PER_KGHM_PER_PERCENT_FIMA: f64 = 9.3706;
```

#### Constant `ENERGY_PER_FISSION_J`

Recoverable energy released per fission \[J\].

Value `3.12e-11 J` (= 194.7 MeV), which is upstream's `312.0e-13` — see
`fgrSCIANTIX.C:649-650` (`Fissionrate = Q/312.0e-13`) and
`MatproCreepModel.C:229` (`F = heatSource/312e-13`).

This is the *recoverable* energy (fission fragments, prompt and delayed
neutrons and gammas, beta decay), not the ~202 MeV total including
antineutrinos, and not the ~168 MeV fragment kinetic energy alone. It varies
by a few percent between fissioning nuclides; upstream uses one number for
all of them, and so does this port.

```rust
pub const ENERGY_PER_FISSION_J: f64 = 3.12e-11;
```

#### Constant `FLUENCE_PER_CM2_TO_PER_M2`

Multiply a fluence in n/cm² by this to get n/m² \[-\]; likewise a flux in
n/cm²/s to get n/m²/s. Exactly `1e4`.

Upstream stores `fastFlux` in n/cm²/s and `fastFluence` in n/cm²
(`constantFastFlux.H`). This crate stores n/m²/s and n/m², matching
[`MaterialState::fast_fluence`](crate::materials::MaterialState::fast_fluence).

```rust
pub const FLUENCE_PER_CM2_TO_PER_M2: f64 = 1.0e4;
```

## Module `corrosion`

Cladding waterside corrosion, hydrogen pickup, and nonlinear-solver
acceleration.

# What waterside corrosion is, for a reader with no fuel-performance
background

A light-water reactor fuel rod is a thin zirconium-alloy (Zircaloy) tube
holding a stack of uranium-dioxide pellets. Its outside is bathed in hot
water — roughly 560–620 K at 15.5 MPa in a PWR — and zirconium is
thermodynamically unstable in water. It oxidises:

```text
Zr + 2 H2O  ->  ZrO2 + 2 H2
```

A layer of zirconia (ZrO2) therefore grows on the outer surface over the
rod's four-to-six-year life. It is not a cosmetic effect; it matters for
three separate reasons, and this module computes all three.

1. **The oxide is a thermal insulator.** ZrO2 conducts about 2 W/(m·K)
   against Zircaloy's ~15 W/(m·K), so every micron of oxide adds thermal
   resistance between the fuel and the coolant and pushes the whole rod
   hotter. Hotter metal oxidises faster, so the effect is self-reinforcing.
   See [`thermal`].
2. **It eats load-bearing wall.** The metal consumed to make the oxide is
   gone from the pressure boundary. Because ZrO2 occupies more volume than
   the zirconium it came from — the **Pilling–Bedworth ratio**, 1.56 for
   Zr — a layer of oxide `S` thick has consumed only `S/1.56` of metal.
   See [`CorrosionModel::metal_loss`].
3. **Some of the hydrogen goes into the metal.** The reaction above
   liberates two H2 per zirconium atom. Most of it leaves with the coolant,
   but a *pickup fraction* — 15–25% depending on the alloy — dissolves into
   the cladding, where above its solubility limit it precipitates as
   zirconium hydride platelets. Hydrides are brittle, so hydrogen pickup is
   the mechanism by which corrosion turns into a **cladding-failure**
   problem rather than merely a heat-transfer one. See [`hydrogen`].

# Sub-transition and post-transition kinetics

Oxide growth is not a single power law. While the layer is thin it is dense
and adherent, and it is itself the diffusion barrier that limits further
oxidation, so growth *decelerates* — approximately cubic in time,
`S^3 ∝ t`. At a **transition thickness** of about 2 µm the layer cracks and
develops interconnected porosity, the diffusion barrier stops thickening in
any useful sense, and growth becomes approximately **linear** in time and
much faster. Every model in [`kinetics`] has this two-regime structure, and
the acceleration at transition is the single most important qualitative
feature to get right.

At accident temperatures (above ~673 K) the mechanism changes again to fast
**parabolic** high-temperature steam oxidation, which is what the
[`CathcartPawel`](kinetics::OxidationKinetics::CathcartPawel) branch
describes.

# What is in this module

| Submodule | Contents |
|---|---|
| [`kinetics`] | [`OxidationKinetics`] — the oxide-growth laws themselves |
| [`model`] | [`CorrosionModel`] — a whole patch-level corrosion model |
| [`state`] | [`CorrosionState`] / [`CorrosionStep`] — inputs and results of one step |
| [`hydrogen`] | [`HydrogenPickupModel`] — hydrogen ingress into the metal |
| [`thermal`] | oxide conductivity and the metal/oxide interface temperature |
| [`acceleration`] | [`AccelerationScheme`] — Anderson mixing, a general nonlinear-solver accelerator |

# Units — raw `f64`, strict SI

Everything crossing a public boundary in this module is raw `f64` in strict
SI, and every item states its unit. Three conversions are done **once**, at
the boundary, rather than being left to each correlation — which is where
upstream does them, and where they are easy to get wrong:

- **Time is seconds.** Upstream's low-temperature kinetics divides by
  `3600*24` internally because its rate constants are quoted per day.
- **Fast flux is n/(m²·s).** Upstream's `fastFlux` field is documented as
  **n/(cm²·s)** (see `constantFastFlux.H`), and the EPRI/KWU/C-E flux term
  is fitted on that basis. This port takes SI and multiplies by `1e-4`
  inside the correlation. Getting this wrong changes the post-transition
  rate substantially, so it is called out here rather than buried.
- **Hydrogen concentration is wt-ppm**, matching
  [`MaterialState::hydrogen_content`](crate::materials::MaterialState::hydrogen_content).
  This is a mass fraction times 1e6, not an SI unit, and is the unit the
  entire hydride literature uses.

# What is deliberately NOT ported: the layer addition/removal topology changer

Upstream's `corrosion/layerAdditionRemovalPolyTopoChanger/` is **not**
translated here, and no stand-in for it is provided.

**What it does.** As the oxide grows, the metal wall thins. Upstream
represents that by physically moving the mesh: `corrosion::updateMesh()`
displaces the boundary points inward by the metal thickness lost
(`-DMetalThickness * n_f`), and the topology changer watches the resulting
near-wall cell layer. When that layer is squashed below a minimum thickness
it **removes** the layer from the mesh; when it is stretched past a maximum
it **adds** one. Upstream sets those bounds automatically per patch — a
quarter of the initial face-to-cell-centre distance for the minimum, four
times it for the maximum — and wraps one OpenFOAM `layerAdditionRemoval`
mesh modifier per boundary patch in a `polyTopoChanger`.

**Why it is deferred.** It is not a correlation with a mesh-shaped
interface; it *is* a mesh operation. Executing it needs a live mutable
`polyMesh` — face zones, a `polyTopoChange` engine that can renumber points,
faces, cells and boundary patches, and a `mapPolyMesh` to carry every
existing field across the renumbering. `outram-foam-basic-lib` provides the
finite-volume substrate this crate builds on, but not runtime topology
modification. Writing a plausible-looking `add_layer` / `remove_layer` here
would produce something that compiles, has no mesh to act on, and could
never be tested — the opposite of useful.

**What you get instead.** The *kinetics* are ported as pure functions, and
[`CorrosionStep::metal_loss`] gives the inward wall displacement in metres
that a caller with a real mesh must apply. Wiring that displacement into a
moving mesh, and adding or removing layers when cells become degenerate, is
left to whoever owns the mesh. This is stated as deferred work rather than
quietly stubbed.

Two smaller pieces of upstream are also left out, for the same "needs a live
mesh" reason: `corrosionByPatch` (the per-patch driver that owns the
`oxideThickness`/`DOxideThickness` surface fields, under-relaxes them
between outer iterations, and prints the area-averaged summary), and the
`oxidePickupFraction` boundary condition's role as an actual finite-volume
flux boundary condition. The physics inside both is here; the field
plumbing is not.

# Status

**AI-assisted translation, reviewed by no human.** Per `RESPONSIBLE_USE.md`
this is untrusted draft material. The tests in this module establish
internal consistency with upstream's algebra and with conservation of the
hydrogen the reaction liberates — they are **not** validation against
measured oxide-thickness data, and nothing here may be described as
validated. Three of them exist specifically to pin **upstream behaviour that
this port reproduces deliberately** — two demonstrable arithmetic defects
(the Cathcart–Pawel 1800–1900 K interpolation, in [`kinetics`], and the
`volFactor` numerator, in [`hydrogen`]) and one surprising-but-intended
discontinuity (the 673 K model switch, in [`kinetics`]). Those tests are
labelled as such and are not an endorsement.

[`OxidationKinetics`]: kinetics::OxidationKinetics
[`CorrosionModel`]: model::CorrosionModel
[`CorrosionModel::metal_loss`]: model::CorrosionModel::metal_loss
[`CorrosionState`]: state::CorrosionState
[`CorrosionStep`]: state::CorrosionStep
[`CorrosionStep::metal_loss`]: state::CorrosionStep::metal_loss
[`HydrogenPickupModel`]: hydrogen::HydrogenPickupModel
[`AccelerationScheme`]: acceleration::AccelerationScheme

```rust
pub mod corrosion { /* ... */ }
```

### Modules

## Module `acceleration`

**Attributes:**

- `Other("#[allow(clippy::neg_cmp_op_on_partial_ord)]")`

Anderson mixing — making a slowly-converging fixed-point iteration converge
fast.

# The problem this solves

Fuel performance is a strongly coupled loop: power sets temperature,
temperature sets thermal expansion and creep, deformation closes the
fuel/cladding gap, gap closure changes the gap conductance, which changes
temperature again. Solvers close that loop by **fixed-point iteration** —
guess a state, run one pass of every physics, take the answer as the next
guess, repeat until it stops changing:

```text
x_{k+1} = g(x_k)
```

This is *Picard iteration*, and it works, but its error decays by a constant
factor each pass. When the coupling is strong that factor is close to one,
and the loop takes hundreds or thousands of passes — each of which is a full
multiphysics solve.

# What Anderson mixing does about it

Instead of taking the newest iterate, Anderson mixing keeps the last few
iterates and forms the linear combination of them that best cancels the
*differences* between successive iterates. Where Picard uses one point of
history, Anderson uses `order + 1`, so it can see the shape of the
convergence and jump ahead along it. On a linear problem it is equivalent to
a Krylov method and can annihilate one error mode per unit of history depth
rather than damping every mode by the same factor.

The method is also known as **Pulay mixing** or **DIIS** (direct inversion
in the iterative subspace), and upstream's implementation is the restarted
variant: it accumulates `order + 1` snapshots, extrapolates, throws the
history away, and starts collecting again.

# It is not specific to corrosion

Upstream keeps this in its own top-level `accelerationSchemes/` directory
and applies it wherever an outer iteration is slow. It lives inside
[`crate::corrosion`] in this port only because that is where the ported
directory landed; nothing about it is chemical. Anything of the shape
`x = g(x)` on a vector of `f64` can use it.

# The algorithm, exactly

With `order = m`, having collected snapshots `x_0 … x_m`:

1. Difference vectors `e_i = x_{i+1} − x_i` for `i = 0 … m−1`. These are the
   per-step corrections, and driving them to zero is the same as converging.
2. Gram matrix `T_ij = <e_i, e_j>` (`m × m`, symmetric).
3. Normalise: `T /= max|T_ij|`, then add `diagonal_factor` to the diagonal
   to make it diagonally dominant. Without that regularisation `T` is
   typically near-singular, because successive corrections are nearly
   parallel — which is precisely the situation Anderson mixing exists to
   exploit.
4. Solve `T b = 1` (a vector of ones) and rescale so `Σ b_i = 1`. This is
   the DIIS condition: minimise `‖Σ b_i e_i‖` subject to the coefficients
   summing to one.
5. New iterate: `x = Σ b_i · ((1 − α)·x_i + α·x_{i+1})`.
6. Discard the history and start again.

# Convergence guarantees — there are none

Anderson mixing accelerates a convergent iteration; it does not make a
divergent one converge, and on a strongly nonlinear problem it can
occasionally take a worse step than plain Picard would have. Upstream's
`diagonal_factor` exists to blunt that. The one case with a firm theoretical
footing is a **linear** fixed point, where the method is a Krylov method and
the speed-up is real and measurable — which is why the tests below use a
linear problem with a closed-form solution as their reference.

# Units

None. This is a pure numerical utility on `f64` vectors; the caller's
vector may hold whatever it likes, in whatever units, as long as the
components are commensurate enough for a Euclidean inner product over them
to mean something. (Upstream has the same caveat, and the same silence about
it: mixing a temperature field and a stress field in one vector makes the
Gram matrix dominated by whichever has the larger numbers.)

```rust
pub mod acceleration { /* ... */ }
```

### Types

#### Enum `AccelerationOutcome`

What a call to [`AccelerationScheme::accelerate`] did.

Returned rather than a bare `bool` because "the history is not full yet" and
"the history is full but the extrapolation was degenerate" are different
events with different consequences, and a caller that conflates them will
silently run an unaccelerated solve believing it is accelerated.

```rust
pub enum AccelerationOutcome {
    Stored,
    Accelerated,
    Degenerate,
}
```

##### Variants

###### `Stored`

The iterate was stored as a snapshot and left unchanged. The history is
not yet full, or the scheme is [`AccelerationScheme::None`].

###### `Accelerated`

The history was full; the iterate has been **overwritten** with the
extrapolated value and the history reset.

###### `Degenerate`

The history was full but no useful extrapolation exists — the
difference vectors are all zero (the iteration has already converged
exactly), or the projection matrix is singular, or the coefficients sum
to zero. The iterate is left **unchanged** and the history is reset.

This is not an error. It is the correct response to "there is nothing
left to extrapolate", and the commonest cause is a converged iteration.
Upstream instead aborts with a fatal error when the coefficients sum to
zero, and produces `NaN` when the difference vectors vanish; both are
reported here instead.

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
    fn clone(self: &Self) -> AccelerationOutcome { /* ... */ }
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
    fn eq(self: &Self, other: &AccelerationOutcome) -> bool { /* ... */ }
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
#### Struct `FixedPointReport`

The outcome of running a fixed-point iteration to convergence.

Returned by [`AccelerationScheme::iterate`].

```rust
pub struct FixedPointReport {
    pub iterations: usize,
    pub residual: f64,
    pub converged: bool,
    pub accelerations: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `iterations` | `usize` | Number of times `g` was evaluated. Each is one full pass of whatever<br>the caller's iteration does, so this is the number to compare between<br>schemes. |
| `residual` | `f64` | Final residual `‖g(x) − x‖₂` — the Euclidean norm of the last Picard<br>correction, **before** any acceleration was applied to it. |
| `converged` | `bool` | Whether [`residual`](Self::residual) reached the requested tolerance<br>within the iteration budget. |
| `accelerations` | `usize` | How many times the scheme actually extrapolated, i.e. returned<br>[`AccelerationOutcome::Accelerated`]. Zero for<br>[`AccelerationScheme::None`]. |

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
    fn clone(self: &Self) -> FixedPointReport { /* ... */ }
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
    fn eq(self: &Self, other: &FixedPointReport) -> bool { /* ... */ }
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
#### Struct `AndersonMixing`

Restarted Anderson mixing (Pulay / DIIS) over `f64` vectors — upstream
`andersonMixingScheme`, `TypeName("andersonMixing")`.

See the [module documentation](self) for the algorithm and for what it does
and does not guarantee.

# Owning the history

The struct owns its snapshots by value in a `Vec<Vec<f64>>`. No `Box`, no
`dyn`, no lifetime parameters — per the workspace `CLAUDE.md` Rust design
rules. Memory is `(order + 1) × n` doubles, so a depth-5 scheme over a
million-cell field costs 48 MB; that is the real cost of the method and it
is why `order` is usually 3–8 rather than 50.

# Example

```
use outram_park_fork_offbeat::corrosion::{AccelerationScheme, AndersonMixing};

// x = g(x) with g(x) = 0.99*x + 1, whose fixed point is x = 100.
let mut scheme = AccelerationScheme::Anderson(AndersonMixing::new(3));
let mut x = vec![0.0];
let report = scheme.iterate(&mut x, 1.0e-12, 5000, |current, next| {
    next[0] = 0.99 * current[0] + 1.0;
});

assert!(report.converged);
assert!((x[0] - 100.0).abs() < 1.0e-8);
```

```rust
pub struct AndersonMixing {
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
  pub fn new(order: usize) -> Self { /* ... */ }
  ```
  Anderson mixing of the given `order` with upstream's default `alpha`

- ```rust
  pub fn with_parameters(order: usize, alpha: f64, diagonal_factor: f64) -> Self { /* ... */ }
  ```
  Anderson mixing with every parameter given explicitly.

- ```rust
  pub fn order(self: &Self) -> usize { /* ... */ }
  ```
  The acceleration order \[-\] — the history depth minus one.

- ```rust
  pub fn alpha(self: &Self) -> f64 { /* ... */ }
  ```
  The blending factor `α` \[-\].

- ```rust
  pub fn diagonal_factor(self: &Self) -> f64 { /* ... */ }
  ```
  The projection-matrix diagonal regularisation \[-\].

- ```rust
  pub fn stored_snapshots(self: &Self) -> usize { /* ... */ }
  ```
  How many snapshots are currently held, from 0 to `order + 1`.

- ```rust
  pub fn reset(self: &mut Self) { /* ... */ }
  ```
  Discard the history.

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
    fn clone(self: &Self) -> AndersonMixing { /* ... */ }
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
    fn eq(self: &Self, other: &AndersonMixing) -> bool { /* ... */ }
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
#### Enum `AccelerationScheme`

Which acceleration scheme an outer iteration uses.

One variant per scheme in upstream OFFBEAT's `accelerationScheme` run-time
selection table. Dispatch is by `match`, never by a trait object, per the
workspace `CLAUDE.md` "No trait objects" rule.

```rust
pub enum AccelerationScheme {
    None,
    Anderson(AndersonMixing),
}
```

##### Variants

###### `None`

No acceleration — plain Picard iteration. Upstream's base
`accelerationScheme`, `TypeName("none")`.

[`accelerate`](Self::accelerate) always returns
[`AccelerationOutcome::Stored`] and leaves the iterate alone, so this is
the honest baseline to compare an accelerated run against — and the
thing to fall back to when acceleration misbehaves.

###### `Anderson`

Restarted Anderson mixing. Upstream `andersonMixingScheme`,
`TypeName("andersonMixing")`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `AndersonMixing` |  |

##### Implementations

###### Methods

- ```rust
  pub fn reset(self: &mut Self) { /* ... */ }
  ```
  Discard any history the scheme holds. No-op for [`None`](Self::None).

- ```rust
  pub fn min_iterations(self: &Self) -> usize { /* ... */ }
  ```
  The minimum number of iterations before this scheme can extrapolate at

- ```rust
  pub fn accelerate(self: &mut Self, x: &mut [f64]) -> AccelerationOutcome { /* ... */ }
  ```
  Offer the current iterate to the scheme, and let it extrapolate if it

- ```rust
  pub fn iterate<G>(self: &mut Self, x: &mut [f64], tolerance: f64, max_iterations: usize, g: G) -> FixedPointReport
where
    G: FnMut(&[f64], &mut [f64]) { /* ... */ }
  ```
  Run `x ← g(x)` to convergence, accelerating with this scheme.

- ```rust
  pub fn iterate_checked<G>(self: &mut Self, x: &mut [f64], tolerance: f64, max_iterations: usize, g: G) -> Result<FixedPointReport>
where
    G: FnMut(&[f64], &mut [f64]) { /* ... */ }
  ```
  [`iterate`](Self::iterate), but reporting non-convergence as an error.

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
    fn clone(self: &Self) -> AccelerationScheme { /* ... */ }
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
    fn eq(self: &Self, other: &AccelerationScheme) -> bool { /* ... */ }
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
### Constants and Statics

#### Constant `DEFAULT_DIAGONAL_FACTOR`

Default regularisation added to the diagonal of the projection matrix —
upstream's `diagonalFactor`, default `1e-4`.

```rust
pub const DEFAULT_DIAGONAL_FACTOR: f64 = 1.0e-4;
```

#### Constant `DEFAULT_ALPHA`

Default blending factor between old and new snapshots — upstream's `alpha`,
default `1.0` (use the newer iterate of each pair).

```rust
pub const DEFAULT_ALPHA: f64 = 1.0;
```

## Module `hydrogen`

**Attributes:**

- `Other("#[allow(clippy::neg_cmp_op_on_partial_ord)]")`

Hydrogen pickup — how much of the corrosion hydrogen ends up in the metal.

# Why this matters more than the oxide itself

The corrosion reaction

```text
Zr + 2 H2O  ->  ZrO2 + 2 H2
```

liberates four hydrogen atoms for every zirconium atom consumed. Most of
that hydrogen leaves with the coolant, but a **pickup fraction** — 15% for
Zircaloy-4 and M5, up to 25% for ZIRLO, per upstream's own documentation —
diffuses into the cladding metal instead.

Zirconium dissolves only a little hydrogen (roughly 80–100 wt-ppm at
operating temperature, far less when cold). Above that solubility limit the
excess precipitates as **zirconium hydride** platelets, which are brittle.
A rod that has picked up 600 wt-ppm has a cladding whose fracture toughness
is a fraction of the fresh material's, and it is hydride embrittlement —
not wall thinning and not the temperature rise — that sets the practical
burnup limit for LWR fuel. This module is therefore the point at which
corrosion becomes a **failure** problem.

# The mass balance, in full

Everything in this module follows from one chain of conversions, and it is
written out here because every constant below is a link in it.

1. An oxide layer `ΔS` \[m\] thick has consumed `ΔS / 1.56` \[m\] of metal
   wall — the Pilling–Bedworth ratio,
   [`PILLING_BEDWORTH_ZIRCONIUM`].
2. Per mole of Zr consumed, 2 moles of H2 are released, i.e. `4·M_H` grams
   of hydrogen per `M_Zr` grams of zirconium. With upstream's atomic masses
   `M_H = 1.00784` and `M_Zr = 91.224`, that is a mass ratio of
   `4.4192e-2`.
3. A fraction `f` of it enters the metal.
4. Expressed as a concentration in the *whole* wall, the hydrogen mass per
   unit outer area is spread over the wall's volume per unit outer area —
   the reciprocal of the surface-to-volume ratio
   [`surface_to_volume`].
5. Multiplying by `1e6` turns the mass fraction into **wt-ppm**.

Collecting steps 1, 2 and 5 gives the single constant
[`HYDROGEN_PER_OXIDE_THICKNESS`] = `28328.13` wt-ppm·m, so that

`ΔC \[wt-ppm\] = 28328.13 · f · ΔS \[m\] · (A/V) \[1/m\]`.

# This is a wall average

Real hydrogen is not uniform: it is driven towards the *cold* side of the
wall by the Soret (thermal-diffusion) effect, so the outer rim of an
operating rod holds several times the average, and that rim is where
hydrides crack. Capturing that needs a hydrogen transport solve — upstream
has one, in `physicsSubSolvers/elementTransport/transportSolvers/hydrogenTransport/`,
which is outside this module's scope. **A wall average under-predicts the
peak local concentration**, and any embrittlement assessment built on the
numbers here inherits that non-conservatism.

# Units

Lengths \[m\], hydrogen concentration \[wt-ppm\], ingress flux
\[wt-ppm·m/s\]. The flux unit looks odd until you notice that
`flux × area × time / volume` must come out in wt-ppm; it is the natural
unit for a boundary condition on a wt-ppm-valued diffusion field, which is
exactly what upstream's `oxidePickupFraction` is.

# Status

AI-assisted translation, reviewed by no human. The tests below establish
that pickup is bounded by the hydrogen the reaction actually liberates and
that the algebra matches upstream's — they are **not** validation against
measured hydrogen data. One test pins an upstream defect in the optional
volume-scaling factor; see [`PickupScaling::UpstreamVolumeFactor`].

```rust
pub mod hydrogen { /* ... */ }
```

### Types

#### Enum `PickupScaling`

Whether the pickup is scaled by upstream's optional `volFactor`.

Upstream's `oxidePickupFraction` boundary condition has a `volFactor`
switch, documented as taking "into account the reduced volume of clad vs the
growing oxide layer", leading to "a more precise H flux". This enum makes
that switch explicit rather than a boolean flag, because the two options
differ by more than an order of magnitude and a reader must be able to see
which is in use.

```rust
pub enum PickupScaling {
    Uniform,
    UpstreamVolumeFactor,
}
```

##### Variants

###### `Uniform`

No volume scaling — upstream's `volFactor false`, which is its default.

The pickup is `f` times the hydrogen the reaction liberated, spread
uniformly over the as-fabricated wall. This is the option to use.

###### `UpstreamVolumeFactor`

Upstream's `volFactor true` scaling, **reproduced verbatim including its
defect**.

Upstream multiplies the ingress flux by

```text
volFactor = (2·r_o·S̄ − S̄²) / (2·r_o·(w − S̄) − w² + S̄²)
```

with `S̄` the mid-step mean oxide thickness and `w = r_o − r_i` the
as-fabricated wall thickness.

# UPSTREAM DEFECT, reproduced deliberately

Read as areas divided by π, the **denominator** is exactly
`(r_o − S̄)² − r_i²`, the cross-section of metal still remaining — which
is the right thing for the stated intent. The **numerator**, however, is
`r_o² − (r_o − S̄)²`, the cross-section of the *oxide*. The factor
upstream computes is therefore

`V_oxide / V_remaining_metal`,

whereas the correction it describes — "the reduced volume of clad" —
is `V_as_fabricated_metal / V_remaining_metal`, which needs the
as-fabricated metal area `r_o² − r_i²` in the numerator instead.

The two differ by **exactly one**: `intended = upstream + 1`, because
the as-fabricated metal area is the oxide area plus the remaining metal
area. That identity is asserted by a unit test, and it is what makes
this a demonstrable transcription error rather than an opinion about
which model is better.

Measured consequence for a 17×17 PWR rod (`r_i = 4.18` mm,
`r_o = 4.75` mm), this port, 2026-07-29:

| mean oxide \[µm\] | upstream factor | intended factor | ratio |
|---|---|---|---|
| 10 | 0.018998 | 1.018998 | 53.6 |
| 30 | 0.059114 | 1.059114 | 17.9 |
| 60 | 0.125207 | 1.125207 | 9.0 |
| 100 | 0.226501 | 1.226501 | 5.4 |

So `volFactor true` **suppresses** hydrogen pickup by between five- and
fifty-fold, most severely early in life, where the intended correction
is a few percent enhancement. Selecting this variant reproduces an
OFFBEAT run; it does not produce defensible hydrogen numbers. Prefer
[`Uniform`](Self::Uniform).

A second, smaller inconsistency is worth knowing if you are comparing
case files: upstream's two constructors disagree on the default radii
(`4.565`/`5.315` against `4.5`/`5.32`), and those magnitudes are
millimetres while its own usage documentation writes them in metres
(`0.004565`). They are overwritten from the dictionary whenever
`volFactor` is on, so nothing depends on them — but this port takes
**metres**, with no defaults.

##### Implementations

###### Methods

- ```rust
  pub fn factor(self: &Self, mean_oxide_thickness: f64, inner_radius: f64, outer_radius: f64) -> f64 { /* ... */ }
  ```
  The dimensionless multiplier this scaling applies to the ingress flux.

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
    fn clone(self: &Self) -> PickupScaling { /* ... */ }
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
    fn eq(self: &Self, other: &PickupScaling) -> bool { /* ... */ }
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
#### Enum `HydrogenPickupModel`

How much of the corrosion hydrogen enters the cladding metal.

One variant for "no pickup modelled" and one for upstream's
`oxidePickupFraction` boundary condition. Dispatch is by `match`, never by a
trait object, per the workspace `CLAUDE.md` "No trait objects" rule.

# Units

Radii \[m\], oxide thicknesses \[m\], results \[wt-ppm\] or
\[wt-ppm·m/s\].

```rust
pub enum HydrogenPickupModel {
    None,
    OxidePickupFraction {
        pickup_fraction: f64,
        inner_radius: f64,
        outer_radius: f64,
        scaling: PickupScaling,
    },
}
```

##### Variants

###### `None`

No hydrogen pickup is modelled — every result is exactly zero.

This is the state of an OFFBEAT case that runs corrosion but does not
put an `oxidePickupFraction` boundary condition on its hydrogen field,
which is the default. It is **not** a statement that no hydrogen is
picked up in reality; it is a statement that this run does not track it.

###### `OxidePickupFraction`

Upstream `oxidePickupFraction` — a fixed fraction of the liberated
hydrogen enters the metal.

`ingress flux \[wt-ppm·m/s\] = 1e6 · 4 · f / 1.56 · (M_H/M_Zr) ·
dS/dt · volFactor`

which is upstream's expression exactly, and equals
[`HYDROGEN_PER_OXIDE_THICKNESS`] `· f · dS/dt · volFactor`.

# Assumptions and limitations

- The pickup fraction is **constant** — independent of temperature,
  burnup, oxide thickness and alloy chemistry. Real pickup fractions
  drift over life, so this is a life-average value, and matching a
  measured end-of-life hydrogen content by tuning `f` conceals that.
- The result is a **wall average**; see the
  [module documentation](self) on the Soret effect.
- No hydrogen ever leaves. There is no desorption term and no
  solubility limit here.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `pickup_fraction` | `f64` | Fraction of the liberated hydrogen that enters the metal \[-\], in<br>`[0, 1]` — upstream's `pickupFraction`, default `0.15`.<br><br>Upstream's own guidance, from<br>`oxidePickupFractionFvPatchScalarField.H`: Zircaloy-4 `0.15`–`0.2`;<br>ZIRLO and optimized ZIRLO `0.25`; M5 `0.15`.<br><br>Values outside `[0, 1]` are unphysical — more than all the hydrogen<br>cannot be absorbed — and are rejected by<br>[`pickup_checked`](HydrogenPickupModel::pickup_checked). |
| `inner_radius` | `f64` | Cladding inner radius \[**m**\] — upstream's `rInner`.<br><br>Note the unit: metres, matching upstream's own usage documentation<br>(`0.004565`), not the millimetre-magnitude numbers in upstream's<br>constructor defaults. About `4.18e-3` m for a 17×17 PWR rod. |
| `outer_radius` | `f64` | Cladding outer radius \[**m**\] — upstream's `rOuter`. About<br>`4.75e-3` m for a 17×17 PWR rod. Must exceed `inner_radius`. |
| `scaling` | `PickupScaling` | Whether upstream's optional volume scaling is applied. See<br>[`PickupScaling`] — and read<br>[`UpstreamVolumeFactor`](PickupScaling::UpstreamVolumeFactor)'s<br>documentation before selecting it. |

##### Implementations

###### Methods

- ```rust
  pub fn zircaloy_4(inner_radius: f64, outer_radius: f64) -> Self { /* ... */ }
  ```
  Upstream's default Zircaloy-4 pickup on a 17×17 PWR rod: a 15% pickup

- ```rust
  pub fn pickup(self: &Self, oxide_thickness_before: f64, oxide_growth: f64) -> f64 { /* ... */ }
  ```
  Increase in the wall-average hydrogen concentration \[wt-ppm\] caused by

- ```rust
  pub fn ingress_flux(self: &Self, oxide_thickness_before: f64, oxide_growth: f64, time_step: f64) -> f64 { /* ... */ }
  ```
  Hydrogen ingress flux \[wt-ppm·m/s\] through the cladding outer surface

- ```rust
  pub fn pickup_checked(self: &Self, oxide_thickness_before: f64, oxide_growth: f64) -> Result<f64> { /* ... */ }
  ```
  [`pickup`](Self::pickup), but returning an error instead of

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
    fn clone(self: &Self) -> HydrogenPickupModel { /* ... */ }
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
    fn eq(self: &Self, other: &HydrogenPickupModel) -> bool { /* ... */ }
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

#### Function `surface_to_volume`

**Attributes:**

- `MustUse { reason: None }`

Outer-surface area per unit metal volume \[1/m\] of a cylindrical cladding
tube of inner radius `inner_radius` and outer radius `outer_radius` \[m\].

`A/V = 2·r_o / (r_o² − r_i²)`.

This is the geometric factor that turns a hydrogen **flux through the outer
surface** into a **concentration change in the wall**. Upstream never writes
it down, because its finite-volume discretisation gets it for free from the
real face areas and cell volumes; this port needs it explicitly.

# Thin-wall limit

For a thin wall `A/V → 1/(r_o − r_i)`, i.e. one over the wall thickness. The
exact expression is larger, because the outer surface is bigger than the
inner one. For a 17×17 PWR rod (`r_i = 4.18` mm, `r_o = 4.75` mm) the exact
value is `1866.4` 1/m against a thin-wall `1754.4` 1/m — **6.4% apart**,
which is enough to matter and small enough to hide, so the exact form is
used.

# Degenerate input

Returns `0.0` if `outer_radius <= inner_radius` or either radius is
negative, rather than a negative or infinite number. A zero surface-to-volume
ratio makes every downstream pickup zero, which is the only answer that
cannot invent hydrogen.

```
use outram_park_fork_offbeat::corrosion::hydrogen::surface_to_volume;

let av = surface_to_volume(4.18e-3, 4.75e-3);
assert!((av - 1866.4).abs() < 0.1);
// Always at least the thin-wall value.
assert!(av > 1.0 / (4.75e-3 - 4.18e-3));
```

```rust
pub fn surface_to_volume(inner_radius: f64, outer_radius: f64) -> f64 { /* ... */ }
```

#### Function `hydrogen_liberated`

**Attributes:**

- `MustUse { reason: None }`

Hydrogen \[wt-ppm\] that the corrosion reaction **liberates** while growing
`oxide_growth` \[m\] of oxide on a tube of the given radii \[m\], expressed
as a wall-average concentration.

This is the total released by `Zr + 2 H2O -> ZrO2 + 2 H2`, *before* any
pickup fraction is applied — i.e. the hard upper bound on
[`HydrogenPickupModel::pickup`]. Nothing in this module may exceed it, and a
unit test asserts exactly that.

Returns `0.0` for a non-positive growth or a degenerate geometry.

```
use outram_park_fork_offbeat::corrosion::hydrogen::hydrogen_liberated;

// 60 um of oxide on a 17x17 PWR rod.
let total = hydrogen_liberated(6.0e-5, 4.18e-3, 4.75e-3);
assert!((total - 3172.2).abs() < 0.5);
```

```rust
pub fn hydrogen_liberated(oxide_growth: f64, inner_radius: f64, outer_radius: f64) -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `HYDROGEN_ATOMIC_MASS`

Atomic mass of hydrogen \[g/mol\] — upstream's `MH_ = 1.00784`.

```rust
pub const HYDROGEN_ATOMIC_MASS: f64 = 1.00784;
```

#### Constant `ZIRCONIUM_ATOMIC_MASS`

Atomic mass of zirconium \[g/mol\] — upstream's `MZr_ = 91.224`.

```rust
pub const ZIRCONIUM_ATOMIC_MASS: f64 = 91.224;
```

#### Constant `HYDROGEN_ATOMS_PER_ZIRCONIUM`

Hydrogen atoms liberated per zirconium atom consumed \[-\].

Four, from `Zr + 2 H2O -> ZrO2 + 2 H2`. Upstream writes the bare `4` in its
expression; naming it makes the stoichiometry visible.

```rust
pub const HYDROGEN_ATOMS_PER_ZIRCONIUM: f64 = 4.0;
```

#### Constant `HYDROGEN_PER_OXIDE_THICKNESS`

Hydrogen liberated per metre of oxide grown, per metre of wall it is spread
over \[wt-ppm·m\].

`1e6 · 4 · M_H / (M_Zr · 1.56) = 28328.13`. This is the whole mass balance
of the [module documentation](self) collapsed into one number: with a 100%
pickup fraction, growing `ΔS` of oxide on a wall of surface-to-volume ratio
`A/V` adds `28328.13 · ΔS · A/V` wt-ppm of hydrogen.

For scale: a typical PWR wall (`A/V = 1866` 1/m) growing 60 µm of oxide
liberates `3172` wt-ppm, of which 15% — `476` wt-ppm — is picked up.

```rust
pub const HYDROGEN_PER_OXIDE_THICKNESS: f64 = _;
```

## Module `kinetics`

**Attributes:**

- `Other("#[allow(clippy::neg_cmp_op_on_partial_ord)]")`

Oxide-growth kinetics — how thick the ZrO2 layer is after a timestep \[m\].

# What these correlations compute

Each variant of [`OxidationKinetics`] answers one question: given the oxide
thickness `S0` \[m\] at the start of a timestep, the temperature at the
**metal/oxide interface** `T` \[K\], the fast-neutron flux, and the step
length `dt` \[s\], what is the thickness `S` \[m\] at the end of the step?

They are integrated forms, not rate equations: the correlation is written as
a closed-form growth law and evaluated over the whole step, so a caller does
not need a sub-stepping ODE integrator. [`thickness`] returns the new
thickness; [`growth`] returns the increment; [`growth_rate`] returns the
increment divided by `dt`, i.e. the **mean** rate over the step, not the
instantaneous rate at either end.

[`thickness`]: OxidationKinetics::thickness
[`growth`]: OxidationKinetics::growth
[`growth_rate`]: OxidationKinetics::growth_rate

# Which temperature

`T` is the **metal/oxide interface** temperature, not the coolant
temperature and not the oxide's outer-surface temperature. Corrosion is
controlled by diffusion through the oxide, which is anchored at the metal
face. As the oxide thickens it insulates, so the interface runs hotter than
the surface and the reaction accelerates. Upstream computes the interface
temperature in `zircaloyOuterCorrosion::correct`; that calculation is ported
separately in [`super::thermal`], and its result is what should be fed here.

# Units

- thickness \[m\], timestep \[s\], temperature \[K\]
- fast flux \[n/(m²·s)\] — **SI**, converted to upstream's n/(cm²·s) basis
  inside the correlation. See the [module documentation](super) for why this
  conversion is done once at the boundary.

# Validity ranges: `thickness` extrapolates, `thickness_checked` refuses

[`thickness`](OxidationKinetics::thickness) evaluates the correlation
wherever it is asked, matching upstream, which enforces nothing.
[`thickness_checked`](OxidationKinetics::thickness_checked) returns
[`OffbeatError::OutOfRange`] outside the variant's stated temperature range,
[`OffbeatError::Unphysical`] for a negative thickness, timestep or flux, and
— uniquely — also refuses the 1800–1900 K window of
[`CathcartPawel`](OxidationKinetics::CathcartPawel), because upstream's
expression there is arithmetically broken. See that variant's documentation.

# Reference

Upstream attributes the constants of both correlations to:

> Dunbar et al., *Fuel performance analysis of Cr-coated Zircaloy-4 cladding
> during a prototypical LOCA event using BISON*, Annals of Nuclear Energy
> **200** (2024) 110411. <https://doi.org/10.1016/j.anucene.2024.110411>

This port has **not** independently checked the constants against that
paper; it reproduces the values in upstream's source. Do not cite it as
agreement with the reference.

[`OffbeatError::OutOfRange`]: crate::error::OffbeatError::OutOfRange
[`OffbeatError::Unphysical`]: crate::error::OffbeatError::Unphysical

```rust
pub mod kinetics { /* ... */ }
```

### Types

#### Enum `OxidationKinetics`

Oxide-growth kinetics for Zircaloy in water or steam.

One variant per oxidation-kinetics model compiled by upstream OFFBEAT's
`oxidationKineticsModel/`. Each variant's documentation names the upstream
class and the string a user writes in the `oxidationKineticsModel` entry of
a case dictionary, so an OFFBEAT case can be translated variant by variant.

Dispatch is by `match`, never by a trait object, per the workspace
`CLAUDE.md` "No trait objects" rule.

# All variants are monotonic and start from `S0`

Every branch has the form "new thickness = f(old thickness, T, dt)" with
`f >= S0` for `dt >= 0`. Oxide does not un-grow; a caller integrating a
power history can chain [`thickness`](Self::thickness) step by step and the
result is monotonically non-decreasing. The unit tests assert this.

```rust
pub enum OxidationKinetics {
    EpriKwuCe,
    CathcartPawel,
    EpriKwuCeCathcartPawel,
}
```

##### Variants

###### `EpriKwuCe`

Low-temperature (normal-operation) waterside oxidation — upstream
`EPRI_KWU_CE`, `ClassName("EPRI-KWU-CE")`.

The industry-standard two-regime LWR corrosion law.

**Sub-transition (`S <= 2 µm`), cubic:**

`S = (C1 · exp(−Q1/(R·T)) · dt_days + S0³)^(1/3)`

with `C1 = 6.3e-9` m³/day and `Q1/R = 16266.10` K. Cubic in thickness
means the *rate* falls as `1/S²`: the layer protects itself.

**Post-transition (`S > 2 µm`), linear:**

`S = S0 + C2(φ) · exp(−Q2/(R·T)) · dt_days`

with `Q2/R = 13775.16` K and a flux-enhanced rate constant

`C2(φ) = (80.4 + 259 · (7.46e-15 · φ_cm)^(1/4))` m/day,

`φ_cm` being the fast flux in n/(cm²·s) — this port converts from the
SI n/(m²·s) it takes. The quarter-power dependence is the empirical
signature of irradiation-enhanced corrosion: fast neutrons damage the
oxide and speed up transport through it. At zero flux `C2 = 80.4` m/day;
at a typical PWR fast flux of `7e17` n/(m²·s) it is `300.57` m/day, i.e.
irradiation multiplies the post-transition rate by 3.74.

**Crossing the transition inside one step.** If `S0` is below 2 µm but
the cubic law would carry it past, upstream splits the step: it finds
the fraction of `dt` needed to reach exactly 2 µm by **linear
interpolation** between `S0` and the cubic end-point, then applies the
post-transition law for whatever is left. That interpolation is an
approximation (the underlying law is cubic, not linear, in time), and it
is reproduced here exactly. The resulting thickness is nevertheless a
*continuous* function of `S0` and `dt` across the transition — the
tests check this to 1e-12 relative.

# Branch selection differs subtly from upstream, deliberately

Upstream selects its branch on the **current outer-iteration estimate**
of `S`, which it receives by non-`const` reference, not on `S0`. That
makes its answer depend on solver iteration state, which a pure function
cannot have. This port instead evaluates the branch that upstream's
iteration **converges to**, which is well defined:

- `S0 >= 2 µm` → post-transition;
- else if the cubic result is `<= 2 µm` → sub-transition;
- else → the crossing branch.

This is not an approximation of upstream: it is upstream's fixed point,
reached on its second outer iteration in every case. It differs from
upstream only if a run stops after a single outer iteration of a step
that crosses the transition.

# Validity

Upstream declares `lowerLimit() = 500` K and `upperLimit() = 673` K.
Note that upstream's own combined model **never consults the lower
limit** — see [`EpriKwuCeCathcartPawel`](Self::EpriKwuCeCathcartPawel) —
so below 500 K upstream silently extrapolates. This port's
[`thickness_checked`](Self::thickness_checked) enforces 500–673 K.

###### `CathcartPawel`

High-temperature (accident) steam oxidation — upstream `CathcartPawel`,
`ClassName("Cathcart-Pawel")`.

A **parabolic** law, appropriate above ~673 K where the oxide is no
longer protective and growth is limited by oxygen diffusion:

`S = sqrt(A · exp(−Q/(R·T)) · dt + S0²)`

with `dt` in **seconds** (unlike
[`EpriKwuCe`](Self::EpriKwuCe), which works in days) and three
temperature branches:

| Range | `A` \[m²/s\] | `Q/R` \[K\] | Source named upstream |
|---|---|---|---|
| `T < 1800 K` | `7.82e-6` | `20214` | Leistikow |
| `1800 <= T < 1900 K` | interpolated | interpolated | "Procedure from G. Schanz — 2003" |
| `T >= 1900 K` | `2.98e-3` | `28420` | Prater–Courtright |

The 1800–1900 K window exists because Zircaloy undergoes a phase change
there and neither fit applies; Schanz's procedure is to fit a single
Arrhenius law through the two branch values at the window edges.

Note that upstream names the class `CathcartPawel` but the constants it
actually contains are attributed in its own comments to Leistikow and
Prater–Courtright, which are different correlations. This port keeps
upstream's class name for traceability and flags the mismatch rather
than silently renaming it.

# UPSTREAM DEFECT in the 1800–1900 K window, reproduced deliberately

Upstream's interpolation branch is arithmetically broken, in two
independent ways, and this port reproduces it verbatim so that a
comparison against an OFFBEAT run is possible. **The values it produces
are not physical and must not be used.**

1. **Missing parentheses.** Upstream writes
   `log(k2 / 7.82e-6 * exp(-20214/1800))` where
   `log(k2 / (7.82e-6 * exp(-20214/1800)))` was intended: C++ evaluates
   `a / b * c` as `(a/b)*c`, so the Leistikow exponential is
   **multiplied** instead of divided. The activation temperature comes
   out as `−692375.6` K instead of `+75756.4` K — negative, so the rate
   *decreases* with temperature inside the window, the opposite of an
   Arrhenius law.
2. **Wrong pre-exponential.** Even with the parentheses fixed, upstream
   forms `A = 7.82e-6 · exp(Q/R/1900)`, using the bare Leistikow
   pre-exponential where the Leistikow *rate constant* at 1800 K
   (`1.0377e-10` m²/s) belongs. That is a further factor of 8224.7.

Measured consequence (this port, 2026-07-29): at 1850 K the effective
rate constant is `1.4809e-1` m²/s, against `2.9e-10` m²/s for a sane
interpolation between the two branches — about nine orders of magnitude
too large. Starting from bare metal, one second of it gives 0.38 **m**
of oxide.

[`thickness`](Self::thickness) reproduces this. **[`thickness_checked`]
refuses it** with [`OffbeatError::Unphysical`], and a unit test pins the
defective numbers so that anyone fixing it upstream is forced to notice.

# Validity

Upstream declares `lowerLimit() = 673` K and `upperLimit() = GREAT`
(unbounded). This port's checked path enforces 673 K to 2500 K — above
roughly 2245 K the ZrO2 itself melts and no solid-layer growth law
applies — and additionally rejects the 1800–1900 K window as described
above.

[`thickness_checked`]: Self::thickness_checked
[`OffbeatError::Unphysical`]: crate::error::OffbeatError::Unphysical

###### `EpriKwuCeCathcartPawel`

The combined operational-plus-accident model — upstream
`lowHighOxidationKineticsModel<EPRI_KWU_CE, CathcartPawel>`, registered
as `"EPRI-KWU-CE|Cathcart-Pawel"`.

This is the variant a realistic LWR case selects, and the only
instantiation of the templated low/high model that upstream actually
compiles. It applies [`EpriKwuCe`](Self::EpriKwuCe) below
[`LOW_HIGH_SWITCH_TEMPERATURE`] (673 K) and
[`CathcartPawel`](Self::CathcartPawel) at or above it, per interface
temperature, so one rod can be in normal-operation kinetics at its
cold end and accident kinetics at a hot spot.

# The switch is a jump, not a blend

The two laws are independent fits with different functional forms
(cubic/linear against parabolic), and upstream makes **no attempt** to
match them at 673 K. There is a genuine discontinuity in the growth rate
there. Measured at 673 K over a 1 s step from a 2 µm layer, with zero
flux (this port, 2026-07-29): the EPRI/KWU/C-E branch grows
`1.2008e-12` m and the Cathcart–Pawel branch `1.7653e-13` m — the rate
**drops by a factor of 6.80** on crossing into the "accident" model.
That is the opposite of the naive expectation, and it happens because
the parabolic law is strongly self-limiting once a 2 µm layer already
exists (its rate goes as `1/S`), whereas the post-transition linear law
does not slow down at all. A unit test pins that ratio; it **documents**
the discontinuity rather than endorsing it, and a caller crossing 673 K
slowly should expect a visible kink in the oxide history.

# Upstream never uses `EPRI_KWU_CE::lowerLimit()`

Upstream's dispatcher tests only `T < lowTModel.upperLimit()`. The
declared lower limit of 500 K is dead code, so an OFFBEAT run at, say,
400 K quietly extrapolates the low-temperature fit. This port's
[`thickness`](Self::thickness) does the same for fidelity;
[`thickness_checked`](Self::thickness_checked) enforces 500 K.

##### Implementations

###### Methods

- ```rust
  pub fn thickness(self: &Self, previous_thickness: f64, interface_temperature: f64, fast_flux: f64, time_step: f64) -> f64 { /* ... */ }
  ```
  Oxide thickness \[m\] at the end of a timestep.

- ```rust
  pub fn growth(self: &Self, previous_thickness: f64, interface_temperature: f64, fast_flux: f64, time_step: f64) -> f64 { /* ... */ }
  ```
  Increase in oxide thickness \[m\] over the step, i.e.

- ```rust
  pub fn growth_rate(self: &Self, previous_thickness: f64, interface_temperature: f64, fast_flux: f64, time_step: f64) -> f64 { /* ... */ }
  ```
  **Mean** oxide growth rate \[m/s\] over the step — the increment divided

- ```rust
  pub fn thickness_checked(self: &Self, previous_thickness: f64, interface_temperature: f64, fast_flux: f64, time_step: f64) -> Result<f64> { /* ... */ }
  ```
  [`thickness`](Self::thickness), but returning an error instead of

- ```rust
  pub fn growth_checked(self: &Self, previous_thickness: f64, interface_temperature: f64, fast_flux: f64, time_step: f64) -> Result<f64> { /* ... */ }
  ```
  [`growth`](Self::growth), but returning an error instead of

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
    fn clone(self: &Self) -> OxidationKinetics { /* ... */ }
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
    fn eq(self: &Self, other: &OxidationKinetics) -> bool { /* ... */ }
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
### Constants and Statics

#### Constant `TRANSITION_THICKNESS`

Oxide thickness \[m\] at which sub-transition (cubic) kinetics give way to
post-transition (linear) kinetics — upstream's `S_trans = 2e-6`.

Physically this is where the dense, protective oxide cracks and stops being
an effective diffusion barrier. It is a fitted constant, not a measured
property of a particular rod.

```rust
pub const TRANSITION_THICKNESS: f64 = 2.0e-6;
```

#### Constant `LOW_HIGH_SWITCH_TEMPERATURE`

Temperature \[K\] at which the combined low/high model hands over from
EPRI/KWU/C-E to Cathcart–Pawel — upstream's `EPRI_KWU_CE::upperLimit()`.

```rust
pub const LOW_HIGH_SWITCH_TEMPERATURE: f64 = 673.0;
```

## Module `model`

The patch-level corrosion model: kinetics plus metal loss plus hydrogen.

```rust
pub mod model { /* ... */ }
```

### Types

#### Enum `CorrosionModel`

A complete waterside-corrosion model for one cladding surface.

One variant per corrosion model registered in upstream OFFBEAT's
`corrosionModel` run-time selection table. Each variant's documentation
names the upstream class and the string a user writes in a case dictionary,
so an OFFBEAT case can be translated variant by variant.

Dispatch is by `match`, never by a trait object, per the workspace
`CLAUDE.md` "No trait objects" rule.

# What a corrosion model owns

Three coupled results, delivered together as a [`CorrosionStep`]:

1. **Oxide thickness** \[m\] and its increment — from the
   [`OxidationKinetics`] this model carries.
2. **Metal loss** \[m\] — the increment divided by the Pilling–Bedworth
   ratio 1.56, i.e. the inward wall displacement a moving-mesh driver must
   apply.
3. **Hydrogen pickup** \[wt-ppm\] — from the [`HydrogenPickupModel`] this
   model carries.

# What it does not own

The mesh. Upstream's corrosion model additionally rewrites the boundary
thermal conductivity in place and drives a topology changer that adds and
removes cell layers; neither ports without a live mesh. The thermal
calculation is available as a pure function in [`super::thermal`], and the
topology changer is deferred — see the [module documentation](super).

# Units

Raw `f64`, strict SI, except hydrogen in wt-ppm. See [`CorrosionState`] and
[`CorrosionStep`].

```rust
pub enum CorrosionModel {
    Constant,
    ZircaloyOuter {
        kinetics: super::kinetics::OxidationKinetics,
        hydrogen: super::hydrogen::HydrogenPickupModel,
    },
}
```

##### Variants

###### `Constant`

The oxide layer does not evolve — upstream `corrosionModel`,
`TypeName("fromLatestTime")`, and equivalently the `corrosion` base
class, `TypeName("constant")`.

Every step returns [`CorrosionStep::unchanged`]: whatever oxide was
there stays there, no metal is consumed, no hydrogen is picked up. This
is the **default** in an OFFBEAT case that does not ask for corrosion,
and it is what a case uses when it wants to *impose* a fixed oxide
profile read from a file rather than compute one.

It is not "no oxide": a non-zero
[`CorrosionState::oxide_thickness`] is carried through unchanged, so
the layer's thermal effect via [`super::thermal`] still applies.

Upstream notes that `fromLatestTime` is deprecated in favour of
`constant`; both name the same do-nothing behaviour, and this port
carries one variant for both.

###### `ZircaloyOuter`

Zircaloy outer-surface (waterside) corrosion — upstream
`zircaloyOuterCorrosion`, `TypeName("zircaloyOuterCorrosion")`.

The real model. It grows the oxide with the [`OxidationKinetics`] given,
converts the growth to metal loss with the Pilling–Bedworth ratio
(upstream's `updateDMetalThickness`, which is literally
`DS_metal = DS_oxide / 1.56`), and — going one step beyond upstream's
corrosion class, which leaves this to a separate boundary condition —
computes the hydrogen that goes into the metal.

# This is the *outer* surface only

Upstream models waterside corrosion on the cladding's coolant-facing
surface. Inner-surface (fuel-side) oxidation, which consumes the small
amount of oxygen released by the fuel and by residual moisture, is a
different and much slower process, and neither upstream nor this port
models it.

# Choosing the parts

- `kinetics` — for a realistic LWR case use
  [`OxidationKinetics::EpriKwuCeCathcartPawel`], which is the only
  combined model upstream compiles. The single-regime variants are for
  studying one branch in isolation.
- `hydrogen` — [`HydrogenPickupModel::None`] to skip hydrogen entirely,
  or [`HydrogenPickupModel::zircaloy_4`] for upstream's defaults.

See [`CorrosionModel::zircaloy_outer_default`] for the usual
combination.

[`CorrosionState::oxide_thickness`]: super::state::CorrosionState::oxide_thickness

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `kinetics` | `super::kinetics::OxidationKinetics` | Which oxide-growth law to use. |
| `hydrogen` | `super::hydrogen::HydrogenPickupModel` | Which hydrogen-pickup model to use, if any. |

##### Implementations

###### Methods

- ```rust
  pub fn zircaloy_outer_default() -> Self { /* ... */ }
  ```
  Zircaloy waterside corrosion with the combined

- ```rust
  pub fn with_hydrogen_pickup(self: Self, hydrogen: HydrogenPickupModel) -> Self { /* ... */ }
  ```
  The same model with a hydrogen-pickup submodel attached.

- ```rust
  pub fn step(self: &Self, state: &CorrosionState) -> CorrosionStep { /* ... */ }
  ```
  Advance one boundary face by one timestep.

- ```rust
  pub fn step_checked(self: &Self, state: &CorrosionState) -> Result<CorrosionStep> { /* ... */ }
  ```
  [`step`](Self::step), but returning an error instead of extrapolating.

- ```rust
  pub fn metal_loss(oxide_growth: f64) -> f64 { /* ... */ }
  ```
  Metal wall thickness \[m\] consumed to grow `oxide_growth` \[m\] of

- ```rust
  pub fn kinetics(self: &Self) -> Option<OxidationKinetics> { /* ... */ }
  ```
  The oxidation kinetics this model uses, if it grows oxide at all.

- ```rust
  pub fn hydrogen_pickup(self: &Self) -> HydrogenPickupModel { /* ... */ }
  ```
  The hydrogen-pickup model this corrosion model carries.

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
    fn clone(self: &Self) -> CorrosionModel { /* ... */ }
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
    fn eq(self: &Self, other: &CorrosionModel) -> bool { /* ... */ }
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
## Module `state`

Inputs and results of one corrosion step, for **one boundary face**.

```rust
pub mod state { /* ... */ }
```

### Types

#### Struct `CorrosionState`

Everything a waterside-corrosion model needs to advance **one boundary
face** by one timestep.

# Why this type exists

Upstream OFFBEAT's corrosion model reaches into the OpenFOAM mesh registry
and looks fields up by name — `"T"`, `"fastFlux"`, `"k"` — plus the
timestep from the `Time` object and the previous thickness from a stored
old-time field. Its dependencies are invisible until you read the body, and
a missing field is a runtime failure. This port inverts that: a corrosion
model takes a `CorrosionState`, so its inputs are visible in the signature
and the compiler checks that they exist.

# Units — raw `f64`, strict SI

Evaluated once per boundary face per timestep, so raw `f64` rather than
`uom` quantities. One field is **not** what a reader of upstream would
expect, and is called out because getting it wrong is silent:

- [`fast_flux`](Self::fast_flux) is in **n/(m²·s)**, whereas upstream's
  `fastFlux` field is in n/(cm²·s). The conversion happens once, inside the
  correlation.

# This is the metal/oxide **interface** temperature

[`interface_temperature`](Self::interface_temperature) is the temperature at
the metal/oxide boundary, not at the oxide's outer surface and not in the
coolant. Use [`oxide_thermal_coupling`](super::thermal::oxide_thermal_coupling)
to obtain it from a surface temperature, a first-cell temperature and the
current oxide thickness. Feeding the surface temperature here instead
**underestimates** corrosion, increasingly so as the layer thickens, because
the insulating oxide makes the interface the hotter of the two.

```rust
pub struct CorrosionState {
    pub interface_temperature: f64,
    pub oxide_thickness: f64,
    pub fast_flux: f64,
    pub time_step: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `interface_temperature` | `f64` | Metal/oxide interface temperature \[K\]. Absolute; must be > 0.<br><br>Typical PWR cladding runs 570–620 K at the interface early in life and<br>climbs as the oxide insulates. |
| `oxide_thickness` | `f64` | Oxide-layer thickness \[m\] at the **start** of the timestep — upstream's<br>`oxideThickness.oldTime()`.<br><br>Zero for fresh cladding. A full-life PWR rod reaches 40–100 µm<br>(`4e-5`–`1e-4` m); regulatory limits are typically around 100 µm. |
| `fast_flux` | `f64` | Fast-neutron flux \[**n/(m²·s)**\], conventionally E > 1 MeV.<br><br>Note the unit: SI, not the n/(cm²·s) upstream's field carries. A<br>representative PWR value is `7e17` n/(m²·s) (= 7e13 n/(cm²·s)).<br><br>Only the post-transition branch of the low-temperature kinetics uses<br>this; the high-temperature branch ignores it entirely. |
| `time_step` | `f64` | Timestep \[s\] to advance over.<br><br>Fuel-performance timesteps span an enormous range — a few seconds<br>through a power ramp, days or weeks through steady irradiation. The<br>kinetics are integrated in closed form over the whole step, so a long<br>step is accurate as long as the interface temperature really is roughly<br>constant across it. |

##### Implementations

###### Methods

- ```rust
  pub fn fresh(interface_temperature: f64, time_step: f64) -> Self { /* ... */ }
  ```
  Fresh cladding — zero oxide — at `interface_temperature` \[K\], with no

- ```rust
  pub fn advanced(self: &Self, step: &CorrosionStep) -> Self { /* ... */ }
  ```
  A copy of this state advanced to the end of `step`, ready for the next

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
    fn clone(self: &Self) -> CorrosionState { /* ... */ }
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
    fn eq(self: &Self, other: &CorrosionState) -> bool { /* ... */ }
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
#### Struct `CorrosionStep`

The result of advancing one boundary face by one corrosion timestep.

Corresponds to the three surface fields upstream's `corrosion` class owns —
`oxideThickness`, `DOxideThickness` and `DMetalThickness` — plus the
hydrogen ingress that upstream's `oxidePickupFraction` boundary condition
computes from the first two.

# Units — raw `f64`, strict SI except hydrogen

Lengths in metres. [`hydrogen_pickup`](Self::hydrogen_pickup) is in
**wt-ppm**, matching
[`MaterialState::hydrogen_content`](crate::materials::MaterialState::hydrogen_content) —
a mass fraction times 1e6, which is the unit the hydride literature uses.

```rust
pub struct CorrosionStep {
    pub oxide_thickness: f64,
    pub oxide_growth: f64,
    pub metal_loss: f64,
    pub hydrogen_pickup: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `oxide_thickness` | `f64` | Oxide-layer thickness \[m\] at the **end** of the step — upstream's<br>`oxideThickness`. |
| `oxide_growth` | `f64` | Increase in oxide thickness \[m\] over the step — upstream's<br>`DOxideThickness`. Always `>= 0`. |
| `metal_loss` | `f64` | Metal wall thickness \[m\] consumed over the step — upstream's<br>`DMetalThickness`, equal to<br>[`oxide_growth`](Self::oxide_growth) divided by the Pilling–Bedworth<br>ratio 1.56. Always `>= 0`.<br><br>**This is the number a moving-mesh driver needs.** Upstream displaces<br>the boundary points inward by exactly this much each step. This port<br>does not own a mesh — see the [module documentation](super) on the<br>deferred layer addition/removal topology changer — so it reports the<br>displacement and leaves applying it to the caller. |
| `hydrogen_pickup` | `f64` | Increase in the wall-average hydrogen concentration \[wt-ppm\] over the<br>step.<br><br>Zero when the model carries no hydrogen-pickup submodel. Bounded above<br>by the hydrogen the reaction actually liberated — see<br>[`super::hydrogen`] — because the pickup fraction is a fraction. |

##### Implementations

###### Methods

- ```rust
  pub fn unchanged(oxide_thickness: f64) -> Self { /* ... */ }
  ```
  A step in which nothing happened: no growth, no metal loss, no pickup,

- ```rust
  pub fn apply_to(self: &Self, state: &mut MaterialState) { /* ... */ }
  ```
  Add this step's [`hydrogen_pickup`](Self::hydrogen_pickup) to a cell's

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
    fn clone(self: &Self) -> CorrosionStep { /* ... */ }
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
    fn eq(self: &Self, other: &CorrosionStep) -> bool { /* ... */ }
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
## Module `thermal`

**Attributes:**

- `Other("#[allow(clippy::neg_cmp_op_on_partial_ord)]")`

What the oxide layer does to heat transfer.

# Why corrosion is a thermal problem, not only a chemical one

Zirconia conducts heat about sixteen times worse than the Zircaloy it grows
on — roughly 0.94 W/(m·K) against 15. A 100 µm oxide layer is thin compared
with a 600 µm cladding wall, but its thermal resistance is nearly three
times the whole wall's, so it raises the temperature of
everything inside it — the metal, the gap, and the fuel. Since the oxidation
rate is Arrhenius in the metal/oxide **interface** temperature, a thicker
oxide makes the interface hotter, which makes the oxide grow faster. The
loop is closed, and this module is the part of it that turns thickness into
temperature.

# What upstream does, and what this module ports

Upstream OFFBEAT does not mesh the oxide layer. Instead, in
`zircaloyOuterCorrosion::correct`, it *modifies the boundary thermal
conductivity* of the outermost metal cell so that the same finite-volume
discretisation reproduces the extra resistance, and reconstructs the
interface temperature from the blend. That calculation is a pure function of
five scalars, so it ports cleanly, and it is here.

Everything around it — the surface fields, the mesh registry lookups, the
`const_cast` write-back into the `k` patch field — does not port and is not
attempted; see the [module documentation](super).

# Units

Temperatures \[K\], conductivities \[W/(m·K)\], lengths \[m\]. Raw `f64`,
strict SI.

```rust
pub mod thermal { /* ... */ }
```

### Types

#### Struct `OxideThermalCoupling`

How an oxide layer couples the metal to its outer surface, thermally.

The result of [`oxide_thermal_coupling`]. All three fields come out of the
same blending factor, and are returned together because a caller reproducing
upstream needs both the temperature (to drive the kinetics) and the modified
conductivity (to feed back into the heat-conduction solve).

```rust
pub struct OxideThermalCoupling {
    pub interface_temperature: f64,
    pub boundary_conductivity: f64,
    pub blending_factor: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `interface_temperature` | `f64` | Metal/oxide interface temperature \[K\] — **this is what the oxidation<br>kinetics must be evaluated at**.<br><br>Lies between the oxide's outer-surface temperature and the first metal<br>cell's temperature, and moves towards the *cell* temperature (hotter, in<br>an operating rod) as the oxide thickens. |
| `boundary_conductivity` | `f64` | Effective boundary thermal conductivity \[W/(m·K)\] that reproduces the<br>oxide's resistance without meshing it — upstream writes this back into<br>the `k` patch field.<br><br>Always less than the metal conductivity passed in, and it falls as the<br>oxide thickens. That reduction *is* the insulating effect. |
| `blending_factor` | `f64` | The blending factor `β` \[-\], in `[0, 1]`.<br><br>`β = α_ox / (α_ox + α_m)`, the oxide's share of the total conductance.<br>`β = 1` means no oxide at all (the interface is the surface);<br>`β → 0` means an oxide so resistive that the interface sits at the metal<br>cell temperature. Exposed because it is the single number that says how<br>much the oxide matters at this face. |

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
    fn clone(self: &Self) -> OxideThermalCoupling { /* ... */ }
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
    fn eq(self: &Self, other: &OxideThermalCoupling) -> bool { /* ... */ }
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

#### Function `oxide_conductivity`

**Attributes:**

- `MustUse { reason: None }`

Thermal conductivity of the zirconia layer \[W/(m·K)\] at `temperature`
\[K\].

Upstream's hard-coded linear fit, from `zircaloyOuterCorrosion.C`:

`k_ox = 0.835 + 1.81e-4 · T`

Weakly increasing with temperature, unlike a fully dense ceramic — the fit
is for the porous, cracked, in-reactor oxide, not for laboratory zirconia.

# Values, for scale

`0.9255` W/(m·K) at 500 K; `0.9436` at 600 K; `0.9617` at 700 K; `1.0160`
at 1000 K. Compare Zircaloy at roughly 15 W/(m·K): the oxide is about
**16 times** the thermal resistance per unit thickness, so 60 µm of oxide
is thermally worth about 1 mm of metal — more than the whole cladding wall.

# Valid range and assumptions

Upstream states no range. The fit is used wherever corrosion is, i.e.
roughly 500–1500 K; it is a straight line and will keep returning finite,
slowly-rising numbers outside that, which is extrapolation, not physics.
Assumes an adherent in-reactor oxide of the kind LWR waterside corrosion
produces; it is not a model of the thick, spalled oxide of a severe
accident.

The temperature to pass is the **oxide's outer-surface** temperature, which
is what upstream uses (`Tb`, the patch field of `T`).

```
use outram_park_fork_offbeat::corrosion::oxide_conductivity;

let k = oxide_conductivity(600.0);
assert!((k - 0.9436).abs() < 1.0e-4);
// Far worse a conductor than the metal underneath it.
assert!(k < 2.0);
```

```rust
pub fn oxide_conductivity(temperature: f64) -> f64 { /* ... */ }
```

#### Function `oxide_thermal_coupling`

**Attributes:**

- `MustUse { reason: None }`

Interface temperature and effective boundary conductivity for an oxidised
wall.

Direct translation of the per-face loop in upstream's
`zircaloyOuterCorrosion::correct`:

```text
k_ox  = 0.835 + 1.81e-4 · T_surface
α_ox  = k_ox / max(d_ox, SMALL)          // oxide conductance per unit area
α_m   = k_metal / distance               // metal-cell conductance per unit area
β     = α_ox / (α_ox + α_m)
k_b   = β · k_metal                      // effective boundary conductivity
T_i   = β · T_surface + (1 − β) · T_cell // interface temperature
```

The two conductances are in series; `β` is the fraction of the total
temperature drop that falls across the *metal* half-cell, so the interface
temperature is that fraction of the way from the cell centre to the surface.

# Parameters

- `surface_temperature` — temperature \[K\] at the oxide's outer face, i.e.
  the coolant-side wall temperature. Upstream's `Tb`.
- `first_cell_temperature` — temperature \[K\] at the centre of the metal
  cell adjoining the wall. Upstream's `Tp`.
- `metal_conductivity` — thermal conductivity \[W/(m·K)\] of the cladding
  metal in that cell; about 15 W/(m·K) for Zircaloy. Must be `>= 0`.
- `cell_to_face_distance` — distance \[m\] from that cell's centre to the
  wall face. Upstream passes its reciprocal (`patch.deltaCoeffs()`); this
  port takes the distance itself because that is the quantity a human can
  picture. Must be `> 0`.
- `mean_oxide_thickness` — oxide thickness \[m\] to charge the resistance
  against. Upstream uses the **mid-step average**,
  `0.5·(S_old + S_new)`, floored at `SMALL`; pass the same if you are
  reproducing an OFFBEAT run.

# Behaviour at the edges

- **Zero oxide** — `α_ox` becomes `k_ox/1e-15`, astronomically larger than
  `α_m`, so `β → 1`, the boundary conductivity is unchanged and the
  interface temperature is the surface temperature. Correct, and it is why
  upstream's `SMALL` floor is there.
- **A non-positive `cell_to_face_distance` or a negative
  `metal_conductivity`** returns `β = 1` and the surface temperature rather
  than dividing by zero. This guard is this port's; upstream has none.

# Example

```
use outram_park_fork_offbeat::corrosion::oxide_thermal_coupling;

// Bare metal: the interface IS the surface.
let bare = oxide_thermal_coupling(600.0, 620.0, 15.0, 5.0e-5, 0.0);
assert!((bare.interface_temperature - 600.0).abs() < 1.0e-6);
assert!((bare.blending_factor - 1.0).abs() < 1.0e-9);

// 60 um of oxide: the interface is pulled towards the metal cell.
let oxidised = oxide_thermal_coupling(600.0, 620.0, 15.0, 5.0e-5, 6.0e-5);
assert!(oxidised.interface_temperature > 600.0);
assert!(oxidised.interface_temperature < 620.0);
assert!(oxidised.boundary_conductivity < 15.0);
```

```rust
pub fn oxide_thermal_coupling(surface_temperature: f64, first_cell_temperature: f64, metal_conductivity: f64, cell_to_face_distance: f64, mean_oxide_thickness: f64) -> OxideThermalCoupling { /* ... */ }
```

### Constants and Statics

#### Constant `PILLING_BEDWORTH_ZIRCONIUM`

Pilling–Bedworth ratio of zirconium \[-\] — the volume of oxide formed per
unit volume of metal consumed.

`1.56` is upstream's hard-coded value in `zircaloyOuterCorrosion.C`, and is
the standard figure for ZrO2 on Zr. Because it is greater than one, the
oxide layer is always thicker than the metal it ate: a 60 µm oxide has
consumed 60/1.56 = 38.5 µm of wall.

Used by [`CorrosionModel::metal_loss`](model::CorrosionModel::metal_loss)
and by the hydrogen-pickup model, which needs the *metal* consumed to know
how much hydrogen the reaction released.

```rust
pub const PILLING_BEDWORTH_ZIRCONIUM: f64 = 1.56;
```

### Re-exports

#### Re-export `AccelerationOutcome`

```rust
pub use acceleration::AccelerationOutcome;
```

#### Re-export `AccelerationScheme`

```rust
pub use acceleration::AccelerationScheme;
```

#### Re-export `AndersonMixing`

```rust
pub use acceleration::AndersonMixing;
```

#### Re-export `FixedPointReport`

```rust
pub use acceleration::FixedPointReport;
```

#### Re-export `HydrogenPickupModel`

```rust
pub use hydrogen::HydrogenPickupModel;
```

#### Re-export `PickupScaling`

```rust
pub use hydrogen::PickupScaling;
```

#### Re-export `OxidationKinetics`

```rust
pub use kinetics::OxidationKinetics;
```

#### Re-export `CorrosionModel`

```rust
pub use model::CorrosionModel;
```

#### Re-export `CorrosionState`

```rust
pub use state::CorrosionState;
```

#### Re-export `CorrosionStep`

```rust
pub use state::CorrosionStep;
```

#### Re-export `oxide_conductivity`

```rust
pub use thermal::oxide_conductivity;
```

#### Re-export `oxide_thermal_coupling`

```rust
pub use thermal::oxide_thermal_coupling;
```

#### Re-export `OxideThermalCoupling`

```rust
pub use thermal::OxideThermalCoupling;
```

## Module `fgr`

Fission-gas release (FGR) — how xenon and krypton get out of the fuel, and
what that does to the rod.

# The physics, for a reader with no fuel-performance background

About 30 fission events in 100 produce an atom of xenon or krypton. These are
noble gases: they are chemically inert, essentially insoluble in the UO2
lattice, and they do not go away. Their fate through life is a three-stage
journey:

1. **Born in the grain.** A fission fragment stops within a few micrometres
   of where it was created, so the gas atom starts inside a UO2 grain
   (typical grain radius 5–10 µm).
2. **Diffuses to the grain boundary.** Thermally activated diffusion carries
   the atom to the grain face, where it joins a lenticular bubble. This is
   strongly temperature-dependent — below roughly 1000–1200 K almost nothing
   moves, above ~1700 K it is fast. Some of the gas is knocked back into the
   lattice by passing fission fragments ("irradiation re-solution"), which is
   why release is a competition rather than a one-way trip.
3. **Vents to the rod free volume.** Once the grain-boundary bubbles
   interlink, or once the fuel cracks, the gas escapes into the gap and
   plenum.

Two consequences make FGR one of the most important couplings in a
fuel-performance code, and both are *bad*:

- **Rod pressure rises.** Released gas adds moles to a nearly fixed volume.
  At high burnup this can lift rod internal pressure above coolant pressure
  and re-open the pellet-cladding gap ("lift-off").
- **Gap conductance collapses.** The as-filled helium in the gap is an
  excellent conductor for a gas; xenon and krypton are roughly an order of
  magnitude worse. Diluting the helium raises the fuel temperature, which
  raises the diffusion rate, which releases more gas — a genuine positive
  feedback that the coupled solve has to resolve.

Gas that does *not* escape is not harmless either: it collects in
intragranular and intergranular bubbles and swells the fuel, closing the gap
from the other side.

# What upstream OFFBEAT actually provides — and what this port contains

This matters, because it is easy to assume a fuel-performance code ships a
menu of simple empirical FGR correlations. **At upstream commit
`80e8445`, OFFBEAT does not.** The whole of
`offbeatLib/fissionGasRelease/` is three classes:

| Upstream typename | File | What it is |
|---|---|---|
| `none` | `fissionGasRelease.C` | Release switched off; the gas fields exist but never evolve. |
| `SCIANTIX` | `fgrSCIANTIX.C` | A coupling shim that calls **SCIANTIX**, a separate MIT-licensed 0-D grain-scale code (Politecnico di Milano), once per fuel cell per outer iteration. |
| `SCIANTIXRIA` | `fgrSCIANTIXRIA.C` | A restart model for reactivity-initiated accidents that **does not call SCIANTIX at all** — it reads the gas inventories left by a base-irradiation `SCIANTIX` run and vents them on temperature/burnup/damage thresholds. |

There is no Vitanza-threshold, ANS-5.4, Forsberg-Massih or Booth-diffusion
model in the tree to port. Rather than invent one and present it as a port,
[`FissionGasReleaseModel`] mirrors exactly what is there:

- [`FissionGasReleaseModel::Disabled`] — a faithful port of `none`.
- [`FissionGasReleaseModel::TransientVenting`] — a faithful port of the
  `SCIANTIXRIA` threshold logic, which is genuinely OFFBEAT's own code.
- [`FissionGasReleaseModel::Sciantix`] — **declared but not implemented**. It
  returns [`OffbeatError::NotImplemented`]. It never returns zero release,
  because a silent zero here would look like "this fuel released no gas",
  which is a physically meaningful and dangerously wrong statement.

The gas *bookkeeping* that OFFBEAT does around whichever model is selected —
Xe/Kr yields, atoms to moles, released volume at reference conditions,
release fraction, and the FGR-driven timestep control — **is** ported, as
free functions and small value types, because it is model-independent and
reusable.

# Units

Raw `f64` in strict SI, with two conventions worth stating up front:

- Gas inventories are **atoms per cubic metre of fuel** \[at/m³\], which is
  upstream's convention for the `Gas_grain`, `Gas_boundary` and `Gas_released`
  fields.
- Release *fractions* here are dimensionless in `[0, 1]`. **Upstream carries
  them as percentages**, so `fgr_` in the C++ is 100x these values; see
  [`release_fraction`].
- Swelling strains are **volumetric** \[-\], not linear. Divide by three for
  the linear equivalent, as [`crate::materials::MaterialState`] documents.

# Status

Scaffold. No human verification or validation. Every test below is a
self-consistency or code-equivalence check against the upstream C++
expressions; none is a validation against experiment or against a
fission-gas-release benchmark.

```rust
pub mod fgr { /* ... */ }
```

### Types

#### Struct `ReleasedGasMoles`

Moles of each released gas species over one timestep.

# What it represents

The **increment** of gas handed to the rod's free-volume / gap-gas model in
one timestep, split by species because xenon, krypton and helium have very
different thermal conductivities and the gap conductance depends on the
mixture, not just the total. Port of upstream's
`fissionGasRelease::gasComponents()` / `gasMols()` pair, which returns the
ordered list `("Xe", "Kr", "He")` and the matching moles.

# Units

All three fields are in **moles** \[mol\], not moles per unit volume: they
are already integrated over the cell (or rod) volume.

# Helium

Helium is not a fission gas in the Xe/Kr sense — it comes from alpha decay of
the actinides and, in some designs, from as-fabricated fill gas or from
(n,alpha) reactions in a burnable poison. Upstream tracks it on a separate
inventory (`Helium_released_`) and this port keeps that separation.

```rust
pub struct ReleasedGasMoles {
    pub xenon: f64,
    pub krypton: f64,
    pub helium: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `xenon` | `f64` | Xenon released this timestep \[mol\]. |
| `krypton` | `f64` | Krypton released this timestep \[mol\]. |
| `helium` | `f64` | Helium released this timestep \[mol\]. |

##### Implementations

###### Methods

- ```rust
  pub fn from_released_atoms(fission_gas_atoms: f64, helium_atoms: f64, volume: f64) -> Result<Self> { /* ... */ }
  ```
  Convert released *atom* inventories into moles of each species.

- ```rust
  pub fn total(self: &Self) -> f64 { /* ... */ }
  ```
  Total moles released \[mol\], all three species.

- ```rust
  pub fn volume_at_reference_conditions(self: &Self) -> f64 { /* ... */ }
  ```
  Volume this gas would occupy \[m³\] at the reference condition

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
    fn clone(self: &Self) -> ReleasedGasMoles { /* ... */ }
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
    fn default() -> ReleasedGasMoles { /* ... */ }
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
    fn eq(self: &Self, other: &ReleasedGasMoles) -> bool { /* ... */ }
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
#### Struct `FissionGasInventory`

The fission-gas and helium inventory of **one fuel cell**, split by where the
gas currently sits.

# Why the three-way split

It is the split that decides what a transient can release. Gas already
*released* is gone from the fuel. Gas at the *grain boundary* is one crack
away from the free volume — a power ramp or cladding failure vents it almost
instantly. Gas still *in the grain* is the deep reservoir; only fuel
restructuring (high-burnup-structure formation, grain-boundary sweeping,
melting) reaches it. The [`FissionGasReleaseModel::TransientVenting`] model
exists precisely to decide which of those reservoirs a given cell dumps.

Field names mirror upstream's SCIANTIX-side fields `Gas_grain_`,
`Gas_boundary_`, `Gas_released_`, `Helium_grain_`, `Helium_boundary_`,
`Helium_released_`, `intragranularGasSwelling_`,
`intergranularGasSwelling_`.

# Units

- all six inventories: **atoms per cubic metre of fuel** \[at/m³\], >= 0.
- both swellings: **volumetric** strain \[-\], >= 0.

```rust
pub struct FissionGasInventory {
    pub gas_in_grain: f64,
    pub gas_at_boundary: f64,
    pub gas_released: f64,
    pub helium_in_grain: f64,
    pub helium_at_boundary: f64,
    pub helium_released: f64,
    pub intragranular_swelling: f64,
    pub intergranular_swelling: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `gas_in_grain` | `f64` | Xe + Kr still dissolved or in intragranular bubbles inside the grains<br>\[at/m³\]. |
| `gas_at_boundary` | `f64` | Xe + Kr accumulated in grain-boundary bubbles \[at/m³\]. |
| `gas_released` | `f64` | Xe + Kr already released to the rod free volume \[at/m³ of fuel\]. |
| `helium_in_grain` | `f64` | Helium still inside the grains \[at/m³\]. |
| `helium_at_boundary` | `f64` | Helium at grain boundaries \[at/m³\]. |
| `helium_released` | `f64` | Helium already released \[at/m³ of fuel\]. |
| `intragranular_swelling` | `f64` | Volumetric swelling strain from intragranular (in-grain) gas bubbles<br>\[-\]. |
| `intergranular_swelling` | `f64` | Volumetric swelling strain from intergranular (grain-boundary) gas<br>bubbles \[-\]. |

##### Implementations

###### Methods

- ```rust
  pub fn total_fission_gas(self: &Self) -> f64 { /* ... */ }
  ```
  Total Xe + Kr present in the cell \[at/m³\] — in-grain plus boundary plus

- ```rust
  pub fn total_helium(self: &Self) -> f64 { /* ... */ }
  ```
  Total helium present in the cell \[at/m³\].

- ```rust
  pub fn validate(self: &Self) -> Result<()> { /* ... */ }
  ```
  Validate that every inventory and swelling is finite and non-negative.

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
    fn clone(self: &Self) -> FissionGasInventory { /* ... */ }
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
    fn default() -> FissionGasInventory { /* ... */ }
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
    fn eq(self: &Self, other: &FissionGasInventory) -> bool { /* ... */ }
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
#### Struct `FuelCellConditions`

The local conditions a fission-gas-release model is evaluated at, for **one
fuel cell**.

Kept separate from [`crate::materials::MaterialState`] because FGR needs one
thing the material correlations do not — the accumulated fuel `damage` from
the mechanics solve — and does not need most of what they do.

# Units and ranges

- `temperature` \[K\], absolute, must be > 0.
- `burnup` \[MWd/kg\], >= 0. **Read the note on
  [`TransientVentingThresholds::hbs_burnup_threshold`] about which mass basis
  this is on** before comparing against a threshold.
- `damage` \[-\], in `[0, 1]`: 0 is intact fuel, 1 is fully cracked/failed.
  Upstream reads this from a `damage` field written by the constitutive-law
  `damageModel`.

```rust
pub struct FuelCellConditions {
    pub temperature: f64,
    pub burnup: f64,
    pub damage: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `temperature` | `f64` | Local fuel temperature \[K\]. |
| `burnup` | `f64` | Local burnup \[MWd/kg\]; see the type docs on the mass basis. |
| `damage` | `f64` | Local accumulated damage \[-\] in `[0, 1]`. |

##### Implementations

###### Methods

- ```rust
  pub fn fresh(temperature: f64) -> Self { /* ... */ }
  ```
  Undamaged, unirradiated fuel at `temperature` \[K\].

- ```rust
  pub fn validate(self: &Self) -> Result<()> { /* ... */ }
  ```
  Validate temperature, burnup and damage.

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
    fn clone(self: &Self) -> FuelCellConditions { /* ... */ }
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
    fn eq(self: &Self, other: &FuelCellConditions) -> bool { /* ... */ }
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
#### Struct `GasReleaseOutcome`

What a fission-gas-release model produced for one cell over one timestep.

# Units

- `gas_released`, `helium_released`: cumulative released inventories
  \[at/m³ of fuel\], **not** increments. Subtract the previous step's values
  to get the increment to feed [`ReleasedGasMoles::from_released_atoms`].
- both swellings: volumetric strain \[-\].

```rust
pub struct GasReleaseOutcome {
    pub gas_released: f64,
    pub helium_released: f64,
    pub intragranular_swelling: f64,
    pub intergranular_swelling: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `gas_released` | `f64` | Cumulative released Xe + Kr \[at/m³ of fuel\]. |
| `helium_released` | `f64` | Cumulative released helium \[at/m³ of fuel\]. |
| `intragranular_swelling` | `f64` | Volumetric intragranular gas swelling after this step \[-\]. |
| `intergranular_swelling` | `f64` | Volumetric intergranular gas swelling after this step \[-\]. |

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
    fn clone(self: &Self) -> GasReleaseOutcome { /* ... */ }
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
    fn default() -> GasReleaseOutcome { /* ... */ }
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
    fn eq(self: &Self, other: &GasReleaseOutcome) -> bool { /* ... */ }
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
#### Struct `TransientVentingThresholds`

Thresholds for the transient venting model
([`FissionGasReleaseModel::TransientVenting`]).

Port of the four `fgrOptions` keywords upstream's `fgrSCIANTIXRIA` reads:
`releaseHBS`, `buReleaseThresholdHBS`, `temperatureReleaseThresholdHBS` and
`damageReleaseThreshold` (`fgrSCIANTIXRIA.C:168-171, 202-209`).

[`Default`] reproduces upstream's defaults exactly.

```rust
pub struct TransientVentingThresholds {
    pub release_hbs: bool,
    pub hbs_burnup_threshold: f64,
    pub hbs_temperature_threshold: f64,
    pub damage_threshold: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `release_hbs` | `bool` | Whether the high-burnup-structure (HBS) release path is active \[-\].<br><br>Upstream default `true`.<br><br>**What HBS is:** at the pellet rim, where the local burnup is far above<br>the pellet average, UO2 restructures into a fine-grained, highly porous<br>"rim" or high-burnup structure. The original micrometre grains are<br>replaced by sub-micrometre ones and the fission gas migrates into large<br>closed pores. That gas is held only weakly, so a transient that heats the<br>rim can vent essentially the whole local inventory at once. |
| `hbs_burnup_threshold` | `f64` | Burnup above which the fuel is treated as restructured into HBS<br>\[MWd/kg\]. Upstream default is `80000` MWd/t, i.e. **80 MWd/kg**.<br><br># Mass-basis warning<br><br>Upstream compares `Effective_burn_up*1000 > buReleaseThresholdHBS`, where<br>`Effective_burn_up` is a SCIANTIX state variable. SCIANTIX carries burnup<br>on the **oxide** basis (its `U_UO2 = 0.8815` constant converts to the<br>metal basis), so upstream's comparison is oxide-basis, whereas<br>[`crate::burnup::BurnupAccumulator`]'s canonical output is heavy-metal<br>basis and is ~13.4 % larger for the same fuel. This port does **not**<br>silently convert: it compares whatever [`FuelCellConditions::burnup`] you<br>give it against whatever threshold you give it. Supply both on the same<br>basis. If you are reproducing an upstream case, pass<br>[`crate::burnup::BurnupAccumulator::burnup_mwd_per_tonne_oxide`] / 1000<br>together with the 80 MWd/kg default. |
| `hbs_temperature_threshold` | `f64` | Temperature above which HBS-held gas is treated as vented \[K\].<br>Upstream default 1000 K. |
| `damage_threshold` | `f64` | Damage above which grain-boundary gas is treated as vented \[-\], in<br>`[0, 1]`. Upstream default 0.85.<br><br>The physical picture: once the fuel is that cracked, the grain-boundary<br>bubble network is connected to the free volume, so boundary gas escapes —<br>but the gas still inside the grains does not, because the grains<br>themselves are intact. |

##### Implementations

###### Methods

- ```rust
  pub fn new(release_hbs: bool, hbs_burnup_threshold: f64, hbs_temperature_threshold: f64, damage_threshold: f64) -> Result<Self> { /* ... */ }
  ```
  Build a validated threshold set.

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
    fn clone(self: &Self) -> TransientVentingThresholds { /* ... */ }
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
    Upstream `fgrSCIANTIXRIA` defaults: HBS release on, 80 MWd/kg

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
    fn eq(self: &Self, other: &TransientVentingThresholds) -> bool { /* ... */ }
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
#### Enum `FissionGasReleaseModel`

Which fission-gas-release model is in effect.

# Why an enum and not a trait object

The set of models is closed and known at compile time, so [`Self::correct`]'s
`match` is exhaustive: adding a model makes every dispatch site a compile
error rather than a runtime surprise. This is the workspace rule (root
`CLAUDE.md`, "No trait objects"); it also keeps the type `Copy` and heap-free,
and go-to-definition works on each variant, which it does not on a `dyn`
implementation.

# Variants map onto upstream's runtime-selectable models

| Here | Upstream `fgr` typename | Implemented? |
|---|---|---|
| [`Self::Disabled`] | `none` | yes, faithfully |
| [`Self::TransientVenting`] | `SCIANTIXRIA` | yes — this is OFFBEAT's own code, not SCIANTIX's |
| [`Self::Sciantix`] | `SCIANTIX` | **no** — returns [`OffbeatError::NotImplemented`] |

See the module documentation for why there is no Vitanza / ANS-5.4 /
Forsberg-Massih variant: upstream has none to port.

```rust
pub enum FissionGasReleaseModel {
    Disabled,
    TransientVenting(TransientVentingThresholds),
    Sciantix,
}
```

##### Variants

###### `Disabled`

Release switched off — the gas inventory and swellings are carried
forward unchanged. Port of upstream's `none`.

This is **not** "zero release": whatever inventory the cell already had
(from a restart, or from initial conditions) is preserved and returned. A
fresh cell with a zero inventory does stay at zero, which is correct for
fresh fuel. Selecting this variant means "I am not modelling gas
evolution", and the resulting rod pressure and gap conductance must be
read in that light.

###### `TransientVenting`

Threshold-driven venting of an already-computed gas inventory, for
transients. Port of upstream's `SCIANTIXRIA`.

# What it does

Upstream's `SCIANTIXRIA` is a **restart** model: it is selected for the
reactivity-initiated-accident (RIA) phase of a two-stage run whose base
irradiation was computed with SCIANTIX. It does not call SCIANTIX at all
— the grain and grain-boundary gas inventories are read from the restart
time directory and this model only decides, cell by cell, how much of
them vents. That is why it can be ported honestly without SCIANTIX: the
logic in `fgrSCIANTIXRIA::correct()` is entirely OFFBEAT's.

# The three branches (`fgrSCIANTIXRIA.C:260-292`)

1. **HBS venting** — if `release_hbs` and burnup > threshold and
   temperature > threshold: release *everything*, in-grain and boundary
   gas alike, and zero both swellings.
2. **Damage venting** — else if damage > threshold: release the boundary
   gas only, and zero the intergranular swelling; the in-grain gas and
   intragranular swelling survive.
3. **Otherwise** — release nothing new; carry both swellings forward.

# What it is not

It is not a diffusion model and cannot generate gas or move it from grain
to boundary. It only vents what is already there. Driving it from a
fabricated inventory produces a fabricated release; the inventory has to
come from somewhere real.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `TransientVentingThresholds` |  |

###### `Sciantix`

The SCIANTIX 0-D grain-scale model — **declared, not implemented**.

[`Self::correct`] returns [`OffbeatError::NotImplemented`] for this
variant. It never returns a zero release, because a silent zero would be
a physically meaningful and badly wrong statement about the fuel.

# Why it is not implemented

SCIANTIX is a **separate code**, not part of OFFBEAT: a 0-D grain-scale
inert-gas-behaviour solver from Politecnico di Milano, distributed under
the MIT licence, which OFFBEAT vendors and calls once per fuel cell per
outer iteration. What lives in `offbeatLib/fissionGasRelease/fgrSCIANTIX.C`
is the *coupling shim* — marshalling ~100 state variables in and out —
not the physics. Porting the physics means porting SCIANTIX itself:
Turnbull single-atom diffusion, Ham trapping, Turnbull irradiation
re-solution, Baker nucleation, Pizzocri intragranular bubble evolution,
Pastore/Barani grain-boundary behaviour and micro-cracking, Ainscough
grain growth, and the SDA/FORMAS numerical solvers behind them. That is a
separate piece of work with its own licence-provenance and V&V
obligations, and it is deliberately out of scope for this module.

The variant is kept so the model-selection surface matches upstream's and
so a case that asks for SCIANTIX fails loudly and specifically rather than
silently selecting something else.

##### Implementations

###### Methods

- ```rust
  pub fn upstream_name(self: &Self) -> &'static str { /* ... */ }
  ```
  Upstream's runtime-selection typename for this model, for logging and

- ```rust
  pub fn is_implemented(self: &Self) -> bool { /* ... */ }
  ```
  Whether this variant is actually implemented in this port.

- ```rust
  pub fn correct(self: &Self, inventory: &FissionGasInventory, conditions: &FuelCellConditions) -> Result<GasReleaseOutcome> { /* ... */ }
  ```
  Advance the fission-gas state of one fuel cell by one timestep.

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
    fn clone(self: &Self) -> FissionGasReleaseModel { /* ... */ }
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
    fn eq(self: &Self, other: &FissionGasReleaseModel) -> bool { /* ... */ }
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

#### Function `fission_gas_production_rate`

Stable fission-gas (Xe + Kr) production rate \[at/(m³·s)\] from a
fission-rate density \[fissions/(m³·s)\].

Multiplies by [`FISSION_GAS_ATOMS_PER_FISSION`]. Chain this with
[`crate::burnup::fission_rate_density`] to go straight from the thermal
solve's volumetric power to a gas production rate.

# Inputs and range

`fission_rate_density` in fissions/(m³·s), finite and >= 0. An LWR pellet at
nominal power runs at ~1.2e19 fissions/(m³·s), giving ~3.7e18 gas atoms per
cubic metre per second.

# Errors

[`OffbeatError::Unphysical`] if the rate is negative or not finite.

```
use outram_park_fork_offbeat::burnup::fission_rate_density;
use outram_park_fork_offbeat::fgr::fission_gas_production_rate;

let f = fission_rate_density(3.79e8).unwrap();
let g = fission_gas_production_rate(f).unwrap();
assert!((g / f - 0.301).abs() < 1e-12);
```

```rust
pub fn fission_gas_production_rate(fission_rate_density: f64) -> crate::error::Result<f64> { /* ... */ }
```

#### Function `release_fraction`

Fraction of the produced fission gas that has been released \[-\], in
`[0, 1]`.

# Methodology (port of `fgrSCIANTIX::updateVariables`)

Upstream computes `fgr_ = released/max(produced, SMALL)*100`, i.e. a
**percentage**, with `SMALL = 1e-15`. This function returns the *fraction*;
multiply by 100 for upstream's number.

# Inputs, ranges and clamping

- `released_atoms` \[at/m³ or atoms — any consistent unit\], >= 0.
- `produced_atoms` \[same unit\], >= 0.

Zero production gives zero release fraction, which is the physically right
answer for fresh fuel and avoids upstream's `0/SMALL` behaviour. The result
is clamped into `[0, 1]`: a release fraction above one is arithmetically
possible from inconsistent inventories but has no meaning, and clamping it
here stops it propagating into a gap-gas composition.

# Errors

[`OffbeatError::Unphysical`] if either argument is negative or not finite.

```rust
pub fn release_fraction(released_atoms: f64, produced_atoms: f64) -> crate::error::Result<f64> { /* ... */ }
```

#### Function `next_time_step_from_release_change`

Timestep \[s\] the fission-gas-release model would like to take next, so that
the change in release fraction stays under `max_change`.

# Methodology (port of `fissionGasRelease::nextDeltaT`, `maxFgrChange` branch)

Upstream computes
`nextDeltaT = deltaT · maxLocalDeltaFgr / max(localMaxDeltaFgr, SMALL)`
with `SMALL = 1e-15`, and separately the same expression for the
volume-averaged total change, returning the smaller of the two. This function
is one of those two branches; call it twice (once with the local maximum
change, once with the volume-averaged change) and take the minimum, which is
what upstream does.

Upstream's `maxLocalFgrChange` / `maxTotalFgrChange` are in **percent**, and
so is its `fgr_` field, so the ratio is unit-free either way. Pass both
arguments here as fractions, or both as percentages — just not one of each.

(Upstream also carries a deprecated `maxFGR` branch that limits moles released
per step rather than release fraction. It is marked "to be removed in the
future" in the C++ and is not ported.)

# Inputs

- `current_dt` \[s\] — the timestep just taken, > 0.
- `max_change` \[-\] — the largest change in release fraction wanted per
  step, > 0. A typical value is 0.01 (1 percentage point).
- `observed_change` \[-\] — the change actually seen over the last step,
  >= 0.

# Returns

The suggested next timestep \[s\]. Like the burnup criterion it is *advice*:
take the minimum over every model's suggestion, and clamp it, because a zero
observed change gives an enormous (but finite) answer.

# Errors

[`OffbeatError::Unphysical`] for a non-finite argument, a non-positive
`current_dt` or `max_change`, or a negative `observed_change`.

```rust
pub fn next_time_step_from_release_change(current_dt: f64, max_change: f64, observed_change: f64) -> crate::error::Result<f64> { /* ... */ }
```

### Constants and Statics

#### Constant `XENON_ATOMS_PER_FISSION`

Xenon atoms produced per fission \[at/fission\].

Value 0.268, from upstream `fgrSCIANTIX.C::gasMols()`, which splits the total
released fission gas as `Xe = (0.268/0.301)·molFGR`. It is a cumulative
fission yield for the stable and long-lived xenon isotopes; it varies by a
few percent with the fissioning nuclide (²³⁵U vs ²³⁹Pu) and upstream uses one
number for all of them.

```rust
pub const XENON_ATOMS_PER_FISSION: f64 = 0.268;
```

#### Constant `KRYPTON_ATOMS_PER_FISSION`

Krypton atoms produced per fission \[at/fission\].

Value 0.033, from upstream `fgrSCIANTIX.C::gasMols()`
(`Kr = (0.033/0.301)·molFGR`). Same caveats as
[`XENON_ATOMS_PER_FISSION`].

```rust
pub const KRYPTON_ATOMS_PER_FISSION: f64 = 0.033;
```

#### Constant `FISSION_GAS_ATOMS_PER_FISSION`

Total stable fission-gas (Xe + Kr) atoms produced per fission
\[at/fission\].

Value 0.301, written as the literal that upstream uses as its denominator in
`fgrSCIANTIX.C::gasMols()` (`0.268/0.301`, `0.033/0.301`) rather than as the
sum [`XENON_ATOMS_PER_FISSION`] + [`KRYPTON_ATOMS_PER_FISSION`]. The two
agree to one unit in the last place — the `f64` sum is
`0.30100000000000005` — and using the literal keeps the Xe and Kr fractions
exactly upstream's ratios.

The familiar rule of thumb "about 30 % of fissions make a noble-gas atom" is
this number.

```rust
pub const FISSION_GAS_ATOMS_PER_FISSION: f64 = 0.301;
```

#### Constant `AVOGADRO`

Avogadro constant \[1/mol\], the 2019 SI defined value 6.02214076e23.

Upstream uses the rounded `6.02e23` in the mole accumulation
(`fgrSCIANTIX.C:673, 680`) and `6.022e23` in the post-processing
(`fgrSCIANTIX.C:811, 822`). Those differ from the exact value by 0.036 % and
0.0024 % respectively. This port uses the exact value everywhere, so mole
counts here are ~0.04 % below upstream's — far inside any FGR model's
uncertainty, but stated so the difference is not mistaken for a bug.

```rust
pub const AVOGADRO: f64 = 6.022_140_76e23;
```

#### Constant `MOLAR_GAS_CONSTANT`

Molar gas constant \[J/(mol·K)\], the 2019 SI defined value 8.314462618.

Upstream uses `8.314` (`fgrSCIANTIX.C:812`), which is the same to 6
significant figures.

```rust
pub const MOLAR_GAS_CONSTANT: f64 = 8.314_462_618;
```

#### Constant `REFERENCE_TEMPERATURE`

Reference temperature \[K\] for quoting a released-gas *volume*.

Value 293 K, upstream's choice in `fgrSCIANTIX.C:812`
(`fgrM3_ = nMoles*8.314*293/101325`). Released FGR is conventionally reported
as a volume at some stated reference condition rather than as moles; there is
no universal standard, so the condition must always be quoted with the
number. Note this is 293 K, not the 273.15 K of "standard temperature".

```rust
pub const REFERENCE_TEMPERATURE: f64 = 293.0;
```

#### Constant `REFERENCE_PRESSURE`

Reference pressure \[Pa\] for quoting a released-gas *volume*.

Value 101 325 Pa (one standard atmosphere), upstream's choice in
`fgrSCIANTIX.C:812`.

```rust
pub const REFERENCE_PRESSURE: f64 = 101_325.0;
```

## Module `gap`

Fuel/cladding gap: gas composition, gap conductance, contact and axial
slicing.

# What this module is for

The gap between the fuel pellet and the cladding is where fuel performance is
decided. Heat leaves the pellet by three parallel paths across it:

1. **Conduction through the fill gas** — a helium fill at beginning of life,
   progressively diluted by released xenon and krypton, which conduct roughly
   twenty times worse.
2. **Radiation** between the two surfaces, which matters only once they are
   hot.
3. **Solid conduction through the contact spots**, once thermal expansion,
   swelling and creep have closed the gap and the surfaces bear on each other.

The resulting gap conductance swings over orders of magnitude through life,
and it feeds straight back into the temperature field that drove the closure.
Getting the closure logic wrong makes the whole rod history wrong.

# Gap conventions — read this before using anything here

Upstream OFFBEAT is not uniform about whether a "gap" is a radial or a
diametral quantity, and [`crate::materials::behavioral::relocation`] already
had to flag one such ambiguity (its `cold_gap` is **diametral**). This module
does not reintroduce it. The rules here are absolute:

- **Every gap width, roughness, jump distance and radius in this module is
  RADIAL** — a surface-to-surface normal separation, not a diameter
  difference. If your input deck quotes a diametral gap, halve it before
  passing it in.
- **The sign convention for a radial gap width differs between the thermal and
  the mechanical side, and both are reproduced faithfully:**
  - [`conductance`] takes an **unsigned, open-only** radial gap width: `0`
    means the surfaces touch, positive means open. Upstream's
    `fuelRodGapFvPatchScalarField::gapWidth()` clips at zero
    (`max((nbrCf - Cf) & nf, 0)`), so a closed gap carries no information
    about *how hard* it is closed — that arrives separately as the interface
    pressure.
  - [`contact`] takes a **signed** radial gap width: positive is open,
    **negative is interpenetration**, which is exactly what the penalty
    formulation needs. Upstream's `contactFvPatchVectorField::gapWidth()`
    does *not* clip.
- **Roughness is a per-surface radial arithmetic-mean roughness \[m\]**, one
  value for the fuel surface and one for the cladding surface. Upstream
  combines them two different ways inside the same routine (an arithmetic
  mean for the empirical exponent, a root-sum-square for the divisor); both
  are reproduced.
- **Temperatures are surface temperatures**, not bulk or cell-centre
  temperatures.

# Units

Raw `f64` in strict SI throughout, per the crate-level units note: metre,
kelvin, pascal, W/m²K for a conductance, W/m/K for a conductivity, kilogram,
mole, m³. The **one** deliberate exception is
[`gas::GapGasSpecies::molar_mass_g_per_mol`], which is g/mol because that is
how upstream tabulates it and how fuel-performance input decks quote it; the
SI companion [`gas::GapGasSpecies::molar_mass`] sits right beside it.

# Module map

| Submodule | What it holds | Upstream origin |
|---|---|---|
| [`gas`] | Gas species, mass/mole composition, mixture conductivity, accommodation coefficient, fission-gas dilution | `gapGasModel.C`, `gapFRAPCON::kappa/a` |
| [`conductance`] | The three parallel heat paths and their sum; the series interface resistance | `fuelRodGap`, `trisoGap`, `resistiveGap` patch fields |
| [`contact`] | Penalty contact: interface pressure from interpenetration, boundary stiffness | `contactFvPatchVectorField`, `gapContactFvPatchVectorField` |
| [`free_volume`] | Rod free volume and the ideal-gas pressure `p = nR / Σ(Vᵢ/Tᵢ)` | `gapFRAPCON::correct`, `correctDish`, `correctCrack` |
| [`mod@slice`] | 1.5D axial slicing (the `sliceMapper` concepts) and volume-weighted slice averaging | `sliceMapper/*` |

# What is deferred, and why

This port covers the **pure functions** of upstream's gap physics. Several
pieces of upstream are not functions of their arguments at all — they are
traversals of an OpenFOAM mesh, an AMI (arbitrary mesh interface) between two
regions, or the multi-region solver's patch-to-patch coupling. Those are
**deferred**, not silently approximated:

- **Gap and plenum volume by the Gauss–Green surface integral**
  (`gapFRAPCON::correctGap`, `correctHole`, `correctPlena`): upstream computes
  `V = ⅓ ∮_S (r_s · n) dS` over the deformed bounding patches. It needs face
  centres, face normals and the displacement field on both sides of the gap.
  [`free_volume`] takes the resulting per-region volumes and temperatures as
  **inputs** and does the thermodynamics; it does not compute them.
- **The gap/plenum scaling factors** (`gapFRAPCON::correctScalingFactors`):
  upstream builds them by intersecting cutting planes with cladding-patch
  edges, precisely because the AMI `weightSum` cannot distinguish "separated
  by a gap" from "partially overlapping axially" on a cylinder. This is
  irreducibly a mesh-topology algorithm. Deferred.
- **AMI interpolation between the fuel-outer and clad-inner patches**, and the
  owner/neighbour averaging in `updateCoeffs()`. The *formulae* evaluated on
  each face are ported; the interpolation that supplies the neighbour values
  is not.
- **Cell-to-material addressing in the slice mappers** (`mat_.matAddrList()`,
  `isA<fuelMaterial>`, the `sliceID` `volScalarField`, and the parallel
  `Pstream` gather/scatter). [`mod@slice`] ports the axial binning arithmetic and
  the volume-weighted average, taking cell axial coordinates and volumes as
  plain slices.
- **Friction and the tangential contact traction** (`contactFvPatchVectorField`'s
  slip/stick update). Only the normal penalty pressure is ported.

Everything deferred is called out again in the doc comment of the item that
would have used it. Nothing here silently substitutes an approximation for a
mesh operation.

# Status

**Untrusted draft material.** Per `RESPONSIBLE_USE.md` this is AI-assisted
output that has had no human verification or validation. Tests in this module
are labelled either *reference-checked* (against a value stated in upstream's
own source) or *self-consistency* (an internal invariant — monotonicity, a
limit, an exact reduction). **No test here is a validation against measured
fuel-rod data**, and nothing in this module may be described as validated.

```rust
pub mod gap { /* ... */ }
```

### Modules

## Module `conductance`

Fuel/cladding gap conductance: gas conduction, radiation and solid contact.

# What this computes

A heat-transfer coefficient `h` \[W/m²K\] across the fuel/cladding interface,
such that the heat flux is `q'' = h · (T_fuel − T_clad)`. It is the sum of
three **parallel** paths, because all three carry heat across the same
interface at the same time:

```text
h_gap = h_gas + h_radiation + h_contact
```

- **`h_gas`** — conduction through the fill gas, `k_gas` divided by an
  *effective* gap thickness. The effective thickness is not the geometric gap:
  it adds the surface roughness (the surfaces are rough, so gas is trapped in
  the asperity valleys), subtracts an empirical offset, and adds a
  temperature-jump distance at each wall (gas molecules do not fully
  equilibrate with a solid in one collision).
- **`h_radiation`** — gray-body exchange between the two surfaces, linearised
  about the surface temperatures so it can enter a linear solve as a
  coefficient.
- **`h_contact`** — solid conduction through the asperity contact spots once
  the surfaces bear on each other. Zero at zero interface pressure, and the
  dominant term once the gap is hard-closed.

# Gap conventions in this module

**Every length here is RADIAL** (see the [module-level conventions]
(super#gap-conventions--read-this-before-using-anything-here)). In particular
[`GapConductanceModel::FuelRodFrapcon::radial_gap_width`] is the *radial*
surface-to-surface separation and is **unsigned**: `0` means the surfaces
touch. It is not a diametral gap; halve a diametral input before passing it.

Upstream computes that width as
`max((C_clad + D_clad − C_fuel − D_fuel) · n, 0)` on each interface face —
the deformed face-centre separation projected on the face normal, clipped at
zero. **That computation is deferred here** (it needs the mesh's face centres,
face normals and the AMI interpolation between the two regions); this module
takes the resulting width as an input.

# Deferred

- The gap-width evaluation and the AMI interpolation of neighbour
  temperatures, conductivities, emissivities and roughnesses, as above.
- The owner/neighbour averaging that upstream's `updateCoeffs()` performs
  (`alpha = ½(hGap_own + interp(hGap_nbr))`). Ported as far as
  [`average_across_interface`], which is the arithmetic without the
  interpolation.
- The `interfaceP` field, which is produced by [`super::contact`] on the
  mechanical side and consumed here as [`GapSurfaces::interface_pressure`].

# Units

Strict SI raw `f64`: kelvin, metre, pascal, W/m/K for a conductivity,
W/m²K for a conductance.

```rust
pub mod conductance { /* ... */ }
```

### Types

#### Struct `GapConductance`

The three parallel contributions to gap conductance, kept separate.

Upstream sums them into a single `hGap_` and discards the split. Keeping it
is the difference between "the gap conductance is 5000 W/m²K" and "the gap
conductance is 5000 W/m²K and 94% of it is contact", which is what a reader
actually needs to interpret a rod history.

# Units

All three in W/(m²·K).

```rust
pub struct GapConductance {
    pub gas: f64,
    pub radiation: f64,
    pub contact: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `gas` | `f64` | Conduction through the fill gas \[W/m²K\] — upstream's `hGas`. |
| `radiation` | `f64` | Gray-body radiation between the surfaces \[W/m²K\] — upstream's `hRad`. |
| `contact` | `f64` | Solid conduction through asperity contacts \[W/m²K\] — upstream's<br>`hContact`. Zero when the interface pressure is zero. |

##### Implementations

###### Methods

- ```rust
  pub fn total(self: &Self) -> f64 { /* ... */ }
  ```
  Total gap conductance \[W/m²K\] — the sum of the three parallel paths.

- ```rust
  pub fn contact_fraction(self: &Self) -> f64 { /* ... */ }
  ```
  Fraction \[-\] of the total carried by solid contact, in `[0, 1]`.

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
    fn clone(self: &Self) -> GapConductance { /* ... */ }
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
    fn default() -> GapConductance { /* ... */ }
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
    fn eq(self: &Self, other: &GapConductance) -> bool { /* ... */ }
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
#### Struct `GapConductanceScaling`

Per-term linear scaling `h → F·h + δ`, for sensitivity studies —
upstream's `F_hGas`/`F_hRad`/`F_hContact` and
`delta_hGas`/`delta_hRad`/`delta_hContact` parameters on
`fuelRodGapFvPatchScalarField`.

# Units

The `*_factor` fields are dimensionless; the `*_offset` fields are in
W/(m²·K). [`Default`] is the identity (`factor = 1`, `offset = 0`), i.e.
upstream's defaults.

# Note

Upstream applies the factor to the **clipped** term
(`F · max(h, 0) + δ`), so a negative offset *can* drive a term negative.
That is reproduced; use [`GapConductanceModel::evaluate_checked`] if you
need a negative total to be an error rather than a number.

```rust
pub struct GapConductanceScaling {
    pub gas_factor: f64,
    pub gas_offset: f64,
    pub radiation_factor: f64,
    pub radiation_offset: f64,
    pub contact_factor: f64,
    pub contact_offset: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `gas_factor` | `f64` | Multiplier \[-\] on the gas term. |
| `gas_offset` | `f64` | Additive offset \[W/m²K\] on the gas term. |
| `radiation_factor` | `f64` | Multiplier \[-\] on the radiation term. |
| `radiation_offset` | `f64` | Additive offset \[W/m²K\] on the radiation term. |
| `contact_factor` | `f64` | Multiplier \[-\] on the contact term. |
| `contact_offset` | `f64` | Additive offset \[W/m²K\] on the contact term. |

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
    fn clone(self: &Self) -> GapConductanceScaling { /* ... */ }
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
    Upstream's defaults: every factor 1, every offset 0 — i.e. no scaling.

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
    fn eq(self: &Self, other: &GapConductanceScaling) -> bool { /* ... */ }
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
#### Struct `GapSurfaces`

The state of the two surfaces bounding the gap, on one interface face.

Upstream gathers these by looking patch fields up by name on both sides of a
`regionCoupledOFFBEAT` patch and interpolating the neighbour's through the
AMI. This port takes them as an explicit argument so the dependencies of the
gap model are visible in its signature.

# Naming

"Fuel" is the inner surface and "clad" the outer one for a fuel rod. For a
TRISO particle they are the inner and outer surfaces of the shell gap; the
physics is symmetric in the two apart from which radius is which.

# Units

Strict SI: kelvin, metre, W/m/K, pascal; emissivity dimensionless.

```rust
pub struct GapSurfaces {
    pub fuel_temperature: f64,
    pub clad_temperature: f64,
    pub fuel_roughness: f64,
    pub clad_roughness: f64,
    pub fuel_emissivity: f64,
    pub clad_emissivity: f64,
    pub fuel_conductivity: f64,
    pub clad_conductivity: f64,
    pub interface_pressure: f64,
    pub meyer_hardness: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `fuel_temperature` | `f64` | Fuel (inner) surface temperature \[K\]. Absolute; must be > 0. |
| `clad_temperature` | `f64` | Cladding (outer) surface temperature \[K\]. Absolute; must be > 0. |
| `fuel_roughness` | `f64` | Fuel surface arithmetic-mean roughness \[m\], **radial**.<br><br>Upstream's `roughness` patch entry. Typical as-fabricated UO2 pellet<br>surface: ~1e-6 m (1 µm). |
| `clad_roughness` | `f64` | Cladding inner-surface arithmetic-mean roughness \[m\], **radial**.<br><br>Typical as-fabricated Zircaloy inner surface: ~0.5e-6 m. |
| `fuel_emissivity` | `f64` | Fuel surface total hemispherical emissivity \[-\], in `(0, 1]`.<br><br>UO2 is roughly 0.85. Values are floored at [`SMALL`] internally, matching<br>upstream, so a zero emissivity gives zero radiative transfer rather than<br>a division by zero. |
| `clad_emissivity` | `f64` | Cladding inner-surface emissivity \[-\], in `(0, 1]`. Oxidised Zircaloy<br>is roughly 0.8. |
| `fuel_conductivity` | `f64` | Fuel **solid** thermal conductivity at the surface \[W/m/K\].<br><br>Used only by the contact term, which conducts through the solid asperity<br>spots. UO2 near 1000 K is roughly 3 W/m/K. Not to be confused with the<br>gas conductivity, which comes from the [`GapGasMixture`]. |
| `clad_conductivity` | `f64` | Cladding **solid** thermal conductivity at the surface \[W/m/K\].<br>Zircaloy is roughly 15 W/m/K. |
| `interface_pressure` | `f64` | Normal interface (contact) pressure \[Pa\], `>= 0`.<br><br>Zero for an open gap. Produced on the mechanical side by<br>[`super::contact::PenaltyContact::interface_pressure`]; upstream passes<br>it through the `interfaceP` field. |
| `meyer_hardness` | `f64` | Meyer hardness of the softer contacting material \[Pa\].<br><br>Upstream hard-codes [`MEYER_HARDNESS_ZIRCALOY`]; use that value to<br>reproduce upstream exactly. |

##### Implementations

###### Methods

- ```rust
  pub fn lwr_open_gap(fuel_temperature: f64, clad_temperature: f64) -> Self { /* ... */ }
  ```
  A representative open-gap LWR interface at the given fuel and cladding

- ```rust
  pub fn mean_temperature(self: &Self) -> f64 { /* ... */ }
  ```
  Arithmetic mean of the two surface temperatures \[K\] — the film

- ```rust
  pub fn validate(self: &Self) -> Result<()> { /* ... */ }
  ```
  Reject a physically impossible surface state.

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
    fn clone(self: &Self) -> GapSurfaces { /* ... */ }
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
    fn eq(self: &Self, other: &GapSurfaces) -> bool { /* ... */ }
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
#### Enum `TrisoJumpDistance`

Where the TRISO gas term's temperature-jump distance comes from — upstream's
`jumpDistance` patch entry on `trisoGapFvPatchScalarField`.

```rust
pub enum TrisoJumpDistance {
    Frapcon,
    Prescribed {
        inner: f64,
        outer: f64,
    },
}
```

##### Variants

###### `Frapcon`

Compute it from the FRAPCON correlation, as the fuel-rod model does.

Upstream selects this branch when the `jumpDistance` entry is negative on
**both** patches (its default is `-1`); a value given on only one side is
a fatal error there, and is unrepresentable here by construction.

###### `Prescribed`

Use prescribed per-surface jump distances \[m\], **radial**.

The two are summed. Upstream requires both to be non-negative.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `inner` | `f64` | Jump distance at the inner surface \[m\]. |
| `outer` | `f64` | Jump distance at the outer surface \[m\]. |

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
    fn clone(self: &Self) -> TrisoJumpDistance { /* ... */ }
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
    fn eq(self: &Self, other: &TrisoJumpDistance) -> bool { /* ... */ }
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
#### Enum `GapConductanceModel`

Gap heat-transfer models — one variant per gap boundary condition upstream
compiles.

Dispatch is by `match`, never by a trait object, per the workspace
`CLAUDE.md` "No trait objects" rule.

# Why geometry lives on the variant

The gap width (rod) and the two radii (TRISO) change every outer iteration as
the mechanics solve deforms the two bodies. They sit on the variant rather
than in [`GapSurfaces`] because they are *geometry*, not surface state, and
because that mirrors the crate's existing precedent in
[`crate::materials::behavioral::relocation`]. **Reconstruct the variant when
the geometry changes** — it is `Copy`, so this is free.

```rust
pub enum GapConductanceModel {
    Fixed {
        coefficient: f64,
    },
    FuelRodFrapcon {
        radial_gap_width: f64,
        scaling: GapConductanceScaling,
    },
    TrisoSpherical {
        reference_radius: f64,
        inner_radius: f64,
        outer_radius: f64,
        jump_distance: TrisoJumpDistance,
    },
}
```

##### Variants

###### `Fixed`

A user-prescribed constant interface conductance \[W/m²K\] — upstream
`resistiveGapFvPatchScalarField` with its `alpha` entry.

All of it is attributed to [`GapConductance::gas`], because upstream's
`resistiveGap` makes no split; the radiation and contact terms are zero.
Use it for a prescribed-conductance sensitivity study or to reproduce a
legacy case, not to model gap closure — a fixed conductance cannot
represent the closure feedback loop that dominates rod behaviour.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `coefficient` | `f64` | The prescribed interface conductance \[W/m²K\], `>= 0`. |

###### `FuelRodFrapcon`

Fuel-rod gap, FRAPCON form — upstream `fuelRodGapFvPatchScalarField`,
patch type `fuelRodGap`.

The full three-path model: gas conduction across a roughness- and
jump-augmented planar gap, gray-body radiation, and Ross–Stoute-style
contact conduction. This is the model an LWR rod calculation uses.

# Valid range

`radial_gap_width >= 0` (see the field). Surface temperatures within the
range of the gas conductivity fits, roughly 300–2000 K. Interface
pressures from 0 to a substantial fraction of the Meyer hardness; the
contact correlation's branches are set by `P_interface / H_Meyer`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `radial_gap_width` | `f64` | **RADIAL** gap width \[m\], **unsigned**: `0` means the surfaces<br>touch, positive means open.<br><br>This is *not* a diametral gap. Upstream's<br>`fuelRodGapFvPatchScalarField::gapWidth()` computes it as the<br>deformed face-centre separation projected on the face normal and<br>clips it at zero, so it carries no information about how hard a<br>closed gap is closed — that arrives as<br>[`GapSurfaces::interface_pressure`]. Typical as-fabricated LWR value:<br>8.5e-5 m (half of a 170 µm diametral gap). |
| `scaling` | `GapConductanceScaling` | Per-term linear scaling, for sensitivity studies.<br>[`GapConductanceScaling::default`] reproduces upstream's defaults. |

###### `TrisoSpherical`

TRISO-particle shell gap — upstream `trisoGapFvPatchScalarField`, patch
type `trisoGap`.

Identical radiation and contact terms to
[`FuelRodFrapcon`](Self::FuelRodFrapcon), but the gas term uses the
**spherical-shell** conduction length `r_ref²·(1/r_in − 1/r_out)` in
place of a planar gap width, and applies the bare jump distance rather
than [`JUMP_DISTANCE_MULTIPLIER`] times it. It also omits the
[`ROUGHNESS_OFFSET`] subtraction. All three differences are upstream's
and are reproduced.

# Valid range

`0 < r_in <= r_out`, `reference_radius > 0`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `reference_radius` | `f64` | Radius of this side of the gap \[m\], **radial** — upstream's<br>`r1_`, the radius of the patch the coefficient is being evaluated on.<br><br># Upstream asymmetry, reproduced<br><br>Upstream's spherical conduction length is `r1²·(1/r_in − 1/r_out)`,<br>using the **current patch's** radius squared, not `r_in²`. The<br>textbook shell resistance referred to the inner surface uses `r_in²`.<br>Consequently each side of the interface computes a different `h_gas`,<br>and `updateCoeffs()` averages the two<br>([`average_across_interface`]). Set this to `inner_radius` to recover<br>the textbook inner-referred form. |
| `inner_radius` | `f64` | Inner radius of the gap \[m\], **radial**. Must be `> 0`. |
| `outer_radius` | `f64` | Outer radius of the gap \[m\], **radial**. Must be `>= inner_radius`. |
| `jump_distance` | `TrisoJumpDistance` | Where the temperature-jump distance comes from. |

##### Implementations

###### Methods

- ```rust
  pub fn evaluate(self: &Self, gas: &GapGasMixture, gas_pressure: f64, surfaces: &GapSurfaces) -> GapConductance { /* ... */ }
  ```
  Evaluate the three parallel conductance terms \[W/m²K\].

- ```rust
  pub fn evaluate_checked(self: &Self, gas: &GapGasMixture, gas_pressure: f64, surfaces: &GapSurfaces) -> Result<GapConductance> { /* ... */ }
  ```
  [`Self::evaluate`], but rejecting inputs it would have had to guard.

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
    fn clone(self: &Self) -> GapConductanceModel { /* ... */ }
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
    fn eq(self: &Self, other: &GapConductanceModel) -> bool { /* ... */ }
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

#### Function `temperature_jump_distance`

**Attributes:**

- `MustUse { reason: None }`

Temperature-jump distance \[m\], **radial** — upstream's `d_jump`.

```text
d_jump = 0.0137 · k · sqrt(T) / (p · a)
```

# What it represents

Gas molecules leaving a solid surface do not carry the surface's full
temperature, so a discontinuity sits at each wall. Its thermal effect is the
same as adding this much extra gas thickness to the gap. It grows with gas
conductivity and temperature and *falls* with pressure — which is why a
depressurised rod has a much worse gap than the geometry alone suggests.

# Arguments

- `conductivity` — gas mixture conductivity \[W/m/K\] at the film temperature.
- `temperature` — film temperature \[K\], the mean of the two surfaces.
- `gas_pressure` — rod internal gas pressure \[Pa\].
- `accommodation` — **upstream's** accommodation coefficient from
  [`GapGasMixture::accommodation_coefficient`]. Note that it is not
  dimensionless (see that method's documented upstream defect); the constant
  `0.0137` absorbs the scaling.

Returns `0.0` if any input is non-positive or non-finite, rather than an
infinity.

```rust
pub fn temperature_jump_distance(conductivity: f64, temperature: f64, gas_pressure: f64, accommodation: f64) -> f64 { /* ... */ }
```

#### Function `effective_roughness_distance`

**Attributes:**

- `MustUse { reason: None }`

Effective roughness thickness \[m\], **radial** — upstream's `d_eff`.

```text
d_eff = exp(−1.25e−3 · P_kgf) · (R_fuel + R_clad)
```

where `P_kgf` is the interface pressure in kilogram-force per square
centimetre. Gas trapped in the asperity valleys adds to the conduction path;
pressing the surfaces together squashes the asperities and removes it, hence
the exponential decay with contact pressure.

# Arguments

- `fuel_roughness`, `clad_roughness` — per-surface arithmetic-mean roughness
  \[m\], **radial**. They are summed, in contrast to the root-sum-square
  combination the contact term uses; both are upstream's.
- `interface_pressure` — contact pressure \[Pa\], `>= 0`.

Negative or non-finite inputs are treated as zero.

```rust
pub fn effective_roughness_distance(fuel_roughness: f64, clad_roughness: f64, interface_pressure: f64) -> f64 { /* ... */ }
```

#### Function `spherical_gap_conduction_length`

**Attributes:**

- `MustUse { reason: None }`

Spherical-shell gas conduction length \[m\], **radial** — upstream's
`gap_sphere` in `trisoGapFvPatchScalarField::hGap()`.

```text
L = max( r_ref² · (1/r_in − 1/r_out), 0 )
```

This is the planar-equivalent thickness of a spherical shell: dividing the
gas conductivity by it gives a conductance referred to the surface of radius
`r_ref`. For a thin shell it tends to `r_out − r_in`, the planar gap width;
for a thick one it does not, which is the whole point of the spherical form
in a TRISO particle where the coating thicknesses are a large fraction of the
radius.

See [`GapConductanceModel::TrisoSpherical::reference_radius`] for why
`r_ref` is a separate argument and not simply `r_in`.

Returns `0.0` for non-positive or non-finite radii, or if `r_out < r_in`.

```rust
pub fn spherical_gap_conduction_length(reference_radius: f64, inner_radius: f64, outer_radius: f64) -> f64 { /* ... */ }
```

#### Function `radiative_conductance`

**Attributes:**

- `MustUse { reason: None }`

Linearised gray-body radiative conductance \[W/m²K\] — upstream's `hRad`.

```text
h_rad = σ (T₁ + T₂)(T₁² + T₂²) / (1/ε₁ + 1/ε₂ − 1)
```

# Why this form

The net exchange between two gray surfaces is
`q'' = σ (T₁⁴ − T₂⁴) / (1/ε₁ + 1/ε₂ − 1)`. Factoring
`T₁⁴ − T₂⁴ = (T₁ + T₂)(T₁² + T₂²)(T₁ − T₂)` leaves exactly the expression
above multiplied by `(T₁ − T₂)`, so `h_rad` is an *exact* linearisation, not
an approximation — it can be used as a conductance in a linear solve without
introducing error, provided it is re-evaluated as the temperatures change.

# Assumption, stated because it is not obviously right here

The denominator `1/ε₁ + 1/ε₂ − 1` is the **infinite-parallel-plate** view
factor. For concentric cylinders the correct form is
`1/ε₁ + (A₁/A₂)(1/ε₂ − 1)`. Upstream uses the plate form for both the rod and
the TRISO geometry; since a fuel/cladding gap is a few tens of microns across
a ~4 mm radius, `A₁/A₂ ≈ 1` and the two agree closely there. For a TRISO
shell, where the area ratio departs from 1, this is a genuine approximation.
It is reproduced rather than corrected.

Emissivities are floored at [`SMALL`], matching upstream, so a zero
emissivity gives zero radiative transfer. Returns `0.0` for non-positive
temperatures.

```rust
pub fn radiative_conductance(surfaces: &GapSurfaces) -> f64 { /* ... */ }
```

#### Function `contact_conductance`

**Attributes:**

- `MustUse { reason: None }`

Solid-contact conductance \[W/m²K\] — upstream's `hContact`, the
Ross–Stoute-style correlation used by both gap patch fields.

```text
P_rel  = P_interface / H_Meyer
k_m    = 2 k₁ k₂ / (k₁ + k₂)                        (harmonic mean)
R      = sqrt(R₁² + R₂²)                            (root-sum-square roughness)
R_f    = ½ (R₁ + R₂)                                (mean roughness)
R_mult = 333.3 · P_rel     if P_rel ≤ 0.0087, else 2.9
E      = exp(5.738 − 0.528 · ln(3.937e7 · R_f))

h_contact = 0.4166 · k_m · P_rel · R_mult / (R · E)   for P_rel > 0.003
          = 0.00125 · k_m / (R · E)                   for 9e−6 < P_rel ≤ 0.003
          = 0.4166 · k_m · sqrt(P_rel) / (R · E)      for P_rel ≤ 9e−6
```

# Reading the branches

They are the three asperity-deformation regimes: elastic at very low relative
pressure (`sqrt(P_rel)`), a plateau, then plastic flattening where the real
contact area grows in proportion to load. **The three branches join
continuously**: at `P_rel = 9e−6` the elastic branch gives
`0.4166·3e−3 = 1.2498e−3 ≈ 0.00125`, and at `P_rel = 0.003` the plastic
branch gives `0.4166·0.003·0.9999 = 1.2497e−3 ≈ 0.00125`. That continuity is
asserted in the tests, and it is the reason the constants look arbitrary.

The `3.937e7` inside `E` converts metres to microinches
(`1 m = 3.937e7 µin`) — another sign of an imperial-unit fit.

# Behaviour at the limits

- **Zero interface pressure**: returns exactly `0.0`. An open gap conducts no
  heat through contact, by definition.
- **Hard contact**: grows linearly in interface pressure, and dominates the
  gas and radiation terms by an order of magnitude or more.

Returns `0.0` for non-finite or degenerate inputs (zero combined roughness,
zero conductivities) rather than an infinity.

```rust
pub fn contact_conductance(surfaces: &GapSurfaces) -> f64 { /* ... */ }
```

#### Function `series_conductance`

**Attributes:**

- `MustUse { reason: None }`

Effective interface conductance \[W/m²K\] of three resistances in series —
upstream's `alphaEff` in `resistiveGapFvPatchScalarField::weights()`.

```text
1/h_eff = 1/h_fuel_wall + 1/h_clad_wall + 1/h_gap
```

The gap resistance sits between the two half-cell wall resistances, so the
three add as resistances (reciprocals), **not** as conductances. This is the
counterpart to the three gap *paths*, which are parallel and do add directly
([`GapConductance::total`]); confusing the two is the classic error here.

Use [`wall_conductance`] to build the two wall terms from a cell
conductivity and its wall distance.

A non-positive term is treated as an infinite resistance, so the result is
`0.0` if any of the three is zero.

```rust
pub fn series_conductance(fuel_wall: f64, clad_wall: f64, gap: f64) -> f64 { /* ... */ }
```

#### Function `wall_conductance`

**Attributes:**

- `MustUse { reason: None }`

Half-cell wall conductance \[W/m²K\] — upstream's `kappa()/deltas` in
`resistiveGapFvPatchScalarField::weights()`.

`h = k / δ`, where `δ` is the normal distance from the cell centre to the
boundary face (`patch.nf() & patch.delta()` upstream). Returns `0.0` for a
non-positive distance or conductivity.

# Deferred

`δ` itself comes from the mesh; this function does the arithmetic only.

```rust
pub fn wall_conductance(conductivity: f64, wall_distance: f64) -> f64 { /* ... */ }
```

#### Function `average_across_interface`

**Attributes:**

- `MustUse { reason: None }`

Arithmetic mean of the owner and neighbour values on an interface —
upstream's `0.5*(hGap() + interpolate(nbr.hGap()))` in `updateCoeffs()`.

Both gap patch fields evaluate the whole model twice, once from each side
(the two sides see different temperatures, different solid conductivities and
— for TRISO — different reference radii), then average. This function is that
average.

# Deferred

The AMI interpolation that brings the neighbour's value onto this patch's
faces. This function assumes the two values already refer to the same face.

```rust
pub fn average_across_interface(owner: f64, neighbour: f64) -> f64 { /* ... */ }
```

#### Function `under_relax`

**Attributes:**

- `MustUse { reason: None }`

Explicit under-relaxation of a gap conductance between outer iterations —
upstream's `hGap_ = relax_*(...) + (1 - relax_)*hGap_`.

Gap conductance and temperature are strongly and non-linearly coupled: a
hotter pellet expands, narrows the gap, raises the conductance, and cools
again. Solved without relaxation that loop oscillates. `factor = 1` is no
relaxation (upstream's default); smaller values damp the loop.

# Arguments

- `new`, `previous` — conductances \[W/m²K\] from this and the last outer
  iteration.
- `factor` — relaxation factor \[-\], clamped to `[0, 1]`.

```rust
pub fn under_relax(new: f64, previous: f64, factor: f64) -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `STEFAN_BOLTZMANN`

Stefan–Boltzmann constant `σ` \[W/(m²·K⁴)\], SI-2019 exact value.

Upstream reads `Foam::constant::physicoChemical::sigma`.

```rust
pub const STEFAN_BOLTZMANN: f64 = 5.670_374_419e-8;
```

#### Constant `SMALL`

Small positive length/denominator floor \[SI\] used where upstream would
divide by zero.

OpenFOAM's `SMALL` for double precision. Upstream's `trisoGap` guards its
divisions with it; upstream's `fuelRodGap` does **not**, and would return an
infinity for a perfectly smooth, perfectly closed, zero-jump-distance gap.
This port applies the guard in both, so a degenerate input yields a very
large finite number rather than an infinity that then poisons a linear solve.
This is the module's only numerical deviation from upstream, and it can only
fire on inputs upstream would have turned into `inf` or `NaN`.

```rust
pub const SMALL: f64 = 1.0e-15;
```

#### Constant `ROUGHNESS_OFFSET`

Empirical offset \[m\] subtracted from the roughness-augmented gap width —
upstream's literal `1.397e-6` in `fuelRodGap::hGap()`.

It is 55 microinches expressed in metres (`55e-6 in × 0.0254 m/in`), which is
the tell that the correlation was fitted in imperial units. It exists to
stop the roughness term over-predicting the gas gap in near-contact
conditions; the sum is clipped at zero, so it can never make the effective
gap negative.

```rust
pub const ROUGHNESS_OFFSET: f64 = 1.397e-6;
```

#### Constant `JUMP_DISTANCE_MULTIPLIER`

Multiplier on the temperature-jump distance in the fuel-rod gas term —
upstream's literal `1.8` in `fuelRodGap::hGap()`.

There is one jump at each of the two surfaces, so a factor of 2 would be the
naive count; 1.8 is the fitted value. **The TRISO variant does not apply
it** — `trisoGap` uses the bare jump distance — and that difference is
reproduced.

```rust
pub const JUMP_DISTANCE_MULTIPLIER: f64 = 1.8;
```

#### Constant `JUMP_DISTANCE_COEFFICIENT`

Coefficient of the temperature-jump-distance correlation \[SI-mixed\] —
upstream's literal `0.0137`.

Appears as `d_jump = 0.0137 · k · sqrt(T) / (p · a)`. It is an empirical
constant fitted *with upstream's unnormalised accommodation coefficient*
(see [`GapGasMixture::accommodation_coefficient`]) already folded in, which
is why this port reproduces that quirk rather than "fixing" it.

```rust
pub const JUMP_DISTANCE_COEFFICIENT: f64 = 0.0137;
```

#### Constant `PRESSURE_TO_KGF_PER_CM2`

Divisor \[Pa per kgf/cm²\] converting interface pressure to the unit the
roughness-compression term expects — upstream's literal `1e4 · 9.8`.

`pI = interfaceP / 1e4 / 9.8` converts pascal to kilogram-force per square
centimetre (the exact conversion is 9.80665e4 Pa; upstream rounds to 9.8e4,
a 0.07% difference, reproduced here).

```rust
pub const PRESSURE_TO_KGF_PER_CM2: f64 = _;
```

#### Constant `ROUGHNESS_PRESSURE_COEFFICIENT`

Exponential decay coefficient \[per kgf/cm²\] of the roughness contribution
under contact pressure — upstream's literal `1.25e-3`.

```rust
pub const ROUGHNESS_PRESSURE_COEFFICIENT: f64 = 1.25e-3;
```

#### Constant `MEYER_HARDNESS_ZIRCALOY`

Meyer hardness of Zircaloy cladding \[Pa\] — upstream's hard-coded
`680e6` in both gap patch fields.

The contact model's relative pressure is `P_rel = P_interface / H_Meyer`; the
harder material of the pair sets how much the asperities flatten. Upstream
hard-codes the Zircaloy value and offers no way to change it; this port
exposes it as [`GapSurfaces::meyer_hardness`] with this constant as the
default, which is a **deliberate widening** of upstream's interface, not a
change to its default behaviour.

```rust
pub const MEYER_HARDNESS_ZIRCALOY: f64 = 680.0e6;
```

## Module `contact`

Mechanical fuel/cladding contact: the penalty interface pressure.

# What this computes

The **normal interface pressure** \[Pa\] that the fuel and cladding exert on
each other once the gap has closed. It is the mechanical half of the closure
loop, and it is what the thermal half — [`super::conductance`] — needs as
[`GapSurfaces::interface_pressure`](super::conductance::GapSurfaces::interface_pressure).

# The penalty formulation

Upstream does not solve a constrained contact problem. It lets the two bodies
interpenetrate slightly and charges a pressure proportional to the
penetration:

```text
P = max(−k_penalty · g, 0)      for g < 0 (penetration)
P = 0                           for g > 0 (open gap)
```

where `g` is the **signed radial gap width** and `k_penalty` \[Pa/m\] is a
stiffness derived from the material and the local mesh spacing. A larger
penalty enforces the non-penetration constraint more tightly but conditions
the linear system worse; upstream's default scale factor is 0.1.

# Gap convention — signed, and RADIAL

**`signed_radial_gap` is positive when the gap is open and NEGATIVE when the
two surfaces interpenetrate.** This is the opposite information content from
the thermal side, which clips at zero — and it is deliberate: the penalty
formulation is driven entirely by the *amount* of interpenetration, which the
thermal side throws away. It is a **radial** normal separation, not a
diametral one; see the [module-level conventions]
(super#gap-conventions--read-this-before-using-anything-here).

Upstream computes it as `(C_nbr + D_nbr − C_own − D_own) · n` on each
interface face, without clipping.

# Deferred

- **The gap-width evaluation itself** (deformed face centres, face normals,
  AMI interpolation of the neighbour patch). Taken as an input here.
- **Friction and the tangential traction** — upstream's slip/stick update,
  `frictionCoeff_`, `penaltyScaleFactFric_` and
  [`boundary_shear_stiffness`]'s consumers. Only the shear-stiffness
  arithmetic is ported; the slip integration is not.
- **The `rigidMasterNormal_` master/slave choice** and the owner-side
  interpolation of the contact pressure back onto the neighbour patch.

# Units

Strict SI raw `f64`: metre, pascal, m², m³, Pa/m for a stiffness.

```rust
pub mod contact { /* ... */ }
```

### Types

#### Struct `PenaltyContact`

Penalty contact parameters — upstream's `contactFvPatchVectorField`
dictionary entries `penaltyFactor`, `relativePenetrationTolerance` and
`relaxInterfacePressure`.

# Units

All three fields are dimensionless. [`Default`] reproduces upstream's
defaults exactly.

```rust
pub struct PenaltyContact {
    pub penalty_scale: f64,
    pub penetration_tolerance: f64,
    pub relaxation: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `penalty_scale` | `f64` | Scale factor \[-\] applied to the smaller of the two boundary stiffnesses<br>— upstream's `penaltyFactor`, default `0.1`.<br><br>Larger means a stiffer contact and less interpenetration, at the cost of<br>a worse-conditioned displacement solve. Must be `> 0` for the constraint<br>to be enforced at all. |
| `penetration_tolerance` | `f64` | Relative penetration below which the pressure is **not updated** —<br>upstream's `relativePenetrationTolerance`, default `0.0`.<br><br>The test is `−g / δ > tolerance`, with `δ` the average cell spacing<br>across the interface, so the tolerance is a penetration expressed as a<br>fraction of a cell. See [`Self::interface_pressure`] for the important<br>consequence of "not updated" meaning *retained*, not *zeroed*. |
| `relaxation` | `f64` | Under-relaxation factor \[-\] applied to the pressure between outer<br>iterations — upstream's `relaxInterfacePressure`, default `1.0` (no<br>relaxation). Clamped to `[0, 1]`. |

##### Implementations

###### Methods

- ```rust
  pub fn penalty_factor(self: &Self, fuel_stiffness: f64, clad_stiffness: f64) -> f64 { /* ... */ }
  ```
  Penalty stiffness \[Pa/m\] for one interface face — upstream's

- ```rust
  pub fn interface_pressure(self: &Self, signed_radial_gap: f64, penalty_factor: f64, average_cell_spacing: f64, previous: f64) -> f64 { /* ... */ }
  ```
  Normal interface pressure \[Pa\] on one interface face — the normal

- ```rust
  pub fn interface_pressure_no_latch(self: &Self, signed_radial_gap: f64, penalty_factor: f64, average_cell_spacing: f64, previous: f64) -> f64 { /* ... */ }
  ```
  [`Self::interface_pressure`] without the latching behaviour: a

- ```rust
  pub fn validate(self: &Self) -> Result<()> { /* ... */ }
  ```
  Reject unusable parameters.

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
    fn clone(self: &Self) -> PenaltyContact { /* ... */ }
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
    Upstream's defaults: `penalty_scale = 0.1`, `penetration_tolerance = 0`,

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
    fn eq(self: &Self, other: &PenaltyContact) -> bool { /* ... */ }
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

#### Function `boundary_stiffness`

**Attributes:**

- `MustUse { reason: None }`

Boundary normal stiffness \[Pa/m\] of a cell touching the interface —
upstream's `contactFvPatchVectorField::boundaryStiffness()`.

```text
k = K · A / V
```

where `K = 3K/3` is the bulk modulus \[Pa\], `A` the interface face area
\[m²\] and `V` the volume \[m³\] of the cell behind that face. Dimensionally
this is a pressure per unit displacement: it is the stiffness the cell
presents to being squashed normal to the face, so it is the natural scale for
a penalty that must be stiff relative to the material but not so stiff that
it destroys the conditioning of the displacement solve.

# Arguments

- `bulk_modulus` — `K` \[Pa\]. Build it from
  [`LinearElastic::three_k`](crate::mechanics::LinearElastic::three_k)
  divided by three, which is exactly what upstream does with its `threeK`
  patch field.
- `face_area` — interface face area \[m²\], `> 0`.
- `cell_volume` — volume \[m³\] of the cell behind the face, `> 0`.

Returns `0.0` for a non-positive volume rather than an infinity.

```rust
pub fn boundary_stiffness(bulk_modulus: f64, face_area: f64, cell_volume: f64) -> f64 { /* ... */ }
```

#### Function `boundary_shear_stiffness`

**Attributes:**

- `MustUse { reason: None }`

Boundary shear stiffness \[Pa/m\] of a cell touching the interface —
upstream's `contactFvPatchVectorField::boundaryShearStiffness()`.

Identical in form to [`boundary_stiffness`] but built from the shear modulus
`μ` \[Pa\] instead of the bulk modulus, and used to scale the *friction*
penalty rather than the normal one.

# Deferred

The friction model this feeds is not ported; the function is here because it
is a one-line pure function and omitting it would leave a visible hole beside
its normal-stiffness twin.

```rust
pub fn boundary_shear_stiffness(shear_modulus: f64, face_area: f64, cell_volume: f64) -> f64 { /* ... */ }
```

#### Function `total_surface_pressure`

**Attributes:**

- `MustUse { reason: None }`

Total normal pressure \[Pa\] on a gap-facing surface — upstream's
`gapContactFvPatchVectorField::updateTraction()`.

```text
P_total = P_contact + P_gas
```

The fill gas presses outward on the cladding and inward on the fuel
everywhere, whether or not the surfaces touch; the contact pressure adds to
it only where they do. Keeping the two separate matters because
[`super::conductance`] must be given the **contact** pressure alone — the gas
pressure does not flatten asperities and must not enter the contact
correlation.

# Note

Upstream carries a `TODO: should the gapGas pressure disappear?` beside this
addition, i.e. its authors were unsure whether the gas pressure should be
dropped once the gap is fully closed and the gas is no longer in contact with
that surface. This port reproduces the current behaviour (always added) and
records the open question rather than resolving it.

```rust
pub fn total_surface_pressure(contact_pressure: f64, gas_pressure: f64) -> f64 { /* ... */ }
```

## Module `free_volume`

Rod free volume and the internal gas pressure.

# What this computes

The pressure of the gas inside a fuel rod. That pressure matters twice over:
it loads the cladding from the inside (and if it exceeds the coolant pressure,
the cladding creeps *outwards*, reopening the gap), and it divides the
temperature-jump distance in the gap conductance
([`super::conductance::temperature_jump_distance`]), so a depressurised rod
has a worse gap than its geometry suggests.

# The model

Upstream (`gapFRAPCON`) makes three assumptions, stated in its own class
documentation:

1. the gas is ideal;
2. each part of the free volume has its **own** volume and temperature;
3. the pressure equalises **instantaneously** everywhere in the free volume.

Assumption 3 is what makes this a single scalar rather than a field. Applying
`pV = nRT` to each region at a common pressure and summing the amounts gives

```text
p = n R / Σᵢ (Vᵢ / Tᵢ)
```

That `Σ V/T` — not `V_total/T_mean` — is the whole content of the model. The
two differ whenever the regions are at different temperatures, and in a rod
they always are: the gap runs hot, the plenum runs near coolant temperature.

# The regions

Upstream tracks, and this module represents: the fuel/cladding **gap**, a
user-supplied gap volume **offset**, the fuel central **hole**, the pellet
**dishes**, the **top** and **bottom plena**, an external gas **reserve**, and
the **cracks** in the relocated fuel.

# Deferred — this module does not compute volumes

Every one of those volumes is computed upstream by walking the mesh:

- gap, hole and plenum volumes come from the Gauss–Green surface integral
  `V = ⅓ ∮_S (r_s · n) dS` over the **deformed** bounding patches, with
  per-face scaling factors built from cutting-plane/edge intersections to
  separate the gap from the plena on a non-conformal cylindrical interface;
- the region temperatures are face- or cell-area/volume-weighted averages of
  the temperature field;
- the dish and crack volumes are cell-volume sums over the fuel material.

All of that needs mesh topology and the multi-region coupling, and is
**deferred**. [`RodFreeVolume`] takes the resulting `(V, T)` or `(V, Σ V/T)`
pairs as inputs and does the thermodynamics. The two cell-level *summands*
that are pure arithmetic — [`crack_volume_contribution`] and
[`dish_volume_contribution`] — are ported, so a caller who has cell volumes
can build those two sums itself.

# Units

Strict SI raw `f64`: m³, kelvin, pascal, mole, kg, m³/K for a `V/T`.

```rust
pub mod free_volume { /* ... */ }
```

### Types

#### Struct `FreeVolumeRegion`

One region of the rod free volume: how big it is and how hot.

Upstream carries each region as a pair of scalars — a volume and either a
temperature or a pre-accumulated `Σ V/T`. Which of the two depends on the
region: the plena and the gas reserve are treated as **isothermal**
(`V/T` computed from a single mean temperature), while the gap, hole, dish
and cracks accumulate `Σ Vᵢ/Tᵢ` face-by-face or cell-by-cell over a
non-uniform temperature. Both forms are representable here, and the
distinction is preserved rather than flattened, because flattening it would
change the pressure.

# Units

[`volume`](Self::volume) in m³, [`v_over_t`](Self::v_over_t) in m³/K.

```rust
pub struct FreeVolumeRegion {
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
  pub fn uniform(name: &'static str, volume: f64, temperature: f64) -> Result<Self> { /* ... */ }
  ```
  An **isothermal** region: volume `volume` \[m³\] all at temperature

- ```rust
  pub fn distributed(name: &'static str, volume: f64, v_over_t: f64) -> Result<Self> { /* ... */ }
  ```
  A region with a **non-uniform** temperature, given its volume \[m³\] and

- ```rust
  pub fn empty(name: &'static str) -> Self { /* ... */ }
  ```
  An empty region — zero volume, zero `V/T`. Contributes nothing.

- ```rust
  pub fn name(self: &Self) -> &'static str { /* ... */ }
  ```
  The region's name, for diagnostics.

- ```rust
  pub fn volume(self: &Self) -> f64 { /* ... */ }
  ```
  Region volume \[m³\].

- ```rust
  pub fn v_over_t(self: &Self) -> f64 { /* ... */ }
  ```
  Region `Σ V/T` \[m³/K\] — the quantity that actually enters the pressure.

- ```rust
  pub fn effective_temperature(self: &Self) -> f64 { /* ... */ }
  ```
  Effective (harmonic-mean) temperature \[K\] of the region, `V / (V/T)`.

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
    fn clone(self: &Self) -> FreeVolumeRegion { /* ... */ }
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
    fn eq(self: &Self, other: &FreeVolumeRegion) -> bool { /* ... */ }
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
#### Struct `RodFreeVolume`

The complete free volume of one rod.

A list of [`FreeVolumeRegion`]s plus the thermodynamics that turns them into
a pressure. Mirrors the eight scalars-plus-eight-temperatures upstream
carries on `gapFRAPCON`, but named and extensible rather than hard-coded, so
a caller modelling a rod without a central hole simply omits that region
instead of setting a magic zero.

# Units

m³ and m³/K, as the regions.

```rust
pub struct RodFreeVolume {
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
  An empty free volume — no regions. Its pressure is undefined until at

- ```rust
  pub fn with_region(self: Self, region: FreeVolumeRegion) -> Self { /* ... */ }
  ```
  Add a region, consuming and returning `self` for chained construction.

- ```rust
  pub fn push(self: &mut Self, region: FreeVolumeRegion) { /* ... */ }
  ```
  Add a region in place.

- ```rust
  pub fn regions(self: &Self) -> &[FreeVolumeRegion] { /* ... */ }
  ```
  The regions, in insertion order.

- ```rust
  pub fn total_volume(self: &Self) -> f64 { /* ... */ }
  ```
  Total free volume \[m³\] — the plain sum of the region volumes.

- ```rust
  pub fn total_v_over_t(self: &Self) -> f64 { /* ... */ }
  ```
  Total `Σ (Vᵢ / Tᵢ)` \[m³/K\] over all regions — the denominator of the

- ```rust
  pub fn effective_temperature(self: &Self) -> f64 { /* ... */ }
  ```
  Effective (harmonic-mean) gas temperature \[K\] of the whole free volume,

- ```rust
  pub fn volume_weighted_temperature(self: &Self) -> f64 { /* ... */ }
  ```
  Volume-weighted mean gas temperature \[K\], `Σ VᵢTᵢ / Σ Vᵢ` — upstream's

- ```rust
  pub fn pressure(self: &Self, moles: f64) -> f64 { /* ... */ }
  ```
  Internal gas pressure \[Pa\] for `moles` \[mol\] of gas —

- ```rust
  pub fn pressure_checked(self: &Self, moles: f64) -> Result<f64> { /* ... */ }
  ```
  [`Self::pressure`], reporting the degenerate cases instead of returning

- ```rust
  pub fn initial_mass(self: &Self, gas: &GapGasMixture, p: f64) -> f64 { /* ... */ }
  ```
  Initial gas mass \[kg\] that fills this rod at pressure `p` \[Pa\] —

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
    fn clone(self: &Self) -> RodFreeVolume { /* ... */ }
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
    fn default() -> RodFreeVolume { /* ... */ }
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
    fn eq(self: &Self, other: &RodFreeVolume) -> bool { /* ... */ }
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
#### Enum `GasPressureModel`

How the rod internal pressure is obtained — upstream's `gasPressureType`
entry (`fromModel` / `fixed` / `fromList`).

Dispatch is by `match`, never by a trait object, per the workspace
`CLAUDE.md` "No trait objects" rule.

```rust
pub enum GasPressureModel {
    FromFreeVolume,
    Fixed(f64),
    Tabulated(Vec<(f64, f64)>),
}
```

##### Variants

###### `FromFreeVolume`

Compute it from the free volume and the gas inventory — upstream's
`fromModel`, the physically meaningful choice.

###### `Fixed`

Hold it at a constant \[Pa\] — upstream's `fixed`.

Useful to isolate the thermal problem from the pressure feedback in a
verification case; not a model of a real rod, whose pressure rises by a
factor of several through life.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

###### `Tabulated`

Read it from a time table — upstream's `fromList` and the
`gapGasTimeTabulated` model.

Pairs of `(time \[s\], pressure \[Pa\])`, which **must be sorted by
increasing time**. Interpolation is linear between points and **clamped**
outside the table (upstream's `outOfBounds clamp` default), i.e. the
first and last values are held rather than extrapolated. This is how a
pressure history measured or computed by another fuel-performance code is
imposed on an OFFBEAT run.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<(f64, f64)>` |  |

##### Implementations

###### Methods

- ```rust
  pub fn pressure(self: &Self, time: f64, free_volume: &RodFreeVolume, moles: f64) -> f64 { /* ... */ }
  ```
  The gas pressure \[Pa\] at time `time` \[s\].

- ```rust
  pub fn validate(self: &Self) -> Result<()> { /* ... */ }
  ```
  Reject an unusable configuration.

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
    fn clone(self: &Self) -> GasPressureModel { /* ... */ }
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
    fn eq(self: &Self, other: &GasPressureModel) -> bool { /* ... */ }
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

#### Function `crack_volume_contribution`

**Attributes:**

- `MustUse { reason: None }`

Crack free volume \[m³\] contributed by one fuel cell — upstream's
`correctCrack()`.

```text
V_crack = 2 · ε_relocation · V_cell
```

# Where the factor of two comes from

Upstream's comment says only *"the following result comes from supposing
relocation as a 2D phenomenon"*. The reading that makes the algebra work: the
relocation strain `ε` is a **radial** strain (see
[`crate::materials::behavioral::relocation`]), and a 2D radial expansion of a
disc by `ε` increases its area by `(1+ε)² − 1 ≈ 2ε` to first order. The
crack volume opened up is that areal increase times the cell height, i.e.
`2 ε V_cell`. The factor is *not* the 3 of a volumetric strain, and it is not
arbitrary.

# Arguments

- `relocation_strain` — the **radial** relocation strain `ε` \[-\], positive
  outward, from
  [`RelocationModel::value`](crate::materials::behavioral::relocation::RelocationModel::value).
- `cell_volume` — the fuel cell's volume \[m³\].

Negative or non-finite inputs contribute zero.

```rust
pub fn crack_volume_contribution(relocation_strain: f64, cell_volume: f64) -> f64 { /* ... */ }
```

#### Function `dish_volume_contribution`

**Attributes:**

- `MustUse { reason: None }`

Dish free volume \[m³\] contributed by one fuel cell — upstream's
`correctDish()`.

`V_dish = f_dish · V_cell`, where `f_dish` \[-\] is the fuel material's dish
fraction: the fraction of a pellet's nominal volume removed by the dishes and
chamfers machined into its end faces. Typical LWR values are a few percent.

Negative or non-finite inputs contribute zero; the fraction is not clamped
above, so a caller supplying a fraction above 1 gets an unphysical answer
rather than a silent clamp — upstream does not clamp either.

```rust
pub fn dish_volume_contribution(dish_fraction: f64, cell_volume: f64) -> f64 { /* ... */ }
```

#### Function `interpolate_clamped`

**Attributes:**

- `MustUse { reason: None }`

Linear interpolation in a `(x, y)` table, **clamped** outside its range —
upstream's `Function1s::Table` with `outOfBounds clamp`.

The table must be sorted by increasing `x`; [`GasPressureModel::validate`]
checks that. Below the first point the first `y` is returned and above the
last point the last `y`, so a history never extrapolates into a nonsense
pressure. Returns `0.0` for an empty table.

Exposed because upstream applies the same rule to its
`gasReserveTemperatureList` and to the tabulated mass fractions of
`gapGasTimeTabulated`, not only to the pressure.

```rust
pub fn interpolate_clamped(table: &[(f64, f64)], x: f64) -> f64 { /* ... */ }
```

## Module `gas`

The fill-gas / fission-gas mixture in the fuel/cladding gap.

# What this computes

A fuel rod is filled with helium at fabrication. As it burns, fission gas —
overwhelmingly **xenon** with some **krypton** — is released from the fuel
matrix into the free volume and dilutes that helium. Xenon conducts heat
roughly twenty times worse than helium at the same temperature, so the
dilution degrades gap conductance, raises fuel temperature, and accelerates
further release. This module holds the composition and the two mixture
properties the gap conductance needs from it:

- [`GapGasMixture::conductivity`] — the mixture thermal conductivity \[W/m/K\],
- [`GapGasMixture::accommodation_coefficient`] — the thermal accommodation
  coefficient that sets the temperature-jump distance at each surface.

# Which gases

Exactly the six noble gases upstream tabulates: helium, neon, argon,
krypton, xenon and radon (see [`GapGasSpecies`]). Helium is the fill gas;
xenon and krypton are the fission products; argon and neon appear as
alternative fill gases in experimental rods; radon is tabulated by upstream
but is not a meaningful rod constituent.

**Upstream defect reproduced:** upstream's default conductivity coefficients
for **neon and radon are placeholders** — `A = 1.0`, `B = 1.0`, giving
`k = T` W/m/K, which at 500 K is 500 W/m/K, four orders of magnitude too
high. The numbers are reproduced bit-for-bit so a comparison against an
OFFBEAT run is not silently shifted, but
[`GapGasSpecies::has_placeholder_conductivity`] flags them and
[`GapGasMixture::conductivity_checked`] refuses a mixture that contains them.

# Which mixing rule

The **Lindsay–Bromley form of the Wassiljewa equation**, i.e. a
mole-fraction-weighted sum with binary interaction factors:

```text
k_i  = A_i · T^(B_i)
φ_ij = [1 + (k_i/k_j)^(1/2) · (M_i/M_j)^(1/4)]²  /  [2^(3/2) · (1 + M_i/M_j)^(1/2)]
ψ_ij = φ_ij · [1 + 2.41 · (M_i − M_j)(M_i − 0.142 M_j) / (M_i + M_j)²]
k_mix = Σ_i  k_i x_i / ( x_i + Σ_{j≠i} ψ_ij x_j )
```

with `x` mole fractions and `M` molar masses. Upstream (`gapFRAPCON::kappa`)
attributes this to the FRAPCON-4.0 manual. Its one structural property worth
knowing: **it reduces exactly to `k_i` at `x_i = 1`**, because the inner sum
is then empty and the term is `k_i · 1 / 1`. That exactness is asserted in
the tests.

# Units

Strict SI (kelvin, W/m/K, kilogram, mole, pascal, m³) with the single
documented exception of [`GapGasSpecies::molar_mass_g_per_mol`].

```rust
pub mod gas { /* ... */ }
```

### Types

#### Enum `GapGasSpecies`

**Attributes:**

- `Repr(AttributeRepr { kind: Rust, align: None, packed: None, int: Some("usize") })`

A noble-gas component of the gap gas — upstream's `species_` entries.

The six species are exactly the keys of upstream `gapFRAPCON`'s default
`speciesW` / `conductivity_A` / `conductivity_B` dictionaries. The
discriminant is used as an array index throughout this module, so the order
is part of the type's contract and must not be reordered.

```rust
pub enum GapGasSpecies {
    Helium = 0,
    Neon = 1,
    Argon = 2,
    Krypton = 3,
    Xenon = 4,
    Radon = 5,
}
```

##### Variants

###### `Helium`

Helium — the as-fabricated fill gas of essentially every LWR rod.

Discriminant: `0`

Discriminant value: `0`

###### `Neon`

Neon — an alternative fill gas in some experimental rods.

**Its upstream conductivity coefficients are placeholders**; see
[`Self::has_placeholder_conductivity`].

Discriminant: `1`

Discriminant value: `1`

###### `Argon`

Argon — an alternative fill gas in some experimental rods.

Discriminant: `2`

Discriminant value: `2`

###### `Krypton`

Krypton — a released fission gas, roughly 10–15% of the released
fission-gas moles alongside xenon.

Discriminant: `3`

Discriminant value: `3`

###### `Xenon`

Xenon — the dominant released fission gas and the dominant cause of
gap-conductance degradation through life.

Discriminant: `4`

Discriminant value: `4`

###### `Radon`

Radon — tabulated by upstream; not a meaningful rod constituent.

**Its upstream conductivity coefficients are placeholders**; see
[`Self::has_placeholder_conductivity`].

Discriminant: `5`

Discriminant value: `5`

##### Implementations

###### Methods

- ```rust
  pub const fn index(self: Self) -> usize { /* ... */ }
  ```
  Array index of this species — its discriminant.

- ```rust
  pub const fn symbol(self: Self) -> &'static str { /* ... */ }
  ```
  Chemical symbol, matching upstream's dictionary keys (`"He"`, `"Xe"`, …).

- ```rust
  pub const fn molar_mass_g_per_mol(self: Self) -> f64 { /* ... */ }
  ```
  Molar mass \[**g/mol**\] — upstream's `speciesW` default dictionary.

- ```rust
  pub fn molar_mass(self: Self) -> f64 { /* ... */ }
  ```
  Molar mass \[kg/mol\] — the SI form of

- ```rust
  pub const fn conductivity_coefficients(self: Self) -> (f64, f64) { /* ... */ }
  ```
  Coefficients `(A, B)` of the pure-gas conductivity fit

- ```rust
  pub const fn has_placeholder_conductivity(self: Self) -> bool { /* ... */ }
  ```
  Whether this species' upstream conductivity coefficients are the

- ```rust
  pub fn conductivity(self: Self, t: f64) -> f64 { /* ... */ }
  ```
  Pure-gas thermal conductivity `k = A·T^B` \[W/m/K\] at temperature

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
    fn clone(self: &Self) -> GapGasSpecies { /* ... */ }
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

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &GapGasSpecies) -> bool { /* ... */ }
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
#### Struct `GapGasMixture`

The gap gas: what it is made of, and how much of it there is.

Mirrors the composition state upstream's `gapGasModel` carries — mass
fractions `Y_`, mole fractions `M_` and total mass `gasM_` — with the
normalisation invariants enforced at construction instead of by a
`correctMassFractions()` call the caller must remember to make.

# Invariants

Both fraction arrays sum to 1 (to within floating-point rounding) at all
times, and the mass is non-negative and finite. Every constructor and mutator
re-normalises, so these hold by construction.

# Units

Mass in kilogram, fractions dimensionless, everything derived in strict SI.

```rust
pub struct GapGasMixture {
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
  pub fn from_mass_fractions(mass_fractions: [f64; 6], mass: f64) -> Result<Self> { /* ... */ }
  ```
  Build from **mass** fractions \[-\] and a total mass \[kg\].

- ```rust
  pub fn pure(species: GapGasSpecies, mass: f64) -> Result<Self> { /* ... */ }
  ```
  A pure single-species gas of the given `mass` \[kg\].

- ```rust
  pub fn mass_fraction(self: &Self, species: GapGasSpecies) -> f64 { /* ... */ }
  ```
  Mass fraction \[-\] of one species.

- ```rust
  pub fn mole_fraction(self: &Self, species: GapGasSpecies) -> f64 { /* ... */ }
  ```
  Mole fraction \[-\] of one species — upstream's `M_`.

- ```rust
  pub fn mass_fractions(self: &Self) -> [f64; 6] { /* ... */ }
  ```
  All mass fractions \[-\], indexed by [`GapGasSpecies::index`].

- ```rust
  pub fn mole_fractions(self: &Self) -> [f64; 6] { /* ... */ }
  ```
  All mole fractions \[-\], indexed by [`GapGasSpecies::index`].

- ```rust
  pub fn mass(self: &Self) -> f64 { /* ... */ }
  ```
  Total gas mass \[kg\] — upstream's `gasM_`.

- ```rust
  pub fn moles(self: &Self) -> f64 { /* ... */ }
  ```
  Total gas amount \[mol\].

- ```rust
  pub fn specific_gas_constant(self: &Self) -> f64 { /* ... */ }
  ```
  Specific gas constant of the mixture \[J/(kg·K)\] — upstream's

- ```rust
  pub fn density(self: &Self, p: f64, t: f64) -> f64 { /* ... */ }
  ```
  Ideal-gas mixture density \[kg/m³\] at pressure `p` \[Pa\] and

- ```rust
  pub fn conductivity(self: &Self, t: f64) -> f64 { /* ... */ }
  ```
  Mixture thermal conductivity \[W/m/K\] at temperature `t` \[K\] —

- ```rust
  pub fn conductivity_checked(self: &Self, t: f64) -> Result<f64> { /* ... */ }
  ```
  [`Self::conductivity`], but refusing a mixture whose conductivity would

- ```rust
  pub fn accommodation_coefficient(self: &Self, t: f64) -> f64 { /* ... */ }
  ```
  Mixture thermal accommodation coefficient at temperature `t` \[K\] —

- ```rust
  pub fn add_released_gas(self: &mut Self, released: [f64; 6]) -> Result<()> { /* ... */ }
  ```
  Add fission gas released from the fuel, in **moles per species** —

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
    fn clone(self: &Self) -> GapGasMixture { /* ... */ }
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
    fn eq(self: &Self, other: &GapGasMixture) -> bool { /* ... */ }
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
### Constants and Statics

#### Constant `GAS_CONSTANT`

Universal gas constant `R` \[J/(mol·K)\], CODATA exact value.

Upstream reads `Foam::constant::physicoChemical::R`, which carries the same
SI-2019 exact definition.

```rust
pub const GAS_CONSTANT: f64 = 8.314_462_618_153_24;
```

#### Constant `N_SPECIES`

Number of gas species tracked — the length of every composition array here.

```rust
pub const N_SPECIES: usize = 6;
```

#### Constant `ACCOMMODATION_T_CAP`

Upper temperature \[K\] at which upstream freezes the accommodation-coefficient
correlations — `min(T, 1300)` in `gapFRAPCON::a`.

```rust
pub const ACCOMMODATION_T_CAP: f64 = 1300.0;
```

## Module `slice`

Axial slicing — the 1.5D / 2D / 3D mapping layer.

# What a slice mapper is for

A fuel rod is a long thin object, and much of fuel-performance physics is
**one-dimensional per axial level**: fission-gas release, relocation, the
FRAPCON gap-closure correlation and the axial power profile are all stated as
functions of a *slice-averaged* burnup, temperature or linear power. But the
mesh a case is solved on may be 1.5D (a stack of independent radial columns),
2D (an r–z axisymmetric mesh) or full 3D.

A slice mapper is what lets one implementation of those correlations serve
all three. It **partitions the cells into axial slices** and provides
volume-weighted averages over each. In 1.5D the partition is trivial — one
slice per column — and in 2D or 3D it collapses a whole ring or disc of cells
onto one number. The correlation itself never learns which mesh it is on.

That is the whole concept, and it is why this module exists in the [`gap`
module](super): the gap models consume slice-averaged quantities
(relocation strain per pellet, gas temperature per axial level), and a
difference in slicing changes the gap history.

# The three strategies

[`AxialSlicing`] has one variant per upstream `sliceMapper`:

| Variant | Upstream | How it slices |
|---|---|---|
| [`None`](AxialSlicing::None) | `sliceMapper` (`"none"`) | No slices at all. Upstream warns that models depending on a mapper — relocation among them — will conflict with this. |
| [`ByMaterial`](AxialSlicing::ByMaterial) | `sliceMapperByMaterial` | A fixed number of slices per material, of equal height or of explicitly-listed heights. |
| [`ByPellets`](AxialSlicing::ByPellets) | `sliceMapperByPellets` | One slice per **pellet** — heights derived by dividing the material height by the pellet count. |
| [`AutoAxial`](AxialSlicing::AutoAxial) | `sliceMapperAutoAxialSlices` | One slice per **distinct mesh axial level**, found by rounding cell-centre coordinates to a precision and grouping equal values. |

# Gap conventions

Nothing in this module is a gap width, so the radial/diametral question does
not arise. The one convention that does: **`axial_coordinate` is a length
\[m\] measured along the pin direction**, and `height_above_bottom` is that
coordinate minus the material's lowest point, so it is always `>= 0` for a
cell inside the material.

# Deferred

Upstream's slice mappers are half arithmetic and half mesh traversal. The
arithmetic is ported; the traversal is not:

- **Cell-to-material addressing** (`mat_.matAddrList()`) — which cells belong
  to which material. Taken as an input: the caller passes the coordinates of
  the cells it wants sliced.
- **The material extent** `h_min`, `h_max`, which upstream finds by walking
  every *point* of every cell (`mesh_.cellPoints()`), not the cell centres.
  [`AxialSlicing::assign`] takes `h_min` as an argument for that reason — it
  is deliberately *not* the minimum of the coordinates passed in, because the
  bottom cell's centre is above the material's bottom face.
- **The `isFuel` tagging** (`isA<fuelMaterial>`), the `sliceID` debug
  `volScalarField`, and the `topoChanging()` re-addressing trigger.
- **The parallel `Pstream` gather/scatter** that reconciles slice
  identities across processors, and the `reduce(sizeI, sumOp)` empty-slice
  check that depends on it. [`SliceAverage`] reports empty slices instead of
  aborting.

# Units

Strict SI raw `f64`: metre for coordinates and heights, m³ for cell volumes.
Averaged quantities carry whatever unit the caller's values carry.

```rust
pub mod slice { /* ... */ }
```

### Types

#### Enum `AxialSlicing`

How the cells of one material are partitioned into axial slices — one variant
per upstream `sliceMapper` implementation.

Dispatch is by `match`, never by a trait object, per the workspace
`CLAUDE.md` "No trait objects" rule.

```rust
pub enum AxialSlicing {
    None,
    ByMaterial {
        slice_heights: Vec<f64>,
    },
    ByPellets {
        pellet_heights: Vec<f64>,
    },
    AutoAxial {
        precision: f64,
    },
}
```

##### Variants

###### `None`

No slicing — upstream `sliceMapper`, `TypeName("none")`.

[`assign`](Self::assign) returns `None` for every cell and
[`n_slices`](Self::n_slices) returns `Some(0)`.

Selecting this is a real modelling decision, not a null one: upstream
warns that models depending on a mapper will conflict with it. The
relocation model in particular is stated per axial slice, so without a
mapper it has no linear power to branch on.

###### `ByMaterial`

A fixed set of slices per material — upstream `sliceMapperByMaterial`,
`TypeName("byMaterial")`.

Built either from a slice count (equal heights, upstream's `nSlices`) or
from an explicit list (upstream's `heightSlices`). Use
[`by_material_uniform`](Self::by_material_uniform) for the former.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `slice_heights` | `Vec<f64>` | Height \[m\] of each slice, bottom to top. Must all be `> 0`, and<br>upstream additionally requires their sum to equal the material height<br>to within 1e-6 m — checked by [`validate`](Self::validate). |

###### `ByPellets`

One slice per pellet — upstream `sliceMapperByPellets`,
`TypeName("byPellets")`.

Structurally identical to [`ByMaterial`](Self::ByMaterial) — upstream
derives equal pellet heights as `material_height / nPellets` — but it
**differs in what happens to a cell that falls in no bin**; see
[`assign`](Self::assign). Kept as a separate variant so that difference
cannot be lost.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `pellet_heights` | `Vec<f64>` | Height \[m\] of each pellet, bottom to top. |

###### `AutoAxial`

One slice per distinct mesh axial level — upstream
`sliceMapperAutoAxialSlices`, `TypeName("autoAxialSlices")`.

Cell-centre axial coordinates are rounded to `precision` and cells
sharing a rounded value form a slice; slices are ordered by increasing
coordinate. The slice count is therefore a property of the **mesh**, not
of this configuration, which is why [`n_slices`](Self::n_slices) returns
`None` for this variant and why [`assign`](Self::assign) must be used to
discover it.

This is the natural choice for a 1.5D mesh, where the axial levels are
already exactly the slices wanted.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `precision` | `f64` | Rounding precision \[m\] — upstream's `precision`, default `1e-6`.<br><br>Coordinates are grouped by `round(z / precision) · precision`. Too<br>coarse and distinct mesh levels merge; too fine and floating-point<br>noise splits one level into several. |

##### Implementations

###### Methods

- ```rust
  pub fn by_material_uniform(n_slices: usize, material_height: f64) -> Result<Self> { /* ... */ }
  ```
  Equal-height slices over a material of total height `material_height`

- ```rust
  pub fn by_pellets_uniform(n_pellets: usize, material_height: f64) -> Result<Self> { /* ... */ }
  ```
  Equal-height pellets over a material of total height `material_height`

- ```rust
  pub fn n_slices(self: &Self) -> Option<usize> { /* ... */ }
  ```
  Number of slices, when it is a property of the configuration.

- ```rust
  pub fn total_height(self: &Self) -> Option<f64> { /* ... */ }
  ```
  Total height \[m\] the slice list spans, or `None` for the variants that

- ```rust
  pub fn slice_index(self: &Self, height_above_bottom: f64) -> Option<usize> { /* ... */ }
  ```
  Slice index of a single point, given its height above the bottom of the

- ```rust
  pub fn assign(self: &Self, axial_coordinates: &[f64], material_bottom: f64) -> Vec<Option<usize>> { /* ... */ }
  ```
  Assign every cell to a slice.

- ```rust
  pub fn validate(self: &Self, material_height: Option<f64>) -> Result<()> { /* ... */ }
  ```
  Reject a configuration upstream would have aborted on.

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
    fn clone(self: &Self) -> AxialSlicing { /* ... */ }
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
    fn eq(self: &Self, other: &AxialSlicing) -> bool { /* ... */ }
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
#### Struct `SliceAverage`

Volume-weighted averages of one field over the axial slices — upstream's
`sliceMapper::sliceAverage<Type>()`.

```text
avg[s] = Σ_{i ∈ s} value_i · V_i  /  Σ_{i ∈ s} V_i
```

Volume weighting, not cell counting: a 2D or 3D mesh has cells of very
different sizes in one slice (an outer ring holds far more material than the
central one), and a cell-count average would over-weight the centre of the
pellet, where the temperature is highest.

# Units

[`means`](Self::means) carry whatever unit the input values carried;
[`slice_volume`](Self::slice_volume) is m³.

```rust
pub struct SliceAverage {
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
  pub fn compute(values: &[f64], volumes: &[f64], slice_ids: &[Option<usize>], n_slices: usize) -> Result<Self> { /* ... */ }
  ```
  Compute the per-slice volume-weighted averages.

- ```rust
  pub fn means(self: &Self) -> &[f64] { /* ... */ }
  ```
  The per-slice averages, bottom to top.

- ```rust
  pub fn slice_volumes(self: &Self) -> &[f64] { /* ... */ }
  ```
  The per-slice total volumes \[m³\], bottom to top.

- ```rust
  pub fn n_slices(self: &Self) -> usize { /* ... */ }
  ```
  Number of slices.

- ```rust
  pub fn mean(self: &Self, s: usize) -> Option<f64> { /* ... */ }
  ```
  Average over slice `s`, or `None` if `s` is out of range.

- ```rust
  pub fn slice_volume(self: &Self, s: usize) -> Option<f64> { /* ... */ }
  ```
  Total volume \[m³\] of slice `s`, or `None` if `s` is out of range.

- ```rust
  pub fn empty_slices(self: &Self) -> Vec<usize> { /* ... */ }
  ```
  Indices of slices that received no volume — the condition upstream treats

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
    fn clone(self: &Self) -> SliceAverage { /* ... */ }
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
    fn eq(self: &Self, other: &SliceAverage) -> bool { /* ... */ }
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

#### Function `axial_coordinate`

**Attributes:**

- `MustUse { reason: None }`

Axial coordinate \[m\] of a point along the pin direction — upstream's
`mesh_.C() & pinDirection_`.

A plain dot product. `pin_direction` is expected to be a **unit** vector
along the rod axis (upstream reads it from its `globalOptions`); if it is
not, every coordinate is scaled by its magnitude and the slice heights no
longer mean metres. This function does not normalise, matching upstream — use
[`axial_coordinate_checked`] to be told.

```rust
pub fn axial_coordinate(position: outram_foam_basic_lib::primitives::Vector3, pin_direction: outram_foam_basic_lib::primitives::Vector3) -> f64 { /* ... */ }
```

#### Function `axial_coordinate_checked`

[`axial_coordinate`], rejecting a non-unit pin direction.

# Errors

[`OffbeatError::Unphysical`] if `|pin_direction| − 1` exceeds 1e-9. A pin
direction that is not a unit vector silently rescales every axial coordinate,
which produces slices of the wrong height rather than an obvious failure.

```rust
pub fn axial_coordinate_checked(position: outram_foam_basic_lib::primitives::Vector3, pin_direction: outram_foam_basic_lib::primitives::Vector3) -> crate::error::Result<f64> { /* ... */ }
```

#### Function `round_to`

**Attributes:**

- `MustUse { reason: None }`

Round `z` \[m\] to the nearest multiple of `precision` \[m\] — upstream's
`round(Cz/precision_)*precision_`.

Rust's `f64::round` and C++'s `std::round` both round halves away from zero,
so this matches upstream bit-for-bit. Returns `z` unchanged for a
non-positive or non-finite precision.

```rust
pub fn round_to(z: f64, precision: f64) -> f64 { /* ... */ }
```

#### Function `auto_axial_levels`

**Attributes:**

- `MustUse { reason: None }`

The distinct rounded axial levels \[m\] present in `axial_coordinates`,
sorted ascending — upstream's `sortedAxialLocationList` in
`sliceMapperAutoAxialSlices::calcAddressing`.

Each level becomes one slice, so the length of the result is the slice count
for [`AxialSlicing::AutoAxial`]. Non-finite coordinates are dropped.

# Deferred

Upstream builds this list per processor and reconciles it with a
`Pstream::gatherList`/`scatterList` pair, so that a level present on only one
processor still exists (empty) on the others. This function is the
single-processor case.

```rust
pub fn auto_axial_levels(axial_coordinates: &[f64], precision: f64) -> Vec<f64> { /* ... */ }
```

### Constants and Statics

#### Constant `SLICE_BOUNDARY_TOLERANCE`

Absolute tolerance \[m\] in the slice-boundary test — upstream's literal
`1e-6` in `sliceMapperByMaterial::calcAddressing` and
`sliceMapperByPellets::calcAddressing`.

The test is `cumulative_height >= height_above_bottom + 1e-6`, so a cell
centre lying **exactly** on a slice boundary is assigned to the slice
**above** it, and a cell centre within 1 µm below a boundary is too. It is an
absolute length, not a relative one, so its effect depends on the rod's
absolute dimensions — for a 1 cm pellet it is a 0.01% band.

```rust
pub const SLICE_BOUNDARY_TOLERANCE: f64 = 1.0e-6;
```

### Re-exports

#### Re-export `GapConductance`

```rust
pub use conductance::GapConductance;
```

#### Re-export `GapConductanceModel`

```rust
pub use conductance::GapConductanceModel;
```

#### Re-export `GapConductanceScaling`

```rust
pub use conductance::GapConductanceScaling;
```

#### Re-export `GapSurfaces`

```rust
pub use conductance::GapSurfaces;
```

#### Re-export `PenaltyContact`

```rust
pub use contact::PenaltyContact;
```

#### Re-export `FreeVolumeRegion`

```rust
pub use free_volume::FreeVolumeRegion;
```

#### Re-export `GasPressureModel`

```rust
pub use free_volume::GasPressureModel;
```

#### Re-export `RodFreeVolume`

```rust
pub use free_volume::RodFreeVolume;
```

#### Re-export `GapGasMixture`

```rust
pub use gas::GapGasMixture;
```

#### Re-export `GapGasSpecies`

```rust
pub use gas::GapGasSpecies;
```

#### Re-export `AxialSlicing`

```rust
pub use slice::AxialSlicing;
```

#### Re-export `SliceAverage`

```rust
pub use slice::SliceAverage;
```

## Module `rheology`

Constitutive laws — what makes fuel behave like fuel rather than a spring.

# What this layer is for

[`crate::mechanics`] solves equilibrium `∇·σ = 0` for a linear-elastic
material loaded by an eigenstrain. That is a spring: unload it and it
returns to where it started, and it never yields, never relaxes, never
creeps. Real fuel and real cladding do all three, and this module is where
that happens.

Upstream calls it from `smallStrain.C` after the displacement solve has
converged: hand it the strain, and it hands back the **corrected stress**
plus the internal variables that advanced. This port keeps that contract,
per cell, in [`ConstitutiveLaw::correct`].

# The two mechanisms, for a reader with no fuel-performance background

Assume continuum mechanics; assume nothing about reactors.

**Plasticity** is a *threshold* phenomenon. Below the yield stress `σ_y`
deformation is elastic and reversible. Reach `σ_y` and the material flows
irreversibly, at whatever rate it takes to keep the stress *on* the yield
surface — you cannot push a perfectly plastic material past `σ_y` no matter
how hard you strain it. Because yielding rearranges dislocations, `σ_y`
itself usually rises as flow accumulates: **work hardening**. In a fuel rod
this matters during power ramps and during pellet–cladding mechanical
interaction, where a swollen pellet pushes hard enough on the tube to yield
it.

**Creep** is a *rate* phenomenon with no threshold. Hold any stress for long
enough and the material keeps deforming, at a rate set by stress and
(usually exponentially) by temperature. Over a laboratory test this is
invisible; over the 3–5 years a fuel rod spends in a reactor it dominates.
Two flavours matter:

- **Thermal creep** — diffusion and dislocation climb, `∝ exp(−Q/RT)`. This
  is what makes a 1500 K fuel pellet slowly conform to the tube around it.
- **Irradiation creep** — fast neutrons continuously knock atoms off their
  lattice sites, and the resulting point defects relieve local stress as
  they migrate. Its rate depends on the neutron **flux**, is nearly
  independent of temperature, and is roughly linear in stress. This is the
  mechanism that actually lets 600 K cladding — far too cold for thermal
  creep to do anything measurable — creep *down* onto the pellet under
  coolant pressure over months. Predicting when the fuel/cladding gap closes
  is not possible without it.

# What `correct` computes

Everything rests on one additive split of the **mechanical** strain, i.e.
the total strain with the stress-free eigenstrain already removed:

`ε_mech = ε_el + ε_p + ε_c`

and one isotropic stress–strain relation in the elastic part alone:

`σ = K tr(ε_el) I + 2μ dev(ε_el)`

Per timestep, per cell, in this order (matching upstream's
`misesPlasticCreep`):

1. Subtract the plastic and creep strain accumulated in previous converged
   steps, giving the elastic **trial** strain and its trial stress.
2. **Creep.** Solve `Δε_c = Δt · ε̇_c(q_trial − 3μ Δε_c)` by Newton for the
   equivalent creep increment, then flow along the deviatoric stress
   direction (Prandtl–Reuss).
3. **Plasticity.** Radial-return the creep-reduced trial stress onto the
   yield surface, solving
   `|s| − 2μΔλ − sqrt(2/3) σ_y(ε_p,eq + sqrt(2/3)Δλ) = 0` by Newton.
4. Assemble `σ` from what elastic strain is left.

Creep is done **before** plasticity because creep relaxes stress: doing it
afterwards leaves more of the increment to be taken up plastically and
over-predicts plastic strain.

Neither Newton iteration is allowed to fail quietly. Both report
[`OffbeatError::ConstitutiveNotConverged`](crate::error::OffbeatError::ConstitutiveNotConverged)
rather than returning a stress that sits outside the yield surface.

# How the mechanics layer must drive this

The contract is not obvious and getting it wrong produces plausible-looking
nonsense, so it is spelled out:

1. **Subtract the eigenstrain first.** [`RheologyInputs::mechanical_strain`]
   is the strain from the displacement solve **minus** thermal expansion,
   swelling, densification and relocation. Passing the total strain makes a
   freely expanding, unconstrained pellet appear to be under enormous stress
   — and it will then dutifully creep.
2. **Own the state.** Keep one [`RheologyState`] per cell. Pass it by
   reference to `correct`, which does not mutate it.
3. **Iterate, then commit.** `correct` may be called many times per timestep
   inside the outer mechanics corrector loop, each time from the *same*
   start-of-step [`RheologyState`]. Only once that loop has converged, call
   [`RheologyState::advance`] once. Advancing inside the loop double-counts
   the inelastic strain — the classic way to make a creep model silently
   over-predict.
4. **Feed the corrected stress back.** The corrected stress is *softer* than
   the elastic one, so the displacement field that produced it is no longer
   an equilibrium field. Upstream restores equilibrium by re-solving with
   the inelastic strain treated as an additional eigenstrain (its
   `correctAdditionalStrain`); [`RheologyState::inelastic_strain`] is the
   quantity to hand back for that.
5. **Limit the timestep.** Creep integration is implicit in stress but
   explicit in state. [`CreepTimeStepControl`] turns last step's increments
   into a bound on the next step.

# Scope of this port

**Implemented:** small-strain elasticity, von Mises plasticity with
isotropic hardening, and plasticity plus creep; three yield-stress models
(constant, tabulated hardening, FRAPTRAN Zircaloy); four creep models (none,
Norton power law, Limbäck Zircaloy, MATPRO fuel); per-material-zone
dispatch.

**Not implemented, and why:**

| Upstream | Reason |
|---|---|
| `hyperElasticity`, `neoHookeanElasticity`, `neoHookeanMisesPlasticity`, `hyperElasticMisesPlasticCreep` | Large-strain (total/updated Lagrangian) laws. [`crate::mechanics`] is small-strain only and does not produce a deformation gradient, so these would have nothing to consume. |
| `GPLS_Hydrogen`, `fastNeutronGPLS` | Anisotropic (Hill) viscoplastic laws for reactivity-initiated-accident transients. They need a Hill tensor, a normalised axial coordinate and an RIA-specific strain-rate range that no other part of this port supplies. |
| `LimbackCreepModelLOCA` | Loss-of-coolant variant covering the Zircaloy α→β phase transformation. Needs a phase-fraction field this port does not have. |
| `MalyginMOXCreepModel`, `RoutbortFastMOXCreepModel`, `TobbeCreepModel`, `TobbeDINCreepModel`, `TobbeAIM1CreepModel`, `ZhangHastelloyCreepModel`, `MonolithicSiCCreepModel`, `PARFUMEBufferCreepModel`, `PARFUMEPyCCreepModel` | Additional material-specific correlations with the same structure as [`CreepModel::Matpro`]. Deferred on effort, not on capability: each is a new enum variant plus a rate function. |
| `constantCreepPrincipalStress`, `correlationCreepPrincipalStress` | Need a principal-stress decomposition and per-direction creep, which the isotropic Prandtl–Reuss flow used here cannot express. |
| `planeStress`, `modifiedPlaneStrain`, `solvePressureEqn` in `rheologyByMaterial` | Mesh- and case-level features rather than constitutive laws: they need the axial slice mapper, the gap-gas model and the momentum-matrix diagonal, all of which live outside this module. |

# Status

**AI-assisted draft, unreviewed.** Per `RESPONSIBLE_USE.md` this is
untrusted material until a human has inspected it. The tests establish
self-consistency and agreement with closed-form plasticity and creep
solutions; none is a validation against experiment, and none may be
described as one.

```rust
pub mod rheology { /* ... */ }
```

### Modules

## Module `aster`

code_aster constitutive laws.

# What this is

A port of the constitutive-law layer of [code_aster](https://gitlab.com/codeaster/src),
EDF's nonlinear structural and thermo-mechanical solver, into this crate's
existing rheology idiom. Plan and module inventory:
`docs/code-aster-port-scoping.md`; tracked as epic `op-a7p`.

# Why it is worth porting

code_aster was built by EDF to justify the integrity and remaining life of
its own reactor fleet, so its constitutive laws are the *nuclear* ones —
irradiation creep, Zircaloy anisotropy, vessel steels — rather than generic
mechanical-engineering fare. That gives one port two consumers:

- **Fuel performance.** [`crate::rheology`] currently offers three
  constitutive laws. code_aster's catalogue carries `ZIRC`, `ZIRC_MECA`,
  `META_LEMA_ANI`, `LEMAITRE_IRRA`, `VISC_IRRA_LOG`, `GRAN_IRRA_LOG` and
  `IRRAD3M` — anisotropic and irradiation creep for cladding, which normal
  operation needs and this crate does not yet have.
- **Severe accident.** Creep rupture of a reactor lower head, the model
  `docs/melcor-scoping.md` phase 5 needs.

# Status

**Verification-tested draft. Nothing here is validated.** Every test in
every module below is *verification* — independent transcription of
upstream's algebra, closed-form limits, invariants, and measured
convergence orders. Nothing has been compared against a cladding-creepdown
measurement or any reactor data, and no such agreement is claimed.

Upstream's `astest` suite **is** available in the read-only clone (an
earlier revision of this note wrongly said it was absent — it was merely
outside the sparse checkout), and it lives in the GPL-3.0-or-later `src`
repository, so it is in scope under `DATA_POLICY.md`. Two of its cases are
now run as integration tests — `tests/astest_ssnv101a.rs` (Chaboche) and
`tests/astest_ssnv126a.rs` (`VENDOCHAB`) — against upstream's own **`VALE_CALC`**
computed values. That makes them *verification against a reference
implementation*: they show this port reproduces code_aster's arithmetic,
**not** that either code reproduces reality. Upstream's `VALE_REFE`
analytical/experimental references are deliberately never asserted here —
promoting a case to validation is the maintainer's call.

Foundations:

- [`catalogue`] — what upstream declares (229 behaviours).
- [`kinematics`] — the Mandel convention and the deformation gradient.
- [`integration`] — the scalar local solvers every law below shares.
- [`log_strain`] — the `GDEF_LOG` large-strain wrapper.
- [`hardening`] — the one isotropic-hardening curve every law above shares.

Constitutive laws:

| Module | Laws |
|---|---|
| [`viscoplastic`] | `NORTON`, `LEMAITRE`, `LEMAITRE_IRRA` |
| [`isotropic`] | `VMIS_ISOT_LINE`/`_PUIS` hardening, `NORTON_HOFF` |
| [`chaboche`] | `VMIS_CIN1/2_CHAB`, `VISC_CIN1/2_CHAB`, `VMIS/VISC_CIN2_MEMO` |
| [`viscochab`] | `VISCOCHAB` — the 27-variable rate system of `rkdcha.F90` |
| [`damage`] | `VENDOCHAB`, `VISC_ENDO_LEMA`, `ROUSS_PR`, `ROUSS_VISC`, `GTN`, `VISC_GTN`, `CRIT_RUPT` |
| [`metallurgy`] | `VISC_IRRA_LOG`, `GRAN_IRRA_LOG`, `IRRAD3M`, `META_LEMA_ANI` |
| [`fracture`] | linear-elastic fracture post-processing only — see below |

Two limitations that change results and must not be discovered late:

- [`fracture`] is roughly **80 % unported** — the closed-form subset only.
  It is *not* blocked on finite elements (an earlier revision of this note
  said so; that was wrong). The G-theta domain integral is
  discretisation-agnostic; what is missing is a crack front as ordered
  data, ring quadrature, and the virtual-extension field. See that module's
  docs for the real difficulty, which is FV gradient accuracy at the
  `1/√r` singularity.
- [`damage`]'s `GTN` is the **local** form only. Without `GRADVARI`
  nonlocal regularisation a structural run will localise into one element
  band and give mesh-dependent answers.

# Provenance

code_aster is GPL-3.0-or-later, compatible with this workspace. Upstream is
**not** vendored — the port is made from a read-only clone kept outside the
working tree, and only upstream's `src` repository is used. Its `validation`
and `data` repositories carry material that may not be freely distributed
and are out of scope per `DATA_POLICY.md`.

```rust
pub mod aster { /* ... */ }
```

### Modules

## Module `catalogue`

Generated registry of code_aster's constitutive-law catalogue.

# What this is, and what it is not

This is **metadata only** -- the declarative half of code_aster's
behaviour catalogue, transcribed mechanically. It records what laws
exist, the `num_lc` number each dispatches on, how many internal state
variables each carries and what they are called, and which
modelisations, strain measures and integration algorithms each
supports.

It contains **no physics**. No stress is computed here. A variant
appearing in [`AsterBehaviour`] means only that upstream declares that
law, not that this port implements it. There is deliberately **no**
`is_implemented` query: a hand-maintained "is it done yet" flag on 229
variants would go stale the moment one more law landed, and a stale flag
reading `true` is worse than no flag. The implemented set is listed in the
[`super`] module documentation and in the crate README, both of which sit
next to the code that would have to change.

# Why generated

231 declarations transcribed by hand would drift from upstream
silently, and the drift would stay invisible until a law dispatched on
the wrong number and produced a plausible wrong stress. Regenerating
from the upstream tree makes any divergence a diff rather than a
mystery.

# Naming

Variant identifiers here are a mechanical transliteration of the ASTER
name (`VISC_CIN2_CHAB` -> `ViscCin2Chab`). They are deliberately *not*
the descriptive English names the hand-written laws carry -- per
`docs/code-aster-port-scoping.md` section 4, `NORTON` surfaces as
`NortonViscoplastic` in the implemented API. Keeping the registry
mechanical means it can be regenerated without overwriting hand-chosen
names; [`AsterBehaviour::aster_name`] is the link between the two.

```rust
pub mod catalogue { /* ... */ }
```

### Types

#### Enum `AsterBehaviour`

**Attributes:**

- `NonExhaustive`

One entry of code_aster's behaviour catalogue.

Every variant corresponds to one `LoiComportement` declaration in
upstream's `code_aster/Behaviours/`. See the module documentation for
why presence here does not imply implementation.

```rust
pub enum AsterBehaviour {
    Acier,
    AcierMeca,
    AcierRevenu,
    Analytique,
    Arme,
    AsseCorn,
    Cable,
    ChocElasTrac,
    ChocEndo,
    ChocEndoPena,
    CritRupt,
    Dashpot,
    DdiPlasEndo,
    Deborst,
    Dhrc,
    DisBiliElas,
    DisChoc,
    DisContact,
    DisEcroCine,
    DisEcroTrac,
    DisGouj2eElas,
    DisGouj2ePlas,
    DisGricra,
    DisVisc,
    ElasMembraneNh,
    ElasMembraneSv,
    ElasPoutreGr,
    Fondation,
    GdefLog,
    GlrcDamage,
    GlrcDm,
    GreenLagrange,
    GrilleCineLine,
    GrilleIsotLine,
    GrotGdep,
    HoekBrownTot,
    Hydr,
    HydrEndo,
    HydrTabbal,
    HydrUtil,
    HydrVgc,
    HydrVgm,
    JointBandis,
    JoncEndoPlas,
    KitCg,
    KitH,
    KitHh,
    KitHh2,
    KitHh2m,
    KitHhm,
    KitHm,
    KitThh,
    KitThh2,
    KitThh2m,
    KitThhm,
    KitThm,
    KitThv,
    MetaGCine,
    MetaGCinePt,
    MetaGCinePtre,
    MetaGCineRe,
    MetaGIsot,
    MetaGIsotPt,
    MetaGIsotPtre,
    MetaGIsotRe,
    MetaPCineLine,
    MetaPIsotLine,
    MetaPIsotTrac,
    MetaVCineLine,
    MetaVIsotLine,
    MetaVIsotTrac,
    Multifibre,
    Petit,
    PetitReac,
    Pmf,
    ReguViscElas,
    RestEcroCine,
    RestEcroEcmi,
    RestEcroIsot,
    RuptFrag,
    SechBazant,
    SechGranger,
    SechMensi,
    SechNappe,
    SechRft,
    TherHydr,
    TherNl,
    Vide,
    ViscMaxwellMt,
    Zirc,
    ZircMeca,
    Edgar,
    Elas,
    InterfPouElas,
    LiquSatu,
    Ther,
    Gaz,
    InterfPouCine,
    ViscIsotLine,
    ViscIsotTrac,
    VmisIsotLine,
    VmisIsotPuis,
    VmisIsotTrac,
    Waeckel,
    Jma,
    LiquVape,
    VmisCineGc,
    VmisCineLine,
    VmisEcmiLine,
    VmisEcmiTrac,
    LiquVapeGaz,
    ViscCin1Chab,
    ViscCin2Chab,
    ViscCin2Memo,
    ViscCin2Nrad,
    ViscMemoNrad,
    VmisCin1Chab,
    VmisCin2Chab,
    VmisCin2Memo,
    VmisCin2Nrad,
    VmisMemoNrad,
    LiquGaz,
    LiquGazAtm,
    EndoOrthBeton,
    Mazars,
    MazarsUnil,
    BetonReglePr,
    LiquAdGazVape,
    CzmExpReg,
    LiquAdGaz,
    CzmLinReg,
    JointBa,
    KitMeta,
    DruckPrager,
    NortonHoff,
    ViscTaheri,
    ElasHyper,
    BetonUmlv,
    CamClay,
    Cjs,
    CorrAcier,
    Rankine,
    BetonGranger,
    BetonGrangerV,
    GranIrraLog,
    LemaitreIrra,
    LemaSeuil,
    ViscIrraLog,
    Lemaitre,
    Irrad3m,
    RoussPr,
    RoussVisc,
    Vendochab,
    ViscEndoLema,
    Hayhurst,
    Norton,
    Viscochab,
    HoekBrown,
    HoekBrownEff,
    Laigle,
    Hujeux,
    Letk,
    EndoIsotBeton,
    Rousselier,
    Sans,
    CzmOuvMix,
    DruckPragNA,
    CzmTacMix,
    ViscDrucPrag,
    CzmFatMix,
    JointMecaRupt,
    CzmTuron,
    EndoScalaire,
    EndoHeterogene,
    JointMecaEndo,
    JointMecaFrot,
    CzmTraMix,
    Umat,
    CzmLabMix,
    VmisJohnCook,
    MohrCoulomb,
    CzmExpMix,
    EndoFissExp,
    BetonAgeing,
    BetonBurger,
    Barcelone,
    Cssm,
    CzmMfront,
    ElasHyperVisc,
    Gonfelas,
    HyperHill,
    Iwan,
    Mcc,
    MetaLemaAni,
    Mfront,
    MetaacierepilPt,
    Mohrcoulombas,
    NlhCsrm,
    ViscIsotPlas,
    Lkr,
    EndoLocaExp,
    ViscMaxwell,
    Gtn,
    ViscGtn,
    ViscIsotNl,
    VmisIsotNl,
    CzmElasMix,
    KicheninNl,
    CzmFrotMix,
    ElasVmisLine,
    ElasVmisPuis,
    ElasVmisTrac,
    EndoLocaTc,
    RelaxAcier,
    VmisAsymLine,
    ElasIsotEner,
    ElasIsotIncr,
    BetonDoubleDp,
    Monocristal,
    Polycristal,
    BetonRag,
    CableGaineFrot,
    FluaPoroBeton,
    EndoPoroBeton,
    FluaEndoPoro,
    RgiBeton,
    RgiBetonBa,
    SimoMiehe,
    KitDdi,
}
```

##### Variants

###### `Acier`

Metallurgical phases for steel - Using in metallurgical behaviour law

ASTER behaviour name: `ACIER` (`num_lc = 0`,
6 state variable(s)).
Upstream declaration: `code_aster/Behaviours/acier.py`.

###### `AcierMeca`

Metallurgical phases for steel - Using in mechanical behaviour law

ASTER behaviour name: `ACIER_MECA` (`num_lc = 0`,
5 state variable(s)).
Upstream declaration: `code_aster/Behaviours/acier_meca.py`.

###### `AcierRevenu`

phases metallurgiques de l'acier

ASTER behaviour name: `ACIER_REVENU` (`num_lc = 0`,
8 state variable(s)).
Upstream declaration: `code_aster/Behaviours/acier_revenu.py`.

###### `Analytique`

Algo analytique pour résolution en contraintes planes.

ASTER behaviour name: `ANALYTIQUE` (`num_lc = 0`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/analytique.py`.

###### `Arme`

Relation de comportement élasto-plastique isotherme pour les armements
de lignes [R5.03.31]

ASTER behaviour name: `ARME` (`num_lc = 0`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/arme.py`.

###### `AsseCorn`

Relation de comportement élasto-plastique isotherme pour les assemblages
boulonnés de cornières de pylônes [R5.03.32]

ASTER behaviour name: `ASSE_CORN` (`num_lc = 0`,
7 state variable(s)).
Upstream declaration: `code_aster/Behaviours/asse_corn.py`.

###### `Cable`

Relation de comportement élastique adaptée aux câbles (DEFORMATION:
'GROT_GDEP' obligatoire) : Le module d'YOUNG du câble peut être
différent en compression et en traction, en particulier il peut être nul
en compression.

ASTER behaviour name: `CABLE` (`num_lc = 0`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/cable.py`.

###### `ChocElasTrac`

Relation de comportement sur élément discret de choc avec un
comportement élastique non-linéaire

ASTER behaviour name: `CHOC_ELAS_TRAC` (`num_lc = 0`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/choc_elas_trac.py`.

###### `ChocEndo`

Relation de comportement sur élément discret à seuil plastique, raideur
et amortissement variables

ASTER behaviour name: `CHOC_ENDO` (`num_lc = 0`,
5 state variable(s)).
Upstream declaration: `code_aster/Behaviours/choc_endo.py`.

###### `ChocEndoPena`

Relation de comportement par pénalisation sur élément discret à seuil
plastique, raideur et amortissement variables

ASTER behaviour name: `CHOC_ENDO_PENA` (`num_lc = 0`,
3 state variable(s)).
Upstream declaration: `code_aster/Behaviours/choc_endo_pena.py`.

###### `CritRupt`

Détection critère de rupture

ASTER behaviour name: `CRIT_RUPT` (`num_lc = 0`,
6 state variable(s)).
Upstream declaration: `code_aster/Behaviours/crit_rupt.py`.

###### `Dashpot`

Relation de type Dashpot pour les éléments discrets

ASTER behaviour name: `DASHPOT` (`num_lc = 0`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/dashpot.py`.

###### `DdiPlasEndo`

Couplage plasticité/endommagement pour GLRC

ASTER behaviour name: `DDI_PLAS_ENDO` (`num_lc = 0`,
6 state variable(s)).
Upstream declaration: `code_aster/Behaviours/ddi_plas_endo.py`.

###### `Deborst`

Algo pour résolution en contraintes planes.

ASTER behaviour name: `DEBORST` (`num_lc = 0`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/deborst.py`.

###### `Dhrc`

Ce modèle homogénéisé permet de représenter l'endommagement et le
glissement interne périodique d'une plaque en béton armé pour des
sollicitations modérées. La loi de comportement s'écrit directement en
terme de contraintes et de déformations généralisées. La modélisation
jusqu'à la rupture n'est pas recommandée, puisque les phénomènes de
plastification des aciers et de propagation de fissures ne sont pas pris
en compte. L'identification des paramètres nécessaires à cette loi de
comportement se fait via une procédure préalable d'homogénéisation. Pour
les précisions sur la formulation du modèle voir [R7.01.36]

ASTER behaviour name: `DHRC` (`num_lc = 0`,
11 state variable(s)).
Upstream declaration: `code_aster/Behaviours/dhrc.py`.

###### `DisBiliElas`

Relation de comportement bilineaire des elements discrets

ASTER behaviour name: `DIS_BILI_ELAS` (`num_lc = 0`,
6 state variable(s)).
Upstream declaration: `code_aster/Behaviours/dis_bili_elas.py`.

###### `DisChoc`

Relation de comportement de choc avec frottement pour les elements
discrets

ASTER behaviour name: `DIS_CHOC` (`num_lc = 0`,
10 state variable(s)).
Upstream declaration: `code_aster/Behaviours/dis_choc.py`.

###### `DisContact`

Relation de comportement de choc et contact-frottement avec des éléments
discrets

ASTER behaviour name: `DIS_CONTACT` (`num_lc = 0`,
9 state variable(s)).
Upstream declaration: `code_aster/Behaviours/dis_contact.py`.

###### `DisEcroCine`

Relation de comportement à écrouissage cinématique des elements discrets

ASTER behaviour name: `DIS_ECRO_CINE` (`num_lc = 0`,
18 state variable(s)).
Upstream declaration: `code_aster/Behaviours/dis_ecro_cine.py`.

###### `DisEcroTrac`

Relation de comportement isotrope pour les éléments discrets

ASTER behaviour name: `DIS_ECRO_TRAC` (`num_lc = 0`,
19 state variable(s)).
Upstream declaration: `code_aster/Behaviours/dis_isot.py`.

###### `DisGouj2eElas`

Relation de comportement élastique des filets des goujons pour des
elements discrets

ASTER behaviour name: `DIS_GOUJ2E_ELAS` (`num_lc = 0`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/dis_gouj2e_elas.py`.

###### `DisGouj2ePlas`

Relation de comportement élastoplastique des filets des goujons pour des
elements discrets

ASTER behaviour name: `DIS_GOUJ2E_PLAS` (`num_lc = 0`,
2 state variable(s)).
Upstream declaration: `code_aster/Behaviours/dis_gouj2e_plas.py`.

###### `DisGricra`

Relation de comportement de la liaison grille-crayons des assemblages
combustibles, applicable à des elements discrets

ASTER behaviour name: `DIS_GRICRA` (`num_lc = 0`,
8 state variable(s)).
Upstream declaration: `code_aster/Behaviours/dis_gricra.py`.

###### `DisVisc`

Relation de comportement visqueuse pour les éléments discrets

ASTER behaviour name: `DIS_VISC` (`num_lc = 0`,
4 state variable(s)).
Upstream declaration: `code_aster/Behaviours/dis_visc.py`.

###### `ElasMembraneNh`

Relation de comportement hyper-élastique utilisant le modèle Néo-Hookéen
applicable uniquement aux MEMBRANE en grandes déformations
(DEFORMATION='GROT_GDEP')

ASTER behaviour name: `ELAS_MEMBRANE_NH` (`num_lc = 0`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/elas_membrane_nh.py`.

###### `ElasMembraneSv`

Relation de comportement hyper-élastique utilisant le modèle de
Saint-Venant Kirchhoff applicable uniquement aux MEMBRANE en grandes
déformations (DEFORMATION='GROT_GDEP')

ASTER behaviour name: `ELAS_MEMBRANE_SV` (`num_lc = 0`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/elas_membrane_sv.py`.

###### `ElasPoutreGr`

ERelation de comportement élastique pour les poutres en grands
déplacements et grandes rotations (DEFORMATION: 'GREEN_GR' est
obligatoire). (Cf. [R5.03.40] pour plus de détail).

ASTER behaviour name: `ELAS_POUTRE_GR` (`num_lc = 0`,
3 state variable(s)).
Upstream declaration: `code_aster/Behaviours/elas_poutre_gr.py`.

###### `Fondation`

Relation de comportement de fondation superficielle avec des éléments
discrets

ASTER behaviour name: `FONDATION` (`num_lc = 0`,
21 state variable(s)).
Upstream declaration: `code_aster/Behaviours/fondation.py`.

###### `GdefLog`

Algo pour résolution en grandes déformations.

ASTER behaviour name: `GDEF_LOG` (`num_lc = 0`,
6 state variable(s)).
Upstream declaration: `code_aster/Behaviours/gdef_log.py`.

###### `GlrcDamage`

Modèle global de plaque en béton armé capable de représenter son
comportement jusqu'à la ruine. Contrairement aux modélisations locales
où chaque constituant du matériau est modélisé à part, dans les modèles
globaux, la loi de comportement s'écrit directement en terme de
contraintes et de déformations généralisées. Les phénomènes pris en
compte sont l'élasto-plasticité couplée entre les effets de membrane et
de flexion (contre une élasto-plasticité en flexion seulement dans GLRC)
et l'endommagement en flexion. L'endommagement couplé membrane/flexion
est traité par GLRC_DM, lequel, par contre, néglige complètement
l'élasto-plasticité. Pour les précisions sur la formulation du modèle
voir [R7.01.31].

ASTER behaviour name: `GLRC_DAMAGE` (`num_lc = 0`,
19 state variable(s)).
Upstream declaration: `code_aster/Behaviours/glrc_damage.py`.

###### `GlrcDm`

Ce modèle global permet de représenter l'endommagement d'une plaque en
béton armé pour des sollicitations modérées. Contrairement aux
modélisations locales où chaque constituant du matériau est modélisé à
part, dans les modèles globaux, la loi de comportement s'écrit
directement en terme de contraintes et de déformations généralisées. La
modélisation jusqu'à la rupture n'est pas recommandée, puisque les
phénomènes de plastification ne sont pas pris en compte, mais le sont
dans GLRC_DAMAGE. En revanche, la modélisation du couplage de
l'endommagement entre les effets de membrane et de flexion dans GLRC_DM
est pris en compte, ce qui n'est pas le cas dans GLRC_DAMAGE. Pour les
précisions sur la formulation du modèle voir [R7.01.32]

ASTER behaviour name: `GLRC_DM` (`num_lc = 0`,
18 state variable(s)).
Upstream declaration: `code_aster/Behaviours/glrc_dm.py`.

###### `GreenLagrange`

Algo pour résolution en grandes déformations.

ASTER behaviour name: `GREEN_LAGRANGE` (`num_lc = 0`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/green_lagrange.py`.

###### `GrilleCineLine`

Relation de comportement des grilles d'armatures de béton armé, à
écrouissage cinématique linéaire

ASTER behaviour name: `GRILLE_CINE_LINE` (`num_lc = 0`,
4 state variable(s)).
Upstream declaration: `code_aster/Behaviours/grille_cine_line.py`.

###### `GrilleIsotLine`

Relation de comportement des grilles d'armatures de béton armé, à
écrouissage isotrope linéaire

ASTER behaviour name: `GRILLE_ISOT_LINE` (`num_lc = 0`,
4 state variable(s)).
Upstream declaration: `code_aster/Behaviours/grille_isot_line.py`.

###### `GrotGdep`

Algo pour résolution en grandes déformations.

ASTER behaviour name: `GROT_GDEP` (`num_lc = 0`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/grot_gdep.py`.

###### `HoekBrownTot`

Relation de comportement de Hoek et Brown modifiée pour la modélisation
du comportement des roches [R7.01.18] pour la mécanique pure. Le
couplage est formulé en contraintes totales. Pour faciliter
l'intégration de ce modèle, on peut utiliser le re-découpage local du
pas de temps (ITER_INTE_PAS).

ASTER behaviour name: `HOEK_BROWN_TOT` (`num_lc = 0`,
3 state variable(s)).
Upstream declaration: `code_aster/Behaviours/hoek_brown_tot.py`.

###### `Hydr`

Loi de comportement hydraulique

ASTER behaviour name: `HYDR` (`num_lc = 0`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/hydr.py`.

###### `HydrEndo`

Loi de comportement hydraulique, si le comportement mécanique est
endommageant (donc si on utilise 'MAZARS' ou 'ENDO_ISOT_BETON') sous
RELATION_KIT. Ce mot clé permet de renseigner la courbe de saturation et
sa dérivée en fonction de la pression capillaire ainsi que la
perméabilité relative et sa dérivée en fonction de la saturation.

ASTER behaviour name: `HYDR_ENDO` (`num_lc = 0`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/hydr_endo.py`.

###### `HydrTabbal`

Loi de comportement hydraulique, si le comportement mécanique est sans
endommagement : Ici et uniquement pour les lois de couplage liquide/gaz
'LIQU_GAZ', 'LIQU_AD_VAPE_GAZ' et 'LIQ_VAP_GAZ', les courbes de
saturation, de perméabilités relatives à l'eau et au gaz et leur
dérivées sont définies par le modèle de Mualem Van-Genuchten.

ASTER behaviour name: `HYDR_TABBAL` (`num_lc = 0`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/hydr_tabbal.py`.

###### `HydrUtil`

Loi de comportement hydraulique, si le comportement mécanique est sans
endommagement : Signifie qu'aucune donnée matériau n'est rentrée en dur.
Concrètement dans le cas saturé, il faudra définir les 6 courbes point
par point (par DEFI_FONCTION) suivantes : - la saturation en fonction de
la pression capillaire, - la dérivée de cette courbe, - la perméabilité
relative au liquide en fonction de la saturation, - sa dérivée. - la
perméabilité relative au gaz en fonction de la saturation, - sa dérivée.

ASTER behaviour name: `HYDR_UTIL` (`num_lc = 0`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/hydr_util.py`.

###### `HydrVgc`

Loi de comportement hydraulique, si le comportement mécanique est sans
endommagement : Ici et uniquement pour les lois de couplage liquide/gaz
'LIQU_GAZ', 'LIQU_AD_VAPE_GAZ' et 'LIQU_AD_GAZ', les courbes de
saturation, de perméabilités relatives à l'eau et leur dérivées sont
définies par le modèle de Mualem Van-Genuchten. Celle au gaz par une loi
cubique

ASTER behaviour name: `HYDR_VGC` (`num_lc = 0`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/hydr_vgc.py`.

###### `HydrVgm`

Loi de comportement hydraulique, si le comportement mécanique est sans
endommagement : Ici et uniquement pour les lois de couplage liquide/gaz
'LIQU_GAZ', 'LIQU_AD_VAPE_GAZ' et 'LIQ_VAP_GAZ', les courbes de
saturation, de perméabilités relatives à l'eau et au gaz et leur
dérivées sont définies par le modèle de Mualem Van-Genuchten.

ASTER behaviour name: `HYDR_VGM` (`num_lc = 0`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/hydr_vgm.py`.

###### `JointBandis`

Bandis

ASTER behaviour name: `JOINT_BANDIS` (`num_lc = 0`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/joint_bandis.py`.

###### `JoncEndoPlas`

Relation de comportement dediee aux jonctions voile-planchers
(comportement elasto-plastique endommageable en rotation autour de l'axe
z local) pour des elements discrets

ASTER behaviour name: `JONC_ENDO_PLAS` (`num_lc = 0`,
9 state variable(s)).
Upstream declaration: `code_aster/Behaviours/dis_jvp.py`.

###### `KitCg`

Loi d'adherence cable/gaine et loi comportement cable

ASTER behaviour name: `KIT_CG` (`num_lc = 0`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/kit_cg.py`.

###### `KitH`

KIT associé au comportement des milieux poreux (modélisations
thermo-hydro-mécanique). Pour plus de détails sur les modélisations
thermo-hydro-mécaniques et les modèles de comportement, on pourra
consulter les documents [R7.01.10] et [R7.01.11], ainsi que la notice
d'utilisation [U2.04.05]. Les relations KIT_XXXX permettent de résoudre
simultanément de deux à quatre équations d'équilibre. Les équations
considérées dépendent du suffixe XXXX avec la règle suivante : - M
désigne l'équation d'équilibre mécanique, - T désigne l'équation
d'équilibre thermique, - H désigne une équation d'équilibre hydraulique.
- V désigne la présence d'une phase sous forme vapeur (en plus du
liquide) Les problemes thermo-hydro-mécaniques associés sont traités de
facon totalement couplée. Une seule lettre H signifie que le milieu
poreux est saturé (une seule variable de pression p), par exemple soit
de gaz, soit de liquide, soit d'un mélange liquide/gaz (dont la pression
du gaz est constante). Deux lettres H signifient que le milieu poreux
est non saturé (deux variables de pression p), par exemple un mélange
liquide/vapeur/gaz. La présence des deux lettres HV signifie que le
milieu poreux est saturé par un composant (en pratique de l'eau), mais
que ce composant peut être sous forme liquide ou vapeur. Il n'y a alors
qu'une équation de conservation de ce composant, donc un seul degré de
liberté pression, mais il y a un flux liquide et un flux vapeur.

ASTER behaviour name: `KIT_H` (`num_lc = 0`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/kit_h.py`.

###### `KitHh`

KIT associé au comportement des milieux poreux (modélisations
thermo-hydro-mécanique). Pour plus de détails sur les modélisations
thermo-hydro-mécaniques et les modèles de comportement, on pourra
consulter les documents [R7.01.10] et [R7.01.11], ainsi que la notice
d'utilisation [U2.04.05]. Les relations KIT_XXXX permettent de résoudre
simultanément de deux à quatre équations d'équilibre. Les équations
considérées dépendent du suffixe XXXX avec la règle suivante : - M
désigne l'équation d'équilibre mécanique, - T désigne l'équation
d'équilibre thermique, - H désigne une équation d'équilibre hydraulique.
- V désigne la présence d'une phase sous forme vapeur (en plus du
liquide) Les problemes thermo-hydro-mécaniques associés sont traités de
facon totalement couplée. Une seule lettre H signifie que le milieu
poreux est saturé (une seule variable de pression p), par exemple soit
de gaz, soit de liquide, soit d'un mélange liquide/gaz (dont la pression
du gaz est constante). Deux lettres H signifient que le milieu poreux
est non saturé (deux variables de pression p), par exemple un mélange
liquide/vapeur/gaz. La présence des deux lettres HV signifie que le
milieu poreux est saturé par un composant (en pratique de l'eau), mais
que ce composant peut être sous forme liquide ou vapeur. Il n'y a alors
qu'une équation de conservation de ce composant, donc un seul degré de
liberté pression, mais il y a un flux liquide et un flux vapeur.

ASTER behaviour name: `KIT_HH` (`num_lc = 0`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/kit_hh.py`.

###### `KitHh2`

KIT associé au comportement des milieux poreux (modélisations
thermo-hydro-mécanique).

ASTER behaviour name: `KIT_HH2` (`num_lc = 0`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/kit_hh2.py`.

###### `KitHh2m`

KIT associé au comportement des milieux poreux (modélisations
thermo-hydro-mécanique).

ASTER behaviour name: `KIT_HH2M` (`num_lc = 0`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/kit_hh2m.py`.

###### `KitHhm`

KIT associé au comportement des milieux poreux (modélisations
thermo-hydro-mécanique). Pour plus de détails sur les modélisations
thermo-hydro-mécaniques et les modèles de comportement, on pourra
consulter les documents [R7.01.10] et [R7.01.11], ainsi que la notice
d'utilisation [U2.04.05]. Les relations KIT_XXXX permettent de résoudre
simultanément de deux à quatre équations d'équilibre. Les équations
considérées dépendent du suffixe XXXX avec la règle suivante : - M
désigne l'équation d'équilibre mécanique, - T désigne l'équation
d'équilibre thermique, - H désigne une équation d'équilibre hydraulique.
- V désigne la présence d'une phase sous forme vapeur (en plus du
liquide) Les problèmes thermo-hydro-mécaniques associés sont traités de
facon totalement couplée. Une seule lettre H signifie que le milieu
poreux est saturé (une seule variable de pression p), par exemple soit
de gaz, soit de liquide, soit d'un mélange liquide/gaz (dont la pression
du gaz est constante). Deux lettres H signifient que le milieu poreux
est non saturé (deux variables de pression p), par exemple un mélange
liquide/vapeur/gaz. La présence des deux lettres HV signifie que le
milieu poreux est saturé par un composant (en pratique de l'eau), mais
que ce composant peut être sous forme liquide ou vapeur. Il n'y a alors
qu'une équation de conservation de ce composant, donc un seul degré de
liberté pression, mais il y a un flux liquide et un flux vapeur.

ASTER behaviour name: `KIT_HHM` (`num_lc = 0`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/kit_hhm.py`.

###### `KitHm`

KIT associé au comportement des milieux poreux (modélisations
thermo-hydro-mécanique). Pour plus de détails sur les modélisations
thermo-hydro-mécaniques et les modèles de comportement, on pourra
consulter les documents [R7.01.10] et [R7.01.11], ainsi que la notice
d'utilisation [U2.04.05]. Les relations KIT_XXXX permettent de résoudre
simultanément de deux à quatre équations d'équilibre. Les équations
considérées dépendent du suffixe XXXX avec la règle suivante : - M
désigne l'équation d'équilibre mécanique, - T désigne l'équation
d'équilibre thermique, - H désigne une équation d'équilibre hydraulique.
- V désigne la présence d'une phase sous forme vapeur (en plus du
liquide) Les problèmes thermo-hydro-mécaniques associés sont traités de
facon totalement couplée. Une seule lettre H signifie que le milieu
poreux est saturé (une seule variable de pression p), par exemple soit
de gaz, soit de liquide, soit d'un mélange liquide/gaz (dont la pression
du gaz est constante). Deux lettres H signifient que le milieu poreux
est non saturé (deux variables de pression p), par exemple un mélange
liquide/vapeur/gaz. La présence des deux lettres HV signifie que le
milieu poreux est saturé par un composant (en pratique de l'eau), mais
que ce composant peut être sous forme liquide ou vapeur. Il n'y a alors
qu'une équation de conservation de ce composant, donc un seul degré de
liberté pression, mais il y a un flux liquide et un flux vapeur.

ASTER behaviour name: `KIT_HM` (`num_lc = 0`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/kit_hm.py`.

###### `KitThh`

KIT associé au comportement des milieux poreux (modélisations
thermo-hydro-mécanique). Pour plus de détails sur les modélisations
thermo-hydro-mécaniques et les modèles de comportement, on pourra
consulter les documents [R7.01.10] et [R7.01.11], ainsi que la notice
d'utilisation [U2.04.05]. Les relations KIT_XXXX permettent de résoudre
simultanément de deux à quatre équations d'équilibre. Les équations
considérées dépendent du suffixe XXXX avec la règle suivante : - M
désigne l'équation d'équilibre mécanique, - T désigne l'équation
d'équilibre thermique, - H désigne une équation d'équilibre hydraulique.
- V désigne la présence d'une phase sous forme vapeur (en plus du
liquide) Les problèmes thermo-hydro-mécaniques associés sont traités de
facon totalement couplée. Une seule lettre H signifie que le milieu
poreux est saturé (une seule variable de pression p), par exemple soit
de gaz, soit de liquide, soit d'un mélange liquide/gaz (dont la pression
du gaz est constante). Deux lettres H signifient que le milieu poreux
est non saturé (deux variables de pression p), par exemple un mélange
liquide/vapeur/gaz. La présence des deux lettres HV signifie que le
milieu poreux est saturé par un composant (en pratique de l'eau), mais
que ce composant peut être sous forme liquide ou vapeur. Il n'y a alors
qu'une équation de conservation de ce composant, donc un seul degré de
liberté pression, mais il y a un flux liquide et un flux vapeur.

ASTER behaviour name: `KIT_THH` (`num_lc = 0`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/kit_thh.py`.

###### `KitThh2`

KIT associé au comportement des milieux poreux (modélisations
thermo-hydro-mécanique).

ASTER behaviour name: `KIT_THH2` (`num_lc = 0`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/kit_thh2.py`.

###### `KitThh2m`

KIT associé au comportement des milieux poreux (modélisations
thermo-hydro-mécanique). Pour plus de détails sur les modélisations
thermo-hydro-mécaniques et les modèles de comportement,

ASTER behaviour name: `KIT_THH2M` (`num_lc = 0`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/kit_thh2m.py`.

###### `KitThhm`

KIT associé au comportement des milieux poreux (modélisations
thermo-hydro-mécanique). Pour plus de détails sur les modélisations
thermo-hydro-mécaniques et les modèles de comportement, on pourra
consulter les documents [R7.01.10] et [R7.01.11], ainsi que la notice
d'utilisation [U2.04.05]. Les relations KIT_XXXX permettent de résoudre
simultanément de deux à quatre équations d'équilibre. Les équations
considérées dépendent du suffixe XXXX avec la règle suivante : - M
désigne l'équation d'équilibre mécanique, - T désigne l'équation
d'équilibre thermique, - H désigne une équation d'équilibre hydraulique.
- V désigne la présence d'une phase sous forme vapeur (en plus du
liquide) Les problèmes thermo-hydro-mécaniques associés sont traités de
facon totalement couplée. Une seule lettre H signifie que le milieu
poreux est saturé (une seule variable de pression p), par exemple soit
de gaz, soit de liquide, soit d'un mélange liquide/gaz (dont la pression
du gaz est constante). Deux lettres H signifient que le milieu poreux
est non saturé (deux variables de pression p), par exemple un mélange
liquide/vapeur/gaz. La présence des deux lettres HV signifie que le
milieu poreux est saturé par un composant (en pratique de l'eau), mais
que ce composant peut être sous forme liquide ou vapeur. Il n'y a alors
qu'une équation de conservation de ce composant, donc un seul degré de
liberté pression, mais il y a un flux liquide et un flux vapeur.

ASTER behaviour name: `KIT_THHM` (`num_lc = 0`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/kit_thhm.py`.

###### `KitThm`

KIT associé au comportement des milieux poreux (modélisations
thermo-hydro-mécanique). Pour plus de détails sur les modélisations
thermo-hydro-mécaniques et les modèles de comportement, on pourra
consulter les documents [R7.01.10] et [R7.01.11], ainsi que la notice
d'utilisation [U2.04.05]. Les relations KIT_XXXX permettent de résoudre
simultanément de deux à quatre équations d'équilibre. Les équations
considérées dépendent du suffixe XXXX avec la règle suivante : - M
désigne l'équation d'équilibre mécanique, - T désigne l'équation
d'équilibre thermique, - H désigne une équation d'équilibre hydraulique.
- V désigne la présence d'une phase sous forme vapeur (en plus du
liquide) Les problèmes thermo-hydro-mécaniques associés sont traités de
facon totalement couplée. Une seule lettre H signifie que le milieu
poreux est saturé (une seule variable de pression p), par exemple soit
de gaz, soit de liquide, soit d'un mélange liquide/gaz (dont la pression
du gaz est constante). Deux lettres H signifient que le milieu poreux
est non saturé (deux variables de pression p), par exemple un mélange
liquide/vapeur/gaz. La présence des deux lettres HV signifie que le
milieu poreux est saturé par un composant (en pratique de l'eau), mais
que ce composant peut être sous forme liquide ou vapeur. Il n'y a alors
qu'une équation de conservation de ce composant, donc un seul degré de
liberté pression, mais il y a un flux liquide et un flux vapeur.

ASTER behaviour name: `KIT_THM` (`num_lc = 0`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/kit_thm.py`.

###### `KitThv`

KIT associé au comportement des milieux poreux (modélisations
thermo-hydro-mécanique). Pour plus de détails sur les modélisations
thermo-hydro-mécaniques et les modèles de comportement, on pourra
consulter les documents [R7.01.10] et [R7.01.11], ainsi que la notice
d'utilisation [U2.04.05]. Les relations KIT_XXXX permettent de résoudre
simultanément de deux à quatre équations d'équilibre. Les équations
considérées dépendent du suffixe XXXX avec la règle suivante : - M
désigne l'équation d'équilibre mécanique, - T désigne l'équation
d'équilibre thermique, - H désigne une équation d'équilibre hydraulique.
- V désigne la présence d'une phase sous forme vapeur (en plus du
liquide) Les problèmes thermo-hydro-mécaniques associés sont traités de
facon totalement couplée. Une seule lettre H signifie que le milieu
poreux est saturé (une seule variable de pression p), par exemple soit
de gaz, soit de liquide, soit d'un mélange liquide/gaz (dont la pression
du gaz est constante). Deux lettres H signifient que le milieu poreux
est non saturé (deux variables de pression p), par exemple un mélange
liquide/vapeur/gaz. La présence des deux lettres HV signifie que le
milieu poreux est saturé par un composant (en pratique de l'eau), mais
que ce composant peut être sous forme liquide ou vapeur. Il n'y a alors
qu'une équation de conservation de ce composant, donc un seul degré de
liberté pression, mais il y a un flux liquide et un flux vapeur.

ASTER behaviour name: `KIT_THV` (`num_lc = 0`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/kit_thv.py`.

###### `MetaGCine`

Loi de comportement prenant en compte la métallurgie - Ecrouissage
cinématique

ASTER behaviour name: `META_G_CINE` (`num_lc = 0`,
7 state variable(s)).
Upstream declaration: `code_aster/Behaviours/meta_g_cine.py`.

###### `MetaGCinePt`

Loi de comportement prenant en compte la métallurgie - Ecrouissage
cinématique avec plastiicité de transformation

ASTER behaviour name: `META_G_CINE_PT` (`num_lc = 0`,
7 state variable(s)).
Upstream declaration: `code_aster/Behaviours/meta_g_cine_pt.py`.

###### `MetaGCinePtre`

Loi de comportement prenant en compte la métallurgie - Ecrouissage
cinématique avec plasticité de transformation et restauration
d'écrouissage

ASTER behaviour name: `META_G_CINE_PTRE` (`num_lc = 0`,
7 state variable(s)).
Upstream declaration: `code_aster/Behaviours/meta_g_cine_ptre.py`.

###### `MetaGCineRe`

Loi de comportement prenant en compte la métallurgie - Ecrouissage
cinématique avec restauration d'écrouissage

ASTER behaviour name: `META_G_CINE_RE` (`num_lc = 0`,
7 state variable(s)).
Upstream declaration: `code_aster/Behaviours/meta_g_cine_re.py`.

###### `MetaGIsot`

Loi de comportement prenant en compte la métallurgie - Ecrouissage
isotrope

ASTER behaviour name: `META_G_ISOT` (`num_lc = 0`,
2 state variable(s)).
Upstream declaration: `code_aster/Behaviours/meta_g_isot.py`.

###### `MetaGIsotPt`

Loi de comportement prenant en compte la métallurgie - Ecrouissage
isotrope avec plasticité de transformation

ASTER behaviour name: `META_G_ISOT_PT` (`num_lc = 0`,
2 state variable(s)).
Upstream declaration: `code_aster/Behaviours/meta_g_isot_pt.py`.

###### `MetaGIsotPtre`

Loi de comportement prenant en compte la métallurgie - Ecrouissage
isotrope avec plasticité de transformation et resturation d'écrouissage

ASTER behaviour name: `META_G_ISOT_PTRE` (`num_lc = 0`,
2 state variable(s)).
Upstream declaration: `code_aster/Behaviours/meta_g_isot_ptre.py`.

###### `MetaGIsotRe`

Loi de comportement prenant en compte la métallurgie - Ecrouissage
isotrope avec resturation d'écrouissage

ASTER behaviour name: `META_G_ISOT_RE` (`num_lc = 0`,
2 state variable(s)).
Upstream declaration: `code_aster/Behaviours/meta_g_isot_re.py`.

###### `MetaPCineLine`

Loi de comportement elastoplastique à écrouissage cinématique linéaire,
prenant en compte la métallurgie

ASTER behaviour name: `META_P_CINE_LINE` (`num_lc = 0`,
6 state variable(s)).
Upstream declaration: `code_aster/Behaviours/meta_p_cine_line.py`.

###### `MetaPIsotLine`

Loi de comportement elastoplastique à écrouissage isotrope linéaire,
prenant en compte la métallurgie

ASTER behaviour name: `META_P_ISOT_LINE` (`num_lc = 0`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/meta_p_isot_line.py`.

###### `MetaPIsotTrac`

Loi de comportement elastoplastique à écrouissage isotrope non linéaire,
prenant en compte la métallurgie

ASTER behaviour name: `META_P_ISOT_TRAC` (`num_lc = 0`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/meta_p_isot_trac.py`.

###### `MetaVCineLine`

Loi de comportement elasto-visco-plastique à écrouissage cinématique
linéaire, prenant en compte la métallurgie

ASTER behaviour name: `META_V_CINE_LINE` (`num_lc = 0`,
6 state variable(s)).
Upstream declaration: `code_aster/Behaviours/meta_v_cine_line.py`.

###### `MetaVIsotLine`

Loi de comportement elasto-visco-plastique à écrouissage isotrope
linéaire, prenant en compte la métallurgie

ASTER behaviour name: `META_V_ISOT_LINE` (`num_lc = 0`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/meta_v_isot_line.py`.

###### `MetaVIsotTrac`

Loi de comportement elasto-visco-plastique à écrouissage isotrope non
linéaire, prenant en compte la métallurgie

ASTER behaviour name: `META_V_ISOT_TRAC` (`num_lc = 0`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/meta_v_isot_trac.py`.

###### `Multifibre`

Poutres multifibres

ASTER behaviour name: `MULTIFIBRE` (`num_lc = 0`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/multifibre.py`.

###### `Petit`

Algo pour résolution en petites déformations.

ASTER behaviour name: `PETIT` (`num_lc = 0`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/petit.py`.

###### `PetitReac`

Algo pour résolution en grandes déformations.

ASTER behaviour name: `PETIT_REAC` (`num_lc = 0`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/petit_reac.py`.

###### `Pmf`

Modélisations PMF

ASTER behaviour name: `PMF` (`num_lc = 0`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/pmf.py`.

###### `ReguViscElas`

Algo pour régularisation.

ASTER behaviour name: `REGU_VISC_ELAS` (`num_lc = 0`,
8 state variable(s)).
Upstream declaration: `code_aster/Behaviours/regu_visc_elas.py`.

###### `RestEcroCine`

Restauration d'écrouissage

ASTER behaviour name: `REST_ECRO_CINE` (`num_lc = 0`,
6 state variable(s)).
Upstream declaration: `code_aster/Behaviours/rest_ecro_cine.py`.

###### `RestEcroEcmi`

Restauration d'écrouissage

ASTER behaviour name: `REST_ECRO_ECMI` (`num_lc = 0`,
7 state variable(s)).
Upstream declaration: `code_aster/Behaviours/rest_ecro_ecmi.py`.

###### `RestEcroIsot`

Restauration d'écrouissage

ASTER behaviour name: `REST_ECRO_ISOT` (`num_lc = 0`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/rest_ecro_isot.py`.

###### `RuptFrag`

Relation de comportement non locale basée sur la formulation de J.J.
Marigo et G. Francfort de la mécanique de la rupture (pas d'équivalent
en version locale). Ce modèle décrit l'apparition et la propagation de
fissures dans un matériau élastique (cf. [R7.02.11]).

ASTER behaviour name: `RUPT_FRAG` (`num_lc = 0`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/rupt_frag.py`.

###### `SechBazant`

Relation de comportement de thermique non lineaire pour modéliser le
séchage du béton suivant le modèle de Bazant

ASTER behaviour name: `SECH_BAZANT` (`num_lc = 0`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/sech_bazant.py`.

###### `SechGranger`

Relation de comportement de thermique non lineaire pour modéliser le
séchage du béton suivant le modèle de Granger

ASTER behaviour name: `SECH_GRANGER` (`num_lc = 0`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/sech_granger.py`.

###### `SechMensi`

Relation de comportement de thermique non lineaire pour modéliser le
séchage du béton suivant le modèle de Mensi

ASTER behaviour name: `SECH_MENSI` (`num_lc = 0`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/sech_mensi.py`.

###### `SechNappe`

Relation de comportement de thermique non lineaire pour modéliser le
séchage du béton

ASTER behaviour name: `SECH_NAPPE` (`num_lc = 0`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/sech_nappe.py`.

###### `SechRft`

Relation de comportement de thermique non lineaire pour modéliser le
séchage du béton suivant le modèle de Richards Fick avec tempétarute
(RFT)

ASTER behaviour name: `SECH_RFT` (`num_lc = 0`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/sech_rft.py`.

###### `TherHydr`

Relation de comportement de thermique non lineaire avec hydratation

ASTER behaviour name: `THER_HYDR` (`num_lc = 0`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/ther_hydr.py`.

###### `TherNl`

Relation de comportement de thermique non lineaire

ASTER behaviour name: `THER_NL` (`num_lc = 0`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/ther_nl.py`.

###### `Vide`

comportement inopérant, nécessaire pour THM quand absence de
comportement mécanique

ASTER behaviour name: `VIDE` (`num_lc = 0`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/vide.py`.

###### `ViscMaxwellMt`

Visco-elastic Maxwell model Mori-Tanaka-Sensei

ASTER behaviour name: `VISC_MAXWELL_MT` (`num_lc = 0`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/visc_maxwell_mt.py`.

###### `Zirc`

phases metallurgiques du zirconium

ASTER behaviour name: `ZIRC` (`num_lc = 0`,
3 state variable(s)).
Upstream declaration: `code_aster/Behaviours/zirc.py`.

###### `ZircMeca`

Metallurgical phases for Zircaloy - Using in mechanical behaviour law

ASTER behaviour name: `ZIRC_MECA` (`num_lc = 0`,
3 state variable(s)).
Upstream declaration: `code_aster/Behaviours/zirc_meca.py`.

###### `Edgar`

Modèle métallurgique standard pour le zircaloy

ASTER behaviour name: `EDGAR` (`num_lc = 1`,
2 state variable(s)).
Upstream declaration: `code_aster/Behaviours/edgar.py`.

###### `Elas`

élasticité linéaire isotrope

ASTER behaviour name: `ELAS` (`num_lc = 1`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/elas.py`.

###### `InterfPouElas`

Loi élastique pour l'élément d'interface sol-pieu (3D_INTERF_POU)

ASTER behaviour name: `INTERF_POU_ELAS` (`num_lc = 1`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/interf_pou_elas.py`.

###### `LiquSatu`

Loi de comportement pour un milieux poreux saturé par un seul liquide
(Cf. [R7.01.11] pour plus de détails).

ASTER behaviour name: `LIQU_SATU` (`num_lc = 1`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/liqu_satu.py`.

###### `Ther`

Relation de comportement thermique pour la thm

ASTER behaviour name: `THER` (`num_lc = 1`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/ther.py`.

###### `Gaz`

Loi de comportement d'un gaz parfait, c'est-à-dire vérifiant la relation
P/rho rho la masse volumique, Mv la masse molaire, R la constante de
Boltzman et T la température (Cf. [R7.01.11]). Pour milieu saturé
uniquement.

ASTER behaviour name: `GAZ` (`num_lc = 2`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/gaz.py`.

###### `InterfPouCine`

Loi à écrouissage linéaire cinématique pour l'élément d'interface
sol-pieu (3D_INTERF_POU)

ASTER behaviour name: `INTERF_POU_CINE` (`num_lc = 2`,
3 state variable(s)).
Upstream declaration: `code_aster/Behaviours/interf_pou_cine.py`.

###### `ViscIsotLine`

Loi viscoplastique avec critère de Von Mises, écrouissage isotrope
linéaire et viscosité en sinh

ASTER behaviour name: `VISC_ISOT_LINE` (`num_lc = 2`,
2 state variable(s)).
Upstream declaration: `code_aster/Behaviours/visc_isot_line.py`.

###### `ViscIsotTrac`

Loi viscoplastique avec critère de Von Mises, écrouissage isotrope et
viscosité en sinh

ASTER behaviour name: `VISC_ISOT_TRAC` (`num_lc = 2`,
2 state variable(s)).
Upstream declaration: `code_aster/Behaviours/visc_isot_trac.py`.

###### `VmisIsotLine`

Loi de plasticité de Von Mises à écrouissage linéaire [R5.03.02]

ASTER behaviour name: `VMIS_ISOT_LINE` (`num_lc = 2`,
2 state variable(s)).
Upstream declaration: `code_aster/Behaviours/vmis_isot_line.py`.

###### `VmisIsotPuis`

Loi de plasticité de Von Mises à écrouissage isotrope défini par une
courbe de traction analytique avec une loi puissance

ASTER behaviour name: `VMIS_ISOT_PUIS` (`num_lc = 2`,
2 state variable(s)).
Upstream declaration: `code_aster/Behaviours/vmis_isot_puis.py`.

###### `VmisIsotTrac`

Loi de plasticité de Von Mises à écrouissage isotrope défini par une
courbe de traction affine par morceaux [R5.03.02]

ASTER behaviour name: `VMIS_ISOT_TRAC` (`num_lc = 2`,
2 state variable(s)).
Upstream declaration: `code_aster/Behaviours/vmis_isot_trac.py`.

###### `Waeckel`

Modèle métallurgique standard pour l'acier

ASTER behaviour name: `WAECKEL` (`num_lc = 2`,
3 state variable(s)).
Upstream declaration: `code_aster/Behaviours/waeckel.py`.

###### `Jma`

Modèle métallurgique avec revenu pour l'acier

ASTER behaviour name: `JMA` (`num_lc = 3`,
4 state variable(s)).
Upstream declaration: `code_aster/Behaviours/jma.py`.

###### `LiquVape`

Loi de comportement pour un milieux poreux saturé par un composant
présent sous forme liquide ou vapeur avec changement de phase (Cf.
[R7.01.11] pour plus de détails).

ASTER behaviour name: `LIQU_VAPE` (`num_lc = 3`,
3 state variable(s)).
Upstream declaration: `code_aster/Behaviours/liqu_vape.py`.

###### `VmisCineGc`

Loi de Von Mises en 1D-2D - Écrouissage cinématique linéaire.
Application aux études en génie civil : armatures, trellis soudés,
plaque tôles

ASTER behaviour name: `VMIS_CINE_GC` (`num_lc = 3`,
12 state variable(s)).
Upstream declaration: `code_aster/Behaviours/vmis_cine_gc.py`.

###### `VmisCineLine`

Loi de Von Mises - avec écrouissage cinématique linéaire

ASTER behaviour name: `VMIS_CINE_LINE` (`num_lc = 3`,
7 state variable(s)).
Upstream declaration: `code_aster/Behaviours/vmis_cine_line.py`.

###### `VmisEcmiLine`

Relation de comportement d'élasto-plasticité de VON MISES à écrouissage
combiné, cinématique linéaire et isotrope linéaire (Cf. [R5.03.16] pour
plus de détails).

ASTER behaviour name: `VMIS_ECMI_LINE` (`num_lc = 3`,
8 state variable(s)).
Upstream declaration: `code_aster/Behaviours/vmis_ecmi_line.py`.

###### `VmisEcmiTrac`

Relation de comportement d'élasto-plasticité de VON MISES à écrouissage
combiné, cinématique linéaire et isotrope non linéaire (Cf. [R5.03.16]
pour plus de détails). L'écrouissage isotrope est donné par une courbe
de traction ou éventuellement par plusieurs courbes si celles ci
dépendent de la température.

ASTER behaviour name: `VMIS_ECMI_TRAC` (`num_lc = 3`,
8 state variable(s)).
Upstream declaration: `code_aster/Behaviours/vmis_ecmi_trac.py`.

###### `LiquVapeGaz`

Loi de comportement pour un milieu poreux non saturé eau/vapeur/air sec
avec changement de phase (Cf. [R7.01.11] pour plus de détails).

ASTER behaviour name: `LIQU_VAPE_GAZ` (`num_lc = 4`,
3 state variable(s)).
Upstream declaration: `code_aster/Behaviours/liqu_vape_gaz.py`.

###### `ViscCin1Chab`

Loi élasto-visco-plastique de Chaboche à 1 variable cinématique

ASTER behaviour name: `VISC_CIN1_CHAB` (`num_lc = 4`,
8 state variable(s)).
Upstream declaration: `code_aster/Behaviours/visc_cin1_chab.py`.

###### `ViscCin2Chab`

Loi élasto-visco-plastique de Chaboche à 2 variables cinématiques

ASTER behaviour name: `VISC_CIN2_CHAB` (`num_lc = 4`,
14 state variable(s)).
Upstream declaration: `code_aster/Behaviours/visc_cin2_chab.py`.

###### `ViscCin2Memo`

Loi élasto-visco-plastique de Chaboche à 2 variables cinématiques et
effet de memoire

ASTER behaviour name: `VISC_CIN2_MEMO` (`num_lc = 4`,
28 state variable(s)).
Upstream declaration: `code_aster/Behaviours/visc_cin2_memo.py`.

###### `ViscCin2Nrad`

Loi élasto-visco-plastique de J.L.Chaboche à 2 variables cinématiques
qui rend compte du comportement cyclique en élasto-plasticité avec 2
tenseurs d'écrouissage cinématique non linéaire, un écrouissage isotrope
non linéaire, un effet d'écrouissage sur les variables tensorielles de
rappel, et prise en compte de la non proportionnalité du chargement.
Toutes les constantes du matériau peuvent éventuellement dépendre de la
température.

ASTER behaviour name: `VISC_CIN2_NRAD` (`num_lc = 4`,
14 state variable(s)).
Upstream declaration: `code_aster/Behaviours/visc_cin2_nrad.py`.

###### `ViscMemoNrad`

Loi élasto-visco-plastique de J.L.Chaboche à 2 variables cinématiques
qui rend compte du comportement cyclique en élasto-plasticité avec 2
tenseurs d'écrouissage cinématique non linéaire, un écrouissage isotrope
non linéaire, un effet d'écrouissage sur les variables tensorielles de
rappel, un effet de mémoire du plus grand écrouissage, et prise en
compte de la non proportionnalité du chargement. Toutes les constantes
du matériau peuvent éventuellement dépendre de la température.

ASTER behaviour name: `VISC_MEMO_NRAD` (`num_lc = 4`,
28 state variable(s)).
Upstream declaration: `code_aster/Behaviours/visc_memo_nrad.py`.

###### `VmisCin1Chab`

Loi élastoplastique de J.L.Chaboche à 1 variable cinématique qui rend
compte du comportement cyclique en élasto-plasticité avec un tenseur
d'écrouissage cinématique non linéaire, un écrouissage isotrope non
linéaire, un effet d'écrouissage sur la variable tensorielle de rappel.
Toutes les constantes du matériau peuvent éventuellement dépendre de la
température.

ASTER behaviour name: `VMIS_CIN1_CHAB` (`num_lc = 4`,
8 state variable(s)).
Upstream declaration: `code_aster/Behaviours/vmis_cin1_chab.py`.

###### `VmisCin2Chab`

Loi élastoplastique de J.L.Chaboche à 2 variables cinématiques qui rend
compte du comportement cyclique en élasto-plasticité avec 2 tenseurs
d'écrouissage cinématique non linéaire, un écrouissage isotrope non
linéaire, un effet d'écrouissage sur les variables tensorielles de
rappel. Toutes les constantes du matériau peuvent éventuellement
dépendre de la température.

ASTER behaviour name: `VMIS_CIN2_CHAB` (`num_lc = 4`,
14 state variable(s)).
Upstream declaration: `code_aster/Behaviours/vmis_cin2_chab.py`.

###### `VmisCin2Memo`

Loi élastoplastique de J.L.Chaboche à 2 variables cinématiques qui rend
compte du comportement cyclique en élasto-plasticité avec 2 tenseurs
d'écrouissage cinématique non linéaire, un écrouissage isotrope non
linéaire, un effet d'écrouissage sur les variables tensorielles de
rappel et une effet de mémoire du plus grand écrouissage. Toutes les
constantes du matériau peuvent éventuellement dépendre de la
température.

ASTER behaviour name: `VMIS_CIN2_MEMO` (`num_lc = 4`,
28 state variable(s)).
Upstream declaration: `code_aster/Behaviours/vmis_cin2_memo.py`.

###### `VmisCin2Nrad`

Loi élastoplastique de J.L.Chaboche à 2 variables cinématiques qui rend
compte du comportement cyclique en élasto-plasticité avec 2 tenseurs
d'écrouissage cinématique non linéaire, un écrouissage isotrope non
linéaire, un effet d'écrouissage sur les variables tensorielles de
rappel, et prise en compte de la non proportionnalité du chargement.
Toutes les constantes du matériau peuvent éventuellement dépendre de la
température.

ASTER behaviour name: `VMIS_CIN2_NRAD` (`num_lc = 4`,
14 state variable(s)).
Upstream declaration: `code_aster/Behaviours/vmis_cin2_nrad.py`.

###### `VmisMemoNrad`

Loi élastoplastique de J.L.Chaboche à 2 variables cinématiques qui rend
compte du comportement cyclique en élasto-plasticité avec 2 tenseurs
d'écrouissage cinématique non linéaire, un écrouissage isotrope non
linéaire, un effet d'écrouissage sur les variables tensorielles de
rappel, un effet de mémoire du plus grand écrouissage, et prise en
compte de la non proportionnalité du chargement. Toutes les constantes
du matériau peuvent éventuellement dépendre de la température.

ASTER behaviour name: `VMIS_MEMO_NRAD` (`num_lc = 4`,
28 state variable(s)).
Upstream declaration: `code_aster/Behaviours/vmis_memo_nrad.py`.

###### `LiquGaz`

Loi de comportement pour un milieu poreux non saturé liquide/gaz sans
changement de phase (Cf. [R7.01.11] pour plus de détails).

ASTER behaviour name: `LIQU_GAZ` (`num_lc = 5`,
2 state variable(s)).
Upstream declaration: `code_aster/Behaviours/liqu_gaz.py`.

###### `LiquGazAtm`

Loi de comportement pour un milieu poreux non saturé avec un liquide et
du gaz à pression atmosphérique (Cf. [R7.01.11] pour plus de détails).

ASTER behaviour name: `LIQU_GAZ_ATM` (`num_lc = 6`,
2 state variable(s)).
Upstream declaration: `code_aster/Behaviours/liqu_gaz_atm.py`.

###### `EndoOrthBeton`

Relation de comportement anisotrope du béton avec endommagement
[R7.01.09]. Il s'agit d'une modélisation locale d'endommagement prenant
en compte la refermeture des fissures.

ASTER behaviour name: `ENDO_ORTH_BETON` (`num_lc = 7`,
7 state variable(s)).
Upstream declaration: `code_aster/Behaviours/endo_orth_beton.py`.

###### `Mazars`

Loi d'endommagement isotrope élastique-fragile du béton, suivant le
modèle de Mazars. Elle permet de prendre en comtpe l'adoucissement et
distingue l'endommagemetn en traction et en compression. Une seule
variable d'endommagement scalaire est utilisée (cf [R7.01.08]). En cas
de chargement thermique, les coefficients matériau dépendent de la
température maximale atteinte au point de Gauss considéré, et la
dilatation thermique, supposée linéaire, ne contribue pas à l'évolution
de l'endommagement.

ASTER behaviour name: `MAZARS` (`num_lc = 8`,
4 state variable(s)).
Upstream declaration: `code_aster/Behaviours/mazars.py`.

###### `MazarsUnil`

Loi de comportement : Mazars Unilatéral dit "Mu model". Loi
d'endommagement isotrope élastique-fragile du béton. Permet de rendre
compte de l'adoucissement en compression et la fragilité en traction. -
Distingue l'endommagement en traction et en compression. - Deux
variables d'endommagement scalaire sont utilisées pour faire la
distinction entre l'endommagement de traction et de compression. Dans le
cas des poutres multifibres : - Le comportement est 1D : SIXX En
contrainte plane : - Le comportement est CP : SIXX, SIYY, SIXY (SIZZ=0)
Cette version permet de rendre mieux compte du cisaillement. Il n'y a
pas de couplage possible avec d'autres phénomènes tels que le fluage.

ASTER behaviour name: `MAZARS_UNIL` (`num_lc = 8`,
8 state variable(s)).
Upstream declaration: `code_aster/Behaviours/mazars_unil.py`.

###### `BetonReglePr`

Relation de comportement de béton (développée par la société NECS) dite
'parabole rectangle' [R7.01.27]. La loi BETON_REGLE_PR est une loi de
béton se rapprochant des lois réglementaires de béton (d'où son nom) qui
a les caractéristiques sommaires suivantes : -c'est une loi 2D et plus
exactement 2 fois 1D : dans le repère propre de déformation, on écrit
une loi 1D contrainte-déformation ; -la loi 1D sur chaque direction de
déformation propre est la suivante : * en traction, linéaire jusqu'à un
pic, adoucissement linéaire jusqu'à 0 ; * en compression, une loi
puissance jusqu'à un plateau (d'ou PR : parabole-rectangle).

ASTER behaviour name: `BETON_REGLE_PR` (`num_lc = 9`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/beton_regle_pr.py`.

###### `LiquAdGazVape`

Loi de comportement pour un milieu poreux non saturé eau/vapeur/air
sec/air dissous avec changement de phase (Cf. [R7.01.11] pour plus de
détails).

ASTER behaviour name: `LIQU_AD_GAZ_VAPE` (`num_lc = 9`,
3 state variable(s)).
Upstream declaration: `code_aster/Behaviours/liqu_ad_gaz_vape.py`.

###### `CzmExpReg`

Relation de comportement cohésive (Cohesive Zone Model EXPonentielle
REGularisée) (Cf. [R7.02.11]) modélisant l'ouverture d'une fissure.
Cette loi est utilisable avec l'élément fini de type joint (Cf.
[R3.06.09]) et permet d'introduire une force de cohésion entre les
lèvres de la fissure. Par ailleurs l'utilisation de ce modèle requiert
souvent la présence du pilotage par PRED_ELAS (cf. [U4.51.03]).

ASTER behaviour name: `CZM_EXP_REG` (`num_lc = 10`,
9 state variable(s)).
Upstream declaration: `code_aster/Behaviours/czm_exp_reg.py`.

###### `LiquAdGaz`

Loi de comportement pour un milieu poreux non saturé eau/vapeur/air
sec/air dissous avec changement de phase (Cf. [R7.01.11] pour plus de
détails).

ASTER behaviour name: `LIQU_AD_GAZ` (`num_lc = 10`,
5 state variable(s)).
Upstream declaration: `code_aster/Behaviours/liqu_ad_gaz.py`.

###### `CzmLinReg`

Relation de comportement cohésive (Cohesive Zone Model LINéaire
REGularisée) (Cf. [R7.02.11]) modélisant l'ouverture et la propagation
d'une fissure. L'intérêt d'une telle loi, comparée à CZM_EXP_REG, est de
pouvoir représenter un vrai front de rupture. Ce dernier est visible
grâce à la variable interne V3 (V3=2 correspond à un élément totalement
cassé). Cette loi est utilisable avec l'élément fini de type joint (Cf.
[R3.06.09]) et permet d'introduire une force de cohésion entre les
lèvres de la fissure. Par ailleurs l'utilisation de ce modèle requiert
souvent la présence du pilotage par PRED_ELAS (cf. [U4.51.03]).

ASTER behaviour name: `CZM_LIN_REG` (`num_lc = 11`,
9 state variable(s)).
Upstream declaration: `code_aster/Behaviours/czm_lin_reg.py`.

###### `JointBa`

Relation de comportement locale en 2D décrivant le phénomène de la
liaison acier - béton pour les structures en béton armé. Elle permet de
rendre compte de l'influence de la liaison dans la redistribution des
contraintes dans le corps du béton ainsi que la prédiction des fissures
et leur espacement. Disponible pour des chargements en monotone et en
cyclique, elle prend en compte les effets du frottement des fissures, et
du confinement. Une seule variable d'endommagement scalaire est utilisée
(cf. [R7.01.21] pour plus de détails).

ASTER behaviour name: `JOINT_BA` (`num_lc = 13`,
6 state variable(s)).
Upstream declaration: `code_aster/Behaviours/joint_ba.py`.

###### `KitMeta`

Loi de comportement prenant en compte la métallurgie

ASTER behaviour name: `KIT_META` (`num_lc = 15`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/kit_meta.py`.

###### `DruckPrager`

Loi de Drucker_Prager, associée, pour la mécanique des sols (cf.
[R7.01.16] pour plus de détails). On suppose toutefois que le
coefficient de dilatation thermique est constant. L'écrouissage peut
être linéaire ou parabolique.

ASTER behaviour name: `DRUCK_PRAGER` (`num_lc = 16`,
3 state variable(s)).
Upstream declaration: `code_aster/Behaviours/druck_prager.py`.

###### `NortonHoff`

Loi de visco-plasticité indépendante de la température, régularisant la
loi rigide-plastique de Von Mises à utiliser pour le calcul de charges
limites de structures, à seuil de VON MISES. Le seul paramètre matériau
est la limite d'élasticité à renseigner dans l'opérateur DEFI_MATERIAU
[U4.43.01] sous le mot-clé ECRO_LINE (Cf. [R7.07.01] et [R5.03.12] pour
plus de détails). Pour le calcul de la charge limite, il existe un mot
clé spécifique sous PILOTAGE pour ce modèle (voir mot clé
PILOTAGE='ANA_LIM' de STAT_NON_LINE [U4.51.03]). Il est fortement
conseillé d'employer de la recherche linéaire (voir mot clé
RECH_LINEAIRE de STAT_NON_LINE [U4.51.03]). En effet, le calcul de la
charge limite requiert beaucoup d'itérations de recherche linéaire (de
l'ordre de 50) et d'itérations de Newton (de l'ordre de 50).

ASTER behaviour name: `NORTON_HOFF` (`num_lc = 17`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/norton_hoff.py`.

###### `ViscTaheri`

Relation de comportement (visco)-plastique de S.Taheri modélisant la
réponse de matériaux sous chargement plastique cyclique, et en
particulier permettant de représenter les effets de rochet. Les données
nécessaires sont fournies dans l'opérateur DEFI_MATERIAU [U4.43.01],
sous les mots clés TAHERI(_FO) pour la description de l'écrouissage,
LEMAITRE(_FO) pour la viscosité et ELAS(_FO) (Cf. [R5.03.05] pour plus
de détails). En l'absence de LEMAITRE, la loi est purement
élasto-plastique.

ASTER behaviour name: `VISC_TAHERI` (`num_lc = 18`,
9 state variable(s)).
Upstream declaration: `code_aster/Behaviours/visc_taheri.py`.

###### `ElasHyper`

Relation de comportement hyper-élastique généralisant le modèle de
Mooney-Rivlin généralisé Sous sa version incrémentale, elle permet de
prendre en compte des déplacements et contraintes initiaux donnés sous
le mot clé ETAT_INIT. Cette relation n'est supportée qu'en grandes
déformations (DEFORMATION='GREEN') cf.[R5.03.23].

ASTER behaviour name: `ELAS_HYPER` (`num_lc = 19`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/elas_hyper.py`.

###### `BetonUmlv`

Comportement de fluage propre du béton avec distinction fluage volumique
et fluage déviatorique (R7.01.16)

ASTER behaviour name: `BETON_UMLV` (`num_lc = 21`,
21 state variable(s)).
Upstream declaration: `code_aster/Behaviours/beton_umlv.py`.

###### `CamClay`

Comportement élastoplastique des sols normalement consolidés (argiles
par exemple). cf. R7.01.14 La partie élastique est non-linéaire. La
partie plastique peut être durcissante ou adoucissante. Si le modèle
CAM_CLAY est utilisé avec la modélisation THM, le mot clé PORO renseigné
sous CAM_CLAY et sous THM_INIT doit être le même.

ASTER behaviour name: `CAM_CLAY` (`num_lc = 22`,
7 state variable(s)).
Upstream declaration: `code_aster/Behaviours/cam_clay.py`.

###### `Cjs`

Comportement élastoplastique multicritère des sols cf. R7.01.13

ASTER behaviour name: `CJS` (`num_lc = 23`,
16 state variable(s)).
Upstream declaration: `code_aster/Behaviours/cjs.py`.

###### `CorrAcier`

Comportement élastoplastique avec endommagement dépendant du taux de
corrosion, cf. R7.01.20

ASTER behaviour name: `CORR_ACIER` (`num_lc = 24`,
3 state variable(s)).
Upstream declaration: `code_aster/Behaviours/corr_acier.py`.

###### `Rankine`

Loi de Rankine, associee, pour les joints de plots (cf. [R7.01.39] pour
plus de details). Pas d'ecrouissage

ASTER behaviour name: `RANKINE` (`num_lc = 25`,
9 state variable(s)).
Upstream declaration: `code_aster/Behaviours/rankine.py`.

###### `BetonGranger`

Comportement de fluage propre du beton, identique à BETON_GRANGER_V mais
traitant uniquement un comportement isotherme. cf. R7.01.01

ASTER behaviour name: `BETON_GRANGER` (`num_lc = 26`,
55 state variable(s)).
Upstream declaration: `code_aster/Behaviours/beton_granger.py`.

###### `BetonGrangerV`

Comportement de fluage propre du beton avec prise en compte du phénomène
de vieillissement, cf. R7.01.01

ASTER behaviour name: `BETON_GRANGER_V` (`num_lc = 26`,
55 state variable(s)).
Upstream declaration: `code_aster/Behaviours/beton_granger_v.py`.

###### `GranIrraLog`

Relation de comportement de fluage et de grandissement sous irradiation
pour les assemblages combustibles, similaire à la loi VISC_IRRA_LOG pour
la déformation viscoplastique, et intégrant en plus une déformation de
grandissement sous irradiation (cf. [R5.03.09]). Le champ de fluence est
défini par le mot-clé AFFE_VARC de la commande AFFE_MATERIAU. Le
grandissement ne se faisant que selon une direction, il est nécessaire
dans les cas 3D et 2D de donner la direction du grandissement par
l'opérande ANGL_REP du mot clé MASSIF de l'opérateur AFFE_CARA_ELEM

ASTER behaviour name: `GRAN_IRRA_LOG` (`num_lc = 28`,
3 state variable(s)).
Upstream declaration: `code_aster/Behaviours/gran_irra_log.py`.

###### `LemaitreIrra`

Relation de comportement de fluage et de grandissement sous irradiation
pour les assemblages combustibles. Le champ de fluence est défini par le
mot-clé AFFE_VARC de la commande AFFE_MATERIAU. Le grandissement ne se
faisant que selon une direction, il est nécessaire dans les cas 3D et 2D
de donner la direction du grandissement par l'opérande ANGL_REP du mot
clé MASSIF de l'opérateur AFFE_CARA_ELEM. Pour les poutres, le fluage et
le grandissement n'ont lieu que dans le sens axial de la poutre : dans
les autres directions, le comportement est élastique. Le schéma
d'intégration est DEKKER ou semi-DEKKER, mais on conseille d'utiliser
une intégration semi-DEKKER c'est-à-dire PARM_THETA=
0.5,RESO_INTE=DEKKER.

ASTER behaviour name: `LEMAITRE_IRRA` (`num_lc = 28`,
3 state variable(s)).
Upstream declaration: `code_aster/Behaviours/lemaitre_irra.py`.

###### `LemaSeuil`

Relation de comportement viscoplastique avec seuil sous irradiation pour
les assemblages combustibles cf. [R5.03.08]

ASTER behaviour name: `LEMA_SEUIL` (`num_lc = 28`,
2 state variable(s)).
Upstream declaration: `code_aster/Behaviours/lema_seuil.py`.

###### `ViscIrraLog`

Loi de fluage axial sous irradiation des assemblages combustibles. Elle
permet de modéliser le fluage primaire et secondaire, paramétrés par la
fluence neutronique (cf. [R5.03.09]). Le champ de fluence est défini par
le mot-clé AFFE_VARC de la commande AFFE_MATERIAU.

ASTER behaviour name: `VISC_IRRA_LOG` (`num_lc = 28`,
2 state variable(s)).
Upstream declaration: `code_aster/Behaviours/visc_irra_log.py`.

###### `Lemaitre`

Relation de comportement visco-plastique non linéaire de Lemaitre (sans
seuil), cf. [R5.03.08]. Un cas particulier de cette relation (en
annulant le paramètre UN_SUR_M) donne une relation de NORTON. La
correspondance des variables internes permet le chaînage avec un calcul
utilisant un comportement élasto-plastique avec écrouissage isotrope
(VMIS_ISOT_LINE, VMIS_ISOT_TRAC, VMIS_ISOT_PUIS). L'ntégration de ce
modèle est réalisée par une méthode semi-DEKKER (PARM_THETA=0.5) ou
DEKKER (PARM_THETA=1)

ASTER behaviour name: `LEMAITRE` (`num_lc = 29`,
2 state variable(s)).
Upstream declaration: `code_aster/Behaviours/lemaitre.py`.

###### `Irrad3m`

Relation de comportement élasto-plastique sous irradiation des aciers
inoxydables 304 et 316, matériaux dont sont constitués les structures
internes de cuve des réacteurs nucléaires (cf. [R5.03.13]). Le champ de
fluence est défini par le mot-clé AFFE_VARC de la commande
AFFE_MATERIAU. Le modèle prend en compte la plasticité, le fluage sous
irradiation, le gonflement sous flux neutronique.

ASTER behaviour name: `IRRAD3M` (`num_lc = 30`,
7 state variable(s)).
Upstream declaration: `code_aster/Behaviours/irrad3m.py`.

###### `RoussPr`

Relation de comportement élasto-plastique de G.Rousselier, en petites
déformations. Elle permet de rendre compte de la croissance des cavités
et de décrire la rupture ductile, cf. [R5.03.06]). On peut également
prendre en compte la nucléation des cavités. Il faut alors renseigner le
paramètre AN (mot clé non activé pour le modèle ROUSSELIER et
ROUSS_VISC) sous ROUSSELIER(_FO). Pour faciliter l'intégration de ce
modèle, il est conseillé d'utiliser le redécoupage automatique local du
pas de temps (mot clé ITER_INTE_PAS)

ASTER behaviour name: `ROUSS_PR` (`num_lc = 30`,
5 state variable(s)).
Upstream declaration: `code_aster/Behaviours/rouss_pr.py`.

###### `RoussVisc`

Relation de comportement élasto-visco-plastique de G.Rousselier, en
petites déformations. Elle permet de rendre compte de la croissance des
cavités et de décrire la rupture ductile. Pour faciliter l'intégration
de ce modèle, il est conseillé d'utiliser le redécoupage automatique
local du pas de temps (ITER_INTE_PAS). Pour l'intégration de cette loi,
une theta-méthode est disponible et on conseille d'utiliser une
intégration semi-NEWTON_1D c'est-à-dire : PARM_THETA = 0.5.

ASTER behaviour name: `ROUSS_VISC` (`num_lc = 30`,
5 state variable(s)).
Upstream declaration: `code_aster/Behaviours/rouss_visc.py`.

###### `Vendochab`

Modèle viscoplastique couplé à l'endommagement isotrope de
Lemaitre-Chaboche [R5.03.15]. Ce modèle s'emploie avec les mots clés
DEFORMATION = PETIT ou PETIT_REAC.

ASTER behaviour name: `VENDOCHAB` (`num_lc = 31`,
10 state variable(s)).
Upstream declaration: `code_aster/Behaviours/vendochab.py`.

###### `ViscEndoLema`

Modèle viscoplastique couplé à l'endommagement isotrope de
Lemaitre-Chaboche [R5.03.15]. Ce modèle s'emploie avec les mots clés
DEFORMATION = PETIT ou PETIT_REAC.

ASTER behaviour name: `VISC_ENDO_LEMA` (`num_lc = 31`,
10 state variable(s)).
Upstream declaration: `code_aster/Behaviours/visc_endo_lema.py`.

###### `Hayhurst`

Modele viscoplastique couple a l'endommagement isotrope de Kachanov.

ASTER behaviour name: `HAYHURST` (`num_lc = 32`,
12 state variable(s)).
Upstream declaration: `code_aster/Behaviours/hayhurst.py`.

###### `Norton`

Modele viscoplastique isotrope de Norton

ASTER behaviour name: `NORTON` (`num_lc = 32`,
7 state variable(s)).
Upstream declaration: `code_aster/Behaviours/norton.py`.

###### `Viscochab`

Modèle élastoviscoplastique de Lemaitre-Chaboche avec effet de mémoire
et restauration. Ce modèle s'emploie avec les mots clés DEFORMATION =
PETIT ou PETIT_REAC.

ASTER behaviour name: `VISCOCHAB` (`num_lc = 32`,
28 state variable(s)).
Upstream declaration: `code_aster/Behaviours/viscochab.py`.

###### `HoekBrown`

Relation de comportement de Hoek et Brown modifiée pour la modélisation
du comportement des roches [R7.01.18] pour la mécanique pure. Pour
faciliter l'intégration de ce modèle, on peut utiliser le re-découpage
local du pas de temps (ITER_INTE_PAS).

ASTER behaviour name: `HOEK_BROWN` (`num_lc = 33`,
3 state variable(s)).
Upstream declaration: `code_aster/Behaviours/hoek_brown.py`.

###### `HoekBrownEff`

Relation de comportement de Hoek et Brown modifiée pour la modélisation
du comportement des roches [R7.01.18] pour la mécanique pure. Le
couplage est formulé en contraintes effectives. Pour faciliter
l'intégration de ce modèle, on peut utiliser le re-découpage local du
pas de temps (ITER_INTE_PAS).

ASTER behaviour name: `HOEK_BROWN_EFF` (`num_lc = 33`,
3 state variable(s)).
Upstream declaration: `code_aster/Behaviours/hoek_brown_eff.py`.

###### `Laigle`

Relation de comportement pour la modélisation des roches suivant le
modèle de Laigle, cf. le document [R7.01.15]. Pour faciliter
l'intégration de ce modèle, on peut utiliser le redécoupage automatique
local du pas de temps (ITER_INTE_PAS).

ASTER behaviour name: `LAIGLE` (`num_lc = 33`,
4 state variable(s)).
Upstream declaration: `code_aster/Behaviours/laigle.py`.

###### `Hujeux`

Relation de comportement élasto-plastique cyclique pour la mécanique des
sols (géomatériaux granulaires : argiles sableuses, normalement
consolidées ou sur-consolidées, graves) (Cf. [R7.01.23] pour plus de
détails). Ce modèle est un modèle multicritère qui comporte un mécanisme
élastique non linéaire, trois mécanismes plastiques déviatoires et un
mécanisme plastique isotrope. Pour faciliter l'intégration de ce modèle,
on peut utiliser le redécoupage automatique local du pas de temps
(ITER_INTE_PAS)

ASTER behaviour name: `HUJEUX` (`num_lc = 34`,
50 state variable(s)).
Upstream declaration: `code_aster/Behaviours/hujeux.py`.

###### `Letk`

Relation de comportement pour la modélisation élasto visco plastique des
roches suivant le modèle de Laigle et Kleine, cf. [R7.01.24].
L'opérateur relatif à la prédiction élastique est celui de l'élasticité
non linéaire spécifique à la loi.

ASTER behaviour name: `LETK` (`num_lc = 35`,
9 state variable(s)).
Upstream declaration: `code_aster/Behaviours/letk.py`.

###### `EndoIsotBeton`

Comportement élastique-fragile qui distingue traction et compression du
bétonRelation de comportement élastique fragile. Il s'agit d'une
modélisation locale à endommagement scalaire et à écrouissage isotrope
linéaire négatif qui distingue le comportement en traction et en
compression du béton (Cf. [R7.01.04] pour plus de détails).

ASTER behaviour name: `ENDO_ISOT_BETON` (`num_lc = 36`,
2 state variable(s)).
Upstream declaration: `code_aster/Behaviours/endo_isot_beton.py`.

###### `Rousselier`

Relation de comportement élasto-plastique de G.Rousselier en grandes
déformations. Elle permet de rendre compte de la croissance des cavités
et de décrire la rupture ductile. Pour faciliter l'intégration de ce
modèle, il est conseillé d'utiliser systématiquement le redécoupage
global du pas de temps (SUBD_PAS).

ASTER behaviour name: `ROUSSELIER` (`num_lc = 37`,
9 state variable(s)).
Upstream declaration: `code_aster/Behaviours/rousselier.py`.

###### `Sans`

comportement inopérant, utile à la simulation des cables de
précontrainte

ASTER behaviour name: `SANS` (`num_lc = 38`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/sans.py`.

###### `CzmOuvMix`

Relation de comportement cohésive (Cohesive Zone Model OUVerture MIXte)
(Cf. [R7.02.11]) modélisant l'ouverture et la propagation d'une fissure.
Cette loi est utilisable avec l'élément fini d'interface basé sur une
formulation mixte lagrangien augmenté (Cf. [R3.06.13]) et permet
d'introduire une force de cohésion entre les lèvres de la fissure en
mode d'ouverture uniquement. Cette loi est utilisée lorsqu'on impose des
conditions de symétrie sur l'élément d'interface. Par ailleurs
l'utilisation de ce modèle requiert souvent la présence du pilotage par
PRED_ELAS (cf. [U4.51.03]).

ASTER behaviour name: `CZM_OUV_MIX` (`num_lc = 40`,
9 state variable(s)).
Upstream declaration: `code_aster/Behaviours/czm_ouv_mix.py`.

###### `DruckPragNA`

Loi de Drucker Prager non associée, pour la mécanique des sols (cf.
[R7.01.16] pour plus de détails)

ASTER behaviour name: `DRUCK_PRAG_N_A` (`num_lc = 40`,
9 state variable(s)).
Upstream declaration: `code_aster/Behaviours/druck_prag_n_a.py`.

###### `CzmTacMix`

Relation de comportement cohésive (Cohesive Zone Model TAlon-Curnier
MIXte) (Cf. [R7.02.11]) modélisant l'ouverture et la propagation d'une
fissure. Cette loi est utilisable avec l'élément fini d'interface basé
sur une formulation mixte lagrangien augmenté (Cf. [R3.06.13]) et permet
d'introduire une force de cohésion entre les lèvres de la fissure dans
les trois modes de rupture avec une irréversibilité de type
Talon-Curnier. Attention, cette loi ne peut être utilisée lorsqu'on
impose des conditions de symétrie sur l'élément d'interface. Dans ce cas
de figure il faut utiliser CZM_OUV_MIX. Par ailleurs l'utilisation de ce
modèle requiert souvent la présence du pilotage par PRED_ELAS (cf.
[U4.51.03]).

ASTER behaviour name: `CZM_TAC_MIX` (`num_lc = 41`,
9 state variable(s)).
Upstream declaration: `code_aster/Behaviours/czm_tac_mix.py`.

###### `ViscDrucPrag`

Modele viscoplastique base sur une loi de type Drucker Prager non
associee, pour la mecanique des roches (cf. [R7.01.22] pour plus de
details).Le fluage suit la loi de Perzyna

ASTER behaviour name: `VISC_DRUC_PRAG` (`num_lc = 42`,
4 state variable(s)).
Upstream declaration: `code_aster/Behaviours/visc_druc_prag.py`.

###### `CzmFatMix`

Relation de comportement cohésive (Cohesive Zone Model FATigue MIXte)
pour la fatigue (Cf. [R7.02.11]) modélisant l'ouverture et la
propagation d'une fissure sous chargement cyclique. Cette loi est
utilisable avec l'élément fini d'interface basé sur une formulation
mixte lagrangien augmenté (Cf. [R3.06.13])

ASTER behaviour name: `CZM_FAT_MIX` (`num_lc = 43`,
9 state variable(s)).
Upstream declaration: `code_aster/Behaviours/czm_fat_mix.py`.

###### `JointMecaRupt`

Relation de comportement de contact, elastique avec resistance a la
traction et rupture pour modéliser les joints dans les barrages. Cette
loi permet également de modéliser le clavage de plots. Enfin elle permet
de modéliser, avec les éléments de joint HM, un couplage entre la
mécanique et l'écoulement de fluide dans la fissure

ASTER behaviour name: `JOINT_MECA_RUPT` (`num_lc = 45`,
20 state variable(s)).
Upstream declaration: `code_aster/Behaviours/joint_meca_rupt.py`.

###### `CzmTuron`

Relation de comportement de type CZM pour modéliser le comportement
d'une interface isotrope transverse. Basée sur le modèle de Turon 2006

ASTER behaviour name: `CZM_TURON` (`num_lc = 46`,
16 state variable(s)).
Upstream declaration: `code_aster/Behaviours/czm_turon.py`.

###### `EndoScalaire`

Comportement elastique-fragile, a endommagement scalaire, seuil
elliptique et ecrouissage isotrope lineaire negatif - R5.03.18

ASTER behaviour name: `ENDO_SCALAIRE` (`num_lc = 46`,
9 state variable(s)).
Upstream declaration: `code_aster/Behaviours/endo_scalaire.py`.

###### `EndoHeterogene`

Comportement élastique-heterogene, à endommagement - R5.03.24

ASTER behaviour name: `ENDO_HETEROGENE` (`num_lc = 47`,
12 state variable(s)).
Upstream declaration: `code_aster/Behaviours/endo_heterogene.py`.

###### `JointMecaEndo`

Relation de comportement de contact, elastique avec resistance a la
traction et rupture pour modéliser les joints dans les barrages. Enfin
elle permet de modéliser, avec les éléments de joint HM, un couplage
entre la mécanique et l'écoulement de fluide dans la fissure

ASTER behaviour name: `JOINT_MECA_ENDO` (`num_lc = 47`,
20 state variable(s)).
Upstream declaration: `code_aster/Behaviours/joint_meca_endo.py`.

###### `JointMecaFrot`

Loi elastoplastique de Mohr-Coulomb avec adhesion pour modélisation de
joints dans les barrages. Elle permet aussi de modéliser, avec les
éléments de joint hydro-mécaniques, un couplage entre la mécanique et
l'écoulement de fluide dans la fissure

ASTER behaviour name: `JOINT_MECA_FROT` (`num_lc = 48`,
18 state variable(s)).
Upstream declaration: `code_aster/Behaviours/joint_meca_frot.py`.

###### `CzmTraMix`

Relation de comportement cohésive (Cohesive Zone Model TRApèze MIXte)
pour la rupture ductile (Cf. [R7.02.11]) modélisant l'ouverture et la
propagation d'une fissure. Cette loi est utilisable avec l'élément fini
d'interface basé sur une formulation mixte lagrangien augmenté (Cf.
[R3.06.13])

ASTER behaviour name: `CZM_TRA_MIX` (`num_lc = 49`,
9 state variable(s)).
Upstream declaration: `code_aster/Behaviours/czm_tra_mix.py`.

###### `Umat`

loi de comportement dont la routine d'intégration est fournie par
l'utilisateur.

ASTER behaviour name: `UMAT` (`num_lc = 50`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/umat.py`.

###### `CzmLabMix`

Relation de comportement pour une liaison acier-béton, basée sur une
formulation mixte (Cf. [R7.02.11])

ASTER behaviour name: `CZM_LAB_MIX` (`num_lc = 51`,
5 state variable(s)).
Upstream declaration: `code_aster/Behaviours/czm_lab_mix.py`.

###### `VmisJohnCook`

Loi de plasticité de Von Mises à écrouissage de Johnson-Cook [R5.03.02]

ASTER behaviour name: `VMIS_JOHN_COOK` (`num_lc = 54`,
5 state variable(s)).
Upstream declaration: `code_aster/Behaviours/vmis_john_cook.py`.

###### `MohrCoulomb`

Loi de Mohr_Coulomb, non-associée, pour la mécanique des sols (cf.
[R7.01.16] pour plus de détails). Pas d'ecrouissage

ASTER behaviour name: `MOHR_COULOMB` (`num_lc = 55`,
3 state variable(s)).
Upstream declaration: `code_aster/Behaviours/mohr_coulomb.py`.

###### `CzmExpMix`

Relation de comportement cohésive (Cohesive Zone Model EXPonentielle en
formulation MIXte) (Cf. [R7.02.11]) modélisant l'ouverture et la
propagation d'une fissure. Cette loi est utilisable avec l'élément fini
d'interface basé sur une formulation mixte lagrangien augmenté (Cf.
[R3.06.13]) et permet d'introduire une force de cohésion entre les
lèvres de la fissure en mode d'ouverture plus proche des matériaux
quasi-fragile. Cette loi est utilisée lorsqu'on impose des conditions de
symétrie sur l'élément d'interface. Par ailleurs l'utilisation de ce
modèle requiert souvent la présence du pilotage par PRED_ELAS (cf.
[U4.51.03]).

ASTER behaviour name: `CZM_EXP_MIX` (`num_lc = 56`,
9 state variable(s)).
Upstream declaration: `code_aster/Behaviours/czm_exp_mix.py`.

###### `EndoFissExp`

Comportement élastique-fragile, à endommagement scalaire, seuil
exponentiel et non local à gradient d'endommagement - R5.03.25

ASTER behaviour name: `ENDO_FISS_EXP` (`num_lc = 57`,
9 state variable(s)).
Upstream declaration: `code_aster/Behaviours/endo_fiss_exp.py`.

###### `BetonAgeing`

To complete ...

ASTER behaviour name: `BETON_AGEING` (`num_lc = 58`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/BETON_AGEINGMFront.py`.

###### `BetonBurger`

Comportement de fluage propre du beton selon modele de burger avec non
linearite sur le fluide de Maxwell (R7.01.35)

ASTER behaviour name: `BETON_BURGER` (`num_lc = 58`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/beton_burgerMfront.py`.

###### `Barcelone`

To complete...

ASTER behaviour name: `Barcelone` (`num_lc = 58`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/BarceloneMFront.py`.

###### `Cssm`

To complete...

ASTER behaviour name: `CSSM` (`num_lc = 58`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/CSSMMFront.py`.

###### `CzmMfront`

Loi de comportement CZM utilisateur dont l'intégration est réalisée par
MFront.

ASTER behaviour name: `CZM_MFRONT` (`num_lc = 58`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/czm_mfront.py`.

###### `ElasHyperVisc`

Comportement visco-hyper-élastique cf.[RXX.XX.XX].

ASTER behaviour name: `ELAS_HYPER_VISC` (`num_lc = 58`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/elas_hyper_visc.py`.

###### `Gonfelas`

To complete ...

ASTER behaviour name: `GonfElas` (`num_lc = 58`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/GonfElasMFront.py`.

###### `HyperHill`

Relation de comportement compressibles cf.[RX.XX.XX].

ASTER behaviour name: `HYPER_HILL` (`num_lc = 58`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/hyper_hill.py`.

###### `Iwan`

To complete ...

ASTER behaviour name: `Iwan` (`num_lc = 58`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/IwanMFront.py`.

###### `Mcc`

To complete...

ASTER behaviour name: `MCC` (`num_lc = 58`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/MCCMFront.py`.

###### `MetaLemaAni`

Loi de comportement viscoplastique anisotrope prenant en compte la
métallurgie, pour le Zirconium uniquement R4.04.04 et R4.04.05

ASTER behaviour name: `META_LEMA_ANI` (`num_lc = 58`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/meta_lema_ani.py`.

###### `Mfront`

Loi de comportement utilisateur dont l'intégration est réalisée par
MFront.

ASTER behaviour name: `MFRONT` (`num_lc = 58`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/mfront.py`.

###### `MetaacierepilPt`

To complete ...

ASTER behaviour name: `MetaAcierEPIL_PT` (`num_lc = 58`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/MetaAcierEPIL_PTMFront.py`.

###### `Mohrcoulombas`

To complete ...

ASTER behaviour name: `MohrCoulombAS` (`num_lc = 58`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/MohrCoulombASMFront.py`.

###### `NlhCsrm`

To complete ...

ASTER behaviour name: `NLH_CSRM` (`num_lc = 58`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/NLH_CSRMMFront.py`.

###### `ViscIsotPlas`

Relation de comportement multi-echelle isotropique elasto-viscoplastique
cf.[R5.03.38].

ASTER behaviour name: `VISC_ISOT_PLAS` (`num_lc = 58`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/visc_isot_plasMFront.py`.

###### `Lkr`

To complete ...

ASTER behaviour name: `LKR` (`num_lc = 59`,
12 state variable(s)).
Upstream declaration: `code_aster/Behaviours/lkr.py`.

###### `EndoLocaExp`

Comportement élastique-fragile, à endommagement scalaire, seuil
exponentiel et non local à gradient d'endommagement - R5.03.25

ASTER behaviour name: `ENDO_LOCA_EXP` (`num_lc = 60`,
5 state variable(s)).
Upstream declaration: `code_aster/Behaviours/endo_loca_exp.py`.

###### `ViscMaxwell`

Visco-elastic Maxwell model

ASTER behaviour name: `VISC_MAXWELL` (`num_lc = 62`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/visc_maxwell.py`.

###### `Gtn`

Loi de plasticité de Gurson Tvergaard Needleman [R5.03.29]

ASTER behaviour name: `GTN` (`num_lc = 75`,
25 state variable(s)).
Upstream declaration: `code_aster/Behaviours/gtn.py`.

###### `ViscGtn`

Loi de viscoplasticité de Gurson Tvergaard Needleman [R5.03.29]

ASTER behaviour name: `VISC_GTN` (`num_lc = 75`,
25 state variable(s)).
Upstream declaration: `code_aster/Behaviours/visc_gtn.py`.

###### `ViscIsotNl`

Loi de viscoplasticité de Von Mises à écrouissage non linéaire
[R5.03.02]

ASTER behaviour name: `VISC_ISOT_NL` (`num_lc = 76`,
8 state variable(s)).
Upstream declaration: `code_aster/Behaviours/visc_isot_nl.py`.

###### `VmisIsotNl`

Loi de plasticité de Von Mises à écrouissage non linéaire [R5.03.02]

ASTER behaviour name: `VMIS_ISOT_NL` (`num_lc = 76`,
8 state variable(s)).
Upstream declaration: `code_aster/Behaviours/vmis_isot_nl.py`.

###### `CzmElasMix`

Relation de comportement cohésive lineaire elastique (avec
eventuellement adherence et conditions de contact

ASTER behaviour name: `CZM_ELAS_MIX` (`num_lc = 77`,
3 state variable(s)).
Upstream declaration: `code_aster/Behaviours/czm_elas_mix.py`.

###### `KicheninNl`

Loi de plasticité / viscoélasticité de Kichenin non linéaire [R5.03.36]

ASTER behaviour name: `KICHENIN_NL` (`num_lc = 77`,
14 state variable(s)).
Upstream declaration: `code_aster/Behaviours/kichenin_nl.py`.

###### `CzmFrotMix`

Relation de comportement cohésive avec contact-frottement de Coulomb

ASTER behaviour name: `CZM_FROT_MIX` (`num_lc = 78`,
7 state variable(s)).
Upstream declaration: `code_aster/Behaviours/czm_frot_mix.py`.

###### `ElasVmisLine`

Elasticité non linéaire de Von Mises - Hencky à écrouissage isotrope
linéaire

ASTER behaviour name: `ELAS_VMIS_LINE` (`num_lc = 78`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/elas_vmis_line.py`.

###### `ElasVmisPuis`

Elasticité non linéaire de Von Mises - Hencky à écrouissage isotrope
défini par une courbe de traction analytique (loi en puissance)

ASTER behaviour name: `ELAS_VMIS_PUIS` (`num_lc = 78`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/elas_vmis_puis.py`.

###### `ElasVmisTrac`

Elasticité non linéaire de Von Mises - Hencky à écrouissage isotrope
défini par une courbe de traction affine par morceaux

ASTER behaviour name: `ELAS_VMIS_TRAC` (`num_lc = 78`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/elas_vmis_trac.py`.

###### `EndoLocaTc`

Comportement quasi-fragile isotrope pour le béton - R7.01.47

ASTER behaviour name: `ENDO_LOCA_TC` (`num_lc = 79`,
9 state variable(s)).
Upstream declaration: `code_aster/Behaviours/endo_loca_tc.py`.

###### `RelaxAcier`

Loi de relaxation pour les câbles précontraint

ASTER behaviour name: `RELAX_ACIER` (`num_lc = 90`,
2 state variable(s)).
Upstream declaration: `code_aster/Behaviours/relax_acier.py`.

###### `VmisAsymLine`

Relation de comportement des barres, à écrouissage isotrope, et seuils
non symétrique en traction et compression

ASTER behaviour name: `VMIS_ASYM_LINE` (`num_lc = 91`,
4 state variable(s)).
Upstream declaration: `code_aster/Behaviours/vmis_asym_line.py`.

###### `ElasIsotEner`

élasticité linéaire isotrope totale (i.e. hyperélasticité)

ASTER behaviour name: `ELAS_ISOT_ENER` (`num_lc = 101`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/elas_isot_ener.py`.

###### `ElasIsotIncr`

élasticité linéaire isotrope incrémentale

ASTER behaviour name: `ELAS_ISOT_INCR` (`num_lc = 102`,
1 state variable(s)).
Upstream declaration: `code_aster/Behaviours/elas_isot_incr.py`.

###### `BetonDoubleDp`

Relation de comportement tridimensionnelle utilisée pour la description
du comportement non linéaire du béton. Il comporte un critere de
Drucker-Prager en traction et un critère de Drucker-Prager en
compression, découplés. Les deux critères peuvent avoir un écrouissage
adoucissant.

ASTER behaviour name: `BETON_DOUBLE_DP` (`num_lc = 120`,
4 state variable(s)).
Upstream declaration: `code_aster/Behaviours/beton_double_dp.py`.

###### `Monocristal`

Ce modèle permet de décrire le comportement d'un monocristal dont les
relations de comportement sont fournies via le concept compor, issu de
DEFI_COMPOR. Le nombre de variables internes est fonction des choix
effectués dans DEFI_COMPOR ; pour plus de précisions consulter
[R5.03.11].

ASTER behaviour name: `MONOCRISTAL` (`num_lc = 137`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/monocristal.py`.

###### `Polycristal`

Comportement poly-cristallin homogénéisé, défini par DEFI_COMPOR

ASTER behaviour name: `POLYCRISTAL` (`num_lc = 137`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/polycristal.py`.

###### `BetonRag`

Loi RAG pour le beton

ASTER behaviour name: `BETON_RAG` (`num_lc = 145`,
35 state variable(s)).
Upstream declaration: `code_aster/Behaviours/beton_rag.py`.

###### `CableGaineFrot`

Relation de comportement cohésive d'adherence Cable/Gaine

ASTER behaviour name: `CABLE_GAINE_FROT` (`num_lc = 152`,
2 state variable(s)).
Upstream declaration: `code_aster/Behaviours/cable_gaine_frot.py`.

###### `FluaPoroBeton`

Loi Fluage pour le beton

ASTER behaviour name: `FLUA_PORO_BETON` (`num_lc = 165`,
114 state variable(s)).
Upstream declaration: `code_aster/Behaviours/flua_poro_beton.py`.

###### `EndoPoroBeton`

Loi d'endommagement pour le beton

ASTER behaviour name: `ENDO_PORO_BETON` (`num_lc = 166`,
114 state variable(s)).
Upstream declaration: `code_aster/Behaviours/endo_poro_beton.py`.

###### `FluaEndoPoro`

lois de fluage et d'endommagement couplées pour le béton

ASTER behaviour name: `FLUA_ENDO_PORO` (`num_lc = 167`,
114 state variable(s)).
Upstream declaration: `code_aster/Behaviours/flua_endo_poro.py`.

###### `RgiBeton`

lois de Réaction de Gonflement interne (RGI), de fluage et
d'endommagement couplées pour le béton

ASTER behaviour name: `RGI_BETON` (`num_lc = 168`,
114 state variable(s)).
Upstream declaration: `code_aster/Behaviours/rgi_beton.py`.

###### `RgiBetonBa`

lois de Réaction de Gonflement interne (RGI), de fluage et
d'endommagement couplées pour le béton avec armatures réparties

ASTER behaviour name: `RGI_BETON_BA` (`num_lc = 169`,
158 state variable(s)).
Upstream declaration: `code_aster/Behaviours/rgi_beton_ba.py`.

###### `SimoMiehe`

Algo pour résolution en grandes déformations.

ASTER behaviour name: `SIMO_MIEHE` (`num_lc = 1000`,
6 state variable(s)).
Upstream declaration: `code_aster/Behaviours/simo_miehe.py`.

###### `KitDdi`

Double Deformation Incrementale

ASTER behaviour name: `KIT_DDI` (`num_lc = 8000`,
0 state variable(s)).
Upstream declaration: `code_aster/Behaviours/kit_ddi.py`.

##### Implementations

###### Methods

- ```rust
  pub const fn aster_name(self: Self) -> &'static str { /* ... */ }
  ```
  The ASTER behaviour name, verbatim (e.g. `"VISC_CIN2_CHAB"`).

- ```rust
  pub const fn num_lc(self: Self) -> u32 { /* ... */ }
  ```
  Upstream's `num_lc` dispatch number.

- ```rust
  pub const fn n_state_variables(self: Self) -> usize { /* ... */ }
  ```
  Number of internal state variables the law carries per integration point.

- ```rust
  pub const fn state_variable_names(self: Self) -> &'static [&'static str] { /* ... */ }
  ```
  Names of the internal state variables, in upstream's order.

- ```rust
  pub const fn lc_types(self: Self) -> &'static [&'static str] { /* ... */ }
  ```
  `lc_type` classification (`MECANIQUE`, `KIT_THM`, ...).

- ```rust
  pub const fn deformations(self: Self) -> &'static [&'static str] { /* ... */ }
  ```
  Strain measures the law supports (`PETIT`, `PETIT_REAC`, `GDEF_LOG`, ...).

- ```rust
  pub const fn integration_algorithms(self: Self) -> &'static [&'static str] { /* ... */ }
  ```
  Integration algorithms upstream offers for this law.

- ```rust
  pub const fn modelisations(self: Self) -> &'static [&'static str] { /* ... */ }
  ```
  Modelisations the law supports (`3D`, `AXIS`, `D_PLAN`, ...).

- ```rust
  pub const fn material_keywords(self: Self) -> &'static [&'static str] { /* ... */ }
  ```
  Material-property keywords the law reads (`ELAS`, `LEMAITRE`, ...).

- ```rust
  pub const fn is_mfront(self: Self) -> bool { /* ... */ }
  ```
  True if upstream declares this law through MFront's DSL rather

- ```rust
  pub fn has_declared_state_variables(self: Self) -> bool { /* ... */ }
  ```
  True if the catalogue states a state-variable count for this law.

- ```rust
  pub fn is_mechanical(self: Self) -> bool { /* ... */ }
  ```
  True if this law is a mechanical (`MECANIQUE`) constitutive law.

- ```rust
  pub fn from_aster_name(name: &str) -> Option<Self> { /* ... */ }
  ```
  Look a law up by its ASTER name.

- ```rust
  pub fn from_num_lc(num_lc: u32) -> Option<Self> { /* ... */ }
  ```
  Look a law up by its upstream `num_lc` dispatch number.

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
    fn clone(self: &Self) -> AsterBehaviour { /* ... */ }
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

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AsterBehaviour) -> bool { /* ... */ }
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
### Constants and Statics

#### Constant `ALL`

Every catalogue entry, ordered by `num_lc` then ASTER name.

```rust
pub const ALL: &[AsterBehaviour] = _;
```

## Module `chaboche`

Chaboche kinematic-hardening elasto-(visco)plastic laws.

# What kinematic hardening is, and why it needs a new state variable

Every law in [`crate::rheology::aster::viscoplastic`] is *isotropic*: the
yield or flow condition depends on the stress only through the von Mises
equivalent of its deviator `s`, so the yield surface is a sphere in
deviatoric space centred on the origin, which can only grow or shrink.

That cannot reproduce what metal actually does under reversed loading. Pull
a steel bar into plasticity, then push it back: it yields in compression
well before the tensile yield stress in magnitude. This is the **Bauschinger
effect**, and an isotropic model gets it exactly wrong — it predicts the
reverse yield stress to be *larger*, not smaller.

Chaboche's answer is to let the yield surface **translate** as well as
resize. Its centre is a deviatoric tensor `X`, the **back stress**, and the
flow condition is written on `s - X` rather than on `s`:

`f = ||s - X||_vm - R(p)`, with `||·||_vm = sqrt(3/2 · (·):(·))`

`X` is a genuine tensorial internal variable with its own evolution law, and
that is the architectural break with the isotropic family: the local
integration is no longer a scalar problem with a fixed flow direction.

# The Armstrong-Frederick evolution law

Each back stress is stored as a dimensionless **back strain** `α`, from
which the stress-dimensioned back stress is recovered as `X = (2/3) C α`.
This is upstream's storage convention (`nmchab.F90` reconstructs
`X = C·α/1.5` when it checks radiality), and it is kept here so a
code_aster state vector can be read across without rescaling.

`α` follows Armstrong-Frederick:

`α̇ = ε̇_p - γ(p) · δ · α · ṗ`

The first term is *linear* (Prager) hardening — the surface centre simply
follows the plastic strain. The second is **dynamic recovery**: a pull-back
toward the origin proportional to `α` itself and to the rate of plastic
flow. Their competition makes `α` saturate rather than grow without bound,
and saturation is what produces a closed, stable hysteresis loop instead of
ever-increasing stress amplitude.

Under monotonic proportional loading the saturated value is exactly

`||X||_vm → C / γ`

which is the classical Armstrong-Frederick result and the sharpest
analytical reference available for verifying this port. It is pinned by
[`the_back_stress_saturates_at_c_over_gamma`](self) in the test module.

Two back stresses (`VISC_CIN2_CHAB` and friends) are used because one
Armstrong-Frederick tensor gives a single exponential approach to
saturation, which fits either the sharp knee just after yield or the long
tail, but not both. A fast-saturating `α₁` plus a slow-saturating `α₂`
reproduces both, and the saturated equivalent stress is simply
`R_∞ + C₁/γ₁ + C₂/γ₂`.

# The coupled local solve, and why it still collapses to one scalar

The unknowns of the local problem are the scalar `Δp` **and** the tensors
`Δα₁`, `Δα₂` — nominally 13 coupled unknowns in 3-D. Solving that as a
13-dimensional Newton system is what a naive port would do. code_aster does
not, and the reason is worth stating because it is the whole architecture of
this module.

Integrate Armstrong-Frederick with a backward-Euler step:

`α = α_m + Δε_p - γ δ Δp α`  →  `α = (α_m + Δε_p) / (1 + γ δ Δp)`

The update is **affine in `Δε_p`**, so

`X = (2/3) M (α_m + Δε_p)`, with `M(Δp) = C / (1 + γ δ Δp)`

Substituting into the elastic-predictor relation `s = s_trial - 2μ Δε_p`:

`s - X = [s_trial - (2/3) M α_m] - (2μ + (2/3) M) Δε_p`

Write `ŝ = s_trial - (2/3) M₁ α_m1 - (2/3) M₂ α_m2` for the bracketed term.
Normality says `Δε_p` is parallel to `s - X`, so the equation above is a
statement that `s - X` is a positive multiple of `ŝ`: **the flow direction
is that of `ŝ`, and the tensorial problem is radial after all.** Taking von
Mises norms of both sides collapses it to one scalar equation:

`||ŝ||_vm = R(p_m + Δp) + (3μ + M₁ n₁ + M₂ n₂) Δp  [+ K (Δp/Δt)^(1/n)]`

which is exactly upstream's `nmchcr` residual. One unknown, one equation —
but note that `ŝ` itself depends on `Δp` through `M(Δp)`, so unlike the
isotropic radial return the flow *direction rotates during the solve*. That
is the substantive difference, and it is why the residual here must
recompute `ŝ` and its norm at every trial `Δp` rather than fixing a
direction once from the elastic predictor.

The collapse is exact only when `δ = 1` (`n₁ = n₂ = 1`). For the non-radial
variants (`δ < 1`, upstream's `CIN2_NRAD` material keyword) upstream keeps
the same scalar equation and folds the non-radiality into the correction
factors `n₁`, `n₂` evaluated on the current direction; that approximation is
reproduced here rather than improved on.

# What is covered

| ASTER name | Back stresses | Rate-dependent | Strain memory | State vars |
|---|---|---|---|---|
| `VMIS_CIN1_CHAB` | 1 | no | no | 8 |
| `VMIS_CIN2_CHAB` | 2 | no | no | 14 |
| `VISC_CIN1_CHAB` | 1 | yes | no | 8 |
| `VISC_CIN2_CHAB` | 2 | yes | no | 14 |
| `VMIS_CIN2_MEMO` | 2 | no | yes | 28 |
| `VISC_CIN2_MEMO` | 2 | yes | yes | 28 |

All six share `num_lc = 4` and dispatch through the same upstream routine,
which is why they are one enum here rather than six.

**`VISCOCHAB` and `VISC_TAHERI` are not in this module** — see the next
section for why.

# Why `VISCOCHAB` and `VISC_TAHERI` are absent

Both were in the original scope for this module and both were left out
deliberately, for different reasons.

- **`VISCOCHAB`** (`num_lc = 32`, 28 state variables, upstream
  `bibfor/algorith/rkdcha.F90` and the `cvm*` family) *is* a Chaboche law
  with two back stresses, but it is not formulated as a yield surface with a
  consistency condition. It is a pure **rate system** — 27 coupled ODEs in
  the viscoplastic strain, two back strains, a memory-surface centre, an
  isotropic radius, a memory radius and the cumulated strain — with static
  thermal recovery terms on the back stresses. Upstream declares
  `algo_inte = ("NEWTON", "NEWTON_RELI", "RUNGE_KUTTA")` for it and
  integrates it as an ODE system. It therefore does *not* share this
  module's scalar-collapse architecture; forcing it in would have meant
  either a second, unrelated architecture in the same file or a distorted
  port. Its rate function is straightforward to transcribe and is the
  natural next tranche.
- **`VISC_TAHERI`** (`num_lc = 18`, upstream `bibfor/comport/nmtahe.F90`
  plus nine `nmta*` helpers) turned out not to be a kinematic-hardening law
  at all. It is Taheri's two-surface ratcheting model, whose unknowns are
  two *scalars* (`dp` and the surface radius `sp`, or `xi` and `sp`) solved
  by a 2x2 Newton with an explicit line search. There is no back-stress
  tensor and nothing of this module's architecture applies to it.

# Convention

Raw `f64` and [`SymmTensor`] throughout, with units stated in prose — the
same convention as [`crate::rheology::aster::viscoplastic`]. Upstream stores
its tensors as Mandel six-vectors (`XX, YY, ZZ, √2·XY, √2·XZ, √2·YZ`, see
[`crate::rheology::aster::kinematics::AsterVoigt`]); this port works with
[`SymmTensor`] directly, so the `√2` scaling `nmchab.F90` applies when it
reads and writes `vim`/`vip` has no counterpart here and no state variable
needs rescaling.

# Status

AI-assisted port, not yet reviewed by a human and not validated against
code_aster output or against experiment. The tests below are *verification*
— against closed-form limits of the model and against internal consistency
conditions — not validation. See `RESPONSIBLE_USE.md`.

```rust
pub mod chaboche { /* ... */ }
```

### Types

#### Struct `ElasticModuli`

Isotropic elastic moduli at one instant.

# Units and valid range

`young` is Young's modulus `E` \[Pa\], strictly positive. `poisson` is
Poisson's ratio `ν` \[-\], which must lie strictly inside `(-1, 1/2)` for
the bulk and shear moduli to be positive; `ν = 1/2` is incompressible and
makes `3K` infinite, so it is rejected rather than returned as an infinity.

# Why two instants are needed

code_aster evaluates the moduli at both ends of the timestep because `E` and
`ν` are temperature-dependent and the temperature changes across a step.
`nmchab.F90` rescales the incoming stress by the ratio of the two so that
the elastic strain implied by it is preserved when the modulus changes. See
[`ThermoElasticStep`].

```rust
pub struct ElasticModuli {
    pub young: f64,
    pub poisson: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `young` | `f64` | Young's modulus `E` \[Pa\], strictly positive. |
| `poisson` | `f64` | Poisson's ratio `ν` \[-\], in `(-1, 1/2)`. |

##### Implementations

###### Methods

- ```rust
  pub fn new(young: f64, poisson: f64) -> Result<Self> { /* ... */ }
  ```
  Build and validate a pair of isotropic moduli.

- ```rust
  pub fn twice_shear_modulus(self: Self) -> f64 { /* ... */ }
  ```
  Twice the shear modulus, `2μ = E/(1+ν)` \[Pa\].

- ```rust
  pub fn three_times_bulk_modulus(self: Self) -> f64 { /* ... */ }
  ```
  Three times the bulk modulus, `3K = E/(1-2ν)` \[Pa\].

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
    fn clone(self: &Self) -> ElasticModuli { /* ... */ }
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
    fn eq(self: &Self, other: &ElasticModuli) -> bool { /* ... */ }
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
#### Struct `ThermoElasticStep`

The thermo-elastic description of one timestep.

# What it carries

The elastic moduli at the start and end of the step (they differ when the
temperature changed), the isotropic thermal strain *increment* accumulated
over the step, and the step duration.

# Units

`thermal_strain_increment` is dimensionless \[-\] and is subtracted from all
three normal components of the total strain increment, exactly as upstream's
`depsth = deps - coef·kron`. `dt` is in seconds \[s\] and must be
non-negative; it is used only by the rate-dependent variants, where the
viscous overstress is `K (Δp/Δt)^(1/n)`.

```rust
pub struct ThermoElasticStep {
    pub start: ElasticModuli,
    pub end: ElasticModuli,
    pub thermal_strain_increment: f64,
    pub dt: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `start` | `ElasticModuli` | Moduli at the start of the step, upstream's `matel(1:2)`. |
| `end` | `ElasticModuli` | Moduli at the end of the step, upstream's `matel(3:4)`. |
| `thermal_strain_increment` | `f64` | Isotropic thermal strain increment over the step \[-\], upstream's<br>`coef` from `verift`. |
| `dt` | `f64` | Step duration `Δt` \[s\], non-negative. |

##### Implementations

###### Methods

- ```rust
  pub fn isothermal(moduli: ElasticModuli, dt: f64) -> Self { /* ... */ }
  ```
  An isothermal step: the same moduli at both ends and no thermal strain.

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
    fn clone(self: &Self) -> ThermoElasticStep { /* ... */ }
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
    fn eq(self: &Self, other: &ThermoElasticStep) -> bool { /* ... */ }
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
#### Struct `BackStress`

The kinematic-hardening state: one or two dimensionless back strains.

# What it holds

`alpha1` and `alpha2` are the **back strains** `α₁`, `α₂` \[-\] —
dimensionless deviatoric tensors, upstream's `ALPHA*` and `ALPHA2*` state
variables. The stress-dimensioned back stress that shifts the yield surface
is recovered with [`BackStress::stress`] as `X = (2/3)(C₁ α₁ + C₂ α₂)`
\[Pa\].

`alpha2` is present but held at zero for the one-tensor laws
(`VMIS_CIN1_CHAB`, `VISC_CIN1_CHAB`); the law variant, not this struct,
decides how many are live.

# Assumptions

Both tensors should be deviatoric (`tr(α) = 0`). Nothing enforces it,
because the plastic strain increment that drives them is deviatoric by
construction, so a zero-initialised state stays deviatoric forever. Seeding
a non-deviatoric `α` is a caller error that will show up as a spurious
hydrostatic term in `X`.

```rust
pub struct BackStress {
    pub alpha1: outram_foam_basic_lib::primitives::SymmTensor,
    pub alpha2: outram_foam_basic_lib::primitives::SymmTensor,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `alpha1` | `outram_foam_basic_lib::primitives::SymmTensor` | First back strain `α₁` \[-\], deviatoric. |
| `alpha2` | `outram_foam_basic_lib::primitives::SymmTensor` | Second back strain `α₂` \[-\], deviatoric. Zero for the one-tensor laws. |

##### Implementations

###### Methods

- ```rust
  pub fn zero() -> Self { /* ... */ }
  ```
  The virgin state: both back strains zero, i.e. a yield surface centred

- ```rust
  pub fn stress(self: Self, c1: f64, c2: f64) -> SymmTensor { /* ... */ }
  ```
  The back stress `X = (2/3)(C₁ α₁ + C₂ α₂)` \[Pa\] — the centre of the

- ```rust
  pub fn equivalent_stress(self: Self, c1: f64, c2: f64) -> f64 { /* ... */ }
  ```
  The equivalent back stress `||X||_vm = sqrt(3/2 X:X)` \[Pa\].

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
    fn clone(self: &Self) -> BackStress { /* ... */ }
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
    fn default() -> BackStress { /* ... */ }
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
    fn eq(self: &Self, other: &BackStress) -> bool { /* ... */ }
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
#### Struct `StrainMemory`

The strain-memory-surface state of the `*_MEMO` variants.

# What the memory surface is for

Plain Chaboche saturates to a cyclic response that depends only on the
current strain, not on the strain amplitudes the material has already seen.
Real metals remember: a specimen cycled at large amplitude and then at small
amplitude hardens far more than one taken straight to the small amplitude.

Chaboche's memory surface models that with a surface in *plastic strain*
space, of radius `q` and centre `ξ`, that is dragged outward whenever the
plastic strain leaves it. The isotropic hardening then saturates toward a
level `Q(q)` set by how far the surface has been pushed, so the largest
amplitude ever reached is remembered.

# Units

`isotropic_increment` is in pascal \[Pa\] and is the *increment over `R₀`*,
matching upstream's state variable 15: the yield radius is
`R = R₀ + isotropic_increment`, **not** the `R_∞ + (R₀-R_∞)e^{-bp}`
expression the non-memory variants use. `memory_radius` and
`memory_centre` are dimensionless plastic strains \[-\], as is
`plastic_strain`.

All four fields are ignored by the variants without a memory surface.

```rust
pub struct StrainMemory {
    pub isotropic_increment: f64,
    pub memory_radius: f64,
    pub memory_centre: outram_foam_basic_lib::primitives::SymmTensor,
    pub plastic_strain: outram_foam_basic_lib::primitives::SymmTensor,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `isotropic_increment` | `f64` | Isotropic hardening accumulated on top of `R₀` \[Pa\]. Upstream `vim(15)`. |
| `memory_radius` | `f64` | Memory-surface radius `q` \[-\]. Upstream `vim(16)`. |
| `memory_centre` | `outram_foam_basic_lib::primitives::SymmTensor` | Memory-surface centre `ξ` in plastic-strain space \[-\]. Upstream<br>`vim(17:22)`. |
| `plastic_strain` | `outram_foam_basic_lib::primitives::SymmTensor` | Accumulated plastic strain tensor `ε_p` \[-\]. Upstream `vim(23:28)`.<br><br>Tracked only by the memory variants, because only they need the plastic<br>strain *tensor* as opposed to its accumulated equivalent `p`. |

##### Implementations

###### Methods

- ```rust
  pub fn zero() -> Self { /* ... */ }
  ```
  The virgin memory state: no accumulated hardening, a point-sized memory

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
    fn clone(self: &Self) -> StrainMemory { /* ... */ }
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
    fn default() -> StrainMemory { /* ... */ }
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
    fn eq(self: &Self, other: &StrainMemory) -> bool { /* ... */ }
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
#### Struct `ChabocheState`

The complete internal state of a Chaboche law at one integration point.

# Correspondence with upstream's `vim`/`vip`

| Field | Upstream slot | Name |
|---|---|---|
| [`accumulated_plastic_strain`](Self::accumulated_plastic_strain) | `vim(1)` | `EPSPEQ` |
| [`local_iterations`](Self::local_iterations) | `vim(2)` | `INDIPLAS` |
| [`back_stress`](Self::back_stress)`.alpha1` | `vim(3:8)` | `ALPHA*` |
| [`back_stress`](Self::back_stress)`.alpha2` | `vim(9:14)` | `ALPHA2*` |
| [`memory`](Self::memory) | `vim(15:28)` | memory surface |

# Units

`accumulated_plastic_strain` is dimensionless \[-\] and non-decreasing.

```rust
pub struct ChabocheState {
    pub accumulated_plastic_strain: f64,
    pub back_stress: BackStress,
    pub memory: StrainMemory,
    pub local_iterations: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `accumulated_plastic_strain` | `f64` | Accumulated equivalent plastic strain `p` \[-\], non-negative and<br>non-decreasing. |
| `back_stress` | `BackStress` | Back strains — see [`BackStress`]. |
| `memory` | `StrainMemory` | Strain-memory state — see [`StrainMemory`]. Untouched by the variants<br>without a memory surface. |
| `local_iterations` | `usize` | Local iterations used by the last step, and *de facto* the plasticity<br>indicator: zero exactly when the step was elastic.<br><br># An upstream oddity, reproduced deliberately<br><br>code_aster's catalogue names state variable 2 `INDIPLAS`, a plasticity<br>indicator, and `nmchab.F90` reads it back as `plast` to select the<br>tangent branch. Yet on output it writes `vip(2) = niter`, the local<br>iteration count. The two happen to agree in effect — `niter` is zero<br>exactly on an elastic step and at least one otherwise — so the overload<br>is harmless, but the stored number is an iteration count and not a 0/1<br>flag. This port stores the iteration count and names the field for what<br>it actually is. See [`ChabocheIncrement::yielded`] for the honest flag. |

##### Implementations

###### Methods

- ```rust
  pub fn zero() -> Self { /* ... */ }
  ```
  The virgin state: no plastic strain, no back stress, no memory.

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
    fn clone(self: &Self) -> ChabocheState { /* ... */ }
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
    fn default() -> ChabocheState { /* ... */ }
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
    fn eq(self: &Self, other: &ChabocheState) -> bool { /* ... */ }
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
#### Struct `ChabocheParameters`

The material parameters of the Chaboche family.

One struct serves all six laws, mirroring upstream's single `mat(1:18)`
array assembled by `nmcham.F90`. Which fields are *read* is decided by the
[`ChabocheLaw`] variant, not by this struct: a one-tensor law ignores
`c2_asymptotic`/`gamma2_initial`, a rate-independent law ignores
`viscous_stress`/`viscous_exponent`, and a law without a memory surface
ignores the four `memory_*` fields. Populating an ignored field is harmless
but has no effect — the same contract upstream has.

# The two hardening mechanisms, and which parameters drive which

**Isotropic** (the surface grows): `r0`, `r_asymptotic`, `b` give
`R(p) = R_∞ + (R₀ - R_∞) e^{-b p}` \[Pa\]. With `R_∞ > R₀` the material
hardens cyclically; with `R_∞ < R₀` it softens, which is what tempered
martensitic steels actually do.

**Kinematic** (the surface translates): `c1_asymptotic`, `gamma1_initial`
and their `2` counterparts, modulated by `k`, `w` and `a_asymptotic`:

- `C_i(p) = C_i∞ · (1 + (k-1) e^{-w p})` \[Pa\]
- `γ_i(p) = γ_i0 · (a_∞ + (1-a_∞) e^{-b p})` \[-\]

Set `k = 1`, `w = 0`, `a_asymptotic = 1` for constant `C` and `γ`, which is
the textbook Armstrong-Frederick model and the configuration the analytical
saturation result `||X||_vm = C/γ` applies to.

# Units, verbatim upstream keyword names

| Field | Upstream keyword | Unit |
|---|---|---|
| [`r0`](Self::r0) | `R_0` | Pa |
| [`r_asymptotic`](Self::r_asymptotic) | `R_I` | Pa |
| [`b`](Self::b) | `B` | - |
| [`c1_asymptotic`](Self::c1_asymptotic) | `C_I` / `C1_I` | Pa |
| [`gamma1_initial`](Self::gamma1_initial) | `G_0` / `G1_0` | - |
| [`c2_asymptotic`](Self::c2_asymptotic) | `C2_I` | Pa |
| [`gamma2_initial`](Self::gamma2_initial) | `G2_0` | - |
| [`k`](Self::k) | `K` | - |
| [`w`](Self::w) | `W` | - |
| [`a_asymptotic`](Self::a_asymptotic) | `A_I` | - |
| [`delta1`](Self::delta1) / [`delta2`](Self::delta2) | `DELTA1` / `DELTA2` | - |
| [`viscous_exponent`](Self::viscous_exponent) | `N` (`LEMAITRE`) | - |
| [`viscous_stress`](Self::viscous_stress) | `1 / UN_SUR_K` (`LEMAITRE`) | Pa |
| [`memory_eta`](Self::memory_eta) | `ETA` (`MEMO_ECRO`) | - |
| [`memory_q0`](Self::memory_q0) | `Q_0` (`MEMO_ECRO`) | Pa |
| [`memory_qm`](Self::memory_qm) | `Q_M` (`MEMO_ECRO`) | Pa |
| [`memory_mu`](Self::memory_mu) | `MU` (`MEMO_ECRO`) | - |

Note that upstream's `LEMAITRE` keyword supplies `UN_SUR_K = 1/K` and this
struct stores `K` itself, the same inversion
[`crate::rheology::aster::viscoplastic::NortonParameters`] makes and for the
same reason: `K` is what the literature tabulates and the one with an
interpretable unit.

```rust
pub struct ChabocheParameters {
    pub r0: f64,
    pub r_asymptotic: f64,
    pub b: f64,
    pub c1_asymptotic: f64,
    pub gamma1_initial: f64,
    pub c2_asymptotic: f64,
    pub gamma2_initial: f64,
    pub k: f64,
    pub w: f64,
    pub a_asymptotic: f64,
    pub delta1: f64,
    pub delta2: f64,
    pub viscous_exponent: f64,
    pub viscous_stress: f64,
    pub memory_eta: f64,
    pub memory_q0: f64,
    pub memory_qm: f64,
    pub memory_mu: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `r0` | `f64` | Initial yield radius `R₀` \[Pa\], upstream `R_0`. Strictly positive. |
| `r_asymptotic` | `f64` | Asymptotic yield radius `R_∞` \[Pa\], upstream `R_I`. May be below `R₀`<br>(cyclic softening). |
| `b` | `f64` | Isotropic-saturation rate `b` \[-\], upstream `B`. Non-negative;<br>upstream warns (`COMPOR1_84`) on a negative value. |
| `c1_asymptotic` | `f64` | Asymptotic first kinematic modulus `C₁∞` \[Pa\], upstream `C_I`/`C1_I`. |
| `gamma1_initial` | `f64` | Initial first dynamic-recovery coefficient `γ₁₀` \[-\], upstream<br>`G_0`/`G1_0`. Zero gives linear (Prager) kinematic hardening. |
| `c2_asymptotic` | `f64` | Asymptotic second kinematic modulus `C₂∞` \[Pa\], upstream `C2_I`.<br>Ignored by the one-tensor laws. |
| `gamma2_initial` | `f64` | Initial second dynamic-recovery coefficient `γ₂₀` \[-\], upstream<br>`G2_0`. Ignored by the one-tensor laws. |
| `k` | `f64` | Kinematic-modulus ratio `k` \[-\], upstream `K`. `C(0) = k C_∞`, so<br>`k = 1` makes `C` constant. |
| `w` | `f64` | Kinematic-modulus saturation rate `w` \[-\], upstream `W`. Non-negative;<br>upstream warns (`COMPOR1_84`) on a negative value. |
| `a_asymptotic` | `f64` | Asymptotic recovery ratio `a_∞` \[-\], upstream `A_I`. `γ(∞) = a_∞ γ₀`,<br>so `a_∞ = 1` makes `γ` constant. |
| `delta1` | `f64` | First non-radiality coefficient `δ₁` \[-\], upstream `DELTA1`<br>(`CIN2_NRAD`). Must lie in `[0, 1]`; `1` is the ordinary radial model. |
| `delta2` | `f64` | Second non-radiality coefficient `δ₂` \[-\], upstream `DELTA2`. |
| `viscous_exponent` | `f64` | Viscous (Norton) exponent `n` \[-\], upstream `N` under `LEMAITRE`.<br>Strictly positive; read only by the `VISC_*` variants. |
| `viscous_stress` | `f64` | Viscous drag stress `K` \[Pa\], the reciprocal of upstream's<br>`UN_SUR_K`. Strictly positive; read only by the `VISC_*` variants.<br><br>The overstress it produces is `K (Δp/Δt)^(1/n)`, so a large `K` means a<br>strongly rate-sensitive material and `K → 0` recovers the<br>rate-independent law. |
| `memory_eta` | `f64` | Memory-surface progression coefficient `η` \[-\], upstream `ETA`. In<br>`[0, 1]`; read only by the `*_MEMO` variants. |
| `memory_q0` | `f64` | Memory-surface initial saturation level `Q₀` \[Pa\], upstream `Q_0`. |
| `memory_qm` | `f64` | Memory-surface maximum saturation level `Q_M` \[Pa\], upstream `Q_M`. |
| `memory_mu` | `f64` | Memory-surface saturation rate `μ` \[-\], upstream `MU`. |

##### Implementations

###### Methods

- ```rust
  pub fn armstrong_frederick(r0: f64, c1: f64, gamma1: f64) -> Self { /* ... */ }
  ```
  A plain Armstrong-Frederick parameter set: constant `C` and `γ`, no

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
    fn clone(self: &Self) -> ChabocheParameters { /* ... */ }
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
    fn eq(self: &Self, other: &ChabocheParameters) -> bool { /* ... */ }
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
#### Enum `ChabocheLaw`

A Chaboche kinematic-hardening law.

Enum dispatch rather than trait objects, per the workspace rule — the six
variants are exactly the six code_aster behaviours that dispatch through
`lc0004.F90`, so the set is closed and known at compile time.

The variant selects three independent switches, matching upstream's
`nmcham.F90` decoding of the behaviour name:

- **one or two back stresses** (`CIN1` vs `CIN2`/`MEMO`), upstream `nbvar`;
- **rate-dependent or not** (`VISC_` vs `VMIS_`), upstream `visc`;
- **strain memory or not** (`_MEMO`), upstream `memo`.

```rust
pub enum ChabocheLaw {
    VmisCin1Chab(ChabocheParameters),
    VmisCin2Chab(ChabocheParameters),
    ViscCin1Chab(ChabocheParameters),
    ViscCin2Chab(ChabocheParameters),
    VmisCin2Memo(ChabocheParameters),
    ViscCin2Memo(ChabocheParameters),
}
```

##### Variants

###### `VmisCin1Chab`

Rate-independent Chaboche with one back stress.

ASTER behaviour name: `VMIS_CIN1_CHAB` (`num_lc = 4`, 8 state
variables). Upstream: `bibfor/comport/nmchab.F90` via
`bibfor/lc/lc0004.F90` — legacy symbols `nmchab`, `lc0004`.
Integration: `SECANTE` or `BRENT`; this port uses
[`brent`].

Yield condition `||s - X||_vm = R(p)` with `X = (2/3) C₁ α₁`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `ChabocheParameters` |  |

###### `VmisCin2Chab`

Rate-independent Chaboche with two back stresses.

ASTER behaviour name: `VMIS_CIN2_CHAB` (`num_lc = 4`, 14 state
variables). Upstream as [`Self::VmisCin1Chab`].

The second Armstrong-Frederick tensor lets one saturate fast (the knee
just past yield) and the other slowly (the long tail), which one tensor
cannot do.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `ChabocheParameters` |  |

###### `ViscCin1Chab`

Rate-dependent Chaboche with one back stress.

ASTER behaviour name: `VISC_CIN1_CHAB` (`num_lc = 4`, 8 state
variables). Upstream as [`Self::VmisCin1Chab`]; the viscous branch is
`nmchcr.F90`'s `rppmdp = rppmdp + kvi·(dp/dt)^(1/n)`.

The yield condition is replaced by a Norton overstress relation:
`||s - X||_vm = R(p) + K (ṗ)^(1/n)`. The stress may now exceed the
yield radius, by an amount set by how fast the material is being
strained.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `ChabocheParameters` |  |

###### `ViscCin2Chab`

Rate-dependent Chaboche with two back stresses.

ASTER behaviour name: `VISC_CIN2_CHAB` (`num_lc = 4`, 14 state
variables). The combination most often used for austenitic stainless
steel at reactor temperatures.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `ChabocheParameters` |  |

###### `VmisCin2Memo`

Rate-independent Chaboche with two back stresses and a strain-memory
surface.

ASTER behaviour name: `VMIS_CIN2_MEMO` (`num_lc = 4`, 28 state
variables). See [`StrainMemory`] for what the memory surface does.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `ChabocheParameters` |  |

###### `ViscCin2Memo`

Rate-dependent Chaboche with two back stresses and a strain-memory
surface.

ASTER behaviour name: `VISC_CIN2_MEMO` (`num_lc = 4`, 28 state
variables). The fullest member of the family.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `ChabocheParameters` |  |

##### Implementations

###### Methods

- ```rust
  pub const fn parameters(self: Self) -> ChabocheParameters { /* ... */ }
  ```
  The material parameters this law was built with.

- ```rust
  pub const fn behaviour(self: Self) -> AsterBehaviour { /* ... */ }
  ```
  The corresponding entry in the generated behaviour catalogue.

- ```rust
  pub const fn aster_name(self: Self) -> &'static str { /* ... */ }
  ```
  The upstream ASTER behaviour name, verbatim.

- ```rust
  pub const fn back_stress_count(self: Self) -> usize { /* ... */ }
  ```
  How many Armstrong-Frederick back stresses are active — 1 or 2.

- ```rust
  pub const fn is_rate_dependent(self: Self) -> bool { /* ... */ }
  ```
  Whether the law carries a viscous overstress — upstream's `visc`.

- ```rust
  pub const fn has_strain_memory(self: Self) -> bool { /* ... */ }
  ```
  Whether the law carries a strain-memory surface — upstream's `memo`.

- ```rust
  pub fn kinematic_moduli(self: Self, p: f64) -> (f64, f64) { /* ... */ }
  ```
  The kinematic moduli `(C₁, C₂)` \[Pa\] at accumulated plastic strain

- ```rust
  pub fn recovery_coefficients(self: Self, p: f64) -> (f64, f64) { /* ... */ }
  ```
  The dynamic-recovery coefficients `(γ₁, γ₂)` \[-\] at accumulated

- ```rust
  pub fn isotropic_radius(self: Self, p: f64) -> f64 { /* ... */ }
  ```
  The isotropic yield radius `R(p)` \[Pa\] of the **non-memory** variants.

- ```rust
  pub fn start_radius(self: Self, state: ChabocheState) -> f64 { /* ... */ }
  ```
  The yield radius at the start of a step, for either kind of variant.

- ```rust
  pub fn elastic_predictor(self: Self, state: ChabocheState, previous_stress: SymmTensor, strain_increment: SymmTensor, step: ThermoElasticStep) -> Result<ChabochePredictor> { /* ... */ }
  ```
  The elastic predictor for one step — everything the local solve needs

- ```rust
  pub fn local_state(self: Self, predictor: ChabochePredictor, dp: f64) -> ChabocheLocalState { /* ... */ }
  ```
  Evaluate the local state at a trial plastic-strain increment `Δp`.

- ```rust
  pub fn integrate(self: Self, state: ChabocheState, previous_stress: SymmTensor, strain_increment: SymmTensor, step: ThermoElasticStep, control: SolverControl) -> Result<ChabocheIncrement> { /* ... */ }
  ```
  Integrate one timestep, returning the end-of-step stress and state.

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
    fn clone(self: &Self) -> ChabocheLaw { /* ... */ }
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
    fn eq(self: &Self, other: &ChabocheLaw) -> bool { /* ... */ }
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
#### Struct `ChabochePredictor`

The elastic predictor of one Chaboche step.

Everything the local solve needs that does not depend on the unknown `Δp`.
Built by [`ChabocheLaw::elastic_predictor`] and consumed by
[`ChabocheLaw::local_state`].

```rust
pub struct ChabochePredictor {
    pub trial_deviator: outram_foam_basic_lib::primitives::SymmTensor,
    pub mean_stress: f64,
    pub yield_function: f64,
    pub twice_shear_modulus: f64,
    pub normalisation: f64,
    pub start_state: ChabocheState,
    pub dt: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `trial_deviator` | `outram_foam_basic_lib::primitives::SymmTensor` | The trial (elastic-predictor) stress deviator `s_trial` \[Pa\]. |
| `mean_stress` | `f64` | The end-of-step hydrostatic stress `tr(σ)/3` \[Pa\]. Plastic flow is<br>deviatoric, so this is already final. |
| `yield_function` | `f64` | The yield function at `Δp = 0`, `||s_trial - X_m||_vm - R(p_m)` \[Pa\].<br>Upstream's `seuil`. Non-positive means the step is elastic. |
| `twice_shear_modulus` | `f64` | `2μ` at the end of the step \[Pa\]. |
| `normalisation` | `f64` | The scale the residual is divided by \[Pa\] — upstream's `denom`. Only<br>sets the residual's magnitude; it cannot move the root. |
| `start_state` | `ChabocheState` | Internal state at the start of the step. |
| `dt` | `f64` | Step duration `Δt` \[s\]. |

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
    fn clone(self: &Self) -> ChabochePredictor { /* ... */ }
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
    fn eq(self: &Self, other: &ChabochePredictor) -> bool { /* ... */ }
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
#### Struct `ChabocheLocalState`

The local state at one trial value of `Δp`.

Returned by [`ChabocheLaw::local_state`]. Upstream leaves the same
quantities in a `COMMON` block; returning them makes the converged state a
matter of one more evaluation rather than of shared mutable state.

```rust
pub struct ChabocheLocalState {
    pub increment: f64,
    pub effective_deviator: outram_foam_basic_lib::primitives::SymmTensor,
    pub effective_equivalent: f64,
    pub flow_direction: outram_foam_basic_lib::primitives::SymmTensor,
    pub plastic_strain_increment: outram_foam_basic_lib::primitives::SymmTensor,
    pub kinematic_modulus: [f64; 2],
    pub non_radial_factor: [f64; 2],
    pub recovery_coefficient: [f64; 2],
    pub isotropic_radius: f64,
    pub memory: StrainMemory,
    pub residual: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `increment` | `f64` | The trial accumulated-plastic-strain increment `Δp` \[-\]. |
| `effective_deviator` | `outram_foam_basic_lib::primitives::SymmTensor` | The shifted trial deviator `ŝ = s_trial - (2/3)(M₁α₁ + M₂α₂)` \[Pa\].<br>Its direction is the flow direction. |
| `effective_equivalent` | `f64` | `||ŝ||_vm` \[Pa\] — upstream's `seq`. |
| `flow_direction` | `outram_foam_basic_lib::primitives::SymmTensor` | The flow direction, normalised so that `n:n = 1` \[-\]. Upstream's<br>`norm`. |
| `plastic_strain_increment` | `outram_foam_basic_lib::primitives::SymmTensor` | The plastic strain increment `Δε_p = sqrt(3/2)·Δp·n` \[-\], deviatoric,<br>whose equivalent measure `sqrt(2/3 Δε_p:Δε_p)` is exactly `Δp`. |
| `kinematic_modulus` | `[f64; 2]` | The implicit Armstrong-Frederick moduli `[M₁, M₂]` \[Pa\],<br>`M_i = C_i/(1 + γ_i δ_i Δp)`. Upstream's `mp`, `m2p`. |
| `non_radial_factor` | `[f64; 2]` | The non-radiality corrections `[n₁, n₂]` \[-\]; both are exactly one for<br>the ordinary radial model (`δ = 1`). |
| `recovery_coefficient` | `[f64; 2]` | The dynamic-recovery coefficients `[γ₁, γ₂]` \[-\] at `p_m + Δp`. |
| `isotropic_radius` | `f64` | The isotropic yield radius `R` \[Pa\] at `p_m + Δp`. |
| `memory` | `StrainMemory` | The memory-surface state at `p_m + Δp`; unchanged from the start for<br>variants without one. |
| `residual` | `f64` | The dimensionless residual — see [`ChabocheLaw::local_state`]. |

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
    fn clone(self: &Self) -> ChabocheLocalState { /* ... */ }
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
    fn eq(self: &Self, other: &ChabocheLocalState) -> bool { /* ... */ }
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
#### Struct `ChabocheIncrement`

The outcome of integrating one Chaboche step.

```rust
pub struct ChabocheIncrement {
    pub stress: outram_foam_basic_lib::primitives::SymmTensor,
    pub state: ChabocheState,
    pub plastic_strain_increment: outram_foam_basic_lib::primitives::SymmTensor,
    pub equivalent_increment: f64,
    pub iterations: usize,
    pub yielded: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `stress` | `outram_foam_basic_lib::primitives::SymmTensor` | End-of-step Cauchy stress \[Pa\]. |
| `state` | `ChabocheState` | End-of-step internal state. |
| `plastic_strain_increment` | `outram_foam_basic_lib::primitives::SymmTensor` | The plastic strain increment `Δε_p` \[-\], deviatoric. |
| `equivalent_increment` | `f64` | The accumulated-plastic-strain increment `Δp` \[-\], non-negative. |
| `iterations` | `usize` | Local-solver iterations used; zero on an elastic step. |
| `yielded` | `bool` | Whether the step yielded.<br><br>The honest flag upstream's `INDIPLAS` was meant to be — see<br>[`ChabocheState::local_iterations`]. |

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
    fn clone(self: &Self) -> ChabocheIncrement { /* ... */ }
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
    fn eq(self: &Self, other: &ChabocheIncrement) -> bool { /* ... */ }
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
## Module `damage`

Damage and rupture laws — bead `op-a7p.4`, phase P3 of the code_aster port.

# What these laws are for

Everything in [`crate::rheology::aster::viscoplastic`] conserves the
material: stress relaxes, strain accumulates, but the solid stays the solid
it started as. The laws here do not. Each carries an internal variable that
records irreversible *loss of load-bearing material* — a scalar damage `D`
for the Lemaitre-Chaboche family, a void volume fraction (porosity) `f` for
the Rousselier and Gurson families — and each feeds that variable back into
the stress so the material softens as it degrades.

That feedback is the whole point and the whole difficulty. It is what lets
the model predict *when a component fails* rather than merely how much it
deforms, and it is also what makes the local integration hard: the softening
branch is where the boundary-value problem loses ellipticity, and where a
local solve that quietly clamps its unknown will report a converged answer
that is not a solution of anything.

# The three families, and how they differ

| Family | State variable | Yield surface | Failure mechanism modelled |
|---|---|---|---|
| [`LemaitreChabocheLaw`] (`VENDOCHAB`, `VISC_ENDO_LEMA`) | scalar damage `D` | von Mises on the **effective** stress `sigma/(1-D)` | creep damage — cavitation under sustained high-temperature load |
| [`RousselierLaw`] (`ROUSS_PR`, `ROUSS_VISC`) | porosity `f` | von Mises **plus** an exponential in the mean stress | ductile rupture — growth and coalescence of voids |
| [`GursonTvergaardNeedleman`] (`GTN`, `VISC_GTN`) | porosity `f` + coalescence | von Mises **plus** a `cosh` in the mean stress | ductile rupture, with nucleation and an explicit coalescence stage |

The Lemaitre-Chaboche family keeps a pressure-independent (von Mises) yield
surface and puts all the damage in a multiplicative `(1-D)` factor. The two
porous-plastic families instead make the **yield surface itself** depend on
the hydrostatic stress, because that is the physics: a void grows under
triaxial tension and closes under triaxial compression, so a porous solid
yields sooner in tension than in compression and its plastic flow is no
longer volume-preserving. That single change is why their local solve cannot
be the scalar radial return used for `NORTON` and `LEMAITRE` — the plastic
increment now has a volumetric component, which changes the porosity, which
moves the yield surface, so the equivalent plastic strain and the porosity
must be solved **together**.

# Softening, and what this port does about it

Every law here softens: past some point, more strain means less stress. Two
things go wrong there, and they are different things.

1. **Loss of ellipticity** of the boundary-value problem. Once the tangent
   modulus goes negative the *structural* problem is ill-posed — the
   solution localises into a band whose width is set by the mesh, not by the
   material. No local integrator can fix that; it needs a regularisation
   (nonlocal or gradient damage). Upstream's `VISC_GTN` supplies one via the
   `GRADVARI` modelisation; **this port does not** (see
   [`GursonTvergaardNeedleman`]).
2. **Failure of the local solve** as `D -> 1` or `f -> f_R`. This *is* the
   integrator's business, and here the port is deliberate: when the local
   system has no solution in the admissible range, the integrator says so.
   It never clamps the unknown at the boundary and reports success.

Concretely, each law has one place where upstream itself saturates rather
than converges, and this port reproduces that saturation **as an explicitly
reported state, not as a converged solve**:

- [`LemaitreChabocheLaw`]: upstream caps damage at `D = 0.99`
  (`dammax` in `nmvecd.F90`) and raises alarm `ALGORITH8_67`. This port
  returns [`DamageOutcome::Saturated`] on the same condition, visible in
  [`LemaitreChabocheIncrement::outcome`].
- [`RousselierLaw`]: upstream declares the point broken once the porosity
  reaches `PORO_LIMI` and ramps the stress to zero. Reported as
  [`RousselierOutcome::Broken`]. A genuinely empty bracket in the porosity
  increment — upstream's "subdivide the step" exit — becomes
  [`OffbeatError::ConstitutiveNotConverged`].
- [`GursonTvergaardNeedleman`]: upstream stops at `dam >= dam_bkn`. Reported
  as [`GtnOutcome::Broken`]; a non-convergent staggered solve returns
  [`OffbeatError::ConstitutiveNotConverged`] instead.

# Conventions

Raw `f64` with units stated in prose, matching
[`crate::rheology::aster::viscoplastic`] — not `uom`. Tensors are
[`SymmTensor`] from `outram-foam-basic-lib`; the `sqrt(2)`-scaled Mandel
six-vector used at the code_aster interface lives in
[`crate::rheology::aster::kinematics::AsterVoigt`] and is not needed here,
because every contraction in this module is done tensorially.

All laws here are **small-strain** (`DEFORMATION = PETIT`). Upstream also
offers `GDEF_LOG` for the Rousselier and GTN families; the logarithmic-strain
pre/post-processing that would wrap these laws is in
[`crate::rheology::aster::log_strain`] and is not applied here.

# Status and what is *not* ported

Ported, with tests: `VENDOCHAB`, `VISC_ENDO_LEMA`, `ROUSS_PR`, `ROUSS_VISC`,
`GTN` / `VISC_GTN` (local form only, see below), `CRIT_RUPT`.

**Not ported.** The `ENDO_*` concrete-damage family (`ENDO_ISOT_BETON`,
`ENDO_ORTH_BETON`, `ENDO_SCALAIRE`, `ENDO_FISS_EXP`, ...) is untouched: it
is a different physical domain (quasi-brittle concrete, with unilateral
crack closure and, for several members, a nonlocal `GRAD_VARI` formulation),
and the workspace's target cases are metals. The `GTN` port covers the
**local** yield surface, coalescence, nucleation and return map but **not**
upstream's `GRADVARI` nonlocal regularisation, nor its bespoke
`SPECIFIQUE` reformulation in `(p, ts)` variables; see
[`GursonTvergaardNeedleman`] for the exact boundary.

# Upstream defects found

Two, both in `VENDOCHAB`, both documented by a test in this module's test
file rather than silently corrected:

1. **`nmvexi.F90` reads the wrong material slots.** It takes the
   multiaxiality weights `ALPHA_D` and `BETA_D` of the damage-equivalent
   stress from `mate(2,2)` and `mate(3,2)`, which `vecmat.F90` fills with
   `UN_SUR_M` and `UN_SUR_K` — the Lemaitre viscosity parameters. The
   correct slots are `mate(5,2)` and `mate(6,2)`, which is what the
   Runge-Kutta path (`rkdvec.F90`) uses.
2. **The implicit path accumulates damage with no plasticity.**
   `nmvecd.F90` evaluates the damage-rate equation unconditionally, so a
   purely elastic step with a non-zero `chi` still damages the material. The
   Runge-Kutta path (`rkdvec.F90`) and the `VISC_ENDO_LEMA` path
   (`nmvend.F90`) both gate damage on the plasticity criterion.

This port follows the Runge-Kutta semantics on both points — they are the
self-consistent ones and the ones that match the declared catalogue — while
keeping the **implicit backward-Euler discretisation** of the `NEWTON` path.
See [`LemaitreChabocheLaw`] for the reasoning.

```rust
pub mod damage { /* ... */ }
```

### Types

#### Struct `IsotropicElasticity`

Isotropic linear elasticity, as the two moduli a return map actually needs.

# Why these two and not `E`, `nu`

Every return map in this module splits the stress into its deviatoric and
hydrostatic parts and treats them separately: the deviator scales with the
shear modulus `mu`, the mean stress with the bulk modulus `K`. Storing
`(mu, K)` therefore removes a conversion from the inner loop and makes the
two roles visible. Use [`IsotropicElasticity::from_young_poisson`] when the
data are tabulated the usual way.

# Units

Both moduli in pascal \[Pa\], both strictly positive.

```rust
pub struct IsotropicElasticity {
    pub shear_modulus: f64,
    pub bulk_modulus: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `shear_modulus` | `f64` | Shear modulus `mu = E / (2(1+nu))` \[Pa\], strictly positive. |
| `bulk_modulus` | `f64` | Bulk modulus `K = E / (3(1-2nu))` \[Pa\], strictly positive.<br><br>Relates mean stress to volumetric strain by `sigma_m = K tr(eps)`. |

##### Implementations

###### Methods

- ```rust
  pub fn from_young_poisson(young: f64, poisson: f64) -> Result<Self> { /* ... */ }
  ```
  Build from Young's modulus \[Pa\] and Poisson's ratio \[-\].

- ```rust
  pub fn young(self: Self) -> f64 { /* ... */ }
  ```
  Young's modulus `E = 9 K mu / (3K + mu)` \[Pa\].

- ```rust
  pub fn poisson(self: Self) -> f64 { /* ... */ }
  ```
  Poisson's ratio `nu = (3K - 2mu) / (2(3K + mu))` \[-\].

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
    fn clone(self: &Self) -> IsotropicElasticity { /* ... */ }
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
    fn eq(self: &Self, other: &IsotropicElasticity) -> bool { /* ... */ }
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
#### Struct `LemaitreChabocheParameters`

Material parameters of the Lemaitre-Chaboche damage-coupled viscoplastic
law.

# Where each one comes from

Upstream reads these from two keyword blocks and packs them into one array
(`vecmat.F90`): `LEMAITRE` supplies `N`, `UN_SUR_M`, `UN_SUR_K`, and
`VENDOCHAB` supplies `SY`, `ALPHA_D`, `BETA_D`, `R_D`, `A_D`, `K_D`. The
upstream names are given per field so a deck can be read across; the
reciprocals `UN_SUR_M` and `UN_SUR_K` are stored here as `m` and `k`
themselves, matching
[`LemaitreParameters`](crate::rheology::aster::viscoplastic::LemaitreParameters).

# The two rate equations

Isotropic hardening variable `r` \[-\] and damage `D` \[-\] evolve as

`dr/dt = ((sigma_eq/(1-D) - SY) / (K r^(1/m)))^n`

`dD/dt = (chi / A_D)^R_D * (1-D)^(-K_D)`

with `chi` the damage-equivalent stress built by
[`LemaitreChabocheLaw::damage_equivalent_stress`]. The accumulated
viscoplastic strain follows `dp/dt = (dr/dt)/(1-D)`: the *effective* section
carries the load, so a damaged material strains faster than its hardening
variable advances.

# Units

`k`, `yield_stress` and `damage_strength` in pascal \[Pa\]; `n`, `m`, the
two weights and the two damage exponents dimensionless.

```rust
pub struct LemaitreChabocheParameters {
    pub n: f64,
    pub m: f64,
    pub k: f64,
    pub yield_stress: f64,
    pub principal_weight: f64,
    pub trace_weight: f64,
    pub damage_exponent: f64,
    pub damage_strength: f64,
    pub damage_closure_exponent: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `n` | `f64` | Stress exponent `n` \[-\]. Upstream `N`. Must be strictly positive;<br>upstream errors out (`ier = 11`) on `N <= 0`. |
| `m` | `f64` | Strain-hardening exponent `m` \[-\]. Upstream stores its reciprocal as<br>`UN_SUR_M`. Enters as `r^(1/m)`, so larger `m` means weaker hardening. |
| `k` | `f64` | Viscosity reference stress `K` \[Pa\]. Upstream stores `1/K` as<br>`UN_SUR_K`. |
| `yield_stress` | `f64` | Yield stress `SY` \[Pa\] below which no viscoplastic flow occurs. The<br>criterion is on the **effective** stress: flow begins when<br>`sigma_eq/(1-D) > SY`. |
| `principal_weight` | `f64` | `ALPHA_D` \[-\] — weight of the largest principal stress in `chi`.<br><br>Turns creep damage multiaxial. Zero recovers a purely deviatoric damage<br>driver. Upstream skips the eigenvalue solve entirely when this is at or<br>below `1e-15`, which this port reproduces. |
| `trace_weight` | `f64` | `BETA_D` \[-\] — weight of the trace (three times the mean stress) in<br>`chi`. Makes damage grow faster under hydrostatic tension. |
| `damage_exponent` | `f64` | `R_D` \[-\] — exponent of the damage rate in `chi`. |
| `damage_strength` | `f64` | `A_D` \[Pa\] — damage strength; the `chi` at which the damage rate is<br>one per second. Strictly positive. |
| `damage_closure_exponent` | `f64` | `K_D` \[-\] — exponent of the `(1-D)` closure term. Positive values make<br>damage accelerate as it accumulates, which is what produces the abrupt<br>tertiary-creep knee.<br><br>Upstream allows `K_D` to be supplied as a two-dimensional `NAPPE` in<br>temperature and `chi`; this port takes a constant only. |

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
    fn clone(self: &Self) -> LemaitreChabocheParameters { /* ... */ }
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
    fn eq(self: &Self, other: &LemaitreChabocheParameters) -> bool { /* ... */ }
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
#### Struct `LemaitreChabocheState`

Internal state of a Lemaitre-Chaboche damage point, at one instant.

Mirrors upstream's ten internal variables `EPSPXX..EPSPYZ`, `EPSPEQ`,
`ECROISOT`, `ENDO`, `INDIPLAS` — the last of which (an iteration counter) is
not state and is not kept.

# Units

The strain tensor and all three scalars are dimensionless.

```rust
pub struct LemaitreChabocheState {
    pub viscoplastic_strain: outram_foam_basic_lib::primitives::SymmTensor,
    pub equivalent_viscoplastic_strain: f64,
    pub hardening_variable: f64,
    pub damage: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `viscoplastic_strain` | `outram_foam_basic_lib::primitives::SymmTensor` | Viscoplastic strain tensor `eps_vp` \[-\]. Deviatoric: this law's flow<br>is volume-preserving, unlike the porous-plastic laws below. |
| `equivalent_viscoplastic_strain` | `f64` | Accumulated equivalent viscoplastic strain `p` \[-\], upstream<br>`EPSPEQ`. Grows as `dr/(1-D)`. |
| `hardening_variable` | `f64` | Isotropic hardening variable `r` \[-\], upstream `ECROISOT`. Grows as<br>`dr`; equals `p` only while the material is undamaged. |
| `damage` | `f64` | Damage `D` \[-\], upstream `ENDO`, in `[0, 1)`. The load-bearing section<br>is `(1-D)` of the nominal one. |

##### Implementations

###### Methods

- ```rust
  pub fn pristine() -> Self { /* ... */ }
  ```
  The pristine state: no viscoplastic strain, no hardening, no damage.

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
    fn clone(self: &Self) -> LemaitreChabocheState { /* ... */ }
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
    fn eq(self: &Self, other: &LemaitreChabocheState) -> bool { /* ... */ }
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
#### Enum `DamageOutcome`

How a Lemaitre-Chaboche step ended.

Reported rather than hidden, because the difference between "the local
system was solved" and "the damage hit its numerical ceiling" is exactly the
difference between a result and an artefact.

```rust
pub enum DamageOutcome {
    Elastic,
    Converged,
    Saturated,
}
```

##### Variants

###### `Elastic`

No viscoplastic flow: the effective equivalent stress stayed at or below
`SY`. Damage does not advance either, following the Runge-Kutta
(`rkdvec.F90`) and `VISC_ENDO_LEMA` (`nmvend.F90`) semantics; see the
module documentation for the discrepancy with `nmvecd.F90`.

###### `Converged`

The coupled `(dr, D)` system was solved with `D` strictly below
[`LEMAITRE_CHABOCHE_DAMAGE_MAX`]. This is the only outcome that is a
genuine solution of the constitutive law.

###### `Saturated`

The damage equation had no root below [`LEMAITRE_CHABOCHE_DAMAGE_MAX`]:
over this step the material would have damaged past upstream's ceiling.

Upstream caps `D` at 0.99, zeroes the damage rate and raises alarm
`ALGORITH8_67`; this port does the same *and says so here*. The returned
stress and state are upstream's capped values and **are not** a solution
of the rate equations — treat the point as failed, or re-run the step
with a smaller `dt` and see whether the cap still binds.

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
    fn clone(self: &Self) -> DamageOutcome { /* ... */ }
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
    fn eq(self: &Self, other: &DamageOutcome) -> bool { /* ... */ }
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
#### Struct `LemaitreChabocheIncrement`

The result of integrating one Lemaitre-Chaboche step.

```rust
pub struct LemaitreChabocheIncrement {
    pub stress: outram_foam_basic_lib::primitives::SymmTensor,
    pub effective_equivalent_stress: f64,
    pub damage_equivalent_stress: f64,
    pub state: LemaitreChabocheState,
    pub outcome: DamageOutcome,
    pub rate_linearised: bool,
    pub damage_iterations: usize,
    pub flow_iterations: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `stress` | `outram_foam_basic_lib::primitives::SymmTensor` | Nominal (damaged) Cauchy stress at the end of the step \[Pa\]. This is<br>what a load cell would read; the material's own sections feel<br>`stress/(1-D)`. |
| `effective_equivalent_stress` | `f64` | Effective von Mises equivalent stress `sigma_eq/(1-D)` \[Pa\] — the<br>quantity the flow rule compares against `SY`. |
| `damage_equivalent_stress` | `f64` | Damage-equivalent stress `chi` \[Pa\] at the end of the step. |
| `state` | `LemaitreChabocheState` | Updated internal state. |
| `outcome` | `DamageOutcome` | How the step ended. |
| `rate_linearised` | `bool` | Whether upstream's overflow guard fired — that is, whether the power-law<br>flow rate exceeded `0.1/dt` and was replaced by its tangent<br>linearisation (`nmvecd.F90`, `etatf(2) = 'TANGENT'`, alarm<br>`ALGORITH8_66`).<br><br>When true, the returned increment comes from a **linearised** flow rule,<br>not the power law. Upstream warns; so does this flag. |
| `damage_iterations` | `usize` | Local iterations used by the outer damage solve. |
| `flow_iterations` | `usize` | Local iterations used by the innermost hardening solve at the converged<br>damage. |

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
    fn clone(self: &Self) -> LemaitreChabocheIncrement { /* ... */ }
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
    fn eq(self: &Self, other: &LemaitreChabocheIncrement) -> bool { /* ... */ }
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
#### Enum `LemaitreChabocheLaw`

Lemaitre-Chaboche viscoplasticity coupled to isotropic damage.

# The model

Two coupled scalar rate equations, listed under
[`LemaitreChabocheParameters`], driving a von Mises flow rule written on the
**effective** stress `sigma/(1-D)`. Damage enters twice: it shrinks the
elastic stiffness (`sigma = (1-D) C : (eps - eps_vp)`), and it accelerates
the flow (`dp = dr/(1-D)`). The result is a creep curve with the
characteristic three stages — a decaying primary transient from the `r^(1/m)`
hardening, a quasi-steady secondary stage, and a tertiary runaway as `D`
grows and feeds back on itself.

# ASTER names and upstream provenance

- `VENDOCHAB` (`num_lc = 31`, 10 state variables), keywords `ELAS` +
  `VENDOCHAB` + `LEMAITRE`, `algo_inte` `NEWTON` or `RUNGE_KUTTA`.
- `VISC_ENDO_LEMA` (`num_lc = 31`, 10 state variables), keywords `ELAS` +
  `VISC_ENDO` + `LEMAITRE`, `algo_inte` `SECANTE`, `BRENT` or `DEKKER`.

Legacy symbols: `lc0031`, `nmveei`, `nmvecd`, `nmvexi` (implicit path);
`nmvprk`, `rkdvec` (Runge-Kutta path); `nmvend`, `nmfend`, `nmfedd`
(`VISC_ENDO_LEMA` reduced path). Documentation reference: R5.03.15.

# Which upstream path this port follows, and why it matters

Upstream has three integrators for one law, and they do not agree. This port
takes the **implicit backward-Euler discretisation** of the `NEWTON` path
with the **rate equations of the Runge-Kutta path**, for two reasons set out
in full in the module documentation:

1. `nmvexi.F90` reads `ALPHA_D` and `BETA_D` from the material slots
   `vecmat.F90` fills with `UN_SUR_M` and `UN_SUR_K`. `rkdvec.F90` reads the
   correct slots. The port uses the parameters as declared.
2. `nmvecd.F90` grows damage on elastic steps; `rkdvec.F90` and
   `nmvend.F90` gate damage on the plasticity criterion. The port gates.

Both discrepancies are pinned by tests in this module's test file rather
than being corrected silently.

# Enum dispatch

The two variants differ only in how the damage driver `chi` is built, which
[`Self::damage_equivalent_stress`] shows in one place. Enum, not trait
objects, per the workspace rule.

```rust
pub enum LemaitreChabocheLaw {
    Vendochab(LemaitreChabocheParameters),
    ViscEndoLema(LemaitreChabocheParameters),
}
```

##### Variants

###### `Vendochab`

`VENDOCHAB` — the full multiaxial damage driver.

`chi = ALPHA_D J0(sigma) + BETA_D tr(sigma) + (1 - ALPHA_D - BETA_D) sigma_eq(sigma)`

evaluated on the **nominal** stress, with the closure exponent `K_D`
active. Upstream: `nmvexi.F90` / `rkdvec.F90`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `LemaitreChabocheParameters` |  |

###### `ViscEndoLema`

`VISC_ENDO_LEMA` — the reduced driver: `chi = sigma_eq/(1-D)`, the
**effective** equivalent stress, with `ALPHA_D`, `BETA_D` and `K_D` all
absent from the material keyword block and therefore ignored.

Upstream: `nmfend.F90`, where the damage increment is
`dD = dt (sigma_eq/((1-D) A_D))^R_D`. Note this is *not* `VENDOCHAB`
with the weights zeroed — the two differ by a factor `(1-D)^R_D`,
because `VENDOCHAB` drives damage with the nominal stress and
`VISC_ENDO_LEMA` with the effective one.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `LemaitreChabocheParameters` |  |

##### Implementations

###### Methods

- ```rust
  pub const fn aster_name(self: Self) -> &'static str { /* ... */ }
  ```
  The upstream ASTER behaviour name.

- ```rust
  pub const fn behaviour(self: Self) -> AsterBehaviour { /* ... */ }
  ```
  The catalogue entry this law corresponds to.

- ```rust
  pub const fn parameters(self: Self) -> LemaitreChabocheParameters { /* ... */ }
  ```
  The material parameters, whichever variant this is.

- ```rust
  pub fn damage_equivalent_stress(self: Self, stress: SymmTensor, damage: f64) -> f64 { /* ... */ }
  ```
  Damage-equivalent stress `chi` \[Pa\] — the scalar that drives damage.

- ```rust
  pub fn damage_rate(self: Self, chi: f64, damage: f64) -> f64 { /* ... */ }
  ```
  Damage rate `dD/dt` \[1/s\] at a given driver and damage.

- ```rust
  pub fn hardening_rate(self: Self, effective_equivalent_stress: f64, hardening_variable: f64, dt: f64) -> (f64, bool) { /* ... */ }
  ```
  Viscoplastic hardening rate `dr/dt` \[1/s\], with upstream's overflow

- ```rust
  pub fn integrate(self: Self, elastic: IsotropicElasticity, state: LemaitreChabocheState, total_strain: SymmTensor, dt: f64) -> Result<LemaitreChabocheIncrement> { /* ... */ }
  ```
  Integrate one timestep of the coupled damage-viscoplasticity system.

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
    fn clone(self: &Self) -> LemaitreChabocheLaw { /* ... */ }
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
    fn eq(self: &Self, other: &LemaitreChabocheLaw) -> bool { /* ... */ }
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
#### Struct `RousselierParameters`

Material parameters of the Rousselier porous-plastic law.

# Upstream keyword block

`ROUSSELIER`, read by `rslmat.F90` (`ROUSS_PR`) and `rsvmat.F90`
(`ROUSS_VISC`) in the order `D`, `SIGM_1`, `PORO_INIT`, `PORO_CRIT`,
`PORO_ACCE`, `PORO_LIMI`, `D_SIGM_EPSI_NORM`, then `AN` and `BETA` for
`ROUSS_PR` or `BETA` alone for `ROUSS_VISC` (which forbids nucleation).

# Units

`sigma_1` in pascal \[Pa\]; every porosity and every coefficient
dimensionless.

```rust
pub struct RousselierParameters {
    pub d: f64,
    pub sigma_1: f64,
    pub initial_porosity: f64,
    pub critical_porosity: f64,
    pub acceleration: f64,
    pub limit_porosity: f64,
    pub broken_unloading_slope: f64,
    pub nucleation_rate: f64,
    pub stored_energy_fraction: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `d` | `f64` | `D` \[-\] — the amplitude of the void term in the yield function.<br>Typically around 2 for structural steels. Strictly positive. |
| `sigma_1` | `f64` | `SIGM_1` \[Pa\] — the stress scale of the exponential. Sets how strongly<br>hydrostatic tension softens the material: the void term grows as<br>`exp(sigma_m/SIGM_1)`, so a small `SIGM_1` makes the law violently<br>triaxiality-sensitive. Strictly positive. |
| `initial_porosity` | `f64` | `PORO_INIT` \[-\] — the initial void volume fraction `f0`, the reference<br>against which the reduced stress is defined. Typically `1e-4` to `1e-3`. |
| `critical_porosity` | `f64` | `PORO_CRIT` \[-\] — porosity above which growth is artificially<br>accelerated, standing in for coalescence. |
| `acceleration` | `f64` | `PORO_ACCE` \[-\] — the acceleration factor applied past `PORO_CRIT`.<br><br>Upstream *divides* the volumetric plastic increment by it in the<br>mean-stress update while keeping the same porosity increment, so a<br>larger value means the porosity reaches a given level for less<br>hydrostatic relaxation. One disables acceleration. Strictly positive. |
| `limit_porosity` | `f64` | `PORO_LIMI` \[-\] — the porosity at which the point is declared broken<br>and its stress ramped to zero. |
| `broken_unloading_slope` | `f64` | `D_SIGM_EPSI_NORM` \[-\] — the rate at which a broken point sheds its<br>stress, as a fraction of Young's modulus per unit equivalent strain<br>increment. |
| `nucleation_rate` | `f64` | `AN` \[-\] — strain-controlled nucleation rate, `f_total = f + AN p`.<br><br>Only `ROUSS_PR` activates this; `lcrous.F90` forces it to zero for<br>`ROUSS_VISC`, and [`RousselierLaw::nucleation_rate`] does the same. |
| `stored_energy_fraction` | `f64` | `BETA` \[-\] — the split of plastic work between dissipated heat and<br>energy stored in the microstructure. Enters the dissipation bookkeeping<br>only, never the stress. |

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
    fn clone(self: &Self) -> RousselierParameters { /* ... */ }
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
    fn eq(self: &Self, other: &RousselierParameters) -> bool { /* ... */ }
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
#### Struct `ViscousSinhParameters`

The `VISC_SINH` viscous overstress that turns `ROUSS_PR` into `ROUSS_VISC`.

# The model

The yield function gains a rate-dependent term

`Phi_visc = Phi - SIGM_0 asinh( (dp/(dt EPSI_0))^(1/M) )`

so the material can sustain a stress above its rate-independent yield
surface, by an amount that grows logarithmically with the plastic strain
rate. The inverse hyperbolic sine is the classical high-stress creep form:
linear in the rate at low rates and logarithmic at high ones, which avoids
the unbounded stress a pure power law gives at high rate.

Upstream: `rslphi.F90`, keyword block `VISC_SINH` with `SIGM_0`, `EPSI_0`,
`M`.

# Units

`sigma_0` in pascal \[Pa\], `reference_strain_rate` in per second \[1/s\],
`exponent` dimensionless.

```rust
pub struct ViscousSinhParameters {
    pub sigma_0: f64,
    pub reference_strain_rate: f64,
    pub exponent: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `sigma_0` | `f64` | `SIGM_0` \[Pa\] — the amplitude of the viscous overstress. |
| `reference_strain_rate` | `f64` | `EPSI_0` \[1/s\] — the reference plastic strain rate. Strictly positive. |
| `exponent` | `f64` | `M` \[-\] — the rate exponent. Strictly positive; larger means a weaker<br>rate dependence. |

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
    fn clone(self: &Self) -> ViscousSinhParameters { /* ... */ }
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
    fn eq(self: &Self, other: &ViscousSinhParameters) -> bool { /* ... */ }
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
#### Struct `RousselierState`

Internal state of a Rousselier point.

Mirrors upstream's five internal variables `EPSPEQ`, `POROSITE`, `DISSIP`,
`EBLOC`, `INDIPLAS`.

```rust
pub struct RousselierState {
    pub equivalent_plastic_strain: f64,
    pub porosity: f64,
    pub dissipation: f64,
    pub blocked_energy: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `equivalent_plastic_strain` | `f64` | Accumulated equivalent plastic strain `p` \[-\], upstream `EPSPEQ`. |
| `porosity` | `f64` | Void volume fraction `f` \[-\], upstream `POROSITE`. Starts at<br>[`RousselierParameters::initial_porosity`]. |
| `dissipation` | `f64` | Plastic dissipation rate \[W/m^3\], upstream `DISSIP`. Bookkeeping only<br>— it never re-enters the stress update. |
| `blocked_energy` | `f64` | Energy stored in the microstructure \[J/m^3\], upstream `EBLOC`. Also<br>bookkeeping only. |

##### Implementations

###### Methods

- ```rust
  pub fn initial(params: RousselierParameters) -> Self { /* ... */ }
  ```
  The initial state for a given material: undeformed, at the material's

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
    fn clone(self: &Self) -> RousselierState { /* ... */ }
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
    fn eq(self: &Self, other: &RousselierState) -> bool { /* ... */ }
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
#### Enum `RousselierOutcome`

How a Rousselier step ended.

```rust
pub enum RousselierOutcome {
    Elastic,
    VonMises,
    Coupled,
    Broken,
}
```

##### Variants

###### `Elastic`

The stress stayed inside the yield surface; no plastic flow, no void
growth.

###### `VonMises`

Plastic, but with a compressive mean stress or a zero starting porosity,
so the yield surface degenerated to von Mises and the coupled solve
reduced to a scalar return at frozen porosity.

Upstream takes this branch explicitly (`lcrous.F90`, the `df2 < 0` and
`fi == 0` tests) and this port reproduces it, because the coupled
bracket in the porosity increment is genuinely empty there — voids
cannot grow under compression.

###### `Coupled`

The coupled `(dp, df)` system was solved.

###### `Broken`

The point was already broken on entry (`f_total >= PORO_LIMI`): the
stress is being ramped to zero over a strain scale set by
`D_SIGM_EPSI_NORM`, the porosity is pinned at one, and no constitutive
solve was attempted. Upstream's "materiau casse" branch.

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
    fn clone(self: &Self) -> RousselierOutcome { /* ... */ }
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
    fn eq(self: &Self, other: &RousselierOutcome) -> bool { /* ... */ }
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
#### Struct `RousselierIncrement`

The result of integrating one Rousselier step.

```rust
pub struct RousselierIncrement {
    pub stress: outram_foam_basic_lib::primitives::SymmTensor,
    pub reduced_equivalent_stress: f64,
    pub reduced_mean_stress: f64,
    pub plastic_strain_increment: f64,
    pub porosity_increment: f64,
    pub state: RousselierState,
    pub outcome: RousselierOutcome,
    pub iterations: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `stress` | `outram_foam_basic_lib::primitives::SymmTensor` | Cauchy stress at the end of the step \[Pa\]. |
| `reduced_equivalent_stress` | `f64` | Reduced equivalent stress `sigma_eq/rho` \[Pa\] at the end of the step,<br>where `rho = (1 - f_total)/(1 - f0)` is the section-loss factor. This is<br>the quantity the yield function compares against `R(p)`. |
| `reduced_mean_stress` | `f64` | Reduced mean stress `sigma_m/rho` \[Pa\] at the end of the step. |
| `plastic_strain_increment` | `f64` | Equivalent plastic strain increment `dp` \[-\] over the step. |
| `porosity_increment` | `f64` | Porosity increment `df` \[-\] over the step. |
| `state` | `RousselierState` | Updated internal state. |
| `outcome` | `RousselierOutcome` | How the step ended. |
| `iterations` | `usize` | Local iterations used. |

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
    fn clone(self: &Self) -> RousselierIncrement { /* ... */ }
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
    fn eq(self: &Self, other: &RousselierIncrement) -> bool { /* ... */ }
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
#### Enum `RousselierLaw`

Rousselier's porous-plastic law for ductile rupture.

# The model

Rousselier's yield function, written on the **reduced stress**
`sigma_tilde = sigma/rho` with `rho = (1 - f_total)/(1 - f0)`:

`Phi = sigma_tilde_eq - R(p) + D SIGM_1 f exp(sigma_tilde_m / SIGM_1)`

The third term is what makes this a damage law rather than a plasticity law.
It is positive, so it *shrinks* the elastic domain; it is proportional to
the porosity, so an initially near-dense material behaves like von Mises and
progressively softens as voids grow; and it is exponential in the mean
stress, so the softening is enormously more aggressive under triaxial
tension than in shear. That last property is the model's whole reason for
existing: it is why a notched tensile bar fails at a fraction of the strain
of a smooth one, and why a crack tip — where triaxiality is highest — is
where ductile tearing initiates.

Void growth follows from normality. The mean-stress term contributes a
volumetric component to the plastic flow, and mass conservation of the
matrix turns that into

`df = f (1 - f) D exp(sigma_tilde_m/SIGM_1) PORO_ACCE dp`

which is upstream's `dp = df/(f (1-f) D exp(...) acc)` inverted
(`rslphi.F90`). The coupling runs both ways — porosity changes the yield
surface, the yield surface changes the plastic increment, the plastic
increment changes the porosity — which is why the local solve is on `df`
with `dp` eliminated, rather than the scalar return of a von Mises law.

# ASTER names and upstream provenance

- `ROUSS_PR` (`num_lc = 30`, 5 state variables), keywords `ELAS` +
  `ROUSSELIER`, `algo_inte` `NEWTON_1D`.
- `ROUSS_VISC` (`num_lc = 30`, 5 state variables), keywords `ELAS` +
  `ROUSSELIER` + `VISC_SINH`, `algo_inte` `NEWTON_1D`.

Legacy symbols: `lc0030`, `plasti`, `lcrous`, `rslphi`, `rslcvx`.
Documentation reference: R5.03.06.

# What this port does and does not reproduce

**Reproduced:** the reduced-stress formulation, the theta-method (upstream
recommends `PARM_THETA = 0.5`), the acceleration past `PORO_CRIT`,
strain-controlled nucleation `AN` for `ROUSS_PR`, the `VISC_SINH` overstress
for `ROUSS_VISC`, the broken-point stress ramp past `PORO_LIMI`, the
exponent guards at `sigma_m/SIGM_1 > 200` and `< -50`, and the dissipation /
stored-energy bookkeeping.

**Not reproduced:** upstream's hand-rolled Newton-with-chord-fallback on the
porosity increment, replaced by [`brent`] on the same bracket upstream
computes — same equation, same bracket, a solver with a convergence
guarantee instead of one without. The consistent tangent operator
(`lcotan`, `rsljpl`) is not ported; upstream itself defaults this law's
tangent to `PERTURBATION`.

**Deliberate addition:** an explicit elastic test. `lcrous.F90` assumes its
caller (`plasti.F90`, via `rslcvx.F90`) has already established that the
point is plastic, and its bracket test `phi1 < 0` — which an elastic point
satisfies — is reported as "strain increment too large, subdivide". This
port evaluates the yield function at zero increment first and returns
[`RousselierOutcome::Elastic`]: the same physics, and a far better
diagnostic.

```rust
pub enum RousselierLaw {
    Plastic(RousselierParameters),
    Viscous(RousselierParameters, ViscousSinhParameters),
}
```

##### Variants

###### `Plastic`

`ROUSS_PR` — rate-independent, with optional strain-controlled void
nucleation through [`RousselierParameters::nucleation_rate`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `RousselierParameters` |  |

###### `Viscous`

`ROUSS_VISC` — with the `VISC_SINH` viscous overstress. Upstream forces
`AN = 0` for this variant, and so does this port.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `RousselierParameters` |  |
| 1 | `ViscousSinhParameters` |  |

##### Implementations

###### Methods

- ```rust
  pub const fn aster_name(self: Self) -> &'static str { /* ... */ }
  ```
  The upstream ASTER behaviour name.

- ```rust
  pub const fn behaviour(self: Self) -> AsterBehaviour { /* ... */ }
  ```
  The catalogue entry this law corresponds to.

- ```rust
  pub const fn parameters(self: Self) -> RousselierParameters { /* ... */ }
  ```
  The Rousselier material parameters, whichever variant this is.

- ```rust
  pub const fn nucleation_rate(self: Self) -> f64 { /* ... */ }
  ```
  The effective nucleation rate `AN` \[-\]: the declared value for

- ```rust
  pub fn yield_function(self: Self, reduced_equivalent: f64, reduced_mean: f64, porosity: f64, flow_stress: f64) -> f64 { /* ... */ }
  ```
  The rate-independent Rousselier yield function \[Pa\].

- ```rust
  pub fn viscous_overstress(self: Self, plastic_increment: f64, dt: f64) -> f64 { /* ... */ }
  ```
  The viscous overstress `SIGM_0 asinh((dp/(dt EPSI_0))^(1/M))` \[Pa\].

- ```rust
  pub fn integrate(self: Self, elastic: IsotropicElasticity, hardening: IsotropicHardening, state: RousselierState, stress_start: SymmTensor, strain_increment: SymmTensor, dt: f64, theta: f64) -> Result<RousselierIncrement> { /* ... */ }
  ```
  Integrate one timestep of the coupled plasticity-porosity system.

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
    fn clone(self: &Self) -> RousselierLaw { /* ... */ }
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
    fn eq(self: &Self, other: &RousselierLaw) -> bool { /* ... */ }
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
#### Struct `GtnNucleation`

Nucleation of new voids, as the sum of upstream's three mechanisms.

# The three terms

Upstream's `Nucleation` in `lcgtn_module.F90` adds:

1. **Chu-Needleman Gaussian** —
   `0.5 FN [erf((k - PN)/(sqrt(2) SN)) + erf(PN/(sqrt(2) SN))]`.
   Second-phase particles decohere over a narrow band of plastic strain
   centred on `PN`; the cumulative Gaussian is the fraction that has done
   so. The second `erf` shifts the curve so it starts from zero at zero
   strain.
2. **Ramp** — `min(C0 (k - KI)/(KF - KI), C0)`, clamped below at zero: a
   linear onset between two strain thresholds, saturating at `C0`.
3. **Linear tail** — `B0 max(p_cum - EPC, 0)`: unbounded nucleation past a
   strain threshold.

# Units

All porosities and strains dimensionless.

```rust
pub struct GtnNucleation {
    pub gaussian_porosity: f64,
    pub gaussian_mean_strain: f64,
    pub gaussian_std_dev: f64,
    pub ramp_porosity: f64,
    pub ramp_start: f64,
    pub ramp_end: f64,
    pub linear_slope: f64,
    pub linear_start: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `gaussian_porosity` | `f64` | `NUCL_GAUSS_PORO` (`FN`) \[-\] — the total void fraction available to<br>Gaussian nucleation. Zero disables the term. |
| `gaussian_mean_strain` | `f64` | `NUCL_GAUSS_PLAS` (`PN`) \[-\] — the mean nucleation strain. Upstream<br>defaults to 0.1. |
| `gaussian_std_dev` | `f64` | `NUCL_GAUSS_DEV` (`SN`) \[-\] — the standard deviation. Upstream<br>defaults to 0.05. Must be strictly positive when<br>`gaussian_porosity > 0`. |
| `ramp_porosity` | `f64` | `NUCL_CRAN_PORO` (`C0`) \[-\] — the saturation of the ramp term. |
| `ramp_start` | `f64` | `NUCL_CRAN_INIT` (`KI`) \[-\] — where the ramp starts. Upstream default<br>0.05. |
| `ramp_end` | `f64` | `NUCL_CRAN_FIN` (`KF`) \[-\] — where the ramp saturates. Upstream<br>default 0.15. Must exceed `ramp_start` when `ramp_porosity > 0`. |
| `linear_slope` | `f64` | `NUCL_EPSI_PENTE` (`B0`) \[-\] — slope of the linear tail. |
| `linear_start` | `f64` | `NUCL_EPSI_INIT` (`EPC`) \[-\] — where the linear tail starts. Upstream<br>default 0.8. |

##### Implementations

###### Methods

- ```rust
  pub fn none() -> Self { /* ... */ }
  ```
  No nucleation at all — every mechanism switched off.

- ```rust
  pub fn porosity(self: Self, kappa: f64, cumulated_plastic_strain: f64) -> f64 { /* ... */ }
  ```
  Nucleated void volume fraction \[-\] at hardening variable `kappa` and

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
    fn clone(self: &Self) -> GtnNucleation { /* ... */ }
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
    fn eq(self: &Self, other: &GtnNucleation) -> bool { /* ... */ }
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
#### Struct `GtnParameters`

Material parameters of the Gurson-Tvergaard-Needleman law.

# Upstream keyword block

`GTN`, read by `Init` in `lcgtn_module.F90`: `Q1`, `Q2`, `PORO_INIT`,
`COAL_PORO`, `COAL_ACCE`, `PORO_RUPT`, then the eight nucleation keywords
carried by [`GtnNucleation`], then `ENDO_CRIT_VISC` and `ENDO_CRIT_RUPT`.

# Units

All dimensionless.

```rust
pub struct GtnParameters {
    pub q1: f64,
    pub q2: f64,
    pub initial_porosity: f64,
    pub coalescence_porosity: f64,
    pub coalescence_slope: f64,
    pub rupture_porosity: f64,
    pub nucleation: GtnNucleation,
    pub broken_damage: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `q1` | `f64` | `Q1` \[-\] — Tvergaard's first correction. Multiplies the porosity<br>everywhere it appears, so `1/Q1` is the porosity at which the material<br>loses all strength. Typically 1.5. Strictly positive. |
| `q2` | `f64` | `Q2` \[-\] — Tvergaard's second correction, inside the `cosh`. Scales<br>the sensitivity to hydrostatic stress. Typically 1.0. Strictly positive. |
| `initial_porosity` | `f64` | `PORO_INIT` (`f0`) \[-\] — the initial void volume fraction, and a floor<br>on the growth porosity thereafter. Strictly positive: Gurson's surface<br>with `f = 0` degenerates to von Mises and upstream asserts against it. |
| `coalescence_porosity` | `f64` | `COAL_PORO` (`fc`) \[-\] — the porosity at which coalescence starts.<br>Below it the effective porosity is the true one. |
| `coalescence_slope` | `f64` | Coalescence slope `hc = COAL_ACCE - 1` \[-\], non-negative.<br><br>Past `fc`, Tvergaard and Needleman's effective porosity is<br>`f* = f + hc (f - fc)`, so the material loses strength faster than the<br>voids actually grow. This is the model's stand-in for the plastic<br>collapse of the ligaments between voids, which the smooth Gurson surface<br>cannot represent. |
| `rupture_porosity` | `f64` | `PORO_RUPT` (`fR`) \[-\] — the porosity at which `f*` reaches `1/Q1` and<br>the material carries nothing. Consistency requires<br>`fR = fc + (1/Q1 - fc)/(1 + hc)`, which<br>[`GtnParameters::rupture_porosity_from_slope`] computes. |
| `nucleation` | `GtnNucleation` | Void nucleation. |
| `broken_damage` | `f64` | `ENDO_CRIT_RUPT` \[-\] — the damage `D = Q1 f*` above which upstream<br>declares the point broken and stops integrating. Upstream additionally<br>caps it at `1 - sqrt(tolerance)`. |

##### Implementations

###### Methods

- ```rust
  pub fn rupture_porosity_from_slope(q1: f64, coalescence_porosity: f64, slope: f64) -> f64 { /* ... */ }
  ```
  The rupture porosity implied by `Q1`, `fc` and the coalescence slope.

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
    fn clone(self: &Self) -> GtnParameters { /* ... */ }
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
    fn eq(self: &Self, other: &GtnParameters) -> bool { /* ... */ }
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
#### Struct `GtnState`

Internal state of a GTN point.

Upstream carries 25 internal variables for `VISC_GTN`, most of them
post-processing echoes of the stress and of intermediate equivalent
stresses. The six kept here are the ones the update actually needs.

# Units

All dimensionless.

```rust
pub struct GtnState {
    pub plastic_strain: outram_foam_basic_lib::primitives::SymmTensor,
    pub hardening_variable: f64,
    pub growth_porosity: f64,
    pub nucleation_porosity: f64,
    pub coalescence_damage: f64,
    pub cumulated_plastic_strain: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `plastic_strain` | `outram_foam_basic_lib::primitives::SymmTensor` | Plastic strain tensor \[-\]. **Not** deviatoric: GTN's flow rule has a<br>volumetric component, and that component is exactly what grows the<br>voids. |
| `hardening_variable` | `f64` | Hardening variable `kappa` \[-\], upstream `EPSPEQ`. Defined by the work<br>equivalence `(1-f) R(kappa) dkappa = sigma : deps_p`, so it is the<br>plastic strain of the *matrix*, not of the porous aggregate. |
| `growth_porosity` | `f64` | Growth part of the porosity \[-\], upstream `poro_grow`. Floored at<br>`f0`. |
| `nucleation_porosity` | `f64` | Nucleated part of the porosity \[-\], upstream `PORO_NUC`. |
| `coalescence_damage` | `f64` | Coalescence contribution to the damage \[-\], upstream `dam_coal`. A<br>ratchet: it never decreases. |
| `cumulated_plastic_strain` | `f64` | Cumulated equivalent plastic strain `sqrt(2/3 deps_p : deps_p)`<br>integrated over the history \[-\], upstream `EPCUM`. Drives the linear<br>nucleation tail; distinct from `kappa`. |

##### Implementations

###### Methods

- ```rust
  pub fn initial(params: GtnParameters) -> Self { /* ... */ }
  ```
  The initial state for a given material: undeformed, at `f0`, no

- ```rust
  pub fn porosity(self: Self) -> f64 { /* ... */ }
  ```
  Total porosity `f = f_growth + f_nucleation` \[-\].

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
    fn clone(self: &Self) -> GtnState { /* ... */ }
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
    fn eq(self: &Self, other: &GtnState) -> bool { /* ... */ }
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
#### Enum `GtnOutcome`

How a GTN step ended.

```rust
pub enum GtnOutcome {
    Elastic,
    Plastic,
    Broken,
}
```

##### Variants

###### `Elastic`

The trial stress was inside the yield surface.

###### `Plastic`

The staggered plastic solve converged.

###### `Broken`

The damage reached [`GtnParameters::broken_damage`]: upstream stops
integrating and returns a zero stress. Not a converged solve — the point
has failed.

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
    fn clone(self: &Self) -> GtnOutcome { /* ... */ }
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
    fn eq(self: &Self, other: &GtnOutcome) -> bool { /* ... */ }
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
#### Struct `GtnIncrement`

The result of integrating one GTN step.

```rust
pub struct GtnIncrement {
    pub stress: outram_foam_basic_lib::primitives::SymmTensor,
    pub equivalent_stress: f64,
    pub mean_stress: f64,
    pub flow_stress: f64,
    pub damage: f64,
    pub state: GtnState,
    pub outcome: GtnOutcome,
    pub iterations: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `stress` | `outram_foam_basic_lib::primitives::SymmTensor` | Cauchy stress at the end of the step \[Pa\]. |
| `equivalent_stress` | `f64` | Von Mises equivalent of [`Self::stress`] \[Pa\]. |
| `mean_stress` | `f64` | Mean stress of [`Self::stress`] \[Pa\]. |
| `flow_stress` | `f64` | Flow stress `sigma_star = R(kappa) + viscous overstress` \[Pa\] at the<br>end of the step — the yield surface's size parameter. |
| `damage` | `f64` | Damage `D = Q1 f* + coalescence` \[-\] at the end of the step, capped at<br>one. |
| `state` | `GtnState` | Updated internal state. |
| `outcome` | `GtnOutcome` | How the step ended. |
| `iterations` | `usize` | Outer (staggered) iterations used. |

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
    fn clone(self: &Self) -> GtnIncrement { /* ... */ }
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
    fn eq(self: &Self, other: &GtnIncrement) -> bool { /* ... */ }
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
#### Struct `NortonOverstress`

Norton viscous overstress used by `VISC_GTN`.

`sigma_v = K (dkappa/dt)^(1/n)` \[Pa\] — the extra stress the matrix
sustains when it is being strained at a finite rate. Upstream:
`visc_norton_module.F90` (`dka_to_vsc`), which asserts `K > 0` and `n > 1`.

```rust
pub struct NortonOverstress {
    pub n: f64,
    pub k: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `n` | `f64` | `N` \[-\] — the Norton exponent. Upstream asserts `N > 1`. |
| `k` | `f64` | `K` \[Pa s^(1/n)\] — the viscosity coefficient. Strictly positive. |

##### Implementations

###### Methods

- ```rust
  pub fn stress(self: Self, dkappa: f64, dt: f64) -> f64 { /* ... */ }
  ```
  Overstress \[Pa\] for a hardening increment `dkappa` \[-\] over `dt`

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
    fn clone(self: &Self) -> NortonOverstress { /* ... */ }
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
    fn eq(self: &Self, other: &NortonOverstress) -> bool { /* ... */ }
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
#### Enum `GursonTvergaardNeedleman`

Gurson-Tvergaard-Needleman porous plasticity.

# The yield surface

Upstream writes it (`f_g` in `lcgtn_module.F90`) as

`Phi = (sigma_eq/sigma_star)^2 + 2 D cosh(3 Q2 sigma_m / (2 sigma_star)) - 1 - D^2`

with `D = Q1 f* + coalescence damage` the Tvergaard damage and `sigma_star`
the matrix flow stress. Two limits make the shape clear:

- `D = 0`: the surface collapses to `sigma_eq = sigma_star`, plain von
  Mises. A dense material is rate-independent J2 plasticity.
- `sigma_eq = 0`: the surface gives
  `cosh(3 Q2 sigma_m/(2 sigma_star)) = (1 + D^2)/(2D)`, a **finite**
  hydrostatic strength. That is the essential difference from J2 plasticity,
  which has none: a porous solid yields under pure pressure, and that is
  what drives ductile tearing at a crack tip.

Compared with [`RousselierLaw`], the `cosh` is symmetric in `sigma_m` where
Rousselier's `exp` is not — so GTN predicts void *collapse* in compression
with the same law that predicts growth in tension, and Rousselier needs a
separate branch for it.

# Coalescence

The effective porosity is Tvergaard and Needleman's
`f* = f + hc max(0, f - fc)` ([`Self::star_porosity`]). Below `fc` nothing
changes; above it the material loses strength `1 + hc` times faster than the
voids grow, reaching zero strength at `f = fR`. This is a phenomenological
stand-in for the plastic collapse of the intervoid ligaments, and it is the
mechanism that makes the failure abrupt rather than asymptotic.

# ASTER names and upstream provenance

- `GTN` (`num_lc = 75`, 25 state variables), keywords `ELAS` + `ECRO_NL` +
  `GTN` (+ `NONLOCAL`), `algo_inte` `SPECIFIQUE`.
- `VISC_GTN` (`num_lc = 75`, 25 state variables), the same plus `NORTON`.

Legacy symbols: `lc0075`, `lcgtn_module`, `visc_norton_module`.
Documentation reference: R5.03.29.

# What this port does and does not reproduce — read this before using it

**Reproduced:** the yield surface, the Tvergaard-Needleman coalescence map,
all three nucleation mechanisms, the implicit growth law
`f_grow = (f_grow_old + tr(deps_p)(1 - f_nucl)) / (1 + tr(deps_p))`, the
porosity cap at `fR`, the coalescence ratchet, the `ECRO_NL` hardening
curve, the Norton overstress `K (dkappa/dt)^(1/n)` of `VISC_GTN`, and the
broken-point cutoff.

**Not reproduced, and this matters:**

- **The `GRADVARI` nonlocal regularisation.** Upstream's `VISC_GTN` is
  normally used with a nonlocal damage variable (the `phi` and `r` fields in
  `lcgtn_module.F90`), precisely because a local softening law gives
  mesh-dependent answers. This port is **local only**. A structural
  calculation with it will localise into one element band and the answer
  will depend on the mesh. That is a property of the model as ported, not a
  bug to be tuned away.
- **Upstream's `SPECIFIQUE` algorithm.** Upstream reformulates the local
  problem in variables `(p, ts)` with bespoke bounds (`bnd_pmin`,
  `bnd_pmax`) and a singular-state branch. This port uses a **staggered
  scheme** instead: an inner bracketed [`brent`] on the plastic multiplier
  at frozen damage and flow stress, wrapped in an outer fixed point on
  `(damage, flow stress)`. Same equations, different iteration. It is
  simpler and its inner bracket is provable; it converges more slowly, and
  near `D -> 1` it can fail to converge at all — in which case it returns
  [`OffbeatError::ConstitutiveNotConverged`] rather than a clamped answer.
- **The consistent tangent.** Upstream builds one; this port does not.
  Upstream's own catalogue offers `PERTURBATION` for this law.
- **`theta`.** Upstream's theta-predictor on the porosity is not exposed;
  this port is fully implicit (`theta = 1`).

```rust
pub enum GursonTvergaardNeedleman {
    RateIndependent(GtnParameters),
    Viscous(GtnParameters, NortonOverstress),
}
```

##### Variants

###### `RateIndependent`

`GTN` — rate-independent.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `GtnParameters` |  |

###### `Viscous`

`VISC_GTN` — with a Norton overstress on the matrix flow stress.

The flow stress becomes `sigma_star = R(kappa) + K (dkappa/dt)^(1/n)`.
Keyword block `NORTON` with `N` and `K`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `GtnParameters` |  |
| 1 | `NortonOverstress` |  |

##### Implementations

###### Methods

- ```rust
  pub const fn aster_name(self: Self) -> &'static str { /* ... */ }
  ```
  The upstream ASTER behaviour name.

- ```rust
  pub const fn behaviour(self: Self) -> AsterBehaviour { /* ... */ }
  ```
  The catalogue entry this law corresponds to.

- ```rust
  pub const fn parameters(self: Self) -> GtnParameters { /* ... */ }
  ```
  The GTN material parameters, whichever variant this is.

- ```rust
  pub fn star_porosity(self: Self, porosity: f64) -> f64 { /* ... */ }
  ```
  Tvergaard-Needleman effective porosity `f* = f + hc max(0, f - fc)`

- ```rust
  pub fn yield_function(self: Self, equivalent_stress: f64, mean_stress: f64, flow_stress: f64, damage: f64) -> f64 { /* ... */ }
  ```
  The GTN yield function \[-\], dimensionless.

- ```rust
  pub fn overstress(self: Self, dkappa: f64, dt: f64) -> f64 { /* ... */ }
  ```
  Viscous overstress \[Pa\] for a hardening increment `dkappa` \[-\] over

- ```rust
  pub fn integrate(self: Self, elastic: IsotropicElasticity, hardening: IsotropicHardening, state: GtnState, total_strain: SymmTensor, dt: f64) -> Result<GtnIncrement> { /* ... */ }
  ```
  Integrate one timestep.

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
    fn clone(self: &Self) -> GursonTvergaardNeedleman { /* ... */ }
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
    fn eq(self: &Self, other: &GursonTvergaardNeedleman) -> bool { /* ... */ }
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
#### Struct `RuptureCriterion`

Parameters of the `CRIT_RUPT` rupture criterion.

# Units

`critical_stress` in pascal \[Pa\]; `stiffness_divisor` dimensionless.

```rust
pub struct RuptureCriterion {
    pub critical_stress: f64,
    pub stiffness_divisor: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `critical_stress` | `f64` | `SIGM_C` \[Pa\] — the critical maximum principal stress. When the<br>element-averaged stress state's largest principal stress exceeds this,<br>the element is declared broken. |
| `stiffness_divisor` | `f64` | `COEF` \[-\] — the factor by which a broken element's Young's modulus is<br>**divided**. Upstream: `e = e/coef` in `rupmat.F90`, so a large `COEF`<br>means a nearly-zero residual stiffness. Strictly positive. |

##### Implementations

###### Methods

- ```rust
  pub const fn aster_name(self: Self) -> &'static str { /* ... */ }
  ```
  The upstream ASTER behaviour name.

- ```rust
  pub const fn behaviour(self: Self) -> AsterBehaviour { /* ... */ }
  ```
  The catalogue entry this corresponds to.

- ```rust
  pub fn evaluate(self: Self, element_mean_stress: SymmTensor, plastic_strain_increment: f64, dt: f64, previous: RuptureState) -> Result<RuptureState> { /* ... */ }
  ```
  Evaluate the criterion for one element over one step.

- ```rust
  pub fn degraded_young_modulus(self: Self, young: f64, broken: bool) -> f64 { /* ... */ }
  ```
  Young's modulus \[Pa\] to use for an element, given whether it is broken.

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
    fn clone(self: &Self) -> RuptureCriterion { /* ... */ }
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
    fn eq(self: &Self, other: &RuptureCriterion) -> bool { /* ... */ }
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
#### Struct `RuptureState`

The six internal variables `CRIT_RUPT` appends to the host law's.

Upstream: `EPSPVIT`, `EDISS`, `EDISSCUM`, `PDISS`, `PDISSCUM`, `CRITRUPT`.

```rust
pub struct RuptureState {
    pub plastic_strain_rate: f64,
    pub dissipated_energy: f64,
    pub cumulated_dissipated_energy: f64,
    pub dissipated_power: f64,
    pub cumulated_dissipated_power: f64,
    pub broken: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `plastic_strain_rate` | `f64` | `EPSPVIT` — equivalent plastic strain rate `dp/dt` \[1/s\]. |
| `dissipated_energy` | `f64` | `EDISS` — energy dissipated over this step, `dp sigma_eq` \[J/m^3\]. |
| `cumulated_dissipated_energy` | `f64` | `EDISSCUM` — cumulated dissipated energy \[J/m^3\]. |
| `dissipated_power` | `f64` | `PDISS` — dissipated power, `dp/dt sigma_eq` \[W/m^3\]. |
| `cumulated_dissipated_power` | `f64` | `PDISSCUM` — cumulated dissipated power \[W/m^3\]. Upstream sums the<br>per-step powers rather than integrating them, so this is a running sum<br>of rates and not an energy; the name is upstream's and the behaviour is<br>reproduced as found. |
| `broken` | `bool` | `CRITRUPT` — the rupture flag. Once true it stays true: upstream<br>re-asserts it on every subsequent step ("la maille etait deja cassee.<br>elle le reste"). |

##### Implementations

###### Methods

- ```rust
  pub fn pristine() -> Self { /* ... */ }
  ```
  The initial state: nothing dissipated, nothing broken.

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
    fn clone(self: &Self) -> RuptureState { /* ... */ }
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
    fn eq(self: &Self, other: &RuptureState) -> bool { /* ... */ }
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

#### Function `mean_stress`

**Attributes:**

- `MustUse { reason: None }`

Mean (hydrostatic) stress `sigma_m = tr(sigma)/3` \[Pa\].

Positive in tension. This is the invariant that drives void growth in the
Rousselier and Gurson families, and the reason their yield surfaces are not
pressure-independent.

```rust
pub fn mean_stress(sigma: outram_foam_basic_lib::primitives::SymmTensor) -> f64 { /* ... */ }
```

#### Function `equivalent_stress`

**Attributes:**

- `MustUse { reason: None }`

Von Mises equivalent stress `sigma_eq = sqrt(3/2 s:s)` of a **full** stress
tensor \[Pa\].

Convenience wrapper that takes the deviator first — unlike
[`von_mises_of_deviator`], which expects the deviator and inflates its
answer if given a stress with a hydrostatic part.

```rust
pub fn equivalent_stress(sigma: outram_foam_basic_lib::primitives::SymmTensor) -> f64 { /* ... */ }
```

#### Function `max_principal_stress`

**Attributes:**

- `MustUse { reason: None }`

Largest principal stress `J0 = max(sigma_1, sigma_2, sigma_3)` \[Pa\].

Upstream's `calcj0`. This is the invariant that makes creep damage
*multiaxial*: cavities open on the planes normal to the greatest tension, so
a state with a large maximum principal stress damages faster than a
deviatorically equivalent state without one. Signed — a wholly compressive
state returns its largest (least negative) eigenvalue.

```rust
pub fn max_principal_stress(sigma: outram_foam_basic_lib::primitives::SymmTensor) -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `LEMAITRE_CHABOCHE_DAMAGE_MAX`

Upstream's saturation damage, `dammax` in `nmvecd.F90` and `nmveei.F90`.

Damage is capped here rather than at 1 because the effective stress
`sigma/(1-D)` and the damage rate's `(1-D)^(-K_D)` factor both blow up at
`D = 1`. 0.99 is upstream's choice; it is a numerical fence, not physics.

```rust
pub const LEMAITRE_CHABOCHE_DAMAGE_MAX: f64 = 0.99;
```

## Module `fracture`

Linear-elastic fracture mechanics: the parts of code_aster's `CALC_G` that
are *not* finite-element work.

# Read this first: most of `bibfor/fracture` is **not** ported yet

code_aster computes the energy release rate `G` and the stress intensity
factors `K_I, K_II, K_III` by the **G-theta method**: a domain integral

`G = ∫_V [σ:∇u · ∇θ - W ∇·θ] dV`

over a ring surrounding the crack front, driven by a *virtual crack
extension field* `θ`.

## Not blocked on finite elements

An earlier revision of this note said the method needs element shape
functions and that this crate therefore cannot host it. **That is wrong and
is corrected here**, because it names the wrong missing piece and would send
a reader off to build an FE framework that is not required.

The integral above is **discretisation-agnostic**. What is FE-specific in
upstream is only that its quadrature happens to use element shape functions.
Finite volume is a viable host: OpenFOAM ships `solidDisplacement`, a
finite-volume segregated solver for linear-elastic small-strain deformation
with thermal stress, and [`crate::mechanics`] is already a port of it.

## What is actually missing, in either discretisation

1. **A crack front as data** — ordered points, curvilinear abscissae, and a
   per-point local basis. [`CrackTipBasis`] is the per-point piece; the
   ordered front is not.
2. **Quadrature over a ring domain** around that front.
3. **The `θ` field and its gradient** on that ring.

Only (1) and (2) are needed for a first working `G`.

## The genuine difficulty with finite volume

Recorded here so it is not rediscovered the hard way: the crack-tip field is
**singular**, `u ~ √r` and `σ ~ 1/√r`. FE handles that with quarter-point or
enriched elements. Cell-centred FV gradient reconstruction is typically first
or second order and **degrades precisely where the singularity is**. G-theta
is usable at all because of its domain-independence property, and that
property depends on an accurate `∇u` throughout the ring — so an FV G-theta
needs either a graded mesh near the front or an enrichment scheme. It is a
research-flavoured task rather than a transcription, and published FV
J-integral work should be consulted before starting. Whether cell-centred FV
can resolve the tip well enough **at all** is an open question, tracked as
bead `op-0xv` — it is not settled here, and nothing in this module should be
read as claiming it is.

`gbilin.F90` / `gbil3d.F90` are the natural first targets once quadrature
exists: `gbil3d` is entirely JEVEUX-free and `gbilin` needs only the material
lookup, both pure per-Gauss-point algebra. They were deliberately not ported
because their only meaningful test — that the ring integral reproduces a
known `G` — needs the quadrature first.

Rather than produce a module that looks like G-theta and computes nothing,
this file ports only the subset that is genuinely closed-form algebra.

# What *is* here (portable now, and verified)

| Item | Upstream | What it is |
|---|---|---|
| [`CrackPlaneState`], [`LinearElasticConstants`] | `chauxi.F90` (`ka`, `mu`) | Kolosov `kappa` and the effective modulus `E'`, per stress state |
| [`irwin_energy_release_rate`], [`ModeEnergyRelease`] | `calcG_type.F90::addValues`, `cakg2d.F90` | Irwin's `G <-> K` relation and the mixed-mode sum |
| [`westergaard_unit_field`], [`NearTipField`] | `chauxi.F90` | Williams/Westergaard near-tip displacement fields and their gradients |
| [`near_tip_stress`] | (Hooke applied to the above) | the singular stress field, used as the verification oracle |
| [`max_hoop_stress_kink_angle`] | `gkmet1.F90`, `gkmet3.F90` (commented out) | the Erdogan-Sih maximum-hoop-stress kink angle |
| [`CrackTipBasis`] | `cakg2d.F90` (the 90-degree rotation), `chauxi.F90` (the `invp` transform) | local crack-tip frame and local/global rotation |
| [`PlanarCrackTipResult`] | `cakg2d.F90` lines 471-493 | the symmetry and axisymmetric corrections applied to a summed 2-D result |
| [`legendre_front_mode`], [`legendre_front_mode_derivative`] | `plegen.F90`, `dplegen.F90` | the `L2`-orthonormal Legendre basis along the crack front |
| [`hat_smooth_front`] | `hatSmooth.F90` | quadratic-front hat smoothing of `G(s)` or `K(s)` |

Everything above is checked against a closed-form reference — the Williams
singular field, Irwin's relation on a centre-cracked infinite plate, the
Legendre three-term recurrence and orthonormality, and (for the kink angle) a
numerical stationary-point search on the hoop stress using this port's own
[`brent`](super::integration::brent) solver.

# What is blocked, and on what

Classified by reading all 72 files of `bibfor/fracture` at the pinned commit.
"JEVEUX-free" below means the file makes no call to upstream's memory manager
(`jeveuo`/`wkvect`/`jemarq`) and no call to the element driver (`calcul`).

**1. The G-theta domain integral itself — blocked on a solid-mechanics FE
framework.** `cgComputeGtheta.F90` (734 lines), `calcG_type.F90` (1953),
`cakg2d.F90` (537), `cakg3d.F90` (559), `mecalg.F90`, `mecagl.F90`. These are
drivers: they assemble field names, call `calcul` to run an element option
over a `LIGREL`, and sum the elementary results with `mesomm`. There is no
physics in them that survives removing the framework.

**2. The per-Gauss-point G-theta integrand — portable in form, blocked on
verification.** `gbilin.F90` (321 lines, 2-D) and `gbil3d.F90` (400 lines,
3-D) are the one real surprise: `gbil3d.F90` is entirely JEVEUX-free and
`gbilin.F90` touches it only through an unused `#include "jeveux.h"` (its
sole framework dependency is the material lookup `rcvalb`/`verift`). Both are
pure algebra
once you supply the four gradient matrices `dudm`, `dvdm`, `dtdm`, `dfdm` and
the elastic constants — they compute the classical term, the thermal term,
the body-force term, the dynamic term and three initial-stress terms, and
return a scalar. They are **deliberately not ported**: their only meaningful
test is that the ring integral of the kernel reproduces a known `G`, and that
test needs the quadrature this crate does not have. Porting them now would
add ~700 lines of untested transcription — exactly the "plausible-looking
module that computes nothing" this port is trying to avoid.

**3. Theta-field construction — blocked on mesh topology.** `gcour2.F90`
(436), `gcour3.F90` (366), `gcou2d.F90`, `gcharf.F90`, `gcharg.F90`,
`gcharm.F90`, `cgComputeLayers.F90`, `cgDiscrField.F90`, `thetapdg.F90`,
`xcourb.F90`. These build the virtual crack-extension field by walking rings
of elements outward from the crack front and interpolating a radial profile.
They need element connectivity, node coordinates, and a crack-front node
group.

**4. Crack-front smoothing systems — blocked on the front discretisation.**
`gkmet1.F90`, `gkmet3.F90`, `gmeth1/2/3.F90`, `gmatr1.F90`, `gmatr2.F90`,
`gmatc3.F90`, `gmate3.F90`, `gmatl3.F90`, `gsyste.F90`. The *basis functions*
these use are ported (see [`legendre_front_mode`]); the Gram matrices they
assemble are 1-D integrals over the crack-front segments, so they are blocked
only on having a crack front — a much smaller dependency than (1)-(3), and
the natural second phase.

**5. Command-language plumbing — out of scope permanently.** `cglect.F90`,
`cglecc.F90`, `cgleco.F90`, `cgcrio.F90`, `cgcrtb.F90`, `cgtyfi.F90`,
`cgvcmo.F90`, `cgvein.F90`, `cgvemf.F90`, `cgverc.F90`, `cgverho.F90`,
`cgVerification.F90`, `cgReadCompor.F90`, `cgComporNodes.F90`,
`cgTempNodes.F90`, `cgCreateCompIncr.F90`, `cgExportTableG.F90`,
`gcsele.F90`, `gcfonc.F90`, `gcchar.F90`, `gchfus.F90`,
`gchs2f.F90`, `medomg.F90`, `mefor0.F90`, `mepres.F90`, `gverfo.F90`,
`gver2d.F90`, `gveri3.F90`, `gverlc.F90`, `foninf2.F90`, `gimpgs.F90`,
`gksimp.F90`. Keyword parsing, `.comm` deck validation, JEVEUX table writing
and formatted printing. These reproduce code_aster's *user interface*, not
its physics; this workspace has its own.

**6. Elastoplastic free energy for `G` — blocked on the material catalogue.**
`nmplru.F90` (216 lines) computes the free-energy density and its temperature
derivative for the plastic `G`. Its algebra is portable, but it is driven by
upstream's tabulated traction curve (`rctrac`/`rcfonc`) and material-field
lookups. Deferred to the point where this port has a hardening-curve
abstraction.

# The smallest FE capability that would unblock the rest

In dependency order, smallest first:

1. **A crack front as data** — an ordered list of front points with
   curvilinear abscissae `s` and a local basis per point. This alone unblocks
   group (4): the Legendre and Lagrange smoothing systems (`gmatr1`,
   `gmatr2`, `gsyste`) become 1-D quadratures over front segments, needing
   only `SE2`/`SE3` shape functions and a Gauss rule on `[-1, 1]`.
2. **Gauss quadrature plus isoparametric shape functions and Jacobians on
   solid elements** (upstream's `elrfvf`/`elrfdf`/`nmgeom`), together with a
   displacement field sampled at Gauss points. With that, `gbilin`/`gbil3d`
   become both portable *and* testable, because the ring integral of the
   kernel over a Westergaard displacement field must return `K^2 / E'`.
3. **Element-ring topology around the front** — "give me the elements whose
   distance from the front is in `[R_inf, R_sup]`". That unblocks group (3),
   the theta field.

Only (1) and (2) are needed for a first working `G`. Item (2) is the real
cost, and it is a general finite-element capability, not a fracture one.

# Provenance and honesty notes

- The read-only upstream clone used here is a **partial** one: it carries
  `bibfor/{fracture,comport,comport_prep,lc,metallurgy,algorith,nonlinear,
  modelisa,utilitai,utilifor,include}` but **not** `bibfor/te` (the element
  routines), `catalo/`, `code_aster/Commands/` or `astest/`. Consequences:
  the element routine that fills `chauxi`'s `ka` argument per modelisation is
  **not visible**, and neither is the regression suite. So the mapping from
  `D_PLAN` / `C_PLAN` / `AXIS` to Kolosov's `kappa` in [`CrackPlaneState`] is
  taken from the standard result (Kolosov/Muskhelishvili), corroborated by
  upstream's own inverse `nu = (3 - ka)/4` in `chauxi.F90`, and **verified
  here** by checking that the near-tip stress it produces is the same
  `1/sqrt(2 pi r)` singularity in plane strain and in plane stress — a check
  that fails if `kappa` and the plane Lame constant are mismatched.
- The kink-angle formula in [`max_hoop_stress_kink_angle`] exists upstream
  **only as commented-out code** in `gkmet1.F90` and `gkmet3.F90`. It is
  ported because it is the criterion those files were reaching for and it has
  a clean analytical reference, but a reader should know it is not live
  upstream.
- **Paris-law fatigue crack growth is not ported, because it is not in the
  upstream source.** A search of the whole clone for `PARIS`, `LOI_PROPA`,
  `DELTA_K_SEUIL` and `PROPA_FISS` returns nothing in any Fortran or Python
  file. Upstream's crack-advance law lives in the command layer, which this
  clone does not carry. Writing one here would have been invention dressed as
  a port.

# Units

Raw `f64` throughout, matching the rest of [`super`]. SI: lengths in metres,
stresses and moduli in pascals, energy release rate `G` in J/m^2 (= Pa m),
stress intensity factors in Pa m^(1/2), angles in radians. Poisson's ratio,
Kolosov's `kappa` and the Legendre abscissa ratio are dimensionless.

```rust
pub mod fracture { /* ... */ }
```

### Types

#### Enum `CrackPlaneState`

Which two-dimensional idealisation the crack-tip field is evaluated under.

# Why this is a separate type and not a boolean

The plane state changes *two* constants at once — Kolosov's `kappa` and the
effective modulus `E'` — and getting one right while getting the other wrong
produces a near-tip field that still looks singular and is still smooth, but
carries the wrong energy. Requiring the caller to name the state at every
entry point is deliberate.

# The mapping

| Variant | code_aster `MODELISATION` | `kappa` | `E'` |
|---|---|---|---|
| [`PlaneStrain`](Self::PlaneStrain) | `D_PLAN` | `3 - 4 nu` | `E / (1 - nu^2)` |
| [`PlaneStress`](Self::PlaneStress) | `C_PLAN` | `(3 - nu) / (1 + nu)` | `E` |
| [`Axisymmetric`](Self::Axisymmetric) | `AXIS` | `3 - 4 nu` | `E / (1 - nu^2)` |
| [`ThreeDimensional`](Self::ThreeDimensional) | `3D` | `3 - 4 nu` | `E / (1 - nu^2)` |

Axisymmetric and three-dimensional both behave as plane strain here, and for
the same reason: a crack front constrains the material in the plane normal to
itself, so the asymptotic field in that plane is a plane-strain one. That is
an *asymptotic* statement about the crack-front neighbourhood, not a claim
that the whole body is in plane strain. Near a free surface, where the true
state relaxes towards plane stress, it is wrong — a limitation, documented
rather than hidden.

# Units

Dimensionless selector. `kappa` is dimensionless; `E'` is in pascals.

```rust
pub enum CrackPlaneState {
    PlaneStrain,
    PlaneStress,
    Axisymmetric,
    ThreeDimensional,
}
```

##### Variants

###### `PlaneStrain`

Plane strain (`D_PLAN`): the out-of-plane strain vanishes, the
out-of-plane stress does not.

###### `PlaneStress`

Plane stress (`C_PLAN`): the out-of-plane stress vanishes, the
out-of-plane strain does not. The thin-sheet idealisation.

###### `Axisymmetric`

Axisymmetric (`AXIS`): a circumferential crack in a body of revolution.
Plane-strain-like in the meridian plane.

###### `ThreeDimensional`

Fully three-dimensional (`3D`), evaluated in the plane normal to the
crack front. Plane-strain-like there.

##### Implementations

###### Methods

- ```rust
  pub const fn aster_name(self: Self) -> &'static str { /* ... */ }
  ```
  The upstream `MODELISATION` token this corresponds to.

- ```rust
  pub const fn is_plane_strain_like(self: Self) -> bool { /* ... */ }
  ```
  Whether the state behaves as plane strain for `kappa` and `E'`.

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
    fn clone(self: &Self) -> CrackPlaneState { /* ... */ }
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
    fn eq(self: &Self, other: &CrackPlaneState) -> bool { /* ... */ }
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
#### Struct `LinearElasticConstants`

Isotropic linear-elastic constants at the crack tip.

# What it holds

Young's modulus `E` in pascals and Poisson's ratio `nu`, dimensionless.
Everything the near-tip field needs — the shear modulus `mu`, the plane Lame
constant, Kolosov's `kappa`, the effective modulus `E'` — is derived from
these two, so there is no way for a caller to supply an inconsistent set.

# Valid range

`E > 0` and `-1 < nu < 0.5`. The upper bound excludes incompressibility,
where the plane-strain Lame constant diverges; the lower bound is the
thermodynamic limit for an isotropic solid. Both are enforced by
[`new`](Self::new).

# Units

`young` in pascals (Pa); `poisson` dimensionless.

```rust
pub struct LinearElasticConstants {
    pub young: f64,
    pub poisson: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `young` | `f64` | Young's modulus `E`, in pascals. |
| `poisson` | `f64` | Poisson's ratio `nu`, dimensionless. |

##### Implementations

###### Methods

- ```rust
  pub fn new(young: f64, poisson: f64) -> Result<Self> { /* ... */ }
  ```
  Build and validate a pair of isotropic elastic constants.

- ```rust
  pub fn shear_modulus(self: Self) -> f64 { /* ... */ }
  ```
  The shear modulus `mu = E / (2 (1 + nu))`, in pascals.

- ```rust
  pub fn kolosov_kappa(self: Self, state: CrackPlaneState) -> f64 { /* ... */ }
  ```
  Kolosov's constant `kappa`, dimensionless.

- ```rust
  pub fn effective_modulus(self: Self, state: CrackPlaneState) -> f64 { /* ... */ }
  ```
  The effective modulus `E'` appearing in Irwin's relation, in pascals.

- ```rust
  pub fn plane_lame_lambda(self: Self, state: CrackPlaneState) -> f64 { /* ... */ }
  ```
  The Lame constant `lambda` to use in the **two-dimensional** Hooke's law

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
    fn clone(self: &Self) -> LinearElasticConstants { /* ... */ }
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
    fn eq(self: &Self, other: &LinearElasticConstants) -> bool { /* ... */ }
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
#### Struct `StressIntensityFactors`

The three stress intensity factors at a point on the crack front.

# What they mean

The amplitudes of the three independent singular modes of the Williams
expansion: opening (`I`), in-plane shear (`II`) and anti-plane shear (`III`).
Each multiplies a universal angular field, so a single number per mode
characterises the whole near-tip state.

# Units

Pa m^(1/2) — pascals times the square root of a metre. In two dimensions
`k3` is identically zero and is carried only so one type serves both cases.

```rust
pub struct StressIntensityFactors {
    pub k1: f64,
    pub k2: f64,
    pub k3: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `k1` | `f64` | Mode I (opening), in Pa m^(1/2). |
| `k2` | `f64` | Mode II (in-plane sliding shear), in Pa m^(1/2). |
| `k3` | `f64` | Mode III (anti-plane tearing shear), in Pa m^(1/2). Zero in 2-D. |

##### Implementations

###### Methods

- ```rust
  pub const fn mode_i(k1: f64) -> Self { /* ... */ }
  ```
  A pure mode-I state of amplitude `k1` (Pa m^(1/2)).

- ```rust
  pub const fn new(k1: f64, k2: f64, k3: f64) -> Self { /* ... */ }
  ```
  A general state. All three amplitudes in Pa m^(1/2).

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
    fn clone(self: &Self) -> StressIntensityFactors { /* ... */ }
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
    fn default() -> StressIntensityFactors { /* ... */ }
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
    fn eq(self: &Self, other: &StressIntensityFactors) -> bool { /* ... */ }
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
#### Struct `ModeEnergyRelease`

The energy release rate split by mode, plus the total.

# What it means

Under linear elasticity the three modes contribute *additively* to the energy
release rate — there is no cross term, because the three angular fields are
orthogonal over a circuit around the tip. That additivity is exactly what
upstream relies on when it forms `G_IRWIN` as a sum of squares in
`calcG_type.F90::addValues` and `gkmet1.F90`.

# Units

All four fields in J/m^2 (equivalently Pa m).

```rust
pub struct ModeEnergyRelease {
    pub mode_i: f64,
    pub mode_ii: f64,
    pub mode_iii: f64,
    pub total: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mode_i` | `f64` | Mode-I contribution `K_I^2 / E'`, in J/m^2. |
| `mode_ii` | `f64` | Mode-II contribution `K_II^2 / E'`, in J/m^2. |
| `mode_iii` | `f64` | Mode-III contribution `K_III^2 / (2 mu)`, in J/m^2. |
| `total` | `f64` | The sum of the three, in J/m^2. |

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
    fn clone(self: &Self) -> ModeEnergyRelease { /* ... */ }
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
    fn default() -> ModeEnergyRelease { /* ... */ }
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
    fn eq(self: &Self, other: &ModeEnergyRelease) -> bool { /* ... */ }
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
#### Enum `CrackOpeningMode`

Which singular crack-tip mode a near-tip field belongs to.

Enum dispatch, not trait objects, per the workspace rule: the set of
crack-opening modes is closed by elasticity itself and cannot grow.

```rust
pub enum CrackOpeningMode {
    Opening,
    InPlaneShear,
    AntiPlaneShear,
}
```

##### Variants

###### `Opening`

Mode I — opening. Upstream's auxiliary field `u1`.

###### `InPlaneShear`

Mode II — in-plane sliding shear. Upstream's `u2`.

###### `AntiPlaneShear`

Mode III — anti-plane tearing shear. Upstream's `u3`.

##### Implementations

###### Methods

- ```rust
  pub const fn number(self: Self) -> usize { /* ... */ }
  ```
  The mode number, 1, 2 or 3, matching the `K1`/`K2`/`K3` table columns

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
    fn clone(self: &Self) -> CrackOpeningMode { /* ... */ }
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
    fn eq(self: &Self, other: &CrackOpeningMode) -> bool { /* ... */ }
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
#### Struct `NearTipField`

A near-tip displacement field and its gradient, in the local crack-tip basis.

# Frame

Both members are expressed in the **local crack-tip basis**: `x` along the
crack-propagation direction (ahead of the tip), `y` normal to the crack
plane, `z` along the crack front. Use [`CrackTipBasis`] to rotate into the
global frame.

# Units

`displacement` in metres per unit stress intensity factor, i.e. m /
(Pa m^(1/2)) = m^(1/2) / Pa. `gradient` is that per metre, i.e.
1 / (Pa m^(1/2)). Multiply by a `K` in Pa m^(1/2) with
[`scaled`](Self::scaled) to get metres and a dimensionless gradient.

```rust
pub struct NearTipField {
    pub displacement: outram_foam_basic_lib::primitives::Vector3,
    pub gradient: outram_foam_basic_lib::primitives::Tensor,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `displacement` | `outram_foam_basic_lib::primitives::Vector3` | Displacement `u`, local basis. |
| `gradient` | `outram_foam_basic_lib::primitives::Tensor` | Displacement gradient `du_i / dx_j` (row `i`, column `j`), local basis. |

##### Implementations

###### Methods

- ```rust
  pub fn scaled(self: Self, k: f64) -> Self { /* ... */ }
  ```
  Scale a unit-`K` field by an actual stress intensity factor.

- ```rust
  pub fn small_strain(self: Self) -> SymmTensor { /* ... */ }
  ```
  The small-strain tensor `eps = (grad u + grad u^T) / 2` of this field.

- ```rust
  pub fn small_strain_mandel(self: Self) -> AsterVoigt { /* ... */ }
  ```
  The small-strain tensor in code_aster's Mandel six-vector convention.

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
    fn clone(self: &Self) -> NearTipField { /* ... */ }
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
    fn eq(self: &Self, other: &NearTipField) -> bool { /* ... */ }
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
#### Struct `CrackTipBasis`

An orthonormal frame attached to a point on the crack front.

# The three directions

- **`x` — propagation.** The direction the crack would extend in, lying in
  the crack plane, normal to the front.
- **`y` — normal.** Normal to the crack plane; the direction the faces
  separate in under mode I.
- **`z` — tangent.** Tangent to the crack front. Degenerate in 2-D, where it
  is the out-of-plane axis.

This is the frame [`westergaard_unit_field`] returns its fields in, and the
frame upstream's `chauxi.F90` calls "the local basis".

# Relation to upstream

In two dimensions `cakg2d.F90` builds it from a single stored vector, with a
comment worth preserving: *"ATTENTION, ON NE SE SERT PAS DU VECTEUR NORMAL DE
BASEFOND MAIS ON FAIT TOURNER DE 90 DEGRES LE VECTEUR DE PROPA"* — it does
**not** use the stored normal, it rotates the propagation vector by 90
degrees. That is what [`from_propagation_direction_2d`](Self::from_propagation_direction_2d)
reproduces, including the specific rotation sense
`(n_x, n_y) = (-t_y, t_x)` from lines 267-279.

In three dimensions `chauxi.F90` rotates the field with `invp`, the inverse
passage matrix, as `du_global(i,j) = sum_kl invp(k,i) du_local(k,l) invp(l,j)`
— which for an orthonormal frame is exactly `P G P^T` with `P` the
local-to-global rotation. [`local_to_global_gradient`](Self::local_to_global_gradient)
is that expression.

# Units

Dimensionless. All three stored vectors are unit vectors.

```rust
pub struct CrackTipBasis {
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
  pub fn from_propagation_direction_2d(direction: Vector3) -> Result<Self> { /* ... */ }
  ```
  Build a two-dimensional crack-tip frame from the propagation direction.

- ```rust
  pub fn from_front_tangent_and_normal(tangent: Vector3, normal: Vector3) -> Result<Self> { /* ... */ }
  ```
  Build a three-dimensional crack-tip frame from the front tangent and the

- ```rust
  pub fn propagation_direction(self: Self) -> Vector3 { /* ... */ }
  ```
  The crack-propagation direction, a unit vector in global coordinates.

- ```rust
  pub fn crack_plane_normal(self: Self) -> Vector3 { /* ... */ }
  ```
  The crack-plane normal, a unit vector in global coordinates.

- ```rust
  pub fn front_tangent(self: Self) -> Vector3 { /* ... */ }
  ```
  The crack-front tangent, a unit vector in global coordinates.

- ```rust
  pub fn local_to_global_vector(self: Self, v: Vector3) -> Vector3 { /* ... */ }
  ```
  Rotate a vector from the local crack-tip frame into global coordinates.

- ```rust
  pub fn global_to_local_vector(self: Self, v: Vector3) -> Vector3 { /* ... */ }
  ```
  Rotate a vector from global coordinates into the local crack-tip frame.

- ```rust
  pub fn local_to_global_gradient(self: Self, g: Tensor) -> Tensor { /* ... */ }
  ```
  Rotate a second-order tensor (a displacement gradient, a stress) from the

- ```rust
  pub fn global_to_local_gradient(self: Self, g: Tensor) -> Tensor { /* ... */ }
  ```
  Rotate a second-order tensor from global coordinates into the local

- ```rust
  pub fn field_to_global(self: Self, field: NearTipField) -> NearTipField { /* ... */ }
  ```
  Rotate a whole near-tip field into global coordinates.

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
    fn clone(self: &Self) -> CrackTipBasis { /* ... */ }
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
    fn eq(self: &Self, other: &CrackTipBasis) -> bool { /* ... */ }
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
#### Struct `PlanarCrackTipResult`

The five quantities upstream's two-dimensional `CALC_K_G` sums out of the
element loop, and the corrections applied to them afterwards.

# What the five are

`cakg2d.F90` calls `mesomm` to sum five elementary values (`fic(1..5)`) into
`valg(1..5)`:

| Slot | Meaning |
|---|---|
| `valg(1)` | `G`, the energy release rate from the domain integral |
| `valg(2)` | the mode-I *Irwin root*, `K_I / sqrt(E')` |
| `valg(3)` | the mode-II Irwin root, `K_II / sqrt(E')` |
| `valg(4)` | `K_I` from the interaction integral |
| `valg(5)` | `K_II` from the interaction integral |

and then forms `G_IRWIN = valg(2)^2 + valg(3)^2` (line 493) — the same
construction `calcG_type.F90` line 1599 uses in 3-D with three modes.

**Only the post-processing is ported.** Producing the five numbers is the
blocked FE work; this type is what you do with them once you have them, and
it is genuinely free of any mesh dependency.

# Why keep the roots separate from `K_I`, `K_II`

Because `G` and `G_IRWIN` are computed by different routes — the direct
domain integral and the interaction integral respectively — and their
*disagreement* is the standard diagnostic for an under-resolved ring. Folding
them together would throw that away, and upstream deliberately reports both.

# Units

`g` and `g_irwin` in J/m^2; `k1`, `k2` in Pa m^(1/2); the Irwin roots in
(J/m^2)^(1/2) = Pa^(1/2) m^(1/2)... more usefully, `K / sqrt(E')`, whose
square is an energy release rate.

```rust
pub struct PlanarCrackTipResult {
    pub g: f64,
    pub mode_i_root: f64,
    pub mode_ii_root: f64,
    pub k1: f64,
    pub k2: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `g` | `f64` | `valg(1)` — energy release rate from the domain integral, J/m^2. |
| `mode_i_root` | `f64` | `valg(2)` — mode-I Irwin root `K_I / sqrt(E')`. |
| `mode_ii_root` | `f64` | `valg(3)` — mode-II Irwin root `K_II / sqrt(E')`. |
| `k1` | `f64` | `valg(4)` — mode-I stress intensity factor, Pa m^(1/2). |
| `k2` | `f64` | `valg(5)` — mode-II stress intensity factor, Pa m^(1/2). |

##### Implementations

###### Methods

- ```rust
  pub fn g_irwin(self: Self) -> f64 { /* ... */ }
  ```
  `G_IRWIN`, the energy release rate reconstructed from the per-mode roots.

- ```rust
  pub fn with_symmetric_half_model(self: Self) -> Self { /* ... */ }
  ```
  Apply upstream's `SYME = 'OUI'` correction for a symmetric half model.

- ```rust
  pub fn with_axisymmetric_normalisation(self: Self, r_tip: f64) -> Result<Self> { /* ... */ }
  ```
  Apply upstream's axisymmetric normalisation by the crack-tip radius.

- ```rust
  pub fn stress_intensity_factors(self: Self) -> StressIntensityFactors { /* ... */ }
  ```
  The stress intensity factors as a [`StressIntensityFactors`], with

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
    fn clone(self: &Self) -> PlanarCrackTipResult { /* ... */ }
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
    fn default() -> PlanarCrackTipResult { /* ... */ }
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
    fn eq(self: &Self, other: &PlanarCrackTipResult) -> bool { /* ... */ }
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

#### Function `irwin_mode_split`

**Attributes:**

- `MustUse { reason: None }`

Split the energy release rate by mode via Irwin's relation.

# What it computes

```text
G_I   = K_I^2   / E'
G_II  = K_II^2  / E'
G_III = K_III^2 / (2 mu)
G     = G_I + G_II + G_III
```

`E'` is [`LinearElasticConstants::effective_modulus`] — `E / (1 - nu^2)` in
plane strain (and 3-D at the front), `E` in plane stress. The mode-III factor
is `(1 + nu) / E = 1 / (2 mu)` and is **the same in every plane state**,
because anti-plane shear is a scalar Laplace problem that never sees the
in-plane constraint. Applying the plane-state factor to mode III is a
plausible-looking error worth naming.

# Assumptions

Linear elastic, isotropic, homogeneous material; small-scale yielding; a
straight crack front with a self-similar advance. Outside those, `G` from
`K` and `G` from the domain integral part company, and the difference is
itself diagnostic — which is why upstream reports both `G` and `G_IRWIN` and
leaves the comparison to the user.

# Units

`k` in Pa m^(1/2), `elastic.young` in Pa; the result in J/m^2.

```rust
pub fn irwin_mode_split(k: StressIntensityFactors, elastic: LinearElasticConstants, state: CrackPlaneState) -> ModeEnergyRelease { /* ... */ }
```

#### Function `irwin_energy_release_rate`

**Attributes:**

- `MustUse { reason: None }`

The total energy release rate from the three stress intensity factors.

A convenience over [`irwin_mode_split`] when only the total is wanted. This
is the quantity upstream tabulates as `G_IRWIN`, formed there as a sum of
squares of per-mode roots (`calcG_type.F90` line 1599, `cakg2d.F90` line
493).

# Units

`k` in Pa m^(1/2), result in J/m^2. See [`irwin_mode_split`] for the
assumptions.

```rust
pub fn irwin_energy_release_rate(k: StressIntensityFactors, elastic: LinearElasticConstants, state: CrackPlaneState) -> f64 { /* ... */ }
```

#### Function `equivalent_mode_i_factor`

**Attributes:**

- `MustUse { reason: None }`

The equivalent mode-I stress intensity factor of an energy release rate.

# What it computes

`K_eq = sqrt(G E')`. This is upstream's `KJ` output, formed in
`calcG_type.F90::addValues` as `sqrt(gth(2))` after the element has already
multiplied by `E'`. It answers: *what pure mode-I loading would release
energy at this rate?* — the standard way to compare a mixed-mode or
elastic-plastic result against a mode-I toughness `K_Ic`.

A negative `G` is returned as zero, matching upstream's guard
(`if (gth(2) >= 0) ... else 0`). A negative energy release rate is not
physical; it arises numerically when the domain integral is evaluated on a
ring too small or too distorted to resolve the field, and upstream chose to
clip rather than fail. That behaviour is **reproduced, not corrected**.

# Units

`g` in J/m^2, `elastic.young` in Pa; result in Pa m^(1/2).

```rust
pub fn equivalent_mode_i_factor(g: f64, elastic: LinearElasticConstants, state: CrackPlaneState) -> f64 { /* ... */ }
```

#### Function `westergaard_unit_field`

The unit-`K` near-tip displacement field and its gradient — a port of
`chauxi.F90`.

# What it computes

The leading (`r^(1/2)`) term of the Williams expansion, normalised so that
the field corresponds to a stress intensity factor of exactly 1 Pa m^(1/2)
in the requested mode. With `mu` the shear modulus and `kappa` Kolosov's
constant:

`u_x = (1 / (2 mu)) sqrt(r / (2 pi)) cos(t/2) (kappa - cos t)` (mode I)

`u_y = (1 / (2 mu)) sqrt(r / (2 pi)) sin(t/2) (kappa - cos t)` (mode I)

`u_x = (1 / (2 mu)) sqrt(r / (2 pi)) sin(t/2) (kappa + 2 + cos t)` (mode II)

`u_y = (1 / (2 mu)) sqrt(r / (2 pi)) cos(t/2) (2 - kappa - cos t)` (mode II)

`u_z = (2 / mu) sqrt(r / (2 pi)) sin(t/2)` (mode III)

transcribed from upstream's `u1l`, `u2l`, `u3l` with upstream's own
coefficients `cr1 = 1/(4 mu sqrt(2 pi r))` and `cr2 = sqrt(r/(2 pi))/(2 mu)`.
The gradient is upstream's `du#dl`: the polar derivatives converted to local
Cartesian components by
`d/dx = cos(t) d/dr - (sin(t)/r) d/dt`, `d/dy = sin(t) d/dr + (cos(t)/r) d/dt`.

# Coordinates

`r` is the distance from the crack tip in metres, strictly positive — the
field is singular at `r = 0` and that is the point of it. `theta` is the
angle in radians measured from the crack-propagation direction, with the
crack faces at `theta = +/- pi`. The field is *not* periodic in `theta`: it
changes sign across `theta = pi`, which is the branch cut representing the
crack itself, so passing `theta` outside `[-pi, pi]` is meaningless and
rejected.

# What is deliberately not ported

Upstream's optional `r_courb` argument adds a higher-order correction for a
*curved* crack front (the `A1..D1`, `A2..D2` coefficient block). It is left
out: it is a `O(r^(3/2))` correction with no closed-form reference available
in this clone to verify it against, and transcribing 60 lines of untested
trigonometry would add risk without adding capability. The straight-front
leading term is exact and is what the verification below pins.

# Errors

[`OffbeatError::Unphysical`] if `r <= 0` (the field is singular there) or if
`theta` is outside `[-pi, pi]`.

# Units

`r` in metres, `theta` in radians. The returned displacement is in
m^(1/2)/Pa and the gradient in 1/(Pa m^(1/2)) — per unit `K`. Multiply by a
`K` in Pa m^(1/2) with [`NearTipField::scaled`].

```rust
pub fn westergaard_unit_field(mode: CrackOpeningMode, r: f64, theta: f64, elastic: LinearElasticConstants, state: CrackPlaneState) -> crate::error::Result<NearTipField> { /* ... */ }
```

#### Function `near_tip_stress`

**Attributes:**

- `MustUse { reason: None }`

The Cauchy stress of a near-tip field, by isotropic Hooke's law.

# What it computes

`sigma_ij = lambda_plane tr(eps_2D) delta_ij + 2 mu eps_ij` for the in-plane
components, with `lambda_plane` from
[`LinearElasticConstants::plane_lame_lambda`]. The out-of-plane components
follow the plane state:

- plane strain (and axisymmetric, and 3-D at the front):
  `sigma_zz = lambda tr(eps_2D)`, the reaction that enforces `eps_zz = 0`;
- plane stress: `sigma_zz = 0` by definition.

The anti-plane shears `sigma_xz` and `sigma_yz` are `2 mu eps_xz` and
`2 mu eps_yz` in every state, because mode III does not couple to the
in-plane constraint.

# Why this exists

Not because a caller needs it — because it is the *verification oracle*.
Applying Hooke to the displacement field must return the Williams singular
stress `sigma_yy = K_I / sqrt(2 pi r)` on the crack plane ahead of the tip,
and that value is **independent of the plane state**. So the test that
`near_tip_stress` gives the same singularity in plane strain and plane stress
is a direct check that `kappa` and `lambda_plane` are mutually consistent —
the check the missing element routines would otherwise have to supply.

# Units

Input field per unit `K` gives a stress in 1/m^(1/2) — multiply the field by
a `K` in Pa m^(1/2) first (via [`NearTipField::scaled`]) to get pascals.

```rust
pub fn near_tip_stress(field: NearTipField, elastic: LinearElasticConstants, state: CrackPlaneState) -> outram_foam_basic_lib::primitives::SymmTensor { /* ... */ }
```

#### Function `max_hoop_stress_kink_angle`

The crack-kink angle predicted by the maximum-hoop-stress criterion.

# What it computes

The angle `theta_c`, in radians, at which the near-tip hoop stress
`sigma_theta_theta` is stationary and maximal, which the Erdogan-Sih
criterion takes as the direction a crack turns to under mixed mode I/II
loading. The stationarity condition is

`K_I sin(theta) + K_II (3 cos(theta) - 1) = 0`

whose relevant root, written with `t = tan(theta_c / 2)`, is

`t = (K_I - sqrt(K_I^2 + 8 K_II^2)) / (4 K_II)`.

# Relation to upstream

That expression is exactly what `gkmet1.F90` and `gkmet3.F90` carry — **as
commented-out code**, guarded by `abs(K_II) >= 1e-12`:

```text
betas(i) = 2*atan2(0.25*(k1s/k2s - sign(1,k2s)*sqrt((k1s/k2s)**2 + 8)), 1)
```

It is not live in the pinned commit, so this is a port of an *inactive*
upstream expression plus its published source, not of running upstream
behaviour. A reader should weigh it accordingly.

# Difference from upstream's form, and why

Upstream evaluates `K_I/K_II - sign(K_II) sqrt((K_I/K_II)^2 + 8)`, a
difference of two nearly equal large numbers whenever `K_II << K_I` — which
is the near-mode-I regime most calculations sit in. It therefore needs the
`1e-12` guard *and* loses several significant figures well before the guard
fires. This port picks whichever of the two algebraically identical forms is
cancellation-free for the sign of `K_I`:

- `K_I >= 0`: `t = -2 K_II / (K_I + sqrt(K_I^2 + 8 K_II^2))` — both terms in
  the denominator are non-negative, and there is no division by `K_II` at
  all, so pure mode I returns exactly zero with no guard.
- `K_I < 0`: `t = (K_I - sqrt(K_I^2 + 8 K_II^2)) / (4 K_II)` — both terms in
  the numerator are negative. This branch does divide by `K_II`, which is why
  `K_II = 0` with `K_I < 0` is the one rejected case.

Multiplying the first form's numerator and denominator by
`K_I + sqrt(K_I^2 + 8 K_II^2)` recovers the second, so this is a numerical
restatement, not a change of behaviour. The tests below sweep the two against
upstream's literal expression and against an independent numerical
stationary-point search, and quantify how much accuracy upstream's form
loses as `K_II` shrinks.

# Sign convention

`theta_c` is measured from the crack-propagation direction, positive
anticlockwise, in `(-pi, pi)`. A positive `K_II` turns the crack **clockwise**
(negative angle) — this is the standard convention and the one upstream's
expression produces.

# Assumptions and limits

Linear elastic, small-scale yielding, mode I/II only. Mode III is ignored:
the maximum-hoop-stress criterion has no accepted three-dimensional
extension, and pretending otherwise would be worse than declining. The
criterion is also a *local* one — it says nothing about whether the crack
grows, only about which way it would turn if it did.

# Errors

[`OffbeatError::Unphysical`] if `k2 == 0` and `k1 <= 0`. That is a closed
crack under pure compression, where the hoop stress is nowhere tensile, has
no maximum, and the criterion does not apply. `k1 = k2 = 0` is rejected by
the same test. Every other input is admissible.

# Units

`k1`, `k2` in Pa m^(1/2) (only their ratio matters); result in radians.

```rust
pub fn max_hoop_stress_kink_angle(k1: f64, k2: f64) -> crate::error::Result<f64> { /* ... */ }
```

#### Function `scaled_hoop_stress`

**Attributes:**

- `MustUse { reason: None }`

The near-tip hoop stress `sigma_theta_theta` times `sqrt(2 pi r)`.

# What it computes

The angular part of the mode I/II hoop stress,

`sqrt(2 pi r) sigma_tt = cos(theta/2) [ K_I cos^2(theta/2) - (3/2) K_II sin(theta) ]`

stripped of its `1/sqrt(r)` singularity so it can be compared at a fixed
radius. This is the function [`max_hoop_stress_kink_angle`] maximises, and it
is exposed so a caller can check that claim rather than take it on trust.

# Units

`k1`, `k2` in Pa m^(1/2), `theta` in radians; result in Pa m^(1/2). Divide by
`sqrt(2 pi r)` for a stress in pascals at radius `r` metres.

```rust
pub fn scaled_hoop_stress(k1: f64, k2: f64, theta: f64) -> f64 { /* ... */ }
```

#### Function `legendre_front_mode`

The `L2`-orthonormal Legendre basis function along the crack front.

# What it computes

`phi_n(s) = sqrt((2n + 1) / L) P_n(xi)` with `xi = 2 s / L - 1`, where `P_n`
is the standard Legendre polynomial and `L` the crack-front length. Ported
from `plegen.F90`.

# What it is for

The G-theta method does not compute `G(s)` pointwise along a
three-dimensional crack front; it computes the projections
`<G, theta_i>` of `G` onto a family of virtual extension fields, then solves
a small linear system for the coefficients of `G(s)` in the same basis. When
`LISSAGE = 'LEGENDRE'`, this is that basis. The *assembly and solve* are
blocked on having a crack front (see the module documentation, group 4); the
basis itself is closed-form and is here.

# Why the normalisation matters

The `sqrt((2n + 1) / L)` factor makes the family orthonormal in `L2(0, L)`:
`integral_0^L phi_m phi_n ds = delta_mn`. That is what makes the Gram matrix
upstream assembles well-conditioned — without it the system degrades rapidly
with degree. It is verified below rather than asserted.

# Errors

[`OffbeatError::Unphysical`] if `front_length <= 0`;
[`OffbeatError::NotImplemented`] if `degree` exceeds
[`MAX_LEGENDRE_FRONT_DEGREE`], matching upstream's assertion.

# Units

`s` and `front_length` in metres, both measured along the front, with `s`
expected in `[0, L]` (values outside are evaluated by extrapolation, as
upstream does, without complaint). The result has units of m^(-1/2), so that
a coefficient times the basis function integrates to a length-independent
quantity.

```rust
pub fn legendre_front_mode(degree: usize, s: f64, front_length: f64) -> crate::error::Result<f64> { /* ... */ }
```

#### Function `legendre_front_mode_derivative`

The derivative with respect to arc length of [`legendre_front_mode`].

# What it computes

`d phi_n / ds = (2 / L) sqrt((2n + 1) / L) P_n'(xi)`, the chain rule applied
through `xi = 2 s / L - 1`. Ported from `dplegen.F90`, whose `coef` is
exactly that `(2/L) sqrt((2n+1)/L)` prefactor.

Needed because the virtual extension field's *gradient* enters the G-theta
bilinear form, not only its value.

# Errors

As [`legendre_front_mode`].

# Units

`s`, `front_length` in metres; result in m^(-3/2).

```rust
pub fn legendre_front_mode_derivative(degree: usize, s: f64, front_length: f64) -> crate::error::Result<f64> { /* ... */ }
```

#### Function `hat_smooth_front`

Smooth a nodal `G(s)` or `K(s)` along a quadratic crack front — a port of
`hatSmooth.F90`.

# What it computes

Given values at the `2 m - 1` nodes of a chain of `m - 1` three-node
(quadratic) front segments, it replaces them with a hat-function-weighted
average. Corner-node values become

- first: `(2 v_0 + v_1) / 3`
- interior `i`: `(lg_i v_{2i-1} + v_{2i} + ld_i v_{2i+1}) / 3`, with
  `lg_i = 2 le_i / (le_i + le_{i+1})` and `ld_i = 2 le_{i+1} / (le_i + le_{i+1})`
  where `le` are the corner-to-corner segment lengths
- last: `(v_{n-2} + 2 v_{n-1}) / 3`

and mid-side values become the mean of their two neighbouring smoothed corner
values.

# Why it exists

The raw per-node `G` from a G-theta calculation on a quadratic front
oscillates between corner and mid-side nodes — an artefact of the quadratic
interpolation, not physics. This is the fixed three-point filter upstream
applies to remove it.

# Properties, and one limitation worth stating

`lg_i + ld_i = 2` identically, so the interior stencil reproduces a constant
exactly. The **end** stencils do not reproduce a linear function: `(2 v_0 +
v_1)/3` is biased towards the interior by one third of the end slope. That is
upstream's behaviour and it is reproduced, not corrected — but a user reading
a smoothed `G(s)` should expect the two end values to be pulled inward.

# Errors

[`OffbeatError::Mesh`] if `abscissae` and `values` differ in length, if the
length is even, or if it is below 3 — a quadratic front needs an odd node
count of at least one segment.

# Units

`abscissae` are curvilinear abscissae along the front in metres; `values`
carry whatever the smoothed quantity does (J/m^2 for `G`, Pa m^(1/2) for
`K`) and are modified in place.

```rust
pub fn hat_smooth_front(abscissae: &[f64], values: &mut [f64]) -> crate::error::Result<()> { /* ... */ }
```

### Constants and Statics

#### Constant `MAX_LEGENDRE_FRONT_DEGREE`

Highest Legendre degree upstream's `plegen.F90` supports.

Degrees 0 through 7 are hard-coded there; anything else hits an
`ASSERT(.false.)`. Reproduced as an error rather than a panic.

```rust
pub const MAX_LEGENDRE_FRONT_DEGREE: usize = 7;
```

## Module `hardening`

The isotropic hardening curve `R(p)`, shared by every law that needs one.

# Why this module exists

Two modules had grown their own `IsotropicHardening` enum independently —
[`super::isotropic`] for the `VMIS_ISOT_*` radial return, and
[`super::damage`] for the Rousselier and GTN porous-plastic laws. They
collided by name, so neither could be re-exported alongside the other, and
a caller reaching for "the hardening curve" had to know which law it was
about to feed. This module replaced both; those two now import from here.

The obvious reading was that one was a duplicate. It was not. Their
`PowerLaw` variants are **different physics**: the `isotropic` one is
upstream's `ecpuis`, `R = σ_y + σ_y (E p / (α σ_y))^(1/n)`, while the
`damage` one is Ludwik, `R = σ_y + K p^n`. Merging them into a single
variant would have silently replaced one curve with the other, so they are
kept apart as [`AsterPower`](IsotropicHardening::AsterPower) and
[`Ludwik`](IsotropicHardening::Ludwik) — the names say which is which where
`PowerLaw` did not.

So this module is the **union**, not the intersection: every curve family
either module offered, each kept distinct, under one type that both use.
The consolidation was behaviour-preserving in the curves themselves; what
it did change is that both callers now get the other's guards (the `p ≥ 0`
clamp and [`SLOPE_SINGULARITY_OFFSET`], previously only in `damage`) and
that [`radial_return`](IsotropicHardening::radial_return) — which lives in
[`super::isotropic`], next to its `nmisot` provenance — now accepts all five
curve families rather than two.

# The curve

`R(p)` is the radius of the yield surface at accumulated equivalent plastic
strain `p`. Every law here needs two things from it — its value, and its
slope `dR/dp`, which the local Newton solves differentiate through.

# Units

Every stress-dimensioned parameter is in pascal \[Pa\]; `p` and every
exponent are dimensionless. `p` is non-negative by construction: it is an
accumulated measure.

```rust
pub mod hardening { /* ... */ }
```

### Types

#### Enum `IsotropicHardening`

An isotropic hardening curve `R(p)`.

Enum dispatch rather than trait objects, per the workspace rule: the set of
curve families is closed, adding one forces every `match` to be revisited,
and rust-analyzer can navigate to each variant.

# Not covered

Tabulated `TRACTION` curves (upstream `rctrac`/`rsliso`). They need the
material data-table infrastructure rather than any new mechanics, so they
are left for a caller to interpolate rather than approximated here.

```rust
pub enum IsotropicHardening {
    Perfect {
        yield_stress: f64,
    },
    Linear {
        yield_stress: f64,
        modulus: f64,
    },
    Ludwik {
        yield_stress: f64,
        coefficient: f64,
        exponent: f64,
    },
    AsterPower {
        yield_stress: f64,
        youngs_modulus: f64,
        alpha: f64,
        exponent: f64,
    },
    EcroNl {
        r0: f64,
        rh: f64,
        r1: f64,
        gamma_1: f64,
        r2: f64,
        gamma_2: f64,
        rk: f64,
        p0: f64,
        gamma_m: f64,
    },
}
```

##### Variants

###### `Perfect`

Perfect plasticity: `R(p) = σ_y`, no hardening at all.

The limiting case, and the one that makes a return map's bracket
degenerate — the residual becomes exactly zero at its upper endpoint —
so it is worth testing against explicitly.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `yield_stress` | `f64` | Initial yield stress `σ_y` \[Pa\], strictly positive. |

###### `Linear`

Linear hardening: `R(p) = σ_y + H p`.

ASTER: the `_LINE` suffix of `VMIS_ISOT_LINE` / `VISC_ISOT_LINE`.
The only family whose radial return has a closed form.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `yield_stress` | `f64` | Initial yield stress `σ_y` \[Pa\], strictly positive. |
| `modulus` | `f64` | Plastic modulus `H` \[Pa\]. Negative values describe linear<br>**softening** and are permitted, but they make the local solve<br>non-monotone — see [`radial_return`](Self::radial_return). |

###### `Ludwik`

Ludwik power-law hardening: `R(p) = σ_y + K p^n`.

The form the Rousselier and GTN porous-plastic laws use. **Distinct
from [`AsterPower`](Self::AsterPower)** — see the module documentation.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `yield_stress` | `f64` | Initial yield stress `σ_y` \[Pa\], strictly positive. |
| `coefficient` | `f64` | Hardening coefficient `K` \[Pa\], non-negative. |
| `exponent` | `f64` | Hardening exponent `n` \[-\], typically 0.05–0.5 for structural<br>steels. |

###### `AsterPower`

code_aster's `ECRO_PUIS` curve:
`R(p) = σ_y + σ_y (E p / (α σ_y))^(1/n)`.

ASTER: the `_PUIS` suffix of `VMIS_ISOT_PUIS`. Upstream: `ecpuis.F90`.
**Distinct from [`Ludwik`](Self::Ludwik)** — this one carries Young's
modulus and a dimensionless `α` inside the bracket, and is linearised
below [`ASTER_POWER_LINEARISATION_STRAIN`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `yield_stress` | `f64` | Initial yield stress `σ_y` \[Pa\], strictly positive. |
| `youngs_modulus` | `f64` | Young's modulus `E` \[Pa\], strictly positive. |
| `alpha` | `f64` | Dimensionless coefficient `α` \[-\], upstream `alfafa`. Strictly<br>positive. |
| `exponent` | `f64` | Hardening exponent `n` \[-\], strictly positive. Upstream stores its<br>reciprocal as `unsurn`. |

###### `EcroNl`

code_aster's `ECRO_NL` nonlinear isotropic hardening, which GTN needs:

`R(p) = R0 + RH p + R1(1 - e^(-γ₁p)) + R2(1 - e^(-γ₂p)) + RK(p + P0)^γm`

Upstream: `f_ecro` in `lcgtn_module.F90`. The two saturating
exponentials give the knee at small strain, the linear term the
far-field slope, and the power term a tunable tail.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `r0` | `f64` | `R0` — initial yield stress \[Pa\], strictly positive. |
| `rh` | `f64` | `RH` — linear hardening modulus \[Pa\]. |
| `r1` | `f64` | `R1` — amplitude of the first saturating term \[Pa\]. |
| `gamma_1` | `f64` | `GAMMA_1` — rate of the first saturating term \[-\]. |
| `r2` | `f64` | `R2` — amplitude of the second saturating term \[Pa\]. |
| `gamma_2` | `f64` | `GAMMA_2` — rate of the second saturating term \[-\]. |
| `rk` | `f64` | `RK` — amplitude of the power term \[Pa\]. |
| `p0` | `f64` | `P0` — offset of the power term \[-\]; keeps it finite at `p = 0`. |
| `gamma_m` | `f64` | `GAMMA_M` — exponent of the power term \[-\]. Upstream defaults it<br>to 1 when the keyword is absent. |

##### Implementations

###### Methods

- ```rust
  pub fn value(self: Self, p: f64) -> f64 { /* ... */ }
  ```
  Flow stress `R(p)` \[Pa\] at accumulated equivalent plastic strain `p`

- ```rust
  pub fn slope(self: Self, p: f64) -> f64 { /* ... */ }
  ```
  Hardening slope `dR/dp` \[Pa\] at accumulated equivalent plastic strain

- ```rust
  pub fn yield_stress(self: Self) -> f64 { /* ... */ }
  ```
  The initial yield stress `R(0)` \[Pa\].

- ```rust
  pub fn aster_name_suffix(self: Self) -> Option<&'static str> { /* ... */ }
  ```
  The ASTER behaviour-name suffix this curve corresponds to, where one

- ```rust
  pub fn validate(self: Self) -> Result<()> { /* ... */ }
  ```
  Reject parameter sets that have no physical meaning.

- ```rust
  pub fn radial_return(self: &Self, trial_equivalent_stress: f64, shear_modulus: f64, accumulated_strain: f64, control: &SolverControl) -> Result<Option<LocalSolution>> { /* ... */ }
  ```
  Solve the von Mises radial return for the plastic multiplier.

- ```rust
  pub fn return_residual(self: &Self, delta_p: f64, trial_equivalent_stress: f64, three_shear_moduli: f64, accumulated_strain: f64) -> f64 { /* ... */ }
  ```
  Upstream's `nmcri2` residual, `R(p_m + Δp) + 3μ Δp - σ_eq^trial`.

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
    fn clone(self: &Self) -> IsotropicHardening { /* ... */ }
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
    fn eq(self: &Self, other: &IsotropicHardening) -> bool { /* ... */ }
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
### Constants and Statics

#### Constant `ASTER_POWER_LINEARISATION_STRAIN`

Below this accumulated plastic strain the `AsterPower` curve is replaced by
its secant through the origin.

Upstream's `p0 = 1.d-10` in `ecpuis.F90`, reproduced exactly. The reason is
that `dR/dp ∝ p^(1/n - 1)` diverges as `p → 0` for any `n > 1`, so a Newton
step taken at `p = 0` would see an infinite slope. Upstream replaces the
curve below `p0` with the straight line joining the origin to `R(p0)`, which
is finite-sloped and continuous with the curve at `p0`.

Note the curve is **C0 but not C1** there: the secant slope comes out
exactly `n` times the curve's own slope at `p0`.

```rust
pub const ASTER_POWER_LINEARISATION_STRAIN: f64 = 1.0e-10;
```

#### Constant `SLOPE_SINGULARITY_OFFSET`

The accumulated plastic strain at which a curve with a genuinely infinite
initial slope is evaluated instead of at `p = 0`.

[`IsotropicHardening::Ludwik`] with `n < 1` and
[`IsotropicHardening::EcroNl`] with `γm < 1` both have `dR/dp → ∞` as
`p → 0`. That divergence is **real physics, not a coding error** — the
Ludwik fit really does rise vertically out of the origin. It is nonetheless
useless to a Newton step, which would propose an infinite correction, so
[`slope`](IsotropicHardening::slope) reports the slope at this offset
instead of an infinity.

Unlike [`ASTER_POWER_LINEARISATION_STRAIN`] this has **no upstream
counterpart**: code_aster reaches these two curves only through the
bracketed solves of the porous-plastic laws, which never ask for a slope at
the origin. It is this port's own guard, and it changes only the *slope*,
never the curve — [`value`](IsotropicHardening::value) is exact everywhere.

```rust
pub const SLOPE_SINGULARITY_OFFSET: f64 = 1.0e-12;
```

## Module `integration`

Local integration algorithms shared by every constitutive law.

# What a "local solve" is

A rate-dependent constitutive law cannot be evaluated in closed form. Given a
strain increment, the stress at the end of the step depends on the inelastic
increment, which itself depends on the end-of-step stress. That circularity
is resolved per integration point, per timestep, by a **scalar root find** —
typically on the equivalent plastic or creep increment.

Every law in code_aster's catalogue declares which algorithm it wants in its
`algo_inte` field, so this handful of solvers is shared machinery: porting it
once unblocks all 151 mechanical laws, which is why it precedes them.

# Coverage of upstream's `algo_inte`

Counting across the 229 catalogue declarations:

| Upstream `algo_inte` | Count | Here |
|---|---|---|
| `ANALYTIQUE` | 68 | [`ScalarAlgorithm::Analytic`] — no local solve; the law inverts in closed form |
| `SANS_OBJET` | 52 | no local solve needed at all |
| `SPECIFIQUE` | 32 | law-specific; each law brings its own |
| `NEWTON` | 23 | [`newton_safeguarded`] |
| `NEWTON_PERT` | 16 | [`newton_perturbed`] |
| `SECANTE` | 12 | [`secant`] |
| `BRENT` | 10 | [`brent`] |
| `NEWTON_1D` | 8 | [`newton_safeguarded`] — the scalar case is the same solve |
| `RUNGE_KUTTA` | few | **not here**: reuses `outram_foam_basic_lib::ode` |

`RUNGE_KUTTA` is deliberately absent. `outram-foam-basic-lib` already carries
`Euler`, `Rkf45` (adaptive Runge-Kutta-Fehlberg) and `Rosenbrock23` (stiff),
driven by an `OdeSystem` trait. A law integrating its internal variables as
an ODE system should use those; re-porting a Runge-Kutta here would duplicate
a Layer-1 primitive at Layer 5, the same mistake as hand-rolling an
eigensolver rather than porting OpenFOAM's into the primitive crate.

# Why safeguarding is not optional

Plain Newton is the obvious choice and the wrong default here. This crate has
already met the failure mode once: upstream's `LimbackCreepModel` omits the
primary-creep derivative from its Jacobian, so the Newton direction is
systematically wrong and the iteration oscillates without converging. The
rheology port had to wrap it in a bracketed safeguard to get an answer at
all.

That is not an isolated defect — an inconsistent or deliberately simplified
Jacobian is common in legacy constitutive code, because the exact tangent is
tedious to derive and the author only needed the residual to vanish. So
[`newton_safeguarded`] keeps a bracket and falls back to bisection whenever
the Newton step would leave it or fails to reduce the interval. That costs
nothing when the Jacobian is good and rescues the iteration when it is not.

# Convergence, and what these are checked against

The tests verify the *published orders of convergence* — quadratic for
Newton, the golden ratio for the secant method — on a classical benchmark
whose root is known analytically. Those orders are theorems, not fitted
constants, which makes them a genuine external reference rather than a
self-consistency check.

```rust
pub mod integration { /* ... */ }
```

### Types

#### Struct `SolverControl`

Convergence control for a local solve.

# Defaults

100 iterations to a residual tolerance of 1e-10 and a step tolerance of
1e-14. The residual tolerance is the one that usually binds; the step
tolerance stops the iteration when the bracket or increment can no longer
be refined in floating point, which is the honest termination criterion when
the residual scale is large.

```rust
pub struct SolverControl {
    pub max_iter: usize,
    pub residual_tol: f64,
    pub step_tol: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `max_iter` | `usize` | Maximum iterations before reporting non-convergence. |
| `residual_tol` | `f64` | Absolute tolerance on the residual `|f(x)|`. |
| `step_tol` | `f64` | Absolute tolerance on the step `|x_{n+1} - x_n|`. |

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
    fn clone(self: &Self) -> SolverControl { /* ... */ }
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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &SolverControl) -> bool { /* ... */ }
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
#### Struct `LocalSolution`

Outcome of a local solve.

Returned rather than logged so a constitutive law can decide what to do —
usually to surface
[`ConstitutiveNotConverged`](crate::error::OffbeatError::ConstitutiveNotConverged)
with the cell index it knows and this solver does not.

```rust
pub struct LocalSolution {
    pub root: f64,
    pub residual: f64,
    pub iterations: usize,
    pub bisection_steps: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `root` | `f64` | The root. |
| `residual` | `f64` | Residual `f(root)` actually achieved. |
| `iterations` | `usize` | Iterations performed. |
| `bisection_steps` | `usize` | How many of those fell back to bisection because the Newton step was<br>rejected.<br><br>Zero means the Jacobian was good throughout. A large count on a law that<br>claims an exact tangent is evidence the tangent is wrong — worth<br>surfacing rather than hiding, since it is otherwise invisible. |

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
    fn clone(self: &Self) -> LocalSolution { /* ... */ }
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
    fn eq(self: &Self, other: &LocalSolution) -> bool { /* ... */ }
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
#### Enum `ScalarAlgorithm`

Which local algorithm a law asks for.

Mirrors upstream's `algo_inte` field. Enum dispatch, not trait objects, per
the workspace rule — the set is closed and known at compile time.

```rust
pub enum ScalarAlgorithm {
    Analytic,
    Newton,
    NewtonPerturbed,
    Secant,
    Brent,
}
```

##### Variants

###### `Analytic`

`ANALYTIQUE` — the law inverts in closed form and needs no iteration.

Present so a law's declared algorithm can be represented faithfully;
attempting to *run* it returns
[`NotImplemented`](crate::error::OffbeatError::NotImplemented) rather
than silently iterating, because a closed-form law that reaches an
iterative solver has been mis-wired.

###### `Newton`

`NEWTON` / `NEWTON_1D` — Newton with a bisection safeguard.

###### `NewtonPerturbed`

`NEWTON_PERT` — Newton with a numerically perturbed derivative.

###### `Secant`

`SECANTE` — the secant method.

###### `Brent`

`BRENT` — Brent's method: bisection, secant and inverse quadratic
interpolation combined.

##### Implementations

###### Methods

- ```rust
  pub const fn aster_name(self: Self) -> &'static str { /* ... */ }
  ```
  The upstream `algo_inte` token this corresponds to.

- ```rust
  pub const fn needs_derivative(self: Self) -> bool { /* ... */ }
  ```
  Whether this algorithm needs an analytic derivative from the caller.

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
    fn clone(self: &Self) -> ScalarAlgorithm { /* ... */ }
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
    fn eq(self: &Self, other: &ScalarAlgorithm) -> bool { /* ... */ }
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

#### Function `newton_safeguarded`

Newton's method with a bisection safeguard on a bracket.

# Arguments

- `f` — the residual. A root is a value where it vanishes.
- `df` — its derivative. If this is inconsistent with `f` the safeguard is
  what saves the solve; see the module documentation.
- `bracket` — `(a, b)` with `f(a)` and `f(b)` of opposite sign. Checked.
- `control` — iteration and tolerance limits.

# Method

Each iteration proposes a Newton step. It is accepted only if it lands
inside the current bracket and is actually reducing the interval; otherwise
the step is replaced by a bisection of the bracket. The bracket is then
tightened using the sign of the new residual, so it always contains a root.
This is the classical safeguarded-Newton arrangement, and it converges
quadratically when the derivative is good while never being worse than
bisection when it is not.

# Errors

[`OffbeatError::Unphysical`] if the supplied bracket does not straddle a
root — that is a caller error, and iterating anyway would return a
confident wrong answer. [`OffbeatError::ConstitutiveNotConverged`] if the
iteration budget is exhausted.

```rust
pub fn newton_safeguarded<F, D>(f: F, df: D, bracket: (f64, f64), control: &SolverControl) -> crate::error::Result<LocalSolution>
where
    F: Fn(f64) -> f64,
    D: Fn(f64) -> f64 { /* ... */ }
```

#### Function `newton_perturbed`

Newton with a numerically perturbed derivative — upstream's `NEWTON_PERT`.

For laws whose analytic tangent is unavailable or known to be inconsistent.
The derivative is approximated by a central difference with step
`perturbation * max(|x|, 1)`, so the step scales with the magnitude of the
unknown rather than being absolute.

# Choosing `perturbation`

Central differencing has truncation error `O(h²)` and round-off error
`O(eps/h)`, which balance near `h = eps^(1/3)`, about 6e-6 for `f64`. That
is the default in [`perturbed_default`]. A much smaller step is not more
accurate — it is noisier.

# Errors

As [`newton_safeguarded`].

```rust
pub fn newton_perturbed<F>(f: F, bracket: (f64, f64), perturbation: f64, control: &SolverControl) -> crate::error::Result<LocalSolution>
where
    F: Fn(f64) -> f64 { /* ... */ }
```

#### Function `perturbed_default`

**Attributes:**

- `MustUse { reason: None }`

The perturbation step that balances truncation against round-off for a
central difference in `f64`: `eps^(1/3)`, about 6.06e-6.

```rust
pub fn perturbed_default() -> f64 { /* ... */ }
```

#### Function `secant`

The secant method — upstream's `SECANTE`.

Replaces Newton's derivative with the slope through the last two iterates,
so it needs no derivative at all. Its order of convergence is the golden
ratio, about 1.618 — superlinear but slower than Newton, which is the price
of not knowing the tangent.

Unlike [`newton_safeguarded`] this keeps no bracket, so it can diverge on a
badly-chosen pair. It is offered because upstream declares it for 12 laws;
prefer [`brent`] when robustness matters more than simplicity.

# Errors

[`OffbeatError::ConstitutiveNotConverged`] if the budget is exhausted or the
secant slope collapses to zero.

```rust
pub fn secant<F>(f: F, x0: f64, x1: f64, control: &SolverControl) -> crate::error::Result<LocalSolution>
where
    F: Fn(f64) -> f64 { /* ... */ }
```

#### Function `brent`

Brent's method — upstream's `BRENT`.

Combines bisection, the secant method and inverse quadratic interpolation:
it takes the fast interpolating step when that step is behaving, and falls
back to bisection when it is not. The result is superlinear convergence with
bisection's guarantee — it cannot fail to converge on a valid bracket.

This is the right default for a constitutive local solve whose residual is
awkward, and the reason upstream offers it alongside Newton.

# Errors

[`OffbeatError::Unphysical`] if the bracket does not straddle a root;
[`OffbeatError::ConstitutiveNotConverged`] if the budget is exhausted.

```rust
pub fn brent<F>(f: F, bracket: (f64, f64), control: &SolverControl) -> crate::error::Result<LocalSolution>
where
    F: Fn(f64) -> f64 { /* ... */ }
```

## Module `isotropic`

Isotropic hardening laws and the Norton-Hoff limit-analysis regularisation.

# What is in here, and what is deliberately not

Two things that both reuse the scalar radial return, and are otherwise
unrelated:

- The scalar radial return that solves for the plastic multiplier against a
  hardening curve — code_aster's `VMIS_ISOT_*` / `VISC_ISOT_*` family.
  Rate-**in**dependent; see the warning below. It is implemented as an
  inherent method on [`IsotropicHardening`], which lives in
  [`super::hardening`] because every law in this port shares it. `_LINE` is
  [`IsotropicHardening::Linear`] and `_PUIS` is
  [`IsotropicHardening::AsterPower`]; the return also accepts the three
  further curve families that module carries.
- [`NortonHoffLimitAnalysis`] — the `NORTON_HOFF` law, which despite its
  name is not a creep law at all but a regularisation used to compute
  **limit loads**.

# Warning: `VISC_ISOT_*` is not rate-dependent through this path

The name invites the assumption that `VISC_ISOT_LINE` and `VISC_ISOT_TRAC`
add a viscous overstress to `VMIS_ISOT_LINE`/`_TRAC`. Through upstream's
`nmisot` they do not. That subroutine's signature carries **no time
instants at all** — no `instam`, no `instap`, no timestep — so nothing in
it can depend on strain *rate*; it branches only on the trailing five
characters of the behaviour name (`_LINE`, `_PUIS`, `_TRAC`) to pick a
hardening curve, and `lc0002` routes both the `VMIS_` and the `VISC_`
spellings into it unchanged.

This port therefore implements the rate-independent return that `nmisot`
actually performs, and does **not** invent a viscous term to justify the
prefix. `VISC_ISOT_NL` is a genuinely different law on a different path
(`lc0076`) and is not ported here.

# The radial return, in one paragraph

Take the elastic trial stress, and measure it with the von Mises equivalent
`σ_eq`. If that is below the current yield `R(p)`, the step was elastic and
nothing happens. If it is above, plastic flow must bring it back onto the
yield surface, and because the flow is deviatoric and isotropic it does so
along the trial deviator's own direction — hence *radial*. The only unknown
is how far: the plastic multiplier `Δp`, fixed by requiring the returned
stress to sit exactly on the surface,

`R(p_m + Δp) + 3μ Δp - σ_eq^trial = 0`.

That is upstream's `nmcri2` residual verbatim, where its `1.5*deuxmu` is
`1.5 × 2μ = 3μ`.

```rust
pub mod isotropic { /* ... */ }
```

### Types

#### Struct `NortonHoffLimitAnalysis`

The Norton-Hoff regularisation used for **limit-load** analysis.

ASTER behaviour name: `NORTON_HOFF` (`num_lc = 17`). Upstream:
`bibfor/nonlinear/nmhoff.F90` — legacy symbol `nmhoff`, dispatched by
`bibfor/lc/lc0017.F90` (`lc0017`).

# This is not a creep law, despite the name

The name is shared with Norton creep and the two are easy to confuse, but
they answer different questions. Norton creep asks *how fast does this
deform under load*. Norton-Hoff limit analysis asks *what is the largest
load this structure can carry at all* — and it gets there by solving a
sequence of nonlinear-elastic problems whose solutions converge onto the
rigid-perfectly-plastic collapse state. There is no accumulated strain, no
internal state, and no history: the stress is a pure function of the
current total strain.

# The law

`σ = A ‖ε‖^(m-2) ε`, with `A = σ_y (2/3)^(m/2)`,

where `‖ε‖` is the Euclidean norm of the strain in Mandel form — which,
because Mandel carries `√2` on the shears, equals the tensor Frobenius norm
`√(ε:ε)`. This is exactly why the port takes an [`AsterVoigt`] and not a
loose six-array: the identity holds in Mandel and fails in engineering
Voigt.

# The continuation parameter

The exponent is driven by a pseudo-time `t`:

`m = 1 + 10^(1-t)`.

At `t = 1`, `m = 2` and the law is **linear** — an ordinary Newtonian
solid. As `t` grows, `m → 1` and the stress magnitude tends to `A`
independent of how large the strain is, which is the **rigid-perfectly-
plastic** limit whose solution is the collapse load. So `t` is not physical
time; it is a homotopy parameter walking the problem from an easy linear
solve to the hard plastic one. Advancing it too fast is the usual reason a
limit-analysis run stops converging.

# Not ported

The consistent tangent `dsidep`. Upstream builds it in the same subroutine
(`coef·I + coef(m-2)/‖ε‖² · ε ⊗ ε`), but it is only consumed by an assembled
FE stiffness matrix, which this crate's mechanics solve does not yet take.
It is a small addition once that exists.

```rust
pub struct NortonHoffLimitAnalysis {
    pub yield_stress: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `yield_stress` | `f64` | Yield stress `σ_y` \[Pa\]. Upstream reads it as `SY` from the<br>`ECRO_LINE` material block. Must be positive. |

##### Implementations

###### Methods

- ```rust
  pub fn new(yield_stress: f64) -> Self { /* ... */ }
  ```
  Build the law from its single material parameter.

- ```rust
  pub fn aster_name(self: &Self) -> &'static str { /* ... */ }
  ```
  The ASTER behaviour name, verbatim.

- ```rust
  pub fn exponent(pseudo_time: f64) -> f64 { /* ... */ }
  ```
  The exponent `m = 1 + 10^(1-t)` \[-\] at pseudo-time `t` \[-\].

- ```rust
  pub fn stress(self: &Self, strain: AsterVoigt, pseudo_time: f64) -> Result<AsterVoigt> { /* ... */ }
  ```
  Stress \[Pa\] from total strain, at pseudo-time `t` \[-\].

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
    fn clone(self: &Self) -> NortonHoffLimitAnalysis { /* ... */ }
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
    fn eq(self: &Self, other: &NortonHoffLimitAnalysis) -> bool { /* ... */ }
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
## Module `kinematics`

Tensor conventions and finite-strain kinematics for the code_aster port.

# Why this module exists before any law is ported

Two things had to be pinned down before a single constitutive law could be
written, and both are **silent-wrong-answer risks** — they produce code that
compiles, runs, and returns plausible but incorrect stresses, announcing
themselves in no test that was not written to catch them.

1. **The Voigt convention.** code_aster stores symmetric tensors as
   six-vectors with `sqrt(2)` on the shear components. Mapping that onto the
   plain `SymmTensor` this port inherits from `outram-foam-basic-lib`
   without the scaling corrupts every shear term in every law downstream.
2. **The strain measure.** The maintainer decided on 2026-08-04 to design
   for finite strain from the start rather than defer it, because
   retrofitting a finite-strain measure after the laws are written is a
   rewrite rather than a patch.

# The Mandel convention, and why the `sqrt(2)` is there

code_aster orders components `(XX, YY, ZZ, XY, XZ, YZ)` and multiplies the
three shear entries by `sqrt(2)` — upstream's `lc0000.F90` carries this as
the literal vector `r2 = [1, 1, 1, sqrt(2), sqrt(2), sqrt(2)]`.

That scaling is not decoration. It makes the six-vector dot product equal
the tensor double-contraction:

`a : b = sum_ij a_ij b_ij = dot(voigt(a), voigt(b))`

which in turn makes the stiffness matrix in this basis symmetric and makes
the Euclidean norm of the vector equal the Frobenius norm of the tensor. It
is the *Mandel* convention, and it is what lets a law written in terms of
six-vectors compute an equivalent stress without inserting factors of two by
hand.

The classical engineering Voigt convention — no scaling, shear strains
doubled but shear stresses not — has none of those properties, and mixing
the two is the classic way to get a von Mises stress wrong by a factor
between 1 and 2 depending on the stress state. [`AsterVoigt`] therefore
carries the scaling internally and converts at the boundary, so no law ever
has to remember it.

# Finite strain

[`DeformationGradient`] and [`hencky_strain`] provide the logarithmic strain
measure code_aster's `GDEF_LOG` wraps its small-strain laws in. The Hencky
strain is an isotropic tensor function — the logarithm applied to the
eigenvalues of the stretch — which is why the spectral decomposition had to
be ported into `outram-foam-basic-lib` first.

## One numerical trap, found the hard way

The spectral route is **ill-conditioned exactly where fuel performance
spends most of its time**: at small deformation. As the strain vanishes
`C -> I`, whose three eigenvalues coincide, and the eigenvectors of a
near-degenerate tensor are arbitrary within the degenerate subspace — yet it
is precisely the tiny differences between those eigenvalues that carry the
strain. Reconstructing in a basis that is mostly noise costs first-order
accuracy.

This is not a subtle few-percent effect. Measured against the engineering
small-strain tensor, the discrepancy stopped falling: it dropped by only
1.32x for a 10x smaller strain, where it should drop 10x. A test written to
assert only "the two agree to a few percent" would have passed and hidden
it.

[`hencky_strain`] therefore switches to the Mercator series for `ln(I + A)`
whenever `A = C - I` is small, which is exact in the same limit. The two
branches agree to better than 1e-9 relative across the switch.

```rust
pub mod kinematics { /* ... */ }
```

### Types

#### Struct `AsterVoigt`

A symmetric tensor in code_aster's six-component Mandel ordering.

Components are `(XX, YY, ZZ, XY, XZ, YZ)` with the three shear entries
scaled by `sqrt(2)`, matching upstream's `r2` vector in `lc0000.F90`.

# Units

Whatever the tensor carries — Pa for stress, dimensionless for strain. This
type is a layout, not a quantity.

# Do not construct component-wise unless you mean the scaled values

[`AsterVoigt::from_components`] takes the six numbers *as code_aster stores
them*, i.e. already scaled. To go from an ordinary tensor use
[`AsterVoigt::from_tensor`], which applies the scaling for you. Mixing the
two up is precisely the error this type exists to prevent.

```rust
pub struct AsterVoigt {
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
  pub const fn from_components(components: [f64; 6]) -> Self { /* ... */ }
  ```
  Build from the six components exactly as code_aster stores them —

- ```rust
  pub const fn components(self: &Self) -> [f64; 6] { /* ... */ }
  ```
  The six components in upstream's storage order and scaling.

- ```rust
  pub fn from_tensor(t: SymmTensor) -> Self { /* ... */ }
  ```
  Convert an ordinary symmetric tensor into code_aster's convention,

- ```rust
  pub fn to_tensor(self: Self) -> SymmTensor { /* ... */ }
  ```
  Convert back to an ordinary symmetric tensor, removing the scaling.

- ```rust
  pub fn dot(self: Self, other: Self) -> f64 { /* ... */ }
  ```
  Dot product of two Mandel six-vectors.

- ```rust
  pub fn norm(self: Self) -> f64 { /* ... */ }
  ```
  Euclidean norm of the six-vector, equal to the tensor's Frobenius norm.

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
    fn clone(self: &Self) -> AsterVoigt { /* ... */ }
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
    fn default() -> AsterVoigt { /* ... */ }
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
    fn eq(self: &Self, other: &AsterVoigt) -> bool { /* ... */ }
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
#### Struct `DeformationGradient`

The deformation gradient `F = dx/dX`, mapping reference to current
configuration.

# What it means

`F` is the complete description of local deformation: it carries both the
stretch and the rotation of a material neighbourhood. `det(F)` is the
volume ratio, so `det(F) = 1` is incompressible and `det(F) <= 0` is
material turned inside out — inadmissible, and rejected rather than
propagated.

# Units

Dimensionless. The identity is the undeformed state.

```rust
pub struct DeformationGradient {
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
  pub fn new(f: Tensor) -> Result<Self> { /* ... */ }
  ```
  Wrap a deformation-gradient tensor, checking admissibility.

- ```rust
  pub fn identity() -> Self { /* ... */ }
  ```
  The undeformed state, `F = I`.

- ```rust
  pub fn from_displacement_gradient(grad_u: Tensor) -> Result<Self> { /* ... */ }
  ```
  Build from a small displacement gradient, `F = I + grad(u)`.

- ```rust
  pub fn tensor(self: Self) -> Tensor { /* ... */ }
  ```
  The underlying tensor.

- ```rust
  pub fn jacobian(self: Self) -> f64 { /* ... */ }
  ```
  The Jacobian `det(F)` — the ratio of deformed to reference volume.

- ```rust
  pub fn right_cauchy_green(self: Self) -> SymmTensor { /* ... */ }
  ```
  The right Cauchy-Green deformation tensor `C = Fᵀ F`.

- ```rust
  pub fn green_lagrange_strain(self: Self) -> SymmTensor { /* ... */ }
  ```
  The Green-Lagrange strain `E = ½(C − I)`.

- ```rust
  pub fn hencky_strain(self: Self) -> Result<SymmTensor> { /* ... */ }
  ```
  The logarithmic (Hencky) strain `E = ½ ln(C)`.

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
    fn clone(self: &Self) -> DeformationGradient { /* ... */ }
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
    fn eq(self: &Self, other: &DeformationGradient) -> bool { /* ... */ }
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

#### Function `hencky_strain`

The logarithmic (Hencky) strain `½ ln(C)` of a right Cauchy-Green tensor.

# Method

`ln` of a symmetric positive-definite tensor is an isotropic tensor
function: decompose `C` spectrally, take the scalar logarithm of each
eigenvalue, and rebuild in the same eigenbasis. The eigen decomposition
comes from `outram-foam-basic-lib`, which is why that had to be ported
first.

# Errors

[`OffbeatError::Unphysical`] if any eigenvalue of `C` is non-positive. `C`
is positive-definite for any admissible deformation, so this indicates a
corrupt input rather than an extreme one.

```rust
pub fn hencky_strain(c: outram_foam_basic_lib::primitives::SymmTensor) -> crate::error::Result<outram_foam_basic_lib::primitives::SymmTensor> { /* ... */ }
```

## Module `log_strain`

The `GDEF_LOG` finite-strain wrapper.

# The idea

Writing a constitutive law at finite strain is hard; writing one at small
strain is comparatively routine, and the literature is full of them. The
logarithmic-strain framework buys the former with the latter: it wraps an
**unmodified small-strain law** in a pre- and post-processing pair, and the
result is a genuine finite-strain model.

Three steps, per integration point, per timestep:

1. **Pre-process.** Turn the deformation gradient `F` into the logarithmic
   (Hencky) strain `E = ½ ln(C)`, `C = FᵀF`.
2. **Call the small-strain law**, handing it `E` as though it were an
   engineering strain. It returns a stress `T` — the quantity work-conjugate
   to `E`, which is *not* any of the usual stress measures.
3. **Post-process.** Map `T` to the second Piola-Kirchhoff stress through the
   projection `S = P : T`, then push forward to the true (Cauchy) stress
   `σ = F S Fᵀ / J`.

# Why it works

Because Hencky strain is *additive* in successive coaxial stretches, a law
calibrated on small-strain data stays meaningful when the strain is large:
stretching by 2 then 3 gives the same logarithmic strain as stretching by 6.
The framework is exact for the isotropic case, not an approximation — the
projection `P` is precisely the derivative that makes `T : dE` equal the
stress power.

# What this module does and does not cover

**Covers:** the kinematic wrapper — strain pre-processing, the projection
tensor, and the stress post-processing, for an isotropic material in 3D.

**Does not cover:** the consistent tangent transformation (the `T:d²E`
geometric-stiffness term, upstream's `gdlog_rigeo`), and the element-level
`B`-matrix machinery, which belongs to a finite-element framework this crate
does not have. A caller wanting a Newton tangent at the structural level
needs those; a caller integrating a constitutive law at a point does not.

# Boundary with OFFBEAT's small-strain mechanics

[`crate::mechanics::MechanicsSolver`] is a **small-strain** solver: it
assembles equilibrium for `ε = ½(∇D + ∇Dᵀ)` and knows nothing about `F`.
These laws are finite-strain. That difference is deliberate and must stay
visible rather than implied, so the two meet only through
[`LogarithmicStrain::from_displacement_gradient`], which is the one place the
conversion happens. Do not feed a small-strain tensor to this wrapper
directly; it expects a deformation gradient.

```rust
pub mod log_strain { /* ... */ }
```

### Types

#### Struct `LogarithmicStrain`

A deformation prepared for a small-strain constitutive law.

Holds the logarithmic strain to hand the law, plus the spectral data needed
to map the law's stress back afterwards. Build one per integration point per
timestep.

# Example

```no_run
use outram_park_fork_offbeat::rheology::aster::{DeformationGradient, LogarithmicStrain};
# use outram_foam_basic_lib::primitives::{SymmTensor, Tensor};
# fn small_strain_law(_e: SymmTensor) -> SymmTensor { SymmTensor::new(0.,0.,0.,0.,0.,0.) }
# fn demo(f: Tensor) -> Result<(), Box<dyn std::error::Error>> {
let gradient = DeformationGradient::new(f)?;
let wrapper = LogarithmicStrain::new(gradient)?;

// The law sees a strain and returns its work-conjugate stress.
let t = small_strain_law(wrapper.log_strain());

// Which the wrapper turns into the true stress.
let cauchy = wrapper.cauchy_from_conjugate(t);
# Ok(())
# }
```

```rust
pub struct LogarithmicStrain {
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
  pub fn new(gradient: DeformationGradient) -> Result<Self> { /* ... */ }
  ```
  Prepare a deformation gradient for a small-strain law.

- ```rust
  pub fn from_displacement_gradient(grad_u: Tensor) -> Result<Self> { /* ... */ }
  ```
  Prepare from a small displacement gradient, `F = I + ∇u`.

- ```rust
  pub fn log_strain(self: &Self) -> SymmTensor { /* ... */ }
  ```
  The logarithmic strain to hand the small-strain law.

- ```rust
  pub fn gradient(self: &Self) -> DeformationGradient { /* ... */ }
  ```
  The deformation gradient this was built from.

- ```rust
  pub fn principal_stretches(self: &Self) -> Vector3 { /* ... */ }
  ```
  The principal stretches `λ` (not their squares), ascending.

- ```rust
  pub fn second_piola_from_conjugate(self: &Self, t: SymmTensor) -> SymmTensor { /* ... */ }
  ```
  Map the law's work-conjugate stress `T` to the second Piola-Kirchhoff

- ```rust
  pub fn cauchy_from_second_piola(self: &Self, s: SymmTensor) -> SymmTensor { /* ... */ }
  ```
  Push the second Piola-Kirchhoff stress forward to Cauchy stress.

- ```rust
  pub fn cauchy_from_conjugate(self: &Self, t: SymmTensor) -> SymmTensor { /* ... */ }
  ```
  The full post-processing step: work-conjugate stress to Cauchy stress.

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
    fn clone(self: &Self) -> LogarithmicStrain { /* ... */ }
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
    fn eq(self: &Self, other: &LogarithmicStrain) -> bool { /* ... */ }
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
## Module `metallurgy`

Metallurgical and irradiation constitutive laws (bead `op-a7p.5`).

# Why a separate module from [`viscoplastic`](super::viscoplastic)

The laws in [`viscoplastic`](super::viscoplastic) are *isotropic* and driven
by *time*. The laws here break one or both of those assumptions, and each
break is the whole point of the law:

| Law | What it breaks |
|---|---|
| [`LogarithmicIrradiationLaw`] | driven by **fluence**, not time; the rate does not depend on the clock at all |
| [`Irrad3m`] | adds **swelling** (a volumetric eigenstrain) and an **incubation threshold** before irradiation creep starts |
| [`MetaLemaAni`] | **anisotropic** — the equivalent stress is a Hill quadratic form, so the return direction is not the stress deviator |

All three exist because cladding and vessel internals live in a neutron
flux, and a flux does things temperature alone does not.

# Fluence and flux units — read this before using any law here

Getting these wrong is the single most likely way to produce a confident
wrong answer from this module, because every quantity involved is a pure
number whose meaning lives entirely in a convention.

**Upstream fixes no unit.** code_aster's `IRRA` is a user-supplied command
variable (`AFFE_VARC`), and every irradiation coefficient is read from the
user's own material record. Consistency between the two is the user's
responsibility, and nothing in the Fortran checks it.

**This port therefore declares a convention per law and states it in the
parameter documentation**, because "unit-agnostic" is not a usable contract
for a Rust API:

- [`LogarithmicIrradiationParameters`] — fast neutron fluence `Φ` in
  **n/m²** (E > 1 MeV), the SI form. A user carrying n/cm² instead must
  multiply both `primary_fluence_constant` and `secondary_compliance` by
  `1e4`; `primary_compliance` is unaffected. Using the wrong one changes the
  creep by a factor of `1e4`, and nothing in the arithmetic will complain.
- [`Irrad3mParameters`] — irradiation dose in **dpa** (displacements per
  atom), which is the convention `R5.03.13` uses for the 304/316 stainless
  internals this law was fitted to. dpa is *not* a fluence: it is a damage
  measure, and the conversion to n/m² is spectrum-dependent and is not
  performed here.

Neither law uses a **flux** at all — both are driven by the *fluence
increment* over the step. That is a deliberate difference from
[`LemaitreIrradiation`](super::viscoplastic::ViscoplasticLaw::LemaitreIrradiation),
which does take a fast flux `φ̇` in n/(m²·s). If you are switching between
the two, that is the boundary at which a factor of the timestep gets lost.

# Temperature

Upstream passes temperature in **degrees Celsius** and adds `r8t0()`
(273.15) at each Arrhenius evaluation. This port takes **kelvin
throughout** — no conversion happens inside, and passing Celsius will give a
wildly wrong Arrhenius factor rather than an error.

# What is ported and what is not

Ported: `VISC_IRRA_LOG`, `GRAN_IRRA_LOG`, `IRRAD3M`, and the *mechanical*
half of `META_LEMA_ANI`. **Not** ported: the `ZIRC` / `ZIRC_META` phase
kinetics (upstream `bibfor/metallurgy/zedgar.F90`) — those are `PHASE`-type
state laws rather than mechanical ones, and [`MetaLemaAni`] takes the β-phase
fraction as an *input* precisely so the two can be ported independently.

# Status

**Verification only.** Every test in this module checks the port against
upstream's algebra, an analytical limit, or an invariant. None of it is
validation: no result here has been compared with reactor data or with
code_aster output, and per `RESPONSIBLE_USE.md` these laws remain untrusted
draft material until the maintainer reviews them.

```rust
pub mod metallurgy { /* ... */ }
```

### Types

#### Struct `LogarithmicIrradiationParameters`

Parameters of the logarithmic irradiation-creep law.

# The physics, for a reader who has not met irradiation creep

A metal in a neutron flux creeps under stresses far below those that would
make it creep thermally. Neutrons knock atoms off their lattice sites,
producing vacancies and interstitials continuously; those point defects
migrate and are absorbed preferentially at dislocations whose orientation
suits the applied stress, and the material flows. The controlling variable
is therefore **accumulated damage** — fluence — not elapsed time. Two
specimens at the same stress and temperature, one irradiated and one not,
will creep by wildly different amounts over the same hour.

# Why "logarithmic"

The creep rate per unit fluence is

`dp/dΦ = σ_eq · exp(-Q/(R·T)) · (A·C_t / (1 + C_t·Φ) + B)`

which integrates in closed form to

`p = σ_eq · exp(-Q/(R·T)) · (A·ln(1 + C_t·Φ) + B·Φ)`

The first term saturates **logarithmically** — that is primary irradiation
creep, fast at first and slowing as the defect microstructure reaches a
steady state. The second is linear in fluence: secondary, steady-state
irradiation creep, which never saturates and is what dominates over a fuel
cycle.

# Linearity in stress, and why that matters

The law is **linear in `σ_eq`** — a stress exponent of exactly 1. That is a
genuine feature of irradiation creep at reactor stress levels, not a
simplification, and it is what makes the step integration closed-form
(upstream declares `algo_inte = ANALYTIQUE`): no local iteration is needed
at all. Contrast
[`LemaitreIrradiation`](super::viscoplastic::ViscoplasticLaw::LemaitreIrradiation),
whose exponent `n` is a free parameter and which therefore needs a root
find.

# Units

Fluence `Φ` in **n/m²** (fast, E > 1 MeV) by this port's convention — see
the module documentation for the n/cm² trap. The parameter units follow from
requiring `dp/dΦ · ΔΦ` to be dimensionless with `σ_eq` in pascal.

```rust
pub struct LogarithmicIrradiationParameters {
    pub primary_compliance: f64,
    pub secondary_compliance: f64,
    pub primary_fluence_constant: f64,
    pub activation_temperature: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `primary_compliance` | `f64` | Primary (saturating) creep amplitude `A` \[1/Pa\]. Upstream `A`.<br><br>Multiplies the logarithmic term. Because the term saturates, `A` sets<br>the *total* primary creep strain per unit stress, not a rate. |
| `secondary_compliance` | `f64` | Secondary (steady-state) creep compliance `B` \[1/(Pa·n/m²)\].<br>Upstream `B`.<br><br>Creep strain per unit stress per unit fluence, once primary creep has<br>saturated. This is the parameter that dominates a full cycle. |
| `primary_fluence_constant` | `f64` | Primary-creep fluence constant `C_t` \[1/(n/m²)\]. Upstream `CSTE_TPS`.<br><br>Its reciprocal is the fluence at which primary creep is roughly half<br>saturated, so `1/C_t` is the natural "primary creep dose". Must be<br>non-negative; zero disables the primary term. |
| `activation_temperature` | `f64` | Activation temperature `Q/R` \[K\]. Upstream `ENER_ACT`.<br><br>Enters as `exp(-Q/(R·T))` with `T` in kelvin. Note that irradiation<br>creep is only weakly thermally activated compared with thermal creep, so<br>this is typically a few thousand kelvin rather than tens of thousands. |

##### Implementations

###### Methods

- ```rust
  pub fn creep_compliance(self: Self, fluence_start: f64, fluence_increment: f64, temperature: f64) -> Result<f64> { /* ... */ }
  ```
  Creep compliance `C` \[1/Pa\] over one step, **exactly as upstream

- ```rust
  pub fn exact_creep_compliance(self: Self, fluence_start: f64, fluence_increment: f64, temperature: f64) -> Result<f64> { /* ... */ }
  ```
  Creep compliance `C` \[1/Pa\] from the **exact** fluence integral.

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
    fn clone(self: &Self) -> LogarithmicIrradiationParameters { /* ... */ }
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
    fn eq(self: &Self, other: &LogarithmicIrradiationParameters) -> bool { /* ... */ }
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
#### Struct `IrradiationGrowthDirection`

The direction along which irradiation growth acts, as upstream's `ANGL_REP`
pair of Euler angles.

# What irradiation growth is

Zirconium alloys are hexagonal and strongly textured after tube drawing.
Under irradiation they *change shape at constant volume with no applied
stress at all*: interstitials condense on prismatic planes and vacancies on
basal planes, so the crystal lengthens along one axis and thins along the
others. In a fuel assembly this elongates the rods and the guide tubes over
a cycle, and it is a design driver for the assembly hold-down springs.

It is a **stress-free eigenstrain**, like thermal expansion: it changes the
strain the elastic predictor sees, and does not itself relax.

# Angles

Both in **radians**. Upstream takes them from `AFFE_CARA_ELEM`'s `MASSIF`
keyword, in degrees, and converts before calling. `azimuth` is upstream's
`alpha`, `elevation` is upstream's `beta`; the intended growth direction is
`n = (cos α cos β, sin α cos β, -sin β)`.

```rust
pub struct IrradiationGrowthDirection {
    pub azimuth: f64,
    pub elevation: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `azimuth` | `f64` | Rotation about the z axis, `α` \[rad\]. Upstream `ANGL_REP(1)`. |
| `elevation` | `f64` | Elevation out of the xy plane, `β` \[rad\]. Upstream `ANGL_REP(2)`.<br><br>Upstream rejects a non-zero value in 2-D (`ALGORITH11_82`). |

##### Implementations

###### Methods

- ```rust
  pub fn unit_vector(self: Self) -> Vector3 { /* ... */ }
  ```
  The unit vector growth is intended to act along.

- ```rust
  pub fn strain_increment(self: Self, growth_strain_increment: f64) -> SymmTensor { /* ... */ }
  ```
  Growth strain increment tensor, **reproducing upstream `nmvpir.F90`

- ```rust
  pub fn strain_increment_rank_one(self: Self, growth_strain_increment: f64) -> SymmTensor { /* ... */ }
  ```
  Growth strain increment as the rank-one dyad `Δε_g · n ⊗ n`.

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
    fn clone(self: &Self) -> IrradiationGrowthDirection { /* ... */ }
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
    fn eq(self: &Self, other: &IrradiationGrowthDirection) -> bool { /* ... */ }
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
#### Enum `LogarithmicIrradiationLaw`

A logarithmic irradiation law — fluence-driven creep, optionally with
irradiation growth.

Enum dispatch rather than trait objects, per the workspace rule.

```rust
pub enum LogarithmicIrradiationLaw {
    Creep(LogarithmicIrradiationParameters),
    CreepAndGrowth {
        creep: LogarithmicIrradiationParameters,
        growth: IrradiationGrowthDirection,
    },
}
```

##### Variants

###### `Creep`

Fluence-driven creep alone.

ASTER behaviour name: `VISC_IRRA_LOG` (`num_lc = 28`, 2 state variables
`EPSPEQ`, `IRVECU`). Upstream: `bibfor/comport/nmvpir.F90` reached
through `bibfor/lc/lc0028.F90` — legacy symbols `nmvpir`, `lc0028`.
Integration: `ANALYTIQUE`, and this port keeps it closed-form.

Intended by upstream for the *axial* creep of fuel assembly structures.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `LogarithmicIrradiationParameters` |  |

###### `CreepAndGrowth`

Fluence-driven creep plus irradiation growth.

ASTER behaviour name: `GRAN_IRRA_LOG` (`num_lc = 28`, 3 state variables
`EPSPEQ`, `IRVECU`, `EPSGRD`). Same upstream driver as
[`Creep`](Self::Creep); the only difference is the extra stress-free
growth eigenstrain, which is subtracted from the strain increment before
the elastic predictor.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `creep` | `LogarithmicIrradiationParameters` | The creep parameters — identical in form to<br>[`Creep`](Self::Creep). |
| `growth` | `IrradiationGrowthDirection` | The direction growth acts along. |

##### Implementations

###### Methods

- ```rust
  pub const fn aster_name(self: Self) -> &'static str { /* ... */ }
  ```
  The upstream ASTER behaviour name.

- ```rust
  pub const fn creep_parameters(self: Self) -> LogarithmicIrradiationParameters { /* ... */ }
  ```
  The creep parameters, whichever variant this is.

- ```rust
  pub fn growth_strain_increment(self: Self, growth_strain_increment: f64) -> SymmTensor { /* ... */ }
  ```
  The stress-free growth strain increment for this step, or zero for

- ```rust
  pub fn integrate(self: Self, trial_stress: SymmTensor, shear_modulus: f64, fluence_start: f64, fluence_increment: f64, temperature: f64) -> Result<CreepIncrement> { /* ... */ }
  ```
  Integrate one step in closed form, returning the creep increment.

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
    fn clone(self: &Self) -> LogarithmicIrradiationLaw { /* ... */ }
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
    fn eq(self: &Self, other: &LogarithmicIrradiationLaw) -> bool { /* ... */ }
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
#### Struct `Irrad3mParameters`

Material parameters of `IRRAD3M`.

# What the law is for

The austenitic stainless internals of a PWR vessel — baffles, formers, the
bolts that hold them — sit in the highest neutron flux of any structural
component in the plant. Over decades they harden and embrittle, creep under
the bolt preload, and **swell**: voids nucleate and grow, and the steel gains
volume. `IRRAD3M` is EDF's model for that combination, and the three
mechanisms are genuinely coupled, because swelling changes the load which
changes the creep.

# Units

Dose in **dpa** throughout (see the module documentation). `ZETA_F` and
`ZETA_G` are dimensionless multipliers that default to 1 upstream and exist
to let a user scale the creep and swelling terms without re-fitting.

```rust
pub struct Irrad3mParameters {
    pub yield_strength: f64,
    pub uniform_elongation: f64,
    pub ultimate_strength: f64,
    pub creep_compliance: f64,
    pub creep_threshold: f64,
    pub swelling_rate: f64,
    pub swelling_sharpness: f64,
    pub swelling_onset_dose: f64,
    pub yield_plateau_factor: f64,
    pub creep_scale: f64,
    pub swelling_scale: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `yield_strength` | `f64` | Conventional yield strength `R_p0.2` \[Pa\]. Upstream `R02`.<br><br>The 0.2 %-offset proof stress. Upstream pins the hardening curve to pass<br>through it at the fixed plastic strain `p_e = 2e-3`. |
| `uniform_elongation` | `f64` | Uniform elongation `ε_u` \[-\]. Upstream `EPSI_U`.<br><br>The *true* plastic strain at the onset of necking, i.e. at the tensile<br>maximum. Typically 0.2-0.4 for unirradiated austenitic steel and far<br>smaller once irradiated. |
| `ultimate_strength` | `f64` | Ultimate tensile strength `R_m` \[Pa\]. Upstream `RM`.<br><br>The *engineering* UTS. Upstream converts it to a true stress with the<br>standard `R_m·exp(ε_u)`, which is where the `exp` in the identification<br>comes from. |
| `creep_compliance` | `f64` | Irradiation-creep compliance `A_i0` \[1/(Pa·dpa)\]. Upstream `AI0`.<br><br>Converts accumulated stress-dose above the threshold into creep strain. |
| `creep_threshold` | `f64` | Irradiation-creep incubation threshold `η_s` \[Pa·dpa\]. Upstream<br>`ETAI_S`.<br><br>Creep does not start until the accumulated stress-dose `η = ∫σ_eq dΦ`<br>exceeds this. A threshold in `σ·Φ` rather than in `Φ` alone means a<br>lightly loaded component may never start creeping at all — which is the<br>point of modelling it. |
| `swelling_rate` | `f64` | Saturated volumetric swelling rate `R_g0` \[1/dpa\]. Upstream `RG0`.<br><br>The steady swelling rate reached once the incubation dose is passed. It<br>is a **volumetric** rate; upstream divides by three to obtain the linear<br>strain, and so does this port. |
| `swelling_sharpness` | `f64` | Swelling transition sharpness `α` \[1/dpa\]. Upstream `ALPHA`.<br><br>Controls how abruptly swelling switches on around<br>[`swelling_onset_dose`](Self::swelling_onset_dose). Zero disables<br>swelling entirely (upstream's `alpha > 0` guard). |
| `swelling_onset_dose` | `f64` | Swelling incubation dose `Φ₀` \[dpa\]. Upstream `PHI0`.<br><br>The dose at which the logistic swelling rate reaches half its saturated<br>value. |
| `yield_plateau_factor` | `f64` | Post-irradiation softening factor `κ` \[-\]. Upstream `KAPPA`.<br><br>Sets the initial plateau of the flow curve at `κ·R_p0.2`, below which<br>the material flows at constant stress. Values below one represent an<br>irradiated microstructure that yields locally before the bulk proof<br>stress is reached. |
| `creep_scale` | `f64` | Irradiation-creep scale factor `ζ_f` \[-\]. Upstream `ZETA_F`, default 1. |
| `swelling_scale` | `f64` | Swelling scale factor `ζ_g` \[-\]. Upstream `ZETA_G`, default 1. |

##### Implementations

###### Methods

- ```rust
  pub fn identify_hardening(self: Self) -> Result<Irrad3mHardening> { /* ... */ }
  ```
  Identify the three-segment hardening curve from the tensile data.

- ```rust
  pub fn swelling_strain(self: Self, dose: f64) -> f64 { /* ... */ }
  ```
  Accumulated **linear** swelling strain at dose `dose` \[dpa\].

- ```rust
  pub fn swelling_strain_increment(self: Self, dose_start: f64, dose_end: f64) -> SymmTensor { /* ... */ }
  ```
  Stress-free swelling strain increment tensor over one step.

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
    fn clone(self: &Self) -> Irrad3mParameters { /* ... */ }
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
    fn eq(self: &Self, other: &Irrad3mParameters) -> bool { /* ... */ }
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
#### Struct `Irrad3mHardening`

The identified hardening curve of an [`Irrad3mParameters`] set.

# Why identification is needed at all

The user supplies three *tensile-test* numbers — proof stress, UTS, uniform
elongation — and the law needs a *flow curve* `σ_y(p)`. Upstream builds a
three-segment curve and fixes its free parameters by requiring it to pass
through both measured points:

- `σ_y(p_e) = R_p0.2` at `p_e = 2e-3`, and
- `σ_y(ε_u) = R_m · exp(ε_u)` — the true stress at necking.

With the power-law form `σ_y = K(p + p₀)^n` and the substitution
`p₀ = n - ε_u`, the second condition gives `K = R_m e^{ε_u} / n^n` outright
and the first collapses to the scalar equation

`1 - (R_m e^{ε_u}/R_p0.2) · (n - n₀)^n / n^n = 0`,  `n₀ = ε_u - p_e`

which upstream solves by dichotomy. That equation is the whole of the
identification, and it is fully checkable — see the module tests.

# The three segments

| Range | Flow stress | Meaning |
|---|---|---|
| `p < p_k` | `κ·R_p0.2` | a constant plateau — irradiated material flowing before bulk yield |
| `p_k ≤ p < p_e` | `a·(p - p_e) + σ(p_e)` | a straight line joining the plateau to the power law |
| `p ≥ p_e` | `K(p + p₀)^n` | the identified power law |

The line's slope `a` is the power law's own slope at `p_e`, so the curve is
`C¹` there and merely continuous at `p_k`.

```rust
pub struct Irrad3mHardening {
    pub coefficient: f64,
    pub exponent: f64,
    pub strain_offset: f64,
    pub stress_at_proof_strain: f64,
    pub slope_at_proof_strain: f64,
    pub plateau_strain: f64,
    pub plateau_stress: f64,
    pub used_fallback: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `coefficient` | `f64` | Power-law coefficient `K` \[Pa\]. Upstream `materf(7,2)`. |
| `exponent` | `f64` | Power-law exponent `n` \[-\]. Upstream `materf(8,2)`. |
| `strain_offset` | `f64` | Power-law strain offset `p₀` \[-\]. Upstream `materf(9,2)`. |
| `stress_at_proof_strain` | `f64` | Flow stress at `p_e`, `σ(p_e)` \[Pa\]. Upstream `materf(16,2)` (`spe`). |
| `slope_at_proof_strain` | `f64` | Slope of the power law at `p_e` \[Pa\]. Upstream `materf(13,2)`<br>(`penpe`). |
| `plateau_strain` | `f64` | Plastic strain at which the plateau ends, `p_k` \[-\]. Upstream<br>`materf(14,2)` (`pk`). |
| `plateau_stress` | `f64` | The plateau flow stress `κ·R_p0.2` \[Pa\]. |
| `used_fallback` | `bool` | `true` if the identification fell back to upstream's default branch<br>because the scalar equation had no root.<br><br>Upstream then sets `n = ε_u`, `p₀ = 0` and `K = R_m e^{ε_u}/ε_u^{ε_u}`,<br>which satisfies the UTS condition but **not** the proof-stress one — the<br>identified curve no longer passes through `R_p0.2`. Surfaced here rather<br>than hidden, because a silently mis-identified flow curve is exactly the<br>kind of plausible-looking wrong answer this port is meant to avoid. |

##### Implementations

###### Methods

- ```rust
  pub fn flow_stress(self: Self, p: f64) -> f64 { /* ... */ }
  ```
  Flow stress `σ_y(p)` \[Pa\] at accumulated plastic strain `p` \[-\].

- ```rust
  pub fn strain_at_flow_stress(self: Self, sigma: f64) -> f64 { /* ... */ }
  ```
  The plastic strain at which the flow stress first reaches `sigma`

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
    fn clone(self: &Self) -> Irrad3mHardening { /* ... */ }
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
    fn eq(self: &Self, other: &Irrad3mHardening) -> bool { /* ... */ }
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
#### Struct `Irrad3mState`

The internal state `IRRAD3M` carries between steps.

Upstream stores seven internal variables (`EPSPEQ`, `SEUIL`, `EPEQIRRA`,
`GONF`, `INDIPLAS`, `IRRA`, `TEMP`); the four that actually *evolve* and are
read back by the residuals are gathered here. The remaining three are either
diagnostics or copies of command variables the caller already has.

```rust
pub struct Irrad3mState {
    pub plastic_strain: f64,
    pub creep_driver: f64,
    pub irradiation_creep_strain: f64,
    pub swelling_strain: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `plastic_strain` | `f64` | Accumulated equivalent **plastic** strain `p` \[-\]. Upstream `EPSPEQ`. |
| `creep_driver` | `f64` | Accumulated stress-dose `η = ∫ σ_eq dΦ` \[Pa·dpa\]. Upstream `SEUIL`.<br><br>The incubation variable for irradiation creep; creep begins once it<br>passes [`Irrad3mParameters::creep_threshold`]. |
| `irradiation_creep_strain` | `f64` | Accumulated equivalent **irradiation-creep** strain `p_i` \[-\].<br>Upstream `EPEQIRRA`.<br><br>Tracked separately from the plastic strain because only the plastic part<br>hardens the material — irradiation creep does not move the flow curve. |
| `swelling_strain` | `f64` | Accumulated **linear** swelling strain \[-\]. Upstream `GONF`. |

##### Implementations

###### Methods

- ```rust
  pub fn advanced(self: Self, increment: &Irrad3mIncrement, swelling_increment: f64) -> Self { /* ... */ }
  ```
  The state at the end of a step, given what the step produced.

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
    fn clone(self: &Self) -> Irrad3mState { /* ... */ }
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
    fn default() -> Irrad3mState { /* ... */ }
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
    fn eq(self: &Self, other: &Irrad3mState) -> bool { /* ... */ }
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
#### Struct `Irrad3m`

The `IRRAD3M` law: parameters plus the hardening curve identified from them.

ASTER behaviour name: `IRRAD3M` (`num_lc = 30`, 7 state variables).
Upstream: `bibfor/algorith/irrmat.F90` (material preparation),
`bibfor/algorith/irrres.F90` (local residuals), reached through
`bibfor/lc/lc0030.F90` and the generic `plasti` driver — legacy symbols
`irrmat`, `irrres`, `lc0030`. Integration: `NEWTON` upstream.

Constructed with [`new`](Self::new) so the identification happens once, not
once per integration point per step.

```rust
pub struct Irrad3m {
    pub parameters: Irrad3mParameters,
    pub hardening: Irrad3mHardening,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `parameters` | `Irrad3mParameters` | The user's material parameters. |
| `hardening` | `Irrad3mHardening` | The hardening curve identified from them. |

##### Implementations

###### Methods

- ```rust
  pub fn new(parameters: Irrad3mParameters) -> Result<Self> { /* ... */ }
  ```
  Build the law, identifying the hardening curve once.

- ```rust
  pub const fn aster_name(self: Self) -> &'static str { /* ... */ }
  ```
  The upstream ASTER behaviour name.

- ```rust
  pub fn irradiation_creep_increment(self: Self, equivalent_stress_start: f64, equivalent_stress_end: f64, driver_start: f64, dose_increment: f64) -> (f64, f64) { /* ... */ }
  ```
  Irradiation-creep driver increment and creep increment for a candidate

- ```rust
  pub fn integrate(self: Self, trial_stress: SymmTensor, shear_modulus: f64, state: Irrad3mState, equivalent_stress_start: f64, dose_increment: f64) -> Result<Irrad3mIncrement> { /* ... */ }
  ```
  Integrate one step: plasticity and irradiation creep together.

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
    fn clone(self: &Self) -> Irrad3m { /* ... */ }
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
    fn eq(self: &Self, other: &Irrad3m) -> bool { /* ... */ }
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
#### Struct `Irrad3mIncrement`

What one `IRRAD3M` step produced.

```rust
pub struct Irrad3mIncrement {
    pub plastic_increment: f64,
    pub irradiation_creep_increment: f64,
    pub creep_driver_increment: f64,
    pub strain_increment: outram_foam_basic_lib::primitives::SymmTensor,
    pub stress: outram_foam_basic_lib::primitives::SymmTensor,
    pub equivalent_stress: f64,
    pub iterations: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `plastic_increment` | `f64` | Equivalent **plastic** strain increment `Δp` \[-\], non-negative. |
| `irradiation_creep_increment` | `f64` | Equivalent **irradiation-creep** strain increment `Δp_i` \[-\],<br>non-negative. |
| `creep_driver_increment` | `f64` | Increment of the stress-dose driver `Δη` \[Pa·dpa\]. |
| `strain_increment` | `outram_foam_basic_lib::primitives::SymmTensor` | Combined inelastic strain increment tensor \[-\], `(Δp + Δp_i)·(3/2)s/σ_eq`.<br><br>Deviatoric: neither plastic flow nor irradiation creep changes volume.<br>Swelling is *not* included — it is a separate, stress-free eigenstrain<br>obtained from<br>[`Irrad3mParameters::swelling_strain_increment`]. |
| `stress` | `outram_foam_basic_lib::primitives::SymmTensor` | Stress at the end of the step \[Pa\]. |
| `equivalent_stress` | `f64` | Von Mises equivalent of [`stress`](Self::stress) \[Pa\]. |
| `iterations` | `usize` | Local-solver iterations used. Zero when the step was purely elastic. |

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
    fn clone(self: &Self) -> Irrad3mIncrement { /* ... */ }
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
    fn eq(self: &Self, other: &Irrad3mIncrement) -> bool { /* ... */ }
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
#### Struct `HillAnisotropy`

A Hill quadratic form — the anisotropic replacement for von Mises.

# Why anisotropy is not optional for cladding

Zircaloy tubing is drawn and pilgered, which leaves the hexagonal grains
strongly textured: the basal poles point predominantly radially. The tube is
therefore *not* the same material in the hoop, axial and radial directions,
and a von Mises law — which assumes it is — gets the *direction* of creep
wrong even when it gets the magnitude right. For a cladding tube creeping
down onto a pellet under external coolant pressure, the direction is the
answer.

# The form

Hill 1948 replaces the von Mises equivalent with a general
pressure-insensitive quadratic,

`σ_H = sqrt(σ : M : σ)`

where `M` is a fourth-order tensor with the same symmetries and the same
null space (the hydrostatic direction) as `(3/2)P_dev`. Six independent
coefficients survive those constraints, and they are exactly the six
upstream tabulates.

# Coefficient meaning, and the von Mises check that pins it

Each field below is the corresponding **diagonal component of `M`** in the
material frame. Setting `M` to its isotropic value `(3/2)P_dev` gives

- normal components `M_xxxx = M_yyyy = M_zzzz = 3/2 · (1 - 1/3) = 1`
- shear components `M_xyxy = M_xzxz = M_yzyz = 3/2 · 1/2 = 3/4`

and [`VON_MISES`](Self::VON_MISES) carries exactly those numbers. That the
resulting `σ_H` reproduces `sqrt(3/2 s:s)` on a general stress state is the
check that fixes the convention beyond doubt, and it is a test in this
module rather than an assertion here.

# Expanded form

With `F = (M_xx + M_yy - M_zz)/2`, `G = (-M_xx + M_yy + M_zz)/2` and
`H = (M_xx - M_yy + M_zz)/2` — upstream's `H_F`, `H_G`, `H_H` — the
quadratic is

`σ_H² = F(σ_xx - σ_yy)² + G(σ_yy - σ_zz)² + H(σ_xx - σ_zz)² + 4(M_xy σ_xy² + M_xz σ_xz² + M_yz σ_yz²)`

which is manifestly zero on a hydrostatic stress, as a plastic-flow
potential for a metal must be.

# Units

All six coefficients are **dimensionless** ratios; `σ_H` carries the unit of
the stress passed in, i.e. pascal.

# The frame these are expressed in

The material frame, not the global one. For a cladding tube upstream names
the axes `(R, T, Z)` — radial, hoop, axial. This port takes the tensor in
whatever frame the caller works in and does not rotate; wiring the material
frame is the caller's job, exactly as `AFFE_CARA_ELEM/MASSIF` is upstream's.

```rust
pub struct HillAnisotropy {
    pub m_xx: f64,
    pub m_yy: f64,
    pub m_zz: f64,
    pub m_xy: f64,
    pub m_xz: f64,
    pub m_yz: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `m_xx` | `f64` | `M_xxxx` \[-\] — upstream `M_RR_RR` for a tube. |
| `m_yy` | `f64` | `M_yyyy` \[-\] — upstream `M_TT_TT` for a tube in 3-D. |
| `m_zz` | `f64` | `M_zzzz` \[-\] — upstream `M_ZZ_ZZ` for a tube in 3-D. |
| `m_xy` | `f64` | `M_xyxy` \[-\].<br><br>Which upstream keyword lands here is not obvious — see<br>[`from_aster_3d`](Self::from_aster_3d), which reproduces upstream's<br>mapping and documents an apparent transposition in it. |
| `m_xz` | `f64` | `M_xzxz` \[-\]. |
| `m_yz` | `f64` | `M_yzyz` \[-\]. |

##### Implementations

###### Methods

- ```rust
  pub const fn from_aster_3d(m_rr_rr: f64, m_tt_tt: f64, m_zz_zz: f64, m_rt_rt: f64, m_rz_rz: f64, m_tz_tz: f64) -> Self { /* ... */ }
  ```
  Build from upstream's six `META_LEMA_ANI` material keywords, using the

- ```rust
  pub fn fgh(self: Self) -> (f64, f64, f64) { /* ... */ }
  ```
  The `(F, G, H)` triple of upstream's `H_F`, `H_G`, `H_H`.

- ```rust
  pub fn contract(self: Self, sigma: SymmTensor) -> SymmTensor { /* ... */ }
  ```
  The contraction `M : σ` \[Pa\] — the gradient of `σ_H²/2`.

- ```rust
  pub fn equivalent_stress(self: Self, sigma: SymmTensor) -> f64 { /* ... */ }
  ```
  Hill equivalent stress `σ_H = sqrt(σ : M : σ)` \[Pa\].

- ```rust
  pub fn flow_direction(self: Self, sigma: SymmTensor) -> SymmTensor { /* ... */ }
  ```
  The flow direction `n = (M : σ)/σ_H` \[-\].

- ```rust
  pub fn blend(self: Self, other: Self, za: f64) -> Self { /* ... */ }
  ```
  Linear blend `za·self + (1-za)·other`, coefficient by coefficient.

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
    fn clone(self: &Self) -> HillAnisotropy { /* ... */ }
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
    fn eq(self: &Self, other: &HillAnisotropy) -> bool { /* ... */ }
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
#### Struct `MetaLemaAniPhase`

Viscoplastic parameters for one metallurgical phase of `META_LEMA_ANI`.

# The flow rule these parametrise

Each phase contributes a viscous stress

`σ_v,i = γ_i · p^{m_i} · (ṗ)^{1/n_i}`,  `γ_i = a_i · exp(Q_i/(n_i·T))`

and the law is satisfied when `σ_H = Σ_i f_i σ_v,i` over the three phases.
Inverting a single phase gives `ṗ = (σ_H/(γ p^m))^{n}` — a Lemaitre law, with
`γ` in the role of the reference stress `K` and `m` the strain-hardening
exponent.

# Why `Q` is divided by `n`

Exactly as in
[`LemaitreIrradiation`](super::viscoplastic::ViscoplasticLaw::LemaitreIrradiation):
because the rate goes as `γ^{-n}`, the `1/n` cancels and the *rate* carries a
clean `exp(-Q/(R·T))`. Transcribing `γ` without it gives an Arrhenius
exponent `n` times too large — wrong by orders of magnitude, and invisible to
any dimensional check.

# Units

```rust
pub struct MetaLemaAniPhase {
    pub amplitude: f64,
    pub hardening_exponent: f64,
    pub stress_exponent: f64,
    pub activation_temperature: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `amplitude` | `f64` | Reference-stress amplitude `a` \[Pa·s^{1/n}\]. Upstream `F1_A`, `F2_A`,<br>`C_A`.<br><br>The fractional-power time unit is not a typo: `γ` multiplies `ṗ^{1/n}`,<br>so `a` must carry `s^{1/n}` for the product to be a stress. |
| `hardening_exponent` | `f64` | Strain-hardening exponent `m` \[-\]. Upstream `F1_M`, `F2_M`, `C_M`.<br><br>Positive `m` raises the flow stress as strain accumulates, so the rate<br>*falls* — primary creep. Note the sign convention is opposite to<br>[`LemaitreParameters::m`](super::viscoplastic::LemaitreParameters::m),<br>which enters as `p^{-n/m}`. |
| `stress_exponent` | `f64` | Stress exponent `n` \[-\]. Upstream `F1_N`, `F2_N`, `C_N`. Strictly<br>positive. |
| `activation_temperature` | `f64` | Activation temperature `Q/R` \[K\]. Upstream `F1_Q`, `F2_Q`, `C_Q`. |

##### Implementations

###### Methods

- ```rust
  pub fn reference_stress(self: Self, temperature: f64) -> f64 { /* ... */ }
  ```
  The reference stress `γ = a·exp(Q/(n·T))` \[Pa·s^{1/n}\] at temperature

- ```rust
  pub fn viscous_stress(self: Self, temperature: f64, accumulated_strain: f64, strain_rate: f64) -> f64 { /* ... */ }
  ```
  Viscous stress `σ_v = γ p^m ṗ^{1/n}` \[Pa\] of this phase alone.

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
    fn clone(self: &Self) -> MetaLemaAniPhase { /* ... */ }
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
    fn eq(self: &Self, other: &MetaLemaAniPhase) -> bool { /* ... */ }
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
#### Struct `MetaLemaAni`

The `META_LEMA_ANI` law — anisotropic Lemaitre creep of Zircaloy with
metallurgical phase dependence.

ASTER behaviour name: `META_LEMA_ANI` (`num_lc = 58`). Declared upstream as
a `LoiComportementMFront`, so
[`AsterBehaviour::MetaLemaAni.is_mfront()`](super::catalogue::AsterBehaviour::is_mfront)
is `true` and the algorithm lives in `mfront/META_LEMA_ANI.mfront` rather
than in a Fortran subroutine. Upstream integration: `NEWTON_PERT` on the
full implicit system with a numerical Jacobian. Documentation: `R4.04.04`
(metallurgy) and `R4.04.05` (mechanics).

# What the law is for

Fuel cladding during a loss-of-coolant accident. On the temperature ramp the
tube passes through the α → β transformation of zirconium, and the two
phases creep utterly differently — β-Zr, body-centred cubic and hot, is
orders of magnitude softer and very much less anisotropic than textured
α-Zr. Ballooning and burst are therefore governed by *where in the
transformation the tube is* when the stress arrives, which is why a
mechanical law here has to carry metallurgy.

# The three phases

Upstream carries three parameter sets, blended by weights that depend on the
α fraction `Za = 1 - Zb`:

| Set | Upstream prefix | Active when |
|---|---|---|
| [`alpha`](Self::alpha) | `F1_` | `Za ≥ 0.99` — essentially pure α |
| [`mixed`](Self::mixed) | `F2_` | `0.1 ≤ Za ≤ 0.9` — the two-phase field |
| [`beta`](Self::beta) | `C_` | `Za ≤ 0.01` — essentially pure β |

with linear ramps across the narrow bands between. See
[`phase_weights`](Self::phase_weights).

# What is not here

The **kinetics of `Zb` itself**. Upstream integrates the β fraction as a
fourth state variable, with separate heating and cooling laws and a
rate-dependent transformation-onset temperature (`R4.04.04`, and the
standalone `ZIRC` / `ZIRC_META` behaviours in
`bibfor/metallurgy/zedgar.F90`). This port takes `Zb` as an **input** to
every method, so a caller can drive it from any phase model — including a
future port of `ZIRC` — without this law having an opinion. That is a real
gap, and it is stated rather than papered over.

```rust
pub struct MetaLemaAni {
    pub alpha: MetaLemaAniPhase,
    pub mixed: MetaLemaAniPhase,
    pub beta: MetaLemaAniPhase,
    pub alpha_anisotropy: HillAnisotropy,
    pub beta_anisotropy: HillAnisotropy,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `alpha` | `MetaLemaAniPhase` | Viscoplastic parameters of the α-phase set (upstream `F1_`). |
| `mixed` | `MetaLemaAniPhase` | Viscoplastic parameters of the two-phase set (upstream `F2_`). |
| `beta` | `MetaLemaAniPhase` | Viscoplastic parameters of the β-phase set (upstream `C_`). |
| `alpha_anisotropy` | `HillAnisotropy` | Hill coefficients of the α phase (upstream `F_M..`). |
| `beta_anisotropy` | `HillAnisotropy` | Hill coefficients of the β phase (upstream `C_M..`).<br><br>β-Zr is cubic and close to isotropic, so this is usually near<br>[`HillAnisotropy::VON_MISES`]. |

##### Implementations

###### Methods

- ```rust
  pub const fn aster_name(self: Self) -> &'static str { /* ... */ }
  ```
  The upstream ASTER behaviour name.

- ```rust
  pub fn phase_weights(za: f64) -> (f64, f64, f64) { /* ... */ }
  ```
  Blending weights `(f_α, f_mixed, f_β)` for the α fraction `za` \[-\].

- ```rust
  pub fn anisotropy_at(self: Self, beta_fraction: f64) -> HillAnisotropy { /* ... */ }
  ```
  The Hill coefficients in force at β-phase fraction `beta_fraction`

- ```rust
  pub fn viscous_stress(self: Self, beta_fraction: f64, temperature: f64, accumulated_strain: f64, strain_rate: f64) -> f64 { /* ... */ }
  ```
  The blended viscous stress `σ_v = Σ_i f_i γ_i p^{m_i} ṗ^{1/n_i}` \[Pa\].

- ```rust
  pub fn integrate(self: Self, trial_stress: SymmTensor, shear_modulus: f64, beta_fraction: f64, temperature: f64, accumulated_strain: f64, dt: f64) -> Result<MetaLemaAniIncrement> { /* ... */ }
  ```
  Integrate one step with an **anisotropic** return.

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
    fn clone(self: &Self) -> MetaLemaAni { /* ... */ }
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
    fn eq(self: &Self, other: &MetaLemaAni) -> bool { /* ... */ }
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
#### Struct `MetaLemaAniIncrement`

What one `META_LEMA_ANI` step produced.

```rust
pub struct MetaLemaAniIncrement {
    pub equivalent_increment: f64,
    pub strain_increment: outram_foam_basic_lib::primitives::SymmTensor,
    pub stress: outram_foam_basic_lib::primitives::SymmTensor,
    pub equivalent_stress: f64,
    pub iterations: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `equivalent_increment` | `f64` | Equivalent viscoplastic strain increment `Δp` \[-\], non-negative. |
| `strain_increment` | `outram_foam_basic_lib::primitives::SymmTensor` | Viscoplastic strain increment tensor \[-\], `Δp · n` with `n` the **Hill**<br>flow direction.<br><br>Deviatoric — the Hill contraction is traceless — but *not* parallel to<br>the stress deviator unless the material is isotropic. |
| `stress` | `outram_foam_basic_lib::primitives::SymmTensor` | Stress at the end of the step \[Pa\]. |
| `equivalent_stress` | `f64` | Hill equivalent of [`stress`](Self::stress) \[Pa\]. |
| `iterations` | `usize` | Local-solver iterations used. |

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
    fn clone(self: &Self) -> MetaLemaAniIncrement { /* ... */ }
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
    fn eq(self: &Self, other: &MetaLemaAniIncrement) -> bool { /* ... */ }
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
### Constants and Statics

#### Constant `IRRAD3M_PROOF_STRAIN`

The plastic strain at which upstream anchors the proof stress, `p_e = 2e-3`.

Hard-coded in `irrmat.F90` as `data pe/2.0d-3/` and **not** a user
parameter, despite the name `R02` implying 0.2 % — which is exactly this
value.

```rust
pub const IRRAD3M_PROOF_STRAIN: f64 = 2.0e-3;
```

## Module `viscochab`

`VISCOCHAB` — unified viscoplasticity with two back stresses, static
recovery and a strain-memory surface.

# What this law is for

A reactor component held hot under load does three things at once that a
simple creep law cannot describe together: it flows at a rate set by how far
the stress exceeds a threshold (viscoplasticity), it *remembers* the
direction it was last loaded in, so reversing the load yields early
(kinematic hardening), and while it sits it slowly forgets — thermal
recovery erodes the hardening it built up. `VISCOCHAB` is EDF's model for
exactly that combination, which is why it is the law reached for on vessel
and piping steels under thermal-mechanical cycling and creep-fatigue holds.

# Why this module looks nothing like [`chaboche`](super::chaboche)

The `VMIS_*_CHAB` family in [`chaboche`](super::chaboche) is *rate
independent*: a yield surface plus a consistency condition, which collapses
to one scalar unknown per step and is solved by radial return. `VISCOCHAB`
has no consistency condition — the overstress drives an explicit flow rate,
and every internal variable evolves by its own differential equation. There
is nothing to collapse. Upstream reflects this by offering a `RUNGE_KUTTA`
integration path (`algo_inte`), and that is the path ported here: the 27
coupled rates of `rkdcha.F90`, integrated by
[`outram_foam_basic_lib::ode::OdeIntegrator`].

# The state — 27 rates in a 28-slot vector

Upstream declares `nb_vari = 28` (`viscochab.py`), of which 27 evolve. In
upstream storage order:

| Slots | Symbol | Meaning | Unit |
|---|---|---|---|
| 1-6 | `evi` | viscoplastic strain `ε^vi` | - |
| 7-12 | `a1v` | first back-strain `α₁` | - |
| 13-18 | `a2v` | second back-strain `α₂` | - |
| 19-24 | `csi` | memory-surface centre `ξ` | - |
| 25 | `rayvi` | isotropic hardening `R` | Pa |
| 26 | `qcum` | memory-surface radius `q` | - |
| 27 | `evcum` | accumulated equivalent viscoplastic strain `p` | - |
| 28 | — | integration-state indicator, rate identically zero | - |

All six-component tensors are in code_aster's Mandel convention — ordering
`(XX, YY, ZZ, XY, XZ, YZ)` with the shear entries scaled by `√2`, so that a
plain dot product *is* the tensor double contraction. Use
[`AsterVoigt`] to convert; constructing the
six numbers by hand without the scaling is the classic way to get this
wrong.

# The equations, as upstream writes them

With `s` the stress deviator, `X_i = (2/3)·C_i·α_i` the back stresses, and
`n̂` the unit direction of the effective deviator:

- effective deviator `smx = s - (2/3)(C₁α₁ + C₂α₂)`, equivalent
  `J = √(3/2 · smx:smx)`
- overstress `F = J - R - K`; **no flow at all** when `F ≤ 0`
- flow rate `ṗ = (F/(K₀ + A_K·R))^N · exp(ALP·(F/(K₀+A_K·R))^(N+1))`
- `ε̇^vi = (3/2)·(smx/J)·ṗ`, so `√(2/3 · ε̇^vi:ε̇^vi) = ṗ` exactly
- `α̇_i = ε̇^vi − γ_i·[D_i·α_i + (1−D_i)(α_i·n̂)n̂]·ṗ − G_Xi·‖X_i‖^(M_i−1)·α_i`
- `γ_i = G_i0·(A_I + (1−A_I)·e^(−B·p))`
- `Ṙ = B·(Q(q) − R)·ṗ + G_R·sign(Q_R − R)·|Q_R − R|^(M_R)`
- memory surface: `q̇ = ETA·(n̂·n̂*)·ṗ` and `ξ̇ = √(3/2)(1−ETA)(n̂·n̂*)·ṗ·n̂*`,
  active only while `√(2/3 · ‖ε^vi − ξ‖²_vM) > q` and `n̂·n̂* > 0`

# The implicit reference rate of 1 s⁻¹

`ṗ = (F/(K₀ + A_K R))^N` is dimensionally a pure number, not a rate.
Upstream's implicit path makes the missing factor explicit — `cvmres.F90`
writes `Δp = Δt·(F/K)^N` — so the parameterisation carries an **implicit
reference rate of 1 s⁻¹**, and `K₀` is only a stress if time is measured in
seconds. The same applies to `G_R`, `G_X1` and `G_X2`, whose units
(`Pa^(1−M)/s`) absorb the time unit. Feeding this law a timestep in hours
and expecting hours out will be wrong by the ratio to the fourth or fifth
power, silently.

# Two places where upstream's explicit and implicit paths disagree

Both were found by transcribing `rkdcha.F90` and `cvmres.F90` side by side.
This port reproduces **`rkdcha.F90`**, because that is the routine the
`RUNGE_KUTTA` algorithm actually runs, and pins both differences with tests
rather than silently correcting them — see the workspace rule on upstream
defects.

1. **`rkdcha.F90` line 124 uses `(1 − D1)` in the `α₂` equation.**
   Upstream reads

   ```text
   da1v(itens) = d1*a1v(itens)+(1.0d0-d1)*xna1v*petin(itens)
   da2v(itens) = d2*a2v(itens)+(1.0d0-d1)*xna2v*petin(itens)
   ```

   The second line's `d2*a2v` establishes that this is the `α₂` equation, so
   the `(1.0d0-d1)` immediately after it is inconsistent with its own line
   and with the `α₁` line above. `cvmres.F90`'s `JF` block — the same
   physics, integrated implicitly — uses `(1.d0-d2)` there
   (`zz = zz*(1.d0-d2)*g20*ccin*dp*2.d0/3.d0`), and every other term in the
   two blocks maps one-to-one once `X_i = (2/3)C_iα_i` is substituted. The
   verdict recorded here is therefore **an upstream typo in the explicit
   path**. It is reproduced verbatim; [`RKDCHA_ALPHA2_USES_D1`] marks it and
   `rkdcha_alpha2_reuses_d1_upstream_typo` measures the resulting
   discrepancy.

2. **`rkdcha.F90` zeroes *every* rate when `F ≤ 0`,** including the static
   recovery of `R`. `cvmres.F90`'s `RF` keeps its recovery term
   `sgn·G_R·Δt·|Q_R − R|^(M_R)` regardless of whether `Δp` is zero. So the
   two upstream paths predict different behaviour during an elastic hold:
   explicit recovers nothing, implicit recovers. This is structural rather
   than a slip of a subscript, so no "typo" verdict is claimed — it is
   recorded and pinned by `elastic_branch_zeroes_every_rate`.

# What is *not* ported

- **`A_R` (coefficient 3).** `cvmcvx.F90` forms the threshold as
  `J − A_R·R − K`; `rkdcha.F90` forms it as `J − R − K`, i.e. it hard-codes
  `A_R = 1`. The explicit path is what is ported, so `A_R` is accepted,
  stored and ignored, exactly as upstream ignores it.
- **Thermal strain, damage coupling, orthotropic elasticity and the
  `C_PLAN` branch** of `calsig.F90`. [`ViscoplasticChabocheSystem`] ports
  the isothermal, isotropic, 3-D branch only.
- **The tangent operator.** Upstream offers `PERTURBATION` /
  `VERIFICATION` only for this law; nothing analytic exists to port.
- **Any Jacobian.** [`OdeSystem::jacobian`] is left at its panicking
  default, so this system must be integrated with an *explicit* stepper
  ([`OdeSolver::rkf45`] or [`OdeSolver::euler`]), matching upstream's
  `RUNGE_KUTTA` path. Selecting [`OdeSolver::rosenbrock23`] will panic.

# Status

**Verification-tested draft; not validated.** Every test here is an
independent check of the transcription — closed-form saturation limits,
tensor invariants, and the two upstream discrepancies above. Nothing has
been compared against code_aster output or against a measured creep-fatigue
curve, and no such agreement is claimed.

```rust
pub mod viscochab { /* ... */ }
```

### Types

#### Struct `ViscoplasticChabocheParameters`

The 25 material coefficients of `VISCOCHAB`.

# Units and the implicit second

Stress-like coefficients are in pascal; exponents and fractions are
dimensionless. Three coefficients — `static_recovery_rate_r`,
`static_recovery_rate_x1`, `static_recovery_rate_x2` — carry
`Pa^(1−M)·s⁻¹`, and the flow rate itself carries an implicit `1 s⁻¹` (see
the module docs). **Time must be in seconds.**

# Field naming

Rust names are descriptive; the upstream keyword is given for every field so
a deck can be read across. Order matches
[`ASTER_COEFFICIENT_NAMES`].

```rust
pub struct ViscoplasticChabocheParameters {
    pub drag_stress: f64,
    pub drag_hardening_coupling: f64,
    pub threshold_hardening_multiplier: f64,
    pub initial_threshold: f64,
    pub flow_exponent: f64,
    pub exponential_flow_coefficient: f64,
    pub isotropic_rate: f64,
    pub static_recovery_exponent_r: f64,
    pub static_recovery_rate_r: f64,
    pub memory_saturation_rate: f64,
    pub hardening_saturation_max: f64,
    pub hardening_saturation_min: f64,
    pub recovery_target_offset: f64,
    pub memory_split: f64,
    pub back_stress_modulus_1: f64,
    pub static_recovery_exponent_x1: f64,
    pub back_stress_recovery_split_1: f64,
    pub static_recovery_rate_x1: f64,
    pub dynamic_recovery_1: f64,
    pub back_stress_modulus_2: f64,
    pub static_recovery_exponent_x2: f64,
    pub back_stress_recovery_split_2: f64,
    pub static_recovery_rate_x2: f64,
    pub dynamic_recovery_2: f64,
    pub dynamic_recovery_floor: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `drag_stress` | `f64` | `K_0` — viscous drag stress at zero isotropic hardening \[Pa\].<br>Strictly positive; it is the denominator of the flow rate. |
| `drag_hardening_coupling` | `f64` | `A_K` — how much the isotropic hardening `R` adds to the drag \[-\].<br>Typically in `[0, 1]`. The effective drag is `K₀ + A_K·R`. |
| `threshold_hardening_multiplier` | `f64` | `A_R` — multiplier on `R` in the threshold \[-\].<br><br>**Stored but unused**: `rkdcha.F90` hard-codes `A_R = 1` while<br>`cvmcvx.F90` honours it. Kept so a deck round-trips and so the<br>difference is visible rather than lost. |
| `initial_threshold` | `f64` | `K` — initial elastic threshold \[Pa\], non-negative. Flow occurs only<br>where the effective equivalent stress exceeds `R + K`. |
| `flow_exponent` | `f64` | `N` — Norton exponent of the flow rate \[-\]. Typically 3-20; larger<br>means a sharper, more nearly rate-independent response. |
| `exponential_flow_coefficient` | `f64` | `ALP` — exponential-overstress coefficient \[-\], non-negative.<br><br>Adds `exp(ALP·(F/K)^(N+1))` to the power law, letting the rate rise<br>faster than any power at high overstress. Upstream skips the factor<br>entirely when `ALP ≤ 1e-30`, and so does this port. |
| `isotropic_rate` | `f64` | `B` — rate of isotropic saturation and of kinematic-recovery decay<br>\[-\]. Enters both `Ṙ` and `γ_i(p)`. |
| `static_recovery_exponent_r` | `f64` | `M_R` — exponent of the static (time) recovery of `R` \[-\]. |
| `static_recovery_rate_r` | `f64` | `G_R` — coefficient of the static recovery of `R` \[Pa^(1−M_R)·s⁻¹\].<br>Zero disables time recovery of the isotropic hardening. |
| `memory_saturation_rate` | `f64` | `MU` — controls how fast the asymptotic hardening `Q` follows the memory<br>radius `q` \[-\], through `1 − exp(−2·MU·q)`. |
| `hardening_saturation_max` | `f64` | `Q_M` — asymptotic isotropic hardening at a fully developed memory<br>surface \[Pa\]. **Must be non-zero**: upstream divides by it when<br>forming `Q_R`. |
| `hardening_saturation_min` | `f64` | `Q_0` — asymptotic isotropic hardening at zero memory radius \[Pa\]. |
| `recovery_target_offset` | `f64` | `QR_0` — amplitude of the recovery target offset \[Pa\], entering<br>`Q_R = Q − QR_0·(1 − ((Q_M − Q)/Q_M)²)`. |
| `memory_split` | `f64` | `ETA` — split of the memory-surface evolution between its radius `q` and<br>its centre `ξ` \[-\], in `[0, 1]`. `ETA = 1` freezes the centre and, in<br>the implicit path, disables the memory surface altogether. |
| `back_stress_modulus_1` | `f64` | `C1` — modulus of the first back stress \[Pa\], through<br>`X₁ = (2/3)·C1·α₁`. |
| `static_recovery_exponent_x1` | `f64` | `M_1` — exponent of the static recovery of `X₁` \[-\]. |
| `back_stress_recovery_split_1` | `f64` | `D1` — split of the first back stress's dynamic recovery between its<br>isotropic part `α₁` and its radial part `(α₁·n̂)n̂` \[-\], in `[0, 1]`.<br>`D1 = 1` gives ordinary Armstrong-Frederick recovery. |
| `static_recovery_rate_x1` | `f64` | `G_X1` — coefficient of the static recovery of `X₁`<br>\[Pa^(1−M_1)·s⁻¹\]. |
| `dynamic_recovery_1` | `f64` | `G1_0` — dynamic-recovery coefficient `γ₁` at zero accumulated strain<br>\[-\]. The saturated back stress is `C1/γ₁` in equivalent measure. |
| `back_stress_modulus_2` | `f64` | `C2` — modulus of the second back stress \[Pa\]. |
| `static_recovery_exponent_x2` | `f64` | `M_2` — exponent of the static recovery of `X₂` \[-\]. |
| `back_stress_recovery_split_2` | `f64` | `D2` — split of the second back stress's dynamic recovery \[-\].<br><br>**Reproduces an upstream defect**: `rkdcha.F90` applies `D2` to the<br>`α₂` term but then uses `(1 − D1)`, not `(1 − D2)`, for the radial part.<br>See [`RKDCHA_ALPHA2_USES_D1`]. |
| `static_recovery_rate_x2` | `f64` | `G_X2` — coefficient of the static recovery of `X₂`<br>\[Pa^(1−M_2)·s⁻¹\]. |
| `dynamic_recovery_2` | `f64` | `G2_0` — dynamic-recovery coefficient `γ₂` at zero accumulated strain<br>\[-\]. |
| `dynamic_recovery_floor` | `f64` | `A_I` — floor of the dynamic-recovery decay \[-\], in `[0, 1]`.<br>`γ_i(p) = G_i0·(A_I + (1 − A_I)·e^(−B·p))`, so `A_I = 1` freezes `γ_i`<br>at `G_i0`. |

##### Implementations

###### Methods

- ```rust
  pub const fn from_aster_coefficients(coeft: [f64; 25]) -> Self { /* ... */ }
  ```
  Read the 25 coefficients from an upstream `coeft(1..25)` array.

- ```rust
  pub const fn to_aster_coefficients(self: Self) -> [f64; 25] { /* ... */ }
  ```
  The 25 coefficients back in upstream's `coeft(1..25)` order.

- ```rust
  pub fn validate(self: &Self) -> Result<()> { /* ... */ }
  ```
  Reject parameter sets the rate function cannot evaluate.

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
    fn clone(self: &Self) -> ViscoplasticChabocheParameters { /* ... */ }
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
    fn eq(self: &Self, other: &ViscoplasticChabocheParameters) -> bool { /* ... */ }
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
#### Struct `ViscoplasticChabocheState`

The 27 evolving internal variables of `VISCOCHAB`.

Tensor fields are in code_aster's Mandel convention — see the module docs.
A pristine material is [`ViscoplasticChabocheState::undeformed`].

```rust
pub struct ViscoplasticChabocheState {
    pub viscoplastic_strain: crate::rheology::aster::kinematics::AsterVoigt,
    pub back_strain_1: crate::rheology::aster::kinematics::AsterVoigt,
    pub back_strain_2: crate::rheology::aster::kinematics::AsterVoigt,
    pub memory_centre: crate::rheology::aster::kinematics::AsterVoigt,
    pub isotropic_hardening: f64,
    pub memory_radius: f64,
    pub accumulated_strain: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `viscoplastic_strain` | `crate::rheology::aster::kinematics::AsterVoigt` | `ε^vi` — viscoplastic strain \[-\], upstream `vini(1:6)`. Deviatoric:<br>viscoplastic flow preserves volume. |
| `back_strain_1` | `crate::rheology::aster::kinematics::AsterVoigt` | `α₁` — first back-strain \[-\], upstream `vini(7:12)`. The back stress<br>is `X₁ = (2/3)·C1·α₁` \[Pa\]. |
| `back_strain_2` | `crate::rheology::aster::kinematics::AsterVoigt` | `α₂` — second back-strain \[-\], upstream `vini(13:18)`. |
| `memory_centre` | `crate::rheology::aster::kinematics::AsterVoigt` | `ξ` — centre of the strain-memory surface \[-\], upstream<br>`vini(19:24)`. |
| `isotropic_hardening` | `f64` | `R` — isotropic hardening \[Pa\], upstream `vini(25)`. Adds to the<br>elastic threshold and to the viscous drag. |
| `memory_radius` | `f64` | `q` — radius of the strain-memory surface \[-\], upstream `vini(26)`.<br>Non-negative; grows only when the strain path leaves the surface. |
| `accumulated_strain` | `f64` | `p` — accumulated equivalent viscoplastic strain \[-\], upstream<br>`vini(27)`. Monotone non-decreasing. |

##### Implementations

###### Methods

- ```rust
  pub fn undeformed() -> Self { /* ... */ }
  ```
  The pristine state: no strain, no hardening, no memory.

- ```rust
  pub fn from_ode_state(y: &[f64]) -> Self { /* ... */ }
  ```
  Unpack from the flat 27-element ODE state vector.

- ```rust
  pub fn to_ode_state(self: Self) -> Vec<f64> { /* ... */ }
  ```
  Pack into the flat 27-element ODE state vector, in upstream's order.

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
    fn clone(self: &Self) -> ViscoplasticChabocheState { /* ... */ }
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
    fn default() -> ViscoplasticChabocheState { /* ... */ }
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
    fn eq(self: &Self, other: &ViscoplasticChabocheState) -> bool { /* ... */ }
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
#### Struct `ViscoplasticChabocheRates`

Time derivatives of the 27 internal variables — the output of
[`ViscoplasticChabocheWithMemory::internal_variable_rates`].

Units are those of the matching [`ViscoplasticChabocheState`] field divided
by seconds.

```rust
pub struct ViscoplasticChabocheRates {
    pub viscoplastic_strain_rate: crate::rheology::aster::kinematics::AsterVoigt,
    pub back_strain_1_rate: crate::rheology::aster::kinematics::AsterVoigt,
    pub back_strain_2_rate: crate::rheology::aster::kinematics::AsterVoigt,
    pub memory_centre_rate: crate::rheology::aster::kinematics::AsterVoigt,
    pub isotropic_hardening_rate: f64,
    pub memory_radius_rate: f64,
    pub accumulated_strain_rate: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `viscoplastic_strain_rate` | `crate::rheology::aster::kinematics::AsterVoigt` | `ε̇^vi` \[1/s\], upstream `devi`. Deviatoric. |
| `back_strain_1_rate` | `crate::rheology::aster::kinematics::AsterVoigt` | `α̇₁` \[1/s\], upstream `da1v`. |
| `back_strain_2_rate` | `crate::rheology::aster::kinematics::AsterVoigt` | `α̇₂` \[1/s\], upstream `da2v`. |
| `memory_centre_rate` | `crate::rheology::aster::kinematics::AsterVoigt` | `ξ̇` \[1/s\], upstream `dcsi`. |
| `isotropic_hardening_rate` | `f64` | `Ṙ` \[Pa/s\], upstream `drayvi`. |
| `memory_radius_rate` | `f64` | `q̇` \[1/s\], upstream `dqcum`. |
| `accumulated_strain_rate` | `f64` | `ṗ` \[1/s\], upstream `devcum`. Non-negative. |

##### Implementations

###### Methods

- ```rust
  pub fn write_ode_derivatives(self: Self, dydx: &mut [f64]) { /* ... */ }
  ```
  Write the rates into a flat 27-element derivative vector, in upstream's

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
    fn clone(self: &Self) -> ViscoplasticChabocheRates { /* ... */ }
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
    fn default() -> ViscoplasticChabocheRates { /* ... */ }
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
    fn eq(self: &Self, other: &ViscoplasticChabocheRates) -> bool { /* ... */ }
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
#### Struct `ViscoplasticChabocheWithMemory`

Elasto-viscoplastic Lemaitre-Chaboche law with strain memory and static
recovery.

ASTER behaviour name: `VISCOCHAB` (`num_lc = 32`, 28 internal variables, 27
of them evolving). Upstream: `bibfor/algorith/rkdcha.F90` for the rates,
reached through `bibfor/comport/lcdvin.F90` and driven by
`bibfor/comport/rdif01.F90` — legacy symbols `rkdcha`, `lcdvin`, `rdif01`.
Integration: `RUNGE_KUTTA` (ported here), or `NEWTON` / `NEWTON_RELI` via
`cvmres`/`cvmjac` (not ported).

The law itself is stateless — it is the parameter set plus the rate
function. State lives in [`ViscoplasticChabocheState`], and integration in
[`ViscoplasticChabocheSystem`].

```rust
pub struct ViscoplasticChabocheWithMemory {
    pub parameters: ViscoplasticChabocheParameters,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `parameters` | `ViscoplasticChabocheParameters` | The 25 material coefficients. |

##### Implementations

###### Methods

- ```rust
  pub fn new(parameters: ViscoplasticChabocheParameters) -> Result<Self> { /* ... */ }
  ```
  Build the law from its coefficients, validating them.

- ```rust
  pub const fn aster_name(self: Self) -> &'static str { /* ... */ }
  ```
  The upstream ASTER behaviour name, `"VISCOCHAB"`.

- ```rust
  pub fn effective_deviator(self: Self, stress: AsterVoigt, state: &ViscoplasticChabocheState) -> (AsterVoigt, f64) { /* ... */ }
  ```
  The effective deviator `smx = dev(σ) − (2/3)(C₁α₁ + C₂α₂)` \[Pa\] and

- ```rust
  pub fn overstress(self: Self, stress: AsterVoigt, state: &ViscoplasticChabocheState) -> f64 { /* ... */ }
  ```
  The overstress `F = J − R − K` \[Pa\]. Flow occurs only where `F > 0`.

- ```rust
  pub fn flow_rate(self: Self, overstress: f64, isotropic_hardening: f64) -> f64 { /* ... */ }
  ```
  Equivalent viscoplastic strain rate `ṗ` \[1/s\] for a given overstress.

- ```rust
  pub fn internal_variable_rates(self: Self, stress: AsterVoigt, state: &ViscoplasticChabocheState) -> ViscoplasticChabocheRates { /* ... */ }
  ```
  The 27 internal-variable rates at a given stress and state — the direct

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
    fn clone(self: &Self) -> ViscoplasticChabocheWithMemory { /* ... */ }
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
    fn eq(self: &Self, other: &ViscoplasticChabocheWithMemory) -> bool { /* ... */ }
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
#### Struct `ViscoplasticChabocheSystem`

The `VISCOCHAB` rate system as a 27-equation
[`OdeSystem`], ready for
[`OdeIntegrator`].

# What the independent variable is

`x` is **time within the step**, running from `0` to
[`step_duration`](Self::step_duration) \[s\]. Upstream's driver `rdif01.F90`
uses the same convention and forms the total strain by linear interpolation,
`ε(x) = ε_start + Δε · x/Δt`; `calsig.F90` then gives the stress. Both are
reproduced here, isothermal and isotropic only (see the module docs for what
is left out).

# Why the stress is not part of the state

Under strain control the stress is a *function* of the state:
`σ = C:(ε(x) − ε^vi)`. Carrying it as an extra unknown would over-determine
the system; upstream recomputes it at every derivative evaluation, and so
does this port.

# Integrator choice

[`OdeSystem::jacobian`] is not implemented, so use
[`OdeSolver::rkf45`] or [`OdeSolver::euler`].
[`OdeSolver::rosenbrock23`] will panic.

```rust
pub struct ViscoplasticChabocheSystem {
    pub law: ViscoplasticChabocheWithMemory,
    pub young_modulus: f64,
    pub poisson_ratio: f64,
    pub total_strain_start: crate::rheology::aster::kinematics::AsterVoigt,
    pub total_strain_increment: crate::rheology::aster::kinematics::AsterVoigt,
    pub step_duration: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `law` | `ViscoplasticChabocheWithMemory` | The constitutive law and its 25 coefficients. |
| `young_modulus` | `f64` | Young's modulus `E` \[Pa\], strictly positive. |
| `poisson_ratio` | `f64` | Poisson's ratio `ν` \[-\], in `(-1, 0.5)`. |
| `total_strain_start` | `crate::rheology::aster::kinematics::AsterVoigt` | Total strain at the start of the step \[-\], Mandel convention. |
| `total_strain_increment` | `crate::rheology::aster::kinematics::AsterVoigt` | Total-strain increment over the step \[-\], Mandel convention. |
| `step_duration` | `f64` | Step duration `Δt` \[s\], strictly positive. |

##### Implementations

###### Methods

- ```rust
  pub fn new(law: ViscoplasticChabocheWithMemory, young_modulus: f64, poisson_ratio: f64, total_strain_start: AsterVoigt, total_strain_increment: AsterVoigt, step_duration: f64) -> Result<Self> { /* ... */ }
  ```
  Assemble a strain-driven `VISCOCHAB` system for one step.

- ```rust
  pub fn stress_at(self: &Self, x: f64, viscoplastic_strain: AsterVoigt) -> AsterVoigt { /* ... */ }
  ```
  Cauchy stress \[Pa\] at time `x` \[s\] within the step, for a given

- ```rust
  pub fn integrate_step(self: Self, state: ViscoplasticChabocheState, solver: OdeSolver, initial_step: f64) -> Result<ViscoplasticChabocheState> { /* ... */ }
  ```
  Integrate one step from `state`, returning the state at `Δt`.

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
    fn clone(self: &Self) -> ViscoplasticChabocheSystem { /* ... */ }
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

- **OdeSystem**
  - ```rust
    fn n_eqns(self: &Self) -> usize { /* ... */ }
    ```

  - ```rust
    fn derivatives(self: &Self, x: f64, y: &[f64], dydx: &mut Vec<f64>) { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ViscoplasticChabocheSystem) -> bool { /* ... */ }
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
### Constants and Statics

#### Constant `INTERNAL_VARIABLE_COUNT`

Number of internal variables upstream declares for `VISCOCHAB`
(`viscochab.py`, `nb_vari = 28`).

```rust
pub const INTERNAL_VARIABLE_COUNT: usize = 28;
```

#### Constant `ODE_EQUATION_COUNT`

Number of internal variables that actually evolve — the size of the ODE
system. The 28th is an integration-state indicator whose rate `rkdcha.F90`
sets identically to zero (`dvin(nvi) = detat`, `detat = 0`).

```rust
pub const ODE_EQUATION_COUNT: usize = 27;
```

#### Constant `RKDCHA_ALPHA2_USES_D1`

Whether this port reproduces the `(1 − D1)` that `rkdcha.F90` line 124 uses
in the **`α₂`** equation, where symmetry with line 122 and the implicit path
`cvmres.F90` both imply `(1 − D2)`.

Always `true`: upstream defects are reproduced, not silently corrected. See
the module documentation for the evidence and
`rkdcha_alpha2_reuses_d1_upstream_typo` for the measured size of the
difference. If a future upstream release fixes the line, this constant and
that test are the two places to change.

```rust
pub const RKDCHA_ALPHA2_USES_D1: bool = true;
```

#### Constant `ASTER_COEFFICIENT_NAMES`

The 25 `VISCOCHAB` material keywords, in the order upstream stores them in
`materf(1..25, 2)`.

`cvmmat.F90` fills those 25 slots from `nomc(4..28)`, so slot `i` here is
upstream's `coeft(i)` as read by `rkdcha.F90`. Kept verbatim — these are
what a code_aster deck contains and what the literature cites.

```rust
pub const ASTER_COEFFICIENT_NAMES: [&str; 25] = _;
```

## Module `viscoplastic`

Isotropic viscoplastic creep laws.

# What creep is, and why a reactor cares

Held under load below its yield stress, metal still deforms — slowly,
continuously, and faster when hot. That is creep, and in a fuel rod it is
not a nuisance but the main event: it is what lets cladding creep down onto
the pellet over months of operation, and what eventually ruptures a reactor
lower head held above temperature under its own weight.

# The shape of every law here

All are **isotropic** and **von Mises**: flow depends on the stress only
through its deviatoric part, measured by the equivalent stress
`σ_eq = √(3/2 · s:s)`, and it occurs in the direction of that deviator.
Each law differs only in the scalar rate `ṗ(σ_eq, p)`:

| Law | Rate |
|---|---|
| [`Norton`](ViscoplasticLaw::Norton) | `(σ_eq / K)^n` |
| [`Lemaitre`](ViscoplasticLaw::Lemaitre) | `(σ_eq / K)^n · p^(-n/m)` |
| [`LemaitreIrradiation`](ViscoplasticLaw::LemaitreIrradiation) | as Lemaitre, with `K` set by fast flux and temperature |

The strain rate is then `ε̇ = (3/2) ṗ s / σ_eq`, whose equivalent measure is
exactly `ṗ` — which is what makes `p` "the accumulated equivalent
viscoplastic strain" rather than an arbitrary internal variable.

**Norton is the `1/m → 0` limit of Lemaitre**, and upstream implements it
that way: `ggplem.F90` branches on `unsurm == 0` and falls back to the pure
power law. That relationship is preserved here and pinned by a test, because
it is the cheapest available check that both are encoded correctly.

# Primary versus secondary creep

Norton describes **secondary** (steady-state) creep: at fixed stress the
rate is constant. Lemaitre adds the `p^(-n/m)` factor, which makes the rate
*decay* as strain accumulates — **primary** creep, the initial transient
where a freshly loaded component creeps quickly and then settles. The
exponent is negative, so more accumulated strain means slower flow; getting
its sign wrong turns a decaying transient into a runaway.

# Integration

The stress at the end of a step depends on how much creep occurred, and the
creep depends on that stress. [`ViscoplasticLaw::integrate`] resolves the
circularity with a radial return: for isotropic von Mises flow the deviator
keeps its direction and only shrinks, so the whole tensorial problem reduces
to one scalar unknown, `Δp`, solved with the safeguarded Newton from
[`crate::rheology::aster::integration`].

```rust
pub mod viscoplastic { /* ... */ }
```

### Types

#### Struct `NortonParameters`

Parameters of the Norton power-law creep rule.

# Units

`k` in pascal, `n` dimensionless. Upstream stores `1/K` (`unsurk`) rather
than `K`; this port stores `K` because that is the quantity the literature
tabulates and the one with an interpretable unit, and inverts internally.

```rust
pub struct NortonParameters {
    pub k: f64,
    pub n: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `k` | `f64` | Reference stress `K` \[Pa\]. Larger `K` means more creep resistance. |
| `n` | `f64` | Stress exponent `n` \[-\]. Typically 3-8 for metals; higher means a<br>sharper dependence on stress. |

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
    fn clone(self: &Self) -> NortonParameters { /* ... */ }
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
    fn eq(self: &Self, other: &NortonParameters) -> bool { /* ... */ }
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
#### Struct `LemaitreParameters`

Parameters of the Lemaitre strain-hardening creep rule.

# Units

`k` in pascal, `n` and `m` dimensionless.

```rust
pub struct LemaitreParameters {
    pub k: f64,
    pub n: f64,
    pub m: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `k` | `f64` | Reference stress `K` \[Pa\]. |
| `n` | `f64` | Stress exponent `n` \[-\]. |
| `m` | `f64` | Strain-hardening exponent `m` \[-\].<br><br>Enters as `p^(-n/m)`, so **larger `m` means weaker hardening** and the<br>law tends to Norton as `m → ∞`. Must be non-zero; use<br>[`ViscoplasticLaw::Norton`] for the no-hardening case rather than a huge<br>`m`. |

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
    fn clone(self: &Self) -> LemaitreParameters { /* ... */ }
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
    fn eq(self: &Self, other: &LemaitreParameters) -> bool { /* ... */ }
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
#### Struct `LemaitreIrradiationParameters`

Parameters of the irradiation-creep variant of Lemaitre.

# What irradiation creep is

Under neutron irradiation a metal creeps far faster than temperature alone
would explain: displacement damage continuously creates point defects, and
their biased absorption at dislocations lets the material flow. For fuel
cladding this is the dominant creep mechanism in normal operation, and it is
what allows the cladding to creep down onto the pellet over months.

# Units

Upstream parameter names are kept verbatim in the field documentation so a
deck can be read across, but the Rust names are descriptive.

```rust
pub struct LemaitreIrradiationParameters {
    pub n: f64,
    pub m: f64,
    pub flux_coefficient: f64,
    pub reference_flux: f64,
    pub flux_exponent: f64,
    pub athermal_term: f64,
    pub activation_temperature: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `n` | `f64` | Stress exponent `n` \[-\]. Upstream `N`. |
| `m` | `f64` | Strain-hardening exponent `m` \[-\]. Upstream `UN_SUR_M` is its<br>reciprocal; this field is `m` itself, matching<br>[`LemaitreParameters::m`]. |
| `flux_coefficient` | `f64` | Flux sensitivity coefficient \[-\]. Upstream `UN_SUR_K`.<br><br>Scales the flux contribution to the creep compliance. Zero neutralises<br>the flux term entirely, which upstream implements by forcing the flux<br>ratio to one. |
| `reference_flux` | `f64` | Reference fast flux \[n/(m²·s)\]. Upstream `PHI_ZERO`. Must be strictly<br>positive — upstream raises a fatal error otherwise. |
| `flux_exponent` | `f64` | Flux exponent \[-\]. Upstream `BETA`. |
| `athermal_term` | `f64` | Athermal additive term \[-\]. Upstream `L`. Keeps creep finite at zero<br>flux. |
| `activation_temperature` | `f64` | Activation energy over the gas constant, `Q/R` \[K\]. Upstream `QSR_K`. |

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
    fn clone(self: &Self) -> LemaitreIrradiationParameters { /* ... */ }
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
    fn eq(self: &Self, other: &LemaitreIrradiationParameters) -> bool { /* ... */ }
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
#### Enum `ViscoplasticLaw`

An isotropic viscoplastic creep law.

Enum dispatch rather than trait objects, per the workspace rule — the set is
closed and known at compile time.

```rust
pub enum ViscoplasticLaw {
    Norton(NortonParameters),
    Lemaitre(LemaitreParameters),
    LemaitreIrradiation(LemaitreIrradiationParameters),
}
```

##### Variants

###### `Norton`

Norton power-law (secondary) creep.

ASTER behaviour name: `NORTON` (`num_lc = 32`, 7 state variables).
Upstream: `bibfor/algorith/norton.F90`, dispatched from
`bibfor/lc/lc0032.F90` — legacy symbols `norton`, `lc0032`.
Integration: `RUNGE_KUTTA` or `NEWTON_PERT` upstream; this port
integrates implicitly, see [`ViscoplasticLaw::integrate`].

`ṗ = (σ_eq / K)^n`. The rate depends on stress alone, so at fixed stress
it is constant — steady-state creep.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `NortonParameters` |  |

###### `Lemaitre`

Lemaitre strain-hardening (primary + secondary) creep.

ASTER behaviour name: `LEMAITRE`. Upstream:
`bibfor/algorith/ggplem.F90` — legacy symbol `ggplem`.

`ṗ = (σ_eq / K)^n · p^(-n/m)`. The accumulated strain slows subsequent
flow, reproducing the decaying primary-creep transient.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `LemaitreParameters` |  |

###### `LemaitreIrradiation`

Lemaitre irradiation creep — the cladding law for normal operation.

ASTER behaviour name: `LEMAITRE_IRRA`. Upstream:
`bibfor/comport/nmvpir.F90` assembles the parameters and calls the same
`ggplem` flow function as `LEMAITRE` — legacy symbols `nmvpir`,
`ggplem`.

Structurally this *is* Lemaitre; what differs is that the creep
compliance `1/K` is not a constant but is built from the fast flux and
the temperature:

`1/K = (A φ̇/φ₀ + L)^(β/n) · exp(-Q/(n·R·T))`

# Why the `1/n` appears twice, and why that is not a mistake

Both the flux exponent and the Arrhenius exponent are divided by `n`
here, which looks like an error until the rate is written out. Since
`ṗ = (σ_eq/K)^n`, the compliance is raised to the `n`-th power, and the
two divisions cancel:

`ṗ = σ_eq^n · (A φ̇/φ₀ + L)^β · exp(-Q/(R·T))`

So the *rate* carries a clean Arrhenius temperature dependence and a
clean power-law flux dependence — which is the physically meaningful
form, and the reason upstream parameterises it this way. Transcribing
the compliance without the `1/n` would give a rate with the exponents
raised to the `n`-th power, wrong by orders of magnitude and in a way no
dimensional check would catch.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `LemaitreIrradiationParameters` |  |

##### Implementations

###### Methods

- ```rust
  pub const fn aster_name(self: Self) -> &'static str { /* ... */ }
  ```
  The upstream ASTER behaviour name.

- ```rust
  pub fn at_irradiation_conditions(self: Self, fast_flux: f64, temperature: f64) -> Result<Self> { /* ... */ }
  ```
  Build the effective Lemaitre law for an irradiation-creep variant at a

- ```rust
  pub fn equivalent_strain_rate(self: Self, sigma_eq: f64, accumulated_strain: f64) -> f64 { /* ... */ }
  ```
  Equivalent viscoplastic strain rate `ṗ` \[1/s\].

- ```rust
  pub fn rate_derivative_wrt_stress(self: Self, sigma_eq: f64, accumulated_strain: f64) -> f64 { /* ... */ }
  ```
  Derivative of the rate with respect to equivalent stress, `∂ṗ/∂σ_eq`

- ```rust
  pub fn integrate(self: Self, trial_stress: SymmTensor, shear_modulus: f64, accumulated_strain: f64, dt: f64) -> Result<CreepIncrement> { /* ... */ }
  ```
  Integrate one timestep by radial return, returning the creep increment.

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
    fn clone(self: &Self) -> ViscoplasticLaw { /* ... */ }
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
    fn eq(self: &Self, other: &ViscoplasticLaw) -> bool { /* ... */ }
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
#### Struct `CreepIncrement`

The result of integrating one creep step.

```rust
pub struct CreepIncrement {
    pub equivalent_increment: f64,
    pub strain_increment: outram_foam_basic_lib::primitives::SymmTensor,
    pub stress: outram_foam_basic_lib::primitives::SymmTensor,
    pub equivalent_stress: f64,
    pub iterations: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `equivalent_increment` | `f64` | Equivalent creep increment `Δp` \[-\], non-negative. |
| `strain_increment` | `outram_foam_basic_lib::primitives::SymmTensor` | Creep strain increment tensor `Δε` \[-\]. Deviatoric — creep is<br>volume-preserving, which<br>[`creep_is_volume_preserving`](self) checks. |
| `stress` | `outram_foam_basic_lib::primitives::SymmTensor` | Relaxed stress at the end of the step \[Pa\]. |
| `equivalent_stress` | `f64` | Von Mises equivalent of [`stress`](Self::stress) \[Pa\]. |
| `iterations` | `usize` | Local-solver iterations used. |

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
    fn clone(self: &Self) -> CreepIncrement { /* ... */ }
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
    fn eq(self: &Self, other: &CreepIncrement) -> bool { /* ... */ }
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

#### Function `von_mises_of_deviator`

**Attributes:**

- `MustUse { reason: None }`

Von Mises equivalent stress `σ_eq = √(3/2 · s:s)` of a **deviator** `s`.

Upstream's `lcnrts`. Note it takes the deviator, not the full stress — pass
a stress tensor and the hydrostatic part inflates the answer.

```rust
pub fn von_mises_of_deviator(s: outram_foam_basic_lib::primitives::SymmTensor) -> f64 { /* ... */ }
```

#### Function `deviator`

**Attributes:**

- `MustUse { reason: None }`

Deviatoric part of a stress tensor, `s = σ - tr(σ)/3 · I`.

Upstream's `lcdevi`.

```rust
pub fn deviator(sigma: outram_foam_basic_lib::primitives::SymmTensor) -> outram_foam_basic_lib::primitives::SymmTensor { /* ... */ }
```

### Re-exports

#### Re-export `AsterBehaviour`

```rust
pub use catalogue::AsterBehaviour;
```

#### Re-export `ALL`

```rust
pub use catalogue::ALL;
```

#### Re-export `BackStress`

```rust
pub use chaboche::BackStress;
```

#### Re-export `ChabocheIncrement`

```rust
pub use chaboche::ChabocheIncrement;
```

#### Re-export `ChabocheLaw`

```rust
pub use chaboche::ChabocheLaw;
```

#### Re-export `ChabocheLocalState`

```rust
pub use chaboche::ChabocheLocalState;
```

#### Re-export `ChabocheParameters`

```rust
pub use chaboche::ChabocheParameters;
```

#### Re-export `ChabochePredictor`

```rust
pub use chaboche::ChabochePredictor;
```

#### Re-export `ChabocheState`

```rust
pub use chaboche::ChabocheState;
```

#### Re-export `ElasticModuli`

```rust
pub use chaboche::ElasticModuli;
```

#### Re-export `StrainMemory`

```rust
pub use chaboche::StrainMemory;
```

#### Re-export `ThermoElasticStep`

```rust
pub use chaboche::ThermoElasticStep;
```

#### Re-export `equivalent_stress`

```rust
pub use damage::equivalent_stress;
```

#### Re-export `max_principal_stress`

```rust
pub use damage::max_principal_stress;
```

#### Re-export `mean_stress`

```rust
pub use damage::mean_stress;
```

#### Re-export `DamageOutcome`

```rust
pub use damage::DamageOutcome;
```

#### Re-export `GtnIncrement`

```rust
pub use damage::GtnIncrement;
```

#### Re-export `GtnNucleation`

```rust
pub use damage::GtnNucleation;
```

#### Re-export `GtnOutcome`

```rust
pub use damage::GtnOutcome;
```

#### Re-export `GtnParameters`

```rust
pub use damage::GtnParameters;
```

#### Re-export `GtnState`

```rust
pub use damage::GtnState;
```

#### Re-export `GursonTvergaardNeedleman`

```rust
pub use damage::GursonTvergaardNeedleman;
```

#### Re-export `IsotropicElasticity`

```rust
pub use damage::IsotropicElasticity;
```

#### Re-export `LemaitreChabocheIncrement`

```rust
pub use damage::LemaitreChabocheIncrement;
```

#### Re-export `LemaitreChabocheLaw`

```rust
pub use damage::LemaitreChabocheLaw;
```

#### Re-export `LemaitreChabocheParameters`

```rust
pub use damage::LemaitreChabocheParameters;
```

#### Re-export `LemaitreChabocheState`

```rust
pub use damage::LemaitreChabocheState;
```

#### Re-export `NortonOverstress`

```rust
pub use damage::NortonOverstress;
```

#### Re-export `RousselierIncrement`

```rust
pub use damage::RousselierIncrement;
```

#### Re-export `RousselierLaw`

```rust
pub use damage::RousselierLaw;
```

#### Re-export `RousselierOutcome`

```rust
pub use damage::RousselierOutcome;
```

#### Re-export `RousselierParameters`

```rust
pub use damage::RousselierParameters;
```

#### Re-export `RousselierState`

```rust
pub use damage::RousselierState;
```

#### Re-export `RuptureCriterion`

```rust
pub use damage::RuptureCriterion;
```

#### Re-export `RuptureState`

```rust
pub use damage::RuptureState;
```

#### Re-export `ViscousSinhParameters`

```rust
pub use damage::ViscousSinhParameters;
```

#### Re-export `LEMAITRE_CHABOCHE_DAMAGE_MAX`

```rust
pub use damage::LEMAITRE_CHABOCHE_DAMAGE_MAX;
```

#### Re-export `equivalent_mode_i_factor`

```rust
pub use fracture::equivalent_mode_i_factor;
```

#### Re-export `hat_smooth_front`

```rust
pub use fracture::hat_smooth_front;
```

#### Re-export `irwin_energy_release_rate`

```rust
pub use fracture::irwin_energy_release_rate;
```

#### Re-export `irwin_mode_split`

```rust
pub use fracture::irwin_mode_split;
```

#### Re-export `legendre_front_mode`

```rust
pub use fracture::legendre_front_mode;
```

#### Re-export `legendre_front_mode_derivative`

```rust
pub use fracture::legendre_front_mode_derivative;
```

#### Re-export `max_hoop_stress_kink_angle`

```rust
pub use fracture::max_hoop_stress_kink_angle;
```

#### Re-export `near_tip_stress`

```rust
pub use fracture::near_tip_stress;
```

#### Re-export `scaled_hoop_stress`

```rust
pub use fracture::scaled_hoop_stress;
```

#### Re-export `westergaard_unit_field`

```rust
pub use fracture::westergaard_unit_field;
```

#### Re-export `CrackOpeningMode`

```rust
pub use fracture::CrackOpeningMode;
```

#### Re-export `CrackPlaneState`

```rust
pub use fracture::CrackPlaneState;
```

#### Re-export `CrackTipBasis`

```rust
pub use fracture::CrackTipBasis;
```

#### Re-export `LinearElasticConstants`

```rust
pub use fracture::LinearElasticConstants;
```

#### Re-export `ModeEnergyRelease`

```rust
pub use fracture::ModeEnergyRelease;
```

#### Re-export `NearTipField`

```rust
pub use fracture::NearTipField;
```

#### Re-export `PlanarCrackTipResult`

```rust
pub use fracture::PlanarCrackTipResult;
```

#### Re-export `StressIntensityFactors`

```rust
pub use fracture::StressIntensityFactors;
```

#### Re-export `MAX_LEGENDRE_FRONT_DEGREE`

```rust
pub use fracture::MAX_LEGENDRE_FRONT_DEGREE;
```

#### Re-export `IsotropicHardening`

```rust
pub use hardening::IsotropicHardening;
```

#### Re-export `ASTER_POWER_LINEARISATION_STRAIN`

```rust
pub use hardening::ASTER_POWER_LINEARISATION_STRAIN;
```

#### Re-export `SLOPE_SINGULARITY_OFFSET`

```rust
pub use hardening::SLOPE_SINGULARITY_OFFSET;
```

#### Re-export `brent`

```rust
pub use integration::brent;
```

#### Re-export `newton_perturbed`

```rust
pub use integration::newton_perturbed;
```

#### Re-export `newton_safeguarded`

```rust
pub use integration::newton_safeguarded;
```

#### Re-export `perturbed_default`

```rust
pub use integration::perturbed_default;
```

#### Re-export `secant`

```rust
pub use integration::secant;
```

#### Re-export `LocalSolution`

```rust
pub use integration::LocalSolution;
```

#### Re-export `ScalarAlgorithm`

```rust
pub use integration::ScalarAlgorithm;
```

#### Re-export `SolverControl`

```rust
pub use integration::SolverControl;
```

#### Re-export `NortonHoffLimitAnalysis`

```rust
pub use isotropic::NortonHoffLimitAnalysis;
```

#### Re-export `hencky_strain`

```rust
pub use kinematics::hencky_strain;
```

#### Re-export `AsterVoigt`

```rust
pub use kinematics::AsterVoigt;
```

#### Re-export `DeformationGradient`

```rust
pub use kinematics::DeformationGradient;
```

#### Re-export `LogarithmicStrain`

```rust
pub use log_strain::LogarithmicStrain;
```

#### Re-export `HillAnisotropy`

```rust
pub use metallurgy::HillAnisotropy;
```

#### Re-export `Irrad3m`

```rust
pub use metallurgy::Irrad3m;
```

#### Re-export `Irrad3mHardening`

```rust
pub use metallurgy::Irrad3mHardening;
```

#### Re-export `Irrad3mIncrement`

```rust
pub use metallurgy::Irrad3mIncrement;
```

#### Re-export `Irrad3mParameters`

```rust
pub use metallurgy::Irrad3mParameters;
```

#### Re-export `Irrad3mState`

```rust
pub use metallurgy::Irrad3mState;
```

#### Re-export `IrradiationGrowthDirection`

```rust
pub use metallurgy::IrradiationGrowthDirection;
```

#### Re-export `LogarithmicIrradiationLaw`

```rust
pub use metallurgy::LogarithmicIrradiationLaw;
```

#### Re-export `LogarithmicIrradiationParameters`

```rust
pub use metallurgy::LogarithmicIrradiationParameters;
```

#### Re-export `MetaLemaAni`

```rust
pub use metallurgy::MetaLemaAni;
```

#### Re-export `MetaLemaAniIncrement`

```rust
pub use metallurgy::MetaLemaAniIncrement;
```

#### Re-export `MetaLemaAniPhase`

```rust
pub use metallurgy::MetaLemaAniPhase;
```

#### Re-export `IRRAD3M_PROOF_STRAIN`

```rust
pub use metallurgy::IRRAD3M_PROOF_STRAIN;
```

#### Re-export `ViscoplasticChabocheParameters`

```rust
pub use viscochab::ViscoplasticChabocheParameters;
```

#### Re-export `ViscoplasticChabocheRates`

```rust
pub use viscochab::ViscoplasticChabocheRates;
```

#### Re-export `ViscoplasticChabocheState`

```rust
pub use viscochab::ViscoplasticChabocheState;
```

#### Re-export `ViscoplasticChabocheSystem`

```rust
pub use viscochab::ViscoplasticChabocheSystem;
```

#### Re-export `ViscoplasticChabocheWithMemory`

```rust
pub use viscochab::ViscoplasticChabocheWithMemory;
```

#### Re-export `RKDCHA_ALPHA2_USES_D1`

```rust
pub use viscochab::RKDCHA_ALPHA2_USES_D1;
```

#### Re-export `deviator`

```rust
pub use viscoplastic::deviator;
```

#### Re-export `von_mises_of_deviator`

```rust
pub use viscoplastic::von_mises_of_deviator;
```

#### Re-export `CreepIncrement`

```rust
pub use viscoplastic::CreepIncrement;
```

#### Re-export `LemaitreParameters`

```rust
pub use viscoplastic::LemaitreParameters;
```

#### Re-export `NortonParameters`

```rust
pub use viscoplastic::NortonParameters;
```

#### Re-export `ViscoplasticLaw`

```rust
pub use viscoplastic::ViscoplasticLaw;
```

### Re-exports

#### Re-export `Rheology`

```rust
pub use by_material::Rheology;
```

#### Re-export `RheologyByMaterial`

```rust
pub use by_material::RheologyByMaterial;
```

#### Re-export `CreepIncrement`

```rust
pub use creep::CreepIncrement;
```

#### Re-export `CreepModel`

```rust
pub use creep::CreepModel;
```

#### Re-export `CreepTimeStepControl`

```rust
pub use creep::CreepTimeStepControl;
```

#### Re-export `ZircaloyCladType`

```rust
pub use creep::ZircaloyCladType;
```

#### Re-export `ConstitutiveLaw`

```rust
pub use law::ConstitutiveLaw;
```

#### Re-export `equivalent_strain`

```rust
pub use state::equivalent_strain;
```

#### Re-export `von_mises`

```rust
pub use state::von_mises;
```

#### Re-export `IrradiationState`

```rust
pub use state::IrradiationState;
```

#### Re-export `RheologyInputs`

```rust
pub use state::RheologyInputs;
```

#### Re-export `RheologyState`

```rust
pub use state::RheologyState;
```

#### Re-export `StressCorrection`

```rust
pub use state::StressCorrection;
```

#### Re-export `HardeningCurve`

```rust
pub use yield_stress::HardeningCurve;
```

#### Re-export `YieldStressModel`

```rust
pub use yield_stress::YieldStressModel;
```

## Module `prelude`

Convenience re-export of the types most fuel-performance code needs.

```rust
use outram_park_fork_offbeat::prelude::*;
```

```rust
pub mod prelude { /* ... */ }
```

### Re-exports

#### Re-export `OffbeatError`

```rust
pub use crate::error::OffbeatError;
```

#### Re-export `Result`

```rust
pub use crate::error::Result;
```

#### Re-export `MaterialState`

```rust
pub use crate::materials::MaterialState;
```

## Re-exports

### Re-export `OffbeatError`

```rust
pub use error::OffbeatError;
```

