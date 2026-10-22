//! **Code Review ▸ User management** (GitHub #1019): the roster of every
//! reviewer, key and agent in the workspace, and the signed actions on it.
//! No egui; the drawing is `app/user_management_view.rs`.
//!
//! Maintainer, 2026-10-22: "under code review, i want a tab called user
//! management. Wherein all keys and reviewers will be there".
//!
//! ```text
//!  open ──> Load (worker: keystore list, kovan_root.toml, every review.md, the engine)
//!            v
//!  Roster: reviewers (role, scope, keys, status) ── warnings strip (Registry::warnings)
//!          one reviewer: admitted by/when, keys with replayed history, stamps, coverage
//!          agents (agent:…): justifications, "never counts as review"
//!          my keys on this machine (the #1020 Signing-key panel, reused)
//!            │ an action + the session key (passphrase only while locked)
//!            v
//!  Act (worker: re-check against the file, sign, append as text) ──> Load again
//! ```
//!
//! **Nothing here invents a trust rule.** Every status is read from the
//! #762 [`Registry`]; every action is a registry signature
//! (`UnlockedKey::{admit, endorse, unretire, revoke, revoke_key,
//! sign_roster_event}`, or the unsigned [`retire_key`]) written by the
//! `kovan_common::review::root_append` text appenders, which keep every
//! byte and read the result back.
//!
//! **Phase 2 (GitHub #1019, #1021; maintainer: "Add missing feature
//! lah").** Edit scope, Make maintainer / Step down, Hand over maintainer,
//! Mark novice and Promote are appended, maintainer-signed
//! `[[code_review.roster_event]]` entries; the admission is never
//! rewritten. A handover is two events in one write, B's promotion then
//! (optionally) A's step-down, both signed by A while A is a maintainer
//! (why two and not one: each is a change to one reviewer's standing that
//! the registry already replays; a dedicated two-party event would need its
//! own rules for the same outcome).
//!
//! **No maintainer left?** There is no bypass of the signature rules. The
//! repository owners' route is an ordinary reviewed commit: they declare a
//! new founder (`[code_review] founder`, a maintainer entry with no
//! admission, trusted on first use). Until a founder-succession record
//! exists (GitHub #1022), that re-roots trust, and admissions signed by the
//! old founder's chain then need re-signing by the new one.
//!
//! **No lag** (root `CLAUDE.md`, HARD RULE): loading (git facts, the
//! engine) and acting (the KDF, signing, the write) run on [`Worker`]s;
//! [`UserPanel::poll`] never blocks.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kovan_common::review::engine::Evaluation;
use kovan_common::review::root::{KeyEventKind, ReviewRoot, ReviewerKey, Revocation, Role, RosterEvent, RosterEventKind};
use kovan_common::review::root_append::{append_key_event, append_revocation, append_roster_event, record_admission};
use kovan_common::review::scope::in_scope;
use kovan_common::review::signed_at::{date_of, now_local};
use kovan_common::review::signing::keystore::{fingerprint, Keystore, UnlockedKey};
use kovan_common::review::signing::registry::{
    retire_key, Admission, AdmissionProblem, FounderProblem, HistoryProblem, InactiveKind, KeyProblem,
    KeyStatus, KeyTrust, Registry, ReviewerTrust, RosterChange, RosterProblem, SignerProblem, Warning,
};
use kovan_common::review::signing::session::{with_session_key, SharedKeySession};
use kovan_common::review::state::StateKind;
use kovan_common::review::types::AGENT_ID_PREFIX;
use zeroize::Zeroize;

use super::flow::{find_keys, session_holds, sign_failure, KeyInfo, SignFailure, Worker};
use super::{write_text, Workspace, ROOT_FILE};

/// The issue for what the registry cannot express yet.
/// Today, `YYYY-MM-DD`, local.
pub fn today() -> String {
    date_of(&now_local()).unwrap_or_default()
}

/// A reviewer's standing, as one word for the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Admitted, every key trusted (or retired).
    Active,
    /// Registered, not admitted: their stamps count for nothing yet.
    AwaitingAdmission(String),
    /// Revoked from `date`: stamps before it keep their standing.
    Revoked { date: String, compromised_from: Option<String> },
    /// Admitted, but a key needs attention (unendorsed, tampered history).
    KeyAttention(String),
}

impl Status {
    /// The list marker: ✔ active, ⚠ attention or awaiting, ✘ revoked
    /// (glyphs egui's default fonts carry).
    pub fn marker(&self) -> &'static str {
        match self {
            Self::Active => "\u{2714}",
            Self::AwaitingAdmission(_) | Self::KeyAttention(_) => "\u{26a0}",
            Self::Revoked { .. } => "\u{2718}",
        }
    }

    /// Plain words. (Novice marks show in the role label.)
    pub fn label(&self) -> String {
        match self {
            Self::Active => "active".into(),
            Self::AwaitingAdmission(why) => format!("awaiting admission ({why})"),
            Self::Revoked { date, compromised_from: None } => {
                format!("revoked {date} (stamps before: valid)")
            }
            Self::Revoked { date, compromised_from: Some(c) } => {
                format!("revoked {date}, compromised from {c} (stamps from {c}: void)")
            }
            Self::KeyAttention(why) => format!("\u{26a0} {why}"),
        }
    }
}

/// Plain words for why a signature by another key does not count.
pub fn signer_text(p: &SignerProblem) -> String {
    match p {
        SignerProblem::Unsigned => "unsigned".into(),
        SignerProblem::NotEligible(r) => format!("{r} may not sign this"),
        SignerProblem::UnknownKey { key } => format!("no eligible key {key}"),
        SignerProblem::NotTrusted { key } => format!("key {key} is not trusted"),
        SignerProblem::Malformed(e) => format!("malformed ({e:?})"),
        SignerProblem::BadSignature => "the signature does not verify (tampered?)".into(),
        SignerProblem::SignerRevoked { signer, date } => format!("{signer} was revoked on {date}"),
        SignerProblem::SignerRetired { signer, key } => format!("{signer}'s key {key} was retired then"),
        SignerProblem::SignerKeyRevoked { signer, key } => format!("{signer}'s key {key} was revoked then"),
    }
}

