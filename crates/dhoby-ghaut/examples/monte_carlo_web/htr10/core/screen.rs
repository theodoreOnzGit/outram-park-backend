//! **The core view** of the `htr10` rung (gh:#786): neutrons flying through
//! the whole HTR-10 model, over a live x-z slice of it, and the live k_eff
//! beside the record, at whatever statistics the reader sets (~~low-statistics~~:
//! the recorded settings are one tap away, gh:#876 item 5).
//!
//! **One tab for the core since 2026-10-25** (maintainer: "change the number
//! of stacks and control rod position under the same tab"): the side panel
//! sets the bed's N (10–20, or the DEM bed) and the ten control rods
//! ([`CoreScreen::panel`]), then runs k_eff; the bottom dock shows that run's
//! k and Shannon entropy with the shared plots of `crate::keffplot`, the same
//! as every rung's Run mode ([`CoreScreen::keff_body`],
//! [`CoreScreen::keff_buttons`]); the right dock holds the top and rod views
//! and the reference graphs (`crate::htr10::rods::RodsView`). A run starts
//! on the reader's rods: if the workers' core holds other rods, Start
//! rebuilds it first ([`CoreScreen::start_run_here`]).
//!
//! It opens on **recorded** tracks (baked natively by [`super::bake`] from the
//! same model, code and seed, and labelled so), which play at once. "Run live
//! in this browser" starts the worker pool ([`super::pool`]); once the data
//! are processed, the tracks come from the run itself: the first
//! [`super::TRACED_PER_GENERATION`] histories of every generation.

use super::pool::{CorePool, Note, PoolPhase, PoolState, RestartKind, RunOrigin, RunState};
use super::{DEFAULT_RUN, RECORD_RUN, RECORD_SEED};
use crate::anim::{animated_speed, draw_track, energy_bar, fmt_speed, legend_markers, Anim};
use crate::history::History;
use crate::keff::{clamp_to_caps, eta_line, plan_line, KeffConfig, MAX_GENERATIONS, MAX_PARTICLES, MIN_ACTIVE};
use crate::raster::{Preset, RasterInfo, Slicer};
use dhoby_ghaut::web_demo::platform::now_s;
use dhoby_ghaut::web_demo::pool::{plan, Device, Plan};
use egui::{Color32, Pos2, Rect, Vec2};
use petir::mathf::RealMath;
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::physics::track_output::{Track, TrackEvent, TrackState};
use std::collections::VecDeque;
use nee_soon::htr10_rmc::zones::{max_temperature, min_temperature, triso_limit, Htr10Zone, ZoneTemperatures};
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::degree_celsius;

/// A core run's time left on this device, from its generations' measured
/// wall times (the pool's, so every worker counts); `None` before the first.
pub fn run_eta(run: &RunState) -> Option<String> {
    let secs: f64 = run.gens.iter().map(|g| g.wall_s).sum();
    eta_line(run.cfg.n_particles, run.cfg.n_inactive + run.cfg.n_active, run.gens.len(), secs)
}

/// The 2026-10-07 record (all 22 points at 10 000 × [5 + 135] on the bounded
/// majorant), read at build time.
const RECORD_CSV: &str = include_str!("../../../../../nee_soon/verification_and_validation/htr10_seker_2026_10_07_10k/results_table.csv");

/// The recorded tracks the view opens on (`--bake-htr10-core`).
const RECORDED_TRACKS: &[u8] = include_bytes!("../data/core_tracks.bin.z");

/// Memory one worker holds once its model is built, MB: measured in headless
/// Chromium, 2026-10-07 (`verification_and_validation/htr10_full_core_web/`).
/// Sizes the pool.
pub const MEASURED_WORKER_MB: f64 = 546.0;

/// **The native random-bed k_eff.** ~~A slot, not a value: the run (#787)
/// is in progress.~~ **Filled 2026-10-08:** the DEM bed cut to 16 681 core
/// pebbles, 10 000 × [5 + 135], the lattice record's settings
/// (`nee_soon/verification_and_validation/htr10_dem_bed_keff_2026_10_08/`).
/// Taken from [`crate::beds::RANDOM_K`], which a test pins to that record, so
/// this view and the beds view quote the same numbers.
pub const DEM_NATIVE_RECORD: Option<(f64, f64, &str)> = Some((
    crate::beds::RANDOM_K.k,
    crate::beds::RANDOM_K.sigma,
    "htr10_dem_bed_keff_2026_10_08, recorded 2026-10-08, one pour",
));

/// What the page says about the random bed's live k, wherever it shows it.
pub const DEM_CAVEAT: &str = "Random bed: at the default 1000 neutrons per generation σ is about 1000 pcm, so it cannot resolve a lattice-against-random difference (any likely one is a few hundred pcm). One pour, one cut; pour-to-pour scatter is not measured.";

/// The native random-bed record as the page prints it.
pub fn dem_record_line() -> String {
    match DEM_NATIVE_RECORD {
        Some((k, s, src)) => {
            let (d, ds) = crate::beds::random_minus_lattice_pcm();
            format!(
                "Native random-bed k (10 000 × [5 + 135]): {k:.6} ± {s:.6} ({src}); random − lattice {d:+.0} ± {ds:.0} pcm"
            )
        }
        None => "Native random-bed k (10 000 × [5 + 135]): PENDING, the run is in progress (#787); no value yet.".into(),
    }
}

/// One N's row of the record: `(k, σ, RMC at equal ball count)`.
pub fn record_row(n: usize) -> Option<(f64, f64, f64)> {
    let mut rows = RECORD_CSV.lines();
    let head: Vec<&str> = rows.next()?.split(',').collect();
    let col = |name: &str| head.iter().position(|h| *h == name);
    let (cl, cn, ck, cs, cr) = (
        col("library")?,
        col("N")?,
        col("k")?,
        col("sigma")?,
        col("rmc")?,
    );
    rows.find_map(|l| {
        let f: Vec<&str> = l.split(',').collect();
        let num = |i: usize| f.get(i).and_then(|v| v.trim().parse::<f64>().ok());
        (f.get(cl) == Some(&"VIII.0") && num(cn)? as usize == n).then_some(())?;
        Some((num(ck)?, num(cs)?, num(cr)?))
    })
}

/// A track in the core's frame as the view draws it: `(x, y, z)` to
/// `(x, z, y)`, so the drawing's x-y is the slice's x-z.
pub fn to_view(index: u64, t: Track) -> History {
    let flip = |p: Position| Position::new(p.x, p.z, p.y);
    let states: Vec<TrackState> = t
        .states
        .into_iter()
        .map(|s| TrackState {
            r: flip(s.r),
            u: Direction {
                u: s.u.u,
                v: s.u.w,
                w: s.u.v,
            },
            ..s
        })
        .collect();
    let (birth, e) = states
        .first()
        .map_or((Position::ZERO, 0.0), |s| (s.r, s.energy));
    History::from_track(
        index,
        birth,
        e,
        false,
        Track {
            states,
            dropped_states: t.dropped_states,
        },
    )
}

/// Tracks as zlib-compressed little-endian `f64`s: `[n, then per track
/// (generation, index, dropped, n_states, states × 12)]`.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))] // the native bake writes
pub fn encode_tracks(tracks: &[(usize, usize, Track)]) -> Vec<u8> {
    let mut v = vec![tracks.len() as f64];
    for (g, i, t) in tracks {
        v.extend([
            *g as f64,
            *i as f64,
            t.dropped_states as f64,
            t.states.len() as f64,
        ]);
        for s in &t.states {
            v.extend([
                s.r.x, s.r.y, s.r.z, s.u.u, s.u.v, s.u.w, s.energy, s.time, s.weight,
            ]);
            v.push(if s.cell == usize::MAX {
                -1.0
            } else {
                s.cell as f64
            });
            v.push(s.material.map_or(-1.0, |m| m as f64));
            v.push(event_code(s.event));
        }
    }
    let bytes: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
    crate::tapes::compress(&bytes)
}

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
fn event_code(e: TrackEvent) -> f64 {
    [
        TrackEvent::Born,
        TrackEvent::SurfaceCrossing,
        TrackEvent::Scatter,
        TrackEvent::Fission,
        TrackEvent::Absorption,
        TrackEvent::Rouletted,
        TrackEvent::Leak,
        TrackEvent::Lost,
    ]
    .iter()
    .position(|&x| x == e)
    .unwrap_or(7) as f64
}

fn event_from(c: f64) -> TrackEvent {
    [
        TrackEvent::Born,
        TrackEvent::SurfaceCrossing,
        TrackEvent::Scatter,
        TrackEvent::Fission,
        TrackEvent::Absorption,
        TrackEvent::Rouletted,
        TrackEvent::Leak,
        TrackEvent::Lost,
    ]
    .get(c as usize)
    .copied()
    .unwrap_or(TrackEvent::Lost)
}

/// The inverse of [`encode_tracks`].
pub fn decode_tracks(z: &[u8]) -> Result<Vec<(usize, usize, Track)>, String> {
    let b = crate::tapes::decompress(z)?;
    let v: Vec<f64> = b
        .chunks_exact(8)
        .map(|c| f64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]))
        .collect();
    let mut at = 1usize;
    let n = *v.first().ok_or("empty track file")? as usize;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let h = v.get(at..at + 4).ok_or("track file too short")?;
        let (g, i, dropped, ns) = (h[0] as usize, h[1] as usize, h[2] as usize, h[3] as usize);
        at += 4;
        let w = v.get(at..at + 12 * ns).ok_or("track file too short")?;
        at += 12 * ns;
        let states = w
            .chunks_exact(12)
            .map(|c| TrackState {
                r: Position::new(c[0], c[1], c[2]),
                u: Direction {
                    u: c[3],
                    v: c[4],
                    w: c[5],
                },
                energy: c[6],
                time: c[7],
                weight: c[8],
                cell: if c[9] < 0.0 {
                    usize::MAX
                } else {
                    c[9] as usize
                },
                material: (c[10] >= 0.0).then_some(c[10] as usize),
                event: event_from(c[11]),
            })
            .collect();
        out.push((
            g,
            i,
            Track {
                states,
                dropped_states: dropped,
            },
        ));
    }
    if at != v.len() {
        return Err("track file: words left over".into());
    }
    Ok(out)
}

/// `?workers=N` as the page was opened (the app rewrites the query once it
/// starts, so this is read at start-up: [`remember_query`]).
static FORCED_WORKERS: std::sync::OnceLock<Option<usize>> = std::sync::OnceLock::new();

/// `?bed=dem` as the page was opened: start on the DEM random bed.
static FORCED_DEM: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
static FORCED_POUR: std::sync::OnceLock<Option<usize>> = std::sync::OnceLock::new();
/// `&engine=gpu` with `?pour=N`: pour on the GPU (gh:#1250).
static FORCED_POUR_GPU: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// `?pour=N` (gh:#1244): pour N pebbles on arrival and use the bed once it
/// settles; the pool waits for it. How an unattended browser check drives
/// the pour end to end.
pub fn forced_pour_gpu() -> bool {
    *FORCED_POUR_GPU.get_or_init(|| {
        let q = dhoby_ghaut::web_demo::platform::query_pairs();
        dhoby_ghaut::web_demo::platform::query_value(&q, "engine") == Some("gpu")
    })
}

