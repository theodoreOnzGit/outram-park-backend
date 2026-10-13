//! **A rung whose model is a small CSG geometry** (gh:#859): what the
//! `jemima` and `hst009` rungs share, and the k-eigenvalue run the `lct008`
//! rung uses. Nothing here re-implements transport.
//!
//! A rung describes its case once, as a [`CsgCase`]: its tapes, its
//! materials and geometry (from `outram_mc_libs::vv`, the model the recorded
//! results were measured on), and where a fresh neutron is born. This module
//! then gives it:
//!
//! - **data processing**, one tape per step, through the shared processed-data
//!   cache ([`Builder`], gh:#818);
//! - **Watch, one neutron at a time** ([`Chain`]): each neutron is a call to
//!   outram-mc-libs' `run_fixed_source_traced` with ONE source particle and
//!   `max_secondaries = 0`, exactly as the Godiva and LCT-008 rungs do. After
//!   a fission the next neutron is born at that fission site from the U-235
//!   fission spectrum at the incident energy; after a leak or a capture, at
//!   the case's [`CsgCase::fresh_birth`]. That chaining is the illustration;
//!   each track is one real history;
//! - **Watch, whole generations** and **Run k_eff** ([`Keff`]):
//!   outram-mc-libs' [`CsgPowerIteration`], the single-thread CSG power
//!   iteration `run_keff_csg` runs, stepped one generation per request.
//!
//! **What a CSG run does not report** (and the page therefore does not show):
//! ~~the Shannon entropy of the source and~~ the leaked / captured / fissioned
//! counts, which `CsgPowerIteration` does not tally; the Godiva rung, on the
//! sphere-only `PowerIteration`, has them. **Corrected 2026-10-14 (gh:#859):**
//! the source entropy is reported, from `CsgPowerIteration::source_entropy`
//! on a 5 × 5 × 5 mesh over [`CsgCase::entropy_box`].
//!
//! **Run mode's drawn neutrons.** While a run is going, [`Loaded::run_next`]
//! starts each traced history at a site of the run's present fission source.
//! They are extra one-neutron fixed-source runs, not the run's own histories,
//! and are not counted in `k`.

// Headless output is native-only.
#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

use crate::engine::Tier;
use crate::history::History;
use crate::keff::{console_line, sample_sites, Generation, KeffConfig, Reference, CONSOLE_HEADER};
use crate::rungs::{LoadedRung, RungBuilder};
use crate::tapes::read_tape;
use crate::triso::model::splitmix;
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::material::material::Material;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::physics::fixed_source::{run_fixed_source_traced, FixedSource, FixedSourceSettings};
use outram_mc_libs::physics::keff::{GenerationReport, HistoryCounts, KeffSettings};
use outram_mc_libs::physics::track_output::{TrackEvent, TrackRecorder};
use outram_mc_libs::physics::transport_csg::{CsgGenerationReport, CsgPowerIteration, SourceBox};
use outram_mc_libs::tally::mesh::RegularMesh;
use std::marker::PhantomData;

/// A thermal scattering law a case attaches to one of its nuclides.
#[derive(Clone, Copy, Debug)]
pub struct ThermalLaw {
    /// Label shown while loading.
    pub label: &'static str,
    /// Tape in `reference-data/endf/`.
    pub tape: &'static str,
    /// The law's name, e.g. `c_H_in_H2O`.
    pub name: &'static str,
    /// Index, in [`CsgCase::nuclides`], of the nuclide that takes it.
    pub nuclide: usize,
}

