# Crate Documentation

**Version:** 0.1.5

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

## Module `primitives`

```rust
pub mod primitives { /* ... */ }
```

### Modules

## Module `scalar`

```rust
pub mod scalar { /* ... */ }
```

### Types

#### Type Alias `Scalar`

```rust
pub type Scalar = f64;
```

#### Type Alias `Label`

```rust
pub type Label = i64;
```

### Constants and Statics

#### Constant `SMALL`

```rust
pub const SMALL: Scalar = 1e-15;
```

#### Constant `VSMALL`

```rust
pub const VSMALL: Scalar = 1e-300;
```

#### Constant `ROOT_SMALL`

```rust
pub const ROOT_SMALL: Scalar = 3.162_277_660_168_379_5e-8;
```

#### Constant `ROOT_VSMALL`

```rust
pub const ROOT_VSMALL: Scalar = 1e-150;
```

#### Constant `GREAT`

```rust
pub const GREAT: Scalar = 1e15;
```

#### Constant `VGREAT`

```rust
pub const VGREAT: Scalar = 1e300;
```

#### Constant `ROOT_GREAT`

```rust
pub const ROOT_GREAT: Scalar = 3.162_277_660_168_379_5e7;
```

## Module `spherical_tensor`

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

```rust
pub fn tr(st: SphericalTensor) -> f64 { /* ... */ }
```

#### Function `det`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn det(st: SphericalTensor) -> f64 { /* ... */ }
```

#### Function `inv`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn inv(st: SphericalTensor) -> SphericalTensor { /* ... */ }
```

#### Function `mag_sqr`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn mag_sqr(st: SphericalTensor) -> f64 { /* ... */ }
```

#### Function `lerp`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn lerp(a: SphericalTensor, b: SphericalTensor, t: f64) -> SphericalTensor { /* ... */ }
```

## Module `vector`

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
    fn mul(self: Self, s: f64) -> Self { /* ... */ }
    ```

  - ```rust
    fn mul(self: Self, v: Vector3) -> Vector3 { /* ... */ }
    ```

  - ```rust
    fn mul(self: Self, v: Vector3) -> Tensor { /* ... */ }
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

```rust
pub fn mag_sqr(v: Vector3) -> f64 { /* ... */ }
```

#### Function `mag`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

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

```rust
pub fn lerp(a: Vector3, b: Vector3, t: f64) -> Vector3 { /* ... */ }
```

## Module `symm_tensor`

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

```rust
pub fn tr(st: SymmTensor) -> f64 { /* ... */ }
```

#### Function `det`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn det(st: SymmTensor) -> f64 { /* ... */ }
```

#### Function `inv`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn inv(st: SymmTensor) -> SymmTensor { /* ... */ }
```

#### Function `dev`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn dev(st: SymmTensor) -> SymmTensor { /* ... */ }
```

#### Function `dev2`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

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

```rust
pub fn mag_sqr(st: SymmTensor) -> f64 { /* ... */ }
```

#### Function `lerp`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn lerp(a: SymmTensor, b: SymmTensor, t: f64) -> SymmTensor { /* ... */ }
```

## Module `tensor`

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

- ```rust
  pub fn row_y(self: Self) -> Vector3 { /* ... */ }
  ```

- ```rust
  pub fn row_z(self: Self) -> Vector3 { /* ... */ }
  ```

- ```rust
  pub fn col_x(self: Self) -> Vector3 { /* ... */ }
  ```

- ```rust
  pub fn col_y(self: Self) -> Vector3 { /* ... */ }
  ```

- ```rust
  pub fn col_z(self: Self) -> Vector3 { /* ... */ }
  ```

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

```rust
pub fn tr(t: Tensor) -> f64 { /* ... */ }
```

#### Function `det`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn det(t: Tensor) -> f64 { /* ... */ }
```

#### Function `inv`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn inv(t: Tensor) -> Tensor { /* ... */ }
```

#### Function `symm`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn symm(t: Tensor) -> super::symm_tensor::SymmTensor { /* ... */ }
```

#### Function `two_symm`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn two_symm(t: Tensor) -> super::symm_tensor::SymmTensor { /* ... */ }
```

#### Function `skew`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn skew(t: Tensor) -> Tensor { /* ... */ }
```

#### Function `dev`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn dev(t: Tensor) -> Tensor { /* ... */ }
```

