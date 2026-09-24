//! `kopitiam-neovim` editor engine + egui adapter (§26, §27,
//! `op-9vo6.17`). Replaces `markdown_editor.rs`'s hand-rolled text editing.
//!
//! # What this reuses, and what it adds
//!
//! `kopitiam-neovim` provides the whole modal-editing engine —
//! `editor::Editor` (buffer, cursor, Normal/Insert/Visual/… modes,
//! motions, operators, undo/redo, registers, search) over its own `Rope`
//! buffer ([`kopitiam_neovim::text::Buffer`]) — deliberately terminal- and
//! UI-independent (its own `key.rs` module doc: "this whole crate stays
//! testable without a terminal at all"). **This module does not
//! reimplement any of that.** What it adds is exactly the missing half:
//! mapping egui's input events onto `kopitiam_neovim::editor::key::Key`,
//! and painting the buffer/cursor/selection/mode into an `egui::Ui` —
//! `kopitiam-neovim`'s own `ui` module is a `ratatui` terminal frontend,
//! which §26 explicitly says not to embed here.
//!
//! `kopitiam-neovim` itself stays ignorant of PDFs, page numbers, Kovan
//! artifacts, classifications, bibliography semantics and synchronisation
//! — exactly §26's boundary. This module knows none of those things
//! either; it only knows how to edit text. A caller feeds it a paper's
//! Markdown text and reads it back out (see [`KvimEditorState::load_text`]/
//! [`KvimEditorState::text`]) — wiring that to a live [`crate::session::PaperSession`]
//! is `op-9vo6.18`'s `SyncController` job, not this module's.
//!
//! # Mouse-friendly modal editing (§27)
//!
//! `kopitiam-neovim`'s own semantics are unchanged by any of this — §27 is
//! explicit that mouse-friendliness is *this adapter's* behaviour, not a
//! change to standalone kvim:
//!
//! - **Click** moves the cursor there. If the editor was not already in
//!   Insert mode, a synthetic `Esc` (harmless from Normal, and returns
//!   cleanly from Visual/Command/OperatorPending) followed by `i` enters
//!   it — "clicking editable text moves the cursor there and enters Insert
//!   mode."
//! - **Drag** enters Visual (charwise) mode at the press position and
//!   moves the cursor to extend the selection as the pointer moves — this
//!   composes with the engine's own selection tracking rather than
//!   reimplementing it: entering Visual mode anchors at the current
//!   cursor, and every subsequent [`Editor::move_cursor`] extends it.
//! - **Wheel** scrolling is the surrounding [`egui::ScrollArea`]'s, free.
//! - **Esc** always returns to Normal, same as pressing it on a keyboard.
//! - The current mode is always shown (`Mode::label`), so a non-Vim user
//!   is never left wondering why typing did something unexpected.
//! - **Ctrl+C / Ctrl+X / Ctrl+V** use the system clipboard, like any other
//!   text box — see [`KvimEditorState::paste`] and GH issue #280. kvim's
//!   own `y`/`p` registers are untouched and still work as in Vim.
//!
//! # Why the clipboard needs code here at all (GH issue #280)
//!
//! It is not a missing key mapping. `egui-winit` recognises the clipboard
//! chords itself and pushes [`egui::Event::Cut`]/[`egui::Event::Copy`]/
//! [`egui::Event::Paste`] — **returning before it emits any
//! [`egui::Event::Key`]**, so those three chords never reach [`map_event`]
//! as keys at all. Until this was handled, the focused editor dropped all
//! three, which is why *"i cannot ctrl-shift-v to paste inside the summary
//! text box"* (maintainer, 2026-09-23).
//!
//! `Ctrl+Shift+V` works for the same reason `Ctrl+V` does: egui-winit's
//! `is_paste_command` is `command && V` and never looks at shift.

use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use kopitiam_neovim::core::{Mode, Position, Range};
use kopitiam_neovim::editor::key::{Key as KvimKey, KeyCode as KvimKeyCode, Modifiers as KvimModifiers};
use kopitiam_neovim::editor::Editor;

use crate::autocomplete::{self, Candidate};
use crate::index::KnowledgeIndex;
use crate::research_record::ResearchRecordIndex;
use crate::root::KovanRoot;
use crate::session::PaperSession;

const CHAR_SIZE: f32 = 14.0;
const LINE_SPACING: f32 = 1.35;

/// What [`KvimEditorState::ui`] needs to answer citation/wiki completion
/// queries (§29/§30, `op-9vo6.16`) — the library, not this widget, is what
/// enumerates candidates; this struct just carries the two things that
/// enumeration needs.
#[derive(Clone, Copy)]
pub struct CompletionSource<'a> {
    pub root: &'a KovanRoot,
    pub index: &'a KnowledgeIndex,
}

/// The trigger a completion popup is currently answering, and where it
/// started in the buffer (so accepting a candidate knows exactly what
/// range of typed text to replace).
#[derive(Debug, Clone, PartialEq)]
enum Trigger {
    /// A bare `@` — §29. `start` is the position of the `@` itself.
    Citation { start: Position },
    /// `[[` — §30. `start` is the position right after the second `[`.
    /// `paper` is `Some` once the query contains `#`, i.e. the user has
    /// picked a paper and is now completing one of its artifacts.
    Wiki {
        start: Position,
        paper: Option<String>,
    },
}

/// Find an active trigger immediately before `cursor` on its own line, and
/// the query text typed since it. `None` means no popup should show this
/// frame — the common case.
fn detect_trigger(line_text: &str, cursor: Position) -> Option<(Trigger, String)> {
    let before_cursor = line_text.get(..cursor.col)?;
    if let Some(at) = before_cursor.rfind('@') {
        let query = &before_cursor[at + 1..];
        if !query
            .chars()
            .any(|c| c.is_whitespace() || c == '@' || c == '[' || c == ']')
        {
            return Some((
                Trigger::Citation {
                    start: Position::new(cursor.line, at),
                },
                query.to_string(),
            ));
        }
    }
    if let Some(open) = before_cursor.rfind("[[") {
        let query = &before_cursor[open + 2..];
        if !query
            .chars()
            .any(|c| c.is_whitespace() || c == '[' || c == ']')
        {
            let (paper, rest, start_col) = match query.split_once('#') {
                Some((p, a)) => (Some(p.to_string()), a, open + 2 + p.len() + 1),
                None => (None, query, open + 2),
            };
            return Some((
                Trigger::Wiki {
                    start: Position::new(cursor.line, start_col),
                    paper,
                },
                rest.to_string(),
            ));
        }
    }
    None
}

