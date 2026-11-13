//! **Founders & lineage in Code Review ▸ User management** (GitHub #1248,
//! NON-FORK scope: several founders, retired founders, the maintainer vote,
//! the unsigned last resort shown forever, and the lineage fingerprint).
//! No egui; the drawing is `app/user_management_view.rs`'s "Founders &
//! lineage" section.
//!
//! **Nothing here invents a trust rule.** Every standing and ballot result
//! is read from the registry (`kovan_common::review::signing::founders`);
//! every action is a signature (`UnlockedKey::{sign_founder_change,
//! sign_founder_ballot, sign_founder_vote}`) appended as text by
//! `kovan_common::review::root_append`, which reads it back. The unsigned
//! last resort has no button: it is a reviewed repository commit, by
//! definition outside kovan.
//!
//! ```text
//!  Registry::lineage ──> LineageView: fingerprint, founders (since, how), retired founders
//!                        (until, by whom), ballots (open / elected / void / no result)
//!  Add founder / Retire founder ── a founder signs [[code_review.founder_change]]
//!  Open ballot  ── a maintainer signs [[code_review.founder_ballot]]  (refused while a founder has an active key)
//!  Vote         ── a maintainer of the electorate signs [[code_review.founder_vote]]
//! ```

use kovan_common::review::root::{FounderBallot, FounderChange, FounderChangeKind, FounderVote, ReviewRoot, Role};
use kovan_common::review::root_append::{append_founder_ballot, append_founder_change, append_founder_vote};
use kovan_common::review::signing::founders::{
    BallotOutcome, BallotProblem, BallotTrust, FounderChangeProblem, FounderOrigin, FounderStanding, VoteProblem,
};
use kovan_common::review::signing::keystore::UnlockedKey;
use kovan_common::review::signing::registry::Registry;

use super::{founder_signer, maintainer_signer, signer_text};

/// One ballot, for the section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BallotRow {
    pub id: String,
    /// [`ballot_line`].
    pub line: String,
    /// Open today: votes can be cast.
    pub open: bool,
    /// The electorate (also the candidates).
    pub electorate: Vec<String>,
    /// Who has a counted vote.
    pub voted: Vec<String>,
}

/// The "Founders & lineage" section's content.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LineageView {
    /// The lineage fingerprint (64 hex digits).
    pub fingerprint: String,
    /// One line per founder in standing now.
    pub founders: Vec<String>,
    /// The ids of the founders in standing now.
    pub founder_ids: Vec<String>,
    /// One line per retired founder span (listed permanently).
    pub retired: Vec<String>,
    pub ballots: Vec<BallotRow>,
    /// A founder who can act today, if any (a ballot is refused then).
    pub active_founder: Option<String>,
    /// Admitted maintainers who are not founders now (Add founder's
    /// choices).
    pub addable: Vec<String>,
}

fn origin_text(s: &FounderStanding) -> String {
    let since = s.from.as_deref().map_or("from the start".to_string(), |d| format!("since {d}"));
    match &s.origin {
        FounderOrigin::FirstUse => format!("{since}, declared in kovan_root.toml (trusted on first use)"),
        FounderOrigin::Added { by, key, .. } => format!("{since}, added by {by} (key {key}) \u{2714}"),
        FounderOrigin::Elected { ballot, casting: None } => format!("{since}, elected by maintainer ballot {ballot}"),
        FounderOrigin::Elected { ballot, casting: Some(c) } => {
            format!("{since}, elected by maintainer ballot {ballot} on {c}'s casting vote at the close")
        }
        FounderOrigin::RepositoryCommit { reviewed_in, .. } => format!(
            "{since}, \u{26a0} trust root replaced by repository commit{}",
            reviewed_in.as_deref().map(|r| format!(" ({r})")).unwrap_or_default()
        ),
    }
}

/// One line about a ballot: its dates, and its result or count so far.
pub fn ballot_line(b: &BallotTrust) -> String {
    let head = format!("ballot {} (opened {}, closes {})", b.id, b.opened, b.closes);
    let n = b.electorate.len();
    match &b.outcome {
        BallotOutcome::Void(p) => format!("{head}: void, {}", ballot_problem_text(p)),
        BallotOutcome::Elected { founder, from, casting } => format!(
            "{head}: {founder} elected founder from {from} ({} vote{} of {n}; {} needed){}",
            b.votes.iter().filter(|v| v.candidate == *founder).count(),
            if b.votes.len() == 1 { "" } else { "s" },
            b.needed,
            casting.as_deref().map(|c| format!(", on {c}'s casting vote at the close")).unwrap_or_default()
        ),
        BallotOutcome::NoResult => {
            let tally: Vec<String> = b.tally().iter().map(|(c, k)| format!("{c} {k}")).collect();
            format!(
                "{head}: no founder elected; {} of {n} maintainers needed; votes: {}",
                b.needed,
                if tally.is_empty() { "none yet".to_string() } else { tally.join(", ") }
            )
        }
    }
}

