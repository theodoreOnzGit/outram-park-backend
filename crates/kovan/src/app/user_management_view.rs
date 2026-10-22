//! **Code Review ▸ User management** (GitHub #1019): every reviewer, key
//! and agent, and the actions on them. Drawing only: every decision (the
//! roster, statuses, warnings, each action's preconditions, signing and
//! writing) is [`crate::stamping::user_management`]'s, tested in
//! `stamping/user_management_tests.rs`; "My keys on this machine" is the
//! #1020 Signing-key panel's own body (`super::key_window::body`).
//!
//! **Mobile-first.** One scrolling column; every row wraps; the sections
//! are collapsing headers, so it fits a phone-width window. **No lag:**
//! loading and signing run on the panel's workers; drawing only polls.

use egui::RichText;
use kovan_common::review::root::Role;

use super::key_window::{self, KeyPick};
use crate::stamping::key_panel::KeyPanel;
use crate::stamping::user_management::{globs, ReviewerRow, UserAction, UserPanel};

/// One action button: enabled only when the panel lists no problem; the
/// problems are its tooltip. GUI drawing code (exempt; the preconditions
/// are `UserPanel::problems`, tested).
fn action_button(ui: &mut egui::Ui, p: &mut UserPanel, action: UserAction) {
    let problems = p.problems(&action);
    let can = problems.is_empty() && !p.busy();
    if ui
        .add_enabled(can, egui::Button::new(action.label()))
        .on_disabled_hover_text(problems.join("\n"))
        .clicked()
    {
        p.submit(action);
    }
}

/// Draw the tab; returns a file picker the key panel asked for. GUI
/// drawing code (exempt; the logic is `UserPanel` and `KeyPanel`, and
/// `code_review_view`'s headless test draws it).
pub(crate) fn show(ui: &mut egui::Ui, p: &mut UserPanel, keys: Option<&mut KeyPanel>) -> Option<KeyPick> {
    p.poll();
    if p.busy() {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(150));
    }
    let mut pick = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.heading("User management");
            if ui.add_enabled(!p.busy(), egui::Button::new("Reload")).clicked() {
                p.load();
            }
            if p.busy() {
                ui.spinner();
                ui.weak("working in the background\u{2026}");
            }
        });
        ui.weak(
            "Every reviewer and key in kovan_root.toml, and the agents that wrote justifications. \
             Statuses come from the signed registry; actions sign with your session key.",
        );
        signing_key(ui, p);
        if let Some(e) = &p.error {
            ui.colored_label(ui.visuals().error_fg_color, e);
        }
        for n in &p.notices {
            ui.colored_label(ui.visuals().hyperlink_color, n);
        }
        ui.separator();
        match &p.data {
            None => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Reading kovan_root.toml, the reviews and the keystore\u{2026}");
                });
            }
            Some(Err(e)) => {
                ui.colored_label(ui.visuals().error_fg_color, e.clone());
            }
            Some(Ok(l)) => {
                if let Some(n) = l.note.clone() {
                    ui.weak(n);
                }
                roster(ui, p);
            }
        }
        ui.separator();
        egui::CollapsingHeader::new(RichText::new("My keys on this machine").strong())
            .id_salt("users-my-keys")
            .default_open(true)
            .show(ui, |ui| match keys {
                Some(k) => pick = key_window::body(ui, k),
                None => {
                    ui.weak("The keystore is not available.");
                }
            });
    });
    pick
}

/// The key that signs, and the passphrase while it is locked. GUI drawing
/// code (exempt).
fn signing_key(ui: &mut egui::Ui, p: &mut UserPanel) {
    ui.horizontal_wrapped(|ui| {
        ui.label("Act as");
        if p.keys.is_empty() {
            ui.weak("no key on this machine: set one up from a Stamp, or import one below");
        } else {
            let text = p.key().map(|k| format!("{} (key {})", k.reviewer, k.key)).unwrap_or_default();
            egui::ComboBox::from_id_salt("users-signing-key").selected_text(text).show_ui(ui, |ui| {
                for (i, k) in p.keys.iter().enumerate() {
                    let label = format!(
                        "{} (key {}){}",
                        k.reviewer,
                        k.key,
                        if k.registered { "" } else { " \u{2014} not registered" }
                    );
                    ui.selectable_value(&mut p.chosen, i, label);
                }
            });
        }
    });
    let unlocked = p.unlocked();
    key_window::passphrase_row(ui, unlocked, &mut p.passphrase);
}

