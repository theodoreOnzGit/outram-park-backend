//! **Founders as a lineage** (GitHub #1248, maintainer 2026-11-05;
//! supersedes the single-handover design of #1022 and keeps its
//! requirement that the old chain still verifies).
//!
//! "instead of founder handover, have multiple 'founders' and a list of
//! retired founders"; "if all founders are gone, then maintainers can vote
//! to vote in a new founder (promote from maintainer), based on majority
//! vote". The design is the #1248 comment "Design decided", section 1, and
//! the "Scope split" comment: forks, cross-lineage recognition and the
//! recognised-lineages table are **deferred** and not built here.
//!
//! ```text
//!  [code_review] founder = "a"  /  founders = ["a", "b"]    trusted on first use (from the start)
//!        │
//!  [[code_review.founder_change]]  add c   (signed by a founder in standing just before it)
//!  [[code_review.founder_change]]  retire a (by a founder; retiring yourself is allowed)
//!        │       a keeps founder standing for everything dated BEFORE the retirement
//!        │
//!  no founder has an active key?
//!        ├─ maintainers vote:  [[code_review.founder_ballot]] + [[code_review.founder_vote]]
//!        │     electorate = maintainers with an active key on the opening date (fixed then)
//!        │     elected when more than half of the electorate signs for one candidate
//!        │     (1 maintainer: only once the ballot has been open 14 days);
//!        │     at close (30 days) without a majority: the earliest-admitted voter's
//!        │     candidate wins if it is one of the two leading; one founder per ballot;
//!        │     refused or void while any founder has an active key
//!        └─ nobody at all with an active key?  unsigned `add` by a reviewed commit:
//!              the last resort, shown forever as "trust root replaced by repository commit"
//! ```
//!
//! # The rules
//!
//! - **Standing.** A founder's standing runs from its start (the first use;
//!   an `add`'s date; a ballot's decision date) until the date of a
//!   `retire`, exclusive: a record dated before the retirement is judged
//!   with them as a founder ([`FounderStanding::covers`]).
//! - **Acting as a founder** on a date ([`super::registry::Registry::founder_can_act`])
//!   needs the standing then, the maintainer role then, an admission, no
//!   revocation by then, and a trusted key active then
//!   ([`super::registry::ReviewerTrust::active_on`]).
//! - **Signed changes** are judged in date order (file order within a
//!   date; changes before ballot results on the same date), each against
//!   the founders in standing just before it, so in a same-day handover a
//!   signs c's `add`, then their own `retire`. Unsigned, forged, out of
//!   order or by a non-founder: no effect, and a warning.
//! - **Add** names an admitted reviewer who is not a founder then.
//!   **Retire** names a founder in standing then.
//! - **The last resort** (an unsigned `add`): counts only when, on its date,
//!   no reviewer is a founder or maintainer with an active key (the new
//!   founder included when admitted already: one founder or maintainer
//!   with a key uses the ballot instead). A new founder with no admission of their own is then
//!   trusted on first use, as the first founder is. Shown forever.
//! - **Ballots** are judged by the registry from the signed records alone
//!   (no clock): a decision may carry a date in the future (the 14-day wait,
//!   or the close), and the standing starts then.
//! - **Lineage fingerprint** ([`Lineage::fingerprint`]): SHA-256 over
//!   `kovan-lineage-v1`, each declared founder's id with its first key (id
//!   and public key), then every founder change that took effect (its
//!   signed bytes and signature, the last resort included, since it
//!   changes the trust root) and every elected ballot (its bytes and
//!   signature, and its counted votes'), in the order they took effect.
//!   Derived only: no new file. Records that change no founder never move
//!   it.

use std::collections::BTreeMap;

use sha2::{Digest as _, Sha256};

use crate::zotero::date::{civil_from_days, days_from_civil};

use super::super::root::{FounderBallot, FounderChangeKind, FounderVote, KeyEventKind, ReviewRoot, Role};
use super::registry::{find_signer, ReviewerTrust, SignerProblem};
use super::{founder_ballot_bytes, founder_change_bytes, founder_vote_bytes, is_date, line};

