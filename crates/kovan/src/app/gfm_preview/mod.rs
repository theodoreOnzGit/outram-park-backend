//! Live GitHub-flavoured Markdown preview for the kvim editor tab
//! (maintainer, 2026-09-28: "make kvim editor tab a kvim editor + markdown
//! renderer ... markdown is gh flavoured"; layout "similar to vscode").
//!
//! Partly a translation of [jbt/markdown-editor](https://github.com/jbt/markdown-editor)
//! (ISC, (c) 2018 James Taylor, commit `58aa8bf`), at the maintainer's
//! direction — see each file's provenance header for exactly which upstream
//! functions were translated, and `crates/kovan/NOTICE`. Upstream's parsing
//! (`markdown-it` in the browser) is **not** ported; `pulldown-cmark` with
//! its GFM options replaces it ([`model::options`]).
//!
//! - [`model`] — egui-free: Markdown -> blocks with source line ranges, the
//!   upstream "scroll to the first changed element" rule, and the VS Code
//!   editor-line -> preview-offset mapping. Unit-tested.
//! - [`view`] — paints the blocks into egui.
//!
//! # Why not `egui_commonmark`
//!
//! It was the first candidate checked (0.25.0 supports egui 0.36, MIT OR
//! Apache-2.0, tables/strikethrough/task lists/footnotes). Two things ruled it
//! out: its `show` reports nothing about where each source line was painted,
//! so the editor-to-preview scroll sync the VS Code layout calls for could
//! only have been a proportional guess; and it pulls `pulldown-cmark` 0.13
//! beside the workspace's 0.12, a second parser in the tree. The maintainer
//! then directed a translation of jbt/markdown-editor, which this is.

pub mod model;
mod view;

use eframe::egui;

use model::{Block, BlockPos};

/// The preview's state: the parsed document (re-parsed only when the text
/// changes), where each block was painted last frame, and any scroll the
/// next frame should apply.
#[derive(Default)]
pub struct GfmPreview {
    source: String,
    blocks: Vec<Block>,
    positions: Vec<BlockPos>,
    /// The editor's top visible line as last followed, so only a *change*
    /// scrolls the preview and the preview can otherwise be scrolled freely.
    last_editor_line: Option<usize>,
    /// A model index whose painted position should be brought into view once
    /// the new text has been laid out (upstream `setOutput`'s rule).
    pending_block: Option<usize>,
    /// An absolute scroll offset to apply on the next paint.
    pending_scroll: Option<f32>,
    /// (offset, height) of the preview viewport as last painted.
    viewport: (f32, f32),
    loaded: bool,
}

impl GfmPreview {
    /// Take the current buffer text; re-parse if it changed. On a change
    /// after the first load, remember the first changed block so the preview
    /// can bring it into view (translated from upstream `setOutput`).
    pub fn set_text(&mut self, text: &str) {
        if self.loaded && self.source == text {
            return;
        }
        let blocks = model::parse(text);
        if self.loaded {
            self.pending_block = model::first_changed_block(&self.blocks, &blocks);
        }
        self.blocks = blocks;
        self.source = text.to_string();
        self.loaded = true;
    }

    /// Forget everything — a different document was opened.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// The parsed document.
    #[cfg(test)]
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }

    /// Follow the editor: when its top visible line has moved since the last
    /// call, scroll the preview so that line's block is at the top.
    pub fn follow_editor_line(&mut self, line: usize) {
        if self.last_editor_line == Some(line) {
            return;
        }
        self.last_editor_line = Some(line);
        if let Some(y) = model::sync_offset(&self.positions, line) {
            self.pending_scroll = Some(y);
            // An explicit editor scroll wins over the changed-block rule.
            self.pending_block = None;
        }
    }

    /// Paint the preview, filling `ui`.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        if let Some(title) = model::document_title(&self.blocks) {
            ui.horizontal(|ui| {
                ui.weak("preview —");
                ui.strong(title);
            });
            ui.separator();
        }
        let mut area = egui::ScrollArea::vertical()
            .id_salt("kvim-gfm-preview")
            .auto_shrink([false, false]);
        if let Some(y) = self.pending_scroll.take() {
            area = area.vertical_scroll_offset(y.max(0.0));
        }
        let blocks = &self.blocks;
        let positions = &mut self.positions;
        let out = area.show(ui, |ui| {
            let origin = ui.min_rect().top();
            view::show_document(ui, blocks, origin, positions);
        });
        self.viewport = (out.state.offset.y, out.inner_rect.height());

        // Upstream `setOutput`: after an edit, show the first changed block.
        // Adapted: only when it is not already on screen, so typing in view
        // does not jerk the preview about.
        if let Some(index) = self.pending_block.take() {
            let target = self
                .blocks
                .get(index)
                .and_then(|b| self.positions.iter().find(|p| p.lines == b.lines))
                .or(self.positions.last());
            if let Some(p) = target {
                let (offset, height) = self.viewport;
                if p.top < offset || p.top > offset + height {
                    self.pending_scroll = Some(p.top);
                    ui.ctx().request_repaint();
                }
            }
        }
    }
}
