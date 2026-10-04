//! The `triso` rung: a 2D analogue of an HTR-10 fuel pebble in a reflective
//! cell, one neutron at a time. (The hook of the Monte Carlo tutorial; rung 5
//! of the ladder, gh:#520.) The worked example of a rung without a Run k_eff
//! mode; see `rungs.rs` for the contract.

pub mod model;
#[cfg(not(target_arch = "wasm32"))]
pub mod render;
pub mod sim;

use crate::anim::Spectrum;
use crate::engine::{Tier, CHAIN_SEED};
use crate::history::History;
use crate::keff::{Generation, KeffConfig};
use crate::rungs::{LoadedRung, McRung, RungBuilder, RungInfo};
use dhoby_ghaut::web_demo::view::View;
use egui::{Color32, Rect, Sense, Stroke, StrokeKind, Vec2};

/// The marker type `rung_table!` names.
pub struct Triso;

const HELIUM: Color32 = Color32::from_rgb(24, 28, 36);
const MATERIAL_COLOURS: [Color32; 7] = [
    Color32::from_rgb(214, 120, 46),  // UO2 kernel
    Color32::from_rgb(58, 58, 62),    // buffer
    Color32::from_rgb(150, 150, 154), // inner PyC
    Color32::from_rgb(200, 176, 112), // SiC
    Color32::from_rgb(150, 150, 154), // outer PyC
    Color32::from_rgb(92, 94, 98),    // matrix graphite
    Color32::from_rgb(74, 76, 80),    // shell graphite
];

static TAPES: std::sync::OnceLock<Vec<(&'static str, &'static str)>> = std::sync::OnceLock::new();
static CENTRES: std::sync::OnceLock<Vec<(f64, f64)>> = std::sync::OnceLock::new();

impl McRung for Triso {
    const INFO: RungInfo = RungInfo {
        name: "triso",
        title: "TRISO pebble: one neutron at a time",
        // No tutorial page for this rung yet; the deep dive explains the code it runs.
        lesson: "deep-dives/monte-carlo/index.html",
        spectrum: Spectrum::Thermal,
    };
    type Builder = model::DataBuilder;

    fn jobs() -> &'static [(&'static str, &'static str)] {
        TAPES.get_or_init(|| model::JOBS.iter().map(|j| (j.label, j.tape)).collect())
    }
    /// Always [`model::SPEED`], the loosened 0.01 tier (maintainer's choice
    /// for this demo, 2026-10-03).
    fn tier(_requested: Tier) -> Tier {
        Tier::Loose
    }
    /// Headless Chromium, 2026-10-03.
    fn job_weights(_tier: Tier) -> Vec<f64> {
        vec![35.1, 28.5, 0.2, 0.1, 0.2, 0.1, 0.1, 0.3, 0.3, 0.3, 21.5]
    }
    fn half_extent() -> f64 {
        model::half_pitch()
    }
    /// The cell, drawn from the same particle centres the geometry was built
    /// from. (The geometry REVIEW images required by the crate's drawing rule
    /// are rendered from the assembled geometry by `--render-geometry`.)
    fn draw(painter: &egui::Painter, rect: Rect, view: &View) {
        let centres = CENTRES.get_or_init(|| model::particle_centres(model::LAYOUT_SEED));
        let p = model::half_pitch();
        let (a, b) = (view.to_screen(rect, -p, p), view.to_screen(rect, p, -p));
        let cell = Rect::from_two_pos(a, b);
        painter.rect_filled(cell, 0.0, HELIUM);
        let s = view.scale as f32;
        let o = view.to_screen(rect, 0.0, 0.0);
        painter.circle_filled(o, model::PEBBLE_R as f32 * s, MATERIAL_COLOURS[model::MAT_SHELL]);
        painter.circle_filled(o, model::FUEL_ZONE_R as f32 * s, MATERIAL_COLOURS[model::MAT_MATRIX]);
        let radii = [
            (model::particle_r(), model::MAT_OPYC),
            (model::sic_r(), model::MAT_SIC),
            (model::ipyc_r(), model::MAT_IPYC),
            (model::buffer_r(), model::MAT_BUFFER),
            (model::KERNEL_R, model::MAT_KERNEL),
        ];
        let clip = painter.clip_rect().expand(8.0);
        for &(x, y) in centres {
            let c = view.to_screen(rect, x, y);
            if !clip.contains(c) {
                continue;
            }
            if model::particle_r() as f32 * s < 1.6 {
                painter.circle_filled(c, 1.2, MATERIAL_COLOURS[model::MAT_KERNEL]);
                continue;
            }
            for (r, m) in radii {
                painter.circle_filled(c, r as f32 * s, MATERIAL_COLOURS[m]);
            }
        }
        painter.rect_stroke(cell, 0.0, Stroke::new(1.5, Color32::from_rgb(120, 170, 255)), StrokeKind::Outside);
        painter.text(
            cell.left_top() + Vec2::new(6.0, 4.0),
            egui::Align2::LEFT_TOP,
            "reflective boundary  ·  helium (void)",
            egui::FontId::proportional(12.0),
            Color32::from_rgb(140, 170, 220),
        );
    }
    fn notes() -> &'static [&'static str] {
        &[
            "Transport: outram-mc-libs continuous-energy Monte Carlo, unmodified. Each neutron is a one-particle fixed-source run with fission progeny switched off, so every track is one real history, drawn projected onto the slice.",
            "Data: ENDF/B-VIII.0 (U-235, U-238, O-16, B-10, B-11, C-12, C-13, Si-28/29/30), reconstructed and Doppler-broadened to 296 K in this browser by OUTRAM PARK's NJOY port, in a background worker. Graphite carbon uses the crystalline-graphite S(α,β) thermal-scattering law.",
            "Covariance data (ENDF files 30–40) are removed before download: transport never reads them. A test proves the stripped tapes give bit-identical cross sections and fission spectra.",
            "Low fidelity, deliberately: reconstruction tolerance 0.01, not NJOY's 0.001.",
            "Geometry: HTR-10 pebble dimensions (IAEA-TECDOC-1382) in 2D. The TRISO particles are therefore infinitely long rods, not spheres: 152 of them, so the fuel fraction of the fuelled zone matches the real pebble (5.0 %). Rods self-shield differently from spheres, so this is a picture of how neutrons move, not a model of HTR-10.",
            "Approximations: carbon in the thin SiC layer is free gas; every fission's next neutron takes the U-235 fission spectrum; helium is void.",
            "Animation speed is proportional to the neutron's real speed (classical kinetic energy, v proportional to √E).",
            "Education and research only.",
        ]
    }
    fn legend(ui: &mut egui::Ui) {
        ui.strong("Materials");
        for (i, name) in model::MATERIAL_NAMES.iter().enumerate() {
            if i == model::MAT_OPYC {
                continue; // same colour and composition as inner PyC
            }
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                ui.painter().rect_filled(r, 2.0, MATERIAL_COLOURS[i]);
                ui.label(if i == model::MAT_IPYC { "Pyrolytic carbon (inner & outer)" } else { name });
            });
        }
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
            ui.painter().rect_filled(r, 2.0, HELIUM);
            ui.label("Helium coolant (modelled as void)");
        });
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
        Err("the TRISO rung has no k_eff mode".into())
    }
    fn keff_step(&mut self) -> Option<Generation> {
        None
    }
    fn keff_finished(&self) -> bool {
        true
    }
}