/// Plain words for a key that is not trusted.
pub fn key_problem_text(p: &KeyProblem) -> String {
    match p {
        KeyProblem::UnsupportedAlgorithm(a) => format!("algorithm {a} is not ed25519"),
        KeyProblem::BadPublicKey(_) => "the public key does not decode".into(),
        KeyProblem::BadCreatedDate(d) => format!("bad created date {d:?}"),
        KeyProblem::DuplicateId => "two keys share this id".into(),
        KeyProblem::ReviewerNotAdmitted => "the reviewer is not admitted".into(),
        KeyProblem::Unendorsed => "unendorsed: its new stamps don't count".into(),
        KeyProblem::Endorsement(s) => format!("endorsement does not count: {}", signer_text(s)),
    }
}

fn admission_problem_text(p: &AdmissionProblem) -> String {
    match p {
        AdmissionProblem::Unadmitted => "no maintainer has admitted them".into(),
        AdmissionProblem::Undated => "the admission has no date".into(),
        AdmissionProblem::BadDate(d) => format!("bad admission date {d:?}"),
        AdmissionProblem::NoKey => "no key".into(),
        AdmissionProblem::DateMismatch { event, admitted } => {
            format!("admission event dated {event}, admitted {admitted}")
        }
        AdmissionProblem::Signer(s) => format!("admission signature does not count: {}", signer_text(s)),
    }
}

fn history_problem_text(p: &HistoryProblem) -> String {
    match p {
        HistoryProblem::MissingCreated => "history does not start with `created`".into(),
        HistoryProblem::CreatedNotFirst { index } => format!("entry {index}: a second `created`"),
        HistoryProblem::BadDate { index, date } => format!("entry {index}: bad date {date:?}"),
        HistoryProblem::OutOfOrder { index } => format!("entry {index}: date goes backwards"),
        HistoryProblem::RetiredWhileRetired { index } => format!("entry {index}: retired while retired"),
        HistoryProblem::UnretiredWhileActive { index } => {
            format!("entry {index}: un-retired while active (a retirement deleted?)")
        }
        HistoryProblem::AdmittedNotOnFirstKey { index } => format!("entry {index}: admission not on the first key"),
        HistoryProblem::BadEventSignature { index, event, problem } => {
            format!("entry {index} ({}): {}", event.as_str(), signer_text(problem))
        }
        HistoryProblem::LegacyFieldsIgnored => "v1 fields ignored beside a history".into(),
    }
}

/// One line of the warnings strip, in plain words.
pub fn warning_text(w: &Warning) -> String {
    match w {
        Warning::NoFounder(p) => match p {
            FounderProblem::NotDeclared => "No founder is declared: nothing is trusted.".into(),
            FounderProblem::Unknown(id) => format!("The founder {id} is not a listed reviewer: nothing is trusted."),
            FounderProblem::NotMaintainer(id) => format!("The founder {id} is not a maintainer: nothing is trusted."),
            FounderProblem::HasAdmission(id) => format!("The founder {id} carries an admission: nothing is trusted."),
        },
        Warning::ReviewerNotAdmitted { reviewer, problem } => {
            format!("{reviewer} is not admitted ({}): their stamps don't count.", admission_problem_text(problem))
        }
        Warning::KeyNotTrusted { reviewer, key, problem } => {
            format!("{reviewer}'s key {key}: {}.", key_problem_text(problem))
        }
        Warning::KeyReset { reviewer, key, status } => format!(
            "{reviewer}'s key {key} is a reset key ({}), shown permanently.",
            if status.is_trusted() { "trusted" } else { "not trusted" }
        ),
        Warning::KeyHistory { reviewer, key, problem } => {
            format!("{reviewer}'s key {key}: {}.", history_problem_text(problem))
        }
        Warning::RevocationUnverified { reviewer, problem } => {
            format!("{reviewer}'s revocation is honoured but its signature does not count ({problem:?}).")
        }
        Warning::RosterEvent { index, reviewer, problem } => {
            format!("Roster event {} about {reviewer} has no effect: {}.", index + 1, roster_problem_text(problem))
        }
    }
}

/// Plain words for a roster event with no effect.
pub fn roster_problem_text(p: &RosterProblem) -> String {
    match p {
        RosterProblem::UnknownReviewer => "it names no reviewer".into(),
        RosterProblem::BadDate(d) => format!("bad date {d:?}"),
        RosterProblem::OutOfOrder => "dated before the event before it".into(),
        RosterProblem::MissingRole => "a role change without a role".into(),
        RosterProblem::Signer(s) => format!("its signature does not count ({})", signer_text(s)),
    }
}

/// One line of a reviewer's standing history: a verified roster event.
pub fn change_line(c: &RosterChange) -> String {
    let what = match (c.kind, c.role) {
        (RosterEventKind::Role, Some(Role::Maintainer)) => "made maintainer".to_string(),
        (RosterEventKind::Role, _) => "role: reviewer".to_string(),
        (RosterEventKind::Scope, _) if c.scope.is_empty() => "scope: none".to_string(),
        (RosterEventKind::Scope, _) => format!("scope: {}", c.scope.join(", ")),
        (RosterEventKind::Novice, _) => format!("marked novice ({})", where_(&c.scope)),
        (RosterEventKind::Trusted, _) => format!("promoted to trusted ({})", where_(&c.scope)),
    };
    format!("{} {what} by {} key {} \u{2714}", c.date, c.by, c.key)
}

fn where_(scope: &[String]) -> String {
    if scope.is_empty() {
        "everywhere".into()
    } else {
        scope.join(", ")
    }
}

/// A reviewer's [`Status`] from the registry.
pub fn status_of(r: &ReviewerTrust) -> Status {
    if let Some(rv) = &r.revocation {
        return Status::Revoked { date: rv.date.clone(), compromised_from: rv.compromised_from.clone() };
    }
    if let Admission::NotAdmitted(p) = &r.admission {
        return Status::AwaitingAdmission(admission_problem_text(p));
    }
    let attention: Vec<String> = r
        .keys
        .iter()
        .filter_map(|k| match &k.status {
            KeyStatus::Untrusted(p) => Some(format!("key {} {}", k.id, key_problem_text(p))),
            _ if !k.history.is_empty() => Some(format!("key {} history problem", k.id)),
            _ => None,
        })
        .collect();
    if attention.is_empty() {
        Status::Active
    } else {
        Status::KeyAttention(attention.join("; "))
    }
}

/// One key of a reviewer, for the page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyRow {
    pub id: String,
    /// `SHA256:…`, or why there is none.
    pub fingerprint: String,
    /// Plain words: trusted how, or why not; retired/revoked now.
    pub status: String,
    pub trusted: bool,
    pub retired: bool,
    /// Revoked or compromised (permanently void from a date).
    pub revoked: bool,
    /// The replayed history, one line per event, oldest first.
    pub history: Vec<String>,
    /// This machine's keystore holds this key.
    pub here: bool,
}

