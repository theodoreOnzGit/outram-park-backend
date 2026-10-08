// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/attributes.js
// Version:     2.4.63, commit 73bc1b44bc7d54d0bfec4e070fd27f5efe024ff9
// Copyright:   (c) 2009-2019 Frank Bennett
// Licence:     AGPL-3.0, taken from upstream's "CPAL-1.0 or AGPL-3.0-or-later"
//              (LICENSE at the commit above; see this crate's NOTICE).
// Modified:    2026-10-08, by the OUTRAM PARK contributors. This file is a
//              Rust translation (port) of the file named above, modified
//              from the original.
// No warranty: this program is distributed in the hope that it will be
//              useful, but WITHOUT ANY WARRANTY; without even the implied
//              warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
//              PURPOSE. See the GNU Affero General Public License.

//! Port of `src/attributes.js`: `CSL.Attributes["@name"]`, the handlers that
//! turn each CSL attribute into token properties and token closures while the
//! style is compiled (`CSL.XmlToToken`, util_nodes.js, calls [`apply`]).
//!
//! Attributes that only set a token property or an engine option are fully
//! ported. Attributes that add behaviour push [`AttributesExec`] /
//! [`AttributesTest`] variants (same count and order as citeproc-js). Their
//! bodies are ported where they need only state, token and item; the others
//! carry a `PORT-LATER` marker naming the machinery they wait for.
//!
//! **Token mutation at run time.** Several upstream closures mutate `this`
//! (the token) while a style runs: `@variable`'s first closure rewrites
//! `this.variables` in place, `@text-case` rewrites `this.strings`.
//! `Exec::run` receives `&mut Token` (the engine takes the token out of its
//! list while it runs, see `State::token_exec`), and all three bodies are ported.
//! `@variable`'s second closure ("check for output") mutates the *item* for
//! a string `authority` or `committee`; that write goes through
//! `util_transform::set_item_prop` (see its module docs).

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::exec::{Exec, Test};
use super::js::{self, Obj};
use super::util_number::{process_number, ShadowLabel, ShadowNumber, ShadowValue};
use super::load::{
    dev_ext_truthy, position_map, DATE_VARIABLES, DESCENDING, GIVENNAME_DISAMBIGUATION_RULES,
    NUMERIC_VARIABLES, POSITION,
};
use super::obj_token::{Token, TokenType};
use super::queue::{self, FormatRef, QueueId};
use super::state::{Area, State};
use super::util_transform::{get_item_prop, set_item_prop};
use super::util_locale::{locale_resolve, LangSpec};
use super::{CslResult, EngineError};
// ---------------------------------------------------------------------------
// Small JS helpers.
// ---------------------------------------------------------------------------

static WS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("[{}]+", js::WS)).expect("static regex"));

/// JS `arg.split(/\s+/)`.
pub fn split_ws(arg: &str) -> Vec<String> {
    js::split(&WS_RE, arg)
}

/// A JS number that may be NaN, as a JSON value (`NaN` serialises as `null`).
pub fn int_or_nan(n: Option<i64>) -> Value {
    match n {
        Some(i) => Value::from(i),
        None => Value::Null,
    }
}

/// JS `parseFloat(s)` as a JSON value: an integer where integral, `null` for
/// NaN.
pub fn parse_float_value(s: &str) -> Value {
    let t = s.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
    let b = t.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let ds = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let mut digits = i - ds;
    if i < b.len() && b[i] == b'.' {
        let mut j = i + 1;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        digits += j - (i + 1);
        i = j;
    }
    if digits == 0 {
        return Value::Null;
    }
    match t[..i].parse::<f64>() {
        Ok(f) if f == f.trunc() && f.abs() < 9.0e15 => Value::from(f as i64),
        Ok(f) => serde_json::Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        Err(_) => Value::Null,
    }
}

/// JS `"" + item[key]`: `"undefined"` when absent.
fn str_of(item: &Value, key: &str) -> String {
    match item.get(key) {
        Some(v) => js::to_js_string(v),
        None => "undefined".to_string(),
    }
}

/// `item[key]` truthiness (`item` may be `null`/absent: falsy).
fn item_truthy(item: &Value, key: &str) -> bool {
    js::truthy_opt(item.get(key))
}

/// JS `x` is an object with truthy-valued own entry (for `for (key in x)`).
fn any_truthy_entry(v: &Value) -> bool {
    match v {
        Value::Array(a) => a.iter().any(js::truthy),
        Value::Object(o) => o.values().any(js::truthy),
        _ => false,
    }
}

/// `state[name]` for the five areas (`citation`, `bibliography`, `intext`,
/// `citation_sort`, `bibliography_sort`); any other name is the JS
/// `TypeError` of reading `.opt` of `undefined`.
pub fn area_ref<'s>(state: &'s State, name: &str) -> CslResult<&'s Area> {
    match name {
        "citation" => Ok(&state.citation),
        "bibliography" => Ok(&state.bibliography),
        "intext" => Ok(&state.intext),
        "citation_sort" => Ok(&state.citation_sort),
        "bibliography_sort" => Ok(&state.bibliography_sort),
        other => Err(EngineError::Csl(format!(
            "TypeError: state.{other} is undefined"
        ))),
    }
}

/// Mutable [`area_ref`].
pub fn area_mut<'s>(state: &'s mut State, name: &str) -> CslResult<&'s mut Area> {
    match name {
        "citation" => Ok(&mut state.citation),
        "bibliography" => Ok(&mut state.bibliography),
        "intext" => Ok(&mut state.intext),
        "citation_sort" => Ok(&mut state.citation_sort),
        "bibliography_sort" => Ok(&mut state.bibliography_sort),
        other => Err(EngineError::Csl(format!(
            "TypeError: state.{other} is undefined"
        ))),
    }
}

/// `obj[key]` as a mutable JSON object, created when absent or not an object.
pub fn obj_entry<'o>(obj: &'o mut Obj, key: &str) -> &'o mut Obj {
    let slot = obj
        .entry(key.to_string())
        .or_insert_with(|| Value::Object(Obj::new()));
    if !slot.is_object() {
        *slot = Value::Object(Obj::new());
    }
    slot.as_object_mut().expect("slot was just made an object")
}

/// `obj[key]` as a mutable JSON array, created when absent or not an array.
pub fn arr_entry<'o>(obj: &'o mut Obj, key: &str) -> &'o mut Vec<Value> {
    let slot = obj
        .entry(key.to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    if !slot.is_array() {
        *slot = Value::Array(Vec::new());
    }
    slot.as_array_mut().expect("slot was just made an array")
}

/// `state.opt["default-locale"][0]` (`"en-US"` when absent).
pub fn default_locale(state: &State) -> String {
    state
        .opt
        .get("default-locale")
        .and_then(|v| v.get(0))
        .and_then(|v| v.as_str())
        .unwrap_or("en-US")
        .to_string()
}

/// `state.tmp.shadow_numbers[variable]` after `processNumber`; reading a
/// property of it when it is missing is a TypeError upstream.
fn shadow<'s>(state: &'s State, variable: &str) -> CslResult<&'s ShadowNumber> {
    state.tmp.shadow_numbers.get(variable).ok_or_else(|| {
        EngineError::BadInput(format!(
            "Cannot read properties of undefined (reading '{variable}')"
        ))
    })
}

