// Part of the kovan port of citeproc-js (GitHub #790, #793).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_transform.js (CSL.Transform)
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

//! Port of `src/util_transform.js`: `CSL.Transform`, the machinery that turns
//! a field of an item into the text to render: the choice among the original,
//! transliterated and translated forms (`getTextSubField`), the abbreviation
//! lookup (`loadAbbreviation`, `abbreviate`, `quashCheck`) and the closure
//! `cs:text` runs for a multilingual variable (`getOutputFunction`).
//!
//! # The host `sys` object
//!
//! `util_transform.js` branches on which optional callbacks the host `sys`
//! provides. The port models the CSL test runner's `sys` (the reference of
//! every comparison here): `getAbbreviation` and `normalizeAbbrevsKey` exist
//! and read [`Sys::abbreviations`](super::Sys) (the runner's `_acache`);
//! `getHumanForm` does not. With an empty cache, which is what a host without
//! abbreviations amounts to, the results are those of a `sys` without
//! `getAbbreviation` (checked by reading the branches in `abbreviate`).
//!
//! # Item mutation
//!
//! Upstream mutates the `Item` it renders (`Item["cite-form"]`, and in
//! attributes.js the authority split). The port passes `&Value`; the write
//! goes through [`set_item_prop`], which stores into the provisional item
//! cache `state.registry.refhash` (the registry's `refhash` upstream), where the
//! next `retrieveItem` finds it, and [`get_item_prop`] reads it back for the
//! rest of the same pass. When the registry lands, both functions are the one
//! place to point at it.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::load::{
    self, abbreviation_segments, demote_noise_words, field_category_remap, get_abbrevs_domain,
    lang_prefs_map, update_group_context_condition, DOT, VARIABLES_WITH_SHORT_FORM,
};
use super::obj_token::{Decoration, Token, TokenType};
use super::queue::{self, AppendArg, FormatRef, QueueId};
use super::state::State;
use super::{js, CslResult, EngineError};

/// `transform.abbrevs`: jurisdiction, then category (the keys of
/// `CSL.AbbreviationSegments`), then normalised key, to the abbreviation.
pub type AbbrevTable = BTreeMap<String, BTreeMap<String, BTreeMap<String, String>>>;

/// `new CSL.AbbreviationSegments()`: twelve empty categories.
pub fn new_segments() -> BTreeMap<String, BTreeMap<String, String>> {
    abbreviation_segments()
        .into_iter()
        .map(|(k, _)| (k, BTreeMap::new()))
        .collect()
}

/// `CSL.Transform` (the data: `this.abbrevs`; its functions are the free
/// functions of this module).
#[derive(Debug, Clone, PartialEq)]
pub struct Transform {
    /// `this.abbrevs`; `abbrevs["default"]` exists from the start.
    pub abbrevs: AbbrevTable,
}

impl Default for Transform {
    /// `new CSL.Transform(state)`: `this.abbrevs = {}` and
    /// `this.abbrevs["default"] = new state.sys.AbbreviationSegments()`.
    fn default() -> Self {
        let mut abbrevs = AbbrevTable::new();
        abbrevs.insert("default".to_string(), new_segments());
        Transform { abbrevs }
    }
}

impl Transform {
    /// `abbrevs[jurisdiction][category][key]`, `None` where JS gives
    /// `undefined` (the jurisdiction, category or key is absent). An empty
    /// string is returned as it is; callers test truthiness as upstream does.
    pub fn abbrev(&self, jurisdiction: &str, category: &str, key: &str) -> Option<&str> {
        self.abbrevs
            .get(jurisdiction)
            .and_then(|c| c.get(category))
            .and_then(|k| k.get(key))
            .map(String::as_str)
    }

