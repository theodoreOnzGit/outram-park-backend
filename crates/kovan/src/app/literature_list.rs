//! The PDF reader's literature list: every PDF in the open Kovan folder's
//! corpora, so a folder's literature can be browsed without a file dialog
//! (maintainer direction, 2026-09-22).
//!
//! What belongs here: finding the PDFs (standard, open and proprietary
//! corpora, plus any paper whose PDF lives elsewhere), marking which are
//! already papers, and drawing the list. What does not: opening a PDF or
//! activating its paper, which the app does with the path this returns.
//!
//! **Standard corpus (corrected 2026-09-30).** ~~Its group was the PDFs
//! found under the standard-corpus folder, each "not ingested yet" unless a
//! paper recorded it.~~ It is now built from the compiled metadata
//! ([`crate::corpus::LITERATURE`]): every entry is listed as ingested, with
//! its title, authors, year, topics and licence status, and is either
//! downloaded (opens from its corpus file, wherever
//! [`crate::standard_corpus::StandardCorpus`] finds it) or known but not
//! downloaded (shows its source URL). Unlisted PDFs in the standard folder
//! are still shown, as plain files.
//!
//! The list is built on demand and kept until the folder's knowledge changes
//! or the user refreshes it; walking the corpora every frame would not scale
//! to a large corpus.

