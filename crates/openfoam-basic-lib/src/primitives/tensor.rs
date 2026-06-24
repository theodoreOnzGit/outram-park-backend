use std::ops::{Add, AddAssign, Div, Mul, MulAssign, Neg, Sub, SubAssign};

use super::spherical_tensor::SphericalTensor;
use super::symm_tensor::SymmTensor;
use super::vector::Vector3;

/// Full (non-symmetric) 3×3 tensor stored row-major.
/// Component order: xx, xy, xz, yx, yy, yz, zx, zy, zz.
/// Maps to `Foam::tensor` (`Foam::Tensor<scalar>`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Tensor {
    pub xx: f64, pub xy: f64, pub xz: f64,
    pub yx: f64, pub yy: f64, pub yz: f64,
    pub zx: f64, pub zy: f64, pub zz: f64,
}

impl Tensor {
    pub const ZERO: Self = Self {
        xx: 0.0, xy: 0.0, xz: 0.0,
        yx: 0.0, yy: 0.0, yz: 0.0,
        zx: 0.0, zy: 0.0, zz: 0.0,
    };
    pub const IDENTITY: Self = Self {
        xx: 1.0, xy: 0.0, xz: 0.0,
        yx: 0.0, yy: 1.0, yz: 0.0,
        zx: 0.0, zy: 0.0, zz: 1.0,
    };

    #[allow(clippy::too_many_arguments)]
    #[inline]
    pub fn new(
        xx: f64, xy: f64, xz: f64,
        yx: f64, yy: f64, yz: f64,
        zx: f64, zy: f64, zz: f64,
    ) -> Self {
        Self { xx, xy, xz, yx, yy, yz, zx, zy, zz }
    }

    /// Construct from three row vectors.
    #[inline]
    pub fn from_rows(x: Vector3, y: Vector3, z: Vector3) -> Self {
        Self {
            xx: x.x, xy: x.y, xz: x.z,
            yx: y.x, yy: y.y, yz: y.z,
            zx: z.x, zy: z.y, zz: z.z,
        }
    }

    /// Construct from three column vectors.
    #[inline]
    pub fn from_cols(x: Vector3, y: Vector3, z: Vector3) -> Self {
        Self {
            xx: x.x, xy: y.x, xz: z.x,
            yx: x.y, yy: y.y, yz: z.y,
            zx: x.z, zy: y.z, zz: z.z,
        }
    }

    // Row access
    #[inline] pub fn row_x(self) -> Vector3 { Vector3::new(self.xx, self.xy, self.xz) }
    #[inline] pub fn row_y(self) -> Vector3 { Vector3::new(self.yx, self.yy, self.yz) }
    #[inline] pub fn row_z(self) -> Vector3 { Vector3::new(self.zx, self.zy, self.zz) }

    // Column access
    #[inline] pub fn col_x(self) -> Vector3 { Vector3::new(self.xx, self.yx, self.zx) }
    #[inline] pub fn col_y(self) -> Vector3 { Vector3::new(self.xy, self.yy, self.zy) }
    #[inline] pub fn col_z(self) -> Vector3 { Vector3::new(self.xz, self.yz, self.zz) }

    /// Diagonal as a vector
    #[inline]
    pub fn diag(self) -> Vector3 { Vector3::new(self.xx, self.yy, self.zz) }

    /// Trace
    #[inline]
    pub fn tr(self) -> f64 { self.xx + self.yy + self.zz }

    /// Sum of squared diagonal entries (not Frobenius)
    #[inline]
    pub fn diag_sqr(self) -> f64 { self.xx * self.xx + self.yy * self.yy + self.zz * self.zz }

    /// Transpose. C++ `.T()`.
    #[inline]
    pub fn transpose(self) -> Self {
        Self {
            xx: self.xx, xy: self.yx, xz: self.zx,
            yx: self.xy, yy: self.yy, yz: self.zy,
            zx: self.xz, zy: self.yz, zz: self.zz,
        }
    }

    /// Determinant
    #[inline]
    pub fn det(self) -> f64 {
        self.xx * (self.yy * self.zz - self.yz * self.zy)
            + self.xy * (self.yz * self.zx - self.yx * self.zz)
            + self.xz * (self.yx * self.zy - self.yy * self.zx)
    }

