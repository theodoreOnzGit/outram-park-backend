use super::*;
use crate::artifact::relation::RelationKind;
use crate::review::id::mint_fn_id;
use crate::review::types::{AuthorshipKind, FieldError, UrlPinError};

/// The id a test function at `path` was minted with.
pub(crate) fn fid(path: &str) -> String {
    mint_fn_id(path, &h('a'), SHA)
}

pub(crate) const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

pub(crate) fn h(c: char) -> String {
    format!("sha256:{}", c.to_string().repeat(64))
}

pub(crate) fn meta(id: &str, kind: &str, target: Option<&str>) -> EntryMeta {
    EntryMeta {
        id: id.into(),
        kind: kind.into(),
        origin: Some("human".into()),
        created: "2026-10-07T10:00:00+08:00".into(),
        modified: "2026-10-07T10:00:00+08:00".into(),
        target: target.map(str::to_string),
    }
}

pub(crate) fn review(function: &str, _file: &str, by: &str) -> ReviewEntry {
    ReviewEntry {
        kovan: meta(
            &format!("review-{}-{}", function.rsplit("::").next().unwrap_or(""), by.replace([':', '@', '.'], "-")),
            "review",
            Some(&fid(function)),
        ),
        review: ReviewBody {
            function: None,
            path: Some(function.into()),
            by: by.into(),
            rung: 3,
            date: "2026-10-07".into(),
            commit: SHA.into(),
            hash: h('a'),
            doc_hash: h('b'),
            cargo_lock: Some(h('c')),
            callees: BTreeMap::new(),
            checklist: BTreeMap::new(),
            no_concept: None,
            authorship: None,
            moved: vec![],
            signature: None,
        },
        relations: vec![],
    }
}

fn doc_of(entries: Vec<Entry>) -> String {
    let parsed: Vec<ParsedEntry> = entries
        .into_iter()
        .enumerate()
        .map(|(i, entry)| ParsedEntry {
            heading: format!("Entry {i}"),
            line: 0,
            entry,
            body: "## Comments\n\nLooks right.".into(),
        })
        .collect();
    render_review_md(&parsed).unwrap()
}

