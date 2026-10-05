//! The workbench's engine thread: everything that takes longer than a frame.
//!
//! The UI thread only draws and handles input (the no-lag HARD RULE); every
//! request below runs here, on the thread `web_demo::link::start_native`
//! starts, and posts its result back as an [`Ev`]. Each request is one
//! self-contained job. A k_eff run is the longest: the CSG driver
//! (`run_keff_csg_hybrid`) runs every generation in one call and returns the
//! per-generation k afterwards, so the UI shows a running clock and the
//! console when the run ends (per-generation streaming would need a stepped
//! CSG power iteration in `outram-mc-libs`; gh:#579).
//!
//! Two engines run: a **geometry** engine (assembly, slices, PDF pages) and a
//! **physics** engine (data and k_eff), sharing the assembled core as an
//! `Arc` (read-only data, per the workspace rule). So the main view keeps
//! re-rendering while a k_eff run occupies the physics thread.
//!
//! No physics is implemented here. The model is `nee_soon::htr10_rmc`
//! (geometry, materials, nuclide plan), transport is `outram-mc-libs`, and the
//! slices are `outram_mc_libs::geometry::plot`, the OpenMC-parity plotter.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use dhoby_ghaut::web_demo::link::NativeEngine;
use nee_soon::htr10_rmc::core_model::{assemble_explicit_triso, AssembledCore};
use nee_soon::htr10_rmc::data::{
    load_htr10_nuclides_with_progress, Htr10DataConfig, Htr10NuclideLayout, LoadProgress,
};
use nee_soon::htr10_rmc::keff_vs_height::{bed_majorant, fissile_entropy_mesh, fissile_source_box};
use nee_soon::htr10_rmc::materials::{htr10_material_set, Htr10MaterialConfig};
use outram_mc_libs::geometry::plot::{render_material_slice, ImageData, PlotBasis, SlicePlot};
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::material::material::Material;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::{ComputeType, KeffSettings, ThreadCount};
use outram_mc_libs::physics::transport_csg::run_keff_csg_hybrid;
use outram_mc_libs::run_diagnostics::RunDiagnostics;
use outram_mc_libs::tally::filter::{EnergyFilter, FilterKind};
use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::kelvin;

/// One tape found by the Step 0 scan.
#[derive(Clone, Debug, PartialEq)]
pub struct TapeInfo {
    /// File name.
    pub file: String,
    /// MAT number (header line 2, columns 67-70).
    pub mat: Option<i32>,
    /// `ZSYMAM` (MF1/MT451 line 5, columns 1-11), e.g. ` 92-U -235 `.
    pub symbol: String,
    /// Sub-library (`NSUB`): 10 incident neutron, 12 thermal scattering, ...
    pub nsub: Option<i64>,
}

impl TapeInfo {
    /// What the sub-library number means, for the table.
    pub fn kind(&self) -> &'static str {
        match self.nsub {
            Some(10) => "neutron",
            Some(12) => "thermal scattering",
            Some(20_040) => "alpha",
            Some(0) => "photo-nuclear",
            Some(_) => "other",
            None => "unreadable header",
        }
    }
}

/// The k_eff job of Step 5.
#[derive(Clone, Debug, PartialEq)]
pub struct KeffJob {
    pub label: String,
    pub particles: usize,
    pub inactive: usize,
    pub active: usize,
    pub seed: u64,
    pub threads: usize,
    pub temperature_k: f64,
    pub bins_per_decade: usize,
}

/// What a k_eff run returns.
#[derive(Clone, Debug, PartialEq)]
pub struct KeffOutcome {
    pub label: String,
    pub k: f64,
    pub sigma: f64,
    pub k_by_generation: Vec<f64>,
    pub entropy: Vec<f64>,
    pub histories: u64,
    pub lost_locate: u64,
    pub transport_s: f64,
    /// Energy-bin edges \[eV\] of the spectrum.
    pub edges: Vec<f64>,
    /// Flux per unit lethargy in each bin, normalised to unit total flux,
    /// with its relative standard error.
    pub phi_per_lethargy: Vec<(f64, f64)>,
    /// Modelling notes the data layout records (mixed libraries,
    /// substitutions, ablations).
    pub notes: Vec<String>,
}

