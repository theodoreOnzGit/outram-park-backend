// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/disambig_cites.js (CSL.Disambiguation)
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

//! `CSL.Disambiguation`: resolves the ambiguous cites of one `ambigcites`
//! group by adding names, given-name levels, `disambiguate="true"` text and
//! year-suffixes until the cites differ (or the modes run out).
//!
//! The JS object keeps its working state in properties that survive from one
//! `run` to the next (`namesetsMax`, `givensMax`, ... are only reassigned
//! where upstream reassigns them); the struct keeps them the same way.
//! [`run`] takes the disambiguator out of `state.disambiguate` while it works
//! (it needs `&mut State` to render cites), so code that runs during a render
//! sees a default one: it must only use [`Disambiguation::pad_base`], which
//! reads nothing from it.
//!
//! Items in the lists are ids; the item data is `registry.refhash[id]`
//! (`refetchItem`), which does not change during a run.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde_json::Value;

use super::js;
use super::obj_ambigconfig::{AmbigConfig, AmbigId};
use super::registry::{register_ambig_token, sort_with};
use super::state::State;
use super::util_disambig::ambig_config_diff;
use super::{CslResult, EngineError};

/// The disambiguation modes (the method names `configModes` pushes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisMode {
    /// `"disNames"`.
    DisNames,
    /// `"disExtraText"`.
    DisExtraText,
    /// `"disYears"`.
    DisYears,
}

/// One entry of `this.lists`: `[config, items]`.
pub type DisList = (AmbigId, Vec<String>);

/// `new CSL.Disambiguation(state)`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Disambiguation {
    /// `modes`.
    pub modes: Vec<DisMode>,
    /// `akey`.
    pub akey: String,
    /// `lists`.
    pub lists: Vec<DisList>,
    /// `base` (`None` is `false`).
    pub base: Option<AmbigId>,
    /// `betterbase`.
    pub betterbase: Option<AmbigId>,
    /// `maxNamesByItemId`.
    pub max_names_by_item_id: BTreeMap<String, Vec<Value>>,
    /// `Item`: the id of the item being scanned.
    pub item: String,
    /// `ItemCite`.
    pub item_cite: String,
    /// `partners`.
    pub partners: Vec<String>,
    /// `nonpartners`.
    pub nonpartners: Vec<String>,
    /// `modeindex`.
    pub modeindex: usize,
    /// `namesMax` (`None` when not a number).
    pub names_max: Option<i64>,
    /// `namesetsMax`.
    pub namesets_max: Option<i64>,
    /// `givensMax`.
    pub givens_max: Option<i64>,
    /// `gnameset`.
    pub gnameset: i64,
    /// `gname`.
    pub gname: i64,
    /// `clashes`.
    pub clashes: [i64; 2],
    /// `listpos`.
    pub listpos: usize,
    /// `initGivens`.
    pub init_givens: bool,
}

fn type_error(what: &str) -> EngineError {
    EngineError::Csl(format!("TypeError: {what}"))
}

/// JS `a > b` / `a < b` on two item ids (`Item.id`: strings or numbers), as an
/// ordering: `Equal` for "neither" (including `NaN` comparisons). Two strings
/// compare by UTF-16 code units, whatever they look like; a string against a
/// number is converted first.
fn compare_ids(a: Option<&Value>, b: Option<&Value>) -> Ordering {
    match (a, b) {
        (Some(Value::String(x)), Some(Value::String(y))) => x.encode_utf16().cmp(y.encode_utf16()),
        (Some(x), Some(y)) => match (as_js_number(x), as_js_number(y)) {
            (Some(p), Some(q)) => p.partial_cmp(&q).unwrap_or(Ordering::Equal),
            _ => Ordering::Equal,
        },
        _ => Ordering::Equal,
    }
}

