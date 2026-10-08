// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_parallel.js
// Version:     2.4.63, commit 73bc1b44bc7d54d0bfec4e070fd27f5efe024ff9
// Copyright:   (c) 2009-2019 Frank Bennett
// Licence:     AGPL-3.0, taken from upstream's "CPAL-1.0 or AGPL-3.0-or-later"
//              (LICENSE at the commit above; see this crate's NOTICE).
// Modified:    2026-10-08, by the OUTRAM PARK contributors. This file is a
//              Rust translation (port) of the files named above, modified
//              from the original.
// No warranty: this program is distributed in the hope that it will be
//              useful, but WITHOUT ANY WARRANTY; without even the implied
//              warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
//              PURPOSE. See the GNU Affero General Public License.

//! `CSL.Parallel`: parallel-cite (multiple versions of one work cited together)
//! and no-repeat tracking. `StartCitation` looks over the items of a cluster
//! and records, for each pair of neighbours, which tracked variables repeat
//! (`state.tmp.suppress_repeats`); `checkRepeats` answers the group nodes'
//! questions from it.
//!
//! The JS `Parallel` object holds only the engine; the state lives in
//! `state.tmp.suppress_repeats` and the registry, so the type here is empty
//! and the operations are functions over `&mut State`
//! ([`start_citation`], [`check_repeats`]).

use std::collections::BTreeMap;

use serde_json::Value;

use super::js::{self, Obj};
use super::state::{GroupContext, State};
use super::{CslResult, EngineError};

/// `new CSL.Parallel(state)`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Parallel {}

impl Parallel {
    /// `new CSL.Parallel(state)`.
    pub fn new() -> Parallel {
        Parallel {}
    }

    /// `parallel.checkRepeats(params)`.
    pub fn check_repeats(&self, state: &State, params: &GroupContext) -> CslResult<bool> {
        check_repeats(state, params)
    }
}

/// JS `a == b` for the values a tracked variable can hold (strings and numbers
/// on the main path; objects fall back to string conversion).
fn loose_equal(a: &Value, b: &Value) -> bool {
    fn num(v: &Value) -> Option<f64> {
        match v {
            Value::Number(n) => n.as_f64(),
            Value::Bool(x) => Some(if *x { 1.0 } else { 0.0 }),
            Value::String(s) => {
                let t = js::trim(s);
                if t.is_empty() {
                    Some(0.0)
                } else {
                    t.parse::<f64>().ok()
                }
            }
            _ => None,
        }
    }
    match (a, b) {
        (Value::String(x), Value::String(y)) => x == y,
        (Value::Number(_) | Value::Bool(_), Value::Number(_) | Value::Bool(_)) => num(a) == num(b),
        (Value::Number(_), Value::String(_)) | (Value::String(_), Value::Number(_)) => {
            matches!((num(a), num(b)), (Some(x), Some(y)) if x == y)
        }
        _ => js::to_js_string(a) == js::to_js_string(b),
    }
}

/// `typeof x.length === "undefined"`: true for a plain object (a date).
fn has_no_length(v: &Value) -> bool {
    matches!(v, Value::Object(_) | Value::Number(_) | Value::Bool(_))
}

/// `idxEnd` of `StartCitation`: `0` initially, `false` or an index after.
#[derive(Debug, Clone, Copy, PartialEq)]
enum IdxEnd {
    False,
    Num(i64),
}

impl IdxEnd {
    fn truthy(self) -> bool {
        matches!(self, IdxEnd::Num(n) if n != 0)
    }
}

