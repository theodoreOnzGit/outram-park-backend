// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_static_locator.js; plus, in the "citation-item input" section,
//              the per-item input steps of makeCitationCluster
//              (src/api_cite.js:131-154)
// Version:     2.4.63, commit 73bc1b44bc7d54d0bfec4e070fd27f5efe024ff9
// Copyright:   (c) 2009-2019 Frank Bennett
// Licence:     AGPL-3.0, taken from upstream's "CPAL-1.0 or AGPL-3.0-or-later"
//              (LICENSE at the commit above; see this crate's NOTICE).
// Modified:    2026-10-08, by the OUTRAM PARK contributors. This file is a
//              Rust translation (port) of the file(s) named above, modified
//              from the original.
// No warranty: this program is distributed in the hope that it will be
//              useful, but WITHOUT ANY WARRANTY; without even the implied
//              warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
//              PURPOSE. See the GNU Affero General Public License.

//! Port of `src/util_static_locator.js`: `remapSectionVariable` (fold a legal
//! item's `section` into the cite's `locator`) and `setNumberLabels`.
//!
//! (The citation-item input steps of `processCitationCluster` that used to
//! follow here, `CSL.parseLocator`, `remapSectionVariable` and the
//! `locator_label_parse` step, moved to `api_cite.rs`: `citation_item_input`.)

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::js::{self, Obj};
use super::state::State;
use super::load::{
    statute_subdiv_string, statute_subdiv_string_reverse, STATUTE_SUBDIV_PLAIN_REGEX,
    STATUTE_SUBDIV_PLAIN_REGEX_FRONT,
};
use super::util_number::{ShadowLabel, ShadowNumber, ShadowValue};
use super::{CslResult, EngineError};

/// JS `.`: anything but `\n`, `\r`, U+2028, U+2029.
const DOT: &str = "[^\\n\\r\\u{2028}\\u{2029}]";

fn rx(src: &str) -> Regex {
    Regex::new(src).unwrap_or_else(|e| panic!("invalid static regex {src:?}: {e}"))
}

/// `/^([^ ]*)\s*(.*)/`.
static LOCATOR_HEAD_RE: LazyLock<Regex> =
    LazyLock::new(|| rx(&format!("^([^ ]*)[{ws}]*({DOT}*)", ws = js::WS)));

/// The legal item types `remapSectionVariable` applies to.
const LEGAL_TYPES: [&str; 5] = ["bill", "gazette", "legislation", "regulation", "treaty"];

/// A JS string property that upstream calls `.trim()` on; a non-string
/// value is a TypeError there.
fn str_of(o: &Obj, key: &str, path: &str) -> CslResult<String> {
    match o.get(key) {
        Some(Value::String(s)) => Ok(s.clone()),
        _ => Err(EngineError::BadInput(format!(
            "{path}.trim is not a function"
        ))),
    }
}

