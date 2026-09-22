//! §37 "Save Document vs Save Repository" (`op-9vo6.19`).
//!
//! Two distinct, clearly labelled operations:
//!
//! - **Save Document** ([`crate::session::PaperSession::save_document`],
//!   built by `op-9vo6.10`) writes the current buffer to disk. No staging,
//!   no commit.
//! - **Save Repository** (this module) is the friendly abstraction over
//!   `git add .` + `git commit`, built directly on `gix` rather than
//!   shelling out, producing a **deterministic, no-AI** commit summary.
//!
//! # Restricted PDFs are structurally excluded, not just gitignored
//!
//! §46's "Save Repository" acceptance scenario requires restricted PDFs
//! excluded from the staged set. [`is_excluded`] enforces that at the
//! tree-building level — it is a property of this code, not something
//! that merely happens to follow from a `.gitignore` a caller could have
//! deleted, misedited, or bypassed some other way.
//!
//! # Why this walks the worktree instead of using `.git/index`
//!
//! "`git add .` + `git commit`" is implemented here as "build a tree that
//! matches the current (non-excluded) worktree exactly, and commit it" —
//! conceptually equivalent, and far simpler than staging into and reading
//! back a real Git index file. `gix`'s object-writing calls
//! ([`gix::Repository::write_blob`]/`write_object`) already deduplicate by
//! content hash, so re-saving unchanged files costs nothing extra.
//!
//! # Submodules are gitlinks, never walked
//!
//! Git would never flatten another repository into this one, so this does
//! not either: a directory with its own `.git` is skipped, and each
//! registered submodule (`.gitmodules`: the standard, open and proprietary
//! corpora, #255) is recorded as a gitlink at its current commit, or at the
//! commit already recorded when it is not downloaded. Before 2026-09-22 only
//! the private submodule was handled, and a Save flattened the standard
//! corpus into a real Kovan repository and dropped all three corpora.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use gix::objs::{tree, Tree};

use crate::root::KovanRoot;

#[derive(Debug)]
pub enum RepositoryError {
    NotAGitRepository {
        path: PathBuf,
    },
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Git(String),
}

impl std::fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAGitRepository { path } => {
                write!(f, "{}: not a git repository", path.display())
            }
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Git(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for RepositoryError {}

/// A deterministic, no-AI summary of what a Save Repository would change
/// (or just committed) — §37's "Added: ... / Edited: ... / Removed: ...".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SaveSummary {
    pub added: Vec<String>,
    pub changed: Vec<String>,
    pub removed: Vec<String>,
}

impl SaveSummary {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.changed.is_empty() && self.removed.is_empty()
    }

    pub fn total(&self) -> usize {
        self.added.len() + self.changed.len() + self.removed.len()
    }

    /// Render as a commit message body.
    pub fn to_commit_message(&self) -> String {
        let mut out = String::from("Save Kovan repository\n");
        for (label, paths) in [
            ("Added", &self.added),
            ("Edited", &self.changed),
            ("Removed", &self.removed),
        ] {
            if paths.is_empty() {
                continue;
            }
            out.push('\n');
            out.push_str(label);
            out.push_str(":\n");
            for p in paths {
                out.push_str("- ");
                out.push_str(p);
                out.push('\n');
            }
        }
        out
    }
}

/// Whether `path` (absolute, under `root`) must never be part of a
/// Save-Repository tree — §4/§46: restricted source documents, Kovan's own
/// disposable state, and `.git` itself.
///
/// Restricted-source documents are excluded **unconditionally**, whether or
/// not a private submodule is configured (`op-3gxp`) — [`build_tree`]
/// separately injects a single gitlink entry for that directory when a
/// ready private submodule exists (see [`SubmoduleGitlink`]); its contents
/// must still never be walked and flattened into the *parent* repository's
/// own tree, or the whole point of a separate private repository is lost.
fn is_excluded(root: &KovanRoot, path: &Path) -> bool {
    path.starts_with(root.restricted_sources_dir())
        || path.starts_with(root.state_dir())
        || path.components().any(|c| c.as_os_str() == ".git")
}

/// Recursively collect every file under `dir` (relative to `base`) that
/// `excluded` does not reject, as `(absolute_path, path_relative_to_base)`
/// pairs. The shared walker behind both [`build_tree`] (the parent
/// repository, excluding restricted sources/`.kovan`/`.git`) and
/// [`save_private_submodule`] (the private submodule's own worktree,
/// excluding only its own `.git`).
fn collect_files(
    base: &Path,
    dir: &Path,
    excluded: &impl Fn(&Path) -> bool,
    out: &mut Vec<(PathBuf, PathBuf)>,
) {
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if excluded(&path) {
            continue;
        }
        if path.is_dir() {
            // Another repository (a corpus submodule, or any nested clone)
            // is never flattened into this one: a registered submodule is
            // recorded as a gitlink instead ([`submodule_gitlinks`]).
            if path.join(".git").exists() {
                continue;
            }
            collect_files(base, &path, excluded, out);
        } else if path.is_file() {
            if let Ok(rel) = path.strip_prefix(base) {
                out.push((path.clone(), rel.to_path_buf()));
            }
        }
    }
}

/// A private submodule's current commit, to be recorded as a gitlink entry
/// (Git's own mechanism for "this path is another repository, pinned at
/// this commit" — mode `0o160000`, [`tree::EntryKind::Commit`]) in the
/// parent tree [`build_tree`] produces, instead of that path's contents
/// being walked and flattened into the parent tree directly.
struct SubmoduleGitlink {
    /// The path the submodule is mounted at, relative to the library root
    /// — [`crate::root::RootPaths::restricted_sources`].
    path: PathBuf,
    /// The submodule's own current `HEAD` commit id.
    commit: gix::ObjectId,
}

