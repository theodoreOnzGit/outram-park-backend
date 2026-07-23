//! Rendering: one draw function per screen, each laying out a narrow
//! single-column phone-terminal frame and registering the tap/scroll hit
//! regions [`App::tap`]/[`App::scroll`] resolve against (op-omf mobile-first
//! layout).
//!
//! # Layout convention
//!
//! Every screen is `[header] [scrollable content] [footer]`, stacked
//! vertically with no side-by-side panes — a phone terminal is narrow, not
//! short, so horizontal splits waste the one dimension there's plenty of and
//! starve the one there is not. Content that overflows the visible height
//! paginates by whole item (card/row), not by pixel row, driven by
//! `app.geometry_scroll` / `app.settings_scroll` — see [`draw_geometry_picker`]
//! and [`draw_settings`].
//!
//! Every tappable element is at least 3 terminal rows tall (a bordered
//! `Block`), a deliberately large touch target for a fingertip on a phone
//! screen, per op-omf.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{current_spectrum, App, Screen, UiAction};
use crate::presets::{GeometryPreset, ALL_PRESETS};
use crate::settings::RunSettings;
use crate::transport::RunPhase;

const ACCENT: Color = Color::Cyan;
const GOOD: Color = Color::Green;
const WARN: Color = Color::Yellow;
const BAD: Color = Color::Red;

/// Top-level draw dispatch, called once per redraw tick from `main.rs`.
/// Clears the previous frame's hit regions first — every screen function
/// repopulates them from scratch, so a stale region from a screen the user
/// just left can never be tapped by accident.
pub fn draw(frame: &mut Frame, app: &mut App) {
    app.hit_regions.clear();
    app.scroll_region = None;

    let area = frame.area();
    let [header, body, footer] =
        Layout::vertical([Constraint::Length(2), Constraint::Min(3), Constraint::Length(2)]).areas(area);

    draw_header(frame, app, header);
    match app.screen {
        Screen::GeometryPicker => draw_geometry_picker(frame, app, body),
        Screen::Settings => draw_settings(frame, app, body),
        Screen::Run => draw_run(frame, app, body),
        Screen::Spectrum => draw_spectrum(frame, app, body),
    }
    draw_footer(frame, app, footer);
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let title = match app.screen {
        Screen::GeometryPicker => "OUTRAM-MC  \u{2022}  pick a geometry",
        Screen::Settings => "OUTRAM-MC  \u{2022}  run settings",
        Screen::Run => "OUTRAM-MC  \u{2022}  k-eigenvalue run",
        Screen::Spectrum => "OUTRAM-MC  \u{2022}  spectrum + cross section",
    };
    let line1 = Line::from(Span::styled(title, Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)));
    let line2 = Line::from(Span::styled(app.selected_preset.title(), Style::default().add_modifier(Modifier::DIM)));
    frame.render_widget(Paragraph::new(vec![line1, line2]), area);
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let msg = app.status.clone().unwrap_or_else(|| match app.screen {
        Screen::GeometryPicker => "tap a card to continue  \u{2022}  q to quit".to_string(),
        Screen::Settings => "tap +/- to adjust  \u{2022}  tap RUN to launch".to_string(),
        Screen::Run => "tap Spectrum / Settings / Rerun below".to_string(),
        Screen::Spectrum => "tap Back below".to_string(),
    });
    frame.render_widget(Paragraph::new(Line::from(Span::styled(msg, Style::default().add_modifier(Modifier::DIM)))), area);
}

/// Register a tap target: draws `block` in `rect` and pushes `(rect, action)`
/// into `app.hit_regions` so `App::tap` resolves it. Every touch button /
/// card / stepper-arrow in this module goes through this one function, so
/// "what can I tap" and "what does tapping here do" can never drift apart.
fn tap_target(frame: &mut Frame, app: &mut App, rect: Rect, action: UiAction, block: Block<'static>, text: Paragraph<'static>) {
    frame.render_widget(block, rect);
    let inner = Rect { x: rect.x + 1, y: rect.y + 1, width: rect.width.saturating_sub(2), height: rect.height.saturating_sub(2) };
    frame.render_widget(text, inner);
    app.hit_regions.push((rect, action));
}

