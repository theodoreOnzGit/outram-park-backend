//! Which workspace crates a change actually affects — the CI selector.
//!
//! # Why this exists
//!
//! `cargo quick-test` over this workspace takes **over 150 minutes** on a CI
//! runner (measured 2026-09-24 on `test / ubuntu-latest`, which had not
//! finished when the next push cancelled it). That is the *fast* tier, with
//! the `long-tests` feature already off, so the cost is not the tests — it is
//! compiling 43 crates in release mode, which the root `CLAUDE.md` mandates
//! and which is not negotiable.
//!
//! Selecting fewer *tests* therefore saves almost nothing. Selecting fewer
//! *crates* is the lever, and this computes which ones a diff can reach.
//!
//! # The correctness requirement, and why the naive version is wrong
//!
//! "Changed paths map to crates, test those crates" is **wrong** and fails
//! silently. Editing `njoy-outram-park-fork` cannot break only itself:
//! `outram-mc-libs` depends on it, and everything depending on *that* is
//! reachable too. A selector that tests only the edited crate reports green
//! while a dependent is broken — worse than no selector, because it carries
//! the authority of a passing run.
//!
//! So this walks the **reverse-dependency closure**: the changed crates, plus
//! every workspace member that can reach one of them through a dependency
//! edge, transitively.
//!
//! **`dev-dependencies` count as edges.** If `B` dev-depends on `A`, then
//! `B`'s *tests* use `A`, and a change to `A` can break them. Omitting dev
//! edges would skip exactly the thing being selected for.
//!
//! # Failing safe
//!
//! A path that is not inside `crates/<name>/` cannot be attributed to a crate,
//! so anything outside them selects the **whole workspace**. That covers the
//! root `Cargo.toml` (where every version lives — see the dependency policy),
//! `Cargo.lock`, the toolchain file, CI workflows, and `scripts/`. It also
//! covers anything new and unrecognised, which is the point: an unknown path
//! is a reason to run everything, never a reason to run nothing.
//!
//! # What this deliberately does not do
//!
//! It does not read `[features]`. A feature entry like
//! `njoy-outram-park-fork/net-fetch` names a crate that is already an edge
//! through the dependency table, so parsing features adds no edge the
//! dependency tables do not already carry — and a feature *union* across a
//! selected subset is not the same as the workspace-wide union, which is a
//! separate hazard noted in the CI workflow rather than papered over here.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

/// How to print the selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// `-p a -p b` — paste straight onto a `cargo` invocation.
    CargoArgs,
    /// One crate name per line.
    List,
}

/// Why a selection could not be computed.
#[derive(Debug)]
pub enum AffectedError {
    Git(String),
    Manifest { path: PathBuf, message: String },
    NoWorkspace(PathBuf),
}

impl std::fmt::Display for AffectedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Git(m) => write!(f, "git: {m}"),
            Self::Manifest { path, message } => write!(f, "{}: {message}", path.display()),
            Self::NoWorkspace(p) => {
                write!(f, "{}: no [workspace] members", p.display())
            }
        }
    }
}

impl std::error::Error for AffectedError {}

/// The workspace's member crate names, read from the root manifest.
///
/// Members are listed by **path**; the crate name is taken as the last path
/// segment. That holds throughout this workspace (`crates/<name>` is always
/// the package `<name>`), and a mismatch would show up immediately as a
/// selected crate `cargo` does not recognise rather than as a silent miss.
fn members(root: &Path) -> Result<Vec<String>, AffectedError> {
    let manifest = root.join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest).map_err(|e| AffectedError::Manifest {
        path: manifest.clone(),
        message: e.to_string(),
    })?;
    let doc: toml::Value = text.parse().map_err(|e: toml::de::Error| {
        AffectedError::Manifest {
            path: manifest.clone(),
            message: e.to_string(),
        }
    })?;
    let list = doc
        .get("workspace")
        .and_then(|w| w.get("members"))
        .and_then(|m| m.as_array())
        .ok_or_else(|| AffectedError::NoWorkspace(manifest.clone()))?;
    Ok(list
        .iter()
        .filter_map(|v| v.as_str())
        .filter_map(|p| p.rsplit('/').next())
        .map(str::to_string)
        .collect())
}

