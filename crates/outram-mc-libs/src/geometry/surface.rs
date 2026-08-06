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

// ── Real-root solver for polynomials up to degree 4 ─────────────────────────
//
// The torus is a degree-4 surface, so its ray intersection is a quartic (unlike
// the quadric surfaces above whose distance reduces to a quadratic). Rather than
// Ferrari/Cardano closed forms — which are numerically delicate around their
// case splits — this uses **derivative bracketing**, which is robust and easy to
// verify: the real roots of a polynomial's derivative partition the real line
// into intervals on which the polynomial is monotonic, so each interval holds at
// most one simple root, found by a guaranteed-convergent sign-change bisection
// (then Newton-polished). The derivative's own roots are found the same way one
// degree down, bottoming out at the quadratic formula. Multiple roots (which sit
// at critical points) are detected separately by a relative-residual test.
//
// No BLAS / eigenvalue solver is used, so this stays Termux/Android-portable.

/// Evaluate a polynomial with **descending** coefficients at `x` (Horner).
/// e.g. `coeffs = [c4, c3, c2, c1, c0]` for a quartic.
fn poly_eval(coeffs: &[f64], x: f64) -> f64 {
    let mut acc = 0.0;
    for &c in coeffs {
        acc = acc * x + c;
    }
    acc
}

/// Sum of the absolute magnitudes of every Horner term — the natural scale used
/// to turn `|P(x)|` into a *relative* residual for multiple-root detection.
fn poly_abs_scale(coeffs: &[f64], x: f64) -> f64 {
    let ax = x.abs();
    let mut acc = 0.0;
    for &c in coeffs {
        acc = acc * ax + c.abs();
    }
    acc
}

/// Cauchy bound: every real root of `coeffs` (descending, `coeffs[0] != 0`)
/// satisfies `|root| <= 1 + max_i |coeffs[i] / coeffs[0]|`.
fn root_bound(coeffs: &[f64]) -> f64 {
    let lead = coeffs[0];
    let mut m = 0.0;
    for &c in &coeffs[1..] {
        let a = (c / lead).abs();
        if a > m {
            m = a;
        }
    }
    1.0 + m
}

/// Insert `r` into `out[0..n]` unless a near-duplicate is already present.
/// Returns the new count. Dedup tolerance is relative (1e-7).
fn push_unique(out: &mut [f64], n: usize, r: f64) -> usize {
    for &existing in &out[..n] {
        if (existing - r).abs() <= 1.0e-7 * (1.0 + r.abs()) {
            return n;
        }
    }
    if n < out.len() {
        out[n] = r;
        n + 1
    } else {
        n
    }
}

/// Ascending insertion sort of a tiny slice (n <= 4).
fn sort_small(s: &mut [f64]) {
    for i in 1..s.len() {
        let mut j = i;
        while j > 0 && s[j - 1] > s[j] {
            s.swap(j - 1, j);
            j -= 1;
        }
    }
}

/// Locate the single root of `coeffs` inside `[lo, hi]`, where `P(lo)` and
/// `P(hi)` are known to straddle zero. Bisection (guaranteed to converge) then
/// a few Newton steps for full-precision polish. `deriv` is `P'` (descending).
fn bisect_root(coeffs: &[f64], deriv: &[f64], lo: f64, hi: f64) -> f64 {
    let mut a = lo;
    let mut b = hi;
    let mut fa = poly_eval(coeffs, a);
    for _ in 0..100 {
        let mid = 0.5 * (a + b);
        let fm = poly_eval(coeffs, mid);
        if fm == 0.0 {
            return mid;
        }
        // Keep the sub-interval that still straddles the root.
        if (fm < 0.0) == (fa < 0.0) {
            a = mid;
            fa = fm;
        } else {
            b = mid;
        }
        if (b - a).abs() <= 1.0e-15 * (1.0 + mid.abs()) {
            break;
        }
    }
    let mut x = 0.5 * (a + b);
    for _ in 0..6 {
        let f = poly_eval(coeffs, x);
        let df = poly_eval(deriv, x);
        if df == 0.0 {
            break;
        }
        let step = f / df;
        x -= step;
        if step.abs() <= 1.0e-16 * (1.0 + x.abs()) {
            break;
        }
    }
    x
}

