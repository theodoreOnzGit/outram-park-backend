// Part of the kovan Zotero port (GitHub #747, #748).
//
// Upstream: Zotero, https://github.com/zotero/zotero (commit 9cbba8c4d281):
//   chrome/content/zotero/xpcom/data/item.js (`Zotero.Item.prototype.toJSON`
//   :6008, `fromJSON` :5656), data/collection.js (`toJSON` :826, `fromJSON`
//   :792), data/dataObject.js (`getRelations` :324, `_postToJSON` :1565),
//   xpcom/attachments.js (link modes :35-39, `linkModeToName` :3436),
//   xpcom/annotations.js (annotation types :31-36), and
//   xpcom/utilities_internal.js (`itemToExportFormat` :1016).
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The Zotero item, collection and library model, in Zotero's Web API /
//! translator JSON shape.
//!
//! [`ZoteroItem`] (de)serialises the JSON that `Zotero.Item#toJSON` writes and
//! `Zotero.Item#fromJSON` reads (also the Web API's `data` object, and,
//! with `uri`, `attachments` and `notes`, the translator export format of
//! `itemToExportFormat`). Reading is lenient where upstream is lenient:
//!
//! * a number in a field becomes a string (item.js:779);
//! * a string under a name that is not a Zotero field, or a field that is
//!   not valid for the item type, is **kept** in [`ZoteroItem::fields`]
//!   (upstream moves it into Extra on import; call
//!   [`ZoteroItem::normalize_fields`] to do that, and
//!   [`ZoteroItem::validate`] to list such problems);
//! * any other unknown property is kept verbatim in
//!   [`ZoteroItem::other`], so it survives a round trip.
//!
//! Reading is strict where upstream is strict: an unknown `itemType`
//! (item.js:5664), an unknown `linkMode` (:5784) or an unknown creator type is
//! an error.

use super::schema_generated::{CreatorType, Field, ItemType};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// How an attachment's file is held (`Zotero.Attachments.LINK_MODE_*`,
/// attachments.js:35-39; JSON names from `linkModeToName`, :3436).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LinkMode {
    /// `imported_file`: a file stored in Zotero's storage.
    ImportedFile,
    /// `imported_url`: a web snapshot stored in Zotero's storage.
    ImportedUrl,
    /// `linked_file`: a file elsewhere on disk (`path` is set).
    LinkedFile,
    /// `linked_url`: a link to a web page, no file.
    LinkedUrl,
    /// `embedded_image`: an image embedded in a note.
    EmbeddedImage,
}

impl LinkMode {
    /// Every link mode, in upstream's numeric order.
    pub const ALL: [LinkMode; 5] = [
        LinkMode::ImportedFile,
        LinkMode::ImportedUrl,
        LinkMode::LinkedFile,
        LinkMode::LinkedUrl,
        LinkMode::EmbeddedImage,
    ];

    /// The JSON name.
    pub const fn as_str(self) -> &'static str {
        match self {
            LinkMode::ImportedFile => "imported_file",
            LinkMode::ImportedUrl => "imported_url",
            LinkMode::LinkedFile => "linked_file",
            LinkMode::LinkedUrl => "linked_url",
            LinkMode::EmbeddedImage => "embedded_image",
        }
    }

    /// Parse a JSON name. Upstream looks up `LINK_MODE_` + the upper-cased
    /// name (item.js:5784), so the match is case-insensitive.
    pub fn from_name(name: &str) -> Option<LinkMode> {
        LinkMode::ALL
            .into_iter()
            .find(|m| m.as_str().eq_ignore_ascii_case(name))
    }
}

/// The kind of a Zotero 7 annotation (`Zotero.Annotations.ANNOTATION_TYPE_*`,
/// annotations.js:31-36).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnnotationType {
    /// `highlight`
    Highlight,
    /// `note`
    Note,
    /// `image`
    Image,
    /// `ink`
    Ink,
    /// `underline`
    Underline,
    /// `text`
    Text,
}

impl AnnotationType {
    /// Every annotation type, in upstream's numeric order.
    pub const ALL: [AnnotationType; 6] = [
        AnnotationType::Highlight,
        AnnotationType::Note,
        AnnotationType::Image,
        AnnotationType::Ink,
        AnnotationType::Underline,
        AnnotationType::Text,
    ];

