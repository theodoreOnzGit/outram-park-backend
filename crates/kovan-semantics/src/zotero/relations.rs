// Part of the kovan Zotero port (GitHub #747, #751).
//
// Upstream: Zotero, https://github.com/zotero/zotero (commit 9cbba8c4d281),
// `chrome/content/zotero/xpcom/data/relations.js` (`Zotero.Relations`),
// `xpcom/data/dataObject.js` (relation methods, 315-620), `xpcom/data/item.js`
// (`addRelatedItem` 1530, `_getRelatedItems` 6400, `_eraseData` 5613),
// `xpcom/uri.js` (`Zotero.URI`).
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0. See this crate's NOTICE, "Upstream: Zotero".

//! Zotero's relations model over a [`ZoteroLibrary`] in memory.
//!
//! A relation is a (predicate, object URI) pair stored on its **subject**
//! (an item or a collection): `ZoteroItem::relations` /
//! `ZoteroCollection::relations`, predicate -> URIs. Zotero uses three
//! predicates (relations.js:29-31):
//!
//! | Predicate | Constant | Meaning in Zotero |
//! |---|---|---|
//! | `dc:relation` | [`RELATED_ITEM_PREDICATE`] | "Related" items. Stored on each side separately: `addRelatedItem` writes one direction only (item.js:1552); the item pane adds both. Same library only. |
//! | `owl:sameAs` | [`LINKED_OBJECT_PREDICATE`] | The same item/collection copied into another library (`_addLinkedObject`, dataObject.js:582): stored once, on the user-library side when one side is the user library. |
//! | `dc:replaces` | [`REPLACED_ITEM_PREDICATE`] | Merge tracking: the master points at each item merged into it (mergeItems.mjs `moveRelations`). |
//!
//! Upstream also keeps a registry (`_subjectsByPredicateIDAndObject`) so it
//! can ask "which subjects point at this URI"; here that is a scan of the
//! library ([`subjects_by_object`]), so it is always current.
//!
//! Object URIs are `http://zotero.org/<library path>/items/<KEY>` (or
//! `/collections/<KEY>`), the library path being `users/<id>`,
//! `users/local/<localUserKey>` for a user who has not synced, or
//! `groups/<id>` ([`LibraryUri`], uri.js:91-121).

use std::collections::BTreeMap;

use kovan_common::zotero::{ZoteroItem, ZoteroLibrary};

/// `Zotero.Relations.relatedItemPredicate`.
pub const RELATED_ITEM_PREDICATE: &str = "dc:relation";
/// `Zotero.Relations.linkedObjectPredicate`.
pub const LINKED_OBJECT_PREDICATE: &str = "owl:sameAs";
/// `Zotero.Relations.replacedItemPredicate`.
pub const REPLACED_ITEM_PREDICATE: &str = "dc:replaces";
/// `Zotero.URI.defaultPrefix`.
pub const URI_PREFIX: &str = "http://zotero.org/";

/// `Zotero.Relations._namespaces` (relations.js:33): prefix -> namespace.
pub const NAMESPACES: [(&str, &str); 4] = [
    ("dc", "http://purl.org/dc/elements/1.1/"),
    ("owl", "http://www.w3.org/2002/07/owl#"),
    ("mendeleyDB", "http://zotero.org/namespaces/mendeleyDB#"),
    ("zotero", "http://zotero.org/namespaces/zotero"),
];

/// Why a relation operation failed, where upstream throws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelationError {
    /// "Predicate not provided" (dataObject.js:373).
    EmptyPredicate,
    /// "Object not provided" (dataObject.js:376).
    EmptyObject,
    /// "Invalid relation predicate" (dataObject.js:446): not `^[a-z]+:[a-z]+$`
    /// (case-insensitive).
    InvalidPredicate(String),
    /// "Invalid prefix" (relations.js:290).
    InvalidPrefix(String),
    /// "Could not parse object URI" (uri.js:386).
    UnparseableUri(String),
    /// "Can't relate item to itself" (item.js:1539; upstream logs and
    /// returns false rather than throwing).
    SelfRelation,
    /// "Cannot relate item to an item in a different library" (item.js:1549).
    DifferentLibrary,
    /// The item or collection has no key, so it has no URI.
    MissingKey,
}

