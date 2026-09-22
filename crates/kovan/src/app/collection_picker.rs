//! A fuzzy picker over the library's topics and projects (#272).
//!
//! Sorting a paper used to mean typing a slash-separated classification path
//! from memory into a comma-separated text field, and a typo did not fail —
//! `entity::ensure_classification_paths` obligingly *created* the mistyped
//! path, so `htgr/materials` quietly joined `htgrs/materials` in the tree.
//!
//! This is the same interaction as the literature finder #252 established
//! (`super::literature_list::LiteratureFinder`): type a few letters, take
//! ranked matches, Enter picks the best. That one is specialised to PDFs, so
//! this is its sibling rather than a reuse of it — but both rank through
//! [`crate::fuzzy::fuzzy_score`], so they order candidates identically.
//!
//! **Creating a new collection while sorting stays possible**, because it is
//! legitimate: the first paper on a subject is how a topic is born. It is
//! offered as a visibly separate "create" entry, never silently as if it were
//! a match, which is the distinction the old free-text field could not draw.

use crate::entity::EntityKind;
use crate::fuzzy::fuzzy_score;
use crate::index::{CollectionEntry, KnowledgeIndex};

/// Most suggestions shown at once. Matches the literature finder's own cap:
/// a list longer than this stops being scannable and starts being a scroll.
pub(super) const PICKER_RESULTS: usize = 8;

/// The collections of `kind` in `index` that match `query`, best first, at
/// most [`PICKER_RESULTS`].
///
/// Ranked on the **path** and on the **name**, keeping whichever scores
/// higher, so `materials` and `htgrs/mat` both find `htgrs/materials` — a
/// path-only match would miss the first, a name-only match the second.
/// Already-chosen paths are excluded: offering one again can only produce a
/// duplicate.
pub(super) fn rank<'a>(
    index: &'a KnowledgeIndex,
    kind: EntityKind,
    query: &str,
    chosen: &[String],
) -> Vec<&'a CollectionEntry> {
    let mut scored: Vec<(i32, usize, &CollectionEntry)> = index
        .collections
        .iter()
        .enumerate()
        .filter(|(_, c)| c.kind == kind && !chosen.iter().any(|p| p == &c.path))
        .filter_map(|(n, c)| {
            let by_path = fuzzy_score(query, &c.path);
            let by_name = fuzzy_score(query, &c.name);
            by_path.max(by_name).map(|s| (s, n, c))
        })
        .collect();
    // Best score first; ties keep the index's own order so the list does not
    // reshuffle as the user types.
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored
        .into_iter()
        .take(PICKER_RESULTS)
        .map(|(_, _, c)| c)
        .collect()
}

/// Whether `query` names a collection that does not exist yet, and so would
/// be **created**. An empty or whitespace-only query creates nothing, and
/// neither does one that exactly matches an existing path.
pub(super) fn would_create(index: &KnowledgeIndex, kind: EntityKind, query: &str) -> bool {
    let q = query.trim();
    !q.is_empty()
        && !index
            .collections
            .iter()
            .any(|c| c.kind == kind && c.path == q)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index(paths: &[(&str, EntityKind)]) -> KnowledgeIndex {
        KnowledgeIndex {
            collections: paths
                .iter()
                .map(|(p, k)| CollectionEntry {
                    path: (*p).to_string(),
                    kind: *k,
                    name: p.rsplit('/').next().unwrap().to_string(),
                })
                .collect(),
            ..Default::default()
        }
    }

    /// The picker ranks on the path *and* the leaf name, so both halves of a
    /// nested path find it.
    #[test]
    fn a_nested_path_is_found_by_either_half() {
        let idx = index(&[
            ("htgrs/materials", EntityKind::Topic),
            ("lwrs/thermal-hydraulics", EntityKind::Topic),
        ]);
        let by_leaf = rank(&idx, EntityKind::Topic, "materials", &[]);
        assert_eq!(by_leaf.first().map(|c| c.path.as_str()), Some("htgrs/materials"));
        let by_path = rank(&idx, EntityKind::Topic, "htgrs/mat", &[]);
        assert_eq!(by_path.first().map(|c| c.path.as_str()), Some("htgrs/materials"));
    }

    /// A topics picker never offers a project, and never re-offers something
    /// already chosen.
    #[test]
    fn kind_is_filtered_and_chosen_paths_are_excluded() {
        let idx = index(&[
            ("htgrs", EntityKind::Topic),
            ("htgr-sim", EntityKind::Project),
        ]);
        let topics = rank(&idx, EntityKind::Topic, "htgr", &[]);
        assert_eq!(topics.len(), 1, "the project is not a topic");
        assert_eq!(topics[0].path, "htgrs");

        let already = rank(&idx, EntityKind::Topic, "htgr", &["htgrs".to_string()]);
        assert!(already.is_empty(), "a chosen topic is not offered again");
    }

    /// Creating is offered only for a path that is genuinely new — never for
    /// an exact existing one, and never for an empty query.
    #[test]
    fn creation_is_offered_only_for_a_genuinely_new_path() {
        let idx = index(&[("htgrs/materials", EntityKind::Topic)]);
        assert!(would_create(&idx, EntityKind::Topic, "htgrs/fuel"));
        assert!(!would_create(&idx, EntityKind::Topic, "htgrs/materials"));
        assert!(!would_create(&idx, EntityKind::Topic, "   "));
        // A path that exists as a *project* is still new as a topic.
        assert!(would_create(&idx, EntityKind::Topic, "htgr-sim"));
    }
}
