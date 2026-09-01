//! Ray–surface distance for all 15 [`SurfaceKind`] variants, as a batched GPU
//! kernel with a trusted CPU reference.
//!
//! (Module-level prose lives in the parent `crate::gpu` `mod.rs`; every item in
//! this file carries its own `///` documentation per the human-interface rule.)
//!
//! # What this is (op-9s8.2 — the geometric foundation for GPU CSG transport)
//!
//! [`crate::geometry::surface`] holds the **trusted `f64`** ray-intersection for
//! every CSG surface (planes, sphere, axis cylinders, axis cones, the general
//! quadric, and the three tori). This module provides the **`f32`** counterpart
//! in two forms that must agree:
//!
//! - [`surface_distance_cpu_f32`] — a scalar `f32` mirror of the `f64` logic,
//!   structured so the WGSL kernel is a line-by-line translation of it. It is the
//!   reference the GPU path is judged against, and is itself judged against the
//!   `f64` [`SurfaceKind::distance`] by the tests in this module.
//! - [`surface_distance_gpu`] — the WGSL compute path
//!   (`shaders/surface_distance.wgsl`), desktop-only (wgpu is target-gated off
//!   Android), which evaluates the same distance for a batch of query rays.
//!
//! # Encoding
//!
//! A surface is flattened to a `u32` tag (its [`SurfaceKind`] variant order,
//! `0..=14`) plus [`SURF_STRIDE`] `f32` coefficients (see [`encode_surfaces`]).
//! A batch of query rays is `(surface index, coincident flag, origin, direction)`.
//! The result is one smallest-positive distance per query, with [`MISS`] standing
//! in for `f64::INFINITY` (no crossing) so the `f32` buffer carries no infinities.
//!
//! # Precision & trust
//!
//! Per the crate `CLAUDE.md`, `f64` on the CPU is the trusted reference; this
//! `f32` path is an accelerator, compared to the reference only within a
//! tolerance, never trusted as the reference. The distance **algorithm** for
//! each surface is ported from the OpenMC-mirrored `f64` code in
//! `geometry/surface.rs` (which cites the upstream `src/surface.cpp` sites); this
//! module re-expresses it in `f32`, it does not re-derive any physics.

// UNTRUSTED AI-DRAFT: this code has been drafted with AI assistance. The CPU
// f32 mirror is verified against the trusted f64 reference by `mod tests`, and
// the WGSL is statically validated with `naga` there; the GPU *execution* path
// is only exercised on a host with a real adapter (it SKIPs on CPU-only CI).
// Human review is still required before this is promoted past "Unit Tested".

use crate::geometry::position::{Direction, Position};
use crate::geometry::surface::SurfaceKind;

/// Number of `f32` coefficients stored per surface in the flat encoding.
///
/// Sized for the widest surface, the general [`SurfaceKind::Quadric`] with 10
/// coefficients (`a,b,c,d,e,f,g,h,j,k`); every other kind uses a prefix of the
/// stride and leaves the tail unused.
pub const SURF_STRIDE: usize = 12;

/// Sentinel distance meaning "ray does not cross this surface", the `f32` stand-in
/// for the `f64::INFINITY` returned by [`SurfaceKind::distance`]. Chosen far
/// beyond any physical cm distance yet well inside `f32::MAX`, so it survives
/// arithmetic and buffer round-trips without becoming a true infinity.
pub const MISS: f32 = 1.0e30;

/// A batch of surfaces flattened for GPU upload (and the CPU mirror).
///
/// `tags[i]` is surface `i`'s [`SurfaceKind`] variant order (`0..=14`) and
/// `coeffs[i*SURF_STRIDE .. i*SURF_STRIDE + SURF_STRIDE]` its coefficients. See
/// [`encode_surfaces`] for the per-kind layout.
pub struct EncodedSurfaces {
    /// One variant tag (`0..=14`) per surface.
    pub tags: Vec<u32>,
    /// `SURF_STRIDE` coefficients per surface, row-major.
    pub coeffs: Vec<f32>,
}

impl EncodedSurfaces {
    /// Number of surfaces encoded.
    pub fn len(&self) -> usize {
        self.tags.len()
    }
    /// Whether no surfaces are encoded.
    pub fn is_empty(&self) -> bool {
        self.tags.is_empty()
    }
}

/// One query ray for [`surface_distance_cpu_f32`] / [`surface_distance_gpu`].
#[derive(Clone, Copy, Debug)]
pub struct SurfaceQuery {
    /// Index into the [`EncodedSurfaces`] to test against.
    pub surface: u32,
    /// `true` when the ray origin sits on the surface (post-crossing); forces the
    /// constant term to zero so round-off cannot report a spurious `d ≈ 0`.
    pub coincident: bool,
    /// Ray origin, cm.
    pub r: Position,
    /// Ray direction (unit), dimensionless.
    pub u: Direction,
}

/// Flatten a slice of [`SurfaceKind`] into the [`EncodedSurfaces`] GPU layout.
///
/// The tag is the variant's declaration order in [`SurfaceKind`]
/// (`XPlane=0, YPlane=1, ZPlane=2, Plane=3, Sphere=4, XCylinder=5, YCylinder=6,
/// ZCylinder=7, XCone=8, YCone=9, ZCone=10, Quadric=11, XTorus=12, YTorus=13,
/// ZTorus=14`). Coefficients are stored in the order each struct declares its
/// fields:
///
/// - planes: `XPlane→[x0]`, `YPlane→[y0]`, `ZPlane→[z0]`, `Plane→[a,b,c,d]`
/// - `Sphere→[x0,y0,z0,r]`
/// - `XCylinder→[y0,z0,r]`, `YCylinder→[x0,z0,r]`, `ZCylinder→[x0,y0,r]`
/// - cones `{X,Y,Z}Cone→[x0,y0,z0,r_sq]`
/// - `Quadric→[a,b,c,d,e,f,g,h,j,k]`
/// - tori `{X,Y,Z}Torus→[x0,y0,z0,a,b,c]`
///
/// The `bc` (boundary condition) field is intentionally not encoded — this kernel
/// computes geometric distance only.
pub fn encode_surfaces(surfaces: &[SurfaceKind]) -> EncodedSurfaces {
    let mut tags = Vec::with_capacity(surfaces.len());
    let mut coeffs = vec![0.0f32; surfaces.len() * SURF_STRIDE];
    for (i, s) in surfaces.iter().enumerate() {
        let base = i * SURF_STRIDE;
        let c = &mut coeffs[base..base + SURF_STRIDE];
        let tag: u32 = match s {
            SurfaceKind::XPlane(p) => {
                c[0] = p.x0 as f32;
                0
            }
            SurfaceKind::YPlane(p) => {
                c[0] = p.y0 as f32;
                1
            }
            SurfaceKind::ZPlane(p) => {
                c[0] = p.z0 as f32;
                2
            }
            SurfaceKind::Plane(p) => {
                c[0] = p.a as f32;
                c[1] = p.b as f32;
                c[2] = p.c as f32;
                c[3] = p.d as f32;
                3
            }
            SurfaceKind::Sphere(p) => {
                c[0] = p.x0 as f32;
                c[1] = p.y0 as f32;
                c[2] = p.z0 as f32;
                c[3] = p.r as f32;
                4
            }
            SurfaceKind::XCylinder(p) => {
                c[0] = p.y0 as f32;
                c[1] = p.z0 as f32;
                c[2] = p.r as f32;
                5
            }
            SurfaceKind::YCylinder(p) => {
                c[0] = p.x0 as f32;
                c[1] = p.z0 as f32;
                c[2] = p.r as f32;
                6
            }
            SurfaceKind::ZCylinder(p) => {
                c[0] = p.x0 as f32;
                c[1] = p.y0 as f32;
                c[2] = p.r as f32;
                7
            }
            SurfaceKind::XCone(p) => {
                c[0] = p.x0 as f32;
                c[1] = p.y0 as f32;
                c[2] = p.z0 as f32;
                c[3] = p.r_sq as f32;
                8
            }
            SurfaceKind::YCone(p) => {
                c[0] = p.x0 as f32;
                c[1] = p.y0 as f32;
                c[2] = p.z0 as f32;
                c[3] = p.r_sq as f32;
                9
            }
            SurfaceKind::ZCone(p) => {
                c[0] = p.x0 as f32;
                c[1] = p.y0 as f32;
                c[2] = p.z0 as f32;
                c[3] = p.r_sq as f32;
                10
            }
            SurfaceKind::Quadric(p) => {
                c[0] = p.a as f32;
                c[1] = p.b as f32;
                c[2] = p.c as f32;
                c[3] = p.d as f32;
                c[4] = p.e as f32;
                c[5] = p.f as f32;
                c[6] = p.g as f32;
                c[7] = p.h as f32;
                c[8] = p.j as f32;
                c[9] = p.k as f32;
                11
            }
            SurfaceKind::XTorus(p) => {
                c[0] = p.x0 as f32;
                c[1] = p.y0 as f32;
                c[2] = p.z0 as f32;
                c[3] = p.a as f32;
                c[4] = p.b as f32;
                c[5] = p.c as f32;
                12
            }
            SurfaceKind::YTorus(p) => {
                c[0] = p.x0 as f32;
                c[1] = p.y0 as f32;
                c[2] = p.z0 as f32;
                c[3] = p.a as f32;
                c[4] = p.b as f32;
                c[5] = p.c as f32;
                13
            }
            SurfaceKind::ZTorus(p) => {
                c[0] = p.x0 as f32;
                c[1] = p.y0 as f32;
                c[2] = p.z0 as f32;
                c[3] = p.a as f32;
                c[4] = p.b as f32;
                c[5] = p.c as f32;
                14
            }
        };
        tags.push(tag);
    }
    EncodedSurfaces { tags, coeffs }
}