    /// Whether `abbrevs[jurisdiction][category]` exists (an object, so truthy).
    pub fn has_category(&self, jurisdiction: &str, category: &str) -> bool {
        self.abbrevs
            .get(jurisdiction)
            .map(|c| c.contains_key(category))
            .unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// Item properties written during rendering
// ---------------------------------------------------------------------------

/// `Item[key] = value` for the item being rendered: stored in the item cache
/// (see the module docs). A no-op for an item without an `id` or not in the
/// cache.
pub fn set_item_prop(state: &mut State, item: &Value, key: &str, value: Value) {
    let Some(id) = item.get("id").map(js::to_js_string) else {
        return;
    };
    if let Some(Value::Object(o)) = state.registry.refhash.get_mut(&id) {
        o.insert(key.to_string(), value);
    }
}

/// `Item[key]` after [`set_item_prop`]: the cached item's value when the
/// cache has the key, else the one of `item` itself.
pub fn get_item_prop(state: &State, item: &Value, key: &str) -> Option<Value> {
    if let Some(id) = item.get("id").map(js::to_js_string) {
        if let Some(v) = state.registry.refhash.get(&id).and_then(|i| i.get(key)) {
            return Some(v.clone());
        }
    }
    item.get(key).cloned()
}

// ---------------------------------------------------------------------------
// Seams to code that belongs to other parts of the port
// ---------------------------------------------------------------------------

/// `state.publisherOutput` (util_publishers.js, created by the group closure
/// of node_group.js). PORT-LATER(w2-names): util_publishers.js is not ported,
/// so there is never one, and `publisherCheck` returns false.
fn publisher_output_present(_state: &State) -> bool {
    false
}

/// `state.tmp["publisher-list"]` (set by util_publishers.js). PORT-LATER(w2-names).
fn publisher_list_active(_state: &State) -> bool {
    false
}

/// `state.tmp.name_node.children.push(state.output.current.value())`
/// (getOutputFunction's last statement, run inside cs:substitute).
/// PORT-LATER(w2-names): `tmp.name_node` belongs to the cs:names code.
fn name_node_push_current(_state: &mut State) -> CslResult<()> {
    Ok(())
}

// ---------------------------------------------------------------------------
// The runner's sys
// ---------------------------------------------------------------------------

/// `sys.getAbbreviation(styleID, abbrevs, jurisdiction, category, key)` of
/// the test runner (scripts/csl-testsuite-reference.cjs `Sys.prototype.
/// getAbbreviation`): walk the jurisdiction and its `:`-prefixes, most
/// specific first, then `default`; copy the first cached abbreviation of
/// `key` into `transform.abbrevs`; return the jurisdiction where it stopped
/// (`default` when none matched).
fn sys_get_abbreviation(
    state: &mut State,
    jurisdiction: &str,
    category: &str,
    key: &str,
) -> String {
    let mut jurisdictions: Vec<String> = vec!["default".to_string()];
    if jurisdiction != "default" {
        let lst: Vec<&str> = jurisdiction.split(':').collect();
        for i in 1..=lst.len() {
            jurisdictions.push(lst[..i].join(":"));
        }
    }
    jurisdictions.reverse();
    let mut myjurisdiction = String::new();
    for j in jurisdictions {
        myjurisdiction = j.clone();
        state
            .transform
            .abbrevs
            .entry(j.clone())
            .or_insert_with(new_segments);
        let cached = state
            .sys
            .abbreviations
            .get(&j)
            .and_then(|c| c.get(category))
            .and_then(|k| k.get(key))
            .filter(|v| !v.is_empty())
            .cloned();
        if let Some(v) = cached {
            if let Some(seg) = state.transform.abbrevs.get_mut(&j) {
                seg.entry(category.to_string())
                    .or_default()
                    .insert(key.to_string(), v);
            }
            break;
        }
    }
    myjurisdiction
}

/// `sys.getHumanForm` does not exist on the runner's `sys`, so
/// `getCountryOrJurisdiction` yields `""` (util_transform.js:59-76).
fn get_country_or_jurisdiction(
    _variable: &str,
    _normalized_key: &str,
    _quash_country: bool,
) -> String {
    String::new()
}

/// `sys.normalizeAbbrevsKey(variable, key)` of the runner for a value that
/// may be absent or not a string (`("" + key).trim()` after a falsy test).
fn normalize_key(variable: &str, key: &Value) -> String {
    if js::truthy(key) {
        super::build_retrieve_item::normalize_abbrevs_key(variable, Some(&js::to_js_string(key)))
    } else {
        super::build_retrieve_item::normalize_abbrevs_key(variable, None)
    }
}

// ---------------------------------------------------------------------------
// loadAbbreviation, abbreviate
// ---------------------------------------------------------------------------

/// `CSL.Transform.loadAbbreviation(jurisdiction, category, orig, lang)`:
/// make sure `transform.abbrevs` has the abbreviation for `orig` (loading it
/// from the host) and return the jurisdiction key it lives under. With an
/// empty `orig` only the empty tables are created.
pub fn load_abbreviation(
    state: &mut State,
    jurisdiction: Option<&str>,
    category: &str,
    orig: &str,
    lang: Option<&str>,
) -> String {
    let mut jurisdiction = match jurisdiction {
        Some(j) if !j.is_empty() => j.to_string(),
        _ => "default".to_string(),
    };
    let country = jurisdiction.split(':').next().unwrap_or("").to_string();
    let domain = get_abbrevs_domain(state, &country, lang.unwrap_or("undefined"));
    if let Some(d) = &domain {
        jurisdiction.push('@');
        jurisdiction.push_str(d);
    }
    if orig.is_empty() {
        let seg = state
            .transform
            .abbrevs
            .entry(jurisdiction.clone())
            .or_insert_with(new_segments);
        seg.entry(category.to_string()).or_default();
        return jurisdiction;
    }
    // The getAbbreviation() function checks the external DB for the content
    // key (here: the runner's cache).
    let got = sys_get_abbreviation(state, &jurisdiction, category, orig);
    if got.is_empty() {
        let mut j = "default".to_string();
        if let Some(d) = &domain {
            j.push('@');
            j.push_str(d);
        }
        return j;
    }
    got
}

/// The internal `abbreviate(state, tok, Item, altvar, basevalue, family_var,
/// use_field)` of util_transform.js:79-169: the abbreviation of `basevalue`
/// (a field value) for the family `family_var`, or `basevalue` itself.
///
/// `family_var` is a variable name here (upstream passes `false` for none and
/// `abbreviate` is only called with a name). Returns the abbreviation, or
/// `basevalue` (any JSON value) when there is none.
fn abbreviate(
    state: &mut State,
    tok: &Token,
    item: &Value,
    altvar: Option<&str>,
    basevalue: &Value,
    family_var: &str,
    use_field: bool,
) -> CslResult<Value> {
    let myabbrev_family = match field_category_remap(family_var) {
        Some(f) => f,
        None => return Ok(basevalue.clone()),
    };
    let mut value = Value::String(String::new());

    let variable = family_var;
    let mut normalized_key = normalize_key(family_var, basevalue);
    let mut quash_country = false;
    if variable == "jurisdiction" && !normalized_key.is_empty() {
        quash_country = !normalized_key.contains(':');
    }
    // Fix up jurisdiction codes
    if variable == "jurisdiction" || variable == "country" {
        let Value::String(b) = basevalue else {
            return Err(EngineError::BadInput(
                "basevalue.toLowerCase is not a function".into(),
            ));
        };
        if *b == b.to_lowercase() {
            normalized_key = b.to_uppercase();
        }
    }

    // Lazy retrieval of abbreviations (sys.getAbbreviation exists).
    let preferred_jurisdiction: String = if [
        "jurisdiction",
        "country",
        "language-name",
        "language-name-original",
    ]
    .contains(&variable)
    {
        "default".to_string()
    } else if js::truthy_opt(item.get("jurisdiction")) {
        item.get("jurisdiction")
            .map(js::to_js_string)
            .unwrap_or_default()
    } else {
        "default".to_string()
    };
    let language = item
        .get("language")
        .filter(|l| js::truthy(l))
        .map(js::to_js_string);
    let jurisdiction = load_abbreviation(
        state,
        Some(&preferred_jurisdiction),
        myabbrev_family,
        &normalized_key,
        language.as_deref(),
    );

    // Some rules: see util_transform.js:116-135.
    if state.transform.has_category(&jurisdiction, myabbrev_family) && !normalized_key.is_empty() {
        // Safe to test presence of abbrev against raw object in this block
        let abbrev = state
            .transform
            .abbrev(&jurisdiction, myabbrev_family, &normalized_key)
            .map(str::to_string);
        let abbrev_truthy = abbrev.as_deref().map(|a| !a.is_empty()).unwrap_or(false);
        if tok.string_opt("form").as_deref() == Some("short") && abbrev_truthy {
            if quash_country {
                value = Value::String(String::new());
            } else {
                value = Value::String(abbrev.unwrap_or_default());
            }
        } else {
            value = Value::String(get_country_or_jurisdiction(
                variable,
                &normalized_key,
                quash_country,
            ));
        }
    }

    // Was for:
    if !js::truthy(&value)
        && (!dev_ext(state, "require_explicit_legal_case_title_short")
            || item.get("type").and_then(Value::as_str) != Some("legal_case"))
    {
        if let (Some(a), true) = (altvar, use_field) {
            if let Some(v) = item.get(a).filter(|v| js::truthy(v)) {
                value = v.clone();
            }
        }
    }
    // (`!state.sys.getAbbreviation && state.sys.getHumanForm`: not the runner.)
    if !js::truthy(&value) && !quash_country {
        // (`!state.sys.getHumanForm || variable !== "jurisdiction"`: no getHumanForm.)
        value = basevalue.clone();
    }
    if dev_ext(state, "force_title_abbrev_fallback")
        && variable == "title"
        && value == *basevalue
        && js::truthy_opt(item.get("title-short"))
    {
        value = item.get("title-short").cloned().unwrap_or(Value::Null);
    }
    Ok(value)
}

fn dev_ext(state: &State, key: &str) -> bool {
    load::dev_ext_truthy(state, key)
}

// ---------------------------------------------------------------------------
// getFieldLocale, getTextSubField
// ---------------------------------------------------------------------------

/// The internal `getFieldLocale(Item, field)` (util_transform.js:171-197).
fn get_field_locale(state: &State, item: &Value, field: &str) -> CslResult<String> {
    let default_locale = state
        .opt
        .get("default-locale")
        .and_then(|v| v.get(0))
        .filter(|v| v.is_string())
        .map(js::to_js_string)
        .ok_or_else(|| {
            EngineError::BadInput("Cannot read properties of undefined (reading 'slice')".into())
        })?;
    let mut ret = js::slice(&default_locale, 0, Some(2));
    let strict = dev_ext(state, "strict_text_case_locales");
    if js::truthy_opt(item.get("language")) {
        let lang = item
            .get("language")
            .map(js::to_js_string)
            .unwrap_or_default();
        // /^([a-zA-Z]{2})(?:$|-.*|.*)/ or, strict, /^([a-zA-Z]{2})(?:$|-.*| .*)/
        let units: Vec<char> = lang.chars().take(3).collect();
        let two_alpha =
            units.len() >= 2 && units[0].is_ascii_alphabetic() && units[1].is_ascii_alphabetic();
        let ok = two_alpha && (!strict || units.len() == 2 || units[2] == '-' || units[2] == ' ');
        ret = if ok {
            units[..2].iter().collect()
        } else {
            // Set garbage to "Klingon".
            "tlh".to_string()
        };
    }
    let main = item
        .get("multi")
        .filter(|m| js::truthy(m))
        .and_then(|m| m.get("main"))
        .filter(|m| js::truthy(m))
        .and_then(|m| m.get(field))
        .filter(|v| js::truthy(v));
    if let Some(m) = main {
        ret = js::to_js_string(m);
    }
    if !strict || dev_ext(state, "normalize_lang_keys_to_lowercase") {
        ret = ret.to_lowercase();
    }
    Ok(ret)
}

/// What `getTextSubField` returns: `{name, usedOrig, locale, token,
/// found_variant_ok}`.
#[derive(Debug, Clone, PartialEq)]
pub struct TextSubField {
    /// `name`: the text (`Value::Null` is JS `undefined`).
    pub name: Value,
    /// `usedOrig`.
    pub used_orig: bool,
    /// `locale` (`None` on the early return for an absent field).
    pub locale: Option<String>,
    /// `token`: a clone of `this`, for the caller to style the output with.
    pub token: Token,
    /// `found_variant_ok`.
    pub found_variant_ok: bool,
}

/// `Item.multi._keys[field][opt]` when truthy (util_transform.js:262-275). A
/// `multi` without `_keys` is a TypeError upstream.
fn multi_key(item: &Value, field: &str, opt: &str) -> CslResult<Option<Value>> {
    let Some(multi) = item.get("multi").filter(|m| js::truthy(m)) else {
        return Ok(None);
    };
    let Some(keys) = multi.get("_keys").filter(|k| !k.is_null()) else {
        return Err(EngineError::BadInput(format!(
            "Cannot read properties of undefined (reading '{field}')"
        )));
    };
    Ok(keys
        .get(field)
        .filter(|f| js::truthy(f))
        .and_then(|f| f.get(opt))
        .filter(|v| js::truthy(v))
        .cloned())
}

/// `transform.getTextSubField(Item, field, locale_type, use_default,
/// stopOrig, family_var)` called with `this` = `this_token` (`None` when
/// called as a method of the transform, as `processNumber` does: `this` is
/// then not a token and its clone is an empty token).
///
/// `locale_type` is `"locale-orig"`, `"locale-translit"`, `"locale-sort"`, ...
/// (`None` is JS `false`). `family_var` is truthy only when the short form
/// was requested.
#[allow(clippy::too_many_arguments)]
pub fn get_text_sub_field(
    state: &State,
    this_token: Option<&Token>,
    item: &Value,
    field: &str,
    locale_type: Option<&str>,
    use_default: bool,
    stop_orig: bool,
    family_var: Option<&str>,
) -> CslResult<TextSubField> {
    let clone_this = || -> Token {
        match this_token {
            Some(t) => t.clone_token(),
            None => Token::new("", TokenType::Start),
        }
    };
    let used_orig = stop_orig;
    let mut using_orig = false;

    if !js::truthy_opt(item.get(field)) {
        return Ok(TextSubField {
            name: Value::String(String::new()),
            used_orig: stop_orig,
            locale: None,
            token: clone_this(),
            found_variant_ok: false,
        });
    }
    // If form="short" is selected ("family_var" is a misnomer here, it means
    // short-form requested), and the variable has a short-form partner (i.e.
    // it is in array VARIABLES_WITH_SHORT_FORM), then it is run here as
    // *-short".
    let mut field = field.to_string();
    let mut sticky_long_form = false;
    if VARIABLES_WITH_SHORT_FORM.contains(&field.as_str())
        && family_var.map(|f| !f.is_empty()).unwrap_or(false)
    {
        field.push_str("-short");
        sticky_long_form = true;
    }
    let mut break_me = false;
    let mut first_value: Option<TextSubField> = None;
    let mut fields_to_try: Vec<String> = Vec::new();
    if js::slice(&field, -6, None) == "-short" {
        fields_to_try.push(field.clone());
        fields_to_try.push(js::slice(&field, 0, Some(-6)));
    } else {
        fields_to_try.push(field.clone());
    }

    let mut ret = TextSubField {
        name: Value::Null,
        used_orig: false,
        locale: None,
        token: clone_this(),
        found_variant_ok: false,
    };
    for (h, f) in fields_to_try.iter().enumerate() {
        field = f.clone();
        let mut variant_match = false;

        ret = TextSubField {
            name: Value::String(String::new()),
            used_orig: stop_orig,
            locale: Some(get_field_locale(state, item, &field)?),
            token: clone_this(),
            found_variant_ok: false,
        };

        let opts: Vec<String> = match locale_type.and_then(|t| state.opt.get(t)) {
            Some(Value::Array(a)) => a.iter().map(js::to_js_string).collect(),
            _ => Vec::new(),
        };
        let mut has_val = false;

        if locale_type == Some("locale-orig") {
            if !stop_orig {
                ret.name = item.get(&field).cloned().unwrap_or(Value::Null);
                ret.used_orig = false;
            }
            has_val = true;
            using_orig = true;
        } else if use_default && opts.is_empty() {
            // If we want the original, or if we don't have any specific
            // guidance and we definitely want output, just return the
            // original value.
            ret.name = item.get(&field).cloned().unwrap_or(Value::Null);
            ret.used_orig = true;
            has_val = true;
            using_orig = true;
        }

        if !has_val {
            for opt in &opts {
                let o = opt.split(['-', '_']).next().unwrap_or("");
                if !opt.is_empty() {
                    if let Some(v) = multi_key(item, &field, opt)? {
                        ret.name = v;
                        ret.locale = Some(opt.clone());
                        variant_match = true;
                        using_orig = false;
                        break;
                    }
                }
                if !o.is_empty() {
                    if let Some(v) = multi_key(item, &field, o)? {
                        ret.name = v;
                        ret.locale = Some(o.to_string());
                        variant_match = true;
                        using_orig = false;
                        break;
                    }
                }
            }
            if !js::truthy(&ret.name) && use_default {
                ret = TextSubField {
                    name: item.get(&field).cloned().unwrap_or(Value::Null),
                    used_orig: true,
                    locale: Some(get_field_locale(state, item, &field)?),
                    token: clone_this(),
                    found_variant_ok: false,
                };
                using_orig = true;
            }
        }
        ret.token = clone_this();
        if h == 0 {
            if variant_match {
                ret.found_variant_ok = true;
            }
            first_value = Some(ret.clone());
            if !sticky_long_form && opts.is_empty() {
                break_me = true;
            }
            if variant_match {
                break_me = true;
            }
        } else if !sticky_long_form && !variant_match && first_value.is_some() {
            if let Some(fv) = first_value.clone() {
                ret = fv;
            }
            field = fields_to_try[0].clone();
        } else if variant_match {
            ret.found_variant_ok = true;
        }
        if ["title", "container-title"].contains(&field.as_str()) {
            let tc = ret.token.string_opt("text-case");
            let tc_plain = !tc.as_deref().map(|t| !t.is_empty()).unwrap_or(false);
            if !used_orig
                && (tc_plain
                    || tc.as_deref() == Some("sentence")
                    || tc.as_deref() == Some("normal"))
            {
                let lang: Option<String> = if using_orig { None } else { ret.locale.clone() };
                let seg = js::slice(&field, 0, Some(-5));
                let sentence_case = tc.as_deref() == Some("sentence");
                let item_obj = item.as_object().cloned().unwrap_or_default();
                let t = state.titlecase_sentence_or_normal(
                    &item_obj,
                    &seg,
                    lang.as_deref(),
                    sentence_case,
                )?;
                ret.name = Value::String(t);
                ret.token.strings.remove("text-case");
            }
        }
        if break_me {
            break;
        }
    }
    Ok(ret)
}

// ---------------------------------------------------------------------------
// quashCheck, citeFormCheck, publisherCheck
// ---------------------------------------------------------------------------

static QUASH_RE: LazyLock<Regex> = LazyLock::new(|| {
    // /^(?:#[0-9]+)*(?:!((?:[-_a-z]+(?:(?:.*)))(?:,(?:[-_a-z]+(?:(?:.*))))*))*>>>/
    #[allow(clippy::expect_used)]
    Regex::new(&format!(
        "^(?:#[0-9]+)*(?:!((?:[-_a-z]+(?:(?:{DOT}*)))(?:,(?:[-_a-z]+(?:(?:{DOT}*))))*))*>>>"
    ))
    .expect("static regex")
});

static QUASH_FIELD_RE: LazyLock<Regex> = LazyLock::new(|| {
    // /^([-_a-z]+)(?:\:(.*))*$/
    #[allow(clippy::expect_used)]
    Regex::new(&format!("^([-_a-z]+)(?::({DOT}*))*$")).expect("static regex")
});

static CITE_FORM_RE: LazyLock<Regex> = LazyLock::new(|| {
    // /^#([0-9]+).*>>>/
    #[allow(clippy::expect_used)]
    Regex::new(&format!("^#([0-9]+){DOT}*>>>")).expect("static regex")
});

/// The internal `citeFormCheck(Item, value)`: a `#N` prefix before `>>>` in
/// an abbreviation sets `Item["cite-form"]` to `N`.
fn cite_form_check(state: &mut State, item: &Value, value: &str) {
    if let Some(m) = CITE_FORM_RE.captures(value).and_then(|c| c.get(1)) {
        if !m.as_str().is_empty() {
            set_item_prop(
                state,
                item,
                "cite-form",
                Value::String(m.as_str().to_string()),
            );
        }
    }
}

/// `transform.quashCheck(jurisdiction, value)`: strip the `!field,field>>>`
/// hack syntax from the front of an abbreviation, quashing the named fields
/// for the rest of the cite (they are added to `done_vars` and recorded in
/// `tmp.abbrev_trimmer`). Returns the rest of `value`.
pub fn quash_check(
    state: &mut State,
    jurisdiction: Option<&str>,
    value: &str,
) -> CslResult<String> {
    let Some(caps) = QUASH_RE.captures(value) else {
        return Ok(value.to_string());
    };
    let end = caps.get(0).map(|m| m.end()).unwrap_or(0);
    let rest = value[end..].to_string();
    if let Some(m1) = caps.get(1) {
        let jurisdiction = jurisdiction.filter(|j| !j.is_empty());
        for raw_field in m1.as_str().split(',') {
            let Some(mm) = QUASH_FIELD_RE.captures(raw_field) else {
                return Err(EngineError::BadInput(
                    "Cannot read properties of null (reading '1')".into(),
                ));
            };
            let field = mm.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
            let arg = mm
                .get(2)
                .map(|m| m.as_str().to_string())
                .filter(|s| !s.is_empty());
            // trimmer is not available in getAmbiguousCite
            if let Some(arg) = arg {
                if let (Some(trimmer), Some(j)) =
                    (state.tmp.abbrev_trimmer.as_mut(), jurisdiction)
                {
                    trimmer
                        .fields
                        .entry(j.to_string())
                        .or_default()
                        .insert(field, arg);
                }
            } else if !state.tmp.done_vars.contains(&field) {
                if let (Some(trimmer), Some(j)) =
                    (state.tmp.abbrev_trimmer.as_mut(), jurisdiction)
                {
                    trimmer
                        .quashes
                        .entry(j.to_string())
                        .or_default()
                        .insert(field.clone(), true);
                }
                state.tmp.done_vars.push(field);
            }
        }
    }
    Ok(rest)
}

/// The internal `publisherCheck(tok, Item, primary, family_var)`: whether the
/// publisher bundle took over `primary`.
/// PORT-LATER(w2-names): needs `state.publisherOutput`
/// (util_publishers.js); there is never one, so this is false.
fn publisher_check(state: &State, _tok: &Token, primary: &Value) -> bool {
    publisher_output_present(state) && js::truthy(primary)
}

// ---------------------------------------------------------------------------
// getOutputFunction
// ---------------------------------------------------------------------------

/// A value as the string `String.prototype.match` etc. would be called on;
/// JS throws for a non-string.
fn expect_string(v: &Value, what: &str) -> CslResult<String> {
    match v {
        Value::String(s) => Ok(s.clone()),
        _ => Err(EngineError::BadInput(format!("{what} is not a function"))),
    }
}

/// `strings[key]` of an `opt.citeAffixes[langPrefs][slot]` entry.
fn cite_affix(state: &State, lang_prefs: &str, slot: &str, which: &str) -> CslResult<String> {
    state
        .opt
        .get("citeAffixes")
        .and_then(|a| a.get(lang_prefs))
        .and_then(|a| a.get(slot))
        .map(|a| {
            a.get(which)
                .map(js::to_js_string)
                .unwrap_or_else(|| "undefined".to_string())
        })
        .ok_or_else(|| {
            EngineError::BadInput(format!(
                "Cannot read properties of undefined (reading '{which}')"
            ))
        })
}

/// Remove the decorations a secondary or tertiary form must not carry (quotes,
/// italics, bold): the loop of util_transform.js:633-637 / 668-672.
fn strip_secondary_decorations(tok: &mut Token) {
    let mut i = tok.decorations.len() as i64 - 1;
    while i > -1 {
        let d = &tok.decorations[i as usize];
        let mut joined = format!("{}/{}", d.name, d.value);
        if let Some(e) = &d.extra {
            joined.push('/');
            joined.push_str(e);
        }
        if [
            "@quotes/true",
            "@font-style/italic",
            "@font-style/oblique",
            "@font-weight/bold",
        ]
        .contains(&joined.as_str())
        {
            tok.decorations.remove(i as usize);
        }
        i -= 1;
    }
}

static TRAILING_SEP_RE: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new("[ .,]+$").expect("static regex")
});

