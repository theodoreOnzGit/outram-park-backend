//! The `htr10` rung's **liberties toggle** (gh:#787): the lattice bed every
//! recorded k run uses, beside a random bed poured by the DEM, cut by the same
//! plane and holding the same number of balls.
//!
//! - **Lattice**: the ball list of `assemble_explicit_triso(14, 12, 0)`, the
//!   assembled core of the zoom ladder and of the recorded runs.
//! - **Random**: the gh:#216 `GranularSystem` bed (µ = 0.1, µ_r = 0; whole-core
//!   φ 0.6047 for all 27 554 pebbles against the published 0.61), cut to the
//!   lattice's ball count by keeping its lowest balls above the floor, as
//!   `nee_soon`'s DEM-bed builder does.
//!
//! Both are baked into `examples/common/htr10_beds.zz` (see
//! [`crate::htr10_beds`]); nothing is computed in the worker. ~~**No k_eff of
//! the random bed is shown or implied**: none has been measured.~~ **Since
//! 2026-10-08 (gh:#787)** the panel shows both recorded k, [`LATTICE_K`] and
//! [`RANDOM_K`], each from its native record; one pour, so the scatter
//! between pours is unmeasured.

use crate::htr10_beds::{self, draw, Bed, BedStats};
use dhoby_ghaut::web_demo::view::View;
use egui::{Color32, Pos2, Rect};

/// Which beds the main view shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    Both,
    Lattice,
    Random,
}

/// The cut plane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CutKind {
    /// The vertical plane `y = 0` through the axis.
    Side,
    /// A horizontal plane at a height the reader picks.
    Plan,
}

/// A recorded native k_eff: value, 1σ, and the record it is read from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RecordedK {
    pub k: f64,
    pub sigma: f64,
    pub record: &'static str,
}

/// The lattice at N = 12 on ENDF/B-VIII.0, 10 000 × [5 + 135], recorded
/// 2026-10-07 (`nee_soon/verification_and_validation/htr10_seker_2026_10_07_10k/`).
pub const LATTICE_K: RecordedK = RecordedK {
    k: 0.995125,
    sigma: 0.001055,
    record: "2026-10-07",
};

/// This random bed (the gh:#216 pour cut to the lattice's 16 681 balls),
/// same driver and statistics, recorded 2026-10-08
/// (`nee_soon/verification_and_validation/htr10_dem_bed_keff_2026_10_08/`).
pub const RANDOM_K: RecordedK = RecordedK {
    k: 0.989293,
    sigma: 0.000962,
    record: "2026-10-08",
};

/// `(random − lattice)` in pcm and its σ.
pub fn random_minus_lattice_pcm() -> (f64, f64) {
    (
        1.0e5 * (RANDOM_K.k - LATTICE_K.k),
        1.0e5 * RANDOM_K.sigma.hypot(LATTICE_K.sigma),
    )
}

/// The two beds, decoded once, at equal ball count.
pub struct Prepared {
    pub lattice: Bed,
    pub random: Bed,
    pub lattice_stats: BedStats,
    pub random_stats: BedStats,
    /// The whole poured bed (27 554), before the cut to the lattice's count.
    pub poured_stats: BedStats,
    pub layers: u32,
}

/// Decode the baked beds and cut the DEM bed to the lattice's ball count.
pub fn prepare(zz: &[u8]) -> Result<Prepared, String> {
    let beds = htr10_beds::decode(zz)?;
    let lattice_stats = beds.lattice.stats();
    let random = beds.dem.trimmed_to_core(lattice_stats.core);
    Ok(Prepared {
        random_stats: random.stats(),
        poured_stats: beds.dem.stats(),
        lattice_stats,
        lattice: beds.lattice,
        random,
        layers: beds.lattice_layers,
    })
}

/// Horizontal offsets (cm) of the lattice and random beds, and whether they
/// are stacked (a portrait screen) rather than side by side.
pub fn layout(which: Which, rect: Rect) -> ([f64; 2], [f64; 2], bool) {
    let stacked = rect.height() > rect.width();
    match which {
        Which::Lattice => ([0.0, 0.0], [f64::NAN, f64::NAN], false),
        Which::Random => ([f64::NAN, f64::NAN], [0.0, 0.0], false),
        Which::Both if stacked => ([0.0, 115.0], [0.0, -115.0], true),
        Which::Both => ([-100.0, 0.0], [100.0, 0.0], false),
    }
}