/// How long a ballot runs (maintainer, #1248 "Design decided").
pub const BALLOT_DAYS: u64 = 30;
/// With a single maintainer, how long the ballot must have been open and
/// visible before their own vote counts.
pub const SOLE_MAINTAINER_WAIT_DAYS: u64 = 14;

/// `date` plus `days`, as `YYYY-MM-DD`; `None` for a malformed date, for
/// one that does not exist (`2026-02-30`), or for a result past year 9999.
///
/// Pure integer calendar arithmetic (the crate's own `days_from_civil` /
/// `civil_from_days`), so it runs on every target, wasm included.
/// ~~It used `chrono`~~ **CHANGED 2026-11-05** (maintainer): chrono stays
/// desktop-only; the Pages wasm build had failed on this import. Pinned
/// against `chrono` itself, over every day of 1999-2101, by the founders
/// tests.
pub fn add_days(date: &str, days: u64) -> Option<String> {
    if !is_date(date) {
        return None;
    }
    let field = |range: std::ops::Range<usize>| date[range].parse::<i64>().ok();
    let (y, m, d) = (field(0..4)?, field(5..7)?, field(8..10)?);
    let day = days_from_civil(y, m, d);
    if civil_from_days(day) != (y, m, d) {
        return None;
    }
    let (y, m, d) = civil_from_days(day.checked_add(i64::try_from(days).ok()?)?);
    if y > 9999 {
        return None;
    }
    Some(format!("{y:04}-{m:02}-{d:02}"))
}

/// The votes a ballot needs: more than half of the electorate.
pub fn votes_needed(electorate: usize) -> usize {
    electorate / 2 + 1
}

/// How a founder came to stand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FounderOrigin {
    /// Declared in `[code_review] founder`/`founders`: trusted on first use.
    FirstUse,
    /// Added by a signed `founder_change` (entry `index`) by founder `by`'s
    /// key `key`.
    Added { index: usize, by: String, key: String },
    /// Elected by ballot `ballot`; `casting`: the voter whose casting vote
    /// decided it at the close.
    Elected { ballot: String, casting: Option<String> },
    /// The unsigned last resort (entry `index`): trust root replaced by
    /// repository commit.
    RepositoryCommit { index: usize, reviewed_in: Option<String> },
}

/// One span of founder standing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FounderStanding {
    pub id: String,
    /// `None`: from the start.
    pub from: Option<String>,
    /// The retirement date (exclusive); `None`: still a founder.
    pub to: Option<String>,
    pub origin: FounderOrigin,
    /// Who retired them, and with which key.
    pub retired_by: Option<(String, String)>,
}

impl FounderStanding {
    /// Whether the standing covers `date`.
    pub fn covers(&self, date: &str) -> bool {
        self.from.as_deref().is_none_or(|f| f <= date) && self.to.as_deref().is_none_or(|t| date < t)
    }
}

/// Why a founder change has no effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FounderChangeProblem {
    BadDate(String),
    /// Dated before the change before it (the list is append-only).
    OutOfOrder,
    /// It names no `[[reviewer]]`.
    UnknownReviewer,
    /// An `add` naming a reviewer who is not admitted.
    NotAdmitted,
    /// An `add` naming a founder in standing then.
    AlreadyFounder,
    /// A `retire` naming someone who is not a founder then.
    NotFounder,
    /// Unsigned (a `retire`), forged, or not by a founder in standing just
    /// before it with a key active then.
    Signer(SignerProblem),
    /// The unsigned last resort while `active` (a founder or maintainer)
    /// has an active key: refused, or it would be a bypass.
    LastResortRefused { active: String },
}