/// State for one `kopitiam-neovim`-backed editor surface.
pub struct KvimEditorState {
    editor: Editor,
    /// Snapshot of the text as of the last [`Self::load_text`] — used to
    /// answer [`Self::is_modified`] without depending on `kopitiam-neovim`'s
    /// own undo-based modified flag, which would report "modified" right
    /// after a fresh load (loading text is itself an undo-tracked edit
    /// from the engine's point of view).
    loaded_text: String,
    /// Whether a Visual-mode drag is in progress (§27).
    dragging: bool,
    /// Top-left of the text area as last painted — the anchor the
    /// completion popup positions itself from.
    text_area_origin: Pos2,
    /// Set by [`Self::jump_to_line`]; consumed (and cleared) by the next
    /// [`Self::text_area`] paint, which scrolls that line into view. A
    /// one-shot flag rather than a persistent "follow the cursor" mode, so
    /// the operator can freely scroll away afterwards without the view
    /// snapping back (op-j178: the PDF reader's page-context panel jumping
    /// to an artifact's heading).
    pending_scroll_to_line: Option<usize>,
    /// Where the caret was at the end of the previous frame, so
    /// [`Self::text_area`] can tell a caret that **moved** from one merely
    /// sitting still. Only a move scrolls -- following every frame would pin
    /// the view to the caret and make wheel-scrolling a focused editor
    /// impossible, the view snapping back the instant you let go.
    last_cursor: Option<Position>,
    /// 0-based, end-exclusive line ranges painted with a faint band — every
    /// block anchored to the PDF reader's current page (op-j178: the
    /// page-context panel's read-only preview marks what is double-clickable).
    /// Persistent; the caller re-sets it each frame.
    anchor_bands: Vec<std::ops::Range<usize>>,
    /// A single stronger band for the block currently hovered on the page
    /// canvas (op-4x5s). Persistent; set/cleared by the caller per frame.
    hover_band: Option<std::ops::Range<usize>>,
    /// The `egui::Id` of the editable text area as last allocated. Used to
    /// re-request keyboard focus after a mouse-click on the completion popup
    /// (GH issue #35 2026-09-02: accepting an autocomplete used to drop
    /// focus, so the cursor greyed out and further typing did nothing —
    /// which reads exactly like being kicked back to Normal mode).
    text_area_id: Option<egui::Id>,
    /// The line a press landed on in the **read-only** preview, kept until
    /// the button comes back up so an unhurried press-release can still be
    /// recognised as a click on that line (GH issue #282 — same egui
    /// threshold that broke click-to-insert in the editable path).
    preview_press_line: Option<usize>,
    /// Set by [`Self::begin_insert`]; consumed by the next [`Self::text_area`],
    /// which hands keyboard focus to the text area once it has an id to give
    /// it. A one-shot flag, not a mode: after that frame the ordinary focus
    /// rules apply (GH issue #282).
    pending_focus: bool,
}

impl Default for KvimEditorState {
    fn default() -> Self {
        Self {
            editor: Editor::new(),
            loaded_text: String::new(),
            dragging: false,
            text_area_origin: Pos2::ZERO,
            pending_scroll_to_line: None,
            last_cursor: None,
            anchor_bands: Vec::new(),
            hover_band: None,
            preview_press_line: None,
            pending_focus: false,
            text_area_id: None,
        }
    }
}

impl KvimEditorState {
    /// Load `text` as the buffer's whole content, replacing whatever was
    /// there. Goes through [`Editor::replace_range`] (the engine's own
    /// sanctioned "swap in new text" path — used for completion-accept and
    /// snippet expansion) rather than reconstructing the buffer directly,
    /// since `Editor` exposes no way to install a different `Buffer` value.
    pub fn load_text(&mut self, text: &str) {
        self.editor = Editor::new();
        let end = self
            .editor
            .buffer()
            .clamp(Position::new(usize::MAX, usize::MAX));
        self.editor
            .replace_range(Range::new(Position::ORIGIN, end), text);
        self.editor.move_cursor(Position::ORIGIN);
        self.loaded_text = text.to_string();
        self.dragging = false;
    }

    /// The buffer's current text.
    pub fn text(&self) -> String {
        self.editor.buffer().text()
    }

    /// Move the cursor to (1-based) `line`'s start and scroll it into view
    /// on the next paint (op-j178: the PDF reader's page-context panel
    /// jumping to an artifact's heading — `Artifact::line`'s own doc:
    /// "1-based line of the heading, for 'jump to it in the editor'").
    /// Out-of-range clamps to the nearest valid line rather than panicking.
    pub fn jump_to_line(&mut self, line: usize) {
        let target = self
            .editor
            .buffer()
            .clamp(Position::new(line.saturating_sub(1), 0));
        self.editor.move_cursor(target);
        self.pending_scroll_to_line = Some(target.line);
    }

    /// Whether the text has changed since the last [`Self::load_text`].
    pub fn is_modified(&self) -> bool {
        self.text() != self.loaded_text
    }

    /// Reload `text` as the buffer content **only if the buffer has no
    /// unsaved edits** (`!is_modified()`) and it actually differs from what
    /// is loaded. Returns whether a reload happened.
    ///
    /// This is how an embedded editor stays live while other flows (a PDF
    /// annotation save, a digitiser CSV save — see
    /// [`crate::app`]) append to the very same document: the caller hands
    /// this the session's current Markdown every frame, and it swaps it in
    /// whenever the operator is not mid-edit, never clobbering typing in
    /// progress.
    pub fn sync_to_disk_text(&mut self, text: &str) -> bool {
        if self.loaded_text == text || self.is_modified() {
            return false;
        }
        self.load_text(text);
        true
    }

    /// The faint bands behind every block anchored to the reader's current
    /// page — re-set each frame by the page-context panel (op-j178). Painted
    /// only in the read-only preview ([`Self::ui_readonly`]).
    pub fn set_anchor_bands(&mut self, bands: Vec<std::ops::Range<usize>>) {
        self.anchor_bands = bands;
    }

    /// The single stronger band for the block currently hovered on the page
    /// canvas (op-4x5s). Re-set each frame; `None` clears it.
    pub fn set_hover_band(&mut self, band: Option<std::ops::Range<usize>>) {
        self.hover_band = band;
    }

