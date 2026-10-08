// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_names_divide.js
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

//! Port of `src/util_names_divide.js`: splitting a names variable into
//! freestanding names, persons with affiliations and institutions
//! (`CSL.NameOutput.prototype.divideAndTransliterateNames` and helpers).

use serde_json::Value;

use super::build_retrieve_item::normalize_abbrevs_key;
use super::js::{self, Obj};
use super::state::State;
use super::util_names::get_raw_name;
use super::util_transform::get_item_prop;
use super::util_names_output::{js_num, NameOutput};
use super::util_names_tests::is_person;
use super::CslResult;

fn spoof(st: &State) -> bool {
    js::truthy_opt(
        st.opt
            .get("development_extensions")
            .and_then(|d| d.get("spoof_institutional_affiliations")),
    )
}

impl NameOutput {
    /// `CSL.NameOutput.prototype.divideAndTransliterateNames()`.
    pub fn divide_and_transliterate_names(&mut self, st: &mut State) -> CslResult<()> {
        let variables = self.variables.clone();
        self.varnames = variables.clone();
        self.freeters = Default::default();
        self.persons = Default::default();
        self.institutions = Default::default();
        self.keys = Vec::new();
        let spoof = spoof(st);
        for v in &variables {
            if !self.keys.contains(v) {
                self.keys.push(v.clone());
            }
            self.variable_offset.insert(v.clone(), self.nameset_offset);
            let mut values = self.normalize_variable_value(st, v);
            let name = self.name_token()?.clone();
            let suppress_min = name.strings.get("suppress-min").cloned();
            let suppress_max = name.strings.get("suppress-max").cloned();
            if suppress_min.as_ref().map(js::truthy).unwrap_or(false)
                && values.len() as f64 >= js_num(suppress_min.as_ref())
            {
                values = Vec::new();
            }
            if suppress_max.as_ref().map(js::truthy).unwrap_or(false)
                && values.len() as f64 <= js_num(suppress_max.as_ref())
            {
                values = Vec::new();
            }
            self.get_freeters(st, v, &mut values)?;
            self.get_persons_and_institutions(st, v, &values)?;
            if spoof {
                let is_zero =
                    |x: Option<&Value>| x.map(|x| js_num(Some(x)) == 0.0).unwrap_or(false);
                if is_zero(name.strings.get("suppress-min")) {
                    self.freeters.insert(v.clone(), Vec::new());
                    if let Some(p) = self.persons.get_mut(v) {
                        for x in p.iter_mut() {
                            *x = Vec::new();
                        }
                    }
                } else if is_zero(
                    self.institution
                        .as_ref()
                        .and_then(|i| i.strings.get("suppress-min")),
                ) {
                    self.institutions.insert(v.clone(), Vec::new());
                    // this.freeters[v] = this.freeters[v].concat(this.persons[v]);
                    // The persons[v] elements are arrays, concatenated whole
                    // (JS pushes the arrays themselves into the names list).
                    let persons = self.persons.get(v).cloned().unwrap_or_default();
                    let fr = self.freeters.entry(v.clone()).or_default();
                    for group in &persons {
                        fr.push(Value::Array(group.clone()));
                    }
                    for group in &persons {
                        for n in group {
                            fr.push(n.clone());
                        }
                    }
                    self.persons.insert(v.clone(), Vec::new());
                }
            }
        }
        Ok(())
    }

    /// `CSL.NameOutput.prototype._normalizeVariableValue(Item, variable)`:
    /// the names of `variable` as a fresh list (a string or number becomes
    /// one literal name; a lone object a one-element list). Read through
    /// [`get_item_prop`], because upstream's `Item` is the shared object the
    /// `@variable` closure has just rewritten with the `authority` split (#808).
    pub fn normalize_variable_value(&mut self, st: &State, variable: &str) -> Vec<Value> {
        match get_item_prop(st, &self.item, variable) {
            Some(Value::String(s)) => {
                // name variable is string or number, not array. Attempting to fix.
                let mut o = Obj::new();
                o.insert("literal".into(), Value::String(s));
                vec![Value::Object(o)]
            }
            Some(Value::Number(n)) => {
                let mut o = Obj::new();
                o.insert(
                    "literal".into(),
                    Value::String(js::to_js_string(&Value::Number(n))),
                );
                vec![Value::Object(o)]
            }
            Some(Value::Array(a)) => a,
            Some(v) if js::truthy(&v) => {
                // name variable is object, not array. Attempting to fix.
                if let Value::Object(o) = &mut self.item {
                    o.insert(variable.to_string(), Value::Array(vec![v.clone()]));
                }
                vec![v]
            }
            _ => Vec::new(),
        }
    }

