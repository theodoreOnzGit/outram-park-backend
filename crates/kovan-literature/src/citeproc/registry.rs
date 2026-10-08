// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/registry.js
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

//! `CSL.Registry`: the persistent store of the registered items. For each
//! item it holds the disambiguation state and sort keys; it also holds the
//! items' normalised data (`refhash`), the citations of the document
//! ([`CitationReg`](super::disambig_citations::CitationReg)) and the name
//! registry ([`NameReg`](super::disambig_names::NameReg)). `updateItems`
//! (api_update.js) drives it through `init`, `dodeletes`, `doinserts`,
//! `dorefreshes`, `rebuildlist`, `setsortkeys`, `setdisambigs`, `sorttokens`
//! and `renumber`, which are the functions of this file.
//!
//! **Shape.** The registry is data in [`State::registry`]; its operations
//! need the rest of the state (rendering an item for its ambiguous cite,
//! retrieving items), so they are free functions taking `&mut State`, named
//! as the JS methods (`registry::doinserts(state, &ids)`).
//!
//! **Ordering note.** JS enumerates the keys of an object with integer-like
//! keys first, then in insertion order. The maps here are `BTreeMap`s
//! (sorted keys). Only the order of independent re-renderings and of
//! disambiguation of unrelated ambiguity groups depends on it, and none of
//! those results feed each other; `mylist`, `reflist` and every other list
//! whose order matters are `Vec`s in upstream's order.
//!
//! The registry tokens (`registry.registry[id]`) are [`RegistryItem`]s;
//! `reflist` holds ids and the tokens are looked up in `registry.registry`
//! (in JS `reflist` and `registry` hold the same objects).

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use super::build_retrieve_item::retrieve_item;
use super::disambig_citations::CitationReg;
use super::disambig_names::NameReg;
use super::js;
use super::load;
use super::obj_ambigconfig::{AmbigConfig, AmbigId};
use super::sort::SortCompare;
use super::state::State;
use super::util_sort::strip_prepositions;
use super::{CslResult, EngineError};

/// A registry token (`registry.registry[id]`), created by `doinserts`.
///
/// The properties other code hangs on it are fields here:
/// `first-reference-note-number`, `first-container-reference-note-number` and
/// `citation-count` (api_cite.js), `master`, `siblings` and
/// `parallel_delimiter_override` (util_parallel.js, node_group.js).
#[derive(Debug, Clone, PartialEq)]
pub struct RegistryItem {
    /// `id`.
    pub id: String,
    /// `seq`: the position in the registered list, from 1.
    pub seq: i64,
    /// `offset`: characters before the item's entry text in the bibliography
    /// (`second-field-align`), written by `Queue.string`.
    pub offset: i64,
    /// `sortkeys`: `None` is `false` or `undefined`.
    pub sortkeys: Option<Vec<Option<String>>>,
    /// `ambig`: the ambiguous-cite key; `None` is `false`.
    pub ambig: Option<String>,
    /// `disambig`: the item's disambiguation config; `None` is `false`.
    pub disambig: Option<AmbigId>,
    /// `newItem`.
    pub new_item: bool,
    /// `"first-reference-note-number"`.
    pub first_reference_note_number: Option<i64>,
    /// `"first-container-reference-note-number"`.
    pub first_container_reference_note_number: Option<i64>,
    /// `"citation-count"`.
    pub citation_count: Option<i64>,
    /// `master` (parallel cites).
    pub master: bool,
    /// `siblings` (parallel cites); `None` is `undefined`.
    pub siblings: Option<Vec<String>>,
    /// `parallel_delimiter_override`.
    pub parallel_delimiter_override: Option<Value>,
    /// `parallel` (read by node_layout.js; never set by 2.4.63).
    pub parallel: Option<Value>,
}

impl RegistryItem {
    /// The token `doinserts` creates (step 4e).
    pub fn new(id: &str) -> RegistryItem {
        RegistryItem {
            id: id.to_string(),
            seq: 0,
            offset: 0,
            sortkeys: None,
            ambig: None,
            disambig: None,
            new_item: true,
            first_reference_note_number: None,
            first_container_reference_note_number: None,
            citation_count: None,
            master: false,
            siblings: None,
            parallel_delimiter_override: None,
            parallel: None,
        }
    }
}

/// `registry.return_data`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReturnData {
    /// `bibchange`.
    pub bibchange: bool,
    /// `citation_errors` (set by `processCitationCluster`).
    pub citation_errors: Vec<Value>,
}

/// `CSL.Registry.Comparifier(state, keyset)`: the comparator of sort keys.
///
/// Upstream captures `state[keyset].opt.sort_directions` (the array object)
/// at construction and the build then pushes onto it, so the comparator
/// effectively reads the final directions. Here they are read from the area
/// each time ([`Comparifier::keyset`]), with the same result for any
/// comparator made at or before the end of the build. (node_citation.js
/// replaces the array before making its comparator, and so does this.)
#[derive(Debug, Clone, PartialEq)]
pub struct Comparifier {
    /// The area whose `opt.sort_directions` apply: `"citation_sort"` or
    /// `"bibliography_sort"`.
    pub keyset: String,
    /// `sortCompare`.
    pub sort_compare: SortCompare,
}

impl Default for Comparifier {
    fn default() -> Self {
        Comparifier {
            keyset: "citation_sort".to_string(),
            sort_compare: SortCompare::default(),
        }
    }
}

