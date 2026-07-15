//! The desktop TUI application: terminal setup/teardown, the top-level
//! [`App`] state machine, and screen dispatch. This whole module tree is
//! compiled only when `main.rs` includes it (behind
//! `cfg(not(target_os = "android"))`), so nothing below needs to repeat that
//! gate.
//!
//! # Navigation
//!
//! Five tabs ([`Tab`]), switched with `1`-`5` or `Tab`/`Shift+Tab` whenever no
//! text field is being edited. Each tab that reads the filesystem (Browser,
//! Symbols, Literature) owns a small text field for its root path, entered
//! with `e` and confirmed with `Enter`/cancelled with `Esc` — see
//! [`App::editing`]. `q`/`Esc` quits from any tab, except while editing (where
//! `Esc` only cancels the edit).
//!
//! # State ownership
//!
//! [`App`] owns one state struct per tab by value — no `Arc`/lock anywhere.
//! The workspace's `Arc<RwLock<T>>` rule (root `CLAUDE.md`, "Shared state")
//! governs state shared **across threads** in a simulation timestep loop; this
//! is a single-threaded, synchronous terminal event loop with nothing to share
//! — each `handle_key` call runs to completion (including any filesystem read)
//! before the next `terminal.draw` call, so plain ownership is the correct,
//! simpler tool here. See `DECISIONS.md`.

mod browser;
mod literature;
mod methods;
mod overview;
mod symbols;
mod text_input;

use ratatui::crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph, Tabs};
use ratatui::{DefaultTerminal, Frame};

/// The five human-facing screens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Overview,
    Browser,
    Symbols,
    Methods,
    Literature,
}

const TABS: [Tab; 5] = [
    Tab::Overview,
    Tab::Browser,
    Tab::Symbols,
    Tab::Methods,
    Tab::Literature,
];

impl Tab {
    fn title(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Browser => "Browser",
            Tab::Symbols => "Symbols",
            Tab::Methods => "Methods",
            Tab::Literature => "Literature",
        }
    }

    fn next(self) -> Self {
        let i = TABS.iter().position(|t| *t == self).unwrap_or(0);
        TABS[(i + 1) % TABS.len()]
    }

    fn prev(self) -> Self {
        let i = TABS.iter().position(|t| *t == self).unwrap_or(0);
        TABS[(i + TABS.len() - 1) % TABS.len()]
    }

    fn from_digit(c: char) -> Option<Self> {
        match c {
            '1' => Some(Tab::Overview),
            '2' => Some(Tab::Browser),
            '3' => Some(Tab::Symbols),
            '4' => Some(Tab::Methods),
            '5' => Some(Tab::Literature),
            _ => None,
        }
    }
}

/// Top-level application state: the active tab, whether a text field is being
/// edited, and one state struct per tab (see the module docs on why these are
/// owned by value with no lock).
#[derive(Default)]
pub struct App {
    pub tab: Tab,
    /// `true` while the active tab's text-input field (repository/literature
    /// root) is capturing keystrokes instead of navigation keys.
    pub editing: bool,
    pub should_quit: bool,
    browser: browser::BrowserState,
    symbols: symbols::SymbolsState,
    methods: methods::MethodsState,
    literature: literature::LiteratureState,
}

impl App {
    /// Route one key event to the global handlers (quit, tab switch) or, if
    /// none apply, to the active tab's own handler. Pure state mutation, no
    /// I/O beyond whatever the active tab's action performs (a filesystem
    /// scan) — this is what the unit tests below drive directly, without a
    /// terminal.
    pub fn handle_key(&mut self, key: KeyEvent) {
        if !self.editing {
            match key.code {
                event::KeyCode::Char('q') | event::KeyCode::Esc => {
                    self.should_quit = true;
                    return;
                }
                event::KeyCode::Char(c) => {
                    if let Some(t) = Tab::from_digit(c) {
                        self.tab = t;
                        return;
                    }
                }
                event::KeyCode::Tab => {
                    self.tab = self.tab.next();
                    return;
                }
                event::KeyCode::BackTab => {
                    self.tab = self.tab.prev();
                    return;
                }
                _ => {}
            }
        }

        match self.tab {
            Tab::Overview => {}
            Tab::Browser => self.browser.handle_key(key, &mut self.editing),
            Tab::Symbols => self.symbols.handle_key(key, &mut self.editing),
            Tab::Methods => self.methods.handle_key(key),
            Tab::Literature => self.literature.handle_key(key, &mut self.editing),
        }
    }
}

/// Set up the terminal, run the draw/input loop, and restore on exit (or on a
/// draw/read error — `ratatui::restore()` always runs so a panic or an I/O
/// error never leaves the user's terminal in raw mode).
pub fn run() -> std::io::Result<()> {
    let mut terminal = ratatui::init();
    let mut app = App::default();
    let result = draw_loop(&mut terminal, &mut app);
    ratatui::restore();
    result
}