#### Function `dev2`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn dev2(t: Tensor) -> Tensor { /* ... */ }
```

#### Function `dev_symm`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn dev_symm(t: Tensor) -> super::symm_tensor::SymmTensor { /* ... */ }
```

#### Function `dev_two_symm`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

```rust
pub fn dev_two_symm(t: Tensor) -> super::symm_tensor::SymmTensor { /* ... */ }
```

#### Function `lerp`

**Attributes:**

- `Other("#[attr = Inline(Hint)]")`

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

### Re-exports

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

## Module `polynomial`

```rust
pub mod polynomial { /* ... */ }
```

### Modules

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

- ```rust
  pub fn log_coeff(self: &Self) -> f64 { /* ... */ }
  ```

- ```rust
  pub fn log_active(self: &Self) -> bool { /* ... */ }
  ```

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
### Re-exports

#### Re-export `RootType`

```rust
pub use roots::RootType;
```

#### Re-export `Roots`

```rust
pub use roots::Roots;
```

#### Re-export `LinearEqn`

```rust
pub use linear_eqn::LinearEqn;
```

#### Re-export `QuadraticEqn`

```rust
pub use quadratic_eqn::QuadraticEqn;
```

#### Re-export `CubicEqn`

```rust
pub use cubic_eqn::CubicEqn;
```

#### Re-export `Polynomial`

```rust
pub use polynomial::Polynomial;
```

## Module `math`

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

- ```rust
  pub fn n(self: &Self) -> usize { /* ... */ }
  ```

- ```rust
  pub fn get(self: &Self, i: usize, j: usize) -> f64 { /* ... */ }
  ```

- ```rust
  pub fn set(self: &mut Self, i: usize, j: usize, v: f64) { /* ... */ }
  ```

- ```rust
  pub fn add(self: &mut Self, i: usize, j: usize, v: f64) { /* ... */ }
  ```

- ```rust
  pub fn fill_zero(self: &mut Self) { /* ... */ }
  ```

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

#### Re-export `SquareMatrix`

```rust
pub use square_matrix::SquareMatrix;
```

#### Re-export `MatrixError`

```rust
pub use square_matrix::MatrixError;
```

## Module `ode`

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
| `config` | `super::OdeSolverConfig` |  |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(n: usize, abs_tol: f64, rel_tol: f64) -> Self { /* ... */ }
  ```

- ```rust
  pub fn solve_step(self: &mut Self, ode: &dyn OdeSystem, x: &mut f64, y: &mut Vec<f64>, dx_try: &mut f64) -> Result<(), OdeError> { /* ... */ }
  ```
  Take one adaptive step. On return `x` and `y` are updated and

- ```rust
  pub fn integrate(self: &mut Self, ode: &dyn OdeSystem, x_start: f64, x_end: f64, y: &mut Vec<f64>, dx_est: &mut f64) -> Result<(), OdeError> { /* ... */ }
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

- **Freeze**
- **From**
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
| `config` | `super::OdeSolverConfig` |  |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(n: usize, abs_tol: f64, rel_tol: f64) -> Self { /* ... */ }
  ```

- ```rust
  pub fn solve_step(self: &mut Self, ode: &dyn OdeSystem, x: &mut f64, y: &mut Vec<f64>, dx_try: &mut f64) -> Result<(), OdeError> { /* ... */ }
  ```

- ```rust
  pub fn integrate(self: &mut Self, ode: &dyn OdeSystem, x_start: f64, x_end: f64, y: &mut Vec<f64>, dx_est: &mut f64) -> Result<(), OdeError> { /* ... */ }
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

- **Freeze**
- **From**
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
| `config` | `super::OdeSolverConfig` |  |
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Methods

- ```rust
  pub fn new(n: usize, abs_tol: f64, rel_tol: f64) -> Self { /* ... */ }
  ```

- ```rust
  pub fn solve_step(self: &mut Self, ode: &dyn OdeSystem, x: &mut f64, y: &mut Vec<f64>, dx_try: &mut f64) -> Result<(), OdeError> { /* ... */ }
  ```
  One adaptive step (retries with smaller dx if error > 1).