impl Disambiguation {
    /// `new CSL.Disambiguation(state)` and its `configModes()`.
    pub fn new(state: &State) -> Disambiguation {
        let mut modes = Vec::new();
        // Modes are function names prototyped to this instance.
        let dagopt = js::truthy_opt(state.opt.get("disambiguate-add-givenname"));
        let gdropt = js::get_str(&state.citation.opt, "givenname-disambiguation-rule");
        if js::truthy_opt(state.opt.get("disambiguate-add-names"))
            || (dagopt && gdropt == Some("by-cite"))
        {
            modes.push(DisMode::DisNames);
        }
        let has_disambiguate = js::truthy_opt(state.opt.get("has_disambiguate"));
        let year_suffix = js::truthy_opt(state.opt.get("disambiguate-add-year-suffix"));
        if state.dev_ext("prioritize_disambiguate_condition") {
            if has_disambiguate {
                modes.push(DisMode::DisExtraText);
            }
            if year_suffix {
                modes.push(DisMode::DisYears);
            }
        } else {
            if year_suffix {
                modes.push(DisMode::DisYears);
            }
            if has_disambiguate {
                modes.push(DisMode::DisExtraText);
            }
        }
        Disambiguation {
            modes,
            ..Disambiguation::default()
        }
    }

    /// `padBase(base)`: give every name of `base` a given-name level.
    /// Reads nothing from the disambiguator, so it is an associated function
    /// as well as a method ([`Disambiguation::pad_base`]).
    pub fn pad_base_config(base: &mut AmbigConfig) {
        for i in 0..base.names.len() {
            while base.givens.len() <= i {
                base.givens.push(Vec::new());
            }
            let wanted = base.names[i].max(0) as usize;
            for j in 0..wanted {
                if base.givens[i].len() <= j {
                    base.givens[i].push(0);
                }
            }
        }
    }

    /// `padBase(base)` as the method name the name code uses
    /// (`state.disambiguate.padBase(state.tmp.disambig_settings)`).
    pub fn pad_base(&self, base: &mut AmbigConfig) {
        Disambiguation::pad_base_config(base);
    }

    fn pad(state: &mut State, id: Option<AmbigId>) {
        if let Some(id) = id {
            Disambiguation::pad_base_config(state.ambig_mut(id));
        }
    }

    /// The config `which` names, as an error when upstream's `false` would
    /// be dereferenced.
    fn cfg(which: Option<AmbigId>, what: &str) -> CslResult<AmbigId> {
        which.ok_or_else(|| type_error(&format!("Cannot read properties of false ({what})")))
    }

    /// `run(akey)`.
    fn run_inner(&mut self, state: &mut State, akey: &str) -> CslResult<()> {
        if self.modes.is_empty() {
            return Ok(());
        }
        self.akey = akey.to_string();
        if self.init_vars(state, akey)? {
            self.run_disambig(state)?;
        }
        Ok(())
    }

    /// `runDisambig()`.
    fn run_disambig(&mut self, state: &mut State) -> CslResult<()> {
        self.init_givens = true;
        //
        // Length of list may change during processing
        while !self.lists.is_empty() {
            self.gnameset = 0;
            self.gname = 0;
            self.clashes = [1, 0];
            //
            // each list is scanned repeatedly until all
            // items either succeed or ultimately fail.
            while !self.lists[0].1.is_empty() {
                self.listpos = 0;
                if self.base.is_none() {
                    self.base = Some(self.lists[0].0);
                }
                let ismax = self.increment_disambig(state)?;
                let list = self.lists[0].clone();
                self.scan_items(state, &list)?;
                self.eval_scan(state, ismax)?;
            }
            self.lists.remove(0);
        }
        Ok(())
    }

    /// `scanItems(list)`.
    fn scan_items(&mut self, state: &mut State, list: &DisList) -> CslResult<()> {
        let first = list.1[0].clone();
        let first_item = refetch(state, &first)?;
        self.item = first.clone();
        self.item_cite = state.get_ambiguous_cite(&first_item, self.base, true, None)?;

        self.partners = vec![first];
        self.nonpartners = Vec::new();
        let mut clashes = 0;

        for other in list.1.iter().skip(1) {
            let other_item = refetch(state, other)?;
            let other_cite = state.get_ambiguous_cite(&other_item, self.base, true, None)?;
            if self.item_cite == other_cite {
                clashes += 1;
                self.partners.push(other.clone());
            } else {
                self.nonpartners.push(other.clone());
            }
        }
        self.clashes[0] = self.clashes[1];
        self.clashes[1] = clashes;
        Ok(())
    }

