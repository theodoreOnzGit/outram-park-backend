// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translate, https://github.com/zotero/translate (commit
//   dd524aea9a55): src/translation/translate.js
//   `Zotero.Translate.IO.parseDOMXML` :2860-2885 (DOMParser "text/xml",
//   reject a document with a <parsererror>, `normalize()`), which
//   `IO.String._initRDF` :2919-2934 hands to the RDF parser; the DOMParser
//   is jsdom's in the translation-server.
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The XML DOM the RDF parser walks: what `parseDOMXML` gives it.
//!
//! Upstream's RDF data mode parses with the same `parseDOMXML` as the XML
//! translators' `Zotero.getXML()`, so the document comes from the framework's
//! XML layer ([`super::super::xml::XmlDocument::parse`]: saxes as jsdom
//! drives it, the `parsererror` check, `normalize()`). This module copies it
//! into a small arena the RDF parser can mutate as upstream's does (it
//! removes attributes as it consumes them): element namespaces, prefixes and
//! local names; attributes in document order, `xmlns` declarations included
//! (the parser registers and removes them, as upstream's `buildFrame` does);
//! text, CDATA, comments and processing instructions as nodes of their own,
//! since they count in `childNodes.length`.
//!
//! ~~This module built its DOM with xml-rs until the XML layer existed.~~
//! **CHANGED 2026-10-07** (merge with develop dbb9e26eb1): the XML layer is
//! the one parser; the RDF references were identical with both.

use crate::zotero::framework::xml::{NodeId as XNodeId, NodeKind as XKind, XmlDocument};

/// A DOM node's index in [`Dom::nodes`].
pub type NodeId = usize;

/// An attribute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attr {
    /// `namespaceURI` (`None` is `null`).
    pub namespace: Option<String>,
    /// `localName`.
    pub local_name: String,
    /// `prefix`.
    pub prefix: Option<String>,
    /// `nodeValue`.
    pub value: String,
}

impl Attr {
    /// `nodeName` / `name`: the qualified name.
    pub fn node_name(&self) -> String {
        match &self.prefix {
            Some(p) => format!("{p}:{}", self.local_name),
            None => self.local_name.clone(),
        }
    }
}

/// What a node is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    /// The document (nodeType 9).
    Document,
    /// An element (nodeType 1).
    Element {
        /// `namespaceURI`.
        namespace: Option<String>,
        /// `localName`.
        local_name: String,
        /// `prefix`.
        prefix: Option<String>,
        /// `attributes`, in document order (mutable: the parser removes them).
        attrs: Vec<Attr>,
    },
    /// A text node (nodeType 3).
    Text(String),
    /// A CDATA section (nodeType 4).
    CData(String),
    /// A processing instruction (nodeType 7).
    ProcessingInstruction,
    /// A comment (nodeType 8).
    Comment,
    /// A document type (nodeType 10).
    DocumentType,
}

/// A node and its children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomNode {
    /// What it is.
    pub kind: NodeKind,
    /// `childNodes`.
    pub children: Vec<NodeId>,
}

impl DomNode {
    /// `nodeType`.
    pub fn node_type(&self) -> u8 {
        match self.kind {
            NodeKind::Element { .. } => 1,
            NodeKind::Text(_) => 3,
            NodeKind::CData(_) => 4,
            NodeKind::ProcessingInstruction => 7,
            NodeKind::Comment => 8,
            NodeKind::Document => 9,
            NodeKind::DocumentType => 10,
        }
    }
}

/// A parsed document; node 0 is the document node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dom {
    /// Every node.
    pub nodes: Vec<DomNode>,
}

impl Dom {
    /// `parseDOMXML(input)` ([`XmlDocument::parse`]), copied into the arena.
    pub fn parse(input: &str) -> Result<Dom, String> {
        let doc = XmlDocument::parse(input).map_err(|e| e.0)?;
        let mut dom = Dom { nodes: Vec::new() };
        dom.copy(&doc, doc.document());
        Ok(dom)
    }

    fn copy(&mut self, doc: &XmlDocument, n: XNodeId) -> NodeId {
        let kind = match doc.kind(n) {
            XKind::Document => NodeKind::Document,
            XKind::DocumentType { .. } => NodeKind::DocumentType,
            XKind::Element(e) => NodeKind::Element {
                namespace: e.namespace.clone(),
                local_name: e.local.clone(),
                prefix: e.prefix.clone(),
                attrs: e
                    .attrs
                    .iter()
                    .map(|a| Attr {
                        namespace: a.namespace.clone(),
                        local_name: a.local.clone(),
                        prefix: a.prefix.clone(),
                        value: a.value.clone(),
                    })
                    .collect(),
            },
            XKind::Text(t) => NodeKind::Text(t.clone()),
            XKind::CData(t) => NodeKind::CData(t.clone()),
            XKind::Comment(_) => NodeKind::Comment,
            XKind::Pi { .. } => NodeKind::ProcessingInstruction,
        };
        let id = self.nodes.len();
        self.nodes.push(DomNode {
            kind,
            children: Vec::new(),
        });
        let children: Vec<NodeId> = doc.children(n).iter().map(|&c| self.copy(doc, c)).collect();
        self.nodes[id].children = children;
        id
    }

    /// The node.
    pub fn node(&self, id: NodeId) -> &DomNode {
        &self.nodes[id]
    }

    /// The attributes of an element (empty for any other node).
    pub fn attrs(&self, id: NodeId) -> &[Attr] {
        match &self.nodes[id].kind {
            NodeKind::Element { attrs, .. } => attrs,
            _ => &[],
        }
    }

    /// The attributes of an element, mutably.
    pub fn attrs_mut(&mut self, id: NodeId) -> Option<&mut Vec<Attr>> {
        match &mut self.nodes[id].kind {
            NodeKind::Element { attrs, .. } => Some(attrs),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_cdata_and_comments_are_separate_children() {
        let d = Dom::parse(r#"<a xmlns="urn:x">t&amp;u<![CDATA[c]]><!--k-->v</a>"#).unwrap();
        let root = d.node(0).children[0];
        let kinds: Vec<u8> = d
            .node(root)
            .children
            .iter()
            .map(|&c| d.node(c).node_type())
            .collect();
        assert_eq!(kinds, [3, 4, 8, 3]);
        assert_eq!(
            d.node(d.node(root).children[0]).kind,
            NodeKind::Text("t&u".into())
        );
    }

    #[test]
    fn attribute_namespaces() {
        let d = Dom::parse(r#"<r:a xmlns:r="urn:r" r:x="1" y="2"/>"#).unwrap();
        let root = d.node(0).children[0];
        let a: Vec<&Attr> = d
            .attrs(root)
            .iter()
            .filter(|a| !a.node_name().starts_with("xmlns"))
            .collect();
        assert_eq!(a.len(), 2);
        assert_eq!(a[0].namespace.as_deref(), Some("urn:r"));
        assert_eq!(a[1].namespace, None);
        assert!(Dom::parse("<a>").is_err());
        assert!(Dom::parse("<parsererror/>").is_err());
    }
}
