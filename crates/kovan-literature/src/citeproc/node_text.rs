// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/node_text.js
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

//! Port of `src/node_text.js`: `CSL.Node.text`.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::attributes::area_ref;
use super::exec::Exec;
use super::formatters::capitalize_first;
use super::js;
use super::load::{
    dev_ext_truthy, update_group_context_condition, CITE_FIELDS, DESCENDING, DOT, MULTI_FIELDS,
    NUMERIC, TOLERANT, TRIGRAPH,
};
use super::node_group;
use super::obj_blob::Blob;
use super::obj_number::{new_numeric_blob, set_formatter, NumArg, NumFormatter};
use super::obj_token::{Decoration, Token, TokenType};
use super::queue::{self, AppendArg, FormatRef, QueueId};
use super::state::State;
use super::util_number::{output_numeric_field, padding, process_number};
use super::util_substitute;
use super::util_transform::run_output_function;
use super::{CslResult, EngineError};

/// The closures `src/node_text.js` stores in `token.execs` (PORTING.md §4).
/// Each variant is one `func`; the ones that capture build-time locals carry
/// them (`form`, `plural`, the transform parameters).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeTextExec {
    /// `citation-number` (node_text.js:82-130).
    CitationNumber,
    /// `year-suffix` (node_text.js:140-185).
    YearSuffix,
    /// `citation-label` (node_text.js:196-215).
    CitationLabel,
    /// `printterm` (node_text.js:222-275): a term, with `form` and `plural`
    /// captured from the build-time locals.
    PrintTerm {
        /// `form` (default `"long"`).
        form: String,
        /// `plural` (`this.strings.plural` or 0): 0, 1 or `false`.
        plural: Value,
    },
    /// The variable-flags closure that precedes a variable's renderer
    /// (node_text.js:281-299).
    VariableFlags,
    /// `state.transform.getOutputFunction(this.variables, abbrevfam, abbrfall,
    /// altvar, transfall)` (node_text.js:300-335). `variables` is the array
    /// `this.variables` as it was at build time (upstream shares the live array).
    TransformOutput {
        /// `this.variables`.
        variables: Vec<String>,
        /// `abbrevfam` (a variable name, or `false` for none).
        abbrevfam: Option<String>,
        /// `abbrfall`.
        abbrfall: bool,
        /// `altvar` (the `-short` variable name, or `false`).
        altvar: Option<String>,
        /// `transfall`.
        transfall: bool,
    },
    /// Per-cite field (locator etc.) read from `item` (node_text.js:340-362).
    CiteField,
    /// `page`, `volume`, ... via `state.processNumber` (node_text.js:364-371).
    NumberField,
    /// `URL` / `DOI` (node_text.js:372-450). `form` is captured.
    UrlOrDoi {
        /// `form`.
        form: String,
    },
    /// `section` (node_text.js:452-462). `form` is captured.
    Section {
        /// `form`.
        form: String,
    },
    /// `hereinafter` (node_text.js:464-472).
    Hereinafter,
    /// Any other variable, output as is (node_text.js:474-487). `form` is captured.
    PlainVariable {
        /// `form`.
        form: String,
    },
    /// The `value` attribute (node_text.js:491-503).
    Value,
}

