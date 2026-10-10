//! The **stamp dialog**'s drawing (GitHub #770, #740): a modal window over
//! the Code Review tab, opened by the review bar's Stamp or Needs fix. Every
//! decision (steps, transitions, form checks, the background jobs) is
//! [`crate::stamping::flow::StampFlow`]'s, tested in
//! `stamping/flow_tests.rs`; this file only draws the current step and turns
//! clicks into the flow's calls.
//!
//! **No lag (root `CLAUDE.md`, HARD RULE).** The flow runs git, the KDF and
//! the writes on worker threads; drawing calls only [`StampFlow::poll`],
//! which never blocks, and shows a spinner while a job runs.

use egui::RichText;
use kovan_common::review::wizard::{GateReason, ReviewWizard, TestAuthorship};

use crate::stamping::flow::{Purpose, StampFlow, Step, WizardForm};

/// Draw the dialog; returns whether it was closed. GUI drawing code
/// (exempt from the test rule): the logic is `stamping::flow`, and
/// `code_review_view`'s headless test draws every step.
pub(crate) fn show(ctx: &egui::Context, f: &mut StampFlow) -> bool {
    f.poll();
    if f.busy() {
        ctx.request_repaint_after(std::time::Duration::from_millis(150));
    }
    let mut close = false;
    let screen = ctx.content_rect().size();
    let max = egui::vec2((screen.x - 48.0).max(240.0), (screen.y - 140.0).max(200.0));
    egui::Modal::new(egui::Id::new("code-review-stamp-dialog")).show(ctx, |ui| {
        let what = match f.purpose {
            Purpose::Stamp => "Stamp",
            Purpose::NeedsFix => "Needs fix",
        };
        ui.heading(format!("{what}: {}", f.target.name));
        ui.monospace(&f.target.function);
        if let Some(k) = f.key() {
            if !matches!(f.step, Step::Loading | Step::PickKey | Step::SetupKey) {
                ui.weak(format!("Signing identity: {} (key {})", k.reviewer, k.key));
            }
        }
        ui.separator();
        egui::Resize::default()
            .id_salt("stamp-dialog-resize")
            .default_size(egui::vec2(640.0_f32.min(max.x), 520.0_f32.min(max.y)))
            .min_size(egui::vec2(240.0, 160.0))
            .max_size(max)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(ui, |ui| body(ui, f));
            });
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            let label = if matches!(f.step, Step::Done(_)) {
                "Close"
            } else {
                "Cancel"
            };
            if ui
                .add_enabled(!f.working(), egui::Button::new(label))
                .on_disabled_hover_text("Wait for the step running to finish")
                .clicked()
            {
                close = true;
            }
        });
    });
    close
}

/// A spinner and a line. GUI drawing code (exempt).
fn working(ui: &mut egui::Ui, text: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.spinner();
        ui.label(text);
    });
    ui.weak("This runs in the background; kovan stays responsive.");
}

fn error(ui: &mut egui::Ui, text: &str) {
    ui.colored_label(ui.visuals().error_fg_color, text);
}

