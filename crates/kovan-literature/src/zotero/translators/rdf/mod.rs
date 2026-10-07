// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "RDF.js" (translatorID
//   5e3ad958-ac79-463d-812b-a86a9235c28f, lastUpdated 2026-08-12 16:51:38):
//   `detectImport` :46-54, `n` :58-82, `getFirstResults` :88-111,
//   `handleCreators` :115-162, `processCollection` :165-199,
//   `processSeeAlso` :201-210, `processTags` :212-233, `getNodeByType`
//   :236-256, `isPart` :262-275, `getNodes` :1449-1481, `doImport`/
//   `startImport`/`importNext` :1483-1569; `detectType` and `importItem`
//   are in the submodules.
// Copyright (c) 2011 Center for History and New Media, George Mason
//   University, Fairfax, Virginia, USA (Simon Kornblith).
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The RDF import translator: Zotero RDF, Dublin Core, PRISM, BIBO,
//! schema.org, Open Graph, eprints and vCard terms, read into items, notes,
//! attachments and collections.
//!
//! Values move between the RDF store and the item the way they do in
//! JavaScript; [`V`] is a JavaScript value as this translator handles it. An
//! RDF term assigned to an item property becomes its `toString()` (what
//! `_itemDone` turns it into), except under the property names `_itemDone`
//! lets hold objects, where it is kept as the JSON `JSON.stringify` gives.

mod detect_type;
mod import_item;

use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::rdf::{Node, RdfSandbox, RdfValue, Res, Term, RDF_NS};
use crate::zotero::framework::utilities::clean_author;
use crate::zotero::framework::{
    CollectionChild, ImportContext, TranslateError, TranslatorCollection, TranslatorCreator,
    TranslatorItem,
};
use serde_json::{json, Value};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "5e3ad958-ac79-463d-812b-a86a9235c28f",
    label: "RDF",
    creator: "Simon Kornblith",
    target: "rdf",
    min_version: "2.1.9",
    priority: 100,
    translator_type: translator_type::IMPORT,
    config_options: &[
        ("async", HeaderValue::Bool(true)),
        ("dataMode", HeaderValue::Str("rdf/xml")),
    ],
    display_options: &[],
    hidden_prefs: &[],
    last_updated: "2026-08-12 16:51:38",
};

/// `n` (:58-82).
pub(crate) mod ns {
    pub const BIB: &str = "http://purl.org/net/biblio#";
    pub const BIBO: &str = "http://purl.org/ontology/bibo/";
    pub const DC1_0: &str = "http://purl.org/dc/elements/1.0/";
    pub const DC: &str = "http://purl.org/dc/elements/1.1/";
    pub const DCTERMS: &str = "http://purl.org/dc/terms/";
    pub const PRISM: &str = "http://prismstandard.org/namespaces/1.2/basic/";
    pub const PRISM2_0: &str = "http://prismstandard.org/namespaces/basic/2.0/";
    pub const PRISM2_1: &str = "http://prismstandard.org/namespaces/basic/2.1/";
    pub const FOAF: &str = "http://xmlns.com/foaf/0.1/";
    pub const VCARD: &str = "http://nwalsh.com/rdf/vCard#";
    pub const VCARD2: &str = "http://www.w3.org/2006/vcard/ns#";
    pub const LINK: &str = "http://purl.org/rss/1.0/modules/link/";
    pub const Z: &str = "http://www.zotero.org/namespaces/export#";
    pub const EPRINTS: &str = "http://purl.org/eprint/terms/";
    pub const OG: &str = "http://ogp.me/ns#";
    pub const ARTICLE: &str = "http://ogp.me/ns/article#";
    pub const BOOK: &str = "http://ogp.me/ns/book#";
    pub const MUSIC: &str = "http://ogp.me/ns/music#";
    pub const VIDEO: &str = "http://ogp.me/ns/video#";
    pub const SO: &str = "http://schema.org/";
    pub const CODEMETA: &str = "https://codemeta.github.io/terms/";
    /// `n.song` is not declared: `n.song + "duration"` is this.
    pub const SONG_UNDEFINED: &str = "undefined";
}

use ns::*;

/// A JavaScript value as this translator handles it.
#[derive(Debug, Clone)]
pub(crate) enum V {
    /// `undefined`.
    Undef,
    /// `false` (what `getNodeByType` returns when it finds nothing).
    False,
    /// A string.
    Str(String),
    /// An RDF term object.
    Node(Node),
    /// An array (what `getFirstResults` returns without `onlyOneString`).
    List(Vec<V>),
}

