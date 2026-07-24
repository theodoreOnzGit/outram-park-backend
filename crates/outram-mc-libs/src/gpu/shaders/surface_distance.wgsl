// Ray–surface distance for all 15 CSG SurfaceKind variants (op-9s8.2).
//
// A line-by-line f32 translation of `surface_distance_one_f32` (+ its polynomial
// solver) in `../surface_distance.rs`, which is itself a scalar f32 mirror of the
// trusted f64 `geometry/surface.rs`. This shader is the GPU accelerator; the CPU
// f64 path stays the trusted reference (see the crate CLAUDE.md). One thread per
// query ray computes the smallest positive distance to its surface, or INF on a
// miss. Correctness is checked two ways: naga static validation (no GPU) and a
// GPU-vs-CPU-mirror agreement test (on a host with an adapter).

// INF sentinel = the Rust `MISS` constant: "ray does not cross". Far past any cm
// distance yet well inside f32::MAX, so it survives arithmetic and read-back.
const INF: f32 = 1e30;

// Per-surface coefficient stride, matching `SURF_STRIDE` in surface_distance.rs.
const STRIDE: u32 = 12u;

@group(0) @binding(0) var<storage, read> tags: array<u32>;     // one per surface
@group(0) @binding(1) var<storage, read> coeffs: array<f32>;   // STRIDE per surface
@group(0) @binding(2) var<storage, read> q_surf: array<u32>;   // surface index per query
@group(0) @binding(3) var<storage, read> q_flag: array<u32>;   // coincident (0/1) per query
@group(0) @binding(4) var<storage, read> q_r: array<f32>;      // origin, stride 3 per query
@group(0) @binding(5) var<storage, read> q_u: array<f32>;      // direction, stride 3 per query
@group(0) @binding(6) var<uniform> params: vec4<u32>;          // (n_surf, n_query, 0, 0)
@group(0) @binding(7) var<storage, read_write> result: array<f32>;

// ── Polynomial real-root solver (f32 twin of the surface.rs solver) ─────────────
//
// Descending-coefficient polynomials packed front-aligned in an array<f32,5> with
// an explicit length. Roots are returned by value (a fixed array + count) so no
// pointers are needed. Same derivative-bracketing + bisection/Newton method as the
// f64 reference; robust and BLAS-free.

struct Roots {
    r: array<f32, 4>,
    n: u32,
};

// Horner evaluation over the first `n` (leading-first) coefficients.
fn poly_eval(c: array<f32, 5>, n: u32, x: f32) -> f32 {
    var acc: f32 = 0.0;
    for (var i: u32 = 0u; i < n; i = i + 1u) {
        acc = acc * x + c[i];
    }
    return acc;
}

// Sum of absolute Horner terms — relative-residual scale for multiple roots.
fn poly_abs_scale(c: array<f32, 5>, n: u32, x: f32) -> f32 {
    let ax = abs(x);
    var acc: f32 = 0.0;
    for (var i: u32 = 0u; i < n; i = i + 1u) {
        acc = acc * ax + abs(c[i]);
    }
    return acc;
}

// Cauchy bound on |root|: 1 + max_i |c[i]/c[0]|.
fn root_bound(c: array<f32, 5>, n: u32) -> f32 {
    let lead = c[0];
    var m: f32 = 0.0;
    for (var i: u32 = 1u; i < n; i = i + 1u) {
        let a = abs(c[i] / lead);
        if (a > m) { m = a; }
    }
    return 1.0 + m;
}

// Insert `r` into `acc` unless a near-duplicate is present; returns updated acc.
fn push_unique(acc: Roots, r: f32) -> Roots {
    var out = acc;
    for (var i: u32 = 0u; i < out.n; i = i + 1u) {
        if (abs(out.r[i] - r) <= 1.0e-5 * (1.0 + abs(r))) {
            return out;
        }
    }
    if (out.n < 4u) {
        out.r[out.n] = r;
        out.n = out.n + 1u;
    }
    return out;
}

