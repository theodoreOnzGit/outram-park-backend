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

use crate::artifact::{parse_document, ArtifactKind};
use crate::entity::EntityKind;
use crate::graph::{artifact_node, collection_node, paper_node};
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
    let Ok(text) = std::fs::read_to_string(root.bibliography_path()) else {
        return Vec::new();
    };
    let Ok(entries) = kovan_literature::parse_bib_entries(&text) else {
        return Vec::new();
    };

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
            let detail = [author, year, title]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" — ");
            Candidate {
                label: e.cite_key.clone(),
                insert_text: e.cite_key,
                detail,
            }
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
            out.push(Candidate {
                label: paper.citekey.clone(),
                insert_text: paper.citekey.clone(),
                detail: "paper".to_string(),
            });
        }
    }
    for collection in &index.collections {
        if matches_query(query, &[&collection.path, &collection.name]) {
            let kind = match collection.kind {
                crate::entity::EntityKind::Topic => "topic",
                crate::entity::EntityKind::Project => "project",
                crate::entity::EntityKind::Paper => continue, // collections never carry this kind
            };
            out.push(Candidate {
                label: collection.name.clone(),
                insert_text: collection.path.clone(),
                detail: kind.to_string(),
            });
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
        .map(|a| Candidate {
            label: a.heading.clone(),
            insert_text: a.id().to_string(),
            detail: format!("{:?}", a.kind()),
        })
        .collect();
    out.sort_by(|a, b| a.label.cmp(&b.label));
    out
}

/// The kind of node a [`LibraryCandidate`] identifies (GitHub issue #35's
/// layer-1 prototype finding, `op-30um.4`: `artifact_candidates` above is
/// per-paper, so "Add connection..." can only offer artifacts from
/// whichever single [`ResearchRecordIndex`] the caller happens to have
/// open — a prototype had to fan out over every paper by hand to work
/// around it).
///
/// Declaration order (`Paper` < `Artifact` < `Topic` < `Project`) is the
/// tie-break order [`library_candidates`] sorts by when two candidates
/// share a label — it carries no meaning beyond "some fixed order", it
/// just has to be a fixed one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CandidateKind {
    /// A paper, identified by [`crate::graph::paper_node`].
    Paper,
    /// An artifact belonging to some paper, identified by
    /// [`crate::graph::artifact_node`].
    Artifact,
    /// A topic collection, identified by [`crate::graph::collection_node`].
    Topic,
    /// A project collection, identified by [`crate::graph::collection_node`].
    Project,
}

/// One hit from [`library_candidates`]: a typed node identity plus the
/// completion payload a caller would otherwise get from
/// [`citation_candidates`]/[`wiki_candidates`]/[`artifact_candidates`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryCandidate {
    /// Which of the four library-wide node kinds this is.
    pub kind: CandidateKind,
    /// The canonical node identity string — `paper:<citekey>`,
    /// `artifact:<citekey>#<id>`, or `collection:<path>` — built with
    /// [`crate::graph::paper_node`]/[`crate::graph::artifact_node`]/
    /// [`crate::graph::collection_node`] so this can never disagree with
    /// what [`crate::graph::KnowledgeGraph`] calls the same node.
    pub node: String,
    /// The display/insert payload, same shape as every other completion
    /// list in this module.
    pub candidate: Candidate,
}

/// A lower-case, space-separated display label for an [`ArtifactKind`],
/// used in [`LibraryCandidate`] detail text so a user sees "digitised
/// graph" rather than the `Debug` rendering `DigitisedGraph` — this module
/// cannot add a `Display` impl on `ArtifactKind` itself (that type lives in
/// `crate::artifact`, owned elsewhere), so the mapping lives here instead.
fn artifact_kind_label(kind: ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Note => "note",
        ArtifactKind::Annotation => "annotation",
        ArtifactKind::SourceReference => "source reference",
        ArtifactKind::Formula => "formula",
        ArtifactKind::DigitisedTable => "digitised table",
        ArtifactKind::DigitisedGraph => "digitised graph",
    }
}

