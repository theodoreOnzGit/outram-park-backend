// Part of the kovan port of citeproc-js (GitHub #790, #792).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_locale.js
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

//! Locale loading and merging (src/util_locale.js): `CSL.localeResolve`,
//! `CSL.Engine.prototype.localeConfigure` and `localeSet`, and the
//! [`Locale`] they fill (`engine.locale[lang]`: `terms`, `opts`, `dates`,
//! `ord`, `noun-genders`).
//!
//! # How a locale is layered
//!
//! `localeConfigure(langspec)` merges, **in this order** into one
//! `engine.locale[best]`: the base-language locale file (e.g. `fr-FR` for
//! `fr-CA`), the `best` locale file, then the style's own `<locale>` elements
//! with no `xml:lang`, with the bare language, with the base and with `best`.
//! Later layers overwrite earlier terms form by form.
//!
//! # Data shapes
//!
//! `terms[name]` is a string (the three `*-range-delimiter` defaults), or an
//! object mapping a form (`long`, `short`, `verb`, ...) to a string or a
//! `[single, multiple]` pair, with gender-specific objects under
//! `masculine`/`feminine`. [`Locale`] keeps them as [`serde_json::Value`]s so
//! the intermediate dump reproduces citeproc-js's object exactly.
//!
//! # Cyclic gender data (quirk kept)
//!
//! The final loop of `localeSet` copies, for a term with a `masculine` or
//! `feminine` entry, every form of the term into the gender object, *including
//! the gender keys themselves*, so `terms[t].feminine.feminine ===
//! terms[t].feminine`: the structure is cyclic, and citeproc-js's intermediate
//! dump overflows the stack on it (`{"error": "Maximum call stack size
//! exceeded"}` for every locale section such an engine has). Lookups never
//! follow those keys, so this port omits them and sets
//! [`Locale::cyclic`] instead, which the dump turns into the same error.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::js::{self, Obj};
use super::load;
use super::state::State;
use super::xmljson::{NodeId, NodeValue, XmlJson, XmlTree};
use super::{CslResult, EngineError};

/// What `CSL.localeResolve` returns: `{base, best, bare, generic}`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LangSpec {
    /// `base`: the language's default locale (`fr` to `fr-FR`), hyphenated.
    pub base: String,
    /// `best`: the full tag, or the base when the language has no region.
    pub best: String,
    /// `bare`: the language subtag.
    pub bare: String,
    /// `generic`: the input was a bare language (`en`).
    pub generic: bool,
}

/// `CSL.localeResolve(langstr, defaultLocale)`.
///
/// `default_locale` defaults to `"en-US"`; an empty `langstr` resolves to it.
/// An unknown language returns `{base: defaultLocale, best: langstr, bare}`.
pub fn locale_resolve(langstr: &str, default_locale: Option<&str>) -> LangSpec {
    let default_locale = default_locale.filter(|d| !d.is_empty()).unwrap_or("en-US");
    let langstr = if langstr.is_empty() {
        default_locale
    } else {
        langstr
    };
    let langlst: Vec<&str> = langstr.split(['-', '_']).collect();
    let Some(base) = load::lang_base(langlst[0]) else {
        return LangSpec {
            base: default_locale.to_string(),
            best: langstr.to_string(),
            bare: langlst[0].to_string(),
            generic: false,
        };
    };
    let generic = langlst.len() == 1;
    let best = if langlst.len() == 1 || langlst[1] == "x" {
        base.replacen('_', "-", 1)
    } else {
        langlst[..2].join("-")
    };
    LangSpec {
        base: base.replacen('_', "-", 1),
        best,
        bare: langlst[0].to_string(),
        generic,
    }
}

/// One entry of `engine.locale`: `{terms, opts, dates, ord, "noun-genders"}`.
#[derive(Debug, Clone, PartialEq)]
pub struct Locale {
    /// `terms` (see the module docs for the shape).
    pub terms: Obj,
    /// `opts`: `skip-words`, `leading-noise-words`, `skip-words-regexp`
    /// (`{"$regexp": source}`, see [`regexp_value`]), `punctuation-in-quote`,
    /// `name-as-sort-order`, `court_key_classes`, ... A `jurisdiction-preference`
    /// entry may exist alone (see [`Locale::partial`]).
    pub opts: Obj,
    /// `dates`: the `<date>` templates by `form`, detached from the XML
    /// document they came from (the style's `<locale>` or a locale file).
    pub dates: BTreeMap<String, XmlTree>,
    /// `ord`: `{"1.0.1": false | {last-digit, last-two-digits, whole-number},
    /// keys: {termname: true}}`.
    pub ord: Obj,
    /// `["noun-genders"]`: term name to its grammatical gender.
    pub noun_genders: Obj,
    /// The locale was created by the `jurisdiction-preference` branch of
    /// `localeSet`, as `{opts: {...}}` with no `terms`; processing it as a
    /// target later throws in upstream.
    pub partial: bool,
    /// Whether upstream's structure for this locale is cyclic (module docs).
    pub cyclic: bool,
}

