//! # Git-awareness (`git`)
//!
//! Read-only git awareness for KOVAN, built on the pure-Rust
//! [`gix`](https://docs.rs/gix) (gitoxide) *library*. Where [`crate::discover`]
//! answers "what files are on disk (honouring `.gitignore`)", this module
//! answers the complementary questions a knowledge layer needs about a
//! repository's *history and provenance*:
//!
//! - **Where is the repo?** — [`GixBackend::discover`] / [`GixBackend::open`]
//!   (walks up to the enclosing `.git`, or opens one exactly).
//! - **What is checked out?** — current branch / `HEAD` commit
//!   ([`GitProvider::head`]).
//! - **What does git actually track?** — [`GitProvider::tracked_files`], read
//!   from the git index. This is the git-truth complement to the filesystem
//!   walk in [`crate::discover`]: `discover` lists what is *present and not
//!   ignored*; `tracked_files` lists what git is *actually versioning*.
//! - **Provenance of a file / symbol** — commit history for a path
//!   ([`GitProvider::history_for_path`]), the last commit that touched it
//!   ([`GitProvider::last_commit_for_path`]), and per-line blame
//!   ([`GitProvider::blame_line`]) so a [`kovan_common::KovanSymbol`] or
//!   [`kovan_common::KovanDocument`] can be tied to the commit that introduced
//!   its definition line.
//! - **Is the working tree clean?** — [`GitProvider::is_worktree_dirty`].
//!
//! Everything here is **deterministic and offline**: `gix` reads the local
//! `.git` directory only — no network, no remote fetch. Given the same
//! repository state, every function returns the same result on every call.
//!
//! ## Library-first, binary-fallback
//!
//! Per KOVAN's "use the library first, fall back on the binary" principle, the
//! backend is an **enum** ([`GitProvider`]) — not a trait object — with two
//! variants:
//!
//! - [`GitProvider::Library`] wrapping [`GixBackend`] — the **default and
//!   primary** path. Pure Rust, so it builds and runs on Android, and it
//!   covers *every* operation this module exposes natively.
//! - [`GitProvider::Binary`] wrapping [`GixCliBackend`] — the **fallback**,
//!   which shells out to the `gix` command-line tool (the one
//!   `kovan setup` installs). It is selected only when the library path cannot
//!   open the repository (see [`GitProvider::open_preferring_library`]).
//!
//! [`GitBackend`] is a compile-time *contract* implemented by both backends so
//! the compiler checks they stay in step; it is **never** used as `dyn Trait`.
//! Dispatch is the exhaustive `match` in [`GitProvider`]'s methods.
//!
//! ### Which operations fall back to the binary?
//!
//! In this implementation, **none at the per-operation level**: the `gix`
//! *library* cleanly covers all of `head` / `tracked_files` / `history` /
//! `last_commit_for_path` / `blame_line` / `is_worktree_dirty`, so the primary
//! path never needs to defer a specific call. The binary backend is the
//! *structural* fallback for the case where the library cannot even open the
//! repository (an unusual on-disk layout, a `gix`-version the library build
//! doesn't understand). The `gix` CLI's higher-level porcelain is still
//! experimental and its output is not a stable contract, so the binary
//! backend's structured queries deliberately return [`GitError::Unsupported`]
//! rather than parse unstable text — it guarantees only availability probing
//! ([`GixCliBackend::is_available`]). The library is authoritative.
//!
//! ### Android
//!
//! The library path is pure Rust and Android-clean. The binary backend cannot
//! run (no `gix` binary on Android), so every [`GixCliBackend`] operation
//! returns [`GitError::BinaryUnavailableOnTarget`] when compiled for
//! `target_os = "android"`. Because the default constructors return the
//! library backend, `cargo check --target aarch64-linux-android` stays clean
//! and the Android code path never depends on the binary.

use std::path::{Path, PathBuf};

use gix::bstr::ByteSlice;

/// One commit, reduced to the provenance facts a knowledge layer records.
///
/// All fields are owned `String`/`i64` (no borrows into the repository), so a
/// `CommitInfo` outlives the [`gix::Repository`] it was read from and can be
/// stored directly on a [`kovan_common::KovanSymbol`] provenance record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitInfo {
    /// Full 40-character hex commit id (the SHA-1 object id).
    pub id: String,
    /// The first 12 characters of [`id`](CommitInfo::id) — the customary
    /// abbreviated commit hash, for human-facing display.
    pub short_id: String,
    /// Commit summary: the first line of the commit message, trimmed. Empty
    /// only if the message itself is empty.
    pub summary: String,
    /// Author name (the person who wrote the change), as recorded in the
    /// commit. May be empty if the commit omits it.
    pub author_name: String,
    /// Author email, as recorded in the commit. May be empty.
    pub author_email: String,
    /// Committer timestamp, in whole seconds since the Unix epoch (UTC). This
    /// is the commit's own date — the point in history the commit represents —
    /// not the author date. Used for newest-first ordering of history.
    pub committed_unix_seconds: i64,
}

