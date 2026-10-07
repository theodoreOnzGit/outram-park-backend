//! Small shared values of the code-review schema and their validation:
//! reviewer identifiers, hashes, commits, pinned upstream URLs and the
//! authorship of a reviewed change.
//!
//! Every validator returns a typed error ([`FieldError`]). A value that
//! fails validation makes the whole entry unreadable (= no review, maintainer
//! 2026-10-07 on #739); nothing is guessed or repaired.

use serde::{Deserialize, Serialize};

/// Why one field of an entry is not acceptable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldError {
    /// `field` is not `sha256:` followed by 64 lowercase hex digits.
    BadHash { field: String, value: String },
    /// `field` is not a full or abbreviated git commit id (7 to 40, or 64,
    /// lowercase hex digits).
    BadCommit { field: String, value: String },
    /// Not `github:<user>`, `gitlab:<user>`, `orcid:<iD>` or an email.
    BadReviewerId(String),
    /// A rung other than 3 or 4 recorded in a review (5 is derived, never
    /// written).
    BadRung(u8),
    /// A free-text field that must hold at least two characters.
    TooShort { field: String },
    /// An upstream link is not pinned to a commit or a tag.
    UnpinnedUrl(UrlPinError),
    /// The `[kovan] target` is not a `code:` target.
    BadTarget(String),
}

impl std::fmt::Display for FieldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadHash { field, value } => {
                write!(f, "{field} = {value:?} is not sha256:<64 hex digits>")
            }
            Self::BadCommit { field, value } => {
                write!(f, "{field} = {value:?} is not a git commit id")
            }
            Self::BadReviewerId(v) => write!(
                f,
                "reviewer {v:?} is not github:<user>, gitlab:<user>, orcid:<iD> or an email"
            ),
            Self::BadRung(r) => write!(f, "rung {r}: a review records rung 3 or 4"),
            Self::TooShort { field } => write!(f, "{field} needs at least 2 characters"),
            Self::UnpinnedUrl(e) => write!(f, "{e}"),
            Self::BadTarget(t) => write!(f, "target {t:?} is not code:<file>::<item>"),
        }
    }
}

impl std::error::Error for FieldError {}

/// The prefix of every hash in the schema.
pub const HASH_PREFIX: &str = "sha256:";

/// `sha256:` + 64 lowercase hex digits.
pub fn check_hash(field: &str, value: &str) -> Result<(), FieldError> {
    let ok = value
        .strip_prefix(HASH_PREFIX)
        .is_some_and(|h| h.len() == 64 && h.bytes().all(is_lower_hex));
    ok.then_some(()).ok_or_else(|| FieldError::BadHash {
        field: field.to_string(),
        value: value.to_string(),
    })
}

/// A git commit id: 7 to 40 lowercase hex digits (SHA-1, possibly
/// abbreviated) or 64 (SHA-256 repositories).
pub fn is_commit_id(value: &str) -> bool {
    let n = value.len();
    ((7..=40).contains(&n) || n == 64) && value.bytes().all(is_lower_hex)
}

/// [`is_commit_id`] as a field check.
pub fn check_commit(field: &str, value: &str) -> Result<(), FieldError> {
    is_commit_id(value)
        .then_some(())
        .ok_or_else(|| FieldError::BadCommit {
            field: field.to_string(),
            value: value.to_string(),
        })
}

fn is_lower_hex(b: u8) -> bool {
    b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
}

/// At least two characters after trimming (the wizard's `Other: ____` rule
/// and the needs-fix note, #740 U3/U4).
pub fn check_text(field: &str, value: &str) -> Result<(), FieldError> {
    (value.trim().chars().count() >= 2)
        .then_some(())
        .ok_or_else(|| FieldError::TooShort {
            field: field.to_string(),
        })
}

/// How a reviewer is identified (#740 U4, corrected 2026-10-07): a GitHub or
/// GitLab username or an ORCID iD first, an email as the fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReviewerIdKind {
    GitHub,
    GitLab,
    Orcid,
    Email,
}

