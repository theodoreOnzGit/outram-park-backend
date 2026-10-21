// Part of the kovan port of citeproc-js (GitHub #790, #792).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/xmljson.js
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

//! `CSL.parseXml` and `CSL.XmlJSON` (src/xmljson.js): the style and locale
//! XML as citeproc-js sees it, a tree of `{name, attrs, children}` nodes whose
//! text children are plain strings.
//!
//! # Node representation (arena)
//!
//! citeproc-js mutates nodes **in place** and holds references to them across
//! those mutations (`CSL.makeBuilder` walks a node's children while
//! `CSL.Util.fixDateNode` replaces one of them; `CSL.Node.sort` and the
//! locale code keep nodes of the style). So an [`XmlJson`] is an **arena**:
//! a `Vec<XmlNode>` indexed by [`NodeId`], and a node's children are
//! [`XmlChild`]s, either a [`NodeId`] or a text string. Replacing a child
//! (`insertChildNodeAfter`) swaps one `XmlChild`; every other id stays valid.
//! One [`XmlJson`] is one document (the style, or one locale file); a
//! [`NodeId`] is meaningful only for the document that made it.
//!
//! A node that has to outlive its document, or move between documents (a
//! locale's `<date>` template is copied into the style tree by
//! `fixDateNode`; the intermediate dump prints it), is exported as an owned
//! [`XmlTree`] and imported again with [`XmlJson::import_tree`] /
//! [`XmlJson::node_copy_tree`]. `XmlTree` keeps attributes in insertion order,
//! because citeproc-js applies a node's attributes in the order the object
//! holds them.
//!
//! Attribute values are [`serde_json::Value`]s: strings from the parser, but
//! `true` (`has-publisher-and-publisher-place`) and numbers (`cslid`) appear.
//! JS `undefined` is "absent".
//!
//! # JS quirks kept
//!
//! * `getNodeValue(array)` returns the array itself (always truthy) so
//!   `expandMacro`'s "undefined macro" error is unreachable; that is in
//!   `util_nodes.rs`, which relies on [`XmlJson::get_nodes_by_name`] returning
//!   a (possibly empty) list.
//! * `deleteNodeByNameAttribute` removes while iterating and so skips the
//!   element that slides into the removed slot.
//! * `nodeCopy` drops every attribute value that is not a string.
//! * `deleteAttribute` throws a TypeError in upstream when the attribute
//!   exists (`attrs.pop` is not a function); so does this port.
//! * `parseXml` is a regex-and-split scraper, not an XML parser: an
//!   attribute value is found by the *last* ` name="..."` in the tag, text
//!   content is taken after the *last* `>`, and the entity decoder runs
//!   `&amp;` first.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Map, Value};

use super::js;
use super::load;
use super::{CslResult, EngineError};

/// Index of a node in its [`XmlJson`] arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub usize);

/// One child of a node: a text string or a node (`children[i]`).
#[derive(Debug, Clone, PartialEq)]
pub enum XmlChild {
    /// A JS string child (`"string" === typeof children[i]`).
    Text(String),
    /// An object child.
    Node(NodeId),
}

/// An arena node: `{name, attrs, children}`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct XmlNode {
    /// `name` (the tag). Empty for upstream's `null` name (degenerate input).
    pub name: String,
    /// `attrs`, in insertion order.
    pub attrs: Vec<(String, Value)>,
    /// `children`.
    pub children: Vec<XmlChild>,
}

/// An owned node tree: a node detached from any arena (see the module docs).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct XmlTree {
    /// `name`.
    pub name: String,
    /// `attrs`, in insertion order.
    pub attrs: Vec<(String, Value)>,
    /// `children`.
    pub children: Vec<XmlTreeChild>,
}

/// A child of an [`XmlTree`].
#[derive(Debug, Clone, PartialEq)]
pub enum XmlTreeChild {
    /// A string child.
    Text(String),
    /// A node child.
    Node(XmlTree),
}

impl XmlTree {
    /// The JSON object citeproc-js holds, `{"name", "attrs", "children"}`;
    /// object keys sort (serde_json's map), which is the canonical form the
    /// intermediate dump needs anyway.
    pub fn to_value(&self) -> Value {
        let mut m = Map::new();
        m.insert("name".into(), Value::String(self.name.clone()));
        let mut attrs = Map::new();
        for (k, v) in &self.attrs {
            attrs.insert(k.clone(), v.clone());
        }
        m.insert("attrs".into(), Value::Object(attrs));
        m.insert(
            "children".into(),
            Value::Array(
                self.children
                    .iter()
                    .map(|c| match c {
                        XmlTreeChild::Text(s) => Value::String(s.clone()),
                        XmlTreeChild::Node(n) => n.to_value(),
                    })
                    .collect(),
            ),
        );
        Value::Object(m)
    }

    /// From a JSON object `{name, attrs, children}` (the serialized-JSON
    /// style form `CSL.setupXml` accepts). Attribute order is the map's.
    pub fn from_value(v: &Value) -> Option<XmlTree> {
        let o = v.as_object()?;
        let name = o
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let mut attrs = Vec::new();
        if let Some(Value::Object(a)) = o.get("attrs") {
            for (k, v) in a {
                attrs.push((k.clone(), v.clone()));
            }
        }
        let mut children = Vec::new();
        if let Some(Value::Array(cs)) = o.get("children") {
            for c in cs {
                match c {
                    Value::String(s) => children.push(XmlTreeChild::Text(s.clone())),
                    other => children.push(XmlTreeChild::Node(XmlTree::from_value(other)?)),
                }
            }
        }
        Some(XmlTree {
            name,
            attrs,
            children,
        })
    }

