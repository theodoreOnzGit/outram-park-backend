//! **The worker side of the EBR-II simulator's MC page** (gh:#1762): the
//! simplified full core (`nee_soon::ebr2_core`, Run 138B's layout, fresh
//! fuel, every subassembly homogenised) on a pool of workers, on the
//! browser pipeline of `dhoby_ghaut::mc_web`, after MSRE's
//! (`crate::msre::web::keff`, gh:#1571/#1578; the steps are the same, with
//! no thermal-scattering law: EBR-II's spectrum is fast).
//!
//! Each worker:
//! 1. processes its share of the [`jobs`] (each nuclide of
//!    `ebr2_core::NUCLIDES` at the study's temperature) through the shared
//!    processed-data cache, posting each product's exact words;
//! 2. takes every other worker's products, relayed by the page;
//! 3. assembles the model ([`KeffWorker::assemble`]): every nuclide built
//!    through the store, the isotope masses from the nuclides' own AWR (as
//!    the native run takes them from the tapes), the homogenised materials
//!    (`Study::transport_materials`) and the geometry;
//! 4. answers the page's source and chunk requests on that model
//!    (`dhoby_ghaut::mc_web::pool::csg_source` / `csg_chunk`, surface
//!    tracking), so a run is `run_keff_csg_par`'s answer for any number of
//!    workers.
//!
//! Research, education and V&V only. A SIMPLIFIED FRESH CORE, not the Run
//! 138B burnup state; NOT VALIDATED.

use dhoby_ghaut::mc_web::keff::KeffConfig;
use dhoby_ghaut::mc_web::pool::{csg_chunk, csg_source, PoolSource};
use dhoby_ghaut::mc_web::processed_cache::{evaluation_key, DataStore, CODE};
use dhoby_ghaut::mc_web::tapes;
use dhoby_ghaut::web_demo::data_cache::{Record, Source};
use nee_soon::ebr2_core::{self as c, Study};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::material::material::Material;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::speed::SpeedTier;
use outram_mc_libs::model::{AtomicMasses, Isotope};
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::tally::mesh::RegularMesh;
use std::collections::BTreeMap;

/// The entropy mesh's bins per axis, over the source box.
pub const ENTROPY_BINS: usize = 8;
/// Histories traced per generation (none are drawn on this page).
pub const TRACED: usize = 0;

/// One processing job: nuclide `index` of `ebr2_core::NUCLIDES` at
/// `temperature_k`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Job {
    pub index: usize,
    pub temperature_k: f64,
}

impl Job {
    /// Its nuclide's name.
    pub fn label(&self) -> String {
        c::NUCLIDES[self.index].0.to_string()
    }

    /// The tape it downloads, relative to `reference-data/endf/`.
    pub fn tape(&self) -> &'static str {
        c::NUCLIDES[self.index].1
    }

    /// Rough cost for dealing the jobs out, s: an ESTIMATE from the HTR-10
    /// pool's native per-job times (U-235 58, U-238 47), 3 for a structural
    /// nuclide, 1 otherwise, as MSRE's. Only the order matters.
    pub fn cost(&self) -> f64 {
        match c::NUCLIDES[self.index].0 {
            "U235" => 58.0,
            "U238" => 47.0,
            n if ["Fe", "Ni", "Cr", "Mo", "Zr", "Na"].iter().any(|p| n.starts_with(p)) => 3.0,
            _ => 1.0,
        }
    }
}

/// The jobs of a study, in the order every worker and the page index them:
/// every nuclide of `ebr2_core::NUCLIDES` at the study's temperature.
pub fn jobs(study: &Study) -> Vec<Job> {
    (0..c::NUCLIDES.len()).map(|index| Job { index, temperature_k: study.temperature_k }).collect()
}

/// The transport settings of a run: the reader's numbers on the library's
/// defaults (URR and RVS on), at the study's temperature.
pub fn run_settings(cfg: &KeffConfig, t_k: f64) -> KeffSettings {
    KeffSettings { n_particles: cfg.n_particles, n_inactive: cfg.n_inactive, n_active: cfg.n_active, seed: cfg.seed, temperature_k: t_k, ..KeffSettings::default() }
}

/// The page's coordinator settings (`KeffPool`): it reads only the seed,
/// the particle count and the generations.
pub fn coordinator_settings(cfg: &KeffConfig) -> KeffSettings {
    run_settings(cfg, c::TEMPERATURE_K)
}