/// Collect the real roots of `coeffs` given its derivative `deriv` and the
/// **sorted** real critical points `crits` (roots of `deriv`). Writes them into
/// `out` (ascending) and returns the count.
///
/// The critical points split the real line into monotonic intervals; a simple
/// root lives wherever consecutive breakpoints straddle zero. Roots of even
/// multiplicity sit *at* a critical point (no sign change), so they are picked
/// up by a separate relative-residual test on each critical point.
fn collect_real_roots(coeffs: &[f64], deriv: &[f64], crits: &[f64], out: &mut [f64]) -> usize {
    // Outer bound enclosing every root and every critical point.
    let mut m = root_bound(coeffs);
    for &c in crits {
        m = m.max(c.abs());
    }
    m += 1.0;

    // Breakpoints: -m, crits (sorted), +m.  crits.len() <= 3 for a quartic.
    let mut bp = [0.0f64; 5];
    let mut nb = 0;
    bp[nb] = -m;
    nb += 1;
    for &c in crits {
        bp[nb] = c;
        nb += 1;
    }
    bp[nb] = m;
    nb += 1;

    let mut n = 0usize;
    for i in 0..nb - 1 {
        let lo = bp[i];
        let hi = bp[i + 1];
        let flo = poly_eval(coeffs, lo);
        let fhi = poly_eval(coeffs, hi);
        if (flo < 0.0 && fhi > 0.0) || (flo > 0.0 && fhi < 0.0) {
            let r = bisect_root(coeffs, deriv, lo, hi);
            n = push_unique(out, n, r);
        }
    }
    // Even-multiplicity roots coincide with critical points.
    for &c in crits {
        let residual = poly_eval(coeffs, c).abs();
        let scale = poly_abs_scale(coeffs, c);
        if residual <= 1.0e-9 * scale + 1.0e-300 {
            n = push_unique(out, n, c);
        }
    }
    sort_small(&mut out[..n]);
    n
}

/// Real roots of `a x² + b x + c = 0`, ascending, into `out` (len >= 2).
/// Degenerates to the linear solve when `a ≈ 0`.
fn quadratic_real_roots(a: f64, b: f64, c: f64, out: &mut [f64]) -> usize {
    if a.abs() < 1.0e-12 * (1.0 + b.abs() + c.abs()) {
        if b.abs() < 1.0e-12 * (1.0 + c.abs()) {
            return 0;
        }
        out[0] = -c / b;
        return 1;
    }
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return 0;
    }
    if disc == 0.0 {
        out[0] = -b / (2.0 * a);
        return 1;
    }
    let sq = disc.sqrt();
    let r1 = (-b - sq) / (2.0 * a);
    let r2 = (-b + sq) / (2.0 * a);
    out[0] = r1.min(r2);
    out[1] = r1.max(r2);
    2
}

/// Real roots of `a x³ + b x² + c x + d = 0`, ascending, into `out` (len >= 3).
/// Degenerates to [`quadratic_real_roots`] when `a ≈ 0`.
fn cubic_real_roots(a: f64, b: f64, c: f64, d: f64, out: &mut [f64]) -> usize {
    if a.abs() < 1.0e-12 * (1.0 + b.abs() + c.abs() + d.abs()) {
        return quadratic_real_roots(b, c, d, out);
    }
    let coeffs = [a, b, c, d];
    let deriv = [3.0 * a, 2.0 * b, c];
    let mut crit = [0.0f64; 2];
    let nc = quadratic_real_roots(3.0 * a, 2.0 * b, c, &mut crit); // already ascending
    collect_real_roots(&coeffs, &deriv, &crit[..nc], out)
}

/// Real roots of `a x⁴ + b x³ + c x² + d x + e = 0`, ascending, into `out`
/// (len >= 4). Degenerates to [`cubic_real_roots`] when the leading coefficient
/// `a ≈ 0` (the promised lower-degree fallback).
fn quartic_real_roots(a: f64, b: f64, c: f64, d: f64, e: f64, out: &mut [f64]) -> usize {
    if a.abs() < 1.0e-12 * (1.0 + b.abs() + c.abs() + d.abs() + e.abs()) {
        return cubic_real_roots(b, c, d, e, out);
    }
    let coeffs = [a, b, c, d, e];
    let deriv = [4.0 * a, 3.0 * b, 2.0 * c, d];
    let mut crit = [0.0f64; 3];
    let nc = cubic_real_roots(4.0 * a, 3.0 * b, 2.0 * c, d, &mut crit);
    sort_small(&mut crit[..nc]);
    collect_real_roots(&coeffs, &deriv, &crit[..nc], out)
}

/// Smallest root `d > eps` of the quartic `coeffs[0] d⁴ + … + coeffs[4] = 0`,
/// or `INFINITY` if none.
///
/// (The tori below need one extra step beyond this — filtering the *spurious*
/// roots introduced by clearing the surface equation's square root — so they
/// call [`torus_distance`] rather than this directly; this is the plain
/// smallest-positive-root entry point, exercised by the solver's own tests.)
//
// Currently referenced only from the test suite (the tori use `torus_distance`,
// which additionally filters spurious roots); kept as the documented plain
// quartic entry point and to guard the solver's public-facing contract.
#[allow(dead_code)]
fn smallest_positive_quartic_root(coeffs: [f64; 5], eps: f64) -> f64 {
    let mut roots = [0.0f64; 4];
    let n = quartic_real_roots(coeffs[0], coeffs[1], coeffs[2], coeffs[3], coeffs[4], &mut roots);
    let mut best = f64::INFINITY;
    for &r in &roots[..n] {
        if r > eps && r < best {
            best = r;
        }
    }
    best
}

