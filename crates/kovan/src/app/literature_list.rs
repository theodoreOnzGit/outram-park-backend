//! The PDF reader's literature list: every PDF in the open Kovan folder's
//! corpora, so a folder's literature can be browsed without a file dialog
//! (maintainer direction, 2026-09-22).
//!
//! What belongs here: finding the PDFs (standard, open and proprietary
//! corpora, plus any paper whose PDF lives elsewhere), marking which are
//! already papers, and drawing the list. What does not: opening a PDF or
//! activating its paper, which the app does with the path this returns.
//!
//! The list is built on demand and kept until the folder's knowledge changes
//! or the user refreshes it; walking the corpora every frame would not scale
//! to a large corpus.

use crate::corpus::STANDARD_CORPUS_FOLDER;
use crate::entity::EntityConfig;
use crate::root::KovanRoot;
use eframe::egui;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One PDF in the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LiteratureItem {
    pub(super) path: PathBuf,
    /// The path shown: relative to its corpus.
    pub(super) label: String,
    /// The paper this PDF belongs to, if it has been ingested.
    pub(super) citekey: Option<String>,
}

/// The PDFs of one corpus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LiteratureGroup {
    pub(super) title: &'static str,
    pub(super) items: Vec<LiteratureItem>,
}

/// What the user asked for this frame.
pub(super) enum LiteratureAction {
    Open(PathBuf),
    Refresh,
    /// Open the fuzzy finder ([`LiteratureFinder`]).
    Find,
}

impl LiteratureItem {
    /// How well `query` matches this item: its path in the corpus, its file
    /// name or its citekey, whichever matches best ([`crate::fuzzy`]).
    pub(super) fn score(&self, query: &str) -> Option<i32> {
        let name = self.label.rsplit('/').next().unwrap_or(&self.label);
        [
            crate::fuzzy::fuzzy_score(query, &self.label),
            crate::fuzzy::fuzzy_score(query, name),
            self.citekey
                .as_deref()
                .and_then(|k| crate::fuzzy::fuzzy_score(query, k)),
        ]
        .into_iter()
        .flatten()
        .max()
    }
}

/// The list for one Kovan folder.
pub(super) struct LiteratureList {
    pub(super) root: PathBuf,
    pub(super) groups: Vec<LiteratureGroup>,
    filter: String,
}

/// Every PDF under `dir`, recursively, skipping `.git`, sorted.
fn pdfs_under(dir: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let path = e.path();
            if path.is_dir() {
                if e.file_name() != ".git" {
                    walk(&path, out);
                }
            } else if path
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("pdf"))
            {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, &mut out);
    out.sort();
    out
}

impl LiteratureList {
    /// Build the list for `root`.
    ///
    /// - **Standard corpus:** its `kovan-standard-open-corpus/` only; the
    ///   rest of that repository is someone's own open corpus.
    /// - **Open corpus:** [`crate::corpus_repos::open_corpus_pdfs`], minus
    ///   the standard folder (when the open corpus is the same repository,
    ///   as the maintainer's is, those PDFs are listed once, as standard).
    /// - **Proprietary corpus:** every PDF in it.
    /// - **Other papers:** papers whose PDF is in none of those.
    pub(super) fn build(root: &KovanRoot) -> Self {
        let owners: BTreeMap<PathBuf, String> = root
            .paper_dirs()
            .into_iter()
            .filter_map(|dir| {
                let config = EntityConfig::load(&dir).ok()?;
                let pdf = dir.join(config.source?.pdf?).canonicalize().ok()?;
                Some((pdf, config.id))
            })
            .collect();
        let item = |path: PathBuf, base: &Path| LiteratureItem {
            label: path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/"),
            citekey: path
                .canonicalize()
                .ok()
                .and_then(|c| owners.get(&c).cloned()),
            path,
        };

        let standard_dir = root.standard_corpus_dir();
        let standard = pdfs_under(&standard_dir.join(STANDARD_CORPUS_FOLDER));
        let open_dir = root.open_corpus_dir();
        let open: Vec<PathBuf> = crate::corpus_repos::open_corpus_pdfs(&open_dir)
            .into_iter()
            .filter(|p| {
                !p.strip_prefix(&open_dir)
                    .is_ok_and(|rel| rel.starts_with(STANDARD_CORPUS_FOLDER))
            })
            .collect();
        let restricted_dir = root.restricted_sources_dir();
        let proprietary = pdfs_under(&restricted_dir);

        let mut groups = vec![
            LiteratureGroup {
                title: "Standard corpus",
                items: standard
                    .into_iter()
                    .map(|p| item(p, &standard_dir))
                    .collect(),
            },
            LiteratureGroup {
                title: "Open corpus",
                items: open.into_iter().map(|p| item(p, &open_dir)).collect(),
            },
            LiteratureGroup {
                title: "Proprietary corpus",
                items: proprietary
                    .into_iter()
                    .map(|p| item(p, &restricted_dir))
                    .collect(),
            },
        ];
        let listed: Vec<PathBuf> = groups
            .iter()
            .flat_map(|g| g.items.iter().filter_map(|i| i.path.canonicalize().ok()))
            .collect();
        let others: Vec<LiteratureItem> = owners
            .iter()
            .filter(|(pdf, _)| !listed.contains(pdf))
            .map(|(pdf, citekey)| LiteratureItem {
                path: pdf.clone(),
                label: citekey.clone(),
                citekey: Some(citekey.clone()),
            })
            .collect();
        if !others.is_empty() {
            groups.push(LiteratureGroup {
                title: "Other papers",
                items: others,
            });
        }
        Self {
            root: root.path().to_path_buf(),
            groups,
            filter: String::new(),
        }
    }