/// How a key is trusted, in words.
fn key_status_text(k: &KeyTrust) -> String {
    let mut s = match &k.status {
        KeyStatus::TrustedOnFirstUse => "trusted on first use (founder)".to_string(),
        KeyStatus::Admitted => "trusted (covered by the admission)".to_string(),
        KeyStatus::Endorsed { by_reviewer, by_key } => format!("trusted (endorsed by {by_reviewer} key {by_key})"),
        KeyStatus::ResetTrustedOnFirstUse => "trusted (founder reset, flagged forever)".to_string(),
        KeyStatus::Untrusted(p) => format!("not trusted: {}", key_problem_text(p)),
    };
    if k.is_retired() {
        s.push_str(" \u{b7} retired");
    }
    for w in k.windows.iter().filter(|w| w.kind != InactiveKind::Retired) {
        let what = if w.kind == InactiveKind::Compromised { "compromised" } else { "revoked" };
        s.push_str(&format!(" \u{b7} {what} from {}", w.from.as_deref().unwrap_or("the start")));
    }
    s
}

/// The key's history, replayed, one line per event: date, event, signer,
/// and ✔ (its signature counts), ✘ (it does not, with why) or "unsigned".
pub fn key_history_lines(k: &ReviewerKey, kt: &KeyTrust) -> Vec<String> {
    k.history
        .iter()
        .enumerate()
        .map(|(index, e)| {
            let mut s = format!("{} {}", if e.date.is_empty() { "????-??-??" } else { &e.date }, e.event.as_str());
            if let Some(sg) = &e.signer {
                s.push_str(&format!(" by {} key {}", sg.reviewer.as_deref().unwrap_or("?"), sg.key));
            }
            let bad = kt.history.iter().find_map(|p| match p {
                HistoryProblem::BadEventSignature { index: i, problem, .. } if *i == index => Some(problem),
                _ => None,
            });
            match (bad, &e.signature) {
                (Some(p), _) => s.push_str(&format!(" \u{2718} {}", signer_text(p))),
                (None, Some(_)) => s.push_str(" \u{2714}"),
                (None, None) if matches!(e.event, KeyEventKind::Created | KeyEventKind::Retired) => {}
                (None, None) => s.push_str(" (unsigned)"),
            }
            if e.legacy {
                s.push_str(" (v1)");
            }
            s
        })
        .collect()
}

/// One stamp of a reviewer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StampRow {
    /// `file.rs::qual`, or the id of a deleted function.
    pub function: String,
    pub date: String,
    pub state: StateKind,
}

/// One agent justification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JustificationRow {
    pub function: String,
    pub verdict: String,
    pub date: String,
    /// "code unchanged since", "code changed since" or "function no longer
    /// indexed".
    pub code: &'static str,
    pub evidence: Vec<String>,
}

/// A human reviewer on the roster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewerRow {
    pub id: String,
    pub name: Option<String>,
    /// The role in force now (after the roster events).
    pub role: Role,
    /// The scope in force now.
    pub scope: Vec<String>,
    /// The novice mark in force now, in words ("novice everywhere except
    /// crates/njoy-*/**"), if any.
    pub novice: Option<String>,
    /// The standing history: admission, then each verified roster event.
    pub standing: Vec<String>,
    pub founder: bool,
    /// "Maintainer · founder", "Reviewer", …
    pub role_label: String,
    /// "all crates", or the globs.
    pub scope_label: String,
    /// "founder, trusted on first use", "2026-10-30 by github:m (key k1) ✔",
    /// or why not.
    pub admitted: String,
    pub status: Status,
    pub keys: Vec<KeyRow>,
    pub stamps: Vec<StampRow>,
    /// Functions with a valid stamp by this reviewer, and current functions
    /// in their scope (all for a maintainer); `None` without the engine.
    pub coverage: Option<(usize, usize)>,
}

impl ReviewerRow {
    /// Stamps by state, for "41 stamps · 38 valid · 3 stale".
    pub fn stamp_counts(&self) -> BTreeMap<StateKind, usize> {
        let mut m = BTreeMap::new();
        for s in &self.stamps {
            *m.entry(s.state).or_insert(0) += 1;
        }
        m
    }

    /// One line: "41 stamps · 38 valid · 3 changed since review · coverage 62 % (41/66)".
    pub fn reviews_line(&self) -> String {
        let mut s = format!("{} stamp{}", self.stamps.len(), if self.stamps.len() == 1 { "" } else { "s" });
        for (k, n) in self.stamp_counts() {
            s.push_str(&format!(" \u{b7} {n} {}", k.label()));
        }
        match self.coverage {
            Some((v, 0)) => s.push_str(&format!(" \u{b7} coverage {v}/0")),
            Some((v, t)) => s.push_str(&format!(" \u{b7} coverage {} % ({v}/{t})", v * 100 / t)),
            None => s.push_str(" \u{b7} coverage not computed"),
        }
        s
    }
}

/// An AI agent: justifications only, never review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRow {
    pub id: String,
    pub justifications: Vec<JustificationRow>,
}

/// The whole roster.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Roster {
    pub reviewers: Vec<ReviewerRow>,
    pub agents: Vec<AgentRow>,
    /// [`Registry::warnings`], in plain words.
    pub warnings: Vec<String>,
}

impl Roster {
    pub fn reviewer(&self, id: &str) -> Option<&ReviewerRow> {
        self.reviewers.iter().find(|r| r.id == id)
    }
}

