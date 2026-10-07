// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translate, https://github.com/zotero/translate (commit
//   dd524aea9a55): src/rdf/serialize.js (Tim Berners-Lee 2006, from
//   http://dig.csail.mit.edu/2005/ajar/ajaw/js/rdf/serialize.js):
//   `suggestPrefix` :71-73, `makeUpPrefix` :83-125, `rootSubjects`
//   :132-278, `_notQNameChars`/`_NCNameRegExp` :285-291, `statementsToXML`
//   :610-879. W3C licence; see NOTICE.
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA;
//   (c) 2006 World Wide Web Consortium (MIT, ERCIM, Keio).
// Licence: AGPL-3.0 (upstream translate: AGPL-3.0-or-later; serialize.js:
//   W3C Software Notice and License, GPL-compatible). Changes (2026-10-07):
//   translated to Rust; the N3 serializer (`statementsToN3`) is not ported,
//   as no ported translator asks for `rdf/n3`.

//! A store as RDF/XML, byte for byte as upstream's `statementsToXML` writes
//! it: subjects in order of their first statement, each subject's
//! statements in insertion order (upstream's sort compares a statement with
//! itself and so never reorders), blank nodes with exactly one incoming arc
//! nested, the rest as `rdf:nodeID` roots, and upstream's line packing
//! (width 80, indent 4, lengths in UTF-16 code units).

use super::store::Store;
use super::term::{Term, RDF_NS};

/// `_notQNameChars + ":"` (serialize.js:285-286).
const NOT_NAME_CHARS: &str = "\t\r\n !\"#$%&'()*,+/;<=>?@[\\]^`{|}~:";

/// The property names a plain JavaScript object (`{}`) inherits: looking one
/// of them up finds a function, so `incoming[x] = incoming[x] || []` style
/// code misbehaves when a literal's value is one of these.
const OBJECT_PROTO: [&str; 12] = [
    "constructor",
    "__defineGetter__",
    "__defineSetter__",
    "hasOwnProperty",
    "__lookupGetter__",
    "__lookupSetter__",
    "isPrototypeOf",
    "propertyIsEnumerable",
    "toString",
    "valueOf",
    "__proto__",
    "toLocaleString",
];

/// What an array (`[]`) inherits on top of that: `makeUpPrefix`'s reverse
/// map is an array, so these prefixes count as taken.
const ARRAY_PROTO: [&str; 38] = [
    "constructor",
    "at",
    "concat",
    "copyWithin",
    "fill",
    "find",
    "findIndex",
    "findLast",
    "findLastIndex",
    "lastIndexOf",
    "pop",
    "push",
    "reverse",
    "shift",
    "unshift",
    "slice",
    "sort",
    "splice",
    "includes",
    "indexOf",
    "join",
    "keys",
    "entries",
    "values",
    "forEach",
    "filter",
    "flat",
    "flatMap",
    "map",
    "every",
    "some",
    "reduce",
    "reduceRight",
    "toLocaleString",
    "toString",
    "toReversed",
    "toSorted",
    "toSpliced",
];

/// A branch of upstream's nested list of strings.
#[derive(Debug, Clone)]
enum Branch {
    Str(String),
    Tree(Vec<Branch>),
}

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// `escapeForXML` (:684-690).
fn escape_for_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

struct Serializer<'a> {
    store: &'a Store,
    /// `prefixes`: namespace URI -> prefix, in insertion order.
    prefixes: Vec<(String, String)>,
    /// `namespaceCounts`: namespaces used, in order of first use.
    namespace_counts: Vec<String>,
    /// `incoming`: object `toString()` -> how many statements point at it.
    incoming: Vec<(String, usize)>,
    /// `subjects`: subject NT -> its statements, in insertion order.
    subjects: Vec<(String, Vec<usize>)>,
}

