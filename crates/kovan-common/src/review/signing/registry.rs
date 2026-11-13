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
//!   **GitHub #1248 (2026-11-05):** ~~the~~ founders are a list
//!   (`founders = [...]`; the old `founder` reads as a one-person list),
//!   each checked and trusted on first use as above; later founders are
//!   added and retired by signed `[[code_review.founder_change]]` entries,
//!   or elected by a maintainer ballot when no founder has an active key,
//!   or, as the unsigned last resort, added by a reviewed commit. A retired
//!   founder keeps founder standing for everything dated before the
//!   retirement. The rules are [`super::founders`]'s; the standing is
//!   [`Registry::lineage`] and [`Registry::is_founder_on`].
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
//!   ~~A `retired` event is unsigned and always honoured (it only takes
//!   trust away).~~ **CORRECTED 2026-11-05 (GitHub #1249):** an unsigned
//!   retirement let anyone who can edit `kovan_root.toml` switch off
//!   someone else's key. A retirement dated on or after
//!   [`SIGNED_KEY_RETIREMENT_FROM`] counts only when it is signed (over
//!   [`super::key_event_bytes`]) by the key's owner (any of their trusted
//!   keys, the retiring key included), a founder, or a maintainer, each
//!   active on its date; otherwise it has **no effect** and is a
//!   [`HistoryProblem::RetirementNoEffect`] warning. One dated before it
//!   (or undated, from a v1 `retired` field) keeps counting as before, and
//!   when unsigned is flagged forever ([`HistoryProblem::UnsignedRetirementLegacy`]).
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
//!
//! # Standing after admission: the roster events (GitHub #1019)
//!
//! The admission is never rewritten. A reviewer's later changes of role and
//! scope, novice marks and promotions are `[[code_review.roster_event]]`
//! entries ([`RosterEvent`]), one list for every reviewer, appended in
//! order and replayed in that order ([`ReviewerTrust::changes`]):
//!
//! ```text
//!   admission (role, scope)  ─ scope ─ novice ─ role=maintainer ─ trusted …
//!   role_at(d), scope_at(d), novice_at(d, path): the admitted values,
//!   replayed through the verified events dated on or before d
//! ```
//!
//! - **Signed by a maintainer just before the event:** an admitted
//!   reviewer whose role, replayed through the events before this one, is
//!   maintainer, with a trusted key active at the event's date, not
//!   revoked by then ([`find_signer`]). So in a same-day handover, A signs
//!   B's promotion and then A's own step-down, in that order.
//! - **Unsigned, forged, out of order or by a non-maintainer:** no effect,
//!   and a [`Warning::RosterEvent`]. (A novice mark only takes trust away,
//!   but it is still not honoured unsigned: a mark anyone could add by hand
//!   would let anyone stall another reviewer's stamps.)
//! - **Judged on the stamp's date:** a stamp counts under the role and
//!   scope in force on its date ([`ReviewerTrust::may_stamp`]; within one
//!   date, the more permissive of its start and end, since events carry no
//!   time), so widening or narrowing a scope later
//!   never changes an old stamp's verdict, and a maintainer who steps down
//!   keeps every stamp made while they were one. Admissions, resets and
//!   revocations are likewise authorised by the signer's role on their own
//!   date ([`maintainers`]).
//! - **Novice** (maintainer, #1019, "People need to earn their trust"): a
//!   reviewer is a novice for a path on a date when the last verified
//!   `novice`/`trusted` event dated on or before it whose scope covers the
//!   path (empty covers everything) is `novice`. Everyone starts trusted,
//!   and a maintainer is never a novice. The staleness engine then needs a
//!   second, trusted sign-off ([`crate::review::engine`]).

use std::collections::BTreeSet;

use ed25519_dalek::VerifyingKey;

