// Part of the kovan Zotero port (GitHub #747, #748).
//
// Upstream: Zotero, https://github.com/zotero/zotero (commit 9cbba8c4d281),
//   chrome/content/zotero/xpcom/data/itemFields.js (Zotero.ItemFields) and
//   chrome/content/zotero/xpcom/data/cachedTypes.js (Zotero.CreatorTypes,
//   Zotero.ItemTypes); and https://github.com/zotero/utilities (commit
//   4051881d59c6), cachedTypes.js and schema.js.
// Copyright (c) 2009 Center for History and New Media, George Mason University;
// (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (Zotero is "GNU Affero General Public License, version 3
// or later"; this crate is AGPL-3.0-only).

//! Schema queries: the port of `Zotero.ItemFields`, `Zotero.ItemTypes` and
//! `Zotero.CreatorTypes` over the generated tables.
//!
//! Zotero keys these by database ids; the port keys them by the generated
//! enums instead (the ids are an artefact of a local SQLite database and are
//! not part of the JSON formats). Where an upstream function takes "an id or a
//! name", the port takes the enum and offers `from_name` for the string.

use super::schema_generated::{
    CreatorType, Field, ItemType, CSL_DATE_FIELDS, CSL_NAMES, CSL_TEXT_FIELDS, CSL_TYPES,
    EN_US_CREATOR_TYPE_LABELS, EN_US_FIELD_LABELS, EN_US_ITEM_TYPE_LABELS, ITEM_TYPE_SCHEMAS,
    META_FIELD_TYPES,
};

/// One field of an item type, with the base field it maps to.
///
/// `websiteTitle` on `webpage` has `base_field = Some(publicationTitle)`:
/// Zotero stores the value under the type-specific name and reads it back
/// through the base name (see [`field_from_type_and_base`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemTypeField {
    /// The field as it appears in this item type's JSON.
    pub field: Field,
    /// The base field it is mapped to, if any.
    pub base_field: Option<Field>,
}

/// The schema of one item type: its fields in display order, its creator
/// types and its primary creator type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemTypeSchema {
    /// The item type.
    pub item_type: ItemType,
    /// Its fields, in schema (display) order.
    pub fields: &'static [ItemTypeField],
    /// Its creator types, in schema order.
    pub creator_types: &'static [CreatorType],
    /// The creator type marked `primary` (`None` for note, attachment and
    /// annotation, which take no creators).
    pub primary_creator_type: Option<CreatorType>,
}

impl ItemType {
    /// The schema entry of this item type.
    pub fn schema(self) -> &'static ItemTypeSchema {
        // ITEM_TYPE_SCHEMAS is in the same order as ItemType::ALL.
        &ITEM_TYPE_SCHEMAS[self as usize]
    }

    /// The fields valid for this item type, in display order
    /// (`Zotero.ItemFields.getItemTypeFields`, itemFields.js:218).
    pub fn fields(self) -> impl Iterator<Item = Field> {
        self.schema().fields.iter().map(|f| f.field)
    }

    /// The en-US label, e.g. "Journal Article".
    pub fn label(self) -> &'static str {
        EN_US_ITEM_TYPE_LABELS
            .iter()
            .find(|(t, _)| *t == self)
            .map(|(_, l)| *l)
            .unwrap_or(self.as_str())
    }

    /// The CSL type this item type exports as (`Zotero.Schema.CSL_TYPE_MAPPINGS`,
    /// utilities schema.js:39-44). `None` for `annotation`, which has no CSL
    /// type; upstream `itemToCSLJSON` throws for it.
    pub fn csl_type(self) -> Option<&'static str> {
        // Later CSL types overwrite earlier ones in upstream's loop; walk in
        // reverse and take the first hit to get the same answer.
        CSL_TYPES
            .iter()
            .rev()
            .find(|(_, types)| types.contains(&self))
            .map(|(csl, _)| *csl)
    }

    /// Whether this is a "regular" item (not a note, attachment or annotation).
    pub fn is_regular(self) -> bool {
        !matches!(
            self,
            ItemType::Note | ItemType::Attachment | ItemType::Annotation
        )
    }
}