/// `[locale].concat(oldLangArray)`.
fn lang_array_with(locale: &str, old: &[String]) -> Vec<String> {
    let mut la = vec![locale.to_string()];
    la.extend(old.iter().cloned());
    la
}

/// The closure `transform.getOutputFunction(variables, family_var,
/// abbreviation_fallback, alternative_varname)` returns, run with `this` =
/// `token` for the item `item` and cite item `cite_item` (`Value::Null` when
/// there is none): render a multilingual field in its primary, secondary and
/// tertiary forms.
///
/// `variables` is read from `token.variables` at run time: upstream passes
/// the token's live array (`this.variables`), which `@variable`'s first
/// closure rewrites in place before this one runs. `family_var` is the
/// variable to abbreviate (`None` for upstream's `false`). The fallback and
/// `transfall` parameters of upstream are never read there and are not
/// modelled.
pub fn run_output_function(
    state: &mut State,
    token: &Token,
    family_var: Option<&str>,
    alternative_varname: Option<&str>,
    item: &Value,
    cite_item: &Value,
) -> CslResult<Option<usize>> {
    let variables: Vec<String> = token.variables.clone();
    let lang_prefs: Option<&'static str> = variables.first().and_then(|v| lang_prefs_map(v));
    let localesets: Option<Vec<String>> = lang_prefs.and_then(|lp| {
        state
            .opt
            .get("cite-lang-prefs")
            .and_then(|p| p.get(lp))
            .and_then(Value::as_array)
            .map(|a| a.iter().map(js::to_js_string).collect())
    });

    let Some(var0) = variables.first().cloned().filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    let altkey = alternative_varname.unwrap_or("false");
    if !js::truthy_opt(item.get(var0.as_str())) && !js::truthy_opt(item.get(altkey)) {
        return Ok(None);
    }
    //
    // Exploring the edges here.
    // "suppress-author" for string variables (mostly titles).
    //
    if !state.tmp.just_looking
        && js::truthy(cite_item)
        && js::truthy_opt(cite_item.get("suppress-author"))
        && !state.tmp.probably_rendered_something
        && state.tmp.can_substitute.len() > 1
    {
        return Ok(None);
    }
    let mut primary_slot: Option<String> = None;
    let mut secondary_slot: Option<String> = None;
    let mut tertiary_slot: Option<String> = None;
    // `state.tmp.multi_layout` is never assigned in citeproc-js 2.4.63
    // (`state.opt.multi_layout` is), so it is always falsy here.
    let multi_layout = false;
    let single_orig = matches!(&localesets, Some(l) if l.len() == 1 && l[0] == "locale-orig");
    if state.tmp.area.ends_with("_sort") {
        primary_slot = Some("locale-sort".to_string());
    } else if single_orig {
        primary_slot = Some("locale-orig".to_string());
    } else if let (Some(ls), false) = (localesets.as_ref(), multi_layout) {
        for i in 0..3usize {
            if (ls.len() as i64) - 1 < i as i64 {
                break;
            }
            if !ls[i].is_empty() {
                let v = Some(format!("locale-{}", ls[i]));
                match i {
                    0 => primary_slot = v,
                    1 => secondary_slot = v,
                    _ => tertiary_slot = v,
                }
            }
        }
    } else {
        primary_slot = Some("locale-orig".to_string());
    }

    let has_item = js::truthy(cite_item);
    if var0 == "title-short"
        || (state.tmp.area != "bibliography"
            && !(state.tmp.area == "citation"
                && js::get_str(&state.opt, "xclass") == Some("note")
                && has_item
                && !js::truthy_opt(cite_item.get("position"))))
    {
        secondary_slot = None;
        tertiary_slot = None;
    }

    if multi_layout {
        secondary_slot = None;
        tertiary_slot = None;
    }

    // Problem for multilingual: we really should be checking for sanity on
    // the basis of the output strings to be actually used. (also below)
    if publisher_list_active(state) {
        // state.tmp["publisher-token"] = this; ... return null
        return Ok(None);
    }

    // tmp.lang_array carries the current locale IDs of the style and the
    // item. Field-level locale IDs are added here, so we clone it to allow
    // reset.
    let old_lang_array = state.tmp.lang_array.clone();

    // True is for transform fallback
    let mut res = get_text_sub_field(
        state,
        Some(token),
        item,
        &var0,
        primary_slot.as_deref(),
        true,
        false,
        family_var,
    )?;
    let mut primary = res.name.clone();
    let primary_locale = res.locale.clone();
    let mut primary_tok = res.token.clone();
    let primary_used_orig = res.used_orig;
    let family = family_var.filter(|f| !f.is_empty());
    if let (Some(fv), false) = (family, res.found_variant_ok) {
        primary = abbreviate(
            state,
            &primary_tok,
            item,
            alternative_varname,
            &primary,
            fv,
            true,
        )?;
        // Suppress subsequent use of another variable if requested by hack
        // syntax in this abbreviation short form.
        if js::truthy(&primary) {
            // We run quash-check in getAmbiguousCite, to possibly pick up a
            // cite-form value.
            let s = expect_string(&primary, "value.match")?;
            cite_form_check(state, item, &s);
            if !state.tmp.just_looking {
                let jur = item
                    .get("jurisdiction")
                    .filter(|j| js::truthy(j))
                    .map(js::to_js_string);
                primary = Value::String(quash_check(state, jur.as_deref(), &s)?);
            }
        }
    }
    if publisher_check(state, token, &primary) {
        state.tmp.lang_array = old_lang_array;
        return Ok(None);
    }

    // No fallback for secondary and tertiary
    let mut secondary = Value::Bool(false);
    let mut tertiary = Value::Bool(false);
    let mut secondary_tok = Token::new("", TokenType::Start);
    let mut tertiary_tok = Token::new("", TokenType::Start);
    let mut secondary_locale: Option<String> = None;
    let mut tertiary_locale: Option<String> = None;
    if let Some(slot) = secondary_slot.clone() {
        // (upstream passes `family_var` as a 7th argument, which the
        // function does not read; its 6th, `family_var`, is null.)
        res = get_text_sub_field(
            state,
            Some(token),
            item,
            &var0,
            Some(&slot),
            false,
            res.used_orig,
            None,
        )?;
        secondary = res.name.clone();
        secondary_locale = res.locale.clone();
        secondary_tok = res.token.clone();
        if let (Some(fv), false) = (family, res.found_variant_ok) {
            if js::truthy(&secondary) {
                // The abbreviate() function could use a cleanup, after Zotero
                // correct to use title-short
                secondary = abbreviate(state, &secondary_tok, item, None, &secondary, fv, true)?;
            }
        }
    }
    if let Some(slot) = tertiary_slot.clone() {
        res = get_text_sub_field(
            state,
            Some(token),
            item,
            &var0,
            Some(&slot),
            false,
            res.used_orig,
            None,
        )?;
        tertiary = res.name.clone();
        tertiary_locale = res.locale.clone();
        tertiary_tok = res.token.clone();
        if let (Some(fv), false) = (family, res.found_variant_ok) {
            if js::truthy(&tertiary) {
                tertiary = abbreviate(state, &tertiary_tok, item, None, &tertiary, fv, true)?;
            }
        }
    }

    // Decoration of primary (currently translit only) goes here
    let mut primary_prefix: Option<String> = None;
    if primary_slot.as_deref() == Some("locale-translit") {
        let lp = lang_prefs.unwrap_or("undefined");
        primary_prefix = Some(cite_affix(state, lp, "locale-translit", "prefix")?);
    }
    // XXX This should probably protect against italics at higher levels.

    if primary_prefix.as_deref() == Some("<i>") && var0 == "title" && !primary_used_orig {
        let mut has_italic = false;
        for d in &primary_tok.decorations {
            if d.name == "@font-style" && d.value == "italic" {
                has_italic = true;
            }
        }
        if !has_italic {
            primary_tok
                .decorations
                .push(Decoration::new("@font-style", "italic"));
        }
    }

    if primary_locale.as_deref() != Some("en")
        && primary_tok.string_opt("text-case").as_deref() == Some("title")
    {
        primary_tok.set_string("text-case", "passthrough");
    }

    if var0 == "title" {
        let noise = token
            .extra
            .get("leading-noise-words")
            .map(js::to_js_string)
            .unwrap_or_default();
        if js::truthy(&primary) {
            let p = js::to_js_string(&primary);
            primary = Value::String(demote_noise_words(state, &p, &noise));
        }
    }
    let lp = lang_prefs.unwrap_or("undefined");
    if js::truthy(&secondary) || js::truthy(&tertiary) {
        queue::open_level(state, QueueId::Output, FormatRef::Name("empty".into()))?;

        // A little too aggressive maybe.
        let suffix = primary_tok.string("suffix");
        primary_tok.set_string("suffix", &TRAILING_SEP_RE.replace(&suffix, ""));
        if let Some(pl) = &primary_locale {
            state.tmp.lang_array = lang_array_with(pl, &old_lang_array);
        }
        let pv = format!(
            "{}{}",
            primary_tok.string("prefix"),
            js::to_js_string(&primary)
        );
        update_group_context_condition(state, None, false, Some(&primary_tok), Some(&pv));
        append_value(state, &primary, &primary_tok)?;
        state.tmp.probably_rendered_something = true;

        if primary == secondary {
            secondary = Value::Bool(false);
        }
        if js::truthy(&secondary) {
            let slot = secondary_slot.clone().unwrap_or_default();
            secondary_tok.set_string("prefix", &cite_affix(state, lp, &slot, "prefix")?);
            secondary_tok.set_string("suffix", &cite_affix(state, lp, &slot, "suffix")?);
            // Add a space if empty
            if secondary_tok.string("prefix").is_empty() {
                secondary_tok.set_string("prefix", " ");
            }
            // Remove quotes
            strip_secondary_decorations(&mut secondary_tok);
            if secondary_locale.as_deref() != Some("en")
                && secondary_tok.string_opt("text-case").as_deref() == Some("title")
            {
                secondary_tok.set_string("text-case", "passthrough");
            }
            if let Some(sl) = &secondary_locale {
                state.tmp.lang_array = lang_array_with(sl, &old_lang_array);
            }
            let mut secondary_outer = Token::new("", TokenType::Start);
            secondary_outer
                .decorations
                .push(Decoration::new("@font-style", "normal"));
            secondary_outer
                .decorations
                .push(Decoration::new("@font-weight", "normal"));
            queue::open_level(state, QueueId::Output, FormatRef::Token(secondary_outer))?;
            append_value(state, &secondary, &secondary_tok)?;
            queue::close_level(state, QueueId::Output, None)?;

            // Suppress supplementary multilingual info on subsequent
            // partners of a parallel cite?
        }
        if primary == tertiary {
            tertiary = Value::Bool(false);
        }

        if js::truthy(&tertiary) {
            let slot = tertiary_slot.clone().unwrap_or_default();
            tertiary_tok.set_string("prefix", &cite_affix(state, lp, &slot, "prefix")?);
            tertiary_tok.set_string("suffix", &cite_affix(state, lp, &slot, "suffix")?);
            // Add a space if empty
            if tertiary_tok.string("prefix").is_empty() {
                tertiary_tok.set_string("prefix", " ");
            }
            // Remove quotes
            strip_secondary_decorations(&mut tertiary_tok);
            if tertiary_locale.as_deref() != Some("en")
                && tertiary_tok.string_opt("text-case").as_deref() == Some("title")
            {
                tertiary_tok.set_string("text-case", "passthrough");
            }
            if let Some(tl) = &tertiary_locale {
                state.tmp.lang_array = lang_array_with(tl, &old_lang_array);
            }
            let mut tertiary_outer = Token::new("", TokenType::Start);
            tertiary_outer
                .decorations
                .push(Decoration::new("@font-style", "normal"));
            tertiary_outer
                .decorations
                .push(Decoration::new("@font-weight", "normal"));
            queue::open_level(state, QueueId::Output, FormatRef::Token(tertiary_outer))?;
            append_value(state, &tertiary, &tertiary_tok)?;
            queue::close_level(state, QueueId::Output, None)?;
        }

        queue::close_level(state, QueueId::Output, None)?;
    } else {
        if let Some(pl) = &primary_locale {
            state.tmp.lang_array = lang_array_with(pl, &old_lang_array);
        }
        let pv = format!(
            "{}{}",
            primary_tok.string("prefix"),
            js::to_js_string(&primary)
        );
        update_group_context_condition(state, None, false, Some(&primary_tok), Some(&pv));
        append_value(state, &primary, &primary_tok)?;
        state.tmp.probably_rendered_something = true;
    }

    state.tmp.lang_array = old_lang_array;

    if state.tmp.can_block_substitute {
        name_node_push_current(state)?;
    }
    Ok(None)
}