/// Methodology: a `review.md` holding one entry of every kind (review with
/// callees, checklist, authorship, a move record, a signature and an
/// `implements` + `part_of` relation; needs_fix; a code annotation with
/// Hypothesis selectors; the folder upstream confirmation; the
/// deleted-functions table; an architecture node with a commit-pinned
/// upstream; and an unrelated `note`) renders and parses back to the same
/// entries, bodies included, and re-renders byte for byte.
///
/// Result (2026-10-07): passes.
#[test]
fn every_entry_kind_round_trips() {
    let mut r = review("crates/t/src/steam.rs::flash", "crates/t/src/steam.rs", "github:theodoreOnzGit");
    r.review.callees.insert("crates/t/src/steam.rs::sat".into(), h('d'));
    r.review.checklist.insert("doc_matches_behaviour".into(), "yes".into());
    r.review.checklist.insert("vv_evidence".into(), "reference_code_to_code".into());
    r.review.rung = 4;
    r.review.authorship = Some(ChangeAuthorship {
        kind: AuthorshipKind::Mixed,
        sessions: vec!["https://claude.ai/code/session_1".into()],
    });
    r.review.moved.push(MoveRecord {
        from: "crates/t/src/old.rs::flash".into(),
        to: Some("crates/t/src/steam.rs::flash".into()),
        commit: SHA.into(),
    });
    r.review.signature = Some(Signature {
        key: "k1".into(),
        alg: "ed25519".into(),
        value: "AAAA".into(),
    });
    r.relations = vec![
        RelationRecord::new("", "artifact:iapws-if97#eq-7", RelationKind::Implements),
        RelationRecord::new("", "artifact:arch-steam#flash-loop", RelationKind::PartOf),
    ];
    let nf = NeedsFixEntry {
        kovan: meta("fix-1", "needs_fix", Some(&fid("crates/t/src/steam.rs::flash"))),
        needs_fix: NeedsFixBody {
            function: None,
            path: Some("crates/t/src/steam.rs::flash".into()),
            by: "github:theodoreOnzGit".into(),
            date: "2026-10-07".into(),
            commit: SHA.into(),
            hash: h('a'),
            note: "guard missing on p < p_triple".into(),
            status: FixStatus::Open,
            resolved_by: None,
            highlights: vec!["hl-1".into()],
        },
    };
    let ann = AnnotationEntry {
        kovan: meta("hl-1", "annotation", Some(&fid("crates/t/src/steam.rs::flash"))),
        annotation: AnnotationBody {
            function: None,
            path: Some("crates/t/src/steam.rs::flash".into()),
            by: "github:theodoreOnzGit".into(),
            commit: SHA.into(),
            selector: vec![
                Selector::TextQuoteSelector(crate::anchoring::selector::TextQuoteSelector {
                    exact: "p.max(0.0)".into(),
                    prefix: Some("let q = ".into()),
                    suffix: None,
                }),
                Selector::TextPositionSelector(crate::anchoring::selector::TextPositionSelector {
                    start: 40,
                    end: 50,
                }),
            ],
            needs_fix: Some("fix-1".into()),
        },
    };
    let up = UpstreamEntry {
        kovan: meta("upstream", "upstream", None),
        upstream: UpstreamTable {
            is_port: true,
            repository: Some("https://github.com/CoolProp/CoolProp".into()),
            commit: Some(SHA.into()),
            tag: Some("v6.4.1".into()),
            files: [(
                "steam.rs".to_string(),
                format!("https://github.com/CoolProp/CoolProp/blob/{SHA}/src/IF97.h"),
            )]
            .into(),
            routines: BTreeMap::new(),
            confirmed_by: "github:theodoreOnzGit".into(),
            date: "2026-10-07".into(),
        },
    };
    let del = DeletedFunctionsEntry {
        kovan: meta("deleted-functions", "deleted_functions", None),
        deleted: vec![DeletedFunction {
            function: "crates/t/src/steam.rs::old".into(),
            path: "crates/t/src/steam.rs::old".into(),
            deleted_commit: Some(SHA.into()),
            branch: Some("develop".into()),
            last_review_commit: SHA.into(),
            reviewers: vec!["github:theodoreOnzGit".into()],
        }],
    };
    let arch = ArchitectureEntry {
        kovan: meta("arch-flash-loop", "architecture", None),
        architecture: ArchitectureBody {
            by: "github:theodoreOnzGit".into(),
            date: "2026-10-07".into(),
            commit: SHA.into(),
            members: vec![fid("crates/t/src/steam.rs::flash")],
            member_paths: vec!["crates/t/src/steam.rs::flash".into()],
            upstream: Some(crate::call_graph::upstream::Upstream {
                style: crate::call_graph::upstream::HeaderStyle::KeyValue,
                line: 1,
                project: Some("CoolProp".into()),
                repository: Some("https://github.com/CoolProp/CoolProp".into()),
                version: None,
                commit: Some(SHA.into()),
                source: None,
                files: vec![],
                licence: None,
                url: Some(format!("https://github.com/CoolProp/CoolProp/blob/{SHA}/src/IF97.h")),
            }),
            upstream_tag: Some("v6.4.1".into()),
            pattern: Some("concept:numerics/newton-iteration".into()),
            signature: None,
        },
        relations: vec![],
    };
    let note = Entry::Other {
        kind: "note".into(),
        toml: "[kovan]\nid = \"n\"\nkind = \"note\"\ncreated = \"c\"\nmodified = \"m\"\n".into(),
    };
    let entries = vec![
        Entry::Review(r),
        Entry::NeedsFix(nf),
        Entry::Annotation(ann),
        Entry::Upstream(up),
        Entry::DeletedFunctions(del),
        Entry::Architecture(arch),
        note,
    ];
    let md = doc_of(entries.clone());
    let doc = parse_review_md(&md);
    assert!(doc.unreadable.is_empty(), "{:?}", doc.unreadable);
    let back: Vec<Entry> = doc.entries.iter().map(|e| e.entry.clone()).collect();
    assert_eq!(back, entries);
    assert!(doc.entries.iter().all(|e| e.body == "## Comments\n\nLooks right."));
    assert_eq!(render_review_md(&doc.entries).unwrap(), md);
    assert!(doc.upstream().unwrap().is_port);
    assert_eq!(doc.architectures().count(), 1);
}

