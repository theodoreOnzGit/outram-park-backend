//! The `ugraphite` rung: uranium mixed evenly through graphite, an infinite
//! homogeneous medium (rung 2 of the Monte Carlo ladder, gh:#524). Watch mode
//! only, like `triso/` and `lct008/`; see `rungs.rs` for the contract.
//!
//! The mixture is rung 2's main case (one HTR-10 fuel pebble's uranium and
//! carbon, 17 wt% U-235, `N_C/N_U` = 767.2) in the reflective cube of
//! `examples/ugraphite_four_factor.rs`, from outram-mc-libs'
//! `vv::ugraphite`. Each track is one real history: fast birth, a hundred-odd
//! collisions with carbon, the S(α,β) / free-gas / at-rest fork at the bottom,
//! and an end in U-235 fission or a capture (in U-238's resonances on the way
//! down, or thermal). No Run k_eff here (see the notes).
//!
//! [`sim`] is shared with the `lumped` rung: the same chain, a different
//! birth region.

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
pub struct Ugraphite;

pub(crate) const MIXTURE: Color32 = Color32::from_rgb(78, 76, 84);
pub(crate) const GRAPHITE: Color32 = Color32::from_rgb(92, 94, 98);
pub(crate) const URANIUM: Color32 = Color32::from_rgb(214, 120, 46);
pub(crate) const OUTSIDE: Color32 = Color32::from_rgb(18, 20, 26);

/// Progress-bar weights for the six jobs at tolerance 0.01 (U-234, U-235,
/// U-238, C-12, C-13, graphite S(α,β)): native seconds measured by
/// `histories_in_the_mixture_end_properly_and_thermalise` (ignored test) on
/// 2026-10-05: 1.8 / 26.4 / 23.7 / 0.0 / 0.0 / 16.1 s (the 0.0 s jobs floored
/// at 0.1). Hardware: Intel Xeon @ 2.10 GHz, one thread pinned to 2 of 4
/// logical cores, 15 GB, Linux, CPU only, machine shared and loaded (load
/// average ~8). Weights only; shared with the `lumped` rung.
pub(crate) const JOB_WEIGHTS_LOOSE: [f64; 6] = [1.8, 26.4, 23.7, 0.1, 0.1, 16.1];

impl McRung for Ugraphite {
    const INFO: RungInfo = RungInfo {
        name: "ugraphite",
        title: "Uranium mixed into graphite",
        lesson: "tutorials/monte-carlo/ugraphite.html",
        spectrum: Spectrum::Thermal,
    };
    type Builder = Builder;