// ── Torus surfaces (Torus{X,Y,Z}) ───────────────────────────────────────────
//
// A torus about an axis, centred at `(x0, y0, z0)`, has three radii:
//   - `a` — **major** radius (radius of the circle of revolution),
//   - `b` — **in-plane minor** radius (tube radius in the radial direction),
//   - `c` — **axial minor** radius (tube radius along the axis).
//
// Writing the radial pair as `(r1, r2)` and the axial displacement as `ax`, and
// `R⊥ = sqrt(r1² + r2²)`, every torus shares one implicit "sense" form
// (OpenMC `SurfaceZTorus`/`SurfaceXTorus`/`SurfaceYTorus`, `src/surface.cpp`):
//
//     f = ((R⊥ − a) / b)² + (ax / c)² − 1
//
// The three structs differ only in which Cartesian axis is the torus axis:
//   Z-torus → radial pair (x, y), axial z;
//   X-torus → radial pair (y, z), axial x;
//   Y-torus → radial pair (x, z), axial y.

/// Torus sense in the axis-independent `(r1, r2, ax)` frame (radial pair +
/// axial). Negative inside the tube, positive outside, zero on the surface.
fn torus_evaluate(r1: f64, r2: f64, ax: f64, a: f64, b: f64, c: f64) -> f64 {
    let rperp = (r1 * r1 + r2 * r2).sqrt();
    let t = (rperp - a) / b;
    let s = ax / c;
    t * t + s * s - 1.0
}

/// Gradient of the torus sense in the `(r1, r2, ax)` frame (un-normalised; the
/// caller maps the components back to Cartesian and normalises).
///
/// From `f = (R⊥ − a)²/b² + ax²/c² − 1` with `R⊥ = sqrt(r1² + r2²)`:
///   ∂f/∂r1 = 2(R⊥ − a)/b² · r1/R⊥,  ∂f/∂r2 = 2(R⊥ − a)/b² · r2/R⊥,
///   ∂f/∂ax = 2·ax/c².
/// The shared factor 2 is dropped (normalisation removes it).
fn torus_gradient(r1: f64, r2: f64, ax: f64, a: f64, b: f64, c: f64) -> (f64, f64, f64) {
    let rperp = (r1 * r1 + r2 * r2).sqrt();
    let coeff = (rperp - a) / (b * b * rperp);
    (coeff * r1, coeff * r2, ax / (c * c))
}

/// Smallest positive ray-torus distance in the `(r1, r2, ax)` frame.
///
/// **Deriving the quartic.** Substitute the ray `P(t) = start + t·u` into the
/// cleared surface equation. `f = 0` multiplied through by `b²` and rearranged
/// isolates the square root:
///
/// ```text
/// ρ²(t) + a² − b² + (b²/c²)·ax²(t)  =  2a·R⊥(t)          (call the LHS G(t))
/// ```
///
/// where `ρ²(t) = r1² + r2²` is quadratic in `t`. Squaring to remove `R⊥`
/// (`R⊥² = ρ²`) gives the quartic
///
/// ```text
/// F(t) = G(t)² − 4a²·ρ²(t) = 0.
/// ```
///
/// With `ρ²(t) = α t² + 2β t + γ`, `ax²(t) = ax_α t² + 2 ax_β t + ax_γ`,
/// `k = b²/c²`, and `G(t) = Ga t² + 2Gb t + Gc` where
///
/// ```text
/// Ga = α + k·ax_α,  Gb = β + k·ax_β,  Gc = γ + a² − b² + k·ax_γ,
/// ```
///
/// expanding `G² − 4a²ρ²` yields the coefficients below.
///
/// **Spurious roots.** Squaring also admits the branch `G(t) = −2a·R⊥(t) ≤ 0`,
/// which is not a geometric intersection. A genuine hit has `G(t) = +2a·R⊥ ≥ 0`,
/// so any real root with `G(t) < 0` is discarded. (For a ring torus, `a > b`,
/// this branch is unreachable and the filter never fires; it is exact cover for
/// horn/spindle tori where `a ≤ b`.)
///
/// `coincident` (`start` sits on the surface) discards roots at/below a small
/// positive `eps` so round-off cannot report `d ≈ 0`.
#[allow(clippy::too_many_arguments)]
fn torus_distance(
    p1: f64,
    p2: f64,
    pax: f64,
    u1: f64,
    u2: f64,
    uax: f64,
    a: f64,
    b: f64,
    c: f64,
    coincident: bool,
) -> f64 {
    let k = (b * b) / (c * c);
    let alpha = u1 * u1 + u2 * u2;
    let beta = p1 * u1 + p2 * u2;
    let gamma = p1 * p1 + p2 * p2;
    let ax_alpha = uax * uax;
    let ax_beta = pax * uax;
    let ax_gamma = pax * pax;

    let ga = alpha + k * ax_alpha;
    let gb = beta + k * ax_beta;
    let gc = gamma + a * a - b * b + k * ax_gamma;
    let a2 = a * a;

    // F(t) = (Ga t² + 2Gb t + Gc)² − 4a²(α t² + 2β t + γ)
    let c4 = ga * ga;
    let c3 = 4.0 * ga * gb;
    let c2 = 4.0 * gb * gb + 2.0 * ga * gc - 4.0 * a2 * alpha;
    let c1 = 4.0 * gb * gc - 8.0 * a2 * beta;
    let c0 = gc * gc - 4.0 * a2 * gamma;

    let mut roots = [0.0f64; 4];
    let n = quartic_real_roots(c4, c3, c2, c1, c0, &mut roots);

    let eps = if coincident { 1.0e-8 } else { 1.0e-10 };
    let mut best = f64::INFINITY;
    for &t in &roots[..n] {
        if t <= eps {
            continue;
        }
        // Reject the spurious branch G(t) = −2a·R⊥.
        let gt = ga * t * t + 2.0 * gb * t + gc;
        let tol = 1.0e-9 * (1.0 + ga * t * t + (2.0 * gb * t).abs() + gc.abs());
        if gt >= -tol && t < best {
            best = t;
        }
    }
    best
}

