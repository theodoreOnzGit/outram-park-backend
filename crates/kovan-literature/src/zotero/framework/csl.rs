// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero utilities, https://github.com/zotero/utilities (commit
//   4051881d59c6): utilities_item.js `itemFromCSLJSON` :417-630 (the branch
//   for a translator's sandbox item, `isZoteroItem` false), `itemToCSLJSON`
//   :161; date.js `formatDate` :567-598.
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! `ZU.itemFromCSLJSON` and `ZU.itemToCSLJSON` as translators call them.
//!
//! kovan-common's [`kovan_common::zotero::csl`] ports both for a Zotero
//! *library* item (`isZoteroItem`: values go through `setField` into the
//! type-specific field). A translator passes its sandbox item instead, and
//! upstream then writes raw values under the **base** field names
//! (`item[field] = cslItem[variable]`, :507) and leaves the base-to-type
//! mapping to `itemToAPIJSON`; [`item_from_csl_json`] ports that branch.
//! [`item_to_csl_json`] converts an export item to a
//! [`ZoteroItem`] and calls kovan-common's port.

use super::item::{TranslatorCreator, TranslatorItem};
use super::js;
use super::api_json::type_field_for_base;
use super::utilities::lpad;
use kovan_common::zotero::csl::{
    csl_date_to_edtf, date_exports_as_literal, item_to_csl_json_with, item_type_for_csl, CslItem,
};
use kovan_common::zotero::date::{str_to_iso, DateOptions};
use kovan_common::zotero::schema::{is_valid_creator_type, is_valid_for_type, primary_creator_type};
use kovan_common::zotero::schema_generated::{
    CreatorType, Field, ItemType, CSL_DATE_FIELDS, CSL_NAMES, CSL_TEXT_FIELDS,
};
use kovan_common::zotero::{Creator, CreatorName, ZoteroItem};
use serde_json::{Map, Value};

/// en-US long month names (`Zotero.Date.getMonths().long`).
const MONTHS_LONG: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// JavaScript `ToNumber` for the values a CSL date part can hold.
fn to_number(v: &Value) -> f64 {
    match v {
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        Value::Bool(b) => f64::from(u8::from(*b)),
        Value::Null => 0.0,
        Value::String(s) => {
            let t = js::trim(s);
            if t.is_empty() {
                0.0
            } else if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                i64::from_str_radix(h, 16).map_or(f64::NAN, |n| n as f64)
            } else if t.chars().all(|c| c.is_ascii_digit() || "+-.eE".contains(c)) {
                t.parse::<f64>().unwrap_or(f64::NAN)
            } else if t == "Infinity" || t == "+Infinity" {
                f64::INFINITY
            } else if t == "-Infinity" {
                f64::NEG_INFINITY
            } else {
                f64::NAN
            }
        }
        _ => f64::NAN,
    }
}

/// The base name of a field, as `itemFromCSLJSON` writes it.
fn target_field(item_type: ItemType, field: Field) -> Field {
    if field.is_base_field() {
        if let Some(t) = type_field_for_base(item_type, field) {
            return t;
        }
    }
    field
}

/// `formatDate(newDate)` (date.js:567-598, not short) on the deep copy of a
/// CSL date that `itemFromCSLJSON` builds (:592-617), with the month already
/// decremented.
fn format_csl_date(new_date: &Map<String, Value>, month0: Option<f64>) -> String {
    let mut s = String::new();
    if let Some(p) = new_date.get("part").filter(|p| js::truthy(Some(p))) {
        s.push_str(&js::to_js_string(p));
        s.push(' ');
    }
    let month_name = month0
        .filter(|m| m.fract() == 0.0 && (0.0..12.0).contains(m))
        .map(|m| MONTHS_LONG[m as usize]);
    if let Some(name) = month_name {
        s.push_str(name);
        match new_date.get("day").filter(|d| js::truthy(Some(d))) {
            Some(d) => {
                s.push(' ');
                s.push_str(&js::to_js_string(d));
                s.push_str(", ");
            }
            None => s.push(' '),
        }
    }
    if let Some(y) = new_date.get("year").filter(|y| js::truthy(Some(y))) {
        s.push_str(&js::to_js_string(y));
    }
    s
}