impl std::fmt::Display for RelationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyPredicate => write!(f, "Predicate not provided"),
            Self::EmptyObject => write!(f, "Object not provided"),
            Self::InvalidPredicate(p) => write!(f, "Invalid relation predicate '{p}'"),
            Self::InvalidPrefix(p) => write!(f, "Invalid prefix '{p}'"),
            Self::UnparseableUri(u) => write!(f, "Could not parse object URI {u}"),
            Self::SelfRelation => write!(f, "Can't relate item to itself"),
            Self::DifferentLibrary => {
                write!(f, "Cannot relate item to an item in a different library")
            }
            Self::MissingKey => write!(f, "object has no key"),
        }
    }
}

impl std::error::Error for RelationError {}

/// Which library a URI names: the path part of `Zotero.URI.getLibraryURI`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LibraryUri {
    /// A synced user library: `users/<userID>`.
    User(u64),
    /// A user library that has never synced: `users/local/<localUserKey>`.
    LocalUser(String),
    /// A group library: `groups/<groupID>`.
    Group(u64),
}

impl LibraryUri {
    /// `Zotero.URI.getLibraryPath` (uri.js:91).
    pub fn path(&self) -> String {
        match self {
            Self::User(id) => format!("users/{id}"),
            Self::LocalUser(k) => format!("users/local/{k}"),
            Self::Group(id) => format!("groups/{id}"),
        }
    }

    /// `Zotero.URI.getLibraryURI`.
    pub fn uri(&self) -> String {
        format!("{URI_PREFIX}{}", self.path())
    }

    /// `Zotero.URI.getItemURI`: `<library>/items/<key>`.
    pub fn item_uri(&self, key: &str) -> String {
        format!("{}/items/{key}", self.uri())
    }

    /// `Zotero.URI.getCollectionURI`: `<library>/collections/<key>`.
    pub fn collection_uri(&self, key: &str) -> String {
        format!("{}/collections/{key}", self.uri())
    }

    /// Whether a parsed URI's library is this one, as `_getURIObjectLibrary`
    /// resolves it (uri.js:420): **any** `users/...` URI without a feed part
    /// resolves to the (one) user library, whatever the user id; a group
    /// URI resolves by group id.
    pub fn resolves(&self, uri: &ZoteroUri) -> bool {
        match (self, &uri.library) {
            (
                Self::User(_) | Self::LocalUser(_),
                LibraryUri::User(_) | LibraryUri::LocalUser(_),
            ) => uri.library_sub.is_none() || uri.library_sub.as_deref() == Some("publications"),
            (Self::Group(a), LibraryUri::Group(b)) => a == b,
            _ => false,
        }
    }
}

/// The object type a URI names (`uriParts[5]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObjectType {
    /// `items`.
    Item,
    /// `collections`.
    Collection,
}

/// A parsed Zotero object URI (`uriPartsRe`, uri.js:38-42).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoteroUri {
    /// The library.
    pub library: LibraryUri,
    /// `publications` or `feeds/<id>` (group 4), when present.
    pub library_sub: Option<String>,
    /// `items`/`collections` and the key, when the URI names an object
    /// rather than a library.
    pub object: Option<(ObjectType, String)>,
}

fn word(s: &str) -> usize {
    s.bytes()
        .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
        .count()
}

