// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/node_key.js
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

//! Port of `src/node_key.js`: `CSL.Node.key` (`cs:key` inside `cs:sort`).
//!
//! A `cs:key` compiles to a `key` START token (reset `done_vars`, open an
//! output level, copy the key's et-al parameters), the tokens that render the
//! sort value (a names block, a variable renderer, a date key, or an expanded
//! macro), and a `key` END token that stores the rendered string in the
//! area's `keys`. All of them go into the `_sort` area's token list.

use serde_json::Value;

use super::attributes;
use super::load::{self, ASCENDING, DATE_VARIABLES, DESCENDING, NAME_VARIABLES, NUMERIC_VARIABLES};
use super::exec::Exec;
use super::js;
use super::node_institution;
use super::node_name;
use super::node_names;
use super::node_sort::with_sort_target;
use super::obj_token::{Token, TokenType};
use super::queue::{self, AppendArg, FormatRef, QueueId, Rendered, StringParent};
use super::state::State;
use super::util_number::padding;
use super::{CslResult, EngineError};

/// The closures `src/node_key.js` stores in `token.execs` (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeKeyExec {
    /// `key` START: `state.tmp.done_vars = []` (node_key.js:18-20).
    InitDoneVars,
    /// `key` START: `state.output.openLevel("empty")` (node_key.js:24-26).
    OpenLevelEmpty,
    /// `key` START, "et al init": copy the key's et-al parameters into
    /// `state.tmp` (node_key.js:50-62).
    EtAlInit,
    /// Text token for `citation-number` (node_key.js:98-117).
    CitationNumber,
    /// Text token for any other numeric variable (node_key.js:119-128).
    NumericVariable {
        /// The sort variable.
        variable: String,
    },
    /// Text token for `citation-label` (node_key.js:131-134).
    CitationLabel,
    /// Text token for a date variable: `CSL.dateAsSortKey` (node_key.js:137).
    DateAsSortKey,
    /// Text token for `title`: `state.transform.getOutputFunction(...)`
    /// (node_key.js:139-145).
    TitleTransform {
        /// `this.variables` at build time.
        variables: Vec<String>,
        /// `abbrevfam` (`"title"`).
        abbrevfam: String,
    },
    /// Text token for `court-class` (node_key.js:147-153).
    CourtClass,
    /// Text token for any other variable (node_key.js:155-160).
    PlainVariable {
        /// The sort variable.
        variable: String,
    },
    /// `key` END: render the queue to a key string and store it
    /// (node_key.js:187-211).
    StoreKey,
    /// `key` END: the year-suffix sort key (node_key.js:216-224).
    YearSuffixKey,
    /// `key` END: reset the key parameters (node_key.js:230-240).
    ResetKeyParams,
}