    fn jobs() -> &'static [(&'static str, &'static str)] {
        &model::JOBS
    }
    /// Always the loosened 0.01 tier: Watch mode is an illustration.
    fn tier(_requested: Tier) -> Tier {
        Tier::Loose
    }
    fn job_weights(_tier: Tier) -> Vec<f64> {
        JOB_WEIGHTS_LOOSE.to_vec()
    }
    fn half_extent() -> f64 {
        model::HALF_CM
    }
    /// The cube seen from above: one uniform mixture, reflective walls.
    fn draw(painter: &egui::Painter, rect: Rect, view: &View) {
        let h = model::HALF_CM;
        painter.rect_filled(painter.clip_rect(), 0.0, OUTSIDE);
        let cell = Rect::from_two_pos(view.to_screen(rect, -h, h), view.to_screen(rect, h, -h));
        painter.rect_filled(cell, 0.0, MIXTURE);
        painter.rect_stroke(cell, 0.0, Stroke::new(1.5, Color32::from_rgb(120, 170, 255)), StrokeKind::Outside);
        painter.text(
            cell.left_top() + Vec2::new(6.0, 4.0),
            egui::Align2::LEFT_TOP,
            "reflective walls: an infinite mixture",
            egui::FontId::proportional(12.0),
            Color32::from_rgb(140, 170, 220),
        );
    }
    fn notes() -> &'static [&'static str] {
        &[
            "Model: rung 2's main case, the uranium and carbon of one HTR-10 fuel pebble (17 wt% U-235) smeared evenly, N_C/N_U = 767.2, at 296 K, in a cube with reflective walls: an infinite homogeneous medium. The atoms are outram-mc-libs' `vv::ugraphite`, the model the recorded k_inf = 1.56777 ± 0.00081 was measured on (lesson page).",
            "Data: ENDF/B-VIII.0 (U-234, U-235, U-238, C-12, C-13), processed in this browser by OUTRAM PARK's NJOY port at tolerance 0.01 (NJOY uses 0.001). Carbon carries the crystalline-graphite S(α,β) law; U-238 its URR probability tables and DBRC.",
            "Each track is one real history (a one-particle fixed-source run), drawn projected from above. Chaining one neutron after the next is the illustration: a fission's next neutron is born at its site, a capture's somewhere random.",
            "Watch a neutron slow down: it is born at about 2 MeV and loses on average 16 % of its energy per carbon collision, so it takes a hundred-odd collisions to reach thermal. Some are caught on the way by U-238's resonances; that is the resonance escape probability p.",
            "There is no Run k_eff for this rung: the recorded k_inf and its four factors are on the lesson page.",
            "Animation speed is proportional to the neutron's real speed (classical kinetic energy, v proportional to √E).",
            "Education and research only. Not for reactor operation, licensing or safety decisions.",
        ]
    }
    fn loading_note(_tier: Tier) -> Option<&'static str> {
        Some("Six tapes, including graphite's thermal-scattering law: allow a minute or two in a browser, longer on a phone.")
    }
    fn legend(ui: &mut egui::Ui) {
        ui.strong("Materials");
        for (c, name) in [(MIXTURE, "Uranium (17 wt% U-235) mixed evenly into graphite"), (OUTSIDE, "Outside: reflective walls")] {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                ui.painter().rect_filled(r, 2.0, c);
                ui.label(name);
            });
        }
    }
}

/// Data processing, then the cube.
pub struct Builder(model::DataBuilder);

impl RungBuilder for Builder {
    type Loaded = Loaded;
    fn new(tier: Tier) -> Self {
        Builder(model::DataBuilder::new(tier.speed()))
    }
    fn step(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.0.step(bytes)
    }
    fn finish(self) -> Result<Loaded, String> {
        let phys = sim::Physics {
            geometry: model::build_geometry(),
            materials: model::materials(),
            nuclides: self.0.finish()?,
            birth: sim::Birth::Cube { half: model::HALF_CM },
        };
        Ok(Loaded { phys, chain: sim::Chain::new(CHAIN_SEED) })
    }
}

/// A rung of this family once its data are processed (shared with `lumped`).
pub struct Loaded {
    pub phys: sim::Physics,
    pub chain: sim::Chain,
}