/// Torus about the **Z** axis, centred at `(x0, y0, z0)`.
///
/// Radial pair `(x, y)`, axial `z`:
///   `((sqrt((x−x0)² + (y−y0)²) − a) / b)² + ((z−z0) / c)² − 1 = 0`.
///
/// `a` = major radius, `b` = in-plane minor radius, `c` = axial minor radius
/// (all cm). Maps to OpenMC `SurfaceZTorus` (`src/surface.cpp`).
pub struct ZTorus {
    pub x0: f64,
    pub y0: f64,
    pub z0: f64,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub bc: BoundaryType,
}

impl Surface for ZTorus {
    fn evaluate(&self, r: Position) -> f64 {
        torus_evaluate(r.x - self.x0, r.y - self.y0, r.z - self.z0, self.a, self.b, self.c)
    }
    fn normal(&self, r: Position) -> Direction {
        let (g1, g2, gax) =
            torus_gradient(r.x - self.x0, r.y - self.y0, r.z - self.z0, self.a, self.b, self.c);
        Direction::from_unnormalised(g1, g2, gax)
    }
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        torus_distance(
            r.x - self.x0, r.y - self.y0, r.z - self.z0,
            u.u, u.v, u.w,
            self.a, self.b, self.c, coincident,
        )
    }
}

/// Torus about the **X** axis, centred at `(x0, y0, z0)`.
///
/// Radial pair `(y, z)`, axial `x`:
///   `((sqrt((y−y0)² + (z−z0)²) − a) / b)² + ((x−x0) / c)² − 1 = 0`.
/// X-axis twin of [`ZTorus`]; maps to OpenMC `SurfaceXTorus` (`src/surface.cpp`).
pub struct XTorus {
    pub x0: f64,
    pub y0: f64,
    pub z0: f64,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub bc: BoundaryType,
}

impl Surface for XTorus {
    fn evaluate(&self, r: Position) -> f64 {
        // radial pair (y, z), axial x
        torus_evaluate(r.y - self.y0, r.z - self.z0, r.x - self.x0, self.a, self.b, self.c)
    }
    fn normal(&self, r: Position) -> Direction {
        let (g1, g2, gax) =
            torus_gradient(r.y - self.y0, r.z - self.z0, r.x - self.x0, self.a, self.b, self.c);
        // (g1, g2) are ∂f/∂y, ∂f/∂z; gax is ∂f/∂x → reorder to (x, y, z).
        Direction::from_unnormalised(gax, g1, g2)
    }
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        torus_distance(
            r.y - self.y0, r.z - self.z0, r.x - self.x0,
            u.v, u.w, u.u,
            self.a, self.b, self.c, coincident,
        )
    }
}

/// Torus about the **Y** axis, centred at `(x0, y0, z0)`.
///
/// Radial pair `(x, z)`, axial `y`:
///   `((sqrt((x−x0)² + (z−z0)²) − a) / b)² + ((y−y0) / c)² − 1 = 0`.
/// Y-axis twin of [`ZTorus`]; maps to OpenMC `SurfaceYTorus` (`src/surface.cpp`).
pub struct YTorus {
    pub x0: f64,
    pub y0: f64,
    pub z0: f64,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub bc: BoundaryType,
}