/// `CSL.Parallel.prototype.StartCitation(sortedItems)` for `[Item, item]`
/// pairs: reset and fill `state.tmp.suppress_repeats`, and mark parallel
/// masters and siblings (`item.parallel`, the registry's `master` and
/// `siblings`).
pub fn start_citation(state: &mut State, sorted_items: &mut [(Value, Obj)]) -> CslResult<()> {
    // This array carries the repeat markers used in rendering the cite.
    state.tmp.suppress_repeats = Some(Vec::new());
    if sorted_items.len() < 2 {
        return Ok(());
    }
    let track: Vec<String> = state
        .opt
        .get("track_repeat")
        .and_then(Value::as_object)
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();
    let mut idx_end = IdxEnd::Num(0);
    let mut parallel_match_list = false;
    let mut sibling_ranges: Vec<(usize, i64)> = Vec::new();
    let mut repeats: Vec<BTreeMap<String, bool>> = Vec::new();

    for i in 0..sorted_items.len() - 1 {
        let mut fresh_match_list = false;
        let mut info: BTreeMap<String, bool> = BTreeMap::new();
        let see_also = sorted_items[i].0.get("seeAlso").and_then(Value::as_array).cloned();
        if see_also.as_ref().map(|a| !a.is_empty()).unwrap_or(false) && !parallel_match_list {
            fresh_match_list = true;
            let see_also = see_also.unwrap_or_default();
            let mut list: Vec<Value> = vec![sorted_items[i].0.get("id").cloned().unwrap_or(Value::Null)];
            list.extend(see_also);
            parallel_match_list = true;
            let mut temp_match_list = list.clone();
            sorted_items[i].1.insert("parallel".into(), Value::String("first".into()));
            let remainder = sorted_items.len() - i;
            for j in 0..remainder {
                let item_id = sorted_items[i + j].0.get("id").cloned().unwrap_or(Value::Null);
                let ididx = temp_match_list.iter().position(|x| *x == item_id);
                idx_end = IdxEnd::False;
                if ididx.is_none() {
                    idx_end = IdxEnd::Num((i + j) as i64 - 1);
                } else if i + j == sorted_items.len() - 1 {
                    idx_end = IdxEnd::Num((i + j) as i64);
                }
                if let (true, IdxEnd::Num(n)) = (idx_end.truthy(), idx_end) {
                    sibling_ranges.push((i, n));
                    break;
                } else if let Some(at) = ididx {
                    temp_match_list.remove(at);
                } else {
                    // `tempMatchList.slice(0, -1).concat(tempMatchList.slice(0))`
                    let mut t = temp_match_list.clone();
                    t.pop();
                    t.extend(temp_match_list.iter().cloned());
                    temp_match_list = t;
                }
            }
        }
        // parallelMatchList/freshMatchList relate only to parallels.
        // no-repeat non-parallels are handled in a separate block.
        if i > 0 && fresh_match_list {
            if let Some(prev) = repeats.get_mut(i - 1) {
                prev.insert("START".to_string(), true);
            }
        }
        let curr_item = &sorted_items[i].0;
        let next_item = &sorted_items[i + 1].0;
        for varname in &track {
            let cur = curr_item.get(varname).filter(|v| js::truthy(v));
            let nxt = next_item.get(varname).filter(|v| js::truthy(v));
            let (Some(cur), Some(nxt)) = (cur, nxt) else {
                // Go ahead and render any value with an empty partner
                info.insert(varname.clone(), false);
                continue;
            };
            if matches!(nxt, Value::String(_) | Value::Number(_)) {
                // Simple comparison of string values
                let (cv, nv) = if varname == "title"
                    && js::truthy_opt(curr_item.get("title-short"))
                    && js::truthy_opt(next_item.get("title-short"))
                {
                    (
                        curr_item.get("title-short").cloned().unwrap_or(Value::Null),
                        next_item.get("title-short").cloned().unwrap_or(Value::Null),
                    )
                } else {
                    (cur.clone(), nxt.clone())
                };
                info.insert(varname.clone(), loose_equal(&cv, &nv));
            } else if has_no_length(cur) {
                // If a date, use only the year
                let mut same = false;
                let cy = cur.get("year").filter(|v| js::truthy(v));
                let ny = nxt.get("year").filter(|v| js::truthy(v));
                if let (Some(cy), Some(ny)) = (cy, ny) {
                    if loose_equal(cy, ny) {
                        same = true;
                    }
                }
                info.insert(varname.clone(), same);
            } else {
                // If a creator value, kludge it
                info.insert(varname.clone(), cur.to_string() == nxt.to_string());
            }
        }
        if !parallel_match_list {
            info.insert("ORPHAN".to_string(), true);
        }
        if idx_end == IdxEnd::Num(i as i64) {
            info.insert("END".to_string(), true);
            parallel_match_list = false;
        }
        repeats.push(info);
    }
    state.tmp.suppress_repeats = Some(repeats);

    // Set no-repeat info here?
    for (start, end) in sibling_ranges {
        let master_id = super::registry::id_key(sorted_items[start].0.get("id"));
        let token = state.registry.registry.get_mut(&master_id).ok_or_else(|| {
            EngineError::Csl("TypeError: Cannot set properties of undefined (setting 'master')".into())
        })?;
        token.master = true;
        token.siblings = Some(Vec::new());
        let mut k = start as i64;
        while k < end {
            let ku = k as usize;
            if let Some(Some(r)) = state.tmp.suppress_repeats.as_mut().map(|v| v.get_mut(ku)) {
                r.insert("SIBLING".to_string(), true);
            }
            let sibling_id =
                js::to_js_string(sorted_items[ku + 1].0.get("id").unwrap_or(&Value::Null));
            sorted_items[ku + 1]
                .1
                .insert("parallel".into(), Value::String("other".into()));
            if let Some(t) = state.registry.registry.get_mut(&master_id) {
                if let Some(s) = t.siblings.as_mut() {
                    s.push(sibling_id);
                }
            }
            k += 1;
        }
    }
    Ok(())
}

