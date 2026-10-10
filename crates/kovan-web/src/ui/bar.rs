//! The **review bar** at the bottom: one component for both modes (#740,
//! maintainer 2026-10-06: "same bar").
//!
//! For the selected function it shows the stamp state (valid, stale or
//! unreviewed, with rung, reviewer and date when stamped), the maturity,
//! and "blocked by N functions underneath": the workspace callees without a
//! valid stamp, each a link into that function (bottom-up navigation).
//!
//! In [`Mode::Web`] the Stamp and Needs-fix buttons are drawn disabled with
//! "stamping is desktop-only"; [`Mode::Desktop`] enables them and returns
//! [`BarAction::Stamp`] / [`BarAction::NeedsFix`], which
//! [`super::CodeReview`] turns into a [`super::HostRequest`] for desktop
//! kovan to take (its stamp dialog, #740, #770).
//! Compact at phone width (one line and a "▸ N blocking" toggle), expanded
//! to list the callees.

use egui::{Color32, RichText};

use kovan_common::review::state::Tone;

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
    /// Schema 2 (#746): the nearest tests reaching this function (id, hops),
    /// the total, and the examples reaching it (name, hops).
    pub tests: Vec<(String, u32)>,
    pub tests_total: usize,
    pub examples: Vec<(String, u32)>,
}

/// What the reader did in the bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BarAction {
    /// Go into this function.
    Goto(String),
    /// Desktop only (#740): becomes [`super::HostRequest::Stamp`].
    Stamp,
    /// Desktop only: becomes [`super::HostRequest::NeedsFix`].
    NeedsFix,
    ViewSource,
}

/// The colour of a state's tone ([`Tone`], shared by every view).
pub fn tone_colour(t: Tone) -> Color32 {
    match t {
        Tone::Neutral => Color32::from_rgb(150, 156, 168),
        Tone::Good => Color32::from_rgb(90, 190, 110),
        Tone::Attention => Color32::from_rgb(235, 160, 60),
        Tone::Blocked => Color32::from_rgb(220, 80, 80),
    }
}

/// The stamp state's colour, shared with the cards.
pub fn review_colour(r: &Review) -> Color32 {
    tone_colour(r.kind().tone())
}

/// `"valid · rung 3 · Theodore Ong · 2026-10-06"`, `"unreviewed"`, or the
/// state's label with its reason, e.g. `"changed since review (code
/// changed) · was rung 3 · …"`.
pub fn review_text(r: &Review) -> String {
    match r {
        Review::Unreviewed => r.label().into(),
        Review::Stamped(k, s) if k.counts() => format!("{} · rung {} · {} · {}", k.label(), s.rung, s.reviewer, s.date),
        Review::Stamped(k, s) => {
            let why = if s.reason.is_empty() { String::new() } else { format!(" ({})", s.reason) };
            format!("{}{why} · was rung {} · {} · {}", k.label(), s.rung, s.reviewer, s.date)
        }
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
        if (n > 0 && !info.loading) || info.tests_total > 0 {
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
    // Tests that reach this (#746): resolved calls only, so a lower bound.
    if *expanded && (info.tests_total > 0 || !info.examples.is_empty()) {
        ui.horizontal_wrapped(|ui| {
            ui.weak(format!("reached by {} test{} (nearest {} shown):", info.tests_total, if info.tests_total == 1 { "" } else { "s" }, info.tests.len()));
            for (id, hops) in &info.tests {
                if ui.link(RichText::new(format!("{} ({hops})", crate::model::short_name(id))).monospace()).on_hover_text(format!("{id}\n{hops} call(s) away")).clicked() {
                    action = Some(BarAction::Goto(id.clone()));
                }
            }
            for (ex, hops) in &info.examples {
                ui.weak(format!("example {ex} ({hops})"));
            }
        });
    } else if !*expanded && info.tests_total > 0 {
        ui.weak(format!("reached by {} tests", info.tests_total));
    }
    action
}
