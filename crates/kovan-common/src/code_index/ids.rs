//! **Stable function ids** for `kovan.toml` (#764 Q1, #767): recovered from
//! `review.md`, minted otherwise.
//!
//! The id is the join key between a folder's `kovan.toml` and its
//! `review.md` (`[review] function`). `kovan.toml` is a disposable cache, so
//! the ids it holds must be **rebuildable from the source and `review.md`
//! alone**: an id that only lived in an old `kovan.toml` would be lost by a
//! regeneration. Hence the rule:
//!
//! 1. **Claims.** Every `review.md` entry that names a function (`review`,
//!    `needs_fix`, `annotation`, and an unreadable one that still names its
//!    function) is a [`Claim`]: the id it uses, the path its `[kovan]
//!    target` gives (`code:<file>::<item>[@L<line>]`), and the hash it was
//!    taken at. A `review.md` speaks only for **its own folder**: a claim
//!    whose target file is in another folder is ignored (a copied fixture
//!    must not capture the functions it names).
//! 2. **By path.** Each claimed id (in sorted order) takes the one function
//!    its path names today (`file::qual`, or `file::qual#k`; an `@L` line
//!    picks among `#k` twins).
//! 3. **By hash.** An id whose path names nothing takes the one unclaimed
//!    function, anywhere in the indexed scope, whose code `hash` equals the
//!    claim's: the function moved or was renamed without an edit. Several
//!    candidates is ambiguous and nothing is taken (reported, never
//!    guessed). Renamed **and** edited matches neither, and the function
//!    starts from new, as #739 D6 decided.
//! 4. **Minted.** Every other function gets [`mint_id`] of its path id:
//!    `fn:` and the first 16 hex digits of
//!    `sha256("kovan-fn-id-v1\n" + <file>::<qual>[#k])`. The same path gives
//!    the same id on every machine and every run. On a collision (with
//!    another minted id or a claimed one) the full 64 digits are used.
//!
//! A legacy claim whose id is itself a path id (`crates/x/src/a.rs::f`, the
//! #764 v1 form) is honoured verbatim: the id stays what `review.md` says.

use std::collections::{BTreeMap, BTreeSet};

use crate::artifact::relation::CodeTarget;
use crate::review::hash::sha256_tagged;
use crate::review::review_md::{Entry, ReviewDocument};

/// The prefix of every minted id.
pub const ID_PREFIX: &str = "fn:";

/// Hex digits of a minted id (64 bits: collisions are handled, not assumed
/// away).
pub const MINT_DIGITS: usize = 16;

fn digest(path_id: &str) -> String {
    let h = sha256_tagged(format!("kovan-fn-id-v1\n{path_id}").as_bytes());
    h.trim_start_matches(crate::review::types::HASH_PREFIX).to_string()
}

/// The minted id of a function whose call-graph path id is `path_id`.
pub fn mint_id(path_id: &str) -> String {
    format!("{ID_PREFIX}{}", &digest(path_id)[..MINT_DIGITS])
}

/// The long form used when [`mint_id`] collides.
pub fn mint_long_id(path_id: &str) -> String {
    format!("{ID_PREFIX}{}", digest(path_id))
}

/// One function named by a `review.md` entry.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Claim {
    /// The join key the entry uses.
    pub id: String,
    /// `file::item` from the target (or a legacy path id).
    pub path: Option<String>,
    /// The target's `@L` line, if any.
    pub line: Option<u32>,
    /// The code hash the entry was taken at.
    pub hash: Option<String>,
    /// The `review.md` it came from.
    pub source: String,
}

fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(d, _)| d)
}

fn looks_like_path_id(id: &str) -> bool {
    id.contains(".rs::")
}