    /// Adjunct (transpose of cofactor matrix)
    #[inline]
    pub fn adjunct(self) -> Self {
        Self {
            xx: self.yy * self.zz - self.zy * self.yz,
            xy: self.xz * self.zy - self.xy * self.zz,
            xz: self.xy * self.yz - self.xz * self.yy,

            yx: self.zx * self.yz - self.yx * self.zz,
            yy: self.xx * self.zz - self.xz * self.zx,
            yz: self.yx * self.xz - self.xx * self.yz,

            zx: self.yx * self.zy - self.yy * self.zx,
            zy: self.xy * self.zx - self.xx * self.zy,
            zz: self.xx * self.yy - self.yx * self.xy,
        }
    }

    /// Cofactor matrix = adjunct().T()
    #[inline]
    pub fn cof(self) -> Self { self.adjunct().transpose() }

    /// Inverse = adjunct / det. Panics (debug) if singular.
    #[inline]
    pub fn inv(self) -> Self {
        let d = self.det();
        debug_assert!(d.abs() > 0.0, "Tensor is singular");
        self.adjunct() / d
    }

    /// Inverse with 2-D fallback: returns ZERO if nearly singular.
    pub fn safe_inv(self) -> Self {
        use super::scalar::{ROOT_VSMALL, SMALL};
        let diag_sqr = self.diag_sqr();
        let threshold = SMALL * diag_sqr;
        let mut work = self;
        let small_xx = self.xx * self.xx < threshold;
        let small_yy = self.yy * self.yy < threshold;
        let small_zz = self.zz * self.zz < threshold;
        if small_xx { work.xx += 1.0; }
        if small_yy { work.yy += 1.0; }
        if small_zz { work.zz += 1.0; }
        let d = work.det();
        if d.abs() < ROOT_VSMALL { return Self::ZERO; }
        let mut result = work.adjunct() / d;
        if small_xx { result.xx -= 1.0; }
        if small_yy { result.yy -= 1.0; }
        if small_zz { result.zz -= 1.0; }
        result
    }

    /// Matrix multiply: `self & rhs`. C++ `operator&(Tensor, Tensor)` / `.inner(t2)`.
    #[inline]
    pub fn mat_mul(self, t: Self) -> Self {
        Self {
            xx: self.xx * t.xx + self.xy * t.yx + self.xz * t.zx,
            xy: self.xx * t.xy + self.xy * t.yy + self.xz * t.zy,
            xz: self.xx * t.xz + self.xy * t.yz + self.xz * t.zz,

            yx: self.yx * t.xx + self.yy * t.yx + self.yz * t.zx,
            yy: self.yx * t.xy + self.yy * t.yy + self.yz * t.zy,
            yz: self.yx * t.xz + self.yy * t.yz + self.yz * t.zz,

            zx: self.zx * t.xx + self.zy * t.yx + self.zz * t.zx,
            zy: self.zx * t.xy + self.zy * t.yy + self.zz * t.zy,
            zz: self.zx * t.xz + self.zy * t.yz + self.zz * t.zz,
        }
    }

    /// Element-wise product (Schur/Hadamard product).
    #[inline]
    pub fn schur(self, t: Self) -> Self {
        Self {
            xx: self.xx * t.xx, xy: self.xy * t.xy, xz: self.xz * t.xz,
            yx: self.yx * t.yx, yy: self.yy * t.yy, yz: self.yz * t.yz,
            zx: self.zx * t.zx, zy: self.zy * t.zy, zz: self.zz * t.zz,
        }
    }

    /// Matrix-vector multiply: `T · v`. C++ `operator&(Tensor, Vector)`.
    #[inline]
    pub fn mat_vec(self, v: Vector3) -> Vector3 {
        Vector3::new(
            self.xx * v.x + self.xy * v.y + self.xz * v.z,
            self.yx * v.x + self.yy * v.y + self.yz * v.z,
            self.zx * v.x + self.zy * v.y + self.zz * v.z,
        )
    }

    /// Vector-matrix multiply: `v · T`. C++ `operator&(Vector, Tensor)`.
    #[inline]
    pub fn vec_mat(v: Vector3, t: Self) -> Vector3 {
        Vector3::new(
            v.x * t.xx + v.y * t.yx + v.z * t.zx,
            v.x * t.xy + v.y * t.yy + v.z * t.zy,
            v.x * t.xz + v.y * t.yz + v.z * t.zz,
        )
    }

