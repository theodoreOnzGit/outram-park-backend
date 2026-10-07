//! **The reviewer registry, checked** (GitHub #762; rules from #739, comments
//! "Stamp signing inside kovan" and "Many maintainers", 2026-10-07).
//!
//! [`Registry::build`] reads the `[[reviewer]]` entries of `kovan_root.toml`
//! ([`ReviewRoot`]) and works out, with the signatures checked, which
//! reviewers are admitted and which keys are trusted. It never fails: every
//! "no" is a typed status on the reviewer or key, and [`Registry::warnings`]
//! lists all of them for the loud display (unendorsed keys, resets,
//! unverified revocations, …).
//!
//! # The trust rules
//!
//! ```text
//!   [code_review] founder = "<id>"  ->  [[reviewer]] <id> (a maintainer, no admitted_by)
//!        │  founding maintainer: its first key is trusted on first use
//!        │
//!        ├── key k2  endorsed_by = { key = "k1", … }   same reviewer's trusted key
//!        ├── key k3  reset = true, no endorsed_by      founder only: trusted, flagged forever
//!        │
//!        └─ admits ─> [[reviewer]] #2  admitted_by = { key = "k1", … }
//!                         (signed over id, role, admitted date, scope, FIRST key)
//!                         ├── key k1   trusted by the admission
//!                         ├── key k2   endorsed by k1
//!                         └── key k3   reset = true, endorsed by a maintainer's key
//! ```
//!
//! - **Founder.** ~~The first `[[reviewer]]` in the file is the founding
//!   maintainer~~ **CORRECTED 2026-10-07 (maintainer, #762 Q1)**: the
//!   founding maintainer is named explicitly by `[code_review] founder`; the
//!   order of entries never matters. It must be a listed `[[reviewer]]` with
//!   `role = "maintainer"` and no `admitted_by`; a missing or unknown founder
//!   is a [`FounderProblem`] (a [`Warning::NoFounder`], and nothing is
//!   trusted), never a guess. Its first key is trusted on first use. No
//!   other key or reviewer is.
//! - **Admission.** Every other reviewer needs `admitted_by`: a signature by
//!   an admitted maintainer's trusted key over [`admission_bytes`], dated by
//!   `admitted` (the maintainer must not be revoked by then). The admission
//!   covers the reviewer's first key, which is then trusted.
//! - **Endorsement.** Every later key needs `endorsed_by`, a signature over
//!   [`endorsement_bytes`] by a trusted key of the **same** reviewer. A key
//!   without one is [`KeyProblem::Unendorsed`]: loud, and its signatures do
//!   not count.
//! - **Reset.** A `reset = true` key (the old passphrase is lost) is
//!   endorsed by any admitted maintainer's trusted key not revoked by the
//!   key's `created` date. The founding maintainer may reset without one
//!   ([`KeyStatus::ResetTrustedOnFirstUse`]). Either way the key carries
//!   `reset` forever and [`Registry::warnings`] lists it.
//! - **Revocation.** `[reviewer.revoked]` is honoured **whether or not its
//!   signature verifies** (it can only take trust away); an unsigned or
//!   badly signed one is listed as a warning. Stamps dated before `date`
//!   stay valid; with `compromised_from`, stamps from that date are void.
//! - **Retirement.** A retired key's stamps dated on or after `retired_on`
//!   do not count, and **neither do its registry statements** (endorsement,
//!   admission, revocation) dated in its retired window
//!   ([`SignerProblem::SignerRetired`]; maintainer, #762 Q4, 2026-10-07).
//!   Its own un-retirement is the one exception; once un-retired it signs
//!   everything again. Un-retiring needs an `unretired` record signed by the
//!   key itself over [`unretire_bytes`] (only someone who unlocked the old
//!   encrypted key file can make it); the gap between the two dates stays
//!   void. Clearing `retired` by hand, without the record, leaves the key
//!   retired ([`UnretireProblem::NoProof`]).
//!
//! Trust is computed to a fixed point, so the order of entries other than
//! the founder's does not matter, and an endorsement cycle never becomes
//! trusted.

use ed25519_dalek::VerifyingKey;

use super::super::root::{KeySignature, ReviewRoot, ReviewerKey, Revocation, Role};
use super::{
    admission_bytes, decode_public, endorsement_bytes, is_date, revocation_bytes,
    unretire_bytes, verify_bytes, CryptoError, UnverifiedReason, ALG,
};

