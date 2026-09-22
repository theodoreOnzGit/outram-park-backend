//! The Git repositories that hold literature (GitHub issues #253, #255).
//!
//! What belongs here, and nothing GUI-side:
//!
//! - [`ensure_repo`]: make a directory a Git repository, by leaving an
//!   existing one alone, cloning a remote into it, or initialising it locally
//!   so the user can add a remote and push later.
//! - [`ensure_library_corpora`]: do that for a Kovan folder's two corpus
//!   repositories, the **open corpus** ([`KovanRoot::open_corpus_dir`]) and
//!   the **proprietary corpus** ([`KovanRoot::restricted_sources_dir`]), from
//!   the remotes in its `[corpora]` table ([`crate::root::CorporaConfig`]).
//! - [`ensure_standard_corpus`]: clone Kovan's standard corpus
//!   ([`crate::corpus::CORPUS_REPOSITORY_URL`]) once into the platform
//!   application-data folder, shared by every Kovan folder.
//! - [`open_corpus_pdfs`]: find the PDFs of an open corpus in either of the two
//!   accepted layouts.
//!
//! # Safety rules
//!
//! - **Nothing is ever overwritten.** A clone into a directory that already
//!   holds files is refused ([`CorpusRepoError::NotEmpty`]); initialising in
//!   place keeps every file that is there.
//! - **Nothing is pulled blindly.** An existing repository is returned as it is
//!   ([`RepoState::Existing`]); updating one is an explicit, separate action.
//! - **Failures are values, never panics**, so a failed clone (offline, no
//!   `git`, a bad URL) leaves the caller free to carry on: the built-in map
//!   never depends on any of this (epic #247).
//!
//! Network operations use the system `git` binary, as
//! [`crate::advanced_git`]'s remote operations do (the workspace's `gix` is
//! built without network features); local initialisation uses `gix::init`, as
//! [`KovanRoot::create`] does.

use crate::root::KovanRoot;
use std::path::{Path, PathBuf};
use std::process::Command;

/// What [`ensure_repo`] found or did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoState {
    /// Already a Git repository; left exactly as it was.
    Existing,
    /// Cloned from the remote.
    Cloned,
    /// Initialised as a new local repository (no remote given), keeping any
    /// files already in the directory.
    Initialised,
}

/// Why [`ensure_repo`] could not make a directory a repository.
#[derive(Debug)]
pub enum CorpusRepoError {
    /// No usable system `git` binary, which cloning needs.
    GitUnavailable,
    /// The directory already holds files and is not a repository, so cloning
    /// into it could destroy them. Nothing was changed.
    NotEmpty(PathBuf),
    /// `git clone` ran and failed (offline, a bad URL, no access).
    Clone { remote: String, stderr: String },
    /// Local initialisation failed.
    Init(String),
    /// A filesystem error.
    Io(std::io::Error),
}

impl std::fmt::Display for CorpusRepoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GitUnavailable => {
                write!(f, "no usable system `git` binary, so nothing can be cloned")
            }
            Self::NotEmpty(dir) => write!(
                f,
                "{} already holds files and is not a Git repository; not cloning over it",
                dir.display()
            ),
            Self::Clone { remote, stderr } => write!(f, "cloning {remote} failed: {stderr}"),
            Self::Init(e) => write!(f, "initialising a repository failed: {e}"),
            Self::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for CorpusRepoError {}

impl From<std::io::Error> for CorpusRepoError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Whether `dir` is the top of a Git repository (has its own `.git`).
pub fn is_git_repo(dir: &Path) -> bool {
    dir.join(".git").exists()
}

/// Make `dir` a Git repository. See the module doc's safety rules.
///
/// - Already a repository: [`RepoState::Existing`], untouched.
/// - `remote` given: cloned into `dir` (`branch`, if given, is the branch to
///   check out), provided `dir` is absent or empty.
/// - No `remote`: initialised in place, creating `dir` if needed.
pub fn ensure_repo(
    dir: &Path,
    remote: Option<&str>,
    branch: Option<&str>,
) -> Result<RepoState, CorpusRepoError> {
    if is_git_repo(dir) {
        return Ok(RepoState::Existing);
    }
    let has_files = dir.is_dir() && std::fs::read_dir(dir)?.next().is_some();
    match remote {
        Some(url) => {
            if has_files {
                return Err(CorpusRepoError::NotEmpty(dir.to_path_buf()));
            }
            clone(url, dir, branch)?;
            Ok(RepoState::Cloned)
        }
        None => {
            std::fs::create_dir_all(dir)?;
            gix::init(dir).map_err(|e| CorpusRepoError::Init(e.to_string()))?;
            Ok(RepoState::Initialised)
        }
    }
}