impl NodeTextExec {
    /// Run the closure.
    pub fn run(
        &self,
        state: &mut State,
        token: &mut Token,
        item: &Value,
        cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            NodeTextExec::CitationNumber => {
                citation_number(state, token, item, cite_item)?;
                Ok(None)
            }
            NodeTextExec::YearSuffix => {
                year_suffix(state, token, item)?;
                Ok(None)
            }
            NodeTextExec::CitationLabel => {
                citation_label(state, token, item)?;
                Ok(None)
            }
            NodeTextExec::PrintTerm { form, plural } => {
                print_term(state, token, item, form, plural)?;
                Ok(None)
            }
            NodeTextExec::VariableFlags => {
                variable_flags(state, token, item)?;
                Ok(None)
            }
            NodeTextExec::TransformOutput {
                abbrevfam, altvar, ..
            } => run_output_function(
                state,
                token,
                abbrevfam.as_deref(),
                altvar.as_deref(),
                item,
                cite_item,
            ),
            NodeTextExec::CiteField => {
                // per-cite fields are read from item, rather than Item
                let v0 = token.variables.first().cloned();
                let key = v0.clone().unwrap_or_else(|| "undefined".to_string());
                if js::truthy(cite_item) && js::truthy_opt(cite_item.get(key.as_str())) {
                    // Code copied to page variable as well; both become
                    // cs:number in MLZ extended schema
                    //
                    // If locator, use cs:number. Otherwise, render normally.

                    // XXX The code below is pretty-much copied from
                    // XXX node_number.js. Should be a common function.
                    // XXX BEGIN
                    process_number(state, Some(token), Some(cite_item), &key)?;
                    output_numeric_field(state, &key, &item_id(item))?;
                    // XXX END

                    let real0 = variables_real(token).first().cloned().unwrap_or_default();
                    if (real0 == "locator" || real0 == "locator-extra") && !state.tmp.just_looking {
                        state.tmp.done_vars.push(real0);
                    }
                }
                Ok(None)
            }
            NodeTextExec::NumberField => {
                let key = token
                    .variables
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "undefined".to_string());
                process_number(state, Some(token), Some(item), &key)?;
                output_numeric_field(state, &key, &item_id(item))?;
                Ok(None)
            }
            NodeTextExec::UrlOrDoi { form } => {
                url_or_doi(state, token, item, form)?;
                Ok(None)
            }
            NodeTextExec::Section { form } => {
                if let Some(v0) = token.variables.first().cloned() {
                    let value = get_variable(state, item, &v0, form)?;
                    if js::truthy_opt(value.as_ref()) {
                        append_text(state, &js_value_string(value), token)?;
                    }
                } else {
                    // state.getVariable(Item, undefined, form) is undefined.
                }
                Ok(None)
            }
            NodeTextExec::Hereinafter => {
                let value = state
                    .transform
                    .abbrev("default", "hereinafter", &item_id(item))
                    .map(str::to_string);
                if let Some(v) = value.filter(|v| !v.is_empty()) {
                    append_text(state, &v, token)?;
                    node_group::tip_mut(state)?.variable_success = true;
                }
                Ok(None)
            }
            NodeTextExec::PlainVariable { form } => {
                // anything left over just gets output in the normal way.
                if let Some(v0) = token.variables.first().cloned().filter(|v| !v.is_empty()) {
                    let value = get_variable(state, item, &v0, form)?;
                    if js::truthy_opt(value.as_ref()) {
                        let s = js_value_string(value);
                        let s = s.split('\\').collect::<Vec<_>>().join("");
                        append_text(state, &s, token)?;
                    }
                }
                Ok(None)
            }
            NodeTextExec::Value => {
                node_group::tip_mut(state)?.term_intended = true;
                // true flags that this is a literal-value term
                let value = token.string("value");
                update_group_context_condition(state, Some(&value), true, Some(token), None);
                append_text(state, &value, token)?;
                if state.tmp.can_block_substitute {
                    // Black magic here. This causes the cs:substitution condition to pass,
                    // blocking further rendering within its scope.
                    state
                        .tmp
                        .can_substitute
                        .replace_literal(Value::Bool(false))?;
                }
                Ok(None)
            }
        }
    }
}

/// `"" + Item.id`.
fn item_id(item: &Value) -> String {
    item.get("id")
        .map(js::to_js_string)
        .unwrap_or_else(|| "undefined".to_string())
}

/// `"" + value` for the result of `getVariable` (`undefined` is only
/// stringified when the caller has checked truthiness first).
fn js_value_string(v: Option<Value>) -> String {
    v.as_ref().map(js::to_js_string).unwrap_or_default()
}

/// `state.getVariable(Item, varname, form)`.
fn get_variable(
    state: &State,
    item: &Value,
    varname: &str,
    form: &str,
) -> CslResult<Option<Value>> {
    match item.as_object() {
        Some(o) => state.get_variable(o, varname, Some(form), None),
        None => Ok(None),
    }
}