impl Field {
    /// The en-US label, e.g. "Publication".
    pub fn label(self) -> &'static str {
        let name = self.as_str();
        EN_US_FIELD_LABELS
            .iter()
            .find(|(f, _)| *f == name)
            .map(|(_, l)| *l)
            .unwrap_or(name)
    }

    /// Whether this field is a base field of at least one item type
    /// (`Zotero.ItemFields.isBaseField`, itemFields.js:237; upstream computes
    /// it as "appears as a `baseFieldID` in baseFieldMappings", itemFields.js:76).
    pub fn is_base_field(self) -> bool {
        ITEM_TYPE_SCHEMAS
            .iter()
            .any(|s| s.fields.iter().any(|f| f.base_field == Some(self)))
    }

    /// Whether this field holds a date (`Zotero.ItemFields.isDate`,
    /// itemFields.js:190): its own `meta.fields` type, or else the type of the
    /// base field it is mapped to.
    pub fn is_date(self) -> bool {
        if let Some((_, ty)) = META_FIELD_TYPES.iter().find(|(f, _)| *f == self) {
            return *ty == "date";
        }
        for s in ITEM_TYPE_SCHEMAS.iter() {
            for f in s.fields {
                if f.field == self {
                    if let Some(base) = f.base_field {
                        return META_FIELD_TYPES
                            .iter()
                            .any(|(m, ty)| *m == base && *ty == "date");
                    }
                }
            }
        }
        false
    }
}

impl CreatorType {
    /// The en-US label, e.g. "Author".
    pub fn label(self) -> &'static str {
        EN_US_CREATOR_TYPE_LABELS
            .iter()
            .find(|(c, _)| *c == self)
            .map(|(_, l)| *l)
            .unwrap_or(self.as_str())
    }

    /// The CSL name variable this creator type maps to (`csl.names`), or
    /// `None` when the schema maps it to none.
    pub fn csl_name(self) -> Option<&'static str> {
        CSL_NAMES.iter().find(|(c, _)| *c == self).map(|(_, v)| *v)
    }
}

/// Whether `field` is valid for `item_type`
/// (`Zotero.ItemFields.isValidForType`, itemFields.js:170).
pub fn is_valid_for_type(field: Field, item_type: ItemType) -> bool {
    item_type.schema().fields.iter().any(|f| f.field == field)
}

/// The type-specific field of `item_type` for `base_field`
/// (`Zotero.ItemFields.getFieldIDFromTypeAndBase`, itemFields.js:278).
///
/// * `webpage` + `publicationTitle` -> `websiteTitle`;
/// * `book` + `publisher` -> `publisher` (a base field valid for the type
///   maps to itself, itemFields.js:552);
/// * `audioRecording` + `number` -> `None`;
/// * a field that is not a base field is returned when valid for the type
///   (itemFields.js:290).
pub fn field_from_type_and_base(item_type: ItemType, base_field: Field) -> Option<Field> {
    if !base_field.is_base_field() {
        return is_valid_for_type(base_field, item_type).then_some(base_field);
    }
    let schema = item_type.schema();
    if let Some(f) = schema
        .fields
        .iter()
        .find(|f| f.base_field == Some(base_field))
    {
        return Some(f.field);
    }
    is_valid_for_type(base_field, item_type).then_some(base_field)
}

/// The base field of `field` within `item_type`
/// (`Zotero.ItemFields.getBaseIDFromTypeAndField`, itemFields.js:311).
///
/// A base field valid for the type maps to itself; `audioRecording` +
/// `label` -> `publisher`; a field with no mapping -> `None`.
pub fn base_from_type_and_field(item_type: ItemType, field: Field) -> Option<Field> {
    let schema = item_type.schema();
    // Upstream tests `_baseTypeFields[itemTypeID][typeFieldID]`, which is
    // truthy for a base field that is valid for the type OR has a
    // type-specific mapping in it (itemFields.js:322, 548-555).
    if field.is_base_field() && field_from_type_and_base(item_type, field).is_some() {
        return Some(field);
    }
    schema
        .fields
        .iter()
        .find(|f| f.field == field)
        .and_then(|f| f.base_field)
}

/// Every type-specific field mapped to `base_field`, across all item types,
/// without duplicates, in schema order (`Zotero.ItemFields.getTypeFieldsFromBase`,
/// itemFields.js:336). Empty when `base_field` is not a base field (upstream
/// returns `false`).
pub fn type_fields_from_base(base_field: Field) -> Vec<Field> {
    let mut out: Vec<Field> = Vec::new();
    for s in ITEM_TYPE_SCHEMAS.iter() {
        for f in s.fields {
            if f.base_field == Some(base_field) && !out.contains(&f.field) {
                out.push(f.field);
            }
        }
    }
    out
}