use crate::corpus::{CorpusLiterature, STANDARD_CORPUS_FOLDER};
use crate::entity::EntityConfig;
use crate::root::KovanRoot;
use crate::standard_corpus::{Availability, StandardCorpus};
use eframe::egui;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One document in the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LiteratureItem {
    /// The PDF; `None` for a standard-corpus document not downloaded here.
    pub(super) path: Option<PathBuf>,
    /// The text shown: the path relative to its corpus, or a corpus
    /// document's id and title.
    pub(super) label: String,
    /// The paper this document is, if it has been ingested. A
    /// standard-corpus document always is: its notes paper's citekey, or its
    /// corpus id until one is filed.
    pub(super) citekey: Option<String>,
    /// The compiled metadata, for a standard-corpus document.
    pub(super) corpus: Option<&'static CorpusLiterature>,
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
    /// A standard-corpus document that is not downloaded was chosen: tell
    /// the user where it can be obtained.
    NotDownloaded(&'static CorpusLiterature),
    Refresh,
    /// Open the fuzzy finder ([`LiteratureFinder`]).
    Find,
}

impl LiteratureItem {
    /// How well `query` matches this item: its path in the corpus, its file
    /// name or its citekey, whichever matches best ([`crate::fuzzy`]).
    /// A standard-corpus document also matches on its corpus id, title,
    /// authors, topics and corpus file.
    pub(super) fn score(&self, query: &str) -> Option<i32> {
        let name = self.label.rsplit('/').next().unwrap_or(&self.label);
        let mut fields: Vec<&str> = vec![&self.label, name];
        fields.extend(self.citekey.as_deref());
        if let Some(lit) = self.corpus {
            fields.push(lit.id);
            fields.push(lit.title);
            fields.extend(lit.authors.iter().copied());
            fields.extend(lit.topics.iter().copied());
            fields.extend(lit.corpus_file);
        }
        fields
            .into_iter()
            .filter_map(|f| crate::fuzzy::fuzzy_score(query, f))
            .max()
    }

    /// The row text: a tick for an ingested document, "not downloaded" for a
    /// corpus document that is not here.
    pub(super) fn row_text(&self) -> String {
        match (&self.citekey, &self.path, self.corpus) {
            (_, None, Some(_)) => format!("\u{25cb} {}  (not downloaded)", self.label),
            (Some(k), _, Some(lit)) if k == lit.id => format!("\u{2713} {}", self.label),
            (Some(k), _, _) => format!("\u{2713} {}  ({k})", self.label),
            (None, _, _) => self.label.clone(),
        }
    }

    /// The tooltip: a corpus document's metadata, or the path and paper.
    pub(super) fn hover_text(&self) -> String {
        if let Some(lit) = self.corpus {
            let availability = match &self.path {
                Some(p) => Availability::Downloaded(p.clone()),
                None => Availability::NotDownloaded,
            };
            let mut text = crate::standard_corpus::describe(lit, &availability);
            if let Some(k) = self.citekey.as_deref().filter(|k| *k != lit.id) {
                text.push_str(&format!("\npaper: {k}"));
            }
            return text;
        }
        let path = self
            .path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        match &self.citekey {
            Some(k) => format!("{path}\npaper: {k}"),
            None => format!("{path}\nnot ingested yet"),
        }
    }

    /// What choosing this item asks for.
    pub(super) fn action(&self) -> Option<LiteratureAction> {
        match (&self.path, self.corpus) {
            (Some(p), _) => Some(LiteratureAction::Open(p.clone())),
            (None, Some(lit)) => Some(LiteratureAction::NotDownloaded(lit)),
            (None, None) => None,
        }
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
    /// - **Standard corpus:** every [`crate::corpus::LITERATURE`] entry,
    ///   downloaded or not (see the module doc), then any other PDF in the
    ///   folder's `kovan-standard-open-corpus/`; the rest of that repository
    ///   is someone's own open corpus.
    /// - **Open corpus:** [`crate::corpus_repos::open_corpus_pdfs`], minus
    ///   the standard folder (when the open corpus is the same repository,
    ///   as the maintainer's is, those PDFs are listed once, as standard).
    /// - **Proprietary corpus:** every PDF in it.
    /// - **Other papers:** papers whose PDF is in none of those.
    ///
    /// Each tier covers **every** repository of that tier
    /// ([`crate::corpus_tiers`], GitHub issue #458); when a tier holds more
    /// than one, each item's label starts with its repository's name.
    pub(super) fn build(root: &KovanRoot, corpus: &StandardCorpus) -> Self {
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
            path: Some(path),
            corpus: None,
        };

        // Papers holding a corpus document's notes, by corpus id.
        let corpus_notes: BTreeMap<String, String> = root
            .paper_dirs()
            .into_iter()
            .filter_map(|dir| {
                let config = EntityConfig::load(&dir).ok()?;
                Some((config.source?.corpus?, config.id))
            })
            .collect();
        let mut standard_items: Vec<LiteratureItem> = crate::corpus::LITERATURE
            .iter()
            .map(|lit| {
                let path = corpus.locate(lit);
                let owner = path
                    .as_ref()
                    .and_then(|p| p.canonicalize().ok())
                    .and_then(|c| owners.get(&c).cloned());
                LiteratureItem {
                    label: format!("{}: {}", lit.id, lit.title),
                    citekey: Some(
                        corpus_notes
                            .get(lit.id)
                            .cloned()
                            .or(owner)
                            .unwrap_or_else(|| lit.id.to_string()),
                    ),
                    path,
                    corpus: Some(lit),
                }
            })
            .collect();
        use crate::corpus_tiers::Tier;
        let repos = root.corpus_repos();
        // The repositories of one tier, each checkout once, with the label
        // prefix its items get (its name, when the tier has several).
        let tier_dirs = |tier: Tier| -> Vec<(PathBuf, String)> {
            let mut dirs: Vec<(PathBuf, String)> = Vec::new();
            for r in repos.iter().filter(|r| r.tier == tier) {
                if !dirs.iter().any(|(d, _)| d == &r.dir) {
                    dirs.push((r.dir.clone(), r.name.clone()));
                }
            }
            let several = dirs.len() > 1;
            dirs.into_iter()
                .map(|(d, n)| {
                    (
                        d,
                        if several {
                            format!("{n}: ")
                        } else {
                            String::new()
                        },
                    )
                })
                .collect()
        };
        let prefixed = |mut i: LiteratureItem, prefix: &str| {
            i.label = format!("{prefix}{}", i.label);
            i
        };
        let known: Vec<PathBuf> = standard_items
            .iter()
            .filter_map(|i| i.path.as_ref()?.canonicalize().ok())
            .collect();
        let mut standard_dirs = tier_dirs(Tier::Standard);
        if standard_dirs.is_empty() {
            standard_dirs.push((root.standard_corpus_dir(), String::new()));
        }
        for (standard_dir, prefix) in &standard_dirs {
            let standard: Vec<PathBuf> = pdfs_under(&standard_dir.join(STANDARD_CORPUS_FOLDER))
                .into_iter()
                .filter(|p| p.canonicalize().map_or(true, |c| !known.contains(&c)))
                .collect();
            standard_items.extend(
                standard
                    .into_iter()
                    .map(|p| prefixed(item(p, standard_dir), prefix)),
            );
        }
        let mut open_items = Vec::new();
        for (open_dir, prefix) in tier_dirs(Tier::Open) {
            let open: Vec<PathBuf> = crate::corpus_repos::open_corpus_pdfs(&open_dir)
                .into_iter()
                .filter(|p| {
                    !p.strip_prefix(&open_dir)
                        .is_ok_and(|rel| rel.starts_with(STANDARD_CORPUS_FOLDER))
                })
                .collect();
            open_items.extend(
                open.into_iter()
                    .map(|p| prefixed(item(p, &open_dir), &prefix)),
            );
        }
        let mut proprietary_items = Vec::new();
        for (restricted_dir, prefix) in tier_dirs(Tier::Proprietary) {
            proprietary_items.extend(
                pdfs_under(&restricted_dir)
                    .into_iter()
                    .map(|p| prefixed(item(p, &restricted_dir), &prefix)),
            );
        }

        let mut groups = vec![
            LiteratureGroup {
                title: "Standard corpus",
                items: standard_items,
            },
            LiteratureGroup {
                title: "Open corpus",
                items: open_items,
            },
            LiteratureGroup {
                title: "Proprietary corpus",
                items: proprietary_items,
            },
        ];
        let listed: Vec<PathBuf> = groups
            .iter()
            .flat_map(|g| {
                g.items
                    .iter()
                    .filter_map(|i| i.path.as_ref()?.canonicalize().ok())
            })
            .collect();
        let others: Vec<LiteratureItem> = owners
            .iter()
            .filter(|(pdf, _)| !listed.contains(pdf))
            .map(|(pdf, citekey)| LiteratureItem {
                path: Some(pdf.clone()),
                label: citekey.clone(),
                citekey: Some(citekey.clone()),
                corpus: None,
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
                                let selected = current.is_some() && current == item.path.as_deref();
                                if ui
                                    .selectable_label(selected, item.row_text())
                                    .on_hover_text(item.hover_text())
                                    .clicked()
                                {
                                    action = item.action();
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

    /// Draw the finder, if open, over `list`. Returns what the chosen item
    /// asks for ([`LiteratureItem::action`]).
    pub(super) fn ui(
        &mut self,
        ctx: &egui::Context,
        list: &LiteratureList,
    ) -> Option<LiteratureAction> {
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
            chosen = results.get(self.selected).and_then(|i| i.action());
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
                        .hint_text("type part of a file name, path, citekey, title or author")
                        .desired_width(f32::INFINITY),
                );
                if self.focus {
                    edit.request_focus();
                    self.focus = false;
                }
                if edit.changed() {
                    self.selected = 0;
                }
                ui.weak(format!("{} of {} documents", results.len(), items.len()));
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt("literature_finder_results")
                    .max_height(420.0)
                    .show(ui, |ui| {
                        for (n, item) in results.iter().enumerate() {
                            let row = ui
                                .selectable_label(n == self.selected, item.row_text())
                                .on_hover_text(item.hover_text());
                            if n == self.selected && (up || down) {
                                row.scroll_to_me(None);
                            }
                            if row.clicked() {
                                chosen = item.action();
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

    /// The corpus searched only inside `root` (never this machine's real
    /// application-data clone).
    fn folder_corpus(root: &KovanRoot) -> StandardCorpus {
        StandardCorpus::with_checkouts(vec![root.standard_corpus_dir(), root.open_corpus_dir()])
    }

    /// Each corpus is listed, the standard folder once even when the open
    /// corpus is the same repository, and `.git` is never walked.
    #[test]
    fn every_corpus_is_listed_once() {
        let tmp = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(tmp.path(), RootConfig::new("lib", "Lib"), false).unwrap();
        touch(
            &root
                .standard_corpus_dir()
                .join("kovan-standard-open-corpus/nrc/a.pdf"),
        );
        touch(
            &root
                .standard_corpus_dir()
                .join("theodore-open-corpus/b.pdf"),
        );
        touch(
            &root
                .open_corpus_dir()
                .join("kovan-standard-open-corpus/nrc/a.pdf"),
        );
        touch(
            &root
                .open_corpus_dir()
                .join("theodore-open-corpus/cc-by/b.pdf"),
        );
        touch(&root.restricted_sources_dir().join("papers/c.pdf"));
        touch(&root.restricted_sources_dir().join(".git/objects/x.pdf"));
        let list = LiteratureList::build(&root, &folder_corpus(&root));
        // The standard group lists every compiled entry (none downloaded in
        // this folder), then the stray PDF in the standard folder.
        let n = crate::corpus::LITERATURE.len();
        let labels: Vec<(&str, Vec<&str>)> = list
            .groups
            .iter()
            .map(|g| {
                let items = g.items.iter().filter(|i| i.corpus.is_none());
                (g.title, items.map(|i| i.label.as_str()).collect())
            })
            .collect();
        assert_eq!(
            labels,
            vec![
                (
                    "Standard corpus",
                    vec!["kovan-standard-open-corpus/nrc/a.pdf"]
                ),
                ("Open corpus", vec!["theodore-open-corpus/cc-by/b.pdf"]),
                ("Proprietary corpus", vec!["papers/c.pdf"]),
            ]
        );
        assert_eq!(list.groups[0].items.len(), n + 1);
        assert!(list
            .groups
            .iter()
            .flat_map(|g| &g.items)
            .filter(|i| i.corpus.is_none())
            .all(|i| i.citekey.is_none()));
    }

    /// The bug (2026-09-30): a pulled standard-corpus document is listed as
    /// ingested, with its corpus metadata, opening from the corpus file; an
    /// entry not pulled is listed as known, not downloaded, with its URL.
    #[test]
    fn standard_corpus_documents_are_listed_as_ingested() {
        let tmp = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(tmp.path(), RootConfig::new("lib", "Lib"), false).unwrap();
        let wash = crate::standard_corpus::entry("wash-1400").unwrap();
        // The maintainer's layout: the corpus repository mounted as the open
        // corpus, not at literature/standard-corpus/.
        let pdf = root.open_corpus_dir().join(wash.corpus_file.unwrap());
        touch(&pdf);
        let list = LiteratureList::build(&root, &folder_corpus(&root));
        let standard = &list.groups[0];
        assert_eq!(standard.title, "Standard corpus");
        let item = standard
            .items
            .iter()
            .find(|i| i.corpus.map(|l| l.id) == Some("wash-1400"))
            .unwrap();
        assert_eq!(item.citekey.as_deref(), Some("wash-1400"));
        assert_eq!(item.path.as_deref(), Some(pdf.as_path()));
        assert!(
            item.row_text().starts_with('\u{2713}'),
            "{}",
            item.row_text()
        );
        assert!(item.hover_text().contains("Reactor Safety Study"));
        assert!(item.hover_text().contains("licence: public domain"));
        assert!(matches!(item.action(), Some(LiteratureAction::Open(p)) if p == pdf));
        assert!(
            item.score("reactor safety").is_some(),
            "title is searchable"
        );
        assert!(item.score("pra").is_some(), "topics are searchable");
        // Not listed a second time under the open corpus.
        assert!(list.groups[1].items.is_empty());

        let absent = standard
            .items
            .iter()
            .find(|i| i.corpus.map(|l| l.id) == Some("nureg-2201"))
            .unwrap();
        assert!(absent.path.is_none());
        assert!(absent.row_text().contains("not downloaded"));
        assert!(
            absent.hover_text().contains("https://"),
            "{}",
            absent.hover_text()
        );
        assert!(matches!(
            absent.action(),
            Some(LiteratureAction::NotDownloaded(l)) if l.id == "nureg-2201"
        ));

        // Once its notes paper exists, the item names that paper.
        crate::standard_corpus::ensure_paper(&root, &folder_corpus(&root), wash).unwrap();
        let list = LiteratureList::build(&root, &folder_corpus(&root));
        let item = list.groups[0]
            .items
            .iter()
            .find(|i| i.corpus.map(|l| l.id) == Some("wash-1400"))
            .unwrap();
        assert_eq!(item.citekey.as_deref(), Some("wash-1400"));
        assert!(list.groups.iter().all(|g| g.title != "Other papers"));
    }

    /// The finder ranks by fuzzy score: a file-name substring first, a
    /// scattered subsequence after it, non-matches dropped.
    #[test]
    fn the_finder_ranks_fuzzy_matches() {
        let item = |label: &str, citekey: Option<&str>| LiteratureItem {
            path: Some(PathBuf::from(label)),
            label: label.to_string(),
            citekey: citekey.map(str::to_string),
            corpus: None,
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
