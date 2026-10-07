// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): "Zotero RDF.js" (translatorID
//   14763d24-8ba0-45df-8f52-b8d1108e7ac9, lastUpdated 2026-03-06 21:33:15):
//   `generateRelations` :38-48, `generateTags` :50-65, `generateCollection`
//   :67-88, `getDisplayTitle` :94-127, `generateItem` :129-500, `doExport`
//   :502-582.
// Copyright (c) Simon Kornblith, Corporation for Digital Scholarship.
// Licence: AGPL-3.0. Upstream carries no licence text; treated as AGPLv3 as
//   part of Zotero, maintainer decision 2026-10-07, #747.

//! The Zotero RDF export translator: Zotero's full-fidelity library format
//! (items, notes, attachments, tags, related items and collections).
//!
//! Values are read as upstream reads them from the export item, which here is
//! loosely typed JSON: a missing value where upstream calls
//! `Zotero.RDF.addStatement` (a creator with no last name, a note with no
//! text) is the same error upstream throws.

use super::rdf_support::{encode_uri, item_unique_fields_order, js_key, INHERITED_NAMES};
use crate::zotero::framework::js;
use crate::zotero::framework::options::{translator_type, HeaderValue, TranslatorMetadata};
use crate::zotero::framework::rdf::{Node, RdfSandbox, Res, RDF_NS};
use crate::zotero::framework::{ExportContext, JsObject, TranslateError, TranslatorItem};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// The translator header.
pub static METADATA: TranslatorMetadata = TranslatorMetadata {
    id: "14763d24-8ba0-45df-8f52-b8d1108e7ac9",
    label: "Zotero RDF",
    creator: "Simon Kornblith",
    target: "rdf",
    min_version: "1.0.0b4.r1",
    priority: 25,
    translator_type: translator_type::EXPORT,
    config_options: &[
        ("getCollections", HeaderValue::Str("true")),
        ("dataMode", HeaderValue::Str("rdf/xml")),
    ],
    display_options: &[
        ("exportNotes", HeaderValue::Bool(true)),
        ("exportFileData", HeaderValue::Bool(false)),
    ],
    hidden_prefs: &[],
    last_updated: "2026-03-06 21:33:15",
};

const BIB: &str = "http://purl.org/net/biblio#";
const DC: &str = "http://purl.org/dc/elements/1.1/";
const DCTERMS: &str = "http://purl.org/dc/terms/";
const PRISM: &str = "http://prismstandard.org/namespaces/1.2/basic/";
const FOAF: &str = "http://xmlns.com/foaf/0.1/";
const VCARD: &str = "http://nwalsh.com/rdf/vCard#";
const VCARD2: &str = "http://www.w3.org/2006/vcard/ns#";
const LINK: &str = "http://purl.org/rss/1.0/modules/link/";
const Z: &str = "http://www.zotero.org/namespaces/export#";

/// `n` (:24-36), in declaration order (the order `addNamespace` runs).
const NAMESPACES: [(&str, &str); 9] = [
    ("bib", BIB),
    ("dc", DC),
    ("dcterms", DCTERMS),
    ("prism", PRISM),
    ("foaf", FOAF),
    ("vcard", VCARD),
    ("vcard2", VCARD2),
    ("link", LINK),
    ("z", Z),
];

fn err(m: impl Into<String>) -> TranslateError {
    TranslateError::Translator(m.into())
}

/// A creator as `generateItem` reads it.
#[derive(Debug, Clone)]
struct GenCreator {
    last_name: Option<Value>,
    first_name: Option<Value>,
    creator_type: Option<Value>,
}

/// An item or attachment as `generateItem` reads it (upstream passes both
/// the top-level export items and the plain attachment objects inside them).
#[derive(Debug, Clone)]
struct GenItem {
    props: JsObject,
    item_type: Option<Value>,
    creators: Option<Vec<GenCreator>>,
    notes: Option<Vec<JsObject>>,
    attachments: Option<Vec<JsObject>>,
    tags: Option<Vec<Value>>,
    unique_fields: Option<Vec<String>>,
}

