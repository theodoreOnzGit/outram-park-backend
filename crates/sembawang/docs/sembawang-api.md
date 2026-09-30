# Crate Documentation

**Version:** 0.0.1

**Format Version:** 60

# Module `sembawang`

# SEMBAWANG

**S**evere-accident **E**volution and **M**elt **B**ehaviour **A**nalysis
**W**orkbench for **A**dvanced **N**uclear **G**eometries.

**Severe accident and source term, and orchestrator of the offsite chain**
(maintainer decision 2026-09-21, GitHub #235; `docs/ecosystem-naming.md`
decision 7).

- **Its own physics:** the fission-product source term (implemented, on
  `boon-lay`'s TRISO-ATOPS fork) and severe-accident progression: melt
  behaviour, relocation, vessel failure, molten-core–concrete interaction,
  hydrogen and aerosol release (**not implemented**).
- **What it orchestrates:** [`changi`] for dispersion and deposition (wired
  in, see [`chain`]), `redhill` for ground transport (a placeholder, not
  wired), and `raffles` for uncertainty propagation (GitHub #238, not yet
  a dependency).

The chain ends at activity released, air concentration and deposition.
**The library computes no dose quantity**, and none of this is described as
PSA. ~~(no dose anywhere in the crate)~~ **UPDATED 2026-09-30:** `buangkok`
is now a dependency (maintainer direction), and **one example**,
`examples/htr10_air_ingress_kora_bound.rs`, computes a research-grade dose
at distance through `buangkok`'s Gaussian plume and FGR coefficients. The
dose arithmetic is `buangkok`'s, not this crate's.

# STATUS: partially implemented, and the unimplemented part is the larger one

~~**PLACEHOLDER. Nothing is implemented.** Created 2026-09-18 to hold a
reserved name and a scope statement, not code. Do not cite it as the
location of a source-term calculation until something has actually been
written here.~~ **SUPERSEDED 2026-09-21** — the text is kept rather than
deleted because it was correct when written and because what replaced it is
narrower than the crate's name suggests. Read the next two paragraphs
before citing this crate for anything.

**What exists: the fission-product release path for TRISO fuel**, built on
`boon-lay`'s TRISO-ATOPS fork, producing a
[`changi::activity::source::SourceTerm`]. That is a *mechanistic release
from intact and defective particles under a prescribed temperature
transient* — diffusion through the kernel and coating layers, and
breakthrough of the SiC.

**What does not exist: severe-accident progression**, which is the rest of
the crate's name and the larger half. There is no melt behaviour, no
relocation, no vessel failure, no molten-core-concrete interaction, no
hydrogen, and no aerosol physics. The temperature transient this crate
consumes is **prescribed by the caller**; nothing here computes it.

So: cite this crate for a TRISO release calculation under a temperature
history you supplied, and (since 2026-09-21) for carrying that release
through `changi` to air concentration and deposition. The line below is the
original, kept because it is still true as far as it goes:

> cite this crate for a TRISO release calculation under a temperature
> history you supplied. Do **not** cite it for accident progression.

The name was already reserved in `docs/ecosystem-naming.md` and
`GOVERNANCE.md`; this crate makes the reservation visible in the workspace
rather than only in prose.

# Where it sits: the offsite chain

```text
             ┌──────────── SEMBAWANG orchestrates ────────────┐
  boon-lay ──► source term ──► CHANGI ──► REDHILL             │
  (TRISO       (what gets      (air, Bq·s/m³;  (ground         │
   release)     released?)      ground, Bq/m²)  transport)     │
             └── RAFFLES: uncertainty propagation (#238) ─────┘
```

~~[`changi`] exists and has a FLEXPART v10.4 port under way; [`redhill`] is,
like this crate, a placeholder. **CHANGI currently has no upstream**: its
own README records that until SEMBAWANG exists, a release-rate time series
has to be supplied by hand.~~ **CORRECTED 2026-09-21**: this crate is no
longer a placeholder, and it feeds [`changi`] directly through
[`chain::pad_for_dispersion`] (see `examples/npmhtgr_chain.rs`). `redhill` is
still a placeholder.

# Honest scope of the gap

This is the **largest** of the reserved names by some margin, and that
should be stated plainly rather than discovered later. [`redhill`] can
build on `outram-park-fork-pflotran`, which already exists in this
workspace, so it is substantially an integration layer.

~~SEMBAWANG has **no engine at all**.~~ **CORRECTED 2026-09-21** — it has
one for *release*: `boon-lay`'s TRISO-ATOPS fork, which is ported,
`uom`-typed and code-to-code verified. That is what made the source-term
half tractable.

It still has **no engine for progression**. Severe-accident progression
would need a MELCOR-class port, which is scoped in `docs/melcor-scoping.md`
and has not been begun. Treat that scoping document as a statement of
intent, not of progress.

# What this crate does NOT compute, listed so it is not assumed

- **The temperature transient.** Prescribed by the caller. An
  `outram-park-digital-twin-engine` trace can be dropped in, but nothing
  here solves for one.
- **The core inventory.** Prescribed. In particular, do **not** try to build
  one from `fission-yields-data`: it exposes *independent* fission yields,
  and Cs-137's independent yield is roughly two orders of magnitude below
  its cumulative yield because Cs-137 arrives down the A = 137 isobaric
  chain. Naive use gives a silently ~100x low inventory for most of the
  species that matter. Cumulative yields need a decay-chain walk.
- **Parent to daughter chaining during the accident**, and so no daughter
  ingrowth.
- **Containment transport, pool scrubbing or iodine chemistry.** What leaves
  the fuel is treated as what leaves the building.
- **Any dose quantity in the library**, here or in [`changi`]. The library
  chain stops at activity in air and on the ground. Dose comes from
  `buangkok`, and is used only by the `htr10_air_ingress_kora_bound` example.

# Intended use, and what it will never be for

Research, education, capability building and V&V only, like the rest of
this workspace (`RESPONSIBLE_USE.md`). Severe-accident and source-term
analysis is precisely the area where an unvalidated tool is most dangerous,
so when this crate does acquire code it must **not** be presented as
supporting emergency response, emergency planning, licensing, or
safety-critical decisions — the same boundary `changi` already carries.

[`redhill`]: https://docs.rs/redhill

## Modules

## Module `accident`

Accident-phase release, orchestrated over `boon-lay`'s TRISO-ATOPS fork.

This module **computes no release physics**. Every diffusion coefficient,
release fraction and breakthrough model lives in
`boon_lay::triso_atops_fork` and is called from here — the workspace rule is
reuse before porting and porting before writing, and the release layer is
already ported, `uom`-typed and code-to-code verified against upstream.

What this module owns is the parts that sit *around* that: pairing the
venting selection with the right temperatures ([`venting`]), and assembling
the result into a [`changi::activity::source::SourceTerm`].

The temperature transient itself is **prescribed by the caller** — see the
crate docs for the full list of what is not computed here.

```rust
pub mod accident { /* ... */ }
```

### Modules

## Module `release`

The driver: a prescribed transient plus an inventory, out to a source term.

# What this does and does not own

Every piece of release physics here is a call into
`boon_lay::triso_atops_fork` — diffusion integrals, release fractions,
breakthrough, the atoms-to-curies conversion. **None of it is reimplemented,
and none of it should be**: that layer is ported, `uom`-typed and verified
code-to-code against upstream TRISO-ATOPS, and a second copy here would
drift from it silently.

What this module owns is the orchestration: the node loops, the venting
pairing (through [`super::venting::VentingWindow`], never a prefix), the
cumulative-to-incremental conversion, and the assembly into a
[`SourceTerm`].

# The 6-to-7 field bridge

`normal_operation_node` returns a `NodalActivities` with **six** channels.
`release_activity` wants a `NormalOperationNode` with **seven**. The extra
one is `kernel_inventory_atoms`, which is not an output of the normal-
operation solve at all — it is the node's own inventory expressed as an atom
count, `atom_count_from_activity(inventory, lambda)`. [`bridge_node`] is that
conversion, in one place, so the two shapes cannot be lined up wrongly at a
call site.

# Cumulative curies to per-window activity

`accident_release_curies` returns a **cumulative** release at each venting
sample. [`SourceTerm`] wants the activity released **during** each window.
The conversion is a first difference, with the first window carrying
everything up to the second venting sample so nothing is dropped:

```text
release[0] = cumulative[1]
release[i] = cumulative[i+1] - cumulative[i]      for i >= 1
```

so the windows sum to `cumulative[last]` exactly.

**Windows are contiguous by construction even when the venting mask is
gappy**, because window `i` spans venting sample `i` to venting sample
`i+1` — a cooling period simply becomes one long window rather than a hole.
That matters because `changi`'s `SourceTerm` rejects gaps.

A negative increment is possible, because upstream's `release_activity`
deliberately does not clamp. It is **not clamped here either** — it is
reported through [`crate::error::Caveats::negative_atom_count_seen`] and
floored to zero only at the [`SourceTerm`] boundary, which rejects negative
activities. Silently clamping and silently passing it on are both worse than
saying so.

```rust
pub mod release { /* ... */ }
```

### Types

#### Struct `PlantParameters`

The geometry, failure fractions and plant constants a release needs.

Every field is prescribed by the caller. There are no defaults, deliberately
— a default geometry would be a specific reactor's, wearing no label.

```rust
pub struct PlantParameters {
    pub fractions: boon_lay::triso_atops_fork::accident::AccidentFractions,
    pub graphite_thickness: uom::si::f64::Length,
    pub kernel_radius: uom::si::f64::Length,
    pub sic_thickness: uom::si::f64::Length,
    pub coolant_pressure: uom::si::f64::Pressure,
    pub x_liftoff: f64,
    pub clean_up_fitted: bool,
    pub pools: PrimaryCircuitPools,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `fractions` | `boon_lay::triso_atops_fork::accident::AccidentFractions` | Failure fractions, normal-operation and accident-phase. |
| `graphite_thickness` | `uom::si::f64::Length` | Matrix graphite diffusion thickness. |
| `kernel_radius` | `uom::si::f64::Length` | Fuel kernel radius. |
| `sic_thickness` | `uom::si::f64::Length` | SiC layer thickness. |
| `coolant_pressure` | `uom::si::f64::Pressure` | Coolant pressure, for the venting calculation. |
| `x_liftoff` | `f64` | Fraction of plated-out activity lifted off during the accident. |
| `clean_up_fitted` | `bool` | Whether a helium purification system is fitted. |
| `pools` | `PrimaryCircuitPools` | The primary-circuit state the accident starts from (GitHub #448). The<br>constructors set [`PrimaryCircuitPools::FromNormalOperation`]; the empty<br>state is an explicit ablation. |

##### Implementations

###### Methods

- ```rust
  pub fn np_mhtgr_reference(incremental_accident: f64, incremental_sic_accident: f64, x_liftoff: f64) -> Self { /* ... */ }
  ```
  The NP-MHTGR reference geometry and normal-operation failure fractions,

- ```rust
  pub fn without_normal_operation_pools(self: Self) -> Self { /* ... */ }
  ```
  The same plant, with the primary-circuit pools **emptied**: an explicit

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> PlantParameters { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PlantParameters) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `PrimaryCircuitPools`

The primary-circuit and fuel-matrix pools an accident starts from.

Upstream (`trisoatops.py::main`, commit `de374c8`) runs `normal_operation`
and feeds its per-node pools into `accident_case`. That does three things
the empty state omits (#448):
1. `release_activity` subtracts the graphite, circulating, plate-out and HPS
   inventory from what the kernel can still release;
2. the graphite pool is released through `RF_graph`;
3. the circuit term `C + x_liftoff · P` is added.

**Default ON** (CLAUDE.md "correct physics is the default"): every
constructor in this crate sets [`Self::FromNormalOperation`].

```rust
pub enum PrimaryCircuitPools {
    FromNormalOperation(NormalOperation),
    EmptyAblation,
}
```

##### Variants

###### `FromNormalOperation`

Run TRISO-ATOPS normal operation per node first, as upstream does.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `NormalOperation` |  |

###### `EmptyAblation`

**Ablation only.** Every pool empty ([`zero_pools`]). Measured in
upstream on a heat-up case (#448), this **over**-predicts the metals
(Ag-110m ×2.0, Cs-137 ×1.6) and slightly **under**-predicts
un-scrubbed noble gases (Kr-85 ×0.87). The direction is
nuclide-dependent.

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> PrimaryCircuitPools { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PrimaryCircuitPools) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `NormalOperation`

The normal-operation inputs upstream's `normal_operation` needs, beyond
the inventory, fractions and geometry already in [`PlantParameters`].

```rust
pub struct NormalOperation {
    pub k_plate: uom::si::f64::Frequency,
    pub k_clean: uom::si::f64::Frequency,
    pub grain_size: uom::si::f64::Length,
    pub run_time: uom::si::f64::Time,
    pub irradiation_time: uom::si::f64::Time,
    pub temperatures: NodeTemperatures,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `k_plate` | `uom::si::f64::Frequency` | Plate-out rate constant `k_plate`. |
| `k_clean` | `uom::si::f64::Frequency` | Helium-purification clean-up rate constant `k_clean`. Applied only when<br>[`PlantParameters::clean_up_fitted`]. |
| `grain_size` | `uom::si::f64::Length` | Kernel grain size `a_grain`. |
| `run_time` | `uom::si::f64::Time` | Reactor run time, for the coolant-pool balances. |
| `irradiation_time` | `uom::si::f64::Time` | Fuel irradiation time, for release-to-birth and the short-lived flag. |
| `temperatures` | `NodeTemperatures` | Normal-operation fuel and graphite temperatures. |

##### Implementations

###### Methods

- ```rust
  pub fn np_mhtgr_reference() -> Self { /* ... */ }
  ```
  The NP-MHTGR reference normal operation, Stoyer et al. 2026 Case A,

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> NormalOperation { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &NormalOperation) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `NodeTemperatures`

Normal-operation temperatures over the core nodes.

```rust
pub enum NodeTemperatures {
    Uniform {
        core: uom::si::f64::ThermodynamicTemperature,
        graphite: uom::si::f64::ThermodynamicTemperature,
    },
    PerNode {
        core: Vec<Vec<uom::si::f64::ThermodynamicTemperature>>,
        graphite: Vec<Vec<uom::si::f64::ThermodynamicTemperature>>,
    },
}
```

##### Variants

###### `Uniform`

One fuel and one graphite temperature for every node.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `core` | `uom::si::f64::ThermodynamicTemperature` | Fuel (kernel) temperature. |
| `graphite` | `uom::si::f64::ThermodynamicTemperature` | Matrix graphite temperature. |

###### `PerNode`

Per node, `[ring][axial]`, matching the transient's node layout.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `core` | `Vec<Vec<uom::si::f64::ThermodynamicTemperature>>` | Fuel (kernel) temperatures. |
| `graphite` | `Vec<Vec<uom::si::f64::ThermodynamicTemperature>>` | Matrix graphite temperatures. |

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> NodeTemperatures { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &NodeTemperatures) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `AccidentRelease`

A completed release calculation.

```rust
pub struct AccidentRelease {
    pub source_term: changi::activity::source::SourceTerm,
    pub caveats: crate::error::Caveats,
    pub venting: super::venting::VentingWindow,
    pub screened_out: Vec<String>,
    pub cumulative_final: Vec<(String, f64)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `source_term` | `changi::activity::source::SourceTerm` | The source term, ready for `changi`. |
| `caveats` | `crate::error::Caveats` | Known upstream behaviours that affected this result. **Report these<br>alongside any number taken from `source_term`** — see [`Caveats`]. |
| `venting` | `super::venting::VentingWindow` | Which samples vented, and how much each released. |
| `screened_out` | `Vec<String>` | Nuclides dropped by the half-life screen, with the reason. |
| `cumulative_final` | `Vec<(String, f64)>` | Per released nuclide, the **unfloored cumulative release at the last<br>venting sample** \[Bq\]: upstream `accident_case`'s last total. The<br>source term's window sum equals this unless a window was floored<br>([`Caveats::negative_atom_count_seen`]). This is the quantity to compare<br>with upstream. |

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> AccidentRelease { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &AccidentRelease) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `Venting`

How released activity leaves the core, i.e. the vented fraction `frac(t)`
that multiplies the cumulative fuel release (upstream
`accident_totals = frac * (kernel + graphite) + …`).

**TRISO-ATOPS is a depressurisation model.** Its user manual: *"releases
are due to a breach in the reactor resulting in a venting of the core"*.
Its only transport is [`Venting::Upstream`]: thermal expansion of the
coolant while the core heats. **It has no air- or water-ingress transport**,
in which gas flows through the core and carries the release out whatever
the temperature does. The other two variants exist for that (GitHub #446),
and they are **this workspace's additions, not upstream behaviour**.

`frac(t)` is the fraction of the fuel's cumulative release up to `t` that
has left the core by `t`. It is dimensionless, in `[0, 1]`.

```rust
pub enum Venting {
    Upstream,
    FullFlowThrough,
    Prescribed(Vec<f64>),
}
```

##### Variants

###### `Upstream`

Upstream TRISO-ATOPS (`trisoatops.py::accident_case`, commit `de374c8`),
both branches:
- **uniform and constant temperature** (every node at every time equal
  to the first, by exact comparison as upstream's
  `np.all(accident_temp == accident_temp[0, 0, 0])`): **`frac = 1` at
  every sample**, and no `coolant_release` call;
- **otherwise:** `coolant_release`. The ideal-gas expansion fraction at
  the heating samples (`dT/dt ≥ 0`), with upstream's forced
  `frac[0] = 1`.

~~Before 2026-09-30 the port always took the second branch~~, so an
isothermal hold vented nothing after `t = 0` and released **0 Bq**
(#446). The first branch was missing from the port and is restored
here.

###### `FullFlowThrough`

Everything released from the fuel leaves the core at once: `frac = 1`
at every sample. The conservative choice for an ingress with no
primary-circuit retention (the #435 bound's assumption). Not upstream.

###### `Prescribed`

A caller-supplied `frac(t)`, **one entry per transient sample**, each
finite and in `[0, 1]`: e.g. the cumulative fraction of the core's gas
exchanged by an ingress flow. The caller owns the number and its
source. Not upstream.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<f64>` |  |

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Venting { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Venting) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `zero_pools`

**Attributes:**

- `MustUse { reason: None }`

A normal-operation state with every pool empty: the
[`PrimaryCircuitPools::EmptyAblation`] state.

~~Starting from zero therefore **under-predicts** the early release~~
**CORRECTED 2026-09-30 (#448):** the direction is **nuclide-dependent**.
Measured in upstream TRISO-ATOPS (3-year irradiation, 900 °C normal
operation, heat to 1600 °C then cool), the ratio empty ÷ real pools is:
Ag-110m ×2.00 and Cs-137 ×1.59 (**over**-predicted: the kernel term is not
reduced by what already left it); Kr-85 ×0.87 without HPS (**under**: the
circuit term is lost); Sr-90 0.99; I-131 1.00. Since #448 the default is
[`PrimaryCircuitPools::FromNormalOperation`], and this is an ablation.

```rust
pub fn zero_pools() -> boon_lay::triso_atops_fork::normal_operation::NodalActivities { /* ... */ }
```

#### Function `bridge_node`

**Attributes:**

- `MustUse { reason: None }`

Bridge `normal_operation_node`'s six channels to `release_activity`'s seven.

The seventh, `kernel_inventory_atoms`, is the node's own inventory as an
atom count — not an output of the normal-operation solve. See the module
docs.

```rust
pub fn bridge_node(activities: &boon_lay::triso_atops_fork::normal_operation::NodalActivities, inventory_curies: f64, decay_constant: uom::si::f64::Frequency) -> boon_lay::triso_atops_fork::accident::NormalOperationNode { /* ... */ }
```

#### Function `deposition_group_of`

**Attributes:**

- `MustUse { reason: None }`

Which `changi` deposition group a TRISO-ATOPS nuclide belongs to.

**Goes through the atomic number, not through `boon-lay`'s
[`ElementGroup`].** The two groupings genuinely differ: Se and Te travel
through TRISO layers like halogens and are grouped with them for *release*,
but deposit as condensed aerosols. Translating one enum into the other would
be wrong for exactly those two, so the classification is re-derived from `Z`.

```rust
pub fn deposition_group_of(nuclide: &boon_lay::triso_atops_fork::TrisoAtopsNuclide) -> changi::activity::deposition::DepositionGroup { /* ... */ }
```

#### Function `accident_release`

Run the accident release and assemble a source term.

# Arguments
- `inventory` — the prescribed core inventory. Its `n_radial`/`n_axial` must
  match `transient`'s.
- `transient` — the prescribed temperature history.
- `plant` — geometry, failure fractions and plant constants.

# The half-life screen sets the nuclide list

`select_nuclides_accident` drops any nuclide whose half-life is below 4 % of
the accident duration, on the grounds that it decays away before it matters.
**That threshold is relative, so lengthening the transient screens out
more.** Whichever nuclides it drops are returned in
[`AccidentRelease::screened_out`] rather than vanishing.

# Errors
[`Error::UnknownNuclide`] if a name is not in the supported table;
[`Error::TransientTooShort`] if the venting calculation has too little to
work with; [`Error::VentingTimeNotOnAxis`] if the venting selection cannot
be reconciled with the time axis.

# Panics
Panics if `inventory` and `transient` disagree on the node counts.

```rust
pub fn accident_release(inventory: &crate::inventory::CoreInventory, transient: &crate::scenario::TemperatureTransient, plant: &PlantParameters) -> crate::error::Result<AccidentRelease> { /* ... */ }
```

#### Function `accident_release_with_venting`

[`accident_release`] with the core-venting (transport) mode chosen
explicitly; see [`Venting`]. [`accident_release`] is this with
[`Venting::Upstream`].

# Errors
As [`accident_release`], plus [`Error::VentingLengthMismatch`] and
[`Error::VentingFractionOutOfRange`] for a bad [`Venting::Prescribed`].

# Panics
As [`accident_release`].

```rust
pub fn accident_release_with_venting(inventory: &crate::inventory::CoreInventory, transient: &crate::scenario::TemperatureTransient, plant: &PlantParameters, venting_mode: &Venting) -> crate::error::Result<AccidentRelease> { /* ... */ }
```

#### Function `temperature_is_inside_the_fitted_range`

**Attributes:**

- `MustUse { reason: None }`

A transient's peak against the diffusion correlation's fitted range, for
reporting.

```rust
pub fn temperature_is_inside_the_fitted_range(t: uom::si::f64::ThermodynamicTemperature) -> bool { /* ... */ }
```

#### Function `source_term_duration`

**Attributes:**

- `MustUse { reason: None }`

The total duration a source term spans, for sizing a dispersion run.

```rust
pub fn source_term_duration(term: &changi::activity::source::SourceTerm) -> uom::si::f64::Time { /* ... */ }
```

### Constants and Statics

#### Constant `DIFFUSION_FIT_MIN_CELSIUS`

Lower edge of the Arrhenius diffusion correlation's fitted range, degrees
Celsius. Outside it `boon-lay` clamps rather than extrapolating; crossing it
sets [`Caveats::diffusion_coefficient_clamped`].

```rust
pub const DIFFUSION_FIT_MIN_CELSIUS: f64 = 700.0;
```

#### Constant `DIFFUSION_FIT_MAX_CELSIUS`

Upper edge of the fitted range, degrees Celsius. See
[`DIFFUSION_FIT_MIN_CELSIUS`].

```rust
pub const DIFFUSION_FIT_MAX_CELSIUS: f64 = 2400.0;
```

## Module `venting`

The venting selection, held as indices so it cannot be mispaired.

# The upstream defect this type exists to make unwriteable

`coolant_release` returns the released fraction at each sample where the
hot-node temperature is **rising** (`mean_dtdt >= 0`), together with the
times of those samples. That is a **gather**: an arbitrary subset of the
full time axis, in order.

Upstream then pairs it with a **prefix** of the temperature array:

```python
frac, times_short = calc.coolant_release(times, accident_temp)
rmv = np.size(times) - np.size(times_short)
times = times_short                         # a SELECTION
accident_temp = accident_temp[:, :-rmv, :]  # a PREFIX
```

A selection of `k` elements and the first `k` elements are the same thing
**only when the venting mask is a contiguous run starting at index 0.** For
a monotonically heating transient it always is, which is why the defect
survives: the reference case never exercises it.

On a transient that heats, cools and reheats, the mask is gappy and the two
diverge — every venting sample after the gap gets paired with the
temperature of an earlier, cooler one. Measured on a heat-cool-reheat case,
one node's Xe-133 release differs by tens of per cent between the two
pairings.

# The fix is structural, not care

Being careful is not a fix, because the next caller has to be careful too.
So: [`VentingWindow`] owns the **indices**, [`VentingWindow::times`] and
[`VentingWindow::gather`] both go through them, and **no method on this type
returns a prefix.** There is no API here that can produce upstream's
pairing; a caller that wants it has to slice an array by hand, which is
visible in a diff.

# Why the indices are recovered by walking, not re-derived

The obvious alternative is to re-evaluate `mean_dtdt >= 0` here and take the
indices where it holds. That would be a **second copy of the predicate**,
and two copies drift — a change to the venting condition in `boon-lay` would
silently stop matching this one, and the failure would be a subtle
mispairing rather than a compile error.

Instead [`VentingWindow::from_coolant_release`] walks the returned venting
times against the full axis in lockstep and asserts every one is consumed.
It carries no opinion about *why* a sample vented, so it cannot disagree
about it.

```rust
pub mod venting { /* ... */ }
```

### Types

#### Struct `VentingWindow`

Which samples of a transient vented, and how much each released.

Constructed from `boon-lay`'s `coolant_release` output. See the module docs
for why it stores indices and why it has no prefix accessor.

```rust
pub struct VentingWindow {
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
  pub fn from_coolant_release(full_times: &[Time], vent_times: &[Time], fractions: Vec<f64>) -> Result<Self> { /* ... */ }
  ```
  Recover the venting indices from `coolant_release`'s output.

- ```rust
  pub fn all_samples(fractions: Vec<f64>) -> Self { /* ... */ }
  ```
  A window over **every** sample of an `n`-sample axis, with the given

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  How many samples vented.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether nothing vented.

- ```rust
  pub const fn full_len(self: &Self) -> usize { /* ... */ }
  ```
  The length of the full time axis this was gathered from.

- ```rust
  pub fn indices(self: &Self) -> &[usize] { /* ... */ }
  ```
  The indices into the full axis, ascending and without repeats.

- ```rust
  pub fn fractions(self: &Self) -> &[f64] { /* ... */ }
  ```
  The released fraction at each venting sample.

- ```rust
  pub fn is_contiguous(self: &Self) -> bool { /* ... */ }
  ```
  Whether the venting samples form a contiguous run.

- ```rust
  pub fn times(self: &Self, full_times: &[Time]) -> Vec<Time> { /* ... */ }
  ```
  The venting samples' times, gathered from the full axis.

- ```rust
  pub fn gather<T: Copy>(self: &Self, full: &[T]) -> Vec<T> { /* ... */ }
  ```
  Any per-sample quantity, gathered at the venting indices.

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> VentingWindow { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &VentingWindow) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Re-exports

#### Re-export `accident_release`

```rust
pub use release::accident_release;
```

#### Re-export `accident_release_with_venting`

```rust
pub use release::accident_release_with_venting;
```

#### Re-export `AccidentRelease`

```rust
pub use release::AccidentRelease;
```

#### Re-export `NodeTemperatures`

```rust
pub use release::NodeTemperatures;
```

#### Re-export `NormalOperation`

```rust
pub use release::NormalOperation;
```

#### Re-export `PlantParameters`

```rust
pub use release::PlantParameters;
```

#### Re-export `PrimaryCircuitPools`

```rust
pub use release::PrimaryCircuitPools;
```

#### Re-export `Venting`

```rust
pub use release::Venting;
```

#### Re-export `VentingWindow`

```rust
pub use venting::VentingWindow;
```

## Module `chain`

Hand a released source term to `changi` for dispersion and deposition.

[`crate::accident::release::accident_release`] produces a
[`SourceTerm`] whose windows span the accident transient. `changi`'s
[`changi::activity::chi_over_q::dilution_factors`] needs something slightly
different: segment boundaries that partition the **whole dispersion run**,
starting at `t = 0` and ending when the run ends. The run must also carry on
after the last release, so the last puffs have time to reach the far
receptors.

[`pad_for_dispersion`] closes that gap and does nothing else. It prepends a
zero-release window if the release starts after `t = 0`, and appends one
zero-release window covering the post-release tail. **No activity is added,
removed or moved**, so the per-nuclide totals are unchanged. That is
asserted in `tests/chain_handoff.rs`.

This module has **no upstream**. It is plumbing between two crates, checked
by consistency tests, not verification.

```rust
pub mod chain { /* ... */ }
```

### Functions

#### Function `pad_for_dispersion`

**Attributes:**

- `MustUse { reason: None }`

Pad `term` so its windows partition a dispersion run from `t = 0` to
`term.end() + tail`.

# Arguments
- `term` — the released source term, e.g. `AccidentRelease::source_term`.
- `tail` — how long the dispersion run continues after the last release,
  in seconds of simulated time. Choose at least the puff lifetime
  (`RunConfig::puff_duration`), or the last puffs are cut off before
  reaching the far receptors.

# Returns
A new [`SourceTerm`] with the same nuclides, labels, decay constants and
deposition groups. Its windows are, in order: an optional leading
zero-release window, the original windows, and one trailing zero-release
window of length `tail`. Use its [`SourceTerm::segment_boundaries`] as the
segment list for `dilution_factors`, and its [`SourceTerm::end`] as the
run duration.

# Panics
Panics if `tail` is not positive, or if the first window starts before
`t = 0`.

```rust
pub fn pad_for_dispersion(term: &changi::activity::source::SourceTerm, tail: uom::si::f64::Time) -> changi::activity::source::SourceTerm { /* ... */ }
```

## Module `error`

Errors, and the caveat channel.

# Why known upstream defects are returned as DATA, not logged

`boon-lay`'s TRISO-ATOPS fork faithfully reproduces several upstream
behaviours that a caller would otherwise have to know about already. Logging
them would put them somewhere a caller does not read; burying them in a doc
comment puts them somewhere a caller does not look at run time. So they come
back attached to the result, as [`Caveats`], and a caller that reports a
number without reporting these is reporting half of it.

```rust
pub mod error { /* ... */ }
```

### Types

#### Enum `Error`

Anything that can go wrong assembling a source term.

```rust
pub enum Error {
    TransientLengthMismatch {
        times: usize,
        temperatures: usize,
    },
    UnknownNuclide(String),
    TransientTooShort(usize),
    VentingTimeNotOnAxis {
        time_s: f64,
    },
    VentingLengthMismatch {
        expected: usize,
        got: usize,
    },
    VentingFractionOutOfRange {
        index: usize,
        value: f64,
    },
}
```

##### Variants

###### `TransientLengthMismatch`

The prescribed temperature transient and its time axis disagree.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `times` | `usize` | Number of time samples supplied. |
| `temperatures` | `usize` | Number of temperature samples supplied. |

###### `UnknownNuclide`

A nuclide was asked for that the TRISO-ATOPS supported table does not
contain. The table has 84 entries; see
`boon_lay::triso_atops_fork::nuclide_model::nuclide_database`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `TransientTooShort`

The transient is too short to integrate.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

###### `VentingTimeNotOnAxis`

A venting time came back from `coolant_release` that is not in the full
time axis it was derived from. This should be impossible and means the
two have drifted apart — see [`crate::accident::venting`].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `time_s` | `f64` | The offending time, in seconds. |

###### `VentingLengthMismatch`

A [`crate::accident::release::Venting::Prescribed`] fraction series does
not have one entry per transient sample.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `expected` | `usize` | Number of transient samples. |
| `got` | `usize` | Number of fractions supplied. |

###### `VentingFractionOutOfRange`

A prescribed venting fraction is not a finite number in `[0, 1]`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `index` | `usize` | Sample index. |
| `value` | `f64` | The offending value. |

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Error { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Error) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Type Alias `Result`

This crate's result type.

```rust
pub type Result<T> = core::result::Result<T, Error>;
```

#### Struct `Caveats`

Known upstream behaviours that affected a result, returned alongside it.

Every field records a **real, deliberate** upstream behaviour that
`boon-lay` reproduces faithfully. None is a bug in this workspace, and none
should be silently corrected here — but a reader who does not know about
them will misread the numbers.

```rust
pub struct Caveats {
    pub first_sample_forced_fully_vented: bool,
    pub negative_atom_count_seen: bool,
    pub diffusion_coefficient_clamped: bool,
    pub booth_transient_floored: bool,
    pub venting_mask_was_gappy: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `first_sample_forced_fully_vented` | `bool` | Upstream's `coolant_release` hard-codes `frac[0] = 1`, so the **first<br>sample is always treated as fully vented** regardless of what the<br>integral says. Always true when a venting calculation ran; recorded so<br>the first window's release is not read as a physical result. |
| `negative_atom_count_seen` | `bool` | A release somewhere in the chain went **negative**. Two different things<br>set this, and **they push the total in opposite directions**, so the<br>flag alone does not tell a reader which way the answer is wrong.<br><br>~~If this is set, at least one node-nuclide pair went negative and the<br>total is correspondingly under-stated.~~ **CORRECTED 2026-09-24** — that<br>was right for one of the two paths and wrong for the other:<br><br>1. **`release_activity` returning a negative atom count.** Upstream<br>   deliberately does not clamp it, and neither does this crate, so the<br>   negative propagates and the total is **under-stated**. This path<br>   needs a non-empty normal-operation pool to fire at all: with<br>   [`crate::accident::release::zero_pools`] every subtracted term is<br>   zero, so it cannot. **Since #448 (2026-09-30) real pools are the<br>   default, so this path CAN fire** (measured: Cs-137 in the HTR-10<br>   DLOFC case), and the flag no longer tells a reader which direction<br>   the total is wrong.<br>2. **A negative per-window first difference**, i.e. a *non-monotonic*<br>   cumulative release. That one is floored to zero at the<br>   [`changi::activity::source::SourceTerm`] boundary, which **raises**<br>   the sum of the windows above the cumulative endpoint — the total is<br>   **over-stated**.<br><br>Path 2 is reachable and is not hypothetical. **Silver** is the case:<br>the transient breakthrough release fraction<br>(`boon_lay::triso_atops_fork::release_models::transient::breakthrough_model_transient`)<br>rises, is then driven negative by its `−a/(2r)` time-lag term and<br>clamped to zero until breakthrough, and only then grows — so the<br>cumulative curie series falls over that stretch. Measured on the HTR-10<br>DLOFC case (`crate::htr10`, 2026-09-24): Ag-110m had **13 of 30 windows<br>negative**, and the windows sum to `2.503345e-2 Ci` against a cumulative<br>endpoint of `2.500569e-2 Ci` — **over-stated by a factor 1.0011**. No<br>other nuclide in that run had a single negative window. |
| `diffusion_coefficient_clamped` | `bool` | The Arrhenius diffusion coefficient is **clamped, never extrapolated**,<br>outside roughly 700-2400 degrees Celsius. If this is set, the transient<br>spent time outside the fitted range and the release there is governed by<br>a held-constant `D`, not by the correlation. |
| `booth_transient_floored` | `bool` | The transient Booth solution **floors at about 1.216e-4** rather than<br>reaching zero, so a nuclide that should release essentially nothing<br>still shows a small release fraction.<br><br>**NOT WIRED — nothing in this crate ever sets this field, so it is<br>always `false` on a computed result.** Stated here because a reader<br>finding it in a caveat struct would reasonably assume the condition is<br>detected, and it is not: verified 2026-09-24 by searching the workspace<br>for writes to it, which occur only in this module's own tests. It is<br>kept rather than deleted because the underlying behaviour is real —<br>`boon_lay::...::release_models::transient::booth_transient` snaps below<br>`BOOTH_TRANSIENT_ZERO_FLOOR = 1e-6` — and detecting it needs the<br>release-fraction values, which<br>[`crate::accident::release::accident_release`] does not currently keep. |
| `venting_mask_was_gappy` | `bool` | The venting mask was **not contiguous**. This is the condition under<br>which upstream's own prefix-versus-selection pairing goes wrong — see<br>[`crate::accident::venting`]. This crate does not have that defect, but<br>a result computed on a gappy mask cannot be compared against upstream's. |

##### Implementations

###### Methods

- ```rust
  pub const fn any(self: Self) -> bool { /* ... */ }
  ```
  Whether anything worth reporting happened. Always true in practice —

- ```rust
  pub fn lines(self: Self) -> Vec<&'static str> { /* ... */ }
  ```
  One line per caveat that fired, for printing beside a result.

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Caveats { /* ... */ }
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
    fn default() -> Caveats { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
    ```

- **Eq**
- **Equivalent**
  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
    ```

  - ```rust
    fn equivalent(self: &Self, key: &K) -> bool { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Caveats) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `htr10`

The HTR-10 depressurised-loss-of-forced-cooling case: PANAMA-I failure
fractions driving the TRISO-ATOPS release chain. See the module docs for
which inputs are HTR-10's own, which are stand-ins and which are estimates.
# HTR-10 depressurised loss of forced cooling — the PANAMA ↔ TRISO-ATOPS seam

This module joins the two halves of `boon-lay` that had never been run
together on one reactor:

```text
  fuel_failure (PANAMA-I)          triso_atops_fork (TRISO-ATOPS)
  phi_1, phi_2 over a transient -> f_inc_acc -> release fractions -> Ci
```

and drives the result with the **published HTR-10 equilibrium-core
inventory** (`changi::activity::inventory`, Liu & Cao 2002 Table 1). It
owns the transient *shape* and the seam; it owns no geometry and no
inventory, both of which already exist elsewhere in the workspace.

**RESEARCH, EDUCATION AND V&V ONLY.** Not a source term for HTR-10 or any
other plant, not for emergency planning, licensing or safety analysis. See
the workspace `RESPONSIBLE_USE.md`. Per `AI_USAGE.md` this is AI-assisted
draft material pending human review.

# Which seam is worth having, and why it is not the normal-operation one

`boon_lay::fuel_failure::htr10` established (2026-09-24) that PANAMA's
in-service failure fraction under **normal operation** is `10⁻¹⁵`–`10⁻⁶`
across HTR-10's plausible fuel-temperature band, against the `3·10⁻⁵`
as-manufactured placeholder that release calculations actually use.
Substituting one for the other would divide every reported activity by
about `10⁷` on the strength of a model answering a different question.

So this module wires PANAMA in where it belongs: as
[`AccidentFractions::incremental_accident`], the **accident-added**
in-service failure fraction. The four normal-operation fractions stay the
caller's, because PANAMA models none of them — its own equivalent, `φ_o`,
is an input to it too (HTA-IB-03/90 page -480-).

# The transient: HTR-Module's DLOFC history, used as a STAND-IN for HTR-10's

HTR-10's own depressurised-loss-of-forced-cooling fuel temperature history
is **not published in this workspace's literature.** What is available, in
the open JRC (V)HTR-Modul volume already used for this fuel line's
strength data, is HTR-Module's:

| Quantity | Value | Where |
|---|---|---|
| peak fuel temperature | **1500 °C** | §9.9.2 / §7.2.2: "After 30 hours the maximum fuel temperature reaches 1 500 °C. After reaching the maximum the temperature decreases." |
| time to peak | **30 h** | same, and §7.2.2 "around 1 500 °C in the hotspot region of the HTR Module after 30 hours" |
| duration above 1500 °C | ~30 h for < 5 % of elements, upper edge ~1550 °C | §7.2.2 |
| nominal / maximum peak | **1450 °C / 1615 °C** | Table 44 (uncertainty analysis, 200 MW HTR-Module) |
| fuel temperature limit | 1600 °C | stated throughout |

**Using HTR-Module's history for HTR-10 is a stand-in, and it is the
conservative direction.** HTR-10 is 10 MW thermal against HTR-Module's
200 MW, at a lower mean power density and with a far shorter heat-transport
path out of the core, so its own DLOFC peak is expected *below* 1500 °C.
The choice is also *consistent*: `boon_lay::fuel_failure::htr10` already
takes its `σ_oo`/`m_oo` and `Γ` stand-ins from this same HTR-Module column
of this same report, so the transient and the fuel strength come from one
reactor rather than two.

# Two parameters of the shape are ESTIMATES, and they are flagged as such

The report states the *rise* (1500 °C at 30 h) and that the temperature
then decreases, but its cooldown leg is a **figure** (Figures 15, 74, 158)
and is not digitised in this workspace. So:

- [`ESTIMATED_COOLDOWN_TIME_CONSTANT_HOURS`] — the post-peak relaxation
  time constant. **Estimated, not cited.**
- [`ESTIMATED_LATE_TIME_CELSIUS`] — the temperature the core relaxes
  towards. **Estimated, not cited.**

Both are swept in the example rather than asserted, and GitHub #296 asks
the maintainer for the digitised figure that would replace them. Their
influence is bounded and stated: TRISO-ATOPS's own venting model releases
only while the core is **heating** (`coolant_release` selects
`dT/dt >= 0`), so the cooldown leg contributes **nothing** to the vented
source term and these two estimates move the reported release by zero. They
affect PANAMA's failure accumulation only, where they act on a leg whose
temperature is falling and whose contribution is correspondingly small.

# The time grid, and the 0.39 % it costs

PANAMA steps between samples at the interval's midpoint temperature, so the
answer depends on the grid and the dependence is **measured** rather than
assumed away. Everything here and in the example runs at **201 samples**
over 200 h, a 1 h step. The convergence study in
[`tests::the_time_grid_converges_at_second_order_and_the_error_is_stated`]
shows second-order convergence from the 2 h step down and puts the
201-sample answer **0.39 % below** the extrapolated limit of
`1.071920·10⁻⁷`. The 4 h step is 16 % low and is not in the asymptotic
range. Nothing here is quoted to better than that.

# What the seam cannot yet do: the failure fraction is a SCALAR

[`crate::accident::release::accident_release`] takes one
[`AccidentFractions`] for the whole run, so a single number has to stand
for `f_inc_acc(t)`. Using the end-of-transient value applies the final
failure fraction from `t = 0` and therefore **over-predicts the early
release**; using the value at the peak under-predicts the late release,
which the venting model discards anyway. Both are reported. A
time-dependent failure fraction threaded through the release chain is the
obvious next step and is **not done here**.

```rust
pub mod htr10 { /* ... */ }
```

### Types

#### Struct `DlofcShape`

A DLOFC fuel-temperature history: linear rise to a peak, then exponential
relaxation towards a late-time level.

**An analytic shape, not a thermal-hydraulic solution.** Nothing in this
workspace computes HTR-10's DLOFC transient; `htgr_sim_v1`'s LOFC scenario
is a *pressurised* circulator trip arrested by Doppler feedback within the
hour and does not reach this regime.

```rust
pub struct DlofcShape {
    pub initial: uom::si::f64::ThermodynamicTemperature,
    pub peak: uom::si::f64::ThermodynamicTemperature,
    pub time_to_peak: uom::si::f64::Time,
    pub late_time: uom::si::f64::ThermodynamicTemperature,
    pub cooldown_time_constant: uom::si::f64::Time,
    pub total: uom::si::f64::Time,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `initial` | `uom::si::f64::ThermodynamicTemperature` | Pre-accident fuel temperature, `T_B`. |
| `peak` | `uom::si::f64::ThermodynamicTemperature` | Peak fuel temperature. |
| `time_to_peak` | `uom::si::f64::Time` | Time from accident start to the peak. |
| `late_time` | `uom::si::f64::ThermodynamicTemperature` | Temperature the core relaxes towards after the peak. |
| `cooldown_time_constant` | `uom::si::f64::Time` | Relaxation time constant of the cooling leg. |
| `total` | `uom::si::f64::Time` | Total duration carried. |

##### Implementations

###### Methods

- ```rust
  pub fn htr_module_jrc() -> Self { /* ... */ }
  ```
  The HTR-Module DLOFC history as the JRC volume states it, with the two

- ```rust
  pub fn with_peak(peak: ThermodynamicTemperature) -> Self { /* ... */ }
  ```
  The same shape at a different peak — for the Table 44 arms, or for a

- ```rust
  pub fn temperature_at(self: &Self, t: Time) -> ThermodynamicTemperature { /* ... */ }
  ```
  The fuel temperature at one instant.

- ```rust
  pub fn sample_times(self: &Self, samples: usize) -> Vec<Time> { /* ... */ }
  ```
  Uniformly spaced sample times over `[0, total]`.

- ```rust
  pub fn history(self: &Self, samples: usize) -> Vec<ThermodynamicTemperature> { /* ... */ }
  ```
  The temperature history at those sample times.

- ```rust
  pub fn transient(self: &Self, samples: usize, n_radial: usize, n_axial: usize) -> Result<TemperatureTransient> { /* ... */ }
  ```
  The shape as a [`TemperatureTransient`], every node on the same history.

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> DlofcShape { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &DlofcShape) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `PanamaOverTransient`

PANAMA-I walked over a DLOFC history: the failure trail and the two scalars
the release chain can consume.

```rust
pub struct PanamaOverTransient {
    pub times: Vec<uom::si::f64::Time>,
    pub temperature_celsius: Vec<f64>,
    pub phi_1: Vec<f64>,
    pub phi_2: Vec<f64>,
    pub in_service: Vec<f64>,
    pub end_of_irradiation_phi_1: f64,
    pub accident_increment_final: f64,
    pub accident_increment_at_peak: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `times` | `Vec<uom::si::f64::Time>` | Sample times, matching [`DlofcShape::sample_times`]. |
| `temperature_celsius` | `Vec<f64>` | Fuel temperature at each sample, °C. |
| `phi_1` | `Vec<f64>` | `φ₁`, pressure-vessel overstress, at each sample. |
| `phi_2` | `Vec<f64>` | `φ₂`, SiC thermal decomposition, at each sample. |
| `in_service` | `Vec<f64>` | `f_inc = 1 − (1−φ₁)(1−φ₂)` at each sample — PANAMA's in-service failure<br>fraction, which is what TRISO-ATOPS calls `incremental`. |
| `end_of_irradiation_phi_1` | `f64` | `φ₁` at the end of irradiation, i.e. `f_inc` at `t = 0`. PANAMA sets<br>`φ₁(t=0)` to this rather than to zero (page -482-). |
| `accident_increment_final` | `f64` | `f_inc(end) − f_inc(0)` — the accident-added failure over the whole<br>transient. The **conservative** arm for `f_inc_acc`. |
| `accident_increment_at_peak` | `f64` | `f_inc(t_peak) − f_inc(0)` — the accident-added failure up to the peak,<br>which is where the venting window ends. The **consistent** arm. |

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> PanamaOverTransient { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &PanamaOverTransient) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `Htr10Geometry`

The HTR-10 particle and pebble dimensions a release needs.

**Deliberately not a constructor with values in it.** The published HTR-10
dimensions live in `tampines::pebble_bed` (`TrisoParticle::htr10`,
`Pebble::htr10`), sourced to IAEA-TECDOC-1382 part 2 Table 4-17, and this
crate does not depend on `tampines`. A second copy of them here is exactly
the drift the workspace forbids, so the caller reads them from `tampines`
and passes them in. The example and this module's tests both do.

```rust
pub struct Htr10Geometry {
    pub kernel_radius: uom::si::f64::Length,
    pub sic_thickness: uom::si::f64::Length,
    pub graphite_thickness: uom::si::f64::Length,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `kernel_radius` | `uom::si::f64::Length` | Fuel kernel radius `r` — HTR-10: 250 µm. |
| `sic_thickness` | `uom::si::f64::Length` | SiC layer thickness `a_SiC` — HTR-10: 35 µm (380 → 415 µm). |
| `graphite_thickness` | `uom::si::f64::Length` | Matrix graphite diffusion thickness `a_graph` — HTR-10: the 5 mm<br>unfuelled pebble shell. |

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Htr10Geometry { /* ... */ }
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

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Htr10Geometry) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Functions

#### Function `panama_over_transient`

**Attributes:**

- `MustUse { reason: None }`

Walk PANAMA-I over a DLOFC history for the HTR-10 particle.

The particle is `boon_lay::fuel_failure::htr10::particle`: HTR-10's
published geometry, kernel compound, derived burnup (`F_b = 0.0851` FIMA)
and derived residence (`t_B = 1080` FPD), with the two named HTR-Module
stand-ins for `σ_oo`/`m_oo` and `Γ`. Read that module before quoting any
number this returns.

`φ_o` is **zero** in the particle, deliberately: the as-manufactured
population is carried by TRISO-ATOPS's own `f_hm` / `f_sic`, and counting it
in both places would double it.

# Arguments
- `shape` — the temperature history.
- `irradiation_temperature` — `T_B`. PANAMA's oxygen and diffusion terms
  need it and it is **not** the accident temperature.
- `samples` — number of history samples; the accident is stepped between
  consecutive samples at their midpoint temperature, which is PANAMA's own
  `T_m` (page -482-).

# Panics
Panics if `samples < 2`.

```rust
pub fn panama_over_transient(shape: &DlofcShape, irradiation_temperature: uom::si::f64::ThermodynamicTemperature, samples: usize) -> PanamaOverTransient { /* ... */ }
```

#### Function `np_mhtgr_normal_operation_fractions`

**Attributes:**

- `MustUse { reason: None }`

TRISO-ATOPS's own NP-MHTGR reference **normal-operation** failure
fractions, with both accident fields zero.

`f_hm = 1·10⁻⁴`, `f_sic = 1·10⁻⁴`, `f_inc = 2.3·10⁻⁵`,
`f_inc_sic = 3.6·10⁻⁵` — read out of
[`PlantParameters::np_mhtgr_reference`] rather than retyped, so there is one
copy of them in this workspace.

**These are NOT HTR-10 fuel-qualification data.** They are a manufacturing
and irradiation result for the NP-MHTGR reference fuel. HTR-10's fuel is
German-lineage, and the measured German record —
`boon_lay::fuel_failure::htr10::qualification::FREE_URANIUM_FRACTIONS`,
`7.8·10⁻⁶` to `50.7·10⁻⁶` — is roughly an order of magnitude *better* than
the `1·10⁻⁴` here. Release from noble gases and halogens is **linear** in
`f_hm`, so that is a factor ~2–13 on every volatile number computed with
this set. Both arms are reported in the example rather than one being
chosen.

```rust
pub fn np_mhtgr_normal_operation_fractions() -> boon_lay::triso_atops_fork::accident::AccidentFractions { /* ... */ }
```

#### Function `with_panama_accident_increment`

**Attributes:**

- `MustUse { reason: None }`

Set the accident-phase incremental fraction from PANAMA, leaving the four
normal-operation fractions untouched.

`f_inc_sic_acc` is set to **zero**, and that is a considered refusal rather
than an omission: PANAMA's `φ₂` is an in-service *loss* of the SiC layer to
thermal decomposition, while TRISO-ATOPS's `f_inc_sic` is a distinct
as-manufactured-population field. `φ₂` is already carried inside
`f_inc_acc` through
`boon_lay::fuel_failure::history::FailureProgress::in_service_failure_fraction`,
so routing it to `incremental_sic_accident` as well would both double-count
it and invent a correspondence neither code states.

```rust
pub fn with_panama_accident_increment(base: boon_lay::triso_atops_fork::accident::AccidentFractions, increment: f64) -> boon_lay::triso_atops_fork::accident::AccidentFractions { /* ... */ }
```

#### Function `plant_parameters`

**Attributes:**

- `MustUse { reason: None }`

Assemble [`PlantParameters`] for an HTR-10 DLOFC.

- `coolant_pressure` is one atmosphere: this is a **depressurised**
  accident, which is what the transient shape describes.
- ~~`x_liftoff` is zero … `zero_pools` means the accident starts with
  nothing plated out … **under-predicts**~~ **CORRECTED 2026-09-30 (#448):**
  the accident now starts from real normal-operation pools, so a zero
  lift-off would silently omit plate-out re-entrainment. `x_liftoff` is
  **0.05, an NP-MHTGR stand-in** (Stoyer et al. 2026 Case A, Table 3). No
  HTR-10 value is in the corpus; Liu & Cao 2002 instead assume desorption
  of 2.4 × the coolant activity.
- `clean_up_fitted` is `true`: HTR-10 has a helium purification system.
- **Normal operation** ([`PrimaryCircuitPools::FromNormalOperation`]):
  - `run_time` **20 y**: Liu & Cao 2002's circulating activity is "at the
    end of 20 years of full-power operation";
  - `irradiation_time` **1080 FPD**: `boon_lay::fuel_failure::htr10::RESIDENCE_FULL_POWER_DAYS`;
  - `k_plate`, `k_clean`, `a_grain`: **NP-MHTGR stand-ins**
    ([`NormalOperation::np_mhtgr_reference`]);
  - fuel and graphite temperatures: **the 776 °C stand-in**
    [`STAND_IN_IRRADIATION_CELSIUS`], uniform (#297).

```rust
pub fn plant_parameters(geometry: Htr10Geometry, fractions: boon_lay::triso_atops_fork::accident::AccidentFractions) -> crate::accident::release::PlantParameters { /* ... */ }
```

#### Function `stand_in_irradiation_temperature`

**Attributes:**

- `MustUse { reason: None }`

`T_B` as a `uom` temperature, for callers that do not want to reach for the
unit themselves.

```rust
pub fn stand_in_irradiation_temperature() -> uom::si::f64::ThermodynamicTemperature { /* ... */ }
```

#### Function `isothermal_failure`

**Attributes:**

- `MustUse { reason: None }`

The in-service failure fraction PANAMA gives for an isothermal hold — the
form the report's own heating experiments have, and the form the
1600 °C/200 h comparison in HTA-IB-03/90 page -504- is stated in.

Returns `(φ₁, φ₂, f_inc)` at the end of the hold.

```rust
pub fn isothermal_failure(irradiation_temperature: uom::si::f64::ThermodynamicTemperature, accident: uom::si::f64::ThermodynamicTemperature, hold: uom::si::f64::Time, steps: usize) -> (f64, f64, f64) { /* ... */ }
```

### Constants and Statics

#### Constant `DLOFC_PEAK_CELSIUS`

Peak fuel temperature of the HTR-Module DLOFC history, **1500 °C**
(JRC EUR 28712 EN §9.9.2: "After 30 hours the maximum fuel temperature
reaches 1 500 °C").

```rust
pub const DLOFC_PEAK_CELSIUS: f64 = 1500.0;
```

#### Constant `DLOFC_TIME_TO_PEAK_HOURS`

Time to that peak, **30 h**, from the same sentence.

```rust
pub const DLOFC_TIME_TO_PEAK_HOURS: f64 = 30.0;
```

#### Constant `DLOFC_TABLE_44_NOMINAL_CELSIUS`

Table 44's **nominal** calculated peak fuel temperature for the same
accident, 1450 °C — the low arm of the report's own uncertainty analysis.

```rust
pub const DLOFC_TABLE_44_NOMINAL_CELSIUS: f64 = 1450.0;
```

#### Constant `DLOFC_TABLE_44_MAXIMUM_CELSIUS`

Table 44's **maximum** calculated peak fuel temperature, 1615 °C — the high
arm, and the only one of the three that exceeds the 1600 °C limit.

```rust
pub const DLOFC_TABLE_44_MAXIMUM_CELSIUS: f64 = 1615.0;
```

#### Constant `FUEL_TEMPERATURE_LIMIT_CELSIUS`

The fuel temperature limit the whole design argument is built around,
1600 °C (stated throughout the JRC volume).

```rust
pub const FUEL_TEMPERATURE_LIMIT_CELSIUS: f64 = 1600.0;
```

#### Constant `REPORTING_WINDOW_HOURS`

How long the transient is carried, **200 h** — the window HTA-IB-03/90
itself reports HTR-Module depressurised failure at (page -504-), so PANAMA
results here are directly comparable with that statement.

```rust
pub const REPORTING_WINDOW_HOURS: f64 = 200.0;
```

#### Constant `STAND_IN_IRRADIATION_CELSIUS`

`T_B`, the pre-accident average fuel temperature, **776 °C** — HTR-Module's
published average (HTA-IB-03/90 Table 2). **A stand-in, not HTR-10 data:**
HTR-10 publishes a *maximum* fuel temperature and PANAMA's `T_B` is an
average. Same stand-in `boon_lay::fuel_failure::htr10` uses.

```rust
pub const STAND_IN_IRRADIATION_CELSIUS: f64 = 776.0;
```

#### Constant `ESTIMATED_COOLDOWN_TIME_CONSTANT_HOURS`

**ESTIMATE, not cited.** Post-peak relaxation time constant, 60 h.

The JRC volume states the peak and that the temperature falls after it, but
gives the decay only as a figure. 60 h is chosen so the core is back near
its late-time level by the 200 h reporting window, which is the shape the
report's figures show; nothing in the text fixes it.

It cannot move the reported release (see the module docs: the venting model
discards every cooling sample), so it is a sensitivity on PANAMA's failure
accumulation and nothing else.

```rust
pub const ESTIMATED_COOLDOWN_TIME_CONSTANT_HOURS: f64 = 60.0;
```

#### Constant `ESTIMATED_LATE_TIME_CELSIUS`

**ESTIMATE, not cited.** The temperature the core relaxes towards after the
peak, 900 °C.

Chosen above the 700 °C lower edge of TRISO-ATOPS's fitted Arrhenius range
(`crate::accident::release::DIFFUSION_FIT_MIN_CELSIUS`) so the late leg is
computed by the correlation rather than by its clamp — which means the
choice is visible in the caveats if it is changed downwards, rather than
silently altering the physics.

```rust
pub const ESTIMATED_LATE_TIME_CELSIUS: f64 = 900.0;
```

## Module `inventory`

The core radionuclide inventory, prescribed.

# This is an INPUT, and this crate does not compute it

Nothing here derives an inventory from power history, burnup or fission
yields. It is supplied by the caller, and the reason is worth stating
because the obvious shortcut is wrong in a way that does not announce
itself:

**Do not build one from `fission-yields-data`.** That crate exposes
**independent** fission yields — the yield *directly* from fission, before
any beta decay. Most of the nuclides that dominate a source term do not
arrive that way. Cs-137's independent yield is roughly **two orders of
magnitude** below its cumulative yield, because Cs-137 is reached down the
A = 137 isobaric chain (Xe-137 to Cs-137) rather than produced directly.
Using independent yields naively gives a silently ~100x low inventory for
most consequence-dominant species — low, so it looks reassuring, and with
no error anywhere.

Getting a cumulative yield right needs a decay-chain walk over the
evaluation. That is real work and is deliberately out of scope here.

# Radial and axial distribution

An inventory is given **per radial ring**, and spread over the axial nodes
by `boon-lay`'s `distribute_inventory_axially`, which is upstream's own
distribution. This crate does not invent a shape.

```rust
pub mod inventory { /* ... */ }
```

### Types

#### Struct `NuclideInventory`

One nuclide's core inventory, before the accident starts.

```rust
pub struct NuclideInventory {
    pub name: String,
    pub per_ring: Vec<uom::si::f64::Radioactivity>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | TRISO-ATOPS nuclide name, e.g. `"Cs-137"`. Must be one of the 84 in<br>`boon_lay::triso_atops_fork::nuclide_model::nuclide_database`. |
| `per_ring` | `Vec<uom::si::f64::Radioactivity>` | Activity in each radial ring, innermost first. Length sets `n_radial`. |

##### Implementations

###### Methods

- ```rust
  pub fn uniform(name: &str, per_ring: Radioactivity, n_radial: usize) -> Self { /* ... */ }
  ```
  Every ring carrying the same activity.

- ```rust
  pub fn total(self: &Self) -> Radioactivity { /* ... */ }
  ```
  This nuclide's total across every ring.

- ```rust
  pub fn axial_curies(self: &Self, ring: usize, n_axial: usize) -> Vec<f64> { /* ... */ }
  ```
  This nuclide's inventory in one ring, spread over the axial nodes.

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> NuclideInventory { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &NuclideInventory) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `CoreInventory`

The whole core's inventory: every nuclide, resolved by radial ring.

```rust
pub struct CoreInventory {
    pub nuclides: Vec<NuclideInventory>,
    pub n_radial: usize,
    pub n_axial: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nuclides` | `Vec<NuclideInventory>` | One entry per nuclide. |
| `n_radial` | `usize` | Number of radial rings. Every nuclide must supply this many. |
| `n_axial` | `usize` | Number of axial nodes each ring is split into. |

##### Implementations

###### Methods

- ```rust
  pub fn new(nuclides: Vec<NuclideInventory>, n_radial: usize, n_axial: usize) -> Self { /* ... */ }
  ```
  Build and validate.

- ```rust
  pub fn unit(names: &[&str], n_radial: usize, n_axial: usize) -> Self { /* ... */ }
  ```
  **A shakedown inventory: exactly one curie of each nuclide, per ring.**

- ```rust
  pub fn names(self: &Self) -> Vec<&str> { /* ... */ }
  ```
  The nuclide names, in order, for handing to

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> CoreInventory { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CoreInventory) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `scenario`

The prescribed temperature transient.

# This crate does not solve for a temperature history

It consumes one. Nothing here is a thermal-hydraulic model, and the shape
of the transient is the single largest determinant of how much is released
— so a run's credibility is the credibility of whatever produced its
transient, not of this code.

Two constructors, deliberately:

- [`TemperatureTransient::from_ramp`] — an analytic ramp and hold. For
  shakedown, sensitivity sweeps and demonstrating the chain. Not a plant
  calculation.
- [`TemperatureTransient::from_nodes`] — an arbitrary per-node history, so a
  trace from `outram-park-digital-twin-engine`'s `htgr_sim_v1` headless mode
  (or any other source) drops in unchanged.

The second exists now rather than later on purpose: adding it while the
shape of the type is still being decided costs nothing, and retrofitting it
after the API has consumers costs a lot.

# Index order

`temperatures[ring][time][axial]`, matching the layout `boon-lay`'s
code-to-code fixture uses (`sc.temps[r][t][k]`), so a caller moving between
the two does not have to transpose.

```rust
pub mod scenario { /* ... */ }
```

### Types

#### Struct `TemperatureTransient`

A prescribed temperature history over the core nodes.

```rust
pub struct TemperatureTransient {
    pub times: Vec<uom::si::f64::Time>,
    pub temperatures: Vec<Vec<Vec<uom::si::f64::ThermodynamicTemperature>>>,
    pub n_radial: usize,
    pub n_axial: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `times` | `Vec<uom::si::f64::Time>` | Sample times, ascending, from the start of the accident. |
| `temperatures` | `Vec<Vec<Vec<uom::si::f64::ThermodynamicTemperature>>>` | `[ring][time][axial]`, degrees Celsius carried as<br>[`ThermodynamicTemperature`]. |
| `n_radial` | `usize` | Number of radial rings. |
| `n_axial` | `usize` | Number of axial nodes. |

##### Implementations

###### Methods

- ```rust
  pub fn from_nodes(times: Vec<Time>, temperatures: Vec<Vec<Vec<ThermodynamicTemperature>>>) -> Result<Self> { /* ... */ }
  ```
  From an arbitrary per-node history.

- ```rust
  pub fn from_ramp(start: ThermodynamicTemperature, peak: ThermodynamicTemperature, ramp: Time, total: Time, samples: usize, n_radial: usize, n_axial: usize) -> Result<Self> { /* ... */ }
  ```
  A linear ramp from `start` to `peak` over `ramp`, then a hold at `peak`

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Number of time samples.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether there are no samples. Never true for a constructed transient,

- ```rust
  pub fn end(self: &Self) -> Time { /* ... */ }
  ```
  When the transient ends.

- ```rust
  pub fn node_history(self: &Self, ring: usize, axial: usize) -> Vec<ThermodynamicTemperature> { /* ... */ }
  ```
  One node's history, `[time]`.

- ```rust
  pub fn all_node_histories(self: &Self) -> Vec<Vec<ThermodynamicTemperature>> { /* ... */ }
  ```
  Every node's history, flattened ring-major, as

- ```rust
  pub fn hot_node_history(self: &Self) -> Vec<ThermodynamicTemperature> { /* ... */ }
  ```
  The history of the node taken as hottest: innermost ring, mid-height.

- ```rust
  pub fn peak_celsius(self: &Self) -> f64 { /* ... */ }
  ```
  The peak temperature anywhere in the transient, for checking it against

- ```rust
  pub fn min_celsius(self: &Self) -> f64 { /* ... */ }
  ```
  The minimum temperature anywhere in the transient.

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> TemperatureTransient { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Downcast**
  - ```rust
    fn downcast(self: &Self) -> &T { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TemperatureTransient) -> bool { /* ... */ }
    ```

- **Pointable**
  - ```rust
    unsafe fn init(init: <T as Pointable>::Init) -> usize { /* ... */ }
    ```

  - ```rust
    unsafe fn deref<''a>(ptr: usize) -> &'a T { /* ... */ }
    ```

  - ```rust
    unsafe fn deref_mut<''a>(ptr: usize) -> &'a mut T { /* ... */ }
    ```

  - ```rust
    unsafe fn drop(ptr: usize) { /* ... */ }
    ```

- **Read**
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
## Module `units`

The one place `boon-lay`'s activity type crosses into `changi`'s.

# The problem in one paragraph

`boon-lay` types activity as `Activity = uom::si::f64::Frequency`, because
the physics relation is `A = lambda * N` and with `N` dimensionless that
reads `Bq = s^-1 * 1`. `changi` types it as
[`uom::si::f64::Radioactivity`], which is the same **dimension** but a
different **Rust type**, and which carries a built-in `@curie` unit so the
`3.7e10` never has to be written down. Neither is wrong and neither is
going to change: `boon-lay`'s alias is code-to-code verified against
upstream TRISO-ATOPS and human-signed-off.

So a conversion is unavoidable. **It lives here, in this file, and nowhere
else in this crate.** A conversion scattered across call sites is how two
type systems of the same dimension quietly start disagreeing about which
one a given number is in — and because they *are* the same dimension, the
compiler would not catch it if someone reached for `.get::<hertz>()` on the
wrong one.

# Why there is no Bq/s type

A becquerel is already `s^-1`, so a release *rate* has dimension `T^-2`.
`uom` will happily name that type and no human reads it as a release rate.
A release therefore travels as an **activity attached to a window**
(`changi`'s [`changi::activity::source::ReleaseWindow`]), and any consumer
wanting a rate divides by the window's own duration at the point of use.

```rust
pub mod units { /* ... */ }
```

### Functions

#### Function `to_radioactivity`

**Attributes:**

- `MustUse { reason: None }`

`boon-lay` activity (a `Frequency` carrying becquerels) into `changi`'s
[`Radioactivity`].

Numerically the identity — both store becquerels — so this costs nothing at
run time. Its whole job is to be the one place the type changes.

```rust
pub fn to_radioactivity(activity: boon_lay::triso_atops_fork::Activity) -> uom::si::f64::Radioactivity { /* ... */ }
```

#### Function `to_boon_lay_activity`

**Attributes:**

- `MustUse { reason: None }`

[`Radioactivity`] back into `boon-lay`'s activity type.

The inverse of [`to_radioactivity`], for feeding a `changi`-side figure back
into a `boon-lay` routine.

```rust
pub fn to_boon_lay_activity(activity: uom::si::f64::Radioactivity) -> boon_lay::triso_atops_fork::Activity { /* ... */ }
```

#### Function `from_curies`

**Attributes:**

- `MustUse { reason: None }`

An activity given in curies.

TRISO-ATOPS run files specify per-nuclide inventories in curies, and its
output columns are in Ci and Ci/s, so this is the boundary most inventory
numbers arrive through. It goes via `uom`'s own `@curie` unit rather than
multiplying by [`BQ_PER_CI`].

```rust
pub fn from_curies(curies: f64) -> uom::si::f64::Radioactivity { /* ... */ }
```

#### Function `in_curies`

**Attributes:**

- `MustUse { reason: None }`

An activity read back out in curies, for comparison against TRISO-ATOPS
output.

```rust
pub fn in_curies(activity: uom::si::f64::Radioactivity) -> f64 { /* ... */ }
```

#### Function `decay_constant`

**Attributes:**

- `MustUse { reason: None }`

A decay constant from a half-life, `lambda = ln(2) / t_half`.

Uses `core`'s [`core::f64::consts::LN_2`], **not** the truncated `0.693147`
that `changi::flexpart::decay::decay_constant` carries — that literal is
upstream FLEXPART's and is reproduced there deliberately, but it has no
business on this path.

# Panics
Panics if `half_life` is not strictly positive.

```rust
pub fn decay_constant(half_life: uom::si::f64::Time) -> uom::si::f64::Frequency { /* ... */ }
```

### Constants and Statics

#### Constant `BQ_PER_CI`

Becquerels per curie, `1 Ci = 3.7e10 Bq`, exact by definition.

Re-exported from `boon-lay` rather than redefined, so the two cannot drift.
Prefer [`from_curies`], which goes through `uom`'s own `@curie` and does not
name the constant at all.

```rust
pub const BQ_PER_CI: f64 = boon_lay::triso_atops_fork::activities::BQ_PER_CI;
```

## Constants and Statics

### Constant `SCOPE`

The scope this crate reserves, as a machine-readable string.

Exists so the placeholder has *something* testable and so a downstream
`use sembawang::SCOPE;` fails loudly if the crate is ever repurposed
without updating its own documentation.

```rust
pub const SCOPE: &str = "severe accident progression; source term";
```

## Re-exports

### Re-export `Error`

```rust
pub use error::Error;
```

### Re-export `Result`

```rust
pub use error::Result;
```

