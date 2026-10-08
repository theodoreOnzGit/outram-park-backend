// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_datenode.js
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

//! Port of `src/util_datenode.js`: `CSL.Util.fixDateNode`.
//!
//! `fixDateNode` merges a style's `cs:date` element (with its `date-part`
//! overrides) into the locale's date template of the same `form`, producing
//! the date node the build loop ([`State::run_builder`]) then compiles. It
//! works on the style XML through `this.cslXml.*` ([`XmlJson`]) and on the
//! engine through `this.getDate`, `this.opt` and `this.build`, so it is a
//! method of [`State`] here.
//!
//! Attribute values are kept as the JSON values they are in the XML tree
//! (`cslid` is a number there), which the `xml` section of the intermediate
//! dump compares with citeproc-js for all 850 cases.
//!
//! V&V: `tests/citeproc_intermediate.rs` (the `xml` and `style` sections of
//! every fixture that has a `cs:date`; all equal citeproc-js's, see its
//! results note).

use serde_json::Value;

use super::js;
use super::state::State;
use super::xmljson::{NodeId, XmlChild};
use super::CslResult;

/// The closures `src/util_datenode.js` stores in `token.execs`: none.
#[derive(Debug, Clone, PartialEq)]
pub enum UtilDatenodeExec {}

impl UtilDatenodeExec {
    /// Run the closure.
    pub fn run(
        &self,
        _state: &mut State,
        _token: &mut super::obj_token::Token,
        _item: &Value,
        _cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match *self {}
    }
}

impl State {
    /// `CSL.Util.fixDateNode.call(state, parent, pos, node)`: replaces the `<date>` node `node` (child `pos` of `parent`) by a copy
    /// of the locale's date template of the same `form`, carrying over the
    /// style's `variable`, affixes, `date-parts` (dropping month/day/year
    /// parts) and per-`date-part` attributes. Returns `parent`.
    pub fn fix_date_node(&mut self, parent: NodeId, pos: usize, node: NodeId) -> CslResult<NodeId> {
        let default_locale = self.csl_xml.get_attribute_value(node, "default-locale");

        // Raise date flag, used to control inclusion of year-suffix key in sorts
        // This may be a little reckless: not sure what happens on no-date conditions
        self.build.date_key = true;

        let form = self.csl_xml.get_attribute_string(node, "form");
        let default_locale_t = js::truthy(&default_locale);
        let lingo = if default_locale_t {
            self.opt
                .get("default-locale")
                .and_then(|d| d.get(0))
                .map(js::to_js_string)
                .unwrap_or_default()
        } else {
            self.csl_xml.get_attribute_string(node, "lingo")
        };

        let Some(template) = self.get_date(&form, default_locale_t)? else {
            return Ok(parent);
        };

        let dateparts = self.csl_xml.get_attribute_value(node, "date-parts");

        let variable = self.csl_xml.get_attribute_value(node, "variable");
        let prefix = self.csl_xml.get_attribute_value(node, "prefix");
        let suffix = self.csl_xml.get_attribute_value(node, "suffix");
        let display = self.csl_xml.get_attribute_value(node, "display");
        let cslid = self.csl_xml.get_attribute_value(node, "cslid");

        //
        // Xml: Copy a node
        //
        let datexml = self.csl_xml.node_copy_tree(&template);
        let lang = js::get_string(&self.opt, "lang").unwrap_or_default();
        self.csl_xml
            .set_attribute(datexml, "lingo", Value::String(lang.clone()));
        self.csl_xml
            .set_attribute(datexml, "form", Value::String(form));
        self.csl_xml.set_attribute(datexml, "date-parts", dateparts);
        self.csl_xml.set_attribute(datexml, "cslid", cslid);
        //
        // Xml: Set attribute
        //
        self.csl_xml.set_attribute(datexml, "variable", variable);
        self.csl_xml
            .set_attribute(datexml, "default-locale", default_locale.clone());
        if js::truthy(&prefix) {
            self.csl_xml.set_attribute(datexml, "prefix", prefix);
        }
        if js::truthy(&suffix) {
            self.csl_xml.set_attribute(datexml, "suffix", suffix);
        }
        if js::truthy(&display) {
            self.csl_xml.set_attribute(datexml, "display", display);
        }
        //
        // Step through any date-part children of the layout date node,
        // and lay their attributes onto the corresponding node in the
        // locale template node copy.
        //
        // tests: language_BaseLocale
        // tests: date_LocalizedTextInStyleLocaleWithTextCase
        //
        for subnode in self.csl_xml.children(datexml) {
            let XmlChild::Node(subnode) = subnode else {
                continue;
            };
            if self.csl_xml.nodename(subnode) == "date-part" {
                let partname = self.csl_xml.get_attribute_string(subnode, "name");
                if default_locale_t {
                    self.csl_xml
                        .set_attribute_on_node_identified_by_name_attribute(
                            datexml,
                            "date-part",
                            &partname,
                            "@default-locale",
                            Value::String("true".to_string()),
                        );
                }
            }
        }

        for subnode in self.csl_xml.children(node) {
            let XmlChild::Node(subnode) = subnode else {
                continue;
            };
            if self.csl_xml.nodename(subnode) == "date-part" {
                let partname = self.csl_xml.get_attribute_string(subnode, "name");
                for (attr, val) in self.csl_xml.attributes(subnode) {
                    if attr == "@name" {
                        continue;
                    }
                    if !lingo.is_empty()
                        && lingo != lang
                        && ["@suffix", "@prefix", "@form"].contains(&attr.as_str())
                    {
                        continue;
                    }
                    self.csl_xml
                        .set_attribute_on_node_identified_by_name_attribute(
                            datexml,
                            "date-part",
                            &partname,
                            &attr,
                            val,
                        );
                }
            }
        }

        let date_parts_attr = self.csl_xml.get_attribute_string(node, "date-parts");
        if date_parts_attr == "year" {
            //
            // Xml: Find one node by attribute and delete
            //
            self.csl_xml.delete_node_by_name_attribute(datexml, "month");
            self.csl_xml.delete_node_by_name_attribute(datexml, "day");
        } else if date_parts_attr == "year-month" {
            self.csl_xml.delete_node_by_name_attribute(datexml, "day");
        } else if date_parts_attr == "month-day" {
            let child_nodes = self.csl_xml.children(datexml);
            for i in 1..child_nodes.len() {
                if let XmlChild::Node(c) = &child_nodes[i] {
                    if self.csl_xml.get_attribute_value(*c, "name").as_str() == Some("year") {
                        if let XmlChild::Node(prev) = &child_nodes[i - 1] {
                            self.csl_xml.set_attribute(
                                *prev,
                                "suffix",
                                Value::String(String::new()),
                            );
                        }
                        break;
                    }
                }
            }
            self.csl_xml.delete_node_by_name_attribute(datexml, "year");
        }
        Ok(self
            .csl_xml
            .insert_child_node_after(parent, node, pos, datexml))
    }
}
