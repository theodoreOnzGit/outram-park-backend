//! Repository Browser tab — a human-facing view over `kovan_discovery`.
//!
//! Type a root directory, cycle the [`FileKind`] filter (or "all"), and press
//! Enter to walk the tree with the same `.gitignore`-aware discovery the
//! `kovan discover` CLI subcommand uses (`kovan_discovery::discover` /
//! `discover_kind`). This tab only *reads* the filesystem.

use std::path::PathBuf;

use kovan_discovery::{discover, discover_kind, FileKind};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use super::text_input::TextInput;

/// The active discovery filter: every [`FileKind`], plus an "all files" option
/// the library enum itself doesn't model (that's [`discover`] with no
/// extension filter, vs. [`discover_kind`] for a specific kind).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KindFilter {
    All,
    Kind(FileKind),
}

impl KindFilter {
    const ORDER: [KindFilter; 5] = [
        KindFilter::All,
        KindFilter::Kind(FileKind::Source),
        KindFilter::Kind(FileKind::Markdown),
        KindFilter::Kind(FileKind::Pdf),
        KindFilter::Kind(FileKind::Metadata),
    ];

    fn label(self) -> &'static str {
        match self {
            KindFilter::All => "all",
            KindFilter::Kind(FileKind::Source) => "source",
            KindFilter::Kind(FileKind::Markdown) => "markdown",
            KindFilter::Kind(FileKind::Pdf) => "pdf",
            KindFilter::Kind(FileKind::Metadata) => "metadata",
        }
    }

    fn step(self, delta: i32) -> Self {
        let i = Self::ORDER.iter().position(|k| *k == self).unwrap_or(0) as i32;
        let n = Self::ORDER.len() as i32;
        let j = ((i + delta) % n + n) % n;
        Self::ORDER[j as usize]
    }
}

/// State for the Repository Browser tab.
pub struct BrowserState {
    pub root: TextInput,
    pub kind: KindFilter,
    pub files: Vec<PathBuf>,
    pub list_state: ListState,
    pub status: String,
}

impl Default for BrowserState {
    fn default() -> Self {
        Self {
            root: TextInput::new("."),
            kind: KindFilter::All,
            files: Vec::new(),
            list_state: ListState::default(),
            status: "press 'e' to edit the root path, Enter to scan".to_string(),
        }
    }
}

impl BrowserState {
    /// Re-run discovery against the current root/filter. Never panics on a
    /// missing root — [`discover`]/[`discover_kind`] already return an empty
    /// `Vec` for one, but a missing root gets its own status message here
    /// rather than silently showing "0 files" (that could otherwise look
    /// identical to "a real empty directory").
    fn run_discovery(&mut self) {
        let root = PathBuf::from(self.root.value());
        if !root.exists() {
            self.files.clear();
            self.list_state.select(None);
            self.status = format!("root does not exist: {}", root.display());
            return;
        }
        self.files = match self.kind {
            KindFilter::All => discover(&root, &[]),
            KindFilter::Kind(k) => discover_kind(&root, k),
        };
        self.status = format!(
            "{} file(s) — filter: {}",
            self.files.len(),
            self.kind.label()
        );
        self.list_state
            .select(if self.files.is_empty() { None } else { Some(0) });
    }

    fn select_next(&mut self) {
        if self.files.is_empty() {
            return;
        }
        let i = self
            .list_state
            .selected()
            .map(|i| (i + 1) % self.files.len())
            .unwrap_or(0);
        self.list_state.select(Some(i));
    }

    fn select_prev(&mut self) {
        if self.files.is_empty() {
            return;
        }
        let i = self
            .list_state
            .selected()
            .map(|i| if i == 0 { self.files.len() - 1 } else { i - 1 })
            .unwrap_or(0);
        self.list_state.select(Some(i));
    }

