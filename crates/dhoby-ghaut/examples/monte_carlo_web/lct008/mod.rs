//! The `lct008` rung: ICSBEP LEU-COMP-THERM-008 case 1, rods of low-enriched
//! UO₂ in borated water, the light-water reactor's lattice (rung 4 of the
//! Monte Carlo ladder, gh:#526). Watch mode only, like `triso/`; see
//! `rungs.rs` for the contract.
//!
//! The core is the real benchmark model: the committed `mit-crpg/benchmarks`
//! cards, parsed by outram-mc-libs' shared `lct008_model.rs` (module
//! [`spec`]), 4961 fuel rods in nested lattices. Each track is one real
//! history through that lattice. No Run k_eff here: the eigenvalue of this
//! core takes ~10⁸ histories to pin down, which is not a browser job. The
//! lesson quotes the recorded result instead.

pub mod model;
#[cfg(not(target_arch = "wasm32"))]
pub mod render;
pub mod sim;

/// The shared LCT-008 model code (XML cards, materials, nested-lattice CSG),
/// used here unchanged so the demo and the V&V runs build one model.
#[path = "../../../../outram-mc-libs/examples/common/lct008_model.rs"]
#[allow(clippy::all)]
pub mod spec;

use crate::anim::Spectrum;
use crate::engine::{Tier, CHAIN_SEED};
use crate::history::History;
use crate::keff::{Generation, KeffConfig};
use crate::rungs::{LoadedRung, McRung, RungBuilder, RungInfo};
use dhoby_ghaut::web_demo::view::View;
use egui::{Color32, Rect, Sense, Stroke, Vec2};

/// The marker type `rung_table!` names.
pub struct Lct008;

const VACUUM: Color32 = Color32::from_rgb(18, 20, 26);
const WATER: Color32 = Color32::from_rgb(28, 62, 112);
const CLAD: Color32 = Color32::from_rgb(150, 150, 154);
const FUEL: Color32 = Color32::from_rgb(214, 96, 52);
const OTHER: Color32 = Color32::from_rgb(200, 200, 120);

/// Pin centres for the picture, from the assembled geometry (built once on
/// the UI side; parsing the cards takes milliseconds).
static PINS: std::sync::OnceLock<Vec<(f64, f64, model::PinKind)>> = std::sync::OnceLock::new();

fn pins() -> &'static [(f64, f64, model::PinKind)] {
    PINS.get_or_init(|| match model::materials() {
        Ok(m) => model::pins(&model::build_geometry(&m)),
        Err(_) => Vec::new(),
    })
}

impl McRung for Lct008 {
    const INFO: RungInfo = RungInfo {
        name: "lct008",
        title: "LEU-COMP-THERM-008: rods in water",
        lesson: "tutorials/monte-carlo/lct008.html",
        spectrum: Spectrum::Thermal,
    };
    type Builder = model::DataBuilder;