use super::super::root::{
    KeyEvent, KeyEventKind, KeySigner, LegacyConflict, ReviewRoot, Reviewer, ReviewerKey, Revocation,
    Role, RosterEvent, RosterEventKind,
};
use super::super::scope::in_scope;
use super::founders::{self, BallotTrust, FounderChangeProblem, Lineage, VoteProblem};
use super::{
    decode_public, event_signed_bytes, is_date, key_event_bytes, revocation_bytes, roster_event_bytes,
    verify_bytes, CryptoError, UnverifiedReason, ALG,
};

/// From this date a key retirement counts only when signed (GitHub #1249,
/// maintainer 2026-11-05: "New retirements count only if signed by the
/// key's owner, a founder or a maintainer"; "Old unsigned retirements keep
/// counting, flagged"). The day after the direction, so a retirement the
/// running kovan wrote unsigned on the day it was given still counts
/// (flagged). An unsigned retirement backdated before it also still counts:
/// it is flagged forever as [`HistoryProblem::UnsignedRetirementLegacy`],
/// and git shows when it was written (Leak Before Break).
pub const SIGNED_KEY_RETIREMENT_FROM: &str = "2026-11-06";

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
    /// A date is not `YYYY-MM-DD` (a malformed revocation voids the key
    /// from the start; ~~so does a malformed retirement~~ **CORRECTED
    /// 2026-11-05, GitHub #1249:** a retirement does only when undated, the
    /// v1 form; any other malformed date is not an old retirement, so it
    /// must be signed, and kovan cannot sign one).
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
    /// GitHub #1249: an **unsigned** retirement dated before
    /// [`SIGNED_KEY_RETIREMENT_FROM`] (or undated, from v1). It still
    /// counts, as it always did, and is shown forever.
    UnsignedRetirementLegacy { index: usize },
    /// GitHub #1249: a retirement dated on or after
    /// [`SIGNED_KEY_RETIREMENT_FROM`] that is unsigned, or not signed by the
    /// key's owner, a founder or a maintainer active on its date: it has
    /// **no effect** (the key stays active).
    RetirementNoEffect { index: usize, problem: SignerProblem },
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

/// A verified roster event: a change to a reviewer's standing (module doc).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosterChange {
    /// Its position in `[[code_review.roster_event]]`.
    pub index: usize,
    pub date: String,
    pub kind: RosterEventKind,
    pub role: Option<Role>,
    pub scope: Vec<String>,
    /// The maintainer and key that signed it.
    pub by: String,
    pub key: String,
}

/// Why a roster event has no effect (listed as a warning).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RosterProblem {
    /// It names no `[[reviewer]]`.
    UnknownReviewer,
    /// `date` is not `YYYY-MM-DD`.
    BadDate(String),
    /// Dated before the entry before it (the list is append-only).
    OutOfOrder,
    /// A `role` event without a role.
    MissingRole,
    /// The signature does not count: unsigned, forged, by someone who was
    /// not a maintainer just before it, or by a key retired or revoked then.
    Signer(SignerProblem),
}

/// One reviewer, judged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewerTrust {
    pub id: String,
    /// The **admitted** role (signed by the admission); the role in force
    /// on a date is [`Self::role_at`].
    pub role: Role,
    /// The **admitted** scope; the one in force is [`Self::scope_at`].
    pub scope: Vec<String>,
    pub admission: Admission,
    /// The reviewer's `admitted` date as written (GitHub #1248: whether
    /// they were admitted by a ballot's opening date).
    pub admitted: Option<String>,
    pub keys: Vec<KeyTrust>,
    pub revocation: Option<RevocationTrust>,
    /// The verified roster events about this reviewer, in list order
    /// (GitHub #1019; module doc).
    pub changes: Vec<RosterChange>,
}

/// The role after replaying `changes` onto `base`.
fn role_after<'c>(base: Role, changes: impl Iterator<Item = &'c RosterChange>) -> Role {
    changes.fold(base, |r, c| match (c.kind, c.role) {
        (RosterEventKind::Role, Some(n)) => n,
        _ => r,
    })
}

impl ReviewerTrust {
    /// The role in force on `date`: the admitted one, replayed through the
    /// verified events dated on or before it.
    pub fn role_at(&self, date: &str) -> Role {
        role_after(self.role, self.changes.iter().filter(|c| c.date.as_str() <= date))
    }

