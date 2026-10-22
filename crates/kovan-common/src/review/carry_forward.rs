//! **Carry-forward records**: how a signed stamp survives a squash, merge,
//! rebase or cherry-pick (GitHub #838; design on #1018, maintainer
//! 2026-10-22: "keep commit. But if squashed, then the squash commit should
//! become the new key automatically" / "or merged for that matter...").
//!
//! # The problem
//!
//! A stamp signs the commit it certifies (`commit`) and the function's hash
//! there (`hash`). When the reviewed code reaches a branch by a squash merge,
//! a rebase or a cherry-pick, that commit is not in the branch's history
//! (and is garbage-collected once the source branch is deleted). The stamp
//! cannot be edited: `commit` is a signed field, and re-signing needs the
//! reviewer's key.
//!
//! # A record, never an edit
//!
//! The signed stamp is left exactly as signed. Beside it, an **unsigned,
//! derived** record says where the same code now is:
//!
//! ```toml
//! [[carry_forward]]
//! stamp = "review-twice-tester-0123abcd"   # the review.md entry's [kovan] id
//! target = "fn:0123abcd45678901"           # the stamp's function id
//! path = "crates/demo/src/lib.rs::twice"   # where the function is at `to`
//! hash = "sha256:…"                        # the stamp's signed hash
//! from = "<40 hex>"                        # the key it replaces
//! to = "<40 hex>"                          # the commit that brought the code
//! ```
//!
//! It needs no signature because **anyone can verify it by re-running one
//! check** ([`check_record`]): the record names the stamp's own `target` and
//! signed `hash`, and the function at `path` in commit `to` hashes to
//! exactly that `hash`. A record that fails is rejected, never repaired: a
//! forged one (a wrong hash, another function, code that changed) gives the
//! stamp nothing.
//!
//! # Chains
//!
//! A stamp's **key** starts as its signed `commit`. While the key is not in
//! `HEAD`'s history, the record whose `from` is the key and that verifies
//! moves the key to its `to` ([`resolve_chain`]). One record per hop: review
//! branch → `develop` → `main` is two records. The key reached is the
//! **effective key** for staleness and anchoring
//! ([`super::engine::CarriedForward`]); a stamp with no record is judged
//! exactly as before.
//!
//! # Where records live: one file per hop, at the kovan root
//!
//! `kovan_carry_forward/<to commit>.toml` beside `kovan_root.toml`
//! ([`CARRY_FORWARD_DIR`], [`file_name`]). Chosen over the two options on
//! #1018:
//!
//! - **Not in the `review.md` entry.** The entry is the human's signed
//!   record and its file must load and re-serialise byte for byte (#1018,
//!   "additive only"); a derived list inside it would make every squash
//!   rewrite every reviewed folder's `review.md`, and two branches carrying
//!   the same stamp to different commits would edit the same lines.
//! - **Not one file per branch.** Branch names are not stable across hops
//!   (a `develop` file arrives on `main` with the next squash), and two
//!   writers on one branch (CI after each merge, a person locally) append to
//!   the same file and collide.
//! - **One file per `to` commit** is written once, by whoever brings that
//!   commit in, and its content is fixed by the commit and the stamps it
//!   carries; a second run for the same commit adds to it ([`CarryForwardFile::merge`]),
//!   and different hops never touch the same file. The records of every
//!   folder sit in one place, so a reader sees a hop as one change.
//!
//! # What is pure here
//!
//! Parsing, writing and judging records, given what git says as closures.
//! Reading git (the hash at a commit, ancestry) is the caller's:
//! `kovan::stamping::carry_forward`. Builds for wasm.

use serde::{Deserialize, Serialize};

use super::review_md::ReviewEntry;

/// The folder of record files, at the kovan root (beside `kovan_root.toml`).
pub const CARRY_FORWARD_DIR: &str = "kovan_carry_forward";

/// The `format` line of a record file.
pub const CARRY_FORWARD_FORMAT: &str = "kovan-carry-forward-v1";

/// The file holding the records whose `to` is `to`.
pub fn file_name(to: &str) -> String {
    format!("{to}.toml")
}