    /// The JSON name.
    pub const fn as_str(self) -> &'static str {
        match self {
            AnnotationType::Highlight => "highlight",
            AnnotationType::Note => "note",
            AnnotationType::Image => "image",
            AnnotationType::Ink => "ink",
            AnnotationType::Underline => "underline",
            AnnotationType::Text => "text",
        }
    }

    /// Parse a JSON name (exact match).
    pub fn from_name(name: &str) -> Option<AnnotationType> {
        AnnotationType::ALL.into_iter().find(|m| m.as_str() == name)
    }
}

/// A creator's name: two fields, or one (`fieldMode: 1`, e.g. an institution).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CreatorName {
    /// `firstName` + `lastName`.
    TwoField {
        /// Given name(s).
        first_name: String,
        /// Family name.
        last_name: String,
    },
    /// `name` (written by `toJSON` for `fieldMode: 1`).
    SingleField {
        /// The whole name.
        name: String,
    },
}

/// One creator of an item.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Creator {
    /// The role, e.g. `author`, `editor`.
    pub creator_type: CreatorType,
    /// The name.
    pub name: CreatorName,
}

impl Creator {
    /// A two-field creator.
    pub fn person(
        creator_type: CreatorType,
        first_name: impl Into<String>,
        last_name: impl Into<String>,
    ) -> Self {
        Creator {
            creator_type,
            name: CreatorName::TwoField {
                first_name: first_name.into(),
                last_name: last_name.into(),
            },
        }
    }

    /// A single-field creator (an organisation).
    pub fn single(creator_type: CreatorType, name: impl Into<String>) -> Self {
        Creator {
            creator_type,
            name: CreatorName::SingleField { name: name.into() },
        }
    }
}

/// One tag. `tag_type` is `None`/`Some(0)` for a manual tag and `Some(1)`
/// for an automatic one (imported with the metadata).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Tag {
    /// The tag text.
    pub tag: String,
    /// The JSON `type`; absent means 0 (manual).
    pub tag_type: Option<u8>,
}

impl Tag {
    /// Whether this is an automatic tag (`type: 1`).
    pub fn is_automatic(&self) -> bool {
        self.tag_type == Some(1)
    }
}

/// The attachment-only properties of an item (item.js:6038-6079, 5783-5806).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AttachmentData {
    /// `linkMode`; `None` only for an attachment JSON that omitted it.
    pub link_mode: Option<LinkMode>,
    /// `contentType`, e.g. `application/pdf`.
    pub content_type: Option<String>,
    /// `charset`.
    pub charset: Option<String>,
    /// `path`, for `linked_file` (absolute, or `attachments:` relative to the
    /// linked-attachment base directory).
    pub path: Option<String>,
    /// `filename`, for stored files.
    pub filename: Option<String>,
    /// `md5` of the stored file (sync only).
    pub md5: Option<String>,
    /// `mtime` of the stored file in ms (sync only).
    pub mtime: Option<i64>,
    /// `lastRead` (Unix seconds; user library only).
    pub last_read: Option<i64>,
}

/// The annotation-only properties of an item (item.js:6089-6101, 5811-5820).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AnnotationData {
    /// `annotationType`; `None` only for an annotation JSON that omitted it.
    pub annotation_type: Option<AnnotationType>,
    /// `annotationAuthorName`.
    pub author_name: Option<String>,
    /// `annotationText` (only highlights and underlines carry text).
    pub text: Option<String>,
    /// `annotationComment` (rich text).
    pub comment: Option<String>,
    /// `annotationColor`, e.g. `#ffd400`.
    pub color: Option<String>,
    /// `annotationPageLabel`.
    pub page_label: Option<String>,
    /// `annotationSortIndex`, e.g. `00015|002431|00000`.
    pub sort_index: Option<String>,
    /// `annotationPosition`: a JSON **string** (e.g.
    /// `{"pageIndex":1,"rects":[[...]]}`), kept as text as upstream does.
    pub position: Option<String>,
}

