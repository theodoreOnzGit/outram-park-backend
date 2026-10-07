//! **Stamp signatures** (GitHub #762; design on #739, comments "Stamp
//! signing inside kovan", "Many maintainers" and "D6", 2026-10-07): the
//! stored field, the exact bytes that get signed, and verification.
//!
//! ~~The cryptography itself is GitHub #762 and is **not here**:
//! [`verify_review`] and [`verify_architecture`] always answer
//! [`SignatureCheck::Unverified`], with the reason, until #762 lands.~~
//! **CORRECTED 2026-10-07 (#762)**: [`verify_review`] and
//! [`verify_architecture`] check the ed25519 signature against the
//! `[[reviewer]]` registry ([`registry::Registry`]) and answer
//! [`SignatureCheck::Verified`] only when every rule below holds.
//!
//! # Who does what
//!
//! ```text
//!   desktop kovan (native only)              everywhere (desktop, web-kovan wasm, CI)
//!   ───────────────────────────              ────────────────────────────────────────
//!   keystore::generate ──> KeyFile (argon2id + AES-256-GCM, kovan config dir)
//!   KeyFile::unlock(passphrase) ──> UnlockedKey (memory only, while the app is open)
//!   UnlockedKey::sign_review ──> [review.signature] ──> verify_review(entry, &Registry)
//!   UnlockedKey::endorse/admit/revoke/unretire ──> kovan_root.toml ──> Registry::build
//! ```
//!
//! `kovan-cli` has no signing command; `keystore` does not exist on wasm.
//!
//! # What a verified stamp needs
//!
//! 1. a signature, `alg = "ed25519"`, by a key listed under the stamp's
//!    `by` reviewer, that verifies over [`signed_bytes`] (any edit to a
//!    signed field, or another key, fails: [`UnverifiedReason::BadSignature`]);
//! 2. a reviewer admitted by the registry, and a trusted key
//!    ([`registry::KeyStatus`]): an unendorsed key's signatures do not count;
//! 3. a stamp `date` before the reviewer's revocation, and before the
//!    compromise date of a compromised one; outside the key's retired window;
//! 4. for a `reviewer` (not a `maintainer`), every path it certifies inside
//!    its `scope` globs ([`super::scope`]).
//!
//! **Time-bound stamps (#739 D6):** `commit` and `hash` are signed fields.
//! Checking that the function's hash *at that commit* is `hash`, and that the
//! stamp arrived in a later commit, is the staleness engine's job (#765);
//! this module only proves the reviewer signed those values. The agent
//! trailer stays a secondary signal ([`super::types::agent_trailer`]).
//!
//! # The signed bytes
//!
//! A UTF-8 text, one `key=value` line per field, in a fixed order, each
//! value written as a JSON string (so a newline or `=` inside a value cannot
//! forge a line), maps sorted by key:
//!
//! ```text
//! kovan-review-signature-v1
//! kind="review"
//! target="code:crates/x/src/a.rs::f"
//! function="crates/x/src/a.rs::f"
//! by="github:theodoreOnzGit"
//! rung="3"
//! date="2026-10-07"
//! commit="<sha>"
//! hash="sha256:…"
//! doc_hash="sha256:…"
//! cargo_lock="sha256:…"          (or "" when absent)
//! authorship="agent"              (or "" when absent)
//! session="https://…"            (one line per session, sorted)
//! callee="<id>" "<hash>"          (one line per callee, sorted by id)
//! checklist="<q>" "<answer>"      (one line per answer, sorted)
//! no_concept="…"                  (or "")
//! moved="<from>" "<commit>"       (one line per move, in order)
//! relation="<kind>" "<target>"    (one line per relation, in order)
//! ```
//!
//! The artifact id, `created`/`modified`, the Markdown comments and the
//! signature itself are not signed (the signing comment on #739 lists the
//! fields: target, hash, commit, `by`, date, checklist; the rest above are
//! the review's other certifying fields). The authorship of the reviewed
//! change is signed (maintainer, #764, 2026-10-07).
//!
//! The registry statements ([`key_event_bytes`] for every signed
//! `[[reviewer.key.history]]` event, [`revocation_bytes`] for a reviewer's
//! revocation; and the v1 [`endorsement_bytes`], [`admission_bytes`],
//! [`unretire_bytes`] that migrated `legacy` events were signed over) use
//! the same line format, each
//! under its own first line, so a signature over one kind of statement can
//! never be replayed as another.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use ed25519_dalek::{Signature as EdSignature, VerifyingKey};
use serde::{Deserialize, Serialize};