    /// `evalScan(maxed)`.
    fn eval_scan(&mut self, state: &mut State, maxed: bool) -> CslResult<()> {
        match self.modes[self.modeindex] {
            DisMode::DisNames => self.dis_names(state, maxed)?,
            DisMode::DisExtraText => self.dis_extra_text(state)?,
            DisMode::DisYears => self.dis_years(state)?,
        }
        if maxed {
            if self.modeindex < self.modes.len() - 1 {
                self.modeindex += 1;
            } else {
                // `this.lists[this.listpos + 1] = [this.base, []]`
                let entry = (Self::cfg(self.base, "base")?, Vec::new());
                let at = self.listpos + 1;
                if at < self.lists.len() {
                    self.lists[at] = entry;
                } else {
                    self.lists.push(entry);
                }
            }
        }
        Ok(())
    }

    /// `register(id, config)`: `registry.registerAmbigToken(akey, "" + id, config)`.
    fn register(&self, state: &mut State, id: &str, config: AmbigId) -> CslResult<()> {
        register_ambig_token(state, &self.akey, id, config)
    }

    /// `disNames(ismax)`.
    fn dis_names(&mut self, state: &mut State, ismax: bool) -> CslResult<()> {
        // New design
        // this.base is a forward-only counter. Values are never
        // reduced, and the counter object is never overwritten.
        // It is methodically pushed forward in single-unit increments
        // in incrementDisambig() until disNames() wipes out the list.

        // this.betterbase is cloned from this.base exactly once,
        // at the start of a disambiguation run. Whenever an operation
        // results in improvement, the just-incremented elements
        // identified as this.base.names[this.gnameset] (number of
        // names)and as this.base.givens[this.gnameset][this.gname]
        // (level of given name) are copied from this.base.
        let betterbase = Self::cfg(self.betterbase, "betterbase")?;
        if self.clashes[1] == 0 && self.nonpartners.len() == 1 {
            self.capture_step_to_base(state)?;
            // ** RESOLUTION [a]: lone partner, one nonpartner
            let (np, p) = (self.nonpartners[0].clone(), self.partners[0].clone());
            self.register(state, &np, betterbase)?;
            self.register(state, &p, betterbase)?;
            self.lists[self.listpos] = (betterbase, Vec::new());
        } else if self.clashes[1] == 0 {
            self.capture_step_to_base(state)?;
            // ** RESOLUTION [b]: lone partner, unknown number of remaining nonpartners
            let p = self.partners[0].clone();
            self.register(state, &p, betterbase)?;
            self.lists[self.listpos] = (betterbase, self.nonpartners.clone());
            if !self.nonpartners.is_empty() {
                self.init_givens = true;
            }
        } else if self.nonpartners.len() == 1 {
            self.capture_step_to_base(state)?;
            // ** RESOLUTION [c]: lone nonpartner, unknown number of partners remaining
            let np = self.nonpartners[0].clone();
            self.register(state, &np, betterbase)?;
            self.lists[self.listpos] = (betterbase, self.partners.clone());
        } else if self.clashes[1] < self.clashes[0] {
            self.capture_step_to_base(state)?;
            // ** RESOLUTION [d]: better result, but no entries safe to register
            self.lists[self.listpos] = (betterbase, self.partners.clone());
            self.lists.push((betterbase, self.nonpartners.clone()));
        } else {
            // ** RESOLUTION [e]: no improvement, and clashes remain
            if ismax {
                self.lists[self.listpos] = (betterbase, self.nonpartners.clone());
                self.lists.push((betterbase, self.partners.clone()));
                if self.modeindex == self.modes.len() - 1 {
                    // (registering clashing entries because we've run out of options)
                    for p in self.partners.clone() {
                        self.register(state, &p, betterbase)?;
                    }
                    self.lists[self.listpos] = (betterbase, Vec::new());
                }
            }
        }
        Ok(())
    }

