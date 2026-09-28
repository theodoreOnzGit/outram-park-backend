// SPDX-License-Identifier: AGPL-3.0-only
//
// Provenance (partial translation — layout only):
//   Upstream project: markdown-editor, "Live (GitHub-Flavored) Markdown Editor"
//                     https://github.com/jbt/markdown-editor
//   Source files:     index.css (`#in`/`#out`: source on the left, rendered
//                     preview on the right, split 50/50) and index.js
//                     `toggleReadMode` (hide the source, preview only)
//   Commit:           58aa8bfe18c5a30af2c6fb08221b00b841688dd9 (2020-05-03)
//   Copyright:        (c) 2018 James Taylor
//   Licence:          ISC (full text in crates/kovan/NOTICE); ISC is
//                     AGPL-compatible.
//   Translated:       2026-09-28, CSS/JavaScript -> Rust/egui.
//
// The edit lock and its confirmation dialog are kovan's own; upstream has no
// equivalent (its editor is always writable).

//! The **Kvim Editor** tab: the kvim source editor on the left and a live
//! GitHub-flavoured Markdown preview on the right, like VS Code's Markdown
//! preview (maintainer, 2026-09-28).
//!
//! # Read-only by default
//!
//! A paper's Markdown is not free prose: kovan reads its artifacts out of
//! fenced `toml` blocks with a `[kovan]` table ([`crate::artifact`]), and a
//! stray edit can detach one. So the editor opens **locked** — the buffer is
//! drawn by [`KvimEditorState::ui_locked`], which forwards no key, clipboard
//! or mouse edit to the engine. **Edit** asks first, in a modal that names
//! what can break; only "I understand the risks — edit" unlocks it, and
//! **Done editing** locks it again. The state machine is [`EditLock`].
//!
//! **The confirmation is per document open**: [`KvimTab::document_opened`]
//! re-locks, and the app calls it whenever it loads a different document into
//! the editor (activating a paper, opening an external file). Saving the same
//! document, or kovan's own background resync of it, does not re-lock.
//!
//! Saving is unchanged: the tab's **Save** button writes the buffer exactly as
//! before, whether locked or not (a locked buffer can still hold edits made
//! before Done editing was pressed).

use eframe::egui;

use super::gfm_preview::GfmPreview;
use super::kvim_editor::{CompletionSource, EditorSignal, KvimEditorState};

/// Whether the kvim tab's buffer may be edited.
///
/// `Locked -> (request_edit) -> ConfirmPending -> (confirm) -> Unlocked`;
/// `ConfirmPending -> (cancel) -> Locked`; anything `-> (lock) -> Locked`.
/// Every other transition is a no-op, so a stray `confirm` can never unlock
/// a buffer nobody asked to edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditLock {
    #[default]
    Locked,
    /// Edit was clicked; the risk warning is showing.
    ConfirmPending,
    Unlocked,
}

impl EditLock {
    /// The Edit button: ask for confirmation.
    pub fn request_edit(&mut self) {
        if *self == Self::Locked {
            *self = Self::ConfirmPending;
        }
    }

    /// "I understand the risks — edit".
    pub fn confirm(&mut self) {
        if *self == Self::ConfirmPending {
            *self = Self::Unlocked;
        }
    }

    /// Cancel (or Esc / clicking outside the dialog).
    pub fn cancel(&mut self) {
        if *self == Self::ConfirmPending {
            *self = Self::Locked;
        }
    }

    /// Done editing, or a new document opened.
    pub fn lock(&mut self) {
        *self = Self::Locked;
    }

    pub fn is_editable(self) -> bool {
        self == Self::Unlocked
    }

    pub fn confirmation_open(self) -> bool {
        self == Self::ConfirmPending
    }
}

/// Which panes the tab shows. Split is upstream's (and VS Code's) default;
/// Preview is upstream's "reading mode"; Source is VS Code's editor alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaneLayout {
    #[default]
    Split,
    SourceOnly,
    PreviewOnly,
}

/// The warning shown before the buffer unlocks. Worded from
/// [`crate::artifact`]'s schema (GH issue #35 §13–§20), so each bullet names
/// a real way a hand edit breaks something.
pub const RISK_WARNING: &[&str] = &[
    "Each artifact is a `#` heading immediately followed by a fenced ```toml block \
     containing a [kovan] table (id, kind, created, modified). Moving, splitting or \
     deleting that fence, or the heading above it, detaches the artifact.",
    "An artifact's `id` must stay unique within the paper: citations, [[paper#id]] \
     wiki links and mindmap relations point at the id, not the heading text. \
     Renaming or duplicating an id breaks them.",
    "`connections` and [[relation]] entries are two-way references with the \
     mindmap document; editing one end by hand leaves the other pointing at nothing.",
    "Digitised datasets keep their data in fenced ```csv blocks. A broken fence or \
     malformed CSV loses the dataset's export. Only add a `reviewed` timestamp if a \
     human really reviewed it.",
    "The first artifact is the paper header (citekey heading, BibTeX in a ```latex \
     fence). Removing it decapitates the paper.",
];