- ```rust
  pub fn integrate(self: &mut Self, ode: &dyn OdeSystem, x_start: f64, x_end: f64, y: &mut Vec<f64>, dx_est: &mut f64) -> Result<(), OdeError> { /* ... */ }
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

- **Freeze**
- **From**
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

```rust
pub enum OdeError {
    StepSizeUnderflow,
    MaxStepsExceeded(usize),
}
```

##### Variants

###### `StepSizeUnderflow`

###### `MaxStepsExceeded`

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

- `n_eqns`
- `derivatives`: Fill `dydx` with the derivatives at `(x, y)`.

##### Provided Methods

- ```rust
  fn jacobian(self: &Self, _x: f64, _y: &[f64], _dfdx: &mut Vec<f64>, _dfdy: &mut SquareMatrix) { /* ... */ }
  ```
  Fill `dfdx` and `dfdy` with the Jacobian at `(x, y)`.

### Re-exports

#### Re-export `Euler`

```rust
pub use euler::Euler;
```

#### Re-export `Rkf45`

```rust
pub use rkf45::Rkf45;
```

#### Re-export `Rosenbrock23`

```rust
pub use rosenbrock23::Rosenbrock23;
```

## Module `interpolation`

```rust
pub mod interpolation { /* ... */ }
```

### Modules

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

### Re-exports

#### Re-export `interpolate_xy`

```rust
pub use interpolate_xy::interpolate_xy;
```

#### Re-export `interpolate_spline_xy`

```rust
pub use interpolate_spline_xy::interpolate_spline_xy;
```

## Module `thermophysics`

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

## Module `eos`

```rust
pub mod eos { /* ... */ }
```

### Modules

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

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
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

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
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
  `poly` coefficients give specific volume [m³/kg] as a polynomial in T [K].

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
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

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
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
### Re-exports

#### Re-export `traits::*`

```rust
pub use traits::*;
```

#### Re-export `perfect_gas::*`

```rust
pub use perfect_gas::*;
```

#### Re-export `rho_const::*`

```rust
pub use rho_const::*;
```

#### Re-export `ico_polynomial::*`

```rust
pub use ico_polynomial::*;
```

#### Re-export `peng_robinson::*`

```rust
pub use peng_robinson::*;
```

## Module `thermo`

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

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
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

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
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

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
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
### Re-exports

#### Re-export `traits::*`

```rust
pub use traits::*;
```

#### Re-export `h_const::*`

```rust
pub use h_const::*;
```

#### Re-export `janaf::*`

```rust
pub use janaf::*;
```

#### Re-export `h_polynomial::*`

```rust
pub use h_polynomial::*;
```

#### Re-export `h_tabulated::*`

```rust
pub use h_tabulated::*;
```

## Module `transport`

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

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
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
  Construct directly from Sutherland coefficients As [kg/(m·s·K^0.5)] and Ts [K].

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
## Module `polynomial`

```rust
pub mod polynomial { /* ... */ }
```

### Types

#### Struct `PolynomialTransport`

Polynomial transport model: μ(T) and κ(T) evaluated from `Polynomial<N>`.

Mirrors `Foam::polynomialTransport<Thermo, PolySize>` from
`src/thermophysicalModels/specie/transport/polynomial/`.

Both mu and kappa are independent polynomials in T [K], returning Pa·s and
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

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
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

#### Re-export `traits::*`

```rust
pub use traits::*;
```

#### Re-export `const_transport::*`

```rust
pub use const_transport::*;
```

#### Re-export `sutherland::*`

```rust
pub use sutherland::*;
```

#### Re-export `polynomial::*`

```rust
pub use polynomial::*;
```

#### Re-export `tabulated::*`

```rust
pub use tabulated::*;
```

## Module `fields`

```rust
pub mod fields { /* ... */ }
```

### Modules

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

- ```rust
  pub fn uniform(n: usize, value: T) -> Self { /* ... */ }
  ```

- ```rust
  pub fn from_fn</* synthetic */ impl Fn(usize) -> T: Fn(usize) -> T>(n: usize, f: impl Fn(usize) -> T) -> Self { /* ... */ }
  ```

- ```rust
  pub fn len(self: &Self) -> usize { /* ... */ }
  ```

- ```rust
  pub fn is_empty(self: &Self) -> bool { /* ... */ }
  ```

- ```rust
  pub fn as_slice(self: &Self) -> &[T] { /* ... */ }
  ```

- ```rust
  pub fn as_mut_slice(self: &mut Self) -> &mut [T] { /* ... */ }
  ```

- ```rust
  pub fn into_vec(self: Self) -> Vec<T> { /* ... */ }
  ```

- ```rust
  pub fn iter(self: &Self) -> std::slice::Iter<''_, T> { /* ... */ }
  ```

- ```rust
  pub fn iter_mut(self: &mut Self) -> std::slice::IterMut<''_, T> { /* ... */ }
  ```

- ```rust
  pub fn map<U: Clone, /* synthetic */ impl Fn(&T) -> U: Fn(&T) -> U>(self: &Self, f: impl Fn(&T) -> U) -> Field<U> { /* ... */ }
  ```

- ```rust
  pub fn zeros(n: usize) -> Self { /* ... */ }
  ```

- ```rust
  pub fn ones(n: usize) -> Self { /* ... */ }
  ```

- ```rust
  pub fn sum(self: &Self) -> f64 { /* ... */ }
  ```

- ```rust
  pub fn mean(self: &Self) -> f64 { /* ... */ }
  ```

- ```rust
  pub fn min(self: &Self) -> f64 { /* ... */ }
  ```

- ```rust
  pub fn max(self: &Self) -> f64 { /* ... */ }
  ```

- ```rust
  pub fn l2_norm(self: &Self) -> f64 { /* ... */ }
  ```

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
  Weighted sum: sum(w[i] * x[i]).

- ```rust
  pub fn zero_vec(n: usize) -> Self { /* ... */ }
  ```

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
## Module `boundary`

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

Covers the BC types required by the target solvers.  More exotic types
(inlet-outlet, total pressure, etc.) will be added when Layer 3 is
implemented.

```rust
pub enum BoundaryCondition<T: Clone> {
    FixedValue(T),
    FixedField(crate::fields::field::Field<T>),
    ZeroGradient,
    Symmetry,
    Empty,
    Calculated(crate::fields::field::Field<T>),
}
```

##### Variants

###### `FixedValue`

Dirichlet: fixed uniform value.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `T` |  |

###### `FixedField`

Dirichlet: fixed per-face values.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::fields::field::Field<T>` |  |

