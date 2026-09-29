// SPDX-License-Identifier: AGPL-3.0-only
//
// Provenance (partial translation — see below for exactly which parts):
//   Upstream project: markdown-editor, "Live (GitHub-Flavored) Markdown Editor"
//                     https://github.com/jbt/markdown-editor
//   Source file:      index.js — `render_tasklist` (a task-list item is drawn
//                     as a *disabled* checkbox with no bullet: the preview
//                     shows the state, it does not toggle it)
//   Commit:           58aa8bfe18c5a30af2c6fb08221b00b841688dd9 (2020-05-03)
//   Copyright:        (c) 2018 James Taylor
//   Licence:          ISC (full text in crates/kovan/NOTICE); ISC is
//                     AGPL-compatible.
//   Translated:       2026-09-28, JavaScript -> Rust/egui.
//
// Everything else in this file — painting the block model into egui — is
// kovan's own. Upstream painted through the browser and `github-markdown.css`
// (which ships in upstream's `lib/` with no licence header of its own, so none
// of it was copied); the heading scale below is GitHub's well-known
// 2 / 1.5 / 1.25 / 1 / 0.875 / 0.85 em ladder, not a transcription of that file.

//! Painting [`super::model`]'s blocks into an `egui::Ui`.

use eframe::egui::{self, Color32, RichText, Stroke};

use super::model::{AlertKind, Block, BlockKind, BlockPos, ListItem, Span};

/// Heading size as a multiple of the body text size, by level (h1..h6).
const HEADING_SCALE: [f32; 6] = [2.0, 1.5, 1.25, 1.0, 0.875, 0.85];

/// Paint `blocks` top to bottom, recording each top-level block's painted
/// extent (relative to `origin_y`, the top of the scrolled content) into
/// `positions` for the scroll sync.
///
/// Footnote definitions are gathered at the end under a rule, as GitHub
/// does, wherever they sit in the source.
pub fn show_document(
    ui: &mut egui::Ui,
    blocks: &[Block],
    origin_y: f32,
    positions: &mut Vec<BlockPos>,
) {
    positions.clear();
    let (notes, body): (Vec<&Block>, Vec<&Block>) = blocks
        .iter()
        .partition(|b| matches!(b.kind, BlockKind::FootnoteDefinition { .. }));
    for (n, block) in body.iter().chain(notes.iter()).enumerate() {
        if n == body.len() && !notes.is_empty() {
            ui.separator();
        }
        let top = ui.cursor().top();
        show_block(ui, block, n);
        let bottom = ui.min_rect().bottom().max(top);
        positions.push(BlockPos {
            lines: block.lines.clone(),
            top: top - origin_y,
            bottom: bottom - origin_y,
        });
    }
}

