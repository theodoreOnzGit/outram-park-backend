// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: w3c-xmlserializer 5.0.0 (MIT,
//   https://github.com/jsdom/w3c-xmlserializer): lib/serialize.js
//   (`xmlSerialization`, `serializeElement`, `recordNamespaceInformation`,
//   `serializeText`, `serializeComment`, `serializeDocumentType`,
//   `serializeProcessingInstruction`, `serializeCDATASection`) and
//   lib/attributes.js (`serializeAttributes`, `serializeAttributeValue`,
//   `preferredPrefixString`, `generatePrefix`): the W3C DOM Parsing "XML
//   serialization" algorithm, which the translation-server's XMLSerializer
//   and jsdom's `innerHTML` (XML documents) call.
// Copyright (c) Sebastian Mayr and the w3c-xmlserializer contributors (MIT).
// Licence: AGPL-3.0 (this port; MIT permits it).

//! `XMLSerializer.serializeToString` and `innerHTML`, byte for byte as the
//! translation-server produces them.
//!
//! Not modelled: the `requireWellFormed` checks on XML names (xml-name-
//! validator) and the `Char` production of text; the translators' exports
//! never reach them. HTML-namespace void elements are modelled.

use super::xml::{NodeId, NodeKind, XmlDocument, HTML_NS, XMLNS_NS, XML_NS};

/// namespace -> prefix list, as the JS object `namespacePrefixMap`
/// (`null` is keyed as ""). `serializeElement` copies the map with
/// `{ ...prefixMap }`, a SHALLOW copy: the prefix arrays are shared between
/// an element's map and its ancestors', and `recordNamespaceInformation`
/// pushes into them. The lists therefore live in an arena ([`Lists`]) and a
/// map holds indices, so a copy shares them exactly as JavaScript does.
type PrefixMap = Vec<(String, usize)>;

/// The prefix arrays of one serialization.
type Lists = Vec<Vec<String>>;

fn map_get<'a>(m: &PrefixMap, lists: &'a Lists, ns: &str) -> Option<&'a Vec<String>> {
    m.iter().find(|(k, _)| k == ns).map(|(_, i)| &lists[*i])
}

/// `map[ns].push(p)`, or `map[ns] = [p]` when absent.
fn map_push(m: &mut PrefixMap, lists: &mut Lists, ns: &str, p: String) {
    match m.iter().find(|(k, _)| k == ns) {
        Some((_, i)) => lists[*i].push(p),
        None => {
            lists.push(vec![p]);
            m.push((ns.to_owned(), lists.len() - 1));
        }
    }
}

/// `map[ns] = [p]` (a new array).
fn map_set_new(m: &mut PrefixMap, lists: &mut Lists, ns: &str, p: String) {
    lists.push(vec![p]);
    let i = lists.len() - 1;
    match m.iter_mut().find(|(k, _)| k == ns) {
        Some(x) => x.1 = i,
        None => m.push((ns.to_owned(), i)),
    }
}

const VOID_ELEMENTS: [&str; 19] = [
    "area", "base", "basefont", "bgsound", "br", "col", "embed", "frame", "hr", "img", "input",
    "keygen", "link", "menuitem", "meta", "param", "source", "track", "wbr",
];

/// Serialize `node` (w3c-xmlserializer's module export): fresh prefix map
/// with `xml`, context namespace null, prefix index 1.
pub fn serialize(doc: &XmlDocument, node: NodeId, require_well_formed: bool) -> Result<String, String> {
    let mut st = State {
        lists: vec![vec!["xml".to_owned()]],
        idx: 1,
        rwf: require_well_formed,
    };
    let map: PrefixMap = vec![(XML_NS.to_owned(), 0)];
    let mut out = String::new();
    xml_serialization(doc, node, None, &map, &mut st, &mut out)?;
    Ok(out)
}

/// What one serialization shares: the prefix arrays, `refs.prefixIndex`,
/// `requireWellFormed`.
struct State {
    lists: Lists,
    idx: u32,
    rwf: bool,
}

