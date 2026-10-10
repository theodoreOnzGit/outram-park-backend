//! The **Organisations & IV&V** window (GitHub #810), opened from the Code
//! Review tab, and the stamp dialog's separation-attestation picker. Every
//! decision (forms, checks, the signing jobs) is
//! [`crate::stamping::organisations::panel::OrgPanel`]'s, tested in
//! `stamping/organisations_tests.rs`; this file only draws and turns clicks
//! into its calls.
//!
//! **No lag (root `CLAUDE.md`, HARD RULE).** The keystore read, the KDF and
//! the write run on worker threads; drawing calls only `OrgPanel::poll`,
//! which never blocks.
//!
//! **Mobile-first.** The window is resizable and scrolls; every section is
//! a collapsing header, folded except the one that applies to the key in
//! use, so it fits a phone-width window.

use egui::RichText;
use kovan_common::review::ivv::AUDIT_RECORD_LABEL;

use crate::stamping::flow::WizardForm;
use crate::stamping::organisations::panel::{Action, OrgPanel};
use crate::stamping::organisations::RecordRow;

/// Draw the window; returns whether it was closed. GUI drawing code
/// (exempt from the test rule): the logic is `OrgPanel`, and
/// `code_review_view`'s headless test draws it.
pub(crate) fn show(ctx: &egui::Context, p: &mut OrgPanel) -> bool {
    p.poll();
    if p.busy() {
        ctx.request_repaint_after(std::time::Duration::from_millis(150));
    }
    let mut open = true;
    let screen = ctx.content_rect().size();
    egui::Window::new("Organisations & IV&V")
        .open(&mut open)
        .resizable(true)
        .default_width(560.0_f32.min(screen.x - 32.0).max(240.0))
        .max_height((screen.y - 80.0).max(200.0))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, true])
                .show(ui, |ui| body(ui, p));
        });
    !open && !p.busy()
}

/// One record line. GUI drawing code (exempt).
fn row(ui: &mut egui::Ui, r: &RecordRow) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(&r.name).strong());
        ui.weak(format!(
            "{} \u{b7} from {} \u{b7} signed: {}",
            r.scope, r.date, r.signed_by
        ));
        if !r.detail.is_empty() {
            ui.weak(&r.detail);
        }
    });
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

/// Problems and the action button. GUI drawing code (exempt).
fn submit(ui: &mut egui::Ui, p: &mut OrgPanel, action: Action, label: &str) {
    let problems = p.problems(action);
    for e in problems.iter().filter(|e| !e.contains("passphrase")) {
        ui.weak(e);
    }
    let can = problems.is_empty() && !p.busy();
    if ui
        .add_enabled(can, egui::Button::new(label))
        .on_disabled_hover_text(problems.join("\n"))
        .clicked()
    {
        p.submit(action);
    }
}

