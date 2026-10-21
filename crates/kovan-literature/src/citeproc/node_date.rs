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
use super::queue::{self, FormatRef, QueueId, StringParent};
use super::state::State;
use super::util_substitute;
use super::CslResult;

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
                    let dateparts: Vec<String> = dateparts_of(token)?;
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
            // node_date.js:21: `CSL.dateMacroAsSortKey` (util_date.js).
            NodeDateExec::DateMacroAsSortKey => {
                super::util_date::date_macro_as_sort_key(state, token, item)?;
                Ok(None)
            }
            NodeDateExec::OpenTag => open_tag(state, token, item),
            NodeDateExec::CloseTag => close_tag(state, token, item),
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
        // The dateput queue is reset (rendering and clearing whatever the
        // previous date node left in it) and a level for this date opened
        // at BUILD time; the run-time closures of cs:date-part continue from
        // that state (node_datepart.js).
        let pending = queue::queue_children(state, QueueId::Dateput);
        queue::string(state, QueueId::Dateput, &pending, StringParent::None)?;
        let mut date_token = token.clone_token();
        date_token.set_string("prefix", "");
        date_token.set_string("suffix", "");
        state.tmp.date_token = Some(date_token);
        queue::open_level(state, QueueId::Dateput, FormatRef::Token(token.clone()))?;
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

/// `this.dateparts` of a `cs:date` token (set by `configureTokenList` on START
/// tokens only). Reading `.length` of it on a token without it (a
/// `cs:date` with no `date-part` children is a SINGLETON) is a TypeError in
/// citeproc-js, and so here.
fn dateparts_of(token: &Token) -> CslResult<Vec<String>> {
    match token.extra.get("dateparts").and_then(Value::as_array) {
        Some(a) => Ok(a.iter().map(js::to_js_string).collect()),
        None => Err(super::EngineError::Csl(
            "TypeError: Cannot read properties of undefined (reading 'length')".to_string(),
        )),
    }
}

/// The "newoutput" closure of `CSL.Node.date.build` (node_date.js:121-151):
/// open the `date` tag, and (for a legal item whose year equals its
/// collection-number) remember where the date blob sits so that the cite
/// can drop it again (`state.tmp.issued_date`, read by api_cite.js:1642).
fn open_tag(state: &mut State, token: &mut Token, item: &Value) -> CslResult<Option<usize>> {
    let var0 = token.variables.first().cloned();
    let Some(var0) = var0.filter(|v| js::truthy_opt(item.get(v.as_str()))) else {
        return Ok(None);
    };
    queue::start_tag(state, QueueId::Output, "date", Some(token))?;
    let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
    // `disable_duplicate_year_suppression.indexOf(Item.country) === -1`
    let country_listed = match (
        state.opt.get("disable_duplicate_year_suppression"),
        item.get("country"),
    ) {
        (Some(Value::Array(a)), Some(c)) => a.iter().any(|x| x == c),
        _ => false,
    };
    // `"" + Item["collection-number"] === "" + state.tmp.date_object.year`
    let js_str = |v: Option<&Value>| match v {
        Some(v) => js::to_js_string(v),
        None => "undefined".to_string(),
    };
    let same_year =
        js_str(item.get("collection-number")) == js_str(state.tmp.date_object.get("year"));
    let preconditions = var0 == "issued"
        && (item_type == "legal_case" || item_type == "legislation")
        && !country_listed
        && state.tmp.extension.is_empty()
        && same_year;
    // `this.dateparts.length` is only read once the conditions before it hold.
    if preconditions && {
        let dateparts = dateparts_of(token)?;
        dateparts.len() == 1 && dateparts[0] == "year"
    } {
        // Set up to (maybe) suppress the year if we're not sorting, and
        // it's the same as the collection-number, and we would render
        // only the year, with not month or day, and this is a legal_case item.
        // We save a pointer to the blob parent and its position here. The
        // blob will be popped from output if at the end of processing for
        // this cite we find that we have rendered the collection-number
        // variable also.
        let has_year_key = state
            .tmp
            .date_object
            .as_object()
            .map(|o| o.keys().any(|k| js::slice(k, 0, Some(4)) == "year"))
            .unwrap_or(false);
        if has_year_key {
            // `state.output.current.mystack.slice(-2)[0].blobs`
            let stack = &state.output.current.mystack;
            let parent = stack
                .len()
                .checked_sub(2)
                .and_then(|i| stack.get(i))
                .or_else(|| stack.first())
                .copied();
            if let Some(list) = parent {
                let n = match &state.blobs.get(list).blobs {
                    super::obj_blob::BlobContent::List(l) => l.len(),
                    super::obj_blob::BlobContent::Text(_) => 0,
                };
                state.tmp.issued_date = Some(super::state::IssuedDate {
                    list,
                    pos: n.saturating_sub(1),
                });
            }
        }
    }
    Ok(None)
}

/// The "mergeoutput" closure of `CSL.Node.date.build` (node_date.js:155-162).
fn close_tag(state: &mut State, token: &mut Token, item: &Value) -> CslResult<Option<usize>> {
    // The END token shares its `variables` array with the START token.
    let var0 = match &state.tmp.date_alias_variables {
        Some(v) => v.first().cloned(),
        None => token.variables.first().cloned(),
    };
    if !js::truthy_opt(var0.and_then(|v| item.get(v.as_str()).cloned()).as_ref()) {
        return Ok(None);
    }
    queue::end_tag(state, QueueId::Output, None)?;
    Ok(None)
}
