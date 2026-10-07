//! End-to-end check of `kovan-cli index` (GitHub #767) on a throwaway
//! one-crate workspace in a temp directory, committed to a fresh git
//! repository.
//!
//! **Methodology.**
//!
//! 1. **No rust-analyzer** (`PATH` holding only `git` and `cargo`):
//!    `kovan-cli index` fails with the typed "rust-analyzer is not
//!    installed" error and writes nothing.
//! 2. **Full run** (`rust-analyzer scip` once): one `kovan.toml` per folder
//!    with `.rs` files (`src/`, `tests/`), `kovan_links.json` at the crate
//!    root that parses and answers go-to-definition for a call in `lib.rs`.
//! 3. **Deterministic**: a second run from the same index writes nothing and
//!    `--check` passes.
//! 4. **Self-healing**: a hand-edited `src/kovan.toml`, a conflict-marked
//!    `tests/kovan.toml` and an orphan written for a folder with no code are
//!    caught by `--check` (fails, writes nothing), then regenerated or
//!    removed by a run, back to the bytes of step 2; a fixture copy naming
//!    another folder is left alone. `review.md` is never written.
//! 5. **Refresh without rust-analyzer**: `twice` edited; `index --refresh`
//!    under the stripped `PATH` marks it `index_out_of_date` and keeps
//!    `leaf` as it was, and the plain `index` still refuses, leaving the
//!    refreshed files as they are.
//!
//! **Gate.** Steps 2-5 need `rust-analyzer` and `git`; without them the test
//! prints why and checks step 1 only.
//!
//! **Result (2026-10-07, rust-analyzer 1.98.0):** passes.

use std::path::{Path, PathBuf};
use std::process::Command;

use kovan_common::code_index::links::LinkIndex;
use kovan_common::review::index::FolderIndex;

const LIB: &str = "pub mod util;

/// Doubles.
pub fn leaf(x: f64) -> f64 {
    x * 2.0
}

pub fn twice(x: f64) -> f64 {
    leaf(x) + util::helper(x)
}

#[cfg(test)]
mod tests {
    #[test]
    fn t_twice() {
        assert!(super::twice(1.0) > 0.0);
    }
}
";

const UTIL: &str = "pub fn helper(x: f64) -> f64 {
    x + 1.0
}
";

const IT: &str = "#[test]
fn it_leaf() {
    assert_eq!(kern::leaf(1.0), 2.0);
}
";

fn write(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

fn read(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).unwrap_or_default()
}

/// A directory holding only `git` and `cargo`: a PATH without
/// rust-analyzer.
fn stripped_path(dir: &Path) -> PathBuf {
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    for tool in ["git", "cargo"] {
        if let Ok(p) = which::which(tool) {
            #[cfg(unix)]
            std::os::unix::fs::symlink(p, bin.join(tool)).unwrap();
        }
    }
    bin
}

fn kovan(args: &[&str], path: Option<&Path>) -> std::process::Output {
    let mut c = Command::new(env!("CARGO_BIN_EXE_kovan-cli"));
    c.args(args);
    if let Some(p) = path {
        c.env("PATH", p);
    }
    c.output().expect("spawn kovan-cli")
}

fn git(root: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.org")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.org")
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {args:?}");
}

fn function<'a>(fi: &'a FolderIndex, file: &str, name: &str) -> &'a kovan_common::review::index::FunctionIndex {
    fi.modules[file].functions.iter().find(|f| f.name == name).unwrap()
}

