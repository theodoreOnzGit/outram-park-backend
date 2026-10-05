//! The workbench's engine thread: everything that takes longer than a frame.
//!
//! The UI thread only draws and handles input (the no-lag HARD RULE); every
//! request below runs here, on the thread `web_demo::link::start_native`
//! starts, and posts its result back as an [`Ev`]. Each request is one
//! self-contained job. A k_eff run is the longest. ~~The CSG driver returns
//! the per-generation k only at the end, so the console fills then (gh:#579).~~
//! **UPDATED 2026-10-05:** it now runs through
//! `run_keff_csg_hybrid_with_progress`, which reports every generation as it
//! finishes (as `openmc.run()` prints one), so the console fills live.
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
use outram_mc_libs::physics::transport_csg::{run_keff_csg_hybrid_with_progress, GenerationProgress};
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

/// Generations of the running k_eff, written by the physics thread as each
/// finishes and read by the UI every frame (shared mutable state is
/// `Arc<RwLock<T>>`, the workspace rule).
pub type LiveGenerations = Arc<std::sync::RwLock<Vec<GenerationProgress>>>;

/// The k_eff job of Step 5.
#[derive(Clone, Debug)]
pub struct KeffJob {
    /// Where each finished generation is appended while the run goes.
    pub live: LiveGenerations,
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
    /// `Some(n)` when built from `n` DEM pebble centres; `None` for the lattice.
    pub dem_pebbles: Option<usize>,
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
    /// Build it from a DEM pour's pebble centres \[m, DEM frame\] with
    /// `nee_soon`'s explicit bed (paper fuel assignment: 57:43 above the
    /// floor, conus and tube all dummy).
    AssembleFromCentres { centres_m: Vec<[f64; 3]>, rings: usize },
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
    /// A ray-traced 3D view of the assembled geometry (the 3D viewport).
    Render3d(View3dJob),
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
        layout: EndfLayout,
        tapes: Vec<TapeInfo>,
        needed: Vec<NeededTape>,
    },
    Assembled(AssemblyInfo, Arc<AssembledCore>),
    View3d {
        id: u64,
        image: ImageData,
        seconds: f64,
    },
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

    /// Every request runs under `catch_unwind`: a panic in a solver call
    /// (an assembly the builder refuses, a data load that asserts) becomes an
    /// [`Ev::Error`] the UI shows, and this thread stays alive for the next
    /// request instead of dying silently and leaving the UI waiting.
    fn handle(&mut self, req: Req, post: &mut impl FnMut(Ev)) {
        let what = req.name();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.handle_inner(req, post)
        }));
        if let Err(e) = result {
            let msg = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_string()))
                .unwrap_or_else(|| "unknown panic".into());
            post(Ev::Error(format!("{what} failed: {msg}")));
        }
    }
}

impl Req {
    /// What the request does, for error messages.
    pub fn name(&self) -> &'static str {
        match self {
            Req::ScanEndf { .. } => "ENDF scan",
            Req::Assemble { .. } => "geometry assembly",
            Req::AssembleFromCentres { .. } => "geometry assembly from the DEM bed",
            Req::UseCore(_) => "geometry hand-over",
            Req::Render { .. } => "slice rendering",
            Req::Render3d(_) => "3D rendering",
            Req::ExportPng { .. } => "PNG export",
            Req::RunKeff(_) => "Monte Carlo run",
            Req::RenderPage { .. } => "PDF page rendering",
        }
    }
}

