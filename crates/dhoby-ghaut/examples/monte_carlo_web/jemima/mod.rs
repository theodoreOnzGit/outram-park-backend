//! The `jemima` rung: ICSBEP IEU-MET-FAST-002, a cylinder of 16 % enriched
//! uranium metal inside natural-uranium reflectors (gh:#859). One of the four
//! benchmark cases the acceptance tests run; here the reader watches it and
//! runs its `k_eff` in the browser, on data this tab processed.
//!
//! **Nothing is retyped here.** Geometry, materials and tapes are
//! [`outram_mc_libs::vv::jemima`], the model the recorded result (route 4 of
//! the five-route study) and the acceptance check were measured on. The
//! rung's machinery is [`crate::csgrung`].

#[cfg(not(target_arch = "wasm32"))]
pub mod render;

use crate::anim::Spectrum;
use crate::csgrung::{self, CsgCase};
use crate::engine::Tier;
use crate::keff::{KeffConfig, Reference};
use crate::rungs::{McRung, RungInfo};
use dhoby_ghaut::web_demo::view::View;
use egui::{Color32, Rect, Sense, Stroke, Vec2};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::material::material::Material;
use outram_mc_libs::physics::transport_csg::SourceBox;
use outram_mc_libs::vv::jemima as model;

/// The marker type `rung_table!` names.
pub struct Jemima;

/// The three tapes, `(label shown while loading, file)`, in
/// [`model::NUCLIDES`] order.
pub const JOBS: [(&str, &str); 3] = [("U-234", model::NUCLIDES[0].1), ("U-235", model::NUCLIDES[1].1), ("U-238", model::NUCLIDES[2].1)];
const NUCLIDES: [(&str, &str, &str); 3] = [
    ("U-234", model::NUCLIDES[0].0, model::NUCLIDES[0].1),
    ("U-235", model::NUCLIDES[1].0, model::NUCLIDES[1].1),
    ("U-238", model::NUCLIDES[2].0, model::NUCLIDES[2].1),
];

const CORE: Color32 = Color32::from_rgb(196, 92, 60);
const REFLECTOR: Color32 = Color32::from_rgb(96, 84, 128);

/// The recorded tracks the page ships (`--bake-tracks jemima`).
const RECORDED_TRACKS: &[u8] = include_bytes!("data/tracks.bin.z");

impl CsgCase for Jemima {
    const TEMPERATURE_K: f64 = model::TEMPERATURE_K;
    const N_U235: usize = 1;
    /// A fresh chain starts from a 1 MeV-incident fission, as in Godiva.
    const FRESH_INCIDENT_EV: f64 = 1.0e6;
    /// A fast neutron here makes a few tens of collisions.
    const MAX_STATES: usize = 20_000;
    fn nuclides() -> &'static [(&'static str, &'static str, &'static str)] {
        &NUCLIDES
    }
    fn materials() -> Vec<Material> {
        model::materials()
    }
    fn geometry() -> Geometry {
        model::geometry()
    }
    fn source_box() -> SourceBox {
        model::source_box()
    }
    /// The whole assembly: the natural-uranium reflector fissions too.
    fn entropy_box() -> SourceBox {
        SourceBox { lower: Position::new(-model::R_OUT, -model::R_OUT, model::Z_BOT), upper: Position::new(model::R_OUT, model::R_OUT, model::Z_TOP) }
    }
    /// A uniform point of the core.
    fn fresh_birth(rng: &mut u64) -> Position {
        csgrung::uniform_in_cylinder(rng, model::R_CORE, model::Z_CORE_LO, model::Z_CORE_HI)
    }
    fn reference() -> Option<Reference> {
        <Jemima as McRung>::reference()
    }
}

impl McRung for Jemima {
    const INFO: RungInfo = RungInfo {
        name: "jemima",
        title: "Jemima: uranium in a uranium reflector",
        lesson: "tutorials/monte-carlo/benchmarks.html",
        spectrum: Spectrum::Fast,
    };
    type Builder = csgrung::Builder<Jemima>;

