// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_names_constraints.js
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


//! Port of `src/util_names_constraints.js`:
//! `CSL.NameOutput.prototype.constrainNames` and `_imposeNameConstraints`.

use serde_json::Value;

use super::js;
use super::state::State;
use super::util_names_disambig::pad_base;
use super::util_names_output::{js_num, slice_head, NameOutput};
use super::CslResult;

impl NameOutput {
    /// `CSL.NameOutput.prototype.constrainNames()`: figure out how many names
    /// to include, in light of the disambiguation parameters.
    pub fn constrain_names(&mut self, st: &mut State) -> CslResult<()> {
        self.names_count = 0;
        for (i, v) in self.variables.clone().iter().enumerate() {
            let pos = self.nameset_base + i as i64;
            // Constrain independent authors here
            let mut freeters = self.freeters.get(v).cloned().unwrap_or_default();
            if !freeters.is_empty() {
                st.tmp.names_max.push_literal(Value::from(freeters.len()));
                let count = self.freeters_count.get(v).copied().unwrap_or(0);
                self.impose_name_constraints(st, &mut freeters, count, pos)?;
                self.names_count += freeters.len() as i64;
                self.freeters.insert(v.clone(), freeters);
            }

            // Constrain institutions here
            let mut insts = self.institutions.get(v).cloned().unwrap_or_default();
            if !insts.is_empty() {
                st.tmp.names_max.push_literal(Value::from(insts.len()));
                let count = self.institutions_count.get(v).copied().unwrap_or(0);
                self.impose_name_constraints(st, &mut insts, count, pos)?;
                if let Some(p) = self.persons.get_mut(v) {
                    p.truncate(insts.len());
                }
                self.names_count += insts.len() as i64;
                self.institutions.insert(v.clone(), insts);
            }

            let jlen = self.persons.get(v).map(Vec::len).unwrap_or(0);
            for j in 0..jlen {
                // Constrain affiliated authors here
                let mut group = self
                    .persons
                    .get(v)
                    .and_then(|p| p.get(j))
                    .cloned()
                    .unwrap_or_default();
                if !group.is_empty() {
                    st.tmp.names_max.push_literal(Value::from(group.len()));
                    let count = self
                        .persons_count
                        .get(v)
                        .and_then(|p| p.get(j))
                        .copied()
                        .unwrap_or(0);
                    self.impose_name_constraints(st, &mut group, count, pos)?;
                    self.names_count += group.len() as i64;
                    if let Some(slot) = self.persons.get_mut(v).and_then(|p| p.get_mut(j)) {
                        *slot = group;
                    }
                }
            }
        }
        Ok(())
    }

    /// `CSL.NameOutput.prototype._imposeNameConstraints(lst, count, key,
    /// pos)` on the list `lst[key]` (passed as `display_names`) whose
    /// original length was `count[key]`.
    pub fn impose_name_constraints(
        &self,
        st: &mut State,
        display_names: &mut Vec<Value>,
        count: i64,
        pos: i64,
    ) -> CslResult<()> {
        let count_f = count as f64;
        let etal_min = js_num(self.etal_min.as_ref());
        let etal_use_first = js_num(self.etal_use_first.as_ref());
        let use_last = self.etal_use_last.as_ref().map(js::truthy).unwrap_or(false);
        // display_names starts as the original length of this list of names.
        let original = display_names.clone();
        let mut discretionary_names_length = js_num(st.tmp.et_al_min.as_ref());

        // Mappings, to allow existing disambiguation machinery to
        // remain untouched.
        let request_names = request_names_at(&st.tmp.disambig_request, pos);
        if st.tmp.suppress_decorations {
            if js::truthy(&st.tmp.disambig_request) && js::truthy_opt(request_names.as_ref()) {
                // Oh. Trouble.
                // state.tmp.nameset_counter is the number of the nameset
                // in the disambiguation try-sequence. Ouch.
                discretionary_names_length = js_num(request_names.as_ref());
            } else if count_f >= etal_min {
                discretionary_names_length = etal_use_first;
            }
        } else {
            if js::truthy(&st.tmp.disambig_request)
                && js_num(request_names.as_ref()) > etal_use_first
            {
                if count_f < etal_min {
                    discretionary_names_length = count_f;
                } else {
                    discretionary_names_length = js_num(request_names.as_ref());
                }
            } else if count_f >= etal_min {
                discretionary_names_length = etal_use_first;
            }
            // XXXX: This is a workaround. Under some conditions.
            // Where namesets disambiguate on one of the two names
            // dropped here, it is possible for more than one
            // in-text citation to be close (and indistinguishable)
            // matches to a single bibliography entry.
            if use_last && discretionary_names_length > (etal_min - 2.0) {
                discretionary_names_length = etal_min - 2.0;
            }
        }
        let sane = etal_min >= etal_use_first;
        let overlength = count_f > discretionary_names_length;
        // This var is used to control contextual join, and
        // lies about the number of names when forceEtAl is true,
        // unless normalized.
        if discretionary_names_length > count_f {
            // Use actual truncated list length, to avoid overrun.
            discretionary_names_length = original.len() as f64;
        }
        // forceEtAl is relevant when the author list is
        // truncated to eliminate clutter.
        if sane && overlength {
            let mut kept = slice_head(&original, discretionary_names_length);
            if use_last {
                if let Some(last) = original.last() {
                    kept.push(last.clone());
                }
            }
            *display_names = kept;
        }
        st.tmp.name_ambig.set_names(pos, display_names.len() as i64);
        pad_base(&mut st.tmp.name_ambig);
        Ok(())
    }
}

/// `state.tmp.disambig_request.names[pos]` (`undefined` = `None`).
fn request_names_at(request: &Value, pos: i64) -> Option<Value> {
    request
        .get("names")
        .and_then(|n| n.get(pos as usize))
        .cloned()
}
