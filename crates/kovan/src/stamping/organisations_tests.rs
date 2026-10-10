//! End-to-end tests of rung 5, IV&V, from desktop kovan's side (GitHub
//! #810) on a throwaway git repository in a temporary folder, with a
//! temporary keystore and test passphrases. Nothing touches the real
//! keystore, `~/.config/kovan` or any `review.md` / `kovan_root.toml` of the
//! real workspace.

use std::collections::{BTreeMap, BTreeSet};

use kovan_common::review::engine::ConceptAreas;
use kovan_common::review::index::FolderIndex;
use kovan_common::review::ivv::{AttestationProblem, Rung5Miss};
use kovan_common::review::root::{
    Qualification, QualificationBasis, QualificationRecord, SeparationAttestation,
};
use kovan_common::review::root_append::{append_separation_attestation, unadmitted_reviewer};
use kovan_common::review::signing::keystore::{generate, Keystore, UnlockedKey};
use kovan_common::review::signing::registry::SignerProblem;
use kovan_common::review::state::StateKind;

use super::panel::{Action, OrgPanel};
use crate::stamping::concepts::ConceptChoice;
use super::*;
use crate::stamping::tests::{clean, founder_key, today, Repo, BY, DIR, LIB, PASS};
use crate::stamping::{
    draft_stamp, evaluate_workspace, evaluate_workspace_with, register_reviewer, write_review,
    StampRequest, REVIEW_MD,
};

/// The independent reviewer. ~~Its id sorts after [`BY`]'s: two reviews on
/// the same date are ordered by reviewer id (the engine has no time of
/// day), and the first must be the maintainer's.~~ **CORRECTED 2026-10-10**
/// (#770): two reviews of one date are ordered by `signed_at`
/// (`kovan_common::review::ivv::earliest_first`), so the maintainer, who
/// signs first, is the first reviewer whatever the ids.
const V: &str = "github:verifier";
const DEV: &str = "Outram Park project";
const IVV: &str = "Example IV&V Ltd";
const URL: &str = "https://github.com/theodoreOnzGit/outram-park-backend/issues/810";
const AREA: &str = "concept:thermal-hydraulics/natural-circulation";
const TEST_FILE: &str = "crates/demo/tests/analytic.rs";

/// `twice`'s `fn:` id.
fn twice_id(r: &Repo) -> String {
    let idx = FolderIndex::parse(&r.read(&format!("{DIR}/kovan.toml"))).unwrap();
    let id = idx
        .functions()
        .find(|(_, f)| f.qual == "twice")
        .unwrap()
        .1
        .id
        .clone();
    id
}

/// A hand-written analytical test reaching `twice`, committed by a human
/// (no agent trailer), and the index listing it in `reached_by`.
fn add_reaching_test(r: &Repo) {
    r.write(
        TEST_FILE,
        "/// twice(1) = 3, by hand.\n#[test]\nfn twice_is_three() {}\n",
    );
    let p = format!("{DIR}/kovan.toml");
    let mut idx = FolderIndex::parse(&r.read(&p)).unwrap();
    for m in idx.modules.values_mut() {
        for f in &mut m.functions {
            if f.qual == "twice" {
                f.reached_by = vec![format!("{TEST_FILE}::twice_is_three")];
            }
        }
    }
    r.write(&p, &idx.to_toml().unwrap());
    r.commit("add the hand-written analytical case for twice");
}

/// The independent reviewer: key `v1` in the same temporary keystore,
/// scoped to the crate, qualified in `area` (with evidence), admitted by
/// the maintainer, registered and committed.
fn register_verifier(
    r: &Repo,
    store: &std::path::Path,
    mk: &UnlockedKey,
    area: &str,
) -> UnlockedKey {
    let ks = Keystore::at(store);
    let (vf, vk) = generate(V, "v1", &today(), PASS).unwrap();
    ks.save(&vf).unwrap();
    let mut v = unadmitted_reviewer(V, Some("Verifier"), vf.reviewer_key());
    v.scope = vec!["crates/demo/**".into()];
    v.admitted = Some(today());
    mk.admit(&mut v).unwrap();
    v.qualification = vec![Qualification::Record(QualificationRecord {
        area: area.into(),
        basis: QualificationBasis::Degree,
        evidence: vec!["https://doi.org/10.1/thesis".into()],
        endorsed_by: None,
        self_declared: false,
    })];
    register_reviewer(r.path(), &v).unwrap();
    r.commit("admit the verifier");
    vk
}