###### `ZeroGradient`

Neumann: zero normal gradient — boundary face value = internal adjacent value.

###### `Symmetry`

Symmetry plane — normal component zeroed.

###### `Empty`

2-D / wedge — zero-area faces; value has no physical meaning.

###### `Calculated`

Value computed by the solver and stored here (read-only from BC side).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `crate::fields::field::Field<T>` |  |

##### Implementations

###### Methods

- ```rust
  pub fn is_fixed_value(self: &Self) -> bool { /* ... */ }
  ```
  True if the BC imposes a value (Dirichlet-like).

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
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
| `bc` | `BoundaryCondition<T>` |  |
| `values` | `crate::fields::field::Field<T>` | Current face values for this patch (length == patch.size). |

##### Implementations

###### Methods

- ```rust
  pub fn fixed_value(size: usize, v: f64) -> Self { /* ... */ }
  ```

- ```rust
  pub fn zero_gradient(size: usize) -> Self { /* ... */ }
  ```

- ```rust
  pub fn empty() -> Self { /* ... */ }
  ```

- ```rust
  pub fn fixed_value_vec(size: usize, v: Vector3) -> Self { /* ... */ }
  ```

- ```rust
  pub fn zero_gradient_vec(size: usize) -> Self { /* ... */ }
  ```