impl GenItem {
    fn from_item(item: &TranslatorItem, original: Option<&JsObject>) -> GenItem {
        GenItem {
            props: item.props.clone(),
            item_type: Some(Value::String(item.item_type.clone())),
            creators: Some(
                item.creators
                    .iter()
                    .map(|c| GenCreator {
                        last_name: c.last_name.clone().map(Value::String),
                        first_name: c.first_name.clone().map(Value::String),
                        creator_type: c.creator_type.clone().map(Value::String),
                    })
                    .collect(),
            ),
            notes: Some(
                item.notes
                    .iter()
                    .map(|n| {
                        let mut o = n.props.clone();
                        o.set("note", n.note.clone());
                        o
                    })
                    .collect(),
            ),
            attachments: Some(item.attachments.clone()),
            tags: Some(
                item.tags
                    .iter()
                    .map(|t| {
                        let mut m = serde_json::Map::new();
                        m.insert("tag".into(), t.tag.clone().into());
                        if let Some(ty) = t.tag_type {
                            m.insert("type".into(), ty.into());
                        }
                        Value::Object(m)
                    })
                    .collect(),
            ),
            unique_fields: item
                .props
                .contains("uniqueFields")
                .then(|| item_unique_fields_order(item, original)),
        }
    }

    /// An attachment object. Its nested key order is `serde_json`'s
    /// (sorted), where upstream keeps the input's: see the module docs of
    /// `tests/zotero_translators.rs`, library round trip.
    fn from_object(o: &JsObject) -> GenItem {
        let arr = |k: &str| o.get(k).and_then(Value::as_array).cloned();
        GenItem {
            props: o.clone(),
            item_type: o.get("itemType").cloned(),
            creators: arr("creators").map(|a| {
                a.iter()
                    .map(|c| GenCreator {
                        last_name: c.get("lastName").cloned(),
                        first_name: c.get("firstName").cloned(),
                        creator_type: c.get("creatorType").cloned(),
                    })
                    .collect()
            }),
            notes: arr("notes").map(|a| {
                a.iter()
                    .map(|n| match n {
                        Value::Object(m) => JsObject::from_map(m),
                        _ => JsObject::new(),
                    })
                    .collect()
            }),
            attachments: arr("attachments").map(|a| {
                a.iter()
                    .map(|n| match n {
                        Value::Object(m) => JsObject::from_map(m),
                        _ => JsObject::new(),
                    })
                    .collect()
            }),
            tags: arr("tags"),
            unique_fields: match o.get("uniqueFields") {
                Some(Value::Object(m)) => Some(m.keys().cloned().collect()),
                _ => None,
            },
        }
    }

    fn get(&self, k: &str) -> Option<&Value> {
        self.props.get(k)
    }

    fn truthy(&self, k: &str) -> bool {
        js::truthy(self.get(k))
    }

    fn string(&self, k: &str) -> String {
        js_key(self.get(k))
    }
}

/// The export's state (upstream's globals).
struct Export {
    rdf: RdfSandbox,
    /// `itemResources`: key (an itemID or URI as a property name) -> URI.
    item_resources: HashMap<String, String>,
    added_collections: HashSet<String>,
    export_notes: bool,
}

/// A JavaScript value as a literal for `addStatement(..., true)`; `undefined`
/// and `null` are errors there.
fn literal_of(v: Option<&Value>) -> Result<String, TranslateError> {
    match v {
        None => Err(err("value must be defined in Zotero.RDF.addStatement")),
        Some(Value::Null) => Err(err("value must be defined in Zotero.RDF.addStatement")),
        Some(v) => Ok(js::to_js_string(v)),
    }
}

impl Export {
    fn res(&self, key: &str) -> Res {
        match self.item_resources.get(key) {
            Some(u) => Res::Uri(u.clone()),
            None => Res::Undefined,
        }
    }

    fn add(&mut self, s: impl Into<Res>, p: &str, o: impl Into<Res>) -> Result<(), TranslateError> {
        self.rdf.add_statement(s, p, o).map_err(err)
    }