    /// Draw the editor and process this frame's input for it. `ui`'s
    /// available space is fully claimed by a scrollable text area plus a
    /// one-line mode/status bar. `completion`, when given, enables §29/§30's
    /// citation/wiki autocomplete popup — omit it for a scratch buffer with
    /// no library context (e.g. before a Kovan root is open).
    pub fn ui(&mut self, ui: &mut egui::Ui, completion: Option<CompletionSource<'_>>) {
        ui.horizontal(|ui| {
            ui.strong(self.mode_label());
            let pos = self.editor.cursor();
            ui.weak(format!("{}:{}", pos.line + 1, pos.col + 1));
            if self.is_modified() {
                ui.weak("[+]");
            }
        });
        ui.separator();

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.text_area(ui, false);
            });

        if let Some(source) = completion {
            self.completion_popup_ui(ui, source);
        }
    }

    /// Draw the buffer **read-only** — the page-context panel's markdown
    /// preview (op-j178/GH issue #35 2026-09-02: "don't allow me to edit it
    /// directly"). No key input; a **single click** returns that line
    /// (0-based) so the caller can open the right per-block editor (GH issue
    /// #35 2026-09-02: "a single click to bring me into insert mode, not
    /// double click"). Paints the anchor/hover bands. `jump_to_line` still
    /// works.
    pub fn ui_readonly(&mut self, ui: &mut egui::Ui) -> Option<usize> {
        ui.horizontal(|ui| {
            ui.weak("preview — click a highlighted block to edit");
        });
        ui.separator();
        let mut clicked_line = None;
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                clicked_line = self.text_area(ui, true);
            });
        clicked_line
    }

    /// §29/§30: while in Insert mode, detect a `@`/`[[` trigger on the
    /// current line and show a mouse-selectable completion popup for it.
    /// Keyboard navigation of the popup list itself (arrow keys to move the
    /// selection) is not implemented in this pass — Up/Down are already
    /// meaningful cursor motions in Insert mode, and resolving that
    /// conflict is real interaction-design work left for later dogfooding;
    /// typing the trigger and clicking a candidate is fully functional
    /// today and is what "the user must not have to memorise citation
    /// keys" actually requires.
    fn completion_popup_ui(&mut self, ui: &mut egui::Ui, source: CompletionSource<'_>) {
        if self.editor.mode() != Mode::Insert {
            return;
        }
        let cursor = self.editor.cursor();
        let Some(line_text) = self.editor.buffer().line(cursor.line) else {
            return;
        };
        let Some((trigger, query)) = detect_trigger(&line_text, cursor) else {
            return;
        };

        let candidates: Vec<Candidate> = match &trigger {
            Trigger::Citation { .. } => autocomplete::citation_candidates(source.root, &query),
            Trigger::Wiki { paper: None, .. } => {
                autocomplete::wiki_candidates(source.index, &query)
            }
            Trigger::Wiki {
                paper: Some(paper), ..
            } => {
                let Ok(session) = PaperSession::open(source.root, paper) else {
                    return;
                };
                let research = ResearchRecordIndex::from_session(&session);
                autocomplete::artifact_candidates(&research, &query)
            }
        };
        if candidates.is_empty() {
            return;
        }

        let font = FontId::monospace(CHAR_SIZE);
        let char_width = ui.ctx().fonts_mut(|f| f.glyph_width(&font, ' ')).max(1.0);
        let line_height = ui.ctx().fonts_mut(|f| f.row_height(&font)) * LINE_SPACING;
        let anchor = self.text_area_origin
            + Vec2::new(
                cursor.col as f32 * char_width,
                (cursor.line + 1) as f32 * line_height,
            );

        let mut chosen = None;
        egui::Area::new(ui.id().with("kvim-completion-popup"))
            .fixed_pos(anchor)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    for candidate in candidates.iter().take(20) {
                        let label = if candidate.detail.is_empty() {
                            candidate.label.clone()
                        } else {
                            format!("{}  —  {}", candidate.label, candidate.detail)
                        };
                        if ui.selectable_label(false, label).clicked() {
                            chosen = Some(candidate.clone());
                        }
                    }
                });
            });

        if let Some(candidate) = chosen {
            // `start` already sits after any `[[paper#` the buffer still
            // holds (see `detect_trigger`), so the replacement only ever
            // supplies what comes *after* that — never the paper name
            // again, which would otherwise duplicate it.
            let replacement = match &trigger {
                Trigger::Citation { .. } => format!("[@{}]", candidate.insert_text),
                Trigger::Wiki { .. } => format!("{}]]", candidate.insert_text),
            };
            let start = match trigger {
                Trigger::Citation { start } => start,
                Trigger::Wiki { start, .. } => start,
            };
            self.editor
                .replace_range(Range::new(start, cursor), &replacement);

            // GH issue #35 2026-09-02: "autocompletes should leave me in
            // insert mode, not normal mode." The mouse-click on the popup
            // pulls keyboard focus off the text area, so the next frame's
            // keystrokes stop reaching the engine (the `has_focus()` gate in
            // `text_area`) and the cursor greys out — indistinguishable from
            // being dropped back to Normal. Re-assert Insert and hand focus
            // straight back to the editor.
            if self.editor.mode() != Mode::Insert {
                let _ = self.editor.handle_key(KvimKey::esc());
                let _ = self.editor.handle_key(KvimKey::char('i'));
            }
            if let Some(id) = self.text_area_id {
                ui.ctx().memory_mut(|m| m.request_focus(id));
            }
        }
    }

    /// The current mode's label — "NORMAL", "INSERT", "VISUAL", … — as shown
    /// in the editor's own status line. The one reading of the engine's mode
    /// a caller outside this module gets, so nothing else has to reach into
    /// [`Editor`].
    pub fn mode_label(&self) -> &'static str {
        self.editor.mode().label()
    }

    /// Open this editor ready to type: Insert mode, with keyboard focus
    /// claimed on the next frame (GH issue #282).
    ///
    /// For the callers that open an editor *because the user already
    /// clicked* — the PDF reader's page-context panel opening a block from
    /// its card or its banded preview line. [`Self::load_text`] builds a
    /// fresh [`Editor`], which starts in Normal mode with no focus, so
    /// without this the click that asked for an editor lands the user in
    /// front of one where typing runs Vim commands. GH issue #35's own ask:
    /// "a single click to bring me into insert mode, not double click".
    ///
    /// Call it *after* `load_text`, which resets the editor.
    pub fn begin_insert(&mut self) {
        if self.editor.mode() != Mode::Insert {
            let _ = self.editor.handle_key(KvimKey::esc());
            let _ = self.editor.handle_key(KvimKey::char('i'));
        }
        self.pending_focus = true;
    }

    /// Finish a mouse drag: a drag that selected **nothing** was really a
    /// click, so treat it as one and enter Insert mode (GH issue #282).
    ///
    /// egui promotes a press to a drag once it passes *either* of
    /// [`egui::Options`]' thresholds — `max_click_dist` (6 px: a trackpad
    /// wobble) or `max_click_duration` (0.8 s: resting on the button) — and
    /// from then on `Response::clicked()` never fires. `text_area`'s
    /// click-to-insert arm therefore missed every unhurried click, leaving
    /// the editor in the Visual mode the drag had entered, where typing runs
    /// Vim commands instead of inserting text. The maintainer's report,
    /// 2026-09-23: "when i click the page context editor in kvim, i expect
    /// to go into insert mode. It doesn't do that."
    ///
    /// A drag that *did* select something is left alone — that is a real
    /// selection the user made on purpose.
    fn end_drag(&mut self) {
        self.dragging = false;
        if self
            .editor
            .selection()
            .is_some_and(|(start, end)| start == end)
        {
            let _ = self.editor.handle_key(KvimKey::esc());
            let _ = self.editor.handle_key(KvimKey::char('i'));
        }
    }

    /// Insert `text` at the cursor — what `Ctrl+V` (and `Ctrl+Shift+V`) does
    /// (GH issue #280).
    ///
    /// With a Visual selection up, the selection is replaced, because that is
    /// what every other text box on the machine does with a paste. The delete
    /// is the engine's own `d`, so charwise, linewise and blockwise
    /// selections are all handled by kvim rather than re-derived here — and
    /// the replaced text lands in kvim's unnamed register, as `v…d` always
    /// would.
    ///
    /// Otherwise the text goes in **at** the cursor, i.e. before the grapheme
    /// the cursor sits on. That is a deliberate difference from Vim's `p`,
    /// which puts *after* it: `Ctrl+V` is the GUI gesture, and a GUI paste
    /// lands where the caret is. `p` is unchanged for anyone who wants Vim's
    /// behaviour.
    ///
    /// Works in every mode, Insert included, and is a single undoable edit
    /// ([`Editor::replace_range`] goes through the buffer's undo stack).
    pub fn paste(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if self.editor.mode().is_visual() {
            let _ = self.editor.handle_key(KvimKey::char('d'));
        }
        let at = self.editor.cursor();
        self.editor.replace_range(Range::point(at), text);
    }

    /// What `Ctrl+C` puts on the system clipboard: the Visual selection, or
    /// — with nothing selected — the whole cursor line, newline included.
    ///
    /// Copying the line rather than nothing is the useful reading of "copy"
    /// when no selection exists; it is also what makes `Ctrl+X` behave like
    /// `dd`, which is the Vim answer to the same gesture.
    pub fn clipboard_text(&self) -> String {
        self.selection_text().unwrap_or_else(|| {
            let line = self.editor.cursor().line;
            match self.editor.buffer().line(line) {
                Some(text) => format!("{text}\n"),
                None => String::new(),
            }
        })
    }

    /// [`Self::clipboard_text`], then delete what was copied — `Ctrl+X`.
    /// Returns the text so the caller can hand it to the clipboard.
    ///
    /// The deletion is `d` over the selection, or `dd` on the line, so it is
    /// kvim's own delete: undoable, and in the unnamed register afterwards.
    pub fn cut(&mut self) -> String {
        let text = self.clipboard_text();
        if self.editor.mode().is_visual() {
            let _ = self.editor.handle_key(KvimKey::char('d'));
        } else {
            let _ = self.editor.handle_key(KvimKey::char('d'));
            let _ = self.editor.handle_key(KvimKey::char('d'));
        }
        text
    }

    /// The text a Visual selection covers, or `None` outside Visual mode.
    ///
    /// All three visual granularities are distinct here, and conflating them
    /// is [`Editor::selection`]'s own documented "classic mistake":
    /// charwise includes the grapheme **under** the cursor (hence the `+ 1`),
    /// linewise takes whole lines and ignores the columns, and blockwise is
    /// the column rectangle, line by line.
    fn selection_text(&self) -> Option<String> {
        let (start, end) = self.editor.selection()?;
        let buffer = self.editor.buffer();
        Some(match self.editor.mode() {
            Mode::VisualLine => {
                let last_line = buffer.line_count().saturating_sub(1);
                let from = Position::new(start.line, 0);
                // Take the trailing newline with the line, unless this is the
                // last line and there is none to take.
                let to = if end.line < last_line {
                    Position::new(end.line + 1, 0)
                } else {
                    Position::new(end.line, buffer.line_len(end.line))
                };
                buffer.slice(Range::new(from, to))
            }
            Mode::VisualBlock => {
                let (first, last) = (start.col.min(end.col), start.col.max(end.col));
                (start.line..=end.line)
                    .map(|line| {
                        let len = buffer.line_len(line);
                        if first >= len {
                            return String::new();
                        }
                        buffer.slice(Range::new(
                            Position::new(line, first),
                            Position::new(line, (last + 1).min(len)),
                        ))
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            _ => {
                let len = buffer.line_len(end.line);
                buffer.slice(Range::new(
                    start,
                    Position::new(end.line, (end.col + 1).min(len)),
                ))
            }
        })
    }

    /// Draw and interact with the buffer. In `read_only` mode no edit
    /// happens — a single **click** returns its 0-based line so
    /// [`Self::ui_readonly`]'s caller can open a per-block editor. Returns
    /// `None` otherwise (and always, when editable).
    fn text_area(&mut self, ui: &mut egui::Ui, read_only: bool) -> Option<usize> {
        let font = FontId::monospace(CHAR_SIZE);
        let char_width = ui.ctx().fonts_mut(|f| f.glyph_width(&font, ' ')).max(1.0);
        let line_height = ui.ctx().fonts_mut(|f| f.row_height(&font)) * LINE_SPACING;

        let line_count = self.editor.buffer().line_count().max(1);
        let width = ui.available_width().max(400.0);
        let height = line_count as f32 * line_height;
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(width, height), Sense::click_and_drag());
        self.text_area_origin = rect.min;

        if let Some(line) = self.pending_scroll_to_line.take() {
            let y = rect.min.y + line as f32 * line_height;
            let target = Rect::from_min_size(Pos2::new(rect.min.x, y), Vec2::new(1.0, line_height));
            ui.scroll_to_rect(target, Some(egui::Align::Center));
        }

        let to_position = |pointer: Pos2| -> Position {
            let rel = pointer - rect.min;
            let line = (rel.y / line_height).max(0.0) as usize;
            let col = (rel.x / char_width).round().max(0.0) as usize;
            self.editor.buffer().clamp(Position::new(line, col))
        };

        if read_only {
            let mut clicked_line = None;
            if let Some(p) = response.interact_pointer_pos() {
                let line = to_position(p).line;
                if response.drag_started() {
                    self.preview_press_line = Some(line);
                }
                if response.clicked() {
                    clicked_line = Some(line);
                } else if response.drag_stopped() {
                    // A press egui promoted to a drag (held past 0.8 s, or
                    // wobbled past 6 px) never reports `clicked()`, so
                    // clicking a banded block to open it used to fail for
                    // exactly the unhurried clicks people make — #282. A
                    // press and release on the *same line* is a click; a
                    // drag across lines is not, and still opens nothing.
                    if self.preview_press_line.take() == Some(line) {
                        clicked_line = Some(line);
                    }
                }
            }
            if !response.is_pointer_button_down_on() {
                self.preview_press_line = None;
            }
            self.paint(ui, rect, char_width, line_height, line_count, &response);
            return clicked_line;
        }

        self.text_area_id = Some(response.id);

        if response.clicked() || response.drag_started() {
            response.request_focus();
        }
        // **Own the keyboard while focused** (GH issue #289). Without this,
        // egui's own focus system gets these keys first: `Memory::begin_pass`
        // reads the focused widget's `EventFilter`, and with the default one
        // **Escape surrenders focus**, Tab moves to the next widget and the
        // arrow keys move focus by direction. In a modal editor that is
        // fatal rather than untidy — pressing Esc to leave Insert dropped
        // focus, so the `u` after it never reached the engine at all and
        // fell through to the app's own shortcuts (`j`/`k` turn the PDF
        // reader's pages). The maintainer, 2026-09-23: "i want kvim to act
        // like vim keys ... u to undo etc."
        //
        // `set_focus_lock_filter` only takes effect from the *second* frame
        // of focus (it requires `had_focus_last_frame`), which is egui's own
        // constraint and the same one its `TextEdit` lives with.
        if response.has_focus() {
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    response.id,
                    egui::EventFilter {
                        tab: true,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: true,
                    },
                );
            });
        }
        // An editor opened by a click elsewhere (a page-context card, a
        // banded preview line) takes focus on its first frame — #282.
        if std::mem::take(&mut self.pending_focus) {
            response.request_focus();
        }

        // Discoverability for §29/§30's triggers — you cannot guess `@`/`[[`
        // from an empty buffer (maintainer, 2026-09-02). Deliberately a slow
        // tooltip: it should surface when someone pauses, not nag while they
        // are typing.
        ui.style_mut().interaction.tooltip_delay = 2.0;
        response
            .clone()
            .on_hover_text("use @ for citation autocomplete, and [[ for other autocomplete");

        if response.drag_started() {
            if let Some(pointer) = response.interact_pointer_pos() {
                let pos = to_position(pointer);
                // Leave Insert mode first (op-6w9n: a drag-to-select started
                // while still in Insert mode used to send the 'v' below
                // straight into the buffer as literal typed text — Insert
                // mode has no notion of "this keystroke means something
                // else", it just inserts whatever it's handed).
                if self.editor.mode() == Mode::Insert {
                    let _ = self.editor.handle_key(KvimKey::esc());
                }
                self.editor.move_cursor(pos);
                let _ = self.editor.handle_key(KvimKey::char('v')); // enter charwise Visual, anchored here
                self.dragging = true;
            }
        } else if self.dragging && response.dragged() {
            if let Some(pointer) = response.interact_pointer_pos() {
                self.editor.move_cursor(to_position(pointer));
            }
        } else if response.drag_stopped() {
            self.end_drag();
        } else if response.clicked() {
            if let Some(pointer) = response.interact_pointer_pos() {
                let pos = to_position(pointer);
                self.editor.move_cursor(pos);
                if self.editor.mode() != kopitiam_neovim::core::Mode::Insert {
                    let _ = self.editor.handle_key(KvimKey::esc());
                    let _ = self.editor.handle_key(KvimKey::char('i'));
                }
            }
        }

        if response.has_focus() {
            // Clipboard first, and separately from the key loop below: these
            // arrive as their own events rather than as keys (see the module
            // doc, GH issue #280), and acting on them needs the whole state,
            // not just `self.editor`.
            let mut clipboard = Vec::new();
            ui.input_mut(|i| {
                i.events.retain(|event| match clipboard_action(event) {
                    Some(action) => {
                        clipboard.push(action);
                        false
                    }
                    None => true,
                });
            });
            for action in clipboard {
                match action {
                    ClipboardAction::Paste(text) => self.paste(&text),
                    ClipboardAction::Copy => {
                        let text = self.clipboard_text();
                        if !text.is_empty() {
                            ui.ctx().copy_text(text);
                        }
                    }
                    ClipboardAction::Cut => {
                        let text = self.cut();
                        if !text.is_empty() {
                            ui.ctx().copy_text(text);
                        }
                    }
                }
            }

            // **Consume** the events this editor handles (GH issue #35
            // 2026-09-02: while typing in Insert mode, `j`/`k` etc. were
            // still reaching the PDF reader's page-turn shortcuts). A
            // focused text editor owns its keystrokes.
            let editor = &mut self.editor;
            ui.input_mut(|i| {
                i.events.retain(|event| match map_event(event) {
                    Some(key) => {
                        let _ = editor.handle_key(key);
                        false
                    }
                    None => true,
                });
            });
        }

        // **Follow the caret.** Typing past the bottom of the viewport -- or
        // moving with `j`/`G`/`n` -- used to leave it off-screen: this is a
        // custom-painted area rather than an `egui::TextEdit`, so the
        // surrounding `ScrollArea` has no idea where the caret is
        // (maintainer, 2026-09-24: "when i go to the next line, the scrollbar
        // should auto go down").
        //
        // `Align::None` scrolls the MINIMUM distance to bring the row into
        // view, so a caret already on screen does not recentre the text under
        // the reader's eyes -- the view moves only when it has to.
        let caret = self.editor.cursor();
        if self.last_cursor != Some(caret) {
            self.last_cursor = Some(caret);
            let target = Rect::from_min_size(
                Pos2::new(
                    rect.min.x + caret.col as f32 * char_width,
                    rect.min.y + caret.line as f32 * line_height,
                ),
                Vec2::new(char_width.max(1.0), line_height),
            );
            ui.scroll_to_rect(target, None);
        }

        self.paint(ui, rect, char_width, line_height, line_count, &response);
        None
    }

    /// Paint the bands, selection, text and cursor into `rect`. Shared by
    /// the editable and read-only [`Self::text_area`] paths.
    #[allow(clippy::too_many_arguments)]
    fn paint(
        &self,
        ui: &egui::Ui,
        rect: Rect,
        char_width: f32,
        line_height: f32,
        line_count: usize,
        response: &egui::Response,
    ) {
        let font = FontId::monospace(CHAR_SIZE);
        let painter = ui.painter_at(rect);

        let band = |range: &std::ops::Range<usize>, fill: Color32| {
            let y0 = rect.min.y + range.start as f32 * line_height;
            let y1 = rect.min.y + range.end.max(range.start + 1) as f32 * line_height;
            painter.rect_filled(
                Rect::from_min_max(Pos2::new(rect.min.x, y0), Pos2::new(rect.max.x, y1)),
                0.0,
                fill,
            );
        };
        for range in &self.anchor_bands {
            band(range, Color32::from_rgba_unmultiplied(255, 230, 60, 16));
        }
        if let Some(range) = &self.hover_band {
            band(range, Color32::from_rgba_unmultiplied(255, 230, 60, 48));
        }

        if let Some((from, to)) = self.editor.selection() {
            let (start, end) = if (from.line, from.col) <= (to.line, to.col) {
                (from, to)
            } else {
                (to, from)
            };
            for line in start.line..=end.line {
                let col_start = if line == start.line { start.col } else { 0 };
                let line_len = self.editor.buffer().line_len(line);
                let col_end = if line == end.line {
                    end.col.max(col_start + 1)
                } else {
                    line_len.max(col_start + 1)
                };
                let y = rect.min.y + line as f32 * line_height;
                let x0 = rect.min.x + col_start as f32 * char_width;
                let x1 = rect.min.x + col_end as f32 * char_width;
                painter.rect_filled(
                    Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x1, y + line_height)),
                    0.0,
                    Color32::from_rgba_unmultiplied(100, 140, 220, 90),
                );
            }
        }

        for line in 0..line_count {
            let Some(text) = self.editor.buffer().line(line) else {
                continue;
            };
            let y = rect.min.y + line as f32 * line_height;
            painter.text(
                Pos2::new(rect.min.x, y),
                egui::Align2::LEFT_TOP,
                text,
                font.clone(),
                ui.visuals().text_color(),
            );
        }

        let cursor = self.editor.cursor();
        let cx = rect.min.x + cursor.col as f32 * char_width;
        let cy = rect.min.y + cursor.line as f32 * line_height;
        let cursor_color = if response.has_focus() {
            Color32::from_rgb(230, 180, 60)
        } else {
            Color32::from_gray(140)
        };
        painter.rect_filled(
            Rect::from_min_size(
                Pos2::new(cx, cy),
                Vec2::new(char_width.max(2.0), line_height),
            ),
            0.0,
            cursor_color.gamma_multiply(0.5),
        );
        painter.rect_stroke(
            Rect::from_min_size(
                Pos2::new(cx, cy),
                Vec2::new(char_width.max(2.0), line_height),
            ),
            0.0,
            Stroke::new(1.0, cursor_color),
            egui::StrokeKind::Outside,
        );
    }
}

