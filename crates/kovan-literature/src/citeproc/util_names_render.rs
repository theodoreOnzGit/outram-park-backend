// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_names_render.js
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

//! Port of `src/util_names_render.js` (epic #790): the input side
//! (`_normalizeNameInput`, `_parseName`, `getStaticOrder`, `_isRomanesque`;
//! section `input side (wave1-input)`) and the output side (everything that
//! renders a name or an institution to blobs; section `output side
//! (wave3-names)`, `impl NameOutput`).

// ---- input side (wave1-input) ----
//
// `CSL.NameOutput.prototype._normalizeNameInput`, `_parseName`,
// `getStaticOrder` and `_isRomanesque`, the functions that turn an input name
// into the form the name renderer consumes. The full `NameOutput` object does
// not exist yet; these methods only ever read `this.state.opt` (two flags)
// and `this.Item.language`, so they live on a small input struct
// ([`NameInputCtx`]).
//
// HOW THEY ATTACH: when `NameOutput` is ported, give it a `NameInputCtx`
// (built once per item by [`NameInputCtx::from_state_item`], since `this.Item`
// is fixed for a `NameOutput`) and make `NameOutput::normalize_name_input`,
// `parse_name`, `get_static_order` and `is_romanesque` one-line calls to the
// free functions below.

mod input_side {

    use regex::Regex;
    use serde_json::Value;

    use super::super::js::{self, Obj};
    use super::super::state::State;
    use super::super::util_name_particles::parse_particles;
    use super::super::{CslResult, EngineError};
    use super::super::load::{
        ROMANESQUE_REGEXP as ROMANESQUE_RE,
        STARTSWITH_ROMANESQUE_REGEXP as STARTSWITH_ROMANESQUE_RE,
        VIETNAMESE_NAMES as VIETNAMESE_NAMES_RE, VIETNAMESE_SPECIALS as VIETNAMESE_SPECIALS_RE,
    };

    fn rx(src: &str) -> Regex {
        Regex::new(src).unwrap_or_else(|e| panic!("invalid static regex {src:?}: {e}"))
    }

    /// What `NameOutput` reads from its `state` and `Item` in the input-side
    /// methods.
    #[derive(Debug, Clone, Default, PartialEq)]
    pub struct NameInputCtx {
        /// `state.opt.development_extensions.parse_names`.
        pub parse_names: bool,
        /// `state.opt["auto-vietnamese-names"]` (truthy).
        pub auto_vietnamese_names: bool,
        /// `this.Item.language` (truthy string), else `None`.
        pub item_language: Option<String>,
    }

    impl NameInputCtx {
        /// Read the flags from `state.opt` and `Item.language` from `item`.
        pub fn from_state_item(state: &State, item: &Value) -> NameInputCtx {
            let parse_names = state
                .opt
                .get("development_extensions")
                .and_then(|d| d.get("parse_names"))
                .map(js::truthy)
                .unwrap_or(false);
            NameInputCtx {
                parse_names,
                auto_vietnamese_names: js::truthy_opt(state.opt.get("auto-vietnamese-names")),
                item_language: item
                    .get("language")
                    .filter(|l| js::truthy(l))
                    .map(js::to_js_string),
            }
        }
    }

    /// JS `"" + (name.x)` for an optional value (`"undefined"` when absent).
    fn concat_part(v: Option<&Value>) -> String {
        match v {
            None => "undefined".to_string(),
            Some(v) => js::to_js_string(v),
        }
    }

    /// `CSL.NameOutput.prototype._normalizeNameInput(value)`: a fresh name
    /// object with the recognised fields copied from `value` (plus
    /// `comma-dropping-particle: ""`), run through [`parse_name`]. Input
    /// names must go through this exactly once; it is not idempotent.
    pub fn normalize_name_input(ctx: &NameInputCtx, value: &Value) -> CslResult<Obj> {
        let mut name = Obj::new();
        for key in [
            "literal",
            "family",
            "isInstitution",
            "given",
            "suffix",
            "comma-suffix",
            "non-dropping-particle",
            "dropping-particle",
            "static-ordering",
            "static-particles",
            "reverse-ordering",
            "full-form-always",
            "parse-names",
        ] {
            if let Some(v) = value.get(key) {
                name.insert(key.to_string(), v.clone());
            }
        }
        name.insert(
            "comma-dropping-particle".into(),
            Value::String(String::new()),
        );
        for key in ["block_initialize", "multi"] {
            if let Some(v) = value.get(key) {
                name.insert(key.to_string(), v.clone());
            }
        }
        parse_name(ctx, &mut name)?;
        Ok(name)
    }

    /// `CSL.NameOutput.prototype._parseName(name)`: unless the name opts out
    /// with a falsy `parse-names`, turn a family-only institution into a
    /// literal, honour a quoted family name (no particle parsing), and split
    /// particles and suffix when `development_extensions.parse_names` is on.
    pub fn parse_name(ctx: &NameInputCtx, name: &mut Obj) -> CslResult<()> {
        if let Some(pn) = name.get("parse-names") {
            if !js::truthy(pn) {
                return Ok(());
            }
        }
        if js::get_truthy(name, "family")
            && !js::get_truthy(name, "given")
            && js::get_truthy(name, "isInstitution")
        {
            let fam = name.get("family").cloned().unwrap_or(Value::Null);
            name.insert("literal".into(), fam);
            name.remove("family");
            name.remove("isInstitution");
        }
        let mut noparse = false;
        if js::get_truthy(name, "family") {
            let family = match name.get("family") {
                Some(Value::String(s)) => s.clone(),
                _ => {
                    return Err(EngineError::BadInput(
                        "name.family.slice is not a function".to_string(),
                    ))
                }
            };
            if js::slice(&family, 0, Some(1)) == "\"" && js::slice(&family, -1, None) == "\"" {
                name.insert(
                    "family".into(),
                    Value::String(js::slice(&family, 1, Some(-1))),
                );
                noparse = true;
                name.insert("parse-names".into(), Value::from(0));
            }
        }
        if ctx.parse_names
            && !js::get_truthy(name, "non-dropping-particle")
            && js::get_truthy(name, "family")
            && !noparse
            && js::get_truthy(name, "given")
            && !js::get_truthy(name, "static-particles")
        {
            parse_particles(name)?;
        }
        Ok(())
    }

    /// `CSL.NameOutput.prototype._isRomanesque(name)`: 0 = entirely
    /// non-romanesque, 1 = mixed content, 2 = pure romanesque (downgraded to
    /// 1 for Japanese or Chinese names).
    pub fn is_romanesque(ctx: &NameInputCtx, name: &Obj) -> CslResult<i64> {
        let family = match name.get("family") {
            Some(Value::String(s)) => s.clone(),
            None => {
                return Err(EngineError::BadInput(
                    "Cannot read properties of undefined (reading 'replace')".into(),
                ))
            }
            Some(Value::Null) => {
                return Err(EngineError::BadInput(
                    "Cannot read properties of null (reading 'replace')".into(),
                ))
            }
            Some(_) => {
                return Err(EngineError::BadInput(
                    "name.family.replace is not a function".into(),
                ))
            }
        };
        let mut ret = 2;
        if !ROMANESQUE_RE.is_match(&family.replace('"', "")) {
            ret = 0;
        }
        if ret == 0 && js::get_truthy(name, "given") {
            let given = js::to_js_string(name.get("given").unwrap_or(&Value::Null));
            if STARTSWITH_ROMANESQUE_RE.is_match(&given) {
                ret = 1;
            }
        }
        if ret == 2 {
            let main = name
                .get("multi")
                .and_then(|m| m.get("main"))
                .filter(|m| js::truthy(m))
                .map(js::to_js_string);
            let top_locale = match (&main, &ctx.item_language) {
                (Some(m), _) => Some(js::slice(m, 0, Some(2))),
                (None, Some(l)) => Some(js::slice(l, 0, Some(2))),
                _ => None,
            };
            if let Some(t) = top_locale {
                if t == "ja" || t == "zh" {
                    ret = 1;
                }
            }
        }
        Ok(ret)
    }