/// One case: everything about the model that the shared code needs.
pub trait CsgCase {
    /// Material temperature, K.
    const TEMPERATURE_K: f64;
    /// Index of U-235 in [`Self::nuclides`] (the Watch chain's birth spectrum).
    const N_U235: usize;
    /// Incident energy of the fission that starts a fresh chain, eV.
    const FRESH_INCIDENT_EV: f64;
    /// Track states kept per history; far above what a history needs, so a
    /// truncated track is a reportable anomaly.
    const MAX_STATES: usize;
    /// `(label shown, nuclide name, tape)`, in the order the materials index.
    fn nuclides() -> &'static [(&'static str, &'static str, &'static str)];
    /// The thermal scattering law, processed after the nuclides.
    fn thermal() -> Option<ThermalLaw> {
        None
    }
    fn materials() -> Vec<Material>;
    fn geometry() -> Geometry;
    /// Where the first generation of a k-eigenvalue run starts.
    fn source_box() -> SourceBox;
    /// The box the source-entropy mesh covers: every place a fission can
    /// happen. The source box, unless the case fissions outside it.
    fn entropy_box() -> SourceBox {
        Self::source_box()
    }
    /// A uniformly random point of the multiplying region, for a fresh chain.
    fn fresh_birth(rng: &mut u64) -> Position;
    /// The recorded result the headless run prints beside its own.
    fn reference() -> Option<Reference>;
}

// ─── Data ────────────────────────────────────────────────────────────────────

/// Incremental processing of a case's tapes, one per [`RungBuilder::step`].
pub struct Builder<C: CsgCase> {
    tier: Tier,
    nuclides: Vec<Nuclide>,
    law: Option<ThermalScattering>,
    case: PhantomData<C>,
}

impl<C: CsgCase> RungBuilder for Builder<C> {
    type Loaded = Loaded<C>;
    fn new(tier: Tier) -> Self {
        Self { tier, nuclides: Vec::new(), law: None, case: PhantomData }
    }
    /// The next tape: RECONR + BROADR to the case's temperature at this
    /// builder's tier (URR tables and DBRC on, the library default), or
    /// THERMR for the thermal law. The expensive half comes from `store`
    /// when it holds it (gh:#818).
    fn step(&mut self, bytes: &[u8], store: &mut crate::processed_cache::DataStore) -> Result<(), String> {
        let i = self.nuclides.len();
        if let Some(&(label, name, _)) = C::nuclides().get(i) {
            let (tape, mat) = read_tape(bytes, label)?;
            let n = store.nuclide(bytes, &tape, mat, name, C::TEMPERATURE_K, self.tier.speed(), label)?;
            self.nuclides.push(n);
            return Ok(());
        }
        match (C::thermal(), &self.law) {
            (Some(law), None) => {
                let (tape, mat) = read_tape(bytes, law.label)?;
                self.law = Some(store.thermal(bytes, &tape, mat, C::TEMPERATURE_K, law.name, law.label)?);
                Ok(())
            }
            _ => Err("no job left".into()),
        }
    }
    fn finish(self) -> Result<Loaded<C>, String> {
        let want = C::nuclides().len();
        if self.nuclides.len() != want {
            return Err(format!("{} of {want} nuclides done", self.nuclides.len()));
        }
        let mut nuclides = self.nuclides;
        if let Some(law) = C::thermal() {
            let s = self.law.ok_or("the thermal scattering law was not processed")?;
            nuclides[law.nuclide] = nuclides[law.nuclide].clone().with_thermal_scattering(s);
        }
        Ok(Loaded { phys: Physics::new::<C>(nuclides), chain: Chain::new(crate::engine::CHAIN_SEED), keff: None, case: PhantomData })
    }
}

/// A case with its data processed and its geometry assembled.
pub struct Physics {
    pub geometry: Geometry,
    pub materials: Vec<Material>,
    pub nuclides: Vec<Nuclide>,
    pub source_box: SourceBox,
    pub temperature_k: f64,
}

impl Physics {
    pub fn new<C: CsgCase>(nuclides: Vec<Nuclide>) -> Self {
        Self { geometry: C::geometry(), materials: C::materials(), nuclides, source_box: C::source_box(), temperature_k: C::TEMPERATURE_K }
    }
}