    /// The value of attribute `name` (JS `node.attrs[name]`), `None` when
    /// absent.
    pub fn attr(&self, name: &str) -> Option<&Value> {
        self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v)
    }
}

static EMPTY_NODE: XmlNode = XmlNode {
    name: String::new(),
    attrs: Vec::new(),
    children: Vec::new(),
};

/// The value `CSL.XmlJSON.getNodeValue` returns.
#[derive(Debug, Clone, PartialEq)]
pub enum NodeValue {
    /// `""` (nothing found, or the found element had no children).
    Empty,
    /// An object: the found node, or `myjson` itself.
    Node(NodeId),
    /// A string: the sole text child of the found node.
    Text(String),
}

/// `CSL.XmlJSON`: one parsed document and the methods that read and edit it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct XmlJson {
    nodes: Vec<XmlNode>,
    /// `dataObj`: the root element (`<style>` or `<locale>`). `None` is JS
    /// `undefined` (a document with no element).
    pub data_obj: Option<NodeId>,
}

fn child_nodes(n: &XmlNode) -> Vec<NodeId> {
    n.children
        .iter()
        .filter_map(|c| match c {
            XmlChild::Node(n) => Some(*n),
            XmlChild::Text(_) => None,
        })
        .collect()
}

impl XmlJson {
    /// `new CSL.XmlJSON(dataObj)` over an empty arena (no root).
    pub fn new() -> XmlJson {
        XmlJson::default()
    }

    /// Add a node to the arena and return its id.
    pub fn new_node(&mut self, node: XmlNode) -> NodeId {
        self.nodes.push(node);
        NodeId(self.nodes.len() - 1)
    }

    /// The node `id` (an empty node for an id from another document, which
    /// would be a port bug, not an input error).
    pub fn node(&self, id: NodeId) -> &XmlNode {
        self.nodes.get(id.0).unwrap_or(&EMPTY_NODE)
    }

    fn node_mut(&mut self, id: NodeId) -> Option<&mut XmlNode> {
        self.nodes.get_mut(id.0)
    }

    /// Number of nodes in the arena (diagnostics).
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the arena is empty.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    // ------------------------------------------------------------------
    // Export / import between documents.

    /// Detach node `id` and its subtree as an owned [`XmlTree`].
    pub fn export_tree(&self, id: NodeId) -> XmlTree {
        let n = self.node(id);
        XmlTree {
            name: n.name.clone(),
            attrs: n.attrs.clone(),
            children: n
                .children
                .iter()
                .map(|c| match c {
                    XmlChild::Text(s) => XmlTreeChild::Text(s.clone()),
                    XmlChild::Node(c) => XmlTreeChild::Node(self.export_tree(*c)),
                })
                .collect(),
        }
    }

    /// Attach an owned tree to this arena (verbatim, attribute types kept).
    pub fn import_tree(&mut self, tree: &XmlTree) -> NodeId {
        let children = tree
            .children
            .iter()
            .map(|c| match c {
                XmlTreeChild::Text(s) => XmlChild::Text(s.clone()),
                XmlTreeChild::Node(n) => XmlChild::Node(self.import_tree(n)),
            })
            .collect();
        self.new_node(XmlNode {
            name: tree.name.clone(),
            attrs: tree.attrs.clone(),
            children,
        })
    }

    /// `JSON`-input form of `CSL.setupXml`: build the document from a JSON
    /// `{name, attrs, children}` object.
    pub fn from_value(v: &Value) -> XmlJson {
        let mut x = XmlJson::new();
        if let Some(t) = XmlTree::from_value(v) {
            let id = x.import_tree(&t);
            x.data_obj = Some(id);
        }
        x
    }

