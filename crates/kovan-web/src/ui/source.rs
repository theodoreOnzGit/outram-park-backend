//! The source panel: the selected function's lines, fetched on demand at
//! the commit the site was built from (maintainer, 2026-10-06: source text
//! is not copied into Pages), beside the canvas, with a pop-out to a new
//! tab on the function's deep link; a full-screen sheet with "< Back" on a
//! phone.
//!
//! Right-click (long-press on touch) an identifier for the LSP-style menu:
//! Go to definition, Go to type definition, Find references, Go to
//! implementations, Callers / Callees. Only Callers / Callees (and "go to"
//! a called function by name) are answered today, from the call graph; the
//! others read the per-file `links` slot of the crate's data
//! (`kovan_common::call_graph::split::SourceFile::links`), which is empty
//! until the link index (#745) fills it, so they show greyed out with
//! "needs the link index (#745)". Filling them is a data change, not a UI
//! change. Read-only: no rename, no code actions.

use egui::{Color32, RichText, Sense};
use kovan_common::call_graph::split::{CrateSlice, LinkKind};

use super::{CodeReview, Snap};
use crate::data::Load;
use crate::model::{self, file_of, DeepLink};

/// Split a line into `(text, is_identifier, start column in chars)`.
fn tokens(line: &str) -> Vec<(String, bool, usize)> {
    let mut out: Vec<(String, bool, usize)> = Vec::new();
    for (col, ch) in line.chars().enumerate() {
        let ident = ch == '_' || ch.is_alphanumeric();
        match out.last_mut() {
            Some((t, i, _)) if *i == ident => t.push(ch),
            _ => out.push((ch.to_string(), ident, col)),
        }
    }
    // A run of digits is not an identifier.
    for t in &mut out {
        if t.1 && t.0.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            t.1 = false;
        }
    }
    out
}

/// The link at `(line, col)` of `file` in `slice`'s reserved links slot.
fn link_at(slice: &CrateSlice, file: &str, line: u32, col: u32, kind: LinkKind) -> Option<String> {
    let f = slice.files.iter().find(|f| f.file == file)?;
    f.links.iter().find(|l| l.line == line && l.col_start <= col && col < l.col_end && l.kind == kind).map(|l| l.target.clone())
}

