//! **Plain-English text for rung 5 (IV&V)** (GitHub #810): every reason a
//! review misses rung 5, and every registry warning, in words a reviewer can
//! act on, for desktop kovan, kovan-web and `kovan-cli review ivv` alike.
//!
//! ~~[`super::ivv::Rung5Miss::reason`] formatted the nested problems with
//! `{:?}` (e.g. `Unverified { date: .., problem: Unsigned }`)~~ **CORRECTED
//! 2026-10-10** (#810): those are now the sentences below, so no view shows
//! a Rust debug dump.
//!
//! Pure functions over the engine's types; no I/O, builds for wasm.

use super::ivv::{
    AttestationProblem, AuditRecordProblem, OrganisationProblem, RecordList, RecordProblem,
    RecordWarning, VvCaseProblem,
};
use super::signing::registry::SignerProblem;

/// A signature problem, in words.
pub fn signer_problem(p: &SignerProblem) -> String {
    match p {
        SignerProblem::Unsigned => "it is not signed".into(),
        SignerProblem::NotEligible(who) => {
            format!("{who} is not allowed to sign it")
        }
        SignerProblem::UnknownKey { key } => format!("no allowed signer has a key {key}"),
        SignerProblem::NotTrusted { key } => format!("key {key} is not trusted"),
        SignerProblem::Malformed(e) => format!("the signature cannot be read ({e})"),
        SignerProblem::BadSignature => "the signature does not verify".into(),
        SignerProblem::SignerRevoked { signer, date } => {
            format!("the signer {signer} was revoked on {date}")
        }
        SignerProblem::SignerRetired { signer, key } => {
            format!("{signer}'s key {key} was retired by then")
        }
        SignerProblem::SignerKeyRevoked { signer, key } => {
            format!("{signer}'s key {key} was revoked by then")
        }
    }
}

/// Why an organisation record does not count, in words.
pub fn organisation_problem(p: &OrganisationProblem) -> String {
    match p {
        OrganisationProblem::NotRecorded => "none is recorded on or before the review date".into(),
        OrganisationProblem::BadDate(d) => format!("the record's date {d:?} is not YYYY-MM-DD"),
        OrganisationProblem::Unverified { date, problem } => format!(
            "the record dated {date} has no valid maintainer signature: {}",
            signer_problem(problem)
        ),
    }
}

/// Why an audit record is not a GitHub issue URL, in words.
pub fn audit_record_problem(p: &AuditRecordProblem) -> String {
    match p {
        AuditRecordProblem::Missing => "there is none".into(),
        AuditRecordProblem::NotAnIssueUrl(u) => {
            format!("{u:?} is not https://github.com/<owner>/<repo>/issues/<number>")
        }
        AuditRecordProblem::BadOwner(o) => format!("{o:?} is not a GitHub user or organisation"),
        AuditRecordProblem::BadRepo(r) => format!("{r:?} is not a GitHub repository name"),
        AuditRecordProblem::BadNumber(n) => format!("{n:?} is not an issue number"),
    }
}

/// Why a separation attestation does not count, in words.
pub fn attestation_problem(p: &AttestationProblem) -> String {
    match p {
        AttestationProblem::NotFound { id } => {
            format!("the reviewer has no separation attestation {id:?}")
        }
        AttestationProblem::Ambiguous { id } => {
            format!("the reviewer has more than one separation attestation {id:?}")
        }
        AttestationProblem::BadDate(d) => format!("its date {d:?} is not YYYY-MM-DD"),
        AttestationProblem::Unverified(s) => format!(
            "it has no valid signature by the reviewer's own key: {}",
            signer_problem(s)
        ),
        AttestationProblem::DatedAfterReview { attested, reviewed } => {
            format!("it is dated {attested}, after the review ({reviewed})")
        }
        AttestationProblem::OtherOrganisations {
            organisation,
            developing_organisation,
        } => format!(
            "it names {organisation} and {developing_organisation}, not the organisations \
             in force at the review"
        ),
        AttestationProblem::AuditRecord(AuditRecordProblem::Missing) => {
            "it names no audit record (a GitHub issue URL)".into()
        }
        AttestationProblem::AuditRecord(a) => {
            format!("its audit record is malformed: {}", audit_record_problem(a))
        }
    }
}

/// Why a review is not a hand-written V&V case, in words.
pub fn vv_case_problem(p: VvCaseProblem) -> &'static str {
    match p {
        VvCaseProblem::NoQualifyingEvidence => {
            "its V&V evidence is not code-to-code, analytical or a convergence study"
        }
        VvCaseProblem::NotWrittenByHand => "the V&V case was not written and verified by hand",
        VvCaseProblem::TestsByAgent => "git shows an AI agent added a reaching test",
        VvCaseProblem::TestAuthorshipUnknown => {
            "git cannot tell who wrote the reaching tests (none reach it, or no commit facts)"
        }
    }
}

