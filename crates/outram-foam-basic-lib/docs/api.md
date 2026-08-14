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

## Module `compute`

Cross-layer — where a numerical kernel runs (`ComputeBackend`: serial,
multi-CPU via `rayon`, or GPU via `wgpu`), the worker-count policy, and the
backend-selection rule. Dispatch only, no kernels.
Where a numerical kernel runs — serial, multi-CPU, or GPU.

# What belongs in this module

The **dispatch layer and nothing else**: the [`ComputeBackend`] enum, the
worker-count policy [`ThreadCount`], the named backend-selection policy
[`select_backend`], and the GPU adapter probe. It is deliberately free of
numerical code so that every kernel in the crate answers the question
"where do I run?" the same way.

# What does NOT belong here

Kernels. A root finder, a sparse matrix-vector product, a quadrature rule
or an ODE ensemble lives in its own module (`polynomial/`, `ldu_matrix/`,
`krylov/`, `math/`, `ode/`) and takes a [`ComputeBackend`] as a parameter.
If you find yourself writing physics or arithmetic in this file, it is in
the wrong file.

# Hybrid means dispatch, not two APIs

A kernel exposes **one** public entry point with the backend as a
parameter. It does not grow a `foo_parallel()` sibling beside `foo()`.
Callers that do not care let [`select_backend`] resolve one for them;
callers that do care name the backend explicitly and get exactly it, or a
documented fallback. There is no `Auto` variant — resolution is a function,
not a backend.

# The serial path is the oracle

[`ComputeBackend::Serial`] is always compiled, on every target, under every
feature combination, and is the `#[default]`. It is the reference
implementation that the other two are checked against, and it is the only
one guaranteed to be bit-for-bit reproducible run to run. Verification and
validation are judged against it. The other backends are *acceleration*.

# Cargo features — both OFF by default

| Feature | Adds | Backend enabled |
|---|---|---|
| *(none)* | — | [`ComputeBackend::Serial`] |
| `parallel` | `rayon` | [`ComputeBackend::CpuMulti`] |
| `gpu` | `wgpu` (non-Android only) | [`ComputeBackend::Gpu`] |

A default build pulls neither dependency, which keeps the crate
dependency-light and Android/Termux clean. Requesting a backend whose
feature is off is **not an error**: it resolves down to the best available
one (see [`ComputeBackend::resolve`]). That way a caller written on a
desktop with `--features gpu` still runs, correctly and more slowly, on a
phone with no features at all.

# Android / Termux

`rayon` is pure Rust with no system component, so the `parallel` feature is
**not** target-gated — it works on Android. `wgpu` is target-gated off
Android in `Cargo.toml`, because Android has no system Vulkan/Metal loader
and the workspace Android rule forbids GPU dependencies in a library build.
Enabling `gpu` on an Android target therefore yields a build with no GPU
backend rather than a broken one.

# Planned: hybrid CPU+GPU co-execution (NOT implemented)

There is no `Hybrid` variant and no GPU kernel in this crate — see
[`gpu_adapter_present`] for the extent of the `wgpu` use. One policy
decision for it is nevertheless already settled, because it constrains how
the kernels are written.

For the **iterative** kernels the shape is *coarse-to-fine*, not a split
batch: a GPU `f32` pass produces only the **initial guess**, and the CPU
converges it to the final answer at `f64`. The result is therefore entirely
`f64` and the `f32` accuracy floors govern the guess, not the answer. This
suits batched root finding especially well — [`RootProblem::guess`] already
guarantees that a bad guess is a performance problem and not a correctness
one, and Newton's quadratic convergence turns an `f32`-accurate start into
a `f64` answer in about two iterations.

[`RootProblem::guess`]: crate::math::parallel::RootProblem::guess

For the **elementwise** kernels, where a batch genuinely is split across the
two devices, the CPU half runs `f32` to match the GPU, so that precision is
uniform across one output array rather than varying with the split ratio.
Those kernels are an `f32`-class throughput path sitting roughly `1e-7`
relative from the [`ComputeBackend::Serial`] oracle.

Numerical differentiation is excluded from both shapes: a finite difference
has no fixed point to converge to, so there is no guess to seed and the
`f32` floor would land directly on the answer.

Full reasoning, the per-kernel floors at `f32`, the measured iteration
savings, and the caveats none of this resolves are in
`docs/hybrid-precision-policy.md`.

# Units

Nothing in this module carries a physical dimension. Sizes are counts of
independent work items, which is a pure number. `uom` typing belongs on the
kernels' own signatures and must not be stripped to get data onto a device —
convert at the buffer boundary and convert back.

[`ComputeBackend`]: crate::compute::ComputeBackend
[`ComputeBackend::Serial`]: crate::compute::ComputeBackend::Serial
[`ComputeBackend::CpuMulti`]: crate::compute::ComputeBackend::CpuMulti
[`ComputeBackend::Gpu`]: crate::compute::ComputeBackend::Gpu
[`ComputeBackend::resolve`]: crate::compute::ComputeBackend::resolve
[`ThreadCount`]: crate::compute::ThreadCount
[`select_backend`]: crate::compute::select_backend
[`gpu_adapter_present`]: crate::compute::gpu_adapter_present

```rust
pub mod compute { /* ... */ }
```

### Types

#### Enum `ComputeBackend`

Which execution backend a numerical kernel should use.

# Variants and what they cost

- [`Serial`](Self::Serial) — one thread, scalar. Always available. The
  deterministic trusted reference; bit-for-bit reproducible run to run.
- [`CpuMulti`](Self::CpuMulti) — `rayon` across CPU cores. Requires the
  `parallel` feature. Pays thread-pool overhead, so it loses on small
  problems; see [`CPU_MULTI_MIN_WORK_ITEMS`]. Reduction order is not
  generally reproducible unless a kernel says otherwise.
- [`Gpu`](Self::Gpu) — `wgpu` compute. Requires the `gpu` feature, a
  non-Android target, and an actual adapter at run time. Pays a host-device
  transfer per dispatch, so it needs a much larger problem to win; see
  [`GPU_MIN_WORK_ITEMS`]. WGSL has no `f64`, so a GPU kernel that computes
  in `f32` will NOT match the serial `f64` path to full precision — each
  kernel must document its own measured deviation.

# Choosing one

Prefer [`select_backend`] over hand-picking. If you do name a backend,
route it through [`resolve`](Self::resolve) so an unavailable choice
degrades instead of failing.

# Units

Dimensionless — this is a mode selector, not a quantity.

```rust
pub enum ComputeBackend {
    Serial,
    CpuMulti,
    Gpu,
}
```

##### Variants

###### `Serial`

Single-threaded scalar execution — always present, always the oracle.

###### `CpuMulti`

Multi-threaded CPU execution via `rayon` (`parallel` feature).

###### `Gpu`

GPU compute via `wgpu` (`gpu` feature, non-Android, adapter present).

##### Implementations

###### Methods

- ```rust
  pub fn is_available(self: Self) -> bool { /* ... */ }
  ```
  Whether this backend can actually run in this build, on this target,

- ```rust
  pub fn resolve(self: Self) -> Self { /* ... */ }
  ```
  Degrade this backend to the best one that is actually available,

- ```rust
  pub fn label(self: Self) -> &'static str { /* ... */ }
  ```
  A short human-readable label, for benchmark tables and log lines.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> ComputeBackend { /* ... */ }
    ```

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
    fn default() -> ComputeBackend { /* ... */ }
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
    fn eq(self: &Self, other: &ComputeBackend) -> bool { /* ... */ }
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
#### Enum `ThreadCount`

How many worker threads [`ComputeBackend::CpuMulti`] should use.

Mirrors the established shape in `boon-lay`'s and `outram-mc-libs`'s own
compute modules so the workspace has one vocabulary for this rather than
three.

# Units

Dimensionless counts. [`Fraction`](Self::Fraction) is a pure ratio.

```rust
pub enum ThreadCount {
    Auto,
    Fixed(usize),
    Fraction(f64),
}
```

##### Variants

###### `Auto`

Every logical core, via [`std::thread::available_parallelism`], falling
back to 1 if the query fails. The default.

###### `Fixed`

An explicit worker count, clamped up to a minimum of 1.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

###### `Fraction`

A fraction of the available logical cores, e.g. `0.5` for half. The
product is rounded to nearest and clamped to at least 1. A non-finite
or non-positive fraction clamps to 1 rather than panicking.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn resolve(self: Self) -> usize { /* ... */ }
  ```
  Resolve to a concrete worker-thread count, always `>= 1`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &ThreadCount) -> bool { /* ... */ }
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

#### Function `select_backend`

**Attributes:**

- `MustUse { reason: None }`

**The** backend-selection policy — one named function, so a reader can find
the rule instead of hunting for scattered `if n > 1024` tests.

# The rule

Given `work_items`, the number of *independent* items a kernel will process:

1. If `work_items >= ` [`GPU_MIN_WORK_ITEMS`] and the GPU is available, use
   [`ComputeBackend::Gpu`].
2. Otherwise if `work_items >= ` [`CPU_MULTI_MIN_WORK_ITEMS`] and the
   `parallel` feature is on, use [`ComputeBackend::CpuMulti`].
3. Otherwise use [`ComputeBackend::Serial`].

# Arguments

- `work_items` — count of independent work items (cells, faces, roots,
  quadrature points). Dimensionless. Pass the honest count: passing a
  matrix's row count when the kernel actually loops over its faces will
  pick the wrong backend.

# Returns

A backend that is guaranteed available, so the caller may use it directly
without a further [`ComputeBackend::resolve`].

# Why a caller might not use this

Benchmarks and parity gates need to run a *named* backend regardless of
size — that is what [`ComputeBackend::resolve`] is for. This function is
for production callers that just want the fastest correct option.

# Example

```
use outram_foam_basic_lib::compute::{select_backend, ComputeBackend};

// A tiny problem always runs serially, whatever features are enabled.
assert_eq!(select_backend(16), ComputeBackend::Serial);

// Whatever it picks for a large problem is guaranteed to be runnable.
assert!(select_backend(1_000_000).is_available());
```

```rust
pub fn select_backend(work_items: usize) -> ComputeBackend { /* ... */ }
```

#### Function `gpu_adapter_present`

**Attributes:**

- `MustUse { reason: None }`

Whether a usable GPU compute adapter exists, probed once and cached.

# Behaviour

- Without the `gpu` feature, or on Android, this is `false` with no probe
  attempted.
- Otherwise it asks `wgpu` for an adapter, once. `false` is a normal
  outcome — headless CI, a container, a machine with no discrete GPU — and
  is **not** an error. Nothing here panics or returns `Err`.

# Returns

`true` only if a GPU kernel could actually be dispatched.

```rust
pub fn gpu_adapter_present() -> bool { /* ... */ }
```

### Constants and Statics

#### Constant `CPU_MULTI_MIN_WORK_ITEMS`

The number of work items below which [`ComputeBackend::CpuMulti`] is not
worth its thread-pool overhead, and [`select_backend`] returns
[`ComputeBackend::Serial`] instead.

# How this number was chosen

**It is a placeholder, it has NOT been justified by measurement, and the
one kernel family that has since been measured found it far too low.**
Bead `op-yvj.4.7` covers the crossover benchmarks that will replace it.

The failure mode this guards is real and easy to reproduce: spawning
threads to add two 50-element vectors is slower than doing it serially.

# Measured counter-evidence — read before relying on this

[`crate::fields::parallel`] measured its own crossover on 2026-08-12 (4
logical cores, release, `--features parallel`) and found that **at exactly
this threshold the parallel path was about 6x SLOWER** — `0.17x` speedup at
`n = 4096`, and it lost 10 sweeps out of 10 at every size below 65 536.
Those kernels are memory-bandwidth bound at 1-2 flops per element, so
thread overhead dominates much further out than it would for compute-dense
work. That module therefore overrides this constant at 131 072 via
[`crate::fields::parallel::field_parallel_crossover`].

[`crate::ldu_matrix::parallel`] measured the same day and found the
opposite for *its* sparse matrix-vector product: 4096 cells is precisely
where that kernel breaks even, so `SPMV_MIN_CELLS` keeps this value. But
the vector operations in that same module want `VECOP_MIN_ELEMENTS =
262 144`, because `axpy` at 4096 elements runs at `0.05x` — twenty times
slower.

[`crate::math::parallel`] then measured the opposite direction:
batched root finding crosses over at **256** problems, 16x *below* this
constant, because it is compute-dense per lane where the field kernels are
memory-bound.

Four kernel families have now been measured and they want **256, 4 096,
131 072 and 262 144** — a 1024x spread, with this constant sitting in the
middle and wrong at both ends. That is the real finding: no single
crate-wide threshold can be right for all of them.

Worse for any hope of a universal number: the root-finding crossover
depends on the *caller's* residual cost, which this crate cannot know. A
cheap residual and an expensive one cross over at different sizes with
identical kernel code.

The honest reading: this constant is a *floor against absurdity*, not a
tuned value, and a kernel that has not measured its own crossover should
not assume this one is safe for it. Per-kernel overrides are expected, not
exceptional.

# Units

A count of independent work items (cells, faces, roots, quadrature points),
dimensionless.

```rust
pub const CPU_MULTI_MIN_WORK_ITEMS: usize = 4_096;
```

#### Constant `GPU_MIN_WORK_ITEMS`

The number of work items below which [`ComputeBackend::Gpu`] is not worth
the host-device transfer, and [`select_backend`] falls back to the best
available CPU backend.

# How this number was chosen

**Placeholder awaiting measurement — see [`CPU_MULTI_MIN_WORK_ITEMS`].**
It is set an order of magnitude above the CPU-multi threshold because a GPU
dispatch pays for a round trip across the PCIe bus in addition to kernel
launch, so it needs correspondingly more work to amortise.

Note that this workspace's own experience argues for caution rather than
optimism about GPU wins: beads `op-u6s.8` and `op-u6s.9` record GPU
transport work struggling to beat a good multi-threaded CPU path, and
`op-u6s.9` is still open.

# Units

A count of independent work items, dimensionless.

```rust
pub const GPU_MIN_WORK_ITEMS: usize = 65_536;
```

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
- [`parallel`](crate::fields::parallel) — the same element-wise algebra and
  the field reductions, dispatched on
  [`ComputeBackend`](crate::compute::ComputeBackend) so a large mesh can use
  every CPU core. One entry point per operation; multi-threading is behind the
  crate's `parallel` feature and the serial path is the trusted reference.

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
## Module `parallel`

Backend-dispatched kernels for the element-wise field algebra.

# What this module is for

A finite-volume solver spends most of a timestep doing *element-wise*
arithmetic on fields that carry one value per mesh cell or per mesh face:
adding an explicit source to a residual, scaling by a relaxation factor,
forming `rho*U`, advancing `phi += dt*ddt(phi)`, and taking a norm to decide
whether the outer loop has converged. Each of those touches every cell,
several times per timestep, so on a large mesh they are the operations worth
spreading across CPU cores.

**The arithmetic is identical to the serial operators** in
[`crate::fields::field`], [`crate::fields::vol_field`] and
[`crate::fields::surface_field`]. Only the execution strategy differs — and,
for the *reductions* only, the order in which floating-point values are
summed (see "Reduction determinism" below).

# One entry point per operation

Hybrid execution here means **dispatch, not two APIs**. There is exactly one
public function per operation and the backend is a parameter:

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::fields::field::Field;
use outram_foam_basic_lib::fields::parallel;

let a = Field::uniform(1_000, 2.0_f64);
let b = Field::uniform(1_000, 3.0_f64);

// Same function, different backend — no `add_parallel` twin exists.
let c_ref = parallel::add(ComputeBackend::Serial, &a, &b);
let c_mt = parallel::add(ComputeBackend::CpuMulti, &a, &b);
assert_eq!(c_ref.as_slice(), c_mt.as_slice());
```

The size threshold that decides whether [`ComputeBackend::CpuMulti`] actually
spreads the work is **one named, documented function** —
[`should_parallelise`], reading [`field_parallel_crossover`] — never an
`if n > …` scattered through the operators. Retuning the policy is a
one-place edit.

# Cargo features

Multi-threading lives behind the crate's **`parallel`** feature (rayon),
which is **off by default** so the default build stays dependency-light and
Android-clean. The public API in this module is the *same either way*: with
the feature off, [`ComputeBackend::CpuMulti`] and [`ComputeBackend::Gpu`]
transparently run the serial path and every result is unchanged. Nothing
here is target-gated — rayon is pure Rust, so with `--features parallel` this
module builds and runs on Android/Termux (`target_os = "android"`) too;
[`std::thread::available_parallelism`] works there and a phone simply
reports fewer cores.

# The `Gpu` backend

There is **no GPU kernel for field algebra yet**. [`ComputeBackend::Gpu`] is
accepted and routed to the best available CPU path (multi-threaded when the
`parallel` feature is on, serial otherwise). This is stated rather than
silently pretended; when a GPU field kernel exists it is wired in here and
nothing about the call sites changes.

# Which thread pool

The kernels use rayon's **global** pool. A caller that wants a dedicated,
explicitly sized pool does not need a second API — bind one with
`rayon::ThreadPool::install` and every call inside routes to it:

```text
// Sketch — requires the `parallel` feature, so it is shown rather than run
// as a doctest (rayon is not a dependency of the default build at all).
let pool = rayon::ThreadPoolBuilder::new().num_threads(8).build().unwrap();
pool.install(|| parallel::axpy_assign(ComputeBackend::CpuMulti, &mut y, dt, &ddt));
```

The `vv_reduction_is_independent_of_thread_count` test in this module does
exactly this and asserts the answer is unchanged.

The reductions are written so that the answer does **not** depend on which
pool or how many threads it has — see below.

# Units

This layer is deliberately **unit-agnostic**, exactly like the serial field
layer it mirrors (see the [`crate::fields`] module docs): a [`Field`] carries
bare `f64` / [`Vector3`] / [`Tensor`](crate::primitives::Tensor) /
[`SymmTensor`](crate::primitives::SymmTensor) values in whatever SI units the
caller assigned them. No `uom` quantity is stripped here because none is
present at this layer — this crate's `uom` discipline lives in the
thermophysics layer and nothing in this module weakens it. Each function's
doc states the units it implies. The one place a physical unit is
unavoidable is [`vol_integral`], which multiplies by the mesh's cell volumes
in `m^3` and therefore returns `[phi]*m^3`.

# Field `name` strings never grow here

Every operation that returns a [`VolField`] or [`SurfaceField`] copies the
**left operand's** `name` verbatim and never composes a new one. This is not
a style preference. A solver that reassigns a persistent field from an
expression containing itself (`rho = rho + div(phi)`) would double the `name`
string every timestep under compositional naming — `2^step` growth, invisible
in the field data. That exact bug once drove this crate's
`compressible_lid_cavity` test to 24 GB and a SIGTERM. See the crate
`CLAUDE.md` "Critical translation gotcha", the matching notes in
[`crate::fields::vol_field`], and the `name_does_not_grow_*` regression tests
in this module, which reassign a field from an expression containing itself
64 times and assert the name length is unchanged.

# Reduction determinism

The reductions ([`sum`], [`l2_norm`], [`dot`], [`vol_integral`], …) use a
**fixed-chunk tree reduction**: the slice is cut into consecutive chunks of
exactly [`REDUCTION_CHUNK`] elements, each chunk is summed sequentially in
index order, and the per-chunk partial sums are then combined sequentially in
index order on the calling thread.

That buys a strong and deliberate guarantee:

- the parallel reduction depends **only** on the data and on the
  compile-time constant [`REDUCTION_CHUNK`];
- it is therefore **bit-reproducible run to run** and **identical for any
  thread count** — unlike a work-stealing `par_iter().sum()`, whose
  accumulation tree depends on how rayon happened to split the work that
  run;
- it is **not** bit-identical to the serial left-to-right sum, because
  floating-point addition is not associative. The two differ in the last
  bits; the measured worst-case deviation is recorded in the V&V test
  `vv_parallel_sum_matches_serial_within_tolerance`.

[`min`] and [`max`] *are* bit-identical to the serial fold, because `min` and
`max` are associative.

[`ComputeBackend::Serial`](crate::compute::ComputeBackend::Serial) is the **deterministic trusted reference** — the
oracle every parallel result in this module is checked against, matching the
convention in `outram-mc-libs` (`src/physics/compute.rs`) and `boon-lay`
(`src/compute.rs`).

# Crossover: parallel is *slower* on small fields

Handing a 50-element addition to four threads loses — the dispatch costs more
than the arithmetic. [`should_parallelise`] therefore falls back to the
serial path below [`field_parallel_crossover`], whose value was **measured on
real hardware**, not guessed; see [`FIELD_PARALLEL_CROSSOVER`] for the table.

```rust
pub mod parallel { /* ... */ }
```

### Functions

#### Function `field_parallel_crossover`

The element count at or above which multi-threading is worth its overhead for
the field-algebra kernels.

Returns [`FIELD_PARALLEL_CROSSOVER`] — the **measured** field-kernel
crossover, deliberately overriding the documented placeholder
[`crate::compute::CPU_MULTI_MIN_WORK_ITEMS`] (see that constant's docs for
why the override exists and the numbers behind it).

It is a *function*, not a bare constant, so the policy can later consult a
runtime-configured value without touching a single operator.

**Units:** a count of field elements — cells for a volume field, internal
faces for a surface field. Dimensionless.

```rust
pub fn field_parallel_crossover() -> usize { /* ... */ }
```

#### Function `should_parallelise`

The one place that decides Serial vs multi-threaded execution for an
element-wise field operation over `n` elements.

Every operator in this module routes through this function; none of them
contains its own size test. It returns `true` only when **all** of:

- `backend` is [`ComputeBackend::CpuMulti`] or [`ComputeBackend::Gpu`]
  (there is no GPU field kernel yet, so `Gpu` asks for the best CPU path);
- [`ComputeBackend::resolve`] confirms `CpuMulti` is actually available, i.e.
  the crate was built with the **`parallel`** feature — availability is asked
  of [`crate::compute`], never re-implemented here;
- `n >= field_parallel_crossover()`.

`n` is counted in field elements — cells for a volume field, internal faces
for a surface field. Dimensionless.

# Relationship to [`crate::compute::select_backend`]

`select_backend` answers "which backend should I ask for?" from a work-item
count; this function answers "given the backend I was asked for, is this
particular field big enough to be worth threading?" using the **measured**
field-kernel crossover rather than the workspace placeholder. A caller may use
either; passing `select_backend(n)` straight in works and is safe, because
this function re-checks the size against the measured threshold.

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::fields::parallel::{field_parallel_crossover, should_parallelise};

// Serial never parallelises, whatever the size.
assert!(!should_parallelise(ComputeBackend::Serial, 100_000_000));
// Nor does any backend on a tiny field.
assert!(!should_parallelise(ComputeBackend::CpuMulti, 8));
// Above the crossover it depends only on whether `parallel` is enabled.
assert_eq!(
    should_parallelise(ComputeBackend::CpuMulti, field_parallel_crossover()),
    cfg!(feature = "parallel"),
);
```

```rust
pub fn should_parallelise(backend: crate::compute::ComputeBackend, n: usize) -> bool { /* ... */ }
```

#### Function `add`

Element-wise sum `c[i] = a[i] + b[i]`, returning a new field.

Generic over the element type: works for `Field<f64>`, `Field<Vector3>`,
`Field<Tensor>` and `Field<SymmTensor>`, all of which are `Copy`.

**Units:** both operands must carry the same physical quantity (this layer
stores bare numbers and cannot check that); the result carries it too.

Every output element is computed by the same expression as the serial
[`std::ops::Add`] impl on [`Field`], so the result is **bit-identical** to
[`ComputeBackend::Serial`](crate::compute::ComputeBackend::Serial) on any backend and any thread count.

# Panics

Panics if `a.len() != b.len()`, matching the serial operator.

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::fields::field::Field;
use outram_foam_basic_lib::fields::parallel;

let a = Field::new(vec![1.0, 2.0, 3.0]);
let b = Field::new(vec![10.0, 20.0, 30.0]);
let c = parallel::add(ComputeBackend::CpuMulti, &a, &b);
assert_eq!(c.as_slice(), &[11.0, 22.0, 33.0]);
```

```rust
pub fn add<T>(backend: crate::compute::ComputeBackend, a: &crate::fields::field::Field<T>, b: &crate::fields::field::Field<T>) -> crate::fields::field::Field<T>
where
    T: Copy + Send + Sync + Add<Output = T> { /* ... */ }
```

#### Function `sub`

Element-wise difference `c[i] = a[i] - b[i]`, returning a new field.

**Units:** as for [`add`] — both operands are the same physical quantity.
Bit-identical to the serial [`std::ops::Sub`] impl on [`Field`].

# Panics

Panics if `a.len() != b.len()`.

```rust
pub fn sub<T>(backend: crate::compute::ComputeBackend, a: &crate::fields::field::Field<T>, b: &crate::fields::field::Field<T>) -> crate::fields::field::Field<T>
where
    T: Copy + Send + Sync + Sub<Output = T> { /* ... */ }
```

#### Function `scale`

Uniform scaling `c[i] = a[i] * s`, returning a new field.

Used for under-relaxation (`s = alpha`, dimensionless), for `1/dt` weighting
(`s` in `s^-1`), and for negation (`s = -1.0`).

**Units:** the result carries `[a] * [s]`; this layer treats `s` as a bare
number and cannot check it. Bit-identical to the serial
[`std::ops::Mul<f64>`] impl on [`Field`].

```rust
pub fn scale<T>(backend: crate::compute::ComputeBackend, a: &crate::fields::field::Field<T>, s: f64) -> crate::fields::field::Field<T>
where
    T: Copy + Send + Sync + Mul<f64, Output = T> { /* ... */ }
```

#### Function `axpy`

Fused combination `c[i] = y[i] + a * x[i]` ("axpy"), returning a new field.

This is the single most common shape in a solver timestep — adding a scaled
explicit source, applying an under-relaxed correction, or advancing an
explicit Euler update `phi_new = phi + dt * ddt(phi)`. It is one fused pass,
so the fields are traversed once instead of twice; that matters because these
kernels are memory-bandwidth bound.

**Units:** `y` and `a*x` must be the same physical quantity.

# Panics

Panics if `y.len() != x.len()`.

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::fields::field::Field;
use outram_foam_basic_lib::fields::parallel;

let y = Field::new(vec![1.0, 1.0]);
let x = Field::new(vec![4.0, 6.0]);
let c = parallel::axpy(ComputeBackend::Serial, &y, 0.5, &x);
assert_eq!(c.as_slice(), &[3.0, 4.0]);
```

```rust
pub fn axpy<T>(backend: crate::compute::ComputeBackend, y: &crate::fields::field::Field<T>, a: f64, x: &crate::fields::field::Field<T>) -> crate::fields::field::Field<T>
where
    T: Copy + Send + Sync + Add<Output = T> + Mul<f64, Output = T> { /* ... */ }
```

#### Function `add_assign`

In-place accumulation `y[i] += x[i]`.

Allocation-free — the preferred form inside a timestep loop, where the
out-of-place [`add`] allocates a fresh `Vec` every call.

**Units:** `y` and `x` are the same physical quantity.

# Panics

Panics if `y.len() != x.len()`.

```rust
pub fn add_assign<T>(backend: crate::compute::ComputeBackend, y: &mut crate::fields::field::Field<T>, x: &crate::fields::field::Field<T>)
where
    T: Copy + Send + Sync + Add<Output = T> { /* ... */ }
```

#### Function `sub_assign`

In-place subtraction `y[i] -= x[i]`. Allocation-free.

**Units:** `y` and `x` are the same physical quantity.

# Panics

Panics if `y.len() != x.len()`.

```rust
pub fn sub_assign<T>(backend: crate::compute::ComputeBackend, y: &mut crate::fields::field::Field<T>, x: &crate::fields::field::Field<T>)
where
    T: Copy + Send + Sync + Sub<Output = T> { /* ... */ }
```

#### Function `scale_assign`

In-place uniform scaling `y[i] *= s`. Allocation-free.

**Units:** the field becomes `[y] * [s]`; `s` is a bare number here.

```rust
pub fn scale_assign<T>(backend: crate::compute::ComputeBackend, y: &mut crate::fields::field::Field<T>, s: f64)
where
    T: Copy + Send + Sync + Mul<f64, Output = T> { /* ... */ }
```

#### Function `axpy_assign`

In-place fused combination `y[i] += a * x[i]` ("axpy"). Allocation-free.

The hot-loop form of [`axpy`]: one traversal, no allocation. This is the
shape an explicit transient update takes every timestep.

**Units:** `y` and `a*x` are the same physical quantity.

# Panics

Panics if `y.len() != x.len()`.

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::fields::field::Field;
use outram_foam_basic_lib::fields::parallel;
use outram_foam_basic_lib::primitives::Vector3;

let mut u = Field::uniform(3, Vector3::new(1.0, 0.0, 0.0));   // [m/s]
let du = Field::uniform(3, Vector3::new(0.0, 2.0, 0.0));      // [m/s^2]
parallel::axpy_assign(ComputeBackend::CpuMulti, &mut u, 0.5, &du);  // dt = 0.5 s
assert_eq!(u[0], Vector3::new(1.0, 1.0, 0.0));
```

```rust
pub fn axpy_assign<T>(backend: crate::compute::ComputeBackend, y: &mut crate::fields::field::Field<T>, a: f64, x: &crate::fields::field::Field<T>)
where
    T: Copy + Send + Sync + Add<Output = T> + Mul<f64, Output = T> { /* ... */ }
```

#### Function `pointwise_mul`

Element-wise product of two **scalar** fields, `c[i] = a[i] * b[i]`.

The workhorse behind `rho*h`, and behind any coefficient-times-field
assembly. **Units multiply:** the result carries `[a]*[b]`.

Bit-identical to [`Field::pointwise_mul`].

# Panics

Panics if `a.len() != b.len()`.

```rust
pub fn pointwise_mul(backend: crate::compute::ComputeBackend, a: &crate::fields::field::Field<f64>, b: &crate::fields::field::Field<f64>) -> crate::fields::field::Field<f64> { /* ... */ }
```

#### Function `pointwise_div`

Element-wise quotient of two **scalar** fields, `c[i] = a[i] / b[i]`.

**Units divide:** the result carries `[a]/[b]`. Division by zero yields
`+/-inf` or `NaN` exactly as `f64` division does — no guard is applied,
matching [`Field::pointwise_div`].

# Panics

Panics if `a.len() != b.len()`.

```rust
pub fn pointwise_div(backend: crate::compute::ComputeBackend, a: &crate::fields::field::Field<f64>, b: &crate::fields::field::Field<f64>) -> crate::fields::field::Field<f64> { /* ... */ }
```

#### Function `scale_by_field`

Scale a vector/tensor field by a per-element scalar field:
`c[i] = v[i] * s[i]`.

This is `rho*U` (density field times velocity field) and every other
"coefficient field times ranked field" product. **Units multiply:**
`[v]*[s]`.

Bit-identical to [`Field::scale`] for `Field<Vector3>`.

# Panics

Panics if `v.len() != s.len()`.

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::fields::field::Field;
use outram_foam_basic_lib::fields::parallel;
use outram_foam_basic_lib::primitives::Vector3;

let u = Field::uniform(2, Vector3::new(1.0, 2.0, 3.0));   // [m/s]
let rho = Field::new(vec![2.0, 0.5]);                     // [kg/m^3]
let rho_u = parallel::scale_by_field(ComputeBackend::CpuMulti, &u, &rho); // [kg/(m^2 s)]
assert_eq!(rho_u[0], Vector3::new(2.0, 4.0, 6.0));
assert_eq!(rho_u[1], Vector3::new(0.5, 1.0, 1.5));
```

```rust
pub fn scale_by_field<T>(backend: crate::compute::ComputeBackend, v: &crate::fields::field::Field<T>, s: &crate::fields::field::Field<f64>) -> crate::fields::field::Field<T>
where
    T: Copy + Send + Sync + Mul<f64, Output = T> { /* ... */ }
```

#### Function `dot_field`

Element-wise dot product of two vector fields → scalar field:
`c[i] = a[i] . b[i]`.

**Units multiply:** `[a]*[b]`. Bit-identical to [`Field::dot_field`].

# Panics

Panics if `a.len() != b.len()`.

```rust
pub fn dot_field(backend: crate::compute::ComputeBackend, a: &crate::fields::field::Field<crate::primitives::Vector3>, b: &crate::fields::field::Field<crate::primitives::Vector3>) -> crate::fields::field::Field<f64> { /* ... */ }
```

#### Function `sum`

Sum of all elements, `sum_i x[i]`.

**Units:** the field's own units. An empty field sums to `0.0`.

# Determinism

Reproducible run to run and independent of thread count, but **not**
bit-identical to the serial [`Field::sum`] — the summation order differs. See
the module-level "Reduction determinism" section for the measured deviation.

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::fields::field::Field;
use outram_foam_basic_lib::fields::parallel;

assert_eq!(parallel::sum(ComputeBackend::Serial, &Field::new(vec![1.0, 2.0, 3.0])), 6.0);
assert_eq!(parallel::sum(ComputeBackend::CpuMulti, &Field::<f64>::new(vec![])), 0.0);
```

```rust
pub fn sum(backend: crate::compute::ComputeBackend, x: &crate::fields::field::Field<f64>) -> f64 { /* ... */ }
```

#### Function `mean`

Arithmetic mean of all elements; returns `0.0` for an empty field, matching
[`Field::mean`].

**Units:** the field's own units.

```rust
pub fn mean(backend: crate::compute::ComputeBackend, x: &crate::fields::field::Field<f64>) -> f64 { /* ... */ }
```

#### Function `min`

Smallest element. Returns `+inf` for an empty field, matching [`Field::min`].

**Units:** the field's own units. **Bit-identical** to the serial fold on
every backend, because `min` is associative.

```rust
pub fn min(backend: crate::compute::ComputeBackend, x: &crate::fields::field::Field<f64>) -> f64 { /* ... */ }
```

#### Function `max`

Largest element. Returns `-inf` for an empty field, matching [`Field::max`].

**Units:** the field's own units. **Bit-identical** to the serial fold on
every backend, because `max` is associative.

```rust
pub fn max(backend: crate::compute::ComputeBackend, x: &crate::fields::field::Field<f64>) -> f64 { /* ... */ }
```

#### Function `l2_norm`

Euclidean (L2) norm, `sqrt(sum_i x[i]^2)`.

This is the convergence measure a solver evaluates on the residual field
every outer iteration. **Units:** the field's own units. An empty field gives
`0.0`.

# Determinism

Same guarantee as [`sum`]: reproducible and thread-count independent, not
bit-identical to [`Field::l2_norm`].

```rust
pub fn l2_norm(backend: crate::compute::ComputeBackend, x: &crate::fields::field::Field<f64>) -> f64 { /* ... */ }
```

#### Function `dot`

Inner product of two scalar fields, `sum_i a[i]*b[i]`.

The Krylov-solver inner product, and — with cell volumes as `b` — the
volume-weighted integral. **Units multiply:** `[a]*[b]`.

# Panics

Panics if `a.len() != b.len()`.

# Determinism

Same guarantee as [`sum`].

```rust
pub fn dot(backend: crate::compute::ComputeBackend, a: &crate::fields::field::Field<f64>, b: &crate::fields::field::Field<f64>) -> f64 { /* ... */ }
```

#### Function `add_vol`

Sum of two volume fields — internal field **and** every boundary patch.

The result takes `a`'s `name`, `a`'s mesh, and `a`'s per-patch boundary
conditions, exactly like the serial [`std::ops::Add`] impl on [`VolField`].

**The name is copied, never composed.** See the module docs: composing names
here produces `2^step` string growth in a solver loop.

**Units:** both operands are the same physical quantity.

# Panics

Panics if the internal fields or any patch pair differ in length.

```rust
use std::sync::Arc;
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::fields::parallel;
use outram_foam_basic_lib::fields::vol_field::VolScalarField;
use outram_foam_basic_lib::mesh::fv_mesh::FvMesh;

let mesh = Arc::new(FvMesh::periodic_1d(8, 1.0, 1.0));
let a = VolScalarField::uniform("rho", mesh.clone(), 1.0);
let b = VolScalarField::uniform("drho", mesh, 0.25);
let c = parallel::add_vol(ComputeBackend::CpuMulti, &a, &b);
assert_eq!(c.name, "rho");                 // NOT "(rho + drho)"
assert!((c.internal[0] - 1.25).abs() < 1e-15);
```

```rust
pub fn add_vol<T>(backend: crate::compute::ComputeBackend, a: &crate::fields::vol_field::VolField<T>, b: &crate::fields::vol_field::VolField<T>) -> crate::fields::vol_field::VolField<T>
where
    T: Copy + Default + Send + Sync + Add<Output = T> { /* ... */ }
```

#### Function `sub_vol`

Difference of two volume fields — internal field and every boundary patch.

Name, mesh, and boundary conditions come from `a`; the name is copied, never
composed. **Units:** both operands are the same physical quantity.

# Panics

Panics if the internal fields or any patch pair differ in length.

```rust
pub fn sub_vol<T>(backend: crate::compute::ComputeBackend, a: &crate::fields::vol_field::VolField<T>, b: &crate::fields::vol_field::VolField<T>) -> crate::fields::vol_field::VolField<T>
where
    T: Copy + Default + Send + Sync + Sub<Output = T> { /* ... */ }
```

#### Function `scale_vol`

Uniform scaling of a volume field, `c = a * s` — internal field and every
boundary patch.

Name, mesh, and boundary conditions come from `a`. **Units:** `[a]*[s]`.

```rust
pub fn scale_vol<T>(backend: crate::compute::ComputeBackend, a: &crate::fields::vol_field::VolField<T>, s: f64) -> crate::fields::vol_field::VolField<T>
where
    T: Copy + Default + Send + Sync + Mul<f64, Output = T> { /* ... */ }
```

#### Function `add_vol_assign`

In-place accumulation on a volume field, `y += x` — internal field and every
boundary patch.

Allocation-free and **name-preserving by construction**: `y.name` is never
touched. **Units:** `y` and `x` are the same physical quantity.

# Panics

Panics if the internal fields or any patch pair differ in length.

```rust
pub fn add_vol_assign<T>(backend: crate::compute::ComputeBackend, y: &mut crate::fields::vol_field::VolField<T>, x: &crate::fields::vol_field::VolField<T>)
where
    T: Copy + Send + Sync + Add<Output = T> { /* ... */ }
```

#### Function `sub_vol_assign`

In-place subtraction on a volume field, `y -= x`. Allocation-free,
name-preserving.

**Units:** `y` and `x` are the same physical quantity.

# Panics

Panics if the internal fields or any patch pair differ in length.

```rust
pub fn sub_vol_assign<T>(backend: crate::compute::ComputeBackend, y: &mut crate::fields::vol_field::VolField<T>, x: &crate::fields::vol_field::VolField<T>)
where
    T: Copy + Send + Sync + Sub<Output = T> { /* ... */ }
```

#### Function `scale_vol_assign`

In-place uniform scaling of a volume field, `y *= s`. Allocation-free,
name-preserving. **Units:** the field becomes `[y]*[s]`.

```rust
pub fn scale_vol_assign<T>(backend: crate::compute::ComputeBackend, y: &mut crate::fields::vol_field::VolField<T>, s: f64)
where
    T: Copy + Send + Sync + Mul<f64, Output = T> { /* ... */ }
```

#### Function `axpy_vol_assign`

In-place fused update on a volume field, `y += a * x` — internal field and
every boundary patch.

This is the explicit-update shape a transient solver runs on every prognostic
field every timestep (`rho += dt * ddt(rho)`). Allocation-free and
name-preserving — the form that structurally cannot reproduce the
`name`-growth bug, because it never constructs a new field.

**Units:** `y` and `a*x` are the same physical quantity.

# Panics

Panics if the internal fields or any patch pair differ in length.

```rust
use std::sync::Arc;
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::fields::parallel;
use outram_foam_basic_lib::fields::vol_field::VolScalarField;
use outram_foam_basic_lib::mesh::fv_mesh::FvMesh;

let mesh = Arc::new(FvMesh::periodic_1d(16, 1.0, 1.0));
let mut rho = VolScalarField::uniform("rho", mesh.clone(), 1.0);
let ddt = VolScalarField::uniform("ddt(rho)", mesh, 2.0);

for _ in 0..10 {
    parallel::axpy_vol_assign(ComputeBackend::CpuMulti, &mut rho, 0.1, &ddt); // dt = 0.1
}
assert!((rho.internal[0] - 3.0).abs() < 1e-12);
assert_eq!(rho.name, "rho");        // still 3 characters after 10 steps
```

```rust
pub fn axpy_vol_assign<T>(backend: crate::compute::ComputeBackend, y: &mut crate::fields::vol_field::VolField<T>, a: f64, x: &crate::fields::vol_field::VolField<T>)
where
    T: Copy + Send + Sync + Add<Output = T> + Mul<f64, Output = T> { /* ... */ }
```

#### Function `add_surface`

Sum of two surface fields — internal faces and every boundary patch.

Name, mesh, and boundary conditions come from `a`; the name is copied, never
composed. **Units:** both operands are the same physical quantity.

# Panics

Panics if the internal fields or any patch pair differ in length.

```rust
pub fn add_surface<T>(backend: crate::compute::ComputeBackend, a: &crate::fields::surface_field::SurfaceField<T>, b: &crate::fields::surface_field::SurfaceField<T>) -> crate::fields::surface_field::SurfaceField<T>
where
    T: Copy + Send + Sync + Add<Output = T> { /* ... */ }
```

#### Function `sub_surface`

Difference of two surface fields — internal faces and every boundary patch.

Name, mesh, and boundary conditions come from `a`. **Units:** both operands
are the same physical quantity.

# Panics

Panics if the internal fields or any patch pair differ in length.

```rust
pub fn sub_surface<T>(backend: crate::compute::ComputeBackend, a: &crate::fields::surface_field::SurfaceField<T>, b: &crate::fields::surface_field::SurfaceField<T>) -> crate::fields::surface_field::SurfaceField<T>
where
    T: Copy + Send + Sync + Sub<Output = T> { /* ... */ }
```

#### Function `scale_surface`

Uniform scaling of a surface field, `c = a * s` — internal faces and every
boundary patch.

Name, mesh, and boundary conditions come from `a`. **Units:** `[a]*[s]`.

```rust
pub fn scale_surface<T>(backend: crate::compute::ComputeBackend, a: &crate::fields::surface_field::SurfaceField<T>, s: f64) -> crate::fields::surface_field::SurfaceField<T>
where
    T: Copy + Send + Sync + Mul<f64, Output = T> { /* ... */ }
```

#### Function `axpy_surface_assign`

In-place fused update on a surface field, `y += a * x` — the flux-field
counterpart of [`axpy_vol_assign`] (`phi += dt * ddt(phi)`).

Allocation-free, name-preserving. **Units:** `y` and `a*x` are the same
physical quantity.

# Panics

Panics if the internal fields or any patch pair differ in length.

```rust
pub fn axpy_surface_assign<T>(backend: crate::compute::ComputeBackend, y: &mut crate::fields::surface_field::SurfaceField<T>, a: f64, x: &crate::fields::surface_field::SurfaceField<T>)
where
    T: Copy + Send + Sync + Add<Output = T> + Mul<f64, Output = T> { /* ... */ }
```

#### Function `vol_integral`

Volume integral over the **internal** cells,
`integral(phi dV) = sum_i phi[i] * V[i]`.

`V[i]` is the cell volume in `m^3`, taken from the field's own mesh, so the
result carries **units `[phi] * m^3`** — for a density field in `kg/m^3` this
is the total mass in `kg`, which is the conservation check a compressible
solver runs each timestep.

Boundary patches are **not** included: they carry face values, not cell
values, and have no volume. An empty mesh integrates to `0.0`.

# Panics

Panics if the internal field length differs from `mesh.n_cells`.

# Determinism

Same guarantee as [`sum`]: reproducible run to run and independent of thread
count, not bit-identical to a serial left fold.

```rust
use std::sync::Arc;
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::fields::parallel;
use outram_foam_basic_lib::fields::vol_field::VolScalarField;
use outram_foam_basic_lib::mesh::fv_mesh::FvMesh;

// 10 cells over a 1 m length, 1 m^2 area => total volume 1 m^3.
let mesh = Arc::new(FvMesh::periodic_1d(10, 1.0, 1.0));
let rho = VolScalarField::uniform("rho", mesh, 1.2);   // [kg/m^3]
let mass = parallel::vol_integral(ComputeBackend::CpuMulti, &rho);
assert!((mass - 1.2).abs() < 1e-12);                   // 1.2 kg
```

```rust
pub fn vol_integral(backend: crate::compute::ComputeBackend, phi: &crate::fields::vol_field::VolScalarField) -> f64 { /* ... */ }
```

#### Function `vol_average`

Volume-weighted average over the internal cells,
`integral(phi dV) / integral(dV)`.

**Units:** the field's own units (the `m^3` cancels). Returns `0.0` when the
total volume is zero (an empty mesh) rather than `NaN`.

# Determinism

Same guarantee as [`sum`].

```rust
pub fn vol_average(backend: crate::compute::ComputeBackend, phi: &crate::fields::vol_field::VolScalarField) -> f64 { /* ... */ }
```

#### Function `vol_l2_norm`

L2 norm of a volume field's **internal** values — the residual measure a
solver's outer loop tests for convergence.

**Units:** the field's own units. Boundary patches are excluded.

# Determinism

Same guarantee as [`sum`].

```rust
pub fn vol_l2_norm(backend: crate::compute::ComputeBackend, phi: &crate::fields::vol_field::VolScalarField) -> f64 { /* ... */ }
```

#### Function `vol_min`

Smallest internal value of a volume field (`+inf` on an empty mesh).

**Units:** the field's own units. Bit-identical to a serial fold.

```rust
pub fn vol_min(backend: crate::compute::ComputeBackend, phi: &crate::fields::vol_field::VolScalarField) -> f64 { /* ... */ }
```

#### Function `vol_max`

Largest internal value of a volume field (`-inf` on an empty mesh).

**Units:** the field's own units. Bit-identical to a serial fold.

```rust
pub fn vol_max(backend: crate::compute::ComputeBackend, phi: &crate::fields::vol_field::VolScalarField) -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `REDUCTION_CHUNK`

Number of consecutive elements summed sequentially inside one chunk of a
parallel reduction.

The reductions cut the field into chunks of exactly this many elements, sum
each chunk in index order, then combine the partial sums in index order.
Fixing this at a compile-time constant — rather than deriving it from the
thread count — is what makes the parallel reduction **bit-reproducible and
thread-count independent** (see the module-level "Reduction determinism"
section).

Value: `4096` elements, i.e. 32 KiB of `f64`, which sits comfortably inside a
typical L1/L2 data cache so a chunk is summed without cache misses. The final
chunk is shorter when the length is not a multiple of 4096; that does not
affect reproducibility because the split is still a pure function of the
length.

```rust
pub const REDUCTION_CHUNK: usize = 4096;
```

#### Constant `FIELD_PARALLEL_CROSSOVER`

Element count at or above which [`ComputeBackend::CpuMulti`] actually spreads
the work across threads. Below it, every operation runs on the calling
thread.

# Why a crossover exists at all

Element-wise field arithmetic is memory-bandwidth bound and rayon's dispatch
costs on the order of microseconds. Below some size the dispatch dominates
and the parallel path is strictly *slower*.

# Measured basis (not a guess)

Measured 2026-08-12, `--release --features parallel`, `f64` fields, **4
logical cores** reported by [`std::thread::available_parallelism`]; operation
`c = a + b` (out-of-place add), best of 9 repeats after two warm-ups, by the
`#[ignore]`d `measure_crossover_add` test in this module, run in isolation
(`--test-threads=1`). Absolute wall-clock per call, from the least-contended
of nine sweeps:

| n | serial | CpuMulti (4 threads) | speedup |
|---|---|---|---|
| 1 024 | 0.43 us | 7.59 us | 0.06x |
| 4 096 | 1.89 us | 11.07 us | 0.17x |
| 16 384 | 5.99 us | 23.11 us | 0.26x |
| 65 536 | 56.27 us | 30.39 us | 1.85x |
| 131 072 | 117.77 us | 49.87 us | 2.36x |
| 262 144 | 239.65 us | 89.52 us | 2.68x |
| 1 048 576 | 2001.64 us | 437.34 us | 4.58x |

# The crossover is a band, not a point — read this before trusting it

The measurement machine is a shared virtualised sandbox. The **serial** column
is highly repeatable (spread under 5% across ten sweeps); the **parallel**
column is not, varying by up to 4x run to run because it competes for cores
with whatever else the host is doing. Counting how often the parallel path
won, over ten independent sweeps:

| n | sweeps won |
|---|---|
| 1 024 / 4 096 / 16 384 | 0 of 10 — never |
| 65 536 | 4 of 10 |
| 131 072 | 6 of 10 |
| 262 144 | 9 of 10 |
| 1 048 576 | 10 of 10 (1.48x-4.58x) |

So the honest statement is: **the crossover lies between 65 536 and 262 144
elements on this hardware, and the measurement is not precise enough to pin it
further.** This constant is set to `131_072`, the smallest size that won a
majority of sweeps. The asymmetry justifies erring low: in the sweeps where
131 072 lost, it lost by 3%-27% of ~120 us (tens of microseconds), whereas
setting the threshold at 262 144 would forfeit a measured 1.3x-2.4x on
mid-sized meshes.

**Re-measure on the target machine.** More cores or more memory bandwidth move
the crossover down; a phone moves it up. Run
`cargo test -p outram-foam-basic-lib --lib --release --features parallel --
--ignored --nocapture --test-threads=1 measure_crossover_add`.

# Relationship to [`crate::compute::CPU_MULTI_MIN_WORK_ITEMS`]

The workspace-level threshold is `4_096` work items and its own documentation
states it is **a placeholder awaiting measurement**, adding that "each kernel
that measures its own crossover should say so in its docs and may override
this". This constant is that override for the field-algebra kernels, and the
measurement says the placeholder is far too low for them: at `n = 4 096` the
parallel path measured **0.17x** — about six times *slower* than serial — and
it did not win a single one of nine sweeps at any size below 65 536. These
kernels are memory-bandwidth bound with only one or two flops per element, so
they need far more elements to amortise dispatch than a compute-dense kernel
would. The measurement here is offered as input to bead `op-yvj.4.7`.

```rust
pub const FIELD_PARALLEL_CROSSOVER: usize = 131_072;
```

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

#### Re-export `field_parallel_crossover`

```rust
pub use parallel::field_parallel_crossover;
```

#### Re-export `should_parallelise`

```rust
pub use parallel::should_parallelise;
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

## Module `naming`

Bounded names for operator-derived fields.

See [`derived_name`](crate::fv_operators::naming::derived_name) for why this exists; it is the shared guard against
the unbounded-`name` failure documented in this crate's `CLAUDE.md`.
Bounded names for operator-derived fields.

# What belongs here

The two helpers every `fvc::` operator uses to name its result, and nothing
else. They exist to make a specific memory bug **structurally impossible**
rather than merely avoided by convention.

# The bug this prevents

This crate's `CLAUDE.md` records that building a field's `name` from its
operands' names is dangerous: a solver that repeatedly reassigns a
persistent field from an expression containing that same field makes the
`name` `String` grow every timestep. In the original incident the growth was
*exponential* — doubling per step, reaching tens of gigabytes within about
25 steps and killing the `compressible_lid_cavity` test with a 24 GB
SIGTERM. The data was always correct; only the label ran away.

The arithmetic operators on [`crate::fields::VolField`] and
[`crate::fields::SurfaceField`] were fixed by keeping the left operand's
name, and that fix is still in place. But the `fvc::` operators legitimately
*do* derive compound names — `div(phi,rho)`, `grad(p)`, `interpolate(T)` —
because those names are genuinely useful diagnostics, and OpenFOAM names
them the same way.

That leaves a narrower hole, which is real rather than theoretical. Some
`fvc::` operators return the **same type** they consume:

- [`crate::fv_operators::fvc::div`] — `VolScalarField` in, `VolScalarField` out
- [`crate::fv_operators::fvc::div_vec`] — `VolVectorField` in, `VolVectorField` out

so `psi = fvc::div(&phi, &psi)` compiles. Each call would then nest the
name one level deeper: `rho`, `div(phi,rho)`, `div(phi,div(phi,rho))`, and
so on without bound. That is linear rather than exponential growth, so it
is slower to bite than the original bug — which is precisely what makes it
easy to miss.

# The rule

An operand that is *already* an operator-derived name is elided to `..`
instead of being nested. So the sequence above reaches a **fixed point**
after one application:

```text
rho  ->  div(phi,rho)  ->  div(phi,..)  ->  div(phi,..)  ->  ...
```

The first application keeps full diagnostic value, which is the one that
matters when reading a solver's field list. Every later application is
idempotent, so the name cannot grow no matter how many timesteps run.

# Units

None — these are diagnostic labels, not quantities.

```rust
pub mod naming { /* ... */ }
```

### Functions

#### Function `is_derived`

**Attributes:**

- `MustUse { reason: None }`

Whether `name` is already an operator-derived name rather than a plain
field label.

# How it decides

A derived name always contains `(`, because every operator in
[`crate::fv_operators::fvc`] wraps its operands in parentheses. A
user-declared field name (`rho`, `U`, `p`, `T`, `alpha.water`) does not.

# Arguments

- `name` — a field name. Dimensionless text.

# Example

```
use outram_foam_basic_lib::fv_operators::naming::is_derived;

assert!(!is_derived("rho"));
assert!(!is_derived("alpha.water"));
assert!(is_derived("div(phi,rho)"));
```

```rust
pub fn is_derived(name: &str) -> bool { /* ... */ }
```

#### Function `derived_name`

**Attributes:**

- `MustUse { reason: None }`

Name the result of a one-operand operator, without unbounded nesting.

# Arguments

- `op` — the operator's name, e.g. `"grad"`, `"interpolate"`, `"snGrad"`.
- `operand` — the input field's name.

# Returns

`op(operand)` when `operand` is a plain field name, or `op(..)` when it is
already derived. The length is therefore bounded by `op.len() + 4` in the
worst case, regardless of how many times the operator is applied.

# Example

```
use outram_foam_basic_lib::fv_operators::naming::derived_name;

assert_eq!(derived_name("grad", "p"), "grad(p)");

// Repeated application reaches a fixed point instead of growing.
let once = derived_name("grad", "p");
let twice = derived_name("grad", &once);
let thrice = derived_name("grad", &twice);
assert_eq!(twice, "grad(..)");
assert_eq!(thrice, "grad(..)");
```

```rust
pub fn derived_name(op: &str, operand: &str) -> String { /* ... */ }
```

#### Function `derived_name2`

**Attributes:**

- `MustUse { reason: None }`

Name the result of a two-operand operator, without unbounded nesting.

Each operand is elided independently, so `div(phi,rho)` keeps both useful
names while `div(phi, div(phi,rho))` collapses to `div(phi,..)`.

# Arguments

- `op` — the operator's name, e.g. `"div"`.
- `first`, `second` — the input fields' names.

# Returns

`op(first,second)` with either operand replaced by `..` if it is already
derived. This is the helper that closes the
[`crate::fv_operators::fvc::div`] self-feedback hole described in the
module docs.

# Example

```
use outram_foam_basic_lib::fv_operators::naming::derived_name2;

assert_eq!(derived_name2("div", "phi", "rho"), "div(phi,rho)");

// The self-feedback pattern `rho = div(phi, rho)` reaches a fixed point.
let step1 = derived_name2("div", "phi", "rho");
let step2 = derived_name2("div", "phi", &step1);
let step3 = derived_name2("div", "phi", &step2);
assert_eq!(step1, "div(phi,rho)");
assert_eq!(step2, "div(phi,..)");
assert_eq!(step3, "div(phi,..)");
```

```rust
pub fn derived_name2(op: &str, first: &str, second: &str) -> String { /* ... */ }
```

### Constants and Statics

#### Constant `ELIDED_OPERAND`

The marker substituted for an operand that is itself operator-derived.

Chosen as plain ASCII so field names stay safe to write into an OpenFOAM
case file and to compare in tests.

```rust
pub const ELIDED_OPERAND: &str = "..";
```

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

# Execution backend

Both solvers run on the hybrid [`ComputeBackend`](crate::compute::ComputeBackend), driving the kernels in
[`crate::ldu_matrix::parallel`]. Each has **one** implementation with the
backend as a parameter — [`bicgstab_prepared`](crate::krylov::bicgstab_prepared()) and [`gmres_prepared`](crate::krylov::gmres_prepared()), which
take a [`HybridLdu`](crate::ldu_matrix::parallel::HybridLdu) so the
cell-gather index is built once per mesh rather than once per solve — plus a
convenience adapter ([`bicgstab`](crate::krylov::bicgstab()), [`gmres`](crate::krylov::gmres())) for a caller holding a bare
[`LduMatrix`](crate::ldu_matrix::LduMatrix), which builds the index and runs on
[`ComputeBackend::Serial`](crate::compute::ComputeBackend::Serial). They are not a serial/parallel pair: they differ
in who owns the index, and the backend is a parameter of both.

Whole-solve wall clock on 4 logical cores, release, `--features parallel`,
512 000 cells, measured 2026-08-13 over five independent runs: **2.4-2.7x**
with Jacobi preconditioning, **~1.5x** with ILU(0) — the gap being ILU(0)'s
inherently sequential triangular solves, measured at 29-46% of the *serial*
solve, which caps it at 1.7-2.1x on 4 cores. Below roughly 13 000 cells the
parallel path **loses** (0.6-0.8x at 4 096 cells). Full tables, methodology
and limitations are on the benchmarks in `hybrid_tests` and summarised on
[`bicgstab_prepared`](crate::krylov::bicgstab_prepared()).

**Backend parity is bitwise**, not tolerance-based: `Serial` and `CpuMulti`
produce identical iterates, identical residual histories and identical
iteration counts at any thread count, because every kernel underneath carries
that guarantee individually.

What is *not* bitwise is the comparison against the solvers as they stood
**before** they took the hybrid path, because
[`crate::ldu_matrix::parallel::dot`] sums in blocks of 1 024 where
[`crate::krylov::vecops::dot`] sums flat, and floating-point addition is not
associative. Below 1 024 elements the two are identical; above it they differ
in the last bits, which in principle can move an iteration count by one when a
residual crosses the tolerance within those bits. Measured rather than assumed:
over a 216-solve sweep of 3 mesh sizes x 3 diagonal dominances x 3 right-hand
sides x 4 tolerances (1e-6 to 1e-12) x both solvers, the iteration count
changed **zero times**, for GMRES — the more exposed of the two — as well as
for BiCGStab. Converged residuals differ by up to 2.2e-4 *relative*, which at
the tightest tolerance swept is an absolute difference of ~1.9e-16 on a
residual of 8.6e-13. See `hybrid_tests` for the full grid and its limitations.

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

Iteration controls shared by [`bicgstab`](crate::krylov::bicgstab()) and [`gmres`](crate::krylov::gmres()).

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
  Apply the preconditioner serially: write `z = M^{-1} r`.

- ```rust
  pub fn apply_on(self: &Self, r: &[f64], z: &mut [f64], backend: ComputeBackend) { /* ... */ }
  ```
  Apply the preconditioner on the chosen [`ComputeBackend`]: write

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

#### Re-export `bicgstab_prepared`

```rust
pub use bicgstab::bicgstab_prepared;
```

#### Re-export `gmres`

```rust
pub use gmres::gmres;
```

#### Re-export `gmres_prepared`

```rust
pub use gmres::gmres_prepared;
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
- [`parallel`](crate::ldu_matrix::parallel) — the same sparse product and the
  per-iteration vector operations on the hybrid execution backend
  ([`HybridLdu`](crate::ldu_matrix::parallel::HybridLdu),
  [`LduTopology`](crate::ldu_matrix::parallel::LduTopology)): one entry point
  per operation taking a [`ComputeBackend`](crate::compute::ComputeBackend),
  serial or multi-CPU, bit-for-bit identical either way.
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
## Module `parallel`

The sparse LDU matrix-vector product and its companion vector operations, on
the hybrid execution backend — the hot path of every implicit finite-volume
solve.

A Krylov solver (conjugate gradient, BiCGStab, GMRES) spends most of its wall
clock inside a handful of operations. This module provides each of them as
**one** public entry point that takes a
[`ComputeBackend`](crate::compute::ComputeBackend) parameter, in the shape the
hybrid-backend epic mandates: dispatch, not a `foo_parallel()` sibling beside
`foo()`.

| Operation | Cost | Entry point |
|---|---|---|
| sparse product `y = A x` | `O(n_cells + n_faces)` | [`HybridLdu::spmv`] / [`HybridLdu::spmv_into`] |
| residual `r = b - A x` | `O(n_cells + n_faces)` | [`HybridLdu::residual`] / [`HybridLdu::residual_into`] |
| scaled residual norm | `O(n_cells + n_faces)` | [`HybridLdu::normalised_residual`] |
| diagonal reciprocal `1 / diag` | `O(n_cells)` | [`HybridLdu::diagonal_reciprocal`] |
| inner product `a . b` | `O(n)` | [`dot`] |
| `y := alpha x + y` | `O(n)` | [`axpy`] |
| `x := alpha x` | `O(n)` | [`scale`] |
| `sqrt(sum x_i^2)` / `sum abs(x_i)` | `O(n)` | [`norm_l2`] / [`norm_l1`] |

The solvers that consume them live in [`crate::krylov`] (BiCGStab, GMRES) and
[`crate::ldu_matrix::solvers`]; see [`crate::krylov::bicgstab_prepared`] for
the entry point that drives this module's kernels end to end.

# The correctness problem this module solves

[`LduMatrix`] stores its off-diagonal coefficients **per internal face**, so
the textbook product is a face-based *scatter*:

```text
for each face f:  y[owner[f]]     += upper[f] * x[neighbour[f]]
                  y[neighbour[f]] += lower[f] * x[owner[f]]
```

Two faces generally share a cell, so parallelising that face loop directly is
a **data race**: two threads read-modify-write the same `y[c]`. Where such a
thing can be written at all (atomics, unchecked split borrows) it produces
silently wrong, run-varying answers.

This module uses the **cell-gather reformulation**. A one-off index build
([`LduTopology`]) inverts the face addressing into a per-cell list of incident
faces, turning the product into

```text
for each cell c:  y[c] = diag[c] * x[c]
                       + sum over faces f incident on c of
                             (c is owner ? upper[f] * x[neighbour[f]]
                                         : lower[f] * x[owner[f]])
```

The loop is now over **cells**, and every output element `y[c]` is written by
exactly one thread. There is no race, no atomic, and no per-thread scratch
buffer.

The two alternatives were considered and rejected. *Per-thread partial
accumulation* costs `threads * n_cells` extra memory and needs a reduction
pass whose association order varies with the schedule, so it is not
reproducible. *Face colouring* needs a graph colouring at build time, still
writes each cell once per colour, and its result depends on the colouring
produced. The cell-gather index costs one `O(n_cells + n_faces)` build that is
amortised over the thousands of products a solve performs, and it buys exact
reproducibility (below).

# Determinism

**Every kernel in this module returns bit-for-bit identical output on
[`ComputeBackend::Serial`](crate::compute::ComputeBackend::Serial) and
[`ComputeBackend::CpuMulti`](crate::compute::ComputeBackend::CpuMulti), at any
thread count, on every run.** That is stronger than the usual
parallel-reduction contract and it is deliberate, because
`ComputeBackend::Serial` is this workspace's documented deterministic oracle.

Two separate mechanisms deliver it:

- **Products and element-wise kernels** ([`HybridLdu::spmv`],
  [`HybridLdu::residual`], [`HybridLdu::diagonal_reciprocal`], [`axpy`],
  [`scale`]) are
  bitwise identical *also to the pre-existing serial reference*
  [`LduMatrix::multiply`] / [`LduMatrix::residual`]. [`LduTopology`] lists
  each cell's incident faces in **ascending face index**, which is exactly the
  order in which the serial scatter reaches that cell, so each `y[c]`
  accumulates the same additions in the same sequence.
- **Reductions** ([`dot`], [`norm_l1`], [`norm_l2`],
  [`HybridLdu::normalised_residual`]) sum in fixed-size blocks of
  [`REDUCTION_BLOCK`] elements and then combine the block partials in
  ascending block order. The association is a function of the array length and
  [`REDUCTION_BLOCK`] alone — never of the thread count or the work-stealing
  schedule — so a 1-thread run agrees bit for bit with a 64-thread run.

The one thing a reduction here is **not** is bitwise equal to a flat
left-to-right sum such as [`crate::krylov::vecops::dot`] or
[`LduMatrix::normalised_residual`]. Blocked summation reassociates, and
floating-point addition is not associative. That difference is small,
bounded, and *measured* rather than asserted — see the "Measured deviation"
sections on [`dot`] and [`HybridLdu::normalised_residual`]. Blocked summation
is in fact the more accurate of the two (it is a two-level pairwise-style
sum), so this is not a loss of accuracy relative to the flat reference.

# When multi-CPU is actually faster

Threading is not free. Below [`SPMV_MIN_CELLS`] cells (products) or
[`VECOP_MIN_ELEMENTS`] elements (vector operations), a `CpuMulti` request runs
the serial kernel on the calling thread instead. Because the two paths are
bitwise identical, that size dispatch changes **no** number a caller can
observe — only the wall clock. Both constants were measured on this
workspace's development machine; the tables are on the constants themselves.

For scale, the sparse product on a 262 144-cell / 774 144-face matrix,
measured 2026-08-12 on 4 logical cores (`spmv_thread_scaling_benchmark`,
best of 7 samples, time per call):

| Worker threads | Time per product | Speed-up |
|---|---|---|
| 1 | 6487.26 us | 1.00x |
| 2 | 3481.42 us | 1.86x |
| 4 | 2322.10 us | 2.79x |
| 8 | 1830.46 us | 3.54x |

The output was asserted bitwise identical at every one of those thread
counts, which is the determinism claim above measured rather than argued.
Eight workers on four cores still help, which is the signature of a
memory-latency-bound gather: extra threads buy more outstanding loads rather
than more arithmetic. **This is one machine and one mesh; it is not a scaling
study**, and nothing here has been measured on Android hardware or on a
many-core server.

# Cargo features

The `rayon` code paths sit behind the crate's `parallel` feature, which is
**off by default**. With the feature off this module still compiles and every
entry point still works: `ComputeBackend::CpuMulti` resolves down to
`ComputeBackend::Serial` via
[`ComputeBackend::resolve`](crate::compute::ComputeBackend::resolve) and the
answer is unchanged. There is no `Gpu` kernel here yet, so a `Gpu` request
also degrades to the best available CPU path.

# Portability

`rayon` is pure Rust with no system component, so everything here compiles and
runs on `aarch64-linux-android` / Termux exactly as on desktop. Nothing in
this module is target-gated.

# Units

All slices are dimensionless `f64` in cell order: element `c` is the value at
cell `c`, in whatever units the assembled equation carries. No `uom` typing is
applied at this layer, for the same reason [`crate::krylov::vecops`] applies
none: a Krylov subspace mixes residuals, search directions and solution
increments that share no single physical dimension. Units belong on the field
and equation layer that assembles the matrix, and are not stripped there.

# Example

```rust
use std::sync::Arc;
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::ldu_matrix::LduMatrix;
use outram_foam_basic_lib::ldu_matrix::parallel::HybridLdu;

// 3-cell symmetric tridiagonal  [[2,-1,0],[-1,2,-1],[0,-1,2]]
let mut m = LduMatrix::new(3, vec![0, 1], vec![1, 2]);
m.diag = vec![2.0, 2.0, 2.0];
m.upper = vec![-1.0, -1.0];
m.lower = vec![-1.0, -1.0];
let m = Arc::new(m);

let ldu = HybridLdu::new(Arc::clone(&m));
let x = vec![1.0, 1.0, 1.0];

assert_eq!(ldu.spmv(&x, ComputeBackend::Serial), vec![1.0, 0.0, 1.0]);

// Asking for multi-CPU gives a bit-for-bit identical answer, whether or not
// the `parallel` feature is compiled in.
assert_eq!(
    ldu.spmv(&x, ComputeBackend::CpuMulti),
    ldu.spmv(&x, ComputeBackend::Serial),
);
```

```rust
pub mod parallel { /* ... */ }
```

### Types

#### Struct `LduTopology`

The face addressing of an [`LduMatrix`], inverted into per-cell incident-face
lists so the matrix-vector product can be parallelised over cells.

This is a pure topology object: it depends only on `n_cells`, `owner` and
`neighbour`, **not** on any coefficient value. A finite-volume solver
reassembles coefficients every outer iteration while the mesh addressing stays
fixed, so build this once and reuse it — see [`HybridLdu::with_matrix`].

# Layout

Compressed-row: `row_start[c] .. row_start[c + 1]` selects cell `c`'s entries
out of the flat `entry_*` arrays. There are exactly `2 * n_internal_faces`
entries, because every internal face is incident on exactly two cells.

Each cell's entries are stored in **ascending internal-face index**. That is
not cosmetic: it is the property that makes the cell-gather product bitwise
reproduce the serial face-scatter of [`LduMatrix::multiply`], because it is
the order in which the scatter reaches that cell.

# Units

Pure indices and counts; dimensionless.

# Example

```rust
use outram_foam_basic_lib::ldu_matrix::LduMatrix;
use outram_foam_basic_lib::ldu_matrix::parallel::LduTopology;

// 3 cells, 2 internal faces: (0,1) and (1,2).
let m = LduMatrix::new(3, vec![0, 1], vec![1, 2]);
let topo = LduTopology::from_matrix(&m);

assert_eq!(topo.n_cells(), 3);
assert_eq!(topo.n_internal_faces(), 2);
// The middle cell touches both faces; the end cells touch one each.
assert_eq!(topo.incident_face_count(0), 1);
assert_eq!(topo.incident_face_count(1), 2);
assert_eq!(topo.incident_face_count(2), 1);
```

```rust
pub struct LduTopology {
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
  pub fn from_matrix(matrix: &LduMatrix) -> Self { /* ... */ }
  ```
  Build the cell-gather index from a matrix's face addressing.

- ```rust
  pub fn n_cells(self: &Self) -> usize { /* ... */ }
  ```
  Number of cells this index was built for.

- ```rust
  pub fn n_internal_faces(self: &Self) -> usize { /* ... */ }
  ```
  Number of internal faces this index was built for.

- ```rust
  pub fn incident_face_count(self: &Self, c: usize) -> usize { /* ... */ }
  ```
  How many internal faces are incident on cell `c` — its off-diagonal count.

- ```rust
  pub fn index_bytes(self: &Self) -> usize { /* ... */ }
  ```
  Approximate heap footprint of the index, in bytes.

- ```rust
  pub fn matches(self: &Self, matrix: &LduMatrix) -> bool { /* ... */ }
  ```
  Whether this index describes `matrix`'s addressing exactly.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> LduTopology { /* ... */ }
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
    fn eq(self: &Self, other: &LduTopology) -> bool { /* ... */ }
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
#### Struct `HybridLdu`

An [`LduMatrix`] bundled with its cell-gather index, exposing the
per-iteration kernels a Krylov solver needs on any [`ComputeBackend`].

Construct once per assembled matrix and call the kernels many times: the
`O(n_cells + n_faces)` index build happens in [`Self::new`], and every kernel
call after that is index-free. Both the matrix and the index are held behind
[`Arc`], so cloning is cheap and the value can be shared across threads.

# Which backend runs

Every kernel takes the backend as a parameter — there is no `_parallel`
sibling API. What actually runs is [`spmv_backend_for`] /
[`vecop_backend_for`] applied to the request: an unavailable backend degrades,
`Gpu` degrades (no GPU kernel here yet), and a problem below the measured
size floor runs serially. None of those degradations changes the answer.
[`Self::auto_backend`] applies the crate-wide policy
[`crate::compute::select_backend`] if you would rather not choose.

# Thread pool

The `parallel` kernels use `rayon`'s ambient pool — the global one by default.
No pool is built here, and none is built per call. A caller that wants a
specific worker count builds its own `rayon::ThreadPool` and calls these
kernels inside its `install(...)` scope; the parallel iterators then run on
that pool. Because every kernel is bitwise deterministic at any thread count,
that choice affects wall time only.

# Units

All vectors are dimensionless `f64` in cell order; see the module
documentation.

# Example

```rust
use std::sync::Arc;
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::ldu_matrix::LduMatrix;
use outram_foam_basic_lib::ldu_matrix::parallel::HybridLdu;

let mut m = LduMatrix::new(4, vec![0, 1, 2], vec![1, 2, 3]);
m.diag = vec![4.0, 4.0, 4.0, 4.0];
m.upper = vec![-1.0, -1.0, -1.0];
m.lower = vec![-1.0, -1.0, -1.0];
let m = Arc::new(m);

let ldu = HybridLdu::new(Arc::clone(&m));
let x = vec![1.0, 2.0, 3.0, 4.0];

// Bit-for-bit agreement with the pre-existing serial reference kernel.
assert_eq!(ldu.spmv(&x, ComputeBackend::CpuMulti), m.multiply(&x));

// Reassembling the same mesh reuses the index instead of rebuilding it.
let mut m2 = (*m).clone();
m2.diag = vec![8.0, 8.0, 8.0, 8.0];
let ldu2 = ldu.with_matrix(Arc::new(m2)).expect("same addressing");
assert_eq!(ldu2.matrix().diag[0], 8.0);
```

```rust
pub struct HybridLdu {
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
  pub fn new(matrix: Arc<LduMatrix>) -> Self { /* ... */ }
  ```
  Build the cell-gather index for `matrix` and wrap it for hybrid execution.

- ```rust
  pub fn with_matrix(self: &Self, matrix: Arc<LduMatrix>) -> Option<Self> { /* ... */ }
  ```
  Reuse this index with a **reassembled** matrix over the same mesh.

- ```rust
  pub fn matrix(self: &Self) -> &Arc<LduMatrix> { /* ... */ }
  ```
  The matrix these kernels operate on.

- ```rust
  pub fn topology(self: &Self) -> &Arc<LduTopology> { /* ... */ }
  ```
  The cell-gather index, shared behind an [`Arc`].

- ```rust
  pub fn auto_backend(self: &Self) -> ComputeBackend { /* ... */ }
  ```
  The backend the crate-wide policy [`crate::compute::select_backend`] picks

- ```rust
  pub fn spmv_into(self: &Self, x: &[f64], y: &mut [f64], backend: ComputeBackend) { /* ... */ }
  ```
  Sparse matrix-vector product `y = A x`, writing into a caller-owned buffer.

- ```rust
  pub fn spmv(self: &Self, x: &[f64], backend: ComputeBackend) -> Vec<f64> { /* ... */ }
  ```
  Sparse matrix-vector product `y = A x`, allocating the result.

- ```rust
  pub fn residual_into(self: &Self, x: &[f64], b: &[f64], r: &mut [f64], backend: ComputeBackend) { /* ... */ }
  ```
  Residual `r = b - A x`, writing into a caller-owned buffer.

- ```rust
  pub fn residual(self: &Self, x: &[f64], b: &[f64], backend: ComputeBackend) -> Vec<f64> { /* ... */ }
  ```
  Residual `r = b - A x`, allocating the result.

- ```rust
  pub fn normalised_residual(self: &Self, x: &[f64], b: &[f64], backend: ComputeBackend) -> f64 { /* ... */ }
  ```
  OpenFOAM-style scaled residual norm,

- ```rust
  pub fn diagonal_reciprocal(self: &Self, backend: ComputeBackend) -> Vec<f64> { /* ... */ }
  ```
  Element-wise reciprocal of the diagonal, `1 / diag[c]` for every cell.

- ```rust
  pub fn diagonal_reciprocal_into(self: &Self, out: &mut [f64], backend: ComputeBackend) { /* ... */ }
  ```
  Element-wise reciprocal of the diagonal, into a caller-owned buffer.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> HybridLdu { /* ... */ }
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
### Functions

#### Function `spmv_backend_for`

**Attributes:**

- `MustUse { reason: None }`

The [`ComputeBackend`] this module would actually use for a sparse product
over `n_cells` cells if asked for `requested` — without running anything.

Useful for logging and for benchmark harnesses that need to report which path
a call took. It applies exactly the same three-step reduction the kernels do
(feature availability, no-GPU-kernel-here, and the [`SPMV_MIN_CELLS`] size
floor), so what it reports is what would run.

# Arguments

- `requested` — the backend a caller would pass to [`HybridLdu::spmv`].
- `n_cells` — the matrix size, dimensionless.

# Returns

Either [`ComputeBackend::Serial`] or [`ComputeBackend::CpuMulti`]; never
[`ComputeBackend::Gpu`], because no GPU kernel exists here yet.

# Example

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::ldu_matrix::parallel::{spmv_backend_for, SPMV_MIN_CELLS};

// Too small to thread, whatever was asked for.
assert_eq!(spmv_backend_for(ComputeBackend::CpuMulti, 100), ComputeBackend::Serial);

// Big enough; the answer now depends only on whether `parallel` is compiled in.
let picked = spmv_backend_for(ComputeBackend::CpuMulti, SPMV_MIN_CELLS);
assert!(picked.is_available());
```

```rust
pub fn spmv_backend_for(requested: crate::compute::ComputeBackend, n_cells: usize) -> crate::compute::ComputeBackend { /* ... */ }
```

#### Function `vecop_backend_for`

**Attributes:**

- `MustUse { reason: None }`

The [`ComputeBackend`] this module would actually use for a vector operation
over `n` elements if asked for `requested` — without running anything.

The vector-operation counterpart of [`spmv_backend_for`], differing only in
using the [`VECOP_MIN_ELEMENTS`] size floor.

# Arguments

- `requested` — the backend a caller would pass to [`dot`], [`axpy`],
  [`norm_l1`] or [`norm_l2`].
- `n` — the vector length, dimensionless.

# Returns

Either [`ComputeBackend::Serial`] or [`ComputeBackend::CpuMulti`].

# Example

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::ldu_matrix::parallel::vecop_backend_for;

assert_eq!(vecop_backend_for(ComputeBackend::CpuMulti, 64), ComputeBackend::Serial);
assert_eq!(vecop_backend_for(ComputeBackend::Serial, 1 << 24), ComputeBackend::Serial);
```

```rust
pub fn vecop_backend_for(requested: crate::compute::ComputeBackend, n: usize) -> crate::compute::ComputeBackend { /* ... */ }
```

#### Function `dot`

**Attributes:**

- `MustUse { reason: None }`

Inner product `sum_i a_i * b_i`, on the chosen backend.

Krylov methods evaluate two or three of these per iteration (conjugate
gradient: `r.r` and `p.Ap`), so on a large mesh they are worth threading even
though the operation is memory-bandwidth bound rather than arithmetic bound.

# Arguments

- `a`, `b` — equal-length dimensionless vectors in cell order.
- `backend` — requested execution backend; see [`vecop_backend_for`] for what
  will actually run.

# Returns

The dimensionless scalar product. Returns `0.0` for empty inputs.

# Determinism, and how this differs from [`crate::krylov::vecops::dot`]

Sums in fixed blocks of [`REDUCTION_BLOCK`] elements, combining block partials
in ascending block order. The result is therefore **bitwise identical between
backends and at any thread count**, and reproducible run to run. It differs
from the flat left-to-right sum in [`crate::krylov::vecops::dot`] only by
floating-point non-associativity.

**Measured deviation.** *Methodology:* two fixed-seed xorshift64\*
pseudorandom vectors with elements uniform on `[-1, 1)`, at lengths 1 024
through 4 194 304 in powers of four; compare against
[`crate::krylov::vecops::dot`] on the same inputs. Two measures are reported,
because the raw relative difference is taken against a heavily cancelled sum
(terms of order 1, total of order `sqrt(n)`) and so overstates the error:
the raw relative difference `|blocked - flat| / max(|blocked|, |flat|)`, and
the conditioning-aware `|blocked - flat| / sum |a_i b_i|`. *Pass criteria:*
worst raw `<= 1e-12` **and** worst conditioned `<= 1e-15`.

*Results, measured 2026-08-12 by the test `dot_matches_flat_reference` in
`parallel/tests.rs`:*

| n | blocked | flat | raw rel diff | vs sum abs terms |
|---|---|---|---|---|
| 1 024 | -1.07439412213983019e1 | -1.07439412213983019e1 | 0 | 0 |
| 4 096 | 2.27305615265860794e0 | 2.27305615265858707e0 | 9.1824e-15 | 2.0525e-17 |
| 16 384 | -4.10927502452607953e1 | -4.10927502452608877e1 | 2.2479e-15 | 2.2574e-17 |
| 65 536 | -1.48877508980721871e2 | -1.48877508980720563e2 | 8.7817e-15 | 7.9476e-17 |
| 262 144 | -2.57469059787491972e2 | -2.57469059787487652e2 | 1.6779e-14 | 6.6111e-17 |
| 1 048 576 | 2.06465247477648091e2 | 2.06465247477646983e2 | 5.3687e-15 | 4.2292e-18 |
| 4 194 304 | -1.50102185250877881e2 | -1.50102185250842240e2 | 2.3744e-13 | 3.3997e-17 |

Worst raw relative difference **2.3744e-13** at n = 4 194 304; worst
conditioned difference **7.9476e-17** at n = 65 536.

*Interpretation:* the difference is summation reassociation at rounding
level, not a defect — relative to the arithmetic actually performed it never
exceeds about a third of one `f64` epsilon. The raw figure grows with `n`
because the exact sum of `n` random signed terms grows only like `sqrt(n)`
while the rounding error grows faster, so the quotient inflates; that is a
property of the test vectors, not of either kernel. Blocked summation is the
two-level, more accurate form — a flat sum of `n` terms has error growing
like `n * eps`, a blocked sum like `(block + n / block) * eps` — so the
blocked result is if anything closer to the exact value.

# Panics

Panics if `a` and `b` have different lengths.

# Example

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::ldu_matrix::parallel::dot;

let a = [1.0, 2.0, 3.0];
let b = [4.0, -5.0, 6.0];
// 1*4 + 2*(-5) + 3*6 = 12
assert_eq!(dot(&a, &b, ComputeBackend::Serial), 12.0);
// Bitwise identical, not merely close.
assert_eq!(
    dot(&a, &b, ComputeBackend::CpuMulti),
    dot(&a, &b, ComputeBackend::Serial),
);
```

```rust
pub fn dot(a: &[f64], b: &[f64], backend: crate::compute::ComputeBackend) -> f64 { /* ... */ }
```

#### Function `norm_l2`

**Attributes:**

- `MustUse { reason: None }`

L2 (Euclidean) norm `sqrt(sum_i x_i^2)`, on the chosen backend.

Computed as `dot(x, x, backend).sqrt()`, so it inherits that function's
determinism guarantee exactly: bitwise identical between backends and at any
thread count, and differing from [`crate::krylov::vecops::nrm2`] only by
summation reassociation.

# Arguments

- `x` — a dimensionless vector in cell order.
- `backend` — requested execution backend.

# Returns

A non-negative dimensionless scalar; `0.0` for an empty input. For very large
element magnitudes the intermediate sum of squares can overflow to infinity —
there is no scaling guard, matching [`crate::krylov::vecops::nrm2`], because
well-scaled linear systems stay far inside normal `f64` range.

# Example

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::ldu_matrix::parallel::norm_l2;

assert_eq!(norm_l2(&[3.0, 4.0], ComputeBackend::CpuMulti), 5.0);
assert_eq!(norm_l2(&[], ComputeBackend::Serial), 0.0);
```

```rust
pub fn norm_l2(x: &[f64], backend: crate::compute::ComputeBackend) -> f64 { /* ... */ }
```

#### Function `norm_l1`

**Attributes:**

- `MustUse { reason: None }`

L1 norm `sum_i abs(x_i)`, on the chosen backend.

This is the norm OpenFOAM's solver convergence test uses, and the one behind
[`HybridLdu::normalised_residual`].

# Arguments

- `x` — a dimensionless vector in cell order.
- `backend` — requested execution backend.

# Returns

A non-negative dimensionless scalar; `0.0` for an empty input.

# Determinism

Blocked summation exactly as in [`dot`]: bitwise identical between backends
and at any thread count, differing from a flat sum only by reassociation.

# Example

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::ldu_matrix::parallel::norm_l1;

assert_eq!(norm_l1(&[1.0, -2.0, 3.0], ComputeBackend::CpuMulti), 6.0);
```

```rust
pub fn norm_l1(x: &[f64], backend: crate::compute::ComputeBackend) -> f64 { /* ... */ }
```

#### Function `axpy`

AXPY update `y := alpha * x + y`, in place, on the chosen backend.

The vector update every Krylov iteration performs several times (advancing the
solution, the residual and the search direction).

# Arguments

- `alpha` — dimensionless scalar multiplier.
- `x` — dimensionless input vector in cell order.
- `y` — dimensionless accumulator, same length as `x`, updated in place.
- `backend` — requested execution backend.

# Determinism

Bitwise identical between backends and at any thread count: each element is an
independent fused expression, so there is no reduction to reassociate. Unlike
the reductions, this also matches [`crate::krylov::vecops::axpy`] exactly.

# Panics

Panics if `x` and `y` have different lengths.

# Example

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::ldu_matrix::parallel::axpy;

let x = [1.0, 1.0, 1.0];
let mut y = [10.0, 20.0, 30.0];
axpy(2.0, &x, &mut y, ComputeBackend::CpuMulti);
assert_eq!(y, [12.0, 22.0, 32.0]);
```

```rust
pub fn axpy(alpha: f64, x: &[f64], y: &mut [f64], backend: crate::compute::ComputeBackend) { /* ... */ }
```

#### Function `scale`

Scale `x := alpha * x`, in place, on the chosen backend.

The third element-wise vector update a Krylov solver needs, alongside [`axpy`]
and the products. GMRES normalises every Arnoldi basis vector with it
(`v := v / ||v||`), twice per inner iteration, so on a large mesh it is on the
hot path; BiCGStab uses it to rescale search directions.

# Arguments

- `alpha` — dimensionless scalar multiplier.
- `x` — dimensionless vector in cell order, updated in place.
- `backend` — requested execution backend; see [`vecop_backend_for`] for what
  will actually run.

# Determinism

Bitwise identical between backends and at any thread count, and bitwise
identical to [`crate::krylov::vecops::scal`]: every element is an independent
single multiplication, so — exactly as for [`axpy`] — there is no reduction to
reassociate and no summation order to depend on. This is the strongest of the
three determinism grades in this module and it holds unconditionally, for any
finite or non-finite `alpha` and any input.

# Example

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::ldu_matrix::parallel::scale;

let mut x = [1.0, -2.0, 3.0];
scale(-2.0, &mut x, ComputeBackend::CpuMulti);
assert_eq!(x, [-2.0, 4.0, -6.0]);
```

```rust
pub fn scale(alpha: f64, x: &mut [f64], backend: crate::compute::ComputeBackend) { /* ... */ }
```

### Constants and Statics

#### Constant `CELL_BLOCK`

Number of cells in one block of the cell-parallel kernels.

`rayon` splits a chunked parallel iterator adaptively, so this is a *lower*
bound on task granularity rather than a fixed task size: it stops the
scheduler subdividing below a block that is too small to pay for itself. 1024
cells is about 8 KiB of output per block, comfortably inside an L1 data cache,
while still leaving hundreds of blocks on any mesh worth threading.

This constant affects wall time only. It cannot affect the value of any
kernel that uses it, because those kernels compute each cell independently.

# Units

A count of cells, dimensionless.

```rust
pub const CELL_BLOCK: usize = 1024;
```

#### Constant `REDUCTION_BLOCK`

Number of elements summed serially inside one block of a reduction, before
block partials are combined.

This constant **does** affect the last bits of every reduction in this module
([`dot`], [`norm_l1`], [`norm_l2`], [`HybridLdu::normalised_residual`]),
because floating-point addition is not associative — but it does so
*reproducibly*. The summation tree is a function of the array length and this
constant alone, never of the thread count or the scheduler, which is exactly
what makes those kernels bitwise reproducible across backends and runs.

Treat it as part of the numerical contract rather than a free tuning knob:
changing it would perturb converged residual histories in the last few digits.

# Units

A count of vector elements, dimensionless.

```rust
pub const REDUCTION_BLOCK: usize = 1024;
```

#### Constant `SPMV_MIN_CELLS`

Cell count below which a [`ComputeBackend::CpuMulti`] request runs the serial
product kernel instead.

# Why a threshold exists

Dispatching a `rayon` parallel iterator costs on the order of a microsecond of
scheduling and synchronisation. A sparse product on a small mesh finishes in
less than that, so threading it is a straight loss. Because the serial and
parallel kernels are **bitwise identical** (see the module documentation,
"Determinism"), dispatching on size changes no number a caller can observe.

# Measured crossover

Measured 2026-08-12 on this workspace's development machine
(`std::thread::available_parallelism()` = **4**, release build, `--features
parallel`, rayon's global pool), on a structured 7-point-stencil LDU matrix
with pseudorandom coefficients. Each figure is the best of 7 samples, each
sample timing enough back-to-back calls to total about 8 million cell
updates, reported as time per [`HybridLdu::spmv_into`] call. Produced by the
`#[ignore]`d `spmv_crossover_benchmark` test in `parallel/tests.rs` and
transcribed from its printed output.

| Cells | Faces | Serial | CpuMulti | Speed-up | Speed-up, repeat run |
|---|---|---|---|---|---|
| 512 | 1 344 | 9.76 us | 9.78 us | 1.00x | 1.00x |
| 1 000 | 2 700 | 20.24 us | 20.08 us | 1.01x | 1.00x |
| 1 728 | 4 752 | 34.55 us | 63.28 us | 0.55x | 0.65x |
| 2 744 | 7 644 | 55.67 us | 69.32 us | 0.80x | 1.21x |
| 4 096 | 11 520 | 83.67 us | 80.61 us | 1.04x | 1.02x |
| 5 832 | 16 524 | 125.50 us | 122.35 us | 1.03x | 1.52x |
| 8 000 | 22 800 | 165.60 us | 114.06 us | 1.45x | 2.02x |
| 15 625 | 45 000 | 330.59 us | 175.99 us | 1.88x | 2.03x |
| 32 768 | 95 232 | 690.40 us | 277.86 us | 2.48x | 2.03x |
| 64 000 | 187 200 | 1448.41 us | 481.12 us | 3.01x | 3.11x |
| 132 651 | 390 150 | 3279.72 us | 1139.65 us | 2.88x | 3.48x |
| 262 144 | 774 144 | 6640.67 us | 2213.45 us | 3.00x | 3.00x |
| 512 000 | 1 516 800 | 13338.47 us | 4822.19 us | 2.77x | 3.48x |

The serial column is highly reproducible between the two runs (within 4%);
the multi-CPU column is not, which is why the second run's speed-ups are
carried alongside rather than averaged away.

Reading the table: at and below 1 000 cells the two paths are *identical*,
because [`CELL_BLOCK`] is 1 024 so there is only one block and rayon never
splits. Between roughly 1 700 and 2 700 cells the parallel path genuinely
loses (0.55x — 1.21x). At 4 096 it breaks even in both runs (1.04x, 1.02x)
and from about 5 800 cells upward it wins in both. This constant is therefore
set to **4 096**, the smallest measured size at which the parallel path did
not lose in either run.

Speed-ups above 3x on 4 cores are superlinear and were reproducible. The
likely cause is cache: the gather reads `x` at scattered indices across the
whole vector, and splitting the cells narrows each worker's working set, so
four threads share more than four times the effective cache. This has not
been confirmed with hardware counters and is stated as a hypothesis.

# Relationship to [`crate::compute::CPU_MULTI_MIN_WORK_ITEMS`]

That crate-wide constant is documented as a placeholder awaiting measurement
and is currently 4 096. **For this kernel the measurement supports it**: 4 096
cells is exactly where the parallel path stops losing. No change to the
crate-wide constant is proposed on the strength of the sparse product alone —
but note that the vector operations in this same module cross over two orders
of magnitude later (see [`VECOP_MIN_ELEMENTS`]), so 4 096 is *not* a good
single number for every kernel, and a crate-wide revision belongs with bead
`op-yvj.4.7` rather than here.

# Limitations

One threshold cannot be right for every machine: a 64-core server pays more
dispatch cost and a 2-core phone less. This value was measured on exactly one
machine, with 4 logical cores, and has **not** been checked on any other — in
particular not on Android/Termux hardware and not on a many-core server. It
was also measured on an otherwise idle machine; on a loaded machine an
earlier, contended run of the same benchmark put 4 096 cells at 0.72x, so the
break-even point moves right under contention.

# Units

A count of cells, dimensionless.

```rust
pub const SPMV_MIN_CELLS: usize = 4_096;
```

#### Constant `VECOP_MIN_ELEMENTS`

Element count below which a [`ComputeBackend::CpuMulti`] request runs the
serial kernel instead, for the vector operations [`dot`], [`axpy`],
[`norm_l1`] and [`norm_l2`].

# Why this is larger than [`SPMV_MIN_CELLS`]

A vector operation does one or two floating-point operations per element
loaded, so it is limited by memory bandwidth rather than by arithmetic. Extra
cores do not add bandwidth, so there is much less to win and the fixed
dispatch cost is amortised much more slowly than it is for the sparse product,
which does roughly seven operations per cell.

# Measured crossover

Measured 2026-08-12 on the same machine and under the same conditions as
[`SPMV_MIN_CELLS`] (4 logical cores, release, `--features parallel`, idle
machine), best of 7 samples, per call. Produced by the `#[ignore]`d
`vecop_crossover_benchmark` test in `parallel/tests.rs` and transcribed from
its printed output. The last column of each pair is the speed-up from a
second, independent run of the same benchmark.

| Elements | `dot` serial | `dot` multi | Speed-up | (repeat) | `axpy` serial | `axpy` multi | Speed-up | (repeat) |
|---|---|---|---|---|---|---|---|---|
| 1 024 | 1.27 us | 1.30 us | 0.98x | 0.97x | 0.34 us | 0.34 us | 0.98x | 0.98x |
| 2 048 | 2.51 us | 14.88 us | 0.17x | 0.11x | 0.63 us | 15.32 us | 0.04x | 0.02x |
| 4 096 | 4.97 us | 12.48 us | 0.40x | 0.21x | 1.59 us | 23.71 us | 0.07x | 0.05x |
| 8 192 | 10.02 us | 31.58 us | 0.32x | 0.24x | 3.14 us | 37.23 us | 0.08x | 0.09x |
| 16 384 | 19.87 us | 22.05 us | 0.90x | 0.69x | 5.55 us | 16.33 us | 0.34x | 0.09x |
| 32 768 | 40.42 us | 26.88 us | 1.50x | 0.96x | 11.69 us | 26.78 us | 0.44x | 0.74x |
| 65 536 | 80.10 us | 53.41 us | 1.50x | 1.85x | 26.13 us | 26.73 us | 0.98x | 0.60x |
| 131 072 | 160.35 us | 84.69 us | 1.89x | 2.23x | 77.26 us | 45.57 us | 1.70x | 0.74x |
| 262 144 | 322.69 us | 106.86 us | 3.02x | 3.06x | 132.15 us | 70.70 us | 1.87x | 1.00x |
| 524 288 | 662.60 us | 187.03 us | 3.54x | 2.13x | 314.93 us | 129.20 us | 2.44x | 1.28x |
| 1 048 576 | 1712.59 us | 547.58 us | 3.13x | 3.35x | 916.51 us | 306.37 us | 2.99x | 2.98x |
| 2 097 152 | 3883.47 us | 1209.26 us | 3.21x | 3.69x | 3248.92 us | 1000.85 us | 3.25x | 3.70x |
| 4 194 304 | 8115.34 us | 2306.65 us | 3.52x | 3.47x | 7447.22 us | 2235.37 us | 3.33x | 3.78x |

The two operations cross over at very different sizes. `dot` is reliably
ahead from 65 536 elements; `axpy` is still break-even at 262 144 (1.87x in
one run, 1.00x in the other) and only reliably ahead from 1 048 576. The
constant is set to **262 144** — the smallest size at which *neither*
operation lost in either run — so the single floor is safe for all four
vector operations. The cost of one shared floor is that `dot` forgoes a
1.5x-2.2x win between 65 536 and 262 144 elements; a per-operation floor is
left as a follow-up rather than shipped on two runs of evidence.

Note the 1 024-element row: the two paths are identical there because
[`REDUCTION_BLOCK`] and [`CELL_BLOCK`] are both 1 024, so there is a single
block and rayon never splits. The worst region is 2 048 — 16 384 elements,
where waking workers costs 15-37 us against a kernel that takes 0.6-20 us.

# Limitations

Measured on one 4-core machine only, idle; see [`SPMV_MIN_CELLS`] for the
same caveats, which apply unchanged. `norm_l1` and `norm_l2` were **not**
separately benchmarked — they share `dot`'s blocked-reduction structure and
inherit its floor, which is an assumption, not a measurement.

# Units

A count of vector elements, dimensionless.

```rust
pub const VECOP_MIN_ELEMENTS: usize = 262_144;
```

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
    pub backend: crate::compute::ComputeBackend,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `preconditioner` | `PreconditionerKind` | Preconditioner to build from the matrix. Default<br>[`PreconditionerKind::Ilu0`]. |
| `restart` | `usize` | GMRES restart length `m` — the Krylov subspace dimension per outer cycle.<br>Larger `m` converges in fewer total inner iterations but costs `O(m·n)`<br>memory. Ignored by [`KrylovMethod::BiCGStab`]. `0` means "no restart"<br>(`m = max_iter`). Default `30`. |
| `backend` | `crate::compute::ComputeBackend` | Where the solver's kernels run — the sparse products, the inner products,<br>the vector updates and the Jacobi preconditioner application.<br><br>This is the field through which the whole finite-volume layer reaches the<br>hybrid backend: [`FvMatrix::solve_bicgstab`](crate::ldu_matrix::FvMatrix::solve_bicgstab)<br>and its siblings take a `KrylovOptions`, so setting this is the only<br>change a caller needs to make.<br><br>**Default [`ComputeBackend::Serial`]**, deliberately: `Serial` is this<br>workspace's documented oracle and the default must be the trusted path.<br>Because every kernel the solvers use is bitwise identical between<br>`Serial` and [`ComputeBackend::CpuMulti`], switching this changes wall<br>clock only — not the answer, not the residual history, not the iteration<br>count. A backend whose Cargo feature is off, or whose hardware is absent,<br>degrades instead of failing. |

##### Implementations

###### Methods

- ```rust
  pub fn with_preconditioner(preconditioner: PreconditionerKind) -> Self { /* ... */ }
  ```
  Options using the given preconditioner and the default restart (`30`) and

- ```rust
  pub fn on_backend(backend: ComputeBackend) -> Self { /* ... */ }
  ```
  Options using the given execution backend, otherwise the defaults

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    Defaults: ILU(0) preconditioning, GMRES restart `m = 30`, and the

- **Freeze**
- **From**
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

#### Function `krylov_solve_prepared`

Solve `A·x = b` with an asymmetric Krylov method, reusing a caller-owned
cell-gather index and a caller-owned preconditioner.

The form [`krylov_solve`] delegates to, and the one a solver loop should call.
It differs only in **what the caller has already prepared**, not in where it
runs — the backend is [`KrylovOptions::backend`] in both cases, so this is not
a parallel sibling of a serial function.

# Why prepare anything

[`krylov_solve`] pays two costs on every call that a repeated solve should not:

- it clones `a` into an [`Arc`] and rebuilds the
  `O(n_cells + n_internal_faces)` cell-gather index
  ([`LduTopology`](crate::ldu_matrix::parallel::LduTopology)), and
- it rebuilds the preconditioner, which for
  [`PreconditionerKind::Ilu0`] is a full incomplete factorisation.

A finite-volume solver reassembles coefficients every outer iteration while
the **mesh addressing never changes**, so it should build the index once and
refresh it with [`HybridLdu::with_matrix`], which is an addressing check
rather than a rebuild. The preconditioner does have to be rebuilt whenever the
coefficients change — ILU(0) factorises them — so it is passed in explicitly
rather than silently reused, which would degrade convergence without any
visible symptom.

# Arguments

- `ldu` — the prepared sparse system.
- `b` — right-hand side, length `n_cells`.
- `x0` — optional initial guess.
- `method` — BiCGStab or GMRES(m).
- `precond` — preconditioner built from **these** coefficients.
- `options` — GMRES restart length and the execution backend.
  [`KrylovOptions::preconditioner`] is **ignored** here, because the
  preconditioner is supplied directly.
- `settings` — `tolerance` and `max_iter`.

# Returns

As [`krylov_solve`].

# Example

```rust
use std::sync::Arc;
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::krylov::Preconditioner;
use outram_foam_basic_lib::ldu_matrix::parallel::HybridLdu;
use outram_foam_basic_lib::ldu_matrix::{
    krylov_solve_prepared, KrylovMethod, KrylovOptions, LduMatrix, SolverSettings,
};

let mut a = LduMatrix::new(3, vec![0, 1], vec![1, 2]);
a.diag = vec![4.0, 4.0, 4.0];
a.lower = vec![-2.0, -2.0];
a.upper = vec![-1.0, -1.0];
let b = vec![1.0, 2.0, 3.0];

let precond = Preconditioner::ilu0(&a);
let ldu = HybridLdu::new(Arc::new(a));

let (x, perf) = krylov_solve_prepared(
    &ldu,
    &b,
    None,
    KrylovMethod::BiCGStab,
    &precond,
    KrylovOptions::on_backend(ComputeBackend::CpuMulti),
    &SolverSettings::default(),
);
assert!(perf.converged);

let ax = ldu.spmv(&x, ComputeBackend::Serial);
for i in 0..3 {
    assert!((ax[i] - b[i]).abs() < 1e-6);
}
```

```rust
pub fn krylov_solve_prepared(ldu: &crate::ldu_matrix::parallel::HybridLdu, b: &[f64], x0: Option<&[f64]>, method: KrylovMethod, precond: &crate::krylov::Preconditioner, options: KrylovOptions, settings: &crate::ldu_matrix::fv_matrix::SolverSettings) -> (Vec<f64>, crate::ldu_matrix::fv_matrix::SolverPerformance) { /* ... */ }
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

#### Re-export `krylov_solve_prepared`

```rust
pub use krylov_solve::krylov_solve_prepared;
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

#### Re-export `HybridLdu`

```rust
pub use parallel::HybridLdu;
```

#### Re-export `LduTopology`

```rust
pub use parallel::LduTopology;
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

#### Re-export `krylov_solve_prepared`

```rust
pub use solvers::krylov_solve::krylov_solve_prepared;
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
It also hosts [`parallel`](crate::math::parallel), the **batched** root finders — bisection, Brent,
bracket-safeguarded Newton, and batched closed-form polynomial roots — each
dispatched across [`crate::compute::ComputeBackend`]. They live here rather
than under `polynomial/` because the general case solves a caller-supplied
residual such as `h(T) - h_target`, which is not a polynomial at all, and
because this module is already where the crate's iterative inversions live.

```rust
pub mod math { /* ... */ }
```

### Modules

## Module `differentiate`

**Numerical differentiation of a supplied function** — finite differences,
batched derivatives, and batched Jacobians, dispatched across
[`ComputeBackend`].

# This is NOT the FV gradient operator

[`crate::fv_operators`] implements the *spatial* finite-volume `grad`, `div`
and `laplacian` over a mesh: they differentiate a **field** with respect to
**position**, using face fluxes and cell volumes, and they are the right tool
for a PDE discretisation. This module differentiates an arbitrary
**caller-supplied function** with respect to its own arguments, by sampling
it. If you are discretising a transport equation you want `fv_operators`; if
you need `df/dx` of a closure, a property correlation or an ODE right-hand
side, you are in the right place.

# The problem this exists to solve

[`crate::ode::OdeSystem::jacobian`] has a default body that is
`unimplemented!()`. Any system that does not hand-code its Jacobian
**panics** the moment [`crate::ode::Rosenbrock23`] — the crate's only stiff
solver — asks for one, and inside
[`crate::ode::parallel::integrate_ensemble`] that panic propagates out
through the `rayon` scope. [`NumericalJacobian`] closes that hole: wrap a
system, and `Rosenbrock23` integrates it with no hand-coded Jacobian at all.
Measured cost of doing so, on Van der Pol (`mu = 5`, `x` in `[0, 10]`,
tolerances `1e-8`) — see "Measured cost against a hand-coded Jacobian" below
— is **1.9x** wall clock for a forward difference and 2.0x for a central
one, for the same answer to all eight printed decimals.

# Provenance — a generalisation of two settled workspace conventions

Nothing here is a new algorithm. Both halves of the formulation are taken
from implementations already working in this workspace, and the divergences
are stated rather than left to be discovered.

**The Jacobian assembly** generalises:

```text
crates/outram-park-fork-dwsim-libs/src/columns/linalg.rs:183
    pub fn finite_difference_jacobian<F>(f: &mut F, x: &[f64], epsilon: f64)
        -> Option<Array2<f64>>
```

itself a port of DWSIM's `NewtonRaphson.vb:669-705` (`FunctionGradient`),
used there for the Naphtali-Sandholm column solver's initial Broyden
Jacobian. Kept from it: the **central** stencil, the **relative**
perturbation, and the **failure-is-`Option`** convention — a caller never
receives a matrix it cannot tell apart from a good one. `dwsim-libs` cannot
be depended on from here (this crate has no internal workspace dependencies,
by policy), so this is a reuse of *formulation*, not of code.

Three deliberate divergences from it, each because the alternative is a known
defect:

| This module | `finite_difference_jacobian` | Why |
|---|---|---|
| `h = rel * max(\|x\|, min_scale)` | `x*(1±eps)`, or `eps` and `2*eps` when `x == 0` | The `x == 0` branch silently switches to a *one-sided* stencil at a *different* step, so the scheme changes with the data. The `max` floor keeps one scheme everywhere. |
| A failed entry is `NaN` and the status says so | a non-finite entry is written as `0.0` | A zero is a *plausible* Jacobian entry. It cannot be detected downstream, and a Newton or Rosenbrock step built on it returns a wrong answer instead of an error. |
| Divides by the realised step `xp - xm` | divides by the requested `2*eps*x` | `x + h` is not representable, so the requested step is not the step taken. See [`derivative`]. |

**The step-size rule** is the one already in:

```text
crates/outram-park-fork-offbeat/src/rheology/aster/integration.rs:298
    pub fn newton_perturbed(...)   // h = perturbation * x.abs().max(1.0)
    pub fn perturbed_default() -> f64   // f64::EPSILON.cbrt()
```

which is upstream Code_Aster's `NEWTON_PERT`. This module adopts both the
`max(|x|, 1)` floor and `eps^(1/3)` for the central scheme verbatim — see
[`CBRT_EPSILON`] and [`DiffSettings::step_for`] — and extends the same
reasoning to the other three orders.

A third, narrower precedent —
`tampines-steam-tables`' `w_ps_eqm_region4_finite_diff_vol`
(`region_4_vap_liq_equilibrium/speed_of_sound_eqm.rs:83`) — takes
`dv/dp|_s` by central differences with a hard-coded `dp = 1e-4 * p`, clamped
at the minimum table pressure. That is a *relative* step of `1e-4`, sixteen
times coarser than [`CBRT_EPSILON`], which is the right call there because
the IF97 flash it differentiates is far noisier than machine epsilon. It is
recorded here as the standing reminder that **the optimal step assumes the
function is evaluated to rounding accuracy**, and a caller whose function is
noisier should raise [`DiffSettings::relative_step`] accordingly.

# Achievable accuracy is `sqrt(eps)` to `eps^(4/5)`, NEVER `eps`

This is the single most common misunderstanding about finite differences, so
it is measured rather than asserted. Truncation error falls as `h^p` while
round-off grows as `eps/h`; their sum is minimised at `h ~ eps^(1/(p+1))`,
where the achievable accuracy is `~ eps^(p/(p+1))`:

| Scheme | Order `p` | Optimal `h` | Predicted accuracy |
|---|---|---|---|
| [`DiffScheme::Forward`] / [`DiffScheme::Backward`] | 1 | `sqrt(eps) = 1.49e-8` | `sqrt(eps) = 1.49e-8` |
| [`DiffScheme::Central`] | 2 | `eps^(1/3) = 6.06e-6` | `eps^(2/3) = 3.67e-11` |
| [`DiffScheme::Central4th`] | 4 | `eps^(1/5) = 7.40e-4` | `eps^(4/5) = 3.00e-13` |

**A caller expecting `1e-15` from a finite difference will be wrong by seven
orders of magnitude for a forward difference.** If you need machine
precision, hand-code the derivative.

*Measured.* `accuracy_floor_at_the_default_step` in `differentiate/tests.rs`,
release, 2026-08-13. Worst relative error over six points in `[0.25, 3.3]`,
each scheme at its own default step:

| Function | forward | backward | central | central-4th |
|---|---|---|---|---|
| `sin` | 1.281191e-8 | 1.401018e-8 | 6.528067e-11 | 1.785239e-13 |
| `exp` | 2.413003e-8 | 2.383524e-8 | 6.640831e-11 | 1.800352e-13 |
| `x^3 - 2x` | 4.470348e-8 | 4.470348e-8 | 4.583556e-11 | 9.992007e-14 |
| `1/(1 + x^2)` | 8.046627e-9 | 8.430242e-9 | 2.949979e-11 | 9.620083e-14 |
| `tanh` | 6.775837e-9 | 8.125324e-9 | 1.244682e-11 | 1.963985e-13 |
| **worst** | **4.470348e-8** | **4.470348e-8** | **6.640831e-11** | **1.963985e-13** |
| *predicted* | *1.490116e-8* | *1.490116e-8* | *3.666853e-11* | *3.000214e-13* |

Every scheme lands within a factor of three of its prediction, and the
**ordering is exactly as predicted: central-4th beats central by 338x, and
central beats forward by 673x.** The theory is a usable bound, not a story.

*Observed convergence order.* `observed_convergence_order_matches_theory`,
same run. Absolute error of `d/dx sin(x)` at `x = 1` (exact
`5.40302305868139765e-1`) against the relative step:

| Relative step | forward | backward | central | central-4th |
|---|---|---|---|---|
| `1e-1` | 4.293855e-2 | 4.113845e-2 | 9.000537e-4 | 1.125295e-7 |
| `1e-2` | 4.216325e-3 | 4.198315e-3 | 9.004993e-6 | 1.126843e-11 |
| `1e-3` | 4.208255e-4 | 4.206454e-4 | 9.005042e-8 | 1.049161e-13 |
| `1e-4` | 4.207445e-5 | 4.207265e-5 | 9.003700e-10 | 3.312906e-13 |
| **observed order** | **1.0079** | **0.9912** | **1.9998** | **3.9994** |

The fourth-order column turning back upward at `1e-4` (1.05e-13 to 3.31e-13)
is the round-off wall arriving, exactly where `eps^(1/5) = 7.4e-4` says it
should.

*The round-off wall.* `a_step_far_below_the_optimum_is_worse_not_better`,
same run. Central difference, `d/dx sin(x)` at `x = 1`, relative error:

| Relative step | Relative error |
|---|---|
| `1.0000e-2` | 1.666658e-5 |
| `6.0555e-6` ([`CBRT_EPSILON`]) | **5.373555e-12** |
| `1.0000e-8` | 5.303737e-9 |
| `1.0000e-10` | 1.909780e-7 |
| `1.0000e-12` | 5.609880e-5 |
| `1.0000e-14` | 7.666335e-3 |

**A step six million times smaller than the optimum is ten million times
worse:** `1e-12` against `6.06e-6` moves the error from 5.373555e-12 to
5.609880e-5, a factor of 1.04e7. "Make `h` tiny for accuracy" is the intuition this table exists to
destroy.

# Forward against central: the cost/accuracy trade, measured

For an `n`-dimensional Jacobian the evaluation counts are `n + 1`, `2n` and
`4n` (see [`DiffScheme::evaluations_per_jacobian`]). The measured accuracy
ratio from the table above is **673x** for one extra evaluation per column
going forward to central, and a further **338x** for two more going to
`Central4th`. Central is very nearly always the right default, which is why
it is what [`DiffSettings::central`] exists for and what
[`NumericalJacobian`]'s documentation recommends; [`DiffScheme::Forward`] is
for the case where the function is genuinely expensive and `1e-8` is enough.

# Verification against analytic Jacobians

*Methodology.* Three systems whose Jacobians can be written down exactly are
differenced and compared entry by entry: a **quadratic** system
`[x0^2 + x1, x0*x1^2]`, a **trigonometric** system
`[sin(x0)cos(x1), exp(x0)*x1]` in which no derivative of any order vanishes,
and a **stiff linear pair** `[-1000*y0 + y1, y0 - y1]` with a 1000:1 entry
spread. Pass criterion: worst relative error below `1e-7` for the first-order
schemes and `1e-10`/`1e-11` for the higher-order ones. All four schemes pass
on all three systems (release, 2026-08-13).

*The stiff pair is the informative one*, because a **linear** system has
exactly zero truncation error — so every digit lost is cancellation, and the
measurement isolates it. Absolute error per entry at `y = [0.4, -0.9]`:

| Scheme | `J[0][0]` (-1000) | `J[0][1]` (1) | `J[1][0]` (1) | `J[1][1]` (-1) |
|---|---|---|---|---|
| forward | 0 | 0 | 0 | 0 |
| backward | 0 | 0 | 0 | 0 |
| central | 6.600658e-10 | **1.778424e-9** | 9.167112e-12 | 1.833422e-11 |
| central-4th | 5.456968e-12 | 6.380474e-11 | 2.498002e-14 | 0 |

**`J[0][1]` is the worst entry by two orders of magnitude, and it is the
small entry in the row that also holds `-1000`.** Row 0 evaluates to about
`-400.9`; perturbing `x1` changes it by `1.2e-5`, so forming that difference
discards eleven significant digits before the division. This is a **general
property of finite-difference Jacobians, not of this implementation**: an
entry is only as accurate as its own magnitude relative to the largest term
in its row. A badly-scaled system loses precision in exactly the entries a
stiff solver most needs. If that matters, hand-code the Jacobian or rescale
the equations.

The one-sided schemes returning **exactly zero** error here is a property of
this particular linear case (`f(x+h) - f(x)` is exact when `f` is affine and
the operands round identically) and must not be read as one-sided differences
being more accurate in general — the accuracy table above shows them 673x
*worse* on smooth non-linear functions.

# Measured cost against a hand-coded Jacobian

*Methodology.* Van der Pol `mu = 5`, `y(0) = [2, 0]`, integrated over
`x` in `[0, 10]` by [`crate::ode::Rosenbrock23`] with `abs_tol = rel_tol =
1e-8`; best of 5, release, default features, the loaded 4-core machine
described on [`DERIVATIVE_BATCH_MIN_POINTS`]. Van der Pol *has* an analytic
Jacobian, which is the baseline. Produced by the `#[ignore]`d
`numerical_jacobian_overhead_benchmark`, two runs.

| Jacobian | Time (A) | Time (B) | vs analytic (A) | (B) | `y0(10)` |
|---|---|---|---|---|---|
| analytic (hand-coded) | 849.99 us | 855.47 us | 1.00x | 1.00x | -1.15870127 |
| [`DiffScheme::Forward`] | 1619.03 us | 1603.88 us | 1.90x | 1.87x | -1.15870127 |
| [`DiffScheme::Backward`] | 1607.45 us | 1614.33 us | 1.89x | 1.89x | -1.15870127 |
| [`DiffScheme::Central`] | 1673.66 us | 1677.88 us | 1.97x | 1.96x | -1.15870127 |
| [`DiffScheme::Central4th`] | 2487.96 us | 2455.24 us | 2.93x | 2.87x | -1.15870127 |

**All four schemes reproduce the analytic result to all eight printed
decimals**, at roughly twice the cost. That is the honest headline: a
numerical Jacobian is not free, it is not exact, and for a `n = 2` stiff
system it costs about a factor of two and changes nothing you can see.

# A NaN Jacobian is NOT reported by the solver — check the counter

When a Jacobian cannot be differenced, this module writes `NaN` into the
entries and says so through [`DiffStatus`]. The natural expectation is that
`Rosenbrock23` then fails loudly. **It does not, and this was measured
rather than assumed** (`a_jacobian_that_cannot_be_differenced_is_counted_and_reaches_the_solver_as_nan`):

- [`crate::ode::Rosenbrock23::integrate`] returns **`Ok(())`**;
- the state vector comes back **`NaN`**.

The cause is in the ODE layer, not here: `ode::normalize_error` folds the
per-equation errors with `f64::max`, and `f64::max(0.0, NaN)` is `0.0` — so a
`NaN` error estimate looks like a *perfectly converged* step and every
sub-step is accepted. Nothing in this module can change that.

**Consequence for callers:** [`NumericalJacobian::non_finite_jacobians`] is
the only in-band signal that anything went wrong. Check it after any
integration whose result you intend to trust. It is not decoration.

# Hybrid means dispatch, not two APIs

Every entry point takes a [`ComputeBackend`] parameter; there is no
`_parallel()` sibling. With the `parallel` feature off,
[`ComputeBackend::CpuMulti`] resolves down to [`ComputeBackend::Serial`] and
the answer is unchanged — bit for bit, not merely close. There is no `Gpu`
kernel here yet, so a `Gpu` request degrades to the best available CPU path.

**Two independent parallel axes** live here, with separately measured
crossovers 2048x apart:

| Entry point | Parallel over | Crossover |
|---|---|---|
| [`derivative_batch`] | independent points | [`DERIVATIVE_BATCH_MIN_POINTS`] = 65 536 |
| [`jacobian_batch`] | independent lanes | [`JACOBIAN_BATCH_MIN_PROBLEMS`] = 256 |
| [`jacobian`] | the columns of **one** Jacobian | [`JACOBIAN_COLUMN_MIN_DIMENSION`] = 32 |

They are not nested: [`jacobian_batch`] runs each lane's columns serially,
because the lane axis is already saturating the pool.

# Determinism — bitwise identical across backends and thread counts

**This module returns bit-for-bit identical output on
[`ComputeBackend::Serial`] and [`ComputeBackend::CpuMulti`], at any thread
count, on every run**, provided the caller's function is a deterministic pure
function of its arguments.

The argument is the same one [`crate::math::minimise`] makes: lane `i`'s (or
column `j`'s) answer is a pure function of its own samples, and **no
arithmetic crosses lanes or columns**. A parallel *sum* would have to
re-associate, and floating-point addition is not associative; a set of
independent difference quotients has nothing to re-associate. Both backends
call the same `#[inline]` per-lane and per-column kernels, and only the
identity of the calling thread differs.

Verified by the `bitwise_*` tests in `differentiate/tests.rs` on 2 048
derivative lanes, 512 four-dimensional Jacobian lanes and one 96-dimensional
Jacobian, all built with points spread over seven decades so the per-lane
step differs. **Measured 2026-08-13 (release, `--features parallel`, 4
logical cores): bit-identical on every observable field of every lane, for
all four [`DiffScheme`] variants, at 1, 2, 4 and 8 workers.** The
single-point [`derivative`] and single-lane [`jacobian`] forms are separately
asserted bit-identical to their one-element batches.

The `#[ignore]`d `differentiate_thread_scaling_benchmark` re-asserts the same
identity while timing it, on 65 536 lanes with
[`DiffScheme::Central4th`] (4 evaluations per lane), best of 7, two runs:

| Worker threads | Time (A) | Speed-up (A) | (B) | Bitwise vs serial |
|---|---|---|---|---|
| *serial reference* | 5902.59 us | 1.00x | 1.00x | — |
| 1 | 6008.47 us | 0.98x | 0.98x | identical |
| 2 | 3181.82 us | 1.86x | 1.85x | identical |
| 4 | 1582.50 us | 3.73x | 3.70x | identical |
| 8 | 1577.90 us | 3.74x | 3.83x | identical |

The "identical" column is asserted by the benchmark, not merely printed.
Scaling is close to linear to 4 workers and flat beyond, which is what four
logical cores should do. **The machine was not idle** (see
[`DERIVATIVE_BATCH_MIN_POINTS`] for the load); one machine, one batch, two
runs, nothing measured on Android hardware or a many-core server.

The one way a caller can break this is to supply a function that is not pure
— one that reads a random number generator, accumulates into shared
interior-mutable state, or depends on the calling thread. The `Sync` bound
permits it; this contract forbids it.

# Failure is reported, never swallowed

- Every lane and every Jacobian carries a [`DiffStatus`].
- [`DerivativeSolution::derivative`] and [`JacobianSolution::matrix`] return
  `Option`, `Some` **only** on success. The diagnostic values are behind the
  deliberately-named [`DerivativeSolution::raw_value`] and
  [`JacobianSolution::raw_matrix`].
- [`DerivativeBatch::values`] and [`JacobianBatch::matrices`] are
  all-or-nothing: they return [`DiffBatchFailure`] naming the failure count
  and the first failing lane, rather than a `Vec` with a `NaN` in it.
- A failed Jacobian column is `NaN`, never `0.0`, and
  [`JacobianSolution::first_bad_column`] names the offending **variable**.

**But read the limits of that guarantee** — the "What is detected, and what
cannot be" section on [`derivative`] lists three classes of bad input that no
finite-difference kernel can detect, chief among them a singularity that a
symmetric stencil steps over.

# What is deliberately NOT here: dual-number autodiff

Bead `op-yvj.4.6` offers forward-mode dual numbers as an optional exact
alternative, *"only if it stays simple"*. It is not implemented, on purpose.
Making it useful means every function a caller wants differentiated must be
generic over the scalar type, which would push a type parameter through
`OdeSystem`, through the thermophysics kernels, and into every caller's own
code. That is precisely the rise in reader context load the crate-level
"Human interface layer" rule forbids, and the bead itself ranks that rule
above the convenience. A caller who wants exact derivatives should hand-code
them; that is what [`crate::ode::OdeSystem::jacobian`] is for.

# Units

Everything here is dimensionless `f64`, and that is a deliberate decision
rather than `uom` being stripped.

**A derivative changes dimension.** `d(enthalpy)/d(temperature)` is a heat
capacity; `d(pressure)/d(volume)` is none of the three. A generic
differentiator therefore has no single `uom` type it could return — the
output type is a *function* of two input types, which Rust can only express
through a trait with an associated output type, i.e. exactly the generic
machinery the "Human interface layer" rule forbids adding for its own sake.
The bead anticipates this and directs that, where the generic form cannot be
typed cleanly, a small number of **concrete typed wrappers** is preferred
over one generic nobody can read.

So: `uom` typing is applied **at the boundary, by the caller** — convert in,
convert out — exactly as [`crate::math::minimise`] and
[`crate::math::parallel`] do. The one place a dimension does appear in this
module's own API is [`DiffSettings::min_scale`], which carries the units of
the variable being perturbed; its documentation says so, because a caller
differentiating with respect to a pressure in pascals near zero must not
leave it at `1.0`.

# Cargo features and portability

The `rayon` paths sit behind the crate's `parallel` feature, which is **off
by default**; with it off this module still compiles and every entry point
still works. `rayon` is pure Rust with no system component, so everything
here compiles and runs on `aarch64-linux-android` / Termux exactly as on
desktop. Nothing in this module is target-gated.

# Example

```rust
use outram_foam_basic_lib::math::differentiate::{DiffSettings, NumericalJacobian};
use outram_foam_basic_lib::ode::{OdeSystem, Rosenbrock23};

// A stiff system with NO hand-coded Jacobian. Without the wrapper the
// default `OdeSystem::jacobian` would panic inside Rosenbrock23.
struct StiffPair;
impl OdeSystem for StiffPair {
    fn n_eqns(&self) -> usize { 2 }
    fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
        dydx.clear();
        dydx.push(-1000.0 * y[0] + y[1]);
        dydx.push(y[0] - y[1]);
    }
}

let system = NumericalJacobian::new(StiffPair, DiffSettings::central());
let mut solver = Rosenbrock23::new(2, 1e-10, 1e-10);
let mut y = vec![1.0_f64, 1.0];
let mut dx = 1e-6;
solver.integrate(&system, 0.0, 1.0, &mut y, &mut dx).expect("integrates");

// The fast mode has decayed; the slow mode (eigenvalue about -0.999) remains.
assert!(y[1].abs() < 1.0 && y[1] > 0.0, "y1 = {}", y[1]);
// ALWAYS check this -- the solver does not report a NaN Jacobian itself.
assert_eq!(system.non_finite_jacobians(), 0);
```

```rust
pub mod differentiate { /* ... */ }
```

### Types

#### Enum `DiffScheme`

Which finite-difference stencil to use.

The choice is a **cost against accuracy** trade, and both halves are
measured — see the module-level "Achievable accuracy" table for the observed
error floors and "Cost" for the evaluation counts.

# Units

Dimensionless — a mode selector, not a quantity.

```rust
pub enum DiffScheme {
    Forward,
    Backward,
    Central,
    Central4th,
}
```

##### Variants

###### `Forward`

`(f(x + h) - f(x)) / h`. Truncation error `O(h)`.

The cheapest scheme for a Jacobian: the base evaluation `f(x)` is shared
by every column, so an `n`-dimensional Jacobian costs `n + 1`
evaluations rather than `2n`.

###### `Backward`

`(f(x) - f(x - h)) / h`. Truncation error `O(h)`.

Same cost and accuracy as [`Forward`](Self::Forward); it exists for
callers whose function is undefined or unphysical just *above* `x` — a
saturation pressure at the phase boundary, a volume fraction at 1.

###### `Central`

`(f(x + h) - f(x - h)) / (2h)`. Truncation error `O(h^2)`. **The
default.**

This is the scheme both existing workspace implementations use — see the
module-level "Provenance" section.

###### `Central4th`

Richardson extrapolation of two central differences, `(4*D(h/2) -
D(h)) / 3`. Truncation error `O(h^4)`.

The most accurate scheme here and the most expensive: 4 evaluations per
derivative and `4n` per Jacobian, because the `h` and `h/2` stencils
share no points.

##### Implementations

###### Methods

- ```rust
  pub fn default_relative_step(self: Self) -> f64 { /* ... */ }
  ```
  The relative step size that balances truncation against round-off for

- ```rust
  pub fn evaluations_per_derivative(self: Self) -> usize { /* ... */ }
  ```
  How many evaluations of the function one **scalar** derivative costs.

- ```rust
  pub fn evaluations_per_jacobian(self: Self, n: usize) -> usize { /* ... */ }
  ```
  How many evaluations of the vector function an `n`-dimensional Jacobian

- ```rust
  pub fn label(self: Self) -> &'static str { /* ... */ }
  ```
  A short human-readable label, for benchmark tables and log lines.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> DiffScheme { /* ... */ }
    ```

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
    fn default() -> DiffScheme { /* ... */ }
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
    fn eq(self: &Self, other: &DiffScheme) -> bool { /* ... */ }
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
#### Struct `DiffSettings`

Step-size policy for every entry point in this module.

# The step-size rule

```text
h = relative_step * max(|x|, min_scale)
```

The step is **relative to the magnitude of the variable being perturbed**,
because a step that is right for `x ~ 1` is far too small for a pressure in
pascals and far too large for a mole fraction. `min_scale` is the floor that
keeps the rule usable at `x = 0` — see [`Self::step_for`].

# Units

`relative_step` is dimensionless. `min_scale` carries the **same units as
the variable being differentiated with respect to**, because it is a
fallback magnitude for `x`, and its default of `1.0` therefore means "one of
whatever unit `x` is in". A caller differentiating with respect to a
pressure in pascals near zero wants `min_scale` set to a pascal-scale
number, not `1.0`.

```rust
pub struct DiffSettings {
    pub scheme: DiffScheme,
    pub relative_step: f64,
    pub min_scale: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `scheme` | `DiffScheme` | Which stencil to use. See [`DiffScheme`]. |
| `relative_step` | `f64` | The relative step, dimensionless. Defaults to<br>[`DiffScheme::default_relative_step`] for the chosen scheme. |
| `min_scale` | `f64` | Floor on `|x|` in the step rule, so `x = 0` still gets a usable step.<br>Same units as `x`. Default `1.0`. |

##### Implementations

###### Methods

- ```rust
  pub fn with_scheme(scheme: DiffScheme) -> Self { /* ... */ }
  ```
  Settings for `scheme`, with that scheme's optimal relative step and

- ```rust
  pub fn forward() -> Self { /* ... */ }
  ```
  [`DiffScheme::Forward`] with its optimal relative step — the `n + 1`

- ```rust
  pub fn backward() -> Self { /* ... */ }
  ```
  [`DiffScheme::Backward`] with its optimal relative step.

- ```rust
  pub fn central() -> Self { /* ... */ }
  ```
  [`DiffScheme::Central`] with its optimal relative step — the `2n`

- ```rust
  pub fn central_4th() -> Self { /* ... */ }
  ```
  [`DiffScheme::Central4th`] with its optimal relative step — the `4n`

- ```rust
  pub fn step_for(self: &Self, x: f64) -> f64 { /* ... */ }
  ```
  The step this policy uses to perturb a variable currently at `x`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> DiffSettings { /* ... */ }
    ```

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
    [`DiffScheme::Central4th`] with its optimal relative step and

- **Freeze**
- **From**
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
    fn eq(self: &Self, other: &DiffSettings) -> bool { /* ... */ }
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
#### Enum `DiffStatus`

Why a derivative or Jacobian entry is, or is not, trustworthy.

# Units

Dimensionless — a status code.

```rust
pub enum DiffStatus {
    Ok,
    InvalidPoint,
    NotFinite,
    DegenerateStep,
    DimensionMismatch,
}
```

##### Variants

###### `Ok`

The difference quotient was formed from finite evaluations with a
non-degenerate step. The value is usable.

###### `InvalidPoint`

The point `x` itself was not finite, so no step could be taken.

###### `NotFinite`

At least one function evaluation returned a non-finite value, or the
difference quotient itself came out non-finite (overflow in the
subtraction, for instance).

###### `DegenerateStep`

The step collapsed: `relative_step` was zero, negative or non-finite, or
`x + h` rounded back to `x` so the realised step was exactly zero. The
quotient would have been a division by zero.

###### `DimensionMismatch`

The vector function returned a different number of components than the
point has, so the Jacobian is not square and cannot be assembled.

Only reachable from [`jacobian`] and its batched form; the square
restriction is documented on [`jacobian`].

##### Implementations

###### Methods

- ```rust
  pub fn is_ok(self: Self) -> bool { /* ... */ }
  ```
  Whether the value this status accompanies may be used.

- ```rust
  pub fn label(self: Self) -> &'static str { /* ... */ }
  ```
  A short human-readable label, for log lines and benchmark tables.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> DiffStatus { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &DiffStatus) -> bool { /* ... */ }
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
#### Struct `DerivativeSolution`

One lane's scalar derivative, with the diagnostics needed to judge it.

# Units

[`Self::raw_value`] carries the units of `f` divided by the units of `x`
— a derivative changes dimension, which is exactly why this module does not
try to `uom`-type the generic form. See the module-level "Units" section.
[`realised_step`](Self::realised_step) carries the units of `x`.

```rust
pub struct DerivativeSolution {
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
  pub fn derivative(self: &Self) -> Option<f64> { /* ... */ }
  ```
  The derivative, **only if** this lane succeeded.

- ```rust
  pub fn raw_value(self: &Self) -> f64 { /* ... */ }
  ```
  The difference quotient as computed, whatever the status — a diagnostic,

- ```rust
  pub fn realised_step(self: &Self) -> f64 { /* ... */ }
  ```
  The step actually taken, after the `x + h` rounding correction described

- ```rust
  pub fn status(self: &Self) -> DiffStatus { /* ... */ }
  ```
  Why this lane succeeded or failed.

- ```rust
  pub fn is_ok(self: &Self) -> bool { /* ... */ }
  ```
  Whether this lane produced a usable derivative.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> DerivativeSolution { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &DerivativeSolution) -> bool { /* ... */ }
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
#### Struct `DiffBatchFailure`

**Attributes:**

- `Other("#[error(\"{failure_count} of {total} differentiation lanes failed; \\\n     first failure at lane {first_index} with status {first_status:?}\")]")`

One or more lanes of a [`DerivativeBatch`] or [`JacobianBatch`] failed.

Returned by the all-or-nothing accessors [`DerivativeBatch::values`] and
[`JacobianBatch::matrices`]. It names both the scale of the problem (how
many of how many) and a specific lane to look at, because "3 of 10 000 lanes
failed" is only actionable once you know *which* lane.

# Units

All counts and indices are dimensionless.

```rust
pub struct DiffBatchFailure {
    pub total: usize,
    pub failure_count: usize,
    pub first_index: usize,
    pub first_status: DiffStatus,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `total` | `usize` | Number of lanes in the batch. |
| `failure_count` | `usize` | Number of lanes that failed. |
| `first_index` | `usize` | Index of the first failing lane. |
| `first_status` | `DiffStatus` | Why that lane failed. |

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
    fn clone(self: &Self) -> DiffBatchFailure { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **Eq**
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
    fn eq(self: &Self, other: &DiffBatchFailure) -> bool { /* ... */ }
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
#### Struct `DerivativeBatch`

The result of [`derivative_batch`] — one [`DerivativeSolution`] per point,
in point order.

```rust
pub struct DerivativeBatch {
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
  pub fn solutions(self: &Self) -> &[DerivativeSolution] { /* ... */ }
  ```
  Every lane's solution, in the order the points were supplied.

- ```rust
  pub fn into_solutions(self: Self) -> Vec<DerivativeSolution> { /* ... */ }
  ```
  Consume the batch, yielding the per-lane solutions.

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Number of lanes.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether the batch has no lanes.

- ```rust
  pub fn get(self: &Self, i: usize) -> Option<DerivativeSolution> { /* ... */ }
  ```
  Lane `i`'s solution, or `None` if `i` is out of range.

- ```rust
  pub fn all_ok(self: &Self) -> bool { /* ... */ }
  ```
  Whether every lane produced a usable derivative.

- ```rust
  pub fn failure_count(self: &Self) -> usize { /* ... */ }
  ```
  How many lanes failed.

- ```rust
  pub fn first_failure(self: &Self) -> Option<(usize, DerivativeSolution)> { /* ... */ }
  ```
  The first failing lane and its solution, if any.

- ```rust
  pub fn failures(self: &Self) -> Vec<(usize, DerivativeSolution)> { /* ... */ }
  ```
  Every failing lane and its solution.

- ```rust
  pub fn values(self: &Self) -> Result<Vec<f64>, DiffBatchFailure> { /* ... */ }
  ```
  Every lane's derivative, **all or nothing**.

- ```rust
  pub fn check_all_ok(self: &Self) -> Result<(), DiffBatchFailure> { /* ... */ }
  ```
  `Err` describing the first failure, if any lane failed.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> DerivativeBatch { /* ... */ }
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
    fn eq(self: &Self, other: &DerivativeBatch) -> bool { /* ... */ }
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
#### Struct `JacobianSolution`

One lane's Jacobian, with the status needed to judge it.

# Units

Entry `(i, j)` of the matrix carries the units of `f_i` divided by the units
of `x_j`. See the module-level "Units" section for why the generic form is
not `uom`-typed.

```rust
pub struct JacobianSolution {
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
  pub fn matrix(self: &Self) -> Option<&SquareMatrix> { /* ... */ }
  ```
  The Jacobian, **only if** every column of this lane succeeded.

- ```rust
  pub fn into_matrix(self: Self) -> Option<SquareMatrix> { /* ... */ }
  ```
  Consume the solution, yielding the Jacobian only if it succeeded.

- ```rust
  pub fn raw_matrix(self: &Self) -> &SquareMatrix { /* ... */ }
  ```
  The matrix as assembled, whatever the status — a diagnostic, **not** an

- ```rust
  pub fn status(self: &Self) -> DiffStatus { /* ... */ }
  ```
  Why this lane succeeded or failed.

- ```rust
  pub fn is_ok(self: &Self) -> bool { /* ... */ }
  ```
  Whether this lane produced a usable Jacobian.

- ```rust
  pub fn first_bad_column(self: &Self) -> usize { /* ... */ }
  ```
  The index of the first column that failed, or `usize::MAX` if none did.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> JacobianSolution { /* ... */ }
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
#### Struct `JacobianBatch`

The result of [`jacobian_batch`] — one [`JacobianSolution`] per lane.

```rust
pub struct JacobianBatch {
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
  pub fn solutions(self: &Self) -> &[JacobianSolution] { /* ... */ }
  ```
  Every lane's solution, in the order the points were supplied.

- ```rust
  pub fn into_solutions(self: Self) -> Vec<JacobianSolution> { /* ... */ }
  ```
  Consume the batch, yielding the per-lane solutions.

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Number of lanes.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether the batch has no lanes.

- ```rust
  pub fn get(self: &Self, i: usize) -> Option<&JacobianSolution> { /* ... */ }
  ```
  Lane `i`'s solution, or `None` if `i` is out of range.

- ```rust
  pub fn all_ok(self: &Self) -> bool { /* ... */ }
  ```
  Whether every lane produced a usable Jacobian.

- ```rust
  pub fn failure_count(self: &Self) -> usize { /* ... */ }
  ```
  How many lanes failed.

- ```rust
  pub fn first_failure(self: &Self) -> Option<(usize, DiffStatus)> { /* ... */ }
  ```
  The first failing lane index and its status, if any.

- ```rust
  pub fn matrices(self: Self) -> Result<Vec<SquareMatrix>, DiffBatchFailure> { /* ... */ }
  ```
  Every lane's Jacobian, **all or nothing**.

- ```rust
  pub fn check_all_ok(self: &Self) -> Result<(), DiffBatchFailure> { /* ... */ }
  ```
  `Err` describing the first failure, if any lane failed.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> JacobianBatch { /* ... */ }
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
#### Struct `NumericalJacobian`

Wrap any [`OdeSystem`] so that [`crate::ode::Rosenbrock23`] can integrate it
**without a hand-coded Jacobian**.

# The problem this solves

[`OdeSystem::jacobian`] has a default body that is `unimplemented!()`, so a
system that does not override it panics the moment a stiff solver asks for a
Jacobian — inside `Rosenbrock23::inner_step`, and, if the integration is
running in an ensemble, out through the `rayon` scope. Every system that
only knows its own `derivatives` is locked out of the crate's only stiff
solver.

Wrapping it in `NumericalJacobian` supplies the missing method by finite
differences and changes nothing else: `n_eqns` and `derivatives` are
forwarded verbatim.

# Owning, not borrowing

The wrapper **owns** the system by value, so it needs no lifetime parameter
and no `Box` — both forbidden by the workspace design rules. Construct it
with [`Self::new`], get the system back with [`Self::into_inner`].

# An analytic Jacobian is still better

Finite differences cost `n + 1` to `4n` extra `derivatives` calls per
Rosenbrock stage and are accurate to roughly `sqrt(eps)` to `eps^(4/5)`
rather than to machine precision — see the module-level "Achievable
accuracy" table. If the analytic Jacobian is available, write it. This
wrapper is for the systems where it is not, and as a **verification oracle**
for the ones where it is: differencing a system that also implements
`jacobian` analytically and comparing is the cheapest real check that the
hand-derived version has no sign or transposition error.

# Units

Inherited from the wrapped system; nothing here is dimensioned.

# Example

```rust
use outram_foam_basic_lib::math::differentiate::{DiffSettings, NumericalJacobian};
use outram_foam_basic_lib::ode::{OdeSystem, Rosenbrock23};

// A stiff scalar system with NO hand-coded Jacobian: dy/dx = -1000 y.
struct StiffDecay;
impl OdeSystem for StiffDecay {
    fn n_eqns(&self) -> usize { 1 }
    fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
        dydx.clear();
        dydx.push(-1000.0 * y[0]);
    }
    // no `jacobian` override -- the default would panic
}

let system = NumericalJacobian::new(StiffDecay, DiffSettings::central());
let mut solver = Rosenbrock23::new(1, 1e-10, 1e-10);
let mut y = vec![1.0_f64];
let mut dx = 1e-5;
solver.integrate(&system, 0.0, 0.01, &mut y, &mut dx).expect("integrates");

// exp(-1000 * 0.01) = exp(-10) = 4.5399929762484854e-5
let exact = (-10.0_f64).exp();
assert!((y[0] - exact).abs() < 1e-8, "got {}, want {exact}", y[0]);
assert_eq!(system.non_finite_jacobians(), 0);
```

```rust
pub struct NumericalJacobian<S> {
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
  pub fn new(system: S, settings: DiffSettings) -> Self { /* ... */ }
  ```
  Wrap `system`, differencing its `derivatives` with `settings`.

- ```rust
  pub fn inner(self: &Self) -> &S { /* ... */ }
  ```
  Borrow the wrapped system.

- ```rust
  pub fn into_inner(self: Self) -> S { /* ... */ }
  ```
  Unwrap, returning the system.

- ```rust
  pub fn settings(self: &Self) -> DiffSettings { /* ... */ }
  ```
  The step-size policy in force.

- ```rust
  pub fn non_finite_jacobians(self: &Self) -> usize { /* ... */ }
  ```
  How many [`OdeSystem::jacobian`] calls have failed since construction.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **OdeSystem**
  - ```rust
    fn n_eqns(self: &Self) -> usize { /* ... */ }
    ```

  - ```rust
    fn derivatives(self: &Self, x: f64, y: &[f64], dydx: &mut Vec<f64>) { /* ... */ }
    ```

  - ```rust
    fn jacobian(self: &Self, x: f64, y: &[f64], dfdx: &mut Vec<f64>, dfdy: &mut SquareMatrix) { /* ... */ }
    ```

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
### Functions

#### Function `derivative_backend_for`

**Attributes:**

- `MustUse { reason: None }`

The [`ComputeBackend`] [`derivative_batch`] would actually use for `n`
points if asked for `requested` — without running anything.

Applies exactly the same reduction the kernel does (feature availability, no
GPU kernel here, and the [`DERIVATIVE_BATCH_MIN_POINTS`] size floor), so what
it reports is what would run.

# Arguments

- `requested` — the backend a caller would pass to [`derivative_batch`].
- `n` — number of independent points in the batch, dimensionless.

# Returns

Either [`ComputeBackend::Serial`] or [`ComputeBackend::CpuMulti`]; never
[`ComputeBackend::Gpu`], because no GPU kernel exists here yet.

# Example

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::math::differentiate::{
    derivative_backend_for, DERIVATIVE_BATCH_MIN_POINTS,
};

assert_eq!(
    derivative_backend_for(ComputeBackend::CpuMulti, 8),
    ComputeBackend::Serial
);
assert!(derivative_backend_for(ComputeBackend::CpuMulti, DERIVATIVE_BATCH_MIN_POINTS)
    .is_available());
```

```rust
pub fn derivative_backend_for(requested: crate::compute::ComputeBackend, n: usize) -> crate::compute::ComputeBackend { /* ... */ }
```

#### Function `jacobian_batch_backend_for`

**Attributes:**

- `MustUse { reason: None }`

The [`ComputeBackend`] [`jacobian_batch`] would actually use for `n`
independent Jacobian problems — without running anything.

# Arguments

- `requested` — the backend a caller would pass to [`jacobian_batch`].
- `n` — number of independent Jacobian problems (lanes), dimensionless.

# Returns

Either [`ComputeBackend::Serial`] or [`ComputeBackend::CpuMulti`].

# Example

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::math::differentiate::jacobian_batch_backend_for;

assert_eq!(
    jacobian_batch_backend_for(ComputeBackend::CpuMulti, 2),
    ComputeBackend::Serial
);
```

```rust
pub fn jacobian_batch_backend_for(requested: crate::compute::ComputeBackend, n: usize) -> crate::compute::ComputeBackend { /* ... */ }
```

#### Function `jacobian_column_backend_for`

**Attributes:**

- `MustUse { reason: None }`

The [`ComputeBackend`] [`jacobian`] would actually use to spread the columns
of **one** `dimension`-dimensional Jacobian — without running anything.

This is the *other* axis of parallelism in this module: [`jacobian_batch`]
spreads independent problems across threads, while [`jacobian`] spreads the
`n` independent column evaluations of a single problem.

# Arguments

- `requested` — the backend a caller would pass to [`jacobian`].
- `dimension` — the length of the point `x`, i.e. the number of Jacobian
  columns. Dimensionless.

# Returns

Either [`ComputeBackend::Serial`] or [`ComputeBackend::CpuMulti`].

# Example

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::math::differentiate::jacobian_column_backend_for;

// A 3-equation ODE Jacobian is never worth threading.
assert_eq!(
    jacobian_column_backend_for(ComputeBackend::CpuMulti, 3),
    ComputeBackend::Serial
);
```

```rust
pub fn jacobian_column_backend_for(requested: crate::compute::ComputeBackend, dimension: usize) -> crate::compute::ComputeBackend { /* ... */ }
```

#### Function `derivative`

**Attributes:**

- `MustUse { reason: None }`

Differentiate one scalar function at one point.

The single-lane form of [`derivative_batch`], for callers with one
derivative to take. It runs on the calling thread — there is nothing to
spread — and calls the *same* per-lane kernel, so it agrees with a
one-element batch bit for bit.

# The realised-step correction

`x + h` is generally not representable, so the value the machine actually
evaluates at differs from `x + h` in the last bits and the true step is not
`h`. This kernel therefore evaluates at `xp = x + h` and divides by
`xp - x`, which **is** exact, rather than by `h`. The device is from
*Numerical Recipes* (Press et al., 3rd ed., section 5.7) and it removes an
error source that would otherwise be comparable to the round-off term the
step rule is trying to balance. [`DerivativeSolution::realised_step`]
reports the corrected denominator.

# Arguments

- `x` — the point, in the caller's own units.
- `settings` — scheme and step-size policy; see [`DiffSettings`].
- `f` — the function. Units of the return value are the caller's.

# Returns

A [`DerivativeSolution`] whose [`derivative`](DerivativeSolution::derivative)
is `Some` only if every evaluation was finite and the realised step was
non-zero. **That is the whole of the guarantee** — read the next section
before relying on it.

# What is detected, and what cannot be

The status is computed from exactly one predicate — every sampled value and
the resulting quotient are finite, and the realised step is non-zero — and
[`DerivativeSolution::derivative`] returns `Some` on exactly that same
predicate. They cannot disagree.

**Detected:** a non-finite sample ([`DiffStatus::NotFinite`]), a non-finite
point ([`DiffStatus::InvalidPoint`]), and a step that rounds away so
`x + h == x` ([`DiffStatus::DegenerateStep`]).

**Not detected, and not detectable by any finite-difference kernel:**

- **A singularity the stencil steps over.** `1/x` at `x = 0` is sampled by
  the central stencil at `+h` and `-h`, both perfectly finite, so it returns
  `1/h^2` with [`DiffStatus::Ok`]. The kernel never evaluates at the pole and
  has no way to learn it is there. A one-sided scheme *does* see this
  particular case, because it evaluates at `x` itself — but it has the
  mirror-image blind spot on the other side.
- **Cancellation that leaves a finite number with no correct digits.** The
  quotient is a perfectly ordinary `f64`; nothing about it says how many of
  its bits survived. This is what the step-size rule exists to bound, and why
  the module documents an *accuracy floor* rather than a guarantee.
- **A function that is not differentiable at `x`.** `|x|` at `0` returns `0`
  from the central stencil, confidently.

If the function may be singular or kinked, bracket it away from the trouble
or check the result against a second scheme; the status field will not do it
for you.

# Example

```rust
use outram_foam_basic_lib::math::differentiate::{
    derivative, DiffSettings, DiffStatus,
};

// d/dx sin(x) at x = 1 is cos(1).
let s = derivative(1.0, DiffSettings::central(), |x: f64| x.sin());
let d = s.derivative().expect("finite everywhere");
assert!((d - 1.0_f64.cos()).abs() < 1e-10, "got {d}");

// A sample that comes back non-finite IS reported: the central stencil for
// sqrt at x = 0 evaluates at -h, which is NaN.
let bad = derivative(0.0, DiffSettings::central(), |x: f64| x.sqrt());
assert_eq!(bad.status(), DiffStatus::NotFinite);
assert!(bad.derivative().is_none());

// But a pole the stencil STEPS OVER is not, and cannot be -- see
// "What is detected, and what cannot be" above.
let undetected = derivative(0.0, DiffSettings::central(), |x: f64| 1.0 / x);
assert_eq!(undetected.status(), DiffStatus::Ok);
assert!(undetected.derivative().is_some());
```

```rust
pub fn derivative<F>(x: f64, settings: DiffSettings, f: F) -> DerivativeSolution
where
    F: Fn(f64) -> f64 { /* ... */ }
```

#### Function `derivative_batch`

**Attributes:**

- `MustUse { reason: None }`

Differentiate `N` independent scalar functions at `N` points, on the chosen
backend.

This is the batched, GPU-shaped form: lane `i` differentiates `f(i, .)` at
`points[i]`, and no arithmetic crosses lanes.

# Arguments

- `points` — one abscissa per lane, in the caller's own units.
- `settings` — scheme and step-size policy, shared by every lane.
- `backend` — requested execution backend. What actually runs is
  [`derivative_backend_for`] applied to it. **None of the degradations
  changes the answer.**
- `f` — `f(i, x)` is lane `i`'s function evaluated at `x`. It **must be a
  pure deterministic function of its arguments** — see the module-level
  "Determinism" section. The `Sync` bound is present in both feature builds
  so enabling `parallel` never changes a public signature.

# Returns

A [`DerivativeBatch`] with one solution per point, in point order. An empty
`points` slice returns an empty batch and calls `f` zero times.

# Cost

[`DiffScheme::evaluations_per_derivative`] calls to `f` per lane — 2 for the
three second-order-or-lower schemes, 4 for [`DiffScheme::Central4th`].

# Example

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::math::differentiate::{
    derivative_batch, DiffSettings,
};

// Lane i differentiates x^(i+1) at x = 2; the answer is (i+1) * 2^i.
let points = vec![2.0_f64; 4];
let batch = derivative_batch(
    &points,
    DiffSettings::central(),
    ComputeBackend::CpuMulti,
    |i, x: f64| x.powi(i as i32 + 1),
);

let d = batch.values().expect("all lanes finite");
for (i, got) in d.iter().enumerate() {
    let want = (i as f64 + 1.0) * 2.0_f64.powi(i as i32);
    assert!((got - want).abs() < 1e-6 * want.abs().max(1.0), "lane {i}: {got} vs {want}");
}
```

```rust
pub fn derivative_batch<F>(points: &[f64], settings: DiffSettings, backend: crate::compute::ComputeBackend, f: F) -> DerivativeBatch
where
    F: Fn(usize, f64) -> f64 + Sync { /* ... */ }
```

#### Function `jacobian`

**Attributes:**

- `MustUse { reason: None }`

Assemble the Jacobian `J[i][j] = d f_i / d x_j` of one `n`-dimensional
vector function at one point, by finite differences.

The direct feeder for multi-dimensional Newton and — through
[`NumericalJacobian`] — for [`crate::ode::Rosenbrock23`].

# Square only

`f` must return exactly `x.len()` components. A rectangular Jacobian is
rejected with [`DiffStatus::DimensionMismatch`] rather than silently padded.
This restriction is deliberate and matches the prior art: the consumer
(`n` ODE equations in `n` states) is square, the crate's [`SquareMatrix`] is
square, and `outram-park-fork-dwsim-libs`' `finite_difference_jacobian`
rejects the non-square case too.

# Arguments

- `x` — the point, one component per variable, in the caller's own units.
- `settings` — scheme and step-size policy; see [`DiffSettings`]. The step
  is computed per-column from that column's own `x[j]`, so variables of
  wildly different magnitude each get an appropriate step.
- `backend` — requested backend for spreading the **columns** of this one
  Jacobian. What actually runs is [`jacobian_column_backend_for`] applied to
  it; a small `n` runs serially. **None of the degradations changes the
  answer.**
- `f` — `f(0, x, out)` must fill `out` with the `n` function components at
  `x`. The lane index is always `0` here; it is in the signature so the same
  closure works with [`jacobian_batch`]. It **must be a pure deterministic
  function of its arguments**.

# Returns

A [`JacobianSolution`] whose [`matrix`](JacobianSolution::matrix) is `Some`
only if every column succeeded.

# Cost

[`DiffScheme::evaluations_per_jacobian`] calls to `f`: `n + 1` for
[`DiffScheme::Forward`]/[`DiffScheme::Backward`], `2n` for
[`DiffScheme::Central`], `4n` for [`DiffScheme::Central4th`].

# Example

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::math::differentiate::{jacobian, DiffSettings};

// f(x, y) = [x^2 * y, sin(x) + y^3]
// J = [[2xy, x^2], [cos(x), 3y^2]]
let point = [1.5_f64, 2.0];
let s = jacobian(
    &point,
    DiffSettings::central(),
    ComputeBackend::Serial,
    |_, v: &[f64], out: &mut Vec<f64>| {
        out.push(v[0] * v[0] * v[1]);
        out.push(v[0].sin() + v[1] * v[1] * v[1]);
    },
);

let j = s.matrix().expect("smooth everywhere");
let (x, y) = (point[0], point[1]);
for (got, want) in [
    (j.get(0, 0), 2.0 * x * y),
    (j.get(0, 1), x * x),
    (j.get(1, 0), x.cos()),
    (j.get(1, 1), 3.0 * y * y),
] {
    assert!((got - want).abs() < 1e-8 * want.abs().max(1.0), "{got} vs {want}");
}
```

```rust
pub fn jacobian<F>(x: &[f64], settings: DiffSettings, backend: crate::compute::ComputeBackend, f: F) -> JacobianSolution
where
    F: Fn(usize, &[f64], &mut Vec<f64>) + Sync { /* ... */ }
```

#### Function `jacobian_batch`

**Attributes:**

- `MustUse { reason: None }`

Assemble `N` independent Jacobians, one per lane, on the chosen backend.

This is the batched form: the parallel axis is the **lane**, not the column,
so it is the right entry point when there are many small Jacobians (a
per-cell chemistry Jacobian over a mesh, an ensemble of ODE systems). Use
[`jacobian`] when there is one large Jacobian instead.

# The flat point layout

`points` is a **flat, row-major** buffer of `lanes * n` values: lane `i`'s
point is `points[i * n .. (i + 1) * n]`. A `&[Vec<f64>]` would be the
obvious alternative and is rejected on purpose — it costs one allocation and
one pointer chase per lane, and it is not the layout a GPU buffer would ever
take. `points.len()` must be an exact multiple of `n`.

# Arguments

- `points` — flat `lanes * n` buffer as above, in the caller's own units.
- `n` — the dimension of each point, dimensionless. Must be non-zero.
- `settings` — scheme and step-size policy, shared by every lane.
- `backend` — requested backend; see [`jacobian_batch_backend_for`]. Each
  lane's columns are computed serially, since the lane axis is already the
  parallel one.
- `f` — `f(i, x, out)` must fill `out` with lane `i`'s `n` function
  components at `x`. It **must be a pure deterministic function of its
  arguments**.

# Returns

A [`JacobianBatch`] with one solution per lane, in lane order. An empty
`points` slice, or `n == 0`, returns an empty batch and calls `f` zero
times. A `points.len()` that is not a multiple of `n` returns an empty batch
as well — it is a caller bug, not a numerical failure, and there is no
sensible lane count to report per-lane statuses against.

# Cost

`lanes * `[`DiffScheme::evaluations_per_jacobian`]`(n)` calls to `f`.

# Example

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::math::differentiate::{jacobian_batch, DiffSettings};

// 3 lanes of the 2-D rotation-like system f = [-k_i * y, k_i * x],
// whose Jacobian is [[0, -k_i], [k_i, 0]].
let k = [1.0_f64, 2.5, 7.0];
let points: Vec<f64> = vec![0.3, -0.7, 0.3, -0.7, 0.3, -0.7];

let batch = jacobian_batch(
    &points,
    2,
    DiffSettings::central(),
    ComputeBackend::CpuMulti,
    |i, v: &[f64], out: &mut Vec<f64>| {
        out.push(-k[i] * v[1]);
        out.push(k[i] * v[0]);
    },
);

let mats = batch.matrices().expect("linear system, exact everywhere");
for (i, m) in mats.iter().enumerate() {
    assert!((m.get(0, 1) + k[i]).abs() < 1e-9);
    assert!((m.get(1, 0) - k[i]).abs() < 1e-9);
}
```

```rust
pub fn jacobian_batch<F>(points: &[f64], n: usize, settings: DiffSettings, backend: crate::compute::ComputeBackend, f: F) -> JacobianBatch
where
    F: Fn(usize, &[f64], &mut Vec<f64>) + Sync { /* ... */ }
```

#### Function `ode_system_jacobian`

Fill an [`OdeSystem`]'s Jacobian slots by finite differences.

This is the free-function form of what [`NumericalJacobian`] does, for
callers who already have an `OdeSystem` and want the numbers rather than a
wrapper. It fills exactly the two buffers
[`OdeSystem::jacobian`] is contracted to fill:

- `dfdy[i][j] = d f_i / d y_j`, an `n x n` [`SquareMatrix`];
- `dfdx[i] = d f_i / d x`, the derivative with respect to the **independent
  variable** (time, for a transient), length `n`.

Both are resized to `n = system.n_eqns()` if the caller's buffers are the
wrong size, so it is safe to pass freshly-defaulted ones.

# Failure is written into the buffers, not swallowed

On failure the offending entries are filled with `NaN` **and** the reason is
returned. Nothing is quietly zeroed. This matters: filling a failed entry
with `0.0` — which
`outram-park-fork-dwsim-libs`' `finite_difference_jacobian` does — turns an
undetectable-at-the-call-site failure into a plausible-looking Jacobian, and
a Rosenbrock step built on it produces a wrong trajectory rather than an
error. With `NaN` the failure propagates into the step, the normalised error
estimate becomes `NaN`, the step controller shrinks `dx` and
[`crate::ode::OdeError::StepSizeUnderflow`] is reported. Loud is better.

# Arguments

- `system` — the ODE system whose [`OdeSystem::derivatives`] is sampled.
- `x` — the independent variable, caller's units.
- `y` — the state, length `system.n_eqns()`.
- `settings` — scheme and step-size policy; see [`DiffSettings`].
- `dfdx`, `dfdy` — output buffers, filled in place.

# Returns

[`DiffStatus::Ok`] if every entry of both outputs was formed from finite
evaluations; the first failing status otherwise.

# Cost

`1 + `[`DiffScheme::evaluations_per_jacobian`]`(n)` calls to
[`OdeSystem::derivatives`] for the one-sided schemes — the base evaluation
is shared between `dfdy`'s columns and `dfdx` — and
[`DiffScheme::evaluations_per_jacobian`]`(n) + 2` or `+ 4` for the symmetric
ones.

# Example

```rust
use outram_foam_basic_lib::math::differentiate::{
    ode_system_jacobian, DiffSettings, DiffStatus,
};
use outram_foam_basic_lib::matrix::SquareMatrix;
use outram_foam_basic_lib::ode::OdeSystem;

// dy/dx = [-2*y0 + y1, x * y0] -- Jacobian [[-2, 1], [x, 0]], dfdx = [0, y0].
struct Linear;
impl OdeSystem for Linear {
    fn n_eqns(&self) -> usize { 2 }
    fn derivatives(&self, x: f64, y: &[f64], dydx: &mut Vec<f64>) {
        dydx.clear();
        dydx.push(-2.0 * y[0] + y[1]);
        dydx.push(x * y[0]);
    }
}

let mut dfdx = Vec::new();
let mut dfdy = SquareMatrix::new(2);
let status = ode_system_jacobian(
    &Linear, 0.5, &[1.0, 2.0], DiffSettings::central(), &mut dfdx, &mut dfdy,
);

assert_eq!(status, DiffStatus::Ok);
assert!((dfdy.get(0, 0) + 2.0).abs() < 1e-8);
assert!((dfdy.get(0, 1) - 1.0).abs() < 1e-8);
assert!((dfdy.get(1, 0) - 0.5).abs() < 1e-8);
assert!(dfdy.get(1, 1).abs() < 1e-8);
assert!(dfdx[0].abs() < 1e-8);
assert!((dfdx[1] - 1.0).abs() < 1e-8); // d/dx (x * y0) = y0 = 1
```

```rust
pub fn ode_system_jacobian<S>(system: &S, x: f64, y: &[f64], settings: DiffSettings, dfdx: &mut Vec<f64>, dfdy: &mut crate::matrix::SquareMatrix) -> DiffStatus
where
    S: OdeSystem + ?Sized { /* ... */ }
```

### Constants and Statics

#### Constant `CBRT_EPSILON`

`f64::EPSILON.cbrt()` = `6.0554544523933395e-6` — the relative step that
balances truncation against round-off for a **central** difference.

A central difference has truncation error proportional to `h^2` (the
coefficient is the third derivative over six) and round-off error
proportional to `eps/h`; minimising their sum over `h` gives
`h ~ eps^(1/3)`. The resulting accuracy is `~ eps^(2/3) = 3.67e-11`, **not**
`eps` — see the module-level "Achievable accuracy" table for the measured
value.

This is the same constant `outram-park-fork-offbeat`'s
`rheology::aster::integration::perturbed_default()` returns, for exactly the
same reason; see the module-level "Provenance" section.

# Units

Dimensionless — it multiplies a length scale in `x`.

```rust
pub const CBRT_EPSILON: f64 = 6.055_454_452_393_339_5e-6;
```

#### Constant `FIFTH_ROOT_EPSILON`

`f64::EPSILON.powf(0.2)` = `7.40095979741405e-4` — the relative step that
balances truncation against round-off for the **fourth-order** Richardson
scheme [`DiffScheme::Central4th`].

Truncation error proportional to `h^4`, round-off proportional to `eps/h`,
so the balance is at `h ~ eps^(1/5)` and the accuracy is
`~ eps^(4/5) = 3.00e-13`.

# Units

Dimensionless.

```rust
pub const FIFTH_ROOT_EPSILON: f64 = 7.400_959_797_414_05e-4;
```

#### Constant `DERIVATIVE_BATCH_MIN_POINTS`

Point count below which a [`ComputeBackend::CpuMulti`] request runs
[`derivative_batch`] on the calling thread instead.

# Measured crossover

*Methodology.* Measured 2026-08-13 on this workspace's development machine,
`std::thread::available_parallelism()` = **4**, release build,
`--features parallel`, `rayon`'s global pool. **The machine was NOT idle:**
1-minute load average was 2.3-3.6 on 4 cores throughout, with a
`bn daemon run` process holding a steady ~37% of one core. Batches of `n`
points spread over seven decades of magnitude, [`DiffSettings::central`],
best of 7 samples per point, wall clock for one whole batch. Produced by the
`#[ignore]`d `differentiate_crossover_benchmark` test and transcribed from
its printed output. `cheap` is a two-flop parabola; `costly` adds an
`ln`/`exp`/`sqrt`/`tanh` chain, standing in for a property evaluation. Two
independent runs are carried side by side rather than averaged, because the
parallel column is far noisier than the serial one.

| Points | cheap serial | cheap speed-up (A) | (B) | costly serial | costly speed-up (A) | (B) |
|---|---|---|---|---|---|---|
| 16 | 0.18 us | 0.03x | 0.01x | 1.15 us | 0.14x | 0.04x |
| 32 | 0.29 us | 0.03x | 0.01x | 2.11 us | 0.20x | 0.07x |
| 64 | 0.51 us | 0.06x | 0.02x | 4.09 us | 0.39x | 0.13x |
| 128 | 0.97 us | 0.10x | 0.04x | 8.07 us | 0.47x | 0.23x |
| 256 | 1.87 us | 0.17x | 0.06x | 16.05 us | 0.71x | 0.40x |
| 512 | 3.70 us | 0.27x | 0.11x | 31.85 us | 0.74x | 0.67x |
| 1 024 | 7.33 us | 0.38x | 0.19x | 63.41 us | 0.85x | 1.12x |
| 4 096 | 28.86 us | 0.56x | 0.52x | 253.71 us | 1.52x | 2.35x |
| 16 384 | 115.53 us | 0.99x | 1.34x | 1025.92 us | 1.82x | 2.50x |
| 65 536 | 484.26 us | **1.34x** | **1.35x** | 4224.20 us | 1.90x | 2.74x |

*Result.* **65 536** is the smallest size at which the cheap objective won in
*both* runs, and it is the value this constant takes. That is 16x above the
crate-wide [`crate::compute::CPU_MULTI_MIN_WORK_ITEMS`] placeholder and 256x
above [`crate::math::minimise::MINIMISE_BATCH_MIN_PROBLEMS`], and the reason
is structural rather than accidental: a scalar finite difference is **two to
four evaluations of the caller's function and one division**, so with a cheap
function the kernel is memory-bandwidth bound in exactly the way
[`crate::fields::parallel`] is — and it lands within a factor of two of that
module's independently measured 131 072. A batched root find, by contrast,
runs *tens* of iterations per lane and crosses over at 256.

**The caller's function cost moves this by more than an order of magnitude.**
The costly objective first wins at 1 024-4 096, sixteen to sixty-four times
lower. A caller who knows its function is expensive should name
[`ComputeBackend::CpuMulti`] explicitly rather than trust this number.

# Limitations

One machine, four logical cores, under load, one objective family. Not
measured on Android/Termux hardware and not on a many-core server. The
absolute timings should be read as ratios only.

# Units

A count of independent points, dimensionless.

```rust
pub const DERIVATIVE_BATCH_MIN_POINTS: usize = 65_536;
```

#### Constant `JACOBIAN_BATCH_MIN_PROBLEMS`

Lane count below which a [`ComputeBackend::CpuMulti`] request runs
[`jacobian_batch`] on the calling thread instead.

# Measured crossover

*Methodology.* Same machine, date, build and load as
[`DERIVATIVE_BATCH_MIN_POINTS`] — 4 logical cores, load average 2.3-3.6,
**not idle**. `n = 4` Jacobians, [`DiffSettings::central`] so each lane costs
8 evaluations of a 4-component function, best of 7, two independent runs.
Produced by the same `#[ignore]`d `differentiate_crossover_benchmark` test.

| Lanes | cheap serial | cheap speed-up (A) | (B) | costly speed-up (A) | (B) |
|---|---|---|---|---|---|
| 4 | 1.56 us | 0.19x | 0.16x | 0.21x | 0.21x |
| 8 | 3.05 us | 0.10x | 0.12x | 0.63x | 0.35x |
| 16 | 6.05 us | 0.19x | 0.21x | 0.60x | 0.65x |
| 32 | 12.06 us | 0.83x | 0.33x | 1.52x | 1.03x |
| 64 | 23.64 us | 0.57x | 0.70x | 1.18x | 2.18x |
| 128 | 48.36 us | 0.84x | 1.34x | 1.86x | 2.46x |
| 256 | 95.16 us | **1.09x** | **1.11x** | 1.88x | 2.61x |
| 1 024 | 373.05 us | 1.63x | 2.18x | 1.86x | 1.50x |
| 4 096 | 1590.74 us | 1.75x | 1.90x | 1.85x | 3.10x |
| 16 384 | 6702.36 us | 1.83x | 2.98x | 2.06x | 2.88x |

*Result.* **256** — the smallest lane count that won in both runs. It lands
on exactly the same value as
[`crate::math::minimise::MINIMISE_BATCH_MIN_PROBLEMS`] and
[`crate::math::parallel::ROOT_BATCH_MIN_PROBLEMS`], and 256x *below* this
module's own [`DERIVATIVE_BATCH_MIN_POINTS`]. The two numbers in one module
disagreeing by 256x is the clearest evidence yet for the point
[`crate::compute::CPU_MULTI_MIN_WORK_ITEMS`] makes: the crossover tracks
**work per lane**, not the algorithm. A Jacobian lane here does 8 function
evaluations, four vector allocations and a matrix assembly; a scalar
derivative lane does 2 evaluations and a division.

# A performance trap found by this measurement, recorded so it is not reintroduced

The first version of the central-difference column copied the point **twice**
per column (`x.to_vec()` for the `+h` probe and again for the `-h` probe).
With that version the parallel path **never won at any size measured**,
topping out at 0.96x on 16 384 lanes — allocation traffic, not arithmetic,
was the whole cost. Reusing a single probe buffer made the serial path ~1.4x
faster *and* restored parallel scaling to 1.8-3.0x. If a future change
reintroduces a per-column allocation, this crossover is the measurement that
will notice.

# Limitations

As for [`DERIVATIVE_BATCH_MIN_POINTS`], plus: measured at `n = 4` only. A
larger `n` raises the per-lane cost and should lower this crossover, but that
has not been measured.

# Units

A count of independent Jacobian problems, dimensionless.

```rust
pub const JACOBIAN_BATCH_MIN_PROBLEMS: usize = 256;
```

#### Constant `JACOBIAN_COLUMN_MIN_DIMENSION`

Dimension below which [`jacobian`] computes one Jacobian's columns on the
calling thread rather than spreading them across `rayon`.

This is the **other** parallel axis, and it is the one bead `op-yvj.4.6`
names: an `n`-dimensional Jacobian's `n + 1` (or `2n`, or `4n`) evaluations
are all independent.

# Measured crossover

*Methodology.* Same machine, date, build and load as
[`DERIVATIVE_BATCH_MIN_POINTS`] — 4 logical cores, load average 2.3-2.9,
**not idle**. One Jacobian of dimension `n`, [`DiffSettings::central`] so
`2n` evaluations, of a residual whose every component sums over all `n`
inputs — so one evaluation is `O(n^2)` and the whole Jacobian is `O(n^3)`,
the shape a genuinely coupled residual has. Best of 7, two independent runs.
Produced by the `#[ignore]`d `jacobian_column_crossover_benchmark` test.

| Dimension | `f` evals | serial | speed-up (A) | (B) |
|---|---|---|---|---|
| 4 | 8 | 0.72 us | 0.09x | 0.09x |
| 8 | 16 | 3.03 us | 0.35x | 0.16x |
| 16 | 32 | 15.93 us | 0.41x | 0.46x |
| 32 | 64 | 103.63 us | **1.49x** | **1.53x** |
| 64 | 128 | 767.15 us | 2.67x | 2.58x |
| 128 | 256 | 6117.53 us | 1.98x | 2.55x |
| 256 | 512 | 48189.43 us | 4.04x | 2.84x |
| 512 | 1024 | 389479.48 us | 3.97x | 3.55x |

*Result.* **32** — the lowest dimension that won in both runs, and the
smallest crossover anywhere in this crate. That is not surprising once the
cost is written down: at `n = 32` a single Jacobian is already 64 evaluations
of an `O(n^2)` residual, which is far more work per dispatch than 32 lanes of
anything else in the crate.

**This crossover is even more caller-dependent than the others**, because it
scales with the residual's own cost in `n`. A residual that is `O(1)` per
component rather than `O(n)` will cross over much later. 32 is set for the
coupled case; a caller with a cheap decoupled residual should pass
[`ComputeBackend::Serial`] explicitly.

# Why an ODE Jacobian does not use this

[`ode_system_jacobian`] and [`NumericalJacobian`] always run their columns
serially, whatever the dimension. An ODE system's `n` is its equation count —
typically single or double digits, well under this threshold — and the
parallel axis that matters for ODE work is the *ensemble lane*, which
[`crate::ode::parallel::integrate_ensemble`] already provides. Nesting a
`rayon` map inside that one would only contend for the same pool.

# Limitations

As for [`DERIVATIVE_BATCH_MIN_POINTS`], plus: one residual shape (`O(n)` per
component). The 128 row losing ground to both its neighbours in run A
(1.98x against 2.67x and 4.04x) is measurement noise on a loaded machine, not
a real effect — do not read this table as a scaling study.

# Units

A count of Jacobian columns, i.e. the length of the point. Dimensionless.

```rust
pub const JACOBIAN_COLUMN_MIN_DIMENSION: usize = 32;
```

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

## Module `minimise`

Batched derivative-free 1-D **golden-section** extremum search on the hybrid
execution backend — contract `N` independent brackets at once, serially or
across CPU cores.

# Provenance — this is a generalisation, not a new algorithm

The algorithm, its contraction constant, its bracket-width stopping rule and
its literature citation are taken from the **working, in-production**
golden-section search already in this workspace:

```text
crates/tampines-steam-tables/src/steam_turbine_equations/
    converging_diverging_nozzles/choked_flow/
    stagnation_point_outside_vle_ph_dome_multiphase.rs:67
        fn golden_section_max_g(...)
```

That function locates the homogeneous-equilibrium choked-flow energy-balance
maximum `G(p) = rho(p,s0) * sqrt(2*(h0 - h(p,s0)))` along an isentrope and is
exercised by that crate's Marviken critical-flow tests. It is cited to:

> Price, C. J., & Robertson, B. L. (2012). *Golden Section Search.* In
> Encyclopedia of Engineering Optimization and Heuristics (pp. 1-4).
> Singapore: Springer Nature Singapore.

This module carries the same citation, because it is the same method. What is
added here is batching across [`ComputeBackend`], per-lane status reporting,
absolute+relative tolerances in place of a hard-coded 1 Pa bracket width,
non-finite handling, and the [`Sense`] switch so one kernel serves both
minimisation and the maximisation the steam-tables caller actually wants.

**One deliberate algorithmic deviation from the original, and it is a
speed-up, not a behaviour change in exact arithmetic:** the original
re-evaluates *both* interior probes on every iteration. Golden section exists
precisely so that one probe of the contracted bracket coincides with a probe
already evaluated, costing **one** objective evaluation per iteration rather
than two — the original's own comment claims that property while its code
does not implement it. This module reuses the retained probe. Consequences:

- Roughly **half** the objective evaluations for the same bracket sequence,
  which matters when the objective is an IF97 flash.
- The retained probe is *reused* rather than *recomputed*, and
  `a + gr*(b - a)` recomputed is not bit-identical to the value it replaces.
  So this module's iterates can differ from the original's in the last bits.
  The bracket sequence, the convergence behaviour and the located extremum to
  within the requested tolerance are unchanged.
- **A measured, quantified cost, recorded here rather than discovered later.**
  The retained probe carries an absolute rounding error fixed at the scale of
  the *older, wider* bracket, while the bracket itself keeps shrinking — so
  the width's *relative* deviation from the closed form `W0 * gr^k` grows
  roughly geometrically with iteration count. Measured by
  `bracket_contracts_at_the_golden_ratio` (release, 2026-08-13, `W0 = 10`):
  `0` at `k = 1`, `2.675637e-14` at `k = 20`, `2.251199e-11` at `k = 40`,
  `2.334157e-6` at `k = 80`. Recomputing both probes — the original's form —
  holds the deviation at `<= 6.7e-16` for every one of those `k`.

  **CORRECTED 2026-08-13 — the original text here understated the effect on
  the located abscissa by about eight orders of magnitude, and the error is
  instructive.** It read: "at `k = 80` the bracket width is `1.9e-16`, so a
  `2.3e-6` relative deviation is `4e-22` absolute". That arithmetic is right
  about the *width* and wrong about the *answer*. The two are not the same
  quantity: a last-bit difference in a probe can flip a comparison on a
  near-flat objective, or change which iteration first trips the stopping
  test, and the returned midpoint then shifts by up to roughly **one bracket
  width at the stopping tolerance** — not by the width at `k = 80`.

  Measured, not reasoned: `tampines-steam-tables` ran both forms side by side
  over 158 real IAPWS-IF97 choked-flow searches and found a worst
  `|Δp_crit|` of **5.990479e-1 Pa** against a 1 Pa stopping rule, and up to
  **3 Pa** on a production case — about `1e8` times the `4e-22` the old text
  implied. The worst relative deviation was `4.604111e-8`, and the objective
  value barely moved at all: worst `|ΔG|/G = 3.429708e-13`.

  **The conclusion survives; only the magnitude was wrong.** Those shifts sat
  four orders inside the consuming test's `0.005` relative tolerance, the
  Marviken and Moody gate outputs were byte-identical, and the objective is
  what a caller actually uses. But "well inside tolerance, measured" is a
  different claim from "`4e-22`, therefore ignorable", and a caller with a
  tighter tolerance than 1 Pa deserves the honest figure. Tracked as
  `op-8kww`.

  Separately and still true: the closed form `W0 * gr^k` stops being an exact
  predictor of [`MinSolution::bracket_width`] at large `k`. If you need the
  width exact rather than the evaluation count halved, recompute both probes.

# Golden section finds a minimum of a UNIMODAL function — read this first

**On a bracket where the objective is not unimodal, golden section returns
*a* local extremum, silently, with `Converged` status and no warning of any
kind.** It cannot do better: it only ever compares two interior probes, so a
second basin outside the surviving sub-bracket is discarded on the first
iteration and can never be recovered. There is no test this module could run
to detect the situation without evaluating the objective densely, which would
defeat the point of using golden section at all.

This is the single easiest way to get a confident wrong answer out of this
module. If you do not *know* your objective is unimodal on the bracket —
from the physics, not from a plot of one case — then coarse-scan first and
hand golden section a bracket around the basin you want. That is exactly what
the steam-tables `dome_crossing_interior_choke` caller does: a 1500-point
scan finds the right basin, and golden section only refines it.

# Convergence is on the BRACKET, never on the value

[`MinSettings`] deliberately has **no `f_tol` field**, unlike its sibling
[`crate::math::parallel::RootSettings`]. A value-based criterion such as
`|f(b) - f(a)| <= f_tol` is actively wrong for minimisation, and the reason is
the defining property of a minimum rather than an implementation detail: near
a smooth minimum `x*`,

```text
f(x* + d) - f(x*) ~= 0.5 * f''(x*) * d^2
```

so the *values* agree to second order long before the *arguments* do. A
function-value test therefore reports success while the abscissa is still
wrong in its most significant digits. This module tests the bracket width and
nothing else.

# Achievable accuracy is about `sqrt(eps)`, not `eps`

The same quadratic flatness bounds how well *any* derivative-free method can
locate a smooth minimum. Two probes a distance `d` apart around `x*` differ in
value by `~0.5*f''*d^2`; once that difference falls below the rounding noise
in the *evaluated* `f`, which is `~eps*|f(x*)|`, the comparison that drives
the contraction is deciding on noise. Equating the two gives

```text
|x_located - x*|  ~  sqrt( 2 * eps * |f(x*)| / |f''(x*)| )
```

and with `f` and `f''` both order unity that is the classical floor
`sqrt(eps) ~ 1.5e-8` — see [`SQRT_EPSILON`]. This surprises people who expect
the `1e-15`-ish accuracy a root finder gets on the same machine, and it is why
[`MinSettings::default`] sets `x_tol_rel` to [`SQRT_EPSILON`] rather than the
`1e-12` [`crate::math::parallel::RootSettings`] uses. Asking for a tighter
tolerance is not rejected — it simply spends ~30 more iterations narrowing a
bracket whose midpoint is no more accurate.

**Read the formula, not just the headline `sqrt(eps)`.** The floor is set by
the objective's *relative precision near its own extremum*, not by golden
section. Two consequences, both measured by
`flat_minimum_exposes_the_sqrt_eps_limit` in `minimise/tests.rs` on 64 lanes
with the tolerance driven below every arithmetic floor (release, 2026-08-13):

| Objective | Predicted floor | Measured worst `\|x - x0\|` |
|---|---|---|
| `1 + (x - x0)^2` — order-unity value | `sqrt(eps) = 1.490116e-8` | **1.053671e-8** |
| `1 + (x - x0)^4` — flatter than quadratic | `eps^(1/4) = 1.220703e-4` | **1.026485e-4** |
| `(x - x0)^2` — value is exactly `0` | *none of the above* | **8.881784e-16** |

The first two rows land just under their predicted floors, so the theory is a
usable bound rather than a story. The **flat objective is 9 742x worse** than
the quadratic one on identical lanes, brackets and settings — that is the
penalty for a minimum that is flatter than quadratic, and every one of those
lanes still reported `Converged` with a bracket width that looked tight.

The third row is the one that stops `sqrt(eps)` being memorised as a universal
constant: an objective whose minimum value is exactly zero, evaluated so that
its relative precision survives (`d*d` is exact to a half-ULP in `d`), has no
order-unity baseline to be swamped by and reaches **8.9e-16**, seven orders
better than the "floor". A physical objective — a mass flux, a residual with a
floor, an enthalpy — is the first row, not the third.

# Hybrid means dispatch, not two APIs

[`golden_section_batch`] takes a [`ComputeBackend`] parameter. There is no
`golden_section_batch_parallel()` sibling. With the `parallel` feature off,
[`ComputeBackend::CpuMulti`] resolves down to [`ComputeBackend::Serial`]
through [`ComputeBackend::resolve`] and the answer is unchanged — bit for
bit, not merely close. There is no `Gpu` kernel here yet, so a `Gpu` request
degrades to the best available CPU path.

# Determinism — bitwise identical across backends and thread counts

**This module returns bit-for-bit identical output on
[`ComputeBackend::Serial`] and [`ComputeBackend::CpuMulti`], at any thread
count, on every run**, provided the caller's objective is a deterministic pure
function of `(index, x)`.

The argument is the same one [`crate::math::parallel`] makes and it is
stronger here than for a reduction: lane `i`'s answer is a pure function of
lane `i`'s bracket and objective evaluations, and **no arithmetic crosses
lanes at all**. A parallel sum has to re-associate `a + b + c` and
floating-point addition is not associative; a batch of independent
contractions has nothing to re-associate. Both backends call the same
`#[inline]` per-lane kernel; only the identity of the calling thread differs.

Verified by the `bitwise_*` tests in `minimise/tests.rs`, which compare serial
against `rayon` pools of 1, 2, 4 and 8 workers on 2 048 lanes deliberately
built with wildly varying per-lane iteration counts. **Measured 2026-08-13
(release, `--features parallel`, 4 logical cores): bit-identical on every
observable field of every lane, at all four thread counts, for both
[`Sense`] variants.**

The `#[ignore]`d `minimise_batch_thread_scaling_benchmark` re-asserts the same
identity while timing it, on 65 536 imbalanced lanes, best of 7, with a second
independent run alongside:

| Worker threads | Time | Speed-up | (repeat) | Bitwise vs serial |
|---|---|---|---|---|
| *serial reference* | 17883.35 us | 1.00x | 1.00x | — |
| 1 | 16192.92 us | 1.10x | 1.07x | identical |
| 2 | 10206.05 us | 1.75x | 1.52x | identical |
| 4 | 11428.69 us | 1.56x | 1.54x | identical |
| 8 | 5708.41 us | 3.13x | 2.31x | identical |

The "identical" column is the determinism claim measured rather than argued,
and it is asserted by the benchmark itself, not merely printed. **The timings
are noisy and should not be read as a scaling study** — the 4-thread row being
slower than the 2-thread row in run 1 is not a real effect, and the machine
was not idle (a concurrent build was running in another checkout). One
machine, four logical cores, one batch, two runs; nothing measured on Android
hardware or on a many-core server.

The one way a caller can break this is to supply an objective that is not a
pure function — one that reads a random number generator, accumulates into
shared interior-mutable state, or depends on the calling thread. The `Sync`
bound permits it; this contract forbids it.

# Non-convergence is reported, never swallowed

- Every lane carries a [`MinStatus`], reachable through [`MinBatch::solutions`].
- [`MinSolution::extremum`] returns `Option<f64>` — `Some(x)` **only** when
  that lane converged. The diagnostic iterate is behind the
  deliberately-named [`MinSolution::last_iterate`].
- [`MinBatch::extrema`] is the all-or-nothing path: it returns
  `Err(`[`MinBatchFailure`]`)` naming the failure count and the first failing
  lane, rather than a plausible-looking `Vec<f64>`.

**A non-converged lane is never handed back as a bracket endpoint dressed up
as a minimum.** A lane that ran out of iterations reports its current bracket
midpoint with [`MinStatus::MaxIterations`], which [`MinSolution::extremum`]
still refuses to return; a lane with a non-finite bracket end reports
[`MinStatus::InvalidBracket`] and a `NaN` iterate, because there is no honest
number to give.

# What is deliberately NOT here: a Brent parabolic-interpolation minimiser

Bead `op-yvj.4.3` offers one as an optional CPU-only faster path. It is not
implemented here, on purpose, because **this workspace already contains two
of them** and a third would be the exact drift defect the workspace's
"Search before building" rule exists to prevent:

| Existing implementation | Provenance |
|---|---|
| `outram-park-fork-dwsim-libs::petroleum::fitting::brent_minimize` | independent implementation from Brent (1973) ch. 5, standing in for DWSIM's `MathEx.BrentOpt.BrentMinimize` |
| `outram-park-fork-dwsim-libs::columns::shortcut::brent_minimize` (private) | faithful port of DWSIM `BrentMinimize.vb:176-295` |

Both are `f64`-scalar, single-problem, and neither is batched — so there is a
real case for a batched Brent minimiser eventually living *here*, since
`outram-foam-basic-lib` sits below `dwsim-libs` and the dependency could only
ever flow this way. That is a maintainer decision about which of three
implementations becomes canonical, not something to settle by quietly adding
a fourth. Brent is also explicitly excluded from the GPU by the bead: its
branching destroys the lockstep property that makes the batched golden
section worth dispatching in the first place.

# Units

Everything here is dimensionless `f64`, for the same reason
[`crate::math::parallel`] is: a generic minimiser has no single physical
dimension. One lane's abscissa may be a pressure in pascals and another's a
blend factor, and the objective's dimension is whatever the caller returns.

`uom` typing is **not stripped** to get here — it is applied at the boundary,
by the caller, exactly as the hybrid-backend epic requires: convert into the
batch, convert back out. The doctest on [`golden_section_batch`] shows that
boundary explicitly, maximising a `uom`-typed mass flux over a `uom`-typed
pressure bracket in the shape of the steam-tables choked-flow caller.

# Cargo features and portability

The `rayon` paths sit behind the crate's `parallel` feature, which is **off by
default**; with it off this module still compiles and every entry point still
works. `rayon` is pure Rust with no system component, so everything here
compiles and runs on `aarch64-linux-android` / Termux exactly as on desktop.
Nothing in this module is target-gated.

# Example

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::math::minimise::{
    golden_section_batch, MinProblem, MinSettings, Sense,
};

// 4 independent parabolas (x - x0)^2, each bracketed on [-5, 5].
let vertices = [-2.0, -0.5, 1.25, 3.0];
let problems: Vec<MinProblem> = (0..4).map(|_| MinProblem::new(-5.0, 5.0)).collect();

let batch = golden_section_batch(
    &problems,
    Sense::Minimise,
    MinSettings::default(),
    ComputeBackend::CpuMulti,
    |i, x| (x - vertices[i]) * (x - vertices[i]),
);

let located = batch.extrema().expect("all four lanes converge");
for (got, want) in located.iter().zip(vertices) {
    // sqrt(eps)-scale accuracy is the floor for a smooth minimum.
    assert!((got - want).abs() < 1e-7, "got {got}, want {want}");
}

// Asking for multi-CPU gives a bit-for-bit identical answer, whether or not
// the `parallel` feature is compiled in.
let serial = golden_section_batch(
    &problems,
    Sense::Minimise,
    MinSettings::default(),
    ComputeBackend::Serial,
    |i, x| (x - vertices[i]) * (x - vertices[i]),
);
assert_eq!(located, serial.extrema().unwrap());
```

```rust
pub mod minimise { /* ... */ }
```

### Types

#### Enum `Sense`

Whether [`golden_section_batch`] should locate a **minimum** or a **maximum**.

Golden section is indifferent: it compares two interior probes and keeps the
sub-bracket containing the better one, and "better" is the only thing this
switch changes. Nothing is negated internally, so the values a solution
reports are the caller's own objective values with their own sign.

[`Maximise`](Self::Maximise) exists because the workspace's production
golden-section caller — the homogeneous-equilibrium choked-flow solver in
`tampines-steam-tables` — maximises a mass flux. A module that only minimised
would force that caller to negate at the boundary, which is exactly the kind
of avoidable sign bug a `uom`-typed codebase is trying to design out.

# Units

Dimensionless — a mode selector, not a quantity.

```rust
pub enum Sense {
    Minimise,
    Maximise,
}
```

##### Variants

###### `Minimise`

Locate the abscissa where the objective is smallest. The default.

###### `Maximise`

Locate the abscissa where the objective is largest.

##### Implementations

###### Methods

- ```rust
  pub fn is_better(self: Self, candidate: f64, incumbent: f64) -> bool { /* ... */ }
  ```
  Whether `candidate` is the better of two objective values under this sense.

- ```rust
  pub fn label(self: Self) -> &'static str { /* ... */ }
  ```
  A short human-readable label, for log lines and failure reports.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Sense { /* ... */ }
    ```

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
    fn default() -> Sense { /* ... */ }
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
    fn eq(self: &Self, other: &Sense) -> bool { /* ... */ }
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
#### Struct `MinProblem`

One bracketed 1-D extremum problem: an interval believed to contain a single
interior extremum of the objective.

# Fields and valid ranges

- `lo`, `hi` — the bracket. Both must be finite, or the lane reports
  [`MinStatus::InvalidBracket`]. They may be given in either order; the kernel
  normalises, exactly as [`crate::math::parallel::RootProblem`] does. A
  degenerate bracket (`lo == hi`) converges immediately at that point with
  zero iterations, which is truthful: a zero-width bracket has already met any
  non-negative tolerance.

# The precondition golden section cannot check

The objective must be **unimodal** on `[lo, hi]` — one interior extremum, no
second basin. This is a precondition on the caller, not something the kernel
verifies; see the module-level warning. Note that a *monotone* objective on
the bracket is a benign special case: the contraction walks to the appropriate
endpoint and converges there, which is the intended behaviour when the real
extremum lies just outside. It is the *multimodal* case that is dangerous,
because the answer looks exactly like a correct one.

There is no `guess` field, unlike [`crate::math::parallel::RootProblem`].
Golden section is driven entirely by the bracket and has nowhere to put a
starting iterate; a method that could use one (Brent's parabolic
interpolation) is deliberately not implemented here — again, see the module
docs.

# Units

Dimensionless `f64` in whatever units the caller's abscissa carries — pascals
for a choke-pressure search, dimensionless for a blend factor. See the
module-level "Units" section.

# Example

```rust
use outram_foam_basic_lib::math::minimise::MinProblem;

let p = MinProblem::new(0.0, 1.0);
assert_eq!(p.width(), 1.0);

// Either order is accepted; the width is still positive.
let q = MinProblem::new(1.0, 0.0);
assert_eq!(q.width(), 1.0);
```

```rust
pub struct MinProblem {
    pub lo: f64,
    pub hi: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `lo` | `f64` | One end of the bracket. Finite; need not be less than `hi`. |
| `hi` | `f64` | The other end of the bracket. Finite; need not be greater than `lo`. |

##### Implementations

###### Methods

- ```rust
  pub fn new(lo: f64, hi: f64) -> Self { /* ... */ }
  ```
  A problem bracketed on `[lo, hi]`.

- ```rust
  pub fn width(self: &Self) -> f64 { /* ... */ }
  ```
  The bracket width `|hi - lo|`, in the abscissa's units.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> MinProblem { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &MinProblem) -> bool { /* ... */ }
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
#### Struct `MinSettings`

Convergence tolerance on the **bracket width**, and the iteration cap.

A lane is converged when the bracket has shrunk to
`x_tol_abs + x_tol_rel * |x_mid|`, where `x_mid` is the current bracket
midpoint. There is **no function-value tolerance** — see the module-level
"Convergence is on the BRACKET, never on the value" section for why one would
be actively misleading here.

# Fields and valid ranges

- `x_tol_abs` — absolute abscissa tolerance, in the abscissa's own units.
  Must be `>= 0`. Guards the case `x* ≈ 0`, where a purely relative tolerance
  can never be met.
- `x_tol_rel` — relative abscissa tolerance, dimensionless, `>= 0`. Values
  below [`SQRT_EPSILON`] are honoured but buy no accuracy on a smooth
  objective; they only cost iterations.
- `max_iterations` — hard cap. On reaching it a lane reports
  [`MinStatus::MaxIterations`] and its current bracket midpoint, which
  [`MinSolution::extremum`] still refuses to hand out. Must be `>= 1`.

A negative tolerance is not rejected; it simply can never be met, and the lane
will report [`MinStatus::MaxIterations`]. That is a truthful outcome rather
than a silent success.

# Defaults, and why they differ from [`crate::math::parallel::RootSettings`]

`x_tol_abs = 1e-12`, `x_tol_rel = ` [`SQRT_EPSILON`] `= 1.4901161193847656e-8`,
`max_iterations = 200`.

The relative tolerance is ~10 000x looser than the root finder's `1e-12`, and
that is not a lowered standard — it is the accuracy a derivative-free
minimiser can actually deliver on a smooth objective (module docs,
"Achievable accuracy"). Defaulting to a far tighter tolerance would spend
about **30 extra iterations** per lane narrowing a bracket whose midpoint does
not improve — measured 2026-08-13: 40-47 iterations at these defaults against
77 at `x_tol_abs = 1e-15, x_tol_rel = 0` on the same 64-lane batch, for a
worst abscissa error that stays at `1.05e-8` either way.

The iteration cap is 200 rather than 100 because golden section contracts by a
fixed factor of [`GOLDEN_RATIO`] per iteration — it has no superlinear phase
to bail it out. Reaching a relative `1.5e-8` from a bracket `1e14` times wider
than the answer costs `ln(1e-14)/ln(0.618) ≈ 67` iterations; 200 leaves room
for a caller that tightens the tolerance well past the useful floor.

# Example

```rust
use outram_foam_basic_lib::math::minimise::{MinSettings, SQRT_EPSILON};

// Struct-update syntax keeps the defaults you did not mean to change.
let s = MinSettings {
    max_iterations: 500,
    ..MinSettings::default()
};
assert_eq!(s.x_tol_rel, SQRT_EPSILON);
assert_eq!(s.max_iterations, 500);

// The steam-tables choked-flow caller's rule -- a 1 Pa bracket -- expressed
// as an absolute tolerance with the relative part switched off.
let one_pascal = MinSettings { x_tol_abs: 1.0, x_tol_rel: 0.0, max_iterations: 100 };
assert_eq!(one_pascal.bracket_tolerance(5.0e6), 1.0);
```

```rust
pub struct MinSettings {
    pub x_tol_abs: f64,
    pub x_tol_rel: f64,
    pub max_iterations: u32,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x_tol_abs` | `f64` | Absolute bracket-width tolerance, in the abscissa's units. `>= 0`. |
| `x_tol_rel` | `f64` | Relative bracket-width tolerance, dimensionless. `>= 0`. |
| `max_iterations` | `u32` | Maximum iterations per lane before reporting<br>[`MinStatus::MaxIterations`]. `>= 1`. |

##### Implementations

###### Methods

- ```rust
  pub fn bracket_tolerance(self: &Self, x: f64) -> f64 { /* ... */ }
  ```
  The bracket-width tolerance at abscissa `x`: `x_tol_abs + x_tol_rel * |x|`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> MinSettings { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &MinSettings) -> bool { /* ... */ }
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
#### Enum `MinStatus`

How one lane of a batch ended.

Only [`Converged`](Self::Converged) means the lane located an extremum. Every
other variant is a failure a caller must handle; see the module-level
"Non-convergence is reported, never swallowed" section for the accessors that
make it hard to skip.

| Variant | `last_iterate` | Meaning |
|---|---|---|
| [`Converged`](Self::Converged) | the located extremum | the bracket met the tolerance |
| [`MaxIterations`](Self::MaxIterations) | current bracket midpoint — **not** an extremum | ran out of iterations |
| [`InvalidBracket`](Self::InvalidBracket) | `NaN` | a bracket end is not finite |
| [`NotFinite`](Self::NotFinite) | the abscissa where it happened | the objective evaluated to `NaN`/infinity |

There is deliberately **no** analogue of
[`crate::math::parallel::RootStatus::NotBracketed`]. A root finder can check
its precondition — a sign change across the bracket is one comparison — and
refuse when it fails. A minimiser's precondition is unimodality, which cannot
be checked from a finite number of evaluations. Adding a status that is never
returned would imply a guarantee this module does not provide.

# Units

Dimensionless — a status tag.

```rust
pub enum MinStatus {
    Converged,
    MaxIterations,
    InvalidBracket,
    NotFinite,
}
```

##### Variants

###### `Converged`

The bracket shrank to the requested tolerance. Note that this says the
bracket converged, **not** that the objective is unimodal — on a
multimodal bracket a lane converges happily to a local extremum.

###### `MaxIterations`

`max_iterations` was reached with the bracket still wider than the
tolerance. The reported iterate is the current bracket midpoint and is
**not** claimed to be an extremum.

###### `InvalidBracket`

A bracket end was infinite or `NaN`, so there is no interval to contract.

###### `NotFinite`

The objective evaluated to a non-finite number. Iteration stops
immediately, because continuing would propagate the `NaN` into a
plausible-looking answer — and unlike a root finder there is no sign
structure left to recover from.

##### Implementations

###### Methods

- ```rust
  pub fn is_converged(self: Self) -> bool { /* ... */ }
  ```
  Whether this status means the lane located an extremum.

- ```rust
  pub fn label(self: Self) -> &'static str { /* ... */ }
  ```
  A short human-readable label, for log lines and failure reports.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> MinStatus { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &MinStatus) -> bool { /* ... */ }
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
#### Struct `MinSolution`

The outcome of a single lane: its status, its located abscissa, the objective
value there, the final bracket width and the work it took.

The fields are private on purpose. The only way to get a number claimed to be
an extremum is [`Self::extremum`], which returns `Option<f64>` and hands back
`Some` only for a converged lane. The raw iterate is available from
[`Self::last_iterate`], whose name is chosen so that using it as an answer is
a visible decision in the calling code rather than an accident.

**Abscissa versus value.** [`Self::extremum`] is the *argument* `x*` — the
argmin (or argmax). [`Self::extremal_value`] is the *objective value* `f(x*)`
there. Mixing those two up is the classic minimisation bug, so they are named
apart rather than both being called "the minimum".

`Copy`, so it can be read out of a [`MinBatch`] without cloning.

# Units

[`Self::extremum`], [`Self::last_iterate`] and [`Self::bracket_width`] are in
the abscissa's units; [`Self::extremal_value`] and [`Self::last_value`] are in
the objective's units; [`Self::iterations`] is a dimensionless count.

```rust
pub struct MinSolution {
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
  pub fn extremum(self: &Self) -> Option<f64> { /* ... */ }
  ```
  The located extremum's **abscissa** `x*`, if this lane converged.

- ```rust
  pub fn extremal_value(self: &Self) -> Option<f64> { /* ... */ }
  ```
  The objective **value** `f(x*)` at the located extremum, if this lane

- ```rust
  pub fn last_iterate(self: &Self) -> f64 { /* ... */ }
  ```
  The last abscissa the solver held, converged or not.

- ```rust
  pub fn last_value(self: &Self) -> f64 { /* ... */ }
  ```
  The objective value at [`Self::last_iterate`], in the objective's units.

- ```rust
  pub fn bracket_width(self: &Self) -> f64 { /* ... */ }
  ```
  Width of the final bracket, in the abscissa's units.

- ```rust
  pub fn iterations(self: &Self) -> u32 { /* ... */ }
  ```
  Iterations performed on this lane, dimensionless.

- ```rust
  pub fn status(self: &Self) -> MinStatus { /* ... */ }
  ```
  How this lane ended.

- ```rust
  pub fn converged(self: &Self) -> bool { /* ... */ }
  ```
  Whether this lane located an extremum.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> MinSolution { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &MinSolution) -> bool { /* ... */ }
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
#### Struct `MinBatch`

A batch of `N` lane outcomes, in the same order as the problems handed in.

Lane `i` of the result corresponds to `problems[i]`, always — the parallel
path preserves order, so no index bookkeeping is needed.

# Getting numbers out

- [`Self::extrema`] — all-or-nothing abscissae. `Ok(Vec<f64>)` only when every
  lane converged; otherwise `Err(`[`MinBatchFailure`]`)`.
- [`Self::extremal_values`] — the same, for the objective values.
- [`Self::solutions`] — per-lane, when the caller wants to handle failures
  individually (widen the bracket, flag the cell, fall back to a scan).

# Units

See [`MinSolution`].

```rust
pub struct MinBatch {
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
  pub fn solutions(self: &Self) -> &[MinSolution] { /* ... */ }
  ```
  Every lane's outcome, in problem order.

- ```rust
  pub fn into_solutions(self: Self) -> Vec<MinSolution> { /* ... */ }
  ```
  Consume the batch and take the outcomes.

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Number of lanes, dimensionless.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether the batch has no lanes.

- ```rust
  pub fn get(self: &Self, i: usize) -> Option<MinSolution> { /* ... */ }
  ```
  Lane `i`'s outcome, or `None` if `i` is out of range.

- ```rust
  pub fn all_converged(self: &Self) -> bool { /* ... */ }
  ```
  Whether every lane converged. Vacuously `true` for an empty batch.

- ```rust
  pub fn failure_count(self: &Self) -> usize { /* ... */ }
  ```
  How many lanes failed to converge, dimensionless.

- ```rust
  pub fn first_failure(self: &Self) -> Option<(usize, MinSolution)> { /* ... */ }
  ```
  The first failing lane and its outcome, if any.

- ```rust
  pub fn failures(self: &Self) -> Vec<(usize, MinSolution)> { /* ... */ }
  ```
  Every failing lane, as `(index, outcome)` pairs.

- ```rust
  pub fn extrema(self: &Self) -> Result<Vec<f64>, MinBatchFailure> { /* ... */ }
  ```
  The located **abscissae**, or an error naming the failures — the

- ```rust
  pub fn extremal_values(self: &Self) -> Result<Vec<f64>, MinBatchFailure> { /* ... */ }
  ```
  The objective **values** at the located extrema, or an error naming the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> MinBatch { /* ... */ }
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
    fn eq(self: &Self, other: &MinBatch) -> bool { /* ... */ }
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
#### Struct `MinBatchFailure`

**Attributes:**

- `Other("#[error(\"{failure_count} of {total} minimisation problems did not converge; \\\n     first failure at lane {first_index} with status {first_status:?} \\\n     after {first_iterations} iterations\")]")`

One or more lanes of a [`MinBatch`] did not converge.

Returned by [`MinBatch::extrema`] and [`MinBatch::extremal_values`]. It names
both the scale of the problem (how many of how many) and a specific lane to
look at, because "3 of 10 000 lanes failed" is only actionable once you know
*which* lane.

# Units

All counts and indices are dimensionless.

```rust
pub struct MinBatchFailure {
    pub total: usize,
    pub failure_count: usize,
    pub first_index: usize,
    pub first_status: MinStatus,
    pub first_iterations: u32,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `total` | `usize` | Number of lanes in the batch. |
| `failure_count` | `usize` | Number of lanes that did not converge. |
| `first_index` | `usize` | Index of the first non-converged lane. |
| `first_status` | `MinStatus` | Why that lane failed. |
| `first_iterations` | `u32` | Iterations that lane performed before giving up. |

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
    fn clone(self: &Self) -> MinBatchFailure { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **Eq**
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
    fn eq(self: &Self, other: &MinBatchFailure) -> bool { /* ... */ }
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
### Functions

#### Function `minimise_backend_for`

**Attributes:**

- `MustUse { reason: None }`

The [`ComputeBackend`] [`golden_section_batch`] would actually use for `n`
problems if asked for `requested` — without running anything.

Applies exactly the same reduction the kernel does (feature availability, no
GPU kernel here, and the [`MINIMISE_BATCH_MIN_PROBLEMS`] size floor), so what
it reports is what would run. Useful for logging and for benchmark harnesses.

# Arguments

- `requested` — the backend a caller would pass to [`golden_section_batch`].
- `n` — the number of independent problems in the batch, dimensionless.

# Returns

Either [`ComputeBackend::Serial`] or [`ComputeBackend::CpuMulti`]; never
[`ComputeBackend::Gpu`], because no GPU kernel exists here yet.

# Example

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::math::minimise::{
    minimise_backend_for, MINIMISE_BATCH_MIN_PROBLEMS,
};

// Too small to thread, whatever was asked for.
assert_eq!(minimise_backend_for(ComputeBackend::CpuMulti, 8), ComputeBackend::Serial);

// Big enough; the answer now depends only on whether `parallel` is compiled in.
let picked = minimise_backend_for(ComputeBackend::CpuMulti, MINIMISE_BATCH_MIN_PROBLEMS);
assert!(picked.is_available());
```

```rust
pub fn minimise_backend_for(requested: crate::compute::ComputeBackend, n: usize) -> crate::compute::ComputeBackend { /* ... */ }
```

#### Function `golden_section_batch`

**Attributes:**

- `MustUse { reason: None }`

Locate `N` independent 1-D extrema by golden-section search, on the chosen
backend.

This is the entry point for derivative-free bracketed extremum search: an
objective you can evaluate on a bracket you believe contains one interior
extremum, for many independent lanes at once. It is the right tool when a
root find is the wrong one — minimising a residual, picking a limiter blend
factor, or locating a property-inversion objective's turning point where that
objective is not monotone.

# Arguments

- `problems` — one [`MinProblem`] per lane. `problems[i]`'s bracket is used
  with `f(i, ·)`.
- `sense` — [`Sense::Minimise`] or [`Sense::Maximise`]; nothing is negated
  internally, so reported values keep the caller's sign.
- `settings` — bracket tolerance and iteration cap; see [`MinSettings`].
- `backend` — requested execution backend. What actually runs is
  [`minimise_backend_for`] applied to it: an unavailable backend degrades,
  `Gpu` degrades (no GPU kernel here yet), and a batch below
  [`MINIMISE_BATCH_MIN_PROBLEMS`] runs serially. **None of those changes the
  answer.**
- `f` — the objective. `f(i, x)` must return the value of lane `i`'s function
  at abscissa `x`. It **must be a pure deterministic function of its
  arguments** — see the module-level "Determinism" section for what breaks if
  it is not. It is called from multiple threads on the `CpuMulti` path, hence
  the `Sync` bound; the bound is present in both feature builds so that
  enabling `parallel` never changes a public signature.

# Returns

A [`MinBatch`] with one [`MinSolution`] per problem, in problem order. An
empty `problems` slice returns an empty batch and calls `f` zero times.

# Cost

Two objective evaluations up front, one per iteration, and one final
evaluation at the returned bracket midpoint. The iteration count is fixed by
the geometry alone — `ceil(ln(tol/W0) / ln(GOLDEN_RATIO))` — and does **not**
depend on the objective's shape, which is what makes this kernel's per-lane
work uniform and its batched form the cleanest fit for lockstep execution in
the whole hybrid-backend epic.

# Preconditions the kernel cannot check

The objective must be unimodal on each bracket. It is not verified and cannot
be; a multimodal bracket yields *a* local extremum with `Converged` status.
See the module-level warning — this is the failure mode to worry about.

# Determinism

Bit-for-bit identical on [`ComputeBackend::Serial`] and
[`ComputeBackend::CpuMulti`], at any thread count, for a pure `f`. Both
backends run the same per-lane kernel and no arithmetic crosses lanes.

# Non-convergence

Per lane, never swallowed. A lane whose bracket end is non-finite reports
[`MinStatus::InvalidBracket`] and a `NaN` iterate; a lane whose objective
returns a non-finite value reports [`MinStatus::NotFinite`] at the offending
abscissa; a lane that exhausts `max_iterations` reports
[`MinStatus::MaxIterations`] and its bracket midpoint, which
[`MinSolution::extremum`] still refuses to hand out. [`MinBatch::extrema`]
turns any failure into an `Err`.

# Verification

*Methodology.* Checked against extrema known in closed form, so the oracle is
exact rather than another implementation. Four families, all on 64 lanes
(except where noted) with vertices spread over `[-3, 3)` and bracket `[-5, 5]`:

1. **Quadratic** `f(x) = 1 + (x - x0)^2` at [`MinSettings::default`]. Exact
   minimiser `x0`, minimum value `1`. The realistic reference case — an
   order-unity value at the extremum, as a physical objective has.
2. **Deliberately flat** `f(x) = 1 + (x - x0)^4`, with the tolerance driven
   *below* every arithmetic floor so the arithmetic binds rather than the
   stopping rule. Same exact minimiser; quartically flat there.
3. **Transcendental** `f(x) = x ln x` on `[0.05, 3]`, one lane, whose
   minimiser is `1/e = 0.36787944117144233` and minimum value `-1/e`, both
   exactly.
4. **Maximisation** `f(x) = sin x` on `[0, pi]`, one lane, maximiser `pi/2`
   and maximum `1`, run under [`Sense::Maximise`] so the sense switch is
   checked against an analytic answer rather than against the minimisation
   path — a mirror test would pass even if both senses were wrong in the same
   way.

*Pass criterion.* `status == Converged` on every lane, and
`|x_located - x_analytic| <= 1e-6` for families 1, 3 and 4 (two orders of
margin over the `sqrt(eps) ≈ 1.5e-8` floor), plus `|f_located - f_analytic| <=
1e-12` for families 3 and 4. Family 2's criterion is deliberately `1e-2`,
because `eps^(1/4) ≈ 1.2e-4` is the floor there and asserting `1e-6` would be
asserting something false.

*Results, measured 2026-08-13 by `golden_section_matches_analytic_minima`,
`flat_minimum_exposes_the_sqrt_eps_limit`,
`golden_section_matches_analytic_transcendental_minimum` and
`maximise_matches_analytic_sine_peak` in `minimise/tests.rs`, release build,
4 logical cores:*

| Family | worst `\|x - x_analytic\|` | worst value error | iterations |
|---|---|---|---|
| 1. `1 + (x - x0)^2`, default tol | **1.151892e-8** | **2.220446e-16** | 40-47 |
| 2. `1 + (x - x0)^4`, tol below the floor | **1.026485e-4** | — | 77 |
| 3. `x ln x` | **6.967008e-10** | **5.551115e-17** | 42 |
| 4. `sin x`, [`Sense::Maximise`] | **0.000000e0** | **0.000000e0** | 39 |

*Interpretation.* Family 1 lands at 0.77x the `sqrt(eps)` floor — the method
delivers exactly the accuracy the arithmetic allows and no more, which is what
it should do given the default `x_tol_rel` *is* that floor. Its value error is
one ULP at `f = 1`, illustrating the module's point about why a value-based
convergence test would be wrong: the value is already correct to the last bit
while the abscissa is still wrong in its 9th digit. Family 2 is **9 742x
worse** in the abscissa than family 1 on identical lanes, which is the flat-
minimum penalty measured rather than asserted. Family 3 confirms the same
behaviour on a transcendental objective, ruling out an artefact of polynomial
test functions. Family 4 recovers `pi/2` and `1.0` exactly, so
[`Sense::Maximise`] is not merely self-consistent but correct, and the value
is returned with the caller's own positive sign — nothing is negated
internally.

The iteration counts in family 1 vary (40-47) only because
[`MinSettings::default`] scales its tolerance with `|x_mid|`; under a purely
absolute tolerance the count is identical on every lane, since golden
section's contraction is fixed by geometry and not by the objective. That
uniformity is verified separately by `bracket_contracts_at_the_golden_ratio`.

# Example — a batch of parabolas

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::math::minimise::{
    golden_section_batch, MinProblem, MinSettings, Sense,
};

// Minimise (x - 2)^2 + 1 on [-10, 10]: the vertex is at x = 2, value 1.
let problems = [MinProblem::new(-10.0, 10.0)];
let batch = golden_section_batch(
    &problems,
    Sense::Minimise,
    MinSettings::default(),
    ComputeBackend::Serial,
    |_, x| (x - 2.0) * (x - 2.0) + 1.0,
);

let s = batch.solutions()[0];
assert!(s.converged());
// The abscissa and the value are separate accessors, on purpose.
assert!((s.extremum().unwrap() - 2.0).abs() < 1e-6);
assert!((s.extremal_value().unwrap() - 1.0).abs() < 1e-12);
```

# Example — maximisation at the `uom` boundary

The batch is dimensionless and the caller converts at its edge. This lane has
the shape of the workspace's production golden-section caller: maximise a
mass flux over a pressure bracket, keeping `uom` typing on both sides of the
call.

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::math::minimise::{
    golden_section_batch, MinProblem, MinSettings, Sense,
};
use uom::si::f64::{MassFlux, Pressure};
use uom::si::mass_flux::kilogram_per_square_meter_second;
use uom::si::pressure::pascal;

// A stand-in for G(p) along an isentrope: a single interior peak at 2 MPa.
// (Analytic, not thermodynamic -- the real one is an IF97 flash.)
let g_of_p = |p: Pressure| -> MassFlux {
    let x = p.get::<pascal>() / 1.0e6;
    MassFlux::new::<kilogram_per_square_meter_second>(1.0e4 * (1.0 - (x - 2.0) * (x - 2.0) / 9.0))
};

// Convert in: the bracket in pascals, as plain f64.
let lo = Pressure::new::<pascal>(0.1e6);
let hi = Pressure::new::<pascal>(5.0e6);
let problems = [MinProblem::new(lo.get::<pascal>(), hi.get::<pascal>())];

let batch = golden_section_batch(
    &problems,
    Sense::Maximise,
    MinSettings { x_tol_abs: 1.0, x_tol_rel: 0.0, max_iterations: 100 },
    ComputeBackend::Serial,
    |_, p_pa| g_of_p(Pressure::new::<pascal>(p_pa)).get::<kilogram_per_square_meter_second>(),
);

// Convert out: back to a typed pressure and mass flux.
let s = batch.solutions()[0];
let p_star = Pressure::new::<pascal>(s.extremum().expect("converged"));
let g_star = MassFlux::new::<kilogram_per_square_meter_second>(
    s.extremal_value().expect("converged"),
);

assert!((p_star.get::<pascal>() - 2.0e6).abs() <= 1.0); // 1 Pa bracket
assert!((g_star.get::<kilogram_per_square_meter_second>() - 1.0e4).abs() < 1.0e-3);
```

```rust
pub fn golden_section_batch<F>(problems: &[MinProblem], sense: Sense, settings: MinSettings, backend: crate::compute::ComputeBackend, f: F) -> MinBatch
where
    F: Fn(usize, f64) -> f64 + Sync { /* ... */ }
```

### Constants and Statics

#### Constant `GOLDEN_RATIO`

The golden-section contraction factor `(sqrt(5) - 1) / 2 = 0.6180339...`.

Each iteration replaces the bracket `[a, b]` by one of `[a, d]` or `[c, b]`,
both of width exactly `GOLDEN_RATIO * (b - a)`. That the two candidate
sub-brackets have the *same* width is what makes the contraction rate
independent of the objective, and the specific value is what makes one probe
of the new bracket coincide with a probe of the old one — so each iteration
after the first costs a single objective evaluation.

Equivalently `1 - GOLDEN_RATIO = (3 - sqrt(5)) / 2 = 0.3819660...`, which is
the form bead `op-yvj.4.3` and most textbooks quote; the two are the same
algorithm written from opposite ends of the bracket.

# Units

Dimensionless.

```rust
pub const GOLDEN_RATIO: f64 = 0.618_033_988_749_894_9;
```

#### Constant `SQRT_EPSILON`

`sqrt(f64::EPSILON)` = `1.4901161193847656e-8` — the practical accuracy floor
for locating a smooth minimum by comparing function values.

See the module-level "Achievable accuracy" section for the derivation. This
is the default [`MinSettings::x_tol_rel`], and it is a *floor*, not a
guarantee: an objective flatter than quadratic at its minimum does worse, and
an objective evaluated with more than rounding-level noise does worse again.

# Units

Dimensionless — it multiplies `|x|` to give an abscissa tolerance.

```rust
pub const SQRT_EPSILON: f64 = 1.490_116_119_384_765_6e-8;
```

#### Constant `MINIMISE_BATCH_MIN_PROBLEMS`

Problem count below which a [`ComputeBackend::CpuMulti`] request runs
[`golden_section_batch`] on the calling thread instead.

# Measured crossover

*Methodology.* Measured 2026-08-13 on this workspace's development machine,
`std::thread::available_parallelism()` = **4**, release build, `--features
parallel`, `rayon`'s global pool, otherwise-idle machine. Batches of `n`
independent problems `(x - x0_i)^2` with the vertices `x0_i` spread over
`[-3, 3)` so per-lane iteration counts differ, bracket `[-5, 5]`,
[`MinSettings::default`], [`Sense::Minimise`]. Best of 7 samples per point,
reported as wall clock for one whole batch. Produced by the `#[ignore]`d
`minimise_batch_crossover_benchmark` test in `minimise/tests.rs` and
transcribed from its printed output. The `cheap` objective is the two-flop
parabola above; the `costly` objective adds a `ln`/`exp`/`sqrt` chain per
evaluation, standing in for an equation-of-state flash. The `(repeat)`
columns are the speed-ups from a second, independent run of the same
benchmark, carried alongside rather than averaged away because the parallel
column is far noisier than the serial one.

| Problems | cheap serial | cheap multi | speed-up | (repeat) | costly serial | costly multi | speed-up | (repeat) |
|---|---|---|---|---|---|---|---|---|
| 16 | 2.24 us | 10.06 us | 0.22x | 0.24x | 13.87 us | 12.94 us | 1.07x | 0.41x |
| 32 | 4.40 us | 29.98 us | 0.15x | 0.13x | 29.80 us | 44.04 us | 0.68x | 0.74x |
| 64 | 8.77 us | 31.47 us | 0.28x | 0.34x | 63.53 us | 64.86 us | 0.98x | 1.24x |
| 128 | 17.96 us | 36.40 us | 0.49x | 0.86x | 140.68 us | 73.43 us | 1.92x | 1.42x |
| 256 | 55.42 us | 49.41 us | 1.12x | 1.17x | 290.00 us | 186.77 us | 1.55x | 2.91x |
| 512 | 148.21 us | 85.96 us | 1.72x | 3.00x | 613.95 us | 360.42 us | 1.70x | 2.94x |
| 1 024 | 370.80 us | 213.96 us | 1.73x | 2.81x | 1255.20 us | 675.41 us | 1.86x | 2.82x |
| 4 096 | 1542.64 us | 466.86 us | 3.30x | 3.23x | 5120.51 us | 1711.31 us | 2.99x | 2.86x |
| 16 384 | 6260.28 us | 1964.94 us | 3.19x | 2.09x | 20732.41 us | 10689.59 us | 1.94x | 2.69x |
| 65 536 | 28254.43 us | 8464.29 us | 3.34x | 1.55x | 84607.33 us | 32110.15 us | 2.63x | 2.03x |

*Result.* **256** is the smallest size at which *neither* objective lost in
*either* run — cheap 1.12x and 1.17x, costly 1.55x and 2.91x — and it is the
value this constant takes. At 128 the cheap objective lost both runs (0.49x,
0.86x) while the costly one won both (1.92x, 1.42x), which is the expected
shape: a more expensive objective crosses over earlier because there is more
work per lane to amortise the same dispatch cost. The honest headline is
therefore that **the crossover depends on the caller's objective**, and this
single number is set for the cheapest objective a caller is likely to pass, so
that it is safe for both.

# Relationship to [`crate::compute::CPU_MULTI_MIN_WORK_ITEMS`]

That constant documents itself as an unmeasured placeholder and invites
per-kernel overrides. This is that override for the batched golden section.

*Interpretation.* It lands on exactly the same value as
[`crate::math::parallel::ROOT_BATCH_MIN_PROBLEMS`] (256), 16x below the
crate-wide placeholder (4 096) and 512x below
[`crate::fields::parallel::FIELD_PARALLEL_CROSSOVER`] (131 072). Two kernels
agreeing is not a coincidence and is worth stating for whoever takes bead
`op-yvj.4.7`: both do tens of iterations of arithmetic per lane against a
handful of loaded bytes, so both are compute bound, where the field kernels do
one or two flops per element loaded and are memory-bandwidth bound. The
crossover tracks that distinction, not the algorithm.

# Limitations

One machine, four logical cores, one objective family, idle. Not measured on
Android/Termux hardware and not on a many-core server. As with
[`crate::math::parallel::ROOT_BATCH_MIN_PROBLEMS`], the crossover moves with
the caller's objective cost, so a caller that knows its own objective is
expensive is better off naming a [`ComputeBackend`] explicitly than trusting
a single number.

# Units

A count of independent minimisation problems, dimensionless.

```rust
pub const MINIMISE_BATCH_MIN_PROBLEMS: usize = 256;
```

## Module `parallel`

Batched scalar root finding on the hybrid execution backend — solve `N`
independent `f(x) = 0` problems at once, serially or across CPU cores.

# The shape of the problem

A finite-volume solver almost never wants *one* root. It wants one root per
cell: given a cell-wise target enthalpy `h_target[c]`, find the temperature
`T[c]` satisfying `h(T[c]) - h_target[c] = 0`, for every cell in the mesh.
Those `N` problems are completely independent of one another — no cell's
iteration reads another cell's iterate — which is what makes the batch worth
threading at all.

This module therefore provides **batched** entry points only. A single scalar
root solve should stay serial and inline in its caller: one root is a few
microseconds of work and threading it is a straight loss.

| Operation | Derivative needed? | Entry point |
|---|---|---|
| bisection or Brent over a bracket | no | [`solve_bracketed_batch`] |
| safeguarded Newton over a bracket | yes | [`solve_newton_batch`] |
| closed-form roots of `a x + b` | — | [`linear_roots_batch`] |
| closed-form roots of `a x^2 + b x + c` | — | [`quadratic_roots_batch`] |
| closed-form roots of `a x^3 + b x^2 + c x + d` | — | [`cubic_roots_batch`] |

# Hybrid means dispatch, not two APIs

Every entry point takes a [`ComputeBackend`] parameter. There is no
`solve_batch_parallel()` sibling beside `solve_batch()`. With the `parallel`
feature off, [`ComputeBackend::CpuMulti`] resolves down to
[`ComputeBackend::Serial`] through [`ComputeBackend::resolve`] and the answer
is unchanged — bit for bit, not merely close. There is no `Gpu` kernel here
yet, so a `Gpu` request degrades to the best available CPU path.

# Determinism — bitwise identical, and why that is stronger here

**Every kernel in this module returns bit-for-bit identical output on
[`ComputeBackend::Serial`] and [`ComputeBackend::CpuMulti`], at any thread
count, on every run** — provided the caller's residual closure is itself a
deterministic pure function of `(index, x)`.

This is easier to achieve than it is for a reduction and the reason is worth
stating, because it is the thing most people get wrong about parallel root
finding. A parallel sum has to re-associate `a + b + c`, and floating-point
addition is not associative, so its last bits depend on the split. A batch of
root solves has **no cross-element arithmetic at all**: lane `i`'s answer is a
pure function of lane `i`'s bracket and closure evaluations. Splitting the
lanes across threads cannot perturb any of them. Both backends call the very
same `#[inline]` per-lane kernel; only the identity of the calling thread
differs.

The one way a caller can break this is to supply a closure that is not a pure
function — one that reads a random number generator, or accumulates into
shared interior-mutable state, or depends on the calling thread. Such a
closure makes the batch non-reproducible no matter what this module does. The
`Sync` bound permits it; the documented contract forbids it.

Verified by the `bitwise_*` tests in `parallel/tests.rs`, which compare
serial against `rayon` pools of 1, 2, 4 and 8 workers on a batch deliberately
built with wildly varying per-lane iteration counts.

# Load imbalance — why there is no hand-rolled partition

Iteration counts vary per lane, sometimes by an order of magnitude: a lane
whose bracket happens to straddle the root tightly converges in three
iterations, and its neighbour takes eighty. A static equal-split across `P`
threads therefore ends up waiting on whichever chunk drew the hard lanes.

The parallel paths here use `rayon`'s adaptive splitting (`par_iter_mut`,
`into_par_iter`) with **no** `min_len` floor on the iterative solvers, so an
idle worker steals down to individual lanes when the work is skewed. That is
a deliberate choice, not an oversight: the whole reason to prefer
work-stealing here is the per-lane cost variance. The closed-form polynomial
kernels are the exception — every lane there costs the same handful of
floating-point operations, so they set a granularity floor of
[`POLY_BLOCK`] lanes to stop the splitter subdividing below a task that
cannot pay for itself. That floor affects wall clock only; it cannot change a
value, because each lane is independent.

For scale, a 65 536-lane deliberately-imbalanced batch (half the lanes given
a 2e-6-wide bracket, half given `[0, 1e6]`) under Brent, measured 2026-08-12
on 4 logical cores by the `#[ignore]`d `root_batch_thread_scaling_benchmark`,
best of 7 samples, with a second independent run alongside:

| Worker threads | Time | Speed-up | (repeat) | Bitwise vs serial |
|---|---|---|---|---|
| *serial reference* | 19322.52 us | 1.00x | 1.00x | — |
| 1 | 20510.52 us | 0.94x | 0.96x | identical |
| 2 | 10123.65 us | 1.91x | 1.88x | identical |
| 4 | 7075.89 us | 2.73x | 3.66x | identical |
| 8 | 7314.01 us | 2.64x | 3.57x | identical |

The "identical" column is the determinism claim above measured rather than
argued, and it is asserted by the benchmark itself, not merely printed. The
one-worker row costs about 6% against plain serial, which is the price of
going through `rayon` at all. Unlike the memory-latency-bound sparse product
in [`crate::ldu_matrix::parallel`], eight workers on four cores buy nothing
further here — the expected signature of a compute-bound kernel that already
saturates the cores it has. **This is one machine, one batch and two runs; it
is not a scaling study**, and nothing here has been measured on Android
hardware or on a many-core server.

# Non-convergence is reported, never swallowed

A batch of 10 000 problems in which 3 fail must say so. This module reports
failure **per lane**, and makes the failure impossible to ignore by
construction:

- Every lane carries a [`RootStatus`] and is reachable through
  [`RootBatch::solutions`].
- [`RootSolution::root`] returns `Option<f64>` — `Some(x)` **only** when that
  lane converged. There is no accessor that hands out a bare `f64` without
  the caller having seen the status; the diagnostic iterate is behind the
  deliberately-named [`RootSolution::last_iterate`].
- [`RootBatch::roots`] is the all-or-nothing path: it returns
  `Err(`[`RootBatchFailure`]`)` naming the failure count and the first
  failing lane, rather than a plausible-looking `Vec<f64>`.

**A non-converged lane is never clamped to a bracket endpoint and called a
root.** A lane that ran out of iterations reports its best iterate with
[`RootStatus::MaxIterations`]; a lane whose bracket does not straddle a root
reports [`RootStatus::NotBracketed`] and a `NaN` iterate, because there is no
honest number to return.

# Why this module lives under `math/` and not `polynomial/`

The bulk of it is a general scalar root finder over a caller-supplied
residual — `h(T) - h_target`, a Peng-Robinson compressibility, a saturation
curve. None of that is polynomial. `math/` is already the home of the
crate's iterative inversions ([`crate::math::inv_inc_gamma`] is a
Newton-refined inverse), so a general solver belongs beside them; a caller
inverting an enthalpy would not think to look in a module called
`polynomial`. The batched closed-form polynomial kernels are thin wrappers
over [`crate::polynomial`]'s existing per-equation solvers and are kept here
with the rest of the batching so that the crate has **one** batched
root-finding vocabulary rather than two dialects in two modules.

# Units

Everything here is dimensionless `f64`, for the same reason
[`crate::ldu_matrix::parallel`] and [`crate::krylov::vecops`] are: a generic
root finder has no single physical dimension. The abscissa `x` of one lane
may be a temperature in kelvin and of another a compressibility factor, and
the residual's dimension is whatever the caller's function returns.

`uom` typing is **not stripped** to get here — it is applied at the boundary,
by the caller, exactly as the hybrid-backend epic requires: convert into the
batch, convert back out. The doctest on [`solve_newton_batch`] shows that
boundary explicitly, inverting a `uom`-typed enthalpy relation for a
`ThermodynamicTemperature`.

# Cargo features and portability

The `rayon` paths sit behind the crate's `parallel` feature, which is **off
by default**; with it off this module still compiles and every entry point
still works. `rayon` is pure Rust with no system component, so everything
here compiles and runs on `aarch64-linux-android` / Termux exactly as on
desktop. Nothing in this module is target-gated.

# Example

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::math::parallel::{
    solve_bracketed_batch, RootMethod, RootProblem, RootSettings,
};

// 4 independent problems: x^2 = k for k = 1, 2, 3, 4, each bracketed on [0, 4].
let targets = [1.0, 2.0, 3.0, 4.0];
let problems: Vec<RootProblem> = (0..4).map(|_| RootProblem::new(0.0, 4.0)).collect();

let batch = solve_bracketed_batch(
    &problems,
    RootMethod::Brent,
    RootSettings::default(),
    ComputeBackend::CpuMulti,
    |i, x| x * x - targets[i],
);

let roots = batch.roots().expect("all four lanes converge");
assert!((roots[1] - 2.0_f64.sqrt()).abs() < 1e-12);

// Asking for multi-CPU gives a bit-for-bit identical answer, whether or not
// the `parallel` feature is compiled in.
let serial = solve_bracketed_batch(
    &problems,
    RootMethod::Brent,
    RootSettings::default(),
    ComputeBackend::Serial,
    |i, x| x * x - targets[i],
);
assert_eq!(roots, serial.roots().unwrap());
```

```rust
pub mod parallel { /* ... */ }
```

### Types

#### Enum `RootMethod`

Which derivative-free bracketed method [`solve_bracketed_batch`] should run.

Both methods keep a bracket `[a, b]` across which the residual changes sign
and shrink it until it meets the tolerance, so both are guaranteed to
converge on a continuous residual that is genuinely bracketed. They differ
only in how fast.

# Units

Dimensionless — a mode selector, not a quantity.

```rust
pub enum RootMethod {
    Bisection,
    Brent,
}
```

##### Variants

###### `Bisection`

Repeated bisection of the bracket.

One residual evaluation per iteration, and the bracket width halves
every time, so reaching a relative tolerance of `1e-12` from an
order-unity bracket costs about 40 evaluations regardless of the
function. Slow but utterly unshakeable: it cannot be defeated by a kink,
a plateau, or a badly conditioned derivative.

###### `Brent`

Brent's method — inverse quadratic interpolation and the secant rule,
falling back to bisection whenever the interpolated step misbehaves.

The default, and the right choice unless you have a specific reason to
want bisection's fixed cost. Superlinear on a smooth residual: the same
`1e-12` tolerance typically costs 6-12 evaluations instead of 40, while
retaining bisection's guarantee because every step that would leave the
bracket or shrink it too slowly is replaced by a bisection step.

Implemented from the published description of the algorithm (R. P.
Brent, *Algorithms for Minimization without Derivatives*, Prentice-Hall
1973, chapter 4), not transcribed from any existing implementation.

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
    fn clone(self: &Self) -> RootMethod { /* ... */ }
    ```

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
    fn default() -> RootMethod { /* ... */ }
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
    fn eq(self: &Self, other: &RootMethod) -> bool { /* ... */ }
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
#### Struct `RootProblem`

One bracketed root problem: an interval believed to contain a root, plus an
optional starting iterate for the Newton solver.

# Fields and valid ranges

- `lo`, `hi` — the bracket. Both must be finite. They may be given in either
  order; the solvers do not require `lo < hi`. The residual must have
  opposite signs at the two ends (or be exactly zero at one of them),
  otherwise the lane reports [`RootStatus::NotBracketed`] rather than
  guessing.
- `guess` — the initial iterate for [`solve_newton_batch`]. Ignored by
  [`solve_bracketed_batch`], which is driven entirely by the bracket. A
  non-finite guess, or one outside `[lo, hi]`, is replaced by the bracket
  midpoint rather than rejected — a bad guess is a performance problem, not
  a correctness one, because the Newton solver is bracket-safeguarded.

# Units

Dimensionless `f64` in whatever units the caller's abscissa carries — kelvin
for a temperature inversion, dimensionless for a compressibility factor. See
the module-level "Units" section.

# Example

```rust
use outram_foam_basic_lib::math::parallel::RootProblem;

// Bracket only; the Newton guess defaults to the midpoint.
let p = RootProblem::new(300.0, 3000.0);
assert_eq!(p.guess, 1650.0);

// Or seed Newton from the previous timestep's answer.
let q = RootProblem::with_guess(300.0, 3000.0, 812.5);
assert_eq!(q.guess, 812.5);
```

```rust
pub struct RootProblem {
    pub lo: f64,
    pub hi: f64,
    pub guess: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `lo` | `f64` | One end of the bracket. Finite; need not be less than `hi`. |
| `hi` | `f64` | The other end of the bracket. Finite; need not be greater than `lo`. |
| `guess` | `f64` | Starting iterate for [`solve_newton_batch`]; ignored by<br>[`solve_bracketed_batch`]. |

##### Implementations

###### Methods

- ```rust
  pub fn new(lo: f64, hi: f64) -> Self { /* ... */ }
  ```
  A problem bracketed on `[lo, hi]`, with the Newton guess set to the

- ```rust
  pub fn with_guess(lo: f64, hi: f64, guess: f64) -> Self { /* ... */ }
  ```
  A problem bracketed on `[lo, hi]` with an explicit Newton starting

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> RootProblem { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &RootProblem) -> bool { /* ... */ }
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
#### Struct `RootSettings`

Convergence tolerances and the iteration cap, shared by both iterative
solvers.

A lane is converged when **either** the bracket (bisection, Brent) or the
step (Newton) has shrunk to `x_tol_abs + x_tol_rel * |x|`, **or** the
residual magnitude has fallen to `f_tol`. An exactly-zero residual always
converges regardless of tolerances.

# Fields and valid ranges

- `x_tol_abs` — absolute abscissa tolerance, in the abscissa's own units.
  Must be `>= 0`. Guards the case `x ≈ 0`, where a purely relative tolerance
  can never be met.
- `x_tol_rel` — relative abscissa tolerance, dimensionless, `>= 0`.
- `f_tol` — residual tolerance, in the residual's own units, `>= 0`. Default
  `0.0`, i.e. **disabled**: with no knowledge of the residual's scale, a
  nonzero default would be a guess. Set it when you know what "small enough"
  means for your function; leaving it at zero simply means the abscissa
  tolerance decides.
- `max_iterations` — hard cap. On reaching it a lane reports
  [`RootStatus::MaxIterations`] and its best iterate. Must be `>= 1`.

A negative tolerance is not rejected; it simply can never be met, and the
lane will report [`RootStatus::MaxIterations`]. That is a truthful outcome
rather than a silent success.

# Defaults

`x_tol_abs = 1e-12`, `x_tol_rel = 1e-12`, `f_tol = 0.0`,
`max_iterations = 100`. One hundred iterations is comfortable for Brent
(typically 6-12) and adequate for bisection over any bracket narrower than
about `2^100` tolerances.

# Example

```rust
use outram_foam_basic_lib::math::parallel::RootSettings;

// Struct-update syntax keeps the defaults you did not mean to change.
let s = RootSettings {
    max_iterations: 200,
    ..RootSettings::default()
};
assert_eq!(s.x_tol_rel, 1e-12);
assert_eq!(s.max_iterations, 200);
```

```rust
pub struct RootSettings {
    pub x_tol_abs: f64,
    pub x_tol_rel: f64,
    pub f_tol: f64,
    pub max_iterations: u32,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x_tol_abs` | `f64` | Absolute abscissa tolerance, in the abscissa's units. `>= 0`. |
| `x_tol_rel` | `f64` | Relative abscissa tolerance, dimensionless. `>= 0`. |
| `f_tol` | `f64` | Residual tolerance, in the residual's units. `0.0` disables it. |
| `max_iterations` | `u32` | Maximum iterations per lane before reporting<br>[`RootStatus::MaxIterations`]. `>= 1`. |

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
    fn clone(self: &Self) -> RootSettings { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &RootSettings) -> bool { /* ... */ }
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
#### Enum `RootStatus`

How one lane of a batch ended.

Only [`Converged`](Self::Converged) means the lane produced a root. Every
other variant is a failure that a caller must handle; see the module-level
"Non-convergence is reported, never swallowed" section for the accessors
that make it hard to skip.

| Variant | `last_iterate` | Meaning |
|---|---|---|
| [`Converged`](Self::Converged) | the root | a tolerance was met |
| [`MaxIterations`](Self::MaxIterations) | best iterate so far — **not** a root | ran out of iterations |
| [`NotBracketed`](Self::NotBracketed) | `NaN` | residual has the same sign at both bracket ends |
| [`InvalidBracket`](Self::InvalidBracket) | `NaN` | a bracket end is not finite |
| [`NotFinite`](Self::NotFinite) | the abscissa where it happened | the residual (or derivative) evaluated to `NaN`/infinity |

# Units

Dimensionless — a status tag.

```rust
pub enum RootStatus {
    Converged,
    MaxIterations,
    NotBracketed,
    InvalidBracket,
    NotFinite,
}
```

##### Variants

###### `Converged`

A root was found to the requested tolerance.

###### `MaxIterations`

`max_iterations` was reached with no tolerance met. The reported iterate
is the best one seen, and is **not** claimed to be a root.

###### `NotBracketed`

The residual has the same (nonzero) sign at both ends of the bracket, so
no root of a continuous residual is guaranteed to lie inside it. The
solver refuses to guess and reports this instead of returning an
endpoint.

###### `InvalidBracket`

A bracket end was infinite or `NaN`, so there is no interval to search.

###### `NotFinite`

The residual — or, for Newton, the value half of the residual/derivative
pair — evaluated to a non-finite number. Iteration stops immediately,
because continuing would propagate the `NaN` into a plausible-looking
answer.

##### Implementations

###### Methods

- ```rust
  pub fn is_converged(self: Self) -> bool { /* ... */ }
  ```
  Whether this status means the lane produced a usable root.

- ```rust
  pub fn label(self: Self) -> &'static str { /* ... */ }
  ```
  A short human-readable label, for log lines and failure reports.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> RootStatus { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &RootStatus) -> bool { /* ... */ }
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
#### Struct `RootSolution`

The outcome of a single lane: its status, its iterate, its residual and the
work it took.

The fields are private on purpose. The only way to get a number that is
claimed to be a root is [`Self::root`], which returns `Option<f64>` and hands
back `Some` only for a converged lane. The raw iterate is available from
[`Self::last_iterate`], whose name is chosen so that using it as a root is a
visible decision in the calling code rather than an accident.

`Copy`, so it can be read out of a [`RootBatch`] without cloning.

# Units

[`Self::root`] and [`Self::last_iterate`] are in the abscissa's units;
[`Self::residual`] is in the residual's units; [`Self::iterations`] and
[`Self::bracket_width`] are a dimensionless count and an abscissa-unit width
respectively.

```rust
pub struct RootSolution {
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
  pub fn root(self: &Self) -> Option<f64> { /* ... */ }
  ```
  The root, if this lane converged.

- ```rust
  pub fn last_iterate(self: &Self) -> f64 { /* ... */ }
  ```
  The last iterate the solver held, converged or not.

- ```rust
  pub fn residual(self: &Self) -> f64 { /* ... */ }
  ```
  The residual at [`Self::last_iterate`], in the residual's units.

- ```rust
  pub fn bracket_width(self: &Self) -> f64 { /* ... */ }
  ```
  Width of the final bracket, in the abscissa's units.

- ```rust
  pub fn iterations(self: &Self) -> u32 { /* ... */ }
  ```
  Iterations performed on this lane, dimensionless.

- ```rust
  pub fn status(self: &Self) -> RootStatus { /* ... */ }
  ```
  How this lane ended.

- ```rust
  pub fn converged(self: &Self) -> bool { /* ... */ }
  ```
  Whether this lane produced a root.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> RootSolution { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &RootSolution) -> bool { /* ... */ }
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
#### Struct `RootBatch`

A batch of `N` lane outcomes, in the same order as the problems handed in.

Lane `i` of the result corresponds to `problems[i]`, always — the parallel
path preserves order, so no index bookkeeping is needed.

# Getting numbers out

- [`Self::roots`] — all-or-nothing. `Ok(Vec<f64>)` only when every lane
  converged; otherwise `Err(`[`RootBatchFailure`]`)`. Use this when a partial
  answer is useless, which for a per-cell thermodynamic inversion it usually
  is.
- [`Self::solutions`] — per-lane, when the caller wants to handle failures
  individually (fall back to a wider bracket, flag the cell, sub-cycle).

# Units

See [`RootSolution`].

```rust
pub struct RootBatch {
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
  pub fn solutions(self: &Self) -> &[RootSolution] { /* ... */ }
  ```
  Every lane's outcome, in problem order.

- ```rust
  pub fn into_solutions(self: Self) -> Vec<RootSolution> { /* ... */ }
  ```
  Consume the batch and take the outcomes.

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Number of lanes, dimensionless.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether the batch has no lanes.

- ```rust
  pub fn get(self: &Self, i: usize) -> Option<RootSolution> { /* ... */ }
  ```
  Lane `i`'s outcome, or `None` if `i` is out of range.

- ```rust
  pub fn all_converged(self: &Self) -> bool { /* ... */ }
  ```
  Whether every lane converged. Vacuously `true` for an empty batch.

- ```rust
  pub fn failure_count(self: &Self) -> usize { /* ... */ }
  ```
  How many lanes failed to converge, dimensionless.

- ```rust
  pub fn first_failure(self: &Self) -> Option<(usize, RootSolution)> { /* ... */ }
  ```
  The first failing lane and its outcome, if any.

- ```rust
  pub fn failures(self: &Self) -> Vec<(usize, RootSolution)> { /* ... */ }
  ```
  Every failing lane, as `(index, outcome)` pairs.

- ```rust
  pub fn roots(self: &Self) -> Result<Vec<f64>, RootBatchFailure> { /* ... */ }
  ```
  All roots, or an error naming the failures — the all-or-nothing path.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> RootBatch { /* ... */ }
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
    fn eq(self: &Self, other: &RootBatch) -> bool { /* ... */ }
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
#### Struct `RootBatchFailure`

**Attributes:**

- `Other("#[error(\"{failure_count} of {total} root problems did not converge; \\\n     first failure at lane {first_index} with status {first_status:?} \\\n     after {first_iterations} iterations\")]")`

One or more lanes of a [`RootBatch`] did not converge.

Returned by [`RootBatch::roots`]. It names both the scale of the problem
(how many of how many) and a specific lane to look at, because "3 of 10 000
cells failed" is only actionable once you know *which* cell.

# Units

All counts and indices are dimensionless.

```rust
pub struct RootBatchFailure {
    pub total: usize,
    pub failure_count: usize,
    pub first_index: usize,
    pub first_status: RootStatus,
    pub first_iterations: u32,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `total` | `usize` | Number of lanes in the batch. |
| `failure_count` | `usize` | Number of lanes that did not converge. |
| `first_index` | `usize` | Index of the first non-converged lane. |
| `first_status` | `RootStatus` | Why that lane failed. |
| `first_iterations` | `u32` | Iterations that lane performed before giving up. |

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
    fn clone(self: &Self) -> RootBatchFailure { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **Eq**
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
    fn eq(self: &Self, other: &RootBatchFailure) -> bool { /* ... */ }
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
### Functions

#### Function `root_batch_backend_for`

**Attributes:**

- `MustUse { reason: None }`

The [`ComputeBackend`] the iterative solvers would actually use for `n`
problems if asked for `requested` — without running anything.

Applies exactly the same reduction the kernels do (feature availability, no
GPU kernel here, and the [`ROOT_BATCH_MIN_PROBLEMS`] size floor), so what it
reports is what would run. Useful for logging and for benchmark harnesses.

# Arguments

- `requested` — the backend a caller would pass to [`solve_bracketed_batch`]
  or [`solve_newton_batch`].
- `n` — the number of independent problems in the batch, dimensionless.

# Returns

Either [`ComputeBackend::Serial`] or [`ComputeBackend::CpuMulti`]; never
[`ComputeBackend::Gpu`], because no GPU kernel exists here yet.

# Example

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::math::parallel::{root_batch_backend_for, ROOT_BATCH_MIN_PROBLEMS};

// Too small to thread, whatever was asked for.
assert_eq!(root_batch_backend_for(ComputeBackend::CpuMulti, 8), ComputeBackend::Serial);

// Big enough; the answer now depends only on whether `parallel` is compiled in.
let picked = root_batch_backend_for(ComputeBackend::CpuMulti, ROOT_BATCH_MIN_PROBLEMS);
assert!(picked.is_available());
```

```rust
pub fn root_batch_backend_for(requested: crate::compute::ComputeBackend, n: usize) -> crate::compute::ComputeBackend { /* ... */ }
```

#### Function `poly_roots_backend_for`

**Attributes:**

- `MustUse { reason: None }`

The [`ComputeBackend`] the closed-form polynomial kernels would actually use
for `n` equations if asked for `requested` — without running anything.

The polynomial counterpart of [`root_batch_backend_for`], differing only in
using the [`POLY_ROOTS_MIN_EQUATIONS`] size floor.

# Arguments

- `requested` — the backend a caller would pass to [`cubic_roots_batch`] and
  friends.
- `n` — the number of equations in the batch, dimensionless.

# Returns

Either [`ComputeBackend::Serial`] or [`ComputeBackend::CpuMulti`].

# Example

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::math::parallel::poly_roots_backend_for;

assert_eq!(poly_roots_backend_for(ComputeBackend::CpuMulti, 64), ComputeBackend::Serial);
assert_eq!(poly_roots_backend_for(ComputeBackend::Serial, 1 << 20), ComputeBackend::Serial);
```

```rust
pub fn poly_roots_backend_for(requested: crate::compute::ComputeBackend, n: usize) -> crate::compute::ComputeBackend { /* ... */ }
```

#### Function `solve_bracketed_batch`

**Attributes:**

- `MustUse { reason: None }`

Solve `N` independent bracketed root problems without a derivative, on the
chosen backend.

This is the entry point for the common case: a residual you can evaluate but
whose derivative you would rather not write, bracketed per lane. Both
[`RootMethod`]s are globally convergent on a continuous, genuinely bracketed
residual.

# Arguments

- `problems` — one [`RootProblem`] per lane. `problems[i]`'s bracket is used
  with `f(i, ·)`. The `guess` field is ignored.
- `method` — [`RootMethod::Brent`] unless you specifically want bisection's
  fixed, function-independent cost.
- `settings` — tolerances and iteration cap; see [`RootSettings`].
- `backend` — requested execution backend. What actually runs is
  [`root_batch_backend_for`] applied to it: an unavailable backend degrades,
  `Gpu` degrades (no GPU kernel here yet), and a batch below
  [`ROOT_BATCH_MIN_PROBLEMS`] runs serially. None of those changes the
  answer.
- `f` — the residual. `f(i, x)` must return the value of lane `i`'s function
  at abscissa `x`. It **must be a pure deterministic function of its
  arguments** — see the module-level "Determinism" section for what breaks if
  it is not. It is called from multiple threads on the `CpuMulti` path,
  hence the `Sync` bound; the bound is present in both feature builds so
  that enabling `parallel` never changes a public signature.

# Returns

A [`RootBatch`] with one [`RootSolution`] per problem, in problem order. An
empty `problems` slice returns an empty batch and calls `f` zero times.

# Determinism

Bit-for-bit identical on [`ComputeBackend::Serial`] and
[`ComputeBackend::CpuMulti`], at any thread count, for a pure `f`. Both
backends run the same per-lane kernel and no arithmetic crosses lanes.

# Non-convergence

Per lane, never swallowed. A lane whose bracket does not straddle a root
reports [`RootStatus::NotBracketed`] and a `NaN` iterate; a lane that
exhausts `max_iterations` reports [`RootStatus::MaxIterations`] and its best
iterate, which [`RootSolution::root`] still refuses to hand out as a root.
[`RootBatch::roots`] turns any failure into an `Err`.

# Verification

*Methodology.* The batch is checked against closed-form analytic roots, which
exist for the test residuals and so are an exact oracle rather than another
implementation. `(x - 1)(x - 2)(x - 3)` is solved on brackets isolating each
of its three known roots, and `x^2 - k` against `sqrt(k)`; the pass criterion
is `|x_computed - x_analytic| <= 1e-12` and `status == Converged` on every
lane.

*Results, measured 2026-08-12 by `bracketed_matches_analytic_cubic_roots` and
`brent_matches_analytic_sqrt` in `parallel/tests.rs`, release build:* worst
absolute error over the three cubic roots **0.000000e0** for *both*
[`RootMethod::Brent`] and [`RootMethod::Bisection`] — every lane recovered
1.0, 2.0 and 3.0 exactly — and worst absolute error over the 64 `sqrt(k)`
lanes **1.776357e-15**. All lanes converged. *Interpretation:* the integer
cubic roots are exactly representable and both methods land on them bit for
bit; the `sqrt(k)` roots are irrational, and 1.78e-15 is 8 units in the last
place at `k = 64`, i.e. the rounding floor. The iteration contributes no
error of its own beyond the residual's conditioning.

# Example

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::math::parallel::{
    solve_bracketed_batch, RootMethod, RootProblem, RootSettings,
};

// Three lanes, each isolating one root of (x-1)(x-2)(x-3).
let cubic = |x: f64| (x - 1.0) * (x - 2.0) * (x - 3.0);
let problems = [
    RootProblem::new(0.5, 1.5),
    RootProblem::new(1.5, 2.5),
    RootProblem::new(2.5, 3.5),
];

let batch = solve_bracketed_batch(
    &problems,
    RootMethod::Brent,
    RootSettings::default(),
    ComputeBackend::CpuMulti,
    |_, x| cubic(x),
);

let roots = batch.roots().expect("all lanes converge");
for (got, want) in roots.iter().zip([1.0, 2.0, 3.0]) {
    assert!((got - want).abs() < 1e-12, "got {got}, want {want}");
}
```

```rust
pub fn solve_bracketed_batch<F>(problems: &[RootProblem], method: RootMethod, settings: RootSettings, backend: crate::compute::ComputeBackend, f: F) -> RootBatch
where
    F: Fn(usize, f64) -> f64 + Sync { /* ... */ }
```

#### Function `solve_newton_batch`

**Attributes:**

- `MustUse { reason: None }`

Solve `N` independent root problems by bracket-safeguarded Newton iteration,
on the chosen backend.

Use this when the derivative is cheap to obtain alongside the value — which
for a thermodynamic inversion it is, because `dh/dT` is the specific heat the
same evaluation already computed. It is typically 2-4x fewer function
evaluations than [`RootMethod::Brent`] on a smooth residual.

# Why it is safeguarded, and not bare Newton

Bare Newton has no guarantee at all: a small derivative throws the iterate to
infinity, and a nearby inflection makes it cycle. Neither failure is
acceptable in a per-cell inversion where one bad cell out of a million stalls
the timestep. This solver therefore keeps the bracket that Newton lacks: it
takes the Newton step **only** when the step lands inside the current bracket
and is shrinking the interval fast enough, and bisects otherwise. The result
converges on anything a bisection would converge on, at Newton's rate
wherever Newton behaves.

# Arguments

- `problems` — one [`RootProblem`] per lane. Both the bracket **and** the
  `guess` are used: the bracket safeguards, the guess starts the iteration.
  A guess outside the bracket or non-finite is replaced by the midpoint.
- `settings` — tolerances and iteration cap. The abscissa tolerance is tested
  against the **step**, `|dx| <= x_tol_abs + x_tol_rel * |x|`.
- `backend` — requested execution backend; see [`root_batch_backend_for`].
- `fdf` — `fdf(i, x)` returns `(value, derivative)` of lane `i`'s residual at
  `x`. Returning both together is deliberate: an enthalpy and its specific
  heat come out of one polynomial evaluation, and asking for them separately
  would double the cost. Must be a pure deterministic function; see the
  module-level "Determinism" section.

# Returns

A [`RootBatch`] with one [`RootSolution`] per problem, in problem order.

# Determinism

Bit-for-bit identical across backends and thread counts for a pure `fdf`, on
the same terms as [`solve_bracketed_batch`].

# Non-convergence

As [`solve_bracketed_batch`], plus one Newton-specific case: a zero or
non-finite derivative is **not** a failure — the lane simply bisects that
iteration and carries on. Only a non-finite *value* stops the lane, with
[`RootStatus::NotFinite`].

# Verification

*Methodology.* Checked against the analytic root of `x^2 - k = 0`, i.e.
`sqrt(k)`, over 64 lanes with `k` spread across `[1, 65]`, and against the
three known roots of `(x - 1)(x - 2)(x - 3)`; pass criterion
`|x_computed - x_analytic| <= 1e-12` with `status == Converged` on every
lane. Separately checked against [`RootMethod::Brent`] on the same problems,
with the iteration counts of both recorded.

*Results, measured 2026-08-12 by `newton_matches_analytic_roots` and
`newton_beats_brent_on_iterations` in `parallel/tests.rs`, release build:*
worst absolute error over the 64 `sqrt(k)` lanes **8.881784e-16**; worst over
the three cubic roots **0.000000e0** (all three recovered exactly); mean
iterations **7.73** for safeguarded Newton against **13.14** for Brent on the
same 64 lanes. *Interpretation:* Newton reaches the same rounding-floor
accuracy — in fact 2x better than Brent on the `sqrt(k)` family — in 59% of
Brent's iterations, which is the expected quadratic-versus-superlinear margin
and is the reason to pay for a derivative. Note the counts include the
safeguard's bisection steps, which is why they are higher than a textbook
bare-Newton count on the same problem.

# Example — the `uom` boundary

The batch is dimensionless, and the caller converts at its edge. This lane
inverts a linear `h(T)` relation for a `ThermodynamicTemperature`, keeping
`uom` typing on both sides of the call:

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::math::parallel::{
    solve_newton_batch, RootProblem, RootSettings,
};
use uom::si::f64::{AvailableEnergy, ThermodynamicTemperature};
use uom::si::available_energy::joule_per_kilogram;
use uom::si::thermodynamic_temperature::kelvin;

// h(T) = cp * T with cp = 1000 J/(kg K); invert for two target enthalpies.
let cp = 1000.0_f64; // J / (kg K)
let targets = [
    AvailableEnergy::new::<joule_per_kilogram>(400_000.0),
    AvailableEnergy::new::<joule_per_kilogram>(900_000.0),
];

// Convert in: brackets in kelvin, as plain f64.
let lo = ThermodynamicTemperature::new::<kelvin>(200.0);
let hi = ThermodynamicTemperature::new::<kelvin>(3000.0);
let problems = [
    RootProblem::new(lo.get::<kelvin>(), hi.get::<kelvin>()),
    RootProblem::new(lo.get::<kelvin>(), hi.get::<kelvin>()),
];

let batch = solve_newton_batch(
    &problems,
    RootSettings::default(),
    ComputeBackend::CpuMulti,
    |i, t| (cp * t - targets[i].get::<joule_per_kilogram>(), cp),
);

// Convert out: back to a typed temperature.
let temperatures: Vec<ThermodynamicTemperature> = batch
    .roots()
    .expect("both lanes converge")
    .into_iter()
    .map(ThermodynamicTemperature::new::<kelvin>)
    .collect();

assert!((temperatures[0].get::<kelvin>() - 400.0).abs() < 1e-9);
assert!((temperatures[1].get::<kelvin>() - 900.0).abs() < 1e-9);
```

```rust
pub fn solve_newton_batch<F>(problems: &[RootProblem], settings: RootSettings, backend: crate::compute::ComputeBackend, fdf: F) -> RootBatch
where
    F: Fn(usize, f64) -> (f64, f64) + Sync { /* ... */ }
```

#### Function `linear_roots_batch`

**Attributes:**

- `MustUse { reason: None }`

Roots of `N` independent linear equations `a x + b = 0`, on the chosen
backend.

A batched wrapper over [`LinearEqn::roots`] — the per-equation closed-form
solver is unchanged and is not reimplemented here. Each lane is an
independent handful of floating-point operations, so this is the
branch-lightest kernel in the module.

# Arguments

- `eqns` — one [`LinearEqn`] per lane. Coefficients are dimensionless `f64`,
  as everywhere in [`crate::polynomial`].
- `backend` — requested execution backend; see [`poly_roots_backend_for`] for
  what will actually run.

# Returns

One [`Roots<1>`] per equation, in input order. Degenerate cases are tagged
rather than hidden: `|a| < VSMALL` yields a `Nan`-tagged root and an
overflowing quotient yields `PosInf`/`NegInf`, exactly as the scalar solver
does.

# Determinism

Bit-for-bit identical to calling [`LinearEqn::roots`] in a loop, on every
backend and at any thread count: each lane is one independent closed-form
evaluation.

# Example

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::math::parallel::linear_roots_batch;
use outram_foam_basic_lib::polynomial::{LinearEqn, RootType};

// 2x - 4 = 0  and  3x + 9 = 0
let eqns = [LinearEqn::new(2.0, -4.0), LinearEqn::new(3.0, 9.0)];
let roots = linear_roots_batch(&eqns, ComputeBackend::CpuMulti);

assert_eq!(roots[0].root_type(0), RootType::Real);
assert_eq!(roots[0].get(0), 2.0);
assert_eq!(roots[1].get(0), -3.0);
```

```rust
pub fn linear_roots_batch(eqns: &[crate::polynomial::LinearEqn], backend: crate::compute::ComputeBackend) -> Vec<crate::polynomial::Roots<1>> { /* ... */ }
```

#### Function `quadratic_roots_batch`

**Attributes:**

- `MustUse { reason: None }`

Roots of `N` independent quadratic equations `a x^2 + b x + c = 0`, on the
chosen backend.

A batched wrapper over [`QuadraticEqn::roots`], which uses a
Kahan-compensated discriminant; the per-equation solver is unchanged and is
not reimplemented here.

# Arguments

- `eqns` — one [`QuadraticEqn`] per lane, dimensionless coefficients.
- `backend` — requested execution backend.

# Returns

One [`Roots<2>`] per equation, in input order. Two `Real` roots when the
discriminant is positive; a `Complex`-tagged (real part, imaginary part) pair
when it is negative; one `Real` plus one `Nan` when `|a|` underflows and the
equation is really linear.

# Determinism

Bit-for-bit identical to calling [`QuadraticEqn::roots`] in a loop, on every
backend and at any thread count.

# Verification

*Methodology.* Every lane's real roots are substituted back into their own
equation and the residual `|a x^2 + b x + c|` compared against zero, over
4 096 equations built as `(x - r1)(x - r2)` with pseudorandom real roots on
`[-8, 8)` from a fixed-seed xorshift64\*; pass criterion: residual
`<= 1e-9` and bitwise equality against the per-equation scalar solver.

*Result, measured 2026-08-12 by `quadratic_batch_residuals_are_analytic` in
`parallel/tests.rs`, release build:* worst residual **7.105427e-15** over
8 192 real roots; bitwise equality against the scalar solver held on every
lane. *Interpretation:* the batching adds nothing to the scalar solver's own
error, which is what "bit-for-bit identical" means measured rather than
argued.

# Example

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::math::parallel::quadratic_roots_batch;
use outram_foam_basic_lib::polynomial::{QuadraticEqn, RootType};

// (x - 1)(x - 2) = x^2 - 3x + 2
let eqns = [QuadraticEqn::new(1.0, -3.0, 2.0)];
let roots = quadratic_roots_batch(&eqns, ComputeBackend::CpuMulti);

let mut real: Vec<f64> = (0..2)
    .filter(|&k| roots[0].root_type(k) == RootType::Real)
    .map(|k| roots[0].get(k))
    .collect();
real.sort_by(f64::total_cmp);
assert!((real[0] - 1.0).abs() < 1e-12);
assert!((real[1] - 2.0).abs() < 1e-12);
```

```rust
pub fn quadratic_roots_batch(eqns: &[crate::polynomial::QuadraticEqn], backend: crate::compute::ComputeBackend) -> Vec<crate::polynomial::Roots<2>> { /* ... */ }
```

#### Function `cubic_roots_batch`

**Attributes:**

- `MustUse { reason: None }`

Roots of `N` independent cubic equations `a x^3 + b x^2 + c x + d = 0`, on
the chosen backend.

A batched wrapper over [`CubicEqn::roots`] — the depressed-cubic Cardano
solver with Kahan-compensated discriminants, unchanged and not reimplemented
here.

This is the kernel a cubic-equation-of-state caller wants: a Peng-Robinson or
Redlich-Kwong compressibility factor is the root of a cubic in `Z`, one cubic
per cell, and the whole field can be solved in a single call.

# Arguments

- `eqns` — one [`CubicEqn`] per lane, dimensionless coefficients.
- `backend` — requested execution backend; see [`poly_roots_backend_for`].

# Returns

One [`Roots<3>`] per equation, in input order. Three `Real` roots, or one
`Real` plus a `Complex`-tagged (real, imaginary) pair, or a triple root, or —
when `|a|` underflows — the quadratic's roots plus a `Nan` slot. The tags are
the caller's guide; do not read a slot without checking
[`Roots::root_type`].

There is no `_into` variant, unlike [`crate::ldu_matrix::parallel`]'s
kernels: [`Roots`] has no `Default`, so a caller-owned output buffer would
have to be pre-filled with a dummy value, and the per-lane Cardano solve
dominates the allocation anyway.

# Determinism

Bit-for-bit identical to calling [`CubicEqn::roots`] in a loop, on every
backend and at any thread count. Each lane is one independent closed-form
evaluation; no arithmetic crosses lanes.

# Verification

*Methodology.* Cubics are constructed from **known** roots as
`(x - r1)(x - r2)(x - r3)` with `r1, r2, r3` drawn from a fixed-seed
xorshift64\* on `[-8, 8)`, expanded to coefficients — so the analytic answer
is known exactly, which makes this an oracle rather than a second opinion.
4 096 equations. Two pass criteria: every `Real`-tagged root satisfies
`|a x^3 + b x^2 + c x + d| <= 1e-6` (loose because a cubic's residual is
steeply amplified near a triple-ish root), and every returned bit is equal to
the per-equation scalar [`CubicEqn::roots`].

*Result, measured 2026-08-12 by `cubic_batch_matches_analytic_construction`
in `parallel/tests.rs`, release build:* worst residual **3.979039e-13** over
12 288 real roots, worst distance from the nearest constructed root
**2.079981e-10**; bitwise equality against the scalar solver held on every
lane, and (per `poly_batch_bitwise_identical_across_backends`) across
`Serial` / `CpuMulti` at 1, 2, 4 and 8 worker threads.

*Interpretation:* both figures belong to the scalar Cardano solver, not to
the batching — the bitwise-equality half of the test proves the batch adds
exactly zero error. That the worst *root displacement* (2.1e-10) is three
orders larger than the worst *residual* (4.0e-13) is the signature of an
ill-conditioned root, not of a bad solve: with roots drawn at random from
`[-8, 8)`, some triples come out nearly coincident, and near a near-double
root the cubic is flat, so a tiny residual still corresponds to a visibly
displaced abscissa. A caller solving a cubic equation of state should expect
the same behaviour near the critical point and judge accuracy by the
residual.

# Example

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::math::parallel::cubic_roots_batch;
use outram_foam_basic_lib::polynomial::{CubicEqn, RootType};

// (x-1)(x-2)(x-3) = x^3 - 6x^2 + 11x - 6
let eqns = [CubicEqn::new(1.0, -6.0, 11.0, -6.0)];
let roots = cubic_roots_batch(&eqns, ComputeBackend::CpuMulti);

let mut real: Vec<f64> = (0..3)
    .filter(|&k| roots[0].root_type(k) == RootType::Real)
    .map(|k| roots[0].get(k))
    .collect();
real.sort_by(f64::total_cmp);
assert_eq!(real.len(), 3);
assert!((real[0] - 1.0).abs() < 1e-9);
assert!((real[2] - 3.0).abs() < 1e-9);
```

```rust
pub fn cubic_roots_batch(eqns: &[crate::polynomial::CubicEqn], backend: crate::compute::ComputeBackend) -> Vec<crate::polynomial::Roots<3>> { /* ... */ }
```

### Constants and Statics

#### Constant `POLY_BLOCK`

Number of lanes in one task of the **closed-form polynomial** kernels
([`linear_roots_batch`], [`quadratic_roots_batch`], [`cubic_roots_batch`]).

`rayon` splits adaptively, so this is a *lower bound* on task granularity
rather than a fixed task size: it stops the splitter subdividing below a
block too small to pay for its own scheduling. A closed-form cubic solve is
on the order of tens of nanoseconds, so splitting down to a single equation
would spend more time in the scheduler than in Cardano's formula.

The iterative solvers deliberately do **not** use a floor — see the
module-level "Load imbalance" section.

This constant affects wall clock only. It cannot change any value, because
every lane is computed independently of every other.

# Units

A count of polynomial equations, dimensionless.

```rust
pub const POLY_BLOCK: usize = 256;
```

#### Constant `ROOT_BATCH_MIN_PROBLEMS`

Problem count below which a [`ComputeBackend::CpuMulti`] request runs the
**iterative** root-finding kernels ([`solve_bracketed_batch`],
[`solve_newton_batch`]) on the calling thread instead.

# Measured crossover

*Methodology.* Measured 2026-08-12 on this workspace's development machine,
`std::thread::available_parallelism()` = **4**, release build, `--features
parallel`, `rayon`'s global pool, otherwise-idle machine. Batches of `n`
independent problems `x^2 - k_i = 0` with `k_i` spread over `[1, 3)` so that
per-lane iteration counts differ, bracket `[0, 4]`, [`RootMethod::Brent`],
[`RootSettings::default`]. Best of 7 samples per point, reported as wall
clock for one whole batch. Produced by the `#[ignore]`d
`root_batch_crossover_benchmark` test in `parallel/tests.rs` and transcribed
from its printed output. The `cheap` closure is the two-flop residual above;
the `costly` closure adds a `ln`/`exp`/`sqrt` chain per evaluation, standing
in for a JANAF enthalpy. The serial columns are from the first run; the
`(repeat)` columns are the speed-ups from a second, independent run of the
same benchmark, carried alongside rather than averaged away because the
parallel column is far noisier than the serial one.

| Problems | cheap serial | cheap multi | speed-up | (repeat) | costly serial | costly multi | speed-up | (repeat) |
|---|---|---|---|---|---|---|---|---|
| 16 | 2.87 us | 34.77 us | 0.08x | 0.07x | 11.21 us | 42.19 us | 0.27x | 0.93x |
| 32 | 6.67 us | 36.60 us | 0.18x | 0.71x | 19.32 us | 43.55 us | 0.44x | 1.35x |
| 64 | 12.99 us | 45.16 us | 0.29x | 0.29x | 42.85 us | 62.99 us | 0.68x | 0.83x |
| 128 | 25.96 us | 57.24 us | 0.45x | 0.59x | 84.88 us | 86.65 us | 0.98x | 1.26x |
| 256 | 52.94 us | 42.79 us | 1.24x | 2.28x | 175.37 us | 126.70 us | 1.38x | 3.18x |
| 512 | 108.80 us | 68.63 us | 1.59x | 2.91x | 356.78 us | 170.71 us | 2.09x | 2.04x |
| 1 024 | 201.83 us | 127.52 us | 1.58x | 2.01x | 695.26 us | 284.70 us | 2.44x | 2.17x |
| 4 096 | 805.49 us | 501.42 us | 1.61x | 3.63x | 2774.06 us | 992.73 us | 2.79x | 3.58x |
| 16 384 | 3338.49 us | 1173.15 us | 2.85x | 2.71x | 11423.08 us | 3916.59 us | 2.92x | 2.83x |
| 65 536 | 13730.89 us | 5071.05 us | 2.71x | 2.94x | 45778.24 us | 16260.96 us | 2.82x | 3.02x |

*Result.* **256** is the smallest size at which *neither* closure lost in
*either* run — cheap 1.24x and 2.28x, costly 1.38x and 3.18x — and it is the
value this constant takes. At 128 the cheap closure lost both runs (0.45x,
0.59x) while the costly one was a coin flip (0.98x, 1.26x), which is the
expected shape: a more expensive residual crosses over earlier because there
is more work per lane to amortise the same dispatch cost. The honest headline
is therefore that **the crossover depends on the caller's residual**, and
this single number is set for the cheapest residual a caller is likely to
pass, so that it is safe for both.

*Interpretation.* This is 16x **lower** than the crate-wide placeholder
[`crate::compute::CPU_MULTI_MIN_WORK_ITEMS`] (4 096) and 512x lower than
[`crate::fields::parallel::FIELD_PARALLEL_CROSSOVER`] (131 072), and the
reason is structural rather than incidental. Field algebra does one or two
flops per element loaded and is memory-bandwidth bound, so extra cores buy
almost nothing; a root solve does tens of iterations of arithmetic per lane
against a handful of loaded bytes, so it is compute bound and threads scale
on it. Keeping the crate-wide placeholder here would forfeit a measured
1.6x-2.4x on batches of 1 024.

# Relationship to [`crate::compute::CPU_MULTI_MIN_WORK_ITEMS`]

That constant documents itself as an unmeasured placeholder and invites
per-kernel overrides. This is that override for the iterative root finders.
It is now the fourth measured value in the crate, and the spread across the
four (**256**, 4 096, 131 072, 262 144 — a factor of 1 024) is further
evidence that no single crate-wide threshold can be right. Offered as input
to bead `op-yvj.4.7`.

# Limitations

One machine, four logical cores, one closure family, idle. Not measured on
Android/Termux hardware and not on a many-core server. The crossover moves
with the caller's residual cost, so a caller that knows its own closure is
expensive is better off calling the kernel with an explicit
[`ComputeBackend`] than trusting a single number.

# Units

A count of independent root problems, dimensionless.

```rust
pub const ROOT_BATCH_MIN_PROBLEMS: usize = 256;
```

#### Constant `POLY_ROOTS_MIN_EQUATIONS`

Equation count below which a [`ComputeBackend::CpuMulti`] request runs the
**closed-form polynomial** kernels on the calling thread instead.

# Measured crossover

*Methodology.* Same machine, build and conditions as
[`ROOT_BATCH_MIN_PROBLEMS`] (4 logical cores, release, `--features
parallel`, idle, best of 7 samples). Batches of `n` cubics
`(x - r1)(x - r2)(x - r3)` with pseudorandom real roots, solved by
[`cubic_roots_batch`]; timed as wall clock for one whole batch. Produced by
the `#[ignore]`d `poly_batch_crossover_benchmark` test in `parallel/tests.rs`
and transcribed from its printed output. The `(repeat)` column is the
speed-up from a second, independent run.

| Cubics | serial | CpuMulti | speed-up | (repeat) |
|---|---|---|---|---|
| 256 | 36.34 us | 36.43 us | 1.00x | 1.00x |
| 512 | 71.92 us | 110.25 us | 0.65x | 0.83x |
| 1 024 | 146.16 us | 96.48 us | 1.51x | 1.82x |
| 2 048 | 296.38 us | 189.08 us | 1.57x | 1.37x |
| 4 096 | 591.18 us | 170.46 us | 3.47x | 2.38x |
| 16 384 | 2377.28 us | 832.47 us | 2.86x | 1.90x |
| 65 536 | 10024.12 us | 3432.25 us | 2.92x | 2.76x |
| 262 144 | 40089.01 us | 11204.91 us | 3.58x | 2.87x |

*Result.* Set to **1 024**, the smallest measured size at which the parallel
path won in both runs (1.51x, 1.82x). At 512 it lost both (0.65x, 0.83x).

*Interpretation.* Note the 256-cubic row, where the two paths are *exactly*
break-even in both runs: [`POLY_BLOCK`] is 256, so there is a single task and
`rayon` never splits — the same artefact the `ldu_matrix` and `fields`
crossover tables show at their own block sizes, and a useful confirmation
that the granularity floor is doing what it claims.

This floor is 4x the iterative one, but **do not read that as a strong
structural difference**: the two kernels' per-lane costs are in fact
comparable. Dividing the largest serial timings by their lane counts gives
about **153 ns** per closed-form cubic against about **209 ns** per cheap
iterative lane. Both break even in the same shallow region, and the 4x gap
between the two constants reflects a conservative reading of a noisy
measurement — taking the smallest size that won in *both* runs — more than it
reflects physics. Both are still far below the crate-wide placeholder,
because both kernels are compute-dense next to a field add.

# Limitations

As [`ROOT_BATCH_MIN_PROBLEMS`]: one machine, four cores, idle, not measured
on Android. Measured with [`cubic_roots_batch`] only —
[`quadratic_roots_batch`] and [`linear_roots_batch`] are cheaper per lane and
so cross over *later*, and inherit this floor as an assumption rather than a
measurement.

# Units

A count of polynomial equations, dimensionless.

```rust
pub const POLY_ROOTS_MIN_EQUATIONS: usize = 1_024;
```

### Re-exports

#### Re-export `derivative`

```rust
pub use differentiate::derivative;
```

#### Re-export `derivative_backend_for`

```rust
pub use differentiate::derivative_backend_for;
```

#### Re-export `derivative_batch`

```rust
pub use differentiate::derivative_batch;
```

#### Re-export `jacobian`

```rust
pub use differentiate::jacobian;
```

#### Re-export `jacobian_batch`

```rust
pub use differentiate::jacobian_batch;
```

#### Re-export `jacobian_batch_backend_for`

```rust
pub use differentiate::jacobian_batch_backend_for;
```

#### Re-export `jacobian_column_backend_for`

```rust
pub use differentiate::jacobian_column_backend_for;
```

#### Re-export `ode_system_jacobian`

```rust
pub use differentiate::ode_system_jacobian;
```

#### Re-export `DerivativeBatch`

```rust
pub use differentiate::DerivativeBatch;
```

#### Re-export `DerivativeSolution`

```rust
pub use differentiate::DerivativeSolution;
```

#### Re-export `DiffBatchFailure`

```rust
pub use differentiate::DiffBatchFailure;
```

#### Re-export `DiffScheme`

```rust
pub use differentiate::DiffScheme;
```

#### Re-export `DiffSettings`

```rust
pub use differentiate::DiffSettings;
```

#### Re-export `DiffStatus`

```rust
pub use differentiate::DiffStatus;
```

#### Re-export `JacobianBatch`

```rust
pub use differentiate::JacobianBatch;
```

#### Re-export `JacobianSolution`

```rust
pub use differentiate::JacobianSolution;
```

#### Re-export `NumericalJacobian`

```rust
pub use differentiate::NumericalJacobian;
```

#### Re-export `CBRT_EPSILON`

```rust
pub use differentiate::CBRT_EPSILON;
```

#### Re-export `DERIVATIVE_BATCH_MIN_POINTS`

```rust
pub use differentiate::DERIVATIVE_BATCH_MIN_POINTS;
```

#### Re-export `FIFTH_ROOT_EPSILON`

```rust
pub use differentiate::FIFTH_ROOT_EPSILON;
```

#### Re-export `JACOBIAN_BATCH_MIN_PROBLEMS`

```rust
pub use differentiate::JACOBIAN_BATCH_MIN_PROBLEMS;
```

#### Re-export `JACOBIAN_COLUMN_MIN_DIMENSION`

```rust
pub use differentiate::JACOBIAN_COLUMN_MIN_DIMENSION;
```

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

#### Re-export `golden_section_batch`

```rust
pub use minimise::golden_section_batch;
```

#### Re-export `minimise_backend_for`

```rust
pub use minimise::minimise_backend_for;
```

#### Re-export `MinBatch`

```rust
pub use minimise::MinBatch;
```

#### Re-export `MinBatchFailure`

```rust
pub use minimise::MinBatchFailure;
```

#### Re-export `MinProblem`

```rust
pub use minimise::MinProblem;
```

#### Re-export `MinSettings`

```rust
pub use minimise::MinSettings;
```

#### Re-export `MinSolution`

```rust
pub use minimise::MinSolution;
```

#### Re-export `MinStatus`

```rust
pub use minimise::MinStatus;
```

#### Re-export `Sense`

```rust
pub use minimise::Sense;
```

#### Re-export `GOLDEN_RATIO`

```rust
pub use minimise::GOLDEN_RATIO;
```

#### Re-export `MINIMISE_BATCH_MIN_PROBLEMS`

```rust
pub use minimise::MINIMISE_BATCH_MIN_PROBLEMS;
```

#### Re-export `SQRT_EPSILON`

```rust
pub use minimise::SQRT_EPSILON;
```

#### Re-export `cubic_roots_batch`

```rust
pub use parallel::cubic_roots_batch;
```

#### Re-export `linear_roots_batch`

```rust
pub use parallel::linear_roots_batch;
```

#### Re-export `poly_roots_backend_for`

```rust
pub use parallel::poly_roots_backend_for;
```

#### Re-export `quadratic_roots_batch`

```rust
pub use parallel::quadratic_roots_batch;
```

#### Re-export `root_batch_backend_for`

```rust
pub use parallel::root_batch_backend_for;
```

#### Re-export `solve_bracketed_batch`

```rust
pub use parallel::solve_bracketed_batch;
```

#### Re-export `solve_newton_batch`

```rust
pub use parallel::solve_newton_batch;
```

#### Re-export `RootBatch`

```rust
pub use parallel::RootBatch;
```

#### Re-export `RootBatchFailure`

```rust
pub use parallel::RootBatchFailure;
```

#### Re-export `RootMethod`

```rust
pub use parallel::RootMethod;
```

#### Re-export `RootProblem`

```rust
pub use parallel::RootProblem;
```

#### Re-export `RootSettings`

```rust
pub use parallel::RootSettings;
```

#### Re-export `RootSolution`

```rust
pub use parallel::RootSolution;
```

#### Re-export `RootStatus`

```rust
pub use parallel::RootStatus;
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
## Module `parallel`

Batched **numerical integration** on the hybrid execution backend — `N`
independent initial-value problems, or `N` independent definite integrals,
advanced at once, serially or across CPU cores.

# The two things in here, and why they share a module

| Operation | The batch is | Entry point |
|---|---|---|
| ODE ensemble, one stepper for every lane | `N` independent IVPs | [`integrate_ensemble`] |
| ODE ensemble, stepper chosen per lane | `N` independent IVPs | [`integrate_ensemble_mixed`] |
| Fixed-rule quadrature | `N` independent definite integrals | [`quadrature_batch`] |
| Adaptive quadrature | `N` independent definite integrals | [`adaptive_quadrature_batch`] |

Both halves are "numerical integration" in the sense bead `op-yvj.4.5` uses
the phrase, and a definite integral genuinely *is* the initial-value problem
`dy/dx = f(x)`, `y(a) = 0`, read at `x = b` — so quadrature sitting beside the
ODE steppers is not a filing accident. They also share every piece of
machinery that is not the arithmetic itself: the lane vocabulary, the
per-lane status reporting, the backend-degradation policy, and the
determinism argument below. Keeping them together means the crate has **one**
batched-integration dialect rather than two.

# Reuse, not reimplementation

**No new integrator is written here.** The ODE half drives the crate's
existing steppers — [`Euler`](crate::ode::Euler),
[`Rkf45`](crate::ode::Rkf45), [`Rosenbrock23`](crate::ode::Rosenbrock23),
selected through the existing [`OdeSolver`] enum — over the existing
adaptive interval loop. This module adds the *ensemble*: the outer loop over
lanes, the per-lane outcome reporting, and the backend dispatch.

The quadrature half is new code, because the workspace had no general
quadrature. The only prior art is
`outram-park-fork-dwsim-libs`'s `clean_energies::pem_fuel_cell::simpson_integrate`,
which is deliberately **not** reused: it is a verbatim port of OPEM's rule
*including its documented flaw* (composite Simpson weights applied without
checking that the sample count is odd), and it integrates a slice of
pre-sampled values rather than a callable integrand. [`QuadratureRule`]
keeps the one convention that does carry over — composite rules over equal
subintervals — and makes the sample-count error structurally impossible by
counting Simpson *panels* of two subintervals each rather than raw samples.
`raffles`'s `distributions::special::integrate_open_unit` is the other
in-workspace quadrature: a composite 8-point Gauss-Legendre over
geometrically graded panels, hard-wired to the open unit interval and to
quantile-function moments. It is not a general routine and is private to that
crate's `special` module, so it is cited here as precedent for the
Gauss-Legendre choice rather than reused.

# Hybrid means dispatch, not two APIs

Every entry point takes a [`ComputeBackend`] parameter, and there is no
`*_parallel()` sibling anywhere. With the `parallel` feature off,
[`ComputeBackend::CpuMulti`] resolves down to [`ComputeBackend::Serial`]
through [`ComputeBackend::resolve`] and the answer is unchanged, bit for bit.
There is no GPU kernel in this module yet, so a [`ComputeBackend::Gpu`]
request degrades to the best available CPU path — see "GPU" below for why
that is not merely laziness for the adaptive paths.

# Determinism — bitwise identical, and the summation-order question

**Every kernel in this module returns bit-for-bit identical output on
[`ComputeBackend::Serial`] and [`ComputeBackend::CpuMulti`], at any thread
count, on every run**, provided the caller's system or integrand is itself a
deterministic pure function of its arguments.

For the ODE ensemble this is the same argument as
[`crate::math::parallel`]'s: lane `i`'s trajectory is a pure function of lane
`i`'s system, initial condition and stepper. No lane reads another lane's
state, so there is no cross-lane arithmetic whose association could change,
and both backends call the very same per-lane kernel. Each lane also gets its
**own** clone of the stepper prototype, so no scratch buffer is ever shared
between lanes and the result cannot depend on which lanes a worker happened
to run first.

Quadrature needs one extra sentence, because a quadrature rule *is* a sum and
floating-point addition is not associative. The reason the answer is still
bit-identical is the shape of the batch: **one lane is one integral, and one
integral is summed sequentially by a single thread.** The parallelism is over
lanes, never within a lane, so no partial sum is ever re-associated. This is
a deliberate design choice with a real cost — it means a single very
expensive integral gets no speed-up from this module at all — and it is taken
because a reduction split across threads would give a different answer at
every thread count, which for a verification oracle is disqualifying.
**Splitting one integral's panels across threads is not offered here**, and
if it is ever added it must be a separate, separately-named entry point whose
documentation says plainly that it is not bit-reproducible.

Verified by the `bitwise_*` tests in `parallel/tests.rs`, which compare
serial against `rayon` pools of 1, 2, 4 and 8 workers on batches built to
have wildly uneven per-lane cost.

For scale, a 4 096-lane deliberately-imbalanced ensemble (half the lanes
decaying at `k` near 1, half at `k` near 60, giving 278 318 accepted steps in
total — a mean of 67.9 per lane against a maximum of 126) under `Rkf45`, measured
2026-08-13 on 4 logical cores by the `#[ignore]`d
`ensemble_thread_scaling_benchmark`, best of 7 samples, with a second
independent run alongside:

| Worker threads | Time | Speed-up | (repeat) | Bitwise vs serial |
|---|---|---|---|---|
| *serial reference* | 35808.33 us | 1.00x | 1.00x | — |
| 1 | 35839.25 us | 1.00x | 1.00x | identical |
| 2 | 18283.82 us | 1.96x | 1.92x | identical |
| 4 | 9276.65 us | 3.86x | 3.09x | identical |
| 8 | 9128.96 us | 3.92x | 3.05x | identical |

The "identical" column is the determinism claim above measured rather than
argued, and it is asserted by the benchmark itself, not merely printed. Going
through `rayon` with a single worker costs essentially nothing here (1.00x),
unlike the batched root finder where it costs about 6% — the per-lane work is
large enough that the iterator machinery disappears into it. Eight workers on
four cores buy nothing further, the expected signature of a compute-bound
kernel that already saturates its cores. **This is one machine, one ensemble
and two runs; it is not a scaling study**, and nothing here has been measured
on Android hardware or on a many-core server.

# Load imbalance — why there is no hand-rolled partition

An adaptive stepper takes a different number of sub-steps in every lane, and
the spread is not small. On the benchmark ensemble below it is a mean of 67.9
accepted steps against a maximum of 126; on the stiff pair in
`parallel/tests.rs` a single lane under [`Rkf45`](crate::ode::Rkf45) burns
all 10 000 of its allotted steps and still spans only a third of the
interval, beside decay lanes finishing in 81. Adaptive quadrature has the
same property by construction — 469 to 1 225 integrand evaluations across the
three verification integrands. A static equal split across `P` threads
therefore ends up waiting on whichever chunk drew the hard lanes.

Every parallel path here uses `rayon`'s adaptive splitting with **no**
`min_len` floor, so an idle worker can steal down to a single lane. That is
the deliberate answer to the imbalance, not an oversight. No granularity
floor is imposed even on the fixed-rule quadrature path, where the crate's
closed-form polynomial kernels do impose one
([`crate::math::parallel::POLY_BLOCK`]): there, every lane provably costs the
same handful of flops, whereas here the per-lane cost is set by the caller
through [`QuadratureRule`]'s panel count *and* by the cost of the caller's
integrand, so any fixed floor would be wrong for most callers. Work-stealing
handles both ends without a number that cannot be justified.

Whatever the splitter does, it cannot change a value — every lane is computed
independently of every other.

# Stiffness — how a mixed ensemble is handled

The realistic per-cell chemistry ensemble is *mixed*: most cells are benign
and a few are stiff. Three things follow, and all three are deliberate.

1. **The stepper is not switched behind the caller's back.** An ensemble run
   with [`integrate_ensemble`] uses one stepper for every lane. A stiff lane
   handed an explicit stepper does not silently return a wrong answer: the
   adaptive controller shrinks the step until it either meets tolerance —
   correct but slow — or runs out, at which point the lane reports
   [`OdeLaneStatus::MaxStepsExceeded`] or
   [`OdeLaneStatus::StepSizeUnderflow`]. Stiffness therefore shows up as a
   *named per-lane failure*, not as silent garbage.
2. **Per-lane stepper selection exists** — [`integrate_ensemble_mixed`] takes
   a closure `Fn(usize) -> OdeSolver`, so a caller that knows which cells are
   stiff (or that has just been told by a failed
   [`integrate_ensemble`] pass) can give those lanes
   [`Rosenbrock23`](crate::ode::Rosenbrock23) and leave the rest on
   [`Rkf45`](crate::ode::Rkf45). This is the intended recovery path and it
   costs nothing when unused.
3. **`Rosenbrock23` needs a Jacobian, and this module does not supply one.**
   [`OdeSystem::jacobian`](crate::ode::OdeSystem::jacobian)'s default
   implementation panics, and a panic inside a `rayon` worker will propagate
   out of the batch. Check
   [`OdeSolver::requires_jacobian`](crate::ode::OdeSolver::requires_jacobian)
   before selecting it for a system you did not write. Batched *numerical*
   Jacobians are bead `op-yvj.4.6`, not this one.

# Non-convergence is reported, never swallowed

An ensemble of 10 000 cells in which 3 fail must say so, and must say which
3. Both halves report failure **per lane**, and make it hard to ignore by
construction — the same shape [`crate::math::parallel`] uses:

- [`OdeLaneSolution::state`] and [`QuadratureSolution::value`] return
  `Option`, and hand back `Some` **only** for a lane that succeeded.
- The raw number is behind the deliberately-named
  [`OdeLaneSolution::last_state`] / [`QuadratureSolution::last_value`], so
  using a failed lane's partial answer is a visible decision in the calling
  code rather than an accident.
- [`OdeEnsemble::states`] and [`QuadratureBatch::values`] are all-or-nothing:
  they return `Err` naming the failure count and the first failing lane,
  rather than a plausible-looking `Vec`.

**A lane that ran out of steps is never presented as if it had reached
`x_end`.** It reports [`OdeLaneStatus::MaxStepsExceeded`], its genuine
partial state, and the `x` it actually reached — which is exactly the
information needed to decide whether to sub-cycle it or re-run it stiff. A
lane whose inputs were unusable reports [`OdeLaneStatus::InvalidLane`] and a
`NaN` state, because there is no honest number to return.

# GPU

There is no `wgpu` kernel here yet; `Gpu` degrades. Two of these four
kernels would map to one if written, and two would not, and the distinction
is worth recording rather than rediscovering:

- **Fixed-rule quadrature is GPU-shaped.** Every lane evaluates the same
  fixed number of nodes with no data-dependent branching, so a warp never
  diverges.
- **Adaptive quadrature is not, and is CPU-only by design.** Its subdivision
  pattern is decided by the integrand, so neighbouring lanes take different
  paths through the code and different numbers of evaluations; on SIMT
  hardware that serialises the divergent branches and needs a per-lane
  work stack. It stays on the CPU deliberately.
- **Adaptive ODE ensembles have the same problem, sharpened.** A batch run
  lockstep at the smallest step any lane needs is simple and correct but
  wastes the whole ensemble's budget on its worst member; per-lane step
  control is correct but divergent. Neither is attempted here.

Note also that WGSL has no `f64`, and the accuracy consequences of an `f32`
quadrature or an `f32` error controller are unmeasured in this workspace.

# Units

Everything here is dimensionless `f64`, for the same reason
[`crate::math::parallel`] is: a general integrator has no single physical
dimension. One lane's abscissa may be a time in seconds and another's a
length in metres, and the state vector's components routinely carry different
dimensions from each other.

`uom` typing is **not stripped** to get here — it is applied at the boundary,
by the caller, exactly as the hybrid-backend epic requires: convert into the
batch, convert back out. The doctests on [`integrate_ensemble`] and
[`quadrature_batch`] show that boundary explicitly, one recovering a
`ThermodynamicTemperature` and the other an `Energy`.

# Cargo features and portability

The `rayon` paths sit behind the crate's `parallel` feature, which is **off
by default**; with it off this module still compiles and every entry point
still works. `rayon` is pure Rust with no system component, so everything
here compiles and runs on `aarch64-linux-android` / Termux exactly as on
desktop. Nothing in this module is target-gated.

# Example — an ensemble of independent decays

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::ode::{OdeSolver, OdeSystem};
use outram_foam_basic_lib::ode::parallel::{integrate_ensemble, OdeLane};

/// `dy/dx = -k y` — one lane per decay constant.
struct Decay { k: f64 }
impl OdeSystem for Decay {
    fn n_eqns(&self) -> usize { 1 }
    fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
        dydx[0] = -self.k * y[0];
    }
}

let lanes: Vec<OdeLane<Decay>> = (1..=4)
    .map(|i| OdeLane::new(Decay { k: i as f64 }, vec![1.0], 0.0, 1.0, 0.1))
    .collect();

let ensemble = integrate_ensemble(
    &lanes,
    &OdeSolver::rkf45(1, 1e-10, 1e-8),
    ComputeBackend::CpuMulti,
);

let states = ensemble.states().expect("all four lanes complete");
for (i, s) in states.iter().enumerate() {
    let exact = (-(i as f64 + 1.0)).exp();
    assert!((s[0] - exact).abs() < 1e-8, "lane {i}: {} vs {exact}", s[0]);
}

// Asking for multi-CPU gives a bit-for-bit identical answer, whether or not
// the `parallel` feature is compiled in.
let serial = integrate_ensemble(
    &lanes,
    &OdeSolver::rkf45(1, 1e-10, 1e-8),
    ComputeBackend::Serial,
);
assert_eq!(states, serial.states().unwrap());
```

```rust
pub mod parallel { /* ... */ }
```

### Types

#### Struct `OdeLane`

One member of an ODE ensemble: a system, its initial condition, and the
interval to advance it over.

The system is owned **by value**, so an ensemble is a plain
`Vec<OdeLane<S>>` with no lifetime parameter anywhere and no possibility of
the system slice and the initial-condition slice disagreeing in length. `S`
may be any concrete type implementing [`OdeSystem`], including the caller's
own enum over several systems — which is how one ensemble holds genuinely
different physics in different lanes.

# Units

`x_start`, `x_end` and `dx0` are in the independent variable's units
(typically seconds); `y0`'s components are in whatever units that lane's
state carries, which need not be the same as each other. All are the
caller's own units — see the module-level "Units" section.

# Validity

A lane is rejected with [`OdeLaneStatus::InvalidLane`], before any
integration, unless all of the following hold:

- `y0.len() == system.n_eqns()`
- `x_start`, `x_end` and every component of `y0` are finite
- `dx0` is finite and strictly positive
- `x_end >= x_start` — **the underlying interval loop integrates forwards
  only**, so a reversed interval is a caller error rather than a backwards
  integration

`x_end == x_start` is legal and is a no-op: the lane completes in zero steps
with its state unchanged.

```rust
pub struct OdeLane<S: OdeSystem> {
    pub system: S,
    pub y0: Vec<f64>,
    pub x_start: f64,
    pub x_end: f64,
    pub dx0: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `system` | `S` | The system this lane integrates, owned outright. |
| `y0` | `Vec<f64>` | Initial state at `x_start`, one entry per equation. |
| `x_start` | `f64` | Start of the integration interval. |
| `x_end` | `f64` | End of the integration interval; must be `>= x_start`. |
| `dx0` | `f64` | First step size to attempt. Must be finite and `> 0`; the adaptive<br>controller adjusts it from there, so it is a starting guess and not a<br>constraint. |

##### Implementations

###### Methods

- ```rust
  pub fn new(system: S, y0: Vec<f64>, x_start: f64, x_end: f64, dx0: f64) -> Self { /* ... */ }
  ```
  Build one ensemble lane.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> OdeLane<S> { /* ... */ }
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
    fn eq(self: &Self, other: &OdeLane<S>) -> bool { /* ... */ }
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
#### Enum `OdeLaneStatus`

How one lane of an ODE ensemble ended.

Only [`Completed`](Self::Completed) means the lane reached `x_end`. Every
other variant is a failure a caller must handle; see the module-level
"Non-convergence is reported, never swallowed" section for the accessors that
make it hard to skip.

| Variant | `last_state` | Meaning |
|---|---|---|
| [`Completed`](Self::Completed) | the state at `x_end` | reached the end of the interval |
| [`MaxStepsExceeded`](Self::MaxStepsExceeded) | genuine partial state at `x_reached` — **not** the answer | ran out of sub-steps |
| [`StepSizeUnderflow`](Self::StepSizeUnderflow) | genuine partial state at `x_reached` | the step shrank below `f64::EPSILON` |
| [`NotFinite`](Self::NotFinite) | the non-finite state | the state went `NaN`/infinite |
| [`InvalidLane`](Self::InvalidLane) | all `NaN` | the lane's inputs were unusable |

The two middle variants are the *stiffness signature*: an explicit stepper on
a stiff lane runs out of budget rather than returning a wrong answer.

# Units

Dimensionless — a status tag.

```rust
pub enum OdeLaneStatus {
    Completed,
    MaxStepsExceeded,
    StepSizeUnderflow,
    NotFinite,
    InvalidLane,
}
```

##### Variants

###### `Completed`

The lane reached `x_end` with a finite state.

###### `MaxStepsExceeded`

`OdeSolverConfig::max_steps` sub-steps were taken without spanning the
interval. The reported state is genuine but partial, at
[`OdeLaneSolution::x_reached`].

###### `StepSizeUnderflow`

The adaptive controller shrank the step below `f64::EPSILON` trying to
meet the tolerance — the system is too stiff for the chosen stepper, or
the tolerances are unattainable. Maps to
[`OdeError::StepSizeUnderflow`](crate::ode::OdeError::StepSizeUnderflow).

###### `NotFinite`

The integration returned success but the final state contains a `NaN` or
an infinity. Reported separately from the two budget failures because it
means the *model* blew up, not that the stepper ran out of room.

###### `InvalidLane`

The lane was rejected before any integration — see [`OdeLane`]'s
"Validity" section for the exact conditions. The state is all `NaN`,
because returning the untouched initial condition would look like a
zero-length integration that succeeded.

##### Implementations

###### Methods

- ```rust
  pub fn is_completed(self: Self) -> bool { /* ... */ }
  ```
  Whether this status means the lane produced a usable final state.

- ```rust
  pub fn label(self: Self) -> &'static str { /* ... */ }
  ```
  A short human-readable label, for log lines and failure reports.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> OdeLaneStatus { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &OdeLaneStatus) -> bool { /* ... */ }
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
#### Struct `OdeLaneSolution`

The outcome of a single ensemble lane: its status, its final state, how far
it got and how much work it took.

The fields are private on purpose. The only way to get a state that is
claimed to be the answer is [`Self::state`], which returns `Option<&[f64]>`
and hands back `Some` only for a completed lane. The raw state is available
from [`Self::last_state`], whose name is chosen so that using a failed lane's
partial trajectory is a visible decision in the calling code.

# Units

[`Self::state`] and [`Self::last_state`] carry the lane's own state units;
[`Self::x_reached`] and [`Self::dx_final`] are in the independent variable's
units; [`Self::steps`] is a dimensionless count.

```rust
pub struct OdeLaneSolution {
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
  pub fn state(self: &Self) -> Option<&[f64]> { /* ... */ }
  ```
  The final state, if this lane reached `x_end`.

- ```rust
  pub fn last_state(self: &Self) -> &[f64] { /* ... */ }
  ```
  The last state the stepper held, completed or not.

- ```rust
  pub fn x_reached(self: &Self) -> f64 { /* ... */ }
  ```
  The independent-variable value the trajectory actually reached.

- ```rust
  pub fn dx_final(self: &Self) -> f64 { /* ... */ }
  ```
  The step size the adaptive controller ended on, in the independent

- ```rust
  pub fn steps(self: &Self) -> u32 { /* ... */ }
  ```
  Accepted sub-steps this lane took, dimensionless.

- ```rust
  pub fn status(self: &Self) -> OdeLaneStatus { /* ... */ }
  ```
  How this lane ended.

- ```rust
  pub fn completed(self: &Self) -> bool { /* ... */ }
  ```
  Whether this lane reached `x_end`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> OdeLaneSolution { /* ... */ }
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
    fn eq(self: &Self, other: &OdeLaneSolution) -> bool { /* ... */ }
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
#### Struct `OdeEnsemble`

A batch of `N` lane outcomes, in the same order as the lanes handed in.

Lane `i` of the result corresponds to `lanes[i]`, always — the parallel path
preserves order, so no index bookkeeping is needed.

# Getting states out

- [`Self::states`] / [`Self::into_states`] — all-or-nothing. `Ok` only when
  every lane completed; otherwise `Err(`[`OdeEnsembleFailure`]`)`.
- [`Self::lanes`] — per-lane, when the caller wants to handle failures
  individually (re-run stiff, sub-cycle, flag the cell).

# Units

See [`OdeLaneSolution`].

```rust
pub struct OdeEnsemble {
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
  pub fn lanes(self: &Self) -> &[OdeLaneSolution] { /* ... */ }
  ```
  Every lane's outcome, in input order.

- ```rust
  pub fn into_lanes(self: Self) -> Vec<OdeLaneSolution> { /* ... */ }
  ```
  Consume the ensemble and take the outcomes.

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Number of lanes, dimensionless.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether the ensemble has no lanes.

- ```rust
  pub fn get(self: &Self, i: usize) -> Option<&OdeLaneSolution> { /* ... */ }
  ```
  Lane `i`'s outcome, or `None` if `i` is out of range.

- ```rust
  pub fn all_completed(self: &Self) -> bool { /* ... */ }
  ```
  Whether every lane completed. Vacuously `true` for an empty ensemble.

- ```rust
  pub fn failure_count(self: &Self) -> usize { /* ... */ }
  ```
  How many lanes failed, dimensionless.

- ```rust
  pub fn first_failure(self: &Self) -> Option<(usize, &OdeLaneSolution)> { /* ... */ }
  ```
  The first failing lane's index and outcome, if any.

- ```rust
  pub fn failures(self: &Self) -> Vec<(usize, &OdeLaneSolution)> { /* ... */ }
  ```
  Every failing lane, as `(index, outcome)` pairs.

- ```rust
  pub fn total_steps(self: &Self) -> u64 { /* ... */ }
  ```
  Total accepted sub-steps over every lane, dimensionless.

- ```rust
  pub fn max_steps_taken(self: &Self) -> u32 { /* ... */ }
  ```
  The largest accepted-sub-step count over all lanes, dimensionless; `0`

- ```rust
  pub fn states(self: &Self) -> Result<Vec<Vec<f64>>, OdeEnsembleFailure> { /* ... */ }
  ```
  Every lane's final state, or an error naming the failures — the

- ```rust
  pub fn into_states(self: Self) -> Result<Vec<Vec<f64>>, OdeEnsembleFailure> { /* ... */ }
  ```
  [`Self::states`] without the clone — consumes the ensemble.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> OdeEnsemble { /* ... */ }
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
    fn eq(self: &Self, other: &OdeEnsemble) -> bool { /* ... */ }
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
#### Struct `OdeEnsembleFailure`

**Attributes:**

- `Other("#[error(\"{failure_count} of {total} ODE lanes did not complete; \\\n     first failure at lane {first_index} with status {first_status:?} \\\n     after {first_steps} steps, reaching x = {first_x_reached}\")]")`

One or more lanes of an [`OdeEnsemble`] did not complete.

Returned by [`OdeEnsemble::states`]. It names both the scale of the problem
(how many of how many) and a specific lane to look at, because "3 of 10 000
cells failed" is only actionable once you know *which* cell and *how far* it
got.

# Units

Counts and indices are dimensionless; `first_x_reached` is in the independent
variable's units.

```rust
pub struct OdeEnsembleFailure {
    pub total: usize,
    pub failure_count: usize,
    pub first_index: usize,
    pub first_status: OdeLaneStatus,
    pub first_steps: u32,
    pub first_x_reached: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `total` | `usize` | Number of lanes in the ensemble. |
| `failure_count` | `usize` | Number of lanes that did not complete. |
| `first_index` | `usize` | Index of the first failing lane. |
| `first_status` | `OdeLaneStatus` | Why that lane failed. |
| `first_steps` | `u32` | Accepted sub-steps that lane took before giving up. |
| `first_x_reached` | `f64` | The independent-variable value that lane reached. |

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
    fn clone(self: &Self) -> OdeEnsembleFailure { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
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
    fn eq(self: &Self, other: &OdeEnsembleFailure) -> bool { /* ... */ }
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
#### Struct `QuadratureInterval`

One lane of a quadrature batch: the limits of one definite integral.

# Units

`a` and `b` are in the integration variable's units. The value the batch
returns carries the product of those units and the integrand's.

# Conventions

- `a == b` integrates to exactly `0.0`, with no integrand evaluations.
- `b < a` is **supported** and returns the negated integral over `[b, a]`,
  the usual orientation convention. (The ODE half deliberately does *not*
  accept a reversed interval, because its underlying loop marches forwards
  only; quadrature has no such constraint.)
- A non-finite `a` or `b` yields [`QuadratureStatus::InvalidInterval`]. There
  is no support for infinite limits; a caller wanting one should substitute
  the variable itself, which is the only way to choose the transformation
  knowingly.

```rust
pub struct QuadratureInterval {
    pub a: f64,
    pub b: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `f64` | Lower limit, in the integration variable's units. |
| `b` | `f64` | Upper limit, in the integration variable's units. |

##### Implementations

###### Methods

- ```rust
  pub fn new(a: f64, b: f64) -> Self { /* ... */ }
  ```
  Build an interval `[a, b]`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> QuadratureInterval { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &QuadratureInterval) -> bool { /* ... */ }
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
#### Enum `GaussOrder`

Node count of a fixed-order Gauss-Legendre rule.

A closed set rather than an open `usize`, so the choice is exhaustive at
every match site and rust-analyzer lists the options on hover — and so the
nodes can be validated once per order rather than for whatever number a
caller happens to pass.

An `n`-point Gauss-Legendre rule integrates any polynomial of degree
`2n - 1` or less **exactly** (to rounding), which is both the reason to
prefer it over Simpson at equal cost and the property the tests use as their
oracle.

| Order | Nodes | Exact to degree |
|---|---|---|
| [`G2`](Self::G2) | 2 | 3 |
| [`G3`](Self::G3) | 3 | 5 |
| [`G4`](Self::G4) | 4 | 7 |
| [`G5`](Self::G5) | 5 | 9 |
| [`G8`](Self::G8) | 8 | 15 |

# Units

Dimensionless — a mode selector.

```rust
pub enum GaussOrder {
    G2,
    G3,
    G4,
    G5,
    G8,
}
```

##### Variants

###### `G2`

Two-point rule, exact to cubics.

###### `G3`

Three-point rule, exact to quintics.

###### `G4`

Four-point rule, exact to degree 7.

###### `G5`

Five-point rule, exact to degree 9.

###### `G8`

Eight-point rule, exact to degree 15.

##### Implementations

###### Methods

- ```rust
  pub const fn points(self: Self) -> usize { /* ... */ }
  ```
  Number of nodes in the rule, dimensionless.

- ```rust
  pub const fn exact_degree(self: Self) -> usize { /* ... */ }
  ```
  The highest polynomial degree this rule integrates exactly, `2n - 1`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> GaussOrder { /* ... */ }
    ```

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
    fn default() -> GaussOrder { /* ... */ }
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
    fn eq(self: &Self, other: &GaussOrder) -> bool { /* ... */ }
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
#### Enum `QuadratureRule`

Which fixed rule [`quadrature_batch`] applies to every lane.

All three are **composite** rules over equal subintervals of each lane's own
interval, and all three cost a fixed, data-independent number of integrand
evaluations per lane — which is what makes them branch-free, and the only
part of this module that would map cleanly onto a GPU.

A `panels` count of `0` is treated as `1`; there is no error path for it,
because it is a compile-time-shaped parameter rather than data.

| Variant | Evaluations per lane | Error order | Exact for |
|---|---|---|---|
| [`Trapezoid`](Self::Trapezoid) | `panels + 1` | `O(h^2)` | linear integrands |
| [`Simpson`](Self::Simpson) | `2 * panels + 1` | `O(h^4)` | cubics |
| [`GaussLegendre`](Self::GaussLegendre) | `panels * order` | `O(h^(2n))` | degree `2n - 1` |

# Relationship to the workspace's other Simpson

`outram-park-fork-dwsim-libs`'s `simpson_integrate` is a faithful port of
OPEM's rule *including* its documented flaw: it applies the `1, 4, 2, ..., 4,
1` weights without requiring an odd sample count, which silently degrades the
order to `O(h)` for an even one. [`Simpson`](Self::Simpson) here counts
**panels of two subintervals each**, so the sample count is odd by
construction and that failure cannot occur. The composite-over-equal-
subintervals convention is shared; the sample-slice interface and the bug are
not.

# Units

Dimensionless — a mode selector plus a count.

```rust
pub enum QuadratureRule {
    Trapezoid {
        panels: usize,
    },
    Simpson {
        panels: usize,
    },
    GaussLegendre {
        order: GaussOrder,
        panels: usize,
    },
}
```

##### Variants

###### `Trapezoid`

Composite trapezoid over `panels` equal subintervals.

The cheapest rule and the only one worth using when the integrand is
sampled rather than smooth. Second-order accurate.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `panels` | `usize` | Number of equal subintervals, `>= 1` (`0` is treated as `1`). |

###### `Simpson`

Composite Simpson over `panels` panels, each spanning **two** equal
subintervals, so `2 * panels` subintervals in total.

Fourth-order accurate and exact for cubics. Counting panels rather than
samples is what makes the even-sample-count error impossible.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `panels` | `usize` | Number of two-subinterval Simpson panels, `>= 1` (`0` is treated as<br>`1`). |

###### `GaussLegendre`

Composite Gauss-Legendre: `panels` equal subintervals, each integrated by
an `order`-point rule.

The best accuracy per evaluation for a smooth integrand, and the rule to
reach for by default. Gauss rules never evaluate the interval endpoints,
so an integrand that is unbounded but integrable at `a` or `b` is handled
without special-casing — the same property `raffles` relies on for
quantile-function moments.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `order` | `GaussOrder` | Nodes per subinterval. |
| `panels` | `usize` | Number of equal subintervals, `>= 1` (`0` is treated as `1`). |

##### Implementations

###### Methods

- ```rust
  pub const fn evaluations(self: Self) -> usize { /* ... */ }
  ```
  Integrand evaluations this rule performs per lane, dimensionless.

- ```rust
  pub const fn label(self: Self) -> &'static str { /* ... */ }
  ```
  A short human-readable label, for benchmark tables and log lines.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> QuadratureRule { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &QuadratureRule) -> bool { /* ... */ }
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
#### Struct `AdaptiveSettings`

Tolerances and work limit for [`adaptive_quadrature_batch`].

# Units

`abs_tol` is in the units of the *integral* (integrand times abscissa);
`rel_tol` is a dimensionless ratio; `max_subdivisions` is a count.

# Example

```rust
use outram_foam_basic_lib::ode::parallel::AdaptiveSettings;

// Struct-update syntax keeps the defaults you did not mean to change.
let s = AdaptiveSettings { abs_tol: 1e-12, ..AdaptiveSettings::default() };
assert_eq!(s.rel_tol, 1e-8);
```

```rust
pub struct AdaptiveSettings {
    pub abs_tol: f64,
    pub rel_tol: f64,
    pub max_subdivisions: u32,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `abs_tol` | `f64` | Absolute tolerance on the whole integral, in the integral's units. |
| `rel_tol` | `f64` | Relative tolerance, dimensionless, applied against the running estimate<br>of the integral's magnitude. |
| `max_subdivisions` | `u32` | Maximum bisections per lane before reporting<br>[`QuadratureStatus::ToleranceNotMet`]. Also bounded by<br>[`MAX_ADAPTIVE_DEPTH`] on depth. |

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
    fn clone(self: &Self) -> AdaptiveSettings { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &AdaptiveSettings) -> bool { /* ... */ }
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
#### Enum `QuadratureStatus`

How one lane of a quadrature batch ended.

| Variant | `last_value` | Meaning |
|---|---|---|
| [`Evaluated`](Self::Evaluated) | the integral | the rule ran to completion |
| [`ToleranceNotMet`](Self::ToleranceNotMet) | best estimate — **not** to tolerance | adaptive lane ran out of subdivisions |
| [`NotFinite`](Self::NotFinite) | `NaN` | the accumulated value was `NaN` or infinite |
| [`InvalidInterval`](Self::InvalidInterval) | `NaN` | a limit is not finite |

# What `Evaluated` does and does not claim

For [`adaptive_quadrature_batch`] it means the requested tolerance was met.
For a fixed [`QuadratureRule`] it means only that every node evaluated to a
finite number and the sum is finite — **it makes no accuracy claim at all**,
because a fixed rule has no way to know. Choosing enough panels for the
integrand is the caller's responsibility, and
[`QuadratureSolution::error_estimate`] is `NaN` on that path for exactly this
reason.

# Units

Dimensionless — a status tag.

```rust
pub enum QuadratureStatus {
    Evaluated,
    ToleranceNotMet,
    NotFinite,
    InvalidInterval,
}
```

##### Variants

###### `Evaluated`

The rule ran to completion with a finite result.

###### `ToleranceNotMet`

Adaptive only: the subdivision budget or [`MAX_ADAPTIVE_DEPTH`] was
reached before the tolerance. The reported value is the best estimate
available and is **not** claimed to meet the tolerance.

###### `NotFinite`

The accumulated value is `NaN` or infinite — either the integrand
returned a non-finite sample, or the sum overflowed.

###### `InvalidInterval`

A limit of the interval was infinite or `NaN`, so there is nothing to
integrate over.

##### Implementations

###### Methods

- ```rust
  pub fn is_evaluated(self: Self) -> bool { /* ... */ }
  ```
  Whether this status means the lane produced a usable value.

- ```rust
  pub fn label(self: Self) -> &'static str { /* ... */ }
  ```
  A short human-readable label, for log lines and failure reports.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> QuadratureStatus { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &QuadratureStatus) -> bool { /* ... */ }
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
#### Struct `QuadratureSolution`

The outcome of a single quadrature lane.

The fields are private on purpose, on the same reasoning as
[`OdeLaneSolution`]: [`Self::value`] returns `Option<f64>` and hands back
`Some` only for a lane that ran to completion, while the raw number is behind
the deliberately-named [`Self::last_value`].

`Copy`, so it can be read out of a [`QuadratureBatch`] without cloning.

# Units

[`Self::value`], [`Self::last_value`] and [`Self::error_estimate`] are in the
integral's units (integrand times abscissa); [`Self::evaluations`] is a
dimensionless count.

```rust
pub struct QuadratureSolution {
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
  pub fn value(self: &Self) -> Option<f64> { /* ... */ }
  ```
  The integral, if this lane ran to completion.

- ```rust
  pub fn last_value(self: &Self) -> f64 { /* ... */ }
  ```
  The last value the rule accumulated, complete or not.

- ```rust
  pub fn error_estimate(self: &Self) -> f64 { /* ... */ }
  ```
  Estimated absolute error, in the integral's units.

- ```rust
  pub fn evaluations(self: &Self) -> u32 { /* ... */ }
  ```
  Integrand evaluations this lane performed, dimensionless.

- ```rust
  pub fn status(self: &Self) -> QuadratureStatus { /* ... */ }
  ```
  How this lane ended.

- ```rust
  pub fn evaluated(self: &Self) -> bool { /* ... */ }
  ```
  Whether this lane produced a usable value.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> QuadratureSolution { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &QuadratureSolution) -> bool { /* ... */ }
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
#### Struct `QuadratureBatch`

A batch of `N` quadrature outcomes, in the same order as the intervals handed
in.

Lane `i` of the result corresponds to `intervals[i]`, always — the parallel
path preserves order.

# Getting values out

- [`Self::values`] — all-or-nothing. `Ok(Vec<f64>)` only when every lane ran
  to completion; otherwise `Err(`[`QuadratureBatchFailure`]`)`.
- [`Self::solutions`] — per-lane, when the caller wants to handle failures
  individually.

# Units

See [`QuadratureSolution`].

```rust
pub struct QuadratureBatch {
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
  pub fn solutions(self: &Self) -> &[QuadratureSolution] { /* ... */ }
  ```
  Every lane's outcome, in input order.

- ```rust
  pub fn into_solutions(self: Self) -> Vec<QuadratureSolution> { /* ... */ }
  ```
  Consume the batch and take the outcomes.

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Number of lanes, dimensionless.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether the batch has no lanes.

- ```rust
  pub fn get(self: &Self, i: usize) -> Option<QuadratureSolution> { /* ... */ }
  ```
  Lane `i`'s outcome, or `None` if `i` is out of range.

- ```rust
  pub fn all_evaluated(self: &Self) -> bool { /* ... */ }
  ```
  Whether every lane ran to completion. Vacuously `true` when empty.

- ```rust
  pub fn failure_count(self: &Self) -> usize { /* ... */ }
  ```
  How many lanes failed, dimensionless.

- ```rust
  pub fn first_failure(self: &Self) -> Option<(usize, QuadratureSolution)> { /* ... */ }
  ```
  The first failing lane and its outcome, if any.

- ```rust
  pub fn failures(self: &Self) -> Vec<(usize, QuadratureSolution)> { /* ... */ }
  ```
  Every failing lane, as `(index, outcome)` pairs.

- ```rust
  pub fn total_evaluations(self: &Self) -> u64 { /* ... */ }
  ```
  Total integrand evaluations over every lane, dimensionless.

- ```rust
  pub fn values(self: &Self) -> Result<Vec<f64>, QuadratureBatchFailure> { /* ... */ }
  ```
  All integrals, or an error naming the failures — the all-or-nothing path.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> QuadratureBatch { /* ... */ }
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
    fn eq(self: &Self, other: &QuadratureBatch) -> bool { /* ... */ }
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
#### Struct `QuadratureBatchFailure`

**Attributes:**

- `Other("#[error(\"{failure_count} of {total} quadrature lanes failed; \\\n     first failure at lane {first_index} with status {first_status:?} \\\n     after {first_evaluations} integrand evaluations\")]")`

One or more lanes of a [`QuadratureBatch`] failed.

Returned by [`QuadratureBatch::values`]. As [`OdeEnsembleFailure`], it names
both the scale of the problem and a specific lane to look at.

# Units

All counts and indices are dimensionless.

```rust
pub struct QuadratureBatchFailure {
    pub total: usize,
    pub failure_count: usize,
    pub first_index: usize,
    pub first_status: QuadratureStatus,
    pub first_evaluations: u32,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `total` | `usize` | Number of lanes in the batch. |
| `failure_count` | `usize` | Number of lanes that failed. |
| `first_index` | `usize` | Index of the first failing lane. |
| `first_status` | `QuadratureStatus` | Why that lane failed. |
| `first_evaluations` | `u32` | Integrand evaluations that lane performed. |

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
    fn clone(self: &Self) -> QuadratureBatchFailure { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Copy**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Display**
  - ```rust
    fn fmt(self: &Self, __formatter: &mut ::core::fmt::Formatter<''_>) -> ::core::fmt::Result { /* ... */ }
    ```

- **Eq**
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
    fn eq(self: &Self, other: &QuadratureBatchFailure) -> bool { /* ... */ }
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
### Functions

#### Function `ensemble_backend_for`

**Attributes:**

- `MustUse { reason: None }`

The [`ComputeBackend`] the ODE ensemble would actually use for `n` lanes if
asked for `requested` — without running anything.

Applies exactly the same reduction the kernels do (feature availability, no
GPU kernel here, and the [`ODE_ENSEMBLE_MIN_LANES`] size floor), so what it
reports is what would run. Useful for logging and for benchmark harnesses.

# Arguments

- `requested` — the backend a caller would pass to [`integrate_ensemble`].
- `n` — the number of independent lanes, dimensionless.

# Returns

Either [`ComputeBackend::Serial`] or [`ComputeBackend::CpuMulti`]; never
[`ComputeBackend::Gpu`], because no GPU kernel exists here.

# Example

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::ode::parallel::{ensemble_backend_for, ODE_ENSEMBLE_MIN_LANES};

// Too small to thread, whatever was asked for.
assert_eq!(ensemble_backend_for(ComputeBackend::CpuMulti, 4), ComputeBackend::Serial);

// Big enough; the answer now depends only on whether `parallel` is compiled in.
let picked = ensemble_backend_for(ComputeBackend::CpuMulti, ODE_ENSEMBLE_MIN_LANES);
assert!(picked.is_available());
```

```rust
pub fn ensemble_backend_for(requested: crate::compute::ComputeBackend, n: usize) -> crate::compute::ComputeBackend { /* ... */ }
```

#### Function `quadrature_backend_for`

**Attributes:**

- `MustUse { reason: None }`

The [`ComputeBackend`] the quadrature kernels would actually use for `n`
intervals if asked for `requested` — without running anything.

The quadrature counterpart of [`ensemble_backend_for`], differing only in
using the [`QUADRATURE_MIN_INTERVALS`] size floor. Both
[`quadrature_batch`] and [`adaptive_quadrature_batch`] use it.

# Arguments

- `requested` — the backend a caller would pass to [`quadrature_batch`].
- `n` — the number of independent definite integrals, dimensionless.

# Returns

Either [`ComputeBackend::Serial`] or [`ComputeBackend::CpuMulti`].

# Example

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::ode::parallel::quadrature_backend_for;

assert_eq!(quadrature_backend_for(ComputeBackend::CpuMulti, 8), ComputeBackend::Serial);
assert_eq!(quadrature_backend_for(ComputeBackend::Serial, 1 << 20), ComputeBackend::Serial);
```

```rust
pub fn quadrature_backend_for(requested: crate::compute::ComputeBackend, n: usize) -> crate::compute::ComputeBackend { /* ... */ }
```

#### Function `integrate_ensemble`

**Attributes:**

- `MustUse { reason: None }`

Integrate `N` independent initial-value problems with **one stepper for every
lane**, on the chosen backend.

This is the entry point for the common case: one ODE per cell, per particle,
or per material point, all of the same character. Each lane gets its own
clone of `solver`, so no scratch buffer is shared and no lane can perturb
another.

For an ensemble of mixed stiffness, use [`integrate_ensemble_mixed`] — see
the module-level "Stiffness" section.

# Arguments

- `lanes` — one [`OdeLane`] per problem. Lane `i` of the result corresponds
  to `lanes[i]`.
- `solver` — the stepper prototype, cloned once per lane. Its
  [`OdeSolverConfig`](crate::ode::OdeSolverConfig) tolerances and
  `max_steps` apply to every lane. It must have been built for the same
  equation count the lanes' systems report, because the steppers size their
  scratch buffers at construction.
- `backend` — requested execution backend. What actually runs is
  [`ensemble_backend_for`] applied to it: an unavailable backend degrades,
  `Gpu` degrades (no GPU kernel here), and an ensemble below
  [`ODE_ENSEMBLE_MIN_LANES`] runs serially. None of those changes the answer.

# Returns

An [`OdeEnsemble`] with one [`OdeLaneSolution`] per lane, in input order.

# Determinism

Bit-for-bit identical across backends and thread counts, for systems whose
`derivatives`/`jacobian` are pure deterministic functions of their arguments.
See the module-level "Determinism" section.

# Cost note

`solver` is cloned once per lane, which clones its per-equation scratch
buffers — a handful of small allocations per lane. This is deliberate: it is
what makes each lane a pure function of its own inputs, and therefore what
makes the bitwise-identity claim hold without depending on every stepper's
buffers being write-before-read. On the measured ensemble it is a small
fraction of the per-lane cost — about 8.6 us of integration per lane over 68
adaptive steps, see [`ODE_ENSEMBLE_MIN_LANES`] — but a caller integrating a
*very* short interval per lane would see it, and it has not been measured
separately from the integration it accompanies.

# Panics

Panics if `solver` is [`OdeSolver::Rosenbrock23`] and a lane's system does
not override
[`OdeSystem::jacobian`](crate::ode::OdeSystem::jacobian), whose default
implementation panics. On the `CpuMulti` path the panic propagates out of the
`rayon` scope. Check
[`OdeSolver::requires_jacobian`](crate::ode::OdeSolver::requires_jacobian)
first.

# Verification

*Methodology.* Checked against the closed-form solution of `dy/dx = -k y`,
`y(0) = 1`, namely `y(x) = exp(-k x)`, over 64 lanes with `k` spread evenly
across `[0.5, 8]`, integrated to `x = 1` by all three steppers; and against
the harmonic oscillator `y1' = -y2`, `y2' = y1` from `y(0) = (1, 0)`, whose
solution is `(cos x, sin x)`, over 16 lanes ending at `m * pi/2` for
`m = 1..=16`. Tolerances `abs_tol = 1e-10`, `rel_tol = 1e-8` for `Rkf45` and
`Rosenbrock23` (`1e-12` / `1e-10` for the oscillator) and `1e-3` / `1e-2`
for `Euler`, which cannot reach the high-order tolerances inside the default
10 000-step budget. Pass criteria: `< 1e-8` for the high-order steppers,
`< 5e-2` for first-order `Euler`.

*Results, measured 2026-08-13 by `ensemble_matches_analytic_decay` and
`ensemble_matches_harmonic_oscillator` in `parallel/tests.rs`, release
build:* worst absolute error over the 64 decay lanes **1.518896e-9**
(`Rkf45`, 3 038 total accepted steps, 81 in the worst lane),
**1.387431e-9** (`Rosenbrock23`, 46 535 steps, 1 186 in the worst lane) and
**1.297370e-3** (`Euler`, 21 817 steps, 421 in the worst lane); worst
absolute error on the oscillator **4.905929e-10** (`Rkf45`).

*Interpretation.* The two high-order steppers agree with the closed form to
their requested tolerance and with each other to about 1e-9, while Euler is
six orders coarser — the expected signature of three genuinely different
steppers being reached through the ensemble, which is what rules out the
wrapper silently routing every lane to one of them. The step counts are the
other half of the story: `Rosenbrock23` needs 15x the steps of `Rkf45` on a
non-stiff problem to reach the same accuracy, which is exactly why
[`integrate_ensemble_mixed`] exists rather than "just use the stiff solver
everywhere".

# Example — the `uom` boundary

The ensemble is dimensionless, and the caller converts at its edge. These
lanes are lumped-capacitance bodies cooling towards ambient, `dT/dt =
-(T - T_inf) / tau`, one lane per time constant, recovering a
`ThermodynamicTemperature`.

**On the tolerance asserted below.** The stepper's `rel_tol` is `1e-8` and
the states are of order 400 K, so the achievable *absolute* accuracy is a
few microkelvin — the error floor is set by the controller's tolerance and
the magnitude of the state, not by the ensemble. Measured 2026-08-13
(release) by `lumped_body_accuracy_is_set_by_the_relative_tolerance` in
`parallel/tests.rs`: worst absolute error **1.150937e-6 K** at
`abs_tol = 1e-10`, `rel_tol = 1e-8`; **1.222958e-8 K** at `1e-12`/`1e-10`;
**1.289209e-10 K** at `1e-14`/`1e-12`. The bound below is `5e-6 K`, set from
the first of those measurements — a tighter assertion would be asserting
something this stepper at this tolerance does not deliver. A caller wanting
sub-nanokelvin must ask for it through the tolerances, and the third row
says what that buys.

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::ode::{OdeSolver, OdeSystem};
use outram_foam_basic_lib::ode::parallel::{integrate_ensemble, OdeLane};
use uom::si::f64::{ThermodynamicTemperature, Time};
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

/// Lumped body: `dT/dt = -(T - T_inf) / tau`, all in SI base units.
struct LumpedBody { tau_s: f64, t_inf_k: f64 }
impl OdeSystem for LumpedBody {
    fn n_eqns(&self) -> usize { 1 }
    fn derivatives(&self, _t: f64, y: &[f64], dydt: &mut Vec<f64>) {
        dydt[0] = -(y[0] - self.t_inf_k) / self.tau_s;
    }
}

// Convert in: typed quantities out to plain f64 in named units.
let t_inf = ThermodynamicTemperature::new::<kelvin>(300.0);
let t0 = ThermodynamicTemperature::new::<kelvin>(500.0);
let horizon = Time::new::<second>(10.0);

let lanes: Vec<OdeLane<LumpedBody>> = [5.0_f64, 20.0]
    .iter()
    .map(|&tau| {
        OdeLane::new(
            LumpedBody { tau_s: tau, t_inf_k: t_inf.get::<kelvin>() },
            vec![t0.get::<kelvin>()],
            0.0,
            horizon.get::<second>(),
            0.1,
        )
    })
    .collect();

let ensemble = integrate_ensemble(
    &lanes,
    &OdeSolver::rkf45(1, 1e-10, 1e-8),
    ComputeBackend::CpuMulti,
);

// Convert out: back to typed temperatures.
let temperatures: Vec<ThermodynamicTemperature> = ensemble
    .states()
    .expect("both lanes complete")
    .iter()
    .map(|s| ThermodynamicTemperature::new::<kelvin>(s[0]))
    .collect();

// Closed form: T(t) = T_inf + (T0 - T_inf) exp(-t / tau). The 5e-6 K bound
// is the measured floor at rel_tol = 1e-8 on a ~400 K state; see above.
for (tau, temperature) in [5.0_f64, 20.0].iter().zip(&temperatures) {
    let exact = 300.0 + 200.0 * (-10.0_f64 / tau).exp();
    assert!((temperature.get::<kelvin>() - exact).abs() < 5e-6);
}
```

```rust
pub fn integrate_ensemble<S>(lanes: &[OdeLane<S>], solver: &super::OdeSolver, backend: crate::compute::ComputeBackend) -> OdeEnsemble
where
    S: OdeSystem + Sync { /* ... */ }
```

#### Function `integrate_ensemble_mixed`

**Attributes:**

- `MustUse { reason: None }`

Integrate `N` independent initial-value problems with the **stepper chosen
per lane**, on the chosen backend.

The mixed-stiffness entry point. `solver_of(i)` is called once for lane `i`
and its return value integrates that lane and no other, so a caller can put
[`Rosenbrock23`](crate::ode::Rosenbrock23) on the handful of stiff cells and
leave the rest on [`Rkf45`](crate::ode::Rkf45) — paying the LU factorisation
only where it is needed.

The natural way to use it is as a second pass: run
[`integrate_ensemble`] with an explicit stepper, read
[`OdeEnsemble::failures`], and re-run just those lanes stiff.

# Arguments

- `lanes` — one [`OdeLane`] per problem.
- `solver_of` — `solver_of(i)` returns the stepper for lane `i`. Called
  exactly once per lane, on whichever thread runs that lane, hence the `Sync`
  bound; the bound is present in both feature builds so that enabling
  `parallel` never changes a public signature. It must be a pure
  deterministic function of `i` — see the module-level "Determinism" section.
- `backend` — requested backend; see [`ensemble_backend_for`].

# Returns

An [`OdeEnsemble`] with one [`OdeLaneSolution`] per lane, in input order.

# Panics

As [`integrate_ensemble`]: a lane given
[`OdeSolver::Rosenbrock23`] whose system does not implement
[`OdeSystem::jacobian`](crate::ode::OdeSystem::jacobian) panics.

# Example — a stiff lane and a benign one in one ensemble

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::matrix::SquareMatrix;
use outram_foam_basic_lib::ode::{OdeSolver, OdeSystem};
use outram_foam_basic_lib::ode::parallel::{integrate_ensemble_mixed, OdeLane};

/// `dy/dx = -k y`, with an analytic Jacobian so the stiff stepper can run.
struct Decay { k: f64 }
impl OdeSystem for Decay {
    fn n_eqns(&self) -> usize { 1 }
    fn derivatives(&self, _x: f64, y: &[f64], d: &mut Vec<f64>) { d[0] = -self.k * y[0]; }
    fn jacobian(&self, _x: f64, _y: &[f64], dfdx: &mut Vec<f64>, dfdy: &mut SquareMatrix) {
        dfdx[0] = 0.0;
        dfdy.set(0, 0, -self.k);
    }
}

// Lane 0 is benign (k = 1), lane 1 is stiff (k = 5000).
let lanes = vec![
    OdeLane::new(Decay { k: 1.0 }, vec![1.0], 0.0, 1.0, 0.1),
    OdeLane::new(Decay { k: 5000.0 }, vec![1.0], 0.0, 1.0, 0.1),
];

let ensemble = integrate_ensemble_mixed(
    &lanes,
    |i| {
        if i == 1 {
            OdeSolver::rosenbrock23(1, 1e-10, 1e-8)
        } else {
            OdeSolver::rkf45(1, 1e-10, 1e-8)
        }
    },
    ComputeBackend::Serial,
);

assert!(ensemble.all_completed());
let states = ensemble.states().unwrap();
assert!((states[0][0] - (-1.0_f64).exp()).abs() < 1e-8);
assert!(states[1][0].abs() < 1e-8); // exp(-5000) underflows to ~0
```

```rust
pub fn integrate_ensemble_mixed<S, G>(lanes: &[OdeLane<S>], solver_of: G, backend: crate::compute::ComputeBackend) -> OdeEnsemble
where
    S: OdeSystem + Sync,
    G: Fn(usize) -> super::OdeSolver + Sync { /* ... */ }
```

#### Function `quadrature_batch`

**Attributes:**

- `MustUse { reason: None }`

Evaluate `N` independent definite integrals with a **fixed rule**, on the
chosen backend.

The GPU-shaped half of this module: every lane performs exactly
[`QuadratureRule::evaluations`] integrand calls with no data-dependent
branching. Use it when the integrand is smooth and the panel count can be
chosen once for the whole batch — a band-averaged cross section, a
face-integrated flux, a cell-integrated source term.

# Arguments

- `intervals` — one [`QuadratureInterval`] per lane.
- `rule` — the rule applied to every lane. [`QuadratureRule::GaussLegendre`]
  is the default worth reaching for on a smooth integrand.
- `backend` — requested backend. What actually runs is
  [`quadrature_backend_for`] applied to it; a batch below
  [`QUADRATURE_MIN_INTERVALS`] runs serially. None of the degradations
  changes the answer.
- `f` — the integrand. `f(i, x)` must return lane `i`'s integrand at abscissa
  `x`. It **must be a pure deterministic function of its arguments** — see
  the module-level "Determinism" section. It is called from multiple threads
  on the `CpuMulti` path, hence the `Sync` bound; the bound is present in
  both feature builds so that enabling `parallel` never changes a public
  signature.

# Returns

A [`QuadratureBatch`] with one [`QuadratureSolution`] per interval, in input
order.

# Determinism

Bit-for-bit identical across backends and thread counts. The sum within a
lane is sequential and is never split across threads — see the module-level
"Determinism" section for why that restriction is deliberate.

# Verification

*Methodology.* Three oracles, all exact rather than another implementation.
(1) *Polynomial exactness*: an `n`-point Gauss-Legendre rule must integrate
every monomial `x^d` for `d <= 2n - 1` exactly, Simpson must be exact for
`d <= 3` and trapezoid for `d <= 1`; checked over `[0, 1]` and `[-2, 3]`
against the closed form `(b^(d+1) - a^(d+1)) / (d + 1)`. (2) *Published
nodes*: the computed 8-point nodes and weights against the Abramowitz &
Stegun 25.4.30 values already carried in this workspace by
`crates/raffles/src/distributions.rs`. (3) *Transcendental reference*:
`integral of exp(-x) sin(x) from 0 to pi = (1 + exp(-pi)) / 2`. Pass
criteria: `< 1e-13` relative for exactness, `< 1e-15` absolute against A&S,
`< 1e-12` absolute for the transcendental.

*Results, measured 2026-08-13 by `gauss_legendre_is_exact_to_its_degree`,
`gauss_nodes_match_the_in_workspace_abramowitz_stegun_values`,
`simpson_and_trapezoid_are_exact_to_their_degree` and
`fixed_rules_match_a_transcendental_reference` in `parallel/tests.rs`,
release build:*

- Exactness sweep (`G2`..`G8`, degrees 0 to 15, both intervals): worst
  relative error **5.769990e-16**. Simpson over degrees 0 to 3:
  **3.552714e-16**. Trapezoid over degrees 0 to 1: **0.000000e0**.
- Computed `G8` against A&S 25.4.30: worst node difference
  **1.110223e-16**, worst weight difference **1.249001e-16**, weights
  summing to **2.00000000000000000**.
- Transcendental reference, closed form **0.52160695913188615**: `G8` over 8
  panels (64 evaluations) gave **0.52160695913188615**, error
  **0.000000e0**; Simpson over 64 panels (129 evaluations) error
  **4.206809e-9**; trapezoid over 128 panels (129 evaluations) error
  **5.236767e-5**.

*Interpretation.* The Gauss nodes and weights are correct to the last bit
`f64` holds — agreeing with a published table to within one unit in the last
place while being computed independently of it — the composite mapping onto
arbitrary intervals is correct, and the order hierarchy behaves as theory
requires: `G8` reaches the rounding floor on a smooth transcendental at half
the evaluations where a 64-panel Simpson is still nine orders away and a
trapezoid four orders beyond that.

# Example — the `uom` boundary

The batch is dimensionless, and the caller converts at its edge. These lanes
integrate a linearly-ramping electrical power over time to recover an
`Energy`:

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::ode::parallel::{
    quadrature_batch, GaussOrder, QuadratureInterval, QuadratureRule,
};
use uom::si::f64::{Energy, Power, Time};
use uom::si::energy::joule;
use uom::si::power::watt;
use uom::si::time::second;

// P(t) = P0 (1 - t / T), shutting down linearly over the window.
let p0 = Power::new::<watt>(2.0e6);
let window = Time::new::<second>(30.0);

// Convert in: limits in seconds, as plain f64.
let intervals = [
    QuadratureInterval::new(0.0, window.get::<second>()),
    QuadratureInterval::new(0.0, 0.5 * window.get::<second>()),
];

let batch = quadrature_batch(
    &intervals,
    QuadratureRule::GaussLegendre { order: GaussOrder::G4, panels: 4 },
    ComputeBackend::CpuMulti,
    |_, t| p0.get::<watt>() * (1.0 - t / window.get::<second>()),
);

// Convert out: back to typed energies.
let energies: Vec<Energy> = batch
    .values()
    .expect("both lanes evaluate")
    .into_iter()
    .map(Energy::new::<joule>)
    .collect();

// Closed form over [0, T] is P0 T / 2; over [0, T/2] it is 3 P0 T / 8.
assert!((energies[0].get::<joule>() - 0.5 * 2.0e6 * 30.0).abs() < 1e-6);
assert!((energies[1].get::<joule>() - 0.375 * 2.0e6 * 30.0).abs() < 1e-6);
```

```rust
pub fn quadrature_batch<F>(intervals: &[QuadratureInterval], rule: QuadratureRule, backend: crate::compute::ComputeBackend, f: F) -> QuadratureBatch
where
    F: Fn(usize, f64) -> f64 + Sync { /* ... */ }
```

#### Function `adaptive_quadrature_batch`

**Attributes:**

- `MustUse { reason: None }`

Evaluate `N` independent definite integrals **adaptively**, on the chosen
backend.

Adaptive Simpson with local error control: each lane bisects wherever the
integrand resists a Simpson panel, and stops where it does not. Use it when
the integrand's difficulty is not known in advance, or differs between lanes,
or is concentrated in a small part of the interval — a peak in a resonance
integral, a boundary layer, a kink at a phase boundary.

# Why this path is CPU-only, and stays so

The subdivision pattern is decided by the integrand at run time, so
neighbouring lanes take different branches and perform different numbers of
evaluations. That is exactly the control-flow divergence SIMT hardware
handles worst — a GPU implementation would serialise the divergent branches
and need a per-lane work stack in device memory. Batching it across CPU cores
costs nothing and works; putting it on a GPU would be a large amount of code
for an unclear win. **It is deliberately CPU-only**, and this is the reason.

The CPU path is still fully hybrid: it takes a [`ComputeBackend`] and threads
across lanes exactly like every other kernel here.

# Arguments

- `intervals` — one [`QuadratureInterval`] per lane.
- `settings` — tolerances and the subdivision budget; see
  [`AdaptiveSettings`].
- `backend` — requested backend; see [`quadrature_backend_for`]. The
  [`QUADRATURE_MIN_INTERVALS`] floor is shared with [`quadrature_batch`] and
  was measured on that path, so it is a conservative assumption here rather
  than a measurement.
- `f` — the integrand, `f(i, x)`. Must be pure and deterministic; called from
  multiple threads on the `CpuMulti` path.

# Returns

A [`QuadratureBatch`]. A lane that met its tolerance reports
[`QuadratureStatus::Evaluated`] and a meaningful
[`QuadratureSolution::error_estimate`]; a lane that exhausted its budget
reports [`QuadratureStatus::ToleranceNotMet`] and its best estimate, and is
excluded from [`QuadratureBatch::values`].

# Determinism

Bit-for-bit identical across backends and thread counts. The subdivision
order within a lane is a deterministic function of the integrand, and no
lane's arithmetic is split across threads.

# Verification

*Methodology.* Run against three integrands with closed forms, two of them
deliberately awkward for a uniform panel layout:
`integral of exp(-x) sin(x) from 0 to pi = (1 + exp(-pi)) / 2`;
`integral of sqrt(x) from 0 to 1 = 2/3`, which is bounded but has an
infinite derivative at the lower limit; and
`integral of 1 / (1 + 400 (x - 1/2)^2) from 0 to 1 = atan(10) / 10`, a peak
occupying about a twentieth of the interval. Tolerances `abs_tol = 1e-11`,
`rel_tol = 1e-10`, `max_subdivisions = 100 000`. Pass criteria: absolute
error below `1e-9`; every lane [`QuadratureStatus::Evaluated`]; and the
reported [`QuadratureSolution::error_estimate`] no more than 100x smaller
than the true error, since an estimate that badly understates the error
would be worse than none.

*Results, measured 2026-08-13 by `adaptive_matches_closed_forms` in
`parallel/tests.rs`, release build:*

| Integrand | Value | Error | Reported estimate | Evaluations |
|---|---|---|---|---|
| `exp(-x) sin(x)` on `[0, pi]` | 0.52160695913188759 | 1.443290e-15 | 2.034556e-11 | 469 |
| `sqrt(x)` on `[0, 1]` | 0.66666666666664931 | 1.731948e-14 | 2.619949e-11 | 1057 |
| narrow peak on `[0, 1]` | 0.14711276743037432 | 8.604228e-16 | 2.682116e-11 | 1225 |

*Interpretation.* All three land at or near the rounding floor, four to five
orders better than the requested `1e-11`, and the reported estimate is
conservative in every case — it overstates the true error by three to four
orders rather than understating it, which is the safe direction for an error
bound. The evaluation counts are the point of the adaptive path and the
source of the load imbalance work-stealing exists to absorb: 469 against
1 225 for problems posed identically, a 2.6x spread decided entirely by the
integrand. `sqrt(x)` is the informative case — its infinite endpoint
derivative makes a uniform panel layout converge slowly, and the adaptive
path instead concentrates its subdivisions near `x = 0`.

# Limitations

**An integrand that is unbounded at an interval endpoint is not supported
here.** Adaptive Simpson evaluates `f(a)` and `f(b)` on its very first step,
so an integrable singularity sitting exactly on a limit — `ln(x)` or
`1/sqrt(x)` at `x = 0` — produces a non-finite first estimate and the lane
reports [`QuadratureStatus::NotFinite`]. This inverts the naive expectation
that the adaptive path is the more capable one: for an endpoint singularity
reach for [`quadrature_batch`] with
[`QuadratureRule::GaussLegendre`] instead, whose nodes are strictly interior
and never touch the limits. That is exactly the property `raffles` relies on
for its quantile-function moments. A singularity in the *interior* of the
interval is also not handled: it will exhaust the subdivision budget and
report [`QuadratureStatus::ToleranceNotMet`], which is at least honest.

# Example

```rust
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::ode::parallel::{
    adaptive_quadrature_batch, AdaptiveSettings, QuadratureInterval,
};

// Three lanes of `integral of exp(-a x) from 0 to 1` = (1 - exp(-a)) / a.
let a = [1.0_f64, 5.0, 20.0];
let intervals: Vec<QuadratureInterval> =
    (0..3).map(|_| QuadratureInterval::new(0.0, 1.0)).collect();

let batch = adaptive_quadrature_batch(
    &intervals,
    AdaptiveSettings::default(),
    ComputeBackend::CpuMulti,
    |i, x| (-a[i] * x).exp(),
);

for (i, v) in batch.values().expect("all lanes meet tolerance").iter().enumerate() {
    let exact = (1.0 - (-a[i]).exp()) / a[i];
    assert!((v - exact).abs() < 1e-10, "lane {i}: {v} vs {exact}");
}
```

```rust
pub fn adaptive_quadrature_batch<F>(intervals: &[QuadratureInterval], settings: AdaptiveSettings, backend: crate::compute::ComputeBackend, f: F) -> QuadratureBatch
where
    F: Fn(usize, f64) -> f64 + Sync { /* ... */ }
```

### Constants and Statics

#### Constant `ODE_ENSEMBLE_MIN_LANES`

Lane count below which a [`ComputeBackend::CpuMulti`] request runs
[`integrate_ensemble`] on the calling thread instead.

# Measured crossover

*Methodology.* Measured 2026-08-13 on this workspace's development machine,
`std::thread::available_parallelism()` = **4**, release build, `--features
parallel`, `rayon`'s global pool, machine otherwise idle (see the
*Contention* note below, which is not a footnote — it changes the answer).
Ensembles of `n` independent one-equation decay problems `dy/dx = -k_i y`,
`y(0) = 1`, integrated from `x = 0` to `x = 1` by
[`Rkf45`](crate::ode::Rkf45) with `abs_tol = 1e-10`, `rel_tol = 1e-8`,
initial step `0.1`. Half the lanes are given `k_i` in `[0.5, 1.5)` and half
in `[50, 70)`, so the ensemble is deliberately imbalanced: on the 4 096-lane
case the accepted-step count averages **67.9** per lane against a
**maximum of 126**. Each lane costs about **8.6 us** of serial work. Best of 7
samples per point, wall clock for one whole ensemble. Produced by the
`#[ignore]`d `ensemble_crossover_benchmark` test in `parallel/tests.rs` and
transcribed from its printed output. Three independent runs are carried
side by side rather than averaged, because the parallel column is far
noisier than the serial one and the spread is the finding.

| Lanes | serial (run A) | speed-up A | speed-up B | speed-up C |
|---|---|---|---|---|
| 8 | 66.85 us | 2.66x | 1.26x | 0.89x |
| 16 | 132.56 us | 2.99x | 2.10x | 3.12x |
| 32 | 267.99 us | 1.23x | 1.53x | 2.51x |
| 64 | 545.25 us | 3.07x | 1.94x | 2.37x |
| 128 | 1092.75 us | 3.27x | 1.96x | 3.78x |
| 256 | 2212.57 us | 3.67x | 2.41x | 2.99x |
| 1 024 | 8814.43 us | 3.55x | 2.94x | 3.72x |
| 4 096 | 35205.42 us | 3.67x | 3.87x | 3.60x |
| 16 384 | 141229.69 us | 3.86x | 3.89x | 3.74x |

*Result.* **16** is the smallest size at which the parallel path won in all
three runs *and* kept winning at every larger size in all three, and it is
the value this constant takes. At 8 lanes it lost run C (0.89x).

*How firm is that.* Not very, and the table says so honestly. Between 16 and
128 lanes the **sign** of the effect is consistent — the parallel path never
loses — but the **magnitude** is not resolved: 32 lanes gave 1.23x and 2.51x
in two runs of the same code on the same data. Anywhere in 16–128 would be
defensible on this evidence. What the table does establish firmly is the
plateau: from 1 024 lanes upward the speed-up sits at 3.5–3.9x on 4 logical
cores, run to run, which is close to the ideal and is the expected signature
of a compute-bound kernel with no cross-lane traffic.

*Interpretation.* This is the lowest crossover measured anywhere in the crate
— 16x below [`crate::math::parallel::ROOT_BATCH_MIN_PROBLEMS`] (256), 256x
below the crate-wide placeholder
[`crate::compute::CPU_MULTI_MIN_WORK_ITEMS`] (4 096) and 8 192x below
[`crate::fields::parallel::FIELD_PARALLEL_CROSSOVER`] (131 072). The reason
is structural: one lane here is 68 adaptive steps, each of them six
derivative evaluations, against a state vector of one `f64`. At about 8.6 us
per lane it is by a wide margin the most compute-dense per work item of the
crate's measured kernels, so `rayon`'s dispatch cost is amortised almost
immediately. Five kernel families have now been measured in this crate and
they want **16, 256, 4 096, 131 072 and 262 144** — a spread of 16 384x,
which is the strongest evidence yet that no single crate-wide threshold can
be right.

The corollary is the one the root finder also found, sharpened here:
**the crossover is set by the caller's problem, not by this module.** A lane
integrating a short interval, or a two-equation system to a loose tolerance,
is an order of magnitude cheaper and crosses over correspondingly later. A
caller whose lanes are very cheap should pass [`ComputeBackend::Serial`]
explicitly rather than trust this number.

# Contention — this threshold assumes the cores are actually free

Three earlier runs of the same benchmark were taken while an unrelated
`rustc` was using ~234% CPU on the same 4-core machine (load average 5.08),
and they are **not** the table above. Under that load the parallel path lost
at 8 lanes in every run and lost once at 64 lanes, and the plateau speed-up
fell from 3.5–3.9x to 2.9–3.9x. The lesson generalises beyond this
measurement: a threshold measured on an idle machine is optimistic for a
process sharing cores with anything else — an MPI rank per core, a coupled
solver threading elsewhere, or a CI box running several jobs. In those
settings prefer an explicit [`ComputeBackend`].

# Limitations

One machine, four logical cores, one system family (scalar linear decay),
one stepper (`Rkf45`). Not measured on Android/Termux hardware, not on a
many-core server, and not with [`Rosenbrock23`](crate::ode::Rosenbrock23),
whose per-lane cost includes an LU factorisation and is therefore higher
still — meaning this floor is conservative for stiff ensembles rather than
wrong for them.

# Units

A count of independent initial-value problems, dimensionless.

```rust
pub const ODE_ENSEMBLE_MIN_LANES: usize = 16;
```

#### Constant `QUADRATURE_MIN_INTERVALS`

Interval count below which a [`ComputeBackend::CpuMulti`] request runs
[`quadrature_batch`] and [`adaptive_quadrature_batch`] on the calling thread
instead.

# Measured crossover

*Methodology.* Same machine, build and conditions as
[`ODE_ENSEMBLE_MIN_LANES`] (4 logical cores, release, `--features parallel`,
idle, best of 7 samples, three independent runs), measured 2026-08-13.
Batches of `n` independent integrals of `exp(-a_i x) sin(b_i x)` with
`a_i` in `[0.1, 3)` and `b_i` in `[1, 30)`, over per-lane intervals inside
`[0, 4]`, evaluated with [`QuadratureRule::GaussLegendre`] at
[`GaussOrder::G5`] over 16 panels — 80 integrand evaluations per lane, about
**1.4 us** of serial work, a mid-cost rule. Produced by the `#[ignore]`d
`quadrature_crossover_benchmark` test in `parallel/tests.rs` and transcribed
from its printed output.

| Intervals | serial (run A) | speed-up A | speed-up B | speed-up C |
|---|---|---|---|---|
| 16 | 20.48 us | 1.36x | 1.16x | 0.51x |
| 32 | 40.96 us | 2.03x | 2.10x | 1.81x |
| 64 | 82.62 us | 1.95x | 2.47x | 2.62x |
| 128 | 170.22 us | 1.96x | 1.58x | 2.75x |
| 256 | 351.75 us | 3.15x | 3.47x | 3.29x |
| 1 024 | 1475.98 us | 3.61x | 3.79x | 3.74x |
| 4 096 | 6090.40 us | 3.89x | 3.75x | 3.97x |
| 16 384 | 24717.71 us | 3.89x | 3.75x | 3.90x |

*Result.* **32** is the smallest size at which the parallel path won in all
three runs and kept winning at every larger size, and it is the value this
constant takes. At 16 intervals it lost run C badly (0.51x).

*Interpretation.* Twice the ODE floor of 16, on lanes about 6x cheaper
(1.4 us against 8.6 us). The two do not scale together, which is the honest
reading of a measurement whose 16–128 region is dominated by run-to-run
noise in both kernels; what both agree on is that a batch of a few hundred
compute-dense lanes is comfortably worth threading and a batch of a handful
is not. From 256 intervals upward the speed-up is a stable 3.1–4.0x on 4
logical cores.

**This one floor is shared by both quadrature entry points**, and it was
measured on the fixed-rule path only. [`adaptive_quadrature_batch`] costs
more per lane than any fixed rule a caller is likely to choose — the
verification lanes on that function needed 469 to 1 225 evaluations against
this rule's 80 — so it crosses over *earlier* and inherits this floor as a
conservative assumption rather than a measurement. A caller whose fixed rule
is very cheap — say [`QuadratureRule::Trapezoid`] with one panel, two
evaluations — is well below what was measured and should pass a backend
explicitly.

# Contention

The same caveat as [`ODE_ENSEMBLE_MIN_LANES`], and it bit harder here.
Three earlier runs taken while an unrelated `rustc` held ~234% CPU on the
same 4-core machine showed the parallel path **losing at every size up to
64** and sitting at 0.98–1.02x — no speed-up at all — at 256 through 4 096,
where the idle machine gives 3.1–4.0x. A threshold measured on an idle
machine is optimistic for a process that shares its cores.

# Limitations

One machine, four logical cores, idle, one integrand family, one rule
(`G5` over 16 panels). Not measured on Android/Termux. Not measured for the
adaptive path, for the trapezoid or Simpson rules, or for other Gauss orders.

# Units

A count of independent definite integrals, dimensionless.

```rust
pub const QUADRATURE_MIN_INTERVALS: usize = 32;
```

#### Constant `MAX_ADAPTIVE_DEPTH`

Hard ceiling on adaptive bisection depth in
[`adaptive_quadrature_batch`], regardless of
[`AdaptiveSettings::max_subdivisions`].

A depth of 50 halvings shrinks an interval by a factor of `2^50` (about
`10^15`), at which point the sub-interval is at the rounding floor of any
abscissa a caller is likely to pass and further subdivision cannot improve
the answer. The ceiling exists so that a pathological integrand — one with a
genuine singularity inside the interval — terminates with
[`QuadratureStatus::ToleranceNotMet`] instead of consuming memory
indefinitely.

# Units

A count of bisections, dimensionless.

```rust
pub const MAX_ADAPTIVE_DEPTH: u32 = 50;
```

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
    NonFiniteState,
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

###### `NonFiniteState`

The system produced a non-finite (NaN or infinite) error estimate, so
the state cannot be trusted and integration stopped.

The usual cause is `derivatives` or `jacobian` returning a non-finite
value — for example a numerically-differenced Jacobian
([`crate::math::differentiate::NumericalJacobian`]) that could not form
a column, or an evaluation that left the model's valid range.

# Why this variant exists

Before bead `op-ad6h` this case did not error at all: it returned
`Ok(())` with a NaN state, because the per-equation error fold used
`f64::max`, which discards NaN. A wrong answer reported as success is
the worst failure mode available to a solver, so it is now a distinct
error rather than being folded into
[`StepSizeUnderflow`](Self::StepSizeUnderflow), which would have named
the wrong cause.

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

- `NumericalJacobian<S>` with <S>
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

#### Re-export `adaptive_quadrature_batch`

```rust
pub use parallel::adaptive_quadrature_batch;
```

#### Re-export `ensemble_backend_for`

```rust
pub use parallel::ensemble_backend_for;
```

#### Re-export `integrate_ensemble`

```rust
pub use parallel::integrate_ensemble;
```

#### Re-export `integrate_ensemble_mixed`

```rust
pub use parallel::integrate_ensemble_mixed;
```

#### Re-export `quadrature_backend_for`

```rust
pub use parallel::quadrature_backend_for;
```

#### Re-export `quadrature_batch`

```rust
pub use parallel::quadrature_batch;
```

#### Re-export `AdaptiveSettings`

```rust
pub use parallel::AdaptiveSettings;
```

#### Re-export `GaussOrder`

```rust
pub use parallel::GaussOrder;
```

#### Re-export `OdeEnsemble`

```rust
pub use parallel::OdeEnsemble;
```

#### Re-export `OdeEnsembleFailure`

```rust
pub use parallel::OdeEnsembleFailure;
```

#### Re-export `OdeLane`

```rust
pub use parallel::OdeLane;
```

#### Re-export `OdeLaneSolution`

```rust
pub use parallel::OdeLaneSolution;
```

#### Re-export `OdeLaneStatus`

```rust
pub use parallel::OdeLaneStatus;
```

#### Re-export `QuadratureBatch`

```rust
pub use parallel::QuadratureBatch;
```

#### Re-export `QuadratureBatchFailure`

```rust
pub use parallel::QuadratureBatchFailure;
```

#### Re-export `QuadratureInterval`

```rust
pub use parallel::QuadratureInterval;
```

#### Re-export `QuadratureRule`

```rust
pub use parallel::QuadratureRule;
```

#### Re-export `QuadratureSolution`

```rust
pub use parallel::QuadratureSolution;
```

#### Re-export `QuadratureStatus`

```rust
pub use parallel::QuadratureStatus;
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

#### Re-export `krylov_solve_prepared`

```rust
pub use crate::ldu_matrix::krylov_solve_prepared;
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

#### Re-export `bicgstab_prepared`

```rust
pub use crate::krylov::bicgstab_prepared;
```

#### Re-export `gmres`

```rust
pub use crate::krylov::gmres;
```

#### Re-export `gmres_prepared`

```rust
pub use crate::krylov::gmres_prepared;
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

#### Re-export `gpu_adapter_present`

```rust
pub use crate::compute::gpu_adapter_present;
```

#### Re-export `select_backend`

```rust
pub use crate::compute::select_backend;
```

#### Re-export `ComputeBackend`

```rust
pub use crate::compute::ComputeBackend;
```

#### Re-export `ThreadCount`

```rust
pub use crate::compute::ThreadCount;
```

#### Re-export `CPU_MULTI_MIN_WORK_ITEMS`

```rust
pub use crate::compute::CPU_MULTI_MIN_WORK_ITEMS;
```

#### Re-export `GPU_MIN_WORK_ITEMS`

```rust
pub use crate::compute::GPU_MIN_WORK_ITEMS;
```

#### Re-export `field_parallel_crossover`

```rust
pub use crate::fields::parallel::field_parallel_crossover;
```

#### Re-export `should_parallelise`

```rust
pub use crate::fields::parallel::should_parallelise;
```

#### Re-export `FIELD_PARALLEL_CROSSOVER`

```rust
pub use crate::fields::parallel::FIELD_PARALLEL_CROSSOVER;
```

#### Re-export `REDUCTION_CHUNK`

```rust
pub use crate::fields::parallel::REDUCTION_CHUNK;
```

#### Re-export `axpy`

```rust
pub use crate::ldu_matrix::parallel::axpy as ldu_axpy;
```

#### Re-export `dot`

```rust
pub use crate::ldu_matrix::parallel::dot as ldu_dot;
```

#### Re-export `norm_l1`

```rust
pub use crate::ldu_matrix::parallel::norm_l1 as ldu_norm_l1;
```

#### Re-export `norm_l2`

```rust
pub use crate::ldu_matrix::parallel::norm_l2 as ldu_norm_l2;
```

#### Re-export `scale`

```rust
pub use crate::ldu_matrix::parallel::scale as ldu_scale;
```

#### Re-export `spmv_backend_for`

```rust
pub use crate::ldu_matrix::parallel::spmv_backend_for;
```

#### Re-export `vecop_backend_for`

```rust
pub use crate::ldu_matrix::parallel::vecop_backend_for;
```

#### Re-export `HybridLdu`

```rust
pub use crate::ldu_matrix::parallel::HybridLdu;
```

#### Re-export `LduTopology`

```rust
pub use crate::ldu_matrix::parallel::LduTopology;
```

#### Re-export `CELL_BLOCK`

```rust
pub use crate::ldu_matrix::parallel::CELL_BLOCK;
```

#### Re-export `REDUCTION_BLOCK`

```rust
pub use crate::ldu_matrix::parallel::REDUCTION_BLOCK;
```

#### Re-export `SPMV_MIN_CELLS`

```rust
pub use crate::ldu_matrix::parallel::SPMV_MIN_CELLS;
```

#### Re-export `VECOP_MIN_ELEMENTS`

```rust
pub use crate::ldu_matrix::parallel::VECOP_MIN_ELEMENTS;
```

#### Re-export `cubic_roots_batch`

```rust
pub use crate::math::parallel::cubic_roots_batch;
```

#### Re-export `linear_roots_batch`

```rust
pub use crate::math::parallel::linear_roots_batch;
```

#### Re-export `poly_roots_backend_for`

```rust
pub use crate::math::parallel::poly_roots_backend_for;
```

#### Re-export `quadratic_roots_batch`

```rust
pub use crate::math::parallel::quadratic_roots_batch;
```

#### Re-export `root_batch_backend_for`

```rust
pub use crate::math::parallel::root_batch_backend_for;
```

#### Re-export `solve_bracketed_batch`

```rust
pub use crate::math::parallel::solve_bracketed_batch;
```

#### Re-export `solve_newton_batch`

```rust
pub use crate::math::parallel::solve_newton_batch;
```

#### Re-export `RootBatch`

```rust
pub use crate::math::parallel::RootBatch;
```

#### Re-export `RootBatchFailure`

```rust
pub use crate::math::parallel::RootBatchFailure;
```

#### Re-export `RootMethod`

```rust
pub use crate::math::parallel::RootMethod;
```

#### Re-export `RootProblem`

```rust
pub use crate::math::parallel::RootProblem;
```

#### Re-export `RootSettings`

```rust
pub use crate::math::parallel::RootSettings;
```

#### Re-export `RootSolution`

```rust
pub use crate::math::parallel::RootSolution;
```

#### Re-export `RootStatus`

```rust
pub use crate::math::parallel::RootStatus;
```

#### Re-export `golden_section_batch`

```rust
pub use crate::math::minimise::golden_section_batch;
```

#### Re-export `minimise_backend_for`

```rust
pub use crate::math::minimise::minimise_backend_for;
```

#### Re-export `MinBatch`

```rust
pub use crate::math::minimise::MinBatch;
```

#### Re-export `MinBatchFailure`

```rust
pub use crate::math::minimise::MinBatchFailure;
```

#### Re-export `MinProblem`

```rust
pub use crate::math::minimise::MinProblem;
```

#### Re-export `MinSettings`

```rust
pub use crate::math::minimise::MinSettings;
```

#### Re-export `MinSolution`

```rust
pub use crate::math::minimise::MinSolution;
```

#### Re-export `MinStatus`

```rust
pub use crate::math::minimise::MinStatus;
```

#### Re-export `Sense`

```rust
pub use crate::math::minimise::Sense;
```

#### Re-export `GOLDEN_RATIO`

```rust
pub use crate::math::minimise::GOLDEN_RATIO;
```

#### Re-export `MINIMISE_BATCH_MIN_PROBLEMS`

```rust
pub use crate::math::minimise::MINIMISE_BATCH_MIN_PROBLEMS;
```

#### Re-export `SQRT_EPSILON`

```rust
pub use crate::math::minimise::SQRT_EPSILON;
```

#### Re-export `adaptive_quadrature_batch`

```rust
pub use crate::ode::parallel::adaptive_quadrature_batch;
```

#### Re-export `ensemble_backend_for`

```rust
pub use crate::ode::parallel::ensemble_backend_for;
```

#### Re-export `integrate_ensemble`

```rust
pub use crate::ode::parallel::integrate_ensemble;
```

#### Re-export `integrate_ensemble_mixed`

```rust
pub use crate::ode::parallel::integrate_ensemble_mixed;
```

#### Re-export `quadrature_backend_for`

```rust
pub use crate::ode::parallel::quadrature_backend_for;
```

#### Re-export `quadrature_batch`

```rust
pub use crate::ode::parallel::quadrature_batch;
```

#### Re-export `AdaptiveSettings`

```rust
pub use crate::ode::parallel::AdaptiveSettings;
```

#### Re-export `GaussOrder`

```rust
pub use crate::ode::parallel::GaussOrder;
```

#### Re-export `OdeEnsemble`

```rust
pub use crate::ode::parallel::OdeEnsemble;
```

#### Re-export `OdeEnsembleFailure`

```rust
pub use crate::ode::parallel::OdeEnsembleFailure;
```

#### Re-export `OdeLane`

```rust
pub use crate::ode::parallel::OdeLane;
```

#### Re-export `OdeLaneSolution`

```rust
pub use crate::ode::parallel::OdeLaneSolution;
```

#### Re-export `OdeLaneStatus`

```rust
pub use crate::ode::parallel::OdeLaneStatus;
```

#### Re-export `QuadratureBatch`

```rust
pub use crate::ode::parallel::QuadratureBatch;
```

#### Re-export `QuadratureBatchFailure`

```rust
pub use crate::ode::parallel::QuadratureBatchFailure;
```

#### Re-export `QuadratureInterval`

```rust
pub use crate::ode::parallel::QuadratureInterval;
```

#### Re-export `QuadratureRule`

```rust
pub use crate::ode::parallel::QuadratureRule;
```

#### Re-export `QuadratureSolution`

```rust
pub use crate::ode::parallel::QuadratureSolution;
```

#### Re-export `QuadratureStatus`

```rust
pub use crate::ode::parallel::QuadratureStatus;
```

#### Re-export `ODE_ENSEMBLE_MIN_LANES`

```rust
pub use crate::ode::parallel::ODE_ENSEMBLE_MIN_LANES;
```

#### Re-export `QUADRATURE_MIN_INTERVALS`

```rust
pub use crate::ode::parallel::QUADRATURE_MIN_INTERVALS;
```

#### Re-export `derivative`

```rust
pub use crate::math::differentiate::derivative;
```

#### Re-export `derivative_backend_for`

```rust
pub use crate::math::differentiate::derivative_backend_for;
```

#### Re-export `derivative_batch`

```rust
pub use crate::math::differentiate::derivative_batch;
```

#### Re-export `jacobian`

```rust
pub use crate::math::differentiate::jacobian;
```

#### Re-export `jacobian_batch`

```rust
pub use crate::math::differentiate::jacobian_batch;
```

#### Re-export `jacobian_batch_backend_for`

```rust
pub use crate::math::differentiate::jacobian_batch_backend_for;
```

#### Re-export `jacobian_column_backend_for`

```rust
pub use crate::math::differentiate::jacobian_column_backend_for;
```

#### Re-export `ode_system_jacobian`

```rust
pub use crate::math::differentiate::ode_system_jacobian;
```

#### Re-export `DerivativeBatch`

```rust
pub use crate::math::differentiate::DerivativeBatch;
```

#### Re-export `DerivativeSolution`

```rust
pub use crate::math::differentiate::DerivativeSolution;
```

#### Re-export `DiffBatchFailure`

```rust
pub use crate::math::differentiate::DiffBatchFailure;
```

#### Re-export `DiffScheme`

```rust
pub use crate::math::differentiate::DiffScheme;
```

#### Re-export `DiffSettings`

```rust
pub use crate::math::differentiate::DiffSettings;
```

#### Re-export `DiffStatus`

```rust
pub use crate::math::differentiate::DiffStatus;
```

#### Re-export `JacobianBatch`

```rust
pub use crate::math::differentiate::JacobianBatch;
```

#### Re-export `JacobianSolution`

```rust
pub use crate::math::differentiate::JacobianSolution;
```

#### Re-export `NumericalJacobian`

```rust
pub use crate::math::differentiate::NumericalJacobian;
```

#### Re-export `CBRT_EPSILON`

```rust
pub use crate::math::differentiate::CBRT_EPSILON;
```

#### Re-export `DERIVATIVE_BATCH_MIN_POINTS`

```rust
pub use crate::math::differentiate::DERIVATIVE_BATCH_MIN_POINTS;
```

#### Re-export `FIFTH_ROOT_EPSILON`

```rust
pub use crate::math::differentiate::FIFTH_ROOT_EPSILON;
```

#### Re-export `JACOBIAN_BATCH_MIN_PROBLEMS`

```rust
pub use crate::math::differentiate::JACOBIAN_BATCH_MIN_PROBLEMS;
```

#### Re-export `JACOBIAN_COLUMN_MIN_DIMENSION`

```rust
pub use crate::math::differentiate::JACOBIAN_COLUMN_MIN_DIMENSION;
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

