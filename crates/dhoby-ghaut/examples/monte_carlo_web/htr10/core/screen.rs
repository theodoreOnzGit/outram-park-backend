//! **The core view** of the `htr10` rung (gh:#786): neutrons flying through
//! the whole HTR-10 model, over a live x-z slice of it, and the live
//! low-statistics k_eff beside the record.
//!
//! It opens on **recorded** tracks (baked natively by [`super::bake`] from the
//! same model, code and seed, and labelled so), which play at once. "Run live
//! in this browser" starts the worker pool ([`super::pool`]); once the data
//! are processed, the tracks come from the run itself: the first
//! [`super::TRACED_PER_GENERATION`] histories of every generation.

use super::pool::{CorePool, GenSummary, Note, PoolPhase, PoolState, RestartKind, RunOrigin};
use super::{DEFAULT_RUN, RECORD_SEED};
use crate::anim::{animated_speed, draw_track, energy_bar, fmt_speed, legend_markers, Anim};
use crate::history::History;
use crate::keff::KeffConfig;
use crate::raster::{Preset, RasterInfo, Slicer};
use dhoby_ghaut::web_demo::platform::now_s;
use dhoby_ghaut::web_demo::pool::{plan, Device, Plan};
use egui::{Color32, Pos2, Rect, Stroke, Vec2};
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::physics::track_output::{Track, TrackEvent, TrackState};
use std::collections::VecDeque;

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
pub const DEM_CAVEAT: &str = "Random bed: a LOW-STATISTICS DEMO. At 1000 neutrons per generation σ is about 1000 pcm, so it cannot resolve a lattice-against-random difference (any likely one is a few hundred pcm). One pour, one cut; pour-to-pour scatter is not measured.";

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

    /// Start the workers.
    pub fn start_pool(&mut self, ctx: &egui::Context) {
        if self.pool.is_none() {
            match CorePool::start(ctx, self.plan.clone(), self.layers as usize) {
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
        if self.autostart && self.pool.is_none() {
            self.start_pool(ctx);
        }
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
            if self.autostart && p.state.phase == PoolPhase::Ready && p.state.run.is_none() {
                let _ = p.state.start_run(self.cfg, now_s());
            }
            // A pool failure is shown once, by the pool's status line.
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
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
        self.slicer.param = self.layers;
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
        // Status on the main view (it shows with the panel folded).
        let mut lines: Vec<(String, Color32)> = Vec::new();
        let label = if self.live {
            "LIVE: neutrons of the k_eff run in this tab's workers"
        } else {
            "RECORDED: tracks baked natively from the same model, code and seed (2026-10-07)"
        };
        lines.push((
            label.into(),
            if self.live {
                Color32::from_rgb(130, 230, 140)
            } else {
                Color32::from_rgb(250, 200, 80)
            },
        ));
        if let Some(p) = &self.pool {
            lines.extend(status_lines(p));
        }
        if let Some(e) = &self.error {
            lines.push((format!("FAILED: {e}"), Color32::from_rgb(255, 110, 110)));
        }
        let mut y = rect.top() + 52.0;
        for (t, c) in lines {
            let g = painter.layout(
                t,
                egui::FontId::proportional(12.5),
                c,
                (rect.width() - 24.0).max(100.0),
            );
            let bg = Rect::from_min_size(
                Pos2::new(rect.left() + 8.0, y - 2.0),
                g.size() + Vec2::new(8.0, 4.0),
            );
            painter.rect_filled(bg, 4.0, Color32::from_rgba_unmultiplied(14, 16, 20, 200));
            let h = g.size().y;
            painter.galley(Pos2::new(rect.left() + 12.0, y), g, c);
            y += h + 6.0;
        }
        // The restart buttons on the main view too, so they show with the
        // panel folded on a phone (gh:#817; the mobile-first rule's status
        // on the main view).
        let w = (rect.width() - 24.0).clamp(120.0, 340.0);
        let mut reprocess = false;
        if let Some(p) = self.pool.as_mut() {
            if restart_wanted(&p.state) {
                for (kind, label) in restart_labels(&p.state) {
                    let b = Rect::from_min_size(Pos2::new(rect.left() + 8.0, y), Vec2::new(w, 36.0));
                    if ui.put(b, egui::Button::new(label)).clicked() {
                        restart(&mut p.state, kind);
                    }
                    y += 42.0;
                }
            } else if matches!(p.state.phase, PoolPhase::Failed(_)) {
                let b = Rect::from_min_size(Pos2::new(rect.left() + 8.0, y), Vec2::new(w, 36.0));
                reprocess = ui.put(b, egui::Button::new("⟳ Reprocess the data (new workers)")).clicked();
            }
        }
        if reprocess {
            self.reprocess(ui.ctx());
        }
        // The k plot along the bottom once a run has generations.
        if let Some(run) = self.pool.as_ref().and_then(|p| p.state.run.as_ref()) {
            if !run.gens.is_empty() {
                let h = (rect.height() * 0.24).clamp(80.0, 170.0);
                let plot = Rect::from_min_max(
                    Pos2::new(rect.left() + 12.0, rect.bottom() - h - 96.0),
                    Pos2::new(rect.right() - 12.0, rect.bottom() - 96.0),
                );
                let dem = self.layers as usize == super::random_bed::DEM_BED;
                // On the random bed the lattice's N = 12 record is the like-for-like
                // ball count, labelled as the lattice's.
                let n = if dem { super::super::LADDER_N as usize } else { self.layers as usize };
                draw_k(
                    painter,
                    plot,
                    &run.gens,
                    run.cfg,
                    record_row(n),
                    if dem { "lattice recorded" } else { "recorded" },
                );
            }
        }
    }

    /// The side panel.
    pub fn panel(&mut self, ui: &mut egui::Ui) {
        ui.label("Neutrons in the whole HTR-10 core: reflector, rods, cavity, the pebble bed, and inside each fuel pebble the TRISO particles. Zoom in (+) on the bed to see the pebbles and particles the neutrons cross.");
        ui.horizontal(|ui| {
            if ui
                .button(if self.playing {
                    "⏸ Pause"
                } else {
                    "▶ Play"
                })
                .clicked()
            {
                self.playing = !self.playing;
            }
            if ui.button("Next neutron").clicked() {
                if let Some(a) = self.current.as_mut() {
                    a.shown_cm = a.total();
                }
            }
        });
        ui.add(
            egui::Slider::new(&mut self.speed, 0.0001..=20_000.0)
                .logarithmic(true)
                .text("speed at 1 eV (cm/s)")
                .custom_formatter(|v, _| fmt_speed(v)),
        );
        ui.weak(format!("= {} at 0.0253 eV (thermal). v is proportional to √E: a fission neutron crosses the core in a moment, a thermal one crawls.", fmt_speed(animated_speed(0.0253, self.speed))));
        legend_markers(ui, true);
        energy_bar(ui);
        ui.separator();
        ui.strong("Live in this browser");
        let ctx = ui.ctx().clone();
        match self.pool.as_mut() {
            None => {
                ui.label(format!(
                    "Processes all 38 nuclides of the recorded model (31 ENDF/B-VIII.0 tapes, 5 thermal laws, about 35 MB to download) in a pool of Web Workers, each nuclide once, then runs a low-statistics k_eff of the whole core. Pool: {}.",
                    self.plan.reason
                ));
                if self.forced.is_none() {
                    ui.weak("Add ?workers=N to the address to choose the pool size.");
                }
                if ui
                    .button(format!("▶ Run live ({} workers)", self.plan.workers))
                    .clicked()
                {
                    self.start_pool(&ctx);
                }
            }
            Some(p) => {
                if let Some(PoolAction::Reprocess) = pool_panel(ui, p, &mut self.cfg, &mut self.layers) {
                    self.reprocess(&ctx);
                }
            }
        }
        ui.separator();
        egui::CollapsingHeader::new("The whole-core view: what it is, and is not")
            .default_open(false)
            .show(ui, |ui| {
                for l in NOTES {
                    ui.label(format!("• {l}"));
                }
            });
    }
}

/// What the view says about the pool, one line each.
fn status_lines(p: &CorePool) -> Vec<(String, Color32)> {
    let s = &p.state;
    let white = Color32::from_rgb(220, 226, 236);
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
            if s.layers == super::random_bed::DEM_BED {
                v.push((DEM_CAVEAT.to_string(), Color32::from_rgb(250, 200, 80)));
                v.push((dem_record_line(), white));
            }
            let amber = Color32::from_rgb(250, 200, 80);
            if s.live_workers() < s.workers {
                v.push((dead_line(s), amber));
            }
            if let Some(run) = &s.run {
                let total = run.cfg.n_inactive + run.cfg.n_active;
                if let RunOrigin::Restart { .. } = run.origin {
                    v.push((format!("This is {}.", run.describe()), amber));
                }
                if let Some(e) = &run.failed {
                    v.push((format!("⚠ {e}"), Color32::from_rgb(255, 110, 110)));
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
                    }
                }
            }
        }
        PoolPhase::Failed(e) => v.push((
            format!("⚠ {e}"),
            Color32::from_rgb(255, 110, 110),
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
}

/// The pool's part of the side panel. GUI drawing, not reached by a test
/// (the test rule's exception); its decisions are tested through
/// `restart_wanted`, `restart_labels`, `restart` and `dead_line`.
fn pool_panel(
    ui: &mut egui::Ui,
    p: &mut CorePool,
    cfg: &mut KeffConfig,
    layers: &mut f64,
) -> Option<PoolAction> {
    let s = &mut p.state;
    ui.label(format!("Pool: {}", p.plan.reason));
    let red = Color32::from_rgb(255, 110, 110);
    let amber = Color32::from_rgb(250, 200, 80);
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
    ui.add_enabled_ui(!running, |ui| {
        // The bed (gh:#786, #787): the record's lattice at N layers, or the
        // DEM random bed cut to the lattice's ball count at N = 12.
        let dem_code = super::random_bed::DEM_BED as f64;
        let mut want = *layers;
        ui.horizontal_wrapped(|ui| {
            ui.label("Bed:");
            if ui.selectable_label(*layers != dem_code, "Şeker lattice (the record's)").clicked() && *layers == dem_code {
                want = super::super::LADDER_N;
            }
            if ui.selectable_label(*layers == dem_code, "DEM random bed").clicked() {
                want = dem_code;
            }
        });
        if want == dem_code {
            ui.weak(super::random_bed::label(super::random_bed::DEM_BED));
        } else {
            let mut n = want;
            ui.add(egui::Slider::new(&mut n, 10.0..=20.0).step_by(1.0).fixed_decimals(0).text("layers N"));
            want = n;
        }
        if want != *layers && s.set_layers(want as usize).is_ok() {
            *layers = want;
        }
    });
    if *layers as usize == super::random_bed::DEM_BED {
        ui.colored_label(Color32::from_rgb(250, 200, 80), DEM_CAVEAT);
        ui.label(dem_record_line());
    }
    ui.strong("Run k_eff (the whole core)");
    egui::Grid::new("core_cfg").num_columns(2).show(ui, |ui| {
        ui.label("neutrons per generation");
        ui.add(egui::DragValue::new(&mut cfg.n_particles).range(100..=20_000));
        ui.end_row();
        ui.label("inactive generations");
        ui.add(egui::DragValue::new(&mut cfg.n_inactive).range(0..=50));
        ui.end_row();
        ui.label("active generations");
        ui.add(egui::DragValue::new(&mut cfg.n_active).range(2..=200));
        ui.end_row();
        ui.label("seed");
        ui.add(egui::DragValue::new(&mut cfg.seed));
        ui.end_row();
    });
    if cfg.seed != RECORD_SEED {
        ui.weak(format!("The record used seed {RECORD_SEED}."));
    }
    ui.horizontal(|ui| {
        if !running && ui.button("▶ Start").clicked() {
            let _ = s.start_run(*cfg, now_s());
        }
        if let Some(r) = s.run.as_ref().filter(|_| running) {
            let paused = r.paused;
            if ui
                .button(if paused { "▶ Resume" } else { "⏸ Pause" })
                .clicked()
            {
                s.set_paused(!paused);
            }
        }
    });
    if let Some(run) = &s.run {
        // What kind of run this is (a restart says how, gh:#817).
        let mut what = run.describe();
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
        let dem = *layers as usize == super::random_bed::DEM_BED;
        if let (true, Some((m, e)), Some((rk, rs, _))) = (
            dem,
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
        if let (false, Some((m, e)), Some((rk, rs, rmc))) = (
            dem,
            run.it.as_ref().and_then(|it| it.k_mean()),
            record_row(*layers as usize),
        ) {
            ui.label(format!("This run: {m:.5} ± {e:.5}"));
            ui.label(format!("Recorded (10 000 × [5 + 135], 2026-10-07): {rk:.6} ± {rs:.6}; you − record = {:+.0} ± {:.0} pcm", (m - rk) * 1e5, (e * e + rs * rs).sqrt() * 1e5));
            ui.label(format!("RMC (Li, Yu & Wei 2014) at equal ball count: {rmc:.6}; you − RMC = {:+.0} ± {:.0} pcm", (m - rmc) * 1e5, e * 1e5));
            ui.weak("Your ± is the spread of this run's active generations, a within-run σ that understates the true one.");
        }
    }
    None
}

/// k by generation, the running mean, the record and RMC.
fn draw_k(
    painter: &egui::Painter,
    rect: Rect,
    gens: &[GenSummary],
    cfg: KeffConfig,
    record: Option<(f64, f64, f64)>,
    record_label: &str,
) {
    painter.rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(14, 16, 20, 215));
    let total = (cfg.n_inactive + cfg.n_active).max(2) as f64;
    let mut ks: Vec<f64> = gens.iter().map(|g| g.gen.k).collect();
    if let Some((k, _, rmc)) = record {
        ks.extend([k, rmc]);
    }
    let lo = ks.iter().copied().fold(f64::INFINITY, f64::min) - 0.01;
    let hi = ks.iter().copied().fold(f64::NEG_INFINITY, f64::max) + 0.01;
    let p = |i: f64, k: f64| {
        Pos2::new(
            rect.left() + 8.0 + (rect.width() - 16.0) * (i / (total - 1.0)) as f32,
            rect.bottom() - 6.0 - (rect.height() - 22.0) * ((k - lo) / (hi - lo)) as f32,
        )
    };
    // RMC labelled at the left end, the record at the right: they are close.
    let hline = |k: f64, c: Color32, label: &str, right: bool| {
        painter.line_segment([p(0.0, k), p(total - 1.0, k)], Stroke::new(1.5, c));
        let (at, align) = if right {
            (
                p(total - 1.0, k) + Vec2::new(-2.0, 2.0),
                egui::Align2::RIGHT_TOP,
            )
        } else {
            (p(0.0, k) + Vec2::new(2.0, -2.0), egui::Align2::LEFT_BOTTOM)
        };
        painter.text(at, align, label, egui::FontId::proportional(10.5), c);
    };
    if let Some((k, _, rmc)) = record {
        hline(
            rmc,
            Color32::from_rgb(235, 235, 235),
            &format!("RMC {rmc:.4}"),
            false,
        );
        hline(
            k,
            Color32::from_rgb(120, 170, 255),
            &format!("{record_label} {k:.4}"),
            true,
        );
    }
    let x0 = p(cfg.n_inactive as f64 - 0.5, lo).x;
    painter.line_segment(
        [Pos2::new(x0, rect.top()), Pos2::new(x0, rect.bottom())],
        Stroke::new(1.0, Color32::from_gray(90)),
    );
    for w in gens.windows(2) {
        painter.line_segment(
            [
                p(w[0].gen.index as f64, w[0].gen.k),
                p(w[1].gen.index as f64, w[1].gen.k),
            ],
            Stroke::new(1.0, Color32::from_rgb(160, 160, 170)),
        );
    }
    for g in gens {
        painter.circle_filled(
            p(g.gen.index as f64, g.gen.k),
            2.5,
            Color32::from_rgb(160, 160, 170),
        );
        if let Some((m, e)) = g.gen.k_mean {
            let c = Color32::from_rgb(255, 200, 80);
            painter.line_segment(
                [p(g.gen.index as f64, m - e), p(g.gen.index as f64, m + e)],
                Stroke::new(1.0, c),
            );
            painter.circle_filled(p(g.gen.index as f64, m), 2.0, c);
        }
    }
    painter.text(
        rect.left_top() + Vec2::new(8.0, 4.0),
        egui::Align2::LEFT_TOP,
        "k by generation (grey), running mean ± σ (amber)",
        egui::FontId::proportional(11.0),
        Color32::from_gray(200),
    );
}

/// "What this is, and is not".
pub const NOTES: &[&str] = &[
    "The model is the one the recorded k-vs-height runs used: nee_soon's assemble_explicit_triso(14, N, 0), the material set of the record, and every nuclide of Htr10DataConfig::default() (ENDF/B-VIII.0 at 300.15 K, tolerance 0.001, URR and DBRC on, every bound thermal law, real Ni/Fe rod steel). Nothing is re-modelled for the browser.",
    "Delta tracking in the bed, surface tracking in the reflector, as the recorded runs; the power iteration is the record's (run_keff_csg_par), split by generation over the workers. The answer does not depend on the number of workers, bit for bit.",
    "The recorded tracks the view opens on were baked natively from the same model, code and seed; the browser's own run can differ from them in the last bits (its maths library is not the platform's), and then in the histories.",
    "A live run of 1000 neutrons per generation has a σ of several hundred pcm, against about 100 pcm for the record's 1.35 million active histories. It shows the core is near critical at N = 12; it does not test the record.",
    "Pebbles on Şeker & Çolak (2003)'s regular lattice, not the reactor's random bed; no k exists here for a random (DEM) bed.",
    "Education and research only. Not for reactor operation, licensing or safety decisions.",
];

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::htr10::core::{AssemblyReport, CoreEv};

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
        let mut p = CorePool::start(&ctx, plan, 12).expect("pool");
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
