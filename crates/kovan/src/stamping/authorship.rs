//! **Who authored a reviewed change** (GitHub #771, #764 `authorship`;
//! maintainer 2026-10-07: "The stamp records authorship of the reviewed
//! change, e.g. reviewed an agent-authored change at b81e"). Agent
//! authorship is read from the commit trailer
//! ([`kovan_common::review::types::agent_trailer`]: `Co-Authored-By:
//! Claude…` or `Claude-Session:`), never guessed from the author name.
//!
//! **The change** is the commits that touched the function's current lines
//! (`git log -L<a>,<b>:<file>`, which follows the lines back through
//! history), limited to those after `since` when given: the commit of the
//! reviewer's previous review of the function (a re-review reviews what
//! changed since), else the function's whole history (a first review
//! reviews all of it). The kind is agent when every such commit carries
//! the trailer, human when none does, mixed otherwise
//! ([`kovan_common::review::types::authorship_from_messages`]), with the
//! session links found.

use std::path::Path;

use kovan_common::review::types::{authorship_from_messages, ChangeAuthorship};

use super::git;

/// The messages of the commits up to `head` (after `since`, when given)
/// that touched lines `a..=b` of `file` (module doc). Empty when git
/// cannot say (no such file at `head`, a bad range).
pub fn change_messages(
    root: &Path,
    head: &str,
    file: &str,
    lines: [u32; 2],
    since: Option<&str>,
) -> Vec<String> {
    let [a, b] = lines;
    if a == 0 || b < a || head.is_empty() {
        return Vec::new();
    }
    let range = match since {
        Some(s) if !s.is_empty() => format!("{s}..{head}"),
        _ => head.to_string(),
    };
    let spec = format!("-L{a},{b}:{file}");
    let Ok(out) = git::git(root, &["log", "-s", "--format=%x1e%B", &spec, &range]) else {
        return Vec::new();
    };
    out.split('\x1e')
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .map(str::to_string)
        .collect()
}

/// The authorship of the change to lines `a..=b` of `file` (module doc);
/// `None` when no commit touched them in the range.
pub fn change_authorship(
    root: &Path,
    head: &str,
    file: &str,
    lines: [u32; 2],
    since: Option<&str>,
) -> Option<ChangeAuthorship> {
    authorship_from_messages(&change_messages(root, head, file, lines, since))
}

/// One line for a row's details or a stamp: "agent-authored (2
/// sessions)", "human-authored", "mixed: agent and human".
pub fn describe(a: &ChangeAuthorship) -> String {
    use kovan_common::review::types::AuthorshipKind;
    let kind = match a.kind {
        AuthorshipKind::Agent => "agent-authored",
        AuthorshipKind::Human => "human-authored",
        AuthorshipKind::Mixed => "mixed: agent and human",
    };
    match a.sessions.len() {
        0 => kind.to_string(),
        1 => format!("{kind}, session {}", a.sessions[0]),
        n => format!("{kind}, {n} sessions"),
    }
}
