// Part of the kovan Zotero port (GitHub #747, #748).
//
// Upstream: Zotero, https://github.com/zotero/zotero (commit 9cbba8c4d281):
//   chrome/content/zotero/xpcom/data/item.js (`fromJSON` :5656-5963, the
//   field branch and the Extra clean-up), xpcom/utilities_internal.js
//   (`combineExtraFields` :1387, `_normalizeExtraKey` :1430,
//   `camelToTitleCase` :1607).
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! Checking an item against the schema, and Zotero's import-time repair of
//! fields that do not belong to the item type.

use super::item::ZoteroItem;
use super::schema::{
    base_from_type_and_field, field_from_type_and_base, is_valid_creator_type, is_valid_for_type,
    type_fields_from_base,
};
use super::schema_generated::{CreatorType, Field, ItemType};
use regex::Regex;
use std::sync::OnceLock;

/// One way an item does not match the schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationIssue {
    /// A property name that is no Zotero field.
    UnknownField(String),
    /// A field that the item type does not have. `mapped` is the
    /// type-specific field the value belongs in when `field` is a base field
    /// with a mapping for this type (e.g. `publicationTitle` on a `webpage`
    /// belongs in `websiteTitle`).
    FieldNotValidForType {
        /// The field as written.
        field: Field,
        /// The type-specific field to use instead, if any.
        mapped: Option<Field>,
    },
    /// A creator whose type is not valid for the item type.
    CreatorTypeNotValidForType(CreatorType),
    /// Creators on a note, attachment or annotation.
    CreatorsOnNonRegularItem,
    /// Attachment properties on an item that is not an attachment.
    AttachmentDataOnNonAttachment,
    /// Annotation properties on an item that is not an annotation.
    AnnotationDataOnNonAnnotation,
    /// An annotation without `parentItem`.
    AnnotationWithoutParent,
}

impl ZoteroItem {
    /// Every way this item departs from the schema (empty when it matches).
    ///
    /// Field validity is `Zotero.ItemFields.isValidForType` (itemFields.js:170)
    /// after the base-field mapping `fromJSON` applies (item.js:5845);
    /// creator validity is `Zotero.CreatorTypes.isValidForItemType`
    /// (cachedTypes.js:288).
    pub fn validate(&self) -> Vec<ValidationIssue> {
        let mut out = Vec::new();
        for name in self.fields.keys() {
            match Field::from_name(name) {
                None => out.push(ValidationIssue::UnknownField(name.clone())),
                Some(f) if !is_valid_for_type(f, self.item_type) => {
                    let mapped = field_from_type_and_base(self.item_type, f).filter(|m| *m != f);
                    out.push(ValidationIssue::FieldNotValidForType { field: f, mapped });
                }
                Some(_) => {}
            }
        }
        if !self.creators.is_empty() && !self.item_type.is_regular() {
            out.push(ValidationIssue::CreatorsOnNonRegularItem);
        } else {
            for c in &self.creators {
                if !is_valid_creator_type(c.creator_type, self.item_type) {
                    out.push(ValidationIssue::CreatorTypeNotValidForType(c.creator_type));
                }
            }
        }
        if self.attachment.is_some() && self.item_type != ItemType::Attachment {
            out.push(ValidationIssue::AttachmentDataOnNonAttachment);
        }
        if self.annotation.is_some() && self.item_type != ItemType::Annotation {
            out.push(ValidationIssue::AnnotationDataOnNonAnnotation);
        }
        if self.item_type == ItemType::Annotation && self.parent_item.is_none() {
            out.push(ValidationIssue::AnnotationWithoutParent);
        }
        out
    }