/// The entropy mesh: [`ENTROPY_BINS`]³ over the source box.
pub fn entropy_mesh() -> RegularMesh {
    let b = c::source_box();
    RegularMesh { lower_left: [b.lower.x, b.lower.y, b.lower.z], upper_right: [b.upper.x, b.upper.y, b.upper.z], dimension: [ENTROPY_BINS; 3] }
}

/// The assembled model one worker transports.
pub struct Model {
    pub geometry: Geometry,
    pub materials: Vec<Material>,
    pub nuclides: Vec<Nuclide>,
    pub temperature_k: f64,
}

/// What a worker reports once its model is built.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Assembly {
    pub nuclides: usize,
    pub materials: usize,
    pub secs: f64,
}

/// One worker's state (module docs).
pub struct KeffWorker {
    pub store: DataStore,
    /// Every product this worker holds, by job index.
    products: BTreeMap<usize, Vec<f64>>,
    pub model: Option<Model>,
}

impl Default for KeffWorker {
    fn default() -> Self {
        #[cfg(target_arch = "wasm32")]
        let store = DataStore::browser();
        #[cfg(not(target_arch = "wasm32"))]
        let store = DataStore::memory();
        Self { store, products: BTreeMap::new(), model: None }
    }
}

/// The isotope masses of `nuclides` (named as `ebr2_core::NUCLIDES`), from
/// their own AWR, as the native run reads them from the tapes.
///
/// # Errors
/// A name that is not an isotope's.
pub fn masses_of(nuclides: &[Nuclide]) -> Result<AtomicMasses, String> {
    let mut pairs = Vec::with_capacity(nuclides.len());
    for (k, n) in nuclides.iter().enumerate() {
        let name = c::NUCLIDES[k].0;
        pairs.push((Isotope::from_name(name).map_err(|e| format!("{name}: {e}"))?, n.awr));
    }
    Ok(AtomicMasses::from_awr(pairs, "EBR-II core tapes (ENDF/B-VIII.0)"))
}

impl KeffWorker {
    /// Process job `index` of `study` from its tape bytes: the product's
    /// words (the processed evaluation, as `DataStore::nuclide` makes it)
    /// and where they came from.
    ///
    /// # Errors
    /// The tape cannot be read, or processing failed.
    pub fn process(&mut self, study: &Study, index: usize, bytes: &[u8]) -> Result<(Vec<f64>, Source), String> {
        let job = *jobs(study).get(index).ok_or_else(|| format!("no job {index}"))?;
        let _ = self.store.take_report();
        let (tape, mat) = tapes::read_tape(bytes, c::NUCLIDES[job.index].0)?;
        let tol = SpeedTier::default().data_tolerance();
        let words = self.store.evaluation(bytes, &tape, mat, job.temperature_k, tol, tol, &job.label())?.to_f64s();
        let source = self.store.take_report().0.last().map(|(_, s)| s.clone()).unwrap_or(Source::Processed);
        self.products.insert(index, words.clone());
        Ok((words, source))
    }

    /// Keep job `index`'s product, made by another worker.
    pub fn keep(&mut self, index: usize, words: Vec<f64>) {
        self.products.insert(index, words);
    }

    /// Build the model of `study` from every product (each offered to the
    /// store under its key, then built through it) and `tape(path)`, the
    /// stripped bytes of each tape.
    ///
    /// # Errors
    /// A product is missing, a tape cannot be read, or a build failed.
    pub fn assemble(&mut self, study: &Study, mut tape: impl FnMut(&str) -> Result<Vec<u8>, String>, now_s: impl Fn() -> f64) -> Result<Assembly, String> {
        let t0 = now_s();
        let list = jobs(study);
        let mut nuclides = Vec::with_capacity(list.len());
        let tol = SpeedTier::default().data_tolerance();
        for (i, job) in list.iter().enumerate() {
            let bytes = tape(job.tape())?;
            let words = self.products.get(&i).cloned().ok_or_else(|| format!("{}: no product", job.label()))?;
            let name = c::NUCLIDES[job.index].0;
            let (t, mat) = tapes::read_tape(&bytes, name)?;
            let key = evaluation_key(&self.store.tape_sha(&bytes), mat, job.temperature_k, tol, tol);
            self.store.offer(vec![Record::seal(key, job.label(), CODE.to_string(), 0.0, words)]);
            nuclides.push(self.store.nuclide(&bytes, &t, mat, name, job.temperature_k, SpeedTier::default(), &job.label())?);
        }
        let _ = self.store.take_report();
        self.store.forget_offered();
        self.products.clear();
        let masses = masses_of(&nuclides)?;
        let materials = study.transport_materials(&masses)?;
        let a = Assembly { nuclides: nuclides.len(), materials: materials.len(), secs: now_s() - t0 };
        self.model = Some(Model { geometry: study.geometry(), materials, nuclides, temperature_k: study.temperature_k });
        Ok(a)
    }