    /// The node and its subtree as the exact text `JSON.stringify` gives the
    /// JS object: keys in the order `name`, `attrs` (insertion order),
    /// `children`. The intermediate test digests it.
    pub fn to_json_text(&self, id: NodeId) -> String {
        let n = self.node(id);
        let q = |s: &str| serde_json::to_string(s).unwrap_or_default();
        let mut out = format!("{{\"name\":{},\"attrs\":{{", q(&n.name));
        for (i, (k, v)) in n.attrs.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&format!(
                "{}:{}",
                q(k),
                serde_json::to_string(v).unwrap_or_default()
            ));
        }
        out.push_str("},\"children\":[");
        for (i, c) in n.children.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            match c {
                XmlChild::Text(t) => out.push_str(&q(t)),
                XmlChild::Node(c) => out.push_str(&self.to_json_text(*c)),
            }
        }
        out.push_str("]}");
        out
    }

    /// The node and its subtree as JSON (see [`XmlTree::to_value`]).
    pub fn to_value(&self, id: NodeId) -> Value {
        self.export_tree(id).to_value()
    }

    // ------------------------------------------------------------------
    // The CSL.XmlJSON prototype.

    /// `CSL.XmlJSON.prototype.clean`: a no-op.
    pub fn clean(&self, json: Option<NodeId>) -> Option<NodeId> {
        json
    }

    /// `getStyleId(myjson, styleName)`: the first text child of the *last*
    /// `<id>` (or, with `style_name`, `<title>`) inside `<info>`. `Some("")`
    /// when there is none, `None` when upstream returns `undefined` (the
    /// element has no children).
    pub fn get_style_id(&self, myjson: NodeId, style_name: bool) -> Option<Value> {
        let tag_name = if style_name { "title" } else { "id" };
        let mut ret = Some(Value::String(String::new()));
        for c in &self.node(myjson).children {
            let XmlChild::Node(info) = c else { continue };
            if self.node(*info).name != "info" {
                continue;
            }
            for g in &self.node(*info).children {
                let XmlChild::Node(g) = g else { continue };
                if self.node(*g).name == tag_name {
                    ret = match self.node(*g).children.first() {
                        None => None,
                        Some(XmlChild::Text(s)) => Some(Value::String(s.clone())),
                        Some(XmlChild::Node(n)) => Some(self.to_value(*n)),
                    };
                }
            }
        }
        ret
    }

    /// `children(myjson)`: a copy of the children list (`false`, an empty
    /// list here, when there are none).
    pub fn children(&self, myjson: NodeId) -> Vec<XmlChild> {
        self.node(myjson).children.clone()
    }

    /// `nodename(myjson)`.
    pub fn nodename(&self, myjson: NodeId) -> String {
        self.node(myjson).name.clone()
    }

    /// `attributes(myjson)`: `"@" + name` to value, in insertion order.
    pub fn attributes(&self, myjson: NodeId) -> Vec<(String, Value)> {
        self.node(myjson)
            .attrs
            .iter()
            .map(|(k, v)| (format!("@{k}"), v.clone()))
            .collect()
    }

    /// `content(myjson)`: the concatenated string children.
    pub fn content(&self, myjson: NodeId) -> String {
        let mut ret = String::new();
        for c in &self.node(myjson).children {
            if let XmlChild::Text(s) = c {
                ret.push_str(s);
            }
        }
        ret
    }

    /// `numberofnodes(list)`: a list's length (JS: `list.length`).
    pub fn numberofnodes<T>(&self, list: &[T]) -> usize {
        list.len()
    }

    /// `getAttributeValue(myjson, name, namespace)`: the attribute when
    /// truthy, else `""`. `namespace` is `""` when upstream passes none.
    pub fn get_attribute_value_ns(&self, myjson: NodeId, name: &str, namespace: &str) -> Value {
        let key = if namespace.is_empty() {
            name.to_string()
        } else {
            format!("{namespace}:{name}")
        };
        match self.node(myjson).attrs.iter().find(|(k, _)| *k == key) {
            Some((_, v)) if js::truthy(v) => v.clone(),
            _ => Value::String(String::new()),
        }
    }

    /// `getAttributeValue(myjson, name)` (no namespace).
    pub fn get_attribute_value(&self, myjson: NodeId, name: &str) -> Value {
        self.get_attribute_value_ns(myjson, name, "")
    }

    /// `getAttributeValue(...)` as the string `"" + value` (the common read).
    pub fn get_attribute_string(&self, myjson: NodeId, name: &str) -> String {
        js::to_js_string(&self.get_attribute_value(myjson, name))
    }

    /// `getNodeValue(myjson, name)` with a node: with `name`, the *last*
    /// child called `name` (`Empty` if it has no children); without, the node
    /// itself; then a node whose only child is a string collapses to that
    /// string.
    pub fn get_node_value(&self, myjson: NodeId, name: Option<&str>) -> NodeValue {
        let mut ret = NodeValue::Empty;
        match name.filter(|n| !n.is_empty()) {
            Some(name) => {
                for c in &self.node(myjson).children {
                    let XmlChild::Node(c) = c else { continue };
                    if self.node(*c).name == name {
                        ret = if self.node(*c).children.is_empty() {
                            NodeValue::Empty
                        } else {
                            NodeValue::Node(*c)
                        };
                    }
                }
            }
            None => ret = NodeValue::Node(myjson),
        }
        if let NodeValue::Node(n) = &ret {
            if let [XmlChild::Text(s)] = self.node(*n).children.as_slice() {
                return NodeValue::Text(s.clone());
            }
        }
        ret
    }

    /// `setAttributeOnNodeIdentifiedByNameAttribute(myjson, nodename,
    /// partname, attrname, val)`: on every direct child called `nodename`
    /// whose `name` attribute is `partname`, set `attrname` (a leading `@`
    /// is stripped) to `val`.
    pub fn set_attribute_on_node_identified_by_name_attribute(
        &mut self,
        myjson: NodeId,
        nodename: &str,
        partname: &str,
        attrname: &str,
        val: Value,
    ) {
        let attrname = attrname.strip_prefix('@').unwrap_or(attrname);
        for k in child_nodes(self.node(myjson)) {
            let hit = self.node(k).name == nodename
                && self
                    .node(k)
                    .attrs
                    .iter()
                    .any(|(a, v)| a == "name" && v.as_str() == Some(partname));
            if hit {
                self.set_attribute(k, attrname, val.clone());
            }
        }
    }

    /// `deleteNodeByNameAttribute(myjson, val)`: removes the children whose
    /// `name` attribute equals `val`, **while iterating**, so the element
    /// that slides into a removed slot is skipped (upstream's behaviour).
    pub fn delete_node_by_name_attribute(&mut self, myjson: NodeId, val: &str) {
        let ilen = self.node(myjson).children.len();
        let mut i = 0;
        while i < ilen {
            let child = self.node(myjson).children.get(i).cloned();
            if let Some(XmlChild::Node(c)) = child {
                let hit = self
                    .node(c)
                    .attrs
                    .iter()
                    .any(|(a, v)| a == "name" && js::to_js_string(v) == val);
                if hit {
                    if let Some(n) = self.node_mut(myjson) {
                        n.children.remove(i);
                    }
                }
            }
            i += 1;
        }
    }

    /// `deleteAttribute(myjson, attrname)`: upstream calls `attrs.pop`, which
    /// does not exist on an object, so it throws when the attribute is
    /// present. Nothing in citeproc-js calls it.
    pub fn delete_attribute(&mut self, myjson: NodeId, attrname: &str) -> CslResult<()> {
        if self.node(myjson).attrs.iter().any(|(k, _)| k == attrname) {
            return Err(EngineError::Csl(
                "TypeError: myjson.attrs.pop is not a function".to_string(),
            ));
        }
        Ok(())
    }

    /// `setAttribute(myjson, attr, val)`: set or replace in place.
    pub fn set_attribute(&mut self, myjson: NodeId, attr: &str, val: Value) {
        if let Some(n) = self.node_mut(myjson) {
            if let Some(slot) = n.attrs.iter_mut().find(|(k, _)| k == attr) {
                slot.1 = val;
            } else {
                n.attrs.push((attr.to_string(), val));
            }
        }
    }

    /// `nodeCopy(myjson)` on a node of this arena: a deep copy as a new
    /// node, which keeps **only string** attribute values (upstream copies
    /// strings and objects and drops numbers and booleans).
    pub fn node_copy(&mut self, myjson: NodeId) -> NodeId {
        let tree = self.export_tree(myjson);
        self.node_copy_tree(&tree)
    }

    /// `nodeCopy` of a node held as an owned tree (a locale's date
    /// template), imported into this arena; non-string attribute values are
    /// dropped as in [`XmlJson::node_copy`].
    pub fn node_copy_tree(&mut self, tree: &XmlTree) -> NodeId {
        let children = tree
            .children
            .iter()
            .map(|c| match c {
                XmlTreeChild::Text(s) => XmlChild::Text(s.clone()),
                XmlTreeChild::Node(n) => XmlChild::Node(self.node_copy_tree(n)),
            })
            .collect();
        let attrs = tree
            .attrs
            .iter()
            .filter(|(_, v)| v.is_string())
            .cloned()
            .collect();
        self.new_node(XmlNode {
            name: tree.name.clone(),
            attrs,
            children,
        })
    }

    /// `getNodesByName(myjson, name, nameattrval)`: the nodes called `name`
    /// (and, if `nameattrval` is non-empty, with that `name` attribute) in
    /// document order, `myjson` itself included. `None` for `myjson` is
    /// upstream's `undefined` argument: no nodes.
    pub fn get_nodes_by_name(
        &self,
        myjson: Option<NodeId>,
        name: &str,
        nameattrval: &str,
    ) -> Vec<NodeId> {
        let mut ret = Vec::new();
        if let Some(m) = myjson {
            self.collect_by_name(m, name, nameattrval, &mut ret);
        }
        ret
    }

    fn collect_by_name(&self, id: NodeId, name: &str, nameattrval: &str, ret: &mut Vec<NodeId>) {
        let n = self.node(id);
        if name == n.name {
            if !nameattrval.is_empty() {
                if n.attrs
                    .iter()
                    .any(|(k, v)| k == "name" && v.as_str() == Some(nameattrval))
                {
                    ret.push(id);
                }
            } else {
                ret.push(id);
            }
        }
        for c in &n.children {
            if let XmlChild::Node(c) = c {
                self.collect_by_name(*c, name, nameattrval, ret);
            }
        }
    }

    /// `nodeNameIs(myjson, name)`.
    pub fn node_name_is(&self, myjson: Option<NodeId>, name: &str) -> bool {
        match myjson {
            None => false,
            Some(m) => self.node(m).name == name,
        }
    }

    /// `makeXml(myjson)` called with no argument (the only way citeproc-js
    /// calls it, in `localeSet`): returns `undefined`.
    pub fn make_xml(&self) -> Option<NodeId> {
        None
    }

    /// `insertChildNodeAfter(parent, node, pos, datejson)` ("misnamed: this
    /// replaces the node"): replace the child `node` of `parent` by
    /// `datejson`. Returns `parent`.
    pub fn insert_child_node_after(
        &mut self,
        parent: NodeId,
        node: NodeId,
        _pos: usize,
        datejson: NodeId,
    ) -> NodeId {
        if let Some(p) = self.node_mut(parent) {
            for c in p.children.iter_mut() {
                if *c == XmlChild::Node(node) {
                    *c = XmlChild::Node(datejson);
                    break;
                }
            }
        }
        parent
    }

    /// `insertPublisherAndPlace(myjson)`: flag a `<group>` that holds exactly
    /// the two bare `<text variable="publisher">` and
    /// `<text variable="publisher-place">` with
    /// `has-publisher-and-publisher-place` (boolean `true`), recursively.
    /// A string child of a group throws in upstream (`attrs` of a string).
    pub fn insert_publisher_and_place(&mut self, myjson: NodeId) -> CslResult<()> {
        if self.node(myjson).name == "group" {
            let mut useme = true;
            let mut must_haves: Vec<&str> = vec!["publisher", "publisher-place"];
            for c in &self.node(myjson).children {
                let XmlChild::Node(c) = c else {
                    return Err(EngineError::Csl(
                        "TypeError: Cannot read properties of undefined (reading 'variable')"
                            .to_string(),
                    ));
                };
                let n = self.node(*c);
                let variable = n
                    .attrs
                    .iter()
                    .find(|(k, _)| k == "variable")
                    .map(|(_, v)| v);
                let have = variable
                    .and_then(Value::as_str)
                    .and_then(|v| must_haves.iter().position(|m| *m == v));
                let is_text = n.name == "text";
                let no_prefix = !n.attrs.iter().any(|(k, v)| k == "prefix" && js::truthy(v));
                let no_suffix = !n.attrs.iter().any(|(k, v)| k == "suffix" && js::truthy(v));
                match have {
                    Some(h) if is_text && no_prefix && no_suffix => {
                        must_haves.remove(h);
                    }
                    _ => {
                        useme = false;
                        break;
                    }
                }
            }
            if useme && must_haves.is_empty() {
                self.set_attribute(
                    myjson,
                    "has-publisher-and-publisher-place",
                    Value::Bool(true),
                );
            }
        }
        for k in child_nodes(self.node(myjson)) {
            self.insert_publisher_and_place(k)?;
        }
        Ok(())
    }

    /// `isChildOfSubstitute(parents)`: whether any ancestor is `substitute`.
    pub fn is_child_of_substitute(&self, parents: &[String]) -> bool {
        parents.iter().any(|p| p == "substitute")
    }

    /// `addMissingNameNodes(myjson, parents)`: give every `<names>` that is
    /// not inside a `<substitute>` and has no `<name>` child an empty
    /// `<name/>` first child.
    pub fn add_missing_name_nodes(&mut self, myjson: NodeId, parents: &mut Vec<String>) {
        if self.node(myjson).name == "names" && !self.is_child_of_substitute(parents) {
            let add_name = !self
                .node(myjson)
                .children
                .iter()
                .any(|c| matches!(c, XmlChild::Node(n) if self.node(*n).name == "name"));
            if add_name {
                let name = self.new_node(XmlNode {
                    name: "name".to_string(),
                    attrs: Vec::new(),
                    children: Vec::new(),
                });
                if let Some(n) = self.node_mut(myjson) {
                    n.children.insert(0, XmlChild::Node(name));
                }
            }
        }
        parents.push(self.node(myjson).name.clone());
        for k in child_nodes(self.node(myjson)) {
            self.add_missing_name_nodes(k, parents);
        }
        parents.pop();
    }

    /// `addInstitutionNodes(myjson)`: after the `<name>` of every `<names>`
    /// that has no `<institution>`, insert the default
    /// `<institution institution-parts="long" delimiter=", ">` with a
    /// `<institution-part name="long"/>`, carrying over the name's
    /// `delimiter`/`and` and the `font-*`/`text-*` attributes of the name and
    /// of its family `name-part`. A string child of a `<name>` throws in
    /// upstream (`attrs` of a string).
    pub fn add_institution_nodes(&mut self, myjson: NodeId) -> CslResult<()> {
        if self.node(myjson).name == "names" {
            let mut attributes: Vec<(String, Value)> = Vec::new();
            fn set(attributes: &mut Vec<(String, Value)>, k: &str, v: Value) {
                if let Some(s) = attributes.iter_mut().find(|(a, _)| a == k) {
                    s.1 = v;
                } else {
                    attributes.push((k.to_string(), v));
                }
            }
            let mut insert_pos: i64 = -1;
            let kids = self.node(myjson).children.clone();
            for (i, c) in kids.iter().enumerate() {
                let XmlChild::Node(c) = c else { continue };
                let cn = self.node(*c).clone();
                if cn.name == "name" {
                    for (k, v) in &cn.attrs {
                        set(&mut attributes, k, v.clone());
                    }
                    // attributes.delimiter = attrs.delimiter (undefined resets)
                    for key in ["delimiter", "and"] {
                        match cn.attrs.iter().find(|(k, _)| k == key) {
                            Some((_, v)) => set(&mut attributes, key, v.clone()),
                            None => attributes.retain(|(a, _)| a != key),
                        }
                    }
                    insert_pos = i as i64;
                    for k in &cn.children {
                        let XmlChild::Node(k) = k else {
                            return Err(EngineError::Csl(
                                "TypeError: Cannot read properties of undefined (reading 'name')"
                                    .to_string(),
                            ));
                        };
                        let kn = self.node(*k);
                        let is_family = kn
                            .attrs
                            .iter()
                            .any(|(a, v)| a == "name" && v.as_str() == Some("family"));
                        if !is_family {
                            continue;
                        }
                        for (a, v) in &kn.attrs {
                            set(&mut attributes, a, v.clone());
                        }
                    }
                }
                if cn.name == "institution" {
                    insert_pos = -1;
                    break;
                }
            }
            if insert_pos > -1 {
                let part = self.new_node(XmlNode {
                    name: "institution-part".to_string(),
                    attrs: vec![("name".to_string(), Value::String("long".to_string()))],
                    children: Vec::new(),
                });
                let institution = self.new_node(XmlNode {
                    name: "institution".to_string(),
                    attrs: vec![
                        (
                            "institution-parts".to_string(),
                            Value::String("long".to_string()),
                        ),
                        ("delimiter".to_string(), Value::String(", ".to_string())),
                    ],
                    children: vec![XmlChild::Node(part)],
                });
                let get = |k: &str| {
                    attributes
                        .iter()
                        .find(|(a, _)| a == k)
                        .map(|(_, v)| v.clone())
                };
                for attrname in load::INSTITUTION_KEYS {
                    if let Some(v) = get(attrname) {
                        self.set_attribute(part, attrname, v);
                    }
                    if let Some(d) = get("delimiter").filter(js::truthy) {
                        self.set_attribute(institution, "delimiter", d);
                    }
                    if let Some(a) = get("and").filter(js::truthy) {
                        self.set_attribute(institution, "and", a);
                    }
                }
                if let Some(n) = self.node_mut(myjson) {
                    let at = (insert_pos as usize + 1).min(n.children.len());
                    n.children.insert(at, XmlChild::Node(institution));
                }
            }
        }
        for c in self.node(myjson).children.clone() {
            if let XmlChild::Node(c) = c {
                self.add_institution_nodes(c)?;
            }
        }
        Ok(())
    }

    /// `flagDateMacros(myjson)`: mark every direct-child `<macro>` that
    /// contains a `<date>` with `macro-has-date="true"`.
    pub fn flag_date_macros(&mut self, myjson: NodeId) {
        for k in child_nodes(self.node(myjson)) {
            if self.node(k).name == "macro" && self.inspect_date_macros(k) {
                self.set_attribute(k, "macro-has-date", Value::String("true".to_string()));
            }
        }
    }

    /// `inspectDateMacros(myjson)`: whether the node is, or contains, a `<date>`.
    pub fn inspect_date_macros(&self, myjson: NodeId) -> bool {
        if self.node(myjson).name == "date" {
            return true;
        }
        self.node(myjson).children.iter().any(|c| match c {
            XmlChild::Node(n) => self.inspect_date_macros(*n),
            XmlChild::Text(_) => false,
        })
    }
}