/// Why a ballot is void.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BallotProblem {
    BadDate(String),
    /// An earlier ballot has the same id.
    DuplicateId,
    /// No maintainer had an active key on the opening date.
    NoElectorate,
    /// Not signed by a maintainer of the electorate.
    Opener(SignerProblem),
    /// Founder `founder` had an active key on `date` (the opening date or
    /// the decision date): a ballot then would bypass them.
    FounderActive { founder: String, date: String },
}

/// Why a vote does not count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoteProblem {
    /// No ballot has this id.
    UnknownBallot,
    /// The ballot it names is void from the start (bad date, duplicate,
    /// unsigned opener, no electorate).
    BallotVoid,
    BadDate(String),
    /// Before the opening or on/after the close.
    OutsideBallot { opened: String, closes: String },
    /// The candidate is not a maintainer of the electorate.
    CandidateNotInElectorate(String),
    /// Unsigned, forged, or not by a maintainer of the electorate with a
    /// key active on the vote's date.
    Signer(SignerProblem),
    /// The voter has a counted vote in this ballot already (the first
    /// stands).
    AlreadyVoted,
}

/// A ballot's result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BallotOutcome {
    Void(BallotProblem),
    /// `founder` is a founder from `from`; `casting`: decided at the close
    /// by that voter's casting vote.
    Elected { founder: String, from: String, casting: Option<String> },
    /// No candidate has reached the bar yet, or none did by the close and
    /// no casting vote decides it.
    NoResult,
}

/// One counted vote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountedVote {
    /// Its position in `[[code_review.founder_vote]]`.
    pub index: usize,
    pub voter: String,
    pub key: String,
    pub candidate: String,
    pub date: String,
}

/// One ballot, judged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BallotTrust {
    /// Its position in `[[code_review.founder_ballot]]`.
    pub index: usize,
    pub id: String,
    pub opened: String,
    /// The opening date plus [`BALLOT_DAYS`] (`""` for a malformed one).
    pub closes: String,
    /// Maintainers with an active key on the opening date, fixed then.
    pub electorate: Vec<String>,
    /// More than half of the electorate.
    pub needed: usize,
    pub votes: Vec<CountedVote>,
    pub outcome: BallotOutcome,
}

impl BallotTrust {
    /// Counted votes per candidate, most first (ties: first voted first).
    pub fn tally(&self) -> Vec<(String, usize)> {
        rank(&self.votes)
    }

    /// Whether it is open on `date`: not void, not decided by then, and
    /// `opened <= date < closes`.
    pub fn open_on(&self, date: &str) -> bool {
        let decided = matches!(&self.outcome, BallotOutcome::Elected { from, .. } if from.as_str() <= date);
        !decided
            && !matches!(self.outcome, BallotOutcome::Void(_))
            && self.opened.as_str() <= date
            && date < self.closes.as_str()
    }
}

/// The founders of this lineage, judged ([`replay`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lineage {
    /// Every span of founder standing, first-use founders first, then in
    /// the order they began.
    pub standings: Vec<FounderStanding>,
    /// Founder changes with no effect: (entry index, founder named, why).
    pub change_problems: Vec<(usize, String, FounderChangeProblem)>,
    pub ballots: Vec<BallotTrust>,
    /// Votes that do not count: (entry index, voter, why).
    pub vote_problems: Vec<(usize, String, VoteProblem)>,
    /// The lineage fingerprint (module doc), 64 lowercase hex digits.
    pub fingerprint: String,
    /// Reviewers (by position in the root) made founders by the last
    /// resort who carry no admission: trusted on first use.
    pub(crate) first_use: Vec<usize>,
}

impl Lineage {
    /// The standings that cover `date`.
    pub fn founders_on<'a>(&'a self, date: &'a str) -> impl Iterator<Item = &'a FounderStanding> + 'a {
        self.standings.iter().filter(move |s| s.covers(date))
    }

    /// The founders in standing now (not retired), in order.
    pub fn current(&self) -> Vec<&FounderStanding> {
        self.standings.iter().filter(|s| s.to.is_none()).collect()
    }

    /// The retired founder spans.
    pub fn retired(&self) -> Vec<&FounderStanding> {
        self.standings.iter().filter(|s| s.to.is_some()).collect()
    }
}

