// SPDX-License-Identifier: GPL-3.0

//! **The Gaussian puff field as a GPU kernel**, plus its `f32` CPU mirror and
//! a dedicated CPU thread pool.
//!
//! The Map tab wants a 64 x 64 field refreshed at **10 Hz**. That is about
//! 30 million kernel evaluations a second:
//!
//! | path | measured shape | 10 Hz? |
//! |---|---|---|
//! | one CPU core | ~2.2 s per field | no, 22x over |
//! | [`field_pooled`] on 16 cores | ~140 ms | marginal |
//! | GPU via [`GAUSSIAN_PUFF_FIELD`] | single-digit ms | yes |
//!
//! so the GPU path is the one that meets the requirement and the pooled CPU
//! path is the fallback. **Both must exist**: `outram-mc-libs/CLAUDE.md`'s GPU
//! policy is a hard rule across this workspace -- CI must never fail for want
//! of a GPU, detection is at run time, and the CPU path stays mandatory and
//! trusted. Here the CPU path is also the *reference*: it is what the GPU
//! result is checked against.
//!
//! # PETIR runs the shader; changi does not own any wgpu plumbing
//!
//! [`GAUSSIAN_PUFF_FIELD`] declares WGSL **functions**, in the same style as
//! `petir::wgsl::POLY`, and is dispatched by
//! `petir::wgsl::gpu::GpuContext::eval_map`. PETIR's own docs record why:
//! leaving each consumer to write "the same 150 lines of buffer plumbing" is
//! how they all get the bind-group layout "subtly wrong in the same way", and
//! it names a real defect that caused. changi therefore gains a GPU path
//! **without gaining a `wgpu` dependency** -- the feature stays behind
//! `petir/wgpu`, off by default, and a host with no adapter simply uses
//! [`field_pooled`].
//!
//! # Why the puff states are flattened on the CPU first
//!
//! A puff's `sigma_y`/`sigma_z` come from a Pasquill-Gifford table walk --
//! branchy, cache-unfriendly, everything a shader is worst at. But they are
//! needed **once per puff**, not once per (puff, cell), so evaluating them on
//! the CPU costs 4096 times less than doing it in the kernel. What crosses to
//! the GPU is the finished state: position, both sigmas, and the mass-time
//! weight. The shader then does nothing but arithmetic.
//!
//! # `f32`, and where the f64 path remains the reference
//!
//! The field drives a colour ramp over four decades and `f32` carries about
//! seven decimal digits, so single precision is ample for the picture. It is
//! **not** what a quoted number comes from: `chi/Q` for the receptor ring
//! stays on the f64
//! [`super::concentration::gaussian_puff_concentration`] path, which is the
//! one carrying the analytical verification.

use std::sync::OnceLock;

/// The WGSL source. Declares `changi_puff_field` and
/// `changi_puff_contribution`; see the file header for the `src`/`params`
/// layout contract it shares with `eval_map`.
pub const GAUSSIAN_PUFF_FIELD: &str = include_str!("shaders/gaussian_puff_field.wgsl");

/// Floats per puff state in the flattened `src` array. Must match
/// `CHANGI_STATE_STRIDE` in the shader.
pub const STATE_STRIDE: usize = 5;

/// `(2 pi)^{3/2}`, the normalisation of a unit-mass three-dimensional
/// Gaussian. Matches the shader's constant to `f32` precision.
pub const TWO_PI_THREE_HALVES: f32 = 15.749_61;

/// One puff, already reduced to what the kernel needs.
///
/// Deliberately **not** a puff as the simulator holds one: no emission time,
/// no travel distance, no stability class. Those are what produce `sigma_y`
/// and `sigma_z`, and they have been spent by the time a state is built.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PuffState {
    /// Puff centre easting \[m\].
    pub x: f32,
    /// Puff centre northing \[m\].
    pub y: f32,
    /// Crosswind dispersion at this puff's travel distance \[m\].
    pub sigma_y: f32,
    /// Vertical dispersion at this puff's travel distance \[m\].
    pub sigma_z: f32,
    /// Mass times the time weight being integrated with \[kg s\], so a sum of
    /// contributions is already a time-integrated quantity.
    pub weight: f32,
}