/// `state.output.append(value, tok)` for a field value: `undefined` appends
/// nothing, a number or string is stringified by `append`.
fn append_value(state: &mut State, value: &Value, tok: &Token) -> CslResult<()> {
    let arg = match value {
        Value::Null => AppendArg::Undefined,
        other => AppendArg::Text(js::to_js_string(other)),
    };
    queue::append(
        state,
        QueueId::Output,
        arg,
        FormatRef::Token(tok.clone()),
        false,
        false,
        false,
    )?;
    Ok(())
}

#[cfg(test)]
#[path = "render_driver.rs"]
mod render_driver;

#[cfg(test)]
mod fixture_tests {
    //! The rendering nodes of wave 2 against citeproc-js, end to end, through
    //! the test-only driver of `render_driver.rs`.
    //!
    //! **Methodology.** Every fixture of the CSL test suite
    //! (`vendor/csl-test-suite/processor-tests/humans`, 845 of them) whose
    //! mode is `citation` and that has `CITATION-ITEMS` (and neither
    //! `BIBENTRIES` nor `CITATIONS`, which need the registry) is built into an
    //! engine as the test runner does and each cluster is rendered by the
    //! driver. The output is compared with what citeproc-js produced for the
    //! same fixture (`tests/data/csl/test_suite_reference.json`, *its* output,
    //! not the fixture's expected `RESULT`). A fixture that reaches a part of
    //! the port that is not there yet (names, dates, the registry) is
    //! *blocked*, not failed. **Pass criterion:** every fixture that renders
    //! equals citeproc-js's output, except those listed in `KNOWN` with the
    //! reason.
    //!
    //! **Results** are printed by the test (`--nocapture`) and recorded in the
    //! doc comment of `fixtures_render_like_citeproc_js` below.
    use std::collections::BTreeMap;

