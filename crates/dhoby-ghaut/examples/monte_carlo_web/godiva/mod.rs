//! The `godiva` rung: ICSBEP HEU-MET-FAST-001, a bare sphere of highly
//! enriched uranium (rung 1 of the Monte Carlo ladder, gh:#521). The worked
//! example of a rung with a Run k_eff mode; see `rungs.rs` for the contract.

pub mod model;
#[cfg(not(target_arch = "wasm32"))]
pub mod render;
pub mod sim;

use crate::anim::Spectrum;
use crate::engine::{Tier, CHAIN_SEED};
use crate::keff::{KeffConfig, Reference};
use crate::rungs::{McRung, RungBuilder, RungInfo};
use dhoby_ghaut::web_demo::view::View;
use egui::{Color32, Rect, Stroke, Vec2};

/// The marker type `rung_table!` names.
pub struct Godiva;

/// The recorded result for this model and data, to compare a reader's run
/// with: **0.99994 ± 0.00005 (−6 ± 5 pcm)**, 1024 seeds × 5000 histories ×
/// [40 inactive + 120 active], seed-to-seed sd 165 pcm,
/// `crates/outram-mc-libs/examples/godiva_keff_ensemble.rs` "Results
/// (2026-10-05)", commit `6faff1ed8` (#546), the number the Godiva lesson
/// quotes. ~~Route 4 of the five-route record, 0.99948 ± 0.00027 (−52 ±
/// 27 pcm), 32 seeds, 2026-09-30, `0414bc8277`, sd 151 pcm~~: **CORRECTED
/// 2026-10-05** (#527): that record was superseded the same day (its 32 seeds
/// had come out low, #546) but this demo still compared with it. A 512-seed
/// re-check after #527's source fix gave −7 ± 8 pcm. Quoted, not re-measured
/// here.
pub const REFERENCE: Reference = Reference {
    label: "1024 seeds x 5000 x [40+120], 2026-10-05, godiva_keff_ensemble.rs at 6faff1ed8",
    k: 0.99994,
    sem: 0.00005,
    sd_one_run: 0.00165,
    histories_one_run: 5000.0 * 120.0,
    experiment: Some(("ICSBEP HEU-MET-FAST-001", outram_mc_libs::vv::godiva::BENCHMARK_K, outram_mc_libs::vv::godiva::BENCHMARK_SIGMA)),
};

impl McRung for Godiva {
    const INFO: RungInfo = RungInfo {
        name: "godiva",
        title: "Godiva: a bare uranium sphere",
        lesson: "tutorials/monte-carlo/godiva.html",
        spectrum: Spectrum::Fast,
    };
    type Builder = model::DataBuilder;

