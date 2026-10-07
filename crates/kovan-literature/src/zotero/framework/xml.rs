// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: the DOM the Zotero translation-server gives XML translators.
//   translation-server (https://github.com/zotero/translation-server,
//   commit 3a9d17614896) src/translation/translate.js :86-95 installs
//   jsdom's DOMParser and an XMLSerializer over w3c-xmlserializer; Zotero
//   translate (commit dd524aea9a55) src/translation/translate.js
//   `Zotero.Translate.IO.parseDOMXML` :2858-2885 and `getXML` :2989-2997.
//   What a translator observes is defined by the WHATWG DOM Standard
//   (https://dom.spec.whatwg.org/: tree order, textContent, getAttribute,
//   setAttribute(NS), createElement(NS), "locate a namespace",
//   getElementsByTagName(NS), normalize) as jsdom 29.0.1 implements it.
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA
//   (translate, translation-server). The DOM algorithms follow the WHATWG
//   DOM Standard (CC BY 4.0).
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! An XML DOM for the XML translators (#749): what `Zotero.getXML()`,
//! `new DOMParser().parseFromString(s, "text/xml")` and the exporters'
//! `createElementNS`/`appendChild`/`XMLSerializer` give a translator in the
//! translation-server.
//!
//! The document is an arena: nodes are [`NodeId`]s into one
//! [`XmlDocument`], and an attribute is addressed as [`XNode::Attr`] (its
//! element and its index), which is what XPath returns for `@name`.
//!
//! | Module | What |
//! |---|---|
//! | this one | the tree, node accessors (`textContent`, `getAttribute`, `children`, `lookupNamespaceURI`, ...), mutation, `getElementsByTagName(NS)`, `querySelectorAll` (the selector subset the translators use) |
//! | [`super::xml_parse`] | parsing: saxes 6.0.0 as jsdom drives it |
//! | [`super::xml_serialize`] | `XMLSerializer` / `innerHTML`: w3c-xmlserializer 5.0.0 |
//! | [`super::xpath`] | `ZU.xpath` / `ZU.xpathText` over wicked-good-xpath 1.3.1-z002 |

use super::context::{ImportContext, TranslateError};

/// The XML namespace (`xml:` prefix).
pub const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";
/// The XMLNS namespace (`xmlns` attributes).
pub const XMLNS_NS: &str = "http://www.w3.org/2000/xmlns/";
/// The HTML namespace.
pub const HTML_NS: &str = "http://www.w3.org/1999/xhtml";

/// A node of an [`XmlDocument`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub usize);

/// A node or an attribute: what an XPath step can select.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XNode {
    /// A tree node.
    Node(NodeId),
    /// The `index`-th attribute of an element.
    Attr(NodeId, usize),
}

impl From<NodeId> for XNode {
    fn from(n: NodeId) -> Self {
        XNode::Node(n)
    }
}

impl XNode {
    /// The tree node (for an attribute, its element).
    pub fn node(self) -> NodeId {
        match self {
            XNode::Node(n) | XNode::Attr(n, _) => n,
        }
    }

    /// The tree node, `None` for an attribute.
    pub fn as_node(self) -> Option<NodeId> {
        match self {
            XNode::Node(n) => Some(n),
            XNode::Attr(..) => None,
        }
    }
}

/// DOM `nodeType` values.
pub mod node_type {
    /// `ELEMENT_NODE`.
    pub const ELEMENT: u8 = 1;
    /// `ATTRIBUTE_NODE`.
    pub const ATTRIBUTE: u8 = 2;
    /// `TEXT_NODE`.
    pub const TEXT: u8 = 3;
    /// `CDATA_SECTION_NODE`.
    pub const CDATA: u8 = 4;
    /// `PROCESSING_INSTRUCTION_NODE`.
    pub const PI: u8 = 7;
    /// `COMMENT_NODE`.
    pub const COMMENT: u8 = 8;
    /// `DOCUMENT_NODE`.
    pub const DOCUMENT: u8 = 9;
    /// `DOCUMENT_TYPE_NODE`.
    pub const DOCTYPE: u8 = 10;
}

/// An attribute: namespace, prefix, local name, value (DOM `Attr`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlAttr {
    /// `namespaceURI`.
    pub namespace: Option<String>,
    /// `prefix`.
    pub prefix: Option<String>,
    /// `localName`.
    pub local: String,
    /// `value`.
    pub value: String,
}