    /// `disExtraText()`.
    fn dis_extra_text(&mut self, state: &mut State) -> CslResult<()> {
        let mut done = false;

        if self.clashes[1] == 0 && self.nonpartners.len() < 2 {
            done = true;
        }

        let base_id = Self::cfg(self.base, "base")?;
        let better_id = Self::cfg(self.betterbase, "betterbase")?;
        // If first encounter in this cycle and multiple modes are
        // available, decrement mode and reset base
        let base_dis_falsy = !js::truthy(&state.ambig(base_id).disambiguate);
        let count = state.tmp.disambiguate_count;
        let max_max = state.tmp.disambiguate_max_max;
        if !done && (base_dis_falsy || count != max_max) {
            // Rerun everything on each subcycle? This doesn't work currently.
            self.modeindex = 0;
            state.ambig_mut(base_id).disambiguate = Value::from(count);
            state.ambig_mut(better_id).disambiguate = Value::from(count);
            if !js::truthy(&state.ambig(base_id).disambiguate) {
                // Evaluate here?
                self.init_givens = true;
                // If disambiguate is false set to true
                state.ambig_mut(base_id).disambiguate = Value::from(1);
                // There may be changes
                for id in &self.lists[self.listpos].1 {
                    state.tmp.tainted_item_ids.insert(id.clone(), true);
                }
            } else {
                self.dis_names(state, false)?;
            }
        } else if done || count == max_max {
            if done || self.modeindex == self.modes.len() - 1 {
                // If this is the end, disambiguation failed.
                // Discard disambiguate=true (?) and set parameters
                let base = self.lists[self.listpos].0;
                for id in self.lists[self.listpos].1.clone() {
                    state.tmp.tainted_item_ids.insert(id.clone(), true);
                    self.register(state, &id, base)?;
                }
                self.lists[self.listpos] = (better_id, Vec::new());
            } else {
                // If this is followed by year-suffix, keep
                // parameters and set disambiguate=true since it MIGHT
                // include the date, needed for year-suffix.
                // This may be a bit over-aggressive for cases in which the
                // disambiguate condition does not add the date
                self.modeindex = self.modes.len() - 1;
                let base = self.lists[self.listpos].0;
                state.ambig_mut(base).disambiguate = Value::Bool(true);
                for id in self.lists[self.listpos].1.clone() {
                    // Always tainting here might be a little over-aggressive, but a taint may be required.
                    state.tmp.tainted_item_ids.insert(id.clone(), true);
                    self.register(state, &id, base)?;
                }
            }
        }
        Ok(())
    }

    /// `disYears()`.
    fn dis_years(&mut self, state: &mut State) -> CslResult<()> {
        let mut tokens: Vec<String> = Vec::new();
        let base = self.lists[self.listpos].0;
        if self.clashes[1] != 0 {
            // That is, if the initial increment on the ambigs group returns no
            // clashes, don't apply suffix. The condition is a necessary failsafe.
            // In original submission order
            for origid in &state.registry.mylist {
                for token in &self.lists[self.listpos].1 {
                    // Warning: token.id can be number. This should be fixed at a higher level in citeproc-js if poss.
                    if token == origid {
                        tokens.push(token.clone());
                        break;
                    }
                }
            }
        }
        {
            let reg = &state.registry;
            let st: &State = state;
            sort_with(&mut tokens, |a, b| {
                reg.sorter.compare_keys(st, keyed_of(reg, a), keyed_of(reg, b))
            })?;
        }
        for (pos, id) in tokens.iter().enumerate() {
            state.ambig_mut(base).year_suffix = Value::String(pos.to_string());
            let old_base = state
                .registry
                .registry
                .get(id)
                .and_then(|t| t.disambig)
                .ok_or_else(|| type_error("Cannot read properties of undefined (reading 'disambig')"))?;
            self.register(state, id, base)?;
            if ambig_config_diff(&state.ambig(old_base).clone(), &state.ambig(base).clone())? {
                state.tmp.tainted_item_ids.insert(id.clone(), true);
            }
        }
        self.lists[self.listpos] = (Self::cfg(self.betterbase, "betterbase")?, Vec::new());
        Ok(())
    }

