// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: the HTML side of the DOM the translation-server gives the note
//   exporters (Note HTML, Note Markdown): jsdom 29.0.1 (MIT,
//   https://github.com/jsdom/jsdom) — `DOMParser.parseFromString(s,
//   "text/html")` and the `innerHTML` setter (parse5 8.0.0, MIT: the WHATWG
//   HTML parsing algorithm, here html5ever, which implements the same
//   algorithm), `outerHTML`/`innerHTML` getters (parse5's serializer,
//   serializer/index.js, with entities' `escapeText`/`escapeAttribute`),
//   `createElement` in an HTML document, `className`, `classList.contains`,
//   `querySelectorAll` (@asamuzakjp/dom-selector) for the selectors the
//   translators use, and `element.style` (jsdom's CSSStyleDeclaration) for the
//   properties they read and set.
// Copyright (c) 2010 Elijah Insua and the jsdom contributors; (c) Ivan
//   Nikulin and parse5 contributors; (c) Felix Boehm (entities). All MIT.
// Licence: AGPL-3.0 (this port; MIT permits it).

//! HTML documents in the [`XmlDocument`] arena: HTML elements are elements
//! in the XHTML namespace with no prefix, as in the DOM.
//!
//! Checked against jsdom 29.0.1 (node, 2026-10-07): `element.style`
//! recognises only lowercase property names (`TEXT-DECORATION: ...` reads
//! as unset) and serialises `name: value;` joined by spaces, colours
//! `#rgb`/`#rrggbb` as `rgb(r, g, b)`, invalid declarations dropped.
//!
//! Not modelled (none occurs in the note exporters' inputs): template
//! contents (dropped), other CSS value normalisations, `noscript` with
//! scripting enabled (DOMParser documents have scripting disabled).

use super::xml::{ElementData, NodeId, NodeKind, XmlAttr, XmlDocument, HTML_NS, XMLNS_NS, XML_NS};
use html5ever::interface::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::{
    parse_document, parse_fragment, Attribute, LocalName, Namespace, ParseOpts, Prefix, QualName,
};
use std::borrow::Cow;
use std::cell::RefCell;

const XLINK_NS: &str = "http://www.w3.org/1999/xlink";

/// The element name html5ever asks for.
#[derive(Debug)]
pub struct OwnedElemName(QualName);

impl html5ever::interface::ElemName for OwnedElemName {
    fn ns(&self) -> &Namespace {
        &self.0.ns
    }
    fn local_name(&self) -> &LocalName {
        &self.0.local
    }
}

/// A TreeSink building into an [`XmlDocument`].
struct Sink {
    doc: RefCell<XmlDocument>,
}

fn attr_of(a: &Attribute) -> XmlAttr {
    let ns: &str = &a.name.ns;
    XmlAttr {
        namespace: (!ns.is_empty()).then(|| ns.to_owned()),
        prefix: a.name.prefix.as_ref().map(|p| p.to_string()),
        local: a.name.local.to_string(),
        value: a.value.to_string(),
    }
}

impl Sink {
    fn append_text(doc: &mut XmlDocument, parent: NodeId, text: &str) {
        if let Some(&last) = doc.children(parent).last() {
            if let NodeKind::Text(_) = doc.kind(last) {
                doc.append_text_data(last, text);
                return;
            }
        }
        let t = doc.create_text_node(text);
        doc.append_child(parent, t);
    }
}

impl TreeSink for Sink {
    type Handle = NodeId;
    type Output = XmlDocument;
    type ElemName<'a>
        = OwnedElemName
    where
        Self: 'a;

    fn finish(self) -> XmlDocument {
        self.doc.into_inner()
    }

    fn parse_error(&self, _msg: Cow<'static, str>) {}

    fn get_document(&self) -> NodeId {
        NodeId(0)
    }

    fn elem_name<'a>(&'a self, target: &'a NodeId) -> OwnedElemName {
        let d = self.doc.borrow();
        let e = d.element(*target).expect("element");
        OwnedElemName(QualName::new(
            e.prefix.as_deref().map(Prefix::from),
            Namespace::from(e.namespace.as_deref().unwrap_or("")),
            LocalName::from(e.local.as_str()),
        ))
    }

    fn create_element(
        &self,
        name: QualName,
        attrs: Vec<Attribute>,
        _flags: ElementFlags,
    ) -> NodeId {
        let ns: &str = &name.ns;
        self.doc.borrow_mut().push(NodeKind::Element(ElementData {
            namespace: (!ns.is_empty()).then(|| ns.to_owned()),
            prefix: name.prefix.as_ref().map(|p| p.to_string()),
            local: name.local.to_string(),
            attrs: attrs.iter().map(attr_of).collect(),
        }))
    }