impl XmlAttr {
    /// `name`: the qualified name.
    pub fn qualified_name(&self) -> String {
        match &self.prefix {
            Some(p) => format!("{p}:{}", self.local),
            None => self.local.clone(),
        }
    }
}

/// An element's name and attributes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementData {
    /// `namespaceURI`.
    pub namespace: Option<String>,
    /// `prefix`.
    pub prefix: Option<String>,
    /// `localName`.
    pub local: String,
    /// The attributes in order (namespace declarations included, as in the
    /// DOM).
    pub attrs: Vec<XmlAttr>,
}

impl ElementData {
    /// `tagName` (= qualified name in an XML document).
    pub fn qualified_name(&self) -> String {
        match &self.prefix {
            Some(p) => format!("{p}:{}", self.local),
            None => self.local.clone(),
        }
    }
}

/// What a node is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    /// The document.
    Document,
    /// `<!DOCTYPE>`.
    DocumentType {
        /// `name`.
        name: String,
        /// `publicId`.
        public_id: String,
        /// `systemId`.
        system_id: String,
    },
    /// An element.
    Element(ElementData),
    /// Text.
    Text(String),
    /// A CDATA section.
    CData(String),
    /// A comment.
    Comment(String),
    /// A processing instruction.
    Pi {
        /// `target`.
        target: String,
        /// `data`.
        data: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NodeData {
    kind: NodeKind,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
}

/// An XML document (DOM `XMLDocument`); node 0 is the document node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlDocument {
    nodes: Vec<NodeData>,
}

impl Default for XmlDocument {
    fn default() -> Self {
        XmlDocument::new()
    }
}

/// Why `parseFromString` gave a `<parsererror>` document (saxes' message).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlParseError(pub String);

impl std::fmt::Display for XmlParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl XmlDocument {
    /// An empty document (no document element).
    pub fn new() -> Self {
        XmlDocument {
            nodes: vec![NodeData {
                kind: NodeKind::Document,
                parent: None,
                children: Vec::new(),
            }],
        }
    }

    /// `Zotero.Translate.IO.parseDOMXML(s)` (translate.js:2858-2885):
    /// `new DOMParser().parseFromString(s, "text/xml")`, an error when the
    /// result has a `parsererror` element (which a malformed document gets),
    /// then `normalize()`.
    pub fn parse(s: &str) -> Result<XmlDocument, XmlParseError> {
        let doc = super::xml_parse::parse_document(s)?;
        if !doc
            .get_elements_by_tag_name(doc.document(), "parsererror")
            .is_empty()
        {
            return Err(XmlParseError(
                "DOMParser error: loading data into data store failed".to_owned(),
            ));
        }
        let mut doc = doc;
        doc.normalize(doc.document());
        Ok(doc)
    }

    /// `new DOMParser().parseFromString(s, type)` as an exporter calls it
    /// (no `parsererror` check, no `normalize`): the document, or the
    /// `<parsererror>` document jsdom builds from saxes' message.
    pub fn parse_from_string(s: &str) -> XmlDocument {
        match super::xml_parse::parse_document(s) {
            Ok(d) => d,
            Err(e) => {
                let mut d = XmlDocument::new();
                let el = d.create_element_ns(
                    Some("http://www.mozilla.org/newlayout/xml/parsererror.xml"),
                    "parsererror",
                );
                let t = d.create_text_node(&e.0);
                d.append_child(el, t);
                let root = d.document();
                d.append_child(root, el);
                d
            }
        }
    }

    pub(super) fn push(&mut self, kind: NodeKind) -> NodeId {
        self.nodes.push(NodeData {
            kind,
            parent: None,
            children: Vec::new(),
        });
        NodeId(self.nodes.len() - 1)
    }

    /// The document node.
    pub fn document(&self) -> NodeId {
        NodeId(0)
    }

    /// `documentElement`.
    pub fn document_element(&self) -> Option<NodeId> {
        self.nodes[0]
            .children
            .iter()
            .copied()
            .find(|&c| self.is_element(c))
    }

    /// The node's kind.
    pub fn kind(&self, n: NodeId) -> &NodeKind {
        &self.nodes[n.0].kind
    }

