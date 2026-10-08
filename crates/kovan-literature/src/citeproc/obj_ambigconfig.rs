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
    /// `use_initials` (set by util_names_disambig.js; `undefined` until then).
    pub use_initials: Option<bool>,
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
            use_initials: None,
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
        assert_eq!(c.use_initials, None);
    }
}