/// The kvim tab's own state (the buffer itself is the app's shared
/// [`KvimEditorState`], also shown by the PDF reader's page-context panel).
#[derive(Default)]
pub struct KvimTab {
    lock: EditLock,
    layout: PaneLayout,
    preview: GfmPreview,
}

impl KvimTab {
    /// A different document was loaded into the editor: re-lock (the
    /// confirmation is per document open) and drop the old preview.
    pub fn document_opened(&mut self) {
        self.lock.lock();
        self.preview.reset();
    }

    #[cfg(test)]
    pub fn lock_state(&self) -> EditLock {
        self.lock
    }

    /// The tab's controls: Edit / Done editing, and the pane layout. Drawn
    /// into the caller's toolbar row.
    pub fn toolbar_ui(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        match self.lock {
            EditLock::Locked | EditLock::ConfirmPending => {
                if ui
                    .button("\u{270F} Edit")
                    .on_hover_text("read-only; editing by hand can break kovan's artifact schema")
                    .clicked()
                {
                    self.lock.request_edit();
                }
            }
            EditLock::Unlocked => {
                if ui
                    .button("\u{1F512} Done editing")
                    .on_hover_text("make the buffer read-only again (unsaved edits are kept)")
                    .clicked()
                {
                    self.lock.lock();
                }
            }
        }
        ui.separator();
        ui.selectable_value(&mut self.layout, PaneLayout::Split, "Source | Preview");
        ui.selectable_value(&mut self.layout, PaneLayout::SourceOnly, "Source");
        ui.selectable_value(&mut self.layout, PaneLayout::PreviewOnly, "Preview");
    }