    /// Repair the fields the way non-strict `Zotero.Item#fromJSON` does on
    /// import (item.js:5823-5963):
    ///
    /// 1. a base field with a type-specific mapping moves to it
    ///    (`publicationTitle` on a `bookSection` becomes `bookTitle`,
    ///    item.js:5845; upstream test itemTest.js:3380);
    /// 2. a field invalid for the type, and an unknown field, moves into
    ///    Extra as `Title Case Name: value` (item.js:5850-5866, 5833-5836);
    /// 3. before that, Extra candidates are de-duplicated as upstream does:
    ///    base-mapped siblings of a field that was set, mapped fields equal to
    ///    an invalid base field, `type`-mapped fields and `audioFileType`,
    ///    and the RDF double assignments (item.js:5877-5955);
    /// 4. Extra is rebuilt with `combineExtraFields` (utilities_internal.js:1387).
    ///
    /// Not ported: parsing *existing* Extra lines into real fields
    /// (`extractExtraFields`), which upstream runs first (item.js:5679-5699).
    pub fn normalize_fields(&mut self) {
        let it = self.item_type;
        let original = std::mem::take(&mut self.fields);
        let extra_in = original.get("extra").cloned().unwrap_or_default();
        let mut set: Vec<(Field, String)> = Vec::new();
        // Insertion-ordered (name, value) pairs moving to Extra.
        let mut extra_fields: Vec<(String, String)> = Vec::new();
        let put_extra = |list: &mut Vec<(String, String)>, k: String, v: String| {
            if let Some(slot) = list.iter_mut().find(|(n, _)| *n == k) {
                slot.1 = v;
            } else {
                list.push((k, v));
            }
        };
        for (name, value) in &original {
            if name == "extra" {
                continue;
            }
            let Some(f) = Field::from_name(name) else {
                put_extra(&mut extra_fields, name.clone(), value.clone());
                continue;
            };
            let target = field_from_type_and_base(it, f).unwrap_or(f);
            if !is_valid_for_type(target, it) {
                put_extra(&mut extra_fields, target.as_str().to_owned(), value.clone());
                continue;
            }
            set.push((target, value.clone()));
        }
        if !extra_fields.is_empty() {
            let set_names: Vec<Field> = set.iter().map(|(f, _)| *f).collect();
            for f in &set_names {
                if let Some(base) = base_from_type_and_field(it, *f) {
                    for mapped in type_fields_from_base(base) {
                        extra_fields.retain(|(n, _)| n != mapped.as_str());
                    }
                }
            }
            let bases: Vec<(Field, String)> = extra_fields
                .iter()
                .filter_map(|(n, v)| {
                    Field::from_name(n)
                        .filter(|f| f.is_base_field())
                        .map(|f| (f, v.clone()))
                })
                .collect();
            for (base, value) in bases {
                for mapped in type_fields_from_base(base) {
                    extra_fields.retain(|(n, v)| !(n == mapped.as_str() && *v == value));
                }
            }
            let mut type_fields: Vec<String> = type_fields_from_base(Field::Type)
                .iter()
                .map(|f| f.as_str().to_owned())
                .collect();
            type_fields.push("audioFileType".into());
            extra_fields.retain(|(n, _)| !type_fields.contains(n));
            const RDF_FIXES: [(&str, &str); 11] = [
                ("versionNumber", "edition"),
                ("conferenceName", "meetingName"),
                ("publicationTitle", "reporter"),
                ("bookTitle", "reporter"),
                ("blogTitle", "reporter"),
                ("dictionaryTitle", "reporter"),
                ("encyclopediaTitle", "reporter"),
                ("forumTitle", "reporter"),
                ("proceedingsTitle", "reporter"),
                ("programTitle", "reporter"),
                ("websiteTitle", "reporter"),
            ];
            let get_set = |name: &str| {
                set.iter()
                    .find(|(f, _)| f.as_str() == name)
                    .map(|(_, v)| v.clone())
            };
            for (a, b) in RDF_FIXES {
                if let Some(sv) = get_set(b) {
                    extra_fields.retain(|(n, v)| !(n == a && *v == sv));
                }
                if let Some(sv) = get_set(a) {
                    extra_fields.retain(|(n, v)| !(n == b && *v == sv));
                }
            }
        }
        for (f, v) in set {
            self.fields.insert(f.as_str().to_owned(), v);
        }
        if !extra_in.is_empty() || !extra_fields.is_empty() {
            let extra = combine_extra_fields(&extra_in, extra_fields);
            if !extra.is_empty() {
                self.fields.insert("extra".into(), extra);
            }
        }
    }
}

/// `_normalizeExtraKey` (utilities_internal.js:1430).
fn normalize_extra_key(key: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = key.trim().chars().collect();
    for (i, c) in chars.iter().enumerate() {
        out.push(*c);
        if c.is_ascii_lowercase() && chars.get(i + 1).is_some_and(|n| n.is_ascii_uppercase()) {
            out.push('-');
        }
    }
    out.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_whitespace() || c == '-' || c == '_' {
                '-'
            } else {
                c
            }
        })
        .collect()
}

/// `Zotero.Utilities.Internal.camelToTitleCase` (utilities_internal.js:1607):
/// `publicationTitle` -> `Publication Title`.
pub fn camel_to_title_case(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    for (i, c) in chars.iter().enumerate() {
        if i == 0 {
            out.extend(c.to_uppercase());
        } else {
            out.push(*c);
        }
        if c.is_ascii_lowercase() && chars.get(i + 1).is_some_and(|n| n.is_ascii_uppercase()) {
            out.push(' ');
        }
    }
    out
}

