//! **The no-concept reason and the upstream link** (GitHub #770).
//!
//! # No concept (#740 U3; #760 question 11)
//!
//! "A function with no concept link can be stamped. It carries a 'no
//! concept' marker with a reason chosen from a list, or `Other: ____`
//! (compulsory, at least 2 characters)." The list is [`NO_CONCEPT_REASONS`];
//! the stored form is the wizard's answer form ([`format_answer`]): the
//! reason's key, or `other: <text>`.
//!
//! For a **ported** function (the folder's confirmed `[upstream]` says it
//! is a port) the reason is **pre-filled** as "upstream control flow /
//! solver structure" and the matching **architecture** artifact is
//! suggested as a `part_of` relation ([`suggest_architecture`]). The
//! reviewer confirms or changes both; nothing is set silently (#760 q11:
//! "it is never set silently"): the prefill is only the form's starting
//! value, and the suggestion is off until ticked.
//!
//! # Upstream opens in the browser (#740 U3)
//!
//! "Upstream opens in the browser at the upstream file on GitHub or
//! GitLab, at the line where possible." [`upstream_url`] builds the link
//! from the folder's `[upstream]` table (repository, commit, our file ->
//! the upstream file, optional per-function routine), reusing the header
//! scanner's host rule ([`kovan_common::call_graph::upstream::link`]):
//! github.com and gitlab.com only, commit-pinned, never a branch.

use kovan_common::artifact::relation::{RelationKind, RelationRecord};
use kovan_common::call_graph::upstream::{link, HeaderStyle, Upstream};
use kovan_common::review::review_md::{ArchitectureEntry, UpstreamTable};
use kovan_common::review::wizard::{format_answer, parse_answer};

/// The reason key pre-filled for a port (#760 question 11).
pub const UPSTREAM_STRUCTURE: &str = "upstream_structure";
/// The `Other: ____` key.
pub const OTHER: &str = "other";

/// The no-concept reasons: (key, label). "upstream control flow / solver
/// structure" is the maintainer's (#740, 2026-10-07); the others are a
/// proposal recorded for the maintainer to confirm (GitHub #770 report).
pub const NO_CONCEPT_REASONS: &[(&str, &str)] = &[
    (
        UPSTREAM_STRUCTURE,
        "upstream control flow / solver structure",
    ),
    ("plumbing", "glue, plumbing or data movement (no physics)"),
    (
        "generic_numerics",
        "a generic numerical or data-structure utility",
    ),
    ("io_format", "input/output, parsing or formatting"),
    ("ui_or_cli", "user interface or command-line glue"),
    (OTHER, "Other"),
];

/// The label of a reason key.
pub fn reason_label(key: &str) -> Option<&'static str> {
    NO_CONCEPT_REASONS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, l)| *l)
}

/// The no-concept part of the wizard form.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NoConceptForm {
    /// The chosen reason key; `None`: the function links a concept (no
    /// marker is written).
    pub choice: Option<String>,
    /// The text of `Other`.
    pub other: String,
    /// The suggested architecture relation is accepted.
    pub link_architecture: bool,
}

impl NoConceptForm {
    /// Start from a stored value (a previous review's `no_concept`, or the
    /// port prefill).
    pub fn from_stored(stored: Option<&str>) -> NoConceptForm {
        let Some(s) = stored else {
            return NoConceptForm::default();
        };
        let (k, t) = parse_answer(s);
        if reason_label(k).is_some() {
            NoConceptForm {
                choice: Some(k.to_string()),
                other: t.unwrap_or("").to_string(),
                link_architecture: false,
            }
        } else {
            // A free-text reason written before the list: kept as Other.
            NoConceptForm {
                choice: Some(OTHER.into()),
                other: s.to_string(),
                link_architecture: false,
            }
        }
    }

    /// What blocks the stamp: an `Other` with under 2 characters.
    pub fn problem(&self) -> Option<String> {
        match self.choice.as_deref() {
            Some(OTHER) if self.other.trim().chars().count() < 2 => {
                Some("No concept: Other needs at least 2 characters".into())
            }
            Some(k) if reason_label(k).is_none() => {
                Some(format!("No concept: unknown reason {k:?}"))
            }
            _ => None,
        }
    }

    /// The value written to `[review] no_concept`, in the wizard's answer
    /// form; `None` when no reason is chosen.
    pub fn stored(&self) -> Option<String> {
        let k = self.choice.as_deref()?;
        Some(if k == OTHER {
            format_answer(k, Some(self.other.trim()))
        } else {
            format_answer(k, None)
        })
    }
}

/// The starting no-concept value (module doc): the reviewer's previous
/// value when re-stamping, else the port prefill for a port, else none.
pub fn no_concept_prefill(previous: Option<&str>, is_port: bool) -> Option<String> {
    match previous {
        Some(p) => Some(p.to_string()),
        None if is_port => Some(UPSTREAM_STRUCTURE.to_string()),
        None => None,
    }
}