/// What `compareKeys` reads from `a` and `b`: `sortkeys` and `seq`.
#[derive(Debug, Clone, Copy)]
pub struct SortKeyed<'a> {
    /// `sortkeys` (empty for `false` / `undefined`).
    pub sortkeys: &'a [Option<String>],
    /// `seq` (`None` for a cite item, which has none).
    pub seq: Option<i64>,
}

impl Comparifier {
    /// `new CSL.Registry.Comparifier(state, keyset)`: the collation comes
    /// from `state.opt["default-locale-sort"]` as it is now.
    pub fn new(state: &State, keyset: &str) -> Comparifier {
        let locale = js::get_str(&state.opt, "default-locale-sort");
        Comparifier {
            keyset: keyset.to_string(),
            sort_compare: SortCompare::new(locale),
        }
    }

    /// `compareKeys(a, b)`: `-1`, `0` or `1` (the key directions' values).
    pub fn compare_keys(&self, state: &State, a: SortKeyed, b: SortKeyed) -> CslResult<i32> {
        let directions = state
            .area_ref(&self.keyset)
            .opt
            .get("sort_directions")
            .and_then(Value::as_array);
        let direction = |pos: usize, which: usize| -> CslResult<i32> {
            directions
                .and_then(|d| d.get(pos))
                .and_then(|d| d.get(which))
                .and_then(Value::as_i64)
                .map(|n| n as i32)
                .ok_or_else(|| {
                    EngineError::Csl(
                        "TypeError: Cannot read properties of undefined (reading 'sort_directions[pos]')"
                            .to_string(),
                    )
                })
        };
        for pos in 0..a.sortkeys.len() {
            //
            // for ascending sort 1 uses 1, -1 uses -1.
            // For descending sort, the values are reversed.
            //
            // Need to handle undefined values.  No way around it.
            // So have to screen .localeCompare (which is also
            // needed) from undefined values.  Everywhere, in all
            // compares.
            //
            let ka = a.sortkeys.get(pos).and_then(|k| k.as_deref());
            let kb = b.sortkeys.get(pos).and_then(|k| k.as_deref());
            let cmp: i32 = match (ka, kb) {
                (x, y) if x == y => 0,
                (None, _) => direction(pos, 1)?,
                (_, None) => direction(pos, 0)?,
                (Some(x), Some(y)) => self.sort_compare.compare(state, x, y),
            };
            if 0 < cmp {
                return direction(pos, 1);
            } else if 0 > cmp {
                return direction(pos, 0);
            }
        }
        match (a.seq, b.seq) {
            (Some(x), Some(y)) if x > y => Ok(1),
            (Some(x), Some(y)) if x < y => Ok(-1),
            _ => Ok(0),
        }
    }

    /// `compareCompositeKeys(a, b)`: `compareKeys(a[1], b[1])` for
    /// `[Item, item]` pairs, whose cite item carries `sortkeys` and no `seq`.
    pub fn compare_composite_keys(
        &self,
        state: &State,
        a: &[Option<String>],
        b: &[Option<String>],
    ) -> CslResult<i32> {
        self.compare_keys(
            state,
            SortKeyed {
                sortkeys: a,
                seq: None,
            },
            SortKeyed {
                sortkeys: b,
                seq: None,
            },
        )
    }
}