    /// The initial source of a run (the page asks one worker).
    ///
    /// # Errors
    /// No model is assembled.
    pub fn source(&self, cfg: &KeffConfig) -> Result<PoolSource, String> {
        let md = self.model.as_ref().ok_or("no model assembled")?;
        Ok(csg_source((&md.geometry, &md.materials, &md.nuclides), c::source_box(), entropy_mesh(), &run_settings(cfg, md.temperature_k)))
    }

    /// Transport one chunk of a generation.
    ///
    /// # Errors
    /// No model is assembled, or the chunk cannot be read.
    pub fn chunk(&self, cfg: &KeffConfig, chunk: &[f64]) -> Result<Vec<f64>, String> {
        let md = self.model.as_ref().ok_or("no model assembled")?;
        csg_chunk((&md.geometry, &md.materials, &md.nuclides), &run_settings(cfg, md.temperature_k), chunk)
    }
}

/// A tape's stripped bytes from `reference-data/endf/` (natively, as the
/// browser gets it, covariances stripped).
///
/// # Errors
/// The tape is not in the workspace.
#[cfg(not(target_arch = "wasm32"))]
pub fn native_tape(path: &str) -> Result<Vec<u8>, String> {
    let p = njoy_outram_park_fork::reference_data::reference_endf(path).ok_or_else(|| format!("{path}: not in reference-data/endf (is the reference-data/ace submodule fetched?)"))?;
    let raw = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    Ok(tapes::strip_covariances(&raw))
}

/// `--prepare-web-data <dir>`: every tape the page can download, stripped
/// and compressed under its served name. CLI glue.
///
/// # Errors
/// A tape is missing or cannot be written.
#[cfg(not(target_arch = "wasm32"))]
pub fn prepare_web_data(dir: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    for (_, t) in c::NUCLIDES {
        let z = tapes::compress(&native_tape(t)?);
        let out = dir.join(served_name(t));
        std::fs::write(&out, &z).map_err(|e| format!("{}: {e}", out.display()))?;
    }
    Ok(())
}

/// The URL a tape is served under, beside the page.
pub fn tape_url(path: &str) -> String {
    format!("data/{}", served_name(path))
}

/// A tape's file name in `data/`: its wire name, by its own file name.
pub fn served_name(path: &str) -> String {
    tapes::wire_name(path.rsplit('/').next().unwrap_or(path))
}

// ─── Messages ────────────────────────────────────────────────────────────────

/// A request to a k_eff worker.
#[derive(Clone)]
pub enum KReq {
    Process { study: Study, jobs: Vec<usize> },
    Product { job: usize, data: bugis::web_demo::link::Floats },
    Assemble { study: Study },
    Source { cfg: KeffConfig, epoch: u32 },
    Chunk { cfg: KeffConfig, data: bugis::web_demo::link::Floats, epoch: u32 },
    /// The material map of a slice of `study`'s assembled geometry.
    Raster { study: Study, view: View, px: usize },
}

/// What a k_eff worker posts.
#[derive(Clone)]
pub enum KEv {
    Started { job: usize },
    Product { job: usize, secs: f64, data: bugis::web_demo::link::Floats, cached_ms: Option<f64> },
    Assembled(Assembly),
    Source { sites: bugis::web_demo::link::Floats, seed: u64, mesh: [f64; 9], epoch: u32 },
    ChunkDone { data: bugis::web_demo::link::Floats, secs: f64, epoch: u32 },
    Failed { message: String, epoch: u32 },
    Raster { view: View, w: usize, h: usize, ids: Vec<u8> },
}

/// Which slice the page draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    /// x-y at the core mid-plane, the whole core.
    Plan,
    /// x-y of rows 1-7, zoomed.
    Core,
    /// x-z through the axis.
    Elevation,
}