impl V {
    /// JavaScript truthiness.
    pub(crate) fn truthy(&self) -> bool {
        match self {
            V::Undef | V::False => false,
            V::Str(s) => !s.is_empty(),
            V::Node(_) | V::List(_) => true,
        }
    }

    pub(crate) fn from_rdf(v: RdfValue) -> V {
        match v {
            RdfValue::Str(s) => V::Str(s),
            RdfValue::Node(n) => V::Node(n),
        }
    }

    /// The array's elements (`[]` for anything else).
    pub(crate) fn list(&self) -> &[V] {
        match self {
            V::List(l) => l,
            _ => &[],
        }
    }

    /// The string, when a string.
    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            V::Str(s) => Some(s),
            _ => None,
        }
    }
}

fn err(m: impl Into<String>) -> TranslateError {
    TranslateError::Translator(m.into())
}

/// The property names `_itemDone` lets hold objects (translate.js:97-106).
const ALLOWED_OBJECTS: [&str; 7] = [
    "complete",
    "attachments",
    "creators",
    "tags",
    "notes",
    "relations",
    "seeAlso",
];

/// The running translation: `Zotero.RDF` and the item saver.
pub(crate) struct Rdf<'a> {
    pub(crate) z: RdfSandbox,
    pub(crate) ctx: &'a mut ImportContext,
}

