//! **Registering reviewers and keys in `kovan_root.toml` as text** (GitHub
//! #762, #770): the "register my key" step of desktop kovan's stamp dialog.
//!
//! `kovan_root.toml` is append-only history (#762 follow-up, 2026-10-07,
//! [`super::engine::history_append_only`]) and carries the maintainer's
//! comments. [`super::root::ReviewRoot::write_into`] re-serialises the file
//! and drops comments, so it is not used here. Like
//! [`super::root::append_rust_analyzer_used`], every function here **adds
//! text** and keeps every existing byte:
//!
//! - [`append_reviewer`]: a new `[[reviewer]]` block at the end of the file;
//! - [`append_reviewer_key`]: a new `[[reviewer.key]]` block spliced in at
//!   the end of that reviewer's section (before the next header that is not
//!   one of its own sub-tables, and before any comment lines that lead into
//!   that header);
//! - [`declare_founder`]: `founder = "<id>"` right under `[code_review]`, or
//!   a new `[code_review]` table at the end. Refused when a founder is
//!   already declared: trust on first use happens once.
//! - [`append_developing_organisation`] (GitHub #810): a new
//!   `[[code_review.developing_organisation]]` block at the end of the file;
//! - [`append_reviewer_organisation`] and [`append_separation_attestation`]
//!   (#810): a new `[[reviewer.organisation]]` / `[[reviewer.separation]]`
//!   block spliced in at the end of that reviewer's section, as a key is.
//!   An attestation whose id the reviewer already uses is refused (a stamp
//!   names it by id).
//! - **Lifecycle (GitHub #1019, the User management tab):**
//!   [`append_key_event`] adds one `[[reviewer.key.history]]` event at the
//!   end of that key's section (endorse, retire, un-retire, revoke or
//!   compromise a key); [`record_admission`] admits a registered reviewer,
//!   replacing only the `role`, `scope` and `admitted` lines of its own
//!   table and appending the `admitted` event to its first key;
//!   [`append_revocation`] adds `[reviewer.revoked]`, never over an
//!   existing one.
//!
//! No workspace member depends on `toml_edit` directly (checked
//! 2026-10-10; it is only `toml`'s own dependency), so the splice is by line, and every result is **read back**: the
//! parsed root after must equal the parsed root before plus exactly the one
//! addition, or nothing is returned ([`AppendError::NotReadBack`]).
//!
//! What a registration does **not** do: endorse a later key or admit a new
//! reviewer. Those are signatures by a trusted key (`UnlockedKey::endorse`,
//! `UnlockedKey::admit`); the caller signs the [`Reviewer`] or
//! [`ReviewerKey`] first and passes it here, or registers it unsigned and
//! the registry shows it as untrusted until it is (Leak Before Break).

use serde::Serialize;

use super::root::{
    DevelopingOrganisation, KeyEvent, KeyEventKind, KeySigner, ReviewRoot, Reviewer, ReviewerKey,
    ReviewerOrganisation, Revocation, RootError, Role, SeparationAttestation,
};

/// Why a registration was refused. Nothing is changed on any refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppendError {
    /// The existing file does not parse.
    Root(RootError),
    /// A `[[reviewer]]` with this id exists already.
    ReviewerExists(String),
    /// No `[[reviewer]]` with this id.
    UnknownReviewer(String),
    /// The reviewer has a key with this id already.
    KeyExists { reviewer: String, key: String },
    /// `[code_review] founder` is already set (to this id).
    FounderAlreadyDeclared(String),
    /// The appended text would not read back as exactly one more entry.
    NotReadBack(String),
    /// The reviewer has a separation attestation with this id already
    /// (#810).
    AttestationExists { reviewer: String, id: String },
    /// No key with this id under the reviewer, or two of them (GitHub
    /// #1019, [`append_key_event`]).
    UnknownKey { reviewer: String, key: String },
    /// The key's history is still in the v1 legacy fields (no
    /// `[[reviewer.key.history]]` in the file): an event appended now would
    /// start a history without its `created` entry (#1019).
    LegacyKey { reviewer: String, key: String },
    /// The reviewer is admitted already, or is the founder (#1019,
    /// [`record_admission`]).
    AlreadyAdmitted(String),
    /// The reviewer already carries a `[reviewer.revoked]` (#1019,
    /// [`append_revocation`]).
    AlreadyRevoked(String),
}

