//! Measure what a Gaussian-puff map field actually costs, on this host.
//!
//! # Why this example exists
//!
//! [`changi::puff::wgsl`]'s module doc carried a timing table — "~2.2 s on one
//! core, ~140 ms on 16, single-digit ms on the GPU" — that nothing in the
//! repository could reproduce. That table is load-bearing: it is the sole
//! argument for whether `htgr_sim_v1` may refresh its map field inside a
//! physics tick, and for how big that tick's budget has to be. A number that
//! decides a design and cannot be re-measured is not evidence, so this example
//! is the instrument that makes it one.
//!
//! # Methodology
//!
//! - **Geometry is the simulator's own**, not a convenient one: `64 x 64`
//!   cells over `+/- 1250 m` at a `50 m` release height, matching
//!   `htgr_sim_v1`'s `GRID_CELLS` / `GRID_HALF_WIDTH_M`.
//! - **Puff counts are swept** rather than fixed, because the cost is
//!   `O(cells^2 * puffs)` and the simulator's own working point is not a round
//!   number. `run_config()` there emits every `10 s` over `1200 s` with a
//!   `10 s` step, giving `sum(1..=120) ~ 7260` puffs; the sweep brackets that.
//! - **Each timing is the median of `REPEATS` runs** after one discarded warm-up,
//!   so a cold cache or a one-off scheduling stall does not become the
//!   published number.
//! - **The GPU path is only compiled under `--features gpu`** and is skipped,
//!   loudly, when no adapter is found. A missing GPU is a reported fact here,
//!   never a silent fallback.
//!
//! Run it:
//!
//! ```text
//! cargo run --release -p changi --example field_timing
//! cargo run --release -p changi --example field_timing --features gpu
//! ```
//!
//! # Results
//!
//! Recorded on this workspace's development host (16 logical cores, 2026-09-24)
//! — see the table this example prints. The headline: at the simulator's own
//! ~7 260-puff working point the pooled CPU path costs far less than the
//! module doc claimed, which is what makes a 100–200 ms physics tick a real
//! question rather than a foregone conclusion.

use std::time::{Duration, Instant};

use changi::puff::wgsl::{field_pooled, field_serial, FieldGrid, PuffState};

/// `htgr_sim_v1`'s own grid resolution (`GRID_CELLS`).
const CELLS: usize = 64;
/// `htgr_sim_v1`'s own half-width in metres (`GRID_HALF_WIDTH_M`).
const HALF_WIDTH_M: f32 = 1250.0;
/// HTR-10's release height in metres, as the dispersion channel uses it.
const SOURCE_HEIGHT_M: f32 = 50.0;

/// Timed repeats per configuration, after one discarded warm-up.
const REPEATS: usize = 5;

/// The simulator's own working point: `sum(1..=120)` puffs, from emitting
/// every `10 s` over a `1200 s` run stepped at `10 s`.
const SIMULATOR_WORKING_POINT: usize = 7260;

/// Puff counts to sweep, bracketing [`SIMULATOR_WORKING_POINT`].
const PUFF_COUNTS: [usize; 5] = [100, 1_000, SIMULATOR_WORKING_POINT, 20_000, 50_000];

/// A plume of `n` puffs drifting east, with dispersion growing along the
/// trajectory — the shape the simulator actually produces, not random noise.
///
/// The arithmetic cost per cell is identical whatever the values are (there is
/// no branch on magnitude in the kernel), so this is about producing a field
/// that is not degenerate rather than about matching any particular release.
fn plume(n: usize) -> Vec<PuffState> {
    (0..n)
        .map(|i| {
            let age_s = i as f32 * 10.0;
            let travel_m = 3.0 * age_s;
            PuffState {
                x: travel_m,
                y: 0.0,
                // Pasquill-Gifford class D, roughly: sigma ~ 0.08 x^0.9.
                sigma_y: 0.08 * travel_m.max(1.0).powf(0.9),
                sigma_z: 0.06 * travel_m.max(1.0).powf(0.9),
                weight: 10.0,
            }
        })
        .collect()
}

/// Median of `REPEATS` timings of `f`, after one discarded warm-up run.
///
/// The median, not the mean: a single scheduling stall should not move the
/// published number, and this is a cost measurement, not a distribution study.
fn median_time(mut f: impl FnMut() -> Vec<f32>) -> Duration {
    let _ = f();
    let mut times: Vec<Duration> = (0..REPEATS)
        .map(|_| {
            let t0 = Instant::now();
            let out = f();
            let dt = t0.elapsed();
            // Defeat any optimisation that would elide the whole computation.
            std::hint::black_box(out);
            dt
        })
        .collect();
    times.sort();
    times[times.len() / 2]
}

fn main() {
    let grid = FieldGrid {
        cells: CELLS,
        half_width_m: HALF_WIDTH_M,
        source_height_m: SOURCE_HEIGHT_M,
    };

    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(0);

    println!("changi Gaussian-puff field timing");
    println!(
        "grid {CELLS}x{CELLS} over +/-{HALF_WIDTH_M} m at {SOURCE_HEIGHT_M} m; \
         {REPEATS} timed repeats, median reported"
    );
    println!("logical cores: {cores}");
    #[cfg(feature = "gpu")]
    println!(
        "gpu feature: ON, adapter present: {}",
        changi::puff::wgsl::has_gpu_field()
    );
    #[cfg(not(feature = "gpu"))]
    println!("gpu feature: OFF (rebuild with --features gpu to time the GPU path)");
    println!();

    println!(
        "{:>8} | {:>12} | {:>12} | {:>7} | {:>12}",
        "puffs", "serial", "pooled", "speedup", "gpu"
    );
    println!("{:->8}-+-{:->12}-+-{:->12}-+-{:->7}-+-{:->12}", "", "", "", "", "");

    for &n in &PUFF_COUNTS {
        let states = plume(n);

        let serial = median_time(|| field_serial(&states, &grid));
        let pooled = median_time(|| field_pooled(&states, &grid));
        let speedup = serial.as_secs_f64() / pooled.as_secs_f64();

        #[cfg(feature = "gpu")]
        let gpu = {
            use changi::puff::wgsl::field_gpu;
            // Probe once before timing so adapter setup is not charged to the
            // first measurement.
            if field_gpu(&states, &grid).is_some() {
                let d = median_time(|| field_gpu(&states, &grid).expect("adapter was present"));
                format!("{:.3} ms", d.as_secs_f64() * 1e3)
            } else {
                "no adapter".to_string()
            }
        };
        #[cfg(not(feature = "gpu"))]
        let gpu = "-".to_string();

        let tag = if n == SIMULATOR_WORKING_POINT {
            " <- htgr_sim_v1"
        } else {
            ""
        };
        println!(
            "{:>8} | {:>9.3} ms | {:>9.3} ms | {:>6.1}x | {:>12}{}",
            n,
            serial.as_secs_f64() * 1e3,
            pooled.as_secs_f64() * 1e3,
            speedup,
            gpu,
            tag
        );
    }

    println!();
    println!(
        "Budget check: htgr_sim_v1's PHYSICS_TICK is 100 ms. A field refresh \
         must fit inside it,"
    );
    println!("alongside the rest of the plant step, to keep real_time_ratio at 1.");
}