/// `ZU.itemFromCSLJSON(item, cslItem)` for a translator's item
/// (utilities_item.js:417-630, the `isZoteroItem == false` branch).
///
/// Omitted: `creator.creatorTypeID` (a numeric schema id, which nothing
/// downstream of a translator reads; `itemToAPIJSON` keeps only
/// `creatorType`).
pub fn item_from_csl_json(
    item: &mut TranslatorItem,
    csl_in: &CslItem,
    opts: &DateOptions,
) -> Result<(), String> {
    let mut csl = csl_in.clone();
    if !js::truthy(csl.get("type")) {
        return Err("No 'type' provided in CSL-JSON".to_owned());
    }
    let it = item_type_for_csl(&csl).ok_or_else(|| "No 'type' provided in CSL-JSON".to_owned())?;
    // :474-475
    if let Some(id) = csl.get("id") {
        item.set("itemID", id.clone());
    }
    item.item_type = it.as_str().to_owned();

    // :479-485: the event-place hack.
    if matches!(
        it,
        ItemType::AudioRecording | ItemType::Presentation | ItemType::VideoRecording
    ) {
        if let Some(ep) = csl.remove("event-place") {
            csl.insert("publisher-place".into(), ep);
        }
    }

    // :488-514: text fields, raw values under the mapping's own name.
    for (variable, fields) in CSL_TEXT_FIELDS.iter() {
        let Some(value) = csl.get(*variable) else {
            continue;
        };
        for field in fields.iter() {
            if is_valid_for_type(target_field(it, *field), it) {
                item.set(field.as_str(), value.clone());
                break;
            }
        }
    }

    // :517-559: creators.
    let mut done: Vec<&str> = Vec::new();
    for (creator_type, csl_var) in CSL_NAMES.iter() {
        if done.contains(csl_var) {
            continue;
        }
        done.push(csl_var);
        let Some(names) = csl.get(*csl_var) else {
            continue;
        };
        let ct: Option<CreatorType> = if is_valid_creator_type(*creator_type, it) {
            Some(*creator_type)
        } else {
            primary_creator_type(it)
        };
        // `for (var i in nameMappings)`: an array's elements or an object's values.
        let list: Vec<&Value> = match names {
            Value::Array(a) => a.iter().collect(),
            Value::Object(o) => o.values().collect(),
            _ => Vec::new(),
        };
        for n in list {
            let get = |k: &str| n.get(k).filter(|v| js::truthy(Some(v)));
            let mut c = TranslatorCreator::default();
            if get("family").is_some() || get("given").is_some() {
                c.last_name = Some(get("family").map(js::to_js_string).unwrap_or_default());
                c.first_name = Some(get("given").map(js::to_js_string).unwrap_or_default());
            } else if let Some(l) = get("literal") {
                c.last_name = Some(js::to_js_string(l));
                c.field_mode = Some(1);
            } else {
                continue;
            }
            c.creator_type = ct.map(|t| t.as_str().to_owned());
            item.creators.push(c);
        }
    }

    // :562-629: dates.
    for (variable, field) in CSL_DATE_FIELDS.iter() {
        let Some(cdate) = csl.get(*variable) else {
            continue;
        };
        if !is_valid_for_type(target_field(it, *field), it) {
            continue;
        }
        let empty = Map::new();
        let cd = cdate.as_object().unwrap_or(&empty);
        let is_accessed = *variable == "accessed";
        let literal = cd.get("literal").filter(|v| js::truthy(Some(v)));
        let raw = cd.get("raw").filter(|v| js::truthy(Some(v)));
        let mut date = Value::String(String::new());
        if let Some(lr) = literal.or(raw) {
            let s = js::to_js_string(lr);
            date = Value::String(s.clone());
            if is_accessed {
                // strToISO gives false when there is no year.
                date = str_to_iso(&s, opts).map_or(Value::Bool(false), Value::String);
            } else if literal.is_some() && !date_exports_as_literal(&s, opts) {
                date = Value::String(format!("\"{s}\""));
            }
        } else if let Some(e) = (!is_accessed).then(|| csl_date_to_edtf(cd)).flatten() {
            date = Value::String(e);
        } else {
            let mut nd = cd.clone();
            if let Some(Value::Array(dp)) = cd.get("date-parts") {
                if let Some(Value::Array(first)) = dp.first() {
                    for (i, k) in ["year", "month", "day"].iter().enumerate() {
                        if let Some(v) = first.get(i).filter(|v| js::truthy(Some(v))) {
                            nd.insert((*k).into(), v.clone());
                        }
                    }
                }
            }
            if let Some(year) = nd.get("year").filter(|y| js::truthy(Some(y))).cloned() {
                if is_accessed {
                    let mut s = lpad(&js::to_js_string(&year), "0", 4);
                    if let Some(m) = nd.get("month").filter(|m| js::truthy(Some(m))) {
                        s.push('-');
                        s.push_str(&lpad(&js::to_js_string(m), "0", 2));
                        if let Some(d) = nd.get("day").filter(|d| js::truthy(Some(d))) {
                            s.push('-');
                            s.push_str(&lpad(&js::to_js_string(d), "0", 2));
                        }
                    }
                    date = Value::String(s);
                } else {
                    // `if(newDate.month) newDate.month--;`
                    let month0 = match nd.get("month") {
                        Some(m) if js::truthy(Some(m)) => Some(to_number(m) - 1.0),
                        Some(Value::Number(n)) => n.as_f64(),
                        _ => None,
                    };
                    let mut s = format_csl_date(&nd, month0);
                    if let Some(season) = nd.get("season").filter(|v| js::truthy(Some(v))) {
                        s = format!("{} {s}", js::to_js_string(season));
                    }
                    date = Value::String(s);
                }
            }
        }
        item.set(field.as_str(), date);
    }
    Ok(())
}