/// The state of `HEAD`: which branch is checked out and the commit it resolves
/// to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadInfo {
    /// Short branch name `HEAD` points at (e.g. `"main"`), or `None` when
    /// `HEAD` is *detached* (checked out directly at a commit, not via a
    /// branch).
    pub branch: Option<String>,
    /// `true` when `HEAD` is detached (no branch). Equivalent to
    /// `branch.is_none()`, exposed as its own flag for readability at call
    /// sites.
    pub detached: bool,
    /// The commit `HEAD` currently resolves to.
    pub commit: CommitInfo,
}

/// Which backend a [`GitProvider`] is dispatching to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    /// The pure-Rust `gix` **library** backend (primary; Android-clean).
    Library,
    /// The `gix` **command-line** fallback backend.
    Binary,
}

/// Errors from the git-awareness layer.
#[derive(Debug)]
pub enum GitError {
    /// No git repository was found at (or above) the given path.
    NotARepository(PathBuf),
    /// Opening/discovering the repository failed (I/O, corrupt `.git`, …). The
    /// string is the underlying `gix` error rendered for a human.
    Open(String),
    /// `HEAD` has no commit yet — a freshly `git init`ed repository with
    /// nothing committed. History/blame/last-commit queries need at least one
    /// commit.
    UnbornHead,
    /// A `gix`-library operation failed after the repository opened
    /// (object lookup, index read, diff, …). The string is the underlying
    /// error rendered for a human.
    Backend(String),
    /// The requested operation is not supported by the **binary** backend.
    /// The `gix` CLI's porcelain for this query is not a stable contract, so
    /// the binary path declines rather than parse unstable output — use the
    /// library backend, which covers it natively. The `&str` names the op.
    Unsupported(&'static str),
    /// The **binary** backend is unavailable on this build target — there is no
    /// `gix` executable on Android. Returned by every [`GixCliBackend`]
    /// operation under `target_os = "android"`.
    BinaryUnavailableOnTarget,
    /// The **binary** backend could not run the `gix` executable (not on
    /// `PATH`, or it exited non-zero). The string carries the detail.
    BinaryFailed(String),
}

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GitError::NotARepository(p) => {
                write!(f, "no git repository at or above {}", p.display())
            }
            GitError::Open(e) => write!(f, "failed to open repository: {e}"),
            GitError::UnbornHead => write!(f, "repository HEAD is unborn (no commits yet)"),
            GitError::Backend(e) => write!(f, "git backend error: {e}"),
            GitError::Unsupported(op) => {
                write!(f, "operation '{op}' is not supported by the gix CLI fallback backend; use the library backend")
            }
            GitError::BinaryUnavailableOnTarget => {
                write!(
                    f,
                    "the gix binary fallback is unavailable on this target (e.g. Android)"
                )
            }
            GitError::BinaryFailed(e) => write!(f, "gix CLI invocation failed: {e}"),
        }
    }
}

impl std::error::Error for GitError {}

/// Compile-time contract every git backend satisfies.
///
/// This trait exists so the compiler verifies [`GixBackend`] and
/// [`GixCliBackend`] expose the same read-only surface. It is **never** used as
/// a trait object (`dyn GitBackend`): dispatch is the exhaustive `match` inside
/// [`GitProvider`]. Downstream code should call the methods on [`GitProvider`],
/// not import this trait — it is the internal contract, not the public entry
/// point.
///
/// Paths passed to the per-path methods are **repository-relative** (e.g.
/// `src/lib.rs`), matching [`kovan_common::KovanSymbol::file`].
pub trait GitBackend {
    /// The absolute path to the repository's working-tree root (or the `.git`
    /// directory for a bare repo).
    fn repo_root(&self) -> &Path;

    /// The current branch and the commit `HEAD` resolves to.
    fn head(&self) -> Result<HeadInfo, GitError>;

    /// Every path git currently tracks (reads the index), sorted, as
    /// repository-relative paths.
    fn tracked_files(&self) -> Result<Vec<PathBuf>, GitError>;

    /// Up to `max` commits of the whole repository, newest first.
    fn history(&self, max: usize) -> Result<Vec<CommitInfo>, GitError>;

