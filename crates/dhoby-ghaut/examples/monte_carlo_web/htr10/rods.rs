//! **The control rods of the `htr10` rung** (gh:#1025; maintainer,
//! 2026-10-23: "a slider which pushes control rods in and out. Build me a
//! curve for keff vs rod position", then "move all rods individually, or
//! sync their movements together", and run any rod's curve in the browser).
//!
//! - [`RodControls`]: the ten insertions the reader sets, synced (one master
//!   slider moves all ten) or one by one. Rod `i` is
//!   `nee_soon::htr10_rmc::core_design::Htr10CoreDesign::rod_insertions[i]`,
//!   in the channel at `reflector_geometry::control_rod_azimuths_deg()[i]`.
//! - [`recorded`] / [`lookup`]: the NATIVE recorded curves (the bank, and
//!   rod [`S3_ROD`] alone) and what they say about the reader's setting. A
//!   recorded value is shown only when the setting lies ON a recorded curve
//!   (all rods equal, or that one rod moving with the other nine withdrawn);
//!   between two computed points it is interpolated and says so. Any other
//!   combination has no recorded result, and the page says that instead of
//!   guessing.
//! - [`LiveSweep`]: k_eff at one setting, or a whole worth curve for the
//!   bank or any one rod, run in this browser on the whole-core worker pool
//!   ([`super::core::pool`]), one position after another: the pool rebuilds
//!   its core with the rods there (`PoolState::set_rods`), then runs at the
//!   reader's statistics. Nothing runs on the UI thread.
//!
//! **S3 is a label, not data.** IAEA-TECDOC-1382 Fig. 4.7 (printed p. 234)
//! names one rod "Control Rod S3", the one whose worth was measured
//! (p. 252), by a leader line in a 416 x 233 px raster; no source in reach
//! places it in a particular channel. Under the model's symmetry (ten
//! channels on a 36° pitch, identical rods) the maintainer accepted one
//! channel standing in for it (2026-10-23): rod 3, the channel at 81°.

use crate::keff::KeffConfig;
use egui::{Color32, Pos2, Rect, Stroke, Vec2};
use petir::mathf::RealMath;
use nee_soon::htr10_rmc::control_rod::{BANK_CURVE_ZT_CM, LOWER_END_INSERTED_CM, LOWER_END_WITHDRAWN_CM, N_CONTROL_RODS};
use nee_soon::htr10_rmc::core_design::Htr10CoreDesign;

/// Rods in the model.
pub const N: usize = N_CONTROL_RODS;

/// The rod labelled S3 (index; "rod 3", the channel at 81°). An assumption
/// under the model's symmetry, not a source's placement (module docs).
pub const S3_ROD: usize = 2;

/// Bed top and the foot of the bed's cylindrical part in TECDOC's `z_T`
/// \[cm\], at the rung's N = 12 (pinned by `the_bed_marks_are_the_models`).
pub const BED_TOP_ZT_CM: f64 = 228.242;
pub const BED_FOOT_ZT_CM: f64 = 351.819;

/// A rod's name on the page.
pub fn rod_label(i: usize) -> String {
    if i == S3_ROD {
        format!("rod {} (\"S3\")", i + 1)
    } else {
        format!("rod {}", i + 1)
    }
}

/// `z_T` \[cm\] of a rod's lower end at insertion `f`.
pub fn zt(f: f64) -> f64 {
    Htr10CoreDesign::lower_end_zt_cm_of(f)
}

/// The insertion of a lower end at `z_T` \[cm\].
pub fn insertion(z_t: f64) -> f64 {
    Htr10CoreDesign::rod_insertion_for_lower_end_zt_cm(z_t)
}

/// What the reader has set.
#[derive(Clone, Debug, PartialEq)]
pub struct RodControls {
    /// One master slider moves every rod.
    pub sync: bool,
    pub master: f64,
    pub rods: [f64; N],
    /// The rod the side view cuts through, and a one-rod curve runs on.
    pub selected: usize,
}

impl Default for RodControls {
    /// Synced, every rod withdrawn (the record's state), "S3" selected.
    fn default() -> Self {
        Self { sync: true, master: 0.0, rods: [0.0; N], selected: S3_ROD }
    }
}

impl RodControls {
    /// Sync on puts every rod at the master's position; off leaves them.
    pub fn set_sync(&mut self, on: bool) {
        self.sync = on;
        if on {
            self.rods = [self.master; N];
        }
    }

    /// The master slider: moves every rod when synced.
    pub fn set_master(&mut self, f: f64) {
        self.master = f.clamp(0.0, 1.0);
        if self.sync {
            self.rods = [self.master; N];
        }
    }

    /// Rod `i`'s slider: synced, it is the master (all follow); otherwise
    /// it moves rod `i` alone. Either way rod `i` becomes the selected rod.
    pub fn set_rod(&mut self, i: usize, f: f64) {
        self.selected = i.min(N - 1);
        if self.sync {
            self.set_master(f);
        } else {
            self.rods[self.selected] = f.clamp(0.0, 1.0);
        }
    }

    /// The rods a page URL asks for (`?rods=` ten insertions in %,
    /// comma-separated, or one value for all; `?sync=0` unsyncs; `?rod=`
    /// the selected rod, 1..10). Anything unreadable leaves the default.
    pub fn from_query(query: &[(String, String)]) -> Self {
        let get = |k: &str| query.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
        let mut c = Self::default();
        if let Some(v) = get("rod").and_then(|v| v.parse::<usize>().ok()).filter(|v| (1..=N).contains(v)) {
            c.selected = v - 1;
        }
        let vals: Vec<f64> = get("rods").map_or(Vec::new(), |v| v.split(',').filter_map(|x| x.trim().parse::<f64>().ok()).map(|x| (x / 100.0).clamp(0.0, 1.0)).collect());
        if vals.len() == 1 {
            c.set_master(vals[0]);
        } else if vals.len() == N {
            c.sync = false;
            c.rods.copy_from_slice(&vals);
            c.master = vals[0];
        }
        if get("sync") == Some("0") {
            c.sync = false;
        }
        c
    }

    /// Withdraw every rod (and the master).
    pub fn withdraw_all(&mut self) {
        self.master = 0.0;
        self.rods = [0.0; N];
    }

    /// What the setting is, for the recorded curves.
    pub fn config(&self) -> Config {
        config_of(&self.rods)
    }
}