// Ascending insertion sort of the first `acc.n` roots.
fn sort_roots(acc: Roots) -> Roots {
    var out = acc;
    for (var i: u32 = 1u; i < out.n; i = i + 1u) {
        var j = i;
        loop {
            if (j == 0u) { break; }
            if (out.r[j - 1u] <= out.r[j]) { break; }
            let tmp = out.r[j - 1u];
            out.r[j - 1u] = out.r[j];
            out.r[j] = tmp;
            j = j - 1u;
        }
    }
    return out;
}

// Bisection + Newton polish of the single root of `c` in [lo, hi] (endpoints
// straddle zero). `d` is the derivative (leading-first, length nd).
fn bisect_root(c: array<f32, 5>, nc: u32, d: array<f32, 5>, nd: u32, lo: f32, hi: f32) -> f32 {
    var a = lo;
    var b = hi;
    var fa = poly_eval(c, nc, a);
    for (var it: u32 = 0u; it < 100u; it = it + 1u) {
        let mid = 0.5 * (a + b);
        let fm = poly_eval(c, nc, mid);
        if (fm == 0.0) { return mid; }
        if ((fm < 0.0) == (fa < 0.0)) {
            a = mid;
            fa = fm;
        } else {
            b = mid;
        }
        if (abs(b - a) <= 1.0e-7 * (1.0 + abs(mid))) { break; }
    }
    var x = 0.5 * (a + b);
    for (var it: u32 = 0u; it < 8u; it = it + 1u) {
        let f = poly_eval(c, nc, x);
        let df = poly_eval(d, nd, x);
        if (df == 0.0) { break; }
        let step = f / df;
        x = x - step;
        if (abs(step) <= 1.0e-7 * (1.0 + abs(x))) { break; }
    }
    return x;
}

// Collect real roots of `c` (len nc) given derivative `d` (len nd) and sorted
// critical points `crits` (len ncr): breakpoints are ±m and the crits; a simple
// root lives wherever consecutive breakpoints straddle zero; even-multiplicity
// roots sit at a critical point (residual test).
fn collect_real_roots(
    c: array<f32, 5>, nc: u32,
    d: array<f32, 5>, nd: u32,
    crits: array<f32, 3>, ncr: u32,
) -> Roots {
    var m = root_bound(c, nc);
    for (var i: u32 = 0u; i < ncr; i = i + 1u) {
        m = max(m, abs(crits[i]));
    }
    m = m + 1.0;

    // Breakpoints: -m, crits..., +m  (at most 5).
    var bp: array<f32, 5>;
    var nb: u32 = 0u;
    bp[nb] = -m; nb = nb + 1u;
    for (var i: u32 = 0u; i < ncr; i = i + 1u) {
        bp[nb] = crits[i]; nb = nb + 1u;
    }
    bp[nb] = m; nb = nb + 1u;

    var acc: Roots;
    acc.n = 0u;
    for (var i: u32 = 0u; i + 1u < nb; i = i + 1u) {
        let lo = bp[i];
        let hi = bp[i + 1u];
        let flo = poly_eval(c, nc, lo);
        let fhi = poly_eval(c, nc, hi);
        if ((flo < 0.0 && fhi > 0.0) || (flo > 0.0 && fhi < 0.0)) {
            let r = bisect_root(c, nc, d, nd, lo, hi);
            acc = push_unique(acc, r);
        }
    }
    for (var i: u32 = 0u; i < ncr; i = i + 1u) {
        let cc = crits[i];
        let residual = abs(poly_eval(c, nc, cc));
        let scale = poly_abs_scale(c, nc, cc);
        if (residual <= 1.0e-6 * scale + 1.0e-30) {
            acc = push_unique(acc, cc);
        }
    }
    return sort_roots(acc);
}

