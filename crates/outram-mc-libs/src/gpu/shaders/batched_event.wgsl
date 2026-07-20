// batched_event.wgsl — ONE full transport event (flight + collision) for a
// resident batch of neutrons, run entirely on the GPU.
//
// WHAT THIS COMPUTES (the op-u6s.8 fix)
// -------------------------------------
// The batched-flight kernel (batched_flight.wgsl) put only the FLIGHT on the GPU
// and left the collision on the CPU, forcing a CPU<->GPU round-trip PER EVENT.
// This kernel does the WHOLE event — flight AND collision physics — on the GPU,
// so a generation's batch stays resident in GPU buffers across every event and
// the only CPU traffic is (a) a 4-byte alive-count read per event and (b) one
// fission-record read-back per GENERATION.
//
// Per invocation (one neutron, global_invocation_id.x = index i):
//   1. If i>=N or the neutron is already dead, do nothing.
//   2. FLIGHT: advance the per-particle 64-bit LCG one step -> xi; look up the
//      macroscopic total Sigma_t at the neutron energy (binary search + linear
//      interp on the shared union grid); sample d_col=-ln(xi)/Sigma_t; find the
//      distance to the bounding sphere; if the sphere is nearer -> LEAK (dead).
//   3. COLLISION (mirrors CPU `collide_batched`, src/physics/keff.rs):
//        a. stream the neutron to the collision site;
//        b. sample the collision nuclide j ~ N_j*sigma_t,j(E);
//        c. partition the reaction on the microscopic total:
//             fission | capture | inelastic(continuum) | elastic;
//        d. fission -> record (production=nu_bar, nuclide j); neutron dies (its
//           daughters are banked on the CPU once per generation, using the
//           handed-back seed, so the coherent stream continues there);
//        e. capture -> dies;
//        f. inelastic -> Weisskopf continuum down-scatter (new E, direction);
//        g. elastic -> two-body kinematics, isotropic-CM below the CE/MG seam or
//           the maximum-entropy exponential-mu law (group mean cosine) above it.
//   4. A neutron that scatters stays alive: atomicAdd(alive_count, 1).
//
// PROVENANCE (CPU sources mirrored, all in this crate + OpenMC)
//   - LCG state advance: OpenMC src/random_lcg.cpp:32-35 (as batched_flight.wgsl).
//   - Sigma_t grid search + interp: OpenMC src/nuclide.cpp:716-740.
//   - Sphere distance: OpenMC src/surface.cpp:607-638.
//   - Nuclide sampling: src/material/material.rs `sample_nuclide`.
//   - Reaction partition: src/physics/keff.rs `collide_batched` / `transport_history`.
//   - Elastic/inelastic kinematics: src/physics/scatter.rs
//     (`two_body_scatter_with_mu`, `continuum_inelastic_scatter`, `rotate_direction`,
//     `cm_to_lab`).
//   - Exponential-mu / Langevin inverse: src/material/nuclide.rs
//     (`sample_exponential_mu`, `langevin_inverse`).
//
// PRECISION / STATUS
//   Everything here is f32; the trusted reference is the raw-f64 CPU transport
//   loop. `advance_event_cpu_mirror` (batched_event.rs) runs the SAME f32 path and
//   is the bit-level reference for this kernel's LOGIC. The uniform is the top-24
//   bits of the advanced 64-bit LCG state (as in batched_flight.wgsl) — the
//   documented f32 divergence from the CPU f64 `prn` VALUE; the integer state
//   stream stays bit-exact. UNTRUSTED, AI-DRAFTED: must pass the V&V gate and
//   human review before it is trusted.
//
// BUFFER LAYOUT (4 storage + 1 uniform; downlevel_defaults allows 4 storage/stage)
//   @binding(0) xs    : array<f32> read       — packed tables, see accessors below
//   @binding(1) istate: array<u32> read_write — seed_lo ++ seed_hi ++ alive ++ fiss_nuc
//   @binding(2) fstate: array<f32> read_write — pos(3N) ++ dir(3N) ++ energy(N) ++ production(N)
//   @binding(3) ctrl  : atomic<u32> read_write — live-neutron count for this event
//   @binding(4) params: uniform Params
//
// xs packing (G = n_grid, NN = n_nuclide), all f32:
//   grid[0..G] ++ macro_total[0..G]
//   ++ micro_total[NN*G] ++ micro_fission[NN*G] ++ micro_absorption[NN*G]
//   ++ micro_inelastic[NN*G] ++ micro_nu_fission[NN*G] ++ micro_mubar[NN*G]
//   ++ atom_density[NN] ++ awr[NN] ++ e_max[NN]
// per-nuclide channel c of nuclide j at grid point k = base_c + j*G + k.
//
// Workgroup size 64 (one invocation per neutron).

