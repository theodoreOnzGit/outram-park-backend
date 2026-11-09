//! Tests of **several founders, retired founders and the maintainer vote**
//! (GitHub #1248, NON-FORK scope; the #1022 requirement that the old chain
//! keeps verifying).
//!
//! Methodology, for every test: throwaway keys from `keystore::generate`
//! (real argon2id + AES-256-GCM + ed25519, in memory only), every founder
//! change, ballot and vote signed with `UnlockedKey` over the real
//! [`founder_change_bytes`] / [`founder_ballot_bytes`] /
//! [`founder_vote_bytes`], every registry judged by [`Registry::build`].
//! Each verdict is checked against the rule in `signing::founders`'s module
//! doc, which was written before the tests: the standing spans
//! ([`Registry::is_founder_on`]), the ballot outcome with its decision date,
//! and the exact problem shown.

use super::founders::{
    add_days, votes_needed, BallotOutcome, BallotProblem, FounderChangeProblem, FounderOrigin, VoteProblem,
};
use super::keystore::UnlockedKey;
use super::registry::{Admission, Registry, SignerProblem, Warning};
use super::tests::{founder_and_alice, key, person, root, signed, ALICE, F, FOUNDER};
use super::*;
use crate::review::engine::SignaturePolicy;
use crate::review::ivv::{check_retirement, RetirementProblem};
use crate::review::root::{
    DevelopingOrganisation, FounderBallot, FounderChange, FounderChangeKind, FounderVote, OrganisationList,
    OrganisationRetirement, ReviewRoot, Reviewer, Role,
};

/// The day of the handover, the last-resort date and the ballot opening.
const D: &str = "2026-11-10";
const R: &str = "2026-11-15";
const O: &str = "2026-12-01";

fn change(k: Option<&UnlockedKey>, event: FounderChangeKind, founder: &str, date: &str) -> FounderChange {
    let mut e = FounderChange {
        event,
        founder: founder.into(),
        date: date.into(),
        reviewed_in: None,
        signer: None,
        signature: None,
    };
    if let Some(k) = k {
        k.sign_founder_change(&mut e).unwrap();
    }
    e
}

fn ballot(k: &UnlockedKey, id: &str, opened: &str) -> FounderBallot {
    let mut b = FounderBallot { id: id.into(), opened: opened.into(), signer: None, signature: None };
    k.sign_founder_ballot(&mut b).unwrap();
    b
}

fn vote(k: &UnlockedKey, b: &FounderBallot, candidate: &str, date: &str) -> FounderVote {
    let mut v = FounderVote { ballot: b.id.clone(), candidate: candidate.into(), date: date.into(), signer: None, signature: None };
    k.sign_founder_vote(&mut v, b).unwrap();
    v
}

fn with(mut rt: ReviewRoot, changes: Vec<FounderChange>, ballots: Vec<FounderBallot>, votes: Vec<FounderVote>) -> ReviewRoot {
    let c = rt.code_review.as_mut().unwrap();
    c.founder_changes = changes;
    c.founder_ballots = ballots;
    c.founder_votes = votes;
    rt
}

/// A maintainer `id` admitted by `by` on `admitted`, with one fresh key.
fn maintainer(by: &UnlockedKey, id: &str, kid: &str, admitted: &str) -> (Reviewer, UnlockedKey) {
    let (k, u) = key(id, kid);
    let mut r = person(id, Role::Maintainer, vec![k]);
    r.admitted = Some(admitted.into());
    by.admit(&mut r).unwrap();
    (r, u)
}

fn change_problems(reg: &Registry) -> Vec<FounderChangeProblem> {
    reg.lineage.change_problems.iter().map(|(_, _, p)| p.clone()).collect()
}