    /// `incrementDisambig()`: returns whether the counter is maxed out.
    fn increment_disambig(&mut self, state: &mut State) -> CslResult<bool> {
        if self.init_givens {
            self.init_givens = false;
            return Ok(false);
        }
        let mut maxed = false;
        let base_id = Self::cfg(self.base, "base")?;
        let better_id = Self::cfg(self.betterbase, "betterbase")?;
        if self.modes[self.modeindex] == DisMode::DisNames {
            // this.gnameset: the index pos of the current nameset
            // this.gname: the index pos of the current name w/in the current nameset

            // Stages:
            // - Increment givenname (optional)
            // - Add a name (optional)
            // - Move to next nameset

            // Incrementing is done forward-only on this.base. Values
            // that improve disambiguation results are copied to
            // this.betterbase, which is used to set the disambig
            // parameters in the processor registry.

            // Increment
            // Max val is always true if a level is inactive.
            let mut increment_names = false;
            if self.givens_max.is_none() {
                increment_names = true;
            }
            let mut increment_namesets = false;
            if self.names_max.is_none() {
                increment_namesets = true;
            }
            let (gnameset, gname) = (self.gnameset as usize, self.gname as usize);
            if let Some(givens_max) = self.givens_max {
                let givens_len = state.ambig(base_id).givens.len();
                let can_increment = if givens_len > 0 {
                    let row = state.ambig(base_id).givens.get(gnameset).ok_or_else(|| {
                        type_error("Cannot read properties of undefined (reading 'gname')")
                    })?;
                    row.get(gname).map(|v| *v < givens_max).unwrap_or(false)
                } else {
                    false
                };
                if can_increment {
                    state.ambig_mut(base_id).givens[gnameset][gname] += 1;
                } else {
                    increment_names = true;
                }
            }
            if let Some(names_max) = self.names_max {
                if increment_names {
                    if js::truthy_opt(state.opt.get("disambiguate-add-names")) {
                        increment_namesets = false;
                        if self.gname < names_max {
                            let b = state.ambig_mut(base_id);
                            let bumped = b.names.get(gnameset).copied().unwrap_or(0) + 1;
                            set_index(&mut b.names, gnameset, bumped);
                            self.gname += 1;
                        } else {
                            increment_namesets = true;
                        }
                    } else {
                        increment_namesets = true;
                    }
                }
            }
            if let Some(namesets_max) = self.namesets_max {
                if increment_namesets && self.gnameset < namesets_max {
                    self.gnameset += 1;
                    let b = state.ambig_mut(base_id);
                    set_index(&mut b.names, self.gnameset as usize, 1);
                    self.gname = 0;
                }
            }
            let (gnameset, gname) = (self.gnameset as usize, self.gname as usize);
            let add_names = js::truthy_opt(state.opt.get("disambiguate-add-names"));
            let base = state.ambig(base_id);
            let cond_a = match self.namesets_max {
                None => true,
                Some(n) => n == -1 || self.gnameset == n,
            };
            let cond_b = !add_names
                || match self.names_max {
                    None => true,
                    Some(n) => self.gname == n,
                };
            let cond_c = match self.givens_max {
                None => true,
                Some(gm) => match base.givens.get(gnameset).and_then(|r| r.get(gname)) {
                    None => true,
                    Some(v) => *v == gm,
                },
            };
            if cond_a && cond_b && cond_c {
                maxed = true;
            }
        } else if self.modes[self.modeindex] == DisMode::DisExtraText {
            let bump = |v: &Value| -> Value {
                // `x += 1`: false/undefined -> NaN-free in practice (set before use).
                let n = match v {
                    Value::Number(n) => n.as_f64().unwrap_or(0.0),
                    Value::Bool(true) => 1.0,
                    _ => 0.0,
                };
                Value::from((n + 1.0) as i64)
            };
            let nb = bump(&state.ambig(base_id).disambiguate);
            state.ambig_mut(base_id).disambiguate = nb;
            let nbb = bump(&state.ambig(better_id).disambiguate);
            state.ambig_mut(better_id).disambiguate = nbb;
        }
        Ok(maxed)
    }