    /// The element data, `None` for another node.
    pub fn element(&self, n: NodeId) -> Option<&ElementData> {
        match &self.nodes[n.0].kind {
            NodeKind::Element(e) => Some(e),
            _ => None,
        }
    }

    fn element_mut(&mut self, n: NodeId) -> Option<&mut ElementData> {
        match &mut self.nodes[n.0].kind {
            NodeKind::Element(e) => Some(e),
            _ => None,
        }
    }

    /// Whether the node is an element.
    pub fn is_element(&self, n: NodeId) -> bool {
        matches!(self.nodes[n.0].kind, NodeKind::Element(_))
    }

    /// `nodeType`.
    pub fn node_type(&self, x: XNode) -> u8 {
        match x {
            XNode::Attr(..) => node_type::ATTRIBUTE,
            XNode::Node(n) => match &self.nodes[n.0].kind {
                NodeKind::Document => node_type::DOCUMENT,
                NodeKind::DocumentType { .. } => node_type::DOCTYPE,
                NodeKind::Element(_) => node_type::ELEMENT,
                NodeKind::Text(_) => node_type::TEXT,
                NodeKind::CData(_) => node_type::CDATA,
                NodeKind::Comment(_) => node_type::COMMENT,
                NodeKind::Pi { .. } => node_type::PI,
            },
        }
    }

    /// `parentNode` (an attribute has none).
    pub fn parent(&self, n: NodeId) -> Option<NodeId> {
        self.nodes[n.0].parent
    }

    /// `childNodes`.
    pub fn children(&self, n: NodeId) -> &[NodeId] {
        &self.nodes[n.0].children
    }

    /// `children`: the element children.
    pub fn element_children(&self, n: NodeId) -> Vec<NodeId> {
        self.nodes[n.0]
            .children
            .iter()
            .copied()
            .filter(|&c| self.is_element(c))
            .collect()
    }

    /// `hasChildNodes()`.
    pub fn has_child_nodes(&self, n: NodeId) -> bool {
        !self.nodes[n.0].children.is_empty()
    }

    /// `firstChild`.
    pub fn first_child(&self, n: NodeId) -> Option<NodeId> {
        self.nodes[n.0].children.first().copied()
    }

    fn sibling_index(&self, n: NodeId) -> Option<(NodeId, usize)> {
        let p = self.nodes[n.0].parent?;
        let i = self.nodes[p.0].children.iter().position(|&c| c == n)?;
        Some((p, i))
    }

    /// `nextSibling`.
    pub fn next_sibling(&self, n: NodeId) -> Option<NodeId> {
        let (p, i) = self.sibling_index(n)?;
        self.nodes[p.0].children.get(i + 1).copied()
    }

    /// `previousSibling`.
    pub fn previous_sibling(&self, n: NodeId) -> Option<NodeId> {
        let (p, i) = self.sibling_index(n)?;
        if i == 0 {
            None
        } else {
            Some(self.nodes[p.0].children[i - 1])
        }
    }

    /// The attribute an [`XNode::Attr`] addresses.
    pub fn attr(&self, x: XNode) -> Option<&XmlAttr> {
        match x {
            XNode::Attr(el, i) => self.element(el).and_then(|e| e.attrs.get(i)),
            XNode::Node(_) => None,
        }
    }

    /// `nodeName`: the qualified name of an element or attribute, `#text`,
    /// `#cdata-section`, `#comment`, `#document`, a PI's target, a doctype's
    /// name.
    pub fn node_name(&self, x: XNode) -> String {
        if let Some(a) = self.attr(x) {
            return a.qualified_name();
        }
        match &self.nodes[x.node().0].kind {
            NodeKind::Document => "#document".to_owned(),
            NodeKind::DocumentType { name, .. } => name.clone(),
            NodeKind::Element(e) => e.qualified_name(),
            NodeKind::Text(_) => "#text".to_owned(),
            NodeKind::CData(_) => "#cdata-section".to_owned(),
            NodeKind::Comment(_) => "#comment".to_owned(),
            NodeKind::Pi { target, .. } => target.clone(),
        }
    }

    /// `tagName` of an element (its qualified name), "" for another node.
    pub fn tag_name(&self, n: NodeId) -> String {
        self.element(n)
            .map(ElementData::qualified_name)
            .unwrap_or_default()
    }