/// The warnings strip, the reviewers list, the open page and the agents.
/// GUI drawing code (exempt).
fn roster(ui: &mut egui::Ui, p: &mut UserPanel) {
    let Some(ro) = p.roster().cloned() else { return };
    if !ro.warnings.is_empty() {
        let n = ro.warnings.len();
        egui::CollapsingHeader::new(
            RichText::new(format!("\u{26a0} {n} warning{}", if n == 1 { "" } else { "s" }))
                .color(ui.visuals().warn_fg_color),
        )
        .id_salt("users-warnings")
        .default_open(true)
        .show(ui, |ui| {
            for w in &ro.warnings {
                ui.colored_label(ui.visuals().warn_fg_color, format!("\u{2022} {w}"));
            }
        });
    }
    ui.label(RichText::new(format!("Reviewers ({})", ro.reviewers.len())).strong());
    for r in &ro.reviewers {
        let here = r.keys.iter().any(|k| k.here);
        let text = format!(
            "{}  {}   {}   {}   {} key{}{}   {}",
            r.status.marker(),
            r.id,
            r.role_label,
            r.scope_label,
            r.keys.len(),
            if r.keys.len() == 1 { "" } else { "s" },
            if here { " \u{1F511}" } else { "" },
            r.status.label()
        );
        let on = p.selected.as_deref() == Some(r.id.as_str());
        if ui.selectable_label(on, text).clicked() {
            p.selected = if on { None } else { Some(r.id.clone()) };
        }
    }
    if let Some(id) = p.selected.clone() {
        if let Some(r) = ro.reviewer(&id) {
            ui.separator();
            reviewer_page(ui, p, r);
        }
    }
    ui.separator();
    egui::CollapsingHeader::new(RichText::new(format!("Agents ({})", ro.agents.len())).strong())
        .id_salt("users-agents")
        .default_open(!ro.agents.is_empty())
        .show(ui, |ui| {
            ui.weak(
                "AI agents write unsigned justifications through kovan-cli. A justification \
                 never counts as review, alone or as a second sign-off; a human may cite it.",
            );
            if ro.agents.is_empty() {
                ui.weak("No agent has written a justification in this workspace.");
            }
            for a in &ro.agents {
                egui::CollapsingHeader::new(format!(
                    "{}  \u{b7} {} justification{} \u{b7} never counts as review",
                    a.id,
                    a.justifications.len(),
                    if a.justifications.len() == 1 { "" } else { "s" }
                ))
                .id_salt(("users-agent", &a.id))
                .show(ui, |ui| {
                    for j in &a.justifications {
                        ui.label(format!("{}  \u{2014} {} on {}, {}", j.function, j.verdict, j.date, j.code));
                        for e in &j.evidence {
                            ui.weak(format!("    - {e}"));
                        }
                    }
                });
            }
        });
}

/// Role, scope, trust, handover and revocation of an admitted reviewer:
/// each a signed roster event (GitHub #1019 phase 2). GUI drawing code
/// (exempt; preconditions and writes are `UserPanel`'s, tested).
fn standing_controls(ui: &mut egui::Ui, p: &mut UserPanel, r: &ReviewerRow) {
    ui.group(|ui| {
        ui.label(RichText::new("Change standing (signed, from today; earlier stamps keep their verdict)").strong());
        if r.role == Role::Reviewer {
            ui.horizontal_wrapped(|ui| {
                ui.label("Edit scope");
                ui.add(egui::TextEdit::singleline(&mut p.scope_edit).hint_text(r.scope.join(", ")));
                let scope = globs(&p.scope_edit);
                action_button(ui, p, UserAction::ChangeScope { reviewer: r.id.clone(), scope });
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Trust where (empty: everywhere)");
                ui.add(egui::TextEdit::singleline(&mut p.novice_scope).hint_text("crates/njoy-*/**"));
                let scope = globs(&p.novice_scope);
                action_button(ui, p, UserAction::MarkNovice { reviewer: r.id.clone(), scope: scope.clone() });
                action_button(ui, p, UserAction::Promote { reviewer: r.id.clone(), scope });
            });
        }
        ui.horizontal_wrapped(|ui| {
            let other = if r.role == Role::Maintainer { Role::Reviewer } else { Role::Maintainer };
            action_button(ui, p, UserAction::ChangeRole { reviewer: r.id.clone(), role: other });
            if r.role == Role::Reviewer {
                action_button(ui, p, UserAction::HandOver { to: r.id.clone(), step_down: p.step_down });
                ui.checkbox(&mut p.step_down, "and I step down");
            }
        });
        ui.horizontal_wrapped(|ui| {
            action_button(ui, p, UserAction::RevokeReviewer { reviewer: r.id.clone(), compromised: p.compromised });
            ui.checkbox(&mut p.compromised, "compromised (void from today)");
        });
    });
}