/// Why the registry has no founding maintainer (then nothing is trusted).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FounderProblem {
    /// `[code_review] founder` is not set.
    NotDeclared,
    /// `founder` names no `[[reviewer]]`.
    Unknown(String),
    /// The founder's `[[reviewer]]` is not a maintainer.
    NotMaintainer(String),
    /// The founder's `[[reviewer]]` carries an `admitted_by`.
    HasAdmission(String),
}

/// Why a signature by another key (an admission, an endorsement, a
/// revocation) does not count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignerProblem {
    /// There is no signature.
    Unsigned,
    /// No eligible signer has a key with this id.
    UnknownKey { key: String },
    /// Eligible keys with this id exist, but none is trusted.
    NotTrusted { key: String },
    /// The signature cannot be decoded.
    Malformed(CryptoError),
    /// No trusted key with this id verifies it.
    BadSignature,
    /// The signer was revoked (or compromised) on `date`, on or before the
    /// statement's date.
    SignerRevoked { signer: String, date: String },
    /// The signing key was retired at the statement's date (#762 Q4).
    SignerRetired { signer: String, key: String },
}

/// Why a reviewer is not admitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionProblem {
    /// Not the founder, and no `admitted_by`.
    Unadmitted,
    /// `admitted_by` without an `admitted` date.
    Undated,
    /// `admitted` is not `YYYY-MM-DD`.
    BadDate(String),
    /// No key: an admission covers the first key, so there must be one.
    NoKey,
    /// The admission signature does not count.
    Signer(SignerProblem),
}

/// How a reviewer is admitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Admission {
    /// The founding maintainer, trusted on first use.
    Founder,
    /// Admitted by maintainer `by`'s key `key`.
    Admitted { by: String, key: String },
    NotAdmitted(AdmissionProblem),
}

/// Why a key is not trusted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyProblem {
    /// `alg` is not `ed25519`.
    UnsupportedAlgorithm(String),
    /// `public` cannot be decoded.
    BadPublicKey(CryptoError),
    /// `created` is not `YYYY-MM-DD`.
    BadCreatedDate(String),
    /// Two keys of this reviewer share the id: neither is trusted.
    DuplicateId,
    /// The reviewer is not admitted.
    ReviewerNotAdmitted,
    /// Not the first key and no `endorsed_by` (a non-founder reset needs a
    /// maintainer's endorsement too).
    Unendorsed,
    /// The endorsement does not count.
    Endorsement(SignerProblem),
}

/// How a key is trusted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyStatus {
    /// The founding maintainer's first key.
    TrustedOnFirstUse,
    /// A reviewer's first key, covered by its admission.
    Admitted,
    /// Endorsed by `by_key` of reviewer `by_reviewer` (the same reviewer, or
    /// a maintainer for a reset).
    Endorsed { by_reviewer: String, by_key: String },
    /// A founding-maintainer reset with no endorsement: trusted, and flagged
    /// in [`Registry::warnings`] forever.
    ResetTrustedOnFirstUse,
    Untrusted(KeyProblem),
}

impl KeyStatus {
    /// Whether signatures by the key can count.
    pub fn is_trusted(&self) -> bool {
        !matches!(self, Self::Untrusted(_))
    }
}

/// Why an un-retirement does not count (the key stays retired).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnretireProblem {
    /// `retired_on` without `retired = true` and without an `unretired`
    /// record: the key was un-retired without the possession proof.
    NoProof,
    /// `unretired` but no `retired_on`.
    NoRetiredDate,
    /// A date is not `YYYY-MM-DD`.
    BadDate(String),
    /// The un-retire date is before the retirement.
    BeforeRetirement,
    /// `unretired` is recorded but `retired` is still `true`.
    StillMarkedRetired,
    /// The un-retire signature is not by the key itself.
    BadSignature(CryptoError),
}

/// A key's retirement state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Retirement {
    Active,
    /// Retired from `since` (`None`: no valid date, so no stamp counts).
    Retired { since: Option<String> },
    /// Retired on `retired_on`, back on `unretired_on`: stamps in between
    /// do not count.
    Unretired { retired_on: String, unretired_on: String },
    /// An un-retirement that does not count; the key stays retired from
    /// `retired_on` (`None`: no valid date, so no stamp counts).
    BadUnretire { retired_on: Option<String>, problem: UnretireProblem },
}

