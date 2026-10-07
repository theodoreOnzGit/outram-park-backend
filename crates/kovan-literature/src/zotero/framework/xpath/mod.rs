// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: wicked-good-xpath 1.3.1-z002 (Zotero's build; MIT,
//   https://github.com/google/wicked-good-xpath; Copyright (c) 2007 Cybozu
//   Labs, Inc., Copyright (c) 2012 Google Inc.), the XPath engine the
//   translation-server installs on jsdom (src/translation/translate.js :86-88
//   `wgxpath.install(dom.window, true)`). Ported from its compiled
//   dist/wgxpath.install-node.js: the lexer (`Ca`, the token regex `Da`),
//   parser (`xb`, `yb`, `Cb`, `Db`, `Eb`, `Bb`), node tests (`F` name test,
//   `G` kind test), axes (`U(...)`), the descendant search (`D`/`La`/`Na`,
//   via getElementsByTagName(NS)), the attribute-predicate shortcut (`o`,
//   `C`), predicates (`ab`), comparisons (`P`), the core functions (`R(...)`)
//   and string values (`A`). Zotero utilities (commit 4051881d59c6)
//   utilities.js `xpath` :1347-1420 and `xpathText` :1434-1452.
// Copyright (c) 2007 Cybozu Labs, Inc.; (c) 2012 Google Inc. (MIT);
//   Corporation for Digital Scholarship (utilities, AGPL-3.0-or-later).
// Licence: AGPL-3.0 (this port; MIT permits it).

//! `ZU.xpath` and `ZU.xpathText`: XPath 1.0 as wicked-good-xpath evaluates
//! it over jsdom, quirks included, because translators were written (and
//! their test cases recorded) against exactly this engine.
//!
//! Quirks that differ from the XPath 1.0 recommendation and are kept:
//!
//! * An **unprefixed name test** matches elements in the document's default
//!   namespace (`document.lookupNamespaceURI(null)`, i.e. the root element's
//!   default namespace), not elements in no namespace. So `crossref/journal`
//!   finds elements in the Crossref namespace when that is the default.
//! * `//name` (child axis, no positional predicate) searches with
//!   `getElementsByTagName(name)`, which matches the **qualified** name: an
//!   element written `x:name` is not found by `//name` even when `x` is
//!   bound to the default namespace (the child axis would find it).
//! * The attribute axis matches by `getNamedItem(qualifiedName)` (or
//!   `getNamedItemNS` for a prefixed test) and ignores the default
//!   namespace; `@*` includes `xmlns` attributes.
//! * `text()` matches Text nodes only, not CDATA sections.
//! * A step whose first predicate is a bare `[@name]` (child-like axes) tests
//!   it as `!!getAttribute(localName)`: an empty attribute does not count.
//! * `number()` of an empty node-set is 0; comparing a node-set with a
//!   boolean converts each node's string value to a boolean.
//! * `namespace-uri()` is always "" and `lang()` always false.
//! * The string value of the document node concatenates the text of every
//!   non-element node under it, comments and PIs included.
//!
//! Strings index `char`s where JavaScript indexes UTF-16 units (see
//! [`super::js`]); the two differ only on astral-plane characters.

mod eval;
mod parse;

use super::context::TranslateError;
use super::xml::{XNode, XmlDocument};
use eval::{Ctx, Eval, Value};
pub use eval::{js_number_to_string, js_to_number};
use parse::compile;

/// An XPath error (upstream throws; `ZU.xpath` rethrows it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XPathError(pub String);

impl From<XPathError> for TranslateError {
    fn from(e: XPathError) -> Self {
        TranslateError::Translator(e.0)
    }
}

pub(crate) type R<T> = Result<T, XPathError>;

pub(crate) fn err<T>(m: impl Into<String>) -> R<T> {
    Err(XPathError(m.into()))
}

// ---------------------------------------------------------------- ZU API

/// `document.evaluate(expr, context, resolver, ORDERED_NODE_ITERATOR_TYPE)`
/// on one context node: the nodes, in document order.
pub fn evaluate(
    doc: &XmlDocument,
    context: XNode,
    expr: &str,
    ns: &[(&str, &str)],
) -> Result<Vec<XNode>, XPathError> {
    let e = compile(expr, ns)?;
    let ev = Eval {
        default_ns: doc.lookup_namespace_uri(doc.document(), None),
    };
    match ev.eval(doc, &e, Ctx::of(context))? {
        Value::Set(s) => Ok(s),
        _ => err("value could not be converted to the specified type"),
    }
}

/// `ZU.xpath(element, xpath, namespaces)` on one node (`namespaces`: prefix
/// and URI pairs; an empty slice is no resolver).
pub fn xpath(
    doc: &XmlDocument,
    node: impl Into<XNode>,
    expr: &str,
    ns: &[(&str, &str)],
) -> Result<Vec<XNode>, TranslateError> {
    Ok(evaluate(doc, node.into(), expr, ns)?)
}

/// `ZU.xpath(elements, xpath, namespaces)` on several nodes: each node's
/// results in turn (not merged).
pub fn xpath_all(
    doc: &XmlDocument,
    nodes: &[XNode],
    expr: &str,
    ns: &[(&str, &str)],
) -> Result<Vec<XNode>, TranslateError> {
    let mut out = Vec::new();
    for &n in nodes {
        out.extend(evaluate(doc, n, expr, ns)?);
    }
    Ok(out)
}