/// Classify and check a reviewer id (`github:theodoreOnzGit`,
/// `gitlab:someone`, `orcid:0000-0002-1825-0097`, `someone@example.org`).
pub fn reviewer_id_kind(id: &str) -> Result<ReviewerIdKind, FieldError> {
    let bad = || FieldError::BadReviewerId(id.to_string());
    let user_ok = |u: &str| {
        !u.is_empty() && u.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
    };
    if let Some(u) = id.strip_prefix("github:") {
        return user_ok(u).then_some(ReviewerIdKind::GitHub).ok_or_else(bad);
    }
    if let Some(u) = id.strip_prefix("gitlab:") {
        return user_ok(u).then_some(ReviewerIdKind::GitLab).ok_or_else(bad);
    }
    if let Some(o) = id.strip_prefix("orcid:") {
        let digits: Vec<char> = o.chars().filter(|c| *c != '-').collect();
        let ok = o.len() == 19
            && digits.len() == 16
            && digits[..15].iter().all(char::is_ascii_digit)
            && (digits[15].is_ascii_digit() || digits[15] == 'X');
        return ok.then_some(ReviewerIdKind::Orcid).ok_or_else(bad);
    }
    match id.split_once('@') {
        Some((local, domain))
            if !local.is_empty()
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
                && !id.chars().any(char::is_whitespace) =>
        {
            Ok(ReviewerIdKind::Email)
        }
        _ => Err(bad()),
    }
}

/// Who authored the change a review certifies, read from the commits'
/// trailers (maintainer, #764/#771, 2026-10-07).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorshipKind {
    /// Every commit of the change carries the agent attribution trailer.
    Agent,
    /// No commit carries it.
    Human,
    /// Some do and some do not.
    Mixed,
}

impl AuthorshipKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Human => "human",
            Self::Mixed => "mixed",
        }
    }
}

/// `[review.authorship]`: the authorship of the reviewed change, with the
/// agent session links found in its trailers. Signed with the stamp.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeAuthorship {
    pub kind: AuthorshipKind,
    /// `Claude-Session:` links, sorted and de-duplicated.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sessions: Vec<String>,
}

/// Whether one commit message carries the agent attribution trailer
/// (`Co-Authored-By: Claude…` or `Claude-Session:`), and the session link
/// when there is one. Case-insensitive on the trailer key.
pub fn agent_trailer(message: &str) -> (bool, Option<String>) {
    let mut agent = false;
    let mut session = None;
    for line in message.lines() {
        let line = line.trim();
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        if key == "co-authored-by" && value.to_ascii_lowercase().starts_with("claude") {
            agent = true;
        } else if key == "claude-session" {
            agent = true;
            if !value.is_empty() {
                session = Some(value.to_string());
            }
        }
    }
    (agent, session)
}

/// The authorship of a change made of the commits with these messages:
/// agent if all carry the trailer, human if none does, mixed otherwise.
/// `None` for an empty change (nothing to attribute).
pub fn authorship_from_messages(messages: &[String]) -> Option<ChangeAuthorship> {
    if messages.is_empty() {
        return None;
    }
    let mut agents = 0usize;
    let mut sessions: Vec<String> = Vec::new();
    for m in messages {
        let (a, s) = agent_trailer(m);
        agents += usize::from(a);
        sessions.extend(s);
    }
    sessions.sort();
    sessions.dedup();
    let kind = match agents {
        0 => AuthorshipKind::Human,
        n if n == messages.len() => AuthorshipKind::Agent,
        _ => AuthorshipKind::Mixed,
    };
    Some(ChangeAuthorship { kind, sessions })
}

/// What a pinned upstream URL is pinned to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UrlPin {
    /// A commit id.
    Commit(String),
    /// A tag (a ref that looks like a version; see [`check_pinned_url`]).
    Tag(String),
    /// Not a GitHub or GitLab file/tree link, so there is no ref to check
    /// (a paper's DOI, a project home page). Reported, not hidden.
    NotARepositoryLink,
}

