//! The first-run setup dialog (GitHub issue #255).
//!
//! What belongs here: the window that asks, on first launch, for the Kovan
//! folder and then, optionally, the user's three GitHub repositories: their
//! own **Kovan repository** (the folder itself), **open corpus** and
//! **proprietary corpus**; and the marker that remembers first run is over.
//! Kovan's standard corpus is not asked for: every folder gets it. The dialog
//! is **skippable** (maintainer decision, 2026-09-22): the built-in
//! nuclear-engineering map shows with or without a folder (epic #247), and the
//! top bar's "⚙ Setup" and the home page's "Set up repositories…" reopen it.
//!
//! What does not belong here: creating the folder or the repositories. The
//! dialog only returns a [`SetupRequest`]; the app acts on it with
//! [`crate::root::KovanRoot`] and [`crate::corpus_repos`], so none of the Git
//! or filesystem logic lives in GUI code.

use crate::root::{CorporaConfig, KovanRoot};
use eframe::egui;
use std::path::{Path, PathBuf};

/// What the user asked the dialog to do.
pub(super) enum SetupRequest {
    /// Open the folder picker for the Kovan folder field.
    Browse,
    /// Set up with these answers. `folder` is required; any remote may be
    /// absent, in which case that corpus is initialised as a local repository
    /// (or, for `library_remote`, the folder keeps no remote).
    Finish {
        folder: PathBuf,
        /// The user's own Kovan repository (the folder's `origin`).
        library_remote: Option<String>,
        open_remote: Option<String>,
        proprietary_remote: Option<String>,
    },
    /// Close without setting anything up; first run counts as done.
    Skip,
}

/// The dialog's state, kept between frames.
#[derive(Default)]
pub(super) struct SetupDialog {
    pub(super) open: bool,
    /// The Kovan folder path, typed or picked.
    pub(super) folder: String,
    library_remote: String,
    open_remote: String,
    proprietary_remote: String,
    message: String,
    /// Which fields the chosen folder already answers for itself, so the
    /// dialog shows them read-only. See [`Locks`].
    locks: Locks,
    /// The `folder` value [`Locks`] were computed for, so they are recomputed
    /// when the user types or browses to a different one — and **only** then,
    /// since deriving them shells out to `git`.
    locked_for: Option<String>,
}

/// Which remotes the chosen folder already has, and so must not be edited
/// here (maintainer, 2026-09-22: *"these should auto-populate and become
/// read-only. I don't [want] users messing up their git like that"*).
///
/// This is a guard against a **silent divergence**, not just a convenience.
/// `ensure_repo` never changes an existing `origin` and `ensure_corpus`
/// adopts a corpus repository that is already there — but `set_corpora`
/// would still write whatever the dialog said into `kovan_root.toml`. A user
/// who edited a field would end up with a recorded remote that Git does not
/// have, and nothing would report it.
///
/// A field is locked when its repository **exists on disk already**, and the
/// value shown is then that repository's own `origin` — the truth, rather
/// than what `kovan_root.toml` intended.
#[derive(Default)]
struct Locks {
    library: bool,
    open: bool,
    proprietary: bool,
}

/// One remote field: editable when the folder does not already answer for
/// it, and read-only with an explanation when it does ([`Locks`]).
///
/// Locked fields are shown rather than hidden: the user needs to see which
/// remote their folder is on, and hiding it would make an unexpected value
/// invisible. `why` says what makes it read-only, so "I cannot type here"
/// never reads as a bug.
#[cfg(all(feature = "gui", not(target_os = "android")))]
fn remote_field(ui: &mut egui::Ui, value: &mut String, locked: bool, hint: &str, why: &str) {
    ui.add_enabled(
        !locked,
        egui::TextEdit::singleline(value)
            .hint_text(hint)
            .desired_width(420.0),
    );
    if locked {
        let shown = if value.trim().is_empty() {
            "a local repository with no remote".to_string()
        } else {
            value.trim().to_string()
        };
        ui.weak(format!(
            "\u{1F512} {why}: {shown}. Kovan will not change it — use Git if you need to."
        ));
    }
}

/// A trimmed remote URL, or `None` for an empty field.
fn remote(field: &str) -> Option<String> {
    let t = field.trim();
    (!t.is_empty()).then(|| t.to_string())
}