impl std::fmt::Display for AppendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Root(e) => write!(f, "{e}"),
            Self::ReviewerExists(id) => write!(f, "kovan_root.toml already lists reviewer {id}"),
            Self::UnknownReviewer(id) => write!(f, "kovan_root.toml lists no reviewer {id}"),
            Self::KeyExists { reviewer, key } => {
                write!(f, "reviewer {reviewer} already has a key {key}")
            }
            Self::FounderAlreadyDeclared(id) => {
                write!(f, "kovan_root.toml already names founder {id}")
            }
            Self::AttestationExists { reviewer, id } => {
                write!(f, "reviewer {reviewer} already has a separation attestation {id}")
            }
            Self::UnknownKey { reviewer, key } => {
                write!(f, "reviewer {reviewer} has no single key {key}")
            }
            Self::LegacyKey { reviewer, key } => write!(
                f,
                "key {key} of {reviewer} has no [[reviewer.key.history]] yet (v1 fields): \
                 nothing can be appended to it"
            ),
            Self::AlreadyAdmitted(id) => write!(f, "{id} is admitted already"),
            Self::AlreadyRevoked(id) => write!(f, "{id} is revoked already"),
            Self::NotReadBack(m) => write!(
                f,
                "kovan_root.toml would not read back as intended ({m}); nothing written"
            ),
        }
    }
}

impl std::error::Error for AppendError {}

fn parse(text: &str) -> Result<ReviewRoot, AppendError> {
    ReviewRoot::parse(text).map_err(AppendError::Root)
}

/// `text` ending in a newline.
fn with_newline(text: &str) -> String {
    let mut s = text.to_string();
    if !s.is_empty() && !s.ends_with('\n') {
        s.push('\n');
    }
    s
}

#[derive(Serialize)]
struct Reviewers<'a> {
    reviewer: [&'a Reviewer; 1],
}

#[derive(Serialize)]
struct Keys<'a> {
    key: [&'a ReviewerKey; 1],
}

/// `existing` with `r` appended as a new `[[reviewer]]` (module doc).
pub fn append_reviewer(existing: &str, r: &Reviewer) -> Result<String, AppendError> {
    let before = parse(existing)?;
    if before.reviewer(&r.id).is_some() {
        return Err(AppendError::ReviewerExists(r.id.clone()));
    }
    let block = toml::to_string_pretty(&Reviewers { reviewer: [r] })
        .map_err(|e| AppendError::NotReadBack(e.to_string()))?;
    let mut text = with_newline(existing);
    if !text.is_empty() {
        text.push('\n');
    }
    text.push_str(&block);
    let after = parse(&text)?;
    let mut expected = before.clone();
    expected.reviewers.push(r.clone());
    if after != expected {
        return Err(AppendError::NotReadBack(format!("[[reviewer]] {}", r.id)));
    }
    Ok(text)
}

/// Whether `line` is a TOML table header (`[x]` or `[[x]]`), and its name.
fn header(line: &str) -> Option<String> {
    let t = line.trim();
    let inner = if let Some(r) = t.strip_prefix("[[") {
        r.split_once("]]")?.0
    } else {
        t.strip_prefix('[')?.split_once(']')?.0
    };
    Some(
        inner
            .split('.')
            .map(str::trim)
            .collect::<Vec<_>>()
            .join("."),
    )
}

/// `existing` with `k` added to reviewer `reviewer`'s keys (module doc).
pub fn append_reviewer_key(
    existing: &str,
    reviewer: &str,
    k: &ReviewerKey,
) -> Result<String, AppendError> {
    let before = parse(existing)?;
    let Some(pos) = before.reviewers.iter().position(|r| r.id == reviewer) else {
        return Err(AppendError::UnknownReviewer(reviewer.into()));
    };
    if before.reviewers[pos].keys.iter().any(|x| x.id == k.id) {
        return Err(AppendError::KeyExists {
            reviewer: reviewer.into(),
            key: k.id.clone(),
        });
    }
    let block = toml::to_string_pretty(&Keys { key: [k] })
        .map_err(|e| AppendError::NotReadBack(e.to_string()))?;
    let block: String = block
        .split_inclusive('\n')
        .map(|l| match l.strip_prefix("[[key") {
            Some(rest) => format!("[[reviewer.key{rest}"),
            None => match l.strip_prefix("[key") {
                Some(rest) => format!("[reviewer.key{rest}"),
                None => l.to_string(),
            },
        })
        .collect();
    let text = splice_into_reviewer(existing, pos, &block)?;
    let after = parse(&text)?;
    let mut expected = before.clone();
    expected.reviewers[pos].keys.push(k.clone());
    if after != expected {
        return Err(AppendError::NotReadBack(format!(
            "[[reviewer.key]] {} of {reviewer}",
            k.id
        )));
    }
    Ok(text)
}