// ─────────────────────────────────────────────────────────────────────────
// Screen 1: geometry picker (op-dyt)
// ─────────────────────────────────────────────────────────────────────────

const CARD_HEIGHT: u16 = 6;

fn draw_geometry_picker(frame: &mut Frame, app: &mut App, area: Rect) {
    app.scroll_region = Some(area);
    let visible = (area.height / CARD_HEIGHT).max(1) as usize;
    let len = ALL_PRESETS.len();
    let max_scroll = len.saturating_sub(visible);
    if app.geometry_scroll as usize > max_scroll {
        app.geometry_scroll = max_scroll as u16;
    }
    let start = app.geometry_scroll as usize;

    for (row, idx) in (start..(start + visible).min(len)).enumerate() {
        let rect = Rect { x: area.x, y: area.y + row as u16 * CARD_HEIGHT, width: area.width, height: CARD_HEIGHT };
        if rect.y + rect.height > area.y + area.height {
            break;
        }
        let card_preset = card_preset_for(app, idx);
        let selected = std::mem::discriminant(&card_preset) == std::mem::discriminant(&app.selected_preset);
        let border_style = if selected { Style::default().fg(ACCENT) } else { Style::default() };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(format!(" {} ", card_preset.title()));
        let body = Paragraph::new(card_preset.summary()).wrap(Wrap { trim: true });
        tap_target(frame, app, rect, UiAction::SelectPreset(card_preset), block, body);

        // The bare-sphere card additionally carries a small isotope-toggle
        // tap target in its title bar (top-right corner cell), so switching
        // Godiva<->Jezebel does not require leaving the picker.
        if matches!(card_preset, GeometryPreset::BareSphere(_)) && rect.width > 14 {
            let toggle_rect = Rect { x: rect.x + rect.width.saturating_sub(12), y: rect.y, width: 11, height: 1 };
            app.hit_regions.push((toggle_rect, UiAction::ToggleSphereVariant));
        }
    }

    if len > visible {
        let hint = Rect { x: area.x, y: area.y + area.height.saturating_sub(1), width: area.width, height: 1 };
        frame.render_widget(
            Paragraph::new(format!("({}/{}) scroll for more", start + 1, len)).style(Style::default().add_modifier(Modifier::DIM)),
            hint,
        );
    }
}

/// Which concrete preset card index `idx` in [`ALL_PRESETS`] shows, preserving
/// the user's Godiva/Jezebel toggle if that card is the currently selected one
/// (see the module docs on [`crate::app::App::dispatch`]'s
/// `ToggleSphereVariant` handling).
fn card_preset_for(app: &App, idx: usize) -> GeometryPreset {
    let base = ALL_PRESETS[idx];
    if matches!(base, GeometryPreset::BareSphere(_)) {
        if let GeometryPreset::BareSphere(v) = app.selected_preset {
            return GeometryPreset::BareSphere(v);
        }
    }
    base
}

// ─────────────────────────────────────────────────────────────────────────
// Screen 2: run settings (op-4h8)
// ─────────────────────────────────────────────────────────────────────────

const ROW_HEIGHT: u16 = 3;

/// Height of the always-visible compute-backend panel: title/value line + GPU
/// status line + the honest compute-dispatch caveat (see
/// `App::compute_dispatch_note`), plus top/bottom border.
const COMPUTE_PANEL_HEIGHT: u16 = 5;