- ```rust
  pub fn empty_vec() -> Self { /* ... */ }
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
| `name` | `String` |  |
| `mesh` | `std::sync::Arc<crate::mesh::fv_mesh::FvMesh>` |  |
| `internal` | `super::field::Field<T>` | Cell-centred values; length == `mesh.n_cells`. |
| `boundary` | `Vec<super::boundary::bc::PatchField<T>>` | One entry per boundary patch; `boundary[i].values` has length<br>`mesh.patches[i].size`. |

##### Implementations

###### Methods

- ```rust
  pub fn new</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, mesh: Arc<FvMesh>, internal: Field<T>, boundary: Vec<PatchField<T>>) -> Self { /* ... */ }
  ```

- ```rust
  pub fn uniform</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, mesh: Arc<FvMesh>, value: f64) -> Self { /* ... */ }
  ```
  Uniform scalar field over the entire domain.

- ```rust
  pub fn zeros</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, mesh: Arc<FvMesh>) -> Self { /* ... */ }
  ```

- ```rust
  pub fn uniform</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, mesh: Arc<FvMesh>, value: Vector3) -> Self { /* ... */ }
  ```
  Uniform vector field over the entire domain.

- ```rust
  pub fn zero</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, mesh: Arc<FvMesh>) -> Self { /* ... */ }
  ```

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

```rust
pub type VolScalarField = VolField<f64>;
```

#### Type Alias `VolVectorField`

```rust
pub type VolVectorField = VolField<crate::primitives::Vector3>;
```

#### Type Alias `VolTensorField`

```rust
pub type VolTensorField = VolField<crate::primitives::Tensor>;
```

#### Type Alias `VolSymmTensorField`

```rust
pub type VolSymmTensorField = VolField<crate::primitives::SymmTensor>;
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
| `name` | `String` |  |
| `mesh` | `std::sync::Arc<crate::mesh::fv_mesh::FvMesh>` |  |
| `internal` | `super::field::Field<T>` | Face values for all internal faces; length == `mesh.n_internal_faces`. |
| `boundary` | `Vec<super::boundary::bc::PatchField<T>>` | One entry per boundary patch; `boundary[i].values` has length<br>`mesh.patches[i].size`. |

##### Implementations

###### Methods

- ```rust
  pub fn new</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, mesh: Arc<FvMesh>, internal: Field<T>, boundary: Vec<PatchField<T>>) -> Self { /* ... */ }
  ```

- ```rust
  pub fn zeros</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, mesh: Arc<FvMesh>) -> Self { /* ... */ }
  ```

- ```rust
  pub fn uniform</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, mesh: Arc<FvMesh>, value: f64) -> Self { /* ... */ }
  ```

- ```rust
  pub fn zero</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, mesh: Arc<FvMesh>) -> Self { /* ... */ }
  ```

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

```rust
pub type SurfaceScalarField = SurfaceField<f64>;
```

#### Type Alias `SurfaceVectorField`

```rust
pub type SurfaceVectorField = SurfaceField<crate::primitives::Vector3>;
```

### Re-exports

#### Re-export `Field`

```rust
pub use field::Field;
```

#### Re-export `vol_field::*`

```rust
pub use vol_field::*;
```

#### Re-export `surface_field::*`

```rust
pub use surface_field::*;
```

#### Re-export `boundary::*`

```rust
pub use boundary::*;
```

## Module `mesh`

```rust
pub mod mesh { /* ... */ }
```

### Modules

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

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
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
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` |  |
| `start` | `usize` | Index of the first face of this patch in the global face list. |
| `size` | `usize` | Number of faces in this patch. |
| `kind` | `PatchKind` |  |

##### Implementations

###### Methods

- ```rust
  pub fn new</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>, start: usize, size: usize, kind: PatchKind) -> Self { /* ... */ }
  ```

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

- **Freeze**
- **From**
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
    Processor,
}
```

##### Variants

###### `Patch`

###### `Wall`

###### `Symmetry`

###### `Empty`

###### `Wedge`

###### `Cyclic`

###### `Processor`

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
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
| `cell_volumes` | `Vec<f64>` | Cell volumes `V[c]` [m³]. |
| `cell_centres` | `Vec<crate::primitives::Vector3>` | Cell centres `C[c]` [m]. |
| `face_area_vectors` | `Vec<crate::primitives::Vector3>` | Face area vectors `Sf[f]` [m²], pointing from owner toward neighbour<br>(or outward for boundary faces). |
| `face_areas` | `Vec<f64>` | Face area magnitudes `|Sf[f]|` [m²]. |
| `face_centres` | `Vec<crate::primitives::Vector3>` | Face centres `Cf[f]` [m]. |

##### Implementations

###### Methods

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

- ```rust
  pub fn n_cells(self: Self, n: usize) -> Self { /* ... */ }
  ```

- ```rust
  pub fn n_internal_faces(self: Self, n: usize) -> Self { /* ... */ }
  ```

- ```rust
  pub fn owner(self: Self, v: Vec<usize>) -> Self { /* ... */ }
  ```

- ```rust
  pub fn neighbour(self: Self, v: Vec<usize>) -> Self { /* ... */ }
  ```

- ```rust
  pub fn patches(self: Self, v: Vec<BoundaryPatch>) -> Self { /* ... */ }
  ```