impl Serializer<'_> {
    fn suggest_prefix(&mut self, prefix: &str, uri: &str) {
        match self.prefixes.iter_mut().find(|(u, _)| u == uri) {
            Some((_, p)) => *p = prefix.to_owned(),
            None => self.prefixes.push((uri.to_owned(), prefix.to_owned())),
        }
    }

    fn prefix_for(&self, uri: &str) -> Option<&str> {
        self.prefixes
            .iter()
            .find(|(u, _)| u == uri)
            .map(|(_, p)| p.as_str())
            .filter(|p| !p.is_empty())
    }

    /// `makeUpPrefix(uri)` (:83-125).
    fn make_up_prefix(&mut self, uri: &str) -> String {
        // The reverse index, built once (canUse adds to `prefixes` but not
        // to it).
        let taken: Vec<String> = self.prefixes.iter().map(|(_, p)| p.clone()).collect();
        let can_use = |pp: &str| {
            !(taken.iter().any(|t| t == pp)
                || ARRAY_PROTO.contains(&pp)
                || OBJECT_PROTO.contains(&pp))
        };
        let chars: Vec<char> = uri.chars().collect();
        let mut end = chars.len();
        while end > 0 && NOT_NAME_CHARS.contains(chars[end - 1]) {
            end -= 1;
        }
        let p: Vec<char> = chars[..end].to_vec();
        let mut chosen: Option<String> = None;
        let base: String;
        if !p.is_empty() {
            let mut start = p.len();
            while start > 0 && !NOT_NAME_CHARS.contains(p[start - 1]) {
                start -= 1;
            }
            let p: Vec<char> = p[start..].to_vec();
            let s = |n: usize| -> String { p.iter().take(n).collect() };
            let whole: String = p.iter().collect();
            if p.len() < 6 && can_use(&whole) {
                chosen = Some(whole);
            } else {
                for n in [3, 2, 4, 1, 5] {
                    if can_use(&s(n)) {
                        chosen = Some(s(n));
                        break;
                    }
                }
            }
            base = s(3);
        } else {
            base = "ns".to_owned();
            if can_use("ns") {
                chosen = Some("ns".to_owned());
            }
        }
        let pok = match chosen {
            Some(c) => c,
            None => (0..)
                .map(|i| format!("{base}{i}"))
                .find(|c| can_use(c))
                .expect("unbounded"),
        };
        self.suggest_prefix(&pok, uri);
        pok
    }

    /// `qname(term)` (:845-863).
    fn qname(&mut self, uri: &str) -> Result<String, String> {
        let chars: Vec<char> = uri.chars().collect();
        let mut start = chars.len();
        while start > 0 && !NOT_NAME_CHARS.contains(chars[start - 1]) {
            start -= 1;
        }
        let j = (start..chars.len())
            .find(|&i| !(chars[i].is_ascii_digit() || chars[i] == '-' || chars[i] == '.'))
            .ok_or_else(|| format!("Cannot make qname out of <{uri}>"))?;
        let localid: String = chars[j..].iter().collect();
        let namesp: String = chars[..j].iter().collect();
        let prefix = match self.prefix_for(&namesp) {
            Some(p) => p.to_owned(),
            None => self.make_up_prefix(&namesp),
        };
        if !self.namespace_counts.contains(&namesp) {
            self.namespace_counts.push(namesp);
        }
        Ok(format!("{prefix}:{localid}"))
    }

    fn incoming_count(&self, key: &str) -> Option<usize> {
        self.incoming
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, n)| *n)
    }

    /// `rootSubjects(sts)` (:132-278): fills `incoming` and `subjects`,
    /// returns the roots (as NT strings).
    fn root_subjects(&mut self) -> Result<Vec<String>, String> {
        for (i, st) in self.store.statements.iter().enumerate() {
            let key = self.store.term_to_string(&st.object.term);
            if OBJECT_PROTO.contains(&key.as_str()) {
                // `incoming[x]` finds the inherited function, and
                // `incoming[x].push` is not a function.
                return Err(format!(
                    "TypeError: incoming[x].push is not a function ({key})"
                ));
            }
            match self.incoming.iter_mut().find(|(k, _)| *k == key) {
                Some((_, n)) => *n += 1,
                None => self.incoming.push((key, 1)),
            }
            let s = st.subject.term.to_nt();
            match self.subjects.iter_mut().find(|(k, _)| *k == s) {
                Some((_, v)) => v.push(i),
                None => self.subjects.push((s, vec![i])),
            }
        }
        let mut roots = Vec::new();
        for (nt, _) in &self.subjects {
            let is_bnode = nt.starts_with("_:");
            if !is_bnode || self.incoming_count(nt) != Some(1) {
                roots.push(nt.clone());
            }
        }
        Ok(roots)
    }

    /// `subjectXMLTree(subject, stats)` (:697-792).
    fn subject_tree(&mut self, subject_nt: &str, subject: &Term) -> Result<Vec<Branch>, String> {
        let sts = self
            .subjects
            .iter()
            .find(|(k, _)| k == subject_nt)
            .map(|(_, v)| v.clone())
            .ok_or_else(|| {
                "TypeError: Cannot read properties of undefined (reading 'sort')".to_owned()
            })?;
        let li_prefix = format!("{RDF_NS}_");
        let mut results: Vec<Branch> = Vec::new();
        let mut ty: Option<String> = None;
        for i in sts {
            let st = &self.store.statements[i];
            let pred = st.predicate.term.uri().unwrap_or("").to_owned();
            let object = st.object.term.clone();
            if pred == format!("{RDF_NS}type") && ty.is_none() {
                if let Term::Symbol(u) = &object {
                    ty = Some(u.clone());
                    continue;
                }
            }
            let mut pred_uri = pred;
            if let Some(number) = pred_uri.strip_prefix(&li_prefix) {
                if is_js_int_string(number) {
                    pred_uri = format!("{RDF_NS}li");
                }
            }
            let t = self.qname(&pred_uri)?;
            match &object {
                Term::BlankNode(_) => {
                    let nt = object.to_nt();
                    if self.incoming_count(&nt) == Some(1) {
                        let sub = self.subject_tree(&nt, &object)?;
                        results.push(Branch::Str(format!("<{t}>")));
                        results.push(Branch::Tree(sub));
                        results.push(Branch::Str(format!("</{t}>")));
                    } else {
                        results.push(Branch::Str(format!("<{t} rdf:nodeID=\"{}\"/>", &nt[2..])));
                    }
                }
                Term::Symbol(u) => results.push(Branch::Str(format!(
                    "<{t} rdf:resource=\"{}\"/>",
                    escape_for_xml(u)
                ))),
                Term::Literal { value, lang, .. } => {
                    // Upstream reads `st.object.dt`, which no literal has, so
                    // a datatype is never written.
                    let lang = lang
                        .as_ref()
                        .map(|l| format!(" xml:lang=\"{l}\""))
                        .unwrap_or_default();
                    results.push(Branch::Str(format!(
                        "<{t}{lang}>{}</{t}>",
                        escape_for_xml(value)
                    )));
                }
                Term::Collection(id) => {
                    results.push(Branch::Str(format!("<{t} rdf:parseType=\"Collection\">")));
                    let mut res = Vec::new();
                    for e in self.store.collection_elements(*id).to_vec() {
                        let nt = e.term.to_nt();
                        res.push(Branch::Tree(self.subject_tree(&nt, &e.term)?));
                    }
                    results.push(Branch::Tree(res));
                    results.push(Branch::Str(format!("</{t}>")));
                }
            }
        }
        let tag = match ty {
            Some(u) => self.qname(&u)?,
            None => "rdf:Description".to_owned(),
        };
        let attrs = match subject {
            Term::BlankNode(_) => {
                let nt = subject.to_nt();
                if self.incoming_count(&nt) != Some(1) {
                    format!(" rdf:nodeID=\"{}\"", &nt[2..])
                } else {
                    String::new()
                }
            }
            Term::Symbol(u) => format!(" rdf:about=\"{}\"", escape_for_xml(u)),
            // `relURI(subject)` of a term with no `uri`: escapeForXML(undefined).
            Term::Collection(_) | Term::Literal { .. } => {
                " rdf:about=\"@@@undefined@@@@\"".to_owned()
            }
        };
        Ok(vec![
            Branch::Str(format!("<{tag}{attrs}>")),
            Branch::Tree(results),
            Branch::Str(format!("</{tag}>")),
        ])
    }
}

