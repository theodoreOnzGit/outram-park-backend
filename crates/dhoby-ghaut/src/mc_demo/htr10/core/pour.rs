//! **The reader's own DEM pour, as the core's bed** (gh:#1244, maintainer
//! 2026-11-05): "run a pebble bed fill (DEM) based on the ratio of fuel to
//! moderator pebbles, then become MC cross section. Allow the user to fill x
//! number of pebbles up to the maximum of 95 % of bed height".
//!
//! - **The pour is not written here.** It is `dem_web`'s engine
//!   ([`engine`], the same file, by `#[path]`): `outram_park_fork_liggghts::
//!   htr10_fill`, the LIGGGHTS port's `GranularSystem` in the HTR-10 vessel at
//!   the gh:#216 settings (µ 0.1, µ_r 0, E 5e8 Pa, dt 35 µs), one thread, in
//!   chunks the page asks for. In the browser it runs in the page's own
//!   worker, the one that draws the slice, so that worker has the bed too.
//! - **The bed it becomes** ([`poured_bed`]): the settled centres in the DEM
//!   frame (`z = 0` the bed floor), cut at [`HEIGHT_LIMIT_CM`] if any pebble
//!   stands above it, fuel flags by `fuel_assignment_at` at the reader's
//!   fraction (the paper's rule: by height from the floor up, the conus and
//!   tube all moderator), then built exactly as the baked DEM bed is:
//!   `ParametricStudy` with `StudyBed::Explicit`, random TRISO packing
//!   (gh:#1243).
//! - **The cap** ([`max_pebbles`]): the most pebbles whose bed surface is
//!   predicted at 95 % of the cavity, from a fresh pour's measured
//!   whole-core φ and the reference pour's count below the floor. It is a
//!   prediction, so the cut above backs it: a pour that stands higher anyway
//!   loses its pebbles above the line, and the page says how many.


// The pour engine: `nee_soon::dem_pour::engine` (gh:#1655), a library call
// since gh:#1742 (2026-11-25); ~~`#[path = "../../../dem_web/engine.rs"]`~~.
pub use nee_soon::dem_pour::engine;

// The pour model (the 95 % line, the cap, the measured pour times, the
// poured bed) moved to `nee_soon::mc_rungs::htr10_core` (gh:#1742,
// 2026-11-25), unchanged.
#[allow(unused_imports)] // two are read only by this file's tests
pub use nee_soon::mc_rungs::htr10_core::{
    expected_pour_s, max_pebbles, poured_bed, PouredBed, BELOW_FLOOR_PEBBLES, DEFAULT_PEBBLES, FRESH_POUR_PHI, HEIGHT_LIMIT_CM,
    MEASURED_BROWSER_POURS, MIN_PEBBLES, RECORD_CORE_PEBBLES, R_CM,
};

/// A time as the page prints it: "4 min", "about 1 min", "under a minute".
pub fn minutes(s: f64) -> String {
    let m = (s / 60.0).round();
    if s < 45.0 {
        "under a minute".into()
    } else if m <= 1.0 {
        "about 1 min".into()
    } else {
        format!("about {m:.0} min")
    }
}

/// What the page says about a bed in a line.
pub fn describe(b: &PouredBed) -> String {
    let core = b.core_balls();
    let fuel = b.fuel_balls();
    let mut s = format!(
        "your pour #{}: {} core pebbles, {} fuel ({:.1} %), top {:.1} cm of {:.1} cm allowed",
        b.version,
        super::random_bed::grouped(core),
        super::random_bed::grouped(fuel),
        100.0 * fuel as f64 / core.max(1) as f64,
        b.top_cm(),
        HEIGHT_LIMIT_CM
    );
    if b.cut > 0 {
        s.push_str(&format!(" · {} pebbles above the 95 % line removed", b.cut));
    }
    s
}