impl Rdf<'_> {
    /// A [`V`] as a resource argument.
    pub(crate) fn res(&self, v: &V) -> Res {
        match v {
            V::Undef => Res::Undefined,
            V::False => Res::False,
            V::Str(s) => Res::Uri(s.clone()),
            V::Node(n) => Res::Node(n.clone()),
            // An array passed as a resource: never reached by these paths.
            V::List(_) => Res::Undefined,
        }
    }

    /// `toString()` of a value.
    pub(crate) fn to_string(&self, v: &V) -> String {
        match v {
            V::Undef => "undefined".to_owned(),
            V::False => "false".to_owned(),
            V::Str(s) => s.clone(),
            V::Node(n) => self.z.store.term_to_string(&n.term),
            V::List(l) => l
                .iter()
                .map(|x| match x {
                    V::Undef => String::new(),
                    other => self.to_string(other),
                })
                .collect::<Vec<_>>()
                .join(","),
        }
    }

    /// `JSON.stringify` of a term object (its own enumerable properties).
    fn node_json(&self, n: &Node) -> Value {
        match &n.term {
            Term::Symbol(u) => json!({"uri": u, "value": u}),
            Term::BlankNode(id) => json!({"id": id, "value": id.to_string()}),
            Term::Literal {
                value,
                lang,
                datatype,
            } => {
                let mut m = serde_json::Map::new();
                m.insert("value".into(), value.clone().into());
                if let Some(l) = lang {
                    m.insert("lang".into(), l.clone().into());
                }
                if let Some(d) = datatype {
                    m.insert("datatype".into(), json!({"uri": d, "value": d}));
                }
                Value::Object(m)
            }
            Term::Collection(id) => {
                let els: Vec<Value> = self
                    .z
                    .store
                    .collection_elements(*id)
                    .iter()
                    .map(|e| self.node_json(e))
                    .collect();
                json!({"id": id, "elements": els, "closed": true})
            }
        }
    }

    /// A value as JSON the way `JSON.stringify` writes it (`undefined` as
    /// `null`; callers drop it).
    pub(crate) fn json_of(&self, v: &V) -> Value {
        match v {
            V::Undef => Value::Null,
            V::False => Value::Bool(false),
            V::Str(s) => Value::String(s.clone()),
            V::Node(n) => self.node_json(n),
            V::List(l) => Value::Array(l.iter().map(|x| self.json_of(x)).collect()),
        }
    }

    /// `item[key] = v` on an item that goes through `_itemDone`: a term
    /// becomes its string (what `_itemDone` makes of an object under any
    /// other name), arrays keep their elements' strings (so that truthiness
    /// and `_itemDone`'s `toString()` still apply), `undefined` is kept as
    /// `null` so that the property keeps its place in the insertion order.
    pub(crate) fn set(&self, item: &mut TranslatorItem, key: &str, v: &V) {
        let value = if ALLOWED_OBJECTS.contains(&key) {
            self.json_of(v)
        } else {
            match v {
                V::Node(_) => Value::String(self.to_string(v)),
                V::List(l) => Value::Array(
                    l.iter()
                        .map(|x| match x {
                            V::Undef => Value::Null,
                            other => Value::String(self.to_string(other)),
                        })
                        .collect(),
                ),
                other => self.json_of(other),
            }
        };
        item.set(key, value);
    }

    /// `getResourceURI(v)`.
    pub(crate) fn resource_uri(&mut self, v: &V) -> V {
        match v {
            V::Str(s) => V::Str(s.clone()),
            V::Node(n) => V::from_rdf(self.z.get_resource_uri(&RdfValue::Node(n.clone()))),
            // `resource.uri` of undefined/false: upstream throws; these paths
            // never pass one.
            other => V::Str(self.to_string(other)),
        }
    }

    /// `getResourceURI(v)` as a string.
    pub(crate) fn resource_uri_string(&mut self, v: &V) -> String {
        let u = self.resource_uri(v);
        self.to_string(&u)
    }

    /// `Zotero.RDF.getTargets(node, prop)` as a [`V`] (`False` when none).
    pub(crate) fn targets(&mut self, node: &V, prop: &str) -> V {
        let r = self.res(node);
        match self.z.get_targets(r, prop) {
            Some(t) => V::List(t.into_iter().map(V::from_rdf).collect()),
            None => V::False,
        }
    }

    /// The URI of a node's first `rdf:type`, if it has one.
    pub(crate) fn first_type(&mut self, node: &V) -> Option<V> {
        match self.targets(node, &format!("{RDF_NS}type")) {
            V::List(l) => {
                let t = l[0].clone();
                Some(self.resource_uri(&t))
            }
            _ => None,
        }
    }

    /// `getFirstResults(nodes, properties, onlyOneString)` (:88-111).
    pub(crate) fn first_results(&mut self, nodes: &V, properties: &[String], only_one: bool) -> V {
        // `if (!nodes.length) nodes = [nodes]`: a non-empty array or string
        // is iterated (a string by its characters).
        let list: Vec<V> = match nodes {
            V::List(l) if !l.is_empty() => l.clone(),
            V::Str(s) if !s.is_empty() => s.chars().map(|c| V::Str(c.to_string())).collect(),
            other => vec![other.clone()],
        };
        for node in &list {
            for p in properties {
                let r = self.targets(node, p);
                if let V::List(result) = r {
                    if only_one {
                        return match &result[0] {
                            V::Str(s) => V::Str(s.clone()),
                            other => {
                                let o = other.clone();
                                self.resource_uri(&o)
                            }
                        };
                    }
                    return V::List(result);
                }
            }
        }
        V::Undef
    }

    /// `getNodeByType(nodes, type)` (:236-256).
    pub(crate) fn node_by_type(&mut self, nodes: &V, types: &[String]) -> V {
        if !nodes.truthy() {
            return V::False;
        }
        for node in nodes.list().to_vec() {
            if let Some(t) = self.first_type(&node) {
                let t = self.to_string(&t);
                if types.contains(&t) {
                    return node;
                }
            }
        }
        V::False
    }

    /// `isPart(node)` (:262-275).
    pub(crate) fn is_part(&mut self, node: &V) -> bool {
        let r = self.res(node);
        let Some(arcs) = self.z.get_arcs_in(r) else {
            return false;
        };
        let skip = [
            format!("{DC}relation"),
            format!("{DC1_0}relation"),
            format!("{DCTERMS}relation"),
            format!("{DCTERMS}hasPart"),
        ];
        arcs.iter().any(|a| !skip.contains(a))
    }

    /// `handleCreators(newItem, creators, creatorType)` (:115-162).
    pub(crate) fn handle_creators(
        &mut self,
        item: &mut TranslatorItem,
        creators: &V,
        creator_type: &str,
    ) -> Result<(), TranslateError> {
        if !creators.truthy() {
            return Ok(());
        }
        let mut list: Vec<Option<V>> = creators.list().iter().cloned().map(Some).collect();
        if let Some(first) = creators.list().first() {
            if first.as_str().is_none() {
                let r = self.res(first);
                let c = self.z.get_container_elements(r);
                if !c.is_empty() {
                    list = c.into_iter().map(|x| x.map(V::from_rdf)).collect();
                }
            }
        }
        for c in list {
            let Some(c) = c else {
                // A hole in the container's numbering: `getFirstResults(undefined)`.
                return Err(err(
                    "TypeError: Cannot read properties of undefined (reading 'length')",
                ));
            };
            if let Some(info) = self.extract_creator_info(&c, creator_type)? {
                item.creators.push(info);
            }
        }
        Ok(())
    }

    fn creator_from_clean(&self, s: &str, creator_type: &str) -> TranslatorCreator {
        let a = clean_author(s, creator_type, s.contains(','));
        TranslatorCreator {
            first_name: a.first_name,
            last_name: Some(a.last_name),
            creator_type: Some(a.creator_type),
            ..Default::default()
        }
    }

    fn extract_creator_info(
        &mut self,
        obj: &V,
        creator_type: &str,
    ) -> Result<Option<TranslatorCreator>, TranslateError> {
        if let V::Str(s) = obj {
            return Ok(Some(self.creator_from_clean(s, creator_type)));
        }
        let p = |l: &[&str]| -> Vec<String> { l.iter().map(|s| s.to_string()).collect() };
        let last = self.first_results(
            obj,
            &p(&[
                &format!("{FOAF}familyName"),
                &format!("{FOAF}lastName"),
                &format!("{FOAF}surname"),
                &format!("{FOAF}family_name"),
                &format!("{SO}familyName"),
            ]),
            true,
        );
        let first = self.first_results(
            obj,
            &p(&[
                &format!("{FOAF}givenName"),
                &format!("{FOAF}firstName"),
                &format!("{FOAF}givenname"),
                &format!("{SO}givenName"),
            ]),
            true,
        );
        let mut c = TranslatorCreator {
            creator_type: Some(creator_type.to_owned()),
            last_name: match &last {
                V::Undef => None,
                v => Some(self.to_string(v)),
            },
            first_name: match &first {
                V::Undef => None,
                v => Some(self.to_string(v)),
            },
            ..Default::default()
        };
        if !first.truthy() {
            c.field_mode = Some(1);
        }
        if first.truthy() || last.truthy() {
            return Ok(Some(c));
        }
        let name = self.first_results(obj, &[format!("{SO}name")], true);
        match name {
            V::Str(s) if !s.is_empty() => Ok(Some(self.creator_from_clean(&s, creator_type))),
            v if v.truthy() => Err(err("TypeError: c.includes is not a function")),
            _ => Ok(None),
        }
    }

    /// `processCollection(node, collection)` (:165-199).
    fn process_collection(&mut self, node: &V) -> TranslatorCollection {
        let name = self.first_results(
            node,
            &[
                format!("{DC}title"),
                format!("{DC1_0}title"),
                format!("{DCTERMS}title"),
            ],
            true,
        );
        let mut col = TranslatorCollection {
            name: match &name {
                V::Undef => String::new(),
                v => self.to_string(v),
            },
            children: Vec::new(),
        };
        let children = self.first_results(node, &[format!("{DCTERMS}hasPart")], false);
        for child in children.list().to_vec() {
            let ty = self.first_type(&child).map(|t| self.to_string(&t));
            if ty.as_deref() == Some(&format!("{BIB}Collection"))
                || ty.as_deref() == Some(&format!("{Z}Collection"))
            {
                let sub = self.process_collection(&child);
                col.children.push(CollectionChild::Collection(sub));
            } else {
                if self.is_part(&child) {
                    continue;
                }
                let id = self.resource_uri_string(&child);
                col.children.push(CollectionChild::Item { id });
            }
        }
        col
    }

    /// `processSeeAlso(node, newItem)` (:201-210), for an item.
    pub(crate) fn process_see_also(&mut self, node: &V, item: &mut TranslatorItem) {
        let relations = self.first_results(
            node,
            &[
                format!("{DC}relation"),
                format!("{DC1_0}relation"),
                format!("{DCTERMS}relation"),
            ],
            false,
        );
        let id = self.resource_uri(node);
        self.set(item, "itemID", &id);
        item.see_also = Vec::new();
        for r in relations.list().to_vec() {
            let u = self.resource_uri(&r);
            item.see_also.push(self.json_of(&u));
        }
    }

    /// `processSeeAlso` and `processTags` (:212-233) for a note (a plain
    /// object): sets `itemID`, `seeAlso`, `tags`.
    pub(crate) fn note_see_also_and_tags(
        &mut self,
        node: &V,
        note: &mut crate::zotero::framework::JsObject,
    ) {
        let relations = self.first_results(
            node,
            &[
                format!("{DC}relation"),
                format!("{DC1_0}relation"),
                format!("{DCTERMS}relation"),
            ],
            false,
        );
        let id = self.resource_uri(node);
        note.set("itemID", self.json_of(&id));
        let mut see = Vec::new();
        for r in relations.list().to_vec() {
            let u = self.resource_uri(&r);
            see.push(self.json_of(&u));
        }
        note.set("seeAlso", Value::Array(see));
        let subjects = self.first_results(
            node,
            &[
                format!("{DC}subject"),
                format!("{DC1_0}subject"),
                format!("{DCTERMS}subject"),
            ],
            false,
        );
        let mut tags = Vec::new();
        for s in subjects.list().to_vec() {
            match &s {
                V::Str(t) => tags.push(Value::String(t.clone())),
                _ => {
                    if let Some(t) = self.first_type(&s) {
                        if self.to_string(&t) == format!("{Z}AutomaticTag") {
                            let v = self.first_results(&s, &[format!("{RDF_NS}value")], true);
                            let mut m = serde_json::Map::new();
                            if !matches!(v, V::Undef) {
                                m.insert("tag".into(), self.json_of(&v));
                            }
                            m.insert("type".into(), 1.into());
                            tags.push(Value::Object(m));
                        }
                    }
                }
            }
        }
        note.set("tags", Value::Array(tags));
    }

    /// `getNodes(skipCollections)` (:1449-1481).
    fn get_nodes(&mut self, skip_collections: bool) -> Vec<V> {
        let nodes = self.z.get_all_resources();
        let mut good = Vec::new();
        for n in nodes {
            let node = V::Node(n);
            let r = self.res(&node);
            if self
                .z
                .get_sources(r.clone(), format!("{DCTERMS}isPartOf"))
                .is_some()
                || self
                    .z
                    .get_sources(r.clone(), format!("{BIB}presentedAt"))
                    .is_some()
                || self
                    .z
                    .get_sources(r.clone(), format!("{LINK}link"))
                    .is_some()
                || self.z.get_sources(r, format!("{DCTERMS}creator")).is_some()
            {
                continue;
            }
            if let Some(t) = self.first_type(&node) {
                let t = self.to_string(&t);
                if (t == format!("{Z}Attachment") || t == format!("{BIB}Memo"))
                    && self.is_part(&node)
                {
                    continue;
                } else if skip_collections
                    && (t == format!("{BIB}Collection") || t == format!("{Z}Collection"))
                {
                    continue;
                }
            }
            good.push(node);
        }
        good
    }
}

