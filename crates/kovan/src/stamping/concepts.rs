//! **Concept links on a review, and the concept areas they give rung 5**
//! (GitHub #770, #810; #740 decision 11; #739 decisions 18–22; #809).
//!
//! # Linking a review to a concept (#740 decision 11)
//!
//! "Linking a review to the knowledge map uses the fuzzy finder by default
//! … type to search Formula artifacts, concepts, papers and code. The 192
//! level-4 `[[implementation]]` seeds act only as **hints**: when a
//! function's module has a seed, its concept … is ranked first in the
//! finder, marked 'suggested'." What is built here is the **concept** part
//! of that finder:
//!
//! - the candidates are the standard corpus's concept tree, levels 1–3
//!   ([`kovan_literature::concept_tree`], the same tree desktop kovan's
//!   standard map draws), with the id `concept:<tree path>` (the form
//!   `[[reviewer.qualification]] area` already uses), plus the open
//!   workspace's own topics and projects ([`crate::index::KnowledgeIndex`]),
//!   with the id `collection:<path>` (the graph's collection id);
//! - ranking is [`kovan_common::fuzzy::fuzzy_score`] on the path and on the
//!   title, keeping the better (as `crate::collection_picker` ranks), with
//!   seeded concepts first ([`seeded_concepts`]);
//! - the link written is an anchor `[[relation]] kind = "implements"`,
//!   `target = "<concept id>"` on the review entry ([`implements`]), signed
//!   with the stamp (relations are part of the signed bytes).
//!
//! **Not built (reported on #770):** searching Formula artifacts, papers and
//! code in the same finder. #739 decision 21 makes the canonical link
//! `implements` → a **Formula artifact**; none exists anywhere in the
//! workspace or the standard corpus today (checked 2026-10-10: no
//! `kind = "formula"` artifact in any tracked Markdown file or in
//! `reactor-literature`), so a concept is linked directly, which #739
//! decision 18 also names ("`implements` → `concept:…`").
//!
//! # The implemented formula (#739 decision 21)
//!
//! "In review mode, the sidebar shows the implemented equation rendered
//! above the review." [`find_formulas`] looks for Formula artifacts filed
//! under a linked concept (the artifact's own `[classification] topics`, or
//! its paper folder's `kovan.toml` `[classification] topics`, at the concept
//! or under it) and for a Formula artifact an `implements` target names
//! directly (`artifact:<id>`), in the workspace's Markdown (the open corpus
//! decision 22 puts project formulas in). Nothing is invented: with none
//! found the view says "no formula recorded for this concept".
//!
//! # Concept areas (rung 5, #809 decision 1)
//!
//! Rung 5 needs the independent reviewer "qualified in every concept area
//! of the function". **Reading taken (the literal one; a decision for the
//! maintainer):** a function's concept areas are the concept-tree nodes its
//! reviews' `implements` relations name, **as linked** (`concept:<path>`),
//! over **every** review entry of the function (the union: more areas can
//! only make the qualification rule stricter). A qualification covers an
//! area when its own area is that node or an ancestor
//! ([`kovan_common::review::root::area_covers`]), so a qualification in an
//! IAEA issue (level 1) covers every concept under it. Not resolved:
//! `implements` → a Formula artifact (decision 21: concept ← paper ←
//! Formula), since no Formula artifact exists yet; such a link gives no
//! area, so the function stays below rung 5 ("no known concept area"),
//! never a silent upgrade.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::OnceLock;

use kovan_common::artifact::relation::{RelationKind, RelationRecord};
use kovan_common::fuzzy::fuzzy_score;
use kovan_common::review::engine::ConceptAreas;
use kovan_literature::concept_tree;

use super::Workspace;

/// The id prefix of a standard concept-tree node.
pub const CONCEPT: &str = "concept:";
/// The id prefix of a workspace topic or project.
pub const COLLECTION: &str = "collection:";
/// The id prefix of an artifact (a Formula, #739 decision 21).
pub const ARTIFACT: &str = "artifact:";
/// Most candidates the finder shows at once (the literature finder's cap).
pub const FINDER_RESULTS: usize = 8;

/// One concept the finder offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConceptChoice {
    /// `concept:<path>` or `collection:<path>`: the relation's target.
    pub id: String,
    /// The path within its tree.
    pub path: String,
    pub title: String,
    /// 1 for an IAEA issue, 2 a review category, 3+ a concept; 0 for a
    /// workspace collection.
    pub level: usize,
    /// A level-4 `[[implementation]]` seed names the function's module
    /// (#740 decision 11: a hint, ranked first, never a link by itself).
    pub suggested: bool,
}

/// `concept:<path>`.
pub fn concept_id(path: &str) -> String {
    format!("{CONCEPT}{}", path.trim_matches('/'))
}