/// A shadow number's `label` as JS truthiness sees it: `None` for `false`.
fn shadow_label(sn: &ShadowNumber) -> Option<String> {
    match &sn.label {
        Some(ShadowLabel::Term(t)) if !t.is_empty() => Some(t.clone()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Closures.
// ---------------------------------------------------------------------------

/// The closures `src/attributes.js` stores in `token.execs` (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum AttributesExec {
    /// `@variable` on names/date/text/number: "set variable names"
    /// (attributes.js:~150): rewrites `this.variables` from `variables_real`,
    /// skipping `done_vars`.
    VariableSetNames,
    /// `@variable` on names/date/text/number: "check for output"
    /// (attributes.js:~162-300).
    VariableCheckOutput,
    /// `@text-case` (attributes.js:1521): sets `this.strings["text-case"]`.
    TextCase {
        /// The attribute value.
        arg: String,
    },
}

impl AttributesExec {
    /// Run the closure.
    pub fn run(
        &self,
        state: &mut State,
        token: &mut Token,
        item: &Value,
        cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            // attributes.js:~150-165. The `variables_real` list is kept in
            // `token.extra`; `this.variables` is cleared in place and refilled
            // with the variables that are not already in `done_vars`.
            AttributesExec::VariableSetNames => {
                let real: Vec<String> = match token.extra.get("variables_real") {
                    Some(Value::Array(a)) => a.iter().map(js::to_js_string).collect(),
                    _ => Vec::new(),
                };
                token.variables.clear();
                for v in &real {
                    // set variable name if not quashed
                    if !state.tmp.done_vars.contains(v) {
                        token.variables.push(v.clone());
                    }
                    if state.tmp.can_block_substitute {
                        state.tmp.done_vars.push(v.clone());
                    }
                }
                Ok(None)
            }
            AttributesExec::VariableCheckOutput => {
                variable_check_output(state, token, item, cite_item)?;
                Ok(None)
            }
            // attributes.js:1493-1503.
            AttributesExec::TextCase { arg } => {
                if arg == "normal" {
                    token
                        .extra
                        .insert("text_case_normal".into(), Value::Bool(true));
                } else {
                    token.set_string("text-case", arg);
                    if arg == "title" && item_truthy(item, "jurisdiction") {
                        token.set_string("text-case", "passthrough");
                    }
                }
                Ok(None)
            }
        }
    }
}

/// The "check for output" closure of `@variable` on `cs:names`, `cs:date`,
/// `cs:text` and `cs:number` (attributes.js:166-300): decide whether the
/// element will render something, and raise the group flags accordingly.
fn variable_check_output(
    state: &mut State,
    token: &mut Token,
    item: &Value,
    cite_item: &Value,
) -> CslResult<()> {
    let has_cite = js::truthy(cite_item);
    let mut output = false;
    let variables: Vec<String> = token.variables.clone();
    for v0 in &variables {
        let mut variable = v0.clone();
        // `Item[variable]` as the closure sees it: a split of authority or
        // committee rewrites it (see util_transform's module docs).
        let mut cur: Option<Value> = item.get(variable.as_str()).cloned();
        if (variable == "authority" || variable == "committee")
            && matches!(cur, Some(Value::String(_)))
            && token.name == "names"
        {
            // Great! So for each of these, we split.
            // And we only recombine everything if the length
            // of all the splits matches.

            // Preflight
            let mut is_valid = true;
            static SEMI_RE: LazyLock<Regex> = LazyLock::new(|| {
                #[allow(clippy::expect_used)]
                Regex::new(&format!("[{ws}]*;[{ws}]*", ws = js::WS)).expect("static regex")
            });
            let text = js::to_js_string(cur.as_ref().unwrap_or(&Value::Null));
            let mut raw_names: Vec<String> = js::split(&SEMI_RE, &text);
            let multi_keys: Option<Value> = item
                .get("multi")
                .filter(|m| js::truthy(m))
                .map(|m| m.get("_keys").cloned().unwrap_or(Value::Null));
            let var_keys: Option<Value> = match &multi_keys {
                Some(Value::Null) => {
                    return Err(EngineError::BadInput(format!(
                        "Cannot read properties of undefined (reading '{variable}')"
                    )))
                }
                Some(k) => k.get(variable.as_str()).filter(|v| js::truthy(v)).cloned(),
                None => None,
            };
            // langTag -> list (split) or the single string (invalid case)
            let mut raw_multi: Vec<(String, Vec<String>)> = Vec::new();
            if let Some(Value::Object(by_lang)) = &var_keys {
                for (lang_tag, v) in by_lang {
                    let parts = js::split(&SEMI_RE, &js::to_js_string(v));
                    let n = parts.len();
                    raw_multi.push((lang_tag.clone(), parts));
                    if n != raw_names.len() {
                        is_valid = false;
                        break;
                    }
                }
            }
            if !is_valid {
                raw_names = vec![text.clone()];
                // `rawMultiNames = Item.multi._keys[variable]`: the strings
                // themselves, which `rawMultiNames[langTag][j]` then indexes
                // by character.
                raw_multi = match &var_keys {
                    Some(Value::Object(by_lang)) => by_lang
                        .iter()
                        .map(|(k, v)| {
                            let s = js::to_js_string(v);
                            let chars: Vec<String> = (0..js::len(&s) as i64)
                                .map(|i| js::char_at(&s, i))
                                .collect();
                            (k.clone(), chars)
                        })
                        .collect(),
                    _ => Vec::new(),
                };
            }
            let mut names: Vec<Value> = Vec::new();
            for (j, name) in raw_names.iter().enumerate() {
                let mut key_obj = js::Obj::new();
                for (lang_tag, list) in &raw_multi {
                    let mut child = js::Obj::new();
                    if let Some(lit) = list.get(j) {
                        child.insert("literal".into(), Value::String(lit.clone()));
                    }
                    key_obj.insert(lang_tag.clone(), Value::Object(child));
                }
                let mut multi = js::Obj::new();
                multi.insert("_key".into(), Value::Object(key_obj));
                let mut parent = js::Obj::new();
                parent.insert("literal".into(), Value::String(name.clone()));
                parent.insert("multi".into(), Value::Object(multi));
                names.push(Value::Object(parent));
            }
            let arr = Value::Array(names);
            set_item_prop(state, item, &variable, arr.clone());
            cur = Some(arr);
        }
        if token.string_opt("form").as_deref() == Some("short") && !js::truthy_opt(cur.as_ref()) {
            if variable == "title" {
                variable = "title-short".to_string();
            } else if variable == "container-title" {
                variable = "container-title-short".to_string();
            }
            cur = item.get(variable.as_str()).cloned();
        }
        if variable == "year-suffix" {
            // year-suffix always signals that it produces output,
            // even when it doesn't. This permits it to be used with
            // the "no date" term inside a group used exclusively
            // to control formatting.
            output = true;
            break;
        } else if DATE_VARIABLES.contains(&variable.as_str()) {
            if dev_ext_truthy(state, "locator_date_and_revision") && variable == "locator-date" {
                // If locator-date is set, it's valid.
                output = true;
                break;
            }
            if let Some(d) = cur.as_ref().filter(|d| js::truthy(d)) {
                let dateparts: Vec<String> = match token.extra.get("dateparts") {
                    Some(Value::Array(a)) => a.iter().map(js::to_js_string).collect(),
                    _ => {
                        return Err(EngineError::BadInput(
                            "Cannot read properties of undefined (reading 'indexOf')".into(),
                        ))
                    }
                };
                if let Value::Object(o) = d {
                    for (key, val) in o {
                        if !dateparts.contains(key) && key != "literal" {
                            continue;
                        }
                        if js::truthy(val) {
                            output = true;
                            break;
                        }
                    }
                }
                if output {
                    break;
                }
            }
        } else if variable == "locator" {
            if has_cite && js::truthy_opt(cite_item.get("locator")) {
                output = true;
            }
            break;
        } else if variable == "locator-extra" {
            if has_cite && js::truthy_opt(cite_item.get("locator-extra")) {
                output = true;
            }
            break;
        } else if variable == "citation-number" || variable == "citation-label" {
            output = true;
            break;
        } else if variable == "first-reference-note-number" {
            if has_cite && js::truthy_opt(cite_item.get("first-reference-note-number")) {
                output = true;
            }
            break;
        } else if variable == "first-container-reference-note-number" {
            if has_cite && js::truthy_opt(cite_item.get("first-container-reference-note-number")) {
                output = true;
            }
            break;
        } else if variable == "hereinafter" {
            let id = item.get("id");
            if js::truthy_opt(id)
                && state
                    .transform
                    .abbrev(
                        "default",
                        "hereinafter",
                        &id.map(js::to_js_string).unwrap_or_default(),
                    )
                    .map(|a| !a.is_empty())
                    .unwrap_or(false)
            {
                output = true;
            }
            break;
        } else if matches!(
            cur,
            Some(Value::Object(_)) | Some(Value::Array(_)) | Some(Value::Null)
        ) {
            break;
        } else if matches!(&cur, Some(Value::String(s)) if !s.is_empty()) {
            output = true;
            break;
        } else if matches!(cur, Some(Value::Number(_))) {
            output = true;
            break;
        }
        if output {
            break;
        }
    }
    if output {
        let real: Vec<String> = match token.extra.get("variables_real") {
            Some(Value::Array(a)) => a.iter().map(js::to_js_string).collect(),
            _ => Vec::new(),
        };
        for variable in &real {
            if variable != "citation-number" || state.tmp.area != "bibliography" {
                state.tmp.cite_renders_content = true;
            }
            super::node_group::tip_mut(state)?.variable_success = true;
            // For util_substitute.js, subsequent-author-substitute
            if state
                .tmp
                .can_substitute
                .value()
                .map(js::truthy)
                .unwrap_or(false)
                && state.tmp.area == "bibliography"
                && matches!(get_item_prop(state, item, variable), Some(Value::String(_)))
            {
                state.tmp.name_node.top = queue::current(state, QueueId::Output);
                let value = get_item_prop(state, item, variable).unwrap_or(Value::Null);
                match state.tmp.rendered_name.as_mut() {
                    Some(v) => v.push(value),
                    None => {
                        return Err(EngineError::Csl(
                            "TypeError: Cannot read properties of false (reading 'push')".into(),
                        ))
                    }
                }
            }
        }
        state
            .tmp
            .can_substitute
            .replace_literal(Value::Bool(false))?;
    } else {
        super::node_group::tip_mut(state)?.variable_attempt = true;
    }
    Ok(())
}

/// The condition closures `src/attributes.js` stores in `token.tests` /
/// `token.test` (PORTING.md §4). Each variant is one `maketest(...)` closure
/// with its captured arguments; all take `(Item, item)` upstream.
#[derive(Debug, Clone, PartialEq)]
pub enum AttributesTest {
    /// `@disambiguate="true"` (attributes.js:6-24).
    Disambiguate,
    /// `@disambiguate="check-ambiguity-and-backreference"` (attributes.js:25-31).
    DisambiguateBackref,
    /// `@is-numeric` for one variable (attributes.js:36-62).
    IsNumeric {
        /// The variable.
        variable: String,
    },
    /// `@is-uncertain-date` for one variable.
    IsUncertainDate {
        /// The variable.
        variable: String,
    },
    /// `@locator` for one label.
    Locator {
        /// The label tried.
        trylabel: String,
    },
    /// `testSubsequentNear` (attributes.js:117).
    PositionNear,
    /// `testSubsequentNotNear` (attributes.js:123).
    PositionFar,
    /// `@position` for one position keyword, mapped to `CSL.POSITION_*`
    /// (`None`: a keyword upstream does not map, which never matches).
    Position {
        /// `tryposition`.
        tryposition: Option<i64>,
    },
    /// `@type` for one type (nested inside a `match.any`).
    Type {
        /// The type tried.
        mytype: String,
    },
    /// `@variable` on `cs:if` / `cs:else-if` / `cs:condition`.
    Variable {
        /// The variable.
        variable: String,
    },
    /// `@page` for one label.
    Page {
        /// The label tried.
        trylabel: String,
    },
    /// `@number` for one label.
    Number {
        /// The label tried.
        trylabel: String,
    },
    /// `@jurisdiction` (a list, one closure).
    Jurisdiction {
        /// The jurisdictions tried.
        tryjurisdictions: Vec<String>,
    },
    /// `@country` (a list, one closure).
    Country {
        /// The countries tried.
        trycountries: Vec<String>,
    },
    /// `@context`.
    Context {
        /// The attribute value.
        arg: String,
    },
    /// `@has-year-only` for one date variable.
    HasYearOnly {
        /// The date variable.
        trydate: String,
    },
    /// `@has-to-month-or-season` for one date variable.
    HasToMonthOrSeason {
        /// The date variable.
        trydate: String,
    },
    /// `@has-day` for one date variable.
    HasDay {
        /// The date variable.
        trydate: String,
    },
    /// `@is-plural`.
    IsPlural {
        /// The names variable.
        arg: String,
    },
    /// `@is-multiple`.
    IsMultiple {
        /// The variable.
        arg: String,
    },
    /// `@locale` on cs:if / cs:else-if (attributes.js:~700): the
    /// `maketest(locale_list, locale_default, locale_bares)` closure.
    Locale {
        /// `locale_list`.
        locale_list: Vec<LangSpec>,
        /// `locale_default`.
        locale_default: String,
        /// `locale_bares`.
        locale_bares: Vec<String>,
    },
    /// `@alternative-node-internal`.
    AlternativeNodeInternal,
    /// `@locale-internal` (attributes.js:~790): `maketest(me)`.
    LocaleInternal {
        /// `me.locale_list`.
        locale_list: Vec<LangSpec>,
        /// `me.locale_bares`.
        locale_bares: Vec<String>,
        /// `me.locale`.
        locale: String,
    },
    /// `@court-class` for one class.
    CourtClass {
        /// The class tried.
        tryclass: String,
    },
    /// `@container-multiple`.
    ContainerMultiple {
        /// `"true" === arg`.
        retval: bool,
    },
    /// `@container-subsequent`.
    ContainerSubsequent {
        /// `"true" === arg`.
        retval: bool,
    },
    /// `@has-subunit`.
    HasSubunit {
        /// The names variable.
        namevar: String,
    },
    /// `@cite-form`.
    CiteForm {
        /// The cite form tried.
        cite_form: String,
    },
}

static IS_MULTIPLE_RE: LazyLock<Regex> = LazyLock::new(|| {
    let ws = js::WS;
    Regex::new(&format!(
        "(?:,[{ws}]|[{ws}](?:tot[{ws}]en[{ws}]met|l\u{012b}dz|oraz|and|bis|\u{03ad}\u{03c9}\u{03c2}|\
         \u{03ba}\u{03b1}\u{03b9}|och|a\u{017e}|do|en|et|in|ir|ja|og|sa|to|un|und|\u{00e9}s|\
         \u{0219}i|i|u|y|\u{00e0}|e|a|\u{0438}|-|\u{2013})[{ws}]|\u{2014}|&)"
    ))
    .expect("static regex")
});

/// JS `Number(v)` for the values `disambiguate` takes (`false`, `true`,
/// numbers): `NaN` otherwise.
fn js_number(v: &Value) -> f64 {
    match v {
        Value::Bool(b) => f64::from(u8::from(*b)),
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        _ => f64::NAN,
    }
}

impl AttributesTest {
    /// Evaluate the condition: `test(Item, item)`.
    pub fn eval(
        &self,
        state: &mut State,
        _token: &mut Token,
        item: &Value,
        cite_item: &Value,
    ) -> CslResult<bool> {
        match self {
            // attributes.js:6-24.
            AttributesTest::Disambiguate => {
                let id = super::registry::id_key(item.get("id"));
                if state.tmp.area == "bibliography" {
                    let reg_disambig = state
                        .registry
                        .registry
                        .get(&id)
                        .and_then(|t| t.disambig)
                        .ok_or_else(|| {
                            EngineError::Csl(
                                "TypeError: Cannot read properties of undefined (reading 'disambig')"
                                    .to_string(),
                            )
                        })?;
                    let bound = js_number(&state.ambig(reg_disambig).disambiguate);
                    if (state.tmp.disambiguate_count as f64) < bound {
                        state.tmp.disambiguate_count += 1;
                        return Ok(true);
                    }
                } else {
                    state.tmp.disambiguate_max_max += 1;
                    let settings = state.disambig_settings().disambiguate.clone();
                    if js::truthy(&settings)
                        && (state.tmp.disambiguate_count as f64) < js_number(&settings)
                    {
                        state.tmp.disambiguate_count += 1;
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            // attributes.js:25-31.
            AttributesTest::DisambiguateBackref => {
                let id = super::registry::id_key(item.get("id"));
                let token = state.registry.registry.get(&id).ok_or_else(|| {
                    EngineError::Csl(
                        "TypeError: Cannot read properties of undefined (reading 'disambig')"
                            .to_string(),
                    )
                })?;
                let disambig = token.disambig.ok_or_else(|| {
                    EngineError::Csl(
                        "TypeError: Cannot read properties of false (reading 'disambiguate')"
                            .to_string(),
                    )
                })?;
                Ok(js::truthy(&state.ambig(disambig).disambiguate)
                    && token.citation_count.map(|c| c > 1).unwrap_or(false))
            }
            // attributes.js:41-61.
            AttributesTest::IsNumeric { variable } => {
                let use_cite =
                    js::truthy(cite_item) && (variable == "locator" || variable == "locator-extra");
                let myitem = if use_cite { cite_item } else { item };
                let Some(val) = myitem.get(variable.as_str()).filter(|v| js::truthy(v)) else {
                    return Ok(false);
                };
                if NUMERIC_VARIABLES.contains(&variable.as_str()) {
                    if !state.tmp.shadow_numbers.contains_key(variable.as_str()) {
                        let myitem = myitem.clone();
                        process_number(state, None, Some(&myitem), variable)?;
                    }
                    Ok(shadow(state, variable)?.numeric == Some(true))
                } else if variable == "title" || variable == "version" {
                    // myitem[variable].slice(-1) === "" + parseInt(...slice(-1), 10)
                    let Value::String(s) = val else {
                        return Err(EngineError::BadInput(format!(
                            "myitem[variable].slice is not a function ({variable})"
                        )));
                    };
                    let last = js::slice(s, -1, None);
                    Ok(js::parse_int(&last).map(|n| n.to_string()) == Some(last))
                } else {
                    Ok(false)
                }
            }
            AttributesTest::IsUncertainDate { variable } => {
                // Item[v] && Item[v].circa
                Ok(item
                    .get(variable.as_str())
                    .map(|d| js::truthy(d) && js::truthy_opt(d.get("circa")))
                    .unwrap_or(false))
            }
            // attributes.js:95-104: the label of the cite's locator.
            AttributesTest::Locator { trylabel } => {
                let myitem = cite_item.clone();
                process_number(state, None, Some(&myitem), "locator")?;
                let label = shadow_label(shadow(state, "locator")?);
                Ok(label.as_deref() == Some(trylabel.as_str()))
            }
            AttributesTest::PositionNear => {
                // item && MAP[item.position] >= MAP[SUBSEQUENT(1)] && item["near-note"]
                let pos = cite_item.get("position").and_then(Value::as_i64);
                Ok(js::truthy(cite_item)
                    && position_map_ge(pos, 1)
                    && item_truthy(cite_item, "near-note"))
            }
            AttributesTest::PositionFar => {
                let pos = cite_item.get("position").and_then(Value::as_i64);
                Ok(js::truthy(cite_item)
                    && pos.and_then(position_map) == position_map(1)
                    && pos.is_some()
                    && !item_truthy(cite_item, "near-note"))
            }
            AttributesTest::Position { tryposition } => {
                if state.tmp.area == "bibliography" {
                    return Ok(false);
                }
                // JS sets `item.position = 0` when undefined; the result below is
                // the same whether or not that write happens (see module docs).
                let has_item = js::truthy(cite_item);
                let pos_value = cite_item.get("position");
                let numeric_pos = match pos_value {
                    None => Some(0),
                    Some(Value::Number(n)) => n.as_i64(),
                    _ => None,
                };
                let pos_is_number = has_item && matches!(pos_value, None | Some(Value::Number(_)));
                if pos_is_number {
                    let p = numeric_pos;
                    if p == Some(0) && *tryposition == Some(0) {
                        return Ok(true);
                    }
                    if let (Some(t), Some(pp)) = (tryposition, p) {
                        if *t > 0 {
                            if let (Some(a), Some(b)) = (position_map(pp), position_map(*t)) {
                                if a >= b {
                                    return Ok(true);
                                }
                            }
                        }
                    }
                } else if *tryposition == Some(0) {
                    return Ok(true);
                }
                Ok(false)
            }
            AttributesTest::Type { mytype } => Ok(item
                .get("type")
                .map(|t| t.as_str() == Some(mytype.as_str()))
                .unwrap_or(false)),
            AttributesTest::Variable { variable } => {
                let use_cite = matches!(
                    variable.as_str(),
                    "locator"
                        | "locator-extra"
                        | "first-reference-note-number"
                        | "first-container-reference-note-number"
                        | "locator-date"
                ) && js::truthy(cite_item);
                let myitem = if use_cite { cite_item } else { item };
                if variable == "hereinafter" && js::truthy_opt(myitem.get("id")) {
                    // We don't run loadAbbreviation() here; it is run by the
                    // application-supplied retrieveItem() if hereinafter
                    // functionality is to be used, so this key will always
                    // exist in memory, possibly with a nil value.
                    let id = myitem.get("id").map(js::to_js_string).unwrap_or_default();
                    return Ok(state
                        .transform
                        .abbrev("default", "hereinafter", &id)
                        .map(|a| !a.is_empty())
                        .unwrap_or(false));
                }
                match myitem.get(variable.as_str()) {
                    Some(v) if js::truthy(v) => match v {
                        Value::Number(_) | Value::String(_) => Ok(true),
                        Value::Array(_) | Value::Object(_) => Ok(any_truthy_entry(v)),
                        _ => Ok(false),
                    },
                    _ => Ok(false),
                }
            }
            // attributes.js:428-450.
            AttributesTest::Page { trylabel } => {
                let myitem = item.clone();
                process_number(state, None, Some(&myitem), "page")?;
                let label = match shadow_label(shadow(state, "page")?) {
                    None => "page".to_string(),
                    Some(l) if l == "sub verbo" => "sub-verbo".to_string(),
                    Some(l) => l,
                };
                if let Some(sn) = state.tmp.shadow_numbers.get_mut("page") {
                    if let Some(ShadowValue::Info(v0)) = sn.values.first_mut() {
                        if v0.gotosleepability == Some(true) {
                            v0.label_visibility = Some(false);
                        }
                    }
                }
                Ok(*trylabel == label)
            }
            // attributes.js:458-472.
            AttributesTest::Number { trylabel } => {
                let myitem = item.clone();
                process_number(state, None, Some(&myitem), "number")?;
                let label =
                    shadow_label(shadow(state, "number")?).unwrap_or_else(|| "number".to_string());
                Ok(*trylabel == label)
            }
            AttributesTest::Jurisdiction { tryjurisdictions } => {
                Ok(match item.get("jurisdiction").filter(|v| js::truthy(v)) {
                    Some(Value::String(j)) => tryjurisdictions.iter().any(|t| t == j),
                    _ => false,
                })
            }
            AttributesTest::Country { trycountries } => {
                Ok(match item.get("country").filter(|v| js::truthy(v)) {
                    Some(Value::String(c)) => trycountries.iter().any(|t| t == c),
                    _ => false,
                })
            }
            AttributesTest::Context { arg } => {
                if arg == "bibliography" || arg == "citation" {
                    let area = js::slice(&state.tmp.area, 0, Some(js::len(arg) as i64));
                    Ok(area == *arg)
                } else if arg == "alternative" {
                    Ok(state.tmp.abort_alternative)
                } else {
                    Ok(false)
                }
            }
            AttributesTest::HasYearOnly { trydate } => {
                let date = item.get(trydate.as_str());
                Ok(match date {
                    Some(d) if js::truthy(d) => {
                        !(js::truthy_opt(d.get("month")) || js::truthy_opt(d.get("season")))
                    }
                    _ => false,
                })
            }
            AttributesTest::HasToMonthOrSeason { trydate } => {
                let date = item.get(trydate.as_str());
                Ok(match date {
                    Some(d) if js::truthy(d) => {
                        (js::truthy_opt(d.get("month")) || js::truthy_opt(d.get("season")))
                            && !js::truthy_opt(d.get("day"))
                    }
                    _ => false,
                })
            }
            AttributesTest::HasDay { trydate } => Ok(match item.get(trydate.as_str()) {
                Some(d) if js::truthy(d) => js::truthy_opt(d.get("day")),
                _ => false,
            }),
            AttributesTest::IsPlural { arg } => {
                let spoof = state
                    .opt
                    .get("development_extensions")
                    .map(|d| js::truthy_opt(d.get("spoof_institutional_affiliations")))
                    .unwrap_or(false);
                if let Some(Value::Array(list)) = item.get(arg.as_str()) {
                    let mut persons = 0;
                    let mut institutions = 0;
                    let mut last_is_person = false;
                    for n in list {
                        if spoof
                            && (js::truthy_opt(n.get("literal"))
                                || (js::truthy_opt(n.get("isInstitution"))
                                    && js::truthy_opt(n.get("family"))
                                    && !js::truthy_opt(n.get("given"))))
                        {
                            institutions += 1;
                            last_is_person = false;
                        } else {
                            persons += 1;
                            last_is_person = true;
                        }
                    }
                    if persons > 1 || institutions > 1 || (institutions > 0 && last_is_person) {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            AttributesTest::IsMultiple { arg } => {
                // JS: ("" + Item[arg]).split(re).length > 1; the pattern has no
                // captures and cannot match empty, so that is "re matches".
                let val = str_of(item, arg);
                Ok(IS_MULTIPLE_RE.is_match(&val))
            }
            AttributesTest::Locale {
                locale_list,
                locale_default,
                locale_bares,
            } => {
                let lang = match item.get("language").filter(|v| js::truthy(v)) {
                    None => locale_default.clone(),
                    Some(v) => js::to_js_string(v),
                };
                let langspec = locale_resolve(&lang, Some(locale_default));
                let mut res = false;
                for l in locale_list {
                    if langspec.best == l.best {
                        push_lang_state(state);
                        set_lang(state, &locale_list[0].best);
                        res = true;
                        break;
                    }
                }
                if !res && locale_bares.contains(&langspec.bare) {
                    push_lang_state(state);
                    set_lang(state, &locale_list[0].best);
                    res = true;
                }
                Ok(res)
            }
            AttributesTest::AlternativeNodeInternal => Ok(!state.tmp.abort_alternative),
            AttributesTest::LocaleInternal {
                locale_list,
                locale_bares,
                locale,
            } => {
                let mut res = false;
                let default_locale = default_locale(state);
                let mut langspec: Option<LangSpec> = None;
                if js::truthy_opt(item.get("language")) {
                    let lang = item
                        .get("language")
                        .map(js::to_js_string)
                        .unwrap_or_default();
                    let ls = locale_resolve(&lang, Some(&default_locale));
                    if ls.best != default_locale {
                        langspec = Some(ls);
                    }
                }
                if let Some(ls) = langspec {
                    // We attempt to match a specific locale from the
                    // list of parameters.  If that fails, we fall back
                    // to the base locale of the first element.  The
                    // locale applied is always the first local
                    // in the list of parameters (or base locale, for a
                    // single two-character language code)
                    let matched = locale_list.iter().any(|l| ls.best == l.best)
                        || locale_bares.contains(&ls.bare);
                    if matched {
                        set_lang(state, locale);
                        state.tmp.last_cite_locale = Some(locale.clone());
                        // Set empty group open tag with locale set marker
                        queue::open_level(state, QueueId::Output, FormatRef::Name("empty".into()))?;
                        if let Some(cur) = queue::current(state, QueueId::Output) {
                            state.blobs.get_mut(cur).new_locale = Some(locale.clone());
                        }
                        res = true;
                    }
                }
                Ok(res)
            }
            AttributesTest::CourtClass { tryclass } => {
                // (called as a method of CSL, so `this.lang` is undefined)
                let cls = super::load::get_court_class(state, None, item, false);
                Ok(cls == *tryclass)
            }
            AttributesTest::ContainerMultiple { retval } => {
                let key = str_of(item, "container_id");
                let n = state
                    .tmp
                    .container_item_count
                    .get(&key)
                    .filter(|v| js::truthy(v))
                    .and_then(Value::as_f64);
                Ok(match n {
                    None => !*retval,
                    Some(c) if c > 1.0 => *retval,
                    Some(_) => !*retval,
                })
            }
            AttributesTest::ContainerSubsequent { retval } => {
                let key = str_of(item, "container_id");
                let n = state
                    .tmp
                    .container_item_pos
                    .get(&key)
                    .and_then(Value::as_f64);
                Ok(match n {
                    Some(p) if p > 1.0 => *retval,
                    _ => !*retval,
                })
            }
            AttributesTest::HasSubunit { namevar } => {
                let mut subunit_count: usize = 0;
                let names: Vec<&Value> = match item.get(namevar.as_str()) {
                    Some(Value::Array(a)) => a.iter().collect(),
                    Some(Value::Object(o)) => o.values().collect(),
                    Some(Value::String(s)) if !s.is_empty() => {
                        return Err(EngineError::Csl(
                            "TypeError: cannot read properties of undefined (reading 'split')"
                                .into(),
                        ))
                    }
                    _ => Vec::new(),
                };
                for name in names {
                    if !js::truthy_opt(name.get("given")) {
                        let inst = if js::truthy_opt(name.get("literal")) {
                            name.get("literal")
                        } else {
                            name.get("family")
                        };
                        let Some(inst) = inst else {
                            return Err(EngineError::Csl(
                                "TypeError: cannot read properties of undefined (reading 'split')"
                                    .into(),
                            ));
                        };
                        let length = js::to_js_string(inst).split('|').count();
                        if subunit_count == 0 || length < subunit_count {
                            subunit_count = length;
                        }
                    }
                }
                Ok(subunit_count > 1)
            }
            AttributesTest::CiteForm { cite_form } => Ok(item
                .get("cite-form")
                .map(|v| v.as_str() == Some(cite_form.as_str()))
                .unwrap_or(false)),
        }
    }
}

/// `CSL.POSITION_MAP[pos] >= CSL.POSITION_MAP[target]` with undefined = false.
fn position_map_ge(pos: Option<i64>, target: i64) -> bool {
    match (pos.and_then(position_map), position_map(target)) {
        (Some(a), Some(b)) => a >= b,
        _ => false,
    }
}

/// `state.tmp.condition_lang_counter_arr.push(state.tmp.condition_counter);
/// state.tmp.condition_lang_val_arr.push(state.opt.lang);`
fn push_lang_state(state: &mut State) {
    state
        .tmp
        .condition_lang_counter_arr
        .push(state.tmp.condition_counter);
    let lang = state
        .opt
        .get("lang")
        .map(js::to_js_string)
        .unwrap_or_default();
    state.tmp.condition_lang_val_arr.push(lang);
}

/// `state.opt.lang = lang`.
fn set_lang(state: &mut State, lang: &str) {
    state
        .opt
        .insert("lang".into(), Value::String(lang.to_string()));
}

// ---------------------------------------------------------------------------
// The attribute handlers.
// ---------------------------------------------------------------------------

fn atest(t: AttributesTest) -> Test {
    Test::Attributes(t)
}

fn strings_vec(v: &[String]) -> Value {
    Value::Array(v.iter().map(|s| Value::String(s.clone())).collect())
}

fn flag_map(vars: &[String]) -> Value {
    let mut o = Obj::new();
    for v in vars {
        o.insert(v.clone(), Value::Bool(true));
    }
    Value::Object(o)
}

fn set_opt_flag(state: &mut State, key: &str, value: Value) {
    state.opt.insert(key.to_string(), value);
}

/// `state.opt.parallel.enable = true`.
fn enable_parallel(state: &mut State) {
    obj_entry(&mut state.opt, "parallel").insert("enable".into(), Value::Bool(true));
}

/// `if (!state.opt.track_repeat) {...}; state.opt.track_repeat[v] = true`.
fn track_repeat(state: &mut State, vars: &[String]) {
    let tr = obj_entry(&mut state.opt, "track_repeat");
    for v in vars {
        tr.insert(v.clone(), Value::Bool(true));
    }
}

/// The attribute handlers that start with `if (!this.tests) {this.tests = []; }`.
const TESTS_INITIALISING: [&str; 25] = [
    "@disambiguate",
    "@is-numeric",
    "@is-uncertain-date",
    "@locator",
    "@position",
    "@type",
    "@variable",
    "@page",
    "@number",
    "@jurisdiction",
    "@country",
    "@context",
    "@has-year-only",
    "@has-to-month-or-season",
    "@has-day",
    "@is-plural",
    "@is-multiple",
    "@locale",
    "@alternative-node-internal",
    "@locale-internal",
    "@court-class",
    "@container-multiple",
    "@container-subsequent",
    "@has-subunit",
    "@cite-form",
];

/// `CSL.Attributes[key].call(token, state, "" + arg)`. Returns `Ok(false)`
/// for an attribute upstream does not define (it only warns). Entry point
/// called by the build loop (`CSL.XmlToToken`) and `setStyleAttributes`.
/// `key` includes the leading `@` (`"@variable"`).
pub fn apply(state: &mut State, token: &mut Token, key: &str, arg: &str) -> CslResult<bool> {
    if TESTS_INITIALISING.contains(&key) {
        // `if (!this.tests) {this.tests = []; }` at the top of the handler.
        token.tests_defined = true;
    }
    match key {
        "@disambiguate" => {
            if arg == "true" {
                set_opt_flag(state, "has_disambiguate", Value::Bool(true));
                token.tests.push(atest(AttributesTest::Disambiguate));
            } else if arg == "check-ambiguity-and-backreference" {
                token.tests.push(atest(AttributesTest::DisambiguateBackref));
            }
        }
        "@is-numeric" => {
            for variable in split_ws(arg) {
                token
                    .tests
                    .push(atest(AttributesTest::IsNumeric { variable }));
            }
        }
        "@is-uncertain-date" => {
            for variable in split_ws(arg) {
                token
                    .tests
                    .push(atest(AttributesTest::IsUncertainDate { variable }));
            }
        }
        "@locator" => {
            let trylabels = arg.replacen("sub verbo", "sub-verbo", 1);
            for trylabel in split_ws(&trylabels) {
                token
                    .tests
                    .push(atest(AttributesTest::Locator { trylabel }));
            }
        }
        "@position" => {
            set_opt_flag(state, "update_mode", Value::from(POSITION));
            for tryposition in split_ws(arg) {
                // Keywords upstream maps to CSL.POSITION_*; anything else stays a
                // string upstream, which no comparison below can match
                // (numeric-looking strings are not supported).
                let mapped = match tryposition.as_str() {
                    "first" => Some(0),
                    "container-subsequent" => Some(4),
                    "subsequent" => Some(1),
                    "ibid" => Some(2),
                    "ibid-with-locator" => Some(3),
                    _ => None,
                };
                if tryposition == "near-note" {
                    token.tests.push(atest(AttributesTest::PositionNear));
                } else if tryposition == "far-note" {
                    token.tests.push(atest(AttributesTest::PositionFar));
                } else {
                    token.tests.push(atest(AttributesTest::Position {
                        tryposition: mapped,
                    }));
                }
            }
        }
        "@type" => {
            // XXX This is ALWAYS composed as an "any" match
            let tests: Vec<Test> = split_ws(arg)
                .into_iter()
                .map(|mytype| atest(AttributesTest::Type { mytype }))
                .collect();
            let combined = state.fun.match_.any(token, state, &tests);
            token.tests.push(combined);
        }
        "@variable" => {
            token.variables = split_ws(arg);
            token
                .extra
                .insert("variables_real".into(), strings_vec(&token.variables));
            // First the non-conditional code.
            if token.name == "label" && !token.variables[0].is_empty() {
                let term = token.variables[0].clone();
                token.set_string("term", &term);
            } else if ["names", "date", "text", "number"].contains(&token.name.as_str()) {
                token
                    .execs
                    .push(Exec::Attributes(AttributesExec::VariableSetNames));
                token
                    .execs
                    .push(Exec::Attributes(AttributesExec::VariableCheckOutput));
            } else if ["if", "else-if", "condition"].contains(&token.name.as_str()) {
                for variable in token.variables.clone() {
                    token
                        .tests
                        .push(atest(AttributesTest::Variable { variable }));
                }
            }
        }
        "@page" => {
            let trylabels = arg.replacen("sub verbo", "sub-verbo", 1);
            for trylabel in split_ws(&trylabels) {
                token.tests.push(atest(AttributesTest::Page { trylabel }));
            }
        }
        "@number" => {
            for trylabel in split_ws(arg) {
                token.tests.push(atest(AttributesTest::Number { trylabel }));
            }
        }
        "@jurisdiction" => {
            token.tests.push(atest(AttributesTest::Jurisdiction {
                tryjurisdictions: split_ws(arg),
            }));
        }
        "@country" => {
            token.tests.push(atest(AttributesTest::Country {
                trycountries: split_ws(arg),
            }));
        }
        "@context" => {
            token.tests.push(atest(AttributesTest::Context {
                arg: arg.to_string(),
            }));
        }
        "@has-year-only" => {
            for trydate in split_ws(arg) {
                token
                    .tests
                    .push(atest(AttributesTest::HasYearOnly { trydate }));
            }
        }
        "@has-to-month-or-season" => {
            for trydate in split_ws(arg) {
                token
                    .tests
                    .push(atest(AttributesTest::HasToMonthOrSeason { trydate }));
            }
        }
        "@has-day" => {
            for trydate in split_ws(arg) {
                token.tests.push(atest(AttributesTest::HasDay { trydate }));
            }
        }
        "@is-plural" => {
            token.tests.push(atest(AttributesTest::IsPlural {
                arg: arg.to_string(),
            }));
        }
        "@is-multiple" => {
            token.tests.push(atest(AttributesTest::IsMultiple {
                arg: arg.to_string(),
            }));
        }
        "@locale" => apply_locale(state, token, arg)?,
        "@alternative-node-internal" => {
            token
                .tests
                .push(atest(AttributesTest::AlternativeNodeInternal));
        }
        "@locale-internal" => apply_locale_internal(state, token, arg)?,
        "@court-class" => {
            for tryclass in split_ws(arg) {
                token
                    .tests
                    .push(atest(AttributesTest::CourtClass { tryclass }));
            }
        }
        "@container-multiple" => {
            token.tests.push(atest(AttributesTest::ContainerMultiple {
                retval: arg == "true",
            }));
        }
        "@container-subsequent" => {
            token.tests.push(atest(AttributesTest::ContainerSubsequent {
                retval: arg == "true",
            }));
        }
        "@has-subunit" => {
            token.tests.push(atest(AttributesTest::HasSubunit {
                namevar: arg.to_string(),
            }));
        }
        "@cite-form" => {
            token.tests.push(atest(AttributesTest::CiteForm {
                cite_form: arg.to_string(),
            }));
        }
        "@disable-duplicate-year-suppression" => {
            set_opt_flag(
                state,
                "disable_duplicate_year_suppression",
                strings_vec(&split_ws(arg)),
            );
        }
        "@consolidate-containers" => {
            track_containers(state, arg);
            state
                .bibliography
                .opt
                .insert("consolidate_containers".into(), strings_vec(&split_ws(arg)));
        }
        "@track-containers" => track_containers(state, arg),
        // These are not evaluated as conditions immediately: they only
        // set parameters that are picked up during processing.
        "@parallel-first" => {
            enable_parallel(state);
            let vars = split_ws(arg);
            track_repeat(state, &vars);
            token.extra.insert("parallel_first".into(), flag_map(&vars));
        }
        "@parallel-last" => {
            enable_parallel(state);
            let vars = split_ws(arg);
            track_repeat(state, &vars);
            token.extra.insert("parallel_last".into(), flag_map(&vars));
        }
        "@parallel-last-to-first" => {
            enable_parallel(state);
            let vars = split_ws(arg);
            token
                .extra
                .insert("parallel_last_to_first".into(), flag_map(&vars));
        }
        "@parallel-delimiter-override" => {
            enable_parallel(state);
            token.set_string("set_parallel_delimiter_override", arg);
        }
        "@parallel-delimiter-override-on-suppress" => {
            enable_parallel(state);
            token.set_string("set_parallel_delimiter_override_on_suppress", arg);
        }
        "@no-repeat" => {
            enable_parallel(state);
            let vars = split_ws(arg);
            track_repeat(state, &vars);
            token.extra.insert("non_parallel".into(), flag_map(&vars));
        }
        "@require" => {
            set_opt_flag(state, "use_context_condition", Value::Bool(true));
            token.set_string("require", arg);
        }
        "@reject" => {
            set_opt_flag(state, "use_context_condition", Value::Bool(true));
            token.set_string("reject", arg);
        }
        "@require-comma-on-symbol" => {
            set_opt_flag(
                state,
                "require_comma_on_symbol",
                Value::String(arg.to_string()),
            );
        }
        "@gender" => {
            token
                .extra
                .insert("gender".into(), Value::String(arg.to_string()));
        }
        "@cslid" => {
            // @cslid is a noop at run time; the id is kept for reverse lookup.
            token
                .extra
                .insert("cslid".into(), int_or_nan(js::parse_int(arg)));
        }
        "@capitalize-if-first" => token.set_string("capitalize_if_first_override", arg),
        "@label-capitalize-if-first" => token.set_string("label_capitalize_if_first_override", arg),
        "@label-form" => token.set_string("label_form_override", arg),
        "@part-separator" => token.set_string("part-separator", arg),
        "@leading-noise-words" => {
            token
                .extra
                .insert("leading-noise-words".into(), Value::String(arg.to_string()));
        }
        "@name-never-short" => {
            token
                .extra
                .insert("name-never-short".into(), Value::String(arg.to_string()));
        }
        "@class" => set_opt_flag(state, "class", Value::String(arg.to_string())),
        "@version" => set_opt_flag(state, "version", Value::String(arg.to_string())),
        "@value" => token.set_string("value", arg),
        "@name" => token.set_string("name", arg),
        "@form" => token.set_string("form", arg),
        "@date-parts" => token.set_string("date-parts", arg),
        "@range-delimiter" => token.set_string("range-delimiter", arg),
        "@macro" => token.postponed_macro = Some(arg.to_string()),
        "@term" => {
            if arg == "sub verbo" {
                token.set_string("term", "sub-verbo");
            } else {
                token.set_string("term", arg);
            }
        }
        "@xmlns" => {}
        "@lang" => {
            if !arg.is_empty() {
                state.build.lang = Some(arg.to_string());
            }
        }
        // Used as a flag during dates processing
        "@lingo" => {}
        "@macro-has-date" => {
            token
                .extra
                .insert("macro-has-date".into(), Value::Bool(true));
        }
        "@suffix" => token.set_string("suffix", arg),
        "@prefix" => token.set_string("prefix", arg),
        "@delimiter" => token.set_string("delimiter", arg),
        "@match" => {
            token
                .extra
                .insert("match".into(), Value::String(arg.to_string()));
        }
        "@names-min" => {
            let val = js::parse_int(arg);
            bump_max_names(state, val)?;
            token.strings.insert("et-al-min".into(), int_or_nan(val));
        }
        "@names-use-first" => {
            token
                .strings
                .insert("et-al-use-first".into(), int_or_nan(js::parse_int(arg)));
        }
        "@names-use-last" => {
            token
                .strings
                .insert("et-al-use-last".into(), Value::Bool(arg == "true"));
        }
        "@sort" => {
            if arg == "descending" {
                token
                    .strings
                    .insert("sort_direction".into(), Value::from(DESCENDING));
            }
        }
        "@plural" => {
            // Accepted values of plural attribute differ on cs:text
            // and cs:label nodes.
            if arg == "always" || arg == "true" {
                token.strings.insert("plural".into(), Value::from(1));
            } else if arg == "never" || arg == "false" {
                token.strings.insert("plural".into(), Value::from(0));
            } else if arg == "contextual" {
                token.strings.insert("plural".into(), Value::Bool(false));
            }
        }
        "@has-publisher-and-publisher-place" => {
            token.strings.insert(
                "has-publisher-and-publisher-place".into(),
                Value::Bool(true),
            );
        }
        "@publisher-delimiter-precedes-last" => {
            token.set_string("publisher-delimiter-precedes-last", arg)
        }
        "@publisher-delimiter" => token.set_string("publisher-delimiter", arg),
        "@publisher-and" => token.set_string("publisher-and", arg),
        "@givenname-disambiguation-rule" => {
            if GIVENNAME_DISAMBIGUATION_RULES.contains(&arg) {
                state.citation.opt.insert(
                    "givenname-disambiguation-rule".into(),
                    Value::String(arg.to_string()),
                );
            }
        }
        "@collapse" => {
            // only one collapse value will be honoured.
            if !arg.is_empty() {
                let name = token.name.clone();
                area_mut(state, &name)?
                    .opt
                    .insert("collapse".into(), Value::String(arg.to_string()));
            }
        }
        "@cite-group-delimiter" => {
            if !arg.is_empty() {
                let area = state.tmp.area.clone();
                area_mut(state, &area)?.opt.insert(
                    "cite_group_delimiter".into(),
                    Value::String(arg.to_string()),
                );
            }
        }
        "@names-delimiter" => state.set_opt(token, "names-delimiter", Value::String(arg.into())),
        "@name-form" => state.set_opt(token, "name-form", Value::String(arg.into())),
        "@subgroup-delimiter" => token.set_string("subgroup-delimiter", arg),
        "@subgroup-delimiter-precedes-last" => {
            token.set_string("subgroup-delimiter-precedes-last", arg)
        }
        "@name-delimiter" => state.set_opt(token, "name-delimiter", Value::String(arg.into())),
        "@et-al-min" => {
            let val = js::parse_int(arg);
            bump_max_names(state, val)?;
            state.set_opt(token, "et-al-min", int_or_nan(val));
        }
        "@et-al-use-first" => {
            state.set_opt(token, "et-al-use-first", int_or_nan(js::parse_int(arg)));
        }
        "@et-al-use-last" => {
            state.set_opt(token, "et-al-use-last", Value::Bool(arg == "true"));
        }
        "@et-al-subsequent-min" => {
            let val = js::parse_int(arg);
            bump_max_names(state, val)?;
            state.set_opt(token, "et-al-subsequent-min", int_or_nan(val));
        }
        "@et-al-subsequent-use-first" => {
            state.set_opt(
                token,
                "et-al-subsequent-use-first",
                int_or_nan(js::parse_int(arg)),
            );
        }
        "@suppress-min" => {
            token
                .strings
                .insert("suppress-min".into(), int_or_nan(js::parse_int(arg)));
        }
        "@suppress-max" => {
            token
                .strings
                .insert("suppress-max".into(), int_or_nan(js::parse_int(arg)));
        }
        "@and" => state.set_opt(token, "and", Value::String(arg.into())),
        "@delimiter-precedes-last" => {
            state.set_opt(token, "delimiter-precedes-last", Value::String(arg.into()))
        }
        "@delimiter-precedes-et-al" => {
            state.set_opt(token, "delimiter-precedes-et-al", Value::String(arg.into()))
        }
        "@initialize-with" => state.set_opt(token, "initialize-with", Value::String(arg.into())),
        "@initialize" => {
            if arg == "false" {
                state.set_opt(token, "initialize", Value::Bool(false));
            }
        }
        "@name-as-reverse-order" => {
            token
                .extra
                .insert("name-as-reverse-order".into(), Value::String(arg.into()));
        }
        "@name-as-sort-order" => {
            if token.name == "style-options" {
                token
                    .extra
                    .insert("name-as-sort-order".into(), Value::String(arg.into()));
            } else {
                state.set_opt(token, "name-as-sort-order", Value::String(arg.into()));
            }
        }
        "@sort-separator" => state.set_opt(token, "sort-separator", Value::String(arg.into())),
        "@require-match" => {
            if arg == "true" {
                token.extra.insert("requireMatch".into(), Value::Bool(true));
            }
        }
        "@exclude-types" => {
            state
                .bibliography
                .opt
                .insert("exclude_types".into(), strings_vec(&split_ws(arg)));
        }
        "@exclude-with-fields" => {
            state
                .bibliography
                .opt
                .insert("exclude_with_fields".into(), strings_vec(&split_ws(arg)));
        }
        "@year-suffix-delimiter" => {
            let name = token.name.clone();
            area_mut(state, &name)?
                .opt
                .insert("year-suffix-delimiter".into(), Value::String(arg.into()));
        }
        "@after-collapse-delimiter" => {
            let name = token.name.clone();
            area_mut(state, &name)?
                .opt
                .insert("after-collapse-delimiter".into(), Value::String(arg.into()));
        }
        "@subsequent-author-substitute" => {
            let name = token.name.clone();
            area_mut(state, &name)?.opt.insert(
                "subsequent-author-substitute".into(),
                Value::String(arg.into()),
            );
        }
        "@subsequent-author-substitute-rule" => {
            let name = token.name.clone();
            area_mut(state, &name)?.opt.insert(
                "subsequent-author-substitute-rule".into(),
                Value::String(arg.into()),
            );
        }
        "@disambiguate-add-names" => {
            if arg == "true" {
                set_opt_flag(state, "disambiguate-add-names", Value::Bool(true));
            }
        }
        "@disambiguate-add-givenname" => {
            if arg == "true" {
                set_opt_flag(state, "disambiguate-add-givenname", Value::Bool(true));
            }
        }
        "@disambiguate-add-year-suffix" => {
            if arg == "true" && state.opt.get("xclass").and_then(Value::as_str) != Some("numeric") {
                set_opt_flag(state, "disambiguate-add-year-suffix", Value::Bool(true));
            }
        }
        "@second-field-align" => {
            if arg == "flush" || arg == "margin" {
                let name = token.name.clone();
                area_mut(state, &name)?
                    .opt
                    .insert("second-field-align".into(), Value::String(arg.into()));
            }
        }
        "@hanging-indent" => {
            if arg == "true" {
                let legacy = state
                    .opt
                    .get("development_extensions")
                    .map(|d| js::truthy_opt(d.get("hanging_indent_legacy_number")))
                    .unwrap_or(false);
                let name = token.name.clone();
                area_mut(state, &name)?.opt.insert(
                    "hangingindent".into(),
                    if legacy {
                        Value::from(2)
                    } else {
                        Value::Bool(true)
                    },
                );
            }
        }
        "@line-spacing" => {
            if !arg.is_empty() && arg.chars().all(|c| c == '.' || c.is_ascii_digit()) {
                let name = token.name.clone();
                area_mut(state, &name)?
                    .opt
                    .insert("line-spacing".into(), parse_float_value(arg));
            }
        }
        "@entry-spacing" => {
            if !arg.is_empty() && arg.chars().all(|c| c == '.' || c.is_ascii_digit()) {
                let name = token.name.clone();
                area_mut(state, &name)?
                    .opt
                    .insert("entry-spacing".into(), parse_float_value(arg));
            }
        }
        "@near-note-distance" => {
            let name = token.name.clone();
            area_mut(state, &name)?
                .opt
                .insert("near-note-distance".into(), int_or_nan(js::parse_int(arg)));
        }
        "@substring" => {
            token
                .extra
                .insert("substring".into(), int_or_nan(js::parse_int(arg)));
        }
        "@text-case" => {
            token.execs.push(Exec::Attributes(AttributesExec::TextCase {
                arg: arg.to_string(),
            }));
        }
        "@page-range-format" => set_opt_flag(state, "page-range-format", Value::String(arg.into())),
        "@year-range-format" => set_opt_flag(state, "year-range-format", Value::String(arg.into())),
        "@default-locale" => apply_default_locale(state, token, arg)?,
        "@default-locale-sort" => {
            set_opt_flag(state, "default-locale-sort", Value::String(arg.into()))
        }
        "@demote-non-dropping-particle" => {
            set_opt_flag(
                state,
                "demote-non-dropping-particle",
                Value::String(arg.into()),
            );
        }
        "@initialize-with-hyphen" => {
            if arg == "false" {
                set_opt_flag(state, "initialize-with-hyphen", Value::Bool(false));
            }
        }
        "@institution-parts" => token.set_string("institution-parts", arg),
        "@if-short" => {
            if arg == "true" {
                token.strings.insert("if-short".into(), Value::Bool(true));
            }
        }
        "@substitute-use-first" => {
            token.strings.insert(
                "substitute-use-first".into(),
                int_or_nan(js::parse_int(arg)),
            );
        }
        "@use-first" => {
            token
                .strings
                .insert("use-first".into(), int_or_nan(js::parse_int(arg)));
        }
        "@use-last" => {
            token
                .strings
                .insert("use-last".into(), int_or_nan(js::parse_int(arg)));
        }
        "@stop-first" => {
            token
                .strings
                .insert("stop-first".into(), int_or_nan(js::parse_int(arg)));
        }
        "@stop-last" => {
            token.strings.insert(
                "stop-last".into(),
                int_or_nan(js::parse_int(arg).map(|n| -n)),
            );
        }
        "@reverse-order" => {
            if arg == "true" {
                token
                    .strings
                    .insert("reverse-order".into(), Value::Bool(true));
            }
        }
        "@display" => {
            // `state.bibliography.tokens.length`, with the list out of the
            // state while it is built (see `Build::bibliography_tokens_len`).
            if state.build.bibliography_tokens_len == 2 {
                set_opt_flag(state, "using_display", Value::Bool(true));
            }
            token.set_string("cls", arg);
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/// `state[state.build.area].opt.max_number_of_names < val` then assign.
fn bump_max_names(state: &mut State, val: Option<i64>) -> CslResult<()> {
    let Some(val) = val else {
        return Ok(());
    };
    let area = state.build.area.clone();
    let opt = &mut area_mut(state, &area)?.opt;
    let cur = opt.get("max_number_of_names").and_then(Value::as_i64);
    if cur.map(|c| c < val).unwrap_or(false) {
        opt.insert("max_number_of_names".into(), Value::from(val));
    }
    Ok(())
}

/// `@track-containers` (also called by `@consolidate-containers`).
fn track_containers(state: &mut State, arg: &str) {
    let args = split_ws(arg);
    let opt = &mut state.bibliography.opt;
    let tci = arr_entry(opt, "track_container_items");
    for a in &args {
        tci.push(Value::String(a.clone()));
    }
    arr_entry(opt, "consolidate_containers");
}

/// `@locale` (attributes.js:~690).
fn apply_locale(state: &mut State, token: &mut Token, arg: &str) -> CslResult<()> {
    // Style default
    let locale_default = default_locale(state);
    if token.name == "layout" {
        // For layout
        token
            .extra
            .insert("locale_raw".into(), Value::String(arg.to_string()));
        if token.tokentype == TokenType::Start {
            arr_entry(&mut state.opt, "multi_layout");
            let mut locale_data: Vec<Value> = Vec::new();
            // Register the primary locale in the set, and others that "map" to it,
            // so that they can be used when generating sort keys. See node_sort.js.
            let locales = split_ws(arg);
            let mut sort_locale = Obj::new();
            let master = locale_resolve(&locales[0], Some(&locale_default));
            locale_data.push(master.to_value());
            // Upstream keys by `localeMaster.generic`, a boolean: the key is
            // the string "true". Kept as is.
            let key = if master.generic {
                "true".to_string()
            } else {
                master.best.clone()
            };
            sort_locale.insert(key, Value::String(master.best.clone()));
            for l in &locales[1..] {
                let servant = locale_resolve(l, Some(&locale_default));
                locale_data.push(servant.to_value());
                let key = if servant.generic {
                    "true".to_string()
                } else {
                    servant.best.clone()
                };
                sort_locale.insert(key, Value::String(master.best.clone()));
            }
            let area = state.build.area.clone();
            arr_entry(&mut area_mut(state, &area)?.opt, "sort_locales")
                .push(Value::Object(sort_locale));
            arr_entry(&mut state.opt, "multi_layout").push(Value::Array(locale_data));
        }
        set_opt_flag(state, "has_layout_locale", Value::Bool(true));
    } else {
        // For if and if-else
        let lst = split_ws(arg);
        let mut locale_bares: Vec<String> = Vec::new();
        let mut locale_list: Vec<LangSpec> = Vec::new();
        for lang in &lst {
            let langspec = locale_resolve(lang, Some(&locale_default));
            if js::len(lang) == 2 {
                // For fallback
                locale_bares.push(langspec.bare.clone());
            }
            // Load the locale terms etc.
            // (second argument causes immediate return if locale already exists)
            state.locale_configure(&langspec, true)?;
            locale_list.push(langspec);
        }
        token.tests.push(atest(AttributesTest::Locale {
            locale_list,
            locale_default,
            locale_bares,
        }));
    }
    Ok(())
}

/// `@locale-internal` (attributes.js:~760).
fn apply_locale_internal(state: &mut State, token: &mut Token, arg: &str) -> CslResult<()> {
    let lst = split_ws(arg);
    let dl = default_locale(state);
    let mut locale_bares: Vec<String> = Vec::new();
    let mut locale_list: Vec<LangSpec> = Vec::new();
    for lang in &lst {
        let langspec = locale_resolve(lang, Some(&dl));
        if js::len(lang) == 2 {
            // For fallback
            locale_bares.push(langspec.bare.clone());
        }
        // Load the locale terms etc.
        state.locale_configure(&langspec, false)?;
        locale_list.push(langspec);
    }
    // Set locale tag on node
    token
        .extra
        .insert("locale_bares".into(), strings_vec(&locale_bares));
    token
        .extra
        .insert("locale_default".into(), Value::String(dl));
    // The locale to set on node children if match is successful
    let locale = locale_list[0].best.clone();
    token
        .extra
        .insert("locale".into(), Value::String(locale.clone()));
    // Locales to test
    token.extra.insert(
        "locale_list".into(),
        Value::Array(locale_list.iter().map(LangSpec::to_value).collect()),
    );
    token.tests.push(atest(AttributesTest::LocaleInternal {
        locale_list,
        locale_bares,
        locale,
    }));
    Ok(())
}

/// `@default-locale` (attributes.js:~1545).
fn apply_default_locale(state: &mut State, token: &mut Token, arg: &str) -> CslResult<()> {
    if token.name == "style" {
        static SPLIT_RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new("-x-(?:sort|translit|translat)-").expect("static regex"));
        let ms: Vec<String> = SPLIT_RE
            .find_iter(arg)
            .map(|m| {
                let s = m.as_str();
                // .replace(/^-x-/, "").replace(/-$/, "")
                s.trim_start_matches("-x-")
                    .trim_end_matches('-')
                    .to_string()
            })
            .collect();
        let lst0 = js::split(&SPLIT_RE, arg);
        let mut ret: Vec<String> = vec![lst0[0].clone()];
        for pos in 1..lst0.len() {
            ret.push(ms.get(pos - 1).cloned().unwrap_or_default());
            ret.push(lst0[pos].clone());
        }
        let len = ret.len();
        let mut pos = 1;
        while pos < len {
            let key = format!("locale-{}", ret[pos]);
            let val = ret.get(pos + 1).cloned().unwrap_or_default();
            // .replace(/^\s*/g, "").replace(/\s*$/g, "")
            let val = js::trim(&val).to_string();
            arr_entry(&mut state.opt, &key).push(Value::String(val));
            pos += 2;
        }
        // `if (lst.length)` is always true: lst has at least one element.
        state.opt.insert(
            "default-locale".into(),
            Value::Array(vec![Value::String(ret[0].clone())]),
        );
    } else if arg == "true" {
        token
            .extra
            .insert("default_locale".into(), Value::Bool(true));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Differential test of the attribute handlers and node builders against
    //! citeproc-js 2.4.63 (`scripts/csl-units/build_tokens.cjs`, data in
    //! `tests/data/csl/units/build_tokens.json`).
    //!
    //! **Method.** Each case is a tree of CSL elements. `scripts/csl-units/
    //! build_tokens.cjs` serialises it to a style, lets citeproc-js build it
    //! (with the XML name/institution/publisher normalisations switched off, so
    //! the token lists are what the node builders make of exactly that tree),
    //! and dumps the five token lists plus the options the handlers write.
    //! This test replays the tree the way `CSL.makeBuilder` / `CSL.XmlToToken`
    //! do (attributes applied in document order by [`apply`], then the node
    //! builder), runs the jump-index pass (`configureTokenList`), and compares
    //! per token: name, tokentype, `execs_n`, `tests_n` (present only when the
    //! `tests` array exists), `has_test`, `strings`, `variables`, every other
    //! data property, `next`/`succeed`/`fail`; plus the compared option keys,
    //! `cite_affixes` and `date_key`. Decorations are not compared (the driver
    //! does not run `CSL.setDecorations`).
    //!
    //! **Pass criterion.** Every compared case matches exactly. Cases that need
    //! machinery outside this wave (macro expansion, locale loading, date
    //! template merging, `getTerm`) end in `NotYetPorted` and are skipped; the
    //! exact skip list is asserted so a newly failing case cannot hide there.
    //!
    //! **Results (2026-10-08).** 84 cases generated; 1 rejected by citeproc-js
    //! itself (`intext_basic`, upstream throws); 7 skipped (see `EXPECTED_SKIPS`);
    //! the rest match.
    use super::*;
    use crate::citeproc::stack::Stack;
    use crate::citeproc::{
        node_alternative, node_alternativetext, node_bibliography, node_choose, node_citation,
        node_comment, node_condition, node_conditions, node_date, node_datepart, node_else,
        node_elseif, node_etal, node_group, node_if, node_info, node_institution,
        node_institutionpart, node_intext, node_key, node_label, node_layout, node_macro,
        node_name, node_namepart, node_names, node_number, node_sort, node_substitute, node_text,
    };

    const DATA: &str = include_str!("../../tests/data/csl/units/build_tokens.json");

    /// Cases that end in `NotYetPorted` today, and why.
    const EXPECTED_SKIPS: [&str; 3] = [
        "macro_in_text",  // needs the style's <macro> node (cslXml); see build_case
        "sort_key_macro", // same
        "date_with_form", // CSL.Util.fixDateNode host: needs cslXml
    ];

    /// Every `locales-<lang>.xml` of `vendor/citeproc-js/locale` (the runner's
    /// `retrieveLocale`, processing instructions dropped); empty when `vendor/` is absent.
    fn test_locales() -> std::sync::Arc<std::collections::BTreeMap<String, String>> {
        static LOCALES: std::sync::LazyLock<
            std::sync::Arc<std::collections::BTreeMap<String, String>>,
        > = std::sync::LazyLock::new(|| {
            let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../vendor/citeproc-js/locale");
            let pi = Regex::new(r"\s*<\?[^>]*\?>\s*\n").expect("static");
            let mut map = std::collections::BTreeMap::new();
            if let Ok(read) = std::fs::read_dir(&dir) {
                for entry in read.flatten() {
                    let file = entry.file_name().to_string_lossy().to_string();
                    if let Some(lang) = file
                        .strip_prefix("locales-")
                        .and_then(|f| f.strip_suffix(".xml"))
                    {
                        if let Ok(xml) = std::fs::read_to_string(entry.path()) {
                            map.insert(lang.to_string(), pi.replace_all(&xml, "").to_string());
                        }
                    }
                }
            }
            std::sync::Arc::new(map)
        });
        LOCALES.clone()
    }

    fn obj(v: Value) -> Obj {
        match v {
            Value::Object(o) => o,
            _ => Obj::new(),
        }
    }

    /// `CSL.Engine` constructor defaults (state.js) for the fields the
    /// handlers and builders touch.
    fn fresh_state(class: &str) -> CslResult<State> {
        let mut s = State::default();
        s.opt = obj(serde_json::json!({
            "parallel": {"enable": false},
            "has_disambiguate": false,
            "inheritedAttributes": {},
            "locale-sort": [], "locale-translit": [], "locale-translat": [],
            "update_mode": 0, "bib_mode": 0, "sort_citations": false,
            "has_layout_locale": false, "disable_duplicate_year_suppression": [],
            "use_context_condition": false,
            "initialize-with-hyphen": true,
            "demote-non-dropping-particle": "display-and-sort",
            "default-locale": ["en-US"], "lang": "en-US", "xclass": class,
            "development_extensions": {},
        }));
        let common = |a: &mut Area| {
            a.opt = obj(serde_json::json!({
                "inheritedAttributes": {}, "collapse": [], "topdecor": [],
                "layout_decorations": [], "layout_prefix": "", "layout_suffix": "",
                "layout_delimiter": "", "sort_locales": [], "max_number_of_names": 0,
            }));
        };
        common(&mut s.citation);
        s.citation
            .opt
            .insert("givenname-disambiguation-rule".into(), "by-cite".into());
        s.citation.opt.insert("near-note-distance".into(), 5.into());
        common(&mut s.intext);
        s.intext
            .opt
            .insert("givenname-disambiguation-rule".into(), "by-cite".into());
        s.intext.opt.insert("near-note-distance".into(), 5.into());
        common(&mut s.bibliography);
        s.bibliography.opt.insert("line-spacing".into(), 1.into());
        s.bibliography.opt.insert("entry-spacing".into(), 1.into());
        for a in [&mut s.citation_sort, &mut s.bibliography_sort] {
            a.opt = obj(serde_json::json!({"sort_directions": [], "topdecor": []}));
        }
        s.citation.root = "citation".into();
        s.intext.root = "intext".into();
        s.bibliography.root = "bibliography".into();
        s.citation_sort.root = "citation".into();
        s.bibliography_sort.root = "bibliography".into();
        s.tmp.area = "citation".into();
        s.tmp.root = "citation".into();
        s.tmp.cite_affixes = obj(serde_json::json!({
            "citation": false, "bibliography": false,
            "citation_sort": false, "bibliography_sort": false,
        }));
        s.build.area = "citation".into();
        s.build.root = "citation".into();
        s.build.substitute_level = Stack::with(0);
        // setStyleAttributes: class, version, default-locale of <style>.
        let mut style = Token::new("style", TokenType::Start);
        apply(&mut s, &mut style, "@class", class)?;
        apply(&mut s, &mut style, "@version", "1.0")?;
        apply(&mut s, &mut style, "@default-locale", "en-US")?;
        // build.js:121: a style without @sort-separator gets ", ".
        apply(&mut s, &mut style, "@sort-separator", ", ")?;
        s.opt
            .insert("default-locale-sort".into(), Value::String("en-US".into()));
        s.sys.locales = test_locales();
        s.locale_configure(&locale_resolve("en-US", None), false)?;
        Ok(s)
    }

    fn dispatch_build(state: &mut State, token: Token, target: &mut Vec<Token>) -> CslResult<()> {
        macro_rules! go {
            ($m:ident) => {
                $m::build(state, token, target, Some(true))
            };
        }
        match token.name.clone().as_str() {
            "alternative" => go!(node_alternative),
            "alternative-text" => go!(node_alternativetext),
            "bibliography" => go!(node_bibliography),
            "choose" => go!(node_choose),
            "citation" => go!(node_citation),
            "#comment" => go!(node_comment),
            "condition" => go!(node_condition),
            "conditions" => go!(node_conditions),
            "date" => go!(node_date),
            "date-part" => go!(node_datepart),
            "else" => go!(node_else),
            "else-if" => go!(node_elseif),
            "et-al" => go!(node_etal),
            "group" => go!(node_group),
            "if" => go!(node_if),
            "info" => go!(node_info),
            "institution" => go!(node_institution),
            "institution-part" => go!(node_institutionpart),
            "intext" => go!(node_intext),
            "key" => go!(node_key),
            "label" => go!(node_label),
            "layout" => go!(node_layout),
            "macro" => go!(node_macro),
            "name" => go!(node_name),
            "name-part" => go!(node_namepart),
            "names" => go!(node_names),
            "number" => go!(node_number),
            "sort" => go!(node_sort),
            "substitute" => go!(node_substitute),
            "text" => go!(node_text),
            other => Err(EngineError::Csl(format!(
                "Undefined node name \"{other}\"."
            ))),
        }
    }

    fn dispatch_configure(state: &mut State, tokens: &mut [Token], pos: usize) -> CslResult<()> {
        match tokens[pos].name.clone().as_str() {
            "choose" => node_choose::configure(state, tokens, pos),
            "else" => node_else::configure(state, tokens, pos),
            "if" => node_if::configure(state, tokens, pos),
            "else-if" => node_elseif::configure(state, tokens, pos),
            "institution" => node_institution::configure(state, tokens, pos),
            _ => Ok(()),
        }
    }

    /// `CSL.Engine.prototype.configureTokenList`.
    fn configure_token_list(state: &mut State, tokens: &mut [Token]) -> CslResult<()> {
        let mut dateparts: Vec<String> = Vec::new();
        for ppos in (0..tokens.len()).rev() {
            if tokens[ppos].name == "date" && tokens[ppos].tokentype == TokenType::End {
                dateparts = Vec::new();
            }
            if tokens[ppos].name == "date-part" && js::truthy_opt(tokens[ppos].strings.get("name"))
            {
                let name = tokens[ppos].string("name");
                for part in ["year", "month", "day"] {
                    if part == name {
                        dateparts.push(name.clone());
                    }
                }
            }
            if tokens[ppos].name == "date" && tokens[ppos].tokentype == TokenType::Start {
                dateparts.reverse();
                tokens[ppos].extra.insert(
                    "dateparts".into(),
                    Value::Array(dateparts.iter().cloned().map(Value::String).collect()),
                );
            }
            tokens[ppos].next = Some(ppos + 1);
            dispatch_configure(state, tokens, ppos)?;
        }
        Ok(())
    }

    fn with_area_list<R>(
        state: &mut State,
        area: &str,
        f: impl FnOnce(&mut State, &mut Vec<Token>) -> CslResult<R>,
    ) -> CslResult<R> {
        let mut list = std::mem::take(&mut area_mut(state, area)?.tokens);
        let r = f(state, &mut list);
        area_mut(state, area)?.tokens = list;
        r
    }

    /// `CSL.XmlToToken` for one element.
    fn xml_to_token(
        state: &mut State,
        node: &Value,
        tokentype: TokenType,
        area: &str,
        var_stack: &mut Vec<Vec<String>>,
    ) -> CslResult<()> {
        let name = node["n"].as_str().unwrap_or_default().to_string();
        if let Some(skip) = state.build.skip.clone() {
            if skip != name {
                return Ok(());
            }
        }
        let attrs: Vec<(String, String)> = node["a"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|p| {
                        (
                            format!("@{}", p[0].as_str().unwrap_or_default()),
                            p[1].as_str().unwrap_or_default().to_string(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let attr = |k: &str| attrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone());
        let mut token = Token::new(&name, tokentype);
        state.build.bibliography_tokens_len = state.bibliography.tokens.len();
        if tokentype != TokenType::End || ["if", "else-if", "layout"].contains(&name.as_str()) {
            for (key, val) in &attrs {
                if tokentype == TokenType::End && key != "@language" && key != "@locale" {
                    continue;
                }
                apply(state, &mut token, key, val)?;
            }
            if attr("@variable")
                .map(|v| crate::citeproc::load::DATE_VARIABLES.contains(&v.as_str()))
                .unwrap_or(false)
            {
                var_stack.push(token.variables.clone());
            }
        } else if tokentype == TokenType::End && attr("@variable").is_some() {
            token.extra.insert("hasVariable".into(), Value::Bool(true));
            if crate::citeproc::load::DATE_VARIABLES
                .contains(&attr("@variable").unwrap_or_default().as_str())
            {
                token.variables = var_stack.pop().unwrap_or_default();
            }
        }
        with_area_list(state, area, |state, list| {
            dispatch_build(state, token, list)
        })
    }

    /// `buildStyle` of `CSL.makeBuilder`.
    fn build_style(
        state: &mut State,
        nodes: &[Value],
        parent: bool,
        area: &str,
        var_stack: &mut Vec<Vec<String>>,
    ) -> CslResult<()> {
        for node in nodes {
            let name = node["n"].as_str().unwrap_or_default();
            if parent && name == "date" {
                // CSL.Util.fixDateNode: raises date_key; with a `form` it merges the
                // locale template (not available here), without one it returns early.
                state.build.date_key = true;
                if node["a"]
                    .as_array()
                    .map(|a| a.iter().any(|p| p[0] == "form"))
                    .unwrap_or(false)
                {
                    return Err(EngineError::NotYetPorted {
                        method: "util_datenode.js fixDateNode host",
                    });
                }
            }
            let children = node["c"].as_array().cloned().unwrap_or_default();
            if !children.is_empty() {
                xml_to_token(state, node, TokenType::Start, area, var_stack)?;
                build_style(state, &children, true, area, var_stack)?;
                xml_to_token(state, node, TokenType::End, area, var_stack)?;
            } else {
                xml_to_token(state, node, TokenType::Singleton, area, var_stack)?;
            }
        }
        Ok(())
    }

    fn build_case(case: &Value) -> CslResult<State> {
        let class = case["class"].as_str().unwrap_or("in-text");
        let mut state = fresh_state(class)?;
        let nodes = case["nodes"].as_array().cloned().unwrap_or_default();
        // A `macro` attribute needs the style's `<macro>` node (`cslid`,
        // `macro-has-date`), which this replay has no `cslXml` for; the real
        // engine covers macros in tests/citeproc_intermediate.rs.
        if case["nodes"].to_string().contains("[\"macro\",") {
            return Err(EngineError::NotYetPorted {
                method: "util_nodes.js CSL.expandMacro (needs cslXml)",
            });
        }
        for area in ["citation", "bibliography", "intext"] {
            state.build.area = area.to_string();
            let Some(node) = nodes.iter().find(|n| n["n"] == area) else {
                continue;
            };
            let mut var_stack = Vec::new();
            build_style(
                &mut state,
                std::slice::from_ref(node),
                false,
                area,
                &mut var_stack,
            )?;
        }
        for area in [
            "citation",
            "citation_sort",
            "bibliography",
            "bibliography_sort",
            "intext",
        ] {
            let mut list = std::mem::take(&mut area_mut(&mut state, area)?.tokens);
            let r = configure_token_list(&mut state, &mut list);
            area_mut(&mut state, area)?.tokens = list;
            r?;
        }
        Ok(state)
    }

    fn strip_decorations(v: &mut Value) {
        match v {
            Value::Object(o) => {
                o.remove("decorations");
                o.values_mut().for_each(strip_decorations);
            }
            Value::Array(a) => a.iter_mut().for_each(strip_decorations),
            _ => {}
        }
    }

    fn dump_token(t: &Token) -> Value {
        let mut v = super::super::node_names::token_to_value(t);
        strip_decorations(&mut v);
        if let Value::Object(o) = &mut v {
            for (k, x) in [("next", t.next), ("succeed", t.succeed), ("fail", t.fail)] {
                if let Some(i) = x {
                    o.insert(k.into(), Value::from(i));
                }
            }
        }
        v
    }

    fn pick(opt: &Obj, keys: &[Value]) -> Obj {
        let mut out = Obj::new();
        for k in keys {
            if let Some(k) = k.as_str() {
                if let Some(v) = opt.get(k) {
                    out.insert(k.to_string(), v.clone());
                }
            }
        }
        out
    }

    #[test]
    fn token_lists_match_citeproc_js() {
        let data: Value = serde_json::from_str(DATA).expect("build_tokens.json parses");
        let opt_keys = data["meta"]["opt_keys"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let area_opt_keys = data["meta"]["area_opt_keys"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let cases = data["cases"].as_array().cloned().unwrap_or_default();
        assert!(cases.len() >= 80);
        let (mut compared, mut skipped, mut rejected) = (0, Vec::new(), 0);
        let mut failures: Vec<String> = Vec::new();
        for case in &cases {
            let id = case["id"].as_str().unwrap_or("?").to_string();
            if case.get("error").is_some() {
                rejected += 1;
                continue;
            }
            let state = match build_case(case) {
                Ok(s) => s,
                Err(EngineError::NotYetPorted { .. }) => {
                    skipped.push(id);
                    continue;
                }
                Err(e) => {
                    failures.push(format!("{id}: Rust build failed: {e}"));
                    continue;
                }
            };
            compared += 1;
            let exp = &case["expected"];
            for area in [
                "citation",
                "citation_sort",
                "bibliography",
                "bibliography_sort",
                "intext",
            ] {
                let rust: Vec<Value> = area_ref(&state, area)
                    .map(|a| a.tokens.iter().map(dump_token).collect())
                    .unwrap_or_default();
                let want = exp["areas"][area]["tokens"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                if rust.len() != want.len() {
                    failures.push(format!(
                        "{id}: {area}: {} tokens, citeproc-js has {}",
                        rust.len(),
                        want.len()
                    ));
                    continue;
                }
                for (i, (r, w)) in rust.iter().zip(&want).enumerate() {
                    if r != w {
                        failures.push(format!("{id}: {area}[{i}]\n   rust {r}\n   js   {w}"));
                    }
                }
                let ropt = Value::Object(pick(
                    &area_ref(&state, area)
                        .map(|a| a.opt.clone())
                        .unwrap_or_default(),
                    &area_opt_keys,
                ));
                if ropt != exp["areas"][area]["opt"] {
                    failures.push(format!(
                        "{id}: {area}.opt\n   rust {ropt}\n   js   {}",
                        exp["areas"][area]["opt"]
                    ));
                }
            }
            let ropt = Value::Object(pick(&state.opt, &opt_keys));
            if ropt != exp["opt"] {
                failures.push(format!("{id}: opt\n   rust {ropt}\n   js   {}", exp["opt"]));
            }
            let ca = Value::Object(state.tmp.cite_affixes.clone());
            if ca != exp["cite_affixes"] {
                failures.push(format!(
                    "{id}: cite_affixes\n   rust {ca}\n   js   {}",
                    exp["cite_affixes"]
                ));
            }
            if state.build.date_key != exp["date_key"].as_bool().unwrap_or(false) {
                failures.push(format!("{id}: build.date_key differs"));
            }
        }
        assert!(
            failures.is_empty(),
            "{} mismatches:\n{}",
            failures.len(),
            failures.join("\n")
        );
        assert_eq!(skipped, EXPECTED_SKIPS.to_vec());
        assert_eq!(rejected, 1);
        assert!(compared >= 75, "compared {compared}");
    }

    /// Differential test of the `processNumber`-based condition closures
    /// (`@is-numeric`, `@locator`, `@page`, `@number`) against citeproc-js
    /// 2.4.63 (`scripts/csl-units/attribute_tests.cjs`, data in
    /// `tests/data/csl/units/attribute_tests.json`).
    ///
    /// **Method.** For every item and cite item of the CSL test suite's
    /// fixtures, in en-US and de-DE, the closure is built by the attribute
    /// handler on a fresh `if` token and evaluated once with
    /// `tmp.shadow_numbers` reset, on an engine built from a minimal style.
    /// **Pass criterion:** every result (true / false / error message) equal.
    /// **Result (2026-10-08):** 6,244 cases, all equal. Needs `vendor/` (the
    /// locale files); skipped, with a message, when it is absent.
    #[test]
    fn number_condition_tests_match_citeproc_js() {
        const DATA: &str = include_str!("../../tests/data/csl/units/attribute_tests.json");
        const STYLE: &str = r#"<style xmlns="http://purl.org/net/xbiblio/csl" class="in-text" version="1.0">
  <info><id/><title/><updated>2009-08-10T04:49:00+09:00</updated></info>
  <citation><layout><text variable="title"/></layout></citation>
</style>"#;
        let locales = test_locales();
        if locales.is_empty() {
            eprintln!("skipped: vendor/citeproc-js/locale is absent");
            return;
        }
        let data: Value = serde_json::from_str(DATA).expect("attribute_tests.json parses");
        let mut engines: std::collections::BTreeMap<String, State> = Default::default();
        let mut bad = Vec::new();
        let cases = data["cases"].as_array().cloned().unwrap_or_default();
        for c in &cases {
            let lang = c["lang"].as_str().unwrap_or("en-US").to_string();
            if !engines.contains_key(&lang) {
                let mut sys = crate::citeproc::Sys::default();
                sys.locales = locales.clone();
                let st = State::new(sys, STYLE, &lang, false).expect("engine");
                engines.insert(lang.clone(), st);
            }
            let st = engines.get_mut(&lang).expect("engine");
            let mut tok = Token::new("if", TokenType::Start);
            let attr = c["attr"].as_str().unwrap_or("");
            apply(st, &mut tok, attr, c["arg"].as_str().unwrap_or("")).expect("attribute");
            st.tmp.shadow_numbers = Default::default();
            let test = tok.tests[0].clone();
            let got = test.eval(st, &mut tok, &c["Item"], &c["item"]);
            let ok = match (&got, c.get("error")) {
                (Ok(r), None) => Value::Bool(*r) == c["r"],
                (Err(e), Some(w)) => {
                    w.as_str()
                        == Some(match e {
                            EngineError::BadInput(m) => m.as_str(),
                            _ => "",
                        })
                }
                _ => false,
            };
            if !ok {
                bad.push(format!("{c}: got {got:?}"));
            }
        }
        assert!(cases.len() > 5000, "{}", cases.len());
        assert!(
            bad.is_empty(),
            "{} of {} differ, first 10:\n{}",
            bad.len(),
            cases.len(),
            bad[..bad.len().min(10)].join("\n")
        );
    }
}
