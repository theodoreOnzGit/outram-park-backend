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
// The CPU reference is `petir::rng::lcg` (`future_seed(1, seed)` advances one
// step), re-exported as `outram_mc_libs::rng::lcg`.
//
// WGSL has NO u64 and NO f64, so the 64-bit multiply-add is emulated with u32
// pairs (16-bit schoolbook for exact carries) so the *integer state advance is
// BIT-EXACT* vs the CPU LCG: the returned (rng_hi, rng_lo) equal CPU
// `future_seed(1, seed)` for every particle. This is the reproducibility linchpin.
//
// WHERE THE LCG CODE LIVES (2026-10-02). ~~This file carried its own
// `lcg_advance`, `mul64_low`, `mul_u32_full` and MULT/INC constants.~~ MOVED to
// PETIR: `petir::wgsl::LCG` (`crates/petir/src/wgsl/shaders/lcg.wgsl`), the ONE
// copy in the workspace, which `batched_event.wgsl` also uses. This file is no
// longer a complete shader on its own: `batched_flight::shader_source()`
// concatenates `petir::wgsl::LCG` ahead of it, and this kernel calls
// `petir_lcg_next(vec2(lo, hi)) -> vec3(new_lo, new_hi, bitcast(xi))`. The
// arithmetic is byte-for-byte what was here; the GPU bit-exactness gates
// (`tests/gpu_lcg_advance_directly.rs`) run on the composed source.
//
// The uniform VALUE used for the flight is NOT the CPU f64 `prn` value (that is
// impossible to match in f32). Instead it is derived from the TOP 24 bits of the
// advanced 64-bit state:
//   xi = f32(state_hi >> 8) * (1.0 / 16777216.0)      // top 24 bits -> [0,1)
// state_hi >> 8 is < 2^24, so f32 represents it exactly.
//
// DOCUMENTED DIVERGENCE. The integer state stream is bit-exact vs the CPU
// (gpu_lcg_advance_directly.rs asserts it as equality, over 256 seeds and over
// 4096-step chains). The uniform VALUE is not, and ~~this is the accepted f32
// acceleration divergence~~ **CORRECTED 2026-09-19** — it is STRUCTURAL, not a
// precision effect, and calling it an f32 cost understated it by seven orders
// of magnitude:
//
//   CPU  petir::rng::lcg::prn (moved from src/rng/lcg.rs:116-117 on
//        2026-10-02) applies a PCG-RXS-M-XS output PERMUTATION to
//        the advanced state, then scales the permuted word by 2^-64.
//   GPU  applies NO permutation and scales the raw top 24 bits by 2^-24.
//
// These are different functions of the same integer and would disagree in
// exact arithmetic. Measured over 1e6 consecutive draws from seed 1, the worst
// gap is 9.995e-01 — effectively two unrelated uniforms.
//
// THE BEHAVIOUR IS STILL SOUND, and that is measured, not assumed. An LCG's
// high bits are its good ones; the permutation exists because OpenMC wanted
// quality across ALL bits. Over the same 1e6 draws the raw top-24 stream has
// mean 4.9977e-01, chi-square 44.67 on 63 dof over 64 equal buckets (5 %
// critical value 82.5), and covariance -6.6e-05 against the CPU stream, inside
// one standard error of zero. Re-measured by
// `the_reference_is_sound_without_a_gpu`.
//
// The CPU single-thread path stays the trusted, bit-reproducible reference.
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

// A sentinel "no intersection" distance (cm). Physically the sampled flight
// distance in this kernel is O(10) cm, so this is always larger than any d_col.
const BIG: f32 = 1e30;
// Coincident-surface epsilon (cm), f32; matches the task spec / OpenMC treatment.
const EPS: f32 = 1e-7;

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
    //    `petir_lcg_next` comes from PETIR's `lcg.wgsl`, concatenated ahead of
    //    this file at pipeline creation (see the header).
    let adv = petir_lcg_next(vec2<u32>(state[nn + i], state[i]));
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