    fn create_comment(&self, text: StrTendril) -> NodeId {
        self.doc.borrow_mut().create_comment(&text)
    }

    fn create_pi(&self, target: StrTendril, data: StrTendril) -> NodeId {
        self.doc
            .borrow_mut()
            .create_processing_instruction(&target, &data)
    }

    fn append(&self, parent: &NodeId, child: NodeOrText<NodeId>) {
        let mut d = self.doc.borrow_mut();
        match child {
            NodeOrText::AppendNode(n) => d.append_child(*parent, n),
            NodeOrText::AppendText(t) => Sink::append_text(&mut d, *parent, &t),
        }
    }

    fn append_based_on_parent_node(
        &self,
        element: &NodeId,
        prev_element: &NodeId,
        child: NodeOrText<NodeId>,
    ) {
        let has_parent = self.doc.borrow().parent(*element).is_some();
        if has_parent {
            self.append_before_sibling(element, child);
        } else {
            self.append(prev_element, child);
        }
    }

    fn append_doctype_to_document(
        &self,
        name: StrTendril,
        public_id: StrTendril,
        system_id: StrTendril,
    ) {
        let mut d = self.doc.borrow_mut();
        let n = d.push(NodeKind::DocumentType {
            name: name.to_string(),
            public_id: public_id.to_string(),
            system_id: system_id.to_string(),
        });
        let root = d.document();
        d.append_child(root, n);
    }

    fn get_template_contents(&self, _target: &NodeId) -> NodeId {
        // Template contents are not modelled (module docs): a detached
        // holder that is never attached.
        self.doc.borrow_mut().create_element("#template-contents")
    }

    fn same_node(&self, x: &NodeId, y: &NodeId) -> bool {
        x == y
    }

    fn set_quirks_mode(&self, _mode: QuirksMode) {}

    fn append_before_sibling(&self, sibling: &NodeId, new_node: NodeOrText<NodeId>) {
        let mut d = self.doc.borrow_mut();
        let Some(parent) = d.parent(*sibling) else {
            return;
        };
        match new_node {
            NodeOrText::AppendNode(n) => d.insert_before(parent, n, Some(*sibling)),
            NodeOrText::AppendText(t) => {
                // Merge with a text node just before the sibling.
                if let Some(prev) = d.previous_sibling(*sibling) {
                    if let NodeKind::Text(_) = d.kind(prev) {
                        d.append_text_data(prev, &t);
                        return;
                    }
                }
                let tn = d.create_text_node(&t);
                d.insert_before(parent, tn, Some(*sibling));
            }
        }
    }

    fn add_attrs_if_missing(&self, target: &NodeId, attrs: Vec<Attribute>) {
        let mut d = self.doc.borrow_mut();
        for a in attrs {
            let x = attr_of(&a);
            let exists = d
                .attributes(*target)
                .iter()
                .any(|b| b.namespace == x.namespace && b.local == x.local);
            if !exists {
                d.push_attribute(*target, x);
            }
        }
    }

    fn remove_from_parent(&self, target: &NodeId) {
        self.doc.borrow_mut().detach(*target);
    }

    fn reparent_children(&self, node: &NodeId, new_parent: &NodeId) {
        let mut d = self.doc.borrow_mut();
        let kids: Vec<NodeId> = d.children(*node).to_vec();
        for k in kids {
            d.append_child(*new_parent, k);
        }
    }

    fn allow_declarative_shadow_roots(&self, _intended_parent: &NodeId) -> bool {
        false
    }
}

/// Parser options: scripting disabled, as in a document from `DOMParser`
/// (so `<noscript>` content is parsed as markup).
fn parse_opts() -> ParseOpts {
    let mut o = ParseOpts::default();
    o.tree_builder.scripting_enabled = false;
    o
}

/// `new DOMParser().parseFromString(s, "text/html")`.
pub fn parse_html_document(s: &str) -> XmlDocument {
    let sink = Sink {
        doc: RefCell::new(XmlDocument::new()),
    };
    parse_document(sink, parse_opts()).one(s)
}