/// The verifier's rung-4 answers: an analytical case written and verified
/// by hand, independence someone else.
fn vv_answers() -> BTreeMap<String, String> {
    let mut a = clean();
    a.insert("vv_evidence".into(), "analytical_case".into());
    a.insert("vv_case_author".into(), "human_wrote_and_verified".into());
    a
}

/// Draft, sign (as the verifier) and write the verifier's stamp of `twice`
/// naming `attestation`, then commit.
fn verifier_stamps(r: &Repo, vk: &UnlockedKey, attestation: Option<&str>) {
    let req = StampRequest {
        function: format!("{LIB}::twice"),
        by: V.into(),
        checklist: vv_answers(),
        separation_attestation: attestation.map(str::to_string),
        ..StampRequest::default()
    };
    let mut d = draft_stamp(r.path(), &req).unwrap();
    assert_eq!(d.entry.review.rung, 4, "the draft derives rung 4");
    assert_eq!(
        d.entry.review.separation_attestation.as_deref(),
        attestation
    );
    vk.sign_review(&mut d.entry).unwrap();
    write_review(r.path(), &d.entry, "Independent analytical check.").unwrap();
    r.commit("verifier stamps twice");
}

/// The verifier's first stamp, through the stamp dialog's state machine
/// (key picker, prepare, wizard, the attestation picker, sign on a worker):
/// the picker offers exactly `id` and starts on none; the review names it.
/// With `concept`, the concept finder links it (found by its title); else
/// the no-concept reason "plumbing" is chosen (a stamp needs one or the
/// other since 2026-10-10, #740 U3).
fn verifier_stamps_via_dialog(r: &Repo, store: &std::path::Path, id: &str, concept: Option<&ConceptChoice>) {
    use crate::stamping::flow::{Purpose, StampFlow, Step, Target};
    let target = Target {
        function: format!("{LIB}::twice"),
        name: "twice".into(),
    };
    let mut f = StampFlow::new(
        r.path().to_path_buf(),
        Keystore::at(store),
        Purpose::Stamp,
        target,
    );
    f.wait();
    assert_eq!(f.step, Step::PickKey);
    f.chosen = f.keys.iter().position(|k| k.reviewer == V).unwrap();
    f.use_key();
    f.wait();
    assert_eq!(f.step, Step::Wizard, "{:?}", f.error);
    let w = f.wizard.as_mut().unwrap();
    let offered: Vec<&str> = w.attestations.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(offered, [id]);
    assert_eq!(w.separation_attestation, None, "none by default");
    for (q, a) in vv_answers() {
        w.options.insert(q, a);
    }
    w.separation_attestation = Some(id.into());
    match concept {
        Some(c) => {
            w.concept_query = c.title.clone();
            assert!(
                w.concept_matches().iter().any(|m| m.id == c.id),
                "the finder finds {}",
                c.id
            );
            w.choose_concept(&c.id);
        }
        None => w.choose_reason(Some("plumbing")),
    }
    w.passphrase = PASS.into();
    assert!(f.sign(), "{:?}", f.wizard.as_ref().map(|w| w.gate()));
    f.wait();
    assert!(
        matches!(f.step, Step::Done(_)),
        "{:?} {:?}",
        f.step,
        f.error
    );
    r.commit("verifier stamps twice (dialog)");
}

/// The verifier's misses on `twice` with concept areas known.
fn misses_with_areas(r: &Repo) -> (Option<u8>, Vec<Rung5Miss>) {
    let areas: ConceptAreas = [(twice_id(r), BTreeSet::from([AREA.to_string()]))].into();
    let we = evaluate_workspace_with(r.path(), &areas).unwrap();
    let f = &we.evaluation.functions[&twice_id(r)];
    let m = f
        .independent_vv
        .candidates
        .iter()
        .find(|c| c.by == V)
        .map(|c| c.misses.clone())
        .unwrap_or_default();
    (f.rung, m)
}

