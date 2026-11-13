//! **The whole HTR-10 core, live, on a pool of Web Workers** (gh:#786).
//!
//! ~~The model is the recorded runs' exactly:
//! `nee_soon::htr10_rmc::core_model::assemble_explicit_triso(14, N, 0)` (the
//! bed delta-tracked against `keff_vs_height::bed_majorant`, everything else
//! surface-tracked), the material set
//! `htr10_material_set(layout, Htr10MaterialConfig::benchmark_default(300.15))`,
//! and the nuclide set of `Htr10DataConfig::default()`~~ **CHANGED 2026-11-03
//! (gh:#1139, #1142):** the model is `nee_soon`'s `ParametricStudy` ([`study`]):
//! the recorded runs' geometry `assemble_explicit_triso_with(14, N, 0, ..)`
//! with the temperature-zone slots placed (the graphite balls and each
//! reflector region on slots of their own; the same cells), each of the eight
//! zones at its own temperature, and each (nuclide, temperature) processed
//! exactly: ENDF/B-VIII.0, tolerance 1e-3, URR and DBRC on, natural carbon,
//! helium (density from the coolant's temperature), real Ni/Fe rod steel,
//! **every** bound thermal law by LEAPR at its zone's temperature (the record
//! read graphite and SiC from the tapes: at 300.15 K that is graphite at the
//! tape's 296 K). Bed delta-tracked against the study's majorant (the bed's
//! materials and the moderator balls' slot), everything else surface-tracked.
//! The run is `run_keff_csg_par`'s power iteration, which the recorded runs
//! used, taken apart by `outram_mc_libs::physics::transport_csg::distributed`.
//!
//! **Why a pool, and how it shares the work** (no `SharedArrayBuffer` on
//! GitHub Pages, so workers share messages only):
//!
//! 1. **Data, split by nuclide.** `nee_soon::htr10_rmc::data_jobs` lists 36
//!    jobs (31 tapes, 5 thermal laws). The page deals them out by cost
//!    ([`schedule`], longest first); each worker processes its own
//!    ([`CoreReq::Process`]) and posts each product as numbers; the page
//!    relays every product to every other worker. Then each worker rebuilds
//!    all 38 nuclides from the products and its own copy of each tape (the
//!    cheap half, [`CoreReq::Assemble`]), builds the core and the majorant.
//! 2. **Histories, split per generation.** The page holds the power
//!    iteration (no data needed), cuts each generation into chunks, deals
//!    them to idle workers, and reduces the results in history order
//!    ([`pool`]). The answer does not depend on the number of workers, bit
//!    for bit.
//!
//! The first histories of every generation are traced, and those tracks are
//! what the Watch view animates: neutrons of the run itself.

pub mod pool;
pub mod pour;
pub mod random_bed;
pub mod screen;
#[cfg(not(target_arch = "wasm32"))]
pub mod bake;

use crate::keff::KeffConfig;
use dhoby_ghaut::web_demo::link::Floats;
use nee_soon::htr10_rmc::core_model::AssembledCore;
use nee_soon::htr10_rmc::data::Tape;
use crate::processed_cache::DataStore;
use dhoby_ghaut::web_demo::data_cache::Source;
use nee_soon::htr10_rmc::data_jobs::{process_job_at, JobProduct, ProcessingJob, TemperedJob, TOLERANCE};
use nee_soon::htr10_rmc::keff_vs_height;
use nee_soon::htr10_rmc::parametric::{ParametricStudy, StudyBed, StudyPlan};
use nee_soon::htr10_rmc::zones::ZoneTemperatures;
use njoy_outram_park_fork::endf::tape::Tape as EndfTape;
use outram_mc_libs::material::material::Material;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
use outram_mc_libs::physics::compute::{ComputeType, ThreadCount};
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::track_output::TrackEvent;
use outram_mc_libs::physics::transport_csg::distributed::{
    transport_chunk, ChunkResult, DistributedPowerIteration, GenerationChunk,
};
use outram_mc_libs::tally::mesh::RegularMesh;

/// The transport settings' run-wide temperature, K (the recorded runs').
/// ~~Temperature of every material and nuclide~~ **CORRECTED 2026-11-03
/// (gh:#1142):** each material carries its zone's temperature, which is what
/// transport reads at a collision; this one only fills `KeffSettings`.
pub const TEMP_K: f64 = 300.15;
/// Rings of the bed tiling, as the recorded runs.
pub const RINGS: usize = 14;
/// Histories traced (every state kept) at the start of each generation.
pub const TRACED_PER_GENERATION: usize = 4;
/// The recorded runs' seed (`htr10_seker_2026_10_07_10k/RUN_PARAMETERS.md`).
pub const RECORD_SEED: u64 = 20_260_917;

/// The live run's default: 1000 × [5 + 20] on the recorded seed (the
/// prediction is in `verification_and_validation/htr10_full_core_web/`).
pub const DEFAULT_RUN: KeffConfig = KeffConfig {
    n_particles: 1000,
    n_inactive: 5,
    n_active: 20,
    seed: RECORD_SEED,
    point_source: false,
    want_sites: false,
};

/// The recorded runs' settings, the "Recorded settings" preset (gh:#876
/// item 5): 10 000 × [5 + 135] on [`RECORD_SEED`]
/// (`htr10_seker_2026_10_07_10k/RUN_PARAMETERS.md`).
pub const RECORD_RUN: KeffConfig = KeffConfig {
    n_particles: 10_000,
    n_inactive: 5,
    n_active: 135,
    seed: RECORD_SEED,
    point_source: false,
    want_sites: false,
};

/// **The study the core view builds** (gh:#1139): bed code `code` (N
/// layers of the lattice, or [`random_bed::DEM_BED`]), the rods, the eight
/// zone temperatures; every other setting `ParametricStudy`'s default
/// (every bound law by LEAPR at its zone's temperature).
///
/// # Errors
/// The DEM pour cannot be read.
pub fn study(code: usize, rods: &[f64; random_bed::N_RODS], temps: &ZoneTemperatures) -> Result<ParametricStudy, String> {
    let bed = if random_bed::is_dem(code) {
        let (centres, is_fuel) = random_bed::explicit_bed(code)?;
        StudyBed::Explicit { centres, is_fuel }
    } else {
        StudyBed::Seker { layers: code.clamp(10, 20) }
    };
    Ok(ParametricStudy {
        bed,
        rod_insertions: *rods,
        temperatures: *temps,
        ..ParametricStudy::default()
    })
}

/// The plan at these zone temperatures: materials, nuclide array, jobs (it
/// depends on the temperatures only, not on the bed or the rods).
pub fn plan(temps: &ZoneTemperatures) -> Result<StudyPlan, String> {
    study(12, &random_bed::WITHDRAWN, temps)?.plan().map_err(|e| e.to_string())
}

/// The jobs at these zone temperatures, in the order every worker and the
/// page index them.
pub fn jobs_at(temps: &ZoneTemperatures) -> Result<Vec<TemperedJob>, String> {
    Ok(plan(temps)?.jobs)
}

/// The jobs with every zone at the record's 300.15 K.
pub fn jobs() -> Result<Vec<TemperedJob>, String> {
    jobs_at(&ZoneTemperatures::record())
}

/// Rough cost of a job, s, for dealing them out and for the progress bar:
/// the native bake's per-job times (2026-10-07, i9-13900K, 5 jobs at once on
/// 5 threads of a loaded machine; `verification_and_validation/
/// htr10_full_core_web/logs/bake_native.log`). Only the order matters.
pub fn job_cost(job: &TemperedJob) -> f64 {
    match job.job.label().as_str() {
        "U235" => 58.0,
        "C-in-SiC S(a,b)" => 57.0,
        "U238" => 47.0,
        "graphite S(a,b)" => 32.0,
        "U-in-UO2 S(a,b)" => 28.0,
        "Si28" => 25.0,
        "O-in-UO2 S(a,b)" => 24.0,
        "Si29" => 22.0,
        "Si-in-SiC S(a,b)" => 21.0,
        "Fe54" | "Fe56" => 3.4,
        "Ni58" => 2.7,
        "Ni60" | "Fe58" => 1.9,
        "Mn55" | "Cr50" | "Cr52" | "Cr53" | "Fe57" => 1.1,
        _ => 0.4,
    }
}

