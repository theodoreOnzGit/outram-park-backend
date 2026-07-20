// xs_interp.wgsl — Batched pointwise cross-section interpolation on a shared energy grid.
//
// PHYSICS / WHAT THIS COMPUTES
// ----------------------------
// This is the inner cross-section lookup of a Monte Carlo neutron transport
// code. Given:
//   - an ascending energy grid  E[0..G]      in electron-volts (eV),
//   - a tabulated microscopic cross section  sigma[0..G]  in barns (b),
//     defined at those same grid points, and
//   - a batch of N independent query energies (eV),
// it produces N linearly-interpolated cross-section values (barn), one per
// query. Each query is fully independent, so this maps perfectly onto the GPU:
// one invocation handles one query energy.
//
// For each query energy q (eV) the kernel finds the grid bracket
//   grid[i_grid] <= q < grid[i_grid + 1],
// computes the linear interpolation factor
//   f = (q - grid[i_grid]) / (grid[i_grid + 1] - grid[i_grid]),
// and returns
//   sigma(q) = (1 - f) * sigma[i_grid] + f * sigma[i_grid + 1]   (barn).
//
// PROVENANCE
// ----------
// Ported from OpenMC src/nuclide.cpp:716-760 (Nuclide::calculate_xs grid-search
// + linear interpolation). The bracket selection (below-grid clamp to 0,
// above-grid clamp to n-2, otherwise binary search), the "two identical energy
// points" guard, and the (1 - f)/f linear blend all mirror those exact lines.
// OpenMC additionally uses a logarithmic union-grid mapping to shrink the
// binary-search range; that is a performance optimisation over the *same*
// bracket, so this kernel performs a plain binary search over the full grid and
// obtains the identical i_grid.
//
// PRECISION
// ---------
// Everything on the GPU is f32: WGSL has no f64. The authoritative CPU-side
// reference implementation is f64; this GPU kernel is an *acceleration only*
// and is only ever compared against the CPU result within a tolerance. Do not
// treat the f32 output as bit-exact with the f64 reference.
//
// STATUS
// ------
// This is UNTRUSTED, AI-DRAFTED scaffolding. The code is real and intended to
// be correct, but it must pass human review + unit testing against the f64 CPU
// reference before it is trusted, per the project's V&V policy.
//
// BINDING LAYOUT (all @group(0))
// ------------------------------
//   @binding(0) grid    : array<f32>, read-only storage  — energy grid (eV),  length = params.n_grid
//   @binding(1) sigma   : array<f32>, read-only storage  — cross section (barn) at each grid point, length = params.n_grid
//   @binding(2) queries : array<f32>, read-only storage  — query energies (eV), length = params.n_query
//   @binding(3) params  : uniform Params { n_grid, n_query, pad0, pad1 } (16 bytes)
//   @binding(4) result  : array<f32>, read-write storage — output cross section (barn), length = params.n_query
//
// Workgroup size: 64 (one invocation per query energy; global_invocation_id.x = query index).

struct Params {
    n_grid: u32,   // number of energy grid / sigma points (G)
    n_query: u32,  // number of query energies (N)
    pad0: u32,     // padding for 16-byte uniform alignment
    pad1: u32,     // padding for 16-byte uniform alignment
};

@group(0) @binding(0) var<storage, read>       grid:    array<f32>;
@group(0) @binding(1) var<storage, read>       sigma:   array<f32>;
@group(0) @binding(2) var<storage, read>       queries: array<f32>;
@group(0) @binding(3) var<uniform>             params:  Params;
@group(0) @binding(4) var<storage, read_write> result:  array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;

    // 1. Guard the tail of the final workgroup: invocations past the batch do nothing.
    if (i >= params.n_query) {
        return;
    }

    let q = queries[i];
    let n = params.n_grid;

    // Degenerate grid: with fewer than 2 points there is no bracket to interpolate.
    // Emit the single available value, or 0.0 barn if the grid is empty.
    if (n <= 1u) {
        if (n == 1u) {
            result[i] = sigma[0];
        } else {
            result[i] = 0.0;
        }
        return;
    }

    // 3. Determine the grid bracket index i_grid, mirroring OpenMC
    //    (src/nuclide.cpp:718-732).
    var i_grid: u32;
    if (q < grid[0]) {
        // Below the grid: clamp to the first interval (OpenMC: i_grid = 0).
        i_grid = 0u;
    } else if (q > grid[n - 1u]) {
        // Above the grid: clamp to the last interval (OpenMC: i_grid = size - 2).
        i_grid = n - 2u;
    } else {
        // In-range: lower_bound binary search for grid[i_grid] <= q < grid[i_grid+1].
        var lo: u32 = 0u;
        var hi: u32 = n - 1u;
        loop {
            if (hi - lo <= 1u) { break; }
            let mid = (lo + hi) >> 1u;
            if (grid[mid] <= q) { lo = mid; } else { hi = mid; }
        }
        i_grid = lo;
    }

    // 4. Interpolation factor with the OpenMC "two identical energy points"
    //    guard (src/nuclide.cpp:734-740). If the bracket has zero width the
    //    ratio is undefined, so pin f = 0 (take the lower point's value).
    let d = grid[i_grid + 1u] - grid[i_grid];
    var f: f32;
    if (d == 0.0) {
        f = 0.0;
    } else {
        f = (q - grid[i_grid]) / d;
    }

    // 5. Linear blend of the bracketing cross-section values (barn).
    result[i] = (1.0 - f) * sigma[i_grid] + f * sigma[i_grid + 1u];
}