/// Build a tree object from every non-excluded file under `base`
/// (recursively), and a flat `relative path -> content id` map of
/// everything in it (used to diff against a previous commit). `gitlink`,
/// when given, adds one additional `Commit`-mode entry at its own path —
/// see [`SubmoduleGitlink`] — rather than that path's contents being
/// walked at all (they are excluded from `base`'s own walk by
/// [`is_excluded`] regardless of `gitlink`).
fn build_tree_from(
    repo: &gix::Repository,
    base: &Path,
    excluded: impl Fn(&Path) -> bool,
    gitlinks: &[SubmoduleGitlink],
) -> Result<(gix::ObjectId, BTreeMap<String, gix::ObjectId>), RepositoryError> {
    let linked: Vec<PathBuf> = gitlinks.iter().map(|l| base.join(&l.path)).collect();
    let excluded = |path: &Path| excluded(path) || linked.iter().any(|l| path.starts_with(l));
    let mut files = Vec::new();
    collect_files(base, base, &excluded, &mut files);

    let mut blobs: BTreeMap<String, gix::ObjectId> = BTreeMap::new();
    let mut dir_files: BTreeMap<PathBuf, Vec<tree::Entry>> = BTreeMap::new();
    let mut all_dirs: BTreeSet<PathBuf> = BTreeSet::new();
    all_dirs.insert(PathBuf::new());

    let mut register_entry = |rel: &Path, mode: tree::EntryMode, oid: gix::ObjectId| {
        blobs.insert(rel.to_string_lossy().replace('\\', "/"), oid);

        let parent = rel.parent().unwrap_or_else(|| Path::new("")).to_path_buf();
        let filename = rel
            .file_name()
            .expect("an entry has a name")
            .to_string_lossy()
            .into_owned();
        dir_files
            .entry(parent.clone())
            .or_default()
            .push(tree::Entry {
                mode,
                filename: filename.into(),
                oid,
            });

        let mut cursor = parent.as_path();
        loop {
            all_dirs.insert(cursor.to_path_buf());
            if cursor == Path::new("") {
                break;
            }
            cursor = cursor.parent().unwrap_or_else(|| Path::new(""));
        }
    };

    for (abs, rel) in &files {
        let bytes = std::fs::read(abs).map_err(|source| RepositoryError::Io {
            path: abs.clone(),
            source,
        })?;
        let oid = repo
            .write_blob(&bytes)
            .map_err(|e| RepositoryError::Git(e.to_string()))?
            .detach();
        register_entry(rel, tree::EntryKind::Blob.into(), oid);
    }

    for link in gitlinks {
        register_entry(&link.path, tree::EntryKind::Commit.into(), link.commit);
    }
    drop(register_entry);

    let mut dirs: Vec<PathBuf> = all_dirs.into_iter().collect();
    dirs.sort_by_key(|p| std::cmp::Reverse(p.components().count()));

    let mut written: BTreeMap<PathBuf, gix::ObjectId> = BTreeMap::new();
    for dir in &dirs {
        let mut entries = dir_files.remove(dir).unwrap_or_default();
        for candidate in &dirs {
            if candidate != dir && candidate.parent() == Some(dir.as_path()) {
                if let Some(&child_oid) = written.get(candidate) {
                    let name = candidate
                        .file_name()
                        .expect("non-root has a name")
                        .to_string_lossy()
                        .into_owned();
                    entries.push(tree::Entry {
                        mode: tree::EntryKind::Tree.into(),
                        filename: name.into(),
                        oid: child_oid,
                    });
                }
            }
        }
        entries.sort();
        let oid = repo
            .write_object(Tree { entries })
            .map_err(|e| RepositoryError::Git(e.to_string()))?
            .detach();
        written.insert(dir.clone(), oid);
    }

    let root_oid = *written
        .get(&PathBuf::new())
        .expect("root directory is always present");
    Ok((root_oid, blobs))
}

/// Build a tree object representing `root`'s current (non-excluded)
/// worktree, and a flat `relative path -> blob id` map of everything in it
/// (used to diff against the previous commit). `gitlink` records a ready
/// private submodule's current commit as a single gitlink entry instead of
/// that directory's contents — see [`build_tree_from`].
fn build_tree(
    repo: &gix::Repository,
    root: &KovanRoot,
    gitlinks: &[SubmoduleGitlink],
) -> Result<(gix::ObjectId, BTreeMap<String, gix::ObjectId>), RepositoryError> {
    build_tree_from(repo, root.path(), |path| is_excluded(root, path), gitlinks)
}

/// Flatten `tree_id`'s contents (recursively) into a `path -> blob id` map,
/// the same shape [`build_tree`] returns — the basis for diffing against a
/// previous commit.
fn flatten_tree(
    repo: &gix::Repository,
    tree_id: gix::ObjectId,
    prefix: &str,
    out: &mut BTreeMap<String, gix::ObjectId>,
) -> Result<(), RepositoryError> {
    let tree = repo
        .find_tree(tree_id)
        .map_err(|e| RepositoryError::Git(e.to_string()))?;
    let decoded = tree
        .decode()
        .map_err(|e| RepositoryError::Git(e.to_string()))?;
    for entry in decoded.entries.iter() {
        let name = entry.filename.to_string();
        let path = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        if entry.mode.is_tree() {
            flatten_tree(repo, entry.oid.to_owned(), &path, out)?;
        } else {
            out.insert(path, entry.oid.to_owned());
        }
    }
    Ok(())
}

