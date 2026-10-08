// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_disambig.js
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

//! `CSL.ambigConfigDiff`, `CSL.cloneAmbigConfig`, `CSL.getAmbigConfig`,
//! `CSL.getMaxVals`, `CSL.getMinVal`, and the pool accessors that stand for
//! the JS object sharing of [`AmbigConfig`] (see [`AmbigId`]).

use serde_json::Value;

use super::obj_ambigconfig::{AmbigConfig, AmbigId};
use super::state::State;
use super::{CslResult, EngineError};

/// `CSL.cloneAmbigConfig(config, oldconfig)`: a copy of `names`, `givens`,
/// `year_suffix` and `disambiguate` (taken from `oldconfig` when given, else
/// from `config`). Upstream's clone is a plain object with just those four
/// properties, so `maxvals`, `minval` and `use_initials` come back as the
/// constructor defaults.
pub fn clone_ambig_config(config: &AmbigConfig, oldconfig: Option<&AmbigConfig>) -> AmbigConfig {
    let source = oldconfig.unwrap_or(config);
    AmbigConfig {
        names: config.names.clone(),
        givens: config.givens.clone(),
        year_suffix: source.year_suffix.clone(),
        disambiguate: source.disambiguate.clone(),
        ..AmbigConfig::default()
    }
}

/// JS `Number(array)` for the `ppos < llen` test of `CSL.ambigConfigDiff`,
/// where upstream wrote `llen = a.givens[pos]` (the array, not its length):
/// `[]` is 0, `[n]` is `n`, longer arrays are `NaN` (the loop never runs).
fn givens_loop_bound(givens: &[i64]) -> f64 {
    match givens.len() {
        0 => 0.0,
        1 => givens[0] as f64,
        _ => f64::NAN,
    }
}

/// `CSL.ambigConfigDiff(a, b)`: `true` (JS `1`) when the configs differ.
///
/// Reproduces upstream's loop bound `llen = a.givens[pos]` (an array, so the
/// loop compares the first `n` given-name levels only when the name has
/// exactly one level `n`): candidate quirk, see DEVIATIONS.md. Where upstream
/// would throw reading `b.givens[pos][ppos]` of a missing array, this
/// returns an error.
pub fn ambig_config_diff(a: &AmbigConfig, b: &AmbigConfig) -> CslResult<bool> {
    // return of true means the ambig configs differ
    if a.names.len() != b.names.len() {
        return Ok(true);
    }
    for pos in 0..a.names.len() {
        if a.names[pos] != b.names[pos] {
            return Ok(true);
        }
        let llen = a.givens.get(pos).map(|g| givens_loop_bound(g)).unwrap_or(f64::NAN);
        let mut ppos = 0usize;
        while (ppos as f64) < llen {
            let av = a.givens.get(pos).and_then(|g| g.get(ppos));
            let bg = b.givens.get(pos).ok_or_else(|| {
                EngineError::Csl(
                    "TypeError: Cannot read properties of undefined (reading 'givens[pos][ppos]')"
                        .to_string(),
                )
            })?;
            if av != bg.get(ppos) {
                return Ok(true);
            }
            ppos += 1;
        }
    }
    // `a.disambiguate != b.disambiguate` (loose)
    if !loose_equal_flag(&a.disambiguate, &b.disambiguate) {
        return Ok(true);
    }
    if a.year_suffix != b.year_suffix {
        return Ok(true);
    }
    Ok(false)
}

/// JS `a == b` for the values `disambiguate` takes: `false`, `true` and
/// numbers (`false == 0`, `true == 1`).
fn loose_equal_flag(a: &Value, b: &Value) -> bool {
    fn num(v: &Value) -> Option<f64> {
        match v {
            Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            Value::Number(n) => n.as_f64(),
            Value::Null => None,
            _ => None,
        }
    }
    match (num(a), num(b)) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}

impl State {
    /// Put `config` in the pool and return its handle.
    pub fn alloc_ambig(&mut self, config: AmbigConfig) -> AmbigId {
        self.registry.ambig_pool.push(config);
        AmbigId(self.registry.ambig_pool.len() - 1)
    }

    /// The config behind `id`.
    pub fn ambig(&self, id: AmbigId) -> &AmbigConfig {
        &self.registry.ambig_pool[id.0]
    }

    /// The config behind `id`, mutably (writes are seen by every holder of `id`).
    pub fn ambig_mut(&mut self, id: AmbigId) -> &mut AmbigConfig {
        &mut self.registry.ambig_pool[id.0]
    }

    /// `CSL.cloneAmbigConfig(config, oldconfig)` into a new pool entry.
    pub fn clone_ambig(&mut self, config: AmbigId, oldconfig: Option<AmbigId>) -> AmbigId {
        let cloned = clone_ambig_config(self.ambig(config), oldconfig.map(|o| self.ambig(o)));
        self.alloc_ambig(cloned)
    }

    /// `state.tmp.disambig_settings`, allocated on first use (upstream sets it
    /// to `false` in places and `citeStart` always replaces it before use).
    pub fn disambig_settings_id(&mut self) -> AmbigId {
        match self.tmp.disambig_settings {
            Some(id) => id,
            None => {
                let id = self.alloc_ambig(AmbigConfig::default());
                self.tmp.disambig_settings = Some(id);
                id
            }
        }
    }

