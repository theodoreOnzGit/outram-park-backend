// Part of the kovan port of citeproc-js (GitHub #790, #792).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      scripts/csl-intermediate-reference.cjs (the canonical form), not a citeproc-js source file
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

//! The intermediate dump of an [`Engine`]'s built state in the canonical form
//! of `scripts/csl-intermediate-reference.cjs` (GitHub #792): the `style` and
//! `locale` sections, which `tests/citeproc_intermediate.rs` compares by
//! SHA-256 with what citeproc-js produced.
//!
//! # The canonical form
//!
//! JSON with every object's keys sorted (serde_json's map does that), `undefined`
//! and functions omitted, a `RegExp` as `{"$regexp": source}`, and a token as its
//! own data properties with its closures reduced to counts: `execs` becomes
//! `execs_n`, `tests` becomes `tests_n` and `test` becomes `has_test: true`.
//! A token reached *inside* another token drops its `next`, `succeed` and
//! `fail`.
//!
//! * **style**: `csl_version`, `processor_version`, `opt`, `areas` (for each
//!   of the five areas `root`, `opt` and the `tokens` citeproc-js built and
//!   configured), `macros` (each macro's tokens), `cite_affixes`
//!   (`tmp.cite_affixes`) and `names_level` (`build.names_level`).
//! * **locale**: `locale` (every language loaded, as [`Locale::to_value`])
//!   and `gender` (`opt.gender`). citeproc-js's structure for a locale with
//!   gendered terms is cyclic and its own dump overflows the stack on it, so
//!   such an engine's whole section is `{"error": "Maximum call stack size
//!   exceeded"}`; so is this port's (see `util_locale.rs`).
//!
//! # Known limits of the token form
//!
//! `tests_n` is emitted when the token has tests. citeproc-js also has tokens
//! whose `tests` array exists but is empty (it prints `tests_n: 0`); the
//! foundation's `Token` cannot tell that from "no array", so those differ
//! until the node builders record it (see the report of wave1-build).

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use super::js::Obj;
use super::load;
use super::obj_token::Token;
use super::state::State;
use super::Engine;

/// The error text citeproc-js records when it overflows the stack on a
/// cyclic locale.
pub const STACK_OVERFLOW: &str = "Maximum call stack size exceeded";

/// A token as the canonical form prints it. `nested` is true for a token
/// reached inside another (its jump indices are dropped). With
/// `closure_counts` false, `execs_n`, `tests_n` and `has_test` are left out
/// (the "reduced" form).
pub fn token_value(t: &Token, nested: bool, closure_counts: bool) -> Value {
    let mut o = Map::new();
    o.insert("name".into(), Value::String(t.name.clone()));
    o.insert("tokentype".into(), Value::from(t.tokentype.as_js()));
    o.insert("strings".into(), Value::Object(t.strings.clone()));
    o.insert(
        "decorations".into(),
        Value::Array(
            t.decorations
                .iter()
                .map(|d| {
                    let mut a = vec![
                        Value::String(d.name.clone()),
                        Value::String(d.value.clone()),
                    ];
                    if let Some(x) = &d.extra {
                        a.push(Value::String(x.clone()));
                    }
                    Value::Array(a)
                })
                .collect(),
        ),
    );
    o.insert(
        "variables".into(),
        Value::Array(t.variables.iter().cloned().map(Value::String).collect()),
    );
    if closure_counts {
        o.insert("execs_n".into(), Value::from(t.execs.len()));
        if !t.tests.is_empty() {
            o.insert("tests_n".into(), Value::from(t.tests.len()));
        }
        if t.test.is_some() {
            o.insert("has_test".into(), Value::Bool(true));
        }
    }
    if !nested {
        if let Some(n) = t.next {
            o.insert("next".into(), Value::from(n));
        }
        if let Some(n) = t.succeed {
            o.insert("succeed".into(), Value::from(n));
        }
        if let Some(n) = t.fail {
            o.insert("fail".into(), Value::from(n));
        }
    }
    if let Some(m) = &t.postponed_macro {
        o.insert("postponed_macro".into(), Value::String(m.clone()));
    }
    for (k, v) in &t.extra {
        o.insert(k.clone(), v.clone());
    }
    Value::Object(o)
}

fn tokens_value(ts: &[Token], closure_counts: bool) -> Value {
    Value::Array(
        ts.iter()
            .map(|t| token_value(t, false, closure_counts))
            .collect(),
    )
}

/// The `style` section of the dump (see the module docs). `closure_counts`
/// false gives the reduced form without `execs_n`/`tests_n`/`has_test`.
pub fn style_section(engine: &Engine, closure_counts: bool) -> Value {
    style_section_of(engine.state(), closure_counts)
}

/// [`style_section`] over a bare [`State`].
pub fn style_section_of(s: &State, closure_counts: bool) -> Value {
    let mut areas = Map::new();
    for a in load::AREAS {
        let area = s.area_ref(a);
        let mut o = Map::new();
        o.insert("root".into(), Value::String(area.root.clone()));
        o.insert("tokens".into(), tokens_value(&area.tokens, closure_counts));
        o.insert("opt".into(), Value::Object(area.opt.clone()));
        areas.insert((*a).to_string(), Value::Object(o));
    }
    let mut macros = Map::new();
    for (k, v) in &s.macros {
        macros.insert(k.clone(), tokens_value(v, closure_counts));
    }
    let mut o = Map::new();
    o.insert("csl_version".into(), Value::String(s.csl_version.clone()));
    o.insert(
        "processor_version".into(),
        Value::String(s.processor_version.clone()),
    );
    o.insert("opt".into(), Value::Object(s.opt.clone()));
    o.insert("areas".into(), Value::Object(areas));
    o.insert("macros".into(), Value::Object(macros));
    o.insert(
        "cite_affixes".into(),
        Value::Object(s.tmp.cite_affixes.clone()),
    );
    o.insert("names_level".into(), Value::from(s.build.names_level));
    Value::Object(o)
}