/// Diff `new_blobs` (the current worktree) against `HEAD`'s tree, if any.
fn diff_against_head(
    repo: &gix::Repository,
    new_blobs: &BTreeMap<String, gix::ObjectId>,
) -> Result<SaveSummary, RepositoryError> {
    let old_blobs = match repo.head_id() {
        Ok(head) => {
            let commit = repo
                .find_commit(head.detach())
                .map_err(|e| RepositoryError::Git(e.to_string()))?;
            let tree_id = commit
                .tree_id()
                .map_err(|e| RepositoryError::Git(e.to_string()))?
                .detach();
            let mut map = BTreeMap::new();
            flatten_tree(repo, tree_id, "", &mut map)?;
            map
        }
        Err(_) => BTreeMap::new(), // unborn HEAD: no previous commit, everything is new.
    };

    let mut summary = SaveSummary::default();
    for (path, oid) in new_blobs {
        match old_blobs.get(path) {
            None => summary.added.push(path.clone()),
            Some(old_oid) if old_oid != oid => summary.changed.push(path.clone()),
            Some(_) => {}
        }
    }
    for path in old_blobs.keys() {
        if !new_blobs.contains_key(path) {
            summary.removed.push(path.clone());
        }
    }
    summary.added.sort();
    summary.changed.sort();
    summary.removed.sort();
    Ok(summary)
}

/// Open `root` as a `gix` repository, or fail with a clear error if it
/// isn't one yet — the caller's cue to offer `§2`'s "initialise Git"
/// prompt rather than this module doing so implicitly.
fn open(root: &KovanRoot) -> Result<gix::Repository, RepositoryError> {
    if !root.has_git() {
        return Err(RepositoryError::NotAGitRepository {
            path: root.path().to_path_buf(),
        });
    }
    gix::open(root.path()).map_err(|e| RepositoryError::Git(e.to_string()))
}

/// Commit `tree_id` into `repo`'s history with the same deterministic
/// technical identity every Save Repository commit uses — not ambient
/// `user.name`/`user.email` config this environment may not have set (§37
/// requires the summary to be deterministic; the author identity should not
/// depend on what happens to be configured on the machine running it).
/// Shared between [`save_repository`] (the parent repository) and
/// [`save_private_submodule`] (a private submodule's own history).
fn commit_tree(
    repo: &mut gix::Repository,
    tree_id: gix::ObjectId,
    message: String,
) -> Result<gix::ObjectId, RepositoryError> {
    let mut snapshot = repo.config_snapshot_mut();
    snapshot
        .append_config(
            ["user.name=Kovan", "user.email=kovan@localhost"],
            gix::config::Source::Api,
        )
        .map_err(|e| RepositoryError::Git(e.to_string()))?;
    snapshot
        .commit()
        .map_err(|e| RepositoryError::Git(e.to_string()))?;

    let parents: Vec<gix::ObjectId> = match repo.head_id() {
        Ok(id) => vec![id.detach()],
        Err(_) => Vec::new(),
    };
    let commit_id = repo
        .commit("HEAD", message, tree_id, parents)
        .map_err(|e| RepositoryError::Git(e.to_string()))?;
    Ok(commit_id.detach())
}

/// The private submodule's current `HEAD` commit id, read-only — used by
/// [`status`] to preview a gitlink entry without writing anything (unlike
/// [`save_private_submodule`], which commits). `Ok(None)` for a submodule
/// repository with no commits yet.
///
/// **Limitation, documented rather than hidden:** this reads the
/// submodule's *last commit*, not a preview of what committing its current
/// worktree would produce — so [`status`] does not surface an in-flight,
/// not-yet-committed change inside the submodule itself as a pending
/// parent-repository change. Previewing that would mean building (though
/// not writing) a full second tree on every status check; not worth it for
/// a lightweight, frequently-polled preview. [`save_repository`] itself has
/// no such gap — it always commits the submodule's actual current state.
fn private_submodule_head(root: &KovanRoot) -> Result<Option<gix::ObjectId>, RepositoryError> {
    let sub_repo = gix::open(root.restricted_sources_dir())
        .map_err(|e| RepositoryError::Git(e.to_string()))?;
    Ok(match sub_repo.head_id() {
        Ok(id) => Some(id.detach()),
        Err(_) => None,
    })
}

/// Commit the private literature submodule's own worktree — everything
/// under `root.restricted_sources_dir()`, excluding only its own `.git` —
/// mirroring [`save_repository`]'s parent-repository logic but scoped to
/// that directory's own history.
///
/// Called from [`save_repository`] **before** any parent-repository write,
/// and its error propagates immediately via `?` — `op-3gxp`'s hard
/// requirement that a private-repository save failure must never let a
/// parent commit referencing an invalid/uncommitted submodule state be
/// created. Since nothing about the parent repository has been touched by
/// the time this runs, an error here simply ends [`save_repository`] with
/// nothing written anywhere, which is exactly the safe failure mode.
///
/// Returns the submodule's current `HEAD` commit id: freshly committed if
/// its worktree had changes, or its pre-existing `HEAD` if it did not.
/// `Ok(None)` only for a submodule repository with no commits at all yet
/// (nothing a gitlink could point at) — [`save_repository`] falls back to
/// excluding the directory entirely in that case, same as an unconfigured
/// or not-ready private submodule.
fn save_private_submodule(root: &KovanRoot) -> Result<Option<gix::ObjectId>, RepositoryError> {
    let submodule_dir = root.restricted_sources_dir();
    let mut sub_repo =
        gix::open(&submodule_dir).map_err(|e| RepositoryError::Git(e.to_string()))?;

    let excluded = |path: &Path| path.components().any(|c| c.as_os_str() == ".git");
    let (tree_id, blobs) = build_tree_from(&sub_repo, &submodule_dir, excluded, &[])?;
    let summary = diff_against_head(&sub_repo, &blobs)?;

    if summary.is_empty() {
        return Ok(match sub_repo.head_id() {
            Ok(id) => Some(id.detach()),
            Err(_) => None,
        });
    }

    let commit_id = commit_tree(&mut sub_repo, tree_id, summary.to_commit_message())?;
    Ok(Some(commit_id))
}