impl NodeKeyExec {
    /// Run the closure.
    pub fn run(
        &self,
        state: &mut State,
        token: &mut Token,
        item: &Value,
        cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            NodeKeyExec::InitDoneVars => {
                state.tmp.done_vars = Vec::new();
                Ok(None)
            }
            NodeKeyExec::OpenLevelEmpty => {
                queue::open_level(state, QueueId::Output, FormatRef::Name("empty".to_string()))?;
                Ok(None)
            }
            NodeKeyExec::EtAlInit => {
                state.tmp.sort_key_flag = true;
                if let Some(v) = state.inherit_opt(token, "et-al-min", None, None) {
                    if js::truthy(&v) {
                        state.tmp.et_al_min = Some(v);
                    }
                }
                if let Some(v) = state.inherit_opt(token, "et-al-use-first", None, None) {
                    if js::truthy(&v) {
                        state.tmp.et_al_use_first = Some(v);
                    }
                }
                if let Some(v) = state.inherit_opt(token, "et-al-use-last", None, None) {
                    if v.is_boolean() {
                        state.tmp.et_al_use_last = Some(v);
                    }
                }
                Ok(None)
            }
            NodeKeyExec::CitationNumber => {
                if state.tmp.area == "bibliography_sort" {
                    let descending = token.strings.get("sort_direction").and_then(Value::as_i64)
                        == Some(DESCENDING);
                    state.bibliography_sort.opt.insert(
                        "citation_number_sort_direction".into(),
                        Value::from(if descending { DESCENDING } else { ASCENDING }),
                    );
                }
                let id = super::registry::id_key(item.get("id"));
                let seq = state
                    .registry
                    .registry
                    .get(&id)
                    .map(|t| t.seq)
                    .ok_or_else(|| {
                        EngineError::Csl(
                            "TypeError: Cannot read properties of undefined (reading 'seq')".into(),
                        )
                    })?;
                let num: Option<Value> = if state.tmp.area == "citation_sort"
                    && js::truthy_opt(state.bibliography_sort.tmp.get("citation_number_map"))
                {
                    state
                        .bibliography_sort
                        .tmp
                        .get("citation_number_map")
                        .and_then(|m| m.get(seq.to_string()))
                        .cloned()
                } else {
                    Some(Value::from(seq))
                };
                // Code currently in util_number.js
                let arg = match num {
                    Some(n) if js::truthy(&n) => AppendArg::Text(padding(&js::to_js_string(&n))),
                    Some(Value::Null) | None => AppendArg::Undefined,
                    Some(other) => AppendArg::Text(js::to_js_string(&other)),
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
                Ok(None)
            }
            NodeKeyExec::NumericVariable { variable } => {
                let num = item.get(variable.as_str()).cloned();
                // XXX What if this is NaN?
                let arg = match num {
                    Some(Value::String(s)) if !s.is_empty() => AppendArg::Text(padding(&s)),
                    Some(Value::String(s)) => AppendArg::Text(s),
                    Some(v) if js::truthy(&v) => {
                        // `CSL.Util.padding(num)` calls `num.match(...)`.
                        return Err(EngineError::Csl(
                            "TypeError: num.match is not a function".to_string(),
                        ));
                    }
                    Some(Value::Number(_)) => AppendArg::Number(0.0),
                    Some(_) | None => AppendArg::Undefined,
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
                Ok(None)
            }
            NodeKeyExec::CitationLabel => {
                let trigraph = state.get_citation_label(item)?;
                queue::append(
                    state,
                    QueueId::Output,
                    AppendArg::Text(trigraph),
                    FormatRef::Token(token.clone()),
                    false,
                    false,
                    false,
                )?;
                Ok(None)
            }
            // node_key.js:137: `func = CSL.dateAsSortKey` (util_date.js).
            NodeKeyExec::DateAsSortKey => {
                super::util_date::date_as_sort_key(state, token, item, false)?;
                Ok(None)
            }
            // PORT-LATER(w2-render): node_key.js:139-145, needs
            // state.transform.getOutputFunction (util_transform.js).
            NodeKeyExec::TitleTransform { .. } => Err(EngineError::NotYetPorted {
                method: "node_key.js:145 state.transform.getOutputFunction",
            }),
            NodeKeyExec::CourtClass => {
                let mut full = item.clone();
                load::init_jurisdiction_macros(state, &mut full, Some(cite_item), "juris-main")?;
                // true is for sortKey mode
                let cls = load::get_court_class(state, None, item, true);
                queue::append_simple(state, QueueId::Output, cls.as_str(), "empty")?;
                Ok(None)
            }
            NodeKeyExec::PlainVariable { variable } => {
                let arg = match item.get(variable.as_str()) {
                    None | Some(Value::Null) => AppendArg::Undefined,
                    Some(Value::String(s)) => AppendArg::Text(s.clone()),
                    Some(Value::Number(n)) => AppendArg::Number(n.as_f64().unwrap_or(f64::NAN)),
                    Some(other) => AppendArg::Text(js::to_js_string(other)),
                };
                queue::append(
                    state,
                    QueueId::Output,
                    arg,
                    FormatRef::Name("empty".to_string()),
                    false,
                    false,
                    false,
                )?;
                Ok(None)
            }
            NodeKeyExec::StoreKey => {
                let children = queue::queue_children(state, QueueId::Output);
                let rendered =
                    queue::string(state, QueueId::Output, &children, StringParent::None)?;
                // (`state.sys.normalizeUnicode` is a host hook the port does not model.)
                let keystring = match rendered {
                    Rendered::Str(s) => s,
                    // `keystring.split` of an array is a TypeError upstream.
                    _ => {
                        return Err(EngineError::Csl(
                            "TypeError: keystring.split is not a function".to_string(),
                        ))
                    }
                };
                let sort_sep = js::get_string(&state.opt, "sort_sep").unwrap_or_default();
                let keystring = if keystring.is_empty() {
                    Value::Null
                } else {
                    Value::String(format!("{}{}", keystring.replace(' ', &sort_sep), sort_sep))
                };
                let root = state.area_ref(&state.tmp.area.clone()).root.clone();
                state.area_mut(&format!("{root}_sort")).keys.push(keystring);
                state.tmp.value = Vec::new();
                Ok(None)
            }
            NodeKeyExec::YearSuffixKey => {
                // year-suffix Key
                let id = super::registry::id_key(item.get("id"));
                let disambig = state
                    .registry
                    .registry
                    .get(&id)
                    .and_then(|t| t.disambig)
                    .ok_or_else(|| {
                        EngineError::Csl(
                            "TypeError: Cannot read properties of undefined (reading 'disambig')"
                                .into(),
                        )
                    })?;
                let mut year_suffix = state.ambig(disambig).year_suffix.clone();
                if !js::truthy(&year_suffix) {
                    year_suffix = Value::from(0);
                }
                let key = padding(&js::to_js_string(&year_suffix));
                let area = state.tmp.area.clone();
                state.area_mut(&area).keys.push(Value::String(key));
                Ok(None)
            }
            NodeKeyExec::ResetKeyParams => {
                state.tmp.et_al_min = None;
                state.tmp.et_al_use_first = None;
                state.tmp.et_al_use_last = None;
                state.tmp.sort_key_flag = false;
                Ok(None)
            }
        }
    }
}

/// `CSL.Node.key.build.call(token, state, target)`. The incoming `target` is
/// ignored: upstream reassigns it to the `_sort` area's token list.
pub fn build(
    state: &mut State,
    token: Token,
    _target: &mut Vec<Token>,
    _real_group: Option<bool>,
) -> CslResult<()> {
    with_sort_target(state, |state, target| build_into(state, &token, target))
}

fn opt_value(v: Option<Value>, strings: &mut super::js::Obj, key: &str) {
    if let Some(v) = v {
        strings.insert(key.to_string(), v);
    }
}

fn build_into(state: &mut State, this: &Token, target: &mut Vec<Token>) -> CslResult<()> {
    let mut start_key = Token::new("key", TokenType::Start);

    state.tmp.root = state.build.root.clone();

    // The params object for build and runtime (tmp) really shouldn't have been separated.
    // Oh, well.
    for k in ["et-al-min", "et-al-use-first", "et-al-use-last"] {
        let v = state.inherit_opt(this, k, None, None);
        opt_value(v, &mut start_key.strings, k);
    }

    // initialize done vars
    start_key
        .execs
        .push(Exec::NodeKey(NodeKeyExec::InitDoneVars));

    // initialize output queue
    start_key
        .execs
        .push(Exec::NodeKey(NodeKeyExec::OpenLevelEmpty));

    // sort direction
    let sort_direction: Vec<Value> =
        if this.strings.get("sort_direction").and_then(Value::as_i64) == Some(DESCENDING) {
            vec![Value::from(1), Value::from(-1)]
        } else {
            vec![Value::from(-1), Value::from(1)]
        };
    let area = state.build.area.clone();
    attributes::arr_entry(
        &mut attributes::area_mut(state, &area)?.opt,
        "sort_directions",
    )
    .push(Value::Array(sort_direction));

    if this
        .variables
        .first()
        .map(|v| DATE_VARIABLES.contains(&v.as_str()))
        .unwrap_or(false)
    {
        state.build.date_key = true;
    }

    // et al init
    start_key.execs.push(Exec::NodeKey(NodeKeyExec::EtAlInit));
    target.push(start_key);

    //
    // ops to initialize the key's output structures
    if !this.variables.is_empty() {
        let variable = this.variables[0].clone();
        if NAME_VARIABLES.contains(&variable.as_str()) {
            //
            // Start tag
            let mut names_start_token = Token::new("names", TokenType::Start);
            names_start_token.variables = this.variables.clone();
            node_names::build(state, names_start_token, target, None)?;
            //
            // Name tag
            let mut name_token = Token::new("name", TokenType::Singleton);
            name_token.set_string("name-as-sort-order", "all");
            name_token.set_string("sort-separator", " ");
            for k in ["et-al-use-last", "et-al-min", "et-al-use-first"] {
                let v = state.inherit_opt(this, k, None, None);
                opt_value(v, &mut name_token.strings, k);
            }
            node_name::build(state, name_token, target, None)?;
            //
            // Institution tag
            let institution_token = Token::new("institution", TokenType::Singleton);
            node_institution::build(state, institution_token, target, None)?;
            //
            // End tag
            let names_end_token = Token::new("names", TokenType::End);
            node_names::build(state, names_end_token, target, None)?;
        } else {
            let mut single_text = Token::new("text", TokenType::Singleton);
            if let Some(sd) = this.strings.get("sort_direction") {
                single_text
                    .strings
                    .insert("sort_direction".into(), sd.clone());
            }
            if let Some(dp) = this.extra.get("dateparts") {
                single_text.extra.insert("dateparts".into(), dp.clone());
            }
            let func = if NUMERIC_VARIABLES.contains(&variable.as_str()) {
                // citation-number is virtualized. As a sort key it has no effect on
                // registry sort order per se, but if set to DESCENDING, it reverses
                // the sequence of numbers representing bib entries.
                if variable == "citation-number" {
                    NodeKeyExec::CitationNumber
                } else {
                    NodeKeyExec::NumericVariable { variable }
                }
            } else if variable == "citation-label" {
                NodeKeyExec::CitationLabel
            } else if DATE_VARIABLES.contains(&variable.as_str()) {
                single_text.variables = this.variables.clone();
                NodeKeyExec::DateAsSortKey
            } else if variable == "title" {
                NodeKeyExec::TitleTransform {
                    variables: this.variables.clone(),
                    abbrevfam: "title".to_string(),
                }
            } else if variable == "court-class" {
                NodeKeyExec::CourtClass
            } else {
                NodeKeyExec::PlainVariable { variable }
            };
            single_text.execs.push(Exec::NodeKey(func));
            target.push(single_text);
        }
    } else {
        // macro
        //
        // if it's not a variable, it's a macro
        let mut token = Token::new("text", TokenType::Singleton);
        if let Some(sd) = this.strings.get("sort_direction") {
            token.strings.insert("sort_direction".into(), sd.clone());
        }
        token.postponed_macro = this.postponed_macro.clone();
        state.expand_macro(&token, target)?;
    }
    //
    // ops to output the key string result to an array go
    // on the closing "key" tag before it is pushed.
    // Do not close the level.
    let mut end_key = Token::new("key", TokenType::End);

    // store key for use
    end_key.execs.push(Exec::NodeKey(NodeKeyExec::StoreKey));

    // Set year-suffix key on anything that looks like a date
    if state.build.date_key {
        if state.build.area == "citation" && state.build.extension == "_sort" {
            // ascending sort always
            let area = state.build.area.clone();
            attributes::arr_entry(
                &mut attributes::area_mut(state, &area)?.opt,
                "sort_directions",
            )
            .push(Value::Array(vec![Value::from(-1), Value::from(1)]));
            end_key
                .execs
                .push(Exec::NodeKey(NodeKeyExec::YearSuffixKey));
        }
        state.build.date_key = false;
    }

    // reset key params
    end_key
        .execs
        .push(Exec::NodeKey(NodeKeyExec::ResetKeyParams));
    target.push(end_key);
    Ok(())
}
