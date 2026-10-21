// Part of the kovan Zotero port (GitHub #747, #748).
//
// Upstream: Zotero, https://github.com/zotero/zotero (commit 9cbba8c4d281),
//   chrome/content/zotero/xpcom/schema.js (`_updateGlobalSchema`, the order
//   in which the client registers fields and creator types), and
//   https://github.com/zotero/utilities (commit 4051881d59c6), schema.js
//   (`Schema.init`, the CSL mapping tables).
// Schema data: https://github.com/zotero/zotero-schema (commit b86c79b56479),
//   schema.json. No licence text upstream; treated as AGPLv3 as part of
//   Zotero, maintainer decision 2026-10-07, #747.
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0.

//! Deterministic generator for the Zotero schema tables in
//! `kovan_common::zotero` (GitHub #748).
//!
//! [`generate_schema_rs`] reads Zotero's `schema.json` (the file in
//! `zotero/zotero-schema`, also served at `https://api.zotero.org/schema`) and
//! returns the Rust source of `crates/kovan-common/src/zotero/schema_generated.rs`:
//!
//! * the `ItemType`, `Field` and `CreatorType` enums, with `as_str`,
//!   `from_name` and `ALL`;
//! * `ITEM_TYPE_SCHEMAS`: each item type's fields **in schema order**, each
//!   field's base field (`baseField`), its creator types and its primary
//!   creator type;
//! * `META_FIELD_TYPES` (the schema's `meta.fields`, e.g. which fields are
//!   dates);
//! * the CSL mappings `CSL_TYPES`, `CSL_TEXT_FIELDS`, `CSL_DATE_FIELDS`,
//!   `CSL_NAMES`, in schema order;
//! * the **en-US** labels only (`EN_US_ITEM_TYPE_LABELS`,
//!   `EN_US_FIELD_LABELS`, `EN_US_CREATOR_TYPE_LABELS`). The other 47 locales
//!   in `schema.json` are deliberately not emitted.
//!
//! Field and creator-type order follow Zotero's own registration order
//! (`_updateGlobalSchema`, zotero `xpcom/schema.js:486-497`): walk item types
//! in order, each one's fields in order, adding `field` then `baseField` the
//! first time each name is seen.
//!
//! Unlike the numerical-method templates, this generator reads data, but it
//! is still deterministic: the output is a pure function of the input text
//! (no hash maps, no clock, no environment), so the same `schema.json` gives
//! byte-identical source. Regenerate the committed file with
//!
//! ```text
//! cargo run --release -p kovan-codegen --example zotero_schema -- \
//!     vendor/zotero-schema/schema.json crates/kovan-common/src/zotero/schema_generated.rs
//! ```
//!
//! and check it with the `zotero_schema_regen` test.

pub mod ordered_json;

use ordered_json::OrderedJson;
use std::fmt::Write as _;

/// The zotero-schema commit the committed tables were generated from.
pub const SCHEMA_SOURCE_COMMIT: &str = "b86c79b56479";

/// Why `schema.json` could not be turned into Rust source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZoteroSchemaError {
    /// The text is not JSON.
    Json(String),
    /// The JSON does not have the shape of a Zotero schema; the payload says
    /// what was missing or malformed.
    Shape(String),
}

impl std::fmt::Display for ZoteroSchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZoteroSchemaError::Json(e) => write!(f, "schema.json is not valid JSON: {e}"),
            ZoteroSchemaError::Shape(e) => write!(f, "schema.json has an unexpected shape: {e}"),
        }
    }
}

impl std::error::Error for ZoteroSchemaError {}

fn shape(msg: impl Into<String>) -> ZoteroSchemaError {
    ZoteroSchemaError::Shape(msg.into())
}

/// One item type as read from `schema.json`.
struct ItemTypeIn {
    name: String,
    /// `(field, baseField)` in schema order.
    fields: Vec<(String, Option<String>)>,
    /// `(creatorType, primary)` in schema order.
    creator_types: Vec<(String, bool)>,
}