/// Methodology: on the stamping fixture (`tests.rs`), the founder `BY`
/// (maintainer, key `k1`) and an independent reviewer `V` (key `v1`,
/// admitted, scoped, qualified in thermal-hydraulics), both in one
/// temporary keystore with the test passphrase. A hand-written analytical
/// test reaching `twice` is committed by a human. Then, through this
/// module's functions and the panel's state machine (workers, passphrase
/// unlock): the maintainer signs the workspace developing organisation
/// and `V`'s organisation; `V` signs a separation attestation with a GitHub
/// issue audit record. The maintainer stamps `twice` first (rung 3), then
/// `V` stamps it as an analytical V&V case (rung 4) naming the attestation
/// (v3 signature). Every write is committed.
///
/// Pass:
/// 1. desktop and web (`stamp_states`; ~~which passes NO concept areas:
///    nothing in kovan resolves them yet~~ **CORRECTED 2026-10-10**: it
///    resolves them from the reviews' concept links, and no review here
///    links one) show `twice` valid at rung 4,
///    with exactly ONE reason `V` misses rung 5, "the function has no known
///    concept area", the audit record as written, and the "independent
///    V&V not counted" flag (the review claims IV&V and misses);
/// 2. the same workspace judged WITH `twice`'s concept area known
///    (`evaluate_workspace_with`) is at rung 5, by `V`;
/// 3. re-stamped naming no attestation: the miss is "no attestation" and no
///    flag; naming an unsigned attestation (appended as text, committed):
///    the miss is "not signed"; naming one `V` does not have: the draft is
///    refused; the stamp dialog's choices hold only the signed one.
///
/// Result (2026-10-10): passes. ~~Rung 5 is unreachable in desktop kovan
/// and kovan-web today because concept areas are not resolved there~~
/// **CORRECTED 2026-10-10**: reachable once a review links a concept
/// (`concept_link_reaches_rung_5_through_evaluate_workspace`, below).
#[test]
fn ivv_end_to_end_signs_records_stamps_and_judges() {
    let r = Repo::new();
    let store = tempfile::tempdir().unwrap();
    let mk = founder_key(&r, store.path());
    add_reaching_test(&r);
    let vk = register_verifier(&r, store.path(), &mk, "concept:thermal-hydraulics");

    // The maintainer's records, through the library.
    let refused = add_developing_organisation(r.path(), &vk, DEV, None, &today()).unwrap_err();
    assert!(refused.contains("not a maintainer"), "{refused}");
    add_developing_organisation(r.path(), &mk, DEV, None, &today()).unwrap();
    add_reviewer_organisation(r.path(), &mk, V, IVV, &today()).unwrap();
    assert!(r.read(ROOT_FILE).starts_with("# The workspace root"));

    // The attestation, through the panel (workers, passphrase).
    let mut p = OrgPanel::new(r.path().to_path_buf(), Keystore::at(store.path()));
    p.wait();
    let o = p.overview.clone().unwrap().unwrap();
    assert_eq!((o.developing.len(), o.reviewer_organisations.len()), (1, 1));
    assert!(o.warnings.is_empty(), "{:?}", o.warnings);
    p.chosen = p.keys.iter().position(|k| k.reviewer == V).unwrap();
    p.forms.attestation = Default::default();
    p.prefill_attestation();
    assert!(!p.is_maintainer());
    assert_eq!(p.forms.attestation.organisation, IVV);
    assert_eq!(p.forms.attestation.developing_organisation, DEV);
    let id = p.forms.attestation.id.clone();
    assert_eq!(id, format!("sep-{}", today()));
    p.forms.attestation.audit_record = "https://github.com/o/r/pull/1".into();
    p.forms.passphrase = PASS.into();
    assert!(p
        .problems(Action::Attestation)
        .iter()
        .any(|e| e.contains("GitHub issue")));
    assert!(p
        .problems(Action::DevelopingOrganisation)
        .iter()
        .any(|e| e.contains("Only a maintainer")));
    assert!(!p.submit(Action::Attestation));
    p.forms.attestation.audit_record = URL.into();
    p.forms.passphrase = "wrong passphrase".into();
    assert!(p.submit(Action::Attestation));
    p.wait();
    assert!(p.error.as_deref().unwrap().contains("Wrong passphrase"));
    assert_eq!(p.forms.attestation.id, id, "kept after a failure");
    p.forms.passphrase = PASS.into();
    assert!(p.submit(Action::Attestation));
    p.wait();
    assert_eq!(p.error, None);
    assert!(p.notices.last().unwrap().contains(&id));
    let o = p.overview.clone().unwrap().unwrap();
    assert_eq!(o.attestations.len(), 1);
    assert_eq!(o.attestations[0].audit_record.as_deref(), Some(URL));
    assert!(o.warnings.is_empty(), "{:?}", o.warnings);
    assert_eq!(
        p.forms.attestation.id,
        format!("sep-{}-2", today()),
        "a fresh id next"
    );
    r.commit("organisations and the attestation");
    let choices = attestation_choices_in(r.path(), V);
    assert_eq!(
        choices.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        [id.as_str()]
    );
    assert!(attestation_choices_in(r.path(), BY).is_empty());

    // The stamps. `leaf` first: `twice` calls it, and stamping is bottom-up
    // (#770, #740: a callee without a valid stamp blocks its caller).
    r.stamp(&mk, "leaf", "Leaf first, bottom-up.");
    r.stamp(&mk, "twice", "First review.");
    verifier_stamps_via_dialog(&r, store.path(), &id, None);
    assert!(r
        .read(&format!("{DIR}/{REVIEW_MD}"))
        .contains(&format!("separation_attestation = \"{id}\"")));

    // 1. What desktop and web show (no concept areas).
    let s = r.state("twice").unwrap();
    assert_eq!(
        (s.state, s.rung),
        (Some(StateKind::Valid), 4),
        "{}",
        s.reason
    );
    let v = s.ivv.as_ref().expect("the IV&V summary is carried");
    assert!(!v.passed());
    let c = v.candidates.iter().find(|c| c.reviewer == V).unwrap();
    assert_eq!(
        c.reasons,
        vec!["the function has no known concept area".to_string()]
    );
    assert_eq!(c.attestation.as_deref(), Some(id.as_str()));
    assert_eq!(c.audit_record.as_deref(), Some(URL));
    assert_eq!(v.not_counted.len(), 1, "{:?}", v.not_counted);
    assert!(v.warnings.is_empty(), "{:?}", v.warnings);
    let by_m = v.candidates.iter().find(|c| c.reviewer == BY).unwrap();
    assert!(by_m.reasons.iter().any(|x| x.contains("first review")));
    let q = kovan_common::review::ivv_view::ivv_queue(
        &evaluate_workspace(r.path()).unwrap().evaluation,
    );
    assert_eq!(q.len(), 1, "{q:?}");
    assert_eq!(q[0].reviewer.as_deref(), Some(V));
    // The desktop need-you queue (#771) words the same miss on its
    // `IndependentVvNotCounted` row, with the next step.
    let ev = evaluate_workspace(r.path()).unwrap().evaluation;
    let lines: Vec<String> = ev
        .functions
        .values()
        .flat_map(|f| f.flags.iter())
        .flat_map(crate::stamping::queue::flag_detail)
        .collect();
    assert!(
        lines.iter().any(|l| l.contains("the function has no known concept area") && l.contains("Next: ")),
        "{lines:?}"
    );

    // `kovan-cli review ivv` prints the same, read-only.
    let cli = crate::commands::review_ivv::render(r.path(), Some("twice")).unwrap();
    for want in [
        "IV&V (rung 5): independent V&V not counted",
        "  - the function has no known concept area",
        &format!("audit record (not verified by kovan): {URL}"),
        &format!("names attestation {id}"),
    ] {
        assert!(cli.contains(want), "{want:?} not in\n{cli}");
    }
    let all = crate::commands::review_ivv::render(r.path(), None).unwrap();
    assert!(all.contains("Needs a person (rung 5):"), "{all}");
    assert!(
        crate::commands::review_ivv::render(r.path(), Some("nothing-like-this"))
            .unwrap()
            .contains("No reviewed function matches.")
    );

    // 2. With the concept area known: rung 5.
    assert_eq!(misses_with_areas(&r), (Some(5), vec![]));

    // 3. Misses.
    verifier_stamps(&r, &vk, None);
    assert_eq!(
        misses_with_areas(&r),
        (Some(4), vec![Rung5Miss::NoAttestation])
    );
    assert!(r
        .state("twice")
        .unwrap()
        .ivv
        .unwrap()
        .not_counted
        .is_empty());
    let unsigned = SeparationAttestation {
        id: "sep-unsigned".into(),
        organisation: IVV.into(),
        developing_organisation: DEV.into(),
        date: today(),
        audit_record: Some(URL.into()),
        key: None,
        signature: None,
    };
    let t = append_separation_attestation(&r.read(ROOT_FILE), V, &unsigned).unwrap();
    r.write(ROOT_FILE, &t);
    r.commit("an unsigned attestation");
    assert_eq!(
        attestation_choices_in(r.path(), V).len(),
        1,
        "never offered"
    );
    verifier_stamps(&r, &vk, Some("sep-unsigned"));
    assert_eq!(
        misses_with_areas(&r),
        (
            Some(4),
            vec![Rung5Miss::Attestation(AttestationProblem::Unverified(
                SignerProblem::Unsigned
            ))]
        )
    );
    let s = r.state("twice").unwrap();
    let v = s.ivv.unwrap();
    assert!(
        v.warnings.iter().any(|w| w.contains("it is not signed")),
        "{:?}",
        v.warnings
    );
    let req = StampRequest {
        function: format!("{LIB}::twice"),
        by: V.into(),
        checklist: vv_answers(),
        separation_attestation: Some("sep-nope".into()),
        ..StampRequest::default()
    };
    assert!(draft_stamp(r.path(), &req)
        .unwrap_err()
        .contains("no separation attestation"));
}