impl SetupDialog {
    /// Open the dialog, prefilled from `root` when a Kovan folder is already
    /// open (so reopening it shows the current remotes).
    pub(super) fn show_for(&mut self, root: Option<&KovanRoot>) {
        self.open = true;
        self.message.clear();
        if let Some(root) = root {
            self.folder = root.path().display().to_string();
            self.library_remote = crate::advanced_git::list_remotes_in(root.path())
                .ok()
                .and_then(|rs| rs.into_iter().find(|r| r.name == "origin"))
                .map(|r| r.url)
                .unwrap_or_default();
            let c = &root.config().corpora;
            self.open_remote = c.open_remote.clone().unwrap_or_default();
            self.proprietary_remote = c.proprietary_remote.clone().unwrap_or_default();
        } else {
            // No folder open: offer the corpora remembered from an earlier
            // setup. An open folder's own `kovan_root.toml` is its authority.
            let remembered = remembered_corpora();
            if self.open_remote.is_empty() {
                self.open_remote = remembered.open_remote.unwrap_or_default();
            }
            if self.proprietary_remote.is_empty() {
                self.proprietary_remote = remembered.proprietary_remote.unwrap_or_default();
            }
        }
    }

    /// The `origin` URL of the repository at `dir`, or `None` when `dir` is
    /// not a repository or has no `origin`.
    fn origin_of(dir: &Path) -> Option<String> {
        if !crate::corpus_repos::is_git_repo(dir) {
            return None;
        }
        crate::advanced_git::list_remotes_in(dir)
            .ok()?
            .into_iter()
            .find(|r| r.name == "origin")
            .map(|r| r.url)
    }

    /// Recompute [`Locks`] for the currently chosen folder, filling each
    /// locked field with the remote its repository actually has.
    ///
    /// A repository that exists but has no `origin` still locks the field:
    /// it is a local corpus Kovan created, and pointing it at a remote here
    /// would not do so on disk. The field shows that plainly instead.
    fn refresh_locks(&mut self) {
        if self.locked_for.as_deref() == Some(self.folder.as_str()) {
            return;
        }
        self.locked_for = Some(self.folder.clone());
        self.locks = Locks::default();
        let folder = PathBuf::from(self.folder.trim());
        if self.folder.trim().is_empty() || !folder.is_dir() {
            return;
        }
        if crate::corpus_repos::is_git_repo(&folder) {
            self.locks.library = true;
            self.library_remote = Self::origin_of(&folder).unwrap_or_default();
        }
        let Ok(root) = KovanRoot::open(&folder) else {
            return;
        };
        let open_dir = root.open_corpus_dir();
        if crate::corpus_repos::is_git_repo(&open_dir) {
            self.locks.open = true;
            self.open_remote = Self::origin_of(&open_dir).unwrap_or_default();
        }
        let restricted_dir = root.restricted_sources_dir();
        if crate::corpus_repos::is_git_repo(&restricted_dir) {
            self.locks.proprietary = true;
            self.proprietary_remote = Self::origin_of(&restricted_dir).unwrap_or_default();
        }
    }

    /// Show a message (a validation problem, or the outcome) in the dialog.
    pub(super) fn set_message(&mut self, message: impl Into<String>) {
        self.message = message.into();
    }