impl View {
    pub const ALL: [View; 3] = [View::Plan, View::Core, View::Elevation];
    pub fn code(self) -> f64 {
        match self {
            View::Plan => 0.0,
            View::Core => 1.0,
            View::Elevation => 2.0,
        }
    }
    pub fn from_code(x: f64) -> View {
        match x as i64 {
            1 => View::Core,
            2 => View::Elevation,
            _ => View::Plan,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            View::Plan => "Plan (x-y)",
            View::Core => "Rows 1-7",
            View::Elevation => "Elevation (x-z)",
        }
    }
    /// The slice's centre, basis and full widths, cm.
    pub fn frame(self) -> (outram_mc_libs::geometry::position::Position, outram_mc_libs::geometry::plot::PlotBasis, [f64; 2]) {
        use outram_mc_libs::geometry::plot::PlotBasis;
        use outram_mc_libs::geometry::position::Position;
        let r = c::R_OUTER_CM + 4.0;
        match self {
            View::Plan => (Position::ZERO, PlotBasis::Xy, [2.0 * r, 2.0 * r]),
            View::Core => {
                let z = 7.5 * c::PITCH_CM;
                (Position::ZERO, PlotBasis::Xy, [2.0 * z, 2.0 * z])
            }
            View::Elevation => (Position::ZERO, PlotBasis::Xz, [2.0 * r, c::COLUMN_CM + 8.0]),
        }
    }
}

/// The material map of `view` of `study`'s geometry, `px` pixels across:
/// per pixel the material's index + 1, 0 void, 254 overlap, 255 outside.
/// Sampled from the assembled CSG (the drawing rule).
pub fn raster(study: &Study, view: View, px: usize) -> (usize, usize, Vec<u8>) {
    use outram_mc_libs::geometry::plot::{SliceHit, SlicePlot};
    let (origin, basis, width) = view.frame();
    let w = px.clamp(16, 1024);
    let h = ((w as f64) * width[1] / width[0]).round().clamp(16.0, 2048.0) as usize;
    let plot = SlicePlot { origin, basis, width, pixels: [w, h], level: None, show_overlaps: true, meshlines: None };
    let g = study.geometry();
    let ids = plot
        .id_map(&g)
        .hits
        .iter()
        .map(|hit| match *hit {
            SliceHit::Found { material: Some(i), .. } => (i + 1).min(253) as u8,
            SliceHit::Found { material: None, .. } => 0,
            SliceHit::Overlap => 254,
            SliceHit::NotFound => 255,
        })
        .collect();
    (w, h, ids)
}

/// A study as wire numbers `[temperature K, control insertion]`.
pub fn study_to_vec(s: &Study) -> [f64; 2] {
    [s.temperature_k, s.control_insertion]
}

/// The inverse of [`study_to_vec`].
pub fn study_from_slice(v: &[f64]) -> Study {
    let d = Study::default();
    Study { temperature_k: v.first().copied().unwrap_or(d.temperature_k), control_insertion: v.get(1).copied().unwrap_or(d.control_insertion) }
}

/// A run's settings as wire numbers `[n, inactive, active, seed]`.
pub fn cfg_to_vec(c: &KeffConfig) -> [f64; 4] {
    [c.n_particles as f64, c.n_inactive as f64, c.n_active as f64, c.seed as f64]
}

/// The inverse of [`cfg_to_vec`].
pub fn cfg_from_slice(v: &[f64]) -> KeffConfig {
    let g = |i: usize| v.get(i).copied().unwrap_or(0.0);
    KeffConfig { n_particles: g(0) as usize, n_inactive: g(1) as usize, n_active: g(2) as usize, seed: g(3) as u64, point_source: false, want_sites: false }
}