/// `number == parseInt(number).toString()`: a canonical decimal integer.
fn is_js_int_string(s: &str) -> bool {
    let digits = s.strip_prefix('-').unwrap_or(s);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    if digits.len() > 1 && digits.starts_with('0') {
        return false;
    }
    if s == "-0" {
        // parseInt("-0") is -0, whose toString() is "0".
        return false;
    }
    // Beyond 2^53 the round trip through a double loses digits.
    digits.len() <= 15
}

/// `XMLtreeToLine` (:625-633).
fn tree_to_line(tree: &[Branch]) -> String {
    let mut s = String::new();
    for b in tree {
        match b {
            Branch::Str(x) => s.push_str(x),
            Branch::Tree(t) => s.push_str(&tree_to_line(t)),
        }
    }
    s
}

/// `XMLtreeToString` (:636-670).
fn tree_to_string(tree: &[Branch], level: i64) -> String {
    const INDENT: i64 = 4;
    const WIDTH: i64 = 80;
    let mut out = String::new();
    let mut last_length: i64 = 100_000;
    for b in tree {
        let mut branch_str: Option<String> = match b {
            Branch::Str(s) => Some(s.clone()),
            Branch::Tree(_) => None,
        };
        if let Branch::Tree(t) = b {
            let mut substr = tree_to_string(t, level + 1);
            if (utf16_len(&substr) as i64) < 10 * (WIDTH - INDENT * level)
                && !substr.contains("\"\"\"")
            {
                let line = tree_to_line(t);
                if (utf16_len(&line) as i64) < WIDTH - INDENT * level {
                    branch_str = Some(format!("   {line}"));
                    substr = String::new();
                }
            }
            if !substr.is_empty() {
                last_length = 10_000;
            }
            out.push_str(&substr);
        }
        if let Some(branch) = branch_str {
            if last_length < INDENT * level + 4 {
                out.pop();
                out.push(' ');
                out.push_str(&branch);
                out.push('\n');
                last_length += utf16_len(&branch) as i64 + 1;
            } else {
                let spaces = " ".repeat((INDENT * level).max(0) as usize);
                let line = format!("{spaces}{branch}");
                out.push_str(&line);
                out.push('\n');
                last_length = utf16_len(&line) as i64;
            }
        }
    }
    out
}