    fn lit(&mut self, s: impl Into<Res>, p: &str, v: &str) -> Result<(), TranslateError> {
        self.rdf.add_literal(s, p, v).map_err(err)
    }

    /// `generateRelations` (:38-48).
    fn generate_relations(
        &mut self,
        resource: &Res,
        relations: &Value,
    ) -> Result<(), TranslateError> {
        let Value::Object(m) = relations else {
            return Ok(());
        };
        for (predicate, uris) in m {
            if predicate != "dc:relation" {
                continue;
            }
            // `for (let uri of relations[predicate])`: an array's elements,
            // a string's characters.
            let list: Vec<String> = match uris {
                Value::Array(a) => a.iter().map(js::to_js_string).collect(),
                Value::String(s) => s.chars().map(String::from).collect(),
                _ => return Err(err("relations[predicate] is not iterable")),
            };
            for uri in list {
                if let Some(target) = self.item_resources.get(&uri).cloned() {
                    self.add(resource.clone(), &format!("{DC}relation"), target)?;
                }
            }
        }
        Ok(())
    }

    /// `generateTags` (:50-65).
    fn generate_tags(
        &mut self,
        resource: &Res,
        tags: Option<&[Value]>,
    ) -> Result<(), TranslateError> {
        let Some(tags) = tags else {
            return Err(err(
                "TypeError: Cannot read properties of undefined (reading 'length')",
            ));
        };
        for tag in tags {
            let ty = tag.get("type");
            // `tag.type == 1`
            let automatic = match ty {
                Some(Value::Number(n)) => n.as_f64() == Some(1.0),
                Some(Value::String(s)) => s.trim() == "1",
                Some(Value::Bool(b)) => *b,
                _ => false,
            };
            if automatic {
                let t = self.rdf.new_resource();
                self.add(
                    t.clone(),
                    &format!("{RDF_NS}type"),
                    format!("{Z}AutomaticTag"),
                )?;
                let v = literal_of(tag.get("tag"))?;
                self.lit(t.clone(), &format!("{RDF_NS}value"), &v)?;
                self.add(resource.clone(), &format!("{DC}subject"), t)?;
            } else {
                let v = literal_of(tag.get("tag"))?;
                self.lit(resource.clone(), &format!("{DC}subject"), &v)?;
            }
        }
        Ok(())
    }

    /// `generateCollection` (:67-88).
    fn generate_collection(&mut self, collection: &JsObject) -> Result<(), TranslateError> {
        let resource = format!("#collection_{}", js_key(collection.get("id")));
        self.add(
            resource.as_str(),
            &format!("{RDF_NS}type"),
            format!("{Z}Collection"),
        )?;
        let name = literal_of(collection.get("name"))?;
        self.lit(resource.as_str(), &format!("{DC}title"), &name)?;
        let children = if collection.truthy("children") {
            collection.get("children")
        } else {
            collection.get("descendents")
        };
        let Some(children) = children.filter(|c| js::truthy(Some(c))) else {
            return Ok(());
        };
        let children: Vec<Value> = children.as_array().cloned().unwrap_or_default();
        for child in children {
            let child_obj = match &child {
                Value::Object(m) => JsObject::from_map(m),
                _ => JsObject::new(),
            };
            if child_obj.get_str("type") == Some("collection") {
                let id = js_key(child_obj.get("id"));
                self.add(
                    resource.as_str(),
                    &format!("{DCTERMS}hasPart"),
                    format!("#collection_{id}"),
                )?;
                self.added_collections.insert(id);
                self.generate_collection(&child_obj)?;
            } else if let Some(target) = self
                .item_resources
                .get(&js_key(child_obj.get("id")))
                .cloned()
            {
                self.add(resource.as_str(), &format!("{DCTERMS}hasPart"), target)?;
            }
        }
        Ok(())
    }