use super::review_md::{ArchitectureEntry, ReviewEntry};
use super::root::{KeyEvent, KeyEventKind, Revocation, Reviewer, ReviewerKey, Role};
use super::scope::in_scope;

pub mod registry;

#[cfg(not(target_arch = "wasm32"))]
pub mod keystore;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "signing/tests.rs"]
mod tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "signing/history_tests.rs"]
mod history_tests;

use registry::{KeyStatus, Registry};

/// The only signature algorithm (#739 signing comment).
pub const ALG: &str = "ed25519";

/// `[review.signature]` / `[architecture.signature]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature {
    /// The reviewer key id (`[[reviewer.key]] id` in `kovan_root.toml`).
    pub key: String,
    /// `ed25519`.
    pub alg: String,
    /// Base64 signature over [`signed_bytes`].
    pub value: String,
}

fn q(s: &str) -> String {
    serde_json::Value::String(s.to_string()).to_string()
}

fn line(out: &mut String, key: &str, values: &[&str]) {
    out.push_str(key);
    out.push('=');
    let quoted: Vec<String> = values.iter().map(|v| q(v)).collect();
    out.push_str(&quoted.join(" "));
    out.push('\n');
}

/// The bytes a review's signature is taken over (module doc).
pub fn signed_bytes(r: &ReviewEntry) -> Vec<u8> {
    let b = &r.review;
    let mut s = String::from("kovan-review-signature-v1\n");
    line(&mut s, "kind", &["review"]);
    line(&mut s, "target", &[r.kovan.target.as_deref().unwrap_or("")]);
    line(&mut s, "function", &[&b.function]);
    line(&mut s, "by", &[&b.by]);
    line(&mut s, "rung", &[&b.rung.to_string()]);
    line(&mut s, "date", &[&b.date]);
    line(&mut s, "commit", &[&b.commit]);
    line(&mut s, "hash", &[&b.hash]);
    line(&mut s, "doc_hash", &[&b.doc_hash]);
    line(&mut s, "cargo_lock", &[b.cargo_lock.as_deref().unwrap_or("")]);
    let (kind, sessions) = match &b.authorship {
        Some(a) => (a.kind.as_str(), a.sessions.clone()),
        None => ("", Vec::new()),
    };
    line(&mut s, "authorship", &[kind]);
    let mut sessions = sessions;
    sessions.sort();
    for sess in &sessions {
        line(&mut s, "session", &[sess]);
    }
    for (id, h) in &b.callees {
        line(&mut s, "callee", &[id, h]);
    }
    for (k, v) in &b.checklist {
        line(&mut s, "checklist", &[k, v]);
    }
    line(&mut s, "no_concept", &[b.no_concept.as_deref().unwrap_or("")]);
    for m in &b.moved {
        line(&mut s, "moved", &[&m.from, &m.commit]);
    }
    for rel in &r.relations {
        line(&mut s, "relation", &[rel.kind.as_str(), &rel.target]);
    }
    s.into_bytes()
}

