use super::*;
use crate::review::review_md::parse_review_md;

fn h(c: char) -> String {
    format!("sha256:{}", c.to_string().repeat(64))
}

const C: &str = "0123456789abcdef0123456789abcdef01234567";

fn loc(p: &str, a: u32, b: u32, c: char) -> Located {
    Located {
        path_id: p.into(),
        lines: [a, b],
        hash: h(c),
    }
}

fn claim(id: &str, path: Option<&str>, hash: Option<char>) -> Claim {
    Claim {
        id: id.into(),
        path: path.map(str::to_string),
        line: None,
        hash: hash.map(h),
        source: "crates/x/src/review.md".into(),
    }
}

/// Methodology: the five-step rule of the module doc. `fn:keep` names
/// `a.rs::f` by path; `fn:moved` names a path that no longer exists but its
/// hash matches `b.rs::g` (a move); `fn:gone` matches nothing; `fn:twin`
/// matches two hashes (ambiguous: nothing taken); `fn:k` picks one of two
/// `#k` twins by its `@L` line; a prior id is kept by path (`e.rs::p`) and
/// by hash (`e.rs::renamed`); every other function is minted with
/// `mint_fn_id(path, hash, commit)`. The result does not depend on input
/// order.
///
/// Result (2026-10-07): passes.
#[test]
fn ids_come_from_claims_then_priors_then_minting() {
    let fns = vec![
        loc("crates/x/src/a.rs::f", 1, 5, 'a'),
        loc("crates/x/src/b.rs::g", 1, 5, 'b'),
        loc("crates/x/src/b.rs::t1", 7, 9, 'c'),
        loc("crates/x/src/b.rs::t2", 10, 12, 'c'),
        loc("crates/x/src/c.rs::T::fmt#1", 1, 3, 'd'),
        loc("crates/x/src/c.rs::T::fmt#2", 5, 8, 'e'),
        loc("crates/x/src/e.rs::p", 1, 2, 'f'),
        loc("crates/x/src/e.rs::renamed", 3, 4, '9'),
    ];
    let mut k = claim("fn:k", Some("crates/x/src/c.rs::T::fmt"), None);
    k.line = Some(6);
    let claims = vec![
        claim("fn:keep", Some("crates/x/src/a.rs::f"), Some('z')),
        claim("fn:moved", Some("crates/x/src/old.rs::g"), Some('b')),
        claim("fn:gone", Some("crates/x/src/old.rs::h"), Some('y')),
        claim("fn:twin", Some("crates/x/src/old.rs::t"), Some('c')),
        k,
    ];
    let priors = vec![
        Prior {
            id: "fn:prior_p".into(),
            path_id: "crates/x/src/e.rs::p".into(),
            hash: h('0'),
        },
        Prior {
            id: "fn:prior_r".into(),
            path_id: "crates/x/src/e.rs::old_name".into(),
            hash: h('9'),
        },
        // A prior that a claim already uses elsewhere is not reused.
        Prior {
            id: "fn:keep".into(),
            path_id: "crates/x/src/b.rs::t1".into(),
            hash: h('c'),
        },
    ];
    let a = assign_ids(&fns, &claims, &priors, C);
    assert_eq!(a.ids["crates/x/src/a.rs::f"], "fn:keep");
    assert_eq!(a.ids["crates/x/src/b.rs::g"], "fn:moved");
    assert_eq!(a.ids["crates/x/src/c.rs::T::fmt#2"], "fn:k");
    assert_eq!(a.ids["crates/x/src/e.rs::p"], "fn:prior_p");
    assert_eq!(a.ids["crates/x/src/e.rs::renamed"], "fn:prior_r");
    assert_eq!(
        a.ids["crates/x/src/c.rs::T::fmt#1"],
        mint_fn_id("crates/x/src/c.rs::T::fmt#1", &h('d'), C)
    );
    assert_eq!(
        a.ids["crates/x/src/b.rs::t1"],
        mint_fn_id("crates/x/src/b.rs::t1", &h('c'), C)
    );
    assert!(is_fn_id(&a.ids["crates/x/src/b.rs::t1"]));
    assert_eq!(
        a.moved,
        vec![Moved {
            id: "fn:moved".into(),
            from: Some("crates/x/src/old.rs::g".into()),
            to: "crates/x/src/b.rs::g".into()
        }]
    );
    let why: Vec<(&str, &str)> = a
        .unmatched
        .iter()
        .map(|u| (u.id.as_str(), u.why.as_str()))
        .collect();
    assert_eq!(
        why,
        vec![
            ("fn:gone", "deleted, or renamed and edited"),
            ("fn:twin", "ambiguous: 2 functions share its hash")
        ]
    );
    let (mut rf, mut rc, mut rp) = (fns.clone(), claims.clone(), priors.clone());
    rf.reverse();
    rc.reverse();
    rp.reverse();
    assert_eq!(assign_ids(&rf, &rc, &rp, C), a);
    assert_eq!(id_outside("crates/x/src/a.rs::f", &claims, &priors).as_deref(), Some("fn:keep"));
    assert_eq!(id_outside("crates/x/src/e.rs::p", &claims, &priors).as_deref(), Some("fn:prior_p"));
    assert_eq!(id_outside("crates/y/src/z.rs::q", &claims, &priors), None);
}

