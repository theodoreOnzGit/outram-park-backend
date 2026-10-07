// Part of the kovan Zotero port (GitHub #747, #751).
//
// No upstream logic is ported in this file: it maps Zotero's relations,
// collections and tags (models ported from Zotero, AGPL-3.0, Corporation
// for Digital Scholarship) onto kovan's own relation and concept schema.

//! Zotero relations, collections and tags -> kovan's `[[relation]]` and
//! mind-map concepts, as **data** for a later step to write.
//!
//! Nothing here writes a file or changes a kovan schema. The drafts use
//! the vocabulary of `crates/kovan/src/relation.rs` (`RelationRecord`'s
//! `source`/`target`/`kind`, `RelationKind::as_str`) and `graph.rs`
//! (`paper:<citekey>`, `collection:<path>`) spelled exactly, so a writer in
//! the `kovan` crate can pass them straight to `relation::add_connection`
//! and to a paper's `kovan.toml` `[classification]`. (kovan-semantics
//! cannot depend on `kovan`: `kovan` depends on it.)
//!
//! | Zotero | kovan | Lossy |
//! |---|---|---|
//! | top-level regular item, not trashed | a paper, `paper:<citekey>` (the [`kovan_common::KovanDocument`] from `ZoteroItem::to_kovan_document`, its `slug` the citekey) | a slug two items share is made unique with `-2`, `-3`, ... (recorded); a trashed item is skipped |
//! | `dc:relation` between two such items of this library | one [`KovanRelationDraft`] `{ source: "paper:a", target: "paper:b", kind: "related_to" }` per unordered pair (`relation::add_connection` writes it as a relation artifact in the mind map) | **direction**: Zotero stores each side separately; kovan gets one relation, source the lower citekey. A relation to or from a child item (note, attachment, annotation), a trashed item, an unknown key or another library is not mapped (recorded) |
//! | `owl:sameAs` (linked copy in another library) | nothing | recorded; the URI survives only in `KovanDocument::zotero_item` |
//! | `dc:replaces` (merge history) | nothing | recorded; survives in `zotero_item` |
//! | any other predicate (`mendeleyDB:...`, ...) | nothing | recorded; survives in `zotero_item` |
//! | collection, not trashed | a concept, `collection:<path>` under **topics** ([`KovanConceptDraft`]) | the name becomes a slug (lower-case ASCII, `-` for other runs; kovan's `classify::slugify`, ported) with `-2`... among siblings; the display name is kept in the draft; Zotero has no topic/project split, so every collection is a topic; collection relations are recorded and dropped |
//! | collection hierarchy (`parentCollection`) | path nesting, `parent/child` | exact; a parent key that names no collection makes the child top-level (recorded) |
//! | item in a collection | a topic in the paper's classification ([`KovanClassificationDraft`]) | an item in no live collection gets `unsorted` (kovan forbids an empty classification, entity.rs `UNSORTED`) |
//! | tags | already mapped by the document conversion: manual tags -> `tags`, automatic (`type: 1`) -> `keywords` | not concepts; nothing else is lost (Zotero's tag colours live in library settings, not item JSON) |

use std::collections::{BTreeMap, BTreeSet};

use kovan_common::zotero::ZoteroLibrary;

use super::relations::{
    parse_object_uri, LibraryUri, ObjectType, LINKED_OBJECT_PREDICATE, RELATED_ITEM_PREDICATE,
    REPLACED_ITEM_PREDICATE,
};

/// `RelationKind::RelatedTo.as_str()`: ~~in `crates/kovan/src/relation.rs`~~
/// **CORRECTED 2026-10-07** defined in `crates/kovan-common/src/artifact/relation.rs`
/// since GitHub #764 (re-exported by `kovan::relation`).
pub const KIND_RELATED_TO: &str = "related_to";
/// `entity::UNSORTED` in `crates/kovan/src/entity.rs`.
pub const UNSORTED: &str = "unsorted";

/// A relation for `relation::add_connection(root, source, target, kind)`;
/// field names as `RelationRecord`'s.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct KovanRelationDraft {
    /// `paper:<citekey>`.
    pub source: String,
    /// `paper:<citekey>`.
    pub target: String,
    /// A `RelationKind` wire name ([`KIND_RELATED_TO`]).
    pub kind: String,
}

/// A concept (topic) for a Zotero collection.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct KovanConceptDraft {
    /// The slash path, e.g. `reactors/htgr`.
    pub path: String,
    /// `collection:<path>`.
    pub node: String,
    /// The collection's name, verbatim.
    pub name: String,
    /// The Zotero collection key.
    pub zotero_key: Option<String>,
}

