// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_names_truncate.js
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

//! Port of `src/util_names_truncate.js`:
//! `CSL.NameOutput.prototype.truncatePersonalNameLists` and
//! `_truncateNameList`.

use serde_json::Value;

use super::js;
use super::state::State;
use super::util_names_output::{js_num, NameOutput};
use super::CslResult;

impl NameOutput {
    /// `CSL.NameOutput.prototype.truncatePersonalNameLists()`: record the
    /// original number of names (for et-al evaluation) and cut absurdly long
    /// lists.
    pub fn truncate_personal_name_lists(&mut self, st: &mut State) -> CslResult<()> {
        self.freeters_count = Default::default();
        self.persons_count = Default::default();
        self.institutions_count = Default::default();
        // `v` is a function-level variable in JS: the loops below leave it at
        // the last key, and the code at the end of the function reads it.
        let mut v: Option<String> = None;
        // By key is okay here, as we don't care about sequence.
        for key in self.keys.clone() {
            v = Some(key.clone());
            let list = self.freeters.get(&key).cloned().unwrap_or_default();
            self.freeters_count.insert(key.clone(), list.len() as i64);
            let truncated = self.truncate_name_list(st, &list);
            self.freeters.insert(key, truncated);
        }

        for key in self.keys.clone() {
            v = Some(key.clone());
            let n_inst = self.institutions.get(&key).map(Vec::len).unwrap_or(0);
            self.institutions_count.insert(key.clone(), n_inst as i64);
            // The result of truncating the institutions is discarded upstream.
            let insts = self.institutions.get(&key).cloned().unwrap_or_default();
            let _ = self.truncate_name_list(st, &insts);
            let mut persons = self.persons.get(&key).cloned().unwrap_or_default();
            persons.truncate(n_inst);
            let mut counts: Vec<i64> = Vec::new();
            for (j, group) in persons.iter_mut().enumerate() {
                let _ = j;
                counts.push(group.len() as i64);
                *group = self.truncate_name_list(st, group);
            }
            self.persons_count.insert(key.clone(), counts);
            self.persons.insert(key, persons);
        }
        let hack = js::truthy_opt(
            st.opt
                .get("development_extensions")
                .and_then(|d| d.get("etal_min_etal_usefirst_hack")),
        );
        let is_one = |x: &Option<Value>| {
            x.as_ref()
                .map(|x| js_num(Some(x)) == 1.0 && x.is_number())
                .unwrap_or(false)
        };
        let chopvar: Option<String> = if hack
            && is_one(&self.etal_min)
            && is_one(&self.etal_use_first)
            && !(!st.tmp.extension.is_empty() || st.tmp.just_looking)
        {
            // chopvar = v (truthy only when v is a variable name)
            v.clone().filter(|s| !s.is_empty())
        } else {
            None
        };
        if chopvar.is_some() || self.please_chop.is_some() {
            for var in self.variables.clone() {
                v = Some(var.clone());
                if !self.freeters.get(&var).map(Vec::is_empty).unwrap_or(true) {
                    if self.please_chop.as_deref() == Some(var.as_str()) {
                        if let Some(f) = self.freeters.get_mut(&var) {
                            if !f.is_empty() {
                                f.remove(0);
                            }
                        }
                        if let Some(c) = self.freeters_count.get_mut(&var) {
                            *c -= 1;
                        }
                        self.please_chop = None;
                    } else if chopvar.is_some() && self.please_chop.is_none() {
                        if let Some(f) = self.freeters.get_mut(&var) {
                            f.truncate(1);
                        }
                        self.freeters_count.insert(var.clone(), 1);
                        self.institutions.insert(var.clone(), Vec::new());
                        self.persons.insert(var.clone(), Vec::new());
                        self.please_chop = chopvar.clone();
                    }
                }
                let jlen = self.persons.get(&var).map(Vec::len).unwrap_or(0);
                for j in 0..jlen {
                    let group_nonempty = self
                        .persons
                        .get(&var)
                        .and_then(|p| p.get(j))
                        .map(|g| !g.is_empty())
                        .unwrap_or(false);
                    if group_nonempty {
                        if self.please_chop.as_deref() == Some(var.as_str()) {
                            if let Some(g) = self.persons.get_mut(&var).and_then(|p| p.get_mut(j)) {
                                g.remove(0);
                            }
                            if let Some(c) =
                                self.persons_count.get_mut(&var).and_then(|p| p.get_mut(j))
                            {
                                *c -= 1;
                            }
                            self.please_chop = None;
                            break;
                        } else if chopvar.is_some() && self.please_chop.is_none() {
                            let first: Vec<Value> = self
                                .persons
                                .get(&var)
                                .and_then(|p| p.get(j))
                                .map(|g| g.iter().take(1).cloned().collect())
                                .unwrap_or_default();
                            self.freeters.insert(var.clone(), first);
                            self.freeters_count.insert(var.clone(), 1);
                            self.institutions.insert(var.clone(), Vec::new());
                            self.persons.insert(var.clone(), Vec::new());
                            self.please_chop = chopvar.clone();
                            break;
                        }
                    }
                }
                if !self
                    .institutions
                    .get(&var)
                    .map(Vec::is_empty)
                    .unwrap_or(true)
                {
                    if self.please_chop.as_deref() == Some(var.as_str()) {
                        if let Some(i) = self.institutions.get_mut(&var) {
                            i.remove(0);
                        }
                        if let Some(c) = self.institutions_count.get_mut(&var) {
                            *c -= 1;
                        }
                        self.please_chop = None;
                    } else if chopvar.is_some() && self.please_chop.is_none() {
                        if let Some(i) = self.institutions.get_mut(&var) {
                            i.truncate(1);
                        }
                        self.institutions_count.insert(var.clone(), 1);
                        self.please_chop = chopvar.clone();
                    }
                }
            }
        }

        // Transliteration and abbreviation mapping: done per name in getName.

        // Could also be factored out to a separate function for clarity.
        // ???? XXX Does this belong?
        // (`v` is not `this.variables[i]`: the loop reads the stale `v` each time.)
        for _ in 0..self.variables.len() {
            let Some(var) = v.clone() else {
                return Err(super::load::type_error(
                    "Cannot read properties of undefined (reading 'length')",
                ));
            };
            if !self
                .institutions
                .get(&var)
                .map(Vec::is_empty)
                .unwrap_or(true)
            {
                self.nameset_offset += 1;
            }
            let groups = self.persons.get(&var).cloned().unwrap_or_default();
            for g in &groups {
                if !g.is_empty() {
                    self.nameset_offset += 1;
                }
            }
        }
        Ok(())
    }

    /// `CSL.NameOutput.prototype._truncateNameList(container, variable,
    /// index)`: keep the first `max_number_of_names` names and the last one
    /// when a list is far too long.
    pub fn truncate_name_list(&self, st: &State, lst: &[Value]) -> Vec<Value> {
        let root = st.area_ref(&st.tmp.area).root.clone();
        let max = st.area_ref(&root).opt.get("max_number_of_names");
        if js::truthy_opt(max) && lst.len() > 50 && (lst.len() as f64) > js_num(max) + 2.0 {
            // Preserve the last name in the list, in case we're rendering with a PI ellipsis (et-al-use-last)
            let limit = js_num(max);
            let mut out = super::util_names_output::slice_head(lst, limit + 1.0);
            if let Some(last) = lst.last() {
                out.push(last.clone());
            }
            return out;
        }
        lst.to_vec()
    }
}