/// A Zotero item in Web API / translator JSON form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoteroItem {
    /// The 8-character object key.
    pub key: Option<String>,
    /// The object version (sync).
    pub version: Option<u64>,
    /// The item type.
    pub item_type: ItemType,
    /// Field name -> value, as stored (type-specific names, e.g.
    /// `websiteTitle` on a `webpage`). Includes `extra` and `accessDate`.
    pub fields: BTreeMap<String, String>,
    /// Creators, in order (regular items only).
    pub creators: Vec<Creator>,
    /// Tags.
    pub tags: Vec<Tag>,
    /// Keys of the collections a top-level item is in.
    pub collections: Vec<String>,
    /// Relations: predicate (e.g. `dc:relation`, `owl:sameAs`) -> object URIs.
    pub relations: BTreeMap<String, Vec<String>>,
    /// `parentItem`: the parent's key, for child notes, attachments and
    /// annotations.
    pub parent_item: Option<String>,
    /// `note`: HTML of a note, or an attachment's note.
    pub note: Option<String>,
    /// Attachment properties; set for `attachment` items.
    pub attachment: Option<AttachmentData>,
    /// Annotation properties; set for `annotation` items.
    pub annotation: Option<AnnotationData>,
    /// `deleted` (in the trash).
    pub deleted: Option<bool>,
    /// `inPublications` (My Publications).
    pub in_publications: Option<bool>,
    /// `dateAdded`, ISO 8601 UTC (e.g. `2015-04-12T09:00:22Z`).
    pub date_added: Option<String>,
    /// `dateModified`, ISO 8601 UTC.
    pub date_modified: Option<String>,
    /// `uri` (translator export format; e.g.
    /// `http://zotero.org/users/local/abc/items/KEY`); becomes the CSL `id`.
    pub uri: Option<String>,
    /// Child attachments (translator export format).
    pub attachments: Vec<ZoteroItem>,
    /// Child notes (translator export format).
    pub notes: Vec<ZoteroItem>,
    /// Every other property, verbatim (e.g. `itemID`, `libraryID`, or
    /// properties of a newer Zotero).
    pub other: BTreeMap<String, Value>,
}

/// Why a JSON value is not a Zotero item or collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZoteroJsonError {
    /// The value is not a JSON object.
    NotAnObject,
    /// `itemType` is missing (item.js:5660).
    MissingItemType,
    /// `itemType` names no known item type (item.js:5664).
    UnknownItemType(String),
    /// `linkMode` names no known link mode (item.js:5784).
    UnknownLinkMode(String),
    /// `annotationType` names no known annotation type.
    UnknownAnnotationType(String),
    /// A creator's `creatorType` names no known creator type.
    UnknownCreatorType(String),
    /// A property has the wrong JSON type; the payload names it.
    BadProperty(String),
    /// A collection has no `name` (collection.js:812).
    MissingCollectionName,
}

impl std::fmt::Display for ZoteroJsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZoteroJsonError::NotAnObject => write!(f, "not a JSON object"),
            ZoteroJsonError::MissingItemType => write!(f, "itemType property not provided"),
            ZoteroJsonError::UnknownItemType(t) => write!(f, "Unknown item type '{t}'"),
            ZoteroJsonError::UnknownLinkMode(m) => write!(f, "Unknown attachment link mode '{m}'"),
            ZoteroJsonError::UnknownAnnotationType(t) => write!(f, "Unknown annotation type '{t}'"),
            ZoteroJsonError::UnknownCreatorType(t) => write!(f, "Unknown creator type '{t}'"),
            ZoteroJsonError::BadProperty(p) => write!(f, "property '{p}' has the wrong JSON type"),
            ZoteroJsonError::MissingCollectionName => {
                write!(f, "'name' property not provided for collection")
            }
        }
    }
}

impl std::error::Error for ZoteroJsonError {}

fn bad(p: &str) -> ZoteroJsonError {
    ZoteroJsonError::BadProperty(p.to_owned())
}

/// A string, or a number written as a string (item.js:779).
fn as_text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// `false`/`null`/absent -> `None`, a string -> `Some` (`parentItem`,
/// `parentCollection`).
fn key_or_false(v: &Value, p: &str) -> Result<Option<String>, ZoteroJsonError> {
    match v {
        Value::Null | Value::Bool(false) => Ok(None),
        Value::String(s) if s.is_empty() => Ok(None),
        Value::String(s) => Ok(Some(s.clone())),
        _ => Err(bad(p)),
    }
}

fn opt_string(v: &Value, p: &str) -> Result<Option<String>, ZoteroJsonError> {
    match v {
        Value::Null => Ok(None),
        other => as_text(other).map(Some).ok_or_else(|| bad(p)),
    }
}

