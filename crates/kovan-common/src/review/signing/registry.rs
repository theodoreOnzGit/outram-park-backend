//! **The reviewer registry, checked** (GitHub #762; rules from #739, comments
//! "Stamp signing inside kovan" and "Many maintainers", and the maintainer's
//! answers on #762, 2026-10-07).
//!
//! [`Registry::build`] reads the `[[reviewer]]` entries of `kovan_root.toml`
//! ([`ReviewRoot`]) and works out, with the signatures checked, which
//! reviewers are admitted, which keys are trusted and when each key was
//! active. It never fails: every "no" is a typed status on the reviewer or
//! key, and [`Registry::warnings`] lists all of them for the loud display
//! (unendorsed keys, resets, tampered history entries, unverified
//! revocations, …).
//!
//! # A key's life is its append-only history
//!
//! Each `[[reviewer.key]]` carries `[[reviewer.key.history]]`: one entry per
//! lifecycle event, in order, never edited or deleted (maintainer, #762,
//! 2026-10-07). The key's state is **derived by replaying it**:
//!
//! ```text
//!   [[reviewer.key]] id = "k1"
//!     history: created ─ endorsed ─ retired ─ unretired ─ retired ─ unretired ─ compromised
//!              (date)    (signed)   (date)    (signed by     (date)  (signed by    (signed)
//!                                             k1 itself)             k1 itself)
//!   active:    ─────────────────────┤ void   ├────────────────┤ void ├───────────┤ void ──>
//! ```
//!
//! **Why per key, not a top-level `[[key_history]]` log.** Every event
//! concerns exactly one key, so it sits under that key: a reader sees one
//! key's whole life in one place, and an entry cannot name a key or
//! reviewer that does not exist (a top-level log would have to repeat both
//! ids on every line and validate them). Events that need another
//! reviewer's authority name it in `signer`.
//!
//! The v1 fields (`endorsed_by`, `reset`, `retired`, `retired_on`,
//! `unretired`, the reviewer's `admitted_by`) still load:
//! [`ReviewRoot::migrate_key_history`] turns them into history events
//! (signed ones marked `legacy`, verified over the v1 bytes), and
//! [`Registry::build`] runs it on a copy first.
//!
//! # The trust rules
//!
//! ```text
//!   [code_review] founder = "<id>"  ->  [[reviewer]] <id> (a maintainer, no admission)
//!        │  founding maintainer: its first key is trusted on first use
//!        ├── key k2  history: endorsed by k1                  same reviewer's trusted key
//!        ├── key k3  history: reset, unsigned                 founder only: trusted, flagged forever
//!        └─ admits ─> [[reviewer]] #2, key a1 history: admitted, signed by the founder's k1
//!                         (signed over id, role, admitted date, scope, FIRST key)
//!                         ├── key a2   endorsed by a1
//!                         └── key a3   reset, signed by a maintainer's key
//! ```
//!
//! - **Founder.** Named by `[code_review] founder` (maintainer, #762 Q1);
//!   entry order never matters. It must be a listed maintainer with no
//!   admission; a missing or unknown founder is a [`FounderProblem`] and
//!   nothing is trusted. Its first key is trusted on first use.
//! - **Admission.** Every other reviewer's first key needs an `admitted`
//!   event signed by an admitted maintainer's trusted key (dated the
//!   reviewer's `admitted`; the maintainer must not be revoked by then).
//! - **Endorsement.** Every later key needs an `endorsed` event signed by a
//!   trusted key of the **same** reviewer, else [`KeyProblem::Unendorsed`]:
//!   loud, and its signatures do not count.
//! - **Reset.** A `reset` event (the old passphrase is lost) is signed by
//!   an admitted maintainer's trusted key; the founder may reset unsigned
//!   ([`KeyStatus::ResetTrustedOnFirstUse`]). Shown forever (#762 Q2).
//! - **Retirement.** A key is void from each `retired` event until the next
//!   `unretired` event, which must be signed by **the key itself** (the
//!   possession proof: only someone who unlocked its encrypted file can make
//!   it). Retire, un-retire, retire, un-retire works; every gap stays void.
//! - **Key revocation.** A `revoked` or `compromised` event voids the key
//!   from its date, permanently. It is honoured even if unsigned or badly
//!   signed (it only takes trust away) and listed as a warning; it is
//!   signed by a maintainer or by the key's own reviewer.
//! - **Inactive keys sign nothing** (maintainer, #762 Q4): a key void at a
//!   statement's date (retired, revoked, compromised) cannot sign an
//!   endorsement, admission or revocation then
//!   ([`SignerProblem::SignerRetired`], [`SignerProblem::SignerKeyRevoked`]);
//!   its own un-retirement is the one exception.
//! - **Reviewer revocation.** `[reviewer.revoked]` revokes the person:
//!   honoured whether or not its signature verifies (#762 Q3), listed as a
//!   warning if it does not. Stamps dated before `date` stay valid; with
//!   `compromised_from`, stamps from that date are void.
//! - **Tampering.** Every signed event is checked on its own; one that does
//!   not verify is a [`HistoryProblem::BadEventSignature`] warning even when
//!   another event already made the key trusted. Dates must not go
//!   backwards ([`HistoryProblem::OutOfOrder`]). A deleted *trailing*
//!   `retired` entry cannot be seen without git; a deleted one followed by
//!   its `unretired` can ([`HistoryProblem::UnretiredWhileActive`]).
//!
//! Trust is computed to a fixed point, so the order of entries does not
//! matter, and an endorsement cycle never becomes trusted.