    /// Double contraction (full Frobenius inner product). C++ `operator&&(Tensor, Tensor)`.
    #[inline]
    pub fn double_inner(self, t: Self) -> f64 {
        self.xx * t.xx + self.xy * t.xy + self.xz * t.xz
            + self.yx * t.yx + self.yy * t.yy + self.yz * t.yz
            + self.zx * t.zx + self.zy * t.zy + self.zz * t.zz
    }

    /// Symmetric part: `0.5*(T + T^T)`. Returns `SymmTensor`.
    #[inline]
    pub fn symm(self) -> SymmTensor {
        SymmTensor {
            xx: self.xx,
            xy: 0.5 * (self.xy + self.yx),
            xz: 0.5 * (self.xz + self.zx),
            yy: self.yy,
            yz: 0.5 * (self.yz + self.zy),
            zz: self.zz,
        }
    }

    /// Twice the symmetric part: `T + T^T`. Returns `SymmTensor`.
    #[inline]
    pub fn two_symm(self) -> SymmTensor {
        SymmTensor {
            xx: 2.0 * self.xx,
            xy: self.xy + self.yx,
            xz: self.xz + self.zx,
            yy: 2.0 * self.yy,
            yz: self.yz + self.zy,
            zz: 2.0 * self.zz,
        }
    }

    /// Skew-symmetric (antisymmetric) part: `0.5*(T - T^T)`.
    #[inline]
    pub fn skew(self) -> Self {
        Self {
            xx: 0.0,
            xy:  0.5 * (self.xy - self.yx),
            xz:  0.5 * (self.xz - self.zx),
            yx:  0.5 * (self.yx - self.xy),
            yy: 0.0,
            yz:  0.5 * (self.yz - self.zy),
            zx:  0.5 * (self.zx - self.xz),
            zy:  0.5 * (self.zy - self.yz),
            zz: 0.0,
        }
    }

    /// Deviatoric part: `T - (tr/3)*I`.
    #[inline]
    pub fn dev(self) -> Self { self - SphericalTensor::new(self.tr() / 3.0) }

    /// Two-thirds deviatoric: `T - (2*tr/3)*I`.
    #[inline]
    pub fn dev2(self) -> Self { self - SphericalTensor::new(2.0 / 3.0 * self.tr()) }

    /// Deviatoric of symmetric part: `symm(T) - (tr/3)*I`. Returns `SymmTensor`.
    #[inline]
    pub fn dev_symm(self) -> SymmTensor {
        let sph = self.tr() / 3.0;
        SymmTensor {
            xx: self.xx - sph,
            xy: 0.5 * (self.xy + self.yx),
            xz: 0.5 * (self.xz + self.zx),
            yy: self.yy - sph,
            yz: 0.5 * (self.yz + self.zy),
            zz: self.zz - sph,
        }
    }

    /// Deviatoric of twice the symmetric part: `twoSymm(T) - (2*tr/3)*I`. Returns `SymmTensor`.
    #[inline]
    pub fn dev_two_symm(self) -> SymmTensor {
        let sph = 2.0 / 3.0 * self.tr();
        SymmTensor {
            xx: 2.0 * self.xx - sph,
            xy: self.xy + self.yx,
            xz: self.xz + self.zx,
            yy: 2.0 * self.yy - sph,
            yz: self.yz + self.zy,
            zz: 2.0 * self.zz - sph,
        }
    }

    /// Hodge dual as a Vector. C++ `operator*(Tensor)`.
    #[inline]
    pub fn hodge_dual(self) -> Vector3 { Vector3::new(self.yz, -self.xz, self.xy) }

    /// First invariant: trace
    #[inline]
    pub fn invariant_i(self) -> f64 { self.tr() }

    /// Second invariant: (xx*yy + yy*zz + xx*zz) - (xy*yx + yz*zy + xz*zx)
    #[inline]
    pub fn invariant_ii(self) -> f64 {
        self.xx * self.yy + self.yy * self.zz + self.xx * self.zz
            - self.xy * self.yx - self.yz * self.zy - self.xz * self.zx
    }

    /// Third invariant: determinant
    #[inline]
    pub fn invariant_iii(self) -> f64 { self.det() }

