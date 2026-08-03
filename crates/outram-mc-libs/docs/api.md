# Crate Documentation

**Version:** 0.0.2

**Format Version:** 61

# Module `outram_mc_libs`

# outram-mc-libs

Pure-Rust port of selected [OpenMC](https://openmc.org) Monte Carlo
neutron-transport kernels (RNG, geometry/CSG, particle tracking,
k-eigenvalue and fixed-source drivers, delta/Woodcock tracking). Data-free:
cross sections are pulled from `njoy-outram-park-fork`'s `XsProvider` surface.

## License & provenance — read this first

This crate is a **derivative work** of [OpenMC](https://openmc.org),
copyright the OpenMC development team (MIT, Massachusetts Institute of
Technology) and Argonne National Laboratory, licensed MIT. That license
is GPL-compatible, so this translation is distributed under
`GPL-3.0-only` — the same license as the rest of the OUTRAM PARK
workspace, as permitted by the terms of the upstream MIT license.

**This is OUTRAM PARK's independent Rust translation of selected OpenMC
algorithms — it is not the official OpenMC software, and is not
affiliated with, endorsed by, or sanctioned by MIT or Argonne National
Laboratory.** See `TRADEMARKS.md` (this crate's directory, mirrored from
the workspace root) for the full attribution and non-affiliation notice.

## Modules

## Module `rng`

```rust
pub mod rng { /* ... */ }
```

### Modules

## Module `lcg`

```rust
pub mod lcg { /* ... */ }
```

### Types

#### Struct `Lcg64`

Stateful 64-bit LCG — drop-in replacement for `oorandom::Rand64`.

Provides the same interface (`new`, `rand_float`, `rand_u64`) so boon-lay
code can substitute `use outram_mc_libs::rng::lcg::Lcg64 as Rand64` with no
other changes to call sites.

```rust
pub struct Lcg64 {
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
  pub fn new(seed: u128) -> Self { /* ... */ }
  ```
  Create a new generator from a 128-bit seed (matches `oorandom::Rand64::new`).

- ```rust
  pub fn rand_float(self: &mut Self) -> f64 { /* ... */ }
  ```
  Return a uniform sample in [0, 1) and advance the state.

- ```rust
  pub fn rand_u64(self: &mut Self) -> u64 { /* ... */ }
  ```
  Return a raw 64-bit integer and advance the state.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Lcg64 { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Lcg64) -> bool { /* ... */ }
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

#### Function `prn`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Advance the seed one step and return a uniform sample in [0, 1).

Maps to `double prn(uint64_t* seed)` in OpenMC.
The upper 52 bits of the new seed are used to form an IEEE double mantissa,
giving uniform floating-point samples with no division.

```rust
pub fn prn(seed: &mut u64) -> f64 { /* ... */ }
```

#### Function `future_seed`

Advance the seed `n` steps in O(log n) using the LCG jump-ahead identity.

Maps to `uint64_t future_seed(uint64_t n, uint64_t seed)`.
Algorithm: each iteration squares `a` and halves `n`, accumulating the
combined multiplier/increment for odd bits.  Identical to Knuth §3.2.1.

```rust
pub fn future_seed(n: u64, seed: u64) -> u64 { /* ... */ }
```

#### Function `init_seed`

Derive an independent seed for particle `id` from a master seed.

Maps to `uint64_t init_seed(int64_t id, int offset)`.
Each particle gets a unique starting seed by striding from the master seed.

```rust
pub fn init_seed(id: i64, offset: i64, master_seed: i64) -> u64 { /* ... */ }
```

### Constants and Statics

#### Constant `MULT`

Linear Congruential Generator — direct port of `Foam::random_lcg`.

C++ source: `src/random_lcg.cpp`, `include/openmc/random_lcg.h`.

OpenMC uses a 64-bit LCG with modulus 2^64 (implicit wrapping):
  x_{n+1} = MULT * x_n + INC  (mod 2^64)

The jump-ahead feature lets each particle own a completely independent
stream by skipping ahead by a per-particle stride (default 152917).
This is the key technique enabling reproducible parallel Monte Carlo.
LCG multiplier — Knuth's choice (identical to PCG-64).

```rust
pub const MULT: u64 = 6364136223846793005;
```

#### Constant `INC`

LCG additive increment.

```rust
pub const INC: u64 = 1442695040888963407;
```

#### Constant `DEFAULT_STRIDE`

Default per-particle stride (number of RNG draws reserved per particle).

```rust
pub const DEFAULT_STRIDE: u64 = 152917;
```

## Module `distributions`

```rust
pub mod distributions { /* ... */ }
```

### Functions

#### Function `uniform`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Sample a uniform deviate on `[low, high)`.

```rust
pub fn uniform(seed: &mut u64, low: f64, high: f64) -> f64 { /* ... */ }
```

#### Function `sample_normal`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Sample a standard normal deviate N(0,1) via Box-Muller transform.

Uses two uniform draws: `u1 = prn(seed)`, `u2 = prn(seed)`.
Returns `√(−2 ln u1) · cos(2π u2)`.

Drop-in replacement for `rand_distr::StandardNormal` in boon-lay diffusion
modules.  For N(μ, σ²): `μ + σ * sample_normal(seed)`.

```rust
pub fn sample_normal(seed: &mut u64) -> f64 { /* ... */ }
```

#### Function `sample_normal_3d`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Sample a 3-D displacement from N(0, σ²) in each axis independently.

Returns `(dx, dy, dz)` with each component drawn from N(0, σ²).
Used by boon-lay Lagrangian diffusion to advance a particle one step.

```rust
pub fn sample_normal_3d(seed: &mut u64, sigma: f64) -> (f64, f64, f64) { /* ... */ }
```

#### Function `sample_exp`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Sample from an exponential distribution with the given `rate` λ.

Uses inverse-CDF: `x = −ln(u) / λ`.  Mean of the distribution is `1/λ`.

Drop-in replacement for `rand_distr::Exp::new(rate).unwrap().sample(&mut rng)`
in boon-lay collision/scattering modules.

```rust
pub fn sample_exp(seed: &mut u64, rate: f64) -> f64 { /* ... */ }
```

#### Function `maxwell`

Sample a Maxwellian energy distribution: f(E) ∝ √E · exp(−E / θ).
`theta` is the temperature parameter in eV; the returned energy is in eV.

Maps to `double maxwell_spectrum(double T, uint64_t* seed)` in
`src/random_dist.cpp`. Uses the standard three-uniform algorithm: with
r₁,r₂,r₃ ∈ [0,1), `E = −θ·(ln r₁ + ln r₂ · cos²(½π r₃))`, which is exact for
the √E·exp(−E/θ) density (Everett & Cashwell).

```rust
pub fn maxwell(seed: &mut u64, theta: f64) -> f64 { /* ... */ }
```

#### Function `watt`

Sample a Watt fission spectrum: f(E) ∝ exp(−E/a) · sinh(√(b·E)).
`a` in eV, `b` in eV⁻¹; the returned energy is in eV.

Maps to `double watt_spectrum(double a, double b, uint64_t* seed)`. Draws a
Maxwellian `w` with temperature `a`, then shifts it:
`E = w + ¼a²b + (2ξ−1)·√(a²b·w)` (Everett & Cashwell). This is the sampler
OpenMC uses for the prompt-fission source.

```rust
pub fn watt(seed: &mut u64, a: f64, b: f64) -> f64 { /* ... */ }
```

#### Function `isotropic_direction`

Sample an isotropic direction on the unit sphere.

Returns direction cosines `(u, v, w)`. The polar cosine μ is uniform on
[−1, 1] and the azimuth φ uniform on [0, 2π): `(μ, √(1−μ²)·cos φ,
√(1−μ²)·sin φ)`. Maps to `Direction::sample_isotropic` / `isotropic()`.

```rust
pub fn isotropic_direction(seed: &mut u64) -> (f64, f64, f64) { /* ... */ }
```

## Module `geometry`

```rust
pub mod geometry { /* ... */ }
```

### Modules

## Module `position`

```rust
pub mod position { /* ... */ }
```

### Types

#### Struct `Position`

Cartesian position in cm.  Maps to `openmc::Position`.

```rust
pub struct Position {
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

- ```rust
  pub fn dot(self: Self, other: Self) -> f64 { /* ... */ }
  ```

- ```rust
  pub fn norm_sqr(self: Self) -> f64 { /* ... */ }
  ```

- ```rust
  pub fn norm(self: Self) -> f64 { /* ... */ }
  ```

- ```rust
  pub fn distance(self: Self, other: Self) -> f64 { /* ... */ }
  ```
  Distance to another position.

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> Position { /* ... */ }
    ```

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
    fn default() -> Position { /* ... */ }
    ```

- **Div**
  - ```rust
    fn div(self: Self, s: f64) -> Self { /* ... */ }
    ```

- **DivAssign**
  - ```rust
    fn div_assign(self: &mut Self, s: f64) { /* ... */ }
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
- **Mul**
  - ```rust
    fn mul(self: Self, s: f64) -> Self { /* ... */ }
    ```

  - ```rust
    fn mul(self: Self, p: Position) -> Position { /* ... */ }
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
    fn eq(self: &Self, other: &Position) -> bool { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `Direction`

Unit direction vector (direction cosines u, v, w).  Always |d| = 1.
Maps to `openmc::Direction` (which is a typedef for `Position` in OpenMC).

```rust
pub struct Direction {
    pub u: f64,
    pub v: f64,
    pub w: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `u` | `f64` |  |
| `v` | `f64` |  |
| `w` | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn new(u: f64, v: f64, w: f64) -> Self { /* ... */ }
  ```
  Construct a Direction from raw components — caller must ensure |d| ≈ 1.

- ```rust
  pub fn from_unnormalised(x: f64, y: f64, z: f64) -> Self { /* ... */ }
  ```
  Normalise an arbitrary vector to obtain a unit direction.

- ```rust
  pub fn dot_pos(self: Self, p: Position) -> f64 { /* ... */ }
  ```
  Dot product with a `Position` (used for projecting displacement onto direction).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Direction { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Direction) -> bool { /* ... */ }
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

#### Function `stream`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

Advance a position by `distance` along `direction`.

Equivalent to `r + d * distance` — the core operation in particle streaming.

```rust
pub fn stream(pos: Position, dir: Direction, distance: f64) -> Position { /* ... */ }
```

## Module `surface`

```rust
pub mod surface { /* ... */ }
```

### Types

#### Enum `BoundaryType`

Surface boundary condition type.  Maps to `openmc::BoundaryType`.

```rust
pub enum BoundaryType {
    Transmissive,
    Vacuum,
    Reflective,
    Periodic,
    White,
}
```

##### Variants

###### `Transmissive`

###### `Vacuum`

###### `Reflective`

###### `Periodic`

###### `White`

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
    fn clone(self: &Self) -> BoundaryType { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &BoundaryType) -> bool { /* ... */ }
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
#### Struct `XPlane`

Infinite plane perpendicular to the X axis: x = x0.

```rust
pub struct XPlane {
    pub x0: f64,
    pub bc: BoundaryType,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x0` | `f64` |  |
| `bc` | `BoundaryType` |  |

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
- **Surface**
  - ```rust
    fn evaluate(self: &Self, r: Position) -> f64 { /* ... */ }
    ```

  - ```rust
    fn normal(self: &Self, _r: Position) -> Direction { /* ... */ }
    ```

  - ```rust
    fn distance(self: &Self, r: Position, u: Direction, coincident: bool) -> f64 { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `YPlane`

Infinite plane perpendicular to the Y axis: y = y0.

```rust
pub struct YPlane {
    pub y0: f64,
    pub bc: BoundaryType,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `y0` | `f64` |  |
| `bc` | `BoundaryType` |  |

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
- **Surface**
  - ```rust
    fn evaluate(self: &Self, r: Position) -> f64 { /* ... */ }
    ```

  - ```rust
    fn normal(self: &Self, _r: Position) -> Direction { /* ... */ }
    ```

  - ```rust
    fn distance(self: &Self, r: Position, u: Direction, coincident: bool) -> f64 { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `ZPlane`

Infinite plane perpendicular to the Z axis: z = z0.

```rust
pub struct ZPlane {
    pub z0: f64,
    pub bc: BoundaryType,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `z0` | `f64` |  |
| `bc` | `BoundaryType` |  |

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
- **Surface**
  - ```rust
    fn evaluate(self: &Self, r: Position) -> f64 { /* ... */ }
    ```

  - ```rust
    fn normal(self: &Self, _r: Position) -> Direction { /* ... */ }
    ```

  - ```rust
    fn distance(self: &Self, r: Position, u: Direction, coincident: bool) -> f64 { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `Sphere`

Sphere: (x-x0)² + (y-y0)² + (z-z0)² = r²

```rust
pub struct Sphere {
    pub x0: f64,
    pub y0: f64,
    pub z0: f64,
    pub r: f64,
    pub bc: BoundaryType,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x0` | `f64` |  |
| `y0` | `f64` |  |
| `z0` | `f64` |  |
| `r` | `f64` |  |
| `bc` | `BoundaryType` |  |

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
- **Surface**
  - ```rust
    fn evaluate(self: &Self, r: Position) -> f64 { /* ... */ }
    ```

  - ```rust
    fn normal(self: &Self, r: Position) -> Direction { /* ... */ }
    ```

  - ```rust
    fn distance(self: &Self, r: Position, u: Direction, coincident: bool) -> f64 { /* ... */ }
    ```
    Smallest positive distance from `r` along `u` to the sphere.

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
#### Struct `ZCylinder`

Infinite cylinder along the Z axis: (x-x0)² + (y-y0)² = r²

```rust
pub struct ZCylinder {
    pub x0: f64,
    pub y0: f64,
    pub r: f64,
    pub bc: BoundaryType,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x0` | `f64` |  |
| `y0` | `f64` |  |
| `r` | `f64` |  |
| `bc` | `BoundaryType` |  |

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
- **Surface**
  - ```rust
    fn evaluate(self: &Self, r: Position) -> f64 { /* ... */ }
    ```

  - ```rust
    fn normal(self: &Self, r: Position) -> Direction { /* ... */ }
    ```

  - ```rust
    fn distance(self: &Self, r: Position, u: Direction, coincident: bool) -> f64 { /* ... */ }
    ```
    Smallest positive distance from `r` along `u` to the infinite Z cylinder.

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
#### Struct `Plane`

General plane: A·x + B·y + C·z = D.

The unrestricted-orientation plane (the axis-aligned [`XPlane`]/[`YPlane`]/
[`ZPlane`] are the cheap special cases). Maps to OpenMC `SurfacePlane`
(`src/surface.cpp`). `(A, B, C)` need not be unit — [`Surface::normal`]
normalises them.

```rust
pub struct Plane {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub bc: BoundaryType,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `f64` |  |
| `b` | `f64` |  |
| `c` | `f64` |  |
| `d` | `f64` |  |
| `bc` | `BoundaryType` |  |

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
- **Surface**
  - ```rust
    fn evaluate(self: &Self, r: Position) -> f64 { /* ... */ }
    ```

  - ```rust
    fn normal(self: &Self, _r: Position) -> Direction { /* ... */ }
    ```

  - ```rust
    fn distance(self: &Self, r: Position, u: Direction, coincident: bool) -> f64 { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `XCylinder`

Infinite cylinder along the X axis: (y-y0)² + (z-z0)² = r².

The X-axis twin of [`ZCylinder`]; same intersection algebra with the radial
pair `(y, z)` and the parallel axis `x`. Ported from OpenMC
`axis_aligned_cylinder_distance<0,1,2>` (`src/surface.cpp`).

```rust
pub struct XCylinder {
    pub y0: f64,
    pub z0: f64,
    pub r: f64,
    pub bc: BoundaryType,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `y0` | `f64` |  |
| `z0` | `f64` |  |
| `r` | `f64` |  |
| `bc` | `BoundaryType` |  |

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
- **Surface**
  - ```rust
    fn evaluate(self: &Self, r: Position) -> f64 { /* ... */ }
    ```

  - ```rust
    fn normal(self: &Self, r: Position) -> Direction { /* ... */ }
    ```

  - ```rust
    fn distance(self: &Self, r: Position, u: Direction, coincident: bool) -> f64 { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `YCylinder`

Infinite cylinder along the Y axis: (x-x0)² + (z-z0)² = r².

The Y-axis twin of [`ZCylinder`]; radial pair `(x, z)`, parallel axis `y`.
Ported from OpenMC `axis_aligned_cylinder_distance<1,0,2>` (`src/surface.cpp`).

```rust
pub struct YCylinder {
    pub x0: f64,
    pub z0: f64,
    pub r: f64,
    pub bc: BoundaryType,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x0` | `f64` |  |
| `z0` | `f64` |  |
| `r` | `f64` |  |
| `bc` | `BoundaryType` |  |

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
- **Surface**
  - ```rust
    fn evaluate(self: &Self, r: Position) -> f64 { /* ... */ }
    ```

  - ```rust
    fn normal(self: &Self, r: Position) -> Direction { /* ... */ }
    ```

  - ```rust
    fn distance(self: &Self, r: Position, u: Direction, coincident: bool) -> f64 { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `ZCone`

Double-napped cone about the Z axis: (x-x0)² + (y-y0)² = r_sq·(z-z0)².

`r_sq` is the **square of the slope** (tan² of the half-opening-angle), the
same parameterisation OpenMC `SurfaceZCone` stores (`src/surface.cpp`). The
surface is the full double cone (both naps); a single nap is selected in CSG
by intersecting with a half-space (e.g. `z > z0`).

```rust
pub struct ZCone {
    pub x0: f64,
    pub y0: f64,
    pub z0: f64,
    pub r_sq: f64,
    pub bc: BoundaryType,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x0` | `f64` |  |
| `y0` | `f64` |  |
| `z0` | `f64` |  |
| `r_sq` | `f64` |  |
| `bc` | `BoundaryType` |  |

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
- **Surface**
  - ```rust
    fn evaluate(self: &Self, r: Position) -> f64 { /* ... */ }
    ```

  - ```rust
    fn normal(self: &Self, r: Position) -> Direction { /* ... */ }
    ```

  - ```rust
    fn distance(self: &Self, r: Position, u: Direction, coincident: bool) -> f64 { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `XCone`

Double-napped cone about the X axis: (y-y0)² + (z-z0)² = r_sq·(x-x0)².

X-axis twin of [`ZCone`]; `r_sq` is the slope². Ported from OpenMC
`SurfaceXCone` (`src/surface.cpp`).

```rust
pub struct XCone {
    pub x0: f64,
    pub y0: f64,
    pub z0: f64,
    pub r_sq: f64,
    pub bc: BoundaryType,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x0` | `f64` |  |
| `y0` | `f64` |  |
| `z0` | `f64` |  |
| `r_sq` | `f64` |  |
| `bc` | `BoundaryType` |  |

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
- **Surface**
  - ```rust
    fn evaluate(self: &Self, r: Position) -> f64 { /* ... */ }
    ```

  - ```rust
    fn normal(self: &Self, r: Position) -> Direction { /* ... */ }
    ```

  - ```rust
    fn distance(self: &Self, r: Position, u: Direction, coincident: bool) -> f64 { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `YCone`

Double-napped cone about the Y axis: (x-x0)² + (z-z0)² = r_sq·(y-y0)².

Y-axis twin of [`ZCone`]; `r_sq` is the slope². Ported from OpenMC
`SurfaceYCone` (`src/surface.cpp`).

```rust
pub struct YCone {
    pub x0: f64,
    pub y0: f64,
    pub z0: f64,
    pub r_sq: f64,
    pub bc: BoundaryType,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x0` | `f64` |  |
| `y0` | `f64` |  |
| `z0` | `f64` |  |
| `r_sq` | `f64` |  |
| `bc` | `BoundaryType` |  |

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
- **Surface**
  - ```rust
    fn evaluate(self: &Self, r: Position) -> f64 { /* ... */ }
    ```

  - ```rust
    fn normal(self: &Self, r: Position) -> Direction { /* ... */ }
    ```

  - ```rust
    fn distance(self: &Self, r: Position, u: Direction, coincident: bool) -> f64 { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `Quadric`

General quadric: A x² + B y² + C z² + D xy + E yz + F xz + G x + H y + J z + K = 0.

The most general second-order surface — every other surface here is a special
case, but the explicit forms above are cheaper and are preferred when the
geometry allows. Maps to OpenMC `SurfaceQuadric` (`src/surface.cpp`).

```rust
pub struct Quadric {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
    pub g: f64,
    pub h: f64,
    pub j: f64,
    pub k: f64,
    pub bc: BoundaryType,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `f64` |  |
| `b` | `f64` |  |
| `c` | `f64` |  |
| `d` | `f64` |  |
| `e` | `f64` |  |
| `f` | `f64` |  |
| `g` | `f64` |  |
| `h` | `f64` |  |
| `j` | `f64` |  |
| `k` | `f64` |  |
| `bc` | `BoundaryType` |  |

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
- **Surface**
  - ```rust
    fn evaluate(self: &Self, r: Position) -> f64 { /* ... */ }
    ```

  - ```rust
    fn normal(self: &Self, r: Position) -> Direction { /* ... */ }
    ```

  - ```rust
    fn distance(self: &Self, r: Position, u: Direction, coincident: bool) -> f64 { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `ZTorus`

Torus about the **Z** axis, centred at `(x0, y0, z0)`.

Radial pair `(x, y)`, axial `z`:
  `((sqrt((x−x0)² + (y−y0)²) − a) / b)² + ((z−z0) / c)² − 1 = 0`.

`a` = major radius, `b` = in-plane minor radius, `c` = axial minor radius
(all cm). Maps to OpenMC `SurfaceZTorus` (`src/surface.cpp`).

```rust
pub struct ZTorus {
    pub x0: f64,
    pub y0: f64,
    pub z0: f64,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub bc: BoundaryType,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x0` | `f64` |  |
| `y0` | `f64` |  |
| `z0` | `f64` |  |
| `a` | `f64` |  |
| `b` | `f64` |  |
| `c` | `f64` |  |
| `bc` | `BoundaryType` |  |

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
- **Surface**
  - ```rust
    fn evaluate(self: &Self, r: Position) -> f64 { /* ... */ }
    ```

  - ```rust
    fn normal(self: &Self, r: Position) -> Direction { /* ... */ }
    ```

  - ```rust
    fn distance(self: &Self, r: Position, u: Direction, coincident: bool) -> f64 { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `XTorus`

Torus about the **X** axis, centred at `(x0, y0, z0)`.

Radial pair `(y, z)`, axial `x`:
  `((sqrt((y−y0)² + (z−z0)²) − a) / b)² + ((x−x0) / c)² − 1 = 0`.
X-axis twin of [`ZTorus`]; maps to OpenMC `SurfaceXTorus` (`src/surface.cpp`).

```rust
pub struct XTorus {
    pub x0: f64,
    pub y0: f64,
    pub z0: f64,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub bc: BoundaryType,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x0` | `f64` |  |
| `y0` | `f64` |  |
| `z0` | `f64` |  |
| `a` | `f64` |  |
| `b` | `f64` |  |
| `c` | `f64` |  |
| `bc` | `BoundaryType` |  |

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
- **Surface**
  - ```rust
    fn evaluate(self: &Self, r: Position) -> f64 { /* ... */ }
    ```

  - ```rust
    fn normal(self: &Self, r: Position) -> Direction { /* ... */ }
    ```

  - ```rust
    fn distance(self: &Self, r: Position, u: Direction, coincident: bool) -> f64 { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `YTorus`

Torus about the **Y** axis, centred at `(x0, y0, z0)`.

Radial pair `(x, z)`, axial `y`:
  `((sqrt((x−x0)² + (z−z0)²) − a) / b)² + ((y−y0) / c)² − 1 = 0`.
Y-axis twin of [`ZTorus`]; maps to OpenMC `SurfaceYTorus` (`src/surface.cpp`).

```rust
pub struct YTorus {
    pub x0: f64,
    pub y0: f64,
    pub z0: f64,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub bc: BoundaryType,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x0` | `f64` |  |
| `y0` | `f64` |  |
| `z0` | `f64` |  |
| `a` | `f64` |  |
| `b` | `f64` |  |
| `c` | `f64` |  |
| `bc` | `BoundaryType` |  |

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
- **Surface**
  - ```rust
    fn evaluate(self: &Self, r: Position) -> f64 { /* ... */ }
    ```

  - ```rust
    fn normal(self: &Self, r: Position) -> Direction { /* ... */ }
    ```

  - ```rust
    fn distance(self: &Self, r: Position, u: Direction, coincident: bool) -> f64 { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Enum `SurfaceKind`

A CSG quadric surface — the closed set the geometry navigator dispatches over.

Wraps each concrete surface struct. Maps to the OpenMC `Surface` polymorphic
hierarchy (`src/surface.cpp`), realised here as an enum so `match` gives
exhaustiveness and rust-analyzer go-to-definition on every variant.

```rust
pub enum SurfaceKind {
    XPlane(XPlane),
    YPlane(YPlane),
    ZPlane(ZPlane),
    Plane(Plane),
    Sphere(Sphere),
    XCylinder(XCylinder),
    YCylinder(YCylinder),
    ZCylinder(ZCylinder),
    XCone(XCone),
    YCone(YCone),
    ZCone(ZCone),
    Quadric(Quadric),
    XTorus(XTorus),
    YTorus(YTorus),
    ZTorus(ZTorus),
}
```

##### Variants

###### `XPlane`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `XPlane` |  |

###### `YPlane`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `YPlane` |  |

###### `ZPlane`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `ZPlane` |  |

###### `Plane`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Plane` |  |

###### `Sphere`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Sphere` |  |

###### `XCylinder`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `XCylinder` |  |

###### `YCylinder`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `YCylinder` |  |

###### `ZCylinder`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `ZCylinder` |  |

###### `XCone`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `XCone` |  |

###### `YCone`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `YCone` |  |

###### `ZCone`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `ZCone` |  |

###### `Quadric`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Quadric` |  |

###### `XTorus`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `XTorus` |  |

###### `YTorus`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `YTorus` |  |

###### `ZTorus`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `ZTorus` |  |

##### Implementations

###### Methods

- ```rust
  pub fn evaluate(self: &Self, r: Position) -> f64 { /* ... */ }
  ```
  Signed surface sense at `r`: negative inside, positive outside.

- ```rust
  pub fn sense(self: &Self, r: Position) -> bool { /* ... */ }
  ```
  Boolean sense used by cell membership: `true` = positive (outside) half-space.

- ```rust
  pub fn distance(self: &Self, r: Position, u: Direction, coincident: bool) -> f64 { /* ... */ }
  ```
  Smallest positive distance along ray `(r, u)` to this surface, or

- ```rust
  pub fn normal(self: &Self, r: Position) -> Direction { /* ... */ }
  ```
  Outward unit normal at `r` (assumes `r` lies on the surface).

- ```rust
  pub fn reflect(self: &Self, r: Position, u: Direction) -> Direction { /* ... */ }
  ```
  Specular reflection of direction `u` off this surface at `r`.

- ```rust
  pub fn bc(self: &Self) -> BoundaryType { /* ... */ }
  ```
  This surface's boundary condition.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
### Traits

#### Trait `Surface`

Trait all surfaces must implement.  Maps to the virtual `Surface` base class.

```rust
pub trait Surface: Send + Sync {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `evaluate`: Evaluate the surface equation at `r`. Negative = inside the surface.
- `distance`: Smallest positive distance along ray `(r, u)` to this surface.
- `normal`: Outward unit normal at point `r` (assumes `r` is on the surface).

##### Provided Methods

- ```rust
  fn reflect(self: &Self, r: Position, u: Direction) -> Direction { /* ... */ }
  ```
  Reflect direction `u` off this surface at position `r`.

##### Implementations

This trait is implemented for the following types:

- `XPlane`
- `YPlane`
- `ZPlane`
- `Sphere`
- `ZCylinder`
- `Plane`
- `XCylinder`
- `YCylinder`
- `ZCone`
- `XCone`
- `YCone`
- `Quadric`
- `ZTorus`
- `XTorus`
- `YTorus`

## Module `cell`

```rust
pub mod cell { /* ... */ }
```

### Types

#### Enum `HalfSpaceSense`

Which side of a surface a half-space token selects.

```rust
pub enum HalfSpaceSense {
    Inside,
    Outside,
}
```

##### Variants

###### `Inside`

Negative side, `evaluate(r) < 0` — the interior of a sphere/cylinder.

###### `Outside`

Positive side, `evaluate(r) > 0` — the exterior.

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
    fn clone(self: &Self) -> HalfSpaceSense { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &HalfSpaceSense) -> bool { /* ... */ }
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
#### Enum `RegionToken`

One token in the RPN region definition. Maps to OpenMC's region token stream
(`src/cell.cpp`), but with the operators named rather than encoded as the
sentinel negative integers OpenMC uses.

```rust
pub enum RegionToken {
    HalfSpace {
        surface_idx: usize,
        sense: HalfSpaceSense,
    },
    Intersection,
    Union,
    Complement,
}
```

##### Variants

###### `HalfSpace`

Half-space of surface `surface_idx` (index into the global surface array).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `surface_idx` | `usize` |  |
| `sense` | `HalfSpaceSense` |  |

###### `Intersection`

Logical AND of the two operands below it on the stack.

###### `Union`

Logical OR of the two operands below it on the stack.

###### `Complement`

Logical NOT of the single operand below it on the stack.

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
    fn clone(self: &Self) -> RegionToken { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &RegionToken) -> bool { /* ... */ }
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
#### Enum `CellFill`

What fills a cell. Maps to OpenMC's `Cell::type_` / `Fill`.

```rust
pub enum CellFill {
    Material(usize),
    Universe(usize),
    Lattice(usize),
    Void,
}
```

##### Variants

###### `Material`

Filled with a material (index into the materials list).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

###### `Universe`

Filled with a nested universe (index into the universe array).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

###### `Lattice`

Filled with a lattice (index into the lattice array).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

###### `Void`

Void — no material, streams freely.

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
    fn clone(self: &Self) -> CellFill { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &CellFill) -> bool { /* ... */ }
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
#### Struct `Cell`

A CSG cell. Maps to `openmc::Cell`.

```rust
pub struct Cell {
    pub id: i32,
    pub region: Vec<RegionToken>,
    pub fill: CellFill,
    pub temperature: f64,
    pub translation: super::position::Position,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `id` | `i32` | User-facing cell id (for reporting/tallies). |
| `region` | `Vec<RegionToken>` | Region definition as an RPN token stream (see [`RegionToken`]). |
| `fill` | `CellFill` | What the cell is filled with. |
| `temperature` | `f64` | Temperature of this cell in Kelvin (passed to the Doppler XS lookup). |
| `translation` | `super::position::Position` | Rigid translation \[cm\] applied to a fill universe's local frame<br>(`coord.r -= translation`). Zero for material cells and untranslated fills.<br>Mirrors `Cell::translation_` in `src/cell.cpp`. |

##### Implementations

###### Methods

- ```rust
  pub fn material(id: i32, region: Vec<RegionToken>, material_idx: usize, temperature: f64) -> Self { /* ... */ }
  ```
  Build a material cell with no translation — the common leaf case.

- ```rust
  pub fn fill(id: i32, region: Vec<RegionToken>, fill: CellFill, translation: Position) -> Self { /* ... */ }
  ```
  Build a fill cell (nested universe or lattice) with an optional translation.

- ```rust
  pub fn contains(self: &Self, r: Position, surfaces: &[SurfaceKind]) -> bool { /* ... */ }
  ```
  Whether position `r` lies inside this cell's region.

- ```rust
  pub fn distance_to_boundary(self: &Self, r: Position, u: Direction, surfaces: &[SurfaceKind], on_surface: usize) -> (f64, usize) { /* ... */ }
  ```
  Distance along ray `(r, u)` to the nearest surface bounding this cell.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
## Module `universe`

```rust
pub mod universe { /* ... */ }
```

### Types

#### Struct `Universe`

A universe — an ordered list of cells searched top-to-bottom.
Maps to `openmc::Universe`.

```rust
pub struct Universe {
    pub id: i32,
    pub cell_indices: Vec<usize>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `id` | `i32` | User-facing universe id. |
| `cell_indices` | `Vec<usize>` | Indices into the global cell array, in search order. |

##### Implementations

###### Methods

- ```rust
  pub fn find_cell(self: &Self, r: Position, surfaces: &[SurfaceKind], cells: &[Cell]) -> Option<usize> { /* ... */ }
  ```
  Find the first cell in this universe that contains `r` (in this universe's

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
## Module `lattice`

Rectangular and hexagonal lattices.

C++ source: `src/lattice.cpp` (1219 LOC), `include/openmc/lattice.h`.

A lattice tiles space with identical universes on a periodic grid. OpenMC
supports two types:
  - `RectLattice` — 3-D rectangular grid (nx × ny × nz pitches)
  - `HexLattice`  — 2-D hexagonal grid (axial rings + axial levels)

Each lattice element maps to a universe index. The lattice is itself a
special kind of universe fill: [`crate::geometry::geometry::Geometry`]
descends into it exactly as it would a nested universe.

```rust
pub mod lattice { /* ... */ }
```

### Types

#### Enum `LatticeType`

Lattice type tag. Maps to `openmc::LatticeType`.

```rust
pub enum LatticeType {
    Rect,
    Hex,
}
```

##### Variants

###### `Rect`

###### `Hex`

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
    fn clone(self: &Self) -> LatticeType { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &LatticeType) -> bool { /* ... */ }
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
#### Struct `RectLattice`

A rectangular lattice. Maps to `openmc::RectLattice`.

```rust
pub struct RectLattice {
    pub id: i32,
    pub n: [usize; 3],
    pub lower_left: super::position::Position,
    pub pitch: [f64; 3],
    pub universes: Vec<usize>,
    pub outer: Option<usize>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `id` | `i32` | User-facing lattice id. |
| `n` | `[usize; 3]` | Number of grid cells in x, y, z (z = 1 for a 2-D lattice). |
| `lower_left` | `super::position::Position` | Lower-left corner of the lattice in cm. |
| `pitch` | `[f64; 3]` | Pitch (cell width) in cm for each axis. |
| `universes` | `Vec<usize>` | Universe index for each lattice element, row-major flat index<br>`nx*ny*iz + nx*iy + ix`. |
| `outer` | `Option<usize>` | Universe filling the region outside the grid (`None` ⇒ no outer; a<br>particle leaving the grid is lost). Maps to `Lattice::outer_`. |

##### Implementations

###### Methods

- ```rust
  pub fn get_indices(self: &Self, r: Position, u: Direction) -> [i32; 3] { /* ... */ }
  ```
  Map a position to a (possibly out-of-range, signed) lattice index triplet.

- ```rust
  pub fn are_valid_indices(self: &Self, i: [i32; 3]) -> bool { /* ... */ }
  ```
  Whether a signed index triplet is inside the grid.

- ```rust
  pub fn universe_at(self: &Self, i: [i32; 3]) -> Option<usize> { /* ... */ }
  ```
  The universe index at tile `i` — the tile's universe if in range, else the

- ```rust
  pub fn get_local_position(self: &Self, r: Position, i: [i32; 3]) -> Position { /* ... */ }
  ```
  Position of `r` recentred into the local frame of tile `i` (tile centre at

- ```rust
  pub fn distance(self: &Self, r: Position, u: Direction) -> (f64, [i32; 3]) { /* ... */ }
  ```
  Distance \[cm\] to the next lattice-tile boundary along `(r, u)`, with `r`

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
#### Enum `HexOrientation`

Orientation of a hexagonal lattice. Maps to `openmc::HexLattice::Orientation`
(`include/openmc/lattice.h:296`).

- [`HexOrientation::Y`] — two sides of every tile are parallel to the y-axis
  (OpenMC default). Flat tile edges face ±x.
- [`HexOrientation::X`] — two sides parallel to the x-axis; the first element
  of each ring starts along +x.

```rust
pub enum HexOrientation {
    Y,
    X,
}
```

##### Variants

###### `Y`

Sides parallel to the y-axis (OpenMC default).

###### `X`

Sides parallel to the x-axis.

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
    fn clone(self: &Self) -> HexOrientation { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &HexOrientation) -> bool { /* ... */ }
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
#### Struct `HexLattice`

A hexagonal lattice. Maps to `openmc::HexLattice`.

C++ source: `src/lattice.cpp:456` (constructor) and the `HexLattice::*`
methods that follow it, `include/openmc/lattice.h:253`.

# What it represents

A hexagonal lattice tiles the plane with `3*n_rings*(n_rings-1) + 1`
hexagonal tiles arranged in `n_rings` concentric rings (the innermost "ring"
is the single central tile). Each tile maps to a universe index. Optionally
the lattice is stacked `n_axial` times along z.

# Indexing (this is the crux)

Internally OpenMC stores the tiles in a **skewed** `(2*n_rings-1) x
(2*n_rings-1)` *square* array, with the unused corner entries set to
[`HEX_NONE`]. A tile is addressed by a signed index triplet `[ix, iy, iz]`
where `ix, iy` are the two skewed lattice axes offset by `n_rings-1` (so the
central tile is `[n_rings-1, n_rings-1, 0]`) and `iz` is the axial level. The
flat storage index is
`(2*n_rings-1)^2 * iz + (2*n_rings-1) * iy + ix` (see
[`Self::flat_index`]). Membership in the hexagon (as opposed to a skipped
corner) is [`Self::are_valid_indices`].

Units: `center`/`pitch` in cm.

```rust
pub struct HexLattice {
    pub id: i32,
    pub orientation: HexOrientation,
    pub n_rings: usize,
    pub n_axial: usize,
    pub center: super::position::Position,
    pub pitch: [f64; 2],
    pub universes: Vec<i32>,
    pub outer: Option<usize>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `id` | `i32` | User-facing lattice id. |
| `orientation` | `HexOrientation` | Orientation of the tiles (see [`HexOrientation`]). |
| `n_rings` | `usize` | Number of radial rings (the central tile is the innermost ring). |
| `n_axial` | `usize` | Number of axial levels (`1` for a 2-D lattice). |
| `center` | `super::position::Position` | Lattice centre in cm. `z` is only used when `n_axial > 1`. |
| `pitch` | `[f64; 2]` | `[radial_pitch, axial_pitch]` in cm. `pitch[1]` is only used when 3-D. |
| `universes` | `Vec<i32>` | Universe index for each tile in the skewed square array (row-major:<br>`(2*n_rings-1)^2 * iz + (2*n_rings-1) * iy + ix`). Unused corners hold<br>[`HEX_NONE`]. Valid entries are non-negative universe indices. |
| `outer` | `Option<usize>` | Universe filling everything outside the hexagon (`None` ⇒ a particle<br>leaving the lattice is lost). Maps to `Lattice::outer_`. |

##### Implementations

###### Methods

- ```rust
  pub fn flat_index(self: &Self, i: [i32; 3]) -> usize { /* ... */ }
  ```
  Flat storage index of tile `[ix, iy, iz]`. Ported from

- ```rust
  pub fn are_valid_indices(self: &Self, i: [i32; 3]) -> bool { /* ... */ }
  ```
  Whether a signed index triplet addresses a real tile inside the hexagon.

- ```rust
  pub fn universe_at(self: &Self, i: [i32; 3]) -> Option<usize> { /* ... */ }
  ```
  The universe index at tile `i` — the tile's universe if it is a valid,

- ```rust
  pub fn get_local_position(self: &Self, r: Position, i: [i32; 3]) -> Position { /* ... */ }
  ```
  Position of `r` recentred into the local frame of tile `i` (tile centre at

- ```rust
  pub fn get_indices(self: &Self, r: Position, u: Direction) -> [i32; 3] { /* ... */ }
  ```
  Map a position + direction to a (possibly out-of-range) skewed index

- ```rust
  pub fn distance(self: &Self, r_local: Position, u: Direction, i_xyz: [i32; 3]) -> (f64, [i32; 3]) { /* ... */ }
  ```
  Distance \[cm\] to the next lattice-tile boundary along `(r, u)`, plus the

- ```rust
  pub fn from_rings(id: i32, orientation: HexOrientation, center: Position, radial_pitch: f64, rings: &[Vec<usize>], outer: Option<usize>) -> Self { /* ... */ }
  ```
  Build a 2-D hexagonal lattice from the user-facing **ring** description,

- ```rust
  pub fn from_rings_3d(id: i32, orientation: HexOrientation, center: Position, radial_pitch: f64, axial_pitch: f64, levels: &[Vec<Vec<usize>>], outer: Option<usize>) -> Self { /* ... */ }
  ```
  Build a **3-D** hexagonal lattice: `levels.len()` axially-stacked copies

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
#### Enum `Lattice`

A lattice fill — dispatched by enum, not a trait object (per the workspace
"enums over `dyn`" rule). [`crate::geometry::geometry::Geometry`] holds a
`Vec<Lattice>` and matches on the variant during descent.

```rust
pub enum Lattice {
    Rect(RectLattice),
    Hex(HexLattice),
}
```

##### Variants

###### `Rect`

A rectangular lattice.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `RectLattice` |  |

###### `Hex`

A hexagonal lattice.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `HexLattice` |  |

##### Implementations

###### Methods

- ```rust
  pub fn id(self: &Self) -> i32 { /* ... */ }
  ```
  The user-facing lattice id.

- ```rust
  pub fn get_indices(self: &Self, r: Position, u: Direction) -> [i32; 3] { /* ... */ }
  ```
  Skewed/signed tile index for `(r, u)` in this lattice's local frame.

- ```rust
  pub fn universe_at(self: &Self, i: [i32; 3]) -> Option<usize> { /* ... */ }
  ```
  Universe index at tile `i` (tile universe, else `outer`, else `None`).

- ```rust
  pub fn get_local_position(self: &Self, r: Position, i: [i32; 3]) -> Position { /* ... */ }
  ```
  Position `r` recentred into tile `i`'s local frame (tile centre at origin).

- ```rust
  pub fn distance(self: &Self, r: Position, u: Direction, i_xyz: [i32; 3]) -> (f64, [i32; 3]) { /* ... */ }
  ```
  Distance to the next tile boundary along `(r, u)` from tile `i_xyz`, with

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
### Constants and Statics

#### Constant `HEX_NONE`

Sentinel for an unused entry in a [`HexLattice`]'s "square" universe array —
the corner cells that fall outside the hexagon. Mirrors OpenMC's `C_NONE`
(`-1`) fill marker (`include/openmc/constants.h`).

```rust
pub const HEX_NONE: i32 = -1;
```

## Module `geometry`

High-level geometry navigation: particle location and boundary crossing.

C++ source: `src/geometry.cpp` (495 LOC), `include/openmc/geometry.h`.

These are the two innermost queries of the transport algorithm:
  1. [`Geometry::locate`] — descend the universe/lattice hierarchy from the
     root universe to find the leaf cell and its material at a point (ported
     from `find_cell_inner`, `src/geometry.cpp:102`).
  2. [`Geometry::distance_to_boundary`] — over every coordinate level, find
     the nearest surface **or** lattice-tile crossing (ported from
     `distance_to_boundary`, `src/geometry.cpp:361`).

A [`Geometry`] owns the flat arrays every index refers to: `surfaces`,
`cells`, `universes`, `lattices`, plus the `root_universe`. It is read-only
after construction, so transport threads share it as `Arc<Geometry>`.

```rust
pub mod geometry { /* ... */ }
```

### Types

#### Struct `Coord`

One coordinate level in a located particle's nesting chain.

Mirrors an OpenMC `LocalCoord`: the universe searched at this level, the cell
found there, and the particle's position/direction expressed in that level's
local frame. `lattice` is `Some` when this level's universe was reached by
descending into a lattice (so a lattice-tile crossing is possible here).

```rust
pub struct Coord {
    pub universe: usize,
    pub cell: usize,
    pub r: super::position::Position,
    pub u: super::position::Direction,
    pub lattice: Option<usize>,
    pub lattice_index: [i32; 3],
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `universe` | `usize` | Universe index searched at this level. |
| `cell` | `usize` | Cell index (global) found containing the particle at this level. |
| `r` | `super::position::Position` | Position \[cm\] in this level's local frame. |
| `u` | `super::position::Direction` | Direction (unit) in this level's local frame. |
| `lattice` | `Option<usize>` | Lattice index if this level was entered via a lattice, else `None`. |
| `lattice_index` | `[i32; 3]` | Lattice tile index `[ix, iy, iz]` for this level (only meaningful if<br>`lattice` is `Some`). |

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
    fn clone(self: &Self) -> Coord { /* ... */ }
    ```

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
#### Struct `GeometryPath`

A fully located particle: its coordinate-level chain plus the leaf material.

```rust
pub struct GeometryPath {
    pub levels: Vec<Coord>,
    pub material: Option<usize>,
    pub on_surface: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `levels` | `Vec<Coord>` | Coordinate levels from root (index 0) down to the material leaf. |
| `material` | `Option<usize>` | Leaf material index, or `None` for a void cell. |
| `on_surface` | `usize` | Global surface index the particle currently sits on (`usize::MAX` if none),<br>used for coincident-distance handling. |

##### Implementations

###### Methods

- ```rust
  pub fn leaf(self: &Self) -> &Coord { /* ... */ }
  ```
  The leaf (lowest) coordinate level — where the material fill lives.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
#### Enum `Crossing`

What the nearest boundary along a flight is.

```rust
pub enum Crossing {
    Surface(usize),
    Lattice,
    None,
}
```

##### Variants

###### `Surface`

A CSG surface with this global index is crossed.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

###### `Lattice`

A lattice-tile boundary is crossed (re-locate into the neighbouring tile).

###### `None`

No boundary within a finite distance (particle streams to infinity).

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
    fn clone(self: &Self) -> Crossing { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Crossing) -> bool { /* ... */ }
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
#### Struct `BoundaryHit`

Result of a [`Geometry::distance_to_boundary`] query.

```rust
pub struct BoundaryHit {
    pub distance: f64,
    pub crossing: Crossing,
    pub coord_level: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `distance` | `f64` | Distance \[cm\] to the nearest boundary (`INFINITY` if none). |
| `crossing` | `Crossing` | What is crossed at that distance. |
| `coord_level` | `usize` | Coordinate level (index into [`GeometryPath::levels`]) of the crossing. |

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
    fn clone(self: &Self) -> BoundaryHit { /* ... */ }
    ```

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
#### Struct `Geometry`

The whole CSG model — the flat arrays every geometry index refers to.

Read-only after construction; share across threads as `Arc<Geometry>`.
Maps to OpenMC's `model::{surfaces,cells,universes,lattices}` globals plus
`model::root_universe`.

```rust
pub struct Geometry {
    pub surfaces: Vec<super::surface::SurfaceKind>,
    pub cells: Vec<crate::geometry::cell::Cell>,
    pub universes: Vec<super::universe::Universe>,
    pub lattices: Vec<super::lattice::Lattice>,
    pub root_universe: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `surfaces` | `Vec<super::surface::SurfaceKind>` | Global surface array; region tokens and `on_surface` index into it. |
| `cells` | `Vec<crate::geometry::cell::Cell>` | Global cell array; universes and paths index into it. |
| `universes` | `Vec<super::universe::Universe>` | Global universe array; the root and every fill index into it. |
| `lattices` | `Vec<super::lattice::Lattice>` | Global lattice array; lattice-fill cells index into it. Each entry is a<br>[`Lattice`] enum ([`Lattice::Rect`] or [`Lattice::Hex`]). |
| `root_universe` | `usize` | Index of the root universe tracking starts in. |

##### Implementations

###### Methods

- ```rust
  pub fn locate(self: &Self, r: Position, u: Direction, on_surface: usize) -> Option<GeometryPath> { /* ... */ }
  ```
  Locate the particle at global position `r` moving along `u`.

- ```rust
  pub fn distance_to_boundary(self: &Self, path: &GeometryPath) -> BoundaryHit { /* ... */ }
  ```
  Distance to the nearest boundary — surface or lattice tile — over all

- ```rust
  pub fn sigma_t_at(self: &Self, r: Position, u: Direction, e: f64, materials: &[crate::material::material::Material], nuclides: &[crate::material::nuclide::Nuclide]) -> Option<f64> { /* ... */ }
  ```
  Total macroscopic cross section of the cell a point is in — a convenience

- ```rust
  pub fn cross_surface(self: &Self, i_surf: usize, r: Position, u: Direction) -> (Position, Direction, bool) { /* ... */ }
  ```
  Apply a surface crossing to a global position/direction and return the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
## Module `particle`

```rust
pub mod particle { /* ... */ }
```

### Modules

## Module `particle`

```rust
pub mod particle { /* ... */ }
```

### Types

#### Enum `ParticleType`

Particle type.  Maps to `openmc::Particle::Type`.

```rust
pub enum ParticleType {
    Neutron,
    Photon,
    Electron,
    Positron,
}
```

##### Variants

###### `Neutron`

###### `Photon`

###### `Electron`

###### `Positron`

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
    fn clone(self: &Self) -> ParticleType { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &ParticleType) -> bool { /* ... */ }
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
#### Enum `TallyEvent`

Event type — recorded each step for event-based tallying.
Maps to `openmc::TallyEvent`.

```rust
pub enum TallyEvent {
    None,
    Scatter,
    Fission,
    Absorption,
    Surface,
    Leak,
}
```

##### Variants

###### `None`

###### `Scatter`

###### `Fission`

###### `Absorption`

###### `Surface`

###### `Leak`

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
    fn clone(self: &Self) -> TallyEvent { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &TallyEvent) -> bool { /* ... */ }
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
#### Struct `Particle`

Full Monte Carlo particle state.  Maps to `openmc::Particle`.

Fields mirror `particle_data.h` then `particle.h` in order.

```rust
pub struct Particle {
    pub r: crate::geometry::position::Position,
    pub u: crate::geometry::position::Direction,
    pub e: f64,
    pub wgt: f64,
    pub cell: usize,
    pub material: usize,
    pub surface: usize,
    pub seed: u64,
    pub particle_type: ParticleType,
    pub alive: bool,
    pub event: TallyEvent,
    pub n_collision: u32,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `r` | `crate::geometry::position::Position` | Current position (cm). |
| `u` | `crate::geometry::position::Direction` | Current direction cosines (unit vector). |
| `e` | `f64` | Current kinetic energy (eV). |
| `wgt` | `f64` | Statistical weight. |
| `cell` | `usize` | Cell index in global cell array. |
| `material` | `usize` | Material index (usize::MAX for void). |
| `surface` | `usize` | Surface index of the last surface crossed (usize::MAX if none). |
| `seed` | `u64` | Primary RNG seed for this particle. |
| `particle_type` | `ParticleType` |  |
| `alive` | `bool` |  |
| `event` | `TallyEvent` |  |
| `n_collision` | `u32` | Number of collisions this particle has had. |

##### Implementations

###### Methods

- ```rust
  pub fn new(r: Position, u: Direction, e: f64, wgt: f64, seed: u64) -> Self { /* ... */ }
  ```
  Create a new particle at the given phase-space coordinates.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
## Module `bank`

```rust
pub mod bank { /* ... */ }
```

### Types

#### Struct `BankSite`

A single entry in the fission site bank.  Maps to `openmc::SourceSite`.

```rust
pub struct BankSite {
    pub r: crate::geometry::position::Position,
    pub u: crate::geometry::position::Direction,
    pub e: f64,
    pub wgt: f64,
    pub seed: u64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `r` | `crate::geometry::position::Position` |  |
| `u` | `crate::geometry::position::Direction` |  |
| `e` | `f64` |  |
| `wgt` | `f64` |  |
| `seed` | `u64` | RNG seed to use when this site becomes a source particle. |

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
    fn clone(self: &Self) -> BankSite { /* ... */ }
    ```

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
#### Struct `Bank`

Particle bank.

```rust
pub struct Bank {
    pub sites: Vec<BankSite>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `sites` | `Vec<BankSite>` |  |

##### Implementations

###### Methods

- ```rust
  pub fn new() -> Self { /* ... */ }
  ```

- ```rust
  pub fn push(self: &mut Self, site: BankSite) { /* ... */ }
  ```

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```

- ```rust
  pub fn clear(self: &mut Self) { /* ... */ }
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
## Module `material`

```rust
pub mod material { /* ... */ }
```

### Modules

## Module `material`

```rust
pub mod material { /* ... */ }
```

### Types

#### Struct `NuclideComponent`

Material composition and macroscopic cross-section lookup.

C++ source: `src/material.cpp` (1603 LOC), `include/openmc/material.h`.

A `Material` is a mixture of nuclides at specified atom/weight densities.
During transport, the material provides:
  - Macroscopic total cross section Σ_t (sum of nuclide contributions)
  - Nuclide sampling (select which nuclide the neutron collides with)
  - Temperature for Doppler-broadened cross-section lookup
A nuclide component within a material.

```rust
pub struct NuclideComponent {
    pub nuclide_idx: usize,
    pub atom_density: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `nuclide_idx` | `usize` | Index into the global nuclide array. |
| `atom_density` | `f64` | Atom density in atoms/barn·cm. |

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
    fn clone(self: &Self) -> NuclideComponent { /* ... */ }
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
#### Struct `MacroXs`

Macroscopic cross sections of a material at one energy \[cm⁻¹\].

Each channel is Σ_x(E) = Σ_i N_i · σ_x,i(E), with N_i the atom density
\[atoms/barn·cm\] and σ in barn, so the product is in cm⁻¹.

```rust
pub struct MacroXs {
    pub total: f64,
    pub elastic: f64,
    pub fission: f64,
    pub nu_fission: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `total` | `f64` | Total Σ_t \[cm⁻¹\] — governs the distance-to-collision sample. |
| `elastic` | `f64` | Elastic scattering Σ_s \[cm⁻¹\]. |
| `fission` | `f64` | Fission Σ_f \[cm⁻¹\]. |
| `nu_fission` | `f64` | Fission production ν̄·Σ_f \[cm⁻¹\] — the k-eigenvalue source term. |

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
    fn clone(self: &Self) -> MacroXs { /* ... */ }
    ```

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
    fn default() -> MacroXs { /* ... */ }
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
#### Struct `Material`

A material — mixture of nuclides.  Maps to `openmc::Material`.

```rust
pub struct Material {
    pub id: i32,
    pub name: String,
    pub components: Vec<NuclideComponent>,
    pub temperature: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `id` | `i32` |  |
| `name` | `String` |  |
| `components` | `Vec<NuclideComponent>` |  |
| `temperature` | `f64` | Temperature in Kelvin (passed straight to the WMP Doppler evaluator). |

##### Implementations

###### Methods

- ```rust
  pub fn macro_xs(self: &Self, e: f64, nuclides: &[Nuclide]) -> MacroXs { /* ... */ }
  ```
  Macroscopic cross sections at energy `e` \[eV\], summed over all nuclides.

- ```rust
  pub fn macro_xs_total(self: &Self, e: f64, nuclides: &[Nuclide]) -> f64 { /* ... */ }
  ```
  Macroscopic total cross section Σ_t(E) \[cm⁻¹\] = Σ_i N_i·σ_t,i(E).

- ```rust
  pub fn sample_nuclide(self: &Self, e: f64, seed: &mut u64, nuclides: &[Nuclide]) -> usize { /* ... */ }
  ```
  Sample which nuclide the neutron collides with, weighted by each

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
## Module `nuclide`

```rust
pub mod nuclide { /* ... */ }
```

### Types

#### Struct `MicroXS`

Microscopic cross sections at a given energy (barn = 1e-24 cm²).

The transport-side currency. `absorption = capture + fission`; `nu_fission`
is ν̄·σ_f (the fission-source production channel). `inelastic` is the total
inelastic scattering σ (MT=51…91, discrete levels + continuum) and is a
*sub-partition* of the scattering channel, not a separate removal — it is
already included in `total`. It is non-zero only for the HIGH (`Pointwise`)
tier, which carries the resolved inelastic level structure; the LOW tier
reports 0 and lumps inelastic into elastic.

`n2n` is the (n,2n) cross section (ENDF MT=16), likewise a *sub-partition* of
scattering already included in `total`. It is carved out so the transport
kernel can give it its true neutron **multiplicity** (yield 2) instead of
sweeping it into single-neutron elastic — see [`crate::physics::keff`]. HIGH
tier only (from the reconstructed MF=3 background); the LOW tier reports 0.

```rust
pub struct MicroXS {
    pub total: f64,
    pub elastic: f64,
    pub fission: f64,
    pub absorption: f64,
    pub inelastic: f64,
    pub n2n: f64,
    pub nu_fission: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `total` | `f64` | Total σ_t \[barn\]. |
| `elastic` | `f64` | Elastic scattering σ_s \[barn\]. |
| `fission` | `f64` | Fission σ_f \[barn\]. |
| `absorption` | `f64` | Absorption σ_a = capture + fission \[barn\]. |
| `inelastic` | `f64` | Total inelastic scattering σ (MT=51…91) \[barn\]; HIGH tier only, else 0. |
| `n2n` | `f64` | (n,2n) scattering σ (MT=16) \[barn\]; HIGH tier only, else 0. Emits 2<br>neutrons — the multiplicity the transport kernel restores. |
| `nu_fission` | `f64` | Fission production ν̄·σ_f \[barn\]. |

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
    fn clone(self: &Self) -> MicroXS { /* ... */ }
    ```

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
    fn default() -> MicroXS { /* ... */ }
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
#### Enum `Inelastic`

A sampled inelastic scattering channel — the outcome of
[`Nuclide::sample_inelastic`], telling the transport kernel which kinematics
to apply.

Kept as an enum (not a trait object) per the workspace design rules: the set
of inelastic secondary-energy laws is closed and known at compile time.

```rust
pub enum Inelastic {
    Level {
        q: f64,
    },
    Continuum,
}
```

##### Variants

###### `Level`

A discrete inelastic level (MT=51…90): two-body kinematics with the level's
Q-value `q` \[eV\] (negative — the neutron gives up the excitation energy).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `q` | `f64` | Reaction Q-value \[eV\] (< 0), i.e. −(level excitation energy). |

###### `Continuum`

The continuum inelastic channel (MT=91): a broad secondary-energy
distribution modelled by a Weisskopf evaporation spectrum.

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
    fn clone(self: &Self) -> Inelastic { /* ... */ }
    ```

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
#### Struct `Nuclide`

One isotope's cross-section data, pulled from `njoy-outram-park-fork`.

Two constructors give the two fidelity tiers, both feeding the *same*
[`Nuclide::xs_at_energy`] seam so the transport kernel never knows the
difference:

- [`Nuclide::from_core`] — **LOW**: embedded WMP + fast MGXS, offline.
- [`Nuclide::from_endf`] — **HIGH**: download raw ENDF, RECONR + BROADR to
  pointwise σ(E) (behind the `net-fetch` feature).

```rust
pub struct Nuclide {
    pub name: String,
    pub awr: f64,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Isotope name, e.g. `"U235"`. |
| `awr` | `f64` | Atomic weight ratio (target mass / neutron mass) — sets scatter kinematics. |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn from_core(name: &str) -> Result<Self, NjoyError> { /* ... */ }
  ```
  **LOW fidelity.** Resolve a nuclide from the embedded CORE nuclear-data

- ```rust
  pub fn with_thermal_scattering(self: Self, thermal: ThermalScattering) -> Self { /* ... */ }
  ```
  Attach a bound-atom S(α,β) [`ThermalScattering`] treatment to this nuclide

- ```rust
  pub fn xs_at_energy(self: &Self, e: f64, temp_k: f64) -> MicroXS { /* ... */ }
  ```
  Microscopic cross sections at incident energy `e` \[eV\] and temperature

- ```rust
  pub fn sample_inelastic(self: &Self, e: f64, seed: &mut u64) -> Inelastic { /* ... */ }
  ```
  Sample which inelastic scattering channel a collision at energy `e` \[eV\]

- ```rust
  pub fn sample_elastic_mu_cm(self: &Self, e: f64, seed: &mut u64) -> Option<f64> { /* ... */ }
  ```
  Sample an elastic scattering cosine in the **centre-of-mass frame** at

- ```rust
  pub fn sample_thermal(self: &Self, e: f64, seed: &mut u64) -> Option<(f64, f64)> { /* ... */ }
  ```
  Sample a bound-atom S(α,β) thermal scatter at incident energy `e` \[eV\],

- ```rust
  pub fn elastic_mubar(self: &Self, e: f64) -> f64 { /* ... */ }
  ```
  The elastic-scattering **mean cosine** μ̄ (CM frame) this nuclide's data

- ```rust
  pub fn e_max_ev(self: &Self) -> f64 { /* ... */ }
  ```
  The upper energy \[eV\] of this nuclide's continuous-energy (WMP) range —

- ```rust
  pub fn sample_fission_energy(self: &Self, e_in: f64, seed: &mut u64) -> f64 { /* ... */ }
  ```
  Sample a fission-neutron birth energy \[eV\] given the incident energy

- ```rust
  pub fn native_energy_grid(self: &Self, e_min_ev: f64, e_max_ev: f64) -> Vec<f64> { /* ... */ }
  ```
  The native energy breakpoints \[eV\] this nuclide's cross-section data

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
## Module `reaction`

```rust
pub mod reaction { /* ... */ }
```

### Types

#### Enum `ReactionMT`

Reaction types and secondary particle sampling.

C++ source: `src/reaction.cpp` (424 LOC), `include/openmc/reaction.h`.
Also: `src/physics_common.cpp` — secondary product angle/energy sampling.

OpenMC models each reaction as a `Reaction` object that stores:
  - MT number (ENDF reaction designation, e.g. MT=2 elastic, MT=18 fission)
  - Q-value (energy release)
  - Secondary product distributions (angle + energy)

The `Reaction` trait here mirrors the C++ virtual base.
ENDF reaction MT number (subset relevant to neutron transport).
MT values are stored as associated constants for documentation; the enum
itself does not carry integer discriminants to allow the `Other(u32)` variant.

```rust
pub enum ReactionMT {
    Elastic,
    Fission,
    Capture,
    Inelastic,
    N2N,
    N3N,
    Total,
    Other(u32),
}
```

##### Variants

###### `Elastic`

###### `Fission`

###### `Capture`

###### `Inelastic`

###### `N2N`

###### `N3N`

###### `Total`

###### `Other`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `u32` |  |

##### Implementations

###### Methods

- ```rust
  pub fn mt_number(self: Self) -> u32 { /* ... */ }
  ```
  ENDF MT integer for this reaction.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> ReactionMT { /* ... */ }
    ```

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

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ReactionMT) -> bool { /* ... */ }
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
### Traits

#### Trait `Reaction`

Interface for a single nuclear reaction.  Maps to the virtual `Reaction` base.

```rust
pub trait Reaction {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `mt`
- `q_value`
- `sample_secondary`: Sample secondary neutron state post-reaction.

## Module `thermal`

S(α,β) thermal scattering tables — the bound-atom scattering treatment.

C++ analogue: `src/thermal.cpp`, `include/openmc/thermal.h` (the ACE
`ThermalScattering` / `ThermalData` blocks).

**Why this exists.** Below a few eV the free-gas treatment of scattering from
bound atoms is wrong: chemical binding raises the bound cross section
(≈ 81.8 b per H in H₂O at E→0 vs the 20.4 b free-atom limit) and lets the
neutron *up-scatter* off the thermal motion of the molecule, which is what
establishes the Maxwellian thermal spectrum in a moderator. A fast-metal
sphere (Godiva) never reaches these energies, but a *thermal* LWR pin-cell
lives or dies on it.

**Data-free, like the rest of `outram-mc-libs`.** This struct holds no ENDF
parsing of its own: it is built from the njoy consumer surface
[`njoy_outram_park_fork::thermr::scattering::IncoherentInelasticScattering`],
which reads the ENDF/B `tsl-*` thermal evaluation and answers the two
transport questions — σ_inel(E) and the secondary energy/angle distribution.
To keep the transport hot-loop cheap (that kernel integrates the S(α,β)
double-differential per call), this type **pre-tabulates** both quantities on
a fixed energy grid at construction, exactly as NJOY/ACE bakes the ITIE/ITXE
blocks, and the run-time path only interpolates and samples.

**Scope.** Only the incoherent-*inelastic* channel — the one H-in-H₂O needs
(light water has no thermal elastic). Coherent / incoherent-elastic bound
scattering (graphite, ZrH) is deliberately not wired here yet.

```rust
pub mod thermal { /* ... */ }
```

### Types

#### Struct `ThermalScattering`

Pre-tabulated incoherent-inelastic S(α,β) thermal scattering for one bound
scatterer (H in H₂O) at one temperature — the transport-side data surface.

Built once per material+temperature with [`from_endf_file`](Self::from_endf_file),
then queried in the transport loop by:
- [`inelastic_xs`](Self::inelastic_xs) — σ_inel(E) **per principal atom**
  \[barn\] at incident energy `E` \[eV\] (multiply by the principal-atom number
  density to get the macroscopic contribution);
- [`sample`](Self::sample) — a laboratory-frame outgoing energy \[eV\] and
  cosine for a thermal scatter.

Cross sections are **per principal atom** (per H for H-in-H₂O). The material
composition (two H per H₂O) is the caller's number-density bookkeeping.

```rust
pub struct ThermalScattering {
    pub name: String,
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Human-readable scatterer name, e.g. `"H in H2O"`. |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn from_endf_file(path: &str, mat: i32, temperature_k: f64, name: &str) -> Result<Self, njoy_outram_park_fork::NjoyError> { /* ... */ }
  ```
  Build the pre-tabulated H-in-H₂O S(α,β) treatment from an ENDF `tsl-*`

- ```rust
  pub fn cutoff_ev(self: &Self) -> f64 { /* ... */ }
  ```
  Upper energy \[eV\] of the S(α,β) treatment (the thermal cutoff). Above it

- ```rust
  pub fn selected_temperature_k(self: &Self) -> f64 { /* ... */ }
  ```
  The tabulated temperature \[K\] actually used (nearest grid point to the

- ```rust
  pub fn inelastic_xs(self: &Self, e: f64) -> f64 { /* ... */ }
  ```
  Incoherent-inelastic cross section σ_inel(E) **per principal atom**

- ```rust
  pub fn sample(self: &Self, e: f64, seed: &mut u64) -> Option<(f64, f64)> { /* ... */ }
  ```
  Sample a thermal scatter at incident energy `e` \[eV\], returning

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
### Constants and Statics

#### Constant `DEFAULT_THERMAL_CUTOFF_EV`

Default upper energy \[eV\] of the S(α,β) treatment — the "thermal cutoff".

Above it the neutron sees the ordinary free-gas / WMP elastic channel; below
it the bound S(α,β) treatment replaces elastic scattering off the principal
atom. 4 eV is the OpenMC/NJOY convention for light-water thermal tables
(`ENERGY_MAX_THERMAL`-class cutoff): by ~4 eV the S(α,β) cross section has
relaxed to the free-atom limit and the up-scatter probability is negligible,
so the join to free-gas is smooth.

```rust
pub const DEFAULT_THERMAL_CUTOFF_EV: f64 = 4.0;
```

## Module `source`

```rust
pub mod source { /* ... */ }
```

### Modules

## Module `source`

```rust
pub mod source { /* ... */ }
```

### Types

#### Struct `SourceSite`

A sampled source particle state.

```rust
pub struct SourceSite {
    pub r: crate::geometry::position::Position,
    pub u: crate::geometry::position::Direction,
    pub e: f64,
    pub wgt: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `r` | `crate::geometry::position::Position` |  |
| `u` | `crate::geometry::position::Direction` |  |
| `e` | `f64` |  |
| `wgt` | `f64` |  |

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
    fn clone(self: &Self) -> SourceSite { /* ... */ }
    ```

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
#### Struct `IndependentSource`

Independent (uncorrelated) external source.  Maps to `openmc::IndependentSource`.

```rust
pub struct IndependentSource {
    pub spatial: Box<dyn super::spatial::SpatialDist>,
    pub energy: Box<dyn super::energy::EnergyDist>,
    pub angle: Box<dyn super::angle::AngleDist>,
    pub strength: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `spatial` | `Box<dyn super::spatial::SpatialDist>` |  |
| `energy` | `Box<dyn super::energy::EnergyDist>` |  |
| `angle` | `Box<dyn super::angle::AngleDist>` |  |
| `strength` | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn sample(self: &Self, seed: &mut u64) -> SourceSite { /* ... */ }
  ```
  Sample one source particle.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
## Module `spatial`

```rust
pub mod spatial { /* ... */ }
```

### Types

#### Struct `PointSource`

Point source — all particles start at the same location.

```rust
pub struct PointSource {
    pub r: crate::geometry::position::Position,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `r` | `crate::geometry::position::Position` |  |

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
- **SpatialDist**
  - ```rust
    fn sample(self: &Self, _seed: &mut u64) -> Position { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `BoxSource`

Uniform box source.

```rust
pub struct BoxSource {
    pub lower_left: crate::geometry::position::Position,
    pub upper_right: crate::geometry::position::Position,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `lower_left` | `crate::geometry::position::Position` |  |
| `upper_right` | `crate::geometry::position::Position` |  |

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
- **SpatialDist**
  - ```rust
    fn sample(self: &Self, seed: &mut u64) -> Position { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
#### Struct `SphericalSource`

Spherical shell source (uniform surface or volume).
TODO: port from `distribution_spatial.cpp`.

```rust
pub struct SphericalSource {
    pub center: crate::geometry::position::Position,
    pub r_inner: f64,
    pub r_outer: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `center` | `crate::geometry::position::Position` |  |
| `r_inner` | `f64` |  |
| `r_outer` | `f64` |  |

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
- **SpatialDist**
  - ```rust
    fn sample(self: &Self, seed: &mut u64) -> Position { /* ... */ }
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
- **Upcast**
  - ```rust
    fn upcast(self: &Self) -> Option<&T> { /* ... */ }
    ```

- **WasmNotSend**
- **WasmNotSendSync**
- **WasmNotSync**
### Traits

#### Trait `SpatialDist`

Trait for spatial distributions.

```rust
pub trait SpatialDist: Send + Sync {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `sample`

##### Implementations

This trait is implemented for the following types:

- `PointSource`
- `BoxSource`
- `SphericalSource`

## Module `energy`

```rust
pub mod energy { /* ... */ }
```

### Types

#### Struct `Monoenergetic`

Monoenergetic source (all particles at the same energy).

```rust
pub struct Monoenergetic {
    pub e: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `e` | `f64` |  |

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

- **EnergyDist**
  - ```rust
    fn sample(self: &Self, _seed: &mut u64) -> f64 { /* ... */ }
    ```

- **Freeze**
- **From**
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
#### Struct `MaxwellSpectrum`

Maxwellian fission spectrum: f(E) ∝ √E · exp(−E / θ). θ in eV.
TODO: port Maxwell sampler from `random_dist.cpp`.

```rust
pub struct MaxwellSpectrum {
    pub theta: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `theta` | `f64` |  |

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

- **EnergyDist**
  - ```rust
    fn sample(self: &Self, seed: &mut u64) -> f64 { /* ... */ }
    ```

- **Freeze**
- **From**
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
#### Struct `WattSpectrum`

Watt fission spectrum: f(E) ∝ exp(−E/a) · sinh(√(b·E)). a, b in eV.
TODO: port from `random_dist.cpp`.

```rust
pub struct WattSpectrum {
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

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **EnergyDist**
  - ```rust
    fn sample(self: &Self, seed: &mut u64) -> f64 { /* ... */ }
    ```

- **Freeze**
- **From**
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
#### Struct `TabulatedEnergy`

Tabulated energy distribution (piecewise linear CDF).
TODO: port interpolation from `distribution_energy.cpp`.

```rust
pub struct TabulatedEnergy {
    pub energies: Vec<f64>,
    pub cdf: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `energies` | `Vec<f64>` |  |
| `cdf` | `Vec<f64>` |  |

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

- **EnergyDist**
  - ```rust
    fn sample(self: &Self, _seed: &mut u64) -> f64 { /* ... */ }
    ```

- **Freeze**
- **From**
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
### Traits

#### Trait `EnergyDist`

Energy source distributions.

C++ source: `src/distribution_energy.cpp`, `include/openmc/distribution_energy.h`.
Trait for energy distributions (returns energy in eV).

```rust
pub trait EnergyDist: Send + Sync {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `sample`

##### Implementations

This trait is implemented for the following types:

- `Monoenergetic`
- `MaxwellSpectrum`
- `WattSpectrum`
- `TabulatedEnergy`

## Module `angle`

```rust
pub mod angle { /* ... */ }
```

### Types

#### Struct `IsotropicAngle`

Isotropic — uniform on the unit sphere.
TODO: port from `distribution_angle.cpp`.

```rust
pub struct IsotropicAngle;
```

##### Implementations

###### Trait Implementations

- **AngleDist**
  - ```rust
    fn sample(self: &Self, seed: &mut u64, _e: f64) -> Direction { /* ... */ }
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
#### Struct `MonodirectionalAngle`

Monodirectional — all particles in the same direction.

```rust
pub struct MonodirectionalAngle {
    pub d: crate::geometry::position::Direction,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `d` | `crate::geometry::position::Direction` |  |

##### Implementations

###### Trait Implementations

- **AngleDist**
  - ```rust
    fn sample(self: &Self, _seed: &mut u64, _e: f64) -> Direction { /* ... */ }
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
### Traits

#### Trait `AngleDist`

Trait for angular distributions.

```rust
pub trait AngleDist: Send + Sync {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `sample`: Sample a direction. `e` is the particle energy (eV) for energy-dependent

##### Implementations

This trait is implemented for the following types:

- `IsotropicAngle`
- `MonodirectionalAngle`

## Module `tally`

```rust
pub mod tally { /* ... */ }
```

### Modules

## Module `tally`

```rust
pub mod tally { /* ... */ }
```

### Types

#### Enum `ScoreType`

Score type.  Maps to `openmc::TallyScore`.

```rust
pub enum ScoreType {
    Flux,
    Total,
    Fission,
    Absorption,
    NuFission,
    KappaFission,
    ScatterN,
    Current,
    Events,
}
```

##### Variants

###### `Flux`

###### `Total`

###### `Fission`

###### `Absorption`

###### `NuFission`

###### `KappaFission`

Fission energy-deposition rate (a.k.a. heating from fission).

Maps to `openmc::SCORE_KAPPA_FISSION` (`src/tallies/tally_scoring.cpp:1480`).
Scores the fission reaction rate multiplied by the recoverable energy per
fission `Q` \[J\], so the accumulated bin is a fission **power** in J per
source-particle-generation. See [`super::scoring::Q_FISSION_J`] for the
constant and its provenance; it is used to normalize a k-eigenvalue tally
to a target reactor thermal power (the `tally-power-normalization`
notebook).

###### `ScatterN`

###### `Current`

###### `Events`

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
    fn clone(self: &Self) -> ScoreType { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &ScoreType) -> bool { /* ... */ }
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
#### Struct `TallyBin`

A single tally accumulator bin: running sum + sum-of-squares for statistics.

```rust
pub struct TallyBin {
    pub sum: f64,
    pub sum_sq: f64,
    pub count: u64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `sum` | `f64` |  |
| `sum_sq` | `f64` |  |
| `count` | `u64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn score(self: &mut Self, value: f64) { /* ... */ }
  ```

- ```rust
  pub fn mean(self: &Self, n_realizations: u64) -> f64 { /* ... */ }
  ```
  Mean over `n_realizations` active batches.

- ```rust
  pub fn rel_std_dev(self: &Self, n_realizations: u64) -> f64 { /* ... */ }
  ```
  Relative standard deviation (as fraction of mean).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> TallyBin { /* ... */ }
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
    fn default() -> TallyBin { /* ... */ }
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
#### Struct `Tally`

A tally.  Maps to `openmc::Tally`.

```rust
pub struct Tally {
    pub id: i32,
    pub name: String,
    pub filters: Vec<Box<dyn Filter>>,
    pub scores: Vec<ScoreType>,
    pub bins: Vec<TallyBin>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `id` | `i32` |  |
| `name` | `String` |  |
| `filters` | `Vec<Box<dyn Filter>>` |  |
| `scores` | `Vec<ScoreType>` |  |
| `bins` | `Vec<TallyBin>` | Accumulated bins, indexed `[filter_bin * n_scores + score_idx]`. |

##### Implementations

###### Methods

- ```rust
  pub fn n_bins(self: &Self) -> usize { /* ... */ }
  ```
  Total number of bins = product of each filter's bin count × number of scores.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
## Module `filter`

```rust
pub mod filter { /* ... */ }
```

### Types

#### Struct `FilterEvent`

Snapshot of particle state passed to filters at scoring time.

```rust
pub struct FilterEvent {
    pub cell_idx: usize,
    pub material_idx: usize,
    pub universe_idx: usize,
    pub energy: f64,
    pub surface_idx: usize,
    pub position: crate::geometry::position::Position,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `cell_idx` | `usize` |  |
| `material_idx` | `usize` |  |
| `universe_idx` | `usize` |  |
| `energy` | `f64` |  |
| `surface_idx` | `usize` | Surface crossed (usize::MAX if not a surface-crossing event). |
| `position` | `crate::geometry::position::Position` | Representative spatial position of the event \[cm\] — the streamed<br>segment's midpoint for the track-length estimator. Used by the spatial<br>filters ([`MeshFilter`], [`SpatialLegendreFilter`]); ignored by the<br>cell/material/universe/energy filters. |

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
#### Struct `CellFilter`

Filter by cell.  Maps to `openmc::CellFilter`.

```rust
pub struct CellFilter {
    pub cell_indices: Vec<usize>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `cell_indices` | `Vec<usize>` |  |

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

- **Filter**
  - ```rust
    fn n_bins(self: &Self) -> usize { /* ... */ }
    ```

  - ```rust
    fn get_bin(self: &Self, ev: &FilterEvent) -> Option<usize> { /* ... */ }
    ```

- **Freeze**
- **From**
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
#### Struct `MaterialFilter`

Filter by material.  Maps to `openmc::MaterialFilter`.

```rust
pub struct MaterialFilter {
    pub material_indices: Vec<usize>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `material_indices` | `Vec<usize>` |  |

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

- **Filter**
  - ```rust
    fn n_bins(self: &Self) -> usize { /* ... */ }
    ```

  - ```rust
    fn get_bin(self: &Self, ev: &FilterEvent) -> Option<usize> { /* ... */ }
    ```

- **Freeze**
- **From**
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
#### Struct `EnergyFilter`

Filter by energy bin (contiguous group boundaries in eV).
Maps to `openmc::EnergyFilter`.

```rust
pub struct EnergyFilter {
    pub bins: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `bins` | `Vec<f64>` |  |

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

- **Filter**
  - ```rust
    fn n_bins(self: &Self) -> usize { /* ... */ }
    ```

  - ```rust
    fn get_bin(self: &Self, ev: &FilterEvent) -> Option<usize> { /* ... */ }
    ```

- **Freeze**
- **From**
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
#### Struct `UniverseFilter`

Filter by universe.  Maps to `openmc::UniverseFilter`.

```rust
pub struct UniverseFilter {
    pub universe_indices: Vec<usize>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `universe_indices` | `Vec<usize>` |  |

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

- **Filter**
  - ```rust
    fn n_bins(self: &Self) -> usize { /* ... */ }
    ```

  - ```rust
    fn get_bin(self: &Self, ev: &FilterEvent) -> Option<usize> { /* ... */ }
    ```

- **Freeze**
- **From**
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
#### Struct `MeshFilter`

Filter by a regular spatial mesh.  Maps to `openmc::MeshFilter`.

Bins an event by the [`RegularMesh`] cell that contains its representative
position ([`FilterEvent::position`], the segment midpoint for the track-length
estimator). `n_bins` is the mesh cell count `nx·ny·nz`; a position outside the
mesh box is unbinned (`get_bin` returns `None`, so the tally drops it).

Ported from `src/tallies/filter_mesh.cpp` (`MeshFilter::get_all_bins`, the
non-track-length branch: `mesh->get_bin(r)`; a single bin, weight 1). The
track-length "bins crossed" sub-segmentation
(`StructuredMesh::bins_crossed`) is a documented gap (bead op-6tz.13) — this
port scores the whole segment into the midpoint's cell, which is exact for a
mesh whose cells are large relative to the mean free path.

```rust
pub struct MeshFilter {
    pub mesh: super::mesh::RegularMesh,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `mesh` | `super::mesh::RegularMesh` |  |

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

- **Filter**
  - ```rust
    fn n_bins(self: &Self) -> usize { /* ... */ }
    ```

  - ```rust
    fn get_bin(self: &Self, ev: &FilterEvent) -> Option<usize> { /* ... */ }
    ```

- **Freeze**
- **From**
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
#### Enum `LegendreAxis`

Axis along which a [`SpatialLegendreFilter`] expands the flux.

Maps to `openmc::LegendreAxis` (`include/openmc/tallies/filter_sptl_legendre.h`).

```rust
pub enum LegendreAxis {
    X,
    Y,
    Z,
}
```

##### Variants

###### `X`

Expand along the x coordinate.

###### `Y`

Expand along the y coordinate.

###### `Z`

Expand along the z coordinate.

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
    fn clone(self: &Self) -> LegendreAxis { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &LegendreAxis) -> bool { /* ... */ }
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
#### Struct `SpatialLegendreFilter`

Functional-expansion (Legendre) filter along one Cartesian axis.

Maps to `openmc::SpatialLegendreFilter` (`src/tallies/filter_sptl_legendre.cpp`).
Instead of binning an event into one spatial cell, this expands the flux into
Legendre moments along `axis` over the interval `[min, max]` \[cm\]: the axis
coordinate `x` is normalized to `ξ = 2·(x − min)/(max − min) − 1 ∈ [−1, 1]`
and each moment bin `n ∈ 0..=order` receives weight `P_n(ξ)` (the Legendre
polynomial, `src/tallies/filter_sptl_legendre.cpp:76-87` via
`calc_pn_c`, `src/math_functions.cpp:105`).

The moment stored in bin `n` is therefore the *raw* moment
`∫ φ(ξ) P_n(ξ) dξ` (track-length weighted). The reconstruction normalization
`(2n+1)/2` is applied at flux *reconstruction* time
(`evaluate_legendre`, `src/math_functions.cpp:118-128`), **not** stored in the
filter weight — this port mirrors that convention exactly.

# Bins and scoring path
`n_bins = order + 1`. Because the base [`Filter`] contract maps to a single
bin, the moment expansion is deposited via [`Filter::expansion_moments`] (the
faithful multi-`(bin, weight)` analogue of OpenMC's `get_all_bins`), which the
scoring path in [`super::scoring::score_track_length`] handles for a *lone*
expansion filter. Combining an expansion filter with other filters in one
tally is not yet supported (documented gap, bead op-6tz.14).

# Fields
- `order` — highest Legendre order retained (bins `P_0 … P_order`).
- `axis` — the Cartesian axis expanded ([`LegendreAxis`]).
- `min` / `max` — the axis interval \[cm\] mapped onto `ξ ∈ [−1, 1]`;
  `max > min` required. Events outside `[min, max]` contribute nothing.

```rust
pub struct SpatialLegendreFilter {
    pub order: usize,
    pub axis: LegendreAxis,
    pub min: f64,
    pub max: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `order` | `usize` | Highest Legendre order retained (⇒ `order + 1` moment bins). |
| `axis` | `LegendreAxis` | Cartesian axis along which the flux is expanded. |
| `min` | `f64` | Low end of the expansion interval \[cm\]. |
| `max` | `f64` | High end of the expansion interval \[cm\] (must exceed `min`). |

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

- **Filter**
  - ```rust
    fn n_bins(self: &Self) -> usize { /* ... */ }
    ```

  - ```rust
    fn get_bin(self: &Self, ev: &FilterEvent) -> Option<usize> { /* ... */ }
    ```
    A single-bin fallback: bin 0 if the event lies in `[min, max]`, else `None`.

  - ```rust
    fn expansion_moments(self: &Self, ev: &FilterEvent) -> Option<Vec<f64>> { /* ... */ }
    ```
    Legendre moment weights `P_0(ξ) … P_order(ξ)`, or `None` if the event's

- **Freeze**
- **From**
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
### Traits

#### Trait `Filter`

Base trait for all filters.  Maps to `openmc::Filter`.

```rust
pub trait Filter: Send + Sync {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `n_bins`: Number of bins this filter produces.
- `get_bin`: Map particle state to a bin index, or `None` if the event doesn't match.

##### Provided Methods

- ```rust
  fn expansion_moments(self: &Self, _event: &FilterEvent) -> Option<Vec<f64>> { /* ... */ }
  ```
  Functional-expansion weights: for an expansion filter (e.g.

##### Implementations

This trait is implemented for the following types:

- `CellFilter`
- `MaterialFilter`
- `EnergyFilter`
- `UniverseFilter`
- `MeshFilter`
- `SpatialLegendreFilter`

## Module `mesh`

Structured spatial meshes for tally binning.

C++ source: `src/mesh.cpp`, `include/openmc/mesh.h`.

A mesh overlays a regular grid on the geometry so a flux (or reaction-rate)
tally can be resolved *spatially* — one bin per grid cell — independent of the
CSG cell structure. This is the spatial counterpart to the energy grouping an
[`super::filter::EnergyFilter`] provides. Only the axis-aligned
[`RegularMesh`] is ported here (the workhorse for the `post-processing`
notebook); rectilinear / cylindrical / spherical meshes are a documented gap
(bead op-6tz.13).

```rust
pub mod mesh { /* ... */ }
```

### Types

#### Struct `RegularMesh`

Axis-aligned regular (equal-spacing) Cartesian mesh.

Maps to `openmc::RegularMesh` (`src/mesh.cpp`). The mesh spans the box
`[lower_left, upper_right]` \[cm\] and is divided into `dimension[i]` equal
cells along each axis `i ∈ {x, y, z}`. A point is binned into the grid cell
containing it; points outside the box are unbinned (`None`).

# Fields (all lengths in cm)
- `lower_left` — the low corner `[x0, y0, z0]` of the meshed box.
- `upper_right` — the high corner `[x1, y1, z1]`; each `upper_right[i]` must
  exceed `lower_left[i]`.
- `dimension` — number of cells `[nx, ny, nz]` along each axis (each ≥ 1). A
  flat 2-D mesh sets one dimension to 1 (e.g. `[4, 4, 1]`).

```rust
pub struct RegularMesh {
    pub lower_left: [f64; 3],
    pub upper_right: [f64; 3],
    pub dimension: [usize; 3],
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `lower_left` | `[f64; 3]` | Low corner `[x0, y0, z0]` of the meshed box \[cm\]. |
| `upper_right` | `[f64; 3]` | High corner `[x1, y1, z1]` of the meshed box \[cm\]. |
| `dimension` | `[usize; 3]` | Number of equal cells `[nx, ny, nz]` along each axis. |

##### Implementations

###### Methods

- ```rust
  pub fn width(self: &Self) -> [f64; 3] { /* ... */ }
  ```
  Cell width `[wx, wy, wz]` \[cm\] along each axis

- ```rust
  pub fn n_bins(self: &Self) -> usize { /* ... */ }
  ```
  Total number of mesh cells = `nx · ny · nz`.

- ```rust
  pub fn get_bin(self: &Self, p: Position) -> Option<usize> { /* ... */ }
  ```
  Flat bin index of the mesh cell containing `p`, or `None` if `p` lies

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> RegularMesh { /* ... */ }
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
    fn eq(self: &Self, other: &RegularMesh) -> bool { /* ... */ }
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
## Module `scoring`

Tally scoring — accumulate scores at collision events (collision estimator).

C++ source: `src/tallies/tally_scoring.cpp`.

After a real collision the transport loop (see
[`crate::physics::transport_csg`]) calls [`score_collision`], which:
  1. builds a [`FilterEvent`] snapshot of the particle state,
  2. maps it through every filter on the tally to a flat bin index (a
     conjunction — if any filter rejects the event, nothing is scored),
  3. accumulates each requested [`ScoreType`] into that bin using the
     **collision estimator**.

# Collision estimator

For a neutron of weight `w` colliding in a material of macroscopic total
`Σ_t`, the collision-estimator contributions are

- flux:            `w / Σ_t`
- reaction rate x: `w · Σ_x / Σ_t`   (fission, ν-fission, …)
- total rate:      `w`               (one collision)

(`src/tallies/tally_scoring.cpp`, `score_general` collision branch.) This is
the simplest unbiased estimator; a track-length estimator (bead op-6tz.9
follow-up) would additionally score along free-flight segments.

```rust
pub mod scoring { /* ... */ }
```

### Functions

#### Function `score_track_length`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

Score one streamed free-flight segment into the **per-batch** accumulator
`batch` via the **track-length estimator**.

This is the primary flux estimator for the CSG k-eigenvalue loop
([`crate::physics::transport_csg::run_keff_csg`]): every segment a particle
streams — whether it ends in a collision or a surface crossing — deposits
`w·d` (flux) and `w·d·Σ_x` (reaction rates) into the bin matching its cell,
material, universe and energy. It is lower-variance than the collision
estimator in optically thin regions because it scores on *every* flight, not
only at collision sites.

# Per-batch accumulation

Contributions are added into `batch` — a scratch buffer the caller zeroes at
the start of each batch (generation) and flushes into the tally's persistent
[`super::tally::TallyBin`]s with [`flush_batch`] at the batch's end. This gives
the standard Monte-Carlo tally statistics: one *realization* per batch, so the
running mean and variance are over batches, not over individual events.
`batch` must have length `tally.n_bins()`.

# Parameters
- `batch` — per-batch flat accumulator, length `tally.n_bins()`.
- `tally` — the tally definition (filters + scores) read immutably.
- `cell_idx` / `material_idx` / `universe_idx` — leaf geometry indices of the
  segment (`material_idx == usize::MAX` in a void).
- `energy` — the particle energy \[eV\] over the segment (constant along a
  free flight; the outgoing energy is only set after the collision is scored).
- `distance` — the streamed segment length \[cm\].
- `position` — a representative spatial point of the segment \[cm\] for the
  spatial filters ([`super::filter::MeshFilter`],
  [`super::filter::SpatialLegendreFilter`]); the caller passes the segment
  **midpoint** `r + 0.5·d·u`, the track-length-representative point. Ignored by
  the non-spatial (cell/material/universe/energy) filters.
- `macro_xs` — the material's macroscopic cross sections at `energy`, or `None`
  in a void (then only the flux score deposits).
- `weight` — particle statistical weight (1.0 for analog transport).

# Functional-expansion (Legendre) path

If the tally carries a *single* expansion filter (a
[`super::filter::SpatialLegendreFilter`], detected via
[`super::filter::Filter::expansion_moments`]), the segment deposits
`w·d·Σ_x · P_n(ξ)` into every moment bin `n` at once (mirroring OpenMC's
multi-`(bin, weight)` `get_all_bins`) rather than routing through the single-bin
[`filter_bin`] path. See [`super::filter::SpatialLegendreFilter`].

```rust
pub fn score_track_length(batch: &mut [f64], tally: &super::tally::Tally, cell_idx: usize, material_idx: usize, universe_idx: usize, energy: f64, distance: f64, position: crate::geometry::position::Position, macro_xs: Option<&crate::material::material::MacroXs>, weight: f64) { /* ... */ }
```

#### Function `flush_batch`

Close a batch (generation): flush every per-batch accumulator into the tally's
persistent bins as one realization, then zero the accumulator for the next
batch.

Each call records one MC realization per bin — so after `n` active batches the
bins' `count` is `n`, [`super::tally::TallyBin::mean`] is the mean batch score,
and [`super::tally::TallyBin::rel_std_dev`] is the batch-to-batch relative
standard deviation (the standard tally uncertainty). `batch` must have length
`tally.bins.len()`.

```rust
pub fn flush_batch(tally: &mut super::tally::Tally, batch: &mut [f64]) { /* ... */ }
```

#### Function `score_collision`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

Score one real collision into `tally` via the collision estimator.

# Parameters
- `tally` — the tally to accumulate into (filters + scores + bins).
- `cell_idx` / `material_idx` / `universe_idx` — the leaf geometry indices of
  the collision site (for Cell/Material/Universe filters).
- `energy` — incident energy \[eV\] (for an EnergyFilter).
- `sigma_t` — macroscopic total Σ_t \[cm⁻¹\] at the collision.
- `macro_xs` — the material's macroscopic cross sections at `energy`, for the
  reaction-rate scores.
- `weight` — particle statistical weight (1.0 for analog transport).

If any attached filter does not match the event, the collision is not scored
(the filters act as a conjunction).

```rust
pub fn score_collision(tally: &mut super::tally::Tally, cell_idx: usize, material_idx: usize, universe_idx: usize, energy: f64, sigma_t: f64, macro_xs: &crate::material::material::MacroXs, weight: f64) { /* ... */ }
```

### Constants and Statics

#### Constant `Q_FISSION_J`

Recoverable energy per fission `Q` \[J\] — the constant multiplier of the
[`ScoreType::KappaFission`] score.

OpenMC reads a per-nuclide, energy-dependent `q_recoverable` from the
`fission_energy_release` data of each nuclide
(`src/nuclide.cpp:335-336`, `fission_q_recov_`) and multiplies the fission
reaction rate by it (`src/tallies/tally_scoring.cpp:1480`,
`SCORE_KAPPA_FISSION`). This crate's LOW-tier embedded WMP/collapsed data
does **not** carry per-nuclide fission-energy-release curves (that is bead
op-6tz.24), so we use a single documented constant appropriate for
U-235-dominated fuel:

`Q ≈ 193.4 MeV = 193.4e6 eV × 1.602176634e-19 J/eV ≈ 3.0982e-11 J`.

The 193.4 MeV recoverable value is the textbook U-235 thermal-fission figure
(e.g. Lamarsh & Baratta, *Introduction to Nuclear Engineering*; the value
excludes the ~10 MeV lost to antineutrinos, matching OpenMC's `q_recoverable`
semantics). Because it is a single constant rather than the true
energy/nuclide-dependent curve, the *absolute* kappa-fission watts this
produces are not a benchmark — but they are exactly proportional to the
fission rate, which is what the power-normalization round-trip needs.

```rust
pub const Q_FISSION_J: f64 = 3.0982e-11;
```

## Module `arithmetic`

Derived-tally arithmetic — combine tally results with uncertainty propagation.

# Provenance: mirroring OpenMC's *Python* tally algebra

Unlike the transport/geometry/scoring kernels, tally arithmetic does **not**
exist in the OpenMC C++ core (`src/tallies/*.cpp`). In OpenMC it lives entirely
in the Python post-processing layer — `openmc/tally.py` overloads `+ - * /` on
`Tally` objects and produces a derived tally whose `mean`/`std_dev` arrays carry
the propagated statistics (see `openmc.Tally.__add__`/`__sub__`/`__mul__`/
`__div__` and `Tally.get_slice`/`Tally.summation`). So per the crate's porting
rule this module is the **sanctioned scaffold-new-work path**: there is no C++
function to mirror, so we mirror the *documented Python semantics* instead, and
flag it as new work rather than a port.

# What it computes

A [`DerivedTally`] is a small value-with-uncertainty vector: parallel arrays of
bin **means** and **absolute standard deviations**, read out of a scored
[`Tally`] via [`DerivedTally::from_tally`] / [`DerivedTally::from_tally_score`].
Elementwise `+ - * /`, scalar multiply, bin summation, slicing and indexing are
provided, each propagating uncertainty under the **standard uncorrelated
(first-order) error-propagation** rules OpenMC's Python layer uses:

- add / sub: `σ = sqrt(σa² + σb²)`
- mul:       `σ = |a·b|·sqrt((σa/a)² + (σb/b)²)`
- div:       `σ = |a/b|·sqrt((σa/a)² + (σb/b)²)`
- scalar·k:  `σ = |k|·σa`
- sum:       `σ = sqrt(Σ σᵢ²)`

These assume the operands are **uncorrelated**, exactly as OpenMC's tally
arithmetic does; combining a tally with itself (e.g. `a + a`) therefore reports
a larger σ than the exact `2·a` (`scalar_mul`), which is the correct behaviour
for the independent-samples assumption and is asserted in the verification test.

No `Box`, no trait objects, no lifetime parameters — a plain owned struct of two
`Vec<f64>` (per the crate's Rust design rules).

```rust
pub mod arithmetic { /* ... */ }
```

### Types

#### Struct `DerivedTally`

A derived tally result: parallel arrays of per-bin **mean** and **absolute
standard deviation**.

Units follow whatever score produced it (flux in track-length units, a reaction
rate in `w·d·Σ_x`, etc. — this layer is unit-agnostic and only carries the
numbers). `values[i]` is bin `i`'s mean estimate and `std_devs[i]` its 1σ
absolute uncertainty; the two vectors always have equal length.

```rust
pub struct DerivedTally {
    pub values: Vec<f64>,
    pub std_devs: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `values` | `Vec<f64>` | Per-bin mean estimate. |
| `std_devs` | `Vec<f64>` | Per-bin absolute standard deviation (1σ), same length as `values`. |

##### Implementations

###### Methods

- ```rust
  pub fn from_tally(tally: &Tally, n_realizations: u64) -> DerivedTally { /* ... */ }
  ```
  Build a derived tally from **all** of a tally's bins, flat, in stored order.

- ```rust
  pub fn from_tally_score(tally: &Tally, score_idx: usize, n_realizations: u64) -> DerivedTally { /* ... */ }
  ```
  Extract a single score (`score_idx`) across every filter bin of a

- ```rust
  pub fn new(values: Vec<f64>, std_devs: Vec<f64>) -> DerivedTally { /* ... */ }
  ```
  Construct directly from parallel mean / std-dev arrays (must be equal length).

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Number of bins.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether the derived tally has no bins.

- ```rust
  pub fn get(self: &Self, i: usize) -> (f64, f64) { /* ... */ }
  ```
  The `(mean, std_dev)` of bin `i`. Panics if out of range.

- ```rust
  pub fn slice(self: &Self, start: usize, end: usize) -> DerivedTally { /* ... */ }
  ```
  A contiguous sub-range `[start, end)` of bins as a new derived tally

- ```rust
  pub fn add(self: &Self, other: &DerivedTally) -> DerivedTally { /* ... */ }
  ```
  Elementwise sum `a + b` with `σ = sqrt(σa² + σb²)`. Panics on length mismatch.

- ```rust
  pub fn sub(self: &Self, other: &DerivedTally) -> DerivedTally { /* ... */ }
  ```
  Elementwise difference `a - b` with `σ = sqrt(σa² + σb²)`. Panics on length

- ```rust
  pub fn mul(self: &Self, other: &DerivedTally) -> DerivedTally { /* ... */ }
  ```
  Elementwise product `a · b` with `σ = |a·b|·sqrt((σa/a)² + (σb/b)²)`.

- ```rust
  pub fn div(self: &Self, other: &DerivedTally) -> DerivedTally { /* ... */ }
  ```
  Elementwise quotient `a / b` with `σ = |a/b|·sqrt((σa/a)² + (σb/b)²)`.

- ```rust
  pub fn scalar_mul(self: &Self, k: f64) -> DerivedTally { /* ... */ }
  ```
  Scale every bin by an **exact** scalar `k`: value `k·a`, `σ = |k|·σa`.

- ```rust
  pub fn sum(self: &Self) -> (f64, f64) { /* ... */ }
  ```
  Reduce all bins to a single `(sum, σ)` with `σ = sqrt(Σ σᵢ²)` (mirrors

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> DerivedTally { /* ... */ }
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
    fn eq(self: &Self, other: &DerivedTally) -> bool { /* ... */ }
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
## Module `physics`

Neutron-transport physics: the collision kernels and the top-level drivers
that iterate them over a geometry.

# Transport drivers (what runs a whole calculation)

- [`keff::run_keff`] — k-eigenvalue power iteration for a homogeneous bare
  sphere (the reference criticality driver; `CPU`/`GPU` backends).
- [`transport_csg::run_keff_csg`] — k-eigenvalue power iteration over
  **general CSG geometry** (surfaces/cells/universes/lattices), with track-
  length tallies. Generalises `keff`.
- [`fixed_source::run_fixed_source`] — **fixed-source** transport: an
  external neutron source (point/box) driving a sub-critical or
  non-multiplying system, scoring track-length tallies. No `k_eff` / power
  iteration; the second canonical MC mode (shielding / detector response).
  Reuses [`transport_csg::transport_history`] for the per-history physics.
- [`physics_mg`] — multigroup transport (group-averaged cross sections;
  pending / partial).

# Collision-level kernels (the per-collision physics the drivers call)

- [`scatter`] — elastic / inelastic scattering, centre-of-mass kinematics.
- [`fission`] — ν̄ sampling and fission-site banking.
- [`compute`] — [`compute::ComputeType`] backend selector shared by the
  drivers (single-thread / multi-thread / GPU).
- [`search`] — reactivity search wrapping the k-eigenvalue driver (root-find
  a geometry/material parameter for a target `k_eff`).

[`transport`] is a stub retained for the generic history-based loop notes;
the live per-history loop is in [`transport_csg`].

```rust
pub mod physics { /* ... */ }
```

### Modules

## Module `compute`

Compute-backend selector for the Monte Carlo transport drivers.

A single [`ComputeType`] value chooses *how* a k-eigenvalue power iteration
executes its per-generation history transport — on one CPU thread, across all
CPU cores with [`rayon`], or with GPU-accelerated cross-section lookups. The
**physics is identical** across backends; only the execution strategy (and,
for the GPU path, the floating-point precision of the cross-section lookup)
differs. Enum dispatch is used deliberately — no trait objects — so every
`match self { … }` site is exhaustively checked at compile time (see the
workspace `CLAUDE.md` "No trait objects" rule).

The driver that honours this selector is
[`run_keff`](crate::physics::keff::run_keff): its [`KeffSettings::compute`]
field is matched on to pick the per-mode entry point
([`run_keff_cpu_single`], [`run_keff_cpu_multi`], [`run_keff_gpu`] in
[`crate::physics::keff`]).

[`KeffSettings::compute`]: crate::physics::keff::KeffSettings::compute
[`run_keff_cpu_single`]: crate::physics::keff::run_keff_cpu_single
[`run_keff_cpu_multi`]: crate::physics::keff::run_keff_cpu_multi
[`run_keff_gpu`]: crate::physics::keff::run_keff_gpu

```rust
pub mod compute { /* ... */ }
```

### Types

#### Enum `ComputeType`

Which compute backend a Monte Carlo transport driver uses to execute its
per-generation neutron histories.

The variants map onto the compute modes the project's maintainer named as
`CPUSingleThread` / `CPUMultiThread` / `GPU`; this enum uses idiomatic Rust
casing (`Cpu` / `Gpu`) so the crate builds clean under clippy's
`upper_case_acronyms` lint. The semantics are exactly those three modes:

| This enum | Maintainer's name | Meaning |
|---|---|---|
| [`CpuSingleThread`](Self::CpuSingleThread) | `CPUSingleThread` | scalar, single-thread |
| [`CpuMultiThread`](Self::CpuMultiThread)  | `CPUMultiThread`  | rayon-parallel over histories |
| [`Gpu`](Self::Gpu)                         | `GPU`             | GPU-accelerated XS lookup, CPU fallback |

# Trust model

[`CpuSingleThread`](Self::CpuSingleThread) is the **trusted, deterministic
reference**: raw `f64` throughout, a single RNG stream threaded through the
generation, so a fixed seed gives a bit-reproducible k-eigenvalue. The other
two backends are **acceleration only** and are validated *against* this
reference within combined statistical uncertainty — never trusted above it
(see this crate's `gpu` module docs and `CLAUDE.md`).

# Portability

The enum and every driver that dispatches on it compile on **all** targets,
including Android (`target_os = "android"`), where the GPU module is
target-gated out. On Android, [`Gpu`](Self::Gpu) transparently runs the CPU
path (there is no adapter to probe), so selecting it is always safe.
[`CpuMultiThread`](Self::CpuMultiThread) is a valid Android backend too —
`rayon` and [`std::thread::available_parallelism`] both work there; a phone
simply resolves to fewer cores, hence fewer threads.

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

Scalar, single-thread transport (maintainer's `CPUSingleThread`).

The current, original behaviour and the **deterministic trusted
reference**: one `f64` RNG stream threaded sequentially through the
generation, so a fixed [`KeffSettings::seed`](crate::physics::keff::KeffSettings::seed)
yields a bit-reproducible k-eigenvalue independent of machine. This is
the default.

###### `CpuMultiThread`

Rayon-parallel transport over the per-generation history bank
(maintainer's `CPUMultiThread`), sized by the carried [`ThreadCount`].

Histories within a generation are embarrassingly parallel, so they are
transported across CPU cores with [`rayon`] in a **dedicated pool** sized
to [`ThreadCount`] — [`ThreadCount::Auto`] (the default) scales with the
machine's logical-core count, so a big desktop CPU gets many threads and
a phone gets few with no special-casing. Each history is given an RNG
stream derived deterministically from `(seed, generation, history index)`
via the LCG jump-ahead, so the result is **reproducible independent of
thread count** and does not race. It will **not** bit-match
[`CpuSingleThread`](Self::CpuSingleThread) — the per-history stream
structure differs from the single sequential stream — but it agrees
within combined statistical uncertainty.

Construct the default form with `CpuMultiThread(ThreadCount::Auto)`.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `ThreadCount` |  |

###### `Gpu`

GPU-accelerated transport (maintainer's `GPU`), with graceful CPU
fallback.

Uses the headless `wgpu` cross-section lookup
([`crate::gpu::union_grid::UnionTotalXs`]) to serve the macroscopic total
Sigma_t during the sweep on the GPU in `f32`. If no GPU adapter is
available — a headless server, CI with no Vulkan/Metal loader, or Android
where the GPU module is compiled out — the driver emits a
`log::debug!` line and transparently runs the CPU path instead. It
**never errors on a missing GPU.** GPU `f32` results are acceleration
only and are held to a tolerance against the CPU reference.

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

The transport driver resolves this to a concrete positive thread count with
[`ThreadCount::resolve`] and builds a dedicated [`rayon::ThreadPool`] of that
size. The default is [`Auto`](Self::Auto), which reads the machine's logical
core count via [`std::thread::available_parallelism`] — a gaming desktop
naturally gets many threads, an Android phone gets few, with no
special-casing. All variants resolve to **at least 1** thread.

```rust
pub enum ThreadCount {
    Auto,
    Fixed(usize),
    Fraction(f64),
}
```

##### Variants

###### `Auto`

Use every logical core: [`std::thread::available_parallelism`]. Scales
with the CPU's strength; falls back to 1 if the query fails. The default.

###### `Fixed`

An explicit worker-thread count. Clamped up to a minimum of 1.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

###### `Fraction`

A fraction of the available logical cores, e.g. `0.5` = half. The product
`fraction * cores` is rounded to the nearest integer and clamped to at
least 1 (so any positive fraction always yields a runnable pool).

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
## Module `transport`

```rust
pub mod transport { /* ... */ }
```

## Module `transport_csg`

k-eigenvalue power iteration over **general CSG geometry** with surface
tracking and boundary conditions.

This generalises [`crate::physics::keff::run_keff`] (which hard-codes a single
bare sphere) to any [`Geometry`]: an arbitrary set of CSG cells, nested
universes and rectangular lattices, with per-surface vacuum / reflective
boundary conditions. It is the assembly point for the `pincell` and
`rectangular-lattice` verification cases (beads op-6tz.8 / op-6tz.10), built
on the geometry navigation of [`Geometry::locate`] and
[`Geometry::distance_to_boundary`] (op-6tz.7).

# Transport algorithm (surface tracking)

Ported in structure from OpenMC `transport_history_based` /
`distance_to_boundary` (`src/physics.cpp`, `src/geometry.cpp`). For each
history the loop is:

1. [`Geometry::locate`] the particle → leaf material and coordinate chain.
2. Sample distance to collision `d_col = −ln ξ / Σ_t(E)` (∞ in void).
3. [`Geometry::distance_to_boundary`] → nearest surface/lattice crossing `d_b`.
4. If a tally is attached, deposit the **track-length** flux/reaction-rate
   contribution for the segment just streamed (`w·d` per bin, at the segment's
   cell + energy) into the current batch's accumulator.
5. If `d_col < d_b`: stream to the collision, sample the reaction (the same
   analog reaction partition as `keff.rs`), and bank fission neutrons.
6. Else: stream to the boundary and apply the crossing
   ([`Geometry::cross_surface`]): reflect off a reflective surface, die at a
   vacuum surface, otherwise pass through and re-locate.

Fission neutrons feed the next generation's source bank; the generation
eigenvalue is `k = (Σ ν̄ over fissions) / n_particles`, averaged over the
active generations exactly as in `keff.rs`.

# Fidelity

Analog transport (weight 1, no implicit capture or variance reduction), same
collision physics and data tiers as [`crate::physics::keff`]. Tallies use the
**track-length estimator**: each streamed segment of length `d` deposits `w·d`
(flux) and `w·d·Σ_x` (reaction rates) into its cell × energy bin, accumulated
per generation and flushed as one realization per active batch (see
[`crate::tally::scoring::score_track_length`] / `flush_batch`). The
collision estimator ([`crate::tally::scoring::score_collision`]) remains
available as an alternative.

**S(α,β) thermal scattering (bead op-6tz.12).** A moderator nuclide carrying a
[`crate::material::thermal::ThermalScattering`] table (attached via
[`Nuclide::with_thermal_scattering`]) now thermalizes correctly: below the
table's cutoff (~4 eV) the scatter branch samples the bound-atom S(α,β) law
([`Nuclide::sample_thermal`]) — lab-frame outgoing energy and cosine, with the
up-scatter that builds a Maxwellian — instead of the free-gas elastic kernel.
Nuclides without a table (fuel, O, clad) stay free-gas/CE. This makes a
*thermal* LWR pin-cell tractable; see the `pincell` verification test.

```rust
pub mod transport_csg { /* ... */ }
```

### Types

#### Struct `SourceBox`

How the initial fission source is seeded spatially — a box the sampler
rejects into the fissile region of the geometry.

The transport itself needs no source region (fission sites regenerate it);
this only bootstraps generation 0. Points are drawn uniformly in the box and
kept only if they land in a cell whose material can fission.

```rust
pub struct SourceBox {
    pub lower: crate::geometry::position::Position,
    pub upper: crate::geometry::position::Position,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `lower` | `crate::geometry::position::Position` | Lower corner \[cm\]. |
| `upper` | `crate::geometry::position::Position` | Upper corner \[cm\]. |

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
    fn clone(self: &Self) -> SourceBox { /* ... */ }
    ```

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
### Functions

#### Function `run_keff_csg`

Run fission-source power iteration over an arbitrary CSG [`Geometry`].

# Parameters
- `geom` — the CSG model (surfaces/cells/universes/lattices + root universe).
- `materials` — global material array the geometry's cells index into.
- `nuclides` — global nuclide array the materials index into.
- `source_box` — box the initial source is rejection-sampled into (must
  overlap the fissile region).
- `settings` — power-iteration controls (see [`KeffSettings`]).
- `tally` — optional track-length tally scored on active generations. Its
  `bins` are accumulated in place, one realization per active generation.

Returns the mean eigenvalue and standard error over the active generations
(a [`KeffResult`], same type as the bare-sphere driver). If a `tally` is
supplied its bins are accumulated in place.

# Compute backend

This is a thin **dispatcher** over [`settings.compute`](KeffSettings::compute),
mirroring [`crate::physics::keff::run_keff`]. The physics is identical across
backends; only the execution strategy differs:

- [`ComputeType::CpuSingleThread`] → [`run_keff_csg_seq`], the scalar,
  single-RNG-stream **reference** — deterministic and bit-reproducible for a
  fixed seed.
- [`ComputeType::CpuMultiThread`] → [`run_keff_csg_par`], [`rayon`]-parallel
  histories per generation, each with an independent jump-ahead RNG stream so
  the result is reproducible independent of thread count. It does **not**
  bit-match the single-thread reference but agrees within combined statistical
  uncertainty.
- [`ComputeType::Gpu`] → **no GPU kernel exists for general CSG geometry** (the
  crate's GPU transport, [`crate::physics::keff::run_keff_gpu`], is bare-sphere
  only), so this transparently runs the multi-threaded CPU path and emits a
  `log::debug!` line. It never errors on the selection. Wiring a genuine GPU
  Sigma_t lookup into CSG/delta transport is tracked as follow-up work
  (bead op-fla).

```rust
pub fn run_keff_csg(geom: &crate::geometry::geometry::Geometry, materials: &[crate::material::material::Material], nuclides: &[crate::material::nuclide::Nuclide], source_box: SourceBox, settings: &crate::physics::keff::KeffSettings, tally: Option<&mut crate::tally::tally::Tally>) -> crate::physics::keff::KeffResult { /* ... */ }
```

#### Function `run_keff_csg_seq`

Scalar, single-thread CSG power iteration — the **trusted, deterministic,
bit-reproducible reference** backend ([`ComputeType::CpuSingleThread`]).

One `f64` RNG stream is threaded sequentially through the whole run (initial
source rejection-sampling, every history's transport, every resample), so a
fixed [`KeffSettings::seed`] yields the same eigenvalue — and the same tally
realizations — bit-for-bit on every machine. [`run_keff_csg_par`] is
acceleration only and is validated against this reference.

```rust
pub fn run_keff_csg_seq(geom: &crate::geometry::geometry::Geometry, materials: &[crate::material::material::Material], nuclides: &[crate::material::nuclide::Nuclide], source_box: SourceBox, settings: &crate::physics::keff::KeffSettings, tally: Option<&mut crate::tally::tally::Tally>) -> crate::physics::keff::KeffResult { /* ... */ }
```

#### Function `run_keff_csg_par`

Rayon-parallel CSG power iteration ([`ComputeType::CpuMultiThread`]).

Same physics and power-iteration structure as [`run_keff_csg_seq`], but the
histories **within each generation** are transported in parallel with
[`rayon`] in a dedicated pool sized to `thread_count` (never the implicit
global pool). The generation loop stays sequential — generation `g+1`'s source
is `g`'s resampled fission bank, a hard data dependency.

# Reproducibility (independent of thread count)

Each history is given a **completely independent, deterministic** RNG stream
derived only from `(settings.seed, generation, history index)` via the LCG
jump-ahead ([`crate::rng::lcg::future_seed`]) — never a shared mutable seed —
so the result never races and is identical regardless of how rayon schedules
the work. This mirrors [`crate::physics::keff::run_keff_cpu_multi`]; see its
docs for the `HIST_STRIDE` / `GEN_STRIDE` non-overlap argument. The initial
source sampling and each resample run on a separate sequential `src_seed`
stream, kept off the parallel path. Because the per-history stream structure
differs from the single sequential stream, this backend does **not** bit-match
[`run_keff_csg_seq`] — it is a statistically independent estimate of the same
eigenvalue and tally, agreeing within combined uncertainty.

# Tally

When a `tally` is attached, each history accumulates its track-length scores
into a **private per-history batch**; the batches are summed in history-index
order into the generation batch (a deterministic reduction) and flushed as one
realization per active generation — the same batch/flush contract as
[`run_keff_csg_seq`], just reduced in parallel.

```rust
pub fn run_keff_csg_par(geom: &crate::geometry::geometry::Geometry, materials: &[crate::material::material::Material], nuclides: &[crate::material::nuclide::Nuclide], source_box: SourceBox, settings: &crate::physics::keff::KeffSettings, tally: Option<&mut crate::tally::tally::Tally>, thread_count: crate::physics::compute::ThreadCount) -> crate::physics::keff::KeffResult { /* ... */ }
```

## Module `fixed_source`

**Fixed-source** Monte Carlo transport — an external neutron source driving
a (sub-critical or non-multiplying) system, scoring track-length tallies.

# New orchestration over the ported collision loop

Per the crate porting rule, the *physics* here is **not** reinvented: every
flight, collision, reaction and boundary crossing is the already-ported
[`transport_history`](crate::physics::transport_csg::transport_history) (the
translation of OpenMC `src/physics.cpp`). What is new — and marked as new,
not a port — is the fixed-source **orchestration** around it: sample source
particles from an external [`FixedSource`], transport each to death, and
transport any fission secondaries they produce (sub-critical multiplication)
until the bank drains. There is no fission-source power iteration and no
`k_eff`; the result is the flux/reaction tally the source induces.

This complements [`crate::physics::keff`] (k-eigenvalue / criticality) with
the second canonical Monte Carlo mode — **shielding / detector-response**
style problems (attenuation, leakage, flux far from a source).

# Scope

Analog transport (no variance reduction). Fission neutrons are tracked as
secondaries with a per-source-particle safety cap, so a **sub-critical**
(`k < 1`) or non-multiplying system converges; a super-critical system would
multiply without bound and is capped (and physically meaningless for a fixed
source). Neutron-only, offline data. See the workspace `RESPONSIBLE_USE.md`.

```rust
pub mod fixed_source { /* ... */ }
```

### Types

#### Enum `FixedSource`

An external neutron source for a fixed-source run. Isotropic in direction;
mono-energetic in energy (the common shielding/detector case). Position is
either a point or uniform in an axis-aligned box.

```rust
pub enum FixedSource {
    Point {
        r: crate::geometry::position::Position,
        energy_ev: f64,
    },
    Box {
        lower: crate::geometry::position::Position,
        upper: crate::geometry::position::Position,
        energy_ev: f64,
    },
}
```

##### Variants

###### `Point`

Isotropic point source at `r` \[cm\] emitting neutrons of energy
`energy_ev` \[eV\].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `r` | `crate::geometry::position::Position` | Emission point \[cm\]. |
| `energy_ev` | `f64` | Emission energy \[eV\]. |

###### `Box`

Isotropic source sampled uniformly in the box `[lower, upper]` \[cm\],
energy `energy_ev` \[eV\].

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `lower` | `crate::geometry::position::Position` | Lower corner \[cm\]. |
| `upper` | `crate::geometry::position::Position` | Upper corner \[cm\]. |
| `energy_ev` | `f64` | Emission energy \[eV\]. |

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
    fn clone(self: &Self) -> FixedSource { /* ... */ }
    ```

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
#### Struct `FixedSourceSettings`

Settings for a fixed-source run.

```rust
pub struct FixedSourceSettings {
    pub n_particles: usize,
    pub n_batches: usize,
    pub temperature_k: f64,
    pub seed: u64,
    pub max_secondaries: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `n_particles` | `usize` | Number of source particles to sample and transport. |
| `n_batches` | `usize` | Number of statistical batches (realizations) the particles are split into<br>for the tally's mean/uncertainty. Each batch is flushed as one<br>realization; read a tally with `n_batches` as the realization count. |
| `temperature_k` | `f64` | Material temperature \[K\] for the cross-section lookup. |
| `seed` | `u64` | Master RNG seed (fixed → reproducible on the single-thread path). |
| `max_secondaries` | `usize` | Safety cap on fission secondaries transported per source particle — the<br>backstop against runaway multiplication if a (mis-specified)<br>super-critical system is run as a fixed source. |

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
    fn clone(self: &Self) -> FixedSourceSettings { /* ... */ }
    ```

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
#### Struct `FixedSourceResult`

The outcome of a fixed-source run (tally results are accumulated in place into
the caller's `Tally`).

```rust
pub struct FixedSourceResult {
    pub source_particles: usize,
    pub total_histories: usize,
    pub multiplication: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `source_particles` | `usize` | Source particles transported. |
| `total_histories` | `usize` | Total histories transported (source particles **plus** every fission<br>secondary) — `> source_particles` reveals sub-critical multiplication. |
| `multiplication` | `f64` | Mean number of fission neutrons produced per **source** particle<br>(`Σ ν·σ_f / σ_t` over collisions) — the sub-critical multiplication `M`.<br>`0` for a non-fissile (shielding) system. |

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
    fn clone(self: &Self) -> FixedSourceResult { /* ... */ }
    ```

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
### Functions

#### Function `run_fixed_source`

Run a fixed-source transport calculation.

Samples `settings.n_particles` neutrons from `source`, transports each (and
its fission secondaries) through `geom` to death, and — if a `tally` is
supplied — accumulates its track-length scores in place (flushed once per
batch, so read it back with `settings.n_batches` realizations). Returns a
[`FixedSourceResult`] balance.

Single-threaded reference path (deterministic for a fixed `seed`).

# Example — void streaming (the analytic check)
```
use outram_mc_libs::physics::fixed_source::{run_fixed_source, FixedSource, FixedSourceSettings};
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{Sphere, SurfaceKind, BoundaryType};
use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::geometry::geometry::Geometry;

// A vacuum sphere of radius R: a point source at the centre streams straight
// out, so every neutron travels exactly R — the mean path length is R.
let r_cm = 5.0;
let geom = Geometry {
    surfaces: vec![SurfaceKind::Sphere(Sphere { x0: 0.0, y0: 0.0, z0: 0.0, r: r_cm, bc: BoundaryType::Vacuum })],
    cells: vec![Cell::fill(1, vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Inside }], CellFill::Void, Position::ZERO)],
    universes: vec![Universe { id: 0, cell_indices: vec![0] }],
    lattices: vec![],
    root_universe: 0,
};
let src = FixedSource::Point { r: Position::ZERO, energy_ev: 2.0e6 };
let res = run_fixed_source(&geom, &[], &[], &src, &FixedSourceSettings { n_particles: 1000, ..Default::default() }, None);
assert_eq!(res.total_histories, 1000); // void: no collisions, no secondaries
```

```rust
pub fn run_fixed_source(geom: &crate::geometry::geometry::Geometry, materials: &[crate::material::material::Material], nuclides: &[crate::material::nuclide::Nuclide], source: &FixedSource, settings: &FixedSourceSettings, tally: Option<&mut crate::tally::tally::Tally>) -> FixedSourceResult { /* ... */ }
```

## Module `scatter`

Neutron scattering kinematics — elastic and inelastic.

C++ source: `src/physics_common.cpp`, `src/physics.cpp`.

The channels differ in both their outgoing *energy* law and their angular law.
Elastic scatter can use an anisotropic centre-of-mass distribution (ENDF MF=4,
sampled by the caller and passed as `mu_cm`) — the dominant reactivity lever for
a bare fast-metal sphere, where forward-peaked elastic off heavy nuclei sets the
transport cross section and hence the leakage. The inelastic channels remain
isotropic-CM in angle for now. By outgoing-energy law:

- **Elastic** (MT=2) — [`elastic_scatter`]: two-body kinematics with `Q = 0`;
  off a heavy actinide the neutron loses almost no energy per collision
  (α = ((A−1)/(A+1))² ≈ 0.98 for A ≈ 238).
- **Discrete-level inelastic** (MT=51…90) — [`two_body_scatter`] with the
  level's `Q < 0`: the neutron gives up the level excitation energy, a *large*
  per-collision energy loss (tens of keV to MeV) that softens the fast
  spectrum. This is the dominant fast-spectrum energy-loss mechanism for heavy
  nuclei, and its absence (inelastic lumped into elastic) was the leading bias
  in the first Godiva Keff — see `docs/development-history.md`.
- **Continuum inelastic** (MT=91) — [`continuum_inelastic_scatter`]: the
  outgoing energy is a distribution, not fixed by a single `Q`. RECONR does not
  reconstruct the ENDF MF=5 continuum law, so this uses a **Weisskopf
  evaporation** model with a nuclear temperature θ = √(E/a), level-density
  parameter a ≈ A/11 MeV⁻¹ (actinide) — an approximation, documented as such.

Anisotropic elastic uses the full ENDF MF=4 tabulated cosine distribution
(sampled in `material::nuclide`, ported from OpenMC), passed here as a CM cosine
via [`two_body_scatter_with_mu`]. Anisotropic *inelastic* angular laws (coupled
to the MF=5/MF=6 energy distributions) remain future work.

```rust
pub mod scatter { /* ... */ }
```

### Functions

#### Function `rotate_direction`

Rotate the unit direction `u` by scattering cosine `mu` and a uniformly
sampled azimuth φ ∈ [0, 2π), returning the new unit direction.

This is the standard OpenMC `rotate_angle`: it builds an orthonormal frame
around `u` and tilts by `(mu, φ)`. The near-pole branch (`|w| ≈ 1`) rotates
about the x-axis instead to avoid dividing by √(1−w²) ≈ 0.

```rust
pub fn rotate_direction(u: crate::geometry::position::Direction, mu: f64, seed: &mut u64) -> crate::geometry::position::Direction { /* ... */ }
```

#### Function `two_body_scatter`

Two-body scatter a neutron of energy `e` \[eV\] and direction `u` off a target
of atomic weight ratio `awr` with reaction Q-value `q` \[eV\], isotropic in the
centre-of-mass frame.

Returns `(e_out, u_out)`. Two-body kinematics fix the outgoing neutron CM
energy from the Q-value (target at rest):

`E_cm = E·(A/(A+1))² + Q·A/(A+1)`,

which is then transformed to the lab via [`cm_to_lab`]. `Q = 0` is elastic;
`Q < 0` is endothermic (a discrete inelastic level of excitation energy |Q|,
with threshold `E = |Q|·(A+1)/A`). Below threshold `E_cm` would be negative;
it is clamped to zero, but the caller should not select a channel below its
(zero) cross section there.

For an anisotropic CM angular law use [`two_body_scatter_with_mu`].

```rust
pub fn two_body_scatter(e: f64, u: crate::geometry::position::Direction, awr: f64, q: f64, seed: &mut u64) -> (f64, crate::geometry::position::Direction) { /* ... */ }
```

#### Function `two_body_scatter_with_mu`

Two-body scatter with a **caller-supplied** centre-of-mass scattering cosine
`mu_cm` — the anisotropic form of [`two_body_scatter`].

Identical to [`two_body_scatter`] except the CM cosine is provided rather than
sampled isotropically: the outgoing CM energy is fixed by the Q-value and
[`cm_to_lab`] maps it to the lab. Use this when an angular distribution (ENDF
MF=4, sampled elsewhere) supplies `mu_cm`; only the azimuth is sampled here.

`mu_cm` is the cosine in the **CM frame** — the frame ENDF elastic angular
distributions are given in (LCT=2) — and is clamped to `[−1, 1]`.

```rust
pub fn two_body_scatter_with_mu(e: f64, u: crate::geometry::position::Direction, awr: f64, q: f64, mu_cm: f64, seed: &mut u64) -> (f64, crate::geometry::position::Direction) { /* ... */ }
```

#### Function `elastic_scatter`

Elastic scatter a neutron of energy `e` \[eV\] and direction `u` off a target
of atomic weight ratio `awr`, isotropic in the centre-of-mass frame.

The `Q = 0` special case of [`two_body_scatter`]. Outgoing energy stays in
`[α·E, E]` with `α = ((A−1)/(A+1))²`; off heavy actinides that is a per-collision
loss of at most a couple of percent.

```rust
pub fn elastic_scatter(e: f64, u: crate::geometry::position::Direction, awr: f64, seed: &mut u64) -> (f64, crate::geometry::position::Direction) { /* ... */ }
```

#### Function `continuum_inelastic_scatter`

Continuum inelastic scatter (MT=91) — the outgoing neutron energy is sampled
from a **Weisskopf evaporation spectrum** rather than fixed by a single level.

`f(E'_cm) ∝ E'_cm · exp(−E'_cm/θ)` with nuclear temperature `θ = √(E/a)` and
level-density parameter `a ≈ A/11 MeV⁻¹` (a standard actinide value). The
sampled CM energy is capped below the elastic CM energy `E·(A/(A+1))²` so the
collision always loses energy, then transformed to the lab isotropically in CM.

This is an **approximation**: RECONR reconstructs cross sections (MF=3) but not
the ENDF MF=5 secondary-energy law, so the true continuum distribution is not
available here. The evaporation model captures the essential physics — a large,
broadly distributed down-scatter — which is what softens the fast spectrum.

```rust
pub fn continuum_inelastic_scatter(e: f64, u: crate::geometry::position::Direction, awr: f64, seed: &mut u64) -> (f64, crate::geometry::position::Direction) { /* ... */ }
```

## Module `fission`

Fission neutron production.

C++ source: `src/physics.cpp` — `fission()`, `create_fission_sites()`.

For a k-eigenvalue calculation each fission collision banks an integer number
of secondary neutrons for the *next* generation. This module ports the
neutron-count sampler; the fission-site energy/direction come from the source
samplers ([`crate::rng::distributions::watt`],
[`crate::rng::distributions::isotropic_direction`]) and the banking itself is
driven by the eigenvalue loop ([`crate::physics::keff`]).

**Delayed neutrons** are folded into the total ν̄ and treated as prompt — the
standard eigenvalue approximation (prompt + delayed born at the same instant).

```rust
pub mod fission { /* ... */ }
```

### Functions

#### Function `sample_num_neutrons`

Sample the integer number of fission neutrons to bank from one fission event.

The expected yield per fission is ν̄; dividing by the running eigenvalue guess
`keff` keeps the fission bank's population stationary from generation to
generation (OpenMC's `create_fission_sites` normalisation). The expected
value `ν̄/keff` is split into a deterministic integer part plus a Bernoulli
draw on the fractional part:

`n = ⌊ν̄/keff⌋ + [ξ < frac(ν̄/keff)]`.

A non-positive or non-finite `keff` falls back to `keff = 1` so a diverging
first generation can't produce a nonsensical count.

```rust
pub fn sample_num_neutrons(nu_bar: f64, keff: f64, seed: &mut u64) -> usize { /* ... */ }
```

## Module `keff`

k-eigenvalue power iteration for a homogeneous bare sphere.

This is the minimal criticality driver — the first end-to-end assembly of the
transport kernel described in `docs/keff-doppler-roadmap.md` (Priority 1). It
deliberately handles only the simplest geometry (one sphere, vacuum outside,
one homogeneous material) so the physics can be exercised without the full CSG
machinery. The pieces it composes:

- **Geometry** — [`crate::geometry::surface::Sphere::distance`] for the one
  surface crossing; "inside" is just `|r| < R`.
- **Data** — macroscopic cross sections from [`Material`], which pulls
  microscopic σ(E,T) from `njoy-outram-park-fork` via [`Nuclide`].
- **Physics** — analog collisions: elastic scatter
  ([`crate::physics::scatter::elastic_scatter`]), fission banking
  ([`crate::physics::fission::sample_num_neutrons`]), and analog capture.
- **Source** — Watt fission energy + isotropic direction for banked neutrons.

# Compute backends

[`run_keff`] is a thin dispatcher over [`KeffSettings::compute`]
([`ComputeType`]). All three backends run the **same physics**; they differ
only in *how* the per-generation histories are executed:

- [`run_keff_cpu_single`] — scalar, single RNG stream. The trusted,
  deterministic, bit-reproducible reference.
- [`run_keff_cpu_multi`] — the histories of a generation transported in
  parallel with [`rayon`], each on an independent deterministic RNG sub-stream.
- [`run_keff_gpu`] — GPU-accelerated macroscopic-Sigma_t lookup, with a
  transparent CPU fallback when no GPU adapter is present.

# Algorithm

Standard fission-source power iteration. Each *generation* transports
`n_particles` histories from the current fission bank; every fission event
contributes ν̄ to the generation's production tally and banks ⌊ν̄/k⌋(+1) sites
for the next generation. The generation eigenvalue is
`k = (Σ ν̄ over fissions) / n_particles`. The first `n_inactive` generations
let the source distribution converge and are discarded; the mean over the
remaining `n_active` generations is the reported k, with the standard error of
that mean.

# Fidelity

Analog transport (no implicit capture / weight windows), target at rest. Both
data tiers now model inelastic down-scatter and forward-peaked elastic; they
differ in how finely that physics is resolved:

- **HIGH tier** ([`Nuclide::from_endf`]) carries the resolved inelastic level
  structure (MT=51…91), so inelastic is a distinct channel with a real
  energy-loss law — discrete-level two-body kinematics (each level's Q-value)
  and a Weisskopf-evaporation continuum. Elastic uses the full ENDF MF=4
  anisotropic angular distribution (per-energy tabulated cosine CDF). `(n,2n)`
  (MT=16, from the reconstructed MF=3 background) is a distinct channel that
  emits its true **yield-2 multiplicity** — one extra same-generation neutron,
  the small positive reactivity a bare fast sphere would otherwise drop. Fission
  neutrons are born from the nuclide's **energy-dependent ENDF MF=5 χ(E→E')**
  (LF=1) rather than a fixed thermal-Watt spectrum.
- **LOW tier** ([`Nuclide::from_core`]) has no resolved levels: inelastic is the
  group remainder (total − elastic − fission − capture), down-scattered by the
  Weisskopf continuum law. Elastic is forward-peaked from a single per-group
  mean cosine μ̄ (baked from MF=4) via a maximum-entropy exponential angular law.
  Above each nuclide's WMP `e_max` the group data is infinite-dilution
  Watt-collapsed with no self-shielding. `(n,2n)` has no group column yet, so
  the LOW tier still lumps it into elastic (no multiplication) — a pending bake.

For a bare fast sphere, forward-peaked elastic and inelastic down-scatter are
the dominant reactivity levers — together they bring **both** tiers' Godiva Keff
into agreement with the ICSBEP benchmark (see `docs/development-history.md`).

# Example

```no_run
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::{run_keff, KeffSettings};

// Godiva: bare HEU sphere, r ≈ 8.741 cm.
let nuclides = vec![
    Nuclide::from_core("U234").unwrap(),
    Nuclide::from_core("U235").unwrap(),
    Nuclide::from_core("U238").unwrap(),
];
let mat = Material {
    id: 1,
    name: "HEU".into(),
    temperature: 293.6,
    components: vec![
        NuclideComponent { nuclide_idx: 0, atom_density: 4.9184e-4 },
        NuclideComponent { nuclide_idx: 1, atom_density: 4.4994e-2 },
        NuclideComponent { nuclide_idx: 2, atom_density: 2.4984e-3 },
    ],
};
let result = run_keff(8.7407, &mat, &nuclides, &KeffSettings::default());
println!("k = {:.5} ± {:.5}", result.k_mean, result.k_std);
```

```rust
pub mod keff { /* ... */ }
```

### Types

#### Struct `KeffSettings`

Settings for a [`run_keff`] power iteration.

```rust
pub struct KeffSettings {
    pub n_particles: usize,
    pub n_inactive: usize,
    pub n_active: usize,
    pub temperature_k: f64,
    pub seed: u64,
    pub watt_a: f64,
    pub watt_b: f64,
    pub compute: crate::physics::compute::ComputeType,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `n_particles` | `usize` | Neutron histories per generation. More ⇒ lower per-generation noise. |
| `n_inactive` | `usize` | Inactive (source-convergence) generations, discarded from the k tally. |
| `n_active` | `usize` | Active generations averaged into the reported eigenvalue. |
| `temperature_k` | `f64` | Material/data temperature \[K\] used for Doppler-broadened lookups. |
| `seed` | `u64` | Master RNG seed. Fixed seed ⇒ bit-reproducible run. |
| `watt_a` | `f64` | Watt fission-spectrum parameter `a` \[eV\] for banked neutron energies. |
| `watt_b` | `f64` | Watt fission-spectrum parameter `b` \[eV⁻¹\]. |
| `compute` | `crate::physics::compute::ComputeType` | Which transport backend [`run_keff`] dispatches to.<br><br>- [`ComputeType::CpuSingleThread`] — the scalar, single-RNG-stream path<br>  ([`run_keff_cpu_single`]); the trusted, bit-reproducible **deterministic<br>  reference**. This is the [`Default`].<br>- [`ComputeType::CpuMultiThread`] — [`rayon`]-parallel over the histories<br>  of each generation ([`run_keff_cpu_multi`]) in a dedicated pool sized by<br>  the carried [`ThreadCount`] (default [`ThreadCount::Auto`] = every<br>  logical core); each history runs on its own deterministically derived RNG<br>  sub-stream, so the eigenvalue is reproducible independent of thread count<br>  (but does **not** bit-match the single-thread stream — see that<br>  function's docs).<br>- [`ComputeType::Gpu`] — GPU-accelerated macroscopic Sigma_t lookup<br>  ([`run_keff_gpu`]), with a transparent CPU fallback (never an error) when<br>  no GPU adapter is available. The GPU is `f32` acceleration only; the CPU<br>  single-thread path stays the trusted reference. |

##### Implementations

###### Methods

- ```rust
  pub fn with_compute(self: Self, compute: ComputeType) -> Self { /* ... */ }
  ```
  Return a copy of these settings with the transport backend set to

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> KeffSettings { /* ... */ }
    ```

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
    A modest run (2000 histories × [30 inactive + 70 active]) with the

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
#### Struct `KeffResult`

Result of a [`run_keff`] power iteration.

```rust
pub struct KeffResult {
    pub k_mean: f64,
    pub k_std: f64,
    pub k_by_generation: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `k_mean` | `f64` | Mean eigenvalue over the active generations. |
| `k_std` | `f64` | Standard error of the mean (1σ) over the active generations. |
| `k_by_generation` | `Vec<f64>` | Per-generation eigenvalue estimates, all generations (inactive first). |

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
    fn clone(self: &Self) -> KeffResult { /* ... */ }
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

#### Function `run_keff`

Run fission-source power iteration on a bare sphere of radius `radius_cm`
(centred at the origin, vacuum outside) filled with `material`.

`nuclides` is the global nuclide array the material's components index into.
Returns the mean eigenvalue and its standard error over the active
generations. See the module docs for the algorithm and fidelity caveats.

This function is a thin **dispatcher**: it selects the transport backend from
[`settings.compute`](KeffSettings::compute) and forwards to the matching
entry point. The physics is identical across backends.

- [`ComputeType::CpuSingleThread`] → [`run_keff_cpu_single`] (the trusted,
  bit-reproducible reference),
- [`ComputeType::CpuMultiThread`] → [`run_keff_cpu_multi`] (rayon-parallel),
- [`ComputeType::Gpu`] → [`run_keff_gpu`] (GPU Sigma_t lookup, CPU fallback).

```rust
pub fn run_keff(radius_cm: f64, material: &crate::material::material::Material, nuclides: &[crate::material::nuclide::Nuclide], settings: &KeffSettings) -> KeffResult { /* ... */ }
```

#### Function `run_keff_cpu_single`

Scalar, single-thread fission-source power iteration — the **trusted,
deterministic, bit-reproducible reference** backend
([`ComputeType::CpuSingleThread`]).

This is the original [`run_keff`] algorithm: raw `f64` throughout, with a
**single** RNG `seed` threaded sequentially through the whole run (initial
source sampling, every history's transport, and every generation's resample
all draw from the one stream). A fixed [`KeffSettings::seed`] therefore yields
the same eigenvalue bit-for-bit on every machine. The other two backends
([`run_keff_cpu_multi`], [`run_keff_gpu`]) are acceleration only and are
validated *against* this reference, never above it.

Returns the mean eigenvalue and its standard error over the active
generations. See the module docs for the algorithm and fidelity caveats.

```rust
pub fn run_keff_cpu_single(radius_cm: f64, material: &crate::material::material::Material, nuclides: &[crate::material::nuclide::Nuclide], settings: &KeffSettings) -> KeffResult { /* ... */ }
```

#### Function `run_keff_cpu_multi`

Rayon-parallel fission-source power iteration ([`ComputeType::CpuMultiThread`]).

Same physics and same power-iteration structure as [`run_keff_cpu_single`],
but the histories **within each generation** are transported in parallel with
[`rayon`]. The generation loop itself stays sequential — generation `g+1`'s
source is the resampled fission bank of generation `g`, a hard data
dependency.

# Thread pool sizing

The parallel sections run inside a **dedicated** [`rayon::ThreadPool`] sized
to `thread_count.resolve()` (min 1), **not** the implicit global pool — the
caller gets explicit, controllable sizing. [`ThreadCount::Auto`] (the
default) resolves via [`std::thread::available_parallelism`], so the pool
scales with the CPU's strength: a big desktop CPU gets many threads, an
Android phone gets few, with no special-casing. [`ThreadCount::Fixed`] pins
an exact count and [`ThreadCount::Fraction`] takes a fraction of the logical
cores (both clamped to `>= 1`). This whole path is Android-clean — `rayon`
and `available_parallelism` both work there — so it is **not** target-gated.

# Reproducibility (independent of thread count)

Each history is given a **completely independent, deterministic** RNG stream
derived only from `(settings.seed, generation index, history index)` — never
from a shared mutable seed — so the result never races and is identical
regardless of how rayon schedules the work. The derivation uses the LCG
jump-ahead ([`crate::rng::lcg::future_seed`]), mirroring OpenMC's per-particle
independent-stream design (`src/particle.cpp`, `src/random_lcg.cpp`
`init_seed`/`future_seed`, the reproducibility guarantee):

- `gen_base_seed = future_seed(gen * GEN_STRIDE, settings.seed)` places each
  generation `GEN_STRIDE = 2^40` draws apart in jump-ahead index space;
- `hist_seed = future_seed(hist_idx * HIST_STRIDE, gen_base_seed)` places each
  history `HIST_STRIDE = 152917` draws apart within its generation.

**Non-overlap argument.** A single bare-sphere history draws far fewer than
`HIST_STRIDE` random numbers, so history `h` stays inside its
`[h*HIST_STRIDE, (h+1)*HIST_STRIDE)` slot and never touches history `h+1`'s
stream. The largest offset any generation uses is `n_particles * HIST_STRIDE`,
which is `< GEN_STRIDE` for any `n_particles < 2^40 / HIST_STRIDE ≈ 7.5e6`, so
generations never overlap either.

The **initial source sampling** and each generation's **resample** run on a
*separate* single sequential seed stream (`src_seed`, started at
`settings.seed`) — cheap, order-independent bookkeeping kept deterministic and
off the parallel path.

# Agreement with the reference

Because the per-history stream structure differs from the single sequential
stream, this backend does **not** bit-match [`run_keff_cpu_single`]. It is a
statistically independent estimate of the same eigenvalue and agrees with the
reference within combined statistical uncertainty.

```rust
pub fn run_keff_cpu_multi(radius_cm: f64, material: &crate::material::material::Material, nuclides: &[crate::material::nuclide::Nuclide], settings: &KeffSettings, thread_count: crate::physics::compute::ThreadCount) -> KeffResult { /* ... */ }
```

#### Function `run_keff_gpu`

GPU-accelerated fission-source power iteration ([`ComputeType::Gpu`]), with a
**graceful CPU fallback** — never an error, never a panic on a missing GPU.

If a usable GPU adapter is present ([`crate::gpu::probe`] returns `Some`), the
run is handed to [`run_keff_gpu_batched`] — the **event-based batched-flight**
path, which keeps a whole batch of live neutrons resident in GPU buffers and
advances them one flight at a time on the GPU (RNG draw + native-union Sigma_t
lookup + collision-distance sample + streaming + leak test), leaving only the
branchy per-collision reaction physics on the CPU (see that function for
exactly how far the GPU reaches into the loop). The earlier
[`run_keff_gpu_inner`] (first-flight-only GPU Sigma_t) is retained for
comparison but no longer the default. If no adapter is available — a headless
server, CI with no Vulkan/Metal loader, or **Android**, where the whole `wgpu`
path is compiled out — it emits a `log::debug!` line and transparently runs
the trusted [`run_keff_cpu_single`] reference instead.

The GPU path is `f32` **acceleration only**; the single-thread CPU path
remains the trusted, deterministic reference.

```rust
pub fn run_keff_gpu(radius_cm: f64, material: &crate::material::material::Material, nuclides: &[crate::material::nuclide::Nuclide], settings: &KeffSettings) -> KeffResult { /* ... */ }
```

#### Function `run_keff_gpu_inner`

**Attributes:**

- `Other("#[attr = CfgTrace([Not(NameValue { name: \"target_os\", value: Some(\"android\"), span: crates/outram-mc-libs/src/physics/keff.rs:551:11: 551:32 (#0) }, crates/outram-mc-libs/src/physics/keff.rs:551:10: 551:33 (#0))])]")`

The genuine GPU path behind [`run_keff_gpu`] (desktop / non-Android only).

# How far the GPU reaches into the transport loop

The transport loop is **structurally identical** to [`run_keff_cpu_single`] —
same sequential single-`seed` threading, same RNG draw order — so `k_gpu`
stays tightly correlated with the single-thread reference. The **only**
difference is where the macroscopic total Sigma_t comes from:

1. **Build once.** A dense log-spaced table of the material's macroscopic
   Sigma_t is tabulated up front over `[1e-3, 2e7]` eV with 16 384 points
   ([`crate::gpu::union_grid::UnionTotalXs::tabulate`]). Temperature is fixed
   for the whole run, so one table serves every generation. 16 384 points is
   dense enough that the resampling error versus a direct
   [`Material::macro_xs_total`] call is small (it is judged against the
   reference below, not trusted above it).
2. **GPU batch per generation (the genuine GPU penetration).** At the start of
   every generation, the birth energies of **all** source sites are looked up
   in **one GPU dispatch** ([`crate::gpu::union_grid::UnionTotalXs::lookup_gpu`],
   `f32`). Each history then **consumes** its GPU-computed `f32` Sigma_t as the
   **first-flight** total cross section (its first collision-distance sample),
   instead of recomputing it.
3. **CPU table lookups thereafter.** Every subsequent per-collision Sigma_t
   within a history — and the first flight of any `(n,2n)` secondary that
   starts a fresh sub-walk — is served from the **same table** by CPU linear
   interpolation ([`crate::gpu::union_grid::UnionTotalXs::lookup_cpu`]).

A history-based random walk yields collision energies **one at a time**, so
dispatching a single-energy GPU kernel per collision would be dominated by
kernel-launch latency — that is the honest limit of GPU penetration into a
branchy history loop (see `src/gpu/mod.rs`: the "history-based transport loop
… branchy, not GPU friendly" note). The batched *first-flight* lookup is the
one place a whole generation's Sigma_t queries are available at once, so it is
the one place the GPU is actually exercised in the eigenvalue loop.

The GPU `f32` values are **acceleration only**; [`run_keff_cpu_single`] stays
the trusted reference. `k_gpu` differs from `k_single` only through (a) the
table's dense-resampling approximation of Sigma_t and (b) `f32` rounding of
the first-flight lookup — the RNG stream is otherwise identical.

```rust
pub fn run_keff_gpu_inner(ctx: &crate::gpu::GpuContext, radius_cm: f64, material: &crate::material::material::Material, nuclides: &[crate::material::nuclide::Nuclide], settings: &KeffSettings) -> KeffResult { /* ... */ }
```

#### Function `run_keff_gpu_batched`

**Attributes:**

- `Other("#[attr = CfgTrace([Not(NameValue { name: \"target_os\", value: Some(\"android\"), span: crates/outram-mc-libs/src/physics/keff.rs:711:11: 711:32 (#0) }, crates/outram-mc-libs/src/physics/keff.rs:711:10: 711:33 (#0))])]")`

**Event-based, batched-flight GPU power iteration** ([`ComputeType::Gpu`]) —
the deep GPU penetration of beads op-u6s.7. Desktop / non-Android only.

# How far the GPU reaches into the transport loop (the honest split)

Unlike the earlier first-flight-only [`run_keff_gpu_inner`], this driver keeps
a **whole batch of live neutrons resident in GPU buffers** and advances them
**one flight (one event) at a time, in parallel, per GPU dispatch**. For each
live particle, one [`crate::gpu::batched_flight::advance_flight_gpu`] dispatch
does, on the GPU (`f32`):

1. **RNG** — advance the particle's own 64-bit LCG one step (the state math is
   bit-exact vs the CPU LCG; the derived uniform is `f32`), giving the
   collision-distance random number.
2. **Sigma_t lookup** — binary-search the shared **native-breakpoint union
   grid** ([`crate::gpu::union_grid::UnionTotalXs::tabulate_native`]) and
   linearly interpolate the macroscopic total Sigma_t at the particle energy.
3. **Distance-to-collision** — `d_col = -ln(xi) / Sigma_t`.
4. **Distance-to-boundary** — the bounding sphere intersection.
5. **Stream + leak test** — move to the nearer of the two; flag `Leaked`
   (reached vacuum) or `Collided` (interacts inside).

So the **regular, memory-bound, per-event streaming work runs on the GPU for
the entire batch at once**. Only the **branchy per-collision reaction physics**
(which nuclide, fission vs capture vs inelastic vs `(n,2n)` vs elastic, the
secondary energy/angle laws) runs on the CPU — it is data-divergent and maps
poorly to a GPU. Each generation therefore issues a *sequence* of GPU
dispatches (one per event depth), each advancing all still-alive particles,
with a CPU collision + compaction pass between dispatches. This is the honest
limit of GPU penetration into a history-based MC walk: the flight is
data-parallel and lives on the GPU; the collision kernel is branchy and stays
on the CPU.

# RNG / reproducibility

Each history owns an **independent LCG sub-stream** derived only from
`(seed, generation, history index)` via jump-ahead — the same scheme as
[`run_keff_cpu_multi`]. The seed is threaded *through* the GPU flight (which
advances it bit-exactly) and continues on the CPU for the collision draws, so
a given particle sees one coherent stream across the GPU/CPU boundary. The
result is **reproducible run-to-run** and independent of GPU scheduling, but
— like the multi-thread backend, and because the flight's uniform + distance
are computed in `f32` — it does **not** bit-match [`run_keff_cpu_single`]. It
is a statistically independent estimate of the same eigenvalue, agreeing with
the trusted reference within combined statistical uncertainty. The CPU
single-thread `f64` path remains the trusted, bit-reproducible reference.

```rust
pub fn run_keff_gpu_batched(ctx: &crate::gpu::GpuContext, radius_cm: f64, material: &crate::material::material::Material, nuclides: &[crate::material::nuclide::Nuclide], settings: &KeffSettings) -> KeffResult { /* ... */ }
```

#### Function `run_keff_event_cpu_mirror`

**CPU-mirror event-based power iteration** — the non-GPU reference for the
fused collision-on-GPU path ([`run_keff_gpu_event`]).

Runs the identical event-based algorithm — a resident batch advanced one event
at a time, flight **and** collision resolved by
[`crate::gpu::batched_event::advance_generation_cpu_mirror`] (the same f32
arithmetic path as the WGSL kernel) — but entirely on the CPU. It builds on
**every** target (no `wgpu`), so it validates the fused event physics on
Android and on CPU-only CI where the GPU path cannot run. Like the multi-thread
and GPU backends it uses independent per-history LCG streams (f32 uniforms), so
it is a statistically independent estimate of the same eigenvalue — not
bit-locked to [`run_keff_cpu_single`], but agreeing within combined uncertainty.

```rust
pub fn run_keff_event_cpu_mirror(radius_cm: f64, material: &crate::material::material::Material, nuclides: &[crate::material::nuclide::Nuclide], settings: &KeffSettings) -> KeffResult { /* ... */ }
```

#### Function `run_keff_gpu_event`

**Attributes:**

- `Other("#[attr = CfgTrace([Not(NameValue { name: \"target_os\", value: Some(\"android\"), span: crates/outram-mc-libs/src/physics/keff.rs:1003:11: 1003:32 (#0) }, crates/outram-mc-libs/src/physics/keff.rs:1003:10: 1003:33 (#0))])]")`

**Event-based COLLISION-on-GPU power iteration** ([`ComputeType::Gpu`]) — the
op-u6s.8 deep-penetration path. Desktop / non-Android only.

# How far the GPU reaches (the honest split)
Unlike [`run_keff_gpu_batched`] (which kept the collision on the CPU and
round-tripped **per event**), this driver keeps a whole generation's batch
**resident in GPU buffers** and advances it through every event — flight **and**
the branchy collision physics — on the GPU
([`crate::gpu::batched_event::advance_generation_gpu`]). Per event, on the GPU
in `f32`: advance each neutron's LCG; look up Σ_t; sample the flight; on
collision sample the nuclide, partition the reaction (fission | capture |
inelastic | elastic), and apply the scatter kinematics. The only CPU traffic is
a 4-byte live-count read per event and one per-generation fission read-back; the
fission **daughters** are then banked on the CPU once per generation
([`bank_event_fission`]) from the handed-back seeds.

# Trust / reproducibility
The `f32` GPU results are **acceleration only**; [`run_keff_cpu_single`] stays
the trusted reference. Independent per-history LCG streams make this a
statistically independent estimate (not bit-locked to single-thread), agreeing
within combined uncertainty. The per-event GPU logic is held to the CPU mirror
([`run_keff_event_cpu_mirror`]) by the V&V gate in
[`crate::gpu::batched_event`].

```rust
pub fn run_keff_gpu_event(ctx: &crate::gpu::GpuContext, radius_cm: f64, material: &crate::material::material::Material, nuclides: &[crate::material::nuclide::Nuclide], settings: &KeffSettings) -> KeffResult { /* ... */ }
```

## Module `search`

Criticality search — a bracketed root-find of a scalar model parameter `p`
such that `k_eff(p) = target` (default `target = 1.0`).

This is the OUTRAM PARK analogue of OpenMC's `openmc.search_for_keff`
(`openmc/search.py`): given a model that turns one scalar knob `p` into a
k-eigenvalue calculation, find the value of `p` that makes the system exactly
critical (or hits any other target eigenvalue). Typical knobs a reactor
physicist sweeps this way:

- **Soluble-boron concentration** \[ppm\] in a PWR pin cell — the canonical
  OpenMC `search.ipynb` case (critical ≈ 1926 ppm; more boron ⇒ lower `k`).
- **Critical dimension** — the radius \[cm\] of a bare fissile sphere, the
  pitch of a lattice, a control-rod insertion depth (bigger ⇒ higher `k`).
- **Enrichment**, **moderator density**, **temperature** — any single scalar
  the k-eigenvalue depends on monotonically over the search bracket.

# What "criticality search" means physically

`k_eff` is the neutron multiplication factor: the ratio of neutrons produced
in one fission generation to those in the previous one. `k_eff = 1` is exact
criticality (a self-sustaining chain reaction, steady in time); `k > 1` is
supercritical, `k < 1` subcritical. A criticality search answers the inverse
design question — *what value of some physical parameter makes `k_eff` equal a
chosen target* — by root-finding the residual `g(p) = k_eff(p) − target`.

# Method (mirrors OpenMC's bisection semantics)

The driver is a **bracketed** root-find: the caller supplies an interval
`[lo, hi]` on which `g` changes sign, guaranteeing a root inside (intermediate
value theorem, `g` assumed continuous and monotone over the bracket). Two
bracket-preserving methods are offered — both keep the root enclosed at every
step, which matters because each `k_eff(p)` is a *noisy* Monte Carlo estimate:

- [`SearchMethod::Bisect`] — halve the bracket each step. The method OpenMC's
  `search.ipynb` uses (`bracketed_method='bisect'`). Linear convergence, one
  `k_eff` solve per iteration, and maximally robust to Monte Carlo noise
  because it only ever uses the *sign* of `g`, never its magnitude.
- [`SearchMethod::Secant`] — bracket-preserving false position (regula falsi):
  the next guess is the `g`-weighted secant intercept, but the sub-interval
  that still straddles the root is always retained. Usually fewer iterations
  than bisection, still guaranteed to converge. (This is the *bracketed*
  secant; unbracketed secant can diverge on a noisy `g`, so it is deliberately
  not offered.)

Both stop when the bracket width falls below [`SearchSettings::tol`] (a
tolerance in the *units of `p`*), or when `|g(mid)|` falls below
[`SearchSettings::k_tol`], or when [`SearchSettings::max_iterations`] is hit.

# No trait objects

The model is passed as a generic closure `F: FnMut(f64) -> KeffResult`, a
compile-time-monomorphised function parameter — **not** a `Box<dyn Fn>` — in
keeping with the workspace's no-trait-object rule. Nothing here stores the
closure in a struct, so there are no lifetime parameters either.

# Example

```no_run
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::{run_keff, KeffSettings};
use outram_mc_libs::physics::search::{search_for_keff, SearchSettings};

// Find the critical radius of a bare HEU (Godiva) sphere: k_eff(r) = 1.
let nuclides = vec![
    Nuclide::from_core("U234").unwrap(),
    Nuclide::from_core("U235").unwrap(),
    Nuclide::from_core("U238").unwrap(),
];
let mat = Material {
    id: 1,
    name: "HEU".into(),
    temperature: 293.6,
    components: vec![
        NuclideComponent { nuclide_idx: 0, atom_density: 4.9184e-4 },
        NuclideComponent { nuclide_idx: 1, atom_density: 4.4994e-2 },
        NuclideComponent { nuclide_idx: 2, atom_density: 2.4984e-3 },
    ],
};
let keff_settings = KeffSettings::default();
// k_eff increases with radius, so the bracket [7, 10] cm straddles k = 1.
let result = search_for_keff(
    |r| run_keff(r, &mat, &nuclides, &keff_settings),
    (7.0, 10.0),
    &SearchSettings::default(),
)
.unwrap();
println!("critical radius = {:.3} cm, k = {:.5}", result.parameter, result.keff);
```

```rust
pub mod search { /* ... */ }
```

### Types

#### Enum `SearchMethod`

Which bracketed root-finding method [`search_for_keff`] uses.

Both variants keep the root enclosed at every step (they never leave the
straddling interval), so both are robust to the Monte Carlo noise on each
`k_eff(p)` estimate. See the module docs for the trade-off.

```rust
pub enum SearchMethod {
    Bisect,
    Secant,
}
```

##### Variants

###### `Bisect`

Bisection — halve the bracket each step. Mirrors OpenMC
`bracketed_method='bisect'` (the `search.ipynb` default). Uses only the
*sign* of the residual, so it is the most noise-tolerant option.

###### `Secant`

Bracket-preserving false position (regula falsi) — the *bracketed*
secant. The next guess is the `g`-weighted linear intercept, but the
straddling sub-interval is always retained. Usually converges in fewer
`k_eff` solves than bisection.

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
    fn clone(self: &Self) -> SearchMethod { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &SearchMethod) -> bool { /* ... */ }
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
#### Struct `SearchSettings`

Settings controlling a [`search_for_keff`] criticality search.

The physical parameter `p` being searched has whatever units the caller's
model uses (ppm boron, cm radius, …); [`tol`](Self::tol) is therefore
expressed in *those same units*.

```rust
pub struct SearchSettings {
    pub target: f64,
    pub tol: f64,
    pub k_tol: f64,
    pub max_iterations: usize,
    pub method: SearchMethod,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `target` | `f64` | Target eigenvalue to solve for. `1.0` = exact criticality (the default). |
| `tol` | `f64` | Convergence tolerance on the **parameter** `p`, in the caller's units of<br>`p`. The search stops once the bracket width `|hi − lo|` falls below this.<br>Choose it comfortably larger than the parameter change that moves `k_eff`<br>by one standard error, so the search stops before Monte Carlo noise<br>dominates the residual sign. |
| `k_tol` | `f64` | Convergence tolerance on the **residual** `|k_eff(mid) − target|`. The<br>search also stops early if a midpoint lands this close to the target `k`.<br>Set it near the per-solve statistical error `k_std`; a value of `0.0`<br>disables this criterion (parameter tolerance / iteration cap only). |
| `max_iterations` | `usize` | Hard cap on the number of iterations (each iteration is one `k_eff`<br>solve). A guard against a non-converging search; a converged search<br>usually needs `ceil(log2(width / tol))` iterations for bisection. |
| `method` | `SearchMethod` | Which bracketed method to use ([`SearchMethod::Bisect`] by default,<br>mirroring the OpenMC `search.ipynb`). |

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
    fn clone(self: &Self) -> SearchSettings { /* ... */ }
    ```

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
    Exact-criticality search (`target = 1.0`) by bisection, tolerances tuned

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
#### Struct `SearchIteration`

One evaluated point of a criticality search: the parameter value tried and
the k-eigenvalue (with its statistical error) the model returned there.

The sequence of these (in [`SearchResult::iterations`]) is the OUTRAM PARK
analogue of the `(guesses, keffs)` trajectory OpenMC's `search_for_keff`
returns — useful for plotting the search path or diagnosing non-convergence.

```rust
pub struct SearchIteration {
    pub parameter: f64,
    pub keff: f64,
    pub keff_std: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `parameter` | `f64` | Parameter value `p` evaluated at this step (caller's units). |
| `keff` | `f64` | Mean k-eigenvalue `k_eff(p)` at this parameter. |
| `keff_std` | `f64` | Standard error (1σ) of that k-eigenvalue estimate. |

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
    fn clone(self: &Self) -> SearchIteration { /* ... */ }
    ```

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
#### Struct `SearchResult`

Outcome of a [`search_for_keff`] criticality search.

```rust
pub struct SearchResult {
    pub parameter: f64,
    pub keff: f64,
    pub keff_std: f64,
    pub converged: bool,
    pub iterations: Vec<SearchIteration>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `parameter` | `f64` | Converged parameter value `p*` with `k_eff(p*) ≈ target` (caller's units). |
| `keff` | `f64` | Mean k-eigenvalue at the converged parameter. |
| `keff_std` | `f64` | Standard error (1σ) of the k-eigenvalue at the converged parameter. |
| `converged` | `bool` | Whether a convergence criterion (parameter tolerance or residual<br>tolerance) was met before the iteration cap. `false` means the search<br>ran out of iterations; [`parameter`](Self::parameter) then holds the best<br>(final-bracket-midpoint) estimate so far. |
| `iterations` | `Vec<SearchIteration>` | Full search trajectory: the two bracket endpoints first, then every<br>midpoint evaluated, in order. Mirrors OpenMC's `(guesses, keffs)`. |

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
    fn clone(self: &Self) -> SearchResult { /* ... */ }
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
#### Enum `SearchError`

Why a [`search_for_keff`] call could not be started.

```rust
pub enum SearchError {
    BracketDoesNotStraddle {
        k_lo: f64,
        k_hi: f64,
        target: f64,
    },
    InvalidBracket {
        lo: f64,
        hi: f64,
    },
}
```

##### Variants

###### `BracketDoesNotStraddle`

The supplied bracket does not straddle the target: the residual
`g(p) = k_eff(p) − target` has the **same sign** at both endpoints, so no
root is guaranteed inside. Widen or move the bracket. Fields are the two
endpoint eigenvalues and the target, for diagnostics.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `k_lo` | `f64` | `k_eff` at the low endpoint `lo`. |
| `k_hi` | `f64` | `k_eff` at the high endpoint `hi`. |
| `target` | `f64` | The target eigenvalue searched for. |

###### `InvalidBracket`

The bracket was degenerate (`lo` and `hi` equal, or non-finite), so no
interval exists to search.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `lo` | `f64` | Low endpoint as supplied. |
| `hi` | `f64` | High endpoint as supplied. |

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
    fn clone(self: &Self) -> SearchError { /* ... */ }
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
    fn eq(self: &Self, other: &SearchError) -> bool { /* ... */ }
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
### Functions

#### Function `search_for_keff`

Bracketed criticality search: find the scalar parameter `p` in `bracket`
such that `k_eff(p) = settings.target`.

# Physical meaning

`k_of_p` is the caller's model: it takes one scalar knob `p` (boron ppm, a
critical radius \[cm\], enrichment, …) and returns a full [`KeffResult`] from
a k-eigenvalue transport solve at that `p`. This function root-finds the
residual `g(p) = k_eff(p) − target` to locate the parameter that hits the
target eigenvalue — exact criticality when `target = 1.0`.

# Arguments

- `k_of_p` — the model closure. Called once per endpoint and once per
  iteration. It is `FnMut` so it may carry mutable scratch state (e.g. a
  reusable geometry buffer); it is a monomorphised generic parameter, **not**
  a trait object.
- `bracket` — `(lo, hi)` in the units of `p`. `g` must change sign across it
  (i.e. one endpoint is subcritical relative to the target and the other
  supercritical); otherwise [`SearchError::BracketDoesNotStraddle`] is
  returned before any iteration. `lo` and `hi` may be given in either order.
- `settings` — target, tolerances, iteration cap, and method (see
  [`SearchSettings`]).

# Returns

A [`SearchResult`] with the converged parameter, the eigenvalue (± 1σ) there,
a `converged` flag, and the full `(parameter, k)` trajectory. Returns
[`SearchError`] only for a bad bracket — a search that merely exhausts its
iteration budget still returns `Ok` with `converged = false` and the best
estimate so far.

# Monotonicity assumption

Like OpenMC's `search_for_keff`, this assumes `k_eff` varies monotonically
with `p` across the bracket (true for the usual knobs over a sensible range).
The bracketed methods stay convergent even when each `k_eff` is a noisy Monte
Carlo estimate, because they retain the straddling sub-interval every step.

```rust
pub fn search_for_keff<F>(k_of_p: F, bracket: (f64, f64), settings: &SearchSettings) -> Result<SearchResult, SearchError>
where
    F: FnMut(f64) -> crate::physics::keff::KeffResult { /* ... */ }
```

## Module `physics_mg`

Multigroup (MG) neutron transport physics and a MG k-eigenvalue driver.

C++ source (structure): `src/physics_mg.cpp`, `src/mgxs.cpp`,
`include/openmc/mgxs.h`; the Python data model this mirrors is
`openmc.XSdata` / `openmc.Macroscopic` / `openmc.MGXSLibrary`.

# What "multigroup" means here

Continuous-energy (CE) transport ([`crate::physics::keff`],
[`crate::physics::transport_csg`]) tracks a neutron's energy `E` as a real
number and looks up σ(E) point data. **Multigroup** transport instead
collapses the energy axis into `G` contiguous **energy groups** (group `0`
is the fastest, group `G-1` the slowest by the usual reactor-physics
convention). A neutron then carries a *group index* `g ∈ 0..G`, and every
cross section is a group-averaged constant Σ_x,g \[cm⁻¹\] rather than a
tabulated function of energy. Group-to-group transfer is described by a
**scattering matrix** Σ_s,g→g'.

This crate is **data-free with respect to MGXS**: *generating* group
constants (flux-weighted collapse of CE data) is `njoy-outram-park-fork`'s
job. This module only *consumes* a supplied MGXS set and runs transport on
it, exactly as OpenMC's MG mode consumes an `mgxs.h5` library produced by the
`openmc.mgxs` tally-to-MGXS machinery.

# The data types

- [`Mgxs`] — the group constants for **one material** (the `XSdata` /
  `Macroscopic` analogue): per-group total, absorption, fission, ν·fission
  and fission spectrum χ, plus the full scattering matrix Σ_s,g→g'.
- [`MgxsLibrary`] — an ordered collection of [`Mgxs`], **indexed by material
  index** so it lines up 1:1 with the material indices a [`Geometry`]'s cells
  already carry (the `MGXSLibrary` analogue).

# The transport kernel

[`run_keff_mg`] is the MG twin of [`run_keff_csg`](crate::physics::transport_csg::run_keff_csg):
the same surface-tracking fission-source power iteration over an arbitrary CSG
[`Geometry`], but with group-indexed collision physics instead of CE lookups.
At each collision in group `g` the reaction is partitioned on the group total
Σ_t,g into fission | capture | scatter; a scatter samples the outgoing group
`g'` from row `g` of the scattering matrix (direction resampled isotropically,
the P0 assumption); a fission banks `n ≈ ν̄_g/k` next-generation sites whose
birth group is drawn from χ.

# Fidelity

Analog transport (weight 1, no variance reduction). Scattering is treated as
isotropic in the lab frame (a P0 / transport-corrected set); anisotropic
(P_N) scattering matrices are not modelled. There is no delayed-neutron
separation (delayed folded into ν̄). This matches the simplest OpenMC MG mode
(`isotropic` angular representation).

# Example

```
use outram_mc_libs::physics::physics_mg::{Mgxs, MgxsLibrary};

// A 1-group bare set: Σ_t = Σ_a + Σ_s, one self-scatter entry.
let one_group = Mgxs::new(
    "fuel",
    /* total       */ vec![0.30],
    /* absorption  */ vec![0.10],
    /* fission     */ vec![0.05],
    /* nu_fission  */ vec![0.13],
    /* chi         */ vec![1.0],
    /* scatter g→g'*/ vec![0.20], // 1×1 matrix, row-major
);
assert_eq!(one_group.n_groups, 1);
let lib = MgxsLibrary::new(vec![one_group]);
assert_eq!(lib.n_groups(), 1);
```

```rust
pub mod physics_mg { /* ... */ }
```

### Types

#### Struct `Mgxs`

Multigroup macroscopic cross sections for **one material**, over `G` groups.

This is the OUTRAM PARK analogue of OpenMC's `XSdata` / `Macroscopic`: a bag
of group-averaged **macroscopic** cross sections \[cm⁻¹\] the MG transport
kernel consumes. Group `0` is fastest, group `G-1` slowest.

# Invariants (checked by [`Mgxs::new`])
- every per-group vector has length `n_groups`;
- the scattering matrix is `n_groups × n_groups`, stored **row-major** as
  `scatter[g * G + g']` = Σ_s,g→g' (out of `g`, into `g'`);
- `fission[g] ≤ absorption[g] ≤ total[g]` (fission is part of absorption);
- χ sums to 1 (renormalised on construction if it does not, and it is
  non-zero).

The transport kernel treats `total[g] − absorption[g]` as the scatter-out
cross section and samples the outgoing group from row `g`; see
[`Mgxs::scatter_out`] and [`Mgxs::consistency_residual`] for the balance a
well-formed set satisfies.

```rust
pub struct Mgxs {
    pub name: String,
    pub n_groups: usize,
    pub total: Vec<f64>,
    pub absorption: Vec<f64>,
    pub fission: Vec<f64>,
    pub nu_fission: Vec<f64>,
    pub chi: Vec<f64>,
    pub scatter: Vec<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Human-facing name (for reporting), e.g. the source material's name. |
| `n_groups` | `usize` | Number of energy groups `G`. |
| `total` | `Vec<f64>` | Total Σ_t,g \[cm⁻¹\], length `G` — governs distance-to-collision. |
| `absorption` | `Vec<f64>` | Absorption Σ_a,g \[cm⁻¹\], length `G` — capture **plus** fission. |
| `fission` | `Vec<f64>` | Fission Σ_f,g \[cm⁻¹\], length `G`. |
| `nu_fission` | `Vec<f64>` | Fission production ν̄·Σ_f,g \[cm⁻¹\], length `G` — the k source term. |
| `chi` | `Vec<f64>` | Fission spectrum χ_g (probability a fission neutron is born in group `g`),<br>length `G`, sums to 1. |
| `scatter` | `Vec<f64>` | Scattering matrix Σ_s,g→g' \[cm⁻¹\], length `G·G`, **row-major**:<br>`scatter[g * G + g']` transfers from group `g` into group `g'`. |

##### Implementations

###### Methods

- ```rust
  pub fn new</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, total: Vec<f64>, absorption: Vec<f64>, fission: Vec<f64>, nu_fission: Vec<f64>, chi: Vec<f64>, scatter: Vec<f64>) -> Self { /* ... */ }
  ```
  Assemble a validated [`Mgxs`] from its per-group components.

- ```rust
  pub fn scatter_out(self: &Self, g: usize) -> f64 { /* ... */ }
  ```
  Scatter-out cross section of group `g` from the matrix: Σ_row = Σ_g' Σ_s,g→g'.

- ```rust
  pub fn consistency_residual(self: &Self) -> f64 { /* ... */ }
  ```
  The maximum absolute self-consistency residual

- ```rust
  pub fn nu_bar(self: &Self, g: usize) -> f64 { /* ... */ }
  ```
  Mean neutrons per fission ν̄_g = ν̄·Σ_f,g / Σ_f,g in group `g` (0 if no

- ```rust
  pub fn is_fissile(self: &Self) -> bool { /* ... */ }
  ```
  Whether this material can fission at all (any group has ν·Σ_f > 0).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Mgxs { /* ... */ }
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
#### Struct `MgxsLibrary`

An ordered multigroup cross-section library — one [`Mgxs`] per material,
**indexed by material index**. The OpenMC `MGXSLibrary` analogue.

The `i`-th entry supplies the group constants for the material a
[`Geometry`] cell fills with material index `i`, so a [`Geometry`] built for
CE transport can be reused unchanged under [`run_keff_mg`] as long as this
library has an entry for every material index the geometry references.

All entries must share the same group count `G` (asserted on construction).

```rust
pub struct MgxsLibrary {
    pub materials: Vec<Mgxs>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `materials` | `Vec<Mgxs>` | Per-material group constants, indexed by material index. |

##### Implementations

###### Methods

- ```rust
  pub fn new(materials: Vec<Mgxs>) -> Self { /* ... */ }
  ```
  Build a library from per-material [`Mgxs`] sets (order = material index).

- ```rust
  pub fn n_groups(self: &Self) -> usize { /* ... */ }
  ```
  Number of energy groups `G` (shared across every material).

- ```rust
  pub fn material(self: &Self, i: usize) -> &Mgxs { /* ... */ }
  ```
  The group constants for material index `i`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> MgxsLibrary { /* ... */ }
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
#### Struct `MgSettings`

Settings for a [`run_keff_mg`] power iteration. The MG twin of the CE
[`KeffSettings`](crate::physics::keff::KeffSettings), without the
energy/temperature fields (MGXS is already collapsed and temperature-fixed).

```rust
pub struct MgSettings {
    pub n_particles: usize,
    pub n_inactive: usize,
    pub n_active: usize,
    pub seed: u64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `n_particles` | `usize` | Neutron histories per generation. |
| `n_inactive` | `usize` | Inactive (source-convergence) generations, discarded from the k tally. |
| `n_active` | `usize` | Active generations averaged into the reported eigenvalue. |
| `seed` | `u64` | Master RNG seed. Fixed seed ⇒ bit-reproducible run. |

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
    fn clone(self: &Self) -> MgSettings { /* ... */ }
    ```

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
### Functions

#### Function `run_keff_mg`

Run multigroup fission-source power iteration over a CSG [`Geometry`].

The MG twin of [`run_keff_csg`](crate::physics::transport_csg::run_keff_csg):
identical surface-tracking transport and power-iteration bookkeeping, but the
particle carries a group index and every cross section comes from `lib`
(indexed by the geometry's material indices) instead of CE nuclide data.

# Parameters
- `geom` — the CSG model (surfaces/cells/universes/lattices + root universe).
- `lib` — MGXS library, one [`Mgxs`] per material index the geometry uses.
- `source_box` — box the initial source is rejection-sampled into (must
  overlap a fissile region).
- `settings` — power-iteration controls (see [`MgSettings`]).

Returns the mean eigenvalue and standard error over the active generations (a
[`KeffResult`], the same type as the CE drivers).

```rust
pub fn run_keff_mg(geom: &crate::geometry::geometry::Geometry, lib: &MgxsLibrary, source_box: crate::physics::transport_csg::SourceBox, settings: &MgSettings) -> crate::physics::keff::KeffResult { /* ... */ }
```

## Module `pebble_beds`

Pebble-bed reactor specialization — the doubly-heterogeneous transport slice.

This module is where `outram-mc-libs` deliberately specializes. The fork's target
application is the **pebble-bed reactor**, whose geometry is *doubly
heterogeneous*: thousands of sub-millimetre TRISO fuel particles are packed in a
graphite matrix to form a pebble, and tens of thousands of pebbles are packed
into the core. A pebble holds O(10⁴) TRISO kernels; a core holds O(10⁵) pebbles —
so a naive surface-tracking transport sweep would spend all its time computing
distances to an astronomical number of spherical shells.

Two families of technique make this tractable, and both live here:

- **[`delta_tracking`]** — Woodcock (delta) tracking. Instead of finding the
  next material boundary, sample a flight on a *majorant* cross section that
  bounds every material, then accept the landing as a real collision with
  probability Σ_t(local)/Σ_maj (else it is a virtual collision and the flight
  continues). The neutron never needs to know how many TRISO surfaces it flew
  past — the dominant win for doubly-heterogeneous media.
- **[`sphere_packing`]** — generating and sampling the random packed geometry
  itself. Random Sequential Addition (RSA) is implemented
  ([`sphere_packing::pack_spheres`]); the RSA–DEM/ODR–DEM high-density hybrids
  are future work. See the [`references`] bibliography.
- **[`keff_delta`]** — the assembly: a fission-source k-eigenvalue power
  iteration over a reflective cube of packed kernels, with every history
  streamed by delta tracking ([`keff_delta::run_keff_delta`]). This is the
  doubly-heterogeneous k-eff the other two modules exist to make tractable.

# See also: the stochastic-media research track

Chord Length Sampling (CLS), Semi-Implicit CLS (SCLS) and the Dynamic Inclusion
Sphere live in [`crate::stochastic`], **not** here. Those methods apply to any
binary random medium — dispersion fuel, burnable-poison particles — not just pebble
beds, so filing them under this reactor-specific module would misplace them. The
link back is [`crate::stochastic::medium::RsaMedium`], which wraps the
[`sphere_packing`] packing generated here as its exact reference model.

# Why a dedicated module

The rest of the crate is a faithful, application-neutral port of OpenMC. The
physics and geometry primitives it provides (surfaces, cells, universes,
lattices, the transport loop) are reused unchanged. What is *specialized* — the
majorant construction, the delta-tracking flight, and the stochastic-media
geometry generators optimized for high TRISO packing fractions — is quarantined
here so the specialization intent is explicit and the general port stays general.

# References

The stochastic-media geometry work draws on Zhe Chuan Tan et al.'s dispersion-fuel
papers in the RMC code; the machine-readable citations are in [`references`].

```rust
pub mod pebble_beds { /* ... */ }
```

### Modules

## Module `delta_tracking`

Woodcock (delta) tracking for doubly-heterogeneous media.

In a pebble-bed core a neutron flies past an enormous number of material
interfaces (TRISO kernel / buffer / IPyC / SiC / OPyC shells, matrix, pebble
surfaces). Surface tracking must find the *nearest* of all those boundaries at
every flight — ruinously expensive when there are O(10⁵) of them along a path.

Delta tracking removes boundary crossings from the inner loop. Pick a
**majorant** cross section `Σ_maj(E) ≥ Σ_t(E)` for *every* material the neutron
could be in. Sample the flight distance on the majorant,
`s = −ln ξ / Σ_maj(E)`. At the landing point read the *local* material's true
`Σ_t`; accept a **real** collision with probability `Σ_t/Σ_maj`, otherwise the
event is a **virtual** (delta) collision — nothing physical happens and the
flight simply continues from the new point. The neutron never counts the
surfaces it flew over; it only ever queries "what material am I in *here*".

The rejected (virtual) fraction is the price: a loose majorant means many
virtual collisions. For pebble beds the majorant is dominated by the strongly
absorbing/scattering fuel, so a material-wise majorant is usually tight enough.

This module provides the tracking *primitives* — majorant construction, the
flight-distance sample, and the real/virtual decision — plus a
[`track_to_collision`] driver that composes them against a caller-supplied
"material at this point" lookup. The lookup is where the doubly-heterogeneous
geometry (lattices of TRISO universes, stochastic packings) plugs in.

Reference: E. R. Woodcock et al., "Techniques used in the GEM code…", ANL-7050
(1965); the method is standard in modern MC codes (OpenMC `delta_tracking`,
Serpent, RMC). See also [`super::references`] for the pebble-bed geometry work.

```rust
pub mod delta_tracking { /* ... */ }
```

### Types

#### Struct `Majorant`

An energy-dependent majorant cross section `Σ_maj(E) ≥ Σ_t(E)` over a set of
materials — the sampling bound for delta tracking.

Stored as a tabulated `(energy [eV], Σ_maj [cm⁻¹])` curve on an ascending grid.
[`Self::at`] returns a value that is conservative (never below the true
tabulated majorant) by taking the larger of the two bracketing grid points, so
a coarse grid stays valid — it just loosens the bound (more virtual collisions).

```rust
pub struct Majorant {
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
  pub fn uniform(sigma_max: f64) -> Self { /* ... */ }
  ```
  A single flat majorant `sigma_max` \[cm⁻¹\] valid at all energies.

- ```rust
  pub fn from_materials(materials: &[Material], nuclides: &[Nuclide], energies: &[f64], margin: f64) -> Self { /* ... */ }
  ```
  Build the majorant `Σ_maj(E) = max_m Σ_t,m(E)` over `materials` on the

- ```rust
  pub fn bounding(materials: &[Material], nuclides: &[Nuclide], e_min: f64, e_max: f64, n_bins: usize, subsamples: usize, margin: f64) -> Self { /* ... */ }
  ```
  Build a **provably bounding** majorant by taking, for each energy bin, the

- ```rust
  pub fn at(self: &Self, e: f64) -> f64 { /* ... */ }
  ```
  The majorant Σ_maj \[cm⁻¹\] at energy `e` \[eV\] — conservative (takes the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Majorant { /* ... */ }
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
    fn default() -> Majorant { /* ... */ }
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
#### Enum `DeltaEvent`

The outcome of one delta-tracking flight segment.

```rust
pub enum DeltaEvent {
    Real,
    Virtual,
}
```

##### Variants

###### `Real`

A physical interaction — sample the actual reaction here.

###### `Virtual`

A virtual (delta) collision — no physics; continue the flight.

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
    fn clone(self: &Self) -> DeltaEvent { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &DeltaEvent) -> bool { /* ... */ }
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
#### Struct `DeltaFlight`

Where a delta-tracking flight ended.

```rust
pub struct DeltaFlight {
    pub position: crate::geometry::position::Position,
    pub distance: f64,
    pub virtual_collisions: u32,
    pub escaped: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `position` | `crate::geometry::position::Position` | Position \[cm\] of the real collision (or of leakage — see `escaped`). |
| `distance` | `f64` | Total path length \[cm\] flown, including all virtual-collision segments. |
| `virtual_collisions` | `u32` | Number of virtual (delta) collisions rejected before the real one. |
| `escaped` | `bool` | `true` if the neutron left the tracking region before a real collision<br>(the `sigma_t_local` lookup returned `None`). |

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
    fn clone(self: &Self) -> DeltaFlight { /* ... */ }
    ```

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
### Functions

#### Function `sample_delta_distance`

Sample a delta-tracking flight distance \[cm\] from an exponential on the
majorant: `s = −ln ξ / Σ_maj`.

This is the ordinary free-flight sample, but on the majorant rather than the
local Σ_t, which is what lets the neutron cross material boundaries without
stopping at them. `majorant` is Σ_maj \[cm⁻¹\] at the current energy; a
non-positive majorant yields an infinite flight (no collisions possible).

```rust
pub fn sample_delta_distance(majorant: f64, seed: &mut u64) -> f64 { /* ... */ }
```

#### Function `classify_collision`

Decide whether a delta-tracking collision is real or virtual by rejection on
the ratio `Σ_t(local)/Σ_maj`.

Accepts a [`DeltaEvent::Real`] with probability `sigma_t_local / majorant`
(the physical collision), else [`DeltaEvent::Virtual`]. `sigma_t_local` is the
true macroscopic total \[cm⁻¹\] of the material at the landing point;
`majorant` is the Σ_maj the flight was sampled on. A `majorant ≤ 0` degenerates
to `Virtual`. The caller must guarantee `sigma_t_local ≤ majorant` (that is the
majorant's whole contract); if it is violated the ratio saturates at 1 (always
real), which is the safe direction.

```rust
pub fn classify_collision(sigma_t_local: f64, majorant: f64, seed: &mut u64) -> DeltaEvent { /* ... */ }
```

#### Function `track_to_collision`

Drive a neutron from `start` along `direction` to its next **real** collision by
delta tracking, looping over virtual collisions internally.

At each step it samples a flight on `majorant.at(energy)`, advances, then asks
the caller "what is Σ_t at this point?" via `sigma_t_at`. That closure returns
`Some(sigma_t)` for a point inside the tracking region (looking up whichever
material — fuel kernel, matrix, pebble, coolant — actually occupies the point)
or `None` if the neutron has left the region (leakage). A real collision ends
the loop; a virtual one continues it.

This is deliberately generic over the geometry lookup (`impl Fn`, no trait
object) so the doubly-heterogeneous machinery — lattice/universe descent or a
stochastic-media membership test — supplies `sigma_t_at` without this core
depending on it.

# Parameters
- `start` / `direction` — the neutron's phase-space point.
- `energy` — incident energy \[eV\] (constant along the flight; scattering
  changes it *after* a real collision, in the caller's transport loop).
- `majorant` — the delta-tracking bound (see [`Majorant`]).
- `max_virtual` — safety cap on virtual collisions before giving up (returns
  `escaped = true`); guards against a pathologically loose majorant.
- `sigma_t_at` — local total Σ_t \[cm⁻¹\] lookup, `None` outside the region.

```rust
pub fn track_to_collision<F>(start: crate::geometry::position::Position, direction: crate::geometry::position::Direction, energy: f64, majorant: &Majorant, max_virtual: u32, seed: &mut u64, sigma_t_at: F) -> DeltaFlight
where
    F: Fn(crate::geometry::position::Position) -> Option<f64> { /* ... */ }
```

## Module `keff_delta`

Doubly-heterogeneous k-eigenvalue power iteration driven by **delta (Woodcock)
tracking**.

This is the assembly point for the random-packed TRISO k-eff: it composes the
[`super::sphere_packing`] packed geometry, the [`super::delta_tracking`]
flight primitives, and the crate's collision physics into a fission-source
power iteration — the doubly-heterogeneous analogue of
[`crate::physics::keff::run_keff`] (bare sphere) and
[`crate::physics::transport_csg::run_keff_csg`] (surface-tracked CSG).

# Why delta tracking here

In a packed TRISO medium a neutron's straight-line path crosses an enormous
number of kernel surfaces. Surface tracking must find the *nearest* of them at
every flight; delta tracking never looks for a surface at all. It samples the
flight on a **majorant** `Σ_maj(E) ≥ Σ_t(E)` bounding every material, lands at a
point, and asks only "**what material is here?**" — a point-membership test the
packed-sphere grid answers in O(1) ([`super::sphere_packing::PackedSpheres::is_inside_kernel`]).
The landing is a real collision with probability `Σ_t(local)/Σ_maj` and a
virtual (do-nothing) collision otherwise. See [`super::delta_tracking`] for the
primitives and their unit tests (unbiased mean free path, correct real/virtual
split).

# Geometry model

A **reflective cube** of half-width `half_width` (an infinite-medium unit cell:
neutrons reflect off the six walls, so the eigenvalue is the infinite-medium
`k∞` of the packed fuel, free of leakage). Inside the cube the caller's
`material_at` closure maps a point to a material index (kernel → fuel, else
matrix). The delta flight reflects the ray off the walls segment by segment, so
the neutron always lands at an interior point where `material_at` is defined.

# Collision physics

At each real collision the analog reaction partition — fission | capture |
inelastic | (n,2n) | elastic — mirrors [`crate::physics::keff`] /
[`crate::physics::transport_csg`] (the same [`crate::physics::scatter`] and
[`crate::physics::fission`] kernels). Only fission neutrons are banked to the
next generation; `(n,2n)` multiplicity is realized in-generation via a local
work stack. Fidelity matches those drivers: analog, target at rest, data tier
set by how the `nuclides` were built ([`Nuclide::from_core`] LOW /
[`Nuclide::from_endf`] HIGH).

# Provenance

The delta-tracking method is standard (Woodcock, ANL-7050, 1965; used in OpenMC,
Serpent, RMC). The collision partition mirrors OpenMC `src/physics.cpp`
(`collision` / `inelastic_scatter`). The reflective-cube flight is new pebble-bed
assembly built on this crate's primitives.

```rust
pub mod keff_delta { /* ... */ }
```

### Functions

#### Function `run_keff_delta`

Run fission-source power iteration over a **reflective cube** filled with a
two-(or-more-)material dispersion medium, transporting each history by delta
(Woodcock) tracking.

# Parameters
- `half_width` — half-width \[cm\] of the reflective cube (infinite-medium cell).
- `materials` — global material array; `material_at` returns indices into it.
- `nuclides` — global nuclide array the materials index into.
- `majorant` — a [`Majorant`] bounding `Σ_t(E)` of **every** material over the
  full energy range the histories span (build it with
  [`Majorant::from_materials`] on a broad grid).
- `material_at` — geometry lookup: the material index at a point inside the cube
  (e.g. kernel → fuel, matrix → moderator). Must be defined everywhere inside
  the closed cube; returning `None` leaks the history.
- `settings` — power-iteration controls (reuses [`KeffSettings`]).

Returns the mean eigenvalue and its standard error over the active generations.
The initial source is rejection-sampled uniformly in the cube for points in a
fissile material.

# Compute backend

This is a thin **dispatcher** over [`settings.compute`](KeffSettings::compute),
mirroring [`crate::physics::keff::run_keff`] and
[`crate::physics::transport_csg::run_keff_csg`]. The physics is identical
across backends; only the execution strategy differs:

- [`ComputeType::CpuSingleThread`] → [`run_keff_delta_seq`], the scalar,
  single-RNG-stream **reference** — deterministic and bit-reproducible for a
  fixed seed.
- [`ComputeType::CpuMultiThread`] → [`run_keff_delta_par`], [`rayon`]-parallel
  histories per generation, each with an independent jump-ahead RNG stream so
  the result is reproducible independent of thread count. It does **not**
  bit-match the single-thread reference but agrees within combined statistical
  uncertainty. (The `material_at` closure must be [`Sync`] to be shared across
  threads — every geometry lookup in this crate already is.)
- [`ComputeType::Gpu`] → **no GPU kernel exists for delta-tracked
  doubly-heterogeneous geometry**, so this transparently runs the
  multi-threaded CPU path and emits a `log::debug!` line. It never errors on
  the selection. Wiring a genuine GPU path into CSG/delta transport is tracked
  as follow-up work (bead op-fla).

```rust
pub fn run_keff_delta<F>(half_width: f64, materials: &[crate::material::material::Material], nuclides: &[crate::material::nuclide::Nuclide], majorant: &crate::pebble_beds::delta_tracking::Majorant, material_at: F, settings: &crate::physics::keff::KeffSettings) -> crate::physics::keff::KeffResult
where
    F: Fn(crate::geometry::position::Position) -> Option<usize> + Sync { /* ... */ }
```

#### Function `run_keff_delta_seq`

Scalar, single-thread delta-tracked power iteration — the **trusted,
deterministic, bit-reproducible reference** backend
([`ComputeType::CpuSingleThread`]).

One `f64` RNG stream is threaded sequentially through the whole run (initial
source rejection-sampling, every history's delta flight, every resample), so a
fixed [`KeffSettings::seed`] yields the same eigenvalue bit-for-bit on every
machine. [`run_keff_delta_par`] is acceleration only and is validated against
this reference.

```rust
pub fn run_keff_delta_seq<F>(half_width: f64, materials: &[crate::material::material::Material], nuclides: &[crate::material::nuclide::Nuclide], majorant: &crate::pebble_beds::delta_tracking::Majorant, material_at: F, settings: &crate::physics::keff::KeffSettings) -> crate::physics::keff::KeffResult
where
    F: Fn(crate::geometry::position::Position) -> Option<usize> { /* ... */ }
```

#### Function `run_keff_delta_par`

Rayon-parallel delta-tracked power iteration ([`ComputeType::CpuMultiThread`]).

Same physics and power-iteration structure as [`run_keff_delta_seq`], but the
histories **within each generation** are delta-tracked in parallel with
[`rayon`] in a dedicated pool sized to `thread_count` (never the implicit
global pool). The generation loop stays sequential — generation `g+1`'s source
is `g`'s resampled fission bank, a hard data dependency.

# Reproducibility (independent of thread count)

Each history is given a **completely independent, deterministic** RNG stream
derived only from `(settings.seed, generation, history index)` via the LCG
jump-ahead ([`crate::rng::lcg::future_seed`]) — never a shared mutable seed —
so the result never races and is identical regardless of how rayon schedules
the work. This mirrors [`crate::physics::keff::run_keff_cpu_multi`]; see its
docs for the `HIST_STRIDE` / `GEN_STRIDE` non-overlap argument. The initial
source sampling and each resample run on a separate sequential `src_seed`
stream, kept off the parallel path. Because the per-history stream structure
differs from the single sequential stream, this backend does **not** bit-match
[`run_keff_delta_seq`] — it is a statistically independent estimate of the same
eigenvalue, agreeing within combined uncertainty.

The `material_at` geometry lookup is shared across threads by reference, so it
must be [`Sync`] (every packed-sphere / membership lookup in this crate is).

```rust
pub fn run_keff_delta_par<F>(half_width: f64, materials: &[crate::material::material::Material], nuclides: &[crate::material::nuclide::Nuclide], majorant: &crate::pebble_beds::delta_tracking::Majorant, material_at: F, settings: &crate::physics::keff::KeffSettings, thread_count: crate::physics::compute::ThreadCount) -> crate::physics::keff::KeffResult
where
    F: Fn(crate::geometry::position::Position) -> Option<usize> + Sync { /* ... */ }
```

## Module `references`

Bibliography for the pebble-bed / dispersion-fuel geometry methods.

These are the stochastic-media and packing-generation papers the
[`super::sphere_packing`] and [`super::delta_tracking`] work builds on —
Zhe Chuan Tan et al.'s dispersion-fuel series in the RMC Monte Carlo code.
Each is a machine-readable [`Reference`] so doc comments elsewhere can point at
a concrete, rust-analyzer-navigable citation rather than a bare DOI string.

Cite them from a doc comment like:
```
use outram_mc_libs::pebble_beds::references::TAN2024_RSA;
assert_eq!(TAN2024_RSA.year, 2024);
```

```rust
pub mod references { /* ... */ }
```

### Types

#### Struct `Reference`

One journal-article citation, in the fields a reader (or a `.bib` exporter)
needs. All strings are `'static` — these are compile-time constants.

```rust
pub struct Reference {
    pub authors: &'static str,
    pub title: &'static str,
    pub journal: &'static str,
    pub year: u16,
    pub doi: &'static str,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `authors` | `&'static str` | Author list, "Last, First and Last, First …" (BibTeX `author` order). |
| `title` | `&'static str` | Article title. |
| `journal` | `&'static str` | Journal name. |
| `year` | `u16` | Publication year. |
| `doi` | `&'static str` | Digital Object Identifier (without the `https://doi.org/` prefix), or `""`<br>when the publisher has not assigned/exposed one. |

##### Implementations

###### Methods

- ```rust
  pub fn to_bibtex(self: &Self) -> String { /* ... */ }
  ```
  Render this reference as a BibTeX `@article` entry (for exporting a `.bib`).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Reference { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Reference) -> bool { /* ... */ }
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
### Constants and Statics

#### Constant `TAN2024_RSA`

Tan, Feng, Wang (2024) — improved parallel Random Sequential Addition (RSA) for
generating dispersion-fuel (TRISO) packings in RMC. The packing generator whose
output the doubly-heterogeneous transport in this module consumes.

```rust
pub const TAN2024_RSA: Reference = _;
```

#### Constant `TAN2025_CLS`

Tan, Feng, Chan, Wang (2025) — a semi-implicit Chord Length Sampling (CLS)
method for dispersion fuel. CLS is the on-the-fly stochastic-geometry sampling
alternative to an explicit packing; relevant to [`super::delta_tracking`].

```rust
pub const TAN2025_CLS: Reference = _;
```

#### Constant `TAN2026_RSA_DEM`

Tan, Feng, Wang (2026) — an iterative RSA–DEM (Discrete Element Method) method
for reaching *high* particle packing fractions in stochastic media, beyond what
plain RSA saturates at.

```rust
pub const TAN2026_RSA_DEM: Reference = _;
```

#### Constant `TAN2026_ODR_DEM`

Tan, Feng, Wang (2026) — coupled ODR–DEM (Ordered/Overlap-Driven Relaxation with
DEM) packing methods for dispersion-fuel analysis in RMC.

```rust
pub const TAN2026_ODR_DEM: Reference = _;
```

#### Constant `ALL`

Every reference in this bibliography, for iterating or exporting a full `.bib`.

```rust
pub const ALL: [Reference; 4] = _;
```

## Module `sphere_packing`

Random sphere packing — generating and querying explicit dispersion-fuel geometry.

This is where the packed-particle geometry for a doubly-heterogeneous
TRISO/pebble layout is *generated* and *queried*. A pebble holds O(10⁴) TRISO
kernels scattered at random through a graphite matrix; this module produces
that random, non-overlapping set of kernel centres and answers the one question
the [`super::delta_tracking`] flight asks over and over: **"is this point inside
a fuel kernel or in the matrix?"**

Despite living under [`super`], the packer itself is **application-neutral**: it
places equal-radius spheres in a cube. TRISO kernels are the motivating case and
supply the naming of [`PackedSpheres::is_inside_kernel`], but the same generator
serves any dispersion medium — B4C burnable-poison particles, for instance.

# Relationship to [`crate::stochastic`]

This module is the **explicit** end of the memory/fidelity spectrum: every inclusion
is stored, so point membership is exact and memory is O(N). The approximate models
that sample geometry instead of storing it — Chord Length Sampling and Semi-Implicit
CLS — live in [`crate::stochastic`], and treat what this module produces as their
reference solution via [`crate::stochastic::medium::RsaMedium`].

(Named `sphere_packing` rather than `stochastic_media` so it is not confused with
that sibling module — the two would otherwise read as the same thing.)

# What is implemented

- [`pack_spheres`] — **Random Sequential Addition (RSA)**: drop kernel centres
  one at a time at uniform-random positions, rejecting any that would overlap an
  already-placed kernel, until the target packing fraction is reached. This is a
  direct port of OpenMC's `_random_sequential_pack` (the RSA half of
  `openmc.model.pack_spheres`, `openmc/model/triso.py:882` and `:1210`), using
  the same overlap-acceleration mesh so the nearest-neighbour test stays O(1)
  per trial rather than O(N).
- [`PackedSpheres`] — owns the packed kernel list plus a uniform spatial hash
  grid, and offers a fast [`PackedSpheres::is_inside_kernel`] point-membership
  test for the transport lookup.

RSA saturates near a packing fraction of ~0.38 (the [`MAX_PF_RSA`] limit); above
that the trial-rejection loop becomes prohibitively slow. The higher-density
close-random-packing methods (Jodrey–Tory contraction, RSA–DEM / ODR–DEM
relaxation — [`PackingMethod::RsaDem`] / [`PackingMethod::OdrDem`], see the
[`super::references`] bibliography) are **not** implemented here; they remain a
follow-up for reaching pebble-bed-realistic (~0.6) packing fractions.

# Provenance

The RSA algorithm and its acceleration mesh mirror OpenMC (MIT):
`openmc/model/triso.py` — `_random_sequential_pack` (line 882),
`_RectangularPrism` container (line 253), `pack_spheres` (line 1210). The
high-packing-fraction DEM methods draw on Tan et al. — see [`super::references`].

```rust
pub mod sphere_packing { /* ... */ }
```

### Types

#### Struct `Sphere`

One packed spherical particle (e.g. a TRISO kernel) in a stochastic medium.

```rust
pub struct Sphere {
    pub center: crate::geometry::position::Position,
    pub radius: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `center` | `crate::geometry::position::Position` | Centre \[cm\]. |
| `radius` | `f64` | Radius \[cm\]. |

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
    fn clone(self: &Self) -> Sphere { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &Sphere) -> bool { /* ... */ }
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
#### Enum `PackingMethod`

Which packing-generation algorithm to use. See [`super::references`].

The set is closed and dispatched by `match` (no trait objects), per the
workspace design rules.

```rust
pub enum PackingMethod {
    Rsa,
    RsaDem,
    OdrDem,
}
```

##### Variants

###### `Rsa`

Random Sequential Addition — reject-until-no-overlap insertion. Simple and
parallelizable, but saturates around ~38 % packing fraction ([`MAX_PF_RSA`]).
Implemented (see [`pack_spheres`]). Ref: [`super::references::TAN2024_RSA`].

###### `RsaDem`

Iterative RSA followed by Discrete-Element-Method relaxation, to reach the
high packing fractions RSA alone cannot. **Not implemented.** Ref:
[`super::references::TAN2026_RSA_DEM`].

###### `OdrDem`

Coupled Ordered/Overlap-Driven-Relaxation with DEM. **Not implemented.**
Ref: [`super::references::TAN2026_ODR_DEM`].

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
    fn clone(self: &Self) -> PackingMethod { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &PackingMethod) -> bool { /* ... */ }
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
#### Struct `PackingConfig`

Parameters for generating a packed dispersion-fuel region in a cubic domain.

The domain is an axis-aligned cube centred at the origin with half-width
`domain_half_width`; centres are placed so every sphere lies fully inside it.

```rust
pub struct PackingConfig {
    pub particle_radius: f64,
    pub packing_fraction: f64,
    pub domain_half_width: f64,
    pub method: PackingMethod,
    pub seed: u64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `particle_radius` | `f64` | Particle radius \[cm\] (all particles equal-radius). |
| `packing_fraction` | `f64` | Target volumetric packing fraction (0…[`MAX_PF_RSA`] for [`PackingMethod::Rsa`]). |
| `domain_half_width` | `f64` | Half-width \[cm\] of the cubic domain the particles pack into. |
| `method` | `PackingMethod` | Generation algorithm. |
| `seed` | `u64` | RNG seed for reproducibility. |

##### Implementations

###### Methods

- ```rust
  pub fn generate(self: &Self) -> Result<Vec<Sphere>, PackingError> { /* ... */ }
  ```
  Generate the packed sphere list for this configuration.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> PackingConfig { /* ... */ }
    ```

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
#### Enum `PackingError`

Errors from stochastic-media generation.

```rust
pub enum PackingError {
    NotImplemented(PackingMethod),
    PackingTooDense {
        requested: f64,
        limit: f64,
    },
    PlacementFailed {
        placed: usize,
        target: usize,
        attempts: usize,
    },
    DomainTooSmall {
        half_width: f64,
        radius: f64,
    },
}
```

##### Variants

###### `NotImplemented`

The requested method is not implemented yet (only [`PackingMethod::Rsa`] is).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `PackingMethod` |  |

###### `PackingTooDense`

The target packing fraction exceeds what RSA can reach ([`MAX_PF_RSA`]).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `requested` | `f64` | The requested packing fraction. |
| `limit` | `f64` | The RSA limit ([`MAX_PF_RSA`]). |

###### `PlacementFailed`

A sphere could not be placed without overlap within the attempt budget —
the domain is effectively saturated (try a lower packing fraction).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `placed` | `usize` | How many spheres were placed before giving up. |
| `target` | `usize` | How many were requested. |
| `attempts` | `usize` | The per-sphere attempt budget that was exhausted. |

###### `DomainTooSmall`

The domain is too small to hold even one sphere (radius ≥ half-width).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `half_width` | `f64` | The domain half-width. |
| `radius` | `f64` | The sphere radius. |

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
    fn clone(self: &Self) -> PackingError { /* ... */ }
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
    fn eq(self: &Self, other: &PackingError) -> bool { /* ... */ }
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
#### Struct `PackedSpheres`

A packed set of equal-radius kernels plus a spatial hash grid for fast
point-membership — the geometry the delta-tracking flight queries.

Owns the [`Sphere`] list produced by [`pack_spheres`] and a uniform grid that
makes [`PackedSpheres::is_inside_kernel`] O(1): each kernel is registered in
every grid cell its body touches, so a membership query scans only the kernels
registered in the query point's own cell.

This type is deliberately *physics-free* — it answers "inside a kernel or not?"
and leaves the mapping to material indices (fuel vs matrix) to the caller, so it
can serve any two-material dispersion medium.

```rust
pub struct PackedSpheres {
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
  pub fn pack(radius: f64, half_width: f64, packing_fraction: f64, seed: u64) -> Result<Self, PackingError> { /* ... */ }
  ```
  Pack a cubic domain by RSA and build the membership grid in one step.

- ```rust
  pub fn from_spheres(spheres: Vec<Sphere>, half_width: f64, radius: f64) -> Self { /* ... */ }
  ```
  Build a membership grid over an already-generated packing.

- ```rust
  pub fn is_inside_kernel(self: &Self, p: Position) -> bool { /* ... */ }
  ```
  Is the point `p` \[cm\] inside any packed kernel?

- ```rust
  pub fn spheres(self: &Self) -> &[Sphere] { /* ... */ }
  ```
  The packed kernels.

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Number of packed kernels.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether the packing is empty.

- ```rust
  pub fn half_width(self: &Self) -> f64 { /* ... */ }
  ```
  The cubic domain half-width \[cm\].

- ```rust
  pub fn radius(self: &Self) -> f64 { /* ... */ }
  ```
  The common kernel radius \[cm\].

- ```rust
  pub fn packing_fraction(self: &Self) -> f64 { /* ... */ }
  ```
  Realized volumetric packing fraction `N · V_sphere / V_cube`.

- ```rust
  pub fn min_center_distance(self: &Self) -> Option<f64> { /* ... */ }
  ```
  The smallest centre-to-centre distance between any two kernels \[cm\], or

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> PackedSpheres { /* ... */ }
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

#### Function `pack_spheres`

Random Sequential Addition packing of equal spheres in a cubic domain.

Places `N = floor(pf · V_cube / V_sphere)` kernel centres one at a time at
uniform-random positions inside the cube (each centre kept at least `radius`
from every wall so the sphere is fully contained), rejecting any trial position
that would bring a new sphere within one diameter of an already-placed one.
Returns the accepted centres as [`Sphere`]s.

# Algorithm (ported from OpenMC, MIT)

Direct port of `_random_sequential_pack` (`openmc/model/triso.py:882`) with the
`_RectangularPrism` container's uniform placement and overlap mesh
(`openmc/model/triso.py:253`, `:396`). A uniform grid of cell size
`≈ 4·radius` overlays the domain; each placed centre is registered in every
grid cell within one diameter of it, so an overlap test for a trial point only
scans the spheres registered in the trial's own cell — O(1) instead of O(N).
The overlap predicate is squared-distance `< (2·radius)²`, exactly as upstream.

# Parameters
- `radius` — sphere radius \[cm\]; all spheres equal-radius.
- `half_width` — half-width \[cm\] of the axis-aligned cube (centred at origin).
- `packing_fraction` — target volumetric fraction; must be ≤ [`MAX_PF_RSA`].
- `seed` — RNG seed (uses the crate LCG [`prn`]) for a reproducible packing.

# Errors
- [`PackingError::PackingTooDense`] if `packing_fraction > MAX_PF_RSA`.
- [`PackingError::DomainTooSmall`] if a sphere cannot fit in the cube.
- [`PackingError::PlacementFailed`] if the domain saturates before all
  `N` spheres are placed (the per-sphere attempt budget is exhausted).

```rust
pub fn pack_spheres(radius: f64, half_width: f64, packing_fraction: f64, seed: u64) -> Result<Vec<Sphere>, PackingError> { /* ... */ }
```

### Constants and Statics

#### Constant `MAX_PF_RSA`

Maximum packing fraction Random Sequential Addition can reach in practice.

Mirrors OpenMC's `MAX_PF_RSP = 0.38` (`openmc/model/triso.py:20`). Above this,
RSA's reject-until-no-overlap loop effectively stops finding room for new
spheres; [`pack_spheres`] returns [`PackingError::PackingTooDense`]
rather than spinning forever.

```rust
pub const MAX_PF_RSA: f64 = 0.38;
```

## Module `stochastic`

Stochastic-media transport — random geometry that is *sampled* rather than stored.

A stochastic (random) medium is one whose geometry is not known deterministically:
dispersion fuel, TRISO kernels scattered through a graphite matrix, burnable-poison
particles in an absorber. The defining problem is scale — a pebble holds O(10⁴) fuel
kernels and a core O(10⁵) pebbles — so the practical question is *how much of that
geometry a transport calculation actually has to remember*.

This module is the research track for that question. Three model families sit on a
memory/fidelity spectrum:

| Model | Geometry stored | Point membership | Cost |
|---|---|---|---|
| Explicit / RSA ([`medium::RsaMedium`]) | all of it | exact | O(N) memory |
| SCLS ([`scls::SclsMedium`]) | a moving local window | partial | bounded |
| CLS ([`cls::ClsMedium`]) | none | sampled | O(1) memory |

- **[`medium`]** — the unifying [`medium::StochasticMedium`] enum: one "which
  material is here?" query across all three models, dispatched by `match`.
- **[`cls`]** — Chord Length Sampling: inclusion crossings re-sampled from
  closed-form chord statistics. Cheap and memoryless, so it forgets geometry.
- **[`scls`]** — Semi-Implicit CLS: CLS plus a bounded window of remembered
  inclusions, governed by the Dynamic Inclusion Sphere ([`scls::InclusionSphere`],
  radius `λ_TMFP + R_largest`). The primary research target.
- **[`spatial_index`]** — the acceleration seam for SCLS history lookup, the
  anticipated bottleneck.

# Why this is not inside `pebble_beds`

[`crate::pebble_beds`] is the *pebble-bed reactor* specialization — delta tracking
and the doubly-heterogeneous k-eigenvalue assembly. The methods here are broader
than that application: chord-length sampling applies to any binary random medium,
including dispersion fuel and burnable-poison particles that have nothing to do with
pebbles. Keeping them separate stops a general method from being filed under one
reactor type.

The two do meet: [`medium::RsaMedium`] wraps the RSA packing generated by
[`crate::pebble_beds::sphere_packing`], which stays where it is because the
delta-tracking path depends on it directly.

# Status: scaffold

The chord statistics, the SCLS retention machinery and the brute-force index are
implemented and unit-tested. The **CLS and SCLS transport drivers are not** — those
paths return typed `NotImplemented` errors rather than fabricated answers.

**No accuracy claim is made for CLS or SCLS.** Whether either reproduces the
explicit-RSA reference is an empirical question the benchmark suite (bead
`op-eby.7`) must measure before anything here may be called validated.

Scaffolded per the *OUTRAM-MC Design Scaffold v0.1* (Theodore Ong, Zhe Chuan Tan),
tracked under beads epic `op-eby`. This is **new work**, not an OpenMC port —
upstream has no CLS or SCLS — so the crate's "mirror the canonical source" rule does
not apply here (see the crate `CLAUDE.md`: new parts are scaffolded only where
genuinely absent upstream).

```rust
pub mod stochastic { /* ... */ }
```

### Modules

## Module `benchmark`

Benchmark suite — RSA vs CLS vs SCLS on a common problem (design doc §18, bead
`op-eby.7`).

The three random-media models ([`super::medium::StochasticMedium`]) only differ in
*how much geometry they remember*, so a benchmark has to exercise the one regime
where memory matters: **back-scattering**. A neutron that scatters isotropically in
the matrix and is absorbed on entering an inclusion repeatedly re-crosses ground it
has already covered — exactly where classical CLS's memoryless assumption breaks and
SCLS's retained inclusions are meant to help.

# The problem

An absorbing-inclusion random walk in the packing's cube domain:

- Inclusions are pure absorbers; the matrix is a pure isotropic scatterer with mean
  free path `scatter_mfp` \[cm\].
- Each history is born at a uniform random point (so birth-in-inclusion has
  probability equal to the packing fraction, consistently for every model), given an
  isotropic direction, and walked: sample a scatter distance, step along it querying
  the medium; entering an inclusion **absorbs**, leaving the cube **leaks**, and
  reaching `max_collisions` without either **survives**.
- The tally is the absorption probability.

[`AbsorptionBenchmark::compare`] builds all three media from **one** RSA packing —
RSA sees the explicit geometry, CLS/SCLS see only its statistics (radius + packing
fraction) — and runs the identical walk on each. RSA is the reference; the gap
`|P_model − P_RSA|` is the model's error.

# No accuracy claim is baked in

This module *measures*; it does not assert that SCLS beats CLS. Whether it does, and
by how much, is regime-dependent (inclusion optical thickness, scattering ratio,
packing fraction) and is the empirical output the suite exists to produce. The unit
test checks the harness runs and returns physical probabilities; a representative
measured comparison is recorded below.

# Representative result (2026-07-21)

For `domain_half_width = 0.5 cm`, `particle_radius = 0.05 cm`, `packing_fraction =
0.2`, `scatter_mfp = 0.3 cm`, 4000 histories (seed 20260721), absorption
probabilities were **RSA = 0.684, CLS = 0.697 (+0.013), SCLS = 0.745 (+0.061)**.

Two honest observations, not accuracy claims:
- Both approximate models land within ~0.06 of the explicit reference, so the harness
  is clearly measuring the same physics on each arm.
- In *this* regime **CLS is actually closer to RSA than SCLS**, and both overestimate
  absorption. That is a real, regime-dependent finding — SCLS's retained inclusions
  raise the re-encounter (and hence absorption) rate on back-scatter, which here
  over-corrects past the reference. Whether SCLS wins in optically-thicker or
  higher-packing regimes is exactly the parameter study this suite exists to run; it
  is not asserted here. These numbers are reproducible from the seed but are a
  generated result, not a committed reference (crate `CLAUDE.md` V&V-output rule).

This module is **new work**, not an OpenMC port.

```rust
pub mod benchmark { /* ... */ }
```

### Types

#### Struct `BenchmarkResult`

The measured outcome distribution for one model.

```rust
pub struct BenchmarkResult {
    pub model: &'static str,
    pub absorption_probability: f64,
    pub leakage_probability: f64,
    pub histories: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `model` | `&'static str` | Model name ([`StochasticMedium::name`]). |
| `absorption_probability` | `f64` | Fraction of histories absorbed in an inclusion. |
| `leakage_probability` | `f64` | Fraction of histories that leaked out of the domain. |
| `histories` | `usize` | Number of histories run. |

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
    fn clone(self: &Self) -> BenchmarkResult { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &BenchmarkResult) -> bool { /* ... */ }
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
#### Struct `AbsorptionBenchmark`

Absorbing-inclusion random-walk benchmark configuration.

```rust
pub struct AbsorptionBenchmark {
    pub domain_half_width: f64,
    pub particle_radius: f64,
    pub packing_fraction: f64,
    pub scatter_mfp: f64,
    pub max_collisions: usize,
    pub histories: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `domain_half_width` | `f64` | Half-width of the cube domain \[cm\]. |
| `particle_radius` | `f64` | Inclusion (absorber) radius \[cm\]. |
| `packing_fraction` | `f64` | Inclusion volume (packing) fraction, in (0, 1). |
| `scatter_mfp` | `f64` | Matrix scatter mean free path \[cm\]. |
| `max_collisions` | `usize` | Maximum matrix collisions before a history is declared "survived". |
| `histories` | `usize` | Histories per model. |

##### Implementations

###### Methods

- ```rust
  pub fn run(self: &Self, medium: &mut StochasticMedium, seed: &mut u64) -> BenchmarkResult { /* ... */ }
  ```
  Run `histories` walks through one medium and tally the outcomes.

- ```rust
  pub fn compare(self: &Self, seed: u64) -> Result<[BenchmarkResult; 3], PackingError> { /* ... */ }
  ```
  Build RSA, CLS and SCLS media from **one** RSA packing and run the identical

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> AbsorptionBenchmark { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &AbsorptionBenchmark) -> bool { /* ... */ }
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
## Module `cls`

Classical Chord Length Sampling (CLS) — memoryless random-media transport.

CLS replaces stored geometry with a *distribution*. Instead of asking an explicit
packing "where is the next kernel surface along this ray?", CLS samples the distance
to the next inclusion crossing from a chord-length distribution whose mean is fixed
by the packing statistics. Nothing is remembered between samples.

```text
explicit:  ray ──►│kernel│────►│kernel│───►     (surfaces looked up)
CLS:       ray ──► sample ℓ₁ ──► sample ℓ₂ ──►  (surfaces re-invented each time)
```

The win is memory and speed: a pebble holds O(10⁴) kernels and a core O(10⁵)
pebbles, none of which CLS stores. The cost is the **Markov (memoryless)
assumption** — each sampled chord is independent of every previous one, so the
model forgets geometry. A neutron that scatters backwards does not re-encounter the
kernel it just traversed, and clustered or correlated packings are not reproduced.
Recovering that memory is what [`super::scls`] exists to do.

# What is implemented here

The chord-length statistics, which are exact, closed-form and independently
testable:

- [`mean_chord_length_sphere`] — Cauchy's mean-chord result for a convex body.
- [`matrix_mean_chord_length`] — the binary-Markovian matrix counterpart.
- [`sample_chord`] — exponential sampling from a mean chord length.

# The flight driver (bead `op-eby.2`, implemented)

[`ClsMedium::material_at`] reconstructs phase occupancy statefully along a flight:
it seeds the phase from the volume-fraction prior, then advances by the scalar path
length between successive queries, toggling phase and re-sampling a chord at every
boundary crossed. Because the chord statistics are direction-independent this needs
only the distance between queries, which *is* the memoryless approximation. Its
defining consistency property — inclusion occupancy converging to the packing
fraction — is unit-tested. Coupling this into the k-eigenvalue transport loop (the
benchmark of bead `op-eby.7`) is the remaining integration step, not a gap in CLS
itself.

# References

- Cauchy's formula for the mean chord of a convex body, `<ℓ> = 4V/S`. For a sphere
  of radius `r` this gives `4r/3`.
- Binary stochastic mixtures and the Markovian chord relation: Lux & Koblinger,
  *Monte Carlo Particle Transport Methods*, CRC Press (1991); Zimmerman & Adams,
  *Algorithms for Monte Carlo particle transport in binary statistical mixtures*
  (1991). See also [`crate::pebble_beds::references`] for the dispersion-fuel bibliography.

This module is **new work**, not a port — OpenMC has no CLS implementation, so the
crate's "mirror the canonical source" rule does not apply here (see the crate
`CLAUDE.md`: new parts are scaffolded only where genuinely absent upstream).

```rust
pub mod cls { /* ... */ }
```

### Types

#### Struct `ClsMedium`

A memoryless chord-length-sampled random medium.

Holds the *statistics* of the packing — inclusion radius, packing fraction, and
the two phase materials — never the inclusions themselves; the only per-history
state is the small [`ClsFlight`] reconstructed lazily on the first
[`Self::material_at`]. That is the whole point: the struct is O(1) in memory
regardless of how many inclusions the medium notionally contains.

Not `Copy`: a medium carries live flight state mid-history, and a silent copy
would fork that state into two inconsistent flights.

```rust
pub struct ClsMedium {
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
  pub fn new(inclusion_radius: f64, packing_fraction: f64, inclusion: MaterialId, matrix: MaterialId) -> Self { /* ... */ }
  ```
  Build a CLS medium from packing statistics.

- ```rust
  pub fn begin_flight(self: &mut Self) { /* ... */ }
  ```
  Discard any in-progress flight, so the next [`Self::material_at`] re-seeds the

- ```rust
  pub fn in_inclusion(self: &Self) -> Option<bool> { /* ... */ }
  ```
  The phase the reconstructed flight is currently in, or `None` before the first

- ```rust
  pub fn inclusion_radius(self: &Self) -> f64 { /* ... */ }
  ```
  Inclusion radius \[cm\].

- ```rust
  pub fn packing_fraction(self: &Self) -> f64 { /* ... */ }
  ```
  Inclusion volume (packing) fraction.

- ```rust
  pub fn inclusion_material(self: &Self) -> MaterialId { /* ... */ }
  ```
  Material id of the inclusion phase.

- ```rust
  pub fn matrix_material(self: &Self) -> MaterialId { /* ... */ }
  ```
  Material id of the matrix phase.

- ```rust
  pub fn mean_chord_inclusion(self: &Self) -> f64 { /* ... */ }
  ```
  Mean chord length \[cm\] through one inclusion — [`mean_chord_length_sphere`].

- ```rust
  pub fn mean_chord_matrix(self: &Self) -> f64 { /* ... */ }
  ```
  Mean chord length \[cm\] through the matrix — [`matrix_mean_chord_length`].

- ```rust
  pub fn sample_distance_to_boundary(self: &Self, in_inclusion: bool, seed: &mut u64) -> f64 { /* ... */ }
  ```
  Sample the distance \[cm\] to the next inclusion boundary, given which phase the

- ```rust
  pub fn material_at(self: &mut Self, position: Position, seed: &mut u64) -> Result<MaterialId, MediumError> { /* ... */ }
  ```
  Material occupying `position` \[cm\], reconstructed along the CLS flight.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> ClsMedium { /* ... */ }
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
    fn eq(self: &Self, other: &ClsMedium) -> bool { /* ... */ }
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

#### Function `mean_chord_length_sphere`

Mean chord length \[cm\] through a sphere of radius `radius` \[cm\].

Cauchy's mean-chord result for a convex body is `<ℓ> = 4V/S`. For a sphere,
`V = (4/3)πr³` and `S = 4πr²`, so

```text
<ℓ> = 4 · (4/3)πr³ / (4πr²) = 4r/3
```

This is the mean distance a uniformly-and-isotropically incident ray spends inside
one inclusion, and it sets the inclusion-phase chord statistics for CLS.

`radius` must be > 0; a non-positive radius yields 0.

```rust
pub fn mean_chord_length_sphere(radius: f64) -> f64 { /* ... */ }
```

#### Function `matrix_mean_chord_length`

Mean chord length \[cm\] through the *matrix* phase between spherical inclusions.

For a binary stochastic mixture the two phases' mean chords are tied to their volume
fractions by the Markovian relation `p_i = <ℓ_i> / (<ℓ_1> + <ℓ_2>)`, i.e.
`<ℓ_matrix> / <ℓ_incl> = p_matrix / p_incl`. With the inclusion phase occupying the
packing fraction `pf` and the matrix the remaining `1 - pf`:

```text
<ℓ_matrix> = (4r/3) · (1 - pf) / pf
```

So a sparse packing gives long matrix flights and a dense one gives short flights,
as expected.

# Parameters
- `radius` — inclusion radius \[cm\].
- `packing_fraction` — inclusion volume fraction, in (0, 1).

Returns [`f64::INFINITY`] when `packing_fraction` is 0 (no inclusions, so the
neutron never hits one) and 0 when it is >= 1 (no matrix to fly through).

```rust
pub fn matrix_mean_chord_length(radius: f64, packing_fraction: f64) -> f64 { /* ... */ }
```

#### Function `sample_chord`

Sample a chord length \[cm\] from an exponential distribution with the given mean.

The Markovian assumption makes chord lengths exponentially distributed, so inverse
-transform sampling gives `ℓ = -<ℓ>·ln(ξ)` for `ξ` uniform on (0, 1]. `seed` is the
crate LCG stream ([`prn`]), advanced in place.

Uses the crate's OpenMC-derived LCG rather than `rand`/`rand_chacha`: the v0.1
design scaffold names those crates, but this crate's reproducibility guarantee
depends on per-particle LCG streams with O(log n) jump-ahead
([`crate::rng::lcg::future_seed`]), which `rand_chacha` would break. Workspace and
crate rules take precedence over the design doc here.

A non-positive `mean_chord` yields 0.

```rust
pub fn sample_chord(mean_chord: f64, seed: &mut u64) -> f64 { /* ... */ }
```

## Module `medium`

Unified "which material is at this point?" query over every random-media model.

A stochastic (random) medium is one whose geometry is not known deterministically:
dispersion fuel, TRISO kernels in a graphite matrix, burnable-poison particles in
an absorber. Three families of model answer the transport loop's material query,
trading memory against fidelity:

- **Explicit / RSA** ([`RsaMedium`]) — every inclusion is generated and stored, so
  the point-membership answer is *exact*. High fidelity, high memory. This is the
  **reference solution** every approximate model is judged against. Built on the
  already-implemented [`crate::pebble_beds::sphere_packing::pack_spheres`].
- **Chord Length Sampling (CLS)** ([`super::cls::ClsMedium`]) — nothing is stored;
  inclusion crossings are re-sampled from a chord-length distribution on the fly.
  Cheap, but *memoryless*: a neutron that turns around does not see the inclusion
  it just crossed.
- **Semi-Implicit CLS (SCLS)** ([`super::scls::SclsMedium`]) — a bounded window of
  recent inclusions is retained, recovering the local geometric memory CLS discards
  while keeping memory bounded. The primary research target of this module.

# Design-doc deviations (workspace rules take precedence)

The v0.1 design scaffold specifies `pub trait StochasticMedium { fn material_at(…) }`
and dispatches through it. The workspace Rust design rules forbid trait-object
dispatch, so this module instead exposes the closed enum [`StochasticMedium`]
dispatched by `match`. The set of random-media models is closed and known at compile
time, which is exactly the case the enum rule is written for: adding a model forces
every `match` site to handle it, and no variant is heap-allocated.

The doc's `material_at(&self, position, neutron: &Neutron)` is also adjusted:

- It takes **`&mut self`**, because CLS and SCLS mutate sampler/history state on
  every query — they are not pure functions of position.
- It takes **`seed: &mut u64`** rather than a whole particle, matching the crate's
  existing LCG convention (see
  [`crate::pebble_beds::delta_tracking::sample_delta_distance`]).
  The query needs randomness, not the full phase-space state.
- It returns a [`Result`], so a not-yet-implemented model reports that instead of
  panicking inside a transport loop.

# A caveat worth stating plainly

Point membership is only a *well-posed* question for an explicit medium. CLS and
SCLS are **flight-level** samplers: they answer "where is the next inclusion
boundary along this ray?", and reconstruct material occupancy statistically rather
than storing it. [`StochasticMedium::material_at`] is therefore exact for
[`StochasticMedium::Rsa`] and approximate-by-construction for the others. Treat a
CLS/SCLS material answer as a sample, not as ground truth.

```rust
pub mod medium { /* ... */ }
```

### Types

#### Struct `MaterialId`

Index of a material in the caller's material table.

A transparent newtype over `usize` rather than a bare integer, so a material index
cannot be silently swapped with a cell index, a nuclide index, or a sphere index.
This crate does not own the material table; the caller maps an id back to its
[`crate::material::material::Material`].

```rust
pub struct MaterialId(pub usize);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

##### Implementations

###### Methods

- ```rust
  pub fn index(self: Self) -> usize { /* ... */ }
  ```
  The raw table index.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> MaterialId { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Comparable**
  - ```rust
    fn compare(self: &Self, key: &K) -> Ordering { /* ... */ }
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

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoEither**
- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &MaterialId) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &MaterialId) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &MaterialId) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
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
#### Enum `MediumError`

Errors from a stochastic-medium material query.

```rust
pub enum MediumError {
    NotImplemented(&'static str),
}
```

##### Variants

###### `NotImplemented`

The model is scaffolded but its sampling logic is not built out yet.

Carries the model name so a transport driver can report which one it hit.

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> MediumError { /* ... */ }
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
    fn eq(self: &Self, other: &MediumError) -> bool { /* ... */ }
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
#### Struct `RsaMedium`

Explicit random medium — every inclusion stored, point membership exact.

Wraps the already-implemented RSA packing ([`crate::pebble_beds::sphere_packing::PackedSpheres`])
and tags the two phases with material ids. This is the reference model: CLS and SCLS
are correct to the extent they reproduce what this returns.

```rust
pub struct RsaMedium {
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
  pub fn new(packing: PackedSpheres, inclusion: MaterialId, matrix: MaterialId) -> Self { /* ... */ }
  ```
  Build an explicit medium from a packing and the two phase materials.

- ```rust
  pub fn material_at(self: &Self, p: Position) -> MaterialId { /* ... */ }
  ```
  Exact point membership: inclusion material if `p` is inside a packed sphere,

- ```rust
  pub fn packing(self: &Self) -> &PackedSpheres { /* ... */ }
  ```
  The underlying packing (inclusion centres and radii).

- ```rust
  pub fn inclusion_material(self: &Self) -> MaterialId { /* ... */ }
  ```
  Material id of the inclusion phase.

- ```rust
  pub fn matrix_material(self: &Self) -> MaterialId { /* ... */ }
  ```
  Material id of the matrix phase.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> RsaMedium { /* ... */ }
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
#### Enum `StochasticMedium`

A random-media model, dispatched by `match` (no trait objects — see module docs).

Ordered by increasing approximation: [`Self::Rsa`] is exact and expensive,
[`Self::Scls`] retains bounded memory, [`Self::Cls`] retains none.

```rust
pub enum StochasticMedium {
    Rsa(RsaMedium),
    Cls(super::cls::ClsMedium),
    Scls(super::scls::SclsMedium),
}
```

##### Variants

###### `Rsa`

Explicit stored geometry — the reference solution. Implemented.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `RsaMedium` |  |

###### `Cls`

Memoryless chord-length sampling. Scaffolded — see [`super::cls`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `super::cls::ClsMedium` |  |

###### `Scls`

Semi-implicit CLS with retained histories. Scaffolded — see [`super::scls`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `super::scls::SclsMedium` |  |

##### Implementations

###### Methods

- ```rust
  pub fn material_at(self: &mut Self, position: Position, seed: &mut u64) -> Result<MaterialId, MediumError> { /* ... */ }
  ```
  Which material occupies `position`.

- ```rust
  pub fn begin_flight(self: &mut Self) { /* ... */ }
  ```
  Discard any in-progress flight state, so the next [`Self::material_at`] starts a

- ```rust
  pub fn name(self: &Self) -> &'static str { /* ... */ }
  ```
  Short model name, for error messages and benchmark tables.

- ```rust
  pub fn is_exact(self: &Self) -> bool { /* ... */ }
  ```
  Whether this model answers [`Self::material_at`] exactly.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> StochasticMedium { /* ... */ }
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
## Module `scls`

Semi-Implicit Chord Length Sampling (SCLS) — CLS with bounded geometric memory.

Classical CLS ([`super::cls`]) is memoryless: every sampled chord is independent, so
a neutron that scatters backwards does not re-encounter the inclusion it just flew
through. That is the dominant error source in CLS, and it grows exactly where
dispersion-fuel problems live — optically thick inclusions and strong scattering.

SCLS keeps the cheap chord sampling but **remembers recently-encountered
inclusions**. Sampled inclusions are promoted to stored [`ParticleHistory`] records
and consulted on subsequent flights, so a neutron re-crossing its own path sees the
same geometry twice — recovering the correlation CLS throws away, without ever
storing the full O(10⁴)-kernel packing an explicit model needs.

Memory is bounded by the **Dynamic Inclusion Sphere** ([`InclusionSphere`]): a ball
that follows the neutron and defines what counts as "local". Histories inside are
retained; histories the neutron has flown away from are culled.

```text
           ┌──────── inclusion sphere (moves with neutron) ────────┐
 culled ○  │   ● retained    ● retained      ◉ neutron             │  ○ culled
           └───────────────────────────────────────────────────────┘
            R = λ_TMFP + R_largest
```

# Sphere radius

```text
R = λ_TMFP + R_largest
```

One transport mean free path is the distance over which the neutron's direction
decorrelates, so geometry beyond it is unlikely to be revisited before being
forgotten anyway; adding the largest inclusion radius guarantees that any inclusion
whose *body* could still intersect the local neighbourhood is retained even when its
centre sits just outside. See [`InclusionSphere::new`].

# What is implemented here

The retention machinery, which is self-contained and testable:

- [`ParticleHistory`] / [`FlightSegment`] — the retained records (design doc §14).
- [`InclusionSphere`] — radius rule, containment, re-centring (design doc §15).
- [`SclsMedium::advance_to`] — move the neutron, re-centre the sphere, cull.
- [`SclsMedium::retained_material_at`] — exact answer *for retained geometry*.

# The transport driver (bead `op-eby.3`, implemented)

[`SclsMedium::material_at`] marches the flight and resolves each phase boundary:
matrix gaps are memoryless exponential chords, but a matrix→inclusion crossing first
tests the ray against every retained inclusion and **re-enters a remembered sphere**
when one is closer than the sampled gap — recovering the correlation classical CLS
discards. A genuinely new crossing spawns a real sphere with a correctly-distributed
impact parameter (`b = R√ξ`, giving the Cauchy mean chord `4R/3`) and remembers it;
inclusion exits are the exact geometric ray–sphere exit. Its occupancy still
converges to the packing fraction, and the memory is demonstrated by a re-crossing
test whose history count saturates instead of growing with the number of passes.
Coupling into the k-eigenvalue loop and the RSA-vs-CLS-vs-SCLS accuracy comparison is
the benchmark of bead `op-eby.7`; the adaptive-radius extension is `op-eby.6`.

**No accuracy claim is made here.** Whether SCLS actually recovers the explicit-RSA
answer is an empirical question that the benchmark suite (bead `op-eby.7`) must
measure before anything in this module may be described as validated.

This module is **new work**, not an OpenMC port — upstream has no SCLS.

```rust
pub mod scls { /* ... */ }
```

### Types

#### Struct `ParticleHistory`

One remembered inclusion — an inclusion the neutron has already encountered.

Design doc §14. Stored so that a later flight through the same neighbourhood sees
the same inclusion rather than re-sampling an independent one.

```rust
pub struct ParticleHistory {
    pub center: crate::geometry::position::Position,
    pub radius: f64,
    pub material_id: crate::stochastic::medium::MaterialId,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `center` | `crate::geometry::position::Position` | Inclusion centre \[cm\]. |
| `radius` | `f64` | Inclusion radius \[cm\]. |
| `material_id` | `crate::stochastic::medium::MaterialId` | Material occupying the inclusion. |

##### Implementations

###### Methods

- ```rust
  pub fn new(center: Position, radius: f64, material_id: MaterialId) -> Self { /* ... */ }
  ```
  A remembered inclusion at `center` \[cm\] with `radius` \[cm\].

- ```rust
  pub fn contains(self: &Self, p: Position) -> bool { /* ... */ }
  ```
  Whether `p` \[cm\] lies inside this inclusion.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> ParticleHistory { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &ParticleHistory) -> bool { /* ... */ }
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
#### Struct `FlightSegment`

One remembered straight-line flight leg between collisions.

Design doc §14. Retaining traversed segments lets an SCLS driver detect when a
neutron re-enters ground it has already covered, which is where the memoryless
assumption does the most damage.

```rust
pub struct FlightSegment {
    pub start: crate::geometry::position::Position,
    pub end: crate::geometry::position::Position,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `start` | `crate::geometry::position::Position` | Segment start \[cm\]. |
| `end` | `crate::geometry::position::Position` | Segment end \[cm\]. |

##### Implementations

###### Methods

- ```rust
  pub fn new(start: Position, end: Position) -> Self { /* ... */ }
  ```
  A flight leg from `start` to `end` \[cm\].

- ```rust
  pub fn length(self: &Self) -> f64 { /* ... */ }
  ```
  Segment length \[cm\].

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> FlightSegment { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &FlightSegment) -> bool { /* ... */ }
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
#### Struct `InclusionSphere`

The Dynamic Inclusion Sphere — the moving ball that bounds SCLS memory.

Design doc §15, the core SCLS innovation. Re-centred on the neutron at every
collision; anything it no longer covers is forgotten.

```rust
pub struct InclusionSphere {
    pub center: crate::geometry::position::Position,
    pub radius: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `center` | `crate::geometry::position::Position` | Current centre \[cm\] — tracks the neutron position. |
| `radius` | `f64` | Current radius \[cm\]. |

##### Implementations

###### Methods

- ```rust
  pub fn new(center: Position, transport_mfp: f64, largest_inclusion_radius: f64) -> Self { /* ... */ }
  ```
  Build the sphere from the physics that sets its size.

- ```rust
  pub fn contains(self: &Self, p: Position) -> bool { /* ... */ }
  ```
  Whether point `p` \[cm\] lies within the sphere.

- ```rust
  pub fn overlaps(self: &Self, center: Position, radius: f64) -> bool { /* ... */ }
  ```
  Whether an inclusion of `radius` \[cm\] centred at `center` \[cm\] overlaps this

- ```rust
  pub fn recenter(self: &mut Self, new_center: Position) { /* ... */ }
  ```
  Move the sphere to follow the neutron to `new_center` \[cm\]. Radius unchanged.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> InclusionSphere { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &InclusionSphere) -> bool { /* ... */ }
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
#### Struct `SclsMedium`

A semi-implicit CLS medium — chord statistics plus a bounded window of remembered
geometry.

Sits between [`super::cls::ClsMedium`] (no memory, O(1) storage) and
[`super::medium::RsaMedium`] (total memory, O(N) storage): memory is O(number of
inclusions within one inclusion-sphere volume), independent of how large the overall
medium is.

```rust
pub struct SclsMedium {
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
  pub fn new(cls: super::cls::ClsMedium, start: Position, transport_mfp: f64) -> Self { /* ... */ }
  ```
  Build an SCLS medium around a CLS sampler.

- ```rust
  pub fn cls(self: &Self) -> &super::cls::ClsMedium { /* ... */ }
  ```
  The underlying memoryless chord sampler.

- ```rust
  pub fn sphere(self: &Self) -> InclusionSphere { /* ... */ }
  ```
  The current retention window.

- ```rust
  pub fn histories(self: &Self) -> &[ParticleHistory] { /* ... */ }
  ```
  Currently remembered inclusions.

- ```rust
  pub fn flights(self: &Self) -> &[FlightSegment] { /* ... */ }
  ```
  Currently remembered flight legs.

- ```rust
  pub fn remember_inclusion(self: &mut Self, history: ParticleHistory) { /* ... */ }
  ```
  Remember an inclusion the neutron has encountered.

- ```rust
  pub fn remember_flight(self: &mut Self, segment: FlightSegment) { /* ... */ }
  ```
  Remember a traversed flight leg.

- ```rust
  pub fn advance_to(self: &mut Self, new_position: Position) -> usize { /* ... */ }
  ```
  Move the neutron to `new_position` \[cm\], re-centre the window, and cull.

- ```rust
  pub fn set_sphere_radius(self: &mut Self, radius: f64) { /* ... */ }
  ```
  Set the retention-window radius \[cm\] directly, then cull anything the resized

- ```rust
  pub fn adapt_radius(self: &mut Self, controller: &mut AdaptiveRadius, track_length: f64) { /* ... */ }
  ```
  One adaptive step: feed the just-completed collision-to-collision `track_length`

- ```rust
  pub fn retained_material_at(self: &Self, p: Position) -> Option<MaterialId> { /* ... */ }
  ```
  Material at `p` \[cm\] **if remembered geometry already answers the question**.

- ```rust
  pub fn begin_flight(self: &mut Self) { /* ... */ }
  ```
  Discard the in-progress flight so the next [`Self::material_at`] re-seeds the

- ```rust
  pub fn in_inclusion(self: &Self) -> Option<bool> { /* ... */ }
  ```
  The phase the reconstructed flight is currently in, or `None` before the first

- ```rust
  pub fn material_at(self: &mut Self, position: Position, seed: &mut u64) -> Result<MaterialId, MediumError> { /* ... */ }
  ```
  Material occupying `position` \[cm\], reconstructed along the SCLS flight

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> SclsMedium { /* ... */ }
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
    fn eq(self: &Self, other: &SclsMedium) -> bool { /* ... */ }
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
#### Struct `AdaptiveRadius`

Adaptive inclusion-sphere radius controller (design doc §17, bead `op-eby.6`).

The fixed-radius SCLS ([`InclusionSphere::new`]) sizes the window from a *static*
transport mean free path `λ_TMFP` supplied up front. In a heterogeneous problem the
true local mean free path varies — long in optically-thin matrix, short in a dense
inclusion cluster — so a single `λ_TMFP` is either too small (forgetting geometry the
neutron will revisit) or too large (paying to remember geometry it never will).

This controller estimates the local mean free path online as an exponential moving
average of the observed collision-to-collision track lengths, and sets

```text
R = clamp(<track> + R_largest, R_min, R_max)
```

mirroring the fixed rule with `<track>` in place of the static `λ_TMFP`. The EMA
weight `alpha` in (0, 1] trades responsiveness (large) against stability (small).

This is a **research extension** whose accuracy/efficiency payoff is an empirical
question for the benchmark (`op-eby.7`); no claim is made here that it beats the
fixed-radius model.

```rust
pub struct AdaptiveRadius {
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
  pub fn new(initial_mfp: f64, alpha: f64, largest_radius: f64, min_radius: f64, max_radius: f64) -> Self { /* ... */ }
  ```
  Build a controller.

- ```rust
  pub fn radius(self: &Self) -> f64 { /* ... */ }
  ```
  The current radius estimate \[cm\] without folding in a new track.

- ```rust
  pub fn mean_track(self: &Self) -> f64 { /* ... */ }
  ```
  The current EMA mean track length \[cm\].

- ```rust
  pub fn update(self: &mut Self, track_length: f64) -> f64 { /* ... */ }
  ```
  Fold a new collision-to-collision `track_length` \[cm\] into the EMA and return the

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> AdaptiveRadius { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &AdaptiveRadius) -> bool { /* ... */ }
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
## Module `spatial_index`

Spatial acceleration for SCLS retained-history lookup.

The anticipated SCLS bottleneck is not the physics but the **lookup**: every flight
asks "which remembered inclusion, if any, covers this point?", and a linear scan over
the retained set makes that O(N) per query. With the Dynamic Inclusion Sphere holding
a few hundred to a few thousand histories, that scan can dominate runtime.

This module puts the query behind one abstraction so a faster backend can be swapped
in and measured without touching the transport code.

# Design-doc deviations (workspace rules take precedence)

Design doc §16 specifies `pub trait SpatialIndex` with brute-force / KD-tree / R-tree
backends, naming the `kiddo` and `rstar` crates. Three workspace rules reshape that:

1. **No trait-object dispatch** — the backend set is closed and known at compile
   time, so this is the enum [`SpatialIndex`], dispatched by `match`.
2. **Dependency policy** — third-party versions live only in the root
   `[workspace.dependencies]`. Adding `kiddo`/`rstar` is a workspace-level decision,
   not something a leaf module does on its own.
3. **Android/Termux portability** — every non-GUI library must build natively on
   Termux, so any new dependency must be checked there before adoption.

So no third-party tree dependency is added. [`SpatialIndex::BruteForce`] is the
correctness baseline; [`SpatialIndex::KdTree`] is a **dependency-free, index-based
kd-tree** ([`KdTreeIndex`]) written to the workspace rules (no `Box`; tree links are
`u32` indices into a flat node `Vec`), so the acceleration seam is real and its
answers are unit-tested to match brute force exactly. [`SpatialIndex::RTree`] remains
the honest "declared but not built" variant — adopting `rstar` is a workspace-level
dependency + Termux-portability decision, not a leaf-module one. Brute force stays the
baseline the trees must beat in the benchmark. Tracked as bead `op-eby.5`.

# Note on the existing grid

[`crate::pebble_beds::sphere_packing::PackedSpheres`] already carries a uniform spatial hash
grid for its RSA overlap test. That grid is tuned for a *static, equal-radius*
packing; SCLS needs an index over a *churning* history set that is rebuilt as the
inclusion sphere moves. Reusing versus rebuilding it is an open question the
benchmark should settle rather than a decision to make up front.

```rust
pub mod spatial_index { /* ... */ }
```

### Types

#### Enum `IndexError`

Errors from a spatial-index query.

```rust
pub enum IndexError {
    BackendNotImplemented(&'static str),
}
```

##### Variants

###### `BackendNotImplemented`

The backend is declared but not built out (see the module docs).

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

- **CastableFrom**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> IndexError { /* ... */ }
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
    fn eq(self: &Self, other: &IndexError) -> bool { /* ... */ }
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
#### Struct `BruteForceIndex`

Linear-scan index — the correctness baseline.

O(N) per query and O(N) memory, with no dependency and no build step. Every faster
backend must reproduce its answers exactly, so it doubles as the test oracle.

```rust
pub struct BruteForceIndex {
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
  An empty index.

- ```rust
  pub fn from_histories(histories: Vec<ParticleHistory>) -> Self { /* ... */ }
  ```
  Build an index over an existing history set.

- ```rust
  pub fn insert(self: &mut Self, history: ParticleHistory) { /* ... */ }
  ```
  Add one remembered inclusion.

- ```rust
  pub fn clear(self: &mut Self) { /* ... */ }
  ```
  Drop every indexed history (called when the inclusion sphere is rebuilt).

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  How many histories are indexed.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether the index is empty.

- ```rust
  pub fn histories(self: &Self) -> &[ParticleHistory] { /* ... */ }
  ```
  The indexed histories.

- ```rust
  pub fn find_containing(self: &Self, p: Position) -> Option<&ParticleHistory> { /* ... */ }
  ```
  The first indexed inclusion containing `p` \[cm\], if any.

- ```rust
  pub fn query_within(self: &Self, p: Position, radius: f64) -> Vec<usize> { /* ... */ }
  ```
  Indices of every inclusion whose *centre* lies within `radius` \[cm\] of `p`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> BruteForceIndex { /* ... */ }
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
    fn default() -> BruteForceIndex { /* ... */ }
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
    fn eq(self: &Self, other: &BruteForceIndex) -> bool { /* ... */ }
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
#### Struct `KdTreeIndex`

A static 3-D kd-tree over inclusion centres — an accelerated alternative to the
linear scan for the retained-history lookup.

The tree is stored as a flat `Vec` of nodes referencing their children by index
(`u32::MAX` = none), not with `Box` pointers — the workspace forbids `Box` and models
tree links as indices (the `CellId(usize)` pattern). It is **static**: built once from
a history snapshot with [`Self::build`], and rebuilt (not incrementally updated) when
the inclusion sphere churns the retained set. Whether periodic rebuilds beat the
brute-force scan for the few-hundred-history working set is exactly what the benchmark
(`op-eby.7`) can now measure, since both backends are real and return identical
answers.

```rust
pub struct KdTreeIndex {
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
  pub fn build(histories: Vec<ParticleHistory>) -> Self { /* ... */ }
  ```
  Build a balanced kd-tree over `histories`. O(N log N) via median partitioning on

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  How many inclusions are indexed.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether the tree is empty.

- ```rust
  pub fn find_containing(self: &Self, p: Position) -> Option<&ParticleHistory> { /* ... */ }
  ```
  The inclusion containing `p`, if any — a range search over centres within

- ```rust
  pub fn query_within(self: &Self, p: Position, radius: f64) -> Vec<usize> { /* ... */ }
  ```
  Indices (into the original history order) of every inclusion whose *centre* lies

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> KdTreeIndex { /* ... */ }
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
    fn default() -> KdTreeIndex { /* ... */ }
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
    fn eq(self: &Self, other: &KdTreeIndex) -> bool { /* ... */ }
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
#### Enum `SpatialIndex`

A spatial-index backend, dispatched by `match` (no trait objects — see module docs).

```rust
pub enum SpatialIndex {
    BruteForce(BruteForceIndex),
    KdTree(KdTreeIndex),
    RTree,
}
```

##### Variants

###### `BruteForce`

Linear scan. Implemented; the correctness baseline.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `BruteForceIndex` |  |

###### `KdTree`

Static kd-tree over inclusion centres (dependency-free, index-based). Implemented;
returns answers identical to [`Self::BruteForce`].

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `KdTreeIndex` |  |

###### `RTree`

R-tree (design doc suggests `rstar`). **Not implemented** — adding the crate is a
workspace-level dependency + Termux-portability decision, kept as the honest
"declared but not built" seam.

##### Implementations

###### Methods

- ```rust
  pub fn brute_force(histories: Vec<ParticleHistory>) -> Self { /* ... */ }
  ```
  A brute-force index over `histories`.

- ```rust
  pub fn kd_tree(histories: Vec<ParticleHistory>) -> Self { /* ... */ }
  ```
  A kd-tree index over `histories` (dependency-free, built in O(N log N)).

- ```rust
  pub fn name(self: &Self) -> &'static str { /* ... */ }
  ```
  Short backend name, for benchmark tables and error messages.

- ```rust
  pub fn find_containing(self: &Self, p: Position) -> Result<Option<&ParticleHistory>, IndexError> { /* ... */ }
  ```
  The inclusion containing `p` \[cm\], if any.

- ```rust
  pub fn query_within(self: &Self, p: Position, radius: f64) -> Result<Vec<usize>, IndexError> { /* ... */ }
  ```
  Indices of inclusions whose centre is within `radius` \[cm\] of `p`.

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  How many histories are indexed (0 for the unimplemented [`Self::RTree`] backend).

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether the index holds no histories.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> SpatialIndex { /* ... */ }
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
    fn eq(self: &Self, other: &SpatialIndex) -> bool { /* ... */ }
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
## Module `depletion`

# Depletion / transmutation driver

Evolves nuclide number densities under combined **radioactive decay** and
**neutron-induced transmutation** (the Bateman equations), and couples that
evolution to the Monte Carlo transport in [`crate::physics`] to form a
**burnup loop**: transport gives one-group reaction rates, the rates build a
depletion matrix, the matrix is exponentiated over a burnup step, and the
updated densities feed the next transport solve.

```text
  transport (k, one-group rates)  ->  DepletionChain::build_matrix
           ^                                     |
           |                                     v
    updated densities   <--  cram16( A, N, dt )  (matrix exponential)
```

## What belongs here / what does not

* [`matrix`] — the dense burnup matrix `A` (`dN/dt = A N`, units `1/s`).
* [`cram`] — the **Chebyshev Rational Approximation Method** matrix-exponential
  solver `N(t+dt) = exp(A dt) N(t)`, the numerically sound scheme OpenMC uses
  for the stiff decay/transmutation system.
* [`chain`] — the [`DepletionChain`](chain::DepletionChain): decay constants,
  branching, neutron-reaction targets, and fission yields, assembled into a
  [`DepletionMatrix`](matrix::DepletionMatrix). Consumes external open data
  (OpenMC ENDF/B-8 depletion-chain decay data and ENDF/B-VIII.0 fission
  yields) rather than rebuilding it.
* [`operator`] — the burnup loop (predictor / forward-Euler integrator)
  coupling transport to the transmutation step.

Nuclear-data *parsing* does **not** belong here — decay data and fission
yields arrive pre-parsed from their data crates; cross sections arrive from
`njoy-outram-park-fork`. This module is the transmutation *algorithm* plus a
thin coupling layer, mirroring OpenMC's `openmc/deplete/` split.

## Provenance

* CRAM coefficients + algorithm: OpenMC `openmc/deplete/cram.py` (MIT).
* Depletion-chain structure: OpenMC `openmc/deplete/chain.py` and the
  `chain_simple.xml` regression chain (`examples/pincell_depletion/`, MIT).
* Decay data: `openmc-endf-8-depletion-lib-a`/`-b` (ENDF/B-8, via OpenMC).
* Fission yields: `fission-yields-data` (ENDF/B-VIII.0 independent yields).

This is an OUTRAM PARK translation, not the official OpenMC software.

## Units (raw `f64`, per this crate's convention)

Following the crate-wide raw-`f64` convention (see the crate `CLAUDE.md`),
depletion quantities are plain `f64` with documented units:

| Quantity | Unit |
|---|---|
| Number density `N` | atoms / (barn·cm) (or any consistent atom unit) |
| Decay constant, reaction rate | 1/s |
| Burnup-matrix coefficient | 1/s |
| Time step `dt` | s |
| One-group scalar flux | neutrons / (cm²·s) |
| One-group microscopic cross section | barn |

```rust
pub mod depletion { /* ... */ }
```

### Modules

## Module `matrix`

Dense depletion (burnup) matrix `A` with units of inverse seconds.

# What this represents

The nuclide-inventory evolution under combined radioactive decay and
neutron-induced transmutation is the linear ODE system

```text
    dN/dt = A N,          N(t) = exp(A t) N(0)
```

where `N` is the vector of nuclide number densities and `A` is the
**burnup matrix** (also called the transmutation or Bateman matrix). This
is the object OpenMC assembles in `openmc/deplete/` before handing it to a
matrix-exponential solver; see the `cram` module for the solver that
consumes it.

# Sign / index convention (read before using `set`/`add`)

`A` is indexed `A[row][col]`, and the ODE is `dN[row]/dt = sum_col A[row][col] * N[col]`:

* **Off-diagonal** `A[i][j]` (`i != j`) is the **production** rate coefficient
  of nuclide `i` from nuclide `j` — units `1/s`, always `>= 0`. For a decay
  `j -> i` with decay constant `lambda_j` and branching `b`, this is
  `+b * lambda_j`. For a neutron reaction `j -> i` with one-group microscopic
  reaction rate `r_j` (`1/s`), this is `+r_j`.
* **Diagonal** `A[j][j]` is the **total removal** rate coefficient of nuclide
  `j` — units `1/s`, always `<= 0`. It is the negative sum of every decay
  constant and every neutron-absorption rate that destroys `j`.

Assembling `A` so that each column `j`'s entries sum to zero (for a pure
rearrangement with no net loss to species outside the tracked set) makes
total-atom conservation exact; the `cram` tests exploit this.

# Units

Every stored coefficient is `1/s`. Number densities passed to [`DepletionMatrix::mat_vec`]
may be in any consistent unit (atoms, atoms/(barn·cm), …) — the matrix is
linear, so the unit of `N` simply carries through to `dN/dt`.

```rust
pub mod matrix { /* ... */ }
```

### Types

#### Struct `DepletionMatrix`

A dense `n x n` depletion (burnup) matrix `A` in units of inverse seconds,
defining the linear system `dN/dt = A N`.

Stored row-major (`A[row][col] == data[row * n + col]`). The matrix is
typically sparse in practice (each nuclide feeds only a handful of
daughters), but depletion chains handled by this crate are small (tens of
nuclides), so a dense store is simplest and keeps the CRAM linear solves
straightforward. See the module docs for the sign/index convention.

```rust
pub struct DepletionMatrix {
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
  pub fn zeros(order: usize) -> Self { /* ... */ }
  ```
  A zero `order x order` matrix (no decay, no transmutation).

- ```rust
  pub fn order(self: &Self) -> usize { /* ... */ }
  ```
  The matrix order `n` — the number of tracked nuclides.

- ```rust
  pub fn get(self: &Self, row: usize, col: usize) -> f64 { /* ... */ }
  ```
  The coefficient `A[row][col]` in `1/s`.

- ```rust
  pub fn set(self: &mut Self, row: usize, col: usize, val: f64) { /* ... */ }
  ```
  Overwrite the coefficient `A[row][col]` (`1/s`).

- ```rust
  pub fn add(self: &mut Self, row: usize, col: usize, val: f64) { /* ... */ }
  ```
  Accumulate `val` (`1/s`) into `A[row][col]`.

- ```rust
  pub fn mat_vec(self: &Self, n_vec: &[f64]) -> Vec<f64> { /* ... */ }
  ```
  The right-hand side `dN/dt = A N` for a number-density vector `n_vec`.

- ```rust
  pub fn as_slice(self: &Self) -> &[f64] { /* ... */ }
  ```
  Read-only view of the row-major coefficient buffer (`1/s`), length `n*n`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> DepletionMatrix { /* ... */ }
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
    fn eq(self: &Self, other: &DepletionMatrix) -> bool { /* ... */ }
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
## Module `cram`

Chebyshev Rational Approximation Method (CRAM) matrix-exponential solver.

Computes the depletion (Bateman) update `N(t + dt) = exp(A * dt) * N(t)`,
where `A` is the burnup matrix (`1/s`) assembled by [`DepletionMatrix`] and
`N` is the vector of nuclide number densities. CRAM is the numerically-sound
scheme for the stiff decay + transmutation system: it approximates the matrix
exponential by a rational function whose poles cluster near the negative real
axis, where the eigenvalues of a physical burnup matrix live.

# Method — Incomplete Partial Factorization (IPF) CRAM

This is a direct port of OpenMC's incomplete-partial-factorization CRAM
solver, `openmc/deplete/cram.py` (MIT-licensed), class `IPFCramSolver`. The
rational approximation of order `2k` is written as a sum over `k` complex
conjugate pole pairs:

```text
    exp(A*dt) N0  ~=  alpha0 * prod over poles [ ... ]   (IPF form)
```

evaluated by the sequential loop (`cram.py:100-105`):

```text
    A := dt * A
    y := N0
    for each pole i in 0..k:
        y += 2 * Re( alpha_i * solve( (A - theta_i * I), y ) )
    y *= alpha0
```

The IPF form is **sequential**: each pole's linear solve operates on the `y`
that the previous pole already mutated, and a single final scale by `alpha0`
closes the sweep. This is *not* the parallel-sum ("partial fraction") form —
the two are algebraically distinct approximations, and this module replicates
the IPF one exactly, as OpenMC does.

Because `y` starts real and each pole adds `2 * Re(complex)`, `y` stays real
throughout — the imaginary parts of conjugate pole pairs cancel by
construction, so only the `k` poles with positive imaginary part are stored.

The order-16 and order-48 coefficient sets are the Pusa (2016) values
embedded verbatim in `cram.py` (lines ~109-131 for order 16, ~139-201 for
order 48): M. Pusa, "Higher-Order Chebyshev Rational Approximation Method and
Application to Burnup Equations," Nucl. Sci. Eng. 182:3, 297-318 (2016),
<https://doi.org/10.13182/NSE15-26>.

# Provenance / license

Ported from OpenMC `openmc/deplete/cram.py` (MIT). The coefficients are the
Pusa (2016) values as embedded in that file. This is an OUTRAM PARK GPL-3.0
translation for education / research / V&V use — **not** official OpenMC, and
not for facility operation or licensing decisions.

# Units

* `a` — burnup matrix, every coefficient `1/s`.
* `dt_seconds` — step length, seconds.
* `n0` / return value — number densities in any consistent unit (atoms,
  atoms/(barn·cm), …); the map is linear so the unit carries through.

```rust
pub mod cram { /* ... */ }
```

### Functions

#### Function `cram16`

Order-16 IPF CRAM approximation of `N(t + dt) = exp(A * dt) * N(t)`.

# What it computes

Evolves the nuclide number densities `n0` forward by `dt_seconds` under the
burnup matrix `a` (radioactive decay + neutron transmutation), using the
order-16 (8 complex pole-pairs) Chebyshev rational approximation of the
matrix exponential in incomplete-partial-factorization form.

# Units

* `a` — burnup matrix, coefficients `1/s`. Sign convention: off-diagonal
  `A[i][j] >= 0` (production of `i` from `j`), diagonal `A[j][j] <= 0` (total
  removal of `j`); see [`DepletionMatrix`].
* `n0` — initial densities (atoms or atoms/(barn·cm), any consistent unit);
  `n0.len()` must equal `a.order()`.
* `dt_seconds` — step length in seconds, `>= 0`.

Returns the evolved densities as a fresh `Vec<f64>` of length `a.order()`.

# Accuracy

Order-16 CRAM reproduces the matrix exponential of a physical burnup matrix
to roughly 1e-5 relative even for stiff systems (eigenvalues spanning many
orders of magnitude); the tests in this module measure the errors actually
achieved. For very high accuracy use [`cram48`].

# Degenerate inputs

`dt_seconds == 0`, a zero-order matrix, or an all-zero matrix return `n0`
unchanged (`exp(0) = I`).

# Provenance

Ported from OpenMC `openmc/deplete/cram.py` (MIT), `IPFCramSolver.__call__`
(lines ~65-105) with the `c16_alpha` / `c16_theta` / `c16_alpha0`
coefficients (lines ~109-131). GPL-3.0 translation; not official OpenMC.

```rust
pub fn cram16(a: &super::matrix::DepletionMatrix, n0: &[f64], dt_seconds: f64) -> Vec<f64> { /* ... */ }
```

#### Function `cram48`

Order-48 IPF CRAM approximation of `N(t + dt) = exp(A * dt) * N(t)`.

Same contract, units, and degenerate-input handling as [`cram16`], but uses
the order-48 (24 complex pole-pairs) coefficient set, which reproduces the
matrix exponential to near machine precision (relative error ~1e-14 for the
analytic chains in the tests). Costs 3x the linear solves of [`cram16`].

# Provenance

Ported from OpenMC `openmc/deplete/cram.py` (MIT), `IPFCramSolver.__call__`
with the `c48_theta` / `c48_alpha` / `c48_alpha0` coefficients (lines
~139-201). GPL-3.0 translation; not official OpenMC.

```rust
pub fn cram48(a: &super::matrix::DepletionMatrix, n0: &[f64], dt_seconds: f64) -> Vec<f64> { /* ... */ }
```

## Module `chain`

Depletion chain — decay constants, branching, neutron-reaction targets, and
fission yields, assembled into a burnup matrix [`DepletionMatrix`].

# What this represents

A [`DepletionChain`] is the graph of a nuclide inventory's evolution under
combined **radioactive decay** and **neutron-induced transmutation**. Each
tracked nuclide carries:

* an optional half-life (`None` = stable) → decay constant
  `lambda = ln(2) / T_half` in `1/s`;
* its decay branches (daughter + branching ratio);
* its neutron reactions (`(n,gamma)`, fission, `(n,2n)`) and their targets;
* its per-fission product yields (for fissionable nuclides).

[`DepletionChain::build_matrix`] combines that structure with one-group
[`ReactionRates`] (already flux-multiplied, `1/s`) to assemble the burnup
matrix `A` in `dN/dt = A N`, following the sign/index convention documented
on [`DepletionMatrix`]. This mirrors OpenMC's `openmc/deplete/chain.py`
(`Chain.form_matrix`), reduced to the closed reaction set this crate models.

# Provenance

* **Chain structure & algorithm:** OpenMC `openmc/deplete/chain.py`
  (`Chain`, `form_matrix`), MIT.
* **The `simple()` regression chain:** transcribed verbatim from OpenMC's
  `examples/pincell_depletion/chain_simple.xml`
  (`/home/teddy0/Documents/research/openmc/examples/pincell_depletion/chain_simple.xml`),
  MIT — the 9-nuclide chain the `depletion.ipynb` notebook uses. Half-lives
  in seconds; fission yields dimensionless per fission at thermal
  (0.0253 eV).
* **Decay data (live):** `openmc-endf-8-depletion-lib-a` (Z <= 47) and
  `openmc-endf-8-depletion-lib-b` (Z >= 48), which package OpenMC's
  ENDF/B-VIII decay chain (`chain_endfb80_*.xml`) as Rust getters. Consumed
  here for the I-135 / Xe-135 half-lives via [`DepletionChain::simple_from_data`].
* **Fission yields (live):** `fission-yields-data` v0.1.4 (ENDF/B-VIII.0
  parent-independent yields). Consumed via the raw `.value` f64 field to
  avoid a `uom` major-version clash (this crate is on `uom` 0.38;
  `fission-yields-data` is on `uom` 0.37 — its quantity types are never named
  here, only their `.value` read).

This is an OUTRAM PARK translation for education / research / V&V, **not**
the official OpenMC software, and not for reactor operation.

# Known limitations of the consumed data crates (documented, not worked around)

1. **Neutron-reaction targets are unreadable from the decay libraries.** On
   `SerdeNuclideData` the `reaction` field (the `<reaction type="(n,gamma)"
   target=...>` entries) is **private** — only `name`, `half_life_seconds`,
   `decay_energy_electronvolt`, and `raw_decay_data` (the decay branches) are
   public. So neutron-reaction *targets* cannot be pulled from these libs;
   they come from the hardcoded `chain_simple.xml` transcription instead.
   Only decay constants / decay branches are cross-checkable against the libs.
2. **U-235 thermal yields are not in `fission-yields-data` 0.1.4's public
   API.** The per-nuclide accessors (`u235_thermal_fission_yield`, …) live in
   `pub(crate)` modules and are not re-exported by the crate `prelude`; the
   only publicly reachable thermal accessor is `u232_thermal_fission_yield`,
   and the general `fission_yield_linear_interpolation` needs a `uom`-0.37
   `Energy` argument this crate cannot construct without the version clash.
   So the U-234/235/238 yields used by `build_matrix` are the
   `chain_simple.xml`-transcribed values (cross-checked against the XML in
   the tests), and the *live* fission-yield consumption is demonstrated with
   the reachable `u232_thermal_fission_yield` accessor.

```rust
pub mod chain { /* ... */ }
```

### Types

#### Enum `ReactionKind`

A neutron-reaction channel type in the depletion chain.

A closed set (enum dispatch, per the workspace design rules — no trait
objects). Each variant maps to the corresponding [`super::MicroRate`] field
that [`DepletionChain::build_matrix`] reads:

* [`ReactionKind::Gamma`] ↔ `MicroRate::gamma` — radiative capture `(n,gamma)`.
* [`ReactionKind::Fission`] ↔ `MicroRate::fission` — fission (products from
  the nuclide's fission yields, not a single target).
* [`ReactionKind::TwoN`] ↔ `MicroRate::n2n` — `(n,2n)`.

```rust
pub enum ReactionKind {
    Gamma,
    Fission,
    TwoN,
}
```

##### Variants

###### `Gamma`

Radiative capture `(n,gamma)`: one atom absorbed, capture daughter produced.

###### `Fission`

Fission: the nuclide is destroyed and fission products are produced
according to its [`NuclideData`] fission yields.

###### `TwoN`

`(n,2n)`: one atom removed, the `A-1` daughter produced.

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
    fn clone(self: &Self) -> ReactionKind { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &ReactionKind) -> bool { /* ... */ }
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
#### Struct `DecayBranch`

A single radioactive-decay branch: which daughter, and with what probability.

```rust
pub struct DecayBranch {
    pub target: String,
    pub branching: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `target` | `String` | Daughter nuclide name (e.g. `"Xe135"`). If it is not a tracked nuclide,<br>the branch contributes only removal (the daughter leaves the chain). |
| `branching` | `f64` | Branching ratio (dimensionless, `0..=1`) — the fraction of decays of the<br>parent that follow this branch. |

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
    fn clone(self: &Self) -> DecayBranch { /* ... */ }
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
    fn eq(self: &Self, other: &DecayBranch) -> bool { /* ... */ }
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
#### Struct `NeutronReaction`

A single neutron-reaction channel: its kind and (for non-fission channels)
the single daughter it produces.

```rust
pub struct NeutronReaction {
    pub kind: ReactionKind,
    pub target: Option<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `kind` | `ReactionKind` | Which reaction channel this is (selects the [`super::MicroRate`] field). |
| `target` | `Option<String>` | The daughter nuclide for [`ReactionKind::Gamma`] / [`ReactionKind::TwoN`].<br><br>`None` means "no in-chain daughter" — either the reaction is fission<br>(products come from the fission yields, so this is `None`) or the target<br>leaves the tracked set (`chain_simple.xml`'s `target="Nothing"` for the<br>Gd-157 `(n,gamma)` burnable-poison sink), i.e. a pure removal with no<br>production term. |

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
    fn clone(self: &Self) -> NeutronReaction { /* ... */ }
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
    fn eq(self: &Self, other: &NeutronReaction) -> bool { /* ... */ }
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
#### Struct `NuclideData`

All chain data for one tracked nuclide.

Fields are public for read access (the struct is a plain data record); build
instances through [`DepletionChain::simple`] / [`DepletionChain::simple_from_data`]
rather than by hand so the internal name→index map stays consistent.

```rust
pub struct NuclideData {
    pub name: String,
    pub half_life_seconds: Option<f64>,
    pub decays: Vec<DecayBranch>,
    pub reactions: Vec<NeutronReaction>,
    pub fission_yields: Vec<(String, f64)>,
    pub fission_q_ev: Option<f64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | Nuclide name in GND form (`"I135"`, `"Xe135"`, `"U235"`, …). This is the<br>key used everywhere: matrix rows, [`ReactionRates`] lookups, indexing. |
| `half_life_seconds` | `Option<f64>` | Half-life in **seconds**, or `None` if the nuclide is treated as stable.<br>The decay constant is `lambda = ln(2) / half_life_seconds` (`1/s`). |
| `decays` | `Vec<DecayBranch>` | Radioactive-decay branches out of this nuclide. |
| `reactions` | `Vec<NeutronReaction>` | Neutron-reaction channels this nuclide undergoes under irradiation. Only<br>the channels listed here are read from [`ReactionRates`]; a nuclide with<br>no reactions is inert under flux (only its decay, if any, applies). |
| `fission_yields` | `Vec<(String, f64)>` | Per-fission product yields (dimensionless atoms produced per fission),<br>keyed by product name. Empty unless the nuclide has a<br>[`ReactionKind::Fission`] channel. |
| `fission_q_ev` | `Option<f64>` | Fission Q-value in **eV** (energy released per fission), transcribed from<br>`chain_simple.xml` for provenance. Not used by [`DepletionChain::build_matrix`]<br>(which assembles number-density rates, not energy), but kept so the chain<br>records what the reference specified. `None` for non-fissionable nuclides. |

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
    fn clone(self: &Self) -> NuclideData { /* ... */ }
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
    fn eq(self: &Self, other: &NuclideData) -> bool { /* ... */ }
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
#### Struct `DepletionChain`

A decay + transmutation chain that assembles a burnup matrix.

Holds the tracked nuclides in a fixed order (the matrix-row order) plus a
name→index map for O(1) lookup. Construct with [`DepletionChain::simple`]
(fast, hardcoded `chain_simple.xml` transcription) or
[`DepletionChain::simple_from_data`] (pulls the fission-product half-lives
live from the ENDF/B-VIII decay libraries and cross-checks them).

```rust
pub struct DepletionChain {
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
  pub fn simple() -> Self { /* ... */ }
  ```
  The OpenMC `chain_simple.xml` regression chain — 9 nuclides, hardcoded.

- ```rust
  pub fn simple_from_data() -> Self { /* ... */ }
  ```
  The `chain_simple.xml` chain, but with the I-135 and Xe-135 half-lives

- ```rust
  pub fn nuclide_names(self: &Self) -> Vec<&str> { /* ... */ }
  ```
  Nuclide names in matrix-row order (the order of matrix rows/columns).

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Number of tracked nuclides — the burnup matrix order.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether the chain tracks no nuclides.

- ```rust
  pub fn index_of(self: &Self, name: &str) -> Option<usize> { /* ... */ }
  ```
  The matrix row/column index of `name`, or `None` if it is not tracked.

- ```rust
  pub fn decay_constant_of(self: &Self, name: &str) -> Option<f64> { /* ... */ }
  ```
  The decay constant `lambda = ln(2) / T_half` in `1/s` for `name`, or

- ```rust
  pub fn build_matrix(self: &Self, rates: &ReactionRates) -> DepletionMatrix { /* ... */ }
  ```
  Assemble the burnup matrix `A` (`dN/dt = A N`, units `1/s`) from one-group

- ```rust
  pub fn decay_matrix(self: &Self) -> DepletionMatrix { /* ... */ }
  ```
  The pure-decay burnup matrix (no flux / no transmutation), i.e.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> DepletionChain { /* ... */ }
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
## Module `operator`

# Burnup loop (predictor / forward-Euler integrator)

Couples cross-section data to the transmutation solver to evolve a fuel
inventory over burnup steps, mirroring OpenMC's
`openmc/deplete/coupled_operator.py` + `openmc/deplete/integrators.py`
`PredictorIntegrator` (MIT) at a reduced fidelity documented below.

## Algorithm (predictor / forward Euler)

For each burnup step of length `dt`:
1. Evaluate one-group microscopic cross sections `sigma_f`, `sigma_gamma`
   for every chain nuclide from the `njoy-outram-park-fork` provider
   ([`Nuclide::from_core`] + [`Nuclide::xs_at_energy`]).
2. Normalise the scalar flux so the fission power in the modelled fuel volume
   equals the requested power (`P = flux * V * sum_j N_j sigma_f_j Q_j`).
3. Freeze the reaction rates over the step (the "predictor" assumption) and
   assemble the burnup matrix `A` from the [`DepletionChain`].
4. Advance the densities `N(t+dt) = exp(A dt) N(t)` with order-16 CRAM
   ([`super::cram::cram16`]).
5. Record the inventory and a one-group infinite-medium `k_inf` estimate.

## Fidelity caveats (honest scope — READ THIS)

This is a **one-group, infinite-medium** burnup demonstration, not a
spectrum-resolved transport-coupled depletion:

* Reaction rates use cross sections evaluated at a **single thermal energy**
  (0.0253 eV by default), not a transport-tallied multi-group flux spectrum.
  So resonance self-shielding and the fast/epithermal parts of the spectrum
  are not represented.
* `k_inf` is the one-group ratio `sum(N nu sigma_f) / sum(N sigma_a)` over the
  **chain nuclides only** (diluents/moderator/clad are omitted, since a
  one-group infinite-medium estimate ignores moderation anyway). It is a
  *relative trend* indicator, **not** comparable in absolute value to the
  notebook's continuous-energy pin-cell Monte Carlo `k`.
* A true transport-coupled `k` (matching the notebook's ~1.46 absolute
  values) needs a moderated pin-cell geometry and spectrum-averaged
  multi-group rates — tracked as a follow-up (see the V&V doc / bead).

What this loop *does* verify against the notebook is the **physically-required
trends**: U-235 depletes monotonically, the fission products (I-135, Xe-135,
Cs-135) build up, Xe-135 rises then saturates (xenon poisoning), and `k_inf`
falls. See [`crate::depletion`] and the `depletion` verification test.

A real Monte Carlo `k` on the evolved actinide inventory is available via
[`mc_keff_of_actinide_sphere`] to demonstrate the transport path is wired.

```rust
pub mod operator { /* ... */ }
```

### Types

#### Struct `BurnupSettings`

Settings for a [`deplete_predictor`] burnup run.

Defaults mirror the `depletion.ipynb` notebook case: a 4.25%-enriched UO₂ pin
at 174 W (unit height), six 30-day steps.

```rust
pub struct BurnupSettings {
    pub power_watts: f64,
    pub fuel_volume_cm3: f64,
    pub step_days: f64,
    pub n_steps: usize,
    pub temperature_k: f64,
    pub one_group_energy_ev: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `power_watts` | `f64` | Total thermal fission power in the modelled fuel volume \[W\]. |
| `fuel_volume_cm3` | `f64` | Modelled fuel volume \[cm³\] the power is produced in. |
| `step_days` | `f64` | Length of each burnup step \[days\]. |
| `n_steps` | `usize` | Number of burnup steps. |
| `temperature_k` | `f64` | Data/lookup temperature \[K\] for the cross-section evaluation. |
| `one_group_energy_ev` | `f64` | One-group energy \[eV\] at which cross sections are evaluated<br>(0.0253 eV = 2200 m/s thermal point by default). |

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
    fn clone(self: &Self) -> BurnupSettings { /* ... */ }
    ```

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
#### Struct `BurnupStep`

The inventory and reactor state recorded at one burnup step.

```rust
pub struct BurnupStep {
    pub step: usize,
    pub time_days: f64,
    pub flux: f64,
    pub k_inf: f64,
    pub densities: Vec<(String, f64)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `step` | `usize` | Step index (0 = beginning of life, before any depletion). |
| `time_days` | `f64` | Cumulative burnup time at this point \[days\]. |
| `flux` | `f64` | One-group scalar flux over the step just taken \[neutrons/(cm²·s)\]<br>(0.0 at the beginning-of-life record). |
| `k_inf` | `f64` | One-group infinite-medium multiplication factor `k_inf` (chain nuclides<br>only — a relative trend indicator, see the module fidelity caveats). |
| `densities` | `Vec<(String, f64)>` | Nuclide inventory, `(name, atom_density)` in atoms/(barn·cm), in chain order. |

##### Implementations

###### Methods

- ```rust
  pub fn density(self: &Self, nuclide: &str) -> f64 { /* ... */ }
  ```
  The atom density \[atoms/(barn·cm)\] of `nuclide` at this step, or 0.0 if

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> BurnupStep { /* ... */ }
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
#### Struct `BurnupResult`

The full trajectory of a [`deplete_predictor`] run: one [`BurnupStep`] per
recorded point (beginning-of-life plus one per burnup step).

```rust
pub struct BurnupResult {
    pub steps: Vec<BurnupStep>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `steps` | `Vec<BurnupStep>` | Recorded steps, `steps[0]` is beginning-of-life. |

##### Implementations

###### Methods

- ```rust
  pub fn bol(self: &Self) -> &BurnupStep { /* ... */ }
  ```
  The beginning-of-life (fresh fuel) record.

- ```rust
  pub fn eol(self: &Self) -> &BurnupStep { /* ... */ }
  ```
  The end-of-life (final) record.

- ```rust
  pub fn history(self: &Self, nuclide: &str) -> Vec<(f64, f64)> { /* ... */ }
  ```
  The time history `(time_days, atom_density)` of one nuclide across every

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> BurnupResult { /* ... */ }
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

#### Function `deplete_predictor`

Run a predictor (forward-Euler) burnup calculation on `chain`, starting from
the inventory `initial` (`(nuclide_name, atom_density)` in atoms/(barn·cm)).

Nuclides in `initial` that are not in `chain` are ignored; chain nuclides
absent from `initial` start at zero density. Returns one [`BurnupStep`] per
recorded point (beginning-of-life plus `settings.n_steps` steps).

This is the honest, one-group demonstration described in the module docs; the
transmutation step itself (CRAM) is verified to analytic accuracy in
[`super::cram`], and the inventory *trends* are checked against the notebook
in the `depletion` verification test.

```rust
pub fn deplete_predictor(chain: &super::chain::DepletionChain, initial: &[(String, f64)], settings: &BurnupSettings) -> BurnupResult { /* ... */ }
```

#### Function `mc_keff_of_actinide_sphere`

Run a real Monte Carlo `k_eff` power iteration on a **bare sphere** of the
given actinide/fission-product inventory — a demonstration that the evolved
depletion inventory feeds straight back into the transport kernel.

`inventory` is `(nuclide_name, atom_density)` in atoms/(barn·cm). Only
nuclides the CORE provider carries are included. `radius_cm` sets the sphere
size. **This is a fast-spectrum bare sphere**, so the absolute `k` is far
below a moderated pin cell's — it is not comparable to the notebook's `k`;
its value is that it exercises the genuine MC transport path on a depleted
inventory. Returns the [`KeffResult`].

```rust
pub fn mc_keff_of_actinide_sphere(inventory: &[(String, f64)], radius_cm: f64, settings: &crate::physics::keff::KeffSettings) -> crate::physics::keff::KeffResult { /* ... */ }
```

### Types

#### Struct `MicroRate`

One-group microscopic **reaction rates** for a single nuclide, in `1/s`.

Each field is already flux-multiplied: `rate = flux * micro_xs`, i.e. the
per-atom probability per second of that reaction. These are the coefficients
[`chain::DepletionChain::build_matrix`] places into the burnup matrix.

Units: `1/s`. A value of `0.0` means the channel is absent or unresolved.

```rust
pub struct MicroRate {
    pub gamma: f64,
    pub fission: f64,
    pub n2n: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `gamma` | `f64` | Radiative-capture `(n,gamma)` rate `1/s` — drives the capture daughter. |
| `fission` | `f64` | Fission rate `1/s` — removes the actinide and produces fission products<br>via the chain's fission yields. |
| `n2n` | `f64` | `(n,2n)` rate `1/s` — removes one atom, producing the A-1 daughter. |

##### Implementations

###### Methods

- ```rust
  pub fn total_removal(self: &Self) -> f64 { /* ... */ }
  ```
  Total neutron-absorption-driven **removal** rate `1/s` for this nuclide

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> MicroRate { /* ... */ }
    ```

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
    fn default() -> MicroRate { /* ... */ }
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
    fn eq(self: &Self, other: &MicroRate) -> bool { /* ... */ }
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
#### Struct `ReactionRates`

One-group reaction rates for a whole material, keyed by nuclide name
(`"U235"`, `"Xe135"`, …), plus the scalar flux they were derived from.

This is the transport → transmutation hand-off: the burnup operator fills it
from a transport solve (or a representative one-group estimate), and
[`chain::DepletionChain::build_matrix`] reads it to assemble `A`.

Units: `flux` in neutrons/(cm²·s); every [`MicroRate`] field in `1/s`.

```rust
pub struct ReactionRates {
    pub flux: f64,
    pub micro_rates: std::collections::HashMap<String, MicroRate>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `flux` | `f64` | One-group scalar flux `neutrons/(cm²·s)` the rates were normalised to. |
| `micro_rates` | `std::collections::HashMap<String, MicroRate>` | Per-nuclide one-group reaction rates (`1/s`), keyed by nuclide name. |

##### Implementations

###### Methods

- ```rust
  pub fn zero() -> Self { /* ... */ }
  ```
  An empty rate set at zero flux (pure-decay depletion — no transmutation).

- ```rust
  pub fn rate_for(self: &Self, nuclide: &str) -> MicroRate { /* ... */ }
  ```
  The reaction rates for `nuclide`, or the all-zero [`MicroRate`] if the

- ```rust
  pub fn set(self: &mut Self, nuclide: &str, rate: MicroRate) { /* ... */ }
  ```
  Insert or overwrite the reaction rates for `nuclide`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> ReactionRates { /* ... */ }
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
    fn default() -> ReactionRates { /* ... */ }
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
### Re-exports

#### Re-export `DepletionMatrix`

```rust
pub use matrix::DepletionMatrix;
```

## Module `gpu`

Optional headless GPU compute (wgpu) for embarrassingly-parallel MC kernels.
Desktop gets the real path; Android gets a CPU-only shim. GPU is acceleration
only — the CPU raw-`f64` path stays the trusted, deterministic reference.
Optional GPU compute for the embarrassingly-parallel Monte Carlo kernels.

This module provides *headless* (no window, no surface) GPU acceleration via
[`wgpu`] for the branch-light, data-parallel sub-kernels of neutron transport
— starting with **batched pointwise cross-section interpolation** (see
[`xs_interp`]). It is **acceleration only**; correctness never depends on it.

## Non-negotiable contract (read before using this module)

1. **Compiles always, runs on CPU when there is no GPU.** [`GpuContext`] and
   [`probe`] are the desktop (real `wgpu`) path; on Android — where `wgpu` is
   target-gated out in `Cargo.toml` — a CPU-only shim gives [`probe`] the same
   `-> Option<GpuContext>` signature returning `None`, so call sites compile
   unchanged. On desktop/CI, [`probe`] returns `None` whenever no usable GPU
   adapter exists (headless servers, no Vulkan loader); callers **must** treat
   `None` as "run the CPU path", never as an error.
2. **CPU is the trusted / deterministic reference.** The transport loop is raw
   `f64` (see this crate's `CLAUDE.md`); the GPU path runs `f32` and its
   floating-point reduction/rounding order will **not** bit-match the CPU. So
   anything feeding V&V or a solver stays on the CPU path. GPU results are only
   ever compared to CPU *within a tolerance*, never trusted as the reference.
3. **No new third-party dependency.** `wgpu`'s `request_adapter` /
   `request_device` return futures; rather than pull in an async runtime this
   module hand-rolls a tiny pure-`std` [`block_on`] (a `Wake`-based thread
   park, the same shape as `pollster::block_on`). Buffer read-back does not
   need it — it uses `Device::poll(PollType::wait_indefinitely())`.

## What belongs here / what does not

- **Belongs:** headless compute contexts, WGSL kernels for embarrassingly
  parallel MC sub-kernels (XS interpolation, majorant evaluation, batched
  free-flight sampling), dense-grid material-total tabulations that reuse
  those kernels ([`union_grid`]), and their CPU reference + GPU-vs-CPU
  agreement tests.
- **Does not:** the history-based transport loop itself (branchy, not GPU
  friendly), any windowing/GUI (out of scope for the library; Android-hostile),
  or anything that would make a plain `cargo build` require a GPU at runtime.

```rust
pub mod gpu { /* ... */ }
```

### Modules

## Module `xs_interp`

Energy-grid cross-section interpolation, CPU reference + GPU compute path.

(Module-level prose lives in the parent `crate::gpu` `mod.rs`; the items in
this file each carry their own `///` documentation as required by the
human-interface-layer rule.)

```rust
pub mod xs_interp { /* ... */ }
```

### Functions

#### Function `interp_xs_cpu`

Linearly interpolate a tabulated cross section onto a set of query energies.

This is the **trusted, deterministic f64 reference path** against which the
GPU path ([`interp_xs_gpu`]) is judged. It mirrors the energy-grid bracket +
linear interpolation performed by OpenMC's `Nuclide::calculate_xs`
(`/home/teddy0/Documents/research/openmc/src/nuclide.cpp:716-760`).

# Physical meaning
- `grid`   — monotonically increasing energy grid points, units **eV**.
- `sigma`  — the cross section tabulated on `grid`, units **barn**. Must be
  the same length as `grid` (`sigma[i]` is the cross section at `grid[i]`).
- `queries` — the energies (eV) at which the cross section is wanted.
- returns — one interpolated cross section (barn) per query, in order.

# Bracketing / extrapolation (mirrors nuclide.cpp:716-740)
For each query energy `q`, with `n = grid.len()`:
- `q < grid[0]`         → `i_grid = 0`      (clamp: use the first interval,
  `f` becomes negative so this linearly extrapolates below the grid — this
  matches OpenMC, where the total XS grid always spans the problem range so
  this branch is effectively a clamp to the first tabulated interval).
- `q > grid[n-1]`       → `i_grid = n-2`    (use the last interval; `f > 1`
  linearly extrapolates above the grid, again mirroring OpenMC).
- otherwise             → `i_grid` such that `grid[i_grid] <= q < grid[i_grid+1]`
  (lower-bound search).

A rare duplicate-energy guard (`grid[i] == grid[i+1]`) advances one index,
exactly as `nuclide.cpp:735`. The interpolation factor is
`f = (q - grid[i]) / (grid[i+1] - grid[i])` and the result is
`(1 - f) * sigma[i] + f * sigma[i+1]`.

# Degenerate grids
- `n == 0` → every query returns `0.0`.
- `n == 1` → every query returns `sigma[0]` (nothing to interpolate).

# Preconditions
`grid` must be sorted ascending and `grid.len() == sigma.len()`. Violating
these is not checked (hot path); results are then unspecified.

```rust
pub fn interp_xs_cpu(grid: &[f64], sigma: &[f64], queries: &[f64]) -> Vec<f64> { /* ... */ }
```

#### Function `interp_xs_gpu`

**Attributes:**

- `Other("#[attr = CfgTrace([Not(NameValue { name: \"target_os\", value: Some(\"android\"), span: crates/outram-mc-libs/src/gpu/xs_interp.rs:173:11: 173:32 (#0) }, crates/outram-mc-libs/src/gpu/xs_interp.rs:173:10: 173:33 (#0))])]")`

Linearly interpolate a tabulated cross section onto query energies **on the
GPU**, via a WGSL compute shader. This is the f32 accelerated counterpart of
the f64 reference [`interp_xs_cpu`]; the two are held to agreement by the
V&V test in this module.

# Physical meaning (identical to [`interp_xs_cpu`], but single precision)
- `grid`    — ascending energy grid points, units **eV** (f32).
- `sigma`   — cross section tabulated on `grid`, units **barn** (f32).
- `queries` — energies (eV) at which the cross section is wanted (f32).
- returns   — one interpolated cross section (barn) per query, in order.

# Preconditions
- `grid.len() == sigma.len()`.
- `grid.len() >= 2` (the shader indexes `i_grid + 1`; a degenerate 0/1-point
  grid is not supported on the GPU path — use [`interp_xs_cpu`] for those).
- `grid` sorted ascending.

An empty `queries` returns an empty `Vec` without touching the GPU.

# How it works
Uploads `grid`, `sigma`, `queries` as read-only storage buffers, a
`[n_grid, n_query, 0, 0]` uniform params block, dispatches
`ceil(n_query / 64)` workgroups of the `main` entry point, copies the result
storage buffer into a mappable staging buffer, blocks on
`device.poll(PollType::wait_indefinitely())`, and reads the result back.

```rust
pub fn interp_xs_gpu(ctx: &crate::gpu::GpuContext, grid: &[f32], sigma: &[f32], queries: &[f32]) -> Vec<f32> { /* ... */ }
```

## Module `surface_distance`

Ray–surface distance for all 15 [`SurfaceKind`] variants, as a batched GPU
kernel with a trusted CPU reference.

(Module-level prose lives in the parent `crate::gpu` `mod.rs`; every item in
this file carries its own `///` documentation per the human-interface rule.)

# What this is (op-9s8.2 — the geometric foundation for GPU CSG transport)

[`crate::geometry::surface`] holds the **trusted `f64`** ray-intersection for
every CSG surface (planes, sphere, axis cylinders, axis cones, the general
quadric, and the three tori). This module provides the **`f32`** counterpart
in two forms that must agree:

- [`surface_distance_cpu_f32`] — a scalar `f32` mirror of the `f64` logic,
  structured so the WGSL kernel is a line-by-line translation of it. It is the
  reference the GPU path is judged against, and is itself judged against the
  `f64` [`SurfaceKind::distance`] by the tests in this module.
- [`surface_distance_gpu`] — the WGSL compute path
  (`shaders/surface_distance.wgsl`), desktop-only (wgpu is target-gated off
  Android), which evaluates the same distance for a batch of query rays.

# Encoding

A surface is flattened to a `u32` tag (its [`SurfaceKind`] variant order,
`0..=14`) plus [`SURF_STRIDE`] `f32` coefficients (see [`encode_surfaces`]).
A batch of query rays is `(surface index, coincident flag, origin, direction)`.
The result is one smallest-positive distance per query, with [`MISS`] standing
in for `f64::INFINITY` (no crossing) so the `f32` buffer carries no infinities.

# Precision & trust

Per the crate `CLAUDE.md`, `f64` on the CPU is the trusted reference; this
`f32` path is an accelerator, compared to the reference only within a
tolerance, never trusted as the reference. The distance **algorithm** for
each surface is ported from the OpenMC-mirrored `f64` code in
`geometry/surface.rs` (which cites the upstream `src/surface.cpp` sites); this
module re-expresses it in `f32`, it does not re-derive any physics.

```rust
pub mod surface_distance { /* ... */ }
```

### Types

#### Struct `EncodedSurfaces`

A batch of surfaces flattened for GPU upload (and the CPU mirror).

`tags[i]` is surface `i`'s [`SurfaceKind`] variant order (`0..=14`) and
`coeffs[i*SURF_STRIDE .. i*SURF_STRIDE + SURF_STRIDE]` its coefficients. See
[`encode_surfaces`] for the per-kind layout.

```rust
pub struct EncodedSurfaces {
    pub tags: Vec<u32>,
    pub coeffs: Vec<f32>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `tags` | `Vec<u32>` | One variant tag (`0..=14`) per surface. |
| `coeffs` | `Vec<f32>` | `SURF_STRIDE` coefficients per surface, row-major. |

##### Implementations

###### Methods

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Number of surfaces encoded.

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether no surfaces are encoded.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
#### Struct `SurfaceQuery`

One query ray for [`surface_distance_cpu_f32`] / [`surface_distance_gpu`].

```rust
pub struct SurfaceQuery {
    pub surface: u32,
    pub coincident: bool,
    pub r: crate::geometry::position::Position,
    pub u: crate::geometry::position::Direction,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `surface` | `u32` | Index into the [`EncodedSurfaces`] to test against. |
| `coincident` | `bool` | `true` when the ray origin sits on the surface (post-crossing); forces the<br>constant term to zero so round-off cannot report a spurious `d ≈ 0`. |
| `r` | `crate::geometry::position::Position` | Ray origin, cm. |
| `u` | `crate::geometry::position::Direction` | Ray direction (unit), dimensionless. |

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
    fn clone(self: &Self) -> SurfaceQuery { /* ... */ }
    ```

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
### Functions

#### Function `encode_surfaces`

Flatten a slice of [`SurfaceKind`] into the [`EncodedSurfaces`] GPU layout.

The tag is the variant's declaration order in [`SurfaceKind`]
(`XPlane=0, YPlane=1, ZPlane=2, Plane=3, Sphere=4, XCylinder=5, YCylinder=6,
ZCylinder=7, XCone=8, YCone=9, ZCone=10, Quadric=11, XTorus=12, YTorus=13,
ZTorus=14`). Coefficients are stored in the order each struct declares its
fields:

- planes: `XPlane→[x0]`, `YPlane→[y0]`, `ZPlane→[z0]`, `Plane→[a,b,c,d]`
- `Sphere→[x0,y0,z0,r]`
- `XCylinder→[y0,z0,r]`, `YCylinder→[x0,z0,r]`, `ZCylinder→[x0,y0,r]`
- cones `{X,Y,Z}Cone→[x0,y0,z0,r_sq]`
- `Quadric→[a,b,c,d,e,f,g,h,j,k]`
- tori `{X,Y,Z}Torus→[x0,y0,z0,a,b,c]`

The `bc` (boundary condition) field is intentionally not encoded — this kernel
computes geometric distance only.

```rust
pub fn encode_surfaces(surfaces: &[crate::geometry::surface::SurfaceKind]) -> EncodedSurfaces { /* ... */ }
```

#### Function `surface_distance_cpu_f32`

CPU `f32` reference for a batch of ray–surface distance queries.

Evaluates [`surface_distance_one_f32`] for each [`SurfaceQuery`], returning one
distance per query in order ([`MISS`] where the ray does not cross). This is
both the deterministic CPU path and the reference the GPU kernel is judged
against; it is itself judged against the trusted `f64` [`SurfaceKind::distance`]
by the tests in this module.

```rust
pub fn surface_distance_cpu_f32(encoded: &EncodedSurfaces, queries: &[SurfaceQuery]) -> Vec<f32> { /* ... */ }
```

#### Function `surface_distance_gpu`

**Attributes:**

- `Other("#[attr = CfgTrace([Not(NameValue { name: \"target_os\", value: Some(\"android\"), span: crates/outram-mc-libs/src/gpu/surface_distance.rs:777:11: 777:32 (#0) }, crates/outram-mc-libs/src/gpu/surface_distance.rs:777:10: 777:33 (#0))])]")`

Compute ray–surface distances for a batch of queries **on the GPU**, via the
`shaders/surface_distance.wgsl` compute shader. The `f32` accelerated twin of
[`surface_distance_cpu_f32`]; the two are held to agreement by the V&V test in
this module.

# Parameters
- `ctx` — a live [`crate::gpu::GpuContext`] from [`crate::gpu::probe`].
- `encoded` — surfaces flattened by [`encode_surfaces`].
- `queries` — the rays to test; each names a surface index, a coincident flag,
  an origin (cm) and a unit direction.

Returns one distance per query in order ([`MISS`] on a miss). An empty
`queries` returns an empty `Vec` without touching the GPU.

# How it works
Uploads the tag, coeff, per-query surface-index, per-query flag, origin, and
direction arrays as read-only storage buffers plus an `[n_surf, n_query, 0, 0]`
uniform, dispatches `ceil(n_query / 64)` workgroups of `main`, copies the
result buffer to a mappable staging buffer, blocks on
`device.poll(PollType::wait_indefinitely())`, and reads it back.

```rust
pub fn surface_distance_gpu(ctx: &crate::gpu::GpuContext, encoded: &EncodedSurfaces, queries: &[SurfaceQuery]) -> Vec<f32> { /* ... */ }
```

### Constants and Statics

#### Constant `SURF_STRIDE`

Number of `f32` coefficients stored per surface in the flat encoding.

Sized for the widest surface, the general [`SurfaceKind::Quadric`] with 10
coefficients (`a,b,c,d,e,f,g,h,j,k`); every other kind uses a prefix of the
stride and leaves the tail unused.

```rust
pub const SURF_STRIDE: usize = 12;
```

#### Constant `MISS`

Sentinel distance meaning "ray does not cross this surface", the `f32` stand-in
for the `f64::INFINITY` returned by [`SurfaceKind::distance`]. Chosen far
beyond any physical cm distance yet well inside `f32::MAX`, so it survives
arithmetic and buffer round-trips without becoming a true infinity.

```rust
pub const MISS: f32 = 1.0e30;
```

## Module `union_grid`

Dense log-spaced tabulation of a material's macroscopic total cross section,
with a batched CPU-reference / GPU-accelerated lookup.

The history-based transport loop (`crate::physics::keff`) queries
[`crate::material::Material::macro_xs_total`] one energy at a time. This
module takes that *same* macroscopic total Sigma_t(E) \[cm^-1\] and
**pre-tabulates** it on a fixed, dense, log-spaced energy grid so that a
whole batch of query energies can be looked up at once — on the CPU (the
trusted `f64` reference) or on the GPU (`f32` acceleration), reusing the
energy-grid interpolation kernel already in [`crate::gpu::xs_interp`].

## Two constructors: dense-log resample vs native-breakpoint union

[`UnionTotalXs::tabulate`] is a **dense LOG-SPACED resampling** of the
macroscopic total: the grid points are `n_points` energies uniformly spaced
in `log10(E)`, and Sigma_t is evaluated by calling `Material::macro_xs_total`
at each of them. It is **not** a union of the constituent nuclides' native
ENDF energy breakpoints, so it can smear or skip narrow resonances that fall
between its log-spaced nodes; its fidelity is bounded by `n_points`.

[`UnionTotalXs::tabulate_native`] (beads op-u6s.7) builds the genuine
**native-breakpoint union grid**: it gathers every energy node the underlying
nuclide data actually tabulates (via [`crate::material::nuclide::Nuclide::native_energy_grid`]
— reconstructed section grids for the HIGH tier; WMP window edges + fast-group
bounds for the LOW/analytic tier), merges them with a log-spaced backbone
floor (so it is never coarser than the equal-size dense grid), and
deduplicates into one sorted strictly-increasing array. This lands grid points
on real data features and is measurably at least as accurate as the equal-size
log grid (see that method's V&V test). Both constructors produce the same
struct and feed the same [`lookup_cpu`](UnionTotalXs::lookup_cpu) /
[`lookup_gpu`](UnionTotalXs::lookup_gpu) binary-search lookups.

Either way this tabulation is **acceleration-only**: judged against a direct
`macro_xs_total` evaluation, never trusted above it. Do not treat it as a
lossless replacement for calling `macro_xs_total` directly.

(Module-level "what belongs here" prose lives in the parent `crate::gpu`
`mod.rs`; the items in this file each carry their own `///` documentation as
required by the human-interface-layer rule.)

```rust
pub mod union_grid { /* ... */ }
```

### Types

#### Struct `UnionTotalXs`

A material's macroscopic total cross section Sigma_t(E) \[cm^-1\] tabulated
on a dense log-spaced energy grid, ready for batched lookup.

# Physical meaning
- `grid` — ascending energy grid points, units **eV**. Log-spaced between the
  `e_min_ev` / `e_max_ev` passed to [`UnionTotalXs::tabulate`].
- `sigma_total` — the **macroscopic** total cross section Sigma_t \[cm^-1\]
  at each grid point, i.e. `sum_i N_i * sigma_t,i(E)` over the material's
  nuclides (`N_i` in atoms/barn-cm, `sigma` in barn ⇒ product in cm^-1). Same
  length as `grid`; `sigma_total[i]` is Sigma_t at `grid[i]`.
- `temperature_k` — the material temperature \[K\] used for every Doppler
  lookup during tabulation (copied from `Material::temperature`).

# Fidelity caveat (read the module docs)
This is a *dense log-spaced resampling*, not a native-breakpoint union of the
nuclides' ENDF energy grids. Narrow resonances between grid points can be
smeared or missed; increase `n_points` to reduce the error. It exists to
accelerate batched Sigma_t lookups, never to replace the direct
[`Material::macro_xs_total`] reference.

```rust
pub struct UnionTotalXs {
    pub grid: Vec<f64>,
    pub sigma_total: Vec<f64>,
    pub temperature_k: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `grid` | `Vec<f64>` | Ascending energy grid \[eV\] (log-spaced). |
| `sigma_total` | `Vec<f64>` | Macroscopic total Sigma_t \[cm^-1\] at each grid point (same length as `grid`). |
| `temperature_k` | `f64` | Material temperature \[K\] used for the tabulation. |

##### Implementations

###### Methods

- ```rust
  pub fn tabulate(material: &Material, nuclides: &[Nuclide], e_min_ev: f64, e_max_ev: f64, n_points: usize) -> Self { /* ... */ }
  ```
  Tabulate [`Material::macro_xs_total`] on `n_points` **log-spaced**

- ```rust
  pub fn tabulate_native(material: &Material, nuclides: &[Nuclide], e_min_ev: f64, e_max_ev: f64, backbone_points: usize) -> Self { /* ... */ }
  ```
  Tabulate the material's macroscopic total Sigma_t \[cm^-1\] on the

- ```rust
  pub fn lookup_cpu(self: &Self, queries_ev: &[f64]) -> Vec<f64> { /* ... */ }
  ```
  Batched **CPU reference** lookup: the macroscopic total Sigma_t \[cm^-1\]

- ```rust
  pub fn lookup_gpu(self: &Self, ctx: &crate::gpu::GpuContext, queries_ev: &[f64]) -> Vec<f32> { /* ... */ }
  ```
  Batched **GPU-accelerated** lookup (`f32`): the same macroscopic total

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
## Module `batched_flight`

Batched next-event GPU free-flight kernel, CPU mirror + GPU compute path.

This module advances a whole **batch** of live neutrons through **one flight
(one event) at a time** in parallel. Instead of one `Sigma_t` lookup per GPU
launch, the batch stays resident in GPU buffers and every dispatch advances
each live particle: draw a per-particle random number (64-bit LCG advanced on
the GPU), look up macroscopic total cross section `Sigma_t` at the particle
energy (binary search + linear interpolation on a shared union grid), sample
the distance to collision `d_col = -ln(xi) / Sigma_t`, compute the distance
to the bounding sphere, stream to the nearer of the two, and flag the outcome
(collided vs leaked). The branchy collision physics (which nuclide, reaction,
secondary energy/angle) stays on the CPU caller — it is **not** done here.

# Precision contract (read `crate::gpu` `mod.rs` first)
The trusted, deterministic reference is the raw-`f64` CPU transport loop. This
kernel is `f32` acceleration only. Within this module,
[`advance_flight_cpu_mirror`] runs the **same `f32` arithmetic path** as the
GPU shader, so it is the bit-level reference for the GPU kernel's *logic*
(the two are held to agreement by the V&V test in this file).

# RNG divergence (documented, intentional)
The 64-bit LCG **integer state advance** is bit-exact vs the CPU LCG
(`crate::rng::lcg`): after one flight the returned `(rng_hi, rng_lo)` equal
`future_seed(1, seed)` for every particle. The **f32 uniform value** used in
the flight is derived from the top 24 bits of the advanced state and does
**not** match the CPU `f64` `prn` value — that is the accepted f32
acceleration divergence. The CPU single-thread path stays the trusted,
bit-reproducible reference.

Items whose names end in `_gpu` are `#[cfg(not(target_os = "android"))]`
(wgpu is target-gated off Android); the SoA structs and the CPU mirror build
on every target.

```rust
pub mod batched_flight { /* ... */ }
```

### Types

#### Struct `FlightBatch`

Structure-of-Arrays batch of live neutrons resident for the GPU flight kernel.

All vectors are `f32`/`u32` acceleration state (the trusted transport loop is
`f64` — see the module docs). For a batch of `N` particles:
- `pos`    — length `3N`, interleaved `x, y, z` per particle, units **cm**.
  READ + WRITE: a collided particle's position is updated to its collision
  site; a leaked particle's position is left unchanged.
- `dir`    — length `3N`, interleaved `u, v, w` unit direction (dimensionless,
  `|(u,v,w)| = 1`). READ only — a flight does not change direction.
- `energy` — length `N`, particle energy, units **eV**. READ only.
- `rng_hi` — length `N`, the high 32 bits of each particle's 64-bit LCG seed.
  READ + WRITE (advanced by exactly one LCG step per flight).
- `rng_lo` — length `N`, the low 32 bits of each particle's 64-bit LCG seed
  (`seed = (rng_hi << 32) | rng_lo`). READ + WRITE.

All five lengths must be consistent (`pos.len() == dir.len() == 3 *
energy.len()`, and `rng_hi.len() == rng_lo.len() == energy.len()`); `N` is
taken from `energy.len()`.

```rust
pub struct FlightBatch {
    pub pos: Vec<f32>,
    pub dir: Vec<f32>,
    pub energy: Vec<f32>,
    pub rng_hi: Vec<u32>,
    pub rng_lo: Vec<u32>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pos` | `Vec<f32>` | Interleaved positions `x,y,z` per particle (cm), length `3N`. READ+WRITE. |
| `dir` | `Vec<f32>` | Interleaved unit directions `u,v,w` per particle, length `3N`. READ only. |
| `energy` | `Vec<f32>` | Per-particle energy (eV), length `N`. READ only. |
| `rng_hi` | `Vec<u32>` | High 32 bits of each particle's 64-bit LCG seed, length `N`. READ+WRITE. |
| `rng_lo` | `Vec<u32>` | Low 32 bits of each particle's 64-bit LCG seed, length `N`. READ+WRITE. |

##### Implementations

###### Methods

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Number of particles `N` in the batch (taken from `energy.len()`).

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether the batch is empty (`N == 0`).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
#### Struct `FlightSphere`

Bounding sphere for the flight — the leakage surface of the flight kernel.

`(x0, y0, z0)` is the center (cm) and `r` the radius (cm). A particle that
would fly past this sphere before its sampled collision distance is flagged
[`FlightOutcome::Leaked`]. All fields are `f32` to match the GPU path.

```rust
pub struct FlightSphere {
    pub x0: f32,
    pub y0: f32,
    pub z0: f32,
    pub r: f32,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x0` | `f32` | Sphere center x (cm). |
| `y0` | `f32` | Sphere center y (cm). |
| `z0` | `f32` | Sphere center z (cm). |
| `r` | `f32` | Sphere radius (cm), must be > 0 for a meaningful boundary. |

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
    fn clone(self: &Self) -> FlightSphere { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &FlightSphere) -> bool { /* ... */ }
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
#### Enum `FlightOutcome`

Per-particle outcome of one flight.

An enum (not a `bool`) so downstream `match` sites stay exhaustive if more
outcomes are ever added (per the workspace "enums over trait objects /
exhaustive dispatch" rule). Maps to the shader's `outcome` codes:
`0 = Leaked`, `1 = Collided`.

```rust
pub enum FlightOutcome {
    Leaked,
    Collided,
}
```

##### Variants

###### `Leaked`

The particle reached the bounding sphere before colliding (dead here).
Its position is left unchanged.

###### `Collided`

The particle collided inside the sphere. Its position in the batch has
been advanced to the collision site; the caller does the collision physics.

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
    fn clone(self: &Self) -> FlightOutcome { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &FlightOutcome) -> bool { /* ... */ }
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

#### Function `advance_flight_cpu_mirror`

Advance every particle in `batch` through **one flight** on the CPU, using
the **exact same `f32` arithmetic path** as the GPU shader.

This is the bit-level reference implementation for the GPU kernel's logic:
same emulated one-step LCG advance (via native `u64`, then split), same
top-24-bit f32 uniform, same f32 grid search / interpolation, same f32 sphere
distance. It is **unconditional** (builds and runs on Android too, where the
GPU path is absent).

# Physical meaning
- `grid`   — ascending union energy grid (eV), `grid.len() >= 2`, f32.
- `sigma`  — macroscopic total cross section `Sigma_t` (cm^-1) tabulated on
  `grid`, same length as `grid`, f32.
- `batch`  — the SoA particle state ([`FlightBatch`]); `pos` and `rng_hi/lo`
  are updated in place (RNG advanced one step for every particle; `pos`
  advanced to the collision site for collided particles).
- `sphere` — the bounding [`FlightSphere`] (cm).
- returns  — one [`FlightOutcome`] per particle, in index order.

# Behaviour per particle
Advance RNG one step → `xi`; interpolate `Sigma_t` at `energy[i]`; if
`Sigma_t <= 0` → `Leaked`; else `d_col = -ln(xi)/Sigma_t`; compute the sphere
distance `d_bound`; if `d_col >= d_bound` → `Leaked` (pos unchanged), else
stream `pos += dir * d_col` and mark `Collided`. An empty batch returns an
empty `Vec`.

# Preconditions
`grid.len() == sigma.len() >= 2`, `grid` sorted ascending; the batch length
invariants in [`FlightBatch`]. Violations are not checked (hot path).

```rust
pub fn advance_flight_cpu_mirror(grid: &[f32], sigma: &[f32], batch: &mut FlightBatch, sphere: FlightSphere) -> Vec<FlightOutcome> { /* ... */ }
```

#### Function `advance_flight_gpu`

**Attributes:**

- `Other("#[attr = CfgTrace([Not(NameValue { name: \"target_os\", value: Some(\"android\"), span: crates/outram-mc-libs/src/gpu/batched_flight.rs:396:11: 396:32 (#0) }, crates/outram-mc-libs/src/gpu/batched_flight.rs:396:10: 396:33 (#0))])]")`

Advance every particle in `batch` through **one flight on the GPU**, via the
WGSL compute shader `shaders/batched_flight.wgsl`. This is the `f32`
accelerated counterpart of [`advance_flight_cpu_mirror`]; the two are held to
agreement by the V&V test in this module.

# Physical meaning (identical to [`advance_flight_cpu_mirror`])
- `grid`   — ascending union energy grid (eV), `grid.len() >= 2`, f32.
- `sigma`  — macroscopic total cross section `Sigma_t` (cm^-1) on `grid`, f32,
  same length as `grid`.
- `batch`  — SoA particle state; on return, `batch.pos` (collision sites) and
  `batch.rng_hi`/`batch.rng_lo` (advanced one LCG step) are updated in place.
- `sphere` — the bounding [`FlightSphere`] (cm).
- returns  — one [`FlightOutcome`] per particle, in index order.

# How it works
Uploads `grid`, `sigma`, `dir`, `energy` as read-only storage buffers, `pos`,
`rng_hi`, `rng_lo`, `outcome` as read-write storage buffers (with `COPY_SRC`
so they can be read back), and a 32-byte `Params` uniform. Dispatches
`ceil(N / 64)` workgroups of the `main` entry point, copies `pos`, `rng_hi`,
`rng_lo`, `outcome` into their own mappable staging buffers, blocks on
`device.poll(PollType::wait_indefinitely())`, and reads them back into
`batch` + the returned outcome vec.

# Preconditions
`grid.len() == sigma.len() >= 2`, `grid` sorted ascending; the batch length
invariants in [`FlightBatch`]. An empty batch returns an empty `Vec` without
touching the GPU.

```rust
pub fn advance_flight_gpu(ctx: &crate::gpu::GpuContext, grid: &[f32], sigma: &[f32], batch: &mut FlightBatch, sphere: FlightSphere) -> Vec<FlightOutcome> { /* ... */ }
```

## Module `collision_grid`

Per-nuclide reaction cross sections tabulated on a shared union energy grid —
the data the GPU **collision** kernel needs to resolve a collision entirely on
device (which nuclide, which reaction, secondary energy/angle) without a
per-event CPU round-trip.

## Why this exists (the op-u6s.8 fix)

The batched-flight path ([`crate::gpu::batched_flight`]) puts the per-event
*flight* on the GPU but keeps the branchy *collision* physics on the CPU,
forcing a CPU↔GPU round-trip **per event** (op-u6s.7's measured bottleneck).
To keep the whole generation GPU-resident, the collision kernel must sample the
nuclide, partition the reaction, and apply scatter kinematics on device. It
cannot call [`crate::material::nuclide::Nuclide::xs_at_energy`] there (that is a
WMP / MGXS evaluation with no GPU form), so — exactly as
[`crate::gpu::union_grid::UnionTotalXs`] does for the macroscopic total — the
per-nuclide reaction cross sections are **pre-tabulated once on the CPU** on a
dense union grid and uploaded; the kernel then does a binary search + linear
interpolation per channel.

## What is tabulated

On a shared ascending energy grid (the native-breakpoint union of
[`crate::gpu::union_grid::UnionTotalXs::tabulate_native`], so grid points land
on real data features), for **each** nuclide component of the material:
its microscopic `total`, `fission`, `absorption`, `inelastic`, `nu_fission`
(all barn), and elastic mean cosine `mubar` (dimensionless). Plus the material's
macroscopic total Σ_t (cm⁻¹) and, per nuclide, its atom density `N` \[atoms/
barn·cm\], atomic weight ratio `awr`, and CE↔MG seam energy `e_max` \[eV\].

Every column mirrors the CPU collision partition in
[`crate::physics::keff`] `collide_batched`, so the on-device collision is the
same physical model as the trusted CPU path — differing only through the f32
tabulation/interpolation (acceleration only, judged against the CPU reference,
never trusted above it; see `crate::gpu` `mod.rs`).

## Fidelity scope (stated honestly)

This is built to reproduce the **LOW (`Core`) tier** collision physics on the
GPU (the Godiva bare-sphere benchmark): WMP + fast-MGXS data, `(n,2n)` = 0, and
elastic anisotropy carried by a single per-group μ̄. For a HIGH (`Pointwise`)
nuclide the `inelastic` column is still tabulated but the elastic angular law
degrades to isotropic-CM on the GPU (`mubar = 0`; see
[`Nuclide::elastic_mubar`](crate::material::nuclide::Nuclide::elastic_mubar)) —
the full MF=4 distribution stays on the trusted CPU backends.

```rust
pub mod collision_grid { /* ... */ }
```

### Types

#### Struct `CollisionTables`

Per-nuclide reaction cross sections on a shared union energy grid, laid out for
upload to the GPU collision kernel.

# Layout
`grid` is the ascending energy grid \[eV\], length `G`. Every per-channel field
is `n_nuclides * G` long, **flattened nuclide-major**: channel `c` for nuclide
`j` at grid point `i` lives at `field[j * G + i]`. The nuclide order matches
`material.components` (so nuclide `j` here is `material.components[j]`).

# Physical meaning / units
- `grid` — ascending energy \[eV\], length `G`.
- `macro_total` — material macroscopic total Σ_t \[cm⁻¹\] at each grid point,
  `sum_j N_j * micro_total[j*G+i]`. Length `G`. (The flight kernel's Σ_t.)
- `micro_total` / `micro_fission` / `micro_absorption` / `micro_inelastic` /
  `micro_nu_fission` — per-nuclide **microscopic** σ \[barn\], length
  `n_nuclides * G`. `absorption = capture + fission`; `nu_fission = ν̄·σ_f`.
- `micro_mubar` — per-nuclide elastic mean cosine μ̄ (CM frame, dimensionless),
  length `n_nuclides * G`; `0` ⇒ isotropic-CM elastic at that energy (see
  [`Nuclide::elastic_mubar`](crate::material::nuclide::Nuclide::elastic_mubar)).
- `atom_density` — per-nuclide `N_j` \[atoms/barn·cm\], length `n_nuclides`.
- `awr` — per-nuclide atomic weight ratio `A_j`, length `n_nuclides`.
- `e_max` — per-nuclide CE↔MG seam energy \[eV\], length `n_nuclides` (elastic
  below it is isotropic-CM, above it uses μ̄).
- `temperature_k` — material temperature \[K\] used for every lookup.

```rust
pub struct CollisionTables {
    pub grid: Vec<f64>,
    pub macro_total: Vec<f64>,
    pub n_nuclides: usize,
    pub micro_total: Vec<f64>,
    pub micro_fission: Vec<f64>,
    pub micro_absorption: Vec<f64>,
    pub micro_inelastic: Vec<f64>,
    pub micro_nu_fission: Vec<f64>,
    pub micro_mubar: Vec<f64>,
    pub atom_density: Vec<f64>,
    pub awr: Vec<f64>,
    pub e_max: Vec<f64>,
    pub temperature_k: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `grid` | `Vec<f64>` | Ascending union energy grid \[eV\], length `G`. |
| `macro_total` | `Vec<f64>` | Macroscopic total Σ_t \[cm⁻¹\] at each grid point, length `G`. |
| `n_nuclides` | `usize` | Number of nuclide components (`= material.components.len()`). |
| `micro_total` | `Vec<f64>` | Per-nuclide microscopic total σ_t \[barn\], length `n_nuclides * G`. |
| `micro_fission` | `Vec<f64>` | Per-nuclide microscopic fission σ_f \[barn\], length `n_nuclides * G`. |
| `micro_absorption` | `Vec<f64>` | Per-nuclide microscopic absorption σ_a \[barn\], length `n_nuclides * G`. |
| `micro_inelastic` | `Vec<f64>` | Per-nuclide microscopic inelastic σ \[barn\], length `n_nuclides * G`. |
| `micro_nu_fission` | `Vec<f64>` | Per-nuclide microscopic ν̄·σ_f \[barn\], length `n_nuclides * G`. |
| `micro_mubar` | `Vec<f64>` | Per-nuclide elastic mean cosine μ̄ (CM), length `n_nuclides * G`. |
| `atom_density` | `Vec<f64>` | Per-nuclide atom density `N_j` \[atoms/barn·cm\], length `n_nuclides`. |
| `awr` | `Vec<f64>` | Per-nuclide atomic weight ratio `A_j`, length `n_nuclides`. |
| `e_max` | `Vec<f64>` | Per-nuclide CE↔MG seam energy \[eV\], length `n_nuclides`. |
| `temperature_k` | `f64` | Material temperature \[K\] used for the tabulation. |

##### Implementations

###### Methods

- ```rust
  pub fn build(material: &Material, nuclides: &[Nuclide], e_min_ev: f64, e_max_ev: f64, backbone_points: usize) -> Self { /* ... */ }
  ```
  Build the per-nuclide reaction tables on the material's

- ```rust
  pub fn n_grid(self: &Self) -> usize { /* ... */ }
  ```
  Number of energy grid points `G`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
## Module `batched_event`

Fused **flight + collision** GPU event kernel — the op-u6s.8 deep-penetration
path — with its CPU mirror (same f32 arithmetic) and a resident per-generation
driver.

Where [`crate::gpu::batched_flight`] advances a batch through one *flight* on
the GPU and hands every collision back to the CPU (a round-trip **per event**),
this module advances a batch through a whole *event* — flight **and** the
branchy collision physics (nuclide sampling, reaction partition, elastic /
inelastic kinematics, fission tagging) — on the GPU. The batch stays resident
in GPU buffers across every event of a generation; the only CPU traffic is a
4-byte live-count read per event and one fission-record read-back per
generation. That removes the per-event PCIe round-trip op-u6s.7 measured as the
bottleneck.

# The honest GPU/CPU split
On the GPU (`f32`): flight (RNG, Σ_t lookup, distance sampling, sphere test),
nuclide sampling, reaction partition (fission | capture | inelastic | elastic),
and the scatter kinematics (two-body elastic, Weisskopf continuum inelastic,
exponential-μ forward elastic). On the CPU (once per generation, not per
event): the fission **daughters** — count `sample_num_neutrons` + birth
energy/angle — replayed from the GPU's per-particle fission record using the
handed-back seed, so the per-history random stream stays coherent across the
boundary. `(n,2n)` secondaries are not produced (the LOW tier this path targets
has `n2n = 0`); the batch therefore only ever shrinks within a generation.

# Precision contract (read `crate::gpu` `mod.rs` first)
The trusted reference is the raw-`f64` CPU transport loop. This path is `f32`
acceleration. [`advance_event_cpu_mirror`] runs the **same f32 arithmetic** as
the WGSL kernel and is the bit-level reference for the kernel's *logic*; the
two are held to agreement by the V&V test in this file. The f32 uniform (top-24
bits of the advanced 64-bit LCG state) diverges from the CPU `f64` `prn` value
exactly as documented for [`crate::gpu::batched_flight`]; the integer LCG state
stays bit-exact.

# Fidelity scope
Built to reproduce the **LOW (`Core`) tier** Godiva collision physics (the
benchmark). HIGH-tier elastic anisotropy (full MF=4) degrades to isotropic-CM
on the GPU (`mubar = 0`); the trusted CPU backends keep the full distribution.

Items whose names end in `_gpu` are `#[cfg(not(target_os = "android"))]` (wgpu
is target-gated off Android); the SoA batch, the packed tables, and the CPU
mirror build on every target.

```rust
pub mod batched_event { /* ... */ }
```

### Types

#### Struct `EventBatch`

A batch of neutrons resident for the fused event kernel (Structure-of-Arrays).

For `N = energy.len()` neutrons, all fields are `f32`/`u32` acceleration state
(the trusted transport loop is `f64`):
- `pos` — length `3N`, interleaved `x,y,z` (cm). READ+WRITE (advanced to the
  collision site each event).
- `dir` — length `3N`, interleaved `u,v,w` unit direction. READ+WRITE (updated
  by scatter).
- `energy` — length `N`, eV. READ+WRITE (updated by scatter; for a fissioned
  neutron it holds the *incident* energy at fission, for CPU χ sampling).
- `seed_lo`/`seed_hi` — length `N`, the low/high 32 bits of each neutron's
  64-bit LCG seed. READ+WRITE (advanced through flight + collision).
- `alive` — length `N`, `1` live / `0` dead. READ+WRITE.
- `fiss_nuc` — length `N`, the **component index** of the nuclide this neutron
  fissioned on, or [`FISS_NONE`]. WRITE (set on fission).
- `production` — length `N`, the ν̄ banked if this neutron fissioned (else 0).
  WRITE.

```rust
pub struct EventBatch {
    pub pos: Vec<f32>,
    pub dir: Vec<f32>,
    pub energy: Vec<f32>,
    pub seed_lo: Vec<u32>,
    pub seed_hi: Vec<u32>,
    pub alive: Vec<u32>,
    pub fiss_nuc: Vec<u32>,
    pub production: Vec<f32>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pos` | `Vec<f32>` | Interleaved positions `x,y,z` (cm), length `3N`. |
| `dir` | `Vec<f32>` | Interleaved unit directions `u,v,w`, length `3N`. |
| `energy` | `Vec<f32>` | Per-neutron energy (eV), length `N`. |
| `seed_lo` | `Vec<u32>` | Low 32 bits of each neutron's 64-bit LCG seed, length `N`. |
| `seed_hi` | `Vec<u32>` | High 32 bits of each neutron's 64-bit LCG seed, length `N`. |
| `alive` | `Vec<u32>` | Alive flag (`1` live, `0` dead), length `N`. |
| `fiss_nuc` | `Vec<u32>` | Fission nuclide component index or [`FISS_NONE`], length `N`. |
| `production` | `Vec<f32>` | ν̄ production if this neutron fissioned (else 0), length `N`. |

##### Implementations

###### Methods

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```
  Number of neutrons `N` in the batch (from `energy.len()`).

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```
  Whether the batch is empty (`N == 0`).

- ```rust
  pub fn n_alive(self: &Self) -> usize { /* ... */ }
  ```
  Number of currently-alive neutrons (`alive[i] == 1`).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
#### Struct `EventSphere`

The bounding sphere for the flight (leakage surface), all `f32` to match the
GPU path. `(x0,y0,z0)` centre (cm), `r` radius (cm).

```rust
pub struct EventSphere {
    pub x0: f32,
    pub y0: f32,
    pub z0: f32,
    pub r: f32,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x0` | `f32` | Centre x (cm). |
| `y0` | `f32` | Centre y (cm). |
| `z0` | `f32` | Centre z (cm). |
| `r` | `f32` | Radius (cm). |

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
    fn clone(self: &Self) -> EventSphere { /* ... */ }
    ```

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
    fn eq(self: &Self, other: &EventSphere) -> bool { /* ... */ }
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
#### Struct `EventTablesF32`

The per-nuclide reaction tables packed into a single `f32` array in the exact
byte layout the WGSL event kernel expects — the CPU-side twin of the GPU `xs`
storage buffer, shared by the mirror and the GPU host so they interpret it
identically.

# Packing (G = `n_grid`, NN = `n_nuclide`), all f32
`grid[G] ++ macro_total[G] ++ micro_total[NN*G] ++ micro_fission[NN*G] ++
micro_absorption[NN*G] ++ micro_inelastic[NN*G] ++ micro_nu_fission[NN*G] ++
micro_mubar[NN*G] ++ atom_density[NN] ++ awr[NN] ++ e_max[NN]`.
Channel `c` of nuclide `j` at grid point `k` is `xs[base_c + j*G + k]`.

```rust
pub struct EventTablesF32 {
    pub xs: Vec<f32>,
    pub n_grid: usize,
    pub n_nuclide: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `xs` | `Vec<f32>` | Packed cross-section data (see the struct docs for the layout). |
| `n_grid` | `usize` | Number of energy grid points `G`. |
| `n_nuclide` | `usize` | Number of nuclide components `NN`. |

##### Implementations

###### Methods

- ```rust
  pub fn from_collision_tables(t: &CollisionTables) -> Self { /* ... */ }
  ```
  Pack a [`CollisionTables`] (built on the CPU in `f64`) into the flat `f32`

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

#### Function `advance_event_cpu_mirror`

Advance **every currently-alive neutron** in `batch` through **one event**
(flight + collision) on the CPU, using the **exact same f32 arithmetic path**
as the GPU kernel `shaders/batched_event.wgsl`.

This is the bit-level logic reference for the GPU kernel (same LCG emulation,
same top-24 uniform, same grid search, same kinematics, same draw order). It is
**unconditional** (builds and runs on Android too).

Per alive neutron `i`: advance RNG → `xi`; look up Σ_t; leak/absorb ⇒
`alive[i]=0`; else stream to the collision site, sample nuclide + reaction, and
either mark fission (`fiss_nuc[i]`, `production[i]`, dead), capture (dead), or
scatter (update `energy`/`dir`, stay alive). Returns the number of neutrons
still alive after the event.

```rust
pub fn advance_event_cpu_mirror(tables: &EventTablesF32, batch: &mut EventBatch, sphere: EventSphere) -> usize { /* ... */ }
```

#### Function `advance_generation_cpu_mirror`

Run a whole generation on the CPU mirror: repeatedly [`advance_event_cpu_mirror`]
until the batch drains or `max_events` is hit. Returns the number of events run.
This is the non-GPU reference driver for the fused event path (Android-clean).

```rust
pub fn advance_generation_cpu_mirror(tables: &EventTablesF32, batch: &mut EventBatch, sphere: EventSphere, max_events: usize) -> usize { /* ... */ }
```

#### Function `advance_generation_gpu`

**Attributes:**

- `Other("#[attr = CfgTrace([Not(NameValue { name: \"target_os\", value: Some(\"android\"), span: crates/outram-mc-libs/src/gpu/batched_event.rs:649:11: 649:32 (#0) }, crates/outram-mc-libs/src/gpu/batched_event.rs:649:10: 649:33 (#0))])]")`

Advance a whole generation on the **GPU**, keeping the batch resident in GPU
buffers across every event ([`crate::gpu`] `mod.rs` precision contract applies).

Uploads the packed tables (`xs`), the integer state (`seed_lo ++ seed_hi ++
alive ++ fiss_nuc`), the float state (`pos ++ dir ++ energy ++ production`), and
a control atomic **once**, then loops: reset the live counter, dispatch the
fused event kernel over all `N` neutrons, and read back the 4-byte live count.
The loop stops when no neutron is alive or `max_events` is reached; only then is
the full state read back into `batch`. Returns the number of events dispatched.

This is the `f32` accelerated counterpart of [`advance_generation_cpu_mirror`];
the per-event GPU logic is held to the CPU mirror by the V&V test in this file.
An empty batch returns `0` without touching the GPU.

```rust
pub fn advance_generation_gpu(ctx: &crate::gpu::GpuContext, tables: &EventTablesF32, batch: &mut EventBatch, sphere: EventSphere, max_events: usize) -> usize { /* ... */ }
```

### Constants and Statics

#### Constant `FISS_NONE`

Sentinel in [`EventBatch::fiss_nuc`] meaning "this neutron did not fission".

```rust
pub const FISS_NONE: u32 = 0xFFFF_FFFF;
```

### Re-exports

#### Re-export `block_on`

**Attributes:**

- `Other("#[attr = CfgTrace([Not(NameValue { name: \"target_os\", value: Some(\"android\"), span: crates/outram-mc-libs/src/gpu/mod.rs:175:11: 175:32 (#0) }, crates/outram-mc-libs/src/gpu/mod.rs:175:10: 175:33 (#0))])]")`

```rust
pub use context::block_on;
```

#### Re-export `probe`

**Attributes:**

- `Other("#[attr = CfgTrace([Not(NameValue { name: \"target_os\", value: Some(\"android\"), span: crates/outram-mc-libs/src/gpu/mod.rs:175:11: 175:32 (#0) }, crates/outram-mc-libs/src/gpu/mod.rs:175:10: 175:33 (#0))])]")`

```rust
pub use context::probe;
```

#### Re-export `GpuContext`

**Attributes:**

- `Other("#[attr = CfgTrace([Not(NameValue { name: \"target_os\", value: Some(\"android\"), span: crates/outram-mc-libs/src/gpu/mod.rs:175:11: 175:32 (#0) }, crates/outram-mc-libs/src/gpu/mod.rs:175:10: 175:33 (#0))])]")`

```rust
pub use context::GpuContext;
```

## Module `perf_report`

Per-machine performance-report generator: detects this host's GPU / CPU / OS
and renders a self-service "what performance is available on my PC" markdown
report from measured benchmark timings. Machine-specific output is written to
a gitignored local path — see [`perf_report`].
Per-machine performance-report generator — the self-service "what performance
is available on my PC" answer for the Monte Carlo compute backends.

Benchmark timings differ from machine to machine, so rather than treating one
development box's static numbers as authoritative, this module **detects the
host hardware and renders a fresh markdown report from the timings measured on
*that* machine**. The machine-specific output is written to a **gitignored**
local path ([`write_local_report`]) so each user's numbers stay local and
never commit; only methodology / template docs are committed.

## What belongs here
- [`HardwareInfo`] — detect this host's GPU adapter (name + backend), CPU
  logical-core count, and OS.
- [`PerfReport`] / [`PerfRow`] — accumulate measured `(batch size, backend)`
  wall-clock + eigenvalue rows and render them to markdown or CSV, including
  the GPU-vs-`CpuMultiThread` speedup and an honest crossover verdict.
- [`write_local_report`] — persist a rendered report under the crate's
  gitignored `verification_and_validation/local_perf/` directory.

## What does NOT belong here
The transport physics or the benchmark loop itself (that lives in the relevant
`physics` / example driver); this module only *describes* measured results.
It is pure formatting + host introspection — no RNG, no transport.

Builds on every target, Android included: GPU detection is target-gated so on
Android (no `wgpu`) the report simply records "CPU only".

```rust
pub mod perf_report { /* ... */ }
```

### Types

#### Struct `HardwareInfo`

A snapshot of the host machine's compute-relevant hardware, for the report
header. All fields are best-effort host introspection.

```rust
pub struct HardwareInfo {
    pub gpu: Option<String>,
    pub cpu_logical_cores: usize,
    pub os: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `gpu` | `Option<String>` | GPU adapter as `"<name> / <backend>"` (e.g. `"NVIDIA GeForce RTX 3050 /<br>Vulkan"`), or `None` when no usable GPU adapter is present (headless<br>server, CI with no loader, or Android — where the report reads<br>"CPU only"). |
| `cpu_logical_cores` | `usize` | Logical CPU cores via [`std::thread::available_parallelism`] (the count<br>[`crate::physics::compute::ThreadCount::Auto`] resolves to); `1` if the<br>query fails. |
| `os` | `String` | Target OS string from [`std::env::consts::OS`] (e.g. `"linux"`,<br>`"windows"`, `"macos"`, `"android"`). |

##### Implementations

###### Methods

- ```rust
  pub fn detect() -> Self { /* ... */ }
  ```
  Introspect the current host: GPU adapter (if any), logical CPU cores, OS.

- ```rust
  pub fn headline(self: &Self) -> String { /* ... */ }
  ```
  A one-line hardware headline, e.g.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> HardwareInfo { /* ... */ }
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
#### Struct `PerfRow`

One measured timing row: a full benchmark run of `backend` at a given batch
size. Wall-clock in seconds; `k_mean`/`k_std` the eigenvalue and its 1-sigma
standard error; `histories_total` the total neutron histories transported
(`batch_size * generations`) used for the throughput figure.

```rust
pub struct PerfRow {
    pub batch_size: usize,
    pub backend: String,
    pub wall_time_s: f64,
    pub k_mean: f64,
    pub k_std: f64,
    pub histories_total: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `batch_size` | `usize` | Neutron histories per generation for this run. |
| `backend` | `String` | Compute backend name, e.g. `"CpuSingleThread"`, `"CpuMultiThread"`, `"Gpu"`. |
| `wall_time_s` | `f64` | Measured wall-clock of the full `run_keff`, seconds. |
| `k_mean` | `f64` | Reported mean eigenvalue. |
| `k_std` | `f64` | 1-sigma standard error of `k_mean`. |
| `histories_total` | `usize` | Total histories transported over the whole run (`batch_size * n_gen`),<br>the numerator of the histories/second throughput. |

##### Implementations

###### Methods

- ```rust
  pub fn throughput_hps(self: &Self) -> f64 { /* ... */ }
  ```
  Throughput in **histories per second** (`histories_total / wall_time_s`);

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> PerfRow { /* ... */ }
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
#### Struct `PerfReport`

An accumulated set of benchmark rows plus the host hardware, renderable to a
per-machine markdown report or CSV.

Build it with [`PerfReport::new`], [`push`](PerfReport::push) each measured
[`PerfRow`], then [`render_markdown`](PerfReport::render_markdown) /
[`to_csv`](PerfReport::to_csv). The markdown includes a hardware headline, a
per-backend timing + throughput table, the GPU-vs-`CpuMultiThread` speedup at
each batch size, and an honest crossover verdict.

```rust
pub struct PerfReport {
    pub title: String,
    pub hardware: HardwareInfo,
    pub rows: Vec<PerfRow>,
    pub date: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `title` | `String` | Free-text title shown at the top of the report. |
| `hardware` | `HardwareInfo` | The host this report was generated on. |
| `rows` | `Vec<PerfRow>` | The measured rows, in insertion order. |
| `date` | `String` | ISO date (`YYYY-MM-DD`) the run was taken; free-text, caller-supplied. |

##### Implementations

###### Methods

- ```rust
  pub fn new</* synthetic */ impl Into<String>: Into<String>, /* synthetic */ impl Into<String>: Into<String>>(title: impl Into<String>, date: impl Into<String>) -> Self { /* ... */ }
  ```
  Start an empty report for `title`, taken on `date` (`YYYY-MM-DD`), with the

- ```rust
  pub fn push(self: &mut Self, row: PerfRow) { /* ... */ }
  ```
  Append one measured [`PerfRow`].

- ```rust
  pub fn render_markdown(self: &Self) -> String { /* ... */ }
  ```
  Render the machine-specific report to GitHub-flavoured markdown.

- ```rust
  pub fn to_csv(self: &Self) -> String { /* ... */ }
  ```
  Render the rows to CSV (same columns as the committed methodology doc

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> PerfReport { /* ... */ }
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

#### Function `write_local_report`

Write `content` to `LOCAL_PERF_DIR/<filename>`, creating the directory if
needed, and return the path written. This directory is gitignored so each
user's machine-specific numbers stay local.

Returns any [`std::io::Error`] from creating the directory or file (e.g. the
process has no write access to the crate tree — harmless for a benchmark).

```rust
pub fn write_local_report(filename: &str, content: &str) -> std::io::Result<std::path::PathBuf> { /* ... */ }
```

### Constants and Statics

#### Constant `LOCAL_PERF_DIR`

The crate-local, **gitignored** directory machine-specific reports are written
to: `verification_and_validation/local_perf/` (relative to the crate root, the
usual working directory for `cargo test`/example runs).

```rust
pub const LOCAL_PERF_DIR: &str = "verification_and_validation/local_perf";
```

## Module `prelude`

```rust
pub mod prelude { /* ... */ }
```

### Re-exports

#### Re-export `prn`

Convenience re-export of the most commonly used types.

```rust
use outram_mc_libs::prelude::*;
```

```rust
pub use crate::rng::lcg::prn;
```

#### Re-export `future_seed`

Convenience re-export of the most commonly used types.

```rust
use outram_mc_libs::prelude::*;
```

```rust
pub use crate::rng::lcg::future_seed;
```

#### Re-export `init_seed`

Convenience re-export of the most commonly used types.

```rust
use outram_mc_libs::prelude::*;
```

```rust
pub use crate::rng::lcg::init_seed;
```

#### Re-export `Position`

```rust
pub use crate::geometry::position::Position;
```

#### Re-export `Direction`

```rust
pub use crate::geometry::position::Direction;
```

#### Re-export `BoundaryType`

```rust
pub use crate::geometry::surface::BoundaryType;
```

#### Re-export `Sphere`

```rust
pub use crate::geometry::surface::Sphere;
```

#### Re-export `SurfaceKind`

```rust
pub use crate::geometry::surface::SurfaceKind;
```

#### Re-export `XPlane`

```rust
pub use crate::geometry::surface::XPlane;
```

#### Re-export `YPlane`

```rust
pub use crate::geometry::surface::YPlane;
```

#### Re-export `ZPlane`

```rust
pub use crate::geometry::surface::ZPlane;
```

#### Re-export `ZCylinder`

```rust
pub use crate::geometry::surface::ZCylinder;
```

#### Re-export `Cell`

```rust
pub use crate::geometry::cell::Cell;
```

#### Re-export `CellFill`

```rust
pub use crate::geometry::cell::CellFill;
```

#### Re-export `HalfSpaceSense`

```rust
pub use crate::geometry::cell::HalfSpaceSense;
```

#### Re-export `RegionToken`

```rust
pub use crate::geometry::cell::RegionToken;
```

#### Re-export `Universe`

```rust
pub use crate::geometry::universe::Universe;
```

#### Re-export `HexLattice`

```rust
pub use crate::geometry::lattice::HexLattice;
```

#### Re-export `HexOrientation`

```rust
pub use crate::geometry::lattice::HexOrientation;
```

#### Re-export `Lattice`

```rust
pub use crate::geometry::lattice::Lattice;
```

#### Re-export `RectLattice`

```rust
pub use crate::geometry::lattice::RectLattice;
```

#### Re-export `BoundaryHit`

```rust
pub use crate::geometry::geometry::BoundaryHit;
```

#### Re-export `Coord`

```rust
pub use crate::geometry::geometry::Coord;
```

#### Re-export `Crossing`

```rust
pub use crate::geometry::geometry::Crossing;
```

#### Re-export `Geometry`

```rust
pub use crate::geometry::geometry::Geometry;
```

#### Re-export `GeometryPath`

```rust
pub use crate::geometry::geometry::GeometryPath;
```

#### Re-export `Particle`

```rust
pub use crate::particle::particle::Particle;
```

#### Re-export `ParticleType`

```rust
pub use crate::particle::particle::ParticleType;
```

#### Re-export `MacroXs`

```rust
pub use crate::material::material::MacroXs;
```

#### Re-export `Material`

```rust
pub use crate::material::material::Material;
```

#### Re-export `MicroXS`

```rust
pub use crate::material::nuclide::MicroXS;
```

#### Re-export `Nuclide`

```rust
pub use crate::material::nuclide::Nuclide;
```

#### Re-export `ThermalScattering`

```rust
pub use crate::material::thermal::ThermalScattering;
```

#### Re-export `ScoreType`

```rust
pub use crate::tally::tally::ScoreType;
```

#### Re-export `Tally`

```rust
pub use crate::tally::tally::Tally;
```

#### Re-export `TallyBin`

```rust
pub use crate::tally::tally::TallyBin;
```

#### Re-export `CellFilter`

```rust
pub use crate::tally::filter::CellFilter;
```

#### Re-export `EnergyFilter`

```rust
pub use crate::tally::filter::EnergyFilter;
```

#### Re-export `Filter`

```rust
pub use crate::tally::filter::Filter;
```

#### Re-export `LegendreAxis`

```rust
pub use crate::tally::filter::LegendreAxis;
```

#### Re-export `MaterialFilter`

```rust
pub use crate::tally::filter::MaterialFilter;
```

#### Re-export `MeshFilter`

```rust
pub use crate::tally::filter::MeshFilter;
```

#### Re-export `SpatialLegendreFilter`

```rust
pub use crate::tally::filter::SpatialLegendreFilter;
```

#### Re-export `UniverseFilter`

```rust
pub use crate::tally::filter::UniverseFilter;
```

#### Re-export `RegularMesh`

```rust
pub use crate::tally::mesh::RegularMesh;
```

#### Re-export `Q_FISSION_J`

```rust
pub use crate::tally::scoring::Q_FISSION_J;
```

#### Re-export `DerivedTally`

```rust
pub use crate::tally::arithmetic::DerivedTally;
```

#### Re-export `ComputeType`

```rust
pub use crate::physics::compute::ComputeType;
```

#### Re-export `ThreadCount`

```rust
pub use crate::physics::compute::ThreadCount;
```

#### Re-export `run_keff`

```rust
pub use crate::physics::keff::run_keff;
```

#### Re-export `KeffResult`

```rust
pub use crate::physics::keff::KeffResult;
```

#### Re-export `KeffSettings`

```rust
pub use crate::physics::keff::KeffSettings;
```

#### Re-export `search_for_keff`

```rust
pub use crate::physics::search::search_for_keff;
```

#### Re-export `SearchError`

```rust
pub use crate::physics::search::SearchError;
```

#### Re-export `SearchIteration`

```rust
pub use crate::physics::search::SearchIteration;
```

#### Re-export `SearchMethod`

```rust
pub use crate::physics::search::SearchMethod;
```

#### Re-export `SearchResult`

```rust
pub use crate::physics::search::SearchResult;
```

#### Re-export `SearchSettings`

```rust
pub use crate::physics::search::SearchSettings;
```

#### Re-export `run_keff_csg`

```rust
pub use crate::physics::transport_csg::run_keff_csg;
```

#### Re-export `SourceBox`

```rust
pub use crate::physics::transport_csg::SourceBox;
```

#### Re-export `run_fixed_source`

```rust
pub use crate::physics::fixed_source::run_fixed_source;
```

#### Re-export `FixedSource`

```rust
pub use crate::physics::fixed_source::FixedSource;
```

#### Re-export `FixedSourceResult`

```rust
pub use crate::physics::fixed_source::FixedSourceResult;
```

#### Re-export `FixedSourceSettings`

```rust
pub use crate::physics::fixed_source::FixedSourceSettings;
```

#### Re-export `track_to_collision`

```rust
pub use crate::pebble_beds::delta_tracking::track_to_collision;
```

#### Re-export `DeltaEvent`

```rust
pub use crate::pebble_beds::delta_tracking::DeltaEvent;
```

#### Re-export `DeltaFlight`

```rust
pub use crate::pebble_beds::delta_tracking::DeltaFlight;
```

#### Re-export `Majorant`

```rust
pub use crate::pebble_beds::delta_tracking::Majorant;
```

#### Re-export `run_keff_delta`

```rust
pub use crate::pebble_beds::keff_delta::run_keff_delta;
```

#### Re-export `pack_spheres`

```rust
pub use crate::pebble_beds::sphere_packing::pack_spheres;
```

#### Re-export `PackedSpheres`

```rust
pub use crate::pebble_beds::sphere_packing::PackedSpheres;
```

#### Re-export `PackingConfig`

```rust
pub use crate::pebble_beds::sphere_packing::PackingConfig;
```

#### Re-export `PackingMethod`

```rust
pub use crate::pebble_beds::sphere_packing::PackingMethod;
```

#### Re-export `MaterialId`

```rust
pub use crate::stochastic::medium::MaterialId;
```

#### Re-export `MediumError`

```rust
pub use crate::stochastic::medium::MediumError;
```

#### Re-export `RsaMedium`

```rust
pub use crate::stochastic::medium::RsaMedium;
```

#### Re-export `StochasticMedium`

```rust
pub use crate::stochastic::medium::StochasticMedium;
```

#### Re-export `mean_chord_length_sphere`

```rust
pub use crate::stochastic::cls::mean_chord_length_sphere;
```

#### Re-export `matrix_mean_chord_length`

```rust
pub use crate::stochastic::cls::matrix_mean_chord_length;
```

#### Re-export `sample_chord`

```rust
pub use crate::stochastic::cls::sample_chord;
```

#### Re-export `ClsMedium`

```rust
pub use crate::stochastic::cls::ClsMedium;
```

#### Re-export `AdaptiveRadius`

```rust
pub use crate::stochastic::scls::AdaptiveRadius;
```

#### Re-export `FlightSegment`

```rust
pub use crate::stochastic::scls::FlightSegment;
```

#### Re-export `InclusionSphere`

```rust
pub use crate::stochastic::scls::InclusionSphere;
```

#### Re-export `ParticleHistory`

```rust
pub use crate::stochastic::scls::ParticleHistory;
```

#### Re-export `SclsMedium`

```rust
pub use crate::stochastic::scls::SclsMedium;
```

#### Re-export `BruteForceIndex`

```rust
pub use crate::stochastic::spatial_index::BruteForceIndex;
```

#### Re-export `IndexError`

```rust
pub use crate::stochastic::spatial_index::IndexError;
```

#### Re-export `KdTreeIndex`

```rust
pub use crate::stochastic::spatial_index::KdTreeIndex;
```

#### Re-export `SpatialIndex`

```rust
pub use crate::stochastic::spatial_index::SpatialIndex;
```

#### Re-export `AbsorptionBenchmark`

```rust
pub use crate::stochastic::benchmark::AbsorptionBenchmark;
```

#### Re-export `BenchmarkResult`

```rust
pub use crate::stochastic::benchmark::BenchmarkResult;
```

#### Re-export `interp_xs_cpu`

```rust
pub use crate::gpu::xs_interp::interp_xs_cpu;
```

#### Re-export `probe`

```rust
pub use crate::gpu::probe as gpu_probe;
```

#### Re-export `GpuContext`

```rust
pub use crate::gpu::GpuContext;
```

#### Re-export `interp_xs_gpu`

**Attributes:**

- `Other("#[attr = CfgTrace([Not(NameValue { name: \"target_os\", value: Some(\"android\"), span: crates/outram-mc-libs/src/prelude.rs:57:11: 57:32 (#0) }, crates/outram-mc-libs/src/prelude.rs:57:10: 57:33 (#0))])]")`

```rust
pub use crate::gpu::xs_interp::interp_xs_gpu;
```

#### Re-export `encode_surfaces`

```rust
pub use crate::gpu::surface_distance::encode_surfaces;
```

#### Re-export `surface_distance_cpu_f32`

```rust
pub use crate::gpu::surface_distance::surface_distance_cpu_f32;
```

#### Re-export `EncodedSurfaces`

```rust
pub use crate::gpu::surface_distance::EncodedSurfaces;
```

#### Re-export `SurfaceQuery`

```rust
pub use crate::gpu::surface_distance::SurfaceQuery;
```

#### Re-export `MISS`

```rust
pub use crate::gpu::surface_distance::MISS;
```

#### Re-export `SURF_STRIDE`

```rust
pub use crate::gpu::surface_distance::SURF_STRIDE;
```

#### Re-export `surface_distance_gpu`

**Attributes:**

- `Other("#[attr = CfgTrace([Not(NameValue { name: \"target_os\", value: Some(\"android\"), span: crates/outram-mc-libs/src/prelude.rs:67:11: 67:32 (#0) }, crates/outram-mc-libs/src/prelude.rs:67:10: 67:33 (#0))])]")`

```rust
pub use crate::gpu::surface_distance::surface_distance_gpu;
```

#### Re-export `advance_flight_cpu_mirror`

```rust
pub use crate::gpu::batched_flight::advance_flight_cpu_mirror;
```

#### Re-export `FlightBatch`

```rust
pub use crate::gpu::batched_flight::FlightBatch;
```

#### Re-export `FlightOutcome`

```rust
pub use crate::gpu::batched_flight::FlightOutcome;
```

#### Re-export `FlightSphere`

```rust
pub use crate::gpu::batched_flight::FlightSphere;
```

#### Re-export `advance_flight_gpu`

**Attributes:**

- `Other("#[attr = CfgTrace([Not(NameValue { name: \"target_os\", value: Some(\"android\"), span: crates/outram-mc-libs/src/prelude.rs:75:11: 75:32 (#0) }, crates/outram-mc-libs/src/prelude.rs:75:10: 75:33 (#0))])]")`

```rust
pub use crate::gpu::batched_flight::advance_flight_gpu;
```

#### Re-export `CollisionTables`

```rust
pub use crate::gpu::collision_grid::CollisionTables;
```

#### Re-export `advance_event_cpu_mirror`

```rust
pub use crate::gpu::batched_event::advance_event_cpu_mirror;
```

#### Re-export `advance_generation_cpu_mirror`

```rust
pub use crate::gpu::batched_event::advance_generation_cpu_mirror;
```

#### Re-export `EventBatch`

```rust
pub use crate::gpu::batched_event::EventBatch;
```

#### Re-export `EventSphere`

```rust
pub use crate::gpu::batched_event::EventSphere;
```

#### Re-export `EventTablesF32`

```rust
pub use crate::gpu::batched_event::EventTablesF32;
```

#### Re-export `FISS_NONE`

```rust
pub use crate::gpu::batched_event::FISS_NONE;
```

#### Re-export `advance_generation_gpu`

**Attributes:**

- `Other("#[attr = CfgTrace([Not(NameValue { name: \"target_os\", value: Some(\"android\"), span: crates/outram-mc-libs/src/prelude.rs:87:11: 87:32 (#0) }, crates/outram-mc-libs/src/prelude.rs:87:10: 87:33 (#0))])]")`

```rust
pub use crate::gpu::batched_event::advance_generation_gpu;
```

#### Re-export `HardwareInfo`

```rust
pub use crate::perf_report::HardwareInfo;
```

#### Re-export `PerfReport`

```rust
pub use crate::perf_report::PerfReport;
```

#### Re-export `PerfRow`

```rust
pub use crate::perf_report::PerfRow;
```