fn review_entry(id: &str, path: &str, by: &str, callee: Option<(&str, char)>) -> String {
    let callees = callee
        .map(|(c, x)| format!("\n[review.callees]\n\"{c}\" = \"{}\"\n", h(x)))
        .unwrap_or_default();
    format!(
        "# Review {by}\n\n```toml\n[kovan]\nid = \"review-{by}\"\nkind = \"review\"\ncreated = \"2026-10-07\"\nmodified = \"2026-10-07\"\ntarget = \"{id}\"\n\n[review]\npath = \"{path}\"\nby = \"github:{by}\"\nrung = 3\ndate = \"2026-10-07\"\ncommit = \"{C}\"\nhash = \"{}\"\ndoc_hash = \"{}\"\n{callees}```\n\n",
        h('a'),
        h('b')
    )
}

/// Methodology: claims read from a `review.md` holding a review (with one
/// callee), a needs-fix, and a review whose path is in another folder,
/// which is ignored with its callee (a `review.md` speaks only for its own
/// folder).
///
/// Result (2026-10-07): passes.
#[test]
fn claims_come_from_review_md_entries_of_their_own_folder() {
    let fix = format!(
        "# Fix\n\n```toml\n[kovan]\nid = \"fix-1\"\nkind = \"needs_fix\"\ncreated = \"2026-10-07\"\nmodified = \"2026-10-07\"\ntarget = \"fn:00000000000000aa\"\n\n[needs_fix]\npath = \"crates/x/src/a.rs::g\"\nby = \"github:bob\"\ndate = \"2026-10-07\"\ncommit = \"{C}\"\nhash = \"{}\"\nnote = \"wrong sign\"\nstatus = \"open\"\n```\n\n",
        h('c')
    );
    let md = format!(
        "{}{}{fix}",
        review_entry(
            "fn:00000000000000ff",
            "crates/x/src/a.rs::f@L3",
            "alice",
            Some(("fn:00000000000000cc", 'd'))
        ),
        review_entry(
            "fn:00000000000000ee",
            "crates/y/src/q.rs::q",
            "carol",
            Some(("fn:00000000000000dd", 'd'))
        )
    );
    let doc = parse_review_md(&md);
    assert!(doc.unreadable.is_empty(), "{:?}", doc.unreadable);
    let claims = claims_from_review_md("crates/x/src/review.md", &doc);
    let got: Vec<(&str, Option<&str>, Option<u32>)> = claims
        .iter()
        .map(|c| (c.id.as_str(), c.path.as_deref(), c.line))
        .collect();
    assert_eq!(
        got,
        vec![
            ("fn:00000000000000aa", Some("crates/x/src/a.rs::g"), None),
            ("fn:00000000000000cc", None, None),
            ("fn:00000000000000ff", Some("crates/x/src/a.rs::f"), Some(3)),
        ]
    );
    assert_eq!(claims[1].hash.as_deref(), Some(h('d').as_str()));
}
