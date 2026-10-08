// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_names_common.js
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

//! Port of `src/util_names_common.js`:
//! `CSL.NameOutput.prototype.checkCommonAuthor`, `setCommonTerm` and
//! `_compareNamesets` (also `CSL.Util.Names.compareNamesets`).

use serde_json::Value;

use super::js;
use super::load::NAME_PARTS;
use super::state::State;
use super::util_names_output::NameOutput;
use super::CslResult;

/// JS `a != b` (abstract inequality) for the scalar values a name part can
/// hold; `undefined` is `None`, and `null` equals `undefined`.
fn loose_ne(a: Option<&Value>, b: Option<&Value>) -> bool {
    let nullish = |v: Option<&Value>| matches!(v, None | Some(Value::Null));
    if nullish(a) || nullish(b) {
        return !(nullish(a) && nullish(b));
    }
    let (a, b) = (a.unwrap_or(&Value::Null), b.unwrap_or(&Value::Null));
    match (a, b) {
        (Value::String(x), Value::String(y)) => x != y,
        (Value::Object(_), _)
        | (_, Value::Object(_))
        | (Value::Array(_), _)
        | (_, Value::Array(_)) => {
            // Objects compare by identity; distinct values are never equal.
            true
        }
        _ => {
            let n = |v: &Value| match v {
                Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
                Value::Number(n) => n.as_f64(),
                Value::String(s) => {
                    let t = js::trim(s);
                    Some(if t.is_empty() {
                        0.0
                    } else {
                        t.parse::<f64>().unwrap_or(f64::NAN)
                    })
                }
                _ => None,
            };
            match (n(a), n(b)) {
                (Some(x), Some(y)) => x != y,
                _ => true,
            }
        }
    }
}

/// `CSL.NameOutput.prototype._compareNamesets(base_nameset, nameset)` over
/// two lists of name objects: equal lengths and equal name parts.
pub fn compare_namesets_slices(base: &[Value], nameset: &[Value]) -> bool {
    if base.len() != nameset.len() {
        return false;
    }
    for i in 0..nameset.len() {
        for part in NAME_PARTS {
            // `!base_nameset[i] || base_nameset[i][part] != nameset[i][part]`
            if !js::truthy(&base[i]) || loose_ne(base[i].get(*part), nameset[i].get(*part)) {
                return false;
            }
        }
    }
    true
}

/// `CSL.NameOutput.prototype._compareNamesets` on `Item[variable]` values
/// (`undefined` is `None`): false unless both are present and equal.
pub fn compare_namesets(base: Option<&Value>, nameset: Option<&Value>) -> bool {
    let (Some(base), Some(nameset)) = (base, nameset) else {
        return false;
    };
    if !js::truthy(base) || !js::truthy(nameset) {
        return false;
    }
    match (base, nameset) {
        (Value::Array(a), Value::Array(b)) => compare_namesets_slices(a, b),
        // `.length` of a non-array is `undefined` (or a string's length):
        // two such values "have equal lengths" and the loop runs 0 times.
        (Value::Array(_), _) | (_, Value::Array(_)) => false,
        _ => true,
    }
}

impl NameOutput {
    /// `CSL.NameOutput.prototype.checkCommonAuthor(requireMatch)`: whether a
    /// `names` element calling two variables that need to match (with a
    /// common term such as `editortranslator`) fails to match.
    pub fn check_common_author(&mut self, st: &mut State, require_match: bool) -> CslResult<bool> {
        if !require_match {
            return Ok(false);
        }
        let mut common_term = String::new();
        if self.variables.len() == 2 {
            let mut varnames = self.variables.clone();
            varnames.sort();
            common_term = varnames.concat();
        }
        if common_term.is_empty() {
            return Ok(false);
        }
        let lang = js::get_string(&st.opt, "lang").unwrap_or_default();
        let locale = st.locale.get(&lang).ok_or_else(|| {
            super::load::type_error("Cannot read properties of undefined (reading 'terms')")
        })?;
        let has_term = js::truthy_opt(locale.terms.get(&common_term));
        if !has_term {
            st.tmp.done_vars.push(self.variables[0].clone());
            st.tmp.done_vars.push(self.variables[1].clone());
            return Ok(false);
        }
        let first_set = self.item.get(self.variables[0].as_str());
        let second_set = self.item.get(self.variables[1].as_str());
        let perfect_match = compare_namesets(first_set, second_set);
        if perfect_match {
            st.tmp.done_vars.push(self.variables[0].clone());
            st.tmp.done_vars.push(self.variables[1].clone());
        }
        // This may be counter-intuitive.
        // This check controls whether we will fail on the this attempt at rendering
        // and proceed with substitution. If the names match exactly (true), then
        // we do *not* want to abort and continue with substitution.
        Ok(!perfect_match)
    }

