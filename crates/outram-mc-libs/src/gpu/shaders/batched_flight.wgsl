// batched_flight.wgsl — One next-event free flight for a resident batch of neutrons.
//
// PHYSICS / WHAT THIS COMPUTES
// ----------------------------
// This is the hot, regular, per-event neutron flight of a Monte Carlo transport
// code, run for a whole BATCH of live neutrons in parallel (one GPU invocation
// per particle). Each dispatch advances every live particle through EXACTLY ONE
// flight (one event):
//   1. draw a uniform random number xi in [0,1) from the per-particle 64-bit LCG,
//   2. look up the macroscopic total cross section  Sigma_t (cm^-1) at the
//      particle energy via binary search + linear interpolation on a shared
//      union energy grid,
//   3. sample the distance to the next collision  d_col = -ln(xi) / Sigma_t  (cm),
//   4. compute the distance to the bounding sphere  d_bound  (cm),
//   5. stream to whichever is nearer and flag the outcome:
//        collided  (d_col <  d_bound) — particle moved to the collision site,
//        leaked    (d_col >= d_bound) — particle left the sphere (dead here).
// The branchy collision physics (which nuclide/reaction, secondary E/angle) is
// NOT done here — it stays on the CPU caller. This kernel only does the flight.
//
// BUFFER PACKING (why arrays are concatenated)
// --------------------------------------------
// `wgpu`'s downlevel default limit is only 4 storage buffers per compute stage,
// so the eight logical SoA arrays are packed into 4 storage buffers plus the
// uniform. For a batch of N = params.n_particle particles and G = params.n_grid
// grid points:
//   xs      (read)       : grid[0..G]  ++ sigma[0..G]            (f32, 2G)
//   part_in (read)       : energy[0..N] ++ dir[0..3N]            (f32, 4N)
//   pos     (read_write) : x,y,z per particle                   (f32, 3N)
//   state   (read_write) : rng_hi[0..N] ++ rng_lo[0..N] ++ outcome[0..N] (u32, 3N)
// Accessors:
//   grid(j)   = xs[j]                 sigma(j)  = xs[G + j]
//   energy(i) = part_in[i]            dir(i,c)  = part_in[N + 3i + c]
//   rng_hi(i) = state[i]              rng_lo(i) = state[N + i]
//   outcome(i)= state[2N + i]  (WRITE: 0 = leaked/dead, 1 = collided)
// pos is READ + WRITE (updated to the collision site on collide); dir/energy/xs
// are READ only.
//
// THE RNG — OpenMC 64-bit LCG, STATE ADVANCE ported bit-exactly
// ------------------------------------------------------------
// OpenMC's LCG (src/random_lcg.cpp:32-35, prn_mult/prn_add on lines 11-12):
//   seed_{n+1} = (MULT * seed_n + INC) mod 2^64
//   MULT = 6364136223846793005 = 0x5851F42D4C957F2D
//   INC  = 1442695040888963407 = 0x14057B7EF767814F
// The CPU reference is `src/rng/lcg.rs` (`future_seed(1, seed)` advances one step).
//
// WGSL has NO u64 and NO f64, so the 64-bit multiply-add is emulated with u32
// pairs (16-bit schoolbook for exact carries) so the *integer state advance is
// BIT-EXACT* vs the CPU LCG: the returned (rng_hi, rng_lo) equal CPU
// `future_seed(1, seed)` for every particle. This is the reproducibility linchpin.
//
// The uniform VALUE used for the flight is NOT the CPU f64 `prn` value (that is
// impossible to match in f32). Instead it is derived from the TOP 24 bits of the
// advanced 64-bit state:
//   xi = f32(state_hi >> 8) * (1.0 / 16777216.0)      // top 24 bits -> [0,1)
// state_hi >> 8 is < 2^24, so f32 represents it exactly. DOCUMENTED DIVERGENCE:
// the integer state stream is bit-exact vs the CPU; the f32 xi VALUE differs from
// the CPU f64 `prn` — the accepted f32 acceleration divergence. The CPU single-
// thread path stays the trusted, bit-reproducible reference.
//
// PROVENANCE
// ----------
// - RNG state advance: OpenMC src/random_lcg.cpp:32-35 (prn), constants 11-12.
// - Sigma_t grid search + linear interp: OpenMC src/nuclide.cpp:716-740
//   (Nuclide::calculate_xs), mirrored exactly as in this crate's xs_interp.wgsl.
// - Bounding-sphere distance: OpenMC src/surface.cpp:607-638
//   (SurfaceSphere::distance), the coincident=false path; also this crate's
//   `src/geometry/surface.rs` Sphere::distance.
//
// PRECISION / STATUS
// ------------------
// Everything on the GPU is f32; the authoritative reference is the raw-f64 CPU
// transport loop, with `advance_flight_cpu_mirror` providing a same-f32-path
// bit-level reference for THIS kernel's logic. UNTRUSTED, AI-DRAFTED: must pass
// the V&V gate (LCG-state bit-exactness + GPU-vs-mirror agreement) and human
// review before it is trusted, per the project's V&V policy.
//
// BINDING LAYOUT (all @group(0)) — see BUFFER PACKING above for element layout.
//   @binding(0) xs      : array<f32> read       — grid ++ sigma
//   @binding(1) part_in : array<f32> read       — energy ++ dir
//   @binding(2) params  : uniform Params
//   @binding(3) pos     : array<f32> read_write — positions (cm, 3N)
//   @binding(4) state   : array<u32> read_write — rng_hi ++ rng_lo ++ outcome
//
// Workgroup size: 64 (one invocation per particle; global_invocation_id.x = index).