/// The lines of `existing`, its table headers (line index, dotted name),
/// and the line span `[start, end)` of the `pos`-th `[[reviewer]]`
/// section: from its header to the next header that is not one of its own
/// sub-tables.
fn reviewer_span(existing: &str, pos: usize) -> Result<(Vec<&str>, Vec<(usize, String)>, usize, usize), AppendError> {
    let lines: Vec<&str> = existing.split_inclusive('\n').collect();
    let heads: Vec<(usize, String)> = lines
        .iter()
        .enumerate()
        .filter_map(|(i, l)| header(l).map(|h| (i, h)))
        .collect();
    // The `[[reviewer]]` header lines, in file order: the parser keeps
    // array order, so the pos-th is this reviewer's.
    let starts: Vec<usize> = heads
        .iter()
        .filter(|(_, h)| h == "reviewer")
        .map(|(i, _)| *i)
        .collect();
    let Some(&start) = starts.get(pos) else {
        return Err(AppendError::NotReadBack(
            "cannot find the [[reviewer]] header".into(),
        ));
    };
    let end = heads
        .iter()
        .find(|(i, h)| *i > start && !h.starts_with("reviewer."))
        .map_or(lines.len(), |(i, _)| *i);
    Ok((lines, heads, start, end))
}

/// `lines` with `block` spliced in before line `end` (a section's end,
/// `start` its header): before the comment and blank lines that lead into
/// the next header, unless `end` is the end of the file.
fn splice_at(lines: &[&str], start: usize, end: usize, block: &str) -> String {
    let mut end = end;
    if end < lines.len() {
        while end > start + 1 && {
            let t = lines[end - 1].trim();
            t.is_empty() || t.starts_with('#')
        } {
            end -= 1;
        }
    }
    let mut text: String = lines[..end].concat();
    text = with_newline(&text);
    text.push('\n');
    text.push_str(block);
    if end < lines.len() {
        text.push('\n');
    }
    text.push_str(&lines[end..].concat());
    text
}

/// `existing` with `block` spliced in at the end of the `pos`-th
/// `[[reviewer]]` section: before the next header that is not one of its
/// own sub-tables, and before any comment lines leading into that header.
fn splice_into_reviewer(existing: &str, pos: usize, block: &str) -> Result<String, AppendError> {
    let (lines, _, start, end) = reviewer_span(existing, pos)?;
    Ok(splice_at(&lines, start, end, block))
}

/// `key = "value"`, TOML-quoted.
fn kv(key: &str, value: &str) -> String {
    format!("{key} = {}\n", toml::Value::String(value.to_string()))
}

/// `signer = { reviewer = "...", key = "..." }`.
fn signer_line(s: &KeySigner) -> String {
    let q = |v: &str| toml::Value::String(v.to_string()).to_string();
    match &s.reviewer {
        Some(r) => format!("signer = {{ reviewer = {}, key = {} }}\n", q(r), q(&s.key)),
        None => format!("signer = {{ key = {} }}\n", q(&s.key)),
    }
}

