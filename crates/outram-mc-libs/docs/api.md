# Crate Documentation

**Version:** 0.1.0

**Format Version:** 60

# Module `outram_mc_libs`

# outram-mc-libs

Pure-Rust port of selected [OpenMC](https://openmc.org) Monte Carlo
neutron-transport kernels (RNG, geometry/CSG, particle tracking,
k-eigenvalue, delta/Woodcock tracking). Data-free: cross sections are
pulled from `njoy-outram-park-fork`'s `XsProvider` surface.

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

- **Freeze**
- **From**
  - ```rust
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

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

- **Freeze**
- **From**
  - ```rust
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

- **Freeze**
- **From**
  - ```rust
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

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
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

- **Freeze**
- **From**
  - ```rust
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

- **Freeze**
- **From**
  - ```rust
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

- **Freeze**
- **From**
  - ```rust
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

- **Freeze**
- **From**
  - ```rust
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

- **Freeze**
- **From**
  - ```rust
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
    fn distance(self: &Self, _r: Position, _u: Direction, _coincident: bool) -> f64 { /* ... */ }
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

## Module `cell`

```rust
pub mod cell { /* ... */ }
```

### Types

#### Enum `RegionToken`

Token in the RPN region definition.  Maps to OpenMC's region token encoding.

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

Half-space: surface index, positive = outside, negative = inside.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `surface_idx` | `usize` |  |
| `sense` | `HalfSpaceSense` |  |

###### `Intersection`

###### `Union`

###### `Complement`

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

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

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `HalfSpaceSense`

```rust
pub enum HalfSpaceSense {
    Inside,
    Outside,
}
```

##### Variants

###### `Inside`

###### `Outside`

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

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

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Enum `CellFill`

What fills a cell.  Maps to OpenMC's `Cell::type_`.

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

Filled with a nested universe (index).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

###### `Lattice`

Filled with a lattice (index).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |

###### `Void`

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

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

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `Cell`

A CSG cell.  Maps to `openmc::Cell`.

```rust
pub struct Cell {
    pub id: i32,
    pub region: Vec<RegionToken>,
    pub fill: CellFill,
    pub temperature: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `id` | `i32` |  |
| `region` | `Vec<RegionToken>` |  |
| `fill` | `CellFill` |  |
| `temperature` | `f64` | Temperature of this cell in eV (1 eV ≈ 11604 K). |

##### Implementations

###### Methods

- ```rust
  pub fn contains(self: &Self, r: Position, surfaces: &[Box<dyn Surface>]) -> bool { /* ... */ }
  ```
  Evaluate the region definition at position `r` using the provided surfaces.

- ```rust
  pub fn distance_to_boundary(self: &Self, r: super::position::Position, u: super::position::Direction, surfaces: &[Box<dyn Surface>]) -> (f64, usize) { /* ... */ }
  ```
  Distance to the nearest surface bounding this cell along ray `(r, u)`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
| `id` | `i32` |  |
| `cell_indices` | `Vec<usize>` | Indices into the global cell array, in search order. |

##### Implementations

###### Methods

- ```rust
  pub fn find_cell(self: &Self, r: Position, u: Direction, _surfaces: &[Box<dyn super::surface::Surface>], _cells: &[super::cell::Cell]) -> Option<usize> { /* ... */ }
  ```
  Find the cell in this universe that contains `r`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
## Module `lattice`

```rust
pub mod lattice { /* ... */ }
```

### Types

#### Enum `LatticeType`

Lattice type tag.  Maps to `openmc::LatticeType`.

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

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `RectLattice`

A rectangular lattice.  Maps to `openmc::RectLattice`.

```rust
pub struct RectLattice {
    pub id: i32,
    pub n: [usize; 3],
    pub lower_left: super::position::Position,
    pub pitch: [f64; 3],
    pub universes: Vec<usize>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `id` | `i32` |  |
| `n` | `[usize; 3]` | Number of grid cells in x, y, z. |
| `lower_left` | `super::position::Position` | Lower-left corner of the lattice in cm. |
| `pitch` | `[f64; 3]` | Pitch (cell width) in cm for each axis. |
| `universes` | `Vec<usize>` | Universe index for each lattice element, row-major `[z][y][x]`. |

##### Implementations

###### Methods

- ```rust
  pub fn get_indices(self: &Self, r: Position) -> Option<[usize; 3]> { /* ... */ }
  ```
  Map a position to a lattice index triplet `[ix, iy, iz]`.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
#### Struct `HexLattice`

A hexagonal lattice.  Maps to `openmc::HexLattice`.

```rust
pub struct HexLattice {
    pub id: i32,
    pub n_rings: usize,
    pub n_axial: usize,
    pub center: super::position::Position,
    pub pitch: [f64; 2],
    pub universes: Vec<usize>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `id` | `i32` |  |
| `n_rings` | `usize` |  |
| `n_axial` | `usize` |  |
| `center` | `super::position::Position` |  |
| `pitch` | `[f64; 2]` |  |
| `universes` | `Vec<usize>` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
## Module `geometry`

```rust
pub mod geometry { /* ... */ }
```

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

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
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

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
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

- **Freeze**
- **From**
  - ```rust
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

- **Freeze**
- **From**
  - ```rust
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

- **Freeze**
- **From**
  - ```rust
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

- **Freeze**
- **From**
  - ```rust
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

- **Freeze**
- **From**
  - ```rust
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

- **Freeze**
- **From**
  - ```rust
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

- **Freeze**
- **From**
  - ```rust
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
  pub fn sample_fission_energy(self: &Self, e_in: f64, seed: &mut u64) -> f64 { /* ... */ }
  ```
  Sample a fission-neutron birth energy \[eV\] given the incident energy

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

```rust
pub mod thermal { /* ... */ }
```

### Types

#### Struct `ThermalScattering`

S(α,β) thermal scattering tables.

C++ source: `src/thermal.cpp`, `include/openmc/thermal.h`.

At low energies (E < ~4 eV), free-gas treatment of scattering from bound
atoms is inaccurate. OpenMC supports tabulated S(α,β) data (ENDF/B-VII+
thermal scattering law files) for materials like H in H₂O, graphite, etc.

TODO: port after the core XS lookup and reaction sampling framework is in place.
S(α,β) thermal scattering table.  Maps to `openmc::ThermalScattering`.

```rust
pub struct ThermalScattering {
    pub name: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Freeze**
- **From**
  - ```rust
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

- **Freeze**
- **From**
  - ```rust
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

- **Freeze**
- **From**
  - ```rust
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

- **Freeze**
- **From**
  - ```rust
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

- **Freeze**
- **From**
  - ```rust
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

- **Freeze**
- **From**
  - ```rust
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

- **Freeze**
- **From**
  - ```rust
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

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
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

- **Freeze**
- **From**
  - ```rust
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

- **Freeze**
- **From**
  - ```rust
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

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
### Traits

#### Trait `Filter`

Tally filters — constrain which phase-space events are scored.

C++ source: `src/tallies/filter_*.cpp` (30+ files), `include/openmc/tallies/filter.h`.

Filters work as a conjunction: a particle event is scored only if it passes
ALL filters attached to a tally.  Each filter maps the event to a bin index.

Implemented here: Cell, Material, Energy, Universe.
TODO: Mesh, Legendre, Zernike, SphericalHarmonics, MuFilter, PolarAzimuthal,
      Surface, DelayedGroup, Time, Particle.
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

##### Implementations

This trait is implemented for the following types:

- `CellFilter`
- `MaterialFilter`
- `EnergyFilter`
- `UniverseFilter`

## Module `scoring`

```rust
pub mod scoring { /* ... */ }
```

## Module `physics`

```rust
pub mod physics { /* ... */ }
```

### Modules

## Module `transport`

```rust
pub mod transport { /* ... */ }
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

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

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

- **Freeze**
- **From**
  - ```rust
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

- **Freeze**
- **From**
  - ```rust
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

#### Function `run_keff`

Run fission-source power iteration on a bare sphere of radius `radius_cm`
(centred at the origin, vacuum outside) filled with `material`.

`nuclides` is the global nuclide array the material's components index into.
Returns the mean eigenvalue and its standard error over the active
generations. See the module docs for the algorithm and fidelity caveats.

```rust
pub fn run_keff(radius_cm: f64, material: &crate::material::material::Material, nuclides: &[crate::material::nuclide::Nuclide], settings: &KeffSettings) -> KeffResult { /* ... */ }
```

## Module `physics_mg`

```rust
pub mod physics_mg { /* ... */ }
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
- **[`stochastic_media`]** — generating and sampling the random packed geometry
  itself (Random Sequential Addition, Chord Length Sampling, RSA–DEM/ODR–DEM
  hybrids). See the [`references`] bibliography.

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

- **Freeze**
- **From**
  - ```rust
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

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
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

- **Freeze**
- **From**
  - ```rust
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

## Module `references`

Bibliography for the pebble-bed / dispersion-fuel geometry methods.

These are the stochastic-media and packing-generation papers the
[`super::stochastic_media`] and [`super::delta_tracking`] work builds on —
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

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
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

## Module `stochastic_media`

Stochastic-media geometry for dispersion fuel — **scaffold (stub)**.

This is where packed-particle geometry generation and sampling will live: the
Random Sequential Addition (RSA), Chord Length Sampling (CLS), and RSA–DEM /
ODR–DEM methods that produce and query the doubly-heterogeneous TRISO/pebble
layout the [`super::delta_tracking`] flight moves through. See the
[`super::references`] bibliography (Tan et al.) for the algorithms.

**Status:** interface sketch only — the generators are not implemented yet.
The types below fix the vocabulary (a packed [`Sphere`], a [`PackingConfig`])
so callers and docs can refer to them, but [`PackingConfig::generate`] returns
[`StochasticMediaError::NotImplemented`] for now.

```rust
pub mod stochastic_media { /* ... */ }
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

- **Freeze**
- **From**
  - ```rust
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

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
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
parallelizable, but saturates around ~38 % packing fraction.
Ref: [`super::references::TAN2024_RSA`].

###### `RsaDem`

Iterative RSA followed by Discrete-Element-Method relaxation, to reach the
high packing fractions RSA alone cannot. Ref:
[`super::references::TAN2026_RSA_DEM`].

###### `OdrDem`

Coupled Ordered/Overlap-Driven-Relaxation with DEM. Ref:
[`super::references::TAN2026_ODR_DEM`].

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

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

- **RefUnwindSafe**
- **Same**
- **Send**
- **StructuralPartialEq**
- **Sync**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **UnsafeUnpin**
- **UnwindSafe**
#### Struct `PackingConfig`

Parameters for generating a packed dispersion-fuel region.

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
| `particle_radius` | `f64` | Particle radius \[cm\] (all particles equal-radius for now). |
| `packing_fraction` | `f64` | Target volumetric packing fraction (0…1). |
| `domain_half_width` | `f64` | Half-width \[cm\] of the cubic domain the particles pack into. |
| `method` | `PackingMethod` | Generation algorithm. |
| `seed` | `u64` | RNG seed for reproducibility. |

##### Implementations

###### Methods

- ```rust
  pub fn generate(self: &Self) -> Result<Vec<Sphere>, StochasticMediaError> { /* ... */ }
  ```
  Generate the packed sphere list — **not implemented (scaffold)**.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

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

- **Freeze**
- **From**
  - ```rust
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
#### Enum `StochasticMediaError`

Errors from stochastic-media generation.

```rust
pub enum StochasticMediaError {
    NotImplemented(PackingMethod),
}
```

##### Variants

###### `NotImplemented`

The generator is not implemented yet (scaffold).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `PackingMethod` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> StochasticMediaError { /* ... */ }
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

- **IntoEither**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &StochasticMediaError) -> bool { /* ... */ }
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

- **RefUnwindSafe**
- **Same**
- **Send**
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

#### Re-export `Tally`

```rust
pub use crate::tally::tally::Tally;
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