    /// `initVars(akey)`: returns whether there is anything to disambiguate.
    fn init_vars(&mut self, state: &mut State, akey: &str) -> CslResult<bool> {
        self.lists = Vec::new();
        self.base = None;
        self.betterbase = None;
        self.akey = akey.to_string();

        self.max_names_by_item_id = BTreeMap::new();

        let my_ids: Vec<String> = match state.registry.ambigcites.get(akey) {
            Some(v) if !v.is_empty() => v.clone(),
            _ => return Ok(false),
        };
        let my_item = refetch(state, &my_ids[0])?;
        self.get_cite_data(state, &my_item, None)?;
        self.base = Some(state.get_ambig_config());
        if my_ids.len() > 1 {
            // Build a composite list of Items and associated
            // max names. This is messy, but it's the only
            // way to get the items sorted by the number of names
            // to be disambiguated. If they are in descending order
            // with name expansions, the processor will hang.
            let mut bundles: Vec<(Vec<Value>, Value)> = Vec::new();
            bundles.push((self.max_names_of(&my_item), my_item));
            for id in my_ids.iter().skip(1) {
                let item = refetch(state, id)?;
                self.get_cite_data(state, &item, self.base)?;
                bundles.push((self.max_names_of(&item), item));
            }
            // `a[0] > b[0]` compares the max-names ARRAYS, as strings
            // ("10" < "2"): candidate quirk C16 in DEVIATIONS.md.
            bundles.sort_by(|a, b| {
                let (sa, sb) = (js::to_js_string(&Value::Array(a.0.clone())), js::to_js_string(&Value::Array(b.0.clone())));
                match sa.encode_utf16().cmp(sb.encode_utf16()) {
                    Ordering::Equal => compare_ids(a.1.get("id"), b.1.get("id")),
                    other => other,
                }
            });
            let my_items: Vec<String> = bundles
                .iter()
                .map(|(_, item)| item_id_string(item))
                .collect();
            let base = Self::cfg(self.base, "base")?;
            self.lists.push((base, my_items.clone()));
            self.item = my_items[0].clone();
        } else {
            self.item = my_ids[0].clone();
        }

        self.modeindex = 0;
        // `if (this.state.citation.opt["disambiguate-add-names"] || true)`
        self.names_max = self
            .max_names_by_item_id
            .get(&self.item)
            .and_then(|v| v.first())
            .and_then(|v| match v {
                Value::Number(n) => n.as_i64(),
                _ => None,
            });

        Self::pad(state, self.base);
        Self::pad(state, self.betterbase);
        let (base, better) = (Self::cfg(self.base, "base")?, Self::cfg(self.betterbase, "betterbase")?);
        state.ambig_mut(base).year_suffix = Value::Bool(false);
        state.ambig_mut(base).disambiguate = Value::Bool(false);
        state.ambig_mut(better).year_suffix = Value::Bool(false);
        state.ambig_mut(better).disambiguate = Value::Bool(false);
        if js::get_str(&state.citation.opt, "givenname-disambiguation-rule") == Some("by-cite")
            && js::truthy_opt(state.opt.get("disambiguate-add-givenname"))
        {
            self.givens_max = Some(2);
        }
        Ok(true)
    }

    fn max_names_of(&self, item: &Value) -> Vec<Value> {
        self.max_names_by_item_id
            .get(&item_id_string(item))
            .cloned()
            .unwrap_or_default()
    }