/// The kind of setting a set of insertions is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Config {
    /// All ten at one insertion (all withdrawn included).
    Bank(f64),
    /// One rod in, the other nine withdrawn.
    Single { rod: usize, f: f64 },
    /// Anything else.
    Other,
}

/// Classify ten insertions.
pub fn config_of(rods: &[f64; N]) -> Config {
    if rods.iter().all(|f| f.to_bits() == rods[0].to_bits()) {
        return Config::Bank(rods[0]);
    }
    let moved: Vec<usize> = (0..N).filter(|&i| rods[i] != 0.0).collect();
    match moved.as_slice() {
        [i] => Config::Single { rod: *i, f: rods[*i] },
        _ => Config::Other,
    }
}

/// The insertions a family of curves holds at insertion `f`.
pub fn rods_for(family: Family, f: f64) -> [f64; N] {
    match family {
        Family::Bank => [f; N],
        Family::Single(i) => {
            let mut r = [0.0; N];
            r[i] = f;
            r
        }
    }
}

/// Which curve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// All ten rods together.
    Bank,
    /// Rod `i` alone, the others withdrawn.
    Single(usize),
}

impl Family {
    pub fn label(self) -> String {
        match self {
            Family::Bank => "all ten rods together".into(),
            Family::Single(i) => format!("{} alone", rod_label(i)),
        }
    }
    /// The family a setting lies on, if any (all withdrawn lies on every
    /// family: it is reported on the bank's).
    pub fn of(c: Config) -> Option<(Family, f64)> {
        match c {
            Config::Bank(f) => Some((Family::Bank, f)),
            Config::Single { rod, f } => Some((Family::Single(rod), f)),
            Config::Other => None,
        }
    }
}

/// One computed point: rod lower end `z_T` \[cm\], `k ± sigma`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub zt: f64,
    pub k: f64,
    pub sigma: f64,
}

/// A recorded native curve.
#[derive(Clone, Debug)]
pub struct RecordedCurve {
    pub family: Family,
    pub label: &'static str,
    pub points: Vec<Point>,
}

/// The native records, read at build time from the V&V folder
/// (`verification_and_validation/htr10_rod_curve/`): the runner's CSVs.
const BANK_CSVS: [&str; 2] = [
    include_str!("../../../verification_and_validation/htr10_rod_curve/b41_bank_n12.csv"),
    include_str!("../../../verification_and_validation/htr10_rod_curve/curve_bank_n12.csv"),
];
const S3_CSV: &str = include_str!("../../../verification_and_validation/htr10_rod_curve/single_s3_n12.csv");

/// The `zt_cm`, `k`, `sigma` columns of a runner CSV (`nee_soon` example
/// `htr10_rod_curve`), sorted by `z_T`. Lines that do not parse are skipped.
pub fn parse_points(csv: &str) -> Vec<Point> {
    let mut lines = csv.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#'));
    let Some(head) = lines.next() else { return Vec::new() };
    let head: Vec<&str> = head.split(',').collect();
    let col = |n: &str| head.iter().position(|h| h.trim() == n);
    let (Some(cz), Some(ck), Some(cs)) = (col("zt_cm"), col("k"), col("sigma")) else { return Vec::new() };
    let mut v: Vec<Point> = lines
        .filter_map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            let num = |i: usize| f.get(i).and_then(|s| s.trim().parse::<f64>().ok());
            Some(Point { zt: num(cz)?, k: num(ck)?, sigma: num(cs)? })
        })
        .collect();
    v.sort_by(|a, b| a.zt.total_cmp(&b.zt));
    v.dedup_by(|a, b| (a.zt - b.zt).abs() < 1e-9);
    v
}

/// Every recorded curve (empty ones included: their absence is said).
pub fn recorded() -> Vec<RecordedCurve> {
    let mut bank: Vec<Point> = BANK_CSVS.iter().flat_map(|c| parse_points(c)).collect();
    bank.sort_by(|a, b| a.zt.total_cmp(&b.zt));
    bank.dedup_by(|a, b| (a.zt - b.zt).abs() < 1e-9);
    // The S3 curve shares the all-withdrawn point with the bank: one run.
    let mut s3 = parse_points(S3_CSV);
    if let Some(p) = bank.first().filter(|p| (p.zt - LOWER_END_WITHDRAWN_CM).abs() < 1e-9) {
        if !s3.is_empty() && s3.iter().all(|q| (q.zt - p.zt).abs() > 1e-9) {
            s3.insert(0, *p);
        }
    }
    vec![
        RecordedCurve { family: Family::Bank, label: "recorded: all ten rods (native, N = 12)", points: bank },
        RecordedCurve { family: Family::Single(S3_ROD), label: "recorded: rod 3 \"S3\" alone (native, N = 12)", points: s3 },
    ]
}

/// What the records say about a setting.
#[derive(Clone, Debug, PartialEq)]
pub enum Lookup {
    /// A computed point.
    Exact { family: Family, point: Point },
    /// Between two computed points, linear in `z_T`: NOT a computed value.
    Interpolated { family: Family, zt: f64, k: f64, lo: Point, hi: Point },
    /// Nothing recorded for this setting, and why.
    None(String),
}

/// The recorded answer for `rods`, or why there is none.
pub fn lookup(curves: &[RecordedCurve], rods: &[f64; N]) -> Lookup {
    let Some((family, f)) = Family::of(config_of(rods)) else {
        return Lookup::None("No recorded result for this configuration (the rods are neither all at one position nor one rod alone). Run it live below.".into());
    };
    let Some(c) = curves.iter().find(|c| c.family == family) else {
        return Lookup::None(format!("No recorded curve for {}. Run it live below.", family.label()));
    };
    if c.points.is_empty() {
        return Lookup::None(format!("The recorded curve for {} is pending (not yet computed). Run it live below.", family.label()));
    }
    let z = zt(f);
    if let Some(p) = c.points.iter().find(|p| (p.zt - z).abs() < 1e-6) {
        return Lookup::Exact { family, point: *p };
    }
    match c.points.windows(2).find(|w| w[0].zt < z && z < w[1].zt) {
        Some(w) => {
            let t = (z - w[0].zt) / (w[1].zt - w[0].zt);
            Lookup::Interpolated { family, zt: z, k: w[0].k + t * (w[1].k - w[0].k), lo: w[0], hi: w[1] }
        }
        None => Lookup::None(format!("{} has no computed point on both sides of z_T = {z:.1} cm. Run it live below.", c.label)),
    }
}