fn xml_serialization(
    doc: &XmlDocument,
    node: NodeId,
    namespace: Option<&str>,
    prefix_map: &PrefixMap,
    st: &mut State,
    out: &mut String,
) -> Result<(), String> {
    let rwf = st.rwf;
    match doc.kind(node) {
        NodeKind::Element(_) => serialize_element(doc, node, namespace, prefix_map, st, out),
        NodeKind::Document => {
            if rwf && doc.document_element().is_none() {
                return Err("Failed to serialize XML: document does not have a document element.".into());
            }
            for &c in doc.children(node) {
                xml_serialization(doc, c, namespace, prefix_map, st, out)?;
            }
            Ok(())
        }
        NodeKind::Comment(data) => {
            if rwf && (data.contains("--") || data.ends_with('-')) {
                return Err("Failed to serialize XML: found hyphens in illegal places in comment node data.".into());
            }
            out.push_str("<!--");
            out.push_str(data);
            out.push_str("-->");
            Ok(())
        }
        NodeKind::Text(data) => {
            out.push_str(&escape_text(data));
            Ok(())
        }
        NodeKind::DocumentType {
            name,
            public_id,
            system_id,
        } => {
            out.push_str("<!DOCTYPE ");
            out.push_str(name);
            if !public_id.is_empty() {
                out.push_str(&format!(" PUBLIC \"{public_id}\""));
            } else if !system_id.is_empty() {
                out.push_str(" SYSTEM");
            }
            if !system_id.is_empty() {
                out.push_str(&format!(" \"{system_id}\""));
            }
            out.push('>');
            Ok(())
        }
        NodeKind::Pi { target, data } => {
            if rwf && (target.contains(':') || target.eq_ignore_ascii_case("xml")) {
                return Err("Failed to serialize XML: processing instruction node target is not well-formed.".into());
            }
            if rwf && data.contains("?>") {
                return Err("Failed to serialize XML: processing instruction node data is not well-formed.".into());
            }
            out.push_str(&format!("<?{target} {data}?>"));
            Ok(())
        }
        NodeKind::CData(data) => {
            out.push_str(&format!("<![CDATA[{data}]]>"));
            Ok(())
        }
    }
}

/// `serializeText`: `&`, `<`, `>` escaped.
pub fn escape_text(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// `serializeAttributeValue`.
pub fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\t', "&#x9;")
        .replace('\n', "&#xA;")
        .replace('\r', "&#xD;")
}

/// `preferredPrefixString`.
fn preferred_prefix(map: &PrefixMap, lists: &Lists, ns: &str, preferred: Option<&str>) -> Option<String> {
    let list = map_get(map, lists, ns)?;
    if let Some(p) = preferred {
        if list.iter().any(|x| x == p) {
            return Some(p.to_owned());
        }
    }
    list.last().cloned()
}

/// `generatePrefix`.
fn generate_prefix(map: &mut PrefixMap, st: &mut State, ns: &str) -> String {
    let p = format!("ns{}", st.idx);
    st.idx += 1;
    map_set_new(map, &mut st.lists, ns, p.clone());
    p
}

/// `recordNamespaceInformation`: the element's default-namespace
/// declaration value; prefix declarations recorded into `map` (pushing into
/// shared arrays, as upstream does) and `local` (prefix -> namespace).
fn record_namespace_information(
    doc: &XmlDocument,
    el: NodeId,
    map: &mut PrefixMap,
    lists: &mut Lists,
    local: &mut Vec<(String, String)>,
) -> Option<String> {
    let mut default_ns = None;
    for a in doc.attributes(el) {
        if a.namespace.as_deref() != Some(XMLNS_NS) {
            continue;
        }
        if a.prefix.is_none() {
            default_ns = Some(a.value.clone());
            continue;
        }
        let def = a.value.clone();
        if def == XML_NS {
            continue;
        }
        if map_get(map, lists, &def).is_some_and(|l| l.contains(&a.local)) {
            continue;
        }
        map_push(map, lists, &def, a.local.clone());
        match local.iter_mut().find(|(k, _)| *k == a.local) {
            Some(x) => x.1 = def,
            None => local.push((a.local.clone(), def)),
        }
    }
    default_ns
}