/// `CSL.Engine.prototype.remapSectionVariable` for one `[Item, item]` pair:
/// for the legal types, prepend the item's `section` (`Item`, mutated: the
/// stored section gets its label) to the cite's `locator` (`item`, mutated,
/// and its `label` is cleared), leaving all parsing to `processNumber`.
pub fn remap_section_variable_one(item_obj: &mut Obj, cite_item: &mut Obj) -> CslResult<()> {
    let is_legal = item_obj
        .get("type")
        .and_then(Value::as_str)
        .map(|t| LEGAL_TYPES.contains(&t))
        .unwrap_or(false);
    if !is_legal {
        return Ok(());
    }
    // If a locator value exists, then leave be an overriding label at the
    // start of the locator field, defaulting to the label value.
    if js::get_truthy(cite_item, "locator") {
        let locator = js::trim(&str_of(cite_item, "locator", "item.locator")?).to_string();
        cite_item.insert("locator".into(), Value::String(locator.clone()));
        if !STATUTE_SUBDIV_PLAIN_REGEX_FRONT.is_match(&locator) {
            let new_locator = if js::get_truthy(cite_item, "label") {
                let label = js::to_js_string(cite_item.get("label").unwrap_or(&Value::Null));
                format!(
                    "{} {}",
                    statute_subdiv_string_reverse(&label).unwrap_or("undefined"),
                    locator
                )
            } else {
                format!("p. {locator}")
            };
            cite_item.insert("locator".into(), Value::String(new_locator));
        }
    }
    // If a section value exists, apply an overriding label at the start of
    // the section field, defaulting to sec.
    let mut section_master_label: Option<String> = None;
    if js::get_truthy(item_obj, "section") {
        let section = js::trim(&str_of(item_obj, "section", "Item.section")?).to_string();
        item_obj.insert("section".into(), Value::String(section.clone()));
        match STATUTE_SUBDIV_PLAIN_REGEX_FRONT.find(&section) {
            None => {
                item_obj.insert("section".into(), Value::String(format!("sec. {section}")));
                section_master_label = Some("sec.".into());
            }
            Some(m) => {
                section_master_label = Some(js::trim(m.as_str()).to_string());
            }
        }
    }
    if js::get_truthy(item_obj, "section") {
        let section = js::to_js_string(item_obj.get("section").unwrap_or(&Value::Null));
        if !js::get_truthy(cite_item, "locator") {
            // section and no locator: the section string is the locator
            cite_item.insert("locator".into(), Value::String(section));
        } else {
            // both exist: a leading "p." is dropped; the section goes first
            let mut locator = js::to_js_string(cite_item.get("locator").unwrap_or(&Value::Null));
            let mut space = " ";
            if let Some(m) = LOCATOR_HEAD_RE.captures(&locator) {
                let m1 = m.get(1).map(|x| x.as_str()).unwrap_or("").to_string();
                let m2 = m.get(2).map(|x| x.as_str()).unwrap_or("").to_string();
                if m1 == "p." && section_master_label.as_deref() != Some("p.") {
                    locator = m2;
                }
                if ["[", "(", ".", ",", ";", ":", "?"]
                    .contains(&js::slice(&locator, 0, Some(1)).as_str())
                {
                    space = "";
                }
            } else {
                space = "";
            }
            cite_item.insert(
                "locator".into(),
                Value::String(format!("{section}{space}{locator}")),
            );
        }
    }
    cite_item.insert("label".into(), Value::String(String::new()));
    Ok(())
}

/// `CSL.Engine.prototype.remapSectionVariable(inputList)` over `[Item, item]`
/// pairs (both mutated in place).
pub fn remap_section_variable(input_list: &mut [(Value, Value)]) -> CslResult<()> {
    for (item_obj, cite_item) in input_list.iter_mut() {
        if let (Value::Object(a), Value::Object(b)) = (item_obj, cite_item) {
            remap_section_variable_one(a, b)?;
        }
    }
    Ok(())
}

/// `CSL.Engine.prototype.setNumberLabels(Item)`: for a legal item with a
/// `number` (and `consolidate_legal_items` on), pre-seed
/// `state.tmp.shadow_numbers["number"]` with the number text and its
/// embedded label, if any.
pub fn set_number_labels(state: &mut State, item: &Value) {
    let consolidate = state
        .opt
        .get("development_extensions")
        .and_then(|d| d.get("consolidate_legal_items"))
        .map(js::truthy)
        .unwrap_or(false);
    let is_legal = item
        .get("type")
        .and_then(Value::as_str)
        .map(|t| LEGAL_TYPES.contains(&t))
        .unwrap_or(false);
    if !(js::truthy_opt(item.get("number"))
        && is_legal
        && consolidate
        && !state.tmp.shadow_numbers.contains_key("number"))
    {
        return;
    }
    let mut sn = ShadowNumber {
        label: Some(ShadowLabel::False),
        plural: Some(0),
        numeric: Some(false),
        ..ShadowNumber::default()
    };
    // Labels embedded in number variable
    let value = js::to_js_string(item.get("number").unwrap_or(&Value::Null)).replace('\\', "");
    // Get first word, parse out labels only if it parses
    static WS_RE: LazyLock<Regex> = LazyLock::new(|| rx(&format!("[{}]+", js::WS)));
    let firstword = js::split(&WS_RE, &value)
        .into_iter()
        .next()
        .unwrap_or_default();
    let firstlabel = statute_subdiv_string(&firstword);
    if let Some(fl) = firstlabel {
        // Get list and match
        let splt = js::split(&STATUTE_SUBDIV_PLAIN_REGEX, &value);
        let value = if splt.len() > 1 {
            // Convert matches to localized form
            let lst: Vec<String> = splt[1..].iter().map(|s| js::trim(s).to_string()).collect();
            lst.join(" ")
        } else {
            splt[0].clone()
        };
        sn.label = Some(ShadowLabel::Term(fl.to_string()));
        sn.values.push(ShadowValue::Raw(vec![
            Value::String("Blob".into()),
            Value::String(value),
            Value::Bool(false),
        ]));
        sn.numeric = Some(false);
    } else {
        sn.values.push(ShadowValue::Raw(vec![
            Value::String("Blob".into()),
            Value::String(value),
            Value::Bool(false),
        ]));
        sn.numeric = Some(true);
    }
    state.tmp.shadow_numbers.insert("number".into(), sn);
}