/// A paper's topics.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct KovanClassificationDraft {
    /// The citekey (the `KovanDocument` slug, made unique).
    pub citekey: String,
    /// `paper:<citekey>`.
    pub paper: String,
    /// The Zotero item key.
    pub zotero_key: String,
    /// Topic paths, sorted.
    pub topics: Vec<String>,
}

/// Why something did not map, or mapped with a change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LossKind {
    /// Two items' slugs collided; the later one was suffixed.
    CitekeyCollision,
    /// An item with no key (no URI, no node).
    ItemWithoutKey,
    /// `owl:sameAs`.
    LinkedObject,
    /// `dc:replaces`.
    ReplacedItem,
    /// A predicate kovan has no counterpart for.
    OtherPredicate,
    /// `dc:relation` whose target is not a live top-level item here.
    UnresolvedRelatedItem,
    /// A relation held by a child item (note, attachment, annotation).
    ChildItemRelation,
    /// A relation held by a collection.
    CollectionRelation,
    /// Two sibling collections' slugs collided; the later one was suffixed.
    ConceptCollision,
    /// A collection whose parent key names no collection.
    MissingParentCollection,
    /// A collection named nothing slug-able (`collection-<key>` used).
    EmptyCollectionSlug,
    /// A collection in the trash (skipped, with its memberships).
    TrashedCollection,
}

/// One recorded loss.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MappingLoss {
    /// What kind.
    pub kind: LossKind,
    /// The Zotero key of the item or collection concerned.
    pub subject: String,
    /// The detail (a URI, a slug, a predicate).
    pub detail: String,
}

/// The whole mapping of a library, every list sorted.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KovanMapping {
    /// Zotero item key -> citekey, for every mapped paper.
    pub citekeys: BTreeMap<String, String>,
    /// One per live collection.
    pub concepts: Vec<KovanConceptDraft>,
    /// One per mapped paper.
    pub classifications: Vec<KovanClassificationDraft>,
    /// One per unordered related pair.
    pub relations: Vec<KovanRelationDraft>,
    /// Everything that did not map exactly.
    pub losses: Vec<MappingLoss>,
    /// Keys of trashed top-level items (skipped).
    pub skipped_trashed: Vec<String>,
}

/// kovan's `classify::slugify` (crates/kovan/src/classify.rs:145), ported
/// because it is `pub(crate)` there: lower-case ASCII alphanumerics, every
/// other run one `-`, no leading/trailing `-`.
pub fn slugify(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_was_dash = true;
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            out.push('-');
            last_was_dash = true;
        }
    }
    if out.ends_with('-') {
        out.pop();
    }
    out
}

fn unique(base: &str, taken: &BTreeSet<String>) -> String {
    if !taken.contains(base) {
        return base.to_owned();
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|c| !taken.contains(c))
        .unwrap_or_default()
}

