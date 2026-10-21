// SPDX-License-Identifier: GPL-3.0
//
// The Gaussian puff concentration kernel, in WGSL (f32).
//
// TRANSCRIBED from `changi::puff::concentration::gaussian_puff_concentration`,
// which is itself a port of the R `puff` package's `gpuff`. This shader and
// that function must agree; `super::wgsl::field_cell` is the f32 CPU mirror
// that lets them be compared with no GPU present, and the tests assert the
// mirror against the f64 kernel.
//
// WRITTEN TO PETIR'S CONVENTION, and dispatched by PETIR'S RUNNER
//
// This file declares FUNCTIONS, not an entry point, exactly as
// `petir::wgsl::POLY` and friends do. `petir::wgsl::gpu::GpuContext::eval_map`
// supplies the entry point, the bind-group layout and the buffer plumbing.
// That is deliberate reuse: PETIR's own docs record that leaving every
// consumer to write "the same 150 lines of buffer plumbing" is how they each
// get the layout "subtly wrong in the same way", and it names a real defect
// that caused. changi gains a GPU path without gaining a wgpu dependency.
//
// LAYOUT CONTRACT with `eval_map`
//
//   src      flattened puff states, STATE_STRIDE floats each:
//              [0] x        puff centre easting  [m]
//              [1] y        puff centre northing [m]
//              [2] sigma_y  crosswind dispersion [m]
//              [3] sigma_z  vertical dispersion  [m]
//              [4] weight   mass x time weight   [kg s]
//   x        the cell's linear index, as f32 (probe value)
//   params.m state count
//   params.k cells per side
//   params.a half-width of the covered square [m]
//   params.b release height [m]
//
// GROUND LEVEL ONLY. The field is evaluated at z = 0, which is what
// deposition and inhalation see and what the map paints. An elevated slice
// would need a receptor height and is not what this is for.
//
// WHAT IS DELIBERATELY NOT HERE
//
// No Pasquill-Gifford lookup. `sigma_y`/`sigma_z` arrive per puff state,
// computed on the CPU, because that lookup is a branchy table walk -- what a
// shader is worst at -- and it is evaluated once per puff rather than once per
// (puff, cell), so it is thousands of times cheaper there anyway.
//
// f32, AND WHY THAT IS ENOUGH HERE
//
// This drives a colour ramp over four decades; f32 carries about seven
// decimal digits. The f64 CPU path stays the reference and is what any
// QUOTED number comes from. This is for the picture.

// Floats per puff state in `src`.
const CHANGI_STATE_STRIDE: u32 = 5u;

// (2 pi)^{3/2}, the normalisation of a unit-mass 3-D Gaussian.
const CHANGI_TWO_PI_THREE_HALVES: f32 = 15.749609945722419;

// One puff state's contribution at a ground-level receptor.
//
// Mirrors `gaussian_puff_concentration` term for term, including the
// ground-reflection image at -H. At z = 0 the real and image terms coincide,
// so the vertical factor is 2 exp(-H^2 / 2 sigma_z^2) -- written out rather
// than folded to the factor of two, so the correspondence with the CPU kernel
// stays readable.
fn changi_puff_contribution(
    px: f32, py: f32, sigma_y: f32, sigma_z: f32, weight: f32,
    rx: f32, ry: f32, h: f32,
) -> f32 {
    // A degenerate sigma is upstream's NA path, mapped to zero. Guarded
    // because a shader has no NaN reporting and one NaN would poison the
    // whole cell.
    if (sigma_y <= 0.0 || sigma_z <= 0.0) {
        return 0.0;
    }
    let sy2 = sigma_y * sigma_y;
    let sz2 = sigma_z * sigma_z;

    let dx = rx - px;
    let dy = ry - py;
    let horizontal = exp(-0.5 * (dx * dx + dy * dy) / sy2);

    // Receptor at z = 0: (0 - h) and (0 + h) are equal in magnitude.
    let vertical = exp(-0.5 * h * h / sz2) + exp(-0.5 * h * h / sz2);

    let amplitude = weight / (CHANGI_TWO_PI_THREE_HALVES * sy2 * sigma_z);
    return amplitude * horizontal * vertical;
}

// The field value at one grid cell: the sum over every puff state.
//
// `cell` is the linear index as f32, row-major and NORTH-UP -- row 0 is the
// northernmost, so the result can be painted straight down the screen without
// the caller flipping it.
fn changi_puff_field(cell: f32, cells: u32, state_count: u32, half_width: f32, h: f32) -> f32 {
    if (cells == 0u) {
        return 0.0;
    }
    let index = u32(cell);
    let column = index % cells;
    let row = index / cells;
    if (row >= cells) {
        return 0.0;
    }

    let n = f32(cells);
    let step = 2.0 * half_width / n;
    // Cell CENTRES, not corners: the value is the concentration in the
    // middle of the cell rather than on its edge.
    let easting = -half_width + (f32(column) + 0.5) * step;
    let northing = half_width - (f32(row) + 0.5) * step;

    var total: f32 = 0.0;
    for (var i: u32 = 0u; i < state_count; i = i + 1u) {
        let base = i * CHANGI_STATE_STRIDE;
        total = total + changi_puff_contribution(
            src[base], src[base + 1u], src[base + 2u], src[base + 3u], src[base + 4u],
            easting, northing, h,
        );
    }
    return total;
}