/// `state.output.append(text, token)`.
fn append_text(state: &mut State, text: &str, token: &Token) -> CslResult<()> {
    queue::append(
        state,
        QueueId::Output,
        AppendArg::Text(text.to_string()),
        FormatRef::Token(token.clone()),
        false,
        false,
        false,
    )?;
    Ok(())
}

/// `state.registry.registry[id].seq`.
///
/// PORT-LATER(w2-engine): the registry (registry.js) is not ported. Until it
/// is, the entry comes from `state.tmp.render.registry_view` (which the tests
/// fill from citeproc-js's registry); its owner replaces this body with the
/// registry lookup. Errors where upstream would read an unregistered item.
fn registry_seq(state: &State, id: &str) -> CslResult<i64> {
    match state.tmp.render.registry_view.get(id) {
        Some(v) => Ok(v.seq),
        None => Err(EngineError::NotYetPorted {
            method: "state.registry.registry[id].seq",
        }),
    }
}

/// `state.registry.registry[id].disambig.year_suffix`: `None` when the item
/// has no registry entry, else the value (`false` when there is no suffix).
///
/// PORT-LATER(w2-engine): as [`registry_seq`].
fn registry_year_suffix(state: &State, id: &str) -> CslResult<Option<Value>> {
    if state.tmp.render.registry_view.is_empty() {
        return Err(EngineError::NotYetPorted {
            method: "state.registry.registry[id].disambig.year_suffix",
        });
    }
    Ok(state
        .tmp
        .render
        .registry_view
        .get(id)
        .map(|v| v.year_suffix.clone()))
}

/// `state.bibliography_sort.tmp.citation_number_map[seq]` when the map exists
/// (`None` when there is no map, `Some(None)` for a missing entry).
fn citation_number_map_lookup(state: &State, seq: i64) -> Option<Option<i64>> {
    let map = state.bibliography_sort.tmp.get("citation_number_map")?;
    if !js::truthy(map) {
        return None;
    }
    let v = match map {
        Value::Array(a) => a.get(seq as usize),
        Value::Object(o) => o.get(&seq.to_string()),
        _ => None,
    };
    Some(v.and_then(Value::as_i64))
}

/// The `citation-number` closure (node_text.js:82-130).
fn citation_number(
    state: &mut State,
    token: &mut Token,
    item: &Value,
    cite_item: &Value,
) -> CslResult<()> {
    let id = item_id(item);
    if !state.tmp.just_looking {
        if state.tmp.area.ends_with("_sort")
            && token.variables.first().map(String::as_str) == Some("citation-number")
        {
            if state.tmp.area == "bibliography_sort" {
                node_group::tip_mut(state)?
                    .done_vars
                    .push("citation-number".to_string());
            }
            let seq = registry_seq(state, &id)?;
            let num = if state.tmp.area == "citation_sort" {
                match citation_number_map_lookup(state, seq) {
                    Some(mapped) => mapped,
                    None => Some(seq),
                }
            } else {
                Some(seq)
            };
            let arg = match num {
                Some(n) if n != 0 => AppendArg::Text(padding(&n.to_string())),
                Some(n) => AppendArg::Text(n.to_string()),
                None => AppendArg::Undefined,
            };
            queue::append(
                state,
                QueueId::Output,
                arg,
                FormatRef::Token(token.clone()),
                false,
                false,
                false,
            )?;
            return Ok(());
        }
        if js::truthy(cite_item) && js::truthy_opt(cite_item.get("author-only")) {
            state
                .tmp
                .element_trace
                .replace_literal("suppress-me".to_string())?;
        }
        let seq = registry_seq(state, &id)?;
        let num = if state.tmp.area != "bibliography_sort"
            && state
                .bibliography_sort
                .tmp
                .contains_key("citation_number_map")
            && state
                .bibliography_sort
                .opt
                .get("citation_number_sort_direction")
                .and_then(Value::as_i64)
                == Some(DESCENDING)
        {
            citation_number_map_lookup(state, seq).unwrap_or(None)
        } else {
            Some(seq)
        };
        if js::truthy_opt(state.opt.get("citation_number_slug")) {
            let slug = state
                .opt
                .get("citation_number_slug")
                .map(js::to_js_string)
                .unwrap_or_default();
            append_text(state, &slug, token)?;
        } else {
            let n = num.ok_or_else(|| {
                EngineError::BadInput("citation number map has no entry for the item".into())
            })?;
            let blob = new_numeric_blob(
                state,
                None,
                NumArg::Number(n),
                Some(token),
                Some(&item_id(item)),
            )?;
            if state.tmp.in_cite_predecessor {
                state.blobs.get_mut(blob).suppress_splice_prefix = Some(true);
            }
            queue::append(
                state,
                QueueId::Output,
                AppendArg::Blob(blob),
                FormatRef::Name("literal".into()),
                false,
                false,
                false,
            )?;
        }
    }
    Ok(())
}

