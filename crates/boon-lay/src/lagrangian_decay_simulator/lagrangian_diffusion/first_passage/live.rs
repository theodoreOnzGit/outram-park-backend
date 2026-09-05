//! Live, interactive Walk-on-Spheres ensemble for real-time visualisation.
//!
//! A [`LiveEnsemble`] owns a population of diffusing atoms and advances them a
//! slice of simulated time per call to [`LiveEnsemble::advance_frame`], choosing
//! the execution backend at runtime with a [`ComputeType`]. It is designed to be
//! driven from a **background worker thread**: the worker advances the ensemble
//! and publishes a small [`Snapshot`] (atom positions + release fraction) through
//! an `Arc<RwLock<…>>`, while the UI thread only reads the latest snapshot and
//! renders. Nothing here touches egui — the GUI examples own the thread and the
//! shared state; this type is the compute core they share.
//!
//! The three backends produce the **same physics**; they differ only in how the
//! independent histories are executed (one thread, all cores via `rayon`, or a
//! `wgpu` kernel). See [`crate::compute::ComputeType`].

use fission_yields_data::prelude::Nuclide;
#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;
#[cfg(target_arch = "wasm32")]
use crate::wasm_par::prelude::*;
#[cfg(target_arch = "wasm32")]
use crate::wasm_par as rayon;
use uom::si::f64::{Length, ThermodynamicTemperature, Time};
use uom::si::length::micrometer;
use uom::si::time::second;
use uom::ConstZero;

use crate::compute::{ComputeType, ThreadCount};
use crate::lagrangian_decay_simulator::lagrangian_diffusion::central_limit_theorem::oorandom_rng::OoRng64;
use crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::ensemble::{
    history_seed, EnsembleConfig,
};
use crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::walk_on_spheres::{
    sample_uniform_in_ball, WalkParams, WoSWalker,
};
use crate::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::TrisoCell;

/// A render-ready snapshot of a [`LiveEnsemble`] at one instant.
///
/// Small and cheap to clone/publish every frame — it carries only what a viewer
/// draws, not the full walker state.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    /// `(x, y)` of every still-contained atom, in micrometres (a 2-D slice).
    pub positions_xy_um: Vec<[f64; 2]>,
    /// Fraction of the ensemble released from the OPyC surface, in `[0, 1]`.
    pub released_fraction: f64,
    /// Simulated time of this snapshot, in seconds.
    pub sim_time_s: f64,
    /// Total number of atoms in the ensemble.
    pub n_total: usize,
}

/// A population of diffusing atoms advanced with a runtime-selectable backend.
pub struct LiveEnsemble {
    cell: TrisoCell,
    params: WalkParams,
    nuclide: Nuclide,
    config: EnsembleConfig,
    walkers: Vec<WoSWalker>,
    released: Vec<bool>,
    sim_time: Time,
    /// Cached rayon pool for [`ComputeType::CpuMultiThread`], rebuilt only when
    /// the resolved thread count changes (never per frame).
    pool: Option<rayon::ThreadPool>,
    pool_threads: usize,
}

impl LiveEnsemble {
    /// Build an ensemble of `n_histories` atoms of `nuclide`, born uniformly in
    /// the fuel kernel of `cell`, at a uniform `temperature`.
    ///
    /// `base_seed` seeds the per-atom RNG streams reproducibly.
    pub fn new(
        mut cell: TrisoCell,
        params: WalkParams,
        nuclide: Nuclide,
        temperature: ThermodynamicTemperature,
        n_histories: usize,
        base_seed: u64,
    ) -> Self {
        cell.set_uniform_temperature(temperature);
        let mut this = Self {
            cell,
            params,
            nuclide,
            config: EnsembleConfig {
                n_histories,
                base_seed,
            },
            walkers: Vec::new(),
            released: Vec::new(),
            sim_time: Time::ZERO,
            pool: None,
            pool_threads: 0,
        };
        this.reset();
        this
    }

    /// Re-birth the whole ensemble at the kernel with a fresh clock (`t = 0`).
    pub fn reset(&mut self) {
        let fuel_radius = self.cell.get_fuel_radius();
        let nuclide = self.nuclide;
        let base_seed = self.config.base_seed;
        self.walkers = (0..self.config.n_histories)
            .map(|i| {
                let mut master = OoRng64::from_u64(history_seed(base_seed, i));
                let start = sample_uniform_in_ball(&mut master.0, fuel_radius);
                let child = OoRng64::from_u64(master.next_u64());
                WoSWalker::new(start, nuclide, child)
            })
            .collect();
        self.released = vec![false; self.config.n_histories];
        self.sim_time = Time::ZERO;
    }

    /// Current simulated time.
    #[inline]
    pub fn sim_time(&self) -> Time {
        self.sim_time
    }

    /// The TRISO geometry this ensemble diffuses in (layer radii for drawing).
    #[inline]
    pub fn cell(&self) -> &TrisoCell {
        &self.cell
    }

    /// Number of atoms.
    #[inline]
    pub fn len(&self) -> usize {
        self.walkers.len()
    }