/// The JSON a `RegExp` is dumped as in the intermediate dump:
/// `{"$regexp": source}`. `source` follows `RegExp.prototype.source`
/// (a `/` is escaped).
pub fn regexp_value(source: &str) -> Value {
    json!({ "$regexp": source.replace('/', "\\/") })
}

impl Locale {
    /// A fresh `engine.locale[lang]` as `localeSet` creates it: empty terms,
    /// the default skip words, empty leading noise words.
    pub fn new() -> Locale {
        let mut opts = Obj::new();
        opts.insert(
            "skip-words".into(),
            Value::Array(
                load::SKIP_WORDS
                    .iter()
                    .map(|w| Value::String((*w).to_string()))
                    .collect(),
            ),
        );
        opts.insert("leading-noise-words".into(), Value::Array(Vec::new()));
        let mut ord = Obj::new();
        ord.insert("1.0.1".into(), Value::Bool(false));
        ord.insert("keys".into(), Value::Object(Obj::new()));
        Locale {
            terms: Obj::new(),
            opts,
            dates: BTreeMap::new(),
            ord,
            noun_genders: Obj::new(),
            partial: false,
            cyclic: false,
        }
    }

    /// `this.locale[lang] = {opts: {}}` (the `jurisdiction-preference` branch).
    fn new_partial() -> Locale {
        Locale {
            terms: Obj::new(),
            opts: Obj::new(),
            dates: BTreeMap::new(),
            ord: Obj::new(),
            noun_genders: Obj::new(),
            partial: true,
            cyclic: false,
        }
    }

    /// The locale as the JSON object citeproc-js holds (before the dump's
    /// canonicalisation, which sorts keys, as serde_json's map does). Not
    /// valid for a [`cyclic`](Locale::cyclic) locale.
    pub fn to_value(&self) -> Value {
        let mut m = Obj::new();
        if self.partial {
            m.insert("opts".into(), Value::Object(self.opts.clone()));
            return Value::Object(m);
        }
        m.insert("terms".into(), Value::Object(self.terms.clone()));
        m.insert("opts".into(), Value::Object(self.opts.clone()));
        let mut dates = Obj::new();
        for (k, v) in &self.dates {
            dates.insert(k.clone(), v.to_value());
        }
        m.insert("dates".into(), Value::Object(dates));
        m.insert("ord".into(), Value::Object(self.ord.clone()));
        m.insert(
            "noun-genders".into(),
            Value::Object(self.noun_genders.clone()),
        );
        Value::Object(m)
    }
}

impl Default for Locale {
    fn default() -> Self {
        Locale::new()
    }
}

fn type_error(what: &str) -> EngineError {
    EngineError::Csl(format!("TypeError: {what}"))
}

/// The string value a locale term node holds (`getNodeValue`), as the JS
/// string methods applied to it need it.
fn node_value_text(v: NodeValue) -> CslResult<String> {
    match v {
        NodeValue::Empty => Ok(String::new()),
        NodeValue::Text(s) => Ok(s),
        NodeValue::Node(_) => Err(type_error("target[form].indexOf is not a function")),
    }
}

/// `String.prototype.split(/\s+/)` over `s`.
fn split_ws(s: &str) -> Vec<String> {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(&format!("{}+", load::WS_CLASS)).expect("static regex")
    });
    js::split(&RE, s)
}

/// `String.prototype.split(/\s*,\s*/)` over `s`.
fn split_comma(s: &str) -> Vec<String> {
    static RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(&format!("{ws}*,{ws}*", ws = load::WS_CLASS)).expect("static regex")
    });
    js::split(&RE, s)
}