impl Surface for YTorus {
    fn evaluate(&self, r: Position) -> f64 {
        // radial pair (x, z), axial y
        torus_evaluate(r.x - self.x0, r.z - self.z0, r.y - self.y0, self.a, self.b, self.c)
    }
    fn normal(&self, r: Position) -> Direction {
        let (g1, g2, gax) =
            torus_gradient(r.x - self.x0, r.z - self.z0, r.y - self.y0, self.a, self.b, self.c);
        // (g1, g2) are ∂f/∂x, ∂f/∂z; gax is ∂f/∂y → reorder to (x, y, z).
        Direction::from_unnormalised(g1, gax, g2)
    }
    fn distance(&self, r: Position, u: Direction, coincident: bool) -> f64 {
        torus_distance(
            r.x - self.x0, r.z - self.z0, r.y - self.y0,
            u.u, u.w, u.v,
            self.a, self.b, self.c, coincident,
        )
    }
}

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
    XTorus(XTorus),
    YTorus(YTorus),
    ZTorus(ZTorus),
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
            Self::XTorus(s) => s.evaluate(r),
            Self::YTorus(s) => s.evaluate(r),
            Self::ZTorus(s) => s.evaluate(r),
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
            Self::XTorus(s) => s.distance(r, u, coincident),
            Self::YTorus(s) => s.distance(r, u, coincident),
            Self::ZTorus(s) => s.distance(r, u, coincident),
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
            Self::XTorus(s) => s.normal(r),
            Self::YTorus(s) => s.normal(r),
            Self::ZTorus(s) => s.normal(r),
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
            Self::XTorus(s) => s.reflect(r, u),
            Self::YTorus(s) => s.reflect(r, u),
            Self::ZTorus(s) => s.reflect(r, u),
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
            Self::XTorus(s) => s.bc,
            Self::YTorus(s) => s.bc,
            Self::ZTorus(s) => s.bc,
        }
    }

    /// Centre `[x0, y0, z0]` (cm) and radius (cm), for surfaces that have them.
    ///
    /// `Some` for [`Sphere`] only; `None` for every other variant. Mirrors
    /// `Surface::get_center` / `Surface::get_radius` in the vendored
    /// `liangjg/openmc` `virtual_lattice` fork
    /// (`include/openmc/surface.h:88`, `src/surface.cpp:762`), where the base
    /// class returns an empty vector and only `SurfaceSphere` overrides them.
    ///
    /// Used by [`crate::geometry::virtual_lattice`] to place TRISO kernels in
    /// voxels and to test point containment.
    #[inline]
    pub fn sphere_centre_radius(&self) -> Option<([f64; 3], f64)> {
        match self {
            Self::Sphere(s) => Some(([s.x0, s.y0, s.z0], s.r)),
            _ => None,
        }
    }

    /// Does this surface overlap the axis-aligned voxel of half-extent
    /// `pitch/2` centred at `centre` (both cm)?
    ///
    /// Ported from `Surface::triso_in_mesh` in the vendored `liangjg/openmc`
    /// `virtual_lattice` fork (`src/surface.cpp`). Used by
    /// [`crate::geometry::virtual_lattice::VirtualLattice::build`] to decide
    /// bucket membership.
    ///
    /// # Only `Sphere` is implemented — and that matches upstream
    ///
    /// Upstream declares `triso_in_mesh` as a virtual on the base `Surface` and
    /// overrides it on all 15 concrete types, but **14 of those 15 overrides
    /// are five-line `return false;` stubs**. Only `SurfaceSphere`
    /// (`src/surface.cpp:724`, 42 lines) carries real logic. The arms below are
    /// written out one per variant rather than collapsed into a wildcard so the
    /// correspondence with upstream stays one-to-one and diffable, and so
    /// adding a variant forces a decision here.
    ///
    /// The practical consequence: a virtual lattice only ever accelerates
    /// spheres. Handing it a cylinder or plane registers that surface in no
    /// voxel, and a traversal will never report it — see
    /// [`crate::geometry::virtual_lattice::BuildReport::unregistered`], which
    /// makes the omission observable rather than silent.
    ///
    /// # Sphere test
    ///
    /// Standard sphere-versus-AABB check: accumulate the squared distance from
    /// the centre to the box along each axis (zero on axes where the centre
    /// lies within the slab), and compare against the radius.
    ///
    /// Upstream writes `sqrt(dis_x + dis_y + dis_z) < radius_`; this port
    /// compares `d2 < r*r` instead. The two are identical for non-negative
    /// operands, and the squared form avoids a `sqrt` in a build loop that runs
    /// 27 times per TRISO particle. The comparison stays **strict**, as
    /// upstream: a sphere exactly tangent to a voxel face is *not* registered
    /// in it.
    #[inline]
    pub fn overlaps_voxel(&self, centre: [f64; 3], pitch: [f64; 3]) -> bool {
        match self {
            Self::Sphere(s) => {
                let c = [s.x0, s.y0, s.z0];
                let mut d2 = 0.0;
                for d in 0..3 {
                    let lo = centre[d] - pitch[d] / 2.0;
                    let hi = centre[d] + pitch[d] / 2.0;
                    let gap = if c[d] < lo {
                        lo - c[d]
                    } else if c[d] > hi {
                        c[d] - hi
                    } else {
                        0.0
                    };
                    d2 += gap * gap;
                }
                d2 < s.r * s.r
            }
            // The 14 upstream stubs. Not "unimplemented here" — upstream
            // returns false for all of these too (see the doc comment above).
            Self::XPlane(_) => false,
            Self::YPlane(_) => false,
            Self::ZPlane(_) => false,
            Self::Plane(_) => false,
            Self::XCylinder(_) => false,
            Self::YCylinder(_) => false,
            Self::ZCylinder(_) => false,
            Self::XCone(_) => false,
            Self::YCone(_) => false,
            Self::ZCone(_) => false,
            Self::Quadric(_) => false,
            Self::XTorus(_) => false,
            Self::YTorus(_) => false,
            Self::ZTorus(_) => false,
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

    /// Is `target` (approximately) present among the first `n` roots?
    fn contains(roots: &[f64], n: usize, target: f64) -> bool {
        roots[..n].iter().any(|&r| (r - target).abs() < 1.0e-6)
    }

    // ── Quartic solver, tested in isolation against polynomials with known roots.
    //
    // Methodology: build each polynomial from known factors, hand-expand its
    // descending coefficients, feed them to `quartic_real_roots`, and assert the
    // returned real roots (ascending) match the analytic factor roots. This
    // exercises the solver *before* any torus geometry trusts it.

    /// Four distinct simple roots.
    /// Methodology: (d−1)(d−2)(d−3)(d−4) = d⁴ − 10d³ + 35d² − 50d + 24.
    /// Result: solver returns exactly [1, 2, 3, 4].
    #[test]
    fn quartic_four_distinct_roots() {
        let mut r = [0.0; 4];
        let n = quartic_real_roots(1.0, -10.0, 35.0, -50.0, 24.0, &mut r);
        assert_eq!(n, 4, "expected 4 roots, got {n}: {:?}", &r[..n]);
        for (i, want) in [1.0, 2.0, 3.0, 4.0].into_iter().enumerate() {
            assert!(close(r[i], want), "root {i} = {} want {want}", r[i]);
        }
    }

    /// A double root plus two simple roots.
    /// Methodology: (d−2)²(d−4)(d−6) = d⁴ − 14d³ + 68d² − 136d + 96.
    /// Result: the three distinct real roots {2, 4, 6} are all found (the
    /// double root at 2 is reported once).
    #[test]
    fn quartic_double_root() {
        let mut r = [0.0; 4];
        let n = quartic_real_roots(1.0, -14.0, 68.0, -136.0, 96.0, &mut r);
        assert!(contains(&r, n, 2.0), "missing double root 2: {:?}", &r[..n]);
        assert!(contains(&r, n, 4.0), "missing root 4: {:?}", &r[..n]);
        assert!(contains(&r, n, 6.0), "missing root 6: {:?}", &r[..n]);
    }

    /// No real roots.
    /// Methodology: (d²+1)(d²+4) = d⁴ + 5d² + 4, whose roots are all imaginary.
    /// Result: solver returns 0 real roots.
    #[test]
    fn quartic_no_real_roots() {
        let mut r = [0.0; 4];
        let n = quartic_real_roots(1.0, 0.0, 5.0, 0.0, 4.0, &mut r);
        assert_eq!(n, 0, "expected 0 real roots, got {n}: {:?}", &r[..n]);
    }

    /// Degenerate leading coefficient falls back to the cubic path.
    /// Methodology: 0·d⁴ + (d−1)(d−2)(d−3) = 0·d⁴ + d³ − 6d² + 11d − 6.
    /// Result: solver returns the cubic's roots [1, 2, 3].
    #[test]
    fn quartic_degenerate_leading_reduces_to_cubic() {
        let mut r = [0.0; 4];
        let n = quartic_real_roots(0.0, 1.0, -6.0, 11.0, -6.0, &mut r);
        assert_eq!(n, 3, "expected 3 roots, got {n}: {:?}", &r[..n]);
        for (i, want) in [1.0, 2.0, 3.0].into_iter().enumerate() {
            assert!(close(r[i], want), "root {i} = {} want {want}", r[i]);
        }
    }

    /// `smallest_positive_quartic_root` honours the `eps` gate.
    /// Methodology: on (d−1)(d−2)(d−3)(d−4), eps=1e−10 skips nothing (→1);
    /// eps=1.5 skips the root at 1 (→2). A no-real-root quartic → INFINITY.
    #[test]
    fn smallest_positive_quartic_root_gate() {
        let coeffs = [1.0, -10.0, 35.0, -50.0, 24.0];
        assert!(close(smallest_positive_quartic_root(coeffs, 1.0e-10), 1.0));
        assert!(close(smallest_positive_quartic_root(coeffs, 1.5), 2.0));
        assert_eq!(
            smallest_positive_quartic_root([1.0, 0.0, 5.0, 0.0, 4.0], 1.0e-10),
            f64::INFINITY
        );
    }

    // ── Z-torus (a=3, b=1, c=1): a circular tube of radius 1 whose centre-line
    // is the circle x²+y²=9 in the z=0 plane. All reference numbers below are
    // hand-derived from the implicit form ((R⊥−3))² + z² = 1 on R⊥=|x|, z lines.

    /// Sense function at hand-verified points.
    /// Methodology: evaluate `((R⊥−3)/1)² + (z/1)² − 1`.
    /// Results (analytic): on-surface (4,0,0),(2,0,0),(3,0,±1) → 0; the tube
    /// centre-line point (3,0,0) → −1 (deepest inside); (3,0,2) → 3, (5,0,0) → 3,
    /// the central hole (0,0,0) → 8 (all outside).
    /// Note: (3,0,0) is the tube *centre*, NOT on the surface — it evaluates to −1.
    #[test]
    fn ztorus_evaluate_analytic() {
        let t = ZTorus { x0: 0.0, y0: 0.0, z0: 0.0, a: 3.0, b: 1.0, c: 1.0, bc: BoundaryType::Transmissive };
        assert!(close(t.evaluate(Position::new(4.0, 0.0, 0.0)), 0.0));
        assert!(close(t.evaluate(Position::new(2.0, 0.0, 0.0)), 0.0));
        assert!(close(t.evaluate(Position::new(3.0, 0.0, 1.0)), 0.0));
        assert!(close(t.evaluate(Position::new(3.0, 0.0, -1.0)), 0.0));
        assert!(close(t.evaluate(Position::new(3.0, 0.0, 0.0)), -1.0)); // tube centre, inside
        assert!(close(t.evaluate(Position::new(3.0, 0.0, 2.0)), 3.0)); // outside (above tube)
        assert!(close(t.evaluate(Position::new(5.0, 0.0, 0.0)), 3.0)); // outside (radially)
        assert!(close(t.evaluate(Position::new(0.0, 0.0, 0.0)), 8.0)); // central hole
    }

    /// Ray along the x-axis crosses the tube four times.
    /// Methodology: from (10,0,0) heading −x, the tube walls on the x-axis are at
    /// x = 4, 2, −2, −4 (where (|x|−3)² = 1), i.e. distances t = 6, 8, 12, 14.
    /// The derived quartic is t⁴ − 40t³ + 580t² − 3600t + 8064 = 0.
    /// Results (analytic): solver roots = {6, 8, 12, 14}; nearest distance = 6.
    #[test]
    fn ztorus_distance_four_roots() {
        // The quartic itself has exactly the four analytic roots.
        let mut r = [0.0; 4];
        let n = quartic_real_roots(1.0, -40.0, 580.0, -3600.0, 8064.0, &mut r);
        assert_eq!(n, 4, "roots: {:?}", &r[..n]);
        for (i, want) in [6.0, 8.0, 12.0, 14.0].into_iter().enumerate() {
            assert!(close(r[i], want), "root {i} = {} want {want}", r[i]);
        }
        // The surface's distance method returns the nearest crossing.
        let t = ZTorus { x0: 0.0, y0: 0.0, z0: 0.0, a: 3.0, b: 1.0, c: 1.0, bc: BoundaryType::Transmissive };
        let d = t.distance(Position::new(10.0, 0.0, 0.0), Direction::new(-1.0, 0.0, 0.0), false);
        assert!(close(d, 6.0), "nearest distance {d}");
    }

    /// Coincident start skips the d≈0 root.
    /// Methodology: start ON the surface at (4,0,0) heading −x; crossings at
    /// x = 4,2,−2,−4 ⇒ t = 0,2,6,8. With `coincident = true` the t≈0 root is
    /// discarded. Result (analytic): nearest returned distance = 2.
    #[test]
    fn ztorus_coincident_skips_zero_root() {
        let t = ZTorus { x0: 0.0, y0: 0.0, z0: 0.0, a: 3.0, b: 1.0, c: 1.0, bc: BoundaryType::Reflective };
        let d = t.distance(Position::new(4.0, 0.0, 0.0), Direction::new(-1.0, 0.0, 0.0), true);
        assert!(close(d, 2.0), "coincident distance {d}");
    }

    /// Outward normals (gradient of the implicit form) at hand-verified points.
    /// Methodology / results (analytic): at the outer equator (4,0,0) the normal
    /// is +x; at the inner equator (2,0,0) it points toward the axis, −x; at the
    /// top of the tube (3,0,1) it is +z.
    #[test]
    fn ztorus_normal_analytic() {
        let t = ZTorus { x0: 0.0, y0: 0.0, z0: 0.0, a: 3.0, b: 1.0, c: 1.0, bc: BoundaryType::Transmissive };
        let n1 = t.normal(Position::new(4.0, 0.0, 0.0));
        assert!(close(n1.u, 1.0) && close(n1.v, 0.0) && close(n1.w, 0.0), "outer eq {:?}", (n1.u, n1.v, n1.w));
        let n2 = t.normal(Position::new(2.0, 0.0, 0.0));
        assert!(close(n2.u, -1.0) && close(n2.v, 0.0) && close(n2.w, 0.0), "inner eq {:?}", (n2.u, n2.v, n2.w));
        let n3 = t.normal(Position::new(3.0, 0.0, 1.0));
        assert!(close(n3.u, 0.0) && close(n3.v, 0.0) && close(n3.w, 1.0), "top {:?}", (n3.u, n3.v, n3.w));
    }

    /// X-torus (axis = x, major circle in the y-z plane), a=3,b=1,c=1.
    /// Methodology: on-surface point (0,4,0) → eval 0; a ray from (0,0,10)
    /// heading −z crosses the tube on the z-axis at z = 4,2,−2,−4 ⇒ nearest
    /// distance 6; the outer-equator normal at (0,4,0) is +y.
    #[test]
    fn xtorus_evaluate_distance_normal() {
        let t = XTorus { x0: 0.0, y0: 0.0, z0: 0.0, a: 3.0, b: 1.0, c: 1.0, bc: BoundaryType::Transmissive };
        assert!(close(t.evaluate(Position::new(0.0, 4.0, 0.0)), 0.0));
        assert!(close(t.evaluate(Position::new(0.0, 3.0, 0.0)), -1.0)); // tube centre
        let d = t.distance(Position::new(0.0, 0.0, 10.0), Direction::new(0.0, 0.0, -1.0), false);
        assert!(close(d, 6.0), "xtorus distance {d}");
        let n = t.normal(Position::new(0.0, 4.0, 0.0));
        assert!(close(n.u, 0.0) && close(n.v, 1.0) && close(n.w, 0.0), "xtorus normal {:?}", (n.u, n.v, n.w));
    }

    /// Y-torus (axis = y, major circle in the x-z plane), a=3,b=1,c=1.
    /// Methodology: on-surface point (4,0,0) → eval 0; a ray from (0,0,10)
    /// heading −z crosses the tube on the z-axis at z = 4,2,−2,−4 ⇒ nearest
    /// distance 6; the outer-equator normal at (4,0,0) is +x.
    #[test]
    fn ytorus_evaluate_distance_normal() {
        let t = YTorus { x0: 0.0, y0: 0.0, z0: 0.0, a: 3.0, b: 1.0, c: 1.0, bc: BoundaryType::Transmissive };
        assert!(close(t.evaluate(Position::new(4.0, 0.0, 0.0)), 0.0));
        assert!(close(t.evaluate(Position::new(3.0, 0.0, 0.0)), -1.0)); // tube centre
        let d = t.distance(Position::new(0.0, 0.0, 10.0), Direction::new(0.0, 0.0, -1.0), false);
        assert!(close(d, 6.0), "ytorus distance {d}");
        let n = t.normal(Position::new(4.0, 0.0, 0.0));
        assert!(close(n.u, 1.0) && close(n.v, 0.0) && close(n.w, 0.0), "ytorus normal {:?}", (n.u, n.v, n.w));
    }

    /// A translated (off-origin) Z-torus tracks the centre.
    /// Methodology: centre (5,0,0), a=3,b=1,c=1. From (15,0,0) heading −x the
    /// translated start is X=10, so distances are again {6,8,12,14}; nearest 6.
    #[test]
    fn ztorus_translated_center() {
        let t = ZTorus { x0: 5.0, y0: 0.0, z0: 0.0, a: 3.0, b: 1.0, c: 1.0, bc: BoundaryType::Transmissive };
        let d = t.distance(Position::new(15.0, 0.0, 0.0), Direction::new(-1.0, 0.0, 0.0), false);
        assert!(close(d, 6.0), "translated ztorus distance {d}");
    }

    /// The enum-dispatched `SurfaceKind` path reaches the torus variants.
    /// Methodology: wrap the a=3,b=1,c=1 Z-torus in `SurfaceKind::ZTorus`; the
    /// same x-axis ray gives distance 6, sense at the central hole (0,0,0) is
    /// `true` (outside), and the boundary condition round-trips.
    #[test]
    fn surfacekind_torus_dispatch() {
        let sk = SurfaceKind::ZTorus(ZTorus {
            x0: 0.0, y0: 0.0, z0: 0.0, a: 3.0, b: 1.0, c: 1.0, bc: BoundaryType::Vacuum,
        });
        let d = sk.distance(Position::new(10.0, 0.0, 0.0), Direction::new(-1.0, 0.0, 0.0), false);
        assert!(close(d, 6.0), "dispatch distance {d}");
        assert!(sk.sense(Position::new(0.0, 0.0, 0.0))); // central hole is outside
        assert!(!sk.sense(Position::new(3.0, 0.0, 0.0))); // tube centre is inside
        assert_eq!(sk.bc(), BoundaryType::Vacuum);
        let n = sk.normal(Position::new(4.0, 0.0, 0.0));
        assert!(close(n.u, 1.0) && close(n.v, 0.0) && close(n.w, 0.0));
    }
}