    fn jobs() -> &'static [(&'static str, &'static str)] {
        &model::JOBS
    }
    /// Run k_eff asks for NJOY's tolerance (so `k` is comparable with the
    /// record); Watch for the loosened 0.01.
    fn tier(requested: Tier) -> Tier {
        requested
    }
    /// Native, 2026-10-04, i9-13900K, one thread, shared machine (U-234 /
    /// U-235 / U-238: 1.6 / 29.5 / 31.1 s at tolerance 0.001, 1.0 / 11.2 /
    /// 11.6 s at 0.01). Progress-bar weights only.
    fn job_weights(tier: Tier) -> Vec<f64> {
        match tier {
            Tier::Exact => vec![1.6, 29.5, 31.1],
            Tier::Loose => vec![1.0, 11.2, 11.6],
        }
    }
    fn half_extent() -> f64 {
        model::RADIUS_CM
    }
    /// A uranium disc (the sphere seen from above), vacuum outside.
    fn draw(painter: &egui::Painter, rect: Rect, view: &View) {
        let r = model::RADIUS_CM;
        let o = view.to_screen(rect, 0.0, 0.0);
        painter.circle_filled(o, (r * view.scale) as f32, Color32::from_rgb(70, 64, 58));
        painter.circle_stroke(o, (r * view.scale) as f32, Stroke::new(1.5, Color32::from_rgb(170, 120, 255)));
        painter.text(
            o + Vec2::new(0.0, -(r * view.scale) as f32 - 6.0),
            egui::Align2::CENTER_BOTTOM,
            format!("HEU metal sphere, r = {r} cm · vacuum outside (leaks)"),
            egui::FontId::proportional(12.0),
            Color32::from_rgb(190, 170, 230),
        );
    }
    fn notes() -> &'static [&'static str] {
        &[
            "Model: ICSBEP HEU-MET-FAST-001 (Godiva), a bare sphere of highly enriched uranium metal, r = 8.7407 cm, U-234/235/238 at the evaluation's atom densities, 293.6 K. The numbers are outram-mc-libs' `vv::godiva`, the model the recorded result was measured on.",
            "Data: ENDF/B-VIII.0, processed in this browser by OUTRAM PARK's NJOY port. Run k_eff: NJOY's tolerance 0.001 (as the record). Watch: the loosened 0.01, whose measured effect on Godiva is +7 ± 41 pcm.",
            "Run k_eff is true Monte Carlo: outram-mc-libs' single-thread reference power iteration, one generation at a time. Its counts (leaked, captured, fissioned) are tallied by that transport.",
            "Watch, one neutron at a time, is an illustration: each track is one real history (a one-particle fixed-source run), chained by hand. Tracks are 3D, drawn projected from above.",
            "Watch, whole generations, is the real power iteration started from a point at the centre, to show the source spreading.",
            "Animation speed is proportional to the neutron's real speed (classical kinetic energy, v proportional to √E).",
            "Education and research only. Not for reactor operation, licensing or safety decisions.",
        ]
    }
    /// **The recorded result's own settings**, 5000 neutrons × [40 inactive
    /// + 120 active], seed 1, so a reader's run is one seed of the ~~32~~ 1024 the
    /// record pools.
    ///
    /// Sized on a measurement: natively (i9-13900K, 16 logical cores, 1
    /// thread used, 62 GB, Linux, CPU only, shared with other builds) the
    /// transport of these 800 000 histories took **2.9 s** on 2026-10-04 and
    /// processing the three tapes at tolerance 0.001 took 62 s. That run
    /// (`--headless-keff 5000 40 120 1`) gave ~~**k = 0.99942 ± 0.00183
    /// (−58 ± 183 pcm)**~~ on the old initial source; **re-run 2026-10-05
    /// after #527** (independent source directions; Intel Xeon @ 2.10 GHz,
    /// 1 of 4 shared logical cores, 15.7 GB, Linux, CPU only): **k = 0.99689
    /// ± 0.00178 (−311 ± 178 pcm)**, transport 5.6 s, data 98.6 s. One draw,
    /// 1.7σ of its own ± from the 1024-seed record's −6 ± 5 pcm. Headless
    /// Chromium (1280 × 800, swiftshader, same shared machine) printed the
    /// same k and counts digit for digit, after 232 s of data processing and
    /// 21 s of transport.
    fn run_default() -> Option<KeffConfig> {
        Some(KeffConfig { n_particles: 5000, n_inactive: 40, n_active: 120, seed: 1, point_source: false, want_sites: false })
    }
    /// A small population started at the centre: enough generations for the
    /// entropy to rise and flatten.
    fn watch_generations() -> Option<KeffConfig> {
        Some(KeffConfig { n_particles: 1000, n_inactive: 15, n_active: 15, seed: 1, point_source: true, want_sites: true })
    }
    fn reference() -> Option<Reference> {
        Some(REFERENCE)
    }
    fn loading_note(tier: Tier) -> Option<&'static str> {
        (tier == Tier::Exact).then_some(
            "Run k_eff processes at NJOY's tolerance 0.001, like the recorded result, so it takes about three \
             times longer than Watch mode (a fast desktop natively: about a minute; a browser or a phone: several).",
        )
    }
}

impl RungBuilder for model::DataBuilder {
    type Loaded = sim::Loaded;
    fn new(tier: Tier) -> Self {
        model::DataBuilder::new(tier.speed())
    }
    fn step(&mut self, bytes: &[u8], store: &mut crate::processed_cache::DataStore) -> Result<(), String> {
        model::DataBuilder::step(self, bytes, store)
    }
    fn finish(self) -> Result<sim::Loaded, String> {
        let data = model::DataBuilder::finish(self)?;
        Ok(sim::Loaded { phys: sim::Physics::new(data), chain: sim::Chain::new(CHAIN_SEED), keff: None })
    }
}
