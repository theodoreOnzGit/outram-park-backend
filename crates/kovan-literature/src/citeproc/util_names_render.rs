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

//! Port of `src/util_names_render.js`. **Mostly not yet ported** (epic #790);
//! only the input side (`_normalizeNameInput`, `_parseName`, `getStaticOrder`,
//! `_isRomanesque`) is here, see the `input side (wave1-input)` section.

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
    use std::sync::LazyLock;

    use regex::Regex;
    use serde_json::Value;

    use super::super::js::{self, Obj};
    use super::super::state::State;
    use super::super::util_name_particles::parse_particles;
    use super::super::{CslResult, EngineError};

    fn rx(src: &str) -> Regex {
        Regex::new(src).unwrap_or_else(|e| panic!("invalid static regex {src:?}: {e}"))
    }

    /// The letter ranges of `CSL.ROMANESQUE_REGEXP` without the leading `-0-9`.
    const ROMANESQUE_LETTERS: &str = "a-zA-Z\\x{0e01}-\\x{0e5b}\\x{00c0}-\\x{017f}\\x{0370}-\\x{03ff}\\x{0400}-\\x{052f}\\x{0590}-\\x{05d4}\\x{05d6}-\\x{05ff}\\x{1f00}-\\x{1fff}\\x{0600}-\\x{06ff}\\x{200c}\\x{200d}\\x{200e}\\x{0218}\\x{0219}\\x{021a}\\x{021b}\\x{202a}-\\x{202e}";

    // DUP-CHECK: load.js CSL.ROMANESQUE_REGEXP
    static ROMANESQUE_RE: LazyLock<Regex> =
        LazyLock::new(|| rx(&format!("[-0-9{ROMANESQUE_LETTERS}]")));

    // DUP-CHECK: load.js CSL.STARTSWITH_ROMANESQUE_REGEXP
    static STARTSWITH_ROMANESQUE_RE: LazyLock<Regex> =
        LazyLock::new(|| rx(&format!("^[&{ROMANESQUE_LETTERS}]")));

    const VIETNAMESE_SPECIAL_CLASS: &str = "\\x{00c0}-\\x{00c3}\\x{00c8}-\\x{00ca}\\x{00cc}\\x{00cd}\\x{00d2}-\\x{00d5}\\x{00d9}\\x{00da}\\x{00dd}\\x{00e0}-\\x{00e3}\\x{00e8}-\\x{00ea}\\x{00ec}\\x{00ed}\\x{00f2}-\\x{00f5}\\x{00f9}\\x{00fa}\\x{00fd}\\x{0101}\\x{0103}\\x{0110}\\x{0111}\\x{0128}\\x{0129}\\x{0168}\\x{0169}\\x{01a0}\\x{01a1}\\x{01af}\\x{01b0}\\x{1ea0}-\\x{1ef9}";

    // DUP-CHECK: load.js CSL.VIETNAMESE_SPECIALS
    static VIETNAMESE_SPECIALS_RE: LazyLock<Regex> =
        LazyLock::new(|| rx(&format!("[{VIETNAMESE_SPECIAL_CLASS}]")));

    // DUP-CHECK: load.js CSL.VIETNAMESE_NAMES
    static VIETNAMESE_NAMES_RE: LazyLock<Regex> = LazyLock::new(|| {
        rx(&format!(
            "^(?:(?:[.AaBbCcDdEeGgHhIiKkLlMmNnOoPpQqRrSsTtUuVvXxYy {VIETNAMESE_SPECIAL_CLASS}]{{2,6}})([{ws}]+|$))+$",
            ws = js::WS
        ))
    });

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