/// One hop of one stamp (module doc). Unsigned and derived.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CarryForward {
    /// The `review.md` entry's `[kovan] id`.
    pub stamp: String,
    /// The stamp's function id (`fn:…`).
    pub target: String,
    /// Where the function is at `to` (`file.rs::Type::name`): where the
    /// check looks. Not trusted: the hash is what is verified.
    pub path: String,
    /// The stamp's signed `hash`.
    pub hash: String,
    /// The key this hop replaces: the signed `commit`, or the previous
    /// hop's `to`.
    pub from: String,
    /// The commit that brought the code onto the branch (a squash, merge,
    /// rebased or cherry-picked commit).
    pub to: String,
}

/// A record file (module doc).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CarryForwardFile {
    pub format: String,
    #[serde(default, rename = "carry_forward")]
    pub records: Vec<CarryForward>,
}

const HEADER: &str = "\
# Carry-forward records (GitHub #838, #1018): unsigned and derived.
# Each says that a signed stamp's function, at commit `to`, hashes exactly to
# the stamp's signed `hash`, so the stamp's key moves from `from` to `to`
# after a squash, merge, rebase or cherry-pick. Anyone can re-check them:
#     kovan-cli review carry-forward --check
# Written by `kovan-cli review carry-forward`; never edit by hand.
";

impl CarryForwardFile {
    /// An empty file.
    pub fn new() -> Self {
        Self {
            format: CARRY_FORWARD_FORMAT.into(),
            records: Vec::new(),
        }
    }

    /// Parse a record file. A file of another `format` is refused.
    pub fn parse(text: &str) -> Result<Self, String> {
        let f: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        if f.format != CARRY_FORWARD_FORMAT {
            return Err(format!(
                "format is {:?}, not {CARRY_FORWARD_FORMAT:?}",
                f.format
            ));
        }
        Ok(f)
    }

    /// The file as written: the header comment, then the records sorted.
    pub fn to_toml(&self) -> String {
        let mut sorted = self.clone();
        sorted.records.sort();
        sorted.records.dedup();
        let body = toml::to_string(&sorted).expect("records serialise");
        format!("{HEADER}{body}")
    }

    /// Add `records`, keeping every record already there; one record per
    /// (stamp, from, to), the newer replacing the older.
    pub fn merge(&mut self, records: impl IntoIterator<Item = CarryForward>) {
        for r in records {
            self.records
                .retain(|o| (&o.stamp, &o.from, &o.to) != (&r.stamp, &r.from, &r.to));
            self.records.push(r);
        }
        self.records.sort();
    }
}

impl Default for CarryForwardFile {
    fn default() -> Self {
        Self::new()
    }
}

/// Why a record does not carry its stamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    /// It names another function than the stamp's `target`.
    TargetMismatch { record: String, signed: String },
    /// Its `hash` is not the stamp's signed hash (a forged record).
    HashNotSigned { record: String, signed: String },
    /// The function at `path` in `to` does not hash to the signed hash (or
    /// is not there): the code changed, or the record is forged.
    ChangedAtTo { found: Option<String> },
}

impl Rejection {
    /// Plain-English text.
    pub fn describe(&self) -> String {
        match self {
            Self::TargetMismatch { record, signed } => {
                format!("names function {record}, but the stamp is of {signed}")
            }
            Self::HashNotSigned { record, signed } => {
                format!("its hash {record} is not the stamp's signed hash {signed}")
            }
            Self::ChangedAtTo { found: Some(h) } => {
                format!("the function at `to` hashes to {h}, not the signed hash")
            }
            Self::ChangedAtTo { found: None } => "the function is not at `path` in `to`".into(),
        }
    }
}

/// The verdict on one record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HopCheck {
    Verified,
    Rejected(Rejection),
    /// `to` is not in this clone (a shallow clone, or another branch's
    /// commit): neither verified nor rejected.
    NotCheckable,
}