/// `?pour=N`: see [`forced_pour_gpu`] for `&engine=gpu`.
pub fn forced_pour() -> Option<usize> {
    *FORCED_POUR.get_or_init(|| {
        let q = dhoby_ghaut::web_demo::platform::query_pairs();
        dhoby_ghaut::web_demo::platform::query_value(&q, "pour").and_then(|v| v.parse().ok())
    })
}

/// Whether the page was opened with `?bed=dem`.
pub fn forced_dem() -> bool {
    *FORCED_DEM.get_or_init(|| {
        let q = dhoby_ghaut::web_demo::platform::query_pairs();
        dhoby_ghaut::web_demo::platform::query_value(&q, "bed") == Some("dem")
    })
}

/// Read `?workers=N` (and `?bed=dem`) before the app rewrites the URL. Call
/// once at start-up.
pub fn remember_query() -> Option<usize> {
    let _ = forced_dem();
    let _ = forced_pour();
    let _ = forced_pour_gpu();
    *FORCED_WORKERS.get_or_init(|| {
        let q = dhoby_ghaut::web_demo::platform::query_pairs();
        dhoby_ghaut::web_demo::platform::query_value(&q, "workers").and_then(|v| v.parse().ok())
    })
}

/// The pool size asked for in the URL, if any.
pub fn forced_workers() -> Option<usize> {
    remember_query()
}

/// The view's state.
pub struct CoreScreen {
    pub slicer: Slicer,
    pub pool: Option<CorePool>,
    pub plan: Plan,
    forced: Option<usize>,
    pub layers: f64,
    pub cfg: KeffConfig,
    recorded: Vec<History>,
    next_recorded: usize,
    /// Tracks from the live run (else the recorded ones).
    pub live: bool,
    queue: VecDeque<History>,
    current: Option<Anim>,
    past: VecDeque<History>,
    pub playing: bool,
    pub speed: f64,
    autostart: bool,
    pub error: Option<String>,
    /// The control rods the reader has set (gh:#1025; the "Control rods"
    /// view's sliders). The pool's core follows them between runs.
    pub rods: crate::htr10::rods::RodControls,
    /// When the rods were last changed (the pool rebuilds once they rest).
    pub rods_changed_s: f64,
    /// A live run or worth curve over rod settings, in progress.
    pub sweep: Option<crate::htr10::rods::LiveSweep>,
    /// What the sweep is doing, for the page.
    pub sweep_status: String,
    /// Every finished live result, with the curve it belongs to.
    pub live_results: Vec<(Option<crate::htr10::rods::Family>, crate::htr10::rods::LiveResult)>,
    /// The eight zone temperatures the pool processes and builds at
    /// (gh:#1142): the record's 27 °C until the reader applies others.
    pub temps: ZoneTemperatures,
    /// The sliders' values, °C, in `Htr10Zone::ALL` order: applied with
    /// "Apply", which restarts the pool at them.
    pub temps_c: [f64; 8],
    /// Whether the sliders move together.
    pub temps_linked: bool,
    /// The reader's own DEM pour (gh:#1244).
    pub pour: super::pour::PourUi,
}

/// `t` in °C.
pub fn celsius(t: ThermodynamicTemperature) -> f64 {
    t.get::<degree_celsius>()
}

/// **The temperatures to apply**, if the sliders (°C) differ from the
/// applied ones and every value is in range; `Err` names a value out of
/// range (gh:#1142).
pub fn temperatures_to_apply(applied: &ZoneTemperatures, sliders_c: &[f64; 8]) -> Result<Option<ZoneTemperatures>, String> {
    let mut want = *applied;
    for (z, &c) in Htr10Zone::ALL.into_iter().zip(sliders_c) {
        // A slider that reads the applied value keeps it to the bit (27 °C
        // round-tripped through °C is not always 300.15 K exactly, and a
        // last-bit difference is another processing key).
        if (c - celsius(applied.get(z))).abs() >= 1e-9 {
            want.set(z, ThermodynamicTemperature::new::<degree_celsius>(c));
        }
    }
    want.check()?;
    let same = Htr10Zone::ALL.into_iter().all(|z| (celsius(want.get(z)) - celsius(applied.get(z))).abs() < 1e-9);
    Ok((!same).then_some(want))
}

/// **Zone temperatures from a page URL** (gh:#1142): `?temps=fuel:900,coolant:500`
/// (°C; the zone keys `fuel`, `moderator`, `coolant`, `top`, `side`,
/// `bottom`, `rods`, `bricks`, and `all` for every zone, applied first).
/// A setting that does not parse or is out of range leaves the record's
/// temperatures.
pub fn temperatures_from_query(query: &[(String, String)]) -> ZoneTemperatures {
    let rec = ZoneTemperatures::record();
    let Some(v) = query.iter().find(|(k, _)| k == "temps").map(|(_, v)| v.as_str()) else {
        return rec;
    };
    let mut t = rec;
    let mut items: Vec<(&str, &str)> = Vec::new();
    for item in v.split(',') {
        let Some(kv) = item.trim().split_once(':') else { return rec };
        items.push(kv);
    }
    items.sort_by_key(|(k, _)| *k != "all");
    for (k, c) in items {
        let Ok(c) = c.trim().parse::<f64>() else { return rec };
        let at = ThermodynamicTemperature::new::<degree_celsius>(c);
        match (k.trim(), Htr10Zone::from_key(k.trim())) {
            ("all", _) => t = ZoneTemperatures::uniform(at),
            (_, Some(z)) => t.set(z, at),
            _ => return rec,
        }
    }
    if t.check().is_ok() { t } else { rec }
}

/// The warning shown above 1600 °C (the TRISO fuel limit), or `None`.
pub fn triso_warning(sliders_c: &[f64; 8]) -> Option<String> {
    let lim = celsius(triso_limit());
    let hot: Vec<&str> = Htr10Zone::ALL
        .into_iter()
        .zip(sliders_c)
        .filter(|(_, &c)| c > lim)
        .map(|(z, _)| z.label())
        .collect();
    (!hot.is_empty()).then(|| {
        format!(
            "⚠ Above {lim:.0} °C, the TRISO fuel limit: {}. The data are processed there all the same (up to {:.0} °C); the model has no fuel failure.",
            hot.join(", "),
            celsius(max_temperature())
        )
    })
}

/// How long the rods must rest before the pool rebuilds its core with them,
/// s (a rebuild costs every worker a geometry build).
pub const RODS_SETTLE_S: f64 = 1.5;

/// Whether the pool should rebuild with the reader's rods now: the data
/// are ready, no run or sweep is going, the rods differ from the pool's and
/// have rested [`RODS_SETTLE_S`].
pub fn rods_resync_due(s: &PoolState, want: &[f64; 10], sweeping: bool, changed_s: f64, now: f64) -> bool {
    !sweeping
        && s.phase == PoolPhase::Ready
        && !s.run.as_ref().is_some_and(RunState::going)
        && want.map(f64::to_bits) != s.rods.map(f64::to_bits)
        && now - changed_s >= RODS_SETTLE_S
}

impl CoreScreen {
    /// `info` is the rung's raster info (its first ladder step, the side view
    /// of the core, is the background); `speed` the rung's default.
    pub fn new(info: RasterInfo, speed: f64, autostart: bool) -> Self {
        let forced = forced_workers();
        // The side view, fitted to the whole model height (610 cm) rather than the ladder's 400 cm window.
        let side = Preset {
            half: 310.0,
            ..info.ladder[0]
        };
        let info = RasterInfo {
            ladder: vec![side],
            start: 0,
            param: super::super::LADDER_N,
            ..info
        };
        let recorded = decode_tracks(RECORDED_TRACKS)
            .map(|v| {
                v.into_iter()
                    .map(|(g, i, t)| to_view((g * 100_000 + i) as u64, t))
                    .collect()
            })
            .unwrap_or_default();
        Self {
            slicer: Slicer::new(info, 0, super::super::LADDER_N),
            pool: None,
            plan: plan(Device::probe(), MEASURED_WORKER_MB, forced),
            forced,
            layers: if forced_dem() { super::random_bed::DEM_BED as f64 } else { super::super::LADDER_N },
            cfg: DEFAULT_RUN,
            recorded,
            next_recorded: 0,
            live: false,
            queue: VecDeque::new(),
            current: None,
            past: VecDeque::new(),
            playing: true,
            speed,
            autostart,
            error: None,
            rods: Default::default(),
            rods_changed_s: 0.0,
            sweep: None,
            sweep_status: String::new(),
            live_results: Vec::new(),
            temps: ZoneTemperatures::record(),
            temps_c: Htr10Zone::ALL.map(|z| celsius(ZoneTemperatures::record().get(z))),
            temps_linked: false,
            pour: match forced_pour() {
                Some(n) => {
                    let mut p = super::pour::PourUi::auto(n);
                    p.gpu = forced_pour_gpu();
                    p
                }
                None => super::pour::PourUi::default(),
            },
        }
    }

    /// The page title: what the view shows and where the pool is (also what
    /// an unattended browser check reads).
    pub fn title(&self) -> String {
        let mut t = String::from(if self.live {
            "whole core · live tracks"
        } else {
            "whole core · recorded tracks"
        });
        if let Some(p) = &self.pool {
            for (l, _) in status_lines(p) {
                t.push_str(" · ");
                t.push_str(&l);
            }
        }
        if let Some(e) = &self.error {
            t.push_str(&format!(" · FAILED · {e}"));
        }
        t
    }

    /// The rods the pool's core is built with (the reader's, before a pool
    /// exists): what the core view draws and runs.
    pub fn pool_rods(&self) -> [f64; 10] {
        self.pool.as_ref().map_or(self.rods.rods, |p| p.state.rods)
    }

    /// Start the workers.
    pub fn start_pool(&mut self, ctx: &egui::Context) {
        if self.pool.is_none() {
            match CorePool::start(ctx, self.plan.clone(), self.layers as usize, &self.temps) {
                Ok(p) => self.pool = Some(p),
                Err(e) => self.error = Some(e),
            }
        }
    }

    /// No worker is left (gh:#817): stop the old pool's workers (a dropped
    /// link would leave a Web Worker running) and start a new pool.
    pub fn reprocess(&mut self, ctx: &egui::Context) {
        if let Some(p) = self.pool.take() {
            log::warn!("htr10 core: reprocessing: stopping the old pool of {} workers", p.state.workers);
            p.terminate();
        }
        self.live = false;
        self.queue.clear();
        self.start_pool(ctx);
    }