/// `existing` with `e` appended as a new
/// `[[code_review.developing_organisation]]` at the end (module doc).
pub fn append_developing_organisation(
    existing: &str,
    e: &DevelopingOrganisation,
) -> Result<String, AppendError> {
    let before = parse(existing)?;
    let mut block = String::from("[[code_review.developing_organisation]]\n");
    if let Some(k) = &e.krate {
        block.push_str(&kv("crate", k));
    }
    block.push_str(&kv("name", &e.name));
    block.push_str(&kv("date", &e.date));
    if let Some(sg) = &e.signer {
        block.push_str(&signer_line(sg));
    }
    if let Some(sig) = &e.signature {
        block.push_str(&kv("signature", sig));
    }
    let mut text = with_newline(existing);
    if !text.is_empty() {
        text.push('\n');
    }
    text.push_str(&block);
    let after = parse(&text)?;
    let mut expected = before.clone();
    expected
        .code_review
        .get_or_insert_with(Default::default)
        .developing_organisation
        .push(e.clone());
    if after != expected {
        return Err(AppendError::NotReadBack(
            "[[code_review.developing_organisation]]".into(),
        ));
    }
    Ok(text)
}

/// `existing` with `e` added to reviewer `reviewer`'s
/// `[[reviewer.organisation]]` list (module doc).
pub fn append_reviewer_organisation(
    existing: &str,
    reviewer: &str,
    e: &ReviewerOrganisation,
) -> Result<String, AppendError> {
    let before = parse(existing)?;
    let Some(pos) = before.reviewers.iter().position(|r| r.id == reviewer) else {
        return Err(AppendError::UnknownReviewer(reviewer.into()));
    };
    let mut block = String::from("[[reviewer.organisation]]\n");
    block.push_str(&kv("name", &e.name));
    block.push_str(&kv("date", &e.date));
    if let Some(sg) = &e.signer {
        block.push_str(&signer_line(sg));
    }
    if let Some(sig) = &e.signature {
        block.push_str(&kv("signature", sig));
    }
    let text = splice_into_reviewer(existing, pos, &block)?;
    let after = parse(&text)?;
    let mut expected = before.clone();
    expected.reviewers[pos].organisations.push(e.clone());
    if after != expected {
        return Err(AppendError::NotReadBack(format!(
            "[[reviewer.organisation]] of {reviewer}"
        )));
    }
    Ok(text)
}

/// `existing` with `a` added to reviewer `reviewer`'s
/// `[[reviewer.separation]]` list (module doc). Refused when the reviewer
/// already has an attestation with `a.id`.
pub fn append_separation_attestation(
    existing: &str,
    reviewer: &str,
    a: &SeparationAttestation,
) -> Result<String, AppendError> {
    let before = parse(existing)?;
    let Some(pos) = before.reviewers.iter().position(|r| r.id == reviewer) else {
        return Err(AppendError::UnknownReviewer(reviewer.into()));
    };
    if before.reviewers[pos].separations.iter().any(|x| x.id == a.id) {
        return Err(AppendError::AttestationExists {
            reviewer: reviewer.into(),
            id: a.id.clone(),
        });
    }
    let mut block = String::from("[[reviewer.separation]]\n");
    block.push_str(&kv("id", &a.id));
    block.push_str(&kv("organisation", &a.organisation));
    block.push_str(&kv("developing_organisation", &a.developing_organisation));
    block.push_str(&kv("date", &a.date));
    if let Some(u) = &a.audit_record {
        block.push_str(&kv("audit_record", u));
    }
    if let Some(k) = &a.key {
        block.push_str(&kv("key", k));
    }
    if let Some(sig) = &a.signature {
        block.push_str(&kv("signature", sig));
    }
    let text = splice_into_reviewer(existing, pos, &block)?;
    let after = parse(&text)?;
    let mut expected = before.clone();
    expected.reviewers[pos].separations.push(a.clone());
    if after != expected {
        return Err(AppendError::NotReadBack(format!(
            "[[reviewer.separation]] {} of {reviewer}",
            a.id
        )));
    }
    Ok(text)
}

/// The value of a top-level `key = value` line (`None`: another key, a
/// header, a comment or not TOML on its own, e.g. the first line of a
/// multi-line array).
fn line_value(line: &str, key: &str) -> Option<toml::Value> {
    let t: toml::Table = toml::from_str(line).ok()?;
    t.get(key).cloned()
}

