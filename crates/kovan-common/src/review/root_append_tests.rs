//! Tests of the `kovan_root.toml` text appenders: every existing byte
//! (comments included) is kept, the result reads back as exactly one more
//! entry, and the founder registered this way is trusted on first use by
//! the #762 registry.

use super::*;
use crate::review::root::{KeyEvent, KeyEventKind};
use crate::review::signing::registry::Registry;

/// A literature root with comments, an unknown key, a rust-analyzer
/// history entry and two reviewers, the first with a sub-table after its
/// key (`[reviewer.revoked]` would be odd on a maintainer; a plain
/// sub-table keeps the test about layout).
const ROOT: &str = r#"# The workspace root. Comments must survive.
schema_version = 1
unknown_key = "kept"

[library]
id = "demo"

[code_review]
rust_analyzer = "1.0.0" # pinned

[[code_review.rust_analyzer_used]]
version = "1.0.0"
date = "2026-10-07"
commit = "none"

[[reviewer]]
id = "github:first"
role = "maintainer"

[[reviewer.key]]
id = "k1"
alg = "ed25519"
public = "AAAA"
created = "2026-10-07"

# The second reviewer, admitted later.
[[reviewer]]
id = "github:second"
role = "reviewer"
scope = ["crates/x/**"]

# Trailing comment.
"#;

fn key(id: &str) -> ReviewerKey {
    ReviewerKey {
        id: id.into(),
        alg: "ed25519".into(),
        public: "BBBB".into(),
        created: "2026-10-10".into(),
        endorsed_by: None,
        reset: false,
        retired: false,
        retired_on: None,
        unretired: None,
        history: vec![KeyEvent::unsigned(KeyEventKind::Created, "2026-10-10")],
    }
}

/// Methodology: add a key to the FIRST reviewer (not the last, so the
/// splice must land inside the file), then append a new reviewer. Pass:
/// every original line is still present in order (comments included),
/// the key lands on the right reviewer, and refusals are typed (duplicate
/// key, unknown reviewer, duplicate reviewer).
///
/// Result (2026-10-10): passes.
#[test]
fn key_and_reviewer_appends_keep_comments() {
    let t = append_reviewer_key(ROOT, "github:first", &key("k2")).unwrap();
    let r = ReviewRoot::parse(&t).unwrap();
    let ids: Vec<&str> = r.reviewers[0].keys.iter().map(|k| k.id.as_str()).collect();
    assert_eq!(ids, ["k1", "k2"]);
    assert!(r.reviewers[1].keys.is_empty());
    let mut rest = t.as_str();
    for l in ROOT.lines().filter(|l| !l.is_empty()) {
        let at = rest
            .find(l)
            .unwrap_or_else(|| panic!("lost or reordered {l:?} in\n{t}"));
        rest = &rest[at + l.len()..];
    }
    let comment_at = t.find("# The second reviewer").unwrap();
    assert!(
        t.find("id = \"k2\"").unwrap() < comment_at,
        "the key goes before the next reviewer's lead-in comment"
    );

    let new = unadmitted_reviewer("github:third", Some("Third"), key("t1"));
    let t2 = append_reviewer(&t, &new).unwrap();
    assert!(t2.starts_with(&t));
    let r2 = ReviewRoot::parse(&t2).unwrap();
    assert_eq!(r2.reviewers.len(), 3);
    assert_eq!(r2.reviewers[2], new);
    assert!(is_bare_key(&r2.reviewers[2].keys[0]));

    assert!(matches!(
        append_reviewer_key(ROOT, "github:first", &key("k1")),
        Err(AppendError::KeyExists { .. })
    ));
    assert!(matches!(
        append_reviewer_key(ROOT, "github:nobody", &key("k9")),
        Err(AppendError::UnknownReviewer(_))
    ));
    assert!(matches!(
        append_reviewer(&t2, &new),
        Err(AppendError::ReviewerExists(_))
    ));
    assert!(matches!(
        append_reviewer("not = [toml", &new),
        Err(AppendError::Root(_))
    ));
}

/// Methodology: declare a founder in a root with a `[code_review]` table
/// (inserted under it) and in an empty file (a new table), register the
/// founding reviewer, and build the #762 registry. Pass: the founder is
/// admitted and its first key trusted on first use; a second declaration
/// is refused.
///
/// Result (2026-10-10): passes.
#[test]
fn founder_declared_once_and_trusted_on_first_use() {
    let t = declare_founder(ROOT, "github:first").unwrap();
    assert!(
        t.contains("rust_analyzer = \"1.0.0\" # pinned\nfounder = \"github:first\"\n")
            || t.contains("[code_review]\nfounder = \"github:first\"\n"),
        "{t}"
    );
    assert!(
        matches!(declare_founder(&t, "github:x"), Err(AppendError::FounderAlreadyDeclared(f)) if f == "github:first")
    );

    let id = "github:founder";
    let t = declare_founder("", id).unwrap();
    let mut k = key("k1");
    let public = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]).verifying_key();
    k.public = crate::review::signing::encode_b64(public.as_bytes());
    let t = append_reviewer(&t, &founding_reviewer(id, None, k, "2026-10-10")).unwrap();
    let root = ReviewRoot::parse(&t).unwrap();
    let reg = Registry::build(&root);
    let rv = reg.reviewer(id).expect("registered");
    assert!(rv.is_admitted(), "{t}");
    assert!(rv.key("k1").unwrap().status.is_trusted());
    assert!(AppendError::FounderAlreadyDeclared("x".into())
        .to_string()
        .contains("founder"));
}