    /// `CSL.NameOutput.prototype.getStaticOrder(name, refresh)`: whether the
    /// name is rendered family-first regardless of the style (a set
    /// `static-ordering`, a non-romanesque family, a Vietnamese or Hungarian
    /// item or name, or a Vietnamese-looking name with
    /// `auto-vietnamese-names`).
    pub fn get_static_order(ctx: &NameInputCtx, name: &Obj, refresh: bool) -> CslResult<bool> {
        let multi_main = name
            .get("multi")
            .and_then(|m| m.get("main"))
            .filter(|m| js::truthy(m))
            .map(js::to_js_string);
        if !refresh && js::get_truthy(name, "static-ordering") {
            return Ok(true);
        }
        if is_romanesque(ctx, name)? == 0 {
            return Ok(true);
        }
        let lang_vi_hu = |s: &str| s == "vi" || s == "hu";
        if multi_main.is_none()
            && ctx
                .item_language
                .as_deref()
                .map(lang_vi_hu)
                .unwrap_or(false)
        {
            return Ok(true);
        }
        if let Some(m) = &multi_main {
            if lang_vi_hu(&js::slice(m, 0, Some(2))) {
                return Ok(true);
            }
        }
        if ctx.auto_vietnamese_names {
            let family = concat_part(name.get("family"));
            let given = concat_part(name.get("given"));
            if VIETNAMESE_NAMES_RE.is_match(&format!("{family} {given}"))
                && VIETNAMESE_SPECIALS_RE.is_match(&format!("{family}{given}"))
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[allow(unused_imports)]
pub use input_side::{get_static_order, is_romanesque, normalize_name_input, parse_name, NameInputCtx};

// ---- output side (wave3-names) ----
//
// The rest of util_names_render.js: rendering the names of one variable.

mod output_side {
    use regex::Regex;
    use serde_json::Value;
    use std::sync::LazyLock;

    use super::super::util_transform::load_and_get_abbreviation;
    use super::super::js::{self, Obj};
    use super::super::load::NAME_PARTS;
    use super::super::obj_blob::{BlobChild, BlobContent, BlobId, JS_WS_CLASS};
    use super::super::obj_token::{Decoration, Token, TokenType};
    use super::super::queue::{AppendArg, FormatRef};
    use super::super::state::State;
    use super::super::util_locale::locale_resolve;
    use super::super::util_names::{initialize_with, un_initialize};
    use super::super::util_names_output::{
        js_num, q_append, q_append_blob, q_append_str, q_close_level, q_open_level, q_pop_blob,
        q_pop_blob_required, NameOutput,
    };
    use super::super::{CslResult, EngineError};
    use super::{get_static_order, is_romanesque, normalize_name_input, NameInputCtx};

    fn type_error(msg: &str) -> EngineError {
        super::super::load::type_error(msg)
    }

    fn rx(src: &str) -> Regex {
        #[allow(clippy::expect_used)]
        Regex::new(src).expect("constant regex")
    }

    /// `/\s*\|\s*/`.
    static RE_BAR: LazyLock<Regex> =
        LazyLock::new(|| rx(&format!("[{JS_WS_CLASS}]*\\|[{JS_WS_CLASS}]*")));
    static RE_QUOTE_YEAR_SPLIT: LazyLock<Regex> = LazyLock::new(|| rx(">>[0-9]{4}>>"));
    static RE_QUOTE_YEAR: LazyLock<Regex> = LazyLock::new(|| rx(">>([0-9]{4})>>"));
    static RE_ET_AL_PARTICLE: LazyLock<Regex> =
        LazyLock::new(|| rx("^et[^\\n\\r\\x{2028}\\x{2029}]?al[^a-z]$"));
    static RE_NBSP_FEFF: LazyLock<Regex> = LazyLock::new(|| rx("[\u{a0}\u{feff}]"));
    static RE_QUASH: LazyLock<Regex> = LazyLock::new(|| {
        rx("^(?:#[0-9]+)*(?:!((?:[-_a-z]+(?:(?:.*)))(?:,(?:[-_a-z]+(?:(?:.*))))*))*>>>")
    });

    /// `split(/\s*\|\s*/)`.
    fn split_bar(s: &str) -> Vec<String> {
        js::split(&RE_BAR, s)
    }

    /// The `{long, short}` lists `fixupInstitution` returns.
    #[derive(Debug, Clone, Default, PartialEq)]
    pub struct InstitutionName {
        /// `["long"]`.
        pub long: Vec<String>,
        /// `["short"]`.
        pub short: Vec<String>,
    }

    /// What `getName` returns: `{name, usedOrig}`.
    #[derive(Debug, Clone, Default, PartialEq)]
    pub struct GotName {
        /// `name` (`false` = `None`).
        pub name: Option<Obj>,
        /// `usedOrig` (`undefined` = `None`).
        pub used_orig: Option<bool>,
    }

    /// The output slots of a name: primary, and (in the bibliography)
    /// secondary and tertiary locale sets.
    #[derive(Debug, Clone, PartialEq)]
    pub struct Slot {
        /// `primary`.
        pub primary: String,
        /// `secondary` (`false` = `None`).
        pub secondary: Option<String>,
        /// `tertiary`.
        pub tertiary: Option<String>,
    }

    /// `getNameParams`' result (and `getName`'s `name_params`). `None` is
    /// `undefined`.
    #[derive(Debug, Clone, Default, PartialEq)]
    struct NameParams {
        static_ordering: Option<bool>,
        reverse_ordering: Option<bool>,
        full_form_always: Option<bool>,
        block_initialize: Option<bool>,
        transliterated: Option<bool>,
    }

    /// `CSL.NameOutput.prototype.getNameParams(langTag)`.
    fn get_name_params(st: &State, ctx: &NameInputCtx, lang_tag: &str) -> CslResult<NameParams> {
        let mut ret = NameParams::default();
        let default_locale = st
            .opt
            .get("default-locale")
            .and_then(|d| d.get(0))
            .map(js::to_js_string)
            .unwrap_or_default();
        let langspec = locale_resolve(
            ctx.item_language.as_deref().unwrap_or(""),
            Some(&default_locale),
        );
        let try_locale = if st.locale.contains_key(&langspec.best) {
            langspec.best.clone()
        } else {
            default_locale
        };
        let locale = st
            .locale
            .get(&try_locale)
            .ok_or_else(|| type_error("Cannot read properties of undefined (reading 'opts')"))?;
        let name_as_sort_order = locale.opts.get("name-as-sort-order");
        let name_as_reverse_order = locale.opts.get("name-as-reverse-order");
        let name_never_short = locale.opts.get("name-never-short");
        let field_lang_bare = lang_tag.split('-').next().unwrap_or("");
        let has = |o: Option<&Value>| js::truthy_opt(o.and_then(|o| o.get(field_lang_bare)));
        if has(name_as_sort_order) {
            ret.static_ordering = Some(true);
            ret.reverse_ordering = Some(false);
        }
        if has(name_as_reverse_order) {
            ret.reverse_ordering = Some(true);
            ret.static_ordering = Some(false);
        }
        if has(name_never_short) {
            ret.full_form_always = Some(true);
        }
        if ret.static_ordering == Some(true) {
            ret.block_initialize = Some(true);
        }
        Ok(ret)
    }

    /// `CSL.NameOutput.prototype.getName(name, slotLocaleset, fallback,
    /// stopOrig)`: a normalised copy of `name` for one output slot (the
    /// original, or its `multi._key` form in the slot's locale), or `None`
    /// when the slot has no form and `fallback` is off. `name` is
    /// normalised in place (`family` and `given` default to `""`, and are
    /// deleted for a literal), as upstream does to the item's name.
    pub fn get_name(
        st: &State,
        ctx: &NameInputCtx,
        name: &mut Value,
        slot_localeset: &str,
        fallback: bool,
        stop_orig: Option<bool>,
    ) -> CslResult<GotName> {
        // Needs to tell us whether we used orig or not.
        if stop_orig == Some(true) && slot_localeset == "locale-orig" {
            return Ok(GotName {
                name: None,
                used_orig: stop_orig,
            });
        }
        let prim_err = |v: &Value| {
            EngineError::BadInput(format!(
                "Cannot create property 'family' on {} '{}'",
                if v.is_boolean() { "boolean" } else { "string" },
                js::to_js_string(v)
            ))
        };
        // Normalize to string
        {
            let o = name
                .as_object_mut()
                .ok_or_else(|| prim_err(&Value::Bool(false)))?;
            if !js::get_truthy(o, "family") {
                o.insert("family".into(), Value::String(String::new()));
            }
            if !js::get_truthy(o, "given") {
                o.insert("given".into(), Value::String(String::new()));
            }
        }

        // Recognized params are: block-initialize, transliterated,
        // static-ordering, full-form-always. All default to false, except for
        // static-ordering, which is initialized with a sniff.
        let mut cur: Obj = name.as_object().cloned().unwrap_or_default();
        let mut name_params = NameParams {
            static_ordering: Some(get_static_order(ctx, &cur, false)?),
            ..NameParams::default()
        };

        let mut found_tag = true;
        // The `multi._key` entry `name` was switched to (upstream mutates it
        // in place, inside the caller's name).
        let mut switched_key: Option<String> = None;
        if slot_localeset != "locale-orig" {
            found_tag = false;
            if js::truthy_opt(cur.get("multi")) {
                let lang_tags: Vec<Value> = match st.opt.get(slot_localeset) {
                    Some(Value::Array(a)) => a.clone(),
                    _ => Vec::new(),
                };
                for lang_tag in &lang_tags {
                    let key = js::to_js_string(lang_tag);
                    let sub = cur
                        .get("multi")
                        .and_then(|m| m.get("_key"))
                        .and_then(|k| k.get(&key))
                        .filter(|s| js::truthy(s))
                        .cloned();
                    if let Some(Value::Object(sub)) = sub {
                        found_tag = true;
                        let is_institution = cur.get("isInstitution").cloned();
                        let mut sub = sub;
                        match is_institution {
                            Some(v) => sub.insert("isInstitution".into(), v),
                            None => sub.remove("isInstitution"),
                        };
                        cur = sub;
                        switched_key = Some(key.clone());
                        // Set name formatting params
                        name_params = get_name_params(st, ctx, &key)?;
                        name_params.transliterated = Some(true);
                        break;
                    }
                }
            }
        }

        if !found_tag {
            let lang_tag: Option<String> = if let Some(m) = cur
                .get("multi")
                .and_then(|m| m.get("main"))
                .filter(|m| js::truthy(m))
            {
                Some(js::to_js_string(m))
            } else {
                ctx.item_language.clone()
            };
            if let Some(lt) = lang_tag {
                name_params = get_name_params(st, ctx, &lt)?;
            }
        }

        if !fallback && !found_tag {
            return Ok(GotName {
                name: None,
                used_orig: stop_orig,
            });
        }

        // Normalize to string (again)
        if !js::get_truthy(&cur, "family") {
            cur.insert("family".into(), Value::String(String::new()));
        }
        if !js::get_truthy(&cur, "given") {
            cur.insert("given".into(), Value::String(String::new()));
        }
        if js::get_truthy(&cur, "literal") {
            cur.remove("family");
            cur.remove("given");
        }
        // (the in-place mutations above hit the caller's `multi._key[tag]`)
        match &switched_key {
            Some(key) => {
                if let Some(slot) = name
                    .get_mut("multi")
                    .and_then(|m| m.get_mut("_key"))
                    .and_then(|k| k.get_mut(key.as_str()))
                {
                    *slot = Value::Object(cur.clone());
                }
            }
            None => *name = Value::Object(cur.clone()),
        }
        // var clone the item before writing into it
        let mut clone = Obj::new();
        let mut put = |k: &str, v: Option<Value>| {
            if let Some(v) = v {
                clone.insert(k.to_string(), v);
            }
        };
        put("family", cur.get("family").cloned());
        put("given", cur.get("given").cloned());
        put(
            "non-dropping-particle",
            cur.get("non-dropping-particle").cloned(),
        );
        put("dropping-particle", cur.get("dropping-particle").cloned());
        put("suffix", cur.get("suffix").cloned());
        put(
            "static-ordering",
            name_params.static_ordering.map(Value::Bool),
        );
        put("static-particles", cur.get("static-particles").cloned());
        put(
            "reverse-ordering",
            name_params.reverse_ordering.map(Value::Bool),
        );
        put(
            "full-form-always",
            name_params.full_form_always.map(Value::Bool),
        );
        put("parse-names", cur.get("parse-names").cloned());
        put("comma-suffix", cur.get("comma-suffix").cloned());
        put(
            "comma-dropping-particle",
            cur.get("comma-dropping-particle").cloned(),
        );
        put(
            "transliterated",
            name_params.transliterated.map(Value::Bool),
        );
        put(
            "block_initialize",
            name_params.block_initialize.map(Value::Bool),
        );
        put("literal", cur.get("literal").cloned());
        put("isInstitution", cur.get("isInstitution").cloned());
        put("multi", cur.get("multi").cloned());
        let mut name_obj = clone;

        if !js::get_truthy(&name_obj, "literal")
            && (!js::get_truthy(&name_obj, "given")
                && js::get_truthy(&name_obj, "family")
                && js::get_truthy(&name_obj, "isInstitution"))
        {
            let fam = name_obj.get("family").cloned().unwrap_or(Value::Null);
            name_obj.insert("literal".into(), fam);
        }
        if js::get_truthy(&name_obj, "literal") {
            name_obj.remove("family");
            name_obj.remove("given");
        }
        let normalised = normalize_name_input(ctx, &Value::Object(name_obj))?;
        let used_orig = if stop_orig == Some(true) {
            stop_orig
        } else {
            Some(!found_tag)
        };
        Ok(GotName {
            name: Some(normalised),
            used_orig,
        })
    }

    impl State {
        /// `state.nameOutput.getName(name, slotLocaleset, fallback, stopOrig)`
        /// (the registry's `evalname` calls it).
        pub fn name_output_get_name(
            &self,
            name: &mut Value,
            slot_localeset: &str,
            fallback: bool,
            stop_orig: Option<bool>,
        ) -> CslResult<GotName> {
            get_name(
                self,
                &self.name_input_ctx,
                name,
                slot_localeset,
                fallback,
                stop_orig,
            )
        }
    }

    /// `String.prototype.replace(/[\'’]/, "")`: the first apostrophe.
    fn remove_first_apostrophe(s: &str) -> String {
        match s.find(['\'', '\u{2019}']) {
            Some(i) => {
                let c = s[i..].chars().next().map(char::len_utf8).unwrap_or(1);
                format!("{}{}", &s[..i], &s[i + c..])
            }
            None => s.to_string(),
        }
    }

    /// JS `list.slice(start, end)` with numeric (possibly NaN or negative)
    /// bounds.
    fn slice_f<T: Clone>(list: &[T], start: f64, end: Option<f64>) -> Vec<T> {
        let len = list.len() as f64;
        let norm = |x: f64| {
            let x = if x.is_nan() { 0.0 } else { x.trunc() };
            if x < 0.0 {
                (len + x).max(0.0)
            } else {
                x.min(len)
            }
        };
        let a = norm(start);
        let b = end.map(norm).unwrap_or(len);
        if a < b {
            list[a as usize..b as usize].to_vec()
        } else {
            Vec::new()
        }
    }

    fn name_get(name: &Obj, key: &str) -> Option<String> {
        name.get(key)
            .filter(|v| js::truthy(v))
            .map(js::to_js_string)
    }

    /// `hasJoiningPunctuation(blob)` of `_renderOnePersonalName`.
    fn has_joining_punctuation(st: &State, blob: Option<BlobId>) -> bool {
        let Some(id) = blob else { return false };
        match &st.blobs.get(id).blobs {
            BlobContent::Text(t) => {
                let last = js::slice(t, -1, None);
                ["\u{2019}", "'", "-", " "].contains(&last.as_str())
            }
            BlobContent::List(l) => match l.last() {
                Some(BlobChild::Blob(b)) => has_joining_punctuation(st, Some(*b)),
                _ => false,
            },
        }
    }

    /// `givenInfo` of `_givenName`.
    struct GivenInfo {
        blob: Option<BlobId>,
        initialization_level: Option<i64>,
    }

    impl NameOutput {
        /// The input context of `this.Item`.
        fn input_ctx(&self, st: &State) -> NameInputCtx {
            NameInputCtx::from_state_item(st, &self.item)
        }

        /// `CSL.NameOutput.prototype.getName(...)`.
        pub fn get_name(
            &self,
            st: &State,
            name: &mut Value,
            slot_localeset: &str,
            fallback: bool,
            stop_orig: Option<bool>,
        ) -> CslResult<GotName> {
            get_name(
                st,
                &self.input_ctx(st),
                name,
                slot_localeset,
                fallback,
                stop_orig,
            )
        }

        /// `CSL.NameOutput.prototype.getNameParams` is internal to
        /// [`get_name`].
        ///
        /// `CSL.NameOutput.prototype._normalizeNameInput(value)`.
        pub fn normalize_name_input(&self, st: &State, value: &Value) -> CslResult<Obj> {
            normalize_name_input(&self.input_ctx(st), value)
        }

        /// `CSL.NameOutput.prototype._isRomanesque(name)`.
        pub fn is_romanesque(&self, st: &State, name: &Obj) -> CslResult<i64> {
            is_romanesque(&self.input_ctx(st), name)
        }

        /// `CSL.NameOutput.prototype.getStaticOrder(name, refresh)`.
        pub fn get_static_order(&self, st: &State, name: &Obj, refresh: bool) -> CslResult<bool> {
            get_static_order(&self.input_ctx(st), name, refresh)
        }

        /// The output slots for `name`: `primary`, `secondary`, `tertiary`
        /// locale sets (the three copies of this logic in
        /// util_names_render.js differ in `sort_key_flag` and in skipping
        /// falsy locale entries, hence the flags).
        fn name_slot(
            &self,
            st: &State,
            name: &Value,
            consider_sort_key: bool,
            skip_falsy_entries: bool,
        ) -> Slot {
            let is_inst =
                js::truthy_opt(name.get("isInstitution")) || js::truthy_opt(name.get("literal"));
            let prefs = st.opt.get("cite-lang-prefs");
            let localesets: Option<Vec<Value>> = if !st.tmp.extension.is_empty() {
                Some(vec![Value::String("sort".into())])
            } else {
                match prefs.and_then(|p| p.get(if is_inst { "institutions" } else { "persons" })) {
                    Some(Value::Array(a)) => Some(a.clone()),
                    _ => None,
                }
            };
            let mut slot = Slot {
                primary: "locale-orig".to_string(),
                secondary: None,
                tertiary: None,
            };
            match localesets {
                Some(ls) => {
                    for k in 0..3usize {
                        if (ls.len() as i64) - 1 < k as i64 {
                            break;
                        }
                        if skip_falsy_entries && !js::truthy(&ls[k]) {
                            continue;
                        }
                        let v = format!("locale-{}", js::to_js_string(&ls[k]));
                        match k {
                            0 => slot.primary = v,
                            1 => slot.secondary = Some(v),
                            _ => slot.tertiary = Some(v),
                        }
                    }
                }
                None => slot.primary = "locale-translat".to_string(),
            }
            let xclass_note = st.opt.get("xclass").and_then(Value::as_str) == Some("note");
            let item_no_position =
                js::truthy(&self.cite_item) && !js::truthy_opt(self.cite_item.get("position"));
            let area = st.tmp.area.as_str();
            if (consider_sort_key && st.tmp.sort_key_flag)
                || (area != "bibliography"
                    && !(area == "citation" && xclass_note && item_no_position))
            {
                slot.secondary = None;
                slot.tertiary = None;
            }
            slot
        }

        /// `CSL.NameOutput.prototype.renderAllNames()`.
        pub fn render_all_names(&mut self, st: &mut State) -> CslResult<()> {
            for (i, v) in self.variables.clone().iter().enumerate() {
                let n_free = self.freeters.get(v).map(Vec::len).unwrap_or(0);
                let n_inst = self.institutions.get(v).map(Vec::len).unwrap_or(0);
                if n_free > 0 || n_inst > 0 {
                    let has_condition = st
                        .tmp
                        .group_context
                        .tip()
                        .map(|t| t.condition.is_some())
                        .unwrap_or(false);
                    if !has_condition {
                        st.tmp.just_did_number = false;
                    }
                }

                let pos = self.nameset_base + i as i64;
                if n_free > 0 {
                    let mut values = self.freeters.get(v).cloned().unwrap_or_default();
                    let blob = self.render_names(st, v, &mut values, pos, None)?;
                    self.freeters.insert(v.clone(), values);
                    self.freeter_blobs.insert(v.clone(), blob);
                }
                let mut rendered_persons: Vec<Option<BlobId>> = Vec::new();
                for j in 0..n_inst {
                    let mut group = self
                        .persons
                        .get(v)
                        .and_then(|p| p.get(j))
                        .cloned()
                        .unwrap_or_default();
                    let blob = self.render_names(st, v, &mut group, pos, Some(j))?;
                    if let Some(slot) = self.persons.get_mut(v).and_then(|p| p.get_mut(j)) {
                        *slot = group;
                    }
                    rendered_persons.push(blob);
                }
                if n_inst > 0 || self.persons.contains_key(v) {
                    self.person_blobs.insert(v.clone(), rendered_persons);
                }
            }
            self.render_institution_names(st)
        }

        /// `CSL.NameOutput.prototype.renderInstitutionNames()`.
        pub fn render_institution_names(&mut self, st: &mut State) -> CslResult<()> {
            for v in self.variables.clone() {
                let n_inst = self.institutions.get(&v).map(Vec::len).unwrap_or(0);
                let mut blobs: Vec<Option<BlobId>> = Vec::new();
                for j in 0..n_inst {
                    let mut name = self
                        .institutions
                        .get(&v)
                        .and_then(|p| p.get(j))
                        .cloned()
                        .unwrap_or(Value::Null);
                    // XXX Start here for institutions
                    // Figure out the three segments: primary, secondary, tertiary
                    let slot = self.name_slot(st, &name, false, true);
                    // Get normalized name object for a start.
                    // true invokes fallback
                    self.set_rendered_name(st, &name)?;
                    // XXXX FROM HERE (instututions)
                    let institution =
                        self.render_institution_name(st, &v, &mut name, &slot, Some(j))?;
                    blobs.push(institution);
                }
                if n_inst > 0 {
                    self.institution_blobs.insert(v.clone(), blobs);
                }
            }
            Ok(())
        }

        /// `CSL.NameOutput.prototype._renderInstitutionName(v, name, slot,
        /// j)`.
        pub fn render_institution_name(
            &mut self,
            st: &mut State,
            v: &str,
            name: &mut Value,
            slot: &Slot,
            j: Option<usize>,
        ) -> CslResult<Option<BlobId>> {
            let res = self.get_name(st, name, &slot.primary, true, None)?;
            let mut used_orig = res.used_orig;
            let primary: Option<InstitutionName> = match res.name {
                Some(p) => Some(self.fixup_institution(st, p, v, j)?),
                None => None,
            };
            let mut secondary: Option<InstitutionName> = None;
            if let Some(sec) = &slot.secondary {
                let res = self.get_name(st, name, sec, false, used_orig)?;
                used_orig = res.used_orig;
                if let Some(s) = res.name {
                    secondary = Some(self.fixup_institution(st, s, v, j)?);
                }
            }
            let mut tertiary: Option<InstitutionName> = None;
            if let Some(ter) = &slot.tertiary {
                let res = self.get_name(st, name, ter, false, used_orig)?;
                if let Some(t) = res.name {
                    tertiary = Some(self.fixup_institution(st, t, v, j)?);
                }
            }
            // n.l / n.s: the long and short lists per slot
            let long_of = |x: &Option<InstitutionName>| x.as_ref().map(|x| x.long.clone());
            let short_of = |x: &Option<InstitutionName>| {
                x.as_ref().map(|x| {
                    if !x.short.is_empty() {
                        x.short.clone()
                    } else {
                        x.long.clone()
                    }
                })
            };
            let n_l = [long_of(&primary), long_of(&secondary), long_of(&tertiary)];
            let n_s = [
                short_of(&primary),
                short_of(&secondary),
                short_of(&tertiary),
            ];
            let parts = self
                .institution
                .as_ref()
                .and_then(|i| i.strings.get("institution-parts"))
                .and_then(Value::as_str)
                .map(str::to_string);
            let primary_ref = primary.as_ref().ok_or_else(|| {
                type_error("Cannot read properties of undefined (reading 'short')")
            })?;
            let institution: Vec<Option<BlobId>>;
            match parts.as_deref() {
                Some("short") => {
                    // No multilingual for pure short form institution names.
                    if !primary_ref.short.is_empty() {
                        let short_style = self.get_short_style();
                        institution =
                            vec![self.compose_one_institution_part(st, n_s, slot, &short_style)?];
                    } else {
                        // Fail over to long.
                        let long_style = self.get_long_style(primary_ref);
                        institution =
                            vec![self.compose_one_institution_part(st, n_l, slot, &long_style)?];
                    }
                }
                Some("short-long") => {
                    let long_style = self.get_long_style(primary_ref);
                    let short_style = self.get_short_style();
                    let institution_short =
                        self.render_one_institution_part(st, &primary_ref.short, &short_style)?;
                    // true is to include multilingual supplement
                    let institution_long =
                        self.compose_one_institution_part(st, n_l, slot, &long_style)?;
                    institution = vec![institution_short, institution_long];
                }
                Some("long-short") => {
                    let long_style = self.get_long_style(primary_ref);
                    let short_style = self.get_short_style();
                    let institution_short =
                        self.render_one_institution_part(st, &primary_ref.short, &short_style)?;
                    let institution_long =
                        self.compose_one_institution_part(st, n_l, slot, &long_style)?;
                    institution = vec![institution_long, institution_short];
                }
                _ => {
                    let long_style = self.get_long_style(primary_ref);
                    institution =
                        vec![self.compose_one_institution_part(st, n_l, slot, &long_style)?];
                }
            }
            let blob = self.join(st, institution, " ", None)?;
            if let Some(b) = blob {
                st.blobs
                    .get_mut(b)
                    .extra
                    .insert("isInstitution".into(), Value::Bool(true));
            }
            st.tmp.name_node.children.push(blob);
            Ok(blob)
        }

        /// `CSL.NameOutput.prototype._composeOneInstitutionPart(names, slot,
        /// style)`: `names` are the primary, secondary and tertiary
        /// string lists.
        pub fn compose_one_institution_part(
            &mut self,
            st: &mut State,
            names: [Option<Vec<String>>; 3],
            slot: &Slot,
            style: &Token,
        ) -> CslResult<Option<BlobId>> {
            let mut primary: Option<BlobId> = None;
            let mut secondary: Option<BlobId> = None;
            let mut tertiary: Option<BlobId> = None;
            if let Some(n0) = &names[0] {
                let mut primary_tok = style.clone_token();
                if js::truthy_opt(st.opt.get("citeAffixes").and_then(|c| c.get(&slot.primary))) {
                    let prefix = st
                        .opt
                        .get("citeAffixes")
                        .and_then(|c| c.get("institutions"))
                        .and_then(|c| c.get(&slot.primary))
                        .and_then(|c| c.get("prefix"))
                        .ok_or_else(|| {
                            type_error("Cannot read properties of undefined (reading 'prefix')")
                        })?;
                    if prefix.as_str() == Some("<i>") {
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
                }
                primary = self.render_one_institution_part(st, n0, &primary_tok)?;
            }
            if let Some(n1) = &names[1] {
                secondary = self.render_one_institution_part(st, n1, style)?;
            }
            if let Some(n2) = &names[2] {
                tertiary = self.render_one_institution_part(st, n2, style)?;
            }
            // Compose
            if secondary.is_some() || tertiary.is_some() {
                q_open_level(st, FormatRef::Name("empty".into()))?;

                q_append_blob(st, primary, FormatRef::None, false)?;

                let mut secondary_tok = style.clone_token();
                if let Some(sec) = &slot.secondary {
                    let (p, s) = citeaffix(st, "institutions", sec)?;
                    secondary_tok.set_string("prefix", &p);
                    secondary_tok.set_string("suffix", &s);
                    // Add a space if empty
                    if p.is_empty() {
                        secondary_tok.set_string("prefix", " ");
                    }
                }
                let mut secondary_outer = Token::new("", TokenType::Start);
                secondary_outer
                    .decorations
                    .push(Decoration::new("@font-style", "normal"));
                secondary_outer
                    .decorations
                    .push(Decoration::new("@font-weight", "normal"));
                q_open_level(st, FormatRef::Token(secondary_outer))?;
                q_append_blob(st, secondary, FormatRef::Token(secondary_tok), false)?;
                q_close_level(st)?;

                let mut tertiary_tok = style.clone_token();
                if let Some(ter) = &slot.tertiary {
                    let (p, s) = citeaffix(st, "institutions", ter)?;
                    tertiary_tok.set_string("prefix", &p);
                    tertiary_tok.set_string("suffix", &s);
                    // Add a space if empty
                    if p.is_empty() {
                        tertiary_tok.set_string("prefix", " ");
                    }
                }
                let mut tertiary_outer = Token::new("", TokenType::Start);
                tertiary_outer
                    .decorations
                    .push(Decoration::new("@font-style", "normal"));
                tertiary_outer
                    .decorations
                    .push(Decoration::new("@font-weight", "normal"));
                q_open_level(st, FormatRef::Token(tertiary_outer))?;
                q_append_blob(st, tertiary, FormatRef::Token(tertiary_tok), false)?;
                q_close_level(st)?;

                q_close_level(st)?;

                q_pop_blob(st)
            } else {
                Ok(primary)
            }
        }

        /// `CSL.NameOutput.prototype._renderOneInstitutionPart(blobs, style)`:
        /// render each string of the part, then join them with the part
        /// separator.
        pub fn render_one_institution_part(
            &mut self,
            st: &mut State,
            strings: &[String],
            style: &Token,
        ) -> CslResult<Option<BlobId>> {
            let mut blobs: Vec<Option<BlobId>> = Vec::with_capacity(strings.len());
            for s in strings {
                if s.is_empty() {
                    // `if (blobs[i])`: an empty string is skipped and stays as is
                    // (falsy), which _join then purges.
                    blobs.push(None);
                    continue;
                }
                let mut str_ = s.clone();
                // XXXXX Cut-and-paste code in multiple locations. This code block should be
                // collected in a function.
                // Tag: strip-periods-block
                if st.tmp.strip_periods != 0 {
                    str_ = str_.replace('.', "");
                } else {
                    for d in &style.decorations {
                        if d.name == "@strip-periods" && d.value == "true" {
                            str_ = str_.replace('.', "");
                            break;
                        }
                    }
                }
                if let Some(tip) = st.tmp.group_context.tip_mut() {
                    tip.variable_success = true;
                }
                st.tmp.can_substitute.replace_literal(Value::Bool(false))?;
                if str_ == "!here>>>" {
                    blobs.push(None);
                } else {
                    q_append(
                        st,
                        AppendArg::Text(str_),
                        FormatRef::Token(style.clone()),
                        true,
                    )?;
                    blobs.push(q_pop_blob(st)?);
                }
            }
            let sep: String = {
                let inst = self.institution.as_mut().ok_or_else(|| {
                    type_error("Cannot read properties of undefined (reading 'strings')")
                })?;
                if !inst.strings.contains_key("part-separator") {
                    inst.strings.insert(
                        "part-separator".into(),
                        Value::String(
                            st.tmp
                                .name_delimiter
                                .clone()
                                .unwrap_or_else(|| "undefined".into()),
                        ),
                    );
                }
                inst.string("part-separator")
            };
            self.join(st, blobs, &sep, None)
        }

        /// `CSL.NameOutput.prototype._renderNames(v, values, pos, j)`: render
        /// the names of one list and join them.
        pub fn render_names(
            &mut self,
            st: &mut State,
            v: &str,
            values: &mut [Value],
            pos: i64,
            j: Option<usize>,
        ) -> CslResult<Option<BlobId>> {
            let mut ret: Option<BlobId> = None;
            if !values.is_empty() {
                let mut names: Vec<Option<BlobId>> = Vec::new();
                for i in 0..values.len() {
                    // XXX We'll start here with attempts.
                    // Figure out the three segments: primary, secondary, tertiary
                    let slot = self.name_slot(st, &values[i], true, false);

                    // primary
                    // true is for fallback
                    self.set_rendered_name(st, &values[i])?;

                    let is_plain = !js::truthy_opt(values[i].get("literal"))
                        && !js::truthy_opt(values[i].get("isInstitution"));
                    if is_plain {
                        let name_blob =
                            self.render_personal_name(st, v, &mut values[i], &slot, pos, i, j)?;
                        let name_token = self.name_token()?.clone_token();
                        q_append_blob(st, name_blob, FormatRef::Token(name_token), true)?;
                        names.push(q_pop_blob(st)?);
                    } else {
                        let b = self.render_institution_name(st, v, &mut values[i], &slot, j)?;
                        names.push(b);
                    }
                }
                ret = self.join_persons(st, names, pos, j)?;
            }
            Ok(ret)
        }

        /// `CSL.NameOutput.prototype._renderPersonalName(v, name, slot, pos,
        /// i, j)`.
        #[allow(clippy::too_many_arguments)]
        pub fn render_personal_name(
            &mut self,
            st: &mut State,
            _v: &str,
            name: &mut Value,
            slot: &Slot,
            pos: i64,
            i: usize,
            j: Option<usize>,
        ) -> CslResult<Option<BlobId>> {
            let res = self.get_name(st, name, &slot.primary, true, None)?;
            let mut got = res.name.ok_or_else(|| {
                type_error("Cannot read properties of undefined (reading 'family')")
            })?;
            let primary = self.render_one_personal_name(st, &mut got, pos, i, j)?;
            let mut secondary: Option<BlobId> = None;
            let mut used = res.used_orig;
            if let Some(sec) = &slot.secondary {
                let r = self.get_name(st, name, sec, false, used)?;
                used = r.used_orig;
                if let Some(mut n) = r.name {
                    secondary = self.render_one_personal_name(st, &mut n, pos, i, j)?;
                }
            }
            let mut tertiary: Option<BlobId> = None;
            if let Some(ter) = &slot.tertiary {
                let r = self.get_name(st, name, ter, false, used)?;
                if let Some(mut n) = r.name {
                    tertiary = self.render_one_personal_name(st, &mut n, pos, i, j)?;
                }
            }
            // Now compose them to a unit
            if secondary.is_some() || tertiary.is_some() {
                q_open_level(st, FormatRef::Name("empty".into()))?;

                q_append_blob(st, primary, FormatRef::None, false)?;

                let mut secondary_tok = Token::new("", TokenType::Start);
                if let Some(sec) = &slot.secondary {
                    let (p, s) = citeaffix(st, "persons", sec)?;
                    secondary_tok.set_string("prefix", &p);
                    secondary_tok.set_string("suffix", &s);
                    // Add a space if empty
                    if p.is_empty() {
                        secondary_tok.set_string("prefix", " ");
                    }
                }
                q_append_blob(st, secondary, FormatRef::Token(secondary_tok), false)?;

                let mut tertiary_tok = Token::new("", TokenType::Start);
                if let Some(ter) = &slot.tertiary {
                    let (p, s) = citeaffix(st, "persons", ter)?;
                    tertiary_tok.set_string("prefix", &p);
                    tertiary_tok.set_string("suffix", &s);
                    // Add a space if empty
                    if p.is_empty() {
                        tertiary_tok.set_string("prefix", " ");
                    }
                }
                q_append_blob(st, tertiary, FormatRef::Token(tertiary_tok), false)?;

                q_close_level(st)?;

                q_pop_blob(st)
            } else {
                Ok(primary)
            }
        }

        /// `CSL.NameOutput.prototype._renderOnePersonalName(value, pos, i, j)`.
        pub fn render_one_personal_name(
            &mut self,
            st: &mut State,
            name: &mut Obj,
            pos: i64,
            i: usize,
            j: Option<usize>,
        ) -> CslResult<Option<BlobId>> {
            let mut dropping_particle = self.dropping_particle(st, name, pos, j)?;
            let mut family = self.family_name(st, name)?;
            let non_dropping_particle = self.non_dropping_particle(st, name)?;
            let given_info = self.given_name(st, name, pos, i)?;
            let given = given_info.blob;
            let mut suffix = self.name_suffix(st, name)?;
            if given.is_none() {
                dropping_particle = None;
                suffix = None;
            }
            let sort_sep: String = self
                .inherit_name_opt(st, "sort-separator", None, None)?
                .filter(js::truthy)
                .map(|v| js::to_js_string(&v))
                .unwrap_or_default();
            let mut sort_sep = sort_sep;
            let suffix_sep = if js::truthy_opt(name.get("comma-suffix")) {
                ", "
            } else {
                " "
            };
            let romanesque = self.is_romanesque(st, name)?;

            let has_hyphenated_non_dropping_particle =
                has_joining_punctuation(st, non_dropping_particle);

            let default_locale0 = st
                .opt
                .get("default-locale")
                .and_then(|d| d.get(0))
                .map(js::to_js_string)
                .ok_or_else(|| {
                    type_error("Cannot read properties of undefined (reading 'slice')")
                })?;
            let nbspace =
                if ["fr", "ru", "cs"].contains(&js::slice(&default_locale0, 0, Some(2)).as_str()) {
                    "\u{a0}"
                } else {
                    " "
                };
            let comma_dp = name
                .get("comma-dropping-particle")
                .map(js::to_js_string)
                .unwrap_or_else(|| "undefined".to_string());

            let blob: Option<BlobId>;
            let static_ordering = js::truthy_opt(name.get("static-ordering"));
            if romanesque == 0 {
                // XXX handle affixes for given and family
                blob = self.join(st, vec![non_dropping_particle, family, given], "", None)?;
            } else if romanesque == 1 || static_ordering {
                // entry likes sort order
                let merged = self.join(st, vec![non_dropping_particle, family], nbspace, None)?;
                blob = self.join(st, vec![merged, given], " ", None)?;
            } else if js::truthy_opt(name.get("reverse-ordering")) {
                // entry likes reverse order
                let merged = self.join(st, vec![non_dropping_particle, family], nbspace, None)?;
                blob = self.join(st, vec![given, merged], " ", None)?;
            } else if st.tmp.sort_key_flag {
                // ok with no affixes here
                let dnd = st
                    .opt
                    .get("demote-non-dropping-particle")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let sort_sep_opt = st
                    .opt
                    .get("sort_sep")
                    .map(js::to_js_string)
                    .unwrap_or_else(|| "undefined".to_string());
                if dnd.as_deref() == Some("never") {
                    let mut merged =
                        self.join(st, vec![non_dropping_particle, family], nbspace, None)?;
                    merged = self.join(st, vec![merged, dropping_particle], " ", None)?;
                    merged = self.join(st, vec![merged, given], &sort_sep_opt, None)?;
                    blob = self.join(st, vec![merged, suffix], " ", None)?;
                } else {
                    let second = self.join(
                        st,
                        vec![given, dropping_particle, non_dropping_particle],
                        " ",
                        None,
                    )?;
                    let merged = self.join(st, vec![family, second], &sort_sep_opt, None)?;
                    blob = self.join(st, vec![merged, suffix], " ", None)?;
                }
            } else {
                let nasso = self.inherit_name_opt(st, "name-as-sort-order", None, None)?;
                let nasso = nasso.as_ref().and_then(Value::as_str);
                if nasso == Some("all")
                    || (nasso == Some("first") && i == 0 && (j == Some(0) || j.is_none()))
                {
                    //
                    // Discretionary sort ordering and inversions
                    //
                    if matches!(
                        name.get("given").and_then(Value::as_str),
                        Some("Lord") | Some("Lady")
                    ) {
                        sort_sep = ", ".to_string();
                    }
                    let dnd = st
                        .opt
                        .get("demote-non-dropping-particle")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    let given_tok_affixes = self
                        .given
                        .as_ref()
                        .map(|g| (g.string("prefix"), g.string("suffix")));
                    let family_tok_affixes = self
                        .family
                        .as_ref()
                        .map(|g| (g.string("prefix"), g.string("suffix")));
                    let merged: Option<BlobId>;
                    let b: Option<BlobId>;
                    if matches!(dnd.as_deref(), Some("always") | Some("display-and-sort")) {
                        // Drop non-dropping particle
                        let second0 = self.join(
                            st,
                            vec![given, dropping_particle],
                            &format!("{comma_dp} "),
                            None,
                        )?;
                        // This would be a problem with al-Ghazali. Avoided by has_hyphenated_non_dropping_particle check above.
                        let second =
                            self.join(st, vec![second0, non_dropping_particle], " ", None)?;
                        if let (Some(sec), Some((p, s))) = (second, &given_tok_affixes) {
                            st.blobs.get_mut(sec).set_string("prefix", p);
                            st.blobs.get_mut(sec).set_string("suffix", s);
                        }
                        if let (Some(fam), Some((p, s))) = (family, &family_tok_affixes) {
                            st.blobs.get_mut(fam).set_string("prefix", p);
                            st.blobs.get_mut(fam).set_string("suffix", s);
                        }
                        merged = self.join(st, vec![family, second], &sort_sep, None)?;
                        b = self.join(st, vec![merged, suffix], &sort_sep, None)?;
                    } else {
                        // Don't drop particle.
                        let first = if has_hyphenated_non_dropping_particle {
                            self.join(st, vec![non_dropping_particle, family], "", None)?
                        } else {
                            self.join(st, vec![non_dropping_particle, family], nbspace, None)?
                        };
                        if let (Some(f), Some((p, s))) = (first, &family_tok_affixes) {
                            st.blobs.get_mut(f).set_string("prefix", p);
                            st.blobs.get_mut(f).set_string("suffix", s);
                        }

                        let second = self.join(
                            st,
                            vec![given, dropping_particle],
                            &format!("{comma_dp} "),
                            None,
                        )?;
                        if let (Some(sec), Some((p, s))) = (second, &given_tok_affixes) {
                            st.blobs.get_mut(sec).set_string("prefix", p);
                            st.blobs.get_mut(sec).set_string("suffix", s);
                        }

                        merged = self.join(st, vec![first, second], &sort_sep, None)?;
                        b = self.join(st, vec![merged, suffix], &sort_sep, None)?;
                    }
                    match b {
                        Some(id) => {
                            st.blobs
                                .get_mut(id)
                                .extra
                                .insert("isInverted".into(), Value::Bool(true));
                        }
                        None => {
                            return Err(EngineError::BadInput(
                                "Cannot create property 'isInverted' on boolean 'false'".into(),
                            ))
                        }
                    }
                    blob = b;
                } else {
                    // plain vanilla
                    if js::truthy_opt(name.get("dropping-particle"))
                        && js::truthy_opt(name.get("family"))
                        && !js::truthy_opt(name.get("non-dropping-particle"))
                    {
                        let dp =
                            js::to_js_string(name.get("dropping-particle").unwrap_or(&Value::Null));
                        let last = js::slice(&dp, -1, None);
                        if ["'", "\u{2bc}", "\u{2019}", "-"].contains(&last.as_str())
                            && js::slice(&dp, 0, Some(-1)) != "de"
                        {
                            family = self.join(st, vec![dropping_particle, family], "", None)?;
                            dropping_particle = None;
                        }
                    }

                    let mut second;
                    if has_hyphenated_non_dropping_particle {
                        second = self.join(st, vec![non_dropping_particle, family], "", None)?;
                        second = self.join(st, vec![dropping_particle, second], nbspace, None)?;
                    } else {
                        second = self.join(
                            st,
                            vec![dropping_particle, non_dropping_particle, family],
                            nbspace,
                            None,
                        )?;
                    }
                    second = self.join(st, vec![second, suffix], suffix_sep, None)?;
                    if let (Some(sec), Some(fam)) = (second, &self.family) {
                        let (p, s) = (fam.string("prefix"), fam.string("suffix"));
                        st.blobs.get_mut(sec).set_string("prefix", &p);
                        st.blobs.get_mut(sec).set_string("suffix", &s);
                    }
                    if let (Some(g), Some(gt)) = (given, &self.given) {
                        let (p, s) = (gt.string("prefix"), gt.string("suffix"));
                        st.blobs.get_mut(g).set_string("prefix", &p);
                        st.blobs.get_mut(g).set_string("suffix", &s);
                    }
                    let Some(second_id) = second else {
                        return Err(type_error(
                            "Cannot read properties of undefined (reading 'prefix')",
                        ));
                    };
                    if !st.blobs.get(second_id).string("prefix").is_empty() {
                        name.insert(
                            "comma-dropping-particle".into(),
                            Value::String(String::new()),
                        );
                    }
                    let comma_dp_now = name
                        .get("comma-dropping-particle")
                        .map(js::to_js_string)
                        .unwrap_or_else(|| "undefined".to_string());

                    let iw = self.inherit_name_opt(st, "initialize-with", None, None)?;
                    let space = if iw.as_ref().map(js::truthy).unwrap_or(false)
                        && RE_NBSP_FEFF
                            .is_match(&iw.as_ref().map(js::to_js_string).unwrap_or_default())
                        && given_info.initialization_level == Some(1)
                    {
                        nbspace
                    } else {
                        " "
                    };
                    blob = self.join(
                        st,
                        vec![given, second],
                        &format!("{comma_dp_now}{space}"),
                        None,
                    )?;
                }
            }
            // XXX Just generally assume for the present that personal names render something
            if let Some(tip) = st.tmp.group_context.tip_mut() {
                tip.variable_success = true;
            }
            st.tmp.can_substitute.replace_literal(Value::Bool(false))?;
            st.tmp.term_predecessor = true;
            st.tmp.name_node.children.push(blob);
            Ok(blob)
        }

        /// `CSL.NameOutput.prototype._stripPeriods(tokname, str)`.
        pub fn strip_periods(&self, st: &State, tokname: &str, s: Option<&str>) -> Option<String> {
            let decor_tok = if tokname == "family" {
                self.family_decor.as_ref()
            } else {
                self.given_decor.as_ref()
            };
            let mut out = s.map(str::to_string);
            if let Some(str_) = out.as_mut() {
                if !str_.is_empty() {
                    if st.tmp.strip_periods != 0 {
                        *str_ = str_.replace('.', "");
                    } else if let Some(dt) = decor_tok {
                        for d in &dt.decorations {
                            if d.name == "@strip-periods" && d.value == "true" {
                                *str_ = str_.replace('.', "");
                                break;
                            }
                        }
                    }
                }
            }
            out
        }

        fn decor_ref(&self, tokname: &str) -> FormatRef {
            let t = if tokname == "family" {
                self.family_decor.as_ref()
            } else {
                self.given_decor.as_ref()
            };
            match t {
                Some(t) => FormatRef::Token(t.clone()),
                None => FormatRef::None,
            }
        }

        /// `CSL.NameOutput.prototype._nonDroppingParticle(name)`.
        pub fn non_dropping_particle(
            &mut self,
            st: &mut State,
            name: &Obj,
        ) -> CslResult<Option<BlobId>> {
            let mut ndp = name_get(name, "non-dropping-particle");
            if st.tmp.sort_key_flag {
                if let Some(n) = ndp.as_mut() {
                    *n = remove_first_apostrophe(n);
                }
            }
            let s = self.strip_periods(st, "family", ndp.as_deref());
            let fr = self.decor_ref("family");
            if q_append_str(st, s.as_deref(), fr, true)? {
                return q_pop_blob(st);
            }
            Ok(None)
        }

        /// `CSL.NameOutput.prototype._droppingParticle(name, pos, j)`.
        pub fn dropping_particle(
            &mut self,
            st: &mut State,
            name: &mut Obj,
            pos: i64,
            j: Option<usize>,
        ) -> CslResult<Option<BlobId>> {
            let mut dp = name_get(name, "dropping-particle");
            if st.tmp.sort_key_flag {
                if let Some(d) = dp.as_mut() {
                    *d = remove_first_apostrophe(d);
                }
            }
            let s = self.strip_periods(st, "given", dp.as_deref());
            let orig_dp = name_get(name, "dropping-particle");
            if orig_dp
                .as_deref()
                .map(|d| RE_ET_AL_PARTICLE.is_match(d))
                .unwrap_or(false)
            {
                let use_last = self
                    .inherit_name_opt(st, "et-al-use-last", None, None)?
                    .map(|v| js::truthy(&v))
                    .unwrap_or(false);
                let val = if use_last { 2 } else { 1 };
                let key = pos.to_string();
                let Some(spec) = self.etal_spec_mut(&key) else {
                    return Err(type_error(
                        "Cannot set properties of undefined (setting 'freeters')",
                    ));
                };
                if j.is_none() {
                    spec.freeters = val;
                } else {
                    // `etal_spec[pos].persons = val` replaces the array by a
                    // number: later `persons[j]` reads are `undefined`.
                    spec.persons = Vec::new();
                }
                name.insert(
                    "comma-dropping-particle".into(),
                    Value::String(String::new()),
                );
            } else {
                let gr = self.decor_ref("given");
                if q_append_str(st, s.as_deref(), gr, true)? {
                    return q_pop_blob(st);
                }
            }
            Ok(None)
        }

        /// `CSL.NameOutput.prototype._familyName(name)`.
        pub fn family_name(&mut self, st: &mut State, name: &Obj) -> CslResult<Option<BlobId>> {
            let fam = name
                .get("family")
                .filter(|v| !v.is_null())
                .map(js::to_js_string);
            let s = self.strip_periods(st, "family", fam.as_deref());
            let fr = self.decor_ref("family");
            if q_append_str(st, s.as_deref(), fr, true)? {
                return q_pop_blob(st);
            }
            Ok(None)
        }

        /// `CSL.NameOutput.prototype._givenName(name, pos, i)`.
        fn given_name(
            &mut self,
            st: &mut State,
            name: &mut Obj,
            pos: i64,
            i: usize,
        ) -> CslResult<GivenInfo> {
            // citation
            //   use disambig as-is
            // biblography
            //   use disambig only if it boosts over the default
            //   SO WHAT IS THE DEFAULT?
            //   A: If "form" is short, it's 0.
            //      If "form" is long, initialize-with exists (and initialize is not false) it's 1
            //      If "form" is long, and initialize_with does not exist, it's 2.
            let form = self.form_opt(st)?;
            let form_is_short = form != "long";
            let initialize_is_turned_on =
                self.inherit_name_opt(st, "initialize", None, None)? != Some(Value::Bool(false));
            let iw = self.inherit_name_opt(st, "initialize-with", None, None)?;
            let mut has_initialize_with = matches!(iw, Some(Value::String(_)))
                && !js::truthy_opt(name.get("block_initialize"));
            let use_level: i64;
            if js::truthy_opt(name.get("full-form-always")) {
                use_level = 2;
            } else {
                let default_level = if form_is_short {
                    0
                } else if has_initialize_with {
                    1
                } else {
                    2
                };
                if !st.disambig_settings_mut().has_givens(pos) {
                    return Err(type_error(
                        "Cannot read properties of undefined (reading '0')",
                    ));
                }
                let requested = st.disambig_settings_mut().given(pos, i);
                use_level = match requested {
                    Some(r) if r > default_level => r,
                    _ => default_level,
                };
            }
            let gdropt = st
                .citation
                .opt
                .get("givenname-disambiguation-rule")
                .filter(|v| js::truthy(v))
                .map(js::to_js_string);
            if let Some(g) = gdropt {
                if js::slice(&g, -14, None) == "-with-initials" {
                    has_initialize_with = true;
                }
            }
            if js::truthy_opt(name.get("family")) && use_level == 1 {
                let given = name_get_or_empty(name, "given");
                if has_initialize_with {
                    let initialize_with_s = self
                        .inherit_name_opt(
                            st,
                            "initialize-with",
                            None,
                            Some(Value::String(String::new())),
                        )?
                        .map(|v| js::to_js_string(&v))
                        .unwrap_or_default();
                    let new =
                        initialize_with(st, &given, &initialize_with_s, !initialize_is_turned_on);
                    name.insert("given".into(), Value::String(new));
                } else {
                    let new = un_initialize(st, &given);
                    name.insert("given".into(), Value::String(new));
                }
            } else if use_level == 0 {
                return Ok(GivenInfo {
                    blob: None,
                    initialization_level: None,
                });
            } else if use_level == 2 {
                let given = name_get_or_empty(name, "given");
                let new = un_initialize(st, &given);
                name.insert("given".into(), Value::String(new));
            }

            let g = name
                .get("given")
                .filter(|v| !v.is_null())
                .map(js::to_js_string);
            let s = self.strip_periods(st, "given", g.as_deref());
            let gr = self.decor_ref("given");
            let rendered = q_append_str(st, s.as_deref(), gr, true)?;
            if rendered {
                let ret = q_pop_blob(st)?;
                return Ok(GivenInfo {
                    blob: ret,
                    initialization_level: Some(use_level),
                });
            }
            Ok(GivenInfo {
                blob: None,
                initialization_level: None,
            })
        }

        /// `CSL.NameOutput.prototype._nameSuffix(name)`.
        pub fn name_suffix(&mut self, st: &mut State, name: &Obj) -> CslResult<Option<BlobId>> {
            let mut s: Option<String> = name
                .get("suffix")
                .filter(|v| !v.is_null())
                .map(js::to_js_string);
            let iw = self.inherit_name_opt(st, "initialize-with", None, None)?;
            if s.as_deref().map(|x| !x.is_empty()).unwrap_or(false) {
                if let Some(Value::String(iw_s)) = &iw {
                    let new = initialize_with(st, s.as_deref().unwrap_or(""), iw_s, true);
                    s = Some(new);
                }
            }
            let mut s = self.strip_periods(st, "family", s.as_deref());
            let mut to_suffix = String::new();
            if let Some(x) = s.as_mut() {
                if js::slice(x, -1, None) == "." {
                    *x = js::slice(x, 0, Some(-1));
                    to_suffix = ".".to_string();
                }
            }
            let rendered = q_append_str(st, s.as_deref(), FormatRef::Name("empty".into()), true)?;
            if rendered {
                let ret = q_pop_blob_required(st)?;
                let cur = st.blobs.get(ret).string("suffix");
                st.blobs
                    .get_mut(ret)
                    .set_string("suffix", &format!("{to_suffix}{cur}"));
                return Ok(Some(ret));
            }
            Ok(None)
        }

        /// `CSL.NameOutput.prototype._getLongStyle(name)`.
        pub fn get_long_style(&self, name: &InstitutionName) -> Token {
            let long_style = if !name.short.is_empty() {
                if let Some(t) = &self.institutionpart.long_with_short {
                    Some(t.clone())
                } else {
                    self.institutionpart.long.clone()
                }
            } else {
                self.institutionpart.long.clone()
            };
            long_style.unwrap_or_else(|| Token::new("", TokenType::Start))
        }

        /// `CSL.NameOutput.prototype._getShortStyle()`.
        pub fn get_short_style(&self) -> Token {
            self.institutionpart
                .short
                .clone()
                .unwrap_or_else(|| Token::new("", TokenType::Start))
        }

        /// `CSL.NameOutput.prototype.setRenderedName(name)`: remember the
        /// string form of the name (bibliography only; used by
        /// `subsequent-author-substitute`).
        pub fn set_rendered_name(&self, st: &mut State, name: &Value) -> CslResult<()> {
            if st.tmp.area == "bibliography" {
                let mut strname = String::new();
                for part in NAME_PARTS {
                    if let Some(v) = name.get(*part).filter(|v| js::truthy(v)) {
                        strname.push_str(&js::to_js_string(v));
                    }
                }
                match st.tmp.rendered_name.as_mut() {
                    Some(r) => r.push(Value::String(strname)),
                    None => {
                        return Err(type_error(
                            "Cannot read properties of undefined (reading 'push')",
                        ))
                    }
                }
            }
            Ok(())
        }

        /// `CSL.NameOutput.prototype.fixupInstitution(name, varname,
        /// listpos)`: the long and short forms of an institution name, as
        /// lists of the `|`-separated parts, with abbreviations applied.
        pub fn fixup_institution(
            &self,
            st: &mut State,
            mut name: Obj,
            varname: &str,
            _listpos: Option<usize>,
        ) -> CslResult<InstitutionName> {
            if !js::get_truthy(&name, "literal") && js::get_truthy(&name, "family") {
                let fam = name.get("family").cloned().unwrap_or(Value::Null);
                name.insert("literal".into(), fam);
                name.remove("family");
            }
            let mut long_name_str: String = match name.get("literal") {
                Some(Value::String(s)) => s.clone(),
                _ => {
                    return Err(type_error(
                        "Cannot read properties of undefined (reading 'split')",
                    ))
                }
            };
            let mut short_name_str = long_name_str.clone();
            let mut ret = InstitutionName {
                long: split_bar(&long_name_str),
                short: split_bar(&short_name_str),
            };
            let inst = self.institution.as_ref().ok_or_else(|| {
                type_error("Cannot read properties of undefined (reading 'strings')")
            })?;
            let parts: Option<String> = inst.string_opt("institution-parts");
            let parts_short_family = matches!(
                parts.as_deref(),
                Some("short") | Some("short-long") | Some("long-short")
            );
            let jurisdiction: Option<String> = self
                .item
                .get("jurisdiction")
                .filter(|j| js::truthy(j))
                .map(js::to_js_string);
            let item_lang: Option<String> = self
                .item
                .get("language")
                .filter(|l| js::truthy(l))
                .map(js::to_js_string);
            // state.sys.getAbbreviation exists (the host's sys).
            // Normalize longNameStr and shortNameStr
            if inst.string_opt("form").as_deref() == Some("short") {
                let mut jur = jurisdiction.clone();
                if let Some(a) = load_and_get_abbreviation(
                    st,
                    &mut jur,
                    "institution-entire",
                    &long_name_str,
                    item_lang.as_deref(),
                ) {
                    long_name_str = a;
                } else {
                    jur = jurisdiction.clone();
                    if let Some(a) = load_and_get_abbreviation(
                        st,
                        &mut jur,
                        "institution-part",
                        &long_name_str,
                        item_lang.as_deref(),
                    ) {
                        long_name_str = a;
                    }
                }
                long_name_str = self.quash_checks(st, jur.as_deref(), &long_name_str)?;
            }
            if parts_short_family {
                let mut jur = jurisdiction.clone();
                if let Some(a) = load_and_get_abbreviation(
                    st,
                    &mut jur,
                    "institution-part",
                    &short_name_str,
                    item_lang.as_deref(),
                ) {
                    short_name_str = a;
                }
                short_name_str = self.quash_checks(st, jur.as_deref(), &short_name_str)?;
                if matches!(parts.as_deref(), Some("short-long") | Some("long-short"))
                    && short_name_str == long_name_str
                {
                    short_name_str = String::new();
                }
            }
            // Split abbreviated strings
            // For pure long, split and we're done.
            ret.long = split_bar(&long_name_str);
            // For short, split and then try abbrev with institution-part on each element
            ret.short = split_bar(&short_name_str);
            if parts_short_family {
                let mut j = ret.short.len() as i64 - 1;
                while j > -1 {
                    let ju = j as usize;
                    let abbrev_key = ret.short[ju].clone();
                    let mut jur = jurisdiction.clone();
                    if let Some(a) = load_and_get_abbreviation(
                        st,
                        &mut jur,
                        "institution-part",
                        &abbrev_key,
                        item_lang.as_deref(),
                    ) {
                        ret.short[ju] = a;
                    }
                    if ret.short[ju].contains('|') {
                        let split_short = split_bar(&ret.short[ju].clone());
                        let mut next: Vec<String> = ret.short[..ju].to_vec();
                        next.extend(split_short);
                        next.extend(ret.short[ju + 1..].iter().cloned());
                        ret.short = next;
                    }
                    j -= 1;
                }
            }
            let legacy = js::truthy_opt(
                st.opt
                    .get("development_extensions")
                    .and_then(|d| d.get("legacy_institution_name_ordering")),
            );
            if legacy {
                ret.short.reverse();
            }
            ret.short = self.trim_institution(&ret.short);
            if js::truthy_opt(inst.strings.get("reverse-order")) {
                ret.short.reverse();
            }
            // trimmer is not available in getAmbiguousCite
            if !st.tmp.just_looking {
                if let Some(j) = &jurisdiction {
                    let pat = st
                        .tmp
                        .abbrev_trimmer
                        .as_ref()
                        .and_then(|t| t.fields.get(j))
                        .and_then(|f| f.get(varname))
                        .cloned();
                    if let Some(pat) = pat {
                        for frag in ret.short.iter_mut() {
                            *frag = js::trim(&frag.replacen(pat.as_str(), "", 1)).to_string();
                        }
                    }
                }
            }
            if legacy {
                ret.long.reverse();
            }
            ret.long = self.trim_institution(&ret.long);
            if js::truthy_opt(inst.strings.get("reverse-order")) {
                ret.long.reverse();
            }
            Ok(ret)
        }

        /// `CSL.NameOutput.prototype._quashChecks(jurisdiction, str)`.
        pub fn quash_checks(
            &self,
            st: &mut State,
            _jurisdiction: Option<&str>,
            s: &str,
        ) -> CslResult<String> {
            let s = super::super::util_transform::quash_check(st, _jurisdiction, s)?;
            // If the abbreviation has date cut-offs, find the most recent
            // abbreviation within scope.
            let mut lst: Vec<String> = js::split(&RE_QUOTE_YEAR_SPLIT, &s);
            let m: Option<String> = RE_QUOTE_YEAR
                .captures(&s)
                .and_then(|c| c.get(1))
                .map(|x| x.as_str().to_string());
            let mut out = lst.pop().unwrap_or_default();
            let date_v = self
                .item
                .get("original-date")
                .filter(|d| js::truthy(d))
                .or_else(|| self.item.get("issued"))
                .filter(|d| js::truthy(d));
            let date: Option<i64> = date_v
                .and_then(|d| d.get("year"))
                .and_then(js::parse_int_value);
            if let Some(date) = date.filter(|d| *d != 0) {
                if !lst.is_empty() {
                    // for (k = m.length-1; k > 0; k--): m is [full, group 1]
                    if let Some(m1) = &m {
                        if date < js::parse_int(m1).unwrap_or(i64::MAX) {
                            out = lst.pop().unwrap_or_default();
                        }
                    }
                }
                out = RE_BAR.replace_all(&out, "|").into_owned();
            }
            Ok(out)
        }

        /// `CSL.NameOutput.prototype._trimInstitution(subunits)`: apply the
        /// `use-first`, `stop-last`, `use-last` and `stop-first` of
        /// `cs:institution` to the parts of an institution name.
        pub fn trim_institution(&self, subunits: &[String]) -> Vec<String> {
            let mut s: Vec<String> = subunits.to_vec();
            let Some(inst) = &self.institution else {
                return subunits.to_vec();
            };
            let get = |k: &str| inst.strings.get(k).cloned();
            let truthy_num = |v: &Option<Value>| v.as_ref().map(js::truthy).unwrap_or(false);
            let use_first = get("use-first");
            let use_last = get("use-last");
            let mut stop_first = get("stop-first");
            let stop_last = get("stop-last");
            // If use_first, apply stop_last, then apply use_first;
            // If use_last, apply stop_first, then apply use_last;
            if truthy_num(&use_first) {
                if truthy_num(&stop_last) {
                    s = slice_f(&s, 0.0, Some(js_num(stop_last.as_ref()) * -1.0));
                }
                s = slice_f(&s, 0.0, Some(js_num(use_first.as_ref())));
            }
            if truthy_num(&use_last) {
                let mut ss = subunits.to_vec();
                if truthy_num(&use_first) {
                    stop_first = use_first.clone();
                } else {
                    s = Vec::new();
                }
                if truthy_num(&stop_first) {
                    ss = slice_f(&ss, js_num(stop_first.as_ref()), None);
                }
                ss = slice_f(&ss, js_num(use_last.as_ref()) * -1.0, None);
                s.extend(ss);
            }
            s
        }
    }

    /// `name[key]` as a string, `""` when falsy (the `name.given` the code
    /// has just normalised).
    fn name_get_or_empty(name: &Obj, key: &str) -> String {
        name.get(key)
            .filter(|v| !v.is_null())
            .map(js::to_js_string)
            .unwrap_or_default()
    }

    /// `state.opt.citeAffixes[kind][locale]` as (prefix, suffix); a TypeError
    /// for a locale set the table does not have.
    fn citeaffix(st: &State, kind: &str, locale: &str) -> CslResult<(String, String)> {
        let e = st
            .opt
            .get("citeAffixes")
            .and_then(|c| c.get(kind))
            .and_then(|c| c.get(locale))
            .ok_or_else(|| type_error("Cannot read properties of undefined (reading 'prefix')"))?;
        let get = |k: &str| e.get(k).map(js::to_js_string).unwrap_or_default();
        Ok((get("prefix"), get("suffix")))
    }

}

#[allow(unused_imports)]
pub use output_side::{get_name, GotName, InstitutionName, Slot};

#[cfg(test)]
mod input_side_tests {
    //! Differential tests against citeproc-js 2.4.63 for the input side:
    //! reference `tests/data/csl/units/names.json` (generator
    //! `scripts/csl-units/names.cjs`). For ~3,000 names (every name in the
    //! fixtures plus generated ones) and 6 settings of `parse_names`,
    //! `auto-vietnamese-names`, `Item.language` and `refresh`:
    //! `_normalizeNameInput`, `getStaticOrder` (also on the raw name with no
    //! defaulting of `family`/`given`) and `_isRomanesque`. Pass criterion:
    //! equal results or equal TypeError text.
    use serde_json::Value;

    use super::*;

    const REF: &str = include_str!("../../tests/data/csl/units/names.json");

    fn err_text(e: &super::super::EngineError) -> String {
        match e {
            super::super::EngineError::BadInput(m) => m.clone(),
            o => o.to_string(),
        }
    }

    #[test]
    fn name_input_matches_citeproc_js() {
        let r: Value = serde_json::from_str(REF).expect("json");
        let variants = r["variants"].as_array().expect("variants");
        let mut n = 0;
        for c in r["cases"].as_array().expect("cases") {
            let name = &c["name"];
            for (vi, v) in variants.iter().enumerate() {
                let ctx = NameInputCtx {
                    parse_names: v["parse_names"].as_bool().unwrap_or(false),
                    auto_vietnamese_names: v["vn"].as_bool().unwrap_or(false),
                    item_language: v["lang"].as_str().map(str::to_string),
                };
                let refresh = v["refresh"].as_bool().unwrap_or(false);
                let want = &c["v"][vi];
                let Value::Object(nobj) = name else { continue };
                // getStaticOrder on a defaulted copy
                let mut for_static = nobj.clone();
                for k in ["family", "given"] {
                    if !crate::citeproc::js::truthy_opt(for_static.get(k)) {
                        for_static.insert(k.into(), Value::String(String::new()));
                    }
                }
                match (
                    get_static_order(&ctx, &for_static, refresh),
                    want.get("static_error"),
                ) {
                    (Ok(b), None) => {
                        assert_eq!(Value::Bool(b), want["static_ordering"], "static {name} {v}")
                    }
                    (Err(e), Some(w)) => {
                        assert_eq!(w.as_str(), Some(err_text(&e).as_str()), "static {name}")
                    }
                    (g, w) => panic!("static {name} [{}]: {g:?} vs {w:?}", v["name"]),
                }
                match (
                    get_static_order(&ctx, nobj, refresh),
                    want.get("static_raw_error"),
                ) {
                    (Ok(b), None) => {
                        assert_eq!(Value::Bool(b), want["static_raw"], "static_raw {name} {v}")
                    }
                    (Err(e), Some(w)) => {
                        assert_eq!(w.as_str(), Some(err_text(&e).as_str()), "static_raw {name}")
                    }
                    (g, w) => panic!("static_raw {name} [{}]: {g:?} vs {w:?}", v["name"]),
                }
                match (normalize_name_input(&ctx, name), want.get("norm_error")) {
                    (Ok(o), None) => {
                        let mut got = Value::Object(o);
                        crate::citeproc::build_retrieve_item::canon_numbers(&mut got);
                        assert_eq!(got, want["norm"], "norm {name} [{}]", v["name"])
                    }
                    (Err(e), Some(w)) => {
                        assert_eq!(w.as_str(), Some(err_text(&e).as_str()), "norm {name}")
                    }
                    (g, w) => panic!("norm {name} [{}]: {g:?} vs {w:?}", v["name"]),
                }
                match (is_romanesque(&ctx, nobj), want.get("romanesque_error")) {
                    (Ok(b), None) => {
                        assert_eq!(Value::from(b), want["romanesque"], "romanesque {name}")
                    }
                    (Err(e), Some(w)) => {
                        assert_eq!(w.as_str(), Some(err_text(&e).as_str()), "romanesque {name}")
                    }
                    (g, w) => panic!("romanesque {name}: {g:?} vs {w:?}"),
                }
                n += 1;
            }
        }
        assert!(n > 15000, "{n}");
    }
}

#[cfg(test)]
mod output_side_tests {
    //! Differential tests against citeproc-js 2.4.63 for the output side of
    //! util_names_render.js that can run without the output queue:
    //! reference `tests/data/csl/units/names_output.json`
    //! (generator `scripts/csl-units/names_output.cjs`).
    //!
    //! * `getName`: 265 names (a sample of the fixtures' plus generated ones with
    //!   `multi` forms, particles, institutions, literals) × 4 settings
    //!   (item language, `locale-translit` / `-translat` lists, parse-names,
    //!   Vietnamese detection; locale options `name-as-sort-order`,
    //!   `name-as-reverse-order`, `name-never-short`) × 4 slots × `fallback` ×
    //!   `stopOrig`: the returned name, `usedOrig`, errors, and the state of the
    //!   caller's name afterwards (upstream normalises it in place);
    //! * `fixupInstitution`: 15 institution names × 18 `cs:institution`
    //!   settings (`form`, `institution-parts`, `use-first`, `use-last`,
    //!   `stop-first`, `stop-last`, `reverse-order`) × 5 items (jurisdiction,
    //!   dates) × `legacy_institution_name_ordering` × `just_looking`, against
    //!   an abbreviation table with entire-name and part abbreviations,
    //!   `>>2001>>` date cut-offs and `#1!field>>>` quash markers: the long
    //!   and short lists, and `done_vars` afterwards (5,400 cases);
    //! * `_trimInstitution` over the same settings.
    //!
    //! Pass criterion: every result equal to citeproc-js's.
    use serde_json::Value;

    use super::*;
    use crate::citeproc::build_retrieve_item::canon_numbers;
    use crate::citeproc::js::Obj;
    use crate::citeproc::obj_token::{Token, TokenType};
    use crate::citeproc::state::State;
    use crate::citeproc::EngineError;
    use crate::citeproc::util_locale::Locale;
    use crate::citeproc::util_names_output::testing::REFERENCE;
    use crate::citeproc::util_names_output::NameOutput;

    fn get_name_state(v: &Value) -> (State, NameInputCtx) {
        let mut st = State::default();
        let strings = |k: &str| -> Value {
            v.get(k)
                .cloned()
                .unwrap_or_else(|| Value::Array(Vec::new()))
        };
        st.opt
            .insert("default-locale".into(), serde_json::json!(["en-US"]));
        st.opt.insert("locale-translit".into(), strings("translit"));
        st.opt.insert("locale-translat".into(), strings("translat"));
        st.opt.insert("locale-sort".into(), strings("sort"));
        let locale = |opts: Value| {
            let mut l = Locale::new();
            if let Value::Object(o) = opts {
                l.opts = o;
            }
            l
        };
        st.locale.insert(
            "en-US".into(),
            locale(serde_json::json!({
                "name-as-sort-order": {"ja": true, "zh": true, "ko": true},
                "name-as-reverse-order": {"hu": true},
                "name-never-short": {"ja": true}
            })),
        );
        st.locale.insert(
            "ja-JP".into(),
            locale(serde_json::json!({
                "name-as-sort-order": {"ja": true},
                "name-as-reverse-order": {},
                "name-never-short": {"zh": true}
            })),
        );
        st.locale
            .insert("fr-FR".into(), locale(serde_json::json!({})));
        let ctx = NameInputCtx {
            parse_names: v.get("parse").and_then(Value::as_bool).unwrap_or(true),
            auto_vietnamese_names: v.get("vn").and_then(Value::as_bool).unwrap_or(false),
            item_language: v.get("lang").and_then(Value::as_str).map(str::to_string),
        };
        (st, ctx)
    }

    #[test]
    fn get_name_matches_citeproc_js() {
        let r = &REFERENCE["getName"];
        let variants = r["variants"].as_array().expect("variants");
        let slots: Vec<&str> = r["slots"]
            .as_array()
            .expect("slots")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        let mut n = 0;
        for c in r["cases"].as_array().expect("cases") {
            for (vi, v) in variants.iter().enumerate() {
                let (st, ctx) = get_name_state(v);
                let mut want_iter = c["v"][vi].as_array().expect("row").iter();
                for slot in &slots {
                    for fallback in [true, false] {
                        for stop in [None, Some(true)] {
                            let want = want_iter.next().expect("want");
                            let mut name = c["name"].clone();
                            let got = get_name(&st, &ctx, &mut name, slot, fallback, stop);
                            match (got, want.get("e")) {
                                (Ok(g), None) => {
                                    let mut gn =
                                        g.name.map(Value::Object).unwrap_or(Value::Bool(false));
                                    canon_numbers(&mut gn);
                                    assert_eq!(
                                        gn,
                                        want["name"]
                                            .clone()
                                            .as_object()
                                            .map(|_| want["name"].clone())
                                            .unwrap_or(Value::Bool(false)),
                                        "getName {} [{}] {slot} fb={fallback} stop={stop:?}",
                                        c["name"],
                                        v["name"]
                                    );
                                    let uo = match g.used_orig {
                                        Some(b) => Value::Bool(b),
                                        None => Value::Null,
                                    };
                                    assert_eq!(
                                        uo, want["usedOrig"],
                                        "usedOrig {} [{}] {slot}",
                                        c["name"], v["name"]
                                    );
                                }
                                (Err(e), Some(w)) => {
                                    let text = match &e {
                                        EngineError::BadInput(m) => m.clone(),
                                        o => o.to_string(),
                                    };
                                    assert_eq!(w.as_str(), Some(text.as_str()), "{}", c["name"]);
                                }
                                (g, w) => panic!(
                                    "getName {} [{}] {slot}: {g:?} vs {w:?}",
                                    c["name"], v["name"]
                                ),
                            }
                            if want.get("e").is_none() {
                                canon_numbers(&mut name);
                                assert_eq!(
                                    name, want["after"],
                                    "caller's name after getName {} [{}] {slot}",
                                    c["name"], v["name"]
                                );
                            }
                            n += 1;
                        }
                    }
                }
            }
        }
        assert!(n > 8000, "{n}");
    }

    fn inst_output(
        strings: &Value,
        item: &Value,
        legacy: bool,
        abbrevs: &Value,
    ) -> (State, NameOutput) {
        let mut st = State::default();
        st.opt.insert(
            "development_extensions".into(),
            serde_json::json!({ "legacy_institution_name_ordering": legacy }),
        );
        let mut table = crate::citeproc::Abbreviations::new();
        if let Value::Object(by_j) = abbrevs {
            for (j, cats) in by_j {
                for (cat, keys) in cats.as_object().into_iter().flatten() {
                    for (k, v) in keys.as_object().into_iter().flatten() {
                        table
                            .entry(j.clone())
                            .or_default()
                            .entry(cat.clone())
                            .or_default()
                            .insert(k.clone(), v.as_str().unwrap_or_default().to_string());
                    }
                }
            }
        }
        st.sys.abbreviations = table;
        let mut tok = Token::new("institution", TokenType::Start);
        if let Value::Object(o) = strings {
            for (k, v) in o {
                tok.strings.insert(k.clone(), v.clone());
            }
        }
        let mut no = NameOutput::new(item, &Value::Null);
        no.institution = Some(tok);
        (st, no)
    }

    #[test]
    fn fixup_institution_matches_citeproc_js() {
        let r = &REFERENCE["inst"];
        let names = r["names"].as_array().expect("names");
        let strings = r["strings"].as_array().expect("strings");
        let items = r["items"].as_array().expect("items");
        let mut n = 0;
        let (mut errors, mut abbreviated, mut quashed) = (0, 0, 0);
        for c in r["cases"].as_array().expect("cases") {
            let nm = c["n"].as_str().expect("n");
            errors += usize::from(c.get("e").is_some());
            quashed += usize::from(c["done"].as_array().map(|d| !d.is_empty()).unwrap_or(false));
            abbreviated += usize::from(
                c["v"]["long"] != c["v"]["short"]
                    && c["v"]["short"]
                        .as_array()
                        .map(|s| !s.is_empty())
                        .unwrap_or(false),
            );
            assert!(names.iter().any(|x| x.as_str() == Some(nm)));
            let (mut st, no) = inst_output(
                &strings[c["s"].as_u64().expect("s") as usize],
                &items[c["i"].as_u64().expect("i") as usize],
                c["l"].as_bool().expect("l"),
                &r["abbrevs"],
            );
            st.tmp.just_looking = c["j"].as_bool().expect("j");
            let mut name = Obj::new();
            name.insert("literal".into(), Value::String(nm.to_string()));
            let got = no.fixup_institution(&mut st, name, "author", Some(0));
            match (got, c.get("e")) {
                (Ok(g), None) => {
                    let want = &c["v"];
                    assert_eq!(
                        serde_json::json!({"long": g.long, "short": g.short}),
                        *want,
                        "fixupInstitution({nm:?}) settings={} item={}",
                        r["strings"][c["s"].as_u64().unwrap_or(0) as usize],
                        r["items"][c["i"].as_u64().unwrap_or(0) as usize]
                    );
                }
                (Err(e), Some(w)) => {
                    assert!(matches!(&e, EngineError::BadInput(_)), "{e:?} vs {w}");
                }
                (g, w) => panic!("fixupInstitution({nm:?}): {g:?} vs {w:?}"),
            }
            assert_eq!(
                serde_json::json!(st.tmp.done_vars),
                c["done"],
                "done_vars after fixupInstitution({nm:?}) settings={}",
                r["strings"][c["s"].as_u64().unwrap_or(0) as usize]
            );
            n += 1;
        }
        assert!(n > 5000, "{n}");
        // The reference is not vacuous: it has abbreviated short forms and quashes.
        assert!(
            abbreviated > 1000 && quashed > 100,
            "{errors} {abbreviated} {quashed}"
        );
        let mut t = 0;
        for c in r["trim"].as_array().expect("trim") {
            let (_st, no) = inst_output(
                &strings[c["s"].as_u64().expect("s") as usize],
                &Value::Null,
                false,
                &Value::Null,
            );
            let lst: Vec<String> = c["l"]
                .as_array()
                .expect("l")
                .iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect();
            assert_eq!(serde_json::json!(no.trim_institution(&lst)), c["v"], "{c}");
            t += 1;
        }
        assert!(t > 50);
    }
}