/// The `year-suffix` closure (node_text.js:140-185).
fn year_suffix(state: &mut State, token: &mut Token, item: &Value) -> CslResult<()> {
    let id = item_id(item);
    let ys = registry_year_suffix(state, &id)?;
    let Some(ys) = ys.filter(|v| *v != Value::Bool(false)) else {
        return Ok(());
    };
    if state.tmp.just_looking {
        return Ok(());
    }
    //state.output.append(state.registry.registry[Item.id].disambig[2],this);
    let num = js::parse_int_value(&ys)
        .ok_or_else(|| EngineError::BadInput("year_suffix is not a number".into()))?;
    let tmp_area = state.tmp.area.clone();
    if let Some(d) = area_ref(state, &tmp_area)?.opt.get("cite_group_delimiter") {
        if js::truthy(d) {
            token.extra.insert("successor_prefix".into(), d.clone());
        }
    }
    let number = new_numeric_blob(state, None, NumArg::Number(num), Some(token), Some(&id))?;
    set_formatter(state, number, NumFormatter::suffixator(None))?;
    queue::append(
        state,
        QueueId::Output,
        AppendArg::Blob(number),
        FormatRef::Name("literal".into()),
        false,
        false,
        false,
    )?;
    let mut firstoutput = false;
    // XXX Can we do something better for length here?
    for flags in &state.tmp.group_context.mystack {
        if !flags.variable_success
            && (flags.variable_attempt || (!flags.variable_attempt && !flags.term_intended))
        {
            firstoutput = true;
            break;
        }
    }
    let specialdelimiter = area_ref(state, &tmp_area)?
        .opt
        .get("year-suffix-delimiter")
        .cloned();
    if let Some(sd) = specialdelimiter.filter(|d| js::truthy(d)) {
        if firstoutput && !state.tmp.sort_key_flag {
            state.splice_delimiter = Some(js::to_js_string(&sd));
        }
    }
    Ok(())
}

/// The `citation-label` closure (node_text.js:196-215).
fn citation_label(state: &mut State, token: &mut Token, item: &Value) -> CslResult<()> {
    let mut label: String;
    if js::truthy_opt(item.get("citation-label")) {
        label = item
            .get("citation-label")
            .map(js::to_js_string)
            .unwrap_or_default();
    } else {
        // PORT-LATER(w2-engine): `state.getCitationLabel(Item)`
        // (util_citationlabel.js).
        return Err(EngineError::NotYetPorted {
            method: "state.getCitationLabel",
        });
    }
    if !state.tmp.just_looking {
        let mut suffix = String::new();
        let ys = registry_year_suffix(state, &item_id(item))?;
        if let Some(ys) = ys.filter(|v| *v != Value::Bool(false)) {
            let num = js::parse_int_value(&ys)
                .ok_or_else(|| EngineError::BadInput("year_suffix is not a number".into()))?;
            suffix = state.fun.suffixator.format(num);
        }
        label.push_str(&suffix);
    }
    append_text(state, &label, token)
}