    /// `localName` (elements and attributes; `None` for other nodes).
    pub fn local_name(&self, x: XNode) -> Option<&str> {
        if let Some(a) = self.attr(x) {
            return Some(&a.local);
        }
        self.element(x.node()).map(|e| e.local.as_str())
    }

    /// `namespaceURI` (elements and attributes).
    pub fn namespace_uri(&self, x: XNode) -> Option<&str> {
        if let Some(a) = self.attr(x) {
            return a.namespace.as_deref();
        }
        self.element(x.node()).and_then(|e| e.namespace.as_deref())
    }

    /// `prefix` (elements and attributes).
    pub fn prefix(&self, x: XNode) -> Option<&str> {
        if let Some(a) = self.attr(x) {
            return a.prefix.as_deref();
        }
        self.element(x.node()).and_then(|e| e.prefix.as_deref())
    }

    /// `nodeValue`: an attribute's value, the data of text, CDATA, comments
    /// and PIs; `None` (null) otherwise.
    pub fn node_value(&self, x: XNode) -> Option<String> {
        if let Some(a) = self.attr(x) {
            return Some(a.value.clone());
        }
        match &self.nodes[x.node().0].kind {
            NodeKind::Text(s) | NodeKind::CData(s) | NodeKind::Comment(s) => Some(s.clone()),
            NodeKind::Pi { data, .. } => Some(data.clone()),
            _ => None,
        }
    }

    /// `textContent`: for an element, its descendant text (Text and CDATA
    /// nodes) in tree order; for an attribute its value; for text, CDATA,
    /// comments and PIs their data; `None` (null) for the document and a
    /// doctype.
    pub fn text_content(&self, x: XNode) -> Option<String> {
        if let Some(a) = self.attr(x) {
            return Some(a.value.clone());
        }
        let n = x.node();
        match &self.nodes[n.0].kind {
            NodeKind::Document | NodeKind::DocumentType { .. } => None,
            NodeKind::Element(_) => {
                let mut out = String::new();
                self.collect_text(n, &mut out);
                Some(out)
            }
            _ => self.node_value(x),
        }
    }

    /// `textContent` as a string ("" where it is null).
    pub fn text(&self, x: impl Into<XNode>) -> String {
        self.text_content(x.into()).unwrap_or_default()
    }

    fn collect_text(&self, n: NodeId, out: &mut String) {
        for &c in &self.nodes[n.0].children {
            match &self.nodes[c.0].kind {
                NodeKind::Text(s) | NodeKind::CData(s) => out.push_str(s),
                NodeKind::Element(_) => self.collect_text(c, out),
                _ => {}
            }
        }
    }

    /// The attributes of an element (empty for another node).
    pub fn attributes(&self, n: NodeId) -> &[XmlAttr] {
        self.element(n).map(|e| e.attrs.as_slice()).unwrap_or(&[])
    }

    /// `getAttribute(qualifiedName)`: the first attribute whose qualified
    /// name is `name` (XML documents do not lowercase).
    pub fn get_attribute(&self, n: NodeId, name: &str) -> Option<&str> {
        self.attribute_index(n, name)
            .map(|i| self.attributes(n)[i].value.as_str())
    }

    fn attribute_index(&self, n: NodeId, name: &str) -> Option<usize> {
        self.attributes(n).iter().position(|a| match &a.prefix {
            None => a.local == name,
            Some(p) => {
                name.len() == p.len() + 1 + a.local.len()
                    && name.starts_with(p.as_str())
                    && name[p.len()..].starts_with(':')
                    && name.ends_with(a.local.as_str())
            }
        })
    }

    /// `hasAttribute(qualifiedName)`.
    pub fn has_attribute(&self, n: NodeId, name: &str) -> bool {
        self.attribute_index(n, name).is_some()
    }

    /// `getAttributeNS(namespace, localName)` (an empty namespace is null).
    pub fn get_attribute_ns(&self, n: NodeId, ns: Option<&str>, local: &str) -> Option<&str> {
        let ns = ns.filter(|s| !s.is_empty());
        self.attributes(n)
            .iter()
            .find(|a| a.namespace.as_deref() == ns && a.local == local)
            .map(|a| a.value.as_str())
    }