    /// True if approximately the identity.
    pub fn is_identity(self, tol: f64) -> bool {
        (self.xx - 1.0).abs() < tol && (self.yy - 1.0).abs() < tol
            && (self.zz - 1.0).abs() < tol
            && self.xy.abs() < tol && self.xz.abs() < tol
            && self.yx.abs() < tol && self.yz.abs() < tol
            && self.zx.abs() < tol && self.zy.abs() < tol
    }

    /// Linear interpolation
    #[inline]
    pub fn lerp(a: Self, b: Self, t: f64) -> Self {
        let ot = 1.0 - t;
        Self {
            xx: ot * a.xx + t * b.xx, xy: ot * a.xy + t * b.xy, xz: ot * a.xz + t * b.xz,
            yx: ot * a.yx + t * b.yx, yy: ot * a.yy + t * b.yy, yz: ot * a.yz + t * b.yz,
            zx: ot * a.zx + t * b.zx, zy: ot * a.zy + t * b.zy, zz: ot * a.zz + t * b.zz,
        }
    }
}

// --- Conversions ---

impl From<SphericalTensor> for Tensor {
    #[inline]
    fn from(st: SphericalTensor) -> Self {
        Self {
            xx: st.ii, xy: 0.0, xz: 0.0,
            yx: 0.0,   yy: st.ii, yz: 0.0,
            zx: 0.0,   zy: 0.0,   zz: st.ii,
        }
    }
}

impl From<SymmTensor> for Tensor {
    #[inline]
    fn from(st: SymmTensor) -> Self {
        Self {
            xx: st.xx, xy: st.xy, xz: st.xz,
            yx: st.xy, yy: st.yy, yz: st.yz,
            zx: st.xz, zy: st.yz, zz: st.zz,
        }
    }
}

/// Hodge dual of a Vector as a skew-symmetric Tensor. C++ `operator*(Vector)`.
pub fn hodge_dual_of_vec(v: Vector3) -> Tensor {
    Tensor {
        xx:  0.0,   xy: -v.z,  xz:  v.y,
        yx:  v.z,   yy:  0.0,  yz: -v.x,
        zx: -v.y,   zy:  v.x,  zz:  0.0,
    }
}

// --- Arithmetic operators (Tensor × Tensor, Tensor × scalar) ---

impl Neg for Tensor {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Self {
            xx: -self.xx, xy: -self.xy, xz: -self.xz,
            yx: -self.yx, yy: -self.yy, yz: -self.yz,
            zx: -self.zx, zy: -self.zy, zz: -self.zz,
        }
    }
}

impl Add for Tensor {
    type Output = Self;
    #[inline]
    fn add(self, r: Self) -> Self {
        Self {
            xx: self.xx + r.xx, xy: self.xy + r.xy, xz: self.xz + r.xz,
            yx: self.yx + r.yx, yy: self.yy + r.yy, yz: self.yz + r.yz,
            zx: self.zx + r.zx, zy: self.zy + r.zy, zz: self.zz + r.zz,
        }
    }
}

impl Sub for Tensor {
    type Output = Self;
    #[inline]
    fn sub(self, r: Self) -> Self {
        Self {
            xx: self.xx - r.xx, xy: self.xy - r.xy, xz: self.xz - r.xz,
            yx: self.yx - r.yx, yy: self.yy - r.yy, yz: self.yz - r.yz,
            zx: self.zx - r.zx, zy: self.zy - r.zy, zz: self.zz - r.zz,
        }
    }
}

impl Mul<f64> for Tensor {
    type Output = Self;
    #[inline]
    fn mul(self, s: f64) -> Self {
        Self {
            xx: self.xx * s, xy: self.xy * s, xz: self.xz * s,
            yx: self.yx * s, yy: self.yy * s, yz: self.yz * s,
            zx: self.zx * s, zy: self.zy * s, zz: self.zz * s,
        }
    }
}

impl Mul<Tensor> for f64 {
    type Output = Tensor;
    #[inline]
    fn mul(self, t: Tensor) -> Tensor { t * self }
}

impl Div<f64> for Tensor {
    type Output = Self;
    #[inline]
    fn div(self, s: f64) -> Self {
        Self {
            xx: self.xx / s, xy: self.xy / s, xz: self.xz / s,
            yx: self.yx / s, yy: self.yy / s, yz: self.yz / s,
            zx: self.zx / s, zy: self.zy / s, zz: self.zz / s,
        }
    }
}

