//! The `dhshort` rung: **the demo of the lesson's step 5 code walk**
//! (`dh_keff_vv.rs::main` → `DhUniverse::keff` → `run_keff_delta_in`, gh:#785).
//! The same FHR unit cell under each `DhTreatment`:
//!
//! - **the demo** (Watch, `view=demo`): what geometry and material each
//!   treatment answers `material_at` with (a live slice, drawn in the worker
//!   from the universe `DhUniverse::pebble` builds), beside the recorded k
//!   bias and speed-up of each, with dates and re-measurement status
//!   ([`record`]). Needs no nuclear data, so that view loads none.
//! - **k∞ (true MC)** (`view=pitch`, the shared `k_inf` view): a live
//!   low-statistics run of one treatment at a time, `DhUniverse::power_iteration`
//!   (which is `DhUniverse::keff` stepped one generation per worker message,
//!   pinned bit for bit), on `dh_keff_vv`'s data processed in the tab
//!   ([`data`]). SCLS is not offered: about 24 times delta tracking's cost.

pub mod data;
pub mod model;
pub mod record;
pub mod ui;

use crate::anim::Spectrum;
use crate::engine::Tier;
use crate::history::History;
use crate::keff::{Generation, KeffConfig, KinfCase, KinfGeneration, LineStyle, RecordedCurve};
use crate::raster::RasterReq;
use crate::rungs::{LoadedRung, McRung, RungBuilder, RungInfo};
use crate::walkdemo::WalkKind;
use dhoby_ghaut::web_demo::platform::now_s;
use dhoby_ghaut::web_demo::view::View;
use egui::Rect;
use outram_mc_libs::dh_universe::{DhPowerIteration, DhUniverse};
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::KeffSettings;

/// The marker type `rung_table!` names.
pub struct DhShort;

/// The treatments the live run offers (indices into [`model::TREATMENTS`]):
/// not SCLS, whose cost is about 24 times delta tracking's (#582's pilot).
pub const LIVE: [usize; 5] = [0, 1, 3, 4, 5];
/// Their labels on the k∞ view's buttons, short enough for a phone's row.
pub const LIVE_LABELS: [&str; 5] = ["delta", "CLS", "smear", "RPT", "CLS-k"];

impl McRung for DhShort {
    const INFO: RungInfo = RungInfo {
        name: "dhshort",
        title: "TRISO step 5: the DH shortcuts, side by side",
        lesson: "tutorials/monte-carlo/triso.html",
        spectrum: Spectrum::Thermal,
    };
    type Builder = Builder;

