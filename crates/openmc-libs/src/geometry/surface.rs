/// Quadric surfaces for CSG geometry.
///
/// C++ source: `src/surface.cpp` (1422 LOC), `include/openmc/surface.h` (419 LOC).
///
/// OpenMC supports: XPlane, YPlane, ZPlane, Plane (general), XCylinder,
/// YCylinder, ZCylinder, Sphere, XCone, YCone, ZCone, Quadric, Torus{X,Y,Z}.
///
/// Each surface implements two core methods:
///   - `evaluate(r)` — signed "sense" function; negative = inside, positive = outside
///   - `distance(r, u, coincident)` — distance to surface intersection along ray
///
/// Boundary conditions: Transmissive, Vacuum, Reflective, Periodic, White.

use super::position::{Direction, Position};

/// Surface boundary condition type.  Maps to `openmc::BoundaryType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryType {
    Transmissive,
    Vacuum,
    Reflective,
    Periodic,
    White,
}

/// Trait all surfaces must implement.  Maps to the virtual `Surface` base class.
pub trait Surface: Send + Sync {
    /// Evaluate the surface equation at `r`. Negative = inside the surface.
    fn evaluate(&self, r: Position) -> f64;

    /// Smallest positive distance along ray `(r, u)` to this surface.
    /// Returns `f64::INFINITY` if no intersection.
    /// `coincident` hints that `r` is already on this surface.
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64;

    /// Outward unit normal at point `r` (assumes `r` is on the surface).
    fn normal(&self, r: Position) -> Direction;

    /// Reflect direction `u` off this surface at position `r`.
    fn reflect(&self, r: Position, u: Direction) -> Direction {
        let n = self.normal(r);
        let dot = u.u * n.u + u.v * n.v + u.w * n.w;
        Direction::new(u.u - 2.0 * dot * n.u,
                       u.v - 2.0 * dot * n.v,
                       u.w - 2.0 * dot * n.w)
    }
}

// ── Concrete surface stubs ────────────────────────────────────────────────────

/// Infinite plane perpendicular to the X axis: x = x0.
pub struct XPlane {
    pub x0: f64,
    pub bc: BoundaryType,
}

impl Surface for XPlane {
    fn evaluate(&self, r: Position) -> f64 { r.x - self.x0 }
    fn normal(&self, _r: Position) -> Direction { Direction::new(1.0, 0.0, 0.0) }
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        let dist_hint = if coincident { 1e-14 } else { 0.0 };
        if u.u.abs() < 1e-14 { return f64::INFINITY; }
        let d = (self.x0 - r.x) / u.u;
        if d > dist_hint { d } else { f64::INFINITY }
    }
}

/// Infinite plane perpendicular to the Y axis: y = y0.
pub struct YPlane { pub y0: f64, pub bc: BoundaryType }

impl Surface for YPlane {
    fn evaluate(&self, r: Position) -> f64 { r.y - self.y0 }
    fn normal(&self, _r: Position) -> Direction { Direction::new(0.0, 1.0, 0.0) }
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        let dist_hint = if coincident { 1e-14 } else { 0.0 };
        if u.v.abs() < 1e-14 { return f64::INFINITY; }
        let d = (self.y0 - r.y) / u.v;
        if d > dist_hint { d } else { f64::INFINITY }
    }
}

/// Infinite plane perpendicular to the Z axis: z = z0.
pub struct ZPlane { pub z0: f64, pub bc: BoundaryType }

impl Surface for ZPlane {
    fn evaluate(&self, r: Position) -> f64 { r.z - self.z0 }
    fn normal(&self, _r: Position) -> Direction { Direction::new(0.0, 0.0, 1.0) }
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        let dist_hint = if coincident { 1e-14 } else { 0.0 };
        if u.w.abs() < 1e-14 { return f64::INFINITY; }
        let d = (self.z0 - r.z) / u.w;
        if d > dist_hint { d } else { f64::INFINITY }
    }
}

/// Sphere: (x-x0)² + (y-y0)² + (z-z0)² = r²
pub struct Sphere {
    pub x0: f64, pub y0: f64, pub z0: f64,
    pub r: f64,
    pub bc: BoundaryType,
}

impl Surface for Sphere {
    fn evaluate(&self, r: Position) -> f64 {
        let dx = r.x - self.x0;
        let dy = r.y - self.y0;
        let dz = r.z - self.z0;
        dx*dx + dy*dy + dz*dz - self.r * self.r
    }
    fn normal(&self, r: Position) -> Direction {
        Direction::from_unnormalised(r.x - self.x0, r.y - self.y0, r.z - self.z0)
    }
    fn distance(&self, _r: Position, _u: Direction, _coincident: bool) -> f64 {
        todo!("Sphere::distance: port from src/surface.cpp")
    }
}

/// Infinite cylinder along the Z axis: (x-x0)² + (y-y0)² = r²
pub struct ZCylinder {
    pub x0: f64, pub y0: f64,
    pub r: f64,
    pub bc: BoundaryType,
}

impl Surface for ZCylinder {
    fn evaluate(&self, r: Position) -> f64 {
        let dx = r.x - self.x0;
        let dy = r.y - self.y0;
        dx*dx + dy*dy - self.r * self.r
    }
    fn normal(&self, r: Position) -> Direction {
        Direction::from_unnormalised(r.x - self.x0, r.y - self.y0, 0.0)
    }
    fn distance(&self, _r: Position, _u: Direction, _coincident: bool) -> f64 {
        todo!("ZCylinder::distance: port from src/surface.cpp")
    }
}

// Additional surfaces to port: XCylinder, YCylinder, XCone, YCone, ZCone,
// general Plane, Quadric, TorusX, TorusY, TorusZ.