    /// Each frame: the pool's events, the next track, autostart.
    pub fn pump(&mut self, ctx: &egui::Context) {
        // `?pour=N`: pour, then use the bed once it settles; the pool waits.
        if self.pour.auto {
            if !self.pour.started() {
                self.pour.start(dhoby_ghaut::web_demo::platform::now_s());
            } else if self.pour.finished() && self.pour.bed.is_none() {
                self.use_pour();
            }
        }
        let waiting_for_pour = self.pour.auto && self.pour.bed.is_none();
        if self.autostart && self.pool.is_none() && !waiting_for_pour {
            self.start_pool(ctx);
        }
        let mut autostart_now = false;
        if let Some(p) = self.pool.as_mut() {
            for n in p.pump() {
                match n {
                    Note::Generation(g) => {
                        if !self.live {
                            self.live = true;
                            self.queue.clear();
                        }
                        for (i, t) in g.gen.tracks.iter().cloned() {
                            self.queue
                                .push_back(to_view((g.gen.index * 100_000 + i) as u64, t));
                        }
                        while self.queue.len() > 24 {
                            self.queue.pop_front();
                        }
                    }
                    Note::RunDone => {}
                }
            }
            // Autostart goes through the same door as the Start button, so
            // it too runs on the reader's rods (a URL may set them).
            autostart_now = self.autostart && p.state.phase == PoolPhase::Ready && p.state.run.is_none() && self.sweep.is_none();
            // Rods (gh:#1025): a sweep drives the pool itself; otherwise the
            // pool's core follows the reader's rods once they rest.
            if let Some(sw) = self.sweep.as_mut() {
                use crate::htr10::rods::SweepStep;
                let step = sw.step(&mut p.state, now_s());
                self.sweep_status = match &step {
                    SweepStep::Waiting(w) => format!("waiting: {w}"),
                    SweepStep::Running { i, n } => format!("running position {} of {n}", i + 1),
                    SweepStep::Done => "done".into(),
                    SweepStep::Failed(e) => format!("stopped: {e}"),
                };
                if sw.done() {
                    let fam = sw.family;
                    for r in sw.results.drain(..) {
                        self.live_results.push((fam, r));
                    }
                    self.sweep = None;
                }
            } else if rods_resync_due(&p.state, &self.rods.rods, false, self.rods_changed_s, now_s()) {
                let _ = p.state.set_rods(self.rods.rods);
            }
            // A pool failure is shown once, by the pool's status line.
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        if autostart_now {
            self.start_run_here();
        }
        // The next neutron.
        if self.current.as_ref().is_none_or(Anim::finished) && self.playing {
            if let Some(a) = self.current.take() {
                self.past.push_back(a.hist);
                while self.past.len() > 6 {
                    self.past.pop_front();
                }
            }
            let next = if self.live {
                self.queue.pop_front()
            } else if !self.recorded.is_empty()
                && (self.layers as usize) == super::super::LADDER_N as usize
                && self.pool_rods() == super::random_bed::WITHDRAWN
            {
                let h = self.recorded[self.next_recorded % self.recorded.len()].clone();
                self.next_recorded += 1;
                Some(h)
            } else {
                None
            };
            self.current = next.map(Anim::new);
        }
    }

    /// The main view: slice, tracks, status, and the restart buttons after a
    /// failed run. GUI drawing, not reached by a test (the test rule's
    /// exception); its decisions are `restart_wanted`, `restart_labels` and
    /// `restart`, which are.
    pub fn canvas(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        painter: &egui::Painter,
        resp: &egui::Response,
        link: Option<
            &dhoby_ghaut::web_demo::link::Link<crate::engine::Request, crate::engine::Event>,
        >,
    ) {
        self.slicer.view.handle_input(ui, resp);
        // The reader's pour, while it is being poured or not yet used: the
        // pebbles themselves, not the core (gh:#1244).
        if self.pour.shown && self.pour.last.is_some() && (self.pour.running() || self.layers as usize != super::random_bed::USER_POUR) {
            draw_pour(painter, rect, &self.slicer.view, self.pour.last.as_ref().map_or(&[][..], |s| &s.centres[..]), ui.visuals());
            return;
        }
        self.slicer.param = self.layers;
        self.slicer.rods = self.pool_rods();
        if let (Some(req), Some(l)) = (self.slicer.pump(rect), link) {
            l.send(crate::engine::Request::Raster(req));
        }
        self.slicer.draw(painter, rect);
        let view = self.slicer.view;
        let to_screen = |x: f64, y: f64| view.to_screen(rect, x, y);
        let n = self.past.len();
        for (i, h) in self.past.iter().enumerate() {
            draw_track(
                painter,
                to_screen,
                h,
                None,
                (30 + 40 * (i + 1) / n.max(1)) as u8,
                false,
                None,
            );
        }
        if let Some(a) = self.current.as_mut() {
            let dt = ui.input(|i| i.stable_dt).min(0.1) as f64;
            if self.playing {
                a.advance(dt, self.speed);
            }
            let upto = if a.finished() { None } else { Some(a.head()) };
            draw_track(painter, to_screen, &a.hist, upto, 255, true, None);
            ui.ctx().request_repaint();
        }
        // On the picture, only what it shows (maintainer 2026-10-26: "except
        // for the number of pebbles in the core and control rod state, all
        // text should be in the bottom panel"): the bed with its pebbles, and
        // the rods the drawn core holds. The run's status, the pool's and any
        // failure are in the bottom panel (`Self::status`).
        let mut y = rect.top() + if rect.width() < 560.0 { 108.0 } else { 52.0 };
        for t in self.picture_lines() {
            let ink = bugis::Ink::of_ui(ui);
            let c = ink.text;
            let g = painter.layout(t, egui::FontId::proportional(12.5), c, (rect.width() - 24.0).max(100.0));
            let bg = Rect::from_min_size(Pos2::new(rect.left() + 8.0, y - 2.0), g.size() + Vec2::new(8.0, 4.0));
            painter.rect_filled(bg, 4.0, ink.backing);
            let h = g.size().y;
            painter.galley(Pos2::new(rect.left() + 12.0, y), g, c);
            y += h + 6.0;
        }
    }

    /// The two lines the picture keeps: the bed drawn (with its core
    /// pebbles) and the rods of the core drawn.
    pub fn picture_lines(&self) -> [String; 2] {
        [
            format!("Bed: {}", super::random_bed::label(self.layers as usize)),
            format!("Control rods: {}", crate::htr10::rods::summary(&self.pool_rods())),
        ]
    }

    /// Everything the run says, for the bottom panel's text pane: whose
    /// neutrons the picture shows, the pool (data, workers, failures), the
    /// run's last generation and its time left. The picture's own two lines
    /// ([`Self::picture_lines`]) are not repeated.
    pub fn status(&self) -> Vec<(String, Color32)> {
        let mut lines: Vec<(String, Color32)> = vec![if self.live {
            ("LIVE: the picture shows neutrons of the k_eff run in this tab's workers".into(), crate::keffplot::LIVE)
        } else {
            ("RECORDED: the picture shows tracks baked natively (2026-10-07) from the record's model: 27 °C everywhere, graphite and SiC laws from the tapes".into(), crate::keffplot::AMBER)
        }];
        if let Some(p) = &self.pool {
            lines.extend(status_lines(p).into_iter().filter(|(t, _)| !t.starts_with("Bed: ") && !t.starts_with("Control rods: ")));
        }
        if let Some(e) = &self.error {
            lines.push((format!("FAILED: {e}"), crate::keffplot::RED));
        }
        if self.sweep.is_some() {
            lines.push((format!("Worth-curve sweep: {}", self.sweep_status), crate::keffplot::STATUS));
        }
        lines
    }

    /// The run's console: one row per generation, as the side panel's table
    /// (`gen  k  H  mean ± σ  wall`).
    pub fn console(&self) -> Vec<String> {
        let Some(run) = self.pool.as_ref().and_then(|p| p.state.run.as_ref()) else { return Vec::new() };
        let mut v = vec![format!("{} ({})", run.describe(), crate::keff::settings_line(&run.cfg)), " gen   k        H      mean ± σ             wall".to_string()];
        v.extend(run.gens.iter().map(|g| {
            let mean = g.gen.k_mean.map_or(String::new(), |(m, e)| format!("  {m:.5} ± {e:.5}"));
            let h = g.gen.entropy.map_or("      ".to_string(), |h| format!("  {h:.3}"));
            format!("{:>4}  {:.5}{h}{mean}  {:.1} s", g.gen.index + 1, g.gen.k, g.wall_s)
        }));
        v
    }

    /// Whether a run or a sweep is going (the bed and the rods the pool
    /// holds cannot change under it).
    pub fn busy(&self) -> bool {
        self.sweep.is_some() || self.pool.as_ref().and_then(|p| p.state.run.as_ref()).is_some_and(RunState::going)
    }

    /// Start k_eff at the reader's setting: straight away when the pool's
    /// core already holds the reader's rods, else as a one-position sweep,
    /// which rebuilds the core with them first (a run never starts on rods
    /// the reader has moved away from).
    pub fn start_run_here(&mut self) {
        let Some(p) = self.pool.as_mut() else { return };
        if p.state.rods.map(f64::to_bits) == self.rods.rods.map(f64::to_bits) {
            if let Err(e) = p.state.start_run(self.cfg, now_s()) {
                log::warn!("htr10 core: start refused: {e}");
            }
        } else {
            self.sweep = Some(crate::htr10::rods::LiveSweep::one(self.rods.rods, self.cfg));
        }
    }

    /// **The eight zone temperatures** (gh:#1142): a slider each, 20 to
    /// 1800 °C in 1 °C steps, "Same for all", a warning above 1600 °C, and Apply, which
    /// restarts the pool at the new temperatures (each zone's nuclides and
    /// laws processed exactly there; what is unchanged loads from this
    /// browser's cache). Fixed while a run or sweep is going. GUI code
    /// (exempt); its decisions are [`temperatures_to_apply`] and
    /// [`triso_warning`], tested.
    fn temperatures_ui(&mut self, ui: &mut egui::Ui) {
        let busy = self.busy();
        let (lo, hi) = (celsius(min_temperature()), celsius(max_temperature()));
        ui.add_enabled_ui(!busy, |ui| {
            ui.checkbox(&mut self.temps_linked, "Same for all zones");
            let mut moved: Option<f64> = None;
            for (i, z) in Htr10Zone::ALL.into_iter().enumerate() {
                let r = ui.add(
                    // 1 °C steps: the record's 27 °C (300.15 K) is on the
                    // grid; with 5 °C steps from 20 the slider snapped it to
                    // 25 the moment it was touched (gh:#1142).
                    egui::Slider::new(&mut self.temps_c[i], lo..=hi)
                        .step_by(1.0)
                        .fixed_decimals(0)
                        .suffix(" °C")
                        .text(z.label()),
                );
                if r.changed() {
                    moved = Some(self.temps_c[i]);
                }
            }
            if let (true, Some(c)) = (self.temps_linked, moved) {
                self.temps_c = [c; 8];
            }
            if ui.button("Reset: every zone at 27 °C (the record's)").clicked() {
                self.temps_c = Htr10Zone::ALL.map(|z| celsius(ZoneTemperatures::record().get(z)));
            }
        });
        if let Some(w) = triso_warning(&self.temps_c) {
            ui.colored_label(ui.visuals().error_fg_color, w);
        }
        match temperatures_to_apply(&self.temps, &self.temps_c) {
            Err(e) => {
                ui.colored_label(ui.visuals().error_fg_color, e);
            }
            Ok(None) => {
                ui.weak("The core is built at these temperatures.");
            }
            Ok(Some(want)) => {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    "Not applied yet. Apply reprocesses each changed zone's nuclides (RECONR, BROADR, HEATR, PURR) and thermal laws (LEAPR) at its temperature: minutes for the fuel, seconds for most; zones you did not change load from this browser's cache.",
                );
                let label = if self.pool.is_some() { "Apply: restart the pool at these temperatures" } else { "Apply (the pool starts at these temperatures)" };
                if ui.add_enabled(!busy, egui::Button::new(label).min_size(Vec2::new(0.0, 36.0))).clicked() {
                    self.temps = want;
                    if self.pool.is_some() {
                        let ctx = ui.ctx().clone();
                        self.reprocess(&ctx);
                    }
                }
            }
        }
        ui.weak("Every bound thermal law is made by LEAPR at its zone's temperature, so at 27 °C everywhere the model differs from the record's (graphite and SiC read from the tapes at 296 K) by that law alone.");
    }