/// The reactivity worth `1/k_ref - 1/k` of a point against a reference, and
/// its σ (independent runs, first order), in Δk/k.
pub fn worth(k_ref: (f64, f64), k: (f64, f64)) -> (f64, f64) {
    let w = 1.0 / k_ref.0 - 1.0 / k.0;
    let s = ((k_ref.1 / (k_ref.0 * k_ref.0)).r_powi(2) + (k.1 / (k.0 * k.0)).r_powi(2)).sqrt();
    (w, s)
}

// ─── live runs on the pool ────────────────────────────────────────────────

/// One live result.
#[derive(Clone, Debug, PartialEq)]
pub struct LiveResult {
    pub rods: [f64; N],
    pub cfg: KeffConfig,
    pub k: f64,
    pub sem: f64,
    pub wall_s: f64,
}

/// What a sweep is doing.
#[derive(Clone, Debug, PartialEq)]
pub enum SweepStep {
    /// Waiting for the pool (data, a rebuild, or another run).
    Waiting(String),
    /// Running position `i` of `n`.
    Running { i: usize, n: usize },
    Done,
    Failed(String),
}

/// **k_eff at a list of rod settings, run one after another on the pool.**
pub struct LiveSweep {
    /// The curve it belongs to (`None`: a one-off setting).
    pub family: Option<Family>,
    pub targets: Vec<[f64; N]>,
    pub cfg: KeffConfig,
    next: usize,
    /// The run for `targets[next]` has been started (and its epoch).
    started: Option<u32>,
    pub results: Vec<LiveResult>,
    pub failed: Option<String>,
}

impl LiveSweep {
    /// One setting.
    pub fn one(rods: [f64; N], cfg: KeffConfig) -> Self {
        Self { family: None, targets: vec![rods], cfg, next: 0, started: None, results: Vec::new(), failed: None }
    }
    /// A worth curve for `family` at the record's positions
    /// ([`BANK_CURVE_ZT_CM`]: withdrawn, the experiment's start, TECDOC
    /// B42's seven).
    pub fn curve(family: Family, cfg: KeffConfig) -> Self {
        let targets = BANK_CURVE_ZT_CM.iter().map(|&z| rods_for(family, insertion(z))).collect();
        Self { family: Some(family), targets, cfg, next: 0, started: None, results: Vec::new(), failed: None }
    }
    pub fn done(&self) -> bool {
        self.next >= self.targets.len() || self.failed.is_some()
    }
    /// Positions finished, and in all.
    pub fn progress(&self) -> (usize, usize) {
        (self.results.len(), self.targets.len())
    }

    /// Drive the sweep one step (call every frame): rebuild the pool's core
    /// with the next setting's rods, start its run, collect it when done.
    pub fn step(&mut self, s: &mut super::core::pool::PoolState, now_s: f64) -> SweepStep {
        use super::core::pool::PoolPhase;
        if let Some(e) = &self.failed {
            return SweepStep::Failed(e.clone());
        }
        let n = self.targets.len();
        if self.next >= n {
            return SweepStep::Done;
        }
        if let Some(epoch) = self.started {
            let Some(run) = s.run.as_ref().filter(|r| r.epoch == epoch) else {
                self.failed = Some("the run was replaced by another one".into());
                return SweepStep::Failed(self.failed.clone().unwrap_or_default());
            };
            if run.going() {
                return SweepStep::Running { i: self.next, n };
            }
            if let Some(e) = &run.failed {
                self.failed = Some(format!("position {} failed: {e}", self.next + 1));
                return SweepStep::Failed(self.failed.clone().unwrap_or_default());
            }
            if run.stopped {
                self.failed = Some("stopped".into());
                return SweepStep::Failed("stopped".into());
            }
            let (k, sem) = run.gens.last().and_then(|g| g.gen.k_mean).unwrap_or((f64::NAN, f64::NAN));
            let wall_s = run.gens.iter().map(|g| g.wall_s).sum();
            self.results.push(LiveResult { rods: self.targets[self.next], cfg: run.cfg, k, sem, wall_s });
            self.next += 1;
            self.started = None;
            return if self.next >= n { SweepStep::Done } else { SweepStep::Waiting("next position".into()) };
        }
        if s.phase != PoolPhase::Ready {
            return SweepStep::Waiting(match s.phase {
                PoolPhase::Assembling => "the workers are rebuilding the core with these rods".into(),
                _ => "the nuclear data are not ready yet".into(),
            });
        }
        if s.run.as_ref().is_some_and(|r| r.going()) {
            return SweepStep::Waiting("another run is going".into());
        }
        let want = self.targets[self.next];
        if want.map(f64::to_bits) != s.rods.map(f64::to_bits) {
            return match s.set_rods(want) {
                Ok(()) => SweepStep::Waiting("the workers are rebuilding the core with these rods".into()),
                Err(e) => {
                    self.failed = Some(e.clone());
                    SweepStep::Failed(e)
                }
            };
        }
        match s.start_run(self.cfg, now_s) {
            Ok(()) => {
                self.started = s.run.as_ref().map(|r| r.epoch);
                SweepStep::Running { i: self.next, n }
            }
            Err(e) => {
                self.failed = Some(e.clone());
                SweepStep::Failed(e)
            }
        }
    }
}

/// One line for a set of insertions: what the core is built with.
pub fn summary(rods: &[f64; N]) -> String {
    match config_of(rods) {
        Config::Bank(f) if f == 0.0 => "all ten withdrawn (the record's state)".into(),
        Config::Bank(f) => format!("all ten at {:.0} % (lower end z_T {:.1} cm)", 100.0 * f, zt(f)),
        Config::Single { rod, f } => format!("{} alone at {:.0} % (z_T {:.1} cm), nine withdrawn", rod_label(rod), 100.0 * f, zt(f)),
        Config::Other => {
            let n = rods.iter().filter(|f| **f != 0.0).count();
            format!("custom: {n} rods in at different depths")
        }
    }
}

// ─── the view ─────────────────────────────────────────────────────────────

/// The "Control rods" Watch view: a side slice through the selected rod, a
/// top slice at bed mid-height, the ten rods' depths, the curves. The rods,
/// the pool and the live results are the core view's
/// ([`super::core::screen::CoreScreen`]), so both views show one state.
pub struct RodsView {
    pub side: crate::raster::Slicer,
    pub top: crate::raster::Slicer,
    pub curves: Vec<RecordedCurve>,
    /// The rod the side slice is cut through.
    shown_rod: usize,
}

