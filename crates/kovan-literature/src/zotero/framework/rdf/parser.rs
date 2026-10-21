// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translate, https://github.com/zotero/translate (commit
//   dd524aea9a55): src/rdf/rdfparser.js (the Tabulator RDF/XML parser by
//   David Sheets: `frameFactory` :107-262, `parse` :308-342, `parseDOM`
//   :343-519, `buildFrame` :533-575). rdfparser.js is under the W3C Software
//   Notice and License; its full text is in this crate's NOTICE.
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA;
//   (c) World Wide Web Consortium (MIT, ERCIM, Keio).
// Licence: AGPL-3.0 (upstream translate: AGPL-3.0-or-later; rdfparser.js:
//   W3C Software Notice and License, GPL-compatible). Changes from
//   upstream (2026-10-07): translated to Rust; frames and DOM nodes are
//   indices into arenas; the provenance argument (`why`) and reification
//   (off upstream: `reify = false`) are not carried.

//! RDF/XML into a [`Store`]: upstream's frame machine, ported step for step,
//! quirks included (the parser is the specification of what Zotero reads).
//!
//! Quirks worth knowing, all upstream's:
//! * text is only a literal when it is an element's only child; text beside
//!   other nodes (including comments) is skipped;
//! * a typed node element that also has an `rdf:type` attribute keeps the
//!   attribute, which then becomes a literal-valued property;
//! * `rdf:parseType="Literal"` stores the literal `[object Element]` (the
//!   element's `toString()`), not its markup;
//! * after an `rdf:parseType="Resource"` or `"Collection"` arc, or a
//!   collection member, the walk resumes from the arc's frame, so later
//!   sibling properties hang off that frame (`pframe` in `parseDOM`).

use super::dom::{Attr, Dom, NodeId, NodeKind};
use super::store::Store;
use super::term::{Node, Term, RDF_NS};
use super::uri::join;
use std::collections::HashMap;

const NODE: u8 = 1;
const ARC: u8 = 2;

#[derive(Debug, Clone, Default)]
struct Frame {
    parent: Option<usize>,
    element: Option<NodeId>,
    last_child: usize,
    base: String,
    lang: String,
    node: Option<Node>,
    node_type: Option<u8>,
    list_index: u64,
    datatype: Option<String>,
    collection: bool,
}

struct Parser<'a> {
    dom: &'a mut Dom,
    store: &'a mut Store,
    frames: Vec<Frame>,
    bnodes: HashMap<String, Node>,
}

/// `RDFParser.parse(document, base)`: add the document's triples to `store`.
pub fn parse(dom: &mut Dom, store: &mut Store, base: &str) -> Result<(), String> {
    let root = dom
        .node(0)
        .children
        .iter()
        .copied()
        .find(|&c| dom.node(c).node_type() == 1)
        .ok_or_else(|| format!("RDFParser: can't find root in {base}. Halting. "))?;
    let mut p = Parser {
        dom,
        store,
        frames: Vec::new(),
        bnodes: HashMap::new(),
    };
    p.frames.push(Frame {
        base: base.to_owned(),
        list_index: 1,
        ..Default::default()
    });
    let f = p.build_frame(Some(0), Some(root));
    p.parse_dom(f)
}

/// `elementURI(el)`: namespace + local name; an element or attribute with
/// no namespace is a syntax error.
fn element_uri(namespace: Option<&str>, local: &str) -> Result<String, String> {
    match namespace {
        None => Err(format!(
            "RDF/XML syntax error: No namespace for {local} in undefined"
        )),
        Some(ns) => Ok(format!("{ns}{local}")),
    }
}

fn attr_uri(a: &Attr) -> Result<String, String> {
    element_uri(a.namespace.as_deref(), &a.local_name)
}

