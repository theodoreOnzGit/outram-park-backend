//! The `natu` rung: a long bar of natural uranium metal, the first demo of
//! Neutronics Lesson 1, *Meet the Neutron* (gh:#906, #911). The reader sees
//! a neutron fly, scatter, change direction and energy, and end in a capture,
//! a fission or a leak, before any of those words is explained.
//!
//! **Real transport, nothing retyped.** Each track is one history traced by
//! outram-mc-libs (`run_fixed_source_traced`, one particle, through
//! [`crate::mc_demo::csgrung`], exactly as the benchmark rungs' Watch mode), on
//! ENDF/B-VIII.0 processed in the browser by the workspace's NJOY port. The
//! uranium is the natural-uranium reflector of ICSBEP IEU-MET-FAST-002
//! (Jemima), [`outram_mc_libs::vv::jemima`]'s `MATERIALS[MAT_REFLECTOR]`:
//! U-234, U-235 (0.72 % of the atoms) and U-238 at the densities the recorded
//! Jemima runs use, and the same three tapes.
//!
//! **Deliberate liberties** (stated in [`McRung::notes`]):
//! - the bar is an idealised shape (60 × 20 × 20 cm, vacuum outside), not an
//!   experiment;
//! - a fresh neutron is born at one point near the bar's left end, from the
//!   U-235 fission spectrum; after a fission the next neutron is born at that
//!   fission site (the chain is the illustration, each track is real);
//! - Watch only: no `k_eff` (a bare bar of natural uranium is far from
//!   critical, and that is not this lesson's question).
//!
//! **What a reader sees, measured** (`main.rs`,
//! `the_natural_uranium_rung_tracks_whole_histories_inside_the_bar`,
//! 2026-10-16, Watch tier, the chain's own seed): of 200 chained histories,
//! 140 leaked, 37 were captured and 23 caused fission, with 12.6 scatters per
//! history on average. Most leak: the bar is only 20 cm across, and a
//! neutron that stays fast in uranium flies centimetres between collisions.
//! That is fine for a first look at a random walk; the lesson does not quote
//! these numbers.

use crate::mc_demo::anim::Spectrum;
use crate::mc_demo::csgrung::{self, CsgCase};
use crate::mc_demo::engine::Tier;
use crate::mc_demo::keff::Reference;
use crate::mc_demo::rungs::{McRung, RungInfo};
use crate::web_demo::view::View;
use egui::{Color32, Rect, Sense, Stroke, StrokeKind, Vec2};

/// The case, its bar and its tapes: `nee_soon::mc_rungs::natu` (moved
/// 2026-11-25, gh:#1742), the type `table.rs` names.
pub use nee_soon::mc_rungs::natu::{Natu, HALF_X, HALF_YZ, JOBS};

const URANIUM: Color32 = Color32::from_rgb(96, 84, 128);

impl CsgCase for Natu {
    fn reference() -> Option<Reference> {
        None
    }
}

impl McRung for Natu {
    const INFO: RungInfo = RungInfo {
        name: "natu",
        title: "Natural uranium: meet the neutron",
        lesson: "science/neutronics/meet-the-neutron.html",
        spectrum: Spectrum::Fast,
    };
    type Builder = csgrung::Builder<Natu>;

    fn jobs() -> &'static [(&'static str, &'static str)] {
        &JOBS
    }
    /// 0.001 (gh:#876 item 3; ~~the loosened 0.01 tier~~ until 2026-10-16).
    fn tier(_requested: Tier) -> Tier {
        Tier::Exact
    }
    /// The Jemima rung's weights for the same three tapes (progress bar only).
    fn job_weights(_tier: Tier) -> Vec<f64> {
        vec![1.0, 11.2, 11.6]
    }
    fn half_extent() -> f64 {
        HALF_X
    }
    /// The bar seen from above, with the birth point.
    fn draw(painter: &egui::Painter, rect: Rect, view: &View) {
        let a = view.to_screen(rect, -HALF_X, -HALF_YZ);
        let b = view.to_screen(rect, HALF_X, HALF_YZ);
        let r = Rect::from_two_pos(a, b);
        painter.rect_filled(r, 0.0, URANIUM);
        painter.rect_stroke(r, 0.0, Stroke::new(1.5, bugis::Ink::of_ctx(painter.ctx()).accent(3)), StrokeKind::Outside);
        painter.text(
            r.center_top() + Vec2::new(0.0, -6.0),
            egui::Align2::CENTER_BOTTOM,
            format!("natural uranium metal, {} × {} × {} cm · vacuum outside (leaks)", 2.0 * HALF_X, 2.0 * HALF_YZ, 2.0 * HALF_YZ),
            egui::FontId::proportional(12.0),
            bugis::Ink::of_ctx(painter.ctx()).accent(3),
        );
    }
    fn notes() -> &'static [&'static str] {
        &[
            "Each track is one real neutron history, traced by outram-mc-libs (a Rust port of OpenMC's transport) on ENDF/B-VIII.0 data that this browser processed with OUTRAM PARK's NJOY port, at NJOY's tolerance 0.001.",
            "The uranium is natural uranium metal: the reflector material of the ICSBEP Jemima benchmark (U-234, U-235 at 0.72 % of the atoms, U-238), at 293.6 K, as outram-mc-libs' vv::jemima records it.",
            "Liberties: the bar (60 × 20 × 20 cm, vacuum outside) is an idealised shape, not an experiment. A fresh neutron is born at x = −20 cm on the axis from the U-235 fission spectrum; after a fission the next neutron starts at that fission site. That chaining is the illustration; each track is real.",
            "Tracks are 3D, drawn projected from above, so a neutron can appear to end inside the bar when it left through the top or bottom face.",
            "Education and research only. Not for reactor operation, licensing or safety decisions.",
        ]
    }
    fn legend(ui: &mut egui::Ui) {
        ui.strong("Material");
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
            ui.painter().rect_filled(r, 2.0, URANIUM);
            ui.label("Natural uranium metal");
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nee_soon::mc_rungs::natu::{geometry, material, BIRTH_X};
    use outram_mc_libs::vv::jemima;

    /// The bar is one cell that holds its own centre and the birth point,
    /// and the material is Jemima's natural uranium, unchanged.
    #[test]
    fn the_bar_is_natural_uranium_and_holds_the_birth_point() {
        let g = geometry();
        assert_eq!(g.cells.len(), 1);
        let m = material();
        let (_, comps) = jemima::MATERIALS[jemima::MAT_REFLECTOR];
        assert_eq!(m.components.len(), comps.len());
        for (c, (_, d)) in m.components.iter().zip(comps) {
            assert_eq!(c.atom_density, *d);
        }
        let u5 = m.components[1].atom_density / m.components.iter().map(|c| c.atom_density).sum::<f64>();
        assert!((u5 - 0.0072).abs() < 0.0001, "U-235 atom fraction {u5}");
        assert!(BIRTH_X.abs() < HALF_X);
    }
}
