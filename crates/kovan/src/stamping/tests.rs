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

pub(super) const BY: &str = "github:tester";
pub(super) const DIR: &str = "crates/demo/src";
pub(super) const LIB: &str = "crates/demo/src/lib.rs";
pub(super) const PASS: &str = "test passphrase, not a real one";

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

pub(super) struct Repo(tempfile::TempDir);

impl Repo {
    pub(super) fn path(&self) -> &Path {
        self.0.path()
    }
    pub(super) fn git(&self, args: &[&str]) -> String {
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
    pub(super) fn write(&self, file: &str, text: &str) {
        let p = self.path().join(file);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    pub(super) fn read(&self, file: &str) -> String {
        std::fs::read_to_string(self.path().join(file)).unwrap()
    }
    pub(super) fn edit(&self, from: &str, to: &str) {
        let t = self.read(LIB);
        assert!(t.contains(from), "fixture lacks {from:?}");
        self.write(LIB, &t.replacen(from, to, 1));
    }
    pub(super) fn commit(&self, msg: &str) {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", msg]);
    }

    /// The fixture: a crate whose `twice` calls `leaf`, a `Cargo.lock`, a
    /// commented `kovan_root.toml`, and the folder's `kovan.toml` as a full
    /// index would write it (built with the rust-analyzer-free refresh,
    /// then the one call `twice -> leaf` filled in by hand, since the call
    /// graph needs rust-analyzer).
    pub(super) fn new() -> Repo {
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

    pub(super) fn state(&self, qual: &str) -> Option<WebStamp> {
        let id = format!("{LIB}::{qual}");
        stamp_states(self.path())
            .unwrap()
            .into_iter()
            .find(|s| s.function == id)
    }

    /// Draft, sign and write a stamp of `qual`, then commit it.
    pub(super) fn stamp(&self, key: &UnlockedKey, qual: &str, comments: &str) -> WrittenEntry {
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
pub(super) fn clean() -> BTreeMap<String, String> {
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

pub(super) fn today() -> String {
    kovan_common::review::signed_at::date_of(&kovan_common::review::signed_at::now_local()).unwrap()
}

/// A key generated with the test passphrase into a temporary keystore and
/// registered in the fixture's root (the founder: first reviewer).
pub(super) fn founder_key(r: &Repo, store: &Path) -> UnlockedKey {
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