    /// `getCiteData(Item, base)`.
    fn get_cite_data(
        &mut self,
        state: &mut State,
        item: &Value,
        base: Option<AmbigId>,
    ) -> CslResult<()> {
        let id = item_id_string(item);
        // Initialize base if first set item seen
        if self.max_names_by_item_id.contains_key(&id) {
            return Ok(());
        }
        state.get_ambiguous_cite(item, base, false, None)?;
        let base = state.get_ambig_config();
        self.max_names_by_item_id
            .insert(id.clone(), state.get_max_vals());
        let settings_givens = state.disambig_settings().givens.clone();
        let reg_disambig = state
            .registry
            .registry
            .get(&id)
            .and_then(|t| t.disambig)
            .ok_or_else(|| type_error("Cannot read properties of undefined (reading 'disambig')"))?;
        // Slice the nested lists as well. Without this, disambiguate_YearSuffixFiftyTwoEntriesByCite fails.
        state.ambig_mut(reg_disambig).givens = settings_givens;
        self.namesets_max = Some(state.ambig(reg_disambig).names.len() as i64 - 1);
        if self.base.is_none() {
            self.base = Some(base);
            self.betterbase = Some(state.clone_ambig(base, None));
        }
        let this_base = Self::cfg(self.base, "base")?;
        if state.ambig(base).names.len() < state.ambig(this_base).names.len() {
            // I don't know what would happen with discrepancies in the number
            // of namesets rendered on items, so we use the fewer of the two
            // and limit the other to that size.
            self.base = Some(base);
        }
        // Padding. Within namesets, we use the longer of the two throughout.
        for i in 0..state.ambig(base).names.len() {
            let this_base = Self::cfg(self.base, "base")?;
            let better = Self::cfg(self.betterbase, "betterbase")?;
            let new_n = state.ambig(base).names[i];
            let old_n = state.ambig(this_base).names.get(i).copied();
            if old_n.map(|o| new_n > o).unwrap_or(false) {
                let row = state.ambig(base).givens.get(i).cloned().unwrap_or_default();
                {
                    let b = state.ambig_mut(this_base);
                    set_index(&mut b.givens, i, row);
                    set_index(&mut b.names, i, new_n);
                }
                let (names, givens) = {
                    let b = state.ambig(this_base);
                    (b.names.clone(), b.givens.clone())
                };
                {
                    let bb = state.ambig_mut(better);
                    bb.names = names;
                    bb.givens = givens;
                }
                Self::pad(state, self.base);
                Self::pad(state, self.betterbase);
            }
        }
        // This shouldn't be necessary
        // getAmbiguousCite() should return a valid and complete
        // givens segment under all conditions, but it does not
        // do so for institution authors, so we clean up after it
        // here.
        // Relevant test: sort_ChicagoYearSuffix2
        let this_base = Self::cfg(self.base, "base")?;
        let better = Self::cfg(self.betterbase, "betterbase")?;
        let givens = state.ambig(this_base).givens.clone();
        state.ambig_mut(better).givens = givens;
        Ok(())
    }

    /// `captureStepToBase()`.
    fn capture_step_to_base(&mut self, state: &mut State) -> CslResult<()> {
        let base_id = Self::cfg(self.base, "base")?;
        let better_id = Self::cfg(self.betterbase, "betterbase")?;
        let (gnameset, gname) = (self.gnameset as usize, self.gname as usize);
        // Be paranoid about the presence of givens
        if js::get_str(&state.citation.opt, "givenname-disambiguation-rule") == Some("by-cite")
            && !state.ambig(base_id).givens.is_empty()
        {
            let value = state
                .ambig(base_id)
                .givens
                .get(gnameset)
                .ok_or_else(|| type_error("Cannot read properties of undefined (reading 'gname')"))?
                .get(gname)
                .copied();
            if let Some(v) = value {
                if state.ambig(better_id).givens.len() < state.ambig(base_id).givens.len() {
                    let all = state.ambig(base_id).givens.clone();
                    state.ambig_mut(better_id).givens = all;
                }
                let row = state
                    .ambig_mut(better_id)
                    .givens
                    .get_mut(gnameset)
                    .ok_or_else(|| type_error("Cannot set properties of undefined (setting 'gname')"))?;
                set_index(row, gname, v);
            }
        }
        let n = state.ambig(base_id).names.get(gnameset).copied().unwrap_or(0);
        set_index(&mut state.ambig_mut(better_id).names, gnameset, n);
        Ok(())
    }
}

/// `arr[i] = v`, growing the array like JS (the holes JS would leave read as
/// `0` here).
fn set_index<T: Default + Clone>(arr: &mut Vec<T>, i: usize, v: T) {
    while arr.len() <= i {
        arr.push(T::default());
    }
    arr[i] = v;
}