    use serde_json::Value;

    use super::render_driver::{load_fixtures, load_locales, run_fixture, Outcome};

    const REFERENCE: &str = include_str!("../../tests/data/csl/test_suite_reference.json");

    /// Fixtures where the driver's output differs from citeproc-js's, with
    /// the reason (none recorded yet).
    const KNOWN: &[(&str, &str)] = &[];

    /// Run the driver over the suite and compare with citeproc-js.
    ///
    /// **Results (2026-10-08, branch citeproc/w2-render):** see the module
    /// docs; the counts by area are printed with `--nocapture`.
    /// **Superseded 2026-10-08 (integration):** with the real registry, dateput and
    /// year-suffix code merged, this stand-in driver (no registry run, no
    /// date collapse) differs on 10 fixtures it formerly reported as blocked; the
    /// real fixture harness `tests/citeproc_test_suite.rs` covers them.
    #[test]
    #[ignore = "superseded by tests/citeproc_test_suite.rs (the stand-in driver has no registry or date-range flow)"]
    fn fixtures_render_like_citeproc_js() {
        let Some(fixtures) = load_fixtures() else {
            println!("SKIP: vendor/csl-test-suite is absent (run scripts/csl-reference.sh)");
            return;
        };
        let Some(locales) = load_locales() else {
            println!("SKIP: vendor/citeproc-js/locale is absent");
            return;
        };
        let reference: Value = serde_json::from_str(REFERENCE).expect("reference json");
        #[derive(Default)]
        struct Counts {
            ok: usize,
            bad: usize,
            blocked: usize,
            skipped: usize,
        }
        let mut by_area: BTreeMap<String, Counts> = BTreeMap::new();
        let mut blocked_why: BTreeMap<String, usize> = BTreeMap::new();
        let mut skipped_why: BTreeMap<String, usize> = BTreeMap::new();
        let mut bad: Vec<String> = Vec::new();
        let verbose = std::env::var("RENDER_DRIVER_VERBOSE").is_ok();
        for (name, fx) in &fixtures {
            let area = name.split('_').next().unwrap_or(name).to_string();
            let c = by_area.entry(area).or_default();
            match run_fixture(fx, &locales) {
                Outcome::Skipped(why) => {
                    c.skipped += 1;
                    *skipped_why.entry(why).or_insert(0) += 1;
                }
                Outcome::Blocked(why) => {
                    c.blocked += 1;
                    let key: String = why.chars().take(90).collect();
                    *blocked_why.entry(key).or_insert(0) += 1;
                    if verbose {
                        println!("BLOCKED {name}: {why}");
                    }
                }
                Outcome::Rendered(out) => {
                    let want = reference["fixtures"][name]["output"].as_str().unwrap_or("");
                    if out == want {
                        c.ok += 1;
                    } else {
                        c.bad += 1;
                        if !KNOWN.iter().any(|(n, _)| n == name) {
                            bad.push(format!("{name}:\n  want {want:?}\n  got  {out:?}"));
                        }
                    }
                }
            }
        }
        let (mut ok, mut badn, mut blocked, mut skipped) = (0, 0, 0, 0);
        println!("area            exact  differs  blocked  skipped");
        for (a, c) in &by_area {
            println!(
                "{a:<14} {:>6} {:>8} {:>8} {:>8}",
                c.ok, c.bad, c.blocked, c.skipped
            );
            ok += c.ok;
            badn += c.bad;
            blocked += c.blocked;
            skipped += c.skipped;
        }
        println!("TOTAL          {ok:>6} {badn:>8} {blocked:>8} {skipped:>8}");
        for (why, n) in &skipped_why {
            println!("skipped x{n}: {why}");
        }
        for (why, n) in &blocked_why {
            println!("blocked x{n}: {why}");
        }
        assert!(
            bad.is_empty(),
            "{} fixtures differ from citeproc-js, first:\n{}",
            bad.len(),
            bad[..bad.len().min(25)].join("\n")
        );
    }
}

