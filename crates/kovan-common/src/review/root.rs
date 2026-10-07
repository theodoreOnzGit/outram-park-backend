//! The code-review sections of the workspace's **`kovan_root.toml`**
//! (maintainer, #739 decision 18 and the signing / many-maintainers
//! comments, 2026-10-07).
//!
//! `kovan_root.toml` is the literature library marker that `kovan::root`
//! already reads (`schema_version`, `[library]`, `[paths]`, …). Code review
//! adds three **top-level, optional** sections; nothing existing changes, and
//! a literature library without them reads as before:
//!
//! ```toml
//! [code_review]
//! rust_analyzer = "0.3.2645"       # pinned version; a mismatch re-indexes
//!
//! [[reviewer]]
//! id = "github:theodoreOnzGit"     # github:/gitlab:/orcid: or an email
//! name = "Theodore Ong"            # display only
//! role = "maintainer"              # maintainer | reviewer
//! scope = ["crates/**"]            # path globs; see `scope`
//! admitted = "2026-10-07"
//!
//! [[reviewer.qualification]]       # one per area; enforced only at rung 5
//! area = "concept:thermal-hydraulics"
//! basis = "degree"                  # degree | publications | track_record | self_study | endorsement
//! evidence = ["https://doi.org/…"]
//! endorsed_by = { by = "github:…" } # optional
//! self_declared = false             # must be true for self_study
//! # (the first-version form, qualification = ["concept:…"], still reads)
//!
//! [[reviewer.key]]
//! id = "k1"
//! alg = "ed25519"
//! public = "<base64>"
//! created = "2026-10-07"
//! endorsed_by = { key = "k0", signature = "<base64>" }   # absent for the founding key
//!
//! [reviewer.revoked]                # optional
//! date = "2026-12-01"
//! compromised_from = "2026-11-20"  # optional: void stamps from this date
//! by = "github:theodoreOnzGit"
//!
//! [[deleted_crate]]                 # a crate renamed or deleted (#739 D6)
//! name = "old-crate"
//! dir = "crates/old-crate"
//! deleted_commit = "<sha>"
//! [[deleted_crate.function]]        # its review history, as review.md keeps it
//! function = "crates/old-crate/src/lib.rs::f"
//! …
//! ```
//!
//! ~~Signatures and endorsements are stored but **not verified**: the crypto is
//! GitHub #762.~~ **CORRECTED 2026-10-07 (#762)**: signatures, endorsements,
//! admissions, revocations and un-retirements are verified by
//! [`crate::review::signing::registry::Registry::build`]; what each one signs
//! is in [`crate::review::signing`].
//!
//! `kovan::root::RootConfig` carries the same three fields (additive), so a
//! literature-side save of `kovan_root.toml` keeps them.

use serde::{Deserialize, Serialize};

use super::review_md::DeletedFunction;
use super::types::{check_commit, reviewer_id_kind, FieldError};

/// `[code_review]`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeReviewSettings {
    /// The pinned rust-analyzer version (D4): a mismatch warns and
    /// regenerates the index.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rust_analyzer: Option<String>,
}

/// A reviewer's role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Admits and revokes reviewers, and stamps anywhere.
    Maintainer,
    /// Stamps only, within `scope`.
    Reviewer,
}

/// A signature by one key over some bytes (an endorsement, an admission or a
/// revocation). Verified by [`crate::review::signing::registry`] (#762).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeySignature {
    /// The signing key's id.
    pub key: String,
    /// Base64 signature.
    pub signature: String,
}

/// `[[reviewer.key]]`: one public key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewerKey {
    pub id: String,
    /// `ed25519` (#739 signing comment).
    pub alg: String,
    /// Base64 public key.
    pub public: String,
    pub created: String,
    /// The existing key of the same reviewer that endorsed this one; absent
    /// only for a reviewer's first key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endorsed_by: Option<KeySignature>,
    /// A deliberate key reset (shown permanently).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reset: bool,
    /// Retired (an un-retire clears it).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub retired: bool,
    /// The date the key was retired (#762, additive). Stamps the key signs
    /// on or after it do not count; kept after an un-retire, so the gap
    /// stays visible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retired_on: Option<String>,
    /// The un-retirement (#762, additive): signed by **this key itself**,
    /// which proves the old private key was unlocked (#739 signing comment).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unretired: Option<Unretirement>,
}

/// `[reviewer.key.unretired]`: a retired key brought back (#762).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unretirement {
    /// From this date the key's stamps count again.
    pub date: String,
    /// Base64 signature by the un-retired key over
    /// [`crate::review::signing::unretire_bytes`].
    pub signature: String,
}

/// `[reviewer.revoked]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revocation {
    /// Stamps dated before this stay valid.
    pub date: String,
    /// For a compromised key: stamps from this date on are void.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compromised_from: Option<String>,
    /// The maintainer who revoked.
    pub by: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<KeySignature>,
}