impl State {
    /// `CSL.Engine.prototype.localeConfigure(langspec, beShy)`: build
    /// `this.locale[langspec.best]` from the locale files and the style's
    /// own `<locale>` elements (see the module docs for the order), then
    /// install the range-delimiter defaults.
    pub fn locale_configure(&mut self, langspec: &LangSpec, be_shy: bool) -> CslResult<()> {
        if be_shy && self.locale.contains_key(&langspec.best) {
            return Ok(());
        }
        // `CSL.setupXml(this.sys.retrieveLocale(tag))`: a locale the sys
        // cannot supply (`false`) is an empty document, which `localeSet`
        // then finds nothing in.
        let load_locale = |sys: &super::Sys, tag: &str| -> CslResult<XmlJson> {
            match sys.retrieve_locale(tag) {
                Some(src) => super::system::setup_xml(src),
                None => Ok(super::system::setup_xml_missing()),
            }
        };
        if langspec.best == "en-US" {
            let localexml = load_locale(&self.sys, "en-US")?;
            self.locale_set(&localexml, "en-US", &langspec.best)?;
        } else {
            if langspec.base != langspec.best {
                let localexml = load_locale(&self.sys, &langspec.base)?;
                self.locale_set(&localexml, &langspec.base, &langspec.best)?;
            }
            let localexml = load_locale(&self.sys, &langspec.best)?;
            self.locale_set(&localexml, &langspec.best, &langspec.best)?;
        }
        // The style's own locale overrides, widest to narrowest.
        let cslxml = std::mem::take(&mut self.csl_xml);
        let r = (|| {
            self.locale_set(&cslxml, "", &langspec.best)?;
            self.locale_set(&cslxml, &langspec.bare, &langspec.best)?;
            if langspec.base != langspec.best {
                self.locale_set(&cslxml, &langspec.base, &langspec.best)?;
            }
            self.locale_set(&cslxml, &langspec.best, &langspec.best)
        })();
        self.csl_xml = cslxml;
        r?;
        let best = langspec.best.clone();
        {
            let loc = self.locale.get_mut(&best).ok_or_else(|| {
                type_error("Cannot read properties of undefined (reading 'terms')")
            })?;
            if !loc.terms.contains_key("page-range-delimiter") {
                let fr_pt =
                    ["fr", "pt"].contains(&js::slice(&best, 0, Some(2)).to_lowercase().as_str());
                loc.terms.insert(
                    "page-range-delimiter".into(),
                    Value::String(if fr_pt { "-" } else { "\u{2013}" }.to_string()),
                );
            }
            if !loc.terms.contains_key("year-range-delimiter") {
                loc.terms.insert(
                    "year-range-delimiter".into(),
                    Value::String("\u{2013}".to_string()),
                );
            }
            if !loc.terms.contains_key("citation-range-delimiter") {
                loc.terms.insert(
                    "citation-range-delimiter".into(),
                    Value::String("\u{2013}".to_string()),
                );
            }
        }
        let normalize = self
            .opt
            .get("development_extensions")
            .and_then(|d| d.get("normalize_lang_keys_to_lowercase"))
            .map(js::truthy)
            .unwrap_or(false);
        if normalize {
            for list in [
                "default-locale",
                "locale-sort",
                "locale-translit",
                "locale-translat",
            ] {
                if let Some(Value::Array(a)) = self.opt.get_mut(list) {
                    for v in a.iter_mut() {
                        if let Value::String(s) = v {
                            *s = s.to_lowercase();
                        }
                    }
                }
            }
            if let Some(Value::String(s)) = self.opt.get_mut("lang") {
                *s = s.to_lowercase();
            }
        }
        Ok(())
    }

