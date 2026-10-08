// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/node_number.js
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

//! Port of `src/node_number.js`: `CSL.Node.number`.

use serde_json::Value;

use super::exec::Exec;
use super::js;
use super::node_group;
use super::obj_token::Token;
use super::queue::{self, AppendArg, FormatRef, QueueId};
use super::state::State;
use super::util_number::{output_numeric_field, process_number};
use super::util_substitute;
use super::{CslResult, EngineError};

use super::attributes::area_ref;

/// The closures `src/node_number.js` stores in `token.execs` (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeNumberExec {
    /// The one closure of `cs:number`: push a number or text to the output
    /// (node_number.js:41-130).
    Render,
}

impl NodeNumberExec {
    /// Run the closure.
    pub fn run(
        &self,
        state: &mut State,
        token: &mut Token,
        item: &Value,
        cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            NodeNumberExec::Render => {
                // NOTE: this works because this is the ONLY function in this
                // node. If further functions are added, they need to start
                // with the same abort condition.
                let Some(varname) = token.variables.first().cloned() else {
                    return Ok(None);
                };
                // `if ("undefined" === typeof item) var item = {}`
                let empty = Value::Object(js::Obj::new());
                let cite = if cite_item.is_null() { &empty } else { cite_item };
                let is_locator = varname == "locator" || varname == "locator-extra";
                if is_locator {
                    if state.tmp.just_looking {
                        return Ok(None);
                    }
                    if !js::truthy_opt(cite.get(varname.as_str())) {
                        return Ok(None);
                    }
                } else if !js::truthy_opt(item.get(varname.as_str())) {
                    return Ok(None);
                }

                if varname == "collection-number"
                    && item.get("type").and_then(Value::as_str) == Some("legal_case")
                {
                    state.tmp.render.renders_collection_number = true;
                }

                // For bill or legislation items that have a label-form
                // attribute set on the cs:number node rendering the locator,
                // the form and pluralism of locator terms are controlled
                // separately from those of the initial label. Form is
                // straightforward: the label uses the value set on the
                // cs:label node that renders it, and the embedded labels use
                // the value of label-form set on the cs:number node. Both
                // default to "long".
                //
                // Pluralism is more complicated. For embedded labels,
                // pluralism is evaluated using a simple heuristic that can be
                // found below (it just looks for comma, ampersand etc). The
                // item.label rendered independently via cs:label defaults to
                // singular. It is always singular if embedded labels exist
                // that (when expanded to their valid CSL value) do not match
                // the value of item.label. Otherwise, if one or more matching
                // embedded labels exist, the cs:label is set to plural.
                //
                // The code that does all this is divided between this module,
                // util_static_locator.js, and util_label.js.

                if node_group::tip(state).force_suppress {
                    return Ok(None);
                }

                let node = token.clone();
                if is_locator {
                    // amazing that we reach this. should abort sooner if no content?
                    process_number(state, Some(&node), Some(cite), &varname)?;
                } else {
                    if node_group::tip(state).condition.is_none() {
                        let v = item
                            .get(varname.as_str())
                            .map(js::to_js_string)
                            .unwrap_or_default();
                        if js::truthy_opt(item.get(varname.as_str())) {
                            state.tmp.just_did_number = js::slice(&v, -1, None)
                                .chars()
                                .next()
                                .map(|c| c.is_ascii_digit())
                                .unwrap_or(false);
                        }
                    }
                    // UPDATE_GROUP_CONTEXT_CONDITION is run by processNumber
                    process_number(state, Some(&node), Some(item), &varname)?;
                }

                let substring = token
                    .extra
                    .get("substring")
                    .and_then(Value::as_i64)
                    .filter(|n| *n != 0);
                if let Some(n) = substring {
                    let val = match item.get(varname.as_str()) {
                        Some(Value::String(s)) => js::slice(s, n, None),
                        _ => {
                            return Err(EngineError::BadInput(
                                "Item[varname].slice is not a function".into(),
                            ))
                        }
                    };
                    queue::append(
                        state,
                        QueueId::Output,
                        AppendArg::Text(val),
                        FormatRef::Token(node),
                        false,
                        false,
                        false,
                    )?;
                } else {
                    let id = item
                        .get("id")
                        .map(js::to_js_string)
                        .unwrap_or_else(|| "undefined".to_string());
                    output_numeric_field(state, &varname, &id)?;
                }

                let real0 = token
                    .extra
                    .get("variables_real")
                    .and_then(Value::as_array)
                    .and_then(|a| a.first())
                    .map(js::to_js_string)
                    .unwrap_or_default();
                if (real0 == "locator" || real0 == "locator-extra") && !state.tmp.just_looking {
                    state.tmp.done_vars.push(real0.clone());
                    node_group::tip_mut(state)?.done_vars.push(real0);
                }
                Ok(None)
            }
        }
    }
}

/// `CSL.Node.number.build.call(token, state, target)`.
pub fn build(
    state: &mut State,
    mut token: Token,
    target: &mut Vec<Token>,
    _real_group: Option<bool>,
) -> CslResult<()> {
    util_substitute::substitute_start(state, &mut token, target)?;
    //
    // This should push a rangeable object to the queue.
    //
    // The formatter is a function upstream; the intermediate dump records its
    // name, so the name is the token property here.
    let formatter = match token.string_opt("form").as_deref() {
        Some("roman") => Some("romanizer"),
        Some("ordinal") => Some("ordinalizer"),
        Some("long-ordinal") => Some("long_ordinalizer"),
        _ => None,
    };
    if let Some(f) = formatter {
        token
            .extra
            .insert("formatter".into(), serde_json::Value::String(f.to_string()));
    }
    let layout_delimiter = area_ref(state, &state.build.area.clone())?
        .opt
        .get("layout_delimiter")
        .cloned();
    if !token.extra.contains_key("successor_prefix") {
        if let Some(d) = &layout_delimiter {
            token.extra.insert("successor_prefix".into(), d.clone());
        }
    }
    if !token.extra.contains_key("splice_prefix") {
        if let Some(d) = &layout_delimiter {
            token.extra.insert("splice_prefix".into(), d.clone());
        }
    }
    //
    // Whether we actually stick a number object on
    // the output queue depends on whether the field
    // contains a pure number.
    //
    // push number or text
    token.execs.push(Exec::NodeNumber(NodeNumberExec::Render));
    // target.push(this); CSL.Util.substituteEnd.call(this, state, target);
    util_substitute::push_with_substitute_end(state, token, target, true)
}