/// Parse a URI as `Zotero.URI._getURIObject` does (trailing slashes
/// dropped, then `uriPartsRe` matched from the start; text after the match
/// is ignored, as an unanchored-at-end regex does). `None` where upstream
/// throws "Could not parse object URI".
///
/// A `users/<id>` id must be digits to be a [`LibraryUri::User`]; upstream's
/// `\w+` also accepts other word characters, which are kept as
/// `LocalUser` only when written `users/local/...`; any other non-numeric
/// user or group id is reported as unparseable here.
pub fn parse_object_uri(uri: &str) -> Option<ZoteroUri> {
    let uri = uri.trim_end_matches('/');
    let rest = uri.strip_prefix(URI_PREFIX)?;
    let (kind, rest) = match rest.strip_prefix("users/") {
        Some(r) => ("users", r),
        None => ("groups", rest.strip_prefix("groups/")?),
    };
    // `(local/)?` applies to both kinds in the regex; for a group it is
    // matched and then ignored (`Zotero.Groups.get(uriParts[3])`).
    let (local, rest) = match rest.strip_prefix("local/") {
        Some(r) => (true, r),
        None => (false, rest),
    };
    let n = word(rest);
    if n == 0 {
        return None;
    }
    let id = &rest[..n];
    let mut rest = &rest[n..];
    let library = match (kind, local) {
        ("users", true) => LibraryUri::LocalUser(id.to_owned()),
        ("users", false) => LibraryUri::User(id.parse().ok()?),
        _ => LibraryUri::Group(id.parse().ok()?),
    };
    let mut library_sub = None;
    if let Some(r) = rest.strip_prefix("/publications") {
        library_sub = Some("publications".to_owned());
        rest = r;
    } else if let Some(r) = rest.strip_prefix("/feeds/") {
        let n = word(r);
        if n > 0 {
            library_sub = Some(format!("feeds/{}", &r[..n]));
            rest = &r[n..];
        }
    }
    let mut object = None;
    for (prefix, t) in [
        ("/items/", ObjectType::Item),
        ("/collections/", ObjectType::Collection),
    ] {
        if let Some(r) = rest.strip_prefix(prefix) {
            let n = word(r);
            if n > 0 {
                object = Some((t, r[..n].to_owned()));
            }
        }
    }
    Some(ZoteroUri {
        library,
        library_sub,
        object,
    })
}

/// `Zotero.Relations._getPrefixAndValue` for a prefixed predicate
/// (relations.js:285): `prefix:value` with a known prefix. Upstream's
/// fallback for a full namespace URI refers to an undefined variable
/// (`namespaces`), so it throws; that is [`RelationError::InvalidPrefix`]
/// here too.
pub fn prefix_and_value(predicate: &str) -> Result<(String, String), RelationError> {
    let mut parts = predicate.split(':');
    let prefix = parts.next().unwrap_or("");
    let value = parts.next().unwrap_or("");
    if !prefix.is_empty() && !value.is_empty() {
        if !NAMESPACES.iter().any(|(p, _)| *p == prefix) {
            return Err(RelationError::InvalidPrefix(prefix.to_owned()));
        }
        return Ok((prefix.to_owned(), value.to_owned()));
    }
    Err(RelationError::InvalidPrefix(predicate.to_owned()))
}

/// The relations map of an item or collection, with upstream's
/// `DataObject` relation methods. Relations are a predicate -> URIs map, as
/// in Zotero's JSON (`getRelations`).
pub type Relations = BTreeMap<String, Vec<String>>;

/// `DataObject#addRelation` (dataObject.js:370): `Ok(false)` when the pair
/// already exists.
pub fn add_relation(
    rels: &mut Relations,
    predicate: &str,
    object: &str,
) -> Result<bool, RelationError> {
    if predicate.is_empty() {
        return Err(RelationError::EmptyPredicate);
    }
    if object.is_empty() {
        return Err(RelationError::EmptyObject);
    }
    let v = rels.entry(predicate.to_owned()).or_default();
    if v.iter().any(|o| o == object) {
        return Ok(false);
    }
    v.push(object.to_owned());
    Ok(true)
}

/// `DataObject#removeRelation` (dataObject.js:409): whether it existed.
pub fn remove_relation(rels: &mut Relations, predicate: &str, object: &str) -> bool {
    let Some(v) = rels.get_mut(predicate) else {
        return false;
    };
    let Some(i) = v.iter().position(|o| o == object) else {
        return false;
    };
    v.remove(i);
    if v.is_empty() {
        rels.remove(predicate);
    }
    true
}

/// `DataObject#hasRelation` (dataObject.js:396).
pub fn has_relation(rels: &Relations, predicate: &str, object: &str) -> bool {
    rels.get(predicate)
        .is_some_and(|v| v.iter().any(|o| o == object))
}

/// `DataObject#getRelationsByPredicate` (dataObject.js:346).
pub fn relations_by_predicate<'a>(rels: &'a Relations, predicate: &str) -> &'a [String] {
    rels.get(predicate).map(Vec::as_slice).unwrap_or(&[])
}

/// Whether `predicate` passes `setRelations`' check (dataObject.js:446):
/// `^[a-z]+:[a-z]+$`, case-insensitive.
pub fn is_valid_predicate(predicate: &str) -> bool {
    let Some((a, b)) = predicate.split_once(':') else {
        return false;
    };
    let letters = |s: &str| !s.is_empty() && s.bytes().all(|c| c.is_ascii_alphabetic());
    letters(a) && letters(b)
}