/// `element.innerHTML = markup` in an HTML document: the HTML fragment
/// parsing algorithm with `el` as the context, its children replaced by the
/// result.
pub fn set_inner_html(doc: &mut XmlDocument, el: NodeId, markup: &str) {
    let (ns, local) = {
        let e = doc.element(el).expect("element");
        (e.namespace.clone().unwrap_or_default(), e.local.clone())
    };
    let sink = Sink {
        doc: RefCell::new(XmlDocument::new()),
    };
    let ctx = QualName::new(
        None,
        Namespace::from(ns.as_str()),
        LocalName::from(local.as_str()),
    );
    let frag = parse_fragment(sink, parse_opts(), ctx, Vec::new(), false).one(markup);
    // The fragment's nodes are the children of its root <html> element.
    for c in doc.children(el).to_vec() {
        doc.detach(c);
    }
    let Some(root) = frag.document_element() else {
        return;
    };
    for c in frag.children(root).to_vec() {
        let n = doc.import_subtree(&frag, c);
        doc.append_child(el, n);
    }
}

/// `document.createElement(name)` in an HTML document: lowercased, in the
/// XHTML namespace.
pub fn create_html_element(doc: &mut XmlDocument, name: &str) -> NodeId {
    let n = doc.create_element(&name.to_ascii_lowercase());
    doc.set_element_namespace(n, Some(HTML_NS));
    n
}

/// Whether `n` is an element in the HTML namespace.
pub fn is_html_element(doc: &XmlDocument, n: NodeId) -> bool {
    doc.element(n)
        .is_some_and(|e| e.namespace.as_deref() == Some(HTML_NS))
}

/// `nodeName` in an HTML document: the qualified name uppercased for HTML
/// elements, `#text`, `#comment`, ... otherwise.
pub fn html_node_name(doc: &XmlDocument, n: NodeId) -> String {
    let name = doc.node_name(n.into());
    if is_html_element(doc, n) {
        name.to_ascii_uppercase()
    } else {
        name
    }
}

/// `nextElementSibling`.
pub fn next_element_sibling(doc: &XmlDocument, n: NodeId) -> Option<NodeId> {
    let mut s = doc.next_sibling(n);
    while let Some(x) = s {
        if doc.is_element(x) {
            return Some(x);
        }
        s = doc.next_sibling(x);
    }
    None
}

/// `classList.contains(token)`.
pub fn class_list_contains(doc: &XmlDocument, n: NodeId, token: &str) -> bool {
    doc.get_attribute(n, "class")
        .is_some_and(|c| c.split_ascii_whitespace().any(|t| t == token))
}

/// `element.outerHTML` in an HTML document (parse5 `serializeOuter`).
pub fn outer_html(doc: &XmlDocument, n: NodeId) -> String {
    let mut out = String::new();
    serialize_node(doc, n, &mut out);
    out
}

/// `element.innerHTML` (getter) in an HTML document.
pub fn inner_html(doc: &XmlDocument, n: NodeId) -> String {
    let mut out = String::new();
    if !is_void(doc, n) {
        for &c in doc.children(n) {
            serialize_node(doc, c, &mut out);
        }
    }
    out
}

const VOID: [&str; 18] = [
    "area", "base", "basefont", "bgsound", "br", "col", "embed", "frame", "hr", "img", "input",
    "keygen", "link", "meta", "param", "source", "track", "wbr",
];

fn is_void(doc: &XmlDocument, n: NodeId) -> bool {
    is_html_element(doc, n)
        && doc
            .element(n)
            .is_some_and(|e| VOID.contains(&e.local.as_str()))
}

fn serialize_node(doc: &XmlDocument, n: NodeId, out: &mut String) {
    match doc.kind(n) {
        NodeKind::Element(e) => {
            let tn = e.qualified_name();
            out.push('<');
            out.push_str(&tn);
            for a in &e.attrs {
                out.push(' ');
                match a.namespace.as_deref() {
                    None => {}
                    Some(XML_NS) => out.push_str("xml:"),
                    Some(XMLNS_NS) => {
                        if a.local != "xmlns" {
                            out.push_str("xmlns:");
                        }
                    }
                    Some(XLINK_NS) => out.push_str("xlink:"),
                    Some(_) => {
                        out.push_str(a.prefix.as_deref().unwrap_or("undefined"));
                        out.push(':');
                    }
                }
                out.push_str(&a.local);
                out.push_str("=\"");
                out.push_str(&escape_attribute(&a.value));
                out.push('"');
            }
            out.push('>');
            if !is_void(doc, n) {
                for &c in doc.children(n) {
                    serialize_node(doc, c, out);
                }
                out.push_str("</");
                out.push_str(&tn);
                out.push('>');
            }
        }
        NodeKind::Text(t) | NodeKind::CData(t) => {
            let raw = doc.parent(n).is_some_and(|p| {
                is_html_element(doc, p)
                    && doc.element(p).is_some_and(|e| {
                        matches!(
                            e.local.as_str(),
                            "style"
                                | "script"
                                | "xmp"
                                | "iframe"
                                | "noembed"
                                | "noframes"
                                | "plaintext"
                        )
                    })
            });
            if raw {
                out.push_str(t);
            } else {
                out.push_str(&escape_text(t));
            }
        }
        NodeKind::Comment(c) => {
            out.push_str("<!--");
            out.push_str(c);
            out.push_str("-->");
        }
        NodeKind::DocumentType { name, .. } => {
            out.push_str("<!DOCTYPE ");
            out.push_str(name);
            out.push('>');
        }
        NodeKind::Document => {
            for &c in doc.children(n) {
                serialize_node(doc, c, out);
            }
        }
        NodeKind::Pi { .. } => {}
    }
}

