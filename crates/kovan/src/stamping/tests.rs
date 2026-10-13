//! End-to-end tests of the stamp layer on a throwaway git repository in a
//! temporary folder: index, register a key (test passphrase, temporary
//! keystore), draft, sign, write `review.md`, judge; then edit and judge
//! again. Nothing touches the real keystore, `~/.config/kovan` or any
//! `review.md` / `kovan_root.toml` of the real workspace.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use kovan_common::call_graph::split::{StampState as WebStamp, StampVerdict};
use kovan_common::code_index::refresh::refresh_folder;
use kovan_common::review::signing::keystore::{generate, Keystore, UnlockedKey};
use kovan_common::review::state::StateKind;

use super::*;

pub(crate) const BY: &str = "github:tester";
pub(crate) const DIR: &str = "crates/demo/src";
pub(crate) const LIB: &str = "crates/demo/src/lib.rs";
pub(crate) const PASS: &str = "test passphrase, not a real one";

const LIB_SRC: &str = "\
/// Doubles x.
pub fn leaf(x: f64) -> f64 {
    x * 2.0
}

/// Twice x, plus one.
pub fn twice(x: f64) -> f64 {
    leaf(x) + 1.0
}

/// One.
pub fn other() -> u32 {
    1
}
";

const ROOT_SRC: &str = "# The workspace root: this comment must survive registration.\nschema_version = 1\n\n[library]\nid = \"demo\" # kept\n";

/// A malformed review entry (no `[review]` table): unreadable.
const BROKEN: &str = "# Review: broken (github:someone)\n\n```toml\n[kovan]\nid = \"review-broken\"\nkind = \"review\"\ncreated = \"c\"\nmodified = \"m\"\n```\n\nKeep   these   bytes.\n";

pub(crate) struct Repo(tempfile::TempDir);