    /// `generateItem(item, zoteroType, resource)` (:129-500).
    fn generate_item(
        &mut self,
        item: &GenItem,
        zotero_type: &str,
        resource: Res,
    ) -> Result<(), TranslateError> {
        let rdf_type = format!("{RDF_NS}type");
        let mut container: Option<String> = None;
        let mut container_element: Option<Res> = None;
        let b = |s: &str| format!("{BIB}{s}");
        let ty: Option<String> = match zotero_type {
            "book" => Some(b("Book")),
            "bookSection" => {
                container = Some(b("Book"));
                Some(b("BookSection"))
            }
            "journalArticle" => {
                container = Some(b("Journal"));
                Some(b("Article"))
            }
            "magazineArticle" => {
                container = Some(b("Periodical"));
                Some(b("Article"))
            }
            "newspaperArticle" => {
                container = Some(b("Newspaper"));
                Some(b("Article"))
            }
            "thesis" => Some(b("Thesis")),
            "letter" => Some(b("Letter")),
            "manuscript" => Some(b("Manuscript")),
            "interview" => Some(b("Interview")),
            "film" => Some(b("MotionPicture")),
            "artwork" => Some(b("Illustration")),
            "webpage" => {
                container = Some(format!("{Z}Website"));
                Some(b("Document"))
            }
            "note" => {
                if !self.export_notes {
                    return Ok(());
                }
                Some(b("Memo"))
            }
            "attachment" => Some(format!("{Z}Attachment")),
            "report" => Some(b("Report")),
            "bill" => Some(b("Legislation")),
            "case" => {
                container = Some(b("CourtReporter"));
                Some(b("Document"))
            }
            "hearing" => Some(b("Report")),
            "patent" => Some(b("Patent")),
            "statute" => Some(b("Legislation")),
            "email" => Some(b("Letter")),
            "map" => Some(b("Image")),
            "blogPost" => {
                container = Some(format!("{Z}Blog"));
                Some(b("Document"))
            }
            "instantMessage" => Some(b("Letter")),
            "forumPost" => {
                container = Some(format!("{Z}Forum"));
                Some(b("Document"))
            }
            "audioRecording" => Some(b("Recording")),
            "presentation" => Some(b("ConferenceProceedings")),
            "videoRecording" | "tvBroadcast" | "radioBroadcast" | "podcast" => Some(b("Recording")),
            "computerProgram" => Some(b("Data")),
            "encyclopediaArticle" | "dictionaryEntry" => {
                container = Some(b("Book"));
                None
            }
            "conferencePaper" => {
                container = Some(b("Journal"));
                None
            }
            _ => None,
        };

        if let Some(t) = &ty {
            self.add(resource.clone(), &rdf_type, t.clone())?;
        }
        self.lit(resource.clone(), &format!("{Z}itemType"), zotero_type)?;

        // section
        let mut section: Option<Node> = None;
        if item.truthy("section") {
            let s = self.rdf.new_resource();
            self.add(s.clone(), &rdf_type, b("Part"))?;
            let v = literal_of(item.get("section"))?;
            self.lit(s.clone(), &format!("{DC}title"), &v)?;
            self.add(resource.clone(), &format!("{DCTERMS}isPartOf"), s.clone())?;
            section = Some(s);
        }

        // container
        if let Some(c) = &container {
            let test_issn = format!("urn:issn:{}", encode_uri(&item.string("ISSN")));
            let ce: Res =
                if item.truthy("ISSN") && self.rdf.get_arcs_in(test_issn.as_str()).is_none() {
                    Res::Uri(test_issn)
                } else {
                    Res::Node(self.rdf.new_resource())
                };
            let attach: Res = match &section {
                Some(s) => Res::Node(s.clone()),
                None => resource.clone(),
            };
            self.add(attach, &format!("{DCTERMS}isPartOf"), ce.clone())?;
            self.add(ce.clone(), &rdf_type, c.clone())?;
            container_element = Some(ce);
        }

        // series
        let mut series: Option<Node> = None;
        if item.truthy("series")
            || item.truthy("seriesTitle")
            || item.truthy("seriesText")
            || item.truthy("seriesNumber")
        {
            let s = self.rdf.new_resource();
            self.add(s.clone(), &rdf_type, b("Series"))?;
            let attach = container_element
                .clone()
                .unwrap_or_else(|| resource.clone());
            self.add(attach, &format!("{DCTERMS}isPartOf"), s.clone())?;
            series = Some(s);
        }

        // publisher
        let mut organization: Option<Node> = None;
        if zotero_type == "nsfReviewer" {
            let o = self.rdf.new_resource();
            self.add(o.clone(), &rdf_type, format!("{VCARD2}Organization"))?;
            self.add(resource.clone(), &format!("{VCARD2}org"), o.clone())?;
            organization = Some(o);
        } else if [
            "publisher",
            "distributor",
            "label",
            "company",
            "institution",
            "place",
        ]
        .iter()
        .any(|k| item.truthy(k))
        {
            let o = self.rdf.new_resource();
            self.add(o.clone(), &rdf_type, format!("{FOAF}Organization"))?;
            self.add(resource.clone(), &format!("{DC}publisher"), o.clone())?;
            organization = Some(o);
        }

        // creators
        if let Some(creators) = &item.creators {
            let mut containers: Vec<(String, Node)> = Vec::new();
            for c in creators {
                let creator = self.rdf.new_resource();
                self.add(creator.clone(), &rdf_type, format!("{FOAF}Person"))?;
                let last = literal_of(c.last_name.as_ref())?;
                self.lit(creator.clone(), &format!("{FOAF}surname"), &last)?;
                if js::truthy(c.first_name.as_ref()) {
                    let first = literal_of(c.first_name.as_ref())?;
                    self.lit(creator.clone(), &format!("{FOAF}givenName"), &first)?;
                }
                let ct = js_key(c.creator_type.as_ref());
                let c_tag = if ["author", "editor", "contributor"].contains(&ct.as_str())
                    && c.creator_type.as_ref().is_some_and(Value::is_string)
                {
                    format!("{BIB}{ct}s")
                } else {
                    format!("{Z}{ct}s")
                };
                let seq = match containers.iter().find(|(t, _)| *t == c_tag) {
                    Some((_, n)) => n.clone(),
                    None => {
                        let r = self.rdf.new_resource();
                        let seq = self.rdf.new_container("seq", r.clone()).map_err(err)?;
                        self.add(resource.clone(), &c_tag, r)?;
                        containers.push((c_tag.clone(), seq.clone()));
                        seq
                    }
                };
                self.rdf.add_container_element(seq, creator).map_err(err)?;
            }
        }

        // notes
        if let Some(notes) = &item.notes {
            if self.export_notes {
                for note in notes {
                    let note_resource = self.res(&js_key(note.get("itemID")));
                    self.add(note_resource.clone(), &rdf_type, b("Memo"))?;
                    let text = literal_of(note.get("note"))?;
                    self.lit(note_resource.clone(), &format!("{RDF_NS}value"), &text)?;
                    self.add(
                        resource.clone(),
                        &format!("{DCTERMS}isReferencedBy"),
                        note_resource.clone(),
                    )?;
                    if let Some(r) = note.get("relations").filter(|r| js::truthy(Some(r))) {
                        let r = r.clone();
                        self.generate_relations(&note_resource, &r)?;
                    }
                    let tags = note.get("tags").and_then(Value::as_array).cloned();
                    self.generate_tags(&note_resource, tags.as_deref())?;
                }
            }
        }

        // child attachments
        if let Some(attachments) = &item.attachments {
            for a in attachments {
                let ar = self.res(&js_key(a.get("itemID")));
                self.add(resource.clone(), &format!("{LINK}link"), ar.clone())?;
                self.generate_item(&GenItem::from_object(a), "attachment", ar)?;
            }
        }

        // path
        if item.truthy("defaultPath") {
            // `item.saveFile(item.defaultPath, true)`: only Zotero desktop
            // with exportFileData gives items a saveFile.
            return Err(err("TypeError: item.saveFile is not a function"));
        } else if item.truthy("path") {
            self.add(resource.clone(), &format!("{Z}path"), item.string("path"))?;
        }

        // relations and tags
        if let Some(r) = item.get("relations").filter(|r| js::truthy(Some(r))) {
            let r = r.clone();
            self.generate_relations(&resource, &r)?;
        }
        if let Some(tags) = &item.tags {
            let tags = tags.clone();
            self.generate_tags(&resource, Some(&tags))?;
        }

        // fields
        let type_properties = [
            "reportType",
            "videoRecordingType",
            "letterType",
            "manuscriptType",
            "mapType",
            "thesisType",
            "websiteType",
            "audioRecordingType",
            "presentationType",
            "postType",
            "audioFileType",
        ];
        let ignore_properties = [
            "itemID",
            "itemType",
            "firstCreator",
            "dateAdded",
            "dateModified",
            "section",
            "sourceItemID",
        ];
        let on_container = |ce: &Option<Res>| ce.clone().unwrap_or_else(|| resource.clone());
        let unique = item.unique_fields.clone().unwrap_or_default();
        for property in &unique {
            let value = item.get(property);
            if !js::truthy(value) {
                continue;
            }
            let v = literal_of(value)?;
            let p = property.as_str();
            match p {
                "title" => {
                    if zotero_type == "nsfReviewer" {
                        self.lit(resource.clone(), &format!("{VCARD2}fn"), &v)?;
                    } else {
                        self.lit(resource.clone(), &format!("{DC}title"), &v)?;
                    }
                }
                "source" => self.lit(resource.clone(), &format!("{DC}source"), &v)?,
                "url" => {
                    if item.truthy("homepage") {
                        self.add(resource.clone(), &format!("{VCARD2}url"), v.as_str())?;
                    } else {
                        let term = self.rdf.new_resource();
                        self.add(term.clone(), &rdf_type, format!("{DCTERMS}URI"))?;
                        self.lit(term.clone(), &format!("{RDF_NS}value"), &v)?;
                        self.add(resource.clone(), &format!("{DC}identifier"), term)?;
                    }
                }
                "accessionNumber" => self.lit(resource.clone(), &format!("{DC}identifier"), &v)?,
                "rights" => self.lit(resource.clone(), &format!("{DC}rights"), &v)?,
                "edition" | "version" => {
                    self.lit(resource.clone(), &format!("{PRISM}edition"), &v)?
                }
                "date" => {
                    if item.truthy("dateSent") {
                        self.lit(resource.clone(), &format!("{DCTERMS}dateSubmitted"), &v)?;
                    } else {
                        self.lit(resource.clone(), &format!("{DC}date"), &v)?;
                    }
                }
                "accessDate" => {
                    self.lit(resource.clone(), &format!("{DCTERMS}dateSubmitted"), &v)?
                }
                "issueDate" => self.lit(resource.clone(), &format!("{DCTERMS}issued"), &v)?,
                "pages" => self.lit(resource.clone(), &format!("{BIB}pages"), &v)?,
                "extra" => self.lit(resource.clone(), &format!("{DC}description"), &v)?,
                "mimeType" => self.lit(resource.clone(), &format!("{LINK}type"), &v)?,
                "charset" => self.lit(resource.clone(), &format!("{LINK}charset"), &v)?,
                "ISSN" => self.lit(
                    on_container(&container_element),
                    &format!("{DC}identifier"),
                    &format!("ISSN {v}"),
                )?,
                "ISBN" => self.lit(
                    on_container(&container_element),
                    &format!("{DC}identifier"),
                    &format!("ISBN {v}"),
                )?,
                "DOI" => self.lit(
                    on_container(&container_element),
                    &format!("{DC}identifier"),
                    &format!("DOI {v}"),
                )?,
                "publicationTitle" | "reporter" => {
                    self.lit(on_container(&container_element), &format!("{DC}title"), &v)?
                }
                "journalAbbreviation" => self.lit(
                    on_container(&container_element),
                    &format!("{DCTERMS}alternative"),
                    &v,
                )?,
                "volume" => self.lit(
                    on_container(&container_element),
                    &format!("{PRISM}volume"),
                    &v,
                )?,
                "issue" | "number" | "patentNumber" => self.lit(
                    on_container(&container_element),
                    &format!("{PRISM}number"),
                    &v,
                )?,
                "callNumber" => {
                    let term = self.rdf.new_resource();
                    self.add(term.clone(), &rdf_type, format!("{DCTERMS}LCC"))?;
                    self.lit(term.clone(), &format!("{RDF_NS}value"), &v)?;
                    self.add(resource.clone(), &format!("{DC}subject"), term)?;
                }
                "abstractNote" => self.lit(resource.clone(), &format!("{DCTERMS}abstract"), &v)?,
                "series" => self.lit(node_or_undefined(&series), &format!("{DC}title"), &v)?,
                "seriesTitle" => self.lit(
                    node_or_undefined(&series),
                    &format!("{DCTERMS}alternative"),
                    &v,
                )?,
                "seriesText" => {
                    self.lit(node_or_undefined(&series), &format!("{DC}description"), &v)?
                }
                "seriesNumber" => {
                    self.lit(node_or_undefined(&series), &format!("{DC}identifier"), &v)?
                }
                "publisher" | "distributor" | "label" | "company" | "institution" => {
                    if zotero_type == "nsfReviewer" {
                        self.lit(
                            node_or_undefined(&organization),
                            &format!("{VCARD2}organization-name"),
                            &v,
                        )?;
                    } else {
                        self.lit(node_or_undefined(&organization), &format!("{FOAF}name"), &v)?;
                    }
                }
                "place" => {
                    let address = self.rdf.new_resource();
                    self.add(address.clone(), &rdf_type, format!("{VCARD}Address"))?;
                    self.lit(address.clone(), &format!("{VCARD}locality"), &v)?;
                    self.add(
                        node_or_undefined(&organization),
                        &format!("{VCARD}adr"),
                        address,
                    )?;
                }
                "archiveLocation" => self.lit(resource.clone(), &format!("{DC}coverage"), &v)?,
                "interviewMedium" | "artworkMedium" => {
                    self.lit(resource.clone(), &format!("{DCTERMS}medium"), &v)?
                }
                "conferenceName" => {
                    let conference = self.rdf.new_resource();
                    self.add(conference.clone(), &rdf_type, b("Conference"))?;
                    self.lit(conference.clone(), &format!("{DC}title"), &v)?;
                    self.add(resource.clone(), &format!("{BIB}presentedAt"), conference)?;
                }
                _ if type_properties.contains(&p) => {
                    self.lit(resource.clone(), &format!("{DC}type"), &v)?
                }
                "note" => {
                    if self.export_notes {
                        let it = item.item_type.as_ref().map(js::to_js_string);
                        if it.as_deref() == Some("attachment") {
                            self.lit(resource.clone(), &format!("{DC}description"), &v)?;
                        } else if it.as_deref() == Some("note") {
                            self.lit(resource.clone(), &format!("{RDF_NS}value"), &v)?;
                        }
                    }
                }
                "address" => {
                    let address = self.rdf.new_resource();
                    self.add(address.clone(), &rdf_type, format!("{VCARD2}Address"))?;
                    self.lit(address.clone(), &format!("{VCARD2}label"), &v)?;
                    self.add(resource.clone(), &format!("{VCARD2}adr"), address)?;
                }
                "telephone" => self.lit(resource.clone(), &format!("{VCARD2}tel"), &v)?,
                "email" => self.lit(resource.clone(), &format!("{VCARD2}email"), &v)?,
                "accepted" => self.lit(resource.clone(), &format!("{DCTERMS}dateAccepted"), &v)?,
                _ if !ignore_properties.contains(&p) => {
                    self.lit(resource.clone(), &format!("{Z}{p}"), &v)?;
                }
                _ => {}
            }
        }

        if let Some(dt) = display_title(item) {
            self.lit(resource, &format!("{Z}displayTitle"), &dt)?;
        }
        Ok(())
    }
}