/// **Schemas never break: an old root with `founder = "x"`.** Methodology:
/// a root written as before GitHub #1248 (only `[code_review] founder`),
/// through TOML text. Pass: it carries no `founders`; it parses and judges
/// exactly as before (founder `Ok`, Alice admitted by the founder, her stamp
/// verifies, no warnings), with one first-use founder standing from the
/// start and never retired; written as `founders = [x]` instead, the
/// registry and the fingerprint are the same; `founder` and `founders`
/// together read as one list, `founder` first, each once; a reader shaped
/// like the old `[code_review]` reads a root with every new table; the
/// committed v1 to v4 fixtures read with none of them. The fingerprint is
/// 64 hex digits and the same on every build.
///
/// Result (2026-11-05): passes.
#[test]
fn an_old_single_founder_root_loads_and_verifies_unchanged() {
    let (founder, f, alice, a) = founder_and_alice();
    let rt = root(vec![founder.clone(), alice.clone()]);
    let text = rt.write_into("schema_version = 1\n").unwrap();
    assert!(text.contains("founder = \"github:founder\"") && !text.contains("founders") && !text.contains("founder_"));
    let old = ReviewRoot::parse(&text).unwrap();
    let reg = Registry::build(&old);
    assert_eq!(reg.founder, Ok(FOUNDER.to_string()));
    assert!(reg.warnings().is_empty(), "{:?}", reg.warnings());
    assert_eq!(reg.reviewer(ALICE).unwrap().admission, Admission::Admitted { by: FOUNDER.into(), key: "k1".into() });
    assert!(verify_review(&signed(&a, F, "2026-12-01"), &reg).is_verified());
    assert_eq!(reg.lineage.standings.len(), 1);
    let s = &reg.lineage.standings[0];
    assert_eq!((s.id.as_str(), s.from.as_deref(), s.to.as_deref(), &s.origin), (FOUNDER, None, None, &FounderOrigin::FirstUse));
    assert!(reg.is_founder_on(FOUNDER, "2000-01-01") && reg.is_founder_on(FOUNDER, "2999-01-01"));
    assert_eq!(reg.current_founders(), vec![FOUNDER.to_string()]);
    assert_eq!(reg.lineage.fingerprint.len(), 64);
    assert!(reg.lineage.fingerprint.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(Registry::build(&old).lineage.fingerprint, reg.lineage.fingerprint);

    let mut listed = rt.clone();
    let c = listed.code_review.as_mut().unwrap();
    c.founder = None;
    c.founders = vec![FOUNDER.into()];
    let listed_text = listed.write_into("schema_version = 1\n").unwrap();
    assert!(listed_text.contains("founders = [\"github:founder\"]"), "{listed_text}");
    let reg2 = Registry::build(&ReviewRoot::parse(&listed_text).unwrap());
    assert_eq!(reg2, reg, "founders = [x] judges as founder = x");

    let mut both = rt.code_review.clone().unwrap();
    both.founders = vec!["github:b".into(), FOUNDER.into(), "github:b".into()];
    assert_eq!(both.declared_founders(), vec![FOUNDER.to_string(), "github:b".into()]);

    // An old-shaped reader, and the old fixtures.
    #[derive(serde::Deserialize)]
    struct OldCodeReview {
        founder: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct OldRoot {
        code_review: Option<OldCodeReview>,
    }
    let b = ballot(&f, "b1", O);
    let full = with(rt.clone(), vec![change(Some(&f), FounderChangeKind::Add, ALICE, D)], vec![b.clone()], vec![vote(&f, &b, FOUNDER, O)]);
    let full_text = full.write_into("schema_version = 1\n").unwrap();
    let o: OldRoot = toml::from_str(&full_text).unwrap();
    assert_eq!(o.code_review.unwrap().founder.as_deref(), Some(FOUNDER));
    assert_eq!(ReviewRoot::parse(&full_text).unwrap(), full, "the new tables round-trip");
    for fx in [
        include_str!("../../../tests/fixtures/review/v1/kovan_root.toml"),
        include_str!("../../../tests/fixtures/review/v2/kovan_root.toml"),
        include_str!("../../../tests/fixtures/review/v3/kovan_root.toml"),
        include_str!("../../../tests/fixtures/review/v4/kovan_root.toml"),
    ] {
        let p = ReviewRoot::parse(fx).unwrap();
        let c = p.code_review.unwrap_or_default();
        assert!(c.founders.is_empty() && c.founder_changes.is_empty() && c.founder_ballots.is_empty() && c.founder_votes.is_empty());
    }
}

/// **The handover #1211 broke, now through a second founder** (#1022's
/// requirement). Methodology: founder F (`k1`), Bob a maintainer and Alice
/// a reviewer, both admitted by F on 2026-10-07; on D, F signs `add Bob`,
/// then `retire F`, in that order. Pass: F is a founder for every date
/// before D and not from D; Bob is one from D; Alice's admission by F still
/// verifies, and so does her stamp; an organisation retirement signed by F
/// dated before D counts, one dated D does not (`NotEligible`), one signed
/// by Bob dated D counts; F can no longer add anyone (`NotEligible`), Bob
/// cannot retire F again (`NotFounder`), an unsigned retirement of Bob has
/// no effect (`Unsigned`), adding Bob twice is `AlreadyFounder`; each
/// accepted change moves the lineage fingerprint, a roster record does
/// not.
///
/// Result (2026-11-05): passes.
#[test]
fn handover_by_adding_a_second_founder_then_retiring_the_first() {
    let (founder, f, alice, a) = founder_and_alice();
    let (bob, b) = maintainer(&f, "github:bob", "b1", "2026-10-07");
    let base = root(vec![founder, bob, alice]);
    let ok = vec![change(Some(&f), FounderChangeKind::Add, "github:bob", D), change(Some(&f), FounderChangeKind::Retire, FOUNDER, D)];
    let rt = with(base.clone(), ok.clone(), vec![], vec![]);
    let reg = Registry::build(&rt);
    assert!(reg.warnings().is_empty(), "{:?}", reg.warnings());
    assert!(reg.is_founder_on(FOUNDER, "2026-11-09") && !reg.is_founder_on(FOUNDER, D));
    assert!(!reg.is_founder_on("github:bob", "2026-11-09") && reg.is_founder_on("github:bob", D));
    assert_eq!(reg.current_founders(), vec!["github:bob".to_string()]);
    let retired = reg.lineage.retired();
    assert_eq!((retired[0].id.as_str(), retired[0].to.as_deref()), (FOUNDER, Some(D)));
    assert_eq!(retired[0].retired_by, Some((FOUNDER.to_string(), "k1".to_string())));
    assert_eq!(reg.reviewer(ALICE).unwrap().admission, Admission::Admitted { by: FOUNDER.into(), key: "k1".into() });
    assert!(verify_review(&signed(&a, F, "2026-12-01"), &reg).is_verified(), "the old chain still verifies");
    assert!(reg.founder_can_act("github:bob", D) && !reg.founder_can_act(FOUNDER, D));
    assert_eq!(reg.active_founder_on(D), Some("github:bob"));

    // Founder-only organisation retirements, judged by the date.
    let mut dev = DevelopingOrganisation { krate: None, name: "Outram Park".into(), date: "2026-10-08".into(), signer: None, signature: None };
    f.sign_developing_organisation(&mut dev).unwrap();
    let mut rt2 = rt.clone();
    rt2.code_review.as_mut().unwrap().developing_organisation = vec![dev.clone()];
    let reg2 = Registry::build(&rt2);
    let retire = |k: &UnlockedKey, date: &str| {
        let mut r = OrganisationRetirement {
            list: OrganisationList::DevelopingOrganisation,
            reviewer: None,
            record: dev.signature.clone().unwrap(),
            date: date.into(),
            signer: None,
            signature: None,
        };
        k.sign_organisation_retirement(&mut r).unwrap();
        check_retirement(&rt2, &reg2, &r, SignaturePolicy::Enforce)
    };
    assert_eq!(retire(&f, "2026-11-09"), Ok(Some((FOUNDER.into(), "k1".into()))));
    assert_eq!(retire(&f, D), Err(RetirementProblem::Unverified(SignerProblem::NotEligible(FOUNDER.into()))));
    assert_eq!(retire(&b, D), Ok(Some(("github:bob".into(), "b1".into()))));

    // Changes that have no effect.
    let mut bad = ok.clone();
    bad.push(change(Some(&f), FounderChangeKind::Add, ALICE, "2026-11-11"));
    bad.push(change(Some(&b), FounderChangeKind::Retire, FOUNDER, "2026-11-12"));
    bad.push(change(None, FounderChangeKind::Retire, "github:bob", "2026-11-13"));
    bad.push(change(Some(&b), FounderChangeKind::Add, "github:bob", "2026-11-14"));
    bad.push(change(Some(&b), FounderChangeKind::Add, "github:nobody", "2026-11-14"));
    bad.push(change(Some(&b), FounderChangeKind::Add, ALICE, "2026-11-01"));
    let reg3 = Registry::build(&with(base.clone(), bad, vec![], vec![]));
    assert_eq!(
        change_problems(&reg3),
        vec![
            FounderChangeProblem::OutOfOrder,
            FounderChangeProblem::Signer(SignerProblem::NotEligible(FOUNDER.into())),
            FounderChangeProblem::NotFounder,
            FounderChangeProblem::Signer(SignerProblem::Unsigned),
            FounderChangeProblem::AlreadyFounder,
            FounderChangeProblem::UnknownReviewer,
        ]
    );
    assert_eq!(reg3.current_founders(), vec!["github:bob".to_string()]);
    assert!(reg3.warnings().iter().any(|w| matches!(w, Warning::FounderChange { index: 2, .. })));

    // The fingerprint: moved by each accepted change, not by other records.
    let fp0 = Registry::build(&base).lineage.fingerprint;
    let fp1 = Registry::build(&with(base.clone(), ok[..1].to_vec(), vec![], vec![])).lineage.fingerprint;
    assert!(fp0 != fp1 && fp1 != reg.lineage.fingerprint && fp0 != reg.lineage.fingerprint);
    assert_eq!(reg3.lineage.fingerprint, reg.lineage.fingerprint, "changes with no effect do not move it");
    assert_eq!(Registry::build(&rt2).lineage.fingerprint, reg.lineage.fingerprint, "an organisation record does not move it");
}

/// **The unsigned last resort** (a commit changes founders). Methodology:
/// founder F retires their own key on D (signed); Bob, a maintainer, has an
/// active key; an unsigned `add` of Bob dated R is refused naming Bob
/// (`LastResortRefused`), as is one dated before D naming F. Then Bob's key
/// is retired (signed, by Bob) before R and Zed, a maintainer entry with no
/// admission, is added unsigned on R with `reviewed_in`. Pass: Zed is a
/// founder from R, trusted on first use (his key is trusted, his stamp
/// verifies), "trust root replaced by repository commit" is a permanent
/// warning, and Zed can then sign an `add` (of Carol, an admitted
/// reviewer, who has no maintainer key to block the last resort).
///
/// Result (2026-11-05): passes.
#[test]
fn the_unsigned_last_resort_counts_only_when_nobody_has_an_active_key() {
    let (mut founder, f, _alice, _) = founder_and_alice();
    let (mut bob, b) = maintainer(&f, "github:bob", "b1", "2026-10-07");
    // Carol is a reviewer: no maintainer's key blocks the last resort.
    let (ck, _c) = key("github:carol", "c1");
    let mut carol = person("github:carol", Role::Reviewer, vec![ck]);
    carol.scope = vec!["crates/tampines/**".into()];
    carol.admitted = Some("2026-10-07".into());
    f.admit(&mut carol).unwrap();
    f.retire_key(FOUNDER, &mut founder.keys[0], D).unwrap();
    let mut last = change(None, FounderChangeKind::Add, "github:bob", R);
    last.reviewed_in = Some("https://github.com/o/r/pull/1".into());
    let early = change(None, FounderChangeKind::Add, "github:bob", "2026-11-01");
    let reg = Registry::build(&with(root(vec![founder.clone(), bob.clone()]), vec![early, last], vec![], vec![]));
    assert_eq!(
        change_problems(&reg),
        vec![
            FounderChangeProblem::LastResortRefused { active: FOUNDER.into() },
            FounderChangeProblem::LastResortRefused { active: "github:bob".into() },
        ]
    );

    b.retire_key("github:bob", &mut bob.keys[0], "2026-11-12").unwrap();
    let (zk, z) = key("github:zed", "z1");
    let zed = person("github:zed", Role::Maintainer, vec![zk]);
    let mut by_commit = change(None, FounderChangeKind::Add, "github:zed", R);
    by_commit.reviewed_in = Some("https://github.com/o/r/pull/2".into());
    let then = change(Some(&z), FounderChangeKind::Add, "github:carol", "2026-11-16");
    let rt = with(root(vec![founder, bob, carol, zed]), vec![by_commit, then], vec![], vec![]);
    let reg = Registry::build(&rt);
    assert_eq!(change_problems(&reg), vec![]);
    assert!(reg.is_founder_on("github:zed", R) && !reg.is_founder_on("github:zed", "2026-11-14"));
    assert!(reg.is_founder_on("github:carol", "2026-11-16"));
    let zt = reg.reviewer("github:zed").unwrap();
    assert_eq!(zt.admission, Admission::Founder);
    assert!(zt.keys[0].status.is_trusted());
    assert!(verify_review(&signed(&z, F, "2026-11-20"), &reg).is_verified());
    assert!(matches!(
        reg.lineage.standings.iter().find(|s| s.id == "github:zed").unwrap().origin,
        FounderOrigin::RepositoryCommit { index: 0, .. }
    ));
    assert!(reg.warnings().contains(&Warning::TrustRootReplacedByCommit {
        founder: "github:zed".into(),
        date: R.into(),
        reviewed_in: Some("https://github.com/o/r/pull/2".into()),
    }));
}

/// Founder F (key retired on D, signed) and `n` maintainers M1..Mn
/// admitted by F on 2026-10-08, 2026-10-09, …: no founder has an active
/// key from D.
fn electorate(n: usize) -> (ReviewRoot, UnlockedKey, Vec<UnlockedKey>) {
    let (fk, f) = key(FOUNDER, "k1");
    let mut founder = person(FOUNDER, Role::Maintainer, vec![fk]);
    founder.admitted = Some("2026-10-07".into());
    let mut people = vec![];
    let mut keys = vec![];
    for i in 1..=n {
        let (r, u) = maintainer(&f, &format!("github:m{i}"), &format!("m{i}"), &format!("2026-10-{:02}", 7 + i));
        people.push(r);
        keys.push(u);
    }
    f.retire_key(FOUNDER, &mut founder.keys[0], D).unwrap();
    let mut all = vec![founder];
    all.extend(people);
    (root(all), f, keys)
}

fn outcome(reg: &Registry, i: usize) -> BallotOutcome {
    reg.lineage.ballots[i].outcome.clone()
}

fn elected(founder: &str, from: &str, casting: Option<&str>) -> BallotOutcome {
    BallotOutcome::Elected { founder: founder.into(), from: from.into(), casting: casting.map(str::to_string) }
}

/// **Vote thresholds for 1, 2, 3 and 4 maintainers** (maintainer, #1248
/// "Design decided": more than half of the electorate; 1 maintainer only
/// after 14 days). Methodology: the [`electorate`] fixture, a ballot
/// opened on O by M1, signed votes. Pass: `votes_needed` is 1, 2, 2, 3, 3
/// for 1 to 5; with 1 maintainer, M1's own vote on O elects M1 from O+14
/// (not before); with 2, one vote elects nobody before the close and two
/// elect on the second vote's date; with 3, two votes for M2 elect M2; with
/// 4, two votes are not enough and the third elects; the electorate and
/// `needed` are reported; the elected founder stands from the decision date
/// and can sign an `add` the day after.
///
/// Result (2026-11-05): passes.
#[test]
fn vote_thresholds_for_one_to_four_maintainers() {
    assert_eq!((1..=5).map(votes_needed).collect::<Vec<_>>(), vec![1, 2, 2, 3, 3]);
    let o1 = add_days(O, 1).unwrap();
    let o2 = add_days(O, 2).unwrap();
    let o3 = add_days(O, 3).unwrap();

    // 1 maintainer: counts only after 14 days open.
    let (rt, _, m) = electorate(1);
    let b = ballot(&m[0], "b", O);
    let reg = Registry::build(&with(rt, vec![], vec![b.clone()], vec![vote(&m[0], &b, "github:m1", O)]));
    let wait = add_days(O, 14).unwrap();
    assert_eq!(wait, "2026-12-15");
    assert_eq!(outcome(&reg, 0), elected("github:m1", &wait, None));
    assert!(!reg.is_founder_on("github:m1", "2026-12-14") && reg.is_founder_on("github:m1", &wait));
    assert_eq!((reg.lineage.ballots[0].electorate.len(), reg.lineage.ballots[0].needed), (1, 1));
    assert!(reg.lineage.ballots[0].open_on("2026-12-14") && !reg.lineage.ballots[0].open_on(&wait));

    // 2 maintainers: 2 votes.
    let (rt, _, m) = electorate(2);
    let b = ballot(&m[0], "b", O);
    let two = vec![vote(&m[0], &b, "github:m2", O), vote(&m[1], &b, "github:m2", &o2)];
    let reg = Registry::build(&with(rt.clone(), vec![], vec![b.clone()], two.clone()));
    assert_eq!(outcome(&reg, 0), elected("github:m2", &o2, None));
    // The elected founder adds another the day after.
    let add = change(Some(&m[1]), FounderChangeKind::Add, "github:m1", &o3);
    let reg = Registry::build(&with(rt, vec![add], vec![b.clone()], two));
    assert!(reg.is_founder_on("github:m1", &o3), "{:?}", reg.lineage.change_problems);
    assert!(matches!(&reg.lineage.standings[1].origin, FounderOrigin::Elected { ballot, casting: None } if ballot == "b"));

    // 3 maintainers: 2 votes.
    let (rt, _, m) = electorate(3);
    let b = ballot(&m[2], "b", O);
    let reg = Registry::build(&with(rt, vec![], vec![b.clone()], vec![vote(&m[0], &b, "github:m2", O), vote(&m[2], &b, "github:m2", &o1)]));
    assert_eq!(outcome(&reg, 0), elected("github:m2", &o1, None));
    assert_eq!(reg.lineage.ballots[0].needed, 2);

    // 4 maintainers: 2 are not enough, the third elects.
    let (rt, _, m) = electorate(4);
    let b = ballot(&m[0], "b", O);
    let mut votes = vec![vote(&m[0], &b, "github:m1", O), vote(&m[1], &b, "github:m1", &o1)];
    let reg = Registry::build(&with(rt.clone(), vec![], vec![b.clone()], votes.clone()));
    assert_eq!(reg.lineage.ballots[0].needed, 3);
    // (Not decided before the close: at the close M1, the earliest
    // admitted voter, casts it.)
    assert_eq!(outcome(&reg, 0), elected("github:m1", &reg.lineage.ballots[0].closes, Some("github:m1")));
    votes.push(vote(&m[3], &b, "github:m1", &o2));
    let reg = Registry::build(&with(rt, vec![], vec![b], votes));
    assert_eq!(outcome(&reg, 0), elected("github:m1", &o2, None));
    assert!(reg.warnings().iter().any(|w| matches!(w, Warning::FounderBallot(t) if t.id == "b")));
}

/// **The tie at the close, the deadline, and the electorate.**
/// Methodology: [`electorate`] with 4 and 5 maintainers. Pass: 2 against 2
/// at the close goes to the earliest-admitted voter's candidate (M1 voted
/// M3: M3 from the close, `casting` M1); when that voter's candidate is not
/// one of the two leading (5 maintainers: M2 2, M3 2, M5 1, M1 voted M5)
/// there is no result; a vote dated on the closing day or before the
/// opening is `OutsideBallot`; a maintainer admitted after the opening
/// cannot vote (not in the electorate: `NotEligible`) or be voted for
/// (`CandidateNotInElectorate`); a second vote by the same voter is
/// `AlreadyVoted`; an unsigned vote does not count; a vote naming no ballot
/// is `UnknownBallot`.
///
/// Result (2026-11-05): passes.
#[test]
fn tie_at_close_deadline_and_a_fixed_electorate() {
    let (rt, f, m) = electorate(4);
    let b = ballot(&m[0], "b", O);
    let closes = add_days(O, 30).unwrap();
    assert_eq!(closes, "2026-12-31");
    let votes = vec![
        vote(&m[0], &b, "github:m3", O),
        vote(&m[1], &b, "github:m4", O),
        vote(&m[2], &b, "github:m4", O),
        vote(&m[3], &b, "github:m3", O),
    ];
    let reg = Registry::build(&with(rt.clone(), vec![], vec![b.clone()], votes.clone()));
    assert_eq!(outcome(&reg, 0), elected("github:m3", &closes, Some("github:m1")));
    assert_eq!(reg.lineage.ballots[0].tally(), vec![("github:m3".to_string(), 2), ("github:m4".to_string(), 2)]);

    // Deadline, electorate, double votes.
    let (late, lk) = maintainer(&f, "github:late", "l1", "2026-12-05");
    let mut rt2 = rt.clone();
    rt2.reviewers.push(late);
    let mut unsigned = vote(&m[1], &b, "github:m1", O);
    unsigned.signature = None;
    let mut nowhere = vote(&m[1], &b, "github:m1", O);
    nowhere.ballot = "zzz".into();
    let more = vec![
        vote(&m[0], &b, "github:m3", &closes),
        vote(&m[0], &b, "github:m3", "2026-11-30"),
        vote(&lk, &b, "github:m3", "2026-12-06"),
        vote(&m[0], &b, "github:late", "2026-12-06"),
        vote(&m[0], &b, "github:m4", "2026-12-07"),
        vote(&m[1], &b, "github:m3", "2026-12-07"),
        vote(&m[1], &b, "github:m3", "2026-12-08"),
        unsigned,
        nowhere,
    ];
    let reg = Registry::build(&with(rt2, vec![], vec![b.clone()], more));
    let probs: Vec<VoteProblem> = reg.lineage.vote_problems.iter().map(|(_, _, p)| p.clone()).collect();
    let outside = VoteProblem::OutsideBallot { opened: O.into(), closes: closes.clone() };
    assert_eq!(
        probs,
        vec![
            outside.clone(),
            outside,
            VoteProblem::Signer(SignerProblem::NotEligible("github:late".into())),
            VoteProblem::CandidateNotInElectorate("github:late".into()),
            VoteProblem::AlreadyVoted,
            VoteProblem::Signer(SignerProblem::Unsigned),
            VoteProblem::UnknownBallot,
        ]
    );
    assert_eq!(reg.lineage.ballots[0].electorate.len(), 4, "fixed at the opening");

    // 5 maintainers: the casting voter's candidate is not one of the two
    // leading: no result.
    let (rt, _, m) = electorate(5);
    let b = ballot(&m[0], "b", O);
    let votes = vec![
        vote(&m[0], &b, "github:m5", O),
        vote(&m[1], &b, "github:m2", O),
        vote(&m[2], &b, "github:m2", O),
        vote(&m[3], &b, "github:m3", O),
        vote(&m[4], &b, "github:m3", O),
    ];
    let reg = Registry::build(&with(rt, vec![], vec![b], votes));
    assert_eq!(outcome(&reg, 0), BallotOutcome::NoResult);
    assert_eq!(reg.current_founders(), vec![FOUNDER.to_string()], "nobody elected");
}

/// **A ballot is refused or void while any founder has an active key; one
/// founder per ballot.** Methodology: [`electorate`] with 2 maintainers.
/// Pass: with the founder's key not retired, a ballot opened on O is void
/// (`FounderActive` naming F on O) even with both votes; with it retired, a
/// ballot elects M1 on O, and a second ballot opened after that is void
/// (`FounderActive` naming M1); a ballot unsigned, opened by a
/// non-maintainer, or with a duplicate id is void from the start and its
/// votes are `BallotVoid`; a founder whose key is un-retired before the
/// decision voids it (`FounderActive` on the decision date).
///
/// Result (2026-11-05): passes.
#[test]
fn a_vote_is_refused_while_a_founder_key_is_active() {
    // F's key active: build the fixture without the retirement.
    let (fk, f) = key(FOUNDER, "k1");
    let founder = person(FOUNDER, Role::Maintainer, vec![fk]);
    let (m1, u1) = maintainer(&f, "github:m1", "m1", "2026-10-08");
    let (m2, u2) = maintainer(&f, "github:m2", "m2", "2026-10-09");
    let rt = root(vec![founder, m1, m2]);
    let b = ballot(&u1, "b", O);
    let reg = Registry::build(&with(rt, vec![], vec![b.clone()], vec![vote(&u1, &b, "github:m1", O), vote(&u2, &b, "github:m1", O)]));
    assert_eq!(outcome(&reg, 0), BallotOutcome::Void(BallotProblem::FounderActive { founder: FOUNDER.into(), date: O.into() }));
    assert_eq!(reg.active_founder_on(O), Some(FOUNDER));
    assert_eq!(reg.current_founders(), vec![FOUNDER.to_string()]);

    let (rt, f, m) = electorate(2);
    let b = ballot(&m[0], "b", O);
    let later = "2026-12-10";
    let b2 = ballot(&m[1], "b2", later);
    let mut unsigned = FounderBallot { id: "b3".into(), opened: O.into(), signer: None, signature: None };
    let dup = ballot(&m[1], "b", O);
    let by_founder = ballot(&f, "b4", O);
    unsigned.signer = None;
    let votes = vec![
        vote(&m[0], &b, "github:m1", O),
        vote(&m[1], &b, "github:m1", O),
        vote(&m[0], &b2, "github:m2", later),
        vote(&m[1], &b2, "github:m2", later),
    ];
    let mut rt2 = with(rt.clone(), vec![], vec![b.clone(), b2, unsigned, dup.clone(), by_founder], votes);
    let mut dv = vote(&m[0], &dup, "github:m2", O);
    dv.ballot = "b3".into();
    rt2.code_review.as_mut().unwrap().founder_votes.push(dv);
    let reg = Registry::build(&rt2);
    assert_eq!(outcome(&reg, 0), elected("github:m1", O, None));
    assert_eq!(outcome(&reg, 1), BallotOutcome::Void(BallotProblem::FounderActive { founder: "github:m1".into(), date: later.into() }));
    assert_eq!(outcome(&reg, 2), BallotOutcome::Void(BallotProblem::Opener(SignerProblem::Unsigned)));
    assert_eq!(outcome(&reg, 3), BallotOutcome::Void(BallotProblem::DuplicateId));
    assert_eq!(outcome(&reg, 4), BallotOutcome::Void(BallotProblem::Opener(SignerProblem::NotEligible(FOUNDER.into()))));
    assert_eq!(reg.lineage.vote_problems.last().map(|(_, _, p)| p.clone()), Some(VoteProblem::BallotVoid));
    // F keeps founder standing (only the key is retired); one founder per
    // ballot: m1.
    assert_eq!(reg.current_founders(), vec![FOUNDER.to_string(), "github:m1".to_string()], "one founder per ballot");

    // The founder's key comes back before the decision: void.
    let (mut rt3, f3, m) = electorate(4);
    f3.unretire(&mut rt3.reviewers[0].keys[0], "2026-12-03").unwrap();
    let b = ballot(&m[0], "b", O);
    let votes = vec![vote(&m[0], &b, "github:m1", O), vote(&m[1], &b, "github:m1", O), vote(&m[2], &b, "github:m1", "2026-12-05")];
    let reg = Registry::build(&with(rt3, vec![], vec![b], votes));
    assert_eq!(
        outcome(&reg, 0),
        BallotOutcome::Void(BallotProblem::FounderActive { founder: FOUNDER.into(), date: "2026-12-05".into() })
    );
    let _ = (f, rt);
}

/// `add_days` without `chrono` (2026-11-05: chrono stays desktop-only, so
/// the founder ballots compile for wasm), checked against `chrono` itself.
///
/// Methodology: for every day from 1999-01-01 to 2101-12-31 (leap years,
/// the 2000 and 2100 century rules, every month end), `add_days(d, n)` for
/// n in {0, 1, 14, 30, 365, 366, 10_000} must equal chrono's
/// `NaiveDate::checked_add_days`. Then the inputs that must be refused: a
/// date that does not exist, malformed text, and a result past year 9999.
///
/// Result (2026-11-05): passes, 37 620 days x 7 offsets, all equal.
#[test]
fn add_days_matches_chrono_and_refuses_bad_dates() {
    use chrono::{Days, NaiveDate};
    let mut d = NaiveDate::from_ymd_opt(1999, 1, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(2101, 12, 31).unwrap();
    let mut checked = 0;
    while d <= end {
        let text = d.format("%Y-%m-%d").to_string();
        for n in [0u64, 1, 14, 30, 365, 366, 10_000] {
            let expected = d.checked_add_days(Days::new(n)).unwrap().format("%Y-%m-%d").to_string();
            assert_eq!(add_days(&text, n).as_deref(), Some(expected.as_str()), "{text} + {n}");
        }
        checked += 1;
        d = d.succ_opt().unwrap();
    }
    assert_eq!(checked, 37_620); // 103 years x 365 + 25 leap days
    for bad in ["2026-02-30", "2027-02-29", "2026-04-31", "2026-13-01", "2026-00-10", "26-01-01", "2026/01/01", ""] {
        assert_eq!(add_days(bad, 1), None, "{bad}");
    }
    assert_eq!(add_days("2028-02-28", 1).as_deref(), Some("2028-02-29"));
    assert_eq!(add_days("9999-12-31", 1), None);
}