impl Repo {
    pub(crate) fn path(&self) -> &Path {
        self.0.path()
    }
    pub(crate) fn git(&self, args: &[&str]) -> String {
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
    pub(crate) fn write(&self, file: &str, text: &str) {
        let p = self.path().join(file);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    pub(crate) fn read(&self, file: &str) -> String {
        std::fs::read_to_string(self.path().join(file)).unwrap()
    }
    pub(crate) fn edit(&self, from: &str, to: &str) {
        let t = self.read(LIB);
        assert!(t.contains(from), "fixture lacks {from:?}");
        self.write(LIB, &t.replacen(from, to, 1));
    }
    pub(crate) fn commit(&self, msg: &str) {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", msg]);
    }

    /// The fixture: a crate whose `twice` calls `leaf`, a `Cargo.lock`, a
    /// commented `kovan_root.toml`, and the folder's `kovan.toml` as a full
    /// index would write it (built with the rust-analyzer-free refresh,
    /// then the one call `twice -> leaf` filled in by hand, since the call
    /// graph needs rust-analyzer).
    pub(crate) fn new() -> Repo {
        let r = Repo(tempfile::tempdir().unwrap());
        r.git(&["init", "-q", "-b", "main"]);
        r.write(LIB, LIB_SRC);
        r.write("Cargo.lock", "version = 3\n");
        r.write(ROOT_FILE, ROOT_SRC);
        r.commit("initial");
        let head = r.git(&["rev-parse", "HEAD"]).trim().to_string();
        let files = BTreeMap::from([("lib.rs".to_string(), LIB_SRC.to_string())]);
        let (mut idx, _) = refresh_folder("demo", DIR, None, &files, None, &[], &head);
        idx.crate_root = true;
        let m = idx.modules.get_mut("lib.rs").unwrap();
        m.path = "crate".into();
        let leaf = m
            .functions
            .iter()
            .find(|f| f.qual == "leaf")
            .unwrap()
            .id
            .clone();
        for f in &mut m.functions {
            f.index_out_of_date = false;
            if f.qual == "twice" {
                f.callees = vec![leaf.clone()];
            }
        }
        r.write(&format!("{DIR}/kovan.toml"), &idx.to_toml().unwrap());
        r.commit("index");
        r
    }

    /// What "Index fresh" does after a code commit: refresh the folder's
    /// `kovan.toml` from the committed source (hashes now, ids kept), mark
    /// it up to date, commit. (Moved here from `review_mode/tests.rs` on
    /// 2026-10-10 so the app's headless tests can use it too.)
    pub(crate) fn reindex(&self) {
        use kovan_common::review::index::FolderIndex;
        let head = self.git(&["rev-parse", "HEAD"]).trim().to_string();
        let toml = format!("{DIR}/kovan.toml");
        let old = FolderIndex::parse(&self.read(&toml)).unwrap();
        let files = BTreeMap::from([("lib.rs".to_string(), self.read(LIB))]);
        let (mut idx, _) = refresh_folder("demo", DIR, Some(&old), &files, None, &[], &head);
        for m in idx.modules.values_mut() {
            for f in &mut m.functions {
                f.index_out_of_date = false;
            }
        }
        self.write(&toml, &idx.to_toml().unwrap());
        self.commit("reindex");
    }

    pub(crate) fn state(&self, qual: &str) -> Option<WebStamp> {
        let id = format!("{LIB}::{qual}");
        stamp_states(self.path())
            .unwrap()
            .into_iter()
            .find(|s| s.function == id)
    }

    /// Draft, sign and write a stamp of `qual`, then commit it.
    pub(crate) fn stamp(&self, key: &UnlockedKey, qual: &str, comments: &str) -> WrittenEntry {
        let req = StampRequest {
            function: format!("{LIB}::{qual}"),
            by: BY.into(),
            checklist: clean(),
            ..StampRequest::default()
        };
        let mut d = draft_stamp(self.path(), &req).unwrap();
        assert_eq!(d.call_graph_id, req.function);
        key.sign_review(&mut d.entry).unwrap();
        let w = write_review(self.path(), &d.entry, comments).unwrap();
        self.commit(&format!("stamp {qual}"));
        w
    }
}

/// A complete wizard answer set that stamps at rung 3 (the one the
/// engine's own tests use).
pub(crate) fn clean() -> BTreeMap<String, String> {
    [
        ("doc_matches_behaviour", "yes"),
        ("limits_and_guards", "guarded_returns_result"),
        ("error_handling", "returns_result"),
        ("numerical_hazards", "none_found"),
        ("test_reach", "reached_and_checked"),
        ("vv_evidence", "unit_tests_only"),
        ("vv_case_author", "agent_wrote_or_cowrote"),
        ("maintainability", "yes"),
        ("independence", "someone_else"),
        ("unintended_function", "no"),
        ("coding_standards", "yes"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

pub(crate) fn today() -> String {
    kovan_common::review::signed_at::date_of(&kovan_common::review::signed_at::now_local()).unwrap()
}

/// A key generated with the test passphrase into a temporary keystore and
/// registered in the fixture's root (the founder: first reviewer).
pub(crate) fn founder_key(r: &Repo, store: &Path) -> UnlockedKey {
    let ks = Keystore::at(store);
    let (kf, key) = generate(BY, "k1", &today(), PASS).unwrap();
    ks.save(&kf).unwrap();
    let back = ks.load(BY, "k1").unwrap().unlock(PASS).unwrap();
    assert_eq!(back.public_b64(), key.public_b64());
    assert_eq!(
        register_key(r.path(), &kf, Some("Test Reviewer"), &today()).unwrap(),
        KeyRegistration::Founder
    );
    r.commit("register key");
    key
}

/// Methodology: on the fixture (module doc), register a founder key, stamp
/// `twice` (draft at HEAD, sign, write `review.md`, commit) and build the
/// states with signatures enforced. Then edit `leaf` (twice's callee) and
/// commit, then edit `twice`'s own body and commit, re-judging each time.
/// Pass: valid at rung 3 with a permalink; inherited stale after the
/// callee edit; directly stale after its own edit; never valid again
/// without a new stamp. `leaf` and `other`, never stamped, are absent (the
/// web shows them unreviewed).
///
/// Also: `write_split` (the `--split-dir` data) carries the stamp.
///
/// Result (2026-10-10): passes.
#[test]
fn stamp_is_valid_then_goes_stale_on_edit() {
    let r = Repo::new();
    let store = tempfile::tempdir().unwrap();
    let key = founder_key(&r, store.path());
    let w = r.stamp(&key, "twice", "Checked against the doc.");
    assert!(!w.replaced && w.review_md == r.path().join(DIR).join(REVIEW_MD));

    let s = r.state("twice").expect("twice has a state");
    assert_eq!(
        (s.state, s.verdict, s.rung),
        (Some(StateKind::Valid), StampVerdict::Valid, 3),
        "{}",
        s.reason
    );
    assert_eq!(s.reviewer, BY);
    assert!(
        s.permalink.contains("/blob/") && s.permalink.ends_with("#L6-L9"),
        "{}",
        s.permalink
    );
    assert!(r.state("leaf").is_none() && r.state("other").is_none());

    // `kovan-cli call-graph --split-dir` (web-kovan's data) carries the
    // same review.md state (maintainer, 2026-10-10: switched from the
    // legacy stamps.toml).
    let split = tempfile::tempdir().unwrap();
    crate::commands::call_graph::write_split(r.path(), &Default::default(), split.path()).unwrap();
    let index: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(split.path().join("index.json")).unwrap())
            .unwrap();
    let stamps = index["stamps"].as_array().expect("index.json has stamps");
    assert!(
        stamps
            .iter()
            .any(|x| x["function"] == s.function.as_str() && x["rung"] == 3),
        "{stamps:?}"
    );

    let we = evaluate_workspace(r.path()).unwrap();
    assert!(we.evaluation.history_warnings.is_empty());

    r.edit("x * 2.0", "x * 3.0");
    r.commit("edit leaf");
    let s = r.state("twice").unwrap();
    assert_eq!(s.state, Some(StateKind::InheritedStale), "{}", s.reason);
    assert_eq!(s.verdict, StampVerdict::Stale);

    r.edit("leaf(x) + 1.0", "leaf(x) + 2.0");
    r.commit("edit twice");
    let s = r.state("twice").unwrap();
    assert_eq!(s.state, Some(StateKind::DirectlyStale), "{}", s.reason);
}

/// Methodology: with an unreadable entry committed in `review.md`, stamp
/// `twice` twice by the same reviewer. Pass: the second write replaces the
/// first (one standing review per reviewer per function, #764), the
/// unreadable entry survives byte for byte through both writes, and the
/// function is still valid. Also: an unsigned review is refused, a draft
/// on an uncommitted edit is refused, a needs-fix on `other` shows
/// "needs fix", a stamp committed with the agent attribution trailer is
/// unverified, and registering a second reviewer and a second key gives
/// "awaiting admission" and "needs endorsement" with the root's comments
/// kept.
///
/// Result (2026-10-10): passes.
#[test]
fn restamp_replaces_keeps_unreadable_and_registration_keeps_comments() {
    let r = Repo::new();
    let store = tempfile::tempdir().unwrap();
    let key = founder_key(&r, store.path());
    r.write(&format!("{DIR}/{REVIEW_MD}"), BROKEN);
    r.commit("a broken entry");
    let first = r.stamp(&key, "twice", "First.");
    assert!(!first.replaced);
    let second = r.stamp(&key, "twice", "Second.");
    assert!(second.replaced);
    let md = r.read(&format!("{DIR}/{REVIEW_MD}"));
    assert!(
        md.starts_with(BROKEN),
        "unreadable entry kept byte for byte:\n{md}"
    );
    assert!(md.contains("Second.") && !md.contains("First."));
    assert_eq!(parse_review_md(&md).reviews().count(), 1);
    assert_eq!(r.state("twice").unwrap().state, Some(StateKind::Valid));

    // Refusals.
    let req = StampRequest {
        function: format!("{LIB}::twice"),
        by: BY.into(),
        checklist: clean(),
        ..StampRequest::default()
    };
    let d = draft_stamp(r.path(), &req).unwrap();
    assert!(write_review(r.path(), &d.entry, "")
        .unwrap_err()
        .contains("unsigned"));
    r.edit("leaf(x) + 1.0", "leaf(x) + 5.0");
    assert!(draft_stamp(r.path(), &req)
        .unwrap_err()
        .contains("not committed"));
    r.git(&["checkout", "--", LIB]);

    // Needs fix.
    let n = draft_needs_fix_for(
        r.path(),
        &format!("{LIB}::other"),
        BY,
        "returns a magic number",
    )
    .unwrap();
    assert!(!write_needs_fix(r.path(), &n, "").unwrap().replaced);
    assert_eq!(
        r.state("other").unwrap().state,
        Some(StateKind::NeedsFixOpen)
    );
    assert!(r.read(&format!("{DIR}/{REVIEW_MD}")).starts_with(BROKEN));

    // A stamp committed with the agent attribution trailer never counts.
    let mut d = draft_stamp(
        r.path(),
        &StampRequest {
            function: format!("{LIB}::leaf"),
            ..req.clone()
        },
    )
    .unwrap();
    key.sign_review(&mut d.entry).unwrap();
    write_review(r.path(), &d.entry, "").unwrap();
    r.git(&["add", "-A"]);
    r.git(&[
        "commit",
        "-q",
        "-m",
        "stamp leaf\n\nClaude-Session: https://example.invalid/session",
    ]);
    let s = r.state("leaf").unwrap();
    assert_eq!(s.state, Some(StateKind::Unverified), "{}", s.reason);
    assert!(s.reason.contains("AgentTrailer"), "{}", s.reason);

    // Registration.
    let (kf2, _) = generate("github:second", "s1", &today(), PASS).unwrap();
    assert_eq!(
        register_key(r.path(), &kf2, None, &today()).unwrap(),
        KeyRegistration::AwaitingAdmission
    );
    let (kf3, _) = generate(BY, "k2", &today(), PASS).unwrap();
    assert_eq!(
        register_key(r.path(), &kf3, None, &today()).unwrap(),
        KeyRegistration::KeyAdded {
            needs_endorsement: true
        }
    );
    let root = r.read(ROOT_FILE);
    assert!(
        root.starts_with("# The workspace root: this comment must survive registration.\n"),
        "{root}"
    );
    assert!(root.contains("id = \"demo\" # kept"));
    let parsed = kovan_common::review::root::ReviewRoot::parse(&root).unwrap();
    assert_eq!(parsed.reviewer(BY).unwrap().keys.len(), 2);
    let new = kovan_common::review::root_append::unadmitted_reviewer(
        "github:third",
        None,
        kf3.reviewer_key(),
    );
    register_reviewer(r.path(), &new).unwrap();
    assert!(register_reviewer(r.path(), &new).is_err());
}

/// Methodology: add a `#[test]` function to the fixture's `lib.rs`, commit
/// and re-index, then open the wizard's context for it and for `leaf`
/// (not a test).
///
/// Pass: the test function's context is marked as a test and starts with
/// the four "this is a test" answers (limits, error handling, test reach,
/// V&V evidence); `leaf` starts with none of them.
///
/// Result (2026-10-12): passes.
#[test]
fn a_test_function_starts_with_its_this_is_a_test_answers() {
    let r = Repo::new();
    let store = tempfile::tempdir().unwrap();
    let _key = founder_key(&r, store.path());
    let src = r.read(LIB);
    r.write(LIB, &format!("{src}\n#[test]\nfn checks_other() {{\n    assert_eq!(other(), 1);\n}}\n"));
    r.commit("add a test");
    r.reindex();

    let t = prepare_stamp(r.path(), &format!("{LIB}::checks_other"), BY).unwrap();
    assert!(t.applicability.is_test);
    for (q, o) in [
        ("limits_and_guards", "test_no_input"),
        ("error_handling", "test_panics_on_failure"),
        ("test_reach", "is_a_test"),
        ("vv_evidence", "is_a_test"),
    ] {
        assert_eq!(t.answers.get(q).map(String::as_str), Some(o), "{q}");
    }
    let l = prepare_stamp(r.path(), &format!("{LIB}::leaf"), BY).unwrap();
    assert!(!l.applicability.is_test);
    assert!(!l.answers.values().any(|a| a.starts_with("test_") || a == "is_a_test"), "{:?}", l.answers);
}

/// The answers a named number (constant) stamps at rung 3 with.
pub(crate) fn clean_named_number() -> BTreeMap<String, String> {
    [
        ("item_matches_description", "yes"),
        ("item_test_use", "no_test_uses"),
        ("item_value_source", "cited"),
        ("vv_evidence", "unit_tests_only"),
        ("vv_case_author", "agent_wrote_or_cowrote"),
        ("maintainability", "yes"),
        ("independence", "someone_else"),
        ("coding_standards", "yes"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

/// The answers a record (struct) stamps at rung 3 with.
pub(crate) fn clean_record() -> BTreeMap<String, String> {
    let mut m = clean_named_number();
    m.remove("item_value_source");
    m.insert("item_holds_extra".into(), "no".into());
    m.insert("item_impossible_values".into(), "no_every_combination_valid".into());
    m
}

/// Methodology (#843): add a struct with a method and a constant to the
/// fixture's library, commit, reindex with the rust-analyzer-free refresh,
/// and record by hand that `twice` uses the struct (the uses need
/// rust-analyzer, as the fixture's one call does). Then review bottom-up.
///
/// Pass: both items are in `kovan.toml` with their shape, and the struct's
/// entry hashes as the struct at HEAD does. `twice` cannot be stamped
/// while the struct it uses has no valid review (the refusal names it),
/// even after its callee `leaf` is stamped. The struct is asked the record
/// questions and the constant the named-number ones; function answers on
/// the struct are refused; each stamps valid at rung 3 with its own
/// answers. `twice` then stamps, and its signed entry records the struct's
/// hash beside `leaf`'s. Editing the struct's **method** makes the struct
/// "changed since review" and `twice` "a callee changed"; the constant is
/// untouched. Quick stamp (re-confirm) of `twice` is then refused until the
/// struct is reviewed again, after which it names the struct as what
/// changed and makes `twice` valid at rung 3 without the wizard (#857).
///
/// Result (2026-10-13): passes.
#[test]
fn types_and_constants_are_reviewed_bottom_up() {
    use kovan_common::review::index::{FolderIndex, ItemShape};
    let r = Repo::new();
    let store = tempfile::tempdir().unwrap();
    let key = founder_key(&r, store.path());
    let src = format!(
        "{}\n/// Failure fraction of each layer.\npub struct Fractions {{\n    pub sic: f64,\n}}\n\nimpl Fractions {{\n    pub fn total(&self) -> f64 {{\n        self.sic\n    }}\n}}\n\n/// Boltzmann constant, J/K, CODATA 2018.\npub const K_B: f64 = 1.380649e-23;\n",
        r.read(LIB)
    );
    r.write(LIB, &src);
    r.commit("add a record and a named number");
    r.reindex();
    let toml = format!("{DIR}/kovan.toml");
    let mut idx = FolderIndex::parse(&r.read(&toml)).unwrap();
    let entry = |idx: &FolderIndex, q: &str| idx.modules["lib.rs"].functions.iter().find(|f| f.qual == q).cloned().unwrap();
    assert_eq!(entry(&idx, "Fractions").shape, Some(ItemShape::Struct));
    assert_eq!(entry(&idx, "K_B").shape, Some(ItemShape::Const));
    assert_eq!(entry(&idx, "Fractions::total").shape, None);
    let fractions = entry(&idx, "Fractions").id;
    for f in &mut idx.modules.get_mut("lib.rs").unwrap().functions {
        if f.qual == "twice" {
            f.uses = vec![fractions.clone()];
        }
    }
    r.write(&toml, &idx.to_toml().unwrap());
    r.commit("record that twice uses Fractions");
    let head = r.git(&["rev-parse", "HEAD"]).trim().to_string();
    let mut cache = states::FileCache::new();
    let at_head = states::fn_at(&mut cache, r.path(), &head, LIB, "Fractions").unwrap();
    assert_eq!(at_head.hashes.hash, entry(&idx, "Fractions").hash);

    let stamp_with = |qual: &str, checklist: BTreeMap<String, String>| -> Result<WrittenEntry, String> {
        let req = StampRequest { function: format!("{LIB}::{qual}"), by: BY.into(), checklist, ..StampRequest::default() };
        let mut d = draft_stamp(r.path(), &req)?;
        key.sign_review(&mut d.entry).unwrap();
        let w = write_review(r.path(), &d.entry, "")?;
        r.commit(&format!("stamp {qual}"));
        Ok(w)
    };
    // Bottom-up: the callee first, then the struct, then the user.
    let e = prepare_stamp(r.path(), &format!("{LIB}::twice"), BY).unwrap_err();
    assert!(e.contains("bottom-up"), "{e}");
    r.stamp(&key, "leaf", "");
    let e = prepare_stamp(r.path(), &format!("{LIB}::twice"), BY).unwrap_err();
    assert!(e.contains("bottom-up") && e.contains("Fractions"), "{e}");

    let ctx = prepare_stamp(r.path(), &format!("{LIB}::Fractions"), BY).unwrap();
    assert_eq!(ctx.applicability.shape, Some(ItemShape::Struct));
    let asked: Vec<&str> = ReviewWizard::embedded().applicable(ctx.applicability).map(|q| q.key.as_str()).collect();
    assert!(asked.contains(&"item_impossible_values") && !asked.contains(&"error_handling"), "{asked:?}");
    let e = stamp_with("Fractions", clean()).unwrap_err();
    assert!(e.contains("error_handling") || e.contains("not applicable") || e.contains("NotApplicable"), "{e}");
    stamp_with("Fractions", clean_record()).unwrap();
    assert_eq!(r.state("Fractions").unwrap().kind(), StateKind::Valid);
    assert_eq!(r.state("Fractions").unwrap().rung, 3);

    let ctx = prepare_stamp(r.path(), &format!("{LIB}::K_B"), BY).unwrap();
    assert_eq!(ctx.applicability.shape, Some(ItemShape::Const));
    assert!(stamp_with("K_B", clean_record()).is_err());
    stamp_with("K_B", clean_named_number()).unwrap();
    assert_eq!(r.state("K_B").unwrap().kind(), StateKind::Valid);

    let w = stamp_with("twice", clean()).unwrap();
    assert_eq!(r.state("twice").unwrap().kind(), StateKind::Valid);
    let written = std::fs::read_to_string(&w.review_md).unwrap();
    assert!(written.contains(&format!("\"{fractions}\" = \"{}\"", entry(&idx, "Fractions").hash)), "{written}");

    // A method of the struct changes: the struct is stale, its user needs
    // a re-confirm, the constant is untouched.
    r.edit("        self.sic\n", "        self.sic * 1.0\n");
    r.commit("edit a method of Fractions");
    r.reindex();
    assert_eq!(r.state("Fractions").unwrap().kind(), StateKind::DirectlyStale);
    assert_eq!(r.state("twice").unwrap().kind(), StateKind::InheritedStale);
    assert_eq!(r.state("K_B").unwrap().kind(), StateKind::Valid);
    assert_eq!(r.state("leaf").unwrap().kind(), StateKind::Valid);

    // Quick stamp covers a changed TYPE too (#857, maintainer 2026-10-13):
    // `twice` is stale only because the record it uses changed. Refused
    // while the record itself has no valid review (bottom-up), then the
    // re-confirm names the record, and signing it makes `twice` valid again
    // with the record's new hash and the same answers.
    let e = reconfirm::prepare_reconfirm(r.path(), &format!("{LIB}::twice"), BY).unwrap_err();
    assert!(e.contains("bottom-up") && e.contains("Fractions"), "{e}");
    stamp_with("Fractions", clean_record()).unwrap();
    assert_eq!(r.state("twice").unwrap().kind(), StateKind::InheritedStale);
    let rc = reconfirm::prepare_reconfirm(r.path(), &format!("{LIB}::twice"), BY).unwrap();
    assert_eq!(rc.changed.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Fractions"]);
    assert_eq!(rc.changed[0].id, fractions);
    // The record's own lines did not change (its method did), so its diff
    // is found and empty: nothing to misread.
    assert_eq!(rc.changed[0].diff.as_ref().map(|d| d.diff.counts()), Ok((0, 0)));
    reconfirm::sign_reconfirm(r.path(), &rc, &key).unwrap();
    r.commit("quick stamp twice");
    let s = r.state("twice").unwrap();
    assert_eq!((s.kind(), s.rung), (StateKind::Valid, 3), "{}", s.reason);
    let md = r.read(&format!("{DIR}/review.md"));
    assert!(md.contains("Re-confirmed on") && md.contains("Fractions changed"), "{md}");
}

/// Methodology (#856): the workspace cache against a full load, on the
/// fixture plus a second indexed folder (`crates/demo/src/sub`). After each
/// change below the cached load must equal [`load_workspace_uncached`]
/// (every folder index, every `review.md`, `HEAD`), and the number of
/// folders it refreshed must be the number the change touches:
///
/// 1. first load: both folders; 2. nothing changed: none;
/// 3. one file edited to a text of the **same length**, not committed: its
///    folder only (the edit is caught by content, since the timestamp is
///    too fresh to trust);
/// 4. commit and reindex, then a stamp is written (`review.md`, new
///    `HEAD`, new claims): the stamped folder only, the other reused;
/// 5. a new file with a new function in `sub` (its id is minted with
///    `HEAD`): `sub` only; then a commit moves `HEAD`: `sub` again, because
///    its minted id depends on it, while the other folder is reused;
/// 6. the file is deleted: `sub` only; 7. `forget`: both again.
///
/// Result (2026-10-13): passes.
#[test]
fn the_cache_gives_what_a_full_load_gives() {
    let r = Repo::new();
    let store = tempfile::tempdir().unwrap();
    let key = founder_key(&r, store.path());
    let sub = format!("{DIR}/sub");
    r.write(&format!("{sub}/a.rs"), "pub fn deep() -> u8 {\n    1\n}\n");
    r.commit("a second folder");
    let head = r.git(&["rev-parse", "HEAD"]).trim().to_string();
    let files = BTreeMap::from([("a.rs".to_string(), r.read(&format!("{sub}/a.rs")))]);
    let (mut idx, _) = refresh_folder("demo", &sub, None, &files, None, &[], &head);
    for m in idx.modules.values_mut() {
        m.path = "crate::sub::a".into();
        for f in &mut m.functions {
            f.index_out_of_date = false;
        }
    }
    r.write(&format!("{sub}/kovan.toml"), &idx.to_toml().unwrap());
    r.commit("index the second folder");

    let same_as_full = |what: &str| -> usize {
        let cached = load_workspace(r.path()).unwrap();
        let full = load_workspace_uncached(r.path()).unwrap();
        assert_eq!(cached.indexes, full.indexes, "{what}");
        assert_eq!(cached.reviews, full.reviews, "{what}");
        assert_eq!(cached.head, full.head, "{what}");
        assert_eq!(format!("{:?}", cached.review_root), format!("{:?}", full.review_root), "{what}");
        ws_cache::refreshed_last(r.path()).unwrap()
    };
    ws_cache::forget(r.path());
    assert_eq!(same_as_full("first load"), 2);
    assert_eq!(same_as_full("nothing changed"), 0);
    r.edit("x * 2.0", "x * 3.0");
    assert_eq!(same_as_full("a same-length edit"), 1);
    assert_eq!(same_as_full("after the edit, nothing more"), 0);
    r.commit("edit leaf");
    r.reindex();
    assert_eq!(same_as_full("reindexed"), 1);
    r.stamp(&key, "leaf", "");
    assert_eq!(same_as_full("a stamp was written"), 1);
    assert_eq!(same_as_full("after the stamp, nothing more"), 0);
    r.write(&format!("{sub}/b.rs"), "pub fn fresh() {}\n");
    assert_eq!(same_as_full("a new file"), 1);
    r.commit("add b.rs");
    assert_eq!(same_as_full("HEAD moved under a minted id"), 1);
    std::fs::remove_file(r.path().join(format!("{sub}/b.rs"))).unwrap();
    assert_eq!(same_as_full("a file deleted"), 1);
    ws_cache::forget(r.path());
    assert_eq!(same_as_full("forgotten"), 2);
}