/// Judge `record` against the stamp `entry` (module doc). `hash_at_to` is
/// the hash of the function at `record.path` in `record.to` (`None`: not
/// there); `to_in_clone` whether this clone holds `record.to`. The record's
/// own fields are checked against the signed ones first, so a forged hash
/// is rejected whatever git says.
pub fn check_record(
    record: &CarryForward,
    entry: &ReviewEntry,
    hash_at_to: Option<&str>,
    to_in_clone: bool,
) -> HopCheck {
    let b = &entry.review;
    if record.target != entry.function_id() {
        return HopCheck::Rejected(Rejection::TargetMismatch {
            record: record.target.clone(),
            signed: entry.function_id(),
        });
    }
    if record.hash != b.hash {
        return HopCheck::Rejected(Rejection::HashNotSigned {
            record: record.hash.clone(),
            signed: b.hash.clone(),
        });
    }
    if !to_in_clone {
        return HopCheck::NotCheckable;
    }
    if hash_at_to != Some(b.hash.as_str()) {
        return HopCheck::Rejected(Rejection::ChangedAtTo {
            found: hash_at_to.map(str::to_string),
        });
    }
    HopCheck::Verified
}

/// A stamp's chain, resolved (module doc).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Chain {
    /// The effective key: the signed `commit`, or the last verified hop's
    /// `to`.
    pub key: String,
    /// Where the function is at the key: the stamp's location, or the last
    /// hop's `path`.
    pub path: Option<String>,
    /// The hops taken, in order. The last one is always verified; one
    /// before it may be unchecked (its `to` not in this clone, see
    /// [`Chain::unchecked`]).
    pub hops: Vec<CarryForward>,
    /// Hops taken whose `to` is not in this clone (an intermediate branch
    /// deleted and garbage-collected): their own fields matched the signed
    /// ones, and a later hop verified the same signed hash at a commit this
    /// clone holds, so the content claim stands; only that hop's
    /// provenance is unchecked.
    pub unchecked: Vec<CarryForward>,
    /// Records of this stamp from a key on the chain that were rejected.
    pub rejected: Vec<(CarryForward, Rejection)>,
}

/// What [`resolve_chain`] asks git.
pub trait ChainGit {
    /// `commit` is `HEAD` or an ancestor of it.
    fn in_history(&mut self, commit: &str) -> bool;
    /// This clone holds `commit`.
    fn in_clone(&mut self, commit: &str) -> bool;
    /// The hash of the function at `path` (`file.rs::qual`) in `commit`.
    fn hash_at(&mut self, commit: &str, path: &str) -> Option<String>;
}