impl Engine {
    fn handle_inner(&mut self, req: Req, post: &mut impl FnMut(Ev)) {
        match req {
            Req::ScanEndf { dir, mut needed } => {
                let (layout, tapes) = scan_endf(&dir);
                for n in &mut needed {
                    n.found = tapes.iter().find(|t| n.matches(t)).map(|t| t.file.clone());
                    n.in_folder = n.found.is_some();
                    n.at_default = n.default_path.exists();
                }
                post(Ev::Scan { dir, layout, tapes, needed });
            }
            Req::Assemble { rings, layers } => {
                let t = Instant::now();
                let core = Arc::new(assemble_explicit_triso(rings, layers, 0));
                let info = assembly_info(&core, rings, layers, None, t);
                self.core = Some(core.clone());
                post(Ev::Assembled(info, core));
            }
            Req::AssembleFromCentres { centres_m, rings } => {
                use nee_soon::htr10_rmc::explicit_bed::{
                    assemble_explicit_triso_from_centres, paper_fuel_assignment,
                };
                use uom::si::f64::Length;
                use uom::si::length::meter;
                let t = Instant::now();
                let centres: Vec<[Length; 3]> = centres_m
                    .iter()
                    .map(|c| c.map(Length::new::<meter>))
                    .collect();
                let fuel = paper_fuel_assignment(&centres);
                let core = Arc::new(assemble_explicit_triso_from_centres(&centres, &fuel, rings, 0));
                let info = assembly_info(&core, rings, 0, Some(centres.len()), t);
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
            Req::Render3d(job) => {
                let Some(core) = &self.core else {
                    post(Ev::Error("no geometry assembled yet".into()));
                    return;
                };
                let t = Instant::now();
                let image = render_3d(&core.geometry, &job);
                post(Ev::View3d {
                    id: job.id,
                    image,
                    seconds: t.elapsed().as_secs_f64(),
                });
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
            let mut diag = RunDiagnostics::new("dhoby-ghaut");
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
        if let Ok(mut l) = job.live.write() {
            l.clear();
        }
        let live = job.live.clone();
        let res = run_keff_csg_hybrid_with_progress(
            &core.geometry,
            &data.materials,
            &data.nuclides,
            std::slice::from_ref(&majorant),
            Some(&mesh),
            fissile_source_box(core),
            &settings,
            Some(&mut tally),
            move |g| {
                if let Ok(mut l) = live.write() {
                    l.push(g);
                }
            },
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

fn assembly_info(
    core: &AssembledCore,
    rings: usize,
    layers: usize,
    dem_pebbles: Option<usize>,
    t: Instant,
) -> AssemblyInfo {
    let found = find_fuel_pebble(&core.geometry);
    AssemblyInfo {
        dem_pebbles,
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
    }
}

/// How the 3D viewport draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shading {
    /// Phong-shaded opaque surfaces (OpenMC's solid ray trace).
    Solid,
    /// Every material faintly transparent, boundaries outlined (OpenMC's
    /// wireframe ray trace, "x-ray").
    XRay,
}

/// One 3D render: the camera, and which materials are drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct View3dJob {
    pub id: u64,
    pub eye: [f64; 3],
    pub look_at: [f64; 3],
    pub pixels: [usize; 2],
    /// `None` is perspective (degrees of horizontal field of view in
    /// `fov_deg`); `Some(width)` is orthographic, `width` cm across.
    pub ortho_width: Option<f64>,
    pub fov_deg: f64,
    pub shading: Shading,
    /// Per material index: drawn or hidden (the cutaway).
    pub visible: Vec<bool>,
    /// A section plane `(normal into the kept side, offset)`; solid shading
    /// only (`outram-blender`'s `ClipPlane`, an extension of the OpenMC port).
    pub clip: Option<([f64; 3], f64)>,
}

/// Ray-trace `job` through `geom` with `outram_mc_libs::geometry::plot`'s
/// port of OpenMC's ray-traced plots (rows in parallel). Hidden materials are
/// left out of the solid plot's opaque set, or given a near-zero attenuation
/// in x-ray mode, so the view sees through them.
pub fn render_3d(
    geom: &outram_mc_libs::geometry::geometry::Geometry,
    job: &View3dJob,
) -> ImageData {
    use outram_mc_libs::geometry::plot::{
        material_count, Camera, ClipPlane, ColourScheme, PlotColourBy, Projection, Rgb, SolidRayTracePlot,
        WireframeRayTracePlot, DEFAULT_PLOTTER_SEED,
    };
    use outram_mc_libs::geometry::position::Direction;
    let pal = palette();
    let n = material_count(geom).max(pal.len());
    let mut seed = DEFAULT_PLOTTER_SEED;
    let mut scheme = ColourScheme::new(PlotColourBy::Material, n, &mut seed)
        .with_background(Rgb::new(48, 48, 52));
    for (i, (c, _)) in pal.iter().enumerate() {
        scheme = scheme.with_colour(i, *c);
    }
    let camera = Camera {
        position: Position::new(job.eye[0], job.eye[1], job.eye[2]),
        look_at: Position::new(job.look_at[0], job.look_at[1], job.look_at[2]),
        up: Direction::new(0.0, 0.0, 1.0),
        pixels: job.pixels,
        projection: match job.ortho_width {
            Some(width) => Projection::Orthographic { width },
            None => Projection::Perspective {
                horizontal_fov_deg: job.fov_deg,
            },
        },
    };
    let shown = |i: usize| job.visible.get(i).copied().unwrap_or(true);
    match job.shading {
        Shading::Solid => {
            let mut plot = SolidRayTracePlot::new(camera, n);
            for i in (0..n).filter(|&i| shown(i)) {
                plot = plot.with_opaque(i);
            }
            plot.diffuse_fraction = 0.35;
            if let Some((normal, offset)) = job.clip {
                plot = plot.with_clip(ClipPlane { normal, offset });
            }
            plot.create_image(geom, &scheme)
        }
        Shading::XRay => {
            let mut plot = WireframeRayTracePlot::new(camera, n);
            for i in 0..n {
                plot = plot.with_xs(i, if shown(i) { 0.02 } else { 0.0 });
            }
            // Outline only what is shown: hidden pebbles' boundaries would
            // otherwise cover the picture.
            plot.wireframe_ids = (0..n).filter(|&i| shown(i)).collect();
            plot.create_image(geom, &scheme)
        }
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
    /// File name (the workspace copy's).
    pub file: String,
    /// MAT and sub-library read from that copy's header: the tape's identity,
    /// which survives a library's own file naming (`n-092_U_235.endf` in an
    /// extracted ENDF/B-VIII.0 against `n-092_U_235-ENDF8.0.endf` here).
    pub mat: Option<i32>,
    pub nsub: Option<i64>,
    /// The scanned tape that matched, as a path relative to the folder.
    pub found: Option<String>,
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
            let h = read_header(&p, f.clone());
            v.push(NeededTape {
                file: f,
                mat: h.mat,
                nsub: h.nsub,
                found: None,
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

impl NeededTape {
    /// Whether scanned tape `t` is this one: same MAT and sub-library, read
    /// from the headers; by file name only if a header could not be read.
    pub fn matches(&self, t: &TapeInfo) -> bool {
        match (self.mat, self.nsub, t.mat, t.nsub) {
            (Some(m), Some(s), Some(tm), Some(ts)) => m == tm && s == ts,
            _ => t.file.rsplit('/').next() == Some(self.file.as_str()),
        }
    }
}

/// How a scanned ENDF folder is laid out.
#[derive(Clone, Debug, PartialEq)]
pub enum EndfLayout {
    /// Tapes directly in the folder (`reference-data/endf/`, or a library's
    /// `neutrons/` picked on its own).
    Flat,
    /// A freshly extracted library (e.g. `ENDF-B-VIII.0/`), its tapes in
    /// sub-library folders. `scanned` are the ones read (incident neutrons and
    /// thermal scattering, what transport needs); `others` are listed only.
    Library { scanned: Vec<String>, others: Vec<String> },
}

/// The sub-library folders transport reads, in an extracted ENDF library.
const LIBRARY_SUBDIRS: [&str; 2] = ["neutrons", "thermal_scatt"];

/// Read every `*.endf` (and `*.dat`) tape `dir` holds: MAT, ZSYMAM and NSUB
/// from the MF1/MT451 header. Understands both a flat folder of tapes and a
/// freshly extracted library with `neutrons/`, `thermal_scatt/`, ...
/// sub-folders; tape names are then relative to `dir` (`neutrons/n-001_H_001.endf`).
pub fn scan_endf(dir: &Path) -> (EndfLayout, Vec<TapeInfo>) {
    let subdirs: Vec<String> = std::fs::read_dir(dir)
        .map(|es| {
            es.flatten()
                .filter(|e| e.path().is_dir())
                .filter_map(|e| e.file_name().to_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let is_library = LIBRARY_SUBDIRS.iter().any(|s| subdirs.iter().any(|d| d == s));
    if !is_library {
        return (EndfLayout::Flat, scan_flat(dir, ""));
    }
    let mut scanned = Vec::new();
    let mut tapes = Vec::new();
    for s in LIBRARY_SUBDIRS {
        if subdirs.iter().any(|d| d == s) {
            scanned.push(s.to_string());
            tapes.extend(scan_flat(&dir.join(s), &format!("{s}/")));
        }
    }
    let mut others: Vec<String> = subdirs.into_iter().filter(|d| !LIBRARY_SUBDIRS.contains(&d.as_str())).collect();
    others.sort();
    // Tapes at the top level too, if any.
    tapes.extend(scan_flat(dir, ""));
    tapes.sort_by(|a, b| a.file.cmp(&b.file));
    (EndfLayout::Library { scanned, others }, tapes)
}

fn scan_flat(dir: &Path, prefix: &str) -> Vec<TapeInfo> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let p = e.path();
        let is_tape = p.is_file()
            && p.extension().and_then(|x| x.to_str()).is_some_and(|x| x == "endf" || x == "dat");
        if !is_tape {
            continue;
        }
        let file = format!("{prefix}{}", p.file_name().and_then(|f| f.to_str()).unwrap_or_default());
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
        Ok(px) => match pixmap_rgba(&px) {
            Some(rgba) => Ev::Page {
                pdf: pdf.to_path_buf(),
                page,
                pages,
                width: px.w as usize,
                height: px.h as usize,
                rgba,
            },
            None => Ev::Error(format!(
                "page {} came back in an unexpected pixel format (n = {}, alpha = {}, stride = {})",
                page + 1,
                px.n,
                px.alpha,
                px.stride
            )),
        },
        Err(e) => Ev::Error(format!("could not render page {}: {e:?}", page + 1)),
    }
}

/// RGBA bytes from a `kopitiam-pdf` pixmap of any layout: grey, RGB or CMYK,
/// with or without alpha, rows padded to `stride`. `None` (never a panic) if
/// the buffer is shorter than its own header says.
fn pixmap_rgba(px: &kopitiam_pdf::mupdf::Pixmap) -> Option<Vec<u8>> {
    let (w, h, n) = (px.w as usize, px.h as usize, px.n as usize);
    let colour = n.checked_sub(usize::from(px.alpha))?;
    if n == 0
        || !matches!(colour, 1 | 3 | 4)
        || px.stride < w * n
        || px.samples.len() < px.stride * h.saturating_sub(1) + w * n
    {
        return None;
    }
    let mut out = Vec::with_capacity(w * h * 4);
    for row in 0..h {
        let line = &px.samples[row * px.stride..row * px.stride + w * n];
        for p in line.chunks_exact(n) {
            let a = if px.alpha { p[n - 1] } else { 255 };
            let [r, g, b] = match colour {
                1 => [p[0]; 3],
                3 => [p[0], p[1], p[2]],
                // CMYK, naive conversion: good enough to read a page by.
                _ => {
                    let k = 255 - u16::from(p[3]);
                    let f = |c: u8| ((255 - u16::from(c)) * k / 255) as u8;
                    [f(p[0]), f(p[1]), f(p[2])]
                }
            };
            out.extend_from_slice(&[r, g, b, a]);
        }
    }
    Some(out)
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
        let (layout, tapes) = scan_endf(&dir);
        assert_eq!(layout, EndfLayout::Flat);
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

    /// An extracted ENDF library (sub-library folders, the library's own file
    /// names) is recognised, and the needed tapes are found in it by MAT and
    /// sub-library, not by name. Built from copies of three reference tapes
    /// renamed the way the official ENDF/B-VIII.0 archive names them.
    #[test]
    fn an_extracted_library_is_scanned_and_matched_by_identity() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/endf");
        let root = std::env::temp_dir().join(format!("dhoby_endf_layout_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("neutrons")).expect("dir");
        std::fs::create_dir_all(root.join("thermal_scatt")).expect("dir");
        std::fs::create_dir_all(root.join("decay")).expect("dir");
        for (from, to) in [
            ("n-092_U_235-ENDF8.0.endf", "neutrons/n-092_U_235.endf"),
            ("n-008_O_016-ENDF8.0.endf", "neutrons/n-008_O_016.endf"),
        ] {
            std::fs::copy(src.join(from), root.join(to)).expect("copy");
        }
        let (layout, tapes) = scan_endf(&root);
        assert_eq!(
            layout,
            EndfLayout::Library { scanned: vec!["neutrons".into(), "thermal_scatt".into()], others: vec!["decay".into()] }
        );
        assert!(tapes.iter().any(|t| t.file == "neutrons/n-092_U_235.endf"));
        let need: Vec<NeededTape> = needed_tapes();
        let u235 = need.iter().find(|n| n.file.starts_with("n-092_U_235")).expect("U-235 is needed");
        let found: Vec<&TapeInfo> = tapes.iter().filter(|t| u235.matches(t)).collect();
        assert_eq!(found.len(), 1, "U-235 found by MAT {:?} / NSUB {:?}", u235.mat, u235.nsub);
        assert_eq!(found[0].file, "neutrons/n-092_U_235.endf");
        let u238 = need.iter().find(|n| n.file.starts_with("n-092_U_238")).expect("U-238 is needed");
        assert!(!tapes.iter().any(|t| u238.matches(t)), "U-238 is not in this small library");
        let _ = std::fs::remove_dir_all(&root);
    }
}
