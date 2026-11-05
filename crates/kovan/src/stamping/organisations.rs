//! **Organisations and separation attestations from desktop kovan**
//! (GitHub #810, follow-up of #809): the GUI-free layer of the
//! organisation sections of Code Review ▸ User management.
//! ~~The drawing is `app/organisations_view.rs`'s "Organisations & IV&V"
//! window, with its own state machine (`organisations_flow.rs`).~~
//! **CORRECTED 2026-11-04** (GitHub #1211, maintainer: "move the IV&V
//! window into the User management tab"): the window and its state machine
//! are gone. The forms are collapsing sections of the User management tab,
//! signed through its panel ([`super::user_management::UserPanel`], one key,
//! one passphrase, one worker); the drawing is
//! `app/organisations_view.rs::sections`. This file decides what may be
//! signed and writes it.
//!
//! ```text
//!   the founder, key unlocked ──> add_developing_organisation ─┐
//!                             ├─> add_reviewer_organisation ───┤  sign, then append
//!                             └─> retire_organisation ─────────┤  as text (comments
//!   a reviewer, key unlocked ───> add_separation_attestation ──┤  kept, read back:
//!                                  (audit record: a GitHub     │  root_append)
//!                                   issue URL, form only)      v
//!                                                         kovan_root.toml
//!   overview(root) ──> what is recorded (retired marked), and every record that does not verify
//!   attestation_choices_in(root, by) ──> the stamp dialog's picker
//! ```
//!
//! **Who signs what.** ~~Developing-organisation and reviewer-organisation
//! records are signed by an admitted maintainer~~ **CORRECTED 2026-11-04**
//! (GitHub #1211, maintainer direction): kovan writes them, and their
//! retirements, signed by the **founder** only (`[code_review] founder`,
//! [`super::user_management::founder_signer`]; since GitHub #1248 any
//! founder in standing on the record's date); a separation attestation
//! is still signed by the independent reviewer alone (maintainer:
//! "agree"), naming a public GitHub issue as its audit record. Kovan checks
//! the URL's form and never fetches it (offline; Leak Before Break: it is
//! shown "audit record (not verified by kovan)").
//!
//! **Records written before #1211 by another maintainer** are read as
//! before: `kovan_common::review::ivv` still verifies a record signed by
//! any admitted maintainer, so they keep counting. They are not silently
//! invalidated; [`overview`] marks each one ([`RecordRow::note`]) so the
//! maintainer can decide (retire it and sign a new one, or keep it).
//!
//! **Removal is retire** (#1211): [`retire_organisation`] appends a
//! founder-signed `[[code_review.organisation_retirement]]`; the record
//! stays in its list, shown retired, and no longer counts from that date.
//!
//! **Refused before anything is written**: an organisation record or a
//! retirement signed by anyone but the founder (with the reason), an
//! attestation by a reviewer the root does not list, an attestation id the
//! reviewer already uses, a malformed date or audit record, a retirement
//! of an unsigned, unknown or already retired record. Every write appends;
//! nothing is edited (the records are append-only, #809).
//!
//! **AI agents never sign**: every function here takes an [`UnlockedKey`],
//! which only its human owner's passphrase unlocks. `kovan-cli` has no
//! signing command.

use std::path::{Path, PathBuf};

use kovan_common::review::engine::SignaturePolicy;
use kovan_common::review::ivv::{parse_audit_record, record_warnings, retirements_of};
use kovan_common::review::ivv_text::{audit_record_problem, record_warning, retirement_problem};
use kovan_common::review::ivv_view::{attestation_choices, AttestationChoice};
use kovan_common::review::root::{
    DevelopingOrganisation, KeySigner, OrganisationList, OrganisationRetirement, ReviewRoot,
    ReviewerOrganisation, Role, SeparationAttestation,
};
use kovan_common::review::root_append::{
    append_developing_organisation, append_organisation_retirement, append_reviewer_organisation,
    append_separation_attestation,
};
use kovan_common::review::signing::keystore::UnlockedKey;
use kovan_common::review::signing::registry::Registry;

use super::user_management::founder_signer;
use super::ROOT_FILE;

/// `<root>/kovan_root.toml`: its path, text and parse.
fn read(root: &Path) -> Result<(PathBuf, String, ReviewRoot), String> {
    let path = root.join(ROOT_FILE);
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("{}: {e} (run Index fresh first)", path.display()))?;
    let parsed = ReviewRoot::parse(&text).map_err(|e| format!("{ROOT_FILE}: {e}"))?;
    Ok((path, text, parsed))
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