/// Methodology: per-entry isolation (maintainer, 2026-10-07: a malformed
/// entry is no review). Four reviews, of which one has a bad hash, one an
/// unknown rung and one is not TOML; the good one still loads, each bad one
/// is unreadable with the function and reviewer it still names, and an
/// unpinned upstream link is refused with the typed error.
///
/// Result (2026-10-07): passes.
#[test]
fn malformed_entries_are_isolated() {
    let good = review("crates/t/src/a.rs::f", "crates/t/src/a.rs", "github:a");
    let mut bad_hash = review("crates/t/src/a.rs::g", "crates/t/src/a.rs", "github:a");
    bad_hash.review.hash = "sha256:nothex".into();
    let mut bad_rung = review("crates/t/src/a.rs::k", "crates/t/src/a.rs", "github:a");
    bad_rung.review.rung = 5;
    let mut md = doc_of(vec![
        Entry::Review(good.clone()),
        Entry::Review(bad_hash),
        Entry::Review(bad_rung),
    ]);
    md.push_str("\n# Broken\n\n```toml\n[kovan]\nid = \"x\"\nkind = \"review\"\n[review\nfunction = \"crates/t/src/a.rs::m\"\n```\n");
    let doc = parse_review_md(&md);
    assert_eq!(doc.reviews().cloned().collect::<Vec<_>>(), vec![good]);
    assert_eq!(doc.unreadable.len(), 3, "{:?}", doc.unreadable);
    assert_eq!(doc.unreadable[0].function.as_deref(), Some(fid("crates/t/src/a.rs::g").as_str()));
    assert_eq!(doc.unreadable[0].by.as_deref(), Some("github:a"));
    assert!(doc.unreadable[1].message.contains("rung 5"));
    assert_eq!(doc.unreadable[2].heading, "Broken", "a broken fence never vanishes");

    let mut up = UpstreamTable {
        is_port: true,
        repository: Some("https://github.com/o/r".into()),
        commit: Some(SHA.into()),
        tag: None,
        files: BTreeMap::new(),
        routines: BTreeMap::new(),
        confirmed_by: "github:a".into(),
        date: "2026-10-07".into(),
    };
    assert_eq!(validate_upstream(&up), Ok(()));
    up.files.insert("a.rs".into(), "https://github.com/o/r/blob/main/a.f90".into());
    assert!(matches!(
        validate_upstream(&up),
        Err(FieldError::UnpinnedUrl(UrlPinError::BranchRef { .. }))
    ));
    up.files.clear();
    up.commit = None;
    assert!(matches!(validate_upstream(&up), Err(FieldError::BadCommit { .. })));
}

/// Methodology: one standing review per reviewer per function. Two reviews
/// of `f` by the same reviewer make both unreadable (neither is chosen
/// silently); a review of `f` by a second reviewer is unaffected.
///
/// Result (2026-10-07): passes.
#[test]
fn a_reviewer_has_one_standing_review_per_function() {
    let a1 = review("crates/t/src/a.rs::f", "crates/t/src/a.rs", "github:a");
    let mut a2 = a1.clone();
    a2.kovan.id = "second".into();
    let b = review("crates/t/src/a.rs::f", "crates/t/src/a.rs", "gitlab:b");
    let doc = parse_review_md(&doc_of(vec![Entry::Review(a1), Entry::Review(a2), Entry::Review(b.clone())]));
    assert_eq!(doc.reviews().cloned().collect::<Vec<_>>(), vec![b]);
    assert_eq!(doc.unreadable.len(), 2);
}