/// The world box (cm) a layout must fit: `(centre, half width, half height)`.
pub fn content_box(which: Which, cut: CutKind, stacked: bool) -> ([f64; 2], f64, f64) {
    // One bed: 180 cm across; a side cut runs from the valve (−62 cm) to
    // above the bed (~130 cm), a plan cut is 180 cm square.
    let (one_w, one_h, cy) = match cut {
        CutKind::Side => (95.0, 100.0, 35.0),
        CutKind::Plan => (95.0, 95.0, 0.0),
    };
    // Plus room for each bed's label above it and the summary line below.
    match which {
        Which::Both if stacked => ([0.0, cy + 5.0], one_w, one_h + 115.0 + 45.0),
        Which::Both => ([0.0, cy + 5.0], one_w + 100.0, one_h + 45.0),
        _ => ([0.0, cy + 5.0], one_w, one_h + 45.0),
    }
}

/// The `half_extent` that makes [`View::fit`] fit a `half_w` × `half_h` box
/// in `rect` (`View` fits a square to the smaller side).
pub fn half_extent_for(rect: Rect, half_w: f64, half_h: f64) -> f64 {
    let (w, h) = (
        f64::from(rect.width().max(1.0)),
        f64::from(rect.height().max(1.0)),
    );
    let scale = (w / (2.0 * half_w)).min(h / (2.0 * half_h));
    w.min(h) / (2.0 * scale)
}

pub struct BedsView {
    pub beds: Result<Prepared, String>,
    pub which: Which,
    pub cut: CutKind,
    /// Height of the plan cut above the bed floor, cm.
    pub plan_z_cm: f64,
    pub view: View,
}

impl BedsView {
    pub fn new() -> Self {
        let beds = prepare(htr10_beds::BAKED);
        Self {
            beds,
            which: Which::Both,
            cut: CutKind::Side,
            plan_z_cm: 61.8,
            view: View::new(200.0),
        }
    }