/// The `locale` section of the dump (see the module docs).
pub fn locale_section(engine: &Engine) -> Value {
    locale_section_of(engine.state())
}

/// [`locale_section`] over a bare [`State`].
pub fn locale_section_of(s: &State) -> Value {
    if s.locale.values().any(|l| l.cyclic) {
        let mut o = Map::new();
        o.insert("error".into(), Value::String(STACK_OVERFLOW.to_string()));
        return Value::Object(o);
    }
    let mut locales = Obj::new();
    for (lang, l) in &s.locale {
        locales.insert(lang.clone(), l.to_value());
    }
    let mut o = Map::new();
    o.insert("locale".into(), Value::Object(locales));
    o.insert(
        "gender".into(),
        s.opt.get("gender").cloned().unwrap_or(Value::Null),
    );
    // `gender: undefined` is omitted by JSON.stringify; the constructor always
    // defines it.
    Value::Object(o)
}

/// The compact JSON text of a section (what the digest is of).
pub fn compact(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

/// Hex SHA-256 of [`compact`]`(v)`: `sha(x)` of the reference script.
pub fn digest(v: &Value) -> String {
    let mut h = Sha256::new();
    h.update(compact(v).as_bytes());
    let out = h.finalize();
    out.iter().map(|b| format!("{b:02x}")).collect()
}

/// [`style_section`] with the reduced closure counts removed *from a
/// reference section* (a `serde_json::Value` read from the committed
/// reference): the same reduction applied to citeproc-js's output, so the
/// reduced forms of both sides can be compared.
pub fn reduce_reference_style(v: &Value) -> Value {
    fn strip(v: &mut Value) {
        match v {
            Value::Object(o) => {
                // Only token objects carry these keys.
                if o.contains_key("tokentype") {
                    o.remove("execs_n");
                    o.remove("tests_n");
                    o.remove("has_test");
                }
                for x in o.values_mut() {
                    strip(x);
                }
            }
            Value::Array(a) => {
                for x in a.iter_mut() {
                    strip(x);
                }
            }
            _ => {}
        }
    }
    let mut c = v.clone();
    strip(&mut c);
    c
}

// ----------------------------------------------------------------------
// Exact text, in the key order the reference script prints.
//
// The reference script's section objects are *literals*: only the objects
// that pass through `canon` have sorted keys, the section wrappers keep the
// order they were written in (`style`: csl_version, processor_version, opt,
// areas in `CSL.AREAS` order each as root/tokens/opt, macros sorted,
// cite_affixes, names_level; `locale`: locale, gender). The digest is of that
// text, so the digests are computed from [`style_text`] and [`locale_text`].

fn json(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

fn tokens_text(ts: &[Token], closure_counts: bool) -> String {
    json(&tokens_value(ts, closure_counts))
}

/// The `style` section as the exact JSON text the reference script's
/// `JSON.stringify` prints.
pub fn style_text(engine: &Engine, closure_counts: bool) -> String {
    let s = engine.state();
    let mut out = String::from("{");
    out.push_str(&format!(
        "\"csl_version\":{},",
        json(&Value::String(s.csl_version.clone()))
    ));
    out.push_str(&format!(
        "\"processor_version\":{},",
        json(&Value::String(s.processor_version.clone()))
    ));
    out.push_str(&format!("\"opt\":{},", json(&Value::Object(s.opt.clone()))));
    out.push_str("\"areas\":{");
    for (i, a) in load::AREAS.iter().enumerate() {
        let area = s.area_ref(a);
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{}:{{\"root\":{},\"tokens\":{},\"opt\":{}}}",
            json(&Value::String((*a).to_string())),
            json(&Value::String(area.root.clone())),
            tokens_text(&area.tokens, closure_counts),
            json(&Value::Object(area.opt.clone()))
        ));
    }
    out.push_str("},\"macros\":{");
    for (i, (k, v)) in s.macros.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{}:{}",
            json(&Value::String(k.clone())),
            tokens_text(v, closure_counts)
        ));
    }
    out.push_str(&format!(
        "}},\"cite_affixes\":{},\"names_level\":{}}}",
        json(&Value::Object(s.tmp.cite_affixes.clone())),
        s.build.names_level
    ));
    out
}

/// The `locale` section as exact JSON text (see [`style_text`]).
pub fn locale_text(engine: &Engine) -> String {
    let s = engine.state();
    if s.locale.values().any(|l| l.cyclic) {
        return format!("{{\"error\":\"{STACK_OVERFLOW}\"}}");
    }
    let mut out = String::from("{\"locale\":{");
    for (i, (lang, l)) in s.locale.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{}:{}",
            json(&Value::String(lang.clone())),
            json(&l.to_value())
        ));
    }
    out.push_str(&format!(
        "}},\"gender\":{}}}",
        json(s.opt.get("gender").unwrap_or(&Value::Null))
    ));
    out
}

/// Hex SHA-256 of a text.
pub fn digest_text(text: &str) -> String {
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}
