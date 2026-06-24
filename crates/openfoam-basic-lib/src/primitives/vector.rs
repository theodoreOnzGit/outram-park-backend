use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// 3-component vector. Maps to `Foam::vector` (`Foam::Vector<scalar>`).
/// Component layout: x, y, z.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vector3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vector3 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0, z: 0.0 };
    pub const ONE: Self = Self { x: 1.0, y: 1.0, z: 1.0 };
    pub const X: Self = Self { x: 1.0, y: 0.0, z: 0.0 };
    pub const Y: Self = Self { x: 0.0, y: 1.0, z: 0.0 };
    pub const Z: Self = Self { x: 0.0, y: 0.0, z: 1.0 };

    #[inline]
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// Squared magnitude: |v|² = x² + y² + z²
    #[inline]
    pub fn mag_sqr(self) -> f64 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    /// Magnitude: |v|
    #[inline]
    pub fn mag(self) -> f64 {
        self.mag_sqr().sqrt()
    }

    /// Squared distance to another vector
    #[inline]
    pub fn dist_sqr(self, other: Self) -> f64 {
        (other - self).mag_sqr()
    }

    /// Distance to another vector
    #[inline]
    pub fn dist(self, other: Self) -> f64 {
        self.dist_sqr(other).sqrt()
    }

    /// Dot (inner) product. C++ `operator&(Vector, Vector)`.
    #[inline]
    pub fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    /// Cross product. C++ `operator^(Vector, Vector)`.
    #[inline]
    pub fn cross(self, other: Self) -> Self {
        Self {
            x: self.y * other.z - self.z * other.y,
            y: self.z * other.x - self.x * other.z,
            z: self.x * other.y - self.y * other.x,
        }
    }

    /// Normalise to unit vector; returns zero if `|v| < tol`.
    pub fn normalise(self, tol: f64) -> Self {
        let s = self.mag();
        if s < tol { Self::ZERO } else { self / s }
    }

    /// Remove the component collinear with `unit_vec`: `self - (self·unit) * unit`.
    #[inline]
    pub fn remove_collinear(self, unit_vec: Self) -> Self {
        self - self.dot(unit_vec) * unit_vec
    }

    /// Linear interpolation: `(1-t)*a + t*b`.
    #[inline]
    pub fn lerp(a: Self, b: Self, t: f64) -> Self {
        let ot = 1.0 - t;
        Self { x: ot * a.x + t * b.x, y: ot * a.y + t * b.y, z: ot * a.z + t * b.z }
    }
}

// --- Standard arithmetic operators ---

impl Neg for Vector3 {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self { Self { x: -self.x, y: -self.y, z: -self.z } }
}

impl Add for Vector3 {
    type Output = Self;
    #[inline]
    fn add(self, r: Self) -> Self { Self { x: self.x + r.x, y: self.y + r.y, z: self.z + r.z } }
}

impl Sub for Vector3 {
    type Output = Self;
    #[inline]
    fn sub(self, r: Self) -> Self { Self { x: self.x - r.x, y: self.y - r.y, z: self.z - r.z } }
}

impl Mul<f64> for Vector3 {
    type Output = Self;
    #[inline]
    fn mul(self, s: f64) -> Self { Self { x: self.x * s, y: self.y * s, z: self.z * s } }
}

impl Mul<Vector3> for f64 {
    type Output = Vector3;
    #[inline]
    fn mul(self, v: Vector3) -> Vector3 { v * self }
}

impl Div<f64> for Vector3 {
    type Output = Self;
    #[inline]
    fn div(self, s: f64) -> Self { Self { x: self.x / s, y: self.y / s, z: self.z / s } }
}

impl AddAssign for Vector3 {
    #[inline]
    fn add_assign(&mut self, r: Self) { *self = *self + r; }
}

impl SubAssign for Vector3 {
    #[inline]
    fn sub_assign(&mut self, r: Self) { *self = *self - r; }
}

impl MulAssign<f64> for Vector3 {
    #[inline]
    fn mul_assign(&mut self, s: f64) { *self = *self * s; }
}

impl DivAssign<f64> for Vector3 {
    #[inline]
    fn div_assign(&mut self, s: f64) { *self = *self / s; }
}

// --- Free functions mirroring OpenFOAM globals ---

#[inline]
pub fn mag_sqr(v: Vector3) -> f64 { v.mag_sqr() }

#[inline]
pub fn mag(v: Vector3) -> f64 { v.mag() }

/// Dot product. C++ `operator&`.
#[inline]
pub fn dot(a: Vector3, b: Vector3) -> f64 { a.dot(b) }

/// Cross product. C++ `operator^`.
#[inline]
pub fn cross(a: Vector3, b: Vector3) -> Vector3 { a.cross(b) }

#[inline]
pub fn lerp(a: Vector3, b: Vector3, t: f64) -> Vector3 { Vector3::lerp(a, b, t) }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dot_and_cross() {
        let x = Vector3::X;
        let y = Vector3::Y;
        let z = Vector3::Z;
        assert_eq!(dot(x, x), 1.0);
        assert_eq!(dot(x, y), 0.0);
        assert_eq!(cross(x, y), z);
        assert_eq!(cross(y, z), x);
        assert_eq!(cross(z, x), y);
    }

    #[test]
    fn cross_orthogonal_to_both_inputs() {
        // For arbitrary vectors a, b: cross(a,b)·a == 0 and cross(a,b)·b == 0
        let a = Vector3::new(1.0, 2.0, 3.0);
        let b = Vector3::new(4.0, 5.0, 6.0);
        let c = cross(a, b);
        assert!(dot(c, a).abs() < 1e-14, "cross(a,b)·a = {}", dot(c, a));
        assert!(dot(c, b).abs() < 1e-14, "cross(a,b)·b = {}", dot(c, b));
    }

    #[test]
    fn mag_and_normalise() {
        let v = Vector3::new(3.0, 4.0, 0.0);
        assert_eq!(v.mag(), 5.0);
        let n = v.normalise(1e-15);
        assert!((n.mag() - 1.0).abs() < 1e-14);
    }

    #[test]
    fn lerp() {
        let a = Vector3::new(0.0, 0.0, 0.0);
        let b = Vector3::new(2.0, 4.0, 6.0);
        let m = Vector3::lerp(a, b, 0.5);
        assert_eq!(m, Vector3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn remove_collinear() {
        let v = Vector3::new(3.0, 4.0, 0.0);
        let n = v.normalise(1e-15);
        let r = v.remove_collinear(n);
        assert!(r.mag() < 1e-14);
    }
}