/// `Zotero.Utilities.Internal.combineExtraFields` (utilities_internal.js:1387):
/// update `Key: value` lines of `extra` that name a field in `fields`, and
/// prepend the rest as sorted `Title Case: value` lines.
pub fn combine_extra_fields(extra: &str, mut fields: Vec<(String, String)>) -> String {
    static LINE: OnceLock<Regex> = OnceLock::new();
    static CHEATER: OnceLock<Regex> = OnceLock::new();
    // `[a-z -_]` in upstream is the range U+0020..U+005F plus a-z.
    let line_re = LINE.get_or_init(|| Regex::new(r"(?i)^([a-z\x20-\x5F]+):(.+)").expect("regex"));
    let cheater_re =
        CHEATER.get_or_init(|| Regex::new(r"(?i)^\{:([a-z\x20-\x5F]+):(.+)\}").expect("regex"));
    let normalized: Vec<(String, String, String)> = fields
        .iter()
        .map(|(k, v)| (normalize_extra_key(k), k.clone(), v.clone()))
        .collect();
    let mut keep: Vec<String> = Vec::new();
    let lines: Vec<&str> = if extra.is_empty() {
        Vec::new()
    } else {
        extra.split('\n').collect()
    };
    for line in lines {
        let caps = line_re.captures(line).or_else(|| cheater_re.captures(line));
        let Some(caps) = caps else {
            keep.push(line.to_owned());
            continue;
        };
        let original = &caps[1];
        let key = normalize_extra_key(original);
        // The LAST field with this normalised key wins (Map#set overwrites).
        if let Some((_, orig_key, value)) = normalized.iter().rev().find(|(n, _, _)| *n == key) {
            keep.push(format!("{original}: {value}"));
            fields.retain(|(k, _)| k != orig_key);
        } else {
            keep.push(line.to_owned());
        }
    }
    let mut pairs: Vec<String> = fields
        .iter()
        .map(|(k, v)| format!("{}: {v}", camel_to_title_case(k)))
        .collect();
    // JavaScript's default sort compares UTF-16 code units; for the ASCII
    // labels Zotero writes this is byte order.
    pairs.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    let mut out = pairs.join("\n");
    if !pairs.is_empty() && !keep.is_empty() {
        out.push('\n');
    }
    out.push_str(&keep.join("\n"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zotero::item::Creator;

    /// itemTest.js:3380: `publicationTitle` on a bookSection is stored as `bookTitle`.
    #[test]
    fn base_field_moves_to_type_specific_field() {
        let mut item = ZoteroItem::new(ItemType::BookSection);
        item.fields
            .insert("publicationTitle".into(), "Publication Title".into());
        assert_eq!(
            item.validate(),
            vec![ValidationIssue::FieldNotValidForType {
                field: Field::PublicationTitle,
                mapped: Some(Field::BookTitle)
            }]
        );
        item.normalize_fields();
        assert_eq!(item.field(Field::BookTitle), Some("Publication Title"));
        assert!(item.validate().is_empty());
    }

    #[test]
    fn invalid_and_unknown_fields_go_to_extra() {
        let mut item = ZoteroItem::new(ItemType::Book);
        item.fields.insert("title".into(), "T".into());
        item.fields.insert("websiteTitle".into(), "Site".into());
        item.fields.insert("fooBar".into(), "x".into());
        item.fields.insert("extra".into(), "Original line".into());
        item.normalize_fields();
        assert_eq!(item.field(Field::Title), Some("T"));
        assert_eq!(
            item.field(Field::Extra),
            Some("Foo Bar: x\nWebsite Title: Site\nOriginal line")
        );
    }

    #[test]
    fn extra_line_for_same_key_is_updated_in_place() {
        let out = combine_extra_fields("DOI: old\nnote", vec![("DOI".into(), "10.1/new".into())]);
        assert_eq!(out, "DOI: 10.1/new\nnote");
    }

    #[test]
    fn creator_type_checks() {
        let mut item = ZoteroItem::new(ItemType::Webpage);
        item.creators
            .push(Creator::person(CreatorType::Author, "A", "B"));
        item.creators
            .push(Creator::person(CreatorType::Director, "C", "D"));
        assert_eq!(
            item.validate(),
            vec![ValidationIssue::CreatorTypeNotValidForType(
                CreatorType::Director
            )]
        );
    }

    #[test]
    fn title_case() {
        assert_eq!(camel_to_title_case("publicationTitle"), "Publication Title");
        assert_eq!(camel_to_title_case("DOI"), "DOI");
        assert_eq!(
            normalize_extra_key("Publication Title"),
            "publication-title"
        );
        assert_eq!(normalize_extra_key("publicationTitle"), "publication-title");
    }
}