/// The `printterm` closure (node_text.js:222-275).
fn print_term(
    state: &mut State,
    token: &mut Token,
    item: &Value,
    form: &str,
    plural: &Value,
) -> CslResult<()> {
    let gender: Option<String> = item
        .get("type")
        .and_then(Value::as_str)
        .and_then(|t| state.opt.get("gender").and_then(|g| g.get(t)))
        .filter(|g| js::truthy(g))
        .map(js::to_js_string);
    let term_name = token.string("term");
    let default_locale = js::truthy_opt(token.extra.get("default_locale"))
        || js::truthy_opt(token.strings.get("default_locale"));
    let plural_n = match plural {
        Value::Number(n) => n.as_i64(),
        _ => None,
    };
    let term = state
        .get_term(
            &term_name,
            Some(form),
            plural_n,
            gender.as_deref(),
            Some(TOLERANT),
            default_locale,
        )?
        .unwrap_or_default();
    // if the term is not an empty string, say that we rendered a term
    if !term.is_empty() {
        node_group::tip_mut(state)?.term_intended = true;
    }
    update_group_context_condition(state, Some(&term), false, Some(token), None);

    // capitalize the first letter of a term, if it is the first thing
    // rendered in a citation (or if it is being rendered immediately after
    // terminal punctuation, I guess, actually).
    let mut myterm = if !state.tmp.term_predecessor
        && !(js::get_str(&state.opt, "class") == Some("in-text") && state.tmp.area == "citation")
    {
        capitalize_first(state, &term)
    } else {
        term
    };

    // XXXXX Cut-and-paste code in multiple locations. This code block should be
    // collected in a function.
    // Tag: strip-periods-block
    if state.tmp.strip_periods != 0 {
        myterm = myterm.replace('.', "");
    } else {
        for d in &token.decorations {
            if d.name == "@strip-periods" && d.value == "true" {
                myterm = myterm.replace('.', "");
                break;
            }
        }
    }
    append_text(state, &myterm, token)?;
    if state.tmp.can_block_substitute {
        // Black magic here. This causes the cs:substitution condition to pass,
        // blocking further rendering within its scope.
        state
            .tmp
            .can_substitute
            .replace_literal(Value::Bool(false))?;
    }
    Ok(())
}

/// The closure that precedes a variable's renderer (node_text.js:281-299).
fn variable_flags(state: &mut State, token: &mut Token, item: &Value) -> CslResult<()> {
    // If some text variable is rendered, we're not collapsing.
    if variables_real(token).first().map(String::as_str) != Some("locator") {
        state.tmp.render.have_collapsed = false;
    }
    let key = token
        .variables
        .first()
        .cloned()
        .unwrap_or_else(|| "undefined".to_string());
    let has_condition = node_group::tip(state).condition.is_some();
    if !has_condition && js::truthy_opt(item.get(key.as_str())) {
        state.tmp.just_did_number = false;
    }
    let val = item.get(key.as_str());
    if js::truthy_opt(val) && !has_condition {
        let s = val.map(js::to_js_string).unwrap_or_default();
        state.tmp.just_did_number = js::slice(&s, -1, None)
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false);
    }
    Ok(())
}

static URL_SHORT_RE: LazyLock<Regex> = LazyLock::new(|| {
    // /(.*\.[^\/]+)\/.*/
    #[allow(clippy::expect_used)]
    Regex::new(&format!(r"({DOT}*\.[^/]+)/{DOT}*")).expect("static regex")
});

static HTTP_RE: LazyLock<Regex> = LazyLock::new(|| {
    // /https?:\/\//
    #[allow(clippy::expect_used)]
    Regex::new("https?://").expect("static regex")
});

