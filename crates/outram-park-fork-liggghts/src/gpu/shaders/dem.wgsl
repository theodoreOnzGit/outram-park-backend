// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// The DEM timestep of `GranularSystem` on the GPU (gh:#1250): Hertz normal
// force with the history tangential model, velocity-Verlet, the analytic
// cylinder and one-sided wall, and a static triangle-mesh wall. A
// translation of this crate's own CPU path (`granular.rs`,
// `granular_system.rs`, `boundary.rs`, `mesh_wall.rs`, `integrator.rs`),
// which ports LIGGGHTS-PUBLIC (`normal_model_hertz.h`,
// `tangential_model_history.h`, `fix_nve_sphere.cpp`). See `dem.rs` for the
// layout, the precision contract and why the result is not bit-identical.
//
// One thread per particle. Each particle GATHERS the force from its own
// contacts (no atomics in the sum): the pair force is antisymmetric (e_n,
// v_tr and the shear spring flip sign from the other side), so each side
// computes its own half exactly, and keeps its own copy of the spring.

struct Params {
    n: u32,
    k_slots: u32,
    cap: u32,
    n_tris: u32,
    dims: vec4<u32>,          // grid cells x, y, z, total
    origin: vec4<f32>,        // grid origin xyz, cell edge in w
    gravity: vec4<f32>,       // xyz, dt in w
    material: vec4<f32>,      // y_eff, g_eff, beta, mu
    cyl: vec4<f32>,           // axis point x, y; radius hi, lo (axis is z)
    valve: vec4<f32>,         // wall point z hi, normal z sign, wall point z lo (one-sided)
    flags_in: vec4<u32>,      // has cylinder, has valve wall, has mesh, tangential damping
    hull: vec4<f32>,          // mesh bounding sphere centre xyz, radius
    one: vec4<f32>,           // (1, 1, 1, 1), known only at run time: see `keep`
};

// A particle: position as hi + lo f32 pairs (WebGPU has no f64), radius,
// mass, velocity, angular velocity.
struct P {
    hi: vec4<f32>,  // xyz, radius (hi)
    lo: vec4<f32>,  // xyz, mass
    v: vec4<f32>,   // xyz, radius (lo)
    w: vec4<f32>,   // xyz, unused
};

@group(0) @binding(0) var<uniform> prm: Params;
@group(0) @binding(1) var<storage, read_write> ps: array<P>;
// force (2i) and torque (2i + 1).
@group(0) @binding(2) var<storage, read_write> ft: array<vec4<f32>>;
// Cell counts [0, total), then `cap` items per cell.
@group(0) @binding(3) var<storage, read_write> cells: array<atomic<u32>>;
// Shear slots: per particle `k_slots` pair slots (xyz spring, w = the
// neighbour index as an f32 value, −1 empty), then 2 wall slots (cylinder,
// one-sided wall; w = 1 live), then `M_SLOTS` mesh-facet slots (w = the facet
// index as an f32 value, −1 empty), in upstream's contact-list order.
@group(0) @binding(4) var<storage, read_write> h_old: array<vec4<f32>>;
@group(0) @binding(5) var<storage, read_write> h_new: array<vec4<f32>>;
// Mesh triangles, `TRI` vec4s each (`GpuSetup::from_system`):
//   0: a (hi), barycentric threshold −precision/(2 rBound)
//   1: a (lo), obtuse-angle index (−1 none)
//   2: b − a, edgeLen[0]      3: c − a, edgeLen[1]
//   4: surfaceNorm, edgeLen[2]
//   5: edgeVec[0], edge/corner activity bits (edge k: bit k, corner k: bit 3 + k)
//   6: edgeVec[1]             7: edgeVec[2]
//   8: bounds (centroid, radius)
//   9, 10: up to 8 coplanar node-neighbour facets (f32 values, −1 empty)
//   11: surfaceNorm's lo part (normal = row 4 + row 11, for the face distance)
//   12, 13: edgeVec[0] and edgeVec[2] lo parts
//   14, 15: barycentric coefficients as hi, lo pairs (`bary_code`)
// The contact is resolved in coordinates local to a (node 0), so magnitudes
// are a facet's size, not the vessel's.
@group(0) @binding(6) var<storage, read_write> tris: array<vec4<f32>>;
// [0] cell overflow, [1] contact-slot overflow.
@group(0) @binding(7) var<storage, read_write> errs: array<atomic<u32>>;