/// Turn a Zotero identifier into a Rust enum variant name.
///
/// `journalArticle` -> `JournalArticle`; an all-caps acronym is title-cased
/// (`DOI` -> `Doi`, `PMCID` -> `Pmcid`) so the variant is idiomatic Rust.
pub fn variant_name(name: &str) -> String {
    let all_upper = name.chars().all(|c| !c.is_ascii_lowercase());
    let mut out = String::with_capacity(name.len());
    for (i, c) in name.chars().enumerate() {
        if i == 0 {
            out.push(c.to_ascii_uppercase());
        } else if all_upper {
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn str_member<'a>(v: &'a OrderedJson, key: &str, ctx: &str) -> Result<&'a str, ZoteroSchemaError> {
    v.get(key)
        .and_then(OrderedJson::as_str)
        .ok_or_else(|| shape(format!("{ctx}: missing string '{key}'")))
}

fn object<'a>(
    v: Option<&'a OrderedJson>,
    ctx: &str,
) -> Result<&'a [(String, OrderedJson)], ZoteroSchemaError> {
    v.and_then(OrderedJson::as_object)
        .ok_or_else(|| shape(format!("{ctx}: expected an object")))
}

fn array<'a>(
    v: Option<&'a OrderedJson>,
    ctx: &str,
) -> Result<&'a [OrderedJson], ZoteroSchemaError> {
    v.and_then(OrderedJson::as_array)
        .ok_or_else(|| shape(format!("{ctx}: expected an array")))
}

fn read_item_types(root: &OrderedJson) -> Result<Vec<ItemTypeIn>, ZoteroSchemaError> {
    let mut out = Vec::new();
    for it in array(root.get("itemTypes"), "itemTypes")? {
        let name = str_member(it, "itemType", "itemTypes[]")?.to_owned();
        let mut fields = Vec::new();
        for f in array(it.get("fields"), &format!("{name}.fields"))? {
            let field = str_member(f, "field", &format!("{name}.fields[]"))?.to_owned();
            let base = match f.get("baseField") {
                None => None,
                Some(b) => Some(
                    b.as_str()
                        .ok_or_else(|| shape(format!("{name}.{field}.baseField: not a string")))?
                        .to_owned(),
                ),
            };
            fields.push((field, base));
        }
        let mut creator_types = Vec::new();
        for c in array(it.get("creatorTypes"), &format!("{name}.creatorTypes"))? {
            let ct = str_member(c, "creatorType", &format!("{name}.creatorTypes[]"))?.to_owned();
            let primary = c
                .get("primary")
                .and_then(OrderedJson::as_bool)
                .unwrap_or(false);
            creator_types.push((ct, primary));
        }
        out.push(ItemTypeIn {
            name,
            fields,
            creator_types,
        });
    }
    Ok(out)
}

/// Push `name` unless already present (insertion-ordered set).
fn push_unique(list: &mut Vec<String>, name: &str) {
    if !list.iter().any(|n| n == name) {
        list.push(name.to_owned());
    }
}

/// Check that no two names map to the same Rust variant.
fn check_variants(kind: &str, names: &[String]) -> Result<(), ZoteroSchemaError> {
    let mut seen: Vec<String> = Vec::with_capacity(names.len());
    for n in names {
        let v = variant_name(n);
        if seen.contains(&v) {
            return Err(shape(format!(
                "{kind} '{n}' collides with another as variant {v}"
            )));
        }
        seen.push(v);
    }
    Ok(())
}

/// Emit `pub enum <ty>` with `ALL`, `as_str` and `from_name`.
fn emit_enum(out: &mut String, ty: &str, doc: &str, names: &[String]) {
    let _ = writeln!(out, "/// {doc}");
    let _ = writeln!(out, "///");
    let _ = writeln!(
        out,
        "/// Serialises as Zotero's own name (`{}`), via [`{ty}::as_str`].",
        names.first().map(String::as_str).unwrap_or("")
    );
    let _ = writeln!(
        out,
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]"
    );
    let _ = writeln!(out, "pub enum {ty} {{");
    for n in names {
        let _ = writeln!(out, "    /// `{n}`");
        let _ = writeln!(out, "    {},", variant_name(n));
    }
    let _ = writeln!(out, "}}\n");
    let _ = writeln!(out, "impl {ty} {{");
    let _ = writeln!(out, "    /// Every variant, in schema order.");
    let _ = writeln!(out, "    pub const ALL: [{ty}; {}] = [", names.len());
    for n in names {
        let _ = writeln!(out, "        {ty}::{},", variant_name(n));
    }
    let _ = writeln!(out, "    ];\n");
    let _ = writeln!(out, "    /// Zotero's name for this {ty}.");
    let _ = writeln!(out, "    pub const fn as_str(self) -> &'static str {{");
    let _ = writeln!(out, "        match self {{");
    for n in names {
        let _ = writeln!(out, "            {ty}::{} => {n:?},", variant_name(n));
    }
    let _ = writeln!(out, "        }}\n    }}\n");
    let _ = writeln!(
        out,
        "    /// The {ty} Zotero calls `name` (exact, case-sensitive match)."
    );
    let _ = writeln!(out, "    pub fn from_name(name: &str) -> Option<{ty}> {{");
    let _ = writeln!(out, "        match name {{");
    for n in names {
        let _ = writeln!(out, "            {n:?} => Some({ty}::{}),", variant_name(n));
    }
    let _ = writeln!(out, "            _ => None,\n        }}\n    }}\n}}\n");
}

