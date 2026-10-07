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
//! the date node the build loop then compiles. It works on the style XML
//! through `this.cslXml.*` methods and on the engine through
//! `this.getDate`, `this.opt` and `this.build`; none of these exist yet in the
//! Rust port, so the function is written against the small trait
//! [`DateNodeHost`] listing exactly the calls it makes. The owner of
//! `xmljson.js` / `build.js` implements the trait for the real engine (a thin
//! glue layer); the tests below implement it over a toy tree.

use serde_json::Value;

use super::state::State;
use super::CslResult;

/// The closures `src/util_datenode.js` stores in `token.execs`: none.
#[derive(Debug, Clone, PartialEq)]
pub enum UtilDatenodeExec {}

impl UtilDatenodeExec {
    /// Run the closure.
    pub fn run(
        &self,
        _state: &mut State,
        _token: &super::obj_token::Token,
        _item: &Value,
        _cite_item: &Value,
    ) -> CslResult<Option<usize>> {
        match *self {}
    }
}

/// The engine and XML operations `CSL.Util.fixDateNode` uses (`this.cslXml.*`,
/// `this.getDate`, `this.opt`, `this.build`). `Node` is the XML node handle.
/// Method names follow the JS methods they stand for.
pub trait DateNodeHost {
    /// The XML node handle (`CSL.XmlJSON` node / DOM element).
    type Node: Clone + PartialEq;

    /// `this.cslXml.getAttributeValue(node, name)`: `""` when absent.
    fn get_attribute_value(&self, node: &Self::Node, name: &str) -> String;
    /// `this.cslXml.nodeCopy(node)`.
    fn node_copy(&mut self, node: &Self::Node) -> Self::Node;
    /// `this.cslXml.setAttribute(node, name, value)`. Upstream stores the
    /// value as given, including `undefined`/`""`; `None` is `undefined`.
    fn set_attribute(&mut self, node: &Self::Node, name: &str, value: Option<&str>);
    /// `this.cslXml.children(node)`.
    fn children(&self, node: &Self::Node) -> Vec<Self::Node>;
    /// `this.cslXml.nodename(node)`.
    fn nodename(&self, node: &Self::Node) -> String;
    /// `this.cslXml.attributes(node)`: `(name, value)` pairs in key order, each
    /// name with its leading `@`.
    fn attributes(&self, node: &Self::Node) -> Vec<(String, String)>;
    /// `this.cslXml.setAttributeOnNodeIdentifiedByNameAttribute(node, nodename,
    /// partname, attrname, val)`.
    fn set_attribute_on_node_identified_by_name_attribute(
        &mut self,
        node: &Self::Node,
        nodename: &str,
        partname: &str,
        attrname: &str,
        val: &str,
    );
    /// `this.cslXml.deleteNodeByNameAttribute(node, val)` (quirk: upstream
    /// removes while iterating and may skip an element).
    fn delete_node_by_name_attribute(&mut self, node: &Self::Node, val: &str);
    /// `this.cslXml.insertChildNodeAfter(parent, node, pos, datexml)`: replaces
    /// `node` by `datexml` inside `parent` and returns the parent.
    fn insert_child_node_after(
        &mut self,
        parent: &Self::Node,
        node: &Self::Node,
        pos: usize,
        datexml: &Self::Node,
    ) -> Self::Node;
    /// `this.getDate(form, forceDefaultLocale)`: the locale's date template
    /// node, `None` when there is none.
    fn get_date(&mut self, form: &str, force_default_locale: bool) -> Option<Self::Node>;
    /// `this.opt["default-locale"][0]`.
    fn default_locale(&self) -> String;
    /// `this.opt.lang`.
    fn lang(&self) -> String;
    /// `this.build.date_key = true`.
    fn raise_date_key(&mut self);
}