/// `CSL.stripXmlProcessingInstruction(xml)`: drop a leading `<?...?>`, every
/// `<!--...-->` comment, and surrounding whitespace.
pub fn strip_xml_processing_instruction(xml: &str) -> String {
    static PI: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^<\?[^?]+\?>").expect("static regex"));
    static COMMENT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"<!--[^>]+-->").expect("static regex"));
    if xml.is_empty() {
        return String::new();
    }
    let s = PI.replace(xml, "");
    let s = COMMENT.replace_all(&s, "");
    js::trim(&s).to_string()
}

// ----------------------------------------------------------------------
// CSL.parseXml

/// JS `.` (anything but `\n`, `\r`, ` `, ` `) as a regex class.
const DOT: &str = r"[^\n\r\x{2028}\x{2029}]";

/// `str.slice(1)` on a string (skips one code point; a non-BMP first
/// character would leave a lone surrogate in JS, which no caller can see).
fn skip1(s: &str) -> &str {
    let mut it = s.chars();
    it.next();
    it.as_str()
}

/// `_decodeHtmlEntities(str)`.
fn decode_html_entities(s: &str) -> String {
    static DEC: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)&#([0-9]{1,6});").expect("static regex"));
    static HEX: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)&#x([a-f0-9]{1,6});").expect("static regex"));
    let s = s
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&gt;", ">")
        .replace("&lt;", "<");
    let from_code = |num: u32| -> String {
        // String.fromCharCode truncates to a UTF-16 code unit; a lone
        // surrogate cannot live in a Rust string and becomes U+FFFD.
        let unit = (num & 0xffff) as u16;
        String::from_utf16_lossy(&[unit])
    };
    let s = DEC.replace_all(&s, |c: &regex::Captures| {
        from_code(c[1].parse::<u32>().unwrap_or(0))
    });
    let s = HEX.replace_all(&s, |c: &regex::Captures| {
        from_code(u32::from_str_radix(&c[1], 16).unwrap_or(0))
    });
    s.into_owned()
}