/// `Zotero.RDF.serialize()` (translate.js:3077-3090) in RDF/XML:
/// `Serializer(store)`, `suggestPrefix` for each of the store's namespaces,
/// `statementsToXML(store.statements)`.
pub fn serialize_xml(store: &Store) -> Result<String, String> {
    let mut sz = Serializer {
        store,
        prefixes: Vec::new(),
        namespace_counts: vec![RDF_NS.to_owned()],
        incoming: Vec::new(),
        subjects: Vec::new(),
    };
    for (prefix, uri) in &store.namespaces {
        sz.suggest_prefix(prefix, uri);
    }
    // statementListToXMLTree
    sz.suggest_prefix("rdf", RDF_NS);
    let roots = sz.root_subjects()?;
    let mut tree = Vec::new();
    for nt in roots {
        let term = if let Some(id) = nt.strip_prefix("_:n") {
            Term::BlankNode(id.parse().unwrap_or(0))
        } else {
            Term::Symbol(nt[1..nt.len() - 1].to_owned())
        };
        tree.push(Branch::Tree(sz.subject_tree(&nt, &term)?));
    }
    let mut head = String::from("<rdf:RDF");
    for ns in &sz.namespace_counts {
        let p = sz
            .prefixes
            .iter()
            .find(|(u, _)| u == ns)
            .map(|(_, p)| p.clone())
            .unwrap_or_else(|| "undefined".to_owned());
        head.push_str(&format!("\n xmlns:{p}=\"{}\"", escape_for_xml(ns)));
    }
    head.push('>');
    let tree2 = vec![
        Branch::Str(head),
        Branch::Tree(tree),
        Branch::Str("</rdf:RDF>".to_owned()),
    ];
    Ok(tree_to_string(&tree2, -1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int_strings() {
        assert!(is_js_int_string("1"));
        assert!(is_js_int_string("0"));
        assert!(!is_js_int_string("01"));
        assert!(!is_js_int_string("x"));
    }
}
