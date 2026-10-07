// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translate, https://github.com/zotero/translate (commit
//   dd524aea9a55): src/translation/translate.js
//   `Zotero.Translate.IO.parseDOMXML` :2860-2885 (DOMParser "text/xml",
//   reject a document with a <parsererror>, `normalize()`); the DOMParser is
//   jsdom's in the translation-server (src/translation/translate.js :89 of
//   https://github.com/zotero/translation-server).
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! The XML DOM the RDF parser walks: what `parseDOMXML` gives it.
//!
//! **XML layer: a minimal choice, local to the RDF code.** Another part of
//! the port is adding a general XML layer (`framework/xml.rs`); until the two
//! are reconciled this module builds its DOM with `xml-rs` (already in the
//! workspace's lock file; pure Rust, MIT). It keeps exactly what the RDF
//! parser can observe of jsdom's document after `normalize()`: element
//! namespaces, local names and prefixes; attributes in document order with
//! their namespaces (`xmlns` declarations are not attributes here, see
//! below); text nodes merged as `normalize()` merges them and never empty;
//! CDATA sections, comments and processing instructions as nodes of their
//! own, because they count in `childNodes.length`.
//!
//! Differences from jsdom that cannot reach a translator's output: `xmlns`
//! attributes are dropped (the parser removes them after registering their
//! prefixes, which only matter to a serializer; import never serializes), and
//! the doctype is not a node (the parser looks only for the first element).
//! Well-formedness is xml-rs's judgement, where jsdom uses saxes; both are
//! conforming XML 1.0 parsers, so they reject the same malformed input except
//! in corners (DTD-declared entities, which neither expands).

use xml::reader::{ParserConfig, XmlEvent};

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
    /// `parseDOMXML(input)`: parse, reject a document containing a
    /// `parsererror` element (what jsdom produces for malformed XML, and what
    /// upstream checks for), and normalise text.
    pub fn parse(input: &str) -> Result<Dom, String> {
        let config = ParserConfig::new()
            .trim_whitespace(false)
            .whitespace_to_characters(true)
            .cdata_to_characters(false)
            .ignore_comments(false)
            .coalesce_characters(true)
            .ignore_root_level_whitespace(true);
        let mut reader = config.create_reader(input.as_bytes());
        let mut dom = Dom {
            nodes: vec![DomNode {
                kind: NodeKind::Document,
                children: Vec::new(),
            }],
        };
        let mut stack: Vec<NodeId> = vec![0];
        loop {
            let ev = reader.next().map_err(|e| {
                format!("DOMParser error: loading data into data store failed ({e})")
            })?;
            let parent = *stack.last().unwrap_or(&0);
            match ev {
                XmlEvent::StartDocument { .. } => {}
                XmlEvent::EndDocument => break,
                XmlEvent::ProcessingInstruction { .. } => {
                    dom.push(parent, NodeKind::ProcessingInstruction);
                }
                XmlEvent::StartElement {
                    name, attributes, ..
                } => {
                    // `getElementsByTagName("parsererror")` matches the
                    // qualified name.
                    let qualified = match &name.prefix {
                        Some(p) => format!("{p}:{}", name.local_name),
                        None => name.local_name.clone(),
                    };
                    if qualified == "parsererror" {
                        return Err(
                            "DOMParser error: loading data into data store failed".to_owned()
                        );
                    }
                    let attrs = attributes
                        .into_iter()
                        .map(|a| Attr {
                            namespace: a.name.namespace.filter(|n| !n.is_empty()),
                            local_name: a.name.local_name,
                            prefix: a.name.prefix,
                            value: a.value,
                        })
                        .collect();
                    let id = dom.push(
                        parent,
                        NodeKind::Element {
                            namespace: name.namespace.filter(|n| !n.is_empty()),
                            local_name: name.local_name,
                            prefix: name.prefix,
                            attrs,
                        },
                    );
                    stack.push(id);
                }
                XmlEvent::EndElement { .. } => {
                    stack.pop();
                }
                XmlEvent::CData(s) => {
                    dom.push(parent, NodeKind::CData(s));
                }
                XmlEvent::Comment(_) => {
                    dom.push(parent, NodeKind::Comment);
                }
                XmlEvent::Characters(s) | XmlEvent::Whitespace(s) => {
                    if parent == 0 || s.is_empty() {
                        continue;
                    }
                    // `normalize()`: adjacent text nodes merge.
                    let last = dom.nodes[parent].children.last().copied();
                    if let Some(l) = last {
                        if let NodeKind::Text(t) = &mut dom.nodes[l].kind {
                            t.push_str(&s);
                            continue;
                        }
                    }
                    dom.push(parent, NodeKind::Text(s));
                }
            }
        }
        Ok(dom)
    }

    fn push(&mut self, parent: NodeId, kind: NodeKind) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(DomNode {
            kind,
            children: Vec::new(),
        });
        self.nodes[parent].children.push(id);
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
        let a = d.attrs(root);
        assert_eq!(a.len(), 2);
        assert_eq!(a[0].namespace.as_deref(), Some("urn:r"));
        assert_eq!(a[1].namespace, None);
        assert!(Dom::parse("<a>").is_err());
        assert!(Dom::parse("<parsererror/>").is_err());
    }
}