/// `_listifyString(str)`.
fn listify_string(input: &str) -> Vec<String> {
    static NEWLINES: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\r\n|\n|\r").expect("static regex"));
    static GAP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r">[\t ]+<").expect("static regex"));
    static COMMENT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(&format!(r"<!--{DOT}*?-->")).expect("static regex"));
    let s = NEWLINES.replace_all(input, " ");
    let s = GAP.replace_all(&s, "><");
    let s = COMMENT.replace_all(&s, "");
    let mut lst: Vec<String> = s.split("><").map(str::to_string).collect();
    let n = lst.len();
    let mut style_pos: Option<usize> = None;
    for i in 0..n {
        if i > 0 {
            lst[i] = format!("<{}", lst[i]);
        }
        if i < n - 1 {
            lst[i].push('>');
        }
        if style_pos.is_none()
            && (js::slice(&lst[i], 0, Some(7)) == "<style "
                || js::slice(&lst[i], 0, Some(8)) == "<locale ")
        {
            style_pos = Some(i);
        }
    }
    // lst.slice(stylePos): a non-number stylePos (null) slices from 0.
    if let Some(p) = style_pos {
        lst.drain(..p);
    }
    // Combine open/close elements for empty terms, so that they will be
    // passed through correctly as empty strings.
    let mut i = lst.len() as i64 - 2;
    while i > -1 {
        let iu = i as usize;
        if !skip1(&lst[iu]).contains('<') {
            let stub = js::slice(&lst[iu], 0, Some(5));
            if js::slice(&lst[iu], -2, None) != "/>" {
                if stub == "<term" {
                    if js::slice(&lst[iu + 1], 0, Some(6)) == "</term" {
                        let next = lst.remove(iu + 1);
                        lst[iu].push_str(&next);
                    }
                } else if (stub == "<sing" || stub == "<mult")
                    && js::slice(&lst[iu + 1], 0, Some(1)) == "<"
                {
                    let next = lst.remove(iu + 1);
                    lst[iu].push_str(&next);
                }
            }
        }
        i -= 1;
    }
    lst
}