fn opt_i64(v: &Value, p: &str) -> Result<Option<i64>, ZoteroJsonError> {
    match v {
        Value::Null => Ok(None),
        Value::Number(n) => n.as_i64().map(Some).ok_or_else(|| bad(p)),
        Value::String(s) => s.parse().map(Some).map_err(|_| bad(p)),
        _ => Err(bad(p)),
    }
}

fn read_creator(v: &Value) -> Result<Creator, ZoteroJsonError> {
    let o = v.as_object().ok_or_else(|| bad("creators[]"))?;
    let ct = o
        .get("creatorType")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("creators[].creatorType"))?;
    let creator_type = CreatorType::from_name(ct)
        .ok_or_else(|| ZoteroJsonError::UnknownCreatorType(ct.to_owned()))?;
    let text = |k: &str| o.get(k).and_then(as_text).unwrap_or_default();
    // `name` is the single-field form; legacy `fieldMode: 1` + `lastName`
    // means the same (utilities_internal.js:1077-1080 writes it).
    let name = if let Some(n) = o.get("name").and_then(as_text) {
        CreatorName::SingleField { name: n }
    } else if o.get("fieldMode").and_then(Value::as_i64) == Some(1) {
        CreatorName::SingleField {
            name: text("lastName"),
        }
    } else {
        CreatorName::TwoField {
            first_name: text("firstName"),
            last_name: text("lastName"),
        }
    };
    Ok(Creator { creator_type, name })
}

fn write_creator(c: &Creator) -> Value {
    let mut o = Map::new();
    o.insert("creatorType".into(), c.creator_type.as_str().into());
    match &c.name {
        CreatorName::TwoField {
            first_name,
            last_name,
        } => {
            o.insert("firstName".into(), first_name.clone().into());
            o.insert("lastName".into(), last_name.clone().into());
        }
        CreatorName::SingleField { name } => {
            o.insert("name".into(), name.clone().into());
        }
    }
    Value::Object(o)
}

fn read_tag(v: &Value) -> Result<Tag, ZoteroJsonError> {
    match v {
        // Tags are sometimes written as bare strings by translators.
        Value::String(s) => Ok(Tag {
            tag: s.clone(),
            tag_type: None,
        }),
        Value::Object(o) => {
            let tag = o
                .get("tag")
                .and_then(as_text)
                .ok_or_else(|| bad("tags[].tag"))?;
            let tag_type = match o.get("type") {
                None | Some(Value::Null) => None,
                Some(t) => Some(
                    t.as_u64()
                        .or_else(|| t.as_str().and_then(|s| s.parse().ok()))
                        .and_then(|n| u8::try_from(n).ok())
                        .ok_or_else(|| bad("tags[].type"))?,
                ),
            };
            Ok(Tag { tag, tag_type })
        }
        _ => Err(bad("tags[]")),
    }
}

fn write_tag(t: &Tag) -> Value {
    let mut o = Map::new();
    o.insert("tag".into(), t.tag.clone().into());
    if let Some(ty) = t.tag_type {
        o.insert("type".into(), ty.into());
    }
    Value::Object(o)
}

fn read_relations(v: &Value) -> Result<BTreeMap<String, Vec<String>>, ZoteroJsonError> {
    let mut out = BTreeMap::new();
    match v {
        Value::Null => {}
        Value::Object(o) => {
            for (pred, objs) in o {
                let list = match objs {
                    Value::String(s) => vec![s.clone()],
                    Value::Array(a) => a
                        .iter()
                        .map(|x| {
                            x.as_str()
                                .map(str::to_owned)
                                .ok_or_else(|| bad("relations"))
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                    _ => return Err(bad("relations")),
                };
                out.insert(pred.clone(), list);
            }
        }
        // An empty relations set is sometimes written as [].
        Value::Array(a) if a.is_empty() => {}
        _ => return Err(bad("relations")),
    }
    Ok(out)
}

fn write_relations(r: &BTreeMap<String, Vec<String>>) -> Value {
    // `getRelations` always gives arrays (dataObject.js:324-338).
    Value::Object(
        r.iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    Value::Array(v.iter().cloned().map(Value::String).collect()),
                )
            })
            .collect(),
    )
}

fn read_string_list(v: &Value, p: &str) -> Result<Vec<String>, ZoteroJsonError> {
    match v {
        Value::Null => Ok(Vec::new()),
        Value::Array(a) => a
            .iter()
            .map(|x| x.as_str().map(str::to_owned).ok_or_else(|| bad(p)))
            .collect(),
        _ => Err(bad(p)),
    }
}