/// ~~The key's reviewer must be a maintainer in `root`.~~ **CORRECTED
/// 2026-11-04** (#1211): the key must be the founder's, trusted and active
/// on `date`, while the founder is a maintainer.
fn require_founder(root: &ReviewRoot, key: &UnlockedKey, date: &str) -> Result<(), String> {
    founder_signer(&Registry::build(root), (key.reviewer(), key.key_id()), date)
        .map_err(|e| format!("{e} Organisation records are signed by the founder only."))
}

/// An organisation name as typed: trimmed, not empty.
fn name(what: &str, s: &str) -> Result<String, String> {
    let t = s.trim();
    if t.is_empty() {
        Err(format!("Enter the {what}."))
    } else {
        Ok(t.to_string())
    }
}

/// `Ok` when `url` is a GitHub issue URL (form only; never fetched), else
/// why not, in words.
pub fn check_audit_record(url: &str) -> Result<(), String> {
    parse_audit_record(url.trim()).map(|_| ()).map_err(|p| {
        format!(
            "The audit record must be a public GitHub issue, \
             https://github.com/<owner>/<repo>/issues/<number>: {}.",
            audit_record_problem(&p)
        )
    })
}

/// Sign, as the founder, a developing-organisation record (`krate`: a
/// per-crate override, else workspace-wide) in force from `date`, and
/// append it to `<root>/kovan_root.toml` (module doc).
pub fn add_developing_organisation(
    root: &Path,
    key: &UnlockedKey,
    organisation: &str,
    krate: Option<&str>,
    date: &str,
) -> Result<DevelopingOrganisation, String> {
    let (path, text, parsed) = read(root)?;
    require_founder(&parsed, key, date)?;
    let mut e = DevelopingOrganisation {
        krate: krate
            .map(str::trim)
            .filter(|k| !k.is_empty())
            .map(str::to_string),
        name: name("developing organisation", organisation)?,
        date: date.to_string(),
        signer: None,
        signature: None,
    };
    key.sign_developing_organisation(&mut e)
        .map_err(|x| x.to_string())?;
    let new = append_developing_organisation(&text, &e).map_err(|x| x.to_string())?;
    write(&path, &new)?;
    Ok(e)
}

/// Sign, as the founder, `reviewer`'s organisation from `date`, and append
/// it (module doc).
pub fn add_reviewer_organisation(
    root: &Path,
    key: &UnlockedKey,
    reviewer: &str,
    organisation: &str,
    date: &str,
) -> Result<ReviewerOrganisation, String> {
    let (path, text, parsed) = read(root)?;
    require_founder(&parsed, key, date)?;
    let mut e = ReviewerOrganisation {
        name: name("reviewer's organisation", organisation)?,
        date: date.to_string(),
        signer: None,
        signature: None,
    };
    key.sign_reviewer_organisation(reviewer.trim(), &mut e)
        .map_err(|x| x.to_string())?;
    let new =
        append_reviewer_organisation(&text, reviewer.trim(), &e).map_err(|x| x.to_string())?;
    write(&path, &new)?;
    Ok(e)
}

/// Which organisation record a retirement names: its list, whose (for a
/// reviewer's organisation), its signature, and its name (for the words).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrgRecordRef {
    pub list: OrganisationList,
    pub reviewer: Option<String>,
    /// The record's `signature`, which names it.
    pub record: String,
    pub name: String,
}

/// What stops retiring `target` in `root` (signatures aside): it must be a
/// signed record in its list, not retired already by a retirement that
/// takes effect. Empty: it may be retired.
pub fn retire_problems(root: &ReviewRoot, target: &OrgRecordRef) -> Vec<String> {
    let named = |sig: Option<&String>| sig.is_some_and(|s| *s == target.record);
    let found = match (target.list, target.reviewer.as_deref()) {
        (OrganisationList::DevelopingOrganisation, None) => root
            .code_review
            .as_ref()
            .is_some_and(|c| c.developing_organisation.iter().any(|e| named(e.signature.as_ref()))),
        (OrganisationList::ReviewerOrganisation, Some(id)) => root
            .reviewer(id)
            .is_some_and(|r| r.organisations.iter().any(|e| named(e.signature.as_ref()))),
        _ => false,
    };
    if !found {
        return vec![format!("{ROOT_FILE} has no signed record {} to retire.", target.name)];
    }
    let reg = Registry::build(root);
    let done = retirements_of(
        root,
        &reg,
        target.list,
        target.reviewer.as_deref(),
        Some(&target.record),
        SignaturePolicy::Enforce,
    )
    .iter()
    .any(|(_, ok)| ok.is_ok());
    if done {
        vec![format!("{} is retired already.", target.name)]
    } else {
        Vec::new()
    }
}

