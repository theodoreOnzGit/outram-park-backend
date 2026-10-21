//! The **relation** records of the kovan artifact schema: a relation
//! artifact's single `[relation]` table, and the `[[relation]]` anchors any
//! artifact carries (GitHub #35, #743), including `code:` targets.
//!
//! Moved here from `kovan::relation` on 2026-10-07 (GitHub #764; placement
//! decided on #743) so that code-review `review.md` artifacts, read in the
//! wasm web view too, use the same anchor type as literature notes.
//! `kovan::relation` re-exports every item, so existing callers and the
//! on-disk form are unchanged. Node ids are plain strings here (`kovan`'s
//! `NodeId` is an alias of `String`).

use serde::{Deserialize, Serialize};

/// What kind of relationship a `kovan::relation::UserRelation` records between two nodes
/// (the layer-1 prototype's `RelationKind`, ported verbatim).
///
/// Deliberately not exhaustive of every scientific-argument shape a user
/// might want — it is the fixed vocabulary the prototype dogfooded and
/// agreed on; widening it is a future decision, not something this module
/// pre-empts by adding a catch-all variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    /// A generic, otherwise-unclassified relationship.
    RelatedTo,
    /// The source's argument or data supports the target's.
    Supports,
    /// The source's argument or data contradicts the target's.
    Contradicts,
    /// The source was derived from the target (e.g. a fit derived from a
    /// digitised dataset).
    DerivedFrom,
    /// The source uses data owned by the target (e.g. a model that consumes
    /// a digitised graph's CSV payload).
    UsesDataFrom,
    /// The source validates the target against reality/experiment.
    Validates,
    /// The source was checked against the target as a verification
    /// reference (numerics/implementation correctness, not physical
    /// validity — see `VERIFICATION_AND_VALIDATION.md`'s verification vs.
    /// validation distinction).
    VerifiedAgainst,
    /// The source implements a method/model the target describes (an
    /// equation or a method; unchanged meaning).
    Implements,
    /// The source is a part of the target: a function's review links the
    /// architecture node or artifact it belongs to (maintainer, #764,
    /// 2026-10-07). Added 2026-10-07; additive, so every relation written
    /// before reads unchanged.
    PartOf,
}

impl RelationKind {
    /// Every variant, in the fixed order the layer-1 prototype declared
    /// them. The one canonical ordering a UI cycles or lists through (e.g.
    /// the PDF canvas connection picker, op-30um.3) — never re-derived
    /// per call site.
    pub const ALL: [RelationKind; 9] = [
        RelationKind::RelatedTo,
        RelationKind::Supports,
        RelationKind::Contradicts,
        RelationKind::DerivedFrom,
        RelationKind::UsesDataFrom,
        RelationKind::Validates,
        RelationKind::VerifiedAgainst,
        RelationKind::Implements,
        RelationKind::PartOf,
    ];

    /// A short, lower-case, human-readable label, e.g. `"supports"` — reads
    /// naturally inline ("this note supports that table").
    pub fn label(self) -> &'static str {
        match self {
            Self::RelatedTo => "related to",
            Self::Supports => "supports",
            Self::Contradicts => "contradicts",
            Self::DerivedFrom => "derived from",
            Self::UsesDataFrom => "uses data from",
            Self::Validates => "validates",
            Self::VerifiedAgainst => "verified against",
            Self::Implements => "implements",
            Self::PartOf => "part of",
        }
    }

    /// The next variant in [`Self::ALL`]'s fixed order, wrapping back to the
    /// first after the last — the whole implementation of a UI's "cycle
    /// kind" button, so no call site hand-rolls its own wraparound.
    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|k| *k == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}


impl RelationKind {
    /// The snake_case wire name, as written in a relation artifact's
    /// `[relation] kind` and shown in its heading.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RelatedTo => "related_to",
            Self::Supports => "supports",
            Self::Contradicts => "contradicts",
            Self::DerivedFrom => "derived_from",
            Self::UsesDataFrom => "uses_data_from",
            Self::Validates => "validates",
            Self::VerifiedAgainst => "verified_against",
            Self::Implements => "implements",
            Self::PartOf => "part_of",
        }
    }
}

/// The `[relation]` table of a relation artifact.
///
/// Both endpoints are explicit: a relation is its own artifact now, not a
/// record nested inside the thing it starts from, so nothing about it is
/// implied by where it is written. The id lives in `[kovan] id`, like every
/// other artifact's.
///
/// The same record is also an **anchor** (GH issue #743): a `[[relation]]`
/// table on a lesson, walk-step or any other artifact, naming the code it
/// explains or the literature it cites. An anchor omits `source` (it is the
/// artifact the table is in) and may carry `page`, `quote` and `commit`.
/// Every one of those is optional and skipped when absent, so a relation
/// written before #743 re-serialises byte for byte.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationRecord {
    /// The node this relation starts at. Empty in an anchor, where it is
    /// implicitly the artifact the `[[relation]]` table belongs to.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
    /// The node this relation points at: a graph id (`paper:`, `artifact:`,
    /// `collection:`), a typed `kovan::node_id::NodeId` string, or a
    /// `code:` target ([`CodeTarget`]).
    pub target: String,
    /// What kind of relationship this is.
    pub kind: RelationKind,
    /// The 1-based page of a literature target the anchor points at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    /// A short quotation from the target, as the reader would search for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote: Option<String>,
    /// The git commit a `code:` target was read at, so a later reader can
    /// see the code the claim was made about.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
}

impl RelationRecord {
    /// A record with only `source`, `target` and `kind`: the shape every
    /// relation artifact had before GH issue #743.
    pub fn new(source: impl Into<String>, target: impl Into<String>, kind: RelationKind) -> Self {
        Self {
            source: source.into(),
            target: target.into(),
            kind,
            page: None,
            quote: None,
            commit: None,
        }
    }