    /// `CSL.Engine.prototype.localeSet(myxml, lang_in, lang_out)`: merge the
    /// `<locale xml:lang="lang_in">` of `myxml` (or `myxml`'s root, if that is
    /// itself a `<locale>`) into `this.locale[lang_out]`: gender assignments
    /// of `type` nodes into `opt.gender`, terms (with their ordinal and
    /// gender bookkeeping), `style-options`, date templates and court classes.
    pub fn locale_set(&mut self, myxml: &XmlJson, lang_in: &str, lang_out: &str) -> CslResult<()> {
        let mut lang_in = lang_in.replacen('_', "-", 1);
        let mut lang_out = lang_out.replacen('_', "-", 1);
        if self
            .opt
            .get("development_extensions")
            .and_then(|d| d.get("normalize_lang_keys_to_lowercase"))
            .map(js::truthy)
            .unwrap_or(false)
        {
            lang_in = lang_in.to_lowercase();
            lang_out = lang_out.to_lowercase();
        }
        if !self.locale.contains_key(&lang_out) {
            self.locale.insert(lang_out.clone(), Locale::new());
        }

        // Test if node is "locale"
        let mut locale: Option<NodeId> = myxml.make_xml();
        if myxml.node_name_is(myxml.data_obj, "locale") {
            locale = myxml.data_obj;
        } else {
            // Get a list of all "locale" nodes
            let nodes = myxml.get_nodes_by_name(myxml.data_obj, "locale", "");
            let mut found_locale = false;
            for blob in nodes {
                // Iterate over all locales, but for non-matching nodes,
                // we set jurisdiction_preference only (processing of the
                // chosen one will process the attribute there, separately).
                let lang_attr = myxml.get_attribute_value_ns(blob, "lang", "xml");
                if !found_locale && lang_attr.as_str() == Some(lang_in.as_str()) {
                    locale = Some(blob);
                    found_locale = true;
                } else {
                    let lang = js::to_js_string(&lang_attr);
                    let style_options = myxml.get_nodes_by_name(Some(blob), "style-options", "");
                    if !lang.is_empty() && !style_options.is_empty() {
                        let jurispref =
                            myxml.get_attribute_value(style_options[0], "jurisdiction-preference");
                        if js::truthy(&jurispref) {
                            let loc = self
                                .locale
                                .entry(lang.clone())
                                .or_insert_with(Locale::new_partial);
                            loc.opts.insert(
                                "jurisdiction-preference".into(),
                                Value::Array(
                                    split_ws(&js::to_js_string(&jurispref))
                                        .into_iter()
                                        .map(Value::String)
                                        .collect(),
                                ),
                            );
                        }
                    }
                }
            }
        }

        // Get a list of any cs:type nodes within locale
        for typenode in myxml.get_nodes_by_name(locale, "type", "") {
            let ty = myxml.get_attribute_string(typenode, "name");
            let gender = myxml.get_attribute_string(typenode, "gender");
            if let Some(Value::Object(g)) = self.opt.get_mut("gender") {
                g.insert(ty, Value::String(gender));
            }
        }

        let mut loc = self
            .locale
            .remove(&lang_out)
            .ok_or_else(|| type_error("Cannot read properties of undefined"))?;
        if loc.partial {
            // `terms` is undefined on a locale made by the
            // jurisdiction-preference branch.
            self.locale.insert(lang_out.clone(), loc);
            return Err(type_error(
                "Cannot read properties of undefined (reading 'terms')",
            ));
        }
        let r = self.locale_set_terms_opts_dates(myxml, locale, &mut loc);
        self.locale.insert(lang_out.clone(), loc);
        r?;

        // Get list of nodes by node type
        super::load::set_court_classes(self, &lang_out, myxml, locale)?;
        Ok(())
    }

