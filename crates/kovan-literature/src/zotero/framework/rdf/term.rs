// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translate, https://github.com/zotero/translate (commit
//   dd524aea9a55, the submodule Zotero 9cbba8c4d281 pins):
//   src/rdf/term.js (the RDF term classes: `Symbol` :35-46, `BlankNode`
//   :56-75, `Literal` :78-113, `Collection` :115-148, `sameTerm` :424-460,
//   `compareTerm` :470-498). term.js is "W3C open source licence 2005"
//   (Tim Berners-Lee's AJAW RDF library); see this crate's NOTICE.
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA;
//   (c) 2005 World Wide Web Consortium (MIT, ERCIM, Keio).
// Licence: AGPL-3.0 (upstream translate: AGPL-3.0-or-later; term.js: W3C
//   Software Notice and License, GPL-compatible).

//! RDF terms as upstream's AJAW library represents them.
//!
//! JavaScript gives every term object an identity; the translators compare
//! by identity in one place (`RDF.js` `detectType`, `processedParts.includes`).
//! A [`Node`] therefore carries `inst`, the identity of the JavaScript object
//! it stands for: each `sym()` call upstream makes a new object, while a blank
//! node or a collection is one object per id. Equality of terms
//! ([`Term::same_term`]) ignores `inst`, as upstream's `sameTerm` does.

use std::cmp::Ordering;

/// The RDF namespace.
pub const RDF_NS: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";

/// An RDF term (upstream's `termType`s: symbol, bnode, literal, collection).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Term {
    /// `Symbol(uri)`: a named resource.
    Symbol(String),
    /// `BlankNode`, by its `id` (upstream's global counter `Term.NextId`).
    BlankNode(u64),
    /// `Literal(value, lang, datatype)`.
    Literal {
        /// `value`.
        value: String,
        /// `lang` (`undefined` when empty).
        lang: Option<String>,
        /// `datatype`, a symbol's URI.
        datatype: Option<String>,
    },
    /// `Collection` (`rdf:parseType="Collection"`), by its `id`; its elements
    /// live in the store.
    Collection(u64),
}

impl Term {
    /// A literal with neither language nor datatype.
    pub fn literal(value: impl Into<String>) -> Term {
        Term::Literal {
            value: value.into(),
            lang: None,
            datatype: None,
        }
    }

    /// `toNT()`, which is also upstream's `hashString()` (identity.js:31-34).
    pub fn to_nt(&self) -> String {
        match self {
            Term::Symbol(u) => format!("<{u}>"),
            Term::BlankNode(id) | Term::Collection(id) => format!("_:n{id}"),
            Term::Literal {
                value,
                lang,
                datatype,
            } => {
                let mut s = String::with_capacity(value.len() + 2);
                s.push('"');
                for c in value.chars() {
                    match c {
                        '\\' => s.push_str("\\\\"),
                        '"' => s.push_str("\\\""),
                        '\n' => s.push_str("\\n"),
                        c => s.push(c),
                    }
                }
                s.push('"');
                if let Some(dt) = datatype {
                    s.push_str("^^<");
                    s.push_str(dt);
                    s.push('>');
                }
                if let Some(l) = lang {
                    s.push('@');
                    s.push_str(l);
                }
                s
            }
        }
    }

    /// `toString()` for every term but a collection (whose `toString` lists
    /// its elements; see [`super::store::Store::term_to_string`]): a symbol is
    /// `<uri>`, a blank node `_:nID`, a literal its value.
    pub fn to_js_string_simple(&self) -> String {
        match self {
            Term::Literal { value, .. } => value.clone(),
            other => other.to_nt(),
        }
    }

    /// `termType`.
    pub fn term_type(&self) -> &'static str {
        match self {
            Term::Symbol(_) => "symbol",
            Term::BlankNode(_) => "bnode",
            Term::Literal { .. } => "literal",
            Term::Collection(_) => "collection",
        }
    }

    /// `sameTerm` (term.js:424-460).
    pub fn same_term(&self, other: &Term) -> bool {
        self == other
    }

    /// The symbol's URI (`term.uri`), if a symbol.
    pub fn uri(&self) -> Option<&str> {
        match self {
            Term::Symbol(u) => Some(u),
            _ => None,
        }
    }

    /// `classOrder` (term.js:465-469).
    fn class_order(&self) -> u8 {
        match self {
            Term::Literal { .. } => 1,
            Term::Collection(_) => 3,
            Term::Symbol(_) => 5,
            Term::BlankNode(_) => 6,
        }
    }

    /// `compareTerm` (term.js:472-498): class order, then value / URI (as
    /// JavaScript compares strings: UTF-16 code units) / id.
    pub fn compare_term(&self, other: &Term) -> Ordering {
        let c = self.class_order().cmp(&other.class_order());
        if c != Ordering::Equal {
            return c;
        }
        match (self, other) {
            (Term::Literal { value: a, .. }, Term::Literal { value: b, .. })
            | (Term::Symbol(a), Term::Symbol(b)) => a.encode_utf16().cmp(b.encode_utf16()),
            (Term::BlankNode(a), Term::BlankNode(b))
            | (Term::Collection(a), Term::Collection(b)) => a.cmp(b),
            // The collection's compareTerm is the blank node's; across the
            // two classes the class order already decided.
            _ => Ordering::Equal,
        }
    }
}

/// A term as a JavaScript object: the term and the identity of the object
/// (module docs).
#[derive(Debug, Clone)]
pub struct Node {
    /// The term.
    pub term: Term,
    /// The JavaScript object's identity.
    pub inst: u64,
}

impl Node {
    /// Whether two nodes are the same JavaScript object (`===`).
    pub fn same_object(&self, other: &Node) -> bool {
        match (&self.term, &other.term) {
            (Term::BlankNode(a), Term::BlankNode(b))
            | (Term::Collection(a), Term::Collection(b)) => a == b,
            _ => self.inst == other.inst,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nt_forms() {
        assert_eq!(Term::Symbol("x".into()).to_nt(), "<x>");
        assert_eq!(Term::BlankNode(7).to_nt(), "_:n7");
        let l = Term::Literal {
            value: "a\"b\\c\nd".into(),
            lang: Some("en".into()),
            datatype: Some("dt".into()),
        };
        assert_eq!(l.to_nt(), "\"a\\\"b\\\\c\\nd\"^^<dt>@en");
    }

    #[test]
    fn compare_orders_class_then_value() {
        let a = Term::Symbol("http://a".into());
        let b = Term::Symbol("http://b".into());
        assert_eq!(a.compare_term(&b), Ordering::Less);
        assert_eq!(Term::literal("z").compare_term(&a), Ordering::Less);
        assert_eq!(Term::BlankNode(1).compare_term(&a), Ordering::Greater);
    }
}