    fn jobs() -> &'static [(&'static str, &'static str)] {
        &model::JOBS
    }
    /// Always the loosened 0.01 tier: Watch mode is an illustration (module
    /// docs of [`model`]).
    fn tier(_requested: Tier) -> Tier {
        Tier::Loose
    }
    /// Native seconds per job at tolerance 0.01, measured 2026-10-04 by the
    /// rung's ignored test (i9-13900K, one thread, shared machine; 0.0 s
    /// jobs floored at 0.1); progress-bar weights only.
    fn job_weights(_tier: Tier) -> Vec<f64> {
        vec![0.1, 0.1, 0.1, 0.8, 14.8, 13.0, 0.1, 0.1, 0.1, 0.1, 0.5, 16.2]
    }
    fn half_extent() -> f64 {
        model::R_CORE
    }
    /// The core seen from above: water inside the r = 76.2 cm cylinder,
    /// vacuum outside, and every rod (clad ring, fuel pellet) at its centre
    /// as read from the assembled geometry. Zoomed out, a rod is a dot.
    fn draw(painter: &egui::Painter, rect: Rect, view: &View) {
        let s = view.scale as f32;
        let o = view.to_screen(rect, 0.0, 0.0);
        painter.rect_filled(painter.clip_rect(), 0.0, VACUUM);
        painter.circle_filled(o, model::R_CORE as f32 * s, WATER);
        let clip = painter.clip_rect().expand(8.0);
        let (rc, rf) = (model::R_CLAD as f32 * s, model::R_FUEL as f32 * s);
        for &(x, y, k) in pins() {
            let c = view.to_screen(rect, x, y);
            if !clip.contains(c) {
                continue;
            }
            let col = if k == model::PinKind::Fuel { FUEL } else { OTHER };
            if rc < 1.5 {
                painter.circle_filled(c, 1.0, col);
                continue;
            }
            painter.circle_filled(c, rc, CLAD);
            painter.circle_filled(c, rf, col);
        }
        painter.circle_stroke(o, model::R_CORE as f32 * s, Stroke::new(1.5, Color32::from_rgb(170, 120, 255)));
        painter.text(
            o + Vec2::new(0.0, -(model::R_CORE as f32) * s - 6.0),
            egui::Align2::CENTER_BOTTOM,
            "core cylinder r = 76.2 cm · vacuum outside (leaks)",
            egui::FontId::proportional(12.0),
            Color32::from_rgb(190, 170, 230),
        );
    }
    fn notes() -> &'static [&'static str] {
        &[
            "Model: ICSBEP LEU-COMP-THERM-008 case 1 (Babcock & Wilcox critical lattice): 4961 rods of 2.459 w/o UO₂ in Al-6061 clad, 1.63576 cm pitch, in water with 1511 ppm boron, arranged as a 7 × 7 core lattice of 15 × 15 pin lattices. Read at run time from the committed mit-crpg/benchmarks OpenMC cards by the same code the recorded result was measured with.",
            "Data: ENDF/B-VIII.0, processed in this browser by OUTRAM PARK's NJOY port at tolerance 0.01 (NJOY uses 0.001; measured effect on this lattice −65 ± 40 pcm). Hydrogen in the water uses the H in H₂O S(α,β) thermal-scattering law. 11 nuclides: the clad's trace elements (Mg, Ti, Cr, Fe, Cu, Zn) and B-11 are left out, as in the quoted result.",
            "Each track is one real history (a one-particle fixed-source run through the nested lattices), drawn projected from above. Chaining one neutron after the next is the illustration.",
            "There is no Run k_eff for this rung: pinning down this core's k takes about 10⁸ histories. The recorded result (+236 ± 8 pcm from k = 1, 96 seeds) is on the lesson page.",
            "Animation speed is proportional to the neutron's real speed (classical kinetic energy, v proportional to √E).",
            "Education and research only. Not for reactor operation, licensing or safety decisions.",
        ]
    }
    fn loading_note(_tier: Tier) -> Option<&'static str> {
        Some("Twelve tapes, including the water's thermal-scattering law: allow a few minutes in a browser, longer on a phone.")
    }
    fn legend(ui: &mut egui::Ui) {
        ui.strong("Materials");
        for (c, name) in [
            (FUEL, "UO₂ fuel, 2.459 w/o U-235"),
            (CLAD, "Al-6061 clad"),
            (WATER, "Water, 1511 ppm boron"),
            (VACUUM, "Vacuum (outside the core)"),
        ] {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                ui.painter().rect_filled(r, 2.0, c);
                ui.label(name);
            });
        }
    }
}

impl RungBuilder for model::DataBuilder {
    type Loaded = Loaded;
    fn new(_tier: Tier) -> Self {
        model::DataBuilder::default()
    }
    fn step(&mut self, bytes: &[u8]) -> Result<(), String> {
        model::DataBuilder::step(self, bytes)
    }
    fn finish(self) -> Result<Loaded, String> {
        Ok(Loaded { phys: sim::Physics::new(model::DataBuilder::finish(self)?), chain: sim::Chain::new(CHAIN_SEED) })
    }
}