impl LoadedRung for Loaded {
    fn run_next(&mut self) -> History {
        self.chain.run_next(&self.phys)
    }
    fn keff_start(&mut self, _cfg: KeffConfig) -> Result<Vec<[f32; 2]>, String> {
        Err("this rung has no k_eff mode".into())
    }
    fn keff_step(&mut self) -> Option<Generation> {
        None
    }
    fn keff_finished(&self) -> bool {
        true
    }
    /// The σ(E) panel (gh:#549): U-238 capture, U-235 fission, and C-12
    /// scattering with graphite's S(α,β) law below its cutoff (C-12 is 98.9 %
    /// of the carbon; C-13 is not drawn).
    fn xs_curves(&self) -> Vec<crate::xs::XsCurve> {
        use crate::xs::Channel;
        crate::xs::curves(
            &self.phys.nuclides,
            model::TEMPERATURE_K,
            &[
                ("U-238 capture", model::N_U238, Channel::Capture),
                ("U-235 fission", model::N_U235, Channel::Fission),
                ("C-12 scattering (bound in graphite below the S(α,β) cutoff)", model::N_C12, Channel::Scatter),
            ],
        )
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) mod tests {
    use super::*;
    use outram_mc_libs::geometry::cell::SurfaceToken;
    use outram_mc_libs::geometry::position::{Direction, Position};
    use outram_mc_libs::physics::track_output::TrackEvent;

    /// The mixture is rung 2's main case (the ratio the records quote) and the
    /// cube is that mixture everywhere inside, nothing outside.
    #[test]
    fn the_cube_is_rung_twos_mixture_everywhere() {
        assert!((model::mixture().c_per_u() - 767.2).abs() < 0.05);
        let g = model::build_geometry();
        let h = model::HALF_CM;
        let dir = Direction::new(0.0, 0.0, 1.0);
        let at = |x: f64, y: f64, z: f64| g.locate(Position::new(x, y, z), dir, SurfaceToken::NONE).map(|p| p.material);
        for p in [(0.0, 0.0, 0.0), (0.99 * h, -0.99 * h, 0.5 * h), (-0.7 * h, 0.2 * h, -0.99 * h)] {
            assert_eq!(at(p.0, p.1, p.2), Some(Some(0)), "inside at {p:?}");
        }
        assert!(at(1.01 * h, 0.0, 0.0).is_none());
        assert_eq!(model::materials().len(), 1);
    }

    /// Process the six tapes natively at the Watch tier, as the worker does.
    pub(crate) fn load(tier: Tier) -> Vec<outram_mc_libs::material::nuclide::Nuclide> {
        let mut b = model::DataBuilder::new(tier.speed());
        for (label, tape) in model::JOBS {
            let t = std::time::Instant::now();
            let path = njoy_outram_park_fork::reference_data::reference_endf(tape).expect("tape");
            let raw = std::fs::read(path).expect("read");
            b.step(&crate::tapes::strip_covariances(&raw)).expect("step");
            eprintln!("  {label:<16} {:6.1} s", t.elapsed().as_secs_f64());
        }
        b.finish().expect("finish")
    }

    /// Process the tapes (tolerance 0.01) and run real histories through the
    /// mixture: none leaks (reflective walls), each ends in capture or
    /// fission inside the cube, and nearly all reach thermal energies before
    /// they end (graphite moderates; at this ratio most absorption is
    /// thermal). Needs `reference-data/endf/` and about a minute natively, so
    /// it is opt-in, as the crate's other data tests.
    #[test]
    #[ignore = "processes 6 ENDF tapes (~1 min); run with --ignored"]
    fn histories_in_the_mixture_end_properly_and_thermalise() {
        let phys = sim::Physics {
            geometry: model::build_geometry(),
            materials: model::materials(),
            nuclides: load(Tier::Loose),
            birth: sim::Birth::Cube { half: model::HALF_CM },
        };
        assert!(phys.nuclides[3].sample_thermal(0.01, &mut 1).is_some(), "C-12 carries the graphite law");
        let mut chain = sim::Chain::new(3);
        let (mut thermal, mut fissions, n) = (0, 0, 60);
        let t = std::time::Instant::now();
        let h_cm = model::HALF_CM * (1.0 + 1e-9);
        for _ in 0..n {
            let h = chain.run_next(&phys);
            assert!(
                matches!(h.outcome, Some(TrackEvent::Fission | TrackEvent::Absorption)),
                "history {}: {}",
                h.index,
                crate::history::outcome_name(h.outcome)
            );
            assert_eq!(h.track.dropped_states, 0);
            for s in &h.track.states {
                assert!(s.r.x.abs() <= h_cm && s.r.y.abs() <= h_cm && s.r.z.abs() <= h_cm, "history {} left the cube", h.index);
            }
            thermal += h.thermalised() as usize;
            fissions += (h.outcome == Some(TrackEvent::Fission)) as usize;
        }
        eprintln!("  {n} histories in {:.2} s, {thermal} thermalised, {fissions} fissions", t.elapsed().as_secs_f64());
        assert!(thermal > n / 2, "graphite thermalises most neutrons; got {thermal} of {n}");
    }
}
