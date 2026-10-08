// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_names_disambig.js
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

//! Port of `src/util_names_disambig.js`:
//! `CSL.NameOutput.prototype.disambigNames` and `_runDisambigNames`, and the
//! part of `tmp.disambig_settings` (`CSL.AmbigConfig`) they read and write.
//!
//! The name register (`disambig_names.rs`), `AmbigConfig` (`tmp.disambig_settings`)
//! and `Disambiguation::pad_base_config` are the engine's.

use serde_json::Value;

use super::js;
use super::load::POSITION_FIRST;
use super::state::State;
use super::util_names_output::NameOutput;
use super::CslResult;

impl NameOutput {
    /// `CSL.NameOutput.prototype.disambigNames()`: register the names with
    /// the name register and choose their given-name levels.
    pub fn disambig_names(&mut self, st: &mut State) -> CslResult<()> {
        for (i, v) in self.variables.clone().iter().enumerate() {
            let pos = self.nameset_base + i as i64;
            let freeters = self.freeters.get(v).cloned().unwrap_or_default();
            if !freeters.is_empty() {
                self.run_disambig_names(st, &freeters, pos)?;
            }
            // Is this even necessary???
            let n_inst = self.institutions.get(v).map(Vec::len).unwrap_or(0);
            if n_inst > 0 {
                if !st.disambig_settings_mut().has_givens(pos) {
                    st.disambig_settings_mut().ensure_row(pos.max(0) as usize);
                }
                for j in 0..n_inst {
                    if st.disambig_settings_mut().given_is_undefined(pos, j) {
                        st.disambig_settings_mut().push_given(pos, Some(2));
                    }
                }
            }
            let groups = self.persons.get(v).cloned().unwrap_or_default();
            for group in &groups {
                if !group.is_empty() {
                    self.run_disambig_names(st, group, pos)?;
                }
            }
        }
        Ok(())
    }

    /// `CSL.NameOutput.prototype._runDisambigNames(lst, pos)`.
    pub fn run_disambig_names(&mut self, st: &mut State, lst: &[Value], pos: i64) -> CslResult<()> {
        let item_id = self
            .item
            .get("id")
            .map(js::to_js_string)
            .unwrap_or_else(|| "undefined".to_string());
        for (i, name) in lst.iter().enumerate() {
            if !js::truthy_opt(name.get("given")) && !js::truthy_opt(name.get("family")) {
                continue;
            }

            let myinitials = self.inherit_name_opt(st, "initialize-with", None, None)?;
            let myinitials_str = myinitials
                .as_ref()
                .and_then(Value::as_str)
                .map(str::to_string);
            super::disambig_names::addname(st, &item_id, name, i as i64)?;
            if !st.disambig_settings_mut().has_givens(pos) {
                // Holes can appear in the list, probably due to institutional
                // names that this doesn't touch. Maybe. This fills them up.
                st.disambig_settings_mut().fill_rows_to(pos);
            }
            let chk = st.disambig_settings_mut().given(pos, i);
            let myform = self.form_opt(st)?;
            if chk.is_none() {
                let p =
                    super::disambig_names::evalname(st, &item_id, name, i as i64, 0, Some(&myform), myinitials_str.as_deref())?;
                st.disambig_settings_mut().push_given(pos, p);
            }
            //
            // set the display mode default for givennames if required
            let paramx =
                super::disambig_names::evalname(st, &item_id, name, i as i64, 0, Some(&myform), myinitials_str.as_deref())?;
            let mut param: Option<i64>;
            if st.tmp.disambig_request.is_some() {
                //
                // fix a request for initials that makes no sense.
                // can't do this in disambig, because the availability
                // of initials is not a global parameter.
                let mut val = st.disambig_settings_mut().given(pos, i);
                // This is limited to by-cite disambiguation.
                // 2012-09-13: added lst[i].given check to condition
                if val == Some(1)
                    && st
                        .citation
                        .opt
                        .get("givenname-disambiguation-rule")
                        .and_then(Value::as_str)
                        == Some("by-cite")
                    && (myinitials.is_none() || name.get("given").is_none())
                {
                    val = Some(2);
                }
                param = val;
                // 2012-09-13: lst[i].given check protects against personal names
                // that have no first name element. These were causing an infinite loop,
                // this prevents that.
                if js::truthy_opt(st.opt.get("disambiguate-add-givenname"))
                    && js::truthy_opt(name.get("given"))
                {
                    param = super::disambig_names::evalname(
                        st,
                        &item_id,
                        name,
                        i as i64,
                        param.unwrap_or(0),
                        Some(&myform),
                        myinitials_str.as_deref(),
                    )?;
                }
            } else {
                //
                // it clicks.  here is where we will put the
                // call to the names register, to get the floor value
                // for an individual name.
                //
                param = paramx;
            }
            // Need to save off the settings based on subsequent
            // form, when first cites are rendered.
            if !st.tmp.just_looking
                && js::truthy(&self.cite_item)
                && self.cite_item.get("position").and_then(Value::as_i64) == Some(POSITION_FIRST)
            {
                if let (Some(px), Some(p)) = (paramx, param) {
                    if px > p {
                        param = paramx;
                    }
                }
            }
            if !st.tmp.sort_key_flag {
                st.disambig_settings_mut().set_given(pos, i, param);
                let initialize = self.name_token()?.strings.get("initialize");
                if myinitials_str.is_some()
                    && (initialize.is_none() || initialize == Some(&Value::Bool(true)))
                {
                    st.disambig_settings_mut().use_initials = true;
                }
            }
        }
        Ok(())
    }

    /// `this.state.inheritOpt(this.name, "form", "name-form", "long")`.
    pub(super) fn form_opt(&self, st: &State) -> CslResult<String> {
        Ok(self
            .inherit_name_opt(
                st,
                "form",
                Some("name-form"),
                Some(Value::String("long".into())),
            )?
            .map(|v| js::to_js_string(&v))
            .unwrap_or_default())
    }
}
