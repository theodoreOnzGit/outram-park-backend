//! The `hst009` rung: ICSBEP HEU-SOL-THERM-009 case 1, a sphere of highly
//! enriched uranium oxyfluoride solution in a thin aluminium tank, reflected
//! by water (gh:#859). One of the four benchmark cases the acceptance tests
//! run; here the reader watches it and runs its `k_eff` in the browser, on
//! data this tab processed.
//!
//! **Nothing is retyped here.** Geometry, materials and tapes are
//! [`outram_mc_libs::vv::hst009`], the model the recorded result (route 4 of
//! the five-route study) and the acceptance check were measured on, with
//! that model's three stated approximations (U-236 and the tank's Cu and Zn
//! omitted, O-17 folded into O-16). The rung's machinery is
//! [`crate::csgrung`].

#[cfg(not(target_arch = "wasm32"))]
pub mod render;

use crate::anim::Spectrum;
use crate::csgrung::{self, CsgCase, ThermalLaw};
use crate::engine::Tier;
use crate::keff::{KeffConfig, Reference};
use crate::rungs::{McRung, RungInfo};
use dhoby_ghaut::web_demo::view::View;
use egui::{Color32, Rect, Sense, Stroke, Vec2};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::material::material::Material;
use outram_mc_libs::physics::transport_csg::SourceBox;
use outram_mc_libs::vv::hst009 as model;

/// The marker type `rung_table!` names.
pub struct Hst009;

/// `(label shown, nuclide name, tape)` in [`model::NUCLIDES`] order.
const NUCLIDES: [(&str, &str, &str); 11] = [
    ("U-234", model::NUCLIDES[0].0, model::NUCLIDES[0].1),
    ("U-235", model::NUCLIDES[1].0, model::NUCLIDES[1].1),
    ("U-238", model::NUCLIDES[2].0, model::NUCLIDES[2].1),
    ("F-19", model::NUCLIDES[3].0, model::NUCLIDES[3].1),
    ("O-16", model::NUCLIDES[4].0, model::NUCLIDES[4].1),
    ("H-1", model::NUCLIDES[5].0, model::NUCLIDES[5].1),
    ("Al-27", model::NUCLIDES[6].0, model::NUCLIDES[6].1),
    ("Si-28", model::NUCLIDES[7].0, model::NUCLIDES[7].1),
    ("Si-29", model::NUCLIDES[8].0, model::NUCLIDES[8].1),
    ("Si-30", model::NUCLIDES[9].0, model::NUCLIDES[9].1),
    ("Mn-55", model::NUCLIDES[10].0, model::NUCLIDES[10].1),
];
const SAB_LABEL: &str = "H in H2O S(a,b)";
/// Every tape, `(label, file)`: the eleven nuclides, then the S(α,β) law.
pub const JOBS: [(&str, &str); 12] = [
    (NUCLIDES[0].0, NUCLIDES[0].2),
    (NUCLIDES[1].0, NUCLIDES[1].2),
    (NUCLIDES[2].0, NUCLIDES[2].2),
    (NUCLIDES[3].0, NUCLIDES[3].2),
    (NUCLIDES[4].0, NUCLIDES[4].2),
    (NUCLIDES[5].0, NUCLIDES[5].2),
    (NUCLIDES[6].0, NUCLIDES[6].2),
    (NUCLIDES[7].0, NUCLIDES[7].2),
    (NUCLIDES[8].0, NUCLIDES[8].2),
    (NUCLIDES[9].0, NUCLIDES[9].2),
    (NUCLIDES[10].0, NUCLIDES[10].2),
    (SAB_LABEL, model::SAB_TAPE),
];
const N_U235: usize = 1;

const SOLUTION: Color32 = Color32::from_rgb(214, 170, 60);
const TANK: Color32 = Color32::from_rgb(170, 170, 176);
const WATER: Color32 = Color32::from_rgb(28, 62, 112);

/// The recorded tracks the page ships (`--bake-tracks hst009`).
const RECORDED_TRACKS: &[u8] = include_bytes!("data/tracks.bin.z");

impl CsgCase for Hst009 {
    const TEMPERATURE_K: f64 = model::TEMPERATURE_K;
    const N_U235: usize = N_U235;
    /// A fresh chain starts from a thermal fission: this is a thermal system.
    const FRESH_INCIDENT_EV: f64 = 0.0253;
    /// A neutron thermalising in water scatters a few hundred times at most.
    const MAX_STATES: usize = 200_000;
    fn nuclides() -> &'static [(&'static str, &'static str, &'static str)] {
        &NUCLIDES
    }
    fn thermal() -> Option<ThermalLaw> {
        Some(ThermalLaw { label: SAB_LABEL, tape: model::SAB_TAPE, name: "c_H_in_H2O", nuclide: model::N_H1 })
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
    /// A uniform point of the solution.
    fn fresh_birth(rng: &mut u64) -> Position {
        csgrung::uniform_in_sphere(rng, model::R_SOLUTION * (1.0 - 1.0e-9))
    }
    fn reference() -> Option<Reference> {
        <Hst009 as McRung>::reference()
    }
}

impl McRung for Hst009 {
    const INFO: RungInfo = RungInfo {
        name: "hst009",
        title: "HEU-SOL-THERM-009: a uranium solution in water",
        lesson: "tutorials/monte-carlo/benchmarks.html",
        spectrum: Spectrum::Thermal,
    };
    type Builder = csgrung::Builder<Hst009>;