impl AddAssign for Tensor {
    #[inline]
    fn add_assign(&mut self, r: Self) { *self = *self + r; }
}

impl SubAssign for Tensor {
    #[inline]
    fn sub_assign(&mut self, r: Self) { *self = *self - r; }
}

impl MulAssign<f64> for Tensor {
    #[inline]
    fn mul_assign(&mut self, s: f64) { *self = *self * s; }
}

// --- Mixed Tensor ± SphericalTensor ---

impl Add<SphericalTensor> for Tensor {
    type Output = Self;
    #[inline]
    fn add(self, st: SphericalTensor) -> Self {
        Self {
            xx: self.xx + st.ii, xy: self.xy, xz: self.xz,
            yx: self.yx, yy: self.yy + st.ii, yz: self.yz,
            zx: self.zx, zy: self.zy, zz: self.zz + st.ii,
        }
    }
}

impl Add<Tensor> for SphericalTensor {
    type Output = Tensor;
    #[inline]
    fn add(self, t: Tensor) -> Tensor { t + self }
}

impl Sub<SphericalTensor> for Tensor {
    type Output = Self;
    #[inline]
    fn sub(self, st: SphericalTensor) -> Self {
        Self {
            xx: self.xx - st.ii, xy: self.xy, xz: self.xz,
            yx: self.yx, yy: self.yy - st.ii, yz: self.yz,
            zx: self.zx, zy: self.zy, zz: self.zz - st.ii,
        }
    }
}

impl Sub<Tensor> for SphericalTensor {
    type Output = Tensor;
    #[inline]
    fn sub(self, t: Tensor) -> Tensor {
        Tensor {
            xx: self.ii - t.xx, xy: -t.xy, xz: -t.xz,
            yx: -t.yx, yy: self.ii - t.yy, yz: -t.yz,
            zx: -t.zx, zy: -t.zy, zz: self.ii - t.zz,
        }
    }
}

// --- Mixed Tensor ± SymmTensor ---

impl Add<SymmTensor> for Tensor {
    type Output = Self;
    #[inline]
    fn add(self, st: SymmTensor) -> Self {
        Self {
            xx: self.xx + st.xx, xy: self.xy + st.xy, xz: self.xz + st.xz,
            yx: self.yx + st.xy, yy: self.yy + st.yy, yz: self.yz + st.yz,
            zx: self.zx + st.xz, zy: self.zy + st.yz, zz: self.zz + st.zz,
        }
    }
}

impl Add<Tensor> for SymmTensor {
    type Output = Tensor;
    #[inline]
    fn add(self, t: Tensor) -> Tensor { t + self }
}

impl Sub<SymmTensor> for Tensor {
    type Output = Self;
    #[inline]
    fn sub(self, st: SymmTensor) -> Self {
        Self {
            xx: self.xx - st.xx, xy: self.xy - st.xy, xz: self.xz - st.xz,
            yx: self.yx - st.xy, yy: self.yy - st.yy, yz: self.yz - st.yz,
            zx: self.zx - st.xz, zy: self.zy - st.yz, zz: self.zz - st.zz,
        }
    }
}

impl Sub<Tensor> for SymmTensor {
    type Output = Tensor;
    #[inline]
    fn sub(self, t: Tensor) -> Tensor {
        Tensor {
            xx: self.xx - t.xx, xy: self.xy - t.xy, xz: self.xz - t.xz,
            yx: self.xy - t.yx, yy: self.yy - t.yy, yz: self.yz - t.yz,
            zx: self.xz - t.zx, zy: self.yz - t.zy, zz: self.zz - t.zz,
        }
    }
}

// SphericalTensor & Tensor → Tensor (scalar multiply of all elements)
impl Mul<Tensor> for SphericalTensor {
    type Output = Tensor;
    #[inline]
    fn mul(self, t: Tensor) -> Tensor { t * self.ii }
}

impl Mul<SphericalTensor> for Tensor {
    type Output = Self;
    #[inline]
    fn mul(self, st: SphericalTensor) -> Self { self * st.ii }
}