/// Make sure `.gitmodules` at the library root registers the configured
/// private submodule, so real `git submodule` tooling (not just Kovan)
/// recognises it. Other entries (the corpus submodules) are kept.
///
/// ~~Always rewritten to the current single-submodule mapping, since Kovan
/// supports exactly one private literature submodule per library.~~
/// **CORRECTED 2026-09-22**: a Kovan folder now has up to three corpus
/// submodules (#255), and a rewrite would have unregistered the other two.
fn write_gitmodules(
    root: &KovanRoot,
    submodule: &crate::root::PrivateSubmoduleConfig,
) -> Result<(), RepositoryError> {
    let rel = root
        .config()
        .paths
        .restricted_sources
        .to_string_lossy()
        .replace('\\', "/");
    if registered_submodules(root)
        .iter()
        .any(|p| p == Path::new(&rel))
    {
        return Ok(());
    }
    let path = root.path().join(".gitmodules");
    let mut contents = std::fs::read_to_string(&path).unwrap_or_default();
    if !contents.is_empty() && !contents.ends_with('\n') {
        contents.push('\n');
    }
    contents.push_str(&format!(
        "[submodule \"{rel}\"]\n\tpath = {rel}\n\turl = {}\n",
        submodule.remote
    ));
    std::fs::write(&path, contents).map_err(|source| RepositoryError::Io { path, source })
}

/// The submodule paths `.gitmodules` registers, relative to the root.
fn registered_submodules(root: &KovanRoot) -> Vec<PathBuf> {
    std::fs::read_to_string(root.path().join(".gitmodules"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.trim().strip_prefix("path"))
        .filter_map(|rest| rest.trim_start().strip_prefix('='))
        .map(|p| PathBuf::from(p.trim()))
        .collect()
}

/// The commit a gitlink at `rel` records in `repo`'s `HEAD`, if any: what a
/// submodule that is not downloaded yet keeps, rather than being dropped.
fn gitlink_in_head(repo: &gix::Repository, rel: &Path) -> Option<gix::ObjectId> {
    let head = repo.head_id().ok()?.detach();
    let mut current = repo.find_commit(head).ok()?.tree_id().ok()?.detach();
    let mut components: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    let last = components.pop()?;
    for comp in &components {
        let t = repo.find_tree(current).ok()?;
        let decoded = t.decode().ok()?;
        let e = decoded
            .entries
            .iter()
            .find(|e| e.filename == comp.as_bytes() && e.mode.is_tree())?;
        current = e.oid.to_owned();
    }
    let t = repo.find_tree(current).ok()?;
    let decoded = t.decode().ok()?;
    decoded
        .entries
        .iter()
        .find(|e| e.filename == last.as_bytes() && e.mode.is_commit())
        .map(|e| e.oid.to_owned())
}

/// The current `HEAD` of the repository at `dir`, if it is one with a commit.
fn repo_head(dir: &Path) -> Option<gix::ObjectId> {
    if !dir.join(".git").exists() {
        return None;
    }
    gix::open(dir).ok()?.head_id().ok().map(|id| id.detach())
}

/// Commit everything in the open-corpus repository at `dir` with the
/// system `git`, so its own `.gitignore` is respected (the open corpus is
/// often shared, e.g. `reactor-literature`, and has build files of its
/// own). The user's Git identity is used when set, Kovan's otherwise.
/// Without `git`, nothing is committed and the current `HEAD` is recorded.
fn commit_open_corpus(dir: &Path) -> Result<(), RepositoryError> {
    use std::process::Command;
    if !crate::advanced_git::system_git_available() {
        return Ok(());
    }
    let git = |args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .map_err(|source| RepositoryError::Io {
                path: dir.to_path_buf(),
                source,
            })
    };
    let failed = |what: &str, o: &std::process::Output| {
        RepositoryError::Git(format!(
            "open corpus {what}: {}",
            String::from_utf8_lossy(&o.stderr).trim()
        ))
    };
    let add = git(&["add", "-A"])?;
    if !add.status.success() {
        return Err(failed("git add", &add));
    }
    if git(&["diff", "--cached", "--quiet"])?.status.success() {
        return Ok(()); // nothing staged
    }
    let has_identity = git(&["config", "user.email"])?.status.success();
    let mut args = Vec::new();
    if !has_identity {
        args.extend(["-c", "user.name=Kovan", "-c", "user.email=kovan@localhost"]);
    }
    args.extend(["commit", "-q", "-m", "Save Kovan repository: open corpus"]);
    let commit = git(&args)?;
    if commit.status.success() {
        Ok(())
    } else {
        Err(failed("git commit", &commit))
    }
}

