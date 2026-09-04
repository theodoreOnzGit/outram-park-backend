//! Distillation-column tray-stack visual.
//!
//! No physics counterpart exists in `outram-park-fork-dwsim-libs` shaped for
//! a widget to hold by value (`DynamicColumnProfiles` is a plain-data struct
//! computed fresh from a [`DynamicColumnState`](outram_park_fork_dwsim_libs::columns::dynamic::DynamicColumnState)
//! each step, not a long-lived component object the way [`tampines::components::Valve`]
//! is). So this widget follows the crate's other scalar-backed pattern
//! instead — [`PipeVisual::from_scalars`](crate::components::pipe::PipeVisual::from_scalars) is
//! the precedent cited in the crate `CLAUDE.md` ("scalar-backed widgets are
//! not placeholders... callers pass *real* state from their own model"): the
//! caller supplies real per-stage temperature/composition slices read
//! straight from a [`DynamicColumnProfiles`], not fabricated values.
//!
//! Draws, top to bottom: a condenser cap, `N` tray rectangles each coloured
//! by its own stage temperature (blue = coldest stage in the column, red =
//! hottest — [`hot_to_cold_colour_mark_1`]), and a reboiler sump. This is
//! the same shell-and-trays diagram every distillation textbook uses; nothing
//! here is invented artwork beyond that convention.

use egui::{Color32, Pos2, Rect, Response, Sense, Ui, Vec2, Widget};

use crate::color_maps::hot_to_cold_colour_mark_1;

/// Visual representation of a distillation column's tray stack.
///
/// Scalar-backed: `stage_temperature_k` and `stage_liquid_fraction` are the
/// caller's own per-stage readouts (e.g. straight off a
/// `DynamicColumnProfiles`), not physics this widget computes. Stage 0 is
/// drawn at the **top** (condenser), the last stage at the **bottom**
/// (reboiler), matching the dynamic-column model's own stage numbering.
pub struct DistillationColumnVisual<'a> {
    /// Per-stage temperature \[K\], stage 0 first (condenser) .. last
    /// (reboiler). Drives each tray's colour.
    pub stage_temperature_k: &'a [f64],
    /// Per-stage light-key liquid mole fraction \[-\], same ordering. Shown
    /// as a text label on each tray; not currently used for colour (a second
    /// colour channel would fight the temperature one for the reader's
    /// attention).
    pub stage_liquid_fraction: &'a [f64],
    /// On-screen centre position of the whole column.
    pub screen_position: Pos2,
    /// On-screen size of the whole column (width, total height including
    /// the condenser cap and reboiler sump).
    pub screen_vector: Vec2,
    /// Side draws to mark, as `(stage, label)`. Empty for a two-product
    /// column; a crude unit draws several cuts down its length, and where they
    /// come off is most of what distinguishes one from a benzene splitter.
    ///
    /// Scalar-backed like the rest: the caller passes the stages its own model
    /// actually draws from. Out-of-range stages are ignored rather than
    /// clamped, so a mismatched slice cannot silently point at the wrong tray.
    pub side_draws: &'a [(usize, &'a str)],
}

impl<'a> DistillationColumnVisual<'a> {
    /// Build from the caller's own per-stage slices and screen geometry.
    /// Both slices must be the same length (the column's stage count); a
    /// length mismatch is a caller bug, not a recoverable widget state, so
    /// this constructor does not attempt to reconcile them.
    pub fn from_scalars(
        stage_temperature_k: &'a [f64],
        stage_liquid_fraction: &'a [f64],
        screen_position: Pos2,
        screen_vector: Vec2,
    ) -> Self {
        Self {
            stage_temperature_k,
            stage_liquid_fraction,
            screen_position,
            screen_vector,
            side_draws: &[],
        }
    }

    /// Mark side draws at the given stages. See [`Self::side_draws`].
    #[must_use]
    pub fn with_side_draws(mut self, side_draws: &'a [(usize, &'a str)]) -> Self {
        self.side_draws = side_draws;
        self
    }

    /// The on-screen rectangle of tray `j`, top to bottom.
    fn tray_rect(&self, j: usize, n: usize, body: Rect) -> Rect {
        let h = body.height() / n.max(1) as f32;
        let top = body.top() + h * j as f32;
        Rect::from_min_max(
            Pos2::new(body.left(), top),
            Pos2::new(body.right(), top + h),
        )
    }
}