    /// The `code:` target, when the target is one.
    pub fn code_target(&self) -> Option<CodeTarget> {
        CodeTarget::parse(&self.target)
    }
}

/// An artifact's `relation` key, in the shape it was written
/// (`kovan::artifact::ArtifactToml::relation`).
///
/// Untagged, so each shape reads from and writes back to its own TOML form:
/// a single `[relation]` table, or an array of `[[relation]]` tables.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Relations {
    /// `[relation]`: the body of a `kind = "relation"` artifact.
    One(RelationRecord),
    /// `[[relation]]`: an artifact's anchors (GH issue #743).
    Many(Vec<RelationRecord>),
}

impl Relations {
    /// The records, in file order.
    pub fn records(&self) -> &[RelationRecord] {
        match self {
            Self::One(r) => std::slice::from_ref(r),
            Self::Many(v) => v,
        }
    }

    /// The records, mutably.
    pub fn records_mut(&mut self) -> &mut [RelationRecord] {
        match self {
            Self::One(r) => std::slice::from_mut(r),
            Self::Many(v) => v,
        }
    }

    /// The single record of a relation artifact; `None` for the array form.
    pub fn one(&self) -> Option<&RelationRecord> {
        match self {
            Self::One(r) => Some(r),
            Self::Many(_) => None,
        }
    }
}

/// The prefix of a code target.
pub const CODE_PREFIX: &str = "code:";

/// A `code:` relation target (GH issue #743), in the code-walk path form:
///
/// ```text
/// code:<path/to/file.rs>::<Type::name>[@L<line>]
/// code:crates/boon-lay/src/release.rs::FuelParticle::release_fraction
/// code:crates/kovan/src/artifact.rs::parse_document@L640
/// ```
///
/// `@L<line>` is optional and only disambiguates two items with the same
/// path in one file, such as `cfg` twins (GH issue #739). The path is
/// repo-relative. Kovan does not resolve the target yet; desktop kovan shows
/// it as a link card with no navigation, which arrives with web-kovan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeTarget {
    /// The repo-relative file path, e.g. `crates/kovan/src/artifact.rs`.
    pub file: String,
    /// The item path inside the file, e.g. `Artifact::csv_block`.
    pub item: String,
    /// The 1-based line that disambiguates same-named items, if given.
    pub line: Option<u32>,
}

impl CodeTarget {
    /// Read `code:<file>::<item>[@L<line>]`. `None` for anything else,
    /// including a `code:` string with an empty file or item, or a line
    /// suffix that is not `@L` followed by a positive number.
    pub fn parse(target: &str) -> Option<Self> {
        let rest = target.strip_prefix(CODE_PREFIX)?;
        let (file, item) = rest.split_once("::")?;
        let (item, line) = match item.rsplit_once("@L") {
            Some((item, n)) => (item, Some(n.parse::<u32>().ok().filter(|n| *n > 0)?)),
            None => (item, None),
        };
        let bad = |s: &str| s.is_empty() || s.chars().any(char::is_whitespace);
        if bad(file) || bad(item) {
            return None;
        }
        Some(Self {
            file: file.to_string(),
            item: item.to_string(),
            line,
        })
    }

    /// Whether `target` is in the `code:` namespace at all (well-formed or
    /// not), so a caller can keep it out of graph-node resolution.
    pub fn is_code(target: &str) -> bool {
        target.starts_with(CODE_PREFIX)
    }
}

impl std::fmt::Display for CodeTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{CODE_PREFIX}{}::{}", self.file, self.item)?;
        if let Some(l) = self.line {
            write!(f, "@L{l}")?;
        }
        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: load-old-data check for the `part_of` addition (#764,
    /// 2026-10-07). Relation tables in both shapes written before it, with
    /// every pre-existing kind, read back unchanged and re-serialise byte for
    /// byte; `part_of` reads and writes as its wire name; an unknown kind is
    /// still refused (the vocabulary stays closed).
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn part_of_is_additive_and_old_relations_load_unchanged() {
        #[derive(Debug, Serialize, Deserialize, PartialEq)]
        struct Holder {
            relation: Relations,
        }
        let old_kinds = [
            "related_to", "supports", "contradicts", "derived_from", "uses_data_from",
            "validates", "verified_against", "implements",
        ];
        for k in old_kinds {
            let one = format!("[relation]\nsource = \"artifact:a#n\"\ntarget = \"paper:b\"\nkind = \"{k}\"\n");
            let h: Holder = toml::from_str(&one).unwrap();
            assert_eq!(h.relation.records()[0].kind.as_str(), k);
            assert_eq!(toml::to_string(&h).unwrap(), one);
            let many = format!("[[relation]]\ntarget = \"code:crates/x/src/a.rs::f\"\nkind = \"{k}\"\npage = 3\n");
            let h: Holder = toml::from_str(&many).unwrap();
            assert_eq!(toml::to_string(&h).unwrap(), many);
        }
        let p = "[[relation]]\ntarget = \"artifact:arch#solver-loop\"\nkind = \"part_of\"\n";
        let h: Holder = toml::from_str(p).unwrap();
        assert_eq!(h.relation.records()[0].kind, RelationKind::PartOf);
        assert_eq!(toml::to_string(&h).unwrap(), p);
        assert!(toml::from_str::<Holder>("[relation]\ntarget = \"x\"\nkind = \"owns\"\n").is_err());
        assert_eq!(RelationKind::ALL.len(), 9);
        assert_eq!(RelationKind::Implements.next(), RelationKind::PartOf);
        assert_eq!(RelationKind::PartOf.next(), RelationKind::RelatedTo);
    }
}
