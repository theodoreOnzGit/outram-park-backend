//! **The whole HTR-10 core, live, on a pool of Web Workers** (gh:#786).
//!
//! The model is the recorded runs' exactly:
//! `nee_soon::htr10_rmc::core_model::assemble_explicit_triso(14, N, 0)` (the
//! bed delta-tracked against `keff_vs_height::bed_majorant`, everything else
//! surface-tracked), the material set
//! `htr10_material_set(layout, Htr10MaterialConfig::benchmark_default(300.15))`,
//! and the nuclide set of `Htr10DataConfig::default()` (ENDF/B-VIII.0 at
//! 300.15 K and tolerance 1e-3, URR and DBRC on, natural carbon, helium, real
//! Ni/Fe rod steel, every bound thermal law, the UO₂ laws by LEAPR). The run
//! is `run_keff_csg_par`'s power iteration, which the recorded runs used,
//! taken apart by `outram_mc_libs::physics::transport_csg::distributed`.
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
pub mod random_bed;
pub mod screen;
#[cfg(not(target_arch = "wasm32"))]
pub mod bake;

use crate::keff::KeffConfig;
use dhoby_ghaut::web_demo::link::Floats;
use nee_soon::htr10_rmc::core_model::AssembledCore;
use nee_soon::htr10_rmc::data::{Htr10DataConfig, Htr10NuclideLayout, Tape};
use nee_soon::htr10_rmc::data_jobs::{assemble_slots, process_job, JobProduct, ProcessingJob};
use nee_soon::htr10_rmc::keff_vs_height;
use nee_soon::htr10_rmc::materials::{htr10_material_set, Htr10MaterialConfig};
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

/// Temperature of every material and nuclide, K (the recorded runs').
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

/// The nuclide plan of the recorded runs.
pub fn layout() -> Result<Htr10NuclideLayout, String> {
    Htr10NuclideLayout::plan(&Htr10DataConfig::default()).map_err(|e| e.to_string())
}

/// The jobs, in the order every worker and the page index them.
pub fn jobs() -> Result<Vec<ProcessingJob>, String> {
    Ok(layout()?.processing_jobs())
}