// Real roots of a x² + b x + c, ascending.
fn quadratic_real_roots(a: f32, b: f32, c: f32) -> Roots {
    var out: Roots;
    out.n = 0u;
    if (abs(a) < 1.0e-7 * (1.0 + abs(b) + abs(c))) {
        if (abs(b) < 1.0e-7 * (1.0 + abs(c))) {
            return out;
        }
        out.r[0] = -c / b;
        out.n = 1u;
        return out;
    }
    let disc = b * b - 4.0 * a * c;
    if (disc < 0.0) { return out; }
    if (disc == 0.0) {
        out.r[0] = -b / (2.0 * a);
        out.n = 1u;
        return out;
    }
    let sq = sqrt(disc);
    let r1 = (-b - sq) / (2.0 * a);
    let r2 = (-b + sq) / (2.0 * a);
    out.r[0] = min(r1, r2);
    out.r[1] = max(r1, r2);
    out.n = 2u;
    return out;
}

// Real roots of a x³ + b x² + c x + d, ascending.
fn cubic_real_roots(a: f32, b: f32, c: f32, d: f32) -> Roots {
    if (abs(a) < 1.0e-7 * (1.0 + abs(b) + abs(c) + abs(d))) {
        return quadratic_real_roots(b, c, d);
    }
    let coeffs = array<f32, 5>(a, b, c, d, 0.0);
    let deriv = array<f32, 5>(3.0 * a, 2.0 * b, c, 0.0, 0.0);
    let crit2 = quadratic_real_roots(3.0 * a, 2.0 * b, c);
    let crits = array<f32, 3>(crit2.r[0], crit2.r[1], 0.0);
    return collect_real_roots(coeffs, 4u, deriv, 3u, crits, crit2.n);
}

// Real roots of a x⁴ + b x³ + c x² + d x + e, ascending.
fn quartic_real_roots(a: f32, b: f32, c: f32, d: f32, e: f32) -> Roots {
    if (abs(a) < 1.0e-7 * (1.0 + abs(b) + abs(c) + abs(d) + abs(e))) {
        return cubic_real_roots(b, c, d, e);
    }
    let coeffs = array<f32, 5>(a, b, c, d, e);
    let deriv = array<f32, 5>(4.0 * a, 3.0 * b, 2.0 * c, d, 0.0);
    let crit3 = cubic_real_roots(4.0 * a, 3.0 * b, 2.0 * c, d);
    // crit3 is already ascending from collect_real_roots / quadratic fallback.
    let crits = array<f32, 3>(crit3.r[0], crit3.r[1], crit3.r[2]);
    return collect_real_roots(coeffs, 5u, deriv, 4u, crits, crit3.n);
}

// ── Quadric / cylinder / torus helpers ──────────────────────────────────────────

// Smallest root d > eps of a d² + b d + c, else INF (linear fallback when a≈0).
fn smallest_positive_root(a: f32, b: f32, c: f32, eps: f32) -> f32 {
    if (abs(a) < 1.0e-7) {
        if (abs(b) < 1.0e-7) { return INF; }
        let d = -c / b;
        return select(INF, d, d > eps);
    }
    let disc = b * b - 4.0 * a * c;
    if (disc < 0.0) { return INF; }
    let sq = sqrt(disc);
    let inv = 0.5 / a;
    let d1 = (-b - sq) * inv;
    let d2 = (-b + sq) * inv;
    let lo = min(d1, d2);
    let hi = max(d1, d2);
    if (lo > eps) { return lo; }
    if (hi > eps) { return hi; }
    return INF;
}

// Axis-aligned cylinder distance in the radial (d1,d2)/(w1,w2) frame.
fn axis_cylinder(d1: f32, d2: f32, w1: f32, w2: f32, radius: f32, coincident: bool) -> f32 {
    let a = w1 * w1 + w2 * w2;
    if (a == 0.0) { return INF; }
    let k = d1 * w1 + d2 * w2;
    let cc = d1 * d1 + d2 * d2 - radius * radius;
    let quad = k * k - a * cc;
    if (quad < 0.0) { return INF; }
    let sq = sqrt(quad);
    if (coincident || abs(cc) < 1.0e-6) {
        if (k >= 0.0) { return INF; }
        return (-k + sq) / a;
    }
    if (cc < 0.0) {
        return (-k + sq) / a;
    }
    let d = (-k - sq) / a;
    if (d < 0.0) { return INF; }
    return d;
}