/// What a qualification rests on (maintainer, #739, 2026-10-07: "demonstrated
/// competence in an area, with evidence. A degree title is not required").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualificationBasis {
    /// A degree or thesis.
    Degree,
    Publications,
    /// A repository track record.
    TrackRecord,
    /// Self-study with evidence: always labelled self-declared.
    SelfStudy,
    /// Another person vouches (see `endorsed_by`).
    Endorsement,
}

impl QualificationBasis {
    pub fn label(self) -> &'static str {
        match self {
            Self::Degree => "degree/thesis",
            Self::Publications => "publications",
            Self::TrackRecord => "track record",
            Self::SelfStudy => "self-study",
            Self::Endorsement => "endorsement",
        }
    }
}

/// Who endorsed a qualification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualificationEndorsement {
    /// The endorser's reviewer id.
    pub by: String,
    /// Not verified until #762.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<KeySignature>,
}

/// `[[reviewer.qualification]]`: competence in one concept-tree area.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualificationRecord {
    /// The concept-tree node, e.g. `concept:thermal-hydraulics`. Covers the
    /// node and everything under it.
    pub area: String,
    pub basis: QualificationBasis,
    /// Links kovan can resolve (theses, papers, repositories).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endorsed_by: Option<QualificationEndorsement>,
    /// Must be `true` for a self-study basis (the label is shown on the
    /// stamp and in the registry).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub self_declared: bool,
}

/// One qualification entry, in either form (additive: the first-version
/// bare string still reads).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Qualification {
    Record(QualificationRecord),
    /// First-version form: a concept path with no basis or evidence. Shown,
    /// but never counts for rung 5.
    Cited(String),
}

/// The part of a concept id that names the tree node (`concept:` dropped).
fn concept_path(c: &str) -> &str {
    c.strip_prefix("concept:").unwrap_or(c).trim_matches('/')
}

/// Whether area `area` covers concept `concept`: the same node or one under
/// it.
pub fn area_covers(area: &str, concept: &str) -> bool {
    let (a, c) = (concept_path(area), concept_path(concept));
    !a.is_empty() && (c == a || c.starts_with(&format!("{a}/")))
}

impl Qualification {
    /// The area, in either form.
    pub fn area(&self) -> &str {
        match self {
            Self::Record(r) => &r.area,
            Self::Cited(s) => s,
        }
    }

    /// Whether it counts for rung 5 in `concept`: a record with evidence
    /// (or an endorsement) whose area covers the concept.
    pub fn qualifies_for(&self, concept: &str) -> bool {
        match self {
            Self::Record(r) => {
                (!r.evidence.is_empty() || r.endorsed_by.is_some()) && area_covers(&r.area, concept)
            }
            Self::Cited(_) => false,
        }
    }

    /// The public label, e.g. `thermal-hydraulics (degree/thesis)` or
    /// `numerics (self-study; self-declared)`.
    pub fn label(&self) -> String {
        match self {
            Self::Record(r) => {
                let mut s = format!("{} ({}", concept_path(&r.area), r.basis.label());
                if r.self_declared {
                    s.push_str("; self-declared");
                }
                if let Some(e) = &r.endorsed_by {
                    s.push_str(&format!("; endorsed by {}", e.by));
                }
                s.push(')');
                s
            }
            Self::Cited(c) => format!("{} (cited, no evidence)", concept_path(c)),
        }
    }
}

/// `[[reviewer]]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reviewer {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub role: Role,
    /// Path globs a `reviewer` may stamp in ([`super::scope`]). Ignored for
    /// a maintainer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope: Vec<String>,
    /// Demonstrated competence, one per area ([`Qualification`]). Either the
    /// first-version form, a bare concept path
    /// (`qualification = ["concept:…"]`), or since 2026-10-07 one
    /// `[[reviewer.qualification]]` table per area with basis and evidence.
    /// Shown beside every stamp; enforced only at rung 5.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub qualification: Vec<Qualification>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admitted: Option<String>,
    /// The maintainer signature that admitted this reviewer; absent for the
    /// founding maintainer (trusted on first use).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admitted_by: Option<KeySignature>,
    #[serde(default, rename = "key", skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<ReviewerKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked: Option<Revocation>,
}

/// `[[deleted_crate]]`: a crate renamed or deleted, with its review history
/// (#739 D6). Rebuildable from git; kept for copies without git.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeletedCrate {
    pub name: String,
    pub dir: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_commit: Option<String>,
    /// For a rename, the new crate name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renamed_to: Option<String>,
    #[serde(default, rename = "function", skip_serializing_if = "Vec::is_empty")]
    pub functions: Vec<DeletedFunction>,
}

