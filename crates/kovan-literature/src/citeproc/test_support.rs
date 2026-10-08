// Part of the kovan port of citeproc-js (GitHub #790, #792).
//
// Upstream:    none (test support written for this port)
// Copyright:   (c) 2026 the OUTRAM PARK contributors
// Licence:     AGPL-3.0, as this crate (see its NOTICE).
// No warranty: this program is distributed in the hope that it will be
//              useful, but WITHOUT ANY WARRANTY; without even the implied
//              warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
//              PURPOSE. See the GNU Affero General Public License.

//! Test support: rebuild the part of `state.locale[lang]` that a differential
//! reference recorded.
//!
//! The reference JSON files of `tests/data/csl/units/` record, for each
//! citeproc-js engine they were produced with, every `getTerm` /
//! `CSL.Engine.getField` query that was made and the answer (`log`,
//! `fields`), and `locale[lang].ord["1.0.1"]`. [`logged_locale`] builds a
//! [`Locale`] whose `terms` answer exactly those queries through
//! [`State::get_term`](super::state::State::get_term), so the number, date and
//! label logic can be tested without the locale files (which live under the
//! git-ignored `vendor/`).

use serde_json::Value;

use super::js::Obj;
use super::state::State;
use super::util_locale::Locale;

/// Put `val` at `terms[name][gender?][form][plural]` (the shape
/// `State::get_field` reads).
fn put(terms: &mut Obj, name: &str, form: &str, plural: usize, gender: Option<&str>, val: &str) {
    let entry = terms
        .entry(name.to_string())
        .or_insert_with(|| Value::Object(Obj::new()));
    let Some(mut cur) = entry.as_object_mut() else {
        return;
    };
    if let Some(g) = gender {
        let slot = cur
            .entry(g.to_string())
            .or_insert_with(|| Value::Object(Obj::new()));
        let Some(o) = slot.as_object_mut() else {
            return;
        };
        cur = o;
    }
    let slot = cur
        .entry(form.to_string())
        .or_insert_with(|| Value::Array(vec![Value::Null, Value::Null]));
    if let Some(a) = slot.as_array_mut() {
        while a.len() <= plural {
            a.push(Value::Null);
        }
        a[plural] = Value::String(val.to_string());
    }
}

/// A [`Locale`] answering the logged queries.
///
/// * `terms`: `{"name|form|plural|gender|mode|force": answer | null}`, the
///   keys of `TermQuery::key` (`~` for unset);
/// * `fields`: `{"mode|name|form|plural|gender": answer | null}`;
/// * `ord_101`: `locale.ord["1.0.1"]`, if the locale has it.
///
/// A `null` answer (JS `undefined`) adds nothing.
pub(crate) fn logged_locale(
    terms: &Value,
    fields: Option<&Value>,
    ord_101: Option<&Value>,
) -> Locale {
    let mut locale = Locale::new();
    if let Some(o) = terms.as_object() {
        for (key, v) in o {
            let Some(val) = v.as_str() else { continue };
            let p: Vec<&str> = key.split('|').collect();
            if p.len() < 4 {
                continue;
            }
            let form = if p[1] == "~" { "long" } else { p[1] };
            let plural = p[2].parse::<usize>().unwrap_or(0);
            let gender = (p[3] != "~").then_some(p[3]);
            put(&mut locale.terms, p[0], form, plural, gender, val);
        }
    }
    if let Some(o) = fields.and_then(Value::as_object) {
        for (key, v) in o {
            let Some(val) = v.as_str() else { continue };
            let p: Vec<&str> = key.split('|').collect();
            if p.len() < 5 {
                continue;
            }
            let plural = p[3].parse::<usize>().unwrap_or(0);
            let gender = (p[4] != "~").then_some(p[4]);
            put(&mut locale.terms, p[1], p[2], plural, gender, val);
        }
    }
    if let Some(o) = ord_101.filter(|v| v.is_object()) {
        locale.ord.insert("1.0.1".into(), o.clone());
    }
    locale
}

/// Install `locale` as `state.locale[state.opt.lang]` and as the default
/// locale (`opt["default-locale"][0]`), setting `opt.lang` to `"en-US"` when
/// the state has none.
pub(crate) fn install_locale(state: &mut State, locale: Locale) {
    let lang = state
        .opt
        .get("lang")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| "en-US".to_string());
    state.opt.insert("lang".into(), Value::String(lang.clone()));
    if let Some(d) = state
        .opt
        .get("default-locale")
        .and_then(|v| v.get(0))
        .and_then(Value::as_str)
    {
        state.locale.insert(d.to_string(), locale.clone());
    }
    state.locale.insert(lang, locale);
}

/// Install an en-US locale holding only what the output code reads through
/// `getTerm` / `getOpt`: the four quote terms, the page, year and citation
/// range delimiters, and `opts["punctuation-in-quote"]` (`piq`).
pub(crate) fn install_output_locale(state: &mut State, piq: bool) {
    let mut locale = Locale::new();
    for (name, val) in [
        ("open-quote", "\u{201C}"),
        ("close-quote", "\u{201D}"),
        ("open-inner-quote", "\u{2018}"),
        ("close-inner-quote", "\u{2019}"),
        ("page-range-delimiter", "\u{2013}"),
        ("year-range-delimiter", "\u{2013}"),
        ("citation-range-delimiter", "\u{2013}"),
    ] {
        put(&mut locale.terms, name, "long", 0, None, val);
    }
    locale
        .opts
        .insert("punctuation-in-quote".into(), Value::Bool(piq));
    state
        .opt
        .insert("lang".into(), Value::String("en-US".into()));
    install_locale(state, locale);
}
