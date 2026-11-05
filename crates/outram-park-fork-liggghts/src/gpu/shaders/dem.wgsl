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
// Shear slots: per particle `k_slots` pair slots (xyz spring, w = bitcast
// neighbour index, 0xffffffff empty), then 3 wall slots (w = 1 live).
@group(0) @binding(4) var<storage, read_write> h_old: array<vec4<f32>>;
@group(0) @binding(5) var<storage, read_write> h_new: array<vec4<f32>>;
// Mesh triangles, six vec4s each: a (hi), a (lo), b − a, c − a, normal,
// bounds (centroid, radius). The closest point is found in coordinates local
// to a, so magnitudes are a facet's size, not the vessel's.
@group(0) @binding(6) var<storage, read_write> tris: array<vec4<f32>>;
// [0] cell overflow, [1] contact-slot overflow.
@group(0) @binding(7) var<storage, read_write> errs: array<atomic<u32>>;

const EMPTY: u32 = 0xffffffffu;
const SQRT_FIVE_OVER_SIX: f32 = 0.9128709291752769;
const INERTIA: f32 = 0.4;

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
// so it is computed as written. Exact: x · 1 = x.
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
    let e = (a - keep3(s - bb)) + (b - bb);
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
// transformations; `fma` gives the exact product error), δ keeps ~1e-7 of
// itself, which is f32's own resolution of the result.

fn ts(a: f32, b: f32) -> vec2<f32> {
    let s = keep(a + b);
    let bb = keep(s - a);
    return vec2<f32>(s, (a - keep(s - bb)) + (b - bb));
}

// (a.x + a.y) + (b.x + b.y), renormalised.
fn dd_add(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    let s = ts(a.x, b.x);
    let lo = keep(s.y + a.y + b.y);
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

// (a.x + a.y)², to double-f32.
fn dd_sq(a: vec2<f32>) -> vec2<f32> {
    let p = keep(a.x * a.x);
    let e = fma(a.x, a.x, -p);
    let lo = keep(e + 2.0 * a.x * a.y);
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

// Ericson's closest point on a triangle (`mesh_wall.rs`, §5.1.5).
fn closest_on_triangle(p: vec3<f32>, a: vec3<f32>, b: vec3<f32>, c: vec3<f32>) -> vec3<f32> {
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d1 = dot(ab, ap);
    let d2 = dot(ac, ap);
    if (d1 <= 0.0 && d2 <= 0.0) { return a; }
    let bp = p - b;
    let d3 = dot(ab, bp);
    let d4 = dot(ac, bp);
    if (d3 >= 0.0 && d4 <= d3) { return b; }
    let vc = d1 * d4 - d3 * d2;
    if (vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0) { return a + ab * (d1 / (d1 - d3)); }
    let cp = p - c;
    let d5 = dot(ab, cp);
    let d6 = dot(ac, cp);
    if (d6 >= 0.0 && d5 <= d6) { return c; }
    let vb = d5 * d2 - d1 * d6;
    if (vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0) { return a + ac * (d2 / (d2 - d6)); }
    let va = d3 * d6 - d5 * d4;
    if (va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0) {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    return a + ab * (vb * denom) + ac * (vc * denom);
}

fn find_prior(i: u32, j: u32) -> vec3<f32> {
    let base = i * (prm.k_slots + 3u);
    for (var s = 0u; s < prm.k_slots; s = s + 1u) {
        let h = h_old[base + s];
        let nb = bitcast<u32>(h.w);
        if (nb == j) { return h.xyz; }
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
    let base = i * (prm.k_slots + 3u);
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
                        h_new[base + used] = vec4<f32>(o.shear, bitcast<f32>(j));
                        used = used + 1u;
                    } else {
                        atomicAdd(&errs[1], 1u);
                    }
                }
            }
        }
    }
    for (var s = used; s < prm.k_slots; s = s + 1u) {
        h_new[base + s] = vec4<f32>(0.0, 0.0, 0.0, bitcast<f32>(EMPTY));
    }

    let p = pi.hi.xyz + pi.lo.xyz;
    // Walls, in `GranularSystem`'s order: the cylinder, the one-sided wall,
    // then the mesh. Wall slot w is base + k_slots + w; a slot not touched
    // this step is cleared (`ShearHistory::end_step`).
    for (var w = 0u; w < 3u; w = w + 1u) {
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
        } else if (w == 2u && prm.flags_in.z != 0u) {
            if (length(p - prm.hull.xyz) - prm.hull.w < ri) {
                var best = 3.4e38;
                var best_n = vec3<f32>(0.0);
                for (var tt = 0u; tt < prm.n_tris; tt = tt + 1u) {
                    let bb = tris[6u * tt + 5u];
                    if (length(p - bb.xyz) - bb.w >= ri) { continue; }
                    // The particle in coordinates local to vertex a.
                    let ah = tris[6u * tt].xyz;
                    let al = tris[6u * tt + 1u].xyz;
                    let lp = (pi.hi.xyz - ah) + (pi.lo.xyz - al);
                    let q = closest_on_triangle(lp, vec3<f32>(0.0), tris[6u * tt + 2u].xyz, tris[6u * tt + 3u].xyz);
                    let dist = length(lp - q);
                    if (dist < best) { best = dist; best_n = tris[6u * tt + 4u].xyz; }
                }
                let dd = ts(ri, -best);
                let d = dd.x + (dd.y + pi.v.w);
                if (d > 0.0) { hit = true; delta_n = d; normal = best_n; }
            }
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

    f = f + prm.gravity.xyz * mi;
    ft[2u * i] = vec4<f32>(f, 0.0);
    ft[2u * i + 1u] = vec4<f32>(t, 0.0);
}