    /// Every tape the live run processes (published once by
    /// `--prepare-web-data`); the demo view loads none ([`McRung::jobs_for`]).
    fn jobs() -> &'static [(&'static str, &'static str)] {
        &data::JOBS
    }
    fn jobs_for(tier: Tier) -> &'static [(&'static str, &'static str)] {
        match tier {
            Tier::Loose => &[],
            Tier::Exact => &data::JOBS,
        }
    }
    /// The demo view needs no data (the loose tier loads none); the k∞ run
    /// processes at NJOY's tolerance, like the record.
    fn tier(requested: Tier) -> Tier {
        requested
    }
    /// Rough native seconds per tape at tolerance 0.001 (progress bar only).
    fn job_weights(tier: Tier) -> Vec<f64> {
        match tier {
            Tier::Loose => Vec::new(),
            Tier::Exact => vec![70.0, 70.0, 10.0, 3.0, 3.0, 2.0, 2.0, 2.0, 4.0, 8.0],
        }
    }
    fn half_extent() -> f64 {
        3.1
    }
    /// The picture is the live slice ([`ui`]); nothing static.
    fn draw(_painter: &egui::Painter, _rect: Rect, _view: &View) {}
    fn notes() -> &'static [&'static str] {
        &[
            "Geometry: dh_keff_vv.rs's FHR unit cell (1.9 cm fuel zone at a particle packing fraction of 0.30, 2.0 cm pebble, FLiBe to a reflective sphere at 3.0 cm), built by DhUniverse::pebble under each treatment in this tab's worker, with the record's material table and packing seed. Every pixel is what DhUniverse::material_at answers there: the same call the delta-tracked power iteration makes.",
            "CLS and SCLS store no geometry. Their material_at SAMPLES, so their picture is drawn as one flight per pixel row (left to right) and a redraw gives a different picture. It is what one neutron would meet, not a map of the fuel.",
            "Recorded results: every record predates a change that could move it (URR and DBRC on by default from 2026-09-20; SCLS's wiring fixed on 2026-09-14), so all are marked re-measurement pending (#582). Quote ratios of speed, not seconds: the records ran on different hosts.",
            "k∞ (true MC): one treatment at a time, a real single-threaded power iteration in this tab's worker, on the record's ten ENDF/B-VIII.0 tapes and the crystalline-graphite law processed here at 600 K and NJOY's tolerance. Its k is today's code (URR and DBRC on), so it is not like for like with the 2026-09-14 record drawn beside it. SCLS is not offered: about 24 times delta tracking's cost.",
            "A bias that is not resolved means the run could not measure it, not that it is zero.",
            "Education and research only. Not for reactor operation, licensing or safety decisions.",
        ]
    }
    fn loading_note(tier: Tier) -> Option<&'static str> {
        (tier == Tier::Exact).then_some(
            "The live run processes the record's ten tapes at NJOY's tolerance 0.001 and 600 K, U-235 and U-238 the longest: several minutes in a browser. The demo view needs none.",
        )
    }
    fn has_tracks() -> bool {
        false
    }
    fn walk_demo() -> Option<WalkKind> {
        Some(WalkKind::Shortcuts)
    }
    /// One treatment at a time; defaults 800 × [15 + 40] (the 2026-09-18
    /// record's size), seed 1 (`dh_keff_vv`'s first draw).
    fn kinf_case() -> Option<KinfCase> {
        let first = &record::RECORDS[0];
        Some(KinfCase {
            title: "the FHR cell's k, one shortcut at a time",
            param: ("treatment", ""),
            range: (0.0, 5.0),
            default: 0.0,
            choices: LIVE.iter().zip(LIVE_LABELS).map(|(&t, l)| (t as f64, l)).collect(),
            marks: Vec::new(),
            cfg: KeffConfig { n_particles: 800, n_inactive: 15, n_active: 40, seed: 1, point_source: false, want_sites: false },
            curves: vec![RecordedCurve {
                label: format!("recorded {} ({}), before the URR/DBRC defaults; re-measurement pending #582", first.date, first.stats),
                style: LineStyle::Ours,
                points: first.arms.iter().filter(|a| LIVE.contains(&a.t)).map(|a| (a.t as f64, a.k, a.sigma)).collect(),
            }],
            notes: vec![
                "dh_keff_vv.rs's FHR unit cell, materials and seed; DhUniverse::power_iteration, which is DhUniverse::keff one generation per worker message (pinned bit for bit by power_iteration_steps_keff_bit_for_bit). Delta (Woodcock) tracking with the library's bounding majorant, ENDF/B-VIII.0 processed in this tab at 600 K and NJOY's tolerance 0.001, crystalline-graphite S(α,β) on the graphite carbon.",
                "Run delta first, then a shortcut: the difference is that shortcut's bias. At 800 neutrons × 40 active generations one arm's σ is roughly 800 pcm, so naive homogenisation's 4000-pcm bias shows and ring-RPT's few hundred does not.",
                "Your ± is the spread of one run's active generations, which understates the true σ. The dotted record predates the URR/DBRC defaults of 2026-09-20.",
            ],
        })
    }
}

/// The data, one tape at a time (none for the demo view).
pub struct Builder(data::DataBuilder);

impl RungBuilder for Builder {
    type Loaded = Loaded;
    fn new(tier: Tier) -> Self {
        Builder(data::DataBuilder::new(tier))
    }
    fn step(&mut self, bytes: &[u8], store: &mut crate::processed_cache::DataStore) -> Result<(), String> {
        self.0.step(bytes, store)
    }
    fn finish(self) -> Result<Loaded, String> {
        Ok(Loaded {
            nuclides: self.0.finish()?,
            ..Loaded::default()
        })
    }
}

/// A live run: treatment, its own freshly built universe (CLS samples, so a
/// run never shares a universe with the picture), the iteration, and its
/// generation count.
pub struct LiveRun {
    pub t: usize,
    pub universe: DhUniverse,
    pub it: DhPowerIteration,
    pub total: usize,
}