/// Build the roster from the root, the registry it gives, the engine's
/// verdict (stamps and coverage; `None`: not computed), the workspace's
/// `review.md` files (agent justifications) and this machine's keys.
pub fn build_roster(root: &ReviewRoot, eval: Option<&Evaluation>, ws: Option<&Workspace>, local: &[KeyInfo]) -> Roster {
    let registry = Registry::build(root);
    let mut migrated = root.clone();
    migrated.migrate_key_history();
    let founder = registry.founder.as_ref().ok();
    let mut reviewers = Vec::new();
    for (r, t) in migrated.reviewers.iter().zip(&registry.reviewers) {
        let is_founder = founder == Some(&r.id);
        let (role, scope) = (t.current_role(), t.current_scope().to_vec());
        let novice = t.novice_now().map(|(at, except)| {
            let mut s = format!("novice {}", where_(&at));
            if !except.is_empty() {
                s.push_str(&format!(", trusted on {}", except.join(", ")));
            }
            s
        });
        let mut role_label = match (role, is_founder) {
            (Role::Maintainer, true) => "Maintainer \u{b7} founder".to_string(),
            (Role::Maintainer, false) => "Maintainer".to_string(),
            (Role::Reviewer, true) => "Reviewer \u{b7} founder (stepped down)".to_string(),
            (Role::Reviewer, false) => "Reviewer".to_string(),
        };
        if let Some(n) = &novice {
            role_label.push_str(&format!(" \u{b7} {n}"));
        }
        let scope_label = match (role, scope.is_empty()) {
            (Role::Maintainer, _) => "all crates".to_string(),
            (Role::Reviewer, true) => "no scope: stamps nowhere".to_string(),
            (Role::Reviewer, false) => scope.join(", "),
        };
        let mut standing = vec![match &t.admission {
            Admission::Founder => format!("founder: {} (trusted on first use)", if r.role == Role::Maintainer { "maintainer" } else { "reviewer" }),
            Admission::Admitted { by, .. } => format!(
                "{} admitted as {}{} by {by} \u{2714}",
                r.admitted.as_deref().unwrap_or("?"),
                if r.role == Role::Maintainer { "maintainer" } else { "reviewer" },
                if r.scope.is_empty() { String::new() } else { format!(", scope {}", r.scope.join(", ")) }
            ),
            Admission::NotAdmitted(_) => "registered, not admitted".to_string(),
        }];
        standing.extend(t.changes.iter().map(change_line));
        let admitted = match &t.admission {
            Admission::Founder => "founder, trusted on first use".to_string(),
            Admission::Admitted { by, key } => format!(
                "{} by {by} (key {key}) \u{2714}",
                r.admitted.as_deref().unwrap_or("?")
            ),
            Admission::NotAdmitted(p) => format!("not admitted: {}", admission_problem_text(p)),
        };
        let keys = r
            .keys
            .iter()
            .zip(&t.keys)
            .map(|(k, kt)| KeyRow {
                id: k.id.clone(),
                fingerprint: fingerprint(&k.public).unwrap_or_else(|_| "no fingerprint (bad public key)".into()),
                status: key_status_text(kt),
                trusted: kt.status.is_trusted(),
                retired: kt.is_retired(),
                revoked: kt.windows.iter().any(|w| w.kind != InactiveKind::Retired),
                history: key_history_lines(k, kt),
                here: local.iter().any(|l| l.reviewer == r.id && l.key == k.id && l.registered),
            })
            .collect();
        let mut stamps = Vec::new();
        let mut coverage = None;
        if let Some(ev) = eval {
            let mut valid = 0usize;
            let mut in_scope_n = 0usize;
            for f in ev.functions.values() {
                let file = f.location.as_ref().map(|l| l.file.as_str()).unwrap_or("");
                let shown = f.location.as_ref().map_or(f.id.clone(), |l| format!("{}::{}", l.file, l.qual));
                let mine: Vec<_> = f.reviews.iter().filter(|x| x.by == r.id).collect();
                for x in &mine {
                    stamps.push(StampRow { function: shown.clone(), date: x.date.clone().unwrap_or_default(), state: x.state.kind() });
                }
                if role == Role::Maintainer || in_scope(&scope, file) {
                    in_scope_n += 1;
                    if mine.iter().any(|x| x.state.kind() == StateKind::Valid) {
                        valid += 1;
                    }
                }
            }
            for f in &ev.deleted {
                for x in f.reviews.iter().filter(|x| x.by == r.id) {
                    stamps.push(StampRow { function: f.id.clone(), date: x.date.clone().unwrap_or_default(), state: x.state.kind() });
                }
            }
            coverage = Some((valid, in_scope_n));
        }
        reviewers.push(ReviewerRow {
            id: r.id.clone(),
            name: r.name.clone(),
            role,
            scope,
            novice,
            standing,
            founder: is_founder,
            role_label,
            scope_label,
            admitted,
            status: status_of(t),
            keys,
            stamps,
            coverage,
        });
    }
    let mut agents: BTreeMap<String, Vec<JustificationRow>> = BTreeMap::new();
    if let Some(ws) = ws {
        let hashes = ws.hashes();
        for file in ws.reviews.values() {
            for j in file.doc.justifications() {
                let b = &j.justification;
                let code = match hashes.get(&j.function_id()) {
                    Some(h) if *h == b.hash => "code unchanged since",
                    Some(_) => "code changed since",
                    None => "function no longer indexed",
                };
                agents.entry(b.by.clone()).or_default().push(JustificationRow {
                    function: j.path().unwrap_or_else(|| j.function_id()),
                    verdict: b.verdict.as_str().to_string(),
                    date: b.date.clone(),
                    code,
                    evidence: b.evidence.clone(),
                });
            }
        }
    }
    Roster {
        reviewers,
        agents: agents
            .into_iter()
            .filter(|(id, _)| id.starts_with(AGENT_ID_PREFIX))
            .map(|(id, justifications)| AgentRow { id, justifications })
            .collect(),
        warnings: registry.warnings().iter().map(warning_text).collect(),
    }
}

/// What the tab can sign (or, for retirement, write unsigned).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserAction {
    /// Admit a registered reviewer with a role (and scope, for a
    /// reviewer); `novice`: marked novice everywhere from the same day (a
    /// roster event in the same write).
    Admit { reviewer: String, role: Role, scope: Vec<String>, novice: bool },
    /// Endorse a reviewer's new key with another of their own trusted keys.
    Endorse { reviewer: String, key: String },
    /// Retire a key (unsigned: it only takes trust away).
    RetireKey { reviewer: String, key: String },
    /// Un-retire a key: signed by that very key.
    UnretireKey { reviewer: String, key: String },
    /// Revoke a key from today (`compromised`: it leaked).
    RevokeKey { reviewer: String, key: String, compromised: bool },
    /// Revoke a reviewer from today (`compromised`: their stamps from today
    /// are void too).
    RevokeReviewer { reviewer: String, compromised: bool },
    /// Replace an admitted reviewer's scope from today (a roster event).
    ChangeScope { reviewer: String, scope: Vec<String> },
    /// Change an admitted reviewer's role from today: Make maintainer, or
    /// Step down to reviewer (a roster event).
    ChangeRole { reviewer: String, role: Role },
    /// Hand maintainer to `to` from today; with `step_down`, the signer
    /// steps down to reviewer right after (two roster events, one write).
    HandOver { to: String, step_down: bool },
    /// Mark a reviewer novice from today where `scope` covers (empty:
    /// everywhere): their stamps there need a second, trusted sign-off.
    MarkNovice { reviewer: String, scope: Vec<String> },
    /// Promote a novice to trusted from today where `scope` covers (empty:
    /// everywhere).
    Promote { reviewer: String, scope: Vec<String> },
}

