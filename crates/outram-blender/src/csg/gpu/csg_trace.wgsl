// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// GPU ray tracer for an ASSEMBLED CSG geometry (gh:#587). An f32 transcription
// of this crate's own CPU plotter, which is the reference:
//   src/csg/plot/raytrace.rs   trace(), PhongRay, ProjectionRay, ClipPlane,
//                              Camera::pixel_ray, WireframeRayTracePlot
//   src/csg/plot/slice.rs      SlicePlot::id_map / pixel_centre
//   src/csg/geometry.rs        Geometry::locate, distance_to_boundary
//   src/csg/cell.rs            Cell::contains, distance_to_boundary
//   src/csg/lattice/{rect,hex}.rs  get_indices, tile_center, distance
//   src/csg/surface/quadric.rs evaluate, normal
// themselves ports of OpenMC (MIT; see NOTICE and LICENSE.openmc). The
// per-surface distance switch is copied from outram-mc-libs'
// src/gpu/shaders/surface_distance.wgsl (op-9s8.2), same tags and layout, minus
// the tori (refused by flat.rs).
//
// Buffer layout: see flat.rs. One invocation = one pixel.
//
// Differences from the CPU, all from f32:
//  - the nudge past a crossing is 1e-5 cm + 4e-7 * distance travelled
//    (CPU: 1e-8 cm), larger than the f32 position error;
//  - after a crossing the path is re-located from the crossing's level after
//    checking that every ancestor cell (and lattice tile) still holds the
//    point (CPU: from the root every time; same answer when cells do not
//    overlap, which the HTR-10 build guarantees);
//  - coincidence tolerances are f32-sized; a crossing closer than 10 nudges
//    is not reported (the CPU: 10 * TINY_BIT), so the shadow ray's
//    re-crossing of the surface it starts on is not taken for a shadow;
//  - the cell entered after a crossing is looked for first among the cells
//    naming that half-space (flat.rs's crossing hints), then everywhere;
//  - a fill whose whole subtree holds no drawn material is crossed as one
//    cell (solid and x-ray only): the CPU walks it, reporting crossings that
//    change nothing in the picture;
//  - in a universe with a grid index (flat.rs, index.rs) a point is looked
//    for only among its voxel's candidates, each cell's region cut down to
//    the surfaces passing through the voxel; the voxel's faces are silent
//    boundaries (nothing reported, no re-location). Same cell, same
//    distances inside the voxel; the CPU scans the whole universe;
//  - a ray outside a closed root grid (no cell outside it) jumps to the
//    grid's box; the CPU steps from one root-cell surface to the next,
//    through their extensions beyond the model;
//  - a crossing out of a cell whose region is not a plain intersection is
//    signed from the surface normal (as the CPU does since 2026-10-05,
//    upstream's distance_complex), not from the region's token.

const INF: f32 = 1e30;
const NONE: u32 = 0xFFFFFFFFu;
const MAX_DEPTH: u32 = 10u;
const TOK_INTER: u32 = 0xFFFFFFF0u;
const TOK_UNION: u32 = 0xFFFFFFF1u;
const TOK_COMP: u32 = 0xFFFFFFF2u;
const SURF_WORDS: u32 = 12u;
const CELL_WORDS: u32 = 8u;
const LAT_WORDS: u32 = 12u;
const CAND_WORDS: u32 = 4u;
// Voxel steps a ray may take (apart from boundary crossings).
const MAX_VOXEL_STEPS: u32 = 4096u;
const FOCAL_PLANE_DIST: f32 = 10.0;
const S3: f32 = 1.7320508;

// Output ids (aux[2p]): >= 0 colour index; these otherwise.
const ID_BACKGROUND: i32 = -1;
const ID_OVERLAP: i32 = -2;
const ID_VOID: i32 = -3;
// Inside a fill whose materials are all hidden (never written out).
const ID_HIDDEN: i32 = -4;

struct Params {
    // 0 solid, 1 x-ray (wireframe), 2 slice
    mode: u32,
    width: u32,
    height: u32,
    row0: u32,
    // padded row length of the colour buffer (pixels)
    stride: u32,
    n_mat: u32,
    // bit0 clip, bit1 orthographic, bit2 wireframe ids filter in use
    flags: u32,
    max_steps: u32,
    // xyz camera position; w orthographic width
    cam: vec4<f32>,
    // camera-to-model matrix rows; m0.w = dx, m1.w = dy (perspective)
    m0: vec4<f32>,
    m1: vec4<f32>,
    m2: vec4<f32>,
    // xyz light position; w diffuse fraction
    light: vec4<f32>,
    // xyz clip normal (into the kept side); w offset
    clip: vec4<f32>,
    // slice: first pixel centre, step along a row, step down a column
    s_start: vec4<f32>,
    s_u: vec4<f32>,
    s_v: vec4<f32>,
    // background, overlap, wire, void colours (0xAABBGGRR)
    colours: vec4<u32>,
};

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var<storage, read> geo: array<u32>;
// per colour index: (rgba, flags bit0 opaque/shown bit1 outlined, xs bits, 0)
@group(0) @binding(2) var<storage, read> mats: array<vec4<u32>>;
@group(0) @binding(3) var<storage, read_write> out_colour: array<u32>;
// per pixel: (id or wire hash, segments)
@group(0) @binding(4) var<storage, read_write> aux: array<u32>;

// ── the path (OpenMC's coordinate stack), per invocation ─────────────────────
var<private> lv_uni: array<u32, MAX_DEPTH>;
var<private> lv_cell: array<u32, MAX_DEPTH>;
var<private> lv_lat: array<u32, MAX_DEPTH>;
var<private> lv_idx: array<vec3<i32>, MAX_DEPTH>;
var<private> lv_off: array<vec3<f32>, MAX_DEPTH>;
var<private> depth: u32;
// The token range each level's cell is tested with: the cell's own, or its
// folded copy in the level's current voxel (lv_vox, NONE when the universe
// has no grid or the point is outside it).
struct Toks {
    s: u32,
    n: u32,
    f: u32,
};
var<private> lv_toks: array<Toks, MAX_DEPTH>;
var<private> lv_vox: array<u32, MAX_DEPTH>;
var<private> cur_mat: i32;
// the surface token: which surface the ray sits on, and on which side
var<private> tok_surf: u32;
var<private> tok_out: bool;
// the ray: position = ray_o + ray_t * ray_u
var<private> ray_o: vec3<f32>;
var<private> ray_u: vec3<f32>;
var<private> ray_t: f32;

fn gf(i: u32) -> f32 { return bitcast<f32>(geo[i]); }
fn h(i: u32) -> u32 { return geo[i]; }

fn pos() -> vec3<f32> { return ray_o + ray_t * ray_u; }
fn local(k: u32) -> vec3<f32> { return (ray_o - lv_off[k]) + ray_t * ray_u; }
fn nudge() -> f32 { return 1.0e-5 + 4.0e-7 * abs(ray_t); }

// ── surfaces ─────────────────────────────────────────────────────────────────

fn sbase(s: u32) -> u32 { return h(3u) + s * SURF_WORDS; }
fn sc(s: u32, k: u32) -> f32 { return gf(sbase(s) + 1u + k); }

