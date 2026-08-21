//! The desktop TUI application: terminal setup/teardown, the top-level
//! [`App`] state machine, and screen dispatch. This whole module tree is
//! compiled only when `main.rs` includes it (behind
//! `cfg(not(target_os = "android"))`), so nothing below needs to repeat that
//! gate.
//!
//! # Navigation
//!
//! Six tabs ([`Tab`]), switched with `1`-`6` or `Tab`/`Shift+Tab` whenever no
//! text field is being edited. Each tab that reads the filesystem (Browser,
//! Symbols, Literature, Ingest) owns a small text field for its root path,
//! entered with `e` and confirmed with `Enter`/cancelled with `Esc` — see
//! [`App::editing`]. `q`/`Esc` quits from any tab, except while editing (where
//! `Esc` only cancels the edit) and except when the Ingest tab has work in
//! flight (see [`App::handle_key`]).
//!
//! # State ownership
//!
//! [`App`] owns one state struct per tab by value — no `Arc`/lock anywhere.
//! The workspace's `Arc<RwLock<T>>` rule (root `CLAUDE.md`, "Shared state")
//! governs state shared **across threads** in a simulation timestep loop. The
//! draw loop itself is single-threaded, and the one background worker (PDF
//! extraction, [`ingest`]) shares no state at all: it owns its input, sends one
//! result down an `mpsc` channel, and exits. So plain ownership remains the
//! correct, simpler tool here. See `DECISIONS.md`.
//!
//! # The loop is polled, not blocking
//!
//! [`draw_loop`] waits on input with `event::poll` and calls [`App::tick`] each
//! time round, so a running extraction can animate and deliver its result while
//! the user does nothing. The poll interval is short only while work is in
//! flight; otherwise it is long, so an idle TUI stays effectively asleep.

mod browser;
mod ingest;
mod literature;
mod methods;
mod overview;
mod symbols;
mod text_input;

use std::time::Duration;

use ratatui::crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph, Tabs};
use ratatui::{DefaultTerminal, Frame};

/// The six human-facing screens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Overview,
    Browser,
    Symbols,
    Methods,
    Literature,
    /// Interactive literature ingestion — the only screen that writes files.
    Ingest,
}

const TABS: [Tab; 6] = [
    Tab::Overview,
    Tab::Browser,
    Tab::Symbols,
    Tab::Methods,
    Tab::Literature,
    Tab::Ingest,
];