/// The view's slices: one x-z plane through each rod's axis (`y` = the
/// rod's), then the x-y plane at bed mid-height.
pub fn slice_info(base: crate::raster::RasterInfo) -> crate::raster::RasterInfo {
    use crate::raster::{Basis, Preset};
    let az = nee_soon::htr10_rmc::reflector_geometry::control_rod_azimuths_deg();
    let r = nee_soon::htr10_rmc::reflector_geometry::ROD_CHANNEL_CENTRE_RADIUS_CM;
    // The vertical plane through the rod's axis that cuts the bed widest:
    // x-z at the rod's y for a rod nearer the x axis, else y-z at its x.
    let mut ladder: Vec<Preset> = az
        .iter()
        .map(|a| {
            let (sn, cs) = a.to_radians().r_sin_cos();
            let (basis, depth) = if cs.abs() >= sn.abs() { (Basis::Xz, r * sn) } else { (Basis::Yz, r * cs) };
            Preset { label: "side, through the selected rod", basis, centre: [0.0, -15.0], depth, half: 230.0 }
        })
        .collect();
    ladder.push(Preset { label: "top, at bed mid-height", basis: Basis::Xy, centre: [0.0, 0.0], depth: 0.0, half: 125.0 });
    crate::raster::RasterInfo { ladder, start: S3_ROD, param: super::LADDER_N, ..base }
}

impl RodsView {
    pub fn new(base: crate::raster::RasterInfo, selected: usize) -> Self {
        let info = slice_info(base);
        let top = crate::raster::Slicer::new(info.clone(), N, super::LADDER_N);
        Self { side: crate::raster::Slicer::new(info, selected, super::LADDER_N), top, curves: recorded(), shown_rod: selected }
    }

    /// A slice arrived (x-z for the side, x-y for the top).
    pub fn receive(&mut self, ctx: &egui::Context, req: crate::raster::RasterReq, map: Vec<u8>, secs: f64) {
        if req.basis == crate::raster::Basis::Xy {
            self.top.receive(ctx, req, map, secs);
        } else {
            self.side.receive(ctx, req, map, secs);
        }
    }

    /// The side panel: sync, the master and the ten rod sliders, what is
    /// recorded for this setting, and the live runs. GUI code, not reached
    /// by a test (exempt); its decisions are [`RodControls`]', [`lookup`]'s
    /// and [`LiveSweep`]'s, which are.
    pub fn panel(&mut self, ui: &mut egui::Ui, core: &mut super::core::screen::CoreScreen) {
        let before = core.rods.clone();
        ui.strong("Control rods");
        ui.label("Ten B4C rods in the side reflector (IAEA-TECDOC-1382 § 4.1.1.5): 0 % withdrawn (lower end z_T 119.2 cm, the benchmark state), 100 % fully in (394.2 cm).");
        let mut sync = core.rods.sync;
        if ui.checkbox(&mut sync, "Sync all rods").changed() {
            core.rods.set_sync(sync);
        }
        ui.weak(if core.rods.sync {
            "Synced: the master slider moves all ten together and every rod slider follows. Moving any rod slider moves them all."
        } else {
            "Not synced: each slider moves its own rod alone. Turning sync on puts every rod at the master slider's position."
        });
        let mut m = core.rods.master;
        ui.add_enabled_ui(core.rods.sync, |ui| {
            if ui.add(egui::Slider::new(&mut m, 0.0..=1.0).text("all rods").custom_formatter(|v, _| format!("{:.0} %", 100.0 * v))).changed() {
                core.rods.set_master(m);
            }
        });
        let az = nee_soon::htr10_rmc::reflector_geometry::control_rod_azimuths_deg();
        egui::Grid::new("rod-sliders").num_columns(2).spacing([6.0, 4.0]).show(ui, |ui| {
            for i in 0..N {
                let sel = core.rods.selected == i;
                if ui.selectable_label(sel, format!("{} · {:.0}°", rod_label(i), az[i])).clicked() {
                    core.rods.selected = i;
                }
                let mut f = core.rods.rods[i];
                if ui.add(egui::Slider::new(&mut f, 0.0..=1.0).show_value(true).custom_formatter(|v, _| format!("{:.0} %", 100.0 * v))).changed() {
                    core.rods.set_rod(i, f);
                }
                ui.end_row();
            }
        });
        if ui.button("Withdraw all").clicked() {
            core.rods.withdraw_all();
        }
        ui.weak("\"S3\" is a label: TECDOC Fig. 4.7 names the measured rod S3 but no source places it in a channel; rod 3 (81°) stands in for it under the model's symmetry. Tap a rod's name to cut the side view through it.");
        if core.rods != before {
            core.rods_changed_s = dhoby_ghaut::web_demo::platform::now_s();
        }
        ui.separator();
        ui.strong("Recorded k_eff (native runs)");
        ui.label(lookup_text(&lookup(&self.curves, &core.rods.rods)));
        ui.separator();
        ui.strong("Run live in this browser");
        ui.weak("On the whole-core worker pool, at the statistics below: lower than the record's, so wider error bars. The workers rebuild the core with your rods before each run.");
        ui.horizontal(|ui| {
            ui.label("neutrons");
            ui.add(egui::DragValue::new(&mut core.cfg.n_particles).range(200..=crate::keff::MAX_PARTICLES));
            ui.label("skip");
            ui.add(egui::DragValue::new(&mut core.cfg.n_inactive).range(2..=50));
            ui.label("count");
            ui.add(egui::DragValue::new(&mut core.cfg.n_active).range(crate::keff::MIN_ACTIVE..=200));
        });
        match core.pool.as_ref() {
            None => {
                ui.label("The pool is not started: it processes the core's nuclear data first (one to two minutes on a desktop, longer on a phone).");
                if ui.button("Start the worker pool").clicked() {
                    core.start_pool(ui.ctx());
                }
            }
            Some(p) => {
                ui.label(format!("Pool: {} workers, {:?}, {:.0} % of the data; core built with: {}.", p.state.live_workers(), p.state.phase, 100.0 * p.state.progress(), summary(&p.state.rods)));
            }
        }
        let busy = core.sweep.is_some();
        let sel = core.rods.selected;
        ui.add_enabled_ui(core.pool.is_some() && !busy, |ui| {
            if ui.button("Run k_eff at this setting").clicked() {
                core.sweep = Some(LiveSweep::one(core.rods.rods, core.cfg));
            }
            if ui.button("Run a worth curve: all ten rods (9 positions)").clicked() {
                core.sweep = Some(LiveSweep::curve(Family::Bank, core.cfg));
            }
            if ui.button(format!("Run a worth curve: {} alone (9 positions)", rod_label(sel))).clicked() {
                core.sweep = Some(LiveSweep::curve(Family::Single(sel), core.cfg));
            }
        });
        if let Some(sw) = core.sweep.as_ref() {
            let (d, n) = sw.progress();
            let gen = core.pool.as_ref().and_then(|p| p.state.run.as_ref()).map(|r| (r.gens.len(), r.cfg.n_inactive + r.cfg.n_active));
            ui.label(format!("{d} of {n} positions done · {}", core.sweep_status));
            if let Some((g, t)) = gen {
                ui.add(egui::ProgressBar::new((d as f32 + g as f32 / t.max(1) as f32) / n as f32).text(format!("generation {g} of {t}")));
            }
            if ui.button("Stop").clicked() {
                if let Some(p) = core.pool.as_mut() {
                    p.state.stop_run();
                }
            }
        }
        if !core.live_results.is_empty() {
            ui.label("Live results (yours, this page):");
            let w0 = core.live_results.iter().find(|(_, r)| r.rods == [0.0; N]).map(|(_, r)| (r.k, r.sem));
            for (fam, r) in core.live_results.iter().rev().take(20) {
                let mut line = format!("{}: k = {:.4} ± {:.4} ({} × [{} + {}], {:.0} s)", summary(&r.rods), r.k, r.sem, r.cfg.n_particles, r.cfg.n_inactive, r.cfg.n_active, r.wall_s);
                if let (Some(w0), true) = (w0, r.rods != [0.0; N]) {
                    let (w, s) = worth(w0, (r.k, r.sem));
                    line.push_str(&format!("; worth against your all-out run {:.2} ± {:.2} % Δk/k", 100.0 * w, 100.0 * s));
                }
                let _ = fam;
                ui.small(line);
            }
            if ui.small_button("Clear live results").clicked() {
                core.live_results.clear();
            }
        }
    }