/// Deal jobs (by `costs`) to `workers`: longest first, each to the least
/// loaded worker so far (LPT). Ties go to the lower worker index.
pub fn schedule(costs: &[f64], workers: usize) -> Vec<Vec<usize>> {
    let workers = workers.max(1);
    let mut order: Vec<usize> = (0..costs.len()).collect();
    order.sort_by(|&a, &b| costs[b].total_cmp(&costs[a]).then(a.cmp(&b)));
    let mut load = vec![0.0_f64; workers];
    let mut out = vec![Vec::new(); workers];
    for j in order {
        let w = (0..workers)
            .min_by(|&a, &b| load[a].total_cmp(&load[b]).then(a.cmp(&b)))
            .unwrap_or(0);
        load[w] += costs[j];
        out[w].push(j);
    }
    out
}

/// The transport settings of a run (as the recorded runs': analog, the
/// default delta-region estimator; the thread count is not used here).
pub fn run_settings(cfg: &KeffConfig) -> KeffSettings {
    KeffSettings {
        n_particles: cfg.n_particles,
        n_inactive: cfg.n_inactive,
        n_active: cfg.n_active,
        seed: cfg.seed,
        temperature_k: TEMP_K,
        compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(1)),
        ..KeffSettings::default()
    }
}

/// Where the browser downloads a tape (`--prepare-web-data` writes it).
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // the browser fetches
pub fn wire_url(t: &Tape) -> String {
    format!("data/{}", crate::tapes::wire_name(t.file))
}

/// What the page asks of a worker.
pub enum CoreReq {
    /// Process these jobs (indices into [`jobs_at`] at `temps`, K per zone),
    /// posting each product.
    Process { jobs: Vec<usize>, temps: [f64; 8] },
    /// Another worker's product.
    Product { job: usize, data: Floats },
    /// Every product is here: build the nuclides, the materials, the core
    /// at `layers` and the bed majorant, every zone at `temps` (K per zone,
    /// gh:#1142).
    Assemble { layers: usize, temps: [f64; 8] },
    /// Rebuild the core at another layer count (nothing else changes).
    Layers { layers: usize },
    /// Rebuild the core with the control rods at these insertions (gh:#1025;
    /// nothing else changes).
    Rods { rods: [f64; random_bed::N_RODS] },
    /// Sample a run's initial source. `epoch` is the run's (gh:#817): the
    /// reply carries it back, so a reply to a replaced run is recognised.
    Source { cfg: KeffConfig, epoch: u32 },
    /// Transport one chunk of a generation (`GenerationChunk::to_f64s`), for
    /// the run `epoch`.
    Chunk {
        cfg: KeffConfig,
        data: Floats,
        epoch: u32,
    },
    /// The reader's DEM pour (gh:#1244): start one, or step it a chunk
    /// (`dem_web`'s engine, [`pour::engine`]). Sent to the page's own worker.
    Pour(pour::engine::Request),
    /// The same on the GPU (gh:#1250): `outram_park_fork_liggghts::
    /// htr10_fill::Htr10GpuFill`, WebGPU in the browser. Replies as
    /// [`CoreEv::Pour`], or an error when there is no WebGPU.
    PourGpu(pour::engine::Request),
    /// Measure a pour's centres (m, flattened): packing fractions and g(r)
    /// ([`pour::diagnostics`]); the reply is [`CoreEv::PourDiagnostics`].
    PourDiagnostics { centres_m: Floats },
    /// The bed [`random_bed::USER_POUR`] means from now on
    /// ([`pour::PouredBed::to_wire`]); no reply. Then [`CoreReq::Layers`]
    /// rebuilds the core on it.
    PourBed {
        centres_cm: Floats,
        fuel: Vec<f64>,
        version: u32,
        cut: usize,
        fuel_fraction: f64,
    },
}

impl CoreReq {
    /// The bed a [`CoreReq::PourBed`] carries.
    ///
    /// # Errors
    /// Not a `PourBed`, or a malformed one.
    pub fn poured_bed(&self) -> Result<pour::PouredBed, String> {
        match self {
            CoreReq::PourBed { centres_cm, fuel, version, cut, fuel_fraction } => {
                pour::PouredBed::from_wire(&centres_cm.to_vec(), fuel, *version, *cut, *fuel_fraction)
            }
            _ => Err("not a poured bed".into()),
        }
    }
    /// The request that hands `b` to a worker.
    pub fn pour_bed(b: &pour::PouredBed) -> Self {
        let (cm, fuel) = b.to_wire();
        CoreReq::PourBed {
            centres_cm: Floats::from_vec(cm),
            fuel,
            version: b.version,
            cut: b.cut,
            fuel_fraction: b.fuel_fraction,
        }
    }
}

/// The epoch of a request that belongs to no run (processing, assembly).
pub const NO_RUN: u32 = 0;

/// What a worker built, and what it cost.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AssemblyReport {
    pub layers: usize,
    /// Downloading and reading every tape, s.
    pub tapes_s: f64,
    /// Rebuilding the 38 nuclides from the products, s.
    pub nuclides_s: f64,
    pub geometry_s: f64,
    pub majorant_s: f64,
    /// This worker's wasm memory, MB (NaN natively).
    pub memory_mb: f64,
}

// The wire format serves the browser (and the tests).
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
impl AssemblyReport {
    fn words(&self) -> [f64; 6] {
        [
            self.layers as f64,
            self.tapes_s,
            self.nuclides_s,
            self.geometry_s,
            self.majorant_s,
            self.memory_mb,
        ]
    }
    fn from_words(w: &[f64]) -> Result<Self, String> {
        if w.len() != 6 {
            return Err("assembly report: wrong length".into());
        }
        Ok(Self {
            layers: w[0] as usize,
            tapes_s: w[1],
            nuclides_s: w[2],
            geometry_s: w[3],
            majorant_s: w[4],
            memory_mb: w[5],
        })
    }
}

/// What a worker tells the page.
pub enum CoreEv {
    JobStarted {
        job: usize,
    },
    /// A finished job's product ([`JobProduct::to_f64s`]), and whether it was
    /// processed now or read from the browser's cache (gh:#818).
    Product {
        job: usize,
        secs: f64,
        data: Floats,
        source: Source,
    },
    /// Something the page must show about the cache (gh:#818).
    DataNote(String),
    Assembled(AssemblyReport),
    /// A run's initial source (`SourceSite::encode`), the source stream's
    /// state after it, and the entropy mesh (`[lower×3, upper×3, dims×3]`),
    /// for the run `epoch`.
    Source {
        sites: Floats,
        seed: u64,
        mesh: [f64; 9],
        epoch: u32,
    },
    /// A chunk's result (`ChunkResult::to_f64s`, tracks thinned), for the
    /// run `epoch`.
    ChunkDone {
        data: Floats,
        secs: f64,
        epoch: u32,
    },
    /// A request failed in a worker that is still alive and still holds its
    /// data (gh:#817): a download, a decode, a chunk. `epoch` is the run's,
    /// or [`NO_RUN`]. The page decides whether that fails the pool (while
    /// the data are being built) or only the run.
    Failed { message: String, epoch: u32 },
    /// The reader's pour, a chunk on (gh:#1244).
    Pour(pour::engine::Event),
    /// A pour's diagnostics ([`pour::PourDiagnostics::to_words`]).
    PourDiagnostics(Vec<f64>),
}

