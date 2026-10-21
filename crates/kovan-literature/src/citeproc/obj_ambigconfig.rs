// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/obj_ambigconfig.js
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

//! `CSL.AmbigConfig`: the ambiguous-cite configuration of one item (how many
//! names, which given-name level per name, the year-suffix, the
//! `disambiguate` count). `registry.registry[id].disambig`,
//! `state.tmp.disambig_request` and `state.tmp.disambig_settings` are all
//! this object; the names code (util_names_*.js) reads and writes its fields
//! under upstream's own names.

use serde_json::Value;

/// A handle to an [`AmbigConfig`] in the registry's pool (`state.registry.ambig_pool`).
///
/// In citeproc-js the same `AmbigConfig` object is held by several owners at
/// once: `registry.registry[id].disambig`, `tmp.disambig_request` and
/// `tmp.disambig_settings` (`citeStart` aliases them), the disambiguator's
/// `base` and `betterbase`, and the entries of its `lists`. The name code
/// writes through `tmp.disambig_settings`, so those writes reach whichever
/// object it aliases. The port keeps that sharing exactly: configs live in a
/// pool and everything refers to them by id. Read and write through
/// [`State::ambig`](super::state::State::ambig) and
/// [`State::ambig_mut`](super::state::State::ambig_mut).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AmbigId(pub usize);

/// `new CSL.AmbigConfig()`.
///
/// `year_suffix` is `false` or the string `"" + pos` that
/// `Disambiguation::disYears` writes; `disambiguate` is `0` (the
/// constructor), `false` (`initVars`), `true` or a count, hence both are
/// [`Value`]s and compared with JS's `!==` / `!=` by the functions in
/// `util_disambig.rs`.
#[derive(Debug, Clone, PartialEq)]
pub struct AmbigConfig {
    /// `maxvals` (never read by upstream; kept for the intermediate dump).
    pub maxvals: Vec<i64>,
    /// `minval`.
    pub minval: i64,
    /// `names`: the number of names to render in each name set.
    pub names: Vec<i64>,
    /// `givens`: the given-name level (0 short, 1 initials, 2 full) of each
    /// name of each name set.
    pub givens: Vec<Vec<i64>>,
    /// `year_suffix`.
    pub year_suffix: Value,
    /// `disambiguate`.
    pub disambiguate: Value,
    /// `use_initials` (set by util_names_disambig.js; `undefined` until then,
    /// read as `false`).
    pub use_initials: bool,
}

impl Default for AmbigConfig {
    /// `CSL.AmbigConfig`'s constructor.
    fn default() -> Self {
        AmbigConfig {
            maxvals: Vec::new(),
            minval: 1,
            names: Vec::new(),
            givens: Vec::new(),
            year_suffix: Value::Bool(false),
            disambiguate: Value::from(0),
            use_initials: false,
        }
    }
}

/// Element-wise accessors for the name code (util_names_*.js), which writes
/// `names[pos]` and `givens[pos][i]` of `tmp.disambig_settings` one element at
/// a time. JS arrays can have holes (`undefined` elements); here a hole reads
/// as absent from [`AmbigConfig::given`] when it is past the end, and is
/// stored as `0` when an element beyond the end is written (upstream's
/// `padBase` gives every shown name a level, `0` by default, anyway).
impl AmbigConfig {
    /// `names[pos] = value`.
    pub fn set_names(&mut self, pos: i64, value: i64) {
        let pos = pos.max(0) as usize;
        while self.names.len() <= pos {
            self.names.push(0);
        }
        self.names[pos] = value;
    }

    /// `givens[pos]` is defined (a row, even an empty one, is truthy in JS).
    pub fn has_givens(&self, pos: i64) -> bool {
        (pos.max(0) as usize) < self.givens.len() && pos >= 0
    }

    /// `givens[pos][i]` (`None` is `undefined`, also when `givens[pos]` is,
    /// where JS would throw: callers check [`AmbigConfig::has_givens`]).
    pub fn given(&self, pos: i64, i: usize) -> Option<i64> {
        self.givens
            .get(pos.max(0) as usize)
            .and_then(|g| g.get(i))
            .copied()
    }

    /// `givens[pos][i] === undefined`.
    pub fn given_is_undefined(&self, pos: i64, i: usize) -> bool {
        self.given(pos, i).is_none()
    }

    /// `if (!givens[pos]) givens[pos] = []` (and the rows before it).
    pub fn ensure_row(&mut self, pos: usize) {
        while self.givens.len() <= pos {
            self.givens.push(Vec::new());
        }
    }

    /// `givens[pos].push(value)` (an `undefined` value is stored as `0`).
    pub fn push_given(&mut self, pos: i64, value: Option<i64>) {
        let pos = pos.max(0) as usize;
        self.ensure_row(pos);
        self.givens[pos].push(value.unwrap_or(0));
    }

    /// `givens[pos][i] = value` (holes are stored as `0`).
    pub fn set_given(&mut self, pos: i64, i: usize, value: Option<i64>) {
        let pos = pos.max(0) as usize;
        self.ensure_row(pos);
        while self.givens[pos].len() <= i {
            self.givens[pos].push(0);
        }
        self.givens[pos][i] = value.unwrap_or(0);
    }

    /// `for (j = 0; j < pos + 1; j++) if (!givens[j]) givens[j] = [];`
    pub fn fill_rows_to(&mut self, pos: i64) {
        for j in 0..=(pos.max(0) as usize) {
            self.ensure_row(j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_constructor_defaults_match_upstream() {
        let c = AmbigConfig::default();
        assert!(c.names.is_empty() && c.givens.is_empty() && c.maxvals.is_empty());
        assert_eq!(c.minval, 1);
        assert_eq!(c.year_suffix, Value::Bool(false));
        assert_eq!(c.disambiguate, Value::from(0));
        assert!(!c.use_initials);
    }

    #[test]
    fn element_accessors_grow_like_js_arrays() {
        let mut c = AmbigConfig::default();
        assert!(!c.has_givens(0));
        c.push_given(1, Some(2));
        assert!(c.has_givens(0) && c.has_givens(1) && !c.has_givens(2));
        assert_eq!(c.given(1, 0), Some(2));
        assert!(c.given_is_undefined(0, 0));
        c.set_given(1, 3, Some(1));
        assert_eq!(c.givens[1], vec![2, 0, 0, 1]);
        c.set_names(2, 5);
        assert_eq!(c.names, vec![0, 0, 5]);
        c.fill_rows_to(3);
        assert_eq!(c.givens.len(), 4);
    }
}