/// The tree path of a concept or collection id (prefix dropped).
pub fn concept_path(id: &str) -> &str {
    id.strip_prefix(CONCEPT)
        .or_else(|| id.strip_prefix(COLLECTION))
        .unwrap_or(id)
        .trim_matches('/')
}

/// Whether `id` names a concept (standard or workspace collection).
pub fn is_concept(id: &str) -> bool {
    id.starts_with(CONCEPT) || id.starts_with(COLLECTION)
}

/// Every standard concept-tree node, levels 1–3, in tree order.
pub fn standard_concepts() -> Vec<ConceptChoice> {
    concept_tree()
        .nodes()
        .iter()
        .map(|n| ConceptChoice {
            id: concept_id(&n.path),
            path: n.path.clone(),
            title: n.title.clone(),
            level: n.level,
            suggested: false,
        })
        .collect()
}

/// The open workspace's own topics and projects (`collection:<path>`), from
/// its `topics/` and `projects/` folders; none when `root` is not a kovan
/// root. Reads the disk: call it off the UI thread.
pub fn workspace_concepts(root: &Path) -> Vec<ConceptChoice> {
    let Ok(r) = crate::root::KovanRoot::open(root) else {
        return Vec::new();
    };
    crate::index::KnowledgeIndex::rebuild(&r)
        .collections
        .into_iter()
        .map(|c| ConceptChoice {
            id: format!("{COLLECTION}{}", c.path),
            title: c.name,
            path: c.path,
            level: 0,
            suggested: false,
        })
        .collect()
}

/// One `[[implementation]]` seed: only the fields the hint needs.
#[derive(Debug, Clone, serde::Deserialize)]
struct Seed {
    concept: String,
    #[serde(rename = "crate")]
    krate: String,
    module: String,
}

#[derive(Debug, Default, serde::Deserialize)]
struct Seeds {
    #[serde(default)]
    implementation: Vec<Seed>,
}

fn seeds() -> &'static [Seed] {
    static SEEDS: OnceLock<Vec<Seed>> = OnceLock::new();
    SEEDS.get_or_init(|| {
        toml::from_str::<Seeds>(kovan_literature::concept_tree::PROPOSALS_TOML)
            .map(|s| s.implementation)
            .unwrap_or_default()
    })
}

/// The concept ids a level-4 seed suggests for a function of crate `krate`
/// in `file` (workspace-relative): seeds of that crate whose `module` is a
/// path segment of the file below `src/` (`src/<module>.rs` or
/// `src/<module>/…`).
pub fn seeded_concepts(krate: &str, file: &str) -> BTreeSet<String> {
    let below = file.split_once("/src/").map_or(file, |(_, b)| b);
    let segments: BTreeSet<&str> = below
        .split('/')
        .map(|s| s.strip_suffix(".rs").unwrap_or(s))
        .collect();
    seeds()
        .iter()
        .filter(|s| s.krate == krate && segments.contains(s.module.as_str()))
        .map(|s| concept_id(&s.concept))
        .collect()
}

/// Every candidate for a function of `krate` in `file`: the standard tree
/// (seeded ones marked suggested), then `extra` (the workspace's own).
pub fn candidates(krate: &str, file: &str, extra: Vec<ConceptChoice>) -> Vec<ConceptChoice> {
    let seeded = seeded_concepts(krate, file);
    let mut all = standard_concepts();
    for c in &mut all {
        c.suggested = seeded.contains(&c.id);
    }
    all.extend(extra);
    all
}

/// The candidates matching `query`, best first, at most [`FINDER_RESULTS`]:
/// suggested ones before the rest, then by the better of the path and title
/// scores, then in candidate order (so the list does not reshuffle while
/// typing). An empty query lists the suggested ones first.
pub fn rank<'a>(candidates: &'a [ConceptChoice], query: &str) -> Vec<&'a ConceptChoice> {
    let mut scored: Vec<(bool, i32, usize, &ConceptChoice)> = candidates
        .iter()
        .enumerate()
        .filter_map(|(n, c)| {
            let s = fuzzy_score(query, &c.path).max(fuzzy_score(query, &c.title))?;
            Some((c.suggested, s, n, c))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
    scored
        .into_iter()
        .take(FINDER_RESULTS)
        .map(|(_, _, _, c)| c)
        .collect()
}

/// The `[[relation]]` anchor a concept link writes.
pub fn implements(id: &str) -> RelationRecord {
    RelationRecord::new("", id, RelationKind::Implements)
}

/// The concept ids among `relations`' `implements` targets, in order.
pub fn linked_concepts(relations: &[RelationRecord]) -> Vec<String> {
    relations
        .iter()
        .filter(|r| r.kind == RelationKind::Implements && is_concept(&r.target))
        .map(|r| r.target.clone())
        .collect()
}

/// A concept id's title: the tree node's, else the path's last segment.
pub fn concept_title(id: &str) -> String {
    let path = concept_path(id);
    id.strip_prefix(CONCEPT)
        .and_then(|_| concept_tree().node(path))
        .map(|n| n.title.clone())
        .unwrap_or_else(|| path.rsplit('/').next().unwrap_or(path).to_string())
}

/// Function id -> its concept areas (module doc): every review entry's
/// `implements` concept targets, unioned per function.
pub fn concept_areas(ws: &Workspace) -> ConceptAreas {
    let mut out = ConceptAreas::new();
    for m in ws.reviews.values() {
        for r in m.doc.reviews() {
            let linked = linked_concepts(&r.relations);
            if !linked.is_empty() {
                out.entry(r.function_id())
                    .or_default()
                    .extend(linked.into_iter().map(|c| normalise(&c)));
            }
        }
    }
    out
}

/// A concept id with its path trimmed of stray slashes.
fn normalise(id: &str) -> String {
    let path = concept_path(id);
    if id.starts_with(COLLECTION) {
        format!("{COLLECTION}{path}")
    } else {
        concept_id(path)
    }
}

/// One Formula artifact found for a link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormulaHit {
    /// The Markdown file, workspace-relative.
    pub file: String,
    pub id: String,
    pub heading: String,
    /// The artifact's body (GFM math, as written).
    pub body: String,
}

