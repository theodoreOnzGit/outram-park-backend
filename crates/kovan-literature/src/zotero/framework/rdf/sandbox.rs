// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translate, https://github.com/zotero/translate (commit
//   dd524aea9a55): src/translation/translate.js
//   `Zotero.Translate.IO._RDFSandbox` :3032-3330 (the `Zotero.RDF` object
//   translators call) and `IO.String._initRDF` :2919-2934.
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! `Zotero.RDF`: the API RDF translators use, over a [`Store`].
//!
//! JavaScript values cross this API loosely typed; here a resource argument
//! is a [`Res`] (a URI string, a term object, or one of the non-objects
//! translators sometimes pass: `undefined`, `false`, `null`) and a value
//! coming back is an [`RdfValue`] (a literal comes back as a string, any
//! other term as the term).
//!
//! As upstream, a string argument becomes a new symbol (`_getResource`), and
//! `undefined` or `false` become the symbols `<undefined>` / `<false>`
//! (`new Symbol(undefined)`): they find nothing unless a document names a
//! resource "undefined" or "false", where upstream's `uri` comparison
//! (`undefined == "undefined"` is false) would still find nothing on two-part
//! patterns. That corner is not reproduced. `null` is a wildcard, as in
//! upstream's `statementsMatching`.

use super::dom::Dom;
use super::parser;
use super::serializer::serialize_xml;
use super::store::Store;
use super::term::{Node, Term, RDF_NS};
use std::collections::HashMap;

/// A resource argument (see the module docs).
#[derive(Debug, Clone)]
pub enum Res {
    /// A URI string: becomes a new symbol.
    Uri(String),
    /// A term object.
    Node(Node),
    /// `undefined`.
    Undefined,
    /// `false`.
    False,
    /// `null`.
    Null,
}

impl From<&str> for Res {
    fn from(s: &str) -> Res {
        Res::Uri(s.to_owned())
    }
}

impl From<String> for Res {
    fn from(s: String) -> Res {
        Res::Uri(s)
    }
}

impl From<&String> for Res {
    fn from(s: &String) -> Res {
        Res::Uri(s.clone())
    }
}

impl From<Node> for Res {
    fn from(n: Node) -> Res {
        Res::Node(n)
    }
}

impl From<&Node> for Res {
    fn from(n: &Node) -> Res {
        Res::Node(n.clone())
    }
}

impl From<&RdfValue> for Res {
    fn from(v: &RdfValue) -> Res {
        match v {
            RdfValue::Str(s) => Res::Uri(s.clone()),
            RdfValue::Node(n) => Res::Node(n.clone()),
        }
    }
}

/// A value returned to a translator: a literal as a string, any other term
/// as an object.
#[derive(Debug, Clone)]
pub enum RdfValue {
    /// A string (a literal's value).
    Str(String),
    /// A term object.
    Node(Node),
}

impl RdfValue {
    /// The string, if a string (`typeof x == "string"`).
    pub fn as_str(&self) -> Option<&str> {
        match self {
            RdfValue::Str(s) => Some(s),
            RdfValue::Node(_) => None,
        }
    }

    /// The node, if an object.
    pub fn as_node(&self) -> Option<&Node> {
        match self {
            RdfValue::Node(n) => Some(n),
            RdfValue::Str(_) => None,
        }
    }
}

/// A statement as `getStatementsMatching` returns it: `[subject, predicate,
/// object]`, the object a string when a literal.
pub type Triple = (Node, Node, RdfValue);

/// `Zotero.RDF` (module docs).
#[derive(Debug, Clone)]
pub struct RdfSandbox {
    /// The data store (`_dataStore`).
    pub store: Store,
    /// `_containerCounts`: the next `rdf:_n` of each container (`None` is
    /// `NaN`: an element added to a container never made by `newContainer`).
    container_counts: HashMap<String, Option<i64>>,
}

impl RdfSandbox {
    /// An empty store whose blank nodes are numbered from `first_id`.
    pub fn new(first_id: u64) -> Self {
        RdfSandbox {
            store: Store::with_first_id(first_id),
            container_counts: HashMap::new(),
        }
    }

    /// `_initRDF` on `text`: parse it as XML (`parseDOMXML`), then as RDF/XML
    /// into a new store, with base URI `""` (the translation-server's
    /// `IO.String` gets no path). Empty input gives an empty store.
    pub fn from_xml(text: &str, first_id: u64) -> Result<Self, String> {
        let mut s = RdfSandbox::new(first_id);
        if !text.is_empty() {
            let mut dom = Dom::parse(text)?;
            parser::parse(&mut dom, &mut s.store, "")?;
        }
        Ok(s)
    }

