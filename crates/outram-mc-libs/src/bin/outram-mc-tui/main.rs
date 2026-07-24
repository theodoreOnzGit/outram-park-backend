//! # outram-mc-tui
//!
//! A mobile-first, touchscreen [`ratatui`] terminal UI over `outram-mc-libs`:
//! pick a preconfigured geometry/material (op-dyt), tune the run settings
//! (op-4h8), launch a k-eigenvalue transport run and watch it converge, then
//! inspect the tallied neutron spectrum overlaid with a cross section
//! (op-iom). See the crate README for the mobile-first/touch design and
//! Termux usage; see `crate::transport` for what "live" honestly means here.
//!
//! Run it: `cargo run -p outram-mc-libs --bin outram-mc-tui --release`.
//!
//! This binary lives **inside** the `outram-mc-libs` crate (a `[[bin]]` target),
//! rather than in a separate crate. `ratatui` is an unconditional dependency of
//! `outram-mc-libs`, so the binary always builds with a plain `cargo build` —
//! no feature flags. The module tree that used to be `outram-mc-tui`'s library
//! (`app`, `presets`, `settings`, `spectrum`, `transport`, `ui`) is declared
//! directly below as private modules of this binary.
//!
//! ## Android/Termux
//!
//! Unlike `kovan-tui` (which target-gates `ratatui` off Android and stubs to a
//! CLI redirect), this crate declares `ratatui.workspace = true`
//! unconditionally and runs as a real touch TUI on Android/Termux — a
//! terminal library is in-scope there per the workspace `CLAUDE.md` "Android
//! portability" rule. The one Android-specific detail is that
//! `outram-mc-libs`'s GPU compute (`wgpu`) is itself target-gated off Android
//! in *that* crate, so on Android `outram_mc_libs::gpu::probe()` always
//! returns `None` and every run transparently executes the CPU path — this
//! binary needs no `cfg(target_os = "android")` of its own to handle that; it
//! falls out of `outram-mc-libs`'s own Android-safe `probe()` contract.

// This binary's module tree (`app`, `presets`, `settings`, `spectrum`,
// `transport`, `ui`) was authored as the `outram-mc-tui` *library* surface, so
// a handful of items are public API that the binary's own event loop does not
// call directly — e.g. `GeometryPreset::blurb`/`next`, `RunSettings::n_generations`,
// `RunOutcome::{compute_requested, gpu_available}`, `RunHandle::reset`. They are
// retained verbatim through the crate→bin move (a behaviour-preserving refactor,
// not a rewrite); allow dead_code so the move doesn't force deleting documented,
// still-meaningful surface. Genuinely-new dead code should still be pruned.
#![allow(dead_code)]

use std::io;
use std::time::Duration;

use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

mod app;
mod presets;
mod settings;
mod spectrum;
mod transport;
mod ui;
#[cfg(test)]
mod smoke;

use crate::app::{App, Screen, UiAction};

/// Redraw/poll tick length. Short enough that the run screen's spinner and
/// live-reveal animation (see `app::App::on_tick`) feel responsive, long
/// enough not to burn CPU spinning on a phone.
const TICK: Duration = Duration::from_millis(80);

fn main() -> io::Result<()> {
    init_terminal()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let result = run(&mut terminal);
    restore_terminal();
    result
}

/// Enable raw mode, the alternate screen, and **mouse capture** — the last is
/// the op-omf requirement this binary adds on top of `ratatui::init()`'s
/// defaults (which do not enable mouse capture). Termux maps a touch tap to a
/// `MouseEventKind::Down(Left)` at the terminal cell under the finger and a
/// drag/flick to `Drag`/`Scroll*` events, all of which only arrive once mouse
/// capture is on.
fn init_terminal() -> io::Result<()> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    // Best-effort: restore the terminal even if we panic mid-frame, same
    // spirit as `ratatui`'s own panic hook (see `ratatui::init`), extended to
    // also disable mouse capture.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default_hook(info);
    }));
    Ok(())
}