/// The code-review sections of `kovan_root.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRoot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_review: Option<CodeReviewSettings>,
    #[serde(default, rename = "reviewer", skip_serializing_if = "Vec::is_empty")]
    pub reviewers: Vec<Reviewer>,
    #[serde(default, rename = "deleted_crate", skip_serializing_if = "Vec::is_empty")]
    pub deleted_crates: Vec<DeletedCrate>,
}

/// Why `kovan_root.toml`'s review sections cannot be read or written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RootError {
    /// Not TOML, or a review section does not match the schema.
    Toml(String),
    /// A reviewer id is not acceptable.
    Field(FieldError),
    /// Two `[[reviewer]]` entries share an id.
    DuplicateReviewer(String),
    /// A self-study qualification not labelled `self_declared = true`, or
    /// one with no evidence (maintainer, #739, 2026-10-07).
    UnlabelledSelfDeclared { reviewer: String, area: String },
    SelfStudyWithoutEvidence { reviewer: String, area: String },
}

impl std::fmt::Display for RootError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Toml(e) => write!(f, "kovan_root.toml: {e}"),
            Self::Field(e) => write!(f, "kovan_root.toml: {e}"),
            Self::DuplicateReviewer(id) => write!(f, "kovan_root.toml: reviewer {id} listed twice"),
            Self::UnlabelledSelfDeclared { reviewer, area } => write!(
                f,
                "kovan_root.toml: {reviewer}'s self-study qualification in {area} must set self_declared = true"
            ),
            Self::SelfStudyWithoutEvidence { reviewer, area } => write!(
                f,
                "kovan_root.toml: {reviewer}'s self-study qualification in {area} needs evidence"
            ),
        }
    }
}

impl std::error::Error for RootError {}

/// The keys this module owns in `kovan_root.toml`.
pub const ROOT_KEYS: [&str; 3] = ["code_review", "reviewer", "deleted_crate"];

impl ReviewRoot {
    /// Read the review sections from a whole `kovan_root.toml`; every other
    /// key (the literature library's) is ignored.
    pub fn parse(text: &str) -> Result<ReviewRoot, RootError> {
        let root: ReviewRoot = toml::from_str(text).map_err(|e| RootError::Toml(e.to_string()))?;
        let mut seen = std::collections::BTreeSet::new();
        for r in &root.reviewers {
            reviewer_id_kind(&r.id).map_err(RootError::Field)?;
            if !seen.insert(r.id.as_str()) {
                return Err(RootError::DuplicateReviewer(r.id.clone()));
            }
            // Dates are ISO strings (Q9, #764, 2026-10-07).
            let dates = r
                .admitted
                .iter()
                .map(|d| ("reviewer.admitted", d))
                .chain(r.keys.iter().map(|k| ("reviewer.key.created", &k.created)))
                .chain(r.revoked.iter().map(|v| ("reviewer.revoked.date", &v.date)))
                .chain(
                    r.revoked
                        .iter()
                        .filter_map(|v| v.compromised_from.as_ref())
                        .map(|d| ("reviewer.revoked.compromised_from", d)),
                );
            for (field, d) in dates {
                super::types::check_date(field, d).map_err(RootError::Field)?;
            }
            for q in &r.qualification {
                if let Qualification::Record(q) = q {
                    if q.basis == QualificationBasis::SelfStudy {
                        let who = || (r.id.clone(), q.area.clone());
                        if !q.self_declared {
                            let (reviewer, area) = who();
                            return Err(RootError::UnlabelledSelfDeclared { reviewer, area });
                        }
                        if q.evidence.is_empty() {
                            let (reviewer, area) = who();
                            return Err(RootError::SelfStudyWithoutEvidence { reviewer, area });
                        }
                    }
                }
            }
        }
        for c in &root.deleted_crates {
            if let Some(sha) = &c.deleted_commit {
                check_commit("deleted_crate.deleted_commit", sha).map_err(RootError::Field)?;
            }
        }
        Ok(root)
    }

    /// The reviewer registered under `id`.
    pub fn reviewer(&self, id: &str) -> Option<&Reviewer> {
        self.reviewers.iter().find(|r| r.id == id)
    }