// The diagnostics, the reference points and the g(r) graph moved to
// `nee_soon::dem_pour` (gh:#1655) so every pebble-bed k_eff simulator shows
// the same pour interface; re-exported here so this rung's call sites stay.
pub use nee_soon::dem_pour::PourDiagnostics;
#[cfg(test)]
use nee_soon::dem_pour::{g_at, REFERENCE_G};

/// **The pour's diagnostics** in this rung's HTR-10 vessel:
/// `nee_soon::dem_pour::diagnostics` (whole-core φ and g(r)) with the
/// vessel's bulk fraction (`engine::vessel_bulk_fraction`) and the worker's
/// clock.
pub fn diagnostics(centres_m: &[f32]) -> PourDiagnostics {
    let t0 = crate::web_demo::platform::now_s();
    let c64: Vec<[f64; 3]> = centres_m.chunks_exact(3).map(|c| [f64::from(c[0]), f64::from(c[1]), f64::from(c[2])]).collect();
    let mut d = nee_soon::dem_pour::diagnostics(centres_m, engine::vessel_bulk_fraction(&c64), 0.0);
    d.secs = crate::web_demo::platform::now_s() - t0;
    d
}

/// **The GPU pour's report** in `dem_web`'s [`engine::Snapshot`] form, so
/// the page treats both pours alike (gh:#1250): the same instruments
/// (`htr10_fill`'s progress, `engine::vessel_bulk_fraction`) over the
/// GPU's read-back state.
pub fn gpu_snapshot(
    id: u32,
    fill: &outram_park_fork_liggghts::htr10_fill::Htr10GpuFill,
    p: outram_park_fork_liggghts::htr10_fill::FillProgress,
    ms_per_step: f64,
) -> engine::Snapshot {
    use uom::si::length::meter;
    use uom::si::time::second;
    let raw = fill.centres();
    let c64: Vec<[f64; 3]> = raw.iter().map(|c| [c.x, c.y, c.z]).collect();
    engine::Snapshot {
        id,
        n: fill.settings().n_pebbles,
        steps: p.steps,
        time_s: p.time.get::<second>(),
        ke_ratio: p.ke_ratio_core,
        phi: p.phi_whole_core,
        phi_bulk: engine::vessel_bulk_fraction(&c64),
        surface_m: p.surface_height.get::<meter>(),
        n_core: p.n_in_core,
        settled: p.settled,
        gave_up: p.gave_up,
        ms_per_step,
        centres: raw.iter().flat_map(|c| [c.x as f32, c.y as f32, c.z as f32]).collect(),
    }
}

/// The message when the browser has no WebGPU.
pub const NO_WEBGPU: &str = "This browser has no WebGPU, which the GPU pour needs (recent Chrome, Edge or Safari; Firefox behind a flag). The CPU pour works everywhere.";

/// Which engine a pour runs on, and what it took: the page's timing table.
#[derive(Clone, Debug, PartialEq)]
pub struct PourTiming {
    pub gpu: bool,
    pub n: usize,
    pub steps: usize,
    pub wall_s: f64,
    pub settled: bool,
}

impl PourTiming {
    /// One line of the table.
    pub fn line(&self) -> String {
        format!(
            "{}: {} pebbles, {} in {:.0} s ({} steps, {:.1} ms/step)",
            if self.gpu { "GPU (WebGPU)" } else { "CPU (one thread)" },
            super::random_bed::grouped(self.n),
            if self.settled { "settled" } else { "stopped, NOT settled," },
            self.wall_s,
            self.steps,
            1e3 * self.wall_s / self.steps.max(1) as f64
        )
    }
}

/// The fewest steps a GPU chunk takes (see `PourUi::on_event`).
pub const GPU_MIN_CHUNK: usize = 200;

/// Compute per request the page aims for, ms: the page's worker also draws
/// the slice, so a chunk is kept short (`dem_web`'s `CHUNK_MS` rule).
pub const CHUNK_MS: f64 = 250.0;

/// **The page's side of the pour** (the request/response rule of `dem_web`'s
/// `app::Pour`, rewritten here without its GUI): one chunk in flight at a
/// time, sized to [`CHUNK_MS`] from the measured cost, stopped by not asking.