/// Why an upstream URL is refused (maintainer, #764, 2026-10-07: "upstream
/// links ... must be pinned to a commit").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UrlPinError {
    /// The ref is a branch name (`main`, `master`, `develop`, …): the link
    /// moves when the branch does.
    BranchRef { url: String, branch: String },
    /// The ref is neither a commit id nor version-like, so it cannot be told
    /// apart from a branch; write the commit id instead.
    UnrecognisedRef { url: String, reference: String },
    /// A GitHub/GitLab repository link with no ref at all.
    NoRef { url: String },
}

impl std::fmt::Display for UrlPinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BranchRef { url, branch } => write!(
                f,
                "{url} points at branch {branch:?}; pin it to a commit id or a tag"
            ),
            Self::UnrecognisedRef { url, reference } => write!(
                f,
                "{url}: cannot tell whether {reference:?} is a tag or a branch; use the commit id"
            ),
            Self::NoRef { url } => write!(f, "{url} names no commit; pin it to a commit id"),
        }
    }
}

/// Branch names refused outright.
pub const BRANCH_NAMES: &[&str] = &[
    "main", "master", "develop", "dev", "devel", "trunk", "head", "default", "stable", "latest",
    "nightly", "next", "release", "gh-pages",
];

/// Check that a GitHub or GitLab link is pinned (maintainer, #764,
/// 2026-10-07): the ref after `/blob/`, `/tree/`, `/raw/`, `/-/blob/`,
/// `/-/tree/`, `/-/raw/` (or the third path segment of
/// `raw.githubusercontent.com`) must be a commit id or a tag.
///
/// **A tag cannot be told from a branch by its name alone**, and this check
/// has no network. The rule: a commit id passes; a known branch name
/// ([`BRANCH_NAMES`], case-insensitive) is refused; a ref with a digit and
/// no `/` passes as a tag (`v1.2.3`, `OpenFOAM-v2306`, `2016.76`); anything
/// else is refused as unrecognised, with "use the commit id". A GitHub
/// repository link with no ref (`https://github.com/o/r`) is refused. Any
/// other URL is not a repository file link and is reported as such.
pub fn check_pinned_url(url: &str) -> Result<UrlPin, UrlPinError> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    let rest = rest.split(['?', '#']).next().unwrap_or(rest);
    let mut segs = rest.split('/');
    let host = segs.next().unwrap_or("").to_ascii_lowercase();
    let segs: Vec<&str> = segs.filter(|s| !s.is_empty()).collect();
    let reference = if host == "raw.githubusercontent.com" {
        segs.get(2).copied()
    } else if let Some(i) = segs.iter().position(|s| *s == "-") {
        // GitLab: <group…>/<project>/-/blob/<ref>/…
        match segs.get(i + 1) {
            Some(&("blob" | "tree" | "raw" | "commit" | "commits")) => segs.get(i + 2).copied(),
            _ => return Ok(UrlPin::NotARepositoryLink),
        }
    } else if host == "github.com" || host.ends_with(".github.com") {
        match segs.get(2) {
            Some(&("blob" | "tree" | "raw" | "commit" | "commits")) => segs.get(3).copied(),
            None if segs.len() == 2 => {
                return Err(UrlPinError::NoRef {
                    url: url.to_string(),
                })
            }
            _ => return Ok(UrlPin::NotARepositoryLink),
        }
    } else {
        return Ok(UrlPin::NotARepositoryLink);
    };
    let Some(r) = reference else {
        return Err(UrlPinError::NoRef {
            url: url.to_string(),
        });
    };
    classify_ref(url, r)
}

