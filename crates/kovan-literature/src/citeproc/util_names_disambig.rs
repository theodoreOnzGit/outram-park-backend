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
//! **Integration note.** `obj_ambigconfig.rs` and the registry
//! (`state.registry.namereg`, `state.disambiguate.padBase`) belong to the
//! engine agent. Until they exist the names code uses [`NameDisambigSettings`]
//! (`state.tmp.name_ambig`: the `names`, `givens` and `use_initials` members of
//! `tmp.disambig_settings`) and the two stubs [`namereg_addname`] and
//! [`namereg_evalname`] and the function [`pad_base`]. The integrator replaces
//! them by the engine's `AmbigConfig` / `NameReg::addname` / `evalname` /
//! `Disambiguation::padBase`.

use serde_json::Value;

use super::js;
use super::load::POSITION_FIRST;
use super::state::State;
use super::util_names_output::NameOutput;
use super::CslResult;

/// The members of `CSL.AmbigConfig` (`tmp.disambig_settings`) that
/// util_names_*.js uses: `names[pos]` (the number of names shown in nameset
/// `pos`), `givens[pos][i]` (the given-name level of name `i`; JS arrays with
/// holes, hence the `Option`s) and `use_initials`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NameDisambigSettings {
    /// `names`.
    pub names: Vec<Option<i64>>,
    /// `givens`.
    pub givens: Vec<Option<Vec<Option<i64>>>>,
    /// `use_initials`.
    pub use_initials: bool,
}

impl NameDisambigSettings {
    /// `names[pos] = value` (holes before `pos` are `undefined`).
    pub fn set_names(&mut self, pos: i64, value: i64) {
        let pos = pos.max(0) as usize;
        while self.names.len() <= pos {
            self.names.push(None);
        }
        self.names[pos] = Some(value);
    }

    /// `givens[pos]` is defined.
    pub fn has_givens(&self, pos: i64) -> bool {
        self.givens
            .get(pos.max(0) as usize)
            .map(Option::is_some)
            .unwrap_or(false)
    }

    /// `givens[pos][i]`: `None` for `undefined` (also when `givens[pos]` is,
    /// where JS would throw: callers check [`NameDisambigSettings::has_givens`]).
    pub fn given(&self, pos: i64, i: usize) -> Option<i64> {
        self.givens
            .get(pos.max(0) as usize)
            .and_then(|g| g.as_ref())
            .and_then(|g| g.get(i))
            .copied()
            .flatten()
    }

    /// `givens[pos][i] === undefined`.
    pub fn given_is_undefined(&self, pos: i64, i: usize) -> bool {
        self.given(pos, i).is_none()
    }

    /// `givens[pos] = []` when `givens[pos]` is `undefined`.
    pub fn ensure_row(&mut self, pos: usize) {
        while self.givens.len() <= pos {
            self.givens.push(None);
        }
        if self.givens[pos].is_none() {
            self.givens[pos] = Some(Vec::new());
        }
    }

    /// `givens[pos].push(value)`.
    pub fn push_given(&mut self, pos: i64, value: Option<i64>) {
        let pos = pos.max(0) as usize;
        self.ensure_row(pos);
        if let Some(Some(row)) = self.givens.get_mut(pos) {
            row.push(value);
        }
    }

    /// `givens[pos][i] = value` (holes filled with `undefined`).
    pub fn set_given(&mut self, pos: i64, i: usize, value: Option<i64>) {
        let pos = pos.max(0) as usize;
        self.ensure_row(pos);
        if let Some(Some(row)) = self.givens.get_mut(pos) {
            while row.len() <= i {
                row.push(None);
            }
            row[i] = value;
        }
    }

    /// `for (j = 0; j < pos + 1; j++) if (!givens[j]) givens[j] = [];`
    pub fn fill_rows_to(&mut self, pos: i64) {
        for j in 0..=(pos.max(0) as usize) {
            self.ensure_row(j);
        }
    }
}