/// Generate the Rust source of `kovan_common::zotero::schema_generated` from
/// the text of Zotero's `schema.json`.
///
/// `source_commit` is recorded in the header (the zotero-schema commit the
/// file came from; [`SCHEMA_SOURCE_COMMIT`] for the committed tables).
///
/// # Errors
/// [`ZoteroSchemaError::Json`] when the text is not JSON;
/// [`ZoteroSchemaError::Shape`] when a required member is missing or of the
/// wrong type, when a `baseField`/CSL mapping names a field or item type the
/// schema does not define, or when two names would produce the same Rust
/// variant.
pub fn generate_schema_rs(
    schema_json: &str,
    source_commit: &str,
) -> Result<String, ZoteroSchemaError> {
    let root =
        OrderedJson::parse(schema_json).map_err(|e| ZoteroSchemaError::Json(e.to_string()))?;
    let version = root
        .get("version")
        .and_then(OrderedJson::as_u64)
        .ok_or_else(|| shape("missing integer 'version'"))?;
    let item_types = read_item_types(&root)?;

    // Zotero's registration order (xpcom/schema.js:486-497).
    let mut fields: Vec<String> = Vec::new();
    let mut creator_types: Vec<String> = Vec::new();
    for it in &item_types {
        for (f, b) in &it.fields {
            push_unique(&mut fields, f);
            if let Some(b) = b {
                push_unique(&mut fields, b);
            }
        }
        for (c, _) in &it.creator_types {
            push_unique(&mut creator_types, c);
        }
    }
    let type_names: Vec<String> = item_types.iter().map(|t| t.name.clone()).collect();
    check_variants("item type", &type_names)?;
    check_variants("field", &fields)?;
    check_variants("creator type", &creator_types)?;

    let field_v = |name: &str| -> Result<String, ZoteroSchemaError> {
        if fields.iter().any(|f| f == name) {
            Ok(format!("Field::{}", variant_name(name)))
        } else {
            Err(shape(format!("unknown field '{name}'")))
        }
    };
    let type_v = |name: &str| -> Result<String, ZoteroSchemaError> {
        if type_names.iter().any(|f| f == name) {
            Ok(format!("ItemType::{}", variant_name(name)))
        } else {
            Err(shape(format!("unknown item type '{name}'")))
        }
    };
    let creator_v = |name: &str| -> Result<String, ZoteroSchemaError> {
        if creator_types.iter().any(|f| f == name) {
            Ok(format!("CreatorType::{}", variant_name(name)))
        } else {
            Err(shape(format!("unknown creator type '{name}'")))
        }
    };

    let mut out = String::new();
    let _ = writeln!(
        out,
        "// @generated by kovan_codegen::zotero::generate_schema_rs -- DO NOT EDIT BY HAND."
    );
    let _ = writeln!(out, "//");
    let _ = writeln!(
        out,
        "// Source: zotero-schema schema.json, schema version {version}, commit {source_commit}"
    );
    let _ = writeln!(out, "// (https://github.com/zotero/zotero-schema).");
    let _ = writeln!(
        out,
        "// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA."
    );
    let _ = writeln!(
        out,
        "// Licence: no licence text upstream; treated as AGPLv3 as part of Zotero,"
    );
    let _ = writeln!(out, "// maintainer decision 2026-10-07, #747.");
    let _ = writeln!(out, "//");
    let _ = writeln!(out, "// Regenerate with:");
    let _ = writeln!(
        out,
        "//   cargo run --release -p kovan-codegen --example zotero_schema -- \\"
    );
    let _ = writeln!(
        out,
        "//       vendor/zotero-schema/schema.json crates/kovan-common/src/zotero/schema_generated.rs"
    );
    let _ = writeln!(
        out,
        "// and verify with kovan-codegen's `zotero_schema_regen` test."
    );
    let _ = writeln!(out, "//");
    let _ = writeln!(
        out,
        "// Only the en-US labels are emitted; the other locales in schema.json are not.\n"
    );
    let _ = writeln!(
        out,
        "use super::schema::{{ItemTypeField, ItemTypeSchema}};\n"
    );
    let _ = writeln!(
        out,
        "/// The `version` of the schema.json these tables were generated from."
    );
    let _ = writeln!(out, "pub const SCHEMA_VERSION: u32 = {version};\n");
    let _ = writeln!(
        out,
        "/// The zotero-schema commit these tables were generated from."
    );
    let _ = writeln!(out, "pub const SCHEMA_COMMIT: &str = {source_commit:?};\n");

    emit_enum(
        &mut out,
        "ItemType",
        "A Zotero item type (schema.json `itemTypes[].itemType`).",
        &type_names,
    );
    emit_enum(
        &mut out,
        "Field",
        "A Zotero item field, in Zotero's registration order (each type's `field`, then its `baseField`).",
        &fields,
    );
    emit_enum(
        &mut out,
        "CreatorType",
        "A Zotero creator type (schema.json `itemTypes[].creatorTypes[]`).",
        &creator_types,
    );

    // Item type schemas.
    let _ = writeln!(
        out,
        "/// Every item type's fields (schema order, with base field) and creator types."
    );
    let _ = writeln!(
        out,
        "pub static ITEM_TYPE_SCHEMAS: [ItemTypeSchema; {}] = [",
        item_types.len()
    );
    for it in &item_types {
        let _ = writeln!(out, "    ItemTypeSchema {{");
        let _ = writeln!(out, "        item_type: {},", type_v(&it.name)?);
        let _ = writeln!(out, "        fields: &[");
        for (f, b) in &it.fields {
            let base = match b {
                Some(b) => format!("Some({})", field_v(b)?),
                None => "None".to_owned(),
            };
            let _ = writeln!(
                out,
                "            ItemTypeField {{ field: {}, base_field: {base} }},",
                field_v(f)?
            );
        }
        let _ = writeln!(out, "        ],");
        let _ = writeln!(out, "        creator_types: &[");
        for (c, _) in &it.creator_types {
            let _ = writeln!(out, "            {},", creator_v(c)?);
        }
        let _ = writeln!(out, "        ],");
        let primaries: Vec<&String> = it
            .creator_types
            .iter()
            .filter(|(_, p)| *p)
            .map(|(c, _)| c)
            .collect();
        let primary = match primaries.as_slice() {
            [] => "None".to_owned(),
            [one] => format!("Some({})", creator_v(one)?),
            _ => {
                return Err(shape(format!(
                    "{}: more than one primary creator type",
                    it.name
                )))
            }
        };
        let _ = writeln!(out, "        primary_creator_type: {primary},");
        let _ = writeln!(out, "    }},");
    }
    let _ = writeln!(out, "];\n");

    // meta.fields
    let meta = object(
        root.get("meta").and_then(|m| m.get("fields")),
        "meta.fields",
    )?;
    let _ = writeln!(
        out,
        "/// schema.json `meta.fields`: the value type of special fields (e.g. `\"date\"`)."
    );
    let _ = writeln!(
        out,
        "pub static META_FIELD_TYPES: [(Field, &str); {}] = [",
        meta.len()
    );
    for (f, v) in meta {
        let ty = str_member(v, "type", &format!("meta.fields.{f}"))?;
        let _ = writeln!(out, "    ({}, {ty:?}),", field_v(f)?);
    }
    let _ = writeln!(out, "];\n");

    // CSL
    let csl = root.get("csl").ok_or_else(|| shape("missing 'csl'"))?;
    let types = object(csl.get("types"), "csl.types")?;
    let _ = writeln!(
        out,
        "/// schema.json `csl.types`: CSL type -> Zotero item types (first is the import default)."
    );
    let _ = writeln!(
        out,
        "pub static CSL_TYPES: [(&str, &[ItemType]); {}] = [",
        types.len()
    );
    for (csl_type, zts) in types {
        let mut list = Vec::new();
        for z in array(Some(zts), &format!("csl.types.{csl_type}"))? {
            let z = z
                .as_str()
                .ok_or_else(|| shape(format!("csl.types.{csl_type}: not a string")))?;
            list.push(type_v(z)?);
        }
        let _ = writeln!(out, "    ({csl_type:?}, &[{}]),", list.join(", "));
    }
    let _ = writeln!(out, "];\n");

    let fields_obj = csl
        .get("fields")
        .ok_or_else(|| shape("missing 'csl.fields'"))?;
    let text = object(fields_obj.get("text"), "csl.fields.text")?;
    let _ = writeln!(
        out,
        "/// schema.json `csl.fields.text`: CSL variable -> Zotero fields, in priority order."
    );
    let _ = writeln!(
        out,
        "pub static CSL_TEXT_FIELDS: [(&str, &[Field]); {}] = [",
        text.len()
    );
    for (var, fs) in text {
        let mut list = Vec::new();
        for f in array(Some(fs), &format!("csl.fields.text.{var}"))? {
            let f = f
                .as_str()
                .ok_or_else(|| shape(format!("csl.fields.text.{var}: not a string")))?;
            list.push(field_v(f)?);
        }
        let _ = writeln!(out, "    ({var:?}, &[{}]),", list.join(", "));
    }
    let _ = writeln!(out, "];\n");

    let date = object(fields_obj.get("date"), "csl.fields.date")?;
    let _ = writeln!(
        out,
        "/// schema.json `csl.fields.date`: CSL date variable -> Zotero field."
    );
    let _ = writeln!(
        out,
        "pub static CSL_DATE_FIELDS: [(&str, Field); {}] = [",
        date.len()
    );
    for (var, f) in date {
        let f = f
            .as_str()
            .ok_or_else(|| shape(format!("csl.fields.date.{var}: not a string")))?;
        let _ = writeln!(out, "    ({var:?}, {}),", field_v(f)?);
    }
    let _ = writeln!(out, "];\n");

    let names = object(csl.get("names"), "csl.names")?;
    let _ = writeln!(
        out,
        "/// schema.json `csl.names`: Zotero creator type -> CSL name variable."
    );
    let _ = writeln!(
        out,
        "pub static CSL_NAMES: [(CreatorType, &str); {}] = [",
        names.len()
    );
    for (ct, var) in names {
        let var = var
            .as_str()
            .ok_or_else(|| shape(format!("csl.names.{ct}: not a string")))?;
        let _ = writeln!(out, "    ({}, {var:?}),", creator_v(ct)?);
    }
    let _ = writeln!(out, "];\n");

    // en-US locale only.
    let en = root
        .get("locales")
        .and_then(|l| l.get("en-US"))
        .ok_or_else(|| shape("missing 'locales.en-US'"))?;
    let it_labels = object(en.get("itemTypes"), "locales.en-US.itemTypes")?;
    let _ = writeln!(
        out,
        "/// en-US item type labels (schema.json `locales.en-US.itemTypes`)."
    );
    let _ = writeln!(
        out,
        "pub static EN_US_ITEM_TYPE_LABELS: [(ItemType, &str); {}] = [",
        it_labels.len()
    );
    for (k, v) in it_labels {
        let v = v
            .as_str()
            .ok_or_else(|| shape(format!("label {k}: not a string")))?;
        let _ = writeln!(out, "    ({}, {v:?}),", type_v(k)?);
    }
    let _ = writeln!(out, "];\n");

    // Field labels are keyed by name: the locale also labels `dateAdded`,
    // `dateModified` and `itemType`, which are item properties, not fields.
    let f_labels = object(en.get("fields"), "locales.en-US.fields")?;
    let _ = writeln!(
        out,
        "/// en-US field labels (schema.json `locales.en-US.fields`), keyed by name because"
    );
    let _ = writeln!(
        out,
        "/// the locale also labels the item properties `dateAdded`, `dateModified`, `itemType`."
    );
    let _ = writeln!(
        out,
        "pub static EN_US_FIELD_LABELS: [(&str, &str); {}] = [",
        f_labels.len()
    );
    for (k, v) in f_labels {
        let v = v
            .as_str()
            .ok_or_else(|| shape(format!("label {k}: not a string")))?;
        let _ = writeln!(out, "    ({k:?}, {v:?}),");
    }
    let _ = writeln!(out, "];\n");

    let c_labels = object(en.get("creatorTypes"), "locales.en-US.creatorTypes")?;
    let _ = writeln!(
        out,
        "/// en-US creator type labels (schema.json `locales.en-US.creatorTypes`)."
    );
    let _ = writeln!(
        out,
        "pub static EN_US_CREATOR_TYPE_LABELS: [(CreatorType, &str); {}] = [",
        c_labels.len()
    );
    for (k, v) in c_labels {
        let v = v
            .as_str()
            .ok_or_else(|| shape(format!("label {k}: not a string")))?;
        let _ = writeln!(out, "    ({}, {v:?}),", creator_v(k)?);
    }
    let _ = writeln!(out, "];");

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A two-type schema in the real shape, small enough to check by eye.
    const MINI: &str = r#"{
        "version": 7,
        "itemTypes": [
            {"itemType": "note", "fields": [], "creatorTypes": []},
            {"itemType": "webpage",
             "fields": [{"field": "title"}, {"field": "websiteTitle", "baseField": "publicationTitle"}, {"field": "DOI"}],
             "creatorTypes": [{"creatorType": "contributor"}, {"creatorType": "author", "primary": true}]}
        ],
        "meta": {"fields": {"date": {"type": "date"}}},
        "csl": {
            "types": {"webpage": ["webpage"], "document": ["note"]},
            "fields": {"text": {"title": ["title"], "container-title": ["publicationTitle"]},
                       "date": {"issued": "date"}},
            "names": {"author": "author"}
        },
        "locales": {"en-US": {"itemTypes": {"note": "Note", "webpage": "Web Page"},
                              "fields": {"title": "Title", "dateAdded": "Date Added"},
                              "creatorTypes": {"author": "Author"}},
                    "de": {"itemTypes": {"note": "Notiz"}}}
    }"#;

    #[test]
    fn variant_names_are_idiomatic() {
        assert_eq!(variant_name("journalArticle"), "JournalArticle");
        assert_eq!(variant_name("DOI"), "Doi");
        assert_eq!(variant_name("PMCID"), "Pmcid");
        assert_eq!(variant_name("case"), "Case");
    }

    #[test]
    fn mini_schema_fails_on_field_only_known_to_meta() {
        // `date` appears in meta.fields and csl but no item type defines it.
        let err = generate_schema_rs(MINI, "test").unwrap_err();
        assert_eq!(err, ZoteroSchemaError::Shape("unknown field 'date'".into()));
    }

    #[test]
    fn mini_schema_generates_in_schema_order() {
        let mini = MINI.replace(
            r#"{"field": "DOI"}"#,
            r#"{"field": "DOI"}, {"field": "date"}"#,
        );
        let src = generate_schema_rs(&mini, "test").expect("generates");
        // Registration order: title, websiteTitle, publicationTitle (base), DOI, date.
        let t = src.find("    Title,").unwrap();
        let w = src.find("    WebsiteTitle,").unwrap();
        let p = src.find("    PublicationTitle,").unwrap();
        let d = src.find("    Doi,").unwrap();
        assert!(t < w && w < p && p < d);
        assert!(src.contains("primary_creator_type: Some(CreatorType::Author),"));
        assert!(src.contains("ItemTypeField { field: Field::WebsiteTitle, base_field: Some(Field::PublicationTitle) },"));
        assert!(src.contains("pub const SCHEMA_VERSION: u32 = 7;"));
        // en-US only: the German label must not appear.
        assert!(!src.contains("Notiz"));
        assert!(src.contains("(\"dateAdded\", \"Date Added\"),"));
    }

    #[test]
    fn generation_is_deterministic() {
        let mini = MINI.replace(
            r#"{"field": "DOI"}"#,
            r#"{"field": "DOI"}, {"field": "date"}"#,
        );
        assert_eq!(
            generate_schema_rs(&mini, "x").unwrap(),
            generate_schema_rs(&mini, "x").unwrap()
        );
    }

    #[test]
    fn malformed_input_is_an_error_not_a_panic() {
        assert!(matches!(
            generate_schema_rs("{", "x"),
            Err(ZoteroSchemaError::Json(_))
        ));
        assert!(matches!(
            generate_schema_rs("{}", "x"),
            Err(ZoteroSchemaError::Shape(_))
        ));
    }
}