/// The line span `[start, end)` of the key `key` of the `pos`-th reviewer,
/// from its `[[reviewer.key]]` header to the next header that is not one of
/// its own sub-tables (`[[reviewer.key.history]]`).
fn key_span(existing: &str, pos: usize, key: &str) -> Result<(Vec<String>, usize, usize), AppendError> {
    let (lines, heads, start, end) = reviewer_span(existing, pos)?;
    let in_section: Vec<&(usize, String)> = heads.iter().filter(|(i, _)| *i > start && *i < end).collect();
    for (n, (i, h)) in in_section.iter().enumerate() {
        if h != "reviewer.key" {
            continue;
        }
        let next = in_section[n + 1..]
            .iter()
            .find(|(_, h2)| !h2.starts_with("reviewer.key."))
            .map_or(end, |(j, _)| *j);
        let body_end = in_section.get(n + 1).map_or(end, |(j, _)| *j);
        let named = lines[*i + 1..body_end]
            .iter()
            .any(|l| line_value(l, "id").and_then(|v| v.as_str().map(|s| s == key)) == Some(true));
        if named {
            return Ok((lines.iter().map(|l| l.to_string()).collect(), *i, next));
        }
    }
    Err(AppendError::NotReadBack(format!("cannot find the [[reviewer.key]] {key}")))
}

/// One `[[reviewer.key.history]]` block.
fn event_block(ev: &KeyEvent) -> String {
    let mut block = String::from("[[reviewer.key.history]]\n");
    block.push_str(&format!("event = {}\n", toml::Value::String(ev.event.as_str().into())));
    block.push_str(&kv("date", &ev.date));
    if let Some(sg) = &ev.signer {
        block.push_str(&signer_line(sg));
    }
    if let Some(sig) = &ev.signature {
        block.push_str(&kv("signature", sig));
    }
    if ev.legacy {
        block.push_str("legacy = true\n");
    }
    block
}

/// `existing` with `ev` appended to the history of key `key` of
/// `reviewer` (GitHub #1019: the User management tab's endorse, retire,
/// un-retire, key revocation and admission events). The block goes at the
/// end of that key's section; every existing byte is kept. Refused for an
/// unknown or duplicated key, and for a key whose history is still in the
/// v1 legacy fields ([`AppendError::LegacyKey`]). The caller signs `ev`
/// first (`UnlockedKey::{endorse, unretire, revoke_key}`) or makes an
/// unsigned one ([`super::signing::registry::retire_key`]).
pub fn append_key_event(
    existing: &str,
    reviewer: &str,
    key: &str,
    ev: &KeyEvent,
) -> Result<String, AppendError> {
    let before = parse(existing)?;
    let Some(pos) = before.reviewers.iter().position(|r| r.id == reviewer) else {
        return Err(AppendError::UnknownReviewer(reviewer.into()));
    };
    let unknown = || AppendError::UnknownKey { reviewer: reviewer.into(), key: key.into() };
    let keys = &before.reviewers[pos].keys;
    if keys.iter().filter(|k| k.id == key).count() != 1 {
        return Err(unknown());
    }
    let j = keys.iter().position(|k| k.id == key).ok_or_else(unknown)?;
    if keys[j].history.is_empty() {
        return Err(AppendError::LegacyKey { reviewer: reviewer.into(), key: key.into() });
    }
    let (lines, start, end) = key_span(existing, pos, key)?;
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let text = splice_at(&refs, start, end, &event_block(ev));
    let after = parse(&text)?;
    let mut expected = before.clone();
    expected.reviewers[pos].keys[j].history.push(ev.clone());
    if after != expected {
        return Err(AppendError::NotReadBack(format!(
            "[[reviewer.key.history]] {} of {reviewer}'s key {key}",
            ev.event.as_str()
        )));
    }
    Ok(text)
}

/// Whether `r` carries an admission (an `admitted` event, or a v1
/// `admitted_by`).
fn has_admission(r: &Reviewer) -> bool {
    r.admitted_by.is_some()
        || r.keys.iter().any(|k| k.history.iter().any(|e| e.event == KeyEventKind::Admitted))
}