    /// `existing` (a whole `kovan_root.toml`) with this module's sections
    /// replaced by `self`'s and every other key kept. Comments are not kept
    /// (the `toml` crate drops them, as `kovan::root` does on save).
    pub fn write_into(&self, existing: &str) -> Result<String, RootError> {
        let mut table: toml::Table =
            toml::from_str(existing).map_err(|e| RootError::Toml(e.to_string()))?;
        for k in ROOT_KEYS {
            table.remove(k);
        }
        let ours = toml::Table::try_from(self).map_err(|e| RootError::Toml(e.to_string()))?;
        table.extend(ours);
        toml::to_string_pretty(&table).map_err(|e| RootError::Toml(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT: &str = r#"schema_version = 1

[library]
id = "outram-park-backend"
name = "OUTRAM PARK backend"

[code_review]
rust_analyzer = "0.3.2645"

[[reviewer]]
id = "github:theodoreOnzGit"
name = "Theodore Ong"
role = "maintainer"
qualification = ["concept:sqa/safety-software-qa-competencies"]

[[reviewer.key]]
id = "k1"
alg = "ed25519"
public = "AAAA"
created = "2026-10-07"

[[reviewer]]
id = "someone@example.org"
role = "reviewer"
scope = ["crates/tampines/**"]
admitted_by = { key = "k1", signature = "BBBB" }

[reviewer.revoked]
date = "2026-12-01"
by = "github:theodoreOnzGit"

[[deleted_crate]]
name = "old"
dir = "crates/old"
deleted_commit = "0123456789abcdef0123456789abcdef01234567"
"#;

    /// Methodology: a literature `kovan_root.toml` with every code-review
    /// section reads, round-trips through [`ReviewRoot::write_into`] with the
    /// literature keys kept, and a literature root with none of them reads
    /// as empty (additive schema).
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn root_sections_round_trip_and_leave_the_library_alone() {
        let r = ReviewRoot::parse(ROOT).unwrap();
        assert_eq!(r.reviewers.len(), 2);
        assert_eq!(r.reviewers[0].role, Role::Maintainer);
        assert_eq!(r.reviewers[1].revoked.as_ref().unwrap().date, "2026-12-01");
        let written = r.write_into(ROOT).unwrap();
        assert_eq!(ReviewRoot::parse(&written).unwrap(), r);
        assert!(written.contains("[library]") && written.contains("outram-park-backend"));
        let plain = "schema_version = 1\n[library]\nid = \"x\"\nname = \"X\"\n";
        assert_eq!(ReviewRoot::parse(plain).unwrap(), ReviewRoot::default());
    }

    /// Methodology: qualification (maintainer, #739, 2026-10-07). The
    /// first-version bare-string form and the per-area table form both read
    /// (additive); a self-study record must be labelled self-declared and
    /// carry evidence (typed errors otherwise); an area covers its node and
    /// everything under it; only a record with evidence or an endorsement
    /// counts for rung 5; labels say "self-declared".
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn qualification_records_and_self_declared_labels() {
        let text = r#"
[[reviewer]]
id = "github:a"
role = "reviewer"
[[reviewer.qualification]]
area = "concept:thermal-hydraulics"
basis = "degree"
evidence = ["https://doi.org/10.1/thesis"]
[[reviewer.qualification]]
area = "concept:numerics"
basis = "self_study"
evidence = ["https://github.com/a/notes"]
self_declared = true
"#;
        let r = ReviewRoot::parse(text).unwrap();
        let q = &r.reviewers[0].qualification;
        assert!(q[0].qualifies_for("concept:thermal-hydraulics/natural-circulation"));
        assert!(!q[0].qualifies_for("concept:thermal-hydraulics-x"));
        assert!(!q[0].qualifies_for("concept:neutronics"));
        assert_eq!(q[1].label(), "numerics (self-study; self-declared)");
        assert_eq!(ReviewRoot::parse(&r.write_into(text).unwrap()).unwrap(), r);
        let unlabelled = text.replace("self_declared = true\n", "");
        assert!(matches!(
            ReviewRoot::parse(&unlabelled),
            Err(RootError::UnlabelledSelfDeclared { .. })
        ));
        let no_ev = text.replace("evidence = [\"https://github.com/a/notes\"]\n", "");
        assert!(matches!(
            ReviewRoot::parse(&no_ev),
            Err(RootError::SelfStudyWithoutEvidence { .. })
        ));
        let old = ReviewRoot::parse(ROOT).unwrap();
        let cited = &old.reviewers[0].qualification[0];
        assert!(matches!(cited, Qualification::Cited(_)));
        assert!(!cited.qualifies_for("concept:sqa/safety-software-qa-competencies"));
    }

    /// Methodology: a bad reviewer id and a duplicate reviewer are typed
    /// errors.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn bad_reviewers_are_refused() {
        let bad = "[[reviewer]]\nid = \"Theodore\"\nrole = \"maintainer\"\n";
        assert!(matches!(ReviewRoot::parse(bad), Err(RootError::Field(_))));
        let dup = "[[reviewer]]\nid = \"a@b.org\"\nrole = \"reviewer\"\n[[reviewer]]\nid = \"a@b.org\"\nrole = \"reviewer\"\n";
        assert!(matches!(ReviewRoot::parse(dup), Err(RootError::DuplicateReviewer(_))));
    }
}