impl UserAction {
    /// The button's words.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Admit { .. } => "Admit",
            Self::Endorse { .. } => "Endorse",
            Self::RetireKey { .. } => "Retire",
            Self::UnretireKey { .. } => "Unretire",
            Self::RevokeKey { compromised: false, .. } => "Revoke key",
            Self::RevokeKey { compromised: true, .. } => "Key compromised",
            Self::RevokeReviewer { .. } => "Revoke reviewer",
            Self::ChangeScope { .. } => "Set scope",
            Self::ChangeRole { role: Role::Maintainer, .. } => "Make maintainer",
            Self::ChangeRole { role: Role::Reviewer, .. } => "Step down to reviewer",
            Self::HandOver { .. } => "Hand over maintainer",
            Self::MarkNovice { .. } => "Mark novice",
            Self::Promote { .. } => "Promote to trusted",
        }
    }
}

/// Whether `signer` (reviewer, key) may sign as an admitted maintainer on
/// `date`: a maintainer, admitted, not revoked by then, with that key
/// trusted and active then. `Err` says why not.
pub fn maintainer_signer(registry: &Registry, signer: (&str, &str), date: &str) -> Result<(), String> {
    let (rid, kid) = signer;
    let r = registry.reviewer(rid).ok_or_else(|| format!("{rid} is not in kovan_root.toml."))?;
    if r.role_at(date) != Role::Maintainer || !r.is_admitted() {
        return Err(format!("Only an admitted maintainer can do this; {rid} is not one."));
    }
    own_key_active(r, kid, date)
}

/// `kid` of `r` is trusted and active on `date`, and `r` not revoked then.
fn own_key_active(r: &ReviewerTrust, kid: &str, date: &str) -> Result<(), String> {
    if r.revocation.as_ref().is_some_and(|v| v.cutoff(date).is_some()) {
        return Err(format!("{} is revoked.", r.id));
    }
    let k = r.key(kid).ok_or_else(|| format!("{} has no key {kid} in kovan_root.toml.", r.id))?;
    if !k.status.is_trusted() {
        return Err(format!("Your key {kid} is not trusted, so it signs nothing."));
    }
    if k.inactive_at(date).is_some() {
        return Err(format!("Your key {kid} is retired or revoked, so it signs nothing."));
    }
    Ok(())
}

/// What stops `action`, signed by `signer` (reviewer, key) on `date`
/// (empty: it may go ahead). The same check runs again on the file just
/// before writing.
pub fn action_problems(root: &ReviewRoot, signer: Option<(&str, &str)>, action: &UserAction, date: &str) -> Vec<String> {
    let registry = Registry::build(root);
    let Some(signer) = signer else {
        return vec!["No signing key: set one up from a Stamp, or import one below.".into()];
    };
    let mut p = Vec::new();
    let target_of = |id: &str| registry.reviewer(id);
    let key_of = |id: &str, key: &str| target_of(id).and_then(|r| r.key(key));
    let mut need = |r: Result<(), String>| {
        if let Err(e) = r {
            p.push(e);
        }
    };
    match action {
        UserAction::Admit { reviewer, role, scope, novice } => {
            if *novice && *role == Role::Maintainer {
                need(Err("A maintainer is never a novice.".into()));
            }
            need(maintainer_signer(&registry, signer, date));
            match target_of(reviewer) {
                None => need(Err(format!("{reviewer} is not registered: they register their key from a Stamp first."))),
                Some(t) => {
                    if t.is_admitted() || matches!(t.admission, Admission::NotAdmitted(ref a) if *a != AdmissionProblem::Unadmitted) {
                        need(Err(format!("{reviewer} is admitted already, or carries an admission.")));
                    }
                    if t.keys.is_empty() {
                        need(Err(format!("{reviewer} has no key to admit.")));
                    }
                    if t.revocation.is_some() {
                        need(Err(format!("{reviewer} is revoked.")));
                    }
                }
            }
            if *role == Role::Reviewer && scope.iter().all(|g| g.trim().is_empty()) {
                need(Err("A reviewer needs a scope (e.g. crates/njoy-*/**).".into()));
            }
        }
        UserAction::Endorse { reviewer, key } => {
            if signer.0 != reviewer {
                need(Err(format!("A key is endorsed by another key of its own reviewer: sign as {reviewer}.")));
            } else if signer.1 == key {
                need(Err("A key cannot endorse itself.".into()));
            } else if let Some(r) = target_of(reviewer) {
                need(own_key_active(r, signer.1, date));
            }
            match key_of(reviewer, key).map(|k| &k.status) {
                None => need(Err(format!("{reviewer} has no key {key}."))),
                Some(KeyStatus::Untrusted(KeyProblem::Unendorsed | KeyProblem::Endorsement(_))) => {}
                Some(_) => need(Err(format!("Key {key} needs no endorsement."))),
            }
        }
        UserAction::RetireKey { reviewer, key } => {
            if signer.0 != reviewer {
                need(maintainer_signer(&registry, signer, date).map_err(|_| {
                    format!("Only {reviewer} or an admitted maintainer retires this key.")
                }));
            }
            match key_of(reviewer, key) {
                None => need(Err(format!("{reviewer} has no key {key}."))),
                Some(k) if k.is_retired() => need(Err(format!("Key {key} is retired already."))),
                Some(_) => {}
            }
        }
        UserAction::UnretireKey { reviewer, key } => {
            if signer != (reviewer.as_str(), key.as_str()) {
                need(Err(format!(
                    "Only key {key} itself can un-retire it (the proof you hold it): unlock {reviewer}'s key {key}."
                )));
            }
            match key_of(reviewer, key) {
                None => need(Err(format!("{reviewer} has no key {key}."))),
                Some(k) if !k.is_retired() => need(Err(format!("Key {key} is not retired."))),
                Some(_) => {}
            }
        }
        UserAction::RevokeKey { reviewer, key, .. } => {
            if signer.0 == reviewer {
                if let Some(r) = target_of(reviewer) {
                    need(own_key_active(r, signer.1, date));
                }
            } else {
                need(maintainer_signer(&registry, signer, date));
            }
            match key_of(reviewer, key) {
                None => need(Err(format!("{reviewer} has no key {key}."))),
                Some(k) if k.windows.iter().any(|w| w.kind != InactiveKind::Retired) => {
                    need(Err(format!("Key {key} is revoked already.")))
                }
                Some(_) => {}
            }
        }
        UserAction::RevokeReviewer { reviewer, .. } => {
            need(maintainer_signer(&registry, signer, date));
            match target_of(reviewer) {
                None => need(Err(format!("{reviewer} is not in kovan_root.toml."))),
                Some(t) if t.revocation.is_some() => need(Err(format!("{reviewer} is revoked already."))),
                Some(_) => {}
            }
        }
        UserAction::ChangeScope { reviewer, scope } => {
            need(maintainer_signer(&registry, signer, date));
            need(standing_target(&registry, reviewer));
            if let Some(t) = target_of(reviewer) {
                if t.current_role() == Role::Maintainer {
                    need(Err(format!("{reviewer} is a maintainer: a maintainer's scope is everywhere.")));
                }
                if t.current_scope() == scope.as_slice() {
                    need(Err("That is the scope in force already.".into()));
                }
            }
            if scope.iter().all(|g| g.trim().is_empty()) {
                need(Err("Enter at least one glob (e.g. crates/njoy-*/**).".into()));
            }
        }
        UserAction::ChangeRole { reviewer, role } => {
            need(maintainer_signer(&registry, signer, date));
            need(standing_target(&registry, reviewer));
            if target_of(reviewer).is_some_and(|t| t.current_role() == *role) {
                need(Err(format!("{reviewer} has that role already.")));
            }
        }
        UserAction::HandOver { to, .. } => {
            need(maintainer_signer(&registry, signer, date));
            if to == signer.0 {
                need(Err("Hand over to someone else.".into()));
            }
            need(standing_target(&registry, to));
            if target_of(to).is_some_and(|t| t.current_role() == Role::Maintainer) {
                need(Err(format!("{to} is a maintainer already.")));
            }
        }
        UserAction::MarkNovice { reviewer, .. } => {
            need(maintainer_signer(&registry, signer, date));
            need(standing_target(&registry, reviewer));
            if target_of(reviewer).is_some_and(|t| t.current_role() == Role::Maintainer) {
                need(Err("A maintainer is never a novice.".into()));
            }
        }
        UserAction::Promote { reviewer, .. } => {
            need(maintainer_signer(&registry, signer, date));
            need(standing_target(&registry, reviewer));
            if target_of(reviewer).is_some_and(|t| t.novice_now().is_none()) {
                need(Err(format!("{reviewer} is not a novice anywhere.")));
            }
        }
    }
    p
}

