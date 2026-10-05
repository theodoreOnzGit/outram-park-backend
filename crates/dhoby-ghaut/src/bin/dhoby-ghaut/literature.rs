//! The literature pane: every prefilled value's source, beside the model.
//!
//! A citation names a kovan citekey. The pane looks for `<citekey>.pdf`
//! under a kovan root (the folder a `kovan_root.toml` lives in, or any folder
//! of PDFs) and shows the cited page, rendered by `kopitiam-pdf`, the engine
//! kovan's own reader uses. The root defaults to `$KOVAN_ROOT`, else the
//! workspace's open corpus (`crates/kovan-literature/reactor-literature`).
//! Restricted literature is never in this repository: point the pane at your
//! own kovan root to see it. A citekey the root does not hold is said to be
//! missing, plainly.

use std::path::{Path, PathBuf};

use dhoby_ghaut::workbench::recipe::Citation;
use egui::{Color32, RichText, TextureHandle, TextureOptions};

use crate::engine::{Ev, Req};

pub struct Literature {
    pub root: String,
    pub selected: Option<Citation>,
    pub pdf: Option<PathBuf>,
    pub page: usize,
    pub pages: usize,
    texture: Option<TextureHandle>,
    pub message: Option<String>,
    pub loading: bool,
}

impl Literature {
    pub fn new() -> Self {
        let root = std::env::var("KOVAN_ROOT").unwrap_or_else(|_| {
            let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../kovan-literature/reactor-literature");
            std::fs::canonicalize(&p).unwrap_or(p).display().to_string()
        });
        Self {
            root,
            selected: None,
            pdf: None,
            page: 0,
            pages: 0,
            texture: None,
            message: None,
            loading: false,
        }
    }

    /// Forget the open citation (the step changed).
    pub fn clear(&mut self) {
        self.selected = None;
        self.pdf = None;
        self.texture = None;
        self.message = None;
        self.loading = false;
    }

    pub fn on_event(&mut self, ctx: &egui::Context, ev: &Ev) -> bool {
        if let Ev::Page {
            pdf,
            page,
            pages,
            width,
            height,
            rgba,
        } = ev
        {
            if Some(pdf) != self.pdf.as_ref() {
                return false;
            }
            self.page = *page;
            self.pages = *pages;
            let ci = egui::ColorImage::from_rgba_unmultiplied([*width, *height], rgba);
            self.texture = Some(ctx.load_texture("lit-page", ci, TextureOptions::LINEAR));
            self.loading = false;
            return true;
        }
        false
    }

    /// Open a citation: find its PDF and ask for the cited page.
    pub fn open(&mut self, c: &Citation, send: &mut impl FnMut(Req)) {
        self.selected = Some(c.clone());
        self.texture = None;
        match find_pdf(Path::new(&self.root), &c.citekey) {
            Some(p) => {
                self.pdf = Some(p.clone());
                self.page = c.page.map_or(0, |p| p.saturating_sub(1) as usize);
                self.message = None;
                self.loading = true;
                send(Req::RenderPage {
                    pdf: p,
                    page: self.page,
                });
            }
            None => {
                self.pdf = None;
                self.message = Some(format!(
                    "`{}` is not under this kovan root. The maintainer supplies the corpus; \
                     point the root at a library that holds it.",
                    c.citekey
                ));
            }
        }
    }

    /// Draw the pane. Returns `true` when the reader asked to choose a new
    /// kovan root (the caller opens the folder picker).
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        cites: &[Citation],
        send: &mut impl FnMut(Req),
    ) -> bool {
        ui.label(RichText::new("Literature (kovan)").strong());
        ui.label(RichText::new(format!("Root: {}", self.root)).color(Color32::GRAY));
        let choose = ui
            .button("Choose kovan root...")
            .on_hover_text("Pick your kovan library folder")
            .clicked();
        ui.separator();
        if cites.is_empty() {
            ui.label("No values in this step carry a citation.");
        }
        for c in cites {
            let sel = self.selected.as_ref().is_some_and(|s| s == c);
            let page = c.page.map_or(String::new(), |p| format!(", p. {p}"));
            let text = format!("[src] {}: {}{}", c.field, c.citekey, page);
            if ui
                .selectable_label(sel, text)
                .on_hover_text(&c.what)
                .clicked()
            {
                self.open(c, send);
            }
        }
        ui.separator();
        if let Some(c) = &self.selected {
            ui.label(RichText::new(&c.what).italics());
        }
        if let Some(m) = &self.message {
            ui.colored_label(Color32::from_rgb(170, 90, 0), m);
        }
        if let Some(pdf) = self.pdf.clone() {
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(self.page > 0, egui::Button::new("«"))
                    .clicked()
                {
                    self.page -= 1;
                    self.loading = true;
                    send(Req::RenderPage {
                        pdf: pdf.clone(),
                        page: self.page,
                    });
                }
                ui.label(format!("page {} of {}", self.page + 1, self.pages.max(1)));
                if ui
                    .add_enabled(self.page + 1 < self.pages, egui::Button::new("»"))
                    .clicked()
                {
                    self.page += 1;
                    self.loading = true;
                    send(Req::RenderPage {
                        pdf: pdf.clone(),
                        page: self.page,
                    });
                }
                if self.loading {
                    ui.spinner();
                }
            });
            ui.small(pdf.display().to_string());
        }
        if let Some(t) = &self.texture {
            let w = ui.available_width();
            let s = t.size_vec2();
            ui.image((t.id(), egui::vec2(w, w * s.y / s.x.max(1.0))));
        }
        choose
    }
}

/// `<citekey>.pdf` (case-insensitive) anywhere under `root`, at most six
/// folders deep, skipping `.git` and `target`.
pub fn find_pdf(root: &Path, citekey: &str) -> Option<PathBuf> {
    let want = format!("{}.pdf", citekey.to_lowercase());
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_lowercase();
            if p.is_dir() {
                if depth < 6 && name != ".git" && name != "target" {
                    stack.push((p, depth + 1));
                }
            } else if name == want {
                return Some(p);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The open corpus holds the HTR-10 OTTO DEM paper; it is found by its
    /// file stem, and an absent citekey is not invented.
    #[test]
    fn citekeys_resolve_to_pdfs_in_the_open_corpus() {
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../kovan-literature/reactor-literature");
        if !root.join("README.md").exists() {
            eprintln!("reactor-literature submodule not checked out; skipping");
            return;
        }
        let p = find_pdf(&root, "putra2021-htr10-otto-dem-monte-carlo").expect("open-corpus PDF");
        assert!(p.ends_with("putra2021-htr10-otto-dem-monte-carlo.pdf"));
        assert_eq!(find_pdf(&root, "no-such-citekey-2099"), None);
    }
}
