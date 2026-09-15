//! Finding the OUTRAM PARK workspace, so `kovan` can be run from anywhere.
//!
//! # Why discovery rather than a `--root .` default
//!
//! A bare `--root .` default is only correct when the command happens to be run
//! from the workspace root. Run it one directory deeper — or from the home
//! directory, which is where a shell usually starts — and it fails with "no
//! `crates/`", which reads as a broken tool rather than a wrong working
//! directory.
//!
//! # The order, and why it is this order
//!
//! 1. **An explicit `--root`.** If the caller named a path, that is the answer,
//!    and a wrong one is an error rather than something quietly overridden by a
//!    search. Predictability beats helpfulness here.
//! 2. **The current directory, walking up.** Being *inside* the workspace is the
//!    strongest possible signal about which workspace is meant, and walking up
//!    means it works from any subdirectory.
//! 3. **The home directory**, then **`Documents/`** and `Documents/research/`,
//!    which is where this workspace actually lives on the maintainer's machines.
//!
//! If none matches, the error names every path tried and tells the caller to
//! pass `--root`. A discovery that fails silently, or picks something plausible
//! but wrong, would be worse than not discovering at all — this command
//! *writes* into the tree it finds.
//!
//! # What counts as the workspace
//!
//! A directory holding both `crates/` and a `Cargo.toml` declaring
//! `[workspace]`. Checking for the marker rather than the *name* means a clone
//! under any directory name is found, and an unrelated directory that happens to
//! be called `outram-park-backend` is not mistaken for one.

use std::io;
use std::path::{Path, PathBuf};

/// Directory name the workspace is conventionally checked out as.
const CONVENTIONAL_NAME: &str = "outram-park-backend";

/// Home-relative directories searched, in order, after the current directory.
const HOME_SUBDIRS: &[&str] = &["", "Documents", "Documents/research"];

/// Does `path` look like a Cargo workspace root with a `crates/` directory?
///
/// Both markers are required. `crates/` alone would match a random directory;
/// `[workspace]` alone would match any workspace, including one this command has
/// no business writing into.
pub fn is_workspace_root(path: &Path) -> bool {
    if !path.join("crates").is_dir() {
        return false;
    }
    let Ok(manifest) = std::fs::read_to_string(path.join("Cargo.toml")) else {
        return false;
    };
    manifest.lines().any(|line| line.trim() == "[workspace]")
}

/// The current directory or the nearest ancestor that is a workspace root.
fn from_current_directory() -> Option<PathBuf> {
    let mut current = std::env::current_dir().ok()?;
    loop {
        if is_workspace_root(&current) {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Every candidate path the search considers, in order, for reporting.
fn home_candidates() -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };
    HOME_SUBDIRS
        .iter()
        .map(|sub| {
            if sub.is_empty() {
                home.join(CONVENTIONAL_NAME)
            } else {
                home.join(sub).join(CONVENTIONAL_NAME)
            }
        })
        .collect()
}

/// Resolve the workspace root.
///
/// `explicit` is the caller's `--root`, which wins outright when given. See the
/// module docs for the search order used otherwise.
///
/// Returns the root and a short phrase describing how it was found, so the
/// command can say which tree it is about to write into — discovery that does
/// not announce its result is discovery you cannot trust.
pub fn resolve(explicit: Option<&Path>) -> io::Result<(PathBuf, String)> {
    if let Some(root) = explicit {
        if !is_workspace_root(root) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "{} is not an OUTRAM PARK workspace root \
                     (needs a crates/ directory and a Cargo.toml declaring [workspace])",
                    root.display()
                ),
            ));
        }
        return Ok((root.to_path_buf(), "given by --root".to_string()));
    }

    if let Some(root) = from_current_directory() {
        return Ok((root, "found from the current directory".to_string()));
    }

    let candidates = home_candidates();
    for candidate in &candidates {
        if is_workspace_root(candidate) {
            return Ok((
                candidate.clone(),
                "found under your home directory".to_string(),
            ));
        }
    }

    let mut tried = vec!["the current directory and its parents".to_string()];
    tried.extend(candidates.iter().map(|p| p.display().to_string()));
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!(
            "could not find the OUTRAM PARK workspace. Tried:\n  {}\nPass --root <path> to name it.",
            tried.join("\n  ")
        ),
    ))
}