use ed25519_dalek::VerifyingKey;

use super::super::root::{
    KeyEvent, KeyEventKind, KeySigner, ReviewRoot, Reviewer, ReviewerKey, Revocation, Role,
};
use super::{
    decode_public, event_signed_bytes, is_date, revocation_bytes, verify_bytes, CryptoError,
    UnverifiedReason, ALG,
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
    /// The founder carries an admission (an `admitted` event, or a v1
    /// `admitted_by`).
    HasAdmission(String),
}

/// Why a signature by another key (an admission, an endorsement, a
/// revocation) does not count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignerProblem {
    /// There is no signature (or no `signer`).
    Unsigned,
    /// `signer.reviewer` is not one of the reviewers allowed to sign this.
    NotEligible(String),
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
    /// The signing key was revoked or compromised (key history) at the
    /// statement's date.
    SignerKeyRevoked { signer: String, key: String },
}

/// Why a reviewer is not admitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionProblem {
    /// Not the founder, and no `admitted` event on the first key.
    Unadmitted,
    /// An admission without the reviewer's `admitted` date.
    Undated,
    /// `admitted` is not `YYYY-MM-DD`.
    BadDate(String),
    /// No key: an admission sits on, and covers, the first key.
    NoKey,
    /// The `admitted` event's date is not the reviewer's `admitted` date.
    DateMismatch { event: String, admitted: String },
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
    /// Not the first key and no `endorsed`/`reset` event (a non-founder
    /// reset needs a maintainer's signature too).
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
    /// A founding-maintainer reset with no signature: trusted, and flagged
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

/// Something wrong in a key's history (`index` is the entry's position).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryProblem {
    /// The history does not start with `created`.
    MissingCreated,
    /// A second `created` entry.
    CreatedNotFirst { index: usize },
    /// A date is not `YYYY-MM-DD` (a malformed retirement or revocation
    /// voids the key from the start).
    BadDate { index: usize, date: String },
    /// A date earlier than the entry before it.
    OutOfOrder { index: usize },
    /// `retired` while already retired (the first retirement stands).
    RetiredWhileRetired { index: usize },
    /// `unretired` with no open retirement before it.
    UnretiredWhileActive { index: usize },
    /// `admitted` on a key other than the reviewer's first.
    AdmittedNotOnFirstKey { index: usize },
    /// A signed event whose signature does not count (tampered, wrong
    /// signer, unsigned where a signature is required, or an unsigned
    /// revocation, which is honoured anyway).
    BadEventSignature { index: usize, event: KeyEventKind, problem: SignerProblem },
    /// The key has both a history and v1 legacy fields: the history is used
    /// and the legacy fields are ignored.
    LegacyFieldsIgnored,
}

