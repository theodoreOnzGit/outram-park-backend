//! **Rung 5 (IV&V) for the views** (GitHub #810): what desktop kovan,
//! kovan-web and `kovan-cli review ivv` show about a function's independent
//! verification and validation, and what the need-you queue (#771) lists.
//!
//! ```text
//!  engine::FunctionReport.independent_vv ─┐
//!  FunctionFlag::IndependentVvNotCounted ─┼─[summarise]──> IvvSummary (serde: carried
//!  Evaluation::ivv_warnings ──────────────┘                 on call_graph::split::StampState)
//!  Evaluation ──────────────────────────────[ivv_queue]──> Vec<IvvQueueRow> (#771's queue)
//!  ReviewRoot + Registry + reviewer ─[attestation_choices]─> the stamp dialog's picker
//! ```
//!
//! Every reason is the engine's [`Rung5Miss::reason`], in words
//! ([`super::ivv_text`]). An audit record is a link shown with
//! [`AUDIT_RECORD_LABEL`]; kovan never fetches it (Leak Before Break: shown,
//! not verified).
//!
//! **What is queued.** Only the misses of a review that *claims* rung 5 (it
//! names a separation attestation: the engine's "independent V&V not
//! counted" flag), and every registry record that does not verify. The
//! misses of an ordinary review (every first review misses rung 5 as the
//! first reviewer) are shown on the function, never queued: they ask nothing
//! of anyone.
//!
//! Pure, no I/O; builds for wasm.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::engine::{Evaluation, FunctionFlag, FunctionReport, SignaturePolicy};
use super::ivv::{check_attestation, RecordList, RecordWarning, Rung5Miss, AUDIT_RECORD_LABEL};
use super::ivv_text::record_warning;
use super::root::ReviewRoot;
use super::signing::registry::Registry;
use super::state::Tone;

/// The review that gives rung 5, as shown.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IvvPassSummary {
    pub reviewer: String,
    /// The reviewer's organisation in force at the review.
    pub organisation: String,
    /// The developing organisation in force for the function's crate.
    pub developing_organisation: String,
    /// The separation attestation's id.
    pub attestation: String,
    /// Its audit record (a GitHub issue URL), shown with
    /// [`AUDIT_RECORD_LABEL`]; never fetched.
    pub audit_record: String,
}

/// One valid review judged for rung 5, as shown.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IvvCandidateSummary {
    pub reviewer: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub date: String,
    /// The `review.md` artifact id.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub review: String,
    /// The separation attestation the review names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attestation: Option<String>,
    /// That attestation's audit record as written in `kovan_root.toml`,
    /// even when malformed (shown, labelled, never fetched).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit_record: Option<String>,
    /// Why it does not give rung 5, in words; empty for the pass.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<String>,
}

/// A function's rung-5 judgement as every view shows it (GitHub #810).
/// Carried, additively, on [`crate::call_graph::split::StampState::ivv`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IvvSummary {
    /// The review that gives rung 5; `None`: not reached.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pass: Option<IvvPassSummary>,
    /// Every valid review, with its reasons.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<IvvCandidateSummary>,
    /// The flag "independent V&V not counted": a review names an
    /// attestation (claims rung 5) and misses; one line per such review.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub not_counted: Vec<String>,
    /// Registry records this function's IV&V could rest on that do not
    /// verify (developing organisation, its reviewers' organisations and
    /// attestations), in words.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl IvvSummary {
    /// Rung 5 is reached.
    pub fn passed(&self) -> bool {
        self.pass.is_some()
    }

    /// The one-line headline every view starts with.
    pub fn headline(&self) -> String {
        match &self.pass {
            Some(p) => format!(
                "IV&V (rung 5): passed, by {} ({}), separate from {}",
                p.reviewer, p.organisation, p.developing_organisation
            ),
            None if !self.not_counted.is_empty() => {
                "IV&V (rung 5): independent V&V not counted".into()
            }
            None => "IV&V (rung 5): not reached".into(),
        }
    }
}

