// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_static_locator.js; plus, in the "citation-item input" section,
//              CSL.parseLocator (src/load.js) and the per-item input steps of
//              makeCitationCluster (src/api_cite.js:131-154)
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
//! The last section, `citation-item input`, holds the input steps
//! `makeCitationCluster` applies to each citation item before rendering
//! (`src/api_cite.js`, the loop over `citation.citationItems`): copy the
//! item, `CSL.parseLocator` (`src/load.js`), `remapSectionVariable`, and the
//! `locator_label_parse` step. They live here, not in `api_cite.rs`, only
//! because the agent that owns this file ported them; the integrator may
//! move them.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::js::{self, Obj};
use super::state::State;
use super::util_number::{
    input_get_term_name, locator_labels_map, statute_subdiv_strings,
    statute_subdiv_strings_reverse, ShadowLabel, ShadowNumber, ShadowValue,
};
use super::{CslResult, EngineError};

/// JS `.`: anything but `\n`, `\r`, U+2028, U+2029.
const DOT: &str = "[^\\n\\r\\u{2028}\\u{2029}]";

/// The alternation of subdivision abbreviations used by the three
/// `STATUTE_SUBDIV_*` / `LOCATOR_LABELS_REGEXP` patterns.
const SUBDIV_ABBREVS: &str = "vrs|sv|subpara|op|subch|add|amend|annot|app|art|bibliog|bk|ch|cl|col|cmt|dec|dept|div|ex|fig|fld|fol|n|hypo|illus|intro|l|no|p|pp|para|pt|pmbl|princ|pub|r|rn|sched|sec|ser|subdiv|subsec|supp|tbl|tit|vol";

fn rx(src: &str) -> Regex {
    Regex::new(src).unwrap_or_else(|e| panic!("invalid static regex {src:?}: {e}"))
}

// DUP-CHECK: load.js CSL.STATUTE_SUBDIV_PLAIN_REGEX_FRONT
// /(?:^\s*[.,;]*\s*(?:vrs|...|vol)\. *)/
static STATUTE_SUBDIV_PLAIN_REGEX_FRONT: LazyLock<Regex> = LazyLock::new(|| {
    rx(&format!(
        "(?:^[{ws}]*[.,;]*[{ws}]*(?:{SUBDIV_ABBREVS})\\. *)",
        ws = js::WS
    ))
});

// DUP-CHECK: load.js CSL.STATUTE_SUBDIV_PLAIN_REGEX
// /(?:(?:^| )(?:vrs|...|vol)\. *)/
static STATUTE_SUBDIV_PLAIN_REGEX: LazyLock<Regex> =
    LazyLock::new(|| rx(&format!("(?:(?:^| )(?:{SUBDIV_ABBREVS})\\. *)")));

