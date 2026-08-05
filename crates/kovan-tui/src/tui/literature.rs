//! Literature tab — a human-facing view over `kovan_literature`.
//!
//! Type a literature-crate root (defaults to `crates/kovan-literature`,
//! matching this workspace's layout when `kovan-tui` is run from the
//! repository root — `docs/kovan.md` § "Storage Layout"), press Enter to list
//! PDFs / Markdown / BibTeX under it, then press Enter again on a selected
//! entry to preview it:
//!
//! - `.pdf` — best-effort [`kovan_literature::extract_metadata`] fields
//!   (title, authors, year, DOI, …) plus the start of the generated Markdown
//!   body.
//! - `.md` / `.markdown` — the heading outline
//!   ([`kovan_literature::markdown_outline`]) followed by the raw text.
//! - `.bib` — the raw BibTeX text as written
//!   ([`kovan_literature::to_bibtex`] generates these files; this tab only
//!   reads them back).
//!
//! Like every other tab, this is a **read-only viewer** — it never writes
//! into the `open/`/`proprietary/`/`generated/` storage tree it browses.

use std::path::PathBuf;

use kovan_discovery::{discover, discover_kind, FileKind};
use kovan_literature::extract_metadata;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use super::text_input::TextInput;

/// Which entry kinds are listed. Unlike [`super::browser::KindFilter`] this is
/// scoped to the three file types the literature pipeline cares about, plus a
/// `.bib` kind that [`kovan_discovery::FileKind`] doesn't model (bibliography
/// files aren't one of that enum's general-purpose categories).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LitKind {
    All,
    Pdf,
    Markdown,
    Bib,
}

const KINDS: [LitKind; 4] = [LitKind::All, LitKind::Pdf, LitKind::Markdown, LitKind::Bib];

impl LitKind {
    fn label(self) -> &'static str {
        match self {
            LitKind::All => "all",
            LitKind::Pdf => "pdf",
            LitKind::Markdown => "markdown",
            LitKind::Bib => "bibtex",
        }
    }

    fn step(self, delta: i32) -> Self {
        let i = KINDS.iter().position(|k| *k == self).unwrap_or(0) as i32;
        let n = KINDS.len() as i32;
        let j = ((i + delta) % n + n) % n;
        KINDS[j as usize]
    }
}

/// State for the Literature tab.
pub struct LiteratureState {
    pub root: TextInput,
    pub kind: LitKind,
    pub entries: Vec<PathBuf>,
    pub list_state: ListState,
    pub status: String,
    pub preview: String,
    pub preview_scroll: u16,
    /// Set when the user presses `i` on a selected PDF: a request for
    /// [`super::App`] to hand this file to the Ingest tab. Drained by
    /// [`LiteratureState::take_ingest_request`] — this tab never imports
    /// anything itself, keeping it a read-only viewer.
    pub ingest_request: Option<PathBuf>,
}

impl Default for LiteratureState {
    fn default() -> Self {
        Self {
            root: TextInput::new("crates/kovan-literature"),
            kind: LitKind::All,
            entries: Vec::new(),
            list_state: ListState::default(),
            status: "press 'e' to edit root, Enter to scan; Enter again to preview, 'i' to import"
                .to_string(),
            preview: String::new(),
            preview_scroll: 0,
            ingest_request: None,
        }
    }
}

impl LiteratureState {
    fn run_scan(&mut self) {
        let root = PathBuf::from(self.root.value());
        if !root.exists() {
            self.entries.clear();
            self.list_state.select(None);
            self.status = format!("root does not exist: {}", root.display());
            return;
        }
        let pdfs = discover_kind(&root, FileKind::Pdf);
        let mds = discover_kind(&root, FileKind::Markdown);
        let bibs = discover(&root, &["bib"]);
        self.entries = match self.kind {
            LitKind::All => {
                let mut v = Vec::new();
                v.extend(pdfs);
                v.extend(mds);
                v.extend(bibs);
                v.sort();
                v
            }
            LitKind::Pdf => pdfs,
            LitKind::Markdown => mds,
            LitKind::Bib => bibs,
        };
        self.status = format!(
            "{} entries — filter: {}",
            self.entries.len(),
            self.kind.label()
        );
        self.list_state.select(if self.entries.is_empty() {
            None
        } else {
            Some(0)
        });
        self.preview.clear();
    }

    fn select_next(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        let i = self
            .list_state
            .selected()
            .map(|i| (i + 1) % self.entries.len())
            .unwrap_or(0);
        self.list_state.select(Some(i));
    }

    fn select_prev(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        let i = self
            .list_state
            .selected()
            .map(|i| {
                if i == 0 {
                    self.entries.len() - 1
                } else {
                    i - 1
                }
            })
            .unwrap_or(0);
        self.list_state.select(Some(i));
    }

