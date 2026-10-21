// Part of the kovan Zotero port (GitHub #747, #751). Kovan's own accounting
// over kovan-common's port of the Zotero data model and its KovanDocument
// conversion; no upstream code.

//! [`import_library`] and its [`ImportReport`] (see the module docs of
//! [`super`] for the policy and how losses are measured).

use std::collections::{BTreeMap, BTreeSet};

use kovan_common::zotero::schema::{base_from_type_and_field, field_from_type_and_base};
use kovan_common::zotero::{Creator, CreatorName, Field, ZoteroItem, ZoteroLibrary};
use kovan_common::{DocumentType, KovanDocument};
use serde_json::Value;

use super::md_cell;

/// Why an item was not imported.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum SkipReason {
    /// A child note, attachment or annotation (`parentItem` set); not
    /// preserved in any kovan document.
    ChildItem {
        /// The parent's key.
        parent: String,
    },
    /// A standalone note, attachment or annotation: not a bibliographic item.
    NotRegular,
    /// In the trash (`deleted: true`).
    Trashed,
    /// No `key`, so no stable kovan id.
    MissingKey,
    /// An item with the same key was already imported.
    DuplicateKey,
}

impl SkipReason {
    fn describe(&self) -> String {
        match self {
            SkipReason::ChildItem { parent } => format!("child of {parent}"),
            SkipReason::NotRegular => "standalone note/attachment/annotation".into(),
            SkipReason::Trashed => "in the trash".into(),
            SkipReason::MissingKey => "no key".into(),
            SkipReason::DuplicateKey => "duplicate key".into(),
        }
    }
}

/// One imported item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedItem {
    /// The Zotero key.
    pub key: String,
    /// The Zotero item type name.
    pub item_type: String,
    /// The kovan document id (`zotero:<key>`).
    pub id: String,
    /// The kovan slug.
    pub slug: String,
    /// The kovan document type.
    pub document_type: DocumentType,
    /// The title.
    pub title: String,
}

/// One item that was not imported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedItem {
    /// The Zotero key, if any.
    pub key: Option<String>,
    /// The Zotero item type name.
    pub item_type: String,
    /// Why.
    pub reason: SkipReason,
}

/// How a property fared in the Zotero -> kovan -> Zotero round trip
/// through kovan's own fields.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum LossKind {
    /// Absent after the round trip.
    Dropped,
    /// Present with a different value.
    Altered {
        /// The value after the round trip (JSON text for non-strings).
        kovan: String,
    },
    /// Kept only as a `Name: value` line of Extra.
    MovedToExtra,
}

impl LossKind {
    fn label(&self) -> &'static str {
        match self {
            LossKind::Dropped => "dropped",
            LossKind::Altered { .. } => "altered",
            LossKind::MovedToExtra => "moved to Extra",
        }
    }
}

/// One property of one imported item that kovan's own fields do not carry.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FieldLoss {
    /// The item key.
    pub key: String,
    /// The JSON property (`date`, `ISSN`, `creators`, `relations`, ...).
    pub property: String,
    /// The original value (JSON text for non-strings; for a creator,
    /// `type: Last, First`).
    pub original: String,
    /// What happened to it.
    pub kind: LossKind,
}

/// The result of [`import_library`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImportReport {
    /// Every item of `ZoteroLibrary::items` considered (nested export-format
    /// children travel with their parent and are not counted).
    pub considered: usize,
    /// Imported items, sorted by key.
    pub imported: Vec<ImportedItem>,
    /// Skipped items, sorted by key (keyless last), item type, reason.
    pub skipped: Vec<SkippedItem>,
    /// Losses, sorted by key, property, original value.
    pub losses: Vec<FieldLoss>,
}

impl ImportReport {
    /// Property -> (dropped, altered, moved to Extra) item counts.
    pub fn losses_by_property(&self) -> BTreeMap<String, [usize; 3]> {
        let mut m: BTreeMap<String, [usize; 3]> = BTreeMap::new();
        // Count items, not entries: an item losing three creators counts once.
        let mut seen: BTreeSet<(&str, &str, usize)> = BTreeSet::new();
        for l in &self.losses {
            let i = match l.kind {
                LossKind::Dropped => 0,
                LossKind::Altered { .. } => 1,
                LossKind::MovedToExtra => 2,
            };
            if seen.insert((l.key.as_str(), l.property.as_str(), i)) {
                m.entry(l.property.clone()).or_default()[i] += 1;
            }
        }
        m
    }