    /// Draw the list. `current` is the document open in the reader, which
    /// is highlighted.
    pub(super) fn ui(
        &mut self,
        ui: &mut egui::Ui,
        current: Option<&Path>,
    ) -> Option<LiteratureAction> {
        let mut action = None;
        ui.horizontal(|ui| {
            ui.strong("Literature");
            if ui
                .small_button("\u{1f50d}")
                .on_hover_text("Find a PDF (Ctrl+P)")
                .clicked()
            {
                action = Some(LiteratureAction::Find);
            }
            if ui
                .small_button("\u{21bb}")
                .on_hover_text("Look for new PDFs in this folder's corpora")
                .clicked()
            {
                action = Some(LiteratureAction::Refresh);
            }
        });
        ui.add(
            egui::TextEdit::singleline(&mut self.filter)
                .hint_text("filter")
                .desired_width(f32::INFINITY),
        );
        let needle = self.filter.trim().to_string();
        egui::ScrollArea::vertical()
            .id_salt("pdf_literature_list")
            .show(ui, |ui| {
                for group in &self.groups {
                    let shown: Vec<&LiteratureItem> = group
                        .items
                        .iter()
                        .filter(|i| i.score(&needle).is_some())
                        .collect();
                    egui::CollapsingHeader::new(format!("{} ({})", group.title, shown.len()))
                        .id_salt(group.title)
                        .default_open(true)
                        .show(ui, |ui| {
                            if shown.is_empty() {
                                ui.weak("none");
                            }
                            for item in shown {
                                let text = match &item.citekey {
                                    Some(k) => format!("\u{2713} {}  ({k})", item.label),
                                    None => item.label.clone(),
                                };
                                let selected = current == Some(item.path.as_path());
                                let hover = match &item.citekey {
                                    Some(k) => format!("{}\npaper: {k}", item.path.display()),
                                    None => format!("{}\nnot ingested yet", item.path.display()),
                                };
                                if ui.selectable_label(selected, text).on_hover_text(hover).clicked()
                                {
                                    action = Some(LiteratureAction::Open(item.path.clone()));
                                }
                            }
                        });
                }
            });
        action
    }
}

/// How many matches the finder shows.
const FINDER_RESULTS: usize = 40;

/// The literature fuzzy finder: a search box over every PDF in the list,
/// opened with Ctrl+P. Type to rank, Up/Down to choose, Enter to open,
/// Escape to close.
#[derive(Default)]
pub(super) struct LiteratureFinder {
    pub(super) open: bool,
    query: String,
    selected: usize,
    focus: bool,
}

/// `items` ranked by how well they match `query`, best first, at most
/// [`FINDER_RESULTS`]. Ties keep the list's order.
pub(super) fn rank<'a>(items: &[&'a LiteratureItem], query: &str) -> Vec<&'a LiteratureItem> {
    let mut scored: Vec<(i32, usize, &LiteratureItem)> = items
        .iter()
        .enumerate()
        .filter_map(|(n, i)| i.score(query).map(|s| (s, n, *i)))
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored
        .into_iter()
        .take(FINDER_RESULTS)
        .map(|(_, _, i)| i)
        .collect()
}

impl LiteratureFinder {
    /// Open the finder with an empty query and the text box focused.
    pub(super) fn show(&mut self) {
        self.open = true;
        self.query.clear();
        self.selected = 0;
        self.focus = true;
    }