/// Geometry facts the UI shows after assembly.
#[derive(Clone, Debug, PartialEq)]
pub struct AssemblyInfo {
    pub rings: usize,
    pub layers: usize,
    pub tiles: usize,
    pub cells: usize,
    pub universes: usize,
    pub bed_radius: f64,
    pub bed_height: f64,
    pub balls: Option<usize>,
    pub z_range: [f64; 2],
    /// Bed half-height, conus floor and cavity top \[cm\].
    pub bed_half_height: f64,
    pub conus_floor: f64,
    pub cavity_top: f64,
    /// A fuel pebble's centre and one of its TRISO particles' centre \[cm\],
    /// for the review gate's zoomed views.
    pub pebble: Option<[f64; 3]>,
    pub particle: Option<[f64; 3]>,
    pub seconds: f64,
}

pub enum Req {
    /// Step 0: list a folder's tapes and check them against `needed`.
    ScanEndf {
        dir: PathBuf,
        needed: Vec<NeededTape>,
    },
    /// Steps 1-4: build the geometry (geometry engine).
    Assemble { rings: usize, layers: usize },
    /// Hand an assembled core to the physics engine.
    UseCore(Arc<AssembledCore>),
    /// A slice of the assembled geometry, raw pixels for the main view.
    Render {
        id: u64,
        basis: PlotBasis,
        origin: [f64; 3],
        width: [f64; 2],
        pixels: [usize; 2],
    },
    /// The same slice with title, legend and axes, written as a PNG.
    ExportPng {
        basis: PlotBasis,
        origin: [f64; 3],
        width: [f64; 2],
        pixels: [usize; 2],
        title: String,
        path: PathBuf,
    },
    /// Step 5.
    RunKeff(KeffJob),
    /// Rasterise one page of a PDF for the literature pane (0-based page).
    RenderPage { pdf: PathBuf, page: usize },
}

pub enum Ev {
    Scan {
        dir: PathBuf,
        tapes: Vec<TapeInfo>,
        needed: Vec<NeededTape>,
    },
    Assembled(AssemblyInfo, Arc<AssembledCore>),
    Slice {
        id: u64,
        basis: PlotBasis,
        origin: [f64; 3],
        width: [f64; 2],
        image: ImageData,
    },
    Exported(PathBuf),
    /// The nuclide plan of a data load, in processing order.
    DataPlan(Vec<String>),
    Data(LoadProgress),
    DataReady {
        seconds: f64,
        cached: bool,
    },
    KeffStarted {
        planned_histories: u64,
    },
    KeffDone(KeffOutcome),
    Page {
        pdf: PathBuf,
        page: usize,
        pages: usize,
        width: usize,
        height: usize,
        rgba: Vec<u8>,
    },
    Error(String),
}

// `Message` is a blanket impl natively (any `Send + Sync + 'static` type).

/// Nuclear data processed at one temperature, kept for the next run.
struct DataCache {
    temperature_k: f64,
    nuclides: Vec<Nuclide>,
    materials: Vec<Material>,
    notes: Vec<String>,
}

#[derive(Default)]
pub struct Engine {
    core: Option<Arc<AssembledCore>>,
    data: Option<DataCache>,
}

/// The HTR-10 material palette, as the committed geometry images use.
pub fn palette() -> Vec<(outram_mc_libs::geometry::plot::Rgb, &'static str)> {
    nee_soon::htr10_rmc::plots::palette()
}

impl NativeEngine for Engine {
    type Req = Req;
    type Ev = Ev;

    fn handle(&mut self, req: Req, post: &mut impl FnMut(Ev)) {
        match req {
            Req::ScanEndf { dir, mut needed } => {
                let tapes = scan_endf(&dir);
                for n in &mut needed {
                    n.in_folder = tapes.iter().any(|t| t.file == n.file);
                    n.at_default = n.default_path.exists();
                }
                post(Ev::Scan { dir, tapes, needed });
            }
            Req::Assemble { rings, layers } => {
                let t = Instant::now();
                let core = Arc::new(assemble_explicit_triso(rings, layers, 0));
                let found = find_fuel_pebble(&core.geometry);
                let info = AssemblyInfo {
                    rings,
                    layers,
                    tiles: core.tiles,
                    cells: core.cells,
                    universes: core.universes,
                    bed_radius: core.bed_radius,
                    bed_height: 2.0 * core.bed_half_height,
                    balls: core.bed.as_ref().and_then(|b| b.core_balls()),
                    z_range: [core.refl_bottom, core.refl_top],
                    bed_half_height: core.bed_half_height,
                    conus_floor: core.conus_floor,
                    cavity_top: core.cavity_top,
                    pebble: found.map(|f| f.0),
                    particle: found.map(|f| f.1),
                    seconds: t.elapsed().as_secs_f64(),
                };
                self.core = Some(core.clone());
                post(Ev::Assembled(info, core));
            }
            Req::UseCore(core) => self.core = Some(core),
            Req::Render {
                id,
                basis,
                origin,
                width,
                pixels,
            } => {
                let Some(core) = &self.core else {
                    post(Ev::Error("no geometry assembled yet".into()));
                    return;
                };
                let plot = SlicePlot::new(
                    basis,
                    Position::new(origin[0], origin[1], origin[2]),
                    width,
                    pixels,
                );
                let (raw, _) = render_material_slice(&core.geometry, &plot, &palette(), "");
                post(Ev::Slice {
                    id,
                    basis,
                    origin,
                    width,
                    image: raw,
                });
            }
            Req::ExportPng {
                basis,
                origin,
                width,
                pixels,
                title,
                path,
            } => {
                let Some(core) = &self.core else {
                    post(Ev::Error("no geometry assembled yet".into()));
                    return;
                };
                let plot = SlicePlot::new(
                    basis,
                    Position::new(origin[0], origin[1], origin[2]),
                    width,
                    pixels,
                );
                let (_, img) = render_material_slice(&core.geometry, &plot, &palette(), &title);
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match img.write_png(&path) {
                    Ok(()) => post(Ev::Exported(path)),
                    Err(e) => post(Ev::Error(format!(
                        "could not write {}: {e}",
                        path.display()
                    ))),
                }
            }
            Req::RunKeff(job) => self.run_keff(job, post),
            Req::RenderPage { pdf, page } => post(render_page(&pdf, page)),
        }
    }
}