/// The architecture node to suggest for function `fn_id`: one listing it
/// as a member, else (a port only) one following the same upstream
/// repository as the folder. Returns the `part_of` relation and the node's
/// id.
pub fn suggest_architecture<'a>(
    fn_id: &str,
    folder_upstream: Option<&UpstreamTable>,
    nodes: impl IntoIterator<Item = &'a ArchitectureEntry>,
) -> Option<RelationRecord> {
    let nodes: Vec<&ArchitectureEntry> = nodes.into_iter().collect();
    let member = nodes
        .iter()
        .find(|a| a.architecture.members.iter().any(|m| m == fn_id));
    let by_repo = || {
        let repo = folder_upstream
            .filter(|u| u.is_port)?
            .repository
            .as_deref()?;
        let norm = |s: &str| {
            s.trim_end_matches('/')
                .trim_end_matches(".git")
                .to_ascii_lowercase()
        };
        nodes.iter().find(|a| {
            a.architecture
                .upstream
                .as_ref()
                .and_then(|u| u.repository.as_deref())
                .is_some_and(|r| norm(r) == norm(repo))
        })
    };
    let node = member.or_else(by_repo)?;
    Some(RelationRecord::new(
        "",
        format!("artifact:{}", node.kovan.id),
        RelationKind::PartOf,
    ))
}

/// `path:12` or `path:12-40` -> (`path`, the line anchor for `host`).
fn split_line(file: &str, gitlab: bool) -> (String, String) {
    if let Some((p, l)) = file.rsplit_once(':') {
        let (a, b) = l.split_once('-').unwrap_or((l, ""));
        let digits = |s: &str| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit());
        if digits(a) && (b.is_empty() || digits(b)) {
            let anchor = match (b.is_empty(), gitlab) {
                (true, _) => format!("#L{a}"),
                (false, false) => format!("#L{a}-L{b}"),
                (false, true) => format!("#L{a}-{b}"),
            };
            return (p.to_string(), anchor);
        }
    }
    (file.to_string(), String::new())
}

