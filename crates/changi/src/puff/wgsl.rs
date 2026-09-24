// SPDX-License-Identifier: GPL-3.0

//! **The Gaussian puff field as a GPU kernel**, plus its `f32` CPU mirror and
//! a dedicated CPU thread pool.
//!
//! The Map tab wants a 64 x 64 field refreshed at **10 Hz**. At
//! `htgr_sim_v1`'s own working point -- 7 260 puffs, from emitting every 10 s
//! over a 1 200 s run -- that is ~30 million kernel evaluations per field.
//!
//! **Measured 2026-09-24**, 16 logical cores, one adapter present, by
//! `cargo run --release -p changi --example field_timing [--features gpu]`
//! (median of 5 after a warm-up, the simulator's own 64x64 grid over
//! +/-1250 m at 50 m):
//!
//! | path | 1 000 puffs | **7 260 puffs** | 20 000 puffs | 50 000 puffs |
//! |---|---|---|---|---|
//! | [`field_serial`], one core | 18.7 ms | **136 ms** | 369 ms | 890 ms |
//! | [`field_pooled`], 16 cores | 2.3 ms | **15.9 ms** | 43.0 ms | 104 ms |
//! | [`field_gpu`] | 0.57 ms | **1.98 ms** | 12.8 ms | 11.6 ms |
//!
//! So at the working point the pooled CPU path takes **~16 ms**, comfortably
//! inside both 10 Hz (100 ms) and `htgr_sim_v1`'s `PHYSICS_TICK`. The GPU is
//! ~8x faster again and is what one would reach for at 20 000+ puffs, but it
//! is **not** required to meet the 10 Hz target.
//!
//! > ~~"one CPU core ~2.2 s per field; [`field_pooled`] on 16 cores ~140 ms,
//! > marginal; GPU single-digit ms -- so the GPU path is the one that meets
//! > the requirement and the pooled CPU path is the fallback."~~
//! > **CORRECTED 2026-09-24.** The pooled row was wrong by ~9x: 140 ms is
//! > close to the *serial* cost at the working point (136 ms), not the pooled
//! > one (15.9 ms), so the table appears to have recorded a one-core timing in
//! > the sixteen-core row. The correction matters because that row was the
//! > sole argument for treating a map refresh as unaffordable inside a physics
//! > tick, and it is not. The old numbers carried no reproducible instrument;
//! > `examples/field_timing.rs` now is one.
//!
//! **Both paths must exist regardless**: `outram-mc-libs/CLAUDE.md`'s GPU
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

// Used by `pool()` (native/Android) and `gpu_context()` (native/Android,
// `gpu` feature) -- both absent on `wasm32-unknown-unknown`, so the import
// is gated the same way rather than left to warn as unused there.
#[cfg(not(target_arch = "wasm32"))]
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

    let vertical =
        (-0.5 * height_m * height_m / sz2).exp() + (-0.5 * height_m * height_m / sz2).exp();

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
///
/// **Native and Android only.** `wasm32-unknown-unknown` has no OS threads;
/// see [`field_pooled`]'s `wasm32` arm below.
#[cfg(not(target_arch = "wasm32"))]
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
#[cfg(not(target_arch = "wasm32"))]
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

/// The whole field, computed serially -- the `wasm32-unknown-unknown` arm of
/// [`field_pooled`].
///
/// `wasm32-unknown-unknown` has no OS threads, so `rayon` is not even a
/// dependency on this target (`Cargo.toml`'s `[target.'cfg(not(target_arch =
/// "wasm32"))'.dependencies]`), matching the constraint `boon-lay` already
/// solved for its own rayon usage (`crates/boon-lay/Cargo.toml`, bead
/// `op-okqo.1`): rayon-core needs real threads to run, not merely to compile.
/// The honest way to keep one source tree building for this target is to let
/// the parallel path degrade to its already-existing sequential twin,
/// [`field_serial`], rather than fake concurrency that is not there.
///
/// This is exact, not an approximation: the per-cell work is embarrassingly
/// parallel with no cross-cell state and no dependence on worker count --
/// `the_pooled_field_is_bit_identical_to_the_serial_one` pins that on the
/// native target, so `field_pooled` and `field_serial` are already known to
/// agree bit for bit before this target ever calls either.
#[cfg(target_arch = "wasm32")]
pub fn field_pooled(states: &[PuffState], grid: &FieldGrid) -> Vec<f32> {
    field_serial(states, grid)
}