    /// `lookupNamespaceURI(prefix)` (DOM "locate a namespace"), on an
    /// element, the document (its document element) or another node (its
    /// parent element).
    pub fn lookup_namespace_uri(&self, n: NodeId, prefix: Option<&str>) -> Option<String> {
        let prefix = prefix.filter(|p| !p.is_empty());
        match &self.nodes[n.0].kind {
            NodeKind::Document => self
                .document_element()
                .and_then(|e| self.lookup_namespace_uri(e, prefix)),
            NodeKind::DocumentType { .. } => None,
            NodeKind::Element(e) => {
                if prefix == Some("xml") {
                    return Some(XML_NS.to_owned());
                }
                if prefix == Some("xmlns") {
                    return Some(XMLNS_NS.to_owned());
                }
                if e.namespace.is_some() && e.prefix.as_deref() == prefix {
                    return e.namespace.clone();
                }
                for a in &e.attrs {
                    if a.namespace.as_deref() != Some(XMLNS_NS) {
                        continue;
                    }
                    let hit = match prefix {
                        Some(p) => a.prefix.as_deref() == Some("xmlns") && a.local == p,
                        None => a.prefix.is_none() && a.local == "xmlns",
                    };
                    if hit {
                        return (!a.value.is_empty()).then(|| a.value.clone());
                    }
                }
                match self.parent(n) {
                    Some(p) if self.is_element(p) => self.lookup_namespace_uri(p, prefix),
                    _ => None,
                }
            }
            _ => match self.parent(n) {
                Some(p) if self.is_element(p) => self.lookup_namespace_uri(p, prefix),
                _ => None,
            },
        }
    }

    /// Every descendant of `root` in tree order (not `root` itself).
    pub fn descendants(&self, root: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        self.push_descendants(root, &mut out);
        out
    }

    fn push_descendants(&self, n: NodeId, out: &mut Vec<NodeId>) {
        for &c in &self.nodes[n.0].children {
            out.push(c);
            self.push_descendants(c, out);
        }
    }

    /// `getElementsByTagName(qualifiedName)` (XML document: exact qualified
    /// name; `*` is every element): descendant elements of `root`.
    pub fn get_elements_by_tag_name(&self, root: NodeId, name: &str) -> Vec<NodeId> {
        self.descendants(root)
            .into_iter()
            .filter(|&d| {
                self.element(d)
                    .is_some_and(|e| name == "*" || e.qualified_name() == name)
            })
            .collect()
    }

    /// `getElementsByTagNameNS(namespace, localName)` (`*` matches any
    /// namespace or local name; an empty namespace is null).
    pub fn get_elements_by_tag_name_ns(
        &self,
        root: NodeId,
        ns: Option<&str>,
        local: &str,
    ) -> Vec<NodeId> {
        let ns = ns.filter(|s| !s.is_empty());
        self.descendants(root)
            .into_iter()
            .filter(|&d| {
                self.element(d).is_some_and(|e| {
                    (ns == Some("*") || e.namespace.as_deref() == ns)
                        && (local == "*" || e.local == local)
                })
            })
            .collect()
    }

    /// `document.getElementById(id)`: the first element in tree order with
    /// an `id` attribute (no namespace) equal to `id`; never for "".
    pub fn get_element_by_id(&self, id: &str) -> Option<NodeId> {
        if id.is_empty() {
            return None;
        }
        self.descendants(self.document()).into_iter().find(|&n| {
            self.element(n).is_some_and(|e| {
                e.attrs
                    .iter()
                    .any(|a| a.namespace.is_none() && a.local == "id" && a.value == id)
            })
        })
    }

    /// `querySelectorAll(selectors)` for the selectors the translators use:
    /// a list of compound selectors (type selector or `*`, then
    /// `[attr]`/`[attr="value"]`) joined by descendant combinators. In an
    /// XML document type selectors and attribute names match case-sensitively
    /// on the local name of the element and the qualified name of the
    /// attribute (nwsapi under jsdom 29.0.1, checked 2026-10-07). Results in
    /// tree order; `root` itself is never a result.
    pub fn query_selector_all(&self, root: NodeId, selector: &str) -> Vec<NodeId> {
        let compounds: Vec<Compound> = selector.split_whitespace().map(Compound::parse).collect();
        if compounds.is_empty() {
            return Vec::new();
        }
        self.descendants(root)
            .into_iter()
            .filter(|&d| self.matches_chain(root, d, &compounds))
            .collect()
    }