/// The model a worker transports through.
pub struct CoreModel {
    pub nuclides: Vec<Nuclide>,
    pub materials: Vec<Material>,
    /// The plan the model was built from, and its zone temperatures
    /// (gh:#1142): a rod or bed change rebuilds the core on them.
    pub plan: StudyPlan,
    pub temps: ZoneTemperatures,
    pub majorant: Majorant,
    pub core: AssembledCore,
    pub layers: usize,
    /// The control rods' insertions the core is built with (gh:#1025).
    pub rods: [f64; random_bed::N_RODS],
}

/// One worker's state.
#[derive(Default)]
pub struct CoreWorker {
    /// Every job's product as it arrives (own and relayed).
    pub products: Vec<Option<Floats>>,
    pub model: Option<CoreModel>,
    /// Natively, what this worker processed (gh:#818); in the browser the
    /// cache is IndexedDB, read per job.
    #[cfg(not(target_arch = "wasm32"))]
    pub cache: DataStore,
    /// The reader's DEM pour, when this is the page's own worker (gh:#1244).
    pub pour: pour::engine::Engine,
    /// The GPU pour natively (gh:#1250; in the browser it lives in the
    /// worker's `GPU_POUR`, because the browser's GPU objects are not `Send`).
    #[cfg(not(target_arch = "wasm32"))]
    pub gpu_pour: Option<(u32, outram_park_fork_liggghts::htr10_fill::Htr10GpuFill)>,
}

/// Serve a GPU pour request natively (blocking on the device), replying as
/// the CPU pour does.
#[cfg(not(target_arch = "wasm32"))]
pub fn serve_gpu_pour(w: &mut CoreWorker, r: pour::engine::Request, send: &mut impl FnMut(CoreEv)) {
    use outram_park_fork_liggghts::htr10_fill::Htr10GpuFill;
    use pour::engine::{Event, Request};
    match r {
        Request::Start { id, n, seed } => match Htr10GpuFill::new(pour::engine::settings(n, seed)) {
            Ok(f) => {
                let p = f.progress();
                send(CoreEv::Pour(Event::Progress(pour::gpu_snapshot(id, &f, p, 0.0))));
                w.gpu_pour = Some((id, f));
            }
            Err(e) => send(CoreEv::Pour(Event::Error(format!("the GPU pour could not start: {e}")))),
        },
        Request::Step { id, steps } => {
            if let Some((pid, f)) = w.gpu_pour.as_mut().filter(|(pid, _)| *pid == id) {
                let t0 = dhoby_ghaut::web_demo::platform::now_s();
                let p = f.advance(steps.max(1));
                let ms = (dhoby_ghaut::web_demo::platform::now_s() - t0) * 1000.0 / steps.max(1) as f64;
                send(CoreEv::Pour(Event::Progress(pour::gpu_snapshot(*pid, f, p, ms))));
            }
        }
    }
}

/// **Run job `i` through the shared cache** (gh:#818): a tape's RECONR +
/// BROADR + PURR product (under the same key every demo uses for that tape
/// and settings) or a law, read back from `store` when it holds it, else
/// processed and handed to `store` to be saved. The product is
/// [`run_job`]'s, bit for bit (`a_cached_job_is_the_processed_job`).
/// Returns the product's words and where it came from.
///
/// # Errors
///
/// The tape is missing or unreadable, or processing failed.
pub fn run_job_cached(
    i: usize,
    temps: &ZoneTemperatures,
    tape: Option<&[u8]>,
    store: &mut DataStore,
) -> Result<(Floats, Source), String> {
    let jobs = jobs_at(temps)?;
    let job = jobs.get(i).ok_or("no such job")?;
    let cfg = ParametricStudy::default().data;
    let t_k = job.kelvin();
    let label = job.job.label();
    let t = match tape {
        Some(b) => Some(EndfTape::read(std::io::Cursor::new(b)).map_err(|e| format!("{label}: {e}"))?),
        None => None,
    };
    let product = match (&job.job, tape, t.as_ref()) {
        (ProcessingJob::Tape { .. }, Some(b), Some(t)) => {
            let mat = *t
                .materials()
                .first()
                .ok_or_else(|| format!("{label}: no material on the tape"))?;
            JobProduct::Nuclide(store.evaluation(b, t, mat, t_k, TOLERANCE, TOLERANCE, &label)?)
        }
        (ProcessingJob::Law(_), _, _) => {
            let source = match tape {
                Some(b) => store.tape_sha(b),
                None => format!("leapr-{label}"),
            };
            let key = crate::processed_cache::law_key(&source, 0, t_k, &label);
            JobProduct::Law(store.law(&key, &label, || match process_job_at(&job.job, &cfg, t.as_ref(), job.temperature) {
                Ok(JobProduct::Law(l)) => Ok(l),
                Ok(JobProduct::Nuclide(_)) => Err(format!("{label}: not a law")),
                Err(e) => Err(e.to_string()),
            })?)
        }
        _ => return Err(format!("{label}: its tape was not supplied")),
    };
    let source = store.sources.last().map_or(Source::Processed, |(_, s)| s.clone());
    Ok((Floats::from_vec(product.to_f64s()), source))
}

/// Run job `i` from its tape's bytes (covariance-stripped, inflated), or
/// none for a LEAPR law, with no cache: what the native bake runs, and the
/// reference [`run_job_cached`] is tested against.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub fn run_job(i: usize, temps: &ZoneTemperatures, tape: Option<&[u8]>) -> Result<Floats, String> {
    let jobs = jobs_at(temps)?;
    let job = jobs.get(i).ok_or("no such job")?;
    let t = match tape {
        Some(b) => Some(
            EndfTape::read(std::io::Cursor::new(b)).map_err(|e| format!("{}: {e}", job.label()))?,
        ),
        None => None,
    };
    // `nee_soon`'s `process_job_at` at the job's temperature: a tape job is
    // route 4, `Nuclide::process_evaluation`, i.e. njoy's `process` (gh:#931),
    // which `run_job_cached`'s store makes too since gh:#932; a law is LEAPR's
    // at the zone's temperature (gh:#1138).
    let p = process_job_at(&job.job, &ParametricStudy::default().data, t.as_ref(), job.temperature)
        .map_err(|e| e.to_string())?;
    Ok(Floats::from_vec(p.to_f64s()))
}

/// Thin a result's tracks to what is drawn: surface crossings removed (the
/// flight is straight between collisions), the rest kept.
pub fn thin_tracks(r: &mut ChunkResult) {
    for (_, t) in &mut r.tracks {
        t.states.retain(|s| s.event != TrackEvent::SurfaceCrossing);
    }
}

impl CoreWorker {
    /// Store a product (own or relayed).
    pub fn store(&mut self, job: usize, data: Floats) {
        if self.products.len() <= job {
            self.products.resize(job + 1, None);
        }
        self.products[job] = Some(data);
    }