/// The worker side: each treatment's universe for the picture, built on first
/// use with how long it took; the data and the live run, if loaded.
#[derive(Default)]
pub struct Loaded {
    universes: Vec<Option<(DhUniverse, f64)>>,
    nuclides: Option<Vec<Nuclide>>,
    run: Option<LiveRun>,
}

/// Message codes of [`LoadedRung::walk`] for this rung.
pub const MSG_DESCRIBE: f64 = 0.0;

impl Loaded {
    /// Treatment `i`'s universe and its build time, s (built if needed).
    pub fn universe(&mut self, i: usize) -> Result<(&DhUniverse, f64), String> {
        if self.universes.len() < model::TREATMENTS.len() {
            self.universes.resize_with(model::TREATMENTS.len(), || None);
        }
        let slot = self
            .universes
            .get_mut(i)
            .ok_or_else(|| format!("no treatment {i}"))?;
        if slot.is_none() {
            let t = now_s();
            let u = model::build(i)?;
            *slot = Some((u, now_s() - t));
        }
        let (u, s) = slot.as_ref().ok_or("not built")?;
        Ok((u, *s))
    }
}

impl LoadedRung for Loaded {
    /// No tracks on this rung (`has_tracks` is false); never asked for.
    fn run_next(&mut self) -> History {
        History::from_track(0, Position::ZERO, 0.0, false, Default::default())
    }
    fn keff_start(&mut self, _cfg: KeffConfig) -> Result<Vec<[f32; 2]>, String> {
        Err("the dhshort rung has no k_eff mode".into())
    }
    fn keff_step(&mut self) -> Option<Generation> {
        None
    }
    fn keff_finished(&self) -> bool {
        true
    }
    /// `param`'s integer part is the treatment; its fraction counts redraws.
    fn raster(&mut self, req: &RasterReq) -> Result<Vec<u8>, String> {
        let (u, _) = self.universe(req.param.max(0.0).floor() as usize)?;
        Ok(model::raster(u, req))
    }
    /// `[MSG_DESCRIBE, i]` → `[MSG_DESCRIBE, i, build s, particles stored,
    /// packing fraction modelled, materials in the table]`.
    fn walk(&mut self, msg: &[f64]) -> Result<Vec<f64>, String> {
        match msg {
            [c, i] if *c == MSG_DESCRIBE => {
                let i = i.max(0.0) as usize;
                let (u, secs) = self.universe(i)?;
                Ok(vec![
                    MSG_DESCRIBE,
                    i as f64,
                    secs,
                    u.particle_count() as f64,
                    u.packing_fraction(),
                    u.materials().len() as f64,
                ])
            }
            _ => Err(format!("dhshort: unknown message {msg:?}")),
        }
    }
    /// Start treatment `param` on a fresh universe: builds its majorant and
    /// samples the source, as `DhUniverse::keff` does.
    fn kinf_start(&mut self, param: f64, cfg: KeffConfig) -> Result<(), String> {
        let t = param.round().max(0.0) as usize;
        if !LIVE.contains(&t) {
            return Err(format!("treatment {t} is not offered live"));
        }
        let nuclides = self
            .nuclides
            .as_ref()
            .ok_or("the live run's data are not loaded")?;
        let settings = KeffSettings {
            n_particles: cfg.n_particles,
            n_inactive: cfg.n_inactive,
            n_active: cfg.n_active,
            seed: cfg.seed,
            temperature_k: model::fhr::TEMP_K,
            ..KeffSettings::default()
        };
        let universe = model::build(t)?;
        let it = universe.power_iteration(nuclides, &settings);
        self.run = Some(LiveRun {
            t,
            universe,
            it,
            total: cfg.n_inactive + cfg.n_active,
        });
        Ok(())
    }
    fn kinf_step(&mut self) -> Option<KinfGeneration> {
        let run = self.run.as_mut()?;
        let nuclides = self.nuclides.as_ref()?;
        let g = run.it.step(&run.universe, nuclides)?;
        let (mean, sem) = g.k_mean.unwrap_or((f64::NAN, f64::NAN));
        Some(KinfGeneration {
            param: run.t as f64,
            index: g.index,
            total: run.total,
            active: g.active,
            k: g.k,
            mean,
            sem,
        })
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::raster::Basis;

    /// The worker answers a describe and a raster for every treatment, as the
    /// page asks for them; the explicit arm's realised packing fraction is
    /// `pack_in_ball`'s, within 0.2 % of the requested 0.30. Without data the
    /// live run refuses rather than runs.
    #[test]
    fn the_worker_serves_every_treatment() {
        let mut l = Builder::new(Tier::Loose).finish().unwrap();
        for i in 0..model::TREATMENTS.len() {
            let d = l.walk(&[MSG_DESCRIBE, i as f64]).unwrap();
            assert_eq!(d.len(), 6);
            assert_eq!(d[1] as usize, i);
            if i == 0 {
                assert!(d[3] > 20_000.0, "particles stored: {}", d[3]);
                assert!((d[4] / 0.30 - 1.0).abs() <= 2.0e-3, "pf {}", d[4]);
            } else {
                assert_eq!(d[3], 0.0);
            }
            let req = RasterReq {
                id: 2,
                basis: Basis::Xy,
                centre: [0.0, 0.0],
                depth: 0.0,
                width: [6.2, 6.2],
                px: [32, 24],
                param: i as f64 + 0.003,
            };
            assert_eq!(l.raster(&req).unwrap().len(), 32 * 24);
        }
        assert!(l.walk(&[9.0]).is_err());
        let cfg = DhShort::kinf_case().unwrap().cfg;
        assert!(l.kinf_start(0.0, cfg).is_err(), "no data loaded");
        assert!(l.kinf_start(2.0, cfg).is_err(), "SCLS is not offered");
        assert!(l.kinf_step().is_none());
        let case = DhShort::kinf_case().unwrap();
        assert_eq!(case.choices.len(), LIVE.len());
        assert_eq!(
            case.curves[0].points.len(),
            4,
            "2026-09-14 has delta, CLS, naive and ring-RPT among the live arms"
        );
    }

    /// The live run on the record's data, natively, at a small size, for the
    /// arms the page offers: prints the timings quoted on gh:#785 and checks
    /// each generation streams with a finite k. Opt-in (processes ten tapes).
    #[test]
    #[ignore = "processes 10 ENDF tapes at tolerance 0.001 (minutes); run with --ignored"]
    fn the_live_run_streams_every_offered_arm() {
        let mut b = Builder::new(Tier::Exact);
        let t = std::time::Instant::now();
        for (label, tape) in data::JOBS {
            let raw = std::fs::read(
                njoy_outram_park_fork::reference_data::reference_endf(tape).expect("tape"),
            )
            .expect("read");
            let t1 = std::time::Instant::now();
            b.step(&crate::tapes::strip_covariances(&raw), &mut crate::processed_cache::DataStore::off())
                .expect("step");
            eprintln!("  {label:<16} {:6.1} s", t1.elapsed().as_secs_f64());
        }
        let mut l = b.finish().unwrap();
        eprintln!("  data {:.1} s", t.elapsed().as_secs_f64());
        let n: usize = std::env::var("DHSHORT_N")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(200);
        let cfg = KeffConfig {
            n_particles: n,
            n_inactive: 5,
            n_active: 10,
            seed: 1,
            point_source: false,
            want_sites: false,
        };
        for &arm in &LIVE {
            let t = std::time::Instant::now();
            l.kinf_start(arm as f64, cfg).unwrap();
            let t_start = t.elapsed().as_secs_f64();
            let t = std::time::Instant::now();
            let mut last = None;
            while let Some(g) = l.kinf_step() {
                assert!(g.k.is_finite() && g.k > 0.5);
                last = Some(g);
            }
            let g = last.unwrap();
            let secs = t.elapsed().as_secs_f64();
            eprintln!(
                "  {:<14} start {t_start:.1} s, transport {secs:.1} s ({:.3} ms/history), {} x [{} + {}]: k = {:.5} +/- {:.5}",
                model::SHORT[arm], 1.0e3 * secs / (n * 15) as f64, n, cfg.n_inactive, cfg.n_active, g.mean, g.sem
            );
        }
    }
}