/// The bytes an architecture node's signature is taken over: the same
/// line format, `kind="architecture"`, then by, date, commit, each member
/// (sorted), the upstream's repository and commit, the pattern, and the
/// relations.
pub fn architecture_signed_bytes(a: &ArchitectureEntry) -> Vec<u8> {
    let b = &a.architecture;
    let mut s = String::from("kovan-review-signature-v1\n");
    line(&mut s, "kind", &["architecture"]);
    line(&mut s, "id", &[&a.kovan.id]);
    line(&mut s, "by", &[&b.by]);
    line(&mut s, "date", &[&b.date]);
    line(&mut s, "commit", &[&b.commit]);
    let mut members = b.members.clone();
    members.sort();
    for m in &members {
        line(&mut s, "member", &[m]);
    }
    let (repo, commit) = match &b.upstream {
        Some(u) => (
            u.repository.clone().unwrap_or_default(),
            u.commit.clone().unwrap_or_default(),
        ),
        None => (String::new(), String::new()),
    };
    line(&mut s, "upstream", &[&repo, &commit]);
    line(&mut s, "pattern", &[b.pattern.as_deref().unwrap_or("")]);
    for rel in &a.relations {
        line(&mut s, "relation", &[rel.kind.as_str(), &rel.target]);
    }
    s.into_bytes()
}


/// *v1 (legacy).* The bytes an endorsement signed: `owner`'s key `k`
/// vouched for by an existing key (of the same reviewer, or a maintainer's
/// for a reset). Used only to verify migrated `legacy` events.
pub fn endorsement_bytes(owner: &str, k: &ReviewerKey) -> Vec<u8> {
    endorsement_bytes_with(owner, k, k.reset)
}

fn endorsement_bytes_with(owner: &str, k: &ReviewerKey, reset: bool) -> Vec<u8> {
    let mut s = String::from("kovan-key-endorsement-v1\n");
    line(&mut s, "reviewer", &[owner]);
    line(&mut s, "key", &[&k.id]);
    line(&mut s, "alg", &[&k.alg]);
    line(&mut s, "public", &[&k.public]);
    line(&mut s, "created", &[&k.created]);
    line(&mut s, "reset", &[if reset { "true" } else { "false" }]);
    s.into_bytes()
}

/// The bytes a maintainer signs to admit reviewer `r` (as the v1
/// `admitted_by`, and inside an `admitted` [`key_event_bytes`]): its id, role,
/// admission date, every scope glob (in order) and its **first key**, so the
/// admission is also what makes that key trusted. Widening a scope or
/// changing the role needs a new admission signature; `qualification` and
/// `name` are citations and display, not authority, and are not signed.
pub fn admission_bytes(r: &Reviewer) -> Vec<u8> {
    let mut s = String::from("kovan-reviewer-admission-v1\n");
    line(&mut s, "reviewer", &[&r.id]);
    line(&mut s, "role", &[role_str(r.role)]);
    line(&mut s, "admitted", &[r.admitted.as_deref().unwrap_or("")]);
    for g in &r.scope {
        line(&mut s, "scope", &[g]);
    }
    match r.keys.first() {
        Some(k) => line(&mut s, "key", &[&k.id, &k.public]),
        None => line(&mut s, "key", &["", ""]),
    }
    s.into_bytes()
}

/// The bytes a maintainer signs to revoke reviewer `reviewer`.
pub fn revocation_bytes(reviewer: &str, rev: &Revocation) -> Vec<u8> {
    let mut s = String::from("kovan-reviewer-revocation-v1\n");
    line(&mut s, "reviewer", &[reviewer]);
    line(&mut s, "date", &[&rev.date]);
    line(&mut s, "compromised_from", &[rev.compromised_from.as_deref().unwrap_or("")]);
    line(&mut s, "by", &[&rev.by]);
    s.into_bytes()
}

/// *v1 (legacy).* The bytes a retired key signed **itself** to come back
/// from `date`, retired since `retired_on`. Used only to verify migrated
/// `legacy` events.
pub fn unretire_bytes(owner: &str, k: &ReviewerKey, retired_on: &str, date: &str) -> Vec<u8> {
    let mut s = String::from("kovan-key-unretire-v1\n");
    line(&mut s, "reviewer", &[owner]);
    line(&mut s, "key", &[&k.id]);
    line(&mut s, "public", &[&k.public]);
    line(&mut s, "retired_on", &[retired_on]);
    line(&mut s, "date", &[date]);
    s.into_bytes()
}