/// One line of the section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IvvLine {
    pub tone: Tone,
    pub text: String,
    /// A link drawn after the text (an audit record).
    pub link: Option<String>,
}

fn line(tone: Tone, text: impl Into<String>) -> IvvLine {
    IvvLine {
        tone,
        text: text.into(),
        link: None,
    }
}

/// The headline's tone: good when passed, attention when a review claims
/// IV&V and misses, neutral otherwise (most functions are not meant to
/// reach rung 5).
pub fn headline_tone(s: &IvvSummary) -> Tone {
    if s.passed() {
        Tone::Good
    } else if !s.not_counted.is_empty() || !s.warnings.is_empty() {
        Tone::Attention
    } else {
        Tone::Neutral
    }
}

/// The section's detail lines, in order: the pass, the flag, each review
/// with its reasons and audit record, then the registry warnings.
pub fn ivv_lines(s: &IvvSummary) -> Vec<IvvLine> {
    let mut out = Vec::new();
    if let Some(p) = &s.pass {
        out.push(line(
            Tone::Good,
            format!(
                "passed: {} ({}) is separate from {}; attestation {}",
                p.reviewer, p.organisation, p.developing_organisation, p.attestation
            ),
        ));
        out.push(IvvLine {
            tone: Tone::Neutral,
            text: format!("{AUDIT_RECORD_LABEL}:"),
            link: Some(p.audit_record.clone()),
        });
    }
    for f in &s.not_counted {
        out.push(line(Tone::Attention, f.clone()));
    }
    for c in &s.candidates {
        let date = if c.date.is_empty() {
            String::new()
        } else {
            format!(" ({})", c.date)
        };
        let att = c
            .attestation
            .as_deref()
            .map_or(String::new(), |a| format!(", names attestation {a}"));
        if c.reasons.is_empty() {
            out.push(line(
                Tone::Good,
                format!("review by {}{date}{att}: gives rung 5", c.reviewer),
            ));
        } else {
            out.push(line(
                Tone::Neutral,
                format!("review by {}{date}{att}: not rung 5, because", c.reviewer),
            ));
            for r in &c.reasons {
                out.push(line(Tone::Neutral, format!("  - {r}")));
            }
        }
        if let Some(u) = &c.audit_record {
            out.push(IvvLine {
                tone: Tone::Neutral,
                text: format!("  {AUDIT_RECORD_LABEL}:"),
                link: Some(u.clone()),
            });
        }
    }
    for w in &s.warnings {
        out.push(line(
            Tone::Attention,
            format!("record does not verify: {w}"),
        ));
    }
    out
}

/// `"audit record (not verified by kovan): <url>"`.
pub fn shown_audit_record(url: &str) -> String {
    format!("{AUDIT_RECORD_LABEL}: {url}")
}

/// The warnings among `warnings` that concern a function reviewed by
/// `reviewers`: every developing-organisation record, and those reviewers'
/// organisation and attestation records.
pub fn relevant_warnings(warnings: &[RecordWarning], reviewers: &BTreeSet<String>) -> Vec<String> {
    warnings
        .iter()
        .filter(|w| match &w.list {
            RecordList::DevelopingOrganisation => true,
            RecordList::ReviewerOrganisation { reviewer }
            | RecordList::SeparationAttestation { reviewer } => reviewers.contains(reviewer),
        })
        .map(record_warning)
        .collect()
}

/// The audit record `reviewer`'s attestation `id` names in `root`, as
/// written (the first, if the id is ambiguous).
fn raw_audit_record(root: &ReviewRoot, reviewer: &str, id: &str) -> Option<String> {
    root.reviewer(reviewer)?
        .separations
        .iter()
        .find(|a| a.id == id)?
        .audit_record
        .clone()
}