/// entities `escapeText`: `&`, `<`, `>`, U+00A0.
pub fn escape_text(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '\u{A0}' => o.push_str("&nbsp;"),
            _ => o.push(c),
        }
    }
    o
}

/// entities `escapeAttribute`: `"`, `&`, U+00A0.
pub fn escape_attribute(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '"' => o.push_str("&quot;"),
            '\u{A0}' => o.push_str("&nbsp;"),
            _ => o.push(c),
        }
    }
    o
}

// ------------------------------------------------------------ selectors

#[derive(Debug, Clone)]
enum Simple {
    Tag(String),
    Class(String),
    Attr(String, Option<String>),
    Not(Vec<Simple>),
}

fn parse_compound(s: &str) -> Vec<Simple> {
    let c: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let ident = |i: &mut usize| -> String {
        let st = *i;
        while *i < c.len() && (c[*i].is_alphanumeric() || c[*i] == '-' || c[*i] == '_') {
            *i += 1;
        }
        c[st..*i].iter().collect()
    };
    while i < c.len() {
        match c[i] {
            '*' => i += 1,
            '.' => {
                i += 1;
                out.push(Simple::Class(ident(&mut i)));
            }
            '[' => {
                let end = c[i..]
                    .iter()
                    .position(|&x| x == ']')
                    .map_or(c.len(), |p| i + p);
                let body: String = c[i + 1..end].iter().collect();
                match body.split_once('=') {
                    Some((k, v)) => out.push(Simple::Attr(
                        k.trim().to_owned(),
                        Some(v.trim().trim_matches(|x| x == '"' || x == '\'').to_owned()),
                    )),
                    None => out.push(Simple::Attr(body.trim().to_owned(), None)),
                }
                i = end + 1;
            }
            ':' => {
                // Only :not(<compound>).
                let rest: String = c[i..].iter().collect();
                if let Some(r) = rest.strip_prefix(":not(") {
                    let close = r.find(')').unwrap_or(r.len());
                    out.push(Simple::Not(parse_compound(&r[..close])));
                    i += 5 + close + 1;
                } else {
                    i += 1;
                }
            }
            _ => out.push(Simple::Tag(ident(&mut i))),
        }
    }
    out
}

fn matches_simple(doc: &XmlDocument, n: NodeId, s: &Simple) -> bool {
    let Some(e) = doc.element(n) else {
        return false;
    };
    match s {
        Simple::Tag(t) => {
            if is_html_element(doc, n) {
                e.local.eq_ignore_ascii_case(t)
            } else {
                e.local == *t
            }
        }
        Simple::Class(c) => class_list_contains(doc, n, c),
        Simple::Attr(k, v) => {
            let k = if is_html_element(doc, n) {
                k.to_ascii_lowercase()
            } else {
                k.clone()
            };
            match v {
                None => doc.has_attribute(n, &k),
                Some(v) => doc.get_attribute(n, &k) == Some(v.as_str()),
            }
        }
        Simple::Not(list) => !list.iter().all(|x| matches_simple(doc, n, x)),
    }
}

fn matches_complex(doc: &XmlDocument, n: NodeId, parts: &[Vec<Simple>]) -> bool {
    let (last, rest) = parts.split_last().expect("non-empty");
    if !last.iter().all(|s| matches_simple(doc, n, s)) {
        return false;
    }
    let mut need = rest.len();
    let mut cur = doc.parent(n);
    while need > 0 {
        let Some(p) = cur else {
            return false;
        };
        if rest[need - 1].iter().all(|s| matches_simple(doc, p, s)) {
            need -= 1;
        }
        cur = doc.parent(p);
    }
    true
}