    /// The body of `localeSet` between the `type` nodes and the court
    /// classes, working on the taken-out target locale `loc`.
    fn locale_set_terms_opts_dates(
        &mut self,
        myxml: &XmlJson,
        locale: Option<NodeId>,
        loc: &mut Locale,
    ) -> CslResult<()> {
        // If we are setting CSL 1.0.1 ordinals inside a style, wipe the
        // slate clean and start over.
        let has_csl_ordinals_101 = !myxml
            .get_nodes_by_name(locale, "term", "ordinal")
            .is_empty();
        if has_csl_ordinals_101 {
            if let Some(Value::Object(keys)) = loc.ord.get("keys").cloned() {
                for key in keys.keys() {
                    loc.terms.remove(key);
                }
            }
            loc.ord = Obj::new();
            loc.ord.insert("1.0.1".into(), Value::Bool(false));
            loc.ord.insert("keys".into(), Value::Object(Obj::new()));
        }

        let nodes = myxml.get_nodes_by_name(locale, "term", "");
        // Collect ordinals info as for 1.0.1, but save only if 1.0.1 toggle triggers
        let mut ordinals101 = json!({"last-digit": {}, "last-two-digits": {}, "whole-number": {}});
        let mut ordinals101_toggle = false;
        let mut genderized_terms: Vec<String> = Vec::new();
        let mut has_placeholder_term = false;
        for term in nodes {
            let mut termname = myxml.get_attribute_string(term, "name");
            if termname == "sub verbo" {
                termname = "sub-verbo".to_string();
            }
            if js::slice(&termname, 0, Some(7)) == "ordinal" {
                if termname == "ordinal" {
                    ordinals101_toggle = true;
                } else {
                    let mut match_ = myxml.get_attribute_string(term, "match");
                    let mut termstub = js::slice(&termname, 8, None);
                    let mut genderform = myxml.get_attribute_string(term, "gender-form");
                    if genderform.is_empty() {
                        genderform = "neuter".to_string();
                    }
                    if match_.is_empty() {
                        match_ = "last-two-digits".to_string();
                        if js::slice(&termstub, 0, Some(1)) == "0" {
                            match_ = "last-digit".to_string();
                        }
                    }
                    if js::slice(&termstub, 0, Some(1)) == "0" {
                        termstub = js::slice(&termstub, 1, None);
                    }
                    let bucket = ordinals101
                        .get_mut(&match_)
                        .and_then(Value::as_object_mut)
                        .ok_or_else(|| {
                            type_error(&format!(
                                "Cannot read properties of undefined (reading '{termstub}')"
                            ))
                        })?;
                    let entry = bucket
                        .entry(termstub)
                        .or_insert_with(|| Value::Object(Obj::new()));
                    if let Value::Object(e) = entry {
                        e.insert(genderform, Value::String(termname.clone()));
                    }
                }
                if let Some(Value::Object(keys)) = loc.ord.get_mut("keys") {
                    keys.insert(termname.clone(), Value::Bool(true));
                }
            }
            if !loc.terms.contains_key(&termname) {
                loc.terms
                    .insert(termname.clone(), Value::Object(Obj::new()));
            }
            let mut form = "long".to_string();
            let mut genderform = String::new();
            // Get string value of form attribute, if any
            if js::truthy(&myxml.get_attribute_value(term, "form")) {
                form = myxml.get_attribute_string(term, "form");
            }
            // Get string value of gender attribute, if any
            if js::truthy(&myxml.get_attribute_value(term, "gender-form")) {
                genderform = myxml.get_attribute_string(term, "gender-form");
            }
            // Set global gender assignment for variable associated with term name
            if js::truthy(&myxml.get_attribute_value(term, "gender")) {
                loc.noun_genders.insert(
                    termname.clone(),
                    Value::String(myxml.get_attribute_string(term, "gender")),
                );
            }
            // The value for this term/form.
            let value = if !myxml
                .get_nodes_by_name(Some(term), "multiple", "")
                .is_empty()
            {
                let single = node_value_text(myxml.get_node_value(term, Some("single")))?;
                if single.contains("%s") {
                    has_placeholder_term = true;
                }
                let multiple = node_value_text(myxml.get_node_value(term, Some("multiple")))?;
                if multiple.contains("%s") {
                    has_placeholder_term = true;
                }
                Value::Array(vec![Value::String(single), Value::String(multiple)])
            } else {
                let v = node_value_text(myxml.get_node_value(term, None))?;
                if v.contains("%s") {
                    has_placeholder_term = true;
                }
                Value::String(v)
            };
            // Work on main segment or gender-specific sub-segment as appropriate
            let Some(Value::Object(t)) = loc.terms.get_mut(&termname) else {
                return Err(type_error("terms[termname] is not an object"));
            };
            if !genderform.is_empty() {
                let mut g = Obj::new();
                g.insert(form, value);
                t.insert(genderform, Value::Object(g));
                if !genderized_terms.contains(&termname) {
                    genderized_terms.push(termname.clone());
                }
            } else {
                t.insert(form, value);
            }
        }
        if has_placeholder_term {
            self.opt
                .insert("hasPlaceholderTerm".into(), Value::Bool(true));
        }
        if !js::truthy_opt(loc.terms.get("supplement")) {
            loc.terms
                .insert("supplement".into(), Value::Object(Obj::new()));
        }
        if let Some(Value::Object(s)) = loc.terms.get_mut("supplement") {
            if !js::truthy_opt(s.get("long")) {
                s.insert("long".into(), json!(["supplement", "supplements"]));
            }
        }
        // If locale had a CSL 1.0.1-style ordinal definition, install the
        // logic object and iterate over gendered terms, filling in default
        // values for use by getTerm.
        if ordinals101_toggle {
            for ikey in &genderized_terms {
                let mut gender_segments: BTreeMap<String, Value> = BTreeMap::new();
                let mut form_segments = 0;
                let current = match loc.terms.get(ikey) {
                    Some(Value::Object(o)) => o.clone(),
                    _ => Obj::new(),
                };
                for (jkey, jval) in &current {
                    if jkey == "masculine" || jkey == "feminine" {
                        gender_segments.insert(jkey.clone(), jval.clone());
                    } else {
                        form_segments += 1;
                    }
                }
                if form_segments == 0 {
                    let source = gender_segments
                        .get("feminine")
                        .or_else(|| gender_segments.get("masculine"));
                    if let Some(Value::Object(src)) = source {
                        if let Some(Value::Object(t)) = loc.terms.get_mut(ikey) {
                            for (jkey, jval) in src {
                                t.insert(jkey.clone(), jval.clone());
                            }
                        }
                    }
                }
            }
            loc.ord.insert("1.0.1".into(), ordinals101);
        }

        // Iterate over main segments, and fill in any holes in
        // gender-specific data sub-segments. (Upstream copies the gender
        // keys too, making the structure cyclic: see the module docs.)
        let names: Vec<String> = loc.terms.keys().cloned().collect();
        for termname in names {
            let Some(Value::Object(t)) = loc.terms.get(&termname).cloned() else {
                continue;
            };
            for genderform in load::GENDERS.iter().copied() {
                if !js::truthy_opt(t.get(genderform)) {
                    continue;
                }
                loc.cyclic = true;
                let Some(Value::Object(tt)) = loc.terms.get_mut(&termname) else {
                    continue;
                };
                for (form, fval) in &t {
                    if form == "masculine" || form == "feminine" {
                        continue;
                    }
                    if let Some(Value::Object(g)) = tt.get_mut(genderform) {
                        let missing = !js::truthy_opt(g.get(form));
                        if missing {
                            g.insert(form.clone(), fval.clone());
                        }
                    }
                }
            }
        }

        // Get list of nodes by node type
        for styleopts in myxml.get_nodes_by_name(locale, "style-options", "") {
            // Get list of attributes on a node
            for (attrname, attrval) in myxml.attributes(styleopts) {
                let val = js::to_js_string(&attrval);
                let key = js::slice(&attrname, 1, None);
                match attrname.as_str() {
                    "@punctuation-in-quote" | "@limit-day-ordinals-to-day-1" => {
                        loc.opts.insert(key, Value::Bool(val == "true"));
                    }
                    "@jurisdiction-preference" => {
                        loc.opts.insert(
                            key,
                            Value::Array(split_ws(&val).into_iter().map(Value::String).collect()),
                        );
                    }
                    "@skip-words" => {
                        loc.opts.insert(
                            key,
                            Value::Array(
                                split_comma(&val).into_iter().map(Value::String).collect(),
                            ),
                        );
                    }
                    "@leading-noise-words" => {
                        loc.opts.insert(
                            "leading-noise-words".into(),
                            Value::Array(
                                split_comma(&val).into_iter().map(Value::String).collect(),
                            ),
                        );
                    }
                    "@name-as-sort-order" | "@name-as-reverse-order" | "@name-never-short" => {
                        // Fallback is okay here.
                        let mut o = Obj::new();
                        for w in split_ws(&val) {
                            o.insert(w, Value::Bool(true));
                        }
                        loc.opts.insert(key, Value::Object(o));
                    }
                    _ => {}
                }
            }
        }

        // Get list of nodes by type
        for date in myxml.get_nodes_by_name(locale, "date", "") {
            let form = myxml.get_attribute_string(date, "form");
            loc.dates.insert(form, myxml.export_tree(date));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_resolve_follows_upstream() {
        let l = locale_resolve("en", None);
        assert_eq!(
            (l.base.as_str(), l.best.as_str(), l.bare.as_str()),
            ("en-US", "en-US", "en")
        );
        assert!(l.generic);
        let l = locale_resolve("fr-CA", None);
        assert_eq!((l.base.as_str(), l.best.as_str()), ("fr-FR", "fr-CA"));
        let l = locale_resolve("xx-YY", None);
        assert_eq!(
            (l.base.as_str(), l.best.as_str(), l.bare.as_str()),
            ("en-US", "xx-YY", "xx")
        );
        let l = locale_resolve("de_x_sort", Some("fr-FR"));
        assert_eq!(l.best, "de-DE");
        assert_eq!(locale_resolve("", None).best, "en-US");
        assert_eq!(locale_resolve("", Some("pt-BR")).best, "pt-BR");
    }

    #[test]
    fn regexp_value_escapes_slashes() {
        assert_eq!(regexp_value("a/b"), json!({"$regexp": "a\\/b"}));
    }
}
