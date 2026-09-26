# Crate Documentation

**Version:** 0.1.3

**Format Version:** 61

# Module `boon_lay`

# BOON LAY

**BO**mbardment of neutrons **O**n **N**uclides with **L**agrangian
transport **a**nd transmutation **Y**ields.

TRISO-particle and Lagrangian decay / transmutation simulator, and host of
the TRISO-ATOPS fork (`triso_atops_fork`), which supplies the fission-product
release physics that `sembawang` orchestrates.

## Modules

## Module `prelude`

prelude is here for easy imports

```rust
pub mod prelude { /* ... */ }
```

### Re-exports

#### Re-export `decay_xml_info_serde`

```rust
pub use crate::decay_xml_info_serde;
```

#### Re-export `ComputeType`

```rust
pub use crate::compute::ComputeType;
```

#### Re-export `ThreadCount`

```rust
pub use crate::compute::ThreadCount;
```

#### Re-export `stochastic_decay_chain`

```rust
pub use crate::lagrangian_decay_simulator::stochastic_decay_chain;
```

#### Re-export `decay_library`

```rust
pub use crate::nuclide_reaction_and_decay_data::decay_library;
```

#### Re-export `DecayType`

```rust
pub use crate::nuclide_reaction_and_decay_data::DecayType;
```

#### Re-export `HalfLifeAndDecayEnergyInfo`

```rust
pub use crate::nuclide_reaction_and_decay_data::HalfLifeAndDecayEnergyInfo;
```

#### Re-export `NuclideReactionAndDecayData`

```rust
pub use crate::nuclide_reaction_and_decay_data::NuclideReactionAndDecayData;
```

#### Re-export `SingleNuclideSimulatorMC`

```rust
pub use crate::lagrangian_decay_simulator::monte_carlo_single_radionuclide_decay_simulator::SingleNuclideSimulatorMC;
```

#### Re-export `activity_from_atom_count`

```rust
pub use crate::triso_atops_fork::activities::activity_from_atom_count;
```

#### Re-export `atom_count_from_activity`

```rust
pub use crate::triso_atops_fork::activities::atom_count_from_activity;
```

#### Re-export `base_activities`

```rust
pub use crate::triso_atops_fork::activities::base_activities;
```

#### Re-export `becquerels_from_curies`

```rust
pub use crate::triso_atops_fork::activities::becquerels_from_curies;
```

#### Re-export `circulating`

```rust
pub use crate::triso_atops_fork::activities::circulating;
```

#### Re-export `circulating_steadystate`

```rust
pub use crate::triso_atops_fork::activities::circulating_steadystate;
```

#### Re-export `clean_up`

```rust
pub use crate::triso_atops_fork::activities::clean_up;
```

#### Re-export `clean_up_steadystate`

```rust
pub use crate::triso_atops_fork::activities::clean_up_steadystate;
```

#### Re-export `curies_from_becquerels`

```rust
pub use crate::triso_atops_fork::activities::curies_from_becquerels;
```

#### Re-export `plate_out`

```rust
pub use crate::triso_atops_fork::activities::plate_out;
```

#### Re-export `plate_out_steadystate`

```rust
pub use crate::triso_atops_fork::activities::plate_out_steadystate;
```

#### Re-export `release_rate`

```rust
pub use crate::triso_atops_fork::activities::release_rate;
```

#### Re-export `FailureFractions`

```rust
pub use crate::triso_atops_fork::activities::FailureFractions;
```

#### Re-export `SourceAndGraphite`

```rust
pub use crate::triso_atops_fork::activities::SourceAndGraphite;
```

#### Re-export `BQ_PER_CI`

```rust
pub use crate::triso_atops_fork::activities::BQ_PER_CI;
```

#### Re-export `diffusion_coefficient`

```rust
pub use crate::triso_atops_fork::diffusion::diffusion_coefficient;
```

#### Re-export `diffusion_coefficient_sic_ag`

```rust
pub use crate::triso_atops_fork::diffusion::diffusion_coefficient_sic_ag;
```

#### Re-export `integrate_diffusion_over_time`

```rust
pub use crate::triso_atops_fork::diffusion::integrate_diffusion_over_time;
```

#### Re-export `DiffusionMaterial`

```rust
pub use crate::triso_atops_fork::diffusion::DiffusionMaterial;
```

#### Re-export `KernelGraphiteDiffusion`

```rust
pub use crate::triso_atops_fork::diffusion::KernelGraphiteDiffusion;
```

#### Re-export `normal_operation_node`

```rust
pub use crate::triso_atops_fork::normal_operation::normal_operation_node;
```

#### Re-export `NodalActivities`

```rust
pub use crate::triso_atops_fork::normal_operation::NodalActivities;
```

#### Re-export `NodalActivitiesCurie`

```rust
pub use crate::triso_atops_fork::normal_operation::NodalActivitiesCurie;
```

#### Re-export `NodeState`

```rust
pub use crate::triso_atops_fork::normal_operation::NodeState;
```

#### Re-export `ParentPools`

```rust
pub use crate::triso_atops_fork::normal_operation::ParentPools;
```

#### Re-export `PlantConstants`

```rust
pub use crate::triso_atops_fork::normal_operation::PlantConstants;
```

#### Re-export `find_nuclide`

```rust
pub use crate::triso_atops_fork::nuclide_model::nuclide_database::find_nuclide;
```

#### Re-export `supported_nuclides`

```rust
pub use crate::triso_atops_fork::nuclide_model::supported_nuclides;
```

#### Re-export `ElementGroup`

```rust
pub use crate::triso_atops_fork::nuclide_model::ElementGroup;
```

#### Re-export `TrisoAtopsNuclide`

```rust
pub use crate::triso_atops_fork::nuclide_model::TrisoAtopsNuclide;
```

#### Re-export `TRISO_ATOPS_NUCLIDE_COUNT`

```rust
pub use crate::triso_atops_fork::nuclide_model::TRISO_ATOPS_NUCLIDE_COUNT;
```

#### Re-export `rb_fail`

```rust
pub use crate::triso_atops_fork::release_models::rb_fail;
```

#### Re-export `release_fraction_transient`

```rust
pub use crate::triso_atops_fork::release_models::release_fraction_transient;
```

#### Re-export `ReleaseMaterial`

```rust
pub use crate::triso_atops_fork::release_models::ReleaseMaterial;
```

#### Re-export `Activity`

```rust
pub use crate::triso_atops_fork::Activity;
```

#### Re-export `DecayConstant`

```rust
pub use crate::triso_atops_fork::DecayConstant;
```

#### Re-export `ReleaseFraction`

```rust
pub use crate::triso_atops_fork::ReleaseFraction;
```

#### Re-export `crate::decay_xml_info_serde::*`

```rust
pub use crate::decay_xml_info_serde::*;
```

## Module `compute`

Compute-backend selector (`ComputeType` / `ThreadCount`) — the runtime CPU vs
wgpu resource switcher for the Walk-on-Spheres ensembles, mirroring the
`outram-mc-libs` `ComputeType`. Compiles on all targets (the GPU *body* is
gated, not this enum), so `Gpu` is always selectable and falls back to CPU.
Compute-backend selector for the Walk-on-Spheres diffusion ensembles.

A single [`ComputeType`] value chooses *how* a Lagrangian ensemble advances
its independent atom histories — on one CPU thread, across all CPU cores with
[`rayon`], or with a `wgpu` GPU compute kernel. The **physics is identical**
across backends; only the execution strategy (and, for the GPU path, the
floating-point precision) differs. Enum dispatch is used deliberately — no
trait objects — so every `match self { … }` site is exhaustively checked at
compile time (see the workspace `CLAUDE.md` "No trait objects" rule).

This mirrors the `outram-mc-libs` `ComputeType` switcher
(`crates/outram-mc-libs/src/physics/compute.rs`, bead op-fla) so the two
crates present the same knob to a user. The driver that honours this selector
is [`LiveEnsemble::advance_frame`] in
[`crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::live`].

[`LiveEnsemble::advance_frame`]:
crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::live::LiveEnsemble::advance_frame

```rust
pub mod compute { /* ... */ }
```

### Types

#### Enum `ComputeType`

Which compute backend advances a Walk-on-Spheres ensemble.

The variants map onto the compute modes named `CPUSingleThread` /
`CPUMultiThread` / `GPU`; this enum uses idiomatic Rust casing (`Cpu` / `Gpu`)
so the crate builds clean under clippy's `upper_case_acronyms` lint.

| This enum | Meaning |
|---|---|
| [`CpuSingleThread`](Self::CpuSingleThread) | scalar, single-thread `f64` |
| [`CpuMultiThread`](Self::CpuMultiThread)   | rayon-parallel over histories |
| [`Gpu`](Self::Gpu)                         | `wgpu` kernel, CPU fallback |

# Trust model

[`CpuSingleThread`](Self::CpuSingleThread) is the **trusted, deterministic
reference**: raw `f64`, per-history RNG streams derived from a base seed, so a
fixed seed gives reproducible output. The other two backends are
**acceleration only** and are validated *against* this reference within
statistical uncertainty — never trusted above it (the GPU kernel runs in
`f32`; see the crate `gpu` module docs).

# Portability

The enum and every driver that dispatches on it compile on **all** targets,
including Android (`target_os = "android"`), where the GPU module is
target-gated out. On Android, [`Gpu`](Self::Gpu) transparently runs the CPU
path (there is no adapter to probe), so selecting it is always safe.
[`CpuMultiThread`](Self::CpuMultiThread) is a valid Android backend too —
`rayon` and [`std::thread::available_parallelism`] both work there; a phone
simply resolves to fewer cores.

`Eq` is deliberately **not** derived: [`ThreadCount::Fraction`] carries an
`f64`, which is only `PartialEq`.

```rust
pub enum ComputeType {
    CpuSingleThread,
    CpuMultiThread(ThreadCount),
    Gpu,
}
```

##### Variants

###### `CpuSingleThread`

Scalar, single-thread advance — the **deterministic trusted reference**.

One `f64` history at a time; a fixed base seed yields reproducible output
independent of machine. This is the default.

###### `CpuMultiThread`

Rayon-parallel advance over the atom histories, sized by [`ThreadCount`].

Histories are embarrassingly parallel, so they run across CPU cores with
[`rayon`] in a **dedicated pool** sized to [`ThreadCount`]
([`ThreadCount::Auto`] scales with the machine's logical-core count).
Each history's RNG stream is derived deterministically from `(base_seed,
index)`, so the result is reproducible independent of thread count and
does not race. It agrees with [`CpuSingleThread`](Self::CpuSingleThread)
within statistical uncertainty.

Construct the default form with `CpuMultiThread(ThreadCount::Auto)`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `ThreadCount` |  |

###### `Gpu`

GPU-accelerated advance via `wgpu`, with graceful CPU fallback.

Runs the Walk-on-Spheres kernel in `f32` on the GPU. If no GPU adapter is
available — a headless server, CI with no Vulkan/Metal loader, or Android
where the GPU module is compiled out — the driver falls back transparently
to the CPU path. It **never errors on a missing GPU.** GPU `f32` results
are acceleration only and are held to a tolerance against the CPU
reference.

##### Implementations

###### Methods

- ```rust
  pub fn label(self: Self) -> &'static str { /* ... */ }
  ```
  A short human-readable label for the selected backend, for UI display.

- ```rust
  pub fn next(self: Self) -> ComputeType { /* ... */ }
  ```
  The next backend in the cycle single → multi → GPU → single, for a

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> ComputeType { /* ... */ }
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
    fn default() -> ComputeType { /* ... */ }
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
    fn eq(self: &Self, other: &ComputeType) -> bool { /* ... */ }
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
#### Enum `ThreadCount`

How many worker threads the [`ComputeType::CpuMultiThread`] backend uses,
sized to the CPU's strength.

The driver resolves this to a concrete positive thread count with
[`ThreadCount::resolve`] and builds a dedicated [`rayon::ThreadPool`] of that
size. The default is [`Auto`](Self::Auto), which reads the machine's logical
core count via [`std::thread::available_parallelism`] — a desktop naturally
gets many threads, a phone gets few, with no special-casing. All variants
resolve to **at least 1** thread.

```rust
pub enum ThreadCount {
    Auto,
    Fixed(usize),
    Fraction(f64),
}
```

##### Variants

###### `Auto`

Use every logical core: [`std::thread::available_parallelism`]. Falls
back to 1 if the query fails. The default.

###### `Fixed`

An explicit worker-thread count. Clamped up to a minimum of 1.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

###### `Fraction`

A fraction of the available logical cores, e.g. `0.5` = half. The product
`fraction * cores` is rounded to the nearest integer and clamped to at
least 1.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn resolve(self: Self) -> usize { /* ... */ }
  ```
  Resolve to a concrete worker-thread count (always `>= 1`).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> ThreadCount { /* ... */ }
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
    fn default() -> ThreadCount { /* ... */ }
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
    fn eq(self: &Self, other: &ThreadCount) -> bool { /* ... */ }
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
## Module `decay_xml_info_serde`

this contains the raw information
based on pwr neutron spectrum

```rust
pub mod decay_xml_info_serde { /* ... */ }
```

### Re-exports

#### Re-export `SerdeNuclideData`

```rust
pub use openmc_endf_8_depletion_lib_b::SerdeNuclideData;
```

#### Re-export `SerdeNuclideVec`

```rust
pub use openmc_endf_8_depletion_lib_b::SerdeNuclideVec;
```

#### Re-export `openmc_endf_8_depletion_lib_b::prelude::*`

```rust
pub use openmc_endf_8_depletion_lib_b::prelude::*;
```

## Module `nuclide_reaction_and_decay_data`

this is the struct that converts the SerdeNuclideData to
NuclideReactionAndDecayData

```rust
pub mod nuclide_reaction_and_decay_data { /* ... */ }
```

### Modules

## Module `get_decay_info`

contains code to access decay information in an easier manner

```rust
pub mod get_decay_info { /* ... */ }
```

## Module `parse_nuclides_to_decay_data`

contains modules to parse nuclides and obtain their respective xml data

```rust
pub mod parse_nuclides_to_decay_data { /* ... */ }
```

## Module `decay_library`

contains a module for a full decay library, which is meant to make it
easy to obtain information based on the nuclide enum

```rust
pub mod decay_library { /* ... */ }
```

### Modules

## Module `indexing_using_nuclide`

this allows users to use nuclides to get appropriate decay data

```rust
pub mod indexing_using_nuclide { /* ... */ }
```

## Module `get_random_number`

this allows users to get a rng

```rust
pub mod get_random_number { /* ... */ }
```

### Types

#### Struct `DecayLibrary`

this is a full decay library constructed at start
incorporating all decays from all radionuclides

```rust
pub struct DecayLibrary {
    pub random_number_generator: outram_mc_libs::rng::lcg::Lcg64,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `random_number_generator` | `outram_mc_libs::rng::lcg::Lcg64` |  |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn try_match_nuclides_to_decay_data(self: &Self, nuclide: Nuclide) -> Option<NuclideReactionAndDecayData> { /* ... */ }
  ```

- ```rust
  pub fn get_random_number_and_rng(self: &mut Self) -> (f64, Rand64) { /* ... */ }
  ```
  allows user to obtain a random number and a clone of

- ```rust
  pub fn new() -> Self { /* ... */ }
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
    fn clone(self: &Self) -> DecayLibrary { /* ... */ }
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
    fn eq(self: &Self, other: &DecayLibrary) -> bool { /* ... */ }
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
### Types

#### Struct `NuclideReactionAndDecayData`

```rust
pub struct NuclideReactionAndDecayData {
    pub nuclide: fission_yields_data::prelude::Nuclide,
    pub half_life_information: HalfLifeAndDecayEnergyInfo,
    pub decay_information: Vec<DecayData>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nuclide` | `fission_yields_data::prelude::Nuclide` |  |
| `half_life_information` | `HalfLifeAndDecayEnergyInfo` |  |
| `decay_information` | `Vec<DecayData>` |  |

##### Implementations

###### Methods

- ```rust
  pub fn try_get_half_life(self: &Self) -> Option<Time> { /* ... */ }
  ```
  this obtains half life of the nuclide

- ```rust
  pub fn is_stable(self: &Self) -> bool { /* ... */ }
  ```
  checks whether the nuclide is stable

- ```rust
  pub fn is_unstable(self: &Self) -> bool { /* ... */ }
  ```
  checks whether nuclide is unstable (just for readability sake)

- ```rust
  pub fn get_decay_energy(self: &Self) -> Option<Energy> { /* ... */ }
  ```
  this obtains decay energy of the nuclide

- ```rust
  pub fn get_decay_branch_info(self: &Self) -> Vec<(Ratio, Nuclide, DecayType)> { /* ... */ }
  ```
  get decay branch, branching ratio, decay type and target

- ```rust
  pub fn get_next_target_nuclide_with_rng(self: &Self, rng: &mut Rand64) -> Option<(Nuclide, DecayType)> { /* ... */ }
  ```

- ```rust
  pub fn get_next_target_nuclide_with_float(self: &Self, random_num_between_0_and_1: f64) -> Option<(Nuclide, DecayType)> { /* ... */ }
  ```

- ```rust
  pub fn computationally_expensive_parse_nuclide_to_decay_data(nuclide: Nuclide) -> Option<NuclideReactionAndDecayData> { /* ... */ }
  ```
  this is a computationally expensive way to obtain decay data

- ```rust
  pub fn parse_nuclides_to_decay_data_vec_by_element(nuclide: &Nuclide) -> Vec<NuclideReactionAndDecayData> { /* ... */ }
  ```
  will parse nuclides to obtain decay information

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> NuclideReactionAndDecayData { /* ... */ }
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

  - ```rust
    fn from(raw_data_serde: SerdeNuclideData) -> Self { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &NuclideReactionAndDecayData) -> bool { /* ... */ }
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
#### Enum `HalfLifeAndDecayEnergyInfo`

```rust
pub enum HalfLifeAndDecayEnergyInfo {
    Stable,
    Unstable(Time, Energy),
}
```

##### Variants

###### `Stable`

###### `Unstable`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Time` |  |
| 1 | `Energy` |  |

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
    fn clone(self: &Self) -> HalfLifeAndDecayEnergyInfo { /* ... */ }
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
    fn eq(self: &Self, other: &HalfLifeAndDecayEnergyInfo) -> bool { /* ... */ }
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
#### Struct `DecayData`

```rust
pub struct DecayData {
    pub decay_type: DecayType,
    pub target: Option<fission_yields_data::prelude::Nuclide>,
    pub branching_ratio: Ratio,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `decay_type` | `DecayType` |  |
| `target` | `Option<fission_yields_data::prelude::Nuclide>` |  |
| `branching_ratio` | `Ratio` |  |

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
    fn clone(self: &Self) -> DecayData { /* ... */ }
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
    fn eq(self: &Self, other: &DecayData) -> bool { /* ... */ }
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
#### Enum `DecayType`

```rust
pub enum DecayType {
    Alpha,
    ElectronCaptureBetaPlus,
    ElectronCaptureBetaPlusAndAlpha,
    BetaMinus,
    BetaMinusAndNeutron,
    BetaMinusAndTwoNeutron,
    BetaMinusAndThreeNeutron,
    BetaMinusAndFourNeutron,
    BetaMinusAndAlpha,
    DoubleBetaMinus,
    IsomericTransition,
    Proton,
    DoubleProton,
    ElectronCaptureBetaPlusAndProton,
    ElectronCaptureBetaPlusDoubleProton,
    SpontaneousFission,
    ElectronCaptureBetaPlusAndSpontaneousFission,
    Neutron,
    DoubleNeutron,
}
```

##### Variants

###### `Alpha`

###### `ElectronCaptureBetaPlus`

###### `ElectronCaptureBetaPlusAndAlpha`

###### `BetaMinus`

###### `BetaMinusAndNeutron`

###### `BetaMinusAndTwoNeutron`

###### `BetaMinusAndThreeNeutron`

###### `BetaMinusAndFourNeutron`

###### `BetaMinusAndAlpha`

###### `DoubleBetaMinus`

###### `IsomericTransition`

###### `Proton`

###### `DoubleProton`

###### `ElectronCaptureBetaPlusAndProton`

###### `ElectronCaptureBetaPlusDoubleProton`

###### `SpontaneousFission`

###### `ElectronCaptureBetaPlusAndSpontaneousFission`

###### `Neutron`

###### `DoubleNeutron`

##### Implementations

###### Methods

- ```rust
  pub fn parse_from_string(string: &str) -> Self { /* ... */ }
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
    fn clone(self: &Self) -> DecayType { /* ... */ }
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
    fn eq(self: &Self, other: &DecayType) -> bool { /* ... */ }
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
## Module `lagrangian_decay_simulator`

this is the part that deals with decay simulation in lagrangian
or monte carlo bit
this part deals only with the terminal user interface

```rust
pub mod lagrangian_decay_simulator { /* ... */ }
```

### Modules

## Module `stochastic_decay_chain`

this code here is meant to simulate decay chains
Basically, it takes information from the nuclide, converts it into decay
data and then terminates it as it reaches stability

  

```rust
pub mod stochastic_decay_chain { /* ... */ }
```

### Modules

## Module `iterator_for_decay_chain`

implements iterator for decay chain

```rust
pub mod iterator_for_decay_chain { /* ... */ }
```

### Types

#### Struct `DecayChainIntoIter`

```rust
pub struct DecayChainIntoIter {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

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

- **ExactSizeIterator**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **FusedIterator**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **IntoIterator**
  - ```rust
    fn into_iter(self: Self) -> I { /* ... */ }
    ```

- **Iterator**
  - ```rust
    fn next(self: &mut Self) -> Option<<Self as >::Item> { /* ... */ }
    ```

  - ```rust
    fn size_hint(self: &Self) -> (usize, Option<usize>) { /* ... */ }
    ```

- **ParallelBridge**
  - ```rust
    fn par_bridge(self: Self) -> IterBridge<T> { /* ... */ }
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
#### Struct `DecayChainIter`

```rust
pub struct DecayChainIter<''a> {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

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

- **ExactSizeIterator**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **FusedIterator**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **IntoIterator**
  - ```rust
    fn into_iter(self: Self) -> I { /* ... */ }
    ```

- **Iterator**
  - ```rust
    fn next(self: &mut Self) -> Option<<Self as >::Item> { /* ... */ }
    ```

  - ```rust
    fn size_hint(self: &Self) -> (usize, Option<usize>) { /* ... */ }
    ```

- **ParallelBridge**
  - ```rust
    fn par_bridge(self: Self) -> IterBridge<T> { /* ... */ }
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
#### Struct `DecayChainIterMut`

```rust
pub struct DecayChainIterMut<''a> {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

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

- **ExactSizeIterator**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **FusedIterator**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **IntoIterator**
  - ```rust
    fn into_iter(self: Self) -> I { /* ... */ }
    ```

- **Iterator**
  - ```rust
    fn next(self: &mut Self) -> Option<<Self as >::Item> { /* ... */ }
    ```

  - ```rust
    fn size_hint(self: &Self) -> (usize, Option<usize>) { /* ... */ }
    ```

- **ParallelBridge**
  - ```rust
    fn par_bridge(self: Self) -> IterBridge<T> { /* ... */ }
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

#### Struct `StochasticDecayChain`

StochasticDecayChain classes give a single path of the decay chain
based on random number generator

```rust
pub struct StochasticDecayChain {
    pub nuclides_and_decay_data_vec: Vec<(fission_yields_data::prelude::Nuclide, crate::prelude::HalfLifeAndDecayEnergyInfo)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nuclides_and_decay_data_vec` | `Vec<(fission_yields_data::prelude::Nuclide, crate::prelude::HalfLifeAndDecayEnergyInfo)>` |  |

##### Implementations

###### Methods

- ```rust
  pub fn iter(self: &Self) -> DecayChainIter<''_> { /* ... */ }
  ```

- ```rust
  pub fn iter_mut(self: &mut Self) -> DecayChainIterMut<''_> { /* ... */ }
  ```

- ```rust
  pub fn new_single_stochastic_chain_from_nuclide(starting_nuclide: Nuclide, decay_library: &mut DecayLibrary) -> StochasticDecayChain { /* ... */ }
  ```
  this function returns a single decay chain

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> StochasticDecayChain { /* ... */ }
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
- **IntoIterator**
  - ```rust
    fn into_iter(self: Self) -> <Self as >::IntoIter { /* ... */ }
    ```

  - ```rust
    fn into_iter(self: Self) -> <Self as >::IntoIter { /* ... */ }
    ```

  - ```rust
    fn into_iter(self: Self) -> <Self as >::IntoIter { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &StochasticDecayChain) -> bool { /* ... */ }
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
## Module `monte_carlo_single_radionuclide_decay_simulator`

this code here is meant to simulate decay chains
basically, a single particle is simulated

The nuclide will be supplied into the simulator,
the simulator will then determine the decay chain
and how much time there is to decay.

The simulator, can of course, determine the radiation as well
released, but that is another time.

this is not really vibe coded (still used chatgpt 5 advise on
some algorithms)

```rust
pub mod monte_carlo_single_radionuclide_decay_simulator { /* ... */ }
```

### Modules

## Module `postprocessing`

```rust
pub mod postprocessing { /* ... */ }
```

### Types

#### Struct `SingleNuclideSimulatorMC`

```rust
pub struct SingleNuclideSimulatorMC {
    pub position: (Length, Length, Length),
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `position` | `(Length, Length, Length)` | a position vector representing the position of the nuclide<br>this is based on cartesian coordinates |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn chain_nuclides_unique_sorted(self: &Self) -> Vec<Nuclide> { /* ... */ }
  ```

- ```rust
  pub fn all_chain_nuclides_unique_sorted(sims: &[SingleNuclideSimulatorMC]) -> Vec<Nuclide> { /* ... */ }
  ```

- ```rust
  pub fn count_nuclides_in_sims_linear(sims: &[SingleNuclideSimulatorMC], unique: &[Nuclide]) -> Vec<(Nuclide, u64)> { /* ... */ }
  ```

- ```rust
  pub fn get_time_to_decay_stochastic(rng: &mut Rand64, half_life: Time) -> Time { /* ... */ }
  ```
  this obtains a time to live stochastically for the decay chain using

- ```rust
  pub fn new_decay_chain_simulation(current_nuclide: Nuclide, decay_library: &mut DecayLibrary) -> Self { /* ... */ }
  ```
  generate a new decay chain simulation

- ```rust
  pub fn transmute_nuclide(self: &mut Self, nuclide: Nuclide, decay_library: &mut DecayLibrary) { /* ... */ }
  ```
  generate a new decay chain simulation

- ```rust
  pub fn advance_timestep(self: &mut Self, timestep: Time) -> (Nuclide, HalfLifeAndDecayEnergyInfo) { /* ... */ }
  ```

- ```rust
  pub fn get_time_to_next_decay(self: &Self) -> Time { /* ... */ }
  ```
  as function name implies, get time to next decay

- ```rust
  pub fn get_current_nuclide(self: &Self) -> Nuclide { /* ... */ }
  ```
  as name implies, gets current nuclide

- ```rust
  pub fn get_next_decay_nuclide(self: &Self) -> Option<Nuclide> { /* ... */ }
  ```
  as function name implies, get nuclide in next decay

- ```rust
  pub fn get_time_to_live_vec(self: &Self) -> Vec<Time> { /* ... */ }
  ```
  as function name implies, get the time to live vector

- ```rust
  pub fn get_decay_chain_vec(self: &Self) -> Vec<Nuclide> { /* ... */ }
  ```

- ```rust
  pub fn get_current_simulated_time(self: &Self) -> Time { /* ... */ }
  ```
  gets current simulated time

- ```rust
  pub fn get_current_elapsed_time(self: &Self) -> Time { /* ... */ }
  ```
  gets current elapsed time

- ```rust
  pub fn get_current_half_life_info(self: &Self) -> HalfLifeAndDecayEnergyInfo { /* ... */ }
  ```

- ```rust
  pub fn get_current_half_life(self: &Self) -> Time { /* ... */ }
  ```

- ```rust
  pub fn get_decay_constant(self: &Self) -> Radioactivity { /* ... */ }
  ```

- ```rust
  pub fn force_decay_to_next_nuclide(self: &mut Self) -> (Nuclide, HalfLifeAndDecayEnergyInfo) { /* ... */ }
  ```

- ```rust
  pub fn check_if_current_nuclide_matches(self: &Self, nuclide_to_check: Nuclide) -> bool { /* ... */ }
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
    fn clone(self: &Self) -> SingleNuclideSimulatorMC { /* ... */ }
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
    fn eq(self: &Self, other: &SingleNuclideSimulatorMC) -> bool { /* ... */ }
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
## Module `lagrangian_diffusion`

Diffusion problems normally run on a continuum basis,

I chose Lagrangian-style diffusion here as it is easy to visualise

moreover, it is compatible with the monte carlo style of the simulator
it is quite visual.

```rust
pub mod lagrangian_diffusion { /* ... */ }
```

### Modules

## Module `central_limit_theorem`

this module contains functions for Gaussian distributions,
where multiple isotropic scatterings are summed together to
produce a Gaussian distribution due to the central limit theorem

this is partly vibe coded from ChatGPT, then edited to fit the needs
of this crate

```rust
pub mod central_limit_theorem { /* ... */ }
```

### Modules

## Module `oorandom_rng`

OoRng64 adapter (now wraps the OpenMC LCG instead of oorandom)

```rust
pub mod oorandom_rng { /* ... */ }
```

### Types

#### Struct `OoRng64`

Stateful RNG adapter for the diffusion simulators.

Previously wrapped `oorandom::Rand64` and implemented `rand_core::RngCore`.
Now wraps the OpenMC LCG `u64` state directly; `rand_core` is no longer a
dependency.  The inner state is public (`pub .0`) so that call sites in
`single_particle_simulator/mod.rs` can pass `&mut rng.0` directly to the
`seed: &mut u64` samplers in `isotropic_scattering` and `central_limit_theorem`.

```rust
pub struct OoRng64(pub u64);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `u64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn from_u128(seed: u128) -> Self { /* ... */ }
  ```
  Create from a 128-bit seed (same signature as the old `oorandom::Rand64::new`).

- ```rust
  pub fn from_u64(seed: u64) -> Self { /* ... */ }
  ```
  Create from a 64-bit seed.

- ```rust
  pub fn rand_float(self: &mut Self) -> f64 { /* ... */ }
  ```
  Return a uniform float in [0, 1) and advance the state.

- ```rust
  pub fn next_u64(self: &mut Self) -> u64 { /* ... */ }
  ```
  Advance the state and return the raw 64-bit word.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> OoRng64 { /* ... */ }
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
    fn eq(self: &Self, other: &OoRng64) -> bool { /* ... */ }
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

#### Function `per_component_variance_from_second_moment_u64`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Compute per-component variance sigma2 for the Gaussian displacement after n isotropic steps.
General case: sigma2 = n * E[S^2] / 3.
For exponential step lengths with mean lambda, E[S^2] = 2 lambda^2 ⇒ sigma2 = n * 2 lambda^2 / 3.


```rust
pub fn per_component_variance_from_second_moment_u64(no_of_collisions: u64, e_s2: Area) -> Area { /* ... */ }
```

#### Function `per_component_variance_exponential_for_3d_vector_u64`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

this obtains the variance given n random collisions
and a mean free path length

denoted as lambda

this is meant for 3d vector

```rust
pub fn per_component_variance_exponential_for_3d_vector_u64(no_of_collisions: u64, mean_free_path: Length) -> Area { /* ... */ }
```

#### Function `per_component_variance_from_second_moment`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Compute per-component variance sigma2 for the Gaussian displacement after n isotropic steps.
General case: sigma2 = n * E[S^2] / 3.
For exponential step lengths with mean lambda, E[S^2] = 2 lambda^2 ⇒ sigma2 = n * 2 lambda^2 / 3.


```rust
pub fn per_component_variance_from_second_moment(no_of_collisions: f64, e_s2: Area) -> Area { /* ... */ }
```

#### Function `per_component_variance_exponential_for_3d_vector`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

this obtains the variance given n random collisions
and a mean free path length

denoted as lambda

this is meant for 3d vector

```rust
pub fn per_component_variance_exponential_for_3d_vector(no_of_collisions: f64, mean_free_path: Length) -> Area { /* ... */ }
```

#### Function `sample_dimensioned_gaussian_vector`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Sample a 3D Gaussian displacement vector X ~ N(0, sigma2 * I3).

```rust
pub fn sample_dimensioned_gaussian_vector(seed: &mut u64, per_component_variance: Area) -> [Length; 3] { /* ... */ }
```

## Module `isotropic_scattering`

contains functions for isotropic scattering
allows particle to finish random walk with isotropic scattering

```rust
pub mod isotropic_scattering { /* ... */ }
```

### Types

#### Struct `Vec3`

```rust
pub struct Vec3 {
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
  pub fn add(self: &Self, other: Vec3) -> Vec3 { /* ... */ }
  ```

- ```rust
  pub fn scale(self: &Self, s: f64) -> Vec3 { /* ... */ }
  ```

- ```rust
  pub fn norm(self: &Self) -> f64 { /* ... */ }
  ```

- ```rust
  pub fn normalize(self: &Self) -> Vec3 { /* ... */ }
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
    fn clone(self: &Self) -> Vec3 { /* ... */ }
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
## Module `temperature_dependent_collisions`

this module converts a thermodynamic temperature into a number
of collisions expected on a per unit time basis

```rust
pub mod temperature_dependent_collisions { /* ... */ }
```

### Modules

## Module `diffusion_coeffs`

```rust
pub mod diffusion_coeffs { /* ... */ }
```

### Functions

#### Function `get_d1_for_ag`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

from Jiang 2023
Jiang, W., Toptan, A., Hales, J. D., Spencer, B. W., &
Novascone, S. R. (2023). Fission product transport in TRISO particles
and pebbles (No. INL/EXT-21-63549-Rev001). Idaho National Lab.(INL),
Idaho Falls, ID (United States).

from Jiang 2023
Jiang, W., Toptan, A., Hales, J. D., Spencer, B. W., &
Novascone, S. R. (2023). Fission product transport in TRISO particles
and pebbles (No. INL/EXT-21-63549-Rev001). Idaho National Lab.(INL),
Idaho Falls, ID (United States).

table on page 13 of 105

```rust
pub fn get_d1_for_ag(triso_layer: crate::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::TrisoPebbleLayerMaterial) -> DiffusionCoefficient { /* ... */ }
```

#### Function `get_q1_for_ag`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn get_q1_for_ag(triso_layer: crate::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::TrisoPebbleLayerMaterial) -> MolarEnergy { /* ... */ }
```

#### Function `get_d1_for_cs`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn get_d1_for_cs(triso_layer: crate::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::TrisoPebbleLayerMaterial, gamma_fast_neutron_fluence: ArealNumberDensity) -> DiffusionCoefficient { /* ... */ }
```

#### Function `get_q1_for_cs`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn get_q1_for_cs(triso_layer: crate::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::TrisoPebbleLayerMaterial) -> MolarEnergy { /* ... */ }
```

#### Function `get_d2_for_cs`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn get_d2_for_cs(triso_layer: crate::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::TrisoPebbleLayerMaterial) -> DiffusionCoefficient { /* ... */ }
```

#### Function `get_q2_for_cs`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn get_q2_for_cs(triso_layer: crate::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::TrisoPebbleLayerMaterial) -> MolarEnergy { /* ... */ }
```

#### Function `get_d1_for_sr`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn get_d1_for_sr(triso_layer: crate::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::TrisoPebbleLayerMaterial) -> DiffusionCoefficient { /* ... */ }
```

#### Function `get_q1_for_sr`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn get_q1_for_sr(triso_layer: crate::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::TrisoPebbleLayerMaterial) -> MolarEnergy { /* ... */ }
```

#### Function `get_d2_for_sr`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn get_d2_for_sr(triso_layer: crate::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::TrisoPebbleLayerMaterial) -> DiffusionCoefficient { /* ... */ }
```

#### Function `get_q2_for_sr`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn get_q2_for_sr(triso_layer: crate::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::TrisoPebbleLayerMaterial) -> MolarEnergy { /* ... */ }
```

#### Function `get_d1_for_kr`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn get_d1_for_kr(triso_layer: crate::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::TrisoPebbleLayerMaterial, temperature: ThermodynamicTemperature) -> DiffusionCoefficient { /* ... */ }
```

#### Function `get_q1_for_kr`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn get_q1_for_kr(triso_layer: crate::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::TrisoPebbleLayerMaterial, temperature: ThermodynamicTemperature) -> MolarEnergy { /* ... */ }
```

#### Function `get_d2_for_kr`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn get_d2_for_kr(triso_layer: crate::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::TrisoPebbleLayerMaterial, temperature: ThermodynamicTemperature) -> DiffusionCoefficient { /* ... */ }
```

#### Function `get_q2_for_kr`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn get_q2_for_kr(triso_layer: crate::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::TrisoPebbleLayerMaterial, temperature: ThermodynamicTemperature) -> MolarEnergy { /* ... */ }
```

### Types

#### Enum `TrisoPebbleLayerMaterial`

triso layer for diffusion

```rust
pub enum TrisoPebbleLayerMaterial {
    KernelUO2,
    PyC,
    SiC,
    MatrixGraphite,
    StructuralGraphite,
    CrackedMaterial,
    Buffer,
}
```

##### Variants

###### `KernelUO2`

###### `PyC`

###### `SiC`

###### `MatrixGraphite`

###### `StructuralGraphite`

###### `CrackedMaterial`

from CRP 6 tests within
Hales, J. D., Jiang, W., Toptan, A., & Gamble,
K. A. (2021). Modeling fission product
diffusion in TRISO fuel particles with BISON.
Journal of Nuclear Materials, 548, 152840.

Tests 3d and 3e have cracked material,
wherein the diffusion coefficient is
1e-6 m2/s

###### `Buffer`

buffer layer

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
    fn clone(self: &Self) -> TrisoPebbleLayerMaterial { /* ... */ }
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
    fn eq(self: &Self, other: &TrisoPebbleLayerMaterial) -> bool { /* ... */ }
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

#### Function `mean_speed`

Mean speed (Maxwell–Boltzmann) at temperature T for a particle of mass m:
v_mean = sqrt(8 k_B T / (pi m))

used uom si botlzmann constant

```rust
pub fn mean_speed(medium_temperature: ThermodynamicTemperature, particle_mass: Mass) -> Velocity { /* ... */ }
```

#### Function `expected_collisions_atomic_jumps`

Expected number of collisions in time t with mean free path ℓ:
E[N(t)] = (t / ℓ) * E[v]
Returns a dimensionless count (f64).


now this is not quite atomic jumps as chatGPT suggested,
D = 1/6 a^2 * nu

However, atomic jumps assume diffusion is only within monocrystalline
material without defects.

In reality, there are defects, grain boundaries, dislocations etc.
Therefore, we need an effective diffusion coefficient to consider
this

```rust
pub fn expected_collisions_atomic_jumps(medium_temperature: ThermodynamicTemperature, particle_mass: Mass, mean_free_path: Length, t: Time) -> f64 { /* ... */ }
```

#### Function `try_get_diffusion_coeff_jiang`

diffusion coefficient
from Jiang 2023
Jiang, W., Toptan, A., Hales, J. D., Spencer, B. W., &
Novascone, S. R. (2023). Fission product transport in TRISO particles
and pebbles (No. INL/EXT-21-63549-Rev001). Idaho National Lab.(INL),
Idaho Falls, ID (United States).

D = D1 exp (-Q1/RT) + D2 exp (-Q2/RT)

Neutron fluence is also a factor,
but if there is no neutron fluence, just give the None enum

```rust
pub fn try_get_diffusion_coeff_jiang(triso_layer: TrisoPebbleLayerMaterial, nuclide: fission_yields_data::prelude::Nuclide, temperature: ThermodynamicTemperature, gamma_neutron_fluence: Option<ArealNumberDensity>) -> Option<DiffusionCoefficient> { /* ... */ }
```

## Module `single_particle_simulator`

this is for simulation of a single particle
isotropic material and isotropic scattering (no medium boundaries and
such).

```rust
pub mod single_particle_simulator { /* ... */ }
```

### Modules

## Module `interaction_with_decaying_nuclide_simulator`

implements conversion and interaction with the
SingleNuclideSimulatorMC

```rust
pub mod interaction_with_decaying_nuclide_simulator { /* ... */ }
```

## Module `movement_within_triso_particle`

implements movement within triso particle regime

```rust
pub mod movement_within_triso_particle { /* ... */ }
```

## Module `constructive_solid_geometry`

next challenge is how do we include geometry?
There is simple constructive solid geometry,
then there are more complex things like STL files


I mean there can be more complex ways to do things,
but the simplest is with constructive solid geometry

simplest thing is a sphere.

where the norm can be used to determine if a a coordinate is within
the sphere or not

```rust
pub mod constructive_solid_geometry { /* ... */ }
```

### Modules

## Module `norms`

```rust
pub mod norms { /* ... */ }
```

### Functions

#### Function `l2_norms_sq_3d_f64`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn l2_norms_sq_3d_f64(vs: &[[f64; 3]], out: &mut [f64]) { /* ... */ }
```

#### Function `l2_norms_sqrt_3d_f64`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn l2_norms_sqrt_3d_f64(vs: &[[f64; 3]], out: &mut [f64]) { /* ... */ }
```

## Module `chatgpt_vibe_coded_sphere_crossing`

this is a vibe coded sphere crossing code
to determine time to sphere crossing

```rust
pub mod chatgpt_vibe_coded_sphere_crossing { /* ... */ }
```

### Types

#### Enum `SphereCrossing`

```rust
pub enum SphereCrossing {
    Entry {
        t: uom::si::f64::Time,
    },
    Exit {
        t: uom::si::f64::Time,
    },
}
```

##### Variants

###### `Entry`

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `t` | `uom::si::f64::Time` |  |

###### `Exit`

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `t` | `uom::si::f64::Time` |  |

##### Implementations

###### Methods

- ```rust
  pub fn time(self: Self) -> Time { /* ... */ }
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
    fn clone(self: &Self) -> SphereCrossing { /* ... */ }
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
    fn eq(self: &Self, other: &SphereCrossing) -> bool { /* ... */ }
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

#### Function `sphere_first_crossing_uom`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Earliest forward-time crossing of the sphere surface by x(t) = p + t*v,
classified as Entry or Exit, using `uom` units.

All computations are done in SI base units (m, s) internally.

```rust
pub fn sphere_first_crossing_uom(center: [uom::si::f64::Length; 3], radius: uom::si::f64::Length, position: [uom::si::f64::Length; 3], velocity: [uom::si::f64::Velocity; 3]) -> Option<SphereCrossing> { /* ... */ }
```

### Types

#### Enum `Region`

```rust
pub enum Region {
    Sphere(Sphere),
}
```

##### Variants

###### `Sphere`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Sphere` |  |

##### Implementations

###### Methods

- ```rust
  pub fn new_sphere(center: [Length; 3], radius: Length) -> Self { /* ... */ }
  ```

- ```rust
  pub fn is_within_region(self: &Self, point: [Length; 3]) -> bool { /* ... */ }
  ```

- ```rust
  pub fn try_return_center_and_radius_of_sphere(self: &Self) -> Option<([Length; 3], Length)> { /* ... */ }
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
    fn clone(self: &Self) -> Region { /* ... */ }
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
    fn eq(self: &Self, other: &Region) -> bool { /* ... */ }
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
#### Struct `TrisoCell`

```rust
pub struct TrisoCell {
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
  pub fn new(fuel_radius: Length, buffer_radius: Length, ipyc_radius: Length, sic_radius: Length, opyc_radius: Length) -> Self { /* ... */ }
  ```
  creates a new triso cell based on the radii

- ```rust
  pub fn new_crp6_geometry() -> Self { /* ... */ }
  ```
  gotten typical triso geometry from:

- ```rust
  pub fn get_triso_region(self: &Self, coordinates: [Length; 3]) -> TrisoRegion { /* ... */ }
  ```
  checks which region the particle is in

- ```rust
  pub fn try_get_diffusion_coefficient(self: &Self, coordinates: [Length; 3], nuclide: Nuclide) -> Option<DiffusionCoefficient> { /* ... */ }
  ```
  checks the diffusion coefficient based on coordinates of the

- ```rust
  pub fn get_lengthscale_for_fourier_number(self: &Self, coordinates: [Length; 3]) -> Length { /* ... */ }
  ```
  for the outside region, i just get opyc radius, not going to

- ```rust
  pub fn get_time_to_sphere_boundary(self: &Self, position: [Length; 3], velocity: [Velocity; 3]) -> Option<Time> { /* ... */ }
  ```

- ```rust
  pub fn set_uniform_temperature(self: &mut Self, temp: ThermodynamicTemperature) { /* ... */ }
  ```
  Sets a uniform temperature across all regions of the TRISO cell.

- ```rust
  pub fn get_uniform_temperature(self: &Self) -> ThermodynamicTemperature { /* ... */ }
  ```
  Gets the current uniform temperature of the TRISO cell.

- ```rust
  pub fn get_fuel_radius(self: &Self) -> Length { /* ... */ }
  ```

- ```rust
  pub fn get_buffer_radius(self: &Self) -> Length { /* ... */ }
  ```

- ```rust
  pub fn get_ipyc_radius(self: &Self) -> Length { /* ... */ }
  ```

- ```rust
  pub fn get_sic_radius(self: &Self) -> Length { /* ... */ }
  ```

- ```rust
  pub fn get_opyc_radius(self: &Self) -> Length { /* ... */ }
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
    fn clone(self: &Self) -> TrisoCell { /* ... */ }
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
    fn eq(self: &Self, other: &TrisoCell) -> bool { /* ... */ }
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
#### Enum `TrisoRegion`

```rust
pub enum TrisoRegion {
    Fuel,
    Buffer,
    IPyC,
    SiC,
    OPyC,
    Outside,
}
```

##### Variants

###### `Fuel`

###### `Buffer`

###### `IPyC`

###### `SiC`

###### `OPyC`

###### `Outside`

##### Implementations

###### Methods

- ```rust
  pub fn get_time_to_sphere_boundary(position: [Length; 3], velocity: [Velocity; 3], triso_cell: TrisoCell) -> Option<Time> { /* ... */ }
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
    fn clone(self: &Self) -> TrisoRegion { /* ... */ }
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
    fn eq(self: &Self, other: &TrisoRegion) -> bool { /* ... */ }
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

#### Re-export `norms::*`

```rust
pub use norms::*;
```

## Module `release_fraction_analytical_solution`

from
https://www-eng.lbl.gov/~shuman/NEXT/MATERIALS&COMPONENTS/Xe_damage/Crank-The-Mathematics-of-Diffusion.pdf
page 91
the total amount of diffusing substance entering or leaving a sphere is
Mt/M_infty = 1 - 6/(pi^2) \sum_(i=1)^infty 1/n^2 exp (- D n^2 pi^2 t/a^2)

Crank, J. (1975). The mathematics of diffusion (2nd ed.). Clarendon Press.

This is for a sphere


```rust
pub mod release_fraction_analytical_solution { /* ... */ }
```

### Functions

#### Function `calculate_analytical_fraction_released`

Calculates the analytical fraction of material released from a sphere over time.

This solution is for diffusion from a sphere of radius `radius`
with a constant diffusion coefficient `diffusion_coefficient`,
assuming a uniform initial concentration within the sphere
and a perfect sink (zero concentration) at the surface.

# Arguments
* `diffusion_coefficient` - The constant diffusion coefficient (e.g., in m²/s).
* `radius` - The radius of the sphere (e.g., in m).
* `time` - The elapsed time (e.g., in s).
* `num_terms` - The number of terms to use in the infinite series summation.
                More terms provide higher accuracy, but 10-20 are usually sufficient.

# Returns
A `f64` representing the fraction of material released (between 0.0 and 1.0).

# Panics
Panics if `radius` is zero or `time` is negative.

```rust
pub fn calculate_analytical_fraction_released(diffusion_coefficient: DiffusionCoefficient, radius: Length, time: Time, num_terms: usize) -> f64 { /* ... */ }
```

## Module `cached_normals`

this is for caching of standard normals so that simulations are sped up

```rust
pub mod cached_normals { /* ... */ }
```

### Types

#### Struct `DiffusionRandomCache`

Fast cached pool of standard normal random numbers for diffusion simulation

```rust
pub struct DiffusionRandomCache {
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
  pub fn new(cache_size: usize) -> Self { /* ... */ }
  ```
  Create a new cache with the specified number of pre-generated samples.

- ```rust
  pub fn get_normal(self: &Self) -> f64 { /* ... */ }
  ```
  Get a single standard normal sample

- ```rust
  pub fn get_normal_3d(self: &Self) -> (f64, f64, f64) { /* ... */ }
  ```
  Get three independent normal samples (for x, y, z)

- ```rust
  pub fn get_displacement_3d(self: &Self, scale: f64) -> (f64, f64, f64) { /* ... */ }
  ```
  Get scaled displacement for 3D diffusion

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn fmt(self: &Self, f: &mut std::fmt::Formatter<''_>) -> std::fmt::Result { /* ... */ }
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
- **Sync**
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
## Module `release_fraction_crp_6_case_1a_1b`

for CRP 6 case 1a and 1b we can compare the Monte Carlo simulation
to the analytical solution

TO BE DONE

```rust
pub mod release_fraction_crp_6_case_1a_1b { /* ... */ }
```

### Modules

## Module `simulation_code`

Monte-Carlo fractional-release simulation for the bare-kernel CRP-6 Case 1.
Monte-Carlo fractional release from a bare fuel kernel (IAEA CRP-6 Case 1).

CRP-6 Case 1 is single-layer diffusion: a spherical fuel kernel with a
**uniform initial concentration** of a fission product and a **perfect-sink**
surface. The fraction released by time `t` has the closed-form Crank series
solution (see [`calculate_analytical_fraction_released`]); this module
reproduces it with the Walk-on-Spheres first-passage engine, which is the
Lagrangian counterpart of that continuum (Eulerian) solution.

[`calculate_analytical_fraction_released`]:
crate::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::release_fraction_analytical_solution::calculate_analytical_fraction_released

```rust
pub mod simulation_code { /* ... */ }
```

### Functions

#### Function `mc_kernel_release_fraction`

Monte-Carlo fractional release of `nuclide` from a bare spherical UO2 kernel.

Places `n_histories` atoms uniformly through the kernel volume, walks each to
the perfect-sink surface with the Walk-on-Spheres engine, and returns the
fraction whose release time is at or before `time`. The kernel diffusion
coefficient is the temperature-dependent Jiang correlation (no neutron
fluence), matching the analytical verification cases in the sibling
`release_fraction_analytical_solution` module.

# Arguments

- `nuclide` — the diffusing fission product (e.g. `Cs137`).
- `kernel_radius` — the UO2 kernel radius (`Length`; 212.5 µm for CRP-6).
- `temperature` — kernel temperature (`ThermodynamicTemperature`).
- `time` — elapsed time at which the release fraction is evaluated.
- `n_histories` — number of independent atoms simulated (statistical error
  scales as `1/sqrt(n_histories)`).
- `seed` — RNG seed for reproducibility.

# Returns

The released fraction in `[0, 1]`.

```rust
pub fn mc_kernel_release_fraction(nuclide: fission_yields_data::prelude::Nuclide, kernel_radius: Length, temperature: ThermodynamicTemperature, time: Time, n_histories: usize, seed: u64) -> f64 { /* ... */ }
```

#### Function `kernel_diffusion_coefficient`

The kernel diffusion coefficient used by [`mc_kernel_release_fraction`], for
reporting alongside a release fraction (e.g. in a V&V record).

```rust
pub fn kernel_diffusion_coefficient(nuclide: fission_yields_data::prelude::Nuclide, temperature: ThermodynamicTemperature) -> DiffusionCoefficient { /* ... */ }
```

### Types

#### Struct `SingleParticleDiffusionSimulatorMC`

```rust
pub struct SingleParticleDiffusionSimulatorMC {
    pub position: (Length, Length, Length),
    pub rng: crate::lagrangian_decay_simulator::lagrangian_diffusion::central_limit_theorem::oorandom_rng::OoRng64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `position` | `(Length, Length, Length)` |  |
| `rng` | `crate::lagrangian_decay_simulator::lagrangian_diffusion::central_limit_theorem::oorandom_rng::OoRng64` | random number generator |

##### Implementations

###### Methods

- ```rust
  pub fn move_single_decaying_particle_isotropically(self: &mut Self, single_particle_sim: &mut SingleNuclideSimulatorMC, sigma_s: LinearNumberDensity) { /* ... */ }
  ```
  moves the particle in the SingleNuclideSimulatorMC

- ```rust
  pub fn move_single_decaying_particle_gaussian_mfp_and_no_of_collisions(self: &mut Self, single_particle_sim: &mut SingleNuclideSimulatorMC, mean_free_path: Length, no_of_collisions: u64) { /* ... */ }
  ```
  moves the particle in the SingleNuclideSimulatorMC

- ```rust
  pub fn move_single_decaying_particle_within_triso(self: &mut Self, single_particle_sim: &mut SingleNuclideSimulatorMC, triso_cell: TrisoCell, timestep: Time) { /* ... */ }
  ```
  moves the particle in the SingleNuclideSimulatorMC

- ```rust
  pub fn move_single_decaying_particle_within_triso_based_on_fourier_no(self: &mut Self, single_particle_sim: &mut SingleNuclideSimulatorMC, triso_cell: TrisoCell, timestep: Time) { /* ... */ }
  ```
  this helps to auto_timestep based on the fourier number

- ```rust
  pub fn scatter_within_triso_particle_gaussian(self: &mut Self, triso_cell: TrisoCell, nuclide: Nuclide, timestep: Time) { /* ... */ }
  ```

- ```rust
  pub fn scatter_within_triso_particle_gaussian_simple(self: &mut Self, triso_cell: TrisoCell, nuclide: Nuclide, timestep: Time) { /* ... */ }
  ```
  this deals with movement within triso particles

- ```rust
  pub fn move_single_decaying_particle_gaussian_triso_particle(self: &mut Self, single_particle_sim: &mut SingleNuclideSimulatorMC, triso_cell: TrisoCell, timestep: Time) { /* ... */ }
  ```
  moves the particle in the SingleNuclideSimulatorMC

- ```rust
  pub fn scatter_within_triso_particle_brute_force(self: &mut Self, triso_cell: TrisoCell, nuclide: Nuclide, timestep: Time) { /* ... */ }
  ```

- ```rust
  pub fn scatter_within_triso_particle_gaussian_cached(self: &mut Self, triso_cell: TrisoCell, nuclide: Nuclide, timestep: Time, cache: &DiffusionRandomCache) { /* ... */ }
  ```
  CACHED VERSION: Scatter within TRISO particle using Gaussian sampling with boundary handling

- ```rust
  pub fn scatter_within_triso_particle_gaussian_simple_cached(self: &mut Self, triso_cell: TrisoCell, nuclide: Nuclide, timestep: Time, cache: &DiffusionRandomCache) { /* ... */ }
  ```
  CACHED VERSION: Simplified Gaussian scattering within TRISO particle

- ```rust
  pub fn move_single_decaying_particle_gaussian_triso_particle_cached(self: &mut Self, single_particle_sim: &mut SingleNuclideSimulatorMC, triso_cell: TrisoCell, timestep: Time, cache: &DiffusionRandomCache) { /* ... */ }
  ```
  CACHED VERSION: Move single decaying particle with Gaussian sampling in TRISO particle

- ```rust
  pub fn move_single_decaying_particle_within_triso_based_on_fourier_no_cached(self: &mut Self, single_particle_sim: &mut SingleNuclideSimulatorMC, triso_cell: TrisoCell, timestep: Time, cache: &DiffusionRandomCache) { /* ... */ }
  ```
  this helps to auto_timestep based on the fourier number

- ```rust
  pub fn new_from_rng(outside_rng: &mut OoRng64) -> Self { /* ... */ }
  ```
  constructor for new diffusion simulator

- ```rust
  pub fn move_particle_using_array(self: &mut Self, length_array: [Length; 3]) { /* ... */ }
  ```
  this moves the particle by an array

- ```rust
  pub fn move_particle_using_tuple(self: &mut Self, length_tuple: (Length, Length, Length)) { /* ... */ }
  ```
  this moves the particle by an tuple

- ```rust
  pub fn move_particle_gaussian_sampling_u64(self: &mut Self, mean_free_path: Length, no_of_collisions: u64) { /* ... */ }
  ```
  move particle assuming normal distribution

- ```rust
  pub fn move_particle_gaussian_sampling_f64(self: &mut Self, mean_free_path: Length, no_of_collisions: f64) { /* ... */ }
  ```
  move particle assuming normal distribution

- ```rust
  pub fn get_gaussian_velocity_vector(self: &mut Self, mean_free_path: Length, collision_rate: Frequency) -> [Velocity; 3] { /* ... */ }
  ```

- ```rust
  pub fn sample_isotropic_direction(self: &mut Self) -> [Ratio; 3] { /* ... */ }
  ```
  samples isotropic direction

- ```rust
  pub fn sample_mean_free_path_given_sigma_s(self: &mut Self, sigma_s: LinearNumberDensity) -> Length { /* ... */ }
  ```
  samples distance travelled given a mean free path

- ```rust
  pub fn scatter_isotropically_using_macro_xs(self: &mut Self, sigma_s: LinearNumberDensity) { /* ... */ }
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
    fn clone(self: &Self) -> SingleParticleDiffusionSimulatorMC { /* ... */ }
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
    fn eq(self: &Self, other: &SingleParticleDiffusionSimulatorMC) -> bool { /* ... */ }
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
## Module `first_passage`

Walk-on-Spheres / Green's-function first-passage diffusion — the exact,
timestep-free replacement for the single-Gaussian step, which overshoots the
thin buffer layer (see `docs/buffer_clt_failure_analysis.md`). Respects the
TRISO layer interfaces by construction.
Walk-on-Spheres / Green's-function first-passage diffusion.

This module replaces the single-Gaussian (central-limit-theorem) diffusion
step with an **exact, timestep-free** random walk that respects the TRISO
layer interfaces. The motivation — why the Gaussian step overshoots the
buffer layer — is written up in `docs/buffer_clt_failure_analysis.md`.

## What belongs here

- [`sphere_fpt`] — first-passage statistics for 3-D Brownian motion started
  at the centre of a sphere: the mean exit time and (added in the CPU
  engine) the exit-time sampler and the uniform exit direction.
- [`walk_on_spheres`] — the [`walk_on_spheres::WoSWalker`] itself and the
  geometry helper [`walk_on_spheres::nearest_interface_distance`] that sizes
  each interface-free hop from the concentric-sphere `TrisoCell` geometry.
- [`interface`] — the transmission/reflection rule applied when a walker
  reaches a layer interface (continuity of concentration and flux, with an
  optional partition coefficient).
- [`depletion`] — decay and (neutron-field) transmutation as competing
  clocks alongside each hop, so the walker changes nuclide as it diffuses;
  the ensemble of identities over time is the depleted inventory.
- [`ensemble`] — rayon-parallel drivers over many independent histories
  (release fraction, depletion outcomes), since each walk is independent.

## What does not belong here

Diffusion-coefficient correlations (they live in
`temperature_dependent_collisions`), the concentric-sphere geometry itself
(it lives in `single_particle_simulator::constructive_solid_geometry`), and
decay/transmutation bookkeeping (that couples in from
`nuclide_reaction_and_decay_data` and the transmutation simulator).

The pre-existing Gaussian-step code under `single_particle_simulator` is
left in place; this engine is additive and is the intended replacement for
the diffusion core.

```rust
pub mod first_passage { /* ... */ }
```

### Modules

## Module `depletion`

Decay and transmutation coupled to the Walk-on-Spheres diffusion.

A diffusing atom does not keep its identity forever: it decays along its
chain, and under a neutron field it can capture, (n,2n), or fission. This
module runs those as **competing exponential clocks alongside each
Walk-on-Spheres hop**. For the current nuclide the total event rate is

```text
lambda_total = lambda_decay + lambda_transmute,
lambda_decay = ln2 / t_half,
lambda_transmute = phi * sigma   (an external neutron field),
```

and the time to the next event is `Exp(lambda_total)`. Each hop takes a
first-passage time `tau`; if an event time falls inside `tau`, the atom
changes nuclide (its diffusion coefficient updates on the spot) instead of
completing that hop.

The ensemble of walker identities over time **is** the depleted inventory —
no Bateman matrix, no CRAM, no stiffness handling (the design the crate's
`CLAUDE.md` calls for).

## Scope and the one approximation

- **Decay** is fully wired to the crate's `DecayLibrary` (real half-lives and
  branching ratios); the daughter is drawn from the walker's own RNG stream.
- **Transmutation** is a framework that takes the neutron field as an
  **external** input ([`Transmutation`]). A single `(n,gamma)` channel is
  provided as the minimal, explicit stand-in; wiring per-nuclide cross
  sections and fission yields to `njoy-outram-park-fork` /
  `outram-mc-libs` flux maps is the deferred follow-up.
- **Placement approximation.** When an event preempts a hop, the atom is left
  at the hop's *start* position (its identity changes there) while its clock
  advances by the exact event time. This is unbiased in *timing* but
  conservative in *position*; it is accurate whenever the hop time is short
  compared with the event time (the usual case, since a mobile — high `D` —
  nuclide has short hops, and a slow — low `D` — nuclide barely moves in the
  event time either way). The exact Brownian-bridge placement is a possible
  refinement.

```rust
pub mod depletion { /* ... */ }
```

### Types

#### Enum `Transmutation`

The external neutron field driving transmutation.

This is where the neutron flux enters the Lagrangian model. It is kept
deliberately explicit and small; a per-nuclide cross-section library and
fission-yield sampling (from `njoy-outram-park-fork`) plug in here later.

```rust
pub enum Transmutation {
    None,
    SingleCapture {
        target: fission_yields_data::prelude::Nuclide,
        capture_rate: uom::si::f64::Frequency,
        product: fission_yields_data::prelude::Nuclide,
    },
}
```

##### Variants

###### `None`

No neutron field — radioactive decay only.

###### `SingleCapture`

A single `(n,gamma)` capture channel: while the walker is `target`, it
captures at rate `capture_rate = phi * sigma_capture` and becomes
`product`. A minimal explicit stand-in for the deferred per-nuclide
cross-section coupling.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `target` | `fission_yields_data::prelude::Nuclide` | The nuclide the channel acts on. |
| `capture_rate` | `uom::si::f64::Frequency` | Capture rate `phi * sigma_capture` folded into one frequency. |
| `product` | `fission_yields_data::prelude::Nuclide` | The nuclide produced by capture. |

##### Implementations

###### Methods

- ```rust
  pub fn rate_for(self: &Self, nuclide: Nuclide) -> Frequency { /* ... */ }
  ```
  Transmutation rate acting on `nuclide` under this field.

- ```rust
  pub fn product_for(self: &Self, nuclide: Nuclide) -> Option<Nuclide> { /* ... */ }
  ```
  The product `nuclide` transmutes into under this field, if any channel

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Transmutation { /* ... */ }
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
    fn eq(self: &Self, other: &Transmutation) -> bool { /* ... */ }
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
#### Enum `DepletionOutcome`

The fate of a walker after [`WoSWalker::advance_until`].

```rust
pub enum DepletionOutcome {
    Released {
        time: uom::si::f64::Time,
        nuclide: fission_yields_data::prelude::Nuclide,
    },
    Surviving {
        nuclide: fission_yields_data::prelude::Nuclide,
    },
    StepLimit {
        nuclide: fission_yields_data::prelude::Nuclide,
    },
}
```

##### Variants

###### `Released`

The atom was released from the OPyC surface at `time` as `nuclide`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `time` | `uom::si::f64::Time` | Simulated release time. |
| `nuclide` | `fission_yields_data::prelude::Nuclide` | Nuclide identity at release. |

###### `Surviving`

The atom was still inside the particle when the requested time was
reached, as `nuclide`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `nuclide` | `fission_yields_data::prelude::Nuclide` | Nuclide identity at the requested time. |

###### `StepLimit`

The step cap was hit before either of the above (a runaway guard).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `nuclide` | `fission_yields_data::prelude::Nuclide` | Nuclide identity when the cap was hit. |

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
    fn clone(self: &Self) -> DepletionOutcome { /* ... */ }
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
    fn eq(self: &Self, other: &DepletionOutcome) -> bool { /* ... */ }
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

#### Function `sample_event_time`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Sample a waiting time `~ Exp(rate)` from a uniform deviate `u` in `[0, 1)`.

Returns `-ln(1 - u) / rate`, whose mean is `1/rate`. An event is deemed to
occur within an interval `tau` iff the returned time is `< tau` (which holds
with probability `1 - exp(-rate*tau)`), so this one draw serves as both the
occurrence test and the event time.

```rust
pub fn sample_event_time(rate: uom::si::f64::Frequency, u: f64) -> uom::si::f64::Time { /* ... */ }
```

## Module `ensemble`

Parallel ensembles of independent Lagrangian histories.

Each atom's Walk-on-Spheres history is completely independent of every other,
so an ensemble is embarrassingly parallel. This module runs the histories
across cores with `rayon`, giving a real, verifiable speedup on the CPU (the
wgpu compute path in [`super::super`]'s GPU module extends the same idea to
the GPU for very large, real-time ensembles).

Reproducibility: each history `i` gets an independent RNG stream seeded
deterministically from a base seed and `i` (a SplitMix64 mix), so a run is
bit-for-bit reproducible and independent of how `rayon` schedules the work.

```rust
pub mod ensemble { /* ... */ }
```

### Types

#### Struct `EnsembleConfig`

Size and seeding of a parallel ensemble.

```rust
pub struct EnsembleConfig {
    pub n_histories: usize,
    pub base_seed: u64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `n_histories` | `usize` | Number of independent atom histories to simulate. |
| `base_seed` | `u64` | Base RNG seed; combined with the history index for per-atom streams. |

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
    fn clone(self: &Self) -> EnsembleConfig { /* ... */ }
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
    fn eq(self: &Self, other: &EnsembleConfig) -> bool { /* ... */ }
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

#### Function `history_seed`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Independent, well-separated RNG seed for history `index` from `base_seed`.

A SplitMix64 finaliser so that consecutive indices produce far-apart streams
(a plain `base + index` would give highly correlated LCG sequences).

```rust
pub fn history_seed(base_seed: u64, index: usize) -> u64 { /* ... */ }
```

#### Function `parallel_kernel_release_fraction`

Parallel Monte-Carlo fractional release from a bare fuel kernel (CRP-6 Case
1), evaluated at `time`.

The parallel counterpart of
[`super::super::single_particle_simulator::release_fraction_crp_6_case_1a_1b::simulation_code::mc_kernel_release_fraction`]:
same physics, histories spread across cores. Returns the released fraction in
`[0, 1]`.

```rust
pub fn parallel_kernel_release_fraction(nuclide: fission_yields_data::prelude::Nuclide, kernel_radius: Length, temperature: ThermodynamicTemperature, time: Time, config: &EnsembleConfig) -> f64 { /* ... */ }
```

#### Function `parallel_advance_until`

Parallel multilayer + depletion ensemble.

Births `config.n_histories` atoms of `initial_nuclide` at the particle centre
and advances each — diffusing while decaying and transmuting — until it is
released or its simulated time reaches `until`. Returns one
[`DepletionOutcome`] per history. The decay library is shared read-only across
threads.

```rust
pub fn parallel_advance_until(initial_nuclide: fission_yields_data::prelude::Nuclide, triso_cell: &crate::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::TrisoCell, params: &crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::walk_on_spheres::WalkParams, decay_library: &crate::nuclide_reaction_and_decay_data::decay_library::DecayLibrary, transmutation: crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::depletion::Transmutation, until: Time, config: &EnsembleConfig) -> Vec<crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::depletion::DepletionOutcome> { /* ... */ }
```

#### Function `released_fraction`

Fraction of an outcome set that was released (an inventory-release summary).

```rust
pub fn released_fraction(outcomes: &[crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::depletion::DepletionOutcome]) -> f64 { /* ... */ }
```

## Module `interface`

Transmission and reflection at a TRISO layer interface.

When a Walk-on-Spheres walker reaches an interface between two materials with
diffusion coefficients `D1` (the side it is on) and `D2` (the side it is
crossing into), it does not pass freely: the diffusion equation requires the
concentration **and** the flux to stay continuous across the interface (the
standard BISON / Jiang TRISO treatment — no chemical segregation unless a
partition coefficient is supplied). The equilibrium such an interface must
reproduce is a **uniform concentration** for `K = 1` (zero net flux implies a
flat profile), or a concentration ratio `c2/c1 = K` for a partition ratio
`K`.

The transmission probability that reproduces this depends on **how often the
particular random-walk scheme visits the interface**. In Walk-on-Spheres the
walker is reinserted a fixed distance from the interface and then takes a hop
whose duration is `tau ~ R^2 / D`, so the *rate* of interface encounters from
side `i` scales as `D_i` (the geometric return probability per hop is the
same on both sides). Detailed balance at equilibrium,
`c1 * D1 * p_(1->2) = c2 * D2 * p_(2->1)` with `c2/c1 = K`, then gives

```text
p_transmit = K * D2 / ( D1 + K * D2 ).
```

(This is the rule for the *Walk-on-Spheres* encounter statistics. A
fixed-time-step walk, whose step length scales as `sqrt(D)` and whose
encounter rate scales as `1/sqrt(D)`, needs the different
`sqrt(D)`-ratio rule — using that here would give the wrong equilibrium. See
the `interface_rule_gives_uniform_equilibrium_density` test in
`walk_on_spheres`, which checks the density is uniform to a few percent
across a 10x diffusivity contrast.)

This is the piece that turns SiC — whose `D` is ~10^6 times smaller than the
pyrolytic-carbon layers around it — into the containment barrier: a walker
arriving from PyC transmits into SiC with probability `~ D_SiC / D_PyC`, i.e.
it is reflected back the overwhelming majority of the time.

```rust
pub mod interface { /* ... */ }
```

### Functions

#### Function `transmission_probability`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Probability that a walker arriving at a `D1 | D2` interface transmits into
the `D2` side (rather than reflecting back into the `D1` side).

Implements `p = K*D2 / (D1 + K*D2)`, the rule that makes the Walk-on-Spheres
scheme reproduce Fickian diffusion with continuity of concentration (a
concentration ratio `K` across the interface). See the module docs for why
this is linear in `D`, not `sqrt(D)`, for this scheme.

# Arguments

- `d_current` — diffusion coefficient on the side the walker is currently on
  (`D1`), a [`DiffusionCoefficient`] (m^2/s).
- `d_next` — diffusion coefficient on the side being entered (`D2`), a
  [`DiffusionCoefficient`] (m^2/s).
- `partition_k` — dimensionless partition/solubility ratio `K` (equilibrium
  `c2/c1`). Use `1.0` for plain concentration continuity (the default TRISO
  assumption).

# Returns

A probability in `[0, 1]`. Returns `0.0` if both coefficients are zero (an
impenetrable interface), so the walker always reflects.

```rust
pub fn transmission_probability(d_current: uom::si::f64::DiffusionCoefficient, d_next: uom::si::f64::DiffusionCoefficient, partition_k: f64) -> f64 { /* ... */ }
```

#### Function `does_transmit`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Decide whether a walker arriving at a `D1 | D2` interface transmits.

Draws one uniform deviate from `seed` (the caller's LCG state) and returns
`true` with probability [`transmission_probability`]`(d_current, d_next,
partition_k)`, `false` otherwise (reflection).

```rust
pub fn does_transmit(seed: &mut u64, d_current: uom::si::f64::DiffusionCoefficient, d_next: uom::si::f64::DiffusionCoefficient, partition_k: f64) -> bool { /* ... */ }
```

## Module `live`

Live, interactive Walk-on-Spheres ensemble for real-time visualisation.

A [`LiveEnsemble`] owns a population of diffusing atoms and advances them a
slice of simulated time per call to [`LiveEnsemble::advance_frame`], choosing
the execution backend at runtime with a [`ComputeType`]. It is designed to be
driven from a **background worker thread**: the worker advances the ensemble
and publishes a small [`Snapshot`] (atom positions + release fraction) through
an `Arc<RwLock<…>>`, while the UI thread only reads the latest snapshot and
renders. Nothing here touches egui — the GUI examples own the thread and the
shared state; this type is the compute core they share.

The three backends produce the **same physics**; they differ only in how the
independent histories are executed (one thread, all cores via `rayon`, or a
`wgpu` kernel). See [`crate::compute::ComputeType`].

```rust
pub mod live { /* ... */ }
```

### Types

#### Struct `Snapshot`

A render-ready snapshot of a [`LiveEnsemble`] at one instant.

Small and cheap to clone/publish every frame — it carries only what a viewer
draws, not the full walker state.

```rust
pub struct Snapshot {
    pub positions_xy_um: Vec<[f64; 2]>,
    pub released_fraction: f64,
    pub sim_time_s: f64,
    pub n_total: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `positions_xy_um` | `Vec<[f64; 2]>` | `(x, y)` of every still-contained atom, in micrometres (a 2-D slice). |
| `released_fraction` | `f64` | Fraction of the ensemble released from the OPyC surface, in `[0, 1]`. |
| `sim_time_s` | `f64` | Simulated time of this snapshot, in seconds. |
| `n_total` | `usize` | Total number of atoms in the ensemble. |

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
    fn clone(self: &Self) -> Snapshot { /* ... */ }
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
    fn default() -> Snapshot { /* ... */ }
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
#### Struct `LiveEnsemble`

A population of diffusing atoms advanced with a runtime-selectable backend.

```rust
pub struct LiveEnsemble {
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
  pub fn new(cell: TrisoCell, params: WalkParams, nuclide: Nuclide, temperature: ThermodynamicTemperature, n_histories: usize, base_seed: u64) -> Self { /* ... */ }
  ```
  Build an ensemble of `n_histories` atoms of `nuclide`, born uniformly in

- ```rust
  pub fn reset(self: &mut Self) { /* ... */ }
  ```
  Re-birth the whole ensemble at the kernel with a fresh clock (`t = 0`).

- ```rust
  pub fn sim_time(self: &Self) -> Time { /* ... */ }
  ```
  Current simulated time.

- ```rust
  pub fn cell(self: &Self) -> &TrisoCell { /* ... */ }
  ```
  The TRISO geometry this ensemble diffuses in (layer radii for drawing).

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Number of atoms.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether the ensemble is empty.

- ```rust
  pub fn released_fraction(self: &Self) -> f64 { /* ... */ }
  ```
  Fraction of atoms released from the OPyC surface so far, in `[0, 1]`.

- ```rust
  pub fn snapshot(self: &Self) -> Snapshot { /* ... */ }
  ```
  Build a render snapshot of the current state.

- ```rust
  pub fn advance_frame(self: &mut Self, compute: ComputeType, until: Time) { /* ... */ }
  ```
  Advance every still-contained atom by pure diffusion until its simulated

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

#### Function `micrometres`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Convenience: a micrometre `Length` (used by GUI callers building geometry).

```rust
pub fn micrometres(x: f64) -> uom::si::f64::Length { /* ... */ }
```

## Module `sphere_fpt`

First-passage statistics for a diffusing atom inside a sphere.

Consider a point Brownian walker with diffusion coefficient `D` started at
the **centre** of an absorbing sphere of radius `R`. The time it takes to
first reach the surface is the *first-passage time* `tau`. Working in the
dimensionless time `theta = D t / R^2`, the probability that the walker is
**still inside** at time `t` (the survival probability) is the eigenfunction
series

```text
S(theta) = 2 * sum_{k>=1} (-1)^(k+1) * exp(-k^2 * pi^2 * theta),
```

obtained by solving the diffusion equation in a sphere with an absorbing
surface and a point source at the centre. Its mean is `E[tau] = R^2 / (6 D)`
(equivalently `E[theta] = 1/6`).

The Walk-on-Spheres engine ([`super::walk_on_spheres`]) advances a walker one
interface-free sphere at a time. Each hop sphere is centred on the walker, so
the **centre-start** distribution above is exactly the per-hop exit-time law:
the exit point is uniform on the sphere (isotropy) and the elapsed time is a
draw from `S`.

Because `theta = D t / R^2` is dimensionless, the distribution of `theta` is
**universal** — independent of `R` and `D`. We therefore build one inverse-CDF
table for `theta` once and reuse it for every hop of every walker, converting
to a physical time with `t = theta * R^2 / D`.

```rust
pub mod sphere_fpt { /* ... */ }
```

### Functions

#### Function `mean_first_passage_time`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Mean first-passage time for a walker started at the centre of a sphere.

Returns `E[tau] = R^2 / (6 D)`, the expected time for 3-D Brownian motion
with diffusion coefficient `diffusion_coefficient` to first reach the
surface of a sphere of radius `radius`, starting from its centre.

# Units

- `radius` — a [`uom`] `Length` (any unit; metres internally).
- `diffusion_coefficient` — a [`uom`] `DiffusionCoefficient` (m^2/s).
- returns a [`uom`] `Time` (seconds).

# Valid range

`radius > 0` and `diffusion_coefficient > 0`. The formula is exact for any
positive inputs; it carries no approximation.

```rust
pub fn mean_first_passage_time(radius: uom::si::f64::Length, diffusion_coefficient: uom::si::f64::DiffusionCoefficient) -> uom::si::f64::Time { /* ... */ }
```

#### Function `survival_probability`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Survival probability `S(theta)` that a centre-started walker is still inside
the sphere at dimensionless time `theta = D t / R^2`.

Evaluates `2 * sum_{k>=1} (-1)^(k+1) exp(-k^2 pi^2 theta)`. The terms are
positive, strictly decreasing in `k`, and alternate in sign, so the
truncation error is bounded by the first omitted term; the sum runs until
that term drops below `1e-15`. Returns `1.0` for `theta <= 0` and is clamped
to `[0, 1]`.

```rust
pub fn survival_probability(theta: f64) -> f64 { /* ... */ }
```

#### Function `sample_dimensionless_exit_time`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Sample the dimensionless exit time `theta = D * tau / R^2` for one hop.

Draws from the universal centre-start first-passage distribution using the
inverse-CDF table. `seed` is the caller's LCG state (advanced by one draw).

```rust
pub fn sample_dimensionless_exit_time(seed: &mut u64) -> f64 { /* ... */ }
```

#### Function `sample_first_passage_time`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Sample the physical first-passage time for a hop of radius `radius` in a
medium with diffusion coefficient `diffusion_coefficient`.

Returns `tau = theta * R^2 / D` with `theta` drawn from the universal
distribution. `seed` is the caller's LCG state (advanced by one draw).

# Units

`radius` is a `Length`, `diffusion_coefficient` a `DiffusionCoefficient`
(m^2/s); the result is a `Time` (seconds).

```rust
pub fn sample_first_passage_time(seed: &mut u64, radius: uom::si::f64::Length, diffusion_coefficient: uom::si::f64::DiffusionCoefficient) -> uom::si::f64::Time { /* ... */ }
```

#### Function `sample_uniform_direction`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Sample a direction uniform on the unit sphere, as a dimensionless
`[x, y, z]` unit vector. Used for the exit point of each Walk-on-Spheres hop
(uniform by the isotropy of Brownian motion started at the sphere centre).

```rust
pub fn sample_uniform_direction(seed: &mut u64) -> [f64; 3] { /* ... */ }
```

#### Function `dimensionless_exit_time_table`

Tabulate the dimensionless exit time `theta` on a **uniform** CDF grid:
entry `j` is `theta` at `F = j / (m - 1)` for `j = 0..m`.

Unlike the internal inverse-CDF table (which is queried by binary search),
this is a directly-indexable lookup — `theta(u) ~ table[u*(m-1)]` with linear
interpolation — so it can be uploaded to a GPU and sampled without a search.
`m` must be at least 2.

```rust
pub fn dimensionless_exit_time_table(m: usize) -> Vec<f64> { /* ... */ }
```

## Module `walk_on_spheres`

The Walk-on-Spheres walker and the geometry that sizes each hop.

A [`WoSWalker`] is a single diffusing atom: a position, the nuclide it
currently is, its accumulated simulated time, and its own random-number
stream. The engine advances it by repeatedly (a) finding the largest sphere
centred on the walker that contains no layer interface, then (b) jumping to
a uniform point on that sphere while adding the corresponding first-passage
time (see [`super::sphere_fpt`]). Because the sphere touches — but never
crosses — the nearest interface, an atom can never teleport across a thin
layer the way the single-Gaussian step does (see
`docs/buffer_clt_failure_analysis.md`).

This Phase-0 scaffold defines the walker type and the geometry helper
[`nearest_interface_distance`], which turns the concentric-sphere `TrisoCell`
into the hop radius `R`. The stochastic `hop` itself, the outer-surface
escape test, and the interface handling are added in the CPU-engine phases.

```rust
pub mod walk_on_spheres { /* ... */ }
```

### Types

#### Enum `HopOutcome`

Outcome of a single Walk-on-Spheres hop within a [`TrisoCell`].

```rust
pub enum HopOutcome {
    Stepped,
    Released,
    ReachedInterface,
}
```

##### Variants

###### `Stepped`

The walker advanced by one interface-free sphere and is still strictly
inside the particle.

###### `Released`

The walker reached the OPyC outer surface and is released from the
particle.

###### `ReachedInterface`

The walker reached an interior layer interface and it was resolved this
step (it transmitted into the neighbour or reflected back). The walk
continues from the reinserted position on the next step.

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
    fn clone(self: &Self) -> HopOutcome { /* ... */ }
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
    fn eq(self: &Self, other: &HopOutcome) -> bool { /* ... */ }
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
#### Struct `WalkParams`

Tunables for a multilayer Walk-on-Spheres walk.

```rust
pub struct WalkParams {
    pub capture_eps: uom::si::f64::Length,
    pub reinsert_factor: f64,
    pub partition_k: f64,
    pub max_steps: u64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `capture_eps` | `uom::si::f64::Length` | Distance below which a hop is treated as having *reached* the nearest<br>interface rather than continuing to shrink. Bounds the geometric<br>approach to a boundary; smaller values are more accurate but take more<br>hops. Choose it small relative to the thinnest layer. |
| `reinsert_factor` | `f64` | After an interface is resolved, the walker is reinserted this many<br>`capture_eps` away from the interface (on the chosen side) so the next<br>hop is a genuine hop and not an immediate re-trigger. Must be `> 1`. |
| `partition_k` | `f64` | Partition/solubility ratio `K` at every interface (`1.0` = plain<br>concentration continuity, the default TRISO assumption). |
| `max_steps` | `u64` | Safety cap on the number of steps before a walk is abandoned (returns<br>`None`), guarding against a walker that never escapes within budget. |

##### Implementations

###### Methods

- ```rust
  pub fn crp6_default() -> Self { /* ... */ }
  ```
  Sensible defaults for a CRP-6-scale particle: `capture_eps = 10 nm`

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> WalkParams { /* ... */ }
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
    fn eq(self: &Self, other: &WalkParams) -> bool { /* ... */ }
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
#### Struct `WoSWalker`

A single diffusing atom tracked by the Walk-on-Spheres engine.

# Fields

- `position` — Cartesian position `[x, y, z]` as [`uom`] `Length`s, measured
  from the TRISO particle centre.
- `nuclide` — the atom's current identity; changes when it decays or
  transmutes (handled in the depletion phase).
- `time` — accumulated simulated time (a [`uom`] `Time`) since the walk
  began; each hop adds its first-passage time to this.
- `rng` — the walker's private RNG stream ([`OoRng64`], the workspace LCG).

```rust
pub struct WoSWalker {
    pub position: [uom::si::f64::Length; 3],
    pub nuclide: fission_yields_data::prelude::Nuclide,
    pub time: uom::si::f64::Time,
    pub rng: crate::lagrangian_decay_simulator::lagrangian_diffusion::central_limit_theorem::oorandom_rng::OoRng64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `position` | `[uom::si::f64::Length; 3]` | Cartesian position from the particle centre, in `uom` `Length`. |
| `nuclide` | `fission_yields_data::prelude::Nuclide` | Current nuclide identity. |
| `time` | `uom::si::f64::Time` | Accumulated simulated time since the walk began. |
| `rng` | `crate::lagrangian_decay_simulator::lagrangian_diffusion::central_limit_theorem::oorandom_rng::OoRng64` | Private random-number stream for this walker. |

##### Implementations

###### Methods

- ```rust
  pub fn decay_rate(self: &Self, decay_library: &DecayLibrary) -> Frequency { /* ... */ }
  ```
  Radioactive decay rate `ln2 / t_half` of the walker's current nuclide,

- ```rust
  pub fn decay_once(self: &mut Self, decay_library: &DecayLibrary) -> bool { /* ... */ }
  ```
  Replace the current nuclide with a stochastically chosen decay daughter,

- ```rust
  pub fn advance_until(self: &mut Self, triso_cell: &TrisoCell, params: &WalkParams, decay_library: &DecayLibrary, transmutation: Transmutation, until: Time) -> DepletionOutcome { /* ... */ }
  ```
  Advance the walker — diffusing while decaying and transmuting — until it

- ```rust
  pub fn new(position: [Length; 3], nuclide: Nuclide, rng: OoRng64) -> Self { /* ... */ }
  ```
  Create a walker at an explicit position with a fresh time of zero.

- ```rust
  pub fn new_at_center(nuclide: Nuclide, rng: OoRng64) -> Self { /* ... */ }
  ```
  Create a walker at the particle centre (the origin) with time zero.

- ```rust
  pub fn radius(self: &Self) -> Length { /* ... */ }
  ```
  Radial distance of the walker from the particle centre, `|position|`.

- ```rust
  pub fn hop(self: &mut Self, triso_cell: &TrisoCell, capture_eps: Length) -> HopOutcome { /* ... */ }
  ```
  Perform one Walk-on-Spheres hop inside the walker's current [`TrisoCell`]

- ```rust
  pub fn walk_to_absorbing_sphere(self: &mut Self, sphere_radius: Length, diffusion_coefficient: DiffusionCoefficient, capture_eps: Length) -> Time { /* ... */ }
  ```
  Walk to the surface of a single homogeneous absorbing sphere and return

- ```rust
  pub fn step_multilayer(self: &mut Self, triso_cell: &TrisoCell, params: &WalkParams) -> HopOutcome { /* ... */ }
  ```
  Advance one multilayer step: either a genuine Walk-on-Spheres hop within

- ```rust
  pub fn diffuse_until(self: &mut Self, triso_cell: &TrisoCell, params: &WalkParams, until: Time) -> bool { /* ... */ }
  ```
  Advance the walker by pure diffusion until its simulated time reaches

- ```rust
  pub fn walk_until_released(self: &mut Self, triso_cell: &TrisoCell, params: &WalkParams) -> Option<Time> { /* ... */ }
  ```
  Run the multilayer walk until the walker is released from the OPyC outer

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> WoSWalker { /* ... */ }
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
    fn eq(self: &Self, other: &WoSWalker) -> bool { /* ... */ }
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

#### Function `sample_uniform_in_ball`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Sample a point uniformly in the volume of a ball of radius `radius`, centred
on the origin, returned as a `[uom]` `Length` triple.

Uses `r = radius * U^(1/3)` for the radial coordinate (so the point is
volume-uniform, not radius-uniform) and an isotropic direction. This is the
birth distribution of a fission product created uniformly in a spherical
fuel kernel, and the initial condition for the Crank release comparison.

```rust
pub fn sample_uniform_in_ball(seed: &mut u64, radius: uom::si::f64::Length) -> [uom::si::f64::Length; 3] { /* ... */ }
```

#### Function `radial_distance`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Radial distance of a point from the particle centre, `sqrt(x^2+y^2+z^2)`.

```rust
pub fn radial_distance(position: [uom::si::f64::Length; 3]) -> uom::si::f64::Length { /* ... */ }
```

#### Function `nearest_interface_distance`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Distance from `position` to the nearest layer interface of `triso_cell`.

This is the radius `R` of the largest interface-free sphere the walker may
hop across. For the fuel kernel the only bounding surface is the kernel
outer sphere, so `R` is the distance out to it. For any coating shell the
walker is bounded on both sides, and `R` is the smaller of the distance in
to the inner sphere and out to the outer sphere.

Returns `None` if the walker is already outside the particle (it has been
released and there is no containing shell).

# Units

`position` components and the returned distance are [`uom`] `Length`s.

```rust
pub fn nearest_interface_distance(triso_cell: &crate::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::TrisoCell, position: [uom::si::f64::Length; 3]) -> Option<uom::si::f64::Length> { /* ... */ }
```

#### Function `shell_bounds`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

The inner and outer bounding-sphere radii of the shell containing
`position`.

Returns `(inner, outer)` where `inner` is `None` for the fuel kernel (which
has no inner boundary) and `outer` is the shell's outer radius. Returns
`None` if the walker is outside the particle. This is the geometry the
Walk-on-Spheres step uses to size hops and to identify which interface a
walker has reached.

```rust
pub fn shell_bounds(triso_cell: &crate::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::TrisoCell, position: [uom::si::f64::Length; 3]) -> Option<(Option<uom::si::f64::Length>, uom::si::f64::Length)> { /* ... */ }
```

### Re-exports

#### Re-export `stochastic_decay_chain::*`

```rust
pub use stochastic_decay_chain::*;
```

## Module `lagrangian_transmutation_and_fission_simulator`

this is the part that deals with transmutation and fission
simulation in lagrangian
Lagrangian transmutation & fission — competing-rate depletion.

The whole point of the Lagrangian Monte Carlo approach is that transmutation
and fission need **no burnup matrix**: each atom samples waiting times from
competing exponential clocks (decay, `(n,gamma)`, `(n,2n)`, fission), and the
population distribution emerges from the ensemble. There is no stiff Bateman
system, no CRAM, no matrix exponential.

The implementation lives with the diffusion engine it is coupled to, in
[`crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::depletion`]:
decay is wired to the crate's [`DecayLibrary`], and the neutron field enters
through the [`Transmutation`] input (currently a single explicit
`(n,gamma)` channel — the point where per-nuclide cross sections and fission
yields from `njoy-outram-park-fork` / `outram-mc-libs` flux maps plug in).

The competing-rates framework is described in the crate `CLAUDE.md`
("MC transmutation design sketch"): total rate `lambda = lambda_decay +
phi*sigma_ng + phi*sigma_n2n + phi*sigma_f`, waiting time `Exp(lambda)`, and
the event chosen by the usual competing-rates method. Fission-fragment yields
come from the ENDF/B-VIII.0 data available via `openmc-endf-8-depletion-lib-b`.

[`DecayLibrary`]: crate::nuclide_reaction_and_decay_data::decay_library::DecayLibrary
[`Transmutation`]: crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::depletion::Transmutation

```rust
pub mod lagrangian_transmutation_and_fission_simulator { /* ... */ }
```

### Re-exports

#### Re-export `sample_event_time`

**Attributes:**

- `Other("#[doc(inline)]")`

```rust
pub use crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::depletion::sample_event_time;
```

#### Re-export `DepletionOutcome`

**Attributes:**

- `Other("#[doc(inline)]")`

```rust
pub use crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::depletion::DepletionOutcome;
```

#### Re-export `Transmutation`

**Attributes:**

- `Other("#[doc(inline)]")`

```rust
pub use crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::depletion::Transmutation;
```

## Module `gpu`

**Attributes:**

- `Other("#[attr = CfgTrace([All([Not(NameValue { name: \"target_os\", value: Some(\"android\"), span: crates/boon-lay/src/lib.rs:52:15: 52:36 (#0) }, crates/boon-lay/src/lib.rs:52:14: 52:37 (#0)), Not(NameValue { name: \"target_arch\", value: Some(\"wasm32\"), span: crates/boon-lay/src/lib.rs:52:43: 52:65 (#0) }, crates/boon-lay/src/lib.rs:52:42: 52:66 (#0))], crates/boon-lay/src/lib.rs:52:10: 52:67 (#0))])]")`

Optional wgpu GPU acceleration for large Walk-on-Spheres ensembles. Compiled
only off Android (the workspace GPU/Android rule) **and off wasm**; the CPU
path in `lagrangian_diffusion::first_passage::ensemble` is always available
and is the trusted reference. See the module docs for the CPU-fallback
contract.

The wasm exclusion is not a policy choice but a type-system one: wgpu's
WebGPU backend holds `Rc<Cell<u32>>` internally, so `GpuContext` is `!Send`
there and the `static OnceLock<Option<GpuContext>>` this module caches it in
cannot compile. Reaching WebGPU from wasm needs a `thread_local!` cache
instead — a real change, not a gate — and is tracked separately.
Optional **wgpu GPU acceleration** for large Walk-on-Spheres ensembles.

Follows the workspace GPU rules and the `outram-park-fork-pflotran` /
`outram-mc-libs` precedent:

1. **Android-gated.** The whole module is compiled only under
   `cfg(not(target_os = "android"))` (wired in `lib.rs`); the Android/Termux
   build never sees `wgpu`, keeping the library headless-buildable there.
2. **CPU is the trusted path; GPU is acceleration only.** The `f64` CPU
   reference is the rayon ensemble
   ([`parallel_kernel_release_fraction`]); the GPU kernel runs in `f32` and
   is an *approximation*. [`kernel_release_fraction_best_effort`] probes for a
   device and silently falls back to the CPU when there is no adapter
   (headless CI, no `/dev/dri`) or the submit fails.

The kernel runs one **single-sphere Walk-on-Spheres history per GPU thread**
(the IAEA CRP-6 Case 1 bare-kernel release): each thread starts an atom
uniformly in the kernel, hops it to the perfect-sink surface using the same
first-passage-time table as the CPU engine (uploaded once), and writes its
release time. The host then reduces the release times to a release fraction.
This is the embarrassingly-parallel core that makes a very large, real-time
ensemble feasible; extending the kernel to the full multilayer geometry with
interfaces and depletion is the next buildout.

> **GPU path unverified in this environment.** Developed where no GPU adapter
> was available, so only the CPU fallback was exercised here. The GPU dispatch
> must be validated on a GPU-equipped host before it is trusted. The kernel is
> written to mirror the verified CPU `walk_to_absorbing_sphere` logic; the
> per-thread `f32` RNG is a lightweight xorshift adequate for a demonstrator.

```rust
pub mod gpu { /* ... */ }
```

### Types

#### Struct `GpuContext`

A ready wgpu device + queue for headless compute.

```rust
pub struct GpuContext {
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
  pub fn adapter_name(self: &Self) -> &str { /* ... */ }
  ```
  Human-readable adapter name (e.g. the GPU model), for UI display.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
#### Enum `GpuError`

A recoverable GPU failure — the caller should fall back to the CPU path.

```rust
pub enum GpuError {
    Poll(String),
    Map(String),
    MapCallbackMissing,
}
```

##### Variants

###### `Poll`

The device could not be polled to completion (device lost).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `Map`

The readback buffer could not be mapped.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `MapCallbackMissing`

The map callback never fired.

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
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, f: &mut std::fmt::Formatter<''_>) -> std::fmt::Result { /* ... */ }
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
### Functions

#### Function `cached_context`

A process-wide cached [`GpuContext`], probed once on first use.

Opening a `wgpu` device is expensive, so the per-frame animation path must not
[`probe`] every frame. The first call probes; every later call reuses the same
device (or the same `None` when there is no adapter). The context is `Send +
Sync`, so the returned reference is safe to use from the render/worker thread.

```rust
pub fn cached_context() -> Option<&'static GpuContext> { /* ... */ }
```

#### Function `gpu_available`

Whether a GPU adapter is available (probed once, cached). For a UI to show the
effective backend without re-opening a device each frame.

```rust
pub fn gpu_available() -> bool { /* ... */ }
```

#### Function `probe`

Probe for a GPU adapter and open a device. Returns `None` when there is no
usable adapter — a **normal, expected** outcome on headless CI / no-GPU hosts,
not an error; the caller then uses the CPU path.

Prefer [`cached_context`] on any hot path — this opens a fresh device each
call.

```rust
pub fn probe() -> Option<GpuContext> { /* ... */ }
```

#### Function `try_kernel_release_fraction_gpu`

GPU single-sphere kernel release fraction (`f32`). Fallible — the caller
should fall back to [`parallel_kernel_release_fraction`] on `Err`.

```rust
pub fn try_kernel_release_fraction_gpu(ctx: &GpuContext, nuclide: fission_yields_data::prelude::Nuclide, kernel_radius: Length, temperature: ThermodynamicTemperature, time: Time, config: &crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::ensemble::EnsembleConfig) -> Result<f64, GpuError> { /* ... */ }
```

#### Function `kernel_release_fraction_best_effort`

Single-sphere kernel release fraction using the GPU when available, otherwise
the rayon CPU reference. Never fails: falls back to
[`parallel_kernel_release_fraction`] when there is no GPU adapter or the GPU
submit errors. GPU results are `f32`-precision.

```rust
pub fn kernel_release_fraction_best_effort(nuclide: fission_yields_data::prelude::Nuclide, kernel_radius: Length, temperature: ThermodynamicTemperature, time: Time, config: &crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::ensemble::EnsembleConfig) -> f64 { /* ... */ }
```

#### Function `try_advance_multilayer_gpu`

Advance an ensemble one frame on the GPU (fallible). On `Ok`, `walkers` and
`released` are updated in place with the GPU's `f32` result.

Mirrors [`WoSWalker::diffuse_until`] for every walker up to `until`. Already
released walkers are left untouched.

```rust
pub fn try_advance_multilayer_gpu(ctx: &GpuContext, cell: &crate::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::TrisoCell, params: &crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::walk_on_spheres::WalkParams, walkers: &mut [crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::walk_on_spheres::WoSWalker], released: &mut [bool], nuclide: fission_yields_data::prelude::Nuclide, until: Time) -> Result<(), GpuError> { /* ... */ }
```

#### Function `advance_multilayer_best_effort`

Advance an ensemble one frame using the GPU when available, updating
`walkers`/`released` in place. Returns `true` if the GPU handled the frame,
`false` if there is no adapter or the submit failed (the caller then advances
on the CPU). Never panics on a missing GPU.

```rust
pub fn advance_multilayer_best_effort(cell: &crate::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::TrisoCell, params: &crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::walk_on_spheres::WalkParams, walkers: &mut [crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::walk_on_spheres::WoSWalker], released: &mut [bool], nuclide: fission_yields_data::prelude::Nuclide, until: Time) -> bool { /* ... */ }
```

## Module `triso_atops_fork`

Eulerian / continuum-diffusion TRISO fission-product release — a Rust fork of
Idaho National Laboratory's TRISO-ATOPS (MIT). This is the continuum
complement to the crate's Lagrangian (single-atom Monte-Carlo) model: it uses
closed-form analytical solutions to the Fickian diffusion equation (Booth,
breakthrough, graphite-attenuation models) to predict per-nuclide release
fractions. See `docs/triso-atops-fork.md` and the module-level docs.
# `triso_atops_fork` — Eulerian / continuum TRISO fission-product release

This module is a Rust **fork of Idaho National Laboratory's TRISO-ATOPS**
(TRISO Analysis TOol for Predictive Source terms). It is the
**Eulerian / continuum-diffusion** complement to the rest of `boon-lay`,
which models the same physics from a **Lagrangian** (single-atom
Monte-Carlo tracking) perspective.

Where the Lagrangian side walks individual atoms through the TRISO layers,
TRISO-ATOPS uses **closed-form analytical solutions to the Fickian
diffusion equation** (the *Booth* equivalent-sphere model, a *breakthrough*
model, and a graphite *attenuation* model) to predict the fraction of each
fission-product nuclide released from the fuel kernel and matrix graphite.
The equations originate from the NP-MHTGR New Production Reactor Program
(Anderson et al., "Generic Reactor Plant Description and Source Terms
Volume 1", EG&G Idaho, 1989); half-lives are from the IAEA Live Chart of
Nuclides.

## What lives where

| Submodule | Physical content |
|---|---|
| [`nuclide_model`](crate::triso_atops_fork::nuclide_model) | The TRISO-ATOPS nuclide record (Z, A, half-life, decay constant, parent), the five transport [`ElementGroup`](crate::triso_atops_fork::nuclide_model::ElementGroup)s, and the supported-nuclide database. |
| [`diffusion`](crate::triso_atops_fork::diffusion) | Arrhenius diffusion coefficients `D(T)` in m^2/s in the kernel, matrix graphite, and (for Ag) the SiC layer, plus the time-integrated `∫D dt` used by transient/accident release. |
| [`release_models`](crate::triso_atops_fork::release_models) | The dimensionless release-fraction / release-to-birth models: Booth (long-lived, short-lived), breakthrough, graphite attenuation, and their transient (accident) variants, plus the group dispatchers. |
| [`activities`](crate::triso_atops_fork::activities) | Circulating / plate-out / clean-up activity bookkeeping and the release-rate / graphite source terms, plus the Ci↔Bq and `A = λN` conversions (bead op-b4a.2.2, done). |
| [`normal_operation`](crate::triso_atops_fork::normal_operation) | Per-node normal-operation orchestration ([`normal_operation_node`](crate::triso_atops_fork::normal_operation::normal_operation_node)) composing the whole chain to curies (bead op-b4a.2.2, done). The JSON run-file driver + accident case are **not ported** — no code exists for either (bead op-b4a.2.3). |

## Derivation, step by step

The whole model is built up from two first-principles laws. This is a
condensed narrative; the full derivation (with limits, term-by-term code
correspondence, and references) is in the crate-root
`TRISO_ATOPS_DERIVATION.md` (Python-model view) and `docs/triso-atops-derivation.md`
(Rust-port view). Each step names the function that implements it.

1. **First principles.** Fickian diffusion `∂C/∂t = D∇²C` and radioactive
   decay `dN/dt = −λN`, with `λ = ln2 / t½`
   ([`TrisoAtopsNuclide::decay_constant`](crate::triso_atops_fork::nuclide_model::TrisoAtopsNuclide::decay_constant)).
   A fission product in the fuel obeys both at once:
   `∂C/∂t = D∇²C − λC + B` (birth rate `B`).
2. **Equivalent sphere.** The Booth idealisation (Booth 1957) replaces the
   real multi-shell TRISO particle with one uniform sphere of radius `a` per
   chemical group. The group partition is
   [`ElementGroup`](crate::triso_atops_fork::nuclide_model::ElementGroup); the
   special-metal sphere radius `a_booth = √(2·a_grain·r)` is formed in
   [`rb_fail`](crate::triso_atops_fork::release_models::rb_fail).
3. **Effective coefficient.** Everything depends on `D` and `a` only through
   `D' = D/a²` (units s⁻¹). `D` follows an Arrhenius law
   `D(T) = D0·exp(−Q/RT)`, implemented in
   [`diffusion_coefficient`](crate::triso_atops_fork::diffusion::diffusion_coefficient)
   and [`diffusion_coefficient_sic_ag`](crate::triso_atops_fork::diffusion::diffusion_coefficient_sic_ag).
4. **Stable-species release.** Diffusion out of the sphere gives the
   fractional release `f = 1 − (6/π²)·Σ n⁻²·exp(−n²π²·D't)` (short-time limit
   `6√(D't/π) − 3D't`), in
   [`booth_longlived`](crate::triso_atops_fork::release_models::steady_state::booth_longlived).
5. **Add decay.** Short-lived species reach a steady release-to-birth ratio
   `⟨R/B⟩ = (3/μ)(coth μ − 1/μ)`, `μ = √(λa²/D)`
   ([`booth_shortlived_fast_diffuse`](crate::triso_atops_fork::release_models::steady_state::booth_shortlived_fast_diffuse)).
   Silver permeates the SiC barrier by the Daynes–Barrer membrane time-lag
   solution ([`breakthrough_model`](crate::triso_atops_fork::release_models::steady_state::breakthrough_model));
   volatiles use an empirical fit
   ([`rb_fail_noble_gases`](crate::triso_atops_fork::release_models::steady_state::rb_fail_noble_gases));
   graphite hold-up is the attenuation factor
   ([`attenuation_factor`](crate::triso_atops_fork::release_models::steady_state::attenuation_factor)).
6. **Assemble.** Per nuclide per node: `D` → `⟨R/B⟩_fail`
   ([`rb_fail`](crate::triso_atops_fork::release_models::rb_fail)) → release
   rate `R` ([`release_rate`](crate::triso_atops_fork::activities::release_rate))
   → source `S` + graphite `G`
   ([`base_activities`](crate::triso_atops_fork::activities::base_activities))
   → loop pools `C`/`P`/`HPS`
   ([`activities::coolant_activity`](crate::triso_atops_fork::activities::coolant_activity))
   → curies. The whole chain is
   [`normal_operation_node`](crate::triso_atops_fork::normal_operation::normal_operation_node).
7. **Transient.** For an accident the products `Dt`, `D't` become time
   integrals `∫D dt`, `∫D' dt`
   ([`integrate_diffusion_over_time`](crate::triso_atops_fork::diffusion::integrate_diffusion_over_time)),
   and the Step 4/5 series are reused in
   [`release_models::transient`](crate::triso_atops_fork::release_models::transient).

## Units

Every public function takes and returns `uom` dimensioned quantities. The
named aliases below spell out what each dimensionless-or-rate quantity means
for a reader hovering in their editor:

- [`DecayConstant`](crate::triso_atops_fork::DecayConstant) — the radioactive
  decay constant `λ = ln 2 / t½`, SI unit `s^-1` (dimensionally a
  [`uom::si::f64::Frequency`]).
- [`ReleaseFraction`](crate::triso_atops_fork::ReleaseFraction) — a
  dimensionless release fraction or release-to-birth ratio in `[0, 1]` (a
  [`uom::si::f64::Ratio`]).

Temperatures are [`uom::si::f64::ThermodynamicTemperature`]; the TRISO-ATOPS
correlations are written in °C internally, so the functions read the input as
both °C (for the valid-range thresholds) and K (for the Arrhenius exponent).

## Scope of this fork

The **GUI** (`trisoatops_gui.py`) is intentionally **not** ported —
`boon-lay` is a headless library and the workspace requires non-GUI library
code to build for Android. See `docs/triso-atops-fork.md` for the full
Python→Rust module map and the port/verification status.

```rust
pub mod triso_atops_fork { /* ... */ }
```

### Modules

## Module `nuclide_model`

# Nuclide model — species records and transport groups

This module ports TRISO-ATOPS's notion of a *nuclide*: the identity
(name, atomic number `Z`, mass number `A`), the decay data (half-life and
the derived decay constant `λ`), and the parent nuclide(s) whose decay feeds
it. It also ports the **five transport groups** the code sorts every nuclide
into, because a nuclide's group decides which release model is applied to it.

## What belongs here

- [`TrisoAtopsNuclide`] — one immutable species record (ports the Python
  `Nuclide` class in `calculation_functions.py`).
- [`ElementGroup`] — the noble-gas / halogen / special-metal / silver / other
  partition used to dispatch release models (ports the `noble_gases`,
  `halogens`, `special_metals` lists and the `z == 47 or z == 46` silver test).
- [`nuclide_database`] — the ~80-nuclide supported table (ports the `nuclides`
  dict), see User Manual §2.1.3 / Table 4.

## What does *not* belong here

The runtime `sl` (short-lived) and `parent_decay` flags. In TRISO-ATOPS those
are **not intrinsic** to a nuclide — they are recomputed for each run from the
reactor's irradiation time (`nuclide_import`) and the accident duration
(`nuclide_import_accident`). They therefore live in the nodal-orchestration
layer ([`crate::triso_atops_fork::normal_operation`], scaffolded).

```rust
pub mod nuclide_model { /* ... */ }
```

### Modules

## Module `nuclide_database`

# Supported-nuclide database

The fixed table of fission-product nuclides TRISO-ATOPS supports. Ports the
module-level `nuclides` dict in `calculation_functions.py`; see User Manual
§2.1.3 and Table 4 ("Supported nuclides").

Half-lives are from the IAEA Live Chart of Nuclides (User Manual §7 ref. 2)
and are stored in **seconds**. Parent names encode the dominant decay
precursor within the TRISO-ATOPS model (e.g. `Xe-135` ← `I-135`), used to
chain parent → daughter activity during a run.

```rust
pub mod nuclide_database { /* ... */ }
```

### Functions

#### Function `supported_nuclides`

**Attributes:**

- `MustUse { reason: None }`

The full TRISO-ATOPS supported-nuclide table.

Returns every [`TrisoAtopsNuclide`] the code knows how to model, in the same
order as the upstream `nuclides` dict. Half-lives are IAEA values in seconds;
parent lists give the modelled decay precursor(s).

# Notes / faithfully-ported upstream quirks
- `Tc-99` is stored with mass number `A = 56` exactly as upstream — this is
  an apparent typo in the source table (physical `A` of Tc-99 is 99); it is
  ported verbatim so the fork stays line-for-line traceable. It does not
  affect any calculation, which uses `Z` and the half-life only.
- Several long-lived nuclides carry very large half-lives (up to ~1.6e18 s);
  these are the stable-on-reactor-timescale species.

```rust
pub fn supported_nuclides() -> Vec<super::TrisoAtopsNuclide> { /* ... */ }
```

#### Function `find_nuclide`

**Attributes:**

- `MustUse { reason: None }`

Look a nuclide up by its canonical TRISO-ATOPS name (case-sensitive).

Returns the matching [`TrisoAtopsNuclide`], or `None` if the name is not in
the supported table. Ports the `nuclides[name]` dictionary access used
throughout `calculation_functions.py` / `trisoatops.py`.

# Arguments
- `name` — canonical name such as `"Cs-137"` or `"Ag-110m"`.

```rust
pub fn find_nuclide(name: &str) -> Option<super::TrisoAtopsNuclide> { /* ... */ }
```

### Constants and Statics

#### Constant `TRISO_ATOPS_NUCLIDE_COUNT`

Number of distinct nuclides in the TRISO-ATOPS supported table.

Matches the number of keys in the upstream `nuclides` dict.

```rust
pub const TRISO_ATOPS_NUCLIDE_COUNT: usize = 84;
```

### Types

#### Struct `TrisoAtopsNuclide`

A single fission-product nuclide as modelled by TRISO-ATOPS.

Ports the Python `Nuclide` class in `calculation_functions.py`. All fields are
intrinsic nuclide data (the run-dependent `sl` / `parent_decay` flags are
**not** stored here — see the module note).

# Fields
- `name` — the canonical TRISO-ATOPS name, e.g. `"Kr-85m"`, `"Ag-110m"`,
  `"Cs-137"` (element symbol, hyphen, mass number, optional metastable `m`).
- `z` — atomic number (number of protons), dimensionless count.
- `a` — mass number (protons + neutrons), dimensionless count.
- `half_life` — the nuclide half-life `t½` (a [`Time`], SI seconds). Values
  are from the IAEA Live Chart of Nuclides (User Manual §7 ref. 2).
- `parents` — canonical names of the nuclide's decay parent(s) within the
  TRISO-ATOPS model; empty if the nuclide is treated purely as a direct
  fission product. Used to chain parent → daughter activity in a run.

```rust
pub struct TrisoAtopsNuclide {
    pub name: &'static str,
    pub z: u32,
    pub a: u32,
    pub half_life: uom::si::f64::Time,
    pub parents: &'static [&'static str],
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `&'static str` | Canonical TRISO-ATOPS nuclide name, e.g. `"Cs-137"`. |
| `z` | `u32` | Atomic number `Z` (proton count). |
| `a` | `u32` | Mass number `A` (nucleon count). |
| `half_life` | `uom::si::f64::Time` | Half-life `t½` (SI seconds). |
| `parents` | `&'static [&'static str]` | Decay parent name(s) inside the TRISO-ATOPS model; empty ⇒ direct fission product. |

##### Implementations

###### Methods

- ```rust
  pub fn from_seconds(name: &'static str, z: u32, a: u32, half_life_seconds: f64, parents: &'static [&'static str]) -> Self { /* ... */ }
  ```
  Build a nuclide record from a half-life given in **seconds**.

- ```rust
  pub fn decay_constant(self: &Self) -> DecayConstant { /* ... */ }
  ```
  The radioactive decay constant `λ = ln 2 / t½`.

- ```rust
  pub fn element_group(self: &Self) -> ElementGroup { /* ... */ }
  ```
  The transport [`ElementGroup`] this nuclide belongs to, from its `Z`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> TrisoAtopsNuclide { /* ... */ }
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
    fn eq(self: &Self, other: &TrisoAtopsNuclide) -> bool { /* ... */ }
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
#### Enum `ElementGroup`

The five transport groups TRISO-ATOPS sorts nuclides into.

A nuclide's group selects which release model is applied to it (Booth vs.
breakthrough vs. the empirical noble-gas/halogen `<R/B>` correlation) and how
its plate-out / graphite activity is handled. Ports the module-level
`noble_gases`, `halogens`, and `special_metals` lists plus the `z == 47 or
z == 46` silver test in `calculation_functions.py`. See User Manual §3.1
("Defining Nuclides").

The variants are dispatched with a `match` (no trait objects), so adding a
group forces every release-model dispatcher to handle it.

```rust
pub enum ElementGroup {
    NobleGas,
    Halogen,
    SpecialMetal,
    Silver,
    Other,
}
```

##### Variants

###### `NobleGas`

Noble gases: He, Ne, Ar, Kr, Xe, Rn (`Z ∈ {2, 10, 18, 36, 54, 86}`).
Released via the empirical `<R/B>_fail` correlation; do not plate out or
build graphite activity.

###### `Halogen`

Halogens **as defined by TRISO-ATOPS**: F, Cl, Br, I, At **plus** the
chalcogens Se and Te grouped here for transport
(`Z ∈ {9, 17, 35, 53, 85, 34, 52}`). Released via the empirical
`<R/B>_fail` correlation like noble gases, but they plate out.

###### `SpecialMetal`

Special metals: Rb, Sr, Cs, Ba, Eu (`Z ∈ {37, 38, 55, 56, 63}`).
Released via the Booth equivalent-sphere model.

###### `Silver`

Silver group: Ag and Pd (`Z ∈ {47, 46}`). Released via the breakthrough
model through the SiC layer (the limiting barrier for Ag).

###### `Other`

Any other fission metal not in the groups above. Assigned a fixed nominal
`<R/B>_fail = 1e-5` and a fixed graphite attenuation in the upstream code.

##### Implementations

###### Methods

- ```rust
  pub fn from_atomic_number(z: u32) -> Self { /* ... */ }
  ```
  Classify an atomic number `Z` into its TRISO-ATOPS transport group.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> ElementGroup { /* ... */ }
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
    fn eq(self: &Self, other: &ElementGroup) -> bool { /* ... */ }
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

#### Re-export `supported_nuclides`

```rust
pub use nuclide_database::supported_nuclides;
```

#### Re-export `TRISO_ATOPS_NUCLIDE_COUNT`

```rust
pub use nuclide_database::TRISO_ATOPS_NUCLIDE_COUNT;
```

## Module `run_selection`

Run set-up: nuclide selection, classification and inventory distribution.
Run set-up: which nuclides a run uses, how they are classified, and how a
bulk inventory is distributed over the axial nodes.

This is the layer between a user's nuclide list and the physics: it
normalises names, looks them up in the database, decides short-lived vs
long-lived against the run's own timescale, and wires parent → daughter
coupling. Upstream calls it `nuclide_import` / `nuclide_import_accident` /
`inventory_processing`.

# Upstream defects reproduced or corrected here — read before trusting

Porting this module meant deciding, for five upstream bugs, whether to
reproduce or correct. The rule applied throughout: **reproduce upstream's
observable behaviour where it is a physics choice, correct it where the
upstream code plainly states an intent its own syntax defeats** — and in
every case make the divergence explicit and selectable, never silent. Each
is documented on the item it affects. Summary:

| Upstream | Here |
|---|---|
| `parent_decay` assigned with `==` (dead short-lived test) | [`ParentDecayPolicy`] — intended logic is the default, bug-compatible mode is explicit |
| Rh-105 defaulted `parent_decay = False` despite having a parent | Reproduced under [`ParentDecayPolicy::UpstreamTableDefault`] only |
| `nuclide_import` mutates the shared module table | Impossible here: values are owned |
| `nuclide_import_accident` unguarded table lookup (`KeyError`) | Returns [`SelectionError::UnknownNuclide`] |
| `nuclide_sort` compares a `list` to `str` (dead reordering) | [`sort_parents_before_daughters`] does what upstream intended |

```rust
pub mod run_selection { /* ... */ }
```

### Types

#### Enum `SelectionError`

Why a nuclide could not be taken into a run.

```rust
pub enum SelectionError {
    Unparseable {
        supplied: String,
    },
    UnknownNuclide {
        normalised: String,
    },
}
```

##### Variants

###### `Unparseable`

The name did not parse as `Element[-]MassNumber[metastable]`, e.g. `"Cs137"`,
`"cs-137"` and `"Cs-137m"` all parse; `"plutonium"` and `"137"` do not.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `supplied` | `String` | The name as supplied by the caller. |

###### `UnknownNuclide`

The name parsed but is absent from the TRISO-ATOPS nuclide table.

Upstream's `nuclide_import` logs a warning and skips; its
`nuclide_import_accident` omits that guard and raises `KeyError`. This
port returns the same error from both and lets the caller decide (see
[`select_nuclides`], which skips, versus
[`select_nuclides_accident`], which also skips — deliberately unlike
upstream).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `normalised` | `String` | The normalised name that was looked up, e.g. `"Cs-137"`. |

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
    fn clone(self: &Self) -> SelectionError { /* ... */ }
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
    fn eq(self: &Self, other: &SelectionError) -> bool { /* ... */ }
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
#### Enum `ParentDecayPolicy`

How to decide whether a daughter's parent-decay coupling is switched on.

# Why this is an enum and not a `bool`

Upstream's `nuclide_import` contains this (lines 275 and 277):

```python
if nuclide_out[parent].sl == True:
    nuclide_out[nuclide].parent_decay == True      # `==`, not `=`
else:
    nuclide_out[nuclide].parent_decay == False     # `==`, not `=`
```

Both statements are comparisons whose results are discarded, so in the
branch that is supposed to *decide* the flag, nothing is written and the
value hard-coded in the nuclide table survives. The consequence is not that
parent decay is disabled — it is that **the short-lived-parent test the
code was written to perform never runs**.

The two behaviours are genuinely different physics, so the port exposes
both rather than picking silently.

```rust
pub enum ParentDecayPolicy {
    ShortLivedParentOnly,
    UpstreamTableDefault,
}
```

##### Variants

###### `ShortLivedParentOnly`

Apply the test upstream's source plainly intends: couple a daughter to
its parent only when the parent is present in the run **and** is
classified short-lived.

This is the default, because the workspace requires correct physics to
be the default rather than an opt-in, and because it is what upstream's
own control flow says it wants. It is **not** what a stock TRISO-ATOPS
run does.

###### `UpstreamTableDefault`

Reproduce the stock TRISO-ATOPS behaviour bug-for-bug: take the flag
from the nuclide table and ignore the parent's half-life entirely,
except that a parent absent from the run still forces it off (upstream
lines 280 and 283 use `=` correctly).

Use this for code-to-code comparison against upstream, or to reproduce
a published TRISO-ATOPS result. Note it also carries upstream's Rh-105
inconsistency — see [`upstream_table_parent_decay`].

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
    fn clone(self: &Self) -> ParentDecayPolicy { /* ... */ }
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
    fn default() -> ParentDecayPolicy { /* ... */ }
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
    fn eq(self: &Self, other: &ParentDecayPolicy) -> bool { /* ... */ }
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
#### Struct `SelectedNuclide`

A nuclide admitted to a run, with the run-dependent classification attached.

Upstream stores `sl` and `parent_decay` by mutating the shared module-level
`nuclides` dictionary, so classification from one run leaks into the next
within a process. Owning the values here makes that class of bug
impossible.

```rust
pub struct SelectedNuclide {
    pub nuclide: super::nuclide_model::TrisoAtopsNuclide,
    pub short_lived: bool,
    pub parent_decay: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nuclide` | `super::nuclide_model::TrisoAtopsNuclide` | The database record (name, `Z`, `A`, half-life, parents). |
| `short_lived` | `bool` | `true` when the half-life is short compared to the run's own timescale.<br><br>Upstream: `hl / irad_time < short_lived_ratio`, default ratio `0.2`.<br>Short-lived species reach equilibrium within the irradiation and take<br>the undivided birth rate in [`release_rate`](super::activities::release_rate);<br>long-lived ones are divided by `1 - exp(-lambda t)`. |
| `parent_decay` | `bool` | `true` when this nuclide's activity is fed by its parent's decay.<br><br>Governed by [`ParentDecayPolicy`]; see that type for why it is not a<br>straightforward read of upstream. |

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
    fn clone(self: &Self) -> SelectedNuclide { /* ... */ }
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
    fn eq(self: &Self, other: &SelectedNuclide) -> bool { /* ... */ }
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

#### Function `upstream_table_parent_decay`

**Attributes:**

- `MustUse { reason: None }`

The nuclides upstream's table defaults to `parent_decay = True`.

Thirteen of the fourteen table rows that carry a parent are defaulted
`True`. The fourteenth, **Rh-105**, is defaulted `False` despite carrying
`['Ru-105']` and the same `# og with parent decay` comment as the other
thirteen — which, because the runtime assignment is defeated by the `==`
bug, means a stock TRISO-ATOPS run never applies parent decay to Rh-105.

That looks like an oversight rather than a decision, but it is upstream's,
so [`ParentDecayPolicy::UpstreamTableDefault`] reproduces it exactly rather
than quietly repairing it.

# Returns
`true` if the stock table would default this nuclide's `parent_decay` flag
on; `false` otherwise (including for every nuclide with no parent).

```rust
pub fn upstream_table_parent_decay(name: &str) -> bool { /* ... */ }
```

#### Function `normalise_nuclide_name`

Normalise a nuclide name to the database's canonical spelling.

Ports the regex `([a-z]{1,2})(?:[-]?)([0-9]+)([a-z]?)` plus the
`f"{element.capitalize()}-{main_number}{suffix}"` reassembly that upstream
applies in both `nuclide_import` and `nuclide_import_accident`. Hand-rolled
rather than pulling in `regex`, which the crate does not otherwise need.

Accepts one or two element letters, an optional hyphen, the mass number,
and an optional single metastable letter. Input case is irrelevant.

# Arguments
- `supplied` — the caller's spelling, e.g. `"cs137"`, `"CS-137"`, `"Cs-137m"`.

# Returns
The canonical name (`"Cs-137"`, `"Cs-137m"`), or
[`SelectionError::Unparseable`] if the pattern does not match.

# Note on strictness
Like upstream's regex this is anchored only at the start, so trailing junk
after the optional metastable letter is rejected here but silently ignored
by Python's `re.match`. That is a deliberate tightening: a name upstream
would have silently truncated is far more likely a typo than an intent.

```rust
pub fn normalise_nuclide_name(supplied: &str) -> Result<String, SelectionError> { /* ... */ }
```

#### Function `select_nuclides`

**Attributes:**

- `MustUse { reason: None }`

Select and classify the nuclides for a **normal-operation** run.

Ports `nuclide_import`. For each supplied name: normalise it, look it up,
classify short-lived against the irradiation time, then wire parent-decay
coupling per `policy`.

# Arguments
- `supplied_names` — the run's nuclide list, in any spelling
  [`normalise_nuclide_name`] accepts.
- `irradiation_time` — the reactor irradiation time the short-lived test is
  measured against (SI seconds; must be `> 0`).
- `short_lived_ratio` — the threshold on `t½ / t_irrad` below which a
  nuclide counts as short-lived. Upstream's default is `0.2`; pass
  `None` for it.
- `policy` — see [`ParentDecayPolicy`].

# Returns
`(selected, skipped)` — the admitted nuclides in input order, and one
[`SelectionError`] per name that could not be taken. Upstream logs those as
warnings and continues; this returns them so a caller can decide, which is
the only behavioural difference.

# Panics
Panics if `irradiation_time` is not strictly positive — upstream would
divide by zero and classify everything as not-short-lived.

```rust
pub fn select_nuclides(supplied_names: &[&str], irradiation_time: uom::si::f64::Time, short_lived_ratio: Option<f64>, policy: ParentDecayPolicy) -> (Vec<SelectedNuclide>, Vec<SelectionError>) { /* ... */ }
```

#### Function `select_nuclides_accident`

**Attributes:**

- `MustUse { reason: None }`

Select the nuclides relevant to an **accident** window.

Ports `nuclide_import_accident`. Keeps only nuclides whose half-life is
long enough to matter over the accident: `t½ / t_accident >= use_ratio`.
Note the inequality runs the opposite way to [`select_nuclides`] — here a
*short* half-life is what disqualifies a nuclide, because anything that has
already decayed away cannot be released.

# Arguments
- `supplied_names` — the run's nuclide list.
- `accident_time` — duration of the accident transient (SI seconds, `> 0`).
- `use_ratio` — threshold on `t½ / t_accident`; upstream's default is
  `0.04`. Pass `None` for it.

# Returns
`(selected, skipped)`. No classification is attached: upstream does not set
`sl` or `parent_decay` on this path, and the accident driver does not read
them.

# Divergence from upstream
Upstream indexes the table **without** the `in nuclides` guard its sibling
has, so a name that satisfies the regex but is absent raises `KeyError` and
aborts the run. This port skips it and reports
[`SelectionError::UnknownNuclide`], matching `nuclide_import`'s
warn-and-continue. Upstream's `else` branch also logs `{match}`, which is
`None` whenever that branch is reached.

# Panics
Panics if `accident_time` is not strictly positive.

```rust
pub fn select_nuclides_accident(supplied_names: &[&str], accident_time: uom::si::f64::Time, use_ratio: Option<f64>) -> (Vec<super::nuclide_model::TrisoAtopsNuclide>, Vec<SelectionError>) { /* ... */ }
```

#### Function `sort_parents_before_daughters`

**Attributes:**

- `MustUse { reason: None }`

Reorder a nuclide list so every parent precedes its daughters.

Ports `run_functions.py::nuclide_sort`, whose stated purpose is to let the
driver accumulate parent activities before the daughters that consume them.

# Divergence: upstream's version never reorders anything

`nuclide_sort` reads

```python
par = calc.nuclides[n].parents          # a LIST, e.g. ['Kr-89']
if par is not None and par in list(nuke_list[:, 0]):
```

which tests whether the *list* `['Kr-89']` is an element of a list of
*strings*. That is never true, so the reordering branch is dead and the
function returns the input order unchanged. This port does what the
function says it does; a caller wanting the upstream no-op can simply not
call it.

# Arguments
- `names` — nuclide names, already normalised.

# Returns
The same names, with each parent that is present moved ahead of its first
daughter. Names absent from the database keep their relative order.
Stable: nuclides with no parent relationship are not moved relative to one
another.

```rust
pub fn sort_parents_before_daughters(names: &[String]) -> Vec<String> { /* ... */ }
```

## Module `diffusion`

# Diffusion coefficients — Arrhenius correlations `D(T)`

Fission-product diffusion coefficients `D` [m^2/s] for the fuel **kernel**,
the matrix **graphite**, and (for silver) the **SiC** layer, as functions of
temperature. Ports `diffusion_coefficient`, `diffusion_coefficient_SiC_Ag`,
and the time-integration helper `integrate` from `calculation_functions.py`.

## Model

Each coefficient is an Arrhenius law

```text
    D(T) = D0 · exp( −Q / (R · T) )
```

with pre-exponential `D0` in m^2/s, activation energy `Q` in J/mol, the molar
gas constant `R = 8.31447 J/(mol·K)`, and absolute temperature `T` in K. Some
species sum two Arrhenius terms (a low- and a high-temperature branch). The
correlations are the NP-MHTGR values (User Manual §1, ref. 1) and are
**valid roughly 700–2400 °C**; the User Manual (§5, "Results look
incorrect") notes that temperatures below a species' valid range are
**clamped** to the boundary value — this port reproduces that clamping
exactly.

## Units

Temperatures are [`ThermodynamicTemperature`]; internally each correlation
reads the temperature both as °C (to apply the valid-range clamp thresholds,
which the upstream code expresses in °C) and as K (for the Arrhenius
exponent). Coefficients are [`DiffusionCoefficient`] in m^2/s. The
time-integrated coefficient `∫D dt` has units of m^2 and is returned as an
[`Area`].

```rust
pub mod diffusion { /* ... */ }
```

### Types

#### Struct `KernelGraphiteDiffusion`

Kernel and matrix-graphite diffusion coefficients returned together.

Both are [`DiffusionCoefficient`]s in m^2/s. Ports the `(D, D_graph)` tuple
returned by the Python `diffusion_coefficient`.

```rust
pub struct KernelGraphiteDiffusion {
    pub kernel: uom::si::f64::DiffusionCoefficient,
    pub graphite: uom::si::f64::DiffusionCoefficient,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `kernel` | `uom::si::f64::DiffusionCoefficient` | Diffusion coefficient in the fuel kernel, m^2/s. |
| `graphite` | `uom::si::f64::DiffusionCoefficient` | Diffusion coefficient in the matrix graphite, m^2/s. |

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
    fn clone(self: &Self) -> KernelGraphiteDiffusion { /* ... */ }
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
    fn eq(self: &Self, other: &KernelGraphiteDiffusion) -> bool { /* ... */ }
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
#### Enum `DiffusionMaterial`

Which material's diffusion coefficient to integrate over time.

Ports the `diffusion_type ∈ {'kernel', 'graphite'}` argument of the Python
`integrate`. For silver in the kernel, the SiC-limited coefficient is used
(matching the upstream `if z != 47` branch).

```rust
pub enum DiffusionMaterial {
    Kernel,
    Graphite,
}
```

##### Variants

###### `Kernel`

Fuel kernel (for Ag, the SiC-limited coefficient).

###### `Graphite`

Matrix graphite.

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
    fn clone(self: &Self) -> DiffusionMaterial { /* ... */ }
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
    fn eq(self: &Self, other: &DiffusionMaterial) -> bool { /* ... */ }
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

#### Function `diffusion_coefficient`

**Attributes:**

- `MustUse { reason: None }`

Kernel and graphite diffusion coefficients for a nuclide, by atomic number.

Ports `diffusion_coefficient(z, T, T_graph)`. The species is selected by its
atomic number `z`; the correlation family and any valid-range clamp are the
NP-MHTGR values (see module docs). Returns [`KernelGraphiteDiffusion`].

**Derivation:** step 3 (crate-root `TRISO_ATOPS_DERIVATION.md` §3) — the
Arrhenius temperature law `D(T) = D0·exp(−Q/RT)` (some species summing a low-
and a high-temperature branch). This `D` becomes the reduced coefficient
`D' = D/a²` inside every release model.

# Element families (by `z`)
- Kr, Te, I, Xe, Se (`z ∈ {36, 52, 53, 54, 34}`): a low-T branch below
  1500 °C and a two-term high-T branch above; graphite `D` equals kernel `D`.
- Rb, Cs (`z ∈ {37, 55}`): kernel `T` clamped to ≥ 700 °C; graphite `T`
  clamped to ≥ 550 °C.
- Sr, Ba, Eu (`z ∈ {38, 56, 63}`): kernel `T` clamped to ≥ 700 °C; graphite
  `T` clamped to ≥ 800 °C.
- Ag, Pd (`z ∈ {47, 46}`): kernel un-clamped; graphite `T` clamped to
  ≥ 490 °C. (For the SiC barrier that actually limits Ag release, use
  [`diffusion_coefficient_sic_ag`].)
- anything else: a fixed nominal `D = D_graph = 1e-19 m^2/s`.

# Arguments
- `z` — atomic number of the nuclide.
- `kernel_temperature` — fuel-kernel temperature (valid ~700–2400 °C).
- `graphite_temperature` — matrix-graphite temperature.

# Assumptions
Inputs outside the ~700–2400 °C validity window are clamped (never
extrapolated) exactly as upstream; results there are boundary values.

```rust
pub fn diffusion_coefficient(z: u32, kernel_temperature: uom::si::f64::ThermodynamicTemperature, graphite_temperature: uom::si::f64::ThermodynamicTemperature) -> KernelGraphiteDiffusion { /* ... */ }
```

#### Function `diffusion_coefficient_sic_ag`

**Attributes:**

- `MustUse { reason: None }`

Diffusion coefficient for silver (Ag) through the **SiC** layer.

Ports `diffusion_coefficient_SiC_Ag(T)`:
`D = 3.6e-9 · exp(−215 kJ/mol / (R·T))`, valid ~700–2400 °C. The SiC layer is
the rate-limiting barrier for Ag transport in an intact TRISO particle, so
this — not the kernel coefficient — governs Ag release.

# Arguments
- `sic_temperature` — temperature of the SiC layer (valid ~700–2400 °C).

# Returns
The Ag-in-SiC [`DiffusionCoefficient`] in m^2/s.

```rust
pub fn diffusion_coefficient_sic_ag(sic_temperature: uom::si::f64::ThermodynamicTemperature) -> uom::si::f64::DiffusionCoefficient { /* ... */ }
```

#### Function `integrate_diffusion_over_time`

**Attributes:**

- `MustUse { reason: None }`

Cumulative time-integral `∫₀ᵗ D(T(t')) dt'` along a temperature history.

Ports the trapezoidal cumulative integration in the Python `integrate`
(accident path): the diffusion coefficient is re-evaluated at each sample's
temperature, then integrated in time with the trapezoid rule and accumulated
(`np.cumsum`). The result feeds the **transient** release models
([`crate::triso_atops_fork::release_models::transient::booth_transient`] and friends),
which are written in terms of `∫D dt` rather than a single `D·t`.

The integral starts at 0 at the first sample (the upstream code prepends a
zero baseline, so the first returned value is 0).

# Arguments
- `z` — atomic number, selecting the diffusion correlation.
- `times` — monotonically increasing sample times (each a [`Time`]); same
  length as `temperatures`.
- `temperatures` — temperature at each sample time.
- `material` — [`DiffusionMaterial::Kernel`] or [`DiffusionMaterial::Graphite`].

# Returns
A `Vec<Area>` of the same length as the inputs, element `i` being
`∫₀^{times[i]} D dt` in m^2.

# Panics
Panics if `times` and `temperatures` differ in length.

```rust
pub fn integrate_diffusion_over_time(z: u32, times: &[uom::si::f64::Time], temperatures: &[uom::si::f64::ThermodynamicTemperature], material: DiffusionMaterial) -> Vec<uom::si::f64::Area> { /* ... */ }
```

### Constants and Statics

#### Constant `GAS_CONSTANT_J_PER_MOL_K`

Molar gas constant `R` used by the TRISO-ATOPS correlations, in J/(mol·K).

The upstream code hard-codes `8.31447`; reproduced verbatim so the exponent
matches bit-for-bit.

```rust
pub const GAS_CONSTANT_J_PER_MOL_K: f64 = 8.31447;
```

## Module `release_models`

# Release models — release-to-birth / release-fraction physics

The dimensionless heart of TRISO-ATOPS: given a species, its diffusion
coefficient, and the reactor state, what fraction escapes the fuel? This
module holds the individual analytical models plus the two **group
dispatchers** that pick the right model for a nuclide:

- [`steady_state`] — normal-operation (constant-temperature) models.
- [`transient`] — accident (time-integrated `∫D dt`) models.
- [`rb_fail`] — the normal-operation dispatcher (ports `R_B_fail`).
- [`release_fraction_transient`] — the accident dispatcher (ports
  `release_fraction`).

The dispatch is by [`ElementGroup`] with an exhaustive `match` (no trait
objects), so a new group is a compile error until every dispatcher handles
it. See User Manual §3.1–3.3.

```rust
pub mod release_models { /* ... */ }
```

### Modules

## Module `steady_state`

# Steady-state (normal-operation) release models

Closed-form release-to-birth / release-fraction models for a reactor at
**steady, constant temperature** (normal operation). Each is a solution (or
empirical fit) to Fickian diffusion out of the fuel, expressed as a single
`<R/B>` or release fraction. Ported from `calculation_functions.py`.

| Function | Upstream | Applies to |
|---|---|---|
| [`rb_fail_noble_gases`] | `RB_fail_Noble_Gases` | Kr, Xe, halogens (empirical `<R/B>` fit) |
| [`breakthrough_model`] | `breakthrough_model` | low-release / barrier-limited species (e.g. Ag through SiC) |
| [`booth_longlived`] | `booth_longlived` | long-lived metals, large release |
| [`booth_shortlived_fast_diffuse`] | `booth_shortlived_fastdiffuse` | short-lived metals, decay-limited |
| [`attenuation_factor`] | `attenuation_factor` | graphite hold-up factor (≥ 1, not a fraction) |

All release fractions are clamped to `[0, 1]` exactly where the upstream code
clamps them. The number of terms kept in each infinite series matches the
upstream defaults ([`BREAKTHROUGH_SERIES_TERMS`], [`BOOTH_SERIES_TERMS`],
[`ATTENUATION_SERIES_TERMS`]).

```rust
pub mod steady_state { /* ... */ }
```

### Functions

#### Function `rb_fail_noble_gases`

**Attributes:**

- `MustUse { reason: None }`

Empirical release-to-birth `<R/B>_fail` for noble gases and halogens.

Ports `RB_fail_Noble_Gases(z, lam, T)`:
`<R/B> = exp( n·ln(1/λ) + B/T_K + C )`, with `(n, B, C)` an empirical fit that
differs for krypton versus xenon/halogens. This is the release from a
**failed/exposed** particle for the volatile species that do not follow the
Booth metal model.

**Derivation:** step 5c (crate-root `TRISO_ATOPS_DERIVATION.md` §5c) — an
*empirical* NP-MHTGR (Anderson et al. 1989) fit, **not** a closed-form
diffusion solution; it captures the observed `λ`- and `T`-dependence of
volatile release from exposed fuel. Ports Python `RB_fail_Noble_Gases`.

# Arguments
- `z` — atomic number. `z == 36` (Kr) selects the krypton fit; any other
  value routed here (Xe `z == 54`, or a halogen) selects the xenon/halogen
  fit. Intended only for noble gases and halogens (see
  [`crate::triso_atops_fork::nuclide_model::ElementGroup`]).
- `decay_constant` — the nuclide decay constant `λ` (s^-1); the fit uses its
  numeric value in s^-1 inside `ln(1/λ)`.
- `temperature` — local temperature; the fit uses absolute temperature in K.

# Returns
The dimensionless `<R/B>_fail` as a [`ReleaseFraction`].

```rust
pub fn rb_fail_noble_gases(z: u32, decay_constant: crate::triso_atops_fork::DecayConstant, temperature: uom::si::f64::ThermodynamicTemperature) -> crate::triso_atops_fork::ReleaseFraction { /* ... */ }
```

#### Function `breakthrough_model`

**Attributes:**

- `MustUse { reason: None }`

Booth "breakthrough" release fraction for a barrier-limited (low-release) species.

Ports `breakthrough_model(D, t, a, r)`. This is the classic low-release
expansion for diffusion through a shell of thickness `a` around a kernel of
radius `r`:

```text
  RF = 3·D·t/(r·a) − a/(2r) − (6a/r)·Σ_{n≥1} (−1)ⁿ/(nπ)² · exp(−(nπ)²·D'·t)
```

with `D' = D/a²`. Used for silver diffusing through the SiC layer (the
limiting barrier). The result is clamped to `[0, 1]`.

**Derivation:** step 5b (crate-root `TRISO_ATOPS_DERIVATION.md` §5b) — the
Daynes–Barrer membrane time-lag solution for cumulative permeation through a
plane barrier of thickness `a` (Crank 1975 §4), multiplied by the spherical
kernel surface-to-volume ratio `3/r` to express it as a kernel release
fraction. The `3·D·t/(r·a)` term is steady permeation, `−a/(2r)` is the time
lag, and the series is the decaying transient. Ports Python `breakthrough_model`.

# Arguments
- `diffusion_coefficient` — `D` in the barrier layer, m^2/s.
- `time` — elapsed (irradiation) time `t`, seconds.
- `layer_thickness` — barrier thickness `a`, metres.
- `kernel_radius` — kernel radius `r`, metres.

# Returns
Release fraction in `[0, 1]` as a [`ReleaseFraction`].

```rust
pub fn breakthrough_model(diffusion_coefficient: uom::si::f64::DiffusionCoefficient, time: uom::si::f64::Time, layer_thickness: uom::si::f64::Length, kernel_radius: uom::si::f64::Length) -> crate::triso_atops_fork::ReleaseFraction { /* ... */ }
```

#### Function `booth_longlived`

**Attributes:**

- `MustUse { reason: None }`

Booth model release fraction for a **long-lived** species with large release.

Ports `booth_longlived(D, t, a)`. The Booth equivalent-sphere fractional
release from diffusion out of a sphere of radius `a`:

```text
  RF = 1 − (6/π²)·Σ_{i≥1} (1/i²)·exp(−(iπ)²·D'·t) ,   D' = D/a²
```

As `D'·t → ∞`, `RF → 1`. For small `D'·t` it recovers the early-time law
`RF ≈ 6·√(D'·t/π) − 3·D'·t`. Used for long-lived special-metal fission
products (Sr, Cs, Ba, Eu).

**Derivation:** step 4 (crate-root `TRISO_ATOPS_DERIVATION.md` §4) — the
separation-of-variables solution of Fickian diffusion out of a sphere with a
perfect-sink surface (Crank 1975 §6). Ports Python `booth_longlived`.

# Arguments
- `diffusion_coefficient` — `D`, m^2/s.
- `time` — elapsed time `t`, seconds.
- `equivalent_sphere_radius` — Booth equivalent-sphere radius `a`, metres.

# Returns
Release fraction in `[0, 1]` as a [`ReleaseFraction`] (the series is
mathematically already in range; not explicitly clamped, matching upstream).

```rust
pub fn booth_longlived(diffusion_coefficient: uom::si::f64::DiffusionCoefficient, time: uom::si::f64::Time, equivalent_sphere_radius: uom::si::f64::Length) -> crate::triso_atops_fork::ReleaseFraction { /* ... */ }
```

#### Function `booth_shortlived_fast_diffuse`

**Attributes:**

- `MustUse { reason: None }`

Booth model `<R/B>` for a **short-lived** species that diffuses fast relative to decay.

Ports `booth_shortlived_fastdiffuse(D, lam, a)`. The steady-state
release-to-birth ratio for a decaying species diffusing out of an
equivalent sphere:

```text
  x = √(λ·a²/D) ,   <R/B> = (3/x)·( coth(x) − 1/x )
```

Limits: as `x → 0` (fast diffusion / long-lived), `<R/B> → 1`; as `x → ∞`
(slow diffusion / short-lived), `<R/B> → 3/x`.

**Derivation:** step 5a (crate-root `TRISO_ATOPS_DERIVATION.md` §5a) — the
*steady-state* solution of the sphere diffusion equation with a decay sink
`−λC` (secular equilibrium), integrating the surface flux against the birth
rate (Booth 1957; NP-MHTGR, Anderson et al. 1989). Here `x = μ = √(λa²/D)`.
Ports Python `booth_shortlived_fastdiffuse`.

# Arguments
- `diffusion_coefficient` — `D`, m^2/s.
- `decay_constant` — `λ`, s^-1.
- `equivalent_sphere_radius` — Booth equivalent-sphere radius `a`, metres.

# Returns
The dimensionless `<R/B>` as a [`ReleaseFraction`].

```rust
pub fn booth_shortlived_fast_diffuse(diffusion_coefficient: uom::si::f64::DiffusionCoefficient, decay_constant: crate::triso_atops_fork::DecayConstant, equivalent_sphere_radius: uom::si::f64::Length) -> crate::triso_atops_fork::ReleaseFraction { /* ... */ }
```

#### Function `attenuation_factor`

**Attributes:**

- `MustUse { reason: None }`

Graphite hold-up (attenuation) factor `Af` for a fission metal.

Ports `attenuation_factor(D_graph, t, a)`. This is **not** a release fraction
— it is a dimensionless factor `Af ≥ 1` describing how much the matrix
graphite attenuates (delays) release of a metal, via

```text
  S = Σ_{i odd} (4/(iπ))·sin(iπ/2)·exp(−(iπ)²·D_graph·t/(4a²)) ,   Af = 1/(1 − S)
```

The upstream code caps `Af` at [`ATTENUATION_FACTOR_CAP`] (1e8) and returns
the cap if `S == 1`, `Af > 1e8`, or `Af < 0`; reproduced here.

**Derivation:** step 5d (crate-root `TRISO_ATOPS_DERIVATION.md` §5d) —
transient slab diffusion through the matrix graphite. At `t = 0` the series is
the Leibniz sum `S = 1` so `Af → ∞` (total hold-up, hence the 1e8 cap); as
`t → ∞`, `S → 0` and `Af → 1` (graphite saturated). The coolant source rate is
`S_coolant = R/Af`. Ports Python `attenuation_factor`.

# Arguments
- `graphite_diffusion_coefficient` — `D_graph`, m^2/s.
- `time` — elapsed time `t`, seconds.
- `graphite_thickness` — graphite region thickness `a`, metres.

# Returns
The dimensionless attenuation factor as a [`Ratio`] (≥ 1; **not** clamped to
`[0, 1]` — it is a hold-up factor, not a fraction).

```rust
pub fn attenuation_factor(graphite_diffusion_coefficient: uom::si::f64::DiffusionCoefficient, time: uom::si::f64::Time, graphite_thickness: uom::si::f64::Length) -> uom::si::f64::Ratio { /* ... */ }
```

### Constants and Statics

#### Constant `BREAKTHROUGH_SERIES_TERMS`

Default number of terms in the [`breakthrough_model`] series (upstream `num_terms=1000`).

```rust
pub const BREAKTHROUGH_SERIES_TERMS: usize = 1000;
```

#### Constant `BOOTH_SERIES_TERMS`

Default number of terms in the [`booth_longlived`] series (upstream `num_terms=5000`).

```rust
pub const BOOTH_SERIES_TERMS: usize = 5000;
```

#### Constant `ATTENUATION_SERIES_TERMS`

Default number of terms in the [`attenuation_factor`] series (upstream `num_terms=500`).

```rust
pub const ATTENUATION_SERIES_TERMS: usize = 500;
```

#### Constant `ATTENUATION_FACTOR_CAP`

Upper cap the upstream places on the graphite attenuation factor.

```rust
pub const ATTENUATION_FACTOR_CAP: f64 = 1e8;
```

## Module `transient`

# Transient (accident) release models

Accident-case counterparts of the [steady-state](super::steady_state) models.
During a transient the temperature — and therefore the diffusion coefficient
— changes with time, so these models are written in terms of the
**time-integrated** diffusion coefficient rather than a single `D·t`:

- `∫D' dt` (dimensionless) where `D' = D/a²`, and
- `∫D dt` (units of m^2, a [`uom::si::f64::Area`]).

Both integrals are produced by
[`crate::triso_atops_fork::diffusion::integrate_diffusion_over_time`]. Ported
from `calculation_functions.py`; see User Manual §3.3 ("Accident").

| Function | Upstream |
|---|---|
| [`breakthrough_model_transient`] | `breakthrough_model_transient` |
| [`booth_transient`] | `booth_transient` |
| [`rf_graph`] | `RF_Graph` |

```rust
pub mod transient { /* ... */ }
```

### Functions

#### Function `breakthrough_model_transient`

**Attributes:**

- `MustUse { reason: None }`

Transient breakthrough release fraction from the kernel/barrier.

Ports `breakthrough_model_transient(int_Dp, int_Dt, a, r)`. The transient
analogue of [`super::steady_state::breakthrough_model`], with `D·t` replaced
by the time-integral `∫D dt` and `D'·t` by `∫D' dt`:

```text
  RF = 3·(∫D dt)/(a·r) − a/(2r) − (6a/r)·Σ_{n≥1} (−1)ⁿ/(nπ)² · exp(−(nπ)²·∫D' dt)
```

Clamped to `[0, 1]`; returns exactly 0 if either integral is 0 (upstream
guard). Used for the silver/SiC accident release.

**Derivation:** step 7 (crate-root `TRISO_ATOPS_DERIVATION.md` §7) — the
transient generalisation of the step-5b breakthrough model: for a
time-varying temperature the products `D·t` and `D'·t` are replaced by their
time integrals `∫D dt` and `∫D' dt`. Ports Python `breakthrough_model_transient`.

# Arguments
- `integrated_d_prime` — `∫D' dt` (dimensionless [`Ratio`]), where `D' = D/a²`.
- `integrated_d` — `∫D dt` (an [`Area`], m^2).
- `layer_thickness` — barrier thickness `a`, metres.
- `kernel_radius` — kernel radius `r`, metres.

# Returns
Release fraction in `[0, 1]` as a [`ReleaseFraction`].

```rust
pub fn breakthrough_model_transient(integrated_d_prime: uom::si::f64::Ratio, integrated_d: uom::si::f64::Area, layer_thickness: uom::si::f64::Length, kernel_radius: uom::si::f64::Length) -> crate::triso_atops_fork::ReleaseFraction { /* ... */ }
```

#### Function `booth_transient`

**Attributes:**

- `MustUse { reason: None }`

Transient Booth release fraction from the kernel.

Ports `booth_transient(int_Dp)`. The transient analogue of
[`super::steady_state::booth_longlived`]:

```text
  RF = 1 − 6·Σ_{i≥1} (1/(iπ)²)·exp(−(iπ)²·∫D' dt)
```

Returns 0 if `∫D' dt == 0`, and snaps values below
[`BOOTH_TRANSIENT_ZERO_FLOOR`] (1e-6) to 0 (upstream behaviour). Used for
special-metal / other fission-product accident release from the kernel.

**Derivation:** step 7 (crate-root `TRISO_ATOPS_DERIVATION.md` §7) — the
transient generalisation of the step-4 Booth sphere release, with `D'·t`
replaced by the time integral `∫D' dt`. Ports Python `booth_transient`.

# Arguments
- `integrated_d_prime` — `∫D' dt` (dimensionless [`Ratio`]), where `D' = D/a²`
  and `a` is the kernel radius.

# Returns
Release fraction in `[0, 1]` as a [`ReleaseFraction`].

```rust
pub fn booth_transient(integrated_d_prime: uom::si::f64::Ratio) -> crate::triso_atops_fork::ReleaseFraction { /* ... */ }
```

#### Function `rf_graph`

**Attributes:**

- `MustUse { reason: None }`

Transient release fraction through the matrix **graphite**.

Ports `RF_Graph(val, a)`:

```text
  RF = Σ_{i odd} (8/(iπ)²)·( 1 − exp(−(iπ)²·(∫D dt)/(4a²)) )
```

As `∫D dt → ∞` this saturates to 1 (since `Σ_{i odd} 8/(iπ)² = 1`).

**Derivation:** step 7 (crate-root `TRISO_ATOPS_DERIVATION.md` §7) — the
graphite *release* fraction from transient slab diffusion (the complement of
the step-5d hold-up), written in terms of the time integral `∫D dt`. Ports
Python `RF_Graph`.

# Arguments
- `integrated_d` — `∫D dt` (an [`Area`], m^2) for the graphite.
- `graphite_thickness` — graphite region thickness `a`, metres.

# Returns
Release fraction in `[0, 1]` as a [`ReleaseFraction`].

```rust
pub fn rf_graph(integrated_d: uom::si::f64::Area, graphite_thickness: uom::si::f64::Length) -> crate::triso_atops_fork::ReleaseFraction { /* ... */ }
```

### Constants and Statics

#### Constant `RF_GRAPH_SERIES_TERMS`

Default number of terms in the [`rf_graph`] series (upstream `num_terms=5000`).

```rust
pub const RF_GRAPH_SERIES_TERMS: usize = 5000;
```

#### Constant `BOOTH_TRANSIENT_ZERO_FLOOR`

Below this value a [`booth_transient`] release fraction is snapped to 0 (upstream `1e-6`).

```rust
pub const BOOTH_TRANSIENT_ZERO_FLOOR: f64 = 1e-6;
```

### Types

#### Enum `ReleaseMaterial`

Which region's transient release fraction to compute.

Ports the `material ∈ {'kernel', 'graphite'}` argument of the Python
`release_fraction`.

```rust
pub enum ReleaseMaterial {
    Kernel,
    Graphite,
}
```

##### Variants

###### `Kernel`

Release from the fuel kernel (Booth, or breakthrough for silver).

###### `Graphite`

Release from the matrix graphite.

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
    fn clone(self: &Self) -> ReleaseMaterial { /* ... */ }
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
    fn eq(self: &Self, other: &ReleaseMaterial) -> bool { /* ... */ }
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

#### Function `rb_fail`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`
- `MustUse { reason: None }`

Normal-operation release-to-birth `<R/B>_fail` dispatcher.

Ports `R_B_fail(z, sl, lam, temps, t, a_grain, a_SiC, r, D)`. Selects the
correct steady-state release model for a nuclide from its transport
[`ElementGroup`]. **Derivation:** step 6(i) and step 2 (crate-root
`TRISO_ATOPS_DERIVATION.md`) — the group dispatch, and the Booth
equivalent-sphere radius `a_booth = √(2·a_grain·r)`.

- **Noble gas / halogen** → empirical [`steady_state::rb_fail_noble_gases`].
- **Special metal** → Booth model: [`steady_state::booth_shortlived_fast_diffuse`]
  if short-lived, else [`steady_state::booth_longlived`], both on the Booth
  equivalent-sphere radius `a_booth = √(2·a_grain·r)`.
- **Silver** → [`steady_state::breakthrough_model`] through the SiC layer
  (using the Ag-in-SiC coefficient), scaled by `√(λ_Ag-110m / λ)` exactly as
  upstream.
- **Other** → the fixed nominal [`OTHER_METAL_RB_FAIL`].

# Arguments
- `z` — atomic number (selects the group).
- `short_lived` — the run-dependent short-lived flag `sl` (see
  [`crate::triso_atops_fork::nuclide_model`] note); only affects special metals.
- `decay_constant` — `λ`, s^-1.
- `temperature` — local temperature (used by noble-gas and silver paths).
- `irradiation_time` — elapsed irradiation time `t`, seconds.
- `grain_size` — UCO fuel grain size `a_grain`, metres.
- `sic_thickness` — SiC layer thickness `a_SiC`, metres.
- `kernel_radius` — kernel radius `r`, metres.
- `kernel_diffusion_coefficient` — the species' kernel `D`, m^2/s (used by the
  special-metal Booth models).

# Returns
The dimensionless `<R/B>_fail` as a [`ReleaseFraction`].

```rust
pub fn rb_fail(z: u32, short_lived: bool, decay_constant: crate::triso_atops_fork::DecayConstant, temperature: uom::si::f64::ThermodynamicTemperature, irradiation_time: uom::si::f64::Time, grain_size: uom::si::f64::Length, sic_thickness: uom::si::f64::Length, kernel_radius: uom::si::f64::Length, kernel_diffusion_coefficient: uom::si::f64::DiffusionCoefficient) -> crate::triso_atops_fork::ReleaseFraction { /* ... */ }
```

#### Function `release_fraction_transient`

**Attributes:**

- `MustUse { reason: None }`

Accident (transient) release-fraction dispatcher.

Ports `release_fraction(z, fractions, integrals, a_primary, a_secondary,
material)`. Given the time-integrated diffusion coefficient `∫D dt`, selects
the transient model:

- **Kernel, non-silver** → [`transient::booth_transient`] on `∫D dt / r²`.
- **Kernel, silver (`z == 47`)** → [`transient::breakthrough_model_transient`]
  using the SiC thickness as the barrier (`∫D dt / a_SiC²`).
- **Graphite, noble gas / halogen** → 0 (volatiles are not held up in
  graphite in this model).
- **Graphite, metal** → [`transient::rf_graph`] on `∫D dt` and the graphite
  thickness.

The upstream `fractions` argument only ever multiplies by 1 in this function
(the failure fractions are applied later, in the release-*activity* step, which
lives in the scaffolded [`crate::triso_atops_fork::activities`] module), so it
is intentionally omitted here.

# Arguments
- `z` — atomic number.
- `element_group` — the nuclide's transport group (only the graphite path
  needs it, to zero out noble gases and halogens).
- `integrated_d` — `∫D dt` for this region (an [`Area`], m^2), from
  [`crate::triso_atops_fork::diffusion::integrate_diffusion_over_time`].
- `primary_thickness` — kernel radius `r` (kernel path) or graphite thickness
  `a_graph` (graphite path), metres.
- `secondary_thickness` — SiC thickness `a_SiC`, metres; required for the
  silver kernel path, ignored otherwise.
- `material` — [`ReleaseMaterial::Kernel`] or [`ReleaseMaterial::Graphite`].

# Returns
Release fraction in `[0, 1]` as a [`ReleaseFraction`].

# Panics
Panics if `material == Kernel`, `z == 47` (silver), and `secondary_thickness`
is `None` (the SiC barrier thickness is mandatory for the silver path).

```rust
pub fn release_fraction_transient(z: u32, element_group: crate::triso_atops_fork::nuclide_model::ElementGroup, integrated_d: uom::si::f64::Area, primary_thickness: uom::si::f64::Length, secondary_thickness: Option<uom::si::f64::Length>, material: ReleaseMaterial) -> crate::triso_atops_fork::ReleaseFraction { /* ... */ }
```

### Constants and Statics

#### Constant `OTHER_METAL_RB_FAIL`

Nominal `<R/B>_fail` assigned to "other" fission metals (upstream `1e-5`).

```rust
pub const OTHER_METAL_RB_FAIL: f64 = 1e-5;
```

## Module `activities`

# Activity bookkeeping — coolant / plate-out / clean-up source terms

This module ports the TRISO-ATOPS **coolant activity** functions that turn a
per-node fission-product *release rate* into the three primary-loop activity
pools — circulating (in the coolant), plate-out (deposited on loop surfaces),
and clean-up (removed by the helium-purification system, HPS) — plus the
**source-term** functions ([`source_terms`]) that produce the release rate and
the graphite hold-up activity from a node's radionuclide inventory.

It ports `calculation_functions.py`'s `circulating*`, `plate_out*`,
`clean_up*`, `release_rate`, and `base_activities`. The infinite-series
`attenuation_factor` used by `base_activities` is already ported in
[`crate::triso_atops_fork::release_models::steady_state`].

## Units — the design decision (this is the deferred `uom` pass, op-b4a.2.2)

The upstream 1989 NP-MHTGR bookkeeping mixed **atoms**, **atoms/s**,
**curies**, and **becquerels** through three hard-coded magic factors:
`× 3.7e10` (Ci→Bq), `÷ (1 − e^{−λt})` (birth-rate normalisation), and a
trailing `× λ / 3.7e10` at output (atom-count → Ci). This port makes each of
those explicit and, where the dimension is genuine, `uom`-checked. The chosen
representation:

- **Radioactivity / activity → [`Activity`] (a `uom` [`uom::si::f64::Frequency`], SI unit `Bq`).**
  A becquerel is *one decay per second*, so activity is dimensionally a
  frequency (`s^-1`) — the same dimension this crate already uses for the
  decay constant ([`crate::triso_atops_fork::DecayConstant`]). Representing
  `Bq` as `Frequency` is therefore consistent with the physics core and is
  dimensionally honest: `A = λ N` has units `s^-1 · (dimensionless count) =
  s^-1 = Bq`. There is no separate SI base unit for "amount of a decaying
  species", so no wrong dimension is being invented. See
  [`activity_from_atom_count`].
- **Ci↔Bq** is the single documented constant [`BQ_PER_CI`] (`1 Ci = 3.7e10
  Bq`), replacing every hard-coded `3.7e10` / `/3.7e10` in the source. Use
  [`becquerels_from_curies`] and [`curies_from_becquerels`].
- **Atom counts / inventories → a documented plain `f64` count** (see
  [`atom_count_from_activity`]). A number of atoms is *dimensionless*; forcing
  a `uom` dimension onto it (e.g. `mol`) would be wrong, and — critically —
  "atoms/s" is dimensionally identical to a rate constant (`s^-1`), so `uom`
  cannot tell a release *rate* from a *decay constant*. The effective-unit
  bookkeeping quantities (release rate, source rate, and the three activity
  pools) are therefore carried as plain `f64` with their meaning spelled out
  in each function's docs, while the genuinely-dimensioned inputs — the decay
  constant, the plate-out / clean-up rate constants, and the elapsed time —
  are `uom` [`uom::si::f64::Frequency`] / [`uom::si::f64::Time`]. This gives
  real dimensional checking exactly where it is meaningful: the sum
  `β = λ + k_plate + k_clean` can only add frequencies, and every exponent
  `β·t`, `λ·t` is checked to be dimensionless.

### What "effective units" means for the pool quantities

Following the upstream convention, the release/source **rates** are carried in
becquerels (`atoms/s`) and the three activity **pools** (circulating,
plate-out, clean-up) and the graphite hold-up are carried as atom **counts**
(`Bq·s = atoms`). Both are `f64`. The final report activity in curies is
recovered uniformly by `× λ / 3.7e10` — for a rate this yields `Ci/s`, for a
count it yields `Ci`, exactly as the upstream output columns are labelled.
[`crate::triso_atops_fork::normal_operation`] performs that final conversion.

## Dimensional-consistency check

The relation `activity = decay_constant × atom_count` is verified in this
module's tests (both dimensionally, via `uom`, and numerically), and the
individual bookkeeping functions are verified against values produced by the
upstream Python on the same inputs (data taken 2026-07-15, commit `de374c8`).

```rust
pub mod activities { /* ... */ }
```

### Modules

## Module `coolant_activity`

# Primary-loop activity pools — circulating, plate-out, clean-up

Given a per-node **source rate** `S` (the rate at which a nuclide enters the
coolant, after graphite hold-up — see [`super::source_terms::base_activities`]),
these functions solve the linear activity-balance for the three primary-loop
pools of an HTGR/FHR:

- **Circulating** `C` — activity carried in the flowing coolant.
- **Plate-out** `P` — activity deposited on primary-loop surfaces (rate
  constant `k_plate`).
- **Clean-up** `HPS` — activity removed by the helium-purification system
  (rate constant `k_clean`).

All three share the total removal rate `β = λ + k_plate + k_clean`, where `λ`
is the nuclide decay constant. `uom` enforces that `β` is a sum of
frequencies and that every `β·t` / `λ·t` exponent is dimensionless.

**Derivation:** step 6(iv) (crate-root `TRISO_ATOPS_DERIVATION.md` §6) — the
three linear activity balances driven by the coolant source rate `S`. Each
`*_steadystate` form is the `t → ∞` limit of its time-dependent counterpart.

## Units

- `source_rate`, and the `*_parent` pool inputs / returned pool values are
  **effective-unit `f64`** (see the [`super`] module docs): the source/removal
  *rate* is in becquerels (`atoms/s`), the pool *amounts* are atom counts
  (`atoms`). They are converted to reportable curies once, downstream, by
  `× λ / 3.7e10`.
- `k_plate`, `k_clean` are plate-out / clean-up **rate constants**
  ([`uom::si::f64::Frequency`], `s^-1`).
- `decay_constant` is `λ` ([`DecayConstant`], `s^-1`).
- `time` is the elapsed reactor run time ([`uom::si::f64::Time`], `s`).

```rust
pub mod coolant_activity { /* ... */ }
```

### Functions

#### Function `circulating_steadystate`

**Attributes:**

- `MustUse { reason: None }`

Steady-state circulating activity `C` for a nuclide.

Ports `circulating_steadystate(S, k_plate, lam, k_clean=0, C_parent=0)`.
Solves `0 = S + λ·C_parent − β·C` for `C`, i.e.
`C = (S + λ·C_parent) / β` with `β = λ + k_plate + k_clean`.

# Arguments
- `source_rate` — source rate `S` into the coolant (effective `f64`, Bq).
- `k_plate` — plate-out rate constant ([`Frequency`], `s^-1`).
- `decay_constant` — nuclide `λ` ([`DecayConstant`], `s^-1`).
- `k_clean` — clean-up (HPS) rate constant ([`Frequency`], `s^-1`); pass
  `Frequency::new::<hertz>(0.0)` when there is no HPS.
- `circulating_parent` — parent nuclide's circulating pool `C_parent`
  (effective `f64`); `0.0` if parent decay is not tracked.

# Returns
The circulating pool `C` (effective `f64`, atom count).

```rust
pub fn circulating_steadystate(source_rate: f64, k_plate: uom::si::f64::Frequency, decay_constant: crate::triso_atops_fork::DecayConstant, k_clean: uom::si::f64::Frequency, circulating_parent: f64) -> f64 { /* ... */ }
```

#### Function `circulating`

**Attributes:**

- `MustUse { reason: None }`

Time-dependent circulating activity `C` at reactor run time `t`.

Ports `circulating(S, k_plate, lam, t, k_clean=0, C_parent=0)`:
`C = S·(1 − e^{−β t}) / β + λ·C_parent / β`, the solution of
`dC/dt = S + λ·C_parent − β·C` started from `C(0) = 0` with `S` held constant.
As `t → ∞` this tends to [`circulating_steadystate`].

# Arguments
See [`circulating_steadystate`], plus:
- `time` — elapsed reactor run time `t` ([`Time`], `s`).

# Returns
The circulating pool `C` at time `t` (effective `f64`, atom count).

```rust
pub fn circulating(source_rate: f64, k_plate: uom::si::f64::Frequency, decay_constant: crate::triso_atops_fork::DecayConstant, time: uom::si::f64::Time, k_clean: uom::si::f64::Frequency, circulating_parent: f64) -> f64 { /* ... */ }
```

#### Function `plate_out_steadystate`

**Attributes:**

- `MustUse { reason: None }`

Steady-state plate-out activity `P` for a nuclide.

Ports `plate_out_steadystate(k_plate, S, lam, k_clean=0, P_parent=0)`:
`P = k_plate·S / (λ·β) + P_parent` with `β = λ + k_plate + k_clean`.

# Arguments
- `k_plate` — plate-out rate constant ([`Frequency`], `s^-1`).
- `source_rate` — source rate `S` (effective `f64`, Bq).
- `decay_constant` — nuclide `λ` ([`DecayConstant`], `s^-1`); must be non-zero.
- `k_clean` — clean-up rate constant ([`Frequency`], `s^-1`).
- `plate_out_parent` — parent nuclide's plate-out pool `P_parent` (effective `f64`).

# Returns
The plate-out pool `P` (effective `f64`, atom count).

```rust
pub fn plate_out_steadystate(k_plate: uom::si::f64::Frequency, source_rate: f64, decay_constant: crate::triso_atops_fork::DecayConstant, k_clean: uom::si::f64::Frequency, plate_out_parent: f64) -> f64 { /* ... */ }
```

#### Function `plate_out`

**Attributes:**

- `MustUse { reason: None }`

Time-dependent plate-out activity `P` at reactor run time `t`.

Ports `plate_out(k_plate, S, lam, t, C, k_clean=0, P_parent=0)`:
`P = k_plate/(β−λ)·(S/λ·(1 − e^{−λ t}) − C) + P_parent`.
The `β − λ = k_plate + k_clean` denominator degenerates to zero when there is
neither plate-out nor clean-up; matching upstream, `P` is then `0`.

# Arguments
See [`plate_out_steadystate`], plus:
- `time` — elapsed run time `t` ([`Time`], `s`).
- `circulating` — the coolant pool `C` at time `t` (effective `f64`), e.g.
  from [`circulating`].

# Returns
The plate-out pool `P` at time `t` (effective `f64`, atom count).

```rust
pub fn plate_out(k_plate: uom::si::f64::Frequency, source_rate: f64, decay_constant: crate::triso_atops_fork::DecayConstant, time: uom::si::f64::Time, circulating: f64, k_clean: uom::si::f64::Frequency, plate_out_parent: f64) -> f64 { /* ... */ }
```

#### Function `clean_up_steadystate`

**Attributes:**

- `MustUse { reason: None }`

Steady-state clean-up (HPS) activity `HPS` for a nuclide.

Ports `clean_up_steadystate(k_plate, S, lam, k_clean, HPS_parent=0)`:
`HPS = k_clean·S / (λ·β)` with `β = λ + k_plate + k_clean`.

# Upstream note
The upstream steady-state form **does not** add the `HPS_parent` term (unlike
the circulating and plate-out steady-state forms, which add their parent
pool). This port preserves that behaviour for numerical fidelity — the
parameter is accepted for signature symmetry but ignored. Parent HPS
contributions are captured by the time-dependent [`clean_up`].

# Arguments
- `k_plate` — plate-out rate constant ([`Frequency`], `s^-1`).
- `source_rate` — source rate `S` (effective `f64`, Bq).
- `decay_constant` — nuclide `λ` ([`DecayConstant`], `s^-1`); must be non-zero.
- `k_clean` — clean-up rate constant ([`Frequency`], `s^-1`).
- `clean_up_parent` — parent HPS pool (accepted but **ignored**, per upstream).

# Returns
The clean-up pool `HPS` (effective `f64`, atom count).

```rust
pub fn clean_up_steadystate(k_plate: uom::si::f64::Frequency, source_rate: f64, decay_constant: crate::triso_atops_fork::DecayConstant, k_clean: uom::si::f64::Frequency, clean_up_parent: f64) -> f64 { /* ... */ }
```

#### Function `clean_up`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`
- `MustUse { reason: None }`

Time-dependent clean-up (HPS) activity `HPS` at reactor run time `t`.

Ports `clean_up(k_plate, S, lam, t, C, k_clean, HPS_parent=0)`:
`HPS = k_clean/(β−λ)·(S/λ·(1 − e^{−λ t}) − C) + HPS_parent`.

# Arguments
See [`clean_up_steadystate`], plus:
- `time` — elapsed run time `t` ([`Time`], `s`).
- `circulating` — coolant pool `C` at time `t` (effective `f64`).
- `clean_up_parent` — parent HPS pool `HPS_parent` (effective `f64`); here it
  **is** added, matching upstream.

# Returns
The clean-up pool `HPS` at time `t` (effective `f64`, atom count).

```rust
pub fn clean_up(k_plate: uom::si::f64::Frequency, source_rate: f64, decay_constant: crate::triso_atops_fork::DecayConstant, time: uom::si::f64::Time, circulating: f64, k_clean: uom::si::f64::Frequency, clean_up_parent: f64) -> f64 { /* ... */ }
```

## Module `source_terms`

# Source terms — release rate and graphite hold-up

These two functions sit between the dimensionless release models
([`crate::triso_atops_fork::release_models`]) and the coolant activity pools
([`super::coolant_activity`]):

- [`release_rate`] turns a node's radionuclide **inventory** (an [`Activity`])
  and its `<R/B>_fail` into the **release rate** `R` at which the nuclide
  leaves the fuel into the fuel-element graphite.
- [`base_activities`] splits `R` into the **source rate** `S` that reaches the
  coolant (after graphite attenuation) and the **graphite hold-up** activity
  `G` retained in the matrix.

## Units

The inventory is a `uom` [`Activity`] (Bq). `R` and `S` are release/source
*rates* carried as effective-unit `f64` becquerels (`atoms/s`); `G` is an atom
**count** (`atoms`). See the [`super`] module docs for why the pool
quantities are `f64` rather than `uom`-typed.

```rust
pub mod source_terms { /* ... */ }
```

### Types

#### Struct `FailureFractions`

The four TRISO fuel failure fractions applied to a release-to-birth ratio.

Ports the upstream `fractions = [f_hm, f_sic, f_inc, f_inc_sic]` list (User
Manual §2.4 keys `f_hm`, `f_sic`, `f_inc`, `f_inc_sic`). All four are
dimensionless fractions in `[0, 1]`; how they combine depends on the transport
group (see [`release_rate`]).

```rust
pub struct FailureFractions {
    pub heavy_metal: f64,
    pub sic: f64,
    pub incremental: f64,
    pub incremental_sic: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `heavy_metal` | `f64` | Heavy-metal contamination fraction `f_hm` (uranium outside intact kernels). |
| `sic` | `f64` | As-manufactured defective-SiC fraction `f_sic`. |
| `incremental` | `f64` | Incremental (in-service) particle failure fraction `f_inc`. |
| `incremental_sic` | `f64` | Incremental SiC-only failure fraction `f_inc_sic`. |

##### Implementations

###### Methods

- ```rust
  pub fn with_fuel_failure_incremental(self: Self, progress: crate::fuel_failure::history::FailureProgress) -> Self { /* ... */ }
  ```
  Replace `incremental` with a value computed by **boon-lay fuel failure**

- ```rust
  pub fn sum(self: &Self) -> f64 { /* ... */ }
  ```
  Sum of all four failure fractions, `f_hm + f_sic + f_inc + f_inc_sic`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> FailureFractions { /* ... */ }
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
    fn eq(self: &Self, other: &FailureFractions) -> bool { /* ... */ }
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
#### Struct `SourceAndGraphite`

The [`base_activities`] result: coolant source rate and graphite hold-up.

```rust
pub struct SourceAndGraphite {
    pub source_rate: f64,
    pub graphite_activity: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `source_rate` | `f64` | Source rate `S` that reaches the coolant, after graphite attenuation<br>(effective-unit `f64`, Bq = `atoms/s`). Feeds<br>[`super::coolant_activity::circulating`]. |
| `graphite_activity` | `f64` | Graphite hold-up activity `G` retained in the fuel-element matrix<br>(effective-unit `f64`, atom count). Zero for volatiles. |

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
    fn clone(self: &Self) -> SourceAndGraphite { /* ... */ }
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
    fn eq(self: &Self, other: &SourceAndGraphite) -> bool { /* ... */ }
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

#### Function `release_rate`

**Attributes:**

- `MustUse { reason: None }`

Per-node fission-product **release rate** `R` from the fuel.

Ports `release_rate(RB_fail, z, fractions, inventories, sl, t, lam)`. The
release-to-birth ratio applied to the birth rate depends on the transport
group (ports the upstream `z in noble_gases/halogens` and `z == 47/46`
branches):

- **Noble gas / halogen** → `(f_hm + f_inc) · <R/B>_fail` (only the fully-
  exposed and in-service-failed fuel releases volatiles).
- **Silver / palladium** → `<R/B>_fail` directly (the breakthrough model
  already embeds the SiC-failure population).
- **Special metal / other** → `Σfractions · <R/B>_fail`.

The birth rate is `A` for a short-lived nuclide (secular equilibrium, the
inventory activity *is* the production rate) and `A / (1 − e^{−λ t})` for a
long-lived one, where `A` is the inventory [`Activity`]. **Derivation:** step
6(ii) (crate-root `TRISO_ATOPS_DERIVATION.md` §6): `R = ⟨R/B⟩ × birth rate`,
with `⟨R/B⟩ = failure fraction × ⟨R/B⟩_fail`. This is the explicit
form of the upstream `didt = inventories × 3.7e10 [ / (1 − e^{−λt})]`, with
the `× 3.7e10` now living only in the Ci→Bq conversion of the inventory (see
[`crate::triso_atops_fork::activities::becquerels_from_curies`]).

# Arguments
- `rb_fail` — the release-to-birth-at-failure ratio `<R/B>_fail`
  ([`ReleaseFraction`]), from [`crate::triso_atops_fork::release_models::rb_fail`].
- `element_group` — the nuclide transport [`ElementGroup`].
- `fractions` — the fuel [`FailureFractions`].
- `inventory` — the node's radionuclide inventory as an [`Activity`] (Bq);
  obtain from curies with
  [`crate::triso_atops_fork::activities::becquerels_from_curies`].
- `short_lived` — the run-dependent short-lived flag `sl`
  (`t½ / t_irad < 0.2` upstream).
- `time` — irradiation time `t` ([`Time`], `s`).
- `decay_constant` — nuclide `λ` ([`DecayConstant`], `s^-1`).

# Returns
The release rate `R` (effective-unit `f64`, Bq = `atoms/s`).

```rust
pub fn release_rate(rb_fail: crate::triso_atops_fork::ReleaseFraction, element_group: crate::triso_atops_fork::nuclide_model::ElementGroup, fractions: FailureFractions, inventory: super::Activity, short_lived: bool, time: uom::si::f64::Time, decay_constant: crate::triso_atops_fork::DecayConstant) -> f64 { /* ... */ }
```

#### Function `base_activities`

**Attributes:**

- `MustUse { reason: None }`

Split a release rate into coolant **source rate** `S` and **graphite** `G`.

Ports `base_activities(z, lam, t, a_graph, D_graph, R)`. **Derivation:** step
6(iii) (crate-root `TRISO_ATOPS_DERIVATION.md` §6): splits `R` into the
coolant source rate `S = R/Af` and the graphite hold-up
`G = R·(1 − 1/Af)·(1 − e^{−λt})/λ`, using the step-5d attenuation factor `Af`.

- **Noble gas / halogen** → `S = R`, `G = 0` (volatiles are not retained in
  graphite; they pass straight to the coolant).
- **Special metal / silver** → attenuation factor `Af` from the graphite
  diffusion series ([`attenuation_factor`]); `S = R / Af`,
  `G = R·(1 − 1/Af)·(1 − e^{−λ t}) / λ`.
- **Other** metals → fixed `Af = ` [`OTHER_ATTENUATION_FACTOR`] (`1e8`), same
  `S`/`G` formulas.

# Arguments
- `element_group` — the nuclide transport [`ElementGroup`].
- `decay_constant` — nuclide `λ` ([`DecayConstant`], `s^-1`); must be non-zero.
- `time` — irradiation time `t` ([`Time`], `s`).
- `graphite_thickness` — graphite layer thickness `a_graph` ([`Length`], `m`).
- `graphite_diffusion_coefficient` — graphite diffusion coefficient `D_graph`
  ([`DiffusionCoefficient`], `m^2/s`), from
  [`crate::triso_atops_fork::diffusion::diffusion_coefficient`].
- `release_rate` — the release rate `R` from [`release_rate`] (effective `f64`, Bq).

# Returns
[`SourceAndGraphite`] with the coolant source rate `S` and graphite hold-up `G`.

```rust
pub fn base_activities(element_group: crate::triso_atops_fork::nuclide_model::ElementGroup, decay_constant: crate::triso_atops_fork::DecayConstant, time: uom::si::f64::Time, graphite_thickness: uom::si::f64::Length, graphite_diffusion_coefficient: uom::si::f64::DiffusionCoefficient, release_rate: f64) -> SourceAndGraphite { /* ... */ }
```

### Constants and Statics

#### Constant `OTHER_ATTENUATION_FACTOR`

Fixed graphite attenuation factor assigned to non-metal "other" nuclides.

Ports the upstream `Af = 1e8` fallback in `base_activities` (a nuclide that is
neither a special metal nor silver is assumed to be almost entirely held up in
the graphite, so its source rate `S = R/Af` is negligible).

```rust
pub const OTHER_ATTENUATION_FACTOR: f64 = 1e8;
```

### Types

#### Type Alias `Activity`

Radioactivity (activity) of a decaying species, SI unit becquerel (`Bq`).

A becquerel is **one nuclear decay per second**, so activity is dimensionally
a *frequency* (`s^-1`). This alias is deliberately the same `uom` quantity as
[`crate::triso_atops_fork::DecayConstant`]: the physics relation is
`A = λ N` (activity equals decay constant times atom count), and with `N`
dimensionless this reads `Bq = s^-1 · 1`. Construct with
`Activity::new::<hertz>(bq)` and read with `.get::<hertz>()` (`hertz == s^-1`;
the unit name is only a dimension label — the value is in becquerels).

The curie is not an SI unit. ~~and is not part of `uom`~~ **CORRECTED
2026-09-21** — `uom` 0.38 *does* carry it: the `Radioactivity` quantity has
a built-in `@curie` unit, so `Radioactivity::new::<curie>(1.0)` reads
`3.7e10 Bq` without `3.7e10` being written anywhere. Verified against
`uom-0.38.0/src/si/radioactivity.rs` and pinned by
`changi::activity::units::tests::uom_carries_the_curie_so_the_conversion_never_has_to_be_written_out`.

**That does not make this alias wrong, and it is deliberately not being
changed.** `Activity = Frequency` is code-to-code verified against upstream
TRISO-ATOPS and human-signed-off; `Radioactivity` is a *different Rust type*
of the same dimension, so switching would churn every signature in this
fork for no physics. Convert with [`becquerels_from_curies`] /
[`curies_from_becquerels`] and the [`BQ_PER_CI`] constant as before.

A downstream crate that prefers `Radioactivity` converts at its own
boundary — that crossing is the consumer's to own, in one file, not this
fork's.

```rust
pub type Activity = uom::si::f64::Frequency;
```

### Functions

#### Function `becquerels_from_curies`

**Attributes:**

- `MustUse { reason: None }`

Convert an activity given in **curies** to an [`Activity`] (becquerels).

`A[Bq] = curies × 3.7e10`. TRISO-ATOPS run files specify per-nuclide fuel
inventories in curies (User Manual §2.3.5, "inventories (in curies)"); this is
the boundary conversion into the SI-typed core.

# Arguments
- `curies` — activity in Ci (a dimensionless magnitude; must be ≥ 0 to be
  physical, though the function does not clamp).

```rust
pub fn becquerels_from_curies(curies: f64) -> Activity { /* ... */ }
```

#### Function `curies_from_becquerels`

**Attributes:**

- `MustUse { reason: None }`

Convert an [`Activity`] (becquerels) to curies.

`curies = A[Bq] / 3.7e10`. This is the report-side conversion used when
emitting the TRISO-ATOPS output columns, which are all in `Ci` or `Ci/s`.

# Arguments
- `activity` — an [`Activity`] (Bq).

```rust
pub fn curies_from_becquerels(activity: Activity) -> f64 { /* ... */ }
```

#### Function `activity_from_atom_count`

**Attributes:**

- `MustUse { reason: None }`

Activity of a pool of `count` atoms of a nuclide: `A = λ N`.

This is the fundamental radioactivity relation and the dimensional anchor of
the whole activity layer: an [`Activity`] (`Bq = s^-1`) is a
[`DecayConstant`] (`λ`, `s^-1`) multiplied by a dimensionless atom **count**.
`uom` enforces that the result is a frequency; the count is a plain `f64`
because a number of atoms carries no SI dimension.

# Arguments
- `count` — number of atoms (dimensionless, ≥ 0 physically).
- `decay_constant` — the nuclide decay constant `λ = ln 2 / t½` ([`DecayConstant`], `s^-1`).

# Returns
The activity `A = λ · count` as an [`Activity`] (Bq).

```rust
pub fn activity_from_atom_count(count: f64, decay_constant: super::DecayConstant) -> Activity { /* ... */ }
```

#### Function `atom_count_from_activity`

**Attributes:**

- `MustUse { reason: None }`

Number of atoms whose activity is `activity`: `N = A / λ`.

The inverse of [`activity_from_atom_count`]. Ports the upstream
`inventory / λ × 3.7e10` step (`trisoatops.py::normal_operation` line 126),
which converts a per-node curie inventory into an atom count: pass
`activity = becquerels_from_curies(inventory_ci)`.

# Arguments
- `activity` — the pool activity ([`Activity`], Bq).
- `decay_constant` — `λ` ([`DecayConstant`], `s^-1`); must be non-zero.

# Returns
The atom count `N = A / λ` as a plain `f64` (dimensionless).

```rust
pub fn atom_count_from_activity(activity: Activity, decay_constant: super::DecayConstant) -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `BQ_PER_CI`

Becquerels per curie: `1 Ci = 3.7 × 10^10 Bq` (exact, by definition).

This single constant replaces every hard-coded `3.7e10` (`Ci→Bq`) and
`/ 3.7e10` (`Bq→Ci`) scattered through the upstream `release_rate`,
`normal_operation`, and `accident_case`. The curie is defined as exactly
`3.7 × 10^10` disintegrations per second (originally the activity of 1 g of
Ra-226).

```rust
pub const BQ_PER_CI: f64 = 3.7e10;
```

### Re-exports

#### Re-export `circulating`

```rust
pub use coolant_activity::circulating;
```

#### Re-export `circulating_steadystate`

```rust
pub use coolant_activity::circulating_steadystate;
```

#### Re-export `clean_up`

```rust
pub use coolant_activity::clean_up;
```

#### Re-export `clean_up_steadystate`

```rust
pub use coolant_activity::clean_up_steadystate;
```

#### Re-export `plate_out`

```rust
pub use coolant_activity::plate_out;
```

#### Re-export `plate_out_steadystate`

```rust
pub use coolant_activity::plate_out_steadystate;
```

#### Re-export `base_activities`

```rust
pub use source_terms::base_activities;
```

#### Re-export `release_rate`

```rust
pub use source_terms::release_rate;
```

#### Re-export `FailureFractions`

```rust
pub use source_terms::FailureFractions;
```

#### Re-export `SourceAndGraphite`

```rust
pub use source_terms::SourceAndGraphite;
```

## Module `normal_operation`

# Nodal orchestration — the per-node normal-operation source term

This module composes the whole normal-operation chain for one nuclide at one
reactor node, tying together the calculation core and the activity layer:

1. kernel & graphite diffusion coefficients ([`crate::triso_atops_fork::diffusion`]),
2. the release-to-birth ratio `<R/B>_fail` ([`crate::triso_atops_fork::release_models::rb_fail`]),
3. the release rate `R` ([`crate::triso_atops_fork::activities::release_rate`]),
4. the coolant source rate `S` and graphite hold-up `G`
   ([`crate::triso_atops_fork::activities::base_activities`]),
5. the three primary-loop pools — circulating `C`, plate-out `P`, clean-up
   `HPS` ([`crate::triso_atops_fork::activities::coolant_activity`]) — with the
   upstream group-dependent `k_plate` / `k_clean` routing and HPS toggle.

It ports the body of `trisoatops.py::normal_operation` (the per-nuclide,
per-node loop) and `calculation_functions.py::higher_activities`. The
whole-reactor sweep over many nuclides and nodes is a thin loop over
[`normal_operation_node`]; parent → daughter chaining is threaded through
[`ParentPools`] (compute a parent nuclide first, feed its
[`NodalActivities::parent_pools`] into the daughter call).

## Units

Geometry, rate constants, and times are `uom`-typed ([`PlantConstants`]); the
inventory is an [`Activity`] (Bq). The intermediate pools in [`NodalActivities`]
are effective-unit `f64` (see [`crate::triso_atops_fork::activities`] for why);
[`NodalActivities::to_curies`] performs the single, documented
`× λ / 3.7e10` conversion to the reportable [`NodalActivitiesCurie`] (all in
curies, or curies/second for the two rates).

## NOT PORTED — the JSON run-file driver and the accident case (bead op-b4a.2.3)

The TRISO-ATOPS GUI writes a `.json` run file (User Manual §2.4) that
`run_functions.py` parses (`process_run_file`, `check_run_file`,
`convert_time`, `nuclide_sort`, `inventory_processing`) before
`trisoatops.py::main` drives `normal_operation` / `accident_case`. That
file-I/O + argparse layer, and the transient `accident_case`, are **not yet
ported** — they are bead **op-b4a.2.3**, which depends on this nodal
orchestration. The Rust entry point there will take a typed `RunConfig` (built
directly from [`PlantConstants`] + a nuclide/inventory list) and may add a
thin `serde` reader for existing GUI run files. The physics it will call —
[`normal_operation_node`] — is complete and verified here.

```rust
pub mod normal_operation { /* ... */ }
```

### Types

#### Struct `PlantConstants`

Reactor-level constants shared by every node in a normal-operation run.

Ports the `constants` array unpacked at the top of
`trisoatops.py::normal_operation` (User Manual §2.4 keys). All fields are
`uom`-typed so an accidental unit slip is a compile error.

```rust
pub struct PlantConstants {
    pub k_plate: uom::si::f64::Frequency,
    pub k_clean: uom::si::f64::Frequency,
    pub graphite_thickness: uom::si::f64::Length,
    pub grain_size: uom::si::f64::Length,
    pub sic_thickness: uom::si::f64::Length,
    pub kernel_radius: uom::si::f64::Length,
    pub run_time: uom::si::f64::Time,
    pub irradiation_time: uom::si::f64::Time,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `k_plate` | `uom::si::f64::Frequency` | Plate-out rate constant `k_plate` ([`Frequency`], `s^-1`). |
| `k_clean` | `uom::si::f64::Frequency` | Clean-up (HPS) rate constant `k_clean` ([`Frequency`], `s^-1`). |
| `graphite_thickness` | `uom::si::f64::Length` | Graphite layer thickness `a_graph` ([`Length`], `m`). |
| `grain_size` | `uom::si::f64::Length` | UCO fuel grain size `a_grain` ([`Length`], `m`). |
| `sic_thickness` | `uom::si::f64::Length` | SiC layer thickness `a_SiC` ([`Length`], `m`). |
| `kernel_radius` | `uom::si::f64::Length` | Fuel kernel radius `r` ([`Length`], `m`). |
| `run_time` | `uom::si::f64::Time` | Reactor run time `t` used for the coolant-pool balances ([`Time`], `s`). |
| `irradiation_time` | `uom::si::f64::Time` | Fuel irradiation time `t_irad` used for release/birth and diffusion<br>([`Time`], `s`). |

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
    fn clone(self: &Self) -> PlantConstants { /* ... */ }
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
#### Struct `NodeState`

The local temperature state of a single reactor node.

Ports the per-node entries of the `core_temp` (fuel) and `graph_temp`
(graphite) profiles.

```rust
pub struct NodeState {
    pub core_temperature: uom::si::f64::ThermodynamicTemperature,
    pub graphite_temperature: uom::si::f64::ThermodynamicTemperature,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `core_temperature` | `uom::si::f64::ThermodynamicTemperature` | Fuel/kernel temperature at the node ([`ThermodynamicTemperature`]). |
| `graphite_temperature` | `uom::si::f64::ThermodynamicTemperature` | Graphite temperature at the node ([`ThermodynamicTemperature`]). |

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
    fn clone(self: &Self) -> NodeState { /* ... */ }
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
#### Struct `ParentPools`

Parent-nuclide activity pools fed into a daughter's node calculation.

Ports the `C_Parent` / `P_Parent` / `HPS_Parent` arrays threaded through
`higher_activities`. All three are **effective-unit `f64`** (same convention
as [`NodalActivities`]); pass [`ParentPools::none`] when the nuclide has no
tracked parent, or a parent's [`NodalActivities::parent_pools`] otherwise.

```rust
pub struct ParentPools {
    pub circulating: f64,
    pub plate_out: f64,
    pub clean_up: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `circulating` | `f64` | Parent circulating pool `C_parent` (effective `f64`). |
| `plate_out` | `f64` | Parent plate-out pool `P_parent` (effective `f64`). |
| `clean_up` | `f64` | Parent clean-up pool `HPS_parent` (effective `f64`). |

##### Implementations

###### Methods

- ```rust
  pub fn none() -> Self { /* ... */ }
  ```
  No parent contribution (all pools zero).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> ParentPools { /* ... */ }
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
    fn default() -> ParentPools { /* ... */ }
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
#### Struct `NodalActivities`

The six normal-operation activity outputs for one nuclide at one node,
in **effective units** (before the `× λ / 3.7e10` curie conversion).

Convert to reportable curies with [`to_curies`](Self::to_curies); chain into a
daughter nuclide with [`parent_pools`](Self::parent_pools).

```rust
pub struct NodalActivities {
    pub release_rate: f64,
    pub source_rate: f64,
    pub graphite_activity: f64,
    pub circulating_activity: f64,
    pub plate_out_activity: f64,
    pub clean_up_activity: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `release_rate` | `f64` | Release rate `R` from the fuel (effective `f64`, Bq = `atoms/s`). |
| `source_rate` | `f64` | Coolant source rate `S` after graphite attenuation (effective `f64`, Bq). |
| `graphite_activity` | `f64` | Graphite hold-up activity `G` (effective `f64`, atom count). |
| `circulating_activity` | `f64` | Circulating coolant pool `C` (effective `f64`, atom count). |
| `plate_out_activity` | `f64` | Plate-out pool `P` (effective `f64`, atom count). |
| `clean_up_activity` | `f64` | Clean-up / HPS pool `HPS` (effective `f64`, atom count); `0` when the HPS<br>is disabled or the nuclide is not a volatile. |

##### Implementations

###### Methods

- ```rust
  pub fn parent_pools(self: &Self) -> ParentPools { /* ... */ }
  ```
  The parent-chaining pools ([`ParentPools`]) to feed into a daughter

- ```rust
  pub fn to_curies(self: &Self, decay_constant: DecayConstant) -> NodalActivitiesCurie { /* ... */ }
  ```
  Convert to reportable curies via the single `× λ / 3.7e10` step.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> NodalActivities { /* ... */ }
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
    fn eq(self: &Self, other: &NodalActivities) -> bool { /* ... */ }
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
#### Struct `NodalActivitiesCurie`

The six normal-operation outputs in reportable **curies** (rates in `Ci/s`).

Field-for-field the curie form of [`NodalActivities`]; the two rate fields
(`release_rate`, `source_rate`) are in `Ci/s`, the four pool fields in `Ci`.

```rust
pub struct NodalActivitiesCurie {
    pub release_rate: f64,
    pub source_rate: f64,
    pub graphite_activity: f64,
    pub circulating_activity: f64,
    pub plate_out_activity: f64,
    pub clean_up_activity: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `release_rate` | `f64` | Release rate `R` (`Ci/s`). |
| `source_rate` | `f64` | Source rate `S` (`Ci/s`). |
| `graphite_activity` | `f64` | Graphite activity `G` (`Ci`). |
| `circulating_activity` | `f64` | Circulating activity `C` (`Ci`). |
| `plate_out_activity` | `f64` | Plate-out activity `P` (`Ci`). |
| `clean_up_activity` | `f64` | Clean-up / HPS activity (`Ci`). |

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
    fn clone(self: &Self) -> NodalActivitiesCurie { /* ... */ }
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
    fn eq(self: &Self, other: &NodalActivitiesCurie) -> bool { /* ... */ }
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

#### Function `normal_operation_node`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`
- `MustUse { reason: None }`

Compute the normal-operation activity source term for one nuclide at one node.

Ports the per-nuclide, per-node body of `trisoatops.py::normal_operation`
together with `calculation_functions.py::higher_activities`. The group-
dependent removal-constant routing follows the upstream exactly:

- **Noble gas** → no plate-out (`k_plate = 0`); plate-out pool forced to `0`;
  clean-up applies (`k_clean`) only when `hps_enabled`.
- **Halogen** → plate-out and (when `hps_enabled`) clean-up both apply.
- **Special metal / silver / other** → plate-out applies; clean-up never
  applies (metals are not scrubbed by the HPS).

When `hps_enabled` is `false`, the clean-up rate constant is treated as `0`
for every group and the clean-up pool is `0`, matching the upstream
`clean is False` branch.

# Arguments
- `nuclide` — the species record ([`TrisoAtopsNuclide`]); supplies `z`, `λ`,
  and transport group.
- `short_lived` — the run-dependent short-lived flag `sl` (upstream:
  `t½ / t_irad < 0.2`).
- `inventory` — the node's radionuclide inventory as an [`Activity`] (Bq);
  from [`crate::triso_atops_fork::activities::becquerels_from_curies`].
- `fractions` — the fuel [`FailureFractions`].
- `plant` — reactor-level [`PlantConstants`].
- `node` — the node [`NodeState`] (core + graphite temperatures).
- `hps_enabled` — whether the helium-purification system is modelled.
- `parent` — parent-nuclide [`ParentPools`]; [`ParentPools::none`] if none.

# Returns
The effective-unit [`NodalActivities`]; call
[`NodalActivities::to_curies`] for the reportable curie values.

```rust
pub fn normal_operation_node(nuclide: &crate::triso_atops_fork::TrisoAtopsNuclide, short_lived: bool, inventory: crate::triso_atops_fork::Activity, fractions: crate::triso_atops_fork::activities::FailureFractions, plant: PlantConstants, node: NodeState, hps_enabled: bool, parent: ParentPools) -> NodalActivities { /* ... */ }
```

## Module `accident`

Depressurisation-accident release: inventory drawdown, coolant venting.
Depressurisation-accident release: how much of the activity a normal
operation left sitting in the fuel and the primary circuit escapes when the
coolant blows down.

The chain upstream's `accident_case` runs, per nuclide and per node:

```text
  integrate(D over the transient T(t))            -> diffusion integral
    -> release_fraction(kernel) / (graphite)      -> dimensionless RF
      -> release_activity(what is left to release) -> atoms
        -> x lambda / 3.7e10                       -> curies
          -> x coolant_release fraction + lift-off -> released curies
```

The first two steps already live in
[`diffusion`](super::diffusion) and
[`release_models`](super::release_models); this module adds the rest.

# Scope limit

Like the whole crate this is **research, education and V&V only**, and an
accident source term especially must not be presented as authoritative for
emergency planning, emergency response or licensing. See the crate docs.

```rust
pub mod accident { /* ... */ }
```

### Types

#### Struct `AccidentFractions`

The six failure fractions an accident run uses.

Upstream assembles these into a bare six-element array
(`trisoatops.py::accident_case`) from `constants[0..3]` plus
`constants[12..13]`; naming them here is what stops an index slip from
silently reinterpreting the source term.

All six are dimensionless fractions in `[0, 1]`.

```rust
pub struct AccidentFractions {
    pub heavy_metal: f64,
    pub sic: f64,
    pub incremental: f64,
    pub incremental_sic: f64,
    pub incremental_accident: f64,
    pub incremental_sic_accident: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `heavy_metal` | `f64` | `f_hm` — heavy-metal contamination fraction (fuel outside intact particles). |
| `sic` | `f64` | `f_sic` — as-manufactured SiC-defective fraction. |
| `incremental` | `f64` | `f_inc` — incremental in-service failure fraction during normal operation. |
| `incremental_sic` | `f64` | `f_inc_sic` — incremental SiC-only failure fraction during normal operation. |
| `incremental_accident` | `f64` | `f_inc_acc` — **additional** incremental failure fraction caused by the<br>accident itself. |
| `incremental_sic_accident` | `f64` | `f_inc_sic_acc` — additional incremental SiC-only failure from the accident. |

##### Implementations

###### Methods

- ```rust
  pub fn total(self: &Self) -> f64 { /* ... */ }
  ```
  Upstream's `np.sum(fractions)` — all six.

- ```rust
  pub fn normal_operation_sum(self: &Self) -> f64 { /* ... */ }
  ```
  Upstream's `np.sum(fractions[:4])` — the four normal-operation fractions.

- ```rust
  pub fn accident_sum(self: &Self) -> f64 { /* ... */ }
  ```
  Upstream's `np.sum(fractions[4:])` — the two accident-only fractions.

- ```rust
  pub fn volatile_sum(self: &Self) -> f64 { /* ... */ }
  ```
  Upstream's `fractions[0] + fractions[2] + fractions[-2]` — the volatile

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> AccidentFractions { /* ... */ }
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
    fn eq(self: &Self, other: &AccidentFractions) -> bool { /* ... */ }
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
#### Struct `NormalOperationNode`

The per-node normal-operation state an accident release draws down.

Mirrors upstream's `nodal_data[nuclide][channel, radial, axial]` array, one
node's worth. Channel 0 is an **atom count**; channels 1-6 are the
activities `normal_operation_node` produces, in the same units it produces
them (atoms, or atoms/second for the two rates — the curie conversion
happens later).

Naming the channels is not cosmetic: upstream indexes this array by bare
integer at eleven sites in `release_activity` alone, and `[5]` versus `[6]`
is the difference between plate-out and clean-up.

```rust
pub struct NormalOperationNode {
    pub kernel_inventory_atoms: f64,
    pub release_rate: f64,
    pub source_rate: f64,
    pub graphite_activity: f64,
    pub circulating_activity: f64,
    pub plate_out_activity: f64,
    pub clean_up_activity: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `kernel_inventory_atoms` | `f64` | Channel 0 — kernel inventory, **atoms**<br>(upstream: `inventory / lambda * 3.7e10`). |
| `release_rate` | `f64` | Channel 1 — TRISO release rate, atoms/s. |
| `source_rate` | `f64` | Channel 2 — source rate into the coolant, atoms/s. |
| `graphite_activity` | `f64` | Channel 3 — activity held up in the matrix graphite, atoms. |
| `circulating_activity` | `f64` | Channel 4 — circulating activity, atoms. |
| `plate_out_activity` | `f64` | Channel 5 — plated-out activity, atoms. |
| `clean_up_activity` | `f64` | Channel 6 — activity removed by the clean-up system, atoms. Zero when<br>no clean-up system is fitted. |

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
    fn clone(self: &Self) -> NormalOperationNode { /* ... */ }
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
    fn default() -> NormalOperationNode { /* ... */ }
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
    fn eq(self: &Self, other: &NormalOperationNode) -> bool { /* ... */ }
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
#### Enum `ReleaseMaterial`

Which material's release fraction is being converted to an activity.

```rust
pub enum ReleaseMaterial {
    Kernel,
    Graphite,
}
```

##### Variants

###### `Kernel`

Release out of the fuel kernel.

###### `Graphite`

Release out of the matrix graphite.

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
    fn clone(self: &Self) -> ReleaseMaterial { /* ... */ }
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
    fn eq(self: &Self, other: &ReleaseMaterial) -> bool { /* ... */ }
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

#### Function `distribute_inventory_axially`

**Attributes:**

- `MustUse { reason: None }`

Distribute a per-radial-ring inventory evenly over the axial nodes.

Ports `inventory_processing`. A run may supply inventories either already
resolved per node (radial × time × axial, passed through untouched) or only
per radial ring, in which case each ring's inventory is **divided equally**
among `n_axial` nodes.

# Arguments
- `ring_inventory` — inventory for one radial ring, in whatever unit the
  caller is working in (upstream uses curies here, before the
  `/ lambda * 3.7e10` conversion to atoms). Must be finite.
- `n_axial` — number of axial nodes to spread it over; must be `>= 1`.

# Returns
`n_axial` equal shares, each `ring_inventory / n_axial`.

# Divergence from upstream
Upstream handles `ndim == 3` (pass through) and `ndim == 2` (split) and has
**no `else`**, so any other rank falls off the end and returns `None`,
which then fails confusingly downstream. Rust's type system removes that
case: the pass-through variant is simply not routed here.

# Panics
Panics if `n_axial == 0`.

```rust
pub fn distribute_inventory_axially(ring_inventory: f64, n_axial: usize) -> Vec<f64> { /* ... */ }
```

#### Function `release_activity`

**Attributes:**

- `MustUse { reason: None }`

The activity still available for accident release at one node, in atoms.

Ports `release_activity`. The idea is a mass balance: take the node's total
kernel inventory, scale it by the failure fraction appropriate to the
nuclide's transport group, then subtract whatever normal operation has
already moved elsewhere — graphite hold-up, circulating activity, plate-out
and clean-up.

Upstream evaluates two expressions and selects between them with a
per-node mask:

```text
  condition = inventory * sum(fractions[:4]) - plate_out > 0
```

i.e. *has normal operation already released more than it plated out?* Where
that holds, the fuller expression (scaled by the group fraction, and
subtracting plate-out as well) is used; elsewhere the accident-only
fractions apply and plate-out is not subtracted.

# Arguments
- `group` — the nuclide's transport group, which picks the failure fraction.
- `fractions` — the six accident failure fractions.
- `node` — that node's normal-operation state.
- `release_fraction` — the dimensionless RF from
  [`release_fraction_transient`](super::release_models::release_fraction_transient).
- `clean` — whether a clean-up system is fitted; when `true` the clean-up
  channel is subtracted too.
- `material` — kernel or graphite.

# Returns
Released activity at this node, **atoms**. May be negative if the
subtractions exceed the scaled inventory; upstream does not clamp, and
neither does this — a negative value is a signal that the normal-operation
and accident fraction sets are inconsistent, and hiding it would hide that.

# UPSTREAM DEFECT reproduced behind a flag: the silver group is `z == 48`

At `calculation_functions.py:906` the silver branch reads
`z == 47 or z == 48` — silver and **cadmium**. Every other silver-group
test in the file (lines 208, 722, 747, 775) reads `z == 47 or z == 46`,
silver and **palladium**. Both affected nuclides ship in the table
(`Pd-107`, `Z = 46`; `Cd-113`, `Z = 48`), so this is reachable: under
upstream, Pd-107 falls through to the all-fractions sum instead of
receiving `fract = 1`, and Cd-113 wrongly receives the silver treatment.

Four sites against one make `48` the near-certain typo, so this port treats
[`ElementGroup::Silver`] (Ag **and** Pd, as the rest of the model defines
it) as the silver branch. Pass `upstream_cadmium_typo = true` to reproduce
the stock behaviour for code-to-code comparison.

```rust
pub fn release_activity(group: super::nuclide_model::ElementGroup, fractions: AccidentFractions, node: NormalOperationNode, release_fraction: f64, clean: bool, material: ReleaseMaterial, upstream_cadmium_typo: bool, z: u32) -> f64 { /* ... */ }
```

#### Function `coolant_release`

**Attributes:**

- `MustUse { reason: None }`

Fraction of the primary coolant vented, over the venting window.

Ports `coolant_release`. During a depressurisation the coolant leaves while
the core is heating: by the ideal gas law at fixed pressure and volume,
`n = PV/RT`, so `dn/dt = -(P/R) (dT/dt) / T^2`. Upstream integrates that
trapezoidally, normalises by the initial mole count, and reports only the
samples where the bed is heating (`dT/dt >= 0`), which is when gas is
actually being pushed out.

# Arguments
- `times` — sample times, SI seconds, strictly increasing, at least 2.
- `mean_dtdt` — mean `dT/dt` across the core at each sample, K/s (upstream
  averages over the radial and axial axes). Same length as `times`.
- `hot_node_temperature` — temperature of the reference (hottest) node at
  each sample. Upstream defaults to the innermost ring, centre axial node,
  and takes these in **degrees Celsius**, converting with `+ 273.15`
  inline; this port takes a `uom` temperature so the unit cannot be
  mistaken. Same length as `times`.
- `pressure` — system pressure; upstream's default is `101.325` in the
  **kilopascal** the `R = 8.31447` J/(mol·K) denominator implies.

# Returns
`(fraction, vent_times)` — the released fraction at each venting sample and
the times those correspond to. Upstream forces the first element to exactly
`1`, which this reproduces.

# The `pressure` argument cannot change the answer — measured, not assumed

`frac = |integral / n_0|`, and both the integral (`dn/dt = -(P/R)...`) and
the normalisation (`n_0 = P/(R T_0)`) carry the same `P/R` factor, so it
cancels exactly. Verified against upstream on 2026-09-21: feeding
`P = 1.0`, `101.325`, `202.65` and `5000.0` through
`calculation_functions.coolant_release` returns **bit-identical** fractions.

**This port agrees to within 1 ulp, not bit-exactly.** The cancellation is
algebraic, and its exactness depends on operation order: upstream's NumPy
expression happens to cancel exactly, while this port's
`-p / R * dTdt / T / T` against `p / R / T_0` does not for every `p`
(measured: `1.0` kPa moves the second sample by one ulp,
`5.07056142185376e-2` against `5.070561421853761e-2`). That is a
floating-point artefact of the same algebra, not a physical dependence.

The parameter is kept because it is upstream's signature and because a
future formulation that tracks absolute moles would need it — but a caller
tuning it expecting a different release fraction is wasting their time, and
this is the only place that says so. [`pressure_does_not_affect_the_fraction`]
pins it, since a fixture comparison cannot: upstream's own output does not
depend on it either.

# Two upstream quirks preserved

1. **`frac[0] = 1`** is hard-coded, so the first venting sample always
   reports a fully released coolant regardless of the integral.
2. **The venting samples need not be contiguous.** `np.where(dTdt_avg >= 0)`
   selects every heating sample, so a transient that cools and re-heats
   produces a gappy set, and the returned times are that same gappy set.

# Panics
Panics if the three slices differ in length, if fewer than two samples are
supplied, or if any temperature is at or below absolute zero.

```rust
pub fn coolant_release(times: &[uom::si::f64::Time], mean_dtdt: &[f64], hot_node_temperature: &[uom::si::f64::ThermodynamicTemperature], pressure: uom::si::f64::Pressure) -> (Vec<f64>, Vec<uom::si::f64::Time>) { /* ... */ }
```

#### Function `mean_temperature_rate`

**Attributes:**

- `MustUse { reason: None }`

Mean `dT/dt` at each sample, averaged across the core.

Ports the first three lines of `coolant_release`, which upstream computes
inline: a backward difference in time, zero-padded at the first sample, then
averaged over the radial and axial axes.

Separated out because it is the only part of the calculation that needs the
full 3-D temperature field; splitting it lets [`coolant_release`] stay a
slice-based function like the rest of the port.

# Arguments
- `times` — sample times, SI seconds, strictly increasing.
- `node_temperatures` — `[node][time]` temperature history for every node
  in the core (radial × axial flattened; the average does not care about
  the layout). Every inner slice must match `times` in length.

# Returns
Mean `dT/dt` in K/s at each sample; the first entry is `0` by construction.

# Note on upstream's epsilon
Upstream divides by `np.diff(times) + np.finfo(float).eps` to avoid a
zero-division on repeated timestamps. This port asserts strictly increasing
times instead, which is the condition that epsilon was papering over.

# Panics
Panics on ragged input, fewer than two samples, or non-increasing times.

```rust
pub fn mean_temperature_rate(times: &[uom::si::f64::Time], node_temperatures: &[Vec<uom::si::f64::ThermodynamicTemperature>]) -> Vec<f64> { /* ... */ }
```

#### Function `accident_release_curies`

**Attributes:**

- `MustUse { reason: None }`

Total released activity for one nuclide over the accident, in **curies**.

Ports the per-nuclide body of `trisoatops.py::accident_case`:

```text
  released(t) = frac(t) * (kernel + graphite)          [curies]
              + circulating + x_liftoff * plate_out    [curies, released at once]
```

The first term is what diffuses out of the fuel and matrix during the
transient, scaled by the fraction of coolant that has actually vented by
time `t`. The second is the primary-circuit inventory: circulating activity
leaves with the coolant, and a `x_liftoff` share of the plated-out activity
is re-entrained by the blowdown. That second term is **not** scaled by
`frac`, and is constant in `t`.

# Arguments
- `kernel_release_curies` — already-converted kernel release at each
  transient sample (i.e. [`release_activity`] on the kernel, times
  `lambda / 3.7e10`).
- `graphite_release_curies` — the same for the graphite path.
- `vent_fraction` — the vented-coolant fraction at each sample, from
  [`coolant_release`]. Must match the two release slices in length.
- `circulating_curies`, `plate_out_curies` — the node-summed
  normal-operation inventories, already in curies.
- `x_liftoff` — re-entrained share of plate-out, dimensionless `[0, 1]`.

# Returns
Released activity in curies at each sample.

# UPSTREAM DEFECT not reproduced: `accident_temp[:, :-rmv, :]`

`accident_case` truncates the temperature history to the venting window
with

```python
rmv = np.size(times) - np.size(times_short)
accident_temp = accident_temp[:, :-rmv, :]
```

When nothing is truncated — every sample is a venting sample, which is
exactly what a monotonic heat-up produces — `rmv` is `0`, and `[:-0]` in
Python is `[:0]`, i.e. **the empty slice**. The temperature history is
silently discarded and every downstream integral is empty.

This port cannot reproduce that: the slices are passed in already aligned,
and a length mismatch is an assertion rather than an empty result. Recorded
here because a reader comparing against a stock TRISO-ATOPS run on a
monotonic transient will see upstream produce nothing and should know why.

# Panics
Panics if the three per-sample slices differ in length, or if `x_liftoff`
is outside `[0, 1]`.

```rust
pub fn accident_release_curies(kernel_release_curies: &[f64], graphite_release_curies: &[f64], vent_fraction: &[f64], circulating_curies: f64, plate_out_curies: f64, x_liftoff: f64) -> Vec<f64> { /* ... */ }
```

#### Function `atoms_to_curies`

**Attributes:**

- `MustUse { reason: None }`

Convert an activity in atoms to curies: `atoms * lambda / 3.7e10`.

Ports the `* lam / 3.7e10` conversion `accident_case` applies to both
release paths. Provided as a named function because upstream writes that
literal at four separate sites, and `3.7e10` is easy to mistype.

# Arguments
- `atoms` — activity as an atom count.
- `decay_constant` — `lambda`, s⁻¹.

```rust
pub fn atoms_to_curies(atoms: f64, decay_constant: f64) -> f64 { /* ... */ }
```

## Module `run_file`

The JSON run file: parsing, validation and unit attachment.
The JSON run file: the document TRISO-ATOPS' GUI writes and its CLI reads.

Upstream passes a bare `np.ndarray` of fifteen constants between every
layer, indexed by position — `constants[6]` is the plate-out rate,
`constants[14]` is the lift-off fraction, and nothing in the type system
says so. This module parses that document once into a named, `uom`-typed
[`RunConfig`], so an index slip becomes impossible rather than silent.

# What is deliberately NOT ported

Three `run_functions.py` entries have no Rust counterpart here, and their
absence is a decision rather than an omission:

- **`create_log`** configures Python's `logging` module. Rust callers pick
  their own facade (`log`, `tracing`, or none); a library that installs a
  global logger is badly behaved. Diagnostics surface as
  [`RunFileError`] values instead, which a caller can log however it likes.
- **`count_errors`** is a counter upstream threads through every function
  because Python has no `Result`. Its job is done by `Result` here.
- **`trisoatops()`** prints a version banner to stdout.

# Unit convention

The JSON carries **bare numbers**, and upstream attaches units positionally
through a parallel `const_units` list: lengths in metres, rate constants in
s⁻¹, and **`run_time` and `irradiation_time` in years**. Those two are the
trap — a caller who assumes seconds is out by a factor of 3.15e7 — so
[`RunFile::to_config`] converts them explicitly and the field docs say so.

```rust
pub mod run_file { /* ... */ }
```

### Types

#### Enum `TimeUnit`

A time unit the run file may express a duration in.

Ports `convert_time`. Upstream returns `None` for an unrecognised unit and
the caller then multiplies by it, raising `TypeError` well away from the
mistake; here an unknown unit is a parse failure at the boundary.

```rust
pub enum TimeUnit {
    Second,
    Minute,
    Hour,
    Day,
    Year,
}
```

##### Variants

###### `Second`

Seconds (`"s"`), factor 1.

###### `Minute`

Minutes (`"min"`), factor 60.

###### `Hour`

Hours (`"hr"`), factor 3600.

###### `Day`

Days (`"d"`), factor 86 400.

###### `Year`

Years (`"yr"`), factor 31 536 000.

**A 365-day year**, not the 365.25-day Julian year. Upstream spells it
`365 * 24 * 3600`; the 0.07 % difference against a Julian year is small
but systematic, so the port keeps upstream's definition rather than
silently improving it.

##### Implementations

###### Methods

- ```rust
  pub fn seconds(self: Self) -> f64 { /* ... */ }
  ```
  Seconds per unit. Ports `convert_time`'s factor table exactly.

- ```rust
  pub fn parse(unit: &str) -> Option<Self> { /* ... */ }
  ```
  Parse upstream's unit string (`"s"`, `"min"`, `"hr"`, `"d"`, `"yr"`).

- ```rust
  pub fn to_time(self: Self, value: f64) -> Time { /* ... */ }
  ```
  Convert a duration in this unit to a `uom` [`Time`].

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> TimeUnit { /* ... */ }
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

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
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
    fn eq(self: &Self, other: &TimeUnit) -> bool { /* ... */ }
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
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

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
#### Enum `RunFileError`

Why a run file could not be turned into a [`RunConfig`].

Ports the conditions `check_run_file` and `process_run_file` log and count.

```rust
pub enum RunFileError {
    MissingKey {
        key: String,
    },
    OutOfRange {
        key: String,
        value: f64,
        expected: String,
    },
    ShapeMismatch {
        key: String,
        found: usize,
        expected: usize,
    },
}
```

##### Variants

###### `MissingKey`

A key the run demands is absent.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `key` | `String` | The absent key, spelled as the JSON uses it. |

###### `OutOfRange`

A value is present but outside its physically admissible range.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `key` | `String` | Which key. |
| `value` | `f64` | The offending value. |
| `expected` | `String` | What was required, in words (e.g. `"a fraction in [0, 1]"`). |

###### `ShapeMismatch`

A temperature or inventory table does not match the declared node counts.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `key` | `String` | Which table. |
| `found` | `usize` | Elements found. |
| `expected` | `usize` | Elements the `n_radial x n_axial` declaration implies. |

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
    fn clone(self: &Self) -> RunFileError { /* ... */ }
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
    fn eq(self: &Self, other: &RunFileError) -> bool { /* ... */ }
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
#### Struct `RunFile`

The run file exactly as it appears on disk.

Field names match the JSON keys upstream's `required_keys` /
`accident_keys` lists demand, so `serde` reads a GUI-written file directly.
Every quantity is a bare number here; [`RunFile::to_config`] is what
attaches units and validates.

```rust
pub struct RunFile {
    pub f_hm: f64,
    pub f_sic: f64,
    pub f_inc: f64,
    pub f_inc_sic: f64,
    pub a_graph: f64,
    pub a_grain: f64,
    pub k_plate: f64,
    pub run_time: f64,
    pub irradiation_time: f64,
    pub k_clean: f64,
    pub r_kernel: f64,
    pub a_sic: f64,
    pub hps_tog: bool,
    pub accident_tog: bool,
    pub n_radial: usize,
    pub n_axial: usize,
    pub nuclides: Vec<String>,
    pub inventories: Vec<f64>,
    pub core_temps: Vec<f64>,
    pub graphite_temps: Vec<f64>,
    pub f_inc_acc: f64,
    pub f_inc_sic_acc: f64,
    pub x_liftoff: f64,
    pub times: Vec<f64>,
    pub accident_temps: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `f_hm` | `f64` | Heavy-metal contamination fraction, dimensionless. |
| `f_sic` | `f64` | As-manufactured SiC-defective fraction, dimensionless. |
| `f_inc` | `f64` | Incremental in-service failure fraction, dimensionless. |
| `f_inc_sic` | `f64` | Incremental SiC-only failure fraction, dimensionless. |
| `a_graph` | `f64` | Matrix graphite thickness, **metres**. |
| `a_grain` | `f64` | Fuel grain size, **metres**. |
| `k_plate` | `f64` | Plate-out rate constant, **s⁻¹**. |
| `run_time` | `f64` | Reactor run time, **years** — see the module's unit note. |
| `irradiation_time` | `f64` | Irradiation time, **years** — see the module's unit note. |
| `k_clean` | `f64` | Clean-up (HPS) rate constant, **s⁻¹**. Ignored when `hps_tog` is false;<br>upstream `continue`s past it rather than reading it, so a run with the<br>HPS off need not supply a meaningful value. |
| `r_kernel` | `f64` | Fuel kernel radius, **metres**. |
| `a_sic` | `f64` | SiC layer thickness, **metres**. |
| `hps_tog` | `bool` | Whether a helium purification (clean-up) system is fitted. |
| `accident_tog` | `bool` | Whether to run the depressurisation accident after normal operation. |
| `n_radial` | `usize` | Number of radial rings. |
| `n_axial` | `usize` | Number of axial nodes. |
| `nuclides` | `Vec<String>` | Nuclide names, in the run file's own spelling. |
| `inventories` | `Vec<f64>` | Per-nuclide inventories, curies. |
| `core_temps` | `Vec<f64>` | Core temperature per node, °C, row-major `n_radial x n_axial`. |
| `graphite_temps` | `Vec<f64>` | Graphite temperature per node, °C, row-major `n_radial x n_axial`. |
| `f_inc_acc` | `f64` | Additional incremental failure fraction from the accident. Required<br>only when `accident_tog`. |
| `f_inc_sic_acc` | `f64` | Additional incremental SiC-only failure from the accident. Accident only. |
| `x_liftoff` | `f64` | Lift-off fraction — the share of plated-out activity re-entrained by the<br>blowdown. Accident only, dimensionless. |
| `times` | `Vec<f64>` | Accident transient sample times, seconds. Accident only. |
| `accident_temps` | `Vec<f64>` | Accident transient temperatures, °C. Accident only. |

##### Implementations

###### Methods

- ```rust
  pub fn to_config(self: &Self) -> Result<RunConfig, Vec<RunFileError>> { /* ... */ }
  ```
  Validate and convert to a [`RunConfig`].

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> RunFile { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
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
    fn eq(self: &Self, other: &RunFile) -> bool { /* ... */ }
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
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
    ```

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
#### Struct `RunConfig`

A validated, unit-carrying run configuration.

This is what upstream's positional `constants` array becomes once every
index has a name and a unit.

```rust
pub struct RunConfig {
    pub f_hm: f64,
    pub f_sic: f64,
    pub f_inc: f64,
    pub f_inc_sic: f64,
    pub graphite_thickness: uom::si::f64::Length,
    pub grain_size: uom::si::f64::Length,
    pub k_plate: uom::si::f64::Frequency,
    pub run_time: uom::si::f64::Time,
    pub irradiation_time: uom::si::f64::Time,
    pub k_clean: uom::si::f64::Frequency,
    pub kernel_radius: uom::si::f64::Length,
    pub sic_thickness: uom::si::f64::Length,
    pub hps: bool,
    pub accident: bool,
    pub n_radial: usize,
    pub n_axial: usize,
    pub nuclides: Vec<String>,
    pub inventories: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `f_hm` | `f64` | Heavy-metal contamination fraction. |
| `f_sic` | `f64` | As-manufactured SiC-defective fraction. |
| `f_inc` | `f64` | Incremental in-service failure fraction. |
| `f_inc_sic` | `f64` | Incremental SiC-only failure fraction. |
| `graphite_thickness` | `uom::si::f64::Length` | Matrix graphite thickness. |
| `grain_size` | `uom::si::f64::Length` | Fuel grain size. |
| `k_plate` | `uom::si::f64::Frequency` | Plate-out rate constant. |
| `run_time` | `uom::si::f64::Time` | Reactor run time, converted from the file's years. |
| `irradiation_time` | `uom::si::f64::Time` | Irradiation time, converted from the file's years. |
| `k_clean` | `uom::si::f64::Frequency` | Clean-up rate constant; exactly zero when no HPS is fitted. |
| `kernel_radius` | `uom::si::f64::Length` | Fuel kernel radius. |
| `sic_thickness` | `uom::si::f64::Length` | SiC layer thickness. |
| `hps` | `bool` | Whether a clean-up system is fitted. |
| `accident` | `bool` | Whether the accident case runs. |
| `n_radial` | `usize` | Radial ring count. |
| `n_axial` | `usize` | Axial node count. |
| `nuclides` | `Vec<String>` | Nuclide names as supplied. |
| `inventories` | `Vec<f64>` | Per-nuclide inventories, curies. |

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
    fn clone(self: &Self) -> RunConfig { /* ... */ }
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
    fn eq(self: &Self, other: &RunConfig) -> bool { /* ... */ }
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
### Types

#### Type Alias `DecayConstant`

The radioactive decay constant `λ = ln 2 / t½`.

SI unit `s^-1`; dimensionally a frequency. Construct with
`Frequency::new::<hertz>(..)` and read with `.get::<hertz>()`
(`hertz` == `s^-1` here — the name is only a dimension label).

```rust
pub type DecayConstant = uom::si::f64::Frequency;
```

#### Type Alias `ReleaseFraction`

A dimensionless release fraction or release-to-birth (`<R/B>`) ratio.

Physically in `[0, 1]`. The release models clamp to this range where the
upstream code does. Construct with `Ratio::new::<ratio>(..)` and read with
`.get::<ratio>()`.

```rust
pub type ReleaseFraction = uom::si::f64::Ratio;
```

### Re-exports

#### Re-export `Activity`

```rust
pub use activities::Activity;
```

#### Re-export `ElementGroup`

```rust
pub use nuclide_model::ElementGroup;
```

#### Re-export `TrisoAtopsNuclide`

```rust
pub use nuclide_model::TrisoAtopsNuclide;
```

## Module `fuel_failure`

**boon-lay fuel failure** — TRISO coated-particle failure fractions. This
is boon-lay's own model: a Rust implementation of the *formulas* published
in the PANAMA-I report (Verfondern & Nabielek, Jülich HTA-IB-03/90, 1990),
coded agentically by an AI coding agent. It is **not** the PANAMA code, whose
source this project does not have; see the module docs on naming. Supplies what
[`triso_atops_fork`]'s `FailureFractions` currently takes as a
hand-entered number. See the module docs for what is implemented and what
is deliberately left as an input.
# boon-lay fuel failure — TRISO coated-particle failure from the PANAMA-I equations

## Naming: this is boon-lay fuel failure, NOT PANAMA

**Call this model "boon-lay fuel failure".** It is boon-lay's own code: a
Rust implementation of the *formulas* published in the PANAMA-I report
(Verfondern & Nabielek, HTA-IB-03/90), coded agentically by an AI coding
agent from the printed equations. **This project does not have the PANAMA
source code.** The PANAMA Fortran is closed-source and was never consulted,
so nothing here is PANAMA, a port of it, or a code-to-code match to it.

Keep the two names apart everywhere, because they are different evidence:

- **"PANAMA-I"** (or "the report") means Verfondern & Nabielek's document
  and **the results it prints** — Tables 1–2, Figs. 1–10. When a test here
  compares against "PANAMA's curve", it is comparing against the report's
  *published output*, which is the only PANAMA output this project has.
- **"boon-lay fuel failure"** (or "this implementation", "the chain")
  means the code in this module and every number it computes. A result
  such as "`φ₁ = 1.2·10⁻¹²` for HTR-10" is boon-lay fuel failure's, never
  "PANAMA's": PANAMA was never run on HTR-10.

Matching the report's printed tables and figures shows the formulas were
transcribed and assembled faithfully. It does not show agreement with the
PANAMA code on any case the report does not print.

## The model

A TRISO particle is a pressure vessel. Fission gas and CO accumulate inside
it, the SiC layer carries the hoop stress, and the particle fails when that
stress exceeds the SiC strength. The PANAMA-I equations couple three failure
populations, and boon-lay fuel failure implements them as follows:

| Term | Mechanism | Where it comes from |
|---|---|---|
| `φ_o` | as-manufactured defects | not modelled; an input (see [`AS_MANUFACTURED_TARGET`]) |
| `φ₁` | **pressure-vessel overstress** | [`weibull_failure_fraction`], this module |
| `φ₂` | SiC **thermal decomposition** above ~2000 °C | [`decomposition`], Eqs (11)–(14b) |

combined by [`total_failure_fraction`].

## What is implemented here, and what is deliberately absent

**Implemented — every equation below was read directly off the source scan
and is reproduced with its printed equation number:**

| Item | Eq. | Page |
|---|---|---|
| Weibull failure probability | (1) | -483- |
| Induced SiC stress, thin shell | (2) | -484- |
| Internal gas pressure, ideal gas | (3) | -484- |
| Failure-population combination | unnumbered | -480- |
| Booth release `f(τ)`, `F_d` | (4) + unnumbered | -485-/-486- |
| Molar volume `V_m` | (6a), (6b), (6c) | -491-/-492- |
| SiC thinning, corrosion rate | (7) | -492- |
| Strength and modulus after irradiation | (8a)–(9b) | -493-/-494- |
| Grain-boundary corrosion, **off by default** | (10b), (10c) | -495- |
| Thermal decomposition | (11)–(14b) | -496-/-497- |
| The time-stepping driver | §3.1 | -482-/-483- |

~~**Absent, and taken as INPUTS rather than guessed.**~~
**CORRECTED 2026-09-24** — every correlation this table used to list as
absent is now implemented in its own module, and the two ambiguities it
flagged are settled: `t_B` is **seconds** (Fig. 3) and the `f(τ)` series
groups the whole `1 − exp(…)` into the numerator (Fig. 1). What remains an
input is what the *report* leaves to the caller — the particle geometry,
`V_k`, `V_f`, `φ_o`, and the `α`/`β` of Eq (13), which the report says must
be determined by experiment and supplies two fits for.

**What is NOT verified, stated plainly.** Eqs (11)–(14b) have no figure or
table in the report to check them against, and Eq (4) has none either
(only the exact identity `F_d(τ_i, 0) = f(τ_i)`). Against Fig. 6 the chain
reproduces the eight-variety ordering 8/8 but carries a residual of
−0.37 … +0.39 decades that runs systematically with `m`; against Fig. 7 it
holds to 4.9 % through all three temperature stages for 300 h and then
drifts to a factor 1.90 by 977 h. Both are recorded with numbers in
`docs/panama-i-units-and-open-questions.md` and pinned by tests in
[`history`].

## The `ln2` in Eq (1) is load-bearing — do not reach for a stock Weibull

Eq (1) is `φ₁ = 1 − exp[−ln2·(σ_t/σ_o)^m]`, **not** the textbook
`1 − exp[−(σ/σ_c)^m]`. The `ln2` normalisation makes `σ_o` the **median**
strength: at `σ_t = σ_o`, `φ₁ = 1 − e^(−ln2) = 0.5` exactly. A stock Weibull
treats its scale parameter as the *characteristic* strength, where
`φ = 1 − e^(−1) ≈ 0.632`. Substituting one for the other misplaces the
strength scale by a factor `(ln2)^(1/m)` — about 4 % at `m = 8` — in a
direction that flatters the answer and produces no error.
[`median_is_the_scale_parameter`] pins this.

## Units

Public signatures are `uom`-typed. Two traps from the source's own symbol
list (-511-): the report prints irradiation and accident temperatures in
**°C** while every Arrhenius term needs **kelvin**, and it never states the
conversion. Using `uom` removes that ambiguity at the boundary — a caller
passes a `ThermodynamicTemperature` and cannot get it wrong.

# Layout

One module per equation group, because a single file was already past 700
lines with four of the report's correlations implemented and there are a
dozen more to come:

| Module | Equations | Page |
|---|---|---|
| [`geometry`] | `r`, `d_o`, `d_act` | -484- |
| [`diffusion`] | `D_S`, both kernel types | -487- |
| [`corrosion`] | (7), `FKOR`, the `v̇` Arrhenius | -492- |
| [`oxygen`] | (5a)–(5f), `OPF` | -488-/-489- |
| [`weibull`] | (1) | -483- |
| [`stress`] | (2) | -484- |
| [`pressure`] | (3) | -484-/-485- |
| [`booth`] | `f(τ)`, (4), `τ_i`/`τ_a` | -485-/-486- |
| [`molar_volume`] | (6a), (6b), (6c) | -491-/-492- |
| [`strength`] | (8a), (8b), (9a), (9b) | -493-/-494- |
| [`grain_boundary`] | (10b), (10c) — off by default | -495- |
| [`decomposition`] | (11), (12), (13), (14a), (14b) | -496-/-497- |
| [`history`] | the time-stepping driver, §3.1 | -482-/-483- |
| [`htr10`] | HTR-10 applied to the model — an **extrapolation** | — |

The assembly (`phi_total`) stays here, since it is what binds them.
Everything is re-exported flat, so a caller writes
`boon_lay::fuel_failure::weibull_failure_fraction` and never needs to know
which file it lives in.

# Units: what the report states, and what it does not

Three of the report's own symbols are ambiguous or wrong as printed. Each
is resolved (or left open) at the point of use, and the register lives in
`docs/panama-i-units-and-open-questions.md`. In brief:

| Symbol | Printed | Used here | How settled |
|---|---|---|---|
| `T_B` | degC (-511-) | **kelvin** | Table 1 reproduces 16/16 on kelvin, 0/16 on degC |
| `Gamma` | 10^25 m^-2 EDN | same, as bare `f64` | a `log10` fit is only valid in its own units |
| Eq (3) grouping | bar spans the denominator | `R*T` in the numerator | dimensions; the printed form makes `p` fall with `T` |
| `t_B` | seconds (-511-) | **seconds** | Fig. 3: seconds 0.0087, days 0.277 |

```rust
pub mod fuel_failure { /* ... */ }
```

### Modules

## Module `booth`

**Fission-gas release** — the Booth function `f(τ)` (unnumbered, page
-485-), Eq (4) for `F_d` (page -485-, Allelein 1983), and the
dimensionless times `τ_i`, `τ_a` (page -486-).

```text
f(τ) = 1 − (6/τ)·Σ_{n=1}^∞ (1 − exp(−n²π²τ)) / (n⁴π⁴)

F_d  = [ (τ_i + τ_a)·f(τ_i + τ_a) − τ_a·f(τ_a) ] / τ_i        (4)

τ_i  = D_S(T_B)·t_B      τ_a = D_S(T)·t
```

`F_d` is the fraction of the stable fission gas that has escaped the kernel
into the buffer void, and it is what multiplies the fission-gas yield in
Eq (3). `f` itself is the classical Booth release integral for a sphere
with a **constant production rate**; Eq (4) differences the
irradiation-plus-accident release against the accident-only part so that
what is left is the release attributable to the inventory built up during
irradiation.

# The printed series has a grouping ambiguity, and only one reading works

On page -485- the fraction bar in the summand spans **only**
`exp(−n²π²τ) / (n⁴π⁴)`, with the `(1 −` opening outside it. Read
literally, the summand is `1 − exp(−n²π²τ)/(n⁴π⁴)`, which tends to `1` as
`n → ∞` — the series **diverges**, and a 1000-term partial sum gives
`f(0.1) = −6.0·10⁴` instead of a number in `[0, 1]`.

The consistent reading puts the whole `1 − exp(…)` in the numerator, as
implemented. It is the reading verified below, and it is also the standard
Booth form.

| reading | `f(0.1)` | `f(0.5)` | `f(1.9)` |
|---|---|---|---|
| literal, bar over the exponential only | −5.99997·10⁴ | −1.19990·10⁴ | −3.1569·10³ |
| **whole `1 − exp(…)` in the numerator** | **0.56365** | **0.86755** | **0.96491** |

## Verification — methodology

Fig. 1 (page -486-) plots `f(τ)` over `τ ∈ [0, 2]` and is therefore a
direct check on the reading. Digitised by the maintainer 2026-09-24:
**78 points**, `τ` from 0.0313 to 1.9077, read off the printed curve.
(The digitiser's y-axis calibration labels the upper gridline `500`; a
least-squares fit of the digitised ordinate against this implementation,
forced through the origin and restricted to `τ ≥ 0.15` where the curve is
not near-vertical, gives a scale of **501.29**, i.e. that gridline is
`f = 1` to within 0.26 %. The comparison below uses `f = y/500`.)

Pass criterion: the consistent reading within digitisation noise over the
whole figure; the literal reading excluded by orders of magnitude.

## Verification — results, 2026-09-24

| sample | n | mean \|Δf\| | median | worst |
|---|---|---|---|---|
| all digitised points | 78 | **0.0066** | 0.0020 | 0.047 |
| `τ ≥ 0.15` | 68 | **0.0028** | — | 0.018 |

The worst point is the first one, `τ = 0.0313`, where the curve is nearly
vertical (`df/dτ ≈ 5`): a 0.009 error in reading `τ` off the page accounts
for the whole 0.047. On an ordinate running 0 to 1 the `τ ≥ 0.15` figure
of 0.0028 is digitisation noise. Pinned by
[`tests::figure_1_is_reproduced`] and
[`tests::the_literal_grouping_is_excluded`].

## Two analytic checks the figure cannot give

Because `Σ 1/(n⁴π⁴) = ζ(4)/π⁴ = 1/90` exactly, the series rearranges to
`f = 1 − 1/(15τ) + (6/τ)·Σ exp(−n²π²τ)/(n⁴π⁴)`, giving two limits that are
independent of the digitisation:

| limit | closed form | agreement |
|---|---|---|
| `τ → ∞` | `1 − 1/(15τ)` | 4·10⁻¹² at `τ = 10` |
| `τ → 0` | `4√(τ/π) − 3τ/2` | 2·10⁻⁷ at `τ = 10⁻⁴` |

Both are pinned by [`tests::the_analytic_limits_are_recovered`]. They also
fix the *normalisation*, which Fig. 1 alone cannot: a factor-of-two error
in the `6/τ` pre-factor would still plot as a plausible rising curve.

# Summation cut-off: the report's 1000-term cap is what binds

Page -485- states the "infinite" sum is terminated after **1000 summands
(caution!)**, or when two consecutive summands differ by no more than
**10⁻²⁰**. Both are implemented, but the second never fires first: the
summand tends to `1/(n⁴π⁴)`, whose consecutive differences reach 10⁻²⁰ only
near `n ≈ 5.3·10³`. The report's own "caution!" is well placed, and the
measured cost of the cap is:

| `τ` | 1000-term sum | error against the rearranged form |
|---|---|---|
| 10⁻² … 10¹ | 0.2107 … 0.9933 | ≤ 2·10⁻⁹ |
| 10⁻⁴ | 0.02242 | 3·10⁻⁷ |
| 10⁻⁶ | 0.002276 | ≤ 2·10⁻⁵ (bound `2/(N³π⁴τ)`) |
| 10⁻⁸ | 6.4·10⁻⁴ | ~4·10⁻⁴ — **larger than the answer** |

So this function is trustworthy for `τ ≳ 10⁻⁵` and degrades below it.
That limit is the report's algorithm, not an implementation shortcut, and
it is left in place rather than silently replaced by the rearranged form —
which is in any case *worse* for small `τ`, since it cancels
`1/(15τ) ≈ 6.7·10⁶` against itself to produce a number of order 10⁻⁴.
[`tests::the_thousand_term_cap_is_the_binding_one`] records both facts.

```rust
pub mod booth { /* ... */ }
```

### Functions

#### Function `booth_release_function`

The Booth release function `f(τ)` (unnumbered, page -485-).

```text
f(τ) = 1 − (6/τ)·Σ_{n=1}^∞ (1 − exp(−n²π²τ)) / (n⁴π⁴)
```

Dimensionless in and out. `f(0) = 0` and `f → 1` as `τ → ∞`; the return is
clamped to `[0, 1]` only at `τ ≤ 0`, where the expression is undefined —
everywhere else the series is left to speak for itself, so that a
mis-transcription shows up as an out-of-range value rather than being
hidden by a clamp.

Accurate for `τ ≳ 10⁻⁵`; see the module docs for the measured behaviour of
the report's 1000-term cap below that.

```rust
pub fn booth_release_function(tau: uom::si::f64::Ratio) -> uom::si::f64::Ratio { /* ... */ }
```

#### Function `dimensionless_time`

A dimensionless diffusion time `τ = D_S·t` (page -486-).

Used for both of the report's arguments: `τ_i = D_S(T_B)·t_B` over the
irradiation and `τ_a = D_S(T)·t` over the accident. `D_S = D_eff/r_o²` is
a [`Frequency`] (the report prints it in `s⁻¹`) so the product is
dimensionless by construction and cannot be assembled from the wrong pair
of quantities.

```rust
pub fn dimensionless_time(reduced_diffusion: uom::si::f64::Frequency, elapsed: uom::si::f64::Time) -> uom::si::f64::Ratio { /* ... */ }
```

#### Function `released_gas_fraction`

**Eq (4)** — the released fraction `F_d` of the stable fission gas
(page -485-, Allelein 1983).

```text
F_d = [ (τ_i + τ_a)·f(τ_i + τ_a) − τ_a·f(τ_a) ] / τ_i
```

- `tau_irradiation` — `τ_i = D_S(T_B)·t_B`.
- `tau_accident` — `τ_a = D_S(T)·t`.

Applies to Xe and Kr. The argument structure is as printed: `τ_i` appears
only inside the sum and in the denominator, so `F_d` is a *time-average*
over the irradiation rather than a release evaluated at its end.

## `τ_i = 0` is not defined by the report

Eq (4) is singular there and the report does not say what to do. This
returns **zero**, on the grounds that no irradiation means no fission-gas
inventory to release — and in Eq (3) the same limit carries `F_b = 0`
alongside, so the pressure is zero either way. That is this
implementation's convention and not the report's; it is recorded in
`docs/panama-i-units-and-open-questions.md`.

```rust
pub fn released_gas_fraction(tau_irradiation: uom::si::f64::Ratio, tau_accident: uom::si::f64::Ratio) -> uom::si::f64::Ratio { /* ... */ }
```

### Constants and Statics

#### Constant `MAX_SUMMANDS`

The report's cap on the number of summands (page -485-).

```rust
pub const MAX_SUMMANDS: usize = 1000;
```

#### Constant `SUMMAND_CONVERGENCE`

The report's convergence criterion on consecutive summands (page -485-).

Never the binding one — see the module docs.

```rust
pub const SUMMAND_CONVERGENCE: f64 = 1.0e-20;
```

## Module `corrosion`

**SiC layer thinning by volume corrosion** — Eq (7) and the corrosion-rate
Arrhenius (page -492-, attributed to Montgomery 1981).

```text
d_act = d_o / (1 + v̇·t/d_o) = d_o / FKOR        (7)
FKOR(t2) = FKOR(t1) + v̇(T_m)·(t2 − t1)/d_o
v̇ = A · exp(−179500/(R·T))   [m/s]
```

`FKOR` is carried forward across time steps rather than recomputed from a
total elapsed time, which is what lets a **varying** temperature history
accumulate correctly: each step adds `v̇(T_m)·Δt/d_o` at that step's own
mean temperature.

# The printed pre-factor is a decade out, and Fig. 4 proves it

The report prints `v̇ = 5.87·10⁻⁷ · exp(−179500/(R·T))`. That value does
**not** reproduce Fig. 4 on the facing page, which plots this very
equation for `d_o = 35 µm` at five isothermal temperatures. Checked
2026-09-24 against a digitisation of all five curves (483 points):

| pre-factor | mean abs error in `d_act/d_o` | worst |
|---|---|---|
| `5.87e-7` as printed | 0.12 – 0.51 per curve | 0.51 |
| **`5.87e-8`** | **0.0055** | 0.0142 |

On an axis running 0 to 1, 0.0055 is digitisation noise. Independently,
fitting `v̇` freely from the figure gives an activation energy of
**160.8 kJ/mol** against the printed 179.5 — but with the pre-factor
corrected the printed activation energy fits every curve, so the free fit
was absorbing the decade rather than finding a different energy.

[`PRINTED_PREFACTOR`] and [`FIGURE_PREFACTOR`] are both exposed and the
**figure's** value is the default, because it is the one the report's own
plotted output is consistent with. This is a departure from the rule used
for `D_S` in [`super::diffusion`], where the equation was preferred over
the figure — the difference is that there the two disagreed in *slope*,
with no single parameter reconciling them, whereas here one factor of ten
reconciles five curves over a 1000 °C span. That is a typo, not a
modelling choice.

```rust
pub mod corrosion { /* ... */ }
```

### Functions

#### Function `corrosion_rate`

The SiC volume-corrosion rate `v̇` \[m/s\] (page -492-, Montgomery 1981).

Uses [`FIGURE_PREFACTOR`]; see the module docs for why, and
[`corrosion_rate_with`] to evaluate the printed value instead.

```rust
pub fn corrosion_rate(temperature: uom::si::f64::ThermodynamicTemperature) -> uom::si::f64::Velocity { /* ... */ }
```

#### Function `corrosion_rate_with`

[`corrosion_rate`] with an explicit pre-factor, so the printed-vs-figure
discrepancy is reproducible rather than merely documented.

```rust
pub fn corrosion_rate_with(prefactor: f64, temperature: uom::si::f64::ThermodynamicTemperature) -> uom::si::f64::Velocity { /* ... */ }
```

#### Function `thinning_factor`

`FKOR` after holding at a constant `temperature` for `elapsed`, starting
from an uncorroded layer (page -492-).

`FKOR = 1 + v̇·t/d_o`, so `d_act = d_o/FKOR`. Starts at 1, never below it.

```rust
pub fn thinning_factor(initial_thickness: uom::si::f64::Length, temperature: uom::si::f64::ThermodynamicTemperature, elapsed: uom::si::f64::Time) -> uom::si::f64::Ratio { /* ... */ }
```

#### Function `advance_thinning_factor`

Advance `FKOR` across one time step at mean temperature `t_m`
(page -492-):

```text
FKOR(t2) = FKOR(t1) + v̇(T_m)·(t2 − t1)/d_o
```

Carried forward rather than recomputed from total elapsed time: that is
what makes a **varying** temperature history accumulate correctly, since
each step contributes at its own temperature. Recomputing from `t_total`
at the current temperature would silently apply the latest temperature to
the whole history.

```rust
pub fn advance_thinning_factor(previous: uom::si::f64::Ratio, initial_thickness: uom::si::f64::Length, mean_temperature: uom::si::f64::ThermodynamicTemperature, step: uom::si::f64::Time) -> uom::si::f64::Ratio { /* ... */ }
```

### Constants and Statics

#### Constant `CORROSION_ACTIVATION_J_PER_MOL`

Activation energy in the corrosion-rate Arrhenius, J/mol (page -492-).

```rust
pub const CORROSION_ACTIVATION_J_PER_MOL: f64 = 179_500.0;
```

#### Constant `PRINTED_PREFACTOR`

The pre-factor **as printed** on page -492-, `5.87e-7` m/s.

Does not reproduce Fig. 4; see the module docs. Exposed so the
discrepancy can be reproduced rather than only described.

```rust
pub const PRINTED_PREFACTOR: f64 = 5.87e-7;
```

#### Constant `FIGURE_PREFACTOR`

The pre-factor **Fig. 4 is consistent with**, `5.87e-8` m/s — the printed
value divided by ten. This is the default.

```rust
pub const FIGURE_PREFACTOR: f64 = 5.87e-8;
```

## Module `decomposition`

**SiC thermal decomposition** — `φ₂`. The Arrhenius "decay constant"
(unnumbered, page -495-, Benz 1982), the action integral `ζ` and its
discrete form Eq (11), the rate constant Eq (12), the failure law Eq (13)
and its two calibrations Eqs (14a)/(14b) (pages -496-, -497-).

```text
k        = k_o·exp(−Q/(R·T)),   Q = 556 kJ/mol                 (p-495)
ζ        = ∫ k(T) dt                                            (p-496)
ζ(t₂)    = ζ(t₁) + k(T_m)·(t₂ − t₁)                             (11)
k(T_m)   = (375/d_o)·exp(−556000/(R·T_m))   [s⁻¹]               (12)
φ₂(t,T)  = 1 − exp(−α·ζ^β)                                      (13)
α = 0.693 (= ln 2), β = 0.88   loose particles                  (14a)
α = 0.0001,         β = 4      particles in a sphere            (14b)
```

Above roughly 2000 °C this is the dominant failure mechanism, and it is
the one `φ₁` cannot see: SiC decomposes to gaseous Si and solid graphite,
so the layer stops being a pressure vessel rather than bursting as one.

# `ζ` carries the history; `φ₂` does not accumulate

This is the structural difference from `φ₁`, and the report is explicit
about it on page -483-: `ζ` increases monotonically step by step and
`φ₂(t₂)` is then read **directly** off Eq (13) at that `ζ`. Only the
*rate* `φ̇₂ = Δφ₂/Δt` is formed by differencing. `φ₁`, by contrast, is
accumulated from positive increments (page -482-).

Accumulating `φ₂` by increments instead would give the same answer for a
monotone temperature history and a different one for any history that
cools, which is exactly the case a reactor transient is. See
[`super::history`], where the two are stepped side by side.

# The units of the 375

Eq (12) prints `375/d_o` with `d_o` in metres and declares the result
`[s⁻¹]`. For that to hold, **375 must carry units of m/s**: it is the
`k_o` of the page -495- Arrhenius made concrete, a decomposition front
velocity divided by the layer it has to eat through. The report never says
so; it is the only reading that balances, and it is why
[`decomposition_rate_constant`] takes a `uom` [`Length`] rather than a
bare number. Recorded in `docs/panama-i-units-and-open-questions.md`.

A consequence worth stating: `k ∝ 1/d_o`, so a 50 µm layer decomposes
30 % more slowly than a 35 µm one at the same temperature, and `ζ` scales
with it directly.

# Verification status — no figure in the report checks this group

**Unlike Eqs (7), (8a), (9a), (5a)–(5f), `D_S` and `f(τ)`, nothing here is
verified against a plotted or tabulated output of the report.** Figs. 10–13's
reactor cases do not state `d_o`, and the Benz 1982 weight-loss data that
fixed `Q` is in the missing reference list (page -510-).

Fig. 6 does, however, **exclude one of the two calibrations**. At 1600 °C
with `d_o = 35 µm`, `ζ(300 h) = 3.6·10⁻³`, which under Eq (14a) gives
`φ₂ = 4.9·10⁻³` — a floor that every one of Fig. 6's eight curves would sit
on, where the figure in fact runs from 2·10⁻⁶ to about 5·10⁻³ and spreads
across three decades. Under Eq (14b) the same `ζ` gives `φ₂ = 1.7·10⁻¹⁴`,
invisible. So Fig. 6 was computed with the **sphere** calibration (14b), or
with `φ₂` switched off; it cannot have used (14a). Pinned by
[`tests::figure_6_excludes_the_loose_particle_calibration`].

What *is* checked here is internal and algebraic, and it is stated as such:

| check | result |
|---|---|
| Eq (14a)'s `α = ln 2` makes `ζ = 1` the median | exact, [`tests::alpha_ln2_makes_unit_action_the_median`] |
| Eq (11) telescopes at constant `T` | 1·10⁻¹² over 30 steps |
| `Q = 556 kJ/mol` reproduces Benz's stated 1600–2200 °C range as ~4 decades in `k` | 3.76 decades |
| `k ∝ 1/d_o` | exact |

An order-of-magnitude comparison against Fig. 9 is recorded in the units
doc; it is **not** a verification, because Fig. 9's caption states neither
the burnup nor the irradiation history.

```rust
pub mod decomposition { /* ... */ }
```

### Types

#### Enum `DecompositionCalibration`

Which of the report's two empirical fits of Eq (13) to use.

The two are not small perturbations of one another — `β` is 0.88 against
4 — so they disagree by orders of magnitude away from `ζ ≈ 1`. Which one
applies is a property of the *experiment* being modelled (a loose particle
in a furnace, or one embedded in a fuel sphere), not a fitting knob.

```rust
pub enum DecompositionCalibration {
    LooseParticles,
    ParticlesInSphere,
}
```

##### Variants

###### `LooseParticles`

**Eq (14a)** — loose particles. Ramp tests to 2500 °C on loose
`UO₂`-TRISO irradiated in DR-S6 (Goodin et al. 1985).
`α = 0.693 (= ln 2)`, `β = 0.88`.

###### `ParticlesInSphere`

**Eq (14b)** — particles embedded in a fuel sphere (Schenk 1984, AVR
GO2). `α = 0.0001`, `β = 4`. The report states the result is the same
for irradiated and unirradiated elements.

##### Implementations

###### Methods

- ```rust
  pub const fn alpha(self: Self) -> f64 { /* ... */ }
  ```
  `α` of Eq (13).

- ```rust
  pub const fn beta(self: Self) -> f64 { /* ... */ }
  ```
  `β` of Eq (13).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> DecompositionCalibration { /* ... */ }
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
    fn eq(self: &Self, other: &DecompositionCalibration) -> bool { /* ... */ }
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

#### Function `decomposition_rate_constant`

**Eq (12)** — the decomposition rate constant `k(T_m)` \[s⁻¹\]
(page -496-).

```text
k(T_m) = (375/d_o)·exp(−556000/(R·T_m))
```

- `initial_thickness` — `d_o`. The **initial** thickness, not `d_act`:
  Eq (12) is written against `d_o` and the report does not couple
  decomposition to the volume-corrosion thinning of Eq (7).
- `mean_temperature` — `T_m`, the step's mean temperature, in kelvin via
  `uom`.

Returns zero at non-positive temperature or thickness rather than a NaN or
an infinity — nothing decomposes at absolute zero, and a vanished layer
has no rate left to define.

```rust
pub fn decomposition_rate_constant(initial_thickness: uom::si::f64::Length, mean_temperature: uom::si::f64::ThermodynamicTemperature) -> uom::si::f64::Frequency { /* ... */ }
```

#### Function `advance_action_integral`

**Eq (11)** — advance the action integral `ζ` across one time step
(page -496-):

```text
ζ(t₂) = ζ(t₁) + k(T_m)·(t₂ − t₁)
```

`ζ` is dimensionless (`s⁻¹ × s`) and starts at zero. It is carried
forward rather than recomputed from the total elapsed time for the same
reason `FKOR` is in [`super::corrosion`]: each step must contribute at its
own mean temperature, and `k` spans decades over a transient.

```rust
pub fn advance_action_integral(previous: uom::si::f64::Ratio, initial_thickness: uom::si::f64::Length, mean_temperature: uom::si::f64::ThermodynamicTemperature, step: uom::si::f64::Time) -> uom::si::f64::Ratio { /* ... */ }
```

#### Function `thermal_decomposition_failure_fraction`

**Eq (13)** — the failed fraction from thermal decomposition (page -496-).

```text
φ₂(t,T) = 1 − exp(−α·ζ^β)
```

Evaluated **directly** from the running `ζ`, never accumulated from
increments: `ζ` already carries the whole temperature–time history
(page -483-).

The form is chosen so that `φ₂ ≤ 1` for any `ζ`. Returns zero for
`ζ ≤ 0`, where `ζ^β` is not defined for the fractional `β` of Eq (14a).

```rust
pub fn thermal_decomposition_failure_fraction(action_integral: uom::si::f64::Ratio, calibration: DecompositionCalibration) -> uom::si::f64::Ratio { /* ... */ }
```

#### Function `thermal_decomposition_failure_fraction_with`

[`thermal_decomposition_failure_fraction`] with an explicit `α` and `β`.

The report states these "must be empirically determined" and gives two
fits; a third measurement would come in here rather than by editing the
enum.

```rust
pub fn thermal_decomposition_failure_fraction_with(action_integral: uom::si::f64::Ratio, alpha: f64, beta: f64) -> uom::si::f64::Ratio { /* ... */ }
```

### Constants and Statics

#### Constant `DECOMPOSITION_ACTIVATION_J_PER_MOL`

The activation energy `Q` of SiC decomposition, J/mol (page -495-,
Benz 1982, 63 specimens decomposed between 1600 °C and 2200 °C).

```rust
pub const DECOMPOSITION_ACTIVATION_J_PER_MOL: f64 = 556_000.0;
```

#### Constant `DECOMPOSITION_FRONT_VELOCITY_M_PER_S`

Eq (12)'s numerator, \[m/s\] — the `k_o` of the page -495- Arrhenius
expressed as a front velocity, so that `k_o = 375/d_o` comes out in s⁻¹.

See the module docs: the unit is not printed in the report and this is the
only reading that balances.

```rust
pub const DECOMPOSITION_FRONT_VELOCITY_M_PER_S: f64 = 375.0;
```

## Module `diffusion`

**The reduced diffusion coefficient `D_S`** for fission gases in the
particle kernel (page -487-).

`D_S = D_eff / r_o^2`, in s^-1 — the Booth equivalent-sphere diffusion
coefficient divided by the equivalent sphere's radius squared, which is
the form the release function needs. It feeds the dimensionless times
`tau_i = D_S(T_B)*t_B` and `tau_a = D_S(T)*t`, and from there the Booth
series, `F_d`, and Eq (3)'s pressure.

Two correlations, one per kernel type, both printed without equation
numbers on page -487- and cited by page.

# Verification against Fig. 2 (page -487-)

The figure plots both correlations, so it is a verification target rather
than a data source. Checked 2026-09-24 against a digitisation of it:

| curve | nominal `F_b` | `F_b` recovered from the fit | mean abs err in `log10(D_S)` |
|---|---|---|---|
| 1 % FIMA | 0.01 | 0.0086 | 0.021 |
| 5 % FIMA | 0.05 | 0.0494 | 0.035 |
| 10 % FIMA | 0.10 | 0.0950 | 0.013 |
| 15 % FIMA | 0.15 | 0.1456 | 0.023 |

Each (Th,U)O2 curve recovers **its own printed label** from a blind fit,
which is a stronger statement than the residuals: it says the burnup term
`3.24/(1 + 0.11/F_b)` is right in form and not only in magnitude. The 1 %
curve is the loosest (−13.6 % in `F_b`) and that is expected — the term is
most sensitive to `F_b` where `F_b` is smallest.

# The UO2 correlation disagrees with the report's own figure

**This is a defect in the source, recorded rather than resolved.** The
printed Horsley correlation sits *above* the figure's dashed UO2 curve
everywhere, by a factor that falls monotonically with temperature:

| `10^4/T` | figure | equation | equation / figure |
|---|---|---|---|
| 3.19 | 10^−5.69 | 10^−4.89 | **6.4×** |
| 5.59 | 10^−7.34 | 10^−6.84 | 3.2× |
| 7.21 | 10^−8.46 | 10^−8.16 | 2.0× |
| 8.59 | 10^−9.40 | 10^−9.28 | 1.3× |

A blind fit to the plotted curve gives a slope of −0.6875 against the
equation's −0.8116, so it is a **slope** disagreement, not an offset —
the two are not reconcilable by a units or a decade error. The
transcription was checked against the page image directly, and the
equation reads `log DS = −2.30 − 0.8116·10^4/T` as implemented.

[`reduced_diffusion_coefficient`] implements **the equation**, because for
a code reconstruction the equation is the specification and the figure is
illustrative. Anyone comparing against Fig. 2 should expect the offset
above and should not "fix" it by tuning.

```rust
pub mod diffusion { /* ... */ }
```

### Types

#### Enum `KernelKind`

Which kernel the correlation is for. Closed set, enum-dispatched per the
workspace Rust design rules.

```rust
pub enum KernelKind {
    ThoriumUraniumOxide,
    UraniumOxide,
}
```

##### Variants

###### `ThoriumUraniumOxide`

`(Th,U)O2` — Myers 1977. Burnup-dependent.

###### `UraniumOxide`

`UO2` — Horsley 1976. The report states this is **also used for UCO**
(page -487-). No burnup dependence.

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
    fn clone(self: &Self) -> KernelKind { /* ... */ }
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
    fn eq(self: &Self, other: &KernelKind) -> bool { /* ... */ }
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

#### Function `reduced_diffusion_coefficient`

The reduced diffusion coefficient `D_S` \[s^-1\] (page -487-).

```text
(Th,U)O2   log10(D_S) = -5.94 + 3.24/(1 + 0.11/F_b) - 0.5460e4/T
UO2, UCO   log10(D_S) = -2.30 - 0.8116e4/T
```

- `temperature` — `T`, in kelvin via `uom`. (The report writes `10^4/T`
  throughout and its symbol list gives temperatures in degC; kelvin is
  established in `docs/panama-i-units-and-open-questions.md`.)
- `burnup` — `F_b`, heavy-metal burnup in FIMA as a **fraction**, not a
  percent. Fig. 2's curves are labelled `1 % FIMA` … `15 % FIMA`, i.e.
  `F_b` = 0.01 … 0.15, and those labels are recovered from the figure by
  [`tests::the_thorium_curves_recover_their_own_burnup_labels`]. Ignored
  for [`KernelKind::UraniumOxide`].

Returns zero at non-positive temperature rather than a NaN or an infinity:
`10^4/T` is undefined there and a zero diffusion coefficient is the
physically right limit (nothing diffuses).

```rust
pub fn reduced_diffusion_coefficient(kernel: KernelKind, temperature: uom::si::f64::ThermodynamicTemperature, burnup: uom::si::f64::Ratio) -> uom::si::f64::Frequency { /* ... */ }
```

## Module `geometry`

**The SiC layer's geometry** — Eq (2)'s `r`, `d_o` and `d_act` (page -484-).

Split out because these are the figure-independent facts about a particle:
every other module here consumes them and none of them depends on the rest.

```rust
pub mod geometry { /* ... */ }
```

### Types

#### Struct `SicLayer`

The SiC layer's geometry, from which Eq (2)'s `r` and `d_o` are derived.

Both radii are to the SiC layer itself — `r_i` its inner surface (the outer
surface of the inner PyC) and `r_a` its outer surface.

```rust
pub struct SicLayer {
    pub inner_radius: uom::si::f64::Length,
    pub outer_radius: uom::si::f64::Length,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `inner_radius` | `uom::si::f64::Length` | Inner radius `r_i` of the SiC layer. |
| `outer_radius` | `uom::si::f64::Length` | Outer radius `r_a` of the SiC layer. |

##### Implementations

###### Methods

- ```rust
  pub fn mean_radius(self: &Self) -> Length { /* ... */ }
  ```
  The report's **average radius** `r = (0.5·(r_a³ + r_i³))^(1/3)`

- ```rust
  pub fn initial_thickness(self: &Self) -> Length { /* ... */ }
  ```
  The original layer thickness `d_o = r_a − r_i` (page -484-).

- ```rust
  pub fn actual_thickness(self: &Self, corrosion_rate: Velocity, elapsed: Time) -> Length { /* ... */ }
  ```
  **Eq (7)** — the actual thickness after volume corrosion (page -492-):

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> SicLayer { /* ... */ }
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
    fn eq(self: &Self, other: &SicLayer) -> bool { /* ... */ }
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
## Module `grain_boundary`

**Eqs (10b)/(10c)** — strength loss of the SiC layer by grain-boundary
corrosion (page -495-).

```text
m  = m_o·(0.44 + 0.56·exp(−η̇·t))                                (10b)
η̇  = 0.565·exp(−187400/(R·T))   [s⁻¹]                            (10c)
```

Fission products attack the SiC grain boundaries, widening the Weibull
distribution — `m` falls toward `0.44·m_o`, so the *scatter* in strength
grows while the median `σ_o` is untouched. Since Eq (1) raises
`σ_t/σ_o` to the power `m`, a smaller `m` lifts the failure fraction
dramatically in the `σ_t ≪ σ_o` regime where a TRISO particle actually
sits: Figs. 7 and 8 show it worth **one to two decades** in the release
fraction at 1600 °C.

# This is OFF by default, and that is the report's own default

Page -495- states plainly that grain-boundary corrosion "is not generally
taken into consideration, although it can be selected by setting one switch
per input", and page -499- that **Eq (10a) (`m = m_o`) is normally used in
place of (10b)**. Both Fig. 7 and Fig. 8 carry `η̇(T) ≡ 0` in their
captions while plotting a "with grain boundary corrosion" curve alongside
for comparison.

So [`GrainBoundaryCorrosion::Disabled`] is the default in
[`super::history`], and that is **not** an instance of the workspace's
"correct physics is the default setting" rule being waived: the rule is
about physics the model supplies, and here the source model's own
specified default is off. Turning it on silently would mean this
reconstruction stopped reproducing the report. The enum is visible at the
call site so the choice is made, not inherited.

# `η̇·t` for a varying history is an extension, not the report

Eq (10b) prints `exp(−η̇·t)` with a single rate and a single time, i.e. it
is written for an isothermal hold. For a varying temperature history the
natural discrete analogue is to accumulate `∫η̇ dt` exactly as Eq (11)
accumulates `∫k dt` — and that is what
[`advance_grain_boundary_exposure`] does. **The report does not state
this**; it is this implementation's reading, chosen for consistency with
Eq (11) and because the alternative (evaluating `η̇` at the current
temperature and multiplying by the total elapsed time) would retroactively
apply the latest temperature to the whole history. It is recorded as an
open item in `docs/panama-i-units-and-open-questions.md`, and it collapses
to the printed form for an isothermal hold — pinned by
[`tests::the_exposure_reduces_to_the_printed_form_when_isothermal`].

# Verification status

**Not verified against any output of the report.** Figs. 7 and 8 plot the
"with grain boundary corrosion" curve, but their captions state neither the
particle geometry, the kernel volume nor the buffer void volume, so the
absolute release fraction cannot be reproduced without inventing three
inputs. What can be said is checked and no more: the qualitative direction
(corrosion raises the failure fraction), the floor at `0.44·m_o`, and the
`0.565`/`187400` Arrhenius as transcribed.

```rust
pub mod grain_boundary { /* ... */ }
```

### Types

#### Enum `GrainBoundaryCorrosion`

Whether the grain-boundary corrosion of Eqs (10b)/(10c) is applied.

[`Disabled`](Self::Disabled) is the default, matching the report's own
switch (page -495-) and its stated normal use of Eq (10a), `m = m_o`
(page -499-).

```rust
pub enum GrainBoundaryCorrosion {
    Disabled,
    Enabled,
}
```

##### Variants

###### `Disabled`

Eq (10a): `m = m_o`. The report's default, and this crate's.

###### `Enabled`

Eqs (10b)/(10c): `m` decays toward `0.44·m_o`.

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
    fn clone(self: &Self) -> GrainBoundaryCorrosion { /* ... */ }
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
    fn default() -> GrainBoundaryCorrosion { /* ... */ }
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
    fn eq(self: &Self, other: &GrainBoundaryCorrosion) -> bool { /* ... */ }
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

#### Function `grain_boundary_corrosion_rate`

**Eq (10c)** — the SiC grain-boundary corrosion rate `η̇` \[s⁻¹\]
(page -495-).

```text
η̇ = 0.565·exp(−187400/(R·T))
```

Returns zero at non-positive temperature rather than a NaN.

```rust
pub fn grain_boundary_corrosion_rate(temperature: uom::si::f64::ThermodynamicTemperature) -> uom::si::f64::Frequency { /* ... */ }
```

#### Function `advance_grain_boundary_exposure`

Advance the accumulated grain-boundary exposure `∫η̇ dt` across one time
step at its own mean temperature.

Dimensionless, starting at zero, and feeding the `η̇·t` slot of Eq (10b).

**This accumulation is an extension of the printed equation, not the
printed equation.** Eq (10b) writes a single `η̇·t`; see the module docs.

```rust
pub fn advance_grain_boundary_exposure(previous: uom::si::f64::Ratio, mean_temperature: uom::si::f64::ThermodynamicTemperature, step: uom::si::f64::Time) -> uom::si::f64::Ratio { /* ... */ }
```

#### Function `corroded_weibull_modulus`

**Eq (10b)** — the Weibull modulus after grain-boundary corrosion
(page -495-):

```text
m = m_o·(0.44 + 0.56·exp(−η̇·t))
```

- `end_of_irradiation_modulus` — `m_o`, from
  [`super::irradiated_weibull_modulus`].
- `exposure` — the accumulated `∫η̇ dt` from
  [`advance_grain_boundary_exposure`], or simply `η̇·t` for an isothermal
  hold.

At zero exposure this returns `m_o` exactly, so Eq (10a) is the `t = 0`
limit of Eq (10b) rather than a separate branch.

```rust
pub fn corroded_weibull_modulus(end_of_irradiation_modulus: f64, exposure: uom::si::f64::Ratio) -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `GRAIN_BOUNDARY_PREFACTOR_PER_S`

Eq (10c)'s pre-factor, \[s⁻¹\] (page -495-).

```rust
pub const GRAIN_BOUNDARY_PREFACTOR_PER_S: f64 = 0.565;
```

#### Constant `GRAIN_BOUNDARY_ACTIVATION_J_PER_MOL`

Eq (10c)'s activation energy, J/mol (page -495-).

```rust
pub const GRAIN_BOUNDARY_ACTIVATION_J_PER_MOL: f64 = 187_400.0;
```

#### Constant `RESIDUAL_MODULUS_FRACTION`

The asymptotic floor of Eq (10b): `m → 0.44·m_o` as `η̇·t → ∞`.

```rust
pub const RESIDUAL_MODULUS_FRACTION: f64 = 0.44;
```

## Module `history`

**The time-stepping driver** — §3.1, pages -482- and -483-.

Everything else in [`super`] is one equation. This is the assembly: it
walks a temperature history interval by interval, carries the three
history variables forward, and reports `φ₁`, `φ₂`, `φ_total` and the
three rates `φ̇` (page -511-, all `s⁻¹`) at every step.

```text
per interval [t₁, t₂] at mean temperature T_m:

  FKOR(t₂) = FKOR(t₁) + v̇(T_m)·Δt/d_o                       (p-492)
  ζ(t₂)    = ζ(t₁)    + k(T_m)·Δt                            (11)

  φ₁ ← φ₁ + max(0, φ₁(t₂,T_m) − φ₁(t₁,T_m))                  (p-482)
  φ₂  = 1 − exp(−α·ζ(t₂)^β)                                  (13)
  φ_total = 1 − (1−φ_o)(1−φ₁)(1−φ₂)                          (p-480)
```

# The two mechanisms are stepped differently, and that is the report's

`φ₁` is **accumulated from positive increments**: page -482- forms
`φ₁(t₂,T_m) − φ₁(t₁,T_m)` — both ends evaluated at *this* interval's mean
temperature — and adds it to the running total only if it is positive. So
`φ₁` is monotone by construction: a particle that has burst does not
un-burst when the transient cools.

`φ₂` is **not** accumulated. Page -483- is explicit: `ζ` carries the whole
history, and `φ₂(t₂)` is read straight off Eq (13) at that `ζ`. Only the
*rate* `φ̇₂` is formed by differencing.

Getting this backwards is invisible on a monotone heat-up — the two agree
exactly there — and wrong on anything that cools, which is every reactor
transient the report goes on to compute (Figs. 10–13).
[`tests::the_two_mechanisms_are_stepped_differently`] pins the
distinction on a history that cools.

# Verification — methodology and results

## 1. Step-size independence at constant temperature (the report's own claim)

Page -482-: "At a constant temperature, the length of the time interval
does not influence the computed result." That is a falsifiable statement
about *this algorithm*, independent of any figure, and it is the sharpest
check available on the stepping itself — a driver that recomputed `FKOR`
or `ζ` from total elapsed time, or that accumulated `φ₂`, would fail it.

**Result, 2026-09-24:** 300 h at 1600 °C taken in 1, 12, 300 and 3000
steps gives `φ_total` agreeing to **2·10⁻¹² relative** (`FKOR` and `ζ` to
better than 10⁻¹²; the extra digit is the `f(τ)` sum). Pinned by
[`tests::the_step_length_does_not_matter_at_constant_temperature`].

## 2. Fig. 6 — the first end-to-end check on the whole chain

Fig. 6 (page -500-) is PANAMA's own output for eight SiC varieties at
1600 °C over ~250 h, and it is the only figure in the report that exercises
Eqs (1), (8a) and (9a) across a *family* of particles. Page -498- states
its basis explicitly: the Table 1 "after irradiation" values, computed at
`T_B = 1000 °C` and `Γ = 1·10²⁵ m⁻² EDN`. Its caption states the kernel
((Th,U)O₂) and the temperature and nothing else — no geometry, no `V_k`,
no `V_f`, no `F_b`, no `t_B` — so the **absolute** failure fraction cannot
be reproduced without inventing four inputs, and inventing them is exactly
the tuning this workspace forbids.

What the figure *can* verify, with no invented input at all, is the
consequence of `σ_t(t)` being **common to all eight curves**. Only `σ_o`
and `m` differ between varieties, so inverting Eq (1) on each digitised
curve,

```text
σ_t^(i)(t) = σ_o,i · ( −ln(1 − φ_i(t)) / ln2 )^(1/m_i)
```

must return the **same** `σ_t(t)` from all eight. That is an eight-fold
over-determined test of Eq (1) together with Eqs (8a)/(9a).

**Results, 2026-09-24** (467 digitised points, maintainer; 121 of them lie
below the figure's plotted 10⁻⁶ floor and are excluded from the inversion):

| quantity | measured |
|---|---|
| rank order of the eight curves | **8/8 reproduced** |
| spread top-to-bottom at 248 h | 4.79 decades |
| recovered common `σ_t` | 132 MPa at 130 h → 163 MPa at 248 h |
| relative s.d. of `σ_t` across varieties | **9.0 % (130 h) … 11.2 % (248 h)** |
| per-curve residual in `log₁₀ φ` at one common `σ_t` | **−0.37 … +0.39**, mean \|·\| **0.23** |

Reproducing the ordering and the 4.8-decade spread of eight curves from
one common stress, to ±0.4 decades, is a real success for Eq (1) and the
Table 1 degradation. But the residual is **systematic, not random**: it
runs monotonically with `m_oo`, from −0.37 decades at `m_oo = 5.0` to
+0.39 at `m_oo = 8.5`. Two hypotheses were tested and neither removes it:

- **Fluence.** Removing the Eq (8a)/(9a) degradation entirely (`Γ → 0`)
  halves the scatter, from 10.1 % to 5.4 % relative s.d. But page -498-
  states Fig. 6's basis is `Γ = 1·10²⁵`, so this is a *disagreement with
  the figure*, not a licence to change the input — and `Γ` was not
  changed.
- **The plotted floor.** Restricting to points inside the figure's own
  10⁻⁶ … 10 axis leaves the trend intact (9.0 → 11.2 % against
  9.3 → 11.3 % unrestricted).

Recorded with numbers in `docs/panama-i-units-and-open-questions.md`.
Pinned by [`tests::figure_6_recovers_one_common_stress_history`].

## 3. Fig. 7 — the staged history, and the first VALIDATION case

Fig. 7 (page -501-) is the FRJ2-K11/03 heating experiment: **measured**
⁸⁵Kr release alongside two PANAMA curves. Unlike every check above it has
a complete stated input set — `σ_oo = 600 MPa`, `m_oo = 6`,
`T_B = 1160 °C`, `t_B = 260 FPD`, `F_B = 0.09 FIMA`,
`Γ = 0.05·10²⁵ m⁻² EDN`, `η̇(T) ≡ 0` — and page -498- states the staging
outright: **100 h at 1400 °C, then 100 h at 1500 °C, then 1600 °C** to
1000 h. Nothing here was inferred from the curve's slope.

### The load-bearing assumption being inherited

PANAMA computes a particle **failure** fraction; Fig. 7 plots a ⁸⁵Kr
**release** fraction, and the report puts them on one axis. That equates
the two — a failed particle releases its whole krypton inventory. **This
is an assumption inherited from the report, not derived here.** If a
comparison matches in shape but sits at a constant offset, it is the first
suspect.

### Code-to-code: does this implementation reproduce PANAMA's own curve?

The absolute level still needs the unstated geometry, so the comparison is
made on `σ_t` recovered from the `Without Grain Boundary Corrosion` curve
(the right comparator, since `η̇ ≡ 0`) against the chain's own `σ_t`, with
**one** free scale — the geometry aggregate `r/(2·d_o·(V_f/V_k))`. No
physics constant is adjusted.

**Results, 2026-09-24** (71 digitised points):

| window | relative s.d. of `σ_t^PANAMA / σ_t^chain` | max/min |
|---|---|---|
| **0–300 h, all three stages** | **4.9 %** | 1.20 |
| 300–1000 h | 15.4 % | 1.77 |
| whole run | 21.9 % | 2.17 |

Per stage the ratio is 0.1561 (1400 °C), 0.1473 (1500 °C), 0.1608
(1600 °C, first 100 h) — within ±4.5 % of each other. **The staging is
reproduced**: the temperature dependence entering through Eq (5c)'s `OPF`,
`D_S(T)`, `v̇(T)` and Eq (3)'s explicit `T` all land together across two
step changes.

**Then it drifts, and that is the finding.** By 977 h PANAMA's implied
`σ_t` is 282 MPa; with the single scale fixed over 0–300 h the chain
predicts 148 MPa — a factor **1.90**.
Diagnosis: PANAMA's curve follows `φ ∝ t^3.21` at late times, i.e.
`σ_t ∝ t^0.54`, whereas in the chain `F_d` has saturated (0.980 at 296 h,
0.9999 at 977 h) and `OPF` is constant at fixed temperature, leaving only
`FKOR` — which rises **4 %** over the last 700 h. Something in PANAMA
keeps the pressure climbing as `√t` after the Booth release is over, and
the printed equations do not say what. Recorded as an open item with these
numbers in `docs/panama-i-units-and-open-questions.md`;
[`tests::figure_7_reproduces_the_staged_history_then_drifts`] pins both
halves so neither can be lost.

### Code-to-data: does PANAMA reproduce the experiment?

This part needs no geometry — it compares the nine measured points against
the report's own two curves.

| comparator | mean residual, `log₁₀` | mean \|·\| | worst |
|---|---|---|---|
| `Without Grain Boundary Corrosion` (`η̇ ≡ 0`, the caption's case) | **+0.51** | 0.52 | +1.14 |
| `With Grain Boundary Corrosion` | −1.49 | 1.49 | −1.85 |

So at 1400–1600 °C PANAMA **under**-predicts FRJ2-K11/03 by half a decade
with grain-boundary corrosion off, and over-predicts by 1.5 decades with
it on; the measurement lies between the two, nearer the "without" curve.
That reproduces page -499-'s own reading — that the with-corrosion model
"covers the measured values in a conservative approximation" — and it is a
statement about **PANAMA**, not about this implementation.
[`tests::figure_7_brackets_the_measurement`] records it.

One digitisation label reads `90% FIMA` where the caption says
`9.0 % FIMA` and `F_B = 0.09`; 0.09 is used, and the slip is noted.

```rust
pub mod history { /* ... */ }
```

### Types

#### Enum `OxygenSource`

Where the step's `OPF` comes from — Eqs (5a)–(5e).

An enum rather than a callback, per the workspace's no-trait-objects rule,
and because the report's own set of sources is closed.

```rust
pub enum OxygenSource {
    Uco,
    ThoriumUraniumOxide {
        thorium_to_u235: f64,
        burnup: uom::si::f64::Ratio,
    },
    UraniumOxide {
        irradiation_temperature: uom::si::f64::ThermodynamicTemperature,
        irradiation_time: uom::si::f64::Time,
    },
    Fixed(uom::si::f64::Ratio),
}
```

##### Variants

###### `Uco`

**Eq (5d)** — `UCO`: no oxygen is produced.

###### `ThoriumUraniumOxide`

**Eq (5a)** — `(Th,U)O₂`, from the accident temperature, the
thorium/²³⁵U ratio `N` and the burnup.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `thorium_to_u235` | `f64` | `N`, the thorium/uranium-235 ratio (5 for AVR, 10 for THTR). |
| `burnup` | `uom::si::f64::Ratio` | `F_b`, heavy-metal burnup in FIMA as a fraction. |

###### `UraniumOxide`

**Eq (5c)** — `UO₂` during heating, from the irradiation history.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `irradiation_temperature` | `uom::si::f64::ThermodynamicTemperature` | `T_B`, the particle surface temperature during irradiation. |
| `irradiation_time` | `uom::si::f64::Time` | `t_B`, the irradiation time. Enters Eq (5b) in **seconds**. |

###### `Fixed`

A fixed `OPF` supplied by the caller.

For the cases where the report states the value rather than the
correlation, and for ablating the oxygen term to see what it carries.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `uom::si::f64::Ratio` |  |

##### Implementations

###### Methods

- ```rust
  pub fn oxygen_per_fission(self: Self, accident_temperature: ThermodynamicTemperature) -> Ratio { /* ... */ }
  ```
  The `OPF` for this step's accident temperature.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> OxygenSource { /* ... */ }
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
    fn eq(self: &Self, other: &OxygenSource) -> bool { /* ... */ }
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
#### Struct `ParticleState`

Everything about the particle that does not change during the accident.

All of it is an **input**. The report's figures state some of these and
not others; where a figure does not state one, this crate takes it from
the caller rather than inventing a value — see the module docs on Fig. 6.

```rust
pub struct ParticleState {
    pub layer: super::geometry::SicLayer,
    pub compound: super::molar_volume::KernelCompound,
    pub diffusion_kernel: super::diffusion::KernelKind,
    pub kernel_volume: uom::si::f64::Volume,
    pub free_volume: uom::si::f64::Volume,
    pub burnup: uom::si::f64::Ratio,
    pub stable_gas_yield: uom::si::f64::Ratio,
    pub dimensionless_irradiation_time: uom::si::f64::Ratio,
    pub median_strength: uom::si::f64::Pressure,
    pub weibull_modulus: f64,
    pub oxygen: OxygenSource,
    pub decomposition: super::decomposition::DecompositionCalibration,
    pub grain_boundary: super::grain_boundary::GrainBoundaryCorrosion,
    pub as_manufactured: super::FailureFraction,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `layer` | `super::geometry::SicLayer` | The SiC layer geometry — `r`, `d_o`. |
| `compound` | `super::molar_volume::KernelCompound` | Which compound the kernel is, for Eqs (6a)–(6c). |
| `diffusion_kernel` | `super::diffusion::KernelKind` | Which `D_S` correlation applies (page -487- uses the `UO₂` relation<br>for `UCO` as well, which is why this is a separate choice from<br>[`ParticleState::compound`]). |
| `kernel_volume` | `uom::si::f64::Volume` | `V_k`, the kernel volume. |
| `free_volume` | `uom::si::f64::Volume` | `V_f`, the void volume in the buffer used as free volume (the report<br>takes 50 % of the buffer volume). |
| `burnup` | `uom::si::f64::Ratio` | `F_b`, heavy-metal burnup in FIMA as a fraction. Feeds both Eq (3) and<br>the `(Th,U)O₂` `D_S` correlation. |
| `stable_gas_yield` | `uom::si::f64::Ratio` | `F_f`, the stable fission-gas yield. The report's value is<br>[`super::STABLE_FISSION_GAS_YIELD`] = 0.31. |
| `dimensionless_irradiation_time` | `uom::si::f64::Ratio` | `τ_i = D_S(T_B)·t_B`, the dimensionless irradiation time (page -486-).<br><br>Supplied rather than derived, because it needs `T_B` *and* `t_B` and<br>the report's figures generally give neither. [`irradiation_tau`] builds<br>it when they are known. |
| `median_strength` | `uom::si::f64::Pressure` | `σ_o`, the SiC median strength at the end of irradiation — Eq (8a),<br>[`super::irradiated_strength`]. |
| `weibull_modulus` | `f64` | `m_o`, the Weibull modulus at the end of irradiation — Eq (9a),<br>[`super::irradiated_weibull_modulus`]. |
| `oxygen` | `OxygenSource` | Where `OPF` comes from. |
| `decomposition` | `super::decomposition::DecompositionCalibration` | Which fit of Eq (13) applies — Eq (14a) or (14b). |
| `grain_boundary` | `super::grain_boundary::GrainBoundaryCorrosion` | Whether Eqs (10b)/(10c) are applied. **Off by default**, as in the<br>report (page -495-). |
| `as_manufactured` | `super::FailureFraction` | `φ_o`, the as-manufactured defective fraction. The report's own runs<br>use zero; see [`super::AS_MANUFACTURED_TARGET`]. |

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
    fn clone(self: &Self) -> ParticleState { /* ... */ }
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
    fn eq(self: &Self, other: &ParticleState) -> bool { /* ... */ }
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
#### Struct `AccidentStep`

One interval of the accident history.

```rust
pub struct AccidentStep {
    pub duration: uom::si::f64::Time,
    pub mean_temperature: uom::si::f64::ThermodynamicTemperature,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `duration` | `uom::si::f64::Time` | `t₂ − t₁`. |
| `mean_temperature` | `uom::si::f64::ThermodynamicTemperature` | `T_m`, the mean temperature prevailing over the interval (page -482-). |

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
    fn clone(self: &Self) -> AccidentStep { /* ... */ }
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
    fn eq(self: &Self, other: &AccidentStep) -> bool { /* ... */ }
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
#### Struct `FailureProgress`

The state carried across intervals, plus the reported quantities.

```rust
pub struct FailureProgress {
    pub elapsed: uom::si::f64::Time,
    pub thinning_factor: uom::si::f64::Ratio,
    pub action_integral: uom::si::f64::Ratio,
    pub grain_boundary_exposure: uom::si::f64::Ratio,
    pub pressure_vessel: super::FailureFraction,
    pub thermal_decomposition: super::FailureFraction,
    pub total: super::FailureFraction,
    pub pressure_vessel_rate: uom::si::f64::Frequency,
    pub thermal_decomposition_rate: uom::si::f64::Frequency,
    pub total_rate: uom::si::f64::Frequency,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `elapsed` | `uom::si::f64::Time` | Accident time elapsed. |
| `thinning_factor` | `uom::si::f64::Ratio` | `FKOR`, the SiC thinning factor (page -492-). Starts at 1. |
| `action_integral` | `uom::si::f64::Ratio` | `ζ`, the action integral (Eq 11). Starts at 0. |
| `grain_boundary_exposure` | `uom::si::f64::Ratio` | `∫η̇ dt`, the grain-boundary exposure. Starts at 0, and stays there<br>unless [`GrainBoundaryCorrosion::Enabled`]. |
| `pressure_vessel` | `super::FailureFraction` | `φ₁`, accumulated from positive increments (page -482-). |
| `thermal_decomposition` | `super::FailureFraction` | `φ₂`, read directly off Eq (13) at the current `ζ` (page -483-). |
| `total` | `super::FailureFraction` | `φ_total` (page -480-). |
| `pressure_vessel_rate` | `uom::si::f64::Frequency` | `φ̇₁ = Δφ₁/Δt` over the interval just taken \[s⁻¹\] (page -511-). |
| `thermal_decomposition_rate` | `uom::si::f64::Frequency` | `φ̇₂ = Δφ₂/Δt` \[s⁻¹\]. |
| `total_rate` | `uom::si::f64::Frequency` | `φ̇_gesamt = Δφ_total/Δt` \[s⁻¹\]. |

##### Implementations

###### Methods

- ```rust
  pub fn in_service_failure_fraction(self: &Self) -> FailureFraction { /* ... */ }
  ```
  The **in-service** failure fraction: `φ₁` and `φ₂` combined, with the

- ```rust
  pub fn at_start(end_of_irradiation_phi_1: FailureFraction, as_manufactured: FailureFraction) -> Self { /* ... */ }
  ```
  The state at `t = 0`: an uncorroded layer, no action integral, and

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> FailureProgress { /* ... */ }
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
    fn eq(self: &Self, other: &FailureProgress) -> bool { /* ... */ }
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
#### Struct `AccidentHistory`

The accident driver: a [`ParticleState`] plus the running
[`FailureProgress`].

```rust
pub struct AccidentHistory {
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
  pub fn new(particle: ParticleState, end_of_irradiation_phi_1: FailureFraction) -> Self { /* ... */ }
  ```
  Start an accident from the end-of-irradiation state.

- ```rust
  pub fn particle(self: &Self) -> ParticleState { /* ... */ }
  ```
  The particle this history is running.

- ```rust
  pub fn progress(self: &Self) -> FailureProgress { /* ... */ }
  ```
  The current state.

- ```rust
  pub fn pressure_vessel_failure_at(self: &Self, elapsed: Time, thinning_factor: Ratio, temperature: ThermodynamicTemperature) -> FailureFraction { /* ... */ }
  ```
  `φ₁(t, T_m)` for a given elapsed accident time and thinning factor —

- ```rust
  pub fn pressure_at(self: &Self, elapsed: Time, temperature: ThermodynamicTemperature) -> Pressure { /* ... */ }
  ```
  **Eq (3)** — the internal gas pressure at an instant, with `F_d` from

- ```rust
  pub fn step(self: &mut Self, step: AccidentStep) -> FailureProgress { /* ... */ }
  ```
  Advance one interval (pages -482-, -483-, -492-, -496-) and return the

- ```rust
  pub fn run(self: &mut Self, steps: &[AccidentStep]) -> FailureProgress { /* ... */ }
  ```
  Walk a whole temperature history and return the final state.

- ```rust
  pub fn run_isothermal(self: &mut Self, temperature: ThermodynamicTemperature, total: Time, n: usize) -> FailureProgress { /* ... */ }
  ```
  Walk an isothermal hold split into `n` equal intervals — the shape

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> AccidentHistory { /* ... */ }
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
    fn eq(self: &Self, other: &AccidentHistory) -> bool { /* ... */ }
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

#### Function `irradiation_tau`

`τ_i = D_S(T_B)·t_B` (page -486-), for the cases where `T_B` and `t_B` are
both known.

```rust
pub fn irradiation_tau(kernel: super::diffusion::KernelKind, irradiation_temperature: uom::si::f64::ThermodynamicTemperature, irradiation_time: uom::si::f64::Time, burnup: uom::si::f64::Ratio) -> uom::si::f64::Ratio { /* ... */ }
```

## Module `htr10`

**HTR-10 applied to boon-lay fuel failure (the PANAMA-I formulas) — an
EXTRAPOLATION, reported as one.** Every number in this module is computed by
boon-lay fuel failure, not by PANAMA: the PANAMA code was never run on
HTR-10, and this project does not have its source.

PANAMA-I was built and validated for **German** TRISO: its reactor cases
are HTR-Module and HTR-500, its heating experiments are FRJ2-K11/03 and
AVR GO 2, and page -479- claims good agreement only over 1600–2500 °C.
HTR-10's fuel is German-lineage — a 60 mm pebble with a 500 µm UO₂ kernel
and 35 µm SiC — which is why applying the model to it is *defensible*.
**It is not a validated application, and nothing here should be quoted as
one.** Two of the inputs are not published for HTR-10 at all and are taken
from the report's own HTR-Module column, by name.

# What is HTR-10's, what is derived, and what is a stand-in

| Input | Value | Where from |
|---|---|---|
| SiC layer `r_i`/`r_a` | 380 / 415 µm | IAEA-TECDOC-1382 pt 2 Table 4-17, via `tampines::pebble_bed::triso::TrisoParticle::htr10` |
| kernel radius | 250 µm | same |
| `V_k` | kernel sphere | **derived** from the above |
| `V_f` | ½ × buffer shell | **derived**; the report's own definition (page -485-) |
| `F_b` | **0.0851 FIMA** | **derived** from the published 80 000 MWd/t — see [`BURNUP_FIMA`] |
| `t_B` | **1080 FPD** | **derived** from 10 MW over 27 000 × 5 g HM — see [`RESIDENCE_FULL_POWER_DAYS`] |
| kernel compound | UO₂, 17 % enriched | IAEA-TECDOC-1382 pt 2 §4 design table |
| `T_B` | **an input** | HTR-10 publishes a *maximum* fuel temperature, not an average |
| `σ_oo`/`m_oo` | **834 MPa / 8.02** | **STAND-IN**: EO 1607, the variety the report's own HTR-Module runs use (footnote 1, page -503-) |
| `Γ` | **1.4·10²⁵ m⁻² EDN** | **STAND-IN**: the report's HTR-Module/HTR-500 value (Table 2, page -504-) |

The two stand-ins are named rather than absorbed. HTR-Module's reference
particle is a **500 µm UO₂-LTI-TRISO** (footnote 1, page -503-), i.e. the
same kernel diameter as HTR-10's, at 0.08 FIMA against HTR-10's 0.0851 and
1020 FPD against 1080 — so it is the closest published case there is. That
is an argument for the stand-in being *reasonable*, not for it being
HTR-10's fuel. Picking a Table 1 variety and calling it HTR-10 without
saying so would be putting one reactor's fuel quality under another's name.

# Result 1 — normal operation: boon-lay fuel failure must NOT replace the `f_inc` placeholder

`htgr_sim_v1`'s `TRISO_ATOPS_REFERENCE_FAILURE_FRACTIONS` carries
`f_inc = 3·10⁻⁵`, documented there as a TRISO-ATOPS reference value rather
than HTR-10 data, and release scales linearly in it. The obvious move is to
compute `f_inc` with boon-lay fuel failure instead. **That would be wrong, and by a very
large margin.**

`φ₁` at the end of irradiation, over the whole plausible fuel-temperature
band (measured 2026-09-24):

| `T_B` | `OPF` | `F_d` | `σ_t` | `φ₁` |
|---|---|---|---|---|
| 700 °C | 1.3·10⁻³ | 0.101 | 6.9 MPa | **2.8·10⁻¹⁵** |
| 776 °C (HTR-Module's average) | 5.7·10⁻³ | 0.196 | 15.2 MPa | **1.2·10⁻¹²** |
| 900 °C | 4.1·10⁻² | 0.452 | 46.3 MPa | **5.0·10⁻⁹** |
| 1000 °C | 0.153 | 0.707 | 103 MPa | **1.6·10⁻⁶** |

So the pressure-vessel mechanism contributes between **10⁻¹⁵ and 10⁻⁶**,
against a placeholder of 3·10⁻⁵ — seven orders of magnitude at the
best-supported temperature. Substituting the computed value would divide
every activity `htgr_sim_v1` reports by about 10⁷.

**The placeholder is not a pressure-vessel number, and that is the point.**
`3·10⁻⁵` is the same order as the PANAMA-I report's own as-manufactured
target `φ_o = 6·10⁻⁵` (page -480-) — a *manufacturing and irradiation*
defect population, which the PANAMA-I equations take as an **input** and do
not model. The right conclusion is the one already written in that module:
`f_inc` there needs HTR-10 fuel-qualification data, not a better model.
boon-lay fuel failure cannot supply it and this crate must not pretend
otherwise.

# Result 2 — accident: this is where the seam is worth having

Under accident conditions, which is what the PANAMA-I equations are *for*,
the number stops being negligible. 200 h isothermal, `T_B = 776 °C`, measured 2026-09-24:

| accident `T` | `F_d` | `FKOR` | `σ_t` | `φ₁` | `φ₂` (14b) |
|---|---|---|---|---|---|
| 1200 °C | 0.373 | 1.0005 | 50 MPa | 4.53·10⁻⁹ | ~0 |
| 1400 °C | 0.630 | 1.0030 | 103 MPa | 6.85·10⁻⁷ | ~0 |
| **1600 °C** | 0.889 | 1.0119 | 178 MPa | **3.10·10⁻⁵** | 3.4·10⁻¹⁵ |
| 1800 °C | 0.992 | 1.0363 | 262 MPa | 4.46·10⁻⁴ | 3.3·10⁻⁹ |
| 2000 °C | 1.000 | 1.0906 | 370 MPa | 4.79·10⁻³ | 2.78·10⁻⁴ |
| 2200 °C | 1.000 | 1.1953 | 536 MPa | 6.07·10⁻² | **0.977** |

**The 1600 °C figure landing on 3.1·10⁻⁵, beside a 3·10⁻⁵ placeholder, is
a coincidence.** They are different quantities — one is 200 h at HTR-10's
accident temperature limit, the other is an as-manufactured defect fraction
for a different fuel line. Reporting the coincidence as agreement would be
exactly the kind of accident this file exists to avoid.

Two things the table does show, and they are the model's own structure:
`φ₂` overtakes `φ₁` between 2000 and 2200 °C, matching the report's
statement (page -508-) that thermal decomposition governs above ~2000 °C;
and `F_d` saturates by 1800 °C, so above that the growth is all `FKOR` and
`OPF`.

# Not verified

**Nothing here is compared against published HTR-10 data.** The workspace's
local literature gives HTR-10's geometry, burnup, enrichment and power but
**no measured fuel failure fraction, free-uranium fraction or release
fraction** — so the comparison that would make this a validation could not
be made, and is not claimed. The nearest available check is the report's
own statement (page -504-) that HTR-Module depressurised stays below 10⁻⁶
at 200 h; a *flat* 200 h at 1600 °C gives 3.1·10⁻⁵ here, which is an upper
bound on a transient that only briefly reaches its peak, so the two are not
in conflict — but without Fig. 10's temperature history it is not a check
either. Digitising Fig. 10 would make it one.

```rust
pub mod htr10 { /* ... */ }
```

### Modules

## Module `qualification`

**What can be compared against, and what cannot — the fuel-qualification
question, answered.**

[`super`] applies boon-lay fuel failure (the PANAMA-I formulas) to HTR-10
and says plainly that no HTR-10 measured failure fraction exists in reach.
That remains true and is recorded below. (Every `φ₁` below is boon-lay fuel
failure's, not the PANAMA code's.) What *was* found, on 2026-09-24, is
that the **fuel line HTR-10's fuel descends from** has open, quantitative
qualification data sitting in this workspace's own open corpus — and it
had not been used.

# 1. HTR-10 itself: nothing. Confirmed, twice, and not worked around.

Searched and found **empty** of any measured failure fraction, free-uranium
fraction or release fraction:

| Searched | Holds | Failure / free-U / release data |
|---|---|---|
| `jaeri-conf-96-010-htr10-general-design` | general design | **none** |
| `iaea-tecdoc-1382` pt 1 and pt 2 | geometry, burnup, enrichment, power | **none** |
| `li2014-htr10-rmc` | RMC neutronics benchmark | **none** |
| `pnnl-20869-htgr-codes-and-standards` | codes, standards, leak-before-break | **none** |
| `crates/nee_soon/src/htr10_rmc/` | core geometry and materials for `k_eff` | **none**; every material at a flat 300.15 K |
| `crates/changi/src/activity/inventory.rs` | 22-nuclide equilibrium core **inventory** (Bq) | **none** — and its own doc says an inventory is not a source term |
| `docs/htr10-rmc-verification-suite.md` | `k_eff` at twelve loading heights | **none** |

A regex sweep for `free[ -]?uranium|failure fraction|heavy metal
contamination` across all ten documents of the local corpus returned
**zero** matches in every file. `pnnl-20869` mentions fuel failure only in
prose, and marks it as unverified: its sole quantitative-sounding line is a
*manufacturer's* claim of no significant release below 2000 °C, followed by
"if and when this claim can be proven to NRC's satisfaction".

**So a direct code-to-data comparison for HTR-10 is not available, and
nothing here pretends otherwise.** That is the answer, not an obstacle to
be routed around.

# 2. German-lineage fuel: there IS data, and it is open

Kugeler, Nabielek & Buckthorpe (2017) — the JRC (V)HTR-Modul safety volume,
already in this workspace — carries the German LEU UO₂ TRISO qualification
record. Two things make it relevant rather than merely adjacent:

1. **The PANAMA-I model was built for exactly this fuel.** HTA-IB-03/90's cases are
   HTR-Module, HTR-500, FRJ2-K11/03 and AVR GO 2. These are the same
   campaigns.
2. **The lineage is stated in the source, not inferred here.** Page 38:
   "Since then, fabrication processes based on those developed by NUKEM
   have been used to manufacture spherical HTGR fuel elements in China …
   and in South Africa." HTR-10's fuel is downstream of the AVR 21 / proof-
   test production line whose numbers appear below.

**This still does not make HTR-10 numbers.** It makes the *stand-in* for
HTR-10's fuel quality a published measurement with a stated uncertainty,
instead of a round number. That is a real improvement and it is all it is.

# 3. What this settles about `φ_o`, and about `htgr_sim_v1`'s placeholder

The PANAMA-I equations do **not** model `φ_o`, the as-manufactured
defective fraction; it is an input (page -480-), and the report offers `6·10⁻⁵` as a target value.
[`BURN_LEACH_DEFECT_FRACTIONS`] is the measured population that number is
standing for: **8·10⁻⁶ to 49·10⁻⁶ expected, 20·10⁻⁶ to 64·10⁻⁶ at the
one-sided upper 95 % limit**, over 2.2 million particles burn-leached.

`htgr_sim_v1`'s `TRISO_ATOPS_REFERENCE_FAILURE_FRACTIONS` carries
`f_hm = 1·10⁻⁵`, `f_sic = 2·10⁻⁵`, `f_inc = 3·10⁻⁵`, `f_inc_sic = 4·10⁻⁵`,
documented there as TRISO-ATOPS reference values and *not* HTR-10 data.
Their sum, `1·10⁻⁴`, sits about **1.6× above the worst measured German
upper-95 % figure (64·10⁻⁶) and about 12× above the best (8·10⁻⁶)**. So the
placeholder is conservative for German-lineage fuel but of the right order
— which is a genuinely useful thing to be able to say about it, and could
not be said before. [`the_triso_atops_placeholders_bracket_the_german_record`]
pins it.

**This is not a licence to replace those constants.** They are TRISO-ATOPS's
own reference set, the code-to-code verification in
`crates/boon-lay/tests/triso_atops_code_to_code.rs` is measured against
them, and German burn-leach numbers are a *different fuel line's* product
quality. Changing them would swap a labelled placeholder for an unlabelled
substitution.

# 4. The one falsifiable check the German record supports

§4.2.4 of the JRC volume states a **burnup ordering at 1600 °C**:

> "High burnup (14 % FIMA) LEU UO₂ TRISO fuels show particle failure during
> the first 300 hours at 1600 °C. No particle failure was observed in lower
> burnup compact (11 % FIMA) at 1600 °C."

and, for spherical elements, that the 1600 °C heating tests showed "no
single particle failures … during the first few hundred hours".

**Caveat stated before the result, because it bounds what the check is
worth:** the 11 %/14 % pair are *compacts* (prismatic fuel), not spheres;
Figure 21's spherical elements are 4–9 % FIMA. The PANAMA-I equations have
no fuel-form parameter, so the check is on the **burnup dependence of the
pressure-vessel mechanism**, which is fuel-form-independent in the model,
and not on the compacts as such.

**Prediction, stated before measuring** (2026-09-24): `F_b` enters Eq (3)
linearly, so `σ_t ∝ F_b` and Eq (1) gives `φ₁ ∝ F_b^m`. Going 11 % → 14 %
FIMA should therefore raise `φ₁` by `(14/11)^m`, and the 4–9 % spherical
band should sit well below the 11 % compact.

**Measured, 2026-09-24**, 300 h at 1600 °C, `T_B = 776 °C`, everything else
as [`super::particle_with`] builds it:

| `F_b` | `φ₁` at 300 h | vs. 11 % FIMA | what the source reports |
|---|---|---|---|
| 4 % FIMA (sphere, low) | 2.35·10⁻⁷ | 1/1110 | no single particle failure |
| 8.51 % FIMA (**HTR-10's own**) | 4.40·10⁻⁵ | 1/5.9 | — (inside the sphere band) |
| 9 % FIMA (sphere, high) | 6.48·10⁻⁵ | 1/4.0 | no single particle failure |
| 11 % FIMA (compact) | 2.60·10⁻⁴ | — | **no** particle failure |
| 14 % FIMA (compact) | 1.39·10⁻³ | **5.32×** | particle failure **within 300 h** |

## The power law holds to three significant figures — with the IRRADIATED
## modulus, which is the part the prediction got wrong

The prediction above said `m ≈ 8` and therefore `(14/11)^8 ≈ 6.8×`. The
measurement is **5.32×**, so on its face the prediction missed by 28 %.

It did not. `m = 8.02` is the **unirradiated** modulus; Eq (9a) degrades it,
and at `T_B = 776 °C` with `Γ = 1.4·10²⁵` the modulus boon-lay fuel
failure actually applies is **`m = 6.932`** (and `σ_o = 756.1 MPa`, down from 834). Then

```text
(14/11)^6.932 = 5.32
```

which is the measured ratio to three significant figures. So `φ₁ ∝ F_b^m`
is **exact** in this model, and the apparent miss was reading `m` off the
wrong row of Table 1. [`the_burnup_power_law_uses_the_irradiated_modulus`]
pins the identity, which makes it a check on Eqs (1), (3) and (9a) acting
together rather than a curiosity.

**Recorded as a correction to my own prediction rather than quietly
restated**, per the workspace rule: the prediction was `6.8×` with `m = 8`,
it was wrong because the wrong `m` was used, and the corrected prediction
is `5.32×`, which is what was measured.

## What the comparison against the experiment actually supports

Three statements, in decreasing order of how well the data supports them:

1. **The ordering is reproduced.** `φ₁` rises monotonically over the
   source's five burnups and the 11 % → 14 % step is the steep one.
2. **The spherical record is consistent.** A KÜFA test on a few spherical
   elements examines order `10⁴`–`10⁵` particles, so "no single particle
   failure" bounds `φ₁ ≲ 10⁻⁵`–`10⁻⁴`. boon-lay fuel failure gives
   `2.3·10⁻⁷ … 6.5·10⁻⁵` over 4–9 % FIMA — **at or below that bound throughout**. HTR-10's own
   8.51 % FIMA lands at `4.4·10⁻⁵`, inside it.
3. **The 11 % compact disagrees, mildly.** boon-lay fuel failure gives
   `2.6·10⁻⁴` where no failure was seen; against a `10⁻⁵`–`10⁻⁴` bound that is an
   over-prediction of **2.6× to 26×**, i.e. roughly half a decade to 1.4
   decades. The 14 % compact, at `1.4·10⁻³`, is above the bound and the
   source reports failure there — so the model and the experiment agree on
   which side of the threshold that one falls.

**This is not a validation and must not be quoted as one.** The "bound" in
(2) and (3) is *inferred* from a particle count the source does not state
for each test, and the 11 %/14 % pair are compacts while the model is
carrying HTR-10 sphere geometry. What it is: the first time this
reconstruction has been put next to a measurement of any kind, and it
survives contact with one, in the conservative direction.

Three things could carry the 11 % over-prediction and none is chosen here:

- the strength stand-in (`σ_oo = 834 MPa`, EO 1607) may be low for the
  coatings actually tested — `φ₁ ∝ σ_oo^(−m)`, so 15 % more strength is
  about a decade less failure, which alone would close it;
- the `Γ = 1.4·10²⁵` fluence stand-in weakens the SiC through Eqs (8a)/(9a),
  and the compacts' own fluence is not stated;
- PANAMA-I is a *conservative design* model, and over-predicting failure is
  the direction a safety code is built to err in.

**No input was changed to close this.** The disagreement is the finding,
and [`the_burnup_ordering_at_1600c_matches_and_the_level_does_not`] pins
both halves of it — so a later change that quietly fixes the level by
moving a stand-in will break the test that records the gap.

```rust
pub mod qualification { /* ... */ }
```

### Types

#### Struct `BurnLeachRow`

One row of the German LEU UO₂ TRISO burn-leach record (Table 8, page 40).

```rust
pub struct BurnLeachRow {
    pub fuel_element: &'static str,
    pub year: u16,
    pub particles_tested: u32,
    pub defects_found: u32,
    pub expected_fraction: f64,
    pub upper_95_fraction: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `fuel_element` | `&'static str` | Fuel element type, as the table names it. |
| `year` | `u16` | Year of production. |
| `particles_tested` | `u32` | Particles burn-leached, `N`. |
| `defects_found` | `u32` | Defects found, `n`. |
| `expected_fraction` | `f64` | Expected defect particle fraction `n/N`. |
| `upper_95_fraction` | `f64` | One-sided upper 95 % limit on `n/N`, by the source's own Eq (19). |

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
    fn clone(self: &Self) -> BurnLeachRow { /* ... */ }
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
    fn eq(self: &Self, other: &BurnLeachRow) -> bool { /* ... */ }
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

#### Function `pressure_vessel_failure_at_burnup`

`φ₁` after `hold` at `accident` for an HTR-10-geometry particle taken to an
arbitrary burnup — the quantity the §4.2.4 ordering is about.

Everything except `burnup` is [`super::particle`]'s: HTR-10's geometry and
kernel, the derived residence `t_B`, and the two HTR-Module stand-ins. Only
the burnup moves, which is what makes the comparison an ordering test of
one variable rather than a fit.

```rust
pub fn pressure_vessel_failure_at_burnup(burnup: uom::si::f64::Ratio, irradiation_temperature: uom::si::f64::ThermodynamicTemperature, accident: uom::si::f64::ThermodynamicTemperature, hold: uom::si::f64::Time, steps: usize) -> uom::si::f64::Ratio { /* ... */ }
```

### Constants and Statics

#### Constant `BURN_LEACH_DEFECT_FRACTIONS`

**The German LEU UO₂ TRISO burn-leach record** — Table 8, page 40 of
Kugeler, Nabielek & Buckthorpe (2017).

This is *measured* as-manufactured fuel quality for the production line
HTR-10's fuel descends from, over **2 202 200 particles** in total. It is
the physical population that the PANAMA-I equations' `φ_o` input stands
for, and boon-lay fuel failure itself does not compute it.

**What it does to any answer:** `φ_o` enters only through
`φ_total = 1 − (1−φ_o)(1−φ₁)(1−φ₂)`, so while every term is small the
total is very nearly `φ_o + φ₁ + φ₂` — additive, not multiplicative. Below
about 1500 °C, where `φ₁ ≪ 10⁻⁵`, `φ_o` **is** the answer and the accident
model contributes nothing to it.

The `AVR 21-2` row is the source's "highest-quality fuel ever produced in
the German fuel development programme" (page 38) and the `Proof test fuel`
row is the HTR-Module proof-test production — the campaign whose
`σ_oo`/`m_oo` [`super::STAND_IN_STRENGTH_MPA`] stands in with.

```rust
pub const BURN_LEACH_DEFECT_FRACTIONS: [BurnLeachRow; 4] = _;
```

#### Constant `FREE_URANIUM_FRACTIONS`

As-manufactured **free-uranium** fractions `U_free/U_total`, Table 7,
page 38 of the same source, for matrix types A3-27 and A3-3.

These are the measured counterpart of TRISO-ATOPS's `f_hm`, the heavy-metal
contamination fraction — uranium outside intact kernels. The source states
them as campaign upper bounds: GLE-3 `< 50.7·10⁻⁶`, GLE-4/1 `< 43·10⁻⁶`,
GLE-4/2 `< 8·10⁻⁶`, HTR-Module proof test `< 13.5·10⁻⁶`.

**What it does to any answer:** `f_hm` multiplies `⟨R/B⟩_fail` directly for
noble gases and halogens (see
`crate::triso_atops_fork::activities::release_rate`), so release from those
groups is **linear** in it. The five values below span a factor 6.5, which
is therefore a factor-6.5 band on any noble-gas release computed with one
of them.

```rust
pub const FREE_URANIUM_FRACTIONS: [f64; 5] = _;
```

#### Constant `BISO_FREE_URANIUM_RANGE`

The earlier German **BISO** free-uranium range, `3·10⁻⁴` to `9·10⁻⁴`
(page 40, attributed to Kania 1980) — an order of magnitude worse than the
LEU UO₂ TRISO record above, quoted by the source as the contrast that
establishes the improvement. Not a stand-in for anything; present so the
TRISO figures have a scale.

```rust
pub const BISO_FREE_URANIUM_RANGE: (f64, f64) = _;
```

#### Constant `KUFA_COMPACT_NO_FAILURE_FIMA`

Burnup of the compact the source reports as showing **no** particle failure
in the first 300 h at 1600 °C (§4.2.4), as a FIMA fraction.

```rust
pub const KUFA_COMPACT_NO_FAILURE_FIMA: f64 = 0.11;
```

#### Constant `KUFA_COMPACT_FAILURE_FIMA`

Burnup of the compact the source reports as **showing** particle failure
within the first 300 h at 1600 °C (§4.2.4), as a FIMA fraction.

```rust
pub const KUFA_COMPACT_FAILURE_FIMA: f64 = 0.14;
```

#### Constant `KUFA_SPHERE_FIMA_RANGE`

The spherical-fuel-element burnup band of the same figure (Figure 21),
4 % to 9 % FIMA — which brackets HTR-10's derived 8.51 %.

```rust
pub const KUFA_SPHERE_FIMA_RANGE: (f64, f64) = _;
```

### Functions

#### Function `sic_layer`

The HTR-10 SiC layer (IAEA-TECDOC-1382 pt 2 Table 4-17).

```rust
pub fn sic_layer() -> super::geometry::SicLayer { /* ... */ }
```

#### Function `kernel_volume`

The kernel volume `V_k`, from the published kernel radius.

```rust
pub fn kernel_volume() -> uom::si::f64::Volume { /* ... */ }
```

#### Function `free_volume`

The free volume `V_f` — **half the buffer shell**, which is the report's
own definition of `V_f` (page -485-: "corresponding to 50 % of buffer
volume"), applied to HTR-10's published buffer.

```rust
pub fn free_volume() -> uom::si::f64::Volume { /* ... */ }
```

#### Function `particle`

The HTR-10 particle as boon-lay fuel failure sees it, at a stated
irradiation temperature.

- `irradiation_temperature` — `T_B`. **An input**: HTR-10 publishes a
  *maximum* fuel temperature (JAERI-Conf 96-010 states a 700 °C margin to
  the 1600 °C limit) but no average, and the PANAMA-I equations' `T_B`
  is an average. The report's HTR-Module average of **776 °C** (Table 2)
  is the nearest published figure for a comparable core and is what
  [`tests`] sweeps around; it is not HTR-10's.

Uses [`STAND_IN_STRENGTH_MPA`], [`STAND_IN_WEIBULL_MODULUS`] and
[`STAND_IN_FLUENCE_E25_PER_M2`] — read their docs before quoting any
number this produces.

```rust
pub fn particle(irradiation_temperature: uom::si::f64::ThermodynamicTemperature) -> super::history::ParticleState { /* ... */ }
```

#### Function `particle_with`

The same particle with `F_b`, `t_B` and `Γ` opened up, for sweeping the
inputs that HTR-10 does not publish or that a sensitivity study needs.

[`particle`] is this with HTR-10's own derived burnup and residence and the
HTR-Module fluence stand-in. Everything else — geometry, kernel compound,
`σ_oo`/`m_oo` — is held at the values [`particle`] uses, because those are
either HTR-10's own or are swept elsewhere.

**Sweeping `Γ` is a sensitivity, not a calibration.** Eqs (8a)/(9a) make
both `σ_o` and `m` fall with fluence, so a larger `Γ` weakens the particle
and raises `φ₁`. The range worth exploring is the one the report's own
cases span (Table 2), not whatever range makes an answer come out right.

```rust
pub fn particle_with(irradiation_temperature: uom::si::f64::ThermodynamicTemperature, burnup: uom::si::f64::Ratio, irradiation_time: uom::si::f64::Time, fluence_e25_per_m2: f64) -> super::history::ParticleState { /* ... */ }
```

#### Function `end_of_irradiation_failure`

`φ₁` at the **end of irradiation** — the value the PANAMA-I report
assigns to `t = 0` of an accident (page -482-), and the one that matters
for normal operation.

# Why this is not an accident step of length zero

It uses **Eq (5b)**, not Eq (5c). Eq (5c) is the `OPF` "during heating"
and carries `−0.404·(10⁴/T − 10⁴/(T_B + 75))`, which does **not** vanish
at `T = T_B`: the `+75 °C` is the report's kernel-versus-surface
correction, so Eq (5c) reduces to Eq (5b) at `T = T_B + 75`, the kernel
temperature, and not at the surface temperature `T_B`. Feeding `T_B`
through the accident path instead would apply a spurious 0.26-decade
cooling term and under-state `OPF` by a factor 1.8.

`FKOR = 1` and `τ_a = 0` here: no accident has happened, so there is no
corrosion and no accident-time gas release.

```rust
pub fn end_of_irradiation_failure(irradiation_temperature: uom::si::f64::ThermodynamicTemperature) -> super::FailureFraction { /* ... */ }
```

#### Function `end_of_irradiation_failure_for`

[`end_of_irradiation_failure`] for an arbitrary [`ParticleState`] — the
form the sweeps need, since they vary `F_b`, `t_B` and `Γ`.

`irradiation_time` must be the same `t_B` the particle was built with:
Eq (5b)'s `OPF` needs it and [`ParticleState`] does not carry it.

```rust
pub fn end_of_irradiation_failure_for(p: &super::history::ParticleState, irradiation_temperature: uom::si::f64::ThermodynamicTemperature, irradiation_time: uom::si::f64::Time) -> super::FailureFraction { /* ... */ }
```

### Constants and Statics

#### Constant `KERNEL_RADIUS_UM`

HTR-10 kernel radius, 250 µm (IAEA-TECDOC-1382 pt 2 Table 4-17).

```rust
pub const KERNEL_RADIUS_UM: f64 = 250.0;
```

#### Constant `BUFFER_OUTER_RADIUS_UM`

Buffer outer radius, 340 µm (same source).

```rust
pub const BUFFER_OUTER_RADIUS_UM: f64 = 340.0;
```

#### Constant `SIC_INNER_RADIUS_UM`

SiC inner radius (= IPyC outer), 380 µm (same source).

```rust
pub const SIC_INNER_RADIUS_UM: f64 = 380.0;
```

#### Constant `SIC_OUTER_RADIUS_UM`

SiC outer radius, 415 µm — a 35 µm layer (same source).

```rust
pub const SIC_OUTER_RADIUS_UM: f64 = 415.0;
```

#### Constant `BURNUP_FIMA`

HTR-10 design mean burnup as a **FIMA fraction**, derived from the
published 80 000 MWd/t (IAEA-TECDOC-1382 pt 2 §4; Li et al. 2014).

Derivation, so it can be checked rather than trusted: at 17 % enrichment
the heavy-metal molar mass is `0.17·235 + 0.83·238 = 237.5 g/mol`, so one
tonne holds `2.536·10²⁷` atoms. One per cent of them fissioning at
200 MeV releases `8.13·10¹⁴ J = 9404 MWd`. Hence
`80 000 / 9404 = 8.51 % FIMA`.

The 200 MeV is the conventional recoverable energy per fission and is the
only assumption not taken from the HTR-10 literature; it moves the answer
by about ±2 % across the usual 195–205 MeV range.

```rust
pub const BURNUP_FIMA: f64 = 0.0851;
```

#### Constant `RESIDENCE_FULL_POWER_DAYS`

HTR-10 mean fuel residence at full power, **1080 FPD**, derived.

The core holds 27 000 elements at 5 g heavy metal each (IAEA-TECDOC-1382
pt 2 §4 design table) — 135 kg — at 10 MW thermal, so the specific power
is `10/0.135 = 74.07 MW/t` and reaching the design mean 80 000 MWd/t takes
`80 000/74.07 = 1080` full-power days. Comparable to the report's own
HTR-Module case (15 passes × 68 d = 1020 FPD).

```rust
pub const RESIDENCE_FULL_POWER_DAYS: f64 = 1080.0;
```

#### Constant `STAND_IN_STRENGTH_MPA`

**STAND-IN.** SiC tensile strength before irradiation, 834 MPa — EO 1607,
the variety the report's own HTR-Module and HTR-500 runs use (footnote 1,
page -503-; Table 2, page -504-). **Not HTR-10 data**; no `σ_oo` for
HTR-10's SiC is published in this workspace's literature.

834/8.02 rather than Table 1's 850/8.0: the two disagree and the units doc
settles on the Fig. 5 / Table 2 pair for reactor reproductions.

```rust
pub const STAND_IN_STRENGTH_MPA: f64 = 834.0;
```

#### Constant `STAND_IN_WEIBULL_MODULUS`

**STAND-IN.** Weibull modulus before irradiation, 8.02 — EO 1607, as above.

```rust
pub const STAND_IN_WEIBULL_MODULUS: f64 = 8.02;
```

#### Constant `STAND_IN_FLUENCE_E25_PER_M2`

**STAND-IN.** Fast fluence at discharge, `1.4·10²⁵ m⁻² EDN` — the report's
HTR-Module and HTR-500 value (Table 2, page -504-). **Not HTR-10 data.**

```rust
pub const STAND_IN_FLUENCE_E25_PER_M2: f64 = 1.4;
```

## Module `molar_volume`

**Eqs (6a)/(6b)/(6c)** — the molar volume `V_m` of the heavy metal in the
kernel (pages -491- and -492-).

```text
(Th,U)O2   V_m = 0.2645 [kg/mol] / 10500 [kg/m³] = 2.51905e-5 m³/mol   (6a)
UO2        V_m = 0.2672 [kg/mol] / 10960 [kg/m³] = 2.43796e-5 m³/mol   (6b)
UCO        V_m = 0.2682 [kg/mol] / 10700 [kg/m³] = 2.50654e-5 m³/mol   (6c)
```

The report defines `V_m` as the weight of one mole of the kernel compound
divided by its density, and Eq (3) divides by it to turn a burnup in FIMA
into a number of moles of gas. It is a fixed property of the compound: no
temperature, no burnup, no irradiation history.

# The third equation is printed as (6b), not (6c)

The UCO relation on page -492- carries the label **(6b)** again — the same
number already used for `UO₂` on page -491-. It is recorded here as (6c),
which is what it must be, with the defect noted rather than silently
renumbered. A reader chasing "Eq (6b)" in the report will find two
different molar volumes under it. See
`docs/panama-i-units-and-open-questions.md`.

# Verification

**Self-verifying**: each equation prints its own quotient to six
significant figures, so the division is a closed check on the
transcription of both constants. All three reproduce their printed result
to within 1 part in 10⁵ — see [`tests::the_printed_quotients_are_exact`].

That is the only verification available. There is **no figure or table in
the report that `V_m` can be checked against independently**, and no
statement of which compound stoichiometry the molar masses correspond to:
0.2672 kg/mol is neither `UO₂` at natural enrichment (0.2700) nor `²³⁵UO₂`
(0.2670), and the report does not say what mixture it assumes. Taken as
printed.

```rust
pub mod molar_volume { /* ... */ }
```

### Types

#### Enum `KernelCompound`

Which kernel compound the molar volume is for.

Three variants, because Eqs (6a)–(6c) give three different values.
Deliberately **not** the same type as [`super::diffusion::KernelKind`],
which has two: the `D_S` correlation on page -487- explicitly uses the
`UO₂` relation for `UCO` as well, while the molar volume does not. Merging
them would silently give `UCO` the `UO₂` molar volume, a 2.8 % error in
the gas pressure that nothing would flag.

```rust
pub enum KernelCompound {
    ThoriumUraniumOxide,
    UraniumOxide,
    UraniumOxycarbide,
}
```

##### Variants

###### `ThoriumUraniumOxide`

`(Th,U)O₂` — Eq (6a).

###### `UraniumOxide`

`UO₂` — Eq (6b).

###### `UraniumOxycarbide`

`UCO` — Eq (6c), printed as a second "(6b)".

##### Implementations

###### Methods

- ```rust
  pub const fn molar_mass_kg_per_mol(self: Self) -> f64 { /* ... */ }
  ```
  The molar mass the report divides by, \[kg/mol\].

- ```rust
  pub const fn density_kg_per_m3(self: Self) -> f64 { /* ... */ }
  ```
  The kernel density the report divides by, \[kg/m³\].

- ```rust
  pub const fn printed_molar_volume_m3_per_mol(self: Self) -> f64 { /* ... */ }
  ```
  The value the report prints for the quotient, \[m³/mol\]. Used only to

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> KernelCompound { /* ... */ }
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
    fn eq(self: &Self, other: &KernelCompound) -> bool { /* ... */ }
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

#### Function `molar_volume`

**Eqs (6a)/(6b)/(6c)** — the molar volume `V_m` of the heavy metal in the
kernel (pages -491-, -492-).

Computed from the printed molar mass and density rather than returned as
the printed quotient, so that the two constants — not a third derived
number — are what this crate carries.

```rust
pub fn molar_volume(kernel: KernelCompound) -> uom::si::f64::MolarVolume { /* ... */ }
```

## Module `oxygen`

**Eqs (5a)–(5f)** — the number of oxygen atoms released per fission,
`OPF` (pages -488- to -491-).

Oxygen freed when a `UO2` or `(Th,U)O2` kernel fissions forms CO, which
adds to the internal gas pressure alongside the fission gases themselves.
`OPF` enters Eq (3) directly, so it sets the **absolute** pressure.

| Eq | Kernel | Regime | Source |
|---|---|---|---|
| (5a) | `(Th,U)O2` | any | Strigl 1984 |
| (5b) | `UO2` | before heating | Proksch 1982 |
| (5c) | `UO2` | during heating | Proksch 1982 |
| (5d) | `UCO` | any | zero by assumption |
| (5e) | all | cap | `OPF_max = 0.625` |

# `t_B` is in SECONDS — settled by Fig. 3

This was the last open units question, and it mattered more than any
other: `(5b)`/`(5c)` carry `2·log t_B`, so a seconds-vs-days confusion
moves `log OPF` by about **9.9 decades**, and `OPF` sets the absolute
pressure for everything downstream.

The report's symbol list (-511-) says seconds. Fig. 3's curve labels say
`1000 °C, 1000 d`; Figs. 7 and 8's captions say `260 FPD` and `500 FPD`;
and page -488- gives the correlation's validity range as "66 and 550 full
power days". Three places in days against one in seconds — so the
extraction record left it open rather than guessing.

Checked 2026-09-24 against a digitisation of Fig. 3's four labelled `UO2`
curves, spanning `T_B` 900–1100 °C and `t_B` 500–1000 d:

| reading of `t_B` | mean abs error in `OPF` |
|---|---|
| **seconds** | **0.0087** |
| days | 0.277 — `OPF` collapses to ~0 everywhere |

So the symbol list was right and the labels are simply human-readable:
the **curve** is titled in days, the **formula** takes seconds. That also
explains why the printed validity range is in full-power days — it
describes the experiments, not the argument.

[`oxygen_per_fission_uo2`] therefore takes a `uom` [`Time`] and converts
internally, so a caller cannot get this wrong at all.

```rust
pub mod oxygen { /* ... */ }
```

### Types

#### Enum `HeatingRegime`

Which side of the accident a `UO2` `OPF` is wanted for.

`PartialEq` only: the `DuringHeating` arm carries a temperature, and a
float has no total equality.

```rust
pub enum HeatingRegime {
    BeforeHeating,
    DuringHeating {
        temperature: uom::si::f64::ThermodynamicTemperature,
    },
}
```

##### Variants

###### `BeforeHeating`

**Eq (5b)** — the inventory built up during irradiation, before the
accident begins. Depends only on the irradiation history.

###### `DuringHeating`

**Eq (5c)** — during the accident, at accident temperature `T`. Adds
the `-0.404*(1e4/T - 1e4/(T_B + 75))` term.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `temperature` | `uom::si::f64::ThermodynamicTemperature` | `T`, the accident temperature. |

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
    fn clone(self: &Self) -> HeatingRegime { /* ... */ }
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
    fn eq(self: &Self, other: &HeatingRegime) -> bool { /* ... */ }
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

#### Function `oxygen_per_fission_thoria`

**Eq (5a)** — `OPF` for a `(Th,U)O2` kernel (page -488-, Strigl 1984).

```text
log10(OPF) = 0.96 - 0.442e4/T + 0.4*log10(N) + 0.3*log10(F_b)
```

- `temperature` — `T`, the accident temperature.
- `thorium_to_u235` — `N`, the thorium / uranium-235 ratio. The report
  gives `N = 5` for AVR and `N = 10` for THTR.
- `burnup` — `F_b`, in FIMA as a fraction.

Unlike the `UO2` correlations this has **no irradiation history** in it at
all — no `T_B`, no `t_B`. That asymmetry is the report's own: it states
that oxygen formation in `UO2` "is greatly dependent on the irradiation
history" and for `(Th,U)O2` it is not.

Capped at [`OPF_MAX`].

```rust
pub fn oxygen_per_fission_thoria(temperature: uom::si::f64::ThermodynamicTemperature, thorium_to_u235: f64, burnup: uom::si::f64::Ratio) -> uom::si::f64::Ratio { /* ... */ }
```

#### Function `oxygen_per_fission_uo2`

**Eqs (5b)/(5c)** — `OPF` for a `UO2` kernel (page -489-, Proksch 1982).

```text
(5b)  log10(OPF) = -10.08 - 0.85e4/T_B + 2*log10(t_B)
(5c)  log10(OPF) = -10.08 - 0.85e4/T_B + 2*log10(t_B)
                   - 0.404*(1e4/T - 1e4/(T_B + 75))
```

- `irradiation_temperature` — `T_B`, the particle **surface** temperature
  during irradiation.
- `irradiation_time` — `t_B`. **Enters the formula in seconds**; taking a
  `uom` [`Time`] here is deliberate, because this was the one unit in the
  whole report that three separate places disagreed about. See the module
  docs.

The report states these are valid for a **constant** irradiation
temperature, over 66–550 full-power days and `T_B` of 950–1525 °C. Those
bounds are not enforced: the report itself plots Fig. 3 outside them
(curves at 1000 d), and silently clamping an input is worse than
returning what the correlation says.

Capped at [`OPF_MAX`].

```rust
pub fn oxygen_per_fission_uo2(irradiation_temperature: uom::si::f64::ThermodynamicTemperature, irradiation_time: uom::si::f64::Time, regime: HeatingRegime) -> uom::si::f64::Ratio { /* ... */ }
```

#### Function `oxygen_per_fission_uco`

**Eq (5d)** — `OPF` for a `UCO` kernel: zero (page -489-).

"No oxygen production is assumed to happen in particles with UCO
kernels." A function rather than a bare constant so a caller dispatching
on kernel type reads the same at every arm.

```rust
pub fn oxygen_per_fission_uco() -> uom::si::f64::Ratio { /* ... */ }
```

### Constants and Statics

#### Constant `OPF_MAX`

**Eq (5e)** — the upper limit on `OPF`, 0.625 (page -489-).

The report notes that for `(Th,U)O2` this would only be exceeded above
5700 K, i.e. never in practice; for `UO2` it binds at accident
temperatures and is visible as the horizontal dashed line in Fig. 3.

```rust
pub const OPF_MAX: f64 = 0.625;
```

#### Constant `KERNEL_SURFACE_OFFSET_K`

The `+75 °C` correction in Eq (5c) (page -489-).

`T_B` is the particle **surface** temperature, while the temperature that
determines release inside a `UO2` TRISO kernel is about 75 degrees higher.

```rust
pub const KERNEL_SURFACE_OFFSET_K: f64 = 75.0;
```

## Module `pressure`

**Eq (3)** — internal gas pressure from the ideal gas law (page -484-),
with the constants printed alongside it on page -485-.

The printed grouping is ambiguous; see [`internal_gas_pressure`] for the
dimensional argument that settles it.

```rust
pub mod pressure { /* ... */ }
```

### Functions

#### Function `internal_gas_pressure`

**Eq (3)** — internal gas pressure from the ideal gas law (page -484-).

```text
p = (F_d·F_f + OPF) · F_b · R · T / [ (V_f/V_k) · V_m ]      [Pa]
```

- `released_gas_fraction` — `F_d`, the relevant fraction of fission gas
  released from the kernel (Eq (4), Allelein 1983 — **not** implemented
  here; supply it).
- `stable_gas_yield` — `F_f`, atoms of stable fission gas per fission;
  [`STABLE_FISSION_GAS_YIELD`] is the report's 0.31.
- `oxygen_per_fission` — `OPF`, CO-forming oxygen atoms per fission
  (Eqs (5a)–(5f) — **not** implemented here; supply it, and see the module
  docs on why).
- `burnup` — `F_b`, heavy-metal burnup in FIMA.
- `free_volume` / `kernel_volume` — `V_f` (buffer void) and `V_k`.
- `molar_volume` — `V_m`, the molar volume of the kernel compound
  (Eqs (6a)–(6c) — supply it).
- `temperature` — `T`, in kelvin via `uom`.

## The printed grouping is ambiguous; this is the dimensionally consistent
reading

As printed, the fraction bar appears to span `(V_f/V_k)·R·T/V_m`, which
would give `p ∝ 1/(R·T)` — dimensionally wrong, and it would make pressure
*fall* as the particle heats. Only one grouping is consistent, and it is
also just `p = nRT/V_f` with `n = (F_d·F_f + OPF)·F_b·V_k/V_m`:
`(F_d·F_f+OPF)·F_b` is dimensionless (moles of gas per mole of heavy metal),
`R·T` is Pa·m³/mol, `V_f/V_k` is dimensionless and `V_m` is m³/mol, leaving
**Pa**. That is what is implemented. [`pressure_is_the_ideal_gas_law`]
pins it against `nRT/V` computed independently.

```rust
pub fn internal_gas_pressure(released_gas_fraction: uom::si::f64::Ratio, stable_gas_yield: uom::si::f64::Ratio, oxygen_per_fission: uom::si::f64::Ratio, burnup: uom::si::f64::Ratio, free_volume: uom::si::f64::Volume, kernel_volume: uom::si::f64::Volume, molar_volume: uom::si::f64::MolarVolume, temperature: uom::si::f64::ThermodynamicTemperature) -> uom::si::f64::Pressure { /* ... */ }
```

### Constants and Statics

#### Constant `GAS_CONSTANT_J_PER_MOL_K`

Molar gas constant `R` in J/(mol·K), as printed with Eq (3) on page -485-.

The report's own value, kept rather than substituting a CODATA figure, so
the arithmetic reproduces the source exactly.

```rust
pub const GAS_CONSTANT_J_PER_MOL_K: f64 = 8.3143;
```

#### Constant `STABLE_FISSION_GAS_YIELD`

Fission yield of the stable fission gases `F_f`, printed with Eq (3)
(page -485-). Dimensionless, atoms per fission.

```rust
pub const STABLE_FISSION_GAS_YIELD: f64 = 0.31;
```

## Module `strength`

**Eqs (8a)/(8b)/(9a)/(9b)** — how irradiation degrades the SiC tensile
strength and the Weibull modulus (pages -493- and -494-, both attributed
to Allelein 1983).

These are the two correlations that supply [`super::weibull`]'s `sigma_o`
and `m`, and they are **verified against the report's own Table 1** — see
`tests::table_1_is_reproduced_exactly`.

```rust
pub mod strength { /* ... */ }
```

### Functions

#### Function `irradiated_strength`

**Eq (8a)** — SiC tensile strength after irradiation (page -493-,
attributed to Allelein 1983), with Eq (8b)'s floor applied.

```text
sigma_o = sigma_oo * (1 - Gamma / Gamma_s)
log10(Gamma_s) = 0.556 + 0.065e4 / T_B
```

- `unirradiated` — `sigma_oo`, the measured strength before irradiation.
- `fluence_e25_per_m2` — `Gamma`, fast-neutron fluence **in the
  correlation's own units of 10^25 m^-2 EDN**. Not `uom`-typed on
  purpose: this is a `log10` fit whose intercept is only meaningful in
  those units, so a dimensioned argument would imply a freedom of unit
  choice the correlation does not have. The name carries the unit instead.
- `irradiation_temperature` — `T_B`, **in kelvin**. The report's symbol
  list prints `T_B` in degC and never states the conversion; that it is
  kelvin is established by [`table_1_is_reproduced_exactly`], which
  reproduces all sixteen of the report's own calculated values only on
  the kelvin reading.

Returns at least [`MIN_TENSILE_STRENGTH_MPA`].

```rust
pub fn irradiated_strength(unirradiated: uom::si::f64::Pressure, fluence_e25_per_m2: f64, irradiation_temperature: uom::si::f64::ThermodynamicTemperature) -> uom::si::f64::Pressure { /* ... */ }
```

#### Function `irradiated_weibull_modulus`

**Eq (9a)** — the Weibull modulus after irradiation (page -494-,
Allelein 1983), with Eq (9b)'s floor applied.

```text
m_o = m_oo * (1 - Gamma / Gamma_m)
log10(Gamma_m) = 0.394 + 0.065e4 / T_B
```

Same `10^4/T_B` coefficient as [`irradiated_strength`]'s `Gamma_s`; only
the intercept differs (0.394 against 0.556), so the modulus degrades
**faster** than the strength -- the distribution widens as it weakens.

Eq (10a) then states `m = m_o`, i.e. this is the modulus that goes into
[`weibull_failure_fraction`].

Returns at least [`MIN_WEIBULL_MODULUS`].

```rust
pub fn irradiated_weibull_modulus(unirradiated: f64, fluence_e25_per_m2: f64, irradiation_temperature: uom::si::f64::ThermodynamicTemperature) -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `MIN_TENSILE_STRENGTH_MPA`

**Eq (8b)** — the floor under the irradiated SiC tensile strength,
196 MPa (page -494-).

```rust
pub const MIN_TENSILE_STRENGTH_MPA: f64 = 196.0;
```

#### Constant `MIN_WEIBULL_MODULUS`

**Eq (9b)** — the floor under the irradiated Weibull modulus, 2
(page -494-).

```rust
pub const MIN_WEIBULL_MODULUS: f64 = 2.0;
```

## Module `stress`

**Eq (2)** — the SiC hoop stress induced by the internal gas pressure
(page -484-), in both the report's approximate and exact thin-shell forms.

```rust
pub mod stress { /* ... */ }
```

### Functions

#### Function `induced_stress`

**Eq (2)** — the SiC hoop stress induced by internal gas pressure, in the
report's preferred approximate form (page -484-).

```text
σ_t = r·p / (2·d_o) · (1 + v̇·t/d_o)      [Pa]
```

The report gives the exact thin-shell result first
([`induced_stress_exact`]) and then states that this approximation
"describes the state of affairs more realistically, in particular for small
actual SiC layer thicknesses" — it is the linearisation of the exact form in
`v̇t/d_o`, and unlike the exact form it stays finite as the layer thins.
**This is the one to use**; the exact form is provided for comparison.

Note the model is **thin-shell throughout**. The report contains no
thick-wall (Lamé) formulation, so none is offered here.

```rust
pub fn induced_stress(layer: &super::geometry::SicLayer, pressure: uom::si::f64::Pressure, corrosion_rate: uom::si::f64::Velocity, elapsed: uom::si::f64::Time) -> uom::si::f64::Pressure { /* ... */ }
```

#### Function `induced_stress_exact`

The thin-shell stress written through the thickness, `σ_t = r·p/(2·d_act)`
with `d_act` from Eq (7) (pages -484- and -492-).

## This EQUALS [`induced_stress`]; it is not an alternative to it

Substituting Eq (7)'s `d_act = d_o/(1 + v̇t/d_o)` gives
`σ_t = r·p·(1 + v̇t/d_o)/(2·d_o)`, which is Eq (2) **exactly**. The
report's description of Eq (2) as an approximation that "describes the
state of affairs more realistically" therefore understates it: given
Eq (7), Eq (2) is not an approximation at all.

What Eq (2) approximates is the *other* form printed on page -484-,
`d_act = d_o·(1 − v̇·t)`, which is dimensionally inconsistent and
contradicts Eq (7). See [`SicLayer::actual_thickness`].

Kept as its own function because computing the stress by two routes and
asserting they agree is a real check on the algebra —
`tests::the_two_routes_to_the_stress_agree_exactly`. `None` only if the
thickness is non-positive, which Eq (7) cannot produce; it guards against
a caller supplying a negative rate.

```rust
pub fn induced_stress_exact(layer: &super::geometry::SicLayer, pressure: uom::si::f64::Pressure, corrosion_rate: uom::si::f64::Velocity, elapsed: uom::si::f64::Time) -> Option<uom::si::f64::Pressure> { /* ... */ }
```

#### Function `induced_stress_with_thinning_factor`

Eq (2) written against a **carried** `FKOR` rather than an elapsed time
(pages -484-, -492-):

```text
σ_t = r·p·FKOR / (2·d_o)      since d_act = d_o/FKOR
```

[`induced_stress`] recomputes `v̇·t/d_o` from a single rate and a single
elapsed time, which is only correct for an **isothermal** hold. A varying
temperature history has to carry `FKOR` forward step by step
([`super::advance_thinning_factor`]), and this is the entry point that
takes it. It is what [`super::history`] uses.

`FKOR` starts at 1 (an uncorroded layer) and rises; values below 1 would
mean a layer that had grown, so they are clamped to 1 rather than
silently producing a stress lower than the uncorroded one.

```rust
pub fn induced_stress_with_thinning_factor(layer: &super::geometry::SicLayer, pressure: uom::si::f64::Pressure, thinning_factor: uom::si::f64::Ratio) -> uom::si::f64::Pressure { /* ... */ }
```

## Module `weibull`

**Eq (1)** — the Weibull pressure-vessel failure law (page -483-).

The `ln2` normalisation is the load-bearing detail; see the function's own
docs and [`super::strength`] for where its `m` comes from.

```rust
pub mod weibull { /* ... */ }
```

### Functions

#### Function `weibull_failure_fraction`

**Eq (1)** — the fraction of particles failed by pressure-vessel overstress
(page -483-, attributed to Nabielek 1984).

```text
φ₁(t,T) = 1 − exp[ −ln2 · (σ_t / σ_o)^m ]
```

- `induced_stress` — `σ_t`, the SiC hoop stress from the internal gas
  pressure ([`induced_stress`]).
- `median_strength` — `σ_o`, the SiC tensile strength **at the end of
  irradiation**. See the module docs: because of the `ln2`, this is the
  *median* of the strength distribution, not its characteristic value.
- `weibull_modulus` — `m`, dimensionless.

Returns a fraction in `[0, 1]`; it is `0.5` exactly when
`induced_stress == median_strength`, for any `m`.

```rust
pub fn weibull_failure_fraction(induced_stress: uom::si::f64::Pressure, median_strength: uom::si::f64::Pressure, weibull_modulus: f64) -> super::FailureFraction { /* ... */ }
```

### Types

#### Type Alias `FailureFraction`

A failed-particle fraction, dimensionless and in `[0, 1]`.

```rust
pub type FailureFraction = uom::si::f64::Ratio;
```

### Functions

#### Function `total_failure_fraction`

The combination of the three failure populations (page -480-).

```text
φ_total = 1 − (1 − φ_o)·(1 − φ₁)·(1 − φ₂)
```

A particle survives only if it survives all three mechanisms, so the
*survival* probabilities multiply. This is why the result is not the sum:
summing would double-count particles failed by more than one mechanism and
can exceed 1.

- `as_manufactured` — `φ_o`. The report's own runs use `0`; see
  [`AS_MANUFACTURED_TARGET`].
- `pressure_vessel` — `φ₁`, from [`weibull_failure_fraction`].
- `thermal_decomposition` — `φ₂`, from
  [`thermal_decomposition_failure_fraction`]. Passing
  `Ratio::new::<ratio>(0.0)` models pressure-vessel failure alone, which is
  non-conservative above ~2000 °C, where the report attributes failure
  principally to SiC decomposition.

```rust
pub fn total_failure_fraction(as_manufactured: FailureFraction, pressure_vessel: FailureFraction, thermal_decomposition: FailureFraction) -> FailureFraction { /* ... */ }
```

### Constants and Statics

#### Constant `AS_MANUFACTURED_TARGET`

The as-manufactured defective fraction `φ_o` used for reactor studies,
`6·10⁻⁵` (page -480-).

The report's own calculations set `φ_o = 0`; this is the value it states may
"without difficulty" be used as a target when considering reactor concepts.
It is offered as a named constant, never as a default — which of the two
applies is the caller's modelling decision.

```rust
pub const AS_MANUFACTURED_TARGET: f64 = 6.0e-5;
```

### Re-exports

#### Re-export `booth_release_function`

```rust
pub use booth::booth_release_function;
```

#### Re-export `dimensionless_time`

```rust
pub use booth::dimensionless_time;
```

#### Re-export `released_gas_fraction`

```rust
pub use booth::released_gas_fraction;
```

#### Re-export `MAX_SUMMANDS`

```rust
pub use booth::MAX_SUMMANDS;
```

#### Re-export `SUMMAND_CONVERGENCE`

```rust
pub use booth::SUMMAND_CONVERGENCE;
```

#### Re-export `advance_thinning_factor`

```rust
pub use corrosion::advance_thinning_factor;
```

#### Re-export `corrosion_rate`

```rust
pub use corrosion::corrosion_rate;
```

#### Re-export `thinning_factor`

```rust
pub use corrosion::thinning_factor;
```

#### Re-export `advance_action_integral`

```rust
pub use decomposition::advance_action_integral;
```

#### Re-export `decomposition_rate_constant`

```rust
pub use decomposition::decomposition_rate_constant;
```

#### Re-export `thermal_decomposition_failure_fraction`

```rust
pub use decomposition::thermal_decomposition_failure_fraction;
```

#### Re-export `DecompositionCalibration`

```rust
pub use decomposition::DecompositionCalibration;
```

#### Re-export `DECOMPOSITION_ACTIVATION_J_PER_MOL`

```rust
pub use decomposition::DECOMPOSITION_ACTIVATION_J_PER_MOL;
```

#### Re-export `reduced_diffusion_coefficient`

```rust
pub use diffusion::reduced_diffusion_coefficient;
```

#### Re-export `KernelKind`

```rust
pub use diffusion::KernelKind;
```

#### Re-export `SicLayer`

```rust
pub use geometry::SicLayer;
```

#### Re-export `advance_grain_boundary_exposure`

```rust
pub use grain_boundary::advance_grain_boundary_exposure;
```

#### Re-export `corroded_weibull_modulus`

```rust
pub use grain_boundary::corroded_weibull_modulus;
```

#### Re-export `grain_boundary_corrosion_rate`

```rust
pub use grain_boundary::grain_boundary_corrosion_rate;
```

#### Re-export `GrainBoundaryCorrosion`

```rust
pub use grain_boundary::GrainBoundaryCorrosion;
```

#### Re-export `irradiation_tau`

```rust
pub use history::irradiation_tau;
```

#### Re-export `AccidentHistory`

```rust
pub use history::AccidentHistory;
```

#### Re-export `AccidentStep`

```rust
pub use history::AccidentStep;
```

#### Re-export `FailureProgress`

```rust
pub use history::FailureProgress;
```

#### Re-export `OxygenSource`

```rust
pub use history::OxygenSource;
```

#### Re-export `ParticleState`

```rust
pub use history::ParticleState;
```

#### Re-export `molar_volume`

```rust
pub use molar_volume::molar_volume;
```

#### Re-export `KernelCompound`

```rust
pub use molar_volume::KernelCompound;
```

#### Re-export `oxygen_per_fission_thoria`

```rust
pub use oxygen::oxygen_per_fission_thoria;
```

#### Re-export `oxygen_per_fission_uco`

```rust
pub use oxygen::oxygen_per_fission_uco;
```

#### Re-export `oxygen_per_fission_uo2`

```rust
pub use oxygen::oxygen_per_fission_uo2;
```

#### Re-export `HeatingRegime`

```rust
pub use oxygen::HeatingRegime;
```

#### Re-export `OPF_MAX`

```rust
pub use oxygen::OPF_MAX;
```

#### Re-export `internal_gas_pressure`

```rust
pub use pressure::internal_gas_pressure;
```

#### Re-export `GAS_CONSTANT_J_PER_MOL_K`

```rust
pub use pressure::GAS_CONSTANT_J_PER_MOL_K;
```

#### Re-export `STABLE_FISSION_GAS_YIELD`

```rust
pub use pressure::STABLE_FISSION_GAS_YIELD;
```

#### Re-export `irradiated_strength`

```rust
pub use strength::irradiated_strength;
```

#### Re-export `irradiated_weibull_modulus`

```rust
pub use strength::irradiated_weibull_modulus;
```

#### Re-export `MIN_TENSILE_STRENGTH_MPA`

```rust
pub use strength::MIN_TENSILE_STRENGTH_MPA;
```

#### Re-export `MIN_WEIBULL_MODULUS`

```rust
pub use strength::MIN_WEIBULL_MODULUS;
```

#### Re-export `induced_stress`

```rust
pub use stress::induced_stress;
```

#### Re-export `induced_stress_exact`

```rust
pub use stress::induced_stress_exact;
```

#### Re-export `induced_stress_with_thinning_factor`

```rust
pub use stress::induced_stress_with_thinning_factor;
```

#### Re-export `weibull_failure_fraction`

```rust
pub use weibull::weibull_failure_fraction;
```

## Re-exports

### Re-export `Nuclide`

import the nuclide enum

```rust
pub use fission_yields_data::prelude::Nuclide;
```

### Re-export `fission_yields_data::prelude::Nuclide::*`

import all nuclides into this crate

```rust
pub use fission_yields_data::prelude::Nuclide::*;
```