    /// The bed: the record's lattice at N layers (10–20), or the DEM bed.
    /// Free before the pool starts (it starts at this N); between runs the
    /// workers rebuild the core when it changes.
    fn bed_ui(&mut self, ui: &mut egui::Ui) {
        let busy = self.busy();
        let dem_code = super::random_bed::DEM_BED as f64;
        let pour_code = super::random_bed::USER_POUR as f64;
        let mut want = self.layers;
        let shown = self.pour.shown;
        ui.add_enabled_ui(!busy, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Bed:");
                let lattice = self.layers != dem_code && self.layers != pour_code;
                if ui.selectable_label(lattice && !shown, "Şeker lattice (the record's)").clicked() {
                    self.pour.shown = false;
                    if !lattice {
                        want = super::super::LADDER_N;
                    }
                }
                if ui.selectable_label(self.layers == dem_code && !shown, "DEM random bed (baked)").clicked() {
                    self.pour.shown = false;
                    want = dem_code;
                }
                if ui.selectable_label(shown, "DEM pour (yours)").clicked() {
                    self.pour.shown = true;
                    if self.pour.bed.is_some() {
                        want = pour_code;
                    }
                }
            });
            if self.pour.shown {
                // The pour's own controls follow, outside this disabled scope.
            } else if want == dem_code {
                ui.weak(super::random_bed::label(super::random_bed::DEM_BED));
            } else {
                let mut n = want;
                ui.add(egui::Slider::new(&mut n, 10.0..=20.0).step_by(1.0).fixed_decimals(0).text("layers N (stacks)"));
                want = n;
                ui.weak(format!("Bed height 9.798 N + 6 = {:.3} cm (Şeker & Çolak 2003's rows; N = 12 is the critical loading).", 9.798 * want + 6.0));
            }
        });
        if busy {
            ui.weak("Fixed while a run or sweep is going.");
        }
        if want != self.layers {
            match self.pool.as_mut() {
                None => self.layers = want,
                Some(p) => {
                    if p.state.set_layers(want as usize).is_ok() {
                        self.layers = want;
                    }
                }
            }
        }
        if self.pour.shown {
            self.pour_ui(ui, busy);
        } else if self.layers as usize == super::random_bed::DEM_BED {
            ui.colored_label(ui.visuals().warn_fg_color, DEM_CAVEAT);
            ui.label(dem_record_line());
        }
    }

    /// The reader's pour (gh:#1244): pebbles, fuel fraction, seed; Pour,
    /// Pause/Resume; "Use this bed", which hands it to every worker and
    /// rebuilds the core on it. GUI drawing (the test rule's exception); its
    /// decisions are `PourUi`'s and `PoolState::use_pour`, which are tested.
    fn pour_ui(&mut self, ui: &mut egui::Ui, busy: bool) {
        let now = dhoby_ghaut::web_demo::platform::now_s();
        let max = super::pour::max_pebbles() as f64;
        let running = self.pour.running();
        ui.add_enabled_ui(!running, |ui| {
            ui.add(
                egui::Slider::new(&mut self.pour.n, super::pour::MIN_PEBBLES as f64..=max)
                    .step_by(100.0)
                    .fixed_decimals(0)
                    .text("pebbles to pour"),
            );
            ui.weak(format!(
                "Starts at {} (the first-criticality loading's {} core pebbles plus the conus and tube's 2 457). Up to {} pebbles: the conus and tube plus a core filled at φ {} to {:.1} cm, 95 % of the {:.3} cm cavity. Any pebble that still stands above that line is removed when the bed is used.",
                super::random_bed::grouped(super::pour::DEFAULT_PEBBLES),
                super::random_bed::grouped(super::pour::RECORD_CORE_PEBBLES),
                super::random_bed::grouped(max as usize),
                super::pour::FRESH_POUR_PHI,
                super::pour::HEIGHT_LIMIT_CM,
                nee_soon::htr10_rmc::core_model::HTR10_CORE_CAVITY_CM
            ));
            ui.horizontal(|ui| {
                ui.label("seed");
                ui.add(egui::DragValue::new(&mut self.pour.seed).range(1..=1_000_000));
            });
        });
        // gh:#1250 (maintainer, 2026-11-05): the pour stays on the CPU, so the
        // page says how long it takes before the reader starts one.
        // gh:#1250: a second engine for the same pour, and the time each took.
        ui.add_enabled_ui(!running, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Engine:");
                ui.selectable_value(&mut self.pour.gpu, false, "CPU (one thread)");
                ui.selectable_value(&mut self.pour.gpu, true, "GPU (WebGPU)");
            });
        });
        let n = self.pour.n.round() as usize;
        if self.pour.gpu {
            ui.label("GPU: the same LIGGGHTS-port physics as WGSL compute shaders (outram-park-fork-liggghts gpu::dem), in f32 with double-f32 overlaps. Not bit-identical to the CPU; it is checked against CPU pours on packing fraction and g(r). Its speed depends on your device: the table below shows what each pour took.");
        } else {
            ui.label(format!(
                "A pour of {} takes {} in a browser (one thread; measured: 4 000 in 4 min, 8 000 in 6 min, 30 687 in 17.5 min on a 4-core machine). The page stays usable while it pours; the baked bed above is ready now.",
                super::random_bed::grouped(n),
                super::pour::minutes(super::pour::expected_pour_s(n))
            ));
        }
        let mut f = 100.0 * self.pour.fuel;
        ui.add(egui::Slider::new(&mut f, 0.0..=100.0).step_by(1.0).fixed_decimals(0).suffix(" %").text("fuel pebbles (rest moderator)"));
        self.pour.fuel = f / 100.0;
        let ratio = if self.pour.fuel < 1.0 { format!("{:.2}", self.pour.fuel / (1.0 - self.pour.fuel)) } else { "∞".into() };
        ui.weak(format!(
            "Fuel : moderator = {ratio} : 1 above the floor (the record is 57 : 43); the conus and tube hold moderator balls only, as the paper's rule says."
        ));
        ui.horizontal_wrapped(|ui| {
            let verb = if self.pour.last.is_some() { "Pour again" } else { "Pour" };
            let label = if self.pour.gpu {
                format!("{verb} on the GPU")
            } else {
                format!("{verb} ({})", super::pour::minutes(super::pour::expected_pour_s(self.pour.n.round() as usize)))
            };
            if ui.button(label).clicked() {
                self.pour.start(now);
            }
            if self.pour.last.is_some() && !self.pour.finished() {
                let label = if self.pour.paused { "Resume" } else { "Pause" };
                if ui.button(label).clicked() {
                    let p = !self.pour.paused;
                    self.pour.set_paused(p);
                }
            }
            let usable = self.pour.last.is_some() && !running && !busy;
            if ui.add_enabled(usable, egui::Button::new("Use this bed for k_eff")).clicked() {
                self.use_pour();
            }
        });
        for l in self.pour.lines(now) {
            ui.label(l);
        }
        if let Some(s) = &self.pour.last {
            if !s.settled && !running {
                ui.colored_label(ui.visuals().warn_fg_color, "This pour has not settled: a bed used now is a snapshot of falling pebbles.");
            }
        }
        if let Some(e) = &self.pour.error {
            ui.colored_label(ui.visuals().error_fg_color, e);
        }
        if let Some(b) = &self.pour.bed {
            ui.label(super::pour::describe(b));
        }
        if !self.pour.timings.is_empty() {
            egui::CollapsingHeader::new("Timings: CPU against GPU (this device)").default_open(true).show(ui, |ui| {
                for t in &self.pour.timings {
                    ui.label(t.line());
                }
                ui.weak("Pour the same count on each engine to compare; a different count or seed is a different pour.");
            });
        }
        // Post-pour diagnostics (maintainer 2026-11-05): packing fractions and g(r).
        if self.pour.last.is_some() && !running && self.pour.diagnostics.is_none() && !self.pour.finished() && ui.button("Measure this pour (φ, g(r))").clicked() {
            self.pour.measure();
        }
        if let Some(d) = &self.pour.diagnostics {
            egui::CollapsingHeader::new("Pour diagnostics: packing fraction and g(r)").default_open(true).show(ui, |ui| {
                for l in d.lines() {
                    ui.label(l);
                }
                if !d.g.is_empty() {
                    rdf_plot(ui, d);
                }
            });
        } else if self.pour.finished() {
            ui.weak("Measuring the pour (φ, g(r)) in the worker…");
        }
        ui.weak("The pour is the LIGGGHTS port's (µ 0.1, E 5e8 Pa, dt 35 µs) on one thread in this page's worker; a full core takes tens of minutes in a browser. The core is then built as the baked DEM bed is, TRISO packed at random.");
        if running {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
        }
    }

    /// Make the bed from the pour and hand it over: to this page (labels,
    /// the slice's worker) and, between runs, to every pool worker; before
    /// the pool starts, it starts on it.
    fn use_pour(&mut self) {
        let b = match self.pour.make_bed() {
            Ok(b) => b,
            Err(e) => {
                self.pour.error = Some(e);
                return;
            }
        };
        log::info!("htr10 core: pour used as the bed: {}", super::pour::describe(&b));
        super::random_bed::set_poured(b.clone());
        // The page's own worker draws the slice: it needs the bed too.
        self.pour.queue(super::CoreReq::pour_bed(&b));
        let code = super::random_bed::USER_POUR;
        match self.pool.as_mut() {
            None => self.layers = code as f64,
            Some(p) => match p.state.use_pour(&b) {
                Ok(()) => self.layers = code as f64,
                Err(e) => self.pour.error = Some(format!("the bed is kept for later: {e}")),
            },
        }
    }

    /// The pour's requests for the page's own worker (the app sends them).
    pub fn take_pour_out(&mut self) -> Vec<super::CoreReq> {
        self.pour.take_out()
    }

    /// The side panel: the core (bed N and control rods) and its run in one
    /// place (maintainer 2026-10-25: "change the number of stacks and control
    /// rod position under the same tab"), then the worth-curve sweeps and the
    /// neutron animation. `curves` are the recorded rod curves.
    pub fn panel(&mut self, ui: &mut egui::Ui, curves: &[crate::htr10::rods::RecordedCurve]) {
        ui.label("The whole HTR-10 core: reflector, rods, cavity, the pebble bed and, inside each fuel pebble, the TRISO particles. Set the bed and the rods here, run k_eff, and read k and entropy in the bottom panel; the top and rod views and the recorded curves are in Views.");
        ui.separator();
        ui.strong("The core: layers and control rods");
        self.bed_ui(ui);
        ui.add_space(4.0);
        ui.add_enabled_ui(!self.busy(), |ui| {
            if crate::htr10::rods::controls_ui(ui, &mut self.rods) {
                self.rods_changed_s = now_s();
            }
        });
        egui::CollapsingHeader::new("Zone temperatures").default_open(true).show(ui, |ui| {
            self.temperatures_ui(ui);
        });
        let built = self.pool_rods();
        if self.pool.is_some() && built.map(f64::to_bits) != self.rods.rods.map(f64::to_bits) {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                format!("The workers' core still has: {}. It is rebuilt with yours once they rest, or when you start a run.", crate::htr10::rods::summary(&built)),
            );
        }
        egui::CollapsingHeader::new("Recorded k_eff at this setting (native runs)").default_open(true).show(ui, |ui| {
            if !self.temps.is_uniform() || (celsius(self.temps.get(Htr10Zone::FuelPebble)) - 27.0).abs() > 1e-9 {
                ui.colored_label(ui.visuals().warn_fg_color, "Every record below is at 27 °C in every zone: it is not a reference for these temperatures.");
            }
            if self.rods.rods == [0.0; 10] && !super::random_bed::is_dem(self.layers as usize) {
                if let Some((k, s, rmc)) = record_row(self.layers as usize) {
                    ui.label(format!("Every rod withdrawn, N = {}: {k:.6} ± {s:.6} (10 000 × [5 + 135], 2026-10-07); RMC (Li, Yu & Wei 2014) at equal ball count {rmc:.6}.", self.layers as usize));
                }
            }
            crate::htr10::rods::recorded_ui(ui, curves, &self.rods.rods);
            ui.weak("The rod curves are recorded at N = 12 only.");
        });
        ui.separator();
        ui.strong("Run k_eff (the whole core)");
        let ctx = ui.ctx().clone();
        let mut start = false;
        match self.pool.as_mut() {
            None => {
                ui.label(format!(
                    "Processes every nuclide of the model at each zone temperature it is needed at ({} jobs at these temperatures: ENDF/B-VIII.0 tapes, about 35 MB to download, and thermal laws by LEAPR) in a pool of Web Workers, each once, then runs k_eff of the whole core at the statistics you set (up to the recorded 10 000 × [5 + 135] and beyond). Pool: {}.",
                    super::jobs_at(&self.temps).map_or(0, |j| j.len()),
                    self.plan.reason
                ));
                if self.forced.is_none() {
                    ui.weak("Add ?workers=N to the address to choose the pool size.");
                }
                if ui.add(egui::Button::new(format!("▶ Start the worker pool ({} workers)", self.plan.workers)).min_size(Vec2::new(0.0, 36.0))).clicked() {
                    self.start_pool(&ctx);
                }
            }
            Some(p) => match pool_panel(ui, p, &mut self.cfg, self.layers) {
                Some(PoolAction::Reprocess) => self.reprocess(&ctx),
                Some(PoolAction::Start) => start = true,
                None => {}
            },
        }
        if start {
            self.start_run_here();
        }
        egui::CollapsingHeader::new("Worth curves: a sweep of runs over rod positions").default_open(self.sweep.is_some()).show(ui, |ui| {
            crate::htr10::rods::sweeps_ui(ui, self);
        });
        ui.separator();
        egui::CollapsingHeader::new("Neutrons on the main view").default_open(false).show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button(if self.playing { "⏸ Pause" } else { "▶ Play" }).clicked() {
                    self.playing = !self.playing;
                }
                if ui.button("Next neutron").clicked() {
                    if let Some(a) = self.current.as_mut() {
                        a.shown_cm = a.total();
                    }
                }
            });
            ui.add(egui::Slider::new(&mut self.speed, 0.0001..=20_000.0).logarithmic(true).text("speed at 1 eV (cm/s)").custom_formatter(|v, _| fmt_speed(v)));
            ui.weak(format!("= {} at 0.0253 eV (thermal). v is proportional to √E: a fission neutron crosses the core in a moment, a thermal one crawls. Zoom in (+) on the bed to see the pebbles and particles the neutrons cross.", fmt_speed(animated_speed(0.0253, self.speed))));
            legend_markers(ui, true);
            energy_bar(ui);
        });
        egui::CollapsingHeader::new("The whole-core view: what it is, and is not").default_open(false).show(ui, |ui| {
            for l in NOTES {
                ui.label(format!("• {l}"));
            }
        });
    }

    /// The bottom dock's buttons, in its header beside the toggle: start
    /// the pool, then Run / Pause / Resume and Stop, so the core runs from
    /// the main view with every panel folded. GUI code (exempt); Start's
    /// decision is [`Self::start_run_here`]'s.
    pub fn keff_buttons(&mut self, ui: &mut egui::Ui, row: Rect) {
        let h = row.height().min(44.0);
        let narrow = row.width() < 300.0;
        let button = |ui: &mut egui::Ui, r: Rect, label: String| ui.put(r, egui::Button::new(egui::RichText::new(label).size(15.0))).clicked();
        let note = |t: String, c: Color32| (t, c);
        let grey = bugis::Ink::of_ui(ui).muted;
        let mut text: Option<(String, Color32)> = None;
        let bw = row.width().min(330.0);
        let one = Rect::from_min_size(row.min, Vec2::new(bw, h));
        let Some(p) = self.pool.as_mut() else {
            let label = if narrow { "▶ Start pool".to_string() } else { format!("▶ Start the worker pool ({} workers)", self.plan.workers) };
            if button(ui, one, label) {
                self.start_pool(ui.ctx());
            }
            return;
        };
        let s = &mut p.state;
        // After a failure, its remedies take the Run button's place (gh:#817;
        // they were on the picture until 2026-10-26).
        if matches!(s.phase, PoolPhase::Failed(_)) {
            if button(ui, one, if narrow { "⟳ Reprocess".into() } else { "⟳ Reprocess the data (new workers)".into() }) {
                self.reprocess(ui.ctx());
            }
            return;
        }
        if restart_wanted(s) {
            let half = Vec2::new(((row.width() - 6.0) / 2.0).min(260.0), h);
            for (i, (kind, label)) in restart_labels(s).into_iter().enumerate() {
                let label = if narrow { label.split(" (").next().unwrap_or(&label).to_string() } else { label };
                if button(ui, Rect::from_min_size(row.min + Vec2::new(i as f32 * (half.x + 6.0), 0.0), half), label) {
                    restart(s, kind);
                }
            }
            return;
        }
        if s.phase != PoolPhase::Ready {
            text = Some(note(format!("Preparing the data: {:.0} %", 100.0 * s.progress()), grey));
        }
        if text.is_none() {
            let run = s.run.as_ref();
            let going = run.is_some_and(RunState::going);
            if self.sweep.is_some() && !going {
                text = Some(note(format!("Rebuilding the core: {}", self.sweep_status), grey));
            } else if going {
                let r = run.expect("going");
                let (done, total) = (r.gens.len(), r.cfg.n_inactive + r.cfg.n_active);
                let paused = r.paused;
                let stop_w = 76.0;
                let main_w = (row.width() - stop_w).clamp(60.0, 330.0);
                let label = match (paused, narrow) {
                    (true, _) => "▶ Resume".to_string(),
                    (false, true) => format!("⏸ {done}/{total}"),
                    (false, false) => format!("⏸ Pause · gen {done}/{total}"),
                };
                if button(ui, Rect::from_min_size(row.min, Vec2::new(main_w, h)), label) {
                    s.set_paused(!paused);
                }
                if button(ui, Rect::from_min_size(row.min + Vec2::new(main_w + 6.0, 0.0), Vec2::new(stop_w - 6.0, h)), "■ Stop".into()) {
                    s.stop_run();
                }
                return;
            } else {
                let label = if narrow { "▶ Run k_eff".to_string() } else { format!("▶ Run k_eff ({})", crate::keff::settings_line(&self.cfg)) };
                if button(ui, one, label) {
                    self.start_run_here();
                }
                return;
            }
        }
        if let Some((t, c)) = text {
            ui.painter().text(Pos2::new(row.left() + 4.0, row.center().y), egui::Align2::LEFT_CENTER, t, egui::FontId::proportional(13.0), c);
        }
    }

    /// The bottom dock's body: this run's k and entropy (the shared plots,
    /// `crate::keffplot`) against what is recorded for the core it runs: the
    /// record and RMC at its N with every rod out, the recorded rod curve at
    /// its rods, else nothing. A line above says the run's mean against them.
    pub fn keff_body(&self, ui: &mut egui::Ui, painter: &egui::Painter, rect: Rect, text: f32, curves: &[crate::htr10::rods::RecordedCurve]) {
        use crate::keffplot::{self, GenPoint, RefLine, RunShape};
        let rect = rect.shrink2(Vec2::new(8.0, 4.0));
        let run = self.pool.as_ref().and_then(|p| p.state.run.as_ref());
        let cfg = run.map_or(self.cfg, |r| r.cfg);
        let pts: Vec<GenPoint> = run.map(|r| r.gens.iter().map(|g| GenPoint::from(&g.gen)).collect()).unwrap_or_default();
        let shape = RunShape { n_inactive: cfg.n_inactive, total: cfg.n_inactive + cfg.n_active };
        // What the run ran (the pool may have been rebuilt since), else what
        // the next run will: its references follow that.
        let (built, layers) = run.map_or((self.pool_rods(), self.layers as usize), |r| (r.rods, r.layers));
        let dem = layers == super::random_bed::DEM_BED;
        // The reader's pour has no recorded reference (`record_row(0)` is none).
        let n = if dem { super::super::LADDER_N as usize } else if layers == super::random_bed::USER_POUR { 0 } else { layers };
        let mut refs = Vec::new();
        if built == super::random_bed::WITHDRAWN {
            if let Some((k, s, rmc)) = record_row(n) {
                refs.push(RefLine { label: format!("RMC {rmc:.4}"), k: rmc, sigma: None, colour: keffplot::REFERENCE_COLOUR });
                let what = if dem { "lattice recorded, N = 12" } else { "recorded" };
                refs.push(RefLine { label: format!("{what} {k:.4} ± {s:.4}"), k, sigma: Some(s), colour: keffplot::RECORD_COLOUR });
            }
        } else if n == super::super::LADDER_N as usize {
            match crate::htr10::rods::lookup(curves, &built) {
                crate::htr10::rods::Lookup::Exact { point, .. } => refs.push(RefLine { label: format!("recorded at these rods {:.4} ± {:.4}", point.k, point.sigma), k: point.k, sigma: Some(point.sigma), colour: keffplot::RECORD_COLOUR }),
                crate::htr10::rods::Lookup::Interpolated { k, .. } => refs.push(RefLine { label: format!("recorded curve, interpolated {k:.4}"), k, sigma: None, colour: keffplot::RECORD_COLOUR }),
                crate::htr10::rods::Lookup::None(_) => {}
            }
        }
        // One line: what is running and the mean against the first reference.
        let mean = pts.last().and_then(|g| g.k_mean);
        let line = match (run, mean) {
            // The bed and the rods are on the picture; the strip says only
            // what the run is (it must fit two lines on a phone).
            (None, _) => format!("No run yet. Next: {}", crate::keff::settings_line(&self.cfg)),
            (Some(_), None) => "Inactive generations: the source is settling; k is not averaged yet".to_string(),
            (Some(_), Some((m, e))) => {
                let mut l = format!("k = {m:.5} ± {e:.5}");
                if let Some(r) = refs.iter().find(|r| r.sigma.is_some()) {
                    let d = (m - r.k) * 1e5;
                    let sd = (e * e + r.sigma.unwrap_or(0.0).powi(2)).sqrt() * 1e5;
                    l.push_str(&format!(" · you − recorded = {d:+.0} ± {sd:.0} pcm"));
                }
                l
            }
        };
        // Laid out as every k_eff run's bottom panel (`keffplot::body_layout`):
        // plots, the text pane (status and console) and the one-line strip.
        let (plots, pane, strip) = keffplot::body_layout(rect);
        keffplot::draw_strip(painter, strip, line, bugis::Ink::of_ui(ui).text, text);
        let ceiling = run.and_then(RunState::entropy_bins).map(|b| (b as f64).r_log2());
        keffplot::draw_pair(painter, plots, &pts, shape, &refs, ceiling, (text * 0.92).max(9.0));
        keffplot::draw_text_pane(ui, painter, pane, &self.status(), &self.console(), "", (text * 0.9).max(9.0), "core-console");
    }
}

