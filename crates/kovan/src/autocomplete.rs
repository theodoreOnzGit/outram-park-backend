//! Citation (`@`) + wiki (`[[`) autocomplete (§29, §30, `op-9vo6.16`).
//!
//! The candidate-generation half of both completions: given what the user
//! has typed after the trigger character, return matches ranked for
//! display. This module is UI-agnostic — `app::kvim_editor`
//! is what detects the trigger and shows a popup; this is what it queries.
//!
//! # Not blocked on `kopitiam-bibliography`
//!
//! An earlier note on this step (and on the GitHub issue thread) said the
//! `@` half was blocked on `op-k25f` (`kopitiam-bibliography`, unpublished)
//! for lack of a BibTeX parser. That was corrected once `op-b1y5` shipped:
//! `kovan_literature::parse_bib_entries` already exists (20 tests), and its
//! `BibEntry::cite_key`/`fields` are exactly what fuzzy citation search
//! needs. `kopitiam-bibliography` remains a real future upgrade (BibLaTeX
//! emission, a citation graph, DOI/identifier handling) but is not a
//! prerequisite for this step.
//!
//! # "Fuzzy", scoped
//!
//! §29/§30 both say "fuzzy". This pass implements case-insensitive
//! substring matching across the relevant fields (citekey, author, title,
//! year, DOI for citations; id/name/path for wiki targets) rather than a
//! scored fuzzy-matching algorithm (e.g. subsequence scoring) — it already
//! satisfies "the user must not have to memorise citation keys" (searching
//! "wang" or "2018" both find `wang2018multiphysics`), and pulling in a
//! fuzzy-matching crate for one completion list is not worth a new
//! dependency at this stage. Upgrading the ranking is a pure addition
//! later, not a breaking change to this module's shape.

use crate::index::KnowledgeIndex;
use crate::research_record::ResearchRecordIndex;
use crate::root::KovanRoot;

/// One completion candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// Shown in the completion popup.
    pub label: String,
    /// What replaces the query text when this candidate is chosen — e.g.
    /// `@wang2018multiphysics` (the caller wraps it in `[...]`) or
    /// `wang2018multiphysics#table-4-4`.
    pub insert_text: String,
    /// Extra context shown alongside the label (title/author/year for a
    /// citation; the kind of thing a wiki target is).
    pub detail: String,
}

fn matches_query(query: &str, haystacks: &[&str]) -> bool {
    if query.is_empty() {
        return true;
    }
    let query = query.to_lowercase();
    haystacks.iter().any(|h| h.to_lowercase().contains(&query))
}

/// §29: fuzzy bibliography completion for `@`, searchable by citekey,
/// author, title, year, DOI. Reads `root`'s bibliography fresh each call —
/// see the module doc on why a cache isn't worth it here.
pub fn citation_candidates(root: &KovanRoot, query: &str) -> Vec<Candidate> {
    let Ok(text) = std::fs::read_to_string(root.bibliography_path()) else { return Vec::new() };
    let Ok(entries) = kovan_literature::parse_bib_entries(&text) else { return Vec::new() };

    let mut out: Vec<Candidate> = entries
        .into_iter()
        .filter(|e| {
            let author = e.fields.get("author").map(String::as_str).unwrap_or("");
            let title = e.fields.get("title").map(String::as_str).unwrap_or("");
            let year = e.fields.get("year").map(String::as_str).unwrap_or("");
            let doi = e.fields.get("doi").map(String::as_str).unwrap_or("");
            matches_query(query, &[&e.cite_key, author, title, year, doi])
        })
        .map(|e| {
            let title = e.fields.get("title").cloned().unwrap_or_default();
            let year = e.fields.get("year").cloned().unwrap_or_default();
            let author = e.fields.get("author").cloned().unwrap_or_default();
            let detail = [author, year, title].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" — ");
            Candidate { label: e.cite_key.clone(), insert_text: e.cite_key, detail }
        })
        .collect();
    out.sort_by(|a, b| a.label.cmp(&b.label));
    out
}