/// The bytes a signed `[[reviewer.key.history]]` event signs: the key
/// (owner, id, alg, public, created), the event kind, its date and its
/// signer. An `unretired` event also signs the date of the retirement it
/// ends (so moving that `retired` entry breaks it); an `admitted` event
/// also signs the [`admission_bytes`] of `admission` (role, admitted date,
/// scope, first key).
pub fn key_event_bytes(
    owner: &str,
    k: &ReviewerKey,
    ev: &KeyEvent,
    retired_on: &str,
    admission: Option<&Reviewer>,
) -> Vec<u8> {
    let mut s = String::from("kovan-key-event-v1\n");
    line(&mut s, "reviewer", &[owner]);
    line(&mut s, "key", &[&k.id]);
    line(&mut s, "alg", &[&k.alg]);
    line(&mut s, "public", &[&k.public]);
    line(&mut s, "created", &[&k.created]);
    line(&mut s, "event", &[ev.event.as_str()]);
    line(&mut s, "date", &[&ev.date]);
    let (sr, sk) = match &ev.signer {
        Some(sg) => (sg.reviewer.as_deref().unwrap_or(""), sg.key.as_str()),
        None => ("", ""),
    };
    line(&mut s, "signer", &[sr, sk]);
    if ev.event == KeyEventKind::Unretired {
        line(&mut s, "retired_on", &[retired_on]);
    }
    let mut out = s.into_bytes();
    if ev.event == KeyEventKind::Admitted {
        if let Some(r) = admission {
            out.extend_from_slice(&admission_bytes(r));
        }
    }
    out
}

/// What an event's signature is checked over: [`key_event_bytes`], or for a
/// `legacy` (migrated v1) event the v1 statement it was signed as.
pub fn event_signed_bytes(
    owner: &str,
    k: &ReviewerKey,
    ev: &KeyEvent,
    retired_on: &str,
    admission: Option<&Reviewer>,
) -> Vec<u8> {
    if ev.legacy {
        match ev.event {
            KeyEventKind::Endorsed => return endorsement_bytes_with(owner, k, false),
            KeyEventKind::Reset => return endorsement_bytes_with(owner, k, true),
            KeyEventKind::Admitted => {
                if let Some(r) = admission {
                    return admission_bytes(r);
                }
            }
            KeyEventKind::Unretired => return unretire_bytes(owner, k, retired_on, &ev.date),
            _ => {}
        }
    }
    key_event_bytes(owner, k, ev, retired_on, admission)
}

fn role_str(r: Role) -> &'static str {
    match r {
        Role::Maintainer => "maintainer",
        Role::Reviewer => "reviewer",
    }
}

/// Whether `s` is a calendar date `YYYY-MM-DD` (the stamp and registry date
/// format; compared as text, which orders correctly in this form).
pub fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let digits = [0, 1, 2, 3, 5, 6, 8, 9];
    if !digits.iter().all(|&i| b[i].is_ascii_digit()) {
        return false;
    }
    let month = (b[5] - b'0') * 10 + (b[6] - b'0');
    let day = (b[8] - b'0') * 10 + (b[9] - b'0');
    (1..=12).contains(&month) && (1..=31).contains(&day)
}

/// Why a key or signature cannot even be decoded or checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CryptoError {
    /// Not standard base64 (`field` names what was being decoded).
    BadBase64 { field: &'static str },
    /// Decoded to the wrong number of bytes (32 for a key, 64 for a signature).
    BadLength { field: &'static str, len: usize },
    /// 32 bytes that are not a valid ed25519 public key.
    BadPublicKey,
    /// Well formed, but does not verify over these bytes with this key.
    Mismatch,
}

impl std::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadBase64 { field } => write!(f, "{field} is not base64"),
            Self::BadLength { field, len } => write!(f, "{field} decodes to {len} bytes"),
            Self::BadPublicKey => write!(f, "not a valid ed25519 public key"),
            Self::Mismatch => write!(f, "signature does not verify"),
        }
    }
}