fn rank(votes: &[CountedVote]) -> Vec<(String, usize)> {
    let mut order: Vec<(String, usize, usize)> = Vec::new();
    for (n, v) in votes.iter().enumerate() {
        match order.iter_mut().find(|(c, _, _)| *c == v.candidate) {
            Some(e) => e.1 += 1,
            None => order.push((v.candidate.clone(), 1, n)),
        }
    }
    order.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.cmp(&b.2)));
    order.into_iter().map(|(c, n, _)| (c, n)).collect()
}

/// Whether reviewer `m` was a maintainer with an active key on `date`
/// (admitted by then, not revoked by then, a trusted key created by then
/// and active then).
fn maintainer_active(root: &ReviewRoot, state: &[ReviewerTrust], m: usize, date: &str) -> bool {
    state[m].role_at(date) == Role::Maintainer && state[m].active_on(date) && created_by(root, state, m, date)
}

/// `m` has a trusted, active key on `date` created on or before it.
fn created_by(root: &ReviewRoot, state: &[ReviewerTrust], m: usize, date: &str) -> bool {
    root.reviewers[m].keys.iter().zip(&state[m].keys).any(|(k, t)| {
        t.status.is_trusted() && t.inactive_at(date).is_none() && k.created.as_str() <= date
    })
}

fn position(state: &[ReviewerTrust], id: &str) -> Option<usize> {
    state.iter().position(|r| r.id == id)
}

/// Whether reviewer `m` carries an admission of their own.
fn has_admission(root: &ReviewRoot, m: usize) -> bool {
    let r = &root.reviewers[m];
    r.admitted_by.is_some() || r.keys.iter().any(|k| k.history.iter().any(|e| e.event == KeyEventKind::Admitted))
}

struct Tallied {
    trust: BallotTrust,
    /// (decision date, winner, casting voter).
    decision: Option<(String, String, Option<String>)>,
    signature: String,
}