/// The variable names of a `parallel_first` / `parallel_last` /
/// `non_parallel` value: its object keys (`None` when it has none).
fn param_keys(v: &Value) -> Option<Vec<String>> {
    match v {
        Value::Object(o) if !o.is_empty() => Some(o.keys().cloned().collect()),
        _ => None,
    }
}

/// `CSL.Parallel.prototype.checkRepeats(params)` where `params` are the
/// group flags (`state.tmp.group_context.tip`): whether the group must be
/// suppressed because its tracked variables repeat the neighbouring cite's.
pub fn check_repeats(state: &State, params: &GroupContext) -> CslResult<bool> {
    let idx = state.tmp.cite_index;
    let Some(repeats) = &state.tmp.suppress_repeats else {
        return Ok(false);
    };
    let empty: BTreeMap<String, bool> = BTreeMap::new();
    let lookup = |arr: &dyn Fn(i64) -> Option<BTreeMap<String, bool>>| -> CslResult<BTreeMap<String, bool>> {
        arr(idx).ok_or_else(|| {
            EngineError::Csl("TypeError: Cannot read properties of undefined (reading 'START')".into())
        })
    };
    if let Some(keys) = param_keys(&params.parallel_first) {
        let arr = |i: i64| -> Option<BTreeMap<String, bool>> {
            if i == 0 {
                Some(empty.clone())
            } else {
                repeats.get((i - 1) as usize).cloned()
            }
        };
        let at = lookup(&arr)?;
        let mut ret = true;
        for varname in keys {
            if !at.get(&varname).copied().unwrap_or(false) || at.get("START").copied().unwrap_or(false) {
                // true --> suppress the entry
                // Test here evaluates as "all", not "any"
                ret = false;
            }
        }
        return Ok(ret);
    }
    if let Some(keys) = param_keys(&params.parallel_last) {
        let arr = |i: i64| -> Option<BTreeMap<String, bool>> {
            if i >= 0 && (i as usize) < repeats.len() {
                repeats.get(i as usize).cloned()
            } else if i as usize == repeats.len() {
                Some(empty.clone())
            } else {
                None
            }
        };
        let at = lookup(&arr)?;
        let mut ret = true;
        for varname in keys {
            if !at.get(&varname).copied().unwrap_or(false) || at.get("END").copied().unwrap_or(false) {
                // "all" match, as above.
                ret = false;
            }
        }
        return Ok(ret);
    }
    if let Some(keys) = param_keys(&params.non_parallel) {
        let arr = |i: i64| -> Option<BTreeMap<String, bool>> {
            if i == 0 {
                Some(empty.clone())
            } else {
                repeats.get((i - 1) as usize).cloned()
            }
        };
        let at = lookup(&arr)?;
        let mut ret = true;
        for varname in keys {
            if !at.get(&varname).copied().unwrap_or(false) {
                ret = false;
            }
        }
        return Ok(ret);
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::citeproc::registry::RegistryItem;
    use serde_json::json;

    fn items(list: Value) -> Vec<(Value, Obj)> {
        list.as_array()
            .map(|a| a.iter().map(|i| (i.clone(), Obj::new())).collect())
            .unwrap_or_default()
    }

    fn state_tracking(vars: &[&str]) -> State {
        let mut s = State::default();
        let mut tr = Obj::new();
        for v in vars {
            tr.insert(v.to_string(), Value::Bool(true));
        }
        s.opt.insert("track_repeat".into(), Value::Object(tr));
        s
    }

    #[test]
    fn no_repeat_info_marks_equal_neighbours_and_orphans() {
        let mut s = state_tracking(&["container-title", "title", "issued"]);
        let mut list = items(json!([
            {"id": "a", "container-title": "J", "title": "T1", "issued": {"year": 2000}},
            {"id": "b", "container-title": "J", "title": "T2", "issued": {"year": 2000}},
            {"id": "c", "container-title": "K"}
        ]));
        start_citation(&mut s, &mut list).unwrap();
        let r = s.tmp.suppress_repeats.clone().unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!(r[0]["container-title"], true);
        assert_eq!(r[0]["title"], false);
        assert_eq!(r[0]["issued"], true);
        assert!(r[0]["ORPHAN"]);
        assert_eq!(r[1]["container-title"], false);
        assert_eq!(r[1]["issued"], false);
    }

    #[test]
    fn see_also_builds_a_master_with_siblings() {
        let mut s = state_tracking(&["title"]);
        for id in ["a", "b", "c"] {
            s.registry.registry.insert(id.into(), RegistryItem::new(id));
        }
        let mut list = items(json!([
            {"id": "a", "seeAlso": ["b"], "title": "X"},
            {"id": "b", "title": "X"},
            {"id": "c", "title": "Y"}
        ]));
        start_citation(&mut s, &mut list).unwrap();
        assert_eq!(list[0].1.get("parallel"), Some(&json!("first")));
        assert_eq!(list[1].1.get("parallel"), Some(&json!("other")));
        let a = &s.registry.registry["a"];
        assert!(a.master);
        assert_eq!(a.siblings, Some(vec!["b".to_string()]));
        let r = s.tmp.suppress_repeats.clone().unwrap();
        assert_eq!(r[0].get("SIBLING"), Some(&true));
    }

    #[test]
    fn check_repeats_reads_the_markers() {
        let mut s = state_tracking(&["title"]);
        let mut list = items(json!([{"id": "a", "title": "X"}, {"id": "b", "title": "X"}]));
        start_citation(&mut s, &mut list).unwrap();
        let mut flags = GroupContext::default();
        flags.non_parallel = json!({"title": true});
        s.tmp.cite_index = 0;
        assert!(!check_repeats(&s, &flags).unwrap());
        s.tmp.cite_index = 1;
        assert!(check_repeats(&s, &flags).unwrap());
        // nothing tracked: never suppress
        assert!(!check_repeats(&s, &GroupContext::default()).unwrap());
        s.tmp.suppress_repeats = None;
        assert!(!check_repeats(&s, &flags).unwrap());
        assert!(loose_equal(&json!("2"), &json!(2)));
        assert!(!loose_equal(&json!("a"), &json!("b")));
    }
}