impl std::error::Error for CryptoError {}

/// Base64 (standard alphabet, padded), the encoding of every key and
/// signature in the schema.
pub fn encode_b64(bytes: &[u8]) -> String {
    B64.encode(bytes)
}

/// Decode base64 into exactly `N` bytes.
pub(crate) fn decode_fixed<const N: usize>(
    field: &'static str,
    b64: &str,
) -> Result<[u8; N], CryptoError> {
    let v = B64.decode(b64.trim()).map_err(|_| CryptoError::BadBase64 { field })?;
    let len = v.len();
    v.try_into().map_err(|_| CryptoError::BadLength { field, len })
}

/// Decode a `[[reviewer.key]] public` value.
pub fn decode_public(b64: &str) -> Result<VerifyingKey, CryptoError> {
    let bytes = decode_fixed::<32>("public key", b64)?;
    VerifyingKey::from_bytes(&bytes).map_err(|_| CryptoError::BadPublicKey)
}

/// Check a base64 signature over `msg` with `key`. Strict ed25519
/// (`verify_strict`): rejects small-order keys and malleable signatures.
pub fn verify_bytes(key: &VerifyingKey, msg: &[u8], sig_b64: &str) -> Result<(), CryptoError> {
    let bytes = decode_fixed::<64>("signature", sig_b64)?;
    let sig = EdSignature::from_bytes(&bytes);
    key.verify_strict(msg, &sig).map_err(|_| CryptoError::Mismatch)
}

/// A signature that counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedStamp {
    /// The reviewer (`by`).
    pub reviewer: String,
    /// The key id that signed.
    pub key: String,
    /// How that key is trusted; a reset key stays visible here forever.
    pub key_status: KeyStatus,
    /// The key is a deliberate reset (shown permanently).
    pub reset: bool,
}

/// Why a signature does not count. Every variant is a "no": nothing here is
/// ever read as a valid review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnverifiedReason {
    /// The entry carries no signature.
    NoSignature,
    /// `alg` is not `ed25519`.
    UnsupportedAlgorithm(String),
    /// `by` is not a `[[reviewer]]` in `kovan_root.toml`.
    UnknownReviewer(String),
    /// The reviewer has no key with this id.
    UnknownKey { reviewer: String, key: String },
    /// The public key or the signature cannot be decoded.
    Malformed(CryptoError),
    /// The signature does not verify: a signed field changed after signing,
    /// or another key signed it.
    BadSignature,
    /// The stamp's `date` is not `YYYY-MM-DD`.
    BadDate(String),
    /// The reviewer is not admitted ([`registry::Admission`]).
    ReviewerNotAdmitted { reviewer: String, problem: registry::AdmissionProblem },
    /// The key is not trusted, e.g. unendorsed (loud: [`Registry::warnings`]).
    KeyNotTrusted { reviewer: String, key: String, problem: registry::KeyProblem },
    /// Dated on or after the reviewer's revocation date.
    Revoked { date: String },
    /// Dated on or after a compromised reviewer's compromise date.
    Compromised { from: String },
    /// Dated inside the key's retired window (`since = None`: no valid
    /// retirement date, so no stamp of the key counts).
    KeyRetired { key: String, since: Option<String> },
    /// Dated on or after a `revoked` event in the key's history (`date =
    /// None`: the event's date is malformed, so nothing counts).
    KeyRevoked { key: String, date: Option<String> },
    /// Dated on or after a `compromised` event in the key's history.
    KeyCompromised { key: String, from: Option<String> },
    /// A `reviewer` certified a path outside its `scope` (or an architecture
    /// node with no member paths: `path` is empty).
    OutsideScope { reviewer: String, path: String },
}

/// The result of checking an entry's signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignatureCheck {
    Verified(VerifiedStamp),
    Unverified(UnverifiedReason),
}