fn show_block(ui: &mut egui::Ui, block: &Block, salt: usize) {
    let body_size = egui::TextStyle::Body.resolve(ui.style()).size;
    match &block.kind {
        BlockKind::Heading { level, spans } => {
            let scale = HEADING_SCALE[(*level as usize).clamp(1, 6) - 1];
            ui.add_space(body_size * 0.6);
            show_spans(ui, spans, Some(body_size * scale), true);
            if *level <= 2 {
                ui.separator();
            }
            ui.add_space(body_size * 0.3);
        }
        BlockKind::Paragraph(spans) => {
            show_spans(ui, spans, None, false);
            ui.add_space(body_size * 0.6);
        }
        BlockKind::CodeBlock { lang, code } => {
            egui::Frame::group(ui.style())
                .fill(ui.visuals().extreme_bg_color)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if !lang.is_empty() {
                        ui.weak(RichText::new(lang).small());
                    }
                    ui.label(RichText::new(code.trim_end_matches('\n')).monospace());
                });
            ui.add_space(body_size * 0.6);
        }
        BlockKind::Quote { alert, blocks } => {
            let color = alert.map_or(ui.visuals().weak_text_color(), alert_color);
            let inner = ui.indent(("gfm-quote", salt), |ui| {
                if let Some(kind) = alert {
                    ui.label(RichText::new(kind.label()).strong().color(color));
                }
                for (i, b) in blocks.iter().enumerate() {
                    show_block(ui, b, salt * 31 + i);
                }
            });
            let r = inner.response.rect;
            let x = r.left() - 6.0;
            ui.painter().line_segment(
                [egui::pos2(x, r.top()), egui::pos2(x, r.bottom())],
                Stroke::new(3.0, color),
            );
        }
        BlockKind::List { start, items } => {
            for (i, item) in items.iter().enumerate() {
                show_item(ui, item, start.map(|s| s + i as u64), salt * 31 + i);
            }
            ui.add_space(body_size * 0.3);
        }
        BlockKind::Table { head, rows } => {
            egui::Grid::new(("gfm-table", salt))
                .striped(true)
                .spacing([16.0, 4.0])
                .show(ui, |ui| {
                    for cell in head {
                        show_spans(ui, cell, None, true);
                    }
                    ui.end_row();
                    for row in rows {
                        for cell in row {
                            show_spans(ui, cell, None, false);
                        }
                        ui.end_row();
                    }
                });
            ui.add_space(body_size * 0.6);
        }
        BlockKind::Rule => {
            ui.separator();
        }
        BlockKind::Html(html) => {
            ui.label(RichText::new(html.trim_end()).monospace().weak());
        }
        BlockKind::FootnoteDefinition { label, blocks } => {
            ui.horizontal_top(|ui| {
                ui.label(RichText::new(format!("[{label}]")).small().strong());
                ui.vertical(|ui| {
                    for (i, b) in blocks.iter().enumerate() {
                        show_block(ui, b, salt * 31 + i);
                    }
                });
            });
        }
    }
}

/// One list item: its marker, then its blocks. A task item (upstream's
/// `render_tasklist`) shows a disabled checkbox in place of the bullet.
fn show_item(ui: &mut egui::Ui, item: &ListItem, number: Option<u64>, salt: usize) {
    ui.horizontal_top(|ui| {
        match (item.task, number) {
            (Some(checked), _) => {
                let mut checked = checked;
                ui.add_enabled(false, egui::Checkbox::without_text(&mut checked));
            }
            (None, Some(n)) => {
                ui.label(format!("{n}."));
            }
            (None, None) => {
                ui.label("\u{2022}");
            }
        }
        ui.vertical(|ui| {
            for (i, b) in item.blocks.iter().enumerate() {
                show_block(ui, b, salt * 31 + i);
            }
        });
    });
}

fn alert_color(kind: AlertKind) -> Color32 {
    match kind {
        AlertKind::Note => Color32::from_rgb(80, 140, 230),
        AlertKind::Tip => Color32::from_rgb(70, 170, 90),
        AlertKind::Important => Color32::from_rgb(150, 100, 220),
        AlertKind::Warning => Color32::from_rgb(210, 150, 40),
        AlertKind::Caution => Color32::from_rgb(220, 70, 70),
    }
}

/// Inline spans, wrapped. Links open through egui's `hyperlink_to`, which
/// asks the platform to open the URL — nothing is fetched by kovan itself.
fn show_spans(ui: &mut egui::Ui, spans: &[Span], size: Option<f32>, strong: bool) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for span in spans {
            let mut text = RichText::new(&span.text);
            if let Some(size) = size {
                text = text.size(size);
            }
            if strong || span.style.strong {
                text = text.strong();
            }
            if span.style.emphasis {
                text = text.italics();
            }
            if span.style.strikethrough {
                text = text.strikethrough();
            }
            if span.style.code {
                text = text.code();
            }
            if span.style.footnote_ref {
                text = text.small_raised();
            }
            match &span.link {
                Some(url) => {
                    ui.hyperlink_to(text, url);
                }
                None => {
                    ui.label(text);
                }
            }
        }
    });
}