/// The summary of one function (module doc); `None` when it has no valid
/// review, no flag and no relevant warning (nothing to judge).
pub fn summarise(
    fr: &FunctionReport,
    root: &ReviewRoot,
    warnings: &[RecordWarning],
) -> Option<IvvSummary> {
    let ivv = &fr.independent_vv;
    let reviewers: BTreeSet<String> = fr.reviews.iter().map(|r| r.by.clone()).collect();
    let not_counted: Vec<String> = fr
        .flags
        .iter()
        .filter_map(|f| match f {
            FunctionFlag::IndependentVvNotCounted { review, misses } => Some(format!(
                "independent V&V not counted: review {review} names a separation attestation \
                 and misses rung 5 ({} reason{})",
                misses.len(),
                if misses.len() == 1 { "" } else { "s" }
            )),
            _ => None,
        })
        .collect();
    if ivv.candidates.is_empty() && not_counted.is_empty() {
        return None;
    }
    let pass = ivv.passed.as_ref().map(|p| IvvPassSummary {
        reviewer: p.by.clone(),
        organisation: p.organisation.name.clone(),
        developing_organisation: p.developing_organisation.name.clone(),
        attestation: p.attestation.clone(),
        audit_record: p.audit_record.url.clone(),
    });
    let candidates = ivv
        .candidates
        .iter()
        .map(|c| IvvCandidateSummary {
            reviewer: c.by.clone(),
            date: c.date.clone().unwrap_or_default(),
            review: c.review.clone().unwrap_or_default(),
            attestation: c.attestation.clone(),
            audit_record: c
                .attestation
                .as_deref()
                .and_then(|id| raw_audit_record(root, &c.by, id)),
            reasons: c.misses.iter().map(Rung5Miss::reason).collect(),
        })
        .collect();
    Some(IvvSummary {
        pass,
        candidates,
        not_counted,
        warnings: relevant_warnings(warnings, &reviewers),
    })
}

/// One row of the need-you queue (#771) from rung 5: a miss of a review
/// that claims IV&V, or a registry record that does not verify.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct IvvQueueRow {
    /// The function's `fn:` id; `None` for a registry record.
    pub function: Option<String>,
    /// `file.rs::qual` where it is now, when known.
    pub path: Option<String>,
    /// The `review.md` artifact id of the review that claims rung 5.
    pub review: Option<String>,
    pub reviewer: Option<String>,
    /// What is wrong, in words.
    pub reason: String,
    /// Who can act and how.
    pub action: String,
}

/// The next step for a miss, in words: who can act, and where.
pub fn next_step(m: &Rung5Miss) -> &'static str {
    match m {
        Rung5Miss::FirstReviewer => {
            "IV&V is a later review by someone other than the first reviewer: ask another \
             organisation's reviewer"
        }
        Rung5Miss::CodeAuthor => "IV&V must come from a reviewer who did not write this code",
        Rung5Miss::NotIndependent => {
            "re-stamp answering independence = someone else, only if that is true"
        }
        Rung5Miss::NoHumanVvCase(_) => {
            "write and verify the V&V case by hand (no AI agent), commit it, and re-stamp"
        }
        Rung5Miss::NoConceptArea => {
            "record the function's concept area (an implements relation); kovan cannot check \
             qualification without one"
        }
        Rung5Miss::NotQualified { .. } => {
            "a maintainer records the reviewer's qualification in these areas"
        }
        Rung5Miss::ReviewerOrganisation(_) => {
            "a maintainer signs the reviewer's organisation (Code Review > Organisations & IV&V)"
        }
        Rung5Miss::DevelopingOrganisation(_) => {
            "a maintainer signs the developing organisation (Code Review > Organisations & IV&V)"
        }
        Rung5Miss::SameOrganisation { .. } => {
            "IV&V must come from an organisation separate from the developing one"
        }
        Rung5Miss::NoAttestation => {
            "the reviewer signs a separation attestation and names it when stamping"
        }
        Rung5Miss::Attestation(_) => {
            "the reviewer signs a new separation attestation with a GitHub issue audit record \
             (Organisations & IV&V) and re-stamps naming it"
        }
    }
}