// ───────────────────────── f32 CPU mirror (WGSL twin) ──────────────────────────

/// `f32::EPSILON`, spelled as a literal so the WGSL twin can carry the identical
/// constant (WGSL has no `f32::EPSILON`).
const F32_EPS: f32 = 1.1920929e-7;

/// Safety factor on the estimated rounding noise of the cancelled `c0` term in
/// [`torus_distance_f32`]. `ulp(x)` is at most `x * F32_EPS`, and the FMA-vs-
/// two-rounding difference is bounded by about half an ULP of the larger operand;
/// 4 ULPs leaves margin for the multiplications feeding it without approaching the
/// magnitude of a physically meaningful crossing.
const C0_CANCELLATION_ULPS: f32 = 4.0;

/// Upper bound on the noise-derived root gate in [`torus_distance_f32`], in cm.
/// Stops a near-zero `c1` from widening the gate far enough to discard a real
/// crossing. 1.0e-3 cm = 10 um, well below any resolvable geometry feature.
const EPS_CAP: f32 = 1.0e-3;

/// A cubic has at most three real roots. f32 noise can split a clustered triple
/// root into four entries that survive `push_unique`'s dedup radius, so both the
/// Rust and WGSL cubic paths clamp to this. Without the clamp the WGSL side
/// indexes a 3-element `crits` with 4 and writes `bp[5]` on a 5-element array —
/// which WGSL silently bounds-clamps rather than trapping. See bead `op-9s8.12`.
const MAX_CUBIC_REAL_ROOTS: usize = 3;
//
// Everything below is a scalar f32 re-expression of geometry/surface.rs, written
// in a WGSL-friendly style (fixed-size arrays, explicit counts, no slices inside
// the polynomial solver) so shaders/surface_distance.wgsl is a direct translation.

/// f32 twin of `smallest_positive_root` (`geometry/surface.rs`): smallest root
/// `d > eps` of `a d² + b d + c = 0`, else [`MISS`]. Degenerates to the linear
/// solve when `a ≈ 0`.
fn smallest_positive_root_f32(a: f32, b: f32, c: f32, eps: f32) -> f32 {
    if a.abs() < 1.0e-7 {
        if b.abs() < 1.0e-7 {
            return MISS;
        }
        let d = -c / b;
        return if d > eps { d } else { MISS };
    }
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return MISS;
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
        MISS
    }
}

/// Horner evaluation of a descending-coefficient polynomial (`coeffs[0]` is the
/// leading term). f32 twin of `poly_eval`.
fn poly_eval_f32(coeffs: &[f32], x: f32) -> f32 {
    let mut acc = 0.0f32;
    for &c in coeffs {
        acc = acc * x + c;
    }
    acc
}

/// Sum of absolute Horner terms — the relative-residual scale for multiple-root
/// detection. f32 twin of `poly_abs_scale`.
fn poly_abs_scale_f32(coeffs: &[f32], x: f32) -> f32 {
    let ax = x.abs();
    let mut acc = 0.0f32;
    for &c in coeffs {
        acc = acc * ax + c.abs();
    }
    acc
}

/// Cauchy root bound. f32 twin of `root_bound`.
fn root_bound_f32(coeffs: &[f32]) -> f32 {
    let lead = coeffs[0];
    let mut m = 0.0f32;
    for &c in &coeffs[1..] {
        let a = (c / lead).abs();
        if a > m {
            m = a;
        }
    }
    1.0 + m
}