fn classify_ref(url: &str, r: &str) -> Result<UrlPin, UrlPinError> {
    if is_commit_id(&r.to_ascii_lowercase()) && r.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Ok(UrlPin::Commit(r.to_string()));
    }
    if BRANCH_NAMES.iter().any(|b| b.eq_ignore_ascii_case(r)) {
        return Err(UrlPinError::BranchRef {
            url: url.to_string(),
            branch: r.to_string(),
        });
    }
    if r.chars().any(|c| c.is_ascii_digit()) {
        return Ok(UrlPin::Tag(r.to_string()));
    }
    Err(UrlPinError::UnrecognisedRef {
        url: url.to_string(),
        reference: r.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: the maintainer's pinning rule (#764, 2026-10-07) on
    /// GitHub, GitLab (including a self-hosted GitLab such as
    /// develop.openfoam.com) and raw links: commits and tags pass, branches
    /// and unrecognised refs are refused with typed errors.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn upstream_urls_must_be_pinned() {
        let sha = "4a5b6c7d8e9f00112233445566778899aabbccdd";
        let ok = [
            format!("https://github.com/njoy/NJOY2016/blob/{sha}/src/broadr.f90#L10"),
            "https://github.com/njoy/NJOY2016/blob/2016.76/src/broadr.f90".to_string(),
            format!("https://develop.openfoam.com/Development/openfoam/-/blob/{sha}/src/x.C"),
            "https://gitlab.com/g/sub/p/-/tree/v1.2.3/src".to_string(),
            format!("https://raw.githubusercontent.com/o/r/{sha}/f.rs"),
        ];
        for u in &ok {
            assert!(check_pinned_url(u).is_ok(), "{u}");
        }
        assert_eq!(
            check_pinned_url("https://github.com/njoy/NJOY2016/blob/master/src/broadr.f90"),
            Err(UrlPinError::BranchRef {
                url: "https://github.com/njoy/NJOY2016/blob/master/src/broadr.f90".into(),
                branch: "master".into()
            })
        );
        for bad in [
            "https://github.com/o/r/blob/main/x.rs",
            "https://gitlab.com/g/p/-/blob/main/x.rs",
            "https://develop.openfoam.com/Development/openfoam/-/blob/develop/src/x.C",
            "https://github.com/o/r/tree/feature-branch/src",
            "https://github.com/o/r",
        ] {
            assert!(check_pinned_url(bad).is_err(), "{bad}");
        }
        assert_eq!(
            check_pinned_url("https://doi.org/10.1016/j.anucene.2020.1"),
            Ok(UrlPin::NotARepositoryLink)
        );
    }

    /// Methodology: reviewer ids of each accepted kind, and rejects.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn reviewer_ids() {
        assert_eq!(reviewer_id_kind("github:theodoreOnzGit"), Ok(ReviewerIdKind::GitHub));
        assert_eq!(reviewer_id_kind("gitlab:a-b"), Ok(ReviewerIdKind::GitLab));
        assert_eq!(reviewer_id_kind("orcid:0000-0002-1825-0097"), Ok(ReviewerIdKind::Orcid));
        assert_eq!(reviewer_id_kind("a@b.org"), Ok(ReviewerIdKind::Email));
        for bad in ["Theodore Ong", "github:", "orcid:123", "a@b", "github:a b"] {
            assert!(reviewer_id_kind(bad).is_err(), "{bad}");
        }
    }

    /// Methodology: authorship from trailers: all, none and some commits
    /// carrying `Co-Authored-By: Claude…` / `Claude-Session:`.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn authorship_from_trailers() {
        let agent = "fix\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>\nClaude-Session: https://claude.ai/code/session_1".to_string();
        let human = "fix by hand\n\nCo-Authored-By: Someone <s@x.org>".to_string();
        let a = authorship_from_messages(&[agent.clone()]).unwrap();
        assert_eq!(a.kind, AuthorshipKind::Agent);
        assert_eq!(a.sessions, ["https://claude.ai/code/session_1"]);
        assert_eq!(authorship_from_messages(&[human.clone()]).unwrap().kind, AuthorshipKind::Human);
        assert_eq!(authorship_from_messages(&[agent, human]).unwrap().kind, AuthorshipKind::Mixed);
        assert_eq!(authorship_from_messages(&[]), None);
    }
}