/// `detectImport()` (:46-54): any input the RDF data mode can parse (its
/// `getAllResources()` is an array, and an array is truthy).
pub fn detect_import(ctx: &mut ImportContext) -> bool {
    RdfSandbox::from_xml(&ctx.input.text(), ctx.options.env.first_blank_node_id).is_ok()
}

/// `doImport()` / `startImport` / `importNext` (:1483-1569).
pub fn do_import(ctx: &mut ImportContext) -> Result<(), TranslateError> {
    let z = RdfSandbox::from_xml(&ctx.input.text(), ctx.options.env.first_blank_node_id)
        .map_err(err)?;
    let mut t = Rdf { z, ctx };
    let nodes = t.get_nodes(false);
    let mut collections = Vec::new();
    for node in nodes {
        if let Some(ty) = t.first_type(&node) {
            let ty = t.to_string(&ty);
            if (ty == format!("{Z}Attachment") || ty == format!("{BIB}Memo")) && t.is_part(&node) {
                continue;
            }
            if ty == format!("{BIB}Collection") || ty == format!("{Z}Collection") {
                collections.push(node);
                continue;
            }
        }
        let mut item = TranslatorItem::new("");
        let id = t.resource_uri(&node);
        t.set(&mut item, "itemID", &id);
        if t.import_item(&mut item, &node)? {
            t.ctx.item_done(item);
        }
    }
    for c in collections {
        let r = t.res(&c);
        if t.z.get_arcs_in(r).is_none() {
            let col = t.process_collection(&c);
            t.ctx.collection_done(col);
        }
    }
    Ok(())
}