fn draw_settings(frame: &mut Frame, app: &mut App, area: Rect) {
    app.scroll_region = Some(area);
    let back_rect = Rect { x: area.x, y: area.y, width: area.width, height: ROW_HEIGHT };
    tap_target(
        frame, app, back_rect, UiAction::GoToGeometryPicker,
        Block::default().borders(Borders::ALL),
        Paragraph::new("< back to geometry"),
    );

    // Compute-backend panel: always shown in full (never scrolled away) since
    // the GPU-availability + dispatch-caveat text is important context for
    // interpreting the run that follows.
    let compute_rect = Rect { x: area.x, y: area.y + ROW_HEIGHT, width: area.width, height: COMPUTE_PANEL_HEIGHT };
    let s: RunSettings = app.settings;
    let compute_text = format!("Compute: {}\n{}\n{}", s.compute_label(), app.gpu_status_line(), app.compute_dispatch_note());
    tap_target(
        frame, app, compute_rect, UiAction::CycleCompute,
        Block::default().borders(Borders::ALL).title(" tap to cycle "),
        Paragraph::new(compute_text).wrap(Wrap { trim: true }),
    );

    let steppers_top = area.y + ROW_HEIGHT + COMPUTE_PANEL_HEIGHT;
    let rows_area = Rect {
        x: area.x,
        y: steppers_top,
        width: area.width,
        height: area.height.saturating_sub(ROW_HEIGHT + COMPUTE_PANEL_HEIGHT + 3),
    };
    let visible = (rows_area.height / ROW_HEIGHT).max(1) as usize;
    const N_ROWS: usize = 4; // particles, inactive, active, seed
    let max_scroll = N_ROWS.saturating_sub(visible);
    if app.settings_scroll as usize > max_scroll {
        app.settings_scroll = max_scroll as u16;
    }
    let start = app.settings_scroll as usize;

    let rows: Vec<(String, UiAction, UiAction)> = vec![
        (format!("Histories/gen: {}", s.n_particles), UiAction::BumpParticles(-1), UiAction::BumpParticles(1)),
        (format!("Inactive gens: {}", s.n_inactive), UiAction::BumpInactive(-1), UiAction::BumpInactive(1)),
        (format!("Active gens (batches): {}", s.n_active), UiAction::BumpActive(-1), UiAction::BumpActive(1)),
        (format!("Seed: {}", s.seed), UiAction::BumpSeed(-1), UiAction::BumpSeed(1)),
    ];

    for (row, i) in (start..(start + visible).min(N_ROWS)).enumerate() {
        let rect = Rect { x: rows_area.x, y: rows_area.y + row as u16 * ROW_HEIGHT, width: rows_area.width, height: ROW_HEIGHT };
        if rect.y + rect.height > rows_area.y + rows_area.height {
            break;
        }
        let (label, minus_action, plus_action) = &rows[i];
        let [minus_r, label_r, plus_r] =
            Layout::horizontal([Constraint::Length(5), Constraint::Min(6), Constraint::Length(5)]).areas(rect);
        tap_target(frame, app, minus_r, *minus_action, Block::default().borders(Borders::ALL), Paragraph::new("-").centered());
        frame.render_widget(Block::default().borders(Borders::ALL).title(Line::from(label.as_str()).centered()), label_r);
        tap_target(frame, app, plus_r, *plus_action, Block::default().borders(Borders::ALL), Paragraph::new("+").centered());
    }

    let run_rect = Rect { x: area.x, y: area.y + area.height.saturating_sub(3), width: area.width, height: 3 };
    tap_target(
        frame, app, run_rect, UiAction::StartRun,
        Block::default().borders(Borders::ALL).border_style(Style::default().fg(GOOD).add_modifier(Modifier::BOLD)),
        Paragraph::new("RUN").centered().style(Style::default().add_modifier(Modifier::BOLD)),
    );
}

// ─────────────────────────────────────────────────────────────────────────
// Screen 3: run / live convergence (op-4h8)
// ─────────────────────────────────────────────────────────────────────────

const SPINNER: [&str; 4] = ["\u{25f0}", "\u{25f3}", "\u{25f2}", "\u{25f1}"];