impl SignatureCheck {
    /// Whether the stamp counts.
    pub fn is_verified(&self) -> bool {
        matches!(self, Self::Verified(_))
    }
}

/// The workspace path of a stable function id (`<path>::<item>`).
fn path_of(function: &str) -> &str {
    function.split("::").next().unwrap_or(function)
}

/// Check a review's signature against the registry (module doc, "What a
/// verified stamp needs"). Scope is checked on the file of
/// `[review] function`.
pub fn verify_review(r: &ReviewEntry, registry: &Registry) -> SignatureCheck {
    let b = &r.review;
    let msg = signed_bytes(r);
    let paths = [path_of(&b.function)];
    verify_stamp(registry, &b.by, &b.date, b.signature.as_ref(), &msg, &paths)
}

/// Check an architecture node's signature against the registry. Scope is
/// checked on the file of every member; a `reviewer` (not a maintainer)
/// cannot sign a node with no members.
pub fn verify_architecture(a: &ArchitectureEntry, registry: &Registry) -> SignatureCheck {
    let b = &a.architecture;
    let msg = architecture_signed_bytes(a);
    let paths: Vec<&str> = b.members.iter().map(|m| path_of(m)).collect();
    verify_stamp(registry, &b.by, &b.date, b.signature.as_ref(), &msg, &paths)
}

fn verify_stamp(
    registry: &Registry,
    by: &str,
    date: &str,
    sig: Option<&Signature>,
    msg: &[u8],
    paths: &[&str],
) -> SignatureCheck {
    match check_stamp(registry, by, date, sig, msg, paths) {
        Ok(v) => SignatureCheck::Verified(v),
        Err(why) => SignatureCheck::Unverified(why),
    }
}

/// Crypto first (so a tampered entry reads as tampered whatever else is
/// wrong), then the registry's policy.
fn check_stamp(
    registry: &Registry,
    by: &str,
    date: &str,
    sig: Option<&Signature>,
    msg: &[u8],
    paths: &[&str],
) -> Result<VerifiedStamp, UnverifiedReason> {
    use UnverifiedReason as U;
    let sig = sig.ok_or(U::NoSignature)?;
    if sig.alg != ALG {
        return Err(U::UnsupportedAlgorithm(sig.alg.clone()));
    }
    let reviewer = registry.reviewer(by).ok_or_else(|| U::UnknownReviewer(by.to_string()))?;
    let key = reviewer.key(&sig.key).ok_or_else(|| U::UnknownKey {
        reviewer: by.to_string(),
        key: sig.key.clone(),
    })?;
    let public = key.public.as_ref().map_err(|e| U::Malformed(e.clone()))?;
    verify_bytes(public, msg, &sig.value).map_err(|e| match e {
        CryptoError::Mismatch => U::BadSignature,
        other => U::Malformed(other),
    })?;
    if !is_date(date) {
        return Err(U::BadDate(date.to_string()));
    }
    if let registry::Admission::NotAdmitted(problem) = &reviewer.admission {
        return Err(U::ReviewerNotAdmitted { reviewer: by.to_string(), problem: problem.clone() });
    }
    if let KeyStatus::Untrusted(problem) = &key.status {
        return Err(U::KeyNotTrusted {
            reviewer: by.to_string(),
            key: sig.key.clone(),
            problem: problem.clone(),
        });
    }
    if let Some(rev) = &reviewer.revocation {
        rev.check(date)?;
    }
    if let Some(why) = key.inactive_at(date) {
        return Err(why);
    }
    if reviewer.role == Role::Reviewer {
        if paths.is_empty() {
            return Err(U::OutsideScope { reviewer: by.to_string(), path: String::new() });
        }
        if let Some(p) = paths.iter().find(|p| !in_scope(&reviewer.scope, p)) {
            return Err(U::OutsideScope { reviewer: by.to_string(), path: p.to_string() });
        }
    }
    Ok(VerifiedStamp {
        reviewer: by.to_string(),
        key: key.id.clone(),
        key_status: key.status.clone(),
        reset: key.reset,
    })
}