/// The gitlinks the Kovan repository's tree records: one per registered
/// submodule (`.gitmodules`), plus the private submodule when it is ready.
///
/// Each is recorded at its repository's current `HEAD`; one not downloaded
/// yet keeps the commit already in `HEAD` (never dropped, the defect that
/// removed all three corpus submodules from a real Kovan repository on
/// 2026-09-22). With `commit`, the user's own corpora are committed first:
/// the private one as before ([`save_private_submodule`]), the open one
/// with Git ([`commit_open_corpus`]). The standard corpus is never
/// committed into: it is read-only to everyone but its maintainer.
fn submodule_gitlinks(
    root: &KovanRoot,
    repo: &gix::Repository,
    commit: bool,
) -> Result<Vec<SubmoduleGitlink>, RepositoryError> {
    let paths = &root.config().paths;
    let mut rels = registered_submodules(root);
    let private_ready = root.private_submodule_ready();
    if private_ready && !rels.contains(&paths.restricted_sources) {
        rels.push(paths.restricted_sources.clone());
    }
    let mut links = Vec::new();
    for rel in rels {
        let dir = root.path().join(&rel);
        let head = if rel == paths.restricted_sources {
            // Registered (setup's `git submodule add`) or configured as the
            // private submodule. Its files are never walked into this tree
            // either way ([`is_excluded`]).
            if repo_head(&dir).is_none() && !private_ready {
                None
            } else if commit {
                save_private_submodule(root)?
            } else {
                private_submodule_head(root)?
            }
        } else {
            if commit && rel == paths.open_sources && repo_head(&dir).is_some() {
                commit_open_corpus(&dir)?;
            }
            repo_head(&dir)
        };
        if let Some(commit) = head.or_else(|| gitlink_in_head(repo, &rel)) {
            links.push(SubmoduleGitlink { path: rel, commit });
        }
    }
    Ok(links)
}

/// What would change if [`save_repository`] ran right now — the "N changes
/// since last repository save" the UI shows (§37) — without writing
/// anything. See [`private_submodule_head`] for the one documented gap in
/// that guarantee's coverage.
pub fn status(root: &KovanRoot) -> Result<SaveSummary, RepositoryError> {
    let repo = open(root)?;
    let gitlinks = submodule_gitlinks(root, &repo, false)?;
    let (_tree_id, blobs) = build_tree(&repo, root, &gitlinks)?;
    diff_against_head(&repo, &blobs)
}