    /// Whether the ensemble is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.walkers.is_empty()
    }

    /// Fraction of atoms released from the OPyC surface so far, in `[0, 1]`.
    pub fn released_fraction(&self) -> f64 {
        if self.released.is_empty() {
            return 0.0;
        }
        let released = self.released.iter().filter(|&&r| r).count();
        released as f64 / self.released.len() as f64
    }

    /// Build a render snapshot of the current state.
    pub fn snapshot(&self) -> Snapshot {
        let positions_xy_um = self
            .walkers
            .iter()
            .zip(&self.released)
            .filter(|(_, &r)| !r)
            .map(|(w, _)| {
                [
                    w.position[0].get::<micrometer>(),
                    w.position[1].get::<micrometer>(),
                ]
            })
            .collect();
        Snapshot {
            positions_xy_um,
            released_fraction: self.released_fraction(),
            sim_time_s: self.sim_time.get::<second>(),
            n_total: self.walkers.len(),
        }
    }

    /// Advance every still-contained atom by pure diffusion until its simulated
    /// time reaches `until`, using the chosen `compute` backend.
    ///
    /// - [`ComputeType::CpuSingleThread`] advances the histories sequentially.
    /// - [`ComputeType::CpuMultiThread`] advances them across a dedicated rayon
    ///   pool sized to the carried [`ThreadCount`].
    /// - [`ComputeType::Gpu`] runs the `wgpu` kernel when a GPU adapter is present
    ///   (off Android), transparently falling back to the multi-thread CPU path
    ///   otherwise.
    ///
    /// Decay/transmutation is **not** applied here — this is the diffusion-only
    /// animation path (use the depletion ensemble for inventory studies).
    pub fn advance_frame(&mut self, compute: ComputeType, until: Time) {
        match compute {
            ComputeType::CpuSingleThread => self.advance_cpu_single(until),
            ComputeType::CpuMultiThread(tc) => self.advance_cpu_multi(until, tc),
            ComputeType::Gpu => self.advance_gpu(until),
        }
        if until > self.sim_time {
            self.sim_time = until;
        }
    }

    fn advance_cpu_single(&mut self, until: Time) {
        let cell = &self.cell;
        let params = &self.params;
        for (walker, is_released) in self.walkers.iter_mut().zip(self.released.iter_mut()) {
            if !*is_released && !walker.diffuse_until(cell, params, until) {
                *is_released = true;
            }
        }
    }

    fn advance_cpu_multi(&mut self, until: Time, thread_count: ThreadCount) {
        let n = thread_count.resolve();
        if self.pool.is_none() || self.pool_threads != n {
            // (Re)build the dedicated pool only when the thread count changes.
            if let Ok(pool) = rayon::ThreadPoolBuilder::new().num_threads(n).build() {
                self.pool = Some(pool);
                self.pool_threads = n;
            }
        }
        let cell = &self.cell;
        let params = &self.params;
        let walkers = &mut self.walkers;
        let released = &mut self.released;
        let mut run = || {
            walkers
                .par_iter_mut()
                .zip(released.par_iter_mut())
                .for_each(|(walker, is_released)| {
                    if !*is_released && !walker.diffuse_until(cell, params, until) {
                        *is_released = true;
                    }
                });
        };
        match &self.pool {
            Some(pool) => pool.install(run),
            None => run(), // pool build failed: fall back to the global pool
        }
    }

    /// GPU-backed advance. Filled in by the multilayer wgpu kernel; until then
    /// (and on Android, on wasm, and whenever no adapter is present) this runs
    /// the CPU path so `ComputeType::Gpu` is always safe to select.
    fn advance_gpu(&mut self, until: Time) {
        #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
        {
            if crate::gpu::advance_multilayer_best_effort(
                &self.cell,
                &self.params,
                &mut self.walkers,
                &mut self.released,
                self.nuclide,
                until,
            ) {
                return;
            }
        }
        // No GPU adapter / Android / wasm / GPU submit failed → CPU fallback.
        self.advance_cpu_multi(until, ThreadCount::Auto);
    }
}

/// Convenience: a micrometre `Length` (used by GUI callers building geometry).
#[inline]
pub fn micrometres(x: f64) -> Length {
    Length::new::<micrometer>(x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::length::micrometer;
    use uom::si::thermodynamic_temperature::degree_celsius;

    fn ensemble(n: usize) -> LiveEnsemble {
        LiveEnsemble::new(
            TrisoCell::new_crp6_geometry(),
            WalkParams::crp6_default(),
            Nuclide::Cs137,
            ThermodynamicTemperature::new::<degree_celsius>(1600.0),
            n,
            0xABCD_1234,
        )
    }

    #[test]
    fn snapshot_starts_full_in_kernel() {
        let e = ensemble(500);
        let s = e.snapshot();
        assert_eq!(s.n_total, 500);
        assert_eq!(s.positions_xy_um.len(), 500); // none released yet
        assert_eq!(s.released_fraction, 0.0);
        // All start inside the fuel kernel (radius 212.5 um).
        for p in &s.positions_xy_um {
            let r = (p[0] * p[0] + p[1] * p[1]).sqrt();
            assert!(r <= 212.5 + 1e-6, "atom born outside kernel: r={r}");
        }
    }

    #[test]
    fn cpu_single_and_multi_advance_time_and_agree() {
        // A short slice keeps the hop count small; the point is determinism, not
        // a long run. Same per-history seeds ⇒ identical result across threading.
        let until = Time::new::<second>(10.0);
        let mut a = ensemble(300);
        let mut b = ensemble(300);
        a.advance_frame(ComputeType::CpuSingleThread, until);
        b.advance_frame(ComputeType::CpuMultiThread(ThreadCount::Auto), until);
        assert_eq!(a.sim_time().get::<second>(), 10.0);
        assert_eq!(b.sim_time().get::<second>(), 10.0);
        assert_eq!(a.released_fraction(), b.released_fraction());
    }

    #[test]
    fn gpu_selection_is_safe_and_advances() {
        // With no adapter (headless), Gpu falls back to CPU and must still work.
        let until = Time::new::<second>(10.0);
        let mut e = ensemble(200);
        e.advance_frame(ComputeType::Gpu, until);
        assert_eq!(e.sim_time().get::<second>(), 10.0);
    }

    #[test]
    fn micrometres_helper_is_um() {
        assert_eq!(micrometres(212.5).get::<micrometer>(), 212.5);
    }
}