// An empty pair slot. The neighbour index is held as an f32 VALUE (exact
// below 2^24), not a bit pattern: `bitcast<f32>(0xffffffff)` is a NaN, which
// Chrome's compiler refuses in a constant (every pipeline was invalid and
// the browser dropped every dispatch, 2026-11-05), and small indices as bit
// patterns are denormals some GPUs flush to zero.
const EMPTY: f32 = -1.0;
const SQRT_FIVE_OVER_SIX: f32 = 0.9128709291752769;
const INERTIA: f32 = 0.4;
// Mesh-facet contact slots per particle (`gpu::dem::M_SLOTS`).
const M_SLOTS: u32 = 8u;
// vec4s per triangle (`gpu::dem::TRI_VEC4S`).
const TRI: u32 = 16u;
// Upstream `SMALL_TRIMESH` and `LARGE_TRIMESH` (`tri_mesh.rs`).
const SMALL_TRIMESH: f32 = 1.0e-10;
const LARGE_TRIMESH: f32 = 1000000.0;

fn pos(i: u32) -> vec3<f32> {
    return ps[i].hi.xyz + ps[i].lo.xyz;
}

fn cell_of(p: vec3<f32>) -> vec3<i32> {
    let c = vec3<i32>(floor((p - prm.origin.xyz) / prm.origin.w));
    return clamp(c, vec3<i32>(0), vec3<i32>(prm.dims.xyz) - vec3<i32>(1));
}

fn cell_index(c: vec3<i32>) -> u32 {
    return u32(c.x) + prm.dims.x * (u32(c.y) + prm.dims.y * u32(c.z));
}

// ── integrator (`integrator.rs`, fix_nve_sphere) ───────────────────────────

// **Why `keep`.** The error-free transformations below (two-sum, the
// double-f32 overlap) rely on each rounded intermediate being used as
// rounded. A shader compiler that reassociates floats (Mesa's lavapipe did,
// 2026-11-05: the hi + lo positions came back with lo = 0) cancels them
// algebraically: `lo − ((t + lo) − t)` becomes 0. Multiplying an
// intermediate by `prm.one`, a 1 the compiler cannot see, makes it opaque,
// so it is computed as written. Exact: x · 1 = x. EVERY partial sum whose
// order matters is kept, not only the identities: `(a − X) + (b − bb)`
// regrouped as `(a + b) − …` loses a tiny `b` in `a + b` (it lost the
// cylinder radius's lo part, 2.4e-8 m, until every partial was kept).
fn keep(x: f32) -> f32 {
    return x * prm.one.x;
}

fn keep3(x: vec3<f32>) -> vec3<f32> {
    return x * prm.one.xyz;
}

// Two-sum: a + b = s + e exactly.
fn two_sum(a: vec3<f32>, b: vec3<f32>) -> array<vec3<f32>, 2> {
    let s = keep3(a + b);
    let bb = keep3(s - a);
    let e = keep3(a - keep3(s - bb)) + keep3(b - bb);
    return array<vec3<f32>, 2>(s, e);
}

@compute @workgroup_size(64)
fn kick_drift(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= prm.n) { return; }
    let dt = prm.gravity.w;
    let half = 0.5 * dt;
    var p = ps[i];
    let m = p.lo.w;
    let r = p.hi.w;
    let inv_i = 1.0 / (INERTIA * m * r * r);
    let v = p.v.xyz + ft[2u * i].xyz * (half / m);
    let w = p.w.xyz + ft[2u * i + 1u].xyz * (half * inv_i);
    // x += dt v, carried exactly in hi + lo.
    let t = two_sum(p.hi.xyz, v * dt);
    let lo = keep3(p.lo.xyz + t[1]);
    let h2 = keep3(t[0] + lo);
    let l2 = lo - keep3(h2 - t[0]);
    p.hi = vec4<f32>(h2, r);
    p.lo = vec4<f32>(l2, m);
    p.v = vec4<f32>(v, p.v.w);
    p.w = vec4<f32>(w, 0.0);
    ps[i] = p;
}

@compute @workgroup_size(64)
fn kick(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= prm.n) { return; }
    let half = 0.5 * prm.gravity.w;
    var p = ps[i];
    let m = p.lo.w;
    let r = p.hi.w;
    let inv_i = 1.0 / (INERTIA * m * r * r);
    p.v = vec4<f32>(p.v.xyz + ft[2u * i].xyz * (half / m), p.v.w);
    p.w = vec4<f32>(p.w.xyz + ft[2u * i + 1u].xyz * (half * inv_i), 0.0);
    ps[i] = p;
}

// ── double-f32 arithmetic for the overlap ───────────────────────────────────
//
// δ = (r_i + r_j) − |x_i − x_j| cancels two numbers near 2r: in plain f32
// it keeps only ~4e-9 m (1e-5 of a 0.4 mm collision overlap, 4e-4 of a
// 10 µm resting one). Carried as hi + lo pairs (Dekker / Knuth error-free
// transformations; Dekker's split gives the exact product error), δ keeps ~1e-7 of
// itself, which is f32's own resolution of the result.

