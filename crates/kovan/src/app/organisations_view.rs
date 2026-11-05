//! The **organisation and IV&V sections** of Code Review ▸ User management
//! (GitHub #810, #1211), and the stamp dialog's separation-attestation
//! picker. ~~The Organisations & IV&V window, opened from the Code Review
//! tab, over its own `OrgPanel`~~ **CORRECTED 2026-11-04** (#1211,
//! maintainer: "move the IV&V window into the User management tab"): the
//! window and its button are gone; these are collapsing sections of the
//! tab, signed through its [`UserPanel`] (one key, one passphrase, one
//! worker). Every decision (forms, checks, the signing jobs) is
//! `UserPanel`'s and [`crate::stamping::organisations`]'s, tested in
//! `stamping/organisations_tests.rs`; this file only draws and turns
//! clicks into their calls.
//!
//! **No lag (root `CLAUDE.md`, HARD RULE).** The KDF and the write run on
//! the panel's worker; drawing never blocks.
//!
//! **Mobile-first.** One scrolling column (the tab's); every row wraps;
//! every section is a collapsing header, folded except the one that
//! applies to the key in use.

use egui::RichText;
use kovan_common::review::ivv::AUDIT_RECORD_LABEL;

use super::user_management_view::action_button;
use crate::stamping::flow::WizardForm;
use crate::stamping::organisations::RecordRow;
use crate::stamping::user_management::{UserAction, UserPanel};

/// One record line, retired ones marked, with its Retire button (founder).
/// GUI drawing code (exempt; what Retire does is `UserPanel`'s, tested).
fn row(ui: &mut egui::Ui, p: &mut UserPanel, r: &RecordRow) {
    ui.horizontal_wrapped(|ui| {
        let name = RichText::new(&r.name).strong();
        ui.label(if r.is_retired { name.strikethrough() } else { name });
        ui.weak(format!(
            "{} \u{b7} from {} \u{b7} signed: {}",
            r.scope, r.date, r.signed_by
        ));
        if !r.detail.is_empty() {
            ui.weak(&r.detail);
        }
        if let Some(t) = &r.retire {
            if p.is_founder() {
                action_button(ui, p, UserAction::RetireOrganisation(t.clone()));
            }
        }
    });
    for l in &r.retired {
        ui.weak(format!("    {l}"));
    }
    if let Some(n) = &r.note {
        ui.colored_label(ui.visuals().warn_fg_color, format!("    \u{26a0} {n}"));
    }
    if let Some(u) = &r.audit_record {
        audit_link(ui, u);
    }
}

/// The audit record, labelled as not verified. GUI drawing code (exempt).
fn audit_link(ui: &mut egui::Ui, url: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.weak(format!("{AUDIT_RECORD_LABEL}:"));
        ui.hyperlink_to(url, url);
    });
}