/// `_getAttributes(elem)`: the attribute names found by the global match.
fn get_attributes(elem: &str) -> Vec<String> {
    static ATTR: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"([^'"=\t ]+)=(?:"[^"]*"|'[^']*')"#).expect("static regex"));
    ATTR.find_iter(elem)
        .map(|m| {
            let t = m.as_str();
            // m[i].replace(/=.*/, "")
            match t.find('=') {
                Some(p) => t[..p].to_string(),
                None => t.to_string(),
            }
        })
        .collect()
}

/// `_getAttribute(elem, attr)`: the value of the **last** ` attr="..."`.
fn get_attribute(elem: &str, attr: &str) -> Option<String> {
    let rex = format!(
        r#"^{DOT}*[\t ]+{}=("(?:[^"]*)"|'(?:[^']*)'){DOT}*$"#,
        regex::escape(attr)
    );
    let re = Regex::new(&rex).ok()?;
    let caps = re.captures(elem)?;
    let m = caps.get(1)?.as_str();
    Some(js::slice(m, 1, Some(-1)))
}

/// `_getTagName(elem)`.
fn get_tag_name(elem: &str) -> Option<String> {
    static TAG: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^<([^\t />]+)").expect("static regex"));
    TAG.captures(elem).map(|c| c[1].to_string())
}