// Smallest positive ray–torus distance in the (r1,r2,ax) frame.
fn torus_distance(
    p1: f32, p2: f32, pax: f32,
    u1: f32, u2: f32, uax: f32,
    a: f32, b: f32, c: f32,
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
    let c0 = gc * gc - 4.0 * a2 * gamma;

    let roots = quartic_real_roots(c4, c3, c2, c1, c0);

    var eps: f32 = 1.0e-7;
    if (coincident) { eps = 1.0e-5; }
    var best: f32 = INF;
    for (var i: u32 = 0u; i < roots.n; i = i + 1u) {
        let t = roots.r[i];
        if (t <= eps) { continue; }
        let gt = ga * t * t + 2.0 * gb * t + gc;
        let tol = 1.0e-6 * (1.0 + ga * t * t + abs(2.0 * gb * t) + abs(gc));
        if (gt >= -tol && t < best) {
            best = t;
        }
    }
    return best;
}

// ── Per-surface distance dispatch (mirror of surface_distance_one_f32) ──────────

fn surface_distance(tag: u32, c: array<f32, 12>, r: vec3<f32>, u: vec3<f32>, coincident: bool) -> f32 {
    let rx = r.x; let ry = r.y; let rz = r.z;
    let uu = u.x; let uv = u.y; let uw = u.z;

    switch (tag) {
        case 0u: { // XPlane { x0 }
            var hint: f32 = 0.0;
            if (coincident) { hint = 1.0e-7; }
            if (abs(uu) < 1.0e-7) { return INF; }
            let d = (c[0] - rx) / uu;
            return select(INF, d, d > hint);
        }
        case 1u: { // YPlane { y0 }
            var hint: f32 = 0.0;
            if (coincident) { hint = 1.0e-7; }
            if (abs(uv) < 1.0e-7) { return INF; }
            let d = (c[0] - ry) / uv;
            return select(INF, d, d > hint);
        }
        case 2u: { // ZPlane { z0 }
            var hint: f32 = 0.0;
            if (coincident) { hint = 1.0e-7; }
            if (abs(uw) < 1.0e-7) { return INF; }
            let d = (c[0] - rz) / uw;
            return select(INF, d, d > hint);
        }
        case 3u: { // Plane { a, b, c, d }
            var hint: f32 = 0.0;
            if (coincident) { hint = 1.0e-7; }
            let denom = c[0] * uu + c[1] * uv + c[2] * uw;
            if (abs(denom) < 1.0e-7) { return INF; }
            let d = -(c[0] * rx + c[1] * ry + c[2] * rz - c[3]) / denom;
            return select(INF, d, d > hint);
        }
        case 4u: { // Sphere { x0, y0, z0, r }
            let ox = rx - c[0];
            let oy = ry - c[1];
            let oz = rz - c[2];
            let k = ox * uu + oy * uv + oz * uw;
            var cc: f32 = 0.0;
            if (!coincident) { cc = ox * ox + oy * oy + oz * oz - c[3] * c[3]; }
            let disc = k * k - cc;
            if (disc < 0.0) { return INF; }
            let sq = sqrt(disc);
            let d_near = -k - sq;
            if (d_near > 1.0e-6) { return d_near; }
            let d_far = -k + sq;
            if (d_far > 1.0e-6) { return d_far; }
            return INF;
        }
        case 5u: { // XCylinder { y0, z0, r } — radial pair (y, z), axis x
            return axis_cylinder(ry - c[0], rz - c[1], uv, uw, c[2], coincident);
        }
        case 6u: { // YCylinder { x0, z0, r } — radial pair (x, z), axis y
            return axis_cylinder(rx - c[0], rz - c[1], uu, uw, c[2], coincident);
        }
        case 7u: { // ZCylinder { x0, y0, r } — radial pair (x, y), axis z
            return axis_cylinder(rx - c[0], ry - c[1], uu, uv, c[2], coincident);
        }
        case 8u: { // XCone { x0, y0, z0, r_sq } — axis x
            let dx = rx - c[0];
            let dy = ry - c[1];
            let dz = rz - c[2];
            let rsq = c[3];
            let a = uv * uv + uw * uw - rsq * uu * uu;
            let k = dy * uv + dz * uw - rsq * dx * uu;
            var cc: f32 = 0.0;
            if (!coincident) { cc = dy * dy + dz * dz - rsq * dx * dx; }
            return smallest_positive_root(a, 2.0 * k, cc, 1.0e-7);
        }
        case 9u: { // YCone — axis y
            let dx = rx - c[0];
            let dy = ry - c[1];
            let dz = rz - c[2];
            let rsq = c[3];
            let a = uu * uu + uw * uw - rsq * uv * uv;
            let k = dx * uu + dz * uw - rsq * dy * uv;
            var cc: f32 = 0.0;
            if (!coincident) { cc = dx * dx + dz * dz - rsq * dy * dy; }
            return smallest_positive_root(a, 2.0 * k, cc, 1.0e-7);
        }
        case 10u: { // ZCone — axis z
            let dx = rx - c[0];
            let dy = ry - c[1];
            let dz = rz - c[2];
            let rsq = c[3];
            let a = uu * uu + uv * uv - rsq * uw * uw;
            let k = dx * uu + dy * uv - rsq * dz * uw;
            var cc: f32 = 0.0;
            if (!coincident) { cc = dx * dx + dy * dy - rsq * dz * dz; }
            return smallest_positive_root(a, 2.0 * k, cc, 1.0e-7);
        }
        case 11u: { // Quadric { a,b,c,d,e,f,g,h,j,k }
            let qa = c[0]; let qb = c[1]; let qc = c[2]; let qd = c[3]; let qe = c[4];
            let qf = c[5]; let qg = c[6]; let qh = c[7]; let qj = c[8]; let qk = c[9];
            let a_q = qa * uu * uu + qb * uv * uv + qc * uw * uw
                + qd * uu * uv + qe * uv * uw + qf * uu * uw;
            let b_q = 2.0 * qa * rx * uu + 2.0 * qb * ry * uv + 2.0 * qc * rz * uw
                + qd * (rx * uv + ry * uu)
                + qe * (ry * uw + rz * uv)
                + qf * (rx * uw + rz * uu)
                + qg * uu + qh * uv + qj * uw;
            var c_q: f32 = 0.0;
            if (!coincident) {
                c_q = qa * rx * rx + qb * ry * ry + qc * rz * rz
                    + qd * rx * ry + qe * ry * rz + qf * rx * rz
                    + qg * rx + qh * ry + qj * rz + qk;
            }
            return smallest_positive_root(a_q, b_q, c_q, 1.0e-7);
        }
        case 12u: { // XTorus — radial pair (y, z), axial x
            return torus_distance(ry - c[1], rz - c[2], rx - c[0], uv, uw, uu, c[3], c[4], c[5], coincident);
        }
        case 13u: { // YTorus — radial pair (x, z), axial y
            return torus_distance(rx - c[0], rz - c[2], ry - c[1], uu, uw, uv, c[3], c[4], c[5], coincident);
        }
        case 14u: { // ZTorus — radial pair (x, y), axial z
            return torus_distance(rx - c[0], ry - c[1], rz - c[2], uu, uv, uw, c[3], c[4], c[5], coincident);
        }
        default: {
            return INF;
        }
    }
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let qi = gid.x;
    let n_query = params.y;
    if (qi >= n_query) { return; }

    let si = q_surf[qi];
    let tag = tags[si];
    let base = si * STRIDE;

    var c: array<f32, 12>;
    for (var k: u32 = 0u; k < STRIDE; k = k + 1u) {
        c[k] = coeffs[base + k];
    }

    let r = vec3<f32>(q_r[qi * 3u], q_r[qi * 3u + 1u], q_r[qi * 3u + 2u]);
    let u = vec3<f32>(q_u[qi * 3u], q_u[qi * 3u + 1u], q_u[qi * 3u + 2u]);
    let coincident = q_flag[qi] != 0u;

    result[qi] = surface_distance(tag, c, r, u, coincident);
}