/// `ZU.xpathText(node, xpath, namespaces, delimiter)`: the matches' values
/// (an attribute's value, else `textContent`) joined by `delimiter`
/// (default ", "), `None` (null) when nothing matches.
pub fn xpath_text(
    doc: &XmlDocument,
    node: impl Into<XNode>,
    expr: &str,
    ns: &[(&str, &str)],
    delimiter: Option<&str>,
) -> Result<Option<String>, TranslateError> {
    let found = xpath(doc, node, expr, ns)?;
    Ok(join_text(doc, &found, delimiter))
}

/// [`xpath_text`] over several context nodes.
pub fn xpath_text_all(
    doc: &XmlDocument,
    nodes: &[XNode],
    expr: &str,
    ns: &[(&str, &str)],
    delimiter: Option<&str>,
) -> Result<Option<String>, TranslateError> {
    let found = xpath_all(doc, nodes, expr, ns)?;
    Ok(join_text(doc, &found, delimiter))
}

fn join_text(doc: &XmlDocument, found: &[XNode], delimiter: Option<&str>) -> Option<String> {
    if found.is_empty() {
        return None;
    }
    let parts: Vec<String> = found
        .iter()
        .map(|&x| doc.text_content(x).unwrap_or_default())
        .collect();
    Some(parts.join(delimiter.unwrap_or(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(d: &XmlDocument, v: &[XNode]) -> Vec<String> {
        v.iter()
            .map(|&x| format!("{}={}", d.node_name(x), d.text(x)))
            .collect()
    }

    /// Results recorded from wicked-good-xpath 1.3.1-z002 on jsdom 29.0.1
    /// (node, 2026-10-07).
    #[test]
    fn matches_wgxpath_on_namespaces() {
        let d = XmlDocument::parse(
            r#"<a xmlns="urn:x" xmlns:p="urn:p" id="1" p:q="2"><b id="3">1</b><p:b>2</p:b><c xmlns="">3</c><B>4</B><!--c--><?pi x?><![CDATA[cd]]>t</a>"#,
        )
        .unwrap();
        let root = XNode::Node(d.document());
        let x = |e: &str, ns: &[(&str, &str)]| names(&d, &evaluate(&d, root, e, ns).unwrap());
        assert_eq!(x("//b", &[]), ["b=1"]);
        assert!(x("//c", &[]).is_empty());
        assert_eq!(x("//p:b", &[("p", "urn:p")]), ["p:b=2"]);
        assert_eq!(x("//*", &[]), ["a=1234cdt", "b=1", "p:b=2", "c=3", "B=4"]);
        assert_eq!(x("//q:b", &[("q", "urn:x")]), ["b=1"]);
        assert_eq!(x("/a/b", &[]), ["b=1"]);
        assert_eq!(x("//@id", &[]), ["id=1", "id=3"]);
        assert_eq!(
            x("/a/@*", &[]),
            ["xmlns=urn:x", "xmlns:p=urn:p", "id=1", "p:q=2"]
        );
        assert_eq!(x("/a/text()", &[]), ["#text=t"]);
        assert_eq!(x("/a/node()", &[]).len(), 8);
    }

    #[test]
    fn predicates_unions_and_functions() {
        let d = XmlDocument::parse(
            r#"<r><x t="a">1</x><x t="">2</x><x>3</x><y><x t="b">4</x></y></r>"#,
        )
        .unwrap();
        let root = XNode::Node(d.document());
        let x = |e: &str| names(&d, &evaluate(&d, root, e, &[]).unwrap());
        assert_eq!(x("//x[1]"), ["x=1", "x=4"]);
        assert_eq!(x("(//x)[1]"), ["x=1"]);
        // The [@name] shortcut: an empty attribute does not count.
        assert_eq!(x("//x[@t]"), ["x=1", "x=4"]);
        assert_eq!(x("//x[not(@t)]"), ["x=3"]);
        assert_eq!(x("//x[@t='b'] | /r/x[last()]"), ["x=3", "x=4"]);
        assert_eq!(
            x("//x[starts-with(text(), '2') or contains(@t, 'a')]"),
            ["x=1", "x=2"]
        );
        assert_eq!(x("//y/x/.."), ["y=4"]);
        assert_eq!(x("/r/x[position() > 1]"), ["x=2", "x=3"]);
        assert!(evaluate(&d, root, "//p:x", &[]).is_err());
    }

    #[test]
    fn js_numbers() {
        assert_eq!(js_number_to_string(1.0), "1");
        assert_eq!(js_number_to_string(0.5), "0.5");
        assert_eq!(js_number_to_string(1e21), "1e+21");
        assert_eq!(js_number_to_string(1e-7), "1e-7");
        assert_eq!(js_number_to_string(123.456), "123.456");
        assert!(js_to_number("abc").is_nan());
        assert_eq!(js_to_number(" 12 "), 12.0);
        assert_eq!(js_to_number(""), 0.0);
        assert_eq!(js_to_number("0x1f"), 31.0);
    }
}
