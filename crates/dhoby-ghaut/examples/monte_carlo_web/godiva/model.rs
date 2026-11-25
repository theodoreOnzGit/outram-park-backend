//! The Godiva rung's model: ICSBEP HEU-MET-FAST-001, a bare sphere of highly
//! enriched uranium metal, r = 8.7407 cm.
//!
//! **Nothing is retyped here.** Radius, temperature, the three nuclides, their
//! tapes and atom densities all come from [`outram_mc_libs::vv::godiva`], the
//! model `examples/godiva_keff_endf_local.rs` runs, i.e. the model the
//! recorded result (~~route 4 of the five-route study, −52 ± 27 pcm~~,
//! superseded 2026-10-05 by `godiva_keff_ensemble.rs`'s 1024 seeds, −6 ± 5 pcm,
//! #546) was measured on.
//!
//! **Data tier.** Run k_eff processes the tapes at NJOY's tolerance 0.001
//! ([`SpeedTier::Fast`], the library default and exactly the record's data),
//! so a reader's `k` is comparable with the record (maintainer decision (a),
//! 2026-10-04). ~~Watch mode is an illustration and may use the loosened 0.01
//! tier ([`SpeedTier::VeryFast`]), whose measured effect on Godiva is
//! +7 ± 41 pcm (`outram-mc-libs/docs/profiling/speed_tiers_2026_09_27.md`).~~
//! **CORRECTED 2026-10-16** (maintainer, gh:#876 item 3): Watch uses the same
//! 0.001 data, one cached load for both modes.

use crate::tapes::read_tape;
use outram_mc_libs::material::material::Material;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::speed::SpeedTier;
use outram_mc_libs::vv::godiva;

// The model half (tapes, U-235's index, the sphere) moved to
// `nee_soon::mc_rungs::godiva` (gh:#1742, 2026-11-25); the data builder,
// which processes through the browser's cache, stays here.
pub use nee_soon::mc_rungs::godiva::{build_geometry, JOBS, N_U235, RADIUS_CM, TEMPERATURE_K};

/// Incremental processing, one [`Job`] per call.
pub struct DataBuilder {
    tier: SpeedTier,
    nuclides: Vec<Nuclide>,
}

impl DataBuilder {
    pub fn new(tier: SpeedTier) -> Self {
        Self { tier, nuclides: Vec::new() }
    }
    /// Process the next job from its (covariance-stripped) tape bytes, as
    /// `godiva_keff_endf_local.rs` does from the file: RECONR + BROADR to
    /// [`TEMPERATURE_K`] at this builder's tier.
    /// The expensive half comes from `store` when it holds it (gh:#818).
    pub fn step(&mut self, bytes: &[u8], store: &mut crate::processed_cache::DataStore) -> Result<(), String> {
        let n = Self::make(self.tier, self.nuclides.len(), bytes, store)?;
        self.nuclides.push(n);
        Ok(())
    }
    /// Job `i`'s nuclide (what [`Self::step`] keeps, and a processing-pool
    /// worker only caches).
    pub fn make(tier: SpeedTier, i: usize, bytes: &[u8], store: &mut crate::processed_cache::DataStore) -> Result<Nuclide, String> {
        let (name, _, _) = *godiva::NUCLIDES.get(i).ok_or("no job left")?;
        let (tape, mat) = read_tape(bytes, name)?;
        store.nuclide(bytes, &tape, mat, name, TEMPERATURE_K, tier, name)
    }
    pub fn finish(self) -> Result<NuclearData, String> {
        if self.nuclides.len() != JOBS.len() {
            return Err(format!("{} of {} jobs done", self.nuclides.len(), JOBS.len()));
        }
        Ok(NuclearData { nuclides: self.nuclides, material: godiva::material() })
    }
}

pub struct NuclearData {
    pub nuclides: Vec<Nuclide>,
    pub material: Material,
}