/// GUI drawing code (exempt).
fn body(ui: &mut egui::Ui, p: &mut OrgPanel) {
    ui.label(
        "Rung 5 is independent V&V by an organisation technically and managerially separate \
         from the developing one (NUREG/BR-0167 \u{a7}3.1). A maintainer signs which \
         organisation develops the code and which each reviewer belongs to; the independent \
         reviewer signs that the two are separate, naming a public GitHub issue as the audit \
         record. Kovan checks the link's form only: it never fetches it.",
    );
    ui.separator();
    let overview = match &p.overview {
        None => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Reading kovan_root.toml and the keystore\u{2026}");
            });
            return;
        }
        Some(Err(e)) => {
            ui.colored_label(ui.visuals().error_fg_color, e);
            return;
        }
        Some(Ok(o)) => o.clone(),
    };
    // The key that signs.
    ui.horizontal_wrapped(|ui| {
        ui.label("Signing key");
        if p.keys.is_empty() {
            ui.weak("none in the keystore: set one up from a Stamp first");
        } else {
            let text = p
                .key()
                .map(|k| format!("{} ({})", k.reviewer, k.key))
                .unwrap_or_default();
            egui::ComboBox::from_id_salt("orgs-key")
                .selected_text(text)
                .show_ui(ui, |ui| {
                    for (i, k) in p.keys.iter().enumerate() {
                        let label = format!(
                            "{} ({}){}",
                            k.reviewer,
                            k.key,
                            if k.registered {
                                ""
                            } else {
                                " \u{2014} not registered"
                            }
                        );
                        ui.selectable_value(&mut p.chosen, i, label);
                    }
                });
        }
        ui.weak(if p.is_maintainer() {
            "maintainer"
        } else {
            "reviewer"
        });
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Passphrase");
        ui.add(egui::TextEdit::singleline(&mut p.forms.passphrase).password(true));
    });
    if p.busy() {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.weak("Signing in the background; kovan stays responsive.");
        });
    }
    if let Some(e) = &p.error {
        ui.colored_label(ui.visuals().error_fg_color, e);
    }
    for n in &p.notices {
        ui.colored_label(ui.visuals().hyperlink_color, n);
    }
    if !overview.warnings.is_empty() {
        ui.separator();
        ui.colored_label(
            ui.visuals().warn_fg_color,
            "Records that do not verify (they count for nothing):",
        );
        for w in &overview.warnings {
            ui.colored_label(ui.visuals().warn_fg_color, format!("- {w}"));
        }
    }
    ui.separator();
    let maintainer = p.is_maintainer();
    egui::CollapsingHeader::new(format!(
        "Developing organisation ({})",
        overview.developing.len()
    ))
    .default_open(maintainer)
    .show(ui, |ui| {
        for r in &overview.developing {
            row(ui, r);
        }
        ui.label(
            "Add (maintainer): in force from today; leave the crate empty for the whole workspace.",
        );
        ui.horizontal_wrapped(|ui| {
            ui.label("Organisation");
            ui.text_edit_singleline(&mut p.forms.dev_name);
            ui.label("Crate (optional)");
            ui.add(egui::TextEdit::singleline(&mut p.forms.dev_crate).desired_width(120.0));
        });
        submit(ui, p, Action::DevelopingOrganisation, "Sign and record");
    });
    egui::CollapsingHeader::new(format!(
        "Reviewers' organisations ({})",
        overview.reviewer_organisations.len()
    ))
    .default_open(maintainer)
    .show(ui, |ui| {
        for r in &overview.reviewer_organisations {
            row(ui, r);
        }
        ui.label("Add (maintainer): the reviewer's organisation from today.");
        ui.horizontal_wrapped(|ui| {
            ui.label("Reviewer");
            let shown = if p.forms.rev_reviewer.is_empty() {
                "choose\u{2026}".to_string()
            } else {
                p.forms.rev_reviewer.clone()
            };
            egui::ComboBox::from_id_salt("orgs-reviewer")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    for (id, _) in &overview.reviewers {
                        ui.selectable_value(&mut p.forms.rev_reviewer, id.clone(), id);
                    }
                });
            ui.label("Organisation");
            ui.text_edit_singleline(&mut p.forms.rev_name);
        });
        submit(ui, p, Action::ReviewerOrganisation, "Sign and record");
    });
    egui::CollapsingHeader::new(format!(
        "Separation attestations ({})",
        overview.attestations.len()
    ))
    .default_open(!maintainer)
    .show(ui, |ui| {
        for r in &overview.attestations {
            row(ui, r);
        }
        ui.label(
            "Sign (as the independent reviewer, with your own key): your organisation is \
             technically and managerially separate from the developing organisation. Name a \
             public GitHub issue (open or closed) as the audit record.",
        );
        let a = &mut p.forms.attestation;
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
        submit(ui, p, Action::Attestation, "Sign attestation");
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
             Code Review > Organisations & IV&V.",
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
