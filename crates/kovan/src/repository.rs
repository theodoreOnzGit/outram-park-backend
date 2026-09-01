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

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use gix::objs::{tree, Tree};

use crate::root::KovanRoot;

#[derive(Debug)]
pub enum RepositoryError {
    NotAGitRepository { path: PathBuf },
    Io { path: PathBuf, source: std::io::Error },
    Git(String),
}

impl std::fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAGitRepository { path } => write!(f, "{}: not a git repository", path.display()),
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
        for (label, paths) in [("Added", &self.added), ("Edited", &self.changed), ("Removed", &self.removed)] {
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
fn is_excluded(root: &KovanRoot, path: &Path) -> bool {
    path.starts_with(root.restricted_sources_dir())
        || path.starts_with(root.state_dir())
        || path.components().any(|c| c.as_os_str() == ".git")
}

/// Recursively collect every non-excluded file under `dir`, as
/// `(absolute_path, path_relative_to_root)` pairs.
fn collect_files(root: &KovanRoot, dir: &Path, out: &mut Vec<(PathBuf, PathBuf)>) {
    let Ok(read_dir) = std::fs::read_dir(dir) else { return };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if is_excluded(root, &path) {
            continue;
        }
        if path.is_dir() {
            collect_files(root, &path, out);
        } else if path.is_file() {
            if let Ok(rel) = path.strip_prefix(root.path()) {
                out.push((path.clone(), rel.to_path_buf()));
            }
        }
    }
}