impl CodeReview {
    pub(crate) fn source_panel(&mut self, ui: &mut egui::Ui, snap: &Snap, sheet: bool) {
        let Some(id) = self.source.clone() else { return };
        let file = file_of(&id).to_string();
        let slice = snap.slice_of(&id).cloned();
        let found = slice.as_ref().and_then(|s| model::find_function(&s.krate, &id).map(|(m, f, _)| (m.clone(), f.clone())));
        ui.horizontal_wrapped(|ui| {
            if sheet && ui.add(egui::Button::new(RichText::new("< Back").size(16.0)).min_size(egui::vec2(72.0, 36.0))).clicked() {
                self.source = None;
                self.sync_hash();
            }
            ui.label(RichText::new(model::short_name(&id)).monospace().strong());
            if ui.button("Pop out").on_hover_text("Open this function's source in a new tab (a link you can share)").clicked() {
                let url = crate::platform::url_with_hash(&DeepLink::Function { id: id.clone(), source: true }.to_hash());
                ui.ctx().open_url(egui::OpenUrl::new_tab(url));
            }
            if let (Some(b), Some((_, f))) = (&snap.build, &found) {
                let url = format!("https://github.com/{}/blob/{}/{}#L{}-L{}", b.repo, b.commit, file, f.start_line, f.end_line);
                ui.add(egui::Hyperlink::from_label_and_url("GitHub", url).open_in_new_tab(true));
            }
            // The ported-from counterpart (#746), only when the header gives
            // a linkable repository and commit.
            if let Some(url) = found.as_ref().and_then(|(m, _)| m.upstream.as_ref()).and_then(|u| u.url.clone()) {
                ui.add(egui::Hyperlink::from_label_and_url("Upstream", url).open_in_new_tab(true)).on_hover_text("The upstream file this one was ported from, at the recorded commit");
            }
            if !sheet && ui.button("×").on_hover_text("Close the source panel").clicked() {
                self.source = None;
                self.sync_hash();
            }
        });
        let Some((module, f)) = found else {
            ui.weak("Loading the function's data…");
            return;
        };
        ui.label(RichText::new(format!("{file}:{}-{}", f.start_line, f.end_line)).small().weak());
        // Pages that cite this function (#746): code walks and concept tags.
        if !f.cited_by.is_empty() {
            let base = if self.store.api_url().is_some() { "../".to_string() } else { slice.as_ref().and_then(|s| s.site_base.clone()).unwrap_or_default() };
            ui.collapsing(format!("Cited by {} page{}", f.cited_by.len(), if f.cited_by.len() == 1 { "" } else { "s" }), |ui| {
                for c in &f.cited_by {
                    match &c.site {
                        Some(site) => {
                            let anchor = if c.anchor.is_empty() || site.contains('#') { String::new() } else { format!("#{}", c.anchor) };
                            ui.add(egui::Hyperlink::from_label_and_url(format!("{} (line {})", c.page, c.line), format!("{base}{site}{anchor}")).open_in_new_tab(true));
                        }
                        None => {
                            ui.weak(format!("{} (line {}, not on the site)", c.page, c.line));
                        }
                    }
                }
            });
        }
        // The file's recent history (#746).
        if !module.history.is_empty() {
            let repo = snap.build.as_ref().map(|b| b.repo.clone()).unwrap_or_else(|| crate::data::DEFAULT_REPO.to_string());
            ui.collapsing(format!("File history ({} newest)", module.history.len()), |ui| {
                for c in &module.history {
                    ui.horizontal_wrapped(|ui| {
                        let short: String = c.sha.chars().take(10).collect();
                        ui.add(egui::Hyperlink::from_label_and_url(RichText::new(short).monospace(), format!("https://github.com/{repo}/commit/{}", c.sha)).open_in_new_tab(true));
                        ui.weak(c.date.chars().take(10).collect::<String>());
                        ui.label(RichText::new(&c.subject).small());
                    });
                }
            });
        }
        ui.separator();
        let text = match snap.files.get(&file) {
            Some(Load::Ready(t)) => t.clone(),
            Some(Load::Failed(e)) => {
                ui.colored_label(Color32::from_rgb(255, 130, 130), format!("Could not fetch the source: {e}"));
                return;
            }
            _ => {
                let at = snap.build.as_ref().map(|b| b.commit.chars().take(10).collect::<String>()).unwrap_or_default();
                ui.weak(format!("Fetching {file} at {at}…"));
                return;
            }
        };
        let slice = slice.expect("found implies a slice");
        let map = snap.map.clone();
        let facts = snap.facts();
        let callees: Vec<String> = map.as_ref().map(|m| facts.callees(m, &slice, &id)).unwrap_or_default();
        let callers: Vec<String> = facts.callers(&slice, &id);
        let mut goto: Option<String> = None;
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 1.0);
            let first = f.start_line as usize;
            for (k, line) in text.lines().enumerate().skip(first.saturating_sub(1)).take((f.end_line - f.start_line + 1) as usize) {
                let n = k as u32 + 1;
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{n:>5}  ")).monospace().weak());
                    for (t, ident, col) in tokens(line) {
                        if !ident {
                            ui.label(RichText::new(t).monospace());
                            continue;
                        }
                        let named: Vec<&String> = callees.iter().filter(|c| model::short_name(c).rsplit("::").next() == Some(t.as_str())).collect();
                        let rt = if named.is_empty() { RichText::new(&t).monospace() } else { RichText::new(&t).monospace().color(Color32::from_rgb(130, 200, 255)) };
                        let mut resp = ui.add(egui::Label::new(rt).sense(Sense::click()));
                        if let Some(c) = named.first() {
                            let sig = map.as_ref().and_then(|_| snap.slice_of(c)).and_then(|s| model::find_function(&s.krate, c).map(|(_, f, _)| format!("{}\n{}", f.signature, f.doc)));
                            resp = resp.on_hover_text(sig.unwrap_or_else(|| (*c).clone()));
                            if resp.clicked() {
                                goto = Some((*c).clone());
                            }
                        }
                        let def = link_at(&slice, &file, n, col as u32, LinkKind::Definition);
                        resp.context_menu(|ui| {
                            let need = "needs the link index (#745)";
                            match &def {
                                Some(target) => {
                                    if ui.button("Go to definition").clicked() {
                                        goto = Some(target.clone());
                                        ui.close();
                                    }
                                }
                                None => {
                                    ui.add_enabled(false, egui::Button::new("Go to definition")).on_disabled_hover_text(need);
                                }
                            }
                            for label in ["Go to type definition", "Find references", "Go to implementations"] {
                                ui.add_enabled(false, egui::Button::new(label)).on_disabled_hover_text(need);
                            }
                            ui.separator();
                            for c in &named {
                                if ui.button(format!("Go to {}", model::short_name(c))).clicked() {
                                    goto = Some((*c).clone());
                                    ui.close();
                                }
                            }
                            ui.menu_button(format!("Callees of {} ({})", f.name, callees.len()), |ui| {
                                for c in &callees {
                                    if ui.button(model::short_name(c)).on_hover_text(c).clicked() {
                                        goto = Some(c.clone());
                                        ui.close();
                                    }
                                }
                            });
                            ui.menu_button(format!("Callers of {} ({})", f.name, callers.len()), |ui| {
                                for c in &callers {
                                    if ui.button(model::short_name(c)).on_hover_text(c).clicked() {
                                        goto = Some(c.clone());
                                        ui.close();
                                    }
                                }
                            });
                        });
                    }
                });
            }
        });
        if let (Some(target), Some(map)) = (goto, map) {
            self.open_function(&target, false, &map);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::tokens;

    #[test]
    fn identifiers_are_split_with_their_columns() {
        let t = tokens("    let x2 = step(3);");
        let idents: Vec<(&str, usize)> = t.iter().filter(|t| t.1).map(|t| (t.0.as_str(), t.2)).collect();
        assert_eq!(idents, vec![("let", 4), ("x2", 8), ("step", 13)]);
    }
}