/// The `URL` / `DOI` closure (node_text.js:372-450).
fn url_or_doi(state: &mut State, token: &mut Token, item: &Value, form: &str) -> CslResult<()> {
    let Some(v0) = token.variables.first().cloned().filter(|v| !v.is_empty()) else {
        return Ok(());
    };
    let value = get_variable(state, item, &v0, form)?;
    if !js::truthy_opt(value.as_ref()) {
        return Ok(());
    }
    let mut value = js_value_string(value);
    if v0 == "URL" && form == "short" {
        value = URL_SHORT_RE.replace(&value, "${1}").into_owned();
        if value.contains("//www.") {
            value = HTTP_RE.replace(&value, "").into_owned();
        }
    }
    // true is for non-suppression of periods
    if dev_ext_truthy(state, "wrap_url_and_doi") {
        let leading_decoration_is_var = token
            .decorations
            .first()
            .map(|d| d.name == format!("@{v0}"))
            .unwrap_or(false);
        if token.decorations.is_empty() || !leading_decoration_is_var {
            // Special-casing to fix https://github.com/Juris-M/citeproc-js/issues/57
            // clone current token, to avoid collateral damage
            let mut clonetoken = token.clone_token();
            // cast a group blob
            let mut groupblob = Blob::new(None, None, Some("url-wrapper"));
            // set the DOI decoration on the blob
            groupblob.decorations.push(Decoration::new("@DOI", "true"));
            let group_id = state.blobs.add(groupblob);
            if variables_real(token).first().map(String::as_str) == Some("DOI") {
                // strip a proper DOI prefix
                let mut prefix: Option<String> = None;
                let tprefix = token.string("prefix");
                // /^.*https:\/\/doi\.org\/$/
                let prefix_matches = tprefix
                    .strip_suffix("https://doi.org/")
                    .map(|head| !head.contains(['\n', '\r', '\u{2028}', '\u{2029}']))
                    .unwrap_or(false);
                if js::truthy_opt(token.strings.get("prefix")) && prefix_matches {
                    value = strip_doi_org(&value);
                    if value.starts_with("http://") || value.starts_with("https://") {
                        // Do not tamper with another protocol + domain if already set in field value
                        prefix = Some(String::new());
                    } else {
                        // Otherwise https + domain
                        prefix = Some("https://doi.org/".to_string());
                    }
                    // set any string prefix on the clone
                    let keep = js::len(&clonetoken.string("prefix")) as i64 - 16;
                    clonetoken.set_string("prefix", &js::slice(&tprefix, 0, Some(keep)));
                }
                // cast a text blob
                // set the prefix as the content of the blob
                let prefixblob = state.blobs.add(Blob::new(prefix.as_deref(), None, None));
                // cast another text blob
                // set the value as the content of the second blob
                let valueblob = state.blobs.add(Blob::new(Some(&value), None, None));
                // append new text token and clone to group token
                state.blobs.push(group_id, prefixblob)?;
                state.blobs.push(group_id, valueblob)?;
                // append group token to output
                queue::append(
                    state,
                    QueueId::Output,
                    AppendArg::Blob(group_id),
                    FormatRef::Token(clonetoken),
                    false,
                    false,
                    true,
                )?;
            } else {
                let valueblob = state.blobs.add(Blob::new(Some(&value), None, None));
                // append new text token and clone to group token
                state.blobs.push(group_id, valueblob)?;
                // append group token to output
                queue::append(
                    state,
                    QueueId::Output,
                    AppendArg::Blob(group_id),
                    FormatRef::Token(clonetoken),
                    false,
                    false,
                    true,
                )?;
            }
        } else {
            queue::append(
                state,
                QueueId::Output,
                AppendArg::Text(value),
                FormatRef::Token(token.clone()),
                false,
                false,
                true,
            )?;
        }
    } else {
        // This is totally unnecessary, isn't it?
        if !token.decorations.is_empty() {
            let name = format!("@{v0}");
            let mut i = token.decorations.len() as i64 - 1;
            while i > -1 {
                if token.decorations[i as usize].name == name {
                    token.decorations.remove(i as usize);
                }
                i -= 1;
            }
        }
        queue::append(
            state,
            QueueId::Output,
            AppendArg::Text(value),
            FormatRef::Token(token.clone()),
            false,
            false,
            true,
        )?;
    }
    Ok(())
}

/// `value.replace(/^https?:\/\/doi\.org\//, "")`.
fn strip_doi_org(value: &str) -> String {
    for p in ["https://doi.org/", "http://doi.org/"] {
        if let Some(rest) = value.strip_prefix(p) {
            return rest.to_string();
        }
    }
    value.to_string()
}