fn ts(a: f32, b: f32) -> vec2<f32> {
    let s = keep(a + b);
    let bb = keep(s - a);
    return vec2<f32>(s, keep(a - keep(s - bb)) + keep(b - bb));
}

// (a.x + a.y) + (b.x + b.y), renormalised.
fn dd_add(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    let s = ts(a.x, b.x);
    let lo = keep(keep(s.y + a.y) + b.y);
    let h = keep(s.x + lo);
    return vec2<f32>(h, lo - keep(h - s.x));
}

// √(a.x + a.y), to double-f32 (one Newton step on the f32 root).
fn dd_sqrt(a: vec2<f32>) -> vec2<f32> {
    let h = sqrt(a.x);
    if (h == 0.0) { return vec2<f32>(0.0); }
    let r = dd_add(a, -dd_sq(vec2<f32>(h, 0.0)));
    return ts(h, (r.x + r.y) / (2.0 * h));
}

// The exact error of a·b = p (Dekker's split product, as `sq_err`).
fn mul_err(a: f32, b: f32, p: f32) -> f32 {
    let ca = keep(4097.0 * a);
    let ah = keep(ca - keep(ca - a));
    let al = keep(a - ah);
    let cb = keep(4097.0 * b);
    let bh = keep(cb - keep(cb - b));
    let bl = keep(b - bh);
    return keep(keep(keep(keep(keep(ah * bh) - p) + keep(ah * bl)) + keep(al * bh)) + keep(al * bl));
}

// (a.x + a.y)(b.x + b.y), to double-f32.
fn dd_mul(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    let p = keep(a.x * b.x);
    let e = mul_err(a.x, b.x, p);
    let lo = keep(keep(e + keep(a.x * b.y)) + keep(a.y * b.x));
    let h = keep(p + lo);
    return vec2<f32>(h, lo - keep(h - p));
}

// The exact error of a·a (Dekker's split product, which needs no fused
// multiply-add: `fma` was not exact on every driver, 2026-11-05).
fn sq_err(a: f32, p: f32) -> f32 {
    let c = keep(4097.0 * a);
    let hi = keep(c - keep(c - a));
    let lo = keep(a - hi);
    return keep(keep(keep(keep(hi * hi) - p) + keep(2.0 * hi * lo)) + keep(lo * lo));
}

// (a.x + a.y)², to double-f32.
fn dd_sq(a: vec2<f32>) -> vec2<f32> {
    let p = keep(a.x * a.x);
    let e = sq_err(a.x, p);
    let lo = keep(e + keep(2.0 * a.x * a.y));
    let h = keep(p + lo);
    return vec2<f32>(h, lo - keep(h - p));
}

// ── cell list (deterministic: entries sorted by index) ──────────────────────

@compute @workgroup_size(64)
fn clear_cells(@builtin(global_invocation_id) gid: vec3<u32>) {
    let c = gid.x;
    if (c >= prm.dims.w) { return; }
    atomicStore(&cells[c], 0u);
}

@compute @workgroup_size(64)
fn bin(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= prm.n) { return; }
    let c = cell_index(cell_of(pos(i)));
    let slot = atomicAdd(&cells[c], 1u);
    if (slot < prm.cap) {
        atomicStore(&cells[prm.dims.w + c * prm.cap + slot], i);
    } else {
        atomicAdd(&errs[0], 1u);
    }
}

@compute @workgroup_size(64)
fn sort_cells(@builtin(global_invocation_id) gid: vec3<u32>) {
    let c = gid.x;
    if (c >= prm.dims.w) { return; }
    let n = min(atomicLoad(&cells[c]), prm.cap);
    let base = prm.dims.w + c * prm.cap;
    for (var a = 1u; a < n; a = a + 1u) {
        let key = atomicLoad(&cells[base + a]);
        var b = a;
        loop {
            if (b == 0u) { break; }
            let prev = atomicLoad(&cells[base + b - 1u]);
            if (prev <= key) { break; }
            atomicStore(&cells[base + b], prev);
            b = b - 1u;
        }
        atomicStore(&cells[base + b], key);
    }
}

// ── contact law (`granular.rs`: Hertz + TANGENTIAL_HISTORY) ─────────────────

struct Kin {
    en: vec3<f32>,
    delta_n: f32,
    vn: f32,
    vtr: vec3<f32>,
    cri: f32,
    r_eff: f32,
    m_eff: f32,
};

struct Out {
    force: vec3<f32>,
    torque: vec3<f32>,
    shear: vec3<f32>,
};