#[cfg(test)]
mod tests {
    //! Differential tests against citeproc-js 2.4.63: reference
    //! `tests/data/csl/units/locator.json` (generator
    //! `scripts/csl-units/locator.cjs`):
    //!
    //! * `remapSectionVariable` over ~2,600 (Item, cite item) pairs: legal
    //!   and other types x sections (labelled, unlabelled, odd punctuation)
    //!   x locators x labels;
    //! * `setNumberLabels` (256 items, both `consolidate_legal_items`
    //!   settings, with and without an existing entry);
    //! * `CSL.parseLocator` (40 locators with `|date extra` forms);
    //! * the per-citation-item input steps (`parseLocator`,
    //!   `remapSectionVariable`, `locator_label_parse`) over ~1,000 items x
    //!   locators x labels, in two locales and five option sets, with the
    //!   locale terms citeproc-js read replayed.
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::super::load::parse_locator;
    use super::*;

    const REF: &str = include_str!("../../tests/data/csl/units/locator.json");

    fn obj(v: &Value) -> Obj {
        v.as_object().cloned().unwrap_or_default()
    }

    fn err_text(e: &EngineError) -> String {
        match e {
            EngineError::BadInput(m) => m.clone(),
            o => o.to_string(),
        }
    }

    fn terms_from(log: &Value) -> BTreeMap<String, Option<String>> {
        log.as_object()
            .map(|o| {
                o.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().map(str::to_string)))
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn remap_section_variable_matches_citeproc_js() {
        let r: Value = serde_json::from_str(REF).expect("json");
        let mut n = 0;
        for c in r["remap"].as_array().expect("remap") {
            let (mut item_obj, mut cite) = (obj(&c["Item"]), obj(&c["item"]));
            let res = remap_section_variable_one(&mut item_obj, &mut cite);
            n += 1;
            match (res, c.get("error")) {
                (Ok(()), None) => {
                    assert_eq!(Value::Object(item_obj), c["Item_out"], "Item {c}");
                    assert_eq!(Value::Object(cite), c["item_out"], "item {c}");
                }
                (Err(e), Some(w)) => assert_eq!(w.as_str(), Some(err_text(&e).as_str()), "{c}"),
                (g, w) => panic!("{c}: {g:?} vs {w:?}"),
            }
        }
        assert!(n > 2000);
    }

    #[test]
    fn set_number_labels_matches_citeproc_js() {
        let r: Value = serde_json::from_str(REF).expect("json");
        for c in r["set_number_labels"].as_array().expect("cases") {
            let mut st = State::default();
            st.opt.insert(
                "development_extensions".into(),
                json!({"consolidate_legal_items": c["ext"]}),
            );
            if c["pre"].as_bool() == Some(true) {
                st.tmp
                    .shadow_numbers
                    .insert("number".into(), ShadowNumber::default());
            }
            set_number_labels(&mut st, &c["Item"]);
            let mut got = Obj::new();
            for (k, v) in &st.tmp.shadow_numbers {
                got.insert(k.clone(), v.to_value());
            }
            assert_eq!(Value::Object(got), c["out"], "{c}");
        }
    }

    #[test]
    fn parse_locator_matches_citeproc_js() {
        let r: Value = serde_json::from_str(REF).expect("json");
        for c in r["parse_locator"].as_array().expect("cases") {
            let mut st = State::default();
            st.opt.insert(
                "development_extensions".into(),
                json!({"locator_date_and_revision": c["ext"]}),
            );
            let mut item = obj(&c["item"]);
            parse_locator(&mut st, &mut item);
            assert_eq!(Value::Object(item), c["out"], "{c}");
        }
    }
}
