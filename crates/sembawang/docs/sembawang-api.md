# Crate Documentation

**Version:** 0.0.1

**Format Version:** 61

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
  hydrogen and aerosol release (~~**not implemented**~~ **CORRECTED 2026-10-29 (gh:#1033)**:
  a reduced, lumped model for ONE plant, Fukushima Daiichi Unit 1, in
  [`bwr_mark1`]: core heat-up, Zr-steam oxidation and hydrogen,
  relocation, a lower-head failure criterion, and containment
  pressurisation. MCCI and aerosols are not implemented).
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
**UPDATED again 2026-09-30 (#452, #453):** [`lwr_comparison`] is library
code that calls that same `buangkok` arithmetic, so that the HTR-10
bounding air-ingress case and its LWR counterparts (NUREG-1465, RG 1.183,
WASH-1400 PWR 8) reach the example and `htgr_sim_v1` from one function.
~~"The library computes no dose quantity"~~ no longer holds for that
module; the dose there is research-grade and indicative only.

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

~~**What does not exist: severe-accident progression**, which is the rest of
the crate's name and the larger half. There is no melt behaviour, no
relocation, no vessel failure, no molten-core-concrete interaction, no
hydrogen, and no aerosol physics. The temperature transient this crate
consumes is **prescribed by the caller**; nothing here computes it.~~
**CORRECTED 2026-10-29 (gh:#1033)** (maintainer: "Physics libraries go to sembawang"):
**a reduced progression model exists for one plant.** [`bwr_mark1`] is
Fukushima Daiichi Unit 1 (BWR/3, Mark I) after the scram: vessel, core
heat-up with Zr-steam oxidation and hydrogen, relocation, a lower-head
failure criterion, and the drywell, wetwell and reactor building. It was
the `dhoby-ghaut` `fukushima_web` demo's own physics until it moved here.
It is lumped (a few nodes), an AI draft, uncalibrated, and **outside the
BSAF Unit 1 bands**: the lower head fails about 2 h early and in-vessel
hydrogen is about 3x low ([`bwr_mark1::compare`], gh:#1028). Still absent
everywhere in the crate: molten-core-concrete interaction, aerosol
physics, fission-product transport in the plant, and any MELCOR-class
control-volume engine. The TRISO release path still takes a
**caller-prescribed** temperature transient; [`bwr_mark1`] does not feed
it.

So: cite this crate for a TRISO release calculation under a temperature
history you supplied, and (since 2026-09-21) for carrying that release
through `changi` to air concentration and deposition. The line below is the
original, kept because it is still true as far as it goes:

> cite this crate for a TRISO release calculation under a temperature
> history you supplied. Do **not** cite it for accident progression.

(2026-10-29: [`bwr_mark1`] may be cited as a reduced, unvalidated Unit 1
model, with its BSAF disagreement; not as a severe-accident code.)

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

It still has **no ~~engine for progression~~ general engine for
progression** (**CORRECTED 2026-10-29 (gh:#1033)**: [`bwr_mark1`] is a reduced
single-plant model, not an engine). General severe-accident progression
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
- ~~**Any dose quantity in the library**, here or in [`changi`]. The library
  chain stops at activity in air and on the ground. Dose comes from
  `buangkok`, and is used only by the `htr10_air_ingress_kora_bound` example.~~
  **CORRECTED 2026-09-30 (#452):** [`lwr_comparison::max_dose`] computes a
  research-grade dose through `buangkok`'s plume and FGR coefficients, for
  the `lwr_nureg1465_counterpart` example and `htgr_sim_v1` (#453). The
  HTR-10 source-term chain itself still stops at activity.
  [`lwr_comparison::rg1183_containment_leakage`] is a one-node LWR
  containment leak for RG 1.183's method only, with no removal credit. It
  is not HTR containment transport, which the entry above still says does
  not exist.

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
| `screened_out` | `Vec<String>` | Nuclides dropped by the **half-life screen** (t½ below 4 % of the<br>transient), as supplied. ~~"with the reason"~~: no reason is stored,<br>and since #449 unknown names are an error, never listed here. |
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    Ventilation {
        rate: uom::si::f64::Frequency,
        cut_off: Option<uom::si::f64::Time>,
    },
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

**Where upstream has no answer (#447, #449):** if *every* sample
vents, as in a monotonic heat-up or any `from_ramp` ramp-and-hold,
upstream's `accident_temp[:, :-0, :]` is empty and it raises
`IndexError`. This port returns the ideal-gas fraction `≈ 1 − T0/T`
instead: a defined answer, but **not an upstream-verified one**.

A spatially **non-uniform field that is constant in time** is now an
**error**, [`Error::NoVentingTransport`] (#447, 2026-09-30).
~~(every `frac` after the first is 0: a silent 0 Bq)~~. Upstream
raises there. Heat-up venting moves nothing, but a real core leaks and
convects, so the caller picks [`Venting::Ventilation`],
[`Venting::FullFlowThrough`] or [`Venting::Prescribed`].

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

###### `Ventilation`

**Ventilation / gas exchange** (#447): the core gas is exchanged at a
constant rate `λ` (air changes per unit time), well mixed, until an
optional cut-off, after which it is sealed:
`frac(t) = 1 − exp(−λ · min(t, cut_off))`.

For transport that upstream's heat-up venting does not have:
convection, leakage and ventilation through a break, which carry the
release out of a core **whatever its temperature does**. See
[`Venting::gao_shi_htr10_cavity_ventilation`] for the sourced HTR-10
case. Not upstream.

**It errs high, and the docs say so:** `frac(t)` multiplies the
*cumulative* fuel release (upstream's `accident_totals` structure), so
activity released late is credited the same escape fraction as activity
released at `t = 0`, and activity released after the cut-off still
escapes at the frozen fraction. A convolution over release time would
give less.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `rate` | `uom::si::f64::Frequency` | Exchange rate `λ`, air changes per unit time. |
| `cut_off` | `Option<uom::si::f64::Time>` | When the exchange stops (the break is sealed). `None` means never. |

##### Implementations

###### Methods

- ```rust
  pub fn gao_shi_htr10_cavity_ventilation() -> Self { /* ... */ }
  ```
  **HTR-10 air ingress, the reactor-cavity ventilation**, from Gao & Shi

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
- `inventory` — the core inventory, prescribed or from depletion
  ([`RadionuclideInventory`], GitHub #951). Its radial rings must match
  `transient`'s; the axial split follows `transient`'s `n_axial`.
- `transient` — the prescribed temperature history.
- `plant` — geometry, failure fractions and plant constants.

# The half-life screen sets the nuclide list

`select_nuclides_accident` drops any nuclide whose half-life is below 4 % of
the accident duration, on the grounds that it decays away before it matters.
**That threshold is relative, so lengthening the transient screens out
more.** Whichever nuclides it drops are returned in
[`AccidentRelease::screened_out`] rather than vanishing.

# Errors
[`Error::UnknownNuclide`] if **any** inventory name does not parse or is not
in the supported table. Non-canonical spellings (`cs137`) are normalised, as
upstream does. ~~(only when no nuclide survives)~~ **CORRECTED 2026-09-30
(#449)**: unknown names were previously listed in `screened_out`;
[`Error::TransientTooShort`] if the venting calculation has too little to
work with; [`Error::VentingTimeNotOnAxis`] if the venting selection cannot
be reconciled with the time axis; [`Error::InventoryRingMismatch`]
(~~a panic~~ an error since GitHub #951) if the inventory and `transient`
disagree on the radial ring count, and [`Error::InventoryNotInRings`] for
an inventory over named regions.

```rust
pub fn accident_release(inventory: &crate::inventory::RadionuclideInventory, transient: &crate::scenario::TemperatureTransient, plant: &PlantParameters) -> crate::error::Result<AccidentRelease> { /* ... */ }
```

#### Function `accident_release_with_venting`

[`accident_release`] with the core-venting (transport) mode chosen
explicitly; see [`Venting`]. [`accident_release`] is this with
[`Venting::Upstream`].

# Errors
As [`accident_release`], plus [`Error::VentingLengthMismatch`] and
[`Error::VentingFractionOutOfRange`] for a bad [`Venting::Prescribed`].

```rust
pub fn accident_release_with_venting(inventory: &crate::inventory::RadionuclideInventory, transient: &crate::scenario::TemperatureTransient, plant: &PlantParameters, venting_mode: &Venting) -> crate::error::Result<AccidentRelease> { /* ... */ }
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
Celsius. Crossing it sets [`Caveats::diffusion_coefficient_clamped`].
~~Outside it `boon-lay` clamps rather than extrapolating~~ **CORRECTED 2026-09-30 (#449):**
`boon-lay` clamps only group-specific lower limits and extrapolates
everything else; see that caveat's docs.

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
**only when the venting mask is a contiguous run starting at index 0.**
~~For a monotonically heating transient it always is, which is why the
defect survives~~ **CORRECTED 2026-09-30 (#449):** if *every* sample heats, upstream's
`[:-0]` slice is empty, and it **raises `IndexError`** (#447). The pairing
defect survives because upstream's reference cases heat and then cool,
giving a contiguous prefix, and never reheat.

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

## Module `ap1000_ted`

AP1000 severe-accident plume-centreline TED digitised from Dadda et al.
(2024) Fig. 7: a published comparison curve for `htgr_sim_v1` (#473).
# AP1000 severe-accident plume-centreline TED (Dadda et al. 2024, Fig. 7)

A **published LWR dose curve**, loaded as a comparison overlay for
`htgr_sim_v1`'s centreline TEDE graph (maintainer request, 2026-10-01,
#473). Nothing here is computed by this crate's own source-term chain: it
is the maintainer's digitisation of a figure, summed and scaled.

**Source.** A. Dadda et al., "Source term analysis and impact study during
a hypothetical accident in Haiyang nuclear power plant", *Radiation Physics
and Chemistry* 218 (2024) 111542, Fig. 7, the April–August period
(Pasquill class B, 4.44 m/s). Data and full provenance (digitisation date,
log-log axis calibration, licence basis) in the header of
`reference/lwr/dadda2024_fig7_ap1000_centreline_ted_apr_aug.csv`, loaded
with `include_str!` (no runtime file I/O; wasm/Android safe).

**Their model, not ours.** HotSpot 3.1; AP1000 at 3400 MWt; RG 1.183
group release fractions (an unmitigated core melt, no containment leak
rate stated); 100 m stack; 2 h release interval; TED = inhalation CEDE +
submersion + ground shine + **resuspension** (their Eq. 7). Every one of
those differs from `htgr_sim_v1`'s accumulated dose ([`MISMATCHES`]).

**Summing the groups.** The eight groups were digitised at different x
points, so [`total_ted`] interpolates each group **linearly in
`(log x, log TED)`** (the axes it was read off) and sums. Outside a
group's own digitised x range that group contributes **0 and is listed in
[`TotalTed::groups_outside`]** -- nothing is extrapolated. (Ce, Ba/Sr and
the alkali metals start at ~0.2 km, the others at ~0.1 km.)

**Power scaling** ([`total_ted_scaled`]): `x P / 3400 MWt`, the #450
convention (dose ∝ released activity ∝ thermal power) that
[`crate::lwr_comparison::pwr_inventory_scaled`] applies to the NuScale
inventory. That helper scales an inventory table, not a dose, so the same
linear ratio is applied here directly ([`scale_to_power`]).

Distances are km and doses Sv as plain `f64`, following the sibling
[`crate::lwr_comparison`] (`x_m`, `total_sv`); the suffix carries the unit.

> **Research, education and V&V only** (`RESPONSIBLE_USE.md`). Not a dose
> assessment for AP1000, Haiyang, HTR-10 or any plant.

## V&V: digitisation gate against Table 3

**Methodology.** Table 3 of the paper prints each group's maximum TED, all
at 0.6 km: noble gases 1.5, halogens 27, alkali metals 4.1, Te 0.443,
Ba/Sr 0.393, noble metals 0.072, Ce 2.2, La 9.51e-3 Sv (sum 35.7 Sv). The
gate compares each group's **largest digitised point** with its Table 3
value, and the sum of those eight peaks with 35.7 Sv, as
`|log10(digitised / table)|`.

**Tolerance, derived before the comparison was run:** one pixel of the
digitiser's y calibration. The y axis spans 1e-16 .. 1e3 Sv (19 decades)
over 233.489 - 27.670 = 205.819 px, i.e. **0.09231 decades/px** (one px =
a factor 1.237). A hand-placed marker on a curve cannot be placed more
finely than the pixel it lands on, so a reading off this axis is uncertain
by about ±1 px; a deviation larger than that is a placement error rather
than quantisation. The total's log deviation is bounded by the largest
per-group one (a weighted mean of ratios), so the same tolerance applies.
[`GATE_TOLERANCE_DECADES`].

**Results (2026-10-01, digitisation of 2026-10-01T01:53:51Z):**

| Group | Digitised peak \[Sv\] at \[km\] | Table 3 \[Sv\] | Deviation | In px |
|---|---|---|---|---|
| noble gases | 1.547 at 0.626 | 1.5 | +3.1 % | +0.14 |
| halogens | 31.08 at 0.592 | 27 | +15.1 % | +0.66 |
| alkali metals | 3.741 at 0.496 | 4.1 | −8.8 % | −0.43 |
| Te | 0.4344 at 0.593 | 0.443 | −1.9 % | −0.09 |
| Ba/Sr | 0.3721 at 0.597 | 0.393 | −5.3 % | −0.26 |
| noble metals | 0.08086 at 0.510 | 0.072 | +12.3 % | +0.55 |
| Ce | 2.107 at 0.598 | 2.2 | −4.2 % | −0.20 |
| La | 0.01073 at 0.575 | 9.51e-3 | +12.9 % | +0.57 |
| **sum of peaks** | **39.38** | **35.7** | **+10.3 %** | **+0.46** |

**All eight groups and the total pass the ±1 px (±0.0923 decade, ×/÷1.237)
gate**; the worst is halogens at 0.66 px. **Interpretation:** the
digitisation reproduces the published peaks to within the axis's own
resolution, and no better -- the overlay carries a ~10–15 % reading
uncertainty near the peak, larger than any curve-shape detail it could be
used to argue from. The halogens (76 % of the total) dominate both the
total and its +10 % excess. The peak of the *summed, interpolated* curve
(≈ 39.1 Sv near 0.59 km, see the test) is slightly below the sum of
peaks, because the groups peak at different digitised x.

Pinned by `tests::digitised_peaks_match_table_3_within_one_pixel`.

```rust
pub mod ap1000_ted { /* ... */ }
```

### Types

#### Struct `GroupCurve`

One group's digitised curve.

```rust
pub struct GroupCurve {
    pub group: String,
    pub points: Vec<(f64, f64)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `group` | `String` | NUREG-1465 group name, as in the CSV. |
| `points` | `Vec<(f64, f64)>` | `(distance_km, ted_sv)`, in digitised (increasing-x) order. |

##### Implementations

###### Methods

- ```rust
  pub fn at(self: &Self, x_km: f64) -> Option<f64> { /* ... */ }
  ```
  The group's TED \[Sv\] at `x_km`, interpolated linearly in

- ```rust
  pub fn peak(self: &Self) -> (f64, f64) { /* ... */ }
  ```
  The largest digitised point `(distance_km, ted_sv)`.

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `TotalTed`

The summed TED at one distance.

```rust
pub struct TotalTed {
    pub distance_km: f64,
    pub total_sv: f64,
    pub groups_outside: Vec<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `distance_km` | `f64` | Downwind distance \[km\]. |
| `total_sv` | `f64` | Sum over the groups whose digitised range covers `distance_km` \[Sv\]. |
| `groups_outside` | `Vec<String>` | Groups outside their digitised range here, counted as **0** (not<br>extrapolated). |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

#### Function `group_curves`

The eight digitised group curves, in CSV order.

```rust
pub fn group_curves() -> Vec<GroupCurve> { /* ... */ }
```

#### Function `digitised_range_km`

The x range any group covers \[km\]: (smallest first point, largest last
point).

```rust
pub fn digitised_range_km() -> (f64, f64) { /* ... */ }
```

#### Function `total_ted`

The eight groups summed at each of `x_km` (the common grid), as published
(3400 MWt). See the module docs for the interpolation and the zero
outside each group's range.

```rust
pub fn total_ted(x_km: &[f64]) -> Vec<TotalTed> { /* ... */ }
```

#### Function `scale_to_power`

A dose \[Sv\] published for 3400 MWt, scaled to `thermal_power_mwth`
(#450: ∝ power).

```rust
pub fn scale_to_power(sv_at_3400_mwth: f64, thermal_power_mwth: f64) -> f64 { /* ... */ }
```

#### Function `total_ted_scaled`

[`total_ted`] scaled to `thermal_power_mwth` by [`scale_to_power`]
(10 MWt for HTR-10).

```rust
pub fn total_ted_scaled(x_km: &[f64], thermal_power_mwth: f64) -> Vec<TotalTed> { /* ... */ }
```

#### Function `log_grid_km`

A log-spaced grid of `n >= 2` points over `[lo_km, hi_km]`.

```rust
pub fn log_grid_km(lo_km: f64, hi_km: f64, n: usize) -> Vec<f64> { /* ... */ }
```

### Constants and Statics

#### Constant `AP1000_MWTH`

AP1000 thermal power the paper models \[MWt\].

```rust
pub const AP1000_MWTH: f64 = 3400.0;
```

#### Constant `CITATION`

The citation, for labels.

```rust
pub const CITATION: &str = "A. Dadda et al., Radiation Physics and Chemistry 218 (2024) 111542, \
     Fig. 7, April-August (class B, 4.44 m/s); maintainer's log-log digitisation, 2026-10-01";
```

#### Constant `MISMATCHES`

What the overlay is NOT, against `htgr_sim_v1`'s accumulated dose. Shown
wherever the curve is drawn.

```rust
pub const MISMATCHES: &str = "AP1000 BDB: unmitigated core melt, RG 1.183 releases, no \
     containment credit stated | 2 h release interval (vs the NRC 96 h reference) | 100 m stack \
     (vs the HTR-10 ground release) | TED includes resuspension (the HTR-10 side does not) | \
     stability class B at 4.44 m/s (vs the simulator's own met) | a different code (HotSpot 3.1) \
     | compare against the HTR-10 beyond-design-basis core burn (DLOFC + air ingress, KORA) | \
     research/education only";
```

#### Constant `TABLE_3_DISTANCE_KM`

Distance of Table 3's maxima \[km\].

```rust
pub const TABLE_3_DISTANCE_KM: f64 = 0.6;
```

#### Constant `TABLE_3_MAX_TED_SV`

Table 3: maximum TED per group \[Sv\], at [`TABLE_3_DISTANCE_KM`].

```rust
pub const TABLE_3_MAX_TED_SV: [(&str, f64); 8] = _;
```

#### Constant `TABLE_3_TOTAL_SV`

Table 3's sum \[Sv\], as printed.

```rust
pub const TABLE_3_TOTAL_SV: f64 = 35.7;
```

#### Constant `GATE_TOLERANCE_DECADES`

The digitisation gate's tolerance \[decades\]: one y pixel of the
calibration, `19 / (233.48883 - 27.67020)` (module docs: derived before
the comparison was run).

```rust
pub const GATE_TOLERANCE_DECADES: f64 = _;
```

## Module `bwr_mark1`

The plant: Fukushima Daiichi Unit 1 after the earthquake's scram.

A BWR/3 in a Mark I containment. This is the model the `dhoby-ghaut`
`fukushima_web` demo draws and steps; it moved here from that example on
2026-10-29 (gh:#1033, maintainer: "Physics libraries go to sembawang").
Where it stands against the BSAF Unit 1 bands: [`compare`].

**Transcendentals go through PETIR** (`petir::mathf::RealMath`: `r_exp`,
`r_ln`, `r_powf`, `r_powi`, `r_cos`; `sqrt` stays, being correctly rounded
by IEEE-754), as everywhere in this crate (gh:#985), so native and wasm
runs take one route. Converted 2026-10-29; the measured effect is in
`verification_and_validation/bwr_mark1/petir-conversion.md`.

Three parts, owned and advanced separately so they can run on three
threads (three Web Workers in the browser), as the CIET Educational
Simulator's browser demo runs its heat transfer and its two flow solves:

| part | owns | runs on |
|---|---|---|
| [`Plant`] (this module) | the reactor pressure vessel's water and steam, the isolation condensers, the relief valves, injection, decay heat, the operators' sequence | the plant worker, which also paces the run |
| [`core::Core`] | the fuel and cladding temperatures, oxidation, hydrogen, relocation | the core worker |
| [`containment::Containment`] | drywell, wetwell, pool, vent, reactor building | the containment worker |

Each step, [`Plant::begin`] reads the vessel at the start of the step and
hands each worker a **snapshot**; the core and the containment advance
from it; [`Plant::finish`] takes their results and closes the vessel's
mass and energy balance. The coupling is explicit across the step (each
part sees the others' start-of-step state), the same splitting CIET v2
uses for its flows. Because the snapshots and results cross as exact
`f64`s ([`wire`]), the three-worker step is bit-for-bit the one-worker
step ([`Plant::step_inline`]); `tests::three_workers_are_one_worker`
holds that.

**AI draft, not validated.** See [`params`] for where every number comes
from (many are placeholders) and each module's "Not in the model" list.
Research, education and V&V only: never for operation, licensing, safety
decisions or emergency response.

```rust
pub mod bwr_mark1 { /* ... */ }
```

### Modules

## Module `aerosol`

Sectional aerosol dynamics for one well-mixed volume: agglomeration and
gravitational settling (gh:#1047, first deliverable; not yet wired into
the plant).

**Kernels** are MELCOR's MAEROS kernels, from the description in its
manual (Vol. 2, SAND2023-10994 O, RN §2.4.2 and Appendix B, which takes
them from Powers, Sprung and Leigh; no source used):
- Brownian, Fuchs' interpolation (Eqs B-1 to B-12);
- gravitational, with the collision efficiency `1.5 d_j^2/(d_i+d_j)^2`
  (Eqs B-13 to B-15, 2-38);
- turbulent shear and inertial, combined in quadrature (B-16 to B-18);
- their sum (Eq 2-40), all with MELCOR's defaults: shape factors 1,
  sticking 1, slip 1.257, dissipation 0.001 m^2/s^3.

Two lines of the extracted manual text are garbled and are pinned here
by their limits instead of being trusted: Fuchs' factor `F` (B-4) is the
**inverse** of its bracket, the only reading that gives the continuum
limit `8kT/(3 mu)` for equal particles and the free-molecular limit
`pi (d_i+d_j)^2 v_ij / 4`; and the mean free path (B-11) is
`nu sqrt(1.89e-4 M_w / T)`, whose constant is `4 pi/(8 R 1000)` (about
0.064 um in air at 298 K, the manual's text quoting about 0.069 um).
The slip correction (B-3) is the form `changi::flexpart::aerosol::part0`
also uses, computed inline there, so not reusable as a function.

**Discretisation is not MAEROS's.** MAEROS uses Gelbard and Seinfeld's
sectional coefficients, which the manual describes but does not give.
This uses the open-literature semi-implicit, volume-conserving sectional
scheme of Jacobson, Turco, Jensen and Toon (1994, *Atmos. Environ.* 28,
1327), with representative volumes in a fixed ratio of at least 2 (the
analogue of MAEROS's constraint `m_(i+1)/m_i > 2`, Eq 2-37). As in that
scheme, ~~particles that outgrow the top section stay in it; MELCOR's
fallout of particles above the largest size is not built (stated)~~
particles that outgrow the top section fall out, as in MELCOR (RN
§2.4.1): the scheme sends them to a virtual section above the top, and
that volume is booked as fallout.

**Deposition** (RN §2.4.2.2) onto floors, walls and ceilings (a pool
surface counts as a floor): gravitational settling (floors positive,
ceilings negative), Brownian diffusion across a boundary layer (Eq
2-42), thermophoresis (Eq 2-43) and diffusiophoresis with the Stefan flow
(Eqs 2-45, 2-46), summed per surface as MELCOR's table says and floored
at zero, then applied as exact exponential decay at `sum A v / V`.
Turbulent deposition in pipes and bends is not built.

```rust
pub mod aerosol { /* ... */ }
```

### Types

#### Struct `Medium`

The gas and the aerosol's material, and MELCOR's model constants.

```rust
pub struct Medium {
    pub t_k: f64,
    pub p_pa: f64,
    pub molar_mass_g: f64,
    pub mu_pa_s: f64,
    pub rho_p: f64,
    pub gamma: f64,
    pub chi: f64,
    pub sticking: f64,
    pub slip: f64,
    pub dissipation: f64,
    pub k_ratio: f64,
    pub accommodation: f64,
    pub boundary_layer_m: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `t_k` | `f64` | Gas temperature, K, pressure, Pa, molar mass, g/mol, viscosity,<br>Pa s. |
| `p_pa` | `f64` |  |
| `molar_mass_g` | `f64` |  |
| `mu_pa_s` | `f64` |  |
| `rho_p` | `f64` | Particle material density, kg/m^3 (MELCOR's RHONOM default 1000). |
| `gamma` | `f64` | Agglomeration and dynamic shape factors, sticking coefficient,<br>slip coefficient, turbulent dissipation, m^2/s^3 (MELCOR defaults:<br>1, 1, 1, 1.257, 0.001). |
| `chi` | `f64` |  |
| `sticking` | `f64` |  |
| `slip` | `f64` |  |
| `dissipation` | `f64` |  |
| `k_ratio` | `f64` | Gas-to-particle conductivity ratio, thermal accommodation constant,<br>diffusion boundary-layer thickness, m (MELCOR's RN1_MS01 defaults:<br>0.05, 2.25, 1e-5). |
| `accommodation` | `f64` |  |
| `boundary_layer_m` | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn air(t_k: f64, p_pa: f64, mu_pa_s: f64) -> Self { /* ... */ }
  ```
  Air at `t_k` and `p_pa`, with MELCOR's default aerosol constants.

- ```rust
  pub fn rho_gas(self: &Self) -> f64 { /* ... */ }
  ```
  Gas density, kg/m^3 (Eq B-12).

- ```rust
  pub fn mean_free_path(self: &Self) -> f64 { /* ... */ }
  ```
  Mean free path, m (Eq B-11, read as `nu sqrt(1.89e-4 M/T)`).

- ```rust
  pub fn slip_correction(self: &Self, d: f64) -> f64 { /* ... */ }
  ```
  Slip correction of a particle of diameter `d` (Eq B-3).

- ```rust
  pub fn settling_velocity(self: &Self, d: f64) -> f64 { /* ... */ }
  ```
  Stokes terminal velocity with slip, m/s (Eq B-14, and the settling

- ```rust
  pub fn diffusion_velocity(self: &Self, d: f64) -> f64 { /* ... */ }
  ```
  Brownian diffusion deposition velocity, m/s (Eq 2-42).

- ```rust
  pub fn thermophoretic_velocity(self: &Self, d: f64, grad_t: f64, t_wall: f64) -> f64 { /* ... */ }
  ```
  Thermophoretic velocity toward a surface, m/s, for a gas

- ```rust
  pub fn brownian(self: &Self, di: f64, dj: f64) -> f64 { /* ... */ }
  ```
  Brownian kernel, m^3/s (Fuchs; Eqs B-1 to B-10).

- ```rust
  pub fn gravitational(self: &Self, di: f64, dj: f64) -> f64 { /* ... */ }
  ```
  Gravitational kernel, m^3/s (Eqs B-13 to B-15).

- ```rust
  pub fn turbulent(self: &Self, di: f64, dj: f64) -> f64 { /* ... */ }
  ```
  Turbulent kernel, shear and inertial in quadrature, m^3/s (Eqs B-16

- ```rust
  pub fn kernel(self: &Self, di: f64, dj: f64) -> f64 { /* ... */ }
  ```
  The total kernel (Eq 2-40).

- ```rust
  pub fn kernel_table(self: &Self, d: &[f64]) -> Vec<Vec<f64>> { /* ... */ }
  ```
  The total kernel for every pair of the diameters `d`, m^3/s: the

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Enum `Facing`

How a deposition surface faces (RN §2.4.2.2).

```rust
pub enum Facing {
    Floor,
    Wall,
    Ceiling,
}
```

##### Variants

###### `Floor`

Upward-facing: floors and pool surfaces.

###### `Wall`

###### `Ceiling`

Downward-facing.

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Surface`

A deposition surface: area, m^2, facing, and the phoretic velocities it
drives, m/s, given per section by the caller (thermophoresis from its
heat flux, diffusiophoresis from its condensation; zero for none).

```rust
pub struct Surface {
    pub area: f64,
    pub facing: Facing,
    pub phoretic: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `area` | `f64` |  |
| `facing` | `Facing` |  |
| `phoretic` | `Vec<f64>` |  |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Sections`

A sectional size distribution in one well-mixed volume: representative
particle volumes `v_k` (fixed ratio), and the aerosol volume
concentration in each, m^3 of particle per m^3 of gas.

```rust
pub struct Sections {
    pub v: Vec<f64>,
    pub conc: Vec<f64>,
    pub fallout: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `v` | `Vec<f64>` |  |
| `conc` | `Vec<f64>` |  |
| `fallout` | `f64` | Particle volume that outgrew the top section, m^3 per m^3 of gas,<br>accumulated. |

##### Implementations

###### Methods

- ```rust
  pub fn new(d_min: f64, n: usize, ratio: f64) -> Self { /* ... */ }
  ```
  `n` sections from diameter `d_min` with particle-volume ratio

- ```rust
  pub fn diameter(self: &Self, k: usize) -> f64 { /* ... */ }
  ```

- ```rust
  pub fn number(self: &Self, k: usize) -> f64 { /* ... */ }
  ```
  Number concentration in section `k`, 1/m^3.

- ```rust
  pub fn coagulate<F: Fn(usize, usize) -> f64>(self: &mut Self, dt: f64, beta: F) { /* ... */ }
  ```
  One semi-implicit coagulation step of `dt` s with the kernel

- ```rust
  pub fn coagulate_with<F: Fn(usize, usize) -> f64>(self: &mut Self, dt: f64, beta: F, tracers: &mut [Vec<f64>]) -> Vec<f64> { /* ... */ }
  ```
  [`Self::coagulate`], carrying `tracers` with the particles: each is

- ```rust
  pub fn deposit(self: &mut Self, medium: &Medium, surfaces: &[Surface], volume: f64, dt: f64) -> Vec<f64> { /* ... */ }
  ```
  Deposition for `dt` s onto `surfaces` of a volume of `volume` m^3:

- ```rust
  pub fn deposit_with(self: &mut Self, medium: &Medium, surfaces: &[Surface], volume: f64, dt: f64, tracers: &mut [Vec<f64>]) -> (Vec<f64>, Vec<Vec<f64>>) { /* ... */ }
  ```
  [`Self::deposit`], carrying per-section `tracers` with the

- ```rust
  pub fn settle(self: &mut Self, medium: &Medium, area: f64, volume: f64, dt: f64) -> f64 { /* ... */ }
  ```
  Gravitational settling for `dt` s onto a floor of `area` m^2 under a

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

#### Function `diffusiophoretic_velocity`

Diffusiophoretic velocity toward a surface, m/s, with the Stefan flow
(Eqs 2-45, 2-46): `w_cond` the steam condensation flux onto it,
kg/(m^2 s) (negative evaporating), `x_steam` the bulk steam mole
fraction, `m_steam`, `m_nc` molar masses (any one unit), `rho_bulk` the
bulk gas density and `rho_sat` the saturated vapour density, kg/m^3.

```rust
pub fn diffusiophoretic_velocity(w_cond: f64, x_steam: f64, m_steam: f64, m_nc: f64, rho_bulk: f64, rho_sat: f64) -> f64 { /* ... */ }
```

## Module `cavity`

Molten core-concrete interaction on the drywell floor, a reduced form of
MELCOR's CAV package (CORCON-Mod3; Vol. 2, CAV RM §3; 2026-10-29,
`Mechanisms::mcci`, plan item F8). AI draft, clean room: written from the
manuals' description, never from MELCOR or CORCON source.

- **One debris layer**, lumped: UO2, metallic Zr and oxides (ZrO2 and the
  ablated concrete's oxides) at one temperature, heated by its decay
  power and by the oxidation of its Zr. No phase change, no layering
  (MELCOR stratifies and mixes layers, §3.2), no crust (stated).
- **Downward**, MELCOR's quasi-steady ablation (§3.3, 3.4): while the
  debris is above the concrete's ablation temperature, the heat to the
  concrete is `h A (T - T_a)` and it ablates `q / h_a` kg/s, the preheated
  region neglected, as MELCOR neglects it. `h` is \[A\]: CORCON's gas-film
  and bubbling correlations are not held.
- **Upward**, with no water above (§3.3: "radiation and convection in the
  absence of overlying water"): from the debris's interior to its top
  surface by the same `h`, and from the surface by gray radiation to the
  drywell's structures at MELCOR's default emissivities; the surface
  temperature from the continuity of the two fluxes.
- **Chemistry** (§3.5): the concrete's H2O and CO2 pass through the debris
  and oxidise its metallic Zr to H2 and CO, completely while Zr lasts
  (equilibrium favours it; MELCOR's default suppresses coking, so no
  condensed carbon), the reaction heat staying in the debris; the
  concrete's other constituents join the oxides. Reduction of the
  concrete's oxides by Zr is not modelled. The gases leave at the
  ablation temperature (stated: MELCOR equilibrates them with the layers
  they pass).

- **Under water** (§3.3: "a complete pool boiling curve"): the top
  surface in film boiling, Berenson's horizontal-surface correlation
  (J. Heat Transfer 83, 1961) with the vapour's properties at the film
  temperature, plus radiation across the vapour film to the water at
  the debris's emissivity; the surface temperature again from the
  continuity of the fluxes. The nucleate and transition regimes are not
  separated (stated: the debris surface here stays far above
  saturation). The concrete ablates beneath, as CORCON's does.

```rust
pub mod cavity { /* ... */ }
```

### Types

#### Struct `Cavity`

The debris on the drywell floor.

```rust
pub struct Cavity {
    pub uo2_kg: f64,
    pub zr_kg: f64,
    pub oxide_kg: f64,
    pub t_k: f64,
    pub seen_uo2_kg: f64,
    pub seen_zr_kg: f64,
    pub ablated_kg: f64,
    pub h2_kg: f64,
    pub co_kg: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `uo2_kg` | `f64` | UO2, metallic Zr and oxides, kg, and the debris temperature, K. |
| `zr_kg` | `f64` |  |
| `oxide_kg` | `f64` |  |
| `t_k` | `f64` |  |
| `seen_uo2_kg` | `f64` | The core's debris inventory last seen (what has arrived), kg. |
| `seen_zr_kg` | `f64` |  |
| `ablated_kg` | `f64` | Concrete ablated, kg, and the H2 and CO made, kg. |
| `h2_kg` | `f64` |  |
| `co_kg` | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn arrive(self: &mut Self, prm: &Params, uo2_kg: f64, zr_kg: f64, t_k: f64) { /* ... */ }
  ```
  Debris arriving: the core's inventory now is `uo2_kg` and `zr_kg`

- ```rust
  pub fn step(self: &mut Self, prm: &Params, q_w: f64, t_wall_k: f64, water: Option<(f64, f64)>, dt: f64) -> CavityFlows { /* ... */ }
  ```
  One step of `dt` s with decay power `q_w` and the drywell's

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `CavityFlows`

What one step of the cavity sends to the drywell.

```rust
pub struct CavityFlows {
    pub q_up_w: f64,
    pub steam_kg: f64,
    pub h2_mol: f64,
    pub co_mol: f64,
    pub co2_mol: f64,
    pub gas_k: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `q_up_w` | `f64` | Radiated up to the drywell's structures, W. |
| `steam_kg` | `f64` | Gases released, at `gas_k`: steam kg, H2, CO and CO2 mol. |
| `h2_mol` | `f64` |  |
| `co_mol` | `f64` |  |
| `co2_mol` | `f64` |  |
| `gas_k` | `f64` |  |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
## Module `compare`

The Unit 1 comparison against BSAF Phase II, and the ablation table
(gh:#1032, plan `docs/fukushima-melcor-gap-plan.md` item F1).

Runs the recorded sequence headless, measures the quantities that have a
BSAF counterpart, and sets each beside its band **and the band's tier**,
from `crates/kovan-literature/derived/nea7525-bsaf-fukushima-daiichi-bands.md`
(§1 and §6). The tier says what a comparison can mean:

- **A** measured: missing it means the model is wrong.
- **B** well-constrained code consensus: a real check.
- **C** wide code spread, read off a figure: context only. Being inside
  proves little.
- **D** known to be wrong: never a target (none is used here).

**The bands are checks, never inputs.** Nothing in the model is tuned
toward them (root `CLAUDE.md`, "Get the PROCESS right first"). A miss is
a result to report, and the ablation rows say how much of it each
modelled mechanism carries.

Acknowledgement: OECD Nuclear Energy Agency, *Benchmark Study of the
Accident at the Fukushima Daiichi Nuclear Power Plant*, NEA No. 7525,
© OECD 2021. Facts and numbers only, cited with acknowledgement.

```rust
pub mod compare { /* ... */ }
```

### Types

#### Enum `Tier`

What a band can mean (see the module docs).

```rust
pub enum Tier {
    A,
    B,
    C,
}
```

##### Variants

###### `A`

###### `B`

###### `C`

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Band`

One BSAF Phase II band for Unit 1.

```rust
pub struct Band {
    pub name: &'static str,
    pub low: f64,
    pub high: f64,
    pub unit: &'static str,
    pub tier: Tier,
    pub source: &'static str,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `&'static str` |  |
| `low` | `f64` |  |
| `high` | `f64` |  |
| `unit` | `&'static str` |  |
| `tier` | `Tier` |  |
| `source` | `&'static str` | Where it is in the derived record. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Metrics`

What the model gives for each band. `None` where the event never
happens in the run.

```rust
pub struct Metrics {
    pub lower_head_h: Option<f64>,
    pub depressurised_h: Option<f64>,
    pub h2_25h_kg: f64,
    pub p_dw_peak_mpa: f64,
    pub vent_h: Option<f64>,
    pub explosion_h: Option<f64>,
    pub fp_released_25h: f64,
    pub uo2_released_25h_kg: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `lower_head_h` | `Option<f64>` |  |
| `depressurised_h` | `Option<f64>` |  |
| `h2_25h_kg` | `f64` |  |
| `p_dw_peak_mpa` | `f64` |  |
| `vent_h` | `Option<f64>` |  |
| `explosion_h` | `Option<f64>` |  |
| `fp_released_25h` | `f64` | Volatile fission products (Xe, Cs, I) released from the fuel by<br>25 h, fraction of inventory (gh:#1040). Not a BSAF band: BSAF<br>reports only that most codes' releases reached an asymptote above<br>0.8 (code-to-code, figure-read; NEA No. 7525 §2, Fig. 2.5). |
| `uo2_released_25h_kg` | `f64` | UO2 released from the fuel by 25 h, kg: the seed aerosol (gh:#1047). |

##### Implementations

###### Methods

- ```rust
  pub fn values(self: &Self) -> [Option<f64>; 6] { /* ... */ }
  ```
  The values in the order of [`BANDS`].

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Elasticity`

How one input moves the metrics: elasticities `(dm/m) / (dp/p)` by
central difference, the input scaled by `1 - rel` and `1 + rel` about
its default with every mechanism on.

**A ranking instrument, not an uncertainty analysis.** It is local (a
derivative at the defaults), and a relative step is not an input's real
uncertainty: 1 % of an activation energy is a large change, 1 % of a
valve area a small one. It says which unverified inputs the answer leans
on hardest, i.e. which to check first against the sources (plan item
F13). It is never used to choose a value (plan rule 1).

```rust
pub struct Elasticity {
    pub name: &'static str,
    pub tag: super::params::Tag,
    pub default: f64,
    pub lower_head: Option<f64>,
    pub h2: f64,
    pub p_dw: f64,
    pub event_changes: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `&'static str` |  |
| `tag` | `super::params::Tag` |  |
| `default` | `f64` |  |
| `lower_head` | `Option<f64>` | `None` when the lower head does not fail in all three runs. |
| `h2` | `f64` |  |
| `p_dw` | `f64` |  |
| `event_changes` | `bool` | Whether the vent or the explosion happens differs between a perturbed<br>run and the default one (~~appears; neither happened at the<br>defaults~~: since gh:#1036 the vent does). |

##### Implementations

###### Methods

- ```rust
  pub fn largest(self: &Self) -> f64 { /* ... */ }
  ```
  The largest of the three magnitudes, for ranking.

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Histories`

The model's histories of the quantities BSAF plots for Unit 1, sampled
every `every_s` seconds to `end_s`: time h, RPV pressure MPa, drywell
pressure MPa, in-vessel H2 kg, volatile fission products released from
fuel (fraction). For comparison with the digitised figures.

```rust
pub struct Histories {
    pub t_h: Vec<f64>,
    pub p_rpv_mpa: Vec<f64>,
    pub p_dw_mpa: Vec<f64>,
    pub h2_kg: Vec<f64>,
    pub fp_released: Vec<f64>,
    pub cs_dw_structures: Vec<f64>,
    pub cs_sc_water: Vec<f64>,
    pub i_dw_structures: Vec<f64>,
    pub i_sc_water: Vec<f64>,
    pub cs_released: Vec<f64>,
    pub i_released: Vec<f64>,
    pub xe_released: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `t_h` | `Vec<f64>` |  |
| `p_rpv_mpa` | `Vec<f64>` |  |
| `p_dw_mpa` | `Vec<f64>` |  |
| `h2_kg` | `Vec<f64>` |  |
| `fp_released` | `Vec<f64>` |  |
| `cs_dw_structures` | `Vec<f64>` | Cesium on the drywell's structures and in the wetwell pool,<br>fractions of the inventory (gh:#1047). |
| `cs_sc_water` | `Vec<f64>` |  |
| `i_dw_structures` | `Vec<f64>` | Iodine likewise. |
| `i_sc_water` | `Vec<f64>` |  |
| `cs_released` | `Vec<f64>` | Cesium released to the environment (vented to the stack and leaked<br>to the reactor building), fraction of the inventory. |
| `i_released` | `Vec<f64>` | Iodine likewise, and the noble gas (fractions of the core<br>inventory). |
| `xe_released` | `Vec<f64>` |  |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `AgainstMeasured`

The model against one measured series (the instrument fixed before any
digitised curve was seen, 2026-10-29): the model interpolated at each
reference time inside the run, then the mean and the largest absolute
difference, in the series' own units, and how many points counted.

```rust
pub struct AgainstMeasured {
    pub n: usize,
    pub mean_abs: f64,
    pub max_abs: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `n` | `usize` |  |
| `mean_abs` | `f64` |  |
| `max_abs` | `f64` |  |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

#### Function `measure`

Run the recorded sequence with `mechanisms` and measure every band's
quantity. Deterministic, like the headless mode.

```rust
pub fn measure(mechanisms: super::Mechanisms) -> Metrics { /* ... */ }
```

#### Function `measure_with`

[`measure`] with the plant's inputs set to `prm` (gh:#1033).

```rust
pub fn measure_with(mechanisms: super::Mechanisms, prm: super::params::Params) -> Metrics { /* ... */ }
```

#### Function `ablations`

The ablation runs: the baseline, then each mechanism off alone.

```rust
pub fn ablations() -> Vec<(&'static str, super::Mechanisms)> { /* ... */ }
```

#### Function `elasticity`

The [`Elasticity`] of the input `name` (from [`Params::INPUTS`]) for a
relative step `rel`, given the default run `base`. `None` for an unknown
name. Runs the sequence twice.

```rust
pub fn elasticity(name: &'static str, rel: f64, base: &Metrics) -> Option<Elasticity> { /* ... */ }
```

#### Function `sensitivity_report`

The elasticity table as Markdown, largest effect first.

```rust
pub fn sensitivity_report(rows: &[Elasticity]) -> String { /* ... */ }
```

#### Function `report`

The comparison and ablation tables, as Markdown.

```rust
pub fn report(runs: &[(&str, Metrics)]) -> String { /* ... */ }
```

#### Function `histories`

```rust
pub fn histories(mechanisms: super::Mechanisms, prm: super::params::Params, every_s: f64, end_s: f64) -> Histories { /* ... */ }
```

#### Function `against_measured`

```rust
pub fn against_measured(t_model: &[f64], v_model: &[f64], reference: &[(f64, f64)]) -> AgainstMeasured { /* ... */ }
```

#### Function `inside_envelope`

The model against a family of code predictions (no measurement): at
each time of `grid_h`, whether the model lies within the codes'
envelope (each code interpolated; only codes that cover the time
count, and at least two must). Returns the fraction of counted times
inside, and how many counted.

```rust
pub fn inside_envelope(t_model: &[f64], v_model: &[f64], codes: &[Vec<(f64, f64)>], grid_h: &[f64]) -> (f64, usize) { /* ... */ }
```

### Constants and Statics

#### Constant `BANDS`

The Unit 1 bands, in the order of [`Metrics::values`].

```rust
pub const BANDS: [Band; 6] = _;
```

#### Constant `COMPARE_END_S`

How long the comparison runs, s: past the recorded explosion.

```rust
pub const COMPARE_END_S: f64 = _;
```

## Module `containment`

The Mark I containment and the reactor building. This is what the
**containment worker** runs.

## Model (AI draft; read the limitations)

- **Drywell**: nitrogen, hydrogen and steam at one temperature, ideal
  gases (steam's energy referenced to IF97's saturated vapour at
  100 degC with a constant `c_v`; N2 and H2 with MELCOR's `c_v(T)`
  fits, [`super::gas`], since 2026-10-29); a lumped wall heat sink (steel and
  concrete) takes heat by convection, **implicitly**; steam above its
  saturation pressure at the gas temperature condenses to the sump.
  **Since 2026-10-29** (`Mechanisms::dw_wall_hs`) the wall's coefficient
  is MELCOR's natural convection and steam condenses on it by diffusion
  through the non-condensables, its latent heat into the wall
  ([`super::pool_surface::structure_surface`]); the fixed coefficient is
  the ablation. The wall's area and heat capacity are still \[A\].
- **Wetwell (torus)**: the pool, one temperature; ~~the gas space above it
  at the pool temperature, saturated with steam (an equilibrium
  assumption)~~ **since 2026-10-29** (`Mechanisms::ww_atmosphere`) the gas
  space has its own temperature and steam, exchanging mass and energy
  with the pool at its surface and with the bubbles leaving it
  ([`super::pool_surface`]: MELCOR CVH/FL RM §5.1 and §6.1); the old
  equilibrium is the ablation. ~~The torus shell is not a heat structure
  (stated).~~ The torus shell above the pool is a heat structure on it
  since 2026-10-29 (`Mechanisms::ww_shell`, its geometry \[A\], its outer
  face adiabatic). Cost: the 30 h headless run 7.8 -> 11.1 s.
- **Downcomers**: drywell gas pushed into the pool once the drywell
  exceeds the wetwell by the submergence head; the steam condenses in the
  pool, the nitrogen and hydrogen bubble through into the wetwell gas at
  the pool's temperature. (With the wetwell atmosphere, by MELCOR's bubble model: below
  about 5 K of subcooling part of the steam passes uncondensed.) **Vacuum breakers** the other way.
- **Relief-valve discharge** goes into the pool (quenchers), its hydrogen
  into the wetwell gas.
- **Wetwell vent** to the stack through a rupture disk, when the operators
  open it; **drywell head-flange leak** to the reactor building at high
  pressure; the **reactor building** is well mixed at atmospheric pressure
  and its hydrogen fraction is compared with the flammability limit and a
  strong-deflagration threshold.

The downcomers, the vacuum breakers and the leaks are stiff at a 1 s
step: when an orifice could pass more than equalises the two volumes in a
step, the transfer is the equalising amount (the implicit answer of a
stiff path), never more. **Not in the model**: drywell spray, wetwell
stratification, pool level and swell, pool scrubbing of aerosols (no
fission products are tracked yet), the torus room, the reactor building's
own heat-up and ventilation, steam in the building, and the combustion
itself (the explosion is a threshold, not a calculation).

```rust
pub mod containment { /* ... */ }
```

### Types

#### Struct `ContainmentSnapshot`

```rust
pub struct ContainmentSnapshot {
    pub dt_s: f64,
    pub srv_steam_kg_s: f64,
    pub srv_h: f64,
    pub srv_h2_kg_s: f64,
    pub leak_steam_kg_s: f64,
    pub leak_h: f64,
    pub leak_h2_kg_s: f64,
    pub leak_gas_k: f64,
    pub vent_ordered: bool,
    pub ex_vessel_w: f64,
    pub pedestal_inflow_kg_s: f64,
    pub vessel_drain_kg_s: f64,
    pub vessel_drain_k: f64,
    pub gas_cv_t: bool,
    pub xe_relief: f64,
    pub xe_leak: f64,
    pub cs_relief_kg: f64,
    pub cs_leak_kg: f64,
    pub uo2_relief_kg: f64,
    pub uo2_leak_kg: f64,
    pub i_relief_kg: f64,
    pub i_leak_kg: f64,
    pub pool_scrubbing: bool,
    pub scrubbing_saturation: bool,
    pub swarm_scrubbing: bool,
    pub sectional_classes: bool,
    pub iodine_partition: bool,
    pub mixture_vent: bool,
    pub phoresis: bool,
    pub iodine_swarm: bool,
    pub rn_relief: super::rn_volume::RnFlow,
    pub rn_leak: super::rn_volume::RnFlow,
    pub condensate_mass: bool,
    pub q_noble_w: f64,
    pub q_alkali_w: f64,
    pub q_halogen_w: f64,
    pub q_chalcogen_w: f64,
    pub ww_atmosphere: bool,
    pub dw_wall_hs: bool,
    pub ww_shell: bool,
    pub outflow_enthalpy: bool,
    pub relief_h2_enthalpy: bool,
    pub srv_gas_k: f64,
    pub condensate_to_floor: bool,
    pub rpv_wall_w: f64,
    pub mcci: bool,
    pub dw_wall_outer: bool,
    pub cav_uo2_kg: f64,
    pub cav_zr_kg: f64,
    pub cav_debris_k: f64,
    pub prm: Params,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `dt_s` | `f64` |  |
| `srv_steam_kg_s` | `f64` | Relief-valve discharge into the pool: steam kg/s, its enthalpy J/kg,<br>hydrogen kg/s. |
| `srv_h` | `f64` |  |
| `srv_h2_kg_s` | `f64` |  |
| `leak_steam_kg_s` | `f64` | RPV leak (and, after lower-head failure, the breach) into the drywell:<br>steam kg/s, enthalpy J/kg, hydrogen kg/s, gas temperature K. |
| `leak_h` | `f64` |  |
| `leak_h2_kg_s` | `f64` |  |
| `leak_gas_k` | `f64` |  |
| `vent_ordered` | `bool` | Operators' order to open the wetwell vent (the disk must still burst). |
| `ex_vessel_w` | `f64` | After the lower head fails: the debris's decay power on the drywell<br>floor, W, and the injected water that now pours through, kg/s (at<br>the injection temperature). |
| `pedestal_inflow_kg_s` | `f64` |  |
| `vessel_drain_kg_s` | `f64` | Water draining from a failed vessel into the pedestal, kg/s, and its<br>temperature, K (`Mechanisms::failed_vessel_drain`). |
| `vessel_drain_k` | `f64` |  |
| `gas_cv_t` | `bool` | MELCOR's `c_v(T)` for N2 and H2 ([`super::gas`]); false is the<br>constant `c_v` ablation. |
| `xe_relief` | `f64` | Noble gas (Xe) arriving this step through the relief valves (into<br>the wetwell gas) and through the leaks (into the drywell), as<br>fractions of the core inventory (gh:#1040). |
| `xe_leak` | `f64` |  |
| `cs_relief_kg` | `f64` | Cesium (as vapour) and UO2 aerosol arriving this step through the<br>relief valves and the leaks, kg (gh:#1047). |
| `cs_leak_kg` | `f64` |  |
| `uo2_relief_kg` | `f64` |  |
| `uo2_leak_kg` | `f64` |  |
| `i_relief_kg` | `f64` | Iodine (as I2 vapour) likewise, kg. |
| `i_leak_kg` | `f64` |  |
| `pool_scrubbing` | `bool` | Pool scrubbing by steam condensation at the vent exits (SPARC-90's<br>DF_EC; `Mechanisms::pool_scrubbing`). |
| `scrubbing_saturation` | `bool` | MELCOR's `X_o` in that scrubbing, and the impaction and swarm-rise<br>factors past the downcomers (`Mechanisms::scrubbing_saturation`,<br>`Mechanisms::swarm_scrubbing`). |
| `swarm_scrubbing` | `bool` |  |
| `sectional_classes` | `bool` | Fission-product classes on the aerosol tracked by section<br>(`Mechanisms::sectional_classes`). |
| `iodine_partition` | `bool` | The iodine partition limit on the condensation scrubbing<br>(`Mechanisms::iodine_partition`). |
| `mixture_vent` | `bool` | The wetwell vent passes the gas mixture (`Mechanisms::mixture_vent`). |
| `phoresis` | `bool` | Thermophoresis and diffusiophoresis onto the drywell's structures<br>(`Mechanisms::phoresis`). |
| `iodine_swarm` | `bool` | The swarm rise's scrubbing of I2 vapour (`Mechanisms::iodine_swarm`). |
| `rn_relief` | `super::rn_volume::RnFlow` | The vessel's aerosol and fission products leaving this step through<br>the relief valves and the leaks, by section (`Mechanisms::<br>vessel_aerosol`); empty, the scalar `*_relief_kg` and `*_leak_kg`<br>above carry them instead. |
| `rn_leak` | `super::rn_volume::RnFlow` |  |
| `condensate_mass` | `bool` | Condensed classes add to their aerosol section's mass<br>(`Mechanisms::condensate_mass`). |
| `q_noble_w` | `f64` | Decay heat of the noble-gas, alkali and halogen classes for the<br>whole core inventory, W (`fp_decay`; zero without<br>`Mechanisms::fp_decay_follows`): each location takes its share of the<br>class's inventory. |
| `q_alkali_w` | `f64` |  |
| `q_halogen_w` | `f64` |  |
| `q_chalcogen_w` | `f64` | And the chalcogens' (Te/Se), when tellurium is transported<br>(`Mechanisms::te_transport`). |
| `ww_atmosphere` | `bool` | The wetwell atmosphere has its own temperature and steam, and<br>exchanges mass and energy with the pool at its surface and with the<br>bubbles leaving the pool (`Mechanisms::ww_atmosphere`). |
| `dw_wall_hs` | `bool` | The drywell structures as MELCOR's heat-structure surface: natural<br>convection and diffusion-limited condensation (`Mechanisms::<br>dw_wall_hs`); false is the fixed coefficient `dw_wall_h`. |
| `ww_shell` | `bool` | The torus shell above the pool as a heat structure on the wetwell<br>atmosphere (`Mechanisms::ww_shell`; needs `ww_atmosphere`). |
| `outflow_enthalpy` | `bool` | Gas leaving the drywell (downcomers, flange leak) takes its enthalpy<br>(`Mechanisms::outflow_enthalpy`); false is its internal energy, the<br>drywell keeping its temperature. |
| `relief_h2_enthalpy` | `bool` | The relief valves' hydrogen arrives with its enthalpy at<br>`srv_gas_k`, the vessel's saturation temperature, as the vessel gave<br>it up (`Mechanisms::relief_h2_enthalpy`; needs `ww_atmosphere`);<br>false is at the pool's temperature. |
| `srv_gas_k` | `f64` |  |
| `condensate_to_floor` | `bool` | Water condensed in the drywell drains to its floor, the pedestal's<br>pool (`Mechanisms::condensate_to_floor`); false is it leaving the<br>model. |
| `rpv_wall_w` | `f64` | Heat through the RPV's insulation into the drywell gas, W<br>(`Mechanisms::rpv_wall`). |
| `mcci` | `bool` | Molten core-concrete interaction on the drywell floor<br>(`Mechanisms::mcci`), and the debris there: the core's UO2 and<br>metallic Zr, kg, and their temperature, K. |
| `dw_wall_outer` | `bool` | The drywell structures lose heat across the gap to the concrete<br>around them, which conducts it to the reactor building<br>(`Mechanisms::dw_wall_outer`). |
| `cav_uo2_kg` | `f64` |  |
| `cav_zr_kg` | `f64` |  |
| `cav_debris_k` | `f64` |  |
| `prm` | `Params` | The plant's inputs (gh:#1033). |

##### Implementations

###### Methods

- ```rust
  pub fn write(self: &Self, w: &mut Writer) { /* ... */ }
  ```

- ```rust
  pub fn read(r: &mut Reader<''_>) -> Self { /* ... */ }
  ```

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `ContainmentResult`

```rust
pub struct ContainmentResult {
    pub p_dw_pa: f64,
    pub p_ww_pa: f64,
    pub t_dw_k: f64,
    pub t_pool_k: f64,
    pub dw_h2_kg: f64,
    pub ww_h2_kg: f64,
    pub rb_h2_fraction: f64,
    pub vent_flowing: bool,
    pub flange_leaking: bool,
    pub rb_flammable: bool,
    pub rb_exploded: bool,
    pub vented_h2_kg: f64,
    pub vented_kg_s: f64,
    pub xe_dw: f64,
    pub xe_ww: f64,
    pub xe_vented: f64,
    pub xe_building: f64,
    pub cs_dw_structures_kg: f64,
    pub cs_ww_pool_kg: f64,
    pub cs_airborne_kg: f64,
    pub cs_vented_kg: f64,
    pub cs_building_kg: f64,
    pub i_dw_structures_kg: f64,
    pub i_ww_pool_kg: f64,
    pub i_airborne_kg: f64,
    pub i_vented_kg: f64,
    pub i_building_kg: f64,
    pub te_kg: f64,
    pub cav_ablated_kg: f64,
    pub cav_h2_kg: f64,
    pub cav_co_kg: f64,
    pub cav_debris_k: f64,
    pub cav_zr_kg: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `p_dw_pa` | `f64` |  |
| `p_ww_pa` | `f64` |  |
| `t_dw_k` | `f64` |  |
| `t_pool_k` | `f64` |  |
| `dw_h2_kg` | `f64` |  |
| `ww_h2_kg` | `f64` |  |
| `rb_h2_fraction` | `f64` |  |
| `vent_flowing` | `bool` |  |
| `flange_leaking` | `bool` |  |
| `rb_flammable` | `bool` |  |
| `rb_exploded` | `bool` |  |
| `vented_h2_kg` | `f64` |  |
| `vented_kg_s` | `f64` |  |
| `xe_dw` | `f64` | Noble gas (Xe) in the drywell and the wetwell gas, vented to the<br>stack and leaked to the reactor building, fractions of the core<br>inventory (gh:#1040). |
| `xe_ww` | `f64` |  |
| `xe_vented` | `f64` |  |
| `xe_building` | `f64` |  |
| `cs_dw_structures_kg` | `f64` | Cesium (gh:#1047), kg: on the drywell's structures, in the wetwell<br>pool (deposited on its surface), airborne in both, vented, and<br>leaked to the building. |
| `cs_ww_pool_kg` | `f64` |  |
| `cs_airborne_kg` | `f64` |  |
| `cs_vented_kg` | `f64` |  |
| `cs_building_kg` | `f64` |  |
| `i_dw_structures_kg` | `f64` | Iodine likewise, kg. |
| `i_ww_pool_kg` | `f64` |  |
| `i_airborne_kg` | `f64` |  |
| `i_vented_kg` | `f64` |  |
| `i_building_kg` | `f64` |  |
| `te_kg` | `f64` | All the tellurium that has reached the containment, kg (in it,<br>vented and leaked): for its conservation check. |
| `cav_ablated_kg` | `f64` | The cavity (`mcci`): concrete ablated, kg; H2 and CO made there, kg;<br>the debris temperature, K, and its metallic Zr, kg. |
| `cav_h2_kg` | `f64` |  |
| `cav_co_kg` | `f64` |  |
| `cav_debris_k` | `f64` |  |
| `cav_zr_kg` | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn write(self: &Self, w: &mut Writer) { /* ... */ }
  ```

- ```rust
  pub fn read(r: &mut Reader<''_>) -> Self { /* ... */ }
  ```

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Containment`

```rust
pub struct Containment {
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
  pub fn new(prm: &Params) -> Self { /* ... */ }
  ```

- ```rust
  pub fn p_dw(self: &Self, prm: &Params) -> f64 { /* ... */ }
  ```

- ```rust
  pub fn p_ww(self: &Self, prm: &Params) -> f64 { /* ... */ }
  ```

- ```rust
  pub fn step(self: &mut Self, s: &ContainmentSnapshot) -> ContainmentResult { /* ... */ }
  ```
  Advance one step. Pure function of `self` and `s`.

- ```rust
  pub fn write_state(self: &Self, w: &mut Writer) { /* ... */ }
  ```
  The whole state, for a stateless worker to step.

- ```rust
  pub fn read_state(r: &mut Reader<''_>) -> Self { /* ... */ }
  ```

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    The containment with the default inputs ([`Params::default`]).

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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
## Module `core`

The core: heat-up of the fuel once the water level falls, zirconium-steam
oxidation, hydrogen, and relocation. This is what the **core worker**
runs (in the browser a Web Worker of its own, as CIET's flow solves are).

## Model (AI draft; read the limitations)

Three equal-area rings, centre outwards, times [`AXIAL_NODES`] axial
nodes over the active length. Each node is two lumps: the UO2 and the
Zircaloy (cladding plus its share of the channel boxes), with
- the decay power shared by ring factor and a chopped-cosine axial shape;
- fuel-to-clad conductance through the pellet and the gap;
- clad-to-coolant: **covered** (below the collapsed level), nucleate
  boiling to water at `T_sat`; **uncovered**, convection to the steam
  rising from the boil-off below, Dittus-Boelter with IF97 steam
  properties at the film temperature, applied in effectiveness form
  (`q = W c_p (T_clad - T_in)(1 - e^(-NTU))`), which cannot heat the steam
  past the cladding: the second law holds by construction, no guard;
- both lumps advanced **implicitly** together (a 2x2 solve per node), so
  any time step is stable;
- **temperature-dependent heat capacities** (gh:#1038): OFFBEAT's MATPRO
  UO2 and MATPRO Zircaloy correlations, reused, not re-entered. The 2x2
  solve takes the heat capacities at the start of the step; each lump's
  energy is then advanced by the heat that solve moved, and its
  temperature found by inverting the correlation's enthalpy (which since
  gh:#1035 includes the heats of fusion, so a node pays 225 kJ per kg of
  Zr at 2098 K), so energy is
  conserved exactly. `Mechanisms::without_temperature_dependent_properties`
  is the ablation (the constant `UO2_CP`/`ZR_CP` path, unchanged);
- oxidation by the parabolic Urbanic-Heidrick law above ~~1000 K~~ 1100 K (MELCOR's TOXMIN), its heat
  into the cladding, limited by the steam that reaches the node (steam
  starvation) and, since 2026-10-29, by MELCOR's gaseous diffusion of
  that steam through the hydrogen (`diffusion_limited_rate`); the steam it uses and the hydrogen it makes travel up.
  The heat is Hess's law at the node (gh:#1046): the 298 K heat of
  reaction, plus the consumed steam's enthalpy above 298 K vapour, minus
  heating the new hydrogen to the stream, which then carries it (steam
  plus hydrogen heat capacity). A steam-starved node does not heat the
  hydrogen passing it;
- a node collapses by MELCOR's criteria (gh:#1035): once its metal is
  gone (oxidised below [`MIN_METAL_M`], or its shell breached at
  [`CLAD_BREACH_K`]) it accumulates time-at-temperature damage and falls
  at [`ROD_COLLAPSE_K`] or when the damage reaches 1; any rod falls at
  [`ROD_COLLAPSE_MAX_K`]. ~~Molten Zr that MELCOR would candle downward
  stays in the node and keeps oxidising (stated simplification).~~
  **Since 2026-10-29** molten Zr candles: once the shell breaches it
  flows down and refreezes in the first node below Zr's melting point
  (`candling`; simplified, see the candling pass). ~~a
  node whose cladding reaches [`ZR_MELT_K`] relocates~~ (that is the
  `without_rod_collapse` ablation). A fallen node relocates: its mass, energy
  and decay power move to a debris lump in the lower plenum, quenched
  while water is there; ~~dry debris above [`LOWER_HEAD_FAILURE_K`] fails
  the lower head~~ (since gh:#1034 that is only the
  `without_lower_head_wall` ablation): dry debris heats the lower-head
  wall (`HeadWall`), which fails on its inner node reaching
  [`HEAD_FAILURE_WALL_K`] or by Larson-Miller creep.

**Not in the model** (each would change the answer): ~~radiation between
rods, to the shroud and upwards (it spreads heat across the core above
about 1200 K);~~ (added 2026-10-29, gh:#1037: ring-to-ring, level-to-level
and outer ring to a shroud lump; ~~still missing: the upper-plenum
structures, the view through emptied cells,~~ since 2026-10-29 the top
of each ring radiates to the dome internals through emptied cells,
`core_top_radiation`; ~~still missing: the view through emptied cells
between rings and levels,~~ and through emptied cells between rings and
levels, `see_through`; still missing: the downcomer side of the
shroud); the two-phase swell of the mixture level (the collapsed
level uncovers the fuel early); ~~candling,~~ (simplified candling added
2026-10-29) blockages, and molten-pool physics; ~~temperature-dependent fuel properties~~ (heat capacities added
2026-10-29, gh:#1038; UO2 conductivity is still constant, waiting for a
cited Unit 1 burnup, which MATPRO's conductivity depends on); the decay heat that
leaves with volatile fission products; boron carbide control-blade
reactions. Debris after the lower head fails leaves the model.

```rust
pub mod core { /* ... */ }
```

### Types

#### Struct `CoreSnapshot`

What the plant hands the core at the start of a step.

```rust
pub struct CoreSnapshot {
    pub dt_s: f64,
    pub p_pa: f64,
    pub t_sat_k: f64,
    pub h_fg: f64,
    pub level_m: f64,
    pub decay_w: f64,
    pub t_dependent_props: bool,
    pub p_dw_pa: f64,
    pub lower_head_wall: bool,
    pub rod_collapse: bool,
    pub core_radiation: bool,
    pub diffusion_limit: bool,
    pub hydrogen_cp_t: bool,
    pub penetrations: bool,
    pub debris_oxidation: bool,
    pub candling: bool,
    pub debris_quench: bool,
    pub wet_head_contact: bool,
    pub penetration_coverage: bool,
    pub core_top_radiation: bool,
    pub upper_k: f64,
    pub see_through: bool,
    pub prm: Params,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `dt_s` | `f64` |  |
| `p_pa` | `f64` |  |
| `t_sat_k` | `f64` |  |
| `h_fg` | `f64` |  |
| `level_m` | `f64` | Collapsed water level above the bottom of active fuel, m (negative:<br>below the core; below `-BAF_M` the lower plenum is dry). |
| `decay_w` | `f64` | Decay power of the whole core, W. |
| `t_dependent_props` | `bool` | Temperature-dependent heat capacities on (the default) or the<br>constant-property ablation (`Mechanisms`). |
| `p_dw_pa` | `f64` | Drywell pressure, Pa: the outside of the lower head, for its creep<br>stress (gh:#1034). |
| `lower_head_wall` | `bool` | The lower-head wall on (the default) or the debris-temperature<br>ablation (`Mechanisms`). |
| `rod_collapse` | `bool` | MELCOR's rod-collapse criteria (the default) or the 2100 K<br>whole-node slump (`Mechanisms`). |
| `core_radiation` | `bool` | In-core radiation and the shroud (the default) or none<br>(`Mechanisms`). |
| `diffusion_limit` | `bool` | MELCOR's gaseous-diffusion limit on oxidation (the default) or the<br>parabolic rate limited only by steam supply (`Mechanisms`). |
| `hydrogen_cp_t` | `bool` | Hydrogen's `c_p(T)` from MELCOR's fit (the default) or the constant<br>[`CP_H2_J_PER_KG_K`] (`Mechanisms`). |
| `penetrations` | `bool` | Lower-head penetration failure (the default) or the wall alone<br>(`Mechanisms`). |
| `debris_oxidation` | `bool` | Oxidation of the lower-plenum debris (`Mechanisms::debris_oxidation`). |
| `candling` | `bool` | Molten Zr candles down the rods (`Mechanisms::candling`). |
| `debris_quench` | `bool` | MELCOR's falling-debris quench and dryout limit<br>(`Mechanisms::debris_quench`). |
| `wet_head_contact` | `bool` | The lower head takes heat from the debris under water too<br>(`Mechanisms::wet_head_contact`). |
| `penetration_coverage` | `bool` | The penetration heated over the debris-covered part of its height<br>only, the rest cooled by the fluid (`Mechanisms::penetration_coverage`). |
| `core_top_radiation` | `bool` | The topmost component of each ring radiates up to the dome<br>internals at `upper_k`, K (`Mechanisms::core_top_radiation`). |
| `upper_k` | `f64` |  |
| `see_through` | `bool` | Radiation looks through emptied cells to the next standing<br>component, radially and axially (`Mechanisms::see_through`). |
| `prm` | `Params` | The plant's inputs (gh:#1033). |

##### Implementations

###### Methods

- ```rust
  pub fn write(self: &Self, w: &mut Writer) { /* ... */ }
  ```

- ```rust
  pub fn read(r: &mut Reader<''_>) -> Self { /* ... */ }
  ```

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `CoreResult`

What the core hands back.

```rust
pub struct CoreResult {
    pub q_fluid_w: f64,
    pub steam_used_kg_s: f64,
    pub h2_kg_s: f64,
    pub q_ox_w: f64,
    pub peak_clad_k: f64,
    pub exit_gas_k: f64,
    pub exit_steam_kg_s: f64,
    pub fp_volatile_released: f64,
    pub uo2_released_kg: f64,
    pub zr_oxidised_kg: f64,
    pub h2_total_kg: f64,
    pub relocated: f64,
    pub debris_k: f64,
    pub debris_uo2_kg: f64,
    pub debris_zr_metal_kg: f64,
    pub lower_head_failed: bool,
    pub ex_vessel_w: f64,
    pub head_inner_k: f64,
    pub head_strain: f64,
    pub q_pool_w: f64,
    pub q_upper_w: f64,
    pub clad_k: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `q_fluid_w` | `f64` | Heat into the vessel's water and steam this step, W (mean). |
| `steam_used_kg_s` | `f64` | Steam used by oxidation, kg/s, and hydrogen made, kg/s. |
| `h2_kg_s` | `f64` |  |
| `q_ox_w` | `f64` | Oxidation heat, W (already inside the core's balance; for display). |
| `peak_clad_k` | `f64` |  |
| `exit_gas_k` | `f64` |  |
| `exit_steam_kg_s` | `f64` | Steam leaving the top of the core, kg/s (with the hydrogen,<br>`h2_kg_s`, at `exit_gas_k`). |
| `fp_volatile_released` | `f64` | Volatile fission products (Xe, Cs, I) released from the fuel so<br>far, fraction of the core inventory (gh:#1040). |
| `uo2_released_kg` | `f64` | UO2 released from the fuel so far, kg: the seed aerosol (gh:#1047). |
| `zr_oxidised_kg` | `f64` | Zirconium oxidised so far, kg; hydrogen made so far, kg. |
| `h2_total_kg` | `f64` |  |
| `relocated` | `f64` | Fraction of the core's nodes relocated, and debris temperature. |
| `debris_k` | `f64` |  |
| `debris_uo2_kg` | `f64` | The debris's UO2 and metallic Zr, kg: once the head has failed, what<br>lies on the drywell floor (`Mechanisms::mcci`). |
| `debris_zr_metal_kg` | `f64` |  |
| `lower_head_failed` | `bool` |  |
| `ex_vessel_w` | `f64` | Decay power of the debris that has left through the failed lower<br>head, W: it goes to the drywell floor. |
| `head_inner_k` | `f64` | The lower-head wall's inner temperature, K, and its creep strain<br>(gh:#1034; zero strain and the initial temperature until debris<br>arrives, and in the ablation). |
| `head_strain` | `f64` |  |
| `q_pool_w` | `f64` | Heat into the water this step, W: covered nodes and the debris<br>quench. The vapour it raises inside the pool sets the swell of the<br>mixture level (gh:#1036). |
| `q_upper_w` | `f64` | Radiated from the top of the core to the dome internals, W<br>(`core_top_radiation`). |
| `clad_k` | `Vec<f64>` | Cladding temperature of every node, ring-major, bottom to top, K<br>(relocated nodes: NaN). |

##### Implementations

###### Methods

- ```rust
  pub fn write(self: &Self, w: &mut Writer) { /* ... */ }
  ```

- ```rust
  pub fn read(r: &mut Reader<''_>) -> Self { /* ... */ }
  ```

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Core`

```rust
pub struct Core {
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
  pub fn new(t_sat_k: f64, prm: &Params) -> Self { /* ... */ }
  ```
  The core just after the scram: fuel at its full-power average

- ```rust
  pub fn step(self: &mut Self, s: &CoreSnapshot) -> CoreResult { /* ... */ }
  ```
  Advance one step. Pure function of `self` and `s`.

- ```rust
  pub fn write_state(self: &Self, w: &mut Writer) { /* ... */ }
  ```
  The whole state, for a stateless worker to step.

- ```rust
  pub fn read_state(r: &mut Reader<''_>) -> Self { /* ... */ }
  ```

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

#### Function `lipinski_dryout_flux`

Lipinski's zero-dimensional dryout heat flux of a particulate bed under
water, W/m^2 (MELCOR Vol. 2, COR §2.3.8, Eqs 2-114, 2-115; C1244: the
constant 0.756, the nominal capillary head 0.089 m for 0.5 mm particles,
scaled as `1/d`, and a minimum porosity of 0.15), at pressure `p_pa`
for a bed `height` m deep.

```rust
pub fn lipinski_dryout_flux(prm: &Params, p_pa: f64, height: f64) -> f64 { /* ... */ }
```

#### Function `oxidation_rate`

Urbanic-Heidrick parabolic rate constant, (kg Zr/m^2)^2/s.

```rust
pub fn oxidation_rate(prm: &Params, t_k: f64) -> f64 { /* ... */ }
```

#### Function `diffusion_limited_rate`

MELCOR's gaseous-diffusion limit on Zr oxidation, kg Zr/(m^2 s) (Vol. 2,
SAND2023-10994 O, COR §2.5.3, Eq 2-168; Vol. 1, SAND2017-0455 O,
C1003; no source used): steam diffusing through the hydrogen to the
surface,

```text
dW/dt = M_Zr k_c p_ox / (n R T_f),   n = 2 mol H2O per mol Zr
```

(`M_Zr/(n R)` is MELCOR's C1003(1), 0.00548 kg K/(Pa m^3), here derived
from the molar masses). `p_ox` is the steam's partial pressure in the
ring's gas (the surface's taken as zero, the limit), `T_f` the film
temperature, and `k_c = Sh D / D_h` by the heat-mass analogy on this
model's own convection correlation (`steam_htc`: Dittus-Boelter with the
Schmidt number for the Prandtl, laminar floor Sh = 4.36; MELCOR's
developing-flow factor is 1 at its 1000 m entrance length and is left
out). `D` is steam in hydrogen ([`Params::h2o_h2_diffusivity_a`]).
Simplification: the Reynolds number and the viscosity are the steam's,
as in `steam_htc`; the density in the Schmidt number is the mixture's.
MELCOR uses this rate when it is below the parabolic one, which is how
the caller applies it.

```rust
pub fn diffusion_limited_rate(prm: &Params, w_steam: f64, w_h2: f64, t_film: f64, p_pa: f64) -> f64 { /* ... */ }
```

#### Function `volatile_release_rate`

Fractional release rate of the volatile fission products (Xe, Cs, I)
from fuel at `t_k`, 1/s: MELCOR's CORSOR-M (Vol. 2, SAND2023-10994 O, RN
§2.3.1, Eq 2-11, C7102; no source used), `k0 exp(-Q / (R T))` per
minute. Xe, Cs, I2 and Te share one row of its table, so one fraction
serves the three tracked here; Te's retention by unoxidised cladding is
not modelled and Te is not tracked. Not modelled either: the gap
inventory's burst release at cladding failure (RN1_GAP00). **Checked and
rejected 2026-10-29:** with MELCOR's defaults it changes nothing
measurable. The initial gap inventory defaults to zero (RN1_GAP is
optional, NMGAP default 0; RN UG), so what remains is the fuel's release
held in the gap until the ring's cladding fails at 1173 K (RN RM
§2.3.2). Below 1173 K this rate releases at most 1.6e-5 of the
inventory per hour (2.6e-7 /min at 1173 K), so the holdup moves the
release by under 1e-5.

```rust
pub fn volatile_release_rate(prm: &Params, t_k: f64) -> f64 { /* ... */ }
```

#### Function `uo2_release_rate`

Fractional release rate of UO2 itself from fuel at `t_k`, 1/s: CORSOR-M's
row for La and UO2 (Vol. 2, RN §2.3.1, C7102), `k0 exp(-Q/(R T))` per
minute. The released UO2 is the aerosol the vapours condense on
(gh:#1047); its inventory is the fuel's UO2.

```rust
pub fn uo2_release_rate(prm: &Params, t_k: f64) -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `RINGS`

```rust
pub const RINGS: usize = 3;
```

#### Constant `NODES`

```rust
pub const NODES: usize = _;
```

#### Constant `WALL_NODES`

Through-wall nodes of the lower-head wall (inner face first).

```rust
pub const WALL_NODES: usize = 5;
```

## Module `decay`

Whole-core decay heat after the scram (gh:#1041, plan item F10).

The form is that of the ANS-5.1-1979 standard, as MELCOR's DCH package
applies it (MELCOR 2.2 Vol. 2, SAND2023-10994 O, DCH §3.2, Eqs 3-1 to 3-5
and Table 3.1; Vol. 1, SAND2017-0455 O, DCH_FPW, DCH_OPT, DCH_NFA, 3201,
3205). No MELCOR source is used:

```text
P(t) = G(t) sum_i P_FP,i(t) + (sum_i P_i / Q_i) [F_U239(t, T) + F_Np239(t, T)]
```

- `P_FP,i` is the fission-product decay heat of fissioning nuclide `i`
  (U-235 thermal, Pu-239 thermal, U-238 fast) after `T` seconds at its
  share `P_i` of the rated power. It comes from `teh-o-prke`'s 23-group
  fit, the 1978 draft ANS standard (reused, not re-entered), integrated
  analytically over the irradiation. MELCOR interpolates the 1979
  standard's tables instead; the two rest on the same England et al.
  evaluation, and the difference is not measured here.
- `G(t)` is the neutron-capture correction. Below 1e4 s it is the
  standard's formula (Eq 3-5). Above, it is the standard's Table 10,
  `G_max(t)`, which is an **upper bound**, i.e. conservative, as MELCOR
  uses it. The step at 1e4 s is the standard's own.
- `F_U239`, `F_Np239` are the decay heat of U-239 and Np-239 (Eqs 3-3,
  3-4).

Each part is a [`Mechanisms`](super::Mechanisms) switch, default on. The
ablation of the first is the model before gh:#1041: U-235 only, at
equilibrium (infinite irradiation).

```rust
pub mod decay { /* ... */ }
```

### Types

#### Struct `DecayParts`

Which parts of the decay heat are on (from `Mechanisms`).

```rust
pub struct DecayParts {
    pub mixed_finite: bool,
    pub capture: bool,
    pub actinides: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mixed_finite` | `bool` | U-235, Pu-239 and U-238 fission products after a finite irradiation;<br>off is U-235 alone at equilibrium. |
| `capture` | `bool` | The capture correction `G(t)`. |
| `actinides` | `bool` | U-239 and Np-239. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `PlantDecay`

The core's decay heat source, advanced with the plant.

```rust
pub struct PlantDecay {
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
  pub fn new(prm: &Params, parts: DecayParts) -> Self { /* ... */ }
  ```
  The decay source just after the scram from rated power.

- ```rust
  pub fn advance(self: &mut Self, dt_s: f64) { /* ... */ }
  ```
  Advance by `dt_s` seconds with no fission.

- ```rust
  pub fn t_s(self: &Self) -> f64 { /* ... */ }
  ```
  Time since the scram, s.

- ```rust
  pub fn fp_power_w(self: &Self) -> f64 { /* ... */ }
  ```
  The fission products' part of the decay power now, W: the power

- ```rust
  pub fn power_w(self: &Self) -> f64 { /* ... */ }
  ```
  Decay power now, W.

- ```rust
  pub fn capture_factor(self: &Self) -> f64 { /* ... */ }
  ```
  `G(t)`: the standard's formula below 1e4 s (MELCOR Eq 3-5), then

- ```rust
  pub fn actinide_power_w(self: &Self) -> f64 { /* ... */ }
  ```
  U-239 and Np-239 decay heat, W (MELCOR Eqs 3-2 to 3-4).

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
## Module `fp_decay`

Fission-product decay heat by element, for decay heat that follows the
fission products out of the core (gh:#1041, plan item F10).

MELCOR's DCH package gives each radionuclide class its own decay heat,
and the package holding a class's mass gets its heat. It weights ORIGEN
elemental decay heats into the classes (MELCOR 2.2 Vol. 2, DCH §2,
Table 2.1). Daughters are "assumed to remain with the parents", so an
element's heat includes its descendants'. MELCOR's elemental tables come
from SAND reports that are not held, so they are **computed here**: a
summation calculation on ENDF/B-VIII data already in the workspace.

- **Decay data**: half-lives, mean decay energies and branches from
  `openmc-endf-8-depletion-lib-a`/`-b` (OpenMC's ENDF/B-VIII chain), for
  the elements Ti to Er.
- **Yields**: ENDF/B-VIII independent fission yields
  (`fission-yields-data`): U-235 and Pu-239 thermal (0.0253 eV), U-238
  fast (500 keV).
- **History**: constant power for `operating_time_s` at the fission
  shares of `decay.rs` (MELCOR's BWR defaults), then decay.
- **Method**: every nuclide's atoms obey `dN/dt = S + sum_p lambda_p b_p
  N_p - lambda N`. The decay graph is acyclic, so one pass in topological
  order per time step suffices. Each step uses the exponential
  integrator, exact for a constant feed, with the parents' feed averaged
  over the step. **Attribution**: each nuclide born in fission is credited
  to its own element, and so are all its descendants, which is MELCOR's
  daughters-stay-with-parents rule.

Not modelled (stated): neutron capture on fission products during
irradiation (the ANS standard's `G` factor covers it for the total), and
delayed-neutron emission is followed only as far as the decay library's
branches give it.

The table is costly to compute (about 0.2 s), so it is committed:
`verification_and_validation/bwr_mark1/fp_decay_by_element.csv`, with
one row per time and each element's share of the fission-product decay
heat. A test regenerates it and compares it byte for byte; re-bless with
`FUKUSHIMA_BLESS=1`.

```rust
pub mod fp_decay { /* ... */ }
```

### Types

#### Struct `Summation`

The summation's results: each element's fission-product decay heat, W,
at [`table_times`] (descendants credited to the element born in
fission), and each element's mass at the scram, kg (by the element the
atoms are then, stable isotopes included).

```rust
pub struct Summation {
    pub heat_w: Vec<Vec<f64>>,
    pub mass_kg: Vec<f64>,
    pub nuclide_kg: Vec<(String, f64)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `heat_w` | `Vec<Vec<f64>>` |  |
| `mass_kg` | `Vec<f64>` |  |
| `nuclide_kg` | `Vec<(String, f64)>` | Each nuclide's mass at the scram, kg, by its ENDF name (`Cs137`). |

##### Implementations

###### Methods

- ```rust
  pub fn nuclide_mass_kg(self: &Self, name: &str) -> f64 { /* ... */ }
  ```
  One nuclide's mass at the scram, kg (0 if absent).

- ```rust
  pub fn element_mass_kg(self: &Self, symbol: &str) -> f64 { /* ... */ }
  ```
  One element's mass at the scram, kg (0 if absent).

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
- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `ElementShares`

The committed table, read: times, s, and each element's share of the
fission-product decay heat at each.

```rust
pub struct ElementShares {
    pub times_s: Vec<f64>,
    pub symbols: Vec<String>,
    pub shares: Vec<Vec<f64>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `times_s` | `Vec<f64>` |  |
| `symbols` | `Vec<String>` |  |
| `shares` | `Vec<Vec<f64>>` |  |

##### Implementations

###### Methods

- ```rust
  pub fn committed() -> Self { /* ... */ }
  ```

- ```rust
  pub fn share(self: &Self, symbols: &[&str], t_s: f64) -> f64 { /* ... */ }
  ```
  The share of the fission-product decay heat carried by the elements

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

#### Function `table_times`

The times of the committed table, s after the scram: 10 s to 600 h,
61 points evenly spaced in the logarithm.

```rust
pub fn table_times() -> Vec<f64> { /* ... */ }
```

#### Function `compute`

The summation (see the module doc).

```rust
pub fn compute(prm: &super::params::Params) -> Summation { /* ... */ }
```

#### Function `table_csv`

The committed table as CSV text: a header of element symbols, then per
time its value and each element's share of the total.

```rust
pub fn table_csv(sum: &Summation) -> String { /* ... */ }
```

#### Function `inventory_csv`

The element masses at the scram as CSV text: symbol, kg.

```rust
pub fn inventory_csv(sum: &Summation) -> String { /* ... */ }
```

#### Function `inventory_kg`

An element's mass at the scram from the committed inventory, kg (0 if
absent).

```rust
pub fn inventory_kg(symbol: &str) -> f64 { /* ... */ }
```

## Module `gas`

Heat capacities of the non-condensable gases, N2 and H2.

Each has its own `c_v(T)`, from MELCOR 2.2's NCG library fits (Vol. 2,
SAND2023-10994 O, NCG/H2O-RM-33/34; the form is Vol. 1, SAND2017-0455 O,
NCG-UG-2, Eq 1.1; no source used):

```text
c_v(T) = cv0 + cv1 T + cv2 T^2 + cv3 T^3 + cvsqrt/sqrt(T) + cvm1/T + cvm2/T^2
```

The coefficients are mass-specific and apply inside `T_low..=T_up`. Outside
that range `c_v` is held at its boundary value. Internal energy is
measured from 0 K, the containment's existing convention, so it is
`c_v(T_low) T` below `T_low`, the integral of the fit inside the range,
and linear in `c_v(T_up)` above `T_up`. Before 2026-10-29 these were
constants, 20.8 (N2) and 20.5 (H2) J/(mol K), and the
`without_gas_cv_t` ablation restores them.

```rust
pub mod gas { /* ... */ }
```

### Functions

#### Function `cv_n2`

Molar `c_v` of N2 and of H2, J/(mol K); `t_dependent` false is the
constant-value ablation.

```rust
pub fn cv_n2(t: f64, t_dependent: bool) -> f64 { /* ... */ }
```

#### Function `cv_h2`

```rust
pub fn cv_h2(t: f64, t_dependent: bool) -> f64 { /* ... */ }
```

#### Function `u_n2`

Molar internal energy of N2 and of H2 from 0 K, J/mol.

```rust
pub fn u_n2(t: f64, t_dependent: bool) -> f64 { /* ... */ }
```

#### Function `u_h2`

```rust
pub fn u_h2(t: f64, t_dependent: bool) -> f64 { /* ... */ }
```

#### Function `ncg_u`

Internal energy of `n2` and `h2` moles at `t`, J. The ablation keeps the
operation order of the code before 2026-10-29, so it reproduces it bit
for bit.

```rust
pub fn ncg_u(n2: f64, h2: f64, t: f64, t_dependent: bool) -> f64 { /* ... */ }
```

#### Function `ncg_c`

Heat capacity of `n2` and `h2` moles at `t`, J/K.

```rust
pub fn ncg_c(n2: f64, h2: f64, t: f64, t_dependent: bool) -> f64 { /* ... */ }
```

#### Function `h2_molar_enthalpy`

Hydrogen's molar enthalpy from 0 K, `u + R T`, J/mol (the ablation as
before: `(c_v + R) T`).

```rust
pub fn h2_molar_enthalpy(t: f64, t_dependent: bool) -> f64 { /* ... */ }
```

#### Function `cp_h2_per_kg`

Hydrogen's `c_p`, J/(kg K), and its enthalpy above 298.15 K, J/kg, for
the core and vessel (the `hydrogen_cp_t` mechanism). The ablation's
`c_p` is the constant the core used before, `(20.5 + R)/M_H2`.

```rust
pub fn cp_h2_per_kg(t: f64, t_dependent: bool) -> f64 { /* ... */ }
```

#### Function `h2_enthalpy_above_298`

```rust
pub fn h2_enthalpy_above_298(t: f64) -> f64 { /* ... */ }
```

## Module `internals`

The steam dome's internals as a heat structure (gh:#1037, plan item F6).

The gas leaving the top of the core passes the shroud head, the steam
separators and the dryer before it reaches the steam lines, the leak
paths and the containment. Those structures are tens of tonnes of steel
at saturation temperature, and they cool a superheated gas. MELCOR
models them as heat structures in the upper-plenum and dome volumes
(Vol. 2, SAND2023-10994 O, HS package; no source used); here they are one
lumped node.

The gas (steam and hydrogen, heat-capacity rate `C_g`) leaves at

```text
T_out = T_s + (T_in - T_s) exp(-NTU),   NTU = h A / C_g
```

and the structure takes `C_g (T_in - T_out)`, which the vessel's energy
gives up (`Plant::finish`). When the gas is cooler than the structure,
the same exchange gives the heat back. The structure's enthalpy is
304L's (`core::steel_h`), as for the steam lines.

With no gas flowing past them they still lose heat to the saturated dome
by natural convection (`lose`, since 2026-10-29; before, nothing cooled
them once the flow stopped, which kept them hot and re-evaporated the
cesium they held).

Its mass, area and heat transfer coefficients are all \[A\]
(`params::INTERNALS_*`): Unit 1's internals are not in a held source.
The ablation,
[`Mechanisms::without_dome_internals`](super::Mechanisms), passes the
core's exit gas straight through: the model before 2026-10-29.

```rust
pub mod internals { /* ... */ }
```

### Types

#### Struct `DomeInternals`

The internals' temperature, K.

```rust
pub struct DomeInternals {
    pub t_k: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `t_k` | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn new(t_k: f64) -> Self { /* ... */ }
  ```

- ```rust
  pub fn step(self: &mut Self, prm: &Params, w_steam: f64, w_h2: f64, cp_h2: f64, t_in: f64, p: f64, dt: f64) -> (f64, f64) { /* ... */ }
  ```
  Pass a gas of `w_steam` and `w_h2` kg/s at `t_in` K (vessel pressure

- ```rust
  pub fn lose(self: &mut Self, prm: &Params, t_sat: f64, dt: f64) -> f64 { /* ... */ }
  ```
  Natural convection from the internals to the vessel's saturated

- ```rust
  pub fn heat(self: &mut Self, prm: &Params, q_w: f64, dt: f64) { /* ... */ }
  ```
  Heat `q_w` W deposited in the internals for `dt` s (the decay heat

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
## Module `iodine`

Iodine in the suppression pool: SPARC-90's equilibrium chemistry and
the partition limit on its scrubbing, as MELCOR describes them (gh:#1047;
no source used; MELCOR 2.2 Vol. 2, SAND2023-10994 O, RN §2.7.1,
Appendix D Eq D-7 and Appendix F).

The pool's dissolved iodine is in fast equilibrium among five reactions
(Eq F.6):

```text
I2(g)          <-> I2(aq)             [I2aq] = K1 [I2g]
I2(aq) + I-    <-> I3-                [I3-] = K2 [I2aq][I-]
I2(aq) + H2O   <-> H+ + I- + HIO      [H+][I-][HIO] = K3 [I2aq]
I2(aq) + H2O   <-> H2OI+ + I-         [H2OI+][I-] = K4 [I2aq]
H2O            <-> H+ + OH-           [H+][OH-] = K5
```

with the total (Eq F.3, as I2 equivalents) `S = 0.5[I-] + 1.5[I3-] +
[I2aq] + 0.5[HIO] + 0.5[H2OI+]` and the charge balance (Eq F.15)
`([I-] - [I-]_p) + [I3-] - [H2OI+] - [HIO] = 0`. Here `[I-]_p` is the
iodide of dissolved CsI and `[OH-]_p` the hydroxide of dissolved CsOH.
The pool's partition coefficient for I2 is then `H = K1 S / [I2aq]`
(Eq F.5).

**The proton balance is derived here, not copied.** From F.16, `[HIO] =
a / [H+]` with `a = K3 [I2aq] / [I-]`; put into F.18, `[H+]^2 -
([HIO] - [OH-]_p)[H+] - K5 = 0`, it gives `[H+]^2 + [OH-]_p [H+] -
(a + K5) = 0`. That is the content of the manual's F.19 to F.21, whose
printed form did not survive extraction.

**The equilibrium constants' forms are inferred.** The manual lists
C7159's coefficients and their units (`moles^-1`, `K`, `K^2`) but not
the functions. `K_i = a_i exp(b_i / T)`, and for water `K5 = a exp(b/T +
c/T^2)`, reproduce the textbook values at 298 K (the test): K1 = 83.6
(I2's water-to-gas ratio, about 80 to 90), K2 = 769 (I3- formation,
about 700 to 770), K3 = 4.0e-13 (I2 hydrolysis, about 5e-13), and Kw =
1.0e-14 (pKw 14.0).

**Where it acts.** MELCOR limits the condensation scrubbing of iodine
vapour by the partition (Eq D-7):

```text
DF_EC,vap = 1 + min(H rho_v / rho_l, 1) (DF_EC - 1)
```

The condensate cannot hold more iodine than the partition allows. And it
stops vapour scrubbing altogether when the pool's own iodine, over H,
exceeds the entering gas's (RN §2.7.1).

```rust
pub mod iodine { /* ... */ }
```

### Types

#### Struct `Equilibrium`

The equilibrium species, mol/L.

```rust
pub struct Equilibrium {
    pub i2_aq: f64,
    pub iodide: f64,
    pub hio: f64,
    pub h_plus: f64,
    pub tri_iodide: f64,
    pub h2oi: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `i2_aq` | `f64` |  |
| `iodide` | `f64` |  |
| `hio` | `f64` |  |
| `h_plus` | `f64` |  |
| `tri_iodide` | `f64` |  |
| `h2oi` | `f64` |  |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

#### Function `constants`

The five equilibrium constants at `t_k` (C7159, forms inferred; see the
module doc): K1 dimensionless, K2 L/mol, K3 mol^2/L^2, K4 mol/L, K5
mol^2/L^2.

```rust
pub fn constants(t_k: f64) -> [f64; 5] { /* ... */ }
```

#### Function `equilibrium`

The pool's equilibrium for a total dissolved iodine `s` (mol of I2
equivalents per litre, Eq F.3), CsOH hydroxide `oh_p` and CsI iodide
`i_p` (mol/L), at `t_k`. Solved as the manual does: for an assumed
`[I2aq]`, the charge balance gives `[I-]`; `[I2aq]` is then adjusted
until the total matches (both by bisection, every function monotone).

```rust
pub fn equilibrium(t_k: f64, s: f64, oh_p: f64, i_p: f64) -> Equilibrium { /* ... */ }
```

#### Function `partition`

The pool's partition coefficient for I2, `H = K1 S / [I2aq]` (Eq F.5):
dissolved iodine (all forms, as I2) per gaseous I2 at equilibrium. With
no dissolved iodine it is the hydrolysis limit at vanishing load, taken
at 1e-15 mol/L.

```rust
pub fn partition(t_k: f64, s: f64, oh_p: f64, i_p: f64) -> f64 { /* ... */ }
```

#### Function `vapour_condensation_df`

MELCOR's Eq D-7: the condensation factor for iodine vapour, from the
gas's `df_ec`, the partition `h` and the vapour-to-liquid density
ratio.

```rust
pub fn vapour_condensation_df(df_ec: f64, h: f64, rho_v_over_rho_l: f64) -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `M_I2`

Molar masses, kg/mol: I2, and Cs (as CsOH, one hydroxide per atom).

```rust
pub const M_I2: f64 = 0.25381;
```

#### Constant `M_CS`

```rust
pub const M_CS: f64 = 0.13291;
```

## Module `msl`

Creep rupture of a main steam line (gh:#1037, plan item F6).

The relief valves sit on the main steam lines inside the drywell, so the
gas they pass flows through the lines first. When the core's exit gas is
hot, it heats the line's wall. Under the vessel's pressure the wall
creeps and, if damage reaches 1, one line ruptures and opens its flow
area to the drywell. The failure model is MELCOR's pair of control
functions for exactly this (no source used; Vol. 1, SAND2017-0455 O, CF
§2.1.9, §2.1.10, §7.8):

```text
PIPE-STR:  sigma = ((R_o^2 + R_i^2) P_i - 2 R_o^2 P_o) / ((R_o + R_i)(R_o - R_i))
LM-CREEP:  t_R = 10^(P_LM/T - C),  P_LM = a1 log10(sigma) + b1,
           damage = sum dt / t_R,  failure at 1
```

MELCOR has no built-in steam-line model; its users build this from the
two functions and a heat structure, as here. The wall is one lumped node
per metre of line (the length cancels):

- inside, forced convection from the relief flow at the core's exit gas
  temperature, Dittus-Boelter on the line's diameter with a laminar
  floor Nu = 4.36 when the valves are shut. The gas temperature is an
  **upper bound**: the steam dome's separators and dryers would cool it,
  and those heat structures are not modelled (the other half of F6);
- outside, insulation to the drywell gas (`msl_insulation_h`, \[A\]);
- the wall's enthalpy is 304L stainless steel's (`core::steel_h`), a
  stated substitution: no carbon-steel property set exists in the
  workspace, and carbon steel's magnetic transition near 1033 K (a `c_p`
  peak) is missed.

The ablation, [`Mechanisms::without_msl_creep`](super::Mechanisms), is
the model before 2026-10-29: no steam-line failure.

```rust
pub mod msl { /* ... */ }
```

### Types

#### Struct `SteamLine`

One main steam line's wall: temperature, accumulated creep damage, and
whether it has ruptured.

```rust
pub struct SteamLine {
    pub wall_k: f64,
    pub damage: f64,
    pub failed: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `wall_k` | `f64` |  |
| `damage` | `f64` |  |
| `failed` | `bool` |  |

##### Implementations

###### Methods

- ```rust
  pub fn new(t_k: f64) -> Self { /* ... */ }
  ```

- ```rust
  pub fn step(self: &mut Self, prm: &Params, w_relief: f64, t_gas: f64, p: f64, p_dw: f64, t_dw: f64, dt: f64) { /* ... */ }
  ```
  Advance by `dt` s: relief flow `w_relief` (kg/s, shared by the

- ```rust
  pub fn break_area_m2(prm: &Params) -> f64 { /* ... */ }
  ```
  The ruptured line's flow area, m^2 (one line).

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

#### Function `pipe_stress`

MELCOR's PIPE-STR: the inner-surface stress of a thick-walled pipe, Pa.

```rust
pub fn pipe_stress(r_i: f64, r_o: f64, p_in: f64, p_out: f64) -> f64 { /* ... */ }
```

#### Function `rupture_time_s`

MELCOR's LM-CREEP time to rupture, s, at stress `sigma` (Pa) and wall
temperature `t` (K), with the material's `a1`, `b1`, `c`.

```rust
pub fn rupture_time_s(prm: &super::params::Params, sigma: f64, t: f64) -> f64 { /* ... */ }
```

## Module `params`

Fukushima Daiichi Unit 1 (GE BWR/3, Mark I containment): every number the
model uses, with where it came from.

**Read this before quoting anything the simulator shows.** This is an AI
draft (2026-10-21). The numbers were gathered by a literature search that
could read search-result excerpts but **not the primary PDFs**. Each is
marked:

- **\[P\]**: a published figure from a named source, not yet checked against
  the primary document;
- **\[U\]**: unverified (from memory, or an ambiguous excerpt);
- **\[A\]**: an assumption or placeholder of this model's, with no source.
  What would replace it is said at the line.

The sources the design data should be checked against (all openly
redistributable): INPO 11-005 (2011) Tables 7.3-1/7.3-2; Gauntt et al.,
SAND2012-6173 (Sandia, MELCOR models of Units 1-3); IAEA, *The Fukushima
Daiichi Accident*, Technical Volume 1 (2015); the Japanese government's
June 2011 report to the IAEA, attachment 4; TEPCO's plant parameter PDFs
(the measured data, for comparison only). The OECD/NEA BSAF specification
is restricted and is **not** used.

```rust
pub mod params { /* ... */ }
```

### Types

#### Enum `Tag`

Where an input's default comes from (the module docs' marks).

```rust
pub enum Tag {
    P,
    U,
    A,
}
```

##### Variants

###### `P`

A published figure from a named source, not yet checked against the
primary document.

###### `U`

Unverified (from memory, or an ambiguous excerpt).

###### `A`

An assumption or placeholder of this model's, with no source.

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Params`

Every scalar input of the model, as one value (gh:#1033): what a
caller changes to run the plant with a corrected or perturbed
input, without recompiling. [`Params::default`] is the constants
above, field for field (`thermal_power_w` is [`THERMAL_POWER_W`],
and so on), so their tags and sources are the defaults'. Left as
constants: the node and train counts, which size arrays
([`AXIAL_NODES`], [`IC_TRAINS`]), and physical constants (molar
masses, [`R_GAS`], [`P_ATM`]).

It travels whole in each part's snapshot, so a worker steps with
the plant's inputs and one worker still equals three.

```rust
pub struct Params {
    pub thermal_power_w: f64,
    pub assemblies: f64,
    pub rods_per_assembly: f64,
    pub active_length_m: f64,
    pub clad_outer_r_m: f64,
    pub clad_wall_m: f64,
    pub pellet_r_m: f64,
    pub uo2_density: f64,
    pub zr_density: f64,
    pub channel_inner_width_m: f64,
    pub channel_wall_m: f64,
    pub channel_length_m: f64,
    pub uo2_cp: f64,
    pub zr_cp: f64,
    pub uo2_k: f64,
    pub gap_h: f64,
    pub boiling_h: f64,
    pub cosine_extrapolation: f64,
    pub rpv_radius_m: f64,
    pub rpv_height_m: f64,
    pub rpv_wall_m: f64,
    pub internals_envelope_m2: f64,
    pub concrete_density: f64,
    pub concrete_ablation_k: f64,
    pub concrete_h2o_fraction: f64,
    pub concrete_co2_fraction: f64,
    pub concrete_cp: f64,
    pub concrete_ablation_j_per_kg: f64,
    pub cavity_area_m2: f64,
    pub cavity_h: f64,
    pub cavity_emissivity: f64,
    pub zr_co2_heat_j_per_kg: f64,
    pub baf_m: f64,
    pub lower_plenum_free: f64,
    pub upper_free: f64,
    pub core_other_area_m2: f64,
    pub normal_level_above_taf_m: f64,
    pub initial_pressure_pa: f64,
    pub msiv_closure_s: f64,
    pub bypass_gain_kg_s_pa: f64,
    pub relief_valves: f64,
    pub relief_open_pa: f64,
    pub relief_close_pa: f64,
    pub safety_open_pa: f64,
    pub safety_close_pa: f64,
    pub relief_throat_m2: f64,
    pub ic_autostart_pa: f64,
    pub ic_ua_w_per_k: f64,
    pub ic_shell_water_kg: f64,
    pub fire_engine_shutoff_pa: f64,
    pub fire_engine_max_kg_s: f64,
    pub injection_temperature_k: f64,
    pub hot_leak_gas_k: f64,
    pub hot_leak_m2: f64,
    pub msl_lines: f64,
    pub msl_inner_radius_m: f64,
    pub msl_wall_m: f64,
    pub msl_insulation_h: f64,
    pub msl_lm_a1: f64,
    pub msl_lm_b1: f64,
    pub msl_lm_c: f64,
    pub internals_kg: f64,
    pub internals_area_m2: f64,
    pub internals_h: f64,
    pub internals_loss_h: f64,
    pub lower_head_failure_k: f64,
    pub breach_m2: f64,
    pub debris_quench_s: f64,
    pub quench_h: f64,
    pub quench_tau_s: f64,
    pub lower_head_thickness_m: f64,
    pub debris_to_head_w_m2_k: f64,
    pub penetration_wall_m: f64,
    pub corsor_k0_per_min: f64,
    pub corsor_q_kcal_per_mol: f64,
    pub corsor_uo2_k0_per_min: f64,
    pub corsor_uo2_q_kcal_per_mol: f64,
    pub cs_inventory_kg: f64,
    pub i_inventory_kg: f64,
    pub te_inventory_kg: f64,
    pub dw_floor_m2: f64,
    pub ww_pool_surface_m2: f64,
    pub head_failure_wall_k: f64,
    pub lm_p0: f64,
    pub lm_slope: f64,
    pub lm_c: f64,
    pub creep_fail_strain: f64,
    pub debris_porosity: f64,
    pub debris_particle_m: f64,
    pub lp_bottom_level_m: f64,
    pub breach_diameter_m: f64,
    pub zr_melt_k: f64,
    pub clad_breach_k: f64,
    pub min_metal_m: f64,
    pub rod_collapse_k: f64,
    pub rod_collapse_max_k: f64,
    pub damage_a: f64,
    pub damage_b: f64,
    pub assembly_pitch_m: f64,
    pub radial_exchange: f64,
    pub axial_exchange: f64,
    pub shroud_thickness_m: f64,
    pub zro2_density: f64,
    pub bubble_rise_m_s: f64,
    pub max_pool_void: f64,
    pub fission_share_u235: f64,
    pub fission_share_pu239: f64,
    pub operating_time_s: f64,
    pub fissions_per_fissile_atom: f64,
    pub q_u235_mev: f64,
    pub q_pu239_mev: f64,
    pub q_u238_mev: f64,
    pub u239_per_fission: f64,
    pub e_u239_mev: f64,
    pub e_np239_mev: f64,
    pub u239_decay_per_s: f64,
    pub np239_decay_per_s: f64,
    pub uh_low_a: f64,
    pub uh_low_b_k: f64,
    pub uh_high_a: f64,
    pub uh_high_b_k: f64,
    pub uh_low_max_k: f64,
    pub uh_high_min_k: f64,
    pub oxidation_min_k: f64,
    pub h2o_h2_diffusivity_a: f64,
    pub h2o_h2_diffusivity_b: f64,
    pub oxidation_heat_j_per_kg_zr: f64,
    pub drywell_volume_m3: f64,
    pub dw_wall_area_m2: f64,
    pub dw_wall_h: f64,
    pub dw_wall_c: f64,
    pub dw_wall_height_m: f64,
    pub dw_gap_m: f64,
    pub dw_gap_emissivity: f64,
    pub dw_concrete_m: f64,
    pub concrete_k: f64,
    pub rb_wall_h: f64,
    pub ww_shell_area_m2: f64,
    pub ww_shell_c: f64,
    pub ww_shell_height_m: f64,
    pub wetwell_gas_m3: f64,
    pub pool_water_m3: f64,
    pub containment_design_pa: f64,
    pub initial_containment_pa: f64,
    pub initial_drywell_k: f64,
    pub initial_pool_k: f64,
    pub vent_submergence_pa: f64,
    pub downcomer_area_m2: f64,
    pub downcomer_count: f64,
    pub discharge_coefficient: f64,
    pub vacuum_breaker_pa: f64,
    pub vacuum_breaker_m2: f64,
    pub vent_rupture_disk_pa: f64,
    pub vent_area_m2: f64,
    pub flange_leak_pa: f64,
    pub flange_leak_m2: f64,
    pub reactor_building_m3: f64,
    pub h2_flammable: f64,
    pub h2_explosion: f64,
    pub ring_factors: [f64; 3],
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `thermal_power_w` | `f64` |  |
| `assemblies` | `f64` |  |
| `rods_per_assembly` | `f64` |  |
| `active_length_m` | `f64` |  |
| `clad_outer_r_m` | `f64` |  |
| `clad_wall_m` | `f64` |  |
| `pellet_r_m` | `f64` |  |
| `uo2_density` | `f64` |  |
| `zr_density` | `f64` |  |
| `channel_inner_width_m` | `f64` |  |
| `channel_wall_m` | `f64` |  |
| `channel_length_m` | `f64` |  |
| `uo2_cp` | `f64` |  |
| `zr_cp` | `f64` |  |
| `uo2_k` | `f64` |  |
| `gap_h` | `f64` |  |
| `boiling_h` | `f64` |  |
| `cosine_extrapolation` | `f64` |  |
| `rpv_radius_m` | `f64` |  |
| `rpv_height_m` | `f64` |  |
| `rpv_wall_m` | `f64` |  |
| `internals_envelope_m2` | `f64` |  |
| `concrete_density` | `f64` |  |
| `concrete_ablation_k` | `f64` |  |
| `concrete_h2o_fraction` | `f64` |  |
| `concrete_co2_fraction` | `f64` |  |
| `concrete_cp` | `f64` |  |
| `concrete_ablation_j_per_kg` | `f64` |  |
| `cavity_area_m2` | `f64` |  |
| `cavity_h` | `f64` |  |
| `cavity_emissivity` | `f64` |  |
| `zr_co2_heat_j_per_kg` | `f64` |  |
| `baf_m` | `f64` |  |
| `lower_plenum_free` | `f64` |  |
| `upper_free` | `f64` |  |
| `core_other_area_m2` | `f64` |  |
| `normal_level_above_taf_m` | `f64` |  |
| `initial_pressure_pa` | `f64` |  |
| `msiv_closure_s` | `f64` |  |
| `bypass_gain_kg_s_pa` | `f64` |  |
| `relief_valves` | `f64` |  |
| `relief_open_pa` | `f64` |  |
| `relief_close_pa` | `f64` |  |
| `safety_open_pa` | `f64` |  |
| `safety_close_pa` | `f64` |  |
| `relief_throat_m2` | `f64` |  |
| `ic_autostart_pa` | `f64` |  |
| `ic_ua_w_per_k` | `f64` |  |
| `ic_shell_water_kg` | `f64` |  |
| `fire_engine_shutoff_pa` | `f64` |  |
| `fire_engine_max_kg_s` | `f64` |  |
| `injection_temperature_k` | `f64` |  |
| `hot_leak_gas_k` | `f64` |  |
| `hot_leak_m2` | `f64` |  |
| `msl_lines` | `f64` |  |
| `msl_inner_radius_m` | `f64` |  |
| `msl_wall_m` | `f64` |  |
| `msl_insulation_h` | `f64` |  |
| `msl_lm_a1` | `f64` |  |
| `msl_lm_b1` | `f64` |  |
| `msl_lm_c` | `f64` |  |
| `internals_kg` | `f64` |  |
| `internals_area_m2` | `f64` |  |
| `internals_h` | `f64` |  |
| `internals_loss_h` | `f64` |  |
| `lower_head_failure_k` | `f64` |  |
| `breach_m2` | `f64` |  |
| `debris_quench_s` | `f64` |  |
| `quench_h` | `f64` |  |
| `quench_tau_s` | `f64` |  |
| `lower_head_thickness_m` | `f64` |  |
| `debris_to_head_w_m2_k` | `f64` |  |
| `penetration_wall_m` | `f64` |  |
| `corsor_k0_per_min` | `f64` |  |
| `corsor_q_kcal_per_mol` | `f64` |  |
| `corsor_uo2_k0_per_min` | `f64` |  |
| `corsor_uo2_q_kcal_per_mol` | `f64` |  |
| `cs_inventory_kg` | `f64` |  |
| `i_inventory_kg` | `f64` |  |
| `te_inventory_kg` | `f64` |  |
| `dw_floor_m2` | `f64` |  |
| `ww_pool_surface_m2` | `f64` |  |
| `head_failure_wall_k` | `f64` |  |
| `lm_p0` | `f64` |  |
| `lm_slope` | `f64` |  |
| `lm_c` | `f64` |  |
| `creep_fail_strain` | `f64` |  |
| `debris_porosity` | `f64` |  |
| `debris_particle_m` | `f64` |  |
| `lp_bottom_level_m` | `f64` |  |
| `breach_diameter_m` | `f64` |  |
| `zr_melt_k` | `f64` |  |
| `clad_breach_k` | `f64` |  |
| `min_metal_m` | `f64` |  |
| `rod_collapse_k` | `f64` |  |
| `rod_collapse_max_k` | `f64` |  |
| `damage_a` | `f64` |  |
| `damage_b` | `f64` |  |
| `assembly_pitch_m` | `f64` |  |
| `radial_exchange` | `f64` |  |
| `axial_exchange` | `f64` |  |
| `shroud_thickness_m` | `f64` |  |
| `zro2_density` | `f64` |  |
| `bubble_rise_m_s` | `f64` |  |
| `max_pool_void` | `f64` |  |
| `fission_share_u235` | `f64` |  |
| `fission_share_pu239` | `f64` |  |
| `operating_time_s` | `f64` |  |
| `fissions_per_fissile_atom` | `f64` |  |
| `q_u235_mev` | `f64` |  |
| `q_pu239_mev` | `f64` |  |
| `q_u238_mev` | `f64` |  |
| `u239_per_fission` | `f64` |  |
| `e_u239_mev` | `f64` |  |
| `e_np239_mev` | `f64` |  |
| `u239_decay_per_s` | `f64` |  |
| `np239_decay_per_s` | `f64` |  |
| `uh_low_a` | `f64` |  |
| `uh_low_b_k` | `f64` |  |
| `uh_high_a` | `f64` |  |
| `uh_high_b_k` | `f64` |  |
| `uh_low_max_k` | `f64` |  |
| `uh_high_min_k` | `f64` |  |
| `oxidation_min_k` | `f64` |  |
| `h2o_h2_diffusivity_a` | `f64` |  |
| `h2o_h2_diffusivity_b` | `f64` |  |
| `oxidation_heat_j_per_kg_zr` | `f64` |  |
| `drywell_volume_m3` | `f64` |  |
| `dw_wall_area_m2` | `f64` |  |
| `dw_wall_h` | `f64` |  |
| `dw_wall_c` | `f64` |  |
| `dw_wall_height_m` | `f64` |  |
| `dw_gap_m` | `f64` |  |
| `dw_gap_emissivity` | `f64` |  |
| `dw_concrete_m` | `f64` |  |
| `concrete_k` | `f64` |  |
| `rb_wall_h` | `f64` |  |
| `ww_shell_area_m2` | `f64` |  |
| `ww_shell_c` | `f64` |  |
| `ww_shell_height_m` | `f64` |  |
| `wetwell_gas_m3` | `f64` |  |
| `pool_water_m3` | `f64` |  |
| `containment_design_pa` | `f64` |  |
| `initial_containment_pa` | `f64` |  |
| `initial_drywell_k` | `f64` |  |
| `initial_pool_k` | `f64` |  |
| `vent_submergence_pa` | `f64` |  |
| `downcomer_area_m2` | `f64` |  |
| `downcomer_count` | `f64` |  |
| `discharge_coefficient` | `f64` |  |
| `vacuum_breaker_pa` | `f64` |  |
| `vacuum_breaker_m2` | `f64` |  |
| `vent_rupture_disk_pa` | `f64` |  |
| `vent_area_m2` | `f64` |  |
| `flange_leak_pa` | `f64` |  |
| `flange_leak_m2` | `f64` |  |
| `reactor_building_m3` | `f64` |  |
| `h2_flammable` | `f64` |  |
| `h2_explosion` | `f64` |  |
| `ring_factors` | `[f64; 3]` | [`RING_FACTORS`]. |

##### Implementations

###### Methods

- ```rust
  pub fn write(self: &Self, w: &mut Writer) { /* ... */ }
  ```

- ```rust
  pub fn read(r: &mut Reader<''_>) -> Self { /* ... */ }
  ```

- ```rust
  pub fn get(self: &Self, name: &str) -> Option<f64> { /* ... */ }
  ```
  The input named `name` (a field name from [`Params::INPUTS`]).

- ```rust
  pub fn set(self: &mut Self, name: &str, value: f64) -> bool { /* ... */ }
  ```
  Sets the input named `name`; `false` if there is none.

- ```rust
  pub fn core_uo2_kg(self: &Self) -> f64 { /* ... */ }
  ```
  UO2 mass of the whole core, kg (from the pellet geometry).

- ```rust
  pub fn core_zr_kg(self: &Self) -> f64 { /* ... */ }
  ```
  Zircaloy mass of the whole core (cladding over the active length plus

- ```rust
  pub fn core_zr_area_m2(self: &Self) -> f64 { /* ... */ }
  ```
  Zircaloy surface exposed to steam over the active length, m^2: the rods'

- ```rust
  pub fn core_rod_area_m2(self: &Self) -> f64 { /* ... */ }
  ```
  Rod surface over the active length, m^2 (the heat-transfer surface).

- ```rust
  pub fn core_flow_area_m2(self: &Self) -> f64 { /* ... */ }
  ```
  Coolant flow area inside the channels, m^2, and their hydraulic diameter, m.

- ```rust
  pub fn hydraulic_diameter_m(self: &Self) -> f64 { /* ... */ }
  ```

- ```rust
  pub fn fuel_to_clad_w_per_m_k(self: &Self) -> f64 { /* ... */ }
  ```
  Fuel-to-clad conductance per unit rod length, W/(m K): the pellet's

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
### Constants and Statics

#### Constant `THERMAL_POWER_W`

Rated thermal power, W. \[P\] TEPCO facility overview: 1380 MWt (460 MWe).

```rust
pub const THERMAL_POWER_W: f64 = 1380.0e6;
```

#### Constant `ASSEMBLIES`

Fuel assemblies in the core. \[P\] TEPCO: 400.

```rust
pub const ASSEMBLIES: f64 = 400.0;
```

#### Constant `RODS_PER_ASSEMBLY`

Fuel rods per assembly: a GE 8x8 bundle with 62 fuel rods (two water
rods). \[U\] Unit 1's 2011 core lattice is not confirmed (INPO Table 7.3).

```rust
pub const RODS_PER_ASSEMBLY: f64 = 62.0;
```

#### Constant `ACTIVE_LENGTH_M`

Active fuel length, m: GE's 12 ft (144 in). \[U\] for Unit 1.

```rust
pub const ACTIVE_LENGTH_M: f64 = 3.66;
```

#### Constant `CLAD_OUTER_R_M`

Clad outer radius, m (12.27 mm OD, GE 8x8). \[U\]

```rust
pub const CLAD_OUTER_R_M: f64 = 0.006135;
```

#### Constant `CLAD_WALL_M`

Clad wall, m (0.81 mm). \[U\]

```rust
pub const CLAD_WALL_M: f64 = 0.00081;
```

#### Constant `PELLET_R_M`

Pellet radius, m (10.4 mm diameter). \[U\]

```rust
pub const PELLET_R_M: f64 = 0.0052;
```

#### Constant `UO2_DENSITY`

UO2 density, kg/m^3: 95 % of 10,960 theoretical. \[A\] typical LWR value.

```rust
pub const UO2_DENSITY: f64 = 10_412.0;
```

#### Constant `ZR_DENSITY`

Zircaloy density, kg/m^3. \[P\] MATPRO, about 6550.

```rust
pub const ZR_DENSITY: f64 = 6550.0;
```

#### Constant `CHANNEL_INNER_WIDTH_M`

Channel box: inner width, m (5.278 in), wall (80 mil), length (about 4.2 m).
\[U\] GE BWR/3-4 channel; counted in the zirconium that oxidises.

```rust
pub const CHANNEL_INNER_WIDTH_M: f64 = 0.134;
```

#### Constant `CHANNEL_WALL_M`

```rust
pub const CHANNEL_WALL_M: f64 = 0.002032;
```

#### Constant `CHANNEL_LENGTH_M`

```rust
pub const CHANNEL_LENGTH_M: f64 = 4.2;
```

#### Constant `UO2_CP`

Specific heats, J/(kg K), held constant. \[A\] UO2 is about 300 near 600 K
and 450 near 2500 K, Zircaloy about 330 with a peak at its alpha-beta
transition (MATPRO). Temperature-dependent values from the OFFBEAT port
(`outram-park-fork-offbeat`) would replace these.

```rust
pub const UO2_CP: f64 = 330.0;
```

#### Constant `ZR_CP`

```rust
pub const ZR_CP: f64 = 350.0;
```

#### Constant `UO2_K`

UO2 thermal conductivity, W/(m K), constant. \[A\] about 3 near 1000 K
(MATPRO); replace with the OFFBEAT port's UO2 conductivity.

```rust
pub const UO2_K: f64 = 3.0;
```

#### Constant `GAP_H`

Pellet-clad gap conductance, W/(m^2 K). \[A\] typical closed-gap value.

```rust
pub const GAP_H: f64 = 5000.0;
```

#### Constant `BOILING_H`

Nucleate-boiling heat transfer coefficient from a covered rod to the
water, W/(m^2 K). \[A\] Order of magnitude for pool boiling at decay-heat
flux; at this value a covered rod sits within a few kelvin of T_sat, so
the answer does not hinge on it. A boiling correlation (Chen, in the
GeN-Foam closures) would replace it.

```rust
pub const BOILING_H: f64 = 2.0e4;
```

#### Constant `RING_FACTORS`

Radial power factors of three equal-area rings, centre outwards, mean 1.
\[A\] an illustrative flattened BWR shape.

```rust
pub const RING_FACTORS: [f64; 3] = _;
```

#### Constant `AXIAL_NODES`

Axial nodes over the active length.

```rust
pub const AXIAL_NODES: usize = 12;
```

#### Constant `COSINE_EXTRAPOLATION`

Extrapolation factor of the chopped-cosine axial shape: the cosine's
zeros lie this many active lengths apart. \[A\]

```rust
pub const COSINE_EXTRAPOLATION: f64 = 1.3;
```

#### Constant `RPV_RADIUS_M`

RPV inner radius, m (about 4.8 m ID). \[U\] SAND2012-6173 / INPO Table 7.3-2.

```rust
pub const RPV_RADIUS_M: f64 = 2.4;
```

#### Constant `RPV_HEIGHT_M`

RPV inner height, m. \[U\] about 20 m.

```rust
pub const RPV_HEIGHT_M: f64 = 20.0;
```

#### Constant `RPV_WALL_M`

RPV wall thickness, m, for its heat capacity (`Mechanisms::rpv_wall`).
\[A\] about 6 in, typical of a BWR/3 shell; not in a held source.

```rust
pub const RPV_WALL_M: f64 = 0.16;
```

#### Constant `INTERNALS_ENVELOPE_M2`

The dome internals' envelope facing the RPV wall, m^2, for their
radiation to it (`Mechanisms::internals_radiation`). \[A\] a 4 m tall
cylinder at the vessel's radius with its top: `2 pi r 4 + pi r^2`.

```rust
pub const INTERNALS_ENVELOPE_M2: f64 = 78.4;
```

#### Constant `CONCRETE_DENSITY`

The concrete: MELCOR's standard CORCON basaltic aggregate concrete (CAV
UG Tables 2.1, 2.2): density, kg/m^3, ablation temperature, K, and the
mass fractions of water (evaporable 3.86 % plus chemically bound 2.00
%) and of CO2 (1.50 %). That Unit 1's basemat is of this type is \[A\]
(not in a held source; limestone/common sand, with far more CO2, is
the alternative); the values are MELCOR's defaults, \[P\].

```rust
pub const CONCRETE_DENSITY: f64 = 2340.0;
```

#### Constant `CONCRETE_ABLATION_K`

```rust
pub const CONCRETE_ABLATION_K: f64 = 1450.0;
```

#### Constant `CONCRETE_H2O_FRACTION`

```rust
pub const CONCRETE_H2O_FRACTION: f64 = 0.0586;
```

#### Constant `CONCRETE_CO2_FRACTION`

```rust
pub const CONCRETE_CO2_FRACTION: f64 = 0.0150;
```

#### Constant `CONCRETE_CP`

Concrete's heat capacity, J/(kg K), and its ablation enthalpy, J/kg:
heating from 303 K to the ablation temperature (1.147 MJ/kg) plus
evaporating its free water (0.0386 x 2.257 MJ/kg = 0.087), releasing its
bound water (Ca(OH)2 -> CaO + H2O, 109 kJ/mol: 0.121), its CO2 (CaCO3 ->
CaO + CO2, 178 kJ/mol: 0.061) and melting (0.4 MJ/kg). \[A\] the heat
capacity and the heat of fusion, typical values; the reaction heats are
standard thermochemistry. Derived, not fitted: 1.82 MJ/kg.

```rust
pub const CONCRETE_CP: f64 = 1000.0;
```

#### Constant `CONCRETE_ABLATION_J_PER_KG`

```rust
pub const CONCRETE_ABLATION_J_PER_KG: f64 = _;
```

#### Constant `CAVITY_AREA_M2`

The cavity: the debris's contact area with the floor, m^2 (the
pedestal, about 5.2 m across), and the heat-transfer coefficient from
the debris's interior to its bottom and to its top surface, W/(m^2 K).
\[A\] both. MELCOR's CORCON-Mod3 computes the coefficient from its gas-
film and bubbling correlations (CAV RM §3.3, not held); 1000 is their
order for an oxidic pool stirred by concrete gas.

```rust
pub const CAVITY_AREA_M2: f64 = 21.2;
```

#### Constant `CAVITY_H`

```rust
pub const CAVITY_H: f64 = 1000.0;
```

#### Constant `CAVITY_EMISSIVITY`

Emissivity of the debris surface and of the drywell surroundings it
radiates to: MELCOR's CAV defaults EMISS.OX and EMISS.SUR (0.9; CAV UG,
the 2.0 defaults). \[P\]

```rust
pub const CAVITY_EMISSIVITY: f64 = 0.9;
```

#### Constant `ZR_CO2_HEAT_J_PER_KG`

Heat of Zr + 2 CO2 -> ZrO2 + 2 CO, J per kg of Zr: -(-1100.6 + 2 x
(-110.53) - 2 x (-393.51)) kJ/mol over 0.091224 kg/mol (NIST standard
enthalpies of formation at 298 K). \[P\]

```rust
pub const ZR_CO2_HEAT_J_PER_KG: f64 = _;
```

#### Constant `BAF_M`

Bottom of active fuel above the vessel bottom, m. \[A\]

```rust
pub const BAF_M: f64 = 5.0;
```

#### Constant `LOWER_PLENUM_FREE`

Fraction of the vessel's cross-section free for water below the core
(control-rod guide tubes, the bottom head's curvature) and above it
(separators, dryers). \[A\]

```rust
pub const LOWER_PLENUM_FREE: f64 = 0.75;
```

#### Constant `UPPER_FREE`

```rust
pub const UPPER_FREE: f64 = 0.85;
```

#### Constant `CORE_OTHER_AREA_M2`

Cross-section taken in the core region by the shroud, jet pumps and
control blades besides the fuel rods and channels, m^2. \[A\]

```rust
pub const CORE_OTHER_AREA_M2: f64 = 1.0;
```

#### Constant `NORMAL_LEVEL_ABOVE_TAF_M`

Normal water level above the top of active fuel, m. \[U\]

```rust
pub const NORMAL_LEVEL_ABOVE_TAF_M: f64 = 4.4;
```

#### Constant `INITIAL_PRESSURE_PA`

Steam-dome pressure at the scram, Pa absolute. \[P\] about 7 MPa
(6.93 MPa gauge rated).

```rust
pub const INITIAL_PRESSURE_PA: f64 = 7.03e6;
```

#### Constant `MSIV_CLOSURE_S`

Main steam isolation valves close this long after the scram, s; until
then the pressure regulator keeps passing steam to the turbine bypass and
condenser, holding the dome at its initial pressure. \[U\] INPO 11-005:
MSIVs closed at about 14:47 on the loss of off-site power.

```rust
pub const MSIV_CLOSURE_S: f64 = 60.0;
```

#### Constant `BYPASS_GAIN_KG_S_PA`

Pressure-regulator gain while the bypass is open, kg/s per Pa above the
initial pressure. \[A\] (any gain that holds the pressure within a few kPa).

```rust
pub const BYPASS_GAIN_KG_S_PA: f64 = 1.0e-3;
```

#### Constant `RELIEF_VALVES`

Relief valves: Unit 1 has electromatic relief valves plus spring safety
valves. \[U\] count, set points and throat. While DC power lasts the relief
valves lift at the relief set point; after the station blackout only the
spring safety valves act, at their higher set point.

```rust
pub const RELIEF_VALVES: f64 = 4.0;
```

#### Constant `RELIEF_OPEN_PA`

```rust
pub const RELIEF_OPEN_PA: f64 = _;
```

#### Constant `RELIEF_CLOSE_PA`

```rust
pub const RELIEF_CLOSE_PA: f64 = _;
```

#### Constant `SAFETY_OPEN_PA`

```rust
pub const SAFETY_OPEN_PA: f64 = 8.3e6;
```

#### Constant `SAFETY_CLOSE_PA`

```rust
pub const SAFETY_CLOSE_PA: f64 = 7.9e6;
```

#### Constant `RELIEF_THROAT_M2`

Throat area of one relief valve, m^2. \[A\] about 87 kg/s of steam each at
7.4 MPa through the steam tables' choked-flow solution.

```rust
pub const RELIEF_THROAT_M2: f64 = 0.0081;
```

#### Constant `IC_TRAINS`

Isolation condenser trains (A and B). \[P\] TEPCO: two.

```rust
pub const IC_TRAINS: usize = 2;
```

#### Constant `IC_AUTOSTART_PA`

Auto-start on high RPV pressure, Pa absolute. \[P\] TEPCO: 7.13 MPa gauge
for 15 s.

```rust
pub const IC_AUTOSTART_PA: f64 = _;
```

#### Constant `IC_UA_W_PER_K`

Heat removal of one IC train: `UA (T_sat - T_shell)`, W/K. \[A\]
**Placeholder**: about 43 MW per train at 7 MPa against 100 degC shell
water. The per-train rating was not found in open sources; SAND2012-6173
or the IAEA Technical Volume 1 would replace it.

```rust
pub const IC_UA_W_PER_K: f64 = 2.3e5;
```

#### Constant `IC_SHELL_WATER_KG`

Shell-side water per train, kg. \[A\] placeholder, about 100 m^3 each
(open sources quote about 8 h of inventory for both trains).

```rust
pub const IC_SHELL_WATER_KG: f64 = 1.0e5;
```

#### Constant `FIRE_ENGINE_SHUTOFF_PA`

Fire-engine injection (fresh water, later seawater): shut-off pressure,
Pa absolute, and flow against atmospheric back-pressure, kg/s. \[A\] A pump
curve `W = W_max sqrt(1 - (p - p_atm)/(p_shut - p_atm))`.

```rust
pub const FIRE_ENGINE_SHUTOFF_PA: f64 = 1.0e6;
```

#### Constant `FIRE_ENGINE_MAX_KG_S`

```rust
pub const FIRE_ENGINE_MAX_KG_S: f64 = 4.0;
```

#### Constant `INJECTION_TEMPERATURE_K`

```rust
pub const INJECTION_TEMPERATURE_K: f64 = 293.15;
```

#### Constant `MSL_LINES`

Main steam lines (gh:#1037, `msl.rs`): number, inner radius, wall
thickness, m. \[A\] all three: Unit 1's line size is not in a held
source. The radius is sized for its rated steam flow, about 667 kg/s
over two lines at about 45 m/s and 36.5 kg/m^3 (inner diameter about
0.51 m). The wall is the ASME thickness `p D / (2 S)` at 8.6 MPa design
pressure and S about 120 MPa (18 mm), plus allowances.

```rust
pub const MSL_LINES: f64 = 2.0;
```

#### Constant `MSL_INNER_RADIUS_M`

```rust
pub const MSL_INNER_RADIUS_M: f64 = 0.254;
```

#### Constant `MSL_WALL_M`

```rust
pub const MSL_WALL_M: f64 = 0.025;
```

#### Constant `MSL_INSULATION_H`

Insulation conductance to the drywell, W/(m^2 K). \[A\] about 0.05 W/(m
K) through 0.1 m of insulation is 0.5; 1 is used.

```rust
pub const MSL_INSULATION_H: f64 = 1.0;
```

#### Constant `MSL_LM_A1`

Larson-Miller coefficients of the line's steel: MELCOR's default for
A-508, "class 2 carbon steel" (Vol. 1, SAND2017-0455 O, CF Table 1;
stress in Pa, T in K, time in s): a1 K, b1 K, C. The coefficients are
\[P\]; the choice of A-508 for Unit 1's line is an assumption (its grade
is not in a held source).

```rust
pub const MSL_LM_A1: f64 = -5335.0;
```

#### Constant `MSL_LM_B1`

```rust
pub const MSL_LM_B1: f64 = 62291.3;
```

#### Constant `MSL_LM_C`

```rust
pub const MSL_LM_C: f64 = 16.44;
```

#### Constant `INTERNALS_KG`

The steam dome's internals as one heat structure (gh:#1037,
`internals.rs`): steel mass, kg, wetted area, m^2, and gas-side heat
transfer coefficient, W/(m^2 K). \[A\] all three: Unit 1's internals are
not in a held source. The mass is the dryer, separators and shroud head
scaled from public BWR/4 dryer masses (about 45 t) to a 460 MWe plant;
the area is that steel at 3 mm (dryer vanes) to 10 mm (separators),
both faces; h is low-velocity forced convection.

```rust
pub const INTERNALS_KG: f64 = 5.0e4;
```

#### Constant `INTERNALS_AREA_M2`

```rust
pub const INTERNALS_AREA_M2: f64 = 2500.0;
```

#### Constant `INTERNALS_H`

```rust
pub const INTERNALS_H: f64 = 10.0;
```

#### Constant `INTERNALS_LOSS_H`

The internals' natural-convection coefficient to the saturated dome when
no gas flows past them, W/(m^2 K). \[A\] an order of magnitude for
natural convection in steam (time constant about 2000 s with the mass
and area above).

```rust
pub const INTERNALS_LOSS_H: f64 = 5.0;
```

#### Constant `CS_INVENTORY_KG`

Unit 1's cesium inventory, kg: the BSAF Phase II benchmark specification's
value (OECD/NEA No. 7525, 2021, p.20-21: "154.0 kg" Cs and 11.98 kg I,
"specified by the benchmark co-ordinator"). \[P\]

```rust
pub const CS_INVENTORY_KG: f64 = 154.0;
```

#### Constant `I_INVENTORY_KG`

Unit 1's iodine inventory, kg (the same source: "11.98 kg"). \[P\]

```rust
pub const I_INVENTORY_KG: f64 = 11.98;
```

#### Constant `TE_INVENTORY_KG`

Unit 1's tellurium inventory, kg. \[A\] derived, no published value
held: the ENDF/B-VIII summation's Te/Cs mass ratio at the scram
(`fp_decay`, 9.508/95.008 kg) times the specification's 154.0 kg Cs.
The summation's own Cs is low (95 kg) because MELCOR's default 584-day
history is shorter than Unit 1's fuel residence; the ratio is the
part used.

```rust
pub const TE_INVENTORY_KG: f64 = _;
```

#### Constant `DW_FLOOR_M2`

Deposition areas not otherwise in the model: the drywell floor, m^2,
and the torus pool's free surface, m^2. \[A\] both: not in a held source.
The floor is about a third of a 3410 m^3 light-bulb drywell's footprint;
the pool surface is a torus of about 15 m major radius filled about 40 %
(the pool's 1750 of 4370 m^3), so about 8 m wide.

```rust
pub const DW_FLOOR_M2: f64 = 200.0;
```

#### Constant `WW_POOL_SURFACE_M2`

```rust
pub const WW_POOL_SURFACE_M2: f64 = 750.0;
```

#### Constant `CORSOR_K0_PER_MIN`

CORSOR-M release of the volatile fission products (Xe, Cs, I), MELCOR's
default row for those classes (Vol. 2, SAND2023-10994 O, RN §2.3.1,
C7102): `k0`, per minute, and `Q`, kcal/mol. \[P\]

```rust
pub const CORSOR_K0_PER_MIN: f64 = 2.0e5;
```

#### Constant `CORSOR_Q_KCAL_PER_MOL`

```rust
pub const CORSOR_Q_KCAL_PER_MOL: f64 = 63.8;
```

#### Constant `CORSOR_UO2_K0_PER_MIN`

CORSOR-M's row for La and UO2 (same source): `k0`, per minute, `Q`,
kcal/mol. \[P\]

```rust
pub const CORSOR_UO2_K0_PER_MIN: f64 = 1.46e7;
```

#### Constant `CORSOR_UO2_Q_KCAL_PER_MOL`

```rust
pub const CORSOR_UO2_Q_KCAL_PER_MOL: f64 = 143.1;
```

#### Constant `HOT_LEAK_GAS_K`

RPV leak to the drywell once the core-exit gas is hot (instrument lines,
SRV gaskets; the BSAF Unit 1 analyses needed such a path). \[A\] Opens above
[`HOT_LEAK_GAS_K`] with throat [`HOT_LEAK_M2`].

```rust
pub const HOT_LEAK_GAS_K: f64 = 1000.0;
```

#### Constant `HOT_LEAK_M2`

```rust
pub const HOT_LEAK_M2: f64 = 3.0e-4;
```

#### Constant `LOWER_HEAD_FAILURE_K`

Lower-head failure in the `without_lower_head_wall` ablation: debris in
the dry lower plenum above this, K (a debris temperature, not MELCOR's
wall-node TPFAIL; see `HEAD_FAILURE_WALL_K`), opens a breach of this
throat, m^2. \[A\] both. MELCOR's default opening is 0.1 m diameter,
7.9e-3 m^2 (MELCOR 2.2 Vol. 2, COR §6.3).

```rust
pub const LOWER_HEAD_FAILURE_K: f64 = 1500.0;
```

#### Constant `BREACH_M2`

```rust
pub const BREACH_M2: f64 = 0.01;
```

#### Constant `LOWER_HEAD_THICKNESS_M`

Lower-head wall thickness, m. \[A\] MELCOR's generic BWR default DZLH
(Vol. 1, COR_VP table), which is for a 3.34 m vessel; Unit 1's vessel is
2.4 m, and its own value needs a plant source (plan L4).

```rust
pub const LOWER_HEAD_THICKNESS_M: f64 = 0.22;
```

#### Constant `DEBRIS_TO_HEAD_W_M2_K`

Debris-to-wall heat transfer coefficient, W/(m^2 K). \[A\] MELCOR's
default HDBLH (Vol. 1, COR_LHF), which its manual calls an
order-of-magnitude value for sensitivity studies.

```rust
pub const DEBRIS_TO_HEAD_W_M2_K: f64 = 1000.0;
```

#### Constant `PENETRATION_WALL_M`

Wall thickness of a lower-head penetration (instrument or control-rod
drive housing), m: its steel per unit heated area. \[A\] Unit 1's
housings are not in a held source. The debris heats it with
`debris_to_head_w_m2_k` (MELCOR's HDBPN default is the same 1000
W/(m^2 K)), and it fails at `head_failure_wall_k` (MELCOR's TPFAIL
applies to penetrations and the inner wall node alike).

```rust
pub const PENETRATION_WALL_M: f64 = 0.01;
```

#### Constant `HEAD_FAILURE_WALL_K`

Inner-wall temperature that fails the head, K. \[A\] MELCOR's default
TPFAIL (Vol. 1, COR_LHF), described as roughly where steel turns plastic.

```rust
pub const HEAD_FAILURE_WALL_K: f64 = 1273.15;
```

#### Constant `LM_P0`

Larson-Miller creep rupture of the head, SA533B1 (NUREG/CR-5582, as MELCOR
Vol. 2, COR §6.3, Eqs 6-23 to 6-25 and C1601 give it): time to rupture
`10^(P/T - C)` s with `P = P0 - slope log10(sigma [Pa])`; plastic strain
grows by `fail_strain dt / t_R` and the head fails at `fail_strain`.
\[P\] V2's slope, 4725 (Vol. 1's control-function table prints 4730).

```rust
pub const LM_P0: f64 = 4.812e4;
```

#### Constant `LM_SLOPE`

```rust
pub const LM_SLOPE: f64 = 4.725e3;
```

#### Constant `LM_C`

```rust
pub const LM_C: f64 = 7.042;
```

#### Constant `CREEP_FAIL_STRAIN`

```rust
pub const CREEP_FAIL_STRAIN: f64 = 0.18;
```

#### Constant `DEBRIS_POROSITY`

Porosity of the debris bed in the lower head, which sets how high it
stands and so the wall area it touches. \[A\]

```rust
pub const DEBRIS_POROSITY: f64 = 0.4;
```

#### Constant `DEBRIS_PARTICLE_M`

Particulate debris diameter, m, for its oxidation area `6 V / D`
(MELCOR COR §3.2.1, Eq 3-28). \[A\] MELCOR has no default (COR_EDR
DHYPD is required input); 1 cm, the order of a pellet.

```rust
pub const DEBRIS_PARTICLE_M: f64 = 0.01;
```

#### Constant `LP_BOTTOM_LEVEL_M`

Height of the lower plenum's bottom axial level, m: the penetrations'
height over which MELCOR scales the debris contact (`dz_d / dz_1`, COR
§6.1). \[A\] no Unit 1 nodalization held; half a metre, the order of a
lower-plenum cell.

```rust
pub const LP_BOTTOM_LEVEL_M: f64 = 0.5;
```

#### Constant `BREACH_DIAMETER_M`

Diameter of the lower head's failure opening, m, through which a failed
vessel's water drains (`failed_vessel_drain`). \[A\] MELCOR has no
default (COR_PEN DFLPN, and the lower-plenum flow path, are user input);
0.1 m, a few failed penetrations.

```rust
pub const BREACH_DIAMETER_M: f64 = 0.1;
```

#### Constant `DEBRIS_QUENCH_S`

Time over which debris in a wet lower plenum gives up its stored heat,
s. \[A\] (quench of a debris bed takes minutes).

```rust
pub const DEBRIS_QUENCH_S: f64 = 120.0;
```

#### Constant `QUENCH_H`

Falling-debris quench heat-transfer coefficient, W/(m^2 K). \[P\]
MELCOR's default (Vol. 1, COR_LP HDBH2O, 100), used over the particle
area (`debris_quench`).

```rust
pub const QUENCH_H: f64 = 100.0;
```

#### Constant `QUENCH_TAU_S`

Time constant of the quench decay factor, s. \[P\] MELCOR's radial
spreading time for solid debris (Vol. 2, COR §2.3.8 and §3.2.5: "an ad
hoc default value of 360 s", C1020(1)). The coefficient table lists
§3.2.5 against C1020(2) = 60 s; the text is followed (stated).

```rust
pub const QUENCH_TAU_S: f64 = 360.0;
```

#### Constant `ZR_MELT_K`

Zircaloy relocation in the `without_rod_collapse` ablation: a node whose
cladding reaches this, K, slumps whole to the lower plenum. \[U\]
Zircaloy melts at 2098 K (MELCOR 2.2 Vol. 2, MP Table 7, citing MATPRO),
but MELCOR does not relocate anything at it (`CLAD_BREACH_K`).

```rust
pub const ZR_MELT_K: f64 = 2100.0;
```

#### Constant `CLAD_BREACH_K`

Cladding temperature at which molten Zr breaches its oxide shell, K.
\[A\] MELCOR's default for cladding (C1131(2)).

```rust
pub const CLAD_BREACH_K: f64 = 2400.0;
```

#### Constant `MIN_METAL_M`

Unoxidised metal below which a rod counts as oxidised, m. \[A\]
MELCOR's default (DRZRMN).

```rust
pub const MIN_METAL_M: f64 = 1.0e-4;
```

#### Constant `ROD_COLLAPSE_K`

An oxidised rod collapses at this fuel temperature, K, and any rod at
the second. \[A\] MELCOR's defaults TRDFAI (2800 K in 1.8.4, lowered on
Phebus evidence for irradiated fuel) and TRDMAX (C1132(1..2)).

```rust
pub const ROD_COLLAPSE_K: f64 = 2500.0;
```

#### Constant `ROD_COLLAPSE_MAX_K`

```rust
pub const ROD_COLLAPSE_MAX_K: f64 = 3100.0;
```

#### Constant `DAMAGE_A`

Time-at-temperature damage of an oxidised rod: lifetime
`1/L = A exp(B T)`, failure when `sum dt/L` reaches 1. \[P\] MELCOR's
50th-percentile fit to VERCORS and the SOARCA uncertainty elicitation,
A in 1/s, B in 1/K (Vol. 2, Eq. 3-29).

```rust
pub const DAMAGE_A: f64 = 2.16e-11;
```

#### Constant `DAMAGE_B`

```rust
pub const DAMAGE_B: f64 = 7.0e-3;
```

#### Constant `ASSEMBLY_PITCH_M`

Fuel-assembly pitch, m: sets the core's equivalent radius, and so the ring
boundaries' areas. \[U\] GE BWR/3-4 lattice, 6 in.

```rust
pub const ASSEMBLY_PITCH_M: f64 = 0.1524;
```

#### Constant `RADIAL_EXCHANGE`

Radial (cell-to-cell, and outer ring to shroud) and axial radiative
exchange factors. \[A\] MELCOR's 2.0 defaults FCELR and FCELA (Vol. 1,
COR_RF; 0.25 in its 1.86 scheme).

```rust
pub const RADIAL_EXCHANGE: f64 = 0.1;
```

#### Constant `AXIAL_EXCHANGE`

```rust
pub const AXIAL_EXCHANGE: f64 = 0.1;
```

#### Constant `SHROUD_THICKNESS_M`

Core shroud wall thickness, m. \[A\] MELCOR's manual describes the BWR
shroud as "about 5 cm" (Vol. 2, COR §2.2.12); Unit 1's own value needs L4.

```rust
pub const SHROUD_THICKNESS_M: f64 = 0.05;
```

#### Constant `ZRO2_DENSITY`

ZrO2 density, kg/m^3, for the oxide thickness that sets the cladding's
emissivity. \[P\] MELCOR's MP default (Vol. 2, MP Table 6.1).

```rust
pub const ZRO2_DENSITY: f64 = 5600.0;
```

#### Constant `BUBBLE_RISE_M_S`

Bubble rise velocity in the pool, m/s, and the largest mean pool void.
\[A\] MELCOR's defaults C4407(1) and C4407(11); its manual says other
assumptions change the answer by about a factor of 2, and calls 0.4
roughly the upper limit of bubbly flow.

```rust
pub const BUBBLE_RISE_M_S: f64 = 0.3;
```

#### Constant `MAX_POOL_VOID`

```rust
pub const MAX_POOL_VOID: f64 = 0.4;
```

#### Constant `FISSION_SHARE_U235`

Shares of the rated power from U-235 thermal and Pu-239 thermal fission
(U-238 fast is the rest). \[A\] MELCOR's BWR defaults, 2.316e9,
1.1114e9 and 1.506e8 W of 3578 MW, from a Sandia-ORIGEN equilibrium cycle
(Vol. 1, DCH_FPW); Unit 1's own cycle needs a plant source (plan L4).

```rust
pub const FISSION_SHARE_U235: f64 = _;
```

#### Constant `FISSION_SHARE_PU239`

```rust
pub const FISSION_SHARE_PU239: f64 = _;
```

#### Constant `OPERATING_TIME_S`

Irradiation time at constant power, s. \[A\] MELCOR's default, 584 days
(2 years at 80 % capacity; Vol. 1, DCH_OPT).

```rust
pub const OPERATING_TIME_S: f64 = 5.05e7;
```

#### Constant `FISSIONS_PER_FISSILE_ATOM`

Fissions per initial fissile atom, for the capture correction. \[A\]
MELCOR's default from Sandia-ORIGEN (Vol. 1, DCH_NFA).

```rust
pub const FISSIONS_PER_FISSILE_ATOM: f64 = 0.713;
```

#### Constant `Q_U235_MEV`

Energy per fission, MeV, fission plus decay. \[A\] MELCOR's defaults
from Sandia-ORIGEN (Vol. 1, 3201).

```rust
pub const Q_U235_MEV: f64 = 199.0;
```

#### Constant `Q_PU239_MEV`

```rust
pub const Q_PU239_MEV: f64 = 210.2;
```

#### Constant `Q_U238_MEV`

```rust
pub const Q_U238_MEV: f64 = 199.3;
```

#### Constant `U239_PER_FISSION`

U-239 atoms made per fission at shutdown. \[A\] MELCOR's default, fitted
so the standard's equation matches ORIGEN (Vol. 1, 3205(1)).

```rust
pub const U239_PER_FISSION: f64 = 0.526;
```

#### Constant `E_U239_MEV`

Mean decay energy of U-239 and Np-239, MeV, and their decay constants,
1/s. \[P\] the ANS-5.1-1979 standard's values (page 5), as MELCOR's
defaults give them (Vol. 1, 3205(2..5)).

```rust
pub const E_U239_MEV: f64 = 0.474;
```

#### Constant `E_NP239_MEV`

```rust
pub const E_NP239_MEV: f64 = 0.419;
```

#### Constant `U239_DECAY_PER_S`

```rust
pub const U239_DECAY_PER_S: f64 = 4.91e-4;
```

#### Constant `NP239_DECAY_PER_S`

```rust
pub const NP239_DECAY_PER_S: f64 = 3.41e-6;
```

#### Constant `UH_LOW_A`

```rust
pub const UH_LOW_A: f64 = 29.6;
```

#### Constant `UH_LOW_B_K`

```rust
pub const UH_LOW_B_K: f64 = 16_820.0;
```

#### Constant `UH_HIGH_A`

```rust
pub const UH_HIGH_A: f64 = 87.9;
```

#### Constant `UH_HIGH_B_K`

```rust
pub const UH_HIGH_B_K: f64 = 16_610.0;
```

#### Constant `UH_LOW_MAX_K`

```rust
pub const UH_LOW_MAX_K: f64 = 1853.0;
```

#### Constant `UH_HIGH_MIN_K`

```rust
pub const UH_HIGH_MIN_K: f64 = 1873.0;
```

#### Constant `OXIDATION_MIN_K`

No oxidation below this temperature, K. ~~Below this the oxidation is
negligible on the accident's time scale. \[A\] 1000 K~~ **CORRECTED
2026-10-29**: MELCOR's TOXMIN for steam, 1100 K (Vol. 1, SAND2017-0455
O, C1004(1); Vol. 2, COR §2.5.4), which is also the lower end of the
Urbanic-Heidrick low branch's stated range (1100-1853 K, Table 2-6), so
the rate is no longer extrapolated below its fit. \[P\]

```rust
pub const OXIDATION_MIN_K: f64 = 1100.0;
```

#### Constant `H2O_H2_DIFFUSIVITY_A`

Binary diffusion coefficient of steam in hydrogen, `D = a T^b / p`
(m^2/s, T in K, p in Pa): MELCOR 2.2 Vol. 2 (SAND2023-10994 O), NCG/H2O
package §6.1, Eq 6-2, which takes it from the MARCH 2 code's reference.
At 1000 K and 1 atm it gives 7.2e-4 m^2/s. \[P\]

```rust
pub const H2O_H2_DIFFUSIVITY_A: f64 = 6.60639e-4;
```

#### Constant `H2O_H2_DIFFUSIVITY_B`

```rust
pub const H2O_H2_DIFFUSIVITY_B: f64 = 1.68;
```

#### Constant `OXIDATION_HEAT_J_PER_KG_ZR`

Zr + 2 H2O(g) -> ZrO2 + 2 H2: heat released per kg Zr, J/kg.
~~\[U\] about 586 kJ/mol, 6.45 MJ/kg Zr~~ (`6.45e6`; also inconsistent with
itself: 586 kJ/mol / 91.224 g/mol is 6.42 MJ/kg). **CORRECTED 2026-10-29
(gh:#1038):** OFFBEAT's
`corrosion::exotherm::ENERGY_PER_KG_ZIRCONIUM`, reused, derived from the
standard formation enthalpies of monoclinic ZrO2 (-1100.6 kJ/mol) and
water vapour (-241.826 kJ/mol) at 298.15 K: 616.95 kJ/mol, **6.763 MJ/kg**.
\[P\] for the formation enthalpies; the derivation is that module's, not
fitted to anything. Not modelled: the drift of the reaction enthalpy with
temperature (Kirchhoff), which that module puts at a few per cent over
300-2000 K (not measured here).

```rust
pub const OXIDATION_HEAT_J_PER_KG_ZR: f64 = outram_park_fork_offbeat::corrosion::exotherm::ENERGY_PER_KG_ZIRCONIUM;
```

#### Constant `M_ZR`

Molar masses, kg/mol.

```rust
pub const M_ZR: f64 = 0.091_224;
```

#### Constant `M_H2O`

```rust
pub const M_H2O: f64 = 0.018_015;
```

#### Constant `M_H2`

```rust
pub const M_H2: f64 = 0.002_016;
```

#### Constant `M_N2`

```rust
pub const M_N2: f64 = 0.028_014;
```

#### Constant `R_GAS`

```rust
pub const R_GAS: f64 = 8.314_462;
```

#### Constant `DW_WALL_AREA_M2`

Drywell structures (steel shell, concrete, the pedestal) as one lumped
heat sink: area, m^2, convective coefficient, W/(m^2 K), and heat
capacity, J/K. \[A\] all three placeholders, moved here from
`containment.rs` 2026-10-29 unchanged; the real heat-sink inventory is in
SAND2012-6173 (not held).
Since 2026-10-29 the coefficient is used only by the
`without_dw_wall_hs` ablation: the default computes it (MELCOR's
heat-structure surface).

```rust
pub const DW_WALL_AREA_M2: f64 = 4000.0;
```

#### Constant `DW_WALL_H`

```rust
pub const DW_WALL_H: f64 = 20.0;
```

#### Constant `DW_WALL_C`

```rust
pub const DW_WALL_C: f64 = 5.0e8;
```

#### Constant `DW_WALL_HEIGHT_M`

Height of the drywell structures, m: the characteristic length of their
natural convection (`Mechanisms::dw_wall_hs`). \[A\] about the height
of a light-bulb drywell's cylinder; in the turbulent regime that applies
here it cancels.

```rust
pub const DW_WALL_HEIGHT_M: f64 = 20.0;
```

#### Constant `DW_GAP_M`

The drywell structures' outer heat path (`Mechanisms::dw_wall_outer`):
the gap between the steel shell and the concrete around it, m, and the
emissivity of its two faces; the concrete's thickness, m, and
conductivity, W/(m K); and the coefficient from its outer face to the
reactor building's air, W/(m^2 K). \[A\] all five: a Mark I shell
stands a few centimetres off its concrete; oxidised steel and concrete
near 0.8; 1.5 m of biological shield; ordinary concrete's
conductivity; natural convection to still air.

```rust
pub const DW_GAP_M: f64 = 0.05;
```

#### Constant `DW_GAP_EMISSIVITY`

```rust
pub const DW_GAP_EMISSIVITY: f64 = 0.8;
```

#### Constant `DW_CONCRETE_M`

```rust
pub const DW_CONCRETE_M: f64 = 1.5;
```

#### Constant `CONCRETE_K`

```rust
pub const CONCRETE_K: f64 = 1.4;
```

#### Constant `RB_WALL_H`

```rust
pub const RB_WALL_H: f64 = 5.0;
```

#### Constant `WW_SHELL_AREA_M2`

The torus shell above the pool (`Mechanisms::ww_shell`): area, m^2,
heat capacity, J/K, and height, m. \[A\] all three, derived from the
same assumed torus as `WW_POOL_SURFACE_M2` (15 m major radius, 4.15 m
minor, 40 % full by volume, so 55 % of its 2458 m^2 above the water), a
15 mm carbon-steel shell (7850 kg/m^3, 500 J/(kg K)), and the gas
space's height; the height cancels in the turbulent regime.

```rust
pub const WW_SHELL_AREA_M2: f64 = 1353.0;
```

#### Constant `WW_SHELL_C`

```rust
pub const WW_SHELL_C: f64 = 7.96e7;
```

#### Constant `WW_SHELL_HEIGHT_M`

```rust
pub const WW_SHELL_HEIGHT_M: f64 = 5.0;
```

#### Constant `DRYWELL_VOLUME_M3`

Drywell free volume, m^3. \[U\] about 3410 (KTH slides, simplyinfo).

```rust
pub const DRYWELL_VOLUME_M3: f64 = 3410.0;
```

#### Constant `WETWELL_GAS_M3`

Wetwell (torus) gas space, m^3, and pool water, m^3. \[P\] Japanese
government report to the IAEA, June 2011, attachment 4: 2620 and 1750.

```rust
pub const WETWELL_GAS_M3: f64 = 2620.0;
```

#### Constant `POOL_WATER_M3`

```rust
pub const POOL_WATER_M3: f64 = 1750.0;
```

#### Constant `CONTAINMENT_DESIGN_PA`

Primary containment design pressure, Pa absolute. \[P\] 0.43 MPa gauge
(NISA, via WNA).

```rust
pub const CONTAINMENT_DESIGN_PA: f64 = _;
```

#### Constant `INITIAL_CONTAINMENT_PA`

Initial containment state: inerted with nitrogen. \[A\] pressure, Pa
absolute, and temperatures, K.

```rust
pub const INITIAL_CONTAINMENT_PA: f64 = 1.05e5;
```

#### Constant `INITIAL_DRYWELL_K`

```rust
pub const INITIAL_DRYWELL_K: f64 = 323.15;
```

#### Constant `INITIAL_POOL_K`

```rust
pub const INITIAL_POOL_K: f64 = 303.15;
```

#### Constant `VENT_SUBMERGENCE_PA`

Drywell to wetwell downcomers: submergence head, Pa (about 1.2 m of
water), total flow area, m^2, and discharge coefficient. \[A\]

```rust
pub const VENT_SUBMERGENCE_PA: f64 = 11_800.0;
```

#### Constant `DOWNCOMER_AREA_M2`

```rust
pub const DOWNCOMER_AREA_M2: f64 = 20.0;
```

#### Constant `DOWNCOMER_COUNT`

Number of downcomers sharing that area. \[A\] the Mark I layout of 96
(not found for Unit 1 in the held sources; each is then 0.515 m across).
Sets each downcomer's flow and diameter in the swarm-rise scrubbing
(`sparc`).

```rust
pub const DOWNCOMER_COUNT: f64 = 96.0;
```

#### Constant `DISCHARGE_COEFFICIENT`

```rust
pub const DISCHARGE_COEFFICIENT: f64 = 0.6;
```

#### Constant `VACUUM_BREAKER_PA`

Wetwell-to-drywell vacuum breakers: opening differential, Pa, and area,
m^2. \[A\]

```rust
pub const VACUUM_BREAKER_PA: f64 = 3400.0;
```

#### Constant `VACUUM_BREAKER_M2`

```rust
pub const VACUUM_BREAKER_M2: f64 = 0.5;
```

#### Constant `VENT_RUPTURE_DISK_PA`

Wetwell vent line to the stack: rupture-disk burst pressure, Pa absolute,
and flow area, m^2. \[U\] burst pressure; \[A\] area.

```rust
pub const VENT_RUPTURE_DISK_PA: f64 = 0.549e6;
```

#### Constant `VENT_AREA_M2`

```rust
pub const VENT_AREA_M2: f64 = 0.07;
```

#### Constant `FLANGE_LEAK_PA`

Drywell head-flange leak to the reactor building at high pressure (the
leading explanation for Unit 1's hydrogen in the building). \[A\] opens
above this pressure, Pa absolute, with this area, m^2.

```rust
pub const FLANGE_LEAK_PA: f64 = 0.75e6;
```

#### Constant `FLANGE_LEAK_M2`

```rust
pub const FLANGE_LEAK_M2: f64 = 5.0e-4;
```

#### Constant `REACTOR_BUILDING_M3`

Reactor building free volume, m^3. \[A\] placeholder (about 38,000 is
quoted for Unit 3).

```rust
pub const REACTOR_BUILDING_M3: f64 = 26_000.0;
```

#### Constant `H2_FLAMMABLE`

Hydrogen in air: lower flammability limit (upward propagation).
\[P\] 4 % (Shapiro diagram).

```rust
pub const H2_FLAMMABLE: f64 = 0.04;
```

#### Constant `H2_EXPLOSION`

Hydrogen mole fraction above which a deflagration is strong enough to
wreck the building's upper floor. \[A\] 8 %.

```rust
pub const H2_EXPLOSION: f64 = 0.08;
```

#### Constant `P_ATM`

Atmosphere, Pa.

```rust
pub const P_ATM: f64 = 1.013e5;
```

## Module `pool_surface`

Mass and energy transfer at the surface of the suppression pool, between
the pool and the wetwell atmosphere (2026-10-29, gh:#1047 follow-up, the
`ww_atmosphere` mechanism). AI draft, clean room: written from the
MELCOR 2.2 reference manuals' equations, never from MELCOR source.

- **The interface** (CVH/FL RM §5.1.1-5.1.2, Eqs 5-1 to 5-23): the
  surface temperature `T_l` satisfies, at once, the diffusion of water
  vapour through the non-condensables (Eq 5-1, the coefficient `C` from
  the heat-mass analogy, Eq 5-2), the energy balance at the surface (Eq
  5-7, the pool's and the atmosphere's heat flows by natural convection
  plus radiation, Eqs 5-9, 5-10, 5-18 to 5-25) and saturation at `T_l`
  (Eq 5-5). Solved by bisection on `T_l`, as MELCOR does ("bound and
  bisect"). With no non-condensables the surface is at the saturation
  temperature of the total pressure (Eq 5-6).
- **The bubbles** entering the pool at a vent (CVH/FL RM §6.1, Eqs 6-1 to
  6-3): an efficiency `epsilon = epsilon_z epsilon_T`, its full result the
  non-condensables leaving at the pool's temperature with water vapour at
  0.99 of its saturation pressure, interpolated linearly to no
  interaction at `epsilon = 0`.
- **Properties** (NCG/H2O RM §4-6): MELCOR's default tables for steam,
  water and hydrogen, the power-law fits for nitrogen, Eucken's
  correlation for hydrogen's conductivity, Wilke's mixing rule for
  viscosity and Wassijewa's with Mason and Saxena's `A_ij` for
  conductivity, and the steam-air and steam-hydrogen diffusivities
  (Eqs 6-1, 6-2; nitrogen takes air's, stated).

**Simplifications, stated:** no forced convection (MELCOR's Eq 5-21
needs the volume's velocity, which this model does not have); the pool's
heat capacity is the model's constant `c_p`; the pool's expansion
coefficient is IAPWS-IF97's.

```rust
pub mod pool_surface { /* ... */ }
```

### Types

#### Struct `Atmosphere`

The wetwell atmosphere's bulk properties.

```rust
pub struct Atmosphere {
    pub t_k: f64,
    pub p_pa: f64,
    pub n_steam: f64,
    pub n_n2: f64,
    pub n_h2: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `t_k` | `f64` |  |
| `p_pa` | `f64` |  |
| `n_steam` | `f64` | Moles of steam, nitrogen and hydrogen. |
| `n_n2` | `f64` |  |
| `n_h2` | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn viscosity_and_conductivity(self: &Self) -> (f64, f64) { /* ... */ }
  ```
  Mixture viscosity, Pa s (Wilke, Eq 5-2) and conductivity, W/(m K)

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Pool`

The pool's bulk state.

```rust
pub struct Pool {
    pub t_k: f64,
    pub kg: f64,
    pub rho: f64,
    pub cp: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `t_k` | `f64` |  |
| `kg` | `f64` |  |
| `rho` | `f64` |  |
| `cp` | `f64` |  |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Surface`

Geometry of the surface: its area, m^2, and the depths of the pool and
of the gas space above it, m (each its volume over the area).

```rust
pub struct Surface {
    pub area_m2: f64,
    pub pool_depth_m: f64,
    pub gas_depth_m: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `area_m2` | `f64` |  |
| `pool_depth_m` | `f64` |  |
| `gas_depth_m` | `f64` |  |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Exchange`

The surface's result for one step: evaporation, kg/s (negative is
condensation), and the heat flows from the pool and from the atmosphere
to the surface, W.

```rust
pub struct Exchange {
    pub evaporation_kg_s: f64,
    pub q_pool_w: f64,
    pub q_atm_w: f64,
    pub t_surface_k: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `evaporation_kg_s` | `f64` |  |
| `q_pool_w` | `f64` |  |
| `q_atm_w` | `f64` |  |
| `t_surface_k` | `f64` |  |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

#### Function `exchange`

MELCOR's pool-surface exchange (CVH/FL RM §5.1.1-5.1.2), at the start
of a step of `dt` s and limited as MELCOR limits it: each phase's
sensible heat to its content above the surface temperature (Eq 5-17),
and condensation to 90 % of the atmosphere's vapour (C4407).

```rust
pub fn exchange(pool: &Pool, atm: &Atmosphere, surface: &Surface, dt: f64) -> Exchange { /* ... */ }
```

#### Function `structure_surface`

MELCOR's heat-structure surface facing an atmosphere (HS RM §2.6.1,
§2.7): natural convection on an external rectangular surface of height
`l_c`, `Nu = 0.59 Ra^1/4` below `Ra = 1e9`, `0.10 Ra^1/3` above `1e10`,
linear in `Ra` between (Table 2.3, Eqs 2-82, 2-85 to 2-87), with the
Grashof number from the density difference while condensing; and
condensation below the dew point by diffusion through the
non-condensables, `m = h_D rho_v ln((P - P_srf) / (P - P_stm))` with
`h_D = Nu Sc^1/3 Pr^-1/3 D / l_c` (Eqs 2-134 to 2-136; `rho_v` the
vapour's density at `T_sat(P)`); above `VPFRAC = 0.9995` of steam the
steam fraction is held at it (stated: MELCOR then lets the
structure's conduction limit it). Transport properties at the film
temperature, density at the bulk. Returns the convective coefficient,
W/(m^2 K), and the condensation flux, kg/(m^2 s) (zero above the dew
point: no film is kept to evaporate, stated).

```rust
pub fn structure_surface(atm: &Atmosphere, t_wall_k: f64, l_c: f64) -> (f64, f64) { /* ... */ }
```

#### Function `bubble_efficiency`

MELCOR's bubble efficiency at a vent below the pool (CVH/FL RM Eqs 6-1
to 6-3): `epsilon_z = (depth - 0.01 m) / h` with `h` the opening's
height, and `epsilon_T = (T_sat(P_j) - T_P - 0.1 K) / 5 K` with `P_j`
the pressure at the opening's depth, each clamped to [0, 1].

```rust
pub fn bubble_efficiency(depth_m: f64, opening_m: f64, p_junction_pa: f64, t_pool_k: f64) -> f64 { /* ... */ }
```

## Module `rn_volume`

One control volume's fission products and aerosol: the sectional aerosol
(`aerosol`), each condensable class (cesium as CsOH, iodine as I2 and,
since 2026-10-29, tellurium, in [`CLASSES`]) as vapour, on the airborne
aerosol and on each surface, and
the transfers between them in one step (gh:#1047).

The step follows MELCOR's order and coupling (Vol. 2, SAND2023-10994 O,
RN §2.4 and §2.5.2; no source used): released aerosol enters the
**smallest section** (RN §2.4.3: "consistent with the production of
small particles by gas-to-particle conversion"); vapour condenses onto
the aerosol sections (Sherwood number 2) and onto the surfaces at their
own temperatures by TRAP-MELT2's analytic step; the aerosol agglomerates
and deposits.

~~**Simplification, stated:** each class on the airborne aerosol is
tracked as one mass, sharing the aerosol's size distribution, which is
MELCOR's rule for classes assigned to one component (RN §2.4.1). MELCOR's
default puts the volatiles in a component of their own; that is not
followed here.~~ **Changed 2026-10-29:** each class on the airborne
aerosol is tracked **by section**. It condenses onto each section in
proportion to that section's mass-transfer coefficient (MELCOR's
condensation onto the aerosol is per section, RN §2.5.2). It moves
through agglomeration by the same coefficients as the particle volume
(Jacobson et al.'s multicomponent scheme) and deposits with its
section. The condensed mass still does not change the particles' size,
which MELCOR's separate volatile component would. The lumped model
before survives as the ablation (`lumped`).

Evaporation from a surface is limited to what it holds (TRAP-MELT2's
step does not limit it; the excess returns to the vapour).

```rust
pub mod rn_volume { /* ... */ }
```

### Types

#### Type Alias `PerClass`

Per-class masses, kg.

```rust
pub type PerClass = [f64; 3];
```

#### Type Alias `PerClassSection`

Per-class masses by aerosol section, kg (`[class][section]`).

```rust
pub type PerClassSection = [Vec<f64>; 3];
```

#### Struct `RnFlow`

What a flow carries from one volume to another (`RnVolume::take`):
particle volume per section, m^3, each class's vapour, kg, and each
class on the aerosol by section, kg. Empty when nothing is carried this
way.

```rust
pub struct RnFlow {
    pub moved: Vec<f64>,
    pub vapour_kg: PerClass,
    pub on_aerosol_kg: PerClassSection,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `moved` | `Vec<f64>` |  |
| `vapour_kg` | `PerClass` |  |
| `on_aerosol_kg` | `PerClassSection` |  |

##### Implementations

###### Methods

- ```rust
  pub fn scaled(self: &Self, f: f64) -> Self { /* ... */ }
  ```
  The flow scaled by `f`.

- ```rust
  pub fn write(self: &Self, w: &mut Writer) { /* ... */ }
  ```

- ```rust
  pub fn read(r: &mut Reader<''_>) -> Self { /* ... */ }
  ```

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `RnSurface`

A deposition surface as this volume sees it: the aerosol surface (area,
facing, phoretic velocities) and, for the vapour, its temperature, K,
and mass-transfer coefficient, m/s.

```rust
pub struct RnSurface {
    pub surface: super::aerosol::Surface,
    pub t_k: f64,
    pub k_vapour: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `surface` | `super::aerosol::Surface` |  |
| `t_k` | `f64` |  |
| `k_vapour` | `f64` |  |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `RnVolume`

What is in the volume.

```rust
pub struct RnVolume {
    pub volume_m3: f64,
    pub sections: super::aerosol::Sections,
    pub vapour_kg: PerClass,
    pub on_aerosol_kg: PerClassSection,
    pub lumped: bool,
    pub condensate_mass: bool,
    pub on_surface_kg: Vec<PerClass>,
    pub aerosol_on_surface_kg: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `volume_m3` | `f64` |  |
| `sections` | `super::aerosol::Sections` |  |
| `vapour_kg` | `PerClass` | Each class in the gas as vapour, and on the airborne aerosol, kg. |
| `on_aerosol_kg` | `PerClassSection` |  |
| `lumped` | `bool` | The ablation (not state; set by the owner before each use): each<br>class on the aerosol is spread over the sections in proportion to<br>the aerosol's volume before every operation, which is the lumped<br>model before 2026-10-29 to round-off. |
| `condensate_mass` | `bool` | Whether a class's mass condensed on (or evaporated from) the aerosol<br>adds to (or takes from) its section's particle volume, at the<br>aerosol density, as MELCOR's sectional mass does (RN §2.5.2: the<br>condensed mass on an aerosol section is that section's aerosol mass,<br>and its area follows the section's mass). Not state; set by the<br>owner before each use. False is the condensate riding on the<br>particles weightless (the model before 2026-10-29). |
| `on_surface_kg` | `Vec<PerClass>` | Per surface: each class on it, kg (by deposition and by<br>condensation), and aerosol deposited on it, kg. |
| `aerosol_on_surface_kg` | `Vec<f64>` |  |

##### Implementations

###### Methods

- ```rust
  pub fn new(volume_m3: f64, sections: Sections, n_surfaces: usize) -> Self { /* ... */ }
  ```

- ```rust
  pub fn set_volume(self: &mut Self, volume_m3: f64) { /* ... */ }
  ```
  Resize the gas volume to `volume_m3` (a vessel's gas space as its

- ```rust
  pub fn aerosol_kg(self: &Self, rho_p: f64) -> f64 { /* ... */ }
  ```
  Airborne aerosol mass, kg, at material density `rho_p`.

- ```rust
  pub fn total_kg(self: &Self, class: usize) -> f64 { /* ... */ }
  ```
  One class's mass everywhere in the volume (vapour, on aerosol, on

- ```rust
  pub fn on_aerosol(self: &Self, class: usize) -> f64 { /* ... */ }
  ```
  One class's mass on the airborne aerosol, all sections, kg.

- ```rust
  pub fn add(self: &mut Self, aerosol_kg: f64, rho_p: f64, vapour_kg: PerClass) { /* ... */ }
  ```
  Add released aerosol, kg, into the smallest section, and vapour of

- ```rust
  pub fn take(self: &mut Self, f: f64) -> (Vec<f64>, PerClass, PerClassSection) { /* ... */ }
  ```
  Take the fraction `f` of the volume's gas (with its aerosol and what

- ```rust
  pub fn receive(self: &mut Self, moved: &[f64], vapour_kg: PerClass, on_aerosol_kg: &PerClassSection) { /* ... */ }
  ```
  Receive what `take` returned from another volume (same sections).

- ```rust
  pub fn write(self: &Self, w: &mut Writer) { /* ... */ }
  ```
  Write the state (the section volumes are fixed by construction).

- ```rust
  pub fn read_into(self: &mut Self, r: &mut Reader<''_>) { /* ... */ }
  ```
  Read the state into a volume built like this one.

- ```rust
  pub fn step(self: &mut Self, medium: &Medium, gases: &[(BulkGas, f64)], surfaces: &[RnSurface], dt: f64) { /* ... */ }
  ```
  One step of `dt` s: condensation of each class, agglomeration,

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
### Constants and Statics

#### Constant `CLASSES`

The condensable classes tracked, in the order of every per-class array.

```rust
pub const CLASSES: [super::vapour::FpClass; 3] = _;
```

#### Constant `N_CLASSES`

How many classes are tracked: cesium (as CsOH), iodine (as I2) and,
since 2026-10-29, tellurium (elemental, MELCOR's class 5).

```rust
pub const N_CLASSES: usize = 3;
```

#### Constant `CS`

Index of cesium and of iodine in the per-class arrays.

```rust
pub const CS: usize = 0;
```

#### Constant `IODINE`

```rust
pub const IODINE: usize = 1;
```

#### Constant `TE`

```rust
pub const TE: usize = 2;
```

## Module `source`

The Unit 1 release as a `changi` source term (gh:#1040, F9's "wire the
release to changi"): the hand-off the offsite chain consumes, as
[`crate::chain::pad_for_dispersion`] expects it.

- **What leaves**: the model's cumulative releases to the environment
  (vented to the stack plus leaked to the reactor building, whose own
  retention is not modelled), as fractions of each class's inventory
  ([`super::compare::Histories`]).
- **Inventories**: each nuclide's activity at the scram, `lambda N`, from
  the model's ENDF/B-VIII summation ([`super::fp_decay`]). Cs-137, which
  builds up through the irradiation, is scaled to the BSAF
  specification's cesium mass (the summation's 95.0 kg against the
  specified 154.0 kg: its 584-day irradiation is MELCOR's default, not
  Unit 1's history). I-131 and Xe-133 are not scaled: at 8 and 5 days
  they stand at equilibrium with the power whatever the history, and an
  element mass (stable I-127 and long-lived I-129 for iodine) says
  nothing about them. The summation has no neutron capture (stated
  there), which biases the Cs-137 share of cesium low.
- **Nuclides**: Cs-137 (aerosol), I-131 (halogen) and Xe-133 (noble gas),
  their classes' isotopes taken to move together. Decay constants from
  `boon-lay` (`changi`'s rule), deposition groups by atomic number
  ([`DepositionGroup::from_atomic_number`]).
- **Windows**: the histories' samples grouped into windows of
  `window_s`; each window's activity is its share of the inventory
  decayed from the scram to the window's middle (the release is taken as
  uniform within a window, as `changi` treats it).

Not done here: the dispersion run itself, which needs the Fukushima
meteorology (not held).

```rust
pub mod source { /* ... */ }
```

### Functions

#### Function `inventory_bq`

One nuclide's activity at the scram, Bq, from the summation: `lambda N`
with `N` the nuclide's mass over `A u`, scaled by `scale`.

```rust
pub fn inventory_bq(sum: &super::fp_decay::Summation, endf_name: &str, boon_lay_name: &str, scale: f64) -> f64 { /* ... */ }
```

#### Function `source_term`

**Attributes:**

- `MustUse { reason: None }`

The release in `h` as a source term of windows `window_s` long (the
first starting at the scram), with Cs-137, I-131 and Xe-133.

# Panics
If the histories are empty, or a nuclide is missing from `boon-lay`.

```rust
pub fn source_term(h: &super::compare::Histories, prm: &super::params::Params, sum: &super::fp_decay::Summation, window_s: f64) -> changi::activity::source::SourceTerm { /* ... */ }
```

## Module `sparc`

Pool scrubbing past the vent exit: SPARC-90's inertial impaction and
swarm-rise removal, as MELCOR describes them (gh:#1047; no source used;
MELCOR 2.2 Vol. 2, SAND2023-10994 O, RN §2.7.1 Eqs 2-112 and 2-113,
Appendices D and E).

MELCOR's overall decontamination factor for each particle size is

```text
DF_OV,i = DF_EC . DF_II,i . DF_ER,i . DF_SR,i                (2-113)
```

`DF_EC`, removal in proportion to the gas volume the steam's condensation
takes away, is in `containment.rs`. This module gives the next two:

- **`DF_II`, inertial impaction** into the front of the decelerating
  globule at the vent exit (Eqs D-8 to D-11), from the Stokes number of
  the particle at the vent's exit velocity.
- **`DF_SR`, the swarm rise** (Eq 2-112, Appendix E). The injected gas
  forms a globule (Eqs D-1 to D-4) that breaks into a swarm of bubbles
  over twelve of its diameters. The rise from the vent exit to the pool
  surface is marched in `N_RISE` steps; in each, the part of the gas
  already in bubbles loses particles to the bubble wall by centrifugal
  and Brownian deposition and by settling (Eqs E.12 to E.25), the part
  still in the globule loses none.

`DF_ER` (deposition during injection) applies only to multihole vents
(MELCOR's `MVENT=1`). The downcomers are single large pipes, so it is 1
and is not built.

**Where this departs from the manual, and why:**

- **The bubble is at the pool's temperature and saturated with
  vapour.** MELCOR iterates the bubble's heat and mass transfer through
  each step (Eqs E.6 to E.11). The gas is taken to be already at the
  pool's temperature when it leaves the vent exit region, which is
  MELCOR's own assumption for that region. Two things follow. The
  evaporation into the bubble is whatever keeps it saturated as it
  expands under the falling static head. And the particles do not grow
  by condensation (no supersaturation, so Eq E.10 is not needed). That
  removes a removal mechanism MELCOR has, so this is a lower bound.
- **The exposure time of the bubble surface** is the boundary-layer
  value for a stretching surface (Levich): `tau_e = int r^2 V_s ds /
  (r^2 V_s^2)`. The manual's "arc length divided by the local surface
  velocity" is the flat-plate limit of this. Taken literally on a
  bubble it diverges at the top stagnation point, where V_s goes to 0
  (the integral of ds/V_s is logarithmic there). On a sphere, Levich's
  form gives Boussinesq's Sh = (2/sqrt(pi)) Pe^(1/2) exactly; the test
  checks it.
- **The surface normal** is the ellipse's own normal: tan beta =
  (b/a)^2 tan theta. The manual's Eq E.15 prints (a/b)^2. With a > b
  (oblate), that would make the normal *less* vertical than the radius
  on the flat top, which is geometrically wrong.
- **The evaporation-hindrance floor** of Eq E.14 ("limited to a minimum
  value of V_D,i") is read as the diffusion velocity actually in
  effect, `xi V_D` of Eq E.24. Read as the unhindered `V_D`, the floor
  would undo E.24 wherever it binds.
- **The fraction of the gas still in the globule** is its volume
  fraction, `(D_g / D_g,o)^3`, from the linear shrinkage of Eq D-4.
  The manual gives the shrinkage but not the fraction.
- **Settling** is Stokes's law with slip (Eq E.16) throughout. MELCOR
  switches to empirical drag above about 70 um (Eqs E.17 to E.19);
  the largest section here is 40 um.

Gas properties are air's, as MELCOR evaluates every aerosol
coefficient (RN §2.4.2.3, constraint c).

```rust
pub mod sparc { /* ... */ }
```

### Types

#### Enum `Vent`

The vent types of MELCOR's globule correlation `v_n = a We^b` (Eq D-1,
C7151 defaults).

```rust
pub enum Vent {
    MultipleHoles,
    Downcomer,
    Horizontal,
}
```

##### Variants

###### `MultipleHoles`

###### `Downcomer`

###### `Horizontal`

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Pool`

The pool where the gas is injected.

```rust
pub struct Pool {
    pub t_k: f64,
    pub p_surface_pa: f64,
    pub depth_m: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `t_k` | `f64` |  |
| `p_surface_pa` | `f64` | Gas pressure at the pool surface, Pa. |
| `depth_m` | `f64` | Submergence of the vent exit, m. |

##### Implementations

###### Methods

- ```rust
  pub fn submerged(t_k: f64, p_surface_pa: f64, head_pa: f64) -> Self { /* ... */ }
  ```
  A pool at `t_k` under gas at `p_surface_pa`, the vent exit under a

- ```rust
  pub fn x_o(self: &Self) -> f64 { /* ... */ }
  ```
  MELCOR's `X_o` (Eq D-6): the non-condensable mole fraction of the

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Injection`

The gas entering one vent: its non-condensable flow, mol/s, its total
volumetric flow at the vent exit before equilibration, m^3/s, and the
vent's diameter, m.

```rust
pub struct Injection {
    pub vent: Vent,
    pub diameter_m: f64,
    pub nc_mol_s: f64,
    pub q_exit_m3_s: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `vent` | `Vent` |  |
| `diameter_m` | `f64` |  |
| `nc_mol_s` | `f64` |  |
| `q_exit_m3_s` | `f64` |  |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `IodineVapour`

The pool's side of iodine-vapour scrubbing: I2's diffusivity in the
bubble gas and in the water, m^2/s, the pool's partition coefficient
(`iodine::partition`), its dissolved iodine, mol of I2 per litre, and
the entering gas's I2 concentration at the vent exit, mol per litre of
gas.

```rust
pub struct IodineVapour {
    pub d_gas: f64,
    pub d_liquid: f64,
    pub h: f64,
    pub sum_aq: f64,
    pub c_gas: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `d_gas` | `f64` |  |
| `d_liquid` | `f64` |  |
| `h` | `f64` |  |
| `sum_aq` | `f64` |  |
| `c_gas` | `f64` |  |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

#### Function `bubble_axes`

The bubble of the swarm, as an oblate spheroid of semi-axes `a`
(horizontal) and `b` (vertical), m, for a non-condensable mole fraction
`x_nc` (Eqs E.1, E.2).

```rust
pub fn bubble_axes(x_nc: f64) -> (f64, f64) { /* ... */ }
```

#### Function `rise_velocity`

The bubble's rise velocity relative to the liquid, m/s (Eq E.3, with
`sigma` in N/m and `rho_l` in kg/m^3 converted to the correlation's cgs).

```rust
pub fn rise_velocity(d_cm: f64, sigma: f64, rho_l: f64) -> f64 { /* ... */ }
```

#### Function `swarm_velocity`

The swarm's rise velocity, m/s (Eqs E.4, E.5): the mean of the
correlation at the vent depth and at the surface, for a total flow
`q_l_s` (litre/s, at half depth) through one vent, at most 1.7 m/s.

```rust
pub fn swarm_velocity(q_l_s: f64, depth_m: f64) -> f64 { /* ... */ }
```

#### Function `globule_diameter`

The initial globule's diameter, m (Eqs D-1 to D-3), for a gas flow
`q` (m^3/s at the vent depth, in equilibrium with the pool).

```rust
pub fn globule_diameter(vent: Vent, d_o: f64, q: f64, sigma: f64, rho_l: f64) -> f64 { /* ... */ }
```

#### Function `impaction_df`

The inertial-impaction factor `DF_II` (Eqs D-8 to D-11) for a particle
of diameter `d` and density `rho_p`, gas viscosity `mu`, vent exit
velocity `v_e` and vent diameter `d_o`.

```rust
pub fn impaction_df(d: f64, rho_p: f64, mu: f64, v_e: f64, d_o: f64) -> f64 { /* ... */ }
```

#### Function `pass_fractions`

Pass fractions `1 / (DF_II DF_SR)` for particles of diameters `d` (m)
and density `rho_p` injected through one vent into `pool`.

```rust
pub fn pass_fractions(pool: &Pool, inj: &Injection, d: &[f64], rho_p: f64) -> Vec<f64> { /* ... */ }
```

#### Function `i2_liquid_diffusivity`

I2's diffusivity in water, m^2/s, at `t_k`: 1.2e-9 at 298.15 K \[A\],
scaled by Stokes-Einstein (`T / mu`) with water's viscosity from the
steam tables. The manual gives no value; with the pool's partition
large (CsOH), the gas side controls and this barely matters (test).

```rust
pub fn i2_liquid_diffusivity(t_k: f64, p_pa: f64) -> f64 { /* ... */ }
```

#### Function `iodine_pass`

The swarm rise's pass fraction for I2 vapour through one vent (MELCOR
Appendix F): in each step and on each strip of the bubble, the
gas-side penetration velocity `sqrt(D_g / (pi tau_e))` brings I2 to the
interface, whose concentration balances it against the liquid side
with the partition (Eq F.2); the net velocity is hindered by the
evaporation as for particles (Eq F.1) and integrated as Eq E.13. The
bubble's I2 concentration is updated after each step (diluted as the
bubble expands and scrubbed). No scrubbing where the pool's iodine over
the partition is at least the bubble's.

```rust
pub fn iodine_pass(pool: &Pool, inj: &Injection, iv: &IodineVapour) -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `N_RISE`

Steps of the rise (MELCOR's XNRISE, C7150(1), default 10).

```rust
pub const N_RISE: usize = 10;
```

## Module `steam`

Water and steam properties for the plant model, all from IAPWS-IF97 as
`tampines-steam-tables` implements it (`tampines_steam_tables`, ~~a
dev-dependency of this crate; ~~through the metacrate~~ directly since the
demo moved here on 2026-10-27~~ a normal dependency of `sembawang` since
the model moved out of the `dhoby-ghaut` demo on 2026-10-29, gh:#1033).

The RPV is solved every step for the saturation pressure whose mixture
matches the vessel's mass and energy, which takes a few dozen saturation
evaluations. So the saturation line is **tabulated once** at start-up
from the steam tables (500 points, log-spaced in pressure from 0.02 to
9.5 MPa) and interpolated linearly in `ln p`. Every worker builds the same
table from the same calls, so it is identical everywhere (the three-worker
step is bit-for-bit the one-worker step).

**Interpolation error** (measured by `the_table_matches_the_steam_tables_between_its_points`):
below 3e-4 relative on `T_sat`, `v_f`, `v_g`, `u_f`, `u_g`, `h_f`, `h_g`
at the midpoints of the table between 0.05 and 9 MPa.

```rust
pub mod steam { /* ... */ }
```

### Types

#### Struct `Sat`

Saturated-water properties at one pressure.

```rust
pub struct Sat {
    pub p_pa: f64,
    pub t_k: f64,
    pub v_f: f64,
    pub v_g: f64,
    pub u_f: f64,
    pub u_g: f64,
    pub h_f: f64,
    pub h_g: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `p_pa` | `f64` |  |
| `t_k` | `f64` |  |
| `v_f` | `f64` |  |
| `v_g` | `f64` |  |
| `u_f` | `f64` |  |
| `u_g` | `f64` |  |
| `h_f` | `f64` |  |
| `h_g` | `f64` |  |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `SatTable`

The tabulated saturation line.

```rust
pub struct SatTable {
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

- ```rust
  pub fn at(self: &Self, p_pa: f64) -> Sat { /* ... */ }
  ```
  Saturation properties at `p_pa`, clamped to the table's range.

- ```rust
  pub fn mixture(self: &Self, v: f64, u: f64) -> Option<(Sat, f64)> { /* ... */ }
  ```
  The saturated mixture whose specific volume is `v` and specific

- ```rust
  pub fn superheated(self: &Self, v: f64, u: f64) -> (f64, f64) { /* ... */ }
  ```
  Superheated steam past dry-out, continued from the saturated vapour

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

#### Function `log_grid`

`n` log-spaced points from `lo`, with the step of `n_grid` points over
`lo..=grid_top`, continued until past `top` (gh:#1036 follow-up).

```rust
pub fn log_grid(lo: f64, grid_top: f64, n_grid: usize, top: f64) -> Vec<f64> { /* ... */ }
```

#### Function `sat_from_tables`

One saturation point straight from the steam tables.

```rust
pub fn sat_from_tables(p_pa: f64) -> Sat { /* ... */ }
```

#### Function `p_sat_pa`

Saturation pressure at a temperature, Pa (IF97 region 4, closed form).

```rust
pub fn p_sat_pa(t_k: f64) -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `P_MIN_PA`

Lowest and highest pressure the table covers, Pa. ~~9.5e6~~ **Raised
2026-10-29** to 20 MPa: a rapid debris quench can push the vessel past
9.5 MPa, and above the table [`SatTable::mixture`] found no state, which
the plant took for "boiled dry" (level at the bottom, the superheated
branch): a wrong state, not physics. 20 MPa stays below the critical
point (22.064 MPa), as MELCOR advises for its own vessel-failure
pressure (MELCOR 2.2 Vol. 2, COR §2.3.8).

```rust
pub const P_MIN_PA: f64 = 2.0e4;
```

#### Constant `P_MAX_PA`

```rust
pub const P_MAX_PA: f64 = 2.0e7;
```

#### Constant `P_GRID_TOP_PA`

The grid: `N_GRID` points log-spaced over `P_MIN_PA` to `P_GRID_TOP_PA`
(the original table), continued at the same step up to `P_MAX_PA`, so
every original point is unchanged bit for bit.

```rust
pub const P_GRID_TOP_PA: f64 = 9.5e6;
```

## Module `vapour`

Fission-product vapours: vapour pressures and condensation onto surfaces
(gh:#1047, second deliverable; not yet wired into the plant).

**Vapour pressures** are MELCOR's default curves (Vol. 1, SAND2017-0455
O, RN sensitivity coefficients 7110; no source used),
`log10 P[mmHg] = -A/T + B + C log10 T` over temperature ranges, zero
below the first. The classes tracked so far: Xe (always a vapour), Cs
(released as CsOH, 7120), I2, and Te (as TeO). **One extracted
coefficient is corrected**: the text reads Cs's first-range `A` as
9.4E4, but the manual says these curves were adjusted to be continuous,
and only `A = 9.4E3` meets the second range at 1553 K (log10 P = 3.570
from both sides; 9.4E4 gives -50.9, a 54-decade miss). The continuity of every
range boundary is pinned by a test, which is what makes the correction
checkable.

**Condensation** onto heat structures, pools and aerosol sections is
TRAP-MELT2's, solved analytically over a step as MELCOR does (Vol. 2,
SAND2023-10994 O, RN §2.5.2, Eqs 2-96 to 2-101): each surface `i` takes
`A_i k_i (C_a - C_i^s)`, the atmosphere gives it up, and over the step
`C_a` relaxes exponentially toward the area-weighted saturation
concentration. This module does not limit evaporation to what a
surface holds; the caller must (stated).

**Vapour diffusivity** is Chapman-Enskog's (Vol. 2, RN §2.5.2, Eqs 2-104,
2-105, after Bird, Stewart and Lightfoot), with the Lennard-Jones
parameters of the classes (Vol. 1, RN coefficients 7111) and of the bulk
gases (Vol. 2, NCG/H2O §5.2), and the collision integral of Vol. 2's
NCG/H2O Table 6.1 with its extrapolations. An aerosol section takes
vapour with the mass-transfer coefficient of Sherwood number 2 (RN
§2.5.2).

```rust
pub mod vapour { /* ... */ }
```

### Types

#### Enum `FpClass`

A fission-product vapour class (MELCOR default names).

```rust
pub enum FpClass {
    Xe,
    Cs,
    I2,
    Te,
}
```

##### Variants

###### `Xe`

###### `Cs`

###### `I2`

###### `Te`

##### Implementations

###### Methods

- ```rust
  pub fn diffusivity(self: Self, gases: &[(BulkGas, f64)], t_k: f64, p_pa: f64) -> f64 { /* ... */ }
  ```
  Diffusivity of this vapour (a trace) in a gas of the mole fractions

- ```rust
  pub fn aerosol_mass_transfer(self: Self, gases: &[(BulkGas, f64)], t_k: f64, p_pa: f64, d: f64) -> f64 { /* ... */ }
  ```
  Mass-transfer coefficient of the vapour onto an aerosol particle of

- ```rust
  pub fn molar_mass(self: Self) -> f64 { /* ... */ }
  ```
  Molar mass of the released compound, kg/mol (C7120: CsOH, I2, TeO).

- ```rust
  pub fn vapour_pressure_pa(self: Self, t_k: f64) -> f64 { /* ... */ }
  ```
  Vapour pressure at `t_k`, Pa: infinite for a permanent gas (Xe), zero

- ```rust
  pub fn saturation_concentration(self: Self, t_k: f64) -> f64 { /* ... */ }
  ```
  Saturation concentration at `t_k`, kg/m^3 (ideal gas).

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Enum `BulkGas`

A bulk gas of the containment or vessel atmosphere.

```rust
pub enum BulkGas {
    Steam,
    H2,
    N2,
}
```

##### Variants

###### `Steam`

###### `H2`

###### `N2`

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `CondensingSurface`

A surface the vapour can condense on: its `A k`, m^3/s (area times
mass-transfer coefficient), its saturation concentration at its own
temperature, kg/m^3, and the mass condensed on it, kg.

```rust
pub struct CondensingSurface {
    pub ak: f64,
    pub c_sat: f64,
    pub mass: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ak` | `f64` |  |
| `c_sat` | `f64` |  |
| `mass` | `f64` |  |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

#### Function `binary_diffusivity`

Binary diffusivity of molecule A in B, m^2/s (Eq 2-105, cm^2/s with P
in atm, converted), from their Lennard-Jones parameters (sigma A,
epsilon/k K, molar mass g/mol).

```rust
pub fn binary_diffusivity(a: (f64, f64, f64), b: (f64, f64, f64), t_k: f64, p_pa: f64) -> f64 { /* ... */ }
```

#### Function `condense`

TRAP-MELT2's analytic step (Eqs 2-98 to 2-101): the atmosphere's vapour
concentration `c_a`, kg/m^3, in `volume` m^3, against `surfaces`, over
`dt` s. Returns the new concentration and updates each surface's mass.
Mass is conserved exactly (`c_a V + sum M`).

```rust
pub fn condense(c_a: f64, volume: f64, surfaces: &mut [CondensingSurface], dt: f64) -> f64 { /* ... */ }
```

## Module `wire`

The flat `f64` wire format snapshots and results cross worker boundaries
in (a `Float64Array` in the browser). Every value crosses as its exact
`f64` bits, so a step computed on another worker is bit-for-bit the step
computed in place.

```rust
pub mod wire { /* ... */ }
```

### Types

#### Struct `Writer`

```rust
pub struct Writer(pub Vec<f64>);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<f64>` |  |

##### Implementations

###### Methods

- ```rust
  pub fn f(self: &mut Self, v: &[f64]) { /* ... */ }
  ```

- ```rust
  pub fn vec(self: &mut Self, v: &[f64]) { /* ... */ }
  ```
  A length-prefixed vector.

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
- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
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
- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Reader`

```rust
pub struct Reader<''a> {
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
  pub fn new(v: &'a [f64]) -> Self { /* ... */ }
  ```

- ```rust
  pub fn f(self: &mut Self) -> f64 { /* ... */ }
  ```
  The next number (NaN past the end: a short message reads as NaN

- ```rust
  pub fn vec(self: &mut Self) -> Vec<f64> { /* ... */ }
  ```

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
- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
### Types

#### Enum `Mode`

Who drives the operator actions.

```rust
pub enum Mode {
    Historical,
    FreePlay,
}
```

##### Variants

###### `Historical`

The recorded Unit 1 sequence (INPO 11-005 timeline, \[U\] times).

###### `FreePlay`

The reader's own actions.

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Mechanisms`

Which modelled mechanisms are on. Every one is **on by default**: the
correct-physics-default rule (root `CLAUDE.md`). Turning one off is an
explicit ablation through a `without_*` builder, used by [`compare`] to
measure how much of each miss against BSAF a mechanism carries
(gh:#1032).

A core mechanism travels to the core worker in its snapshot
(`CoreSnapshot::t_dependent_props`); the others act in the plant.

```rust
pub struct Mechanisms {
    pub hot_leak: bool,
    pub ic_h2_blanketing: bool,
    pub temperature_dependent_properties: bool,
    pub lower_head_wall: bool,
    pub rod_collapse: bool,
    pub core_radiation: bool,
    pub level_swell: bool,
    pub decay_mix: bool,
    pub capture_correction: bool,
    pub actinide_decay: bool,
    pub mixture_choking: bool,
    pub gas_cv_t: bool,
    pub diffusion_limit: bool,
    pub hydrogen_cp_t: bool,
    pub msl_creep: bool,
    pub dome_internals: bool,
    pub penetrations: bool,
    pub failed_vessel_outflow: bool,
    pub pool_scrubbing: bool,
    pub in_vessel_deposition: bool,
    pub internals_heat_loss: bool,
    pub scrubbing_saturation: bool,
    pub swarm_scrubbing: bool,
    pub sectional_classes: bool,
    pub iodine_partition: bool,
    pub mixture_vent: bool,
    pub phoresis: bool,
    pub iodine_swarm: bool,
    pub vessel_aerosol: bool,
    pub condensate_mass: bool,
    pub fp_decay_follows: bool,
    pub te_transport: bool,
    pub debris_oxidation: bool,
    pub candling: bool,
    pub debris_quench: bool,
    pub wet_head_contact: bool,
    pub penetration_coverage: bool,
    pub failed_vessel_drain: bool,
    pub ww_atmosphere: bool,
    pub dw_wall_hs: bool,
    pub ww_shell: bool,
    pub outflow_enthalpy: bool,
    pub relief_h2_enthalpy: bool,
    pub condensate_to_floor: bool,
    pub rpv_wall: bool,
    pub internals_radiation: bool,
    pub core_top_radiation: bool,
    pub see_through: bool,
    pub mcci: bool,
    pub dw_wall_outer: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `hot_leak` | `bool` | The RPV hot-gas leak to the drywell ([`HOT_LEAK_M2`], `[A]`). |
| `ic_h2_blanketing` | `bool` | Hydrogen blanketing of the isolation-condenser tubes (`[A]`). |
| `temperature_dependent_properties` | `bool` | MATPRO temperature-dependent fuel and cladding heat capacities<br>(OFFBEAT, gh:#1038); off is the constant `UO2_CP`/`ZR_CP` model. |
| `lower_head_wall` | `bool` | The lower-head wall: heated by the debris, failing on its inner<br>temperature or by creep (gh:#1034); off is the debris-temperature<br>criterion `LOWER_HEAD_FAILURE_K`. |
| `rod_collapse` | `bool` | MELCOR's rod collapse (gh:#1035): breach at `CLAD_BREACH_K`,<br>collapse at `ROD_COLLAPSE_K` or by time-at-temperature damage; off<br>is the 2100 K whole-node slump `ZR_MELT_K`. |
| `core_radiation` | `bool` | In-core radiation between rings and levels, and from the outer ring<br>to the shroud (gh:#1037); off is no radiation at all. |
| `level_swell` | `bool` | The core sees the swollen two-phase mixture level, not the collapsed<br>one (gh:#1036); off is the collapsed level. |
| `decay_mix` | `bool` | Decay heat from U-235, Pu-239 and U-238 fission products after a<br>finite irradiation (gh:#1041); off is U-235 alone at equilibrium. |
| `capture_correction` | `bool` | The ANS neutron-capture correction G(t) (gh:#1041). |
| `actinide_decay` | `bool` | U-239 and Np-239 decay heat (gh:#1041). |
| `mixture_choking` | `bool` | Choked flow of the vessel's hydrogen-steam gas from its own molar<br>mass (gh:#1037); off is the steam-only flux for any gas. |
| `gas_cv_t` | `bool` | MELCOR's temperature-dependent `c_v` for the containment's N2 and H2<br>([`gas`], 2026-10-29); off is the constant 20.8 / 20.5 J/(mol K). |
| `diffusion_limit` | `bool` | MELCOR's gaseous-diffusion limit on Zr oxidation<br>([`core::diffusion_limited_rate`], 2026-10-29); off is the parabolic<br>rate limited only by the steam supply. |
| `hydrogen_cp_t` | `bool` | Hydrogen's `c_p(T)` in the core and the vessel (MELCOR's fit,<br>[`gas`], 2026-10-29); off is the constant `core::CP_H2_J_PER_KG_K`. |
| `msl_creep` | `bool` | Creep rupture of a main steam line under the relief flow's hot gas<br>([`msl`], MELCOR's PIPE-STR and LM-CREEP, 2026-10-29); off is no<br>steam-line failure. |
| `dome_internals` | `bool` | The steam dome's internals as a heat structure between the core and<br>the steam lines, leaks and containment ([`internals`], 2026-10-29);<br>off passes the core's exit gas straight through. |
| `penetrations` | `bool` | Lower-head penetration failure at the wall's temperature limit<br>(`core::HeadWall`, MELCOR §6.3, 2026-10-29); off is the wall alone. |
| `failed_vessel_outflow` | `bool` | After lower-head failure the vessel's gas (the hydrogen and steam<br>made after it) leaves through the breach on the vessel's own<br>pressure (2026-10-29); off is the vessel sealed at the drywell's<br>pressure, nothing leaving. |
| `pool_scrubbing` | `bool` | Pool scrubbing of aerosol and iodine where gas enters the pool (the<br>relief valves' quenchers, the downcomers), by the steam's<br>condensation: SPARC-90's DF_EC (2026-10-29). Off is MELCOR's<br>default with pool interaction off: everything passes. |
| `in_vessel_deposition` | `bool` | Cesium and iodine vapour condensing on the dome internals in the<br>vessel (2026-10-29); off is everything leaving the core reaching the<br>containment. |
| `internals_heat_loss` | `bool` | The dome internals lose heat to the saturated dome by natural<br>convection (2026-10-29); off is no heat loss once the flow stops. |
| `scrubbing_saturation` | `bool` | The condensation scrubbing stops where the pool is near saturation:<br>MELCOR's `DF_EC = X_o / X_i` with `X_o` the non-condensable fraction<br>of the gas in equilibrium with the pool at the vent depth (Eqs D-5,<br>D-6; 2026-10-29). Off is `X_o = 1`: all the steam condenses. |
| `swarm_scrubbing` | `bool` | Inertial impaction and swarm-rise scrubbing past the downcomer exits<br>(SPARC-90's `DF_II` and `DF_SR`, [`sparc`]; 2026-10-29). Off is<br>condensation scrubbing alone. |
| `sectional_classes` | `bool` | Cesium and iodine on the aerosol tracked by size section: condensed<br>in proportion to each section's mass transfer, carried through<br>agglomeration and deposited with their section (`rn_volume`;<br>2026-10-29). Off is the lumped model: each class shares the aerosol's<br>volume distribution. |
| `iodine_partition` | `bool` | The condensation scrubbing of iodine vapour limited by the pool's<br>partition coefficient for I2, from SPARC-90's equilibrium chemistry<br>with the pool's dissolved CsOH (MELCOR Eq D-7, Appendix F; [`iodine`];<br>2026-10-29). Off is iodine vapour scrubbed like the gas. |
| `mixture_vent` | `bool` | The wetwell vent passes the wetwell's gas mixture through its<br>orifice, the pool boiling to supply the steam, and the boiled-off<br>water takes its sensible heat too (gh:#1053; 2026-10-29). Off is the<br>vent sized for the non-condensables alone, with steam added in<br>proportion and only latent heat leaving the pool. |
| `phoresis` | `bool` | Thermophoresis (from the walls' heat flux) and diffusiophoresis<br>(from the steam condensing on them) onto the drywell's structures,<br>MELCOR's defaults (RN §2.4.2.2; 2026-10-29). Off is gravity and<br>Brownian diffusion alone. |
| `iodine_swarm` | `bool` | The swarm rise past the downcomers scrubs I2 vapour by diffusion to<br>the bubbles' walls, limited by the pool's partition (MELCOR Appendix<br>F; [`sparc::iodine_pass`]; 2026-10-29). Off is iodine vapour scrubbed<br>by condensation alone. |
| `vessel_aerosol` | `bool` | The vessel's gas as a sectional aerosol volume (`rn_volume`;<br>2026-10-29): the released aerosol and vapour agglomerate, condense<br>and deposit on the dome internals (at their temperature, with<br>thermophoresis from their heat flux) and settle on a floor of the<br>vessel's cross-section; what leaves carries its size distribution<br>into the containment. Off is the aerosol passing straight through<br>and the vapour condensing on the internals alone. |
| `condensate_mass` | `bool` | Cesium and iodine condensed on the aerosol add to their section's<br>particle mass, as MELCOR's sectional mass does (RN §2.5.2; the area,<br>the agglomeration and the deposition follow the section's total;<br>2026-10-29). Off is the condensate riding weightless on the UO2<br>particles. |
| `fp_decay_follows` | `bool` | The decay heat of the released noble gases, alkali metals and<br>halogens leaves the core with them and heats wherever they are: the<br>vessel, the drywell's gas and structures, the pool; what is vented<br>or leaked takes its heat away (MELCOR's DCH by class, the shares from<br>`fp_decay`'s summation; 2026-10-29). Chalcogens (Te) are released but<br>not transported, so their heat stays in the core (stated). Off is all<br>decay heat in the core. |
| `te_transport` | `bool` | Tellurium (MELCOR's chalcogen class) released with the volatiles<br>and transported like cesium and iodine, with its own vapour pressure<br>(RN 7110), its decay heat following it (2026-10-29; needs<br>`vessel_aerosol`). Off is tellurium staying in the core, heat and<br>all. |
| `debris_oxidation` | `bool` | The lower-plenum debris oxidises while it is quenched, by the<br>parabolic law over MELCOR's particulate area (COR §3.2.1,<br>2026-10-29). Off is debris that never oxidises. |
| `candling` | `bool` | Molten Zr candles down the rods once the oxide shell breaches and<br>refreezes in the first cooler node below, or reaches the debris<br>(MELCOR COR §3.1, simplified; 2026-10-29). Off is the metal staying<br>in its node until the rod falls. |
| `debris_quench` | `bool` | MELCOR's falling-debris quench: the debris in the wet lower plenum<br>gives the water `h A_p (T - T_sat)` at COR_LP's 100 W/(m^2 K) over<br>its particle area, with the decay factor, and is then limited by<br>Lipinski's dryout flux (COR §2.3.8; 2026-10-29). Off is the excess<br>heat released over a 120 s time constant \[A\]. |
| `wet_head_contact` | `bool` | The lower head takes heat from the debris that touches it while the<br>plenum holds water, and can fail then (as MELCOR's head conducts<br>from the debris in contact; 2026-10-29). Off is the head boiling<br>against the plenum's water while there is any. |
| `penetration_coverage` | `bool` | The penetrations take the debris's heat over the part of their<br>height it covers (MELCOR's `dz_d / dz_1`, COR §6.1), the rest<br>cooled by the plenum's water while there is any (2026-10-29). Off is<br>the penetration heated by the debris over all of it. |
| `failed_vessel_drain` | `bool` | A failed vessel's water drains through the breach into the<br>pedestal (2026-10-29; MELCOR opens a lower-plenum flow path at<br>failure). Off is the water kept in the failed vessel. |
| `ww_atmosphere` | `bool` | The wetwell atmosphere has its own temperature and steam: MELCOR's<br>pool-surface exchange (CVH/FL RM §5.1) and bubble model at the vents<br>(§6.1, the efficiency `epsilon_z epsilon_T`), flashing, and the vent<br>and vacuum breakers passing the atmosphere as it is (2026-10-29). Off<br>is the gas at the pool's temperature, saturated. |
| `dw_wall_hs` | `bool` | The drywell structures as MELCOR's heat-structure surface (HS RM<br>§2.6.1, §2.7): natural convection and condensation by diffusion<br>through the non-condensables, its latent heat into the wall<br>(2026-10-29). Off is the fixed 20 W/(m^2 K) \[A\] and condensation<br>only of supersaturated steam. |
| `ww_shell` | `bool` | The torus shell above the pool as a heat structure on the wetwell<br>atmosphere: MELCOR's natural convection and condensation, its outer<br>face adiabatic (2026-10-29; needs `ww_atmosphere`). Off is no<br>structure in the wetwell gas. |
| `outflow_enthalpy` | `bool` | Gas leaving the drywell through the downcomers and the flange leak<br>takes its enthalpy, so the drywell cools as it blows down and the<br>pool receives the flow work (MELCOR CVH/FL RM Eqs 6-5, 6-6;<br>2026-10-29). Off is the internal energy only, the drywell keeping its<br>temperature. |
| `relief_h2_enthalpy` | `bool` | The relief valves' hydrogen reaches the wetwell with its enthalpy<br>at the vessel's saturation temperature, closing the energy the<br>vessel gives up with it (2026-10-29; needs `ww_atmosphere`). Off is<br>the hydrogen at the pool's temperature. |
| `condensate_to_floor` | `bool` | Water condensed in the drywell (on its structures, and steam above<br>saturation) drains to the floor, the pedestal's pool, where debris<br>can boil it again (2026-10-29; MELCOR drains a structure's film to<br>its volume's pool). Off is the condensate leaving the model. |
| `rpv_wall` | `bool` | The RPV wall (cylinder and top head, 0.16 m of steel \[A\]) as a<br>lumped heat structure: it exchanges with the vessel's saturated<br>contents while they hold water, and loses heat through its<br>insulation to the drywell (2026-10-29, gh:#1037). Off is no wall. |
| `internals_radiation` | `bool` | The dome internals radiate to the RPV wall (MELCOR's gray pair, HS<br>RM Eq 2-96, with its default steel emissivity; 2026-10-29; needs<br>`rpv_wall`). Off is the internals with no heat path once the gas<br>stops flowing past them. |
| `core_top_radiation` | `bool` | The top of the core radiates to the dome internals above it, its<br>axial boundary structure (MELCOR COR RM §2.1.2, `Fcell,a` =<br>`axial_exchange`; 2026-10-29; needs `core_radiation`). Off is the top<br>of the core exchanging nothing. |
| `see_through` | `bool` | The core's radiation looks through emptied cells to the next<br>standing component, radially (or to the shroud) and axially, as<br>MELCOR's does (COR RM §2.1.2; 2026-10-29). Off is no view across an<br>emptied cell. |
| `mcci` | `bool` | Molten core-concrete interaction on the drywell floor: a reduced<br>form of MELCOR's CAV package ([`cavity`]; CORCON basaltic concrete,<br>quasi-steady ablation, the concrete's H2O and CO2 reduced by the<br>debris's Zr; 2026-10-29, F8). Off is the dry debris's heat going<br>straight into the drywell's structures. |
| `dw_wall_outer` | `bool` | The drywell structures lose heat across the gap to the concrete<br>around them, a 1-D conduction slab to the reactor building<br>(2026-10-29; MELCOR's heat structures conduct). Off is the lumped<br>structures adiabatic outside. |

##### Implementations

###### Methods

- ```rust
  pub fn without_hot_leak(self: Self) -> Self { /* ... */ }
  ```
  No RPV hot-gas leak: the vessel loses gas only through the relief

- ```rust
  pub fn without_temperature_dependent_properties(self: Self) -> Self { /* ... */ }
  ```
  Constant fuel and cladding heat capacities (`UO2_CP`, `ZR_CP`, both

- ```rust
  pub fn without_lower_head_wall(self: Self) -> Self { /* ... */ }
  ```
  The lower head fails when dry debris exceeds `LOWER_HEAD_FAILURE_K`

- ```rust
  pub fn without_rod_collapse(self: Self) -> Self { /* ... */ }
  ```
  A node slumps whole to the lower plenum when its cladding reaches

- ```rust
  pub fn without_core_radiation(self: Self) -> Self { /* ... */ }
  ```
  No radiation inside the core and no shroud heat structure: the model

- ```rust
  pub fn without_level_swell(self: Self) -> Self { /* ... */ }
  ```
  The core sees the collapsed level: the model before gh:#1036.

- ```rust
  pub fn without_decay_mix(self: Self) -> Self { /* ... */ }
  ```
  U-235 fission products alone, at equilibrium: the decay heat before

- ```rust
  pub fn without_capture_correction(self: Self) -> Self { /* ... */ }
  ```
  No neutron-capture correction (G = 1).

- ```rust
  pub fn without_actinide_decay(self: Self) -> Self { /* ... */ }
  ```
  No U-239 or Np-239 decay heat.

- ```rust
  pub fn without_mixture_choking(self: Self) -> Self { /* ... */ }
  ```
  The steam-only choked flux for the vessel gas, however much hydrogen

- ```rust
  pub fn without_gas_cv_t(self: Self) -> Self { /* ... */ }
  ```
  Constant `c_v` for the containment's N2 (20.8) and H2 (20.5 J/(mol

- ```rust
  pub fn without_diffusion_limit(self: Self) -> Self { /* ... */ }
  ```
  No gaseous-diffusion limit on oxidation: the model before

- ```rust
  pub fn without_hydrogen_cp_t(self: Self) -> Self { /* ... */ }
  ```
  Hydrogen at the constant `c_p` in the core and the vessel: the model

- ```rust
  pub fn without_msl_creep(self: Self) -> Self { /* ... */ }
  ```
  No main-steam-line creep rupture: the model before 2026-10-29.

- ```rust
  pub fn without_dome_internals(self: Self) -> Self { /* ... */ }
  ```
  No dome internals: the core's exit gas reaches the steam lines, the

- ```rust
  pub fn without_penetrations(self: Self) -> Self { /* ... */ }
  ```
  The lower head fails by its wall alone, no penetration: the model

- ```rust
  pub fn without_failed_vessel_outflow(self: Self) -> Self { /* ... */ }
  ```
  A failed vessel passes nothing to the drywell: the model before

- ```rust
  pub fn without_pool_scrubbing(self: Self) -> Self { /* ... */ }
  ```
  No pool scrubbing: everything entering the pool passes to the

- ```rust
  pub fn without_in_vessel_deposition(self: Self) -> Self { /* ... */ }
  ```
  No in-vessel deposition: the model before 2026-10-29.

- ```rust
  pub fn without_internals_heat_loss(self: Self) -> Self { /* ... */ }
  ```
  The dome internals lose no heat once the flow past them stops: the

- ```rust
  pub fn without_scrubbing_saturation(self: Self) -> Self { /* ... */ }
  ```
  Condensation scrubbing as if all the steam condensed: the model

- ```rust
  pub fn without_swarm_scrubbing(self: Self) -> Self { /* ... */ }
  ```
  No impaction or swarm-rise scrubbing: the model before 2026-10-29.

- ```rust
  pub fn without_sectional_classes(self: Self) -> Self { /* ... */ }
  ```
  Each class on the aerosol lumped over its volume distribution: the

- ```rust
  pub fn without_iodine_partition(self: Self) -> Self { /* ... */ }
  ```
  Iodine vapour scrubbed like the gas, no partition limit: the model

- ```rust
  pub fn without_mixture_vent(self: Self) -> Self { /* ... */ }
  ```
  The vent sized for the non-condensables alone: the model before

- ```rust
  pub fn without_phoresis(self: Self) -> Self { /* ... */ }
  ```
  No phoresis onto the drywell's structures: the model before

- ```rust
  pub fn without_iodine_swarm(self: Self) -> Self { /* ... */ }
  ```
  No swarm scrubbing of iodine vapour: the model before 2026-10-29.

- ```rust
  pub fn without_vessel_aerosol(self: Self) -> Self { /* ... */ }
  ```
  No aerosol dynamics in the vessel: the model before 2026-10-29.

- ```rust
  pub fn without_condensate_mass(self: Self) -> Self { /* ... */ }
  ```
  Condensate weightless on the particles: the model before

- ```rust
  pub fn without_fp_decay_follows(self: Self) -> Self { /* ... */ }
  ```
  All decay heat stays in the core: the model before 2026-10-29.

- ```rust
  pub fn without_te_transport(self: Self) -> Self { /* ... */ }
  ```
  No tellurium transport: the model before 2026-10-29.

- ```rust
  pub fn without_debris_oxidation(self: Self) -> Self { /* ... */ }
  ```
  Debris that never oxidises: the model before 2026-10-29.

- ```rust
  pub fn without_candling(self: Self) -> Self { /* ... */ }
  ```
  No candling: the model before 2026-10-29.

- ```rust
  pub fn without_debris_quench(self: Self) -> Self { /* ... */ }
  ```
  The 120 s quench: the model before 2026-10-29.

- ```rust
  pub fn without_wet_head_contact(self: Self) -> Self { /* ... */ }
  ```
  The head boils against the water while there is any: the model

- ```rust
  pub fn without_penetration_coverage(self: Self) -> Self { /* ... */ }
  ```
  The penetration fully covered by any debris: the model before

- ```rust
  pub fn without_failed_vessel_drain(self: Self) -> Self { /* ... */ }
  ```
  A failed vessel keeps its water: the model before 2026-10-29.

- ```rust
  pub fn without_ww_atmosphere(self: Self) -> Self { /* ... */ }
  ```
  The wetwell gas at the pool's temperature and saturated: the model

- ```rust
  pub fn without_dw_wall_hs(self: Self) -> Self { /* ... */ }
  ```
  The drywell structures with the fixed coefficient: the model before

- ```rust
  pub fn without_ww_shell(self: Self) -> Self { /* ... */ }
  ```
  No structure in the wetwell gas: the model before 2026-10-29.

- ```rust
  pub fn without_outflow_enthalpy(self: Self) -> Self { /* ... */ }
  ```
  Drywell outflows at internal energy: the model before 2026-10-29.

- ```rust
  pub fn without_relief_h2_enthalpy(self: Self) -> Self { /* ... */ }
  ```
  The relief valves' hydrogen at the pool's temperature: the model

- ```rust
  pub fn without_condensate_to_floor(self: Self) -> Self { /* ... */ }
  ```
  The drywell's condensate leaves the model: before 2026-10-29.

- ```rust
  pub fn without_rpv_wall(self: Self) -> Self { /* ... */ }
  ```
  No RPV wall: the model before 2026-10-29.

- ```rust
  pub fn without_internals_radiation(self: Self) -> Self { /* ... */ }
  ```
  The internals do not radiate to the wall: before 2026-10-29.

- ```rust
  pub fn without_core_top_radiation(self: Self) -> Self { /* ... */ }
  ```
  The top of the core exchanging nothing: before 2026-10-29.

- ```rust
  pub fn without_see_through(self: Self) -> Self { /* ... */ }
  ```
  No view across an emptied cell: before 2026-10-29.

- ```rust
  pub fn without_mcci(self: Self) -> Self { /* ... */ }
  ```
  No core-concrete interaction: before 2026-10-29.

- ```rust
  pub fn without_dw_wall_outer(self: Self) -> Self { /* ... */ }
  ```
  The drywell structures adiabatic outside: before 2026-10-29.

- ```rust
  pub fn without_ic_h2_blanketing(self: Self) -> Self { /* ... */ }
  ```
  Isolation-condenser condensation unaffected by hydrogen in the vessel.

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Controls`

What the operators (or the historical sequence) have set.

```rust
pub struct Controls {
    pub ic_open: [bool; 2],
    pub dc_power: bool,
    pub relief_held_open: bool,
    pub injection: bool,
    pub vent_ordered: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ic_open` | `[bool; 2]` |  |
| `dc_power` | `bool` |  |
| `relief_held_open` | `bool` | Hold the relief valves open (manual depressurisation; needs DC). |
| `injection` | `bool` |  |
| `vent_ordered` | `bool` |  |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Event`

One recorded moment of the Unit 1 accident, seconds after the scram
(14:46 JST, 11 March 2011). \[U\] Times are from the INPO 11-005 timeline
as recalled, not yet checked against the report.

**Partly checked 2026-10-29 (gh:#1044)** against the observed anchors in
BSAF Phase II (`crates/kovan-literature/derived/nea7525-bsaf-fukushima-daiichi-bands.md`
§1.1; NEA No. 7525, © OECD 2021, cited with acknowledgement):
- the reactor-building explosion, 15:36 on 12 March = 24.83 h, **agrees**
  with BSAF's observed 24.83 h;
- the wetwell vent order, 14:30 = 23.73 h, is **consistent** with BSAF's
  "manually vented ≈24 h" (vent closure 24.68 h, a BSAF boundary
  condition this sequence does not have).

The other five times (IC stop, IC cycling, station blackout, the three
injection changes) have no BSAF anchor and stay `[U]` until checked
against INPO 11-005, IAEA TV1 or the Japanese government report (#1031).

```rust
pub struct Event {
    pub t_s: f64,
    pub label: &'static str,
    pub kind: EventKind,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `t_s` | `f64` |  |
| `label` | `&'static str` |  |
| `kind` | `EventKind` |  |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Enum `EventKind`

```rust
pub enum EventKind {
    IcStopped,
    IcCycleA,
    StationBlackout,
    InjectionOn,
    InjectionOff,
    VentOpened,
    Explosion,
}
```

##### Variants

###### `IcStopped`

Operators stop both isolation condensers (cooldown above 55 degC/h).

###### `IcCycleA`

Operators cycle train A to hold the pressure in a band, until the
station blackout.

###### `StationBlackout`

Tsunami: AC and DC lost; the IC valves fail closed.

###### `InjectionOn`

Fire-engine injection starts (fresh water, later seawater).

###### `InjectionOff`

###### `VentOpened`

The wetwell vent is opened.

###### `Explosion`

Recorded for comparison only: the model computes its own.

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Vessel`

The vessel's state derived from its mass and energy.

```rust
pub struct Vessel {
    pub sat: steam::Sat,
    pub x: f64,
    pub p_pa: f64,
    pub level_m: f64,
    pub gas_m3: f64,
    pub h2_fraction: f64,
    pub dry: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `sat` | `steam::Sat` |  |
| `x` | `f64` |  |
| `p_pa` | `f64` |  |
| `level_m` | `f64` | Collapsed level above the bottom of active fuel, m. |
| `gas_m3` | `f64` |  |
| `h2_fraction` | `f64` |  |
| `dry` | `bool` | True when no saturated mixture matches (boiled dry): an ideal-gas<br>steam fallback, outside what the model represents. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Frame`

Everything the page shows, flattened (see [`Frame::write`]).

```rust
pub struct Frame {
    pub t_s: f64,
    pub mode_free: bool,
    pub controls: Controls,
    pub vessel: Vessel,
    pub decay_w: f64,
    pub q_ic_w: f64,
    pub ic_shell_k: [f64; 2],
    pub ic_shell_kg: [f64; 2],
    pub relief_open: bool,
    pub w_relief_kg_s: f64,
    pub w_bypass_kg_s: f64,
    pub w_leak_kg_s: f64,
    pub w_injection_kg_s: f64,
    pub injected_kg: f64,
    pub rpv_h2_kg: f64,
    pub hot_leak_open: bool,
    pub msl: msl::SteamLine,
    pub internals_k: f64,
    pub dome_gas_k: f64,
    pub xe_vessel: f64,
    pub cs_vessel_kg: f64,
    pub i_vessel_kg: f64,
    pub on_internals_kg: [f64; 2],
    pub core: self::core::CoreResult,
    pub containment: containment::ContainmentResult,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `t_s` | `f64` |  |
| `mode_free` | `bool` |  |
| `controls` | `Controls` |  |
| `vessel` | `Vessel` |  |
| `decay_w` | `f64` |  |
| `q_ic_w` | `f64` |  |
| `ic_shell_k` | `[f64; 2]` |  |
| `ic_shell_kg` | `[f64; 2]` |  |
| `relief_open` | `bool` |  |
| `w_relief_kg_s` | `f64` |  |
| `w_bypass_kg_s` | `f64` |  |
| `w_leak_kg_s` | `f64` |  |
| `w_injection_kg_s` | `f64` |  |
| `injected_kg` | `f64` |  |
| `rpv_h2_kg` | `f64` |  |
| `hot_leak_open` | `bool` |  |
| `msl` | `msl::SteamLine` | The main steam line's wall temperature, K, creep damage, and whether<br>it has ruptured. |
| `internals_k` | `f64` | The dome internals' temperature and the gas leaving them, K. |
| `dome_gas_k` | `f64` |  |
| `xe_vessel` | `f64` | Noble gas (Xe) in the vessel's gas, fraction of inventory (the<br>containment's shares are in `containment`). |
| `cs_vessel_kg` | `f64` | Cesium in the vessel's gas, kg (gh:#1047). |
| `i_vessel_kg` | `f64` |  |
| `on_internals_kg` | `[f64; 2]` | Cesium and iodine on the dome internals, kg. |
| `core` | `self::core::CoreResult` |  |
| `containment` | `containment::ContainmentResult` |  |

##### Implementations

###### Methods

- ```rust
  pub fn write(self: &Self, w: &mut Writer) { /* ... */ }
  ```

- ```rust
  pub fn read(r: &mut Reader<''_>) -> Self { /* ... */ }
  ```

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Plant`

The plant part of the model (see the module docs).

```rust
pub struct Plant {
    pub mode: Mode,
    pub mechanisms: Mechanisms,
    pub controls: Controls,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mode` | `Mode` |  |
| `mechanisms` | `Mechanisms` |  |
| `controls` | `Controls` |  |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(mode: Mode) -> Self { /* ... */ }
  ```

- ```rust
  pub fn with_mechanisms(mode: Mode, mechanisms: Mechanisms) -> Self { /* ... */ }
  ```
  A fresh plant with some mechanisms ablated (see [`Mechanisms`]).

- ```rust
  pub fn with_params(mode: Mode, mechanisms: Mechanisms, prm: Params) -> Self { /* ... */ }
  ```
  A fresh plant with its own inputs (gh:#1033), for a corrected or

- ```rust
  pub fn parts(self: &Self) -> (Core, Containment) { /* ... */ }
  ```
  A fresh core and containment matching a fresh plant.

- ```rust
  pub fn params(self: &Self) -> &Params { /* ... */ }
  ```
  The inputs this plant runs with.

- ```rust
  pub fn time_s(self: &Self) -> f64 { /* ... */ }
  ```

- ```rust
  pub fn begin(self: &mut Self) -> Snapshots { /* ... */ }
  ```
  Start a step: decide the flows from the start-of-step state and hand

- ```rust
  pub fn finish(self: &mut Self, core: CoreResult, containment: ContainmentResult) { /* ... */ }
  ```
  Finish the step with the core's and the containment's results.

- ```rust
  pub fn gas_out_k(self: &Self) -> f64 { /* ... */ }
  ```
  The temperature of the gas leaving the vessel through the steam

- ```rust
  pub fn step_inline(self: &mut Self, core: &mut Core, containment: &mut Containment) { /* ... */ }
  ```
  One whole step with the core and containment advanced here, in

- ```rust
  pub fn frame(self: &Self) -> Frame { /* ... */ }
  ```

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Snapshots`

The core and containment snapshots of one step.

```rust
pub struct Snapshots {
    pub core: self::core::CoreSnapshot,
    pub containment: containment::ContainmentSnapshot,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `core` | `self::core::CoreSnapshot` |  |
| `containment` | `containment::ContainmentSnapshot` |  |

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
- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

#### Function `mixture_flux_factor`

Choked mass flux of a hydrogen-steam gas with hydrogen mole fraction
`x_h2`, relative to pure steam at the same pressure and temperature:
`sqrt(M_mix / M_H2O)` (ideal gas, fixed gamma).

```rust
pub fn mixture_flux_factor(x_h2: f64) -> f64 { /* ... */ }
```

#### Function `swollen_level`

The swollen two-phase mixture level above the bottom of active fuel, m,
from the collapsed one (gh:#1036), after MELCOR's CVH pool model (MELCOR
2.2 Vol. 2, CVH/FL §5.1.3, Eqs 5-26 to 5-30): bubbles rise at
`bubble_rise_m_s`, so with the vapour flux through the pool top `J` the
mean void is `J / (2 v0)`, capped at `max_pool_void`, and the swollen pool
fills `V_liquid / (1 - alpha)`. `q_pool_w` W raise vapour of specific
volume `v_g` with latent heat `h_fg`; `J` is over the free cross-section
at the collapsed level.

```rust
pub fn swollen_level(prm: &Params, level_m: f64, q_pool_w: f64, h_fg: f64, v_g: f64) -> f64 { /* ... */ }
```

#### Function `volume_below`

Free volume of the vessel below height `z` above its bottom, m^3.

```rust
pub fn volume_below(prm: &Params, z: f64) -> f64 { /* ... */ }
```

#### Function `height_of_volume`

Height (above the vessel bottom) at which the free volume below is `v`.

```rust
pub fn height_of_volume(prm: &Params, v: f64) -> f64 { /* ... */ }
```

#### Function `csv_row`

```rust
pub fn csv_row(f: &Frame) -> String { /* ... */ }
```

#### Function `headless_csv`

The headless run: `steps` steps of the historical sequence, one CSV row
every `every` steps. Deterministic: no clock, no randomness, no I/O.

```rust
pub fn headless_csv(steps: usize, every: usize) -> String { /* ... */ }
```

### Constants and Statics

#### Constant `DT_S`

The plant's time step, s.

```rust
pub const DT_S: f64 = 1.0;
```

#### Constant `SCENARIO_END_S`

Seconds of the accident the historical sequence runs to (30 h).

```rust
pub const SCENARIO_END_S: f64 = _;
```

#### Constant `EVENTS`

```rust
pub const EVENTS: [Event; 9] = _;
```

#### Constant `CSV_HEADER`

```rust
pub const CSV_HEADER: &str = "t_s,p_rpv_mpa,level_m,decay_mw,q_ic_mw,w_relief_kg_s,w_injection_kg_s,peak_clad_k,exit_gas_k,h2_kg,relocated,p_dw_mpa,p_ww_mpa,t_pool_k,rb_h2_fraction,flags";
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
    NoVentingTransport,
    Inventory(outram_mc_libs::inventory::InventoryError),
    InventoryNotInRings(Vec<String>),
    InventoryRingMismatch {
        inventory: usize,
        transient: usize,
    },
    InventoryRingOutOfRange {
        ring: usize,
        rings: usize,
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

###### `NoVentingTransport`

`Venting::Upstream` on a field that is **constant in time but not
uniform in space** (#447). Upstream raises `IndexError` here, and the
port used to return a silent 0 Bq. TRISO-ATOPS transports activity only
by heat-up expansion, which is zero here, but a real core still leaks,
convects and is ventilated.

###### `Inventory`

The inventory could not be built (empty, a ring count that disagrees,
a negative or non-finite amount, a nuclide twice). See
`outram_mc_libs::inventory::InventoryError` (GitHub #951).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `outram_mc_libs::inventory::InventoryError` |  |

###### `InventoryNotInRings`

The inventory is over named regions (components, depletion zones), and
this crate needs radial rings. State the mapping with
`RadionuclideInventory::with_regions`; this crate does not guess it.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Vec<String>` |  |

###### `InventoryRingMismatch`

The inventory and the temperature transient disagree on the number of
radial rings. ~~A panic~~ An error since GitHub #951.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `inventory` | `usize` | Rings in the inventory. |
| `transient` | `usize` | Rings in the transient. |

###### `InventoryRingOutOfRange`

A ring index past the inventory's last ring.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `ring` | `usize` | The ring asked for. |
| `rings` | `usize` | Rings in the inventory. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
  - ```rust
    fn source(self: &Self) -> ::core::option::Option<&dyn ::thiserror::__private21::Error + ''static> { /* ... */ }
    ```

- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

  - ```rust
    fn from(source: outram_mc_libs::inventory::InventoryError) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
| `first_sample_forced_fully_vented` | `bool` | Upstream's `coolant_release` hard-codes `frac[0] = 1`, so the **first<br>sample is always treated as fully vented** regardless of what the<br>integral says. ~~Always true when a venting calculation ran; recorded so<br>the first window's release is not read as a physical result.~~<br>**CORRECTED 2026-09-30 (#449):** set only when `coolant_release` ran, so it is false<br>for the uniform-constant branch, `FullFlowThrough` and `Prescribed`.<br>And the forced value does **not** enter the source term: the per-window<br>conversion starts from `cumulative[1]`, so `frac[0]` is never used.<br>It is informational. |
| `negative_atom_count_seen` | `bool` | A release somewhere in the chain went **negative**. Two different things<br>set this, and **they push the total in opposite directions**, so the<br>flag alone does not tell a reader which way the answer is wrong.<br><br>~~If this is set, at least one node-nuclide pair went negative and the<br>total is correspondingly under-stated.~~ **CORRECTED 2026-09-24** — that<br>was right for one of the two paths and wrong for the other:<br><br>1. **`release_activity` returning a negative atom count.** Upstream<br>   deliberately does not clamp it, and neither does this crate, so the<br>   negative propagates and the total is **under-stated**. This path<br>   needs a non-empty normal-operation pool to fire at all: with<br>   [`crate::accident::release::zero_pools`] every subtracted term is<br>   zero, so it cannot. **Since #448 (2026-09-30) real pools are the<br>   default, so this path CAN fire** (measured: Cs-137 in the HTR-10<br>   DLOFC case), and the flag no longer tells a reader which direction<br>   the total is wrong.<br>2. **A negative per-window first difference**, i.e. a *non-monotonic*<br>   cumulative release. That one is floored to zero at the<br>   [`changi::activity::source::SourceTerm`] boundary, which **raises**<br>   the sum of the windows above the cumulative endpoint — the total is<br>   **over-stated**.<br><br>Path 2 is reachable and is not hypothetical. **Silver** is the case:<br>the transient breakthrough release fraction<br>(`boon_lay::triso_atops_fork::release_models::transient::breakthrough_model_transient`)<br>rises, is then driven negative by its `−a/(2r)` time-lag term and<br>clamped to zero until breakthrough, and only then grows — so the<br>cumulative curie series falls over that stretch. Measured on the HTR-10<br>DLOFC case (`crate::htr10`, 2026-09-24): Ag-110m had **13 of 30 windows<br>negative**, and the windows sum to `2.503345e-2 Ci` against a cumulative<br>endpoint of `2.500569e-2 Ci` — **over-stated by a factor 1.0011**. No<br>other nuclide in that run had a single negative window. |
| `diffusion_coefficient_clamped` | `bool` | The transient left the correlation's nominal **700-2400 °C** fitted<br>range. ~~"clamped, never extrapolated"~~ **CORRECTED 2026-09-30 (#449):** `boon-lay`<br>clamps only group-specific **lower** limits (Rb/Cs kernel 700, graphite<br>550; Sr/Ba/Eu kernel 700, graphite 800; Ag/Pd graphite 490 °C). It never<br>clamps above, and never clamps Kr, Xe, I, Te or Se. So this flag<br>**over-flags** some nuclides (it is set for noble gases, which are<br>extrapolated, not clamped) and **misses** Sr graphite clamping between<br>700 and 800 °C. Read it as "outside the fitted range", not "clamped". |
| `booth_transient_floored` | `bool` | The transient Booth solution **floors at about 1.216e-4** rather than<br>reaching zero, so a nuclide that should release essentially nothing<br>still shows a small release fraction.<br><br>**NOT WIRED — nothing in this crate ever sets this field, so it is<br>always `false` on a computed result.** Stated here because a reader<br>finding it in a caveat struct would reasonably assume the condition is<br>detected, and it is not: verified 2026-09-24 by searching the workspace<br>for writes to it, which occur only in this module's own tests. It is<br>kept rather than deleted because the underlying behaviour is real —<br>`boon_lay::...::release_models::transient::booth_transient` snaps below<br>`BOOTH_TRANSIENT_ZERO_FLOOR = 1e-6` — and detecting it needs the<br>release-fraction values, which<br>[`crate::accident::release::accident_release`] does not currently keep. |
| `venting_mask_was_gappy` | `bool` | The venting mask was **not contiguous**. This is the condition under<br>which upstream's own prefix-versus-selection pairing goes wrong — see<br>[`crate::accident::venting`]. This crate does not have that defect, but<br>a result computed on a gappy mask cannot be compared against upstream's. |

##### Implementations

###### Methods

- ```rust
  pub const fn any(self: Self) -> bool { /* ... */ }
  ```
  Whether anything worth reporting happened. ~~Always true in practice —

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

The pools and the lift-off fraction are the named
[`primary_circuit_pools`] and [`X_LIFTOFF_NP_MHTGR_STAND_IN`] (#469), so
`htgr_sim_v1` reads the same source rather than a second set of literals.

```rust
pub fn plant_parameters(geometry: Htr10Geometry, fractions: boon_lay::triso_atops_fork::accident::AccidentFractions) -> crate::accident::release::PlantParameters { /* ... */ }
```

#### Function `primary_circuit_pools`

**Attributes:**

- `MustUse { reason: None }`

The HTR-10 **primary-circuit pools** an accident starts from:
[`PrimaryCircuitPools::FromNormalOperation`] over

- `run_time` [`OPERATING_HISTORY_S`] (20 y, Liu & Cao 2002);
- `irradiation_time` **1080 FPD**,
  `boon_lay::fuel_failure::htr10::RESIDENCE_FULL_POWER_DAYS`;
- `k_plate`, `k_clean`, `a_grain`: **NP-MHTGR stand-ins**
  ([`NormalOperation::np_mhtgr_reference`]);
- fuel and graphite at the **776 °C stand-in**
  [`STAND_IN_IRRADIATION_CELSIUS`], uniform (#297).

The named constructor #469 item 3 asks for; [`plant_parameters`] uses it.

```rust
pub fn primary_circuit_pools() -> crate::accident::release::PrimaryCircuitPools { /* ... */ }
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

#### Constant `LIU_CAO_DESIGN_FREE_URANIUM`

HTR-10 design **as-manufactured free uranium**, `< 3·10⁻⁴`: Liu & Cao
2002 §2.1 (p.82), a design specification, not a measurement
(`crates/kovan-literature/derived/liu-cao2002-htr10-source-term-assumptions.md` §1).
TRISO-ATOPS's `f_hm` in the bounding case and the audited example.

```rust
pub const LIU_CAO_DESIGN_FREE_URANIUM: f64 = 3.0e-4;
```

#### Constant `LIU_CAO_DESIGN_IRRADIATION_FAILURE`

HTR-10 design **particle failure fraction from irradiation**, `5·10⁻⁴`:
Liu & Cao 2002 §2.1 (p.82), a design specification. TRISO-ATOPS's `f_inc`,
the in-service failure that accumulates as the fuel is irradiated
(#469 item 1).

**Why this value (maintainer-accepted 2026-10-01, #471).** It is the
published HTR-10 design figure for irradiation-induced failure; with the
free uranium above it makes up Liu & Cao's total `< 8·10⁻⁴` at **maximum
design burnup**. Applying it to the whole core is therefore conservative:
the simulator's fuel sits at the 1080 FPD mean residence, not at the
discharge burnup. It is not the `5·10⁻³` Liu & Cao used for their Table 3
helium activity, which they call *"somewhat arbitrarily set"*. Source:
`crates/kovan-literature/derived/liu-cao2002-htr10-source-term-assumptions.md`.

Read by `htgr_sim_v1` and by
[`crate::lwr_comparison::bound::F_INC`].

```rust
pub const LIU_CAO_DESIGN_IRRADIATION_FAILURE: f64 = 5.0e-4;
```

#### Constant `X_LIFTOFF_NP_MHTGR_STAND_IN`

Plate-out lift-off fraction `x_liftoff`, **0.05, an NP-MHTGR stand-in**
(Stoyer, Raichart & Petti, Nucl. Technol. 2026, Case A, Table 3). No HTR-10
value is in the corpus; Liu & Cao 2002 §4.1.1.2 instead assume desorption
of 2.4 × the coolant activity, which `htgr_sim_v1`'s live DLOFC applies.

```rust
pub const X_LIFTOFF_NP_MHTGR_STAND_IN: f64 = 0.05;
```

#### Constant `OPERATING_HISTORY_S`

Full-power operating history the primary-circuit pools have accumulated
over \[s\]: **20 years** of 365 d, Liu & Cao 2002's circulating-activity
basis ("at the end of 20 years of full-power operation", Table 3). One copy:
[`primary_circuit_pools`] and `htgr_sim_v1`'s live-pool opening history both
read it (#469).

```rust
pub const OPERATING_HISTORY_S: f64 = _;
```

## Module `inventory`

The core radionuclide inventory: outram-mc's [`RadionuclideInventory`],
read in TRISO-ATOPS's terms.

# One inventory type (GitHub #951)

~~This crate's own `CoreInventory` / `NuclideInventory`~~ **REPLACED
2026-10-17 (maintainer: "Change sembawang to use RadionuclideInventory
instead"):** the inventory is `outram_mc_libs::inventory::RadionuclideInventory`,
the type a depletion run produces, re-exported here. What changed and what
did not:

| `CoreInventory` held | Now |
|---|---|
| `name: String` per nuclide, a TRISO-ATOPS spelling | an [`Isotope`] (Z, A, metastable state). [`prescribed`] parses names through upstream's own normaliser, so `cs137` is still accepted; [`triso_atops_name`] spells it back (`Ag-110m`) |
| `per_ring: Vec<Radioactivity>` | one [`Amount`] per region, whose activity is the same `Radioactivity` value, untouched |
| `n_radial` | [`Regions::RadialRings`]; [`Regions::WholeCore`] reads as one ring |
| `n_axial` | **dropped**: it duplicated the temperature transient's `n_axial`, which is what the axial split has always used. The axial split itself is still upstream's `distribute_inventory_axially`, in [`axial_curies`] |

So every number this crate produced before the change is reproduced bit
for bit (`tests/inventory_handoff_fingerprints.rs`). A non-canonical
spelling now comes back canonical in `AccidentRelease::screened_out`
(`cs137` is reported as `Cs-137`).

# A prescribed inventory is still an INPUT, and this crate does not compute it

Nothing here derives an inventory from power history, burnup or fission
yields. A caller prescribes one ([`prescribed`], [`unit()`]) or hands over
one a depletion code produced (`RadionuclideInventory::from_burnup_step`
in outram-mc, then [`restrict_to_triso_atops`]). The reason the obvious
shortcut is wrong is worth keeping, because it does not announce itself:

**Do not build one from `fission-yields-data`.** That crate exposes
**independent** fission yields — the yield *directly* from fission, before
any beta decay. Most of the nuclides that dominate a source term do not
arrive that way. Cs-137's independent yield is roughly **two orders of
magnitude** below its cumulative yield, because Cs-137 is reached down the
A = 137 isobaric chain (Xe-137 to Cs-137) rather than produced directly.
Using independent yields naively gives a silently ~100x low inventory for
most consequence-dominant species — low, so it looks reassuring, and with
no error anywhere. A depletion run that walks the decay chain does not
have this problem; that is the outram-mc path.

# Radial and axial distribution

An inventory is given **per radial ring**, innermost first, and spread over
the axial nodes by `boon-lay`'s `distribute_inventory_axially`, which is
upstream's own distribution. This crate does not invent a shape. An
inventory over **named** regions (components, depletion zones) is refused
until the caller states the ring mapping with
`RadionuclideInventory::with_regions`.

```rust
pub mod inventory { /* ... */ }
```

### Functions

#### Function `triso_atops_name`

**Attributes:**

- `MustUse { reason: None }`

The TRISO-ATOPS spelling of `isotope`: `Cs-137`, `Ag-110m` (first isomer
`m`). A second or higher isomer is spelled `m2`, ...; no TRISO-ATOPS
nuclide is one and upstream's name parser does not accept that spelling,
so such a nuclide is refused by the release as unknown (reported, never
dropped).

```rust
pub fn triso_atops_name(isotope: Isotope) -> String { /* ... */ }
```

#### Function `isotope_from_triso_atops_name`

Parse a nuclide name the way upstream's `nuclide_import_accident` does
(`cs137`, `CS-137` and `Cs-137` are one nuclide), into an [`Isotope`].

# Errors
[`Error::UnknownNuclide`] when the name does not parse or names no nuclide.

```rust
pub fn isotope_from_triso_atops_name(name: &str) -> crate::error::Result<Isotope> { /* ... */ }
```

#### Function `prescribed`

**A prescribed inventory**: activity per radial ring, innermost first, by
nuclide name, in the given order (a parent must precede its daughter for
the normal-operation solve to see its pools). `reference` says where the
numbers come from.

# Errors
[`Error::UnknownNuclide`] for a name that does not parse, and
[`Error::Inventory`] for an empty list, a ring count that disagrees with
`n_radial`, a negative or non-finite activity, or a nuclide given twice.

```rust
pub fn prescribed<S: AsRef<str>>(entries: Vec<(S, Vec<uom::si::f64::Radioactivity>)>, n_radial: usize, reference: &str) -> crate::error::Result<RadionuclideInventory> { /* ... */ }
```

#### Function `unit`

**A shakedown inventory: exactly one curie of each nuclide, per ring.**

Not a physical inventory and not intended to be one. It exists so the
chain can be run and its *shape* checked — the arithmetic is linear in
the inventory, so a run with this and a run with a real one differ only
by a per-nuclide scale factor. Anything reported from it is a
demonstration, never a result.

Use it to answer "does the plumbing work", never "how much comes out".

# Errors
As [`prescribed`].

```rust
pub fn unit(names: &[&str], n_radial: usize) -> crate::error::Result<RadionuclideInventory> { /* ... */ }
```

#### Function `n_rings`

The number of radial rings `inventory` is resolved into.

# Errors
[`Error::InventoryNotInRings`] for [`Regions::Named`]: which ring a
component is in is the caller's statement to make
(`RadionuclideInventory::with_regions`), not this crate's guess.

```rust
pub fn n_rings(inventory: &RadionuclideInventory) -> crate::error::Result<usize> { /* ... */ }
```

#### Function `names`

**Attributes:**

- `MustUse { reason: None }`

The TRISO-ATOPS names of every nuclide, in order, for handing to
`select_nuclides_accident`.

```rust
pub fn names(inventory: &RadionuclideInventory) -> Vec<String> { /* ... */ }
```

#### Function `axial_curies`

One nuclide's inventory in one ring, spread over `n_axial` nodes.

Delegates to upstream's `distribute_inventory_axially`; the returned
values are in **curies**, which is the unit TRISO-ATOPS works in
internally.

# Errors
[`Error::InventoryRingOutOfRange`] if `ring` is past the last region.

```rust
pub fn axial_curies(entry: &NuclideEntry, ring: usize, n_axial: usize) -> crate::error::Result<Vec<f64>> { /* ... */ }
```

#### Function `restrict_to_triso_atops`

Keep the nuclides TRISO-ATOPS models (its 84-entry table), in order, and
**report** the rest rather than dropping them silently: the path for an
inventory from depletion, which carries every chain nuclide, stable ones
included.

# Errors
[`Error::Inventory`] if no nuclide is left.

```rust
pub fn restrict_to_triso_atops(inventory: &RadionuclideInventory) -> crate::error::Result<(RadionuclideInventory, Vec<Isotope>)> { /* ... */ }
```

### Re-exports

#### Re-export `zai`

```rust
pub use outram_mc_libs::inventory::zai;
```

#### Re-export `Amount`

```rust
pub use outram_mc_libs::inventory::Amount;
```

#### Re-export `InventoryError`

```rust
pub use outram_mc_libs::inventory::InventoryError;
```

#### Re-export `InventoryPoint`

```rust
pub use outram_mc_libs::inventory::InventoryPoint;
```

#### Re-export `InventorySource`

```rust
pub use outram_mc_libs::inventory::InventorySource;
```

#### Re-export `NuclideEntry`

```rust
pub use outram_mc_libs::inventory::NuclideEntry;
```

#### Re-export `RadionuclideInventory`

```rust
pub use outram_mc_libs::inventory::RadionuclideInventory;
```

#### Re-export `Regions`

```rust
pub use outram_mc_libs::inventory::Regions;
```

#### Re-export `Isotope`

```rust
pub use outram_mc_libs::model::Isotope;
```

## Module `lwr_comparison`

# HTR-10 against an equivalent-power LWR (GitHub #450, #452, #453, #464)

~~**Framing (maintainer decisions, 2026-09-30, #450).** PRIMARY:
**design basis against design basis** ... SECONDARY: the
**beyond-design-basis bounding** pair, the KORA bound against WASH-1400
PWR 8.~~ **CHANGED 2026-09-30 (maintainer decision, #464): paired by
INITIATING EVENT and severity**, DLOFC (HTR) against LOCA (LWR), in two
tiers ([`Tier`]):

- **Design basis: DLOFC vs LOCA.** HTR-10 depressurisation
  ([`htr10_dba_release`], Liu & Cao Table 8) against the LWR MHA LOCA
  ([`nuscale_mha_loca`], RG 1.183 Rev. 1).
- **Beyond design basis: DLOFC + air ingress (bounding) vs LOCA + core
  melt.** The KORA bound ([`htr10_air_ingress_bound`]) against the LWR
  LOCA with ECCS failure ([`nuscale_severe_loca`], NUREG-1465 Table 3.13,
  all phases, into an **intact** containment leaking at `L_a`).
- **Context:** WASH-1400 PWR 8, no-melt, uncontained; paired by
  containment state, not initiator.

[`bounding_comparison`] returns every arm, in tier order ([`ARM_COLUMNS`]).

**Crediting basis (maintainer decision, 2026-09-30):** "The comparison
neglects the pools because we are comparing technology at the reactor
level, not what is surrounding the reactor. If NuScale gives pool credit,
then HTGR can be submerged in a pool as well." Each side is credited
**only with its reactor-level inherent barrier**: for the LWR, the
containment vessel leaking at `L_a`; for the HTR, the TRISO particles.
Nothing surrounding the reactor is credited on either side (NuScale's
reactor pool, reactor building, sprays, filters; the HTR confinement or
building, #409, or a hypothetical pool). Pool scrubbing is excluded **by
design**, not pending literature. In-containment natural deposition is part
of the containment barrier, so that arm stays (pending literature).
[`REACTOR_LEVEL_BASIS`] carries this wherever the comparison is printed.

> **Research, education and V&V only** (`RESPONSIBLE_USE.md`). Nothing here
> is a source term, a dose or a siting argument for HTR-10, NuScale or any
> plant. The HTR-10 arm is a **bounding case, not a transient** (#420); the
> LWR arms are **published design / risk-study source terms with different
> accident physics** (#450). The comparison puts them through the **same**
> dispersion and dose arithmetic so the difference is the source term alone.

One library function per arm, so the `lwr_nureg1465_counterpart` example
and `htgr_sim_v1`'s map read the same numbers (maintainer direction,
#452/#453):

| Arm | Function | Boundary | What it is |
|---|---|---|---|
| **DB: HTR-10 DLOFC** | [`htr10_dba_release`] | to the environment | Liu & Cao (2002) Table 8, published (depressurisation; water ingress); cross-checked with [`htr10_dba_vs_table9`] |
| **DB: LWR LOCA** | [`nuscale_mha_loca`] | to the environment via containment leakage | RG 1.183 Rev. 1 MHA LOCA, NuScale inventory, `L_a` = 0.20 %/day ([`NUSCALE_LA_PERCENT_PER_DAY`]); no removal, and natural deposition ([`NaturalDeposition`], pending literature) |
| **BDB: HTR-10 DLOFC + air ingress** | [`htr10_air_ingress_bound`] | to the environment (no building credit, #409) | 1400 °C / 140 h failure fractions, KORA f_ox, TRISO-ATOPS release over the dose window, vented full flow-through ([`AIR_INGRESS_VENTING`], #469), + Liu & Cao circulating at 100 % |
| **BDB: LWR LOCA + core melt** | [`nuscale_severe_loca`] | to the environment via containment leakage | NUREG-1465 Table 3.13 PWR, all four phases, Table 3.6 timing ([`nureg1465_pwr_phases`]); intact containment at `L_a`; no removal, and natural deposition (pending literature). [`CONTAINED_CORE_MELT_ASSUMPTION`] |
| Context: WASH-1400 PWR 8 | [`wash1400_pwr8_to_atmosphere`] | **to the atmosphere** | Table 5-1: gap release, containment not isolated, no core melt (#451) |
| LWR, NUREG-1465 | [`nureg1465_pwr_into_containment`] | **into containment** | Table 3.13 (PWR), all four phases or gap + early in-vessel |
| LWR, RG 1.183 Rev. 1 | [`rg1183_pwr_into_containment`], [`rg1183_containment_leakage`] | into containment; then **to the environment** at the TS leak rate `L_a` | Table 2 (MHA LOCA), Table 5 timing, Appendix A-2.7 leakage |

Both LWR LOCA arms leak through one integrator, [`containment_leak`], and
differ only in the phased source they feed it.

The LWR inventory is NuScale's Table B-5 (one module) scaled by thermal
power, [`pwr_inventory_scaled`]; the 160 MWt module power is the
maintainer's attribution (see the CSV header).

**`L_a` is plant-specific and not in RG 1.183.** [`rg1183_containment_leakage`]
takes it as an input and also returns the release per unit `L_a`
(the small-leak limit), so a caller without a sourced `L_a` reports
"per 1 %/day" rather than inventing one. **Since 2026-09-30** NuScale's is
sourced: 0.20 wt%/day (NRC Phase 4 SER Ch. 6, PDF p. 91), used by
[`nuscale_mha_loca`].

~~**Removal credit taken in containment: none.** ... the iodine species
split (A-1.1: 95 % CsI, 4.85 % elemental, 0.15 % organic) is therefore
not needed for transport.~~ **CHANGED 2026-09-30:** two arms. (i) No
removal credit. (ii) **Natural deposition only**, as App. A-2.2 allows
(model: SRP 6.5.2, or NUREG/CR-6189 case by case), through
[`rg1183_leak`] with explicit rates and the A-1.1 species split. The rates
are **pending literature** ([`NATURAL_DEPOSITION_PENDING`]) and never
defaulted. Sprays, filters and scrubbing are not credited. Decay during
hold-up is applied. The containment is scaled down with power
([`scaled_containment`], maintainer decision; an assumption).

Dose: [`max_dose`], the same `buangkok` single-plume Gaussian, FGR-15
submersion and groundshine, FGR-11 inhalation, adult, worst stability class
at 1 m/s, as `examples/htr10_air_ingress_kora_bound.rs`, whose chain
[`htr10_air_ingress_bound`] reproduces.

```rust
pub mod lwr_comparison { /* ... */ }
```

### Modules

## Module `bound`

The bounding case's constants, with the sources of
`examples/htr10_air_ingress_kora_bound.rs` (the maintainer's bounding case
B, 2026-09-30): whole core 1400 °C for 140 h, every particle exposed.

```rust
pub mod bound { /* ... */ }
```

### Constants and Statics

#### Constant `HOLD_CELSIUS`

Hold temperature \[°C\].

```rust
pub const HOLD_CELSIUS: f64 = 1400.0;
```

#### Constant `HOLD_HOURS`

Hold duration the failure fractions are taken at \[h\].

```rust
pub const HOLD_HOURS: f64 = 140.0;
```

#### Constant `F_HM`

f_hm, Liu & Cao 2002 s.2.1 (HTR-10 design free uranium); read from
[`crate::htr10::LIU_CAO_DESIGN_FREE_URANIUM`], one copy (#469).

```rust
pub const F_HM: f64 = crate::htr10::LIU_CAO_DESIGN_FREE_URANIUM;
```

#### Constant `F_INC`

f_inc, Liu & Cao 2002 s.2.1 (design irradiation failure); read from
[`crate::htr10::LIU_CAO_DESIGN_IRRADIATION_FAILURE`], one copy (#469).

```rust
pub const F_INC: f64 = crate::htr10::LIU_CAO_DESIGN_IRRADIATION_FAILURE;
```

#### Constant `F_SIC_STAND_IN`

f_sic, **stand-in** (NP-MHTGR reference; no HTR-10 value).

```rust
pub const F_SIC_STAND_IN: f64 = 1.0e-4;
```

#### Constant `F_INC_SIC_STAND_IN`

f_inc_sic, **stand-in** (NP-MHTGR).

```rust
pub const F_INC_SIC_STAND_IN: f64 = 3.6e-5;
```

#### Constant `F_OX_KORA`

KORA AVR 92/22: about 20 of 16 400 particles failed in air at 1400 °C
for 140 h (IAEA-TECDOC-978 Table 5-7 = Kugeler 2017 Table 9). Pinned
to the committed Table 5-7 row by
`tests::f_ox_kora_is_the_table_5_7_sphere_test` (#453).

```rust
pub const F_OX_KORA: f64 = 1.2e-3;
```

#### Constant `BL_STEPS`

boon-lay fuel-failure integration steps over the hold.

```rust
pub const BL_STEPS: usize = 200;
```

## Module `kora`

IAEA-TECDOC-978 (IAEA, Vienna, 1997) air-oxidation fuel data, the
maintainer's kovan digitisations, committed under `reference/tecdoc978/`
with their provenance (#453). Proprietary tier: the values are cited, the
PDF is not redistributed.

```rust
pub mod kora { /* ... */ }
```

### Types

#### Struct `HeatingTest`

One row of TECDOC-978 Table 5-7 (KORA heating tests in air, Kr-85
release).

```rust
pub struct HeatingTest {
    pub sample: String,
    pub particles: f64,
    pub max_celsius: f64,
    pub hours: f64,
    pub failed: f64,
    pub failed_fraction: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `sample` | `String` | Fuel sample (e.g. `AVR 92/22`). |
| `particles` | `f64` | Particles in the sample. |
| `max_celsius` | `f64` | Maximum temperature \[°C\]. |
| `hours` | `f64` | Time at temperature \[h\]. |
| `failed` | `f64` | Failed particles. |
| `failed_fraction` | `f64` | Printed fraction of failed particles. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

#### Function `table_5_7`

Table 5-7, every row.

# Panics
Never for the shipped CSV (a test parses it).

```rust
pub fn table_5_7() -> Vec<HeatingTest> { /* ... */ }
```

#### Function `sphere_test`

The Table 5-7 **whole-sphere** test (16 400 particles; the 10-particle
rows are loose particles, not fuel in a sphere) at `celsius` for
`hours`, if Table 5-7 has one.

```rust
pub fn sphere_test(celsius: f64, hours: f64) -> Option<HeatingTest> { /* ... */ }
```

#### Function `fig_5_23`

One Fig. 5-23 series, by its legend name: `(hours, failure fraction)`,
in digitised order.

```rust
pub fn fig_5_23(series: &str) -> Vec<(f64, f64)> { /* ... */ }
```

#### Function `nabielek_1400c_prediction`

Fig. 5-23's **1400 °C Nabielek prediction** (dashed line) at `hours`,
interpolated log-linearly between the digitised points (the figure's
y axis is logarithmic). `None` outside the digitised range. A
**model prediction** shown for context, not a measurement, and not
used in the bound.

```rust
pub fn nabielek_1400c_prediction(hours: f64) -> Option<f64> { /* ... */ }
```

## Module `n1465_timing`

NUREG-1465 PWR release-phase timing, **Table 3.6** "Release Phase
Durations for PWRs and BWRs" (printed p. 9, PDF p. 18) and s.3.3 (printed
pp. 7-9), in hours.

```rust
pub mod n1465_timing { /* ... */ }
```

### Constants and Statics

#### Constant `GAP_ONSET_H`

End of the coolant-activity phase = gap onset \[h\]: Table 3.6 gives
"10 to 30 seconds" for PWRs without leak-before-break approval; the
**30 s** end of that range is taken (it is also RG 1.183's 0.5 min).

```rust
pub const GAP_ONSET_H: f64 = _;
```

#### Constant `GAP_H`

Gap-activity phase duration \[h\] (Table 3.6; = Table 3.13 header).

```rust
pub const GAP_H: f64 = 0.5;
```

#### Constant `EARLY_IN_VESSEL_H`

Early in-vessel duration \[h\] (Table 3.6; = Table 3.13 header). It
ends at **vessel breach** (s.3.3, p. 8).

```rust
pub const EARLY_IN_VESSEL_H: f64 = 1.3;
```

#### Constant `EX_VESSEL_H`

Ex-vessel duration \[h\], from vessel breach (Table 3.6, s.3.3 p. 9).

```rust
pub const EX_VESSEL_H: f64 = 2.0;
```

#### Constant `LATE_IN_VESSEL_H`

Late in-vessel duration \[h\]. It "commences at vessel breach and
proceeds simultaneously with the occurrence of the ex-vessel phase"
(s.3.3, p. 9), so it starts with the ex-vessel phase, not after it.

```rust
pub const LATE_IN_VESSEL_H: f64 = 10.0;
```

#### Constant `VESSEL_BREACH_H`

Vessel breach \[h\]: gap onset + gap + early in-vessel = 1.808 h.

```rust
pub const VESSEL_BREACH_H: f64 = _;
```

### Types

#### Type Alias `Releases`

Activity released per nuclide \[Bq\].

```rust
pub type Releases = Vec<(String, f64)>;
```

#### Enum `N1465Phases`

Which NUREG-1465 phases to include.

```rust
pub enum N1465Phases {
    GapAndEarlyInVessel,
    All,
}
```

##### Variants

###### `GapAndEarlyInVessel`

Gap + early in-vessel (the part a design-basis LOCA conventionally uses).

###### `All`

All four phases (gap, early in-vessel, ex-vessel, late in-vessel).

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Leakage`

RG 1.183 containment -> environment for one leak rate.

```rust
pub struct Leakage {
    pub at_la: Option<Releases>,
    pub per_percent_per_day: Releases,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `at_la` | `Option<Releases>` | Released to the environment at the given `L_a` \[Bq per nuclide\];<br>`None` when no `L_a` was supplied. |
| `per_percent_per_day` | `Releases` | Released per unit `L_a` in the small-leak limit \[Bq per nuclide per<br>(1 %/day)\] -- what a caller without a sourced `L_a` reports. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `ReleasePhase`

One phase of a release **into containment**: `fraction` of the core
inventory, released **linearly** between `onset_s` and `end_s` \[s after
the initiating event\].

```rust
pub struct ReleasePhase {
    pub fraction: f64,
    pub onset_s: f64,
    pub end_s: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `fraction` | `f64` | Fraction of core inventory released in this phase. |
| `onset_s` | `f64` | Phase onset \[s\]. |
| `end_s` | `f64` | Phase end \[s\]. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `NaturalDeposition`

Natural-deposition removal rates in the containment **at the NuScale
module** (160 MWt) \[1/s\]: an explicit input, never defaulted. `None`
means pending literature ([`NATURAL_DEPOSITION_PENDING`]).

```rust
pub struct NaturalDeposition {
    pub aerosol_per_s: Option<f64>,
    pub elemental_iodine_per_s: Option<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `aerosol_per_s` | `Option<f64>` | Aerosol removal rate at the reference containment \[1/s\]. |
| `elemental_iodine_per_s` | `Option<f64>` | Elemental-iodine removal rate at the reference containment \[1/s\]. |

##### Implementations

###### Methods

- ```rust
  pub fn at_power(self: &Self, thermal_power_mwth: f64) -> Option<(f64, f64)> { /* ... */ }
  ```
  The rates at `thermal_power_mwth`, scaled by the `S/V` factor of

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `LwrContainedRelease`

An LWR arm released through the intact, leaking containment: the no-removal
result and, when the rates are given, the natural-deposition result
\[Bq per nuclide, to the environment\]. Used by both LWR LOCA arms,
[`nuscale_mha_loca`] (design basis) and [`nuscale_severe_loca`] (beyond
design basis, core melt).

```rust
pub struct LwrContainedRelease {
    pub no_removal: Releases,
    pub natural_deposition: Option<Releases>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `no_removal` | `Releases` | No removal credit. |
| `natural_deposition` | `Option<Releases>` | Natural deposition only; `None` while pending literature. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Type Alias `LwrDba`

The design-basis arm's name for [`LwrContainedRelease`] (kept for callers).

```rust
pub type LwrDba = LwrContainedRelease;
```

#### Enum `Group`

The comparison's reporting groups.

```rust
pub enum Group {
    NobleGases,
    Halogens,
    AlkaliMetals,
    Tellurium,
    BariumStrontium,
    Silver,
    Other,
}
```

##### Variants

###### `NobleGases`

Xe, Kr.

###### `Halogens`

I, Br.

###### `AlkaliMetals`

Cs, Rb.

###### `Tellurium`

Te, Sb, Se.

###### `BariumStrontium`

Ba, Sr.

###### `Silver`

Ag, Pd (HTR-10's silver; not a NUREG-1465 group).

###### `Other`

Everything else (noble metals, lanthanides, cerium, Mo).

##### Implementations

###### Methods

- ```rust
  pub fn of(nuclide: &str) -> Group { /* ... */ }
  ```
  The group of a nuclide label.

- ```rust
  pub fn label(self: Self) -> &'static str { /* ... */ }
  ```
  A short label.

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `DoseAssumptions`

The site and dose assumptions every arm shares -- those of
`examples/htr10_air_ingress_kora_bound.rs`.

```rust
pub struct DoseAssumptions {
    pub release_height_m: f64,
    pub measurement_height_m: f64,
    pub breathing_m3_per_s: f64,
    pub exposure_s: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `release_height_m` | `f64` | Release height \[m\]: **0, ground level**, for every arm (the HTR-10<br>example's; an LWR containment leak is also conventionally a ground-level<br>release). Kept identical so the comparison is like-for-like. |
| `measurement_height_m` | `f64` | Wind measurement height \[m\]. |
| `breathing_m3_per_s` | `f64` | Adult breathing rate \[m^3/s\] (FGR-11 convention, 20 L/min). |
| `exposure_s` | `f64` | Groundshine exposure period \[s\]. |

##### Implementations

###### Methods

- ```rust
  pub fn bounding_example() -> Self { /* ... */ }
  ```
  The HTR-10 bounding example's: ground release, 10 m wind, 20 L/min,

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Dose`

A maximum dose at one distance.

```rust
pub struct Dose {
    pub class: buangkok::pydoseia::dispersion::StabilityClass,
    pub chi_over_q: f64,
    pub total_sv: f64,
    pub by_group: Vec<(Group, f64)>,
    pub missing: Vec<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `class` | `buangkok::pydoseia::dispersion::StabilityClass` | Worst stability class at 1 m/s (largest chi/Q). |
| `chi_over_q` | `f64` | chi/Q used \[s/m^3\]. |
| `total_sv` | `f64` | Total over nuclides with coefficients \[Sv\]. |
| `by_group` | `Vec<(Group, f64)>` | By group \[Sv\]. |
| `missing` | `Vec<String>` | Nuclides lacking a coefficient on some pathway. ~~(NOT counted as<br>zero)~~ **CORRECTED 2026-09-30:** a missing pathway contributes<br>nothing to `total_sv` or `by_group`, so those are LOWER BOUNDS; this<br>list says which nuclides make them so (#456). |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `Table9Check`

One distance of the Table 9 cross-check of [`htr10_dba_release`] through
the shared dose chain.

```rust
pub struct Table9Check {
    pub distance_m: f64,
    pub ours_msv: f64,
    pub published_whole_body_msv: f64,
    pub ratio: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `distance_m` | `f64` | Receptor distance \[m\]. |
| `ours_msv` | `f64` | Our maximum dose \[mSv\]: [`max_dose`] with<br>[`DoseAssumptions::bounding_example`] (worst class, 1 m/s, ground<br>release, 96 h, submersion + groundshine + inhalation). |
| `published_whole_body_msv` | `f64` | Liu & Cao Table 9 "whole-body" \[mSv\] (their 40 m stack and their<br>unpublished weather; STOERNEU). |
| `ratio` | `f64` | `ours / published`. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Enum `Tier`

The comparison's tiers (maintainer decision, 2026-09-30, #450, #464):
**paired by initiating event and severity**, DLOFC (HTR) against LOCA
(LWR).

```rust
pub enum Tier {
    DesignBasis,
    BeyondDesignBasis,
    Context,
}
```

##### Variants

###### `DesignBasis`

HTR-10 DLOFC (Liu & Cao Table 8) vs LWR design-basis LOCA (RG 1.183).

###### `BeyondDesignBasis`

HTR-10 DLOFC + air ingress (KORA bound) vs LWR LOCA with ECCS failure
and core melt, in an intact containment.

###### `Context`

WASH-1400 PWR 8: context only.

##### Implementations

###### Methods

- ```rust
  pub fn label(self: Self) -> &'static str { /* ... */ }
  ```
  The tier's heading, used by the #452 example, the map table and the

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `ArmColumn`

One dose column of the comparison: its tier, a heading and a CSV key.

```rust
pub struct ArmColumn {
    pub tier: Tier,
    pub heading: &'static str,
    pub csv_key: &'static str,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `tier` | `Tier` | Which tier the column belongs to. |
| `heading` | `&'static str` | Short heading (the unit, mSv, is the caller's to add). |
| `csv_key` | `&'static str` | Column name for CSV output, in mSv. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `ComparisonRow`

One distance of the HTR-10 / LWR comparison: the maximum 96 h dose of
each arm \[Sv\], same site, weather, height and receptor (#452, #453).

~~**Framing (maintainer decision, 2026-09-30, #450): design basis against
design basis is the PRIMARY comparison** ... The KORA bound against
WASH-1400 PWR 8 is the SECONDARY, beyond-design-basis bounding
comparison.~~ **CHANGED 2026-09-30 (maintainer decision, #464): paired by
initiating event and severity**, DLOFC against LOCA, in two tiers
([`Tier`]): design basis (HTR-10 DLOFC vs RG 1.183 LOCA) and beyond design
basis (HTR-10 DLOFC + air-ingress bound vs LWR LOCA + core melt).
WASH-1400 PWR 8 is context only: no melt, uncontained, paired by
containment state rather than initiator. Fields are in tier order.

```rust
pub struct ComparisonRow {
    pub distance_m: f64,
    pub class: buangkok::pydoseia::dispersion::StabilityClass,
    pub htr10_dba_depressurisation_sv: f64,
    pub lwr_dba_no_removal_sv: f64,
    pub lwr_dba_natural_deposition_sv: Option<f64>,
    pub htr10_bound_sv: f64,
    pub lwr_severe_loca_no_removal_sv: f64,
    pub lwr_severe_loca_natural_deposition_sv: Option<f64>,
    pub wash1400_pwr8_sv: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `distance_m` | `f64` | Receptor distance \[m\]. |
| `class` | `buangkok::pydoseia::dispersion::StabilityClass` | Worst stability class at 1 m/s. It depends on distance only, so it is<br>the same for every arm. |
| `htr10_dba_depressurisation_sv` | `f64` | Design basis: HTR-10 DLOFC (depressurisation), Liu & Cao Table 8<br>([`htr10_dba_release`]). |
| `lwr_dba_no_removal_sv` | `f64` | Design basis: LWR LOCA, RG 1.183 MHA ([`nuscale_mha_loca`]), `L_a`<br>0.20 %/day, **no removal credit**. |
| `lwr_dba_natural_deposition_sv` | `Option<f64>` | Design basis: the same with **natural deposition only**; `None` while<br>pending literature ([`NATURAL_DEPOSITION_PENDING`]). |
| `htr10_bound_sv` | `f64` | Beyond design basis: HTR-10 DLOFC + air ingress, KORA bound<br>([`htr10_air_ingress_bound`]). |
| `lwr_severe_loca_no_removal_sv` | `f64` | Beyond design basis: LWR LOCA + core melt, NUREG-1465 Table 3.13 all<br>phases, intact containment at `L_a` ([`nuscale_severe_loca`]), **no<br>removal credit**. |
| `lwr_severe_loca_natural_deposition_sv` | `Option<f64>` | Beyond design basis: the same with natural deposition only; `None`<br>while pending literature. |
| `wash1400_pwr8_sv` | `f64` | Context: WASH-1400 PWR 8, to the atmosphere. |

##### Implementations

###### Methods

- ```rust
  pub fn arm_doses_sv(self: &Self) -> [Option<f64>; 7] { /* ... */ }
  ```
  The seven doses \[Sv\] in the order of [`ARM_COLUMNS`]; `None` is a

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `IncompleteShares`

Share of each arm's released Bq whose nuclide lacks an FGR coefficient on
some pathway. Those pathways count zero, so a non-zero share marks a
LOWER-BOUND dose (#456).

```rust
pub struct IncompleteShares {
    pub htr10_dba: f64,
    pub lwr_dba: f64,
    pub htr10_bound: f64,
    pub lwr_severe_loca: f64,
    pub wash1400: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `htr10_dba` | `f64` | HTR-10 depressurisation DBA (H-3, C-14). |
| `lwr_dba` | `f64` | LWR MHA LOCA, no removal. |
| `htr10_bound` | `f64` | HTR-10 KORA bound. |
| `lwr_severe_loca` | `f64` | LWR LOCA + core melt, no removal. |
| `wash1400` | `f64` | WASH-1400 PWR 8. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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
#### Struct `BoundingComparison`

The comparison over `distances_m` (#452, and `htgr_sim_v1`'s map, #453),
from one call so that the two cannot drift apart.

```rust
pub struct BoundingComparison {
    pub rows: Vec<ComparisonRow>,
    pub incomplete: IncompleteShares,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `rows` | `Vec<ComparisonRow>` | One row per distance, in the order given. |
| `incomplete` | `IncompleteShares` | Coverage of the FGR tables per arm. |

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

#### Function `htr10_air_ingress_bound`

The HTR-10 bounding air-ingress release over `window` \[Bq per nuclide\]:
boon-lay fuel failure at 1400 °C/140 h plus KORA f_ox as the accident
increment, TRISO-ATOPS release (real normal-operation pools, #448) of the
Liu & Cao Table 1 inventory under a flat 1400 °C hold over the window,
vented by [`AIR_INGRESS_VENTING`] (full flow-through, stated since #469),
**plus** Liu & Cao Table 3's circulating activity released at 100 %
(conservative; it double-counts the model's own circuit term, as the
example states). The chain of `examples/htr10_air_ingress_kora_bound.rs`.

`geometry` is HTR-10's (the caller reads it from `tampines::pebble_bed`).

# Errors
If the release chain rejects its inputs.

```rust
pub fn htr10_air_ingress_bound(geometry: crate::htr10::Htr10Geometry, window: uom::si::f64::Time) -> Result<Releases, crate::Error> { /* ... */ }
```

#### Function `pwr_inventory_scaled`

NuScale Table B-5 (one module) scaled to `thermal_power_mwth` \[Bq\].

```rust
pub fn pwr_inventory_scaled(thermal_power_mwth: f64) -> Releases { /* ... */ }
```

#### Function `nureg1465_pwr_into_containment`

NUREG-1465 Table 3.13 PWR release **into containment** \[Bq per nuclide\].
Nuclides whose element has no Table 3.8 group are omitted.

```rust
pub fn nureg1465_pwr_into_containment(inventory: &Releases, phases: N1465Phases) -> Releases { /* ... */ }
```

#### Function `rg1183_pwr_into_containment`

RG 1.183 Rev. 1 Table 2 PWR release **into containment** (gap + early
in-vessel) \[Bq per nuclide\], no decay.

```rust
pub fn rg1183_pwr_into_containment(inventory: &Releases) -> Releases { /* ... */ }
```

#### Function `rg1183_containment_leakage`

**RG 1.183 Rev. 1 containment leakage to the environment** over `window`.

- Source into the well-mixed containment: Table 2 fractions, released
  **linearly** over each Table 5 phase (gap 0.5 min -> 0.23 h, early
  in-vessel 0.23 -> 4.5 h; the RG's stated default), release terminating
  at the end of early in-vessel (Appendix A-2.1).
- Leakage (Appendix A-2.7): `L_a` for the first 24 h, `L_a / 2` after (PWR).
- Removal: **none credited** (module doc). Radioactive decay applied.
- `leak_rate_percent_per_day`: the plant's TS `L_a`. **Plant-specific, not
  in RG 1.183; never defaulted here.** `None` -> only the per-unit result.

Integrated by explicit exponential steps of 60 s (`dA/dt = S - (lambda +
L) A`, released `= integral L A dt`).

```rust
pub fn rg1183_containment_leakage(inventory: &Releases, leak_rate_percent_per_day: Option<f64>, window: uom::si::f64::Time) -> Leakage { /* ... */ }
```

#### Function `nureg1465_pwr_phases`

**NUREG-1465 Table 3.13 PWR, all four phases**, timed by Table 3.6
([`n1465_timing`]): gap (30 s -> 0.508 h), early in-vessel (-> vessel
breach at 1.808 h), ex-vessel (1.808 -> 3.808 h) and late in-vessel
(1.808 -> 11.808 h, concurrent with ex-vessel). Release is **linear
within each phase**, an assumption: NUREG-1465 gives durations only, and
linear is RG 1.183 Rev. 1's stated default for its own phases. `None` if
the element has no Table 3.8 group (Table 3.8 grouping, as
[`nureg1465_pwr_into_containment`]).

```rust
pub fn nureg1465_pwr_phases(nuclide: &str) -> Option<[ReleasePhase; 4]> { /* ... */ }
```

#### Function `rg1183_leak`

RG 1.183 containment -> environment at leak rate `l_percent` \[%/day\],
with an optional first-order removal `(aerosol, elemental iodine)`
\[1/s\] inside the containment (natural deposition). `None` is no
removal credit. Source: Table 2 fractions, linear over the Table 5
phases, terminating at the end of early in-vessel (App. A-2.1); leak
`L_a` for 24 h, then `L_a/2` (PWR, App. A-2.7); decay applied.

The integrator is [`containment_leak`] (shared with the severe-LOCA arm,
[`nuscale_severe_loca`]).

```rust
pub fn rg1183_leak(inventory: &Releases, l_percent: f64, window: uom::si::f64::Time, removal: Option<(f64, f64)>) -> Releases { /* ... */ }
```

#### Function `containment_leak`

**Intact containment -> environment**, for any phased source: `phases`
gives each nuclide's releases into a well-mixed containment ([`None`]
drops the nuclide). Leak rate `l_percent` \[%/day\] for the first 24 h,
then half (RG 1.183 Rev. 1 App. A-2.7, PWR); optional first-order removal
`(aerosol, elemental iodine)` \[1/s\], split by the RG 1.183 App. A-1.1
iodine species; radioactive decay applied. The containment is assumed to
stay **intact** for the whole window (no failure, no bypass): the only
path out is the leak.

Integrated by exact exponential steps of 60 s (`dA/dt = S - (lambda + L
+ lambda_removal) A`, released `= integral L A dt`). The source over a
step is the phase's rate times its overlap with the step, so every phase
delivers exactly its fraction. ~~Source evaluated at the step midpoint~~
**CHANGED 2026-09-30 (#464):** the midpoint rule gave RG 1.183's gap phase
(0.5 min -> 0.23 h = 798 s) 14 steps = 840 s, over-delivering it by 5.3 %
and early in-vessel short by 0.08 %; found while adding the severe-LOCA
arm; the DBA dose moved as recorded on #464.

```rust
pub fn containment_leak<P, V>(inventory: &Releases, phases: P, l_percent: f64, window: uom::si::f64::Time, removal: Option<(f64, f64)>) -> Releases
where
    P: Fn(&str) -> Option<V>,
    V: AsRef<[ReleasePhase]> { /* ... */ }
```

#### Function `scaled_containment`

**Containment scaled DOWN with power** (maintainer decision, 2026-09-30),
an ASSUMPTION stated as such: geometric similarity, free volume `V ∝ P`,
surface `S ∝ V^(2/3)`, so `S/V` grows by `(P_ref/P)^(1/3)`. Returns `(V
[m^3], S/V factor relative to the NuScale module)`. At 10 MWth: 375 ft^3
= 10.62 m^3 and `(160/10)^(1/3) = 2.520`. `L_a` (a fraction per day) is
unchanged; natural deposition (`∝ S/V`) is multiplied by the factor.

```rust
pub fn scaled_containment(thermal_power_mwth: f64) -> (f64, f64) { /* ... */ }
```

#### Function `nuscale_mha_loca`

The **LWR design-basis arm** (maintainer decision, 2026-09-30, #450):
RG 1.183 Rev. 1 MHA LOCA with the NuScale inventory scaled to
`thermal_power_mwth`, leaking at NuScale's `L_a` = 0.20 %/day (24 h, then
half), released to the environment over `window`.

```rust
pub fn nuscale_mha_loca(thermal_power_mwth: f64, window: uom::si::f64::Time, deposition: &NaturalDeposition) -> LwrContainedRelease { /* ... */ }
```

#### Function `nuscale_severe_loca`

The **LWR beyond-design-basis arm: LOCA with ECCS failure -> core melt**
(maintainer decision, 2026-09-30, #464). The NuScale inventory scaled to
`thermal_power_mwth` (as [`nuscale_mha_loca`]) is released into the
containment by **NUREG-1465 Table 3.13 PWR, all four phases**
([`nureg1465_pwr_phases`]: gap, early in-vessel, ex-vessel, late
in-vessel, Table 3.6 timing). The containment is **intact** and leaks at
NuScale's `L_a` = 0.20 %/day for 24 h, then half, with the RG 1.183 App.
A-1.1 iodine species (95 % CsI, 4.85 % elemental, 0.15 % organic) for the
removal arm; decay applied; to the environment over `window`. The
integrator is the design-basis arm's ([`containment_leak`]).

**Assumption, stated** ([`CONTAINED_CORE_MELT_ASSUMPTION`]): this is a
*contained* core melt. No early containment failure, no bypass, no
basemat melt-through inside the window. NUREG-1465's ex-vessel phase
(core-concrete interaction) is taken as releasing into the same intact
atmosphere. WASH-1400 PWR 1-3 are the containment-failure categories.

```rust
pub fn nuscale_severe_loca(thermal_power_mwth: f64, window: uom::si::f64::Time, deposition: &NaturalDeposition) -> LwrContainedRelease { /* ... */ }
```

#### Function `wash1400_pwr8_to_atmosphere`

WASH-1400 Table 5-1 PWR 8 release **to the atmosphere** \[Bq per nuclide\].
Iodine = the I column + the organic-I column.

```rust
pub fn wash1400_pwr8_to_atmosphere(inventory: &Releases) -> Releases { /* ... */ }
```

#### Function `max_dose`

Maximum dose at `x_m` from `releases`, the whole release passing the
receptor at the worst class, 1 m/s: submersion (FGR-15) + inhalation
(FGR-11, max over classes) + groundshine (FGR-15, deposited at buangkok's
velocity, decaying over the exposure period). The arithmetic of
`examples/htr10_air_ingress_kora_bound.rs` section 5.

```rust
pub fn max_dose(releases: &Releases, x_m: f64, a: DoseAssumptions) -> Dose { /* ... */ }
```

#### Function `htr10_dba_release`

**HTR-10 design-basis releases** to the environment \[Bq per nuclide\]:
Liu & Cao (2002, NED 218:81-90) **Table 8**, either the depressurisation
(DN65 charging-tube rupture, s.4.1.1) or the water ingress (two SG tubes,
relief failed, s.4.1.2), as `changi::activity::accident_airborne_release`
holds it. Published, not computed here. H-3 and C-14 are included; their
missing FGR coefficients are reported by [`max_dose`], not zeroed silently
(#452, 2026-09-30).

```rust
pub fn htr10_dba_release(case: AccidentCase) -> Releases { /* ... */ }
```

#### Function `htr10_dba_vs_table9`

Cross-check [`htr10_dba_release`] through [`max_dose`] against Liu & Cao
Table 9 at its own distances. **Different conditions, stated:** the
shared chain is a ground-level release at the worst class and 1 m/s (the
#452 conditions), while Table 9 comes from a 40 m stack under weather the
paper does not give. The ratio is a finding, not a gate, and nothing is
tuned to it. Liu & Cao's own conditions are swept (external pathways only)
in `buangkok/tests/liu_cao_external_dose_cross_check.rs` (#379).

Measured 2026-09-30: ours/Table 9 = 0.12 (depressurisation) and 0.08
(water ingress) at 250 m, 0.01-0.04 from 0.75 to 15 km, and 1.46 / 1.13
at 75 km. That is the gap #379 found (Table 9's unstated integration
period and weather), made larger here by the 96 h groundshine window.

```rust
pub fn htr10_dba_vs_table9(case: AccidentCase) -> Vec<Table9Check> { /* ... */ }
```

#### Function `bounding_comparison`

Build the [`BoundingComparison`]: the design-basis tier (HTR-10 DLOFC,
[`htr10_dba_release`]; LWR LOCA, [`nuscale_mha_loca`] with `deposition`),
the beyond-design-basis tier (HTR-10 DLOFC + air-ingress bound,
[`htr10_air_ingress_bound`]; LWR LOCA + core melt, [`nuscale_severe_loca`]
with `deposition`) and the WASH-1400 PWR 8 context arm, with the LWR
inventory and containment scaled to `mwth`, through [`max_dose`] with
[`DoseAssumptions::bounding_example`].

# Errors
If the HTR-10 release chain rejects its inputs.

```rust
pub fn bounding_comparison(geometry: crate::htr10::Htr10Geometry, window: uom::si::f64::Time, mwth: f64, distances_m: &[f64], deposition: &NaturalDeposition) -> Result<BoundingComparison, crate::Error> { /* ... */ }
```

#### Function `incomplete_share`

Share of `rel`'s Bq whose nuclide lacks an FGR coefficient on some pathway
(see [`Dose::missing`]).

```rust
pub fn incomplete_share(rel: &Releases) -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `AIR_INGRESS_VENTING`

How the bounding air ingress carries the release out of the core:
**[`Venting::FullFlowThrough`]**, `frac = 1` at every sample — the
conservative limit of [`Venting::Prescribed`], and the assumption of the
#435 bound ("every particle exposed", no primary-circuit retention).

**Why not [`Venting::Upstream`] (#469 item 4).** Upstream TRISO-ATOPS is a
depressurisation model: its only transport is gas expansion while the core
heats. It gives `frac = 1` on an *exactly* uniform, constant hold (#446),
but a hold drifting by 0.1 K takes its `coolant_release` branch and vents
`≈ 1 − T0/T`, so the bound fell from 1.69e13 Bq to 4.94e10 Bq
(`tests::a_near_isothermal_air_ingress_still_vents`, measured 2026-09-30).
Air ingress convects the release out whatever the temperature does, so the
transport is stated explicitly. On the exact hold the two agree, so the
recorded bounding numbers do not move.

```rust
pub const AIR_INGRESS_VENTING: crate::accident::release::Venting = Venting::FullFlowThrough;
```

#### Constant `NUSCALE_MODULE_MWTH`

NuScale module thermal power the Table B-5 inventory is attributed to
\[MWth\] -- **the maintainer's attribution**, not stated in the document
(see the CSV header).

```rust
pub const NUSCALE_MODULE_MWTH: f64 = 160.0;
```

#### Constant `NUSCALE_LA_PERCENT_PER_DAY`

NuScale's maximum allowable containment (CNV) leak rate `L_a` \[%/day of
the containment air mass\]: **0.20 wt%/day at P_a**. NRC, *Phase 4 SER,
Chapter 6* (NuScale DCA), PDF p. 91: "The NuScale maximum allowable CNV
leak rate, La, is 0.20 wt% of the containment air mass per day at the
calculated Pa" (proprietary-filed here; cited, not redistributed). As a
fraction per day it is **unchanged by the power scaling** (maintainer
decision, 2026-09-30).

```rust
pub const NUSCALE_LA_PERCENT_PER_DAY: f64 = 0.20;
```

#### Constant `NUSCALE_CNV_FREE_VOLUME_FT3`

NuScale's minimum containment free volume \[ft^3\]: **6,000 ft^3**. Same
SER, PDF pp. 19-20 ("the minimum containment free volume is 6,000 ft3",
ADAMS ML18304A128).

```rust
pub const NUSCALE_CNV_FREE_VOLUME_FT3: f64 = 6_000.0;
```

#### Constant `NATURAL_DEPOSITION_PENDING`

What the natural-deposition arm still needs, printed wherever it would be
(RG 1.183 Rev. 1 App. A-2.2: SRP 6.5.2 is the acceptable model;
NUREG/CR-6189 only case by case, adjusted to the Rev. 1 source term, at
10th-percentile values).

```rust
pub const NATURAL_DEPOSITION_PENDING: &str = "pending literature: RG 1.183 Rev. 1 App. A-2.2 \
     accepts the natural-deposition model of NUREG-0800 (SRP) Section 6.5.2 (or NUREG/CR-6189, \
     ML100130305, case by case, adjusted, 10th percentile); neither is held, nor is NuScale's CNV \
     internal surface area (DCA Part 2 Tier 2 Ch. 6/15, or SER Section 15.0.3)";
```

#### Constant `CONTAINED_CORE_MELT_ASSUMPTION`

What the severe-LOCA arm assumes about the containment, printed wherever
the arm is shown (maintainer decision, 2026-09-30, #464).

```rust
pub const CONTAINED_CORE_MELT_ASSUMPTION: &str = "contained core melt: the containment is \
     ASSUMED to stay intact (no early failure, no bypass) and to leak only at L_a; WASH-1400 \
     PWR 1-3 are the containment-failure categories, for context";
```

#### Constant `REACTOR_LEVEL_BASIS`

The comparison's crediting basis (maintainer decision, 2026-09-30,
#450/#464), printed wherever the comparison is shown: "The comparison
neglects the pools because we are comparing technology at the reactor
level, not what is surrounding the reactor. If NuScale gives pool credit,
then HTGR can be submerged in a pool as well." **Each side is credited only
with its reactor-level inherent barrier**: the containment vessel leaking
at `L_a` (LWR), the TRISO particles (HTR). Nothing surrounding the reactor
is credited on either side. Pool scrubbing is excluded **by design**, not
pending literature; in-containment natural deposition is part of the
containment barrier and stays (pending literature).

```rust
pub const REACTOR_LEVEL_BASIS: &str = "reactor-level comparison (maintainer decision, \
     2026-09-30): each side is credited only with its reactor-level inherent barrier -- the \
     containment vessel leaking at L_a (LWR), the TRISO particles (HTR). Nothing surrounding the \
     reactor is credited: not NuScale's reactor pool, reactor building, sprays or filters; not \
     the HTR confinement/building (#409) or a hypothetical pool. Pool scrubbing is excluded by \
     design, not pending literature. \"If NuScale gives pool credit, then HTGR can be submerged \
     in a pool as well.\"";
```

#### Constant `ARM_COLUMNS`

The seven dose columns, **ordered by tier**, in the order of
[`ComparisonRow::arm_doses_sv`]. The map, the headless CSV and the #452
example all read this, so they cannot disagree on order or labels.

```rust
pub const ARM_COLUMNS: [ArmColumn; 7] = _;
```

### Re-exports

#### Re-export `AccidentCase`

```rust
pub use changi::activity::accident_airborne_release::AccidentCase;
```

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
    fn clone(self: &Self) -> Self { /* ... */ }
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
    fn eq(self: &Self, other: &Self) -> bool { /* ... */ }
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
    fn try_from(value: U) -> Result<T, never> { /* ... */ }
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

