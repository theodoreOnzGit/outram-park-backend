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
    /// Smallest positive distance from `r` along `u` to the sphere.
    ///
    /// Solves |o + d·u|² = R² with o = r − center. Since |u| = 1 the quadratic
    /// is d² + 2(o·u)d + (o·o − R²) = 0, so d = −k ± √(k² − c) where k = o·u and
    /// c = o·o − R². Returns the nearest root with `d > ε`, or `INFINITY` if the
    /// ray misses (discriminant < 0) or both roots are behind the particle.
    ///
    /// `coincident` (the particle is sitting on this surface, e.g. just after a
    /// boundary crossing) forces c = 0 so round-off can't reflect the tangent
    /// root back inside — the standard OpenMC treatment.
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        const EPS: f64 = 1.0e-10;
        let ox = r.x - self.x0;
        let oy = r.y - self.y0;
        let oz = r.z - self.z0;
        let k = ox * u.u + oy * u.v + oz * u.w; // o·u
        let c = if coincident { 0.0 } else { ox * ox + oy * oy + oz * oz - self.r * self.r };
        let disc = k * k - c;
        if disc < 0.0 {
            return f64::INFINITY;
        }
        let sq = disc.sqrt();
        // Roots in increasing order: (−k − sq) ≤ (−k + sq).
        let d_near = -k - sq;
        if d_near > EPS {
            d_near
        } else {
            let d_far = -k + sq;
            if d_far > EPS { d_far } else { f64::INFINITY }
        }
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
    /// Smallest positive distance from `r` along `u` to the infinite Z cylinder.
    ///
    /// Ported from OpenMC `axis_aligned_cylinder_distance<2,0,1>`
    /// (`src/surface.cpp:401`). With `a = u.u² + u.v²`, `k = Δx·u + Δy·v` and
    /// `c = Δx² + Δy² − R²` the intersections are `d = (−k ± √(k²−a·c))/a`.
    /// `coincident` (or `|c|` tiny) means the particle sits on the surface: the
    /// sign of `k` says whether it faces out (no hit) or in.
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        const FP_COINCIDENT: f64 = 1.0e-12;
        let a = u.u * u.u + u.v * u.v;
        if a == 0.0 {
            return f64::INFINITY; // travelling parallel to the axis
        }
        let dx = r.x - self.x0;
        let dy = r.y - self.y0;
        let k = dx * u.u + dy * u.v;
        let c = dx * dx + dy * dy - self.r * self.r;
        let quad = k * k - a * c;
        if quad < 0.0 {
            return f64::INFINITY; // ray misses the cylinder
        }
        let sq = quad.sqrt();
        if coincident || c.abs() < FP_COINCIDENT {
            // On the surface: one root is ~0. Facing out (k≥0) ⇒ no forward hit.
            if k >= 0.0 { f64::INFINITY } else { (-k + sq) / a }
        } else if c < 0.0 {
            // Inside: exactly one positive root, the +√ branch.
            (-k + sq) / a
        } else {
            // Outside: nearest forward root is the −√ branch, if positive.
            let d = (-k - sq) / a;
            if d < 0.0 { f64::INFINITY } else { d }
        }
    }
}

// Additional surfaces to port: XCylinder, YCylinder, XCone, YCone, ZCone,
// general Plane, Quadric, TorusX, TorusY, TorusZ.

// ── Enum dispatch over the concrete surfaces ─────────────────────────────────
//
// Per the workspace design rules (enums over trait objects), CSG navigation
// dispatches over this closed set by `match` rather than `Box<dyn Surface>`.
// The `Surface` trait above stays as the compiler-enforced contract each
// concrete surface satisfies.

/// A CSG quadric surface — the closed set the geometry navigator dispatches over.
///
/// Wraps each concrete surface struct. Maps to the OpenMC `Surface` polymorphic
/// hierarchy (`src/surface.cpp`), realised here as an enum so `match` gives
/// exhaustiveness and rust-analyzer go-to-definition on every variant.
pub enum SurfaceKind {
    XPlane(XPlane),
    YPlane(YPlane),
    ZPlane(ZPlane),
    Sphere(Sphere),
    ZCylinder(ZCylinder),
}

impl SurfaceKind {
    /// Signed surface sense at `r`: negative inside, positive outside.
    /// Delegates to the wrapped surface's [`Surface::evaluate`].
    #[inline]
    pub fn evaluate(&self, r: Position) -> f64 {
        match self {
            Self::XPlane(s) => s.evaluate(r),
            Self::YPlane(s) => s.evaluate(r),
            Self::ZPlane(s) => s.evaluate(r),
            Self::Sphere(s) => s.evaluate(r),
            Self::ZCylinder(s) => s.evaluate(r),
        }
    }

    /// Boolean sense used by cell membership: `true` = positive (outside) half-space.
    /// Mirrors OpenMC `Surface::sense` (`src/surface.cpp`), position-only form.
    #[inline]
    pub fn sense(&self, r: Position) -> bool {
        self.evaluate(r) > 0.0
    }

    /// Smallest positive distance along ray `(r, u)` to this surface, or
    /// `INFINITY` if it is not crossed. `coincident` hints `r` sits on the surface.
    #[inline]
    pub fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        match self {
            Self::XPlane(s) => s.distance(r, u, coincident),
            Self::YPlane(s) => s.distance(r, u, coincident),
            Self::ZPlane(s) => s.distance(r, u, coincident),
            Self::Sphere(s) => s.distance(r, u, coincident),
            Self::ZCylinder(s) => s.distance(r, u, coincident),
        }
    }

    /// Outward unit normal at `r` (assumes `r` lies on the surface).
    #[inline]
    pub fn normal(&self, r: Position) -> Direction {
        match self {
            Self::XPlane(s) => s.normal(r),
            Self::YPlane(s) => s.normal(r),
            Self::ZPlane(s) => s.normal(r),
            Self::Sphere(s) => s.normal(r),
            Self::ZCylinder(s) => s.normal(r),
        }
    }

    /// Specular reflection of direction `u` off this surface at `r`.
    #[inline]
    pub fn reflect(&self, r: Position, u: Direction) -> Direction {
        match self {
            Self::XPlane(s) => s.reflect(r, u),
            Self::YPlane(s) => s.reflect(r, u),
            Self::ZPlane(s) => s.reflect(r, u),
            Self::Sphere(s) => s.reflect(r, u),
            Self::ZCylinder(s) => s.reflect(r, u),
        }
    }

    /// This surface's boundary condition.
    #[inline]
    pub fn bc(&self) -> BoundaryType {
        match self {
            Self::XPlane(s) => s.bc,
            Self::YPlane(s) => s.bc,
            Self::ZPlane(s) => s.bc,
            Self::Sphere(s) => s.bc,
            Self::ZCylinder(s) => s.bc,
        }
    }
}