/// The rung once its data are processed.
pub struct Loaded {
    pub phys: sim::Physics,
    pub chain: sim::Chain,
}

impl LoadedRung for Loaded {
    fn run_next(&mut self) -> History {
        self.chain.run_next(&self.phys)
    }
    fn keff_start(&mut self, _cfg: KeffConfig) -> Result<Vec<[f32; 2]>, String> {
        Err("the LCT-008 rung has no k_eff mode".into())
    }
    fn keff_step(&mut self) -> Option<Generation> {
        None
    }
    fn keff_finished(&self) -> bool {
        true
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use outram_mc_libs::physics::track_output::TrackEvent;

    /// The picture and the births use the rods read back from the assembled
    /// geometry; case 1 has 4961 fuel rods and nothing else (ring-RPT V&V
    /// record, Interpretation 18), so a wrong walk of the nested lattices
    /// shows here. And the shared model's own 200 000-point self-check of
    /// `locate` against hand arithmetic passes on the geometry this rung builds.
    #[test]
    fn the_core_has_the_benchmark_rods_and_passes_its_self_check() {
        let mats = model::materials().expect("materials");
        let geom = model::build_geometry(&mats);
        let p = model::pins(&geom);
        assert_eq!(p.iter().filter(|q| q.2 == model::PinKind::Fuel).count(), 4961);
        assert_eq!(p.len(), 4961);
        let shares = spec::check_geometry(&geom, &mats);
        assert_eq!(shares.len(), 3);
    }

    /// Process the tapes (tolerance 0.01) and run real histories through the
    /// core: each ends in a leak, capture or fission, inside the core
    /// cylinder, and most of them reach thermal energies (this is a
    /// water-moderated lattice). Needs `reference-data/endf/` and about a
    /// minute natively, so it is opt-in, as the crate's other data tests.
    #[test]
    #[ignore = "processes 12 ENDF tapes (~1 min); run with --ignored"]
    fn histories_through_the_lattice_end_properly_and_thermalise() {
        let mut b = model::DataBuilder::default();
        for (label, tape) in model::JOBS {
            let t = std::time::Instant::now();
            let path = njoy_outram_park_fork::reference_data::reference_endf(tape).expect("tape");
            let raw = std::fs::read(path).expect("read");
            b.step(&crate::tapes::strip_covariances(&raw)).expect("step");
            eprintln!("  {label:<16} {:6.1} s", t.elapsed().as_secs_f64());
        }
        let phys = sim::Physics::new(b.finish().expect("finish"));
        assert_eq!(phys.fuel.len(), 4961);
        let mut chain = sim::Chain::new(3);
        let (mut thermal, n) = (0, 60);
        let t = std::time::Instant::now();
        for _ in 0..n {
            let h = chain.run_next(&phys);
            assert!(
                matches!(h.outcome, Some(TrackEvent::Leak | TrackEvent::Fission | TrackEvent::Absorption)),
                "history {}: {}",
                h.index,
                crate::history::outcome_name(h.outcome)
            );
            assert_eq!(h.track.dropped_states, 0);
            for s in &h.track.states {
                let r = (s.r.x * s.r.x + s.r.y * s.r.y).sqrt();
                assert!(r <= model::R_CORE * (1.0 + 1e-9) && s.r.z.abs() <= model::Z_HI * (1.0 + 1e-9));
            }
            thermal += h.thermalised() as usize;
        }
        eprintln!("  {n} histories in {:.2} s, {thermal} thermalised", t.elapsed().as_secs_f64());
        assert!(thermal > n / 2, "a water lattice thermalises most neutrons; got {thermal} of {n}");
    }
}