/// `root.querySelectorAll(selectors)` for a selector list of compound
/// selectors (type, `.class`, `[attr]`, `[attr="v"]`, `:not(...)`) joined
/// by descendant combinators. Type selectors match HTML elements ASCII
/// case-insensitively. Results in tree order, `root` excluded.
pub fn query_selector_all(doc: &XmlDocument, root: NodeId, selectors: &str) -> Vec<NodeId> {
    let groups: Vec<Vec<Vec<Simple>>> = selectors
        .split(',')
        .map(|g| g.split_whitespace().map(parse_compound).collect())
        .filter(|g: &Vec<Vec<Simple>>| !g.is_empty())
        .collect();
    doc.descendants(root)
        .into_iter()
        .filter(|&d| doc.is_element(d) && groups.iter().any(|g| matches_complex(doc, d, g)))
        .collect()
}

// ------------------------------------------------------------ style

/// The declarations of a `style` attribute as jsdom's CSSStyleDeclaration
/// keeps them: lowercase-named `name: value` pairs, others dropped.
fn declarations(style: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for d in style.split(';') {
        let Some((k, v)) = d.split_once(':') else {
            continue;
        };
        let k = k.trim();
        let v = v.trim();
        if k.is_empty() || v.is_empty() || !k.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
            continue;
        }
        let v = normalise_value(v);
        match out.iter_mut().find(|(n, _)| n == k) {
            Some(x) => x.1 = v,
            None => out.push((k.to_owned(), v)),
        }
    }
    out
}

/// Colour normalisation: `#rgb` / `#rrggbb` as `rgb(r, g, b)`.
fn normalise_value(v: &str) -> String {
    if let Some(h) = v.strip_prefix('#') {
        if h.chars().all(|c| c.is_ascii_hexdigit()) && (h.len() == 3 || h.len() == 6) {
            let full: String = if h.len() == 3 {
                h.chars().flat_map(|c| [c, c]).collect()
            } else {
                h.to_owned()
            };
            let p = |i: usize| u8::from_str_radix(&full[i..i + 2], 16).unwrap_or(0);
            return format!("rgb({}, {}, {})", p(0), p(2), p(4));
        }
    }
    v.to_owned()
}

/// `element.style.<property>` (CSS property name, e.g. `padding-left`).
pub fn style_get(doc: &XmlDocument, n: NodeId, prop: &str) -> String {
    let style = doc.get_attribute(n, "style").unwrap_or("");
    declarations(style)
        .into_iter()
        .find(|(k, _)| k == prop)
        .map(|(_, v)| v)
        .unwrap_or_default()
}

/// `element.style.<property> = value` ("" removes it): the `style`
/// attribute rewritten as jsdom serialises it.
pub fn style_set(doc: &mut XmlDocument, n: NodeId, prop: &str, value: &str) {
    let style = doc.get_attribute(n, "style").unwrap_or("").to_owned();
    let mut decls = declarations(&style);
    if value.is_empty() {
        decls.retain(|(k, _)| k != prop);
    } else {
        match decls.iter_mut().find(|(k, _)| k == prop) {
            Some(x) => x.1 = normalise_value(value),
            None => decls.push((prop.to_owned(), normalise_value(value))),
        }
    }
    let text: Vec<String> = decls.iter().map(|(k, v)| format!("{k}: {v};")).collect();
    doc.set_attribute(n, "style", &text.join(" "));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Outputs recorded from jsdom 29.0.1 (node, 2026-10-07).
    #[test]
    fn matches_jsdom() {
        let mut d = parse_html_document(
            "<p style=\"padding-left: 40px\">a</p><p style=\"color: #ff0000;  PADDING-LEFT:10px ; bogus\">b</p><p>c</p>",
        );
        let body = query_selector_all(&d, d.document(), "body")[0];
        for p in query_selector_all(&d, d.document(), "p") {
            style_set(&mut d, p, "margin-left", "30px");
        }
        assert_eq!(
            inner_html(&d, body),
            "<p style=\"padding-left: 40px; margin-left: 30px;\">a</p><p style=\"color: rgb(255, 0, 0); margin-left: 30px;\">b</p><p style=\"margin-left: 30px;\">c</p>"
        );
        let div = create_html_element(&mut d, "DIV");
        d.set_attribute(div, "class", "x");
        set_inner_html(
            &mut d,
            div,
            "<p>a&nbsp;\"b\"</p><noscript><b>x</b></noscript><pre>\nq</pre><textarea>\nz</textarea>",
        );
        assert_eq!(
            outer_html(&d, div),
            "<div class=\"x\"><p>a&nbsp;\"b\"</p><noscript><b>x</b></noscript><pre>q</pre><textarea>z</textarea></div>"
        );
        assert_eq!(html_node_name(&d, div), "DIV");
        // The div is detached: not found from the document.
        assert_eq!(query_selector_all(&d, d.document(), "P").len(), 3);
        assert_eq!(
            query_selector_all(&d, div, "pre:not(.math), noscript b").len(),
            2
        );
    }
}