    /// The scope in force on `date` (the admitted one, then the last
    /// `scope` event dated on or before it).
    pub fn scope_at(&self, date: &str) -> &[String] {
        self.changes
            .iter()
            .rev()
            .find(|c| c.kind == RosterEventKind::Scope && c.date.as_str() <= date)
            .map_or(self.scope.as_slice(), |c| c.scope.as_slice())
    }

    /// Whether a stamp dated `date` may certify `path` (GitHub #1019): a
    /// maintainer anywhere, a reviewer within scope, under the role and
    /// scope in force on that date. Events carry a date and no time, so a
    /// stamp counts under the more permissive of the standing at the start
    /// of its date (events dated before it) and at the end (events dated on
    /// or before it): a change never voids a stamp made the same day, and
    /// a widening counts from its own day. With no roster events both are
    /// the admitted role and scope, as before.
    pub fn may_stamp(&self, date: &str, path: &str) -> bool {
        self.may_stamp_where(date, Some(path))
    }

    /// Whether the reviewer was a maintainer at the start or the end of
    /// `date` (a maintainer may sign a node with no member paths).
    pub fn maintainer_on(&self, date: &str) -> bool {
        self.may_stamp_where(date, None)
    }

    fn may_stamp_where(&self, date: &str, path: Option<&str>) -> bool {
        let ok = |role: Role, scope: &[String]| role == Role::Maintainer || path.is_some_and(|p| in_scope(scope, p));
        let before: Vec<&RosterChange> = self.changes.iter().filter(|c| c.date.as_str() < date).collect();
        let scope_before = before
            .iter()
            .rev()
            .find(|c| c.kind == RosterEventKind::Scope)
            .map_or(self.scope.as_slice(), |c| c.scope.as_slice());
        ok(self.role_at(date), self.scope_at(date)) || ok(role_after(self.role, before.into_iter()), scope_before)
    }

    /// The role in force now (after every event).
    pub fn current_role(&self) -> Role {
        role_after(self.role, self.changes.iter())
    }

    /// The scope in force now.
    pub fn current_scope(&self) -> &[String] {
        self.changes
            .iter()
            .rev()
            .find(|c| c.kind == RosterEventKind::Scope)
            .map_or(self.scope.as_slice(), |c| c.scope.as_slice())
    }

    /// Whether the reviewer is a **novice** for `path` on `date` (module
    /// doc): never as a maintainer then; else the last verified
    /// `novice`/`trusted` event dated on or before `date` whose scope
    /// covers `path` (empty: everywhere) decides; none: trusted.
    pub fn novice_at(&self, date: &str, path: &str) -> bool {
        if self.role_at(date) == Role::Maintainer {
            return false;
        }
        self.changes
            .iter()
            .rev()
            .filter(|c| c.date.as_str() <= date)
            .filter(|c| matches!(c.kind, RosterEventKind::Novice | RosterEventKind::Trusted))
            .find(|c| c.scope.is_empty() || in_scope(&c.scope, path))
            .is_some_and(|c| c.kind == RosterEventKind::Novice)
    }

    /// The novice mark in force now, for the roster display (`novice_at`
    /// is the rule): where the reviewer is a novice (empty: everywhere),
    /// and where a later promotion made them trusted again; `None` when no
    /// mark stands (or they are a maintainer now).
    pub fn novice_now(&self) -> Option<(Vec<String>, Vec<String>)> {
        if self.current_role() == Role::Maintainer {
            return None;
        }
        let mut mark: Option<(Vec<String>, Vec<String>)> = None;
        for c in &self.changes {
            match c.kind {
                RosterEventKind::Novice => mark = Some((c.scope.clone(), Vec::new())),
                RosterEventKind::Trusted if c.scope.is_empty() => mark = None,
                RosterEventKind::Trusted => {
                    if let Some((_, except)) = mark.as_mut() {
                        except.extend(c.scope.iter().cloned());
                    }
                }
                RosterEventKind::Role | RosterEventKind::Scope => {}
            }
        }
        mark
    }