/// Retire, as the founder, the organisation record `target` from `date`:
/// append a founder-signed `[[code_review.organisation_retirement]]`. The
/// record itself stays as it is (module doc).
pub fn retire_organisation(
    root: &Path,
    key: &UnlockedKey,
    target: &OrgRecordRef,
    date: &str,
) -> Result<OrganisationRetirement, String> {
    let (path, text, parsed) = read(root)?;
    require_founder(&parsed, key, date)?;
    let problems = retire_problems(&parsed, target);
    if !problems.is_empty() {
        return Err(problems.join(" "));
    }
    let mut r = OrganisationRetirement {
        list: target.list,
        reviewer: target.reviewer.clone(),
        record: target.record.clone(),
        date: date.to_string(),
        signer: None,
        signature: None,
    };
    key.sign_organisation_retirement(&mut r)
        .map_err(|x| x.to_string())?;
    let new = append_organisation_retirement(&text, &r).map_err(|x| x.to_string())?;
    write(&path, &new)?;
    Ok(r)
}

/// What a separation attestation states.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AttestationInput {
    /// Unique among the reviewer's attestations.
    pub id: String,
    /// The reviewer's organisation.
    pub organisation: String,
    /// The organisation responsible for developing the software.
    pub developing_organisation: String,
    /// A public GitHub issue URL.
    pub audit_record: String,
    /// `YYYY-MM-DD`.
    pub date: String,
}

/// What stops `reviewer` signing `input` against `root` (empty: it may).
pub fn attestation_problems(root: &ReviewRoot, reviewer: &str, input: &AttestationInput) -> Vec<String> {
    let mut p = Vec::new();
    match root.reviewer(reviewer) {
        None => p.push(format!("{ROOT_FILE} lists no reviewer {reviewer}: register your key first")),
        Some(r) if r.separations.iter().any(|a| a.id == input.id.trim()) => {
            p.push(format!("You have an attestation {} already: choose another id.", input.id.trim()))
        }
        Some(_) => {}
    }
    if input.id.trim().is_empty() {
        p.push("Enter an id for the attestation.".into());
    }
    if input.organisation.trim().is_empty() {
        p.push("Enter your organisation.".into());
    }
    if input.developing_organisation.trim().is_empty() {
        p.push("Enter the developing organisation.".into());
    }
    if let Err(e) = check_audit_record(&input.audit_record) {
        p.push(e);
    }
    p
}

/// Sign, as the key's own reviewer, a separation attestation and append it
/// to that reviewer's section (module doc).
pub fn add_separation_attestation(
    root: &Path,
    key: &UnlockedKey,
    input: &AttestationInput,
) -> Result<SeparationAttestation, String> {
    let (path, text, parsed) = read(root)?;
    if parsed.reviewer(key.reviewer()).is_none() {
        return Err(format!(
            "{ROOT_FILE} lists no reviewer {}: register your key first",
            key.reviewer()
        ));
    }
    check_audit_record(&input.audit_record)?;
    let id = input.id.trim();
    if id.is_empty() {
        return Err("Enter an id for the attestation.".into());
    }
    let mut a = SeparationAttestation {
        id: id.to_string(),
        organisation: name("your organisation", &input.organisation)?,
        developing_organisation: name("developing organisation", &input.developing_organisation)?,
        date: input.date.clone(),
        audit_record: Some(input.audit_record.trim().to_string()),
        key: None,
        signature: None,
    };
    key.attest_separation(&mut a).map_err(|x| x.to_string())?;
    let new =
        append_separation_attestation(&text, key.reviewer(), &a).map_err(|x| x.to_string())?;
    write(&path, &new)?;
    Ok(a)
}

/// The first of `sep-<date>`, `sep-<date>-2`, … that `reviewer` does not use.
pub fn next_attestation_id(root: &ReviewRoot, reviewer: &str, date: &str) -> String {
    let used: Vec<&str> = root
        .reviewer(reviewer)
        .map(|r| r.separations.iter().map(|a| a.id.as_str()).collect())
        .unwrap_or_default();
    let base = format!("sep-{date}");
    std::iter::once(base.clone())
        .chain((2..).map(|n| format!("{base}-{n}")))
        .find(|c| !used.contains(&c.as_str()))
        .unwrap_or(base)
}

