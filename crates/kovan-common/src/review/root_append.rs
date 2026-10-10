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

use super::root::{KeyEvent, KeyEventKind, ReviewRoot, Reviewer, ReviewerKey, RootError, Role};

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
    let mut end = heads
        .iter()
        .find(|(i, h)| *i > start && !h.starts_with("reviewer."))
        .map_or(lines.len(), |(i, _)| *i);
    if end < lines.len() {
        while end > start + 1 && {
            let t = lines[end - 1].trim();
            t.is_empty() || t.starts_with('#')
        } {
            end -= 1;
        }
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
    let mut text: String = lines[..end].concat();
    text = with_newline(&text);
    text.push('\n');
    text.push_str(&block);
    if end < lines.len() {
        text.push('\n');
    }
    text.push_str(&lines[end..].concat());
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