    /// Whether the reviewer can sign anything on `date` (GitHub #1248):
    /// admitted (a founder from the start, anyone else from their
    /// `admitted` date), not revoked by then, and a trusted key active
    /// then.
    pub fn active_on(&self, date: &str) -> bool {
        let admitted = match &self.admission {
            Admission::Founder => true,
            Admission::Admitted { .. } => self.admitted.as_deref().is_none_or(|d| d <= date),
            Admission::NotAdmitted(_) => false,
        };
        admitted
            && self.revocation.as_ref().is_none_or(|v| v.cutoff(date).is_none())
            && self.keys.iter().any(|k| k.status.is_trusted() && k.inactive_at(date).is_none())
    }

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
    /// Roster event `index` has no effect (GitHub #1019).
    RosterEvent { index: usize, reviewer: String, problem: RosterProblem },
    /// GitHub #1248: a declared founder (after the first problem, which is
    /// [`Warning::NoFounder`] when no declared founder passes) that fails
    /// the founder checks: not a founder.
    FounderListed(FounderProblem),
    /// GitHub #1248: `[[code_review.founder_change]]` entry `index` has no
    /// effect.
    FounderChange { index: usize, founder: String, problem: FounderChangeProblem },
    /// GitHub #1248: a founder made by the unsigned last resort, shown
    /// forever: "trust root replaced by repository commit".
    TrustRootReplacedByCommit { founder: String, date: String, reviewed_in: Option<String> },
    /// GitHub #1248: every founder ballot, shown from the moment it is in
    /// the file (open, elected, void or without a result).
    FounderBallot(BallotTrust),
    /// GitHub #1248: `[[code_review.founder_vote]]` entry `index` does not
    /// count.
    FounderVote { index: usize, voter: String, problem: VoteProblem },
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

/// Append an **unsigned** `retired` event. ~~Unsigned: retiring only takes
/// trust away; no private key needed, so a lost key can be retired.~~
/// **CORRECTED 2026-11-05 (GitHub #1249):** an unsigned retirement dated on
/// or after [`SIGNED_KEY_RETIREMENT_FROM`] has no effect; kovan signs
/// retirements with `keystore::UnlockedKey::retire_key` (by the owner, a
/// founder or a maintainer, so a lost key is still retired by someone
/// else). This stays for v1-era histories and tests.
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
    /// The first declared founder that passes the founder checks (or, when
    /// none does, why the first one fails). ~~The founder~~ Since GitHub
    /// #1248 there may be several, and they change over time:
    /// [`Self::lineage`] and [`Self::is_founder_on`] are the rule.
    pub founder: Result<String, FounderProblem>,
    pub reviewers: Vec<ReviewerTrust>,
    /// Roster events with no effect, and why (GitHub #1019).
    pub roster_problems: Vec<(usize, String, RosterProblem)>,
    /// GitHub #1248: every declared founder that fails the founder checks.
    pub declared_problems: Vec<FounderProblem>,
    /// GitHub #1248: the founders over time, the ballots and the lineage
    /// fingerprint ([`super::founders`]).
    pub lineage: Lineage,
}