/// One Markdown document to search: its path, text, and the topics its
/// folder's `kovan.toml` files it under (a paper's classification).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormulaSource {
    pub file: String,
    pub text: String,
    pub folder_topics: Vec<String>,
}

/// Whether topic `t` is at concept path `path` or under it.
fn under(t: &str, path: &str) -> bool {
    let t = t.trim_matches('/');
    !path.is_empty() && (t == path || t.starts_with(&format!("{path}/")))
}

/// The Formula artifacts in `docs` for the link `target` (module doc):
/// filed at or under a concept, or named by an `artifact:` target. Pure.
pub fn formulas_in(target: &str, docs: &[FormulaSource]) -> Vec<FormulaHit> {
    let named = target
        .strip_prefix(ARTIFACT)
        .map(|a| a.rsplit('#').next().unwrap_or(a));
    let path = is_concept(target).then(|| concept_path(target));
    let mut out = Vec::new();
    for d in docs {
        let parsed = crate::artifact::parse_document(&d.text);
        for a in &parsed.artifacts {
            if a.kind() != crate::artifact::ArtifactKind::Formula {
                continue;
            }
            let hit = match (named, path) {
                (Some(id), _) => a.id() == id,
                (None, Some(p)) => a
                    .toml
                    .classification
                    .topics
                    .iter()
                    .chain(d.folder_topics.iter())
                    .any(|t| under(t, p)),
                _ => false,
            };
            if hit {
                out.push(FormulaHit {
                    file: d.file.clone(),
                    id: a.id().to_string(),
                    heading: a.heading.clone(),
                    body: a.body.trim().to_string(),
                });
            }
        }
    }
    out
}

/// The `[classification] topics` of the `kovan.toml` beside `md`, if any.
fn folder_topics(md: &Path) -> Vec<String> {
    let Some(text) = md
        .parent()
        .and_then(|d| std::fs::read_to_string(d.join(super::KOVAN_TOML)).ok())
    else {
        return Vec::new();
    };
    toml::from_str::<toml::Table>(&text)
        .ok()
        .and_then(|t| {
            t.get("classification")?.get("topics")?.as_array().map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
        })
        .unwrap_or_default()
}