/// A cite item's `sortkeys` (a JSON array of strings and nulls) as sort keys.
pub fn sortkeys_of_item(item: &js::Obj) -> Vec<Option<String>> {
    match item.get("sortkeys") {
        Some(Value::Array(a)) => a
            .iter()
            .map(|k| match k {
                Value::String(s) => Some(s.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Sort keys (`Vec<Option<String>>`) as the JSON array stored on a cite item.
pub fn sortkeys_to_value(keys: &[Option<String>]) -> Value {
    Value::Array(
        keys.iter()
            .map(|k| match k {
                Some(s) => Value::String(s.clone()),
                None => Value::Null,
            })
            .collect(),
    )
}

/// `Array.prototype.sort(comparator)` with a comparator that can fail: the
/// first error stops further comparisons (they report equal) and is
/// returned. Stable, as V8's TimSort is.
pub fn sort_with<T>(
    list: &mut [T],
    mut cmp: impl FnMut(&T, &T) -> CslResult<i32>,
) -> CslResult<()> {
    let mut failure: Option<EngineError> = None;
    list.sort_by(|a, b| {
        if failure.is_some() {
            return std::cmp::Ordering::Equal;
        }
        match cmp(a, b) {
            Ok(n) => n.cmp(&0),
            Err(e) => {
                failure = Some(e);
                std::cmp::Ordering::Equal
            }
        }
    });
    match failure {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// `new CSL.Registry(state)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Registry {
    /// `registry`: the registry tokens by item id.
    pub registry: BTreeMap<String, RegistryItem>,
    /// `reflist`: the ids of the registered items, in bibliography order.
    pub reflist: Vec<String>,
    /// `refhash`: the normalised items by id (what `retrieveItem` returns).
    pub refhash: BTreeMap<String, Value>,
    /// `namereg`.
    pub namereg: NameReg,
    /// `citationreg`.
    pub citationreg: CitationReg,
    /// `authorstrings` (set by the name output).
    pub authorstrings: BTreeMap<String, String>,
    /// `mylist`.
    pub mylist: Vec<String>,
    /// `myhash`.
    pub myhash: BTreeSet<String>,
    /// `uncited`.
    pub uncited: BTreeSet<String>,
    /// `refreshes`.
    pub refreshes: BTreeSet<String>,
    /// `akeys`.
    pub akeys: BTreeSet<String>,
    /// `touched`.
    pub touched: BTreeSet<String>,
    /// `ambigsTouched`.
    pub ambigs_touched: BTreeSet<String>,
    /// `oldseq`.
    pub oldseq: BTreeMap<String, i64>,
    /// `return_data`.
    pub return_data: ReturnData,
    /// `ambigcites`: by ambiguous-cite key, the ids sharing it.
    pub ambigcites: BTreeMap<String, Vec<String>>,
    /// `ambigresets`.
    pub ambigresets: BTreeMap<String, usize>,
    /// `reflist_inserts`.
    pub reflist_inserts: Vec<String>,
    /// `sorter`: `new CSL.Registry.Comparifier(state, "bibliography_sort")`.
    pub sorter: Comparifier,
    /// The pool of [`AmbigConfig`]s that the registry tokens, the tmp state
    /// and the disambiguator share (see [`AmbigId`]).
    pub ambig_pool: Vec<AmbigConfig>,
    /// Counter behind the generated `citationID`s (`setCitationId`).
    pub citation_id_counter: u64,
}

impl Default for Registry {
    fn default() -> Self {
        Registry::new(Comparifier {
            keyset: "bibliography_sort".to_string(),
            sort_compare: SortCompare::default(),
        })
    }
}

impl Registry {
    /// `new CSL.Registry(state)` with its sorter already made
    /// (`Comparifier::new(state, "bibliography_sort")`).
    pub fn new(sorter: Comparifier) -> Registry {
        Registry {
            registry: BTreeMap::new(),
            reflist: Vec::new(),
            refhash: BTreeMap::new(),
            namereg: NameReg::default(),
            citationreg: CitationReg::default(),
            authorstrings: BTreeMap::new(),
            mylist: Vec::new(),
            myhash: BTreeSet::new(),
            uncited: BTreeSet::new(),
            refreshes: BTreeSet::new(),
            akeys: BTreeSet::new(),
            touched: BTreeSet::new(),
            ambigs_touched: BTreeSet::new(),
            oldseq: BTreeMap::new(),
            return_data: ReturnData::default(),
            ambigcites: BTreeMap::new(),
            ambigresets: BTreeMap::new(),
            reflist_inserts: Vec::new(),
            sorter,
            ambig_pool: Vec::new(),
            citation_id_counter: 0,
        }
    }

    /// `registry.registry[id].disambig.year_suffix` (`None` when the item is
    /// not registered or its suffix is `false`): the value `cs:date-part` and
    /// `cs:text variable="year-suffix"` turn into a letter.
    pub fn year_suffix(&self, id: &str) -> Option<Value> {
        let d = self.registry.get(id)?.disambig?;
        let v = &self.ambig_pool.get(d.0)?.year_suffix;
        if *v == Value::Bool(false) { None } else { Some(v.clone()) }
    }

    /// `getSortedIds()`: the ids of `reflist`.
    pub fn get_sorted_ids(&self) -> Vec<String> {
        self.reflist.clone()
    }

    /// The sort keys view of a registry token.
    fn keyed(&self, id: &str) -> SortKeyed<'_> {
        match self.registry.get(id) {
            Some(t) => SortKeyed {
                sortkeys: t.sortkeys.as_deref().unwrap_or(&[]),
                seq: Some(t.seq),
            },
            None => SortKeyed {
                sortkeys: &[],
                seq: None,
            },
        }
    }
}

/// `"" + id` for the `id` property of an item (`None` is an absent `id`,
/// which JS prints as `"undefined"`: the test suite has items without one).
pub fn id_key(id: Option<&Value>) -> String {
    match id {
        None => "undefined".to_string(),
        Some(v) => js::to_js_string(v),
    }
}

fn type_error(what: &str) -> EngineError {
    EngineError::Csl(format!("TypeError: {what}"))
}

/// `registry.init(itemIDs, uncited_flag)`.
pub fn init(state: &mut State, item_ids: Vec<String>, uncited_flag: bool) {
    let reg = &mut state.registry;
    reg.oldseq = BTreeMap::new();
    //  1. Receive list as function argument, store as hash and as list.
    //
    // Result:
    //   this.mylist: a list of all itemIDs of referenced items, cited and uncited.
    //   this.myhash: a hash of index positions in this.mylist.
    //   this.uncited: hash of uncited itemIDs.
    //
    // Proceed as follows.
    //
    if uncited_flag {
        // If uncited_flag is non-nil, add any missing itemIDs to this.mylist
        // from itemIDs input list, and set the itemIDs in itemIDs on this.uncited.
        reg.uncited = BTreeSet::new();
        for id in &item_ids {
            if !reg.myhash.contains(id) {
                reg.mylist.push(id.clone());
            }
            reg.uncited.insert(id.clone());
            reg.myhash.insert(id.clone());
        }
    } else {
        // If uncited_flag is nil, remove duplicate itemIDs from itemIDs input
        // list, set the result on this.mylist, and add missing itemIDs to
        // this.mylist from itemIDs input list.
        let mut ids = item_ids;
        for key in &reg.uncited {
            ids.push(key.clone());
        }
        let mut myhash = BTreeSet::new();
        let mut i = ids.len();
        while i > 0 {
            i -= 1;
            if myhash.contains(&ids[i]) {
                ids.remove(i);
            } else {
                myhash.insert(ids[i].clone());
            }
        }
        reg.mylist = ids;
        reg.myhash = myhash;
    }
    //
    //  2. Initialize refresh list.  Never needs sorting, only hash required.
    //
    reg.refreshes = BTreeSet::new();
    reg.touched = BTreeSet::new();
    reg.ambigs_touched = BTreeSet::new();
    reg.ambigresets = BTreeMap::new();
}

/// `registry.dopurge(myhash)`: remove any uncited items not in `myhash`.
pub fn dopurge(state: &mut State, myhash: &BTreeSet<String>) -> CslResult<()> {
    let mut i = state.registry.mylist.len();
    while i > 0 {
        i -= 1;
        // Might not want to be quite this restrictive.
        if let Some(by_item) = &state.registry.citationreg.citations_by_item_id {
            let id = state.registry.mylist[i].clone();
            if !by_item.contains_key(&id) && !myhash.contains(&id) {
                state.registry.myhash.remove(&id);
                state.registry.uncited.remove(&id);
                state.registry.mylist.remove(i);
            }
        }
    }
    let keep = state.registry.myhash.clone();
    dodeletes(state, &keep)
}

/// `registry.dodeletes(myhash)`: delete the registered items that are not in
/// `myhash`.
pub fn dodeletes(state: &mut State, myhash: &BTreeSet<String>) -> CslResult<()> {
    //
    //  3. Delete loop.
    //
    let keys: Vec<String> = state.registry.registry.keys().cloned().collect();
    for key in keys {
        if myhash.contains(&key) {
            continue;
        }
        // skip items explicitly marked as uncited
        if state.registry.uncited.contains(&key) {
            continue;
        }
        //
        //  3a. Delete names in items to be deleted from names reg.
        //
        let otheritems = super::disambig_names::delitems(state, std::slice::from_ref(&key))?;
        //
        //  3b. Complement refreshes list with items affected by
        //      possible name changes.  We'll actually perform the refresh once
        //      all of the necessary data and parameters have been established
        //      in the registry.
        //
        for kkey in otheritems.keys() {
            state.registry.refreshes.insert(kkey.clone());
        }
        //
        //  3c. Delete all items to be deleted from their disambig pools.
        //
        let ambig = state
            .registry
            .registry
            .get(&key)
            .and_then(|t| t.ambig.clone())
            .unwrap_or_else(|| "false".to_string());
        let pool = state
            .registry
            .ambigcites
            .get(&ambig)
            .cloned()
            .ok_or_else(|| type_error("Cannot read properties of undefined (reading 'indexOf')"))?;
        if let Some(mypos) = pool.iter().position(|x| *x == key) {
            let mut items = pool.clone();
            items.remove(mypos);
            let n = items.len();
            state.registry.ambigcites.insert(ambig.clone(), items);
            state.registry.ambigresets.insert(ambig.clone(), n);
        }
        //
        // XX. What we've missed is to provide an update of all
        // items sharing the same ambig  += -1 the remaining items in
        // ambigcites.  So let's do that here, just in case the
        // names update above doesn't catch them all.
        //
        let remaining = state
            .registry
            .ambigcites
            .get(&ambig)
            .cloned()
            .unwrap_or_default();
        for id in remaining {
            state.registry.refreshes.insert(id);
        }
        //
        // 3d-0. Remove parallel id references and realign
        // parallel ID refs.
        //
        if let Some(siblings) = state.registry.registry.get(&key).and_then(|t| t.siblings.clone()) {
            if siblings.len() == 1 {
                let lone = siblings[0].clone();
                let lone_token = state.registry.registry.get_mut(&lone).ok_or_else(|| {
                    type_error("Cannot read properties of undefined (reading 'siblings')")
                })?;
                if let Some(s) = lone_token.siblings.as_mut() {
                    s.pop();
                    lone_token.master = true;
                    // this.registry[loneSiblingID].parallel = false;
                }
            } else if siblings.len() > 1 {
                let mut remove_ids = vec![key.clone()];
                let is_master = state.registry.registry.get(&key).map(|t| t.master).unwrap_or(false);
                if is_master {
                    let newmaster_id = siblings[0].clone();
                    if let Some(newmaster) = state.registry.registry.get_mut(&newmaster_id) {
                        newmaster.master = true;
                    } else {
                        return Err(type_error("Cannot set properties of undefined (setting 'master')"));
                    }
                    // newmaster.parallel_delimiter is set externally, if at all
                    remove_ids.push(newmaster_id);
                }
                let mut buffer: Vec<String> = Vec::new();
                let mut rest = siblings.clone();
                while let Some(sibling_id) = rest.pop() {
                    if !remove_ids.contains(&sibling_id) {
                        buffer.push(sibling_id);
                    }
                }
                buffer.reverse();
                if let Some(t) = state.registry.registry.get_mut(&key) {
                    t.siblings = Some(buffer);
                }
            }
        }
        //
        // 3d-1. Remove item from reflist
        state.registry.reflist.retain(|id| *id != key);
        //
        //  3d. Delete all items in deletion list from hash.
        //
        state.registry.registry.remove(&key);
        state.registry.refhash.remove(&key);

        // For processCitationCluster()
        state.registry.return_data.bibchange = true;
    }
    Ok(())
}

/// `registry.doinserts(mylist)`: register the items in `mylist` that are not
/// registered yet.
pub fn doinserts(state: &mut State, mylist: &[String]) -> CslResult<()> {
    //
    //  4. Insert loop.
    //
    for item in mylist {
        if state.registry.registry.contains_key(item) {
            continue;
        }
        //
        //  4a. Retrieve entries for items to insert.
        //
        let item_data = retrieve_item(state, item)?;

        //
        //  4b. Generate ambig key.
        //
        // AND
        //
        //  4c. Add names in items to be inserted to names reg
        //      (implicit in getAmbiguousCite).
        //
        let akey = state.get_ambiguous_cite(&item_data, None, false, None)?.to_js_string();
        state.registry.ambigs_touched.insert(akey.clone());
        //
        //  4d. Record ambig pool key on akey list (used for updating further
        //      down the chain).
        //
        if !js::truthy_opt(item_data.get("legislation_id")) {
            state.registry.akeys.insert(akey.clone());
        }
        //
        //  4e. Create registry token.
        //
        let mut newitem = RegistryItem::new(item);
        //
        //  4f(a). Add first reference note number
        //         (this may be redundant)
        if let Some(by_item) = &state.registry.citationreg.citations_by_item_id {
            if let Some(first) = by_item.get(item).and_then(|v| v.first()) {
                newitem.first_reference_note_number =
                    Some(state.registry.citationreg.get(*first).note_index());
            }
        }
        //
        //  4f. Add item ID to hash.
        //
        state.registry.registry.insert(item.clone(), newitem);

        //
        //  4g. Set and record the base token to hold disambiguation
        //      results ("disambig" in the object above).
        //
        let abase = state.get_ambig_config();
        register_ambig_token(state, &akey, item, abase)?;

        //
        //  4h. Make a note that this item needs its sort keys refreshed.
        //
        state.registry.touched.insert(item.clone());
        // For processCitationCluster()
        state.registry.return_data.bibchange = true;
    }
    Ok(())
}

/// `registry.rebuildlist(nosort)`.
pub fn rebuildlist(state: &mut State, nosort: bool) -> CslResult<()> {
    //
    //  5. Create "new" list of hash pointers, in the order given in the argument
    //     to the update function.
    //
    //
    // XXX Keep reflist in place.
    //
    let mylist = state.registry.mylist.clone();
    let reg = &mut state.registry;
    if !nosort {
        reg.reflist_inserts = Vec::new();
        //
        //  6. Apply citation numbers to new list,
        //     saving off old sequence numbers as we go.
        //
        // XXX Just memo inserts -- actual insert happens below, at last "sort"
        //
        for (pos, item) in mylist.iter().enumerate() {
            let token = reg
                .registry
                .get_mut(item)
                .ok_or_else(|| type_error("Cannot read properties of undefined (reading 'newItem')"))?;
            if token.new_item {
                reg.reflist_inserts.push(item.clone());
            }
            reg.oldseq.insert(item.clone(), token.seq);
            token.seq = (pos + 1) as i64;
        }
    } else {
        reg.reflist = Vec::new();
        for (pos, item) in mylist.iter().enumerate() {
            let token = reg
                .registry
                .get_mut(item)
                .ok_or_else(|| type_error("Cannot read properties of undefined (reading 'seq')"))?;
            reg.reflist.push(item.clone());
            reg.oldseq.insert(item.clone(), token.seq);
            token.seq = (pos + 1) as i64;
        }
    }
    Ok(())
}

/// `registry.dorefreshes()`.
pub fn dorefreshes(state: &mut State) -> CslResult<()> {
    //
    //  7. Refresh items requiring update.
    //
    // It looks like we need to do four things on each cite for refresh:
    // (1) Generate the akey for the cite.
    // (2) Register it on the ambig token.
    // (3) Register the akey in this.akeys
    // (4) Register the item ID in this.touched
    //
    let keys: Vec<String> = state.registry.refreshes.iter().cloned().collect();
    for key in keys {
        let Some(regtoken) = state.registry.registry.get(&key) else {
            continue;
        };
        let mut akey: Option<String> = regtoken.ambig.clone();
        if let Some(t) = state.registry.registry.get_mut(&key) {
            t.sortkeys = None;
        }
        let mut item = state
            .refetch_item(&key)
            .ok_or_else(|| type_error("Cannot read properties of undefined (reading 'legislation_id')"))?;

        // `"undefined" === typeof akey` is never true for a registered token
        // (`ambig` is `false` or a string), as upstream.
        let resets: Vec<(String, usize)> = state
            .registry
            .ambigresets
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        for (_akkey, count) in resets {
            if count == 1 {
                // (upstream indexes `ambigcites` with `akey`, not `akkey`)
                let lookup = akey.clone().unwrap_or_else(|| "false".to_string());
                let lone_key = state
                    .registry
                    .ambigcites
                    .get(&lookup)
                    .and_then(|v| v.first())
                    .cloned()
                    .ok_or_else(|| type_error("Cannot read properties of undefined (reading '0')"))?;
                item = state.refetch_item(&lone_key).ok_or_else(|| {
                    type_error("Cannot read properties of undefined (reading 'legislation_id')")
                })?;
                let fresh = state.alloc_ambig(AmbigConfig::default());
                state
                    .registry
                    .registry
                    .get_mut(&lone_key)
                    .ok_or_else(|| type_error("Cannot set properties of undefined (setting 'disambig')"))?
                    .disambig = Some(fresh);
                state.tmp.disambig_settings = None;
                let new_akey = state.get_ambiguous_cite(&item, None, false, None)?.to_js_string();
                akey = Some(new_akey.clone());
                let abase = state.get_ambig_config();
                register_ambig_token(state, &new_akey, &lone_key, abase)?;
            }
        }
        state.tmp.tainted_item_ids.insert(key.clone(), true);
        let akey = akey.unwrap_or_else(|| "false".to_string());
        state.registry.ambigs_touched.insert(akey.clone());
        if !js::truthy_opt(item.get("legislation_id")) {
            state.registry.akeys.insert(akey);
        }
        state.registry.touched.insert(key);
    }
    Ok(())
}

/// `registry.setdisambigs()`: main disambiguation. Resolves all
/// disambiguation issues for the cites touched by the update.
pub fn setdisambigs(state: &mut State) -> CslResult<()> {
    //
    // Okay, more changes.  Here is where we resolve all disambiguation
    // issues for cites touched by the update.  The this.ambigcites set is
    // based on the complete short form of citations, and is the basis on
    // which names are added and minimal adding of initials or given names
    // is performed.
    //

    //
    //  8.  Set disambiguation parameters on each inserted item token.
    //
    let touched: Vec<String> = state.registry.ambigs_touched.iter().cloned().collect();
    for akey in touched {
        //
        // Disambiguation is fully encapsulated.
        // Disambiguator will run only if there are multiple
        // items, and at least one disambiguation mode is
        // in effect.
        super::disambig_cites::run(state, &akey)?;
    }
    state.registry.ambigs_touched = BTreeSet::new();
    state.registry.akeys = BTreeSet::new();
    Ok(())
}

/// `registry.renumber()`: reset citation numbers on list items.
pub fn renumber(state: &mut State) {
    //
    // 19. Reset citation numbers on list items
    //
    let descending = state
        .bibliography_sort
        .opt
        .get("citation_number_sort_direction")
        .and_then(Value::as_i64)
        == Some(load::DESCENDING);
    if descending {
        state
            .bibliography_sort
            .tmp
            .insert("citation_number_map".into(), Value::Object(js::Obj::new()));
    }
    let len = state.registry.reflist.len();
    let update_mode = state.opt.get("update_mode").and_then(Value::as_i64);
    for pos in 0..len {
        let id = state.registry.reflist[pos].clone();
        // save the overhead of rerenderings if citation-number is not
        // used in the style.
        let seq = (pos + 1) as i64;
        if let Some(t) = state.registry.registry.get_mut(&id) {
            t.seq = seq;
        }
        if descending {
            if let Some(Value::Object(map)) = state.bibliography_sort.tmp.get_mut("citation_number_map") {
                map.insert(seq.to_string(), Value::from(len as i64 - seq + 1));
            }
        }
        // update_mode is set to CSL.NUMERIC if citation-number is rendered
        // in citations.
        let changed = state.registry.oldseq.get(&id) != Some(&seq);
        if update_mode == Some(load::NUMERIC) && changed {
            state.tmp.tainted_item_ids.insert(id.clone(), true);
        }
        if changed {
            state.registry.return_data.bibchange = true;
        }
    }
}

/// `registry.setsortkeys()`.
pub fn setsortkeys(state: &mut State) -> CslResult<()> {
    //
    // 17. Set sort keys on each item token.
    //
    let mylist = state.registry.mylist.clone();
    for key in mylist {
        // The last of these conditions may create some thrashing on styles that do not require sorting.
        let token = state
            .registry
            .registry
            .get(&key)
            .ok_or_else(|| type_error("Cannot read properties of undefined (reading 'sortkeys')"))?;
        if state.registry.touched.contains(&key)
            || state.tmp.tainted_item_ids.get(&key).copied().unwrap_or(false)
            || token.sortkeys.is_none()
        {
            let item = retrieve_item(state, &key)?;
            let keys = state.get_sort_keys(&item, "bibliography_sort")?;
            if let Some(t) = state.registry.registry.get_mut(&key) {
                t.sortkeys = Some(keys_to_options(&keys));
            }
        }
    }
    Ok(())
}

/// The strings and `undefined`s of a key array as `Option<String>`s.
pub fn keys_to_options(keys: &[Value]) -> Vec<Option<String>> {
    keys.iter()
        .map(|k| match k {
            Value::String(s) => Some(s.clone()),
            _ => None,
        })
        .collect()
}

/// `registry._locationOf(element, array, start, end)`: binary search for the
/// position after which `element` belongs in `array` (a list of ids).
fn location_of(
    state: &State,
    element: &str,
    array: &[String],
    start: usize,
    end: usize,
) -> CslResult<i64> {
    if array.is_empty() {
        return Ok(-1);
    }
    // `start = start || 0; end = end || array.length;`
    let end = if end == 0 { array.len() } else { end };
    let pivot = (start + end) >> 1;
    let reg = &state.registry;
    let c = reg
        .sorter
        .compare_keys(state, reg.keyed(element), reg.keyed(&array[pivot]))?;
    if end - start <= 1 {
        return Ok(if c == -1 { pivot as i64 - 1 } else { pivot as i64 });
    }
    match c {
        -1 => location_of(state, element, array, start, pivot),
        0 => Ok(pivot as i64),
        1 => location_of(state, element, array, pivot, end),
        // (no case matches: upstream returns undefined)
        _ => Ok(-1),
    }
}

/// `registry._insertItem(element, array)`.
fn insert_item(state: &mut State, element: &str, array: &mut Vec<String>) -> CslResult<()> {
    let loc = location_of(state, element, array, 0, 0)?;
    array.insert((loc + 1).max(0) as usize, element.to_string());
    Ok(())
}

/// `registry.sorttokens(nosort)`.
pub fn sorttokens(state: &mut State, nosort: bool) -> CslResult<()> {
    //
    // 18. Resort token list.
    //
    if nosort {
        return Ok(());
    }
    let mylist = state.registry.mylist.clone();
    state.registry.reflist_inserts = Vec::new();
    for item in &mylist {
        let token = state
            .registry
            .registry
            .get(item)
            .ok_or_else(|| type_error("Cannot read properties of undefined (reading 'newItem')"))?;
        if token.new_item {
            state.registry.reflist_inserts.push(item.clone());
        }
    }
    // There is a thin possibility that tainted items in a sorted list
    // will change position due to disambiguation. We cover for that here.
    let tainted: Vec<String> = state.tmp.tainted_item_ids.keys().cloned().collect();
    for key in tainted {
        let is_old = state
            .registry
            .registry
            .get(&key)
            .map(|t| !t.new_item)
            .unwrap_or(false);
        if is_old {
            // Move tainted items from reflist to reflist_inserts
            let mut i = state.registry.reflist.len();
            while i > 0 {
                i -= 1;
                if state.registry.reflist[i] == key {
                    let id = state.registry.reflist.remove(i);
                    state.registry.reflist_inserts.push(id);
                }
            }
        }
    }
    let inserts = state.registry.reflist_inserts.clone();
    let mut reflist = std::mem::take(&mut state.registry.reflist);
    for id in inserts {
        if let Some(t) = state.registry.registry.get_mut(&id) {
            t.new_item = false;
        }
        insert_item(state, &id, &mut reflist)?;
    }
    state.registry.reflist = reflist;
    for (pos, item) in mylist.iter().enumerate() {
        if let Some(t) = state.registry.registry.get_mut(item) {
            t.seq = (pos + 1) as i64;
        }
    }
    Ok(())
}

/// `registry.compareRegistryTokens(a, b)`.
pub fn compare_registry_tokens(a: &RegistryItem, b: &RegistryItem) -> i32 {
    if a.seq > b.seq {
        1
    } else if a.seq < b.seq {
        -1
    } else {
        0
    }
}

/// `registry.registerAmbigToken(akey, id, ambig_config)`.
pub fn register_ambig_token(
    state: &mut State,
    akey: &str,
    id: &str,
    ambig_config: AmbigId,
) -> CslResult<()> {
    // Taint if number of names to be included has changed
    let old_id = state.registry.registry.get(id).and_then(|t| t.disambig);
    if let Some(old_id) = old_id {
        let new = state.ambig(ambig_config).clone();
        let old = state.ambig(old_id).clone();
        let mut taint = false;
        for i in 0..new.names.len() {
            let new_names_params = new.names.get(i);
            let old_names_params = old.names.get(i);
            if new_names_params != old_names_params {
                taint = true;
            } else if let Some(new_g) = new.givens.get(i) {
                // Compare givenses only if the number of names is aligned.
                let old_g = old.givens.get(i).ok_or_else(|| {
                    type_error("Cannot read properties of undefined (reading 'givens[i][j]')")
                })?;
                for j in 0..new_g.len() {
                    if new_g.get(j) != old_g.get(j) {
                        taint = true;
                    }
                }
            }
        }
        if taint {
            state.tmp.tainted_item_ids.insert(id.to_string(), true);
        }
    }

    let pool = state.registry.ambigcites.entry(akey.to_string()).or_default();
    if !pool.iter().any(|x| x == id) {
        pool.push(id.to_string());
    }
    let cloned = state.clone_ambig(ambig_config, None);
    let token = state
        .registry
        .registry
        .get_mut(id)
        .ok_or_else(|| type_error("Cannot set properties of undefined (setting 'ambig')"))?;
    token.ambig = Some(akey.to_string());
    token.disambig = Some(cloned);
    Ok(())
}

impl State {
    /// `CSL.getSortKeys.call(state, Item, key_type)`: render the sort-key
    /// area `key_type` (`"bibliography_sort"` or `"citation_sort"`) for
    /// `Item` and return the keys it produced (strings, or `null` for
    /// `undefined`), with leading `a`, `an` and `the` stripped.
    pub fn get_sort_keys(&mut self, item: &Value, key_type: &str) -> CslResult<Vec<Value>> {
        let area = self.tmp.area.clone();
        let root = self.tmp.root.clone();
        let extension = self.tmp.extension.clone();
        self.tmp.area = key_type.to_string();
        // Gawdawful, this.
        self.tmp.root = if key_type.contains('_') {
            js::slice(key_type, 0, Some(-5))
        } else {
            key_type.to_string()
        };
        self.tmp.extension = "_sort".to_string();
        self.tmp.disambig_override = true;
        self.tmp.disambig_request = None;
        self.tmp.suppress_decorations = true;
        let r = self.get_cite(item, &Value::Null, None, false);
        self.tmp.suppress_decorations = false;
        self.tmp.disambig_override = false;
        r?;
        let keys: Vec<Value> = self
            .area_ref(key_type)
            .keys
            .iter()
            .map(strip_prepositions)
            .collect();
        self.area_mut(key_type).keys = keys.clone();

        self.tmp.area = area;
        self.tmp.root = root;
        self.tmp.extension = extension;
        Ok(keys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keyed(keys: &[Option<&str>], seq: Option<i64>) -> (Vec<Option<String>>, Option<i64>) {
        (
            keys.iter().map(|k| k.map(str::to_string)).collect(),
            seq,
        )
    }

    fn state_with_directions(dirs: &[[i64; 2]]) -> State {
        let mut s = State::default();
        s.bibliography_sort.opt.insert(
            "sort_directions".into(),
            Value::Array(
                dirs.iter()
                    .map(|d| Value::Array(vec![Value::from(d[0]), Value::from(d[1])]))
                    .collect(),
            ),
        );
        s
    }

    fn cmp(s: &State, a: &(Vec<Option<String>>, Option<i64>), b: &(Vec<Option<String>>, Option<i64>)) -> i32 {
        Comparifier::new(s, "bibliography_sort")
            .compare_keys(
                s,
                SortKeyed { sortkeys: &a.0, seq: a.1 },
                SortKeyed { sortkeys: &b.0, seq: b.1 },
            )
            .unwrap()
    }

    #[test]
    fn compare_keys_orders_by_key_then_direction_then_seq() {
        let s = state_with_directions(&[[-1, 1], [1, -1]]);
        let a = keyed(&[Some("Alpha"), Some("x")], Some(2));
        let b = keyed(&[Some("beta"), Some("x")], Some(1));
        assert_eq!(cmp(&s, &a, &b), -1);
        assert_eq!(cmp(&s, &b, &a), 1);
        // equal first key (case-insensitively), descending second key
        let c = keyed(&[Some("ALPHA"), Some("y")], Some(3));
        assert_eq!(cmp(&s, &a, &c), 1);
        // all keys equal: fall back to seq
        let d = keyed(&[Some("alpha"), Some("x")], Some(9));
        assert_eq!(cmp(&s, &a, &d), -1);
        assert_eq!(cmp(&s, &a, &a), 0);
        // undefined keys: an undefined `a` key takes direction [1], an undefined `b` key [0]
        let u = keyed(&[None, Some("x")], Some(1));
        assert_eq!(cmp(&s, &u, &a), 1);
        assert_eq!(cmp(&s, &a, &u), -1);
        // keys beyond the directions fail like upstream's TypeError
        let long = keyed(&[Some("a"), Some("b"), Some("c")], Some(1));
        let longer = keyed(&[Some("a"), Some("b"), Some("d")], Some(1));
        let comparifier = Comparifier::new(&s, "bibliography_sort");
        assert!(comparifier
            .compare_keys(
                &s,
                SortKeyed { sortkeys: &long.0, seq: long.1 },
                SortKeyed { sortkeys: &longer.0, seq: longer.1 }
            )
            .is_err());
    }

    #[test]
    fn init_removes_earlier_duplicates_and_resets_the_scratch_sets() {
        let mut s = State::default();
        s.registry.refreshes.insert("x".into());
        init(&mut s, vec!["a".into(), "b".into(), "a".into(), "c".into()], false);
        assert_eq!(s.registry.mylist, vec!["b", "a", "c"]);
        assert!(s.registry.refreshes.is_empty());
        init(&mut s, vec!["z".into(), "b".into()], true);
        assert_eq!(s.registry.mylist, vec!["b", "a", "c", "z"]);
        assert!(s.registry.uncited.contains("z") && s.registry.uncited.contains("b"));
        // a following ordinary init keeps the uncited ones
        init(&mut s, vec!["q".into()], false);
        assert_eq!(s.registry.mylist, vec!["q", "b", "z"]);
    }

    #[test]
    fn sort_with_is_stable_and_reports_the_first_error() {
        let mut v = vec![(2, 'a'), (1, 'b'), (2, 'c'), (1, 'd')];
        sort_with(&mut v, |x, y| Ok((x.0 - y.0) as i32)).unwrap();
        assert_eq!(v, vec![(1, 'b'), (1, 'd'), (2, 'a'), (2, 'c')]);
        let mut w = vec![3, 2, 1];
        assert!(sort_with(&mut w, |_, _| Err(EngineError::Csl("x".into()))).is_err());
    }

    #[test]
    fn location_of_finds_the_insertion_point_like_the_binary_search_upstream() {
        let mut s = state_with_directions(&[[-1, 1]]);
        s.registry.sorter = Comparifier::new(&s, "bibliography_sort");
        for (id, key, seq) in [("a", "apple", 1), ("c", "cherry", 2), ("e", "elder", 3), ("b", "banana", 4), ("z", "zebra", 5), ("0", "aardvark", 6)] {
            let mut t = RegistryItem::new(id);
            t.sortkeys = Some(vec![Some(key.to_string())]);
            t.seq = seq;
            s.registry.registry.insert(id.to_string(), t);
        }
        let mut list: Vec<String> = vec![];
        for id in ["a", "c", "e", "b", "z", "0"] {
            insert_item(&mut s, id, &mut list).unwrap();
        }
        assert_eq!(list, vec!["0", "a", "b", "c", "e", "z"]);
    }

    #[test]
    fn registry_item_defaults_are_those_of_doinserts() {
        let t = RegistryItem::new("x");
        assert!(t.new_item && t.seq == 0 && t.sortkeys.is_none() && t.ambig.is_none());
        assert_eq!(compare_registry_tokens(&t, &RegistryItem::new("y")), 0);
    }
}