    /// `querySelector(selectors)`: the first of [`Self::query_selector_all`].
    pub fn query_selector(&self, root: NodeId, selector: &str) -> Option<NodeId> {
        self.query_selector_all(root, selector).into_iter().next()
    }

    fn matches_compound(&self, n: NodeId, c: &Compound) -> bool {
        let Some(e) = self.element(n) else {
            return false;
        };
        if c.tag != "*" && c.tag != e.local {
            return false;
        }
        c.attrs.iter().all(|(name, val)| match val {
            None => self.has_attribute(n, name),
            Some(v) => self.get_attribute(n, name) == Some(v.as_str()),
        })
    }

    /// Whether `n` matches the last compound and its ancestors (up to the
    /// whole tree: selectors are matched against the document, not scoped
    /// to `root`) match the rest in order.
    fn matches_chain(&self, _root: NodeId, n: NodeId, cs: &[Compound]) -> bool {
        let (last, rest) = cs.split_last().expect("non-empty");
        if !self.matches_compound(n, last) {
            return false;
        }
        let mut need = rest.len();
        let mut cur = self.parent(n);
        while need > 0 {
            let Some(p) = cur else {
                return false;
            };
            if self.matches_compound(p, &rest[need - 1]) {
                need -= 1;
            }
            cur = self.parent(p);
        }
        true
    }

    /// Document order of two nodes (DOM `compareDocumentPosition`): an
    /// attribute follows its element and precedes the element's children;
    /// attributes of one element are in attribute order. Nodes in different
    /// trees (a detached node) compare by their roots' ids.
    pub fn compare_order(&self, a: XNode, b: XNode) -> std::cmp::Ordering {
        if a == b {
            return std::cmp::Ordering::Equal;
        }
        let key = |x: XNode| -> Vec<usize> {
            let mut path = Vec::new();
            let mut n = x.node();
            while let Some((p, i)) = self.sibling_index(n) {
                // Children sort after the element's attributes.
                path.push(i + 1_000_000);
                n = p;
            }
            path.push(n.0);
            path.reverse();
            if let XNode::Attr(_, i) = x {
                path.push(i);
            }
            path
        };
        key(a).cmp(&key(b))
    }

    /// `normalize()`: merge adjacent Text nodes (not CDATA) and drop empty
    /// ones, in the whole subtree.
    pub fn normalize(&mut self, n: NodeId) {
        let children = std::mem::take(&mut self.nodes[n.0].children);
        let mut out: Vec<NodeId> = Vec::with_capacity(children.len());
        for c in children {
            if let NodeKind::Text(s) = &self.nodes[c.0].kind {
                if s.is_empty() {
                    self.nodes[c.0].parent = None;
                    continue;
                }
                if let Some(&prev) = out.last() {
                    if let NodeKind::Text(_) = self.nodes[prev.0].kind {
                        let add = s.clone();
                        if let NodeKind::Text(p) = &mut self.nodes[prev.0].kind {
                            p.push_str(&add);
                        }
                        self.nodes[c.0].parent = None;
                        continue;
                    }
                }
            }
            out.push(c);
        }
        self.nodes[n.0].children = out.clone();
        for c in out {
            if self.is_element(c) {
                self.normalize(c);
            }
        }
    }

    /// `createElementNS(namespace, qualifiedName)` (an empty namespace is
    /// null; a `prefix:local` name sets the prefix).
    pub fn create_element_ns(&mut self, ns: Option<&str>, qname: &str) -> NodeId {
        let ns = ns.filter(|s| !s.is_empty()).map(str::to_owned);
        let (prefix, local) = match qname.split_once(':') {
            Some((p, l)) => (Some(p.to_owned()), l.to_owned()),
            None => (None, qname.to_owned()),
        };
        self.push(NodeKind::Element(ElementData {
            namespace: ns,
            prefix,
            local,
            attrs: Vec::new(),
        }))
    }