pub struct PourUi {
    /// The reader's choices: pebbles to pour, fuel fraction above the floor,
    /// seed.
    pub n: f64,
    pub fuel: f64,
    pub seed: u64,
    /// Whether the panel shows the pour (the "your pour" bed is selected).
    pub shown: bool,
    /// Pour on the GPU (WebGPU, `gpu::dem`) rather than the CPU (gh:#1250).
    pub gpu: bool,
    /// Whether the current pour runs on the GPU.
    running_gpu: bool,
    /// Every finished pour's engine and time, for the side-by-side table.
    pub timings: Vec<PourTiming>,
    /// Pour on arrival and use the bed once settled (`?pour=N`).
    pub auto: bool,
    id: u32,
    in_flight: bool,
    pub paused: bool,
    chunk: usize,
    /// The newest report of the current pour.
    pub last: Option<engine::Snapshot>,
    /// When the current pour started (page clock, s).
    pub started_s: f64,
    /// Wall time when it settled or gave up.
    pub finished_s: Option<f64>,
    /// The bed in use (handed to the workers), and how many were made.
    pub bed: Option<PouredBed>,
    /// The finished pour's diagnostics, once the worker has measured them.
    pub diagnostics: Option<PourDiagnostics>,
    versions: u32,
    pub error: Option<String>,
    out: Vec<super::CoreReq>,
}

impl Default for PourUi {
    fn default() -> Self {
        Self {
            n: DEFAULT_PEBBLES as f64,
            fuel: nee_soon::htr10_rmc::table1::FUEL_BALL_FRACTION,
            seed: 1,
            shown: false,
            gpu: false,
            running_gpu: false,
            timings: Vec::new(),
            auto: false,
            id: 0,
            in_flight: false,
            paused: false,
            chunk: 20,
            last: None,
            started_s: 0.0,
            finished_s: None,
            bed: None,
            diagnostics: None,
            versions: 0,
            error: None,
            out: Vec::new(),
        }
    }
}

impl PourUi {
    /// A pour of `n` pebbles started on arrival and used once settled.
    pub fn auto(n: usize) -> Self {
        Self { n: n as f64, shown: true, auto: true, ..Self::default() }
    }
    /// Start a new pour of `n` pebbles (any running one is dropped).
    pub fn start(&mut self, now_s: f64) {
        self.id += 1;
        let n = (self.n.round() as usize).clamp(MIN_PEBBLES, max_pebbles());
        self.running_gpu = self.gpu;
        self.out.push(self.request(engine::Request::Start { id: self.id, n, seed: self.seed }));
        self.in_flight = true;
        self.paused = false;
        self.chunk = 20;
        self.last = None;
        self.error = None;
        self.started_s = now_s;
        self.finished_s = None;
        self.diagnostics = None;
    }
    /// The request on the current pour's engine.
    fn request(&self, r: engine::Request) -> super::CoreReq {
        if self.running_gpu {
            super::CoreReq::PourGpu(r)
        } else {
            super::CoreReq::Pour(r)
        }
    }

    /// Whether the current pour runs on the GPU.
    pub fn running_on_gpu(&self) -> bool {
        self.running_gpu
    }

