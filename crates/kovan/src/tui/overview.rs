//! Overview tab — the original static module map, kept as the landing screen.
//!
//! No state, no interaction beyond tab-switching: this is the "what is KOVAN"
//! screen a first-time user lands on.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

/// Render the Overview tab into `area`.
pub fn draw(frame: &mut Frame, area: Rect) {
    let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).split(area);

    frame.render_widget(
        Paragraph::new("Knowledge without hallucination")
            .block(Block::default().borders(Borders::ALL).title("KOVAN")),
        chunks[0],
    );

    let modules = [
        "kovan-common      — canonical types (source of truth)",
        "kovan-discovery   — fd + ripgrep file discovery / search        (Browser tab)",
        "kovan-literature  — PDF -> Markdown -> KovanDocument -> BibTeX  (Literature tab)",
        "kovan-semantics   — ripgrep-first, then language servers        (Symbols tab)",
        "kovan-codegen     — deterministic numerical-method templates    (Methods tab)",
    ];
    let items: Vec<ListItem> = modules.iter().map(|m| ListItem::new(*m)).collect();
    frame.render_widget(
        List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Modules"))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED)),
        chunks[1],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn rendered_text(width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|f| draw(f, f.area()))
            .expect("draw must not panic");
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    #[test]
    fn renders_the_tagline_and_every_module_name() {
        let text = rendered_text(100, 20);
        assert!(text.contains("Knowledge without hallucination"));
        assert!(text.contains("kovan-common"));
        assert!(text.contains("kovan-discovery"));
        assert!(text.contains("kovan-literature"));
        assert!(text.contains("kovan-semantics"));
        assert!(text.contains("kovan-codegen"));
    }
}