/// Methodology (#770, #810; the brief's end-to-end): on the stamping
/// fixture, the founder `BY` and an independent reviewer `V` qualified in
/// a level-1 node of the standard concept tree (an IAEA issue, with
/// evidence). A hand-written analytical test reaches `twice`. The
/// maintainer signs the developing organisation and `V`'s organisation; `V`
/// signs a separation attestation with a GitHub issue audit record (all
/// through the library), committed. The maintainer stamps `leaf` and
/// `twice` (rung 3); `V` stamps `twice` through the stamp dialog, naming
/// the attestation and **linking a level-3 concept under that issue with
/// the concept finder**; committed.
///
/// Pass: the review carries the `implements` relation; the workspace judged
/// through `evaluate_workspace` (no `_with`: the concept areas are
/// resolved from the review) has `twice` at **rung 5** by `V` with no miss;
/// desktop and web (`stamp_states`) show rung 5 and the IV&V summary
/// passed; `kovan-cli review ivv` no longer says "no known concept area".
/// Then the same review with the concept unlinked (re-stamped with a
/// no-concept reason) falls back to rung 4 with exactly that miss.
///
/// Result (2026-10-10): passes.
#[test]
fn concept_link_reaches_rung_5_through_evaluate_workspace() {
    let concept = crate::stamping::concepts::standard_concepts()
        .into_iter()
        .find(|c| c.level == 3)
        .unwrap();
    let issue = concept.path.split('/').next().unwrap().to_string();
    let r = Repo::new();
    let store = tempfile::tempdir().unwrap();
    let mk = founder_key(&r, store.path());
    add_reaching_test(&r);
    let vk = register_verifier(&r, store.path(), &mk, &format!("concept:{issue}"));
    add_developing_organisation(r.path(), &mk, DEV, None, &today()).unwrap();
    add_reviewer_organisation(r.path(), &mk, V, IVV, &today()).unwrap();
    let id = format!("sep-{}", today());
    add_separation_attestation(
        r.path(),
        &vk,
        &AttestationInput {
            id: id.clone(),
            organisation: IVV.into(),
            developing_organisation: DEV.into(),
            audit_record: URL.into(),
            date: today(),
        },
    )
    .unwrap();
    r.commit("organisations and the attestation");
    r.stamp(&mk, "leaf", "Leaf first, bottom-up.");
    r.stamp(&mk, "twice", "First review.");
    verifier_stamps_via_dialog(&r, store.path(), &id, Some(&concept));
    let md = r.read(&format!("{DIR}/{REVIEW_MD}"));
    assert!(
        md.contains("kind = \"implements\"") && md.contains(&concept.id),
        "{md}"
    );

    let we = evaluate_workspace(r.path()).unwrap();
    let f = &we.evaluation.functions[&twice_id(&r)];
    let by_v = f.independent_vv.candidates.iter().find(|c| c.by == V).unwrap();
    assert_eq!(by_v.misses, vec![], "{:?}", f.independent_vv);
    assert_eq!(f.rung, Some(5), "{:?}", f.independent_vv);
    let s = r.state("twice").unwrap();
    assert_eq!((s.state, s.rung), (Some(StateKind::Valid), 5), "{}", s.reason);
    assert!(s.ivv.as_ref().unwrap().passed());
    let cli = crate::commands::review_ivv::render(r.path(), Some("twice")).unwrap();
    assert!(!cli.contains("  - the function has no known concept area"), "{cli}");

    // Unlinked: back to rung 4, with exactly the area miss.
    verifier_stamps(&r, &vk, Some(&id));
    let we = evaluate_workspace(r.path()).unwrap();
    let f = &we.evaluation.functions[&twice_id(&r)];
    let by_v = f.independent_vv.candidates.iter().find(|c| c.by == V).unwrap();
    assert_eq!(f.rung, Some(4));
    assert_eq!(by_v.misses, vec![Rung5Miss::NoConceptArea]);
}