fn read_bool(v: &Value, p: &str) -> Result<Option<bool>, ZoteroJsonError> {
    match v {
        Value::Null => Ok(None),
        Value::Bool(b) => Ok(Some(*b)),
        Value::Number(n) => Ok(Some(n.as_i64() != Some(0))),
        _ => Err(bad(p)),
    }
}

impl ZoteroItem {
    /// An empty item of the given type, with the attachment/annotation
    /// sub-record present when the type needs it.
    pub fn new(item_type: ItemType) -> Self {
        ZoteroItem {
            key: None,
            version: None,
            item_type,
            fields: BTreeMap::new(),
            creators: Vec::new(),
            tags: Vec::new(),
            collections: Vec::new(),
            relations: BTreeMap::new(),
            parent_item: None,
            note: None,
            attachment: (item_type == ItemType::Attachment).then(AttachmentData::default),
            annotation: (item_type == ItemType::Annotation).then(AnnotationData::default),
            deleted: None,
            in_publications: None,
            date_added: None,
            date_modified: None,
            uri: None,
            attachments: Vec::new(),
            notes: Vec::new(),
            other: BTreeMap::new(),
        }
    }

    /// The value stored under `field` (exact name, no base-field mapping).
    pub fn field(&self, field: Field) -> Option<&str> {
        self.fields.get(field.as_str()).map(String::as_str)
    }

    /// Store `value` under `field` (exact name; an empty value removes it,
    /// as `setField(field, '')` does).
    pub fn set_field(&mut self, field: Field, value: impl Into<String>) {
        let value = value.into();
        if value.is_empty() {
            self.fields.remove(field.as_str());
        } else {
            self.fields.insert(field.as_str().to_owned(), value);
        }
    }

    /// The value of a field read through its **base** field, as
    /// `getField(base)` resolves it: `webpage` + `publicationTitle` reads
    /// `websiteTitle` (itemFields.js:278).
    pub fn field_via_base(&self, base: Field) -> Option<&str> {
        let f = super::schema::field_from_type_and_base(self.item_type, base)?;
        self.field(f)
    }

