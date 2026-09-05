//! Cross-section log-log plot screen (op-1lj) + temperature control (op-dzl).
//!
//! The chart itself is a [`ratatui::widgets::Chart`] with a braille
//! [`ratatui::symbols::Marker`] dataset — ratatui's `Chart` renders through
//! its `Canvas` internally, which is what the task brief's "Canvas/Chart
//! (braille)" is asking for; there is no need to hand-roll a second
//! braille-plotting path on top of it. Because `Chart`'s axes are linear,
//! the log-log look comes from plotting `log10(E)`/`log10(sigma)` directly
//! ([`crate::xsdata::sample_log_log`]) and reformatting the axis labels back
//! to `10^n`-style physical values for display.

use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, Paragraph};
use ratatui::Frame;

use crate::app::PlotState;
use crate::xsdata::MtChannel;

use super::{back_button_rect, mt_button_rects, plot_layout, point_in, temp_button_rects};

/// What a plot-screen input handler wants the caller ([`crate::app::App`])
/// to do next. Kept as an enum (not a bool) so a future action doesn't need
/// a signature change — matches the workspace's "enums over ad hoc bools"
/// spirit even for UI plumbing this small.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlotAction {
    None,
    Back,
}

/// Format a `log10`-space axis value back to a physical `10^n`-ish label,
/// e.g. `2.0 -> "1e2"`, matching how JANIS and similar tools label a log
/// cross-section axis.
fn log_label(log_value: f64) -> String {
    format!("{:.0e}", 10f64.powf(log_value))
}

pub fn render(f: &mut Frame, area: Rect, state: &PlotState) {
    let layout = plot_layout(area);

    let z = state.nuclide.name();
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " < back ",
                Style::default().add_modifier(Modifier::REVERSED),
            ),
            Span::raw(format!(" {z}  ({} sigma(E,T))", state.mt.mt_reaction())),
        ])),
        layout.title,
    );

    f.render_widget(
        Paragraph::new(format!(
            " unit: [{}]  (tap to switch {}<->{}) ",
            state.temp_unit.label(),
            crate::temperature::TempUnit::Celsius.label(),
            crate::temperature::TempUnit::Kelvin.label(),
        ))
        .style(Style::default().fg(Color::Cyan)),
        layout.unit_toggle,
    );

    let temp_rects = temp_button_rects(layout.temp_row);
    for (delta, rect) in temp_rects {
        let label = if delta == 0.0 {
            format!("{:.1} {}", state.temp_value, state.temp_unit.label())
        } else {
            format!("{delta:+.0}")
        };
        f.render_widget(
            Paragraph::new(label).alignment(ratatui::layout::Alignment::Center),
            rect,
        );
    }

    for (ch, rect) in mt_button_rects(layout.mt_row) {
        let style = if ch == state.mt {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };
        f.render_widget(
            Paragraph::new(ch.label())
                .alignment(ratatui::layout::Alignment::Center)
                .style(style),
            rect,
        );
    }

    render_chart(f, layout.chart, state);

    f.render_widget(
        Paragraph::new(" tap MT/unit/+-  |  arrows nudge T  |  Esc = back ")
            .style(Style::default().fg(Color::DarkGray)),
        layout.footer,
    );
}