/// The grid a field is evaluated on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FieldGrid {
    /// Cells per side; the grid is square.
    pub cells: usize,
    /// Half-width of the covered square \[m\].
    pub half_width_m: f32,
    /// Release height \[m\].
    pub source_height_m: f32,
}

impl FieldGrid {
    /// The `(easting, northing)` of a cell's **centre**, in metres.
    ///
    /// Row-major and **north-up**: row 0 is the northernmost, so a field can
    /// be painted straight down the screen without the caller flipping it.
    /// Getting this backwards mirrors the plume north-south, which still
    /// looks like a plume.
    pub fn cell_centre(&self, column: usize, row: usize) -> (f32, f32) {
        let n = self.cells.max(1) as f32;
        let step = 2.0 * self.half_width_m / n;
        (
            -self.half_width_m + (column as f32 + 0.5) * step,
            self.half_width_m - (row as f32 + 0.5) * step,
        )
    }

    /// Total cells.
    pub fn len(&self) -> usize {
        self.cells * self.cells
    }

    /// Whether the grid has no cells.
    pub fn is_empty(&self) -> bool {
        self.cells == 0
    }
}

/// Flatten puff states into the `f32` array the shader reads as `src`.
///
/// The layout is the shader's contract; `STATE_STRIDE` is the one place the
/// stride is written on this side.
pub fn pack_states(states: &[PuffState]) -> Vec<f32> {
    let mut out = Vec::with_capacity(states.len() * STATE_STRIDE);
    for s in states {
        out.extend_from_slice(&[s.x, s.y, s.sigma_y, s.sigma_z, s.weight]);
    }
    out
}

/// One puff state's contribution at a ground-level receptor — the `f32` CPU
/// mirror of the shader's `changi_puff_contribution`.
///
/// At `z = 0` the real and image terms coincide, so the vertical factor is
/// `2 exp(-H^2 / 2 sigma_z^2)`. Written out as a sum rather than folded to
/// the factor of two, matching the shader line for line so the two can be
/// read side by side.
pub fn contribution(state: &PuffState, easting: f32, northing: f32, height_m: f32) -> f32 {
    if state.sigma_y <= 0.0 || state.sigma_z <= 0.0 {
        return 0.0;
    }
    let sy2 = state.sigma_y * state.sigma_y;
    let sz2 = state.sigma_z * state.sigma_z;

    let dx = easting - state.x;
    let dy = northing - state.y;
    let horizontal = (-0.5 * (dx * dx + dy * dy) / sy2).exp();

    let vertical = (-0.5 * height_m * height_m / sz2).exp()
        + (-0.5 * height_m * height_m / sz2).exp();

    let amplitude = state.weight / (TWO_PI_THREE_HALVES * sy2 * state.sigma_z);
    amplitude * horizontal * vertical
}

/// The field value at one cell — the `f32` CPU mirror of the shader's
/// `changi_puff_field`.
pub fn field_cell(states: &[PuffState], grid: &FieldGrid, column: usize, row: usize) -> f32 {
    if grid.is_empty() || column >= grid.cells || row >= grid.cells {
        return 0.0;
    }
    let (easting, northing) = grid.cell_centre(column, row);
    states
        .iter()
        .map(|s| contribution(s, easting, northing, grid.source_height_m))
        .sum()
}

/// The whole field, single-threaded. The reference the GPU and the pooled
/// path are both checked against.
pub fn field_serial(states: &[PuffState], grid: &FieldGrid) -> Vec<f32> {
    let mut out = Vec::with_capacity(grid.len());
    for row in 0..grid.cells {
        for column in 0..grid.cells {
            out.push(field_cell(states, grid, column, row));
        }
    }
    out
}