/// `existing` with a registered, not yet admitted reviewer admitted
/// (GitHub #1019): `admitted` is that reviewer as `UnlockedKey::admit`
/// left it, i.e. the entry in the file with `role`, `scope` and
/// `admitted` set and one `admitted` event appended to its first key.
/// Those three lines of the reviewer's own table are replaced (every other
/// line, comments included, is kept) and the event is appended as
/// [`append_key_event`] does. Refused when the reviewer is unknown, is the
/// founder, is admitted already, or `admitted` differs from the entry in
/// any other way.
pub fn record_admission(existing: &str, admitted: &Reviewer) -> Result<String, AppendError> {
    let before = parse(existing)?;
    let id = admitted.id.as_str();
    let Some(pos) = before.reviewers.iter().position(|r| r.id == id) else {
        return Err(AppendError::UnknownReviewer(id.into()));
    };
    let old = &before.reviewers[pos];
    let founder = before.code_review.as_ref().and_then(|c| c.founder.as_deref());
    if has_admission(old) || founder == Some(id) {
        return Err(AppendError::AlreadyAdmitted(id.into()));
    }
    let (Some(k0), Some(ev)) = (old.keys.first(), admitted.keys.first().and_then(|k| k.history.last())) else {
        return Err(AppendError::NotReadBack(format!("{id} has no first key to admit")));
    };
    let mut expected_r = old.clone();
    expected_r.role = admitted.role;
    expected_r.scope = admitted.scope.clone();
    expected_r.admitted = admitted.admitted.clone();
    expected_r.keys[0].history.push(ev.clone());
    if ev.event != KeyEventKind::Admitted || expected_r != *admitted {
        return Err(AppendError::NotReadBack(format!(
            "the admission of {id} changes more than its role, scope, admitted date and one \
             admitted event"
        )));
    }
    // The reviewer's own table: header line to its first sub-table.
    let (lines, heads, start, end) = reviewer_span(existing, pos)?;
    let head_end = heads.iter().find(|(i, _)| *i > start && *i < end).map_or(end, |(i, _)| *i);
    let mut kept: Vec<String> = Vec::new();
    let mut skipping_array = 0i32;
    for l in &lines[start + 1..head_end] {
        if skipping_array > 0 {
            skipping_array += bracket_balance(l);
            continue;
        }
        let key = l.split('=').next().map(str::trim).unwrap_or("");
        if l.contains('=') && matches!(key, "role" | "scope" | "admitted") {
            let b = bracket_balance(l);
            if b > 0 {
                skipping_array = b;
            }
            continue;
        }
        kept.push(l.to_string());
    }
    // New lines after the last line with content (comments and blank
    // lines leading into the next table stay where they were).
    let at = kept
        .iter()
        .rposition(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with('#')
        })
        .map_or(0, |i| i + 1);
    let mut fields = format!("role = {}\n", toml::Value::String(role_name(admitted.role).into()));
    if !admitted.scope.is_empty() {
        let globs = toml::Value::Array(admitted.scope.iter().cloned().map(toml::Value::String).collect());
        fields.push_str(&format!("scope = {globs}\n"));
    }
    if let Some(d) = &admitted.admitted {
        fields.push_str(&kv("admitted", d));
    }
    if at > 0 && !kept[at - 1].ends_with('\n') {
        kept[at - 1].push('\n');
    }
    kept.insert(at, fields);
    let mut text: String = lines[..=start].concat();
    text.push_str(&kept.concat());
    text.push_str(&lines[head_end..].concat());
    // Then the event, on the first key.
    let text = append_key_event(&text, id, &k0.id, ev)?;
    let after = parse(&text)?;
    let mut expected = before.clone();
    expected.reviewers[pos] = admitted.clone();
    if after != expected {
        return Err(AppendError::NotReadBack(format!("the admission of {id}")));
    }
    Ok(text)
}

/// `[` minus `]` on a line, outside strings (good enough for the arrays of
/// globs this file holds; the read-back check catches the rest).
fn bracket_balance(line: &str) -> i32 {
    let mut n = 0;
    let mut in_str = false;
    for c in line.chars() {
        match c {
            '"' => in_str = !in_str,
            '[' if !in_str => n += 1,
            ']' if !in_str => n -= 1,
            '#' if !in_str => break,
            _ => {}
        }
    }
    n
}

fn role_name(r: Role) -> &'static str {
    match r {
        Role::Maintainer => "maintainer",
        Role::Reviewer => "reviewer",
    }
}