/// `DataObject#setRelations` (dataObject.js:436): replace the relations
/// after validating every predicate (duplicate URIs under a predicate are
/// kept, as `flattenRelations` keeps them). `Ok(false)` when unchanged.
pub fn set_relations(rels: &mut Relations, new: Relations) -> Result<bool, RelationError> {
    for p in new.keys() {
        if !is_valid_predicate(p) {
            return Err(RelationError::InvalidPredicate(p.clone()));
        }
    }
    let flat = |r: &Relations| {
        let mut v: Vec<(String, String)> = r
            .iter()
            .flat_map(|(p, os)| os.iter().map(move |o| (p.clone(), o.clone())))
            .collect();
        v.sort();
        v
    };
    let new: Relations = new.into_iter().filter(|(_, v)| !v.is_empty()).collect();
    if flat(rels) == flat(&new) {
        return Ok(false);
    }
    *rels = new;
    Ok(true)
}

/// `Zotero.Item#addRelatedItem` (item.js:1534): add `dc:relation` -> `other`
/// on `item` only (one direction, as upstream). `item` is in `library`,
/// `other` in `other_library`; they must be the same library.
pub fn add_related_item(
    library: &LibraryUri,
    item: &mut ZoteroItem,
    other_library: &LibraryUri,
    other: &ZoteroItem,
) -> Result<bool, RelationError> {
    let key = other.key.as_deref().ok_or(RelationError::MissingKey)?;
    if item.key.as_deref() == Some(key) && library == other_library {
        return Err(RelationError::SelfRelation);
    }
    if library != other_library {
        return Err(RelationError::DifferentLibrary);
    }
    add_relation(
        &mut item.relations,
        RELATED_ITEM_PREDICATE,
        &library.item_uri(key),
    )
}

/// `Zotero.Item#removeRelatedItem` (item.js:1560).
pub fn remove_related_item(library: &LibraryUri, item: &mut ZoteroItem, other_key: &str) -> bool {
    remove_relation(
        &mut item.relations,
        RELATED_ITEM_PREDICATE,
        &library.item_uri(other_key),
    )
}

/// `Zotero.Item#relatedItems` (`_getRelatedItems`, item.js:6400): the keys
/// of the item URIs under `dc:relation` (any library, as
/// `getURIItemLibraryKey` does not check the library exists here).
pub fn related_item_keys(item: &ZoteroItem) -> Vec<String> {
    relations_by_predicate(&item.relations, RELATED_ITEM_PREDICATE)
        .iter()
        .filter_map(|u| parse_object_uri(u))
        .filter_map(|u| match u.object {
            Some((ObjectType::Item, k)) => Some(k),
            _ => None,
        })
        .collect()
}

/// A subject found by [`subjects_by_object`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Subject {
    /// Index into `items` or `collections`.
    pub index: usize,
    /// The predicate pointing at the object.
    pub predicate: String,
}

/// `Zotero.Relations.getByObject(objectType, object)` (relations.js:149):
/// every (subject, predicate) whose relations include `object`, by subject
/// index then predicate. Upstream orders by predicate id then insertion.
pub fn subjects_by_object(
    library: &ZoteroLibrary,
    object_type: ObjectType,
    object: &str,
) -> Vec<Subject> {
    let rels: Vec<&Relations> = match object_type {
        ObjectType::Item => library.items.iter().map(|i| &i.relations).collect(),
        ObjectType::Collection => library.collections.iter().map(|c| &c.relations).collect(),
    };
    let mut out = Vec::new();
    for (index, r) in rels.into_iter().enumerate() {
        for (p, os) in r {
            if os.iter().any(|o| o == object) {
                out.push(Subject {
                    index,
                    predicate: p.clone(),
                });
            }
        }
    }
    out
}

/// `Zotero.Relations.getByPredicateAndObject` (relations.js:122): the
/// subjects with `predicate` -> `object`. The predicate must be prefixed
/// (`dc:relation`), as upstream normalises it with `_getPrefixAndValue`.
pub fn subjects_by_predicate_and_object(
    library: &ZoteroLibrary,
    object_type: ObjectType,
    predicate: &str,
    object: &str,
) -> Result<Vec<usize>, RelationError> {
    let (p, v) = prefix_and_value(predicate)?;
    let predicate = format!("{p}:{v}");
    Ok(subjects_by_object(library, object_type, object)
        .into_iter()
        .filter(|s| s.predicate == predicate)
        .map(|s| s.index)
        .collect())
}