    /// The main view: side slice in `rect` (zoomable), and in `aside` the
    /// top slice, the ten rods' depths and the curves. GUI drawing, not
    /// reached by a test (exempt).
    pub fn canvas(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        aside: Option<Rect>,
        painter: &egui::Painter,
        full: &egui::Painter,
        resp: &egui::Response,
        core: &crate::htr10::core::screen::CoreScreen,
        link: Option<&dhoby_ghaut::web_demo::link::Link<crate::engine::Request, crate::engine::Event>>,
        text: f32,
    ) {
        let rods = core.rods.rods;
        if core.rods.selected != self.shown_rod {
            self.shown_rod = core.rods.selected;
            self.side.go(self.shown_rod);
        }
        self.side.view.handle_input(ui, resp);
        self.side.rods = rods;
        self.top.rods = rods;
        if let (Some(req), Some(l)) = (self.side.pump(rect), link) {
            l.send(crate::engine::Request::Raster(req));
        }
        self.side.draw(painter, rect);
        // The selected rod's channel, marked on the side slice (its
        // horizontal coordinate in the slice's plane).
        let az = nee_soon::htr10_rmc::reflector_geometry::control_rod_azimuths_deg()[self.shown_rod];
        let (sn, cs) = az.to_radians().r_sin_cos();
        let rc = nee_soon::htr10_rmc::reflector_geometry::ROD_CHANNEL_CENTRE_RADIUS_CM;
        let x = if self.side.info.ladder[self.shown_rod].basis == crate::raster::Basis::Yz { rc * sn } else { rc * cs };
        let p = self.side.view.to_screen(rect, x, 200.0);
        painter.text(p - Vec2::new(0.0, 9.0), egui::Align2::CENTER_BOTTOM, rod_label(self.shown_rod), egui::FontId::proportional(text), Color32::WHITE);
        painter.add(egui::Shape::convex_polygon(vec![p + Vec2::new(-5.0, -8.0), p + Vec2::new(5.0, -8.0), p], Color32::WHITE, Stroke::NONE));
        painter.text(rect.left_top() + Vec2::new(10.0, 8.0), egui::Align2::LEFT_TOP, format!("Side slice through {} · {}", rod_label(self.shown_rod), summary(&rods)), egui::FontId::proportional(text * 0.9), Color32::from_rgb(210, 216, 226));
        if self.side.busy() || !self.side.settled() || self.top.busy() || !self.top.settled() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(80));
        }
        let Some(a) = aside else { return };
        let (upper, plot) = a.split_top_bottom_at_fraction(0.46);
        let (topr, bank) = upper.split_left_right_at_fraction(0.5);
        let topr = topr.shrink(3.0);
        // Fitted to its box whenever the box changes size (no zoom on it).
        if (self.top.view.fit_scale(topr) - self.top.view.scale).abs() > 1e-9 * self.top.view.scale.max(1e-9) {
            self.top.view.fit(topr);
        }
        if let (Some(req), Some(l)) = (self.top.pump(topr), link) {
            l.send(crate::engine::Request::Raster(req));
        }
        full.rect_filled(topr, 4.0, BG);
        self.top.draw(&full.with_clip_rect(topr), topr);
        let r = nee_soon::htr10_rmc::reflector_geometry::ROD_CHANNEL_CENTRE_RADIUS_CM;
        let azs = nee_soon::htr10_rmc::reflector_geometry::control_rod_azimuths_deg();
        let small = egui::FontId::proportional((text * 0.75).max(8.0));
        for (i, a) in azs.iter().enumerate() {
            let (sn, cs) = a.to_radians().r_sin_cos();
            let q = self.top.view.to_screen(topr, (r + 14.0) * cs, (r + 14.0) * sn);
            if topr.contains(q) {
                full.text(q, egui::Align2::CENTER_CENTER, format!("{}:{:.0}%", i + 1, 100.0 * rods[i]), small.clone(), if i == self.shown_rod { Color32::WHITE } else { GREY });
            }
        }
        full.text(topr.left_top() + Vec2::new(4.0, 2.0), egui::Align2::LEFT_TOP, "top, bed mid-height (B4C shows where a rod reaches it)", small, GREY);
        draw_bank(full, bank.shrink(3.0), &rods, self.shown_rod, text);
        let live: Vec<(Family, Point)> = core
            .live_results
            .iter()
            .filter_map(|(_, r)| Family::of(config_of(&r.rods)).map(|(fam, f)| (fam, Point { zt: zt(f), k: r.k, sigma: r.sem })))
            .filter(|(fam, _)| Family::of(core.rods.config()).is_some_and(|(g, _)| g == *fam) || core.rods.config() == Config::Bank(0.0))
            .collect();
        let here = lookup(&self.curves, &rods);
        draw_curves(full, plot.shrink(3.0), &self.curves, &live, &here, text);
        if let Lookup::None(m) = &here {
            let g = full.layout(m.clone(), egui::FontId::proportional(text * 0.85), Color32::from_rgb(250, 200, 80), plot.width() - 20.0);
            full.galley(Pos2::new(plot.left() + 10.0, plot.top() + 2.0 * text + 8.0), g, Color32::from_rgb(250, 200, 80));
        }
    }
}