/// Rough cost of a job, s, for dealing them out and for the progress bar:
/// the native bake's per-job times (2026-10-07, i9-13900K, 5 jobs at once on
/// 5 threads of a loaded machine; `verification_and_validation/
/// htr10_full_core_web/logs/bake_native.log`). Only the order matters.
pub fn job_cost(job: &ProcessingJob) -> f64 {
    match job.label().as_str() {
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
    /// Process these jobs (indices into [`jobs`]), posting each product.
    Process { jobs: Vec<usize> },
    /// Another worker's product.
    Product { job: usize, data: Floats },
    /// Every product is here: build the 38 nuclides, the materials, the core
    /// at `layers` and the bed majorant.
    Assemble { layers: usize },
    /// Rebuild the core at another layer count (nothing else changes).
    Layers { layers: usize },
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
    /// A finished job's product ([`JobProduct::to_f64s`]).
    Product {
        job: usize,
        secs: f64,
        data: Floats,
    },
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
}

/// The model a worker transports through.
pub struct CoreModel {
    pub nuclides: Vec<Nuclide>,
    pub materials: Vec<Material>,
    pub majorant: Majorant,
    pub core: AssembledCore,
    pub layers: usize,
}

/// One worker's state.
#[derive(Default)]
pub struct CoreWorker {
    /// Every job's product as it arrives (own and relayed).
    pub products: Vec<Option<Floats>>,
    pub model: Option<CoreModel>,
}

/// Run job `i` from its tape's bytes (covariance-stripped, inflated), or
/// none for a LEAPR law.
pub fn run_job(i: usize, tape: Option<&[u8]>) -> Result<Floats, String> {
    let jobs = jobs()?;
    let job = jobs.get(i).ok_or("no such job")?;
    let t = match tape {
        Some(b) => Some(
            EndfTape::read(std::io::Cursor::new(b)).map_err(|e| format!("{}: {e}", job.label()))?,
        ),
        None => None,
    };
    let p = process_job(job, &Htr10DataConfig::default(), t.as_ref()).map_err(|e| e.to_string())?;
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
    /// stripped bytes of each tape). Returns the report without the tape
    /// time, which the caller measures.
    pub fn assemble(
        &mut self,
        layers: usize,
        mut tape: impl FnMut(&Tape) -> Result<Vec<u8>, String>,
    ) -> Result<AssemblyReport, String> {
        let now = dhoby_ghaut::web_demo::platform::now_s;
        let cfg = Htr10DataConfig::default();
        let layout = layout()?;
        let jobs = layout.processing_jobs();
        if self.products.len() < jobs.len()
            || self.products.iter().take(jobs.len()).any(Option::is_none)
        {
            return Err("not every product has arrived".into());
        }
        self.model = None;
        let t0 = now();
        let products = &self.products;
        let nuclides = assemble_slots(
            &cfg,
            &layout,
            &jobs,
            |i| {
                let v = products[i].as_ref().map(Floats::to_vec).unwrap_or_default();
                JobProduct::from_f64s(&v)
            },
            |t| {
                let b =
                    tape(t).map_err(nee_soon::htr10_rmc::data::Htr10DataError::InvalidConfig)?;
                EndfTape::read(std::io::Cursor::new(b)).map_err(|e| {
                    nee_soon::htr10_rmc::data::Htr10DataError::InvalidConfig(format!(
                        "{}: {e}",
                        t.file
                    ))
                })
            },
        )
        .map_err(|e| e.to_string())?;
        // The relayed copies are no longer needed: free them.
        self.products.clear();
        let t1 = now();
        let materials = htr10_material_set(&layout, Htr10MaterialConfig::benchmark_default(TEMP_K));
        // `layers` is a bed code: N for the lattice, 0 for the DEM bed.
        let core = random_bed::build_core(layers)?;
        let t2 = now();
        let majorant = keff_vs_height::bed_majorant(&materials, &nuclides);
        let t3 = now();
        self.model = Some(CoreModel {
            nuclides,
            materials,
            majorant,
            core,
            layers,
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
        if m.layers != layers {
            m.core = random_bed::build_core(layers)?;
            m.layers = layers;
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
        let mesh = keff_vs_height::fissile_entropy_mesh(&m.core);
        let mut w = [0.0; 9];
        w[..3].copy_from_slice(&mesh.lower_left);
        w[3..6].copy_from_slice(&mesh.upper_right);
        for k in 0..3 {
            w[6 + k] = mesh.dimension[k] as f64;
        }
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
            CoreReq::Process { jobs: list } => {
                let all = jobs()?;
                for i in list {
                    send(CoreEv::JobStarted { job: i });
                    let t0 = std::time::Instant::now();
                    let bytes = match all
                        .get(i)
                        .ok_or("no such job")?
                        .tape(&Htr10DataConfig::default())
                        .map_err(|e| e.to_string())?
                    {
                        Some(t) => Some(native_tape(&t)?),
                        None => None,
                    };
                    let data = run_job(i, bytes.as_deref())?;
                    w.store(i, data.clone());
                    send(CoreEv::Product {
                        job: i,
                        secs: t0.elapsed().as_secs_f64(),
                        data,
                    });
                }
            }
            CoreReq::Product { job, data } => w.store(job, data),
            CoreReq::Assemble { layers } => {
                let mut tapes_s = 0.0;
                let mut rep = w.assemble(layers, |t| {
                    let t0 = std::time::Instant::now();
                    let b = native_tape(t);
                    tapes_s += t0.elapsed().as_secs_f64();
                    b
                })?;
                rep.tapes_s = tapes_s;
                rep.nuclides_s -= tapes_s;
                send(CoreEv::Assembled(rep));
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

    impl CoreReq {
        pub fn to_js(&self, o: &js_sys::Object) {
            match self {
                CoreReq::Process { jobs } => {
                    js::set(o, "kind", "core_process");
                    js::set(
                        o,
                        "jobs",
                        js::f64s(&jobs.iter().map(|&j| j as f64).collect::<Vec<_>>()),
                    );
                }
                CoreReq::Product { job, data } => {
                    js::set(o, "kind", "core_product");
                    js::set(o, "job", *job as f64);
                    js::set(o, "data", data.js());
                }
                CoreReq::Assemble { layers } => {
                    js::set(o, "kind", "core_assemble");
                    js::set(o, "layers", *layers as f64);
                }
                CoreReq::Layers { layers } => {
                    js::set(o, "kind", "core_layers");
                    js::set(o, "layers", *layers as f64);
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
            }
        }
        /// `None` if `kind` is not a core request.
        pub fn from_js(kind: &str, v: &JsValue) -> Option<Result<Self, String>> {
            let n = |k: &str| js::get_f64(v, k).unwrap_or(0.0) as usize;
            let floats = |k: &str| Floats::get(v, k).ok_or_else(|| format!("{kind}: no {k}"));
            Some(match kind {
                "core_process" => Ok(CoreReq::Process {
                    jobs: js::get_f64s(v, "jobs")
                        .into_iter()
                        .map(|j| j as usize)
                        .collect(),
                }),
                "core_product" => floats("data").map(|data| CoreReq::Product {
                    job: n("job"),
                    data,
                }),
                "core_assemble" => Ok(CoreReq::Assemble {
                    layers: n("layers"),
                }),
                "core_layers" => Ok(CoreReq::Layers {
                    layers: n("layers"),
                }),
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
                CoreEv::Product { job, secs, data } => {
                    js::set(o, "kind", "core_done");
                    js::set(o, "job", *job as f64);
                    js::set(o, "secs", *secs);
                    js::set(o, "data", data.js());
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
                }),
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
            CoreReq::Process { jobs: list } => {
                let core = core.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let all = match jobs() {
                        Ok(a) => a,
                        Err(e) => return fail(e),
                    };
                    for i in list {
                        send(CoreEv::JobStarted { job: i });
                        let t0 = js_sys::Date::now();
                        let bytes = match all.get(i).map(|j| j.tape(&Htr10DataConfig::default())) {
                            Some(Ok(Some(t))) => match tape_bytes(&t).await {
                                Ok(b) => Some(b),
                                Err(e) => return fail(e),
                            },
                            Some(Ok(None)) => None,
                            Some(Err(e)) => return fail(e.to_string()),
                            None => return fail(format!("no job {i}")),
                        };
                        match run_job(i, bytes.as_deref()) {
                            Ok(data) => {
                                if let Ok(mut w) = core.write() {
                                    w.store(i, data.clone());
                                }
                                send(CoreEv::Product {
                                    job: i,
                                    secs: (js_sys::Date::now() - t0) / 1000.0,
                                    data,
                                });
                            }
                            Err(e) => return fail(e),
                        }
                    }
                });
            }
            CoreReq::Assemble { layers } => {
                let core = core.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    // Every tape's bytes first (awaited, so relayed products
                    // still land), then the blocking assembly.
                    let t0 = js_sys::Date::now();
                    let tapes: Vec<Tape> = match layout() {
                        Ok(l) => {
                            let mut v: Vec<Tape> = Vec::new();
                            for s in &l.slots {
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
                        Ok(mut w) => w.assemble(layers, |t| {
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

    /// The jobs are the layout's (no drift), every one has a cost, and the
    /// schedule deals every job exactly once, longest first, balancing load.
    #[test]
    fn the_jobs_are_dealt_once_each_longest_first() {
        let jobs = jobs().expect("jobs");
        assert_eq!(jobs.len(), 36);
        let costs: Vec<f64> = jobs.iter().map(job_cost).collect();
        assert!(costs.iter().all(|&c| c > 0.0));
        for w in [1, 2, 4, 7] {
            let s = schedule(&costs, w);
            assert_eq!(s.len(), w);
            let mut all: Vec<usize> = s.iter().flatten().copied().collect();
            all.sort_unstable();
            assert_eq!(all, (0..jobs.len()).collect::<Vec<_>>(), "{w} workers");
            // U-235 (the longest) goes first, to worker 0.
            let u235 = jobs.iter().position(|j| j.label() == "U235").expect("U235");
            assert_eq!(s[0][0], u235);
            if w >= 2 {
                // U-238 does not queue behind U-235.
                let u238 = jobs.iter().position(|j| j.label() == "U238").expect("U238");
                assert!(!s[0].contains(&u238));
            }
        }
        // Every job's tape has a download name.
        let cfg = Htr10DataConfig::default();
        for j in &jobs {
            if let Some(t) = j.tape(&cfg).expect("tape") {
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