/// The claims in one `review.md` (workspace-relative path `review_md`).
pub fn claims_from_review_md(review_md: &str, doc: &ReviewDocument) -> Vec<Claim> {
    let dir = parent(review_md);
    let mut out = Vec::new();
    let mut push = |id: &str, target: Option<&str>, hash: Option<&str>| {
        let parsed = target.and_then(CodeTarget::parse);
        let (path, line) = match &parsed {
            Some(t) => (Some(format!("{}::{}", t.file, t.item)), t.line),
            None if looks_like_path_id(id) => (Some(id.to_string()), None),
            None => (None, None),
        };
        if let Some(p) = &path {
            let file = p.split("::").next().unwrap_or("");
            if parent(file) != dir {
                return;
            }
        }
        out.push(Claim {
            id: id.to_string(),
            path,
            line,
            hash: hash.map(str::to_string),
            source: review_md.to_string(),
        });
    };
    for e in &doc.entries {
        match &e.entry {
            Entry::Review(r) => push(
                &r.review.function,
                r.kovan.target.as_deref(),
                Some(&r.review.hash),
            ),
            Entry::NeedsFix(n) => push(
                &n.needs_fix.function,
                n.kovan.target.as_deref(),
                Some(&n.needs_fix.hash),
            ),
            Entry::Annotation(a) => {
                push(&a.annotation.function, a.kovan.target.as_deref(), None)
            }
            _ => {}
        }
    }
    for u in &doc.unreadable {
        // `function` falls back to the target when the entry's own function
        // field was unreadable; a bare target is not an id.
        if let Some(f) = u.function.as_deref().filter(|f| !CodeTarget::is_code(f)) {
            push(f, None, None);
        }
    }
    out.sort();
    out.dedup();
    out
}

/// One function to give an id: its path id and where it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Located {
    /// The call graph's id, `file::qual[#k]`.
    pub path_id: String,
    pub lines: [u32; 2],
    pub hash: String,
}

/// A claimed id that moved to another function by hash.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Moved {
    pub id: String,
    /// Where `review.md` says it was.
    pub from: Option<String>,
    /// Where it is now.
    pub to: String,
}

/// A claimed id that found no function.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Unmatched {
    pub id: String,
    pub path: Option<String>,
    /// `deleted or renamed and edited`, or `ambiguous: k functions share the hash`.
    pub why: String,
}

/// The outcome of [`assign_ids`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Assignment {
    /// path id -> id, for every located function.
    pub ids: BTreeMap<String, String>,
    pub moved: Vec<Moved>,
    pub unmatched: Vec<Unmatched>,
}

fn strip_k(path_id: &str) -> &str {
    match path_id.rsplit_once('#') {
        Some((p, k)) if k.bytes().all(|b| b.is_ascii_digit()) && !k.is_empty() => p,
        _ => path_id,
    }
}

/// Give every located function an id (module doc).
pub fn assign_ids(fns: &[Located], claims: &[Claim]) -> Assignment {
    // id -> its claims (an id may be named by several entries).
    let mut by_id: BTreeMap<&str, Vec<&Claim>> = BTreeMap::new();
    for c in claims {
        by_id.entry(c.id.as_str()).or_default().push(c);
    }
    let mut taken: BTreeMap<usize, &str> = BTreeMap::new(); // fn index -> id
    let mut done: BTreeSet<&str> = BTreeSet::new();
    // 2. By path.
    for (id, cs) in &by_id {
        let mut hits: BTreeSet<usize> = BTreeSet::new();
        for c in cs {
            let Some(p) = c.path.as_deref() else { continue };
            let exact: Vec<usize> = (0..fns.len()).filter(|&i| fns[i].path_id == p).collect();
            let cands = if !exact.is_empty() {
                exact
            } else {
                let mut v: Vec<usize> = (0..fns.len())
                    .filter(|&i| strip_k(&fns[i].path_id) == strip_k(p))
                    .collect();
                if let Some(l) = c.line {
                    v.retain(|&i| fns[i].lines[0] <= l && l <= fns[i].lines[1]);
                }
                v
            };
            if cands.len() == 1 {
                hits.insert(cands[0]);
            }
        }
        if hits.len() == 1 {
            let i = *hits.iter().next().unwrap();
            if let std::collections::btree_map::Entry::Vacant(v) = taken.entry(i) {
                v.insert(id);
                done.insert(id);
            }
        }
    }
    // 3. By hash.
    let mut out = Assignment::default();
    for (id, cs) in &by_id {
        if done.contains(id) {
            continue;
        }
        let hashes: BTreeSet<&str> = cs.iter().filter_map(|c| c.hash.as_deref()).collect();
        let from = cs.iter().find_map(|c| c.path.clone());
        let cands: Vec<usize> = (0..fns.len())
            .filter(|i| !taken.contains_key(i) && hashes.contains(fns[*i].hash.as_str()))
            .collect();
        match cands.as_slice() {
            [i] => {
                taken.insert(*i, id);
                done.insert(id);
                out.moved.push(Moved {
                    id: id.to_string(),
                    from,
                    to: fns[*i].path_id.clone(),
                });
            }
            [] => out.unmatched.push(Unmatched {
                id: id.to_string(),
                path: from,
                why: "deleted, or renamed and edited".into(),
            }),
            many => out.unmatched.push(Unmatched {
                id: id.to_string(),
                path: from,
                why: format!("ambiguous: {} functions share its hash", many.len()),
            }),
        }
    }
    // 4. Minted.
    let mut used: BTreeSet<String> = by_id.keys().map(|s| s.to_string()).collect();
    let mut order: Vec<usize> = (0..fns.len()).collect();
    order.sort_by(|a, b| fns[*a].path_id.cmp(&fns[*b].path_id));
    for i in order {
        let id = match taken.get(&i) {
            Some(id) => id.to_string(),
            None => {
                let short = mint_id(&fns[i].path_id);
                let id = if used.contains(&short) {
                    mint_long_id(&fns[i].path_id)
                } else {
                    short
                };
                used.insert(id.clone());
                id
            }
        };
        out.ids.insert(fns[i].path_id.clone(), id);
    }
    out.moved.sort();
    out.unmatched.sort();
    out
}