impl Engine {
    fn run_keff(&mut self, job: KeffJob, post: &mut impl FnMut(Ev)) {
        let Some(core) = self.core.clone() else {
            post(Ev::Error(
                "assemble the geometry (Steps 1-4) before running".into(),
            ));
            return;
        };
        let core = core.as_ref();
        // Nuclear data, processed once per temperature.
        let fresh = self
            .data
            .as_ref()
            .is_none_or(|d| (d.temperature_k - job.temperature_k).abs() > 1e-9);
        if fresh {
            let cfg = Htr10DataConfig {
                temperature: ThermodynamicTemperature::new::<kelvin>(job.temperature_k),
                ..Htr10DataConfig::default()
            };
            let layout = match Htr10NuclideLayout::plan(&cfg) {
                Ok(l) => l,
                Err(e) => {
                    post(Ev::Error(format!("nuclide plan refused: {e}")));
                    return;
                }
            };
            post(Ev::DataPlan(planned_items(&layout)));
            let mut diag = RunDiagnostics::new("hifi_workbench");
            let t = Instant::now();
            let loaded =
                load_htr10_nuclides_with_progress(&cfg, &layout, &mut diag, |p| post(Ev::Data(p)));
            let nuclides = match loaded {
                Ok(n) => n,
                Err(e) => {
                    post(Ev::Error(format!("nuclear data: {e}")));
                    return;
                }
            };
            let materials = htr10_material_set(
                &layout,
                Htr10MaterialConfig::benchmark_default(job.temperature_k),
            );
            self.data = Some(DataCache {
                temperature_k: job.temperature_k,
                nuclides,
                materials,
                notes: layout.notes.clone(),
            });
            post(Ev::DataReady {
                seconds: t.elapsed().as_secs_f64(),
                cached: false,
            });
        } else {
            post(Ev::DataReady {
                seconds: 0.0,
                cached: true,
            });
        }
        let Some(data) = &self.data else { return };

        // The spectrum tally: track-length flux on log bins, 1e-5 eV - 20 MeV.
        let edges = log_edges(1.0e-5, 2.0e7, job.bins_per_decade.max(1));
        let n_bins = edges.len() - 1;
        let mut tally = Tally {
            id: 1,
            name: "workbench spectrum".into(),
            filters: vec![FilterKind::Energy(EnergyFilter {
                bins: edges.clone(),
            })],
            scores: vec![ScoreType::Flux],
            bins: vec![TallyBin::default(); n_bins],
        };
        let settings = KeffSettings {
            n_particles: job.particles,
            n_inactive: job.inactive,
            n_active: job.active,
            temperature_k: job.temperature_k,
            seed: job.seed,
            compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(job.threads.max(1))),
            ..KeffSettings::default()
        };
        post(Ev::KeffStarted {
            planned_histories: (job.particles * (job.inactive + job.active)) as u64,
        });
        // The same call as `nee_soon::htr10_rmc::keff_vs_height::run_core`,
        // with a tally attached.
        let majorant = bed_majorant(&data.materials, &data.nuclides);
        let mesh = fissile_entropy_mesh(core);
        let t = Instant::now();
        let res = run_keff_csg_hybrid(
            &core.geometry,
            &data.materials,
            &data.nuclides,
            std::slice::from_ref(&majorant),
            Some(&mesh),
            fissile_source_box(core),
            &settings,
            Some(&mut tally),
        );
        let transport_s = t.elapsed().as_secs_f64();
        let n = job.active as u64;
        let total: f64 = tally
            .bins
            .iter()
            .map(|b| b.mean(n))
            .sum::<f64>()
            .max(f64::MIN_POSITIVE);
        let phi_per_lethargy = tally
            .bins
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let du = (edges[i + 1] / edges[i]).ln();
                (b.mean(n) / total / du, b.rel_std_dev(n))
            })
            .collect();
        post(Ev::KeffDone(KeffOutcome {
            label: job.label,
            k: res.k_mean,
            sigma: res.k_std,
            k_by_generation: res.k_by_generation,
            entropy: res.entropy,
            histories: res.histories,
            lost_locate: res.lost_locate,
            transport_s,
            edges,
            phi_per_lethargy,
            notes: data.notes.clone(),
        }));
    }
}

