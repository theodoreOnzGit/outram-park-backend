//! # njoy-tui
//!
//! A **mobile-first, touchscreen** ratatui terminal browser for
//! `njoy-outram-park-fork`'s nuclear data — a JANIS-like nuclide/cross-section
//! viewer built for a phone terminal (Termux) first, desktop second. See
//! `README.md` for the mobile-first design rationale, the touch/mouse
//! interaction model, and how to run this on Termux.
//!
//! ## Screens
//! 1. **Finder** ([`ui::finder`], op-ixe) — type-to-filter fuzzy search over
//!    the embedded 125-nuclide WMP CORE set ([`nuclides::NuclideCatalog`]),
//!    tap a row (or press Enter) to open it.
//! 2. **Plot** ([`ui::plot`], op-1lj + op-dzl) — log-log sigma(E) chart for
//!    the selected nuclide/MT channel, with a degC/K temperature toggle and
//!    tap-stepper feeding the analytic Doppler-broadened evaluation
//!    ([`xsdata::NuclideXs::micro`]).
//!
//! ## Why ratatui is a normal (non-Android-gated) dependency here
//! Unlike `kovan-tui`, which is desktop-scope and stubs out on Android, this
//! crate's whole point is a **touch-driven terminal app for Termux** — a
//! terminal library is fully in-scope for Android per the workspace
//! `CLAUDE.md` (only windowing GUI, e.g. `egui`/`eframe`, is out of scope).
//! `ratatui.workspace = true` is declared unconditionally in `Cargo.toml`.

mod app;
mod elements;
mod nuclides;
#[cfg(test)]
mod smoke_test;
mod temperature;
mod ui;
mod xsdata;

use std::io;
use std::time::Duration;

use ratatui::crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;

use app::{App, Screen};

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    // EnableMouseCapture is the touchscreen path: Termux maps a tap to a
    // crossterm MouseEventKind::Down, a two-finger scroll / trackpad wheel to
    // ScrollUp/ScrollDown, and a held-finger drag to a stream of
    // MouseEventKind::Drag events (see `ui::finder::handle_mouse`'s
    // drag-to-scroll code for how those are turned into a scroll delta).
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let mut app = App::new();

    loop {
        if let Screen::Plot(state) = &mut app.screen {
            state.refresh();
        }

        terminal.draw(|f| draw(f, &app))?;

        // A short poll timeout keeps the loop responsive to a resize/redraw
        // even with no input, without busy-spinning the CPU on a phone.
        if event::poll(Duration::from_millis(200))? {
            let size = terminal.size()?;
            let area = ratatui::layout::Rect {
                x: 0,
                y: 0,
                width: size.width,
                height: size.height,
            };
            match event::read()? {
                Event::Mouse(ev) => handle_mouse(&mut app, ev, area),
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    handle_key(&mut app, key, area)
                }
                _ => {}
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

fn draw(f: &mut ratatui::Frame, app: &App) {
    let area = f.area();
    match &app.screen {
        Screen::Finder(state) => ui::finder::render(f, area, &app.catalog, state),
        Screen::Plot(state) => ui::plot::render(f, area, state),
    }
}

fn handle_mouse(app: &mut App, ev: event::MouseEvent, area: ratatui::layout::Rect) {
    let mut navigate_to: Option<String> = None;
    let mut go_back = false;
    match &mut app.screen {
        Screen::Finder(state) => {
            navigate_to = ui::finder::handle_mouse(ev, area, &app.catalog, state);
        }
        Screen::Plot(state) => {
            if ui::plot::handle_mouse(ev, area, state) == ui::plot::PlotAction::Back {
                go_back = true;
            }
        }
    }
    if let Some(name) = navigate_to {
        app.open_nuclide(&name);
    }
    if go_back {
        app.back_to_finder();
    }
}

fn handle_key(app: &mut App, key: event::KeyEvent, area: ratatui::layout::Rect) {
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};

    // Ctrl+C always quits, from either screen. A bare 'q' is deliberately
    // NOT a quit key: on the finder screen the user is typing a filter
    // query, and 'q' is a perfectly normal character to search with (e.g.
    // any nuclide name containing 'q' as a subsequence).
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.should_quit = true;
        return;
    }

    let mut navigate_to: Option<String> = None;
    let mut go_back = false;
    match &mut app.screen {
        Screen::Finder(state) => {
            if key.code == KeyCode::Esc {
                if state.query.is_empty() {
                    app.should_quit = true;
                    return;
                }
                state.query.clear();
                state.selected = 0;
                state.scroll = 0;
            } else {
                navigate_to = ui::finder::handle_key(key, area, &app.catalog, state);
            }
        }
        Screen::Plot(state) => {
            if ui::plot::handle_key(key, state) == ui::plot::PlotAction::Back {
                go_back = true;
            }
        }
    }
    if let Some(name) = navigate_to {
        app.open_nuclide(&name);
    }
    if go_back {
        app.back_to_finder();
    }
}