/// A roster event's subject: admitted and not revoked.
fn standing_target(registry: &Registry, id: &str) -> Result<(), String> {
    match registry.reviewer(id) {
        None => Err(format!("{id} is not in kovan_root.toml.")),
        Some(t) if !t.is_admitted() => Err(format!("{id} is not admitted: admit them first.")),
        Some(t) if t.revocation.is_some() => Err(format!("{id} is revoked.")),
        Some(_) => Ok(()),
    }
}

/// A roster event about `reviewer`, signed by `key`.
fn roster_event(key: &UnlockedKey, reviewer: &str, kind: RosterEventKind, date: &str, role: Option<Role>, scope: &[String]) -> Result<RosterEvent, String> {
    let mut e = RosterEvent {
        reviewer: reviewer.to_string(),
        event: kind,
        date: date.to_string(),
        role,
        scope: scope.iter().map(|g| g.trim().to_string()).filter(|g| !g.is_empty()).collect(),
        signer: None,
        signature: None,
    };
    key.sign_roster_event(&mut e).map_err(|x| x.to_string())?;
    Ok(e)
}

/// Sign `action` with `key`, dated `date`, and append it to
/// `<root>/kovan_root.toml`; the notice to show. Re-checks
/// [`action_problems`] against the file as it is now, and writes nothing on
/// any refusal.
pub fn apply_action(root: &Path, key: &UnlockedKey, action: &UserAction, date: &str) -> Result<String, String> {
    let path = root.join(ROOT_FILE);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{ROOT_FILE}: {e}"))?;
    let parsed = ReviewRoot::parse(&text).map_err(|e| format!("{ROOT_FILE}: {e}"))?;
    let problems = action_problems(&parsed, Some((key.reviewer(), key.key_id())), action, date);
    if !problems.is_empty() {
        return Err(problems.join(" "));
    }
    let err = |e: &dyn std::fmt::Display| e.to_string();
    let stored_key = |reviewer: &str, kid: &str| -> Result<ReviewerKey, String> {
        parsed
            .reviewer(reviewer)
            .and_then(|r| r.keys.iter().find(|k| k.id == kid))
            .cloned()
            .ok_or_else(|| format!("{reviewer} has no key {kid}."))
    };
    let (new, notice) = match action {
        UserAction::Admit { reviewer, role, scope, novice } => {
            let mut r = parsed.reviewer(reviewer).cloned().ok_or("unknown reviewer")?;
            r.role = *role;
            r.scope = if *role == Role::Reviewer {
                scope.iter().map(|g| g.trim().to_string()).filter(|g| !g.is_empty()).collect()
            } else {
                Vec::new()
            };
            r.admitted = Some(date.to_string());
            key.admit(&mut r).map_err(|e| err(&e))?;
            let mut t = record_admission(&text, &r).map_err(|e| err(&e))?;
            if *novice {
                let e = roster_event(key, reviewer, RosterEventKind::Novice, date, None, &[])?;
                t = append_roster_event(&t, &e).map_err(|e| err(&e))?;
            }
            let role = match (*role, *novice) {
                (Role::Maintainer, _) => "maintainer",
                (Role::Reviewer, true) => "novice reviewer",
                (Role::Reviewer, false) => "reviewer",
            };
            (t, format!("Admitted {reviewer} as {role} from {date}, signed with your key {}.", key.key_id()))
        }
        UserAction::Endorse { reviewer, key: kid } => {
            let mut k = stored_key(reviewer, kid)?;
            key.endorse(reviewer, &mut k, date).map_err(|e| err(&e))?;
            let t = append_key_event(&text, reviewer, kid, k.history.last().ok_or("no event")?).map_err(|e| err(&e))?;
            (t, format!("Endorsed {reviewer}'s key {kid} with key {}.", key.key_id()))
        }
        UserAction::RetireKey { reviewer, key: kid } => {
            let mut k = stored_key(reviewer, kid)?;
            retire_key(&mut k, date).map_err(|e| err(&e))?;
            let t = append_key_event(&text, reviewer, kid, k.history.last().ok_or("no event")?).map_err(|e| err(&e))?;
            (t, format!("Retired {reviewer}'s key {kid} from {date} (unsigned: retiring only takes trust away)."))
        }
        UserAction::UnretireKey { reviewer, key: kid } => {
            let mut k = stored_key(reviewer, kid)?;
            key.unretire(&mut k, date).map_err(|e| err(&e))?;
            let t = append_key_event(&text, reviewer, kid, k.history.last().ok_or("no event")?).map_err(|e| err(&e))?;
            (t, format!("Un-retired {reviewer}'s key {kid} from {date}, signed by the key itself."))
        }
        UserAction::RevokeKey { reviewer, key: kid, compromised } => {
            let mut k = stored_key(reviewer, kid)?;
            key.revoke_key(reviewer, &mut k, date, *compromised).map_err(|e| err(&e))?;
            let t = append_key_event(&text, reviewer, kid, k.history.last().ok_or("no event")?).map_err(|e| err(&e))?;
            let what = if *compromised { "compromised" } else { "revoked" };
            (t, format!("Recorded {reviewer}'s key {kid} as {what} from {date}."))
        }
        UserAction::RevokeReviewer { reviewer, compromised } => {
            let mut rev = Revocation {
                date: date.to_string(),
                compromised_from: compromised.then(|| date.to_string()),
                by: key.reviewer().to_string(),
                signature: None,
            };
            key.revoke(reviewer, &mut rev).map_err(|e| err(&e))?;
            let t = append_revocation(&text, reviewer, &rev).map_err(|e| err(&e))?;
            (t, format!("Revoked {reviewer} from {date}; their stamps before it keep their standing."))
        }
        UserAction::ChangeScope { reviewer, scope } => {
            let e = roster_event(key, reviewer, RosterEventKind::Scope, date, None, scope)?;
            let t = append_roster_event(&text, &e).map_err(|x| err(&x))?;
            (t, format!("{reviewer}'s scope is {} from {date}; their earlier stamps are judged by the old one.", e.scope.join(", ")))
        }
        UserAction::ChangeRole { reviewer, role } => {
            let e = roster_event(key, reviewer, RosterEventKind::Role, date, Some(*role), &[])?;
            let t = append_roster_event(&text, &e).map_err(|x| err(&x))?;
            let what = if *role == Role::Maintainer { "a maintainer" } else { "a reviewer" };
            (t, format!("{reviewer} is {what} from {date}."))
        }
        UserAction::HandOver { to, step_down } => {
            let e = roster_event(key, to, RosterEventKind::Role, date, Some(Role::Maintainer), &[])?;
            let mut t = append_roster_event(&text, &e).map_err(|x| err(&x))?;
            if *step_down {
                let me = roster_event(key, key.reviewer(), RosterEventKind::Role, date, Some(Role::Reviewer), &[])?;
                t = append_roster_event(&t, &me).map_err(|x| err(&x))?;
            }
            (t, format!(
                "{} handed maintainer to {to} from {date}{}; earlier stamps keep their standing and trust marks carry over.",
                key.reviewer(),
                if *step_down { " and stepped down" } else { "" }
            ))
        }
        UserAction::MarkNovice { reviewer, scope } => {
            let e = roster_event(key, reviewer, RosterEventKind::Novice, date, None, scope)?;
            let t = append_roster_event(&text, &e).map_err(|x| err(&x))?;
            (t, format!("{reviewer} is a novice ({}) from {date}: their stamps there need a trusted second sign-off.", where_(&e.scope)))
        }
        UserAction::Promote { reviewer, scope } => {
            let e = roster_event(key, reviewer, RosterEventKind::Trusted, date, None, scope)?;
            let t = append_roster_event(&text, &e).map_err(|x| err(&x))?;
            (t, format!("{reviewer} is trusted ({}) from {date}: their later stamps there count alone.", where_(&e.scope)))
        }
    };
    write_text(&path, &new)?;
    Ok(notice)
}