/// Walk `entry`'s chain through `records` (module doc): from the signed
/// commit, while the key is not in `HEAD`'s history, take a record from
/// the key and move on. A verified record whose `to` is in history is
/// preferred, then any verified one, then one whose `to` is not in this
/// clone (an intermediate commit since collected, [`Chain::unchecked`]);
/// a rejected record is never taken. Stops when the key is in history or
/// no record from it can be taken; trailing unchecked hops are then
/// dropped, so the chain always ends on a verified hop (or none). Every
/// record is used at most once, so a cycle ends.
pub fn resolve_chain(entry: &ReviewEntry, records: &[CarryForward], git: &mut impl ChainGit) -> Chain {
    let mut chain = Chain {
        key: entry.review.commit.clone(),
        path: entry.review.path.clone(),
        ..Chain::default()
    };
    let mut used = vec![false; records.len()];
    let mut verified: Vec<bool> = Vec::new();
    while !git.in_history(&chain.key) {
        // (rank, index): 2 verified and in history, 1 verified, 0 unchecked.
        let mut next: Option<(u8, usize)> = None;
        for (i, r) in records.iter().enumerate() {
            if used[i] || r.stamp != entry.kovan.id || r.from != chain.key {
                continue;
            }
            let in_clone = git.in_clone(&r.to);
            let hash = if in_clone { git.hash_at(&r.to, &r.path) } else { None };
            let rank = match check_record(r, entry, hash.as_deref(), in_clone) {
                HopCheck::Verified if git.in_history(&r.to) => 2,
                HopCheck::Verified => 1,
                HopCheck::NotCheckable => 0,
                HopCheck::Rejected(why) => {
                    used[i] = true;
                    chain.rejected.push((r.clone(), why));
                    continue;
                }
            };
            if next.is_none_or(|(best, _)| rank > best) {
                next = Some((rank, i));
            }
        }
        let Some((rank, i)) = next else { break };
        used[i] = true;
        chain.key = records[i].to.clone();
        chain.path = Some(records[i].path.clone());
        chain.hops.push(records[i].clone());
        verified.push(rank > 0);
    }
    while verified.last() == Some(&false) {
        verified.pop();
        chain.hops.pop();
    }
    if let Some(last) = chain.hops.last() {
        chain.key = last.to.clone();
        chain.path = Some(last.path.clone());
    } else {
        chain.key = entry.review.commit.clone();
        chain.path = entry.review.path.clone();
    }
    chain.unchecked = chain
        .hops
        .iter()
        .zip(&verified)
        .filter(|(_, v)| !**v)
        .map(|(h, _)| h.clone())
        .collect();
    chain
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;
    use crate::review::review_md::parse_review_md;

    const C0: &str = "cccccccccccccccccccccccccccccccccccccccc";
    const HA: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const HB: &str = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const HC: &str = "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

    const ENTRY: &str = "# Review: f (github:t)\n\n```toml\n[kovan]\nid = \"review-f-t-01\"\nkind = \"review\"\norigin = \"human\"\ncreated = \"c\"\nmodified = \"m\"\ntarget = \"fn:0123456789abcdef\"\n\n[review]\npath = \"crates/x/src/a.rs::f\"\nby = \"github:t\"\nrung = 3\ndate = \"2026-10-22\"\ncommit = \"cccccccccccccccccccccccccccccccccccccccc\"\nhash = \"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"\ndoc_hash = \"sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd\"\n```\n";

    fn entry() -> ReviewEntry {
        let d = parse_review_md(ENTRY);
        let e = d.reviews().next().cloned();
        e.unwrap_or_else(|| panic!("{:?}", d.unreadable))
    }

    fn rec(from: &str, to: &str, hash: &str) -> CarryForward {
        CarryForward {
            stamp: "review-f-t-01".into(),
            target: "fn:0123456789abcdef".into(),
            path: "crates/x/src/a.rs::f".into(),
            hash: hash.into(),
            from: from.into(),
            to: to.into(),
        }
    }

    /// Git as data: commits in history, commits in the clone, and the hash
    /// of `f` at each commit.
    struct FakeGit {
        history: BTreeSet<&'static str>,
        clone: BTreeSet<&'static str>,
        hashes: BTreeMap<&'static str, &'static str>,
    }

    impl ChainGit for FakeGit {
        fn in_history(&mut self, c: &str) -> bool {
            self.history.contains(c)
        }
        fn in_clone(&mut self, c: &str) -> bool {
            self.clone.contains(c) || self.history.contains(c)
        }
        fn hash_at(&mut self, c: &str, _: &str) -> Option<String> {
            self.hashes.get(c).map(|h| h.to_string())
        }
    }

    /// Methodology: a stamp signed at `c0` (gone), squashed into `s1` on
    /// develop, then `s1` squashed into `s2` on main. Judge chains with git
    /// as data (no repository).
    ///
    /// Pass: on main, two hops reach `s2`; on develop, one hop reaches
    /// `s1`; a stamp whose commit is in history takes no hop; a record with
    /// a forged hash, one naming another function, and one whose `to` holds
    /// changed code are each rejected with their reason and move nothing; a
    /// record whose `to` is not in the clone is neither, and a chain never
    /// ends on one; a collected intermediate commit is passed (unchecked)
    /// when the next hop verifies; a cycle of records ends.
    ///
    /// Result (2026-10-22): passes.
    #[test]
    fn chains_follow_verified_hops_only() {
        let e = entry();
        let records = vec![rec(C0, "s1", HA), rec("s1", "s2", HA)];
        let mut main = FakeGit {
            history: BTreeSet::from(["s2", "m0"]),
            clone: BTreeSet::from(["s1"]),
            hashes: BTreeMap::from([("s1", HA), ("s2", HA)]),
        };
        let c = resolve_chain(&e, &records, &mut main);
        assert_eq!(c.key, "s2");
        assert_eq!(c.hops.len(), 2);
        assert!(c.rejected.is_empty());

        let mut develop = FakeGit {
            history: BTreeSet::from(["s1"]),
            clone: BTreeSet::new(),
            hashes: BTreeMap::from([("s1", HA)]),
        };
        let c = resolve_chain(&e, &records, &mut develop);
        assert_eq!((c.key.as_str(), c.hops.len()), ("s1", 1));

        let mut before = FakeGit {
            history: BTreeSet::from([C0, "s1"]),
            clone: BTreeSet::new(),
            hashes: BTreeMap::new(),
        };
        let c = resolve_chain(&e, &records, &mut before);
        assert_eq!((c.key.as_str(), c.hops.len()), (C0, 0));

        // Forged: a wrong hash, another function, changed code at `to`.
        let mut other = rec(C0, "s2", HA);
        other.target = "fn:ffffffffffffffff".into();
        let forged = vec![rec(C0, "s2", HB), other, rec(C0, "x9", HA)];
        let mut g = FakeGit {
            history: BTreeSet::from(["s2", "x9"]),
            clone: BTreeSet::new(),
            hashes: BTreeMap::from([("s2", HA), ("x9", HC)]),
        };
        let c = resolve_chain(&e, &forged, &mut g);
        assert_eq!((c.key.as_str(), c.hops.len()), (C0, 0));
        let why: Vec<_> = c.rejected.iter().map(|(_, w)| w.clone()).collect();
        assert!(matches!(why[0], Rejection::HashNotSigned { .. }), "{why:?}");
        assert!(matches!(why[1], Rejection::TargetMismatch { .. }), "{why:?}");
        assert_eq!(why[2], Rejection::ChangedAtTo { found: Some(HC.into()) });
        assert!(why.iter().all(|w| !w.describe().is_empty()));

        // `to` not in this clone: not checkable, nothing moves.
        let mut shallow = FakeGit {
            history: BTreeSet::from(["s2"]),
            clone: BTreeSet::new(),
            hashes: BTreeMap::new(),
        };
        let r = rec(C0, "gone", HA);
        assert_eq!(check_record(&r, &e, None, false), HopCheck::NotCheckable);
        let c = resolve_chain(&e, &[r], &mut shallow);
        assert!(c.hops.is_empty() && c.rejected.is_empty());
        assert_eq!(c.key, C0);

        // An intermediate commit collected: c0 -> s1 (gone) -> s2 (here).
        let mut collected = FakeGit {
            history: BTreeSet::from(["s2"]),
            clone: BTreeSet::new(),
            hashes: BTreeMap::from([("s2", HA)]),
        };
        let c = resolve_chain(&e, &records, &mut collected);
        assert_eq!((c.key.as_str(), c.hops.len(), c.unchecked.len()), ("s2", 2, 1));
        assert_eq!(c.unchecked[0].to, "s1");

        // A cycle (c0 -> a -> c0) ends.
        let mut cyc = FakeGit {
            history: BTreeSet::new(),
            clone: BTreeSet::from(["a", C0]),
            hashes: BTreeMap::from([("a", HA), (C0, HA)]),
        };
        let c = resolve_chain(&e, &[rec(C0, "a", HA), rec("a", C0, HA)], &mut cyc);
        assert_eq!(c.hops.len(), 2);
    }

    /// Methodology: write a file with two records out of order, parse it
    /// back, merge a duplicate and a new record.
    ///
    /// Pass: the text starts with the header comment and parses to the
    /// sorted records; merging keeps one record per (stamp, from, to); a
    /// file of another format is refused.
    ///
    /// Result (2026-10-22): passes.
    #[test]
    fn files_round_trip_and_merge() {
        let mut f = CarryForwardFile::new();
        f.records = vec![rec("s1", "s2", HA), rec(C0, "s1", HA)];
        let text = f.to_toml();
        assert!(text.starts_with("# Carry-forward records"), "{text}");
        let back = CarryForwardFile::parse(&text).unwrap();
        assert_eq!(back.records[0].from, C0);
        assert_eq!(back.records.len(), 2);
        let mut m = back.clone();
        m.merge([rec(C0, "s1", HA), rec(C0, "s3", HA)]);
        assert_eq!(m.records.len(), 3);
        assert_eq!(file_name("abc"), "abc.toml");
        let e = CarryForwardFile::parse("format = \"other\"\n").unwrap_err();
        assert!(e.contains("format"), "{e}");
        assert_eq!(CarryForwardFile::default(), CarryForwardFile::new());
    }
}