/// Methodology: the pure helpers. `check_audit_record` accepts a GitHub
/// issue URL (open or closed: kovan cannot tell, and does not fetch) and
/// refuses a pull request, another host and an empty string with a reason
/// in words; `next_attestation_id` skips ids in use; a missing root gives
/// an error, not a panic, and no choices.
///
/// Result (2026-10-10): passes.
#[test]
fn audit_record_ids_and_missing_root() {
    assert!(check_audit_record(URL).is_ok());
    assert!(check_audit_record(&format!(" {URL} ")).is_ok());
    for bad in [
        "https://github.com/o/r/pull/1",
        "https://gitlab.com/o/r/issues/1",
        "",
    ] {
        let e = check_audit_record(bad).unwrap_err();
        assert!(
            e.contains("GitHub issue") && !e.contains("NotAnIssueUrl"),
            "{e}"
        );
    }
    let mut root = kovan_common::review::root::ReviewRoot::default();
    assert_eq!(
        next_attestation_id(&root, V, "2026-10-10"),
        "sep-2026-10-10"
    );
    let mut v = unadmitted_reviewer(
        V,
        None,
        generate(V, "v1", "2026-10-10", PASS)
            .unwrap()
            .0
            .reviewer_key(),
    );
    for id in ["sep-2026-10-10", "sep-2026-10-10-2"] {
        v.separations.push(SeparationAttestation {
            id: id.into(),
            organisation: IVV.into(),
            developing_organisation: DEV.into(),
            date: "2026-10-10".into(),
            audit_record: None,
            key: None,
            signature: None,
        });
    }
    root.reviewers.push(v);
    assert_eq!(
        next_attestation_id(&root, V, "2026-10-10"),
        "sep-2026-10-10-3"
    );
    let d = tempfile::tempdir().unwrap();
    assert!(overview(d.path()).unwrap_err().contains("Index fresh"));
    assert!(attestation_choices_in(d.path(), V).is_empty());
    let mut p = OrgPanel::new(d.path().to_path_buf(), Keystore::at(d.path().join("keys")));
    p.wait();
    assert!(matches!(&p.overview, Some(Err(e)) if e.contains("kovan_root.toml")));
    assert!(p.key().is_none() && !p.is_maintainer());
    assert!(!p.submit(Action::ReviewerOrganisation));
}