    /// Up to `max` commits that changed `rela_path`, newest first.
    fn history_for_path(&self, rela_path: &Path, max: usize) -> Result<Vec<CommitInfo>, GitError>;

    /// The most recent commit that changed `rela_path`, or `None` if the path
    /// is not present in history.
    fn last_commit_for_path(&self, rela_path: &Path) -> Result<Option<CommitInfo>, GitError>;

    /// The commit that introduced the current content of 1-based `line` in
    /// `rela_path`, or `None` if the line/path is not blamable.
    fn blame_line(&self, rela_path: &Path, line: u32) -> Result<Option<CommitInfo>, GitError>;

    /// Whether the working tree differs from the index/`HEAD` for *tracked*
    /// files (untracked files do **not** make it dirty).
    fn is_worktree_dirty(&self) -> Result<bool, GitError>;
}

// ---------------------------------------------------------------------------
// Library backend (primary): pure-Rust gix.
// ---------------------------------------------------------------------------

/// The **primary** git backend, built on the pure-Rust `gix` library.
///
/// Owns an opened [`gix::Repository`] by value (no lifetimes, per the workspace
/// design rules). Construct with [`GixBackend::discover`] (walk up to the
/// enclosing repo) or [`GixBackend::open`] (open a repo at an exact path).
pub struct GixBackend {
    root: PathBuf,
    repo: gix::Repository,
}

impl GixBackend {
    /// Discover the git repository at or above `start`, opening it. This is the
    /// usual entry point: pass any path inside a working tree and it walks up
    /// to the enclosing `.git`.
    ///
    /// # Errors
    ///
    /// [`GitError::NotARepository`] if no repository encloses `start`;
    /// [`GitError::Open`] for other open failures.
    pub fn discover(start: &Path) -> Result<Self, GitError> {
        let repo = gix::discover(start).map_err(|e| match e {
            gix::discover::Error::Discover(_) => GitError::NotARepository(start.to_path_buf()),
            other => GitError::Open(other.to_string()),
        })?;
        Ok(Self::from_repo(repo))
    }

    /// Open the git repository located *exactly* at `path` (does not walk up).
    ///
    /// # Errors
    ///
    /// [`GitError::Open`] if `path` is not a repository or cannot be opened.
    pub fn open(path: &Path) -> Result<Self, GitError> {
        let repo = gix::open(path).map_err(|e| GitError::Open(e.to_string()))?;
        Ok(Self::from_repo(repo))
    }

    fn from_repo(repo: gix::Repository) -> Self {
        let root = repo
            .workdir()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| repo.git_dir().to_path_buf());
        Self { root, repo }
    }

    /// Borrow the underlying opened [`gix::Repository`] for callers that need a
    /// `gix` operation this module does not wrap.
    pub fn repository(&self) -> &gix::Repository {
        &self.repo
    }

    /// Build a [`CommitInfo`] from an opened `gix` commit.
    fn commit_info(commit: &gix::Commit<'_>) -> Result<CommitInfo, GitError> {
        let id = commit.id().to_string();
        let short_id = id.chars().take(12).collect();

        let summary = commit
            .message()
            .map_err(|e| GitError::Backend(e.to_string()))?
            .summary()
            .to_string();

        let author = commit
            .author()
            .map_err(|e| GitError::Backend(e.to_string()))?;
        let author_name = author.name.to_str_lossy().into_owned();
        let author_email = author.email.to_str_lossy().into_owned();

        let time = commit
            .time()
            .map_err(|e| GitError::Backend(e.to_string()))?;

        Ok(CommitInfo {
            id,
            short_id,
            summary,
            author_name,
            author_email,
            committed_unix_seconds: time.seconds,
        })
    }

    /// The blob object id of `rela_path` within `commit`'s tree, or `None` if
    /// the path is absent from that commit.
    fn path_blob_id(
        commit: &gix::Commit<'_>,
        rela_path: &Path,
    ) -> Result<Option<gix::ObjectId>, GitError> {
        let tree = commit
            .tree()
            .map_err(|e| GitError::Backend(e.to_string()))?;
        let entry = tree
            .lookup_entry_by_path(rela_path)
            .map_err(|e| GitError::Backend(e.to_string()))?;
        Ok(entry.map(|e| e.object_id()))
    }

    /// The head commit, mapping the "unborn HEAD" case to [`GitError::UnbornHead`].
    fn head_commit(&self) -> Result<gix::Commit<'_>, GitError> {
        self.repo.head_commit().map_err(|e| {
            let msg = e.to_string();
            if msg.contains("unborn") || msg.contains("does not exist") {
                GitError::UnbornHead
            } else {
                GitError::Backend(msg)
            }
        })
    }

    /// Walk history newest-first from `HEAD`, yielding each commit's `Info`.
    fn rev_walk_newest_first(&self) -> Result<gix::revision::Walk<'_>, GitError> {
        let head = self.head_commit()?;
        self.repo
            .rev_walk(Some(head.id))
            .sorting(gix::revision::walk::Sorting::ByCommitTime(
                gix::traverse::commit::simple::CommitTimeOrder::NewestFirst,
            ))
            .all()
            .map_err(|e| GitError::Backend(e.to_string()))
    }
}