/// `CSL.Disambiguation.prototype.padBase(base)` for the members this module
/// models (disambig_cites.js:504): every name that is shown gets a given-name
/// level, defaulting to 0.
///
/// STUB(disambig_cites): the engine agent owns `Disambiguation`.
pub fn pad_base(base: &mut NameDisambigSettings) {
    for i in 0..base.names.len() {
        if !base.has_givens(i as i64) {
            base.fill_rows_to(i as i64);
        }
        let n = base.names[i].unwrap_or(0);
        for j in 0..n.max(0) as usize {
            let cur = base.given(i as i64, j);
            if cur.map(|c| c == 0).unwrap_or(true) {
                base.set_given(i as i64, j, Some(0));
            }
        }
    }
}

/// `state.registry.namereg.addname(item_id, nameobj, namenum)`.
///
/// STUB(disambig_names): the name register is the engine agent's; it
/// records the name under its family/initial/given keys.
pub fn namereg_addname(_st: &mut State, _item_id: &str, _nameobj: &Value, _namenum: usize) {}

/// `state.registry.namereg.evalname(item_id, nameobj, namenum, request_base,
/// form, initials)`: the given-name level (0 none, 1 initials, 2 full) to
/// show for a name; `None` is JS `undefined`.
///
/// STUB(disambig_names): this is the part of `evalname` that does not need
/// the name register's counts, which is its whole result when the name is
/// not ambiguous with another registered one (`!dagopt`, and `by-cite`
/// requests): the level implied by the form and `initialize-with`, raised to
/// `request_base` for `by-cite`.
pub fn namereg_evalname(
    st: &State,
    _item_id: &str,
    _nameobj: &Value,
    _namenum: usize,
    request_base: i64,
    form: &str,
    initials: Option<&str>,
) -> Option<i64> {
    let mut param = 2;
    if form == "short" {
        param = 0;
    } else if initials.is_some() {
        param = 1;
    }
    let gdropt_orig = st
        .citation
        .opt
        .get("givenname-disambiguation-rule")
        .and_then(Value::as_str);
    if gdropt_orig == Some("by-cite") && param <= request_base {
        return Some(request_base);
    }
    Some(param)
}

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
                if !st.tmp.name_ambig.has_givens(pos) {
                    st.tmp.name_ambig.ensure_row(pos.max(0) as usize);
                }
                for j in 0..n_inst {
                    if st.tmp.name_ambig.given_is_undefined(pos, j) {
                        st.tmp.name_ambig.push_given(pos, Some(2));
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
            namereg_addname(st, &item_id, name, i);
            if !st.tmp.name_ambig.has_givens(pos) {
                // Holes can appear in the list, probably due to institutional
                // names that this doesn't touch. Maybe. This fills them up.
                st.tmp.name_ambig.fill_rows_to(pos);
            }
            let chk = st.tmp.name_ambig.given(pos, i);
            let myform = self.form_opt(st)?;
            if chk.is_none() {
                let p =
                    namereg_evalname(st, &item_id, name, i, 0, &myform, myinitials_str.as_deref());
                st.tmp.name_ambig.push_given(pos, p);
            }
            //
            // set the display mode default for givennames if required
            let paramx =
                namereg_evalname(st, &item_id, name, i, 0, &myform, myinitials_str.as_deref());
            let mut param: Option<i64>;
            if js::truthy(&st.tmp.disambig_request) {
                //
                // fix a request for initials that makes no sense.
                // can't do this in disambig, because the availability
                // of initials is not a global parameter.
                let mut val = st.tmp.name_ambig.given(pos, i);
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
                    param = namereg_evalname(
                        st,
                        &item_id,
                        name,
                        i,
                        param.unwrap_or(0),
                        &myform,
                        myinitials_str.as_deref(),
                    );
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
                st.tmp.name_ambig.set_given(pos, i, param);
                let initialize = self.name_token()?.strings.get("initialize");
                if myinitials_str.is_some()
                    && (initialize.is_none() || initialize == Some(&Value::Bool(true)))
                {
                    st.tmp.name_ambig.use_initials = true;
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
