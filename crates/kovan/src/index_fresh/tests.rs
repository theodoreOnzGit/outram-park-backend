//! Tests of the fresh index's library halves (GitHub #780) on throwaway
//! fixture folders: a workspace, a single crate, a corrupt root, a git
//! history to restore from. No rust-analyzer is run here; the end-to-end
//! run with rust-analyzer is `tests/index_fresh_cli.rs`.

use std::path::Path;
use std::process::Command;

use kovan_common::review::review_md::{parse_review_md, Entry};
use kovan_common::review::root::{ReviewRoot, RustAnalyzerUsed};

use super::detect::{detect_project, DetectError, ProjectKind};
use super::plan::{cost_message, plan_fresh, run_fresh, FileAction, FreshChoices, FreshError};
use super::recent::RecentWorkspaces;
use super::root_file::*;
use super::skeleton::{create_missing_skeletons, missing_review_mds, review_skeleton};
use crate::commands::index_control::RunControl;

const V1_ROOT: &str =
    include_str!("../../../kovan-common/tests/fixtures/review/v1/kovan_root.toml");
const V2_ROOT: &str =
    include_str!("../../../kovan-common/tests/fixtures/review/v2/kovan_root.toml");

fn used(version: &str) -> RustAnalyzerUsed {
    RustAnalyzerUsed {
        version: version.into(),
        date: "2026-10-07".into(),
        commit: "none".into(),
    }
}

fn write(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

fn single_crate(root: &Path) {
    write(
        root,
        "Cargo.toml",
        "[package]\nname = \"solo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    write(
        root,
        "src/lib.rs",
        "pub mod a;\n/// Doubles.\npub fn f(x: u32) -> u32 {\n    a::g(x) * 2\n}\n",
    );
    write(
        root,
        "src/a.rs",
        "pub fn g(x: u32) -> u32 {\n    x + 1\n}\n",
    );
    write(root, "src/main.rs", "fn main() {}\n");
    write(
        root,
        "tests/it.rs",
        "#[test]\nfn it() {\n    assert_eq!(solo::f(1), 4);\n}\n",
    );
}

fn git(root: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.org")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.org")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
fn workspace_or_crate_comes_from_cargo_toml_alone() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    assert_eq!(
        detect_project(p),
        Err(DetectError::NoManifest(p.to_path_buf()))
    );
    write(p, "Cargo.toml", "[workspace]\nmembers = [\"a\"]\n");
    assert_eq!(detect_project(p), Ok(ProjectKind::Workspace));
    write(p, "Cargo.toml", "[package]\nname = \"x\"\n[workspace]\n");
    assert_eq!(
        detect_project(p),
        Ok(ProjectKind::Workspace),
        "a root-package workspace is a workspace"
    );
    write(p, "Cargo.toml", "[package]\nname = \"x\"\n");
    assert_eq!(detect_project(p), Ok(ProjectKind::SingleCrate));
    assert_eq!(ProjectKind::SingleCrate.label(), "single crate");
    write(p, "Cargo.toml", "[dependencies]\n");
    assert!(matches!(
        detect_project(p),
        Err(DetectError::NeitherWorkspaceNorPackage(_))
    ));
    write(p, "Cargo.toml", "[package\n");
    let e = detect_project(p).unwrap_err();
    assert!(matches!(e, DetectError::Unreadable { .. }) && !e.to_string().is_empty());
}