/// Judge the ballots and votes on their own (they do not depend on who is
/// a founder; the founder-active check comes after, in [`replay`]).
fn tally(
    root: &ReviewRoot,
    state: &[ReviewerTrust],
    ballots: &[FounderBallot],
    votes: &[FounderVote],
    vote_problems: &mut Vec<(usize, String, VoteProblem)>,
) -> Vec<Tallied> {
    let mut out: Vec<Tallied> = Vec::new();
    let mut electorates: Vec<Vec<usize>> = Vec::new();
    for (index, b) in ballots.iter().enumerate() {
        let closes = add_days(&b.opened, BALLOT_DAYS).unwrap_or_default();
        let electorate: Vec<usize> = if is_date(&b.opened) {
            (0..state.len()).filter(|&m| maintainer_active(root, state, m, &b.opened)).collect()
        } else {
            Vec::new()
        };
        let void = if !is_date(&b.opened) {
            Some(BallotProblem::BadDate(b.opened.clone()))
        } else if ballots[..index].iter().any(|o| o.id == b.id) {
            Some(BallotProblem::DuplicateId)
        } else if electorate.is_empty() {
            Some(BallotProblem::NoElectorate)
        } else {
            match (&b.signer, &b.signature) {
                (Some(sg), Some(sig)) => find_signer(state, &electorate, sg, sig, &founder_ballot_bytes(b), Some(&b.opened), &b.opened)
                    .err()
                    .map(BallotProblem::Opener),
                _ => Some(BallotProblem::Opener(SignerProblem::Unsigned)),
            }
        };
        out.push(Tallied {
            trust: BallotTrust {
                index,
                id: b.id.clone(),
                opened: b.opened.clone(),
                closes,
                electorate: electorate.iter().map(|&m| state[m].id.clone()).collect(),
                needed: votes_needed(electorate.len()),
                votes: Vec::new(),
                outcome: match void {
                    Some(p) => BallotOutcome::Void(p),
                    None => BallotOutcome::NoResult,
                },
            },
            decision: None,
            signature: b.signature.clone().unwrap_or_default(),
        });
        electorates.push(electorate);
    }
    for (vi, v) in votes.iter().enumerate() {
        let voter = v.signer.as_ref().and_then(|s| s.reviewer.clone()).unwrap_or_default();
        let mut fail = |p: VoteProblem| vote_problems.push((vi, voter.clone(), p));
        let Some(bi) = out.iter().position(|t| t.trust.id == v.ballot) else {
            fail(VoteProblem::UnknownBallot);
            continue;
        };
        let t = &out[bi];
        if matches!(t.trust.outcome, BallotOutcome::Void(_)) {
            fail(VoteProblem::BallotVoid);
            continue;
        }
        if !is_date(&v.date) {
            fail(VoteProblem::BadDate(v.date.clone()));
            continue;
        }
        if v.date < t.trust.opened || v.date >= t.trust.closes {
            fail(VoteProblem::OutsideBallot { opened: t.trust.opened.clone(), closes: t.trust.closes.clone() });
            continue;
        }
        if !t.trust.electorate.contains(&v.candidate) {
            fail(VoteProblem::CandidateNotInElectorate(v.candidate.clone()));
            continue;
        }
        let signed = match (&v.signer, &v.signature) {
            (Some(sg), Some(sig)) => find_signer(
                state,
                &electorates[bi],
                sg,
                sig,
                &founder_vote_bytes(v, &t.signature),
                Some(&v.date),
                &v.date,
            ),
            _ => Err(SignerProblem::Unsigned),
        };
        let (who, key) = match signed {
            Ok(x) => x,
            Err(p) => {
                fail(VoteProblem::Signer(p));
                continue;
            }
        };
        if t.trust.votes.iter().any(|c| c.voter == who) {
            fail(VoteProblem::AlreadyVoted);
            continue;
        }
        out[bi].trust.votes.push(CountedVote { index: vi, voter: who, key, candidate: v.candidate.clone(), date: v.date.clone() });
    }
    for t in &mut out {
        if matches!(t.trust.outcome, BallotOutcome::Void(_)) {
            continue;
        }
        t.trust.votes.sort_by(|a, b| a.date.cmp(&b.date).then(a.index.cmp(&b.index)));
        let n = t.trust.electorate.len();
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for v in &t.trust.votes {
            let c = counts.entry(v.candidate.as_str()).or_default();
            *c += 1;
            if *c >= t.trust.needed {
                let mut from = v.date.clone();
                if n == 1 {
                    let wait = add_days(&t.trust.opened, SOLE_MAINTAINER_WAIT_DAYS).unwrap_or_default();
                    if wait > from {
                        from = wait;
                    }
                }
                t.decision = Some((from, v.candidate.clone(), None));
                break;
            }
        }
        if t.decision.is_none() && !t.trust.votes.is_empty() {
            // At the close: the earliest-admitted voter's candidate, if it
            // is one of the two leading.
            let leading: Vec<String> = rank(&t.trust.votes).into_iter().take(2).map(|(c, _)| c).collect();
            let admitted = |id: &str| {
                let m = root.reviewers.iter().position(|r| r.id == id).unwrap_or(usize::MAX);
                let d = root.reviewers.get(m).and_then(|r| r.admitted.clone()).unwrap_or_default();
                (d, m)
            };
            let casting = t.trust.votes.iter().min_by_key(|v| admitted(&v.voter));
            if let Some(c) = casting.filter(|c| leading.contains(&c.candidate)) {
                t.decision = Some((t.trust.closes.clone(), c.candidate.clone(), Some(c.voter.clone())));
            }
        }
    }
    out
}