fn draw_run(frame: &mut Frame, app: &mut App, area: Rect) {
    let [status_area, chart_area, nav_area] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(5), Constraint::Length(3)]).areas(area);

    // `app.last_outcome` is the single source of truth once a run has
    // finished (`App::on_tick` copies it out of `run_handle` the moment the
    // background thread reports `Done`, and the smoke test injects it
    // directly without a real thread) — the live `run_handle` snapshot is
    // only consulted for the Idle/Running states, which have no outcome yet.
    match &app.last_outcome {
        Some(Ok(outcome)) => {
            let text = format!(
                "k_mean = {:.5} \u{00b1} {:.5}   ({:.2?}, {} gens)",
                outcome.k_mean, outcome.k_std, outcome.elapsed, outcome.k_by_generation.len()
            );
            frame.render_widget(Paragraph::new(text).style(Style::default().fg(GOOD)), status_area);
            draw_convergence_chart(frame, chart_area, &outcome.k_by_generation, app.revealed);
        }
        Some(Err(e)) => {
            frame.render_widget(Paragraph::new(format!("run failed: {e}")).style(Style::default().fg(BAD)), status_area);
        }
        None => match app.run_handle.snapshot() {
            RunPhase::Running { started } => {
                let elapsed = started.elapsed().as_secs_f32();
                let frame_idx = (elapsed * 4.0) as usize % SPINNER.len();
                let s = app.settings;
                let text = format!(
                    "{} running... {:.1}s\n{} histories x [{} inactive + {} active] gens, seed {}",
                    SPINNER[frame_idx], elapsed, s.n_particles, s.n_inactive, s.n_active, s.seed
                );
                frame.render_widget(Paragraph::new(text).style(Style::default().fg(WARN)), status_area);
            }
            RunPhase::Idle | RunPhase::Done(_) => {
                frame.render_widget(Paragraph::new("no run started yet"), status_area);
            }
        },
    }

    let can_spectrum = app.spectrum_available();
    let [back_r, rerun_r, spectrum_r] = Layout::horizontal([
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
    ])
    .areas(nav_area);
    tap_target(frame, app, back_r, UiAction::GoToSettings, Block::default().borders(Borders::ALL), Paragraph::new("< Settings").centered());
    tap_target(frame, app, rerun_r, UiAction::StartRun, Block::default().borders(Borders::ALL), Paragraph::new("Rerun").centered());
    let spectrum_style = if can_spectrum { Style::default().fg(ACCENT) } else { Style::default().add_modifier(Modifier::DIM) };
    tap_target(
        frame, app, spectrum_r, UiAction::GoToSpectrum,
        Block::default().borders(Borders::ALL).border_style(spectrum_style),
        Paragraph::new(if can_spectrum { "Spectrum >" } else { "Spectrum (n/a)" }).centered(),
    );
}

fn draw_convergence_chart(frame: &mut Frame, area: Rect, k_by_generation: &[f64], revealed: usize) {
    if k_by_generation.is_empty() {
        return;
    }
    let n = revealed.min(k_by_generation.len()).max(1);
    let points: Vec<(f64, f64)> = k_by_generation[..n].iter().enumerate().map(|(i, &k)| (i as f64, k)).collect();
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for &(_, k) in &points {
        lo = lo.min(k);
        hi = hi.max(k);
    }
    if !lo.is_finite() || !hi.is_finite() {
        lo = 0.0;
        hi = 2.0;
    }
    let pad = ((hi - lo).abs() * 0.15).max(0.02);
    let (y_lo, y_hi) = (lo - pad, hi + pad);
    let x_hi = (k_by_generation.len().saturating_sub(1)).max(1) as f64;

    let dataset = Dataset::default()
        .name("k per generation")
        .marker(Marker::Dot)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(ACCENT))
        .data(&points);

    let chart = Chart::new(vec![dataset])
        .block(Block::default().borders(Borders::ALL).title(" convergence "))
        .x_axis(Axis::default().title("generation").bounds([0.0, x_hi]).labels(["0".to_string(), format!("{x_hi:.0}")]))
        .y_axis(Axis::default().title("k").bounds([y_lo, y_hi]).labels([format!("{y_lo:.3}"), format!("{y_hi:.3}")]));
    frame.render_widget(chart, area);
}