#[cfg(test)]
mod render_case_tests {
    //! Randomly generated styles and items against citeproc-js.
    //!
    //! **Methodology.** `scripts/csl-units/render.cjs` (node, seeded, run by
    //! hand) generated 500 styles without names or dates (nested groups with
    //! and without rendered variables, `cs:choose` with every condition
    //! attribute and `match` mode, `cs:text` variables / terms / values /
    //! macros with quotes, strip-periods, text-case, affixes, decorations and
    //! `display`, `cs:label`, `cs:number` in every form) and 1 to 3 random
    //! items each (multilingual fields, abbreviations of titles, containers,
    //! places and numbers, locators and cite affixes), in six locales and
    //! five output formats. Each was run through citeproc-js 2.4.63 the way
    //! the test runner does (`updateItems`, `makeCitationCluster`,
    //! `makeBibliography`); the outputs are in
    //! `tests/data/csl/units/render.json`. The port renders the same through
    //! the test-only driver (`render_driver.rs`).
    //!
    //! **Pass criterion.** Every case where citeproc-js produced output
    //! equals the port's output, and a case where citeproc-js threw makes the
    //! port return an error as well (the messages are not compared).
    use serde_json::Value;

    use super::render_driver::{load_locales, run_case};

    const REF: &str = include_str!("../../tests/data/csl/units/render.json");