/// A node, or `undefined` where upstream's variable was never assigned.
fn node_or_undefined(n: &Option<Node>) -> Res {
    match n {
        Some(n) => Res::Node(n.clone()),
        None => Res::Undefined,
    }
}

/// `getDisplayTitle(item)` (:94-127).
fn display_title(item: &GenItem) -> Option<String> {
    let it = item
        .item_type
        .as_ref()
        .map(js::to_js_string)
        .unwrap_or_default();
    if !item.truthy("title") && (it == "interview" || it == "letter") {
        let letter = it == "letter";
        let names: Vec<String> = item
            .creators
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .filter(|c| {
                let ct = js_key(c.creator_type.as_ref());
                (letter && ct == "recipient") || (!letter && ct == "interviewer")
            })
            .map(|c| js_key(c.last_name.as_ref()))
            .collect();
        let mut dt = format!("[{}", if letter { "Letter" } else { "Interview" });
        if !names.is_empty() {
            dt.push_str(if letter { " to " } else { " of " });
            dt.push_str(&names[0]);
            match names.len() {
                2 => dt.push_str(&format!(" and {}", names[1])),
                3 => dt.push_str(&format!(", {}, and {}", names[1], names[2])),
                n if n > 3 => dt.push_str(" et al."),
                _ => {}
            }
        }
        return Some(format!("{dt}]"));
    }
    if it == "case" && item.truthy("title") && item.truthy("reporter") {
        return Some(format!(
            "{} ({})",
            item.string("title"),
            item.string("reporter")
        ));
    }
    None
}