impl GitBackend for GixBackend {
    fn repo_root(&self) -> &Path {
        &self.root
    }

    fn head(&self) -> Result<HeadInfo, GitError> {
        let branch = self
            .repo
            .head_name()
            .map_err(|e| GitError::Backend(e.to_string()))?
            .map(|name| name.shorten().to_str_lossy().into_owned());
        let commit = self.head_commit()?;
        let info = GixBackend::commit_info(&commit)?;
        Ok(HeadInfo {
            detached: branch.is_none(),
            branch,
            commit: info,
        })
    }

    fn tracked_files(&self) -> Result<Vec<PathBuf>, GitError> {
        let index = self
            .repo
            .index()
            .map_err(|e| GitError::Backend(e.to_string()))?;
        // The git index is already stored sorted by path, so the resulting
        // order is deterministic without an explicit sort. We sort anyway to
        // make the guarantee independent of index internals.
        let mut files: Vec<PathBuf> = index
            .entries()
            .iter()
            .map(|entry| gix::path::from_bstr(entry.path(&index)).into_owned())
            .collect();
        files.sort();
        Ok(files)
    }

    fn history(&self, max: usize) -> Result<Vec<CommitInfo>, GitError> {
        let mut out = Vec::new();
        for info in self.rev_walk_newest_first()? {
            if out.len() >= max {
                break;
            }
            let info = info.map_err(|e| GitError::Backend(e.to_string()))?;
            let commit = info
                .object()
                .map_err(|e| GitError::Backend(e.to_string()))?;
            out.push(GixBackend::commit_info(&commit)?);
        }
        Ok(out)
    }

    fn history_for_path(&self, rela_path: &Path, max: usize) -> Result<Vec<CommitInfo>, GitError> {
        let mut out = Vec::new();
        for info in self.rev_walk_newest_first()? {
            if out.len() >= max {
                break;
            }
            let info = info.map_err(|e| GitError::Backend(e.to_string()))?;
            let commit = info
                .object()
                .map_err(|e| GitError::Backend(e.to_string()))?;

            let current = GixBackend::path_blob_id(&commit, rela_path)?;
            if current.is_none() {
                // Path absent in this commit; it cannot have been changed here.
                continue;
            }

            // A commit "changed" the path if the path's blob differs from that
            // in its first parent (first-parent history simplification, the
            // same convention as `git log --first-parent -- <path>`). A root
            // commit (no parents) that contains the path introduced it.
            let first_parent = commit.parent_ids().next();
            let changed = match first_parent {
                None => true,
                Some(pid) => {
                    let parent = self
                        .repo
                        .find_commit(pid.detach())
                        .map_err(|e| GitError::Backend(e.to_string()))?;
                    GixBackend::path_blob_id(&parent, rela_path)? != current
                }
            };
            if changed {
                out.push(GixBackend::commit_info(&commit)?);
            }
        }
        Ok(out)
    }

    fn last_commit_for_path(&self, rela_path: &Path) -> Result<Option<CommitInfo>, GitError> {
        Ok(self.history_for_path(rela_path, 1)?.into_iter().next())
    }

    fn blame_line(&self, rela_path: &Path, line: u32) -> Result<Option<CommitInfo>, GitError> {
        if line == 0 {
            return Ok(None);
        }
        let head = self.head_commit()?;
        let path_bstr = gix::path::into_bstr(rela_path);

        let ranges = gix::blame::BlameRanges::from_one_based_inclusive_range(line..=line)
            .map_err(|e| GitError::Backend(e.to_string()))?;
        let options = gix::repository::blame_file::Options {
            ranges,
            ..Default::default()
        };
        let outcome = self
            .repo
            .blame_file(path_bstr.as_ref(), head.id, options)
            .map_err(|e| GitError::Backend(e.to_string()))?;

        // We restricted the blame to a single line, so the covering hunk is the
        // first (and only) entry. Resolve its commit to a full CommitInfo.
        let Some(entry) = outcome.entries.into_iter().next() else {
            return Ok(None);
        };
        let commit = self
            .repo
            .find_commit(entry.commit_id)
            .map_err(|e| GitError::Backend(e.to_string()))?;
        Ok(Some(GixBackend::commit_info(&commit)?))
    }