/// What the view says about the pool, one line each.
/// **The pour as it falls** (gh:#1244), side-on in the core view's frame:
/// the pebbles cut by the plane `y = 0` (each a disc of its cut radius), the
/// vessel wall (`htr10_fill::wall_radius_at`), the floor and the 95 % line.
/// The DEM frame's floor is drawn where the built core would put it at the
/// pour's height (`z_core = z_DEM − h/2`, `h` the top of the highest pebble,
/// the explicit bed's frame). GUI drawing, not reached by a test (the test
/// rule's exception).
fn draw_pour(painter: &egui::Painter, rect: Rect, view: &dhoby_ghaut::web_demo::view::View, centres_m: &[f32], visuals: &egui::Visuals) {
    use outram_park_fork_liggghts::htr10_fill::{wall_radius_at, VALVE_Z_M};
    let theme = bugis::Theme::current(visuals);
    let r = super::pour::R_CM;
    painter.rect_filled(rect, 0.0, visuals.extreme_bg_color);
    let top = centres_m.chunks_exact(3).map(|c| 100.0 * f64::from(c[2]) + r).fold(2.0 * r, f64::max);
    let shift = 0.5 * top.min(super::pour::HEIGHT_LIMIT_CM);
    let at = |x: f64, z: f64| view.to_screen(rect, x, z - shift);
    // The wall, from the valve to the cavity top.
    let wall = egui::Stroke::new(1.5, theme.accent(4));
    let z_hi = nee_soon::htr10_rmc::core_model::HTR10_CORE_CAVITY_CM;
    let n = 120;
    for side in [-1.0, 1.0] {
        let pts: Vec<Pos2> = (0..=n)
            .map(|i| {
                let z = 100.0 * VALVE_Z_M + (z_hi - 100.0 * VALVE_Z_M) * i as f64 / n as f64;
                at(side * 100.0 * wall_radius_at(z / 100.0), z)
            })
            .collect();
        painter.add(egui::Shape::line(pts, wall));
    }
    let line = |z: f64, c: Color32| {
        painter.line_segment([at(-90.0, z), at(90.0, z)], egui::Stroke::new(1.0, c));
    };
    line(0.0, visuals.weak_text_color());
    line(super::pour::HEIGHT_LIMIT_CM, visuals.error_fg_color);
    line(z_hi, theme.accent(4));
    let scale = (at(1.0, 0.0).x - at(0.0, 0.0).x).abs();
    for c in centres_m.chunks_exact(3) {
        let (x, y, z) = (100.0 * f64::from(c[0]), 100.0 * f64::from(c[1]), 100.0 * f64::from(c[2]));
        if y.abs() >= r {
            continue;
        }
        let rr = (r * r - y * y).sqrt() * f64::from(scale);
        let col = if z > 0.0 { visuals.text_color() } else { visuals.weak_text_color() };
        painter.circle_filled(at(x, z), rr.max(1.0) as f32, col);
    }
    painter.text(
        at(92.0, super::pour::HEIGHT_LIMIT_CM),
        egui::Align2::LEFT_CENTER,
        "95 %",
        egui::FontId::proportional(12.0),
        visuals.error_fg_color,
    );
}