/// Choose where a generated directory such as `agent-docs/` should live.
///
/// # The order
///
/// 1. **An explicit `--out`.** The caller's path wins outright.
/// 2. **Inside the workspace**, if one can be found — `<workspace>/agent-docs`.
///    This is the normal case and keeps the bundle beside the code it describes,
///    where the repository's `.gitignore` already covers it.
/// 3. **`$HOME/<name>`**, then **`$HOME/Documents/<name>`**, for running the
///    command with no workspace to hand.
///
/// Returns the directory and a phrase describing the choice, so the command can
/// say where it wrote — a generator that silently picks a location is one whose
/// output you then have to hunt for.
///
/// # A caution worth stating
///
/// Outside the workspace there is no `.gitignore` protecting the result. The
/// bundle is several megabytes of generated copies; if it is placed inside some
/// *other* repository, that repository will see it as new files to commit.
pub fn output_dir(explicit: Option<&Path>, name: &str) -> io::Result<(PathBuf, String)> {
    if let Some(out) = explicit {
        return Ok((out.to_path_buf(), "given by --out".to_string()));
    }

    if let Some(root) = from_current_directory() {
        return Ok((root.join(name), "in the workspace".to_string()));
    }
    for candidate in home_candidates() {
        if is_workspace_root(&candidate) {
            return Ok((candidate.join(name), "in the workspace".to_string()));
        }
    }

    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no workspace found and no HOME set -- pass --out <path>",
        ));
    };
    // No workspace: fall back to the home directory, then Documents. Both are
    // *created* rather than required to exist, since the point is to have
    // somewhere to write.
    let documents = home.join("Documents");
    if documents.is_dir() {
        return Ok((
            documents.join(name),
            "under ~/Documents (no workspace found)".to_string(),
        ));
    }
    Ok((
        home.join(name),
        "under your home directory (no workspace found)".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_workspace(root: &Path) {
        fs::create_dir_all(root.join("crates")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\n",
        )
        .unwrap();
    }

    #[test]
    fn both_markers_are_required() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        assert!(
            !is_workspace_root(root),
            "empty directory is not a workspace"
        );

        fs::create_dir_all(root.join("crates")).unwrap();
        assert!(
            !is_workspace_root(root),
            "a crates/ directory alone would match almost anything"
        );

        fs::write(root.join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
        assert!(
            !is_workspace_root(root),
            "a plain package is not a workspace root"
        );

        fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
        assert!(is_workspace_root(root));
    }

    /// A directory merely NAMED `outram-park-backend` must not be mistaken for
    /// the workspace -- this command writes into whatever it finds.
    #[test]
    fn the_name_alone_does_not_make_a_workspace() {
        let tmp = tempfile::tempdir().unwrap();
        let impostor = tmp.path().join("outram-park-backend");
        fs::create_dir_all(&impostor).unwrap();
        assert!(!is_workspace_root(&impostor));
    }

    #[test]
    fn an_explicit_root_wins_and_a_wrong_one_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("ws");
        make_workspace(&root);

        let (found, how) = resolve(Some(&root)).unwrap();
        assert_eq!(found, root);
        assert!(how.contains("--root"));

        let error = resolve(Some(tmp.path())).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(
            error
                .to_string()
                .contains("not an OUTRAM PARK workspace root"),
            "a wrong --root must be an error, never silently replaced by a search"
        );
    }

    /// The failure must name what it tried, or "not found" is unactionable.
    #[test]
    fn a_failed_search_names_every_path_it_tried() {
        // Point HOME somewhere empty so the search cannot succeed, and run from
        // a directory with no workspace above it.
        let tmp = tempfile::tempdir().unwrap();
        let empty_home = tmp.path().join("home");
        fs::create_dir_all(&empty_home).unwrap();

        let previous_home = std::env::var_os("HOME");
        let previous_dir = std::env::current_dir().ok();
        // SAFETY-adjacent: this test mutates process-global state, so it must
        // restore both before returning.
        unsafe { std::env::set_var("HOME", &empty_home) };
        std::env::set_current_dir(&empty_home).unwrap();

        let error = resolve(None).unwrap_err();

        if let Some(home) = previous_home {
            unsafe { std::env::set_var("HOME", home) };
        }
        if let Some(dir) = previous_dir {
            let _ = std::env::set_current_dir(dir);
        }

        let text = error.to_string();
        assert!(text.contains("could not find the OUTRAM PARK workspace"));
        assert!(text.contains("the current directory and its parents"));
        assert!(text.contains("Documents"));
        assert!(text.contains("--root"));
    }

    #[test]
    fn an_explicit_out_directory_wins() {
        let tmp = tempfile::tempdir().unwrap();
        let chosen = tmp.path().join("somewhere");
        let (dir, how) = output_dir(Some(&chosen), "agent-docs").unwrap();
        assert_eq!(dir, chosen);
        assert!(how.contains("--out"));
    }

    /// Preference 1: inside the workspace, beside the code it describes, where
    /// the repository's .gitignore already covers it.
    #[test]
    fn the_workspace_is_preferred_when_one_can_be_found() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("ws");
        make_workspace(&root);

        let previous_dir = std::env::current_dir().ok();
        std::env::set_current_dir(&root).unwrap();
        let resolved = output_dir(None, "agent-docs");
        if let Some(dir) = previous_dir {
            let _ = std::env::set_current_dir(dir);
        }

        let (dir, how) = resolved.unwrap();
        assert_eq!(dir.file_name().unwrap(), "agent-docs");
        assert!(dir.starts_with(&root), "got {}", dir.display());
        assert!(how.contains("workspace"));
    }

    #[test]
    fn the_home_search_order_is_home_then_documents_then_research() {
        assert_eq!(HOME_SUBDIRS, &["", "Documents", "Documents/research"]);
    }
}
