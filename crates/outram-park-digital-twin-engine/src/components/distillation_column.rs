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
        }
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
