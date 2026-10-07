//! **Draft `[upstream]` entries** from provenance headers (#767), for a
//! human to confirm: `kovan-cli index --draft-upstream` prints them and
//! never writes `review.md` (human-owned; the upstream confirmation is a
//! human statement, #740).
//!
//! For a folder whose `review.md` has no `upstream` entry, the attribution
//! headers of its files ([`crate::call_graph::upstream`], already parsed
//! into the call graph's `Module::upstream`) are folded into one
//! [`UpstreamTable`]: the repository and commit most of the files record
//! (ties broken by the smaller value), and `files` mapping each of our
//! files to its upstream file(s). Anything a human must settle is a
//! **note**: files that disagree on the repository or commit, a commit
//! recorded abbreviated (resolve it to the full hash in a clone: #764 Q7
//! takes commit hashes only), no repository URL recorded, files of the
//! folder with no header. `confirmed_by` is left as a placeholder the
//! reviewer replaces.

use std::collections::BTreeMap;

use crate::call_graph::upstream::Upstream;
use crate::review::review_md::{EntryMeta, UpstreamEntry, UpstreamTable};

/// A proposed entry and what the human must check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    pub dir: String,
    pub table: UpstreamTable,
    pub notes: Vec<String>,
}

/// The placeholder put in `confirmed_by`.
pub const CONFIRM_PLACEHOLDER: &str = "github:REPLACE-WITH-YOUR-ID";

fn majority<'a>(vals: impl Iterator<Item = &'a str>) -> (Option<String>, usize) {
    let mut n: BTreeMap<&str, usize> = BTreeMap::new();
    for v in vals {
        *n.entry(v).or_default() += 1;
    }
    let distinct = n.len();
    let best = n
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(a.0)))
        .map(|(v, _)| v.to_string());
    (best, distinct)
}

/// The draft for folder `dir` from its files' headers (`file name ->
/// parsed header`; `all_files` lists every `.rs` file of the folder).
/// `None` when no file has a header.
pub fn draft(dir: &str, headers: &BTreeMap<String, Upstream>, all_files: &[String], date: &str) -> Option<Draft> {
    if headers.is_empty() {
        return None;
    }
    let mut notes = Vec::new();
    let (repository, nrepo) = majority(headers.values().filter_map(|u| u.repository.as_deref()));
    let (commit, ncommit) = majority(headers.values().filter_map(|u| u.commit.as_deref()));
    if nrepo > 1 {
        notes.push(format!("files record {nrepo} different repositories; the most common is proposed"));
    }
    if ncommit > 1 {
        notes.push(format!("files record {ncommit} different commits; the most common is proposed"));
    }
    if repository.is_none() {
        let projects: Vec<&str> = headers.values().filter_map(|u| u.project.as_deref()).collect();
        notes.push(format!(
            "no repository URL recorded (project: {}); add it",
            if projects.is_empty() { "unknown".to_string() } else { projects.join(", ") }
        ));
    }
    match &commit {
        None => notes.push("no commit recorded; a port names the commit it was ported from".into()),
        Some(c) if c.len() < 40 => notes.push(format!(
            "commit {c} is abbreviated; resolve it to the full hash (commit hashes only, #764 Q7)"
        )),
        _ => {}
    }
    let missing: Vec<&str> = all_files
        .iter()
        .map(String::as_str)
        .filter(|f| !headers.contains_key(*f))
        .collect();
    if !missing.is_empty() {
        notes.push(format!("no attribution header in: {}", missing.join(", ")));
    }
    let files = headers
        .iter()
        .filter(|(_, u)| !u.files.is_empty())
        .map(|(f, u)| (f.clone(), u.files.join(", ")))
        .collect();
    Some(Draft {
        dir: dir.to_string(),
        table: UpstreamTable {
            is_port: true,
            repository,
            commit: commit.map(|c| c.to_ascii_lowercase()),
            tag: None,
            files,
            routines: BTreeMap::new(),
            confirmed_by: CONFIRM_PLACEHOLDER.to_string(),
            date: date.to_string(),
        },
        notes,
    })
}

impl Draft {
    /// The proposed `review.md` entry, as Markdown, preceded by the notes
    /// as a list. Printed, never written.
    pub fn to_markdown(&self) -> String {
        let entry = UpstreamEntry {
            kovan: EntryMeta {
                id: "upstream".into(),
                kind: "upstream".into(),
                origin: Some("human".into()),
                created: self.table.date.clone(),
                modified: self.table.date.clone(),
                target: None,
            },
            upstream: self.table.clone(),
        };
        let toml = toml::to_string_pretty(&entry).unwrap_or_default();
        let mut s = format!("<!-- DRAFT for {}/review.md: confirm before pasting -->\n", self.dir);
        for n in &self.notes {
            s.push_str(&format!("<!-- check: {n} -->\n"));
        }
        s.push_str(&format!("# Upstream: {}\n\n```toml\n{toml}```\n", self.dir));
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::call_graph::upstream::HeaderStyle;
    use crate::review::review_md::{parse_review_md, Entry};

    fn up(repo: Option<&str>, commit: &str, file: &str) -> Upstream {
        Upstream {
            style: HeaderStyle::Prose,
            line: 1,
            project: Some("NJOY2016".into()),
            repository: repo.map(str::to_string),
            version: None,
            commit: Some(commit.into()),
            source: None,
            files: vec![file.into()],
            licence: None,
            url: None,
        }
    }

    /// Methodology: three files of a folder, two recording the same
    /// repository and abbreviated commit, one another commit, and a fourth
    /// file with no header. The draft takes the majority, lists every file
    /// mapping, and notes the disagreement, the abbreviated commit and the
    /// file with no header; its Markdown parses back as an `upstream` entry
    /// once the placeholder is replaced and the commit expanded. A folder
    /// with no header has no draft.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn headers_fold_into_one_draft_with_notes() {
        let repo = "https://github.com/njoy/NJOY2016";
        let mut h = BTreeMap::new();
        h.insert("a.rs".to_string(), up(Some(repo), "ac5adf5", "src/broadr.f90"));
        h.insert("b.rs".to_string(), up(Some(repo), "ac5adf5", "src/reconr.f90"));
        h.insert("c.rs".to_string(), up(Some(repo), "1234567", "src/groupr.f90"));
        let all: Vec<String> = ["a.rs", "b.rs", "c.rs", "mod.rs"].iter().map(|s| s.to_string()).collect();
        let d = draft("crates/n/src/broadr", &h, &all, "2026-10-07").unwrap();
        assert_eq!(d.table.repository.as_deref(), Some(repo));
        assert_eq!(d.table.commit.as_deref(), Some("ac5adf5"));
        assert_eq!(d.table.files["b.rs"], "src/reconr.f90");
        assert_eq!(d.notes.len(), 3, "{:?}", d.notes);
        assert!(d.notes[2].contains("mod.rs"));
        let full = "ac5adf5000000000000000000000000000000000";
        let md = d
            .to_markdown()
            .replace(CONFIRM_PLACEHOLDER, "github:theodoreOnzGit")
            .replace("\"ac5adf5\"", &format!("\"{full}\""));
        let doc = parse_review_md(&md);
        assert!(doc.unreadable.is_empty(), "{:?}", doc.unreadable);
        assert!(matches!(doc.entries[0].entry, Entry::Upstream(_)));
        assert!(draft("x", &BTreeMap::new(), &all, "2026-10-07").is_none());
    }
}