impl Registry {
    /// Judge every reviewer and key in `root` (module doc). The v1 legacy
    /// fields are migrated into history on a copy first.
    pub fn build(root: &ReviewRoot) -> Registry {
        let mut root = root.clone();
        let conflicts = root.migrate_key_history();
        let root = &root;
        let declared = root.code_review.as_ref().map(|c| c.declared_founders()).unwrap_or_default();
        let checked: Vec<Result<usize, FounderProblem>> = declared.iter().map(|id| check_founder(root, id)).collect();
        let initial: Vec<usize> = checked.iter().filter_map(|c| c.as_ref().ok().copied()).collect();
        let founder = match (initial.first(), checked.first()) {
            (Some(&i), _) => Ok(root.reviewers[i].id.clone()),
            (None, Some(Err(p))) => Err(p.clone()),
            _ => Err(FounderProblem::NotDeclared),
        };
        let declared_problems: Vec<FounderProblem> = checked.into_iter().filter_map(Result::err).collect();
        // GitHub #1249: which new retirements have no effect depends on who
        // could sign them, which depends on which keys are retired. Start
        // with every retirement in effect, drop the ones that do not
        // verify, and judge again until the set settles (bounded; the last
        // round stands, as in the trust fixed point). GitHub #1248: the
        // founders trusted on first use (the declared ones, plus a founder
        // by the unsigned last resort who has no admission) settle in the
        // same loop.
        let mut ignored: BTreeSet<(usize, usize, usize)> = BTreeSet::new();
        let mut tofu: BTreeSet<usize> = initial.iter().copied().collect();
        let mut rounds = 0usize;
        loop {
            rounds += 1;
            let (mut state, roster_problems) = judge(root, &conflicts, &tofu, &ignored);
            let lineage = founders::replay(root, &state, &declared, &initial);
            let verdicts = retirement_verdicts(root, &state, &lineage);
            let next: BTreeSet<(usize, usize, usize)> = verdicts
                .iter()
                .filter(|(_, v)| matches!(v, Err(HistoryProblem::RetirementNoEffect { .. })))
                .map(|(at, _)| *at)
                .collect();
            let next_tofu: BTreeSet<usize> = initial.iter().chain(&lineage.first_use).copied().collect();
            if (next == ignored && next_tofu == tofu) || rounds >= 8 {
                for ((i, j, _), v) in verdicts {
                    if let Err(p) = v {
                        state[i].keys[j].history.push(p);
                    }
                }
                return Registry { founder, reviewers: state, roster_problems, declared_problems, lineage };
            }
            ignored = next;
            tofu = next_tofu;
        }
    }

    /// The reviewer registered under `id`.
    pub fn reviewer(&self, id: &str) -> Option<&ReviewerTrust> {
        self.reviewers.iter().find(|r| r.id == id)
    }

    /// Whether `id` has founder standing on `date` (GitHub #1248): a
    /// retired founder keeps it for everything dated before the retirement.
    pub fn is_founder_on(&self, id: &str, date: &str) -> bool {
        self.lineage.standings.iter().any(|s| s.id == id && s.covers(date))
    }

    /// Whether `id` can act as a founder on `date`: founder standing then,
    /// a maintainer then, admitted, not revoked, with a trusted key active
    /// then ([`ReviewerTrust::active_on`]).
    pub fn founder_can_act(&self, id: &str, date: &str) -> bool {
        self.is_founder_on(id, date)
            && self.reviewer(id).is_some_and(|r| r.role_at(date) == Role::Maintainer && r.active_on(date))
    }

    /// A founder who can act on `date`, if any (a ballot is refused then).
    pub fn active_founder_on(&self, date: &str) -> Option<&str> {
        self.lineage
            .standings
            .iter()
            .find(|s| self.founder_can_act(&s.id, date))
            .map(|s| s.id.as_str())
    }

    /// The founders in standing now (not retired), in order, each once.
    pub fn current_founders(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for s in self.lineage.current() {
            if !out.contains(&s.id) {
                out.push(s.id.clone());
            }
        }
        out
    }