    /// `_getResource(about)`.
    fn get_resource(&mut self, r: &Res) -> Option<Node> {
        match r {
            Res::Node(n) => Some(n.clone()),
            Res::Uri(u) => Some(self.store.sym(u.clone())),
            Res::Undefined => Some(self.store.sym("undefined")),
            Res::False => Some(self.store.sym("false")),
            Res::Null => None,
        }
    }

    /// `serialize()` (RDF/XML).
    pub fn serialize(&self) -> Result<String, String> {
        serialize_xml(&self.store)
    }

    /// `addStatement(about, relation, value, false)`: a resource object.
    pub fn add_statement(
        &mut self,
        about: impl Into<Res>,
        relation: impl Into<Res>,
        value: impl Into<Res>,
    ) -> Result<(), String> {
        let (about, relation, value) = (about.into(), relation.into(), value.into());
        for (r, name) in [
            (&about, "about"),
            (&relation, "relation"),
            (&value, "value"),
        ] {
            if matches!(r, Res::Undefined | Res::Null) {
                return Err(format!("{name} must be defined in Zotero.RDF.addStatement"));
            }
        }
        let v = self.get_resource(&value).expect("checked");
        let s = self.get_resource(&about).expect("checked");
        let p = self.get_resource(&relation).expect("checked");
        self.store.add(s, p, v);
        Ok(())
    }

    /// `addStatement(about, relation, value, true)`: a literal, with the
    /// control characters Mozilla would mangle removed.
    pub fn add_literal(
        &mut self,
        about: impl Into<Res>,
        relation: impl Into<Res>,
        value: &str,
    ) -> Result<(), String> {
        let (about, relation) = (about.into(), relation.into());
        for (r, name) in [(&about, "about"), (&relation, "relation")] {
            if matches!(r, Res::Undefined | Res::Null) {
                return Err(format!("{name} must be defined in Zotero.RDF.addStatement"));
            }
        }
        let cleaned: String = value
            .chars()
            .filter(|c| !matches!(c, '\u{0}'..='\u{8}' | '\u{B}' | '\u{C}' | '\u{E}'..='\u{1F}'))
            .collect();
        let s = self.get_resource(&about).expect("checked");
        let p = self.get_resource(&relation).expect("checked");
        let o = self.store.literal(cleaned, None, None);
        self.store.add(s, p, o);
        Ok(())
    }

    /// `newResource()`: a new blank node.
    pub fn new_resource(&mut self) -> Node {
        self.store.bnode()
    }

    /// `newContainer(type, about)`.
    pub fn new_container(&mut self, ty: &str, about: impl Into<Res>) -> Result<Node, String> {
        let t = match ty.to_lowercase().as_str() {
            "bag" => "Bag",
            "seq" => "Seq",
            "alt" => "Alt",
            _ => return Err("Invalid container type in Zotero.RDF.newContainer".to_owned()),
        };
        let about = self
            .get_resource(&about.into())
            .ok_or("about must be defined in Zotero.RDF.addStatement")?;
        self.add_statement(
            about.clone(),
            format!("{RDF_NS}type"),
            format!("{RDF_NS}{t}"),
        )?;
        self.container_counts.insert(about.term.to_nt(), Some(1));
        Ok(about)
    }

    /// `addContainerElement(about, element, false)`.
    pub fn add_container_element(
        &mut self,
        about: impl Into<Res>,
        element: Node,
    ) -> Result<(), String> {
        let about = self
            .get_resource(&about.into())
            .ok_or("TypeError: Cannot read properties of null (reading 'toNT')")?;
        let key = about.term.to_nt();
        let n = self.container_counts.get(&key).copied().flatten();
        self.container_counts.insert(key, n.map(|n| n + 1));
        let pred = match n {
            Some(n) => format!("{RDF_NS}_{n}"),
            None => format!("{RDF_NS}_NaN"),
        };
        let p = self.store.sym(pred);
        self.store.add(about, p, element);
        Ok(())
    }

    fn value_of(&self, n: &Node) -> RdfValue {
        match &n.term {
            Term::Literal { value, .. } => RdfValue::Str(value.clone()),
            _ => RdfValue::Node(n.clone()),
        }
    }