impl Tab {
    fn title(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Browser => "Browser",
            Tab::Symbols => "Symbols",
            Tab::Methods => "Methods",
            Tab::Literature => "Literature",
            Tab::Ingest => "Ingest",
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
            '6' => Some(Tab::Ingest),
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
    ingest: ingest::IngestState,
}

impl App {
    /// Route one key event to the global handlers (quit, tab switch) or, if
    /// none apply, to the active tab's own handler. Pure state mutation, no
    /// I/O beyond whatever the active tab's action performs (a filesystem
    /// scan, or writing the reviewed document on the Ingest tab) — this is what
    /// the unit tests below drive directly, without a terminal.
    ///
    /// One exception to the global bindings: `q`/`Esc` do **not** quit while the
    /// Ingest tab has an extraction running or an unsaved review on screen
    /// (`IngestState::blocks_quit`). A reflexive `q` there would throw away
    /// hand-corrected metadata; the user must press `x` to discard first.
    pub fn handle_key(&mut self, key: KeyEvent) {
        if !self.editing {
            match key.code {
                event::KeyCode::Char('q') | event::KeyCode::Esc => {
                    if self.tab == Tab::Ingest && self.ingest.blocks_quit() {
                        self.ingest.note_blocked_quit();
                        return;
                    }
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
            Tab::Literature => {
                self.literature.handle_key(key, &mut self.editing);
                // The Literature tab's `i` hands a selected PDF over to the
                // Ingest tab rather than importing in place, so the read-only
                // viewer stays read-only.
                if let Some(pdf) = self.literature.take_ingest_request() {
                    self.ingest.ingest_path(pdf);
                    self.tab = Tab::Ingest;
                }
            }
            Tab::Ingest => self.ingest.handle_key(key, &mut self.editing),
        }
    }

    /// Advance any background work by one draw-loop iteration.
    ///
    /// Currently only the Ingest tab has any: it polls its worker thread and
    /// advances the progress spinner. Returns `true` when the screen must be
    /// fully repainted (a phase change; see `IngestState::tick`).
    pub fn tick(&mut self) -> bool {
        self.ingest.tick()
    }

    /// How long the draw loop should wait for a key before looping again —
    /// short while background work is in flight so progress stays live, long
    /// when idle so the process is effectively asleep.
    fn poll_interval(&self) -> Duration {
        if self.ingest.is_busy() {
            Duration::from_millis(100)
        } else {
            Duration::from_millis(1000)
        }
    }
}

/// Set up the terminal, run the draw/input loop, and restore on exit (or on a
/// draw/read error — `ratatui::restore()` always runs, and `ratatui::init()`
/// installs a panic hook that restores first, so neither a panic nor an I/O
/// error can leave the user's terminal in raw mode).
pub fn run() -> std::io::Result<()> {
    let mut terminal = ratatui::init();
    let mut app = App::default();
    let result = draw_loop(&mut terminal, &mut app);
    ratatui::restore();
    result
}

/// Draw, wait briefly for input, tick background work, repeat.
///
/// The wait is `event::poll` rather than a blocking `event::read` so a running
/// PDF extraction can keep its elapsed-time display current and deliver its
/// result without the user touching the keyboard.
fn draw_loop(terminal: &mut DefaultTerminal, app: &mut App) -> std::io::Result<()> {
    loop {
        terminal.draw(|f| draw(f, app))?;
        if event::poll(app.poll_interval())? {
            if let Event::Key(key) = event::read()? {
                // Some backends (notably Windows) report both press and release;
                // only act on press so a single physical keystroke is one action.
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key);
                }
            }
        }
        if app.tick() {
            // A worker that panicked will have printed through the default hook
            // and may have smeared the frame; repaint everything.
            terminal.clear()?;
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
        Tab::Ingest => ingest::draw(frame, chunks[1], &mut app.ingest, app.editing),
    }

    let help = if app.editing {
        "editing — type, Enter to confirm, Esc to cancel"
    } else {
        match app.tab {
            Tab::Overview => "1-6 / Tab: switch tabs   q / Esc: quit",
            Tab::Browser => "e: edit root  Enter: rescan  Left/Right: filter  Up/Down: select  1-6: tabs  q: quit",
            Tab::Symbols => "e: edit root  Enter: rescan  Left/Right: language  m: markdown view  1-6: tabs  q: quit",
            Tab::Methods => "Left/Right: family  Up/Down: method  Enter: generate  PgUp/PgDn: scroll  q: quit",
            Tab::Literature => "e: edit root  Enter: preview  i: import selected PDF  r: rescan  Left/Right: filter  q: quit",
            Tab::Ingest => app.ingest.help_line(),
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
        for _ in 0..TABS.len() {
            app.handle_key(key(KeyCode::Tab));
        }
        assert_eq!(
            app.tab,
            Tab::Overview,
            "one full cycle from Overview must wrap back"
        );
    }

    #[test]
    fn back_tab_cycles_backward_and_wraps_from_overview() {
        let mut app = App::default();
        app.handle_key(key(KeyCode::BackTab));
        assert_eq!(app.tab, Tab::Ingest, "Ingest is the last tab");
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
    fn digit_six_reaches_the_ingest_tab() {
        let mut app = App::default();
        app.handle_key(key(KeyCode::Char('6')));
        assert_eq!(app.tab, Tab::Ingest);
    }

    #[test]
    fn literature_i_hands_the_selected_pdf_to_the_ingest_tab() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("report.pdf"), b"not a real pdf").unwrap();

        let mut app = App {
            tab: Tab::Literature,
            ..Default::default()
        };
        app.literature.root.set(dir.path().to_str().unwrap());
        app.handle_key(key(KeyCode::Char('r'))); // scan
        app.handle_key(key(KeyCode::Char('i'))); // import the selected PDF

        assert_eq!(app.tab, Tab::Ingest, "the hand-off switches tabs");
        assert!(app.ingest.is_busy(), "extraction started on the worker");
        assert!(
            app.literature.take_ingest_request().is_none(),
            "the request must be drained, not left pending"
        );

        // Let the worker finish (the payload is not a real PDF, so it fails) —
        // otherwise it would outlive the temp directory.
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(20) && !app.tick() {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!app.ingest.is_busy(), "the job must have been collected");
    }

    #[test]
    fn quitting_is_blocked_while_an_import_is_in_flight() {
        let dir = tempfile::tempdir().expect("tempdir");
        let pdf = dir.path().join("report.pdf");
        std::fs::write(&pdf, b"not a real pdf").unwrap();

        let mut app = App {
            tab: Tab::Ingest,
            ..Default::default()
        };
        app.ingest.ingest_path(pdf);
        app.handle_key(key(KeyCode::Char('q')));
        assert!(!app.should_quit, "q must not discard a running import");

        app.handle_key(key(KeyCode::Char('x'))); // abandon
        app.handle_key(key(KeyCode::Char('q')));
        assert!(app.should_quit, "q quits once nothing is in flight");
    }

    #[test]
    fn poll_interval_is_short_only_while_work_is_in_flight() {
        let dir = tempfile::tempdir().expect("tempdir");
        let pdf = dir.path().join("report.pdf");
        std::fs::write(&pdf, b"not a real pdf").unwrap();

        let mut app = App::default();
        assert_eq!(app.poll_interval(), Duration::from_millis(1000));
        app.ingest.ingest_path(pdf);
        assert_eq!(app.poll_interval(), Duration::from_millis(100));

        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(20) && !app.tick() {
            std::thread::sleep(Duration::from_millis(5));
        }
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