    /// `CSL.NameOutput.prototype.setCommonTerm()`: when the variables share
    /// a combined term (`editortranslator`) and their names are identical,
    /// `common_term` names it; otherwise it is cleared.
    pub fn set_common_term(&mut self, st: &mut State) -> CslResult<()> {
        let mut varnames = self.variables.clone();
        varnames.sort();
        let common = varnames.concat();
        // When no varnames are on offer
        if common.is_empty() {
            self.common_term = None;
            return Ok(());
        }
        self.common_term = Some(common.clone());
        let mut has_term = false;
        if let Some(first) = self.variables.first() {
            if let Some(lp) = self.label.get(first) {
                let form = if let Some(b) = &lp.before {
                    Some(b.string_opt("form"))
                } else {
                    lp.after.as_ref().map(|a| a.string_opt("form"))
                };
                if let Some(form) = form {
                    has_term = st
                        .get_term(&common, form.as_deref(), Some(0), None, None, false)?
                        .map(|t| !t.is_empty())
                        .unwrap_or(false);
                }
            }
        }

        // When there is no common term
        let lang = js::get_string(&st.opt, "lang").unwrap_or_default();
        let locale = st.locale.get(&lang).ok_or_else(|| {
            super::load::type_error("Cannot read properties of undefined (reading 'terms')")
        })?;
        if !js::truthy_opt(locale.terms.get(&common)) || !has_term || self.variables.len() < 2 {
            self.common_term = None;
            return Ok(());
        }
        for i in 0..self.variables.len() - 1 {
            let v = &self.variables[i];
            let vv = &self.variables[i + 1];
            let fv = self.freeters.get(v).cloned().unwrap_or_default();
            let fvv = self.freeters.get(vv).cloned().unwrap_or_default();
            if !fv.is_empty() || !fvv.is_empty() {
                let sv = self.etal_spec(v).map(|s| s.freeters);
                let svv = self.etal_spec(vv).map(|s| s.freeters);
                if sv != svv || !compare_namesets_slices(&fv, &fvv) {
                    self.common_term = None;
                    return Ok(());
                }
            }
            let pv = self.persons.get(v).cloned().unwrap_or_default();
            let pvv = self.persons.get(vv).cloned().unwrap_or_default();
            if pv.len() != pvv.len() {
                self.common_term = None;
                return Ok(());
            }
            for j in 0..pv.len() {
                let a = self.etal_spec(v).and_then(|s| s.persons.get(j).copied());
                let b = self.etal_spec(vv).and_then(|s| s.persons.get(j).copied());
                if a != b || !compare_namesets_slices(&pv[j], &pvv[j]) {
                    self.common_term = None;
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    /// `CSL.NameOutput.prototype._compareNamesets`.
    pub fn compare_namesets(&self, base: &[Value], nameset: &[Value]) -> bool {
        compare_namesets_slices(base, nameset)
    }
}

#[cfg(test)]
mod tests {
    //! Differential test against citeproc-js 2.4.63 for `_compareNamesets`:
    //! reference `tests/data/csl/units/names_output.json`, section
    //! `person.compare` (all 121 ordered pairs of eleven name lists: equal,
    //! different lengths, a missing, empty or null `given`, suffixes,
    //! particles, literals, and `undefined` arguments). Pass criterion: equal
    //! results (including `undefined != ""`).
    use super::*;
    use crate::citeproc::util_names_output::testing::REFERENCE;

    #[test]
    fn compare_namesets_matches_citeproc_js() {
        let rows = REFERENCE["person"]["compare"].as_array().expect("rows");
        let mut equal = 0;
        for r in rows {
            let got = if r.get("undefinedA").is_some() {
                compare_namesets(None, r.get("b"))
            } else {
                compare_namesets(r.get("a"), r.get("b"))
            };
            assert_eq!(Some(got), r["v"].as_bool(), "{} vs {}", r["a"], r["b"]);
            equal += usize::from(got);
        }
        assert!(rows.len() > 120 && equal > 5 && equal < rows.len());
    }
}