struct Params {
    n_grid: u32,      // number of energy grid / sigma points (G)
    n_particle: u32,  // number of particles in the batch (N)
    pad0: u32,        // padding for 16-byte uniform alignment
    pad1: u32,        // padding for 16-byte uniform alignment
    sphere: vec4<f32>,// bounding sphere: (center_x, center_y, center_z, radius) cm
};

@group(0) @binding(0) var<storage, read>       xs:      array<f32>;
@group(0) @binding(1) var<storage, read>       part_in: array<f32>;
@group(0) @binding(2) var<uniform>             params:  Params;
@group(0) @binding(3) var<storage, read_write> pos:     array<f32>;
@group(0) @binding(4) var<storage, read_write> state:   array<u32>;

// LCG constants split into 32-bit halves (see header).
const MULT_HI: u32 = 0x5851F42Du;
const MULT_LO: u32 = 0x4C957F2Du;
const INC_HI:  u32 = 0x14057B7Eu;
const INC_LO:  u32 = 0xF767814Fu;

// A sentinel "no intersection" distance (cm). Physically the sampled flight
// distance in this kernel is O(10) cm, so this is always larger than any d_col.
const BIG: f32 = 1e30;
// Coincident-surface epsilon (cm), f32; matches the task spec / OpenMC treatment.
const EPS: f32 = 1e-7;

// Full 32x32 -> 64-bit unsigned product via 16-bit schoolbook decomposition, so
// all carries are exact. Returns vec2(lo32, hi32).
fn mul_u32_full(x: u32, y: u32) -> vec2<u32> {
    let x0 = x & 0xFFFFu;
    let x1 = x >> 16u;
    let y0 = y & 0xFFFFu;
    let y1 = y >> 16u;
    let t0 = x0 * y0;               // < 2^32
    let s  = x0 * y1;               // < 2^32
    let t1 = s + x1 * y0;           // wraps in u32; carry captured below
    let carry1 = select(0u, 1u, t1 < s); // bit 32 of the true (s + x1*y0)
    let t2 = x1 * y1;               // < 2^32
    let lo_lo16 = t0 & 0xFFFFu;
    let mid = (t0 >> 16u) + (t1 & 0xFFFFu); // < 2^17
    let lo = lo_lo16 | ((mid & 0xFFFFu) << 16u);
    let carry_mid = mid >> 16u;     // 0 or 1
    let hi = t2 + (t1 >> 16u) + carry_mid + (carry1 << 16u);
    return vec2<u32>(lo, hi);
}

// Low 64 bits of the 64x64 product (a_hi:a_lo) * (b_hi:b_lo). Only the low 64
// bits are needed for the mod-2^64 LCG advance.
fn mul64_low(a_lo: u32, a_hi: u32, b_lo: u32, b_hi: u32) -> vec2<u32> {
    let p = mul_u32_full(a_lo, b_lo);
    // (a_lo*b_hi + a_hi*b_lo) contributes only its low 32 bits (it is scaled by
    // 2^32); u32 multiply/add already wraps to those low 32 bits.
    let cross = a_lo * b_hi + a_hi * b_lo;
    let res_lo = p.x;
    let res_hi = p.y + cross;       // wraps in u32 = mod 2^32
    return vec2<u32>(res_lo, res_hi);
}

