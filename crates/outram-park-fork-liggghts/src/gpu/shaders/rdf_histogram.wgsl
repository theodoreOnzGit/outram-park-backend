// SPDX-License-Identifier: GPL-3.0-only
// Pair-separation histogram for the radial distribution function.
//
// One invocation owns one shell centre and walks every pebble, binning the
// separations that fall inside r_max. Counts accumulate first into a
// workgroup-local histogram, so the contact peak -- which takes a large share
// of all hits and would otherwise serialise on one global atomic -- is
// contended only within a workgroup. Each workgroup then flushes its local
// histogram to global memory with one atomic per non-empty bin.
//
// PRECISION: WGSL has no f64, so positions and separations are f32 here while
// the CPU reference is f64. A pair whose separation lies within f32 rounding of
// a bin edge can therefore land in a neighbouring bin. This is measured, not
// assumed -- see `tests/rdf_backends.rs`.

struct Params {
    r_max_sq  : f32,
    inv_dr    : f32,
    n_bins    : u32,
    n_points  : u32,
    n_centres : u32,
    _pad0     : u32,
    _pad1     : u32,
    _pad2     : u32,
};

@group(0) @binding(0) var<uniform>             params     : Params;
// Positions as vec4 (xyz used, w padding): vec3 in a storage array has
// surprising stride rules, and vec4 keeps the 16-byte alignment explicit.
@group(0) @binding(1) var<storage, read>       positions  : array<vec4<f32>>;
@group(0) @binding(2) var<storage, read>       centre_idx : array<u32>;
@group(0) @binding(3) var<storage, read_write> histogram  : array<atomic<u32>>;

// Matches MAX_BINS in the Rust side; a workgroup histogram must be a
// compile-time size.
const MAX_BINS : u32 = 1024u;
const WG_SIZE  : u32 = 64u;

var<workgroup> local_hist : array<atomic<u32>, MAX_BINS>;

@compute @workgroup_size(WG_SIZE)
fn main(
    @builtin(global_invocation_id) gid : vec3<u32>,
    @builtin(local_invocation_id)  lid : vec3<u32>,
) {
    // Zero the workgroup histogram cooperatively.
    var b = lid.x;
    loop {
        if (b >= params.n_bins) { break; }
        atomicStore(&local_hist[b], 0u);
        b = b + WG_SIZE;
    }
    workgroupBarrier();

    let c = gid.x;
    if (c < params.n_centres) {
        let ci = centre_idx[c];
        let a  = positions[ci].xyz;
        for (var j : u32 = 0u; j < params.n_points; j = j + 1u) {
            if (j == ci) { continue; }
            let dvec = positions[j].xyz - a;
            let d2   = dot(dvec, dvec);
            if (d2 >= params.r_max_sq) { continue; }
            let bin = u32(sqrt(d2) * params.inv_dr);
            if (bin < params.n_bins) {
                atomicAdd(&local_hist[bin], 1u);
            }
        }
    }

    workgroupBarrier();

    // Flush: one global atomic per non-empty bin per workgroup.
    var k = lid.x;
    loop {
        if (k >= params.n_bins) { break; }
        let v = atomicLoad(&local_hist[k]);
        if (v != 0u) {
            atomicAdd(&histogram[k], v);
        }
        k = k + WG_SIZE;
    }
}