// DUP-CHECK: load.js CSL.LOCATOR_LABELS_REGEXP
// new RegExp("^((vrs|...|vol)\\.)\\s+(.*)")
static LOCATOR_LABELS_REGEXP: LazyLock<Regex> = LazyLock::new(|| {
    rx(&format!(
        "^(({SUBDIV_ABBREVS})\\.)[{ws}]+({DOT}*)",
        ws = js::WS
    ))
});

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
        _ => Err(EngineError::Csl(format!("{path}.trim is not a function"))),
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
                    statute_subdiv_strings_reverse(&label).unwrap_or("undefined"),
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
                if ["[", "(", ".", ",", ";", ":", "?"].contains(&js::slice(&locator, 0, Some(1)).as_str()) {
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
    let firstword = js::split(&WS_RE, &value).into_iter().next().unwrap_or_default();
    let firstlabel = statute_subdiv_strings(&firstword);
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

// ---- citation-item input (api_cite.js, load.js) ----

/// `CSL.parseLocator.call(state, item)` (load.js): with
/// `locator_date_and_revision`, split `locator|date rest` into `locator`,
/// `locator-date` (a parsed date object) and `locator-extra`; then strip
/// trailing whitespace from `locator`. `item` is mutated.
pub fn parse_locator(state: &State, item: &mut Obj) {
    let ext = state
        .opt
        .get("development_extensions")
        .and_then(|d| d.get("locator_date_and_revision"))
        .map(js::truthy)
        .unwrap_or(false);
    if ext && js::get_truthy(item, "locator") {
        let locator = js::to_js_string(item.get("locator").unwrap_or(&Value::Null));
        item.insert("locator".into(), Value::String(locator.clone()));
        let idx = js::index_of(&locator, "|", 0);
        if idx > -1 {
            let mut raw_locator = locator.clone();
            item.insert(
                "locator".into(),
                Value::String(js::slice(&raw_locator, 0, Some(idx))),
            );
            raw_locator = js::slice(&raw_locator, idx + 1, None);
            static DATE_RE: LazyLock<Regex> =
                LazyLock::new(|| rx(&format!("^([0-9]{{4}}-[0-9]{{2}}-[0-9]{{2}}){DOT}*")));
            if let Some(m) = DATE_RE.captures(&raw_locator) {
                let m1 = m.get(1).map(|x| x.as_str()).unwrap_or("").to_string();
                item.insert(
                    "locator-date".into(),
                    Value::Object(state.fun.dateparser.parse_date_to_object(&m1)),
                );
                raw_locator = js::slice(&raw_locator, js::len(&m1) as i64, None);
            }
            item.insert(
                "locator-extra".into(),
                Value::String(js::trim(&raw_locator).to_string()),
            );
        }
    }
    if js::get_truthy(item, "locator") {
        let l = js::to_js_string(item.get("locator").unwrap_or(&Value::Null));
        let trimmed = l.trim_end_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
        item.insert("locator".into(), Value::String(trimmed.to_string()));
    }
}

/// The per-citation-item input steps of `makeCitationCluster`
/// (`src/api_cite.js:131-154`) applied to `item` (already a shallow copy of
/// the caller's citation item) and its `Item`:
/// `CSL.parseLocator`; `remapSectionVariable` when
/// `consolidate_legal_items`; and, with `locator_label_parse`, moving an
/// embedded label such as `"ch. 3"` out of a plain locator into
/// `item.label` when the locale has a term for it.
///
/// `Item` may be mutated (`remapSectionVariable` rewrites `Item.section`).
pub fn citation_item_input(state: &State, item_obj: &mut Obj, item: &mut Obj) -> CslResult<()> {
    parse_locator(state, item);
    let ext = |name: &str| {
        state
            .opt
            .get("development_extensions")
            .and_then(|d| d.get(name))
            .map(js::truthy)
            .unwrap_or(false)
    };
    if ext("consolidate_legal_items") {
        remap_section_variable_one(item_obj, item)?;
    }
    if ext("locator_label_parse") {
        let is_legal = item_obj
            .get("type")
            .and_then(Value::as_str)
            .map(|t| LEGAL_TYPES.contains(&t))
            .unwrap_or(false);
        let label_is_page_or_none = !js::get_truthy(item, "label")
            || item.get("label").and_then(Value::as_str) == Some("page");
        if js::get_truthy(item, "locator") && !is_legal && label_is_page_or_none {
            let locator = js::to_js_string(item.get("locator").unwrap_or(&Value::Null));
            if let Some(m) = LOCATOR_LABELS_REGEXP.captures(&locator) {
                let m2 = m.get(2).map(|x| x.as_str()).unwrap_or("");
                let m3 = m.get(3).map(|x| x.as_str()).unwrap_or("");
                let try_label = locator_labels_map(m2);
                if input_get_term_name(state, try_label)
                    .map(|t| !t.is_empty())
                    .unwrap_or(false)
                {
                    item.insert(
                        "label".into(),
                        Value::String(try_label.unwrap_or("").to_string()),
                    );
                    item.insert("locator".into(), Value::String(m3.to_string()));
                }
            }
        }
    }
    Ok(())
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

    use super::*;

    const REF: &str = include_str!("../../tests/data/csl/units/locator.json");

    fn obj(v: &Value) -> Obj {
        v.as_object().cloned().unwrap_or_default()
    }

    fn err_text(e: &EngineError) -> String {
        match e {
            EngineError::Csl(m) => m.clone(),
            o => o.to_string(),
        }
    }

    fn terms_from(log: &Value) -> BTreeMap<String, Option<String>> {
        log.as_object()
            .map(|o| o.iter().map(|(k, v)| (k.clone(), v.as_str().map(str::to_string))).collect())
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
                st.tmp.shadow_numbers.insert("number".into(), ShadowNumber::default());
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
            parse_locator(&st, &mut item);
            assert_eq!(Value::Object(item), c["out"], "{c}");
        }
    }

    #[test]
    fn citation_item_input_matches_citeproc_js() {
        let r: Value = serde_json::from_str(REF).expect("json");
        let mut n = 0;
        for c in r["citation_item_input"].as_array().expect("cases") {
            let e = &r["cii_engines"][c["engine"].as_str().unwrap_or("")];
            let mut st = State::default();
            st.opt = obj(&e["opt"]);
            st.input_locale.terms = terms_from(&e["log"]);
            let (mut item_obj, mut item) = (obj(&c["Item"]), obj(&c["ci"]));
            let res = citation_item_input(&st, &mut item_obj, &mut item);
            n += 1;
            match (res, c.get("error")) {
                (Ok(()), None) => {
                    assert_eq!(Value::Object(item), c["item_out"], "item {c}");
                    assert_eq!(Value::Object(item_obj), c["Item_out"], "Item {c}");
                }
                (Err(e), Some(w)) => assert_eq!(w.as_str(), Some(err_text(&e).as_str()), "{c}"),
                (g, w) => panic!("{c}: {g:?} vs {w:?}"),
            }
        }
        assert!(n > 800);
    }
}
