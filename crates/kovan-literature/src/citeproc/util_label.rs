// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_label.js
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

//! Port of `src/util_label.js`: `CSL.evaluateLabel` and `CSL.castLabel`, which
//! pick the term and plural form for a `<label>` node.
//!
//! Both read and write `state.tmp.group_context.tip` (`label_form`,
//! `label_capitalize_if_first`, `label_static`) and `state.tmp.strip_periods`;
//! those fields belong to the group machinery (wave 2) and do not exist yet,
//! so the functions take them as a [`LabelContext`] that the caller fills from
//! `state.tmp` and reads back afterwards.

use serde_json::Value;

use super::js;
use super::obj_token::{Decoration, Token};
use super::state::State;
use super::load::TOLERANT;
use super::util_number::{input_get_term, process_number, ShadowNumber, TermQuery};
use super::{CslResult, EngineError};

/// The parts of `state.tmp` that `evaluateLabel` / `castLabel` use.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LabelContext {
    /// `state.tmp.group_context.tip.label_form`.
    pub tip_label_form: Option<String>,
    /// `state.tmp.group_context.tip.label_capitalize_if_first`.
    pub tip_label_capitalize_if_first: Option<Value>,
    /// `state.tmp.group_context.tip.label_static` (output: set by
    /// `castLabel` for a `form="static"` label inside a labelled group).
    pub tip_label_static: bool,
    /// `state.tmp.strip_periods`.
    pub strip_periods: bool,
}

/// `CSL.evaluateLabel(node, state, Item, item)`: the label text for `node`
/// (a `<label>` token) given the item and the citation item (`None` outside
/// a citation).
///
/// Chooses the term (`locator` becomes the cite's `label`, default `page`),
/// then, unless the node fixes `plural`, parses the variable's value with
/// [`process_number`] to decide singular or plural, recording the label form
/// and decorations in `state.tmp.shadow_numbers[term]`.
pub fn evaluate_label(
    state: &mut State,
    node: &mut Token,
    ctx: &mut LabelContext,
    item: &Value,
    cite_item: Option<&Value>,
) -> CslResult<String> {
    let term = node.string_opt("term").unwrap_or_default();
    let mut myterm: Option<String> = None;
    if node.strings.contains_key("term") && term == "locator" {
        if let Some(ci) = cite_item {
            if js::truthy_opt(ci.get("label")) {
                let label = js::to_js_string(ci.get("label").unwrap_or(&Value::Null));
                myterm = Some(if label == "sub verbo" {
                    "sub-verbo".into()
                } else {
                    label
                });
            }
        }
        if myterm.as_deref().map(str::is_empty).unwrap_or(true) {
            myterm = Some("page".into());
        }
    } else {
        myterm = node.string_opt("term");
    }

    // Plurals detection.
    let mut plural: Option<i64> = match node.strings.get("plural") {
        Some(Value::Number(n)) => n.as_i64(),
        _ => None,
    };
    let plural_is_number = matches!(node.strings.get("plural"), Some(Value::Number(_)));
    if !plural_is_number {
        // (node, ItemObject, variable, type)
        let the_item = match cite_item {
            Some(ci) if term == "locator" => ci,
            _ => item,
        };
        if js::truthy_opt(the_item.get(term.as_str())) {
            process_number(state, None, Some(the_item), &term)?;
            let (sn_plural, has_form, has_decor) = {
                let sn = state
                    .tmp
                    .shadow_numbers
                    .get(&term)
                    .cloned()
                    .unwrap_or_default();
                (
                    sn.plural,
                    sn.label_form
                        .as_deref()
                        .map(|f| !f.is_empty())
                        .unwrap_or(false),
                    sn.label_decorations.is_some(),
                )
            };
            plural = sn_plural;
            if !has_form && !has_decor {
                let form = node.strings.get("form").cloned();
                let cap = node.strings.get("capitalize_if_first").cloned();
                let decorations: Vec<Decoration> = node.decorations.clone();
                if let Some(sn) = state.tmp.shadow_numbers.get_mut(&term) {
                    if form.as_ref().map(js::truthy).unwrap_or(false) {
                        sn.label_form = form.as_ref().map(js::to_js_string);
                    } else if let Some(f) = ctx.tip_label_form.as_ref().filter(|f| !f.is_empty()) {
                        sn.label_form = Some(f.clone());
                    }
                    sn.label_capitalize_if_first = cap;
                    sn.label_decorations = Some(decorations);
                }
            }
            if ["locator", "number", "page"].contains(&term.as_str()) {
                let sn: Option<&ShadowNumber> = state.tmp.shadow_numbers.get(&term);
                if let Some(super::util_number::ShadowLabel::Term(t)) =
                    sn.and_then(|s| s.label.as_ref())
                {
                    if !t.is_empty() {
                        myterm = Some(t.clone());
                    }
                }
            }
            let csl_reverse = state
                .opt
                .get("development_extensions")
                .and_then(|d| d.get("csl_reverse_lookup_support"))
                .map(js::truthy)
                .unwrap_or(false);
            if csl_reverse {
                // node.decorations.reverse(); push(["@showid","true",node.cslid]); reverse()
                let cslid = node.extra.get("cslid").map(js::to_js_string);
                node.decorations.reverse();
                let mut d = Decoration::new("@showid", "true");
                d.extra = cslid;
                node.decorations.push(d);
                node.decorations.reverse();
            }
        }
    }
    cast_label(state, node, ctx, myterm.as_deref(), plural, TOLERANT)
}