    /// The report as Markdown.
    pub fn to_markdown(&self) -> String {
        let mut s = String::new();
        s.push_str("# Zotero import report\n\n");
        s.push_str(
            "\"Lossy\" means not carried by kovan's own document fields, measured by a \
             Zotero -> kovan -> Zotero round trip without `zotero_item`. Every imported \
             item is also kept verbatim in `zotero_item`, so the full round trip is \
             lossless. Skipped items are not preserved.\n\n",
        );
        s.push_str("| Considered | Imported | Skipped | Lossy properties (entries) |\n");
        s.push_str("|---:|---:|---:|---:|\n");
        s.push_str(&format!(
            "| {} | {} | {} | {} |\n\n",
            self.considered,
            self.imported.len(),
            self.skipped.len(),
            self.losses.len()
        ));
        s.push_str("## Imported\n\n");
        if self.imported.is_empty() {
            s.push_str("None.\n\n");
        } else {
            s.push_str("| Key | Item type | Kovan id | Document type | Title |\n");
            s.push_str("|---|---|---|---|---|\n");
            for i in &self.imported {
                s.push_str(&format!(
                    "| {} | {} | {} | {:?} | {} |\n",
                    md_cell(&i.key, 20),
                    i.item_type,
                    md_cell(&i.id, 40),
                    i.document_type,
                    md_cell(&i.title, 80)
                ));
            }
            s.push('\n');
        }
        s.push_str("## Skipped\n\n");
        if self.skipped.is_empty() {
            s.push_str("None.\n\n");
        } else {
            s.push_str("| Key | Item type | Reason |\n|---|---|---|\n");
            for i in &self.skipped {
                s.push_str(&format!(
                    "| {} | {} | {} |\n",
                    md_cell(i.key.as_deref().unwrap_or("(no key)"), 20),
                    i.item_type,
                    md_cell(&i.reason.describe(), 60)
                ));
            }
            s.push('\n');
        }
        s.push_str("## Lossy, by property (items)\n\n");
        let by = self.losses_by_property();
        if by.is_empty() {
            s.push_str("None.\n\n");
        } else {
            s.push_str("| Property | Dropped | Altered | Moved to Extra |\n");
            s.push_str("|---|---:|---:|---:|\n");
            for (p, [d, a, x]) in &by {
                s.push_str(&format!("| {} | {d} | {a} | {x} |\n", md_cell(p, 40)));
            }
            s.push('\n');
        }
        s.push_str("## Lossy, per item\n\n");
        if self.losses.is_empty() {
            s.push_str("None.\n");
        } else {
            s.push_str("| Key | Property | Loss | Zotero value | After round trip |\n");
            s.push_str("|---|---|---|---|---|\n");
            for l in &self.losses {
                let after = match &l.kind {
                    LossKind::Altered { kovan } => md_cell(kovan, 60),
                    _ => String::new(),
                };
                s.push_str(&format!(
                    "| {} | {} | {} | {} | {} |\n",
                    md_cell(&l.key, 20),
                    md_cell(&l.property, 40),
                    l.kind.label(),
                    md_cell(&l.original, 60),
                    after
                ));
            }
        }
        s
    }
}

/// Convert every importable item of `lib` to a [`KovanDocument`]
/// (`ZoteroItem::to_kovan_document`) and account for it. Documents are
/// returned in the order of [`ImportReport::imported`] (sorted by key).
pub fn import_library(lib: &ZoteroLibrary) -> (Vec<KovanDocument>, ImportReport) {
    let mut report = ImportReport::default();
    let mut docs: BTreeMap<String, KovanDocument> = BTreeMap::new();
    for item in &lib.items {
        report.considered += 1;
        let skip = |reason: SkipReason| SkippedItem {
            key: item.key.clone(),
            item_type: item.item_type.as_str().to_owned(),
            reason,
        };
        let reason = if let Some(p) = &item.parent_item {
            Some(SkipReason::ChildItem { parent: p.clone() })
        } else if !item.item_type.is_regular() {
            Some(SkipReason::NotRegular)
        } else if item.deleted == Some(true) {
            Some(SkipReason::Trashed)
        } else {
            match item.key.as_deref() {
                None | Some("") => Some(SkipReason::MissingKey),
                Some(k) if docs.contains_key(k) => Some(SkipReason::DuplicateKey),
                Some(_) => None,
            }
        };
        if let Some(r) = reason {
            report.skipped.push(skip(r));
            continue;
        }
        let key = item.key.clone().unwrap_or_default();
        let doc = item.to_kovan_document();
        report.imported.push(ImportedItem {
            key: key.clone(),
            item_type: item.item_type.as_str().to_owned(),
            id: doc.id.clone(),
            slug: doc.slug.clone(),
            document_type: doc.document_type,
            title: doc.title.clone(),
        });
        report.losses.extend(losses_of(item, &doc, &key));
        docs.insert(key, doc);
    }
    report.imported.sort_by(|a, b| a.key.cmp(&b.key));
    report.skipped.sort_by(|a, b| {
        (a.key.is_none(), &a.key, &a.item_type, &a.reason).cmp(&(
            b.key.is_none(),
            &b.key,
            &b.item_type,
            &b.reason,
        ))
    });
    report.losses.sort();
    (docs.into_values().collect(), report)
}