fn resolve(prior: vec3<f32>, k: Kin) -> Out {
    let y_eff = prm.material.x;
    let g_eff = prm.material.y;
    let beta = prm.material.z;
    let mu = prm.material.w;
    let sqrtval = sqrt(k.r_eff * k.delta_n);
    let sn = 2.0 * y_eff * sqrtval;
    let st = 8.0 * g_eff * sqrtval;
    let kn = 4.0 / 3.0 * y_eff * sqrtval;
    let kt = st;
    let gamman = -2.0 * SQRT_FIVE_OVER_SIX * beta * sqrt(sn * k.m_eff);
    var gammat = 0.0;
    if (prm.flags_in.w != 0u) {
        gammat = -2.0 * SQRT_FIVE_OVER_SIX * beta * sqrt(st * k.m_eff);
    }
    let fn_scalar = -gamman * k.vn + kn * k.delta_n;

    // 1. accumulate and re-project onto the tangent plane
    var shear = prior + k.vtr * prm.gravity.w;
    shear = shear - k.en * dot(shear, k.en);
    let shrmag = length(shear);
    var f_t = shear * (-kt);
    let ft_shear = kt * shrmag;
    let ft_friction = mu * abs(fn_scalar);
    var stored = shear;
    if (ft_shear > ft_friction) {
        // 3. sliding: rescale the force and the stored displacement
        if (shrmag != 0.0) {
            f_t = f_t * (ft_friction / ft_shear);
            stored = f_t * (-1.0 / kt);
        } else {
            f_t = vec3<f32>(0.0);
        }
    } else {
        // 4. sticking: add damping
        f_t = f_t - k.vtr * gammat;
    }
    var o: Out;
    o.force = k.en * fn_scalar + f_t;
    o.torque = cross(k.en, f_t) * (-k.cri);
    o.shear = stored;
    return o;
}

// The wall kinematics of `ContactKinematics::wall` (wall at rest).
fn wall_kin(i: u32, normal: vec3<f32>, delta_n: f32) -> Kin {
    let r = ps[i].hi.w;
    let cr = r - 0.5 * delta_n;
    let delta = normal * r;
    let vr = ps[i].v.xyz;
    let vn = dot(vr, normal);
    let vt = vr - normal * vn;
    let wr = ps[i].w.xyz * (cr / r);
    var k: Kin;
    k.en = normal;
    k.delta_n = delta_n;
    k.vn = vn;
    k.vtr = vt + cross(delta, wr);
    k.cri = cr;
    k.r_eff = r;
    k.m_eff = ps[i].lo.w;
    return k;
}

// The kinematics of `ContactKinematics::mesh_wall` (facet at rest), with
// `delta` = centre − closest point, `d` = |delta| and `ov` = r − d. Upstream:
// r = radi − deltan (= d), en = delta / r, c_r = radi − deltan / 2,
// wr = c_r ω / r, v_tr = v_t + delta × wr; the overlap handed to the normal
// model is `ov`, formed in double-f32 by the caller.
fn mesh_kin(i: u32, delta: vec3<f32>, d: f32, ov: f32) -> Kin {
    let r = ps[i].hi.w;
    let rinv = 1.0 / d;
    let en = delta * rinv;
    let vr = ps[i].v.xyz;
    let vn = dot(vr, en);
    let vt = vr - en * vn;
    let cr = r - 0.5 * ov;
    let wr = ps[i].w.xyz * (cr * rinv);
    var k: Kin;
    k.en = en;
    k.delta_n = ov;
    k.vn = vn;
    k.vtr = vt + cross(delta, wr);
    k.cri = cr;
    k.r_eff = r;
    k.m_eff = ps[i].lo.w;
    return k;
}

// ── facet resolution: `TriMeshState::resolve_tri_sphere_contact_bary` ─────
// (`tri_mesh.rs`, a translation of LIGGGHTS-PUBLIC 3d5c00f2 `tri_mesh_I.h`:
// `resolveTriSphereContactBary`, `resolveCornerContactBary`,
// `resolveEdgeContactBary`, `resolveFaceContactBary`; `math_extra_liggghts.h`:
// `calcBaryTriCoords`), in coordinates local to node 0, in f32. Same
// branches, same thresholds; the barycentric coordinates themselves are not
// kept (they weight the node velocities of a moving mesh, and this mesh is
// at rest).

struct Facet {
    d: f32,           // distance centre → closest point (LARGE_TRIMESH: skipped)
    delta: vec3<f32>, // closest point − centre
    code: u32,        // barycentric region code, 7 = face
};

fn tri_node(t: u32, k: u32) -> vec3<f32> {
    if (k == 0u) { return vec3<f32>(0.0); }
    return tris[TRI * t + 1u + k].xyz;
}

fn tri_edge_vec(t: u32, k: u32) -> vec3<f32> {
    return tris[TRI * t + 5u + k].xyz;
}

fn tri_edge_len(t: u32, k: u32) -> f32 {
    return tris[TRI * t + 2u + k].w;
}