/// **The pour's g(r)**, with the reference random bed's points
/// ([`super::pour::REFERENCE_G`]) marked, √2 d (where a crystal would peak),
/// √3 d and 2 d as guides, and g = 1. Colours from the Gruvbox theme
/// (bugis). GUI drawing, not reached by a test (the test rule's exception);
/// its numbers are `pour::diagnostics`, which is.
fn rdf_plot(ui: &mut egui::Ui, d: &super::pour::PourDiagnostics) {
    let theme = bugis::Theme::current(ui.visuals());
    let fg = ui.visuals().text_color();
    let faint = ui.visuals().weak_text_color();
    let w = ui.available_width().max(160.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 170.0), egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);
    let plot = Rect::from_min_max(rect.min + Vec2::new(34.0, 8.0), rect.max - Vec2::new(8.0, 22.0));
    let x_max = d.r_max_d.max(1.0);
    // Log-free linear axis, clipped at 6 so the second shell reads; the
    // contact peak's height is in the line above.
    let y_max = 6.0;
    let at = |x: f64, y: f64| Pos2::new(plot.left() + (x / x_max) as f32 * plot.width(), plot.bottom() - (y.min(y_max) / y_max) as f32 * plot.height());
    for (x, label) in [(1.0, "d"), (2f64.sqrt(), "√2"), (3f64.sqrt(), "√3"), (2.0, "2d")] {
        if x <= x_max {
            p.line_segment([at(x, 0.0), at(x, y_max)], egui::Stroke::new(0.5, faint));
            p.text(at(x, 0.0) + Vec2::new(0.0, 3.0), egui::Align2::CENTER_TOP, label, egui::FontId::proportional(10.0), faint);
        }
    }
    p.line_segment([at(0.0, 1.0), at(x_max, 1.0)], egui::Stroke::new(0.5, faint));
    for y in [0.0, 1.0, 2.0, 4.0, 6.0] {
        p.text(Pos2::new(plot.left() - 4.0, at(0.0, y).y), egui::Align2::RIGHT_CENTER, format!("{y}"), egui::FontId::proportional(10.0), faint);
    }
    let pts: Vec<Pos2> = d.r_over_d.iter().zip(&d.g).filter(|(_, g)| g.is_finite()).map(|(&x, &g)| at(x, g)).collect();
    p.add(egui::Shape::line(pts, egui::Stroke::new(1.5, theme.accent(0))));
    let [g2, g3, g4, _] = super::pour::REFERENCE_G;
    for (x, g) in [(2f64.sqrt(), g2), (3f64.sqrt(), g3), (2.0, g4)] {
        if x <= x_max {
            p.circle_stroke(at(x, g), 3.5, egui::Stroke::new(1.5, theme.accent(1)));
        }
    }
    p.text(plot.right_top(), egui::Align2::RIGHT_TOP, "g(r): yours (line), reference bed (circles)", egui::FontId::proportional(10.5), fg);
    p.text(Pos2::new(plot.center().x, rect.bottom() - 2.0), egui::Align2::CENTER_BOTTOM, "r / d", egui::FontId::proportional(10.0), faint);
}

fn status_lines(p: &CorePool) -> Vec<(String, Color32)> {
    let s = &p.state;
    // Named colours: `keffplot::themed` gives each its theme colour when drawn.
    let white = crate::keffplot::STATUS;
    let mut v = Vec::new();
    let el = now_s() - s.started_s;
    match &s.phase {
        PoolPhase::Processing => {
            v.push((
                format!(
                    "Processing nuclear data in {} workers: {}/{} jobs, {:.0} % · {el:.0} s",
                    s.workers,
                    s.secs.iter().filter(|x| x.is_some()).count(),
                    s.secs.len(),
                    100.0 * s.progress()
                ),
                white,
            ));
            if s.sources.iter().any(Option::is_some) {
                v.push((format!("So far: {}", s.data_source()), white));
            }
        }
        PoolPhase::Assembling => v.push((
            format!(
                "Building nuclides, core and majorant in every worker: {}/{} · {el:.0} s",
                s.assembled.iter().filter(|a| a.is_some()).count(),
                s.workers
            ),
            white,
        )),
        PoolPhase::Ready => {
            let data = s.data_ready_s.map_or(0.0, |t| t - s.started_s);
            v.push((
                format!(
                    "Data ready in {data:.0} s ({} workers, up to {:.0} MB of wasm memory each)",
                    s.workers,
                    s.assembled
                        .iter()
                        .flatten()
                        .map(|a| a.memory_mb)
                        .fold(0.0, f64::max)
                ),
                white,
            ));
            // Where the data came from (gh:#818, Leak Before Break).
            v.push((format!("Nuclear data: {}", s.data_source()), white));
            v.push((format!("Bed: {}", super::random_bed::label(s.layers)), white));
            // gh:#1025: what the workers' core holds (set in "Control rods").
            v.push((format!("Control rods: {}", crate::htr10::rods::summary(&s.rods)), white));
            if s.layers == super::random_bed::DEM_BED {
                v.push((DEM_CAVEAT.to_string(), crate::keffplot::AMBER));
                v.push((dem_record_line(), white));
            }
            let amber = crate::keffplot::AMBER;
            if s.live_workers() < s.workers {
                v.push((dead_line(s), amber));
            }
            if let Some(run) = &s.run {
                let total = run.cfg.n_inactive + run.cfg.n_active;
                if let RunOrigin::Restart { .. } = run.origin {
                    v.push((format!("This is {}.", run.describe()), amber));
                }
                if let Some(e) = &run.failed {
                    v.push((format!("⚠ {e}"), crate::keffplot::RED));
                }
                match run.gens.last() {
                    None if run.failed.is_some() => {}
                    None => v.push(("k_eff: sampling the initial source…".into(), white)),
                    Some(g) => {
                        let mean = g.gen.k_mean.map_or(String::from("(inactive)"), |(m, e)| {
                            format!("mean {m:.5} ± {e:.5}")
                        });
                        v.push((
                            format!(
                                "k_eff generation {}/{total}: k = {:.5}, {mean} · {:.1} s/gen",
                                g.gen.index + 1,
                                g.gen.k,
                                g.wall_s
                            ),
                            white,
                        ));
                        if let Some(l) = run.going().then(|| run_eta(run)).flatten() {
                            let colour = if l.starts_with('⚠') { amber } else { white };
                            v.push((l, colour));
                        }
                    }
                }
            }
        }
        PoolPhase::Failed(e) => v.push((
            format!("⚠ {e}"),
            crate::keffplot::RED,
        )),
    }
    v
}

/// Which workers have stopped, and why (gh:#817).
fn dead_line(s: &PoolState) -> String {
    let dead: Vec<String> = (0..s.workers)
        .filter_map(|w| s.died[w].as_ref().map(|why| format!("worker {} ({why})", w + 1)))
        .collect();
    format!(
        "{} of {} workers left; their data are intact. Stopped: {}.",
        s.live_workers(),
        s.workers,
        dead.join("; ")
    )
}

