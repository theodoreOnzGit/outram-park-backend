// Part of the kovan port of citeproc-js (GitHub #790, #792).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_modules.js
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

//! Jurisdiction style modules (src/util_modules.js): `getJurisdictionList`,
//! `loadStyleModule`, `retrieveAllStyleModules`.
//!
//! A style module is a separate CSL file of legal macros for one
//! jurisdiction (`juris-main`, `juris-title`, ...), fetched through the
//! `sys.retrieveStyleModule(jurisdiction, preference)` hook and compiled
//! into `state.juris[jurisdiction]`. The test suite's runner defines the hook
//! as `return null`, and this port's [`Sys`](super::Sys) has no such hook
//! ([`Sys::has_retrieve_style_module`](super::Sys::has_retrieve_style_module)
//! is false), so no module is ever loaded; the loading code is ported so the
//! path exists, but is **untested** (nothing can reach it).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use regex::Regex;

use super::js;
use super::load;
use super::obj_token::Token;
use super::state::State;
use super::system;
use super::CslResult;

/// One entry of `state.juris[jurisdiction]`: `{types: {...}, <macro name>:
/// [tokens]}`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Juris {
    /// `types`: the item types the module covers (`CSL.MODULE_TYPES` unless
    /// the module's `law-module` says otherwise).
    pub types: BTreeSet<String>,
    /// The module's compiled macros by name (`juris-main`, ...).
    pub macros: BTreeMap<String, Vec<Token>>,
}

impl State {
    /// `CSL.Engine.prototype.getJurisdictionList(jurisdiction)`: the
    /// jurisdiction and its ancestors (`us:c:ma` → `us:c:ma`, `us:c`, `us`),
    /// each followed by its configured fallback, and `us` last if absent.
    pub fn get_jurisdiction_list(&self, jurisdiction: &str) -> Vec<String> {
        let mut list: Vec<String> = Vec::new();
        let elems: Vec<&str> = jurisdiction.split(':').collect();
        for j in (1..=elems.len()).rev() {
            let composed_id = elems[..j].join(":");
            list.push(composed_id.clone());
            let fallback = self
                .opt
                .get("jurisdiction_fallbacks")
                .and_then(|f| f.get(&composed_id))
                .filter(|f| js::truthy(f))
                .map(js::to_js_string);
            if let Some(fallback) = fallback {
                list.push(fallback);
            }
        }
        if !list.iter().any(|j| j == "us") {
            list.push("us".to_string());
        }
        list
    }