/// changi's cached GPU probe, so a field refreshing at 10 Hz does not
/// re-probe the adapter every call. Probing is expensive; the result cannot
/// change over a process's lifetime, so one probe per process is correct.
///
/// `None` means "no usable adapter" and is not an error -- exactly
/// [`petir::wgsl::gpu::GpuContext::probe`]'s own contract.
#[cfg(all(
    feature = "gpu",
    not(target_os = "android"),
    not(target_arch = "wasm32")
))]
fn gpu_context() -> Option<&'static petir::wgsl::gpu::GpuContext> {
    static CONTEXT: OnceLock<Option<petir::wgsl::gpu::GpuContext>> = OnceLock::new();
    CONTEXT
        .get_or_init(petir::wgsl::gpu::GpuContext::probe)
        .as_ref()
}

/// The whole field, dispatched as a WGSL kernel through PETIR's runner.
///
/// changi owns no `wgpu` plumbing of its own -- see the module doc on why:
/// [`GAUSSIAN_PUFF_FIELD`] declares functions in PETIR's own convention and
/// [`petir::wgsl::gpu::GpuContext::eval_map`] supplies the entry point, the
/// bind-group layout and the buffer plumbing.
///
/// Returns `None` when there is no usable adapter, or when `eval_map` itself
/// returns `None` (an empty grid). **Never panics**, and never falls back to
/// the CPU path itself -- [`field_auto`] is what selects between the two.
#[cfg(all(
    feature = "gpu",
    not(target_os = "android"),
    not(target_arch = "wasm32")
))]
pub fn field_gpu(states: &[PuffState], grid: &FieldGrid) -> Option<Vec<f32>> {
    use petir::wgsl::gpu::KernelParams;

    let gpu = gpu_context()?;

    let data = pack_states(states);
    let probe: Vec<f32> = (0..grid.len()).map(|i| i as f32).collect();
    let params = KernelParams {
        n: data.len() as u32,
        m: states.len() as u32,
        k: grid.cells as u32,
        a: grid.half_width_m,
        b: grid.source_height_m,
        ..Default::default()
    };

    gpu.eval_map(
        &[GAUSSIAN_PUFF_FIELD],
        "changi_puff_field(x, params.k, params.m, params.a, params.b)",
        &data,
        &probe,
        params,
    )
}

/// The field, GPU-accelerated when the `gpu` feature is on and a usable
/// adapter is found, falling back to [`field_pooled`] otherwise.
///
/// This is what a caller should reach for. With the `gpu` feature off this
/// compiles down to [`field_pooled`] alone, with no `cfg` visible at the call
/// site. The CPU path is not a stopgap: `outram-mc-libs/CLAUDE.md`'s GPU
/// policy makes it mandatory and trusted, and here it is also the reference
/// the GPU result is checked against -- see [`field_serial`] and the
/// GPU-vs-CPU comparison test below.
pub fn field_auto(states: &[PuffState], grid: &FieldGrid) -> Vec<f32> {
    #[cfg(all(
        feature = "gpu",
        not(target_os = "android"),
        not(target_arch = "wasm32")
    ))]
    {
        if let Some(field) = field_gpu(states, grid) {
            return field;
        }
    }
    field_pooled(states, grid)
}