    /// Parse Zotero item JSON (see the module docs for what is lenient and
    /// what is an error).
    pub fn from_json_value(v: &Value) -> Result<Self, ZoteroJsonError> {
        let o = v.as_object().ok_or(ZoteroJsonError::NotAnObject)?;
        let type_name = o
            .get("itemType")
            .and_then(Value::as_str)
            .ok_or(ZoteroJsonError::MissingItemType)?;
        let item_type = ItemType::from_name(type_name)
            .ok_or_else(|| ZoteroJsonError::UnknownItemType(type_name.to_owned()))?;
        let mut item = ZoteroItem::new(item_type);
        let mut att = AttachmentData::default();
        let mut has_att = false;
        let mut ann = AnnotationData::default();
        let mut has_ann = false;
        for (k, val) in o {
            match k.as_str() {
                "itemType" => {}
                "key" => item.key = opt_string(val, k)?,
                "version" => {
                    item.version = match val {
                        Value::Null => None,
                        _ => Some(val.as_u64().ok_or_else(|| bad(k))?),
                    }
                }
                "creators" => {
                    if let Value::Array(a) = val {
                        item.creators = a.iter().map(read_creator).collect::<Result<_, _>>()?;
                    } else if !val.is_null() {
                        return Err(bad(k));
                    }
                }
                "tags" => {
                    if let Value::Array(a) = val {
                        item.tags = a.iter().map(read_tag).collect::<Result<_, _>>()?;
                    } else if !val.is_null() {
                        return Err(bad(k));
                    }
                }
                "collections" => item.collections = read_string_list(val, k)?,
                "relations" => item.relations = read_relations(val)?,
                "parentItem" => item.parent_item = key_or_false(val, k)?,
                "note" => item.note = opt_string(val, k)?,
                "deleted" => item.deleted = read_bool(val, k)?,
                "inPublications" => item.in_publications = read_bool(val, k)?,
                "dateAdded" => item.date_added = opt_string(val, k)?,
                "dateModified" => item.date_modified = opt_string(val, k)?,
                "uri" => item.uri = opt_string(val, k)?,
                "attachments" | "notes" => {
                    let list = match val {
                        Value::Array(a) => a
                            .iter()
                            .map(ZoteroItem::from_json_value)
                            .collect::<Result<Vec<_>, _>>()?,
                        Value::Null => Vec::new(),
                        _ => return Err(bad(k)),
                    };
                    if k == "attachments" {
                        item.attachments = list;
                    } else {
                        item.notes = list;
                    }
                }
                "linkMode" => {
                    has_att = true;
                    att.link_mode = match val {
                        Value::Null => None,
                        Value::String(s) => Some(
                            LinkMode::from_name(s)
                                .ok_or_else(|| ZoteroJsonError::UnknownLinkMode(s.clone()))?,
                        ),
                        // The legacy export format writes the number
                        // (utilities_internal.js:1109).
                        Value::Number(n) => Some(
                            n.as_u64()
                                .and_then(|i| LinkMode::ALL.get(i as usize).copied())
                                .ok_or_else(|| ZoteroJsonError::UnknownLinkMode(n.to_string()))?,
                        ),
                        _ => return Err(bad(k)),
                    };
                }
                "contentType" => {
                    has_att = true;
                    att.content_type = opt_string(val, k)?;
                }
                "charset" => {
                    has_att = true;
                    att.charset = opt_string(val, k)?;
                }
                "path" => {
                    has_att = true;
                    att.path = opt_string(val, k)?;
                }
                "filename" => {
                    has_att = true;
                    att.filename = opt_string(val, k)?;
                }
                "md5" => {
                    has_att = true;
                    att.md5 = opt_string(val, k)?;
                }
                "mtime" => {
                    has_att = true;
                    att.mtime = opt_i64(val, k)?;
                }
                "lastRead" => {
                    has_att = true;
                    att.last_read = opt_i64(val, k)?;
                }
                "annotationType" => {
                    has_ann = true;
                    ann.annotation_type = match val {
                        Value::Null => None,
                        Value::String(s) => Some(
                            AnnotationType::from_name(s)
                                .ok_or_else(|| ZoteroJsonError::UnknownAnnotationType(s.clone()))?,
                        ),
                        _ => return Err(bad(k)),
                    };
                }
                "annotationAuthorName"
                | "annotationText"
                | "annotationComment"
                | "annotationColor"
                | "annotationPageLabel"
                | "annotationSortIndex" => {
                    has_ann = true;
                    let s = opt_string(val, k)?;
                    match k.as_str() {
                        "annotationAuthorName" => ann.author_name = s,
                        "annotationText" => ann.text = s,
                        "annotationComment" => ann.comment = s,
                        "annotationColor" => ann.color = s,
                        "annotationPageLabel" => ann.page_label = s,
                        _ => ann.sort_index = s,
                    }
                }
                "annotationPosition" => {
                    has_ann = true;
                    ann.position = match val {
                        Value::Null => None,
                        Value::String(s) => Some(s.clone()),
                        // The reader's object form (annotations.js:164);
                        // stored as text like upstream (annotations.js:247).
                        obj @ Value::Object(_) => Some(obj.to_string()),
                        _ => return Err(bad(k)),
                    };
                }
                _ => {
                    // A field (known or not) when the value is text, as
                    // upstream's default branch (item.js:5823-5839);
                    // anything else is kept verbatim.
                    let is_field = Field::from_name(k).is_some();
                    match (is_field, as_text(val), val) {
                        (true, Some(s), _) => {
                            if !s.is_empty() {
                                item.fields.insert(k.clone(), s);
                            }
                        }
                        (false, _, Value::String(s)) => {
                            item.fields.insert(k.clone(), s.clone());
                        }
                        _ => {
                            item.other.insert(k.clone(), val.clone());
                        }
                    }
                }
            }
        }
        if has_att || item_type == ItemType::Attachment {
            item.attachment = Some(att);
        }
        if has_ann || item_type == ItemType::Annotation {
            item.annotation = Some(ann);
        }
        Ok(item)
    }