    /// Build the model from every product and `tape(t)` (the inflated,
    /// stripped bytes of each tape), every zone at `temps` (gh:#1142).
    /// Returns the report without the tape time, which the caller measures.
    pub fn assemble(
        &mut self,
        layers: usize,
        temps: &ZoneTemperatures,
        mut tape: impl FnMut(&Tape) -> Result<Vec<u8>, String>,
    ) -> Result<AssemblyReport, String> {
        let now = dhoby_ghaut::web_demo::platform::now_s;
        temps.check()?;
        let study = study(layers, &random_bed::WITHDRAWN, temps)?;
        let plan = study.plan().map_err(|e| e.to_string())?;
        let n_jobs = plan.jobs.len();
        if self.products.len() < n_jobs || self.products.iter().take(n_jobs).any(Option::is_none) {
            return Err("not every product has arrived".into());
        }
        self.model = None;
        let t0 = now();
        let products = &self.products;
        let nuclides = study
            .assemble_nuclides(
                &plan,
                |i| {
                    let v = products[i].as_ref().map(Floats::to_vec).unwrap_or_default();
                    JobProduct::from_f64s(&v)
                },
                |t| {
                    let b = tape(t).map_err(nee_soon::htr10_rmc::data::Htr10DataError::InvalidConfig)?;
                    EndfTape::read(std::io::Cursor::new(b)).map_err(|e| {
                        nee_soon::htr10_rmc::data::Htr10DataError::InvalidConfig(format!("{}: {e}", t.file))
                    })
                },
            )
            .map_err(|e| e.to_string())?;
        // The relayed copies are no longer needed: free them.
        self.products.clear();
        let t1 = now();
        // `layers` is a bed code: N for the lattice, 0 for the DEM bed.
        let core = study.core(&plan)?;
        let t2 = now();
        let majorant = ParametricStudy::majorant(&plan, &nuclides);
        let t3 = now();
        self.model = Some(CoreModel {
            nuclides,
            materials: plan.materials.clone(),
            plan,
            temps: *temps,
            majorant,
            core,
            layers,
            rods: random_bed::WITHDRAWN,
        });
        Ok(AssemblyReport {
            layers,
            tapes_s: 0.0,
            nuclides_s: t1 - t0,
            geometry_s: t2 - t1,
            majorant_s: t3 - t2,
            memory_mb: dhoby_ghaut::web_demo::pool::wasm_memory_mb().unwrap_or(f64::NAN),
        })
    }

    /// Rebuild the core for bed code `layers` ([`random_bed`]: N layers of
    /// the lattice, or 0 for the DEM random bed).
    pub fn set_layers(&mut self, layers: usize) -> Result<(), String> {
        let m = self.model.as_mut().ok_or("the model is not built")?;
        // The reader's pour is rebuilt every time: the page asks only when
        // it has handed over a new one.
        if m.layers != layers || layers == random_bed::USER_POUR {
            m.core = study(layers, &m.rods, &m.temps)?.core(&m.plan)?;
            m.layers = layers;
        }
        Ok(())
    }

    /// Rebuild the core with the control rods at `rods` (gh:#1025), the bed
    /// unchanged. Nothing is rebuilt when they are already there.
    ///
    /// # Errors
    /// The model is not built, or an insertion is outside `[0, 1]`.
    pub fn set_rods(&mut self, rods: [f64; random_bed::N_RODS]) -> Result<(), String> {
        let m = self.model.as_mut().ok_or("the model is not built")?;
        if m.rods.map(f64::to_bits) != rods.map(f64::to_bits) {
            m.core = study(m.layers, &rods, &m.temps)?.core(&m.plan)?;
            m.rods = rods;
        }
        Ok(())
    }

    /// A run's initial source, the source stream after it, and the mesh,
    /// tagged with the run's `epoch`.
    pub fn source(&self, cfg: &KeffConfig, epoch: u32) -> Result<CoreEv, String> {
        let m = self.model.as_ref().ok_or("the model is not built")?;
        let (sites, seed) = DistributedPowerIteration::initial_source(
            &m.core.geometry,
            &m.materials,
            &m.nuclides,
            keff_vs_height::fissile_source_box(&m.core),
            &run_settings(cfg),
        );
        let w = mesh_words(&keff_vs_height::fissile_entropy_mesh(&m.core));
        Ok(CoreEv::Source {
            sites: Floats::from_vec(
                outram_mc_libs::physics::transport_csg::distributed::SourceSite::encode(&sites),
            ),
            seed,
            mesh: w,
            epoch,
        })
    }

    /// Transport one chunk; the result with its tracks thinned.
    pub fn chunk(&self, cfg: &KeffConfig, data: &[f64]) -> Result<Vec<f64>, String> {
        let m = self.model.as_ref().ok_or("the model is not built")?;
        let c = GenerationChunk::from_f64s(data)?;
        let mut r = transport_chunk(
            &m.core.geometry,
            &m.materials,
            &m.nuclides,
            std::slice::from_ref(&m.majorant),
            &run_settings(cfg),
            &c,
        );
        thin_tracks(&mut r);
        Ok(r.to_f64s())
    }
}

/// An entropy mesh as a [`CoreEv::Source`]'s words
/// (`[lower×3, upper×3, dims×3]`); [`mesh_from_words`] reads them back.
pub fn mesh_words(mesh: &RegularMesh) -> [f64; 9] {
    let mut w = [0.0; 9];
    w[..3].copy_from_slice(&mesh.lower_left);
    w[3..6].copy_from_slice(&mesh.upper_right);
    for k in 0..3 {
        w[6 + k] = mesh.dimension[k] as f64;
    }
    w
}

/// The entropy mesh from a [`CoreEv::Source`]'s words.
pub fn mesh_from_words(w: &[f64; 9]) -> RegularMesh {
    RegularMesh {
        lower_left: [w[0], w[1], w[2]],
        upper_right: [w[3], w[4], w[5]],
        dimension: [w[6] as usize, w[7] as usize, w[8] as usize],
    }
}

// ─── Native: the worker as an engine thread ──────────────────────────────────

/// A tape from the workspace, stripped of covariances as the browser gets it.
#[cfg(not(target_arch = "wasm32"))]
pub fn native_tape(t: &Tape) -> Result<Vec<u8>, String> {
    let raw = std::fs::read(t.path()).map_err(|e| format!("{}: {e}", t.path().display()))?;
    Ok(crate::tapes::strip_covariances(&raw))
}

/// Serve one request natively (the engine thread of a pool member).
#[cfg(not(target_arch = "wasm32"))]
pub fn serve_native(w: &mut CoreWorker, req: CoreReq, post: &mut impl FnMut(crate::engine::Event)) {
    use crate::engine::Event;
    let mut send = |e: CoreEv| post(Event::Core(e));
    // The run a failure belongs to (gh:#817): a failed chunk fails its run,
    // not the pool; the worker keeps its data.
    let epoch = match &req {
        CoreReq::Source { epoch, .. } | CoreReq::Chunk { epoch, .. } => *epoch,
        _ => NO_RUN,
    };
    let r: Result<(), String> = (|| {
        match req {
            CoreReq::Process { jobs: list, temps } => {
                let temps = ZoneTemperatures::from_kelvin(temps);
                let all = jobs_at(&temps)?;
                let cfg = ParametricStudy::default().data;
                for i in list {
                    send(CoreEv::JobStarted { job: i });
                    let t0 = std::time::Instant::now();
                    let bytes = match all
                        .get(i)
                        .ok_or("no such job")?
                        .job
                        .tape(&cfg)
                        .map_err(|e| e.to_string())?
                    {
                        Some(t) => Some(native_tape(&t)?),
                        None => None,
                    };
                    if let Some(b) = &bytes {
                        w.cache.begin_tape(b);
                    }
                    let (data, source) = run_job_cached(i, &temps, bytes.as_deref(), &mut w.cache)?;
                    let (_, notes) = w.cache.take_report();
                    for n in notes {
                        send(CoreEv::DataNote(n));
                    }
                    w.store(i, data.clone());
                    send(CoreEv::Product {
                        job: i,
                        secs: t0.elapsed().as_secs_f64(),
                        data,
                        source,
                    });
                }
            }
            CoreReq::Product { job, data } => w.store(job, data),
            CoreReq::Pour(r) => w.pour.serve(r, &mut |e| send(CoreEv::Pour(e))),
            CoreReq::PourGpu(r) => serve_gpu_pour(w, r, &mut send),
            CoreReq::PourDiagnostics { centres_m } => {
                let c: Vec<f32> = centres_m.to_vec().iter().map(|&v| v as f32).collect();
                send(CoreEv::PourDiagnostics(pour::diagnostics(&c).to_words()));
            }
            r @ CoreReq::PourBed { .. } => random_bed::set_poured(r.poured_bed()?),
            CoreReq::Assemble { layers, temps } => {
                let mut tapes_s = 0.0;
                let mut rep = w.assemble(layers, &ZoneTemperatures::from_kelvin(temps), |t| {
                    let t0 = std::time::Instant::now();
                    let b = native_tape(t);
                    tapes_s += t0.elapsed().as_secs_f64();
                    b
                })?;
                rep.tapes_s = tapes_s;
                rep.nuclides_s -= tapes_s;
                send(CoreEv::Assembled(rep));
            }
            CoreReq::Rods { rods } => {
                w.set_rods(rods)?;
                if let Some(m) = &w.model {
                    send(CoreEv::Assembled(AssemblyReport {
                        layers: m.layers,
                        tapes_s: 0.0,
                        nuclides_s: 0.0,
                        geometry_s: 0.0,
                        majorant_s: 0.0,
                        memory_mb: f64::NAN,
                    }));
                }
            }
            CoreReq::Layers { layers } => {
                w.set_layers(layers)?;
                if let Some(m) = &w.model {
                    send(CoreEv::Assembled(AssemblyReport {
                        layers: m.layers,
                        tapes_s: 0.0,
                        nuclides_s: 0.0,
                        geometry_s: 0.0,
                        majorant_s: 0.0,
                        memory_mb: f64::NAN,
                    }));
                }
            }
            CoreReq::Source { cfg, epoch } => send(w.source(&cfg, epoch)?),
            CoreReq::Chunk { cfg, data, epoch } => {
                let t0 = std::time::Instant::now();
                let out = w.chunk(&cfg, &data.to_vec())?;
                send(CoreEv::ChunkDone {
                    data: Floats::from_vec(out),
                    secs: t0.elapsed().as_secs_f64(),
                    epoch,
                });
            }
        }
        Ok(())
    })();
    if let Err(e) = r {
        post(Event::Core(CoreEv::Failed {
            message: format!("core worker: {e}"),
            epoch,
        }));
    }
}