#[test]
fn a_fresh_root_parses_both_ways_and_leaves_an_unknown_founder_visibly_unset() {
    let text = fresh_root_text("solo", None, Some(&used("1.98.0")), "2026-10-07").unwrap();
    assert!(text.contains("founder: UNSET"), "{text}");
    let r = ReviewRoot::parse(&text).unwrap();
    assert_eq!(r.rust_analyzer_history(), &[used("1.98.0")]);
    let cr = r.code_review.unwrap();
    // The version used is recorded, never pinned (maintainer, 2026-10-07).
    assert_eq!((cr.founder, cr.rust_analyzer.as_deref()), (None, None));
    assert!(r.reviewers.is_empty() && r.deleted_crates.is_empty());
    // The literature side opens it too, so the folder is a kovan library.
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join(ROOT_FILE), &text).unwrap();
    let lib = crate::root::KovanRoot::open(d.path()).unwrap();
    assert_eq!(lib.config().library.id, "solo");

    let text = fresh_root_text("solo", Some("github:someone"), None, "2026-10-07").unwrap();
    let cr = ReviewRoot::parse(&text).unwrap().code_review.unwrap();
    assert_eq!(cr.founder.as_deref(), Some("github:someone"));
    assert!(text.contains("rust-analyzer: not recorded"));
    assert!(fresh_root_text("solo", Some("not an id"), None, "2026-10-07").is_err());
}

#[test]
fn founder_comes_only_from_keystore_identities() {
    assert_eq!(
        FounderChoice::from_ids(Vec::<String>::new()),
        FounderChoice::Unset
    );
    assert_eq!(
        keystore_founders(),
        FounderChoice::Unset,
        "tests never read the user's keystore"
    );
    let one = FounderChoice::from_ids([
        "github:a".to_string(),
        "github:a".to_string(),
        "bad id".to_string(),
    ]);
    assert_eq!(one, FounderChoice::One("github:a".into()));
    assert_eq!(one.default_founder().as_deref(), Some("github:a"));
    let several = FounderChoice::from_ids([
        "orcid:0000-0002-1825-0097".to_string(),
        "github:a".to_string(),
    ]);
    assert_eq!(
        several.default_founder(),
        None,
        "several: the user picks, nothing is guessed"
    );
    assert_eq!(several.ids().len(), 2);
    assert!(FounderChoice::Unset.ids().is_empty());
}

/// Schemas never break: roots written before #780 (the v1 and v2 review
/// fixtures) still read as valid. ~~They are kept byte for byte.~~
/// **CORRECTED 2026-10-07** (maintainer: record the rust-analyzer versions
/// used, never overwrite): every old byte is kept, as a prefix of the new
/// file, and one `[[code_review.rust_analyzer_used]]` entry is appended;
/// the pin is unchanged, and a second run with the same version appends
/// nothing.
#[test]
fn old_roots_still_load_and_are_kept_untouched() {
    for old in [V1_ROOT, V2_ROOT] {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join(ROOT_FILE), old).unwrap();
        let RootState::Valid { rust_analyzer, .. } = inspect_root(d.path()) else {
            panic!("old root must be valid")
        };
        assert_eq!(rust_analyzer.as_deref(), Some("0.3.2645"));
        let out = settle_root(
            d.path(),
            None,
            Some("github:x"),
            Some(&used("1.98.0")),
            "2026-10-07",
        )
        .unwrap();
        assert_eq!(
            out,
            RootOutcome::Kept {
                pinned: Some("0.3.2645".into()),
                pin_differs: true
            }
        );
        assert!(out.describe().contains("differs"));
        let new = std::fs::read_to_string(d.path().join(ROOT_FILE)).unwrap();
        assert!(new.starts_with(old) && new.len() > old.len());
        assert_eq!(rust_analyzer_history(d.path()), vec![used("1.98.0")]);
        let RootState::Valid { rust_analyzer, .. } = inspect_root(d.path()) else {
            panic!("still valid")
        };
        assert_eq!(rust_analyzer.as_deref(), Some("0.3.2645"), "pin unchanged");
        settle_root(d.path(), None, None, Some(&used("1.98.0")), "2026-10-08").unwrap();
        assert_eq!(
            std::fs::read_to_string(d.path().join(ROOT_FILE)).unwrap(),
            new,
            "same version: nothing appended"
        );
        // The literature side still opens it.
        crate::root::KovanRoot::open(d.path()).unwrap();
    }
}