/// Insert `r` into `out[0..n]` unless a near-duplicate is present; returns the new
/// count. f32 twin of `push_unique` (relative dedup tol widened to f32 scale).
fn push_unique_f32(out: &mut [f32], n: usize, r: f32) -> usize {
    for &existing in &out[..n] {
        if (existing - r).abs() <= 1.0e-5 * (1.0 + r.abs()) {
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

/// Ascending insertion sort of a tiny slice. f32 twin of `sort_small`.
fn sort_small_f32(s: &mut [f32]) {
    for i in 1..s.len() {
        let mut j = i;
        while j > 0 && s[j - 1] > s[j] {
            s.swap(j - 1, j);
            j -= 1;
        }
    }
}

/// Bisection + Newton polish of the single root of `coeffs` in `[lo, hi]` where
/// the endpoints straddle zero. f32 twin of `bisect_root` (tolerances at f32
/// scale; bisection alone reaches f32 precision well within the iteration cap).
fn bisect_root_f32(coeffs: &[f32], deriv: &[f32], lo: f32, hi: f32) -> f32 {
    let mut a = lo;
    let mut b = hi;
    let mut fa = poly_eval_f32(coeffs, a);
    for _ in 0..100 {
        let mid = 0.5 * (a + b);
        let fm = poly_eval_f32(coeffs, mid);
        if fm == 0.0 {
            return mid;
        }
        if (fm < 0.0) == (fa < 0.0) {
            a = mid;
            fa = fm;
        } else {
            b = mid;
        }
        if (b - a).abs() <= 1.0e-7 * (1.0 + mid.abs()) {
            break;
        }
    }
    let mut x = 0.5 * (a + b);
    for _ in 0..8 {
        let f = poly_eval_f32(coeffs, x);
        let df = poly_eval_f32(deriv, x);
        if df == 0.0 {
            break;
        }
        let step = f / df;
        x -= step;
        if step.abs() <= 1.0e-7 * (1.0 + x.abs()) {
            break;
        }
    }
    x
}

/// Collect real roots of `coeffs` given `deriv` and its **sorted** critical points
/// `crits`, writing ascending into `out`; returns the count. f32 twin of
/// `collect_real_roots`.
fn collect_real_roots_f32(coeffs: &[f32], deriv: &[f32], crits: &[f32], out: &mut [f32]) -> usize {
    let mut m = root_bound_f32(coeffs);
    for &c in crits {
        m = m.max(c.abs());
    }
    m += 1.0;

    let mut bp = [0.0f32; 5];
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
        let flo = poly_eval_f32(coeffs, lo);
        let fhi = poly_eval_f32(coeffs, hi);
        if (flo < 0.0 && fhi > 0.0) || (flo > 0.0 && fhi < 0.0) {
            let r = bisect_root_f32(coeffs, deriv, lo, hi);
            n = push_unique_f32(out, n, r);
        }
    }
    for &c in crits {
        let residual = poly_eval_f32(coeffs, c).abs();
        let scale = poly_abs_scale_f32(coeffs, c);
        if residual <= 1.0e-6 * scale + 1.0e-30 {
            n = push_unique_f32(out, n, c);
        }
    }
    sort_small_f32(&mut out[..n]);
    n
}

/// Real roots of `a x² + b x + c` (ascending) into `out`. f32 twin of
/// `quadratic_real_roots`.
fn quadratic_real_roots_f32(a: f32, b: f32, c: f32, out: &mut [f32]) -> usize {
    if a.abs() < 1.0e-7 * (1.0 + b.abs() + c.abs()) {
        if b.abs() < 1.0e-7 * (1.0 + c.abs()) {
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

/// Real roots of `a x³ + b x² + c x + d` (ascending) into `out`. f32 twin of
/// `cubic_real_roots`.
fn cubic_real_roots_f32(a: f32, b: f32, c: f32, d: f32, out: &mut [f32]) -> usize {
    if a.abs() < 1.0e-7 * (1.0 + b.abs() + c.abs() + d.abs()) {
        return quadratic_real_roots_f32(b, c, d, out);
    }
    let coeffs = [a, b, c, d];
    let deriv = [3.0 * a, 2.0 * b, c];
    let mut crit = [0.0f32; 2];
    let nc = quadratic_real_roots_f32(3.0 * a, 2.0 * b, c, &mut crit);
    // Clamp to the mathematical maximum — see [`MAX_CUBIC_REAL_ROOTS`]. Here the
    // caller's buffer already bounds the count, so this is normally a no-op; it is
    // stated explicitly so the invariant is enforced identically on both sides
    // rather than resting on a buffer size on one side and nothing on the other.
    collect_real_roots_f32(&coeffs, &deriv, &crit[..nc], out).min(MAX_CUBIC_REAL_ROOTS)
}

/// Real roots of `a x⁴ + b x³ + c x² + d x + e` (ascending) into `out`. f32 twin
/// of `quartic_real_roots`.
fn quartic_real_roots_f32(a: f32, b: f32, c: f32, d: f32, e: f32, out: &mut [f32]) -> usize {
    if a.abs() < 1.0e-7 * (1.0 + b.abs() + c.abs() + d.abs() + e.abs()) {
        return cubic_real_roots_f32(b, c, d, e, out);
    }
    let coeffs = [a, b, c, d, e];
    let deriv = [4.0 * a, 3.0 * b, 2.0 * c, d];
    let mut crit = [0.0f32; 3];
    let nc = cubic_real_roots_f32(4.0 * a, 3.0 * b, 2.0 * c, d, &mut crit);
    sort_small_f32(&mut crit[..nc]);
    collect_real_roots_f32(&coeffs, &deriv, &crit[..nc], out)
}

/// Smallest positive ray–torus distance in the axis-independent `(r1, r2, ax)`
/// frame. f32 twin of `torus_distance` (same quartic derivation and spurious-root
/// filter). Returns [`MISS`] when the ray does not hit.
#[allow(clippy::too_many_arguments)]
fn torus_distance_f32(
    p1: f32,
    p2: f32,
    pax: f32,
    u1: f32,
    u2: f32,
    uax: f32,
    a: f32,
    b: f32,
    c: f32,
    coincident: bool,
) -> f32 {
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

    let c4 = ga * ga;
    let c3 = 4.0 * ga * gb;
    let c2 = 4.0 * gb * gb + 2.0 * ga * gc - 4.0 * a2 * alpha;
    let c1 = 4.0 * gb * gc - 8.0 * a2 * beta;

    // `c0` is the quartic's constant term, i.e. the torus sense at t = 0. When the
    // ray origin lies on (or very near) the torus it is a *catastrophically
    // cancelling* difference of two nearly equal positive quantities — for an
    // origin exactly on the surface the two operands are bit-identical and every
    // mantissa bit is lost.
    //
    // That matters because the CPU and GPU do not round it the same way. Rust
    // never contracts `x*y - z`, so this evaluates to exactly 0.0 here; WGSL/SPIR-V
    // permit fusing it into a single FMA (naga emits no `NoContraction`
    // decoration, so the driver is free to), which keeps `gc*gc` exact through one
    // rounding and leaves ~half an ULP of residue instead of zero. That residue
    // displaces the true root at t = 0 to `-c0/c1`, which then reads as a genuine
    // near-zero crossing. See bead `op-9s8.11`.
    //
    // Both sides therefore snap `c0` to exactly zero once it is at or below the
    // rounding noise of the subtraction that produced it. Below that threshold the
    // value carries no information — it is indistinguishable from zero in f32.
    let c0_raw = gc * gc - 4.0 * a2 * gamma;
    let c0_noise = C0_CANCELLATION_ULPS * F32_EPS * (gc * gc).abs().max((4.0 * a2 * gamma).abs());
    let c0 = if c0_raw.abs() <= c0_noise {
        0.0
    } else {
        c0_raw
    };

    let mut roots = [0.0f32; 4];
    let n = quartic_real_roots_f32(c4, c3, c2, c1, c0, &mut roots);

    // The root gate must also track that noise floor, not sit below it. A residual
    // perturbation `dc0` displaces a simple root near zero by `dc0/|c1|`; with the
    // old fixed 1.0e-7 the gate sat about an order of magnitude *under* the actual
    // f32 noise floor for cm-scale torus geometry, so noise roots always passed.
    // (The f64 reference uses 1.0e-10, where the same displacement is ~1e-15 — five
    // orders of margin. Scaling that to 1.0e-7 for f32 under-provisioned it.)
    //
    // `EPS_CAP` bounds the gate so a near-zero `c1` cannot widen it enough to
    // swallow a legitimate crossing; at cm scale 1.0e-3 cm is 10 um, far below any
    // resolvable geometry feature.
    let base_eps: f32 = if coincident { 1.0e-5 } else { 1.0e-7 };
    let noise_eps: f32 = if c1.abs() > 0.0 {
        (c0_noise / c1.abs()).min(EPS_CAP)
    } else {
        0.0
    };
    let eps = base_eps.max(noise_eps);
    let mut best = MISS;
    for &t in &roots[..n] {
        if t <= eps {
            continue;
        }
        let gt = ga * t * t + 2.0 * gb * t + gc;
        let tol = 1.0e-6 * (1.0 + ga * t * t + (2.0 * gb * t).abs() + gc.abs());
        if gt >= -tol && t < best {
            best = t;
        }
    }
    best
}

/// Scalar `f32` distance from ray `(r, u)` to encoded surface `tag`/`c`.
///
/// The `f32` twin and exact structural mirror of [`SurfaceKind::distance`]; the
/// WGSL `surface_distance` function is a line-by-line translation of this. `c`
/// is the surface's [`SURF_STRIDE`]-long coefficient row from [`encode_surfaces`].
/// Returns [`MISS`] when the ray does not cross.
fn surface_distance_one_f32(
    tag: u32,
    c: &[f32],
    r: Position,
    u: Direction,
    coincident: bool,
) -> f32 {
    let (rx, ry, rz) = (r.x as f32, r.y as f32, r.z as f32);
    let (uu, uv, uw) = (u.u as f32, u.v as f32, u.w as f32);
    match tag {
        // XPlane { x0 }
        0 => {
            let hint = if coincident { 1.0e-7 } else { 0.0 };
            if uu.abs() < 1.0e-7 {
                return MISS;
            }
            let d = (c[0] - rx) / uu;
            if d > hint {
                d
            } else {
                MISS
            }
        }
        // YPlane { y0 }
        1 => {
            let hint = if coincident { 1.0e-7 } else { 0.0 };
            if uv.abs() < 1.0e-7 {
                return MISS;
            }
            let d = (c[0] - ry) / uv;
            if d > hint {
                d
            } else {
                MISS
            }
        }
        // ZPlane { z0 }
        2 => {
            let hint = if coincident { 1.0e-7 } else { 0.0 };
            if uw.abs() < 1.0e-7 {
                return MISS;
            }
            let d = (c[0] - rz) / uw;
            if d > hint {
                d
            } else {
                MISS
            }
        }
        // Plane { a, b, c, d }
        3 => {
            let hint = if coincident { 1.0e-7 } else { 0.0 };
            let denom = c[0] * uu + c[1] * uv + c[2] * uw;
            if denom.abs() < 1.0e-7 {
                return MISS;
            }
            let d = -(c[0] * rx + c[1] * ry + c[2] * rz - c[3]) / denom;
            if d > hint {
                d
            } else {
                MISS
            }
        }
        // Sphere { x0, y0, z0, r }
        4 => {
            const EPS: f32 = 1.0e-6;
            let ox = rx - c[0];
            let oy = ry - c[1];
            let oz = rz - c[2];
            let k = ox * uu + oy * uv + oz * uw;
            let cc = if coincident {
                0.0
            } else {
                ox * ox + oy * oy + oz * oz - c[3] * c[3]
            };
            let disc = k * k - cc;
            if disc < 0.0 {
                return MISS;
            }
            let sq = disc.sqrt();
            let d_near = -k - sq;
            if d_near > EPS {
                d_near
            } else {
                let d_far = -k + sq;
                if d_far > EPS {
                    d_far
                } else {
                    MISS
                }
            }
        }
        // XCylinder { y0, z0, r } — radial pair (y, z), axis x
        5 => axis_cylinder_f32(ry - c[0], rz - c[1], uv, uw, c[2], coincident),
        // YCylinder { x0, z0, r } — radial pair (x, z), axis y
        6 => axis_cylinder_f32(rx - c[0], rz - c[1], uu, uw, c[2], coincident),
        // ZCylinder { x0, y0, r } — radial pair (x, y), axis z
        7 => axis_cylinder_f32(rx - c[0], ry - c[1], uu, uv, c[2], coincident),
        // XCone { x0, y0, z0, r_sq } — axis x
        8 => {
            let dx = rx - c[0];
            let dy = ry - c[1];
            let dz = rz - c[2];
            let rsq = c[3];
            let a = uv * uv + uw * uw - rsq * uu * uu;
            let k = dy * uv + dz * uw - rsq * dx * uu;
            let cc = if coincident {
                0.0
            } else {
                dy * dy + dz * dz - rsq * dx * dx
            };
            smallest_positive_root_f32(a, 2.0 * k, cc, 1.0e-7)
        }
        // YCone { x0, y0, z0, r_sq } — axis y
        9 => {
            let dx = rx - c[0];
            let dy = ry - c[1];
            let dz = rz - c[2];
            let rsq = c[3];
            let a = uu * uu + uw * uw - rsq * uv * uv;
            let k = dx * uu + dz * uw - rsq * dy * uv;
            let cc = if coincident {
                0.0
            } else {
                dx * dx + dz * dz - rsq * dy * dy
            };
            smallest_positive_root_f32(a, 2.0 * k, cc, 1.0e-7)
        }
        // ZCone { x0, y0, z0, r_sq } — axis z
        10 => {
            let dx = rx - c[0];
            let dy = ry - c[1];
            let dz = rz - c[2];
            let rsq = c[3];
            let a = uu * uu + uv * uv - rsq * uw * uw;
            let k = dx * uu + dy * uv - rsq * dz * uw;
            let cc = if coincident {
                0.0
            } else {
                dx * dx + dy * dy - rsq * dz * dz
            };
            smallest_positive_root_f32(a, 2.0 * k, cc, 1.0e-7)
        }
        // Quadric { a,b,c,d,e,f,g,h,j,k }
        11 => {
            let (qa, qb, qc, qd, qe, qf, qg, qh, qj, qk) =
                (c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7], c[8], c[9]);
            let a_q = qa * uu * uu
                + qb * uv * uv
                + qc * uw * uw
                + qd * uu * uv
                + qe * uv * uw
                + qf * uu * uw;
            let b_q = 2.0 * qa * rx * uu
                + 2.0 * qb * ry * uv
                + 2.0 * qc * rz * uw
                + qd * (rx * uv + ry * uu)
                + qe * (ry * uw + rz * uv)
                + qf * (rx * uw + rz * uu)
                + qg * uu
                + qh * uv
                + qj * uw;
            let c_q = if coincident {
                0.0
            } else {
                qa * rx * rx
                    + qb * ry * ry
                    + qc * rz * rz
                    + qd * rx * ry
                    + qe * ry * rz
                    + qf * rx * rz
                    + qg * rx
                    + qh * ry
                    + qj * rz
                    + qk
            };
            smallest_positive_root_f32(a_q, b_q, c_q, 1.0e-7)
        }
        // XTorus { x0,y0,z0,a,b,c } — radial pair (y, z), axial x
        12 => torus_distance_f32(
            ry - c[1],
            rz - c[2],
            rx - c[0],
            uv,
            uw,
            uu,
            c[3],
            c[4],
            c[5],
            coincident,
        ),
        // YTorus — radial pair (x, z), axial y
        13 => torus_distance_f32(
            rx - c[0],
            rz - c[2],
            ry - c[1],
            uu,
            uw,
            uv,
            c[3],
            c[4],
            c[5],
            coincident,
        ),
        // ZTorus — radial pair (x, y), axial z
        14 => torus_distance_f32(
            rx - c[0],
            ry - c[1],
            rz - c[2],
            uu,
            uv,
            uw,
            c[3],
            c[4],
            c[5],
            coincident,
        ),
        _ => MISS,
    }
}

/// Shared axis-aligned-cylinder distance in the radial `(d1, d2)` / `(w1, w2)`
/// frame. f32 twin of the `{X,Y,Z}Cylinder::distance` body (identical for all
/// three axes once the radial pair is chosen). Returns [`MISS`] on a miss.
fn axis_cylinder_f32(d1: f32, d2: f32, w1: f32, w2: f32, radius: f32, coincident: bool) -> f32 {
    const FP_COINCIDENT: f32 = 1.0e-6;
    let a = w1 * w1 + w2 * w2;
    if a == 0.0 {
        return MISS;
    }
    let k = d1 * w1 + d2 * w2;
    let cc = d1 * d1 + d2 * d2 - radius * radius;
    let quad = k * k - a * cc;
    if quad < 0.0 {
        return MISS;
    }
    let sq = quad.sqrt();
    if coincident || cc.abs() < FP_COINCIDENT {
        if k >= 0.0 {
            MISS
        } else {
            (-k + sq) / a
        }
    } else if cc < 0.0 {
        (-k + sq) / a
    } else {
        let d = (-k - sq) / a;
        if d < 0.0 {
            MISS
        } else {
            d
        }
    }
}

/// CPU `f32` reference for a batch of ray–surface distance queries.
///
/// Evaluates [`surface_distance_one_f32`] for each [`SurfaceQuery`], returning one
/// distance per query in order ([`MISS`] where the ray does not cross). This is
/// both the deterministic CPU path and the reference the GPU kernel is judged
/// against; it is itself judged against the trusted `f64` [`SurfaceKind::distance`]
/// by the tests in this module.
pub fn surface_distance_cpu_f32(encoded: &EncodedSurfaces, queries: &[SurfaceQuery]) -> Vec<f32> {
    queries
        .iter()
        .map(|q| {
            let si = q.surface as usize;
            let tag = encoded.tags[si];
            let base = si * SURF_STRIDE;
            let c = &encoded.coeffs[base..base + SURF_STRIDE];
            surface_distance_one_f32(tag, c, q.r, q.u, q.coincident)
        })
        .collect()
}

// ─────────────────────────────── GPU compute path ──────────────────────────────

/// Reinterpret an `&[f32]` as native-endian bytes for buffer upload.
#[cfg(not(target_os = "android"))]
fn f32_slice_to_bytes(data: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(data.len() * 4);
    for &x in data {
        bytes.extend_from_slice(&x.to_ne_bytes());
    }
    bytes
}

/// Reinterpret an `&[u32]` as native-endian bytes for buffer upload.
#[cfg(not(target_os = "android"))]
fn u32_slice_to_bytes(data: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(data.len() * 4);
    for &x in data {
        bytes.extend_from_slice(&x.to_ne_bytes());
    }
    bytes
}

/// Rebuild a `Vec<f32>` of length `n` from a native-endian byte view.
#[cfg(not(target_os = "android"))]
fn bytes_to_f32_vec(bytes: &[u8], n: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(n);
    for chunk in bytes.chunks_exact(4).take(n) {
        out.push(f32::from_ne_bytes(chunk.try_into().unwrap()));
    }
    out
}

/// Compute ray–surface distances for a batch of queries **on the GPU**, via the
/// `shaders/surface_distance.wgsl` compute shader. The `f32` accelerated twin of
/// [`surface_distance_cpu_f32`]; the two are held to agreement by the V&V test in
/// this module.
///
/// # Parameters
/// - `ctx` — a live [`crate::gpu::GpuContext`] from [`crate::gpu::probe`].
/// - `encoded` — surfaces flattened by [`encode_surfaces`].
/// - `queries` — the rays to test; each names a surface index, a coincident flag,
///   an origin (cm) and a unit direction.
///
/// Returns one distance per query in order ([`MISS`] on a miss). An empty
/// `queries` returns an empty `Vec` without touching the GPU.
///
/// # How it works
/// Uploads the tag, coeff, per-query surface-index, per-query flag, origin, and
/// direction arrays as read-only storage buffers plus an `[n_surf, n_query, 0, 0]`
/// uniform, dispatches `ceil(n_query / 64)` workgroups of `main`, copies the
/// result buffer to a mappable staging buffer, blocks on
/// `device.poll(PollType::wait_indefinitely())`, and reads it back.
#[cfg(not(target_os = "android"))]
pub fn surface_distance_gpu(
    ctx: &crate::gpu::GpuContext,
    encoded: &EncodedSurfaces,
    queries: &[SurfaceQuery],
) -> Vec<f32> {
    use wgpu::util::DeviceExt;

    let n_query = queries.len();
    if n_query == 0 {
        return Vec::new();
    }
    let n_surf = encoded.len();

    let device = &ctx.device;
    let queue = &ctx.queue;

    // Split queries into the SoA buffers the shader binds.
    let q_surf: Vec<u32> = queries.iter().map(|q| q.surface).collect();
    let q_flag: Vec<u32> = queries.iter().map(|q| q.coincident as u32).collect();
    let mut q_r = Vec::with_capacity(n_query * 3);
    let mut q_u = Vec::with_capacity(n_query * 3);
    for q in queries {
        q_r.push(q.r.x as f32);
        q_r.push(q.r.y as f32);
        q_r.push(q.r.z as f32);
        q_u.push(q.u.u as f32);
        q_u.push(q.u.v as f32);
        q_u.push(q.u.w as f32);
    }

    let storage = |label: &str, contents: &[u8]| {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents,
            usage: wgpu::BufferUsages::STORAGE,
        })
    };

    let tags_buf = storage("surf_dist.tags", &u32_slice_to_bytes(&encoded.tags));
    let coeffs_buf = storage("surf_dist.coeffs", &f32_slice_to_bytes(&encoded.coeffs));
    let qsurf_buf = storage("surf_dist.q_surf", &u32_slice_to_bytes(&q_surf));
    let qflag_buf = storage("surf_dist.q_flag", &u32_slice_to_bytes(&q_flag));
    let qr_buf = storage("surf_dist.q_r", &f32_slice_to_bytes(&q_r));
    let qu_buf = storage("surf_dist.q_u", &f32_slice_to_bytes(&q_u));

    let params: [u32; 4] = [n_surf as u32, n_query as u32, 0, 0];
    let params_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("surf_dist.params"),
        contents: &u32_slice_to_bytes(&params),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let result_size = (n_query * std::mem::size_of::<f32>()) as wgpu::BufferAddress;
    let result_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("surf_dist.result"),
        size: result_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let staging_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("surf_dist.staging"),
        size: result_size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("surf_dist.shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shaders/surface_distance.wgsl").into()),
    });

    let storage_ro = wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Storage { read_only: true },
        has_dynamic_offset: false,
        min_binding_size: None,
    };
    let ro_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: storage_ro,
        count: None,
    };
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("surf_dist.bgl"),
        entries: &[
            ro_entry(0),
            ro_entry(1),
            ro_entry(2),
            ro_entry(3),
            ro_entry(4),
            ro_entry(5),
            wgpu::BindGroupLayoutEntry {
                binding: 6,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 7,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("surf_dist.pl"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });

    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("surf_dist.pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("surf_dist.bg"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: tags_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: coeffs_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: qsurf_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: qflag_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: qr_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: qu_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: params_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: result_buf.as_entire_binding(),
            },
        ],
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("surf_dist.enc"),
    });
    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("surf_dist.pass"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        let wg = (n_query as u32).div_ceil(64);
        cpass.dispatch_workgroups(wg, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&result_buf, 0, &staging_buf, 0, result_size);
    queue.submit(Some(encoder.finish()));

    let slice = staging_buf.slice(..);
    slice.map_async(wgpu::MapMode::Read, |res| {
        res.unwrap();
    });
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();

    let view = slice.get_mapped_range().expect("staging buffer mapping failed after a completed poll");
    let out = bytes_to_f32_vec(&view, n_query);
    drop(view);
    staging_buf.unmap();
    out
}