// ─────────────────────────────────────────────────────────────────────────
// Screen 4: neutron spectrum + cross-section overlay (op-iom)
// ─────────────────────────────────────────────────────────────────────────

fn draw_spectrum(frame: &mut Frame, app: &mut App, area: Rect) {
    let [status_area, chart_area, nav_area] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(5), Constraint::Length(3)]).areas(area);

    match current_spectrum(app) {
        Some(overlay) => {
            frame.render_widget(
                Paragraph::new(format!(
                    "flux (line) vs Sigma_t of {} (dashed) - shared log-E axis, each normalized to its own max",
                    overlay.xs_material_name
                ))
                .wrap(Wrap { trim: true }),
                status_area,
            );
            draw_overlay_chart(frame, chart_area, overlay);
        }
        None => {
            let msg = if app.spectrum_available() {
                "no finished run yet - go back and tap RUN first."
            } else {
                "this preset's driver (delta/Woodcock tracking) has no tally hook in this \
                 crate version, so no spectrum is tallied for it. See the crate README / \
                 outram-mc-libs pebble_beds::keff_delta docs."
            };
            frame.render_widget(Paragraph::new(msg).wrap(Wrap { trim: true }).style(Style::default().fg(WARN)), status_area);
        }
    }

    let [back_r, _spacer] = Layout::horizontal([Constraint::Length(14), Constraint::Min(1)]).areas(nav_area);
    tap_target(frame, app, back_r, UiAction::GoToRun, Block::default().borders(Borders::ALL), Paragraph::new("< Back").centered());
}

fn draw_overlay_chart(frame: &mut Frame, area: Rect, overlay: &crate::spectrum::SpectrumOverlay) {
    let mids = overlay.mid_energies();
    let log_e: Vec<f64> = mids.iter().map(|e| e.max(1.0e-10).log10()).collect();

    let flux_max = overlay.flux_mean.iter().cloned().fold(0.0_f64, f64::max).max(1.0e-300);
    let xs_max = overlay.xs_mid.iter().cloned().fold(0.0_f64, f64::max).max(1.0e-300);

    let flux_points: Vec<(f64, f64)> = log_e.iter().zip(&overlay.flux_mean).map(|(&x, &y)| (x, y / flux_max)).collect();
    let xs_points: Vec<(f64, f64)> = log_e.iter().zip(&overlay.xs_mid).map(|(&x, &y)| (x, y / xs_max)).collect();

    let x_lo = log_e.first().copied().unwrap_or(-3.0);
    let x_hi = log_e.last().copied().unwrap_or(7.0);

    let flux_ds = Dataset::default()
        .name("flux (norm.)")
        .marker(Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(ACCENT))
        .data(&flux_points);
    let xs_ds = Dataset::default()
        .name("Sigma_t (norm.)")
        .marker(Marker::Dot)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(WARN))
        .data(&xs_points);

    let chart = Chart::new(vec![flux_ds, xs_ds])
        .block(Block::default().borders(Borders::ALL).title(" flux vs Sigma_t (log10 E/eV) "))
        .x_axis(
            Axis::default()
                .title("log10(E/eV)")
                .bounds([x_lo, x_hi])
                .labels([format!("{x_lo:.1}"), format!("{:.1}", (x_lo + x_hi) / 2.0), format!("{x_hi:.1}")]),
        )
        .y_axis(Axis::default().title("normalized").bounds([0.0, 1.05]).labels(["0".to_string(), "1".to_string()]));
    frame.render_widget(chart, area);
}