#[test]
fn a_missing_root_is_created_and_a_corrupt_one_is_never_silently_overwritten() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    assert_eq!(inspect_root(p), RootState::Missing);
    let out = settle_root(p, None, None, Some(&used("1.98.0")), "2026-10-07").unwrap();
    assert_eq!(out, RootOutcome::Created { founder: None });
    assert!(out.describe().contains("UNSET"));
    assert!(matches!(inspect_root(p), RootState::Valid { .. }));

    let bad = "[code_review\nfounder = 1\n";
    std::fs::write(p.join(ROOT_FILE), bad).unwrap();
    let RootState::Corrupt { error, text } = inspect_root(p) else {
        panic!()
    };
    assert!(!error.is_empty() && text == bad);
    // No choice: refused, untouched.
    assert!(settle_root(p, None, None, None, "2026-10-07")
        .unwrap_err()
        .contains("corrupt"));
    assert_eq!(std::fs::read_to_string(p.join(ROOT_FILE)).unwrap(), bad);
    // Start fresh: the bad file is kept aside first.
    let out = settle_root(
        p,
        Some(&CorruptAction::StartFresh),
        Some("github:x"),
        None,
        "2026-10-07",
    )
    .unwrap();
    let RootOutcome::Replaced { kept_as, founder } = &out else {
        panic!("{out:?}")
    };
    assert_eq!(founder.as_deref(), Some("github:x"));
    assert!(kept_as.ends_with("kovan_root.toml.corrupt-2026-10-07"));
    assert_eq!(std::fs::read_to_string(kept_as).unwrap(), bad);
    assert!(matches!(inspect_root(p), RootState::Valid { .. }));
    // A second corrupt file the same day never overwrites the first.
    std::fs::write(p.join(ROOT_FILE), "x = \n").unwrap();
    let second = quarantine(p, "2026-10-07").unwrap();
    assert!(second.ends_with("kovan_root.toml.corrupt-2026-10-07-2"));
    assert_eq!(std::fs::read_to_string(kept_as).unwrap(), bad);
    // A reviewer-schema error (not a TOML error) is corrupt too.
    std::fs::write(
        p.join(ROOT_FILE),
        "[[reviewer]]\nid = \"nobody\"\nrole = \"reviewer\"\n",
    )
    .unwrap();
    assert!(matches!(inspect_root(p), RootState::Corrupt { .. }));
    std::fs::write(p.join(ROOT_FILE), [0xff, 0xfe]).unwrap();
    assert!(matches!(inspect_root(p), RootState::Corrupt { .. }));
    assert!(
        write_new(&p.join(ROOT_FILE), "x").is_err(),
        "write_new never overwrites"
    );
}

#[test]
fn a_corrupt_root_is_restored_from_the_last_committed_version_that_parses() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    if !git(p, &["init", "-q"]) {
        eprintln!("git unavailable: skipped");
        return;
    }
    assert_eq!(last_good_committed_root(p), Ok(None), "never committed");
    let good = fresh_root_text("solo", None, Some(&used("1.98.0")), "2026-10-01").unwrap();
    std::fs::write(p.join(ROOT_FILE), &good).unwrap();
    assert!(git(p, &["add", ROOT_FILE]) && git(p, &["commit", "-qm", "good root"]));
    std::fs::write(p.join(ROOT_FILE), "broken = [\n").unwrap();
    assert!(git(p, &["commit", "-qam", "broken root committed"]));
    std::fs::write(p.join(ROOT_FILE), "worse = [[\n").unwrap();
    let last = last_good_committed_root(p)
        .unwrap()
        .expect("the good commit");
    assert_eq!(last.text, good);
    assert_eq!(last.date.len(), 10);
    assert!(
        committed_root_at(p, "HEAD").is_err(),
        "the newest commit does not parse"
    );
    let out = settle_root(
        p,
        Some(&CorruptAction::Restore {
            commit: last.commit.clone(),
        }),
        None,
        None,
        "2026-10-07",
    )
    .unwrap();
    let RootOutcome::Restored { commit, kept_as } = &out else {
        panic!("{out:?}")
    };
    assert_eq!(commit, &last.commit);
    assert_eq!(std::fs::read_to_string(kept_as).unwrap(), "worse = [[\n");
    assert_eq!(std::fs::read_to_string(p.join(ROOT_FILE)).unwrap(), good);
    assert!(out.describe().contains("restored"));
    // Outside git: an error that says so, never a guess.
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(outside.path().join("x")).unwrap();
    if !git(outside.path(), &["rev-parse", "--git-dir"]) {
        assert!(last_good_committed_root(outside.path()).is_err());
    }
}