fn json_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn is_empty_json(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.is_empty(),
        _ => false,
    }
}

fn creator_text(c: &Creator) -> String {
    let name = match &c.name {
        CreatorName::TwoField {
            first_name,
            last_name,
        } => format!("{last_name}, {first_name}"),
        CreatorName::SingleField { name } => name.clone(),
    };
    format!("{}: {name}", c.creator_type.as_str())
}

/// The values of the `Name: value` lines of an Extra field.
fn extra_values(extra: Option<&str>) -> BTreeSet<String> {
    extra
        .unwrap_or("")
        .lines()
        .filter_map(|l| l.split_once(':').map(|(_, v)| v.trim().to_owned()))
        .collect()
}

/// The properties of `item` that `doc`'s own fields do not carry.
fn losses_of(item: &ZoteroItem, doc: &KovanDocument, key: &str) -> Vec<FieldLoss> {
    let mut bare = doc.clone();
    bare.zotero_item = None;
    let back = ZoteroItem::from_kovan_document(&bare);
    let orig_json = item.to_json_value();
    let back_json = back.to_json_value();
    let empty = serde_json::Map::new();
    let (o, b) = (
        orig_json.as_object().unwrap_or(&empty),
        back_json.as_object().unwrap_or(&empty),
    );
    let extra_back = extra_values(back.field(Field::Extra));
    let mut out = Vec::new();
    let mut push = |property: &str, original: String, kind: LossKind| {
        out.push(FieldLoss {
            key: key.to_owned(),
            property: property.to_owned(),
            original,
            kind,
        })
    };
    for (prop, v) in o {
        if is_empty_json(v) || prop == "key" {
            continue;
        }
        match prop.as_str() {
            "creators" => {
                let mut remaining: Vec<&Creator> = back.creators.iter().collect();
                for c in &item.creators {
                    if let Some(i) = remaining.iter().position(|r| *r == c) {
                        remaining.remove(i);
                    } else if let Some(i) = remaining.iter().position(|r| r.name == c.name) {
                        let r = remaining.remove(i);
                        push(
                            "creators",
                            creator_text(c),
                            LossKind::Altered {
                                kovan: creator_text(r),
                            },
                        );
                    } else {
                        push("creators", creator_text(c), LossKind::Dropped);
                    }
                }
            }
            "tags" => {
                let set = |i: &ZoteroItem| -> BTreeSet<(String, bool)> {
                    i.tags
                        .iter()
                        .map(|t| (t.tag.clone(), t.is_automatic()))
                        .collect()
                };
                if set(item) != set(&back) {
                    let after = b.get("tags").cloned().unwrap_or(Value::Null);
                    push(
                        "tags",
                        json_text(v),
                        if is_empty_json(&after) {
                            LossKind::Dropped
                        } else {
                            LossKind::Altered {
                                kovan: json_text(&after),
                            }
                        },
                    );
                }
            }
            _ => {
                // A schema field: compare through its base field.
                let after: Option<Value> = match Field::from_name(prop)
                    .filter(|_| item.fields.contains_key(prop.as_str()))
                {
                    Some(f) => {
                        let base = base_from_type_and_field(item.item_type, f).unwrap_or(f);
                        field_from_type_and_base(back.item_type, base)
                            .and_then(|bf| back.field(bf))
                            .or_else(|| back.field(f))
                            .map(|s| Value::String(s.to_owned()))
                    }
                    None => b.get(prop).cloned(),
                };
                match after {
                    Some(a) if a == *v => {}
                    Some(a) if !is_empty_json(&a) => push(
                        prop,
                        json_text(v),
                        LossKind::Altered {
                            kovan: json_text(&a),
                        },
                    ),
                    _ => {
                        let moved = prop != "extra"
                            && v.is_string()
                            && extra_back.contains(json_text(v).trim());
                        push(
                            prop,
                            json_text(v),
                            if moved {
                                LossKind::MovedToExtra
                            } else {
                                LossKind::Dropped
                            },
                        )
                    }
                }
            }
        }
    }
    out
}