impl Retirement {
    /// `Err` when a stamp by key `key` dated `date` falls in a retired window.
    pub fn check(&self, key: &str, date: &str) -> Result<(), UnverifiedReason> {
        let retired = |since: &Option<String>| UnverifiedReason::KeyRetired {
            key: key.to_string(),
            since: since.clone(),
        };
        match self {
            Self::Active => Ok(()),
            Self::Retired { since } | Self::BadUnretire { retired_on: since, .. } => match since {
                Some(s) if date < s.as_str() => Ok(()),
                _ => Err(retired(since)),
            },
            Self::Unretired { retired_on, unretired_on } => {
                if date >= retired_on.as_str() && date < unretired_on.as_str() {
                    Err(retired(&Some(retired_on.clone())))
                } else {
                    Ok(())
                }
            }
        }
    }
}

/// One key, decoded and judged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyTrust {
    pub id: String,
    pub public: Result<VerifyingKey, CryptoError>,
    pub status: KeyStatus,
    /// A deliberate reset (shown permanently).
    pub reset: bool,
    pub retirement: Retirement,
}

/// Why a revocation's signature does not count. The revocation is honoured
/// anyway; this is a warning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RevocationProblem {
    /// `date` or `compromised_from` is not `YYYY-MM-DD` (then every stamp of
    /// the reviewer is void).
    BadDate(String),
    /// `by` is not an admitted maintainer.
    SignerNotMaintainer(String),
    Signer(SignerProblem),
}

/// `[reviewer.revoked]`, judged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevocationTrust {
    pub date: String,
    pub compromised_from: Option<String>,
    pub by: String,
    /// The signing key id, or why the signature does not count.
    pub signed: Result<String, RevocationProblem>,
}

impl RevocationTrust {
    /// The cut-off that voids something dated `date`, if any: the
    /// compromise date, else the revocation date. A malformed date voids
    /// everything.
    pub fn cutoff(&self, date: &str) -> Option<UnverifiedReason> {
        if let Some(c) = &self.compromised_from {
            if !is_date(c) || date >= c.as_str() {
                return Some(UnverifiedReason::Compromised { from: c.clone() });
            }
        }
        if !is_date(&self.date) || date >= self.date.as_str() {
            return Some(UnverifiedReason::Revoked { date: self.date.clone() });
        }
        None
    }

    /// `Err` when a stamp dated `date` is void by this revocation.
    pub fn check(&self, date: &str) -> Result<(), UnverifiedReason> {
        match self.cutoff(date) {
            Some(why) => Err(why),
            None => Ok(()),
        }
    }
}

/// One reviewer, judged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewerTrust {
    pub id: String,
    pub role: Role,
    pub scope: Vec<String>,
    pub admission: Admission,
    pub keys: Vec<KeyTrust>,
    pub revocation: Option<RevocationTrust>,
}

impl ReviewerTrust {
    /// The key with this id.
    pub fn key(&self, id: &str) -> Option<&KeyTrust> {
        self.keys.iter().find(|k| k.id == id)
    }

    /// Whether the reviewer is admitted (founder or by a maintainer).
    pub fn is_admitted(&self) -> bool {
        !matches!(self.admission, Admission::NotAdmitted(_))
    }
}

/// Everything the loud display must show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Warning {
    NoFounder(FounderProblem),
    ReviewerNotAdmitted { reviewer: String, problem: AdmissionProblem },
    KeyNotTrusted { reviewer: String, key: String, problem: KeyProblem },
    /// A reset key, trusted or not: shown permanently.
    KeyReset { reviewer: String, key: String, status: KeyStatus },
    KeyRetirement { reviewer: String, key: String, problem: UnretireProblem },
    /// A revocation honoured although its signature does not count.
    RevocationUnverified { reviewer: String, problem: RevocationProblem },
}

/// The judged registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registry {
    pub founder: Result<String, FounderProblem>,
    pub reviewers: Vec<ReviewerTrust>,
}