/// Why a key was void for a while.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InactiveKind {
    Retired,
    Revoked,
    Compromised,
}

/// A span in which the key is void: from `from` (`None`: malformed date,
/// so from the start) until `to` (`None`: still).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    pub kind: InactiveKind,
    pub from: Option<String>,
    pub to: Option<String>,
}

impl Window {
    fn covers(&self, date: &str) -> bool {
        let after_start = match &self.from {
            Some(f) => date >= f.as_str(),
            None => true,
        };
        let before_end = match &self.to {
            Some(t) => date < t.as_str(),
            None => true,
        };
        after_start && before_end
    }
}

/// One key, decoded, replayed and judged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyTrust {
    pub id: String,
    pub public: Result<VerifyingKey, CryptoError>,
    pub status: KeyStatus,
    /// A `reset` event is in the history (shown permanently).
    pub reset: bool,
    /// Every span in which the key is void, from the history.
    pub windows: Vec<Window>,
    /// Problems found in the history.
    pub history: Vec<HistoryProblem>,
}

impl KeyTrust {
    /// Why something the key signed, dated `date`, does not count; `None`
    /// when the key was active then.
    pub fn inactive_at(&self, date: &str) -> Option<UnverifiedReason> {
        let w = self.windows.iter().find(|w| w.covers(date))?;
        let key = self.id.clone();
        Some(match w.kind {
            InactiveKind::Retired => UnverifiedReason::KeyRetired { key, since: w.from.clone() },
            InactiveKind::Revoked => UnverifiedReason::KeyRevoked { key, date: w.from.clone() },
            InactiveKind::Compromised => UnverifiedReason::KeyCompromised { key, from: w.from.clone() },
        })
    }

    /// Whether the key is retired now (an open retirement window).
    pub fn is_retired(&self) -> bool {
        self.windows.iter().any(|w| w.kind == InactiveKind::Retired && w.to.is_none())
    }
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
    /// A problem in a key's history (a tampered entry, an unsigned
    /// revocation, an out-of-order date, …).
    KeyHistory { reviewer: String, key: String, problem: HistoryProblem },
    /// A reviewer revocation honoured although its signature does not count.
    RevocationUnverified { reviewer: String, problem: RevocationProblem },
}

/// Why a lifecycle event cannot be appended ([`retire_key`],
/// `keystore::UnlockedKey::unretire`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleError {
    /// The date is not `YYYY-MM-DD`, or is before the history's last entry.
    BadDate(String),
    /// [`retire_key`] on a key already retired.
    AlreadyRetired,
    /// Un-retiring a key that is not retired.
    NotRetired,
}

impl std::fmt::Display for LifecycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadDate(d) => write!(f, "bad or backwards date {d:?}"),
            Self::AlreadyRetired => write!(f, "the key is already retired"),
            Self::NotRetired => write!(f, "the key is not retired"),
        }
    }
}

impl std::error::Error for LifecycleError {}

/// The date of `k`'s open retirement (replaying `retired`/`unretired` only,
/// signatures unchecked), if it is retired now.
pub fn open_retirement(k: &ReviewerKey) -> Option<String> {
    let mut open = None;
    for e in &k.history {
        match e.event {
            KeyEventKind::Retired if open.is_none() => open = Some(e.date.clone()),
            KeyEventKind::Unretired => open = None,
            _ => {}
        }
    }
    open
}

