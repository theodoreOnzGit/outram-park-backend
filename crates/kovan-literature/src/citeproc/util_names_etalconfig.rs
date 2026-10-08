// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_names_etalconfig.js
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

//! Port of `src/util_names_etalconfig.js`:
//! `CSL.NameOutput.prototype.getEtAlConfig`.

use serde_json::Value;

use super::js;
use super::state::State;
use super::util_names_output::{q_append_str, q_pop_blob_required, BlobPair, NameOutput};
use super::CslResult;

impl NameOutput {
    /// `CSL.NameOutput.prototype.getEtAlConfig()`: build the et-al blobs and
    /// choose the et-al parameters of this cite.
    pub fn get_et_al_config(&mut self, st: &mut State) -> CslResult<()> {
        // this["et-al"] = {};
        let style = self.etal_style.format_ref();
        let term = self.etal_term.clone();

        q_append_str(st, term.as_deref(), style.clone(), true)?;
        let single = q_pop_blob_required(st)?;
        st.blobs
            .get_mut(single)
            .set_string("suffix", &self.etal_suffix);
        st.blobs
            .get_mut(single)
            .set_string("prefix", &self.etal_prefix_single);

        q_append_str(st, term.as_deref(), style, true)?;
        let multiple = q_pop_blob_required(st)?;
        st.blobs
            .get_mut(multiple)
            .set_string("suffix", &self.etal_suffix);
        st.blobs
            .get_mut(multiple)
            .set_string("prefix", &self.etal_prefix_multiple);
        self.et_al = Some(BlobPair {
            single: Some(single),
            multiple: Some(multiple),
        });

        // Et-al style parameters (may be sidestepped by disambiguation
        // in util_names_constraints.js)
        let position_truthy = js::truthy_opt(self.cite_item.get("position"));
        if position_truthy {
            let sub_min = self.inherit_name_opt(st, "et-al-subsequent-min", None, None)?;
            if sub_min.as_ref().map(js::truthy).unwrap_or(false) {
                self.etal_min = sub_min;
            } else {
                self.etal_min = self.inherit_name_opt(st, "et-al-min", None, None)?;
            }
            let sub_first = self.inherit_name_opt(st, "et-al-subsequent-use-first", None, None)?;
            if sub_first.as_ref().map(js::truthy).unwrap_or(false) {
                self.etal_use_first = sub_first;
            } else {
                self.etal_use_first = self.inherit_name_opt(st, "et-al-use-first", None, None)?;
            }
        } else {
            if st.tmp.et_al_min.as_ref().map(js::truthy).unwrap_or(false) {
                self.etal_min = st.tmp.et_al_min.clone();
            } else {
                self.etal_min = self.inherit_name_opt(st, "et-al-min", None, None)?;
            }
            if st
                .tmp
                .et_al_use_first
                .as_ref()
                .map(js::truthy)
                .unwrap_or(false)
            {
                self.etal_use_first = st.tmp.et_al_use_first.clone();
            } else {
                self.etal_use_first = self.inherit_name_opt(st, "et-al-use-first", None, None)?;
            }
            if let Some(Value::Bool(_)) = st.tmp.et_al_use_last {
                self.etal_use_last = st.tmp.et_al_use_last.clone();
            } else {
                self.etal_use_last = self.inherit_name_opt(st, "et-al-use-last", None, None)?;
            }
        }
        // Provided for use as the starting level for disambiguation.
        if !st.tmp.et_al_min.as_ref().map(js::truthy).unwrap_or(false) {
            st.tmp.et_al_min = self.etal_min.clone();
        }
        Ok(())
    }
}