/// `CSL.castLabel(state, node, term, plural, mode)`: the label term in the
/// node's (or group's) form, capitalised and with periods stripped as the
/// node asks.
///
/// PORT-LATER(wave1-output): util_label.js:68, needs
/// `CSL.Output.Formatters["capitalize-first"]` (formatters.js); a label that
/// asks for it returns `NotYetPorted`.
pub fn cast_label(
    state: &mut State,
    node: &Token,
    ctx: &mut LabelContext,
    term: Option<&str>,
    plural: Option<i64>,
    mode: i64,
) -> CslResult<String> {
    let mut label_form = node.strings.get("form").cloned();
    let mut label_capitalize_if_first = node.strings.get("capitalize_if_first").cloned();
    if let Some(tip_form) = ctx.tip_label_form.as_ref().filter(|f| !f.is_empty()) {
        if label_form.as_ref().and_then(Value::as_str) == Some("static") {
            ctx.tip_label_static = true;
        } else {
            label_form = Some(Value::String(tip_form.clone()));
        }
    }
    if let Some(c) = ctx
        .tip_label_capitalize_if_first
        .as_ref()
        .filter(|c| js::truthy(c))
    {
        label_capitalize_if_first = Some(c.clone());
    }
    let ret = match term {
        None => None,
        Some(t) => {
            let mut q = TermQuery::new(t);
            q.form = label_form
                .as_ref()
                .and_then(Value::as_str)
                .map(str::to_string);
            q.plural = plural.unwrap_or(0);
            q.mode = Some(mode);
            q.force_default_locale = js::truthy_opt(node.extra.get("default_locale"))
                || node
                    .strings
                    .get("default_locale")
                    .map(js::truthy)
                    .unwrap_or(false);
            input_get_term(state, &q)
        }
    };
    // getTerm in tolerant mode turns "undefined" into "".
    let mut ret = match ret {
        Some(r) => r,
        None if mode == TOLERANT => String::new(),
        None => {
            return Err(EngineError::BadInput(
                "Cannot read properties of undefined (reading 'replace')".into(),
            ))
        }
    };
    if label_capitalize_if_first
        .as_ref()
        .map(js::truthy)
        .unwrap_or(false)
    {
        return Err(EngineError::NotYetPorted {
            method: "CSL.Output.Formatters[\"capitalize-first\"]",
        });
    }
    // Tag: strip-periods-block
    if ctx.strip_periods {
        ret = ret.replace('.', "");
    } else {
        for d in &node.decorations {
            if d.name == "@strip-periods" && d.value == "true" {
                ret = ret.replace('.', "");
                break;
            }
        }
    }
    Ok(ret)
}