/// `dependency -> the members that depend on it`, over workspace members only.
///
/// Every dependency table is read — normal, dev and build — for the reason in
/// the module docs: a dev edge is how a change reaches another crate's tests.
fn reverse_edges(
    root: &Path,
    members: &[String],
) -> Result<BTreeMap<String, BTreeSet<String>>, AffectedError> {
    let member_set: BTreeSet<&str> = members.iter().map(String::as_str).collect();
    let mut rev: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for name in members {
        let manifest = root.join("crates").join(name).join("Cargo.toml");
        let Ok(text) = std::fs::read_to_string(&manifest) else {
            continue; // a member path that is not under crates/ — nothing to read
        };
        let doc: toml::Value = text.parse().map_err(|e: toml::de::Error| {
            AffectedError::Manifest {
                path: manifest.clone(),
                message: e.to_string(),
            }
        })?;
        for table in ["dependencies", "dev-dependencies", "build-dependencies"] {
            let Some(t) = doc.get(table).and_then(|d| d.as_table()) else {
                continue;
            };
            for dep in t.keys() {
                if member_set.contains(dep.as_str()) && dep != name {
                    rev.entry(dep.clone()).or_default().insert(name.clone());
                }
            }
        }
    }
    Ok(rev)
}

/// The files `base...HEAD` changed, as repo-relative paths.
///
/// Three dots, not two: this is the diff against the **merge base**, which is
/// what "what did this branch change" means. Two dots would also report every
/// change that landed on the base since the branch left it, selecting crates
/// the branch never touched.
fn changed_files(root: &Path, base: &str) -> Result<Vec<String>, AffectedError> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["diff", "--name-only", &format!("{base}...HEAD")])
        .output()
        .map_err(|e| AffectedError::Git(e.to_string()))?;
    if !out.status.success() {
        return Err(AffectedError::Git(
            String::from_utf8_lossy(&out.stderr).trim().to_string(),
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .filter(|l| !l.is_empty())
        .collect())
}

/// What a diff selects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    /// Something outside any crate changed — run everything.
    Everything(String),
    /// These crates, already closed over reverse dependencies.
    Crates(BTreeSet<String>),
}

/// Map changed files to a [`Selection`].
///
/// Pure, so the closure logic is testable without a git repository or a
/// filesystem walk.
pub fn select(
    changed: &[String],
    members: &[String],
    rev: &BTreeMap<String, BTreeSet<String>>,
) -> Selection {
    let member_set: BTreeSet<&str> = members.iter().map(String::as_str).collect();
    let mut seeds: BTreeSet<String> = BTreeSet::new();

    for file in changed {
        let Some(rest) = file.strip_prefix("crates/") else {
            // Outside every crate: fail safe, not fail quiet.
            return Selection::Everything(format!("{file} is outside crates/"));
        };
        let Some(name) = rest.split('/').next() else {
            return Selection::Everything(format!("{file} names no crate"));
        };
        if !member_set.contains(name) {
            return Selection::Everything(format!("{file} is not a workspace member"));
        }
        seeds.insert(name.to_string());
    }

    if seeds.is_empty() {
        return Selection::Crates(BTreeSet::new());
    }

    // Reverse-dependency closure: breadth-first over `rev`.
    let mut out = seeds.clone();
    let mut queue: Vec<String> = seeds.into_iter().collect();
    while let Some(c) = queue.pop() {
        if let Some(dependents) = rev.get(&c) {
            for d in dependents {
                if out.insert(d.clone()) {
                    queue.push(d.clone());
                }
            }
        }
    }
    Selection::Crates(out)
}