fn serialize_element(
    doc: &XmlDocument,
    node: NodeId,
    namespace: Option<&str>,
    prefix_map: &PrefixMap,
    st: &mut State,
    out: &mut String,
) -> Result<(), String> {
    let rwf = st.rwf;
    let el = doc.element(node).expect("element");
    let mut markup = String::from("<");
    let qualified_name;
    let mut ignore_ns_def_attr = false;
    let mut map = prefix_map.clone();
    let mut local_prefixes: Vec<(String, String)> = Vec::new();
    let local_default_ns =
        record_namespace_information(doc, node, &mut map, &mut st.lists, &mut local_prefixes);
    let mut inherited_ns: Option<String> = namespace.map(str::to_owned);
    let ns = el.namespace.clone();
    if inherited_ns == ns {
        if local_default_ns.is_some() {
            ignore_ns_def_attr = true;
        }
        qualified_name = if ns.as_deref() == Some(XML_NS) {
            format!("xml:{}", el.local)
        } else {
            el.local.clone()
        };
        markup.push_str(&qualified_name);
    } else {
        let mut prefix = el.prefix.clone();
        // A null namespace is keyed "" here (upstream: "null"; no prefix is
        // ever bound to either).
        let ns_key = ns.clone().unwrap_or_default();
        let mut candidate = preferred_prefix(&map, &st.lists, &ns_key, prefix.as_deref());
        if prefix.as_deref() == Some("xmlns") {
            if rwf {
                return Err("Failed to serialize XML: element nodes can't have a prefix of \"xmlns\".".into());
            }
            candidate = Some("xmlns".into());
        }
        if let Some(c) = candidate {
            qualified_name = format!("{c}:{}", el.local);
            if let Some(d) = &local_default_ns {
                if d != XML_NS {
                    inherited_ns = if d.is_empty() { None } else { Some(d.clone()) };
                }
            }
            markup.push_str(&qualified_name);
        } else if let Some(mut p) = prefix.take() {
            if local_prefixes.iter().any(|(k, _)| *k == p) {
                p = generate_prefix(&mut map, st, &ns_key);
            }
            map_push(&mut map, &mut st.lists, &ns_key, p.clone());
            qualified_name = format!("{p}:{}", el.local);
            markup.push_str(&format!(
                "{qualified_name} xmlns:{p}=\"{}\"",
                escape_attr(&ns_key)
            ));
            if let Some(d) = &local_default_ns {
                inherited_ns = if d.is_empty() { None } else { Some(d.clone()) };
            }
        } else if local_default_ns.is_none() || local_default_ns != ns {
            ignore_ns_def_attr = true;
            qualified_name = el.local.clone();
            inherited_ns = ns.clone();
            markup.push_str(&format!(
                "{qualified_name} xmlns=\"{}\"",
                escape_attr(ns.as_deref().unwrap_or(""))
            ));
        } else {
            qualified_name = el.local.clone();
            inherited_ns = ns.clone();
            markup.push_str(&qualified_name);
        }
    }

    markup.push_str(&serialize_attributes(
        doc,
        node,
        &mut map,
        &local_prefixes,
        ignore_ns_def_attr,
        st,
    )?);

    let children = doc.children(node);
    let is_html = ns.as_deref() == Some(HTML_NS);
    if is_html && children.is_empty() && VOID_ELEMENTS.contains(&el.local.as_str()) {
        markup.push_str(" />");
        out.push_str(&markup);
        return Ok(());
    }
    if !is_html && children.is_empty() {
        markup.push_str("/>");
        out.push_str(&markup);
        return Ok(());
    }
    markup.push('>');
    out.push_str(&markup);
    for &c in children {
        xml_serialization(doc, c, inherited_ns.as_deref(), &map, st, out)?;
    }
    out.push_str(&format!("</{qualified_name}>"));
    Ok(())
}