fn push(token: &mut Token, e: NodeTextExec) {
    token.execs.push(Exec::NodeText(e));
}

fn variables_real(token: &Token) -> Vec<String> {
    token
        .extra
        .get("variables_real")
        .and_then(Value::as_array)
        .map(|a| a.iter().map(js::to_js_string).collect())
        .unwrap_or_default()
}

/// `CSL.Node.text.build.call(token, state, target)`.
pub fn build(
    state: &mut State,
    mut token: Token,
    target: &mut Vec<Token>,
    _real_group: Option<bool>,
) -> CslResult<()> {
    if let Some(macro_name) = token.postponed_macro.clone() {
        let mut group_start = token.clone_token();
        group_start.name = "group".to_string();
        group_start.tokentype = TokenType::Start;
        node_group::build(state, group_start, target, None)?;

        state.expand_macro(&token, target)?;

        let mut group_end = token.clone_token();
        group_end.name = "group".to_string();
        group_end.tokentype = TokenType::End;
        if macro_name == "juris-locator-label" {
            group_end
                .extra
                .insert("isJurisLocatorLabel".into(), Value::Bool(true));
        }
        node_group::build(state, group_end, target, None)?;
        return Ok(());
    }

    util_substitute::substitute_start(state, &mut token, target)?;
    // ...
    //
    // Do non-macro stuff

    // Guess again. this.variables is ephemeral, adjusted by an initial
    // function set on the node via @variable attribute setup.
    if !token.extra.contains_key("variables_real") {
        token
            .extra
            .insert("variables_real".into(), Value::Array(Vec::new()));
    }
    // (`if (!this.variables) this.variables = []` is a no-op for a Vec.)

    let mut form = "long".to_string();
    let mut plural = Value::from(0);
    if js::truthy_opt(token.strings.get("form")) {
        form = token.string("form");
    }
    if js::truthy_opt(token.strings.get("plural")) {
        plural = token.strings.get("plural").cloned().unwrap_or(Value::Null);
    }
    let vreal = variables_real(&token);
    let v0 = vreal.first().cloned();
    let area_opt = |state: &State, name: &str, key: &str| -> CslResult<Option<Value>> {
        Ok(area_ref(state, name)?.opt.get(key).cloned())
    };
    if matches!(
        v0.as_deref(),
        Some("citation-number") | Some("year-suffix") | Some("citation-label")
    ) {
        //
        // citation-number and year-suffix are super special,
        // because they are rangeables, and require a completely
        // different set of formatting parameters on the output
        // queue.
        if v0.as_deref() == Some("citation-number") {
            if state.build.root == "citation" {
                state.opt.insert("update_mode".into(), Value::from(NUMERIC));
            }
            if state.build.root == "bibliography" {
                state.opt.insert("bib_mode".into(), Value::from(NUMERIC));
            }
            let tmp_area = state.tmp.area.clone();
            if area_opt(state, &tmp_area, "collapse")?
                .and_then(|v| v.as_str().map(str::to_string))
                .as_deref()
                == Some("citation-number")
            {
                let t =
                    state.get_term("citation-range-delimiter", None, None, None, None, false)?;
                token.extra.insert(
                    "range_prefix".into(),
                    t.map(Value::String).unwrap_or(Value::Null),
                );
            }
            let ld = area_opt(state, &state.build.area.clone(), "layout_delimiter")?;
            set_opt_extra(&mut token, "successor_prefix", ld.clone());
            set_opt_extra(&mut token, "splice_prefix", ld);
            push(&mut token, NodeTextExec::CitationNumber);
        } else if v0.as_deref() == Some("year-suffix") {
            state
                .opt
                .insert("has_year_suffix".into(), Value::Bool(true));

            let tmp_area = state.tmp.area.clone();
            if area_opt(state, &tmp_area, "collapse")?
                .and_then(|v| v.as_str().map(str::to_string))
                .as_deref()
                == Some("year-suffix-ranged")
            {
                let t =
                    state.get_term("citation-range-delimiter", None, None, None, None, false)?;
                token.extra.insert(
                    "range_prefix".into(),
                    t.map(Value::String).unwrap_or(Value::Null),
                );
            }
            let build_area = state.build.area.clone();
            let ld = area_opt(state, &build_area, "layout_delimiter")?;
            set_opt_extra(&mut token, "successor_prefix", ld);
            if let Some(d) = area_opt(state, &tmp_area, "year-suffix-delimiter")? {
                if js::truthy(&d) {
                    // JS reads the delimiter from state[state.build.area].opt here.
                    let d2 = area_opt(state, &build_area, "year-suffix-delimiter")?;
                    set_opt_extra(&mut token, "successor_prefix", d2);
                }
            }
            push(&mut token, NodeTextExec::YearSuffix);
        } else {
            // citation-label
            if state.build.root == "bibliography" {
                state.opt.insert("bib_mode".into(), Value::from(TRIGRAPH));
            }
            state
                .opt
                .insert("has_year_suffix".into(), Value::Bool(true));
            push(&mut token, NodeTextExec::CitationLabel);
        }
    } else if js::truthy_opt(token.strings.get("term")) {
        // printterm
        push(
            &mut token,
            NodeTextExec::PrintTerm {
                form: form.clone(),
                plural,
            },
        );
        state.build.term = Value::Bool(false);
        state.build.form = Value::Bool(false);
        state.build.plural = Value::Bool(false);
    } else if !vreal.is_empty() {
        push(&mut token, NodeTextExec::VariableFlags);

        // plain string fields

        // Deal with multi-fields and ordinary fields separately.
        let v = vreal[0].as_str();
        let func = if MULTI_FIELDS.contains(&v)
            || v.contains("-main")
            || v.contains("-sub")
            || ["language-name", "language-name-original"].contains(&v)
        {
            // multi-fields
            // Initialize transform factory according to whether
            // abbreviation is desired.
            let mut abbrevfam: Option<String> = token.variables.first().cloned();
            let mut altvar: Option<String> = None;
            if form == "short" {
                if js::slice(v, -6, None) != "-short" {
                    altvar = Some(format!("{v}-short"));
                }
            } else {
                abbrevfam = None;
            }
            // multi-fields for sorting get a sort transform,
            // (abbreviated if the short form was selected)
            let transfall = true;
            let abbrfall = state.build.extension.is_empty();
            NodeTextExec::TransformOutput {
                variables: token.variables.clone(),
                abbrevfam,
                abbrfall,
                altvar,
                transfall,
            }
        } else if CITE_FIELDS.contains(&v) {
            // per-cite fields are read from item, rather than Item
            NodeTextExec::CiteField
        } else if [
            "page",
            "page-first",
            "chapter-number",
            "collection-number",
            "edition",
            "issue",
            "number",
            "number-of-pages",
            "number-of-volumes",
            "volume",
        ]
        .contains(&v)
        {
            // page gets mangled with the correct collapsing algorithm
            NodeTextExec::NumberField
        } else if v == "URL" || v == "DOI" {
            NodeTextExec::UrlOrDoi { form: form.clone() }
        } else if v == "section" {
            // Sections for statutes are special.
            NodeTextExec::Section { form: form.clone() }
        } else if v == "hereinafter" {
            NodeTextExec::Hereinafter
        } else {
            // anything left over just gets output in the normal way.
            NodeTextExec::PlainVariable { form: form.clone() }
        };
        push(&mut token, func);
    } else if js::truthy_opt(token.strings.get("value")) {
        // for the text value attribute.
        push(&mut token, NodeTextExec::Value);
        // otherwise no output
    }
    // target.push(this); CSL.Util.substituteEnd.call(this, state, target);
    util_substitute::push_with_substitute_end(state, token, target, true)
}

fn set_opt_extra(token: &mut Token, key: &str, v: Option<Value>) {
    match v {
        Some(v) => {
            token.extra.insert(key.to_string(), v);
        }
        None => {
            token.extra.remove(key);
        }
    }
}