    /// `CSL.NameOutput.prototype._getFreeters(v, values)`: moves the
    /// freestanding names out of `values`.
    pub fn get_freeters(
        &mut self,
        st: &mut State,
        v: &str,
        values: &mut Vec<Value>,
    ) -> CslResult<()> {
        let mut freeters: Vec<Value> = Vec::new();
        if spoof(st) {
            let mut i = values.len() as i64 - 1;
            while i > -1 {
                let last_is_person = values.last().map(is_person).unwrap_or(false);
                if last_is_person {
                    let popped = values.pop().unwrap_or(Value::Null);
                    let value = self.check_nickname(st, popped)?;
                    if js::truthy(&value) {
                        freeters.push(value);
                    }
                } else {
                    break;
                }
                i -= 1;
            }
        } else {
            let mut i = values.len() as i64 - 1;
            while i > -1 {
                let mut value = values.pop().unwrap_or(Value::Null);
                if is_person(&value) {
                    value = self.check_nickname(st, value)?;
                }
                freeters.push(value);
                i -= 1;
            }
        }
        freeters.reverse();
        if !freeters.is_empty() {
            self.nameset_offset += 1;
        }
        self.freeters.insert(v.to_string(), freeters);
        Ok(())
    }

    /// `CSL.NameOutput.prototype._getPersonsAndInstitutions(v, values)`.
    pub fn get_persons_and_institutions(
        &mut self,
        st: &mut State,
        v: &str,
        values: &[Value],
    ) -> CslResult<()> {
        self.persons.insert(v.to_string(), Vec::new());
        self.institutions.insert(v.to_string(), Vec::new());
        if !spoof(st) {
            return Ok(());
        }
        let mut persons: Vec<Value> = Vec::new();
        let mut has_affiliates = false;
        let mut first = true;
        let mut out_persons: Vec<Vec<Value>> = Vec::new();
        let mut out_institutions: Vec<Value> = Vec::new();
        let mut i = values.len() as i64 - 1;
        while i > -1 {
            let val = &values[i as usize];
            if is_person(val) {
                let value = self.check_nickname(st, val.clone())?;
                if js::truthy(&value) {
                    persons.push(value);
                }
            } else {
                has_affiliates = true;
                out_institutions.push(val.clone());
                if !first {
                    persons.reverse();
                    out_persons.push(std::mem::take(&mut persons));
                }
                first = false;
            }
            i -= 1;
        }
        if has_affiliates {
            persons.reverse();
            out_persons.push(persons);
            out_persons.reverse();
            out_institutions.reverse();
        }
        self.persons.insert(v.to_string(), out_persons);
        self.institutions.insert(v.to_string(), out_institutions);
        Ok(())
    }

    /// `CSL.NameOutput.prototype._clearValues(values)`: pop everything.
    pub fn clear_values(&self, values: &mut Vec<Value>) {
        values.clear();
    }

    /// `CSL.NameOutput.prototype._checkNickname(name)`: replace the name of
    /// an interview or personal communication by its nickname abbreviation,
    /// or `false` for the `!here>>>` marker.
    pub fn check_nickname(&self, st: &mut State, name: Value) -> CslResult<Value> {
        let ty = self.item.get("type").and_then(Value::as_str);
        if matches!(ty, Some("interview") | Some("personal_communication")) {
            let author = get_raw_name(&name);
            let suppress = js::truthy(&self.cite_item)
                && js::truthy_opt(self.cite_item.get("suppress-author"));
            if !author.is_empty() && !suppress {
                // sys.getAbbreviation and sys.normalizeAbbrevsKey exist (the
                // host's sys; build_retrieve_item.rs).
                let normalized_key = normalize_abbrevs_key("author", Some(&author));
                let mut jur = Some("default".to_string());
                let lang = self.item.get("language").filter(|l| js::truthy(l)).map(js::to_js_string);
                let my_local_name = super::util_transform::load_and_get_abbreviation(
                    st,
                    &mut jur,
                    "nickname",
                    &normalized_key,
                    lang.as_deref(),
                );
                if let Some(local) = my_local_name {
                    if local == "!here>>>" {
                        return Ok(Value::Bool(false));
                    }
                    let mut o = Obj::new();
                    o.insert("family".into(), Value::String(local));
                    o.insert("given".into(), Value::String(String::new()));
                    return Ok(Value::Object(o));
                }
            }
        }
        Ok(name)
    }
}