    fn jobs() -> &'static [(&'static str, &'static str)] {
        &JOBS
    }
    /// Run k_eff asks for NJOY's tolerance (so `k` is comparable with the
    /// record); Watch for the loosened 0.01.
    fn tier(requested: Tier) -> Tier {
        requested
    }
    /// Native seconds per job, i9-13900K, one thread, shared machine: the
    /// uranium tapes as the Godiva rung measured them, the light nuclides and
    /// the thermal law as the LCT-008 rung did at tolerance 0.01; F-19 was
    /// not measured and is a guess. Progress-bar weights only.
    fn job_weights(tier: Tier) -> Vec<f64> {
        match tier {
            Tier::Exact => vec![1.6, 29.5, 31.1, 1.0, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 1.0, 16.2],
            Tier::Loose => vec![1.0, 11.2, 11.6, 0.5, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.5, 16.2],
        }
    }
    fn half_extent() -> f64 {
        model::R_REFLECTOR
    }
    /// The three concentric spheres, cut through the centre.
    fn draw(painter: &egui::Painter, rect: Rect, view: &View) {
        let s = view.scale as f32;
        let o = view.to_screen(rect, 0.0, 0.0);
        painter.circle_filled(o, model::R_REFLECTOR as f32 * s, WATER);
        painter.circle_filled(o, model::R_TANK as f32 * s, TANK);
        painter.circle_filled(o, model::R_SOLUTION as f32 * s, SOLUTION);
        painter.circle_stroke(o, model::R_REFLECTOR as f32 * s, Stroke::new(1.5, Color32::from_rgb(170, 120, 255)));
        painter.text(
            o + Vec2::new(0.0, -(model::R_REFLECTOR as f32) * s - 6.0),
            egui::Align2::CENTER_BOTTOM,
            format!("solution r = {} cm · Al tank to {} cm · water to {} cm · vacuum outside", model::R_SOLUTION, model::R_TANK, model::R_REFLECTOR),
            egui::FontId::proportional(12.0),
            Color32::from_rgb(190, 170, 230),
        );
    }
    fn notes() -> &'static [&'static str] {
        &[
            "Model: ICSBEP HEU-SOL-THERM-009 case 1: a sphere of highly enriched uranium oxyfluoride (UO₂F₂) solution, r = 11.5177 cm, in a 1100-aluminium tank 0.1587 cm thick, reflected by water to r = 35 cm, 293.6 K. The numbers are outram-mc-libs' `vv::hst009`, the model the recorded result was measured on.",
            "Three stated approximations, as in the record: U-236 is omitted (0.5 % of the uranium, no tape here), O-17 is folded into O-16, and the tank's copper and zinc are omitted.",
            "Data: ENDF/B-VIII.0, eleven nuclides, processed in this browser by OUTRAM PARK's NJOY port. Hydrogen uses the H in H₂O S(α,β) thermal-scattering law, in the solution and in the reflector. Run k_eff: NJOY's tolerance 0.001 (as the record). Watch: the loosened 0.01, whose measured effect on this case is −57 ± 35 pcm.",
            "Run k_eff is true Monte Carlo: outram-mc-libs' single-thread CSG power iteration (`CsgPowerIteration`, what `run_keff_csg` runs), one generation at a time.",
            "The benchmark k is 0.9990 ± 0.0043 (430 pcm), not 1. On this data the case sits inside that band: OpenMC on NJOY2016 data gives +334 ± 22 pcm from the benchmark k and outram-mc +298 ± 32 pcm (32 seeds each).",
            "Watch, one neutron at a time, is an illustration: each track is one real history (a one-particle fixed-source run), chained by hand. Tracks are 3D, drawn projected onto the plane of the picture.",
            "Watch, whole generations, is the real power iteration started from the centre of the solution, to show the source spreading.",
            "The Shannon entropy of the fission source is on a 5 × 5 × 5 mesh over the solution's bounding box, so at most 6.97 bits. A CSG run reports no leaked / captured / fissioned counts (the sphere-only iteration of the Godiva rung does).",
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
            "hst009",
            "five-route study route 4, 32 seeds x 5000 x [40+120], 2026-09-30, at 0414bc8277",
            crate::route1_label!("32 seeds x 5000 x [40+120]"),
            5000.0 * 120.0,
            ("ICSBEP HEU-SOL-THERM-009 case 1 benchmark k, as tabulated by van der Marck (2006)", model::BENCHMARK_K, model::BENCHMARK_SIGMA),
        )
    }
    fn loading_note(_tier: Tier) -> Option<&'static str> {
        Some("Twelve tapes, including the water's thermal-scattering law: allow a few minutes in a browser, longer on a phone. Later visits read the processed data from this browser's cache.")
    }
    fn legend(ui: &mut egui::Ui) {
        ui.strong("Materials");
        for (c, name) in [(SOLUTION, "UO₂F₂ solution, highly enriched"), (TANK, "1100 aluminium tank"), (WATER, "Water reflector")] {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                ui.painter().rect_filled(r, 2.0, c);
                ui.label(name);
            });
        }
    }
    fn recorded_tracks() -> Option<&'static [u8]> {
        Some(RECORDED_TRACKS)
    }
}
