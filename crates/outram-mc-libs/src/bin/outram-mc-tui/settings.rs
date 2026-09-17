//! Run settings the user tunes in-TUI before launching a transport run (op-4h8).
//!
//! [`RunSettings`] carries exactly what the bead asked for: GPU-vs-CPU compute
//! backend, histories-per-generation, active/inactive generations ("batches"),
//! and the RNG seed. It maps directly onto
//! [`outram_mc_libs::physics::keff::KeffSettings`] — this module only adds the
//! touch-stepper increment/clamp logic the settings screen drives.

use outram_mc_libs::physics::compute::{ComputeType, ThreadCount};
use outram_mc_libs::physics::keff::KeffSettings;

/// In-TUI run configuration (op-4h8). Adjusted with touch steppers on the
/// settings screen; converted to a [`KeffSettings`] by [`RunSettings::to_keff_settings`]
/// right before a run starts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RunSettings {
    /// CPU-single / CPU-multi / GPU compute backend. See
    /// [`ComputeType`] — GPU transparently falls back to CPU when
    /// [`outram_mc_libs::gpu::probe`] finds no adapter (always the case on
    /// Android; possibly also on a headless desktop CI box).
    pub compute: ComputeType,
    /// Neutron histories transported per generation ("histories/gen").
    pub n_particles: usize,
    /// Inactive (source-convergence) generations, discarded from the k tally.
    pub n_inactive: usize,
    /// Active generations ("batches") averaged into the reported eigenvalue.
    pub n_active: usize,
    /// Master RNG seed. A fixed seed reruns bit-reproducibly on the
    /// single-thread CPU backend.
    pub seed: u64,
}

impl Default for RunSettings {
    /// A deliberately tiny "quick preview" run (30 histories x [1 inactive +
    /// 2 active]): a noisy but fast first-look k-eigenvalue, sized to the
    /// **slowest** preset this crate ships (the free-gas-hydrogen LWR cell —
    /// see below), not the fastest.
    ///
    /// **Why so much smaller than `outram-mc-libs`'s own 400x45-generation
    /// default** ([`outram_mc_libs::physics::keff::KeffSettings::default`],
    /// documented there as "enough for a first-look Keff in seconds"): this
    /// crate's [`crate::presets::GeometryPreset::LwrCell`] preset measured at
    /// roughly **50ms per (history x generation)** on this development
    /// machine — an UO2/Zircaloy/free-gas-hydrogen reflective cell has no
    /// thermal cutoff (see that preset's documented approximation), so a
    /// neutron's random walk down through the resonance region and toward
    /// thermal can take many thousands of collisions per history before
    /// leaking or being absorbed. At the `outram-mc-libs` default scale
    /// (400 x 45 = 18 000 history-generations) that preset alone measured
    /// **over 90 seconds** in this environment - unacceptable for a phone
    /// app's default experience. This default (30 x 3 = 90 history-generations)
    /// keeps every preset in this crate, including that one, in the
    /// low-single-digit seconds on the same machine. Raise the steppers on
    /// the settings screen for a tighter (slower) estimate once you know
    /// which preset you picked can afford it — see the crate README's
    /// "Known gaps" section.
    fn default() -> Self {
        Self {
            compute: ComputeType::CpuSingleThread,
            n_particles: 30,
            n_inactive: 1,
            n_active: 2,
            seed: 1,
        }
    }
}

/// Stepper bounds, shared by the settings screen's tap/keyboard handlers so a
/// touch tap and an arrow-key press move by the same increment and clamp the
/// same way.
pub const N_PARTICLES_STEP: usize = 100;
pub const N_PARTICLES_MIN: usize = 50;
pub const N_PARTICLES_MAX: usize = 20_000;
pub const N_GEN_STEP: usize = 5;
pub const N_GEN_MIN: usize = 1;
pub const N_GEN_MAX: usize = 500;

impl RunSettings {
    /// Cycle the compute backend: CPU-single -> CPU-multi(Auto) -> GPU -> CPU-single.
    pub fn cycle_compute(&mut self) {
        self.compute = match self.compute {
            ComputeType::CpuSingleThread => ComputeType::CpuMultiThread(ThreadCount::Auto),
            ComputeType::CpuMultiThread(_) => ComputeType::Gpu,
            ComputeType::Gpu => ComputeType::CpuSingleThread,
        };
    }

    /// A short label for the compute-backend row.
    pub fn compute_label(&self) -> &'static str {
        match self.compute {
            ComputeType::CpuSingleThread => "CPU (single-thread, deterministic reference)",
            ComputeType::CpuMultiThread(_) => "CPU (multi-thread, rayon over all cores)",
            ComputeType::Gpu => "GPU (falls back to CPU if no adapter)",
        }
    }

    pub fn bump_particles(&mut self, delta: i64) {
        self.n_particles = bump_usize(
            self.n_particles,
            delta * N_PARTICLES_STEP as i64,
            N_PARTICLES_MIN,
            N_PARTICLES_MAX,
        );
    }
    pub fn bump_inactive(&mut self, delta: i64) {
        self.n_inactive = bump_usize(
            self.n_inactive,
            delta * N_GEN_STEP as i64,
            N_GEN_MIN,
            N_GEN_MAX,
        );
    }
    pub fn bump_active(&mut self, delta: i64) {
        self.n_active = bump_usize(
            self.n_active,
            delta * N_GEN_STEP as i64,
            N_GEN_MIN,
            N_GEN_MAX,
        );
    }
    pub fn bump_seed(&mut self, delta: i64) {
        let s = self.seed as i64 + delta;
        self.seed = s.max(1) as u64;
    }

    /// Total generations this run will execute (inactive + active) — used by
    /// the run screen to size the convergence chart's x-axis up front.
    pub fn n_generations(&self) -> usize {
        self.n_inactive + self.n_active
    }

    /// Build the [`KeffSettings`] the transport drivers actually take. Watt
    /// fission-spectrum parameters and temperature are left at
    /// [`KeffSettings::default`] — the bead's requested knobs are compute
    /// backend, particle/generation counts, and seed only.
    pub fn to_keff_settings(&self) -> KeffSettings {
        KeffSettings {
            n_particles: self.n_particles,
            n_inactive: self.n_inactive,
            n_active: self.n_active,
            seed: self.seed,
            compute: self.compute,
            ..KeffSettings::default()
        }
    }
}

fn bump_usize(value: usize, delta: i64, min: usize, max: usize) -> usize {
    let v = value as i64 + delta;
    v.clamp(min as i64, max as i64) as usize
}