    fn is_worktree_dirty(&self) -> Result<bool, GitError> {
        self.repo
            .is_dirty()
            .map_err(|e| GitError::Backend(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Binary backend (fallback): shells out to the `gix` CLI.
// ---------------------------------------------------------------------------

/// The **fallback** git backend, which shells out to the `gix` command-line
/// tool (installed by `kovan setup`).
///
/// This exists to satisfy KOVAN's "library first, fall back on the binary"
/// principle when the [`GixBackend`] library path cannot open a repository. The
/// `gix` CLI's structured porcelain is still experimental and its text output
/// is not a stable contract, so the structured queries here deliberately return
/// [`GitError::Unsupported`] rather than parse unstable output — the library
/// backend is authoritative for those. What the binary backend *does*
/// guarantee is availability detection ([`GixCliBackend::is_available`]).
///
/// On `target_os = "android"` there is no `gix` executable, so every operation
/// returns [`GitError::BinaryUnavailableOnTarget`].
pub struct GixCliBackend {
    root: PathBuf,
    // Only read when actually shelling out to `gix` (non-Android); on Android
    // `version()` short-circuits to `BinaryUnavailableOnTarget` and never
    // touches it, so it is dead there.
    #[cfg_attr(target_os = "android", allow(dead_code))]
    program: String,
}

impl GixCliBackend {
    /// A binary backend rooted at `root`, invoking the default `gix` program
    /// name (resolved on `PATH`).
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            program: "gix".to_string(),
        }
    }

    /// A binary backend that invokes `program` instead of the default `gix`
    /// (e.g. an absolute path to the executable).
    pub fn with_program(root: &Path, program: impl Into<String>) -> Self {
        Self {
            root: root.to_path_buf(),
            program: program.into(),
        }
    }

    /// Whether the `gix` executable is runnable — probes `gix --version`.
    /// Always `false` on Android (no binary on that target).
    pub fn is_available(&self) -> bool {
        self.version().is_ok()
    }

    /// The `gix --version` string, or an error if the binary can't be run.
    ///
    /// This is the one operation the binary backend performs by actually
    /// shelling out; it is how [`GitProvider::open_preferring_library`] decides
    /// whether a binary fallback is usable.
    #[cfg(not(target_os = "android"))]
    pub fn version(&self) -> Result<String, GitError> {
        let output = std::process::Command::new(&self.program)
            .arg("--version")
            .output()
            .map_err(|e| GitError::BinaryFailed(e.to_string()))?;
        if !output.status.success() {
            return Err(GitError::BinaryFailed(format!(
                "`{} --version` exited with {}",
                self.program, output.status
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// On Android there is no `gix` executable, so probing always fails.
    #[cfg(target_os = "android")]
    pub fn version(&self) -> Result<String, GitError> {
        Err(GitError::BinaryUnavailableOnTarget)
    }

    /// The error every structured (non-probe) operation returns: the target's
    /// "no binary" error on Android, otherwise "unsupported by the CLI".
    fn decline(op: &'static str) -> GitError {
        #[cfg(target_os = "android")]
        {
            let _ = op;
            GitError::BinaryUnavailableOnTarget
        }
        #[cfg(not(target_os = "android"))]
        {
            GitError::Unsupported(op)
        }
    }
}

impl GitBackend for GixCliBackend {
    fn repo_root(&self) -> &Path {
        &self.root
    }

    fn head(&self) -> Result<HeadInfo, GitError> {
        Err(GixCliBackend::decline("head"))
    }

    fn tracked_files(&self) -> Result<Vec<PathBuf>, GitError> {
        Err(GixCliBackend::decline("tracked_files"))
    }

    fn history(&self, _max: usize) -> Result<Vec<CommitInfo>, GitError> {
        Err(GixCliBackend::decline("history"))
    }

    fn history_for_path(
        &self,
        _rela_path: &Path,
        _max: usize,
    ) -> Result<Vec<CommitInfo>, GitError> {
        Err(GixCliBackend::decline("history_for_path"))
    }

    fn last_commit_for_path(&self, _rela_path: &Path) -> Result<Option<CommitInfo>, GitError> {
        Err(GixCliBackend::decline("last_commit_for_path"))
    }

    fn blame_line(&self, _rela_path: &Path, _line: u32) -> Result<Option<CommitInfo>, GitError> {
        Err(GixCliBackend::decline("blame_line"))
    }

    fn is_worktree_dirty(&self) -> Result<bool, GitError> {
        Err(GixCliBackend::decline("is_worktree_dirty"))
    }
}

// ---------------------------------------------------------------------------
// Provider: enum dispatch (no dyn) over the two backends.
// ---------------------------------------------------------------------------

/// Git-awareness entry point: an **enum** over the two backends, dispatched by
/// exhaustive `match` (never `dyn Trait`).
///
/// The [`GitProvider::Library`] (pure-Rust `gix`) variant is the default and
/// primary path; [`GitProvider::Binary`] (the `gix` CLI) is the fallback. Use
/// [`GitProvider::open`] for the usual "just use the library" case, or
/// [`GitProvider::open_preferring_library`] to opt into the fallback when the
/// library cannot open the repo.
// The `Library` variant embeds a full `gix::Repository` (~1.2 KiB), so it
// dwarfs the `Binary` variant. Clippy would suggest boxing the large field,
// but the workspace design rules forbid `Box<T>` (own by value / `Arc`); a
// `GitProvider` is a single owned repository handle, never stored in bulk
// collections, so the size asymmetry is harmless here.
#[allow(clippy::large_enum_variant)]
pub enum GitProvider {
    /// The pure-Rust `gix` library backend (primary, Android-clean).
    Library(GixBackend),
    /// The `gix` CLI fallback backend.
    Binary(GixCliBackend),
}

impl GitProvider {
    /// Discover and open the repository at or above `start` using the
    /// **library** backend — the default, always-preferred path.
    ///
    /// # Errors
    ///
    /// [`GitError::NotARepository`] if no repository encloses `start`, or
    /// [`GitError::Open`] for other open failures.
    pub fn open(start: &Path) -> Result<Self, GitError> {
        Ok(GitProvider::Library(GixBackend::discover(start)?))
    }

    /// Try the **library** backend first; if it cannot open the repository,
    /// fall back to the **binary** backend when the `gix` CLI is available
    /// (never on Android). This is the explicit realisation of KOVAN's
    /// "library first, fall back on the binary" principle.
    ///
    /// If the library fails *and* no usable binary fallback exists, the
    /// original library error is returned.
    pub fn open_preferring_library(start: &Path) -> Result<Self, GitError> {
        match GixBackend::discover(start) {
            Ok(backend) => Ok(GitProvider::Library(backend)),
            Err(lib_err) => {
                let binary = GixCliBackend::new(start);
                if binary.is_available() {
                    Ok(GitProvider::Binary(binary))
                } else {
                    Err(lib_err)
                }
            }
        }
    }

    /// Construct a provider that explicitly uses the **binary** backend rooted
    /// at `root`. Mainly for tests and for callers that deliberately want the
    /// CLI path; ordinary code should prefer [`GitProvider::open`].
    pub fn binary(root: &Path) -> Self {
        GitProvider::Binary(GixCliBackend::new(root))
    }

    /// Which backend this provider dispatches to.
    pub fn backend_kind(&self) -> BackendKind {
        match self {
            GitProvider::Library(_) => BackendKind::Library,
            GitProvider::Binary(_) => BackendKind::Binary,
        }
    }

    /// The repository root — see [`GitBackend::repo_root`].
    pub fn repo_root(&self) -> &Path {
        match self {
            GitProvider::Library(b) => b.repo_root(),
            GitProvider::Binary(b) => b.repo_root(),
        }
    }

    /// Current branch / `HEAD` commit — see [`GitBackend::head`].
    pub fn head(&self) -> Result<HeadInfo, GitError> {
        match self {
            GitProvider::Library(b) => b.head(),
            GitProvider::Binary(b) => b.head(),
        }
    }

    /// Every path git tracks — see [`GitBackend::tracked_files`].
    pub fn tracked_files(&self) -> Result<Vec<PathBuf>, GitError> {
        match self {
            GitProvider::Library(b) => b.tracked_files(),
            GitProvider::Binary(b) => b.tracked_files(),
        }
    }

    /// Up to `max` commits, newest first — see [`GitBackend::history`].
    pub fn history(&self, max: usize) -> Result<Vec<CommitInfo>, GitError> {
        match self {
            GitProvider::Library(b) => b.history(max),
            GitProvider::Binary(b) => b.history(max),
        }
    }

    /// Up to `max` commits that changed `rela_path`, newest first — see
    /// [`GitBackend::history_for_path`].
    pub fn history_for_path(
        &self,
        rela_path: &Path,
        max: usize,
    ) -> Result<Vec<CommitInfo>, GitError> {
        match self {
            GitProvider::Library(b) => b.history_for_path(rela_path, max),
            GitProvider::Binary(b) => b.history_for_path(rela_path, max),
        }
    }

    /// The most recent commit that changed `rela_path` — see
    /// [`GitBackend::last_commit_for_path`].
    pub fn last_commit_for_path(&self, rela_path: &Path) -> Result<Option<CommitInfo>, GitError> {
        match self {
            GitProvider::Library(b) => b.last_commit_for_path(rela_path),
            GitProvider::Binary(b) => b.last_commit_for_path(rela_path),
        }
    }

    /// The commit introducing 1-based `line` of `rela_path` — see
    /// [`GitBackend::blame_line`].
    pub fn blame_line(&self, rela_path: &Path, line: u32) -> Result<Option<CommitInfo>, GitError> {
        match self {
            GitProvider::Library(b) => b.blame_line(rela_path, line),
            GitProvider::Binary(b) => b.blame_line(rela_path, line),
        }
    }

    /// Whether the working tree is dirty — see [`GitBackend::is_worktree_dirty`].
    pub fn is_worktree_dirty(&self) -> Result<bool, GitError> {
        match self {
            GitProvider::Library(b) => b.is_worktree_dirty(),
            GitProvider::Binary(b) => b.is_worktree_dirty(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    /// Run `git` with `args` in `dir`, asserting success. Used only to build a
    /// throwaway repository fixture; the code under test never shells out to
    /// `git` (the library backend is pure `gix`).
    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "Kovan Tester")
            .env("GIT_AUTHOR_EMAIL", "tester@example.com")
            .env("GIT_COMMITTER_NAME", "Kovan Tester")
            .env("GIT_COMMITTER_EMAIL", "tester@example.com")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .output()
            .expect("run git");
        assert!(
            status.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&status.stderr)
        );
    }

    /// A temp repository with two commits:
    ///  - commit 1: `README.md` + `src/lib.rs`
    ///  - commit 2: modifies `src/lib.rs` only
    ///
    /// So `README.md`'s last commit is #1 and `src/lib.rs`'s is #2.
    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();

        git(root, &["init", "-q", "-b", "main"]);

        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("README.md"), "# Fixture\n").unwrap();
        fs::write(root.join("src/lib.rs"), "pub fn one() {}\n").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-q", "-m", "initial: readme + lib"]);

        fs::write(
            root.join("src/lib.rs"),
            "pub fn one() {}\npub fn two() {}\n",
        )
        .unwrap();
        git(root, &["add", "src/lib.rs"]);
        git(root, &["commit", "-q", "-m", "add two() to lib"]);

        dir
    }

    #[test]
    fn discovers_repo_and_reports_root_and_branch() {
        let repo = fixture();
        // Discover from a subdirectory to exercise the walk-up.
        let provider = GitProvider::open(&repo.path().join("src")).expect("discover");
        assert_eq!(provider.backend_kind(), BackendKind::Library);

        // Root should be the working tree (canonicalise both: macOS /var symlink etc.).
        assert_eq!(
            fs::canonicalize(provider.repo_root()).unwrap(),
            fs::canonicalize(repo.path()).unwrap()
        );

        let head = provider.head().expect("head");
        assert_eq!(head.branch.as_deref(), Some("main"));
        assert!(!head.detached);
        assert_eq!(head.commit.summary, "add two() to lib");
        assert_eq!(head.commit.id.len(), 40);
        assert_eq!(head.commit.short_id, &head.commit.id[..12]);
        assert_eq!(head.commit.author_name, "Kovan Tester");
    }

    #[test]
    fn open_on_non_repo_is_not_a_repository() {
        let dir = tempfile::tempdir().unwrap();
        match GitProvider::open(dir.path()) {
            Err(GitError::NotARepository(_)) => {}
            Err(e) => panic!("expected NotARepository, got error {e:?}"),
            Ok(_) => panic!("expected NotARepository, got an opened provider"),
        }
    }

    #[test]
    fn tracked_files_lists_what_git_versions_sorted() {
        let repo = fixture();
        // An untracked file must NOT appear (contrast with the filesystem walk).
        fs::write(repo.path().join("scratch.txt"), "untracked\n").unwrap();

        let provider = GitProvider::open(repo.path()).expect("open");
        let tracked = provider.tracked_files().expect("tracked");

        assert_eq!(
            tracked,
            vec![PathBuf::from("README.md"), PathBuf::from("src/lib.rs")],
            "only the two committed files, sorted, and no untracked scratch.txt"
        );
    }

    #[test]
    fn history_is_newest_first_and_bounded() {
        let repo = fixture();
        let provider = GitProvider::open(repo.path()).expect("open");

        let all = provider.history(10).expect("history");
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].summary, "add two() to lib");
        assert_eq!(all[1].summary, "initial: readme + lib");
        // Newest-first: the head commit's time is >= the parent's.
        assert!(all[0].committed_unix_seconds >= all[1].committed_unix_seconds);

        let bounded = provider.history(1).expect("history");
        assert_eq!(bounded.len(), 1);
        assert_eq!(bounded[0].summary, "add two() to lib");
    }

    #[test]
    fn last_commit_for_path_attributes_each_file() {
        let repo = fixture();
        let provider = GitProvider::open(repo.path()).expect("open");

        let readme = provider
            .last_commit_for_path(Path::new("README.md"))
            .expect("query")
            .expect("readme has history");
        assert_eq!(
            readme.summary, "initial: readme + lib",
            "README.md was only touched by the first commit"
        );

        let lib = provider
            .last_commit_for_path(Path::new("src/lib.rs"))
            .expect("query")
            .expect("lib has history");
        assert_eq!(
            lib.summary, "add two() to lib",
            "src/lib.rs was last touched by the second commit"
        );

        // A path never committed has no last commit.
        assert!(provider
            .last_commit_for_path(Path::new("does/not/exist.rs"))
            .expect("query")
            .is_none());
    }

    #[test]
    fn history_for_path_only_lists_touching_commits() {
        let repo = fixture();
        let provider = GitProvider::open(repo.path()).expect("open");

        let readme_hist = provider
            .history_for_path(Path::new("README.md"), 10)
            .expect("history");
        assert_eq!(
            readme_hist.len(),
            1,
            "README.md changed in exactly one commit"
        );
        assert_eq!(readme_hist[0].summary, "initial: readme + lib");

        let lib_hist = provider
            .history_for_path(Path::new("src/lib.rs"), 10)
            .expect("history");
        assert_eq!(lib_hist.len(), 2, "src/lib.rs changed in both commits");
        assert_eq!(lib_hist[0].summary, "add two() to lib");
        assert_eq!(lib_hist[1].summary, "initial: readme + lib");
    }

    #[test]
    fn blame_attributes_lines_to_their_introducing_commit() {
        let repo = fixture();
        let provider = GitProvider::open(repo.path()).expect("open");

        // Line 1 (`pub fn one()`) came from the first commit.
        let line1 = provider
            .blame_line(Path::new("src/lib.rs"), 1)
            .expect("blame")
            .expect("line 1 blamable");
        assert_eq!(line1.summary, "initial: readme + lib");

        // Line 2 (`pub fn two()`) came from the second commit.
        let line2 = provider
            .blame_line(Path::new("src/lib.rs"), 2)
            .expect("blame")
            .expect("line 2 blamable");
        assert_eq!(line2.summary, "add two() to lib");
    }

    #[test]
    fn worktree_dirty_flag_tracks_edits() {
        let repo = fixture();
        let provider = GitProvider::open(repo.path()).expect("open");

        assert!(
            !provider.is_worktree_dirty().expect("clean"),
            "fresh checkout is clean"
        );

        // Modify a tracked file → dirty.
        fs::write(
            repo.path().join("src/lib.rs"),
            "pub fn one() {}\n// edited\n",
        )
        .unwrap();
        assert!(
            provider.is_worktree_dirty().expect("dirty"),
            "edited tracked file is dirty"
        );
    }

    #[test]
    fn binary_backend_declines_structured_ops_without_panicking() {
        let repo = fixture();
        // The gix CLI is not assumed installed in CI; every structured op must
        // return a clean error (Unsupported / BinaryUnavailableOnTarget), never
        // panic. `is_available()` may be true or false depending on the host.
        let provider = GitProvider::binary(repo.path());
        assert_eq!(provider.backend_kind(), BackendKind::Binary);
        assert!(provider.head().is_err());
        assert!(provider.tracked_files().is_err());
        assert!(provider.history(1).is_err());
        assert!(provider
            .last_commit_for_path(Path::new("README.md"))
            .is_err());
        assert!(provider.blame_line(Path::new("src/lib.rs"), 1).is_err());
        assert!(provider.is_worktree_dirty().is_err());
    }

    #[test]
    fn open_preferring_library_uses_library_for_a_real_repo() {
        let repo = fixture();
        let provider = GitProvider::open_preferring_library(repo.path()).expect("open");
        assert_eq!(
            provider.backend_kind(),
            BackendKind::Library,
            "a real repo must resolve via the library, not the fallback"
        );
    }
}