/// Build a tree object representing `root`'s current (non-excluded)
/// worktree, and a flat `relative path -> blob id` map of everything in it
/// (used to diff against the previous commit).
fn build_tree(
    repo: &gix::Repository,
    root: &KovanRoot,
) -> Result<(gix::ObjectId, BTreeMap<String, gix::ObjectId>), RepositoryError> {
    let mut files = Vec::new();
    collect_files(root, root.path(), &mut files);

    let mut blobs: BTreeMap<String, gix::ObjectId> = BTreeMap::new();
    let mut dir_files: BTreeMap<PathBuf, Vec<tree::Entry>> = BTreeMap::new();
    let mut all_dirs: BTreeSet<PathBuf> = BTreeSet::new();
    all_dirs.insert(PathBuf::new());

    for (abs, rel) in &files {
        let bytes = std::fs::read(abs).map_err(|source| RepositoryError::Io { path: abs.clone(), source })?;
        let oid = repo.write_blob(&bytes).map_err(|e| RepositoryError::Git(e.to_string()))?.detach();
        blobs.insert(rel.to_string_lossy().replace('\\', "/"), oid);

        let parent = rel.parent().unwrap_or_else(|| Path::new("")).to_path_buf();
        let filename = rel.file_name().expect("a file has a name").to_string_lossy().into_owned();
        dir_files.entry(parent.clone()).or_default().push(tree::Entry {
            mode: tree::EntryKind::Blob.into(),
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
    }

    let mut dirs: Vec<PathBuf> = all_dirs.into_iter().collect();
    dirs.sort_by_key(|p| std::cmp::Reverse(p.components().count()));

    let mut written: BTreeMap<PathBuf, gix::ObjectId> = BTreeMap::new();
    for dir in &dirs {
        let mut entries = dir_files.remove(dir).unwrap_or_default();
        for candidate in &dirs {
            if candidate != dir && candidate.parent() == Some(dir.as_path()) {
                if let Some(&child_oid) = written.get(candidate) {
                    let name = candidate.file_name().expect("non-root has a name").to_string_lossy().into_owned();
                    entries.push(tree::Entry { mode: tree::EntryKind::Tree.into(), filename: name.into(), oid: child_oid });
                }
            }
        }
        entries.sort();
        let oid = repo.write_object(Tree { entries }).map_err(|e| RepositoryError::Git(e.to_string()))?.detach();
        written.insert(dir.clone(), oid);
    }

    let root_oid = *written.get(&PathBuf::new()).expect("root directory is always present");
    Ok((root_oid, blobs))
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
    let tree = repo.find_tree(tree_id).map_err(|e| RepositoryError::Git(e.to_string()))?;
    let decoded = tree.decode().map_err(|e| RepositoryError::Git(e.to_string()))?;
    for entry in decoded.entries.iter() {
        let name = entry.filename.to_string();
        let path = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
        if entry.mode.is_tree() {
            flatten_tree(repo, entry.oid.to_owned(), &path, out)?;
        } else {
            out.insert(path, entry.oid.to_owned());
        }
    }
    Ok(())
}

/// Diff `new_blobs` (the current worktree) against `HEAD`'s tree, if any.
fn diff_against_head(repo: &gix::Repository, new_blobs: &BTreeMap<String, gix::ObjectId>) -> Result<SaveSummary, RepositoryError> {
    let old_blobs = match repo.head_id() {
        Ok(head) => {
            let commit = repo.find_commit(head.detach()).map_err(|e| RepositoryError::Git(e.to_string()))?;
            let tree_id = commit.tree_id().map_err(|e| RepositoryError::Git(e.to_string()))?.detach();
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
        return Err(RepositoryError::NotAGitRepository { path: root.path().to_path_buf() });
    }
    gix::open(root.path()).map_err(|e| RepositoryError::Git(e.to_string()))
}

/// What would change if [`save_repository`] ran right now — the "N changes
/// since last repository save" the UI shows (§37) — without writing
/// anything.
pub fn status(root: &KovanRoot) -> Result<SaveSummary, RepositoryError> {
    let repo = open(root)?;
    let (_tree_id, blobs) = build_tree(&repo, root)?;
    diff_against_head(&repo, &blobs)
}

/// §37's "Save Repository": build a tree from the current (non-excluded)
/// worktree and commit it with a deterministic summary message, no AI.
/// Returns `Ok(None)` — a no-op — when there is nothing to commit.
pub fn save_repository(root: &KovanRoot) -> Result<Option<SaveSummary>, RepositoryError> {
    let mut repo = open(root)?;
    let (tree_id, blobs) = build_tree(&repo, root)?;
    let summary = diff_against_head(&repo, &blobs)?;
    if summary.is_empty() {
        return Ok(None);
    }

    // A deterministic technical identity, not ambient `user.name`/
    // `user.email` config this environment may not have set — §37
    // requires the summary to be deterministic; the author identity
    // should not depend on what happens to be configured on the machine
    // running it.
    let mut snapshot = repo.config_snapshot_mut();
    snapshot
        .append_config(["user.name=Kovan", "user.email=kovan@localhost"], gix::config::Source::Api)
        .map_err(|e| RepositoryError::Git(e.to_string()))?;
    snapshot.commit().map_err(|e| RepositoryError::Git(e.to_string()))?;

    let parents: Vec<gix::ObjectId> = match repo.head_id() {
        Ok(id) => vec![id.detach()],
        Err(_) => Vec::new(),
    };
    repo.commit("HEAD", summary.to_commit_message(), tree_id, parents)
        .map_err(|e| RepositoryError::Git(e.to_string()))?;

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

    #[test]
    fn first_save_repository_commits_the_skeleton() {
        let (_dir, root) = make_root();
        let summary = save_repository(&root).unwrap().expect("a fresh library has files to commit");
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

        EntityConfig::paper(CiteKey::parse("wang2018multiphysics").unwrap(), Access::Open)
            .with_topics(["htgrs"])
            .save_paper(&root.paper_dir("wang2018multiphysics"))
            .unwrap();

        let summary = save_repository(&root).unwrap().unwrap();
        assert!(summary.added.iter().any(|p| p.contains("wang2018multiphysics")));
    }

    #[test]
    fn restricted_pdfs_are_never_part_of_the_saved_tree() {
        let (_dir, root) = make_root();
        std::fs::create_dir_all(root.restricted_sources_dir()).unwrap();
        std::fs::write(root.restricted_sources_dir().join("secret.pdf"), b"proprietary bytes").unwrap();

        let summary = save_repository(&root).unwrap().unwrap();
        assert!(!summary.added.iter().any(|p| p.contains("secret.pdf")), "{:?}", summary.added);

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
}