struct Params {
    n_grid: u32,       // G
    n_particle: u32,   // N
    n_nuclide: u32,    // NN
    pad0: u32,
    sphere: vec4<f32>, // (center_x, center_y, center_z, radius) cm
};

@group(0) @binding(0) var<storage, read>        xs:     array<f32>;
@group(0) @binding(1) var<storage, read_write>  istate: array<u32>;
@group(0) @binding(2) var<storage, read_write>  fstate: array<f32>;
@group(0) @binding(3) var<storage, read_write>  ctrl:   atomic<u32>;
@group(0) @binding(4) var<uniform>              params: Params;

// LCG constants split into 32-bit halves (OpenMC src/random_lcg.cpp:11-12).
const MULT_HI: u32 = 0x5851F42Du;
const MULT_LO: u32 = 0x4C957F2Du;
const INC_HI:  u32 = 0x14057B7Eu;
const INC_LO:  u32 = 0xF767814Fu;

const BIG: f32 = 1e30;
const EPS: f32 = 1e-7;
const PI:  f32 = 3.14159265358979323846;
const FISS_NONE: u32 = 0xFFFFFFFFu; // "did not fission" sentinel in fiss_nuc

// ---- 64-bit LCG emulation (bit-exact integer state; f32 top-24 uniform) -------

fn mul_u32_full(x: u32, y: u32) -> vec2<u32> {
    let x0 = x & 0xFFFFu; let x1 = x >> 16u;
    let y0 = y & 0xFFFFu; let y1 = y >> 16u;
    let t0 = x0 * y0;
    let s  = x0 * y1;
    let t1 = s + x1 * y0;
    let carry1 = select(0u, 1u, t1 < s);
    let t2 = x1 * y1;
    let lo_lo16 = t0 & 0xFFFFu;
    let mid = (t0 >> 16u) + (t1 & 0xFFFFu);
    let lo = lo_lo16 | ((mid & 0xFFFFu) << 16u);
    let carry_mid = mid >> 16u;
    let hi = t2 + (t1 >> 16u) + carry_mid + (carry1 << 16u);
    return vec2<u32>(lo, hi);
}

fn mul64_low(a_lo: u32, a_hi: u32, b_lo: u32, b_hi: u32) -> vec2<u32> {
    let p = mul_u32_full(a_lo, b_lo);
    let cross = a_lo * b_hi + a_hi * b_lo;
    return vec2<u32>(p.x, p.y + cross);
}

// Advance the split seed (s.x = lo, s.y = hi) one step; return the new state
// (.xy) and the top-24-bit f32 uniform bitcast into .z.
fn rng_next(s: vec2<u32>) -> vec3<u32> {
    let m = mul64_low(s.x, s.y, MULT_LO, MULT_HI);
    let new_lo = m.x + INC_LO;
    let carry = select(0u, 1u, new_lo < INC_LO);
    let new_hi = m.y + INC_HI + carry;
    let xi = f32(new_hi >> 8u) * (1.0 / 16777216.0);
    return vec3<u32>(new_lo, new_hi, bitcast<u32>(xi));
}

