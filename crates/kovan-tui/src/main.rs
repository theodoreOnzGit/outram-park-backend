//! # kovan-tui
//!
//! The **human-facing** entry point to KOVAN: a terminal UI for browsing
//! literature, repositories, and generated knowledge. Agents should use the
//! `kovan` CLI instead.
//!
//! Built on [`ratatui`]. TUI is desktop scope — on Android the binary compiles
//! to a stub that redirects to the CLI, keeping the workspace Android-buildable.
//!
//! Placeholder stage: renders a static overview screen listing the KOVAN
//! modules and quits on `q`/`Esc`. The real browser panes are TODO(kovan).

#[cfg(not(target_os = "android"))]
fn main() -> std::io::Result<()> {
    tui::run()
}

/// On Android there is no TUI; point the user at the agent-facing CLI.
#[cfg(target_os = "android")]
fn main() {
    eprintln!("kovan-tui is desktop-only. Use the `kovan` CLI on Android/Termux.");
}

#[cfg(not(target_os = "android"))]
mod tui {
    use ratatui::crossterm::event::{self, Event, KeyCode};
    use ratatui::layout::{Constraint, Layout};
    use ratatui::style::{Modifier, Style};
    use ratatui::widgets::{Block, List, ListItem, Paragraph};
    use ratatui::{DefaultTerminal, Frame};

    /// Set up the terminal, run the draw loop, and restore on exit.
    pub fn run() -> std::io::Result<()> {
        let mut terminal = ratatui::init();
        let result = draw_loop(&mut terminal);
        ratatui::restore();
        result
    }

    fn draw_loop(terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        loop {
            terminal.draw(ui)?;
            if let Event::Key(key) = event::read()? {
                if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
                    return Ok(());
                }
            }
        }
    }

    fn ui(frame: &mut Frame) {
        let chunks = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(frame.area());

        frame.render_widget(
            Paragraph::new("Knowledge without hallucination")
                .block(Block::bordered().title("KOVAN")),
            chunks[0],
        );

        let modules = [
            "kovan-common      — canonical types (source of truth)",
            "kovan-discovery   — fd + ripgrep file discovery / search",
            "kovan-literature  — PDF → Markdown → KovanDocument → BibTeX",
            "kovan-semantics   — ripgrep-first, then language servers",
            "kovan-codegen     — deterministic numerical-method templates",
        ];
        let items: Vec<ListItem> = modules.iter().map(|m| ListItem::new(*m)).collect();
        frame.render_widget(
            List::new(items)
                .block(Block::bordered().title("Modules"))
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED)),
            chunks[1],
        );

        frame.render_widget(
            Paragraph::new("placeholder UI — press q / Esc to quit"),
            chunks[2],
        );
    }
}