/// Whether [`field_auto`] will actually take the GPU path on this host.
///
/// # Why a caller needs to ask
///
/// [`field_auto`] always returns the right answer, but not at the same
/// *cost*: the CPU path is measured in this module's doc at ~140 ms on 16
/// cores and ~2.2 s on one, for the 64x64 grid. A caller on a real-time
/// budget -- `htgr_sim_v1`'s physics thread has 100 ms per tick -- therefore
/// cannot afford to call [`field_auto`] at map cadence unless the GPU path is
/// genuinely available, and "is the `gpu` feature on" is not the same
/// question: the feature can be on with no usable adapter behind it.
///
/// This probes the adapter (once, cached in [`gpu_context`]) rather than
/// reporting the feature flag, so it answers what the caller actually needs
/// to know. `false` with the feature off, `false` with the feature on and no
/// adapter, `false` on Android and wasm where `petir::wgsl::gpu` is itself
/// gated out.
///
/// This is a scheduling hint and nothing more. It must never select a
/// different *model*: both paths compute the same field, and
/// `field_gpu_agrees_with_the_serial_reference` pins that to 1e-4 relative.
pub fn has_gpu_field() -> bool {
    #[cfg(all(
        feature = "gpu",
        not(target_os = "android"),
        not(target_arch = "wasm32")
    ))]
    {
        return gpu_context().is_some();
    }
    #[cfg(not(all(
        feature = "gpu",
        not(target_os = "android"),
        not(target_arch = "wasm32")
    )))]
    {
        false
    }
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
        let sig = pasquill_gifford_sigmas(class, Length::new::<meter>(distance)).expect("sigmas");
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
            PuffState {
                x: 1.0,
                y: 2.0,
                sigma_y: 3.0,
                sigma_z: 4.0,
                weight: 5.0,
            },
            PuffState {
                x: 6.0,
                y: 7.0,
                sigma_y: 8.0,
                sigma_z: 9.0,
                weight: 10.0,
            },
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
        let empty_grid = FieldGrid {
            cells: 0,
            half_width_m: 1000.0,
            source_height_m: 40.0,
        };
        assert!(field_serial(&[], &empty_grid).is_empty());
        assert!(field_pooled(&[], &empty_grid).is_empty());

        let g = grid();
        let no_states = field_pooled(&[], &g);
        assert_eq!(no_states.len(), g.len());
        assert!(
            no_states.iter().all(|v| *v == 0.0),
            "no puffs is a zero field"
        );
    }

    /// **GPU-vs-CPU agreement.** `field_gpu` must reproduce [`field_serial`]
    /// -- not `field_pooled` -- to the `f32` budget, on any adapter this
    /// happens to run on.
    ///
    /// `field_serial` is deliberately the reference here rather than the
    /// pooled path: a scheduling bug in changi's own thread pool must not be
    /// able to mask a shader bug by agreeing with a CPU path that shares the
    /// same defect. `the_pooled_field_is_bit_identical_to_the_serial_one`
    /// separately pins pooled against serial.
    ///
    /// **Skips cleanly with no adapter** -- per `outram-mc-libs/CLAUDE.md`'s
    /// GPU policy, detection is at run time and CI must never fail for want
    /// of a GPU. This is not hypothetical here: `gpu` is off by default, so
    /// even a machine with a GPU only reaches this path under
    /// `--features gpu`.
    #[cfg(all(
        feature = "gpu",
        not(target_os = "android"),
        not(target_arch = "wasm32")
    ))]
    #[test]
    fn field_gpu_agrees_with_the_serial_reference() {
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

        let Some(gpu) = field_gpu(&states, &g) else {
            eprintln!("SKIP field_gpu_agrees_with_the_serial_reference: no GPU adapter");
            return;
        };
        let serial = field_serial(&states, &g);
        assert_eq!(gpu.len(), serial.len());

        let mut worst = 0.0_f32;
        for (a, b) in gpu.iter().zip(serial.iter()) {
            let tolerance = b.abs() * 1e-4 + 1e-30;
            worst = worst.max((a - b).abs());
            assert!(
                (a - b).abs() <= tolerance,
                "GPU field disagrees with field_serial: {a:e} vs {b:e}"
            );
        }
        eprintln!("field_gpu vs field_serial: worst absolute difference {worst:e}");
    }

    /// **The default build's behaviour is pinned.** With the `gpu` feature
    /// off, [`field_auto`] must be exactly [`field_pooled`] -- no `cfg`
    /// leakage, no silent behaviour change for the overwhelming majority of
    /// builds that never enable `gpu`.
    #[cfg(not(feature = "gpu"))]
    #[test]
    fn field_auto_is_field_pooled_with_the_feature_off() {
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

        let auto = field_auto(&states, &g);
        let pooled = field_pooled(&states, &g);
        assert_eq!(
            auto, pooled,
            "field_auto must equal field_pooled with `gpu` off"
        );
    }
}