// ---- Shared grid search (binary search + interpolation factor) ----------------

struct GridLoc { i: u32, f: f32 };

fn locate(e: f32) -> GridLoc {
    let n = params.n_grid;
    var i_grid: u32;
    if (e < xs[0]) {
        i_grid = 0u;
    } else if (e > xs[n - 1u]) {
        i_grid = n - 2u;
    } else {
        var lo: u32 = 0u;
        var hi: u32 = n - 1u;
        loop {
            if (hi - lo <= 1u) { break; }
            let mid = (lo + hi) >> 1u;
            if (xs[mid] <= e) { lo = mid; } else { hi = mid; }
        }
        i_grid = lo;
    }
    let d = xs[i_grid + 1u] - xs[i_grid];
    var f: f32;
    if (d == 0.0) { f = 0.0; } else { f = (e - xs[i_grid]) / d; }
    return GridLoc(i_grid, f);
}

// Linear-interpolate a packed per-nuclide channel (base offset `base`) for
// nuclide `j` at the located grid point.
fn interp_channel(base: u32, j: u32, loc: GridLoc) -> f32 {
    let g = params.n_grid;
    let k = base + j * g + loc.i;
    return (1.0 - loc.f) * xs[k] + loc.f * xs[k + 1u];
}

// ---- Kinematics (ports of src/physics/scatter.rs, f32) ------------------------

// Rotate unit direction `u` by cosine `mu` and a sampled azimuth; returns the new
// direction and advanced seed (one RNG draw for phi).
struct DirSeed { dir: vec3<f32>, seed: vec2<u32> };

fn rotate_direction(u: vec3<f32>, mu: f32, seed_in: vec2<u32>) -> DirSeed {
    let r = rng_next(seed_in);
    let seed = r.xy;
    let phi = 2.0 * PI * bitcast<f32>(r.z);
    let sinphi = sin(phi);
    let cosphi = cos(phi);
    let a = sqrt(max(1.0 - mu * mu, 0.0));
    let b0 = sqrt(max(1.0 - u.z * u.z, 0.0));
    var dir: vec3<f32>;
    if (b0 > 1.0e-10) {
        dir = vec3<f32>(
            mu * u.x + a * (u.x * u.z * cosphi - u.y * sinphi) / b0,
            mu * u.y + a * (u.y * u.z * cosphi + u.x * sinphi) / b0,
            mu * u.z - a * b0 * cosphi,
        );
    } else {
        let b = sqrt(max(1.0 - u.y * u.y, 0.0));
        dir = vec3<f32>(
            mu * u.x + a * (u.x * u.y * cosphi + u.z * sinphi) / b,
            mu * u.y - a * b * cosphi,
            mu * u.z + a * (u.y * u.z * cosphi - u.x * sinphi) / b,
        );
    }
    return DirSeed(dir, seed);
}

// CM outgoing energy + CM cosine -> lab (e_out, mu_lab). Mirrors cm_to_lab.
fn cm_to_lab(e: f32, e_cm_out: f32, mu_cm: f32, awr: f32) -> vec2<f32> {
    let ap1 = awr + 1.0;
    let e_trans = e / (ap1 * ap1);
    let cross = 2.0 * mu_cm * sqrt(e_cm_out * e_trans);
    let e_out = max(e_cm_out + e_trans + cross, 0.0);
    var mu_lab: f32;
    if (e_out > 0.0) {
        mu_lab = clamp(mu_cm * sqrt(e_cm_out / e_out) + sqrt(e_trans / e_out), -1.0, 1.0);
    } else {
        mu_lab = 1.0;
    }
    return vec2<f32>(e_out, mu_lab);
}

struct Scatter { e: f32, dir: vec3<f32>, seed: vec2<u32> };