    /// Draw the dialog, if open. Returns what the user asked for this frame.
    pub(super) fn ui(&mut self, ctx: &egui::Context) -> Option<SetupRequest> {
        if !self.open {
            return None;
        }
        self.refresh_locks();
        let mut request = None;
        egui::Window::new("Set up Kovan")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label(
                    "Kovan always shows its built-in nuclear-engineering map. Set up \
                     your own Kovan folder to add your knowledge on top of it: your \
                     Kovan repository, plus your open and proprietary corpora. Kovan's \
                     standard corpus is added to every folder automatically.",
                );
                ui.add_space(8.0);

                ui.strong("1. Kovan folder (required)");
                ui.weak("An existing Kovan folder is opened; any other folder becomes a new one.");
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.folder).desired_width(360.0));
                    if ui.button("Browse…").clicked() {
                        request = Some(SetupRequest::Browse);
                    }
                });
                ui.weak(
                    "Your own Kovan repository on GitHub (optional): an empty folder is \
                     cloned from it, with its corpora; an existing one gets it as its remote.",
                );
                remote_field(
                    ui,
                    &mut self.library_remote,
                    self.locks.library,
                    "https://github.com/you/your-kovan-repo.git",
                    "This folder is already a Git repository",
                );
                ui.add_space(6.0);

                ui.strong("2. Open corpus (optional)");
                ui.weak(
                    "A GitHub repository of redistributable literature: PDFs at its root, \
                     or in top-level folders whose name contains \"open-corpus\". Left \
                     empty, a local repository is created for you to push later.",
                );
                remote_field(
                    ui,
                    &mut self.open_remote,
                    self.locks.open,
                    "https://github.com/you/your-open-corpus.git",
                    "This corpus already exists in the folder",
                );
                ui.add_space(6.0);

                ui.strong("3. Proprietary corpus (optional, must be a PRIVATE repository)");
                ui.weak(
                    "Literature you may not redistribute. Kovan never commits it to your \
                     Kovan folder. Left empty, a local repository is created instead.",
                );
                remote_field(
                    ui,
                    &mut self.proprietary_remote,
                    self.locks.proprietary,
                    "https://github.com/you/your-private-corpus.git",
                    "This corpus already exists in the folder",
                );
                ui.weak("URLs only: never put a password or token here.");

                if !self.message.is_empty() {
                    ui.add_space(6.0);
                    ui.colored_label(ui.visuals().warn_fg_color, &self.message);
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Set up").clicked() {
                        let folder = self.folder.trim();
                        if folder.is_empty() {
                            self.message = "Choose a Kovan folder first.".into();
                        } else if [
                            &self.library_remote,
                            &self.open_remote,
                            &self.proprietary_remote,
                        ]
                        .iter()
                        .any(|r| r.trim().contains(char::is_whitespace))
                        {
                            self.message = "A repository URL cannot contain spaces.".into();
                        } else {
                            request = Some(SetupRequest::Finish {
                                folder: PathBuf::from(folder),
                                library_remote: remote(&self.library_remote),
                                open_remote: remote(&self.open_remote),
                                proprietary_remote: remote(&self.proprietary_remote),
                            });
                        }
                    }
                    if ui.button("Skip for now").clicked() {
                        request = Some(SetupRequest::Skip);
                    }
                });
            });
        request
    }
}

/// Where Kovan remembers the user's open and closed corpus remotes, in its
/// platform config folder: every new Kovan folder gets these corpora
/// (maintainer direction, 2026-09-22), not only the one the dialog set up.
fn remembered_corpora_file() -> Option<PathBuf> {
    if cfg!(test) {
        return None; // never the user's real config under test
    }
    directories::ProjectDirs::from("org", "OUTRAM PARK", "kovan")
        .map(|d| d.config_dir().join("corpora.toml"))
}

/// The user's remembered corpus remotes; empty when none were ever given.
pub(super) fn remembered_corpora() -> CorporaConfig {
    remembered_corpora_file()
        .map(|p| read_corpora(&p))
        .unwrap_or_default()
}

/// Remember the remotes `given` names, keeping any it leaves out. Best
/// effort: failing to write only means new folders are not given them.
pub(super) fn remember_corpora(given: &CorporaConfig) {
    if let Some(p) = remembered_corpora_file() {
        let _ = write_corpora(&p, given);
    }
}

fn read_corpora(path: &Path) -> CorporaConfig {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| toml::from_str(&t).ok())
        .unwrap_or_default()
}