impl Parser<'_> {
    fn el_uri(&self, el: NodeId) -> Result<String, String> {
        match &self.dom.node(el).kind {
            NodeKind::Element {
                namespace,
                local_name,
                ..
            } => element_uri(namespace.as_deref(), local_name),
            _ => Err("not an element".to_owned()),
        }
    }

    /// `getAttributeNodeNS(el, RDF, name)`: the attribute's value.
    fn rdf_attr(&self, el: NodeId, name: &str) -> Option<String> {
        self.dom
            .attrs(el)
            .iter()
            .find(|a| a.namespace.as_deref() == Some(RDF_NS) && a.local_name == name)
            .map(|a| a.value.clone())
    }

    /// `removeAttributeNode` of that attribute.
    fn remove_rdf_attr(&mut self, el: NodeId, name: &str) {
        if let Some(attrs) = self.dom.attrs_mut(el) {
            if let Some(i) = attrs
                .iter()
                .position(|a| a.namespace.as_deref() == Some(RDF_NS) && a.local_name == name)
            {
                attrs.remove(i);
            }
        }
    }

    /// `buildFrame(parent, element)` (:533-575).
    fn build_frame(&mut self, parent: Option<usize>, element: Option<NodeId>) -> usize {
        let mut frame = Frame {
            parent,
            element,
            list_index: 1,
            ..Default::default()
        };
        if let Some(p) = parent {
            frame.base = self.frames[p].base.clone();
            frame.lang = self.frames[p].lang.clone();
        }
        if let Some(el) = element {
            if self.dom.node(el).node_type() == 1 {
                // xml:base and xml:lang (by qualified name), then every
                // attribute whose name starts with "xml", last first.
                let attrs = self.dom.attrs_mut(el).expect("element");
                if let Some(i) = attrs.iter().position(|a| a.node_name() == "xml:base") {
                    frame.base = attrs.remove(i).value;
                }
                if let Some(i) = attrs.iter().position(|a| a.node_name() == "xml:lang") {
                    frame.lang = attrs.remove(i).value;
                }
                let mut x = attrs.len();
                let mut prefixes = Vec::new();
                while x > 0 {
                    x -= 1;
                    let name = attrs[x].node_name();
                    if name.starts_with("xml") {
                        // `xmlns:p` registers p (the parser's base is "",
                        // so the URI is not joined with it).
                        if let Some(p) = name.strip_prefix("xmlns:") {
                            prefixes.push((p.to_owned(), attrs[x].value.clone()));
                        }
                        attrs.remove(x);
                    }
                }
                for (p, u) in prefixes {
                    self.store.set_prefix_for_uri(&p, &u);
                }
            }
        }
        self.frames.push(frame);
        self.frames.len() - 1
    }

    fn parent(&self, f: usize) -> Option<usize> {
        self.frames[f].parent
    }

    /// `addSymbol(type, uri)`.
    fn add_symbol(&mut self, f: usize, node_type: u8, uri: &str) {
        let uri = join(uri, &self.frames[f].base);
        self.frames[f].node = Some(self.store.sym(uri));
        self.frames[f].node_type = Some(node_type);
    }

    /// `isTripleToLoad()`.
    fn is_triple_to_load(&self, f: usize) -> bool {
        let Some(p) = self.parent(f) else {
            return false;
        };
        let Some(pp) = self.parent(p) else {
            return false;
        };
        self.frames[f].node_type == Some(NODE)
            && self.frames[p].node_type == Some(ARC)
            && self.frames[pp].node_type == Some(NODE)
    }

    /// `loadTriple()` (no reification).
    fn load_triple(&mut self, f: usize) {
        let p = self.parent(f).expect("checked");
        let pp = self.parent(p).expect("checked");
        let node = self.frames[f].node.clone().expect("node");
        if self.frames[pp].collection {
            if let Some(Node {
                term: Term::Collection(id),
                ..
            }) = &self.frames[pp].node
            {
                let id = *id;
                self.store.collection_append(id, node);
            }
        } else {
            let s = self.frames[pp].node.clone().expect("node");
            let pr = self.frames[p].node.clone().expect("node");
            self.store.add(s, pr, node);
        }
    }

    fn maybe_load(&mut self, f: usize) {
        if self.is_triple_to_load(f) {
            self.load_triple(f);
        }
    }

    /// `addNode(uri)`.
    fn add_node(&mut self, f: usize, uri: &str) {
        self.add_symbol(f, NODE, uri);
        self.maybe_load(f);
    }

    /// `addCollection()`.
    fn add_collection(&mut self, f: usize) {
        self.frames[f].node_type = Some(NODE);
        self.frames[f].node = Some(self.store.collection());
        self.frames[f].collection = true;
        self.maybe_load(f);
    }

    /// `addBNode(id)`.
    fn add_bnode(&mut self, f: usize, id: Option<&str>) {
        let node = match id {
            Some(id) => match self.bnodes.get(id) {
                Some(n) => n.clone(),
                None => {
                    let n = self.store.bnode();
                    self.bnodes.insert(id.to_owned(), n.clone());
                    n
                }
            },
            None => self.store.bnode(),
        };
        self.frames[f].node = Some(node);
        self.frames[f].node_type = Some(NODE);
        self.maybe_load(f);
    }

    /// `addArc(uri)`.
    fn add_arc(&mut self, f: usize, uri: &str) {
        let uri = if uri == format!("{RDF_NS}li") {
            let p = self.parent(f).expect("an arc has a parent");
            let n = self.frames[p].list_index;
            self.frames[p].list_index += 1;
            format!("{RDF_NS}_{n}")
        } else {
            uri.to_owned()
        };
        self.add_symbol(f, ARC, &uri);
    }

    /// `addLiteral(value)`.
    fn add_literal(&mut self, f: usize, value: &str) {
        let p = self.parent(f);
        // `if (this.parent.datatype)`: an empty datatype is falsy.
        let datatype = p
            .and_then(|p| self.frames[p].datatype.clone())
            .filter(|d| !d.is_empty());
        let node = match datatype {
            Some(dt) => self.store.literal(value, None, Some(&dt)),
            None => {
                let lang = self.frames[f].lang.clone();
                self.store.literal(value, Some(&lang), None)
            }
        };
        self.frames[f].node = Some(node);
        self.frames[f].node_type = Some(NODE);
        self.maybe_load(f);
    }

    /// `parseDOM(frame)` (:343-519).
    fn parse_dom(&mut self, mut frame: usize) -> Result<(), String> {
        let mut dig = true;
        while self.parent(frame).is_some() {
            let dom = self.frames[frame]
                .element
                .expect("a walked frame has an element");
            let kind = self.dom.node(dom).kind.clone();
            match &kind {
                NodeKind::Text(t) | NodeKind::CData(t) => {
                    let p = self.parent(frame).expect("loop condition");
                    if self.frames[p].node_type == Some(NODE) {
                        self.add_arc(frame, &format!("{RDF_NS}value"));
                        frame = self.build_frame(Some(frame), None);
                    }
                    self.add_literal(frame, t);
                }
                NodeKind::Element { .. } if self.el_uri(dom)? != format!("{RDF_NS}RDF") => {
                    if let Some(p) = self.parent(frame) {
                        if self.frames[p].collection {
                            self.frames[frame].node_type = Some(ARC);
                            let el = self.frames[frame].element;
                            frame = self.build_frame(Some(frame), el);
                            let p = self.parent(frame).expect("just built");
                            self.frames[p].element = None;
                        }
                    }
                    let p = self.parent(frame);
                    let needs_node = match p {
                        None => true,
                        Some(p) => matches!(self.frames[p].node_type, None | Some(ARC)),
                    };
                    if needs_node {
                        self.node_element(frame, dom)?;
                    } else {
                        let (f2, d2) = self.property_element(frame, dom)?;
                        frame = f2;
                        if !d2 {
                            dig = false;
                        }
                    }
                }
                _ => {}
            }
            // dig dug
            let mut dom = self.frames[frame].element;
            while self.parent(frame).is_some() {
                let pframe = frame;
                while dom.is_none() {
                    frame = self
                        .parent(frame)
                        .ok_or("RDFParser: walked off the top frame")?;
                    dom = self.frames[frame].element;
                }
                let d = dom.expect("loop above");
                let candidate = self
                    .dom
                    .node(d)
                    .children
                    .get(self.frames[frame].last_child)
                    .copied();
                match candidate {
                    Some(c) if dig => {
                        let ct = self.dom.node(c).node_type();
                        let n_children = self.dom.node(d).children.len();
                        if !matches!(ct, 1 | 3 | 4) || (matches!(ct, 3 | 4) && n_children != 1) {
                            self.frames[frame].last_child += 1;
                        } else {
                            self.frames[frame].last_child += 1;
                            frame = self.build_frame(Some(pframe), Some(c));
                            break;
                        }
                    }
                    _ => {
                        // terminateFrame(): closing a collection does nothing
                        // observable.
                        match self.parent(frame) {
                            None => break,
                            Some(p) => frame = p,
                        }
                        dom = self.frames[frame].element;
                        dig = true;
                    }
                }
            }
        }
        Ok(())
    }

    /// The "we need a node" branch (:381-436).
    fn node_element(&mut self, frame: usize, dom: NodeId) -> Result<(), String> {
        let about = self.rdf_attr(dom, "about");
        let rdfid = self.rdf_attr(dom, "ID");
        if about.is_some() && rdfid.is_some() {
            return Err(format!(
                "RDFParser: {} has both rdf:id and rdf:about. Halting. Only one of these properties may be specified on a node.",
                self.qualified_name(dom)
            ));
        }
        match (about, rdfid) {
            (None, Some(id)) => {
                self.add_node(frame, &format!("#{id}"));
                self.remove_rdf_attr(dom, "ID");
            }
            (None, None) => match self.rdf_attr(dom, "nodeID") {
                Some(bnid) => {
                    self.add_bnode(frame, Some(&bnid));
                    self.remove_rdf_attr(dom, "nodeID");
                }
                None => self.add_bnode(frame, None),
            },
            (Some(about), _) => {
                self.add_node(frame, &about);
                self.remove_rdf_attr(dom, "about");
            }
        }
        // Typed nodes.
        let el_uri = self.el_uri(dom)?;
        let mut rdftype = self.rdf_attr(dom, "type").map(|v| (v, true));
        if el_uri != format!("{RDF_NS}Description") {
            rdftype = Some((el_uri, false));
        }
        if let Some((value, is_attr)) = rdftype {
            let node = self.frames[frame].node.clone().expect("node");
            let ty = self.store.sym(format!("{RDF_NS}type"));
            let obj = self.store.sym(join(&value, &self.frames[frame].base));
            self.store.add(node, ty, obj);
            if is_attr {
                self.remove_rdf_attr(dom, "type");
            }
        }
        // Property attributes, last first.
        let attrs = self.dom.attrs(dom).to_vec();
        let lang = self.frames[frame].lang.clone();
        for a in attrs.iter().rev() {
            let node = self.frames[frame].node.clone().expect("node");
            let p = self.store.sym(attr_uri(a)?);
            let o = self.store.literal(a.value.clone(), Some(&lang), None);
            self.store.add(node, p, o);
        }
        Ok(())
    }

    /// The arc branch (:437-500). Returns the frame the walk continues from
    /// and whether to dig into the element's children.
    fn property_element(&mut self, mut frame: usize, dom: NodeId) -> Result<(usize, bool), String> {
        let mut dig = true;
        let uri = self.el_uri(dom)?;
        self.add_arc(frame, &uri);
        let parsetype = self.rdf_attr(dom, "parseType");
        if let Some(dt) = self.rdf_attr(dom, "datatype") {
            self.frames[frame].datatype = Some(dt);
            self.remove_rdf_attr(dom, "datatype");
        }
        if let Some(nv) = parsetype {
            match nv.as_str() {
                "Literal" => {
                    self.frames[frame].datatype = Some(format!("{RDF_NS}XMLLiteral"));
                    frame = self.build_frame(Some(frame), None);
                    // `addLiteral(dom)`: the element's toString().
                    self.add_literal(frame, "[object Element]");
                    dig = false;
                }
                "Resource" => {
                    let el = self.frames[frame].element;
                    frame = self.build_frame(Some(frame), el);
                    let p = self.parent(frame).expect("just built");
                    self.frames[p].element = None;
                    self.add_bnode(frame, None);
                }
                "Collection" => {
                    let el = self.frames[frame].element;
                    frame = self.build_frame(Some(frame), el);
                    let p = self.parent(frame).expect("just built");
                    self.frames[p].element = None;
                    self.add_collection(frame);
                }
                _ => {}
            }
            self.remove_rdf_attr(dom, "parseType");
        }
        if !self.dom.attrs(dom).is_empty() {
            let resource = self.rdf_attr(dom, "resource");
            let bnid = self.rdf_attr(dom, "nodeID");
            frame = self.build_frame(Some(frame), None);
            if let Some(r) = resource {
                self.add_node(frame, &r);
                self.remove_rdf_attr(dom, "resource");
            } else if let Some(id) = bnid {
                self.add_bnode(frame, Some(&id));
                self.remove_rdf_attr(dom, "nodeID");
            } else {
                self.add_bnode(frame, None);
            }
            let attrs = self.dom.attrs(dom).to_vec();
            for a in attrs.iter().rev() {
                let f = self.build_frame(Some(frame), None);
                let au = attr_uri(a)?;
                self.add_arc(f, &au);
                let g = self.build_frame(Some(f), None);
                if au == format!("{RDF_NS}type") {
                    self.add_node(g, &a.value);
                } else {
                    self.add_literal(g, &a.value);
                }
            }
        } else if self.dom.node(dom).children.is_empty() {
            let g = self.build_frame(Some(frame), None);
            self.add_literal(g, "");
        }
        Ok((frame, dig))
    }

    fn qualified_name(&self, el: NodeId) -> String {
        match &self.dom.node(el).kind {
            NodeKind::Element {
                local_name, prefix, ..
            } => match prefix {
                Some(p) => format!("{p}:{local_name}"),
                None => local_name.clone(),
            },
            _ => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triples(xml: &str) -> Vec<String> {
        let mut dom = Dom::parse(xml).unwrap();
        let mut s = Store::new();
        parse(&mut dom, &mut s, "").unwrap();
        s.statements
            .iter()
            .map(|st| {
                format!(
                    "{} {} {}",
                    st.subject.term.to_nt(),
                    st.predicate.term.to_nt(),
                    st.object.term.to_nt()
                )
            })
            .collect()
    }

    #[test]
    fn typed_nodes_nested_resources_and_lists() {
        let t = triples(
            r##"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:d="urn:d#">
  <d:Book rdf:about="#b1" d:a="x">
    <d:title>T</d:title>
    <d:authors><rdf:Seq><rdf:li><d:P><d:n>N</d:n></d:P></rdf:li></rdf:Seq></d:authors>
    <d:empty/>
  </d:Book>
</rdf:RDF>"##,
        );
        let rdf = RDF_NS;
        assert_eq!(
            t,
            [
                format!("<#b1> <{rdf}type> <urn:d#Book>"),
                "<#b1> <urn:d#a> \"x\"".to_owned(),
                "<#b1> <urn:d#title> \"T\"".to_owned(),
                "<#b1> <urn:d#authors> _:n0".to_owned(),
                format!("_:n0 <{rdf}type> <{rdf}Seq>"),
                format!("_:n0 <{rdf}_1> _:n1"),
                format!("_:n1 <{rdf}type> <urn:d#P>"),
                "_:n1 <urn:d#n> \"N\"".to_owned(),
                "<#b1> <urn:d#empty> \"\"".to_owned(),
            ]
        );
    }
}