#[test]
fn index_builds_heals_and_refreshes_without_rust_analyzer() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("ws");
    write(&root, "Cargo.toml", "[workspace]\nmembers = [\"crates/kern\"]\nresolver = \"2\"\n");
    write(
        &root,
        "crates/kern/Cargo.toml",
        "[package]\nname = \"kern\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    write(&root, "crates/kern/src/lib.rs", LIB);
    write(&root, "crates/kern/src/util.rs", UTIL);
    write(&root, "crates/kern/tests/it.rs", IT);
    write(&root, ".gitignore", "target/\n");
    let root_s = root.to_str().unwrap().to_string();
    let bare = stripped_path(tmp.path());

    // 1. No rust-analyzer: typed error, nothing written.
    let out = kovan(&["index", "--workspace", &root_s], Some(&bare));
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("rust-analyzer is not installed"), "{err}");
    assert!(err.contains("--refresh"), "{err}");
    assert!(!root.join("crates/kern/src/kovan.toml").exists());

    if which::which("rust-analyzer").is_err() || which::which("git").is_err() {
        eprintln!("SKIPPED steps 2-5: rust-analyzer or git is not on PATH");
        return;
    }
    git(&root, &["init", "-q"]);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "fixture"]);

    // 2. Full run.
    let out = kovan(&["index", "--workspace", &root_s], None);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let src = FolderIndex::parse(&read(&root, "crates/kern/src/kovan.toml")).unwrap();
    let tests = FolderIndex::parse(&read(&root, "crates/kern/tests/kovan.toml")).unwrap();
    assert!(src.crate_root && src.commit.is_none());
    assert_eq!(src.modules["util.rs"].path, "crate::util");
    assert_eq!(tests.modules["it.rs"].path, "test:it");
    let leaf = function(&src, "lib.rs", "leaf");
    let twice = function(&src, "lib.rs", "twice");
    let helper = function(&src, "util.rs", "helper");
    assert!(twice.callees.contains(&leaf.id) && twice.callees.contains(&helper.id));
    assert!(leaf.reached_by.contains(&"crates/kern/tests/it.rs::it_leaf".to_string()));
    let links = LinkIndex::parse(&read(&root, "crates/kern/kovan_links.json")).unwrap();
    // `leaf(x)` in `twice`, line 9 (0-based 8), column 4.
    let def = links.definition_at("src/lib.rs", 8, 5).expect("a link at leaf(x)");
    assert_eq!((def.path.as_str(), def.line), ("crates/kern/src/lib.rs", 3));
    assert!(links.is_current("src/lib.rs", LIB));
    let first: Vec<String> = ["crates/kern/src/kovan.toml", "crates/kern/tests/kovan.toml", "crates/kern/kovan_links.json"]
        .iter()
        .map(|p| read(&root, p))
        .collect();
    let scip = root.join("target/kovan-scip/index.scip");
    let scip_s = scip.to_str().unwrap().to_string();

    // 3. Deterministic.
    let out = kovan(&["index", "--workspace", &root_s, "--scip", &scip_s, "--check"], None);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    // 4. Self-healing.
    write(&root, "crates/kern/src/kovan.toml", &format!("{}\n# hand note\n", first[0]));
    write(&root, "crates/kern/tests/kovan.toml", &format!("<<<<<<< HEAD\n{}=======\n>>>>>>> x\n", first[1]));
    let orphan = "schema_version = 1\nkind = \"code_folder\"\ncrate = \"kern\"\ndir = \"crates/kern/src/gone\"\n";
    write(&root, "crates/kern/src/gone/kovan.toml", orphan);
    let fixture = orphan.replace("crates/kern/src/gone", "crates/elsewhere/src");
    write(&root, "crates/kern/fixtures/kovan.toml", &fixture);
    let review = "# a note\n";
    write(&root, "crates/kern/src/review.md", review);
    let out = kovan(&["index", "--workspace", &root_s, "--scip", &scip_s, "--check"], None);
    assert!(!out.status.success());
    assert!(root.join("crates/kern/src/gone/kovan.toml").exists());
    let out = kovan(&["index", "--workspace", &root_s, "--scip", &scip_s], None);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(read(&root, "crates/kern/src/kovan.toml"), first[0]);
    assert_eq!(read(&root, "crates/kern/tests/kovan.toml"), first[1]);
    assert!(!root.join("crates/kern/src/gone/kovan.toml").exists());
    assert_eq!(read(&root, "crates/kern/fixtures/kovan.toml"), fixture);
    assert_eq!(read(&root, "crates/kern/src/review.md"), review);

    // 5. Refresh without rust-analyzer.
    write(&root, "crates/kern/src/lib.rs", &LIB.replace("leaf(x) + util", "leaf(x) - util"));
    let out = kovan(&["index", "--workspace", &root_s, "--refresh"], Some(&bare));
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let refreshed = FolderIndex::parse(&read(&root, "crates/kern/src/kovan.toml")).unwrap();
    let t2 = function(&refreshed, "lib.rs", "twice");
    assert!(t2.index_out_of_date && t2.id == twice.id && t2.callees == twice.callees);
    assert_ne!(t2.hash, twice.hash);
    assert_eq!(function(&refreshed, "lib.rs", "leaf"), leaf);
    let before = read(&root, "crates/kern/src/kovan.toml");
    let out = kovan(&["index", "--workspace", &root_s], Some(&bare));
    assert!(!out.status.success());
    assert_eq!(read(&root, "crates/kern/src/kovan.toml"), before);
}