// ─── Watch: one neutron at a time ────────────────────────────────────────────

pub struct Chain {
    rng: u64,
    next_index: u64,
    pending_fission: Option<(Position, f64)>,
}

impl Chain {
    pub fn new(seed: u64) -> Self {
        Self { rng: seed, next_index: 0, pending_fission: None }
    }

    pub fn run_next<C: CsgCase>(&mut self, phys: &Physics) -> History {
        let u235 = &phys.nuclides[C::N_U235];
        let from_fission = self.pending_fission.is_some();
        let (birth, e_in) = match self.pending_fission.take() {
            Some(site) => site,
            None => (C::fresh_birth(&mut self.rng), C::FRESH_INCIDENT_EV),
        };
        let birth_energy_ev = u235.sample_fission_energy(e_in, &mut self.rng);
        let settings = FixedSourceSettings {
            n_particles: 1,
            n_batches: 1,
            seed: splitmix(&mut self.rng),
            max_secondaries: 0,
            temperature_k: phys.temperature_k,
            ..Default::default()
        };
        let mut recorder = TrackRecorder::new(1, C::MAX_STATES);
        run_fixed_source_traced(
            &phys.geometry,
            &phys.materials,
            &phys.nuclides,
            &FixedSource::Point { r: birth, energy_ev: birth_energy_ev },
            &settings,
            None,
            Some(&mut recorder),
            None,
            None,
        );
        let track = recorder.tracks.into_iter().next().unwrap_or_default();
        let h = History::from_track(self.next_index, birth, birth_energy_ev, from_fission, track);
        self.next_index += 1;
        if h.outcome == Some(TrackEvent::Fission) {
            if let Some(s) = h.track.states.last() {
                self.pending_fission = Some((s.r, s.energy));
            }
        }
        h
    }
}

// ─── Generations: the real power iteration ───────────────────────────────────

/// A k-eigenvalue run in progress on a CSG geometry.
pub struct Keff {
    pub cfg: KeffConfig,
    it: CsgPowerIteration,
    /// The source-entropy mesh, [`ENTROPY_MESH`]³ on the source box unless
    /// [`Self::with_entropy_box`] moved it.
    mesh: RegularMesh,
}

/// Bins per side of the source-entropy mesh, as on the Godiva rung.
pub const ENTROPY_MESH: usize = 5;

fn entropy_mesh(b: SourceBox) -> RegularMesh {
    RegularMesh { lower_left: [b.lower.x, b.lower.y, b.lower.z], upper_right: [b.upper.x, b.upper.y, b.upper.z], dimension: [ENTROPY_MESH; 3] }
}

/// Half-width of the box a Watch "whole generations" run starts in, cm: every
/// neutron at the middle of the source box, to show the source spreading.
const POINT_HALF_WIDTH_CM: f64 = 0.05;

impl Keff {
    /// Sample the initial source as `run_keff_csg` does, in `source_box` (or,
    /// for `cfg.point_source`, in a 1 mm box at its centre).
    pub fn new(geometry: &Geometry, materials: &[Material], nuclides: &[Nuclide], source_box: SourceBox, temperature_k: f64, cfg: KeffConfig) -> Self {
        let settings = KeffSettings {
            n_particles: cfg.n_particles,
            n_inactive: cfg.n_inactive,
            n_active: cfg.n_active,
            seed: cfg.seed,
            temperature_k,
            ..KeffSettings::default()
        };
        let source = if cfg.point_source {
            let (lo, hi) = (source_box.lower, source_box.upper);
            let c = Position::new(0.5 * (lo.x + hi.x), 0.5 * (lo.y + hi.y), 0.5 * (lo.z + hi.z));
            let d = POINT_HALF_WIDTH_CM;
            SourceBox { lower: Position::new(c.x - d, c.y - d, c.z - d), upper: Position::new(c.x + d, c.y + d, c.z + d) }
        } else {
            source_box
        };
        Self { cfg, it: CsgPowerIteration::new(geometry, materials, nuclides, source, &settings), mesh: entropy_mesh(source_box) }
    }