#[cfg(test)]
mod tests {
    //! Differential tests against citeproc-js 2.4.63: reference
    //! `tests/data/csl/units/locator.json` (generator
    //! `scripts/csl-units/locator.cjs`), section `label_cases`: ~2,000
    //! `CSL.evaluateLabel` calls over label terms, forms, plurals, group
    //! label forms, strip-periods, default-locale and reverse-lookup
    //! settings, items and locators, in three locales. Compared: the label
    //! text, `label_static`, the node's decorations and the label fields
    //! recorded in `shadow_numbers`. Labels asking for `capitalize-first`
    //! reach the output formatters (not ported here) and must report
    //! `NotYetPorted`.

    use serde_json::json;

    use super::*;
    use crate::citeproc::js::Obj;
    use crate::citeproc::obj_token::TokenType;

    const REF: &str = include_str!("../../tests/data/csl/units/locator.json");

    #[test]
    fn evaluate_label_matches_citeproc_js() {
        let r: Value = serde_json::from_str(REF).expect("json");
        let mut n = 0;
        let mut deferred = 0;
        for c in r["label_cases"].as_array().expect("cases") {
            let e = &r["label_engines"][c["engine"].as_str().unwrap_or("")];
            let mut st = State::default();
            st.opt = e["opt"].as_object().cloned().unwrap_or_default();
            super::super::test_support::install_locale(
                &mut st,
                super::super::test_support::logged_locale(&e["log"], None, None),
            );
            let mut node = Token::new("label", TokenType::Singleton);
            node.strings = c["node"]["strings"]
                .as_object()
                .cloned()
                .unwrap_or_else(Obj::new);
            node.decorations = c["node"]["decorations"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|d| {
                            Decoration::new(
                                d[0].as_str().unwrap_or(""),
                                d[1].as_str().unwrap_or(""),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            node.extra
                .insert("cslid".into(), c["node"]["cslid"].clone());
            if c["node"]["default_locale"].as_bool() == Some(true) {
                node.extra
                    .insert("default_locale".into(), Value::Bool(true));
            }
            let mut ctx = LabelContext {
                tip_label_form: c["tip_form"].as_str().map(str::to_string),
                strip_periods: c["strip"].as_bool().unwrap_or(false),
                ..LabelContext::default()
            };
            let cite = if c["cite"].is_null() {
                None
            } else {
                Some(&c["cite"])
            };
            let res = evaluate_label(&mut st, &mut node, &mut ctx, &c["Item"], cite);
            n += 1;
            let cap = node
                .strings
                .get("capitalize_if_first")
                .map(js::truthy)
                .unwrap_or(false);
            match (res, c.get("error")) {
                (Err(EngineError::NotYetPorted { .. }), _) if cap => deferred += 1,
                (Ok(s), None) => {
                    assert_eq!(Value::String(s), c["out"], "label {c}");
                    assert_eq!(
                        Value::Bool(ctx.tip_label_static),
                        c["tip_static"],
                        "label_static {c}"
                    );
                    let deco: Vec<Value> = node
                        .decorations
                        .iter()
                        .map(|d| match &d.extra {
                            Some(x) => json!([d.name, d.value, x.parse::<i64>().unwrap_or(0)]),
                            None => json!([d.name, d.value]),
                        })
                        .collect();
                    assert_eq!(
                        Value::Array(deco),
                        c["node_decorations"],
                        "node decorations {c}"
                    );
                    let mut shadow = Obj::new();
                    for (k, v) in &st.tmp.shadow_numbers {
                        shadow.insert(k.clone(), v.to_value());
                    }
                    assert_eq!(Value::Object(shadow), c["shadow"], "shadow_numbers {c}");
                }
                (Err(e), Some(w)) => assert_eq!(
                    w.as_str(),
                    Some(match &e {
                        EngineError::BadInput(m) => m.as_str(),
                        _ => "",
                    }),
                    "{c}"
                ),
                (g, w) => panic!("{c}: {g:?} vs {w:?}"),
            }
        }
        assert!(n > 1500 && deferred > 0, "{n} {deferred}");
    }
}