fn render_chart(f: &mut Frame, area: Rect, state: &PlotState) {
    if state.cached_points.is_empty() {
        f.render_widget(
            Paragraph::new(format!(
                "no {} data for {} in this energy range (channel is zero everywhere)",
                state.mt.label(),
                state.nuclide.name()
            ))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" sigma(E, T) [barn] vs E [eV] — log-log "),
            ),
            area,
        );
        return;
    }

    let (x_lo, x_hi) = (
        state
            .cached_points
            .iter()
            .map(|p| p.0)
            .fold(f64::INFINITY, f64::min),
        state
            .cached_points
            .iter()
            .map(|p| p.0)
            .fold(f64::NEG_INFINITY, f64::max),
    );
    let (y_lo, y_hi) = (
        state
            .cached_points
            .iter()
            .map(|p| p.1)
            .fold(f64::INFINITY, f64::min),
        state
            .cached_points
            .iter()
            .map(|p| p.1)
            .fold(f64::NEG_INFINITY, f64::max),
    );
    // A hairline-flat channel (x_lo == x_hi or y_lo == y_hi) would give Chart
    // a zero-width axis range; pad it so the widget always has something to
    // divide by.
    let x_hi = if (x_hi - x_lo).abs() < 1e-9 {
        x_lo + 1.0
    } else {
        x_hi
    };
    let y_hi = if (y_hi - y_lo).abs() < 1e-9 {
        y_lo + 1.0
    } else {
        y_hi
    };

    let dataset = Dataset::default()
        .name(state.mt.label())
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(Color::Green))
        .data(&state.cached_points);

    // ratatui::widgets::Axis::labels currently only positions >3 labels
    // correctly at 2 or 3 (see its doc comment / ratatui#334) — stick to
    // exactly 3 (start/mid/end) on each axis.
    let x_axis = Axis::default()
        .title("E [eV]")
        .style(Style::default().fg(Color::Gray))
        .bounds([x_lo, x_hi])
        .labels([
            log_label(x_lo),
            log_label((x_lo + x_hi) / 2.0),
            log_label(x_hi),
        ]);
    let y_axis = Axis::default()
        .title("sigma [barn]")
        .style(Style::default().fg(Color::Gray))
        .bounds([y_lo, y_hi])
        .labels([
            log_label(y_lo),
            log_label((y_lo + y_hi) / 2.0),
            log_label(y_hi),
        ]);

    let chart = Chart::new(vec![dataset])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" sigma(E, T) [barn] vs E [eV] — log-log "),
        )
        .x_axis(x_axis)
        .y_axis(y_axis);
    f.render_widget(chart, area);
}

/// Temperature nudge applied by a tap on a stepper button, matching the
/// values [`super::temp_button_rects`] hands out (skip the middle `0.0`
/// no-op cell).
pub fn handle_mouse(ev: MouseEvent, area: Rect, state: &mut PlotState) -> PlotAction {
    let layout = plot_layout(area);
    match ev.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if point_in(back_button_rect(layout.title), ev.column, ev.row) {
                return PlotAction::Back;
            }
            if point_in(layout.unit_toggle, ev.column, ev.row) {
                state.toggle_unit();
                return PlotAction::None;
            }
            if point_in(layout.temp_row, ev.column, ev.row) {
                for (delta, rect) in temp_button_rects(layout.temp_row) {
                    if delta != 0.0 && point_in(rect, ev.column, ev.row) {
                        state.nudge_temp(delta);
                        break;
                    }
                }
                return PlotAction::None;
            }
            if point_in(layout.mt_row, ev.column, ev.row) {
                for (ch, rect) in mt_button_rects(layout.mt_row) {
                    if point_in(rect, ev.column, ev.row) {
                        state.mt = ch;
                        break;
                    }
                }
            }
        }
        MouseEventKind::ScrollUp => cycle_mt(state, -1),
        MouseEventKind::ScrollDown => cycle_mt(state, 1),
        _ => {}
    }
    PlotAction::None
}

fn cycle_mt(state: &mut PlotState, dir: i32) {
    let all = MtChannel::ALL;
    let i = all.iter().position(|c| *c == state.mt).unwrap_or(0) as i32;
    let n = all.len() as i32;
    let next = ((i + dir).rem_euclid(n)) as usize;
    state.mt = all[next];
}

pub fn handle_key(key: KeyEvent, state: &mut PlotState) -> PlotAction {
    match key.code {
        KeyCode::Esc => return PlotAction::Back,
        KeyCode::Tab | KeyCode::Char('u') => state.toggle_unit(),
        KeyCode::Left => cycle_mt(state, -1),
        KeyCode::Right => cycle_mt(state, 1),
        KeyCode::Up => state.nudge_temp(1.0),
        KeyCode::Down => state.nudge_temp(-1.0),
        KeyCode::PageUp => state.nudge_temp(10.0),
        KeyCode::PageDown => state.nudge_temp(-10.0),
        _ => {}
    }
    PlotAction::None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_label_formats_round_powers_of_ten() {
        assert_eq!(log_label(0.0), "1e0");
        assert_eq!(log_label(2.0), "1e2");
        assert_eq!(log_label(-3.0), "1e-3");
    }

    #[test]
    fn cycle_mt_wraps_both_directions() {
        let mut s = PlotState::new(crate::xsdata::NuclideXs::load("U238").unwrap());
        s.mt = MtChannel::Total;
        cycle_mt(&mut s, -1);
        assert_eq!(s.mt, MtChannel::Capture); // wrapped to the last channel
        cycle_mt(&mut s, 1);
        assert_eq!(s.mt, MtChannel::Total);
    }
}