    /// `getContainerElements(about)`: the `rdf:_n` members by `n`, as a
    /// JavaScript array may hold them: `None` is a hole.
    pub fn get_container_elements(&mut self, about: impl Into<Res>) -> Vec<Option<RdfValue>> {
        let li = format!("{RDF_NS}_");
        let Some(about) = self.get_resource(&about.into()) else {
            return Vec::new();
        };
        let mut out: Vec<Option<RdfValue>> = Vec::new();
        for st in self
            .store
            .statements_matching(Some(&about), None, None, false)
        {
            let s = &self.store.statements[st];
            let Some(number) = s.predicate.term.uri().and_then(|u| u.strip_prefix(&li)) else {
                continue;
            };
            let Some(n) = js_int(number) else { continue };
            if n < 1 {
                // containerElements[-1] etc.: a property, not an element.
                continue;
            }
            let i = (n - 1) as usize;
            if out.len() <= i {
                out.resize(i + 1, None);
            }
            out[i] = Some(self.value_of(&s.object));
        }
        out
    }

    /// `addNamespace(prefix, uri)`.
    pub fn add_namespace(&mut self, prefix: &str, uri: &str) {
        self.store.set_prefix_for_uri(prefix, uri);
    }

    /// `getResourceURI(resource)` (:3209-3222): a URI string, a blank node's
    /// `rdf:value` (which may itself be an object), or its NT form.
    pub fn get_resource_uri(&mut self, r: &RdfValue) -> RdfValue {
        let n = match r {
            RdfValue::Str(s) => return RdfValue::Str(s.clone()),
            RdfValue::Node(n) => n.clone(),
        };
        if let Some(u) = n.term.uri() {
            if !u.is_empty() {
                return RdfValue::Str(u.to_owned());
            }
        }
        if let Some(v) = self.get_statements_matching(
            &Res::Node(n.clone()),
            &Res::Uri(format!("{RDF_NS}value")),
            &Res::Null,
            false,
        ) {
            return v[0].2.clone();
        }
        RdfValue::Str(n.term.to_nt())
    }

    /// [`RdfSandbox::get_resource_uri`] when the caller only uses a string
    /// (an object answer is converted with `toString()`).
    pub fn get_resource_uri_string(&mut self, r: &RdfValue) -> String {
        match self.get_resource_uri(r) {
            RdfValue::Str(s) => s,
            RdfValue::Node(n) => self.store.term_to_string(&n.term),
        }
    }

    /// `getAllResources()`: the subject of the first statement under each
    /// subject-index key, in key order.
    pub fn get_all_resources(&self) -> Vec<Node> {
        self.store
            .subject_index
            .entries()
            .map(|(_, l)| self.store.statements[l[0]].subject.clone())
            .collect()
    }

    /// The index key upstream's `getArcsIn`/`getArcsOut` use: the canonical
    /// term as a property name, i.e. its `toString()` (for a collection that
    /// lists its elements, which never matches an index key).
    fn arc_key(&mut self, r: &Res) -> Option<String> {
        let n = self.get_resource(r)?;
        let c = self.store.canon(&n);
        Some(self.store.term_to_string(&c.term))
    }

    /// `getArcsIn(resource)`: the predicates of the statements pointing at
    /// it (`None` is `false`).
    pub fn get_arcs_in(&mut self, r: impl Into<Res>) -> Option<Vec<String>> {
        let key = self.arc_key(&r.into())?;
        let list = self.store.object_index.get(&key)?;
        Some(
            list.iter()
                .map(|&i| {
                    self.store.statements[i]
                        .predicate
                        .term
                        .uri()
                        .unwrap_or("")
                        .to_owned()
                })
                .collect(),
        )
    }

    /// `getArcsOut(resource)`.
    pub fn get_arcs_out(&mut self, r: impl Into<Res>) -> Option<Vec<String>> {
        let key = self.arc_key(&r.into())?;
        let list = self.store.subject_index.get(&key)?;
        Some(
            list.iter()
                .map(|&i| {
                    self.store.statements[i]
                        .predicate
                        .term
                        .uri()
                        .unwrap_or("")
                        .to_owned()
                })
                .collect(),
        )
    }

    /// `getSources(resource, property)`: subjects (`None` is `false`).
    pub fn get_sources(
        &mut self,
        r: impl Into<Res>,
        property: impl Into<Res>,
    ) -> Option<Vec<Node>> {
        let o = self.get_resource(&r.into());
        let p = self.get_resource(&property.into());
        let m = self
            .store
            .statements_matching(None, p.as_ref(), o.as_ref(), false);
        if m.is_empty() {
            return None;
        }
        Some(
            m.iter()
                .map(|&i| self.store.statements[i].subject.clone())
                .collect(),
        )
    }

