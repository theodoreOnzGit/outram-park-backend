//! The **"Signing key" window** and the passphrase row every signing form
//! shares (GitHub #1020). Drawing only: the logic is
//! [`crate::stamping::key_panel::KeyPanel`] and the key session in
//! `kovan_common::review::signing::session`, tested there.

use egui::RichText;

use crate::stamping::key_panel::{export_file_name, status_line, KeyPanel, AUTO_LOCK_MINUTES};

/// What the window asks the host for: a file picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum KeyPick {
    /// A save picker for the chosen key's export, with a default file name.
    Export(String),
    /// A file picker for a key file to import.
    Import,
}

/// The passphrase box of a signing form, or, while the session holds the
/// key, a line saying none is needed. GUI drawing code (exempt; whether
/// the key is held is `stamping::flow::session_holds`, tested there).
pub(crate) fn passphrase_row(ui: &mut egui::Ui, unlocked: bool, passphrase: &mut String) {
    ui.horizontal_wrapped(|ui| {
        if unlocked {
            ui.label("\u{1F513} Your key is unlocked for this session: no passphrase needed.");
        } else {
            ui.label("\u{1F512} Passphrase");
            ui.add(egui::TextEdit::singleline(passphrase).password(true));
        }
    });
}

/// Draw the window; returns `(still open, a picker to open)`. GUI drawing
/// code (exempt; the logic is `KeyPanel`, tested in `key_panel_tests`).
pub(crate) fn show(ctx: &egui::Context, p: &mut KeyPanel) -> (bool, Option<KeyPick>) {
    let mut open = true;
    let mut pick = None;
    egui::Window::new("Signing key")
        .id(egui::Id::new("code-review-signing-key"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size(egui::vec2(480.0, 420.0))
        .min_size(egui::vec2(260.0, 200.0))
        .constrain(true)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                pick = body(ui, p);
            });
        });
    (open, pick)
}

/// The window's contents, also drawn inline in Code Review ▸ User
/// management (GitHub #1019: "My keys on this machine"), so the two never
/// drift. Polls the panel and applies Lock now, the auto-lock and Import;
/// returns a picker to open. GUI drawing code (exempt; the logic is
/// `KeyPanel`, tested in `key_panel_tests`).
pub(crate) fn body(ui: &mut egui::Ui, p: &mut KeyPanel) -> Option<KeyPick> {
    p.poll();
    if p.busy() {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(150));
    } else {
        // The minutes left tick down.
        ui.ctx().request_repaint_after(std::time::Duration::from_secs(20));
    }
    let mut pick = None;
    let mut lock = false;
    let mut import = false;
    let mut minutes = p.auto_lock_minutes;
    {
        let status = p.status();
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(status_line(status.as_ref())).strong());
            if status.is_some() && ui.button("Lock now").clicked() {
                lock = true;
            }
        });
        if let Some(s) = &status {
            ui.weak(format!("Fingerprint {}", s.fingerprint));
        }
        ui.weak(
            "The key is unlocked at your first signing and kept in memory only, never \
             on disk; it locks after the time below without signing, on Lock now, and \
             when kovan closes.",
        );
        ui.horizontal_wrapped(|ui| {
            ui.label("Auto-lock after");
            ui.add(egui::DragValue::new(&mut minutes).range(AUTO_LOCK_MINUTES).suffix(" min"));
        });
        ui.separator();
        ui.label(RichText::new("Your keys").strong());
        ui.weak(format!("Key files live in {}", p.keystore_dir().display()));
        if p.keys.is_empty() && !p.busy() {
            ui.weak("No key on this machine yet: import one below, or set one up with Stamp.");
        }
        for i in 0..p.keys.len() {
            let k = &p.keys[i];
            let text = format!(
                "{} (key {}){}",
                k.reviewer,
                k.key,
                if k.registered { "" } else { " \u{2014} not in kovan_root.toml" }
            );
            let fp = k.fingerprint.clone();
            ui.radio_value(&mut p.chosen, i, text);
            ui.indent(("key-fp", i), |ui| ui.monospace(fp));
        }
        if let Some(k) = p.key() {
            if ui
                .add_enabled(!p.busy(), egui::Button::new("Export\u{2026}"))
                .on_hover_text(
                    "Write a copy of this key's encrypted file where you choose, to carry \
                     to another machine yourself. It stays encrypted with your passphrase.",
                )
                .clicked()
            {
                pick = Some(KeyPick::Export(export_file_name(k)));
            }
        }
        ui.separator();
        ui.label(RichText::new("Import a key from another machine").strong());
        ui.weak(
            "The same key on every machine: nothing new to endorse. Only a key this \
             workspace's kovan_root.toml already lists is accepted, and an existing \
             different key file is never overwritten.",
        );
        ui.horizontal_wrapped(|ui| {
            if ui.add_enabled(!p.busy(), egui::Button::new("Choose key file\u{2026}")).clicked() {
                pick = Some(KeyPick::Import);
            }
            match &p.import_source {
                Some(src) => ui.monospace(src.display().to_string()),
                None => ui.weak("no file chosen"),
            };
        });
        if p.import_source.is_some() {
            ui.horizontal_wrapped(|ui| {
                ui.label("Its passphrase");
                ui.add(egui::TextEdit::singleline(&mut p.import_passphrase).password(true));
            });
            if ui
                .add_enabled(p.can_import(), egui::Button::new("Import"))
                .on_disabled_hover_text("Type the key's passphrase")
                .clicked()
            {
                import = true;
            }
        }
        if p.busy() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.weak("Working in the background; kovan stays responsive.");
            });
        }
        for n in &p.notices {
            ui.label(n);
        }
        if let Some(e) = &p.error {
            ui.colored_label(ui.visuals().error_fg_color, e);
        }
    }
    if minutes != p.auto_lock_minutes {
        p.set_auto_lock_minutes(minutes);
    }
    if lock {
        p.lock_now();
    }
    if import {
        p.import();
    }
    pick
}