// Activity bit: edge k is bit k, corner k is bit 3 + k.
fn tri_flag(t: u32, bit: u32) -> bool {
    return ((u32(tris[TRI * t + 5u].w) >> bit) & 1u) != 0u;
}

// Whether facet `s` (an f32 value) is one of t's coplanar node neighbours
// (`areCoplanarNodeNeighs`).
fn coplanar(t: u32, s: f32) -> bool {
    let a = tris[TRI * t + 9u];
    let b = tris[TRI * t + 10u];
    return any(a == vec4<f32>(s)) || any(b == vec4<f32>(s));
}

fn facet_at(p: vec3<f32>, closest: vec3<f32>, code: u32) -> Facet {
    var o: Facet;
    o.delta = closest - p;
    o.d = length(p - closest);
    o.code = code;
    return o;
}

fn facet_skip(code: u32) -> Facet {
    var o: Facet;
    o.d = LARGE_TRIMESH;
    o.delta = vec3<f32>(0.0);
    o.code = code;
    return o;
}

fn resolve_edge(t: u32, ie: u32, p: vec3<f32>, code: u32) -> Facet {
    let ip = (ie + 1u) % 3u;
    let n0 = tri_node(t, ie);
    let dist = dot(p - n0, tri_edge_vec(t, ie));
    if (dist < -SMALL_TRIMESH) {
        if (!tri_flag(t, 3u + ie)) { return facet_skip(code); }
        return facet_at(p, n0, code);
    } else if (dist > tri_edge_len(t, ie) + SMALL_TRIMESH) {
        if (!tri_flag(t, 3u + ip)) { return facet_skip(code); }
        return facet_at(p, tri_node(t, ip), code);
    }
    if (!tri_flag(t, ie)) { return facet_skip(code); }
    return facet_at(p, n0 + tri_edge_vec(t, ie) * dist, code);
}

fn resolve_corner(t: u32, inode: u32, obtuse: bool, p: vec3<f32>, code: u32) -> Facet {
    let ip = (inode + 1u) % 3u;
    let ipp = (inode + 2u) % 3u;
    let n = tri_node(t, inode);
    if (obtuse) {
        let node_to_p = p - n;
        let d1 = dot(node_to_p, tri_edge_vec(t, ipp));
        if (d1 < SMALL_TRIMESH) {
            if (d1 > -tri_edge_len(t, ipp)) {
                if (!tri_flag(t, ipp)) { return facet_skip(code); }
                return facet_at(p, n + tri_edge_vec(t, ipp) * d1, code);
            }
            if (!tri_flag(t, 3u + ipp)) { return facet_skip(code); }
            return facet_at(p, tri_node(t, ipp), code);
        }
        let d2 = dot(node_to_p, tri_edge_vec(t, inode));
        if (d2 > -SMALL_TRIMESH) {
            if (d2 < tri_edge_len(t, inode)) {
                if (!tri_flag(t, inode)) { return facet_skip(code); }
                return facet_at(p, n + tri_edge_vec(t, inode) * d2, code);
            }
            if (!tri_flag(t, 3u + ip)) { return facet_skip(code); }
            return facet_at(p, tri_node(t, ip), code);
        }
    }
    if (!tri_flag(t, 3u + inode)) { return facet_skip(code); }
    return facet_at(p, n, code);
}

// a · (b.x + b.y) for a double-f32 vector a and a hi + lo direction.
fn dd_dot(ax: vec2<f32>, ay: vec2<f32>, az: vec2<f32>, h: vec3<f32>, l: vec3<f32>) -> vec2<f32> {
    return dd_add(
        dd_add(dd_mul(vec2<f32>(h.x, l.x), ax), dd_mul(vec2<f32>(h.y, l.y), ay)),
        dd_mul(vec2<f32>(h.z, l.z), az),
    );
}

// The barycentric region code of `calcBaryTriCoords` + the sign test of
// `resolveTriSphereContactBary`, in double-f32: `q` is node 0 → centre as
// hi + lo, the edge vectors are hi + lo and the coefficients
// 1/(len0 (1 − c²)), −c/(len0 (1 − c²)), c/(len2 (1 − c²)), −1/(len2 (1 − c²))
// are formed in f64 by the set-up. In f32 the coordinates of a 0.75 m facet
// carry ~1e-7 against upstream's threshold −1e-8/(2 rBound): a pebble on a
// quad's diagonal read as an edge contact on both coplanar facets, whose
// shared edge is inactive, and lost the wall (gh:#1503, case 5, ball 2).
fn bary_code(t: u32, qx: vec2<f32>, qy: vec2<f32>, qz: vec2<f32>) -> u32 {
    let a = dd_dot(qx, qy, qz, tri_edge_vec(t, 0u), tris[TRI * t + 12u].xyz);
    let b = dd_dot(qx, qy, qz, tri_edge_vec(t, 2u), tris[TRI * t + 13u].xyz);
    let k = tris[TRI * t + 14u];
    let m = tris[TRI * t + 15u];
    let b1 = dd_add(dd_mul(a, k.xy), dd_mul(b, k.zw));
    let b2 = dd_add(dd_mul(a, m.xy), dd_mul(b, m.zw));
    let b0 = dd_add(ts(1.0, -b1.x), dd_add(vec2<f32>(-b1.y, 0.0), vec2<f32>(-b2.x, -b2.y)));
    let thr = tris[TRI * t].w;
    return u32(b0.x + b0.y > thr) + 2u * u32(b1.x + b1.y > thr) + 4u * u32(b2.x + b2.y > thr);
}