    /// The panes, filling `ui`, plus the confirmation modal when it is up.
    /// Returns the editor's ex-command signal (`:w`/`:q`), if any, exactly as
    /// [`KvimEditorState::ui`] does.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        editor: &mut KvimEditorState,
        completion: Option<CompletionSource<'_>>,
    ) -> Option<EditorSignal> {
        let mut signal = None;
        let editable = self.lock.is_editable();
        let mut source = |ui: &mut egui::Ui| {
            if editable {
                signal = editor.ui(ui, completion);
            } else {
                editor.ui_locked(ui);
            }
        };
        match self.layout {
            PaneLayout::SourceOnly => source(ui),
            PaneLayout::PreviewOnly => {
                self.preview.set_text(&editor.text());
                self.preview.ui(ui);
            }
            PaneLayout::Split => {
                let half = (ui.available_width() * 0.5).max(200.0);
                egui::Panel::left("kvim-tab-source")
                    .resizable(true)
                    .default_size(half)
                    .min_size(200.0)
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| source(ui));
                self.preview.set_text(&editor.text());
                self.preview.follow_editor_line(editor.top_visible_line());
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE.inner_margin(egui::Margin {
                        left: 12,
                        ..Default::default()
                    }))
                    .show(ui, |ui| self.preview.ui(ui));
            }
        }

        if self.lock.confirmation_open() && self.confirmation_ui(ui.ctx()) {
            editor.begin_insert();
        }
        signal
    }

    /// The risk warning. Returns whether the operator just confirmed.
    fn confirmation_ui(&mut self, ctx: &egui::Context) -> bool {
        let mut confirmed = false;
        let mut cancelled = false;
        let modal = egui::Modal::new(egui::Id::new("kvim-edit-risk-warning")).show(ctx, |ui| {
            ui.set_max_width(520.0);
            ui.heading("Edit this document by hand?");
            ui.add_space(6.0);
            ui.label(
                "This is a kovan document, not just prose: kovan reads structured artifacts \
                 out of it, and a hand edit can break that structure.",
            );
            ui.add_space(4.0);
            for risk in RISK_WARNING {
                ui.horizontal_top(|ui| {
                    ui.label("\u{2022}");
                    ui.label(*risk);
                });
            }
            ui.add_space(4.0);
            ui.label(
                "A malformed [kovan] block is reported as a problem and that artifact is dropped \
                 from the Wiki, Mindmap and reader until it is fixed. Nothing is written to disk \
                 until you press Save.",
            );
            ui.add_space(6.0);
            ui.strong("Know the risks before you continue.");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("I understand the risks \u{2014} edit").clicked() {
                    confirmed = true;
                }
                if ui.button("Cancel").clicked() {
                    cancelled = true;
                }
            });
        });
        if confirmed {
            self.lock.confirm();
        } else if cancelled || modal.should_close() {
            self.lock.cancel();
        }
        confirmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run the tab headlessly (no window, no GPU) for `frames`, each a list
    /// of input events.
    fn drive(
        tab: &mut KvimTab,
        editor: &mut KvimEditorState,
        frames: Vec<Vec<egui::Event>>,
    ) -> egui::Context {
        let ctx = egui::Context::default();
        for (n, events) in frames.into_iter().enumerate() {
            let input = egui::RawInput {
                time: Some(n as f64 * 0.1),
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200.0, 800.0),
                )),
                events,
                ..Default::default()
            };
            let _ = ctx.run_ui(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    tab.ui(ui, editor, None);
                });
            });
        }
        ctx
    }

    /// Type `x` into an editor that has been asked to take focus in Insert
    /// mode — what reaches the buffer if (and only if) the tab lets it.
    fn try_typing(tab: &mut KvimTab, editor: &mut KvimEditorState) {
        editor.begin_insert();
        drive(
            tab,
            editor,
            vec![Vec::new(), Vec::new(), vec![egui::Event::Text("x".into())]],
        );
    }

    #[test]
    fn the_tab_starts_locked() {
        assert_eq!(KvimTab::default().lock_state(), EditLock::Locked);
    }

    #[test]
    fn typing_while_locked_changes_nothing() {
        let mut tab = KvimTab::default();
        let mut editor = KvimEditorState::default();
        editor.load_text("[kovan]\n");
        try_typing(&mut tab, &mut editor);
        assert_eq!(editor.text(), "[kovan]\n");
        assert!(!editor.is_modified());
    }

    #[test]
    fn edit_shows_the_modal_and_cancel_leaves_it_locked() {
        let mut tab = KvimTab::default();
        let mut editor = KvimEditorState::default();
        editor.load_text("body\n");
        tab.lock.request_edit();
        assert_eq!(tab.lock_state(), EditLock::ConfirmPending);
        let ctx = drive(&mut tab, &mut editor, vec![Vec::new(), Vec::new()]);
        assert!(
            ctx.memory(|m| m.top_modal_layer()).is_some(),
            "the risk warning is not showing"
        );
        // Still not editable while the warning is up.
        try_typing(&mut tab, &mut editor);
        assert_eq!(editor.text(), "body\n");

        tab.lock.cancel();
        assert_eq!(tab.lock_state(), EditLock::Locked);
        try_typing(&mut tab, &mut editor);
        assert_eq!(editor.text(), "body\n");
    }

    #[test]
    fn escape_on_the_modal_cancels() {
        let mut tab = KvimTab::default();
        let mut editor = KvimEditorState::default();
        editor.load_text("body\n");
        tab.lock.request_edit();
        let esc = egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        drive(&mut tab, &mut editor, vec![Vec::new(), vec![esc]]);
        assert_eq!(tab.lock_state(), EditLock::Locked);
    }

    #[test]
    fn confirm_unlocks_and_done_editing_locks_again() {
        let mut tab = KvimTab::default();
        let mut editor = KvimEditorState::default();
        editor.load_text("body\n");

        // A confirm nobody asked for does nothing.
        tab.lock.confirm();
        assert_eq!(tab.lock_state(), EditLock::Locked);

        tab.lock.request_edit();
        tab.lock.confirm();
        assert!(tab.lock_state().is_editable());
        try_typing(&mut tab, &mut editor);
        assert_eq!(editor.text(), "xbody\n", "an unlocked buffer takes typing");

        tab.lock.lock();
        try_typing(&mut tab, &mut editor);
        assert_eq!(editor.text(), "xbody\n", "a re-locked buffer does not");
        assert!(editor.is_modified(), "locking keeps unsaved edits");
    }

    #[test]
    fn opening_a_new_document_relocks() {
        let mut tab = KvimTab::default();
        tab.lock.request_edit();
        tab.lock.confirm();
        tab.document_opened();
        assert_eq!(tab.lock_state(), EditLock::Locked);
    }

    #[test]
    fn the_preview_renders_a_gfm_document_headlessly() {
        let mut tab = KvimTab::default();
        let mut editor = KvimEditorState::default();
        editor.load_text(
            "# Paper\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n~~old~~ new\n\n- [x] done\n- [ ] todo\n\n\
             see https://example.org\n",
        );
        drive(&mut tab, &mut editor, vec![Vec::new(), Vec::new()]);
        let kinds: Vec<_> = tab
            .preview
            .blocks()
            .iter()
            .map(|b| std::mem::discriminant(&b.kind))
            .collect();
        assert_eq!(kinds.len(), 5, "heading, table, paragraph, list, paragraph");
        assert_eq!(
            tab.lock_state(),
            EditLock::Locked,
            "rendering does not unlock"
        );
    }
}
