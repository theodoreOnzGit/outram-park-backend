//! End-to-end check of `kovan-cli index --fresh` (GitHub #780), the
//! headless path of the desktop app's "Index fresh" button (both call
//! `kovan::index_fresh::run_fresh`), on throwaway repositories in a temp
//! directory.
//!
//! **Methodology.**
//!
//! 1. **Single crate** (`[package]`, no `[workspace]`), a git repository
//!    with no commit yet: `--fresh` writes `kovan_root.toml` (founder
//!    UNSET, the rust-analyzer version recorded, no reviewers), `src/` and
//!    `tests/` `kovan.toml`, the crate's `kovan_links.json` at the root,
//!    and a `review.md` skeleton in each of the two folders. Nothing is
//!    committed (`git log` still has no commit).
//! 2. **Rerun**: nothing changes, byte for byte (the root is kept, the
//!    skeletons kept, the cache unchanged).
//! 3. **Corrupt root**, after a commit of the good one: without
//!    `--corrupt-root` the run is refused and the corrupt file is left
//!    exactly as it was; `--corrupt-root restore` keeps it as
//!    `kovan_root.toml.corrupt-<date>` and restores the committed bytes;
//!    `--corrupt-root fresh` does the same with a fresh root.
//! 4. **Workspace** (`[workspace]` with one member under `crates/`, no
//!    `crates/` convention required beyond that): `--fresh` indexes the
//!    member, the link file in the member's folder.
//!
//! **Gate.** Needs `rust-analyzer`, `git` and `cargo`; without
//! rust-analyzer the test prints why and stops.
//!
//! **Result (2026-10-07, rust-analyzer 1.98.0):** passes.

use std::path::Path;
use std::process::Command;

fn write(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

fn read(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).unwrap_or_default()
}

fn git(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.org")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.org")
        .output()
        .unwrap()
}

fn fresh(dir: &Path, extra: &[&str]) -> std::process::Output {
    let mut args = vec!["index", "--fresh", "--workspace", dir.to_str().unwrap()];
    args.extend_from_slice(extra);
    Command::new(env!("CARGO_BIN_EXE_kovan-cli"))
        .args(&args)
        .output()
        .unwrap()
}

const LIB: &str = "pub mod geom;\n\n/// Area of a square.\npub fn square(s: f64) -> f64 {\n    geom::rect(s, s)\n}\n";
const GEOM: &str = "pub fn rect(w: f64, h: f64) -> f64 {\n    w * h\n}\n";
const IT: &str = "#[test]\nfn it_square() {\n    assert_eq!(tiny::square(2.0), 4.0);\n}\n";
const FILES: [&str; 6] = [
    "kovan_root.toml",
    "kovan_links.json",
    "src/kovan.toml",
    "tests/kovan.toml",
    "src/review.md",
    "tests/review.md",
];

#[test]
fn index_fresh_on_a_single_crate_and_a_workspace() {
    if which::which("rust-analyzer").is_err() {
        eprintln!("rust-analyzer not installed: index --fresh not exercised");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let c = tmp.path().join("tiny");
    write(
        &c,
        "Cargo.toml",
        "[package]\nname = \"tiny\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    write(&c, "src/lib.rs", LIB);
    write(&c, "src/geom.rs", GEOM);
    write(&c, "tests/it.rs", IT);
    write(&c, ".gitignore", "/target\n");
    assert!(git(&c, &["init", "-q"]).status.success());

    // 1. Fresh.
    let out = fresh(&c, &[]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    for f in FILES {
        assert!(c.join(f).is_file(), "{f} missing");
    }
    let root = read(&c, "kovan_root.toml");
    assert!(
        root.contains("founder: UNSET") || root.contains("founder = "),
        "{root}"
    );
    let rr = kovan_common::review::root::ReviewRoot::parse(&root).unwrap();
    assert!(rr.reviewers.is_empty());
    assert!(rr.code_review.unwrap().rust_analyzer.is_some());
    let doc = kovan_common::review::review_md::parse_review_md(&read(&c, "src/review.md"));
    assert!(doc.unreadable.is_empty() && doc.entries.len() == 1);
    assert!(read(&c, "src/kovan.toml").contains("name = \"square\""));
    assert!(
        !git(&c, &["rev-parse", "--verify", "HEAD"]).status.success(),
        "nothing committed"
    );

    // 2. Rerun: byte-identical.
    let before: Vec<String> = FILES.iter().map(|f| read(&c, f)).collect();
    let out = fresh(&c, &[]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let after: Vec<String> = FILES.iter().map(|f| read(&c, f)).collect();
    assert_eq!(before, after);

    // 3. Corrupt root.
    assert!(git(&c, &["add", "-A"]).status.success());
    assert!(git(&c, &["commit", "-qm", "index"]).status.success());
    let good = read(&c, "kovan_root.toml");
    write(&c, "kovan_root.toml", "schema_version = [\n");
    let out = fresh(&c, &[]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("corrupt"));
    assert_eq!(
        read(&c, "kovan_root.toml"),
        "schema_version = [\n",
        "refused: untouched"
    );
    let out = fresh(&c, &["--corrupt-root", "restore"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(read(&c, "kovan_root.toml"), good);
    let kept: Vec<String> = std::fs::read_dir(&c)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with("kovan_root.toml.corrupt-"))
        .collect();
    assert_eq!(kept.len(), 1);
    assert_eq!(read(&c, &kept[0]), "schema_version = [\n");
    write(&c, "kovan_root.toml", "[[reviewer]]\nid = \"x\"\n");
    let out = fresh(
        &c,
        &["--corrupt-root", "fresh", "--founder", "github:someone"],
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let root = read(&c, "kovan_root.toml");
    assert!(root.contains("founder = \"github:someone\""), "{root}");

    // 4. Workspace.
    let w = tmp.path().join("ws");
    write(
        &w,
        "Cargo.toml",
        "[workspace]\nmembers = [\"crates/tiny\"]\nresolver = \"2\"\n",
    );
    for (rel, text) in [
        (
            "Cargo.toml",
            "[package]\nname = \"tiny\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        ),
        ("src/lib.rs", LIB),
        ("src/geom.rs", GEOM),
    ] {
        write(&w, &format!("crates/tiny/{rel}"), text);
    }
    let out = fresh(&w, &[]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("Cargo workspace"));
    for f in [
        "kovan_root.toml",
        "crates/tiny/kovan_links.json",
        "crates/tiny/src/kovan.toml",
        "crates/tiny/src/review.md",
    ] {
        assert!(w.join(f).is_file(), "{f} missing");
    }
}