// SymmTensor & Tensor → Tensor (matrix multiply)
impl Mul<Tensor> for SymmTensor {
    type Output = Tensor;
    #[inline]
    fn mul(self, t: Tensor) -> Tensor {
        Tensor {
            xx: self.xx * t.xx + self.xy * t.yx + self.xz * t.zx,
            xy: self.xx * t.xy + self.xy * t.yy + self.xz * t.zy,
            xz: self.xx * t.xz + self.xy * t.yz + self.xz * t.zz,
            yx: self.xy * t.xx + self.yy * t.yx + self.yz * t.zx,
            yy: self.xy * t.xy + self.yy * t.yy + self.yz * t.zy,
            yz: self.xy * t.xz + self.yy * t.yz + self.yz * t.zz,
            zx: self.xz * t.xx + self.yz * t.yx + self.zz * t.zx,
            zy: self.xz * t.xy + self.yz * t.yy + self.zz * t.zy,
            zz: self.xz * t.xz + self.yz * t.yz + self.zz * t.zz,
        }
    }
}

impl Mul<SymmTensor> for Tensor {
    type Output = Self;
    #[inline]
    fn mul(self, st: SymmTensor) -> Self {
        Self {
            xx: self.xx * st.xx + self.xy * st.xy + self.xz * st.xz,
            xy: self.xx * st.xy + self.xy * st.yy + self.xz * st.yz,
            xz: self.xx * st.xz + self.xy * st.yz + self.xz * st.zz,
            yx: self.yx * st.xx + self.yy * st.xy + self.yz * st.xz,
            yy: self.yx * st.xy + self.yy * st.yy + self.yz * st.yz,
            yz: self.yx * st.xz + self.yy * st.yz + self.yz * st.zz,
            zx: self.zx * st.xx + self.zy * st.xy + self.zz * st.xz,
            zy: self.zx * st.xy + self.zy * st.yy + self.zz * st.yz,
            zz: self.zx * st.xz + self.zy * st.yz + self.zz * st.zz,
        }
    }
}

// SymmTensor & SymmTensor → Tensor (matrix multiply, result is NOT symmetric)
impl Mul<SymmTensor> for SymmTensor {
    type Output = Tensor;
    #[inline]
    fn mul(self, st: SymmTensor) -> Tensor {
        Tensor {
            xx: self.xx * st.xx + self.xy * st.xy + self.xz * st.xz,
            xy: self.xx * st.xy + self.xy * st.yy + self.xz * st.yz,
            xz: self.xx * st.xz + self.xy * st.yz + self.xz * st.zz,
            yx: self.xy * st.xx + self.yy * st.xy + self.yz * st.xz,
            yy: self.xy * st.xy + self.yy * st.yy + self.yz * st.yz,
            yz: self.xy * st.xz + self.yy * st.yz + self.yz * st.zz,
            zx: self.xz * st.xx + self.yz * st.xy + self.zz * st.xz,
            zy: self.xz * st.xy + self.yz * st.yy + self.zz * st.yz,
            zz: self.xz * st.xz + self.yz * st.yz + self.zz * st.zz,
        }
    }
}

// Outer product: Vector ⊗ Vector → Tensor. C++ `operator*(Vector, Vector)`.
impl Mul<Vector3> for Vector3 {
    type Output = Tensor;
    #[inline]
    fn mul(self, v: Vector3) -> Tensor {
        Tensor {
            xx: self.x * v.x, xy: self.x * v.y, xz: self.x * v.z,
            yx: self.y * v.x, yy: self.y * v.y, yz: self.y * v.z,
            zx: self.z * v.x, zy: self.z * v.y, zz: self.z * v.z,
        }
    }
}

// --- Free functions ---

#[inline]
pub fn tr(t: Tensor) -> f64 { t.tr() }

#[inline]
pub fn det(t: Tensor) -> f64 { t.det() }

#[inline]
pub fn inv(t: Tensor) -> Tensor { t.inv() }

#[inline]
pub fn symm(t: Tensor) -> SymmTensor { t.symm() }

#[inline]
pub fn two_symm(t: Tensor) -> SymmTensor { t.two_symm() }

#[inline]
pub fn skew(t: Tensor) -> Tensor { t.skew() }

#[inline]
pub fn dev(t: Tensor) -> Tensor { t.dev() }

#[inline]
pub fn dev2(t: Tensor) -> Tensor { t.dev2() }

#[inline]
pub fn dev_symm(t: Tensor) -> SymmTensor { t.dev_symm() }