/// The current step. GUI drawing code (exempt).
fn body(ui: &mut egui::Ui, f: &mut StampFlow) {
    for n in &f.notices {
        ui.label(RichText::new(n).strong());
    }
    if !f.notices.is_empty() {
        ui.add_space(4.0);
    }
    match f.step.clone() {
        Step::Loading => working(ui, "Reading your signing keys\u{2026}"),
        Step::NoRoot => {
            error(ui, "This workspace has no kovan_root.toml (its reviewer registry).");
            ui.label("Run Index fresh in the Code Map tab first, then press Stamp again.");
        }
        Step::SetupKey => setup_key(ui, f),
        Step::Generating => working(
            ui,
            "Generating your key, encrypting it with your passphrase (argon2) and registering it\u{2026}",
        ),
        Step::PickKey => {
            ui.label("Several signing keys are in your keystore. Which one is yours for this workspace?");
            for i in 0..f.keys.len() {
                let k = &f.keys[i];
                let text = format!(
                    "{} (key {}){}",
                    k.reviewer,
                    k.key,
                    if k.registered { "" } else { " \u{2014} not in kovan_root.toml yet" }
                );
                ui.radio_value(&mut f.chosen, i, text);
            }
            if ui.button("Use this key").clicked() {
                f.use_key();
            }
        }
        Step::Register => {
            if let Some(k) = f.key() {
                ui.label(format!(
                    "Your key {} (key {}) is not in this workspace's kovan_root.toml, so a stamp \
                     signed with it would not count.",
                    k.reviewer, k.key
                ));
                ui.weak(k.path.display().to_string());
            }
            if let Some(e) = &f.error {
                error(ui, e);
            }
            if ui
                .button("Register it in kovan_root.toml")
                .on_hover_text("Appends your reviewer entry or key; every existing line is kept. kovan commits nothing.")
                .clicked()
            {
                f.register_chosen();
            }
        }
        Step::Registering => working(ui, "Registering your key in kovan_root.toml\u{2026}"),
        Step::Preparing => working(ui, "Checking the function at HEAD and reading git\u{2026}"),
        Step::Refused { message, hint } => {
            ui.label("kovan cannot stamp this function now:");
            ui.add(egui::Label::new(
                RichText::new(&message)
                    .monospace()
                    .color(ui.visuals().error_fg_color),
            ));
            ui.label(&hint);
            if ui.button("Try again").clicked() {
                f.prepare();
            }
        }
        Step::Wizard => wizard(ui, f),
        Step::Signing => working(
            ui,
            "Unlocking your key (argon2, a second or two), signing and writing review.md\u{2026}",
        ),
        Step::NeedsFixForm => {
            ui.label("What needs fixing? This is written to the folder's review.md as an open needs-fix entry.");
            ui.weak("A needs-fix entry is not signed (the review.md format has no signature for it).");
            ui.add(
                egui::TextEdit::multiline(&mut f.note)
                    .desired_rows(4)
                    .desired_width(f32::INFINITY),
            );
            if let Some(e) = &f.error {
                error(ui, e);
            }
            if ui
                .add_enabled(f.can_write_needs_fix(), egui::Button::new("Write needs fix"))
                .clicked()
            {
                f.write_needs_fix();
            }
        }
        Step::WritingNeedsFix => working(ui, "Writing the needs-fix entry\u{2026}"),
        Step::Done(o) => {
            ui.label(
                RichText::new(format!(
                    "Wrote {}{}",
                    o.review_md,
                    if o.replaced {
                        " (replacing your earlier entry)"
                    } else {
                        ""
                    }
                ))
                .strong(),
            );
            ui.label("Commit it yourself (lazygit); kovan does not commit yet (#771).");
            ui.weak("A stamp counts once it is committed after the commit it certifies; until then it shows as unverified.");
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                if f.busy() {
                    ui.spinner();
                    ui.label("Refreshing the review states\u{2026}");
                } else {
                    match &f.target_state {
                        Some(s) => {
                            let kind = s.state.map(|k| k.label().to_string()).unwrap_or_default();
                            ui.label(format!("Now: {kind}"));
                            if !s.reason.is_empty() {
                                ui.weak(&s.reason);
                            }
                        }
                        None => {
                            ui.weak("Now: no recorded state");
                        }
                    }
                }
                if let Some(e) = &f.refresh_error {
                    error(ui, e);
                }
                if ui
                    .add_enabled(!f.busy(), egui::Button::new("Refresh states"))
                    .on_hover_text("Recompute every function's state from review.md (after you commit)")
                    .clicked()
                {
                    f.refresh();
                }
            });
        }
        Step::Failed(e) => error(ui, &e),
    }
}

/// The new-key form. GUI drawing code (exempt).
fn setup_key(ui: &mut egui::Ui, f: &mut StampFlow) {
    ui.strong("Set up your signing key");
    ui.label(
        "Stamps are signed with your own key, encrypted with a passphrase only you know. \
         kovan never stores the passphrase.",
    );
    ui.weak(format!("Key files live in {}", f.keystore_dir().display()));
    egui::Grid::new("stamp-key-setup")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("Reviewer id");
            ui.add(egui::TextEdit::singleline(&mut f.setup.reviewer).hint_text("github:<handle>"));
            ui.end_row();
            ui.label("Display name");
            ui.add(egui::TextEdit::singleline(&mut f.setup.name).hint_text("optional"));
            ui.end_row();
            ui.label("Passphrase");
            ui.add(egui::TextEdit::singleline(&mut f.setup.passphrase).password(true));
            ui.end_row();
            ui.label("Confirm");
            ui.add(egui::TextEdit::singleline(&mut f.setup.confirm).password(true));
            ui.end_row();
        });
    let problems = f.setup.problems();
    for p in &problems {
        ui.weak(p);
    }
    if let Some(e) = &f.error {
        error(ui, e);
    }
    if ui
        .add_enabled(problems.is_empty(), egui::Button::new("Generate"))
        .clicked()
    {
        f.generate_key();
    }
}

fn authorship(t: TestAuthorship) -> &'static str {
    match t {
        TestAuthorship::Human => "no agent trailer",
        TestAuthorship::Agent => "an agent trailer",
        TestAuthorship::Unknown => "not known",
    }
}