    /// A pour has been asked for.
    pub fn started(&self) -> bool {
        self.id > 0
    }
    /// Settled, or out of steps.
    pub fn finished(&self) -> bool {
        self.last.as_ref().is_some_and(|s| s.settled || s.gave_up)
    }
    /// A pour is going (asked for, not finished, not paused).
    pub fn running(&self) -> bool {
        self.id > 0 && !self.finished() && !self.paused && self.error.is_none()
    }
    /// Take in the worker's report; ask for the next chunk if one is due.
    pub fn on_event(&mut self, e: engine::Event, now_s: f64) {
        match e {
            engine::Event::Progress(s) if s.id == self.id => {
                self.in_flight = false;
                if s.ms_per_step > 0.0 {
                    // The GPU pour runs asynchronously in the worker, so a
                    // long chunk does not stall it; its cost per step
                    // includes reading the whole state back, which a short
                    // chunk would repeat every few steps (6 steps a chunk
                    // in the first browser run, 2026-11-05).
                    let floor = if self.running_gpu { GPU_MIN_CHUNK } else { 1 };
                    self.chunk = ((CHUNK_MS / s.ms_per_step) as usize).clamp(floor, 4000);
                }
                let done = s.settled || s.gave_up;
                // The console gets a line every 2 000 steps and at the end,
                // for an unattended check.
                let before = self.last.as_ref().map_or(0, |l| l.steps) / 2000;
                let report = s.steps / 2000 > before || done;
                self.last = Some(s);
                if done && self.finished_s.is_none() {
                    self.finished_s = Some(now_s);
                    if let Some(s) = &self.last {
                        self.timings.push(PourTiming { gpu: self.running_gpu, n: s.n, steps: s.steps, wall_s: now_s - self.started_s, settled: s.settled });
                    }
                    // Measure it once, in the worker.
                    if let Some(s) = &self.last {
                        self.out.push(super::CoreReq::PourDiagnostics {
                            centres_m: crate::web_demo::link::Floats::from_vec(s.centres.iter().map(|&c| f64::from(c)).collect()),
                        });
                    }
                }
                if report {
                    log::info!("htr10 core: pour: {}", self.lines(now_s).join(" | "));
                }
                self.ask();
            }
            engine::Event::Progress(_) => {}
            engine::Event::Error(m) => {
                self.in_flight = false;
                self.error = Some(m);
            }
        }
    }
    fn ask(&mut self) {
        if self.in_flight || self.paused || self.finished() || self.last.is_none() || self.error.is_some() {
            return;
        }
        self.in_flight = true;
        self.out.push(self.request(engine::Request::Step { id: self.id, steps: self.chunk }));
    }
    /// Pause (no further chunk is asked for) or resume.
    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
        self.ask();
    }
    /// **The current pour as the core's bed** at the reader's fuel
    /// fraction, numbered; it becomes [`Self::bed`].
    ///
    /// # Errors
    /// Nothing poured yet, or no pebble above the floor.
    pub fn make_bed(&mut self) -> Result<PouredBed, String> {
        let s = self.last.as_ref().ok_or("nothing has been poured yet")?;
        let b = poured_bed(&s.centres, self.fuel, self.versions + 1)?;
        self.versions += 1;
        self.bed = Some(b.clone());
        Ok(b)
    }
    /// Take in the worker's diagnostics (and log them for an unattended
    /// check).
    pub fn on_diagnostics(&mut self, w: &[f64]) {
        match PourDiagnostics::from_words(w) {
            Ok(d) => {
                for l in d.lines() {
                    log::info!("htr10 core: pour diagnostics: {l}");
                }
                self.diagnostics = Some(d);
            }
            Err(e) => self.error = Some(e),
        }
    }
    /// Ask the worker to measure the current pour now (a paused or used
    /// one; a finished pour is measured by itself).
    pub fn measure(&mut self) {
        if let Some(s) = &self.last {
            self.diagnostics = None;
            self.out.push(super::CoreReq::PourDiagnostics {
                centres_m: crate::web_demo::link::Floats::from_vec(s.centres.iter().map(|&c| f64::from(c)).collect()),
            });
        }
    }
    /// Queue a request for the page's own worker.
    pub fn queue(&mut self, r: super::CoreReq) {
        self.out.push(r);
    }
    /// The requests for the page's own worker.
    pub fn take_out(&mut self) -> Vec<super::CoreReq> {
        std::mem::take(&mut self.out)
    }
    /// The page's lines on the pour: count, steps, φ, surface, cost.
    pub fn lines(&self, now_s: f64) -> Vec<String> {
        let Some(s) = &self.last else {
            return vec![if self.id > 0 { "Seeding the pour…".into() } else { "Not poured yet.".into() }];
        };
        let wall = self.finished_s.unwrap_or(now_s) - self.started_s;
        let state = if s.settled {
            "settled"
        } else if s.gave_up {
            "NOT settled: out of steps"
        } else if self.paused {
            "paused"
        } else {
            "pouring"
        };
        let engine = if self.running_gpu { "GPU" } else { "CPU" };
        vec![
            format!("{} pebbles on the {engine}, {state}: {} steps, {:.2} s simulated, {:.0} s wall", super::random_bed::grouped(s.n), s.steps, s.time_s, wall),
            format!(
                "surface {:.1} cm (95 % line {:.1} cm) · whole-core φ {:.4} · bulk φ {:.4}",
                100.0 * s.surface_m,
                HEIGHT_LIMIT_CM,
                s.phi,
                s.phi_bulk
            ),
            format!("KE ratio {:.2e} (settled below 1e-3) · {:.1} ms per step", s.ke_ratio, s.ms_per_step),
        ]
    }
}