// Two-body scatter with a supplied CM cosine (Q-value `q`). One RNG draw (azimuth).
fn two_body_with_mu(e: f32, u: vec3<f32>, awr: f32, q: f32, mu_cm: f32, seed_in: vec2<u32>) -> Scatter {
    let a = awr;
    let ap1 = a + 1.0;
    let e_cm_out = max(e * (a / ap1) * (a / ap1) + q * a / ap1, 0.0);
    let lab = cm_to_lab(e, e_cm_out, clamp(mu_cm, -1.0, 1.0), a);
    let ds = rotate_direction(u, lab.y, seed_in);
    return Scatter(lab.x, ds.dir, ds.seed);
}

// Invert the Langevin function L(l)=coth l - 1/l = mu_bar (Newton). No draws.
fn langevin_inverse(mu_bar: f32) -> f32 {
    let x = abs(mu_bar);
    var lambda: f32;
    if (x < 0.3) { lambda = 3.0 * x; } else { lambda = min(1.0 / (1.0 - x), 50.0); }
    for (var it: u32 = 0u; it < 30u; it = it + 1u) {
        let coth = 1.0 / tanh(lambda);
        let f = coth - 1.0 / lambda - x;
        let d = 1.0 / (lambda * lambda) - (coth * coth - 1.0);
        if (abs(d) < 1.0e-12) { break; }
        let step = f / d;
        lambda = lambda - step;
        if (lambda <= 0.0) { lambda = 1.0e-3; }
        if (abs(step) < 1.0e-10) { break; }
    }
    if (mu_bar < 0.0) { return -lambda; }
    return lambda;
}

// Sample an elastic CM cosine from the exponential-mu law with mean `mubar`,
// using the supplied uniform `xi`. Mirrors sample_exponential_mu (no draws here;
// the caller drew `xi`).
fn exponential_mu(mubar: f32, xi: f32) -> f32 {
    let mu_bar = clamp(mubar, -0.99, 0.99);
    if (abs(mu_bar) < 1.0e-4) { return clamp(2.0 * xi - 1.0, -1.0, 1.0); }
    let lambda = langevin_inverse(mu_bar);
    if (abs(lambda) < 1.0e-6) { return clamp(2.0 * xi - 1.0, -1.0, 1.0); }
    let mu = 1.0 + log(xi + (1.0 - xi) * exp(-2.0 * lambda)) / lambda;
    return clamp(mu, -1.0, 1.0);
}

// Continuum inelastic (Weisskopf evaporation). Mirrors continuum_inelastic_scatter:
// draw order = e_cm_out seed (1), then up to 64 iters of (r1,r2), then mu (1), then
// rotate (1).
fn continuum_inelastic(e: f32, u: vec3<f32>, awr: f32, seed_in: vec2<u32>) -> Scatter {
    var seed = seed_in;
    let a = awr;
    let ap1 = a + 1.0;
    let e_cm_elastic = e * (a / ap1) * (a / ap1);
    let a_ld = max(a / 11.0, 1.0);
    let theta = max(sqrt(e * 1.0e-6 / a_ld) * 1.0e6, 1.0);

    var r = rng_next(seed); seed = r.xy;
    var e_cm_out = e_cm_elastic * bitcast<f32>(r.z);
    for (var it: u32 = 0u; it < 64u; it = it + 1u) {
        var r1v = rng_next(seed); seed = r1v.xy;
        var r2v = rng_next(seed); seed = r2v.xy;
        let cand = -theta * log(bitcast<f32>(r1v.z) * bitcast<f32>(r2v.z));
        if (cand <= e_cm_elastic) { e_cm_out = cand; break; }
    }
    var rmu = rng_next(seed); seed = rmu.xy;
    let mu_cm = 2.0 * bitcast<f32>(rmu.z) - 1.0;
    let lab = cm_to_lab(e, e_cm_out, mu_cm, a);
    let ds = rotate_direction(u, lab.y, seed);
    return Scatter(lab.x, ds.dir, ds.seed);
}