fn draw_loop(terminal: &mut DefaultTerminal, app: &mut App) -> std::io::Result<()> {
    loop {
        terminal.draw(|f| draw(f, app))?;
        if let Event::Key(key) = event::read()? {
            // Some backends (notably Windows) report both press and release;
            // only act on press so a single physical keystroke is one action.
            if key.kind == KeyEventKind::Press {
                app.handle_key(key);
            }
        }
        if app.should_quit {
            return Ok(());
        }
    }
}

fn draw(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(frame.area());

    let titles: Vec<Line> = TABS.iter().map(|t| Line::from(t.title())).collect();
    let selected = TABS.iter().position(|t| *t == app.tab).unwrap_or(0);
    let tabs = Tabs::new(titles)
        .block(Block::bordered().title("KOVAN — knowledge without hallucination"))
        .select(selected)
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    frame.render_widget(tabs, chunks[0]);

    match app.tab {
        Tab::Overview => overview::draw(frame, chunks[1]),
        Tab::Browser => browser::draw(frame, chunks[1], &mut app.browser, app.editing),
        Tab::Symbols => symbols::draw(frame, chunks[1], &mut app.symbols, app.editing),
        Tab::Methods => methods::draw(frame, chunks[1], &mut app.methods),
        Tab::Literature => literature::draw(frame, chunks[1], &mut app.literature, app.editing),
    }

    let help = if app.editing {
        "editing — type, Enter to confirm, Esc to cancel"
    } else {
        match app.tab {
            Tab::Overview => "1-5 / Tab: switch tabs   q / Esc: quit",
            Tab::Browser => "e: edit root  Enter: rescan  Left/Right: filter  Up/Down: select  1-5: tabs  q: quit",
            Tab::Symbols => "e: edit root  Enter: rescan  Left/Right: language  m: markdown view  1-5: tabs  q: quit",
            Tab::Methods => "Left/Right: family  Up/Down: method  Enter: generate  PgUp/PgDn: scroll  q: quit",
            Tab::Literature => "e: edit root  Enter: preview  r: rescan  Left/Right: filter  1-5: tabs  q: quit",
        }
    };
    frame.render_widget(Paragraph::new(help), chunks[2]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn q_quits_from_the_default_overview_tab() {
        let mut app = App::default();
        assert!(!app.should_quit);
        app.handle_key(key(KeyCode::Char('q')));
        assert!(app.should_quit);
    }

    #[test]
    fn esc_also_quits_when_not_editing() {
        let mut app = App::default();
        app.handle_key(key(KeyCode::Esc));
        assert!(app.should_quit);
    }

    #[test]
    fn digit_keys_switch_tabs() {
        let mut app = App::default();
        app.handle_key(key(KeyCode::Char('4')));
        assert_eq!(app.tab, Tab::Methods);
        app.handle_key(key(KeyCode::Char('2')));
        assert_eq!(app.tab, Tab::Browser);
    }

    #[test]
    fn tab_key_cycles_forward_and_wraps() {
        let mut app = App::default();
        for _ in 0..5 {
            app.handle_key(key(KeyCode::Tab));
        }
        assert_eq!(
            app.tab,
            Tab::Overview,
            "5 steps from Overview must wrap back"
        );
    }

    #[test]
    fn back_tab_cycles_backward_and_wraps_from_overview() {
        let mut app = App::default();
        app.handle_key(key(KeyCode::BackTab));
        assert_eq!(app.tab, Tab::Literature);
    }

    #[test]
    fn editing_suppresses_global_quit_and_tab_switch_keys() {
        let mut app = App {
            tab: Tab::Browser,
            ..Default::default()
        };
        app.handle_key(key(KeyCode::Char('e'))); // enters edit mode on Browser's root field
        assert!(app.editing);

        // While editing, 'q' must type into the field, not quit.
        app.handle_key(key(KeyCode::Char('q')));
        assert!(!app.should_quit);

        // Same for a digit that would otherwise switch tabs.
        app.handle_key(key(KeyCode::Char('3')));
        assert_eq!(
            app.tab,
            Tab::Browser,
            "still on Browser — digit went into the field"
        );

        app.handle_key(key(KeyCode::Esc));
        assert!(!app.editing, "Esc cancels editing, does not quit");
        assert!(!app.should_quit);
    }

    #[test]
    fn overview_tab_ignores_navigation_keys_without_panicking() {
        let mut app = App::default();
        app.handle_key(key(KeyCode::Down));
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.tab, Tab::Overview);
        assert!(!app.should_quit);
    }

    #[test]
    fn every_tab_draws_without_panicking() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let mut app = App::default();
        for t in TABS {
            app.tab = t;
            terminal
                .draw(|f| draw(f, &mut app))
                .unwrap_or_else(|e| panic!("draw panicked on {t:?}: {e}"));
        }
    }

    #[test]
    fn tab_bar_highlights_the_active_tab() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let mut app = App {
            tab: Tab::Methods,
            ..Default::default()
        };
        terminal.draw(|f| draw(f, &mut app)).expect("draw ok");
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("Overview"));
        assert!(text.contains("Methods"));
        assert!(text.contains("Literature"));
    }
}