impl Registry {
    /// Judge every reviewer and key in `root` (module doc).
    pub fn build(root: &ReviewRoot) -> Registry {
        let founder = founder_of(root);
        let mut state: Vec<ReviewerTrust> = root
            .reviewers
            .iter()
            .map(|r| ReviewerTrust {
                id: r.id.clone(),
                role: r.role,
                scope: r.scope.clone(),
                admission: Admission::NotAdmitted(AdmissionProblem::Unadmitted),
                keys: r.keys.iter().map(|k| initial_key(&r.id, k, &r.keys)).collect(),
                revocation: r.revoked.as_ref().map(|rv| RevocationTrust {
                    date: rv.date.clone(),
                    compromised_from: rv.compromised_from.clone(),
                    by: rv.by.clone(),
                    signed: Err(RevocationProblem::Signer(SignerProblem::Unsigned)),
                }),
            })
            .collect();
        let founder_idx = founder.as_ref().ok().and_then(|id| state.iter().position(|r| &r.id == id));
        loop {
            let mut changed = false;
            for i in 0..state.len() {
                if !state[i].is_admitted() {
                    let res = if founder_idx == Some(i) {
                        Ok(Admission::Founder)
                    } else {
                        try_admit(root, &state, i)
                    };
                    state[i].admission = match res {
                        Ok(a) => {
                            changed = true;
                            a
                        }
                        Err(p) => Admission::NotAdmitted(p),
                    };
                }
                for j in 0..state[i].keys.len() {
                    if state[i].keys[j].status.is_trusted() || key_is_final(&state[i].keys[j]) {
                        continue;
                    }
                    let res = if state[i].is_admitted() {
                        try_key(root, &state, i, j, founder_idx == Some(i))
                    } else {
                        Err(KeyProblem::ReviewerNotAdmitted)
                    };
                    state[i].keys[j].status = match res {
                        Ok(s) => {
                            changed = true;
                            s
                        }
                        Err(p) => KeyStatus::Untrusted(p),
                    };
                }
            }
            if !changed {
                break;
            }
        }
        for i in 0..state.len() {
            if let Some(rv) = &root.reviewers[i].revoked {
                let signed = check_revocation(&state, i, rv);
                if let Some(t) = state[i].revocation.as_mut() {
                    t.signed = signed;
                }
            }
        }
        Registry { founder, reviewers: state }
    }

    /// The reviewer registered under `id`.
    pub fn reviewer(&self, id: &str) -> Option<&ReviewerTrust> {
        self.reviewers.iter().find(|r| r.id == id)
    }

    /// Every problem and every reset, for the loud display.
    pub fn warnings(&self) -> Vec<Warning> {
        let mut out = Vec::new();
        if let Err(p) = &self.founder {
            out.push(Warning::NoFounder(p.clone()));
        }
        for r in &self.reviewers {
            if let Admission::NotAdmitted(p) = &r.admission {
                out.push(Warning::ReviewerNotAdmitted { reviewer: r.id.clone(), problem: p.clone() });
            }
            for k in &r.keys {
                if let KeyStatus::Untrusted(p) = &k.status {
                    out.push(Warning::KeyNotTrusted {
                        reviewer: r.id.clone(),
                        key: k.id.clone(),
                        problem: p.clone(),
                    });
                }
                if k.reset {
                    out.push(Warning::KeyReset {
                        reviewer: r.id.clone(),
                        key: k.id.clone(),
                        status: k.status.clone(),
                    });
                }
                if let Retirement::BadUnretire { problem, .. } = &k.retirement {
                    out.push(Warning::KeyRetirement {
                        reviewer: r.id.clone(),
                        key: k.id.clone(),
                        problem: problem.clone(),
                    });
                }
            }
            if let Some(RevocationTrust { signed: Err(p), .. }) = &r.revocation {
                out.push(Warning::RevocationUnverified { reviewer: r.id.clone(), problem: p.clone() });
            }
        }
        out
    }
}

fn founder_of(root: &ReviewRoot) -> Result<String, FounderProblem> {
    let id = root
        .code_review
        .as_ref()
        .and_then(|c| c.founder.as_deref())
        .ok_or(FounderProblem::NotDeclared)?;
    let f = root.reviewer(id).ok_or_else(|| FounderProblem::Unknown(id.to_string()))?;
    if f.role != Role::Maintainer {
        return Err(FounderProblem::NotMaintainer(id.to_string()));
    }
    if f.admitted_by.is_some() {
        return Err(FounderProblem::HasAdmission(id.to_string()));
    }
    Ok(id.to_string())
}