- ```rust
  pub fn cell_volumes(self: Self, v: Vec<f64>) -> Self { /* ... */ }
  ```

- ```rust
  pub fn cell_centres(self: Self, v: Vec<Vector3>) -> Self { /* ... */ }
  ```

- ```rust
  pub fn face_area_vectors(self: Self, v: Vec<Vector3>) -> Self { /* ... */ }
  ```

- ```rust
  pub fn face_areas(self: Self, v: Vec<f64>) -> Self { /* ... */ }
  ```

- ```rust
  pub fn face_centres(self: Self, v: Vec<Vector3>) -> Self { /* ... */ }
  ```

- ```rust
  pub fn build(self: Self) -> Result<FvMesh, MeshError> { /* ... */ }
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
| `mesh_a` | `std::sync::Arc<crate::mesh::fv_mesh::FvMesh>` |  |
| `patch_a` | `usize` |  |
| `mesh_b` | `std::sync::Arc<crate::mesh::fv_mesh::FvMesh>` |  |
| `patch_b` | `usize` |  |
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

## Module `ldu_matrix`

```rust
pub mod ldu_matrix { /* ... */ }
```

### Modules

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
| `n_cells` | `usize` |  |
| `n_internal_faces` | `usize` |  |
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
| `mesh` | `std::sync::Arc<crate::mesh::fv_mesh::FvMesh>` |  |
| `ldu` | `super::ldu_matrix::LduMatrix` |  |
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
| `tolerance` | `f64` |  |
| `max_iter` | `usize` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
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
| `n_iterations` | `usize` |  |
| `final_residual` | `f64` |  |
| `converged` | `bool` |  |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Borrow**
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
| `mesh` | `std::sync::Arc<crate::mesh::fv_mesh::FvMesh>` |  |
| `ldu` | `super::ldu_matrix::LduMatrix` |  |
| `source` | `crate::fields::field::Field<crate::primitives::Vector3>` |  |

##### Implementations

###### Methods

- ```rust
  pub fn new(mesh: Arc<FvMesh>) -> Self { /* ... */ }
  ```

- ```rust
  pub fn add_to_diag(self: &mut Self, coeff: &Field<f64>) { /* ... */ }
  ```

- ```rust
  pub fn add_to_source(self: &mut Self, term: &Field<Vector3>) { /* ... */ }
  ```

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
## Module `solvers`

```rust
pub mod solvers { /* ... */ }
```

### Modules

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

A DIC-preconditioned CG ([`conjugate_gradient`](super::conjugate_gradient))
needs O(√κ) ≈ O(Nₓ) iterations on the pressure Poisson equation — the count
grows as the mesh is refined. Multigrid eliminates error at every length
scale by recursing onto coarser grids, so it converges in a handful of
V-cycles almost independently of mesh size. It is OpenFOAM's default
pressure solver for this reason.

## The algorithm (recursive correction-scheme V-cycle)

Each V-cycle is the textbook correction scheme with pre- and post-smoothing
([`GamgCycle::solve_level`]):

1. **Pre-smooth** the current level with Gauss-Seidel (`N_PRE_SWEEPS`).
2. Form the residual `r = b − A·x` and **restrict** it to the next coarser
   level (additive, [`restrict_field`]).
3. **Recurse** to compute the coarse correction; the coarsest level is
   solved directly by dense LU ([`solve_coarsest`]).
4. **Prolong** the correction back (injection, [`prolong_field`]) and add it.
5. **Post-smooth** the current level (`N_POST_SWEEPS`).

Pre- *and* post-smoothing makes this a symmetric V-cycle, which converges far
faster than a post-smoothing-only sawtooth. OpenFOAM's `GAMGSolver::Vcycle`
reaches similar robustness with `nPreSweeps = 0` plus correction *scaling*;
the symmetric form is the cleaner equivalent here.

The outer loop ([`gamg`]) repeats V-cycles until the relative residual
`‖r‖₂ / ‖b‖₂` falls below `settings.tolerance` — the same convergence metric
[`conjugate_gradient`](super::conjugate_gradient) uses, so the two solvers
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

Drop-in counterpart of [`conjugate_gradient`](super::conjugate_gradient):
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

### Re-exports

#### Re-export `gauss_seidel`

```rust
pub use gauss_seidel::gauss_seidel;
```

#### Re-export `conjugate_gradient`

```rust
pub use conjugate_gradient::conjugate_gradient;
```

#### Re-export `gamg`

```rust
pub use gamg::gamg;
```

### Re-exports

#### Re-export `LduMatrix`

```rust
pub use ldu_matrix::LduMatrix;
```

#### Re-export `FvMatrix`

```rust
pub use fv_matrix::FvMatrix;
```

#### Re-export `SolverSettings`

```rust
pub use fv_matrix::SolverSettings;
```

#### Re-export `SolverPerformance`

```rust
pub use fv_matrix::SolverPerformance;
```

#### Re-export `FvVectorMatrix`

```rust
pub use fv_vector_matrix::FvVectorMatrix;
```

#### Re-export `gauss_seidel`

```rust
pub use solvers::gauss_seidel;
```

#### Re-export `gauss_seidel`

```rust
pub use solvers::gauss_seidel;
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

