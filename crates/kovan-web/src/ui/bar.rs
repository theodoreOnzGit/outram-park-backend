//! The **review bar** at the bottom: one component for both modes (#740,
//! maintainer 2026-10-06: "same bar").
//!
//! For the selected function it shows the stamp state (valid, stale or
//! unreviewed, with rung, reviewer and date when stamped), the maturity,
//! and "blocked by N functions underneath": the workspace callees without a
//! valid stamp, each a link into that function (bottom-up navigation).
//!
//! In [`Mode::Web`] the Stamp and Needs-fix buttons are drawn disabled with
//! "stamping is desktop-only"; [`Mode::Desktop`] (not implemented, #740)
//! enables them and returns [`BarAction::Stamp`] / [`BarAction::NeedsFix`].
//! Compact at phone width (one line and a "▸ N blocking" toggle), expanded
//! to list the callees.

use egui::{Color32, RichText};

use crate::model::Review;
use crate::Mode;

/// What the bar shows.
pub struct BarInfo {
    pub name: String,
    pub id: String,
    pub review: Review,
    pub maturity: Option<u8>,
    /// Callees without a valid stamp, each with its state.
    pub blocked: Vec<(String, Review)>,
    /// All workspace callees (blocked or not).
    pub callees: usize,
    /// The callee list is still loading.
    pub loading: bool,
}

/// What the reader did in the bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BarAction {
    /// Go into this function.
    Goto(String),
    /// Desktop only (#740).
    Stamp,
    NeedsFix,
    ViewSource,
}

/// The stamp state's colour, shared with the cards.
pub fn review_colour(r: &Review) -> Color32 {
    match r {
        Review::Unreviewed => Color32::from_rgb(150, 156, 168),
        Review::Valid(_) => Color32::from_rgb(90, 190, 110),
        Review::Stale(_) => Color32::from_rgb(235, 160, 60),
    }
}

/// `"valid · rung 3 · Theodore Ong · 2026-10-06"`, or `"unreviewed"`.
pub fn review_text(r: &Review) -> String {
    match r {
        Review::Unreviewed => "unreviewed".into(),
        Review::Valid(s) => format!("valid · rung {} · {} · {}", s.rung, s.reviewer, s.date),
        Review::Stale(s) => format!("stale ({}) · was rung {} · {} · {}", s.reason, s.rung, s.reviewer, s.date),
    }
}

/// The bar. `expanded` is the reader's toggle (kept by the caller).
pub fn review_bar(ui: &mut egui::Ui, mode: Mode, info: &BarInfo, expanded: &mut bool, narrow: bool) -> Option<BarAction> {
    let mut action = None;
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(&info.name).strong().monospace());
        ui.label(RichText::new(review_text(&info.review)).color(review_colour(&info.review)));
        if let Some(m) = info.maturity {
            ui.label(format!("· M{m} {}", kovan_common::code_map::maturity_label(m)));
        }
        let n = info.blocked.len();
        let blocked = if info.loading {
            "callees loading…".to_string()
        } else if n == 0 {
            format!("not blocked ({} callees)", info.callees)
        } else if narrow {
            format!("{} {n} blocking", if *expanded { "hide" } else { "show" })
        } else {
            format!("{} blocked by {n} function{} underneath", if *expanded { "hide" } else { "show" }, if n == 1 { "" } else { "s" })
        };
        if n > 0 && !info.loading {
            if ui.button(blocked).on_hover_text("The workspace functions this one calls that have no valid stamp. Bottom-up: they are reviewed first.").clicked() {
                *expanded = !*expanded;
            }
        } else {
            ui.weak(blocked);
        }
        if ui.button("Source").clicked() {
            action = Some(BarAction::ViewSource);
        }
        let can = mode.can_stamp();
        let why = "stamping is desktop-only";
        let stamp = ui.add_enabled(can, egui::Button::new("Stamp"));
        let fix = ui.add_enabled(can, egui::Button::new("Needs fix"));
        if can {
            if stamp.clicked() {
                action = Some(BarAction::Stamp);
            }
            if fix.clicked() {
                action = Some(BarAction::NeedsFix);
            }
        } else {
            stamp.on_disabled_hover_text(why);
            fix.on_disabled_hover_text(why);
            if !narrow {
                ui.weak(why);
            }
        }
    });
    if *expanded && !info.blocked.is_empty() {
        egui::ScrollArea::vertical().max_height(if narrow { 140.0 } else { 120.0 }).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (id, r) in &info.blocked {
                    let label = RichText::new(crate::model::short_name(id)).monospace().color(review_colour(r));
                    if ui.link(label).on_hover_text(id).clicked() {
                        action = Some(BarAction::Goto(id.clone()));
                    }
                }
            });
        });
    }
    // Hook (#746, call-graph schema 2): "Tests that reach this" goes here,
    // below the blocked-by list, once the data carries it.
    action
}