/// A fuel pebble's centre and a TRISO particle's centre near the core axis,
/// found by locating kernels in the assembled geometry.
///
/// Ported from `find_fuel_pebble` in `crates/nee_soon/examples/htr10_geometry_images.rs`
/// (a 2-D scan, stepped through one TRISO pitch in z, because the lattice puts
/// particle centres at fractional-pitch offsets and one line can miss every
/// kernel).
pub fn find_fuel_pebble(
    g: &outram_mc_libs::geometry::geometry::Geometry,
) -> Option<([f64; 3], [f64; 3])> {
    use nee_soon::htr10_rmc::core_model::mat;
    use outram_mc_libs::geometry::cell::SurfaceToken;
    use outram_mc_libs::geometry::position::Direction;
    let u = Direction::new(0.0, 0.0, 1.0);
    for (kz, j) in (0..20).flat_map(|kz| (0..40).map(move |j| (kz, j))) {
        for i in 0..4000 {
            let p = Position::new(0.01 * i as f64, 0.01 * j as f64, 0.01 * kz as f64);
            if let Some(path) = g.locate(p, u, SurfaceToken::NONE) {
                let n = path.levels.len();
                if path.material == Some(mat::KERNEL) && n >= 3 {
                    let tile = &path.levels[n - 2];
                    let peb = tile.offset + g.cells[tile.cell].translation;
                    let part = path.levels[n - 1].offset;
                    return Some(([peb.x, peb.y, peb.z], [part.x, part.y, part.z]));
                }
            }
        }
    }
    None
}

/// The thermal laws and nuclide slots of `layout`, in the order
/// `load_htr10_nuclides_with_progress` processes them.
pub fn planned_items(layout: &Htr10NuclideLayout) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for law in layout.slots.iter().filter_map(|s| s.thermal) {
        let l = law.label().to_string();
        if !v.contains(&l) {
            v.push(l);
        }
    }
    v.extend(layout.slots.iter().map(|s| s.name.to_string()));
    v
}

/// One tape the data plan reads, and where it was found.
#[derive(Clone, Debug, PartialEq)]
pub struct NeededTape {
    /// File name.
    pub file: String,
    /// Where the HTR-10 loader reads it (`reference-data/endf/` or the ACE
    /// submodule's ENDF/B-VIII.0 folder).
    pub default_path: PathBuf,
    /// Present in the scanned folder.
    pub in_folder: bool,
    /// Present where the loader reads it.
    pub at_default: bool,
}

/// The tapes a run at the default configuration reads.
pub fn needed_tapes() -> Vec<NeededTape> {
    use nee_soon::htr10_rmc::data::{DataDir, ThermalLaw};
    let cfg = Htr10DataConfig::default();
    let Ok(layout) = Htr10NuclideLayout::plan(&cfg) else {
        return Vec::new();
    };
    let mut v: Vec<NeededTape> = Vec::new();
    let mut add = |p: PathBuf| {
        let Some(f) = p.file_name().and_then(|f| f.to_str()).map(str::to_string) else {
            return;
        };
        if !v.iter().any(|x| x.file == f) {
            v.push(NeededTape {
                file: f,
                default_path: p,
                in_folder: false,
                at_default: false,
            });
        }
    };
    for s in &layout.slots {
        add(s.tape.path());
        if let Some(ThermalLaw::Graphite { file, .. }) = s.thermal {
            add(DataDir::Endf.path().join(file));
        }
    }
    v
}