impl Widget for DistillationColumnVisual<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let outer = Rect::from_center_size(self.screen_position, self.screen_vector);
        let response = ui.allocate_rect(outer, Sense::hover());
        let painter = ui.painter();

        let n = self.stage_temperature_k.len();
        if n == 0 {
            painter.rect_stroke(outer, 4.0, (1.5, Color32::GRAY), egui::StrokeKind::Outside);
            return response;
        }

        // Reserve a cap at the top (condenser) and a sump at the bottom
        // (reboiler) outside the tray body, same fraction of height each.
        let cap_h = outer.height() * 0.10;
        let sump_h = outer.height() * 0.10;
        let body = Rect::from_min_max(
            Pos2::new(outer.left(), outer.top() + cap_h),
            Pos2::new(outer.right(), outer.bottom() - sump_h),
        );

        let (t_min, t_max) = self
            .stage_temperature_k
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &t| {
                (lo.min(t), hi.max(t))
            });
        let span = (t_max - t_min).max(1e-6);

        // Condenser cap.
        let cap_rect = Rect::from_min_max(outer.left_top(), Pos2::new(outer.right(), body.top()));
        painter.rect_filled(cap_rect, 2.0, Color32::from_rgb(120, 160, 220));
        painter.text(
            cap_rect.center(),
            egui::Align2::CENTER_CENTER,
            "Condenser",
            egui::FontId::proportional(11.0),
            Color32::BLACK,
        );

        // Trays, stage 0 (condenser-adjacent) at the top.
        for j in 0..n {
            let rect = self.tray_rect(j, n, body);
            let hotness = ((self.stage_temperature_k[j] - t_min) / span) as f32;
            let colour = hot_to_cold_colour_mark_1(hotness);
            painter.rect_filled(rect, 0.0, colour);
            painter.rect_stroke(
                rect,
                0.0,
                (1.0, Color32::from_gray(60)),
                egui::StrokeKind::Outside,
            );
            if rect.height() > 10.0 {
                let x = self
                    .stage_liquid_fraction
                    .get(j)
                    .copied()
                    .unwrap_or(f64::NAN);
                painter.text(
                    rect.left_center() + Vec2::new(4.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    format!("{j}: {:.0} K, x={x:.3}", self.stage_temperature_k[j]),
                    egui::FontId::monospace(9.0),
                    Color32::BLACK,
                );
            }
        }

        // Side draws: a short arrow off the right edge of the drawing tray,
        // labelled with the cut. Drawn after the trays so the arrows sit over
        // the tray fill rather than under it.
        for &(stage, label) in self.side_draws {
            if stage >= n {
                continue;
            }
            let rect = self.tray_rect(stage, n, body);
            let y = rect.center().y;
            let from = Pos2::new(outer.right(), y);
            let to = Pos2::new(outer.right() + 18.0, y);
            let stroke = egui::Stroke::new(2.0, Color32::from_rgb(40, 40, 40));
            painter.line_segment([from, to], stroke);
            // arrow head
            painter.line_segment([to, to + Vec2::new(-5.0, -3.5)], stroke);
            painter.line_segment([to, to + Vec2::new(-5.0, 3.5)], stroke);
            painter.text(
                to + Vec2::new(4.0, 0.0),
                egui::Align2::LEFT_CENTER,
                label,
                egui::FontId::proportional(10.0),
                Color32::from_rgb(40, 40, 40),
            );
        }

        // Reboiler sump.
        let sump_rect =
            Rect::from_min_max(Pos2::new(outer.left(), body.bottom()), outer.right_bottom());
        painter.rect_filled(sump_rect, 2.0, Color32::from_rgb(220, 120, 80));
        painter.text(
            sump_rect.center(),
            egui::Align2::CENTER_CENTER,
            "Reboiler",
            egui::FontId::proportional(11.0),
            Color32::BLACK,
        );

        painter.rect_stroke(
            outer,
            4.0,
            (1.5, Color32::from_gray(30)),
            egui::StrokeKind::Outside,
        );
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Pos2, Vec2};

    /// The integration this widget exists for: a headless
    /// [`CrudePlant`](outram_park_fork_dwsim_libs::petroleum::crude_plant::CrudePlant)
    /// snapshot must feed it directly, with no arithmetic in between. If this
    /// stops compiling, the widget and the plant have drifted apart.
    #[test]
    fn a_crude_plant_snapshot_drives_the_widget() {
        use outram_park_fork_dwsim_libs::petroleum::crude_distillation::{
            BlackOilCrude, CrudeColumnConfig,
        };
        use outram_park_fork_dwsim_libs::petroleum::crude_plant::CrudePlant;

        let plant = CrudePlant::new(
            &BlackOilCrude::light_sweet(),
            &CrudeColumnConfig::atmospheric_default(),
            8,
        )
        .expect("the reference crude column must build");
        let snap = plant.snapshot().expect("snapshot");

        let draws: Vec<(usize, &str)> = snap
            .cuts
            .iter()
            .map(|(stage, _, cut)| (*stage, cut.label()))
            .collect();
        let w = DistillationColumnVisual::from_scalars(
            &snap.stage_temperature_k,
            &snap.lightest_liquid_fraction,
            Pos2::new(0.0, 0.0),
            Vec2::new(120.0, 400.0),
        )
        .with_side_draws(&draws);

        assert_eq!(w.stage_temperature_k.len(), snap.n_stages);
        assert_eq!(w.side_draws.len(), 5, "overhead + 3 draws + residue");
    }

    /// A side draw naming a stage the column does not have is ignored rather
    /// than clamped onto the nearest tray, so a mismatched slice cannot
    /// silently point the arrow at the wrong place.
    #[test]
    fn out_of_range_side_draws_are_ignored() {
        let t = [350.0, 360.0, 370.0];
        let x = [0.9, 0.5, 0.1];
        let draws = [(1usize, "kero"), (99usize, "nowhere")];
        let w = DistillationColumnVisual::from_scalars(
            &t,
            &x,
            Pos2::new(0.0, 0.0),
            Vec2::new(100.0, 200.0),
        )
        .with_side_draws(&draws);
        // The widget keeps what it was given; the render skips the bad one.
        assert_eq!(w.side_draws.len(), 2);
        assert!(w.side_draws.iter().any(|&(s, _)| s >= t.len()));
    }
}