    /// Draw the finder, if open, over `list`. Returns the PDF chosen.
    pub(super) fn ui(&mut self, ctx: &egui::Context, list: &LiteratureList) -> Option<PathBuf> {
        if !self.open {
            return None;
        }
        let items: Vec<&LiteratureItem> = list.groups.iter().flat_map(|g| &g.items).collect();
        let results = rank(&items, &self.query);
        let (up, down, enter, escape) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Escape),
            )
        });
        if escape {
            self.open = false;
            return None;
        }
        if down && self.selected + 1 < results.len() {
            self.selected += 1;
        }
        if up {
            self.selected = self.selected.saturating_sub(1);
        }
        self.selected = self.selected.min(results.len().saturating_sub(1));
        let mut chosen = None;
        if enter {
            chosen = results.get(self.selected).map(|i| i.path.clone());
        }
        let mut open = self.open;
        egui::Window::new("Find literature")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(560.0)
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 60.0))
            .show(ctx, |ui| {
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .hint_text("type part of a file name, path or citekey")
                        .desired_width(f32::INFINITY),
                );
                if self.focus {
                    edit.request_focus();
                    self.focus = false;
                }
                if edit.changed() {
                    self.selected = 0;
                }
                ui.weak(format!("{} of {} PDFs", results.len(), items.len()));
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt("literature_finder_results")
                    .max_height(420.0)
                    .show(ui, |ui| {
                        for (n, item) in results.iter().enumerate() {
                            let text = match &item.citekey {
                                Some(k) => format!("\u{2713} {}  ({k})", item.label),
                                None => item.label.clone(),
                            };
                            let row = ui.selectable_label(n == self.selected, text);
                            if n == self.selected && (up || down) {
                                row.scroll_to_me(None);
                            }
                            if row.clicked() {
                                chosen = Some(item.path.clone());
                            }
                        }
                    });
            });
        self.open = open && chosen.is_none();
        chosen
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::root::RootConfig;

    fn touch(p: &Path) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"pdf").unwrap();
    }

    /// Each corpus is listed, the standard folder once even when the open
    /// corpus is the same repository, and `.git` is never walked.
    #[test]
    fn every_corpus_is_listed_once() {
        let tmp = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(tmp.path(), RootConfig::new("lib", "Lib"), false).unwrap();
        touch(&root.standard_corpus_dir().join("kovan-standard-open-corpus/nrc/a.pdf"));
        touch(&root.standard_corpus_dir().join("theodore-open-corpus/b.pdf"));
        touch(&root.open_corpus_dir().join("kovan-standard-open-corpus/nrc/a.pdf"));
        touch(&root.open_corpus_dir().join("theodore-open-corpus/cc-by/b.pdf"));
        touch(&root.restricted_sources_dir().join("papers/c.pdf"));
        touch(&root.restricted_sources_dir().join(".git/objects/x.pdf"));
        let list = LiteratureList::build(&root);
        let labels: Vec<(&str, Vec<&str>)> = list
            .groups
            .iter()
            .map(|g| (g.title, g.items.iter().map(|i| i.label.as_str()).collect()))
            .collect();
        assert_eq!(
            labels,
            vec![
                ("Standard corpus", vec!["kovan-standard-open-corpus/nrc/a.pdf"]),
                ("Open corpus", vec!["theodore-open-corpus/cc-by/b.pdf"]),
                ("Proprietary corpus", vec!["papers/c.pdf"]),
            ]
        );
        assert!(list.groups.iter().flat_map(|g| &g.items).all(|i| i.citekey.is_none()));
    }

    /// The finder ranks by fuzzy score: a file-name substring first, a
    /// scattered subsequence after it, non-matches dropped.
    #[test]
    fn the_finder_ranks_fuzzy_matches() {
        let item = |label: &str, citekey: Option<&str>| LiteratureItem {
            path: PathBuf::from(label),
            label: label.to_string(),
            citekey: citekey.map(str::to_string),
        };
        let a = item("theodore-open-corpus/cc-by/she2021pangu.pdf", None);
        let b = item("theodore-open-corpus/cc-by/putra2021-htr10-otto.pdf", None);
        let c = item("papers/x.pdf", Some("pangu2020"));
        let items = vec![&a, &b, &c];
        let ranked: Vec<&str> = rank(&items, "pangu")
            .iter()
            .map(|i| i.label.as_str())
            .collect();
        assert_eq!(ranked.len(), 2);
        assert!(ranked.contains(&"papers/x.pdf"), "citekey matches too");
        assert_eq!(rank(&items, "htrotto")[0].label, b.label);
        assert_eq!(rank(&items, "").len(), 3);
    }
}