fn resolve_facet(t: u32, p: vec3<f32>, code: u32) -> Facet {
    let ob = i32(tris[TRI * t + 1u].w);
    var out = facet_skip(code);
    switch code {
        case 1u: { out = resolve_corner(t, 0u, ob == 0, p, code); }
        case 2u: { out = resolve_corner(t, 1u, ob == 1, p, code); }
        case 3u: { out = resolve_edge(t, 0u, p, code); }
        case 4u: { out = resolve_corner(t, 2u, ob == 2, p, code); }
        case 5u: { out = resolve_edge(t, 2u, p, code); }
        case 6u: { out = resolve_edge(t, 1u, p, code); }
        case 7u: {
            // resolveFaceContactBary: the foot of the perpendicular
            let sn = tris[TRI * t + 4u].xyz;
            out = facet_at(p, p - sn * dot(sn, p), code);
        }
        default: { out = facet_skip(code); }
    }
    return out;
}

fn find_prior(i: u32, j: u32) -> vec3<f32> {
    let base = i * (prm.k_slots + 2u + M_SLOTS);
    for (var s = 0u; s < prm.k_slots; s = s + 1u) {
        let h = h_old[base + s];
        let nb = h.w;
        if (nb == f32(j)) { return h.xyz; }
        if (nb == EMPTY) { break; }
    }
    return vec3<f32>(0.0);
}

