//! Read-only CSV preview + one-click copy button for the digitiser's current
//! dataset (op-5sdc / op-8ixa — GitHub issue #30's "I think I want to have a
//! csv preview on the right hand side with a copy button similar to
//! htgr_sim_v1").
//!
//! Ports the "single read-only [`egui::TextEdit::multiline`] + Copy button"
//! widget from
//! `outram_park_digital_twin_engine::app_scaffold::csv_display::draw_csv_panel`
//! (`crates/outram-park-digital-twin-engine/src/app_scaffold/csv_display.rs`)
//! rather than depending on that crate — a knowledge-layer crate (`kovan`)
//! depending on a digital-twin simulator's GUI engine would be the wrong
//! dependency direction, and the widget itself is ~15 lines with no
//! simulator-specific content, so it is reproduced here with this citation
//! instead of vendored as a dependency. Unlike that crate's
//! `CsvSnapshotPanel`, there is no snapshot-on-click/interval-filtering here:
//! [`DigitisedDataset::to_csv_string`] is cheap and the dataset is small
//! (tens to low hundreds of points), so redrawing it every frame needs no
//! extra machinery.

use eframe::egui;

/// Whether a CSV preview carries its "Copy CSV" button.
///
/// Shown in the digitiser tabs, where the CSV is the thing being produced
/// and copying it out is the point. Hidden on the PDF reader's page-context
/// cards, where the preview is there to identify an artifact at a glance
/// and the button is clutter (maintainer, GH issue #35, 2026-09-08) — the
/// body is still drag-selectable there, so nothing is actually lost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyButton {
    /// Draw the button beside the heading.
    Shown,
    /// Heading only.
    Hidden,
}

/// Draw a "CSV preview" heading, optionally with a copy button, then
/// `csv_text` in a scrollable, monospace, read-only text box below.
///
/// `id_salt` must be unique among the previews drawn in one frame —
/// several digitised artifacts can be anchored to the same page, and egui
/// would otherwise give their scroll areas the same id and make them share
/// one scroll position.
///
/// The box is read-only in practice, not enforced — see the cited
/// `draw_csv_panel`'s doc comment for why a `TextEdit` over a throwaway
/// per-frame copy is used instead of a plain `Label` (native drag-select
/// works over the whole body as one contiguous selection that way).
pub fn draw_csv_preview(ui: &mut egui::Ui, csv_text: &str, copy: CopyButton, id_salt: &str) {
    ui.horizontal(|ui| {
        ui.heading("CSV preview");
        if copy == CopyButton::Shown && ui.button("\u{1F4CB} Copy CSV").clicked() {
            ui.ctx().copy_text(csv_text.to_string());
        }
    });
    egui::ScrollArea::both()
        .id_salt(id_salt)
        .show(ui, |ui| {
            let mut scratch = csv_text.to_string();
            ui.add(
                egui::TextEdit::multiline(&mut scratch)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY)
                    .desired_rows(24),
            );
        });
}