    /// Write Zotero item JSON, in the shape of `Zotero.Item#toJSON`
    /// (item.js:6008): fields with empty values omitted, `creators` for
    /// regular items, `parentItem`/attachment/annotation properties for
    /// child items, `tags`, `relations`, `collections` (when any).
    pub fn to_json_value(&self) -> Value {
        let mut o = Map::new();
        if let Some(k) = &self.key {
            o.insert("key".into(), k.clone().into());
        }
        if let Some(v) = self.version {
            o.insert("version".into(), v.into());
        }
        o.insert("itemType".into(), self.item_type.as_str().into());
        for (k, v) in &self.fields {
            if !v.is_empty() {
                o.insert(k.clone(), v.clone().into());
            }
        }
        if self.item_type.is_regular() || !self.creators.is_empty() {
            o.insert(
                "creators".into(),
                Value::Array(self.creators.iter().map(write_creator).collect()),
            );
        }
        if let Some(p) = &self.parent_item {
            o.insert("parentItem".into(), p.clone().into());
        }
        if let Some(a) = &self.attachment {
            if let Some(m) = a.link_mode {
                o.insert("linkMode".into(), m.as_str().into());
            }
            let mut put = |k: &str, v: &Option<String>| {
                if let Some(v) = v {
                    o.insert(k.into(), v.clone().into());
                }
            };
            put("contentType", &a.content_type);
            put("charset", &a.charset);
            put("path", &a.path);
            put("filename", &a.filename);
            put("md5", &a.md5);
            if let Some(m) = a.mtime {
                o.insert("mtime".into(), m.into());
            }
            if let Some(m) = a.last_read {
                o.insert("lastRead".into(), m.into());
            }
        }
        if let Some(n) = &self.note {
            o.insert("note".into(), n.clone().into());
        }
        if let Some(a) = &self.annotation {
            if let Some(t) = a.annotation_type {
                o.insert("annotationType".into(), t.as_str().into());
            }
            for (k, v) in [
                ("annotationAuthorName", &a.author_name),
                ("annotationText", &a.text),
                ("annotationComment", &a.comment),
                ("annotationColor", &a.color),
                ("annotationPageLabel", &a.page_label),
                ("annotationSortIndex", &a.sort_index),
                ("annotationPosition", &a.position),
            ] {
                if let Some(v) = v {
                    o.insert(k.into(), v.clone().into());
                }
            }
        }
        o.insert(
            "tags".into(),
            Value::Array(self.tags.iter().map(write_tag).collect()),
        );
        if !self.collections.is_empty() || self.parent_item.is_none() {
            o.insert(
                "collections".into(),
                Value::Array(
                    self.collections
                        .iter()
                        .cloned()
                        .map(Value::String)
                        .collect(),
                ),
            );
        }
        o.insert("relations".into(), write_relations(&self.relations));
        if let Some(d) = self.deleted {
            o.insert("deleted".into(), d.into());
        }
        if let Some(d) = self.in_publications {
            o.insert("inPublications".into(), d.into());
        }
        if let Some(d) = &self.date_added {
            o.insert("dateAdded".into(), d.clone().into());
        }
        if let Some(d) = &self.date_modified {
            o.insert("dateModified".into(), d.clone().into());
        }
        if let Some(u) = &self.uri {
            o.insert("uri".into(), u.clone().into());
        }
        if !self.attachments.is_empty() {
            o.insert(
                "attachments".into(),
                Value::Array(
                    self.attachments
                        .iter()
                        .map(ZoteroItem::to_json_value)
                        .collect(),
                ),
            );
        }
        if !self.notes.is_empty() {
            o.insert(
                "notes".into(),
                Value::Array(self.notes.iter().map(ZoteroItem::to_json_value).collect()),
            );
        }
        for (k, v) in &self.other {
            o.insert(k.clone(), v.clone());
        }
        Value::Object(o)
    }
}

impl serde::Serialize for ZoteroItem {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.to_json_value().serialize(s)
    }
}

impl<'de> serde::Deserialize<'de> for ZoteroItem {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = Value::deserialize(d)?;
        ZoteroItem::from_json_value(&v).map_err(serde::de::Error::custom)
    }
}

/// A Zotero collection (collection.js:826 `toJSON`, :792 `fromJSON`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoteroCollection {
    /// The 8-character object key.
    pub key: Option<String>,
    /// The object version.
    pub version: Option<u64>,
    /// The collection name (required).
    pub name: String,
    /// The parent collection's key; `None` for a top-level collection
    /// (written as `false`, as upstream does).
    pub parent_collection: Option<String>,
    /// Relations, as for items.
    pub relations: BTreeMap<String, Vec<String>>,
    /// `deleted` (in the trash).
    pub deleted: Option<bool>,
    /// Every other property, verbatim.
    pub other: BTreeMap<String, Value>,
}

