// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/sort.js, src/util_sort.js (the comparison side of the registry)
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

//! `CSL.getSortCompare` (sort.js): the string comparison the registry and the
//! citation sorts use. citeproc-js builds it from `localeCompare` with
//! `{sensitivity: "base", ignorePunctuation: true, numeric: true}` after
//! lower-casing both sides with `CSL.toLocaleLowerCase`; here the collation is
//! [`js::locale_compare_sort`] (ICU4X, differentially tested against node's
//! ICU: DECISIONS.md "Collation for the citeproc port").

use super::js;
use super::load::to_locale_lower_case;
use super::state::State;

/// The function `CSL.getSortCompare(default_locale)` returns.
///
/// `CSL.stringCompare` (a host `sys.stringCompare`) is not modelled: the
/// port's `Sys` is data only.
#[derive(Debug, Clone, PartialEq)]
pub struct SortCompare {
    /// The locale (`default_locale`, `"en-US"` when falsy).
    pub locale: String,
    /// `getBracketPreSort()` is truthy: the collation does not ignore `[`,
    /// so strings are compared after `stripPunct`. (With ICU's
    /// `ignorePunctuation` it never is; the probe is run for real.)
    pub bracket_pre_sort: bool,
}

impl Default for SortCompare {
    fn default() -> Self {
        SortCompare::new(None)
    }
}

/// `str.replace(/^[\[\]\'\"]*/g, "")`.
fn strip_punct(s: &str) -> &str {
    s.trim_start_matches(['[', ']', '\'', '"'])
}

impl SortCompare {
    /// `CSL.getSortCompare.call(state, default_locale)`.
    pub fn new(default_locale: Option<&str>) -> SortCompare {
        let locale = default_locale
            .filter(|l| !l.is_empty())
            .unwrap_or("en-US")
            .to_string();
        // getBracketPreSort: `if (!strcmp("[x","x")) return false`. Lower-casing
        // "[x" and "x" is the identity in every locale.
        let probe = js::locale_compare_sort("[x", "x", &locale) == std::cmp::Ordering::Equal;
        SortCompare {
            locale,
            bracket_pre_sort: !probe,
        }
    }

    /// `strcmp(a, b)`: `CSL.toLocaleLowerCase.call(me, a).localeCompare(
    /// CSL.toLocaleLowerCase.call(me, b), default_locale, strcmp_opts)`.
    fn strcmp(&self, state: &State, a: &str, b: &str) -> i32 {
        let la = to_locale_lower_case(state, a);
        let lb = to_locale_lower_case(state, b);
        js::ordering_to_i32(js::locale_compare_sort(&la, &lb, &self.locale))
    }

    /// `sortCompare(a, b)`: negative, zero or positive.
    pub fn compare(&self, state: &State, a: &str, b: &str) -> i32 {
        if self.bracket_pre_sort {
            self.strcmp(state, strip_punct(a), strip_punct(b))
        } else {
            self.strcmp(state, a, b)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_ignores_case_accents_punctuation_and_orders_numbers() {
        let s = State::default();
        let c = SortCompare::new(Some("en-US"));
        assert!(!c.bracket_pre_sort);
        assert_eq!(c.compare(&s, "Smith", "smith"), 0);
        assert_eq!(c.compare(&s, "[x", "x"), 0);
        assert!(c.compare(&s, "a2", "a10") < 0);
        assert!(c.compare(&s, "b", "a") > 0);
        assert_eq!(SortCompare::new(Some("")).locale, "en-US");
        assert_eq!(strip_punct("[\"'x]"), "x]");
    }
}