/// §37's "Save Repository": build a tree from the current (non-excluded)
/// worktree and commit it with a deterministic summary message, no AI.
/// Returns `Ok(None)` — a no-op — when there is nothing to commit.
///
/// `op-3gxp`: when a private literature submodule is configured and ready
/// (see [`crate::root::KovanRoot::private_submodule_ready`]), its own
/// worktree is committed **first** ([`save_private_submodule`]), before
/// anything about the parent repository is touched — a failure there aborts
/// this whole call via `?`, so a parent commit can never reference an
/// invalid or uncommitted submodule state. The parent tree then records the
/// submodule's current commit as a gitlink entry (see [`SubmoduleGitlink`])
/// instead of walking its contents, and `.gitmodules` is written/refreshed
/// so real `git submodule` tooling recognises it too. When no private
/// submodule is configured or ready, behaviour is unchanged from before
/// this existed: the directory is excluded from the parent tree entirely,
/// same as any other gitignored, local-only content.
pub fn save_repository(root: &KovanRoot) -> Result<Option<SaveSummary>, RepositoryError> {
    let mut repo = open(root)?;

    if root.private_submodule_ready() {
        if let Some(submodule) = root.private_submodule() {
            write_gitmodules(root, submodule)?;
        }
    }
    let gitlinks = submodule_gitlinks(root, &repo, true)?;

    let (tree_id, blobs) = build_tree(&repo, root, &gitlinks)?;
    let summary = diff_against_head(&repo, &blobs)?;
    if summary.is_empty() {
        return Ok(None);
    }

    commit_tree(&mut repo, tree_id, summary.to_commit_message())?;
    Ok(Some(summary))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{Access, CiteKey, EntityConfig};
    use crate::root::RootConfig;

    fn make_root() -> (tempfile::TempDir, KovanRoot) {
        let dir = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(dir.path(), RootConfig::new("lib", "Lib"), true).unwrap();
        (dir, root)
    }

    /// A library with a private literature submodule configured AND
    /// actually initialised as its own git repository — i.e.
    /// `private_submodule_ready()` is true.
    fn make_root_with_private_submodule() -> (tempfile::TempDir, KovanRoot) {
        let dir = tempfile::tempdir().unwrap();
        let config = RootConfig::new("lib", "Lib")
            .with_private_submodule("git@example.com:org/private-literature.git");
        let root = KovanRoot::create(dir.path(), config, true).unwrap();
        gix::init(root.restricted_sources_dir()).unwrap();
        (dir, root)
    }

    /// The `EntryMode` at `path` inside `tree_id`, descending through
    /// intermediate directories — used to confirm a gitlink was recorded
    /// as a `Commit`-mode entry, not walked as an ordinary `Tree`.
    fn entry_mode_at(
        repo: &gix::Repository,
        tree_id: gix::ObjectId,
        path: &Path,
    ) -> Option<tree::EntryMode> {
        let mut components: Vec<String> = path
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        let last = components.pop()?;
        let mut current = tree_id;
        for comp in &components {
            let decoded_tree = repo.find_tree(current).ok()?;
            let decoded = decoded_tree.decode().ok()?;
            current = decoded
                .entries
                .iter()
                .find(|e| e.filename == comp.as_bytes())?
                .oid
                .to_owned();
        }
        let decoded_tree = repo.find_tree(current).ok()?;
        let decoded = decoded_tree.decode().ok()?;
        decoded
            .entries
            .iter()
            .find(|e| e.filename == last.as_bytes())
            .map(|e| e.mode)
    }

    #[test]
    fn first_save_repository_commits_the_skeleton() {
        let (_dir, root) = make_root();
        let summary = save_repository(&root)
            .unwrap()
            .expect("a fresh library has files to commit");
        assert!(summary.added.iter().any(|p| p == "kovan_root.toml"));
        assert!(summary.added.iter().any(|p| p == ".gitignore"));
        assert!(summary.changed.is_empty());
        assert!(summary.removed.is_empty());
    }

    #[test]
    fn a_second_save_with_no_changes_is_a_no_op() {
        let (_dir, root) = make_root();
        save_repository(&root).unwrap();
        let second = save_repository(&root).unwrap();
        assert!(second.is_none());
    }

    #[test]
    fn adding_a_paper_then_saving_reports_it_as_added() {
        let (_dir, root) = make_root();
        save_repository(&root).unwrap();

        EntityConfig::paper(
            CiteKey::parse("wang2018multiphysics").unwrap(),
            Access::Open,
        )
        .with_topics(["htgrs"])
        .save_paper(&root.paper_dir("wang2018multiphysics"))
        .unwrap();

        let summary = save_repository(&root).unwrap().unwrap();
        assert!(
            summary
                .added
                .iter()
                .any(|p| p.contains("wang2018multiphysics"))
        );
    }

    #[test]
    fn restricted_pdfs_are_never_part_of_the_saved_tree() {
        let (_dir, root) = make_root();
        std::fs::create_dir_all(root.restricted_sources_dir()).unwrap();
        std::fs::write(
            root.restricted_sources_dir().join("secret.pdf"),
            b"proprietary bytes",
        )
        .unwrap();

        let summary = save_repository(&root).unwrap().unwrap();
        assert!(
            !summary.added.iter().any(|p| p.contains("secret.pdf")),
            "{:?}",
            summary.added
        );

        // Structural, not gitignore-convention: even with the restricted
        // directory's .gitignore pattern removed, the same file must not
        // be staged.
        let gitignore_path = root.path().join(".gitignore");
        std::fs::write(&gitignore_path, "").unwrap();
        let summary = save_repository(&root).unwrap();
        let flat = summary.map(|s| s.added).unwrap_or_default();
        assert!(!flat.iter().any(|p| p.contains("secret.pdf")));
    }

    #[test]
    fn status_reports_without_committing() {
        let (_dir, root) = make_root();
        let before = status(&root).unwrap();
        assert!(!before.is_empty());
        // Calling status again gives the same answer — it must not have committed.
        let again = status(&root).unwrap();
        assert_eq!(before, again);
    }

    #[test]
    fn editing_a_tracked_file_then_saving_reports_it_as_changed() {
        let (_dir, root) = make_root();
        save_repository(&root).unwrap();
        std::fs::write(root.path().join("README.md"), "hello").unwrap();
        let summary = save_repository(&root).unwrap();
        assert!(summary.unwrap().added.iter().any(|p| p == "README.md"));

        std::fs::write(root.path().join("README.md"), "hello again").unwrap();
        let summary = save_repository(&root).unwrap().unwrap();
        assert!(summary.changed.iter().any(|p| p == "README.md"));
    }

    #[test]
    fn not_a_git_repository_is_a_clear_error() {
        let dir = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(dir.path(), RootConfig::new("lib", "Lib"), false).unwrap();
        let err = save_repository(&root).unwrap_err();
        assert!(matches!(err, RepositoryError::NotAGitRepository { .. }));
    }

    // -------------------------------------------------------------------
    // Private literature submodule — op-3gxp, GH issue #35's 2026-09-01
    // amendment
    // -------------------------------------------------------------------

    #[test]
    fn an_unready_private_submodule_still_excludes_the_directory_exactly_as_before() {
        // Configured, but never actually `git init`-ed at that path -- not
        // ready. Must fall back to the exact pre-op-3gxp behaviour: fully
        // excluded, no gitlink, no error.
        let dir = tempfile::tempdir().unwrap();
        let config =
            RootConfig::new("lib", "Lib").with_private_submodule("git@example.com:org/private.git");
        let root = KovanRoot::create(dir.path(), config, true).unwrap();
        assert!(!root.private_submodule_ready());

        std::fs::write(
            root.restricted_sources_dir().join("secret.pdf"),
            b"proprietary bytes",
        )
        .unwrap();
        let summary = save_repository(&root).unwrap().unwrap();
        let rel = root
            .config()
            .paths
            .restricted_sources
            .to_string_lossy()
            .replace('\\', "/");
        assert!(
            !summary
                .added
                .iter()
                .any(|p| p == &rel || p.contains("secret.pdf")),
            "{:?}",
            summary.added
        );
    }

    #[test]
    fn save_repository_commits_the_private_submodule_first_and_records_a_gitlink() {
        let (_dir, root) = make_root_with_private_submodule();
        save_repository(&root).unwrap(); // initial skeleton; submodule is still empty, so still excluded this round.

        std::fs::write(
            root.restricted_sources_dir().join("secret.pdf"),
            b"proprietary bytes",
        )
        .unwrap();
        let summary = save_repository(&root).unwrap().unwrap();

        let rel = root
            .config()
            .paths
            .restricted_sources
            .to_string_lossy()
            .replace('\\', "/");
        // The gitlink path itself is reported, never the file inside it.
        assert!(summary.added.contains(&rel), "{:?}", summary.added);
        assert!(
            !summary.added.iter().any(|p| p.contains("secret.pdf")),
            "{:?}",
            summary.added
        );

        // The submodule genuinely has its own commit now.
        let sub_repo = gix::open(root.restricted_sources_dir()).unwrap();
        let sub_head = sub_repo.head_id().unwrap().detach();

        // The parent tree really records a gitlink (Commit-mode entry)
        // pointing at that exact commit -- not a Tree entry, and not the
        // file's own contents walked into the parent's history at all.
        let parent_repo = gix::open(root.path()).unwrap();
        let parent_head = parent_repo.head_id().unwrap().detach();
        let commit = parent_repo.find_commit(parent_head).unwrap();
        let tree_id = commit.tree_id().unwrap().detach();
        let mode = entry_mode_at(
            &parent_repo,
            tree_id,
            &root.config().paths.restricted_sources,
        )
        .unwrap();
        assert!(
            mode.is_commit(),
            "expected a gitlink (Commit-mode) entry, got {mode:?}"
        );

        let mut flat = BTreeMap::new();
        flatten_tree(&parent_repo, tree_id, "", &mut flat).unwrap();
        assert_eq!(
            flat.get(&rel),
            Some(&sub_head),
            "the gitlink must point at the submodule's actual HEAD"
        );

        // .gitmodules exists and names the right path + remote.
        let gitmodules = std::fs::read_to_string(root.path().join(".gitmodules")).unwrap();
        assert!(gitmodules.contains(&rel), "{gitmodules}");
        assert!(
            gitmodules.contains("git@example.com:org/private-literature.git"),
            "{gitmodules}"
        );
    }

    #[test]
    fn a_second_submodule_content_change_bumps_the_gitlink_and_reports_it_as_changed() {
        let (_dir, root) = make_root_with_private_submodule();
        std::fs::write(root.restricted_sources_dir().join("a.pdf"), b"one").unwrap();
        save_repository(&root).unwrap();

        std::fs::write(root.restricted_sources_dir().join("a.pdf"), b"two, edited").unwrap();
        let summary = save_repository(&root).unwrap().unwrap();

        let rel = root
            .config()
            .paths
            .restricted_sources
            .to_string_lossy()
            .replace('\\', "/");
        assert!(summary.changed.contains(&rel), "{:?}", summary.changed);
    }

    #[test]
    fn a_private_submodule_save_failure_leaves_the_parent_repository_untouched() {
        // Configured and looks ready (a `.git` entry exists) but is not
        // actually a valid repository -- `gix::open` on it must fail, and
        // that failure must propagate before any parent-repository write.
        let dir = tempfile::tempdir().unwrap();
        let config =
            RootConfig::new("lib", "Lib").with_private_submodule("git@example.com:org/private.git");
        let root = KovanRoot::create(dir.path(), config, true).unwrap();
        std::fs::write(
            root.restricted_sources_dir().join(".git"),
            "not a real gitdir pointer",
        )
        .unwrap();
        assert!(
            root.private_submodule_ready(),
            "a .git entry, however invalid, is enough to look ready"
        );

        let parent_repo = gix::open(root.path()).unwrap();
        let head_before = parent_repo.head_id().ok().map(|id| id.detach());

        let err = save_repository(&root).unwrap_err();
        assert!(matches!(err, RepositoryError::Git(_)), "{err}");

        let parent_repo = gix::open(root.path()).unwrap();
        let head_after = parent_repo.head_id().ok().map(|id| id.detach());
        assert_eq!(
            head_before, head_after,
            "a failed private-submodule save must not produce a parent commit"
        );
    }

    #[test]
    fn status_previews_the_gitlink_without_committing_anything() {
        let (_dir, root) = make_root_with_private_submodule();
        std::fs::write(root.restricted_sources_dir().join("a.pdf"), b"one").unwrap();
        save_repository(&root).unwrap(); // seed a real submodule commit to preview against

        std::fs::write(root.path().join("README.md"), "hello").unwrap();
        let before = status(&root).unwrap();
        let after = status(&root).unwrap();
        assert_eq!(
            before, after,
            "status must not itself change what a second status call sees"
        );
        assert!(before.added.iter().any(|p| p == "README.md"));
    }

    // -------------------------------------------------------------------
    // Corpus submodules (#255): the 2026-09-22 regression, in which a Save
    // flattened the standard corpus into a real Kovan repository and
    // dropped all three corpus submodules.
    // -------------------------------------------------------------------

    fn git(dir: &Path, args: &[&str]) {
        let ok = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(args)
            .output()
            .unwrap()
            .status
            .success();
        assert!(ok, "git {args:?}");
    }

    /// A repository at `dir` holding `file`, to stand in for a remote.
    fn source_repo(dir: &Path, file: &str) -> String {
        let f = dir.join(file);
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(&f, b"pdf").unwrap();
        git(dir, &["init", "-q", "-b", "main"]);
        git(dir, &["add", "."]);
        git(dir, &["commit", "-q", "-m", "x"]);
        dir.to_string_lossy().to_string()
    }

    /// A Kovan folder whose three corpora are submodules, as setup makes it.
    fn root_with_corpora(tmp: &Path) -> KovanRoot {
        let standard = source_repo(&tmp.join("std"), "kovan-standard-open-corpus/a.pdf");
        let open = source_repo(&tmp.join("open"), "me-open-corpus/b.pdf");
        let private = source_repo(&tmp.join("private"), "papers/c.pdf");
        let mut root =
            KovanRoot::create(&tmp.join("lib"), RootConfig::new("lib", "Lib"), true).unwrap();
        root.set_corpora(crate::root::CorporaConfig {
            open_remote: Some(open),
            proprietary_remote: Some(private),
        })
        .unwrap();
        let setup = crate::corpus_repos::ensure_library_corpora_with(&root, &standard, "main");
        assert!(setup.standard.is_ok() && setup.open.is_ok() && setup.proprietary.is_ok());
        root
    }

    fn head_tree_paths(root: &KovanRoot) -> BTreeMap<String, gix::ObjectId> {
        let repo = gix::open(root.path()).unwrap();
        let commit = repo.find_commit(repo.head_id().unwrap().detach()).unwrap();
        let mut map = BTreeMap::new();
        flatten_tree(&repo, commit.tree_id().unwrap().detach(), "", &mut map).unwrap();
        map
    }

    /// Every corpus is recorded as a gitlink; none of its files are.
    #[test]
    fn corpora_are_saved_as_gitlinks_never_flattened() {
        if !crate::advanced_git::system_git_available() {
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let root = root_with_corpora(tmp.path());
        save_repository(&root).unwrap().unwrap();
        let repo = gix::open(root.path()).unwrap();
        let head = repo.find_commit(repo.head_id().unwrap().detach()).unwrap();
        let tree = head.tree_id().unwrap().detach();
        for rel in [
            "literature/standard-corpus",
            "literature/open-corpus",
            "literature/proprietary",
        ] {
            let mode = entry_mode_at(&repo, tree, Path::new(rel));
            assert!(mode.is_some_and(|m| m.is_commit()), "{rel}: {mode:?}");
        }
        let paths = head_tree_paths(&root);
        assert!(
            !paths.keys().any(|p| p.ends_with(".pdf")),
            "corpus files flattened into the Kovan repository: {paths:?}"
        );
        assert!(paths.contains_key(".gitmodules"));
    }

    /// Corpora registered but not downloaded (a plain clone, or a download
    /// still running) keep their gitlinks: nothing is removed.
    #[test]
    fn undownloaded_corpora_keep_their_gitlinks() {
        if !crate::advanced_git::system_git_available() {
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let root = root_with_corpora(tmp.path());
        save_repository(&root).unwrap().unwrap();
        let copy = tmp.path().join("copy");
        let ok = std::process::Command::new("git")
            .arg("clone")
            .arg("-q")
            .arg(root.path())
            .arg(&copy)
            .status()
            .unwrap()
            .success();
        assert!(ok);
        let cloned = KovanRoot::open(&copy).unwrap();
        assert!(!cloned.standard_corpus_dir().join(".git").exists());
        assert!(status(&cloned).unwrap().is_empty());
        assert_eq!(save_repository(&cloned).unwrap(), None);
        assert_eq!(head_tree_paths(&cloned), head_tree_paths(&root));
    }

    /// A PDF ingested into the open corpus is committed THERE, and the Kovan
    /// repository records only the bumped gitlink; the standard corpus is
    /// never committed into.
    #[test]
    fn saving_commits_the_open_corpus_and_bumps_only_its_gitlink() {
        if !crate::advanced_git::system_git_available() {
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let root = root_with_corpora(tmp.path());
        save_repository(&root).unwrap().unwrap();
        let standard_before = repo_head(&root.standard_corpus_dir());

        std::fs::write(root.open_corpus_dir().join("me-open-corpus/new.pdf"), b"n").unwrap();
        std::fs::write(root.standard_corpus_dir().join("stray.pdf"), b"s").unwrap();
        let summary = save_repository(&root).unwrap().unwrap();
        assert_eq!(summary.changed, vec!["literature/open-corpus".to_string()]);
        assert!(
            summary.added.is_empty() && summary.removed.is_empty(),
            "{summary:?}"
        );
        assert_eq!(repo_head(&root.standard_corpus_dir()), standard_before);
        let open = gix::open(root.open_corpus_dir()).unwrap();
        let tip = open.find_commit(open.head_id().unwrap().detach()).unwrap();
        let mut files = BTreeMap::new();
        flatten_tree(&open, tip.tree_id().unwrap().detach(), "", &mut files).unwrap();
        assert!(files.contains_key("me-open-corpus/new.pdf"), "{files:?}");
    }

    /// Registering the private submodule keeps the corpus entries in
    /// `.gitmodules` (it used to rewrite the file with its entry alone).
    #[test]
    fn registering_the_private_submodule_keeps_other_entries() {
        let (_dir, root) = make_root_with_private_submodule();
        let gm = root.path().join(".gitmodules");
        std::fs::write(
            &gm,
            "[submodule \"literature/open-corpus\"]\n\tpath = literature/open-corpus\n\turl = u\n",
        )
        .unwrap();
        write_gitmodules(&root, root.private_submodule().unwrap()).unwrap();
        write_gitmodules(&root, root.private_submodule().unwrap()).unwrap();
        let text = std::fs::read_to_string(&gm).unwrap();
        assert!(text.contains("path = literature/open-corpus"), "{text}");
        assert_eq!(
            text.matches("path = literature/proprietary").count(),
            1,
            "{text}"
        );
    }
}