    /// Put the source-entropy mesh on `b` ([`CsgCase::entropy_box`]).
    pub fn with_entropy_box(mut self, b: SourceBox) -> Self {
        self.mesh = entropy_mesh(b);
        self
    }

    /// A site of the source the next generation starts from, picked with
    /// `rng`: where Run mode's drawn neutrons are born.
    pub fn source_site(&self, rng: &mut u64) -> Option<Position> {
        let sites = self.it.source_positions();
        (!sites.is_empty()).then(|| sites[(splitmix(rng) % sites.len() as u64) as usize])
    }

    /// The source before the first generation (Watch draws it).
    pub fn initial_sites(&self) -> Vec<[f32; 2]> {
        sample_sites(&self.it.source_positions())
    }

    /// Run one generation. `None` once the run is over.
    pub fn step(&mut self, geometry: &Geometry, materials: &[Material], nuclides: &[Nuclide]) -> Option<Generation> {
        let r = self.it.step(geometry, materials, nuclides)?;
        let sites = if self.cfg.want_sites { sample_sites(&self.it.source_positions()) } else { Vec::new() };
        Some(Generation { report: report(&r, self.cfg.n_particles, self.it.source_entropy(&self.mesh)), sites })
    }

    pub fn finished(&self) -> bool {
        self.it.finished()
    }
}

/// A CSG generation as the page's shared [`GenerationReport`]: `entropy` is
/// the Shannon entropy of the source this generation produced
/// (`CsgPowerIteration::source_entropy`); no history counts (module docs);
/// the production is `k` × source neutrons, which is how `k` is defined.
pub fn report(r: &CsgGenerationReport, n_particles: usize, entropy: Option<f64>) -> GenerationReport {
    GenerationReport {
        index: r.index,
        active: r.active,
        k: r.k,
        entropy,
        k_mean: r.k_mean,
        bank_size: r.bank_size,
        production: r.k * n_particles as f64,
        counts: HistoryCounts::default(),
    }
}

/// A headless Run k_eff: the console, then the result against the case's
/// record. Deterministic for a given config and data. Returns `(k, σ)`.
pub fn headless_keff<C: CsgCase>(phys: &Physics, cfg: KeffConfig, mut out: impl FnMut(&str)) -> (f64, f64) {
    let mut k = Keff::new(&phys.geometry, &phys.materials, &phys.nuclides, phys.source_box, phys.temperature_k, cfg).with_entropy_box(C::entropy_box());
    for l in CONSOLE_HEADER {
        out(l);
    }
    let mut last = (f64::NAN, f64::NAN);
    while let Some(g) = k.step(&phys.geometry, &phys.materials, &phys.nuclides) {
        out(&console_line(&g.report));
        if let Some(m) = g.report.k_mean {
            last = m;
        }
    }
    out("");
    out(&format!(" k-effective = {:.5} +/- {:.5}   ({:+.0} +/- {:.0} pcm from 1)", last.0, last.1, (last.0 - 1.0) * 1e5, last.1 * 1e5));
    if let Some(r) = C::reference() {
        if let Some((name, kx, s)) = r.experiment {
            out(&format!(" Experiment ({name}): {kx:.4} +/- {s:.4}"));
        }
        out(&format!(" Recorded ({}): {:.5} +/- {:.5} ({:+.0} +/- {:.0} pcm)", r.label, r.k, r.sem, (r.k - 1.0) * 1e5, r.sem * 1e5));
    }
    last
}

/// The rung once its data are processed: what the engine serves.
pub struct Loaded<C: CsgCase> {
    pub phys: Physics,
    pub chain: Chain,
    pub keff: Option<Keff>,
    case: PhantomData<C>,
}