// ─── Browser: messages, and the worker's async side ──────────────────────────

#[cfg(target_arch = "wasm32")]
pub mod web {
    use super::*;
    use outram_mc_libs::physics::transport_csg::distributed::{seed_from_words, seed_words};
    use crate::engine::Event;
    use dhoby_ghaut::web_demo::link::{fetch_bytes, js, Poster};
    use std::sync::{Arc, RwLock};
    use wasm_bindgen::JsValue;

    fn cfg_words(c: &KeffConfig) -> Vec<f64> {
        let s = seed_words(c.seed);
        vec![
            c.n_particles as f64,
            c.n_inactive as f64,
            c.n_active as f64,
            s[0],
            s[1],
        ]
    }
    fn cfg_from(v: &[f64]) -> Result<KeffConfig, String> {
        if v.len() != 5 {
            return Err("core run settings: wrong length".into());
        }
        Ok(KeffConfig {
            n_particles: v[0] as usize,
            n_inactive: v[1] as usize,
            n_active: v[2] as usize,
            seed: seed_from_words(v[3], v[4]),
            point_source: false,
            want_sites: false,
        })
    }

    thread_local! {
        /// The browser's GPU pour (its GPU objects are not `Send`, so it
        /// lives with this worker's thread), and the newest pour id asked for.
        static GPU_POUR: std::cell::RefCell<Option<(u32, outram_park_fork_liggghts::htr10_fill::Htr10GpuFill)>> = const { std::cell::RefCell::new(None) };
        static GPU_LATEST: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    }

    /// Serve a GPU pour request in the browser: WebGPU, without blocking.
    /// The pour is taken out while a chunk runs and put back only if no
    /// newer pour has started meanwhile.
    async fn gpu_pour(r: pour::engine::Request, send: impl Fn(CoreEv)) {
        use outram_park_fork_liggghts::htr10_fill::Htr10GpuFill;
        use pour::engine::{Event, Request};
        let now = dhoby_ghaut::web_demo::platform::now_s;
        match r {
            Request::Start { id, n, seed } => {
                GPU_LATEST.with(|l| l.set(id));
                GPU_POUR.with(|g| g.borrow_mut().take());
                let Some(ctx) = outram_park_fork_liggghts::gpu::probe_async().await else {
                    return send(CoreEv::Pour(Event::Error(pour::NO_WEBGPU.into())));
                };
                log::info!("htr10 core: GPU pour on {} ({:?})", ctx.info.name, ctx.info.backend);
                match Htr10GpuFill::new_async(pour::engine::settings(n, seed), ctx).await {
                    Ok(f) => {
                        let p = f.progress();
                        send(CoreEv::Pour(Event::Progress(pour::gpu_snapshot(id, &f, p, 0.0))));
                        if GPU_LATEST.with(|l| l.get()) == id {
                            GPU_POUR.with(|g| *g.borrow_mut() = Some((id, f)));
                        }
                    }
                    Err(e) => send(CoreEv::Pour(Event::Error(format!("the GPU pour could not start: {e}")))),
                }
            }
            Request::Step { id, steps } => {
                let taken = GPU_POUR.with(|g| {
                    let mut g = g.borrow_mut();
                    if g.as_ref().is_some_and(|(pid, _)| *pid == id) { g.take() } else { None }
                });
                let Some((pid, mut f)) = taken else { return };
                let t0 = now();
                let p = f.advance_async(steps.max(1)).await;
                let ms = (now() - t0) * 1000.0 / steps.max(1) as f64;
                send(CoreEv::Pour(Event::Progress(pour::gpu_snapshot(pid, &f, p, ms))));
                if GPU_LATEST.with(|l| l.get()) == pid {
                    GPU_POUR.with(|g| *g.borrow_mut() = Some((pid, f)));
                }
            }
        }
    }

