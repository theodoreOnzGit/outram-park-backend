//! Process schematic: the column tray stack plus labelled stream arrows for
//! feed, reflux, distillate, and bottoms.
//!
//! Composes [`outram_park_digital_twin_engine::components::DistillationColumnVisual`]
//! (op-akkj) for the column body -- real per-stage temperature/composition
//! read straight off the snapshot, not fabricated. The four external streams
//! are drawn as plain labelled arrows rather than through
//! `components::pipe::PipeVisual`: that widget's scalar path needs a mass
//! flow rate, and this plant's flows are molar (`DynamicColumnProfiles` is a
//! molar-balance model, no molar-mass-weighted mass flow is computed
//! anywhere in this plant), so routing through it would mean inventing a
//! mass flow this model does not produce.

use egui::{Color32, Pos2, Stroke, Ui, Vec2};

use outram_park_digital_twin_engine::components::DistillationColumnVisual;

use super::state::ColumnSnapshot;

/// Draw the column schematic: tray stack in the centre, one labelled stream
/// arrow per external connection.
pub fn draw_schematic(ui: &mut Ui, snapshot: &ColumnSnapshot) {
    let desired = Vec2::new(ui.available_width().max(500.0), 560.0);
    let (rect, _response) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let painter = ui.painter_at(rect);

    let column_centre = rect.center();
    let column_size = Vec2::new(160.0, 460.0);

    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.put(
            egui::Rect::from_center_size(column_centre, column_size),
            DistillationColumnVisual::from_scalars(
                &snapshot.stage_temperature_k,
                &snapshot.liquid_benzene_fraction,
                column_centre,
                column_size,
            ),
        );
    });

    let top = Pos2::new(column_centre.x, column_centre.y - column_size.y / 2.0);
    let bottom = Pos2::new(column_centre.x, column_centre.y + column_size.y / 2.0);
    let left = column_centre.x - column_size.x / 2.0;
    let right = column_centre.x + column_size.x / 2.0;

    // Feed, into the side of the column at the feed stage's approximate height.
    let feed_frac = crate::physics::column_config::FEED_STAGE as f32
        / crate::physics::column_config::N_STAGES.max(1) as f32;
    let feed_y = top.y + column_size.y * feed_frac;
    draw_stream_arrow(
        &painter,
        Pos2::new(left - 90.0, feed_y),
        Pos2::new(left, feed_y),
        Color32::from_rgb(90, 140, 90),
        &format!(
            "Feed {:.2} mol/s\n(z_benzene = {:.2})",
            crate::physics::column_config::FEED_FLOW_MOL_S,
            crate::physics::column_config::FEED_Z_BENZENE,
        ),
    );

    // Reflux, back into the top of the column.
    draw_stream_arrow(
        &painter,
        Pos2::new(right + 90.0, top.y - 20.0),
        Pos2::new(right, top.y - 20.0),
        Color32::from_rgb(90, 130, 220),
        "Reflux\n(returns to stage 0)",
    );

    // Distillate, drawn off the top.
    draw_stream_arrow(
        &painter,
        top,
        Pos2::new(top.x, top.y - 70.0),
        Color32::from_rgb(90, 130, 220),
        &format!("Distillate\n{:.4} mol/s", snapshot.distillate_mol_s),
    );

    // Bottoms, drawn off the bottom.
    draw_stream_arrow(
        &painter,
        bottom,
        Pos2::new(bottom.x, bottom.y + 70.0),
        Color32::from_rgb(220, 130, 90),
        &format!("Bottoms\n{:.4} mol/s", snapshot.bottoms_mol_s),
    );

    ui.label(format!(
        "Reflux ratio R = {:.2}   |   Reboiler duty = {:.0} W   |   t = {:.0} s",
        snapshot.reflux_ratio, snapshot.reboiler_duty_watts, snapshot.sim_time_s
    ));
}

/// A single straight arrow from `from` to `to`, with a caption drawn beside
/// its midpoint. Deliberately minimal -- see the module doc for why this
/// does not go through `components::pipe::PipeVisual`.
fn draw_stream_arrow(
    painter: &egui::Painter,
    from: Pos2,
    to: Pos2,
    colour: Color32,
    caption: &str,
) {
    painter.arrow(from, to - from, Stroke::new(2.5, colour));
    let mid = from + (to - from) * 0.5;
    painter.text(
        mid + Vec2::new(6.0, -6.0),
        egui::Align2::LEFT_BOTTOM,
        caption,
        egui::FontId::proportional(10.0),
        colour,
    );
}
