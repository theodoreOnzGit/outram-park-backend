//! The model of the `ugraphite` rung (and the data of the `lumped` rung):
//! uranium and graphite at 296 K, ENDF/B-VIII.0.
//!
//! **Nothing is retyped here.** The tapes, the temperature and the
//! compositions are outram-mc-libs' [`outram_mc_libs::vv::ugraphite`], the
//! module `examples/ugraphite_four_factor.rs` (rung 2) and
//! `examples/lumped_ugraphite_kinf.rs` (rung 3) run, so the demo moves the
//! same atoms the recorded results were measured on.
//!
//! - **Composition shown** ([`mixture`]): rung 2's main case, the uranium and
//!   carbon of one HTR-10 fuel pebble smeared over the ball (17 wt% U-235,
//!   `N_C/N_U` = 767.2), whose recorded `k_inf` is 1.56777 ± 0.00081.
//! - **Geometry** ([`build_geometry`]): the same reflective cube
//!   (`homogeneous_cube`, half-width [`HALF_CM`]) the example uses, so a
//!   neutron reaching a wall comes back: an infinite medium.
//! - **Data tier.** ~~Watch mode is an illustration and processes the tapes at
//!   the loosened tolerance 0.01 ([`SpeedTier::VeryFast`]); its measured
//!   effect on the thermal benchmarks is about −60 pcm
//!   (`outram-mc-libs/docs/profiling/speed_tiers_2026_09_27.md`).~~
//!   **CORRECTED 2026-10-16** (maintainer, gh:#876 item 3): NJOY's 0.001,
//!   as every demo.

use crate::mc_demo::tapes::read_tape;
use outram_mc_libs::material::material::Material;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::speed::SpeedTier;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::vv::ugraphite as vv;

// The model half (tapes, indices, mixture, materials, cube) moved to
// `nee_soon::mc_rungs::ugraphite` (gh:#1742, 2026-11-25); the data builder,
// which processes through the browser's cache, stays here.
pub use nee_soon::mc_rungs::ugraphite::{build_geometry, materials, HALF_CM, JOBS, N_U235, N_U238, TEMPERATURE_K};

/// Incremental processing, one job per call, as `load_nuclides` in
/// `examples/common/ugraphite_common.rs` does from files: RECONR + BROADR to
/// 296 K, URR and DBRC by the constructor's defaults, and the graphite law
/// given per material to the carbon of the materials that hold it
/// (`vv::bind_graphite`, GitHub #462 and #1635 item 3). Compile-checked only
/// here: a browser demo, not run.
pub struct DataBuilder {
    tier: SpeedTier,
    nuclides: Vec<Nuclide>,
    sab: Option<ThermalScattering>,
}

impl DataBuilder {
    pub fn new(tier: SpeedTier) -> Self {
        Self { tier, nuclides: Vec::new(), sab: None }
    }
    /// The expensive half comes from `store` when it holds it (gh:#818).
    pub fn step(&mut self, bytes: &[u8], store: &mut crate::mc_demo::processed_cache::DataStore) -> Result<(), String> {
        let i = self.nuclides.len() + self.sab.is_some() as usize;
        match Self::make(self.tier, i, bytes, store)? {
            crate::mc_demo::rungs::Product::Nuclide(n) => self.nuclides.push(n),
            crate::mc_demo::rungs::Product::Law(l) => self.sab = Some(l),
        }
        Ok(())
    }
    /// Job `i`'s product (what [`Self::step`] keeps, and a processing-pool
    /// worker only caches).
    pub fn make(tier: SpeedTier, i: usize, bytes: &[u8], store: &mut crate::mc_demo::processed_cache::DataStore) -> Result<crate::mc_demo::rungs::Product, String> {
        let (label, _) = *JOBS.get(i).ok_or("no job left")?;
        let (tape, mat) = read_tape(bytes, label)?;
        if i < vv::TAPES.len() {
            Ok(crate::mc_demo::rungs::Product::Nuclide(store.nuclide(bytes, &tape, mat, vv::TAPES[i].0, TEMPERATURE_K, tier, label)?))
        } else {
            Ok(crate::mc_demo::rungs::Product::Law(store.thermal(bytes, &tape, mat, TEMPERATURE_K, "c_Graphite", label)?))
        }
    }
    /// The five free nuclides in [`vv::TAPES`] order with bound carbon copies
    /// appended, and `materials` repointed at them: the graphite law goes to
    /// the carbon of every material in `materials` that holds carbon. The
    /// returned [`vv::RoleIndex`] says where each role sits afterwards (the
    /// C-12 the σ(E) panel draws is the bound copy, not slot 3).
    pub fn finish(self, materials: &[Material]) -> Result<vv::GraphiteBinding, String> {
        let sab = self.sab.as_ref().ok_or("the graphite S(a,b) job was not done")?;
        if self.nuclides.len() != vv::TAPES.len() {
            return Err(format!("{} of {} nuclides done", self.nuclides.len(), vv::TAPES.len()));
        }
        vv::bind_graphite(materials, &self.nuclides, Some(sab))
    }
}