/// Check that `date` can be appended to `k`'s history.
pub fn check_append_date(k: &ReviewerKey, date: &str) -> Result<(), LifecycleError> {
    let last = k.history.last().map(|e| e.date.as_str()).unwrap_or("");
    if !is_date(date) || date < last {
        return Err(LifecycleError::BadDate(date.to_string()));
    }
    Ok(())
}

/// Append a `retired` event (unsigned: retiring only takes trust away; no
/// private key needed, so a lost key can be retired).
pub fn retire_key(k: &mut ReviewerKey, date: &str) -> Result<(), LifecycleError> {
    check_append_date(k, date)?;
    if open_retirement(k).is_some() {
        return Err(LifecycleError::AlreadyRetired);
    }
    k.history.push(KeyEvent::unsigned(KeyEventKind::Retired, date));
    Ok(())
}

/// The judged registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registry {
    pub founder: Result<String, FounderProblem>,
    pub reviewers: Vec<ReviewerTrust>,
}

impl Registry {
    /// Judge every reviewer and key in `root` (module doc). The v1 legacy
    /// fields are migrated into history on a copy first.
    pub fn build(root: &ReviewRoot) -> Registry {
        let mut root = root.clone();
        let conflicts = root.migrate_key_history();
        let root = &root;
        let founder = founder_of(root);
        let mut state: Vec<ReviewerTrust> = root
            .reviewers
            .iter()
            .map(|r| ReviewerTrust {
                id: r.id.clone(),
                role: r.role,
                scope: r.scope.clone(),
                admission: Admission::NotAdmitted(AdmissionProblem::Unadmitted),
                keys: r.keys.iter().enumerate().map(|(j, k)| initial_key(r, j, k)).collect(),
                revocation: r.revoked.as_ref().map(|rv| RevocationTrust {
                    date: rv.date.clone(),
                    compromised_from: rv.compromised_from.clone(),
                    by: rv.by.clone(),
                    signed: Err(RevocationProblem::Signer(SignerProblem::Unsigned)),
                }),
            })
            .collect();
        for c in &conflicts {
            if let Some(k) = state
                .iter_mut()
                .find(|r| r.id == c.reviewer)
                .and_then(|r| r.keys.iter_mut().find(|k| k.id == c.key))
            {
                k.history.push(HistoryProblem::LegacyFieldsIgnored);
            }
        }
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
        // Every signed event on its own, so a tampered entry shows even
        // when another one already made the key trusted.
        let mut found = Vec::new();
        for (i, r) in root.reviewers.iter().enumerate() {
            for (j, k) in r.keys.iter().enumerate() {
                for (index, ev) in k.history.iter().enumerate() {
                    if let Some(Err(problem)) = check_event(root, &state, i, j, ev, founder_idx == Some(i)) {
                        found.push((i, j, HistoryProblem::BadEventSignature { index, event: ev.event, problem }));
                    }
                }
            }
        }
        for (i, j, p) in found {
            state[i].keys[j].history.push(p);
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
                for p in &k.history {
                    out.push(Warning::KeyHistory {
                        reviewer: r.id.clone(),
                        key: k.id.clone(),
                        problem: p.clone(),
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
    let admitted_event = f
        .keys
        .iter()
        .any(|k| k.history.iter().any(|e| e.event == KeyEventKind::Admitted));
    if f.admitted_by.is_some() || admitted_event {
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

fn initial_key(r: &Reviewer, j: usize, k: &ReviewerKey) -> KeyTrust {
    let public = decode_public(&k.public);
    let status = if k.alg != ALG {
        KeyStatus::Untrusted(KeyProblem::UnsupportedAlgorithm(k.alg.clone()))
    } else if let Err(e) = &public {
        KeyStatus::Untrusted(KeyProblem::BadPublicKey(e.clone()))
    } else if !is_date(&k.created) {
        KeyStatus::Untrusted(KeyProblem::BadCreatedDate(k.created.clone()))
    } else if r.keys.iter().filter(|o| o.id == k.id).count() > 1 {
        KeyStatus::Untrusted(KeyProblem::DuplicateId)
    } else {
        KeyStatus::Untrusted(KeyProblem::ReviewerNotAdmitted)
    };
    let (windows, history) = replay(r, j, k, public.as_ref().ok());
    KeyTrust {
        id: k.id.clone(),
        reset: k.history.iter().any(|e| e.event == KeyEventKind::Reset),
        public,
        status,
        windows,
        history,
    }
}

/// Replay the history for the void windows and the structural problems.
/// `unretired` is verified here (it is signed by the key itself, so it needs
/// no trust from anyone else).
fn replay(
    r: &Reviewer,
    j: usize,
    k: &ReviewerKey,
    public: Option<&VerifyingKey>,
) -> (Vec<Window>, Vec<HistoryProblem>) {
    let mut windows = Vec::new();
    let mut problems = Vec::new();
    if k.history.first().map(|e| e.event) != Some(KeyEventKind::Created) {
        problems.push(HistoryProblem::MissingCreated);
    }
    let mut prev: Option<&str> = None;
    let mut open: Option<Option<String>> = None;
    for (index, e) in k.history.iter().enumerate() {
        if index > 0 && e.event == KeyEventKind::Created {
            problems.push(HistoryProblem::CreatedNotFirst { index });
        }
        if j > 0 && e.event == KeyEventKind::Admitted {
            problems.push(HistoryProblem::AdmittedNotOnFirstKey { index });
        }
        let date = is_date(&e.date).then(|| e.date.clone());
        if date.is_none() {
            problems.push(HistoryProblem::BadDate { index, date: e.date.clone() });
        }
        if let (Some(d), Some(p)) = (date.as_deref(), prev) {
            if d < p {
                problems.push(HistoryProblem::OutOfOrder { index });
            }
        }
        if date.is_some() {
            prev = Some(&e.date);
        }
        match e.event {
            KeyEventKind::Retired => {
                if open.is_some() {
                    problems.push(HistoryProblem::RetiredWhileRetired { index });
                } else {
                    open = Some(date);
                }
            }
            KeyEventKind::Unretired => match open.clone() {
                None => problems.push(HistoryProblem::UnretiredWhileActive { index }),
                Some(from) => {
                    let retired_on = from.clone().unwrap_or_default();
                    match check_self_signed(r, k, e, &retired_on, public) {
                        Ok(()) if date.is_some() => {
                            windows.push(Window { kind: InactiveKind::Retired, from, to: date.clone() });
                            open = None;
                        }
                        Ok(()) => {}
                        Err(problem) => problems.push(HistoryProblem::BadEventSignature {
                            index,
                            event: e.event,
                            problem,
                        }),
                    }
                }
            },
            KeyEventKind::Revoked => {
                windows.push(Window { kind: InactiveKind::Revoked, from: date, to: None });
            }
            KeyEventKind::Compromised => {
                windows.push(Window { kind: InactiveKind::Compromised, from: date, to: None });
            }
            KeyEventKind::Created
            | KeyEventKind::Endorsed
            | KeyEventKind::Reset
            | KeyEventKind::Admitted => {}
        }
    }
    if let Some(from) = open {
        windows.push(Window { kind: InactiveKind::Retired, from, to: None });
    }
    (windows, problems)
}

/// An event signed by the key itself (`unretired`, or a self-revocation).
fn check_self_signed(
    r: &Reviewer,
    k: &ReviewerKey,
    e: &KeyEvent,
    retired_on: &str,
    public: Option<&VerifyingKey>,
) -> Result<(), SignerProblem> {
    let (sg, sig) = match (&e.signer, &e.signature) {
        (Some(sg), Some(sig)) => (sg, sig),
        _ => return Err(SignerProblem::Unsigned),
    };
    if sg.key != k.id || sg.reviewer.as_deref().is_some_and(|x| x != r.id) {
        return Err(SignerProblem::UnknownKey { key: sg.key.clone() });
    }
    let pk = public.ok_or(SignerProblem::Malformed(CryptoError::BadPublicKey))?;
    let msg = event_signed_bytes(&r.id, k, e, retired_on, Some(r));
    match verify_bytes(pk, &msg, sig) {
        Ok(()) => Ok(()),
        Err(CryptoError::Mismatch) => Err(SignerProblem::BadSignature),
        Err(other) => Err(SignerProblem::Malformed(other)),
    }
}

/// Find a trusted key named by `signer` among `candidates` (reviewer
/// indices) that verifies `sig` over `msg`; with `as_of`, its reviewer must
/// not be revoked on or before that date. The key must be active (not
/// retired, revoked or compromised) at `dated`, the statement's own date
/// (#762 Q4).
fn find_signer(
    state: &[ReviewerTrust],
    candidates: &[usize],
    signer: &KeySigner,
    sig: &str,
    msg: &[u8],
    as_of: Option<&str>,
    dated: &str,
) -> Result<(String, String), SignerProblem> {
    let candidates: Vec<usize> = match &signer.reviewer {
        Some(id) => {
            let c: Vec<usize> = candidates.iter().copied().filter(|&i| &state[i].id == id).collect();
            if c.is_empty() {
                return Err(SignerProblem::NotEligible(id.clone()));
            }
            c
        }
        None => candidates.to_vec(),
    };
    let (mut any_key, mut any_trusted) = (false, false);
    let mut malformed = None;
    for &ri in &candidates {
        let r = &state[ri];
        for k in r.keys.iter().filter(|k| k.id == signer.key) {
            any_key = true;
            if !k.status.is_trusted() {
                continue;
            }
            any_trusted = true;
            let Ok(pk) = &k.public else { continue };
            match verify_bytes(pk, msg, sig) {
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
                    match k.inactive_at(dated) {
                        None => return Ok((r.id.clone(), k.id.clone())),
                        Some(UnverifiedReason::KeyRetired { .. }) => {
                            return Err(SignerProblem::SignerRetired { signer: r.id.clone(), key: k.id.clone() })
                        }
                        Some(_) => {
                            return Err(SignerProblem::SignerKeyRevoked { signer: r.id.clone(), key: k.id.clone() })
                        }
                    }
                }
                Err(CryptoError::Mismatch) => {}
                Err(e) => malformed = Some(e),
            }
        }
    }
    if !any_key {
        Err(SignerProblem::UnknownKey { key: signer.key.clone() })
    } else if !any_trusted {
        Err(SignerProblem::NotTrusted { key: signer.key.clone() })
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

/// Verify one signed event (`None` for an event that carries no signature
/// rule: `created`, `retired`, and `unretired`, which [`replay`] checks).
/// The `reset` of the founder may be unsigned; a `revoked`/`compromised`
/// may be signed by a maintainer or by the key's own reviewer (or the key
/// itself).
fn check_event(
    root: &ReviewRoot,
    state: &[ReviewerTrust],
    i: usize,
    j: usize,
    e: &KeyEvent,
    is_founder: bool,
) -> Option<Result<(String, String), SignerProblem>> {
    let r = &root.reviewers[i];
    let k = &r.keys[j];
    let signed = || -> Result<(&KeySigner, &str), SignerProblem> {
        match (&e.signer, &e.signature) {
            (Some(sg), Some(sig)) => Ok((sg, sig.as_str())),
            _ => Err(SignerProblem::Unsigned),
        }
    };
    let msg = event_signed_bytes(&r.id, k, e, "", Some(r));
    Some(match e.event {
        KeyEventKind::Created | KeyEventKind::Retired | KeyEventKind::Unretired => return None,
        KeyEventKind::Endorsed => signed()
            .and_then(|(sg, sig)| find_signer(state, &[i], sg, sig, &msg, None, &e.date)),
        KeyEventKind::Reset if is_founder && e.signature.is_none() => return None,
        KeyEventKind::Reset => signed().and_then(|(sg, sig)| {
            find_signer(state, &maintainers(state, None), sg, sig, &msg, Some(&e.date), &e.date)
        }),
        KeyEventKind::Admitted => signed().and_then(|(sg, sig)| {
            find_signer(state, &maintainers(state, Some(i)), sg, sig, &msg, Some(&e.date), &e.date)
        }),
        KeyEventKind::Revoked | KeyEventKind::Compromised => signed().and_then(|(sg, sig)| {
            if check_self_signed(r, k, e, "", state[i].keys[j].public.as_ref().ok()).is_ok() {
                return Ok((r.id.clone(), k.id.clone()));
            }
            let mut c = maintainers(state, None);
            if !c.contains(&i) {
                c.push(i);
            }
            find_signer(state, &c, sg, sig, &msg, Some(&e.date), &e.date)
        }),
    })
}

fn try_admit(root: &ReviewRoot, state: &[ReviewerTrust], i: usize) -> Result<Admission, AdmissionProblem> {
    let r = &root.reviewers[i];
    let k0 = r.keys.first().ok_or(AdmissionProblem::NoKey)?;
    let events: Vec<&KeyEvent> =
        k0.history.iter().filter(|e| e.event == KeyEventKind::Admitted).collect();
    if events.is_empty() {
        return Err(AdmissionProblem::Unadmitted);
    }
    let date = r.admitted.as_deref().ok_or(AdmissionProblem::Undated)?;
    if !is_date(date) {
        return Err(AdmissionProblem::BadDate(date.to_string()));
    }
    let mut last = AdmissionProblem::Unadmitted;
    for e in events {
        if e.date != date {
            last = AdmissionProblem::DateMismatch { event: e.date.clone(), admitted: date.to_string() };
            continue;
        }
        match check_event(root, state, i, 0, e, false) {
            Some(Ok((by, key))) => return Ok(Admission::Admitted { by, key }),
            Some(Err(p)) => last = AdmissionProblem::Signer(p),
            None => {}
        }
    }
    Err(last)
}

fn try_key(
    root: &ReviewRoot,
    state: &[ReviewerTrust],
    i: usize,
    j: usize,
    is_founder: bool,
) -> Result<KeyStatus, KeyProblem> {
    if j == 0 {
        return Ok(if is_founder { KeyStatus::TrustedOnFirstUse } else { KeyStatus::Admitted });
    }
    let k = &root.reviewers[i].keys[j];
    let mut last = KeyProblem::Unendorsed;
    for e in k.history.iter().filter(|e| matches!(e.event, KeyEventKind::Endorsed | KeyEventKind::Reset)) {
        if e.event == KeyEventKind::Reset && is_founder && e.signature.is_none() {
            return Ok(KeyStatus::ResetTrustedOnFirstUse);
        }
        if e.signature.is_none() {
            // Unsigned: no endorsement (and a warning from the final pass).
            continue;
        }
        match check_event(root, state, i, j, e, is_founder) {
            Some(Ok((by_reviewer, by_key))) => {
                // A key never endorses itself: `find_signer` only accepts
                // trusted keys, and this one is not trusted yet.
                return Ok(KeyStatus::Endorsed { by_reviewer, by_key });
            }
            Some(Err(p)) => last = KeyProblem::Endorsement(p),
            None => {}
        }
    }
    Err(last)
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
    let signer = KeySigner { reviewer: Some(rv.by.clone()), key: ks.key.clone() };
    let (_, key) = find_signer(state, &[by], &signer, &ks.signature, &msg, as_of, &rv.date)
        .map_err(RevocationProblem::Signer)?;
    Ok(key)
}