/// The need-you queue's rung-5 rows (module doc): one per miss of every
/// review flagged "independent V&V not counted", then one per registry
/// warning. Sorted, so the queue is stable.
///
/// The desktop need-you queue (#771) shows the same misses as the detail
/// lines of its `IndependentVvNotCounted` row (`kovan::stamping::queue::
/// flag_detail`, wired at the merge 2026-10-10, with [`next_step`]);
/// `kovan-cli review ivv` prints these rows, registry warnings included,
/// under "Needs a person (rung 5)".
pub fn ivv_queue(ev: &Evaluation) -> Vec<IvvQueueRow> {
    let mut out = Vec::new();
    for fr in ev.functions.values() {
        for f in &fr.flags {
            let FunctionFlag::IndependentVvNotCounted { review, misses } = f else {
                continue;
            };
            let reviewer = fr
                .reviews
                .iter()
                .find(|r| r.artifact.as_deref() == Some(review.as_str()))
                .map(|r| r.by.clone());
            for m in misses {
                out.push(IvvQueueRow {
                    function: Some(fr.id.clone()),
                    path: fr
                        .location
                        .as_ref()
                        .map(|l| format!("{}::{}", l.file, l.qual)),
                    review: Some(review.clone()),
                    reviewer: reviewer.clone(),
                    reason: m.reason(),
                    action: next_step(m).into(),
                });
            }
        }
    }
    for w in &ev.ivv_warnings {
        let reviewer = match &w.list {
            RecordList::DevelopingOrganisation => None,
            RecordList::ReviewerOrganisation { reviewer }
            | RecordList::SeparationAttestation { reviewer } => Some(reviewer.clone()),
        };
        let action = match &w.list {
            RecordList::SeparationAttestation { .. } => {
                "the reviewer signs a new attestation (append-only: the old entry stays)"
            }
            _ => "a maintainer signs a new entry (append-only: the old entry stays)",
        };
        out.push(IvvQueueRow {
            function: None,
            path: None,
            review: None,
            reviewer,
            reason: record_warning(w),
            action: action.into(),
        });
    }
    out.sort();
    out
}

/// A separation attestation the stamp dialog may offer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttestationChoice {
    pub id: String,
    pub organisation: String,
    pub developing_organisation: String,
    pub date: String,
    pub audit_record: String,
}

impl AttestationChoice {
    /// `"sep-1: A, separate from B (2026-10-10)"`.
    pub fn label(&self) -> String {
        format!(
            "{}: {}, separate from {} ({})",
            self.id, self.organisation, self.developing_organisation, self.date
        )
    }
}

/// The attestations `reviewer` may name in a stamp: their own, signed,
/// verifying with their own trusted key, with a well-formed audit record
/// and a unique id. Whether it also fits the review (dates, organisations)
/// is the engine's judgement after the stamp.
pub fn attestation_choices(
    root: &ReviewRoot,
    registry: &Registry,
    reviewer: &str,
) -> Vec<AttestationChoice> {
    let Some(r) = root.reviewer(reviewer) else {
        return Vec::new();
    };
    r.separations
        .iter()
        .filter_map(|a| {
            let c = check_attestation(root, registry, reviewer, &a.id, SignaturePolicy::Enforce);
            let audit = c.audit_record?;
            c.problems.is_empty().then(|| AttestationChoice {
                id: a.id.clone(),
                organisation: a.organisation.clone(),
                developing_organisation: a.developing_organisation.clone(),
                date: a.date.clone(),
                audit_record: audit.url,
            })
        })
        .collect()
}

/// Serde for `Option<Arc<IvvSummary>>` (serde's `rc` feature is off in
/// the workspace): the same JSON as `Option<IvvSummary>`. The summary is
/// shared through an `Arc` so [`crate::call_graph::split::StampState`]
/// stays small (it sits in kovan-web's `Review` enum).
pub mod serde_opt_arc {
    use std::sync::Arc;

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::IvvSummary;

    pub fn serialize<S: Serializer>(v: &Option<Arc<IvvSummary>>, s: S) -> Result<S::Ok, S::Error> {
        v.as_deref().serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Option<Arc<IvvSummary>>, D::Error> {
        Ok(Option::<IvvSummary>::deserialize(d)?.map(Arc::new))
    }
}

// Tests: `engine_ivv_tests.rs` (they reuse its signed registry fixture).