/// `serializeAttributes`.
fn serialize_attributes(
    doc: &XmlDocument,
    node: NodeId,
    map: &mut PrefixMap,
    local_prefixes: &[(String, String)],
    ignore_ns_def_attr: bool,
    st: &mut State,
) -> Result<String, String> {
    let rwf = st.rwf;
    let mut result = String::new();
    let mut seen: Vec<(Option<String>, String)> = Vec::new();
    for a in doc.attributes(node) {
        if rwf && seen.iter().any(|(n, l)| *n == a.namespace && *l == a.local) {
            return Err("Found duplicated attribute".into());
        }
        seen.push((a.namespace.clone(), a.local.clone()));
        let mut candidate: Option<String> = None;
        if let Some(ans) = &a.namespace {
            candidate = preferred_prefix(map, &st.lists, ans, a.prefix.as_deref());
            if ans == XMLNS_NS {
                let skip = a.value == XML_NS
                    || (a.prefix.is_none() && ignore_ns_def_attr)
                    || (a.prefix.is_some()
                        && local_prefixes
                            .iter()
                            .find(|(k, _)| *k == a.local)
                            .map(|(_, v)| v.as_str())
                            != Some(a.value.as_str())
                        && map_get(map, &st.lists, &a.value).is_some_and(|l| l.contains(&a.local)));
                if skip {
                    continue;
                }
                if rwf && a.value == XMLNS_NS {
                    return Err("The XMLNS namespace is reserved and cannot be applied as an element's namespace via XML parsing".into());
                }
                if rwf && a.value.is_empty() {
                    return Err("Namespace prefix declarations cannot be used to undeclare a namespace".into());
                }
                if a.prefix.as_deref() == Some("xmlns") {
                    candidate = Some("xmlns".into());
                }
            } else if candidate.is_none() {
                let p = generate_prefix(map, st, ans);
                result.push_str(&format!(" xmlns:{p}=\"{}\"", escape_attr(ans)));
                candidate = Some(p);
            }
        }
        result.push(' ');
        if let Some(c) = candidate {
            result.push_str(&c);
            result.push(':');
        }
        if rwf && (a.local.contains(':') || (a.local == "xmlns" && a.namespace.is_none())) {
            return Err("Invalid attribute localName value".into());
        }
        result.push_str(&format!("{}=\"{}\"", a.local, escape_attr(&a.value)));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use crate::zotero::framework::xml::XmlDocument;

    /// Outputs recorded from w3c-xmlserializer 5.0.0 under jsdom 29.0.1
    /// (2026-10-07).
    #[test]
    fn matches_w3c_xmlserializer() {
        let d = XmlDocument::parse(
            "<?xml version=\"1.0\"?><!DOCTYPE x [<!ENTITY foo \"bar&amp;\">]><m:mets xmlns:m=\"urn:m\" xmlns:xlink=\"urn:l\"><m:METS/><mets>&foo;</mets><m:fileSec><m:file><m:FLocat LOCTYPE=\"URL\" xlink:href=\"h\"/></m:file></m:fileSec><Mets/></m:mets>",
        )
        .unwrap();
        let root = d.document_element().unwrap();
        assert_eq!(
            d.inner_html(root).unwrap(),
            "<m:METS xmlns:m=\"urn:m\"/><mets>bar&amp;amp;</mets><m:fileSec xmlns:m=\"urn:m\"><m:file><m:FLocat LOCTYPE=\"URL\" xmlns:ns1=\"urn:l\" ns1:href=\"h\"/></m:file></m:fileSec><Mets/>"
        );
        assert_eq!(
            d.serialize(d.document()),
            "<!DOCTYPE x><m:mets xmlns:m=\"urn:m\" xmlns:xlink=\"urn:l\"><m:METS/><mets>bar&amp;amp;</mets><m:fileSec><m:file><m:FLocat LOCTYPE=\"URL\" xlink:href=\"h\"/></m:file></m:fileSec><Mets/></m:mets>"
        );
        let d2 = XmlDocument::parse(
            "<a xmlns=\"urn:x\"><b xmlns=\"\">t&lt;&gt;\"'</b><c:d xmlns:c=\"urn:c\" e=\"1&#10;\t&quot;\"/></a>",
        )
        .unwrap();
        let a = d2.document_element().unwrap();
        assert_eq!(
            d2.inner_html(a).unwrap(),
            "<b>t&lt;&gt;\"'</b><c:d xmlns:c=\"urn:c\" e=\"1&#xA; &quot;\"/>"
        );
        assert_eq!(
            d2.serialize(d2.document()),
            "<a xmlns=\"urn:x\"><b xmlns=\"\">t&lt;&gt;\"'</b><c:d xmlns:c=\"urn:c\" e=\"1&#xA; &quot;\"/></a>"
        );
    }
}