/// `vv_case_problem` of each, joined.
pub fn vv_case_problems(ps: &[VvCaseProblem]) -> String {
    ps.iter()
        .map(|p| vv_case_problem(*p))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Which list a registry record is in, in words.
pub fn record_list(l: &RecordList) -> String {
    match l {
        RecordList::DevelopingOrganisation => "developing organisation".into(),
        RecordList::ReviewerOrganisation { reviewer } => format!("{reviewer}'s organisation"),
        RecordList::SeparationAttestation { reviewer } => {
            format!("{reviewer}'s separation attestation")
        }
    }
}

/// A registry warning, in words: `"<list> record <n>: <problem>"` (`n`
/// 1-based, as a person counts entries in the file).
pub fn record_warning(w: &RecordWarning) -> String {
    let what = match &w.problem {
        RecordProblem::Organisation(p) => organisation_problem(p),
        RecordProblem::Attestation(p) => attestation_problem(p),
    };
    format!("{} record {}: {what}", record_list(&w.list), w.index + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: every variant of every problem type gives a non-empty
    /// sentence with no Rust debug structure (`{`, `::`, a variant name).
    ///
    /// Result (2026-10-10): passes.
    #[test]
    fn every_problem_reads_as_words() {
        let signers = [
            SignerProblem::Unsigned,
            SignerProblem::NotEligible("github:x".into()),
            SignerProblem::UnknownKey { key: "k9".into() },
            SignerProblem::NotTrusted { key: "k1".into() },
            SignerProblem::BadSignature,
            SignerProblem::SignerRevoked {
                signer: "github:x".into(),
                date: "2026-01-01".into(),
            },
            SignerProblem::SignerRetired {
                signer: "github:x".into(),
                key: "k1".into(),
            },
            SignerProblem::SignerKeyRevoked {
                signer: "github:x".into(),
                key: "k1".into(),
            },
        ];
        let mut all: Vec<String> = signers.iter().map(signer_problem).collect();
        all.push(organisation_problem(&OrganisationProblem::NotRecorded));
        all.push(organisation_problem(&OrganisationProblem::BadDate(
            "x".into(),
        )));
        all.push(organisation_problem(&OrganisationProblem::Unverified {
            date: "2026-10-10".into(),
            problem: SignerProblem::Unsigned,
        }));
        for a in [
            AuditRecordProblem::Missing,
            AuditRecordProblem::NotAnIssueUrl("u".into()),
            AuditRecordProblem::BadOwner("-".into()),
            AuditRecordProblem::BadRepo("..".into()),
            AuditRecordProblem::BadNumber("0".into()),
        ] {
            all.push(audit_record_problem(&a));
            all.push(attestation_problem(&AttestationProblem::AuditRecord(a)));
        }
        for p in [
            AttestationProblem::NotFound { id: "s".into() },
            AttestationProblem::Ambiguous { id: "s".into() },
            AttestationProblem::BadDate("x".into()),
            AttestationProblem::Unverified(SignerProblem::BadSignature),
            AttestationProblem::DatedAfterReview {
                attested: "2026-10-10".into(),
                reviewed: "2026-10-09".into(),
            },
            AttestationProblem::OtherOrganisations {
                organisation: "A".into(),
                developing_organisation: "B".into(),
            },
        ] {
            all.push(attestation_problem(&p));
        }
        for p in [
            VvCaseProblem::NoQualifyingEvidence,
            VvCaseProblem::NotWrittenByHand,
            VvCaseProblem::TestsByAgent,
            VvCaseProblem::TestAuthorshipUnknown,
        ] {
            all.push(vv_case_problem(p).into());
        }
        all.push(vv_case_problems(&[
            VvCaseProblem::NotWrittenByHand,
            VvCaseProblem::TestsByAgent,
        ]));
        let w = RecordWarning {
            list: RecordList::SeparationAttestation {
                reviewer: "github:v".into(),
            },
            index: 0,
            problem: RecordProblem::Attestation(AttestationProblem::Unverified(
                SignerProblem::Unsigned,
            )),
        };
        let text = record_warning(&w);
        assert_eq!(
            text,
            "github:v's separation attestation record 1: it has no valid signature by the \
             reviewer's own key: it is not signed"
        );
        all.push(text);
        all.push(record_list(&RecordList::DevelopingOrganisation));
        all.push(record_list(&RecordList::ReviewerOrganisation {
            reviewer: "github:v".into(),
        }));
        for s in &all {
            assert!(!s.trim().is_empty());
            assert!(!s.contains('{') && !s.contains("::"), "debug text: {s}");
            for v in [
                "Unsigned",
                "NotRecorded",
                "Unverified",
                "AuditRecord",
                "Problem",
            ] {
                assert!(!s.contains(v), "variant name in {s}");
            }
        }
    }
}