/// Whether some reviewer other than `except` acts as a founder on `date`
/// (in standing, a maintainer, an active key): their id.
fn founder_active(
    root: &ReviewRoot,
    state: &[ReviewerTrust],
    standings: &[FounderStanding],
    date: &str,
    except: Option<usize>,
) -> Option<String> {
    standings
        .iter()
        .filter(|s| s.covers(date))
        .filter_map(|s| position(state, &s.id))
        .find(|&m| Some(m) != except && maintainer_active(root, state, m, date))
        .map(|m| state[m].id.clone())
}

/// Judge the founders of `root` given the judged reviewers `state`
/// (module doc). `declared` are the first-use founders as written;
/// `initial` the ones that passed the founder checks (reviewer positions).
pub(crate) fn replay(root: &ReviewRoot, state: &[ReviewerTrust], declared: &[String], initial: &[usize]) -> Lineage {
    let cr = root.code_review.as_ref();
    let changes = cr.map_or(&[][..], |c| c.founder_changes.as_slice());
    let ballots = cr.map_or(&[][..], |c| c.founder_ballots.as_slice());
    let votes = cr.map_or(&[][..], |c| c.founder_votes.as_slice());
    let mut lin = Lineage::default();
    lin.standings = initial
        .iter()
        .map(|&m| FounderStanding { id: state[m].id.clone(), from: None, to: None, origin: FounderOrigin::FirstUse, retired_by: None })
        .collect();
    let mut hash = Sha256::new();
    let mut fp = String::from("kovan-lineage-v1\n");
    for id in declared {
        let k = root.reviewer(id).and_then(|r| r.keys.first());
        line(&mut fp, "founder", &[id, k.map_or("", |k| k.id.as_str()), k.map_or("", |k| k.public.as_str())]);
    }
    hash.update(fp.as_bytes());
    let mut tallied = tally(root, state, ballots, votes, &mut lin.vote_problems);

    // Changes in file order must not go backwards.
    let mut items: Vec<(String, u8, usize)> = Vec::new();
    let mut prev: Option<&str> = None;
    for (k, e) in changes.iter().enumerate() {
        if !is_date(&e.date) {
            lin.change_problems.push((k, e.founder.clone(), FounderChangeProblem::BadDate(e.date.clone())));
        } else if prev.is_some_and(|p| e.date.as_str() < p) {
            lin.change_problems.push((k, e.founder.clone(), FounderChangeProblem::OutOfOrder));
        } else {
            prev = Some(&e.date);
            items.push((e.date.clone(), 0, k));
        }
    }
    for (b, t) in tallied.iter().enumerate() {
        if let Some((d, _, _)) = &t.decision {
            items.push((d.clone(), 1, b));
        }
    }
    items.sort();
    for (date, kind, k) in items {
        if kind == 1 {
            let t = &mut tallied[k];
            let Some((_, winner, casting)) = t.decision.clone() else { continue };
            let active = founder_active(root, state, &lin.standings, &t.trust.opened, None)
                .map(|f| (f, t.trust.opened.clone()))
                .or_else(|| founder_active(root, state, &lin.standings, &date, None).map(|f| (f, date.clone())));
            if let Some((founder, d)) = active {
                t.trust.outcome = BallotOutcome::Void(BallotProblem::FounderActive { founder, date: d });
                continue;
            }
            t.trust.outcome = BallotOutcome::Elected { founder: winner.clone(), from: date.clone(), casting: casting.clone() };
            if !lin.standings.iter().any(|s| s.id == winner && s.covers(&date)) {
                lin.standings.push(FounderStanding {
                    id: winner,
                    from: Some(date),
                    to: None,
                    origin: FounderOrigin::Elected { ballot: t.trust.id.clone(), casting },
                    retired_by: None,
                });
            }
            hash.update(founder_ballot_bytes(&ballots[t.trust.index]));
            hash.update(sig_line(&t.signature).as_bytes());
            for v in &t.trust.votes {
                let fv = &votes[v.index];
                hash.update(founder_vote_bytes(fv, &t.signature));
                hash.update(sig_line(fv.signature.as_deref().unwrap_or("")).as_bytes());
            }
            continue;
        }
        let e = &changes[k];
        let mut fail = |p: FounderChangeProblem| lin.change_problems.push((k, e.founder.clone(), p));
        let Some(t) = position(state, &e.founder) else {
            fail(FounderChangeProblem::UnknownReviewer);
            continue;
        };
        let standing_now = lin.standings.iter().position(|s| s.id == e.founder && s.covers(&date));
        let origin = match (&e.signer, &e.signature) {
            (Some(sg), Some(sig)) => {
                let eligible: Vec<usize> = lin
                    .standings
                    .iter()
                    .filter(|s| s.covers(&date))
                    .filter_map(|s| position(state, &s.id))
                    .filter(|&m| state[m].is_admitted() && state[m].role_at(&date) == Role::Maintainer)
                    .collect();
                match find_signer(state, &eligible, sg, sig, &founder_change_bytes(e), Some(&date), &date) {
                    Ok((by, key)) => FounderOrigin::Added { index: k, by, key },
                    Err(p) => {
                        fail(FounderChangeProblem::Signer(p));
                        continue;
                    }
                }
            }
            _ if e.event == FounderChangeKind::Retire => {
                fail(FounderChangeProblem::Signer(SignerProblem::Unsigned));
                continue;
            }
            _ => {
                // The last resort: nobody with an active key (the new
                // founder counts too when admitted already: they would
                // use a ballot; one with no admission gets a key only by
                // this very entry).
                let other = (0..state.len()).find(|&m| {
                    (m != t || has_admission(root, t))
                        && (state[m].role_at(&date) == Role::Maintainer
                            || lin.standings.iter().any(|s| s.id == state[m].id && s.covers(&date)))
                        && state[m].active_on(&date)
                        && created_by(root, state, m, &date)
                });
                if let Some(m) = other {
                    fail(FounderChangeProblem::LastResortRefused { active: state[m].id.clone() });
                    continue;
                }
                FounderOrigin::RepositoryCommit { index: k, reviewed_in: e.reviewed_in.clone() }
            }
        };
        match e.event {
            FounderChangeKind::Add => {
                let last_resort = matches!(origin, FounderOrigin::RepositoryCommit { .. });
                if !last_resort && !state[t].is_admitted() {
                    fail(FounderChangeProblem::NotAdmitted);
                    continue;
                }
                if standing_now.is_some() {
                    fail(FounderChangeProblem::AlreadyFounder);
                    continue;
                }
                if last_resort && !has_admission(root, t) && !lin.first_use.contains(&t) {
                    lin.first_use.push(t);
                }
                lin.standings.push(FounderStanding { id: e.founder.clone(), from: Some(date.clone()), to: None, origin, retired_by: None });
            }
            FounderChangeKind::Retire => {
                let Some(s) = standing_now.filter(|&s| lin.standings[s].to.is_none()) else {
                    fail(FounderChangeProblem::NotFounder);
                    continue;
                };
                lin.standings[s].to = Some(date.clone());
                if let FounderOrigin::Added { by, key, .. } = origin {
                    lin.standings[s].retired_by = Some((by, key));
                }
            }
        }
        hash.update(founder_change_bytes(e));
        hash.update(sig_line(e.signature.as_deref().unwrap_or("")).as_bytes());
    }
    // A ballot with no decision is void if a founder could act when it
    // opened.
    for t in &mut tallied {
        if matches!(t.trust.outcome, BallotOutcome::NoResult) {
            if let Some(f) = founder_active(root, state, &lin.standings, &t.trust.opened, None) {
                t.trust.outcome = BallotOutcome::Void(BallotProblem::FounderActive { founder: f, date: t.trust.opened.clone() });
            }
        }
    }
    lin.ballots = tallied.into_iter().map(|t| t.trust).collect();
    lin.fingerprint = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    lin
}

fn sig_line(sig: &str) -> String {
    let mut s = String::new();
    line(&mut s, "signature", &[sig]);
    s
}