## Module `fv_operators`

```rust
pub mod fv_operators { /* ... */ }
```

### Modules

## Module `fvc`

Explicit finite-volume operators — return a new field.

Usage mirrors `Foam::fvc::` from `src/finiteVolume/finiteVolume/fvc/`.

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

#### Re-export `flux`

```rust
pub use flux::flux;
```

#### Re-export `buoyancy_flux`

```rust
pub use flux::buoyancy_flux;
```

#### Re-export `grad`

```rust
pub use grad::grad;
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

```rust
pub mod fvm { /* ... */ }
```

### Re-exports

#### Re-export `ddt`

```rust
pub use ddt::ddt;
```

#### Re-export `ddt_coeff`

```rust
pub use ddt::ddt_coeff;
```

#### Re-export `ddt_vec`

```rust
pub use ddt_vec::ddt_vec;
```

#### Re-export `ddt_coeff_vec`

```rust
pub use ddt_vec::ddt_coeff_vec;
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

#### Re-export `laplacian_vec`

```rust
pub use laplacian_vec::laplacian_vec;
```

### Re-exports

#### Re-export `adjust_phi`

```rust
pub use adjust_phi::adjust_phi;
```

## Module `fluid_thermo`

```rust
pub mod fluid_thermo { /* ... */ }
```

### Modules

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

- `mesh`
- `p`: Pressure field [Pa].
- `p_mut`
- `t`: Temperature field [K].
- `rho`: Density field [kg/m³].
- `he`: Energy field — sensible enthalpy `hs` [J/kg] by default.
- `he_mut`
- `psi`: Compressibility field ψ = ∂ρ/∂p|_T [s²/m²].
- `mu`: Dynamic viscosity field μ [Pa·s] — computed on demand.
- `kappa`: Thermal conductivity field κ [W/(m·K)] — computed on demand.
- `alpha_h`: Thermal diffusivity αh = κ/Cp [kg/(m·s)] — computed on demand.
- `correct`: Recompute `T`, `ρ`, and `ψ` from `he` + `p`.
- `correct_rho`: Clamp density after the pressure equation:

##### Implementations

This trait is implemented for the following types:

- `PsiThermo<M>` with <M: TransportModel>
- `RhoThermo<M>` with <M: TransportModel>

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
| `p` | `crate::fields::vol_field::VolScalarField` |  |
| `t` | `crate::fields::vol_field::VolScalarField` |  |
| `he` | `crate::fields::vol_field::VolScalarField` | Sensible enthalpy `hs` [J/kg]. |
| `rho` | `crate::fields::vol_field::VolScalarField` |  |
| `psi` | `crate::fields::vol_field::VolScalarField` |  |

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
| `p` | `crate::fields::vol_field::VolScalarField` |  |
| `t` | `crate::fields::vol_field::VolScalarField` |  |
| `he` | `crate::fields::vol_field::VolScalarField` | Sensible enthalpy `hs` [J/kg]. |
| `rho` | `crate::fields::vol_field::VolScalarField` |  |
| `psi` | `crate::fields::vol_field::VolScalarField` | Compressibility ψ = ∂ρ/∂p|_T [s²/m²] — stored for the pressure eqn. |

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
| `t` | `crate::fields::vol_field::VolScalarField` |  |
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