/// What the load worker reads.
#[derive(Debug, Clone)]
pub struct Loaded {
    pub root: ReviewRoot,
    pub roster: Roster,
    /// Why stamps and coverage are missing, if they are.
    pub note: Option<String>,
}

type LoadResult = Result<(Vec<KeyInfo>, Loaded), String>;

enum Job {
    Load(Worker<LoadResult>),
    Act(Worker<Result<String, SignFailure>>),
}

/// Globs from a comma- or newline-separated field.
pub fn globs(text: &str) -> Vec<String> {
    text.split([',', '\n']).map(str::trim).filter(|g| !g.is_empty()).map(str::to_string).collect()
}

/// The Admit form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmitForm {
    pub role: Role,
    /// Admit as a novice reviewer (a second sign-off is needed).
    pub novice: bool,
    /// Globs, comma- or newline-separated.
    pub scope: String,
}

impl Default for AdmitForm {
    fn default() -> Self {
        AdmitForm { role: Role::Reviewer, novice: false, scope: String::new() }
    }
}

impl AdmitForm {
    pub fn globs(&self) -> Vec<String> {
        globs(&self.scope)
    }
}

/// The tab's whole state (module doc).
pub struct UserPanel {
    root: PathBuf,
    keystore: Keystore,
    session: SharedKeySession,
    /// This machine's key files.
    pub keys: Vec<KeyInfo>,
    /// Index into `keys` of the key that signs.
    pub chosen: usize,
    /// `None` while loading.
    pub data: Option<Result<Loaded, String>>,
    /// The reviewer or agent whose page is open.
    pub selected: Option<String>,
    pub admit: AdmitForm,
    /// The page's Edit scope / novice-scope / promote-scope fields (globs,
    /// comma-separated), and Hand over's "and step down".
    pub scope_edit: String,
    pub novice_scope: String,
    pub step_down: bool,
    /// Revocations as compromised.
    pub compromised: bool,
    pub passphrase: String,
    pub notices: Vec<String>,
    pub error: Option<String>,
    job: Option<Job>,
    /// An action wrote `kovan_root.toml` since the host last asked
    /// ([`Self::take_changed`]).
    changed: bool,
}