fn clone(url: &str, dir: &Path, branch: Option<&str>) -> Result<(), CorpusRepoError> {
    if !crate::advanced_git::system_git_available() {
        return Err(CorpusRepoError::GitUnavailable);
    }
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut cmd = Command::new("git");
    cmd.arg("clone");
    if let Some(b) = branch {
        cmd.args(["--branch", b]);
    }
    let output = cmd.arg(url).arg(dir).output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(CorpusRepoError::Clone {
            remote: url.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

/// The outcome for each of a Kovan folder's two corpus repositories.
#[derive(Debug)]
pub struct CorporaSetup {
    pub open: Result<RepoState, CorpusRepoError>,
    pub proprietary: Result<RepoState, CorpusRepoError>,
}

/// Make `root`'s open corpus and proprietary corpus Git repositories, from
/// the remotes in its `[corpora]` table, or locally where none is given
/// (#255). Each is attempted independently; one failing does not stop the
/// other.
pub fn ensure_library_corpora(root: &KovanRoot) -> CorporaSetup {
    let corpora = &root.config().corpora;
    CorporaSetup {
        open: ensure_repo(
            &root.open_corpus_dir(),
            corpora.open_remote.as_deref(),
            None,
        ),
        proprietary: ensure_repo(
            &root.restricted_sources_dir(),
            corpora.proprietary_remote.as_deref(),
            None,
        ),
    }
}

/// Where Kovan keeps its clone of the standard corpus: the platform
/// application-data folder (`~/.local/share/kovan/` on Linux), shared by every
/// Kovan folder. `None` when the platform reports no home directory.
pub fn standard_corpus_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("org", "OUTRAM PARK", "kovan")
        .map(|d| d.data_dir().join("standard-corpus"))
}

/// Clone the standard corpus ([`crate::corpus::CORPUS_REPOSITORY_URL`], branch
/// [`crate::corpus::CORPUS_REPOSITORY_BRANCH`]) into [`standard_corpus_dir`]
/// if it is not there yet (#253). An existing clone is left as it is.
pub fn ensure_standard_corpus() -> Result<RepoState, CorpusRepoError> {
    let dir = standard_corpus_dir().ok_or_else(|| {
        CorpusRepoError::Io(std::io::Error::other(
            "no application-data directory on this platform",
        ))
    })?;
    ensure_repo(
        &dir,
        Some(crate::corpus::CORPUS_REPOSITORY_URL),
        Some(crate::corpus::CORPUS_REPOSITORY_BRANCH),
    )
}

/// The local path of a hardcoded corpus entry's PDF
/// ([`crate::corpus::CorpusLiterature::corpus_file`]) in the standard-corpus
/// clone, if the clone is present and holds it.
pub fn standard_corpus_pdf(corpus_file: &str) -> Option<PathBuf> {
    let path = standard_corpus_dir()?.join(corpus_file);
    path.is_file().then_some(path)
}

/// The folders of an open-corpus repository that hold its documents, in the
/// two accepted layouts (maintainer direction, 2026-09-22):
///
/// 1. **Top-level folders whose name contains `open-corpus`**, such as
///    `theodore-open-corpus/` or `kovan-standard-open-corpus/`. When any
///    exist, only they are used.
/// 2. Otherwise **the repository root itself**.
pub fn open_corpus_folders(repo: &Path) -> Vec<PathBuf> {
    let mut folders: Vec<PathBuf> = std::fs::read_dir(repo)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter(|e| e.file_name().to_string_lossy().contains("open-corpus"))
        .map(|e| e.path())
        .collect();
    folders.sort();
    if folders.is_empty() {
        vec![repo.to_path_buf()]
    } else {
        folders
    }
}

/// Every PDF in an open-corpus repository's document folders
/// ([`open_corpus_folders`]), searched recursively, skipping `.git`, sorted.
pub fn open_corpus_pdfs(repo: &Path) -> Vec<PathBuf> {
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
    for folder in open_corpus_folders(repo) {
        walk(&folder, &mut out);
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::root::RootConfig;

    fn touch(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"%PDF-1.4").unwrap();
    }

    /// With no remote a directory is initialised in place, keeping its files;
    /// a second call finds the repository and leaves it alone.
    #[test]
    fn with_no_remote_a_repository_is_initialised_and_files_are_kept() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("literature/proprietary");
        touch(&dir.join("already-here.pdf"));
        assert_eq!(
            ensure_repo(&dir, None, None).unwrap(),
            RepoState::Initialised
        );
        assert!(is_git_repo(&dir));
        assert!(dir.join("already-here.pdf").exists(), "nothing removed");
        assert_eq!(ensure_repo(&dir, None, None).unwrap(), RepoState::Existing);
    }

    /// A clone never overwrites a folder that already holds files.
    #[test]
    fn cloning_over_existing_files_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("open-corpus");
        touch(&dir.join("mine.pdf"));
        let err = ensure_repo(&dir, Some("https://example.invalid/x.git"), None).unwrap_err();
        assert!(matches!(err, CorpusRepoError::NotEmpty(_)), "{err}");
        assert!(dir.join("mine.pdf").exists());
        assert!(!is_git_repo(&dir));
    }

    /// Cloning works, from a local repository so the test needs no network;
    /// skipped when there is no system `git`.
    #[test]
    fn a_remote_is_cloned() {
        if !crate::advanced_git::system_git_available() {
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("source");
        touch(&src.join("theodore-open-corpus/a.pdf"));
        let git = |args: &[&str]| {
            let ok = Command::new("git")
                .arg("-C")
                .arg(&src)
                .args(["-c", "user.name=t", "-c", "user.email=t@t"])
                .args(args)
                .output()
                .unwrap()
                .status
                .success();
            assert!(ok, "git {args:?}");
        };
        git(&["init", "-q"]);
        git(&["add", "."]);
        git(&["commit", "-q", "-m", "x"]);
        let dest = tmp.path().join("lib/literature/open-corpus");
        let url = src.to_string_lossy().to_string();
        assert_eq!(
            ensure_repo(&dest, Some(&url), None).unwrap(),
            RepoState::Cloned
        );
        assert!(dest.join("theodore-open-corpus/a.pdf").exists());
        assert_eq!(
            ensure_repo(&dest, Some(&url), None).unwrap(),
            RepoState::Existing
        );
    }

    /// A Kovan folder with no `[corpora]` remotes gets two local repositories.
    #[test]
    fn a_library_without_remotes_gets_two_local_repositories() {
        let tmp = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(tmp.path(), RootConfig::new("lib", "Lib"), false).unwrap();
        let setup = ensure_library_corpora(&root);
        assert_eq!(setup.open.unwrap(), RepoState::Initialised);
        assert_eq!(setup.proprietary.unwrap(), RepoState::Initialised);
        assert!(is_git_repo(&root.open_corpus_dir()));
        assert!(is_git_repo(&root.restricted_sources_dir()));
    }

    /// Both accepted layouts: `*open-corpus*` top-level folders when present
    /// (and only those), otherwise the repository root.
    #[test]
    fn open_corpus_pdfs_are_found_in_either_layout() {
        let tmp = tempfile::tempdir().unwrap();
        let foldered = tmp.path().join("a");
        touch(&foldered.join("theodore-open-corpus/nrc/x.pdf"));
        touch(&foldered.join("kovan-standard-open-corpus/y.PDF"));
        touch(&foldered.join("notes/ignored.pdf"));
        let names: Vec<String> = open_corpus_pdfs(&foldered)
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, ["y.PDF", "x.pdf"], "only the open-corpus folders");

        let flat = tmp.path().join("b");
        touch(&flat.join("paper.pdf"));
        touch(&flat.join("sub/other.pdf"));
        touch(&flat.join(".git/objects/not-a-doc.pdf"));
        assert_eq!(open_corpus_folders(&flat), vec![flat.clone()]);
        assert_eq!(
            open_corpus_pdfs(&flat).len(),
            2,
            "root layout, .git skipped"
        );
    }
}