/// `doExport()` (:502-582).
///
/// `originals` are the export input objects the context's items were made
/// from, in order; they give the order of each item's `uniqueFields`
/// ([`super::rdf_support::unique_fields_order`]).
pub fn do_export(ctx: &mut ExportContext, originals: &[JsObject]) -> Result<(), TranslateError> {
    let mut ex = Export {
        rdf: RdfSandbox::new(ctx.options.env.first_blank_node_id),
        item_resources: HashMap::new(),
        added_collections: HashSet::new(),
        export_notes: js::truthy(ctx.get_option("exportNotes")),
    };
    for (p, u) in NAMESPACES {
        ex.rdf.add_namespace(p, u);
    }
    let mut used: HashSet<String> = HashSet::new();
    let is_used =
        |used: &HashSet<String>, k: &str| used.contains(k) || INHERITED_NAMES.contains(&k);
    let mut items: Vec<GenItem> = Vec::new();
    let mut n = 0;
    while let Some(item) = ctx.next_item() {
        let g = GenItem::from_item(&item, originals.get(n));
        n += 1;
        let item_id = js_key(item.get("itemID"));
        let uri = js_key(item.get("uri"));
        let test_isbn = format!("urn:isbn:{}", encode_uri(&g.string("ISBN")));
        let r = if g.truthy("ISBN") && !is_used(&used, &test_isbn) {
            used.insert(test_isbn.clone());
            test_isbn
        } else if item.item_type != "attachment"
            && g.truthy("url")
            && !is_used(&used, &g.string("url"))
        {
            let u = g.string("url");
            used.insert(u.clone());
            u
        } else {
            format!("#item_{item_id}")
        };
        ex.item_resources.insert(item_id, r.clone());
        ex.item_resources.insert(uri, r);
        for n in g.notes.as_deref().unwrap_or(&[]) {
            let r = format!("#item_{}", js_key(n.get("itemID")));
            ex.item_resources.insert(js_key(n.get("itemID")), r.clone());
            ex.item_resources.insert(js_key(n.get("uri")), r);
        }
        for a in g.attachments.as_deref().unwrap_or(&[]) {
            let r = format!("#item_{}", js_key(a.get("itemID")));
            ex.item_resources.insert(js_key(a.get("itemID")), r.clone());
            ex.item_resources.insert(js_key(a.get("uri")), r);
        }
        items.push(g);
    }
    for g in &items {
        let it = g
            .item_type
            .as_ref()
            .map(js::to_js_string)
            .unwrap_or_default();
        let resource = ex.res(&js_key(g.get("itemID")));
        ex.generate_item(g, &it, resource)?;
    }
    while let Some(c) = ctx.next_collection() {
        if ex.added_collections.contains(&js_key(c.get("id"))) {
            continue;
        }
        ex.generate_collection(&c)?;
    }
    let out = ex.rdf.serialize().map_err(err)?;
    ctx.write(&out);
    Ok(())
}