/// Link target -> the Formula artifacts found for it in the workspace's
/// Markdown (every `.md` file but `review.md`, `.gitignore` honoured).
/// Reads the disk: call it off the UI thread. Empty `targets` reads
/// nothing.
pub fn find_formulas(root: &Path, targets: &[String]) -> BTreeMap<String, Vec<FormulaHit>> {
    if targets.is_empty() {
        return BTreeMap::new();
    }
    let docs: Vec<FormulaSource> = kovan_discovery::discover(root, &["md"])
        .into_iter()
        .filter(|p| p.file_name().and_then(|f| f.to_str()) != Some(super::REVIEW_MD))
        .filter_map(|p| {
            let text = std::fs::read_to_string(&p).ok()?;
            // Only a document holding a formula can match: skip the rest
            // before parsing.
            text.contains("formula").then(|| FormulaSource {
                file: p
                    .strip_prefix(root)
                    .map(|r| r.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_default(),
                folder_topics: folder_topics(&p),
                text,
            })
        })
        .collect();
    targets
        .iter()
        .map(|t| (t.clone(), formulas_in(t, &docs)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: the finder's candidates are the standard tree (levels
    /// 1–3, ids `concept:<path>`) plus workspace collections; ranking finds a
    /// concept by a word of its title and by part of its path, puts a
    /// seeded concept first even with an empty query, and caps the list.
    /// The seed hint matches a crate's module by a path segment below
    /// `src/` (farrer-park's `assembly` seed, from the compiled-in
    /// proposals file).
    ///
    /// Result (2026-10-10): passes.
    #[test]
    fn finder_ranks_seeded_first_and_matches_title_or_path() {
        let all = standard_concepts();
        assert!(all.len() > 19, "levels 1-3");
        assert!(all
            .iter()
            .all(|c| c.id.starts_with(CONCEPT) && c.level >= 1));
        let seeded = seeded_concepts("farrer-park", "crates/farrer-park/src/assembly/mod.rs");
        assert!(!seeded.is_empty(), "the assembly seed");
        assert!(seeded_concepts("farrer-park", "crates/farrer-park/src/other.rs").is_empty());
        assert!(seeded_concepts("tampines", "crates/tampines/src/assembly.rs").is_empty());

        let extra = vec![ConceptChoice {
            id: "collection:htgrs/materials".into(),
            path: "htgrs/materials".into(),
            title: "materials".into(),
            level: 0,
            suggested: false,
        }];
        let c = candidates("farrer-park", "crates/farrer-park/src/assembly.rs", extra);
        let first = rank(&c, "");
        assert!(first[0].suggested, "{first:?}");
        assert!(first.len() <= FINDER_RESULTS);
        let n = &all[0];
        let word = n.title.split_whitespace().last().unwrap();
        assert!(rank(&c, word).iter().any(|x| x.id == n.id), "by title");
        assert!(rank(&c, &n.path).iter().any(|x| x.id == n.id), "by path");
        assert_eq!(rank(&c, "htgrs/mat")[0].id, "collection:htgrs/materials");
        assert!(rank(&c, "zzzzqqqq-nothing").is_empty());
        assert_eq!(concept_title(&n.id), n.title);
        assert_eq!(concept_title("collection:a/b"), "b");
    }

    /// Methodology: a concept link is an `implements` anchor; only concept
    /// and collection targets count as linked concepts; a formula is found
    /// by its own classification, by its paper folder's topics (at the
    /// concept or under it) and by an `artifact:` target naming it, and a
    /// concept with none gives an empty list (the view then says "no formula
    /// recorded for this concept").
    ///
    /// Result (2026-10-10): passes.
    #[test]
    fn links_and_formulas() {
        let rels = vec![
            implements("concept:a/b"),
            RelationRecord::new("", "artifact:x#eq-1", RelationKind::Implements),
            RelationRecord::new("", "concept:c", RelationKind::PartOf),
        ];
        assert_eq!(linked_concepts(&rels), vec!["concept:a/b".to_string()]);
        assert_eq!(normalise("concept:/a/b/"), "concept:a/b");

        let note = "# Energy balance\n\n```toml\n[kovan]\nid = \"eq-1\"\nkind = \"formula\"\ncreated = \"c\"\nmodified = \"m\"\n\n[classification]\ntopics = [\"a/b/c\"]\n```\n\n$$ q = m c_p \\Delta T $$\n";
        let other = "# Plain\n\n```toml\n[kovan]\nid = \"eq-2\"\nkind = \"formula\"\ncreated = \"c\"\nmodified = \"m\"\n```\n\n$$ x = 1 $$\n";
        let docs = vec![
            FormulaSource {
                file: "p/one.md".into(),
                text: note.into(),
                folder_topics: vec![],
            },
            FormulaSource {
                file: "p/two.md".into(),
                text: other.into(),
                folder_topics: vec!["z".into()],
            },
        ];
        let hits = formulas_in("concept:a/b", &docs);
        assert_eq!(hits.len(), 1);
        assert_eq!(
            (hits[0].id.as_str(), hits[0].file.as_str()),
            ("eq-1", "p/one.md")
        );
        assert!(hits[0].body.contains("c_p"));
        assert!(
            formulas_in("concept:a/bb", &docs).is_empty(),
            "a sibling, not a child"
        );
        assert_eq!(
            formulas_in("collection:z", &docs)[0].id,
            "eq-2",
            "by folder topics"
        );
        assert_eq!(formulas_in("artifact:x#eq-2", &docs)[0].id, "eq-2");
        assert!(formulas_in("concept:nothing", &docs).is_empty());

        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("p")).unwrap();
        std::fs::write(d.path().join("p/one.md"), note).unwrap();
        std::fs::write(d.path().join("p/review.md"), note).unwrap();
        let found = find_formulas(d.path(), &["concept:a".into(), "concept:q".into()]);
        assert_eq!(found["concept:a"].len(), 1, "review.md is not searched");
        assert!(found["concept:q"].is_empty());
        assert!(find_formulas(d.path(), &[]).is_empty());
        assert!(workspace_concepts(d.path()).is_empty(), "not a kovan root");
    }
}