/// The labels of the two restart buttons (gh:#817).
fn restart_labels(s: &PoolState) -> [(RestartKind, String); 2] {
    let n = s.live_workers();
    [
        (
            RestartKind::Parallel,
            format!("↻ Restart run in parallel ({n} worker{})", if n == 1 { "" } else { "s" }),
        ),
        (RestartKind::Serial, "↻ Restart run serially (1 worker)".to_string()),
    ]
}

/// Restart the failed run, reporting a refusal in the console.
fn restart(s: &mut PoolState, kind: RestartKind) {
    if let Err(e) = s.restart_run(kind, now_s()) {
        log::warn!("htr10 core: restart refused: {e}");
    }
}

/// The run failed and the data are intact: offer the restarts.
fn restart_wanted(s: &PoolState) -> bool {
    s.phase == PoolPhase::Ready && s.run.as_ref().is_some_and(|r| r.failed.is_some())
}

/// What a button asked of the screen that the pool cannot do itself.
pub enum PoolAction {
    /// Stop every worker and process the data again (no worker left).
    Reprocess,
    /// Run k_eff at the reader's setting ([`CoreScreen::start_run_here`]:
    /// the rods may need a rebuild first).
    Start,
}

/// The pool's part of the side panel. GUI drawing, not reached by a test
/// (the test rule's exception); its decisions are tested through
/// `restart_wanted`, `restart_labels`, `restart` and `dead_line`.
fn pool_panel(
    ui: &mut egui::Ui,
    p: &mut CorePool,
    cfg: &mut KeffConfig,
    layers: f64,
) -> Option<PoolAction> {
    let s = &mut p.state;
    ui.label(format!("Pool: {}", p.plan.reason));
    let red = ui.visuals().error_fg_color;
    let amber = ui.visuals().warn_fg_color;
    if let PoolPhase::Failed(e) = &s.phase {
        ui.colored_label(red, e);
        if ui
            .add(egui::Button::new("⟳ Reprocess the data (new workers)").min_size(Vec2::new(0.0, 36.0)))
            .clicked()
        {
            return Some(PoolAction::Reprocess);
        }
    }
    if s.live_workers() < s.workers {
        ui.colored_label(amber, dead_line(s));
    }
    if restart_wanted(s) {
        if let Some(e) = s.run.as_ref().and_then(|r| r.failed.clone()) {
            ui.colored_label(red, e);
        }
        for (kind, label) in restart_labels(s) {
            if ui
                .add(egui::Button::new(label).min_size(Vec2::new(0.0, 36.0)))
                .clicked()
            {
                restart(s, kind);
            }
        }
    }
    if !s.log.is_empty() {
        egui::CollapsingHeader::new(format!(
            "Pool log ({} entries, {} stale replies discarded)",
            s.log.len(),
            s.stale_discarded
        ))
        .default_open(false)
        .show(ui, |ui| {
            for l in &s.log {
                ui.weak(l);
            }
        });
    }
    let ready = s.phase == PoolPhase::Ready;
    egui::CollapsingHeader::new(format!("Data: {:.0} % ({} workers)", 100.0 * s.progress(), s.workers)).default_open(!ready).show(ui, |ui| {
        ui.label(format!("Nuclear data: {}", s.data_source()));
        egui::Grid::new("core_jobs").num_columns(4).striped(true).show(ui, |ui| {
            for (j, l) in s.labels.iter().enumerate() {
                ui.label(l);
                ui.label(format!("w{}", s.owner[j] + 1));
                ui.label(match (s.secs[j], s.started[j]) {
                    (Some(t), _) => format!("{t:.1} s"),
                    (None, true) => "…".into(),
                    (None, false) => "queued".into(),
                });
                ui.label(match &s.sources[j] {
                    Some(dhoby_ghaut::web_demo::data_cache::Source::Cached { .. }) => "cached",
                    Some(_) => "processed",
                    None => "",
                });
                ui.end_row();
            }
        });
        for (w, a) in s.assembled.iter().enumerate() {
            if let Some(a) = a {
                ui.label(format!(
                    "worker {}: tapes {:.1} s, nuclides {:.1} s, core {:.1} s, majorant {:.1} s, memory {:.0} MB",
                    w + 1,
                    a.tapes_s,
                    a.nuclides_s,
                    a.geometry_s,
                    a.majorant_s,
                    a.memory_mb
                ));
            }
        }
    });
    if !ready {
        return None;
    }
    // Going from the moment it starts, including while the initial source is
    // sampled: a second Start in that window queued generation 0 twice. A
    // failed run is not going (gh:#817).
    let running = s.run.as_ref().is_some_and(|r| r.going());
    // Held to the demo's caps (keff::MAX_PARTICLES, MAX_GENERATIONS in all;
    // 2026-10-15, were 20 000 and 50 + 200 here).
    egui::Grid::new("core_cfg").num_columns(2).show(ui, |ui| {
        ui.label("neutrons per generation");
        ui.add(egui::DragValue::new(&mut cfg.n_particles).range(100..=MAX_PARTICLES).speed(100));
        ui.end_row();
        ui.label("inactive generations");
        let hi = MAX_GENERATIONS - cfg.n_active.max(MIN_ACTIVE);
        ui.add(egui::DragValue::new(&mut cfg.n_inactive).range(0..=hi));
        ui.end_row();
        ui.label("active generations");
        let hi = MAX_GENERATIONS - cfg.n_inactive.min(MAX_GENERATIONS - MIN_ACTIVE);
        ui.add(egui::DragValue::new(&mut cfg.n_active).range(MIN_ACTIVE..=hi));
        ui.end_row();
        ui.label("seed");
        ui.add(egui::DragValue::new(&mut cfg.seed));
        ui.end_row();
    });
    *cfg = clamp_to_caps(*cfg, 0);
    if cfg.seed != RECORD_SEED {
        ui.weak(format!("The record used seed {RECORD_SEED}."));
    }
    // Repeat the record (gh:#876 item 5): same settings and seed.
    ui.add_enabled_ui(!running, |ui| {
        ui.horizontal_wrapped(|ui| {
            if ui.button("Defaults").clicked() {
                *cfg = DEFAULT_RUN;
            }
            if ui.button(format!("Recorded settings ({})", crate::keff::settings_line(&RECORD_RUN))).clicked() {
                *cfg = RECORD_RUN;
            }
        });
    });
    let page = dhoby_ghaut::web_demo::platform::page_url();
    crate::keff::settings_link_row(ui, &crate::keff::settings_link(&page, "htr10", "watch", Some("core"), cfg));
    ui.weak(crate::keff::POOL_TIP);
    let mut action = None;
    ui.horizontal(|ui| {
        if !running && ui.button("▶ Start").clicked() {
            action = Some(PoolAction::Start);
        }
        if let Some(r) = s.run.as_ref().filter(|_| running) {
            let paused = r.paused;
            if ui
                .button(if paused { "▶ Resume" } else { "⏸ Pause" })
                .clicked()
            {
                s.set_paused(!paused);
            }
            if ui.button("■ Stop").clicked() {
                s.stop_run();
            }
        }
    });
    match s.run.as_ref().filter(|_| running) {
        Some(run) => {
            if let Some(l) = run_eta(run) {
                ui.label(l);
            }
        }
        None => {
            ui.weak(plan_line(cfg, None));
        }
    }
    if let Some(run) = &s.run {
        // What kind of run this is (a restart says how, gh:#817).
        let mut what = format!("{} ({})", run.describe(), crate::keff::settings_line(&run.cfg));
        if let RunOrigin::Restart { after, .. } = &run.origin {
            what.push_str(&format!(", after: {after}"));
        }
        ui.weak(what);
        for g in run.gens.iter().rev().take(8).rev() {
            let mean = g
                .gen
                .k_mean
                .map_or(String::new(), |(m, e)| format!("  {m:.5} ± {e:.5}"));
            let h = g
                .gen
                .entropy
                .map_or(String::new(), |h| format!("  H {h:.3}"));
            ui.monospace(format!(
                "{:>4}  {:.5}{h}{mean}  {:.1} s",
                g.gen.index + 1,
                g.gen.k,
                g.wall_s
            ));
        }
        if let Some(g) = run.gens.last() {
            // Busy time over wall time × workers: how well the chunks kept the pool fed.
            ui.weak(format!(
                "Run time {:.0} s; last generation {:.1} s wall, {:.1} s of worker time ({:.0} % of {} workers), {:.1} ms per history per worker.",
                now_s() - run.started_s,
                g.wall_s,
                g.busy_s,
                100.0 * g.busy_s / (g.wall_s * s.workers as f64).max(1e-9),
                s.workers,
                1e3 * g.busy_s / g.gen.counts.histories.max(1) as f64
            ));
        }
        let dem = layers as usize == super::random_bed::DEM_BED;
        if let (true, true, Some((m, e)), Some((rk, rs, _))) = (
            dem,
            s.rods == super::random_bed::WITHDRAWN,
            run.it.as_ref().and_then(|it| it.k_mean()),
            record_row(super::super::LADDER_N as usize),
        ) {
            ui.label(format!("This run, random bed: {m:.5} ± {e:.5}"));
            ui.label(dem_record_line());
            ui.label(format!(
                "Lattice at the same ball count (N = 12, recorded): {rk:.6} ± {rs:.6}; random − lattice here = {:+.0} ± {:.0} pcm, not resolvable at this σ",
                (m - rk) * 1e5,
                (e * e + rs * rs).sqrt() * 1e5
            ));
        }
        if let (false, true, Some((m, e)), Some((rk, rs, rmc))) = (
            dem,
            s.rods == super::random_bed::WITHDRAWN,
            run.it.as_ref().and_then(|it| it.k_mean()),
            record_row(layers as usize),
        ) {
            ui.label(format!("This run: {m:.5} ± {e:.5}"));
            ui.label(format!("Recorded (10 000 × [5 + 135], 2026-10-07): {rk:.6} ± {rs:.6}; you − record = {:+.0} ± {:.0} pcm", (m - rk) * 1e5, (e * e + rs * rs).sqrt() * 1e5));
            ui.label(format!("RMC (Li, Yu & Wei 2014) at equal ball count: {rmc:.6}; you − RMC = {:+.0} ± {:.0} pcm", (m - rmc) * 1e5, e * 1e5));
            ui.weak("Your ± is the spread of this run's active generations, a within-run σ that understates the true one.");
        }
    }
    action
}