/// The wizard, the live gate and the sign form. GUI drawing code (exempt).
fn wizard(ui: &mut egui::Ui, f: &mut StampFlow) {
    let mut to_needs_fix = false;
    let mut sign = false;
    let can_sign = f.can_sign();
    let err = f.error.clone();
    let Some(w) = f.wizard.as_mut() else { return };
    let a = w.applicability();
    ui.weak(format!(
        "Port: {} \u{b7} physical interface: {} \u{b7} git on the reaching tests: {}{}",
        if a.is_port { "yes" } else { "no" },
        if a.physical_interface { "yes" } else { "no" },
        authorship(a.tests),
        if w.ctx.restamp {
            " \u{b7} re-stamp: starts from your previous answers"
        } else {
            ""
        }
    ));
    ui.add_space(4.0);
    questions(ui, w);
    ui.separator();
    let g = w.gate();
    let wz = ReviewWizard::embedded();
    let qtext = |k: &str| {
        wz.question(k)
            .map(|q| q.text.trim().to_string())
            .unwrap_or_else(|| k.to_string())
    };
    let otext = |q: &str, o: &str| {
        wz.question(q)
            .and_then(|q| q.option(o))
            .map(|o| o.label.trim().to_string())
            .unwrap_or_else(|| o.to_string())
    };
    ui.label(
        RichText::new(format!(
            "Rung {}: derived from the V&V answers and git, never chosen",
            g.rung.as_u8()
        ))
        .strong(),
    );
    for b in &g.blocked_by {
        let line = match b {
            GateReason::Unanswered(q) => format!("Not answered: {}", qtext(q)),
            GateReason::Answer(c) => format!(
                "Blocks the stamp: {} \u{2014} {}",
                qtext(&c.question),
                otext(&c.question, &c.option)
            ),
            GateReason::Invalid(e) => format!("Answer not accepted: {e}"),
        };
        error(ui, &line);
    }
    for c in &g.flags {
        ui.colored_label(
            ui.visuals().warn_fg_color,
            format!(
                "Needs improvement (does not block): {} \u{2014} {}",
                qtext(&c.question),
                otext(&c.question, &c.option)
            ),
        );
    }
    if !g.prompts.is_empty() {
        for c in &g.prompts {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                format!(
                    "{} \u{2014} {}",
                    qtext(&c.question),
                    otext(&c.question, &c.option)
                ),
            );
        }
        if ui.button("Mark as Needs fix instead?").clicked() {
            to_needs_fix = true;
        }
    }
    ui.add_space(6.0);
    ui.label("Comments (written under the entry in review.md, optional)");
    ui.add(
        egui::TextEdit::multiline(&mut w.comments)
            .desired_rows(2)
            .desired_width(f32::INFINITY),
    );
    ui.horizontal_wrapped(|ui| {
        ui.label("Passphrase");
        ui.add(egui::TextEdit::singleline(&mut w.passphrase).password(true));
    });
    if let Some(e) = &err {
        error(ui, e);
    }
    if ui
        .add_enabled(can_sign, egui::Button::new("Sign and write review.md"))
        .on_disabled_hover_text("Answer every question (nothing blocking) and type your passphrase")
        .clicked()
    {
        sign = true;
    }
    if to_needs_fix {
        f.switch_to_needs_fix();
    } else if sign {
        f.sign();
    }
}

/// Every applicable question: radio options, a text box for an option
/// that needs text, the source clauses folded. GUI drawing code (exempt).
fn questions(ui: &mut egui::Ui, w: &mut WizardForm) {
    for q in w.questions() {
        ui.add_space(6.0);
        ui.label(RichText::new(q.text.trim()).strong());
        for o in &q.options {
            let selected = w.options.get(&q.key) == Some(&o.key);
            if ui.radio(selected, o.label.trim()).clicked() {
                w.options.insert(q.key.clone(), o.key.clone());
            }
            if selected && o.requires_text {
                let t = w.texts.entry(q.key.clone()).or_default();
                ui.indent(("stamp-q-text", &q.key), |ui| {
                    ui.add(
                        egui::TextEdit::singleline(t)
                            .hint_text("at least 2 characters")
                            .desired_width(f32::INFINITY),
                    );
                });
            }
        }
        if !q.sources.is_empty() || q.workspace_rule.is_some() {
            egui::CollapsingHeader::new(RichText::new("Sources").small())
                .id_salt(("stamp-q-src", &q.key))
                .show(ui, |ui| {
                    if let Some(r) = &q.workspace_rule {
                        ui.small(format!("Workspace rule: {r}"));
                    }
                    for s in &q.sources {
                        let mut line = s.document.clone();
                        if let Some(sec) = &s.section {
                            line.push_str(&format!(", {sec}"));
                        }
                        if let Some(p) = &s.page {
                            line.push_str(&format!(", p. {p}"));
                        }
                        ui.small(line);
                        if let Some(qt) = &s.quote {
                            ui.small(
                                RichText::new(format!("\u{201c}{}\u{201d}", qt.trim())).italics(),
                            );
                        }
                    }
                });
        }
    }
}