/// `CSL.Util.fixDateNode.call(engine, parent, pos, node)`: returns the
/// (possibly modified) `parent`.
pub fn fix_date_node<H: DateNodeHost>(
    host: &mut H,
    parent: &H::Node,
    pos: usize,
    node: &H::Node,
) -> H::Node {
    let default_locale = host.get_attribute_value(node, "default-locale");

    // Raise date flag, used to control inclusion of year-suffix key in sorts
    // This may be a little reckless: not sure what happens on no-date conditions
    host.raise_date_key();

    let form = host.get_attribute_value(node, "form");
    let lingo = if !default_locale.is_empty() {
        host.default_locale()
    } else {
        host.get_attribute_value(node, "lingo")
    };

    let force_default = !default_locale.is_empty();
    if host.get_date(&form, force_default).is_none() {
        return parent.clone();
    }

    let dateparts = host.get_attribute_value(node, "date-parts");

    let variable = host.get_attribute_value(node, "variable");
    let prefix = host.get_attribute_value(node, "prefix");
    let suffix = host.get_attribute_value(node, "suffix");
    let display = host.get_attribute_value(node, "display");
    let cslid = host.get_attribute_value(node, "cslid");

    //
    // Xml: Copy a node
    //
    let Some(template) = host.get_date(&form, force_default) else {
        return parent.clone();
    };
    let datexml = host.node_copy(&template);
    let lang = host.lang();
    host.set_attribute(&datexml, "lingo", Some(&lang));
    host.set_attribute(&datexml, "form", Some(&form));
    host.set_attribute(&datexml, "date-parts", Some(&dateparts));
    host.set_attribute(&datexml, "cslid", Some(&cslid));
    //
    // Xml: Set attribute
    //
    host.set_attribute(&datexml, "variable", Some(&variable));
    host.set_attribute(&datexml, "default-locale", Some(&default_locale));
    //
    // Xml: Set flag
    //
    if !prefix.is_empty() {
        host.set_attribute(&datexml, "prefix", Some(&prefix));
    }
    if !suffix.is_empty() {
        host.set_attribute(&datexml, "suffix", Some(&suffix));
    }
    if !display.is_empty() {
        host.set_attribute(&datexml, "display", Some(&display));
    }
    //
    // Step through any date-part children of the layout date node,
    // and lay their attributes onto the corresponding node in the
    // locale template node copy.
    //
    // tests: language_BaseLocale
    // tests: date_LocalizedTextInStyleLocaleWithTextCase
    //
    for subnode in host.children(&datexml) {
        if host.nodename(&subnode) == "date-part" {
            let partname = host.get_attribute_value(&subnode, "name");
            if !default_locale.is_empty() {
                host.set_attribute_on_node_identified_by_name_attribute(
                    &datexml,
                    "date-part",
                    &partname,
                    "@default-locale",
                    "true",
                );
            }
        }
    }

    for subnode in host.children(node) {
        if host.nodename(&subnode) == "date-part" {
            let partname = host.get_attribute_value(&subnode, "name");
            for (attr, val) in host.attributes(&subnode) {
                if attr == "@name" {
                    continue;
                }
                if !lingo.is_empty()
                    && lingo != host.lang()
                    && ["@suffix", "@prefix", "@form"].contains(&attr.as_str())
                {
                    continue;
                }
                host.set_attribute_on_node_identified_by_name_attribute(
                    &datexml,
                    "date-part",
                    &partname,
                    &attr,
                    &val,
                );
            }
        }
    }

    let date_parts_attr = host.get_attribute_value(node, "date-parts");
    if date_parts_attr == "year" {
        //
        // Xml: Find one node by attribute and delete
        //
        host.delete_node_by_name_attribute(&datexml, "month");
        //
        // Xml: Find one node by attribute and delete
        //
        host.delete_node_by_name_attribute(&datexml, "day");
    } else if date_parts_attr == "year-month" {
        host.delete_node_by_name_attribute(&datexml, "day");
    } else if date_parts_attr == "month-day" {
        //
        // Xml: Get child nodes
        //
        let child_nodes = host.children(&datexml);
        for i in 1..child_nodes.len() {
            //
            // Xml: Get attribute value (for string comparison)
            //
            if host.get_attribute_value(&child_nodes[i], "name") == "year" {
                //
                // Xml: Set attribute value
                //
                host.set_attribute(&child_nodes[i - 1], "suffix", Some(""));
                break;
            }
        }
        host.delete_node_by_name_attribute(&datexml, "year");
    }
    host.insert_child_node_after(parent, node, pos, &datexml)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A toy tree host: nodes are indices into an arena.
    #[derive(Default)]
    struct Toy {
        names: Vec<String>,
        attrs: Vec<Vec<(String, String)>>,
        kids: Vec<Vec<usize>>,
        template: Option<usize>,
        date_key: bool,
    }

    impl Toy {
        fn add(&mut self, name: &str, attrs: &[(&str, &str)], kids: &[usize]) -> usize {
            self.names.push(name.to_string());
            self.attrs.push(
                attrs
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            );
            self.kids.push(kids.to_vec());
            self.names.len() - 1
        }
    }

    impl DateNodeHost for Toy {
        type Node = usize;
        fn get_attribute_value(&self, n: &usize, name: &str) -> String {
            self.attrs[*n]
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.clone())
                .unwrap_or_default()
        }
        fn node_copy(&mut self, n: &usize) -> usize {
            let (name, attrs) = (self.names[*n].clone(), self.attrs[*n].clone());
            let kids: Vec<usize> = self.kids[*n].clone();
            let copies: Vec<usize> = kids.iter().map(|k| self.node_copy(k)).collect();
            self.names.push(name);
            self.attrs.push(attrs);
            self.kids.push(copies);
            self.names.len() - 1
        }
        fn set_attribute(&mut self, n: &usize, name: &str, value: Option<&str>) {
            let v = value.unwrap_or("").to_string();
            match self.attrs[*n].iter_mut().find(|(k, _)| k == name) {
                Some(p) => p.1 = v,
                None => self.attrs[*n].push((name.to_string(), v)),
            }
        }
        fn children(&self, n: &usize) -> Vec<usize> {
            self.kids[*n].clone()
        }
        fn nodename(&self, n: &usize) -> String {
            self.names[*n].clone()
        }
        fn attributes(&self, n: &usize) -> Vec<(String, String)> {
            self.attrs[*n]
                .iter()
                .map(|(k, v)| (format!("@{k}"), v.clone()))
                .collect()
        }
        fn set_attribute_on_node_identified_by_name_attribute(
            &mut self,
            n: &usize,
            nodename: &str,
            partname: &str,
            attrname: &str,
            val: &str,
        ) {
            let attrname = attrname.strip_prefix('@').unwrap_or(attrname).to_string();
            for k in self.kids[*n].clone() {
                if self.names[k] == nodename && self.get_attribute_value(&k, "name") == partname {
                    self.set_attribute(&k, &attrname, Some(val));
                }
            }
        }
        fn delete_node_by_name_attribute(&mut self, n: &usize, val: &str) {
            // Upstream removes while iterating, so an element right after a
            // removed one is skipped.
            let ilen = self.kids[*n].len();
            for i in 0..ilen {
                if i >= self.kids[*n].len() {
                    continue;
                }
                let k = self.kids[*n][i];
                if self.get_attribute_value(&k, "name") == val {
                    self.kids[*n].remove(i);
                }
            }
        }
        fn insert_child_node_after(
            &mut self,
            parent: &usize,
            node: &usize,
            _pos: usize,
            datexml: &usize,
        ) -> usize {
            if let Some(i) = self.kids[*parent].iter().position(|k| k == node) {
                self.kids[*parent][i] = *datexml;
            }
            *parent
        }
        fn get_date(&mut self, _form: &str, _force: bool) -> Option<usize> {
            self.template
        }
        fn default_locale(&self) -> String {
            "en-US".into()
        }
        fn lang(&self) -> String {
            "en-US".into()
        }
        fn raise_date_key(&mut self) {
            self.date_key = true;
        }
    }

    #[test]
    fn date_parts_year_deletes_month_and_day_and_replaces_the_node() {
        let mut t = Toy::default();
        let y = t.add("date-part", &[("name", "year")], &[]);
        let m = t.add("date-part", &[("name", "month")], &[]);
        let d = t.add("date-part", &[("name", "day")], &[]);
        let template = t.add("date", &[("form", "numeric")], &[y, m, d]);
        t.template = Some(template);
        let style_part = t.add("date-part", &[("name", "year"), ("suffix", ".")], &[]);
        let node = t.add(
            "date",
            &[("form", "numeric"), ("variable", "issued"), ("date-parts", "year")],
            &[style_part],
        );
        let parent = t.add("macro", &[], &[node]);
        let out = fix_date_node(&mut t, &parent, 0, &node);
        assert_eq!(out, parent);
        assert!(t.date_key);
        let new_node = t.kids[parent][0];
        assert_ne!(new_node, node);
        assert_eq!(t.get_attribute_value(&new_node, "variable"), "issued");
        assert_eq!(t.get_attribute_value(&new_node, "date-parts"), "year");
        let names: Vec<String> = t.kids[new_node]
            .iter()
            .map(|k| t.get_attribute_value(k, "name"))
            .collect();
        assert_eq!(names, vec!["year".to_string()]);
        // the style's date-part attributes were laid onto the template copy
        let y2 = t.kids[new_node][0];
        assert_eq!(t.get_attribute_value(&y2, "suffix"), ".");
    }

    #[test]
    fn missing_locale_template_returns_parent_unchanged() {
        let mut t = Toy::default();
        let node = t.add("date", &[("form", "text")], &[]);
        let parent = t.add("macro", &[], &[node]);
        assert_eq!(fix_date_node(&mut t, &parent, 0, &node), parent);
        assert_eq!(t.kids[parent][0], node);
    }
}
