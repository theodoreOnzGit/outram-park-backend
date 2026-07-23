//! Transport run driver: wires a built preset case to the crate's transport
//! entry points, and runs it on a background thread so the UI stays responsive
//! to touch input while a run is in flight (op-4h8 "LIVE convergence").
//!
//! # Which entry point runs which case
//!
//! - [`crate::presets::BuiltCase::Csg`] -> [`run_keff_csg`], with an
//!   [`crate::spectrum`] energy-flux tally attached (these are exactly the
//!   presets [`crate::presets::GeometryPreset::supports_spectrum`] reports
//!   `true` for).
//! - [`crate::presets::BuiltCase::Delta`] -> [`run_keff_delta`], no tally (the
//!   driver has no tally parameter in this crate version).
//!
//! # "Live" convergence, honestly described
//!
//! Neither `run_keff_csg`, `run_keff_delta`, nor the bare-sphere `run_keff`
//! expose a per-generation progress callback in this crate version — a whole
//! run is one blocking call that returns [`KeffResult::k_by_generation`] only
//! at completion. So "live" here means two real things, not a simulated one:
//!
//! 1. The blocking call runs on a background [`std::thread`] (via
//!    [`RunHandle::start`]), so the render loop keeps polling touch/keyboard
//!    input and redrawing a busy spinner + elapsed-time clock the whole time —
//!    the UI never freezes for the run's duration.
//! 2. Once the thread finishes, the run screen animates *revealing* the
//!    already-computed `k_by_generation` trace a few points per redraw tick
//!    (see [`crate::ui::run_screen`]), so the convergence history draws itself
//!    on screen rather than appearing all at once.
//!
//! A future crate version that adds a progress channel to the transport
//! drivers could replace step 2 with genuinely incremental data; documented
//! here so this isn't overclaimed as continuous per-generation streaming.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::physics::compute::ComputeType;
use outram_mc_libs::physics::transport_csg::run_keff_csg;
use outram_mc_libs::pebble_beds::keff_delta::run_keff_delta;

use crate::presets::{BuiltCase, GeometryPreset};
use crate::settings::RunSettings;
use crate::spectrum::{new_spectrum_tally, SpectrumOverlay};

/// Outcome of one completed transport run — everything the run/spectrum
/// screens need to render, already extracted from the crate's `KeffResult`
/// (and, when available, the finished spectrum tally) so the UI never has to
/// touch `outram-mc-libs` types directly.
#[derive(Debug, Clone)]
pub struct RunOutcome {
    /// Mean eigenvalue over the active generations.
    pub k_mean: f64,
    /// Standard error of the mean (1 sigma) over the active generations.
    pub k_std: f64,
    /// Per-generation eigenvalue estimate, inactive generations first —
    /// the trace the run screen's convergence chart plots.
    pub k_by_generation: Vec<f64>,
    /// Wall-clock time the blocking transport call took.
    pub elapsed: Duration,
    /// Which compute backend actually ran (what [`RunSettings::compute`] was
    /// set to — the drivers fall back to CPU silently on a missing GPU, so
    /// this does not by itself prove the GPU path executed; see
    /// [`RunOutcome::gpu_available`]).
    pub compute_requested: ComputeType,
    /// Whether [`outram_mc_libs::gpu::probe`] found a usable headless GPU
    /// adapter on this machine (always `false` on Android, where the GPU
    /// module compiles to a CPU-only shim).
    pub gpu_available: bool,
    /// Spectrum/cross-section overlay data, when the preset's driver supports
    /// tallying (see [`crate::presets::GeometryPreset::supports_spectrum`]).
    pub spectrum: Option<SpectrumOverlay>,
}

/// Run one preset case to completion — the synchronous, blocking driver.
/// Called from inside [`RunHandle::start`]'s background thread; also called
/// directly (never through a thread) by the headless smoke test, which needs
/// a synchronous call it can assert on immediately.
pub fn run_case(preset: GeometryPreset, settings: RunSettings) -> Result<RunOutcome, String> {
    let t0 = Instant::now();
    let keff_settings = settings.to_keff_settings();
    let built = preset.build()?;
    let gpu_available = outram_mc_libs::gpu::probe().is_some();

    let (k_mean, k_std, k_by_generation, spectrum) = match built {
        BuiltCase::Csg { geom, materials, nuclides, source } => {
            let (mut tally, edges) = new_spectrum_tally();
            let result = run_keff_csg(&geom, &materials, &nuclides, source, &keff_settings, Some(&mut tally));
            let overlay = SpectrumOverlay::finish(&tally, &edges, keff_settings.n_active as u64, &materials[0], &nuclides);
            (result.k_mean, result.k_std, result.k_by_generation, Some(overlay))
        }
        BuiltCase::Delta { half_width, materials, nuclides, majorant, packed, fuel_material_idx, matrix_material_idx } => {
            let material_at =
                move |p: Position| Some(if packed.is_inside_kernel(p) { fuel_material_idx } else { matrix_material_idx });
            let result = run_keff_delta(half_width, &materials, &nuclides, &majorant, material_at, &keff_settings);
            (result.k_mean, result.k_std, result.k_by_generation, None)
        }
    };

    Ok(RunOutcome {
        k_mean,
        k_std,
        k_by_generation,
        elapsed: t0.elapsed(),
        compute_requested: keff_settings.compute,
        gpu_available,
        spectrum,
    })
}

/// Lifecycle of a background transport run, as observed by the render loop.
#[derive(Clone)]
pub enum RunPhase {
    /// No run started yet (or the user backed out before one finished).
    Idle,
    /// A run is executing on the background thread; `started` feeds the run
    /// screen's elapsed-time clock and spinner.
    Running { started: Instant },
    /// The background thread finished; `Err` carries a preset-build failure
    /// (e.g. a CORE-library nuclide lookup miss) rather than a panic.
    Done(Result<RunOutcome, String>),
}

/// Owns the shared state a background transport run reports back through.
/// [`RunHandle::start`] spawns the thread; the render loop calls
/// [`RunHandle::snapshot`] every tick (never blocking — a
/// [`Mutex::lock`] on a small enum is effectively instant) to decide what the
/// run screen shows.
#[derive(Clone)]
pub struct RunHandle {
    phase: Arc<Mutex<RunPhase>>,
}

impl RunHandle {
    pub fn new() -> Self {
        Self { phase: Arc::new(Mutex::new(RunPhase::Idle)) }
    }

    /// Start a run on a background thread, replacing whatever phase was
    /// previously stored (starting a new run always supersedes an old
    /// finished/idle one — there is only ever one run in flight).
    pub fn start(&self, preset: GeometryPreset, settings: RunSettings) {
        *self.phase.lock().expect("run-phase mutex poisoned") = RunPhase::Running { started: Instant::now() };
        let phase = Arc::clone(&self.phase);
        thread::spawn(move || {
            let outcome = run_case(preset, settings);
            *phase.lock().expect("run-phase mutex poisoned") = RunPhase::Done(outcome);
        });
    }

    /// Reset to [`RunPhase::Idle`] — used when the user backs out of the run
    /// screen to reconfigure and rerun.
    pub fn reset(&self) {
        *self.phase.lock().expect("run-phase mutex poisoned") = RunPhase::Idle;
    }

    /// A cheap clone of the current phase for this redraw tick.
    pub fn snapshot(&self) -> RunPhase {
        self.phase.lock().expect("run-phase mutex poisoned").clone()
    }
}

impl Default for RunHandle {
    fn default() -> Self {
        Self::new()
    }
}