/// The attestations `by` may name in a stamp (`kovan_common`'s
/// [`attestation_choices`], over `<root>/kovan_root.toml`). Empty when
/// there is no root.
pub fn attestation_choices_in(root: &Path, by: &str) -> Vec<AttestationChoice> {
    match read(root) {
        Ok((_, _, parsed)) => attestation_choices(&parsed, &Registry::build(&parsed), by),
        Err(_) => Vec::new(),
    }
}

/// One record as the sections list it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordRow {
    /// `workspace`, `crate <name>`, or the reviewer id.
    pub scope: String,
    pub name: String,
    pub date: String,
    /// Who signed it (`reviewer key`), or `unsigned`.
    pub signed_by: String,
    /// For an attestation: its id and the developing organisation named.
    pub detail: String,
    /// For an attestation: the audit record as written.
    pub audit_record: Option<String>,
    /// GitHub #1211: how it stands against the founder-only rule, when it
    /// is signed by a maintainer who is not a founder on its date (GitHub #1248) (it still counts,
    /// as before; the maintainer decides).
    pub note: Option<String>,
    /// GitHub #1211: "retired from <date> by <founder> (key <k>)" for each
    /// retirement that takes effect; a retirement that does not is in the
    /// warnings and here as "retirement … has no effect: …".
    pub retired: Vec<String>,
    /// Whether a retirement takes effect (the record no longer counts from
    /// its date).
    pub is_retired: bool,
    /// What a Retire button retires: a signed organisation record not yet
    /// retired. `None` for an attestation, an unsigned or a retired record.
    pub retire: Option<OrgRecordRef>,
}

/// What `kovan_root.toml` records for IV&V, and what does not verify.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Overview {
    pub developing: Vec<RecordRow>,
    pub reviewer_organisations: Vec<RecordRow>,
    pub attestations: Vec<RecordRow>,
    /// Every record that does not verify, in words.
    pub warnings: Vec<String>,
    /// Every reviewer id with its role.
    pub reviewers: Vec<(String, Role)>,
    /// The declared founder, if any (who signs organisation records).
    /// ~~The~~ GitHub #1248: the first declared founder; [`Self::founders`]
    /// are the ones who sign now.
    pub founder: Option<String>,
    /// GitHub #1248: the founders in standing now (any of them signs
    /// organisation records).
    pub founders: Vec<String>,
}

/// Read `<root>/kovan_root.toml` into an [`Overview`], signatures checked.
pub fn overview(root: &Path) -> Result<Overview, String> {
    let (_, _, r) = read(root)?;
    Ok(overview_of(&r))
}

/// `signed_by` and `note` of an organisation record dated `date`: the note
/// when its signer was not a founder then (GitHub #1248: ~~not the
/// founder~~ any founder in standing on its date).
fn signer_words(s: Option<&KeySigner>, sig: Option<&String>, reg: &Registry, date: &str) -> (String, Option<String>) {
    match (s, sig) {
        (Some(s), Some(_)) => {
            let who = s.reviewer.as_deref().unwrap_or("(unnamed)");
            let note = (!reg.is_founder_on(who, date)).then(|| {
                format!(
                    "signed by {who}, not a founder on {date}: still counts as before #1211 (kovan \
                     writes new records founder-signed only); the maintainer decides whether to retire it"
                )
            });
            (format!("{who} {}", s.key), note)
        }
        _ => ("unsigned".into(), None),
    }
}

/// The retirement lines of a record, whether one takes effect, and what a
/// Retire button would retire.
fn retirement_words(
    r: &ReviewRoot,
    reg: &Registry,
    list: OrganisationList,
    reviewer: Option<&str>,
    signature: Option<&String>,
    name: &str,
) -> (Vec<String>, bool, Option<OrgRecordRef>) {
    let found = retirements_of(r, reg, list, reviewer, signature.map(String::as_str), SignaturePolicy::Enforce);
    let lines = found
        .iter()
        .map(|(x, ok)| match ok {
            Ok(Some((who, key))) => format!("retired from {} by {who} (key {key}) \u{2714}", x.date),
            Ok(None) => format!("retired from {}", x.date),
            Err(p) => format!("retirement dated {} has no effect: {}", x.date, retirement_problem(p)),
        })
        .collect();
    let is_retired = found.iter().any(|(_, ok)| ok.is_ok());
    let retire = match (signature, is_retired) {
        (Some(sig), false) => Some(OrgRecordRef {
            list,
            reviewer: reviewer.map(str::to_string),
            record: sig.clone(),
            name: name.to_string(),
        }),
        _ => None,
    };
    (lines, is_retired, retire)
}

