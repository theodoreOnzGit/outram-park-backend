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

// ── Concrete surfaces ─────────────────────────────────────────────────────────

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

/// Smallest root `d > eps` of `a·d² + b·d + c = 0`, or `INFINITY` if none.
///
/// The shared ray-quadric intersection solver for the surfaces below whose
/// distance reduces to a general quadratic (cones and the general [`Quadric`]).
/// Degenerates to the linear solve `b·d + c = 0` when `a ≈ 0` (a ray parallel
/// to a cone's generator, or a quadric that is locally planar along `u`).
fn smallest_positive_root(a: f64, b: f64, c: f64, eps: f64) -> f64 {
    if a.abs() < 1.0e-14 {
        if b.abs() < 1.0e-14 {
            return f64::INFINITY;
        }
        let d = -c / b;
        return if d > eps { d } else { f64::INFINITY };
    }
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return f64::INFINITY;
    }
    let sq = disc.sqrt();
    let inv = 0.5 / a;
    let d1 = (-b - sq) * inv;
    let d2 = (-b + sq) * inv;
    let (lo, hi) = if d1 <= d2 { (d1, d2) } else { (d2, d1) };
    if lo > eps {
        lo
    } else if hi > eps {
        hi
    } else {
        f64::INFINITY
    }
}

/// General plane: A·x + B·y + C·z = D.
///
/// The unrestricted-orientation plane (the axis-aligned [`XPlane`]/[`YPlane`]/
/// [`ZPlane`] are the cheap special cases). Maps to OpenMC `SurfacePlane`
/// (`src/surface.cpp`). `(A, B, C)` need not be unit — [`Surface::normal`]
/// normalises them.
pub struct Plane {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub bc: BoundaryType,
}

impl Surface for Plane {
    fn evaluate(&self, r: Position) -> f64 {
        self.a * r.x + self.b * r.y + self.c * r.z - self.d
    }
    fn normal(&self, _r: Position) -> Direction {
        Direction::from_unnormalised(self.a, self.b, self.c)
    }
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        let dist_hint = if coincident { 1.0e-14 } else { 0.0 };
        let denom = self.a * u.u + self.b * u.v + self.c * u.w;
        if denom.abs() < 1.0e-14 {
            return f64::INFINITY; // ray parallel to the plane
        }
        let d = -(self.a * r.x + self.b * r.y + self.c * r.z - self.d) / denom;
        if d > dist_hint { d } else { f64::INFINITY }
    }
}

/// Infinite cylinder along the X axis: (y-y0)² + (z-z0)² = r².
///
/// The X-axis twin of [`ZCylinder`]; same intersection algebra with the radial
/// pair `(y, z)` and the parallel axis `x`. Ported from OpenMC
/// `axis_aligned_cylinder_distance<0,1,2>` (`src/surface.cpp`).
pub struct XCylinder {
    pub y0: f64,
    pub z0: f64,
    pub r: f64,
    pub bc: BoundaryType,
}

impl Surface for XCylinder {
    fn evaluate(&self, r: Position) -> f64 {
        let dy = r.y - self.y0;
        let dz = r.z - self.z0;
        dy * dy + dz * dz - self.r * self.r
    }
    fn normal(&self, r: Position) -> Direction {
        Direction::from_unnormalised(0.0, r.y - self.y0, r.z - self.z0)
    }
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        const FP_COINCIDENT: f64 = 1.0e-12;
        let a = u.v * u.v + u.w * u.w;
        if a == 0.0 {
            return f64::INFINITY;
        }
        let dy = r.y - self.y0;
        let dz = r.z - self.z0;
        let k = dy * u.v + dz * u.w;
        let c = dy * dy + dz * dz - self.r * self.r;
        let quad = k * k - a * c;
        if quad < 0.0 {
            return f64::INFINITY;
        }
        let sq = quad.sqrt();
        if coincident || c.abs() < FP_COINCIDENT {
            if k >= 0.0 { f64::INFINITY } else { (-k + sq) / a }
        } else if c < 0.0 {
            (-k + sq) / a
        } else {
            let d = (-k - sq) / a;
            if d < 0.0 { f64::INFINITY } else { d }
        }
    }
}