/// The organisation and attestation sections, inside the User management
/// tab's scrolling column. GUI drawing code (exempt; the logic is
/// `UserPanel` over `stamping::organisations`, and `code_review_view`'s
/// headless test draws it).
pub(crate) fn sections(ui: &mut egui::Ui, p: &mut UserPanel) {
    let Some(overview) = p.ivv().cloned() else { return };
    let founder = p.is_founder();
    egui::CollapsingHeader::new(RichText::new("Organisations & IV&V (rung 5)").strong())
        .id_salt("users-ivv")
        .default_open(false)
        .show(ui, |ui| {
            ui.label(
                "Rung 5 is independent V&V by an organisation technically and managerially \
                 separate from the developing one (NUREG/BR-0167 \u{a7}3.1). A founder signs \
                 which organisation develops the code and which each reviewer belongs to; the \
                 independent reviewer signs that the two are separate, naming a public GitHub \
                 issue as the audit record. Kovan checks the link's form only: it never fetches \
                 it. Removal is retire: the record stays, marked retired.",
            );
            ui.weak(if overview.founders.is_empty() {
                "No founder is in standing: no organisation record can be signed.".to_string()
            } else {
                format!("Founders (any one signs): {}", overview.founders.join(", "))
            });
            if !overview.warnings.is_empty() {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    "Records that do not verify (they count for nothing):",
                );
                for w in &overview.warnings {
                    ui.colored_label(ui.visuals().warn_fg_color, format!("- {w}"));
                }
            }
        });
    egui::CollapsingHeader::new(format!(
        "Developing organisation ({})",
        overview.developing.len()
    ))
    .id_salt("users-ivv-dev")
    .default_open(founder)
    .show(ui, |ui| {
        for r in &overview.developing {
            row(ui, p, r);
        }
        ui.label(
            "Add (founder): in force from today; leave the crate empty for the whole workspace.",
        );
        ui.horizontal_wrapped(|ui| {
            ui.label("Organisation");
            ui.text_edit_singleline(&mut p.orgs.dev_name);
            ui.label("Crate (optional)");
            ui.add(egui::TextEdit::singleline(&mut p.orgs.dev_crate).desired_width(120.0));
        });
        let a = p.developing_action();
        action_button(ui, p, a);
    });
    egui::CollapsingHeader::new(format!(
        "Reviewers' organisations ({})",
        overview.reviewer_organisations.len()
    ))
    .id_salt("users-ivv-rev")
    .default_open(founder)
    .show(ui, |ui| {
        for r in &overview.reviewer_organisations {
            row(ui, p, r);
        }
        ui.label("Add (founder): the reviewer's organisation from today.");
        ui.horizontal_wrapped(|ui| {
            ui.label("Reviewer");
            let shown = if p.orgs.rev_reviewer.is_empty() {
                "choose\u{2026}".to_string()
            } else {
                p.orgs.rev_reviewer.clone()
            };
            egui::ComboBox::from_id_salt("orgs-reviewer")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    for (id, _) in &overview.reviewers {
                        ui.selectable_value(&mut p.orgs.rev_reviewer, id.clone(), id);
                    }
                });
            ui.label("Organisation");
            ui.text_edit_singleline(&mut p.orgs.rev_name);
        });
        let a = p.reviewer_org_action();
        action_button(ui, p, a);
    });
    egui::CollapsingHeader::new(format!(
        "Separation attestations ({})",
        overview.attestations.len()
    ))
    .id_salt("users-ivv-att")
    .default_open(!founder)
    .show(ui, |ui| {
        for r in &overview.attestations {
            row(ui, p, r);
        }
        ui.label(
            "Sign (as the independent reviewer, with your own key): your organisation is \
             technically and managerially separate from the developing organisation. Name a \
             public GitHub issue (open or closed) as the audit record.",
        );
        let a = &mut p.orgs.attestation;
        egui::Grid::new("orgs-attest")
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("Id");
                ui.text_edit_singleline(&mut a.id);
                ui.end_row();
                ui.label("Your organisation");
                ui.text_edit_singleline(&mut a.organisation);
                ui.end_row();
                ui.label("Developing organisation");
                ui.text_edit_singleline(&mut a.developing_organisation);
                ui.end_row();
                ui.label("Audit record");
                ui.add(
                    egui::TextEdit::singleline(&mut a.audit_record)
                        .hint_text("https://github.com/<owner>/<repo>/issues/<n>"),
                );
                ui.end_row();
            });
        if ui.small_button("Fill from the records").clicked() {
            p.prefill_attestation();
        }
        let a = p.attestation_action();
        action_button(ui, p, a);
    });
}

/// The stamp dialog's separation-attestation picker (GitHub #810): only
/// the reviewer's own signed attestations; none by default. GUI drawing
/// code (exempt): the choices and what is signed are
/// [`crate::stamping::flow`]'s, tested in `stamping/organisations_tests.rs`.
pub(crate) fn attestation_picker(ui: &mut egui::Ui, w: &mut WizardForm) {
    ui.add_space(4.0);
    ui.label(RichText::new("Independent V&V (rung 5)").strong());
    if w.attestations.is_empty() {
        ui.weak(
            "You have no signed separation attestation: this review claims no IV&V. Sign one in \
             Code Review > User management > Separation attestations.",
        );
        w.separation_attestation = None;
        return;
    }
    let shown = w
        .separation_attestation
        .as_ref()
        .and_then(|id| w.attestations.iter().find(|c| &c.id == id))
        .map_or("none (no IV&V claimed)".to_string(), |c| c.label());
    egui::ComboBox::from_id_salt("stamp-attestation")
        .selected_text(shown)
        .show_ui(ui, |ui| {
            ui.selectable_value(
                &mut w.separation_attestation,
                None,
                "none (no IV&V claimed)",
            );
            for c in &w.attestations {
                ui.selectable_value(&mut w.separation_attestation, Some(c.id.clone()), c.label());
            }
        });
    if let Some(c) = w
        .separation_attestation
        .as_ref()
        .and_then(|id| w.attestations.iter().find(|c| &c.id == id))
    {
        audit_link(ui, &c.audit_record);
        ui.weak(
            "The attestation is signed into this stamp. Kovan then judges rung 5; every reason \
             it is not reached is shown on the function.",
        );
    }
}