/// [`overview`] of a parsed root.
pub fn overview_of(r: &ReviewRoot) -> Overview {
    let reg = Registry::build(r);
    let founder = reg.founder.as_ref().ok().cloned();
    let mut o = Overview {
        warnings: record_warnings(r, &reg, SignaturePolicy::Enforce)
            .iter()
            .map(record_warning)
            .collect(),
        reviewers: r.reviewers.iter().map(|x| (x.id.clone(), x.role)).collect(),
        founder,
        founders: reg.current_founders(),
        ..Overview::default()
    };
    if let Some(cr) = &r.code_review {
        for e in &cr.developing_organisation {
            let (signed_by, note) = signer_words(e.signer.as_ref(), e.signature.as_ref(), &reg, &e.date);
            let (retired, is_retired, retire) = retirement_words(
                r,
                &reg,
                OrganisationList::DevelopingOrganisation,
                None,
                e.signature.as_ref(),
                &e.name,
            );
            o.developing.push(RecordRow {
                scope: e
                    .krate
                    .as_ref()
                    .map_or("workspace".into(), |k| format!("crate {k}")),
                name: e.name.clone(),
                date: e.date.clone(),
                signed_by,
                detail: String::new(),
                audit_record: None,
                note,
                retired,
                is_retired,
                retire,
            });
        }
    }
    for rv in &r.reviewers {
        for e in &rv.organisations {
            let (signed_by, note) = signer_words(e.signer.as_ref(), e.signature.as_ref(), &reg, &e.date);
            let (retired, is_retired, retire) = retirement_words(
                r,
                &reg,
                OrganisationList::ReviewerOrganisation,
                Some(&rv.id),
                e.signature.as_ref(),
                &e.name,
            );
            o.reviewer_organisations.push(RecordRow {
                scope: rv.id.clone(),
                name: e.name.clone(),
                date: e.date.clone(),
                signed_by,
                detail: String::new(),
                audit_record: None,
                note,
                retired,
                is_retired,
                retire,
            });
        }
        for a in &rv.separations {
            o.attestations.push(RecordRow {
                scope: rv.id.clone(),
                name: a.organisation.clone(),
                date: a.date.clone(),
                signed_by: match (&a.key, &a.signature) {
                    (Some(k), Some(_)) => format!("{} {k}", rv.id),
                    _ => "unsigned".into(),
                },
                detail: format!("{}: separate from {}", a.id, a.developing_organisation),
                audit_record: a.audit_record.clone(),
                note: None,
                retired: Vec::new(),
                is_retired: false,
                retire: None,
            });
        }
    }
    o
}

/// The organisation sections' forms (no passphrase: the User management
/// tab's one is used).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OrgForms {
    pub dev_name: String,
    /// Empty: workspace-wide; else the crate it overrides.
    pub dev_crate: String,
    pub rev_reviewer: String,
    pub rev_name: String,
    pub attestation: AttestationInput,
}

/// Start the attestation form `a` from what `o` records for `reviewer`:
/// their organisation and the workspace developing organisation (the last
/// recorded and not retired of each), and a fresh id for `date` in `root`.
/// Fields already typed are kept.
pub fn prefill_attestation(a: &mut AttestationInput, o: &Overview, root: &ReviewRoot, reviewer: &str, date: &str) {
    if a.organisation.trim().is_empty() {
        if let Some(r) = o
            .reviewer_organisations
            .iter()
            .rev()
            .find(|r| r.scope == reviewer && !r.is_retired)
        {
            a.organisation = r.name.clone();
        }
    }
    if a.developing_organisation.trim().is_empty() {
        if let Some(d) = o
            .developing
            .iter()
            .rev()
            .find(|d| d.scope == "workspace" && !d.is_retired)
        {
            a.developing_organisation = d.name.clone();
        }
    }
    if a.id.trim().is_empty() {
        a.id = next_attestation_id(root, reviewer, date);
    }
}

#[cfg(test)]
#[path = "organisations_tests.rs"]
mod tests;