fn write_corpora(path: &Path, given: &CorporaConfig) -> std::io::Result<()> {
    let old = read_corpora(path);
    let merged = CorporaConfig {
        open_remote: given.open_remote.clone().or(old.open_remote),
        proprietary_remote: given.proprietary_remote.clone().or(old.proprietary_remote),
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = toml::to_string(&merged).map_err(std::io::Error::other)?;
    std::fs::write(path, text)
}

/// The file whose presence records that first run is over, in Kovan's
/// platform config folder (beside `recent_roots.toml`).
fn first_run_marker() -> Option<PathBuf> {
    if cfg!(test) {
        return None; // never the user's real config under test
    }
    directories::ProjectDirs::from("org", "OUTRAM PARK", "kovan")
        .map(|d| d.config_dir().join("setup_done"))
}

/// Whether this is Kovan's first run (the setup dialog has never been
/// finished or skipped). `false` when there is no config folder, so a
/// platform without one is not nagged every launch.
pub(super) fn is_first_run() -> bool {
    first_run_marker().is_some_and(|p| !p.exists())
}

/// Record that first run is over. Best effort: failing to write the marker
/// only means the dialog shows again next launch.
pub(super) fn mark_first_run_done() {
    if let Some(p) = first_run_marker() {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, "Kovan setup was finished or skipped.\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder that already answers for a remote locks that field and shows
    /// the repository's **own** `origin`, not whatever was typed before
    /// (maintainer, 2026-09-22: a user must not be able to edit Kovan into
    /// disagreeing with Git). A corpus that exists without a remote locks
    /// too, because pointing it somewhere here would not move it on disk.
    #[test]
    fn an_existing_repository_locks_its_remote_field() {
        if !crate::advanced_git::system_git_available() {
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let lib = tmp.path().join("lib");
        let root = KovanRoot::create(&lib, crate::root::RootConfig::new("lib", "Lib"), true)
            .unwrap();
        // An open corpus that exists locally, with no remote of its own.
        crate::corpus_repos::ensure_repo(&root.open_corpus_dir(), None, None).unwrap();

        let mut dialog = SetupDialog {
            folder: lib.display().to_string(),
            // What a user might have typed before choosing this folder.
            open_remote: "https://example.com/typed-by-hand.git".into(),
            ..Default::default()
        };
        dialog.refresh_locks();

        assert!(dialog.locks.library, "the folder is a Git repository");
        assert!(dialog.locks.open, "its open corpus already exists");
        assert!(
            !dialog.locks.proprietary,
            "the proprietary corpus has no repository yet"
        );
        assert_eq!(
            dialog.open_remote, "",
            "the typed value is replaced by the repository's own (absent) origin"
        );

        // Recomputed only when the folder changes, so the lock does not cost
        // a `git` call every frame.
        dialog.folder = tmp.path().join("elsewhere").display().to_string();
        dialog.refresh_locks();
        assert!(!dialog.locks.library);
        assert!(!dialog.locks.open);
    }

    #[test]
    fn empty_remote_fields_mean_no_remote() {
        assert_eq!(remote("   "), None);
        assert_eq!(
            remote("  https://github.com/a/b.git "),
            Some("https://github.com/a/b.git".to_string())
        );
    }

    /// Remembered remotes are merged: a later setup that names only one
    /// corpus keeps the other.
    #[test]
    fn remembered_corpora_merge() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("cfg/corpora.toml");
        assert!(read_corpora(&file).is_empty());
        write_corpora(
            &file,
            &CorporaConfig {
                open_remote: Some("https://example.com/o.git".into()),
                proprietary_remote: Some("https://example.com/p.git".into()),
            },
        )
        .unwrap();
        write_corpora(
            &file,
            &CorporaConfig {
                open_remote: Some("https://example.com/o2.git".into()),
                proprietary_remote: None,
            },
        )
        .unwrap();
        let back = read_corpora(&file);
        assert_eq!(
            back.open_remote.as_deref(),
            Some("https://example.com/o2.git")
        );
        assert_eq!(
            back.proprietary_remote.as_deref(),
            Some("https://example.com/p.git")
        );
    }

    /// Reopening the dialog on an open folder shows its current remotes.
    #[test]
    fn reopening_prefills_from_the_open_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let mut root = KovanRoot::create(
            tmp.path(),
            crate::root::RootConfig::new("lib", "Lib"),
            false,
        )
        .unwrap();
        root.set_corpora(crate::root::CorporaConfig {
            open_remote: Some("https://example.com/o.git".into()),
            proprietary_remote: None,
        })
        .unwrap();
        let mut d = SetupDialog::default();
        d.show_for(Some(&root));
        assert!(d.open);
        assert_eq!(d.open_remote, "https://example.com/o.git");
        assert_eq!(d.proprietary_remote, "");
        assert_eq!(d.folder, tmp.path().display().to_string());
    }
}