/// Infinite cylinder along the Y axis: (x-x0)² + (z-z0)² = r².
///
/// The Y-axis twin of [`ZCylinder`]; radial pair `(x, z)`, parallel axis `y`.
/// Ported from OpenMC `axis_aligned_cylinder_distance<1,0,2>` (`src/surface.cpp`).
pub struct YCylinder {
    pub x0: f64,
    pub z0: f64,
    pub r: f64,
    pub bc: BoundaryType,
}

impl Surface for YCylinder {
    fn evaluate(&self, r: Position) -> f64 {
        let dx = r.x - self.x0;
        let dz = r.z - self.z0;
        dx * dx + dz * dz - self.r * self.r
    }
    fn normal(&self, r: Position) -> Direction {
        Direction::from_unnormalised(r.x - self.x0, 0.0, r.z - self.z0)
    }
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        const FP_COINCIDENT: f64 = 1.0e-12;
        let a = u.u * u.u + u.w * u.w;
        if a == 0.0 {
            return f64::INFINITY;
        }
        let dx = r.x - self.x0;
        let dz = r.z - self.z0;
        let k = dx * u.u + dz * u.w;
        let c = dx * dx + dz * dz - self.r * self.r;
        let quad = k * k - a * c;
        if quad < 0.0 {
            return f64::INFINITY;
        }
        let sq = quad.sqrt();
        if coincident || c.abs() < FP_COINCIDENT {
            if k >= 0.0 { f64::INFINITY } else { (-k + sq) / a }
        } else if c < 0.0 {
            (-k + sq) / a
        } else {
            let d = (-k - sq) / a;
            if d < 0.0 { f64::INFINITY } else { d }
        }
    }
}

/// Double-napped cone about the Z axis: (x-x0)² + (y-y0)² = r_sq·(z-z0)².
///
/// `r_sq` is the **square of the slope** (tan² of the half-opening-angle), the
/// same parameterisation OpenMC `SurfaceZCone` stores (`src/surface.cpp`). The
/// surface is the full double cone (both naps); a single nap is selected in CSG
/// by intersecting with a half-space (e.g. `z > z0`).
pub struct ZCone {
    pub x0: f64,
    pub y0: f64,
    pub z0: f64,
    pub r_sq: f64,
    pub bc: BoundaryType,
}

impl Surface for ZCone {
    fn evaluate(&self, r: Position) -> f64 {
        let dx = r.x - self.x0;
        let dy = r.y - self.y0;
        let dz = r.z - self.z0;
        dx * dx + dy * dy - self.r_sq * dz * dz
    }
    fn normal(&self, r: Position) -> Direction {
        Direction::from_unnormalised(
            2.0 * (r.x - self.x0),
            2.0 * (r.y - self.y0),
            -2.0 * self.r_sq * (r.z - self.z0),
        )
    }
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        let dx = r.x - self.x0;
        let dy = r.y - self.y0;
        let dz = r.z - self.z0;
        let a = u.u * u.u + u.v * u.v - self.r_sq * u.w * u.w;
        let k = dx * u.u + dy * u.v - self.r_sq * dz * u.w; // half the linear coeff
        let c = if coincident { 0.0 } else { dx * dx + dy * dy - self.r_sq * dz * dz };
        smallest_positive_root(a, 2.0 * k, c, 1.0e-10)
    }
}

/// Double-napped cone about the X axis: (y-y0)² + (z-z0)² = r_sq·(x-x0)².
///
/// X-axis twin of [`ZCone`]; `r_sq` is the slope². Ported from OpenMC
/// `SurfaceXCone` (`src/surface.cpp`).
pub struct XCone {
    pub x0: f64,
    pub y0: f64,
    pub z0: f64,
    pub r_sq: f64,
    pub bc: BoundaryType,
}