// ────────────────────────── hybrid CPU + GPU dispatch ──────────────────────────

/// Storage buffers this kernel binds in a single compute stage.
///
/// `surf_dist.bgl` binds bindings 0-5 and 7 as storage (tags, coeffs, query
/// surface/flag/r/u, results) plus binding 6 as a uniform. A device whose
/// `max_storage_buffers_per_shader_stage` is below this **cannot** host the
/// kernel — that is exactly the `op-bjd` failure, and
/// [`surface_distance_hybrid`] now turns it into a CPU fallback rather than a
/// panic.
pub const STORAGE_BUFFERS_NEEDED: u32 = 7;

/// Bytes per query in the largest per-query binding (`q_r`/`q_u`: three `f32`).
/// Used with the driver's `max_storage_buffer_binding_size` to size dispatch
/// chunks.
pub const QUERY_STRIDE_BYTES: u64 = 12;

/// Threads per workgroup in `surface_distance.wgsl` (`@workgroup_size(64)`).
pub const WORKGROUP_SIZE: u32 = 64;

/// Evaluate a query batch across **both** devices: the GPU takes the front of
/// the batch, the CPU takes the back, and the two run concurrently.
///
/// Returns the distances in query order (identical layout to
/// [`surface_distance_cpu_f32`]) together with the [`WorkSplit`] actually used,
/// so a caller can log or assert which devices did work.
///
/// # How the split is decided
///
/// Entirely from **sourced hardware capability**, never assumption — see
/// [`crate::gpu::capabilities`]. [`plan_split`] is given this kernel's real
/// requirements ([`STORAGE_BUFFERS_NEEDED`], [`QUERY_STRIDE_BYTES`],
/// [`WORKGROUP_SIZE`]) and returns an all-CPU plan whenever the GPU cannot host
/// the kernel, is absent, or the policy asks for none.
///
/// # Concurrency
///
/// The CPU half runs on a `rayon` parallel iterator inside a scoped thread while
/// this thread drives the GPU dispatch, so the two genuinely overlap rather than
/// running one after the other. The GPU half is issued in chunks of at most
/// `split.gpu_chunk_items` so an oversized batch cannot exceed the driver's
/// binding-size or workgroup-count limits.
///
/// # Accuracy
///
/// The GPU half is `f32` and will not bit-match the CPU half (crate `CLAUDE.md`
/// contract 2). **Do not use this for anything feeding V&V** — use
/// [`surface_distance_cpu_f32`] there. Results are mixed from two different
/// arithmetic paths, so a hybrid batch is inherently less reproducible than
/// either path alone.
#[cfg(not(target_os = "android"))]
pub fn surface_distance_hybrid(
    ctx: Option<&crate::gpu::GpuContext>,
    encoded: &EncodedSurfaces,
    queries: &[SurfaceQuery],
    policy: crate::gpu::capabilities::SplitPolicy,
) -> (Vec<f32>, crate::gpu::capabilities::WorkSplit) {
    use crate::gpu::capabilities::{plan_split, HardwareCapabilities};
    use rayon::prelude::*;

    let caps = match ctx {
        Some(c) => c.capabilities(),
        None => HardwareCapabilities::with_gpu(None),
    };
    let split = plan_split(
        queries.len(),
        &caps,
        policy,
        STORAGE_BUFFERS_NEEDED,
        QUERY_STRIDE_BYTES,
        WORKGROUP_SIZE,
    );

    if !split.uses_gpu() {
        return (surface_distance_cpu_f32(encoded, queries), split);
    }

    let (gpu_queries, cpu_queries) = queries.split_at(split.gpu_items);
    let ctx = ctx.expect("uses_gpu() implies a context");

    // CPU half on a scoped thread (rayon-parallel across the machine's cores)
    // while this thread drives the GPU. The two overlap.
    let (gpu_out, cpu_out) = std::thread::scope(|scope| {
        let cpu_handle = scope.spawn(|| {
            cpu_queries
                .par_iter()
                .map(|q| {
                    let si = q.surface as usize;
                    let tag = encoded.tags[si];
                    let base = si * SURF_STRIDE;
                    let c = &encoded.coeffs[base..base + SURF_STRIDE];
                    surface_distance_one_f32(tag, c, q.r, q.u, q.coincident)
                })
                .collect::<Vec<f32>>()
        });

        // GPU half, chunked to the driver's dispatch ceiling.
        let mut gpu_out = Vec::with_capacity(gpu_queries.len());
        for chunk in gpu_queries.chunks(split.gpu_chunk_items) {
            gpu_out.extend(surface_distance_gpu(ctx, encoded, chunk));
        }

        let cpu_out = cpu_handle.join().expect("CPU half panicked");
        (gpu_out, cpu_out)
    });

    let mut out = gpu_out;
    out.extend(cpu_out);
    debug_assert_eq!(out.len(), queries.len());
    (out, split)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::surface::{
        BoundaryType, Plane, Quadric, Sphere, XCone, XCylinder, XPlane, XTorus, YCone, YCylinder,
        YPlane, YTorus, ZCone, ZCylinder, ZPlane, ZTorus,
    };

    const BC: BoundaryType = BoundaryType::Transmissive;

    /// One representative surface of every [`SurfaceKind`], in declaration order.
    fn one_of_each() -> Vec<SurfaceKind> {
        vec![
            SurfaceKind::XPlane(XPlane { x0: 0.7, bc: BC }),
            SurfaceKind::YPlane(YPlane { y0: -0.3, bc: BC }),
            SurfaceKind::ZPlane(ZPlane { z0: 1.2, bc: BC }),
            SurfaceKind::Plane(Plane {
                a: 1.0,
                b: 2.0,
                c: -1.0,
                d: 0.5,
                bc: BC,
            }),
            SurfaceKind::Sphere(Sphere {
                x0: 0.1,
                y0: -0.2,
                z0: 0.3,
                r: 1.5,
                bc: BC,
            }),
            SurfaceKind::XCylinder(XCylinder {
                y0: 0.2,
                z0: -0.1,
                r: 0.8,
                bc: BC,
            }),
            SurfaceKind::YCylinder(YCylinder {
                x0: -0.4,
                z0: 0.25,
                r: 0.9,
                bc: BC,
            }),
            SurfaceKind::ZCylinder(ZCylinder {
                x0: 0.15,
                y0: 0.05,
                r: 1.1,
                bc: BC,
            }),
            SurfaceKind::XCone(XCone {
                x0: 0.0,
                y0: 0.0,
                z0: 0.0,
                r_sq: 0.25,
                bc: BC,
            }),
            SurfaceKind::YCone(YCone {
                x0: 0.1,
                y0: -0.1,
                z0: 0.2,
                r_sq: 0.4,
                bc: BC,
            }),
            SurfaceKind::ZCone(ZCone {
                x0: -0.2,
                y0: 0.3,
                z0: 0.0,
                r_sq: 0.3,
                bc: BC,
            }),
            // An ellipsoid quadric: x²/1 + y²/0.5² + z²/0.75² − 1 = 0.
            SurfaceKind::Quadric(Quadric {
                a: 1.0,
                b: 1.0 / 0.25,
                c: 1.0 / 0.5625,
                d: 0.0,
                e: 0.0,
                f: 0.0,
                g: 0.0,
                h: 0.0,
                j: 0.0,
                k: -1.0,
                bc: BC,
            }),
            SurfaceKind::XTorus(XTorus {
                x0: 0.0,
                y0: 0.0,
                z0: 0.0,
                a: 1.0,
                b: 0.3,
                c: 0.3,
                bc: BC,
            }),
            SurfaceKind::YTorus(YTorus {
                x0: 0.1,
                y0: 0.0,
                z0: -0.1,
                a: 1.2,
                b: 0.25,
                c: 0.35,
                bc: BC,
            }),
            SurfaceKind::ZTorus(ZTorus {
                x0: 0.0,
                y0: 0.2,
                z0: 0.0,
                a: 0.9,
                b: 0.2,
                c: 0.2,
                bc: BC,
            }),
        ]
    }

    /// Deterministic direction sampler (Fibonacci-sphere) — no RNG dependency.
    fn dir(i: usize, n: usize) -> Direction {
        let golden = std::f64::consts::PI * (3.0 - 5.0_f64.sqrt());
        let z = 1.0 - 2.0 * (i as f64 + 0.5) / n as f64;
        let rho = (1.0 - z * z).max(0.0).sqrt();
        let phi = golden * i as f64;
        Direction::new(rho * phi.cos(), rho * phi.sin(), z)
    }

    /// V&V (CPU mirror vs trusted f64 reference): for every [`SurfaceKind`], the
    /// scalar `f32` [`surface_distance_cpu_f32`] must agree with the trusted `f64`
    /// [`SurfaceKind::distance`] over a spread of ray origins and directions.
    ///
    /// **Methodology.** One representative surface of each of the 15 kinds
    /// ([`one_of_each`]). For each, fire ~48 rays: origins on a small lattice of
    /// points spanning inside/outside the feature, directions from a
    /// Fibonacci-sphere so every octant is covered. The reference is the `f64`
    /// `SurfaceKind::distance`; the candidate is the `f32` mirror on the encoded
    /// form. A "miss" is `f64::INFINITY` on the reference and `>= MISS` on the
    /// mirror. **Pass criterion:** both miss together, or both hit and
    /// `|d_f32 − d_f64| <= 3e-3 * (1 + d_f64)` — a mixed abs/rel band sized for f32
    /// evaluation of the quadratic/quartic roots over cm-scale geometry.
    ///
    /// **Results (2026-07-24).** All 15 kinds agree within the band across the
    /// full ray set (recorded on this host). This verifies the encoding and the
    /// f32 distance algorithm; the WGSL kernel — a line-by-line translation of the
    /// same mirror — is separately validated for well-formedness by
    /// [`wgsl_shader_is_valid`], and executed against this mirror by
    /// [`gpu_matches_cpu_reference`] on a host with a real GPU adapter.
    #[test]
    fn cpu_f32_mirror_matches_f64_reference() {
        let surfaces = one_of_each();
        let encoded = encode_surfaces(&surfaces);

        // A lattice of origins spanning roughly [-2, 2]^3.
        let mut origins = Vec::new();
        for ix in -1..=1 {
            for iy in -1..=1 {
                for iz in -1..=1 {
                    origins.push(Position::new(
                        ix as f64 * 1.3,
                        iy as f64 * 1.3,
                        iz as f64 * 1.3,
                    ));
                }
            }
        }

        let n_dir = 48usize;
        let mut n_hit = 0usize;
        for (si, s) in surfaces.iter().enumerate() {
            for &o in &origins {
                for i in 0..n_dir {
                    let u = dir(i, n_dir);
                    let d_ref = s.distance(o, u, false);
                    let q = SurfaceQuery {
                        surface: si as u32,
                        coincident: false,
                        r: o,
                        u,
                    };
                    let d_f32 = surface_distance_cpu_f32(&encoded, std::slice::from_ref(&q))[0];

                    let ref_miss = !d_ref.is_finite();
                    let f32_miss = d_f32 >= MISS;
                    if ref_miss || f32_miss {
                        assert_eq!(
                            ref_miss,
                            f32_miss,
                            "surface {si} ({}): miss disagreement — f64={d_ref}, f32={d_f32}",
                            kind_name(s)
                        );
                        continue;
                    }
                    n_hit += 1;
                    let tol = 3.0e-3 * (1.0 + d_ref as f32);
                    assert!(
                        (d_f32 - d_ref as f32).abs() <= tol,
                        "surface {si} ({}): f64={d_ref}, f32={d_f32}, |diff|={} > tol={tol} (origin {o:?}, dir {u:?})",
                        kind_name(s),
                        (d_f32 - d_ref as f32).abs()
                    );
                }
            }
        }
        // Sanity: the ray set actually produced a healthy number of real hits.
        assert!(
            n_hit > 100,
            "too few hits ({n_hit}) — test not exercising intersections"
        );
    }

    /// Coincident-origin handling: a ray starting exactly on a surface and
    /// pointing back through it must recover the far intersection, matching the
    /// `f64` reference's `coincident=true` branch (no spurious `d ≈ 0`).
    ///
    /// **Results (2026-07-24).** For a sphere and a Z-cylinder, an origin placed
    /// on the surface with a direction through the body yields the far-side
    /// distance in both `f64` and the `f32` mirror, agreeing within the band.
    #[test]
    fn cpu_f32_mirror_coincident_matches_f64() {
        let sph = SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r: 1.0,
            bc: BC,
        });
        let cyl = SurfaceKind::ZCylinder(ZCylinder {
            x0: 0.0,
            y0: 0.0,
            r: 1.0,
            bc: BC,
        });
        let enc = encode_surfaces(std::slice::from_ref(&sph));
        let enc_c = encode_surfaces(std::slice::from_ref(&cyl));

        // On +x of the unit sphere, aim in -x → far side at d = 2.
        let o = Position::new(1.0, 0.0, 0.0);
        let u = Direction::new(-1.0, 0.0, 0.0);
        let d_ref = sph.distance(o, u, true);
        let q = SurfaceQuery {
            surface: 0,
            coincident: true,
            r: o,
            u,
        };
        let d_f32 = surface_distance_cpu_f32(&enc, std::slice::from_ref(&q))[0];
        assert!((d_ref - 2.0).abs() < 1e-9, "f64 sphere far chord {d_ref}");
        assert!(
            (d_f32 - d_ref as f32).abs() < 3e-3,
            "f32 sphere far chord {d_f32}"
        );

        // On +x of the unit cylinder, aim in -x → far side at d = 2.
        let d_ref_c = cyl.distance(o, u, true);
        let qc = SurfaceQuery {
            surface: 0,
            coincident: true,
            r: o,
            u,
        };
        let d_f32_c = surface_distance_cpu_f32(&enc_c, std::slice::from_ref(&qc))[0];
        assert!((d_ref_c - 2.0).abs() < 1e-9, "f64 cyl far chord {d_ref_c}");
        assert!(
            (d_f32_c - d_ref_c as f32).abs() < 3e-3,
            "f32 cyl far chord {d_f32_c}"
        );
    }

    /// Short human name for assertion messages.
    fn kind_name(s: &SurfaceKind) -> &'static str {
        match s {
            SurfaceKind::XPlane(_) => "XPlane",
            SurfaceKind::YPlane(_) => "YPlane",
            SurfaceKind::ZPlane(_) => "ZPlane",
            SurfaceKind::Plane(_) => "Plane",
            SurfaceKind::Sphere(_) => "Sphere",
            SurfaceKind::XCylinder(_) => "XCylinder",
            SurfaceKind::YCylinder(_) => "YCylinder",
            SurfaceKind::ZCylinder(_) => "ZCylinder",
            SurfaceKind::XCone(_) => "XCone",
            SurfaceKind::YCone(_) => "YCone",
            SurfaceKind::ZCone(_) => "ZCone",
            SurfaceKind::Quadric(_) => "Quadric",
            SurfaceKind::XTorus(_) => "XTorus",
            SurfaceKind::YTorus(_) => "YTorus",
            SurfaceKind::ZTorus(_) => "ZTorus",
        }
    }

    /// V&V (shader well-formedness, no GPU required): the WGSL kernel must parse
    /// and pass `naga` validation. This catches syntax/type errors on CPU-only
    /// CI where [`gpu_matches_cpu_reference`] can only SKIP.
    ///
    /// **Methodology.** Parse `shaders/surface_distance.wgsl` with
    /// `naga::front::wgsl` and run the full `naga::valid::Validator`
    /// (all capabilities, default subgroup stages). **Pass criterion:** parse and
    /// validation both succeed.
    ///
    /// **Results (2026-07-24).** The shader parses and validates clean on this
    /// host (no GPU adapter needed).
    #[cfg(not(target_os = "android"))]
    #[test]
    fn wgsl_shader_is_valid() {
        let src = include_str!("shaders/surface_distance.wgsl");
        let module = naga::front::wgsl::parse_str(src)
            .unwrap_or_else(|e| panic!("WGSL parse failed: {e:?}"));
        let mut validator = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        );
        validator
            .validate(&module)
            .unwrap_or_else(|e| panic!("WGSL validation failed: {e:?}"));
    }

    /// V&V GATE (GPU vs CPU mirror): the WGSL kernel must reproduce the scalar
    /// `f32` [`surface_distance_cpu_f32`] within tight f32 tolerance for a batch
    /// of rays over all 15 surface kinds. SKIPs (green) when no GPU adapter is
    /// present, exactly like the `xs_interp` gate; the algorithm itself is still
    /// covered on CPU-only CI by [`cpu_f32_mirror_matches_f64_reference`].
    ///
    /// **Methodology.** Build [`one_of_each`], enumerate the same origin lattice ×
    /// Fibonacci directions as the CPU test into a single batch, run
    /// [`surface_distance_cpu_f32`] and [`surface_distance_gpu`] on it. **Pass
    /// criterion:** per query, both miss (`>= MISS`) or `|gpu − cpu| <=
    /// 1e-3 * (1 + |cpu|)` (they share the same f32 algorithm, so agreement is
    /// tight — this only absorbs GPU rounding-order differences).
    ///
    /// **Results (measured 2026-08-06, NVIDIA RTX A5000 / NVK GA102, Vulkan).**
    /// **PASSES — all 19 440 queries across all 15 surface types.** On CPU-only
    /// CI it prints a SKIP line and returns green.
    ///
    /// Getting here took two fixes, both worth recording because the first hid
    /// the second:
    ///
    /// 1. **`op-bjd`** — the test could not execute a single query. `probe`
    ///    requested `Limits::downlevel_defaults()`, capping
    ///    `max_storage_buffers_per_shader_stage` at 4 while `surf_dist.bgl`
    ///    binds 7, so the layout was rejected on hardware that allows 524 288.
    ///    Fixed by sourcing `adapter.limits()`.
    /// 2. **`op-9s8.11`** — with the kernel finally running, **79 of 19 440
    ///    queries (0.41 %) disagreed**: XTorus 70, ZTorus 9, YTorus 0,
    ///    non-torus types 0 of 15 552. Every one was the GPU returning a
    ///    spurious near-zero root (min `1.006e-7`, median `1.85e-7`, max
    ///    `3.5e-6`), and every one came from a ray origin lying *exactly* on a
    ///    torus — 4 of the 27 lattice origins sit on the XTorus, 1 on the
    ///    ZTorus, 0 on the YTorus, which is precisely the observed 70/9/0.
    ///    Cause: `c0 = gc*gc - 4*a2*gamma` is a fully cancelling difference
    ///    there (operands bit-identical at `6.7599992752075195`), and WGSL
    ///    permits fusing it into an FMA where Rust does not — leaving
    ///    `c0 = 2.2888e-7` instead of `0.0`, which displaces the true `t = 0`
    ///    root to `3.1208810e-7`. Fixed by snapping `c0` to zero below its own
    ///    rounding noise and making the root gate track that noise floor.
    ///
    /// Worked example, query 16033 (surface 12 `XTorus`, origin `(0, -1.3, 0)`):
    /// before the fix `gpu = 3.120881e-7` against `cpu = 0.49672258`; after it
    /// both sides agree, and the trusted `f64` path gives `0.4967226591440329`.
    #[cfg(not(target_os = "android"))]
    #[test]
    fn gpu_matches_cpu_reference() {
        let Some(ctx) = crate::gpu::probe() else {
            eprintln!("SKIP surface_distance gpu test: no GPU adapter (CPU-only CI)");
            return;
        };
        let surfaces = one_of_each();
        let encoded = encode_surfaces(&surfaces);

        let mut origins = Vec::new();
        for ix in -1..=1 {
            for iy in -1..=1 {
                for iz in -1..=1 {
                    origins.push(Position::new(
                        ix as f64 * 1.3,
                        iy as f64 * 1.3,
                        iz as f64 * 1.3,
                    ));
                }
            }
        }
        let n_dir = 48usize;
        let mut queries = Vec::new();
        for si in 0..surfaces.len() {
            for &o in &origins {
                for i in 0..n_dir {
                    queries.push(SurfaceQuery {
                        surface: si as u32,
                        coincident: false,
                        r: o,
                        u: dir(i, n_dir),
                    });
                }
            }
        }

        let cpu = surface_distance_cpu_f32(&encoded, &queries);
        let gpu = surface_distance_gpu(&ctx, &encoded, &queries);
        assert_eq!(gpu.len(), cpu.len(), "GPU returned wrong length");

        for (i, (&g, &c)) in gpu.iter().zip(cpu.iter()).enumerate() {
            let c_miss = c >= MISS;
            let g_miss = g >= MISS;
            if c_miss || g_miss {
                assert_eq!(
                    c_miss, g_miss,
                    "query {i}: miss disagreement gpu={g}, cpu={c}"
                );
                continue;
            }
            let tol = 1e-3 * (1.0 + c.abs());
            assert!(
                (g - c).abs() <= tol,
                "query {i}: gpu={g}, cpu={c}, |diff|={} > tol={tol}",
                (g - c).abs()
            );
        }
    }

    /// Diagnostic: print the hardware capabilities actually sourced from this
    /// machine's adapter, plus the CPU thread count and the split the planner
    /// derives from them.
    ///
    /// `#[ignore]`d because it asserts almost nothing and its output is
    /// machine-specific — it exists to make the numbers behind `op-bjd`
    /// inspectable. Run with:
    /// `cargo test -p outram-mc-libs --lib --release probed_hardware -- --ignored --nocapture`
    #[cfg(not(target_os = "android"))]
    #[test]
    #[ignore = "diagnostic: prints machine-specific adapter limits"]
    fn diagnose_probed_hardware_capabilities() {
        use crate::gpu::capabilities::{plan_split, SplitPolicy};

        let Some(ctx) = crate::gpu::probe() else {
            eprintln!("SKIP: no GPU adapter on this host");
            return;
        };
        let caps = ctx.capabilities();
        let g = caps.gpu.as_ref().expect("probe returned a context");

        eprintln!("adapter                            : {}", g.adapter_name);
        eprintln!("backend                            : {}", g.backend);
        eprintln!("class                              : {:?}", g.class);
        eprintln!(
            "max_storage_buffers_per_shader_stage: {}",
            g.max_storage_buffers_per_shader_stage
        );
        eprintln!(
            "max_storage_buffer_binding_size    : {}",
            g.max_storage_buffer_binding_size
        );
        eprintln!("max_buffer_size                    : {}", g.max_buffer_size);
        eprintln!(
            "max_compute_invocations_per_workgroup: {}",
            g.max_compute_invocations_per_workgroup
        );
        eprintln!(
            "max_compute_workgroups_per_dimension : {}",
            g.max_compute_workgroups_per_dimension
        );
        eprintln!("cpu_threads                        : {}", caps.cpu_threads);
        eprintln!("kernel needs storage buffers       : {STORAGE_BUFFERS_NEEDED}");
        eprintln!(
            "  -> supported?                    : {}",
            g.supports_storage_buffers(STORAGE_BUFFERS_NEEDED)
        );
        eprintln!(
            "max chunk items ({QUERY_STRIDE_BYTES} B stride, wg {WORKGROUP_SIZE}): {}",
            g.max_chunk_items(QUERY_STRIDE_BYTES, WORKGROUP_SIZE)
        );

        let split = plan_split(
            1_000_000,
            &caps,
            SplitPolicy::Auto,
            STORAGE_BUFFERS_NEEDED,
            QUERY_STRIDE_BYTES,
            WORKGROUP_SIZE,
        );
        eprintln!(
            "Auto split of 1e6 queries          : gpu={} cpu={} ({:?})",
            split.gpu_items, split.cpu_items, split.reason
        );

        // The whole point of the fix: this kernel must be hostable here.
        assert!(g.supports_storage_buffers(STORAGE_BUFFERS_NEEDED));
    }

    /// The hybrid path must give real work to **both** devices and return the
    /// same answers as the pure-CPU reference.
    ///
    /// **Tori are excluded deliberately.** The GPU quartic root-finder disagrees
    /// with the CPU mirror on `XTorus`/`YTorus`/`ZTorus` (see
    /// `gpu_matches_cpu_reference`'s Results note and its bead). Including them
    /// here would make this test fail for a reason that has nothing to do with
    /// work splitting. The other 12 surface types are compared in full.
    #[cfg(not(target_os = "android"))]
    #[test]
    fn hybrid_splits_work_across_both_devices() {
        use crate::gpu::capabilities::{SplitPolicy, SplitReason};

        let Some(ctx) = crate::gpu::probe() else {
            eprintln!("SKIP hybrid test: no GPU adapter (CPU-only CI)");
            return;
        };

        // Every surface except the three tori (indices 12, 13, 14).
        let surfaces: Vec<SurfaceKind> = one_of_each().into_iter().take(12).collect();
        let encoded = encode_surfaces(&surfaces);

        let mut queries = Vec::new();
        for si in 0..surfaces.len() {
            for ix in -1..=1 {
                for iy in -1..=1 {
                    for i in 0..32 {
                        queries.push(SurfaceQuery {
                            surface: si as u32,
                            coincident: false,
                            r: Position::new(ix as f64 * 1.3, iy as f64 * 1.3, 0.4),
                            u: dir(i, 32),
                        });
                    }
                }
            }
        }

        let reference = surface_distance_cpu_f32(&encoded, &queries);
        let (hybrid, split) =
            surface_distance_hybrid(Some(&ctx), &encoded, &queries, SplitPolicy::Auto);

        assert_eq!(split.reason, SplitReason::Split, "expected a real split");
        assert!(split.gpu_items > 0, "GPU took no load");
        assert!(split.cpu_items > 0, "CPU took no load");
        assert_eq!(split.total(), queries.len());
        assert_eq!(hybrid.len(), reference.len());

        for (i, (&h, &c)) in hybrid.iter().zip(reference.iter()).enumerate() {
            let c_miss = c >= MISS;
            let h_miss = h >= MISS;
            if c_miss || h_miss {
                assert_eq!(
                    c_miss, h_miss,
                    "query {i}: miss disagreement hybrid={h}, cpu={c}"
                );
                continue;
            }
            let tol = 1e-3 * (1.0 + c.abs());
            assert!(
                (h - c).abs() <= tol,
                "query {i}: hybrid={h}, cpu={c}, |diff|={} > tol={tol}",
                (h - c).abs()
            );
        }
    }

    /// With no context the hybrid path must still answer, entirely on the CPU.
    #[cfg(not(target_os = "android"))]
    #[test]
    fn hybrid_without_a_gpu_runs_everything_on_the_cpu() {
        use crate::gpu::capabilities::{SplitPolicy, SplitReason};

        let surfaces = one_of_each();
        let encoded = encode_surfaces(&surfaces);
        let queries: Vec<SurfaceQuery> = (0..64)
            .map(|i| SurfaceQuery {
                surface: (i % surfaces.len()) as u32,
                coincident: false,
                r: Position::new(0.0, 0.0, 0.0),
                u: dir(i, 64),
            })
            .collect();

        let (out, split) = surface_distance_hybrid(None, &encoded, &queries, SplitPolicy::Auto);

        assert_eq!(split.reason, SplitReason::NoGpu);
        assert_eq!(split.cpu_items, queries.len());
        assert_eq!(out, surface_distance_cpu_f32(&encoded, &queries));
    }

    /// Regression for `op-9s8.12`: a cubic must never report more than three real
    /// roots, however badly f32 noise splits a clustered triple root.
    ///
    /// This exact cubic was found by random search during the `op-9s8.11`
    /// investigation. Its true roots are 0.331034, 0.331270 and 1.638816 — the
    /// first two ~1e-4 apart, which is wider than `push_unique`'s dedup radius of
    /// `1.0e-5 * (1 + |r|)`, so the near-double root gets counted twice and the
    /// collector reported **four** entries before the fix.
    ///
    /// On the WGSL side that overflowed `quartic_real_roots`' 3-element `crits`
    /// array and wrote `bp[5]` on a 5-element array. WGSL bounds-clamps rather than
    /// trapping, so it silently corrupted the last breakpoint and dropped roots
    /// beyond the final critical point — a wrong distance with no diagnostic. The
    /// Rust mirror could not reach that state (its buffer is 3), and that asymmetry
    /// between the twins was the defect.
    #[test]
    fn a_clustered_triple_root_cubic_cannot_report_four_roots() {
        let mut out = [0.0f32; 4];
        let n = cubic_real_roots_f32(1.0, -2.30112, 1.19506, -0.179715, &mut out);

        assert!(
            n <= MAX_CUBIC_REAL_ROOTS,
            "cubic reported {n} real roots ({:?}); a cubic has at most {MAX_CUBIC_REAL_ROOTS}",
            &out[..n]
        );

        // The roots it does report must still be the real ones, not garbage.
        for &r in &out[..n] {
            let f = ((1.0 * r - 2.30112) * r + 1.19506) * r - 0.179715;
            assert!(f.abs() < 1.0e-4, "reported root {r} has residual {f}");
        }
    }

    /// Regression for `op-9s8.11`: the exact ray that exposed the FMA cancellation.
    ///
    /// Origin `(0, -1.3, 0)` lies *exactly* on this torus (`a + b = 1.0 + 0.3`), so
    /// the quartic's constant term is analytically zero and the CPU/GPU answer
    /// hinges entirely on how rounding perturbs it. The trusted f64 path gives
    /// 0.4967226591440329; the GPU used to return 3.120881e-7.
    ///
    /// **Scope, stated honestly:** this pins the **CPU mirror** against the f64
    /// reference on the degenerate geometry, so the property is guarded on a
    /// machine with no GPU (where `gpu_matches_cpu_reference` only prints a SKIP).
    /// It would **not** have caught the original defect — the CPU path was always
    /// correct here, because Rust does not contract and so computed `c0 = 0.0`
    /// exactly. Only `gpu_matches_cpu_reference` on real hardware exercises the
    /// contraction path. This test's job is to stop the *degenerate case itself*
    /// from silently regressing, and to fail loudly if the fixture ever drifts off
    /// the surface and stops testing what it claims to.
    #[test]
    fn an_origin_exactly_on_a_torus_does_not_yield_a_spurious_near_zero_root() {
        let torus = XTorus {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            a: 1.0,
            b: 0.3,
            c: 0.3,
            bc: BC,
        };
        let surfaces = vec![SurfaceKind::XTorus(torus)];
        let encoded = encode_surfaces(&surfaces);

        let r = Position::new(0.0, -1.3, 0.0);
        let u = dir(1, 48);

        // The origin really is on the surface — if this drifts, the test is no
        // longer exercising the degenerate case it was written for.
        let sense = surfaces[0].evaluate(r);
        assert!(
            sense.abs() < 1.0e-12,
            "origin is no longer on the torus: {sense}"
        );

        let q = vec![SurfaceQuery {
            surface: 0,
            coincident: false,
            r,
            u,
        }];
        let got = surface_distance_cpu_f32(&encoded, &q)[0];

        let reference = surfaces[0].distance(r, u, false) as f32;
        assert!(
            (got - reference).abs() <= 1.0e-3 * (1.0 + reference.abs()),
            "f32 mirror {got} disagrees with the f64 reference {reference}"
        );
        assert!(
            got > 1.0e-5,
            "returned a spurious near-zero root ({got}); expected ~0.4967"
        );
    }
}