    /// The side panel. GUI drawing (exempt from the reaching-test rule: the
    /// numbers come from tested [`prepare`]).
    pub fn panel(&mut self, ui: &mut egui::Ui) {
        ui.label("The lattice bed every recorded k run uses, beside a random bed poured by the DEM, cut by the same plane and holding the same number of balls.");
        let before = (self.which, self.cut);
        ui.horizontal_wrapped(|ui| {
            ui.label("Show:");
            ui.selectable_value(&mut self.which, Which::Both, "both");
            ui.selectable_value(&mut self.which, Which::Lattice, "lattice");
            ui.selectable_value(&mut self.which, Which::Random, "random (DEM)");
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Cut:");
            ui.selectable_value(&mut self.cut, CutKind::Side, "side (through the axis)");
            ui.selectable_value(&mut self.cut, CutKind::Plan, "plan");
        });
        if before != (self.which, self.cut) {
            // Refit to the new picture.
            self.view.fitted = false;
            self.view.touched = false;
        }
        if self.cut == CutKind::Plan {
            ui.add(
                egui::Slider::new(&mut self.plan_z_cm, -30.0..=120.0)
                    .text("height above the floor, cm"),
            );
        }
        match &self.beds {
            Ok(b) => {
                egui::Grid::new("beds")
                    .num_columns(3)
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label("");
                        ui.strong("lattice");
                        ui.strong("random");
                        ui.end_row();
                        ui.label("balls above the floor");
                        ui.label(format!("{}", b.lattice_stats.core));
                        ui.label(format!("{}", b.random_stats.core));
                        ui.end_row();
                        ui.label("bed surface (p99 + r)");
                        ui.label(format!("{:.1} cm", 100.0 * b.lattice_stats.surface_m));
                        ui.label(format!("{:.1} cm", 100.0 * b.random_stats.surface_m));
                        ui.end_row();
                        ui.label("whole-core φ");
                        ui.label(format!("{:.4}", b.lattice_stats.phi_whole_core));
                        ui.label(format!("{:.4}", b.random_stats.phi_whole_core));
                        ui.end_row();
                        ui.label("k_eff (native, recorded)");
                        for r in [LATTICE_K, RANDOM_K] {
                            ui.label(format!("{:.5} ± {:.5} ({})", r.k, r.sigma, r.record));
                        }
                        ui.end_row();
                    });
                let (d, s) = random_minus_lattice_pcm();
                ui.label(format!(
                    "Random − lattice: {d:+.0} ± {s:.0} pcm ({:+.1}σ). One pour: the scatter between pours is not measured, so this is not yet the arrangement's worth.",
                    d / s
                ));
                ui.label(format!(
                    "The random bed is the lowest {} balls of the {}-pebble pour, whose whole-core φ is {:.4} against the published 0.61 (gh:#216).",
                    b.random_stats.core, b.poured_stats.total, b.poured_stats.phi_whole_core
                ));
            }
            Err(e) => {
                ui.colored_label(
                    Color32::from_rgb(255, 110, 110),
                    format!("beds unreadable: {e}"),
                );
            }
        }
        ui.separator();
        for line in NOTES {
            ui.label(format!("• {line}"));
        }
        ui.add(
            egui::Hyperlink::from_label_and_url(
                "Watch a DEM pour settle (demo)",
                format!("{}demos/dem/", dhoby_ghaut::web_demo::lesson::SITE),
            )
            .open_in_new_tab(true),
        );
    }

    /// The main view. GUI drawing (exempt; the cuts come from tested
    /// [`Bed::side_cut`] / [`Bed::plan_cut`], the layout from tested
    /// [`layout`] and [`content_box`]).
    pub fn draw(
        &mut self,
        ui: &egui::Ui,
        rect: Rect,
        painter: &egui::Painter,
        resp: &egui::Response,
    ) {
        let (lat_off, ran_off, stacked) = layout(self.which, rect);
        let (centre, hw, hh) = content_box(self.which, self.cut, stacked);
        self.view.half_extent = half_extent_for(rect, hw, hh);
        if !self.view.touched {
            self.view.home = centre;
        }
        self.view.handle_input(ui, resp);
        let Ok(b) = &self.beds else { return };
        let view = self.view;
        let z0 = (self.plan_z_cm / 100.0) as f32;
        let font = egui::FontId::proportional(13.0);
        let beds = [
            (
                &b.lattice,
                lat_off,
                Color32::from_rgb(120, 150, 205),
                format!("LATTICE, N = {}: the bed of every recorded k", b.layers),
            ),
            (
                &b.random,
                ran_off,
                Color32::from_rgb(200, 150, 100),
                format!(
                    "RANDOM (DEM pour): k {:.5} ± {:.5}, one pour",
                    RANDOM_K.k, RANDOM_K.sigma
                ),
            ),
        ];
        for (bed, off, colour, label) in beds {
            if off[0].is_nan() {
                continue;
            }
            let (dx, dz) = (off[0], off[1]);
            let shifted = |c: &[htr10_beds::Cut]| -> Vec<htr10_beds::Cut> {
                c.iter()
                    .map(|k| htr10_beds::Cut {
                        v: k.v + (dz / 100.0) as f32,
                        ..*k
                    })
                    .collect()
            };
            let label_at = match self.cut {
                CutKind::Side => {
                    // Wall and pebbles, lifted by dz.
                    let lifted = View {
                        centre: [view.centre[0], view.centre[1] - dz],
                        ..view
                    };
                    draw::vessel_side(painter, rect, &lifted, dx, 1.35);
                    draw::cuts(
                        painter,
                        rect,
                        &view,
                        dx,
                        &shifted(&bed.side_cut(0.0)),
                        colour,
                    );
                    view.to_screen(rect, dx - 90.0, dz + 142.0)
                }
                CutKind::Plan => {
                    let lifted = View {
                        centre: [view.centre[0], view.centre[1] - dz],
                        ..view
                    };
                    draw::vessel_plan(painter, rect, &lifted, dx, f64::from(z0));
                    draw::cuts(
                        painter,
                        rect,
                        &view,
                        dx,
                        &shifted(&bed.plan_cut(z0)),
                        colour,
                    );
                    view.to_screen(rect, dx - 90.0, dz + 100.0)
                }
            };
            painter.text(
                label_at,
                egui::Align2::LEFT_BOTTOM,
                label,
                font.clone(),
                colour,
            );
        }
        let summary = format!(
            "same {} balls in both; lattice bed {:.1} cm, φ {:.3} · random bed {:.1} cm, φ {:.3}",
            b.lattice_stats.core,
            100.0 * b.lattice_stats.surface_m,
            b.lattice_stats.phi_whole_core,
            100.0 * b.random_stats.surface_m,
            b.random_stats.phi_whole_core
        );
        let g = painter.layout(
            summary,
            egui::FontId::proportional(12.0),
            Color32::from_rgb(200, 206, 216),
            rect.width() - 24.0,
        );
        painter.galley(
            Pos2::new(rect.left() + 12.0, rect.bottom() - 40.0 - g.size().y),
            g,
            Color32::WHITE,
        );
    }
}