/// `existing` with `rev` as reviewer `reviewer`'s `[reviewer.revoked]`
/// (GitHub #1019), spliced in at the end of that reviewer's section.
/// Refused when the reviewer is unknown or revoked already: a revocation is
/// never edited.
pub fn append_revocation(existing: &str, reviewer: &str, rev: &Revocation) -> Result<String, AppendError> {
    let before = parse(existing)?;
    let Some(pos) = before.reviewers.iter().position(|r| r.id == reviewer) else {
        return Err(AppendError::UnknownReviewer(reviewer.into()));
    };
    if before.reviewers[pos].revoked.is_some() {
        return Err(AppendError::AlreadyRevoked(reviewer.into()));
    }
    let mut block = String::from("[reviewer.revoked]\n");
    block.push_str(&kv("date", &rev.date));
    if let Some(c) = &rev.compromised_from {
        block.push_str(&kv("compromised_from", c));
    }
    block.push_str(&kv("by", &rev.by));
    if let Some(s) = &rev.signature {
        let q = |v: &str| toml::Value::String(v.to_string()).to_string();
        block.push_str(&format!("signature = {{ key = {}, signature = {} }}\n", q(&s.key), q(&s.signature)));
    }
    let text = splice_into_reviewer(existing, pos, &block)?;
    let after = parse(&text)?;
    let mut expected = before.clone();
    expected.reviewers[pos].revoked = Some(rev.clone());
    if after != expected {
        return Err(AppendError::NotReadBack(format!("[reviewer.revoked] of {reviewer}")));
    }
    Ok(text)
}

/// `existing` with `[code_review] founder = id` (module doc).
pub fn declare_founder(existing: &str, id: &str) -> Result<String, AppendError> {
    let before = parse(existing)?;
    if let Some(f) = before.code_review.as_ref().and_then(|c| c.founder.clone()) {
        return Err(AppendError::FounderAlreadyDeclared(f));
    }
    let q = toml::Value::String(id.to_string()).to_string();
    let lines: Vec<&str> = existing.split_inclusive('\n').collect();
    let at = lines.iter().position(|l| {
        let t = l.trim();
        !t.starts_with("[[") && header(l).as_deref() == Some("code_review")
    });
    let text = match at {
        Some(i) => {
            let mut t: String = with_newline(&lines[..=i].concat());
            t.push_str(&format!("founder = {q}\n"));
            t.push_str(&lines[i + 1..].concat());
            t
        }
        None => {
            let mut t = with_newline(existing);
            if !t.is_empty() {
                t.push('\n');
            }
            t.push_str(&format!("[code_review]\nfounder = {q}\n"));
            t
        }
    };
    let after = parse(&text)?;
    let mut expected = before.clone();
    expected
        .code_review
        .get_or_insert_with(Default::default)
        .founder = Some(id.to_string());
    if after != expected {
        return Err(AppendError::NotReadBack("[code_review] founder".into()));
    }
    Ok(text)
}

/// The founding maintainer's `[[reviewer]]` (trusted on first use,
/// [`super::signing::registry`]): a maintainer with no admission, whose one
/// key is `key`, admitted on `date`.
pub fn founding_reviewer(id: &str, name: Option<&str>, key: ReviewerKey, date: &str) -> Reviewer {
    Reviewer {
        id: id.into(),
        name: name.map(str::to_string),
        role: Role::Maintainer,
        scope: Vec::new(),
        qualification: Vec::new(),
        admitted: Some(date.into()),
        admitted_by: None,
        keys: vec![key],
        revoked: None,
        organisations: Vec::new(),
        separations: Vec::new(),
    }
}

/// A new, **not yet admitted** reviewer with one key and no scope: what a
/// person who is not the founder can register on their own. It counts for
/// nothing until a maintainer admits it (`UnlockedKey::admit`, which signs
/// over role, scope, admitted date and first key).
pub fn unadmitted_reviewer(id: &str, name: Option<&str>, key: ReviewerKey) -> Reviewer {
    Reviewer {
        role: Role::Reviewer,
        admitted: None,
        ..founding_reviewer(id, name, key, "")
    }
}

/// `true` when `k`'s history holds only its `created` event (a key that no
/// one has endorsed, admitted or reset yet).
pub fn is_bare_key(k: &ReviewerKey) -> bool {
    k.history
        .iter()
        .all(|e: &KeyEvent| e.event == KeyEventKind::Created)
}

#[cfg(test)]
#[path = "root_append_tests.rs"]
mod tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "root_append_ivv_tests.rs"]
mod ivv_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "root_append_lifecycle_tests.rs"]
mod lifecycle_tests;