impl KeffWorker {
    /// Answer `req` natively, posting as it goes.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn answer_native(&mut self, req: KReq, post: &mut impl FnMut(KEv)) {
        let now = std::time::Instant::now;
        match req {
            KReq::Process { study, jobs: js } => {
                let list = jobs(&study);
                for j in js {
                    post(KEv::Started { job: j });
                    let t = now();
                    let Some(job) = list.get(j) else { return post(KEv::Failed { message: format!("no job {j}"), epoch: 0 }) };
                    let bytes = match native_tape(job.tape()) {
                        Ok(b) => b,
                        Err(message) => return post(KEv::Failed { message, epoch: 0 }),
                    };
                    match self.process(&study, j, &bytes) {
                        Ok((w, src)) => post(KEv::Product {
                            job: j,
                            secs: t.elapsed().as_secs_f64(),
                            data: bugis::web_demo::link::Floats::from_vec(w),
                            cached_ms: match src {
                                Source::Cached { created_ms, .. } => Some(created_ms),
                                Source::Processed => None,
                            },
                        }),
                        Err(message) => return post(KEv::Failed { message, epoch: 0 }),
                    }
                }
            }
            KReq::Assemble { study } => {
                let t0 = now();
                match self.assemble(&study, native_tape, move || t0.elapsed().as_secs_f64()) {
                    Ok(a) => post(KEv::Assembled(a)),
                    Err(message) => post(KEv::Failed { message, epoch: 0 }),
                }
            }
            other => {
                for e in self.answer_now(other) {
                    post(e);
                }
            }
        }
    }

    /// The requests answered at once on either platform: a relayed product,
    /// a source, a chunk, a raster.
    pub fn answer_now(&mut self, req: KReq) -> Vec<KEv> {
        match req {
            KReq::Product { job, data } => {
                self.keep(job, data.to_vec());
                Vec::new()
            }
            KReq::Source { cfg, epoch } => match self.source(&cfg) {
                Ok(s) => vec![KEv::Source {
                    sites: bugis::web_demo::link::Floats::from_vec(outram_mc_libs::physics::transport_csg::distributed::SourceSite::encode(&s.sites)),
                    seed: s.seed,
                    mesh: dhoby_ghaut::mc_web::pool::mesh_words(&s.mesh),
                    epoch,
                }],
                Err(message) => vec![KEv::Failed { message, epoch }],
            },
            KReq::Chunk { cfg, data, epoch } => {
                let t = bugis::web_demo::platform::now_s();
                match self.chunk(&cfg, &data.to_vec()) {
                    Ok(out) => vec![KEv::ChunkDone { data: bugis::web_demo::link::Floats::from_vec(out), secs: bugis::web_demo::platform::now_s() - t, epoch }],
                    Err(message) => vec![KEv::Failed { message, epoch }],
                }
            }
            KReq::Raster { study, view, px } => {
                let (w, h, ids) = raster(&study, view, px);
                vec![KEv::Raster { view, w, h, ids }]
            }
            KReq::Process { .. } | KReq::Assemble { .. } => vec![KEv::Failed { message: "processing is the platform's".into(), epoch: 0 }],
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    /// The jobs are every nuclide once at the study's temperature, every
    /// tape is served under its own name, and the wire round-trips.
    #[test]
    fn jobs_tapes_and_wire() {
        let s = Study { temperature_k: 600.0, control_insertion: 0.25 };
        let j = jobs(&s);
        assert_eq!(j.len(), c::NUCLIDES.len());
        assert!(j.iter().all(|x| x.temperature_k == 600.0 && x.cost() > 0.0));
        let served: std::collections::BTreeSet<String> = c::NUCLIDES.iter().map(|(_, t)| served_name(t)).collect();
        assert_eq!(served.len(), c::NUCLIDES.len());
        assert_eq!(study_from_slice(&study_to_vec(&s)), s);
        assert_eq!(study_from_slice(&[]), Study::default());
        let k = KeffConfig { n_particles: 4000, n_inactive: 20, n_active: 80, seed: 7, point_source: false, want_sites: false };
        assert_eq!(cfg_from_slice(&cfg_to_vec(&k)), k);
        for v in View::ALL {
            assert_eq!(View::from_code(v.code()), v);
        }
        assert_eq!(tape_url("../ace/endf/endf-b-viii.0/n-042_Mo_092-ENDF8.0.endf"), "data/n-042_Mo_092-ENDF8.0.endf.zz");
    }

    /// The drawing is sampled from the assembled geometry: the plan finds
    /// the driver, the blanket, the reflector and sodium, the core view
    /// the control rods' B4C at the mid-plane, and nothing overlaps.
    #[test]
    fn the_raster_draws_the_assembled_core() {
        let s = Study::default();
        let has = |ids: &[u8], m: usize| ids.contains(&((m + 1) as u8));
        let (w, h, ids) = raster(&s, View::Plan, 160);
        assert_eq!(ids.len(), w * h);
        for m in [c::DRIVER, c::BLANKET, c::REFLECTOR, c::SODIUM] {
            assert!(has(&ids, m), "{} missing from the plan", c::MATERIALS[m]);
        }
        assert!(!ids.contains(&254));
        let (_, _, core) = raster(&s, View::Core, 160);
        assert!(has(&core, c::B4C) && has(&core, c::HALF) && has(&core, c::ROD_FUEL));
        let (_, _, el) = raster(&s, View::Elevation, 120);
        assert!(!el.contains(&254));
    }

    /// A worker without a model refuses runs; assembly without tapes fails.
    #[test]
    fn a_worker_without_a_model_refuses_runs() {
        let mut w = KeffWorker::default();
        let cfg = KeffConfig { n_particles: 10, n_inactive: 1, n_active: 1, seed: 1, point_source: false, want_sites: false };
        assert!(matches!(w.answer_now(KReq::Source { cfg, epoch: 3 })[0], KEv::Failed { epoch: 3, .. }));
        assert!(w.assemble(&Study::default(), |_| Err("no tapes".into()), || 0.0).is_err());
    }
}