impl Drop for UserPanel {
    fn drop(&mut self) {
        self.passphrase.zeroize();
    }
}

impl UserPanel {
    /// Open the tab over workspace `root`, with `keystore` (tests inject a
    /// temporary one) and the app's key `session`. Loading starts on a
    /// worker.
    pub fn new(root: PathBuf, keystore: Keystore, session: SharedKeySession) -> UserPanel {
        let mut p = UserPanel {
            root,
            keystore,
            session,
            keys: Vec::new(),
            chosen: 0,
            data: None,
            selected: None,
            admit: AdmitForm::default(),
            scope_edit: String::new(),
            novice_scope: String::new(),
            step_down: true,
            compromised: false,
            passphrase: String::new(),
            notices: Vec::new(),
            error: None,
            job: None,
            changed: false,
        };
        p.load();
        p
    }

    /// Whether an action wrote the registry since the last call (the host
    /// then recomputes the review states: trust may have changed).
    pub fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    /// The signing key, if any.
    pub fn key(&self) -> Option<&KeyInfo> {
        self.keys.get(self.chosen)
    }

    /// The session holds the signing key: no passphrase is asked.
    pub fn unlocked(&self) -> bool {
        session_holds(&self.session, self.key())
    }

    /// Read everything again (worker).
    pub fn load(&mut self) {
        let (root, ks) = (self.root.clone(), self.keystore.clone());
        self.data = None;
        self.job = Some(Job::Load(Worker::spawn(move || load(&root, &ks))));
    }

    /// Take a finished job's result; never blocks.
    pub fn poll(&mut self) {
        let Some(job) = self.job.take() else { return };
        match job {
            Job::Load(w) => match w.try_take() {
                None => self.job = Some(Job::Load(w)),
                Some(Err(e)) => self.data = Some(Err(e)),
                Some(Ok((keys, loaded))) => {
                    let keep = self.key().map(|k| (k.reviewer.clone(), k.key.clone()));
                    self.keys = keys;
                    self.chosen = keep
                        .and_then(|(r, k)| self.keys.iter().position(|i| i.reviewer == r && i.key == k))
                        .or_else(|| {
                            // An admitted maintainer's key first: the one most actions need.
                            self.keys.iter().position(|k| {
                                loaded.roster.reviewer(&k.reviewer).is_some_and(|r| {
                                    r.role == Role::Maintainer && r.status == Status::Active && k.registered
                                })
                            })
                        })
                        .or_else(|| self.keys.iter().position(|k| k.registered))
                        .unwrap_or(0);
                    self.data = Some(Ok(loaded));
                }
            },
            Job::Act(w) => match w.try_take() {
                None => self.job = Some(Job::Act(w)),
                Some(Err(SignFailure::WrongPassphrase)) => {
                    self.error = Some("Wrong passphrase (or the key file was altered). Nothing was written.".into());
                }
                Some(Err(SignFailure::Other(e))) => self.error = Some(format!("{e} Nothing was written.")),
                Some(Ok(n)) => {
                    self.notices.push(n);
                    self.changed = true;
                    self.load();
                }
            },
        }
    }

    /// The loaded roster, if any.
    pub fn roster(&self) -> Option<&Roster> {
        match &self.data {
            Some(Ok(l)) => Some(&l.roster),
            _ => None,
        }
    }

    /// What stops `action` now (empty: the button is enabled).
    pub fn problems(&self, action: &UserAction) -> Vec<String> {
        let Some(Ok(l)) = &self.data else {
            return vec!["Still reading kovan_root.toml.".into()];
        };
        let k = self.key();
        let mut p = action_problems(&l.root, k.map(|k| (k.reviewer.as_str(), k.key.as_str())), action, &today());
        if k.is_some_and(|k| !k.registered) {
            p.push("This key is not registered in kovan_root.toml.".into());
        }
        if self.passphrase.is_empty() && !self.unlocked() && !matches!(action, UserAction::RetireKey { .. }) {
            p.push("Type your passphrase (asked once per session).".into());
        }
        p
    }

    /// Run `action` (worker): unlock through the session, sign, append.
    /// Refused (nothing starts) while it has problems or a job runs.
    pub fn submit(&mut self, action: UserAction) -> bool {
        if self.job.is_some() || !self.problems(&action).is_empty() {
            return false;
        }
        let Some(k) = self.key().cloned() else { return false };
        let mut pass = std::mem::take(&mut self.passphrase);
        let (root, ks, session) = (self.root.clone(), self.keystore.clone(), self.session.clone());
        self.error = None;
        let date = today();
        self.job = Some(Job::Act(Worker::spawn(move || {
            let kf = ks.load(&k.reviewer, &k.key).map_err(|e| SignFailure::Other(e.to_string()))?;
            let done = with_session_key(&session, &kf, &pass, |key| apply_action(&root, key, &action, &date));
            pass.zeroize();
            done.map_err(sign_failure)?.map_err(SignFailure::Other)
        })));
        true
    }

    /// Poll until no job runs (tests only; the UI never waits).
    #[cfg(test)]
    pub fn wait(&mut self) {
        let start = std::time::Instant::now();
        while self.busy() {
            assert!(start.elapsed() < std::time::Duration::from_secs(120), "user panel job did not finish");
            std::thread::sleep(std::time::Duration::from_millis(5));
            self.poll();
        }
    }
}

/// The load worker: keys, root, workspace and the engine's verdict.
fn load(root: &Path, ks: &Keystore) -> LoadResult {
    let found = find_keys(root, ks)?;
    if !found.root_exists {
        return Err("No kovan_root.toml: run Index fresh in the Code Map tab first.".into());
    }
    let text = std::fs::read_to_string(root.join(ROOT_FILE)).map_err(|e| format!("{ROOT_FILE}: {e}"))?;
    let parsed = ReviewRoot::parse(&text).map_err(|e| format!("{ROOT_FILE}: {e}"))?;
    let (roster, note) = match super::load_workspace(root) {
        Ok(ws) => {
            let ev = super::evaluate_loaded(root, &ws);
            (build_roster(&parsed, Some(&ev.evaluation), Some(&ws), &found.keys), None)
        }
        Err(e) => (
            build_roster(&parsed, None, None, &found.keys),
            Some(format!("Stamps and coverage not computed: {e}")),
        ),
    };
    Ok((found.keys, Loaded { root: parsed, roster, note }))
}

#[cfg(test)]
#[path = "user_management_tests.rs"]
pub(crate) mod tests;