/// Plain words for a void ballot.
pub fn ballot_problem_text(p: &BallotProblem) -> String {
    match p {
        BallotProblem::BadDate(d) => format!("bad opening date {d:?}"),
        BallotProblem::DuplicateId => "an earlier ballot has the same id".into(),
        BallotProblem::NoElectorate => "no maintainer had an active key when it opened".into(),
        BallotProblem::Opener(s) => format!("not opened by a maintainer of its electorate ({})", signer_text(s)),
        BallotProblem::FounderActive { founder, date } => {
            format!("founder {founder} had an active key on {date} (a ballot then would bypass them)")
        }
    }
}

/// Plain words for a founder change with no effect.
pub fn change_problem_text(p: &FounderChangeProblem) -> String {
    match p {
        FounderChangeProblem::BadDate(d) => format!("bad date {d:?}"),
        FounderChangeProblem::OutOfOrder => "dated before the change before it".into(),
        FounderChangeProblem::UnknownReviewer => "it names no reviewer".into(),
        FounderChangeProblem::NotAdmitted => "the new founder is not admitted".into(),
        FounderChangeProblem::AlreadyFounder => "already a founder then".into(),
        FounderChangeProblem::NotFounder => "not a founder then".into(),
        FounderChangeProblem::Signer(s) => format!("not signed by a founder then ({})", signer_text(s)),
        FounderChangeProblem::LastResortRefused { active } => format!(
            "an unsigned change counts only when nobody has an active key, and {active} has one (ask them, or hold a maintainer ballot)"
        ),
    }
}

/// Plain words for a vote that does not count.
pub fn vote_problem_text(p: &VoteProblem) -> String {
    match p {
        VoteProblem::UnknownBallot => "it names no ballot".into(),
        VoteProblem::BallotVoid => "its ballot is void".into(),
        VoteProblem::BadDate(d) => format!("bad date {d:?}"),
        VoteProblem::OutsideBallot { opened, closes } => format!("dated outside the ballot ({opened} to before {closes})"),
        VoteProblem::CandidateNotInElectorate(c) => format!("{c} is not a maintainer of the ballot's electorate"),
        VoteProblem::Signer(s) => format!("not signed by a maintainer of the electorate ({})", signer_text(s)),
        VoteProblem::AlreadyVoted => "the voter has voted in this ballot already (the first vote stands)".into(),
    }
}

/// The section's content from the registry, as of `today`.
pub fn lineage_view(registry: &Registry, today: &str) -> LineageView {
    let lin = &registry.lineage;
    let current = registry.current_founders();
    LineageView {
        fingerprint: lin.fingerprint.clone(),
        founders: lin.current().iter().map(|s| format!("{} \u{2014} {}", s.id, origin_text(s))).collect(),
        founder_ids: current.clone(),
        retired: lin
            .retired()
            .iter()
            .map(|s| {
                format!(
                    "{} \u{2014} founder {} until {}{}; records dated before keep their founder standing",
                    s.id,
                    origin_text(s),
                    s.to.as_deref().unwrap_or("?"),
                    s.retired_by.as_ref().map(|(b, k)| format!(" (retired by {b}, key {k})")).unwrap_or_default()
                )
            })
            .collect(),
        ballots: lin
            .ballots
            .iter()
            .map(|b| BallotRow {
                id: b.id.clone(),
                line: ballot_line(b),
                open: b.open_on(today),
                electorate: b.electorate.clone(),
                voted: b.votes.iter().map(|v| v.voter.clone()).collect(),
            })
            .collect(),
        active_founder: registry.active_founder_on(today).map(str::to_string),
        addable: registry
            .reviewers
            .iter()
            .filter(|r| r.is_admitted() && r.current_role() == Role::Maintainer && !current.contains(&r.id))
            .map(|r| r.id.clone())
            .collect(),
    }
}

/// The section's forms.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FounderForms {
    /// Add founder's choice.
    pub add: String,
    /// Open ballot's id (empty: `ballot-<today>`).
    pub ballot_id: String,
    /// Vote's candidate.
    pub candidate: String,
}

/// The ballot id Open ballot uses: the typed one, else `ballot-<date>`.
pub fn ballot_id(typed: &str, date: &str) -> String {
    let t = typed.trim();
    if t.is_empty() {
        format!("ballot-{date}")
    } else {
        t.to_string()
    }
}

