//! Symbol Catalogue tab — a human-facing view over `kovan_semantics`.
//!
//! Point at a repository root, cycle the [`LanguageAdapter`], and press Enter
//! to run the ripgrep-first catalogue
//! ([`kovan_semantics::catalogue_symbols_detailed`]). Press `m` to flip
//! between the raw symbol list and the generated `symbols.md` Markdown
//! artifact ([`kovan_semantics::symbols_markdown`]) — the exact text KOVAN
//! would write to disk, previewed here instead.

use std::path::PathBuf;

use kovan_common::KovanSymbol;
use kovan_semantics::{catalogue_symbols, symbols_markdown, LanguageAdapter};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use super::text_input::TextInput;

const ADAPTERS: [LanguageAdapter; 4] = [
    LanguageAdapter::Rust,
    LanguageAdapter::Cpp,
    LanguageAdapter::Python,
    LanguageAdapter::Fortran,
];

fn adapter_label(a: LanguageAdapter) -> &'static str {
    match a {
        LanguageAdapter::Rust => "rust",
        LanguageAdapter::Cpp => "cpp",
        LanguageAdapter::Python => "python",
        LanguageAdapter::Fortran => "fortran",
    }
}

fn adapter_step(a: LanguageAdapter, delta: i32) -> LanguageAdapter {
    let i = ADAPTERS.iter().position(|x| *x == a).unwrap_or(0) as i32;
    let n = ADAPTERS.len() as i32;
    let j = ((i + delta) % n + n) % n;
    ADAPTERS[j as usize]
}

/// State for the Symbol Catalogue tab.
pub struct SymbolsState {
    pub root: TextInput,
    pub adapter: LanguageAdapter,
    pub symbols: Vec<KovanSymbol>,
    pub list_state: ListState,
    pub status: String,
    pub markdown_view: bool,
    pub markdown_scroll: u16,
}

impl Default for SymbolsState {
    fn default() -> Self {
        Self {
            root: TextInput::new("."),
            adapter: LanguageAdapter::Rust,
            symbols: Vec::new(),
            list_state: ListState::default(),
            status: "press 'e' to edit root, Left/Right for language, Enter to scan".to_string(),
            markdown_view: false,
            markdown_scroll: 0,
        }
    }
}

impl SymbolsState {
    fn run_catalogue(&mut self) {
        let root = PathBuf::from(self.root.value());
        if !root.exists() {
            self.symbols.clear();
            self.list_state.select(None);
            self.status = format!("root does not exist: {}", root.display());
            return;
        }
        match catalogue_symbols(&root, self.adapter) {
            Ok(syms) => {
                self.status = format!(
                    "{} symbol(s) — language: {}",
                    syms.len(),
                    adapter_label(self.adapter)
                );
                self.symbols = syms;
                self.list_state.select(if self.symbols.is_empty() {
                    None
                } else {
                    Some(0)
                });
            }
            Err(e) => {
                self.symbols.clear();
                self.list_state.select(None);
                self.status = format!("scan error: {e}");
            }
        }
        self.markdown_scroll = 0;
    }

    fn select_next(&mut self) {
        if self.symbols.is_empty() {
            return;
        }
        let i = self
            .list_state
            .selected()
            .map(|i| (i + 1) % self.symbols.len())
            .unwrap_or(0);
        self.list_state.select(Some(i));
    }

    fn select_prev(&mut self) {
        if self.symbols.is_empty() {
            return;
        }
        let i = self
            .list_state
            .selected()
            .map(|i| {
                if i == 0 {
                    self.symbols.len() - 1
                } else {
                    i - 1
                }
            })
            .unwrap_or(0);
        self.list_state.select(Some(i));
    }