/// Map `library` (whose URIs are `this`'s) onto kovan drafts.
pub fn map_library(library: &ZoteroLibrary, this: &LibraryUri) -> KovanMapping {
    let mut m = KovanMapping::default();

    // Papers.
    let mut taken: BTreeSet<String> = BTreeSet::new();
    for it in &library.items {
        if !it.item_type.is_regular() || it.parent_item.is_some() {
            continue;
        }
        let Some(key) = it.key.clone() else {
            m.losses.push(MappingLoss {
                kind: LossKind::ItemWithoutKey,
                subject: String::new(),
                detail: it.fields.get("title").cloned().unwrap_or_default(),
            });
            continue;
        };
        if it.deleted == Some(true) {
            m.skipped_trashed.push(key);
            continue;
        }
        let slug = it.to_kovan_document().slug;
        let base = if slug.is_empty() {
            format!("zotero-{}", key.to_ascii_lowercase())
        } else {
            slug
        };
        let citekey = unique(&base, &taken);
        if citekey != base {
            m.losses.push(MappingLoss {
                kind: LossKind::CitekeyCollision,
                subject: key.clone(),
                detail: format!("{base} -> {citekey}"),
            });
        }
        taken.insert(citekey.clone());
        m.citekeys.insert(key, citekey);
    }

    // Concepts: resolve each live collection's path from the root down.
    let live: BTreeMap<String, usize> = library
        .collections
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            let k = c.key.clone()?;
            if c.deleted == Some(true) {
                None
            } else {
                Some((k, i))
            }
        })
        .collect();
    for c in &library.collections {
        if c.deleted == Some(true) {
            m.losses.push(MappingLoss {
                kind: LossKind::TrashedCollection,
                subject: c.key.clone().unwrap_or_default(),
                detail: c.name.clone(),
            });
        }
    }
    let mut paths: BTreeMap<String, String> = BTreeMap::new();
    // Process parents before children: repeat until no progress.
    let mut siblings: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut pending: Vec<String> = live.keys().cloned().collect();
    while !pending.is_empty() {
        let before = pending.len();
        let mut next = Vec::new();
        for key in pending {
            let c = &library.collections[live[&key]];
            let parent_path = match c.parent_collection.as_deref() {
                None => Some(String::new()),
                Some(p) if !live.contains_key(p) => {
                    m.losses.push(MappingLoss {
                        kind: LossKind::MissingParentCollection,
                        subject: key.clone(),
                        detail: p.to_owned(),
                    });
                    Some(String::new())
                }
                Some(p) => paths.get(p).cloned(),
            };
            let Some(parent_path) = parent_path else {
                next.push(key);
                continue;
            };
            let mut base = slugify(&c.name);
            if base.is_empty() {
                base = format!("collection-{}", key.to_ascii_lowercase());
                m.losses.push(MappingLoss {
                    kind: LossKind::EmptyCollectionSlug,
                    subject: key.clone(),
                    detail: c.name.clone(),
                });
            }
            let sib = siblings.entry(parent_path.clone()).or_default();
            let seg = unique(&base, sib);
            if seg != base {
                m.losses.push(MappingLoss {
                    kind: LossKind::ConceptCollision,
                    subject: key.clone(),
                    detail: format!("{base} -> {seg}"),
                });
            }
            sib.insert(seg.clone());
            let path = if parent_path.is_empty() {
                seg
            } else {
                format!("{parent_path}/{seg}")
            };
            paths.insert(key.clone(), path.clone());
            m.concepts.push(KovanConceptDraft {
                node: format!("collection:{path}"),
                path,
                name: c.name.clone(),
                zotero_key: Some(key.clone()),
            });
            for (p, objs) in &c.relations {
                for o in objs {
                    m.losses.push(MappingLoss {
                        kind: LossKind::CollectionRelation,
                        subject: key.clone(),
                        detail: format!("{p} {o}"),
                    });
                }
            }
        }
        if next.len() == before {
            // A parent cycle: map the rest as top-level next round.
            for key in &next {
                m.losses.push(MappingLoss {
                    kind: LossKind::MissingParentCollection,
                    subject: key.clone(),
                    detail: "cycle".into(),
                });
            }
            let mut rest = next;
            rest.sort();
            for key in rest {
                let c = &library.collections[live[&key]];
                let base = slugify(&c.name);
                let base = if base.is_empty() {
                    format!("collection-{}", key.to_ascii_lowercase())
                } else {
                    base
                };
                let sib = siblings.entry(String::new()).or_default();
                let seg = unique(&base, sib);
                sib.insert(seg.clone());
                paths.insert(key.clone(), seg.clone());
                m.concepts.push(KovanConceptDraft {
                    node: format!("collection:{seg}"),
                    path: seg,
                    name: c.name.clone(),
                    zotero_key: Some(key),
                });
            }
            break;
        }
        pending = next;
    }
    m.concepts.sort();

    // Classifications.
    for it in &library.items {
        let Some(key) = it.key.as_deref() else {
            continue;
        };
        let Some(citekey) = m.citekeys.get(key) else {
            continue;
        };
        let mut topics: Vec<String> = it
            .collections
            .iter()
            .filter_map(|c| paths.get(c).cloned())
            .collect();
        topics.sort();
        topics.dedup();
        if topics.is_empty() {
            topics.push(UNSORTED.to_owned());
        }
        m.classifications.push(KovanClassificationDraft {
            citekey: citekey.clone(),
            paper: format!("paper:{citekey}"),
            zotero_key: key.to_owned(),
            topics,
        });
    }
    m.classifications.sort();

    // Relations.
    let mut pairs: BTreeSet<(String, String)> = BTreeSet::new();
    for it in &library.items {
        let key = it.key.clone().unwrap_or_default();
        let source = m.citekeys.get(&key).cloned();
        for (p, objs) in &it.relations {
            for o in objs {
                let loss = |kind: LossKind| MappingLoss {
                    kind,
                    subject: key.clone(),
                    detail: format!("{p} {o}"),
                };
                let top_level = it.item_type.is_regular() && it.parent_item.is_none();
                if top_level && source.is_none() {
                    // A trashed or keyless paper: skipped, recorded above.
                    continue;
                }
                let Some(src) = source.as_ref() else {
                    m.losses.push(loss(LossKind::ChildItemRelation));
                    continue;
                };
                match p.as_str() {
                    RELATED_ITEM_PREDICATE => {
                        let target =
                            parse_object_uri(o)
                                .filter(|u| this.resolves(u))
                                .and_then(|u| match u.object {
                                    Some((ObjectType::Item, k)) => m.citekeys.get(&k).cloned(),
                                    _ => None,
                                });
                        match target {
                            Some(t) if t != *src => {
                                let (a, b) = if *src < t {
                                    (src.clone(), t)
                                } else {
                                    (t, src.clone())
                                };
                                pairs.insert((a, b));
                            }
                            _ => m.losses.push(loss(LossKind::UnresolvedRelatedItem)),
                        }
                    }
                    LINKED_OBJECT_PREDICATE => m.losses.push(loss(LossKind::LinkedObject)),
                    REPLACED_ITEM_PREDICATE => m.losses.push(loss(LossKind::ReplacedItem)),
                    _ => m.losses.push(loss(LossKind::OtherPredicate)),
                }
            }
        }
    }
    m.relations = pairs
        .into_iter()
        .map(|(a, b)| KovanRelationDraft {
            source: format!("paper:{a}"),
            target: format!("paper:{b}"),
            kind: KIND_RELATED_TO.to_owned(),
        })
        .collect();
    m.losses.sort();
    m.skipped_trashed.sort();
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use kovan_common::zotero::{Field, ItemType, ZoteroCollection, ZoteroItem};

    fn paper(key: &str, citekey: &str) -> ZoteroItem {
        let mut i = ZoteroItem::new(ItemType::JournalArticle);
        i.key = Some(key.into());
        i.set_field(Field::CitationKey, citekey);
        i
    }

    #[test]
    fn maps_relations_collections_and_losses() {
        let lib_uri = LibraryUri::LocalUser("LOCAL123".into());
        let mut a = paper("AAAAAAAA", "smith2020");
        let mut b = paper("BBBBBBBB", "jones2021");
        let c = paper("CCCCCCCC", "smith2020");
        a.relations.insert(
            RELATED_ITEM_PREDICATE.into(),
            vec![lib_uri.item_uri("BBBBBBBB")],
        );
        b.relations.insert(
            RELATED_ITEM_PREDICATE.into(),
            vec![lib_uri.item_uri("AAAAAAAA"), lib_uri.item_uri("ZZZZZZZZ")],
        );
        b.relations.insert(
            LINKED_OBJECT_PREDICATE.into(),
            vec!["http://zotero.org/groups/1/items/GGGGGGGG".into()],
        );
        a.collections = vec!["K2".into()];
        let mut parent = ZoteroCollection::new("Reactors & Fuel");
        parent.key = Some("K1".into());
        let mut child = ZoteroCollection::new("HTGR");
        child.key = Some("K2".into());
        child.parent_collection = Some("K1".into());
        let lib = ZoteroLibrary {
            collections: vec![child, parent],
            items: vec![a, b, c],
            ..Default::default()
        };
        let m = map_library(&lib, &lib_uri);
        assert_eq!(m.citekeys["CCCCCCCC"], "smith2020-2");
        assert_eq!(
            m.relations,
            vec![KovanRelationDraft {
                source: "paper:jones2021".into(),
                target: "paper:smith2020".into(),
                kind: "related_to".into()
            }]
        );
        let paths: Vec<&str> = m.concepts.iter().map(|c| c.path.as_str()).collect();
        assert_eq!(paths, vec!["reactors-fuel", "reactors-fuel/htgr"]);
        let a_cls = m
            .classifications
            .iter()
            .find(|c| c.zotero_key == "AAAAAAAA")
            .unwrap();
        assert_eq!(a_cls.topics, vec!["reactors-fuel/htgr".to_string()]);
        let b_cls = m
            .classifications
            .iter()
            .find(|c| c.zotero_key == "BBBBBBBB")
            .unwrap();
        assert_eq!(b_cls.topics, vec![UNSORTED.to_string()]);
        let kinds: Vec<LossKind> = m.losses.iter().map(|l| l.kind).collect();
        assert_eq!(
            kinds,
            vec![
                LossKind::CitekeyCollision,
                LossKind::LinkedObject,
                LossKind::UnresolvedRelatedItem
            ]
        );
    }

    #[test]
    fn slugify_matches_kovans_examples() {
        // classify.rs:1126 `slugify_matches_the_issues_own_examples`.
        assert_eq!(
            slugify("Coupled neutronics methodology"),
            "coupled-neutronics-methodology"
        );
        assert_eq!(
            slugify("Table 4.4 — Core component materials"),
            "table-4-4-core-component-materials"
        );
        assert_eq!(slugify("   ---   "), "");
    }
}
