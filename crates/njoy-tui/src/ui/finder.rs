//! Fuzzy nuclide finder screen (op-ixe): type-to-filter search box over a
//! touch-scrollable result list. See [`crate::nuclides`] for the matcher and
//! [`super::finder_layout`] for the shared render/hit-test layout.

use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::FinderState;
use crate::nuclides::NuclideCatalog;

use super::{finder_layout, point_in};

/// Rows scrolled per mouse-wheel tick.
const SCROLL_STEP: usize = 3;

pub fn render(f: &mut Frame, area: Rect, catalog: &NuclideCatalog, state: &FinderState) {
    let layout = finder_layout(area);
    let results = catalog.search(&state.query);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" njoy-tui ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!("— {} nuclide(s)", results.len())),
        ])),
        layout.title,
    );

    let search_block = Block::default()
        .borders(Borders::ALL)
        .title(" find (tap a result, or type) ");
    f.render_widget(
        Paragraph::new(format!("{}\u{2588}", state.query)).block(search_block),
        layout.search,
    );

    let visible_rows = layout.list.height as usize;
    for i in 0..visible_rows {
        let idx = state.scroll + i;
        let row = Rect {
            x: layout.list.x,
            y: layout.list.y + i as u16,
            width: layout.list.width,
            height: 1,
        };
        let Some(entry) = results.get(idx) else {
            continue;
        };
        let style = if idx == state.selected {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        let text = format!(" {:<8} Z={:<3} A={:<3} ", entry.label, entry.z, entry.a);
        f.render_widget(Paragraph::new(text).style(style), row);
    }

    f.render_widget(
        Paragraph::new(" tap a row = open | wheel/drag = scroll | Ctrl+C = quit ")
            .style(Style::default().fg(Color::DarkGray)),
        layout.footer,
    );
}

/// Clamp `state.selected`/`state.scroll` to `results.len()` and `list`'s
/// visible height. Shared by both input handlers so keyboard and touch
/// navigation can never leave the cursor pointing past the filtered list
/// (e.g. right after a keystroke narrows the results).
fn clamp(state: &mut FinderState, list: Rect, results_len: usize) {
    if results_len == 0 {
        state.selected = 0;
        state.scroll = 0;
        return;
    }
    state.selected = state.selected.min(results_len - 1);
    let visible = list.height.max(1) as usize;
    if state.selected < state.scroll {
        state.scroll = state.selected;
    } else if state.selected >= state.scroll + visible {
        state.scroll = state.selected + 1 - visible;
    }
    state.scroll = state.scroll.min(results_len.saturating_sub(1));
}

/// Handle one mouse event. Returns `Some(name)` — the WMP name of the
/// nuclide to open — when a tap lands on a result row.
pub fn handle_mouse(
    ev: MouseEvent,
    area: Rect,
    catalog: &NuclideCatalog,
    state: &mut FinderState,
) -> Option<String> {
    let layout = finder_layout(area);
    let results = catalog.search(&state.query);

    match ev.kind {
        MouseEventKind::ScrollDown => {
            state.scroll = state.scroll.saturating_add(SCROLL_STEP);
            clamp(state, layout.list, results.len());
        }
        MouseEventKind::ScrollUp => {
            state.scroll = state.scroll.saturating_sub(SCROLL_STEP);
            clamp(state, layout.list, results.len());
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            // Touch-drag scrolling: Termux reports a drag as a stream of
            // Drag events at successive rows while the finger stays down.
            // Track the previous row in `last_drag_row` so each event's
            // vertical delta becomes a scroll step (drag down = content
            // moves up as on a phone, hence the subtraction sign flip).
            if point_in(layout.list, ev.column, ev.row) {
                if let Some(prev) = state.last_drag_row {
                    let delta = prev as i32 - ev.row as i32;
                    state.scroll = (state.scroll as i32 + delta).max(0) as usize;
                    clamp(state, layout.list, results.len());
                }
                state.last_drag_row = Some(ev.row);
            }
        }
        MouseEventKind::Down(MouseButton::Left) => {
            state.last_drag_row = Some(ev.row);
            if point_in(layout.list, ev.column, ev.row) {
                let idx = state.scroll + (ev.row - layout.list.y) as usize;
                if let Some(entry) = results.get(idx) {
                    state.selected = idx;
                    return Some(entry.name.clone());
                }
            }
        }
        MouseEventKind::Up(_) => {
            state.last_drag_row = None;
        }
        _ => {}
    }
    None
}

/// Handle one key event. Returns `Some(name)` when Enter confirms the
/// highlighted row.
pub fn handle_key(
    key: KeyEvent,
    area: Rect,
    catalog: &NuclideCatalog,
    state: &mut FinderState,
) -> Option<String> {
    let layout = finder_layout(area);
    match key.code {
        KeyCode::Char(c) => {
            state.query.push(c);
            clamp(state, layout.list, catalog.search(&state.query).len());
        }
        KeyCode::Backspace => {
            state.query.pop();
            clamp(state, layout.list, catalog.search(&state.query).len());
        }
        KeyCode::Down => {
            state.selected += 1;
            clamp(state, layout.list, catalog.search(&state.query).len());
        }
        KeyCode::Up => {
            state.selected = state.selected.saturating_sub(1);
            clamp(state, layout.list, catalog.search(&state.query).len());
        }
        KeyCode::Enter => {
            let results = catalog.search(&state.query);
            if let Some(entry) = results.get(state.selected) {
                return Some(entry.name.clone());
            }
        }
        _ => {}
    }
    None
}
