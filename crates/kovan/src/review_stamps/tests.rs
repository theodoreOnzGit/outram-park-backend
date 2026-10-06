//! End-to-end tests of the stamps on a throwaway git repository: stamp a
//! function, change the code in each way the normalisation distinguishes,
//! and check what `check` says. Every test builds its own repository in a
//! temporary folder; nothing touches the real `review/stamps.toml`.

use std::path::Path;
use std::process::Command;

use super::*;
use crate::code_map::{CrateNode, MaturityModule, Topic};

const LIB: &str = "crates/demo/src/lib.rs";

const LIB_SRC: &str = "\
//! A demo crate.

/// Doubles x.
pub fn double(x: f64) -> f64 {
    x * 2.0
}

pub struct S;

impl S {
    /// Halves x.
    pub fn half(&self, x: f64) -> f64 {
        x / 2.0
    }
}

pub fn other() -> u32 {
    1
}

pub mod sub;

#[cfg(test)]
mod tests {
    #[test]
    fn t() {}
}
";

const SUB_SRC: &str = "/// In sub.\npub fn sub_fn() -> u8 {\n    7\n}\n";

struct Repo(tempfile::TempDir);

impl Repo {
    fn new() -> Repo {
        let r = Repo(tempfile::tempdir().unwrap());
        r.git(&["init", "-q", "-b", "main"]);
        r.write(LIB, LIB_SRC);
        r.write("crates/demo/src/sub.rs", SUB_SRC);
        r.commit("initial");
        r
    }
    fn path(&self) -> &Path {
        self.0.path()
    }
    fn git(&self, args: &[&str]) -> String {
        let out = Command::new("git")
            .current_dir(self.path())
            .args([
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    }
    fn write(&self, file: &str, text: &str) {
        let p = self.path().join(file);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    fn edit(&self, from: &str, to: &str) {
        let text = std::fs::read_to_string(self.path().join(LIB)).unwrap();
        assert!(text.contains(from), "fixture lacks {from:?}");
        self.write(LIB, &text.replacen(from, to, 1));
    }
    fn commit(&self, msg: &str) {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", msg]);
    }
    fn stamp(&self, f: &str) -> Stamp {
        let s =
            stamp_function(self.path(), &format!("{LIB}::{f}"), 3, "Test Reviewer", "").unwrap();
        self.commit(&format!("stamp {f}"));
        s
    }
    fn only(&self) -> StampCheck {
        let r = check(self.path(), &Scope::All).unwrap();
        assert_eq!(r.checked.len(), 1);
        r.checked[0].clone()
    }
}

/// Methodology: stamp a method; the record carries the path, file, lines
/// (doc comment included), HEAD's commit id and a hash; the file is created
/// from the documented template; the stamp checks valid. Refusals: rung 2,
/// a blank reviewer, an unknown function, and a function whose file has
/// uncommitted changes.
///
/// Result (2026-10-06): passes.
#[test]
fn stamping_records_the_committed_function_and_refuses_bad_requests() {
    let repo = Repo::new();
    let head = repo.git(&["rev-parse", "HEAD"]).trim().to_string();
    let s = stamp_function(
        repo.path(),
        &format!("{LIB}::S::half"),
        4,
        "Test Reviewer",
        "checked",
    )
    .unwrap();
    assert_eq!(
        s.function.as_deref(),
        Some("crates/demo/src/lib.rs::S::half")
    );
    assert_eq!(
        (s.file.as_str(), s.lines, s.commit.as_str()),
        (LIB, [11, 14], head.as_str())
    );
    assert!(s.hash.starts_with("sha256:"));
    assert_eq!(s.date.to_string().len(), 10);
    let text = std::fs::read_to_string(repo.path().join(STAMPS_FILE)).unwrap();
    assert!(text.starts_with(TEMPLATE) && text.contains("[[stamp]]") && text.contains("rung = 4"));
    assert_eq!(load(repo.path()).unwrap(), vec![s.clone()]);
    assert_eq!(repo.only().verdict, Verdict::Valid);
    assert!(s
        .permalink(DEFAULT_REPO_URL)
        .ends_with(&format!("/blob/{head}/{LIB}#L11-L14")));

    assert!(stamp_function(repo.path(), &format!("{LIB}::double"), 2, "R", "").is_err());
    assert!(stamp_function(repo.path(), &format!("{LIB}::double"), 3, "  ", "").is_err());
    assert!(stamp_function(repo.path(), &format!("{LIB}::nope"), 3, "R", "").is_err());
    repo.edit("x * 2.0", "x * 2.5");
    let e = stamp_function(repo.path(), &format!("{LIB}::double"), 3, "R", "").unwrap_err();
    assert!(e.contains("uncommitted"), "{e}");
    assert_eq!(
        load(repo.path()).unwrap().len(),
        1,
        "a refusal must not append"
    );
}

/// Methodology: one stamp, then each kind of edit on its own (committed or
/// not; `check` reads the working tree): a code token, rustfmt-style
/// reformatting, a `//` comment, a `///` line, moving the function lower in
/// the file, and renaming it.
///
/// Result (2026-10-06): passes — code edit VOID (code changed), reformat
/// VALID, `//` edit VALID, `///` edit VOID (doc comment changed), move VALID
/// with the new lines reported, rename VOID (function not found).
#[test]
fn each_kind_of_edit_keeps_or_voids_the_stamp_as_specified() {
    let repo = Repo::new();
    repo.stamp("double");
    let clean = std::fs::read_to_string(repo.path().join(LIB)).unwrap();
    let reset = || repo.write(LIB, &clean);

    repo.edit("x * 2.0", "x * 2.0 + 0.0");
    assert_eq!(repo.only().verdict, Verdict::Void(VoidReason::CodeChanged));
    reset();

    repo.edit(
        "pub fn double(x: f64) -> f64 {\n    x * 2.0\n}",
        "pub fn double(x:f64)->f64{x*2.0}",
    );
    assert_eq!(repo.only().verdict, Verdict::Valid);
    reset();

    repo.edit(
        "    x * 2.0\n",
        "    // why: doubling is the model\n    x * 2.0 // exact\n",
    );
    assert_eq!(repo.only().verdict, Verdict::Valid);
    reset();

    repo.edit("/// Doubles x.", "/// Doubles x exactly.");
    assert_eq!(repo.only().verdict, Verdict::Void(VoidReason::DocChanged));
    reset();

    repo.edit("/// Doubles x.", "/// Triples x.");
    repo.edit("x * 2.0", "x * 3.0");
    assert_eq!(
        repo.only().verdict,
        Verdict::Void(VoidReason::CodeAndDocChanged)
    );
    reset();

    let moved = clean
        .replacen(
            "/// Doubles x.\npub fn double(x: f64) -> f64 {\n    x * 2.0\n}\n",
            "",
            1,
        )
        .replacen(
            "pub mod sub;\n",
            "pub mod sub;\n\n/// Doubles x.\npub fn double(x: f64) -> f64 {\n    x * 2.0\n}\n",
            1,
        );
    repo.write(LIB, &moved);
    let c = repo.only();
    assert_eq!(c.verdict, Verdict::Valid);
    assert_eq!((c.stamp.lines, c.now_lines), ([3, 6], Some([19, 22])));
    assert!(
        render_report(&check(repo.path(), &Scope::All).unwrap(), DEFAULT_REPO_URL)
            .contains("now lines 19-22 (stamped 3-6)")
    );
    reset();

    repo.edit("pub fn double(", "pub fn twice(");
    let c = repo.only();
    assert_eq!(c.verdict, Verdict::Void(VoidReason::FunctionNotFound));
    let report = render_report(&check(repo.path(), &Scope::All).unwrap(), DEFAULT_REPO_URL);
    assert!(
        report.contains("VOID   #1 crates/demo/src/lib.rs::double")
            && report.contains("function not found"),
        "{report}"
    );
    assert!(
        report.contains(&format!("stamped: {DEFAULT_REPO_URL}/blob/")),
        "{report}"
    );
    assert!(
        report.ends_with("0 valid, 1 void, 0 stale but superseded, 0 unchecked\n"),
        "{report}"
    );
}

/// Methodology: two stamps (a free function and a method). A commit that
/// changes only an unstamped function puts nothing in `--diff` scope; a
/// commit that changes the method puts only its stamp in scope, void,
/// judged at the range's end even after the working tree is restored; a
/// diff over both commits covers it too. Then a re-stamp of the method makes
/// the old stamp superseded (stale, not a failure); and `check` never
/// rewrites `stamps.toml`.
///
/// Result (2026-10-06): passes.
#[test]
fn diff_limits_scope_and_a_restamp_supersedes() {
    let repo = Repo::new();
    repo.stamp("double");
    repo.stamp("S::half");
    let before = std::fs::read_to_string(repo.path().join(STAMPS_FILE)).unwrap();

    repo.edit("    1\n", "    2\n");
    repo.commit("change other");
    let r = check(repo.path(), &Scope::Diff("HEAD~1..HEAD".into())).unwrap();
    assert_eq!((r.recorded, r.checked.len()), (2, 0));

    repo.edit("x / 2.0", "x * 0.5");
    repo.commit("change half");
    repo.edit("x * 0.5", "x / 2.0"); // working tree only: --diff must ignore it
    let r = check(repo.path(), &Scope::Diff("HEAD~1..HEAD".into())).unwrap();
    assert_eq!(r.checked.len(), 1);
    assert_eq!(
        r.checked[0].stamp.function.as_deref(),
        Some("crates/demo/src/lib.rs::S::half")
    );
    assert_eq!(r.checked[0].verdict, Verdict::Void(VoidReason::CodeChanged));
    assert_eq!(r.failures().count(), 1);
    assert_eq!(
        check(repo.path(), &Scope::Diff("HEAD~2..".into()))
            .unwrap()
            .checked
            .len(),
        1
    );
    assert!(check(repo.path(), &Scope::Diff("HEAD".into())).is_err());

    repo.edit("x / 2.0", "x * 0.5");
    repo.stamp("S::half");
    let r = check(repo.path(), &Scope::All).unwrap();
    let verdicts: Vec<_> = r
        .checked
        .iter()
        .map(|c| (c.verdict.clone(), c.superseded_by))
        .collect();
    assert_eq!(
        verdicts,
        vec![
            (Verdict::Valid, None),
            (Verdict::Void(VoidReason::CodeChanged), Some(2)),
            (Verdict::Valid, None)
        ]
    );
    assert_eq!(r.failures().count(), 0);
    assert!(render_report(&r, DEFAULT_REPO_URL).contains("STALE  #2"));
    let after = std::fs::read_to_string(repo.path().join(STAMPS_FILE)).unwrap();
    assert!(after.starts_with(&before), "stamps are only ever appended");
}

/// Methodology: the schema is not function-only (#743): an `artifact`
/// stamp loads and is reported UNCHECKED (neither valid nor a failure);
/// setting both targets, or neither, is refused on load.
///
/// Result (2026-10-06): passes.
#[test]
fn artifact_stamps_load_and_are_reported_unchecked() {
    let repo = Repo::new();
    let art = "[[stamp]]\nartifact = \"review/walks/x.md#step-1\"\nfile = \"review/walks/x.md\"\nlines = [1, 9]\ncommit = \"0\"\nhash = \"sha256:0\"\nrung = 3\nreviewer = \"R\"\ndate = 2026-10-06\n";
    repo.write(STAMPS_FILE, art);
    let r = check(repo.path(), &Scope::All).unwrap();
    assert!(matches!(r.checked[0].verdict, Verdict::Unchecked(_)));
    assert_eq!(r.failures().count(), 0);
    repo.write(
        STAMPS_FILE,
        &art.replace(
            "[[stamp]]\n",
            &format!("[[stamp]]\nfunction = \"{LIB}::double\"\n"),
        ),
    );
    assert!(load(repo.path()).unwrap_err().contains("exactly one"));
}

fn node(dir: &Path, maturity: u8, modules: &[(&str, u8)]) -> CrateNode {
    CrateNode {
        name: "demo".into(),
        description: None,
        row: 1,
        topic: Topic::Utility,
        fidelity: None,
        maturity,
        maturity_modules: modules
            .iter()
            .map(|(m, l)| MaturityModule {
                module: m.to_string(),
                level: *l,
                why: String::new(),
            })
            .collect(),
        lib_dir: Some(dir.join("crates/demo/src").to_string_lossy().into_owned()),
        dir: Some("crates/demo".into()),
    }
}

/// Methodology: a crate tagged at rung 3 with module `sub` at rung 4. With
/// no stamps both claims are UNSUPPORTED and list every function (the
/// `#[cfg(test)]` function excluded); stamping `sub_fn` at rung 4 supports
/// the module; stamping the other three lib functions at rung 3 supports
/// the crate and places it at 3 from stamps; a void stamp is listed as void;
/// a rung-2 claim is not applicable; a missing module is a problem.
///
/// Result (2026-10-06): passes.
#[test]
fn derived_levels_report_which_claims_stamps_support() {
    let repo = Repo::new();
    let d = derived_levels_for(repo.path(), &node(repo.path(), 3, &[("sub", 4)])).unwrap();
    assert_eq!((d.claims[0].functions, d.claims[1].functions), (4, 1));
    assert!(matches!(d.claims[0].support, Support::Unsupported(_)));
    assert_eq!(
        d.claims[1].unstamped,
        vec!["crates/demo/src/sub.rs::sub_fn".to_string()]
    );
    assert_eq!(d.from_stamps, None);

    stamp_function(repo.path(), "crates/demo/src/sub.rs::sub_fn", 4, "R", "").unwrap();
    for f in ["double", "S::half", "other"] {
        stamp_function(repo.path(), &format!("{LIB}::{f}"), 3, "R", "").unwrap();
    }
    repo.commit("stamps");
    let d = derived_levels_for(repo.path(), &node(repo.path(), 3, &[("sub", 4)])).unwrap();
    assert_eq!(
        d.claims[0].support,
        Support::Supported,
        "{}",
        levels::render(&d)
    );
    assert_eq!(d.claims[1].support, Support::Supported);
    assert_eq!(d.from_stamps, Some(3));
    let d4 = derived_levels_for(repo.path(), &node(repo.path(), 4, &[])).unwrap();
    assert_eq!(d4.claims[0].below_level.len(), 3);

    repo.edit("    1\n", "    2\n");
    let d = derived_levels_for(repo.path(), &node(repo.path(), 2, &[("nope", 3)])).unwrap();
    assert_eq!(d.claims[0].support, Support::NotApplicable);
    assert_eq!(d.claims[0].void, vec![format!("{LIB}::other")]);
    assert!(
        matches!(d.claims[1].support, Support::Unsupported(_)) && !d.claims[1].problems.is_empty()
    );
    assert!(levels::render(&d).contains("module nope = 3 (human reviewed): UNSUPPORTED"));
}