/// Methodology: the signed bytes change when any certifying field changes
/// (hash, a callee hash, a checklist answer, the change authorship) and not
/// when the artifact id, timestamps or the comments change; verification is
/// ~~a stub that never answers "verified" (#762 not landed).~~
/// **CORRECTED 2026-10-07 (#762)**: verification is real; an unsigned entry
/// and a signature by a reviewer missing from the registry do not count
/// (the full rules are tested in `signing/tests.rs`).
///
/// Result (2026-10-07): passes.
#[test]
fn signed_bytes_cover_the_certifying_fields() {
    use crate::review::signing::registry::Registry;
    use crate::review::signing::{signed_bytes, verify_review, SignatureCheck, UnverifiedReason};
    let base = review("crates/t/src/a.rs::f", "crates/t/src/a.rs", "github:a");
    let b0 = signed_bytes(&base);
    let mut same = base.clone();
    same.kovan.id = "other".into();
    same.kovan.modified = "later".into();
    assert_eq!(signed_bytes(&same), b0);
    let mut edits: Vec<ReviewEntry> = Vec::new();
    let mut e = base.clone();
    e.review.hash = h('e');
    edits.push(e);
    let mut e = base.clone();
    e.review.callees.insert("x".into(), h('f'));
    edits.push(e);
    let mut e = base.clone();
    e.review.checklist.insert("vv_evidence".into(), "analytical_case".into());
    edits.push(e);
    let mut e = base.clone();
    e.review.authorship = Some(ChangeAuthorship {
        kind: AuthorshipKind::Agent,
        sessions: vec![],
    });
    edits.push(e);
    for e in &edits {
        assert_ne!(signed_bytes(e), b0);
    }
    let registry = Registry::build(&crate::review::root::ReviewRoot::default());
    assert_eq!(
        verify_review(&base, &registry),
        SignatureCheck::Unverified(UnverifiedReason::NoSignature)
    );
    let mut signed = base.clone();
    signed.review.signature = Some(Signature {
        key: "k".into(),
        alg: "ed25519".into(),
        value: "AAAA".into(),
    });
    assert_eq!(
        verify_review(&signed, &registry),
        SignatureCheck::Unverified(UnverifiedReason::UnknownReviewer("github:a".into()))
    );
}

/// Methodology: migration from the first-version ids (maintainer, #764,
/// 2026-10-07: "a migration test from the call-graph-key ids"). A
/// `review.md` written in the first-version form (`[review] function` =
/// call-graph key, `target = "code:…"`, a needs-fix and a highlight on the
/// same function, an architecture node listing it) reads with no
/// unreadable entries; every entry of one function gets the same `fn:` id,
/// minted from the key and its earliest review's hash and commit; the path
/// keeps the old key; re-rendering writes the new form, which reads back
/// unchanged with nothing left to migrate.
///
/// Result (2026-10-07): passes.
#[test]
fn first_version_ids_migrate() {
    let key = "crates/t/src/steam.rs::flash";
    let old = format!(r#"# Review

```toml
[kovan]
id = "r1"
kind = "review"
created = "c"
modified = "m"
target = "code:crates/t/src/steam.rs::flash"

[review]
function = "{key}"
by = "github:a"
rung = 3
date = "2026-10-07"
commit = "{SHA}"
hash = "{ha}"
doc_hash = "{hb}"

[[review.moved]]
from = "crates/t/src/old.rs::flash"
commit = "{SHA}"
```

# Needs fix

```toml
[kovan]
id = "n1"
kind = "needs_fix"
created = "c"
modified = "m"
target = "code:crates/t/src/steam.rs::flash"

[needs_fix]
function = "{key}"
by = "github:a"
date = "2026-10-07"
commit = "{SHA}"
hash = "{ha}"
note = "guard missing"
status = "open"
```

# Highlight

```toml
[kovan]
id = "h1"
kind = "annotation"
created = "c"
modified = "m"

[annotation]
function = "{key}"
by = "github:a"
commit = "{SHA}"
```

# Arch

```toml
[kovan]
id = "a1"
kind = "architecture"
created = "c"
modified = "m"

[architecture]
by = "github:a"
date = "2026-10-07"
commit = "{SHA}"
members = ["{key}"]
```
"#, ha = h('a'), hb = h('b'));
    let doc = parse_review_md(&old);
    assert!(doc.unreadable.is_empty(), "{:?}", doc.unreadable);
    let id = mint_fn_id(key, &h('a'), SHA);
    assert_eq!(doc.migrated, vec![(key.to_string(), id.clone())]);
    let r = doc.reviews().next().unwrap();
    assert_eq!(r.function_id(), id);
    assert_eq!(r.path().as_deref(), Some(key));
    assert_eq!(r.review.function, None);
    assert_eq!(r.review.moved[0].to, None, "a first-version move record keeps its shape");
    assert_eq!(doc.needs_fixes().next().unwrap().function_id(), id);
    assert_eq!(doc.architectures().next().unwrap().architecture.members, vec![id.clone()]);
    let new = render_review_md(&doc.entries).unwrap();
    assert!(new.contains(&format!("target = \"{id}\"")) && !new.contains("function ="));
    let again = parse_review_md(&new);
    assert!(again.migrated.is_empty());
    let entries = |d: &ReviewDocument| d.entries.iter().map(|e| e.entry.clone()).collect::<Vec<_>>();
    assert_eq!(entries(&again), entries(&doc));
}