/// Mirror-image of [`init_terminal`]: disable mouse capture, leave the
/// alternate screen, disable raw mode. Errors are swallowed — this only runs
/// on the way out (including from the panic hook), where there is no good way
/// to report a failure and no point aborting the shutdown partway.
fn restore_terminal() {
    let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
    let _ = disable_raw_mode();
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let mut app = App::new();
    let mut drag_last_row: Option<u16> = None;

    loop {
        app.on_tick();
        terminal.draw(|frame| ui::draw(frame, &mut app))?;

        if event::poll(TICK)? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if let Some(action) = map_key(&app, key.code) {
                        app.dispatch(action);
                    }
                }
                Event::Mouse(m) => match m.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        drag_last_row = Some(m.row);
                        app.tap(m.column, m.row);
                    }
                    MouseEventKind::Up(MouseButton::Left) => {
                        drag_last_row = None;
                    }
                    MouseEventKind::Drag(MouseButton::Left) => {
                        if let Some(last) = drag_last_row {
                            let delta = last as i32 - m.row as i32;
                            if delta != 0 {
                                app.scroll(m.column, m.row, delta);
                            }
                        }
                        drag_last_row = Some(m.row);
                    }
                    MouseEventKind::ScrollDown => app.scroll(m.column, m.row, 1),
                    MouseEventKind::ScrollUp => app.scroll(m.column, m.row, -1),
                    _ => {}
                },
                Event::Resize(_, _) => {}
                _ => {}
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

/// Keyboard is the **secondary** input path (op-omf) — every action here is
/// also reachable by tapping the corresponding on-screen button, so this is a
/// convenience for a desktop terminal, not a requirement for phone use.
fn map_key(app: &App, code: KeyCode) -> Option<UiAction> {
    if matches!(code, KeyCode::Char('q')) {
        return Some(UiAction::Quit);
    }
    match app.screen {
        Screen::GeometryPicker => match code {
            KeyCode::Char('1') => Some(UiAction::SelectPreset(presets::ALL_PRESETS[0])),
            KeyCode::Char('2') => Some(UiAction::SelectPreset(presets::ALL_PRESETS[1])),
            KeyCode::Char('3') => Some(UiAction::SelectPreset(presets::ALL_PRESETS[2])),
            KeyCode::Char('4') => Some(UiAction::SelectPreset(presets::ALL_PRESETS[3])),
            KeyCode::Char('g') => Some(UiAction::ToggleSphereVariant),
            KeyCode::Enter => Some(UiAction::GoToSettings),
            KeyCode::Down => Some(UiAction::ScrollBy(1)),
            KeyCode::Up => Some(UiAction::ScrollBy(-1)),
            KeyCode::Esc => Some(UiAction::Quit),
            _ => None,
        },
        Screen::Settings => match code {
            KeyCode::Esc | KeyCode::Backspace => Some(UiAction::GoToGeometryPicker),
            KeyCode::Char('c') => Some(UiAction::CycleCompute),
            KeyCode::Char('p') => Some(UiAction::BumpParticles(-1)),
            KeyCode::Char('P') => Some(UiAction::BumpParticles(1)),
            KeyCode::Char('i') => Some(UiAction::BumpInactive(-1)),
            KeyCode::Char('I') => Some(UiAction::BumpInactive(1)),
            KeyCode::Char('a') => Some(UiAction::BumpActive(-1)),
            KeyCode::Char('A') => Some(UiAction::BumpActive(1)),
            KeyCode::Char('s') => Some(UiAction::BumpSeed(-1)),
            KeyCode::Char('S') => Some(UiAction::BumpSeed(1)),
            KeyCode::Enter => Some(UiAction::StartRun),
            KeyCode::Down => Some(UiAction::ScrollBy(1)),
            KeyCode::Up => Some(UiAction::ScrollBy(-1)),
            _ => None,
        },
        Screen::Run => match code {
            KeyCode::Esc | KeyCode::Backspace => Some(UiAction::GoToSettings),
            KeyCode::Char('r') => Some(UiAction::StartRun),
            KeyCode::Char('v') => Some(UiAction::GoToSpectrum),
            _ => None,
        },
        Screen::Spectrum => match code {
            KeyCode::Esc | KeyCode::Backspace => Some(UiAction::GoToRun),
            _ => None,
        },
    }
}