    fn jobs() -> &'static [(&'static str, &'static str)] {
        &JOBS
    }
    /// 0.001 for every mode (gh:#876 item 3), so Watch and Run share one
    /// load.
    fn tier(_requested: Tier) -> Tier {
        Tier::Exact
    }
    /// The Godiva rung's measured seconds for the same three tapes
    /// (progress-bar weights only).
    fn job_weights(tier: Tier) -> Vec<f64> {
        match tier {
            Tier::Exact => vec![1.6, 29.5, 31.1],
            // Never asked: this rung always processes at 0.001 (`tier`).
            Tier::NoData => Vec::new(),
        }
    }
    fn half_extent() -> f64 {
        model::R_OUT
    }
    /// The assembly seen from above: the core disc inside the reflector
    /// annulus, vacuum outside. (The reflector also covers the core above
    /// and below; the side view is in the geometry images.)
    fn draw(painter: &egui::Painter, rect: Rect, view: &View) {
        let s = view.scale as f32;
        let o = view.to_screen(rect, 0.0, 0.0);
        painter.circle_filled(o, model::R_OUT as f32 * s, REFLECTOR);
        painter.circle_filled(o, model::R_CORE as f32 * s, CORE);
        painter.circle_stroke(o, model::R_OUT as f32 * s, Stroke::new(1.5, Color32::from_rgb(170, 120, 255)));
        painter.text(
            o + Vec2::new(0.0, -(model::R_OUT as f32) * s - 6.0),
            egui::Align2::CENTER_BOTTOM,
            format!("core r = {} cm · natural-U reflector to r = {} cm · vacuum outside (leaks)", model::R_CORE, model::R_OUT),
            egui::FontId::proportional(12.0),
            Color32::from_rgb(190, 170, 230),
        );
    }
    fn notes() -> &'static [&'static str] {
        &[
            "Model: ICSBEP IEU-MET-FAST-002 (Jemima), from the mit-crpg/benchmarks OpenMC model: a cylinder of 16 % enriched uranium metal, r = 19.05 cm and 31.951 cm tall, inside natural-uranium reflectors (7.62 cm below, 7.5184 cm above, to r = 26.6446 cm), 293.6 K. The numbers are outram-mc-libs' `vv::jemima`, the model the recorded result was measured on.",
            "Data: ENDF/B-VIII.0 (U-234, U-235, U-238), processed in this browser by OUTRAM PARK's NJOY port. Run k_eff and Watch both use NJOY's tolerance 0.001 (as the record), one load cached for both.",
            "Run k_eff is true Monte Carlo on a pool of workers (since 2026-10-16, gh:#907): outram-mc-libs' `run_keff_csg_par` power iteration taken apart by `transport_csg::distributed`. Each generation's histories are shared among the workers and added up in history order, so k is the same bit for bit whatever their number, and is what `run_keff_csg_par` (the driver of the recorded five-route results) gives for the same seed and data. Each worker holds its own copy of the processed data (read from this browser's cache). Before 2026-10-16 this ran the single-thread `CsgPowerIteration` in one worker.",
            "The benchmark k is 1.0000 ± 0.0030 (300 pcm). On this data the case sits below that band: OpenMC on NJOY2016 data gives −406 ± 17 pcm and outram-mc −391 ± 27 pcm (32 seeds each). The two codes agree with each other; the difference from the benchmark is in the nuclear data or the benchmark model, not in the transport.",
            "Watch, one neutron at a time, is an illustration: each track is one real history (a one-particle fixed-source run), chained by hand. Tracks are 3D, drawn projected from above, so a neutron in the reflector above the core is drawn over the core.",
            "Watch, whole generations, is the real power iteration started from the middle of the core, to show the source spreading.",
            "The Shannon entropy of the fission source is on a 5 × 5 × 5 mesh over the whole assembly (the reflector fissions too), so at most 6.97 bits. A CSG run reports no leaked / captured / fissioned counts (the sphere-only iteration of the Godiva rung does).",
            "During Run k_eff the neutrons drawn on the geometry are real histories born at sites of the run's present fission source. They are run in addition to the run's own neutrons and are not counted in k.",
            "Education and research only. Not for reactor operation, licensing or safety decisions.",
        ]
    }
    /// **The recorded result's own settings**, 5000 neutrons × [40 inactive
    /// + 120 active], seed 1: one seed of the 32 the record pools.
    fn run_default() -> Option<KeffConfig> {
        Some(KeffConfig { n_particles: 5000, n_inactive: 40, n_active: 120, seed: 1, point_source: false, want_sites: false })
    }
    fn watch_generations() -> Option<KeffConfig> {
        Some(KeffConfig { n_particles: 1000, n_inactive: 15, n_active: 15, seed: 1, point_source: true, want_sites: true })
    }
    /// Route 4 of the five-route study (outram-mc on the Rust NJOY port,
    /// ENDF read directly), read from the committed campaign summary.
    fn reference() -> Option<Reference> {
        crate::keff::five_route_reference(
            "jemima",
            "five-route study route 4, 32 seeds x 5000 x [40+120], 2026-10-15, at 093d8590cf",
            crate::route1_label!("32 seeds x 5000 x [40+120]"),
            5000.0 * 120.0,
            ("ICSBEP IEU-MET-FAST-002 benchmark k, as tabulated by Mosteller (LA-UR-10-06230) and van der Marck (2006)", model::BENCHMARK_K, model::BENCHMARK_SIGMA),
        )
    }
    fn loading_note(tier: Tier) -> Option<&'static str> {
        (tier == Tier::Exact).then_some(
            "Watch and Run k_eff share one load at NJOY's tolerance 0.001, like the recorded result, its tapes \
             shared among several workers. The first visit processes the tapes; later visits read them from this browser's cache.",
        )
    }
    fn legend(ui: &mut egui::Ui) {
        ui.strong("Materials");
        for (c, name) in [(CORE, "Core: uranium metal, 16 % U-235"), (REFLECTOR, "Reflector: natural uranium")] {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                ui.painter().rect_filled(r, 2.0, c);
                ui.label(name);
            });
        }
    }
    /// Run k_eff on a worker pool (gh:#907). **150 MB per worker is an
    /// ESTIMATE** (3 nuclides), not yet measured in a browser; it only sizes
    /// the pool. Replace it with the measured `wasm_memory_mb` (gh:#907).
    fn run_pool_mb() -> Option<f64> {
        Some(150.0)
    }
    fn recorded_tracks() -> Option<&'static [u8]> {
        Some(RECORDED_TRACKS)
    }
}