/// "What this is, and is not".
pub const NOTES: &[&str] = &[
    "The model is the one the recorded k-vs-height runs used: nee_soon's assemble_explicit_triso(14, N, 0), the material set of the record, and every nuclide of Htr10DataConfig::default() (ENDF/B-VIII.0 at 300.15 K, tolerance 0.001, URR and DBRC on, every bound thermal law, real Ni/Fe rod steel). Nothing is re-modelled for the browser.",
    "Delta tracking in the bed, surface tracking in the reflector, as the recorded runs; the power iteration is the record's (run_keff_csg_par), split by generation over the workers. The answer does not depend on the number of workers, bit for bit.",
    "The recorded tracks the view opens on were baked natively (2026-10-07) from the record's model: every zone at 27 °C, graphite and SiC laws from the tapes. Since gh:#1142 the live run's model makes every law by LEAPR at its zone's temperature, so its histories differ from the recorded ones even at 27 °C; the browser's own run also differs in the last bits (its maths library is not the platform's).",
    "A live run of 1000 neutrons per generation has a σ of several hundred pcm, against about 100 pcm for the record's 1.35 million active histories. It shows the core is near critical at N = 12; it does not test the record.",
    "Pebbles on Şeker & Çolak (2003)'s regular lattice, not the reactor's random bed; no k exists here for a random (DEM) bed.",
    "Education and research only. Not for reactor operation, licensing or safety decisions.",
];

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::htr10::core::{AssemblyReport, CoreEv};

    /// **The temperature sliders** (gh:#1142): unchanged sliders apply
    /// nothing; a moved one gives the new temperatures (°C to K exactly); a
    /// value outside 20..=1800 °C is refused by name; the TRISO warning
    /// names the zones above 1600 °C only; and the screen starts its pool
    /// at the applied temperatures, whose jobs grow when zones differ.
    #[test]
    fn the_temperature_sliders_apply_check_and_warn() {
        let rec = ZoneTemperatures::record();
        let same = Htr10Zone::ALL.map(|z| celsius(rec.get(z)));
        assert_eq!(temperatures_to_apply(&rec, &same).expect("in range"), None);
        let mut s = same;
        s[0] = 900.0;
        s[2] = 500.0;
        let want = temperatures_to_apply(&rec, &s).expect("in range").expect("changed");
        assert!((want.kelvin(Htr10Zone::FuelPebble) - 1173.15).abs() < 1e-9);
        assert!((want.kelvin(Htr10Zone::Coolant) - 773.15).abs() < 1e-9);
        assert_eq!(want.kelvin(Htr10Zone::SideReflector).to_bits(), rec.kelvin(Htr10Zone::SideReflector).to_bits());
        // A slider snapped to a whole 27 °C keeps the record's 300.15 K to
        // the bit, and the record's 27 °C is a whole slider step.
        let mut snapped = s;
        snapped[3] = 27.0;
        let w = temperatures_to_apply(&rec, &snapped).expect("in range").expect("changed");
        assert_eq!(w.kelvin(Htr10Zone::TopReflector).to_bits(), rec.kelvin(Htr10Zone::TopReflector).to_bits());
        assert!((celsius(rec.get(Htr10Zone::FuelPebble)) - 27.0).abs() < 1e-9);
        s[7] = 1850.0;
        assert!(temperatures_to_apply(&rec, &s).unwrap_err().contains("Boronated bricks"));
        assert_eq!(triso_warning(&same), None);
        let mut hot = same;
        hot[0] = 1650.0;
        hot[1] = 1600.0;
        let w = triso_warning(&hot).expect("warned");
        assert!(w.contains("Fuel pebbles") && !w.contains("Moderator pebbles"), "{w}");
        assert!(super::super::jobs_at(&want).expect("jobs").len() > super::super::jobs().expect("jobs").len());
        let info = crate::table::Rung::Htr10.raster_info().expect("raster");
        let screen = CoreScreen::new(info, 1.0, false);
        assert_eq!(screen.temps, rec);
        assert_eq!(screen.temps_c, same);
        // The URL: `all` first whatever its place, then each zone; anything
        // unreadable or out of range leaves the record's.
        let q = |v: &str| vec![("temps".to_string(), v.to_string())];
        let t = temperatures_from_query(&q("fuel:900,all:600"));
        assert!((t.kelvin(Htr10Zone::FuelPebble) - 1173.15).abs() < 1e-9);
        assert!((t.kelvin(Htr10Zone::BoronatedBricks) - 873.15).abs() < 1e-9);
        for bad in ["fuel:hot", "core:900", "fuel:1900", "fuel900"] {
            assert_eq!(temperatures_from_query(&q(bad)), rec, "{bad}");
        }
        assert_eq!(temperatures_from_query(&[]), rec);
    }

    /// The picture keeps only the bed (with its pebbles) and the rods; every
    /// other line is the bottom panel's, and it does not repeat those two
    /// (maintainer 2026-10-26).
    #[test]
    fn the_picture_keeps_only_the_bed_and_the_rods() {
        let info = crate::table::Rung::Htr10.raster_info().expect("raster");
        let mut screen = CoreScreen::new(info, 1.0, false);
        assert_eq!(screen.picture_lines(), ["Bed: Şeker lattice, N = 12 · 16 681 pebbles".to_string(), "Control rods: all ten withdrawn (the record's state)".to_string()]);
        let status = screen.status();
        assert!(status[0].0.starts_with("RECORDED"), "{status:?}");
        assert!(status.iter().all(|(t, _)| !t.starts_with("Bed: ") && !t.starts_with("Control rods: ")));
        assert!(screen.console().is_empty(), "no run, no console");
        screen.layers = 20.0;
        assert!(screen.picture_lines()[0].ends_with("27 321 pebbles"));
    }

    /// The pool follows the reader's rods only between runs, once they
    /// rest, and never while a sweep drives it (gh:#1025).
    #[test]
    fn the_pool_follows_the_rods_once_they_rest() {
        let mut s = PoolState::new(1, Vec::new(), Vec::new(), 12, 0.0);
        let want = [0.5; 10];
        assert!(!rods_resync_due(&s, &want, false, 0.0, 10.0), "data not ready");
        s.phase = PoolPhase::Ready;
        assert!(rods_resync_due(&s, &want, false, 0.0, 10.0));
        assert!(!rods_resync_due(&s, &want, false, 9.5, 10.0), "still moving");
        assert!(!rods_resync_due(&s, &want, true, 0.0, 10.0), "a sweep drives the pool");
        assert!(!rods_resync_due(&s, &s.rods.clone(), false, 0.0, 10.0), "already there");
    }

    /// After a failed run the screen offers both restarts (on the live
    /// workers, and on one), says which workers stopped, and draws the
    /// failure on the main view; "Reprocess" replaces the pool (gh:#817).
    #[test]
    fn a_failed_run_offers_both_restarts_and_names_the_dead() {
        let ctx = egui::Context::default();
        let plan = Plan {
            workers: 3,
            reason: "test".into(),
        };
        let mut p = CorePool::start(&ctx, plan, 12, &ZoneTemperatures::record()).expect("pool");
        // Data ready without processing anything (the protocol is scripted).
        let n = p.state.labels.len();
        for j in 0..n {
            let ev = CoreEv::Product {
                job: j,
                secs: 0.0,
                data: dhoby_ghaut::web_demo::link::Floats::from_vec(vec![0.0]),
                source: dhoby_ghaut::web_demo::data_cache::Source::Processed,
            };
            p.state.on_event(0, ev, 0.0);
        }
        while !p.state.outbox(usize::MAX).is_empty() {}
        for w in 0..3 {
            let r = AssemblyReport {
                layers: 12,
                tapes_s: 0.0,
                nuclides_s: 0.0,
                geometry_s: 0.0,
                majorant_s: 0.0,
                memory_mb: 1.0,
            };
            p.state.on_event(w, CoreEv::Assembled(r), 0.0);
        }
        assert_eq!(p.state.phase, PoolPhase::Ready);
        assert!(!restart_wanted(&p.state));
        p.state.start_run(DEFAULT_RUN, 0.0).expect("start");
        let failed = CoreEv::Failed {
            message: "core worker: boom".into(),
            epoch: 1,
        };
        p.state.on_event(0, failed, 0.0);
        assert!(restart_wanted(&p.state));
        p.state.on_error(2, "physics worker: out of memory".into());
        let [(k0, l0), (k1, l1)] = restart_labels(&p.state);
        assert_eq!((k0, k1), (RestartKind::Parallel, RestartKind::Serial));
        assert!(l0.contains("in parallel (2 workers)") && l1.contains("serially (1 worker)"));
        assert!(dead_line(&p.state).contains("2 of 3 workers left") && dead_line(&p.state).contains("worker 3"));
        let lines: Vec<String> = status_lines(&p).into_iter().map(|(l, _)| l).collect();
        assert!(lines.iter().any(|l| l.contains("boom")), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("2 of 3 workers left")), "{lines:?}");
        restart(&mut p.state, RestartKind::Serial);
        assert!(!restart_wanted(&p.state));
        let lines: Vec<String> = status_lines(&p).into_iter().map(|(l, _)| l).collect();
        assert!(lines.iter().any(|l| l.contains("serial restart")), "{lines:?}");
        // A refused restart only logs.
        restart(&mut p.state, RestartKind::Parallel);
        p.terminate();
        // Reprocess: the old pool is stopped and a new one started.
        let info = crate::table::Rung::Htr10.raster_info().expect("raster");
        let mut screen = CoreScreen::new(info, 1.0, false);
        screen.pool = Some(p);
        screen.reprocess(&ctx);
        let fresh = screen.pool.as_ref().expect("a new pool");
        assert_eq!(fresh.state.phase, PoolPhase::Processing);
        assert!(fresh.state.log.is_empty());
        if let Some(p) = screen.pool.take() {
            p.terminate();
        }
    }

    /// Tracks survive the file, the record's rows are read, the view frame
    /// swaps y and z, and the recorded tracks committed with the demo decode.
    #[test]
    fn tracks_round_trip_and_the_record_is_read() {
        let s = |x: f64, e: TrackEvent| TrackState {
            r: Position::new(x, 2.0 * x, 3.0 * x),
            u: Direction::new(0.0, 0.0, 1.0),
            energy: 1.0e6 / (1.0 + x),
            time: x,
            weight: 1.0,
            cell: 3,
            material: Some(1),
            event: e,
        };
        let t = Track {
            states: vec![
                s(0.0, TrackEvent::Born),
                s(1.0, TrackEvent::Scatter),
                s(2.0, TrackEvent::Fission),
            ],
            dropped_states: 0,
        };
        let back = decode_tracks(&encode_tracks(&[(2, 5, t.clone())])).expect("decode");
        assert_eq!(back, vec![(2, 5, t.clone())]);
        let h = to_view(1, t);
        assert_eq!(h.track.states[1].r, Position::new(1.0, 3.0, 2.0));
        assert_eq!(h.outcome, Some(TrackEvent::Fission));
        let (k, sigma, rmc) = record_row(12).expect("N = 12");
        assert_eq!((k, sigma, rmc), (0.995125, 0.001055, 0.999419));
        assert!(record_row(9).is_none());
        // The live view quotes the native record the beds view quotes
        // (`beds::tests::the_recorded_k_are_the_records` pins it to the CSV).
        let line = dem_record_line();
        assert_eq!(
            DEM_NATIVE_RECORD.map(|r| (r.0, r.1)),
            Some((crate::beds::RANDOM_K.k, crate::beds::RANDOM_K.sigma))
        );
        assert!(
            line.contains("0.989293 ± 0.000962") && line.contains("-583 ± 143 pcm"),
            "{line}"
        );
        assert!(DEM_CAVEAT.contains("cannot resolve"));
        assert!(forced_workers().is_none() && !forced_dem());
        let recorded = decode_tracks(RECORDED_TRACKS).expect("recorded tracks");
        assert!(!recorded.is_empty());
        assert!(recorded
            .iter()
            .all(|(_, _, t)| t.states.len() >= 2 && t.states[0].event == TrackEvent::Born));
    }
}
