//! The `lumped` rung: natural uranium gathered into a lump inside graphite,
//! one Wigner-Seitz cell with a white boundary (rung 3 of the Monte Carlo
//! ladder, gh:#525). Watch mode only; see `rungs.rs` for the contract.
//!
//! Same atoms in the same proportion as a homogeneous natural-uranium
//! mixture at `N_C/N_U` = 600, but the uranium is a metal sphere: neutrons
//! slow down in the graphite, and at a U-238 resonance the lump's skin
//! absorbs them, so its interior is shielded. The chain, the tapes and the
//! data processing are the `ugraphite` rung's ([`crate::ugraphite`]); only
//! the geometry and the birth region differ.

pub mod model;
#[cfg(not(target_arch = "wasm32"))]
pub mod render;

use crate::anim::Spectrum;
use crate::engine::{Tier, CHAIN_SEED};
use crate::rungs::{McRung, RungBuilder, RungInfo};
use crate::ugraphite::{self, sim, Loaded, GRAPHITE, OUTSIDE, URANIUM};
use dhoby_ghaut::web_demo::view::View;
use egui::{Color32, Rect, Sense, Stroke, Vec2};

/// The marker type `rung_table!` names.
pub struct Lumped;

impl McRung for Lumped {
    const INFO: RungInfo = RungInfo {
        name: "lumped",
        title: "Uranium in lumps: one cell",
        lesson: "tutorials/monte-carlo/lumped.html",
        spectrum: Spectrum::Thermal,
    };
    type Builder = Builder;

    fn jobs() -> &'static [(&'static str, &'static str)] {
        &ugraphite::model::JOBS
    }
    /// Always the loosened 0.01 tier: Watch mode is an illustration.
    fn tier(_requested: Tier) -> Tier {
        Tier::Loose
    }
    fn job_weights(_tier: Tier) -> Vec<f64> {
        ugraphite::JOB_WEIGHTS_LOOSE.to_vec()
    }
    fn half_extent() -> f64 {
        model::cell_r()
    }
    /// The cell cut through its centre: the lump, the graphite, the white
    /// boundary. Tracks are 3D and drawn projected, so they stay inside the
    /// outer circle.
    fn draw(painter: &egui::Painter, rect: Rect, view: &View) {
        let s = view.scale as f32;
        let o = view.to_screen(rect, 0.0, 0.0);
        let big_r = model::cell_r() as f32 * s;
        painter.rect_filled(painter.clip_rect(), 0.0, OUTSIDE);
        painter.circle_filled(o, big_r, GRAPHITE);
        painter.circle_filled(o, model::LUMP_R_CM as f32 * s, URANIUM);
        painter.circle_stroke(o, big_r, Stroke::new(1.5, Color32::from_rgb(235, 235, 245)));
        painter.text(
            o + Vec2::new(0.0, -big_r - 6.0),
            egui::Align2::CENTER_BOTTOM,
            format!("white boundary, R = {:.2} cm", model::cell_r()),
            egui::FontId::proportional(12.0),
            Color32::from_rgb(220, 220, 235),
        );
    }
    fn notes() -> &'static [&'static str] {
        &[
            "Model: rung 3's Wigner-Seitz cell. A sphere of natural uranium metal (19.05 g/cm³), radius 2 cm, at the centre of a graphite sphere (1.73 g/cm³), sized so the cell holds 600 carbon atoms per uranium atom, the same proportion as the homogeneous mixture it is compared with. The atoms are outram-mc-libs' `vv::ugraphite`, the cell `examples/lumped_ugraphite_kinf.rs`'s.",
            "Boundary: white. A neutron reaching the outer sphere is sent back in with a cosine-law direction, as if from a neighbouring cell. That approximates an infinite lattice of lumps; a mirror (specular) sphere would not, because it traps neutrons that miss the lump for ever.",
            "Data: ENDF/B-VIII.0 (U-234, U-235, U-238, C-12, C-13), processed in this browser by OUTRAM PARK's NJOY port at tolerance 0.01 (NJOY uses 0.001). Carbon carries the crystalline-graphite S(α,β) law; U-238 its URR probability tables and DBRC.",
            "Each track is one real history (a one-particle fixed-source run), drawn projected onto the slice. Chaining one neutron after the next is the illustration: a fission's next neutron is born at its site, a capture's somewhere random in the lump.",
            "Watch where captures happen: neutrons slowing down in the graphite are caught at the lump's surface by U-238's resonances, and few reach the inside. That is spatial self-shielding.",
            "There is no Run k_eff for this rung: the recorded k_inf against lump radius is on the lesson page.",
            "Animation speed is proportional to the neutron's real speed (classical kinetic energy, v proportional to √E).",
            "Education and research only. Not for reactor operation, licensing or safety decisions.",
        ]
    }
    fn loading_note(_tier: Tier) -> Option<&'static str> {
        Some("Six tapes, including graphite's thermal-scattering law: allow a minute or two in a browser, longer on a phone.")
    }
    fn legend(ui: &mut egui::Ui) {
        ui.strong("Materials");
        for (c, name) in [(URANIUM, "Natural uranium metal (the lump)"), (GRAPHITE, "Graphite"), (OUTSIDE, "Outside the cell")] {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                ui.painter().rect_filled(r, 2.0, c);
                ui.label(name);
            });
        }
    }
}