    impl CoreReq {
        pub fn to_js(&self, o: &js_sys::Object) {
            match self {
                CoreReq::Process { jobs, temps } => {
                    js::set(o, "kind", "core_process");
                    js::set(
                        o,
                        "jobs",
                        js::f64s(&jobs.iter().map(|&j| j as f64).collect::<Vec<_>>()),
                    );
                    js::set(o, "temps", js::f64s(temps));
                }
                CoreReq::Product { job, data } => {
                    js::set(o, "kind", "core_product");
                    js::set(o, "job", *job as f64);
                    js::set(o, "data", data.js());
                }
                CoreReq::Assemble { layers, temps } => {
                    js::set(o, "kind", "core_assemble");
                    js::set(o, "layers", *layers as f64);
                    js::set(o, "temps", js::f64s(temps));
                }
                CoreReq::Layers { layers } => {
                    js::set(o, "kind", "core_layers");
                    js::set(o, "layers", *layers as f64);
                }
                CoreReq::Rods { rods } => {
                    js::set(o, "kind", "core_rods");
                    js::set(o, "rods", js::f64s(rods));
                }
                CoreReq::Source { cfg, epoch } => {
                    js::set(o, "kind", "core_source");
                    js::set(o, "cfg", js::f64s(&cfg_words(cfg)));
                    js::set(o, "epoch", *epoch as f64);
                }
                CoreReq::Chunk { cfg, data, epoch } => {
                    js::set(o, "kind", "core_chunk");
                    js::set(o, "cfg", js::f64s(&cfg_words(cfg)));
                    js::set(o, "data", data.js());
                    js::set(o, "epoch", *epoch as f64);
                }
                CoreReq::Pour(r) => {
                    js::set(o, "kind", "core_pour");
                    js::set(o, "data", js::f64s(&r.to_wire()));
                }
                CoreReq::PourGpu(r) => {
                    js::set(o, "kind", "core_pour_gpu");
                    js::set(o, "data", js::f64s(&r.to_wire()));
                }
                CoreReq::PourDiagnostics { centres_m } => {
                    js::set(o, "kind", "core_pour_diag");
                    js::set(o, "data", centres_m.js());
                }
                CoreReq::PourBed { centres_cm, fuel, version, cut, fuel_fraction } => {
                    js::set(o, "kind", "core_pour_bed");
                    js::set(o, "data", centres_cm.js());
                    js::set(o, "fuel", js::f64s(fuel));
                    js::set(o, "head", js::f64s(&[*version as f64, *cut as f64, *fuel_fraction]));
                }
            }
        }
        /// `None` if `kind` is not a core request.
        pub fn from_js(kind: &str, v: &JsValue) -> Option<Result<Self, String>> {
            let n = |k: &str| js::get_f64(v, k).unwrap_or(0.0) as usize;
            let floats = |k: &str| Floats::get(v, k).ok_or_else(|| format!("{kind}: no {k}"));
            let temps = || {
                <[f64; 8]>::try_from(js::get_f64s(v, "temps").as_slice())
                    .map_err(|_| format!("{kind}: not eight zone temperatures"))
            };
            Some(match kind {
                "core_process" => temps().map(|temps| CoreReq::Process {
                    jobs: js::get_f64s(v, "jobs")
                        .into_iter()
                        .map(|j| j as usize)
                        .collect(),
                    temps,
                }),
                "core_product" => floats("data").map(|data| CoreReq::Product {
                    job: n("job"),
                    data,
                }),
                "core_assemble" => temps().map(|temps| CoreReq::Assemble {
                    layers: n("layers"),
                    temps,
                }),
                "core_layers" => Ok(CoreReq::Layers {
                    layers: n("layers"),
                }),
                "core_rods" => <[f64; random_bed::N_RODS]>::try_from(js::get_f64s(v, "rods").as_slice())
                    .map(|rods| CoreReq::Rods { rods })
                    .map_err(|_| "core_rods: not ten insertions".to_string()),
                "core_source" => cfg_from(&js::get_f64s(v, "cfg")).map(|cfg| CoreReq::Source {
                    cfg,
                    epoch: n("epoch") as u32,
                }),
                "core_chunk" => cfg_from(&js::get_f64s(v, "cfg")).and_then(|cfg| {
                    Ok(CoreReq::Chunk {
                        cfg,
                        data: floats("data")?,
                        epoch: n("epoch") as u32,
                    })
                }),
                "core_pour" => pour::engine::Request::from_wire(&js::get_f64s(v, "data")).map(CoreReq::Pour),
                "core_pour_gpu" => pour::engine::Request::from_wire(&js::get_f64s(v, "data")).map(CoreReq::PourGpu),
                "core_pour_diag" => floats("data").map(|centres_m| CoreReq::PourDiagnostics { centres_m }),
                "core_pour_bed" => floats("data").and_then(|centres_cm| {
                    let h = js::get_f64s(v, "head");
                    if h.len() != 3 {
                        return Err("core_pour_bed: bad head".into());
                    }
                    Ok(CoreReq::PourBed {
                        centres_cm,
                        fuel: js::get_f64s(v, "fuel"),
                        version: h[0] as u32,
                        cut: h[1] as usize,
                        fuel_fraction: h[2],
                    })
                }),
                _ => return None,
            })
        }
    }

    impl CoreEv {
        pub fn to_js(&self, o: &js_sys::Object) {
            match self {
                CoreEv::JobStarted { job } => {
                    js::set(o, "kind", "core_started");
                    js::set(o, "job", *job as f64);
                }
                CoreEv::Product { job, secs, data, source } => {
                    js::set(o, "kind", "core_done");
                    js::set(o, "job", *job as f64);
                    js::set(o, "secs", *secs);
                    js::set(o, "data", data.js());
                    crate::engine::web::set_source(o, source);
                }
                CoreEv::DataNote(m) => {
                    js::set(o, "kind", "core_data_note");
                    js::set(o, "message", m.as_str());
                }
                CoreEv::Assembled(r) => {
                    js::set(o, "kind", "core_assembled");
                    js::set(o, "data", js::f64s(&r.words()));
                }
                CoreEv::Source { sites, seed, mesh, epoch } => {
                    js::set(o, "kind", "core_source");
                    js::set(o, "data", sites.js());
                    js::set(o, "seed", js::f64s(&seed_words(*seed)));
                    js::set(o, "mesh", js::f64s(mesh));
                    js::set(o, "epoch", *epoch as f64);
                }
                CoreEv::ChunkDone { data, secs, epoch } => {
                    js::set(o, "kind", "core_chunk");
                    js::set(o, "data", data.js());
                    js::set(o, "secs", *secs);
                    js::set(o, "epoch", *epoch as f64);
                }
                CoreEv::Failed { message, epoch } => {
                    js::set(o, "kind", "core_failed");
                    js::set(o, "message", message.as_str());
                    js::set(o, "epoch", *epoch as f64);
                }
                CoreEv::PourDiagnostics(w) => {
                    js::set(o, "kind", "core_pour_diag");
                    js::set(o, "data", js::f64s(w));
                }
                CoreEv::Pour(e) => {
                    let (d, t) = e.to_wire();
                    js::set(o, "kind", "core_pour");
                    js::set(o, "data", js::f64s(&d));
                    js::set(o, "text", t.as_str());
                }
            }
        }
        pub fn from_js(kind: &str, v: &JsValue) -> Option<Result<Self, String>> {
            let floats = |k: &str| Floats::get(v, k).ok_or_else(|| format!("{kind}: no {k}"));
            let epoch = || js::get_f64(v, "epoch").unwrap_or(0.0) as u32;
            Some(match kind {
                "core_started" => Ok(CoreEv::JobStarted {
                    job: js::get_f64(v, "job").unwrap_or(0.0) as usize,
                }),
                "core_done" => floats("data").map(|data| CoreEv::Product {
                    job: js::get_f64(v, "job").unwrap_or(0.0) as usize,
                    secs: js::get_f64(v, "secs").unwrap_or(0.0),
                    data,
                    source: crate::engine::web::get_source(v),
                }),
                "core_data_note" => Ok(CoreEv::DataNote(js::get_str(v, "message"))),
                "core_assembled" => {
                    AssemblyReport::from_words(&js::get_f64s(v, "data")).map(CoreEv::Assembled)
                }
                "core_source" => floats("data").and_then(|sites| {
                    let s = js::get_f64s(v, "seed");
                    let m = js::get_f64s(v, "mesh");
                    if s.len() != 2 || m.len() != 9 {
                        return Err("core source: bad seed or mesh".into());
                    }
                    let mut mesh = [0.0; 9];
                    mesh.copy_from_slice(&m);
                    Ok(CoreEv::Source {
                        sites,
                        seed: seed_from_words(s[0], s[1]),
                        mesh,
                        epoch: epoch(),
                    })
                }),
                "core_chunk" => floats("data").map(|data| CoreEv::ChunkDone {
                    data,
                    secs: js::get_f64(v, "secs").unwrap_or(0.0),
                    epoch: epoch(),
                }),
                "core_failed" => Ok(CoreEv::Failed {
                    message: js::get_str(v, "message"),
                    epoch: epoch(),
                }),
                "core_pour" => pour::engine::Event::from_wire(&js::get_f64s(v, "data"), &js::get_str(v, "text")).map(CoreEv::Pour),
                "core_pour_diag" => Ok(CoreEv::PourDiagnostics(js::get_f64s(v, "data"))),
                _ => return None,
            })
        }
    }

    async fn tape_bytes(t: &Tape) -> Result<Vec<u8>, String> {
        crate::tapes::decompress(&fetch_bytes(&wire_url(t)).await?)
    }