#[test]
fn review_skeletons_parse_and_never_overwrite() {
    let md = review_skeleton("crates/a/src", "2026-10-07T00:00:00Z").unwrap();
    let doc = parse_review_md(&md);
    assert!(doc.unreadable.is_empty(), "{:?}", doc.unreadable);
    assert_eq!(doc.entries.len(), 1);
    assert!(matches!(&doc.entries[0].entry, Entry::Other { kind, .. } if kind == "note"));
    assert_eq!(doc.reviews().count(), 0, "a skeleton counts as no review");
    assert!(parse_review_md(&review_skeleton("", "t").unwrap())
        .unreadable
        .is_empty());

    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    write(p, "src/x.rs", "");
    write(p, "tests/t.rs", "");
    write(
        p,
        "tests/review.md",
        "# mine\n\n```toml\n[kovan]\nkind = \"review\"\n```\n",
    );
    let folders = vec!["src".to_string(), "tests".to_string(), "gone".to_string()];
    assert_eq!(
        missing_review_mds(p, &folders),
        vec!["src/review.md".to_string(), "gone/review.md".to_string()]
    );
    let before = std::fs::read_to_string(p.join("tests/review.md")).unwrap();
    let r = create_missing_skeletons(p, &folders, "2026-10-07T00:00:00Z").unwrap();
    assert_eq!(r.created, vec!["src/review.md".to_string()]);
    assert_eq!(r.kept, vec!["tests/review.md".to_string()]);
    assert_eq!(r.unreadable, vec![("tests/review.md".to_string(), 1)]);
    assert_eq!(
        std::fs::read_to_string(p.join("tests/review.md")).unwrap(),
        before
    );
    assert!(!p.join("gone").exists());
    // Again: nothing new, the created one kept.
    let r = create_missing_skeletons(p, &folders, "later").unwrap();
    assert!(r.created.is_empty() && r.kept.len() == 2);
}

#[test]
fn recent_workspaces_are_their_own_list() {
    let d = tempfile::tempdir().unwrap();
    let file = d.path().join("cfg/recent_code_workspaces.toml");
    assert_eq!(
        RecentWorkspaces::load_from(&file),
        RecentWorkspaces::default()
    );
    let mut r = RecentWorkspaces::default();
    for i in 0..(super::recent::CAP + 3) {
        r.push(Path::new(&format!("/w{i}")));
    }
    r.push(Path::new("/w5"));
    assert_eq!(r.paths.len(), super::recent::CAP);
    assert_eq!(r.paths[0], Path::new("/w5"));
    assert_eq!(r.paths.iter().filter(|p| *p == Path::new("/w5")).count(), 1);
    r.save_to(&file).unwrap();
    assert_eq!(RecentWorkspaces::load_from(&file), r);
    assert_eq!(
        super::recent::default_file(),
        None,
        "tests never touch the user's list"
    );
}