/// One reviewer's page. GUI drawing code (exempt).
fn reviewer_page(ui: &mut egui::Ui, p: &mut UserPanel, r: &ReviewerRow) {
    ui.label(RichText::new(match &r.name {
        Some(n) => format!("{} ({n})", r.id),
        None => r.id.clone(),
    })
    .heading());
    ui.horizontal_wrapped(|ui| {
        ui.label(format!("Role: {}", r.role_label));
        ui.label(format!("\u{b7} Admitted: {}", r.admitted));
    });
    ui.label(format!("Scope: {}", r.scope_label));
    ui.label(format!("Status: {}", r.status.label()));
    egui::CollapsingHeader::new("Standing history (append-only)")
        .id_salt(("users-standing", &r.id))
        .default_open(true)
        .show(ui, |ui| {
            for l in &r.standing {
                ui.weak(l);
            }
        });
    if !r.admitted.starts_with("not admitted") {
        standing_controls(ui, p, r);
    }
    if r.admitted.starts_with("not admitted") {
        ui.group(|ui| {
            ui.label(RichText::new("Admit").strong());
            ui.horizontal_wrapped(|ui| {
                ui.label("Role");
                if ui.radio(p.admit.role == Role::Reviewer && !p.admit.novice, "Reviewer").clicked() {
                    (p.admit.role, p.admit.novice) = (Role::Reviewer, false);
                }
                if ui
                    .radio(p.admit.novice, "Novice reviewer")
                    .on_hover_text("Their stamps need a second sign-off by a trusted reviewer or a maintainer until promoted")
                    .clicked()
                {
                    (p.admit.role, p.admit.novice) = (Role::Reviewer, true);
                }
                if ui.radio(p.admit.role == Role::Maintainer, "Maintainer").clicked() {
                    (p.admit.role, p.admit.novice) = (Role::Maintainer, false);
                }
            });
            if p.admit.role == Role::Reviewer {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Scope");
                    ui.add(
                        egui::TextEdit::singleline(&mut p.admit.scope)
                            .hint_text("crates/njoy-*/**, crates/boon-lay/**"),
                    );
                });
            } else {
                ui.weak("A maintainer stamps anywhere and can admit and revoke reviewers.");
            }
            let action = UserAction::Admit {
                reviewer: r.id.clone(),
                role: p.admit.role,
                scope: p.admit.globs(),
                novice: p.admit.novice,
            };
            action_button(ui, p, action);
        });
    }
    ui.label(RichText::new(format!("Keys ({})", r.keys.len())).strong());
    for k in &r.keys {
        ui.group(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(&k.id).strong());
                ui.monospace(&k.fingerprint);
                if k.here {
                    ui.label("\u{1F511} on this machine");
                }
            });
            ui.weak(&k.status);
            for h in &k.history {
                ui.weak(format!("  {h}"));
            }
            ui.horizontal_wrapped(|ui| {
                let (rid, kid) = (r.id.clone(), k.id.clone());
                if !k.trusted {
                    action_button(ui, p, UserAction::Endorse { reviewer: rid.clone(), key: kid.clone() });
                }
                if k.retired {
                    action_button(ui, p, UserAction::UnretireKey { reviewer: rid.clone(), key: kid.clone() });
                } else {
                    action_button(ui, p, UserAction::RetireKey { reviewer: rid.clone(), key: kid.clone() });
                }
                if !k.revoked {
                    action_button(ui, p, UserAction::RevokeKey { reviewer: rid.clone(), key: kid.clone(), compromised: false });
                    action_button(ui, p, UserAction::RevokeKey { reviewer: rid, key: kid, compromised: true });
                }
            });
        });
    }
    ui.label(RichText::new("Reviews").strong());
    ui.label(r.reviews_line());
    egui::CollapsingHeader::new(format!("Show stamps ({})", r.stamps.len()))
        .id_salt(("users-stamps", &r.id))
        .show(ui, |ui| {
            for s in r.stamps.iter().take(500) {
                ui.label(format!("{}  {}  \u{2014} {}", s.date, s.function, s.state.label()));
            }
            if r.stamps.len() > 500 {
                ui.weak(format!("\u{2026} and {} more", r.stamps.len() - 500));
            }
        });
}
