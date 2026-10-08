// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/node_date.js
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

//! Port of `src/node_date.js`: `CSL.Node.date`.

use serde_json::Value;

use super::exec::Exec;
use super::js;
use super::obj_token::{Token, TokenType};
use super::state::State;
use super::util_substitute;
use super::{CslResult, EngineError};

/// The closures `src/node_date.js` stores in `token.execs` (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeDateExec {
    /// START / SINGLETON, when `state.build.extension` is empty: analyse the
    /// date and set `date_object`, `donesies`, `dateparts` and
    /// `date_collapse_at` (node_date.js:30-117).
    Analyse,
    /// START / SINGLETON, when sorting (`state.build.extension` set): the
    /// function `CSL.dateMacroAsSortKey` itself (node_date.js:21).
    DateMacroAsSortKey,
    /// START / SINGLETON "newoutput": open the date tag (node_date.js:121-151).
    OpenTag,
    /// END / SINGLETON "mergeoutput": close the date tag (node_date.js:155-162).
    CloseTag,
}

impl NodeDateExec {
    /// Run the closure.
    pub fn run(
        &self,
        state: &mut State,
        token: &mut Token,
        item: &Value,
        cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match self {
            NodeDateExec::Analyse => {
                state.tmp.element_rendered_ok = false;
                state.tmp.donesies = Vec::new();
                state.tmp.dateparts = Vec::new();
                let mut dp: Vec<String> = Vec::new();
                let first_var = token.variables.first().cloned();
                if let Some(var0) =
                    first_var.filter(|v| !(state.tmp.just_looking && v == "accessed"))
                {
                    let date_obj: Value = match item.get(var0.as_str()) {
                        Some(v) => v.clone(),
                        None => {
                            let mut d = serde_json::json!({"date-parts": [[0]]});
                            let dev = state
                                .opt
                                .get("development_extensions")
                                .map(|d| js::truthy_opt(d.get("locator_date_and_revision")))
                                .unwrap_or(false);
                            if dev
                                && js::truthy(cite_item)
                                && var0 == "locator-date"
                                && js::truthy_opt(cite_item.get("locator-date"))
                            {
                                if let Some(v) = cite_item.get("locator-date") {
                                    d = v.clone();
                                }
                            }
                            d
                        }
                    };
                    state.tmp.date_object = date_obj.clone();
                    //
                    // Call a function here to analyze the
                    // data and set the name of the date-part that
                    // should collapse for this range, if any.
                    //
                    // (1) build a filtered list, in y-m-d order,
                    // consisting only of items that are (a) in the
                    // date-parts and (b) in the *_end data.
                    // (note to self: remember that season is a
                    // fallback var when month and day are empty)
                    let dateparts: Vec<String> = token
                        .extra
                        .get("dateparts")
                        .and_then(Value::as_array)
                        .map(|a| a.iter().map(js::to_js_string).collect())
                        .unwrap_or_default();
                    let get = |k: &str| date_obj.get(k);
                    for part in &dateparts {
                        if get(&format!("{part}_end")).is_some() {
                            dp.push(part.clone());
                        } else if part == "month" && get("season_end").is_some() {
                            dp.push(part.clone());
                        }
                    }
                    let mut dpx: Vec<String> = Vec::new();
                    for part in ["year", "month", "day"] {
                        if dp.iter().any(|d| d == part) {
                            dpx.push(part.to_string());
                        }
                    }
                    dp = dpx;
                    //
                    // (2) Reverse the list and step through in
                    // reverse order, popping each item if the
                    // primary and *_end data match.
                    let mut mypos = 2usize;
                    for (pos, part) in dp.iter().enumerate() {
                        let start = get(part);
                        let end = get(&format!("{part}_end"));
                        // JS `start !== end` on JSON scalars.
                        if start != end {
                            mypos = pos;
                            break;
                        }
                    }
                    //
                    // (3) When finished, the first item in the
                    // list, if any, is the date-part where
                    // the collapse should occur.
                    state.tmp.date_collapse_at = dp.get(mypos..).unwrap_or(&[]).to_vec();
                } else {
                    state.tmp.date_object = Value::Bool(false);
                }
                Ok(None)
            }
            // PORT-LATER(wave1-input): node_date.js:21, `CSL.dateMacroAsSortKey`
            // (util_date.js) is not ported yet.
            NodeDateExec::DateMacroAsSortKey => Err(EngineError::NotYetPorted {
                method: "util_date.js CSL.dateMacroAsSortKey",
            }),
            // PORT-LATER(wave1-output): node_date.js:121-151, needs
            // state.output.startTag("date", this) and
            // state.output.current.mystack (queue.rs).
            NodeDateExec::OpenTag => Err(EngineError::NotYetPorted {
                method: "node_date.js:121 closure",
            }),
            // PORT-LATER(wave1-output): node_date.js:155-162, needs
            // state.output.endTag() (queue.rs).
            NodeDateExec::CloseTag => Err(EngineError::NotYetPorted {
                method: "node_date.js:155 closure",
            }),
        }
    }
}

/// `CSL.Node.date.build.call(token, state, target)`.
pub fn build(
    state: &mut State,
    mut token: Token,
    target: &mut Vec<Token>,
    _real_group: Option<bool>,
) -> CslResult<()> {
    if token.tokentype == TokenType::Start || token.tokentype == TokenType::Singleton {
        // used to collect rendered date part names in node_datepart,
        // for passing through to node_key, for use in dates embedded
        // in macros
        // PORT-LATER(wave1-output): node_date.js:12,17 need
        // state.dateput.string(state, state.dateput.queue) and
        // state.dateput.openLevel(this) (queue.rs); the dateput queue is not
        // touched yet.
        let mut date_token = token.clone_token();
        date_token.set_string("prefix", "");
        date_token.set_string("suffix", "");
        state.tmp.date_token = Some(date_token);
        state.build.date_parts = Vec::new();
        state.build.date_variables = token.variables.clone();
        if state.build.extension.is_empty() {
            util_substitute::substitute_start(state, &mut token, target)?;
        }
        if !state.build.extension.is_empty() {
            token
                .execs
                .push(Exec::NodeDate(NodeDateExec::DateMacroAsSortKey));
        } else {
            token.execs.push(Exec::NodeDate(NodeDateExec::Analyse));
        }

        // newoutput
        token.execs.push(Exec::NodeDate(NodeDateExec::OpenTag));
    }

    if state.build.extension.is_empty()
        && (token.tokentype == TokenType::End || token.tokentype == TokenType::Singleton)
    {
        // mergeoutput
        token.execs.push(Exec::NodeDate(NodeDateExec::CloseTag));
    }
    let end_or_singleton =
        token.tokentype == TokenType::End || token.tokentype == TokenType::Singleton;
    if end_or_singleton && state.build.extension.is_empty() {
        // target.push(this); CSL.Util.substituteEnd.call(this, state, target);
        util_substitute::push_with_substitute_end(state, token, target, true)
    } else {
        target.push(token);
        Ok(())
    }
}