#[test]
fn the_plan_of_a_single_crate_lists_its_files_and_cost() {
    assert!(cost_message(2, 3, 40).contains("2 crate(s), 3 indexed .rs file(s), 40 lines"));
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("solo");
    single_crate(&p);
    write(&p, "src/review.md", "# kept\n");
    let plan = match plan_fresh(&p) {
        Ok(plan) => plan,
        Err(e) => {
            eprintln!("cargo metadata unavailable ({e}): skipped");
            return;
        }
    };
    assert_eq!(plan.kind, ProjectKind::SingleCrate);
    assert_eq!(plan.crates, vec![("solo".to_string(), String::new())]);
    assert_eq!(plan.root, RootState::Missing);
    assert_eq!(plan.founders, FounderChoice::Unset);
    assert_eq!(
        (plan.cost.crates, plan.cost.rs_files),
        (1, 3),
        "lib.rs, a.rs, tests/it.rs; not main.rs"
    );
    let action = |path: &str| plan.files.iter().find(|f| f.path == path).map(|f| f.action);
    assert_eq!(action(ROOT_FILE), Some(FileAction::Create));
    assert_eq!(action("kovan_links.json"), Some(FileAction::Create));
    assert_eq!(action("src/kovan.toml"), Some(FileAction::Create));
    assert_eq!(action("tests/kovan.toml"), Some(FileAction::Create));
    assert_eq!(action("src/review.md"), Some(FileAction::Keep));
    assert_eq!(action("tests/review.md"), Some(FileAction::Create));
    assert!(FileAction::SetAside.label().contains("corrupt"));
    // The code map of the untagged crate: one placeholder, its own title.
    let (map, untagged) = crate::code_map::load_any_workspace(&p).unwrap();
    assert_eq!(untagged, vec!["solo".to_string()]);
    assert_eq!(map.root, "solo");
    assert!(
        crate::code_map::load_workspace(&p).is_err(),
        "the strict loader still wants tags"
    );

    // A corrupt root: the plan offers no restore outside git, and the run
    // refuses before writing anything.
    std::fs::write(p.join(ROOT_FILE), "nope = [\n").unwrap();
    let plan = plan_fresh(&p).unwrap();
    assert!(matches!(plan.root, RootState::Corrupt { .. }));
    let ctl = RunControl::default();
    let e = run_fresh(&p, &FreshChoices::default(), &ctl).unwrap_err();
    assert!(matches!(e, FreshError::CorruptRoot { .. }), "{e}");
    assert!(e.to_string().contains("not touched"));
    assert!(!p.join("src/kovan.toml").exists() && !p.join("kovan_links.json").exists());
    assert_eq!(
        std::fs::read_to_string(p.join(ROOT_FILE)).unwrap(),
        "nope = [\n"
    );
    // Not a Rust project at all.
    let e = run_fresh(d.path(), &FreshChoices::default(), &ctl).unwrap_err();
    assert!(matches!(e, FreshError::Detect(_)));
}