/// The liberty, as the panel states it.
pub const NOTES: [&str; 4] = [
    "Every recorded HTR-10 k (the layers view) is on the lattice: it is the bed both reference models use, so the comparison with them is like for like, but not with the reactor, whose bed is random.",
    "The random bed here is the LIGGGHTS port's GranularSystem pour of gh:#216 (µ = 0.1 graphite-on-graphite friction from the literature, not fitted), re-run on 2026-10-07 and byte-identical. Verification against LIGGGHTS, not validation: no measured bed is compared.",
    "At the same ball count the two beds stand at almost the same height; what differs is the arrangement: rows and gaps in the lattice, none in the random bed.",
    "~~What the random arrangement does to k has not been measured.~~ Measured 2026-10-08 on this one pour, natively at 10 000 × [5 + 135]: the random bed is about 580 pcm below the lattice at 4σ. One pour is one arrangement; how much pours scatter is not measured yet.",
];

#[cfg(test)]
mod tests {
    use super::*;

    /// The baked beds decode, the random bed is cut to the lattice's count,
    /// and the two stand within a few centimetres of each other.
    #[test]
    fn the_two_beds_hold_the_same_balls() {
        let b = prepare(htr10_beds::BAKED).expect("baked beds");
        assert_eq!(b.random_stats.core, b.lattice_stats.core);
        assert_eq!(b.poured_stats.total, 27_554);
        assert!((b.random_stats.surface_m - b.lattice_stats.surface_m).abs() < 0.05);
        assert!(b.lattice.side_cut(0.0).len() > 100 && b.random.side_cut(0.0).len() > 100);
    }

    /// The two k the panel shows are the ones in the native records.
    #[test]
    fn the_recorded_k_are_the_records() {
        let vv = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../nee_soon/verification_and_validation/"
        );
        let lat =
            std::fs::read_to_string(format!("{vv}htr10_seker_2026_10_07_10k/results_table.csv"))
                .expect("the lattice record");
        let row: Vec<&str> = lat
            .lines()
            .find(|l| l.starts_with("VIII.0,12,"))
            .expect("N = 12 on VIII.0")
            .split(',')
            .collect();
        assert_eq!((row[4], row[5]), ("0.995125", "0.001055"));
        assert_eq!(LATTICE_K.k, 0.995125);
        assert_eq!(LATTICE_K.sigma, 0.001055);
        let dem = std::fs::read_to_string(format!("{vv}htr10_dem_bed_keff_2026_10_08/results.csv"))
            .expect("the DEM-bed record");
        let row: Vec<&str> = dem.lines().nth(1).expect("one row").split(',').collect();
        assert_eq!((row[3], row[4], row[5]), ("16681", "0.989293", "0.000962"));
        assert_eq!(RANDOM_K.k, 0.989293);
        assert_eq!(RANDOM_K.sigma, 0.000962);
        let (d, s) = random_minus_lattice_pcm();
        assert!(
            (d + 583.2).abs() < 0.1 && (s - 142.8).abs() < 0.1,
            "{d} ± {s}"
        );
    }

    #[test]
    fn the_layout_stacks_on_a_portrait_screen_and_fits_its_box() {
        let phone = Rect::from_min_size(Pos2::ZERO, egui::vec2(390.0, 600.0));
        let desk = Rect::from_min_size(Pos2::ZERO, egui::vec2(900.0, 700.0));
        assert!(layout(Which::Both, phone).2 && !layout(Which::Both, desk).2);
        assert!(layout(Which::Lattice, phone).1[0].is_nan());
        for (r, stacked) in [(phone, true), (desk, false)] {
            let (_, hw, hh) = content_box(Which::Both, CutKind::Side, stacked);
            let mut v = View::new(half_extent_for(r, hw, hh));
            v.fit(r);
            let shown_w = f64::from(r.width()) / v.scale;
            let shown_h = f64::from(r.height()) / v.scale;
            assert!(
                shown_w >= 2.0 * hw * 0.999 && shown_h >= 2.0 * hh * 0.999,
                "{shown_w} x {shown_h} for {hw} x {hh}"
            );
        }
    }
}