fn refetch(state: &State, id: &str) -> CslResult<Value> {
    state
        .refetch_item(id)
        .ok_or_else(|| type_error("Cannot read properties of undefined (reading 'id')"))
}

/// `Item.id` as a string (the registry key).
fn item_id_string(item: &Value) -> String {
    item.get("id").map(js::to_js_string).unwrap_or_default()
}

/// JS `Number(v)` for an item id value: numbers as themselves, strings
/// converted (`None` is `NaN`), other values `NaN`.
fn as_js_number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => {
            let t = js::trim(s);
            if t.is_empty() {
                Some(0.0)
            } else {
                t.parse::<f64>().ok().filter(|f| f.is_finite())
            }
        }
        _ => None,
    }
}

fn keyed_of<'a>(
    reg: &'a super::registry::Registry,
    id: &str,
) -> super::registry::SortKeyed<'a> {
    match reg.registry.get(id) {
        Some(t) => super::registry::SortKeyed {
            sortkeys: t.sortkeys.as_deref().unwrap_or(&[]),
            seq: Some(t.seq),
        },
        None => super::registry::SortKeyed {
            sortkeys: &[],
            seq: None,
        },
    }
}

/// `state.disambiguate.run(akey)`.
pub fn run(state: &mut State, akey: &str) -> CslResult<()> {
    let mut d = std::mem::take(&mut state.disambiguate);
    let r = d.run_inner(state, akey);
    state.disambiguate = d;
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_base_gives_every_name_a_level() {
        let mut c = AmbigConfig {
            names: vec![2, 1],
            givens: vec![vec![2]],
            ..AmbigConfig::default()
        };
        Disambiguation::pad_base_config(&mut c);
        assert_eq!(c.givens, vec![vec![2, 0], vec![0]]);
    }

    #[test]
    fn modes_follow_the_style_options_and_their_order() {
        let mut s = State::default();
        assert!(Disambiguation::new(&s).modes.is_empty());
        s.opt.insert("disambiguate-add-names".into(), Value::Bool(true));
        s.opt
            .insert("disambiguate-add-year-suffix".into(), Value::Bool(true));
        s.opt.insert("has_disambiguate".into(), Value::Bool(true));
        assert_eq!(
            Disambiguation::new(&s).modes,
            vec![DisMode::DisNames, DisMode::DisYears, DisMode::DisExtraText]
        );
        s.opt.insert(
            "development_extensions".into(),
            serde_json::json!({"prioritize_disambiguate_condition": true}),
        );
        assert_eq!(
            Disambiguation::new(&s).modes,
            vec![DisMode::DisNames, DisMode::DisExtraText, DisMode::DisYears]
        );
    }

    #[test]
    fn item_ids_compare_like_js() {
        let j = |v: serde_json::Value| v;
        assert_eq!(compare_ids(Some(&j(serde_json::json!("ITEM-2"))), Some(&j(serde_json::json!("ITEM-10")))), Ordering::Greater);
        // two strings compare as strings, even numeric-looking ones
        assert_eq!(compare_ids(Some(&serde_json::json!("2")), Some(&serde_json::json!("10"))), Ordering::Greater);
        assert_eq!(compare_ids(Some(&serde_json::json!(2)), Some(&serde_json::json!(10))), Ordering::Less);
        assert_eq!(compare_ids(Some(&serde_json::json!(2)), Some(&serde_json::json!("x"))), Ordering::Equal);
        assert_eq!(compare_ids(Some(&serde_json::json!(2)), Some(&serde_json::json!("10"))), Ordering::Less);
        assert_eq!(as_js_number(&serde_json::json!(" 7 ")), Some(7.0));
        assert_eq!(as_js_number(&serde_json::json!("a")), None);
    }

    #[test]
    fn set_index_grows_like_a_js_array() {
        let mut v = vec![1i64];
        set_index(&mut v, 3, 9);
        assert_eq!(v, vec![1, 0, 0, 9]);
    }
}
