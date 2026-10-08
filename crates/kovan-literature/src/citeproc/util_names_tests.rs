// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_names_tests.js
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

//! Port of `src/util_names_tests.js`: `CSL.NameOutput.prototype.isPerson`.

use serde_json::Value;

use super::js;
use super::util_names_output::NameOutput;

impl NameOutput {
    /// `CSL.NameOutput.prototype.isPerson(value)`: false for a literal name
    /// and for an institution given only as a family name.
    pub fn is_person(&self, value: &Value) -> bool {
        is_person(value)
    }
}

/// `CSL.NameOutput.prototype.isPerson(value)` (it reads nothing from
/// `this`).
pub fn is_person(value: &Value) -> bool {
    let get = |k: &str| js::truthy_opt(value.get(k));
    !(get("literal") || (!get("given") && get("family") && get("isInstitution")))
}

#[cfg(test)]
mod tests {
    //! Differential test against citeproc-js 2.4.63 for `isPerson`: reference
    //! `tests/data/csl/units/names_output.json`, section `person.isPerson`
    //! (nine edge-case names and 300 names of the fixtures). Pass criterion:
    //! equal results.
    use super::*;
    use crate::citeproc::util_names_output::testing::REFERENCE;

    #[test]
    fn is_person_matches_citeproc_js() {
        let rows = REFERENCE["person"]["isPerson"].as_array().expect("rows");
        assert!(rows.len() > 300);
        let mut persons = 0;
        for r in rows {
            let got = is_person(&r["n"]);
            assert_eq!(Some(got), r["v"].as_bool(), "isPerson({})", r["n"]);
            persons += usize::from(got);
        }
        assert!(persons > 100 && persons < rows.len());
    }
}
