//! Mobile-first, touch-primary layout + rendering (op-omf).
//!
//! # Design principles this module follows
//!
//! - **Single column, narrow-first.** Every screen is one vertical stack of
//!   full-width bands (title / controls / content / footer) — no side panels
//!   that would get crushed on a phone-width terminal. Layout functions here
//!   take only a [`Rect`] and return where each control lives, so they adapt
//!   continuously as the terminal is resized (Termux users rotate their
//!   phone, resize the terminal app, change font size, ...).
//! - **Layout is a pure function of area, reused for hit-testing.** Every
//!   `*_layout` function below is called both by the renderer and by the
//!   mouse handler in [`crate::app`]. There is no separate "remembered
//!   rects from last frame" bookkeeping to go stale — a tap is resolved by
//!   recomputing the same deterministic layout the draw call just used.
//! - **Touch is primary, keyboard is secondary.** Every control that can be
//!   operated by a key can also be tapped (see `handle_mouse` in
//!   [`finder`]/[`plot`]); the reverse is not required by op-omf but is
//!   provided anyway as a convenience for a physical/Termux soft keyboard.
//! - **Large tap targets.** Rows and buttons are at least one full terminal
//!   line tall and span most of the available width — small multi-target
//!   rows (e.g. the MT selector) drop from one row to two on a narrow
//!   screen (see [`mt_button_rects`]) rather than shrinking each button
//!   below a tappable size.

pub mod finder;
pub mod plot;

use ratatui::layout::Rect;

use crate::xsdata::MtChannel;

/// Whether terminal coordinates `(col, row)` fall inside `area`.
pub fn point_in(area: Rect, col: u16, row: u16) -> bool {
    col >= area.x && col < area.x + area.width && row >= area.y && row < area.y + area.height
}

/// Screen bands for the finder (op-ixe): title, search box, scrollable
/// result list, footer hint. See the module docs — this exact function is
/// called by both the renderer and the mouse hit-tester.
pub struct FinderLayout {
    pub title: Rect,
    pub search: Rect,
    pub list: Rect,
    pub footer: Rect,
}

pub fn finder_layout(area: Rect) -> FinderLayout {
    use ratatui::layout::{Constraint, Direction, Layout};
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title
            Constraint::Length(3), // search box (bordered)
            Constraint::Min(1),    // result list (fills remaining space)
            Constraint::Length(1), // footer hint
        ])
        .split(area);
    FinderLayout {
        title: chunks[0],
        search: chunks[1],
        list: chunks[2],
        footer: chunks[3],
    }
}

/// Screen bands for the plot view (op-1lj/op-dzl): title/back, unit toggle,
/// temperature stepper row, MT selector, the log-log chart, footer hint.
pub struct PlotLayout {
    pub title: Rect,
    pub unit_toggle: Rect,
    pub temp_row: Rect,
    pub mt_row: Rect,
    pub chart: Rect,
    pub footer: Rect,
}

/// Below this width the MT selector wraps from one row of 4 to two rows of
/// 2 (see [`mt_button_rects`]) so each button keeps a tappable width.
pub const MT_WIDE_MIN_WIDTH: u16 = 40;

pub fn plot_layout(area: Rect) -> PlotLayout {
    use ratatui::layout::{Constraint, Direction, Layout};
    let mt_rows: u16 = if area.width >= MT_WIDE_MIN_WIDTH {
        1
    } else {
        2
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),       // title / back button
            Constraint::Length(1),       // unit toggle
            Constraint::Length(1),       // temperature stepper
            Constraint::Length(mt_rows), // MT selector (1 or 2 rows)
            Constraint::Min(4),          // chart
            Constraint::Length(1),       // footer hint
        ])
        .split(area);
    PlotLayout {
        title: chunks[0],
        unit_toggle: chunks[1],
        temp_row: chunks[2],
        mt_row: chunks[3],
        chart: chunks[4],
        footer: chunks[5],
    }
}

/// Width of the "< back" tap target at the left of the title row.
pub const BACK_BUTTON_WIDTH: u16 = 8;

pub fn back_button_rect(title: Rect) -> Rect {
    Rect {
        x: title.x,
        y: title.y,
        width: title.width.min(BACK_BUTTON_WIDTH),
        height: 1,
    }
}

/// The five temperature-stepper tap targets within `temp_row`, evenly split:
/// `[-10, -1, <value display>, +1, +10]`. The middle cell is a no-op tap
/// target (exact entry stays a keyboard affordance — see [`plot`] docs); the
/// other four return the delta (in the currently displayed unit) tapping
/// them applies.
pub fn temp_button_rects(temp_row: Rect) -> [(f64, Rect); 5] {
    use ratatui::layout::{Constraint, Direction, Layout};
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 5); 5])
        .split(temp_row);
    [
        (-10.0, cols[0]),
        (-1.0, cols[1]),
        (0.0, cols[2]),
        (1.0, cols[3]),
        (10.0, cols[4]),
    ]
}

/// The MT-channel tap targets within `mt_row`, one per [`MtChannel::ALL`].
/// Lays out as a single row of 4 when `mt_row` is wide enough
/// ([`MT_WIDE_MIN_WIDTH`]), otherwise as 2 rows of 2 — the narrow-screen
/// adaptation `op-omf` requires instead of shrinking buttons past a tappable
/// width.
pub fn mt_button_rects(mt_row: Rect) -> Vec<(MtChannel, Rect)> {
    use ratatui::layout::{Constraint, Direction, Layout};
    let channels = MtChannel::ALL;
    if mt_row.width >= MT_WIDE_MIN_WIDTH {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Ratio(1, 4); 4])
            .split(mt_row);
        channels.into_iter().zip(cols.iter().copied()).collect()
    } else {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(mt_row);
        let mut out = Vec::with_capacity(4);
        for (r, pair) in rows.iter().zip(channels.chunks(2)) {
            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Ratio(1, 2); 2])
                .split(*r);
            for (ch, c) in pair.iter().zip(cols.iter().copied()) {
                out.push((*ch, c));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mt_buttons_cover_all_four_channels_wide_and_narrow() {
        for width in [20u16, 40, 80] {
            let area = Rect {
                x: 0,
                y: 0,
                width,
                height: 2,
            };
            let rects = mt_button_rects(area);
            assert_eq!(rects.len(), 4, "width {width}");
            for (i, ch) in MtChannel::ALL.iter().enumerate() {
                assert_eq!(&rects[i].0, ch);
            }
        }
    }

    #[test]
    fn temp_buttons_span_the_row_without_overlap() {
        let area = Rect {
            x: 0,
            y: 0,
            width: 30,
            height: 1,
        };
        let rects = temp_button_rects(area);
        let mut x = area.x;
        for (_, r) in rects {
            assert_eq!(r.x, x);
            x += r.width;
        }
        assert_eq!(x, area.x + area.width);
    }

    #[test]
    fn point_in_respects_area_bounds() {
        let area = Rect {
            x: 5,
            y: 5,
            width: 10,
            height: 2,
        };
        assert!(point_in(area, 5, 5));
        assert!(point_in(area, 14, 6));
        assert!(!point_in(area, 15, 5));
        assert!(!point_in(area, 5, 7));
        assert!(!point_in(area, 4, 5));
    }
}
