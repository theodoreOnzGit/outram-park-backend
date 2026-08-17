# Crate Documentation

**Version:** 0.1.3

**Format Version:** 61

# Module `boon_lay`

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

- `Other("#[attr = CfgTrace([Not(NameValue { name: \"target_os\", value: Some(\"android\"), span: crates/boon-lay/src/lib.rs:36:11: 36:32 (#0) }, crates/boon-lay/src/lib.rs:36:10: 36:33 (#0))])]")`

Optional wgpu GPU acceleration for large Walk-on-Spheres ensembles. Compiled
only off Android (the workspace GPU/Android rule); the CPU rayon path in
`lagrangian_diffusion::first_passage::ensemble` is always available and is the
trusted reference. See the module docs for the CPU-fallback contract.
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
| [`normal_operation`](crate::triso_atops_fork::normal_operation) | Per-node normal-operation orchestration ([`normal_operation_node`](crate::triso_atops_fork::normal_operation::normal_operation_node)) composing the whole chain to curies (bead op-b4a.2.2, done). The JSON run-file driver + accident case remain scaffolded (bead op-b4a.2.3). |

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

#### Re-export `supported_nuclides`

```rust
pub use nuclide_database::supported_nuclides;
```

#### Re-export `TRISO_ATOPS_NUCLIDE_COUNT`

```rust
pub use nuclide_database::TRISO_ATOPS_NUCLIDE_COUNT;
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

Curies are **not** an SI unit and are not part of `uom`; convert with
[`becquerels_from_curies`] / [`curies_from_becquerels`] and the [`BQ_PER_CI`]
constant.

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

## Still scaffolded — the JSON run-file driver (bead op-b4a.2.3)

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