/// Whether `creator_type` is valid for `item_type`
/// (`Zotero.CreatorTypes.isValidForItemType`, cachedTypes.js:288).
pub fn is_valid_creator_type(creator_type: CreatorType, item_type: ItemType) -> bool {
    item_type.schema().creator_types.contains(&creator_type)
}

/// The primary creator type of `item_type`
/// (`Zotero.CreatorTypes.getPrimaryIDForType`).
pub fn primary_creator_type(item_type: ItemType) -> Option<CreatorType> {
    item_type.schema().primary_creator_type
}

/// The Zotero item types a CSL type imports as, first being the default
/// (`Zotero.Schema.CSL_TYPE_MAPPINGS_REVERSE`, utilities schema.js:43).
pub fn item_types_for_csl_type(csl_type: &str) -> Option<&'static [ItemType]> {
    CSL_TYPES
        .iter()
        .find(|(c, _)| *c == csl_type)
        .map(|(_, t)| *t)
}

/// The CSL variable a Zotero field maps to
/// (`Zotero.Schema.CSL_FIELD_MAPPINGS_REVERSE`, utilities schema.js:48-57):
/// text variables first, then date variables, a later mapping overwriting an
/// earlier one exactly as upstream's object assignment does.
pub fn csl_variable_for_field(field: Field) -> Option<&'static str> {
    let mut found = None;
    for (var, fields) in CSL_TEXT_FIELDS.iter() {
        if fields.contains(&field) {
            found = Some(*var);
        }
    }
    for (var, f) in CSL_DATE_FIELDS.iter() {
        if *f == field {
            found = Some(*var);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schemas_are_indexed_by_enum_order() {
        for (i, t) in ItemType::ALL.iter().enumerate() {
            assert_eq!(ITEM_TYPE_SCHEMAS[i].item_type, *t);
        }
    }

    /// The examples in the doc comments of itemFields.js:266-277 and 298-310.
    #[test]
    fn upstream_doc_comment_examples() {
        assert_eq!(
            field_from_type_and_base(ItemType::AudioRecording, Field::Publisher),
            Some(Field::Label)
        );
        assert_eq!(
            field_from_type_and_base(ItemType::Book, Field::Publisher),
            Some(Field::Publisher)
        );
        assert_eq!(
            field_from_type_and_base(ItemType::AudioRecording, Field::Number),
            None
        );
        assert_eq!(
            base_from_type_and_field(ItemType::AudioRecording, Field::Label),
            Some(Field::Publisher)
        );
        assert_eq!(
            base_from_type_and_field(ItemType::Book, Field::Publisher),
            Some(Field::Publisher)
        );
        assert_eq!(
            base_from_type_and_field(ItemType::AudioRecording, Field::RunningTime),
            None
        );
        assert_eq!(
            base_from_type_and_field(ItemType::Note, Field::RunningTime),
            None
        );
        // "'publisher' returns fieldIDs for [university, studio, label, network]"
        // (itemFields.js:334) -- the schema has grown since; those four are present.
        let pubs = type_fields_from_base(Field::Publisher);
        for f in [
            Field::University,
            Field::Studio,
            Field::Label,
            Field::Network,
        ] {
            assert!(pubs.contains(&f), "{f:?}");
        }
    }

    #[test]
    fn website_title_maps_to_publication_title() {
        assert_eq!(
            field_from_type_and_base(ItemType::Webpage, Field::PublicationTitle),
            Some(Field::WebsiteTitle)
        );
        assert!(!is_valid_for_type(
            Field::PublicationTitle,
            ItemType::Webpage
        ));
        assert!(Field::PublicationTitle.is_base_field());
        assert!(!Field::WebsiteTitle.is_base_field());
    }

    #[test]
    fn date_fields() {
        assert!(Field::Date.is_date());
        // dateDecided (case) is mapped to base `date`.
        assert!(Field::DateDecided.is_date());
        assert!(!Field::Title.is_date());
        assert!(!Field::AccessDate.is_date());
    }
}