/// A key whose problem no signature can fix.
fn key_is_final(k: &KeyTrust) -> bool {
    matches!(
        k.status,
        KeyStatus::Untrusted(
            KeyProblem::UnsupportedAlgorithm(_)
                | KeyProblem::BadPublicKey(_)
                | KeyProblem::BadCreatedDate(_)
                | KeyProblem::DuplicateId
        )
    )
}

fn initial_key(owner: &str, k: &ReviewerKey, all: &[ReviewerKey]) -> KeyTrust {
    let public = decode_public(&k.public);
    let status = if k.alg != ALG {
        KeyStatus::Untrusted(KeyProblem::UnsupportedAlgorithm(k.alg.clone()))
    } else if let Err(e) = &public {
        KeyStatus::Untrusted(KeyProblem::BadPublicKey(e.clone()))
    } else if !is_date(&k.created) {
        KeyStatus::Untrusted(KeyProblem::BadCreatedDate(k.created.clone()))
    } else if all.iter().filter(|o| o.id == k.id).count() > 1 {
        KeyStatus::Untrusted(KeyProblem::DuplicateId)
    } else {
        KeyStatus::Untrusted(KeyProblem::ReviewerNotAdmitted)
    };
    KeyTrust {
        id: k.id.clone(),
        retirement: retirement_of(owner, k, public.as_ref().ok()),
        public,
        status,
        reset: k.reset,
    }
}

fn retirement_of(owner: &str, k: &ReviewerKey, public: Option<&VerifyingKey>) -> Retirement {
    let since = k.retired_on.clone().filter(|d| is_date(d));
    let bad = |problem| Retirement::BadUnretire { retired_on: since.clone(), problem };
    if let Some(d) = k.retired_on.as_ref().filter(|d| !is_date(d)) {
        return bad(UnretireProblem::BadDate(d.clone()));
    }
    match (&k.unretired, k.retired) {
        (None, true) => Retirement::Retired { since },
        (Some(_), true) => bad(UnretireProblem::StillMarkedRetired),
        (None, false) if k.retired_on.is_some() => bad(UnretireProblem::NoProof),
        (None, false) => Retirement::Active,
        (Some(u), false) => {
            let Some(retired_on) = since.clone() else {
                return bad(UnretireProblem::NoRetiredDate);
            };
            if !is_date(&u.date) {
                return bad(UnretireProblem::BadDate(u.date.clone()));
            }
            if u.date < retired_on {
                return bad(UnretireProblem::BeforeRetirement);
            }
            let Some(pk) = public else {
                return bad(UnretireProblem::BadSignature(CryptoError::BadPublicKey));
            };
            match verify_bytes(pk, &unretire_bytes(owner, k, &u.date), &u.signature) {
                Ok(()) => Retirement::Unretired { retired_on, unretired_on: u.date.clone() },
                Err(e) => bad(UnretireProblem::BadSignature(e)),
            }
        }
    }
}

/// Find a trusted key with `ks.key` among `candidates` (reviewer indices)
/// that verifies `ks.signature` over `msg`; with `as_of`, its reviewer must
/// not be revoked on or before that date. The key must not be retired at
/// `dated`, the statement's own date (#762 Q4).
fn find_signer(
    state: &[ReviewerTrust],
    candidates: &[usize],
    ks: &KeySignature,
    msg: &[u8],
    as_of: Option<&str>,
    dated: &str,
) -> Result<(String, String), SignerProblem> {
    let (mut any_key, mut any_trusted) = (false, false);
    let mut malformed = None;
    for &ri in candidates {
        let r = &state[ri];
        for k in r.keys.iter().filter(|k| k.id == ks.key) {
            any_key = true;
            if !k.status.is_trusted() {
                continue;
            }
            any_trusted = true;
            let Ok(pk) = &k.public else { continue };
            match verify_bytes(pk, msg, &ks.signature) {
                Ok(()) => {
                    if let (Some(d), Some(rev)) = (as_of, &r.revocation) {
                        let date = match rev.cutoff(d) {
                            Some(UnverifiedReason::Compromised { from }) => Some(from),
                            Some(UnverifiedReason::Revoked { date }) => Some(date),
                            _ => None,
                        };
                        if let Some(date) = date {
                            return Err(SignerProblem::SignerRevoked { signer: r.id.clone(), date });
                        }
                    }
                    if k.retirement.check(&k.id, dated).is_err() {
                        return Err(SignerProblem::SignerRetired { signer: r.id.clone(), key: k.id.clone() });
                    }
                    return Ok((r.id.clone(), k.id.clone()));
                }
                Err(CryptoError::Mismatch) => {}
                Err(e) => malformed = Some(e),
            }
        }
    }
    if !any_key {
        Err(SignerProblem::UnknownKey { key: ks.key.clone() })
    } else if !any_trusted {
        Err(SignerProblem::NotTrusted { key: ks.key.clone() })
    } else if let Some(e) = malformed {
        Err(SignerProblem::Malformed(e))
    } else {
        Err(SignerProblem::BadSignature)
    }
}