impl<C: CsgCase> LoadedRung for Loaded<C> {
    /// While a k-eigenvalue run is going, the neutron is born at a site of
    /// the run's present fission source (Run mode draws these, gh:#859);
    /// otherwise the Watch chain's next history.
    fn run_next(&mut self) -> History {
        if let Some(k) = self.keff.as_ref().filter(|k| !k.finished()) {
            if let Some(r) = k.source_site(&mut self.chain.rng) {
                self.chain.pending_fission = Some((r, C::FRESH_INCIDENT_EV));
            }
        }
        self.chain.run_next::<C>(&self.phys)
    }
    fn keff_start(&mut self, cfg: KeffConfig) -> Result<Vec<[f32; 2]>, String> {
        let p = &self.phys;
        let k = Keff::new(&p.geometry, &p.materials, &p.nuclides, p.source_box, p.temperature_k, cfg).with_entropy_box(C::entropy_box());
        if k.finished() {
            return Err("the initial source is empty: no fissionable material in the source box".into());
        }
        let sites = if cfg.want_sites { k.initial_sites() } else { Vec::new() };
        self.keff = Some(k);
        Ok(sites)
    }
    fn keff_step(&mut self) -> Option<Generation> {
        let p = &self.phys;
        self.keff.as_mut().and_then(|k| k.step(&p.geometry, &p.materials, &p.nuclides))
    }
    fn keff_finished(&self) -> bool {
        self.keff.as_ref().is_none_or(|k| k.finished())
    }
}

/// A uniform point in the sphere of radius `r` about the origin.
pub fn uniform_in_sphere(rng: &mut u64, r: f64) -> Position {
    use crate::triso::model::uniform;
    loop {
        let p = [0; 3].map(|_| (2.0 * uniform(rng) - 1.0) * r);
        if p[0] * p[0] + p[1] * p[1] + p[2] * p[2] < r * r {
            return Position::new(p[0], p[1], p[2]);
        }
    }
}

/// A uniform point in the cylinder of radius `r` about the z axis, between
/// `z_lo` and `z_hi`.
pub fn uniform_in_cylinder(rng: &mut u64, r: f64, z_lo: f64, z_hi: f64) -> Position {
    use crate::triso::model::uniform;
    let rho = r * (1.0 - 1.0e-9) * uniform(rng).sqrt();
    let t = std::f64::consts::TAU * uniform(rng);
    let z = z_lo + (z_hi - z_lo) * (0.001 + 0.998 * uniform(rng));
    Position::new(rho * t.cos(), rho * t.sin(), z)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    /// A CSG generation crosses to the page with its `k`, its running mean
    /// and a production of `k` × source neutrons, with the entropy it is
    /// given and no counts (the CSG iteration tallies none). The fresh-birth
    /// samplers stay inside their regions.
    #[test]
    fn a_csg_generation_is_reported_with_its_entropy_and_no_counts() {
        let r = CsgGenerationReport { index: 7, active: true, k: 1.25, k_mean: Some((1.2, 0.01)), bank_size: 900 };
        let g = report(&r, 800, Some(5.5));
        assert_eq!((g.index, g.active, g.k, g.k_mean, g.bank_size), (7, true, 1.25, Some((1.2, 0.01)), 900));
        assert_eq!(g.production, 1000.0);
        assert!(g.entropy == Some(5.5) && g.counts.tracked == 0);
        assert!(report(&r, 800, None).entropy.is_none());
        let mut rng = 3u64;
        for _ in 0..2000 {
            let p = uniform_in_sphere(&mut rng, 2.0);
            assert!(p.x * p.x + p.y * p.y + p.z * p.z < 4.0);
            let q = uniform_in_cylinder(&mut rng, 3.0, 1.0, 5.0);
            assert!(q.x * q.x + q.y * q.y < 9.0 && q.z > 1.0 && q.z < 5.0);
        }
    }
}