/// **The geometry review images of a poured core** (the crate's drawing
/// rule), from the ASSEMBLED geometry the workers would transport: a pour's
/// CSV (`id,x,y,z,..` in metres, `htr10_fresh_fill`'s output) made into a bed
/// by [`poured_bed`] at `fuel_fraction`, built through
/// [`super::random_bed::USER_POUR`]. Writes the whole model side-on, the bed
/// side-on, a plan at mid-bed, and one fuel pebble with its random TRISO.
/// Returns the bed's description. `--render-pour` runs it.
///
/// # Errors
/// The CSV cannot be read, or the bed cannot be made.
#[cfg(not(target_arch = "wasm32"))]
pub fn render_review(csv: &str, fuel_fraction: f64, dir: &std::path::Path) -> Result<String, String> {
    use outram_mc_libs::geometry::plot::{render_material_slice, PlotBasis, SlicePlot};
    use outram_mc_libs::geometry::position::Position;
    let mut c: Vec<f32> = Vec::new();
    for (i, line) in csv.lines().enumerate().skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        let f: Vec<f64> = line.split(',').map(|v| v.trim().parse::<f64>()).collect::<Result<_, _>>().map_err(|e| format!("line {}: {e}", i + 1))?;
        if f.len() < 4 {
            return Err(format!("line {}: short", i + 1));
        }
        c.extend([f[1] as f32, f[2] as f32, f[3] as f32]);
    }
    let bed = poured_bed(&c, fuel_fraction, 1)?;
    let text = describe(&bed);
    super::random_bed::set_poured(bed.clone());
    let core = super::random_bed::build_core(super::random_bed::USER_POUR)?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let pal = nee_soon::htr10_rmc::plots::palette();
    let (lo, hi) = (core.conus_floor, core.bed_half_height);
    // A fuel pebble near the bed's mid-height, in the core frame.
    let half = 0.5 * bed.top_cm();
    let fuel_at = bed
        .centres
        .iter()
        .zip(&bed.is_fuel)
        .filter(|(_, f)| **f)
        .map(|(p, _)| p.map(|v| v.get::<uom::si::length::centimeter>()))
        .min_by(|a, b| (a[2] - half).abs().total_cmp(&(b[2] - half).abs()))
        .ok_or("no fuel pebble")?;
    let shots = [
        ("pour_core_xz.png", SlicePlot::new(PlotBasis::Xz, Position::new(0.0, 0.0, -15.0), [400.0, 620.0], [400, 620]), "HTR-10, YOUR DEM POUR: X-Z, WHOLE MODEL"),
        ("pour_bed_xz.png", SlicePlot::new(PlotBasis::Xz, Position::new(0.0, 0.0, 0.5 * (lo + hi)), [190.0, hi - lo + 10.0], [600, 600]), "YOUR DEM POUR: X-Z, THE BED"),
        ("pour_bed_xy.png", SlicePlot::new(PlotBasis::Xy, Position::new(0.0, 0.0, 0.0), [190.0, 190.0], [600, 600]), "YOUR DEM POUR: X-Y, MID-BED"),
        (
            "pour_fuel_pebble_xy.png",
            SlicePlot::new(PlotBasis::Xy, Position::new(fuel_at[0], fuel_at[1], fuel_at[2] - half), [7.0, 7.0], [700, 700]),
            "YOUR DEM POUR: ONE FUEL PEBBLE, RANDOM TRISO",
        ),
    ];
    for (name, plot, title) in shots {
        let (_raw, img) = render_material_slice(&core.geometry, &plot, &pal, title);
        img.write_png(&dir.join(name)).map_err(|e| e.to_string())?;
    }
    Ok(text)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use uom::si::length::meter;

    /// The cap: the reference pour's count below the floor is the constant,
    /// and the cap is the φ formula's (gh:#1244).
    #[test]
    fn the_cap_is_derived_from_the_reference_pour() {
        let csv = include_str!("../../../../../../reference-data/liggghts/htr10_conus_presettled_mu10_mur00.csv");
        let below = csv
            .lines()
            .skip(1)
            .filter(|l| !l.trim().is_empty())
            .filter(|l| l.split(',').nth(3).and_then(|z| z.trim().parse::<f64>().ok()).is_some_and(|z| z <= 0.0))
            .count();
        assert_eq!(below, BELOW_FLOOR_PEBBLES);
        assert!((HEIGHT_LIMIT_CM - 210.7271).abs() < 1e-3);
        let m = max_pebbles();
        println!("max pebbles {m}");
        assert_eq!(m, 30_687);
        assert!(m > MIN_PEBBLES);
        assert_eq!(DEFAULT_PEBBLES, 19_347);
        assert!(DEFAULT_PEBBLES < m);
        assert_eq!(PourUi::default().n, DEFAULT_PEBBLES as f64);
        // The time guide passes through every measured pour and rises with n.
        for (n, t) in MEASURED_BROWSER_POURS {
            assert!((expected_pour_s(n) - t).abs() < 1e-9, "{n}");
        }
        assert!(expected_pour_s(MIN_PEBBLES) > 0.0 && expected_pour_s(MIN_PEBBLES) < 242.0);
        let d = expected_pour_s(DEFAULT_PEBBLES);
        assert!(d > 339.0 && d < 1051.0, "{d}");
        assert_eq!(minutes(1051.0), "about 18 min");
        assert_eq!(minutes(70.0), "about 1 min");
        assert_eq!(minutes(30.0), "under a minute");
    }

    /// The diagnostics on the reference pour reproduce its recorded
    /// figures: whole-core φ 0.604688 (V&V § 4.9 bake re-run) and the g(r)
    /// points `bed_rdf_check` recorded (same estimator, domain and bins);
    /// the wire round-trips; a shallow bed has no g(r) and says so.
    #[test]
    fn the_diagnostics_reproduce_the_reference_bed() {
        let csv = include_str!("../../../../../../reference-data/liggghts/htr10_conus_presettled_mu10_mur00.csv");
        let c: Vec<f32> = csv
            .lines()
            .skip(1)
            .filter(|l| !l.trim().is_empty())
            .flat_map(|l| {
                let v: Vec<f64> = l.split(',').map(|x| x.trim().parse().expect("number")).collect();
                [v[1] as f32, v[2] as f32, v[3] as f32]
            })
            .collect();
        let d = diagnostics(&c);
        for l in d.lines() {
            println!("{l}");
        }
        println!("measured in {:.2} s", d.secs);
        // f32 centres (as the worker streams them), so φ to 4 places.
        assert!((d.phi_whole_core - 0.604688).abs() < 2e-4, "{}", d.phi_whole_core);
        assert_eq!(d.r_max_d, 5.0);
        let peak = d.g.iter().copied().fold(0.0, f64::max);
        let [r2, r3, r4, rp] = REFERENCE_G;
        assert!((g_at(&d, 2f64.sqrt()) - r2).abs() < 0.01, "{}", g_at(&d, 2f64.sqrt()));
        assert!((g_at(&d, 3f64.sqrt()) - r3).abs() < 0.01, "{}", g_at(&d, 3f64.sqrt()));
        assert!((g_at(&d, 2.0) - r4).abs() < 0.01, "{}", g_at(&d, 2.0));
        assert!((peak - rp).abs() < 0.2, "{peak}");
        assert_eq!(PourDiagnostics::from_words(&d.to_words()).expect("words"), d);
        let shallow = diagnostics(&[0.0, 0.0, 0.03, 0.07, 0.0, 0.03]);
        assert!(shallow.g.is_empty() && shallow.lines()[1].contains("too shallow"));
        assert!(PourDiagnostics::from_words(&[1.0]).is_err());
    }

    /// A pour becomes a bed: the pebbles above the 95 % line are cut, fuel is
    /// the fraction asked for above the floor and none below, and the wire
    /// form round-trips.
    #[test]
    fn a_pour_becomes_a_bed_cut_at_the_line() {
        // A column of pebbles from the conus up past the line, 6 cm apart.
        let mut c: Vec<f32> = Vec::new();
        for k in 0..45 {
            let z = -0.2 + 0.06 * f64::from(k);
            c.extend([0.0, 0.0, z as f32]);
        }
        let b = poured_bed(&c, 0.5, 7).expect("bed");
        assert!(b.top_cm() <= HEIGHT_LIMIT_CM);
        let above_line = (0..45).filter(|&k| (-0.2 + 0.06 * f64::from(k)) * 100.0 + R_CM > HEIGHT_LIMIT_CM).count();
        assert_eq!(b.cut, above_line);
        assert!(b.cut > 0);
        let core = b.core_balls();
        assert_eq!(b.fuel_balls(), (core as f64 * 0.5).round() as usize, "fuel {} of {core}", b.fuel_balls());
        for (p, f) in b.centres.iter().zip(&b.is_fuel) {
            if p[2].get::<meter>() <= 0.0 {
                assert!(!f, "a conus pebble is fuel");
            }
        }
        let (cm, fl) = b.to_wire();
        let back = PouredBed::from_wire(&cm, &fl, 7, b.cut, 0.5).expect("wire");
        assert_eq!(back.is_fuel, b.is_fuel);
        assert_eq!(back.core_balls(), core);
        assert!(describe(&b).contains("removed"));
        assert!(poured_bed(&[0.0, 0.0, -0.1], 0.5, 1).is_err());
    }

    fn snap(id: u32, steps: usize, ms: f64, settled: bool) -> engine::Snapshot {
        engine::Snapshot {
            id,
            n: 4,
            steps,
            time_s: 0.0,
            ke_ratio: 0.0,
            phi: 0.6,
            phi_bulk: 0.6,
            surface_m: 0.1,
            n_core: 2,
            settled,
            gave_up: false,
            ms_per_step: ms,
            centres: vec![0.0, 0.0, -0.1, 0.0, 0.0, 0.03, 0.1, 0.0, 0.03, 0.0, 0.2, 2.2],
        }
    }

    /// The page pulls the pour one chunk at a time, sized to the cost,
    /// ignores an old pour's report, stops when paused or settled, and makes
    /// numbered beds from the last report.
    #[test]
    /// The GPU engine (gh:#1250): the same protocol on `PourGpu` requests,
    /// each finished pour lands in the timing table with its engine, and the
    /// native worker pours on the GPU when an adapter exists (skips, saying
    /// so, when none does).
    fn the_gpu_engine_uses_its_own_requests_and_records_its_time() {
        let mut p = PourUi::default();
        p.gpu = true;
        p.n = 3_000.0;
        p.start(0.0);
        assert!(p.running_on_gpu());
        assert!(matches!(p.take_out().as_slice(), [super::super::CoreReq::PourGpu(engine::Request::Start { .. })]));
        p.on_event(engine::Event::Progress(snap(1, 0, 0.0, false)), 0.0);
        assert!(matches!(p.take_out().as_slice(), [super::super::CoreReq::PourGpu(engine::Request::Step { .. })]));
        p.on_event(engine::Event::Progress(snap(1, 20, 40.0, false)), 1.0);
        assert!(matches!(p.take_out().as_slice(), [super::super::CoreReq::PourGpu(engine::Request::Step { steps: GPU_MIN_CHUNK, .. })]), "a GPU chunk is at least {GPU_MIN_CHUNK} steps");
        p.on_event(engine::Event::Progress(snap(1, 20, 5.0, true)), 12.0);
        assert_eq!(p.timings.len(), 1);
        assert!(p.timings[0].gpu && p.timings[0].settled);
        assert!(p.timings[0].line().starts_with("GPU (WebGPU): 4 pebbles, settled in 12 s"), "{}", p.timings[0].line());
        assert!(p.lines(12.0)[0].contains("on the GPU"));

        let mut w = super::super::CoreWorker::default();
        let mut got = Vec::new();
        super::super::serve_gpu_pour(&mut w, engine::Request::Start { id: 3, n: 3_000, seed: 1 }, &mut |e| got.push(e));
        match got.first() {
            Some(super::super::CoreEv::Pour(engine::Event::Progress(s))) => {
                assert_eq!((s.id, s.n), (3, 3_000));
                got.clear();
                super::super::serve_gpu_pour(&mut w, engine::Request::Step { id: 3, steps: 50 }, &mut |e| got.push(e));
                assert!(matches!(got.first(), Some(super::super::CoreEv::Pour(engine::Event::Progress(s))) if s.steps == 50 && s.ms_per_step > 0.0));
            }
            Some(super::super::CoreEv::Pour(engine::Event::Error(m))) => eprintln!("skipping the native GPU pour: {m}"),
            _ => panic!("no reply"),
        }
    }

    #[test]
    fn the_page_pulls_the_pour_and_makes_beds() {
        let mut p = PourUi::auto(1_000_000_000);
        assert!(p.auto && p.shown && !p.started());
        p.start(0.0);
        match p.take_out().as_slice() {
            [super::super::CoreReq::Pour(engine::Request::Start { id: 1, n, .. })] => assert_eq!(*n, max_pebbles()),
            _ => panic!("one start"),
        }
        p.on_event(engine::Event::Progress(snap(1, 0, 0.0, false)), 0.1);
        assert!(matches!(p.take_out().as_slice(), [super::super::CoreReq::Pour(engine::Request::Step { id: 1, steps: 20 })]));
        p.on_event(engine::Event::Progress(snap(1, 20, 50.0, false)), 1.0);
        assert!(matches!(p.take_out().as_slice(), [super::super::CoreReq::Pour(engine::Request::Step { steps: 5, .. })]));
        p.on_event(engine::Event::Progress(snap(9, 20, 50.0, false)), 1.0);
        assert!(p.take_out().is_empty(), "a stale pour's report asks for nothing");
        p.set_paused(true);
        assert!(p.take_out().is_empty());
        p.on_event(engine::Event::Progress(snap(1, 25, 50.0, false)), 2.0);
        assert!(p.take_out().is_empty(), "paused");
        p.set_paused(false);
        assert_eq!(p.take_out().len(), 1);
        p.on_event(engine::Event::Progress(snap(1, 30, 50.0, true)), 3.0);
        // Settled: no further chunk, one measurement asked for.
        assert!(matches!(p.take_out().as_slice(), [super::super::CoreReq::PourDiagnostics { .. }]));
        assert!(p.finished() && !p.running());
        assert_eq!(p.finished_s, Some(3.0));
        assert!(p.lines(9.0)[0].contains("settled"));
        let b1 = p.make_bed().expect("bed");
        let b2 = p.make_bed().expect("bed");
        assert_eq!((b1.version, b2.version), (1, 2));
        assert_eq!(b1.cut, 1, "the pebble at 220 cm is above the 95 % line");
    }
}