    /// `createElement(localName)` in an XML document: no namespace, no
    /// prefix (DOM: the HTML namespace only for HTML/XHTML documents).
    pub fn create_element(&mut self, local: &str) -> NodeId {
        self.push(NodeKind::Element(ElementData {
            namespace: None,
            prefix: None,
            local: local.to_owned(),
            attrs: Vec::new(),
        }))
    }

    /// `createTextNode(data)`.
    pub fn create_text_node(&mut self, data: &str) -> NodeId {
        self.push(NodeKind::Text(data.to_owned()))
    }

    /// `createCDATASection(data)`.
    pub fn create_cdata_section(&mut self, data: &str) -> NodeId {
        self.push(NodeKind::CData(data.to_owned()))
    }

    /// `createComment(data)`.
    pub fn create_comment(&mut self, data: &str) -> NodeId {
        self.push(NodeKind::Comment(data.to_owned()))
    }

    /// `createProcessingInstruction(target, data)`.
    pub fn create_processing_instruction(&mut self, target: &str, data: &str) -> NodeId {
        self.push(NodeKind::Pi {
            target: target.to_owned(),
            data: data.to_owned(),
        })
    }

    /// `parent.appendChild(child)` (a child with a parent is moved).
    pub fn append_child(&mut self, parent: NodeId, child: NodeId) {
        self.detach(child);
        self.nodes[parent.0].children.push(child);
        self.nodes[child.0].parent = Some(parent);
    }

    /// `parent.insertBefore(child, reference)` (`None`: append).
    pub fn insert_before(&mut self, parent: NodeId, child: NodeId, reference: Option<NodeId>) {
        self.detach(child);
        let i = reference
            .and_then(|r| self.nodes[parent.0].children.iter().position(|&c| c == r))
            .unwrap_or(self.nodes[parent.0].children.len());
        self.nodes[parent.0].children.insert(i, child);
        self.nodes[child.0].parent = Some(parent);
    }

    /// `node.remove()` / `parent.removeChild(node)`.
    pub fn detach(&mut self, child: NodeId) {
        if let Some(p) = self.nodes[child.0].parent.take() {
            self.nodes[p.0].children.retain(|&c| c != child);
        }
    }

    /// `element.textContent = s`: replace all children by one text node (none
    /// for "").
    pub fn set_text_content(&mut self, n: NodeId, s: &str) {
        for c in std::mem::take(&mut self.nodes[n.0].children) {
            self.nodes[c.0].parent = None;
        }
        if !s.is_empty() {
            let t = self.create_text_node(s);
            self.append_child(n, t);
        }
    }

    /// `setAttribute(qualifiedName, value)` on an element of an XML
    /// document: change the first attribute with that qualified name, or
    /// append one with no namespace and no prefix.
    pub fn set_attribute(&mut self, n: NodeId, name: &str, value: &str) {
        match self.attribute_index(n, name) {
            Some(i) => {
                if let Some(e) = self.element_mut(n) {
                    e.attrs[i].value = value.to_owned();
                }
            }
            None => {
                if let Some(e) = self.element_mut(n) {
                    e.attrs.push(XmlAttr {
                        namespace: None,
                        prefix: None,
                        local: name.to_owned(),
                        value: value.to_owned(),
                    });
                }
            }
        }
    }

    /// `setAttributeNS(namespace, qualifiedName, value)`: change the
    /// attribute with that namespace and local name (keeping its prefix), or
    /// append one.
    pub fn set_attribute_ns(&mut self, n: NodeId, ns: Option<&str>, qname: &str, value: &str) {
        let ns = ns.filter(|s| !s.is_empty()).map(str::to_owned);
        let (prefix, local) = match qname.split_once(':') {
            Some((p, l)) => (Some(p.to_owned()), l.to_owned()),
            None => (None, qname.to_owned()),
        };
        if let Some(e) = self.element_mut(n) {
            match e
                .attrs
                .iter_mut()
                .find(|a| a.namespace == ns && a.local == local)
            {
                Some(a) => a.value = value.to_owned(),
                None => e.attrs.push(XmlAttr {
                    namespace: ns,
                    prefix,
                    local,
                    value: value.to_owned(),
                }),
            }
        }
    }

    /// `removeAttribute(qualifiedName)`.
    pub fn remove_attribute(&mut self, n: NodeId, name: &str) {
        if let Some(i) = self.attribute_index(n, name) {
            if let Some(e) = self.element_mut(n) {
                e.attrs.remove(i);
            }
        }
    }