/// `Zotero.Relations.updateUser(fromUserID, toUserID)` (relations.js:197):
/// `REPLACE(object, 'zotero.org/users/<from>/', 'zotero.org/users/<to>/')`
/// on every item and collection relation. `from` is e.g. `"1"` or
/// `"local/abcdefgh"` (upstream's default for a null `fromUserID`).
pub fn update_user(library: &mut ZoteroLibrary, from: &str, to: &str) {
    let a = format!("zotero.org/users/{from}/");
    let b = format!("zotero.org/users/{to}/");
    let fix = |r: &mut Relations| {
        for os in r.values_mut() {
            for o in os.iter_mut() {
                if o.contains(&a) {
                    *o = o.replace(&a, &b);
                }
            }
        }
    };
    for i in &mut library.items {
        fix(&mut i.relations);
    }
    for c in &mut library.collections {
        fix(&mut c.relations);
    }
}

/// `Zotero.Relations.purge()` (relations.js:244): remove every relation
/// (other than `dc:replaces`) whose object is a Zotero URI of the subject's
/// own type that does not resolve to an object in this library. Returns
/// how many were removed.
pub fn purge(library: &mut ZoteroLibrary, this: &LibraryUri) -> Result<usize, RelationError> {
    let item_keys: Vec<String> = library.items.iter().filter_map(|i| i.key.clone()).collect();
    let coll_keys: Vec<String> = library
        .collections
        .iter()
        .filter_map(|c| c.key.clone())
        .collect();
    let dangling = |uri: &str, t: ObjectType, keys: &[String]| -> Result<bool, RelationError> {
        let marker = match t {
            ObjectType::Item => "/items/",
            ObjectType::Collection => "/collections/",
        };
        if !uri.contains(URI_PREFIX) || !uri.contains(marker) {
            return Ok(false);
        }
        let p =
            parse_object_uri(uri).ok_or_else(|| RelationError::UnparseableUri(uri.to_owned()))?;
        let resolves =
            this.resolves(&p) && matches!(&p.object, Some((ot, k)) if *ot == t && keys.contains(k));
        Ok(!resolves)
    };
    let mut removed = 0;
    let mut purge_one =
        |r: &mut Relations, t: ObjectType, keys: &[String]| -> Result<(), RelationError> {
            let mut drop: Vec<(String, String)> = Vec::new();
            for (p, os) in r.iter() {
                if p == REPLACED_ITEM_PREDICATE {
                    continue;
                }
                for o in os {
                    if dangling(o, t, keys)? {
                        drop.push((p.clone(), o.clone()));
                    }
                }
            }
            for (p, o) in drop {
                remove_relation(r, &p, &o);
                removed += 1;
            }
            Ok(())
        };
    for i in &mut library.items {
        purge_one(&mut i.relations, ObjectType::Item, &item_keys)?;
    }
    for c in &mut library.collections {
        purge_one(&mut c.relations, ObjectType::Collection, &coll_keys)?;
    }
    Ok(removed)
}

/// The `dc:relation` cleanup of `Zotero.Item#_eraseData` (item.js:5613):
/// before an item is erased, every item with a related-item relation to it
/// drops that relation. Returns the indices changed.
pub fn remove_relations_to_erased_item(
    library: &mut ZoteroLibrary,
    this: &LibraryUri,
    key: &str,
) -> Vec<usize> {
    let uri = this.item_uri(key);
    let mut changed = Vec::new();
    for (i, it) in library.items.iter_mut().enumerate() {
        if remove_relation(&mut it.relations, RELATED_ITEM_PREDICATE, &uri) {
            changed.push(i);
        }
    }
    changed
}

/// One library, as [`linked_object`] needs it.
#[derive(Debug, Clone)]
pub struct LibraryRef {
    /// The library's URI.
    pub uri: LibraryUri,
    /// The library.
    pub library: ZoteroLibrary,
}