    fn preview_selected(&mut self) {
        let Some(i) = self.list_state.selected() else {
            self.preview = "no entry selected".to_string();
            return;
        };
        let Some(path) = self.entries.get(i).cloned() else {
            return;
        };
        self.preview_scroll = 0;
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        self.preview = match ext.as_str() {
            "pdf" => match extract_metadata(&path) {
                Ok(doc) => format_document_preview(&doc),
                Err(e) => format!("metadata extraction failed: {e}"),
            },
            "md" | "markdown" => match std::fs::read_to_string(&path) {
                Ok(text) => {
                    let headings = kovan_literature::markdown_outline(&text);
                    format_markdown_preview(&headings, &text)
                }
                Err(e) => format!("could not read file: {e}"),
            },
            "bib" => std::fs::read_to_string(&path)
                .unwrap_or_else(|e| format!("could not read file: {e}")),
            _ => "no preview available for this file type".to_string(),
        };
    }

    /// Ask for the selected entry to be imported on the Ingest tab.
    ///
    /// Only PDFs can be ingested; anything else sets an explanatory status and
    /// leaves the request unset.
    fn request_ingest(&mut self) {
        let Some(path) = self
            .list_state
            .selected()
            .and_then(|i| self.entries.get(i))
            .cloned()
        else {
            self.status = "no entry selected".to_string();
            return;
        };
        let is_pdf = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("pdf"));
        if is_pdf {
            self.status = format!("importing {} on the Ingest tab", path.display());
            self.ingest_request = Some(path);
        } else {
            self.status = "only PDFs can be imported — select a .pdf entry".to_string();
        }
    }

    /// Take a pending ingest request, if any, leaving `None` behind. Called by
    /// [`super::App`] right after dispatching a key to this tab.
    pub fn take_ingest_request(&mut self) -> Option<PathBuf> {
        self.ingest_request.take()
    }

    /// Handle one key event. `editing` is the shared edit-mode flag owned by
    /// [`super::App`]. Unlike the Browser/Symbols tabs, a plain (non-editing)
    /// Enter here previews the selected entry rather than re-running the
    /// scan — `r` re-scans instead, since "preview this file" is the more
    /// useful default action once a list is already on screen. `i` hands the
    /// selected PDF to the Ingest tab.
    pub fn handle_key(&mut self, key: KeyEvent, editing: &mut bool) {
        if *editing {
            match key.code {
                KeyCode::Enter => {
                    *editing = false;
                    self.run_scan();
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
            KeyCode::Char('r') => self.run_scan(),
            KeyCode::Char('i') => self.request_ingest(),
            KeyCode::Enter => self.preview_selected(),
            KeyCode::Left => self.kind = self.kind.step(-1),
            KeyCode::Right => self.kind = self.kind.step(1),
            KeyCode::Down => self.select_next(),
            KeyCode::Up => self.select_prev(),
            KeyCode::PageDown => self.preview_scroll = self.preview_scroll.saturating_add(10),
            KeyCode::PageUp => self.preview_scroll = self.preview_scroll.saturating_sub(10),
            _ => {}
        }
    }
}

fn format_document_preview(doc: &kovan_common::KovanDocument) -> String {
    let mut s = String::new();
    s.push_str(&format!("Title: {}\n", doc.title));
    if !doc.authors.is_empty() {
        let names: Vec<String> = doc
            .authors
            .iter()
            .map(|a| {
                if a.given.is_empty() {
                    a.family.clone()
                } else {
                    format!("{} {}", a.given, a.family)
                }
            })
            .collect();
        s.push_str(&format!("Authors: {}\n", names.join(", ")));
    }
    if let Some(y) = doc.year {
        s.push_str(&format!("Year: {y}\n"));
    }
    if let Some(doi) = &doc.doi {
        s.push_str(&format!("DOI: {doi}\n"));
    }
    if let Some(j) = &doc.journal {
        s.push_str(&format!("Journal: {j}\n"));
        // Journal locators only make sense alongside a journal name.
        if let Some(v) = &doc.volume {
            s.push_str(&format!("Volume: {v}\n"));
        }
        if let Some(n) = &doc.number {
            s.push_str(&format!("Number: {n}\n"));
        }
        if let Some(p) = &doc.pages {
            s.push_str(&format!("Pages: {p}\n"));
        }
    }
    if let Some(i) = &doc.institution {
        s.push_str(&format!("Institution: {i}\n"));
    }
    if let Some(pc) = doc.page_count {
        s.push_str(&format!("Page count: {pc}\n"));
    }
    s.push_str(&format!("Type: {:?}\n", doc.document_type));
    s.push_str(&format!("Visibility: {:?}\n", doc.visibility));
    if !doc.keywords.is_empty() {
        s.push_str(&format!("Keywords: {}\n", doc.keywords.join(", ")));
    }
    s.push('\n');
    s.push_str("--- Markdown body (first 2000 chars) ---\n");
    s.push_str(&doc.markdown_body.chars().take(2000).collect::<String>());
    s
}

fn format_markdown_preview(headings: &[kovan_literature::Heading], text: &str) -> String {
    let mut s = String::new();
    if headings.is_empty() {
        s.push_str("(no headings found)\n\n");
    } else {
        s.push_str("Outline:\n");
        for h in headings {
            s.push_str(&"  ".repeat(h.level.saturating_sub(1) as usize));
            s.push_str(&format!("- {}\n", h.text));
        }
        s.push('\n');
    }
    s.push_str("--- Content (first 4000 chars) ---\n");
    s.push_str(&text.chars().take(4000).collect::<String>());
    s
}

/// Render the Literature tab into `area`.
pub fn draw(frame: &mut Frame, area: Rect, state: &mut LiteratureState, editing: bool) {
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
                .title("Literature root ('e' to edit, Enter to scan)"),
        ),
        chunks[0],
    );

    frame.render_widget(
        Paragraph::new(format!(
            "Filter: {}  (Left/Right)   {}",
            state.kind.label(),
            state.status
        )),
        chunks[1],
    );

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(chunks[2]);

    let items: Vec<ListItem> = state
        .entries
        .iter()
        .map(|p| ListItem::new(p.display().to_string()))
        .collect();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Entries (Enter preview, 'i' import PDF, 'r' rescan)"),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    frame.render_stateful_widget(list, cols[0], &mut state.list_state);

    frame.render_widget(
        Paragraph::new(state.preview.as_str())
            .wrap(Wrap { trim: false })
            .scroll((state.preview_scroll, 0))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Preview (PgUp/PgDn to scroll)"),
            ),
        cols[1],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn fixture_tree() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("generated/markdown")).unwrap();
        std::fs::create_dir_all(dir.path().join("generated/bibtex")).unwrap();
        std::fs::write(
            dir.path().join("generated/markdown/doc.md"),
            "# Title\n\nSome body text.\n\n## Section\n\nMore text.\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("generated/bibtex/doc.bib"),
            "@article{doc2026,\n  title = {A Title},\n}\n",
        )
        .unwrap();
        dir
    }

    #[test]
    fn edit_mode_types_into_root_and_enter_confirms_and_scans() {
        let dir = fixture_tree();
        let mut state = LiteratureState::default();
        let mut editing = false;

        state.handle_key(key(KeyCode::Char('e')), &mut editing);
        assert!(editing);
        state.root.clear();
        for c in dir.path().to_str().expect("utf8 tempdir path").chars() {
            state.handle_key(key(KeyCode::Char(c)), &mut editing);
        }
        state.handle_key(key(KeyCode::Enter), &mut editing);

        assert!(!editing);
        assert_eq!(
            state.entries.len(),
            2,
            "one .md and one .bib: {:?}",
            state.entries
        );
    }

    #[test]
    fn kind_filter_narrows_the_entry_list() {
        let dir = fixture_tree();
        let mut state = LiteratureState::default();
        state.root.set(dir.path().to_str().unwrap());
        state.kind = LitKind::Markdown;
        state.run_scan();
        assert_eq!(state.entries.len(), 1);
        assert!(state.entries[0].extension().unwrap() == "md");
    }

    #[test]
    fn preview_markdown_entry_shows_outline_and_content() {
        let dir = fixture_tree();
        let mut state = LiteratureState::default();
        state.root.set(dir.path().to_str().unwrap());
        state.kind = LitKind::Markdown;
        state.run_scan();
        let mut editing = false;
        state.handle_key(key(KeyCode::Enter), &mut editing);
        assert!(state.preview.contains("Title"));
        assert!(state.preview.contains("Section"));
        assert!(state.preview.contains("Some body text"));
    }

    #[test]
    fn preview_bib_entry_shows_raw_text() {
        let dir = fixture_tree();
        let mut state = LiteratureState::default();
        state.root.set(dir.path().to_str().unwrap());
        state.kind = LitKind::Bib;
        state.run_scan();
        let mut editing = false;
        state.handle_key(key(KeyCode::Enter), &mut editing);
        assert!(state.preview.contains("@article{doc2026"));
    }

    #[test]
    fn r_rescans_without_entering_edit_mode() {
        let dir = fixture_tree();
        let mut state = LiteratureState::default();
        state.root.set(dir.path().to_str().unwrap());
        let mut editing = false;
        state.handle_key(key(KeyCode::Char('r')), &mut editing);
        assert!(!editing);
        assert_eq!(state.entries.len(), 2);
    }

    #[test]
    fn nonexistent_root_reports_status_without_panicking() {
        let mut state = LiteratureState::default();
        state.root.set("/this/path/should/not/exist/kovan-tui-test");
        state.run_scan();
        assert!(state.entries.is_empty());
        assert!(state.status.contains("does not exist"));
    }

    #[test]
    fn draw_does_not_panic_on_an_empty_scan() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let mut state = LiteratureState::default();
        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|f| draw(f, f.area(), &mut state, false))
            .expect("draw must not panic");
    }
}