/// The id of a function outside the indexed scope (a cross-crate callee of
/// a scoped run): the one claim whose path names it exactly, else minted.
/// A claim that would match it only by hash is not seen (documented limit
/// of a scoped run; a whole-workspace run has no outside functions).
pub fn id_outside(path_id: &str, claims: &[Claim]) -> String {
    let ids: BTreeSet<&str> = claims
        .iter()
        .filter(|c| c.path.as_deref() == Some(path_id))
        .map(|c| c.id.as_str())
        .collect();
    match ids.len() {
        1 => ids.into_iter().next().unwrap().to_string(),
        _ => mint_id(path_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::review_md::parse_review_md;

    fn h(c: char) -> String {
        format!("sha256:{}", c.to_string().repeat(64))
    }

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

    /// Methodology: the minting rule. Same path -> same id; `fn:` plus 16
    /// lowercase hex digits; different paths -> different ids; the long form
    /// extends the short one.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn minting_is_deterministic_and_opaque() {
        let a = mint_id("crates/x/src/a.rs::f");
        assert_eq!(a, mint_id("crates/x/src/a.rs::f"));
        assert_eq!(a.len(), 3 + MINT_DIGITS);
        assert!(a[3..].bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
        assert_ne!(a, mint_id("crates/x/src/a.rs::g"));
        assert!(mint_long_id("crates/x/src/a.rs::f").starts_with(&a));
    }

    /// Methodology: the four-step rule of the module doc on five functions.
    /// `fn:keep` names `a.rs::f` by path; `fn:moved` names a path that no
    /// longer exists but its hash matches `b.rs::g` (a move); `fn:gone`
    /// names nothing and matches no hash; `fn:twin` matches two hashes
    /// (ambiguous, so nothing is taken); `fn:k` picks one of two `#k`
    /// twins by its `@L` line. Every other function is minted; a legacy
    /// path id claim is kept verbatim.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn claims_are_matched_by_path_then_hash_and_the_rest_minted() {
        let fns = vec![
            loc("crates/x/src/a.rs::f", 1, 5, 'a'),
            loc("crates/x/src/b.rs::g", 1, 5, 'b'),
            loc("crates/x/src/b.rs::t1", 7, 9, 'c'),
            loc("crates/x/src/b.rs::t2", 10, 12, 'c'),
            loc("crates/x/src/c.rs::T::fmt#1", 1, 3, 'd'),
            loc("crates/x/src/c.rs::T::fmt#2", 5, 8, 'e'),
            loc("crates/x/src/d.rs::legacy", 1, 2, 'f'),
        ];
        let mut k = claim("fn:k", Some("crates/x/src/c.rs::T::fmt"), None);
        k.line = Some(6);
        let claims = vec![
            claim("fn:keep", Some("crates/x/src/a.rs::f"), Some('z')),
            claim("fn:moved", Some("crates/x/src/old.rs::g"), Some('b')),
            claim("fn:gone", Some("crates/x/src/old.rs::h"), Some('y')),
            claim("fn:twin", Some("crates/x/src/old.rs::t"), Some('c')),
            k,
            claim("crates/x/src/d.rs::legacy", Some("crates/x/src/d.rs::legacy"), None),
        ];
        let a = assign_ids(&fns, &claims);
        assert_eq!(a.ids["crates/x/src/a.rs::f"], "fn:keep");
        assert_eq!(a.ids["crates/x/src/b.rs::g"], "fn:moved");
        assert_eq!(a.ids["crates/x/src/c.rs::T::fmt#2"], "fn:k");
        assert_eq!(a.ids["crates/x/src/c.rs::T::fmt#1"], mint_id("crates/x/src/c.rs::T::fmt#1"));
        assert_eq!(a.ids["crates/x/src/b.rs::t1"], mint_id("crates/x/src/b.rs::t1"));
        assert_eq!(a.ids["crates/x/src/d.rs::legacy"], "crates/x/src/d.rs::legacy");
        assert_eq!(
            a.moved,
            vec![Moved {
                id: "fn:moved".into(),
                from: Some("crates/x/src/old.rs::g".into()),
                to: "crates/x/src/b.rs::g".into()
            }]
        );
        let why: Vec<(&str, &str)> = a.unmatched.iter().map(|u| (u.id.as_str(), u.why.as_str())).collect();
        assert_eq!(
            why,
            vec![
                ("fn:gone", "deleted, or renamed and edited"),
                ("fn:twin", "ambiguous: 2 functions share its hash")
            ]
        );
        // Order of the inputs does not matter.
        let mut rev = fns.clone();
        rev.reverse();
        let mut rc = claims.clone();
        rc.reverse();
        assert_eq!(assign_ids(&rev, &rc), a);
        // A path id outside the scope.
        assert_eq!(id_outside("crates/x/src/a.rs::f", &claims), "fn:keep");
        assert_eq!(id_outside("crates/y/src/z.rs::q", &claims), mint_id("crates/y/src/z.rs::q"));
    }

    /// Methodology: a minted id that collides with a claimed one takes the
    /// long form (the claim is forged here to equal the short mint).
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn a_collision_takes_the_long_form() {
        let p = "crates/x/src/a.rs::f";
        let fns = vec![loc(p, 1, 2, 'a')];
        let claims = vec![claim(&mint_id(p), Some("crates/x/src/gone.rs::q"), None)];
        let a = assign_ids(&fns, &claims);
        assert_eq!(a.ids[p], mint_long_id(p));
    }

    /// Methodology: claims read from a `review.md` with a review, a
    /// needs-fix, an annotation and a review whose target is in another
    /// folder (ignored: a review.md speaks only for its own folder).
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn claims_come_from_review_md_entries_of_their_own_folder() {
        let sha = "0123456789abcdef0123456789abcdef01234567";
        let review = |id: &str, target: &str, by: &str| {
            format!(
                "# Review {id} {by}\n\n```toml\n[kovan]\nid = \"review-{by}-{id}\"\nkind = \"review\"\ncreated = \"2026-10-07\"\nmodified = \"2026-10-07\"\ntarget = \"code:{target}\"\n\n[review]\nfunction = \"{id}\"\nby = \"github:{by}\"\nrung = 3\ndate = \"2026-10-07\"\ncommit = \"{sha}\"\nhash = \"{}\"\ndoc_hash = \"{}\"\n```\n\n",
                h('a'),
                h('b')
            )
        };
        let fix = format!(
            "# Fix\n\n```toml\n[kovan]\nid = \"fix-1\"\nkind = \"needs_fix\"\ncreated = \"2026-10-07\"\nmodified = \"2026-10-07\"\ntarget = \"code:crates/x/src/a.rs::g\"\n\n[needs_fix]\nfunction = \"fn:g\"\nby = \"github:bob\"\ndate = \"2026-10-07\"\ncommit = \"{sha}\"\nhash = \"{}\"\nnote = \"wrong sign\"\nstatus = \"open\"\n```\n\n",
            h('c')
        );
        let md = format!(
            "{}{}{fix}",
            review("fn:f", "crates/x/src/a.rs::f@L3", "alice"),
            review("fn:other", "crates/y/src/q.rs::q", "alice")
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
                ("fn:f", Some("crates/x/src/a.rs::f"), Some(3)),
                ("fn:g", Some("crates/x/src/a.rs::g"), None),
            ]
        );
    }
}