/// A translator's export item as a kovan-common [`ZoteroItem`], for
/// `itemToCSLJSON`: string properties named like fields become fields (what
/// `field in zoteroItem` finds; upstream skips non-string text values);
/// creators with a known type keep their names (`name`, or `fieldMode: 1`
/// with a last name and no first name, is a literal; upstream skips unknown
/// creator types too, since they have no CSL mapping and cannot be the
/// primary type); `uri` and `note` carry over.
pub fn export_item_to_zotero_item(item: &TranslatorItem) -> Result<ZoteroItem, String> {
    let it = ItemType::from_name(&item.item_type)
        .ok_or_else(|| format!("Unexpected Zotero Item type \"{}\"", item.item_type))?;
    let mut z = ZoteroItem::new(it);
    for (k, v) in item.props.iter() {
        let Value::String(s) = v else { continue };
        match k {
            "uri" => z.uri = Some(s.clone()),
            "note" => z.note = Some(s.clone()),
            _ if Field::from_name(k).is_some() => {
                z.fields.insert(k.to_owned(), s.clone());
            }
            _ => {}
        }
    }
    for c in &item.creators {
        let Some(ct) = c.creator_type.as_deref().and_then(CreatorType::from_name) else {
            continue;
        };
        let first = c.first_name.clone().unwrap_or_default();
        let last = c.last_name.clone().unwrap_or_default();
        let name = match c.other.get("name").filter(|n| js::truthy(Some(n))) {
            Some(n) => CreatorName::SingleField {
                name: js::to_js_string(n),
            },
            None if c.field_mode == Some(1) && !last.is_empty() && first.is_empty() => {
                CreatorName::SingleField { name: last }
            }
            None => CreatorName::TwoField {
                first_name: first,
                last_name: last,
            },
        };
        z.creators.push(Creator {
            creator_type: ct,
            name,
        });
    }
    Ok(z)
}

/// `ZU.itemToCSLJSON(item)` for an export item (kovan-common's port of
/// utilities 4051881d59c6).
pub fn item_to_csl_json(item: &TranslatorItem, opts: &DateOptions) -> Result<CslItem, String> {
    let z = export_item_to_zotero_item(item)?;
    item_to_csl_json_with(&z, opts).map_err(|e| e.to_string())
}