/// What stops `AddFounder` / `RetireFounder` / `OpenBallot` / `Vote` (the
/// founder actions of [`super::UserAction`]), signed by `signer` on `date`.
pub(super) fn founder_action_problems(
    root: &ReviewRoot,
    registry: &Registry,
    signer: (&str, &str),
    action: &super::UserAction,
    date: &str,
) -> Vec<String> {
    use super::UserAction as A;
    let mut p = Vec::new();
    let mut need = |r: Result<(), String>| {
        if let Err(e) = r {
            p.push(e);
        }
    };
    match action {
        A::AddFounder { reviewer } => {
            need(founder_signer(registry, signer, date));
            match registry.reviewer(reviewer) {
                None => need(Err(format!("{reviewer} is not in kovan_root.toml."))),
                Some(t) if !t.is_admitted() || t.current_role() != Role::Maintainer => {
                    need(Err(format!("{reviewer} is not an admitted maintainer: make them one first.")))
                }
                Some(_) if registry.is_founder_on(reviewer, date) => need(Err(format!("{reviewer} is a founder already."))),
                Some(_) => {}
            }
        }
        A::RetireFounder { founder } => {
            need(founder_signer(registry, signer, date));
            if !registry.is_founder_on(founder, date) {
                need(Err(format!("{founder} is not a founder now.")));
            }
        }
        A::OpenBallot { id } => {
            need(maintainer_signer(registry, signer, date));
            if let Some(f) = registry.active_founder_on(date) {
                need(Err(format!(
                    "Refused: founder {f} has an active key. A ballot is only for when no founder can act; ask them to add a founder."
                )));
            }
            let id = ballot_id(id, date);
            if root.code_review.as_ref().is_some_and(|c| c.founder_ballots.iter().any(|b| b.id == id)) {
                need(Err(format!("A ballot {id} exists already: type another id.")));
            }
        }
        A::Vote { ballot, candidate } => {
            need(maintainer_signer(registry, signer, date));
            match registry.lineage.ballots.iter().find(|b| b.id == *ballot) {
                None => need(Err(format!("No ballot {ballot}."))),
                Some(b) => {
                    if !b.open_on(date) {
                        need(Err(format!("Ballot {ballot} is not open today.")));
                    }
                    if !b.electorate.iter().any(|e| e == signer.0) {
                        need(Err(format!("{} is not in ballot {ballot}'s electorate (fixed when it opened).", signer.0)));
                    }
                    if b.votes.iter().any(|v| v.voter == signer.0) {
                        need(Err(format!("{} has voted in ballot {ballot} already.", signer.0)));
                    }
                    if !b.electorate.contains(candidate) {
                        need(Err("Choose a candidate from the electorate.".into()));
                    }
                }
            }
        }
        _ => {}
    }
    p
}

/// Sign and append a founder action to `text`; the new text and the notice.
pub(super) fn apply_founder_action(
    text: &str,
    root: &ReviewRoot,
    key: &UnlockedKey,
    action: &super::UserAction,
    date: &str,
) -> Result<(String, String), String> {
    use super::UserAction as A;
    let err = |e: &dyn std::fmt::Display| e.to_string();
    let change = |event: FounderChangeKind, founder: &str| -> Result<String, String> {
        let mut e = FounderChange {
            event,
            founder: founder.to_string(),
            date: date.to_string(),
            reviewed_in: None,
            signer: None,
            signature: None,
        };
        key.sign_founder_change(&mut e).map_err(|x| err(&x))?;
        append_founder_change(text, &e).map_err(|x| err(&x))
    };
    match action {
        A::AddFounder { reviewer } => Ok((
            change(FounderChangeKind::Add, reviewer)?,
            format!("{reviewer} is a founder from {date}, signed with your key {}.", key.key_id()),
        )),
        A::RetireFounder { founder } => Ok((
            change(FounderChangeKind::Retire, founder)?,
            format!("{founder} is no longer a founder from {date}; records dated before keep their founder standing."),
        )),
        A::OpenBallot { id } => {
            let mut b = FounderBallot { id: ballot_id(id, date), opened: date.to_string(), signer: None, signature: None };
            key.sign_founder_ballot(&mut b).map_err(|x| err(&x))?;
            let t = append_founder_ballot(text, &b).map_err(|x| err(&x))?;
            Ok((t, format!("Opened founder ballot {} on {date}; it runs 30 days.", b.id)))
        }
        A::Vote { ballot, candidate } => {
            let b = root
                .code_review
                .as_ref()
                .and_then(|c| c.founder_ballots.iter().find(|b| b.id == *ballot))
                .cloned()
                .ok_or_else(|| format!("No ballot {ballot}."))?;
            let mut v = FounderVote {
                ballot: ballot.clone(),
                candidate: candidate.clone(),
                date: date.to_string(),
                signer: None,
                signature: None,
            };
            key.sign_founder_vote(&mut v, &b).map_err(|x| err(&x))?;
            let t = append_founder_vote(text, &v).map_err(|x| err(&x))?;
            Ok((t, format!("Voted for {candidate} in ballot {ballot}, signed with your key {}.", key.key_id())))
        }
        other => Err(format!("{} is not a founder action.", other.label())),
    }
}