/// §30: wiki completion for `[[`, across papers, topics/projects/subtopics.
pub fn wiki_candidates(index: &KnowledgeIndex, query: &str) -> Vec<Candidate> {
    let mut out = Vec::new();

    for paper in &index.papers {
        if matches_query(query, &[&paper.citekey]) {
            out.push(Candidate { label: paper.citekey.clone(), insert_text: paper.citekey.clone(), detail: "paper".to_string() });
        }
    }
    for collection in &index.collections {
        if matches_query(query, &[&collection.path, &collection.name]) {
            let kind = match collection.kind {
                crate::entity::EntityKind::Topic => "topic",
                crate::entity::EntityKind::Project => "project",
                crate::entity::EntityKind::Paper => continue, // collections never carry this kind
            };
            out.push(Candidate { label: collection.name.clone(), insert_text: collection.path.clone(), detail: kind.to_string() });
        }
    }
    out.sort_by(|a, b| a.label.cmp(&b.label));
    out
}

/// §30's `#`-completion: once a paper has been selected in a `[[...]]`
/// link, complete its artifacts/anchors.
pub fn artifact_candidates(index: &ResearchRecordIndex, query: &str) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = index
        .artifacts()
        .iter()
        .filter(|a| matches_query(query, &[a.id(), &a.heading]))
        .map(|a| Candidate { label: a.heading.clone(), insert_text: a.id().to_string(), detail: format!("{:?}", a.kind()) })
        .collect();
    out.sort_by(|a, b| a.label.cmp(&b.label));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{Access, CiteKey, EntityConfig};
    use crate::root::RootConfig;

    fn make_root() -> (tempfile::TempDir, KovanRoot) {
        let dir = tempfile::tempdir().unwrap();
        let root = KovanRoot::create(dir.path(), RootConfig::new("lib", "Lib"), false).unwrap();
        std::fs::write(
            root.bibliography_path(),
            "@article{wang2018multiphysics,\n  author = {Wang, Yan},\n  title = {A Multiphysics Study},\n  year = {2018},\n}\n\
             @article{lee2020corrosion,\n  author = {Lee, Kim},\n  title = {Corrosion in HTGRs},\n  year = {2020},\n}\n",
        )
        .unwrap();
        (dir, root)
    }

    #[test]
    fn citation_candidates_matches_across_citekey_author_title_year() {
        let (_dir, root) = make_root();
        assert_eq!(citation_candidates(&root, "wang").len(), 1);
        assert_eq!(citation_candidates(&root, "2020").len(), 1);
        assert_eq!(citation_candidates(&root, "corrosion").len(), 1);
        assert_eq!(citation_candidates(&root, "").len(), 2, "empty query lists everything");
        assert!(citation_candidates(&root, "nonexistentxyz").is_empty());
    }

    #[test]
    fn citation_candidate_detail_carries_author_year_title() {
        let (_dir, root) = make_root();
        let c = &citation_candidates(&root, "wang")[0];
        assert_eq!(c.insert_text, "wang2018multiphysics");
        assert!(c.detail.contains("Yan") && c.detail.contains("2018"));
    }

    #[test]
    fn wiki_candidates_covers_papers_and_collections() {
        let (_dir, root) = make_root();
        EntityConfig::topic("htgrs", "HTGRs").save(&root.topics_dir().join("htgrs")).unwrap();
        EntityConfig::paper(CiteKey::parse("wang2018multiphysics").unwrap(), Access::Open)
            .with_topics(["htgrs"])
            .save_paper(&root.paper_dir("wang2018multiphysics"))
            .unwrap();

        let index = KnowledgeIndex::rebuild(&root);
        let all = wiki_candidates(&index, "");
        assert!(all.iter().any(|c| c.insert_text == "wang2018multiphysics" && c.detail == "paper"));
        assert!(all.iter().any(|c| c.insert_text == "htgrs" && c.detail == "topic"));

        let filtered = wiki_candidates(&index, "wang");
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn artifact_candidates_matches_heading_and_id() {
        let (_dir, root) = make_root();
        EntityConfig::paper(CiteKey::parse("wang2018multiphysics").unwrap(), Access::Open)
            .with_topics(["htgrs"])
            .save_paper(&root.paper_dir("wang2018multiphysics"))
            .unwrap();
        let mut session = crate::session::PaperSession::open(&root, "wang2018multiphysics").unwrap();
        session.append_block(
            "## Table 4.4\n\n```toml\n[kovan]\nid = \"table-4-4\"\nkind = \"digitised_table\"\ncreated = \"c\"\nmodified = \"m\"\n\n[source]\npage = 3\n```\n",
        );
        let index = ResearchRecordIndex::from_session(&session);

        assert_eq!(artifact_candidates(&index, "table").len(), 1);
        assert_eq!(artifact_candidates(&index, "4.4").len(), 1);
        assert!(artifact_candidates(&index, "nope").is_empty());
    }
}
