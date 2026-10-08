// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_names_etal.js
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

//! Port of `src/util_names_etal.js`:
//! `CSL.NameOutput.prototype.setEtAlParameters` and `_setEtAlParameter`.

use super::js;
use super::state::State;
use super::util_names_output::{EtalSpec, NameOutput};
use super::CslResult;

/// Which list of a variable `_setEtAlParameter` looks at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtalKind {
    /// `"freeters"`.
    Freeters,
    /// `"persons"` (with an index).
    Persons,
    /// `"institutions"`.
    Institutions,
}

impl NameOutput {
    /// `CSL.NameOutput.prototype.setEtAlParameters()`: for each variable,
    /// record whether each of its name lists was cut (1 = et-al, 2 = ellipsis).
    pub fn set_et_al_parameters(&mut self, st: &mut State) -> CslResult<()> {
        for (i, v) in self.variables.clone().iter().enumerate() {
            if self.etal_spec(v).is_none() {
                self.etal_specs.push(EtalSpec::default());
                self.etal_spec_keys
                    .insert(v.clone(), self.etal_specs.len() - 1);
            }
            let idx = self.etal_spec_keys.get(v).copied().unwrap_or(0);
            self.etal_spec_keys
                .insert((self.nameset_base + i as i64).to_string(), idx);
            if !self.freeters.get(v).map(Vec::is_empty).unwrap_or(true) {
                self.set_et_al_parameter(st, EtalKind::Freeters, v, 0)?;
            }
            let jlen = self.persons.get(v).map(Vec::len).unwrap_or(0);
            for j in 0..jlen {
                // `etal_spec[v][j]` is always undefined upstream (the key is
                // `persons[j]`); the assignment below happens regardless.
                if let Some(spec) = self.etal_spec_mut(v) {
                    while spec.persons.len() <= j {
                        spec.persons.push(0);
                    }
                    spec.persons[j] = 0;
                }
                self.set_et_al_parameter(st, EtalKind::Persons, v, j)?;
            }
            if !self.institutions.get(v).map(Vec::is_empty).unwrap_or(true) {
                self.set_et_al_parameter(st, EtalKind::Institutions, v, 0)?;
            }
        }
        Ok(())
    }

    /// `CSL.NameOutput.prototype._setEtAlParameter(type, v, j)`.
    pub fn set_et_al_parameter(
        &mut self,
        st: &State,
        kind: EtalKind,
        v: &str,
        j: usize,
    ) -> CslResult<()> {
        let (len, count) = match kind {
            EtalKind::Persons => (
                self.persons
                    .get(v)
                    .and_then(|p| p.get(j))
                    .map(Vec::len)
                    .unwrap_or(0) as i64,
                self.persons_count
                    .get(v)
                    .and_then(|p| p.get(j))
                    .copied()
                    .unwrap_or(0),
            ),
            EtalKind::Freeters => (
                self.freeters.get(v).map(Vec::len).unwrap_or(0) as i64,
                self.freeters_count.get(v).copied().unwrap_or(0),
            ),
            EtalKind::Institutions => (
                self.institutions.get(v).map(Vec::len).unwrap_or(0) as i64,
                self.institutions_count.get(v).copied().unwrap_or(0),
            ),
        };
        let value: i64 = if len < count && !st.tmp.sort_key_flag {
            if self.etal_use_last.as_ref().map(js::truthy).unwrap_or(false) {
                2
            } else {
                1
            }
        } else {
            0
        };
        let Some(spec) = self.etal_spec_mut(v) else {
            return Err(super::load::type_error(
                "Cannot set properties of undefined (setting 'freeters')",
            ));
        };
        match kind {
            EtalKind::Persons => {
                while spec.persons.len() <= j {
                    spec.persons.push(0);
                }
                spec.persons[j] = value;
            }
            EtalKind::Freeters => spec.freeters = value,
            EtalKind::Institutions => spec.institutions = value,
        }
        Ok(())
    }
}