/// `DataObject#_getLinkedObject(libraryID, bidirectional)` for items
/// (dataObject.js:501): the item in `target` linked to `item` (in
/// `source`) by `owl:sameAs`, skipping trashed items. With
/// `bidirectional`, also an item in `target` whose `owl:sameAs` points at
/// this one. The index into `target.library.items`.
pub fn linked_item(
    source: &LibraryRef,
    item: &ZoteroItem,
    target: &LibraryRef,
    bidirectional: bool,
) -> Result<Option<usize>, RelationError> {
    let prefix = format!("{}/items/", target.uri.uri());
    for u in relations_by_predicate(&item.relations, LINKED_OBJECT_PREDICATE) {
        if !u.starts_with(&prefix) {
            continue;
        }
        let key = &u[prefix.len()..];
        if let Some(i) = target
            .library
            .items
            .iter()
            .position(|t| t.key.as_deref() == Some(key))
        {
            if target.library.items[i].deleted == Some(true) {
                continue;
            }
            return Ok(Some(i));
        }
    }
    if bidirectional {
        let key = item.key.as_deref().ok_or(RelationError::MissingKey)?;
        let this_uri = source.uri.item_uri(key);
        for i in subjects_by_predicate_and_object(
            &target.library,
            ObjectType::Item,
            LINKED_OBJECT_PREDICATE,
            &this_uri,
        )? {
            if target.library.items[i].deleted == Some(true) {
                continue;
            }
            return Ok(Some(i));
        }
    }
    Ok(None)
}

/// `DataObject#_addLinkedObject` (dataObject.js:582): link `a` (in library
/// `a_lib`) and `b` (in `b_lib`, a different library) with `owl:sameAs`,
/// stored on `a` when `a` is in the user library or `b` is not, else on `b`.
/// `Ok(false)` when `a` already links to `b`.
pub fn add_linked_item(
    a_lib: &LibraryUri,
    a: &mut ZoteroItem,
    b_lib: &LibraryUri,
    b: &mut ZoteroItem,
) -> Result<bool, RelationError> {
    if a_lib == b_lib {
        return Err(RelationError::DifferentLibrary);
    }
    let a_uri = a_lib.item_uri(a.key.as_deref().ok_or(RelationError::MissingKey)?);
    let b_uri = b_lib.item_uri(b.key.as_deref().ok_or(RelationError::MissingKey)?);
    if has_relation(&a.relations, LINKED_OBJECT_PREDICATE, &b_uri) {
        return Ok(false);
    }
    let is_user = |l: &LibraryUri| matches!(l, LibraryUri::User(_) | LibraryUri::LocalUser(_));
    if is_user(a_lib) || !is_user(b_lib) {
        add_relation(&mut a.relations, LINKED_OBJECT_PREDICATE, &b_uri)?;
    } else {
        add_relation(&mut b.relations, LINKED_OBJECT_PREDICATE, &a_uri)?;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_round_trip() {
        let l = LibraryUri::LocalUser("v3aG8nQf".into());
        let u = l.item_uri("ABCD2345");
        assert_eq!(u, "http://zotero.org/users/local/v3aG8nQf/items/ABCD2345");
        let p = parse_object_uri(&u).unwrap();
        assert_eq!(p.library, l);
        assert_eq!(p.object, Some((ObjectType::Item, "ABCD2345".into())));
        let g = parse_object_uri("http://zotero.org/groups/12/collections/XYZ/").unwrap();
        assert_eq!(g.library, LibraryUri::Group(12));
        assert_eq!(g.object, Some((ObjectType::Collection, "XYZ".into())));
        assert!(parse_object_uri("https://example.com/x").is_none());
    }

    #[test]
    fn predicates() {
        assert!(is_valid_predicate("dc:relation"));
        assert!(is_valid_predicate("OWL:sameAs"));
        assert!(!is_valid_predicate("0"));
        assert!(!is_valid_predicate("dc:rel-ation"));
        assert!(prefix_and_value("owl:sameAs").is_ok());
        assert!(prefix_and_value("foo:bar").is_err());
    }

    #[test]
    fn add_remove_has() {
        let mut r = Relations::new();
        assert_eq!(add_relation(&mut r, "owl:sameAs", "u"), Ok(true));
        assert_eq!(add_relation(&mut r, "owl:sameAs", "u"), Ok(false));
        assert!(has_relation(&r, "owl:sameAs", "u"));
        assert!(remove_relation(&mut r, "owl:sameAs", "u"));
        assert!(r.is_empty());
        assert_eq!(
            add_relation(&mut r, "", "u"),
            Err(RelationError::EmptyPredicate)
        );
    }
}
