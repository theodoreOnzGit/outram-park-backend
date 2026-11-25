//! The model: geometry, materials and the nuclear data they are built from.
//!
//! **MOVED 2026-11-25 to `nee_soon::mc_rungs::triso`** (gh:#1742), verbatim
//! (only paths changed), with its provenance notes; re-exported here whole,
//! so every `model::…` path of this rung and of `delta_tracking_web` stays.
//! What stays is the data processing through the browser's cache
//! ([`DataBuilder`]) and the uncached reference ([`nuclide_from_bytes`]).

// Headless output and data preparation are native-only; the browser build
// does not call them.
#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

use crate::tapes::read_tape;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;

pub use nee_soon::mc_rungs::triso::*;

/// A nuclide straight from the tape, no cache: the reference the
/// covariance-stripping test of `monte_carlo_web` compares against
/// (`delta_tracking_web`, which shares this file, does not use it).
/// `Nuclide::from_tape_with_speed` (route 4, njoy's `process`), which
/// [`DataBuilder::step`]'s store equals bit for bit since gh:#932
/// (~~the demos' pre-gh:#931 chain, `processed_cache::pre931_nuclide`~~).
#[allow(dead_code)]
pub fn nuclide_from_bytes(bytes: &[u8], name: &str) -> Result<Nuclide, String> {
    let (tape, mat) = read_tape(bytes, name)?;
    Nuclide::from_tape_with_speed(&tape, mat, name, TEMPERATURE_K, SPEED).map_err(|e| format!("{name}: {e}"))
}


/// What one TRISO job makes.
pub enum Made {
    Nuclide(Nuclide),
    Sab(ThermalScattering),
}

/// Incremental processing, one [`Job`] per call, so a UI can redraw between
/// the (long, blocking) jobs.
#[derive(Default)]
pub struct DataBuilder {
    done: usize,
    nuclides: Vec<Nuclide>,
    sab: Option<ThermalScattering>,
}

impl DataBuilder {
    pub fn next_job(&self) -> Option<Job> {
        JOBS.get(self.done).copied()
    }
    /// Process the next job from its (covariance-stripped) tape bytes, the
    /// expensive half from `store` when it holds it (gh:#818): the same
    /// nuclide as [`nuclide_from_bytes`], bit for bit.
    pub fn step(&mut self, bytes: &[u8], store: &mut crate::processed_cache::DataStore) -> Result<(), String> {
        match Self::make(self.done, bytes, store)? {
            Made::Nuclide(n) => self.nuclides.push(n),
            Made::Sab(l) => self.sab = Some(l),
        }
        self.done += 1;
        Ok(())
    }
    /// Job `i`'s product (what [`Self::step`] keeps, and a processing-pool
    /// worker only caches).
    /// (Its own [`Made`], not the Monte Carlo demo's `rungs::Product`:
    /// `delta_tracking_web` compiles this file too.)
    pub fn make(i: usize, bytes: &[u8], store: &mut crate::processed_cache::DataStore) -> Result<Made, String> {
        let job = JOBS.get(i).copied().ok_or("no job left")?;
        let (tape, mat) = read_tape(bytes, job.label)?;
        Ok(match job.kind {
            JobKind::Nuclide => Made::Nuclide(store.nuclide(bytes, &tape, mat, job.label, TEMPERATURE_K, SPEED, job.label)?),
            JobKind::GraphiteSab => Made::Sab(store.thermal(bytes, &tape, mat, TEMPERATURE_K, "c_Graphite", job.label)?),
        })
    }
    pub fn finish(self) -> Result<NuclearData, String> {
        if self.done != JOBS.len() {
            return Err(format!("{} of {} jobs done", self.done, JOBS.len()));
        }
        NuclearData::assemble(self.nuclides, self.sab.ok_or("no S(a,b)")?)
    }
}