    /// Handle one key event. `editing` is the shared edit-mode flag owned by
    /// [`super::App`].
    pub fn handle_key(&mut self, key: KeyEvent, editing: &mut bool) {
        if *editing {
            match key.code {
                KeyCode::Enter => {
                    *editing = false;
                    self.run_catalogue();
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
            KeyCode::Enter => self.run_catalogue(),
            KeyCode::Left => self.adapter = adapter_step(self.adapter, -1),
            KeyCode::Right => self.adapter = adapter_step(self.adapter, 1),
            KeyCode::Char('m') => self.markdown_view = !self.markdown_view,
            KeyCode::Down if self.markdown_view => {
                self.markdown_scroll = self.markdown_scroll.saturating_add(1)
            }
            KeyCode::Up if self.markdown_view => {
                self.markdown_scroll = self.markdown_scroll.saturating_sub(1)
            }
            KeyCode::Down => self.select_next(),
            KeyCode::Up => self.select_prev(),
            _ => {}
        }
    }
}

/// Render the Symbols tab into `area`.
pub fn draw(frame: &mut Frame, area: Rect, state: &mut SymbolsState, editing: bool) {
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
            "Language: {}  (Left/Right)   'm': {}   {}",
            adapter_label(state.adapter),
            if state.markdown_view {
                "list view"
            } else {
                "markdown view"
            },
            state.status
        )),
        chunks[1],
    );

    if state.markdown_view {
        let repo_name = state.root.value().to_string();
        let md = symbols_markdown(&repo_name, &state.symbols);
        frame.render_widget(
            Paragraph::new(md)
                .wrap(Wrap { trim: false })
                .scroll((state.markdown_scroll, 0))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("symbols.md preview (Up/Down to scroll)"),
                ),
            chunks[2],
        );
    } else {
        let items: Vec<ListItem> = state
            .symbols
            .iter()
            .map(|s| {
                ListItem::new(format!(
                    "{:<10} {}  ({}:{})",
                    s.kind, s.qualified_name, s.file, s.line
                ))
            })
            .collect();
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Symbols"))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        frame.render_stateful_widget(list, chunks[2], &mut state.list_state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn write_fixture(dir: &std::path::Path) {
        std::fs::write(dir.join("lib.rs"), "pub fn alpha() {}\npub struct Beta;\n")
            .expect("write fixture");
    }

    #[test]
    fn edit_mode_types_into_root_and_enter_confirms_and_scans() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_fixture(dir.path());

        let mut state = SymbolsState::default();
        let mut editing = false;

        state.handle_key(key(KeyCode::Char('e')), &mut editing);
        assert!(editing);
        state.root.clear();
        for c in dir.path().to_str().expect("utf8 tempdir path").chars() {
            state.handle_key(key(KeyCode::Char(c)), &mut editing);
        }
        state.handle_key(key(KeyCode::Enter), &mut editing);

        assert!(!editing);
        assert!(state.symbols.iter().any(|s| s.qualified_name == "alpha"));
        assert!(state.symbols.iter().any(|s| s.qualified_name == "Beta"));
    }

    #[test]
    fn adapter_cycles_and_wraps_both_directions() {
        let mut state = SymbolsState::default();
        let mut editing = false;
        assert_eq!(state.adapter, LanguageAdapter::Rust);

        state.handle_key(key(KeyCode::Left), &mut editing);
        assert_eq!(state.adapter, LanguageAdapter::Fortran, "Left must wrap");

        state.handle_key(key(KeyCode::Right), &mut editing);
        assert_eq!(state.adapter, LanguageAdapter::Rust);
        state.handle_key(key(KeyCode::Right), &mut editing);
        assert_eq!(state.adapter, LanguageAdapter::Cpp);
    }

    #[test]
    fn m_toggles_markdown_view() {
        let mut state = SymbolsState::default();
        let mut editing = false;
        assert!(!state.markdown_view);
        state.handle_key(key(KeyCode::Char('m')), &mut editing);
        assert!(state.markdown_view);
        state.handle_key(key(KeyCode::Char('m')), &mut editing);
        assert!(!state.markdown_view);
    }

    #[test]
    fn markdown_view_scroll_does_not_move_the_list_selection() {
        let mut state = SymbolsState {
            markdown_view: true,
            ..Default::default()
        };
        let mut editing = false;
        state.handle_key(key(KeyCode::Down), &mut editing);
        assert_eq!(state.markdown_scroll, 1);
        assert_eq!(state.list_state.selected(), None);
    }

    #[test]
    fn nonexistent_root_reports_status_without_panicking() {
        let mut state = SymbolsState::default();
        state.root.set("/this/path/should/not/exist/kovan-tui-test");
        state.run_catalogue();
        assert!(state.symbols.is_empty());
        assert!(state.status.contains("does not exist"));
    }

    #[test]
    fn draw_list_view_and_markdown_view_do_not_panic() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let dir = tempfile::tempdir().expect("tempdir");
        write_fixture(dir.path());
        let mut state = SymbolsState::default();
        state.root.set(dir.path().to_str().expect("utf8 path"));
        state.run_catalogue();
        assert!(!state.symbols.is_empty());

        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");

        terminal
            .draw(|f| draw(f, f.area(), &mut state, false))
            .expect("list view draw must not panic");
        let list_text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(list_text.contains("alpha"));

        state.markdown_view = true;
        terminal
            .draw(|f| draw(f, f.area(), &mut state, false))
            .expect("markdown view draw must not panic");
        let md_text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(md_text.contains("Symbols"));
    }
}