    /// `CSL.Engine.prototype.loadStyleModule(jurisdiction, xmlSource,
    /// skipFallback)`: compile the `juris-*` macros of a module. Returns the
    /// module's `fallback` jurisdiction, if it names one and `skip_fallback`
    /// is off.
    pub fn load_style_module(
        &mut self,
        jurisdiction: &str,
        xml_source: &str,
        skip_fallback: bool,
    ) -> CslResult<Option<String>> {
        static WS: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(&format!("{}+", load::WS_CLASS)).expect("static"));
        let mut my_fallback: Option<String> = None;
        self.juris
            .insert(jurisdiction.to_string(), Juris::default());
        let mut my_xml = system::setup_xml(xml_source)?;
        if let Some(root) = my_xml.data_obj {
            my_xml.add_missing_name_nodes(root, &mut Vec::new());
            my_xml.add_institution_nodes(root)?;
            my_xml.insert_publisher_and_place(root)?;
            my_xml.flag_date_macros(root);
        }
        let mut types_set = false;
        for node in my_xml.get_nodes_by_name(my_xml.data_obj, "law-module", "") {
            let my_types = my_xml.get_attribute_string(node, "types");
            if !my_types.is_empty() {
                let mut types = BTreeSet::new();
                for t in js::split(&WS, &my_types) {
                    types.insert(t);
                }
                if let Some(j) = self.juris.get_mut(jurisdiction) {
                    j.types = types;
                }
                types_set = true;
            }
            if !skip_fallback {
                let fb = my_xml.get_attribute_string(node, "fallback");
                my_fallback = Some(fb.clone());
                if !fb.is_empty() && jurisdiction != "us" {
                    if let Some(serde_json::Value::Object(f)) =
                        self.opt.get_mut("jurisdiction_fallbacks")
                    {
                        f.insert(jurisdiction.to_string(), serde_json::Value::String(fb));
                    }
                }
            }
        }
        let lang = match js::get_str(&self.opt, "lang") {
            Some(l) if !l.is_empty() => l.to_string(),
            _ => self
                .opt
                .get("default-locale")
                .and_then(|d| d.get(0))
                .map(js::to_js_string)
                .unwrap_or_default(),
        };
        load::set_court_classes(self, &lang, &my_xml, my_xml.data_obj)?;
        if !types_set {
            if let Some(j) = self.juris.get_mut(jurisdiction) {
                j.types = load::MODULE_TYPES.iter().map(|s| s.to_string()).collect();
            }
        }
        // Must use the same XML parser for style and modules: the builder
        // reads nodes through `self.csl_xml`, so the module's document is
        // swapped in while its macros are built.
        let macro_nodes = my_xml.get_nodes_by_name(my_xml.data_obj, "macro", "");
        let saved = std::mem::replace(&mut self.csl_xml, my_xml);
        let mut result: CslResult<()> = Ok(());
        for node in macro_nodes {
            let name = self.csl_xml.get_attribute_string(node, "name");
            if !load::is_module_macro(&name) {
                // CSL.debug("CSL: skipping non-modular macro name ...")
                continue;
            }
            let mut tokens: Vec<Token> = Vec::new();
            if let Err(e) = self.build_token_lists(&[node], &mut tokens) {
                result = Err(e);
                break;
            }
            if let Err(e) = self.configure_token_list(&mut tokens) {
                result = Err(e);
                break;
            }
            if let Some(j) = self.juris.get_mut(jurisdiction) {
                j.macros.insert(name, tokens);
            }
        }
        self.csl_xml = saved;
        result?;
        Ok(my_fallback.filter(|f| !f.is_empty()))
    }

    /// `CSL.Engine.prototype.retrieveAllStyleModules(jurisdictionList)`: ask
    /// `sys.retrieveStyleModule` for each jurisdiction, for each of the
    /// locale's `jurisdiction-preference`s (last first, then none), marking
    /// jurisdictions "seen" in `opt.jurisdictions_seen`. Returns the sources
    /// found, in the order found (later finds replace earlier ones).
    pub fn retrieve_all_style_modules(
        &mut self,
        jurisdiction_list: &[String],
    ) -> Vec<(String, String)> {
        let mut ret: Vec<(String, String)> = Vec::new();
        let preferences: Vec<String> = self
            .locale
            .get(js::get_str(&self.opt, "lang").unwrap_or(""))
            .and_then(|l| l.opts.get("jurisdiction-preference"))
            .and_then(serde_json::Value::as_array)
            .map(|a| a.iter().map(js::to_js_string).collect())
            .unwrap_or_default();
        let mut prefs = vec![String::new()];
        prefs.extend(preferences);
        for preference in prefs.iter().rev() {
            for jurisdiction in jurisdiction_list {
                // If we've "seen" it, we have it already, or we're not going to get it.
                let seen = self
                    .opt
                    .get("jurisdictions_seen")
                    .and_then(|s| s.get(jurisdiction))
                    .map(js::truthy)
                    .unwrap_or(false);
                if seen {
                    continue;
                }
                // Try to get the module
                let res = self.sys.retrieve_style_module(jurisdiction, preference);
                // If we fail and we've run out of preferences, mark as "seen"
                // Otherwise mark as "seen" if we get something.
                if (res.is_none() && preference.is_empty()) || res.is_some() {
                    if let Some(serde_json::Value::Object(s)) =
                        self.opt.get_mut("jurisdictions_seen")
                    {
                        s.insert(jurisdiction.clone(), serde_json::Value::Bool(true));
                    }
                }
                // Don't memo unless get got style code.
                let Some(res) = res else { continue };
                if let Some(slot) = ret.iter_mut().find(|(k, _)| k == jurisdiction) {
                    slot.1 = res;
                } else {
                    ret.push((jurisdiction.clone(), res));
                }
            }
        }
        // Give 'em what we got.
        ret
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn jurisdiction_list_walks_up_and_ends_with_us() {
        let mut st = State::default();
        st.opt
            .insert("jurisdiction_fallbacks".into(), json!({"us:c": "ca"}));
        assert_eq!(
            st.get_jurisdiction_list("us:c:ma"),
            vec!["us:c:ma", "us:c", "ca", "us"]
        );
        assert_eq!(st.get_jurisdiction_list("de"), vec!["de", "us"]);
    }
}