/// What [`lookup`] says, as the panel prints it.
pub fn lookup_text(l: &Lookup) -> String {
    match l {
        Lookup::Exact { family, point } => format!("{} at z_T {:.1} cm: k = {:.5} ± {:.5} (a computed point).", family.label(), point.zt, point.k, point.sigma),
        Lookup::Interpolated { family, zt, k, lo, hi } => format!(
            "{} at z_T {zt:.1} cm: k ≈ {k:.5}, INTERPOLATED linearly between the computed points at {:.1} cm ({:.5} ± {:.5}) and {:.1} cm ({:.5} ± {:.5}); not a computed value.",
            family.label(), lo.zt, lo.k, lo.sigma, hi.zt, hi.k, hi.sigma
        ),
        Lookup::None(m) => m.clone(),
    }
}

// ─── drawing (GUI drawing: exempt from the reaching-test rule) ────────────

const BG: Color32 = Color32::from_rgb(18, 21, 28);
const GREY: Color32 = Color32::from_rgb(170, 176, 190);
pub const BANK_COLOUR: Color32 = Color32::from_rgb(120, 170, 255);
pub const S3_COLOUR: Color32 = Color32::from_rgb(255, 170, 90);
pub const LIVE_COLOUR: Color32 = Color32::from_rgb(120, 220, 140);

/// The ten rods as columns: each one's B4C span against the bed's top and
/// foot, the selected rod outlined. GUI drawing, not reached by a test (the
/// test rule's exception); its positions are [`zt`]'s, which is tested.
pub fn draw_bank(painter: &egui::Painter, rect: Rect, rods: &[f64; N], selected: usize, text: f32) {
    painter.rect_filled(rect, 4.0, BG);
    let f = egui::FontId::proportional((text * 0.8).max(8.0));
    let top = rect.top() + text + 6.0;
    let bottom = rect.bottom() - text - 4.0;
    let y = |z_t: f64| top + (bottom - top) * ((z_t - 0.0) / 450.0) as f32;
    painter.text(rect.left_top() + Vec2::new(6.0, 3.0), egui::Align2::LEFT_TOP, "Rods (side, z_T down): B4C in red, bed in green", f.clone(), GREY);
    let left = rect.left() + 34.0;
    let w = (rect.right() - 6.0 - left) / N as f32;
    let bed = Rect::from_min_max(Pos2::new(left, y(BED_TOP_ZT_CM)), Pos2::new(rect.right() - 6.0, y(BED_FOOT_ZT_CM)));
    painter.rect_filled(bed, 0.0, Color32::from_rgb(40, 80, 50));
    painter.text(Pos2::new(rect.left() + 4.0, bed.top()), egui::Align2::LEFT_CENTER, "bed", f.clone(), GREY);
    for (i, &fr) in rods.iter().enumerate() {
        let x0 = left + w * i as f32 + w * 0.25;
        let x1 = left + w * (i as f32 + 0.75);
        let lo = zt(fr);
        let hi = lo - nee_soon::htr10_rmc::control_rod::AXIAL_SECTIONS_CM.iter().sum::<f64>();
        let r = Rect::from_min_max(Pos2::new(x0, y(hi.max(0.0))), Pos2::new(x1, y(lo)));
        painter.rect_filled(r, 1.0, Color32::from_rgb(200, 60, 60));
        if i == selected {
            painter.rect_stroke(r.expand(2.0), 2.0, Stroke::new(1.5, Color32::WHITE), egui::StrokeKind::Outside);
        }
        painter.text(Pos2::new(0.5 * (x0 + x1), bottom + 2.0), egui::Align2::CENTER_TOP, format!("{}", i + 1), f.clone(), if i == selected { Color32::WHITE } else { GREY });
    }
}