/// Methodology: the run's recording step on disk
/// ([`crate::commands::index::record_rust_analyzer_used`]): a missing root
/// gets nothing, a corrupt one is left byte for byte, a commented root keeps
/// every byte (old text a prefix of the new), the same version twice
/// appends once, a new version appends a second entry, and the report has
/// one line per version used.
///
/// Result (2026-10-07): passes.
#[test]
fn index_runs_append_the_rust_analyzer_used_and_never_rewrite_the_root() {
    use crate::commands::index::{record_rust_analyzer_used, used_commit, RecordUsed, NO_COMMIT};
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    let u = used("1.98.0");
    assert_eq!(record_rust_analyzer_used(p, &u), Ok(RecordUsed::NoRoot));
    assert!(RecordUsed::NoRoot.describe(&u).contains("not recorded"));
    std::fs::write(p.join(ROOT_FILE), "broken = [\n").unwrap();
    let r = record_rust_analyzer_used(p, &u).unwrap();
    assert!(matches!(r, RecordUsed::Unreadable(_)) && r.describe(&u).contains("untouched"));
    assert_eq!(
        std::fs::read_to_string(p.join(ROOT_FILE)).unwrap(),
        "broken = [\n"
    );
    assert!(
        settle_root(p, None, None, Some(&u), "2026-10-07").is_err(),
        "corrupt: refused"
    );
    let old = "# my notes, kept\nschema_version = 1\n\n[code_review] # inline comment\nrust_analyzer = \"0.3.2645\"\n# last line";
    std::fs::write(p.join(ROOT_FILE), old).unwrap();
    let r = record_rust_analyzer_used(p, &u).unwrap();
    assert_eq!(r, RecordUsed::Appended);
    assert!(r.describe(&u).contains("appended"));
    let once = std::fs::read_to_string(p.join(ROOT_FILE)).unwrap();
    assert!(once.starts_with(old), "byte-for-byte prefix:\n{once}");
    let r = record_rust_analyzer_used(p, &u).unwrap();
    assert_eq!(r, RecordUsed::AlreadyLast);
    assert!(r.describe(&u).contains("already"));
    assert_eq!(std::fs::read_to_string(p.join(ROOT_FILE)).unwrap(), once);
    assert_eq!(
        record_rust_analyzer_used(p, &used("1.99.0")),
        Ok(RecordUsed::Appended)
    );
    let twice = std::fs::read_to_string(p.join(ROOT_FILE)).unwrap();
    assert!(twice.starts_with(&once));
    let h = rust_analyzer_history(p);
    assert_eq!(h, vec![used("1.98.0"), used("1.99.0")]);
    let lines = history_lines(&h);
    assert_eq!(lines.len(), 2);
    assert!(lines[1].contains("1.99.0") && lines[1].contains("2026-10-07"));
    assert!(history_lines(&[])[0].contains("none recorded"));
    assert_eq!(used_commit(NO_COMMIT), "none");
    assert_eq!(used_commit("abc"), "abc");
    // Outside git (or no commit yet): "none".
    let outside = tempfile::tempdir().unwrap();
    if !git(outside.path(), &["rev-parse", "--verify", "HEAD"]) {
        assert_eq!(head_or_none(outside.path()), "none");
    }
}

/// Methodology: a single crate with source files, and a SCIP index that
/// holds one document of another workspace (its path written with `\`, as
/// rust-analyzer does on Windows). The index's tool version is the
/// installed rust-analyzer's, so the run reads the file and does not
/// regenerate it. The run must stop with an error that says the index holds
/// no document of the crate, and write nothing.
///
/// Before this check (GitHub #820, 2026-10-10) such a run finished without
/// error and wrote an empty link index and functions with no callees.
///
/// Result (2026-10-10, Windows 11, rust-analyzer 1.97.1): passes.
#[test]
fn an_index_that_matches_no_crate_is_refused_and_nothing_is_written() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    single_crate(p);
    let version =
        crate::commands::index::installed_rust_analyzer().unwrap_or_else(|| "1.97.1".into());
    let f = "rust-analyzer cargo other 0.1.0 m/f().";
    let occ = [(&[1u32, 7, 8][..], f, 1u64)];
    let elsewhere = tempfile::tempdir().unwrap();
    let scip = elsewhere.path().join("other.scip");
    let bytes = crate::scip::encode::index(&version, &[("crates\\other\\src\\lib.rs", &occ)]);
    std::fs::write(&scip, bytes).unwrap();
    let choices = FreshChoices {
        scip: Some(scip),
        ..FreshChoices::default()
    };
    let e = run_fresh(p, &choices, &RunControl::default()).unwrap_err();
    assert!(matches!(e, FreshError::Index(_)), "{e:?}");
    assert!(e.to_string().contains("holds no document"), "{e}");
    for rel in [ROOT_FILE, "kovan_links.json", "src/kovan.toml", "src/review.md"] {
        assert!(!p.join(rel).exists(), "{rel} was written");
    }
}