    /// **Results (2026-10-08, branch citeproc/w2-render):** printed by the
    /// test with `--nocapture`.
    #[test]
    fn render_cases_match_citeproc_js() {
        let Some(locales) = load_locales() else {
            println!("SKIP: vendor/citeproc-js/locale is absent");
            return;
        };
        let reference: Value = serde_json::from_str(REF).expect("render.json");
        let cases = reference["cases"].as_array().expect("cases");
        let (mut ok, mut blocked, mut bad) = (0usize, 0usize, Vec::<String>::new());
        let mut why: std::collections::BTreeMap<String, usize> = Default::default();
        for c in cases {
            let (cite, bib) = run_case(c, &locales);
            for (kind, got, want, err) in [
                ("citation", cite, &c["citation"], &c["citation_error"]),
                (
                    "bibliography",
                    bib,
                    &c["bibliography"],
                    &c["bibliography_error"],
                ),
            ] {
                if err.is_string() {
                    if got.is_ok() {
                        bad.push(format!(
                            "{} {kind}: citeproc-js threw {err}, port rendered {got:?}",
                            c["name"]
                        ));
                    } else {
                        ok += 1;
                    }
                    continue;
                }
                match got {
                    Err(e) if e.contains("not ported yet") || e.contains("DRIVER") => {
                        blocked += 1;
                        *why.entry(e.chars().take(80).collect()).or_default() += 1;
                    }
                    Err(e) => bad.push(format!("{} {kind}: port failed: {e}", c["name"])),
                    Ok(s) => {
                        if want.as_str() == Some(s.as_str()) {
                            ok += 1;
                        } else {
                            bad.push(format!(
                                "{} {kind}:\n  want {want}\n  got  {s:?}\n  csl {}",
                                c["name"], c["csl"]
                            ));
                        }
                    }
                }
            }
        }
        println!(
            "render cases: {} x2, exact {ok}, blocked {blocked}, differing {}",
            cases.len(),
            bad.len()
        );
        for (w, n) in &why {
            println!("blocked x{n}: {w}");
        }
        assert!(
            bad.is_empty(),
            "{} outputs differ from citeproc-js, first:\n{}",
            bad.len(),
            bad[..bad.len().min(6)].join("\n")
        );
    }
}