@compute @workgroup_size(64)
fn forces(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= prm.n) { return; }
    let pi = ps[i];
    let ri = pi.hi.w;
    let mi = pi.lo.w;
    let base = i * (prm.k_slots + 2u + M_SLOTS);
    var f = vec3<f32>(0.0);
    var t = vec3<f32>(0.0);
    var used = 0u;

    // Pairs: the 27 cells around i, each cell's entries in index order.
    let ci = cell_of(pi.hi.xyz + pi.lo.xyz);
    for (var dz = -1; dz <= 1; dz = dz + 1) {
        for (var dy = -1; dy <= 1; dy = dy + 1) {
            for (var dx = -1; dx <= 1; dx = dx + 1) {
                let cc = ci + vec3<i32>(dx, dy, dz);
                if (any(cc < vec3<i32>(0)) || any(cc >= vec3<i32>(prm.dims.xyz))) { continue; }
                let c = cell_index(cc);
                let cnt = min(atomicLoad(&cells[c]), prm.cap);
                for (var s = 0u; s < cnt; s = s + 1u) {
                    let j = atomicLoad(&cells[prm.dims.w + c * prm.cap + s]);
                    if (j == i) { continue; }
                    let pj = ps[j];
                    // x_i - x_j and |x_i - x_j|² in double-f32.
                    let dx = dd_add(ts(pi.hi.x, -pj.hi.x), vec2<f32>(pi.lo.x - pj.lo.x, 0.0));
                    let dy = dd_add(ts(pi.hi.y, -pj.hi.y), vec2<f32>(pi.lo.y - pj.lo.y, 0.0));
                    let dzz = dd_add(ts(pi.hi.z, -pj.hi.z), vec2<f32>(pi.lo.z - pj.lo.z, 0.0));
                    let r2 = dd_add(dd_add(dd_sq(dx), dd_sq(dy)), dd_sq(dzz));
                    let delta = vec3<f32>(dx.x + dx.y, dy.x + dy.y, dzz.x + dzz.y);
                    let rr = sqrt(r2.x + r2.y);
                    let rj = pj.hi.w;
                    let radsum2 = dd_add(ts(ri, rj), vec2<f32>(pi.v.w + pj.v.w, 0.0));
                    let radsum = radsum2.x + radsum2.y;
                    // δ = (R² − r²) / (R + r), the difference taken in double-f32.
                    let num = dd_add(dd_sq(radsum2), vec2<f32>(-r2.x, -r2.y));
                    let delta_n = (num.x + num.y) / (radsum + rr);
                    if (delta_n <= 0.0 || rr <= 0.0) { continue; }
                    let rinv = 1.0 / rr;
                    let en = delta * rinv;
                    let vr = pi.v.xyz - pj.v.xyz;
                    let vn = dot(vr, en);
                    let vt = vr - en * vn;
                    let cri = ri - 0.5 * delta_n;
                    let crj = rj - 0.5 * delta_n;
                    let wr = (pi.w.xyz * cri + pj.w.xyz * crj) * rinv;
                    var k: Kin;
                    k.en = en;
                    k.delta_n = delta_n;
                    k.vn = vn;
                    k.vtr = vt + cross(delta, wr);
                    k.cri = cri;
                    k.r_eff = ri * rj / radsum;
                    k.m_eff = mi * pj.lo.w / (mi + pj.lo.w);
                    let o = resolve(find_prior(i, j), k);
                    f = f + o.force;
                    t = t + o.torque;
                    if (used < prm.k_slots) {
                        h_new[base + used] = vec4<f32>(o.shear, f32(j));
                        used = used + 1u;
                    } else {
                        atomicAdd(&errs[1], 1u);
                    }
                }
            }
        }
    }
    for (var s = used; s < prm.k_slots; s = s + 1u) {
        h_new[base + s] = vec4<f32>(0.0, 0.0, 0.0, EMPTY);
    }

    let p = pi.hi.xyz + pi.lo.xyz;
    // Walls, in `GranularSystem`'s order: the cylinder, the one-sided wall,
    // then the mesh. Wall slot w is base + k_slots + w; a slot not touched
    // this step is cleared (`ShearHistory::end_step`).
    for (var w = 0u; w < 2u; w = w + 1u) {
        var hit = false;
        var normal = vec3<f32>(0.0);
        var delta_n = 0.0;
        if (w == 0u && prm.flags_in.x != 0u) {
            // δ = r − (R − ρ), every term in double-f32.
            let qx = dd_add(ts(pi.hi.x, -prm.cyl.x), vec2<f32>(pi.lo.x, 0.0));
            let qy = dd_add(ts(pi.hi.y, -prm.cyl.y), vec2<f32>(pi.lo.y, 0.0));
            let rho = dd_sqrt(dd_add(dd_sq(qx), dd_sq(qy)));
            if (rho.x > 0.0) {
                let dd = dd_add(dd_add(ts(ri, -prm.cyl.z), vec2<f32>(pi.v.w - prm.cyl.w, 0.0)), rho);
                let d = dd.x + dd.y;
                let rad = vec2<f32>(qx.x + qx.y, qy.x + qy.y);
                if (d > 0.0) { hit = true; delta_n = d; normal = vec3<f32>(-rad / (rho.x + rho.y), 0.0); }
            }
        } else if (w == 1u && prm.flags_in.y != 0u) {
            // s = (z − z_wall) n_z, δ = r − s, in double-f32.
            let sz = dd_add(ts(pi.hi.z, -prm.valve.x), vec2<f32>(pi.lo.z - prm.valve.z, 0.0));
            let s2 = vec2<f32>(sz.x * prm.valve.y, sz.y * prm.valve.y);
            let dd = dd_add(ts(ri, -s2.x), vec2<f32>(pi.v.w - s2.y, 0.0));
            let d = dd.x + dd.y;
            if (d > 0.0) { hit = true; delta_n = d; normal = vec3<f32>(0.0, 0.0, prm.valve.y); }
        }
        let slot = base + prm.k_slots + w;
        if (hit) {
            let prior_h = h_old[slot];
            var prior = vec3<f32>(0.0);
            if (prior_h.w != 0.0) { prior = prior_h.xyz; }
            let o = resolve(prior, wall_kin(i, normal, delta_n));
            f = f + o.force;
            t = t + o.torque;
            h_new[slot] = vec4<f32>(o.shear, 1.0);
        } else {
            h_new[slot] = vec4<f32>(0.0);
        }
    }

    // The mesh: `GranularSystem::compute_mesh_wall_forces`, i.e. upstream's
    // `FixWallGran::post_force_mesh` (CPU since gh:#1416; here gh:#1503).
    // Every facet the sphere overlaps is its own contact, in ascending facet
    // order, resolved by `resolveTriSphereContactBary` (inactive edges and
    // corners skipped). The contact list (`FixContactHistoryMesh`) keeps one
    // slot per facet: a new face contact is dropped when a coplanar
    // neighbour facet was already handled this step, and otherwise inherits
    // the spring of the last coplanar neighbour in slot order; a slot not
    // touched this step is emptied (`cleanUpContacts`).
    if (prm.flags_in.z != 0u) {
        let mb = base + prm.k_slots + 2u;
        var sid: array<f32, M_SLOTS>;
        var ssh: array<vec3<f32>, M_SLOTS>;
        var stouch: array<bool, M_SLOTS>;
        for (var m = 0u; m < M_SLOTS; m = m + 1u) {
            let h = h_old[mb + m];
            sid[m] = h.w;
            ssh[m] = h.xyz;
            stouch[m] = false;
        }
        if (length(p - prm.hull.xyz) - prm.hull.w < ri) {
            for (var tt = 0u; tt < prm.n_tris; tt = tt + 1u) {
                let bb = tris[TRI * tt + 8u];
                if (length(p - bb.xyz) - bb.w >= ri) { continue; }
                // The particle in coordinates local to node 0.
                let lp = keep3(pi.hi.xyz - tris[TRI * tt].xyz) + keep3(pi.lo.xyz - tris[TRI * tt + 1u].xyz);
                let ah = tris[TRI * tt].xyz;
                let al = tris[TRI * tt + 1u].xyz;
                let qx = dd_add(ts(pi.hi.x, -ah.x), vec2<f32>(keep(pi.lo.x - al.x), 0.0));
                let qy = dd_add(ts(pi.hi.y, -ah.y), vec2<f32>(keep(pi.lo.y - al.y), 0.0));
                let qz = dd_add(ts(pi.hi.z, -ah.z), vec2<f32>(keep(pi.lo.z - al.z), 0.0));
                var c = resolve_facet(tt, lp, bary_code(tt, qx, qy, qz));
                // overlap = r − d (upstream deltan = d − r <= 0), r as hi + lo.
                let dd = ts(ri, -c.d);
                var ov = dd.x + keep(dd.y + pi.v.w);
                if (c.code == 7u && ov > -ri) {
                    // A face contact: d = |n · (x − a)| re-formed in
                    // double-f32 (x, a and n each as hi + lo). In f32 a
                    // facet 0.75 m long (the conus) leaves ~1e-7 m in d
                    // against a ~1e-4 m overlap (gh:#1503, case 5).
                    var dn = dd_dot(qx, qy, qz, tris[TRI * tt + 4u].xyz, tris[TRI * tt + 11u].xyz);
                    if (dn.x < 0.0) { dn = -dn; }
                    let o2 = dd_add(dd_add(ts(ri, -dn.x), vec2<f32>(-dn.y, 0.0)), vec2<f32>(pi.v.w, 0.0));
                    ov = o2.x + o2.y;
                    c.d = dn.x + dn.y;
                }
                if (ov < 0.0) { continue; }
                let face = c.code == 7u;
                let id = f32(tt);
                var slot = M_SLOTS;
                // haveContact
                for (var m = 0u; m < M_SLOTS; m = m + 1u) {
                    if (sid[m] == id) { slot = m; break; }
                }
                if (slot < M_SLOTS) {
                    stouch[slot] = true;
                } else {
                    // coplanarContactAlready
                    if (face) {
                        var already = false;
                        for (var m = 0u; m < M_SLOTS; m = m + 1u) {
                            if (sid[m] >= 0.0 && sid[m] != id && stouch[m] && coplanar(tt, sid[m])) {
                                already = true;
                            }
                        }
                        if (already) { continue; }
                    }
                    // addNewTriContactToExistingParticle: the first empty slot
                    for (var m = 0u; m < M_SLOTS; m = m + 1u) {
                        if (sid[m] < 0.0) { slot = m; break; }
                    }
                    if (slot == M_SLOTS) {
                        atomicAdd(&errs[1], 1u);
                        continue;
                    }
                    sid[slot] = id;
                    stouch[slot] = true;
                    ssh[slot] = vec3<f32>(0.0);
                    // checkCoplanarContactHistory
                    if (face) {
                        for (var m = 0u; m < M_SLOTS; m = m + 1u) {
                            if (m != slot && sid[m] >= 0.0 && sid[m] != id && coplanar(tt, sid[m])) {
                                ssh[slot] = ssh[m];
                            }
                        }
                    }
                }
                let o = resolve(ssh[slot], mesh_kin(i, -c.delta, c.d, ov));
                f = f + o.force;
                t = t + o.torque;
                ssh[slot] = o.shear;
            }
        }
        // cleanUpContacts
        for (var m = 0u; m < M_SLOTS; m = m + 1u) {
            if (stouch[m]) {
                h_new[mb + m] = vec4<f32>(ssh[m], sid[m]);
            } else {
                h_new[mb + m] = vec4<f32>(0.0, 0.0, 0.0, EMPTY);
            }
        }
    }

    f = f + prm.gravity.xyz * mi;
    ft[2u * i] = vec4<f32>(f, 0.0);
    ft[2u * i + 1u] = vec4<f32>(t, 0.0);
}