    /// Handle one key event. `editing` is the shared edit-mode flag owned by
    /// [`super::App`]: while `true`, keys type into `root` instead of
    /// navigating.
    pub fn handle_key(&mut self, key: KeyEvent, editing: &mut bool) {
        if *editing {
            match key.code {
                KeyCode::Enter => {
                    *editing = false;
                    self.run_discovery();
                }
                KeyCode::Esc => *editing = false,
                KeyCode::Backspace => self.root.backspace(),
                KeyCode::Char(c) => self.root.push_char(c),
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Char('e') => *editing = true,
            KeyCode::Enter => self.run_discovery(),
            KeyCode::Left => self.kind = self.kind.step(-1),
            KeyCode::Right => self.kind = self.kind.step(1),
            KeyCode::Down => self.select_next(),
            KeyCode::Up => self.select_prev(),
            _ => {}
        }
    }
}

/// Render the Browser tab into `area`.
pub fn draw(frame: &mut Frame, area: Rect, state: &mut BrowserState, editing: bool) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);

    let root_style = if editing {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    };
    frame.render_widget(
        Paragraph::new(state.root.value()).style(root_style).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Root ('e' to edit, Enter to scan)"),
        ),
        chunks[0],
    );

    frame.render_widget(
        Paragraph::new(format!(
            "Filter: {}  (Left/Right to change)   {}",
            state.kind.label(),
            state.status
        )),
        chunks[1],
    );

    let items: Vec<ListItem> = state
        .files
        .iter()
        .map(|p| ListItem::new(p.display().to_string()))
        .collect();
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Files"))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    frame.render_stateful_widget(list, chunks[2], &mut state.list_state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn edit_mode_types_into_root_and_enter_confirms_and_scans() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").expect("write fixture");

        let mut state = BrowserState::default();
        let mut editing = false;

        state.handle_key(key(KeyCode::Char('e')), &mut editing);
        assert!(editing, "'e' must enter edit mode");

        state.root.clear();
        for c in dir.path().to_str().expect("utf8 tempdir path").chars() {
            state.handle_key(key(KeyCode::Char(c)), &mut editing);
        }
        state.handle_key(key(KeyCode::Enter), &mut editing);

        assert!(!editing, "Enter must exit edit mode");
        assert_eq!(state.files.len(), 1);
        assert!(state.files[0].ends_with("a.rs"));
    }

    #[test]
    fn esc_cancels_editing_without_running_a_scan() {
        let mut state = BrowserState::default();
        let mut editing = false;
        state.handle_key(key(KeyCode::Char('e')), &mut editing);
        state.handle_key(key(KeyCode::Char('x')), &mut editing);
        state.handle_key(key(KeyCode::Esc), &mut editing);
        assert!(!editing);
        assert_eq!(state.root.value(), ".x");
        assert!(state.files.is_empty());
    }

    #[test]
    fn kind_filter_cycles_and_wraps_both_directions() {
        let mut state = BrowserState::default();
        let mut editing = false;
        assert_eq!(state.kind, KindFilter::All);

        state.handle_key(key(KeyCode::Left), &mut editing);
        assert_eq!(
            state.kind,
            KindFilter::Kind(FileKind::Metadata),
            "Left from All must wrap to the last entry"
        );

        state.handle_key(key(KeyCode::Right), &mut editing);
        assert_eq!(state.kind, KindFilter::All);
        state.handle_key(key(KeyCode::Right), &mut editing);
        assert_eq!(state.kind, KindFilter::Kind(FileKind::Source));
    }

    #[test]
    fn nonexistent_root_reports_status_without_panicking() {
        let mut state = BrowserState {
            root: TextInput::new("/this/path/should/not/exist/kovan-tui-test"),
            ..Default::default()
        };
        state.run_discovery();
        assert!(state.files.is_empty());
        assert!(state.status.contains("does not exist"));
    }

    #[test]
    fn selection_wraps_up_and_down() {
        let mut state = BrowserState {
            files: vec![PathBuf::from("a"), PathBuf::from("b")],
            ..Default::default()
        };
        state.list_state.select(Some(0));
        let mut editing = false;

        state.handle_key(key(KeyCode::Down), &mut editing);
        assert_eq!(state.list_state.selected(), Some(1));
        state.handle_key(key(KeyCode::Down), &mut editing);
        assert_eq!(state.list_state.selected(), Some(0), "Down must wrap");
        state.handle_key(key(KeyCode::Up), &mut editing);
        assert_eq!(state.list_state.selected(), Some(1), "Up must wrap");
    }

    #[test]
    fn navigation_on_empty_file_list_does_not_panic() {
        let mut state = BrowserState::default();
        let mut editing = false;
        state.handle_key(key(KeyCode::Down), &mut editing);
        state.handle_key(key(KeyCode::Up), &mut editing);
        assert_eq!(state.list_state.selected(), None);
    }

    #[test]
    fn draw_does_not_panic_and_renders_status() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let mut state = BrowserState {
            status: "3 file(s) — filter: all".to_string(),
            ..Default::default()
        };
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|f| draw(f, f.area(), &mut state, false))
            .expect("draw must not panic");
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("3 file(s)"));
    }
}