/// One clipboard gesture taken off the event queue, to be applied after the
/// borrow of `ui.input_mut` ends.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ClipboardAction {
    Paste(String),
    Copy,
    Cut,
}

/// Recognise the three clipboard events egui delivers instead of keys
/// (GH issue #280). `None` leaves the event on the queue for [`map_event`].
fn clipboard_action(event: &egui::Event) -> Option<ClipboardAction> {
    match event {
        egui::Event::Paste(text) => Some(ClipboardAction::Paste(text.clone())),
        egui::Event::Copy => Some(ClipboardAction::Copy),
        egui::Event::Cut => Some(ClipboardAction::Cut),
        _ => None,
    }
}

/// Map one egui input event onto a `kopitiam-neovim` [`KvimKey`], or `None`
/// for events this adapter does not forward (pointer movement, focus
/// changes, and a *repeat* of a key already delivered as text).
fn map_event(event: &egui::Event) -> Option<KvimKey> {
    match event {
        egui::Event::Text(text) => text.chars().next().map(KvimKey::char),
        egui::Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } => {
            let mods = KvimModifiers {
                ctrl: modifiers.ctrl,
                alt: modifiers.alt,
                shift: modifiers.shift,
            };
            let code = match key {
                egui::Key::Enter => KvimKeyCode::Enter,
                egui::Key::Escape => KvimKeyCode::Esc,
                egui::Key::Backspace => KvimKeyCode::Backspace,
                egui::Key::Tab => KvimKeyCode::Tab,
                egui::Key::ArrowLeft => KvimKeyCode::Left,
                egui::Key::ArrowRight => KvimKeyCode::Right,
                egui::Key::ArrowUp => KvimKeyCode::Up,
                egui::Key::ArrowDown => KvimKeyCode::Down,
                egui::Key::Home => KvimKeyCode::Home,
                egui::Key::End => KvimKeyCode::End,
                egui::Key::PageUp => KvimKeyCode::PageUp,
                egui::Key::PageDown => KvimKeyCode::PageDown,
                egui::Key::Delete => KvimKeyCode::Delete,
                // A plain, unmodified letter/digit already arrives as
                // `Event::Text` — only forward it here for a Ctrl-chord
                // (`<C-d>`, `<C-r>`, …), which egui does not also emit as
                // text.
                _ if modifiers.ctrl => key
                    .name()
                    .chars()
                    .next()
                    .map(|c| c.to_ascii_lowercase())
                    .map(KvimKeyCode::Char)?,
                _ => return None,
            };
            Some(KvimKey::new(code, mods))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_text_then_text_round_trips() {
        let mut state = KvimEditorState::default();
        state.load_text("# Title\n\n## Summary\n\nBody.\n");
        assert_eq!(state.text(), "# Title\n\n## Summary\n\nBody.\n");
        assert!(!state.is_modified());
    }

    /// op-6w9n: entering Visual mode for a mouse-drag selection must not
    /// leak a literal 'v' into the buffer when the drag starts from Insert
    /// mode — reproduces the maintainer's dogfooding report by driving the
    /// same handle_key sequence `text_area`'s drag_started branch now uses.
    #[test]
    fn entering_visual_mode_from_insert_does_not_type_a_literal_v() {
        let mut state = KvimEditorState::default();
        state.load_text("hello world\n");
        state.editor.handle_key(KvimKey::char('i')).unwrap();
        assert_eq!(state.editor.mode(), Mode::Insert);

        // The fixed drag_started sequence: Esc out of Insert first, then 'v'.
        state.editor.handle_key(KvimKey::esc()).unwrap();
        state.editor.handle_key(KvimKey::char('v')).unwrap();

        assert_eq!(
            state.text(),
            "hello world\n",
            "no 'v' should have been inserted into the buffer"
        );
        assert_eq!(state.editor.mode(), Mode::Visual);
    }

    /// op-j178: `Artifact::line` is 1-based ("1-based line of the heading");
    /// `jump_to_line` must land the cursor at 0-based `line - 1`, not `line`.
    #[test]
    fn jump_to_line_moves_the_cursor_to_the_zero_based_equivalent() {
        let mut state = KvimEditorState::default();
        state.load_text("a\nb\nc\nd\n");
        state.jump_to_line(3);
        assert_eq!(state.editor.cursor(), Position::new(2, 0));
        assert_eq!(state.pending_scroll_to_line, Some(2));
    }

    #[test]
    fn jump_to_line_clamps_past_the_end_of_the_buffer() {
        let mut state = KvimEditorState::default();
        state.load_text("a\nb\n");
        state.jump_to_line(999);
        assert!(state.editor.cursor().line < 999);
    }

    // ------------------------------------------------------------------
    // GH issue #289 — the editor owns the keyboard while it has focus.
    // ------------------------------------------------------------------

    /// Drive the editable editor headlessly (no window, no GPU) across
    /// `frames`, each a list of events, and give back the buffer at the end.
    fn drive(state: &mut KvimEditorState, frames: Vec<Vec<egui::Event>>) -> String {
        let ctx = egui::Context::default();
        for (n, events) in frames.into_iter().enumerate() {
            let input = egui::RawInput {
                time: Some(n as f64 * 0.1),
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                events,
                ..Default::default()
            };
            let _ = ctx.run_ui(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    state.ui(ui, None);
                });
            });
        }
        state.text()
    }

    fn key(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// The maintainer, 2026-09-23: "i want kvim to act like vim keys ... u to
    /// undo etc." Type in Insert, press Escape, press `u` — and the edit is
    /// undone.
    ///
    /// The failure this pins is not in the engine (which undoes correctly
    /// when driven directly) but in egui's focus system: with a default
    /// `EventFilter`, **Escape surrenders focus**, so the `u` that follows
    /// never reaches the editor and is handled by the app instead. Checked
    /// capable of failing by removing the `set_focus_lock_filter` call.
    #[test]
    fn escape_then_u_undoes_rather_than_dropping_focus() {
        let mut state = KvimEditorState::default();
        state.load_text("hello\n");
        state.begin_insert();

        let text = drive(
            &mut state,
            vec![
                Vec::new(),                          // focus lands
                Vec::new(),                          // focus lock applies
                vec![egui::Event::Text("X".into())], // type in Insert
                vec![key(egui::Key::Escape)],        // leave Insert
                vec![egui::Event::Text("u".into())], // undo
            ],
        );

        assert_eq!(
            state.mode_label(),
            "NORMAL",
            "Escape did not reach the engine"
        );
        assert_eq!(text, "hello\n", "`u` did not undo: {text:?}");
    }

    /// Tab and the arrow keys are the editor's too, not egui's focus
    /// navigation — the same filter, and the same failure mode if it is not
    /// set: the keystroke moves focus to another widget instead of editing.
    #[test]
    fn arrow_keys_move_the_cursor_instead_of_moving_focus() {
        let mut state = KvimEditorState::default();
        state.load_text("abc\ndef\n");
        state.begin_insert();

        drive(
            &mut state,
            vec![
                Vec::new(),
                Vec::new(),
                vec![key(egui::Key::Escape)],
                vec![key(egui::Key::ArrowDown), key(egui::Key::ArrowRight)],
                vec![egui::Event::Text("x".into())],
            ],
        );

        // `x` in Normal mode deletes the grapheme under the cursor, so where
        // it landed says where the arrows took it.
        assert_eq!(
            state.text(),
            "abc\ndf\n",
            "the arrows did not move the cursor to line 2, column 2"
        );
    }

    // ------------------------------------------------------------------
    // GH issue #282 — click-to-insert.
    // ------------------------------------------------------------------

    /// The maintainer's report, 2026-09-23: clicking the page-context editor
    /// did not enter Insert mode. egui calls a press a *drag* once it passes
    /// `max_click_dist` (6 px) or `max_click_duration` (0.8 s), so an
    /// unhurried click never reached the `clicked()` arm and left the editor
    /// in the Visual mode the drag had entered. A drag that selected nothing
    /// is a click.
    #[test]
    fn a_drag_that_selected_nothing_is_a_click_and_enters_insert_mode() {
        let mut state = KvimEditorState::default();
        state.load_text("hello world\n");
        // Exactly what `text_area` does on `drag_started` with no movement.
        state.editor.move_cursor(Position::new(0, 4));
        state.editor.handle_key(KvimKey::char('v')).unwrap();
        state.dragging = true;

        state.end_drag();

        assert_eq!(state.editor.mode(), Mode::Insert);
        assert!(!state.dragging);
        assert_eq!(
            state.text(),
            "hello world\n",
            "no key leaked into the buffer"
        );
    }

    /// A drag that *did* select something is a real selection and is left
    /// alone — the fix must not undo dragging-to-select.
    #[test]
    fn a_drag_that_selected_text_stays_in_visual_mode() {
        let mut state = KvimEditorState::default();
        state.load_text("hello world\n");
        select(&mut state, Position::new(0, 0), Position::new(0, 4));
        state.dragging = true;

        state.end_drag();

        assert_eq!(state.editor.mode(), Mode::Visual);
        assert_eq!(state.clipboard_text(), "hello");
    }

    /// An editor opened by a click elsewhere (a page-context card, a banded
    /// preview line) comes up ready to type, and asks for focus once.
    #[test]
    fn begin_insert_opens_ready_to_type_and_claims_focus_once() {
        let mut state = KvimEditorState::default();
        state.load_text("the prose body\n");
        assert_eq!(
            state.editor.mode(),
            Mode::Normal,
            "load_text starts in Normal"
        );

        state.begin_insert();

        assert_eq!(state.editor.mode(), Mode::Insert);
        assert!(state.pending_focus);
        // The flag is one-shot: the next paint consumes it.
        assert!(std::mem::take(&mut state.pending_focus));
        assert!(!state.pending_focus);
        assert_eq!(
            state.text(),
            "the prose body\n",
            "no key leaked into the buffer"
        );
    }

    /// Drive the read-only preview headlessly (no window, no GPU) through a
    /// press at one position and a release at the same position `hold`
    /// seconds later, and report which line it opened.
    fn preview_press_release(hold: f64) -> Option<usize> {
        let ctx = egui::Context::default();
        let mut state = KvimEditorState::default();
        state.load_text("one\ntwo\nthree\nfour\nfive\n");
        let pos = egui::pos2(30.0, 70.0);
        let mut opened = None;

        let mut frame = |time: f64, events: Vec<egui::Event>| {
            let input = egui::RawInput {
                time: Some(time),
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                events,
                ..Default::default()
            };
            let _ = ctx.run_ui(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    if let Some(line) = state.ui_readonly(ui) {
                        opened = Some(line);
                    }
                });
            });
        };

        let button = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(0.0, vec![egui::Event::PointerMoved(pos)]);
        frame(0.1, vec![button(true)]);
        frame(0.1 + hold, Vec::new());
        frame(0.1 + hold, vec![button(false)]);
        opened
    }

    /// GH issue #282, the other half: the page-context **preview** opens a
    /// block on click, and a press egui promoted to a drag (held past its
    /// 0.8 s `max_click_duration`) never reports `clicked()` — so an
    /// unhurried click on a banded block used to open nothing at all.
    ///
    /// Asserted as "the slow press opens the same line the quick one does",
    /// so the test does not depend on font metrics or panel layout.
    #[test]
    fn a_slow_press_on_the_preview_opens_the_same_line_a_quick_click_does() {
        let quick = preview_press_release(0.0);
        assert!(
            quick.is_some(),
            "the quick click did not land on the text area"
        );
        assert_eq!(
            preview_press_release(1.5),
            quick,
            "a press held past egui's max_click_duration must still open its line"
        );
    }

    // ------------------------------------------------------------------
    // GH issue #280 — the system clipboard.
    // ------------------------------------------------------------------

    /// Put the editor in charwise Visual mode from `from` to `to`, the way
    /// a mouse drag does.
    fn select(state: &mut KvimEditorState, from: Position, to: Position) {
        state.editor.move_cursor(from);
        state.editor.handle_key(KvimKey::char('v')).unwrap();
        state.editor.move_cursor(to);
    }

    /// The three events egui delivers *instead of* a key are recognised, and
    /// nothing else is taken off the queue — a typed character must still
    /// reach the editor as text.
    #[test]
    fn only_the_clipboard_events_are_taken_off_the_queue() {
        assert_eq!(
            clipboard_action(&egui::Event::Paste("hello".into())),
            Some(ClipboardAction::Paste("hello".into()))
        );
        assert_eq!(
            clipboard_action(&egui::Event::Copy),
            Some(ClipboardAction::Copy)
        );
        assert_eq!(
            clipboard_action(&egui::Event::Cut),
            Some(ClipboardAction::Cut)
        );
        assert_eq!(clipboard_action(&egui::Event::Text("a".into())), None);
        assert_eq!(
            clipboard_action(&egui::Event::Key {
                key: egui::Key::V,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::CTRL,
            }),
            None,
            "a Ctrl+V that did arrive as a key is kvim's <C-v>, not a paste"
        );
    }

    /// The maintainer's actual case (2026-09-23): a multi-line licence
    /// statement pasted into a paper's `## Summary` while ingesting the NJOY
    /// manual. Pastes at the cursor, keeps its line breaks, and marks the
    /// buffer modified so Save offers itself.
    #[test]
    fn pasting_lands_at_the_cursor_and_keeps_its_line_breaks() {
        let mut state = KvimEditorState::default();
        state.load_text("# njoy2016\n\n## Summary\n\n\n");
        state.editor.move_cursor(Position::new(4, 0));
        state.paste("Distributed under the LGPL.\nSee LICENSE.\n");
        assert_eq!(
            state.text(),
            "# njoy2016\n\n## Summary\n\nDistributed under the LGPL.\nSee LICENSE.\n\n"
        );
        assert!(state.is_modified());
    }

    /// A paste with a selection up replaces it, as every other text box on
    /// the machine does.
    #[test]
    fn pasting_over_a_selection_replaces_it() {
        let mut state = KvimEditorState::default();
        state.load_text("hello world\n");
        select(&mut state, Position::new(0, 0), Position::new(0, 4));
        state.paste("goodbye");
        assert_eq!(state.text(), "goodbye world\n");
        assert_eq!(state.editor.mode(), Mode::Normal);
    }

    /// Pasting mid-typing must work too — Insert mode is where a user
    /// actually is when they reach for Ctrl+V.
    #[test]
    fn pasting_works_from_insert_mode() {
        let mut state = KvimEditorState::default();
        state.load_text("ab\n");
        state.editor.handle_key(KvimKey::char('i')).unwrap();
        state.editor.handle_key(KvimKey::char('X')).unwrap();
        state.paste("YZ");
        assert_eq!(state.text(), "XYZab\n");
    }

    #[test]
    fn an_empty_paste_changes_nothing() {
        let mut state = KvimEditorState::default();
        state.load_text("hello\n");
        state.paste("");
        assert_eq!(state.text(), "hello\n");
        assert!(!state.is_modified());
    }

    /// Copy takes the selection, including the grapheme under the cursor —
    /// the charwise rule `Editor::selection` warns about.
    #[test]
    fn copy_takes_the_charwise_selection_cursor_grapheme_included() {
        let mut state = KvimEditorState::default();
        state.load_text("hello world\n");
        select(&mut state, Position::new(0, 6), Position::new(0, 10));
        assert_eq!(state.clipboard_text(), "world");
    }

    /// A linewise selection takes whole lines with their newlines, columns
    /// ignored — the other half of that rule.
    #[test]
    fn copy_takes_whole_lines_in_visual_line_mode() {
        let mut state = KvimEditorState::default();
        state.load_text("one\ntwo\nthree\n");
        state.editor.move_cursor(Position::new(0, 2));
        state.editor.handle_key(KvimKey::char('V')).unwrap();
        state.editor.move_cursor(Position::new(1, 1));
        assert_eq!(state.editor.mode(), Mode::VisualLine);
        assert_eq!(state.clipboard_text(), "one\ntwo\n");
    }

    /// With nothing selected, Ctrl+C copies the cursor line — and Ctrl+X
    /// removes it, i.e. behaves as `dd`.
    #[test]
    fn with_no_selection_copy_takes_the_line_and_cut_removes_it() {
        let mut state = KvimEditorState::default();
        state.load_text("one\ntwo\nthree\n");
        state.editor.move_cursor(Position::new(1, 1));
        assert_eq!(state.clipboard_text(), "two\n");
        assert_eq!(state.cut(), "two\n");
        assert_eq!(state.text(), "one\nthree\n");
    }

    #[test]
    fn cutting_a_selection_returns_it_and_removes_it() {
        let mut state = KvimEditorState::default();
        state.load_text("hello world\n");
        select(&mut state, Position::new(0, 0), Position::new(0, 5));
        assert_eq!(state.cut(), "hello ");
        assert_eq!(state.text(), "world\n");
    }

    #[test]
    fn typing_marks_the_buffer_modified() {
        let mut state = KvimEditorState::default();
        state.load_text("hello\n");
        // Enter insert mode and type, exactly as the click-to-insert path does.
        state.editor.handle_key(KvimKey::char('i')).unwrap();
        state.editor.handle_key(KvimKey::char('X')).unwrap();
        assert!(state.text().starts_with('X'));
        assert!(state.is_modified());
    }

    #[test]
    fn sync_to_disk_text_reloads_a_clean_buffer_but_never_a_dirty_one() {
        let mut state = KvimEditorState::default();
        state.load_text("original\n");

        // Clean buffer: an external change is pulled in.
        assert!(state.sync_to_disk_text("external change\n"));
        assert_eq!(state.text(), "external change\n");
        assert!(!state.is_modified());

        // Already in sync: no-op.
        assert!(!state.sync_to_disk_text("external change\n"));

        // Dirty buffer: the external change is refused, edits kept.
        state.editor.handle_key(KvimKey::char('i')).unwrap();
        state.editor.handle_key(KvimKey::char('Z')).unwrap();
        assert!(state.is_modified());
        assert!(!state.sync_to_disk_text("something else\n"));
        assert!(state.text().starts_with('Z'));
    }

    #[test]
    fn map_event_forwards_plain_text_and_navigation_keys() {
        let text_event = egui::Event::Text("a".to_string());
        assert_eq!(map_event(&text_event), Some(KvimKey::char('a')));

        let esc = egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        };
        assert_eq!(map_event(&esc), Some(KvimKey::esc()));
    }

    #[test]
    fn map_event_ignores_key_up_and_unmodified_letters_already_sent_as_text() {
        let key_up = egui::Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        };
        assert_eq!(map_event(&key_up), None);

        let plain_a = egui::Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        };
        assert_eq!(
            map_event(&plain_a),
            None,
            "a plain letter arrives via Event::Text, not Event::Key"
        );
    }

    #[test]
    fn map_event_forwards_a_ctrl_chord() {
        let ctrl_d = egui::Event::Key {
            key: egui::Key::D,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                ctrl: true,
                ..Default::default()
            },
        };
        assert_eq!(map_event(&ctrl_d), Some(KvimKey::ctrl('d')));
    }

    // ------------------------------------------------------------------
    // Caret-follow scrolling (maintainer, 2026-09-24).
    // ------------------------------------------------------------------

    /// The editor must know where the caret is after a frame, because that is
    /// what the ScrollArea is asked to follow. A custom-painted text area gets
    /// none of this for free the way `egui::TextEdit` would.
    #[test]
    fn a_frame_records_where_the_caret_is() {
        let mut state = KvimEditorState::default();
        state.load_text("one\ntwo\nthree\n");
        assert_eq!(state.last_cursor, None, "nothing recorded before a frame");
        drive(&mut state, vec![vec![]]);
        assert_eq!(
            state.last_cursor,
            Some(state.editor.cursor()),
            "after a frame the recorded caret is the live one"
        );
    }

    /// Moving down a line must change the recorded caret -- that difference is
    /// precisely the signal that triggers a scroll. If this stops changing,
    /// the view silently stops following.
    #[test]
    fn moving_to_the_next_line_moves_the_recorded_caret() {
        let mut state = KvimEditorState::default();
        state.load_text("one\ntwo\nthree\nfour\n");
        state.begin_insert();
        // All frames in ONE `drive`: it builds a fresh `egui::Context` per
        // call, so focus does not survive across calls.
        drive(
            &mut state,
            vec![
                Vec::new(),                          // focus lands
                Vec::new(),                          // focus lock applies
                vec![key(egui::Key::Escape)],        // to Normal
                vec![egui::Event::Text("j".into())], // down one line
            ],
        );
        assert_eq!(
            state.last_cursor.expect("recorded").line,
            1,
            "j must move down a line, and that move must be recorded"
        );
    }

    /// Typing a newline in Insert mode moves the caret down -- the exact
    /// reported case ("when i go to the next line, the scrollbar should auto
    /// go down").
    #[test]
    fn a_newline_in_insert_mode_advances_the_recorded_caret() {
        let mut state = KvimEditorState::default();
        state.load_text("");
        state.begin_insert();
        drive(
            &mut state,
            vec![Vec::new(), Vec::new(), vec![key(egui::Key::Enter)]],
        );
        assert_eq!(
            state.last_cursor.expect("recorded").line,
            1,
            "Enter in Insert mode must advance the recorded caret a line"
        );
    }

    /// An idle frame must NOT re-trigger a scroll, or the view would be pinned
    /// to the caret and wheel-scrolling a focused editor would snap straight
    /// back. The caret is unchanged across consecutive idle frames.
    #[test]
    fn an_idle_frame_does_not_move_the_caret() {
        let mut state = KvimEditorState::default();
        state.load_text("one\ntwo\n");
        drive(&mut state, vec![vec![]]);
        let first = state.last_cursor;
        drive(&mut state, vec![vec![], vec![]]);
        assert_eq!(
            state.last_cursor, first,
            "idle frames leave the caret alone, so nothing asks to scroll"
        );
    }
}
