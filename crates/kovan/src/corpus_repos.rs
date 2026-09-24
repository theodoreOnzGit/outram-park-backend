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
    /// Already a repository with no `origin`; the given remote was added as
    /// `origin` (a corpus first created locally, whose URL the user gave
    /// later), so it can be pushed without typing the URL again.
    RemoteAdded,
    /// Mounted in the Kovan folder as a Git submodule of its remote: cloned
    /// as one, or an existing repository at the path adopted as one.
    SubmoduleAdded,
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
/// - Already a repository: [`RepoState::Existing`], untouched, except that a
///   given `remote` is added as `origin` when the repository has no `origin`
///   yet ([`RepoState::RemoteAdded`]). An existing `origin` is never changed.
/// - `remote` given: cloned into `dir` (`branch`, if given, is the branch to
///   check out), provided `dir` is absent or empty.
/// - No `remote`: initialised in place, creating `dir` if needed.
pub fn ensure_repo(
    dir: &Path,
    remote: Option<&str>,
    branch: Option<&str>,
) -> Result<RepoState, CorpusRepoError> {
    if is_git_repo(dir) {
        if let Some(url) = remote {
            let has_origin = crate::advanced_git::list_remotes_in(dir)
                .map(|rs| rs.iter().any(|r| r.name == "origin"))
                .unwrap_or(true);
            if !has_origin {
                crate::advanced_git::add_remote_in(dir, "origin", url).map_err(|e| {
                    CorpusRepoError::Clone {
                        remote: url.to_string(),
                        stderr: e.to_string(),
                    }
                })?;
                return Ok(RepoState::RemoteAdded);
            }
        }
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

/// Clone `url` into `dir`, **with its submodules**.
///
/// A Kovan repository keeps its whole literature corpus in submodules, so a
/// clone that does not recurse lands a folder of empty directories and no
/// PDFs — which is what a user sees as "nothing was cloned in" (maintainer
/// report, 2026-09-22).
///
/// **A submodule that cannot be fetched does not fail the clone.** Git exits
/// non-zero from `--recurse-submodules` if *any* submodule fetch fails, and
/// the ordinary case is a **private** corpus the user has no credentials for
/// (`literature/proprietary`). The superproject is on disk by then, so this
/// reports success and leaves the unfetched corpus to [`ensure_corpus`],
/// which fetches each one separately and reports per-corpus failures the
/// caller can show. Failing the whole clone here would abort the setup before
/// the Kovan folder is ever opened.
///
/// `GIT_TERMINAL_PROMPT=0` so a private submodule fails fast instead of
/// blocking the background job on a credential prompt no GUI window can
/// answer. Configured credential helpers (including GUI askpass) still run —
/// this disables only Git's own terminal prompt.
fn clone(url: &str, dir: &Path, branch: Option<&str>) -> Result<(), CorpusRepoError> {
    if !crate::advanced_git::system_git_available() {
        return Err(CorpusRepoError::GitUnavailable);
    }
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut cmd = Command::new("git");
    // Local file-path remotes under test: Git refuses them for submodules by
    // default, and only `-c` reaches the clone Git runs for each submodule.
    // Same reasoning as `submodule_git`.
    #[cfg(test)]
    cmd.args(["-c", "protocol.file.allow=always"]);
    cmd.arg("clone").arg("--recurse-submodules");
    if let Some(b) = branch {
        cmd.args(["--branch", b]);
    }
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    let output = cmd.arg(url).arg(dir).output()?;
    if output.status.success() || is_git_repo(dir) {
        Ok(())
    } else {
        Err(CorpusRepoError::Clone {
            remote: url.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

/// The outcome for each of a Kovan folder's three corpus repositories.
#[derive(Debug)]
pub struct CorporaSetup {
    /// Kovan's standard corpus, the same for every user.
    pub standard: Result<RepoState, CorpusRepoError>,
    /// The user's open corpus.
    pub open: Result<RepoState, CorpusRepoError>,
    /// The user's proprietary (closed) corpus.
    pub proprietary: Result<RepoState, CorpusRepoError>,
}

/// Make the three corpus repositories of `root` (maintainer direction,
/// 2026-09-22: a Kovan folder is its own repository plus the standard, open
/// and closed corpora):
///
/// - **standard corpus** at [`KovanRoot::standard_corpus_dir`], from
///   [`crate::corpus::CORPUS_REPOSITORY_URL`], the same for every user;
/// - **open corpus** at [`KovanRoot::open_corpus_dir`] and **proprietary
///   corpus** at [`KovanRoot::restricted_sources_dir`], from the user's own
///   remotes in `[corpora]`.
///
/// Each is attempted independently; one failing does not stop the others.
/// See [`ensure_corpus`] for how each is set up.
pub fn ensure_library_corpora(root: &KovanRoot) -> CorporaSetup {
    ensure_library_corpora_with(
        root,
        crate::corpus::CORPUS_REPOSITORY_URL,
        crate::corpus::CORPUS_REPOSITORY_BRANCH,
    )
}

/// [`ensure_library_corpora`] with the standard corpus's remote and branch
/// given, so tests can use a local repository instead of the network.
pub fn ensure_library_corpora_with(
    root: &KovanRoot,
    standard_remote: &str,
    standard_branch: &str,
) -> CorporaSetup {
    let corpora = &root.config().corpora;
    CorporaSetup {
        standard: ensure_corpus(
            root,
            &root.standard_corpus_dir(),
            Some(standard_remote),
            Some(standard_branch),
        ),
        open: ensure_corpus(
            root,
            &root.open_corpus_dir(),
            corpora.open_remote.as_deref(),
            None,
        ),
        proprietary: ensure_corpus(
            root,
            &root.restricted_sources_dir(),
            corpora.proprietary_remote.as_deref(),
            None,
        ),
    }
}

/// Set up one corpus repository at `dir` inside `root`.
///
/// - **`root` is a Git repository and `remote` is known:** the corpus becomes
///   a **submodule** of `root` ([`RepoState::SubmoduleAdded`]), cloned into
///   `dir` or, if a repository is already there, adopted as it is (its files
///   are never replaced). Already a submodule: [`RepoState::Existing`], or
///   [`RepoState::Cloned`] after fetching one that was registered but not yet
///   fetched (a plain clone of someone's Kovan repository).
///   `--force` is used because the corpus paths are gitignored for the local
///   case below; ignore rules do not apply to tracked paths.
/// - Otherwise: [`ensure_repo`], a plain clone or a local repository.
pub fn ensure_corpus(
    root: &KovanRoot,
    dir: &Path,
    remote: Option<&str>,
    branch: Option<&str>,
) -> Result<RepoState, CorpusRepoError> {
    let Some(url) = remote.filter(|_| root.has_git()) else {
        return ensure_repo(dir, remote, branch);
    };
    let rel = dir.strip_prefix(root.path()).unwrap_or(dir);
    let rel_str = rel.to_string_lossy().replace('\\', "/");
    if is_submodule(root.path(), &rel_str) {
        if is_git_repo(dir) {
            return Ok(RepoState::Existing);
        }
        // Registered but not fetched, as after a plain clone of someone's
        // Kovan repository: fetch it now.
        let output = submodule_git(root.path())
            .args(["submodule", "update", "--init", "--", &rel_str])
            .output()?;
        return if output.status.success() {
            attach_to_branch(dir);
            Ok(RepoState::Cloned)
        } else {
            Err(CorpusRepoError::Clone {
                remote: url.to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            })
        };
    }
    let has_files = dir.is_dir() && std::fs::read_dir(dir)?.next().is_some();
    if has_files && !is_git_repo(dir) {
        return Err(CorpusRepoError::NotEmpty(dir.to_path_buf()));
    }
    if dir.is_dir() && !has_files {
        // The folder skeleton's empty directory: `git submodule add` refuses
        // an existing path that is not a repository, empty or not.
        std::fs::remove_dir(dir)?;
    }
    if !crate::advanced_git::system_git_available() {
        return Err(CorpusRepoError::GitUnavailable);
    }
    let mut cmd = submodule_git(root.path());
    cmd.args(["submodule", "add", "--force"]);
    if let (Some(b), false) = (branch, is_git_repo(dir)) {
        cmd.args(["-b", b]);
    }
    let output = cmd.arg(url).arg(&rel_str).output()?;
    if output.status.success() {
        Ok(RepoState::SubmoduleAdded)
    } else {
        Err(CorpusRepoError::Clone {
            remote: url.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        })
    }
}

/// Put a freshly fetched submodule at `dir` back on a branch: `git
/// submodule update` leaves it on a detached `HEAD`, where a commit (an
/// ingested PDF) is on no branch and cannot simply be pushed. Only a remote
/// branch whose tip IS the recorded commit is used, so nothing moves; when
/// there is none, the submodule stays detached. Best effort.
fn attach_to_branch(dir: &Path) {
    let git = |args: &[&str]| Command::new("git").arg("-C").arg(dir).args(args).output();
    let Ok(out) = git(&[
        "for-each-ref",
        "--points-at",
        "HEAD",
        "--format=%(refname:strip=3)",
        "refs/remotes/origin",
    ]) else {
        return;
    };
    let branches = String::from_utf8_lossy(&out.stdout).to_string();
    if let Some(branch) = branches.lines().find(|b| !b.is_empty() && *b != "HEAD") {
        let _ = git(&[
            "checkout",
            "-q",
            "-B",
            branch,
            "--track",
            &format!("origin/{branch}"),
        ]);
    }
}

/// `git -C root`, for a submodule command. Under test, local file-path
/// remotes are allowed: Git refuses them for submodules by default, the
/// tests' stand-in remotes are local paths, and a repository-local setting
/// does not reach the clone Git runs for the submodule, whereas `-c` does.
fn submodule_git(root: &Path) -> Command {
    let mut cmd = Command::new("git");
    #[cfg(test)]
    cmd.args(["-c", "protocol.file.allow=always"]);
    // Fail fast rather than block a background job on a credential prompt;
    // see `clone`.
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd.arg("-C").arg(root);
    cmd
}

/// Whether `rel` (a path relative to the repository at `root`) is a
/// submodule there: tracked as a gitlink, index mode `160000`.
fn is_submodule(root: &Path, rel: &str) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "--stage", "--", rel])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).starts_with("160000"))
        .unwrap_or(false)
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

/// Where an ingested open PDF is stored in the open-corpus repository at
/// `repo`: the user's own `*open-corpus*` folder when there is exactly one
/// besides [`crate::corpus::STANDARD_CORPUS_FOLDER`] (so a user whose open
/// corpus is `reactor-literature` ingests into `theodore-open-corpus/`, never
/// the standard corpus), otherwise the repository root.
///
/// The root is right for a plain open corpus. With several folders of the
/// user's own, which one a document belongs in is theirs to decide, so it
/// goes to the root to be moved by hand.
pub fn open_corpus_ingest_dir(repo: &Path) -> PathBuf {
    let own: Vec<PathBuf> = open_corpus_folders(repo)
        .into_iter()
        .filter(|f| f != repo)
        .filter(|f| {
            f.file_name()
                .is_none_or(|n| n != crate::corpus::STANDARD_CORPUS_FOLDER)
        })
        .collect();
    match own.as_slice() {
        [one] => one.clone(),
        _ => repo.to_path_buf(),
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

    /// A corpus created locally gets its URL as `origin` when the user gives
    /// one later; an existing `origin` is never replaced.
    #[test]
    fn a_later_url_becomes_origin_but_never_replaces_one() {
        if !crate::advanced_git::system_git_available() {
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("open-corpus");
        assert_eq!(
            ensure_repo(&dir, None, None).unwrap(),
            RepoState::Initialised
        );
        let url = "https://example.com/mine.git";
        assert_eq!(
            ensure_repo(&dir, Some(url), None).unwrap(),
            RepoState::RemoteAdded
        );
        let origin = |d: &Path| {
            crate::advanced_git::list_remotes_in(d)
                .unwrap()
                .into_iter()
                .find(|r| r.name == "origin")
                .map(|r| r.url)
        };
        assert_eq!(origin(&dir).as_deref(), Some(url));
        assert_eq!(
            ensure_repo(&dir, Some("https://example.com/other.git"), None).unwrap(),
            RepoState::Existing
        );
        assert_eq!(origin(&dir).as_deref(), Some(url), "not replaced");
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

    /// A local repository with one commit, standing in for a remote.
    fn source_repo(dir: &Path, file: &str) -> String {
        touch(&dir.join(file));
        let git = |args: &[&str]| {
            let ok = Command::new("git")
                .arg("-C")
                .arg(dir)
                .args(["-c", "user.name=t", "-c", "user.email=t@t"])
                .args(args)
                .output()
                .unwrap()
                .status
                .success();
            assert!(ok, "git {args:?}");
        };
        git(&["init", "-q", "-b", "main"]);
        git(&["add", "."]);
        git(&["commit", "-q", "-m", "x"]);
        dir.to_string_lossy().to_string()
    }

    /// A Kovan folder that is not a Git repository and has no remotes of its
    /// own gets local repositories for its corpora; the standard corpus is
    /// cloned.
    #[test]
    fn a_library_without_git_or_remotes_gets_local_repositories() {
        if !crate::advanced_git::system_git_available() {
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let standard = source_repo(&tmp.path().join("std"), "kovan-standard-open-corpus/a.pdf");
        let lib = tmp.path().join("lib");
        let root = KovanRoot::create(&lib, RootConfig::new("lib", "Lib"), false).unwrap();
        let setup = ensure_library_corpora_with(&root, &standard, "main");
        assert_eq!(setup.standard.unwrap(), RepoState::Cloned);
        assert_eq!(setup.open.unwrap(), RepoState::Initialised);
        assert_eq!(setup.proprietary.unwrap(), RepoState::Initialised);
        assert!(
            root.standard_corpus_dir()
                .join("kovan-standard-open-corpus/a.pdf")
                .exists()
        );
    }

    /// In a Kovan folder that is a Git repository, every corpus with a remote
    /// becomes a submodule; one without stays a local repository; a second
    /// run changes nothing.
    #[test]
    fn corpora_with_remotes_become_submodules() {
        if !crate::advanced_git::system_git_available() {
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let standard = source_repo(&tmp.path().join("std"), "kovan-standard-open-corpus/a.pdf");
        let open = source_repo(&tmp.path().join("open"), "my-open-corpus/b.pdf");
        let lib = tmp.path().join("lib");
        let mut root = KovanRoot::create(&lib, RootConfig::new("lib", "Lib"), true).unwrap();
        root.set_corpora(crate::root::CorporaConfig {
            open_remote: Some(open.clone()),
            proprietary_remote: None,
        })
        .unwrap();
        let setup = ensure_library_corpora_with(&root, &standard, "main");
        assert_eq!(setup.standard.unwrap(), RepoState::SubmoduleAdded);
        assert_eq!(setup.open.unwrap(), RepoState::SubmoduleAdded);
        assert_eq!(setup.proprietary.unwrap(), RepoState::Initialised);
        assert!(is_submodule(&lib, "literature/standard-corpus"));
        assert!(is_submodule(&lib, "literature/open-corpus"));
        assert!(!is_submodule(&lib, "literature/proprietary"));
        assert!(root.open_corpus_dir().join("my-open-corpus/b.pdf").exists());
        let again = ensure_library_corpora_with(&root, &standard, "main");
        assert_eq!(again.standard.unwrap(), RepoState::Existing);
        assert_eq!(again.open.unwrap(), RepoState::Existing);
    }

    /// Cloning someone's Kovan repository brings its corpus submodules **with
    /// it** — `clone` recurses, so the PDFs are there before any setup runs
    /// (maintainer report 2026-09-22: a non-recursive clone landed a folder of
    /// empty directories). Setting it up afterwards then finds them present.
    #[test]
    fn a_cloned_kovan_repository_brings_its_corpora_with_it() {
        if !crate::advanced_git::system_git_available() {
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let standard = source_repo(&tmp.path().join("std"), "kovan-standard-open-corpus/a.pdf");
        let open = source_repo(&tmp.path().join("open"), "my-open-corpus/b.pdf");
        let lib = tmp.path().join("lib");
        let mut root = KovanRoot::create(&lib, RootConfig::new("lib", "Lib"), true).unwrap();
        root.set_corpora(crate::root::CorporaConfig {
            open_remote: Some(open),
            proprietary_remote: None,
        })
        .unwrap();
        ensure_library_corpora_with(&root, &standard, "main");
        for args in [&["add", "-A"][..], &["commit", "-q", "-m", "corpora"]] {
            let ok = Command::new("git")
                .arg("-C")
                .arg(&lib)
                .args(["-c", "user.name=t", "-c", "user.email=t@t"])
                .args(args)
                .status()
                .unwrap()
                .success();
            assert!(ok, "git {args:?}");
        }

        let copy = tmp.path().join("copy");
        assert_eq!(
            ensure_repo(&copy, Some(&lib.to_string_lossy()), None).unwrap(),
            RepoState::Cloned
        );
        let cloned = KovanRoot::open(&copy).unwrap();
        // The clone recursed: the corpus PDFs are already on disk.
        assert!(
            cloned
                .open_corpus_dir()
                .join("my-open-corpus/b.pdf")
                .exists()
        );
        let setup = ensure_library_corpora_with(&cloned, &standard, "main");
        assert_eq!(setup.standard.unwrap(), RepoState::Existing);
        assert_eq!(setup.open.unwrap(), RepoState::Existing);
    }

    /// The recovery path still works for a repository cloned by some other
    /// means — `git clone` without `--recurse-submodules`, someone else's
    /// script, an older Kovan. The corpora are registered but empty, and
    /// setting the folder up fetches them.
    #[test]
    fn a_repository_cloned_without_recursion_still_fetches_its_corpora() {
        if !crate::advanced_git::system_git_available() {
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let standard = source_repo(&tmp.path().join("std"), "kovan-standard-open-corpus/a.pdf");
        let open = source_repo(&tmp.path().join("open"), "my-open-corpus/b.pdf");
        let lib = tmp.path().join("lib");
        let mut root = KovanRoot::create(&lib, RootConfig::new("lib", "Lib"), true).unwrap();
        root.set_corpora(crate::root::CorporaConfig {
            open_remote: Some(open),
            proprietary_remote: None,
        })
        .unwrap();
        ensure_library_corpora_with(&root, &standard, "main");
        for args in [&["add", "-A"][..], &["commit", "-q", "-m", "corpora"]] {
            let ok = Command::new("git")
                .arg("-C")
                .arg(&lib)
                .args(["-c", "user.name=t", "-c", "user.email=t@t"])
                .args(args)
                .status()
                .unwrap()
                .success();
            assert!(ok, "git {args:?}");
        }

        // Deliberately non-recursive, standing in for a clone Kovan did not make.
        let copy = tmp.path().join("copy");
        let ok = Command::new("git")
            .args(["-c", "protocol.file.allow=always", "clone", "-q"])
            .arg(&lib)
            .arg(&copy)
            .status()
            .unwrap()
            .success();
        assert!(ok, "plain clone");
        let cloned = KovanRoot::open(&copy).unwrap();
        assert!(
            !cloned
                .open_corpus_dir()
                .join("my-open-corpus/b.pdf")
                .exists()
        );
        let setup = ensure_library_corpora_with(&cloned, &standard, "main");
        assert_eq!(setup.standard.unwrap(), RepoState::Cloned);
        assert_eq!(setup.open.unwrap(), RepoState::Cloned);
        assert!(
            cloned
                .open_corpus_dir()
                .join("my-open-corpus/b.pdf")
                .exists()
        );
        // Fetched onto its branch, not a detached HEAD, so an ingest there
        // can be committed and pushed.
        assert_eq!(
            crate::advanced_git::current_branch_in(&cloned.open_corpus_dir()).as_deref(),
            Some("main")
        );
    }

    /// A submodule that cannot be fetched — the ordinary case for a
    /// **private** corpus the user has no credentials for — must not fail the
    /// whole clone. The superproject lands, and the unfetched corpus is left
    /// for `ensure_corpus` to report on its own.
    #[test]
    fn an_unreachable_submodule_does_not_fail_the_clone() {
        if !crate::advanced_git::system_git_available() {
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let standard = source_repo(&tmp.path().join("std"), "kovan-standard-open-corpus/a.pdf");
        let secret_dir = tmp.path().join("secret");
        let secret = source_repo(&secret_dir, "private/c.pdf");
        let lib = tmp.path().join("lib");
        let mut root = KovanRoot::create(&lib, RootConfig::new("lib", "Lib"), true).unwrap();
        root.set_corpora(crate::root::CorporaConfig {
            open_remote: None,
            proprietary_remote: Some(secret),
        })
        .unwrap();
        ensure_library_corpora_with(&root, &standard, "main");
        for args in [&["add", "-A"][..], &["commit", "-q", "-m", "corpora"]] {
            let ok = Command::new("git")
                .arg("-C")
                .arg(&lib)
                .args(["-c", "user.name=t", "-c", "user.email=t@t"])
                .args(args)
                .status()
                .unwrap()
                .success();
            assert!(ok, "git {args:?}");
        }
        // Make the proprietary corpus unreachable, as a private repository is
        // to someone without credentials.
        std::fs::remove_dir_all(&secret_dir).unwrap();

        let copy = tmp.path().join("copy");
        assert_eq!(
            ensure_repo(&copy, Some(&lib.to_string_lossy()), None).unwrap(),
            RepoState::Cloned,
            "an unfetchable submodule must not fail the clone"
        );
        // The superproject is there, which is what lets setup continue.
        assert!(copy.join("kovan_root.toml").exists());
        assert!(KovanRoot::open(&copy).is_ok());
    }

    /// Ingest goes to the user's own open-corpus folder, never the standard
    /// corpus's; to the root when there is no single one.
    #[test]
    fn ingest_goes_to_the_users_own_open_corpus_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("r");
        touch(&repo.join("kovan-standard-open-corpus/a.pdf"));
        assert_eq!(open_corpus_ingest_dir(&repo), repo);
        touch(&repo.join("theodore-open-corpus/b.pdf"));
        assert_eq!(
            open_corpus_ingest_dir(&repo),
            repo.join("theodore-open-corpus")
        );
        touch(&repo.join("second-open-corpus/c.pdf"));
        assert_eq!(open_corpus_ingest_dir(&repo), repo);
        let plain = tmp.path().join("plain");
        touch(&plain.join("x.pdf"));
        assert_eq!(open_corpus_ingest_dir(&plain), plain);
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
