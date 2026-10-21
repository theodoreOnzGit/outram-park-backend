// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/node_sort.js
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

//! Port of `src/node_sort.js`: `CSL.Node.sort`.

use serde_json::Value;

use super::attributes::{self, area_mut, area_ref};
use super::exec::Exec;
use super::js;
use super::obj_token::{Token, TokenType};
use super::state::State;
use super::CslResult;

/// The closures `src/node_sort.js` stores in `token.execs` (PORTING.md §4).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeSortExec {
    /// START: with multiple layout locales, switch `state.opt.lang` to the
    /// language the item sorts under (node_sort.js:14-37).
    Start,
    /// END: restore `state.opt.lang` (node_sort.js:44-51).
    End,
}

impl NodeSortExec {
    /// Run the closure.
    pub fn run(
        &self,
        state: &mut State,
        _token: &mut Token,
        item: &Value,
        _cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        let has_layout_locale = js::truthy_opt(state.opt.get("has_layout_locale"));
        match self {
            NodeSortExec::Start => {
                if has_layout_locale {
                    let dl = attributes::default_locale(state);
                    let language = item
                        .get("language")
                        .filter(|v| js::truthy(v))
                        .map(js::to_js_string)
                        .unwrap_or_default();
                    let langspec = super::util_locale::locale_resolve(&language, Some(&dl));
                    // state[state.tmp.area.slice(0,-5)].opt.sort_locales
                    let area_name = js::slice(&state.tmp.area, 0, Some(-5));
                    let sort_locales: Vec<Value> = area_ref(state, &area_name)?
                        .opt
                        .get("sort_locales")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    let mut lang_for_item: Option<Value> = None;
                    for sl in &sort_locales {
                        let mut found = sl.get(&langspec.bare).cloned();
                        if !found.as_ref().map(js::truthy).unwrap_or(false) {
                            found = sl.get(&langspec.best).cloned();
                        }
                        let ok = found.as_ref().map(js::truthy).unwrap_or(false);
                        lang_for_item = found;
                        if ok {
                            break;
                        }
                    }
                    if !lang_for_item.as_ref().map(js::truthy).unwrap_or(false) {
                        lang_for_item = Some(Value::String(dl));
                    }
                    state.tmp.lang_sort_hold = state.opt.get("lang").map(js::to_js_string);
                    if let Some(l) = lang_for_item {
                        state.opt.insert("lang".into(), l);
                    }
                }
                Ok(None)
            }
            NodeSortExec::End => {
                if has_layout_locale {
                    match state.tmp.lang_sort_hold.take() {
                        Some(l) => {
                            state.opt.insert("lang".into(), Value::String(l));
                        }
                        None => {
                            state.opt.remove("lang");
                        }
                    }
                }
                Ok(None)
            }
        }
    }
}

/// Run `f` with `state[state.build.root + "_sort"].tokens` as the target list
/// (`target = state[state.build.root + "_sort"].tokens`). The list is moved
/// out of the state for the duration of `f`, so builders can take both
/// `&mut State` and the target, and put back afterwards (also on error).
pub fn with_sort_target<R>(
    state: &mut State,
    f: impl FnOnce(&mut State, &mut Vec<Token>) -> CslResult<R>,
) -> CslResult<R> {
    let name = format!("{}_sort", state.build.root);
    let mut list = std::mem::take(&mut area_mut(state, &name)?.tokens);
    let result = f(state, &mut list);
    if let Ok(area) = area_mut(state, &name) {
        area.tokens = list;
    }
    result
}

/// `CSL.Node.sort.build.call(token, state, target)`. The incoming `target` is
/// ignored: upstream reassigns it to the `_sort` area's token list.
pub fn build(
    state: &mut State,
    mut token: Token,
    _target: &mut Vec<Token>,
    _real_group: Option<bool>,
) -> CslResult<()> {
    if token.tokentype == TokenType::Start {
        if state.build.area == "citation" {
            state.opt.insert("sort_citations".into(), Value::Bool(true));
        }
        state.build.area = format!("{}_sort", state.build.root);
        state.build.extension = "_sort".to_string();

        token.execs.push(Exec::NodeSort(NodeSortExec::Start));
    }
    if token.tokentype == TokenType::End {
        state.build.area = state.build.root.clone();
        state.build.extension = String::new();
        token.execs.push(Exec::NodeSort(NodeSortExec::End));
    }
    with_sort_target(state, |_state, list| {
        list.push(token);
        Ok(())
    })
}