    /// `new XMLSerializer().serializeToString(node)` (w3c-xmlserializer,
    /// `requireWellFormed` false).
    pub fn serialize(&self, n: NodeId) -> String {
        super::xml_serialize::serialize(self, n, false)
            .expect("serialization without requireWellFormed cannot fail")
    }

    /// `element.innerHTML` in an XML document: each child serialized on its
    /// own, `requireWellFormed` true (jsdom `fragmentSerialization`); an
    /// error is jsdom's `InvalidStateError` message.
    pub fn inner_html(&self, n: NodeId) -> Result<String, String> {
        let mut out = String::new();
        for &c in self.children(n) {
            out.push_str(&super::xml_serialize::serialize(self, c, true)?);
        }
        Ok(out)
    }
}

/// A compound selector: a type selector and attribute conditions.
#[derive(Debug, Clone)]
struct Compound {
    tag: String,
    attrs: Vec<(String, Option<String>)>,
}

impl Compound {
    fn parse(s: &str) -> Compound {
        let (tag, mut rest) = match s.find('[') {
            Some(i) => (&s[..i], &s[i..]),
            None => (s, ""),
        };
        let mut attrs = Vec::new();
        while let Some(r) = rest.strip_prefix('[') {
            let end = r.find(']').unwrap_or(r.len());
            let body = &r[..end];
            match body.split_once('=') {
                Some((k, v)) => {
                    let v = v.trim_matches(|c| c == '"' || c == '\'');
                    attrs.push((k.to_owned(), Some(v.to_owned())));
                }
                None => attrs.push((body.to_owned(), None)),
            }
            rest = r.get(end + 1..).unwrap_or("");
        }
        Compound {
            tag: if tag.is_empty() { "*".to_owned() } else { tag.to_owned() },
            attrs,
        }
    }
}

/// `Zotero.getXML()` (translate.js:2989-2997): the whole input parsed as
/// XML; a parse failure is the error the translator sees thrown.
pub fn get_xml(ctx: &ImportContext) -> Result<XmlDocument, TranslateError> {
    XmlDocument::parse(&ctx.input.text()).map_err(|e| TranslateError::Translator(e.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_content_attributes_and_namespaces() {
        let d = XmlDocument::parse(
            r#"<a xmlns="urn:x" xmlns:p="urn:p" id="1"><b>t<![CDATA[<c>]]><!--n--></b><p:c p:k="v"/></a>"#,
        )
        .unwrap();
        let a = d.document_element().unwrap();
        assert_eq!(d.text(a), "t<c>");
        assert_eq!(d.get_attribute(a, "id"), Some("1"));
        let c = d.element_children(a)[1];
        assert_eq!(d.tag_name(c), "p:c");
        assert_eq!(d.get_attribute(c, "p:k"), Some("v"));
        assert_eq!(d.get_attribute_ns(c, Some("urn:p"), "k"), Some("v"));
        assert_eq!(d.lookup_namespace_uri(d.document(), None).as_deref(), Some("urn:x"));
        assert_eq!(d.lookup_namespace_uri(c, Some("p")).as_deref(), Some("urn:p"));
        assert_eq!(d.attributes(a).len(), 3);
    }

    #[test]
    fn query_selector_matches_local_names_case_sensitively() {
        let d = XmlDocument::parse(
            r#"<m:mets xmlns:m="urn:m"><m:METS/><mets/><m:fileSec><m:file><m:FLocat LOCTYPE="URL"/></m:file></m:fileSec></m:mets>"#,
        )
        .unwrap();
        let root = d.document();
        assert_eq!(d.query_selector_all(root, "mets").len(), 2);
        assert_eq!(d.query_selector_all(root, "fileSec file").len(), 1);
        assert_eq!(d.query_selector_all(root, "FLocat[LOCTYPE=\"URL\"]").len(), 1);
        assert_eq!(d.query_selector_all(root, "FLocat[loctype=\"URL\"]").len(), 0);
    }

    #[test]
    fn malformed_is_an_error() {
        assert!(XmlDocument::parse("<a><b></a>").is_err());
        assert!(XmlDocument::parse("TY  - JOUR").is_err());
    }
}