/// Data processing (the `ugraphite` rung's), then the cell.
pub struct Builder(ugraphite::model::DataBuilder);

impl RungBuilder for Builder {
    type Loaded = Loaded;
    fn new(tier: Tier) -> Self {
        Builder(ugraphite::model::DataBuilder::new(tier.speed()))
    }
    fn step(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.0.step(bytes)
    }
    fn finish(self) -> Result<Loaded, String> {
        let phys = sim::Physics {
            geometry: model::build_geometry(),
            materials: model::materials(),
            nuclides: self.0.finish()?,
            birth: sim::Birth::Ball { r: model::LUMP_R_CM },
        };
        Ok(Loaded { phys, chain: sim::Chain::new(CHAIN_SEED) })
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use outram_mc_libs::geometry::cell::SurfaceToken;
    use outram_mc_libs::geometry::position::{Direction, Position};
    use outram_mc_libs::physics::track_output::TrackEvent;
    use outram_mc_libs::vv::ugraphite as vv;

    /// The cell holds the requested cell-average ratio, and the assembled
    /// geometry puts uranium inside `r`, graphite between `r` and `R`, and no
    /// cell outside `R`.
    #[test]
    fn the_cell_is_a_uranium_lump_in_graphite_at_the_rung_three_ratio() {
        let (r, big_r) = (model::LUMP_R_CM, model::cell_r());
        let v = (r / big_r).powi(3);
        let n_u: f64 = vv::uranium_metal().u.iter().sum();
        let ratio = (1.0 - v) * vv::graphite().c / (v * n_u);
        assert!((ratio - model::C_PER_U).abs() < 1e-9 * model::C_PER_U, "cell-average N_C/N_U {ratio}");
        let g = model::build_geometry();
        let dir = Direction::new(0.0, 0.0, 1.0);
        let at = |x: f64, y: f64, z: f64| g.locate(Position::new(x, y, z), dir, SurfaceToken::NONE).map(|p| p.material);
        for p in [(0.0, 0.0, 0.0), (0.99 * r, 0.0, 0.0), (0.0, 0.0, -0.99 * r)] {
            assert_eq!(at(p.0, p.1, p.2), Some(Some(0)), "uranium at {p:?}");
        }
        for p in [(1.01 * r, 0.0, 0.0), (0.0, 0.5 * (r + big_r), 0.0), (0.0, 0.0, 0.99 * big_r), (0.57 * big_r, 0.57 * big_r, 0.57 * big_r)] {
            assert_eq!(at(p.0, p.1, p.2), Some(Some(1)), "graphite at {p:?}");
        }
        assert!(at(1.01 * big_r, 0.0, 0.0).is_none());
    }

    /// Real histories in the cell: none leaks (white boundary), each ends in
    /// capture or fission inside the cell, and some end in the lump. Not all:
    /// with 600 carbon atoms per uranium atom, graphite's thermal capture
    /// takes a visible share (that is `1 - f`).
    #[test]
    #[ignore = "processes 6 ENDF tapes (~1 min); run with --ignored"]
    fn histories_in_the_cell_end_properly() {
        let phys = sim::Physics {
            geometry: model::build_geometry(),
            materials: model::materials(),
            nuclides: crate::ugraphite::tests::load(Tier::Loose),
            birth: sim::Birth::Ball { r: model::LUMP_R_CM },
        };
        let mut chain = sim::Chain::new(3);
        let (n, mut thermal, mut in_lump) = (60, 0, 0);
        let big_r = model::cell_r() * (1.0 + 1e-9);
        let t = std::time::Instant::now();
        for _ in 0..n {
            let h = chain.run_next(&phys);
            assert!(matches!(h.outcome, Some(TrackEvent::Fission | TrackEvent::Absorption)), "history {}: {}", h.index, crate::history::outcome_name(h.outcome));
            assert_eq!(h.track.dropped_states, 0);
            for s in &h.track.states {
                assert!((s.r.x * s.r.x + s.r.y * s.r.y + s.r.z * s.r.z).sqrt() <= big_r, "history {} left the cell", h.index);
            }
            let end = h.track.states.last().expect("states").r;
            in_lump += ((end.x * end.x + end.y * end.y + end.z * end.z).sqrt() <= model::LUMP_R_CM * (1.0 + 1e-9)) as usize;
            thermal += h.thermalised() as usize;
        }
        eprintln!("  {n} histories in {:.2} s, {thermal} thermalised, {in_lump} ended in the lump", t.elapsed().as_secs_f64());
        // How many end in the lump is reported, not gated: the expected share
        // (resonance capture plus f times the thermal absorptions) comes from
        // the rung-3 scan's measured factors at this radius, and a threshold
        // guessed before that run is not a check.
        assert!(in_lump > 0, "no history ended in the lump");
        assert!(thermal > n / 3, "got {thermal} of {n} thermalised");
    }
}
