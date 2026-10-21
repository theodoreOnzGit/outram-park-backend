//! **Organisations and separation attestations from desktop kovan**
//! (GitHub #810, follow-up of #809): the GUI-free layer of the Code Review
//! tab's "Organisations & IV&V" panel. The drawing is
//! `app/organisations_view.rs`; this file decides what it may do.
//!
//! ```text
//!   a maintainer, key unlocked ──> add_developing_organisation ─┐
//!                              └─> add_reviewer_organisation ───┤  sign, then append
//!   a reviewer, key unlocked ───> add_separation_attestation ───┤  as text (comments
//!                                  (audit record: a GitHub      │  kept, read back:
//!                                   issue URL, form only)       v  root_append)
//!                                                         kovan_root.toml
//!   overview(root) ──> what is recorded, and every record that does not verify
//!   attestation_choices_in(root, by) ──> the stamp dialog's picker
//! ```
//!
//! **Who signs what** (maintainer decisions on #809, 2026-10-08):
//! developing-organisation and reviewer-organisation records are signed by
//! an admitted maintainer; a separation attestation by the independent
//! reviewer alone, naming a public GitHub issue as its audit record. Kovan
//! checks the URL's form and never fetches it (offline; Leak Before Break:
//! it is shown "audit record (not verified by kovan)").
//!
//! **Refused before anything is written**: a maintainer record signed with
//! a key whose reviewer is not a maintainer in `kovan_root.toml` (it could
//! never count), an attestation by a reviewer the root does not list, an
//! attestation id the reviewer already uses, a malformed date or audit
//! record. Every write appends; nothing is edited (the records are
//! append-only, #809).
//!
//! **AI agents never sign**: every function here takes an [`UnlockedKey`],
//! which only its human owner's passphrase unlocks. `kovan-cli` has no
//! signing command.

use std::path::{Path, PathBuf};

use kovan_common::review::engine::SignaturePolicy;
use kovan_common::review::ivv::{parse_audit_record, record_warnings};
use kovan_common::review::ivv_text::{audit_record_problem, record_warning};
use kovan_common::review::ivv_view::{attestation_choices, AttestationChoice};
use kovan_common::review::root::{
    DevelopingOrganisation, ReviewRoot, ReviewerOrganisation, Role, SeparationAttestation,
};
use kovan_common::review::root_append::{
    append_developing_organisation, append_reviewer_organisation, append_separation_attestation,
};
use kovan_common::review::signing::keystore::UnlockedKey;
use kovan_common::review::signing::registry::Registry;

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

/// The key's reviewer must be a maintainer in `root`.
fn require_maintainer(root: &ReviewRoot, key: &UnlockedKey) -> Result<(), String> {
    match root.reviewer(key.reviewer()) {
        Some(r) if r.role == Role::Maintainer => Ok(()),
        Some(_) => Err(format!(
            "{} is not a maintainer in {ROOT_FILE}: only a maintainer's signature counts here",
            key.reviewer()
        )),
        None => Err(format!("{ROOT_FILE} lists no reviewer {}", key.reviewer())),
    }
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

/// Sign, as a maintainer, a developing-organisation record (`krate`:
/// a per-crate override, else workspace-wide) in force from `date`, and
/// append it to `<root>/kovan_root.toml` (module doc).
pub fn add_developing_organisation(
    root: &Path,
    key: &UnlockedKey,
    organisation: &str,
    krate: Option<&str>,
    date: &str,
) -> Result<DevelopingOrganisation, String> {
    let (path, text, parsed) = read(root)?;
    require_maintainer(&parsed, key)?;
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

/// Sign, as a maintainer, `reviewer`'s organisation from `date`, and
/// append it (module doc).
pub fn add_reviewer_organisation(
    root: &Path,
    key: &UnlockedKey,
    reviewer: &str,
    organisation: &str,
    date: &str,
) -> Result<ReviewerOrganisation, String> {
    let (path, text, parsed) = read(root)?;
    require_maintainer(&parsed, key)?;
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

/// One record as the panel lists it.
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
}

/// Read `<root>/kovan_root.toml` into an [`Overview`], signatures checked.
pub fn overview(root: &Path) -> Result<Overview, String> {
    let (_, _, r) = read(root)?;
    let reg = Registry::build(&r);
    let signer =
        |s: Option<&kovan_common::review::root::KeySigner>, sig: Option<&String>| match (s, sig) {
            (Some(s), Some(_)) => {
                format!("{} {}", s.reviewer.as_deref().unwrap_or("(unnamed)"), s.key)
            }
            _ => "unsigned".into(),
        };
    let mut o = Overview {
        warnings: record_warnings(&r, &reg, SignaturePolicy::Enforce)
            .iter()
            .map(record_warning)
            .collect(),
        reviewers: r.reviewers.iter().map(|x| (x.id.clone(), x.role)).collect(),
        ..Overview::default()
    };
    if let Some(cr) = &r.code_review {
        for e in &cr.developing_organisation {
            o.developing.push(RecordRow {
                scope: e
                    .krate
                    .as_ref()
                    .map_or("workspace".into(), |k| format!("crate {k}")),
                name: e.name.clone(),
                date: e.date.clone(),
                signed_by: signer(e.signer.as_ref(), e.signature.as_ref()),
                detail: String::new(),
                audit_record: None,
            });
        }
    }
    for rv in &r.reviewers {
        for e in &rv.organisations {
            o.reviewer_organisations.push(RecordRow {
                scope: rv.id.clone(),
                name: e.name.clone(),
                date: e.date.clone(),
                signed_by: signer(e.signer.as_ref(), e.signature.as_ref()),
                detail: String::new(),
                audit_record: None,
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
            });
        }
    }
    Ok(o)
}

#[path = "organisations_flow.rs"]
pub mod panel;

#[cfg(test)]
#[path = "organisations_tests.rs"]
mod tests;