    /// Every problem and every reset, for the loud display.
    pub fn warnings(&self) -> Vec<Warning> {
        let mut out = Vec::new();
        if let Err(p) = &self.founder {
            out.push(Warning::NoFounder(p.clone()));
        }
        let skip = usize::from(self.founder.is_err());
        for p in self.declared_problems.iter().skip(skip) {
            out.push(Warning::FounderListed(p.clone()));
        }
        for s in &self.lineage.standings {
            if let founders::FounderOrigin::RepositoryCommit { reviewed_in, .. } = &s.origin {
                out.push(Warning::TrustRootReplacedByCommit {
                    founder: s.id.clone(),
                    date: s.from.clone().unwrap_or_default(),
                    reviewed_in: reviewed_in.clone(),
                });
            }
        }
        for (index, founder, problem) in &self.lineage.change_problems {
            out.push(Warning::FounderChange { index: *index, founder: founder.clone(), problem: problem.clone() });
        }
        for b in &self.lineage.ballots {
            out.push(Warning::FounderBallot(b.clone()));
        }
        for (index, voter, problem) in &self.lineage.vote_problems {
            out.push(Warning::FounderVote { index: *index, voter: voter.clone(), problem: problem.clone() });
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
        for (index, reviewer, problem) in &self.roster_problems {
            out.push(Warning::RosterEvent { index: *index, reviewer: reviewer.clone(), problem: problem.clone() });
        }
        out
    }
}

/// One judgement of every reviewer and key, with the retirements in
/// `ignored` (reviewer, key, history index) left out of the void windows
/// ([`Registry::build`]).
fn judge(
    root: &ReviewRoot,
    conflicts: &[LegacyConflict],
    tofu: &BTreeSet<usize>,
    ignored: &BTreeSet<(usize, usize, usize)>,
) -> (Vec<ReviewerTrust>, Vec<(usize, String, RosterProblem)>) {
    let mut state: Vec<ReviewerTrust> = root
        .reviewers
        .iter()
        .enumerate()
        .map(|(i, r)| ReviewerTrust {
            id: r.id.clone(),
            role: r.role,
            scope: r.scope.clone(),
            admission: Admission::NotAdmitted(AdmissionProblem::Unadmitted),
            admitted: r.admitted.clone(),
            keys: r
                .keys
                .iter()
                .enumerate()
                .map(|(j, k)| {
                    let skip: BTreeSet<usize> =
                        ignored.iter().filter(|(a, b, _)| (*a, *b) == (i, j)).map(|(_, _, e)| *e).collect();
                    initial_key(r, j, k, &skip)
                })
                .collect(),
            revocation: r.revoked.as_ref().map(|rv| RevocationTrust {
                date: rv.date.clone(),
                compromised_from: rv.compromised_from.clone(),
                by: rv.by.clone(),
                signed: Err(RevocationProblem::Signer(SignerProblem::Unsigned)),
            }),
            changes: Vec::new(),
        })
        .collect();
    for c in conflicts {
        if let Some(k) = state
            .iter_mut()
            .find(|r| r.id == c.reviewer)
            .and_then(|r| r.keys.iter_mut().find(|k| k.id == c.key))
        {
            k.history.push(HistoryProblem::LegacyFieldsIgnored);
        }
    }
    let events = root.code_review.as_ref().map_or(&[][..], |c| c.roster_events.as_slice());
    let mut roster_problems: Vec<(usize, String, RosterProblem)>;
    // A role change can take authority away, so the fixed point is not
    // monotone in general; it is bounded (a well-formed history settles
    // in a few rounds), and the last round's result stands.
    let mut rounds = 0usize;
    let cap = 8 + 2 * (state.len() + events.len());
    loop {
        rounds += 1;
        let mut changed = false;
        let (changes, problems) = replay_roster(events, &state);
        for (r, c) in state.iter_mut().zip(changes) {
            if r.changes != c {
                r.changes = c;
                changed = true;
            }
        }
        roster_problems = problems;
        for i in 0..state.len() {
            if !state[i].is_admitted() {
                let res = if tofu.contains(&i) {
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
                    try_key(root, &state, i, j, tofu.contains(&i))
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
        if !changed || rounds >= cap {
            break;
        }
    }
    // Every signed event on its own, so a tampered entry shows even
    // when another one already made the key trusted.
    let mut found = Vec::new();
    for (i, r) in root.reviewers.iter().enumerate() {
        for (j, k) in r.keys.iter().enumerate() {
            for (index, ev) in k.history.iter().enumerate() {
                if let Some(Err(problem)) = check_event(root, &state, i, j, ev, tofu.contains(&i)) {
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
    (state, roster_problems)
}

/// Whether a retirement dated `date` is from before signed retirements
/// ([`SIGNED_KEY_RETIREMENT_FROM`]): a date before it, or the empty date a
/// v1 `retired` field migrates to.
pub fn retirement_is_legacy(date: &str) -> bool {
    date.is_empty() || (is_date(date) && date < SIGNED_KEY_RETIREMENT_FROM)
}

/// The verdict on every `retired` event (GitHub #1249; module doc), keyed
/// by (reviewer, key, history index): `Ok` when it counts with nothing to
/// show, else the history problem to show (a
/// [`HistoryProblem::RetirementNoEffect`] is the one that does not count).
fn retirement_verdicts(
    root: &ReviewRoot,
    state: &[ReviewerTrust],
    lineage: &Lineage,
) -> Vec<((usize, usize, usize), Result<(), HistoryProblem>)> {
    let mut out = Vec::new();
    for (i, r) in root.reviewers.iter().enumerate() {
        for (j, k) in r.keys.iter().enumerate() {
            for (index, e) in k.history.iter().enumerate() {
                if e.event != KeyEventKind::Retired {
                    continue;
                }
                let signed = check_retirement(root, state, i, j, e, lineage);
                let v = match (retirement_is_legacy(&e.date), signed) {
                    (_, Ok(_)) => Ok(()),
                    (true, Err(SignerProblem::Unsigned)) => Err(HistoryProblem::UnsignedRetirementLegacy { index }),
                    (true, Err(problem)) => Err(HistoryProblem::BadEventSignature { index, event: e.event, problem }),
                    (false, Err(problem)) => Err(HistoryProblem::RetirementNoEffect { index, problem }),
                };
                out.push(((i, j, index), v));
            }
        }
    }
    out
}

/// Who signed retirement `e` of key `j` of reviewer `i`: the key's owner
/// (any trusted key of theirs, the retiring one included), a founder in
/// standing on its date (GitHub #1248), or a maintainer, active and not
/// revoked on its date (GitHub #1249).
fn check_retirement(
    root: &ReviewRoot,
    state: &[ReviewerTrust],
    i: usize,
    j: usize,
    e: &KeyEvent,
    lineage: &Lineage,
) -> Result<(String, String), SignerProblem> {
    let (Some(sg), Some(sig)) = (&e.signer, &e.signature) else {
        return Err(SignerProblem::Unsigned);
    };
    let r = &root.reviewers[i];
    let msg = key_event_bytes(&r.id, &r.keys[j], e, "", None);
    let mut c = maintainers(state, None, &e.date);
    let founders = lineage.founders_on(&e.date).filter_map(|s| state.iter().position(|r| r.id == s.id));
    for x in std::iter::once(i).chain(founders) {
        if !c.contains(&x) {
            c.push(x);
        }
    }
    // The retiring key signs its own retirement while still active: its
    // own window does not count against it.
    let mut st = state.to_vec();
    st[i].keys[j]
        .windows
        .retain(|w| !(w.kind == InactiveKind::Retired && w.from.as_deref() == Some(e.date.as_str())));
    find_signer(&st, &c, sg, sig, &msg, Some(&e.date), &e.date)
}

/// The founder checks on one declared founder (#762; GitHub #1248 applies
/// them to each of the list): a listed maintainer with no admission. `Ok`:
/// its position in the root.
fn check_founder(root: &ReviewRoot, id: &str) -> Result<usize, FounderProblem> {
    let pos = root.reviewers.iter().position(|r| r.id == id).ok_or_else(|| FounderProblem::Unknown(id.to_string()))?;
    let f = &root.reviewers[pos];
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
    Ok(pos)
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

fn initial_key(r: &Reviewer, j: usize, k: &ReviewerKey, skip: &BTreeSet<usize>) -> KeyTrust {
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
    let (windows, history) = replay(r, j, k, public.as_ref().ok(), skip);
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
/// no trust from anyone else). The `retired` entries at the indices in
/// `skip` have no effect (GitHub #1249), and neither has the `unretired`
/// that ends one.
fn replay(
    r: &Reviewer,
    j: usize,
    k: &ReviewerKey,
    public: Option<&VerifyingKey>,
    skip: &BTreeSet<usize>,
) -> (Vec<Window>, Vec<HistoryProblem>) {
    let mut windows = Vec::new();
    let mut problems = Vec::new();
    if k.history.first().map(|e| e.event) != Some(KeyEventKind::Created) {
        problems.push(HistoryProblem::MissingCreated);
    }
    let mut prev: Option<&str> = None;
    let mut open: Option<Option<String>> = None;
    let mut skipped_open = false;
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
            KeyEventKind::Retired if skip.contains(&index) => skipped_open = open.is_none(),
            KeyEventKind::Unretired if open.is_none() && skipped_open => skipped_open = false,
            KeyEventKind::Retired => {
                skipped_open = false;
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
pub(crate) fn find_signer(
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

/// Admitted reviewers other than `except` whose role on `date` is
/// maintainer ([`ReviewerTrust::role_at`]; with no roster events, the
/// admitted role, as before GitHub #1019).
pub(crate) fn maintainers(state: &[ReviewerTrust], except: Option<usize>, date: &str) -> Vec<usize> {
    (0..state.len())
        .filter(|&m| Some(m) != except && state[m].role_at(date) == Role::Maintainer && state[m].is_admitted())
        .collect()
}

/// Replay the roster events in list order (module doc): each reviewer's
/// verified changes, and every event with no effect.
fn replay_roster(events: &[RosterEvent], state: &[ReviewerTrust]) -> (Vec<Vec<RosterChange>>, Vec<(usize, String, RosterProblem)>) {
    let mut changes: Vec<Vec<RosterChange>> = vec![Vec::new(); state.len()];
    let mut problems = Vec::new();
    let mut prev: Option<&str> = None;
    for (index, e) in events.iter().enumerate() {
        let mut fail = |p: RosterProblem| problems.push((index, e.reviewer.clone(), p));
        let Some(t) = state.iter().position(|r| r.id == e.reviewer) else {
            fail(RosterProblem::UnknownReviewer);
            continue;
        };
        if !is_date(&e.date) {
            fail(RosterProblem::BadDate(e.date.clone()));
            continue;
        }
        if prev.is_some_and(|p| e.date.as_str() < p) {
            fail(RosterProblem::OutOfOrder);
            continue;
        }
        prev = Some(&e.date);
        if e.event == RosterEventKind::Role && e.role.is_none() {
            fail(RosterProblem::MissingRole);
            continue;
        }
        let (Some(sg), Some(sig)) = (&e.signer, &e.signature) else {
            fail(RosterProblem::Signer(SignerProblem::Unsigned));
            continue;
        };
        // Maintainers just before this event: the admitted role replayed
        // through the events verified so far.
        let eligible: Vec<usize> = (0..state.len())
            .filter(|&m| state[m].is_admitted() && role_after(state[m].role, changes[m].iter()) == Role::Maintainer)
            .collect();
        match find_signer(state, &eligible, sg, sig, &roster_event_bytes(e), Some(&e.date), &e.date) {
            Ok((by, key)) => changes[t].push(RosterChange {
                index,
                date: e.date.clone(),
                kind: e.event,
                role: e.role,
                scope: e.scope.clone(),
                by,
                key,
            }),
            Err(p) => fail(RosterProblem::Signer(p)),
        }
    }
    (changes, problems)
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
            find_signer(state, &maintainers(state, None, &e.date), sg, sig, &msg, Some(&e.date), &e.date)
        }),
        KeyEventKind::Admitted => signed().and_then(|(sg, sig)| {
            find_signer(state, &maintainers(state, Some(i), &e.date), sg, sig, &msg, Some(&e.date), &e.date)
        }),
        KeyEventKind::Revoked | KeyEventKind::Compromised => signed().and_then(|(sg, sig)| {
            if check_self_signed(r, k, e, "", state[i].keys[j].public.as_ref().ok()).is_ok() {
                return Ok((r.id.clone(), k.id.clone()));
            }
            let mut c = maintainers(state, None, &e.date);
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
        .position(|m| m.id == rv.by && m.role_at(&rv.date) == Role::Maintainer && m.is_admitted())
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