- `mesh`
- `t`: Temperature field [K].
- `t_mut`
- `kappa`: Thermal conductivity κ [W/(m·K)] — used in `fvm::laplacian(kappa, T)`.
- `rho_cp`: Volumetric heat capacity ρ·Cp [J/(m³·K)] — used in `fvm::ddt(rho_cp, T)`.
- `correct`: Recompute temperature-dependent properties after T has been updated.

##### Implementations

This trait is implemented for the following types:

- `ConstSolidThermo`

### Re-exports

#### Re-export `FluidThermo`

```rust
pub use traits::FluidThermo;
```

#### Re-export `PsiThermo`

```rust
pub use psi_thermo::PsiThermo;
```

#### Re-export `RhoThermo`

```rust
pub use rho_thermo::RhoThermo;
```

#### Re-export `SolidThermo`

```rust
pub use solid_thermo::SolidThermo;
```

#### Re-export `ConstSolidThermo`

```rust
pub use solid_thermo::ConstSolidThermo;
```

## Module `prelude`

```rust
pub mod prelude { /* ... */ }
```

### Re-exports

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

#### Re-export `Euler`

```rust
pub use crate::ode::Euler;
```

#### Re-export `OdeError`

```rust
pub use crate::ode::OdeError;
```

#### Re-export `OdeSystem`

```rust
pub use crate::ode::OdeSystem;
```

#### Re-export `OdeSolverConfig`

```rust
pub use crate::ode::OdeSolverConfig;
```

#### Re-export `Rkf45`

```rust
pub use crate::ode::Rkf45;
```

#### Re-export `Rosenbrock23`

```rust
pub use crate::ode::Rosenbrock23;
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

#### Re-export `Compressibility`

```rust
pub use crate::thermophysics::quantities::Compressibility;
```

#### Re-export `ThermoError`

```rust
pub use crate::thermophysics::error::ThermoError;
```

#### Re-export `Field`

```rust
pub use crate::fields::Field;
```

#### Re-export `VolField`

```rust
pub use crate::fields::VolField;
```

#### Re-export `VolScalarField`

```rust
pub use crate::fields::VolScalarField;
```

#### Re-export `VolVectorField`

```rust
pub use crate::fields::VolVectorField;
```

#### Re-export `VolTensorField`

```rust
pub use crate::fields::VolTensorField;
```

#### Re-export `VolSymmTensorField`

```rust
pub use crate::fields::VolSymmTensorField;
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

#### Re-export `BoundaryCondition`

```rust
pub use crate::fields::BoundaryCondition;
```

#### Re-export `PatchField`

```rust
pub use crate::fields::PatchField;
```

#### Re-export `FvMesh`

```rust
pub use crate::mesh::FvMesh;
```

#### Re-export `FvMeshBuilder`

```rust
pub use crate::mesh::FvMeshBuilder;
```

#### Re-export `BoundaryPatch`

```rust
pub use crate::mesh::BoundaryPatch;
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

#### Re-export `LduMatrix`

```rust
pub use crate::ldu_matrix::LduMatrix;
```

#### Re-export `FvMatrix`

```rust
pub use crate::ldu_matrix::FvMatrix;
```

#### Re-export `FvVectorMatrix`

```rust
pub use crate::ldu_matrix::FvVectorMatrix;
```

#### Re-export `SolverSettings`

```rust
pub use crate::ldu_matrix::SolverSettings;
```

#### Re-export `SolverPerformance`

```rust
pub use crate::ldu_matrix::SolverPerformance;
```

#### Re-export `fvc`

```rust
pub use crate::fv_operators::fvc;
```

#### Re-export `fvm`

```rust
pub use crate::fv_operators::fvm;
```

#### Re-export `adjust_phi`

```rust
pub use crate::fv_operators::adjust_phi;
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

#### Re-export `ConstSolidThermo`

```rust
pub use crate::fluid_thermo::ConstSolidThermo;
```

#### Re-export `gauss_seidel`

```rust
pub use crate::ldu_matrix::gauss_seidel;
```

#### Re-export `gauss_seidel`

```rust
pub use crate::ldu_matrix::gauss_seidel;
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

#### Re-export `interface`

```rust
pub use crate::interface;
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

## Module `interface`

this part is extension in Rust 
Now under here, I want to expose the openfoam primitives to something 
that can be human readable

Also useful add-ons for the underlying libraries are put here, 
eg. generating one dimensional meshes for system code type simulations 
in TAMPINES

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