impl ZoteroCollection {
    /// A top-level collection with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        ZoteroCollection {
            key: None,
            version: None,
            name: name.into(),
            parent_collection: None,
            relations: BTreeMap::new(),
            deleted: None,
            other: BTreeMap::new(),
        }
    }

    /// Parse collection JSON.
    pub fn from_json_value(v: &Value) -> Result<Self, ZoteroJsonError> {
        let o = v.as_object().ok_or(ZoteroJsonError::NotAnObject)?;
        let name = o
            .get("name")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .ok_or(ZoteroJsonError::MissingCollectionName)?;
        let mut c = ZoteroCollection::new(name);
        for (k, val) in o {
            match k.as_str() {
                "name" => {}
                "key" => c.key = opt_string(val, k)?,
                "version" => c.version = val.as_u64(),
                "parentCollection" => c.parent_collection = key_or_false(val, k)?,
                "relations" => c.relations = read_relations(val)?,
                "deleted" => c.deleted = read_bool(val, k)?,
                _ => {
                    c.other.insert(k.clone(), val.clone());
                }
            }
        }
        Ok(c)
    }

    /// Write collection JSON as `toJSON` does.
    pub fn to_json_value(&self) -> Value {
        let mut o = Map::new();
        if let Some(k) = &self.key {
            o.insert("key".into(), k.clone().into());
        }
        if let Some(v) = self.version {
            o.insert("version".into(), v.into());
        }
        o.insert("name".into(), self.name.clone().into());
        o.insert(
            "parentCollection".into(),
            match &self.parent_collection {
                Some(p) => p.clone().into(),
                None => false.into(),
            },
        );
        o.insert("relations".into(), write_relations(&self.relations));
        if let Some(d) = self.deleted {
            o.insert("deleted".into(), d.into());
        }
        for (k, v) in &self.other {
            o.insert(k.clone(), v.clone());
        }
        Value::Object(o)
    }
}

impl serde::Serialize for ZoteroCollection {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.to_json_value().serialize(s)
    }
}

impl<'de> serde::Deserialize<'de> for ZoteroCollection {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = Value::deserialize(d)?;
        ZoteroCollection::from_json_value(&v).map_err(serde::de::Error::custom)
    }
}

/// A Zotero library: its collections, its items (top-level and child items
/// side by side, children pointing at parents through `parentItem`, as the
/// Web API lists them) and its saved searches.
///
/// This is a container for import/export, not a port of `Zotero.Library`
/// (which is a database handle). Serialises as
/// `{"collections": [...], "items": [...]}`, plus `"searches": [...]` when
/// there are saved searches.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct ZoteroLibrary {
    /// Collections.
    #[serde(default)]
    pub collections: Vec<ZoteroCollection>,
    /// Items, including child notes, attachments and annotations.
    #[serde(default)]
    pub items: Vec<ZoteroItem>,
    /// Saved searches (search.js `toJSON`). Added 2026-10-07 (#750) for the
    /// Zotero database reader; omitted from the JSON when empty, so a library
    /// without searches serialises exactly as before.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub searches: Vec<super::search::ZoteroSearch>,
}

impl ZoteroLibrary {
    /// The item with this key.
    pub fn item(&self, key: &str) -> Option<&ZoteroItem> {
        self.items.iter().find(|i| i.key.as_deref() == Some(key))
    }

    /// The child items (notes, attachments, annotations) of `parent_key`.
    pub fn children(&self, parent_key: &str) -> impl Iterator<Item = &ZoteroItem> {
        let p = parent_key.to_owned();
        self.items
            .iter()
            .filter(move |i| i.parent_item.as_deref() == Some(p.as_str()))
    }

    /// The items in the collection with key `collection_key`.
    pub fn items_in_collection(&self, collection_key: &str) -> impl Iterator<Item = &ZoteroItem> {
        let c = collection_key.to_owned();
        self.items
            .iter()
            .filter(move |i| i.collections.contains(&c))
    }

    /// The child collections of `parent_key` (`None` for top-level ones).
    pub fn subcollections(
        &self,
        parent_key: Option<&str>,
    ) -> impl Iterator<Item = &ZoteroCollection> {
        let p = parent_key.map(str::to_owned);
        self.collections
            .iter()
            .filter(move |c| c.parent_collection == p)
    }
}