impl Surface for XCone {
    fn evaluate(&self, r: Position) -> f64 {
        let dx = r.x - self.x0;
        let dy = r.y - self.y0;
        let dz = r.z - self.z0;
        dy * dy + dz * dz - self.r_sq * dx * dx
    }
    fn normal(&self, r: Position) -> Direction {
        Direction::from_unnormalised(
            -2.0 * self.r_sq * (r.x - self.x0),
            2.0 * (r.y - self.y0),
            2.0 * (r.z - self.z0),
        )
    }
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        let dx = r.x - self.x0;
        let dy = r.y - self.y0;
        let dz = r.z - self.z0;
        let a = u.v * u.v + u.w * u.w - self.r_sq * u.u * u.u;
        let k = dy * u.v + dz * u.w - self.r_sq * dx * u.u;
        let c = if coincident { 0.0 } else { dy * dy + dz * dz - self.r_sq * dx * dx };
        smallest_positive_root(a, 2.0 * k, c, 1.0e-10)
    }
}

/// Double-napped cone about the Y axis: (x-x0)² + (z-z0)² = r_sq·(y-y0)².
///
/// Y-axis twin of [`ZCone`]; `r_sq` is the slope². Ported from OpenMC
/// `SurfaceYCone` (`src/surface.cpp`).
pub struct YCone {
    pub x0: f64,
    pub y0: f64,
    pub z0: f64,
    pub r_sq: f64,
    pub bc: BoundaryType,
}

impl Surface for YCone {
    fn evaluate(&self, r: Position) -> f64 {
        let dx = r.x - self.x0;
        let dy = r.y - self.y0;
        let dz = r.z - self.z0;
        dx * dx + dz * dz - self.r_sq * dy * dy
    }
    fn normal(&self, r: Position) -> Direction {
        Direction::from_unnormalised(
            2.0 * (r.x - self.x0),
            -2.0 * self.r_sq * (r.y - self.y0),
            2.0 * (r.z - self.z0),
        )
    }
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        let dx = r.x - self.x0;
        let dy = r.y - self.y0;
        let dz = r.z - self.z0;
        let a = u.u * u.u + u.w * u.w - self.r_sq * u.v * u.v;
        let k = dx * u.u + dz * u.w - self.r_sq * dy * u.v;
        let c = if coincident { 0.0 } else { dx * dx + dz * dz - self.r_sq * dy * dy };
        smallest_positive_root(a, 2.0 * k, c, 1.0e-10)
    }
}

/// General quadric: A x² + B y² + C z² + D xy + E yz + F xz + G x + H y + J z + K = 0.
///
/// The most general second-order surface — every other surface here is a special
/// case, but the explicit forms above are cheaper and are preferred when the
/// geometry allows. Maps to OpenMC `SurfaceQuadric` (`src/surface.cpp`).
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

impl Surface for Quadric {
    fn evaluate(&self, r: Position) -> f64 {
        let (x, y, z) = (r.x, r.y, r.z);
        self.a * x * x
            + self.b * y * y
            + self.c * z * z
            + self.d * x * y
            + self.e * y * z
            + self.f * x * z
            + self.g * x
            + self.h * y
            + self.j * z
            + self.k
    }
    fn normal(&self, r: Position) -> Direction {
        let (x, y, z) = (r.x, r.y, r.z);
        Direction::from_unnormalised(
            2.0 * self.a * x + self.d * y + self.f * z + self.g,
            2.0 * self.b * y + self.d * x + self.e * z + self.h,
            2.0 * self.c * z + self.e * y + self.f * x + self.j,
        )
    }
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        let (x, y, z) = (r.x, r.y, r.z);
        // Substitute r + d·u into the quadric → a_q·d² + b_q·d + c_q = 0.
        let a_q = self.a * u.u * u.u
            + self.b * u.v * u.v
            + self.c * u.w * u.w
            + self.d * u.u * u.v
            + self.e * u.v * u.w
            + self.f * u.u * u.w;
        let b_q = 2.0 * self.a * x * u.u
            + 2.0 * self.b * y * u.v
            + 2.0 * self.c * z * u.w
            + self.d * (x * u.v + y * u.u)
            + self.e * (y * u.w + z * u.v)
            + self.f * (x * u.w + z * u.u)
            + self.g * u.u
            + self.h * u.v
            + self.j * u.w;
        let c_q = if coincident { 0.0 } else { self.evaluate(r) };
        smallest_positive_root(a_q, b_q, c_q, 1.0e-10)
    }
}