/// k against the rod's lower end: recorded curves (markers with ±1σ bars),
/// live points, the marker for the reader's setting. GUI drawing, not
/// reached by a test (exempt); what it shows is [`lookup`]'s, which is.
pub fn draw_curves(painter: &egui::Painter, rect: Rect, curves: &[RecordedCurve], live: &[(Family, Point)], here: &Lookup, text: f32) {
    painter.rect_filled(rect, 4.0, BG);
    let f = egui::FontId::proportional(text);
    let small = egui::FontId::proportional((text * 0.82).max(8.0));
    painter.text(rect.left_top() + Vec2::new(8.0, 4.0), egui::Align2::LEFT_TOP, "k_eff against rod lower end z_T", f, Color32::WHITE);
    let all: Vec<Point> = curves.iter().flat_map(|c| c.points.iter().copied()).chain(live.iter().map(|l| l.1)).collect();
    let plot = Rect::from_min_max(rect.left_top() + Vec2::new(52.0, text + 30.0), rect.right_bottom() - Vec2::new(10.0, 2.2 * text + 10.0));
    let (mut lo, mut hi) = all.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |a, p| (a.0.min(p.k - p.sigma), a.1.max(p.k + p.sigma)));
    if !lo.is_finite() {
        (lo, hi) = (0.8, 1.05);
    }
    let pad = 0.06 * (hi - lo).max(0.01);
    let (lo, hi) = (lo - pad, hi + pad);
    let x = |z: f64| plot.left() + plot.width() * ((z - LOWER_END_WITHDRAWN_CM) / (LOWER_END_INSERTED_CM - LOWER_END_WITHDRAWN_CM)) as f32;
    let y = |k: f64| plot.bottom() - plot.height() * ((k - lo) / (hi - lo)) as f32;
    painter.rect_stroke(plot, 0.0, Stroke::new(1.0, Color32::from_rgb(70, 76, 90)), egui::StrokeKind::Inside);
    for z in [BED_TOP_ZT_CM, BED_FOOT_ZT_CM] {
        painter.line_segment([Pos2::new(x(z), plot.top()), Pos2::new(x(z), plot.bottom())], Stroke::new(1.0, Color32::from_rgb(40, 80, 50)));
    }
    painter.text(Pos2::new(x(BED_TOP_ZT_CM) + 2.0, plot.bottom() - 2.0), egui::Align2::LEFT_BOTTOM, "bed top", small.clone(), GREY);
    painter.text(Pos2::new(x(BED_FOOT_ZT_CM) + 2.0, plot.bottom() - 2.0), egui::Align2::LEFT_BOTTOM, "bed foot", small.clone(), GREY);
    for z in [119.2, 200.0, 300.0, 394.2] {
        painter.text(Pos2::new(x(z), plot.bottom() + 3.0), egui::Align2::CENTER_TOP, format!("{z}"), small.clone(), GREY);
    }
    painter.text(Pos2::new(plot.center().x, plot.bottom() + text + 4.0), egui::Align2::CENTER_TOP, "rod lower end z_T (cm): 119.2 withdrawn → 394.2 fully in", small.clone(), GREY);
    for k in [lo + 0.25 * (hi - lo), lo + 0.75 * (hi - lo)] {
        painter.text(Pos2::new(plot.left() - 4.0, y(k)), egui::Align2::RIGHT_CENTER, format!("{k:.3}"), small.clone(), GREY);
    }
    let mut ly = plot.top() + 4.0;
    for c in curves {
        let col = if c.family == Family::Bank { BANK_COLOUR } else { S3_COLOUR };
        let pts: Vec<Pos2> = c.points.iter().map(|p| Pos2::new(x(p.zt), y(p.k))).collect();
        for w in pts.windows(2) {
            painter.line_segment([w[0], w[1]], Stroke::new(1.0, col.gamma_multiply(0.6)));
        }
        for p in &c.points {
            painter.line_segment([Pos2::new(x(p.zt), y(p.k - p.sigma)), Pos2::new(x(p.zt), y(p.k + p.sigma))], Stroke::new(1.2, col));
            painter.circle_filled(Pos2::new(x(p.zt), y(p.k)), 3.0, col);
        }
        let note = if c.points.is_empty() { " (pending)" } else { "" };
        painter.text(Pos2::new(plot.right() - 4.0, ly), egui::Align2::RIGHT_TOP, format!("{}{note}", c.label), small.clone(), col);
        ly += text + 2.0;
    }
    if !live.is_empty() {
        for (_, p) in live {
            painter.line_segment([Pos2::new(x(p.zt), y(p.k - p.sigma)), Pos2::new(x(p.zt), y(p.k + p.sigma))], Stroke::new(1.2, LIVE_COLOUR));
            painter.rect_filled(Rect::from_center_size(Pos2::new(x(p.zt), y(p.k)), Vec2::splat(6.0)), 0.0, LIVE_COLOUR);
        }
        painter.text(Pos2::new(plot.right() - 4.0, ly), egui::Align2::RIGHT_TOP, "live in this browser (lower statistics)", small.clone(), LIVE_COLOUR);
    }
    match here {
        Lookup::Exact { point, .. } => {
            painter.circle_stroke(Pos2::new(x(point.zt), y(point.k)), 7.0, Stroke::new(2.0, Color32::WHITE));
        }
        Lookup::Interpolated { zt, k, .. } => {
            let p = Pos2::new(x(*zt), y(*k));
            painter.circle_stroke(p, 6.0, Stroke::new(1.5, Color32::WHITE));
            painter.text(p + Vec2::new(8.0, -8.0), egui::Align2::LEFT_BOTTOM, "interpolated", small, Color32::WHITE);
        }
        Lookup::None(_) => {}
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    /// Sync, master and single-rod sliders do what the panel says.
    #[test]
    fn the_sliders_sync_and_unsync() {
        let mut c = RodControls::default();
        assert_eq!(c.config(), Config::Bank(0.0));
        c.set_master(0.4);
        assert_eq!(c.rods, [0.4; N]);
        c.set_rod(7, 0.9); // synced: every rod follows
        assert_eq!(c.rods, [0.9; N]);
        assert_eq!(c.selected, 7);
        c.set_sync(false);
        c.set_rod(3, 0.1);
        assert_eq!(c.rods[3], 0.1);
        assert_eq!(c.rods[2], 0.9);
        assert_eq!(c.config(), Config::Other);
        c.set_sync(true); // back on: every rod at the master's position
        assert_eq!(c.rods, [0.9; N]);
        c.set_sync(false);
        c.withdraw_all();
        c.set_rod(S3_ROD, 2.0);
        assert_eq!(c.config(), Config::Single { rod: S3_ROD, f: 1.0 });
    }

    /// A URL can carry the rods.
    #[test]
    fn the_rods_come_from_the_url() {
        let q = |s: &[(&str, &str)]| s.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect::<Vec<_>>();
        assert_eq!(RodControls::from_query(&[]), RodControls::default());
        let c = RodControls::from_query(&q(&[("rods", "50")]));
        assert_eq!(c.rods, [0.5; N]);
        assert!(c.sync);
        let c = RodControls::from_query(&q(&[("rods", "0,0,100,0,0,0,0,0,0,0"), ("rod", "3")]));
        assert_eq!(c.config(), Config::Single { rod: 2, f: 1.0 });
        assert!(!c.sync);
        assert_eq!(c.selected, 2);
        assert!(!RodControls::from_query(&q(&[("sync", "0")])).sync);
        assert_eq!(RodControls::from_query(&q(&[("rods", "1,2")])).rods, [0.0; N]);
    }

    /// The records are read; a setting on a curve gets the recorded value
    /// (exact, or interpolated and said so), any other gets none.
    #[test]
    fn a_recorded_value_only_on_a_recorded_curve() {
        let pts = parse_points("layers,zt_cm,insertion,k,sigma\n12,394.2,1,0.85,0.002\n12,119.2,0,0.99,0.002\nbad,line\n");
        assert_eq!(pts.len(), 2);
        assert_eq!(pts[0].zt, 119.2);
        let curves = vec![
            RecordedCurve { family: Family::Bank, label: "b", points: pts.clone() },
            RecordedCurve { family: Family::Single(S3_ROD), label: "s", points: Vec::new() },
        ];
        assert_eq!(lookup(&curves, &[0.0; N]), Lookup::Exact { family: Family::Bank, point: pts[0] });
        match lookup(&curves, &[0.5; N]) {
            Lookup::Interpolated { k, zt, .. } => {
                assert!((zt - 256.7).abs() < 1e-9);
                assert!((k - 0.92).abs() < 1e-12);
            }
            o => panic!("{o:?}"),
        }
        let s3 = rods_for(Family::Single(S3_ROD), 0.5);
        assert!(matches!(lookup(&curves, &s3), Lookup::None(m) if m.contains("pending")));
        let mut other = [0.0; N];
        other[1] = 0.3;
        other[4] = 0.6;
        assert!(matches!(lookup(&curves, &other), Lookup::None(m) if m.contains("No recorded result")));
        // The committed records parse (each file may still be empty).
        let r = recorded();
        assert_eq!(r.len(), 2);
        assert!(r.iter().all(|c| c.points.windows(2).all(|w| w[0].zt < w[1].zt)));
        let (w, s) = worth((1.0, 0.001), (0.9, 0.001));
        assert!((w - (1.0 - 1.0 / 0.9)).abs() < 1e-12 && s > 0.0);
    }

    /// The page's one-line summaries and the lookup text say what they are.
    #[test]
    fn the_summaries_say_what_is_built() {
        assert!(summary(&[0.0; N]).contains("withdrawn"));
        assert!(summary(&[0.5; N]).contains("256.7"));
        assert!(summary(&rods_for(Family::Single(S3_ROD), 1.0)).contains("rod 3 (\"S3\") alone"));
        let mut o = [0.0; N];
        o[0] = 0.2;
        o[5] = 0.4;
        assert!(summary(&o).starts_with("custom: 2 rods"));
        let p = Point { zt: 119.2, k: 1.0, sigma: 0.001 };
        let q = Point { zt: 171.2, k: 0.99, sigma: 0.001 };
        assert!(lookup_text(&Lookup::Interpolated { family: Family::Bank, zt: 150.0, k: 0.995, lo: p, hi: q }).contains("INTERPOLATED"));
        assert!(lookup_text(&Lookup::Exact { family: Family::Bank, point: p }).contains("computed point"));
        let info = slice_info({ use crate::rungs::McRung; crate::htr10::Htr10::raster_info().expect("htr10 slices") });
        assert_eq!(info.ladder.len(), N + 1);
        assert_eq!(info.ladder[N].basis, crate::raster::Basis::Xy);
        // Rod 3 at 81° is nearer the y axis: a y-z plane at its x. Rod 1 at
        // 9°: an x-z plane at its y. Both planes contain the rod's axis.
        assert_eq!(info.ladder[S3_ROD].basis, crate::raster::Basis::Yz);
        assert!((info.ladder[S3_ROD].depth - 102.1 * 81f64.to_radians().r_sin_cos().1).abs() < 1e-9);
        assert_eq!(info.ladder[0].basis, crate::raster::Basis::Xz);
        assert!((info.ladder[0].depth - 102.1 * 9f64.to_radians().r_sin_cos().0).abs() < 1e-9);
    }

    /// The bed marks are the built N = 12 core's, and S3 is rod 3 at 81°.
    #[test]
    fn the_bed_marks_are_the_models() {
        let c = super::super::model::core(12);
        assert!((c.refl_top - c.bed_half_height - BED_TOP_ZT_CM).abs() < 1e-3, "{}", c.refl_top - c.bed_half_height);
        assert!((c.refl_top + c.bed_half_height - BED_FOOT_ZT_CM).abs() < 1e-3, "{}", c.refl_top + c.bed_half_height);
        let az = nee_soon::htr10_rmc::reflector_geometry::control_rod_azimuths_deg();
        assert!((az[S3_ROD] - 81.0).abs() < 1e-9);
        assert_eq!(rod_label(S3_ROD), "rod 3 (\"S3\")");
    }

    /// A sweep asks the pool to rebuild with each setting's rods before it
    /// starts that setting's run, and stops when its run is stopped.
    #[test]
    fn a_live_sweep_rebuilds_then_runs() {
        use super::super::core::pool::{PoolPhase, PoolState};
        use super::super::core::{AssemblyReport, CoreEv};
        let rep = AssemblyReport { layers: 12, tapes_s: 0.0, nuclides_s: 0.0, geometry_s: 0.0, majorant_s: 0.0, memory_mb: 1.0 };
        let mut s = PoolState::new(2, Vec::new(), Vec::new(), 12, 0.0);
        let cfg = KeffConfig { n_particles: 100, n_inactive: 1, n_active: 2, seed: 1, point_source: false, want_sites: false };
        let mut sw = LiveSweep::curve(Family::Single(S3_ROD), cfg);
        assert_eq!(sw.targets.len(), 9);
        assert_eq!(sw.targets[0], [0.0; N]);
        assert!((sw.targets[8][S3_ROD] - 1.0).abs() < 1e-12 && sw.targets[8][0] == 0.0);
        assert!(matches!(sw.step(&mut s, 0.0), SweepStep::Waiting(_)), "data not ready");
        for w in 0..2 {
            s.on_event(w, CoreEv::Assembled(rep), 0.0);
        }
        assert_eq!(s.phase, PoolPhase::Ready);
        // Withdrawn is what the pool holds: the run starts at once.
        assert_eq!(sw.step(&mut s, 1.0), SweepStep::Running { i: 0, n: 9 });
        assert!(s.run.as_ref().is_some_and(|r| r.going()));
        s.stop_run();
        assert!(matches!(sw.step(&mut s, 2.0), SweepStep::Failed(m) if m == "stopped"));
        // A one-off at another setting rebuilds first.
        let mut one = LiveSweep::one(rods_for(Family::Bank, 0.5), cfg);
        assert!(matches!(one.step(&mut s, 3.0), SweepStep::Waiting(_)));
        assert_eq!(s.phase, PoolPhase::Assembling);
        assert_eq!(s.rods, [0.5; N]);
        for w in 0..2 {
            s.on_event(w, CoreEv::Assembled(rep), 4.0);
        }
        assert_eq!(one.step(&mut s, 5.0), SweepStep::Running { i: 0, n: 1 });
        assert_eq!(one.progress(), (0, 1));
        assert!(!one.done());
    }
}