// Advance the split LCG seed (s_hi:s_lo) by ONE step and return the new state
// plus the derived f32 uniform. Returns vec3(new_lo, new_hi, bitcast<u32>(xi)).
// (xi is bitcast into the .z lane to keep a single return value.)
fn lcg_advance(s_lo: u32, s_hi: u32) -> vec3<u32> {
    let m = mul64_low(s_lo, s_hi, MULT_LO, MULT_HI);
    let new_lo = m.x + INC_LO;
    let carry = select(0u, 1u, new_lo < INC_LO); // add carry into the high word
    let new_hi = m.y + INC_HI + carry;
    // Uniform from the top 24 bits of the 64-bit state (state_hi >> 8) in [0,1).
    let top24 = new_hi >> 8u;
    let xi = f32(top24) * (1.0 / 16777216.0);
    return vec3<u32>(new_lo, new_hi, bitcast<u32>(xi));
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    let nn = params.n_particle;

    // 1. Tail guard: only live particles i < N are uploaded; the rest do nothing.
    if (i >= nn) {
        return;
    }

    // 2. Advance the RNG one step; write the new split seed back regardless of
    //    outcome (every flight consumes exactly one LCG step).
    //    state layout: rng_hi[0..N] ++ rng_lo[0..N] ++ outcome[0..N].
    let adv = lcg_advance(state[nn + i], state[i]);
    state[i] = adv.y;               // rng_hi
    state[nn + i] = adv.x;          // rng_lo
    let xi = bitcast<f32>(adv.z);

    // 3. Sigma_t at energy[i] via binary search + linear interpolation on the
    //    shared union grid (mirrors OpenMC src/nuclide.cpp:716-740, as in
    //    xs_interp.wgsl). xs layout: grid[0..G] ++ sigma[0..G].
    let e = part_in[i];             // energy[i]
    let n = params.n_grid;
    var i_grid: u32;
    if (e < xs[0]) {
        i_grid = 0u;                    // below grid: clamp to first interval
    } else if (e > xs[n - 1u]) {
        i_grid = n - 2u;                // above grid: clamp to last interval
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
    if (d == 0.0) {
        f = 0.0;
    } else {
        f = (e - xs[i_grid]) / d;
    }
    let sigma_t = (1.0 - f) * xs[n + i_grid] + f * xs[n + i_grid + 1u];

    // 4. Non-positive Sigma_t: no collision possible -> leaked (dead).
    if (sigma_t <= 0.0) {
        state[2u * nn + i] = 0u;       // outcome = leaked
        return;
    }

    // 5. Distance to collision (cm). xi == 0 -> log(0) = -inf -> d_col = +inf,
    //    which is >= d_bound below, i.e. treated as a leak.
    let d_col = -log(xi) / sigma_t;

    // 6. Distance to the bounding sphere (cm). Mirrors OpenMC
    //    src/surface.cpp:607-638 SurfaceSphere::distance (coincident = false).
    //    part_in dir layout: dir(i,c) = part_in[N + 3i + c].
    let px = pos[3u * i + 0u];
    let py = pos[3u * i + 1u];
    let pz = pos[3u * i + 2u];
    let ux = part_in[nn + 3u * i + 0u];
    let uy = part_in[nn + 3u * i + 1u];
    let uz = part_in[nn + 3u * i + 2u];
    let ox = px - params.sphere.x;
    let oy = py - params.sphere.y;
    let oz = pz - params.sphere.z;
    let rad = params.sphere.w;
    let k = ox * ux + oy * uy + oz * uz;      // o . u
    let c = ox * ox + oy * oy + oz * oz - rad * rad;
    let disc = k * k - c;
    var d_bound: f32;
    if (disc < 0.0) {
        d_bound = BIG;                         // ray misses the sphere
    } else {
        let sq = sqrt(disc);
        let d_near = -k - sq;
        if (d_near > EPS) {
            d_bound = d_near;
        } else {
            let d_far = -k + sq;
            if (d_far > EPS) {
                d_bound = d_far;
            } else {
                d_bound = BIG;                 // both roots behind the particle
            }
        }
    }

    // 7. Stream to the nearer event.
    if (d_col >= d_bound) {
        state[2u * nn + i] = 0u;               // leaked; leave pos unchanged
        return;
    }
    // 8. Collided: advance position to the collision site.
    pos[3u * i + 0u] = px + ux * d_col;
    pos[3u * i + 1u] = py + uy * d_col;
    pos[3u * i + 2u] = pz + uz * d_col;
    state[2u * nn + i] = 1u;                    // collided
}