/// One library-wide fuzzy search across every paper, artifact, topic and
/// project `root` currently has, so a single field (e.g. an "Add
/// connection..." target picker) can search the whole library instead of
/// fanning out over each paper's own [`ResearchRecordIndex`] by hand.
///
/// `kinds` restricts which [`CandidateKind`]s are searched at all — pass
/// `&[]` to search every kind. Filtering this way (rather than searching
/// everything and letting the caller discard rows) is what lets a caller
/// skip the expensive half described below when it doesn't need artifacts.
///
/// # Paper matching reuses [`citation_candidates`], not just the citekey
///
/// A `Paper` hit is produced by calling [`citation_candidates`] (which
/// searches citekey, author, title, year and DOI from the bibliography)
/// and keeping only the results whose citekey is also in `index.papers` —
/// the bibliography can list papers that were never added to this library,
/// and those must not appear here. This means a query for an author
/// surname or a title word finds a paper here exactly as it would in the
/// `@`-completion popup; matching on citekey alone was tried and rejected
/// because it silently narrowed this surface below what already shipped.
///
/// One consequence: a paper that is in `index.papers` but has **no**
/// matching entry in the bibliography (`root.bibliography_path()`) will
/// not surface as a `Paper` hit — its citekey has nothing to be matched
/// against. Its artifacts and any topic/project it belongs to are
/// unaffected, since those are read from `index`/disk directly, not from
/// the bibliography.
///
/// # Cost, and what it does not cache
///
/// - Paper matching parses the bibliography file once per call (via
///   [`citation_candidates`], same cost as calling it directly) and then
///   filters against `index.papers` in memory. Topic/project matching
///   reuses `index` (a [`KnowledgeIndex`] the caller already scanned, e.g.
///   via [`KnowledgeIndex::load_or_rebuild`]) entirely in memory — no
///   filesystem walk beyond the one bibliography read, cheap enough to run
///   on every keystroke.
/// - Cross-paper artifact matching is the expensive part: there is no
///   library-wide artifact cache yet (only [`crate::graph::KnowledgeGraph`]
///   has anything close, and it only records artifacts that carry a
///   topic/project classification — not enough to search *all* artifacts
///   by heading), so this function re-reads and re-parses **every paper's**
///   canonical Markdown from disk on every call — O(papers) file reads,
///   each proportional to that paper's file size. On a library with many
///   long papers this dominates; pass `kinds` without
///   [`CandidateKind::Artifact`] to skip it entirely.
/// - Because artifact matching reads from disk rather than from a live
///   [`crate::session::PaperSession`] buffer, it can miss an *unsaved* edit
///   in whichever paper the caller currently has open — the same
///   disk-vs-buffer gap [`ResearchRecordIndex`]'s module doc warns about.
///   A caller that also has a session open for one particular paper and
///   needs that paper's freshest state should still consult
///   [`artifact_candidates`] over that session's own
///   [`ResearchRecordIndex`] for that paper and merge the two result sets;
///   this function alone is sufficient for every other paper in the
///   library.
///
/// # Ranking
///
/// Deterministic and stable for a given `(root, index, query, kinds)`:
/// primarily by label (case-sensitive, matching every other list in this
/// module), then by [`CandidateKind`], then by node identity string —
/// never by iteration order over `index.papers`/`index.collections` (both
/// already `Vec`s, not hash maps) or over the filesystem walk order used to
/// build `index` in the first place.
///
/// A query that matches nothing returns an empty `Vec`, never an error —
/// same total contract as every other function in this module.
pub fn library_candidates(
    root: &KovanRoot,
    index: &KnowledgeIndex,
    query: &str,
    kinds: &[CandidateKind],
) -> Vec<LibraryCandidate> {
    let want = |k: CandidateKind| kinds.is_empty() || kinds.contains(&k);
    let mut out: Vec<LibraryCandidate> = Vec::new();

    if want(CandidateKind::Paper) {
        for candidate in citation_candidates(root, query) {
            if !index.papers.iter().any(|p| p.citekey == candidate.insert_text) {
                continue; // bibliography entry with no corresponding library paper
            }
            let node = paper_node(&candidate.insert_text);
            out.push(LibraryCandidate {
                kind: CandidateKind::Paper,
                node,
                candidate,
            });
        }
    }

    if want(CandidateKind::Topic) || want(CandidateKind::Project) {
        for collection in &index.collections {
            let kind = match collection.kind {
                EntityKind::Topic => CandidateKind::Topic,
                EntityKind::Project => CandidateKind::Project,
                EntityKind::Paper => continue, // collections never carry this kind
            };
            if !want(kind) || !matches_query(query, &[&collection.path, &collection.name]) {
                continue;
            }
            let detail = match kind {
                CandidateKind::Topic => "topic",
                CandidateKind::Project => "project",
                CandidateKind::Paper | CandidateKind::Artifact => unreachable!(),
            };
            out.push(LibraryCandidate {
                kind,
                node: collection_node(&collection.path),
                candidate: Candidate {
                    label: collection.name.clone(),
                    insert_text: collection.path.clone(),
                    detail: detail.to_string(),
                },
            });
        }
    }

    if want(CandidateKind::Artifact) {
        for paper in &index.papers {
            let Ok(text) = std::fs::read_to_string(root.paper_markdown(&paper.citekey)) else {
                continue;
            };
            let parsed = parse_document(&text);
            for artifact in &parsed.artifacts {
                if !matches_query(query, &[artifact.id(), &artifact.heading]) {
                    continue;
                }
                out.push(LibraryCandidate {
                    kind: CandidateKind::Artifact,
                    node: artifact_node(&paper.citekey, artifact.id()),
                    candidate: Candidate {
                        label: artifact.heading.clone(),
                        insert_text: format!("{}#{}", paper.citekey, artifact.id()),
                        detail: format!(
                            "{} — {}",
                            artifact_kind_label(artifact.kind()),
                            paper.citekey
                        ),
                    },
                });
            }
        }
    }

    out.sort_by(|a, b| {
        a.candidate
            .label
            .cmp(&b.candidate.label)
            .then_with(|| a.kind.cmp(&b.kind))
            .then_with(|| a.node.cmp(&b.node))
    });
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
        assert_eq!(
            citation_candidates(&root, "").len(),
            2,
            "empty query lists everything"
        );
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
        EntityConfig::topic("htgrs", "HTGRs")
            .save(&root.topics_dir().join("htgrs"))
            .unwrap();
        EntityConfig::paper(
            CiteKey::parse("wang2018multiphysics").unwrap(),
            Access::Open,
        )
        .with_topics(["htgrs"])
        .save_paper(&root.paper_dir("wang2018multiphysics"))
        .unwrap();

        let index = KnowledgeIndex::rebuild(&root);
        let all = wiki_candidates(&index, "");
        assert!(all
            .iter()
            .any(|c| c.insert_text == "wang2018multiphysics" && c.detail == "paper"));
        assert!(all
            .iter()
            .any(|c| c.insert_text == "htgrs" && c.detail == "topic"));

        let filtered = wiki_candidates(&index, "wang");
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn artifact_candidates_matches_heading_and_id() {
        let (_dir, root) = make_root();
        EntityConfig::paper(
            CiteKey::parse("wang2018multiphysics").unwrap(),
            Access::Open,
        )
        .with_topics(["htgrs"])
        .save_paper(&root.paper_dir("wang2018multiphysics"))
        .unwrap();
        let mut session =
            crate::session::PaperSession::open(&root, "wang2018multiphysics").unwrap();
        session.append_block(
            "## Table 4.4\n\n```toml\n[kovan]\nid = \"table-4-4\"\nkind = \"digitised_table\"\ncreated = \"c\"\nmodified = \"m\"\n\n[source]\npage = 3\n```\n",
        );
        let index = ResearchRecordIndex::from_session(&session);

        assert_eq!(artifact_candidates(&index, "table").len(), 1);
        assert_eq!(artifact_candidates(&index, "4.4").len(), 1);
        assert!(artifact_candidates(&index, "nope").is_empty());
    }

    /// Builds a two-paper, one-topic, one-project fixture library on disk:
    /// `wang2018multiphysics` (topic `htgrs`) carries an artifact whose
    /// heading contains "Conduction"; `lee2020corrosion` (project
    /// `reactor-vessel`) carries an unrelated artifact whose heading
    /// contains "Corrosion Rate". Both papers' artifacts are saved to disk
    /// (via `save_document`), since [`library_candidates`] reads from disk,
    /// not from a live session buffer.
    fn make_library() -> (tempfile::TempDir, KovanRoot, KnowledgeIndex) {
        let (dir, root) = make_root();
        EntityConfig::topic("htgrs", "HTGRs")
            .save(&root.topics_dir().join("htgrs"))
            .unwrap();
        EntityConfig::project("reactor-vessel", "Reactor Vessel")
            .save(&root.projects_dir().join("reactor-vessel"))
            .unwrap();

        EntityConfig::paper(
            CiteKey::parse("wang2018multiphysics").unwrap(),
            Access::Open,
        )
        .with_topics(["htgrs"])
        .save_paper(&root.paper_dir("wang2018multiphysics"))
        .unwrap();
        let mut wang = crate::session::PaperSession::open(&root, "wang2018multiphysics").unwrap();
        wang.append_block(
            "## Conduction Coefficient\n\n```toml\n[kovan]\nid = \"conduction-coeff\"\nkind = \"note\"\ncreated = \"c\"\nmodified = \"m\"\n```\n",
        );
        wang.save_document().unwrap();

        EntityConfig::paper(CiteKey::parse("lee2020corrosion").unwrap(), Access::Open)
            .with_projects(["reactor-vessel"])
            .save_paper(&root.paper_dir("lee2020corrosion"))
            .unwrap();
        let mut lee = crate::session::PaperSession::open(&root, "lee2020corrosion").unwrap();
        lee.append_block(
            "## Corrosion Rate Table\n\n```toml\n[kovan]\nid = \"corrosion-rate\"\nkind = \"digitised_table\"\ncreated = \"c\"\nmodified = \"m\"\n```\n",
        );
        lee.save_document().unwrap();

        let index = KnowledgeIndex::rebuild(&root);
        (dir, root, index)
    }

    #[test]
    fn library_candidates_finds_artifacts_across_every_paper() {
        let (_dir, root, index) = make_library();

        // "conduction" only lives in wang2018multiphysics's artifact.
        let hits = library_candidates(&root, &index, "conduction", &[]);
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert_eq!(hits[0].kind, CandidateKind::Artifact);
        assert_eq!(hits[0].node, "artifact:wang2018multiphysics#conduction-coeff");
        assert_eq!(hits[0].candidate.insert_text, "wang2018multiphysics#conduction-coeff");

        // "corrosion" matches lee2020corrosion's artifact heading AND the
        // citation-style paper citekey itself is unrelated — but the
        // bibliography-derived citekey "lee2020corrosion" also contains
        // "corrosion", so both the paper and its artifact should hit.
        let hits = library_candidates(&root, &index, "corrosion", &[]);
        assert!(
            hits.iter()
                .any(|c| c.kind == CandidateKind::Paper && c.node == "paper:lee2020corrosion"),
            "{hits:?}"
        );
        assert!(
            hits.iter().any(|c| c.kind == CandidateKind::Artifact
                && c.node == "artifact:lee2020corrosion#corrosion-rate"),
            "{hits:?}"
        );
    }

    #[test]
    fn library_candidates_finds_papers_topics_and_projects() {
        let (_dir, root, index) = make_library();

        let hits = library_candidates(&root, &index, "htgrs", &[]);
        assert!(hits
            .iter()
            .any(|c| c.kind == CandidateKind::Topic && c.node == "collection:htgrs"));

        let hits = library_candidates(&root, &index, "reactor", &[]);
        assert!(hits.iter().any(
            |c| c.kind == CandidateKind::Project && c.node == "collection:reactor-vessel"
        ));

        let hits = library_candidates(&root, &index, "wang2018multiphysics", &[]);
        assert!(hits
            .iter()
            .any(|c| c.kind == CandidateKind::Paper && c.node == "paper:wang2018multiphysics"));
    }

    #[test]
    fn library_candidates_kind_filter_restricts_search() {
        let (_dir, root, index) = make_library();

        // Empty query would match every paper/topic/project/artifact if
        // every kind were searched; restrict to artifacts only.
        let hits = library_candidates(&root, &index, "", &[CandidateKind::Artifact]);
        assert!(!hits.is_empty());
        assert!(hits.iter().all(|c| c.kind == CandidateKind::Artifact));

        let hits = library_candidates(&root, &index, "", &[CandidateKind::Paper]);
        assert!(!hits.is_empty());
        assert!(hits.iter().all(|c| c.kind == CandidateKind::Paper));

        // A kind that has no matches for a narrow query returns empty, not
        // an error, even when other kinds would have hit.
        let hits = library_candidates(
            &root,
            &index,
            "conduction",
            &[CandidateKind::Paper, CandidateKind::Topic, CandidateKind::Project],
        );
        assert!(hits.is_empty(), "{hits:?}");
    }

    #[test]
    fn library_candidates_ordering_is_deterministic_and_stable() {
        let (_dir, root, index) = make_library();

        let run_a = library_candidates(&root, &index, "", &[]);
        let run_b = library_candidates(&root, &index, "", &[]);
        assert_eq!(run_a, run_b, "same (library, query) must rank identically");

        // Sorted by label first: verify the whole run is non-decreasing by
        // label, and that within equal labels the (kind, node) tie-break
        // holds — this fixture has no same-label collisions, so this
        // mainly guards against a future regression that reintroduces
        // HashMap-order-dependent output.
        for pair in run_a.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            let key_a = (&a.candidate.label, a.kind, &a.node);
            let key_b = (&b.candidate.label, b.kind, &b.node);
            assert!(key_a <= key_b, "{a:?} then {b:?} is out of order");
        }
    }

    #[test]
    fn library_candidates_empty_query_lists_everything_no_match_query_is_empty() {
        let (_dir, root, index) = make_library();

        let all = library_candidates(&root, &index, "", &[]);
        // 2 papers + 1 topic + 1 project + 2 artifacts = 6.
        assert_eq!(all.len(), 6, "{all:?}");

        assert!(library_candidates(&root, &index, "nonexistentxyz123", &[]).is_empty());
    }

    #[test]
    fn library_candidates_paper_matches_author_and_title_not_just_citekey() {
        let (_dir, root, index) = make_library();

        // "Yan" is the author's given name in wang2018multiphysics's bib
        // entry (`author = {Wang, Yan}`) and is not a substring of the
        // citekey itself — this only finds the paper through
        // `citation_candidates`'s richer author/title/year/doi search,
        // exactly like the `@`-popup already does.
        let hits = library_candidates(&root, &index, "Yan", &[CandidateKind::Paper]);
        assert!(
            hits.iter().any(|c| c.node == "paper:wang2018multiphysics"),
            "author-surname query found nothing: {hits:?}"
        );

        // "Kim" is the author surname in lee2020corrosion's bib entry
        // (`author = {Lee, Kim}`) and is likewise not a substring of that
        // citekey.
        let hits = library_candidates(&root, &index, "Kim", &[CandidateKind::Paper]);
        assert!(
            hits.iter().any(|c| c.node == "paper:lee2020corrosion"),
            "author-surname query found nothing: {hits:?}"
        );

        // "Study" is a title word from wang2018multiphysics's bib entry
        // (`title = {A Multiphysics Study}`) and is not a substring of the
        // citekey either.
        let hits = library_candidates(&root, &index, "Study", &[CandidateKind::Paper]);
        assert!(
            hits.iter().any(|c| c.node == "paper:wang2018multiphysics"),
            "title-word query found nothing: {hits:?}"
        );
    }

    #[test]
    fn library_candidates_paper_omits_bibliography_entries_with_no_library_paper() {
        let (_dir, root) = make_root();
        // Only add wang2018multiphysics to the library; lee2020corrosion
        // stays in the bibliography only (as if never ingested as a
        // paper), even though `make_root`'s bibliography lists both.
        EntityConfig::paper(
            CiteKey::parse("wang2018multiphysics").unwrap(),
            Access::Open,
        )
        .save_paper(&root.paper_dir("wang2018multiphysics"))
        .unwrap();
        let index = KnowledgeIndex::rebuild(&root);

        let hits = library_candidates(&root, &index, "corrosion", &[CandidateKind::Paper]);
        assert!(
            hits.is_empty(),
            "a bibliography-only entry with no matching library paper must not surface: {hits:?}"
        );

        // Sanity check: the in-library paper's own bib entry still matches.
        let hits = library_candidates(&root, &index, "multiphysics", &[CandidateKind::Paper]);
        assert!(hits.iter().any(|c| c.node == "paper:wang2018multiphysics"));
    }
}