fn surf_eval(s: u32, r: vec3<f32>) -> f32 {
    let tag = h(sbase(s));
    switch (tag) {
        case 0u: { return r.x - sc(s, 0u); }
        case 1u: { return r.y - sc(s, 0u); }
        case 2u: { return r.z - sc(s, 0u); }
        case 3u: { return sc(s, 0u) * r.x + sc(s, 1u) * r.y + sc(s, 2u) * r.z - sc(s, 3u); }
        case 4u: {
            let d = r - vec3<f32>(sc(s, 0u), sc(s, 1u), sc(s, 2u));
            return dot(d, d) - sc(s, 3u) * sc(s, 3u);
        }
        case 5u: { let a = r.y - sc(s, 0u); let b = r.z - sc(s, 1u); return a * a + b * b - sc(s, 2u) * sc(s, 2u); }
        case 6u: { let a = r.x - sc(s, 0u); let b = r.z - sc(s, 1u); return a * a + b * b - sc(s, 2u) * sc(s, 2u); }
        case 7u: { let a = r.x - sc(s, 0u); let b = r.y - sc(s, 1u); return a * a + b * b - sc(s, 2u) * sc(s, 2u); }
        case 8u: {
            let d = r - vec3<f32>(sc(s, 0u), sc(s, 1u), sc(s, 2u));
            return d.y * d.y + d.z * d.z - sc(s, 3u) * d.x * d.x;
        }
        case 9u: {
            let d = r - vec3<f32>(sc(s, 0u), sc(s, 1u), sc(s, 2u));
            return d.x * d.x + d.z * d.z - sc(s, 3u) * d.y * d.y;
        }
        case 10u: {
            let d = r - vec3<f32>(sc(s, 0u), sc(s, 1u), sc(s, 2u));
            return d.x * d.x + d.y * d.y - sc(s, 3u) * d.z * d.z;
        }
        case 11u: {
            let x = r.x; let y = r.y; let z = r.z;
            return sc(s, 0u) * x * x + sc(s, 1u) * y * y + sc(s, 2u) * z * z
                + sc(s, 3u) * x * y + sc(s, 4u) * y * z + sc(s, 5u) * x * z
                + sc(s, 6u) * x + sc(s, 7u) * y + sc(s, 8u) * z + sc(s, 9u);
        }
        default: { return 0.0; }
    }
}

fn surf_normal(s: u32, r: vec3<f32>) -> vec3<f32> {
    let tag = h(sbase(s));
    switch (tag) {
        case 0u: { return vec3<f32>(1.0, 0.0, 0.0); }
        case 1u: { return vec3<f32>(0.0, 1.0, 0.0); }
        case 2u: { return vec3<f32>(0.0, 0.0, 1.0); }
        case 3u: { return vec3<f32>(sc(s, 0u), sc(s, 1u), sc(s, 2u)); }
        case 4u: { return r - vec3<f32>(sc(s, 0u), sc(s, 1u), sc(s, 2u)); }
        case 5u: { return vec3<f32>(0.0, r.y - sc(s, 0u), r.z - sc(s, 1u)); }
        case 6u: { return vec3<f32>(r.x - sc(s, 0u), 0.0, r.z - sc(s, 1u)); }
        case 7u: { return vec3<f32>(r.x - sc(s, 0u), r.y - sc(s, 1u), 0.0); }
        case 8u: {
            let d = r - vec3<f32>(sc(s, 0u), sc(s, 1u), sc(s, 2u));
            return vec3<f32>(-2.0 * sc(s, 3u) * d.x, 2.0 * d.y, 2.0 * d.z);
        }
        case 9u: {
            let d = r - vec3<f32>(sc(s, 0u), sc(s, 1u), sc(s, 2u));
            return vec3<f32>(2.0 * d.x, -2.0 * sc(s, 3u) * d.y, 2.0 * d.z);
        }
        case 10u: {
            let d = r - vec3<f32>(sc(s, 0u), sc(s, 1u), sc(s, 2u));
            return vec3<f32>(2.0 * d.x, 2.0 * d.y, -2.0 * sc(s, 3u) * d.z);
        }
        case 11u: {
            let x = r.x; let y = r.y; let z = r.z;
            return vec3<f32>(
                2.0 * sc(s, 0u) * x + sc(s, 3u) * y + sc(s, 5u) * z + sc(s, 6u),
                2.0 * sc(s, 1u) * y + sc(s, 3u) * x + sc(s, 4u) * z + sc(s, 7u),
                2.0 * sc(s, 2u) * z + sc(s, 4u) * y + sc(s, 5u) * x + sc(s, 8u));
        }
        default: { return vec3<f32>(0.0, 0.0, 1.0); }
    }
}

// SurfaceKind::sense: true = positive side; on the surface, by direction.
fn surf_sense(s: u32, r: vec3<f32>, u: vec3<f32>) -> bool {
    let f = surf_eval(s, r);
    if (f == 0.0) {
        return dot(surf_normal(s, r), u) > 0.0;
    }
    return f > 0.0;
}

// From outram-mc-libs surface_distance.wgsl.
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

// From outram-mc-libs surface_distance.wgsl.
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