fn type_error(what: &str) -> EngineError {
    EngineError::Csl(format!("TypeError: {what}"))
}

/// `CSL.parseXml(str)`: scrape a style or locale into an [`XmlJson`]; the
/// document's root (`_obj.children[0]`) is its `data_obj`.
pub fn parse_xml(input: &str) -> CslResult<XmlJson> {
    static COMPOSITE_TEXT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(&format!(r"^{DOT}*>([^<]*)<{DOT}*$")).expect("static regex"));
    let mut xml = XmlJson::new();
    // The virtual root's children, and a stack of open elements (None = the root).
    let mut top: Vec<NodeId> = Vec::new();
    let mut stack: Vec<Option<NodeId>> = vec![None];

    fn cast_from_opening_tag(xml: &mut XmlJson, elem: &str) -> CslResult<NodeId> {
        let name = get_tag_name(elem).unwrap_or_default();
        let mut attrs: Vec<(String, Value)> = Vec::new();
        for a in get_attributes(elem) {
            let value = get_attribute(elem, &a)
                .ok_or_else(|| type_error("Cannot read properties of null (reading 'split')"))?;
            let v = Value::String(decode_html_entities(&value));
            if let Some(slot) = attrs.iter_mut().find(|(k, _)| *k == a) {
                slot.1 = v;
            } else {
                attrs.push((a, v));
            }
        }
        Ok(xml.new_node(XmlNode {
            name,
            attrs,
            children: Vec::new(),
        }))
    }

    fn append(
        xml: &mut XmlJson,
        top: &mut Vec<NodeId>,
        stack: &[Option<NodeId>],
        obj: NodeId,
    ) -> CslResult<()> {
        match stack.last() {
            Some(None) => {
                top.push(obj);
                Ok(())
            }
            Some(Some(parent)) => {
                if let Some(p) = xml.node_mut(*parent) {
                    p.children.push(XmlChild::Node(obj));
                }
                Ok(())
            }
            None => Err(type_error(
                "Cannot read properties of undefined (reading 'push')",
            )),
        }
    }

    for elem in listify_string(input) {
        if skip1(&elem).contains('<') {
            // with text
            let end = js::index_of(&elem, ">", 0);
            let tag = js::slice(&elem, 0, Some(end + 1));
            let obj = cast_from_opening_tag(&mut xml, &tag)?;
            let text = match COMPOSITE_TEXT.captures(&elem) {
                Some(c) => decode_html_entities(&c[1]),
                None => return Err(type_error("Cannot read properties of null (reading '1')")),
            };
            if let Some(n) = xml.node_mut(obj) {
                n.children = vec![XmlChild::Text(text)];
            }
            append(&mut xml, &mut top, &stack, obj)?;
        } else if js::slice(&elem, -2, None) == "/>" {
            // singleton
            let obj = cast_from_opening_tag(&mut xml, &elem)?;
            if get_tag_name(&elem).as_deref() == Some("term") {
                if let Some(n) = xml.node_mut(obj) {
                    n.children.push(XmlChild::Text(String::new()));
                }
            }
            append(&mut xml, &mut top, &stack, obj)?;
        } else if js::slice(&elem, 0, Some(2)) == "</" {
            // close
            stack.pop();
        } else {
            // open
            let obj = cast_from_opening_tag(&mut xml, &elem)?;
            append(&mut xml, &mut top, &stack, obj)?;
            stack.push(Some(obj));
        }
    }
    xml.data_obj = top.first().copied();
    Ok(xml)
}