/// The browser link to the upstream counterpart of `file_name` (our file,
/// a name in the folder) and, when recorded, of function `fn_id`'s routine
/// (module doc). `None` when the folder is not a port, the host is not
/// GitHub/GitLab, or no commit is recorded.
pub fn upstream_url(u: &UpstreamTable, file_name: &str, fn_id: &str) -> Option<String> {
    if !u.is_port {
        return None;
    }
    if let Some(r) = u.routines.get(fn_id).filter(|r| r.starts_with("https://")) {
        return Some(r.clone());
    }
    let mapped = u.files.get(file_name);
    if let Some(m) = mapped.filter(|m| m.starts_with("https://")) {
        return Some(m.clone());
    }
    let gitlab = u
        .repository
        .as_deref()
        .is_some_and(|r| r.contains("gitlab.com"));
    let (path, anchor) = mapped.map(|m| split_line(m, gitlab)).unwrap_or_default();
    let up = Upstream {
        style: HeaderStyle::KeyValue,
        line: 0,
        project: None,
        repository: u.repository.clone(),
        version: None,
        commit: u.commit.clone(),
        source: None,
        // `link` makes a blob link only for a path with a `/`; a top-level
        // upstream file gets `./`, which `link` strips again.
        files: match path.as_str() {
            "" => Vec::new(),
            p if p.contains('/') => vec![path],
            p => vec![format!("./{p}")],
        },
        licence: None,
        url: None,
    };
    let base = link(&up)?;
    Some(if base.contains("/blob/") {
        format!("{base}{anchor}")
    } else {
        base
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kovan_common::review::review_md::{ArchitectureBody, EntryMeta};
    use std::collections::BTreeMap;

    const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

    fn table(repo: &str, files: &[(&str, &str)]) -> UpstreamTable {
        UpstreamTable {
            is_port: true,
            repository: Some(repo.into()),
            commit: Some(SHA.into()),
            tag: None,
            files: files
                .iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect(),
            routines: BTreeMap::new(),
            confirmed_by: "github:tester".into(),
            date: "2026-10-10".into(),
        }
    }

    fn arch(id: &str, members: &[&str], repo: Option<&str>) -> ArchitectureEntry {
        ArchitectureEntry {
            kovan: EntryMeta {
                id: id.into(),
                kind: "architecture".into(),
                origin: Some("human".into()),
                created: "c".into(),
                modified: "m".into(),
                target: None,
            },
            architecture: ArchitectureBody {
                by: "github:tester".into(),
                date: "2026-10-10".into(),
                signed_at: None,
                commit: SHA.into(),
                members: members.iter().map(|s| s.to_string()).collect(),
                member_paths: Vec::new(),
                upstream: repo.map(|r| Upstream {
                    style: HeaderStyle::KeyValue,
                    line: 0,
                    project: None,
                    repository: Some(r.into()),
                    version: None,
                    commit: Some(SHA.into()),
                    source: None,
                    files: Vec::new(),
                    licence: None,
                    url: None,
                }),
                upstream_tag: None,
                pattern: None,
                signature: None,
            },
            relations: Vec::new(),
        }
    }

    /// Methodology: the #760 q11 rule. Pass: a port with no previous value
    /// starts at "upstream control flow / solver structure"; a previous
    /// value wins; a non-port starts empty; the form round-trips the stored
    /// form and enforces Other >= 2 characters.
    #[test]
    fn port_prefill_and_form_rules() {
        assert_eq!(
            no_concept_prefill(None, true).as_deref(),
            Some(UPSTREAM_STRUCTURE)
        );
        assert_eq!(no_concept_prefill(None, false), None);
        assert_eq!(
            no_concept_prefill(Some("io_format"), true).as_deref(),
            Some("io_format")
        );
        let f = NoConceptForm::from_stored(Some(UPSTREAM_STRUCTURE));
        assert_eq!(f.stored().as_deref(), Some(UPSTREAM_STRUCTURE));
        assert!(!f.link_architecture, "never accepted silently");
        let mut o = NoConceptForm {
            choice: Some(OTHER.into()),
            other: "x".into(),
            link_architecture: false,
        };
        assert!(o.problem().is_some());
        o.other = "a test helper".into();
        assert_eq!(o.problem(), None);
        assert_eq!(o.stored().as_deref(), Some("other: a test helper"));
        assert_eq!(NoConceptForm::from_stored(o.stored().as_deref()), o);
        let legacy = NoConceptForm::from_stored(Some("free text from before"));
        assert_eq!(legacy.choice.as_deref(), Some(OTHER));
        assert_eq!(NoConceptForm::default().stored(), None);
        assert_eq!(
            reason_label(UPSTREAM_STRUCTURE),
            Some("upstream control flow / solver structure")
        );
    }

    /// Methodology: a member match beats a same-repository match; a
    /// non-port gets only the member match. Pass: the `part_of` target is
    /// `artifact:<node id>`.
    #[test]
    fn architecture_suggestion_prefers_membership() {
        let up = table("https://github.com/njoy/NJOY2016", &[]);
        let a = arch(
            "arch-broadr",
            &[],
            Some("https://github.com/njoy/NJOY2016.git"),
        );
        let b = arch("arch-member", &["fn:0123456789abcdef"], None);
        let r = suggest_architecture("fn:0123456789abcdef", Some(&up), [&a, &b]).unwrap();
        assert_eq!(r.target, "artifact:arch-member");
        assert_eq!(r.kind, RelationKind::PartOf);
        let r = suggest_architecture("fn:1111111111111111", Some(&up), [&a, &b]).unwrap();
        assert_eq!(r.target, "artifact:arch-broadr");
        let mut not_port = up.clone();
        not_port.is_port = false;
        assert!(suggest_architecture("fn:1111111111111111", Some(&not_port), [&a, &b]).is_none());
    }

    /// Methodology: GitHub and GitLab links from the `[upstream]` table,
    /// with and without a line, and a routine URL. Pass: commit-pinned
    /// blob links with the host's line anchor; an unknown host gives none.
    #[test]
    fn upstream_links_are_pinned_with_lines() {
        let gh = table(
            "https://github.com/njoy/NJOY2016",
            &[("broadr.rs", "src/broadr.f90:120-180")],
        );
        assert_eq!(
            upstream_url(&gh, "broadr.rs", "fn:x").unwrap(),
            format!("https://github.com/njoy/NJOY2016/blob/{SHA}/src/broadr.f90#L120-L180")
        );
        let gl = table(
            "https://gitlab.com/foam-for-nuclear/offbeat",
            &[("a.rs", "src/a.C:7")],
        );
        assert_eq!(
            upstream_url(&gl, "a.rs", "fn:x").unwrap(),
            format!("https://gitlab.com/foam-for-nuclear/offbeat/-/blob/{SHA}/src/a.C#L7")
        );
        let bare = table("https://github.com/njoy/NJOY2016", &[]);
        assert_eq!(
            upstream_url(&bare, "x.rs", "fn:x").unwrap(),
            format!("https://github.com/njoy/NJOY2016/tree/{SHA}")
        );
        let mut r = bare.clone();
        r.routines.insert(
            "fn:x".into(),
            format!("https://github.com/njoy/NJOY2016/blob/{SHA}/src/x.f90#L3"),
        );
        assert!(upstream_url(&r, "x.rs", "fn:x").unwrap().ends_with("#L3"));
        let other = table("https://example.org/a/b", &[]);
        assert_eq!(upstream_url(&other, "x.rs", "fn:x"), None);
    }
}