// From outram-mc-libs surface_distance.wgsl (tags 0-11).
fn surf_distance(s: u32, r: vec3<f32>, u: vec3<f32>, coincident: bool) -> f32 {
    let tag = h(sbase(s));
    let rx = r.x; let ry = r.y; let rz = r.z;
    let uu = u.x; let uv = u.y; let uw = u.z;
    var hint: f32 = 0.0;
    if (coincident) { hint = 1.0e-7; }
    switch (tag) {
        case 0u: {
            if (abs(uu) < 1.0e-7) { return INF; }
            let d = (sc(s, 0u) - rx) / uu;
            return select(INF, d, d > hint);
        }
        case 1u: {
            if (abs(uv) < 1.0e-7) { return INF; }
            let d = (sc(s, 0u) - ry) / uv;
            return select(INF, d, d > hint);
        }
        case 2u: {
            if (abs(uw) < 1.0e-7) { return INF; }
            let d = (sc(s, 0u) - rz) / uw;
            return select(INF, d, d > hint);
        }
        case 3u: {
            let denom = sc(s, 0u) * uu + sc(s, 1u) * uv + sc(s, 2u) * uw;
            if (abs(denom) < 1.0e-7) { return INF; }
            let d = -(sc(s, 0u) * rx + sc(s, 1u) * ry + sc(s, 2u) * rz - sc(s, 3u)) / denom;
            return select(INF, d, d > hint);
        }
        case 4u: {
            let ox = rx - sc(s, 0u);
            let oy = ry - sc(s, 1u);
            let oz = rz - sc(s, 2u);
            let k = ox * uu + oy * uv + oz * uw;
            var cc: f32 = 0.0;
            if (!coincident) { cc = ox * ox + oy * oy + oz * oz - sc(s, 3u) * sc(s, 3u); }
            let disc = k * k - cc;
            if (disc < 0.0) { return INF; }
            let sq = sqrt(disc);
            let d_near = -k - sq;
            if (d_near > 1.0e-6) { return d_near; }
            let d_far = -k + sq;
            if (d_far > 1.0e-6) { return d_far; }
            return INF;
        }
        case 5u: { return axis_cylinder(ry - sc(s, 0u), rz - sc(s, 1u), uv, uw, sc(s, 2u), coincident); }
        case 6u: { return axis_cylinder(rx - sc(s, 0u), rz - sc(s, 1u), uu, uw, sc(s, 2u), coincident); }
        case 7u: { return axis_cylinder(rx - sc(s, 0u), ry - sc(s, 1u), uu, uv, sc(s, 2u), coincident); }
        case 8u: {
            let dx = rx - sc(s, 0u); let dy = ry - sc(s, 1u); let dz = rz - sc(s, 2u);
            let rsq = sc(s, 3u);
            let a = uv * uv + uw * uw - rsq * uu * uu;
            let k = dy * uv + dz * uw - rsq * dx * uu;
            var cc: f32 = 0.0;
            if (!coincident) { cc = dy * dy + dz * dz - rsq * dx * dx; }
            return smallest_positive_root(a, 2.0 * k, cc, 1.0e-7);
        }
        case 9u: {
            let dx = rx - sc(s, 0u); let dy = ry - sc(s, 1u); let dz = rz - sc(s, 2u);
            let rsq = sc(s, 3u);
            let a = uu * uu + uw * uw - rsq * uv * uv;
            let k = dx * uu + dz * uw - rsq * dy * uv;
            var cc: f32 = 0.0;
            if (!coincident) { cc = dx * dx + dz * dz - rsq * dy * dy; }
            return smallest_positive_root(a, 2.0 * k, cc, 1.0e-7);
        }
        case 10u: {
            let dx = rx - sc(s, 0u); let dy = ry - sc(s, 1u); let dz = rz - sc(s, 2u);
            let rsq = sc(s, 3u);
            let a = uu * uu + uv * uv - rsq * uw * uw;
            let k = dx * uu + dy * uv - rsq * dz * uw;
            var cc: f32 = 0.0;
            if (!coincident) { cc = dx * dx + dy * dy - rsq * dz * dz; }
            return smallest_positive_root(a, 2.0 * k, cc, 1.0e-7);
        }
        case 11u: {
            let qa = sc(s, 0u); let qb = sc(s, 1u); let qc = sc(s, 2u); let qd = sc(s, 3u); let qe = sc(s, 4u);
            let qf = sc(s, 5u); let qg = sc(s, 6u); let qh = sc(s, 7u); let qj = sc(s, 8u); let qk = sc(s, 9u);
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
        default: { return INF; }
    }
}

// ── cells ────────────────────────────────────────────────────────────────────

fn cbase(c: u32) -> u32 { return h(5u) + c * CELL_WORDS; }

// One half-space token, honouring the surface token (Cell::contains).
fn halfspace(t: u32, r: vec3<f32>, u: vec3<f32>) -> bool {
    let s = t >> 1u;
    let want_out = (t & 1u) == 1u;
    if (s == tok_surf) {
        return tok_out == want_out;
    }
    return surf_sense(s, r, u) == want_out;
}

fn full_toks(c: u32) -> Toks {
    let b = cbase(c);
    return Toks(h(10u) + h(b), h(b + 1u), h(b + 7u));
}

fn cell_contains(c: u32, r: vec3<f32>, u: vec3<f32>) -> bool {
    return toks_contain(full_toks(c), r, u);
}

fn toks_contain(t: Toks, r: vec3<f32>, u: vec3<f32>) -> bool {
    let ts = t.s;
    let tl = t.n;
    if (tl == 0u) { return true; }
    if ((t.f & 1u) == 1u) {
        for (var i: u32 = 0u; i < tl; i = i + 1u) {
            let t = h(ts + i);
            if (t < TOK_INTER && !halfspace(t, r, u)) { return false; }
        }
        return true;
    }
    var st: u32 = 0u;
    var sp: u32 = 0u;
    for (var i: u32 = 0u; i < tl; i = i + 1u) {
        let t = h(ts + i);
        if (t < TOK_INTER) {
            if (sp >= 32u) { return false; }
            let v = select(0u, 1u, halfspace(t, r, u));
            st = (st & ~(1u << sp)) | (v << sp);
            sp = sp + 1u;
        } else if (t == TOK_COMP) {
            if (sp < 1u) { return false; }
            st = st ^ (1u << (sp - 1u));
        } else {
            if (sp < 2u) { return false; }
            let a = (st >> (sp - 2u)) & 1u;
            let bb = (st >> (sp - 1u)) & 1u;
            var v: u32 = a & bb;
            if (t == TOK_UNION) { v = a | bb; }
            sp = sp - 1u;
            st = (st & ~(1u << (sp - 1u))) | (v << (sp - 1u));
        }
    }
    if (sp == 0u) { return false; }
    return ((st >> (sp - 1u)) & 1u) == 1u;
}

struct Hit {
    d: f32,
    s: u32,
};

// Cell::distance_to_boundary.
fn cell_distance(c: u32, r: vec3<f32>, u: vec3<f32>) -> Hit {
    return toks_distance(full_toks(c), r, u);
}

fn toks_distance(tk: Toks, r: vec3<f32>, u: vec3<f32>) -> Hit {
    let ts = tk.s;
    let tl = tk.n;
    var best = Hit(INF, NONE);
    for (var i: u32 = 0u; i < tl; i = i + 1u) {
        let t = h(ts + i);
        if (t >= TOK_INTER) { continue; }
        let s = t >> 1u;
        let d = surf_distance(s, r, u, s == tok_surf);
        if (d < best.d) {
            best = Hit(d, s);
        }
    }
    return best;
}

// The side of `s` a ray leaving cell `c` across it ends on (boundary_token).
// boundary_token (plot/raytrace.rs): the side of `s` opposite cell `c`, for
// a ray at r leaving c along `dir`. A simple region (ANDs only): the side its
// token does not select. Any other: from the normal (upstream's
// distance_complex; complements are not read as written, gh:#587).
fn side_after(c: u32, s: u32, r: vec3<f32>, dir: vec3<f32>) -> bool {
    let ft = full_toks(c);
    if ((ft.f & 1u) == 1u) { return toks_leaving(ft, s); }
    return dot(surf_normal(s, r), dir) > 0.0;
}

fn toks_leaving(tk: Toks, s: u32) -> bool {
    let ts = tk.s;
    let tl = tk.n;
    for (var i: u32 = 0u; i < tl; i = i + 1u) {
        let t = h(ts + i);
        if (t < TOK_INTER && (t >> 1u) == s) {
            return (t & 1u) == 0u;
        }
    }
    return false;
}

// find_cell, trying first the cells whose region names half-space `key`
// (the side of the surface just crossed): flat.rs's crossing hints.
fn find_cell_hint(un: u32, r: vec3<f32>, u: vec3<f32>, key: u32) -> u32 {
    if (key != NONE) {
        let b = h(7u) + un * 4u;
        let hs = h(15u);
        var lo = h(b + 2u);
        var hi = lo + h(b + 3u);
        let end = hi;
        loop {
            if (lo >= hi) { break; }
            let mid = (lo + hi) / 2u;
            if (h(hs + 3u * mid) < key) { lo = mid + 1u; } else { hi = mid; }
        }
        if (lo < end && h(hs + 3u * lo) == key) {
            let start = hs + h(hs + 3u * lo + 1u);
            let n = h(hs + 3u * lo + 2u);
            for (var i: u32 = 0u; i < n; i = i + 1u) {
                let c = h(start + i);
                if (cell_contains(c, r, u)) { return c; }
            }
        }
    }
    return find_cell(un, r, u);
}

fn find_cell(un: u32, r: vec3<f32>, u: vec3<f32>) -> u32 {
    let b = h(7u) + un * 4u;
    let start = h(11u) + h(b);
    let n = h(b + 1u);
    for (var i: u32 = 0u; i < n; i = i + 1u) {
        let c = h(start + i);
        if (cell_contains(c, r, u)) { return c; }
    }
    return NONE;
}

// ── grid index (flat.rs H_GRIDS, index.rs) ──────────────────────────────────

// Absolute offset of universe un's grid record, or NONE.
fn grid_of(un: u32) -> u32 {
    let g = h(16u);
    let o = h(g + un);
    if (o == NONE) { return NONE; }
    return g + o;
}

fn grid_lo(gb: u32) -> vec3<f32> { return vec3<f32>(gf(gb), gf(gb + 1u), gf(gb + 2u)); }
fn grid_d(gb: u32) -> vec3<f32> { return vec3<f32>(gf(gb + 3u), gf(gb + 4u), gf(gb + 5u)); }
fn grid_n(gb: u32) -> vec3<u32> { return vec3<u32>(h(gb + 6u), h(gb + 7u), h(gb + 8u)); }

// The voxel holding r (x fastest), or NONE outside the grid.
fn voxel_at(gb: u32, r: vec3<f32>) -> u32 {
    let f = floor((r - grid_lo(gb)) / grid_d(gb));
    let n = grid_n(gb);
    if (any(f < vec3<f32>(0.0)) || any(f >= vec3<f32>(n))) { return NONE; }
    let i = vec3<u32>(f);
    return i.x + n.x * (i.y + n.y * i.z);
}

// Absolute offset of voxel v's candidate list, or NONE (no cell reaches it).
fn vox_list(gb: u32, v: u32) -> u32 {
    let g = h(16u);
    let o = h(g + h(gb + 10u) + v);
    if (o == NONE) { return NONE; }
    return g + o;
}

fn cand_toks(e: u32) -> Toks {
    return Toks(h(10u) + h(e + 1u), h(e + 2u), h(e + 3u));
}

// Distance from r to where the ray leaves voxel v.
fn vox_exit(gb: u32, v: u32, r: vec3<f32>, u: vec3<f32>) -> f32 {
    let n = grid_n(gb);
    let i = vec3<f32>(f32(v % n.x), f32((v / n.x) % n.y), f32(v / (n.x * n.y)));
    let lo = grid_lo(gb) + i * grid_d(gb);
    let hi = lo + grid_d(gb);
    var d: f32 = INF;
    for (var k: u32 = 0u; k < 3u; k = k + 1u) {
        if (u[k] > 0.0) { d = min(d, (hi[k] - r[k]) / u[k]); }
        if (u[k] < 0.0) { d = min(d, (lo[k] - r[k]) / u[k]); }
    }
    return max(d, 0.0);
}

// Distance along the ray to the grid's box (0 inside), INF if it misses.
fn grid_entry(gb: u32, r: vec3<f32>, u: vec3<f32>) -> f32 {
    let lo = grid_lo(gb);
    let hi = lo + grid_d(gb) * vec3<f32>(grid_n(gb));
    var t0: f32 = 0.0;
    var t1: f32 = INF;
    for (var k: u32 = 0u; k < 3u; k = k + 1u) {
        if (u[k] == 0.0) {
            if (r[k] < lo[k] || r[k] > hi[k]) { return INF; }
        } else {
            let a = (lo[k] - r[k]) / u[k];
            let b = (hi[k] - r[k]) / u[k];
            t0 = max(t0, min(a, b));
            t1 = min(t1, max(a, b));
        }
    }
    if (t0 > t1) { return INF; }
    return t0;
}

// find_cell at level k: among the voxel's candidates when the universe has a
// grid, else the whole universe (crossing hints first). Sets lv_vox[k] and
// lv_toks[k].
fn find_cell_at(k: u32, un: u32, r: vec3<f32>, u: vec3<f32>, hint: u32) -> u32 {
    lv_vox[k] = NONE;
    let gb = grid_of(un);
    if (gb != NONE) {
        let v = voxel_at(gb, r);
        if (v != NONE) {
            lv_vox[k] = v;
            let lb = vox_list(gb, v);
            if (lb == NONE) { return NONE; }
            let n = h(lb);
            for (var i: u32 = 0u; i < n; i = i + 1u) {
                let e = lb + 1u + i * CAND_WORDS;
                let t = cand_toks(e);
                if (toks_contain(t, r, u)) {
                    lv_toks[k] = t;
                    return h(e);
                }
            }
            return NONE;
        }
        // Outside a closed grid no cell holds the point.
        if ((h(gb + 9u) & 1u) == 0u) { return NONE; }
    }
    let c = find_cell_hint(un, r, u, hint);
    if (c != NONE) { lv_toks[k] = full_toks(c); }
    return c;
}

// Bring level k's voxel up to date with the ray's position: the same cell's
// folded tokens in the voxel the point is now in. False if that voxel does
// not list the cell (the caller re-locates).
fn sync_level(k: u32) -> bool {
    let gb = grid_of(lv_uni[k]);
    if (gb == NONE) { return true; }
    let v = voxel_at(gb, local(k));
    if (v == lv_vox[k]) { return true; }
    lv_vox[k] = v;
    if (v == NONE) {
        lv_toks[k] = full_toks(lv_cell[k]);
        return true;
    }
    let lb = vox_list(gb, v);
    if (lb == NONE) { return false; }
    let n = h(lb);
    for (var i: u32 = 0u; i < n; i = i + 1u) {
        let e = lb + 1u + i * CAND_WORDS;
        if (h(e) == lv_cell[k]) {
            lv_toks[k] = cand_toks(e);
            return true;
        }
    }
    return false;
}

// ── lattices ─────────────────────────────────────────────────────────────────

fn lbase(l: u32) -> u32 { return h(9u) + l * LAT_WORDS; }

fn rect_idx(num: f32, pitch: f32, dir: f32) -> i32 {
    let f = num / pitch;
    let close = round(f);
    if (abs(f - close) < 1.0e-12) {
        if (dir > 0.0) { return i32(close); }
        return i32(close) - 1;
    }
    return i32(floor(f));
}

fn hex_center(b: u32, i: vec3<i32>) -> vec3<f32> {
    let nr = f32(h(b + 2u));
    let p = gf(b + 7u);
    let ix = f32(i.x);
    let iy = f32(i.y);
    var off = vec3<f32>(0.0, 0.0, 0.0);
    if (h(b + 1u) == 0u) {
        off.x = gf(b + 4u) + S3 / 2.0 * (ix - nr + 1.0) * p;
        off.y = gf(b + 5u) + (iy - nr + 1.0) * p + (ix - nr + 1.0) * p / 2.0;
    } else {
        off.x = gf(b + 4u) + (ix - nr + 1.0) * p + (iy - nr + 1.0) * p / 2.0;
        off.y = gf(b + 5u) + S3 / 2.0 * (iy - nr + 1.0) * p;
    }
    if (h(b + 3u) > 1u) {
        off.z = gf(b + 6u) - (0.5 * f32(h(b + 3u)) - f32(i.z) - 0.5) * gf(b + 8u);
    }
    return off;
}

// Lattice::tile_center (0 on axes get_local_position leaves alone).
fn lat_center(l: u32, i: vec3<i32>) -> vec3<f32> {
    let b = lbase(l);
    if (h(b) == 0u) {
        var c = vec3<f32>(
            gf(b + 4u) + (f32(i.x) + 0.5) * gf(b + 7u),
            gf(b + 5u) + (f32(i.y) + 0.5) * gf(b + 8u),
            0.0);
        if (h(b + 3u) > 1u) { c.z = gf(b + 6u) + (f32(i.z) + 0.5) * gf(b + 9u); }
        return c;
    }
    return hex_center(b, i);
}

// Lattice::get_indices.
fn lat_indices(l: u32, r: vec3<f32>, u: vec3<f32>) -> vec3<i32> {
    let b = lbase(l);
    if (h(b) == 0u) {
        var iz: i32 = 0;
        if (h(b + 3u) > 1u) { iz = rect_idx(r.z - gf(b + 6u), gf(b + 9u), u.z); }
        return vec3<i32>(
            rect_idx(r.x - gf(b + 4u), gf(b + 7u), u.x),
            rect_idx(r.y - gf(b + 5u), gf(b + 8u), u.y),
            iz);
    }
    let p = gf(b + 7u);
    let n_rings = i32(h(b + 2u));
    let n_axial = h(b + 3u);
    var ro = vec3<f32>(r.x - gf(b + 4u), r.y - gf(b + 5u), r.z);
    var iz: i32 = 0;
    if (n_axial > 1u) {
        ro.z = ro.z - gf(b + 6u);
        let izf = ro.z / gf(b + 8u) + 0.5 * f32(n_axial);
        let izc = round(izf);
        if (abs(izf - izc) < 1.0e-12) {
            iz = select(i32(izc) - 1, i32(izc), u.z > 0.0);
        } else {
            iz = i32(floor(izf));
        }
    }
    var i0: i32;
    var i1: i32;
    if (h(b + 1u) == 0u) {
        let alpha = ro.y - ro.x / S3;
        i0 = i32(floor(ro.x / (0.5 * S3 * p)));
        i1 = i32(floor(alpha / p));
    } else {
        let alpha = ro.y - ro.x * S3;
        i0 = i32(floor(-alpha / (S3 * p)));
        i1 = i32(floor(ro.y / (0.5 * S3 * p)));
    }
    i0 = i0 + n_rings - 1;
    i1 = i1 + n_rings - 1;
    var c0: i32 = 0;
    var c1: i32 = 0;
    var d_min: f32 = INF;
    var dp_min: f32 = INF;
    for (var i: i32 = 0; i < 2; i = i + 1) {
        for (var j: i32 = 0; j < 2; j = j + 1) {
            let cand = vec3<i32>(i0 + j, i1 + i, iz);
            let rt = r - hex_center(b, cand);
            let d = rt.x * rt.x + rt.y * rt.y;
            let on_boundary = abs(1.0 - d_min / d) < 1.0e-12;
            if (d < d_min || on_boundary) {
                let inv = sqrt(d);
                let dp = u.x * (rt.x / inv) + u.y * (rt.y / inv);
                if (on_boundary && dp > dp_min) { continue; }
                d_min = d;
                c0 = j;
                c1 = i;
                dp_min = dp;
            }
        }
    }
    return vec3<i32>(i0 + c0, i1 + c1, iz);
}

// Lattice::universe_at.
fn lat_universe(l: u32, i: vec3<i32>) -> u32 {
    let b = lbase(l);
    let outer = h(b + 10u);
    let map = h(12u) + h(b + 11u);
    if (h(b) == 0u) {
        let n = vec3<i32>(i32(h(b + 1u)), i32(h(b + 2u)), i32(h(b + 3u)));
        if (any(i < vec3<i32>(0)) || any(i >= n)) { return outer; }
        return h(map + u32(n.x * n.y * i.z + n.x * i.y + i.x));
    }
    let nr = i32(h(b + 2u));
    let valid = i.x >= 0 && i.y >= 0 && i.z >= 0
        && i.x < 2 * nr - 1 && i.y < 2 * nr - 1
        && i.x + i.y > nr - 2 && i.x + i.y < 3 * nr - 2
        && i.z < i32(h(b + 3u));
    if (!valid) { return outer; }
    let ns = 2 * nr - 1;
    let v = h(map + u32(ns * ns * i.z + ns * i.y + i.x));
    if (v == NONE) { return outer; }
    return v;
}

// Lattice::distance, r in the tile's local frame (the hexagonal form is the
// neighbour-relative upstream one rewritten in the tile frame: the same
// faces, see hex.rs).
fn lat_distance(l: u32, r: vec3<f32>, u: vec3<f32>) -> f32 {
    let b = lbase(l);
    var d: f32 = INF;
    if (h(b) == 0u) {
        let x0 = select(-0.5, 0.5, u.x >= 0.0) * gf(b + 7u);
        let y0 = select(-0.5, 0.5, u.y >= 0.0) * gf(b + 8u);
        if (u.x != 0.0) { d = min(d, (x0 - r.x) / u.x); }
        if (u.y != 0.0) { d = min(d, (y0 - r.y) / u.y); }
        if (h(b + 3u) > 1u) {
            let z0 = select(-0.5, 0.5, u.z >= 0.0) * gf(b + 9u);
            if (u.z != 0.0) { d = min(d, (z0 - r.z) / u.z); }
        }
        return d;
    }
    let half = 0.5 * gf(b + 7u);
    var n1: vec2<f32>;
    var n2: vec2<f32>;
    var n3: vec2<f32>;
    if (h(b + 1u) == 0u) {
        n1 = vec2<f32>(S3 / 2.0, 0.5);
        n2 = vec2<f32>(S3 / 2.0, -0.5);
        n3 = vec2<f32>(0.0, 1.0);
    } else {
        n1 = vec2<f32>(1.0, 0.0);
        n2 = vec2<f32>(0.5, -S3 / 2.0);
        n3 = vec2<f32>(0.5, S3 / 2.0);
    }
    let rr = r.xy;
    let uu = u.xy;
    for (var k: u32 = 0u; k < 3u; k = k + 1u) {
        var n = n1;
        if (k == 1u) { n = n2; }
        if (k == 2u) { n = n3; }
        let dd = dot(n, uu);
        if (dd == 0.0) { continue; }
        let edge = select(-half, half, dd > 0.0);
        let beta = dot(n, rr);
        if (abs(beta - edge) > 1.0e-14) {
            d = min(d, (edge - beta) / dd);
        }
    }
    if (h(b + 3u) > 1u && u.z != 0.0) {
        let z0 = select(-0.5, 0.5, u.z >= 0.0) * gf(b + 8u);
        if (abs(r.z - z0) > 1.0e-14) {
            d = min(d, (z0 - r.z) / u.z);
        }
    }
    return d;
}

// ── locate (Geometry::locate, from level `start`) ────────────────────────────

fn locate_from(start: u32, hint: u32) -> bool {
    var k = start;
    loop {
        if (k >= MAX_DEPTH) { return false; }
        let r = local(k);
        var hk = NONE;
        if (k == start) { hk = hint; }
        let c = find_cell_at(k, lv_uni[k], r, ray_u, hk);
        if (c == NONE) { return false; }
        lv_cell[k] = c;
        let b = cbase(c);
        let kind = h(b + 2u);
        let fill = h(b + 3u);
        if (kind == 0u) { cur_mat = i32(fill); depth = k + 1u; return true; }
        if (kind == 3u) { cur_mat = -1; depth = k + 1u; return true; }
        // A fill whose whole subtree is hidden is crossed as one cell (the
        // CPU walks it and reports only crossings of hidden materials,
        // which change nothing in a solid or x-ray picture).
        if (P.mode != 2u && !subtree_shown(select(h(6u) + fill, fill, kind == 1u))) {
            cur_mat = ID_HIDDEN;
            depth = k + 1u;
            return true;
        }
        if (k + 1u >= MAX_DEPTH) { return false; }
        let tr = vec3<f32>(gf(b + 4u), gf(b + 5u), gf(b + 6u));
        if (kind == 1u) {
            lv_uni[k + 1u] = fill;
            lv_lat[k + 1u] = NONE;
            lv_off[k + 1u] = lv_off[k] + tr;
        } else {
            let cr = r - tr;
            let idx = lat_indices(fill, cr, ray_u);
            let un = lat_universe(fill, idx);
            if (un == NONE) { return false; }
            lv_uni[k + 1u] = un;
            lv_lat[k + 1u] = fill;
            lv_idx[k + 1u] = idx;
            lv_off[k + 1u] = lv_off[k] + tr + lat_center(fill, idx);
        }
        k = k + 1u;
    }
    return false;
}

fn locate_root() -> bool {
    lv_uni[0] = h(1u);
    lv_lat[0] = NONE;
    lv_off[0] = vec3<f32>(0.0, 0.0, 0.0);
    return locate_from(0u, NONE);
}

// After a crossing at level `lvl`: keep every ancestor that still holds the
// point (its cell, and the lattice tile below it), re-locate from the first
// that does not.
fn relocate(lvl: u32, hint: u32) -> bool {
    for (var k: u32 = 0u; k < lvl; k = k + 1u) {
        let r = local(k);
        if (!sync_level(k) || !toks_contain(lv_toks[k], r, ray_u)) {
            return locate_from(k, NONE);
        }
        let l = lv_lat[k + 1u];
        if (l != NONE) {
            let b = cbase(lv_cell[k]);
            let tr = vec3<f32>(gf(b + 4u), gf(b + 5u), gf(b + 6u));
            let idx = lat_indices(l, r - tr, ray_u);
            if (any(idx != lv_idx[k + 1u])) {
                let un = lat_universe(l, idx);
                if (un == NONE) { return false; }
                lv_uni[k + 1u] = un;
                lv_idx[k + 1u] = idx;
                lv_off[k + 1u] = lv_off[k] + tr + lat_center(l, idx);
                return locate_from(k + 1u, NONE);
            }
        }
    }
    return locate_from(lvl, hint);
}

struct Boundary {
    d: f32,
    // NONE: lattice crossing or nothing
    s: u32,
    level: u32,
    found: bool,
    // distance to the nearest voxel face of any level (INF: none)
    vox: f32,
};

// Geometry::distance_to_boundary, plus the nearest voxel face.
fn distance_to_boundary() -> Boundary {
    var best = Boundary(INF, NONE, 0u, false, INF);
    for (var k: u32 = 0u; k < depth; k = k + 1u) {
        if (!sync_level(k)) {
            // The voxel the point moved into does not list its cell (a
            // boundary within rounding of the voxel face): re-locate.
            if (!locate_from(k, NONE)) { return best; }
        }
        let r = local(k);
        let hs = toks_distance(lv_toks[k], r, ray_u);
        if (hs.d < best.d) {
            best = Boundary(hs.d, hs.s, k, hs.s != NONE, best.vox);
        }
        if (lv_vox[k] != NONE) {
            best.vox = min(best.vox, vox_exit(grid_of(lv_uni[k]), lv_vox[k], r, ray_u));
        }
        let l = lv_lat[k];
        if (l != NONE) {
            let dl = lat_distance(l, r, ray_u);
            // The bounding surface wins a tie with the lattice edge.
            let tie = best.s != NONE && abs(best.d - dl) < nudge();
            if (dl < best.d && !tie) {
                best = Boundary(dl, NONE, k, true, best.vox);
            }
        }
    }
    return best;
}

// ── colours ──────────────────────────────────────────────────────────────────

fn unpack(c: u32) -> vec3<f32> {
    return vec3<f32>(f32(c & 255u), f32((c >> 8u) & 255u), f32((c >> 16u) & 255u));
}

fn pack(c: vec3<f32>) -> u32 {
    let v = vec3<u32>(clamp(c, vec3<f32>(0.0), vec3<f32>(255.0)));
    return v.x | (v.y << 8u) | (v.z << 16u) | 0xFF000000u;
}

// Rgb::scaled: each channel times x, truncated.
fn scaled(c: u32, x: f32) -> u32 {
    return pack(floor(unpack(c) * x));
}

fn mat_colour(m: i32) -> u32 { return mats[u32(m)].x; }
// Universe i (or lattice i - n_universes) holds a drawn material.
fn subtree_shown(i: u32) -> bool {
    let w = P.n_mat * 4u + i;
    return mats[w / 4u][w % 4u] != 0u;
}
fn shown(m: i32) -> bool {
    return m >= 0 && u32(m) < P.n_mat && (mats[u32(m)].y & 1u) == 1u;
}
fn outlined(m: i32) -> bool {
    return m >= 0 && u32(m) < P.n_mat && (mats[u32(m)].y & 2u) == 2u;
}

// ── per-pixel ray state for the solid and x-ray shaders ──────────────────────

var<private> reflected: bool;
var<private> orig: i32;
var<private> result: u32;
var<private> out_id: i32;
// x-ray
var<private> nseg: u32;
var<private> prev_id: i32;
var<private> prev_len: f32;
var<private> prev_surf: u32;
var<private> acc: vec3<f32>;
var<private> trans: f32;
var<private> hash: u32;
// The point is already located (the section plane's cut-face lookup).
var<private> prelocated: bool;
// Boundary crossings this pixel took (aux[2p + 1] of a solid picture).
var<private> steps: u32;
// Silent voxel steps this pixel took.
var<private> vsteps: u32;

fn mix_hash(x: u32) {
    hash = (hash ^ x) * 16777619u;
}

// Actions: 0 continue, 1 stop, 2 re-aim at the light.
var<private> new_u: vec3<f32>;
var<private> new_tok_surf: u32;
var<private> new_tok_out: bool;

// PhongRay::on_intersection, plus the section rule: a shadow ray that passes
// into the cut-away half is lit (the half is not there).
fn on_solid(hit_mat: i32, s: u32, side_out: bool, level: u32, inside: bool) -> u32 {
    let r = pos();
    let cam = P.cam.xyz;
    if (reflected) {
        if (dot(r - cam, ray_u) >= 0.0) { return 1u; }
        if ((P.flags & 1u) == 1u && dot(P.clip.xyz, r) < P.clip.w) { return 1u; }
    }
    if (!shown(hit_mat)) { return 0u; }
    if (reflected) {
        result = scaled(mat_colour(orig), P.light.w);
        return 1u;
    }
    reflected = true;
    result = mat_colour(hit_mat);
    out_id = hit_mat;
    orig = hit_mat;
    if (s == NONE) {
        result = P.colours.y;
        out_id = ID_OVERLAP;
        return 1u;
    }
    if (!inside) { return 1u; }
    let lvl = min(level, depth - 1u);
    var n = normalize(surf_normal(s, local(lvl)));
    if (dot(n, ray_u) > 0.0) { n = -n; }
    let to_light = normalize(P.light.xyz - r);
    let df = P.light.w;
    let modulation = df + (1.0 - df) * max(dot(n, to_light), 0.0);
    result = scaled(result, modulation);
    new_u = to_light;
    new_tok_surf = s;
    new_tok_out = !side_out;
    return 2u;
}

// ProjectionRay::on_intersection, composited front to back as it streams.
fn on_xray(hit_mat: i32, s: u32, trav: f32) {
    if (nseg > 0u && prev_id >= 0 && u32(prev_id) < P.n_mat) {
        let xs = bitcast<f32>(mats[u32(prev_id)].z);
        let m = exp(-xs * (trav - prev_len));
        acc = acc + trans * (1.0 - m) * unpack(mat_colour(prev_id));
        trans = trans * m;
    }
    let filtered = (P.flags & 4u) == 4u;
    if (!filtered || outlined(hit_mat)) {
        mix_hash(bitcast<u32>(hit_mat));
        mix_hash(s);
        if (filtered) { mix_hash(prev_surf); }
    }
    prev_surf = s;
    prev_id = hit_mat;
    prev_len = trav;
    nseg = nseg + 1u;
}

fn on_hit(hit_mat: i32, s: u32, side_out: bool, level: u32, inside: bool, trav: f32) -> u32 {
    if (P.mode == 1u) {
        on_xray(hit_mat, s, trav);
        return 0u;
    }
    return on_solid(hit_mat, s, side_out, level, inside);
}

// The CPU's trace(): reach the model, then cross boundary after boundary.
fn trace() {
    tok_surf = NONE;
    steps = 0u;
    var have = prelocated;
    if (!have) {
        have = locate_root();
    }
    var entry_s: u32 = NONE;
    var entry_out = false;
    vsteps = 0u;
    loop {
        if (have) { break; }
        // Advance to the nearest root-cell boundary from outside the model.
        var best: f32 = INF;
        var bs: u32 = NONE;
        var bc: u32 = NONE;
        // Distance of a silent step (a voxel face, or the grid's box).
        var silent: f32 = INF;
        let r = pos();
        let gb = grid_of(h(1u));
        if (gb != NONE && lv_vox[0] != NONE) {
            // In the grid, in no cell: the voxel's candidates only.
            let lb = vox_list(gb, lv_vox[0]);
            if (lb != NONE) {
                let n = h(lb);
                for (var i: u32 = 0u; i < n; i = i + 1u) {
                    let e = lb + 1u + i * CAND_WORDS;
                    let hs = toks_distance(cand_toks(e), r, ray_u);
                    if (hs.d < best) {
                        best = hs.d;
                        bs = hs.s;
                        bc = h(e);
                    }
                }
            }
            silent = vox_exit(gb, lv_vox[0], r, ray_u);
        } else if (gb != NONE && (h(gb + 9u) & 1u) == 0u) {
            // Outside a closed grid: nothing until its box.
            silent = grid_entry(gb, r, ray_u);
            if (silent > 1.0e29) { return; }
            silent = max(silent, nudge());
        } else {
            let ub = h(7u) + h(1u) * 4u;
            let start = h(11u) + h(ub);
            let n = h(ub + 1u);
            for (var i: u32 = 0u; i < n; i = i + 1u) {
                let c = h(start + i);
                let hs = cell_distance(c, r, ray_u);
                if (hs.d < best) {
                    best = hs.d;
                    bs = hs.s;
                    bc = c;
                }
            }
        }
        lv_uni[0] = h(1u);
        lv_lat[0] = NONE;
        lv_off[0] = vec3<f32>(0.0, 0.0, 0.0);
        if (silent < INF && (bs == NONE || best > silent + 2.0 * nudge())) {
            ray_t = ray_t + silent + nudge();
            have = locate_from(0u, NONE);
            vsteps = vsteps + 1u;
            if (vsteps > MAX_VOXEL_STEPS) { return; }
            continue;
        }
        if (best > 1.0e29 || bs == NONE) { return; }
        ray_t = ray_t + best + nudge();
        entry_s = bs;
        // The side opposite the cell entered (a ray leaving it backwards).
        entry_out = side_after(bc, bs, pos(), -ray_u);
        have = locate_from(0u, (bs << 1u) | select(1u, 0u, entry_out));
        steps = steps + 1u;
        if (steps > 64u) { return; }
    }
    var trav: f32 = 0.0;
    // Distance since the last crossing, voxel steps included: the CPU's
    // "closer than 10 nudges" rule is about the crossing just taken, not
    // about the last silent step.
    var since: f32 = 0.0;
    if (entry_s != NONE) {
        let a = on_hit(cur_mat, entry_s, entry_out, 0u, true, trav);
        if (a == 1u) { return; }
        if (a == 2u) {
            ray_o = pos();
            ray_t = 0.0;
            ray_u = new_u;
            tok_surf = new_tok_surf;
            tok_out = new_tok_out;
            if (!relocate(0u, (new_tok_surf << 1u) | select(0u, 1u, new_tok_out))) { return; }
        }
    }
    tok_surf = NONE;
    loop {
        let bh = distance_to_boundary();
        // A voxel face nearer than every boundary: a silent step (nothing
        // crossed, nothing reported; the next distance_to_boundary moves the
        // level to its new voxel). A boundary within two nudges of the face
        // is taken as the crossing, so no surface is stepped over.
        if (bh.vox < 1.0e29 && (!bh.found || bh.d > bh.vox + 2.0 * nudge())) {
            let dv = bh.vox + nudge();
            ray_t = ray_t + dv;
            trav = trav + dv;
            since = since + dv;
            // Off the surface just crossed: its token (which makes a surface
            // distance "coincident", measured as from a point on it) no
            // longer applies. Within 10 nudges it still does, as on the CPU.
            if (since > 10.0 * nudge()) { tok_surf = NONE; }
            vsteps = vsteps + 1u;
            if (vsteps > MAX_VOXEL_STEPS) { return; }
            continue;
        }
        if (!bh.found || bh.d >= 1.0e29 || bh.d < 0.0) { return; }
        // The CPU skips a crossing closer than 10 nudges (10 * TINY_BIT).
        let call = since + bh.d >= 10.0 * nudge();
        since = 0.0;
        let d = bh.d + nudge();
        ray_t = ray_t + d;
        trav = trav + d;
        let old_mat = cur_mat;
        var side_out = false;
        if (bh.s != NONE) {
            side_out = side_after(lv_cell[bh.level], bh.s, local(bh.level), ray_u);
            tok_surf = bh.s;
            tok_out = side_out;
        } else {
            tok_surf = NONE;
        }
        var hint = NONE;
        if (bh.s != NONE) { hint = (bh.s << 1u) | select(0u, 1u, side_out); }
        let inside = relocate(bh.level, hint);
        var hit_mat = old_mat;
        if (inside) { hit_mat = cur_mat; }
        if (call) {
            let a = on_hit(hit_mat, bh.s, side_out, bh.level, inside, trav);
            if (a == 1u) { return; }
            if (a == 2u) {
                ray_o = pos();
                ray_t = 0.0;
                ray_u = new_u;
                tok_surf = new_tok_surf;
                tok_out = new_tok_out;
                let key = (new_tok_surf << 1u) | select(0u, 1u, new_tok_out);
                if (!relocate(bh.level, key)) { return; }
            }
        }
        if (!inside) { return; }
        steps = steps + 1u;
        if (steps > P.max_steps) { return; }
    }
}

// Camera::pixel_ray.
fn pixel_ray(x: u32, y: u32) {
    let p0 = f32(P.width);
    let p1 = f32(P.height);
    if ((P.flags & 2u) == 0u) {
        let dx = P.m0.w;
        let dy = P.m1.w;
        let v = normalize(vec3<f32>(FOCAL_PLANE_DIST, -0.5 * dx + f32(x) * dx / p0, 0.5 * dy - f32(y) * dy / p1));
        ray_o = P.cam.xyz;
        ray_u = normalize(vec3<f32>(dot(v, P.m0.xyz), dot(v, P.m1.xyz), dot(v, P.m2.xyz)));
    } else {
        let xp = (f32(x) - p0 / 2.0) / p0;
        let yp = (f32(y) - p1 / 2.0) / p1;
        let cy = vec3<f32>(P.m0.y, P.m1.y, P.m2.y);
        let cz = vec3<f32>(P.m0.z, P.m1.z, P.m2.z);
        ray_o = P.cam.xyz + cy * xp * P.cam.w + cz * yp * P.cam.w;
        ray_u = normalize(vec3<f32>(P.m0.x, P.m1.x, P.m2.x));
    }
    ray_t = 0.0;
}

fn put_px(x: u32, y: u32, colour: u32, a0: u32, a1: u32) {
    out_colour[y * P.stride + x] = colour;
    let p = (y * P.width + x) * 2u;
    aux[p] = a0;
    aux[p + 1u] = a1;
}

@compute @workgroup_size(8, 8)
fn render(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let y = gid.y + P.row0;
    if (x >= P.width || y >= P.height) { return; }
    tok_surf = NONE;
    tok_out = false;
    depth = 0u;
    cur_mat = -1;

    if (P.mode == 2u) {
        // A slice: one locate at the pixel centre (SlicePlot::id_map).
        ray_o = P.s_start.xyz - P.s_v.xyz * f32(y) + P.s_u.xyz * f32(x);
        ray_u = vec3<f32>(0.70710677, 0.70710677, 0.0);
        ray_t = 0.0;
        if (!locate_root()) {
            put_px(x, y, P.colours.x, bitcast<u32>(ID_BACKGROUND), 0u);
        } else if (cur_mat < 0) {
            put_px(x, y, P.colours.w, bitcast<u32>(ID_VOID), 0u);
        } else {
            put_px(x, y, mat_colour(cur_mat), bitcast<u32>(cur_mat), 0u);
        }
        return;
    }

    pixel_ray(x, y);
    reflected = false;
    orig = -1;
    result = P.colours.x;
    out_id = ID_BACKGROUND;
    nseg = 0u;
    prev_id = -1;
    prev_len = 0.0;
    prev_surf = NONE;
    acc = vec3<f32>(0.0);
    trans = 1.0;
    hash = 2166136261u;
    prelocated = false;

    if (P.mode == 0u && (P.flags & 1u) == 1u) {
        // ClipPlane::start and SolidRayTracePlot::cut_face.
        let n = P.clip.xyz;
        let nr = dot(n, ray_o);
        if (nr < P.clip.w) {
            let nu = dot(n, ray_u);
            if (nu <= 0.0) {
                put_px(x, y, P.colours.x, bitcast<u32>(ID_BACKGROUND), 0u);
                return;
            }
            ray_t = (P.clip.w - nr) / nu + nudge();
            let found = locate_root();
            if (found && shown(cur_mat)) {
                var cn = normalize(n);
                if (dot(cn, ray_u) > 0.0) { cn = -cn; }
                let tl = normalize(P.light.xyz - pos());
                let df = P.light.w;
                let c = scaled(mat_colour(cur_mat), df + (1.0 - df) * max(dot(cn, tl), 0.0));
                put_px(x, y, c, bitcast<u32>(cur_mat), 0u);
                return;
            }
            // Trace on from the point just located.
            prelocated = found;
        }
    }

    trace();

    if (P.mode == 1u) {
        var c = P.colours.x;
        if (nseg > 1u) {
            c = pack(floor(acc + trans * unpack(P.colours.x)));
        }
        var hv = hash;
        if (nseg == 0u) { hv = 0u; }
        put_px(x, y, c, hv, nseg);
    } else {
        put_px(x, y, result, bitcast<u32>(out_id), min(steps, 65535u) | (min(vsteps, 65535u) << 16u));
    }
}

// WireframeRayTracePlot::create_image's outline pass (thickness 1): a pixel
// whose track stack differs from its left or upper neighbour's is a line.
@compute @workgroup_size(8, 8)
fn wire(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let y = gid.y;
    if (x >= P.width || y >= P.height) { return; }
    let p = (y * P.width + x) * 2u;
    let hv = aux[p];
    let n = aux[p + 1u];
    let filtered = (P.flags & 4u) == 4u;
    var edge = false;
    // Left neighbour: only for a pixel with more than one segment.
    if (x > 0u && n > 1u) {
        let q = (y * P.width + x - 1u) * 2u;
        edge = edge || differs(hv, aux[q], filtered);
    }
    // Upper neighbour; row 0 is compared with an empty stack.
    var above: u32 = 0u;
    if (y > 0u) { above = aux[((y - 1u) * P.width + x) * 2u]; }
    edge = edge || differs(hv, above, filtered);
    if (edge) {
        out_colour[y * P.stride + x] = P.colours.z;
    }
}

// trackstack_equivalent on hashes: an empty stack (hash 0) is equivalent to
// anything when only some ids are outlined, as upstream's loop never runs.
fn differs(a: u32, b: u32, filtered: bool) -> bool {
    if (filtered && (a == 0u || b == 0u)) { return false; }
    return a != b;
}