/// Admitted maintainers other than `except`.
fn maintainers(state: &[ReviewerTrust], except: Option<usize>) -> Vec<usize> {
    (0..state.len())
        .filter(|&m| Some(m) != except && state[m].role == Role::Maintainer && state[m].is_admitted())
        .collect()
}

fn try_admit(root: &ReviewRoot, state: &[ReviewerTrust], i: usize) -> Result<Admission, AdmissionProblem> {
    let r = &root.reviewers[i];
    let ks = r.admitted_by.as_ref().ok_or(AdmissionProblem::Unadmitted)?;
    let date = r.admitted.as_deref().ok_or(AdmissionProblem::Undated)?;
    if !is_date(date) {
        return Err(AdmissionProblem::BadDate(date.to_string()));
    }
    if r.keys.is_empty() {
        return Err(AdmissionProblem::NoKey);
    }
    let candidates = maintainers(state, Some(i));
    let (by, key) = find_signer(state, &candidates, ks, &admission_bytes(r), Some(date), date)
        .map_err(AdmissionProblem::Signer)?;
    Ok(Admission::Admitted { by, key })
}

fn try_key(
    root: &ReviewRoot,
    state: &[ReviewerTrust],
    i: usize,
    j: usize,
    is_founder: bool,
) -> Result<KeyStatus, KeyProblem> {
    let r = &root.reviewers[i];
    let k = &r.keys[j];
    if j == 0 {
        return Ok(if is_founder { KeyStatus::TrustedOnFirstUse } else { KeyStatus::Admitted });
    }
    let Some(ks) = &k.endorsed_by else {
        return if k.reset && is_founder {
            Ok(KeyStatus::ResetTrustedOnFirstUse)
        } else {
            Err(KeyProblem::Unendorsed)
        };
    };
    let msg = endorsement_bytes(&r.id, k);
    let found = if k.reset {
        find_signer(state, &maintainers(state, None), ks, &msg, Some(&k.created), &k.created)
    } else {
        find_signer(state, &[i], ks, &msg, None, &k.created)
    };
    // A key never endorses itself: `find_signer` only accepts trusted keys,
    // and this one is not trusted yet.
    let (by_reviewer, by_key) = found.map_err(KeyProblem::Endorsement)?;
    Ok(KeyStatus::Endorsed { by_reviewer, by_key })
}

fn check_revocation(
    state: &[ReviewerTrust],
    i: usize,
    rv: &Revocation,
) -> Result<String, RevocationProblem> {
    for d in std::iter::once(&rv.date).chain(rv.compromised_from.iter()) {
        if !is_date(d) {
            return Err(RevocationProblem::BadDate(d.clone()));
        }
    }
    let by = state
        .iter()
        .position(|m| m.id == rv.by && m.role == Role::Maintainer && m.is_admitted())
        .ok_or_else(|| RevocationProblem::SignerNotMaintainer(rv.by.clone()))?;
    let ks = rv.signature.as_ref().ok_or(RevocationProblem::Signer(SignerProblem::Unsigned))?;
    // A maintainer revoking someone must not be revoked by then; revoking
    // themselves is the exception (their own cut-off is this very date).
    let as_of = if by == i { None } else { Some(rv.date.as_str()) };
    let msg = revocation_bytes(&state[i].id, rv);
    let (_, key) = find_signer(state, &[by], ks, &msg, as_of, &rv.date).map_err(RevocationProblem::Signer)?;
    Ok(key)
}
