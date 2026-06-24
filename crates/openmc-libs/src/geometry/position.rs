/// 3D position and direction vectors.
///
/// C++ source: `include/openmc/position.h`, `src/position.cpp`.
///
/// Units: OpenMC uses **centimetres (cm)** throughout. All `Position` values
/// are in cm; this is not enforced by the type system (raw f64) because the
/// particle tracking inner loop is performance-critical.

use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// Cartesian position in cm.  Maps to `openmc::Position`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Position {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

/// Unit direction vector (direction cosines u, v, w).  Always |d| = 1.
/// Maps to `openmc::Direction` (which is a typedef for `Position` in OpenMC).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Direction {
    pub u: f64,
    pub v: f64,
    pub w: f64,
}

impl Position {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0, z: 0.0 };

    #[inline]
    pub fn new(x: f64, y: f64, z: f64) -> Self { Self { x, y, z } }

    #[inline]
    pub fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    #[inline]
    pub fn norm_sqr(self) -> f64 { self.dot(self) }

    #[inline]
    pub fn norm(self) -> f64 { self.norm_sqr().sqrt() }

    /// Distance to another position.
    #[inline]
    pub fn distance(self, other: Self) -> f64 { (other - self).norm() }
}

impl Direction {
    /// Construct a Direction from raw components — caller must ensure |d| ≈ 1.
    #[inline]
    pub fn new(u: f64, v: f64, w: f64) -> Self { Self { u, v, w } }

    /// Normalise an arbitrary vector to obtain a unit direction.
    pub fn from_unnormalised(x: f64, y: f64, z: f64) -> Self {
        let s = (x * x + y * y + z * z).sqrt();
        Self { u: x / s, v: y / s, w: z / s }
    }

    /// Dot product with a `Position` (used for projecting displacement onto direction).
    #[inline]
    pub fn dot_pos(self, p: Position) -> f64 {
        self.u * p.x + self.v * p.y + self.w * p.z
    }
}

// --- Position arithmetic ---

impl Neg for Position {
    type Output = Self;
    #[inline] fn neg(self) -> Self { Self::new(-self.x, -self.y, -self.z) }
}
impl Add for Position {
    type Output = Self;
    #[inline] fn add(self, r: Self) -> Self { Self::new(self.x+r.x, self.y+r.y, self.z+r.z) }
}
impl Sub for Position {
    type Output = Self;
    #[inline] fn sub(self, r: Self) -> Self { Self::new(self.x-r.x, self.y-r.y, self.z-r.z) }
}
impl Mul<f64> for Position {
    type Output = Self;
    #[inline] fn mul(self, s: f64) -> Self { Self::new(self.x*s, self.y*s, self.z*s) }
}
impl Mul<Position> for f64 {
    type Output = Position;
    #[inline] fn mul(self, p: Position) -> Position { p * self }
}
impl Div<f64> for Position {
    type Output = Self;
    #[inline] fn div(self, s: f64) -> Self { Self::new(self.x/s, self.y/s, self.z/s) }
}
impl AddAssign for Position {
    #[inline] fn add_assign(&mut self, r: Self) { *self = *self + r; }
}
impl SubAssign for Position {
    #[inline] fn sub_assign(&mut self, r: Self) { *self = *self - r; }
}
impl MulAssign<f64> for Position {
    #[inline] fn mul_assign(&mut self, s: f64) { *self = *self * s; }
}
impl DivAssign<f64> for Position {
    #[inline] fn div_assign(&mut self, s: f64) { *self = *self / s; }
}

/// Advance a position by `distance` along `direction`.
///
/// Equivalent to `r + d * distance` — the core operation in particle streaming.
#[inline]
pub fn stream(pos: Position, dir: Direction, distance: f64) -> Position {
    Position::new(
        pos.x + dir.u * distance,
        pos.y + dir.v * distance,
        pos.z + dir.w * distance,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_moves_along_direction() {
        let p = Position::new(0.0, 0.0, 0.0);
        let d = Direction::new(1.0, 0.0, 0.0);
        let p2 = stream(p, d, 5.0);
        assert!((p2.x - 5.0).abs() < 1e-14);
        assert!(p2.y.abs() < 1e-14);
    }

    #[test]
    fn from_unnormalised_gives_unit_vector() {
        let d = Direction::from_unnormalised(3.0, 4.0, 0.0);
        let mag = (d.u * d.u + d.v * d.v + d.w * d.w).sqrt();
        assert!((mag - 1.0).abs() < 1e-14, "mag={mag}");
    }
}