    /// `getTargets(resource, property)`: objects, literals as strings
    /// (`None` is `false`).
    pub fn get_targets(
        &mut self,
        r: impl Into<Res>,
        property: impl Into<Res>,
    ) -> Option<Vec<RdfValue>> {
        let s = self.get_resource(&r.into());
        let p = self.get_resource(&property.into());
        let m = self
            .store
            .statements_matching(s.as_ref(), p.as_ref(), None, false);
        if m.is_empty() {
            return None;
        }
        Some(
            m.iter()
                .map(|&i| self.value_of(&self.store.statements[i].object))
                .collect(),
        )
    }

    /// `getStatementsMatching(subj, pred, obj, objLiteral, justOne)`
    /// (:3315-3329). A falsy subject, predicate or object is a wildcard; with
    /// `objLiteral` true and an object given, upstream passes the boolean
    /// `true` as the object pattern, which becomes the literal `"1"`
    /// (`RDFMakeTerm(true)`), and that is reproduced. `None` is `false`.
    pub fn get_statements_matching(
        &mut self,
        subj: &Res,
        pred: &Res,
        obj: &Res,
        obj_literal: bool,
        // `justOne`
    ) -> Option<Vec<Triple>> {
        self.statements_matching_impl(subj, pred, obj, obj_literal, false)
    }

    /// [`RdfSandbox::get_statements_matching`] with `justOne`.
    pub fn get_statements_matching_one(
        &mut self,
        subj: &Res,
        pred: &Res,
        obj: &Res,
        obj_literal: bool,
    ) -> Option<Vec<Triple>> {
        self.statements_matching_impl(subj, pred, obj, obj_literal, true)
    }

    fn statements_matching_impl(
        &mut self,
        subj: &Res,
        pred: &Res,
        obj: &Res,
        obj_literal: bool,
        just_one: bool,
    ) -> Option<Vec<Triple>> {
        let falsy = |r: &Res| {
            matches!(r, Res::Undefined | Res::False | Res::Null)
                || matches!(r, Res::Uri(u) if u.is_empty())
        };
        let s = if falsy(subj) {
            None
        } else {
            self.get_resource(subj)
        };
        let p = if falsy(pred) {
            None
        } else {
            self.get_resource(pred)
        };
        let o = if falsy(obj) {
            None
        } else if obj_literal {
            Some(
                self.store
                    .literal("1", None, Some("http://www.w3.org/2001/XMLSchema#boolean")),
            )
        } else {
            self.get_resource(obj)
        };
        let m = self
            .store
            .statements_matching(s.as_ref(), p.as_ref(), o.as_ref(), just_one);
        if m.is_empty() {
            return None;
        }
        Some(
            m.iter()
                .map(|&i| {
                    let st = &self.store.statements[i];
                    (
                        st.subject.clone(),
                        st.predicate.clone(),
                        self.value_of(&st.object),
                    )
                })
                .collect(),
        )
    }
}

/// `number == parseInt(number).toString()`: the integer, when `number` is
/// its canonical decimal form.
fn js_int(s: &str) -> Option<i64> {
    let digits = s.strip_prefix('-').unwrap_or(s);
    if digits.is_empty() || digits.len() > 15 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if (digits.len() > 1 && digits.starts_with('0')) || s == "-0" {
        return None;
    }
    s.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn containers_number_their_members() {
        let mut r = RdfSandbox::new(0);
        let b = r.new_resource();
        let c = r.new_container("seq", b.clone()).unwrap();
        let p1 = r.new_resource();
        let p2 = r.new_resource();
        r.add_container_element(c.clone(), p1).unwrap();
        r.add_container_element(c.clone(), p2).unwrap();
        let els = r.get_container_elements(c);
        assert_eq!(els.len(), 2);
        assert!(matches!(&els[1], Some(RdfValue::Node(n)) if n.term == Term::BlankNode(2)));
    }

    #[test]
    fn targets_and_resource_uris() {
        let mut r = RdfSandbox::new(0);
        r.add_literal("#a", "urn:p", "v\u{1}x").unwrap();
        let t = r.get_targets("#a", "urn:p").unwrap();
        assert_eq!(t[0].as_str(), Some("vx"));
        assert!(r.get_targets("#a", "urn:q").is_none());
        let b = r.new_resource();
        r.add_literal(b.clone(), format!("{RDF_NS}value"), "http://x")
            .unwrap();
        assert_eq!(
            r.get_resource_uri(&RdfValue::Node(b)).as_str(),
            Some("http://x")
        );
        assert_eq!(r.get_arcs_out("#a"), Some(vec!["urn:p".to_owned()]));
    }
}