/// Compute and print the selection for `base`.
pub fn run(root: &Path, base: &str, format: Format) -> Result<(), AffectedError> {
    let members = members(root)?;
    let rev = reverse_edges(root, &members)?;
    let changed = changed_files(root, base)?;

    match select(&changed, &members, &rev) {
        Selection::Everything(why) => {
            eprintln!("selecting the whole workspace: {why}");
            match format {
                // No `-p` at all *is* the whole workspace, so a caller can
                // splice the output in unconditionally.
                Format::CargoArgs => println!(),
                Format::List => {
                    for m in members {
                        println!("{m}");
                    }
                }
            }
        }
        Selection::Crates(set) => {
            eprintln!("{} crate(s) affected by {} file(s)", set.len(), changed.len());
            match format {
                Format::CargoArgs => {
                    let args: Vec<String> = set.iter().map(|c| format!("-p {c}")).collect();
                    println!("{}", args.join(" "));
                }
                Format::List => {
                    for c in &set {
                        println!("{c}");
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Vec<String>, BTreeMap<String, BTreeSet<String>>) {
        let members: Vec<String> = ["njoy", "mc", "twin", "kovan"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        // mc -> njoy, twin -> mc, kovan stands alone.
        let mut rev: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        rev.entry("njoy".into()).or_default().insert("mc".into());
        rev.entry("mc".into()).or_default().insert("twin".into());
        (members, rev)
    }

    /// A change to a dependency selects everything that can reach it,
    /// transitively — the property the naive "just the edited crate" version
    /// gets wrong, and gets wrong *silently*.
    #[test]
    fn the_closure_reaches_transitive_dependents() {
        let (members, rev) = fixture();
        let changed = vec!["crates/njoy/src/acer/mod.rs".to_string()];
        let Selection::Crates(sel) = select(&changed, &members, &rev) else {
            panic!("expected a crate selection");
        };
        assert!(sel.contains("njoy"), "the edited crate");
        assert!(sel.contains("mc"), "its direct dependent");
        assert!(sel.contains("twin"), "and the dependent's dependent");
        assert!(!sel.contains("kovan"), "but nothing unreachable");
    }

    /// A leaf change selects only the leaf.
    #[test]
    fn a_leaf_change_selects_only_the_leaf() {
        let (members, rev) = fixture();
        let changed = vec!["crates/kovan/src/lib.rs".to_string()];
        let Selection::Crates(sel) = select(&changed, &members, &rev) else {
            panic!("expected a crate selection");
        };
        assert_eq!(sel.len(), 1);
        assert!(sel.contains("kovan"));
    }

    /// Anything outside `crates/` runs the whole workspace. The root manifest
    /// carries every version in this workspace, so a change there can reach
    /// any crate; an unrecognised path is treated the same way on purpose.
    #[test]
    fn anything_outside_a_crate_selects_everything() {
        let (members, rev) = fixture();
        for file in [
            "Cargo.toml",
            "Cargo.lock",
            ".github/workflows/fast-tests.yml",
            "scripts/check-wasm.sh",
            "some-new-top-level-thing",
        ] {
            assert!(
                matches!(
                    select(&[file.to_string()], &members, &rev),
                    Selection::Everything(_)
                ),
                "{file} should select the whole workspace"
            );
        }
    }

    /// A path under `crates/` naming something that is not a member is also
    /// a whole-workspace trigger: it means the member list and the tree
    /// disagree, which is not a case to guess at.
    #[test]
    fn an_unknown_crate_directory_selects_everything() {
        let (members, rev) = fixture();
        let changed = vec!["crates/not-a-member/src/lib.rs".to_string()];
        assert!(matches!(
            select(&changed, &members, &rev),
            Selection::Everything(_)
        ));
    }

    /// An empty diff selects nothing, rather than everything. A push with no
    /// file changes has nothing to test.
    #[test]
    fn an_empty_diff_selects_nothing() {
        let (members, rev) = fixture();
        let Selection::Crates(sel) = select(&[], &members, &rev) else {
            panic!("expected a crate selection");
        };
        assert!(sel.is_empty());
    }
}