    /// `state.tmp.disambig_settings`.
    pub fn disambig_settings(&mut self) -> &AmbigConfig {
        let id = self.disambig_settings_id();
        self.ambig(id)
    }

    /// `state.tmp.disambig_settings`, mutably: the name code writes
    /// `names[pos]` and `givens[pos]` here.
    pub fn disambig_settings_mut(&mut self) -> &mut AmbigConfig {
        let id = self.disambig_settings_id();
        self.ambig_mut(id)
    }

    /// `state.tmp.disambig_request` (`None` is JS `false`).
    pub fn disambig_request(&self) -> Option<&AmbigConfig> {
        self.tmp.disambig_request.map(|id| self.ambig(id))
    }

    /// `CSL.getAmbigConfig.call(state)`: a clone of `tmp.disambig_request`, or
    /// of `tmp.disambig_settings` when there is no request.
    pub fn get_ambig_config(&mut self) -> AmbigId {
        let config = match self.tmp.disambig_request {
            Some(id) => id,
            None => self.disambig_settings_id(),
        };
        self.clone_ambig(config, None)
    }

    /// `CSL.getMaxVals.call(state)`: a copy of `tmp.names_max.mystack`.
    pub fn get_max_vals(&self) -> Vec<Value> {
        self.tmp.names_max.mystack.clone()
    }

    /// `CSL.getMinVal.call(state)`: `tmp["et-al-min"]` (`None` is `undefined`).
    pub fn get_min_val(&self) -> Option<Value> {
        self.tmp.et_al_min.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cfg(names: &[i64], givens: &[&[i64]], ys: Value, dis: Value) -> AmbigConfig {
        AmbigConfig {
            names: names.to_vec(),
            givens: givens.iter().map(|g| g.to_vec()).collect(),
            year_suffix: ys,
            disambiguate: dis,
            ..AmbigConfig::default()
        }
    }

    #[test]
    fn clone_copies_four_fields_and_resets_the_rest() {
        let mut c = cfg(&[2, 1], &[&[0, 1], &[2]], json!("3"), json!(true));
        c.minval = 9;
        c.use_initials = Some(true);
        let k = clone_ambig_config(&c, None);
        assert_eq!(k.names, vec![2, 1]);
        assert_eq!(k.givens, vec![vec![0, 1], vec![2]]);
        assert_eq!(k.year_suffix, json!("3"));
        assert_eq!(k.disambiguate, json!(true));
        assert_eq!((k.minval, k.use_initials), (1, None));
        let old = cfg(&[], &[], json!(false), json!(5));
        let k2 = clone_ambig_config(&c, Some(&old));
        assert_eq!((k2.year_suffix, k2.disambiguate), (json!(false), json!(5)));
    }

    #[test]
    fn diff_follows_upstream_including_the_array_loop_bound() {
        let a = cfg(&[2], &[&[0, 1]], json!(false), json!(0));
        assert!(!ambig_config_diff(&a, &a.clone()).unwrap());
        // different number of nameset entries
        assert!(ambig_config_diff(&a, &cfg(&[2, 1], &[], json!(false), json!(0))).unwrap());
        // names differ
        assert!(ambig_config_diff(&a, &cfg(&[3], &[&[0, 1]], json!(false), json!(0))).unwrap());
        // `disambiguate` is compared loosely: false == 0, but 0 != 1
        assert!(!ambig_config_diff(&a, &cfg(&[2], &[&[0, 1]], json!(false), json!(false))).unwrap());
        assert!(ambig_config_diff(&a, &cfg(&[2], &[&[0, 1]], json!(false), json!(1))).unwrap());
        // `year_suffix` is compared strictly: false !== "0"
        assert!(ambig_config_diff(&a, &cfg(&[2], &[&[0, 1]], json!("0"), json!(0))).unwrap());
        // upstream's `llen = a.givens[pos]`: a one-element array [2] loops twice,
        // so a differing second level is seen only through the bound.
        let one = cfg(&[1], &[&[2]], json!(false), json!(0));
        let other = cfg(&[1], &[&[1]], json!(false), json!(0));
        assert!(ambig_config_diff(&one, &other).unwrap());
        // a two-element array is NaN: the loop never runs, so differing levels are not seen.
        let two_a = cfg(&[1], &[&[1, 2]], json!(false), json!(0));
        let two_b = cfg(&[1], &[&[2, 1]], json!(false), json!(0));
        assert!(!ambig_config_diff(&two_a, &two_b).unwrap());
    }

    #[test]
    fn pool_sharing_is_visible_through_every_holder() {
        let mut s = State::default();
        let id = s.alloc_ambig(AmbigConfig::default());
        s.tmp.disambig_request = Some(id);
        s.tmp.disambig_settings = Some(id);
        s.disambig_settings_mut().names.push(3);
        assert_eq!(s.disambig_request().map(|c| c.names.clone()), Some(vec![3]));
        let k = s.get_ambig_config();
        s.ambig_mut(k).names.push(4);
        assert_eq!(s.ambig(id).names, vec![3]);
        s.tmp.names_max.push_literal(json!(2));
        assert_eq!(s.get_max_vals(), vec![json!(2)]);
        assert_eq!(s.get_min_val(), None);
    }
}