#[inline]
pub fn dev_two_symm(t: Tensor) -> SymmTensor { t.dev_two_symm() }

#[inline]
pub fn lerp(a: Tensor, b: Tensor, t: f64) -> Tensor { Tensor::lerp(a, b, t) }

/// Outer product v ⊗ w. Same as `v * w` but as a named function.
#[inline]
pub fn outer(v: Vector3, w: Vector3) -> Tensor { v * w }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_properties() {
        let i = Tensor::IDENTITY;
        assert_eq!(i.tr(), 3.0);
        assert_eq!(i.det(), 1.0);
        let inv = i.inv();
        assert!((inv.xx - 1.0).abs() < 1e-14);
    }

    #[test]
    fn mat_mul_identity() {
        let i = Tensor::IDENTITY;
        let t = Tensor::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);
        assert_eq!(i.mat_mul(t), t);
        assert_eq!(t.mat_mul(i), t);
    }

    #[test]
    fn mat_vec_identity() {
        let v = Vector3::new(1.0, 2.0, 3.0);
        assert_eq!(Tensor::IDENTITY.mat_vec(v), v);
    }

    #[test]
    fn transpose() {
        let t = Tensor::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);
        let tt = t.transpose();
        assert_eq!(tt.xy, t.yx);
        assert_eq!(tt.yx, t.xy);
        assert_eq!(tt.xz, t.zx);
    }

    #[test]
    fn symm_and_skew_sum_to_original() {
        let t = Tensor::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);
        let s: Tensor = Tensor::from(t.symm());
        let sk = t.skew();
        let sum = s + sk;
        assert!((sum.xx - t.xx).abs() < 1e-14);
        assert!((sum.xy - t.xy).abs() < 1e-14);
        assert!((sum.yz - t.yz).abs() < 1e-14);
    }

    #[test]
    fn outer_product() {
        let x = Vector3::X;
        let y = Vector3::Y;
        let xy = x * y;
        assert_eq!(xy.xx, 0.0);
        assert_eq!(xy.xy, 1.0);
        assert_eq!(xy.yx, 0.0);
    }

    #[test]
    fn dev_traceless() {
        let t = Tensor::new(4.0, 1.0, 0.0, 1.0, 2.0, 0.0, 0.0, 0.0, 3.0);
        assert!((t.dev().tr()).abs() < 1e-14);
    }

    #[test]
    fn inv_roundtrip_is_identity() {
        // T · T⁻¹ should equal I (up to floating-point noise).
        let t = Tensor::new(2.0, 1.0, 0.0, 1.0, 3.0, 1.0, 0.0, 1.0, 2.0);
        assert!(t.mat_mul(t.inv()).is_identity(1e-12),
            "T · T⁻¹ is not identity");
    }

    #[test]
    fn double_inner_is_symmetric() {
        // T1:T2 == T2:T1 (double inner product / Frobenius inner product)
        let t1 = Tensor::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);
        let t2 = Tensor::new(2.0, 0.0, 1.0, -1.0, 3.0, 0.0, 2.0, 1.0, 4.0);
        let diff = (t1.double_inner(t2) - t2.double_inner(t1)).abs();
        assert!(diff < 1e-12, "double_inner asymmetry = {diff:.3e}");
    }

    #[test]
    fn dev2_is_two_thirds_trace_removed() {
        // dev2(T) = T − (2/3)·tr(T)·I (OpenFOAM convention)
        let t = Tensor::new(6.0, 1.0, 0.0, 1.0, 3.0, 0.0, 0.0, 0.0, 3.0);
        let tr = t.tr(); // 12.0
        let d = t.dev2();
        assert!((d.xx - (t.xx - 2.0 / 3.0 * tr)).abs() < 1e-14);
        assert!((d.yy - (t.yy - 2.0 / 3.0 * tr)).abs() < 1e-14);
        assert!((d.zz - (t.zz - 2.0 / 3.0 * tr)).abs() < 1e-14);
        assert!((d.xy - t.xy).abs() < 1e-14);
        // dev2 is NOT trace-free: tr(dev2) = tr(T) - 2*tr(T) = -tr(T)
        assert!((d.tr() - (-tr)).abs() < 1e-14,
            "tr(dev2) = {:.3e}, expected {:.3e}", d.tr(), -tr);
    }
}