    /// Serve one request in the worker. Downloads are awaited (the worker
    /// keeps receiving messages meanwhile: relayed products are stored as
    /// they come); the processing itself blocks this worker only.
    pub fn handle(core: &Arc<RwLock<CoreWorker>>, req: CoreReq, post: Poster<Event>) {
        let send = move |e: CoreEv| post.post(Event::Core(e));
        // A failed request is not a dead worker (gh:#817): it says so as a
        // core event, tagged with its run, and the page decides what fails.
        let epoch = match &req {
            CoreReq::Source { epoch, .. } | CoreReq::Chunk { epoch, .. } => *epoch,
            _ => NO_RUN,
        };
        let fail = move |e: String| {
            post.post(Event::Core(CoreEv::Failed {
                message: format!("core worker: {e}"),
                epoch,
            }))
        };
        match req {
            CoreReq::Product { job, data } => {
                if let Ok(mut w) = core.write() {
                    w.store(job, data);
                }
            }
            CoreReq::Pour(r) => {
                if let Ok(mut w) = core.write() {
                    w.pour.serve(r, &mut |e| send(CoreEv::Pour(e)));
                }
            }
            CoreReq::PourGpu(r) => wasm_bindgen_futures::spawn_local(gpu_pour(r, move |e| send(e))),
            CoreReq::PourDiagnostics { centres_m } => {
                let c: Vec<f32> = centres_m.to_vec().iter().map(|&v| v as f32).collect();
                send(CoreEv::PourDiagnostics(pour::diagnostics(&c).to_words()));
            }
            r @ CoreReq::PourBed { .. } => match r.poured_bed() {
                Ok(b) => random_bed::set_poured(b),
                Err(e) => fail(e),
            },
            CoreReq::Process { jobs: list, temps } => {
                let core = core.clone();
                let temps = ZoneTemperatures::from_kelvin(temps);
                wasm_bindgen_futures::spawn_local(async move {
                    let cfg = ParametricStudy::default().data;
                    let all = match jobs_at(&temps) {
                        Ok(a) => a,
                        Err(e) => return fail(e),
                    };
                    let mut store = DataStore::browser();
                    let mut health = crate::processed_cache::web::Health::default();
                    for i in list {
                        send(CoreEv::JobStarted { job: i });
                        let t0 = js_sys::Date::now();
                        let bytes = match all.get(i).map(|j| j.job.tape(&cfg)) {
                            Some(Ok(Some(t))) => match tape_bytes(&t).await {
                                Ok(b) => Some(b),
                                Err(e) => return fail(e),
                            },
                            Some(Ok(None)) => None,
                            Some(Err(e)) => return fail(e.to_string()),
                            None => return fail(format!("no job {i}")),
                        };
                        // The shared cache (gh:#818): this job's products
                        // read from IndexedDB first (a LEAPR law has no
                        // tape: its key starts `leapr-<label>`).
                        use crate::processed_cache::web as cache;
                        match &bytes {
                            Some(b) => cache::before_step(&mut store, b, &mut health).await,
                            None => {
                                let label = all.get(i).map(|j| j.job.label()).unwrap_or_default();
                                let prefix = crate::processed_cache::tape_prefix(&format!("leapr-{label}"));
                                cache::before_prefix(&mut store, &prefix, &mut health).await;
                            }
                        }
                        match run_job_cached(i, &temps, bytes.as_deref(), &mut store) {
                            Ok((data, source)) => {
                                if let Ok(mut w) = core.write() {
                                    w.store(i, data.clone());
                                }
                                send(CoreEv::Product {
                                    job: i,
                                    secs: (js_sys::Date::now() - t0) / 1000.0,
                                    data,
                                    source,
                                });
                                // Saved after the product is on its way, so
                                // the relay never waits for the write.
                                cache::after_step(&mut store, &mut health).await;
                                let (_, notes) = store.take_report();
                                for n in notes {
                                    send(CoreEv::DataNote(n));
                                }
                            }
                            Err(e) => return fail(e),
                        }
                    }
                });
            }
            CoreReq::Assemble { layers, temps } => {
                let core = core.clone();
                let temps = ZoneTemperatures::from_kelvin(temps);
                wasm_bindgen_futures::spawn_local(async move {
                    // Every tape's bytes first (awaited, so relayed products
                    // still land), then the blocking assembly.
                    let t0 = js_sys::Date::now();
                    let tapes: Vec<Tape> = match plan(&temps) {
                        Ok(p) => {
                            let mut v: Vec<Tape> = Vec::new();
                            for s in &p.layout.slots {
                                if !v.contains(&s.tape) {
                                    v.push(s.tape);
                                }
                            }
                            v
                        }
                        Err(e) => return fail(e),
                    };
                    let mut got: Vec<(Tape, Vec<u8>)> = Vec::new();
                    for t in tapes {
                        match fetch_bytes(&wire_url(&t)).await {
                            Ok(z) => got.push((t, z)),
                            Err(e) => return fail(e),
                        }
                    }
                    let tapes_s = (js_sys::Date::now() - t0) / 1000.0;
                    let r = match core.write() {
                        Ok(mut w) => w.assemble(layers, &temps, |t| {
                            let z = got
                                .iter()
                                .find(|(k, _)| k == t)
                                .map(|(_, z)| z)
                                .ok_or_else(|| format!("{}: not downloaded", t.file))?;
                            crate::tapes::decompress(z)
                        }),
                        Err(_) => Err("worker state poisoned".into()),
                    };
                    match r {
                        Ok(mut rep) => {
                            rep.tapes_s = tapes_s;
                            send(CoreEv::Assembled(rep));
                        }
                        Err(e) => fail(e),
                    }
                });
            }
            CoreReq::Layers { layers } => match core.write() {
                Ok(mut w) => match w.set_layers(layers) {
                    Ok(()) => send(CoreEv::Assembled(AssemblyReport {
                        layers,
                        tapes_s: 0.0,
                        nuclides_s: 0.0,
                        geometry_s: 0.0,
                        majorant_s: 0.0,
                        memory_mb: dhoby_ghaut::web_demo::pool::wasm_memory_mb()
                            .unwrap_or(f64::NAN),
                    })),
                    Err(e) => fail(e),
                },
                Err(_) => fail("worker state poisoned".into()),
            },
            CoreReq::Rods { rods } => match core.write() {
                Ok(mut w) => match w.set_rods(rods) {
                    Ok(()) => {
                        let layers = w.model.as_ref().map_or(0, |m| m.layers);
                        send(CoreEv::Assembled(AssemblyReport {
                            layers,
                            tapes_s: 0.0,
                            nuclides_s: 0.0,
                            geometry_s: 0.0,
                            majorant_s: 0.0,
                            memory_mb: dhoby_ghaut::web_demo::pool::wasm_memory_mb()
                                .unwrap_or(f64::NAN),
                        }))
                    }
                    Err(e) => fail(e),
                },
                Err(_) => fail("worker state poisoned".into()),
            },
            CoreReq::Source { cfg, epoch } => match core
                .read()
                .map_err(|_| "poisoned".to_string())
                .and_then(|w| w.source(&cfg, epoch))
            {
                Ok(ev) => send(ev),
                Err(e) => fail(e),
            },
            CoreReq::Chunk { cfg, data, epoch } => {
                let t0 = js_sys::Date::now();
                match core
                    .read()
                    .map_err(|_| "poisoned".to_string())
                    .and_then(|w| w.chunk(&cfg, &data.to_vec()))
                {
                    Ok(out) => send(CoreEv::ChunkDone {
                        data: Floats::from_vec(out),
                        secs: (js_sys::Date::now() - t0) / 1000.0,
                        epoch,
                    }),
                    Err(e) => fail(e),
                }
            }
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    /// **A worker rebuilds its core with the rods asked for** (gh:#1025):
    /// a model on the lattice at N = 12 (no nuclear data needed to build
    /// geometry), `set_rods` with rod 3 alone fully in, then `set_layers`.
    /// Pass: the core's per-rod lower ends follow the rods, a layer change
    /// keeps them, and withdrawn rods give `build_core`'s core exactly.
    #[test]
    fn a_worker_rebuilds_its_core_with_the_rods() {
        let mut w = CoreWorker::default();
        assert!(w.set_rods(random_bed::WITHDRAWN).is_err(), "no model yet");
        let temps = ZoneTemperatures::record();
        let p = plan(&temps).expect("plan");
        w.model = Some(CoreModel {
            nuclides: Vec::new(),
            materials: p.materials.clone(),
            core: study(12, &random_bed::WITHDRAWN, &temps).expect("study").core(&p).expect("core"),
            plan: p,
            temps,
            majorant: Majorant::uniform(1.0),
            layers: 12,
            rods: random_bed::WITHDRAWN,
        });
        let mut rods = random_bed::WITHDRAWN;
        rods[2] = 1.0;
        w.set_rods(rods).expect("rods");
        let ends = |w: &CoreWorker| w.model.as_ref().unwrap().core.rod_lower_ends_zt_cm.expect("rods built");
        assert_eq!(ends(&w)[2], 394.2);
        assert_eq!(ends(&w)[0], 119.2);
        w.set_layers(13).expect("layers");
        assert_eq!(ends(&w)[2], 394.2, "a layer change keeps the rods");
        assert!(w.set_rods([2.0; random_bed::N_RODS]).is_err());
        w.set_rods(random_bed::WITHDRAWN).expect("withdraw");
        let a = &w.model.as_ref().unwrap().core;
        let temps = ZoneTemperatures::record();
        let b = study(13, &random_bed::WITHDRAWN, &temps).unwrap().core(&plan(&temps).unwrap()).unwrap();
        assert_eq!(format!("{:?}", a.geometry), format!("{:?}", b.geometry));
        // The same cells as the record's core: only the zone slots differ.
        assert_eq!(a.geometry.cells.len(), random_bed::build_core(13).unwrap().geometry.cells.len());
    }

    /// **A cached job is the processed job, bit for bit** (gh:#818): the
    /// cheapest tape job and the Si-in-SiC law (THERMR, tens of seconds) are
    /// run through a store, the records offered to a second store as the
    /// browser does, and run again. Pass: the words are [`run_job`]'s (the
    /// path every recorded run took) word for word, both times; the second
    /// run reports the cache and processes nothing.
    ///
    /// Results, 2026-10-09 (release): O-16 (the cheapest tape job), 76 849
    /// words, and Si-in-SiC, 229 552 words, identical processed and cached.
    #[test]
    fn a_cached_job_is_the_processed_job() {
        let temps = ZoneTemperatures::record();
        let all = jobs_at(&temps).expect("jobs");
        let cfg = ParametricStudy::default().data;
        let cheap = (0..all.len())
            .filter(|&i| matches!(all[i].job, ProcessingJob::Tape { .. }))
            .min_by(|&a, &b| job_cost(&all[a]).total_cmp(&job_cost(&all[b])))
            .expect("a tape job");
        // Since gh:#1142 every law is LEAPR's (no tape); Si-in-SiC is the
        // cheapest LEAPR-made law here and is offered with no bytes.
        let law = all.iter().position(|j| j.job.label() == "Si-in-SiC S(a,b)").expect("Si-in-SiC");
        for i in [cheap, law] {
            let bytes = match all[i].job.tape(&cfg).expect("tape") {
                Some(t) => match native_tape(&t) {
                    Ok(b) => Some(b),
                    Err(_) => {
                        eprintln!("SKIP {}: tape not present", all[i].label());
                        continue;
                    }
                },
                None => None,
            };
            let want: Vec<u64> = run_job(i, &temps, bytes.as_deref()).expect("direct").to_vec().iter().map(|x| x.to_bits()).collect();
            let mut first = DataStore::browser();
            if let Some(b) = &bytes {
                first.begin_tape(b);
            }
            let (made, src) = run_job_cached(i, &temps, bytes.as_deref(), &mut first).expect("made");
            assert_eq!(src, Source::Processed);
            assert_eq!(made.to_vec().iter().map(|x| x.to_bits()).collect::<Vec<_>>(), want, "{}: processed", all[i].label());
            let mut second = DataStore::browser();
            if let Some(b) = &bytes {
                second.begin_tape(b);
            }
            second.offer(first.take_fresh());
            let (cached, src) = run_job_cached(i, &temps, bytes.as_deref(), &mut second).expect("cached");
            assert!(matches!(src, Source::Cached { .. }), "{}: {src:?}", all[i].label());
            assert!(second.take_fresh().is_empty());
            assert_eq!(cached.to_vec().iter().map(|x| x.to_bits()).collect::<Vec<_>>(), want, "{}: cached", all[i].label());
            eprintln!("{}: {} words identical, processed and cached", all[i].label(), want.len());
        }
        // A LEAPR law's key has no tape: `leapr-<label>`.
        let mut s = DataStore::off();
        assert!(run_job_cached(cheap, &temps, None, &mut s).is_err(), "a tape job needs its tape");
    }

    /// The jobs are the layout's (no drift), every one has a cost, and the
    /// schedule deals every job exactly once, longest first, balancing load.
    #[test]
    fn the_jobs_are_dealt_once_each_longest_first() {
        let jobs = jobs().expect("jobs");
        assert_eq!(jobs.len(), 36);
        // At eight zone temperatures there are more jobs, each with a cost.
        let mut hot = ZoneTemperatures::record();
        for (i, z) in nee_soon::htr10_rmc::zones::Htr10Zone::ALL.into_iter().enumerate() {
            hot.set(z, uom::si::f64::ThermodynamicTemperature::new::<uom::si::thermodynamic_temperature::kelvin>(400.0 + 50.0 * i as f64));
        }
        assert!(jobs_at(&hot).expect("hot jobs").len() > 36);
        let costs: Vec<f64> = jobs.iter().map(job_cost).collect();
        assert!(costs.iter().all(|&c| c > 0.0));
        for w in [1, 2, 4, 7] {
            let s = schedule(&costs, w);
            assert_eq!(s.len(), w);
            let mut all: Vec<usize> = s.iter().flatten().copied().collect();
            all.sort_unstable();
            assert_eq!(all, (0..jobs.len()).collect::<Vec<_>>(), "{w} workers");
            // U-235 (the longest) goes first, to worker 0.
            let u235 = jobs.iter().position(|j| j.job.label() == "U235").expect("U235");
            assert_eq!(s[0][0], u235);
            if w >= 2 {
                // U-238 does not queue behind U-235.
                let u238 = jobs.iter().position(|j| j.job.label() == "U238").expect("U238");
                assert!(!s[0].contains(&u238));
            }
        }
        // Every job's tape has a download name.
        let cfg = ParametricStudy::default().data;
        for j in &jobs {
            if let Some(t) = j.job.tape(&cfg).expect("tape") {
                assert!(wire_url(&t).starts_with("data/") && wire_url(&t).ends_with(".zz"));
            }
        }
        assert_eq!(run_settings(&DEFAULT_RUN).seed, RECORD_SEED);
        let m = mesh_from_words(&[-1.0, -2.0, -3.0, 1.0, 2.0, 3.0, 4.0, 4.0, 4.0]);
        assert_eq!(m.dimension, [4, 4, 4]);
        let r = AssemblyReport {
            layers: 12,
            tapes_s: 1.0,
            nuclides_s: 2.0,
            geometry_s: 0.2,
            majorant_s: 3.0,
            memory_mb: 400.0,
        };
        assert_eq!(AssemblyReport::from_words(&r.words()).expect("report"), r);
    }
}