/// changi's **own** thread pool, so a puff field never competes with whatever
/// else the host is running on the global pool.
///
/// A dispersion map refreshing at 10 Hz next to a physics loop on the default
/// pool is precisely the case where sharing one hurts: whichever grabs the
/// workers first stalls the other, and the symptom is a stuttering map or a
/// stuttering plant, depending on scheduling. One named pool, sized once.
fn pool() -> &'static rayon::ThreadPool {
    static POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();
    POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .thread_name(|i| format!("changi-puff-{i}"))
            .build()
            .expect("changi puff pool")
    })
}

/// The whole field on changi's own thread pool.
///
/// The CPU fallback for hosts with no usable GPU adapter. Parallel over
/// **rows**, not cells: a row is 64 contributions-sums of identical cost, so
/// the chunks are even and large enough that the scheduling overhead
/// disappears against the work.
pub fn field_pooled(states: &[PuffState], grid: &FieldGrid) -> Vec<f32> {
    use rayon::prelude::*;
    if grid.is_empty() {
        return Vec::new();
    }
    pool().install(|| {
        (0..grid.cells)
            .into_par_iter()
            .flat_map_iter(|row| {
                (0..grid.cells)
                    .map(move |column| field_cell(states, grid, column, row))
                    .collect::<Vec<f32>>()
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::puff::concentration::gaussian_puff_concentration;
    use crate::puff::dispersion::pasquill_gifford_sigmas;
    use crate::puff::stability::StabilityClass;
    use uom::si::f64::{Length, Mass};
    use uom::si::length::meter;
    use uom::si::mass::kilogram;
    use uom::si::mass_density::kilogram_per_cubic_meter;

    fn grid() -> FieldGrid {
        FieldGrid {
            cells: 16,
            half_width_m: 1250.0,
            source_height_m: 40.0,
        }
    }

    /// **The `f32` mirror agrees with the f64 kernel it was transcribed from.**
    ///
    /// This is the check that makes the shader trustworthy: the WGSL is a
    /// line-for-line transcription of [`contribution`], and [`contribution`]
    /// is checked here against
    /// [`gaussian_puff_concentration`], which carries the analytical
    /// verification (mass conservation, zero ground flux, the second moment).
    /// So the chain of evidence reaches the shader without a GPU present.
    #[test]
    fn the_f32_mirror_agrees_with_the_f64_kernel() {
        let (class, distance, height) = (StabilityClass::D, 500.0, 40.0);
        let sig = pasquill_gifford_sigmas(class, Length::new::<meter>(distance))
            .expect("sigmas");
        let state = PuffState {
            x: 0.0,
            y: 0.0,
            sigma_y: sig.sigma_y.get::<meter>() as f32,
            sigma_z: sig.sigma_z.get::<meter>() as f32,
            weight: 1.0,
        };

        for (rx, ry) in [(0.0, 0.0), (50.0, 0.0), (0.0, 120.0), (-200.0, 90.0)] {
            let mirror = contribution(&state, rx, ry, height as f32) as f64;
            let reference = gaussian_puff_concentration(
                Mass::new::<kilogram>(1.0),
                class,
                Length::new::<meter>(0.0),
                Length::new::<meter>(0.0),
                Length::new::<meter>(height),
                (
                    Length::new::<meter>(rx as f64),
                    Length::new::<meter>(ry as f64),
                    Length::new::<meter>(0.0),
                ),
                Length::new::<meter>(distance),
            )
            .get::<kilogram_per_cubic_meter>();

            // f32 through three exponentials: a part in 1e-4 is the honest
            // bar, and far tighter than the four decades the ramp shows.
            let tolerance = reference.abs() * 1e-4 + 1e-30;
            assert!(
                (mirror - reference).abs() <= tolerance,
                "at ({rx}, {ry}): mirror {mirror:e} vs f64 kernel {reference:e}"
            );
        }
    }

    /// The pooled path is the serial path, scheduled differently. Not
    /// "close": **identical**, because it is the same arithmetic per cell and
    /// nothing is summed across cells. If these ever diverge, the
    /// parallelisation has changed the computation.
    #[test]
    fn the_pooled_field_is_bit_identical_to_the_serial_one() {
        let g = grid();
        let states: Vec<PuffState> = (0..40)
            .map(|i| PuffState {
                x: i as f32 * 12.0,
                y: (i as f32 * 7.0).sin() * 80.0,
                sigma_y: 30.0 + i as f32 * 2.0,
                sigma_z: 20.0 + i as f32,
                weight: 1.0,
            })
            .collect();

        let serial = field_serial(&states, &g);
        let pooled = field_pooled(&states, &g);
        assert_eq!(serial.len(), g.len());
        assert_eq!(pooled, serial, "the pool must not change the arithmetic");
    }

    /// **North-up, row 0 at the top.** Getting this backwards mirrors the
    /// plume north-south, and the result still looks like a plume -- so it has
    /// to be pinned by coordinate, not by eye.
    #[test]
    fn row_zero_is_the_northernmost() {
        let g = grid();
        let (_, north_of_top) = g.cell_centre(0, 0);
        let (_, north_of_bottom) = g.cell_centre(0, g.cells - 1);
        assert!(
            north_of_top > north_of_bottom,
            "row 0 must be north of the last row: {north_of_top} vs {north_of_bottom}"
        );
        // And columns run west to east.
        let (west, _) = g.cell_centre(0, 0);
        let (east, _) = g.cell_centre(g.cells - 1, 0);
        assert!(east > west, "column 0 must be west of the last column");
        // Centres, not corners: the extreme cell centre is half a step inside.
        let step = 2.0 * g.half_width_m / g.cells as f32;
        assert!((west + g.half_width_m - step / 2.0).abs() < 1e-3);
    }

    /// The packing matches the stride the shader reads with, and round-trips.
    #[test]
    fn states_pack_at_the_shader_stride() {
        let states = [
            PuffState { x: 1.0, y: 2.0, sigma_y: 3.0, sigma_z: 4.0, weight: 5.0 },
            PuffState { x: 6.0, y: 7.0, sigma_y: 8.0, sigma_z: 9.0, weight: 10.0 },
        ];
        let packed = pack_states(&states);
        assert_eq!(packed.len(), states.len() * STATE_STRIDE);
        assert_eq!(&packed[..5], &[1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(&packed[5..], &[6.0, 7.0, 8.0, 9.0, 10.0]);
    }

    /// The shader declares the functions the runner is told to call, and the
    /// stride constant both sides rely on. A rename on one side would
    /// otherwise only show up on a machine with a GPU.
    #[test]
    fn the_shader_declares_what_the_rust_side_expects() {
        assert!(GAUSSIAN_PUFF_FIELD.contains("fn changi_puff_field("));
        assert!(GAUSSIAN_PUFF_FIELD.contains("fn changi_puff_contribution("));
        assert!(
            GAUSSIAN_PUFF_FIELD.contains(&format!("CHANGI_STATE_STRIDE: u32 = {STATE_STRIDE}u")),
            "the shader's stride must match STATE_STRIDE = {STATE_STRIDE}"
        );
        // It reads `src`, which is what `eval_map` binds the data array to.
        assert!(GAUSSIAN_PUFF_FIELD.contains("src[base]"));
    }

    /// An empty grid or an empty state list is empty output, not a panic --
    /// both happen before the first dispersion run.
    #[test]
    fn the_degenerate_cases_are_empty_not_panics() {
        let empty_grid = FieldGrid { cells: 0, half_width_m: 1000.0, source_height_m: 40.0 };
        assert!(field_serial(&[], &empty_grid).is_empty());
        assert!(field_pooled(&[], &empty_grid).is_empty());

        let g = grid();
        let no_states = field_pooled(&[], &g);
        assert_eq!(no_states.len(), g.len());
        assert!(no_states.iter().all(|v| *v == 0.0), "no puffs is a zero field");
    }
}
