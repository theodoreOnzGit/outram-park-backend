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

use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use kopitiam_neovim::core::{Position, Range};
use kopitiam_neovim::editor::key::{Key as KvimKey, KeyCode as KvimKeyCode, Modifiers as KvimModifiers};
use kopitiam_neovim::editor::Editor;

const CHAR_SIZE: f32 = 14.0;
const LINE_SPACING: f32 = 1.35;

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
}

impl Default for KvimEditorState {
    fn default() -> Self {
        Self { editor: Editor::new(), loaded_text: String::new(), dragging: false }
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
        let end = self.editor.buffer().clamp(Position::new(usize::MAX, usize::MAX));
        self.editor.replace_range(Range::new(Position::ORIGIN, end), text);
        self.editor.move_cursor(Position::ORIGIN);
        self.loaded_text = text.to_string();
        self.dragging = false;
    }

    /// The buffer's current text.
    pub fn text(&self) -> String {
        self.editor.buffer().text()
    }

    /// Whether the text has changed since the last [`Self::load_text`].
    pub fn is_modified(&self) -> bool {
        self.text() != self.loaded_text
    }

    /// Draw the editor and process this frame's input for it. `ui`'s
    /// available space is fully claimed by a scrollable text area plus a
    /// one-line mode/status bar.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.strong(self.editor.mode().label());
            let pos = self.editor.cursor();
            ui.weak(format!("{}:{}", pos.line + 1, pos.col + 1));
            if self.is_modified() {
                ui.weak("[+]");
            }
        });
        ui.separator();

        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            self.text_area(ui);
        });
    }

    fn text_area(&mut self, ui: &mut egui::Ui) {
        let font = FontId::monospace(CHAR_SIZE);
        let char_width = ui.ctx().fonts_mut(|f| f.glyph_width(&font, ' ')).max(1.0);
        let line_height = ui.ctx().fonts_mut(|f| f.row_height(&font)) * LINE_SPACING;

        let line_count = self.editor.buffer().line_count().max(1);
        let width = ui.available_width().max(400.0);
        let height = line_count as f32 * line_height;
        let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click_and_drag());

        if response.clicked() || response.drag_started() {
            response.request_focus();
        }

        let to_position = |pointer: Pos2| -> Position {
            let rel = pointer - rect.min;
            let line = (rel.y / line_height).max(0.0) as usize;
            let col = (rel.x / char_width).round().max(0.0) as usize;
            self.editor.buffer().clamp(Position::new(line, col))
        };

        if response.drag_started() {
            if let Some(pointer) = response.interact_pointer_pos() {
                let pos = to_position(pointer);
                self.editor.move_cursor(pos);
                let _ = self.editor.handle_key(KvimKey::char('v')); // enter charwise Visual, anchored here
                self.dragging = true;
            }
        } else if self.dragging && response.dragged() {
            if let Some(pointer) = response.interact_pointer_pos() {
                self.editor.move_cursor(to_position(pointer));
            }
        } else if response.drag_stopped() {
            self.dragging = false;
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
            for event in ui.input(|i| i.events.clone()) {
                if let Some(key) = map_event(&event) {
                    let _ = self.editor.handle_key(key);
                }
            }
        }

        let painter = ui.painter_at(rect);

        if let Some((from, to)) = self.editor.selection() {
            let (start, end) = if (from.line, from.col) <= (to.line, to.col) { (from, to) } else { (to, from) };
            for line in start.line..=end.line {
                let col_start = if line == start.line { start.col } else { 0 };
                let line_len = self.editor.buffer().line_len(line);
                let col_end = if line == end.line { end.col.max(col_start + 1) } else { line_len.max(col_start + 1) };
                let y = rect.min.y + line as f32 * line_height;
                let x0 = rect.min.x + col_start as f32 * char_width;
                let x1 = rect.min.x + col_end as f32 * char_width;
                painter.rect_filled(Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x1, y + line_height)), 0.0, Color32::from_rgba_unmultiplied(100, 140, 220, 90));
            }
        }

        for line in 0..line_count {
            let Some(text) = self.editor.buffer().line(line) else { continue };
            let y = rect.min.y + line as f32 * line_height;
            painter.text(Pos2::new(rect.min.x, y), egui::Align2::LEFT_TOP, text, font.clone(), ui.visuals().text_color());
        }

        let cursor = self.editor.cursor();
        let cx = rect.min.x + cursor.col as f32 * char_width;
        let cy = rect.min.y + cursor.line as f32 * line_height;
        let cursor_color = if response.has_focus() { Color32::from_rgb(230, 180, 60) } else { Color32::from_gray(140) };
        painter.rect_filled(
            Rect::from_min_size(Pos2::new(cx, cy), Vec2::new(char_width.max(2.0), line_height)),
            0.0,
            cursor_color.gamma_multiply(0.5),
        );
        painter.rect_stroke(
            Rect::from_min_size(Pos2::new(cx, cy), Vec2::new(char_width.max(2.0), line_height)),
            0.0,
            Stroke::new(1.0, cursor_color),
            egui::StrokeKind::Outside,
        );
    }
}

/// Map one egui input event onto a `kopitiam-neovim` [`KvimKey`], or `None`
/// for events this adapter does not forward (pointer movement, focus
/// changes, and a *repeat* of a key already delivered as text).
fn map_event(event: &egui::Event) -> Option<KvimKey> {
    match event {
        egui::Event::Text(text) => text.chars().next().map(KvimKey::char),
        egui::Event::Key { key, pressed: true, modifiers, .. } => {
            let mods = KvimModifiers { ctrl: modifiers.ctrl, alt: modifiers.alt, shift: modifiers.shift };
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
                _ if modifiers.ctrl => key.name().chars().next().map(|c| c.to_ascii_lowercase()).map(KvimKeyCode::Char)?,
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
        assert_eq!(map_event(&plain_a), None, "a plain letter arrives via Event::Text, not Event::Key");
    }

    #[test]
    fn map_event_forwards_a_ctrl_chord() {
        let ctrl_d = egui::Event::Key {
            key: egui::Key::D,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers { ctrl: true, ..Default::default() },
        };
        assert_eq!(map_event(&ctrl_d), Some(KvimKey::ctrl('d')));
    }
}