// ---- Main event kernel --------------------------------------------------------

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    let nn = params.n_particle;
    if (i >= nn) { return; }
    // Already dead (leaked/absorbed/fissioned on a previous event): skip.
    if (istate[2u * nn + i] == 0u) { return; }

    let n_nuc = params.n_nuclide;
    let g = params.n_grid;

    // Reassemble the seed (istate: seed_lo[0..N] ++ seed_hi[N..2N]).
    var seed = vec2<u32>(istate[i], istate[nn + i]);

    // Neutron phase-space (fstate: pos(3N) ++ dir(3N) ++ energy(N) ++ prod(N)).
    var px = fstate[3u * i + 0u];
    var py = fstate[3u * i + 1u];
    var pz = fstate[3u * i + 2u];
    let ux = fstate[3u * nn + 3u * i + 0u];
    let uy = fstate[3u * nn + 3u * i + 1u];
    let uz = fstate[3u * nn + 3u * i + 2u];
    let e = fstate[6u * nn + i];

    // ---- 1. FLIGHT ---------------------------------------------------------
    var rf = rng_next(seed); seed = rf.xy;
    let xi_flight = bitcast<f32>(rf.z);

    let loc = locate(e);
    // macroscopic total Sigma_t = macro_total column (offset G).
    let sigma_t = (1.0 - loc.f) * xs[g + loc.i] + loc.f * xs[g + loc.i + 1u];
    if (sigma_t <= 0.0) {
        istate[2u * nn + i] = 0u; // no interaction possible -> dead
        istate[i] = seed.x; istate[nn + i] = seed.y;
        return;
    }
    let d_col = -log(xi_flight) / sigma_t;

    // Distance to the bounding sphere (src/surface.cpp:607-638).
    let ox = px - params.sphere.x;
    let oy = py - params.sphere.y;
    let oz = pz - params.sphere.z;
    let kdot = ox * ux + oy * uy + oz * uz;
    let cc = ox * ox + oy * oy + oz * oz - params.sphere.w * params.sphere.w;
    let disc = kdot * kdot - cc;
    var d_bound: f32;
    if (disc < 0.0) {
        d_bound = BIG;
    } else {
        let sq = sqrt(disc);
        let d_near = -kdot - sq;
        if (d_near > EPS) {
            d_bound = d_near;
        } else {
            let d_far = -kdot + sq;
            if (d_far > EPS) { d_bound = d_far; } else { d_bound = BIG; }
        }
    }

    if (d_col >= d_bound) {
        istate[2u * nn + i] = 0u; // leaked
        istate[i] = seed.x; istate[nn + i] = seed.y;
        return;
    }

    // ---- 2. COLLISION ------------------------------------------------------
    // Stream to the collision site.
    px = px + ux * d_col;
    py = py + uy * d_col;
    pz = pz + uz * d_col;
    fstate[3u * i + 0u] = px;
    fstate[3u * i + 1u] = py;
    fstate[3u * i + 2u] = pz;

    // Channel base offsets in the xs buffer.
    let base_tot   = 2u * g;
    let base_fis   = 2u * g + 1u * n_nuc * g;
    let base_abs   = 2u * g + 2u * n_nuc * g;
    let base_inel  = 2u * g + 3u * n_nuc * g;
    let base_nufis = 2u * g + 4u * n_nuc * g;
    let base_mubar = 2u * g + 5u * n_nuc * g;
    let base_N     = 2u * g + 6u * n_nuc * g;
    let base_awr   = base_N + n_nuc;
    let base_emax  = base_N + 2u * n_nuc;

    // (a) sample the collision nuclide j ~ N_j * sigma_t,j(E).
    var sig_tot: f32 = 0.0;
    for (var j: u32 = 0u; j < n_nuc; j = j + 1u) {
        sig_tot = sig_tot + xs[base_N + j] * interp_channel(base_tot, j, loc);
    }
    var rn = rng_next(seed); seed = rn.xy;
    let xi_n = bitcast<f32>(rn.z) * sig_tot;
    var jsel: u32 = n_nuc - 1u;
    var acc: f32 = 0.0;
    for (var j: u32 = 0u; j < n_nuc; j = j + 1u) {
        acc = acc + xs[base_N + j] * interp_channel(base_tot, j, loc);
        if (xi_n < acc) { jsel = j; break; }
    }

    // (b) channel cross sections for the chosen nuclide at E.
    let x_tot   = interp_channel(base_tot, jsel, loc);
    let x_fis   = interp_channel(base_fis, jsel, loc);
    let x_abs   = interp_channel(base_abs, jsel, loc);
    let x_inel  = interp_channel(base_inel, jsel, loc);
    let x_nufis = interp_channel(base_nufis, jsel, loc);
    let mubar   = interp_channel(base_mubar, jsel, loc);
    let awr_j   = xs[base_awr + jsel];
    let emax_j  = xs[base_emax + jsel];

    // (c) reaction partition on the microscopic total (fission|capture|inelastic|
    //     elastic — (n,2n)=0 for the LOW tier, so it collapses out).
    var rr = rng_next(seed); seed = rr.xy;
    let xi_r = bitcast<f32>(rr.z) * x_tot;

    let u_in = vec3<f32>(ux, uy, uz);

    if (xi_r < x_fis) {
        // Fission: record nu_bar and the nuclide; the neutron dies here. Its
        // daughters (count + birth energy/angle) are sampled on the CPU per
        // generation from the handed-back seed, so the stream stays coherent.
        var nu_bar: f32 = 0.0;
        if (x_fis > 0.0) { nu_bar = x_nufis / x_fis; }
        fstate[7u * nn + i] = nu_bar;      // production
        istate[3u * nn + i] = jsel;        // fiss_nuc
        istate[2u * nn + i] = 0u;          // dead (terminal)
        istate[i] = seed.x; istate[nn + i] = seed.y;
        return;
    } else if (xi_r < x_abs) {
        // Radiative capture -> dead.
        istate[2u * nn + i] = 0u;
        istate[i] = seed.x; istate[nn + i] = seed.y;
        return;
    } else if (xi_r < x_abs + x_inel) {
        // Continuum inelastic down-scatter (Weisskopf).
        let sc = continuum_inelastic(e, u_in, awr_j, seed);
        seed = sc.seed;
        fstate[6u * nn + i] = sc.e;
        fstate[3u * nn + 3u * i + 0u] = sc.dir.x;
        fstate[3u * nn + 3u * i + 1u] = sc.dir.y;
        fstate[3u * nn + 3u * i + 2u] = sc.dir.z;
    } else {
        // Elastic. Below the CE/MG seam (or |mubar|<1e-4) -> isotropic-CM;
        // above -> exponential-mu forward scatter. Both draw one uniform for the
        // CM cosine, then rotate_direction draws the azimuth (see scatter.rs).
        var mu_cm: f32;
        var re = rng_next(seed); seed = re.xy;
        let xi_e = bitcast<f32>(re.z);
        if (e <= emax_j || abs(mubar) < 1.0e-4) {
            mu_cm = 2.0 * xi_e - 1.0;         // isotropic-CM
        } else {
            mu_cm = exponential_mu(mubar, xi_e); // forward-elastic
        }
        let sc = two_body_with_mu(e, u_in, awr_j, 0.0, mu_cm, seed);
        seed = sc.seed;
        fstate[6u * nn + i] = sc.e;
        fstate[3u * nn + 3u * i + 0u] = sc.dir.x;
        fstate[3u * nn + 3u * i + 1u] = sc.dir.y;
        fstate[3u * nn + 3u * i + 2u] = sc.dir.z;
    }

    // Survived (scattered): still alive for the next event.
    istate[i] = seed.x; istate[nn + i] = seed.y;
    atomicAdd(&ctrl, 1u);
}