/// `n` log-spaced bins per decade from `lo` to `hi` eV (edges).
pub fn log_edges(lo: f64, hi: f64, per_decade: usize) -> Vec<f64> {
    let decades = (hi / lo).log10();
    let n = (decades * per_decade as f64).ceil() as usize;
    (0..=n)
        .map(|i| lo * 10f64.powf(decades * i as f64 / n as f64))
        .collect()
}

/// Read every `*.endf` (and `*.dat`) in `dir`: MAT, ZSYMAM and NSUB from the
/// MF1/MT451 header.
pub fn scan_endf(dir: &Path) -> Vec<TapeInfo> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let p = e.path();
        let is_tape = p
            .extension()
            .and_then(|x| x.to_str())
            .is_some_and(|x| x == "endf" || x == "dat");
        if !is_tape {
            continue;
        }
        let file = p
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or_default()
            .to_string();
        out.push(read_header(&p, file));
    }
    out.sort_by(|a, b| a.file.cmp(&b.file));
    out
}

fn read_header(path: &Path, file: String) -> TapeInfo {
    use std::io::{BufRead, BufReader};
    let mut info = TapeInfo {
        file,
        mat: None,
        symbol: String::new(),
        nsub: None,
    };
    let Ok(f) = std::fs::File::open(path) else {
        return info;
    };
    let lines: Vec<String> = BufReader::new(f)
        .lines()
        .take(6)
        .map_while(Result::ok)
        .collect();
    let field = |l: &str, a: usize, b: usize| {
        l.get(a..b.min(l.len()))
            .map(str::trim)
            .unwrap_or("")
            .to_string()
    };
    if let Some(l) = lines.get(1) {
        info.mat = field(l, 66, 70).parse().ok();
    }
    if let Some(l) = lines.get(3) {
        info.nsub = field(l, 44, 55).parse().ok();
    }
    if let Some(l) = lines.get(5) {
        info.symbol = field(l, 0, 11);
    }
    info
}

/// Rasterise page `page` (0-based) of `pdf` with kovan's PDF engine.
fn render_page(pdf: &Path, page: usize) -> Ev {
    use kopitiam_pdf::mupdf::{rasterize_page, PdfDocument};
    let bytes = match std::fs::read(pdf) {
        Ok(b) => b,
        Err(e) => return Ev::Error(format!("could not read {}: {e}", pdf.display())),
    };
    let doc = match PdfDocument::open(bytes) {
        Ok(d) => d,
        Err(e) => return Ev::Error(format!("could not open {}: {e:?}", pdf.display())),
    };
    let pages = doc.page_count();
    let page = page.min(pages.saturating_sub(1));
    match rasterize_page(&doc, page, 110.0) {
        Ok(px) => {
            let (w, h) = (px.w as usize, px.h as usize);
            let rgba = if px.alpha {
                px.samples.clone()
            } else {
                px.samples
                    .chunks(3)
                    .flat_map(|c| [c[0], c[1], c[2], 255])
                    .collect()
            };
            Ev::Page {
                pdf: pdf.to_path_buf(),
                page,
                pages,
                width: w,
                height: h,
                rgba,
            }
        }
        Err(e) => Ev::Error(format!("could not render page {}: {e:?}", page + 1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_edges_span_the_range_with_the_asked_density() {
        let e = log_edges(1.0e-5, 2.0e7, 10);
        assert!((e[0] - 1.0e-5).abs() < 1e-18);
        assert!((e.last().copied().unwrap_or(0.0) - 2.0e7).abs() < 1e-3);
        assert_eq!(e.len(), 125);
        assert!(e.windows(2).all(|w| w[1] > w[0]));
    }

    /// The scan reads the reference library's own headers: U-235 is MAT 9228,
    /// an incident-neutron tape.
    #[test]
    fn the_scan_reads_mat_and_sublibrary_from_the_reference_tapes() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/endf");
        let tapes = scan_endf(&dir);
        let u235 = tapes
            .iter()
            .find(|t| t.file.starts_with("n-092_U_235"))
            .expect("U-235 tape");
        assert_eq!(u235.mat, Some(9228));
        assert_eq!(u235.kind(), "neutron");
        assert!(u235.symbol.contains("235"), "{:?}", u235.symbol);
    }

    /// Every tape the HTR-10 data plan reads is in `reference-data/endf/` or
    /// the ACE submodule (the Step 0 check relies on this list being right).
    #[test]
    fn the_default_plan_needs_tapes_that_exist() {
        let need = needed_tapes();
        assert!(need.iter().any(|t| t.file.contains("U_235")), "{need:?}");
        for t in &need {
            assert!(
                t.default_path.exists(),
                "{} is in the plan but not at {}",
                t.file,
                t.default_path.display()
            );
        }
    }
}