// Remaining to port: Torus{X,Y,Z} — these are quartic (not quadric) surfaces,
// so they need a separate quartic root solver rather than this file's quadratic
// path; tracked as follow-up work.

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
    Plane(Plane),
    Sphere(Sphere),
    XCylinder(XCylinder),
    YCylinder(YCylinder),
    ZCylinder(ZCylinder),
    XCone(XCone),
    YCone(YCone),
    ZCone(ZCone),
    Quadric(Quadric),
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
            Self::Plane(s) => s.evaluate(r),
            Self::Sphere(s) => s.evaluate(r),
            Self::XCylinder(s) => s.evaluate(r),
            Self::YCylinder(s) => s.evaluate(r),
            Self::ZCylinder(s) => s.evaluate(r),
            Self::XCone(s) => s.evaluate(r),
            Self::YCone(s) => s.evaluate(r),
            Self::ZCone(s) => s.evaluate(r),
            Self::Quadric(s) => s.evaluate(r),
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
            Self::Plane(s) => s.distance(r, u, coincident),
            Self::Sphere(s) => s.distance(r, u, coincident),
            Self::XCylinder(s) => s.distance(r, u, coincident),
            Self::YCylinder(s) => s.distance(r, u, coincident),
            Self::ZCylinder(s) => s.distance(r, u, coincident),
            Self::XCone(s) => s.distance(r, u, coincident),
            Self::YCone(s) => s.distance(r, u, coincident),
            Self::ZCone(s) => s.distance(r, u, coincident),
            Self::Quadric(s) => s.distance(r, u, coincident),
        }
    }

    /// Outward unit normal at `r` (assumes `r` lies on the surface).
    #[inline]
    pub fn normal(&self, r: Position) -> Direction {
        match self {
            Self::XPlane(s) => s.normal(r),
            Self::YPlane(s) => s.normal(r),
            Self::ZPlane(s) => s.normal(r),
            Self::Plane(s) => s.normal(r),
            Self::Sphere(s) => s.normal(r),
            Self::XCylinder(s) => s.normal(r),
            Self::YCylinder(s) => s.normal(r),
            Self::ZCylinder(s) => s.normal(r),
            Self::XCone(s) => s.normal(r),
            Self::YCone(s) => s.normal(r),
            Self::ZCone(s) => s.normal(r),
            Self::Quadric(s) => s.normal(r),
        }
    }

    /// Specular reflection of direction `u` off this surface at `r`.
    #[inline]
    pub fn reflect(&self, r: Position, u: Direction) -> Direction {
        match self {
            Self::XPlane(s) => s.reflect(r, u),
            Self::YPlane(s) => s.reflect(r, u),
            Self::ZPlane(s) => s.reflect(r, u),
            Self::Plane(s) => s.reflect(r, u),
            Self::Sphere(s) => s.reflect(r, u),
            Self::XCylinder(s) => s.reflect(r, u),
            Self::YCylinder(s) => s.reflect(r, u),
            Self::ZCylinder(s) => s.reflect(r, u),
            Self::XCone(s) => s.reflect(r, u),
            Self::YCone(s) => s.reflect(r, u),
            Self::ZCone(s) => s.reflect(r, u),
            Self::Quadric(s) => s.reflect(r, u),
        }
    }

    /// This surface's boundary condition.
    #[inline]
    pub fn bc(&self) -> BoundaryType {
        match self {
            Self::XPlane(s) => s.bc,
            Self::YPlane(s) => s.bc,
            Self::ZPlane(s) => s.bc,
            Self::Plane(s) => s.bc,
            Self::Sphere(s) => s.bc,
            Self::XCylinder(s) => s.bc,
            Self::YCylinder(s) => s.bc,
            Self::ZCylinder(s) => s.bc,
            Self::XCone(s) => s.bc,
            Self::YCone(s) => s.bc,
            Self::ZCone(s) => s.bc,
            Self::Quadric(s) => s.bc,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: f64 = 1.0e-9;
    fn close(a: f64, b: f64) -> bool { (a - b).abs() < TOL }

    // ── general Plane ──────────────────────────────────────────────────────────
    #[test]
    fn plane_sense_distance_normal() {
        // Plane x + y = 0 (a=1,b=1,c=0,d=0); normal (1,1,0)/√2.
        let p = Plane { a: 1.0, b: 1.0, c: 0.0, d: 0.0, bc: BoundaryType::Transmissive };
        assert!(p.evaluate(Position::new(1.0, 0.0, 0.0)) > 0.0); // outside (+)
        assert!(p.evaluate(Position::new(-1.0, 0.0, 0.0)) < 0.0); // inside (−)
        let n = p.normal(Position::ZERO);
        assert!(close(n.u, 1.0 / 2f64.sqrt()) && close(n.v, 1.0 / 2f64.sqrt()) && close(n.w, 0.0));
        // From (1,0,0) heading −x, reach x+y=0 at x=0 → distance 1.
        let d = p.distance(Position::new(1.0, 0.0, 0.0), Direction::new(-1.0, 0.0, 0.0), false);
        assert!(close(d, 1.0), "plane distance {d}");
        // Parallel ray never hits.
        let d2 = p.distance(Position::new(1.0, 0.0, 0.0), Direction::new(1.0, -1.0, 0.0), false);
        assert_eq!(d2, f64::INFINITY);
    }

    #[test]
    fn plane_reflect() {
        let p = Plane { a: 1.0, b: 0.0, c: 0.0, d: 0.0, bc: BoundaryType::Reflective }; // x=0
        let refl = p.reflect(Position::ZERO, Direction::new(0.6, 0.8, 0.0));
        assert!(close(refl.u, -0.6) && close(refl.v, 0.8) && close(refl.w, 0.0));
    }

    // ── X / Y cylinders ────────────────────────────────────────────────────────
    #[test]
    fn xcylinder_distance_and_axis_miss() {
        let cyl = XCylinder { y0: 0.0, z0: 0.0, r: 2.0, bc: BoundaryType::Transmissive };
        assert!(cyl.evaluate(Position::new(9.0, 0.0, 0.0)) < 0.0); // on axis → inside
        assert!(cyl.evaluate(Position::new(0.0, 5.0, 0.0)) > 0.0); // outside
        // From (0,-5,0) heading +y: hit near wall at y=-2 → distance 3.
        let d = cyl.distance(Position::new(0.0, -5.0, 0.0), Direction::new(0.0, 1.0, 0.0), false);
        assert!(close(d, 3.0), "xcyl distance {d}");
        // Axis-parallel ray never hits.
        let d2 = cyl.distance(Position::new(0.0, 0.0, 0.0), Direction::new(1.0, 0.0, 0.0), false);
        assert_eq!(d2, f64::INFINITY);
    }

    #[test]
    fn ycylinder_distance() {
        let cyl = YCylinder { x0: 0.0, z0: 0.0, r: 2.0, bc: BoundaryType::Transmissive };
        // From (-5,0,0) heading +x: hit near wall at x=-2 → distance 3.
        let d = cyl.distance(Position::new(-5.0, 0.0, 0.0), Direction::new(1.0, 0.0, 0.0), false);
        assert!(close(d, 3.0), "ycyl distance {d}");
        assert!(cyl.evaluate(Position::new(0.0, 9.0, 0.0)) < 0.0); // on axis → inside
    }

    // ── cones (45° double-napped, r_sq = 1) ──────────────────────────────────────
    #[test]
    fn zcone_sense_and_distance() {
        let cone = ZCone { x0: 0.0, y0: 0.0, z0: 0.0, r_sq: 1.0, bc: BoundaryType::Transmissive };
        assert!(cone.evaluate(Position::new(0.0, 0.0, 1.0)) < 0.0); // inside the nap
        assert!(cone.evaluate(Position::new(2.0, 0.0, 1.0)) > 0.0); // outside
        assert!(close(cone.evaluate(Position::new(1.0, 0.0, 1.0)), 0.0)); // on surface
        // From (2,0,1) heading −x: near nap at x=1 → distance 1 (far nap at x=−1 is 3).
        let d = cone.distance(Position::new(2.0, 0.0, 1.0), Direction::new(-1.0, 0.0, 0.0), false);
        assert!(close(d, 1.0), "zcone distance {d}");
    }

    #[test]
    fn zcone_coincident_skips_zero_root() {
        let cone = ZCone { x0: 0.0, y0: 0.0, z0: 0.0, r_sq: 1.0, bc: BoundaryType::Reflective };
        // On the surface at (1,0,1), heading −x: skip d≈0, next crossing (far nap) at x=−1 → 2.
        let d = cone.distance(Position::new(1.0, 0.0, 1.0), Direction::new(-1.0, 0.0, 0.0), true);
        assert!(close(d, 2.0), "coincident zcone distance {d}");
    }

    #[test]
    fn xcone_and_ycone_distance() {
        let xc = XCone { x0: 0.0, y0: 0.0, z0: 0.0, r_sq: 1.0, bc: BoundaryType::Transmissive };
        // From (1,2,0) heading −y: near nap at y=1 (x²=y²) → distance 1.
        let d = xc.distance(Position::new(1.0, 2.0, 0.0), Direction::new(0.0, -1.0, 0.0), false);
        assert!(close(d, 1.0), "xcone distance {d}");
        assert!(xc.evaluate(Position::new(1.0, 0.0, 0.0)) < 0.0); // on axis → inside

        let yc = YCone { x0: 0.0, y0: 0.0, z0: 0.0, r_sq: 1.0, bc: BoundaryType::Transmissive };
        // From (2,1,0) heading −x: near nap at x=1 → distance 1.
        let d2 = yc.distance(Position::new(2.0, 1.0, 0.0), Direction::new(-1.0, 0.0, 0.0), false);
        assert!(close(d2, 1.0), "ycone distance {d2}");
        assert!(yc.evaluate(Position::new(0.0, 1.0, 0.0)) < 0.0); // on axis → inside
    }

    // ── general Quadric reproduces a sphere ─────────────────────────────────────
    #[test]
    fn quadric_matches_sphere() {
        // x² + y² + z² − 4 = 0  ⇔  Sphere(center 0, r=2).
        let q = Quadric {
            a: 1.0, b: 1.0, c: 1.0, d: 0.0, e: 0.0, f: 0.0, g: 0.0, h: 0.0, j: 0.0, k: -4.0,
            bc: BoundaryType::Transmissive,
        };
        let s = Sphere { x0: 0.0, y0: 0.0, z0: 0.0, r: 2.0, bc: BoundaryType::Transmissive };
        for p in [Position::new(0.0, 0.0, 0.0), Position::new(3.0, 1.0, -2.0), Position::new(2.0, 0.0, 0.0)] {
            assert!(close(q.evaluate(p), s.evaluate(p)), "quadric vs sphere eval at {:?}", (p.x, p.y, p.z));
        }
        // Ray from (0,0,−5) heading +z: both give distance 3 (hit at z=−2).
        let r0 = Position::new(0.0, 0.0, -5.0);
        let u = Direction::new(0.0, 0.0, 1.0);
        assert!(close(q.distance(r0, u, false), s.distance(r0, u, false)));
        assert!(close(q.distance(r0, u, false), 3.0));
        // Normals agree (up to sign/normalisation) at a surface point.
        let sp = Position::new(2.0, 0.0, 0.0);
        let nq = q.normal(sp);
        let ns = s.normal(sp);
        assert!(close(nq.u, ns.u) && close(nq.v, ns.v) && close(nq.w, ns.w));
    }

    // ── SurfaceKind enum dispatch reaches the new variants ──────────────────────
    #[test]
    fn surfacekind_dispatch_new_variants() {
        let sk = SurfaceKind::Quadric(Quadric {
            a: 1.0, b: 1.0, c: 1.0, d: 0.0, e: 0.0, f: 0.0, g: 0.0, h: 0.0, j: 0.0, k: -4.0,
            bc: BoundaryType::Vacuum,
        });
        assert!(close(sk.distance(Position::new(0.0, 0.0, -5.0), Direction::new(0.0, 0.0, 1.0), false), 3.0));
        assert!(sk.sense(Position::new(3.0, 0.0, 0.0))); // outside r=2 sphere
        assert_eq!(sk.bc(), BoundaryType::Vacuum);
    }
}