#[cfg(test)]
mod tests {
    use super::*;

    const STYLE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<style xmlns="http://purl.org/net/xbiblio/csl" class="note" version="1.0">
  <info><id>http://x/y</id><title>T &amp; U</title></info>
  <locale xml:lang="en"><terms><term name="a"/><term name="b"></term><term name="c"><single>s</single><multiple>m</multiple></term></terms></locale>
  <macro name="m"><names variable="author"><substitute><names variable="editor"/></substitute></names><date variable="issued"/></macro>
  <citation><layout><group><text variable="publisher"/><text variable="publisher-place"/></group></layout></citation>
</style>"#;

    #[test]
    fn parses_a_style_and_finds_nodes() {
        let x = parse_xml(STYLE).unwrap();
        let root = x.data_obj.unwrap();
        assert_eq!(x.nodename(root), "style");
        assert_eq!(x.get_attribute_string(root, "class"), "note");
        assert_eq!(
            x.get_style_id(root, false),
            Some(Value::String("http://x/y".into()))
        );
        assert_eq!(
            x.get_style_id(root, true),
            Some(Value::String("T & U".into()))
        );
        let terms = x.get_nodes_by_name(Some(root), "term", "");
        assert_eq!(terms.len(), 3);
        // Empty terms (singleton and open/close pair) carry an empty string.
        assert_eq!(
            x.node(terms[0]).children,
            vec![XmlChild::Text(String::new())]
        );
        assert_eq!(
            x.node(terms[1]).children,
            vec![XmlChild::Text(String::new())]
        );
        assert_eq!(
            x.get_node_value(terms[2], Some("single")),
            NodeValue::Text("s".into())
        );
    }

    #[test]
    fn preprocessing_passes() {
        let mut x = parse_xml(STYLE).unwrap();
        let root = x.data_obj.unwrap();
        x.add_missing_name_nodes(root, &mut Vec::new());
        x.add_institution_nodes(root).unwrap();
        x.insert_publisher_and_place(root).unwrap();
        x.flag_date_macros(root);
        let names = x.get_nodes_by_name(Some(root), "names", "");
        // The outer names gets <name/>, then <institution/> after it.
        let outer = x.node(names[0]);
        assert_eq!(outer.children.len(), 3, "name, institution, substitute");
        let group = x.get_nodes_by_name(Some(root), "group", "")[0];
        assert_eq!(
            x.get_attribute_value(group, "has-publisher-and-publisher-place"),
            Value::Bool(true)
        );
        let m = x.get_nodes_by_name(Some(root), "macro", "m")[0];
        assert_eq!(x.get_attribute_string(m, "macro-has-date"), "true");
    }

    #[test]
    fn delete_node_by_name_attribute_skips_like_upstream() {
        let mut x = XmlJson::new();
        let mk = |x: &mut XmlJson, n: &str| {
            x.new_node(XmlNode {
                name: "date-part".into(),
                attrs: vec![("name".into(), Value::String(n.into()))],
                children: vec![],
            })
        };
        let a = mk(&mut x, "month");
        let b = mk(&mut x, "month");
        let c = mk(&mut x, "day");
        let p = x.new_node(XmlNode {
            name: "date".into(),
            attrs: vec![],
            children: vec![XmlChild::Node(a), XmlChild::Node(b), XmlChild::Node(c)],
        });
        x.delete_node_by_name_attribute(p, "month");
        // The second "month" slid into slot 0 and was skipped.
        assert_eq!(x.node(p).children.len(), 2);
    }

    #[test]
    fn node_copy_drops_non_string_attributes() {
        let mut x = XmlJson::new();
        let n = x.new_node(XmlNode {
            name: "a".into(),
            attrs: vec![
                ("s".into(), Value::String("v".into())),
                ("n".into(), Value::from(3)),
                ("b".into(), Value::Bool(true)),
            ],
            children: vec![XmlChild::Text("t".into())],
        });
        let c = x.node_copy(n);
        assert_eq!(x.node(c).attrs.len(), 1);
        assert_eq!(x.node(c).children, vec![XmlChild::Text("t".into())]);
    }

    #[test]
    fn entity_decoding_runs_amp_first() {
        assert_eq!(decode_html_entities("&amp;lt;"), "<");
        assert_eq!(decode_html_entities("&#65;&#x42;"), "AB");
    }

    #[test]
    fn attribute_lookup_takes_the_last_occurrence() {
        assert_eq!(
            get_attribute(r#"<text a="1" prefix="x name='y'" name="z">"#, "name").as_deref(),
            Some("z")
        );
        assert_eq!(get_attributes(r#"<a b="1" c='2'/>"#), vec!["b", "c"]);
    }
}
