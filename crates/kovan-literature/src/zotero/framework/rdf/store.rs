// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translate, https://github.com/zotero/translate (commit
//   dd524aea9a55): src/rdf/identity.js (`IndexedFormula`: the constructor
//   and its property/class actions :21-112, `newPropertyAction` :118-130,
//   `setPrefixForURI` :132-139, `equate` :152-166, `replaceWith` :170-215,
//   `canon` :218-223, `add` :282-321, `statementsMatching` :350-411) and
//   src/rdf/term.js (`Formula.prototype.any` :531-540, `Collection`
//   :115-148). identity.js: Tim Berners-Lee 2005, W3C licence; see NOTICE.
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA;
//   (c) 2005 World Wide Web Consortium (MIT, ERCIM, Keio).
// Licence: AGPL-3.0 (upstream translate: AGPL-3.0-or-later; identity.js:
//   W3C Software Notice and License, GPL-compatible).

//! The triple store (`IndexedFormula`): statements in insertion order,
//! indexed by subject, predicate and object, with upstream's "smushing" of
//! nodes declared identical (`owl:sameAs`, inverse-functional and functional
//! properties).
//!
//! Upstream keys its indexes by `hashString()` in JavaScript objects, whose
//! string keys iterate in insertion order; [`OrderedIndex`] keeps that order,
//! which `Zotero.RDF.getAllResources` exposes. The provenance (`why`) index
//! is not kept: nothing a translator can call reads it.

use super::term::{Node, Term};
use std::collections::HashMap;

/// `http://www.w3.org/2002/07/owl#`.
const OWL_NS: &str = "http://www.w3.org/2002/07/owl#";

/// A triple.
#[derive(Debug, Clone)]
pub struct Statement {
    /// `subject`.
    pub subject: Node,
    /// `predicate`.
    pub predicate: Node,
    /// `object`.
    pub object: Node,
}

/// A JavaScript object used as a map from hash strings to lists, keys in
/// insertion order (none of the keys is integer-like: they start with `<`,
/// `_` or `"`).
#[derive(Debug, Clone, Default)]
pub struct OrderedIndex {
    order: Vec<Option<String>>,
    map: HashMap<String, (usize, Vec<usize>)>,
}

impl OrderedIndex {
    /// `ix[h]`.
    pub fn get(&self, h: &str) -> Option<&Vec<usize>> {
        self.map.get(h).map(|(_, v)| v)
    }

    /// `if (ix[h] == undefined) ix[h] = []; ix[h].push(st)`.
    fn push(&mut self, h: &str, st: usize) {
        if let Some((_, v)) = self.map.get_mut(h) {
            v.push(st);
        } else {
            self.order.push(Some(h.to_owned()));
            self.map
                .insert(h.to_owned(), (self.order.len() - 1, vec![st]));
        }
    }

    /// `moveIndex` inside `replaceWith` (identity.js:175-185).
    fn move_key(&mut self, old: &str, new: &str) {
        let Some((pos, oldlist)) = self.map.remove(old) else {
            return;
        };
        self.order[pos] = None;
        match self.map.get_mut(new) {
            None => {
                self.order.push(Some(new.to_owned()));
                self.map
                    .insert(new.to_owned(), (self.order.len() - 1, oldlist));
            }
            Some((_, newlist)) => {
                let mut l = oldlist;
                l.extend_from_slice(newlist);
                *newlist = l;
            }
        }
    }

    /// The keys in iteration order with their lists.
    pub fn entries(&self) -> impl Iterator<Item = (&str, &Vec<usize>)> {
        self.order
            .iter()
            .flatten()
            .map(|k| (k.as_str(), &self.map[k].1))
    }
}

/// An action run when a statement is added (`propertyActions`,
/// `classActions`): upstream stores closures; these are the four it has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    /// `handleRDFType` (on `rdf:type`).
    RdfType,
    /// `owl:sameAs` -> `equate`.
    SameAs,
    /// `{X rdf:type owl:InverseFunctionalProperty}` -> watch X with `handle_IFP`.
    DeclareIfp,
    /// `{X rdf:type owl:FunctionalProperty}` -> watch X with `handle_FP`.
    DeclareFp,
    /// `handle_IFP`.
    Ifp,
    /// `handle_FP`.
    Fp,
}

/// Action lists keyed by hash string (`propertyActions`, `classActions`).
#[derive(Debug, Clone, Default)]
struct Actions(HashMap<String, Vec<Action>>);

impl Actions {
    fn move_key(&mut self, old: &str, new: &str) {
        let Some(oldlist) = self.0.remove(old) else {
            return;
        };
        match self.0.get_mut(new) {
            None => {
                self.0.insert(new.to_owned(), oldlist);
            }
            Some(newlist) => {
                let mut l = oldlist;
                l.extend_from_slice(newlist);
                *newlist = l;
            }
        }
    }
}

/// Which position of a statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// `subject`.
    Subject,
    /// `predicate`.
    Predicate,
    /// `object`.
    Object,
}

/// `IndexedFormula` (module docs).
#[derive(Debug, Clone, Default)]
pub struct Store {
    /// `statements`, in insertion order.
    pub statements: Vec<Statement>,
    /// `subjectIndex`.
    pub subject_index: OrderedIndex,
    /// `predicateIndex`.
    pub predicate_index: OrderedIndex,
    /// `objectIndex`.
    pub object_index: OrderedIndex,
    redirections: HashMap<String, Node>,
    aliases: HashMap<String, Vec<Node>>,
    property_actions: Actions,
    class_actions: Actions,
    /// `namespaces`: prefix -> URI, in insertion order.
    pub namespaces: Vec<(String, String)>,
    collections: HashMap<u64, Vec<Node>>,
    /// The next blank-node / collection id (upstream's global
    /// `Term.NextId`; here per store, see [`Store::with_first_id`]).
    next_id: u64,
    next_inst: u64,
}

impl Store {
    /// `new IndexedFormula()` with the default features (sameAs,
    /// InverseFunctionalProperty, FunctionalProperty), blank-node ids from 0.
    pub fn new() -> Self {
        Self::with_first_id(0)
    }

    /// A store whose blank nodes are numbered from `first`. Upstream's
    /// counter is global to the process, so its ids depend on everything the
    /// process did before; only their order is meaningful.
    pub fn with_first_id(first: u64) -> Self {
        let mut s = Store {
            next_id: first,
            ..Default::default()
        };
        s.property_actions.0.insert(
            format!("<{}type>", super::term::RDF_NS),
            vec![Action::RdfType],
        );
        s.property_actions
            .0
            .insert(format!("<{OWL_NS}sameAs>"), vec![Action::SameAs]);
        s.class_actions.0.insert(
            format!("<{OWL_NS}InverseFunctionalProperty>"),
            vec![Action::DeclareIfp],
        );
        s.class_actions.0.insert(
            format!("<{OWL_NS}FunctionalProperty>"),
            vec![Action::DeclareFp],
        );
        s
    }

    fn new_inst(&mut self) -> u64 {
        self.next_inst += 1;
        self.next_inst
    }

    /// `sym(uri)`: a new symbol object.
    pub fn sym(&mut self, uri: impl Into<String>) -> Node {
        Node {
            term: Term::Symbol(uri.into()),
            inst: self.new_inst(),
        }
    }

    /// `literal(value, lang, datatype)` (an empty `lang` is none).
    pub fn literal(
        &mut self,
        value: impl Into<String>,
        lang: Option<&str>,
        datatype: Option<&str>,
    ) -> Node {
        Node {
            term: Term::Literal {
                value: value.into(),
                lang: lang.filter(|l| !l.is_empty()).map(str::to_owned),
                datatype: datatype.map(str::to_owned),
            },
            inst: self.new_inst(),
        }
    }

    /// `bnode()` / `new BlankNode()`: the next id.
    pub fn bnode(&mut self) -> Node {
        let id = self.next_id;
        self.next_id += 1;
        Node {
            term: Term::BlankNode(id),
            inst: self.new_inst(),
        }
    }

    /// `collection()` / `new Collection()`.
    pub fn collection(&mut self) -> Node {
        let id = self.next_id;
        self.next_id += 1;
        self.collections.insert(id, Vec::new());
        Node {
            term: Term::Collection(id),
            inst: self.new_inst(),
        }
    }

    /// `collection.append(el)`.
    pub fn collection_append(&mut self, id: u64, el: Node) {
        self.collections.entry(id).or_default().push(el);
    }

    /// The elements of a collection.
    pub fn collection_elements(&self, id: u64) -> &[Node] {
        self.collections.get(&id).map_or(&[], Vec::as_slice)
    }

    /// A term's JavaScript `toString()` (a collection lists its elements:
    /// `(a b )`, term.js:136-142).
    pub fn term_to_string(&self, t: &Term) -> String {
        match t {
            Term::Collection(id) => {
                let mut s = String::from("(");
                for e in self.collection_elements(*id) {
                    s.push_str(&self.term_to_string(&e.term));
                    s.push(' ');
                }
                s.push(')');
                s
            }
            other => other.to_js_string_simple(),
        }
    }

    /// `setPrefixForURI(prefix, uri)` (identity.js:132-139).
    pub fn set_prefix_for_uri(&mut self, prefix: &str, uri: &str) {
        if prefix == "tab" && self.namespaces.iter().any(|(p, _)| p == "tab") {
            return;
        }
        match self.namespaces.iter_mut().find(|(p, _)| p == prefix) {
            Some((_, u)) => *u = uri.to_owned(),
            None => self.namespaces.push((prefix.to_owned(), uri.to_owned())),
        }
    }

    /// `canon(term)` (identity.js:218-223).
    pub fn canon(&self, n: &Node) -> Node {
        self.redirections
            .get(&n.term.to_nt())
            .cloned()
            .unwrap_or_else(|| n.clone())
    }

    fn canon_hash(&self, n: &Node) -> String {
        match self.redirections.get(&n.term.to_nt()) {
            Some(r) => r.term.to_nt(),
            None => n.term.to_nt(),
        }
    }

    /// `add(subj, pred, obj)` (identity.js:282-321): run the predicate's
    /// actions, then index and append the statement (always: upstream adds it
    /// even when an action reports it unneeded). Returns its index.
    pub fn add(&mut self, subject: Node, predicate: Node, object: Node) -> usize {
        let ph = self.canon_hash(&predicate);
        let mut done = false;
        let mut i = 0;
        // `for (i < actions.length)`: the list is re-read every pass, as an
        // action can append to it.
        while let Some(a) = self
            .property_actions
            .0
            .get(&ph)
            .and_then(|v| v.get(i))
            .copied()
        {
            if !done {
                done = self.run_action(a, &subject, &predicate, &object);
            }
            i += 1;
        }
        let st = self.statements.len();
        let sh = self.canon_hash(&subject);
        let oh = self.canon_hash(&object);
        self.subject_index.push(&sh, st);
        self.predicate_index.push(&ph, st);
        self.object_index.push(&oh, st);
        self.statements.push(Statement {
            subject,
            predicate,
            object,
        });
        st
    }

    fn run_action(&mut self, a: Action, subj: &Node, pred: &Node, obj: &Node) -> bool {
        match a {
            Action::RdfType => {
                // handleRDFType: classActions[obj.hashString()] (not canon).
                let h = obj.term.to_nt();
                let mut done = false;
                let mut i = 0;
                while let Some(c) = self.class_actions.0.get(&h).and_then(|v| v.get(i)).copied() {
                    if !done {
                        done = self.run_action(c, subj, pred, obj);
                    }
                    i += 1;
                }
                done
            }
            Action::SameAs => {
                self.equate(subj, obj);
                true
            }
            Action::DeclareIfp => self.new_property_action(subj, Action::Ifp),
            Action::DeclareFp => self.new_property_action(subj, Action::Fp),
            Action::Ifp => {
                // handle_IFP: s1 = any(undefined, pred, obj)
                let m = self.statements_matching(None, Some(pred), Some(obj), true);
                let Some(&s) = m.first() else { return false };
                let s1 = self.statements[s].subject.clone();
                self.equate(&s1, subj);
                true
            }
            Action::Fp => {
                let m = self.statements_matching(Some(subj), Some(pred), None, true);
                let Some(&s) = m.first() else { return false };
                let o1 = self.statements[s].object.clone();
                self.equate(&o1, obj);
                true
            }
        }
    }

    /// `newPropertyAction(pred, action)` (identity.js:118-130).
    fn new_property_action(&mut self, pred: &Node, action: Action) -> bool {
        let hash = pred.term.to_nt();
        self.property_actions
            .0
            .entry(hash)
            .or_default()
            .push(action);
        let to_fix = self.statements_matching(None, Some(pred), None, false);
        let mut done = false;
        for st in to_fix {
            if done {
                break;
            }
            let (s, o) = (
                self.statements[st].subject.clone(),
                self.statements[st].object.clone(),
            );
            done = self.run_action(action, &s, pred, &o);
        }
        done
    }

    /// `equate(u1, u2)` (identity.js:152-166): replace the bigger term with the
    /// smaller.
    pub fn equate(&mut self, u1: &Node, u2: &Node) {
        let u1 = self.canon(u1);
        let u2 = self.canon(u2);
        match u1.term.compare_term(&u2.term) {
            std::cmp::Ordering::Equal => {}
            std::cmp::Ordering::Less => self.replace_with(&u2, &u1),
            std::cmp::Ordering::Greater => self.replace_with(&u1, &u2),
        }
    }

    /// `replaceWith(big, small)` (identity.js:170-215).
    fn replace_with(&mut self, big: &Node, small: &Node) {
        let oldhash = big.term.to_nt();
        let newhash = small.term.to_nt();
        self.subject_index.move_key(&oldhash, &newhash);
        self.predicate_index.move_key(&oldhash, &newhash);
        self.object_index.move_key(&oldhash, &newhash);
        self.redirections.insert(oldhash.clone(), small.clone());
        if big.term.uri().is_some_and(|u| !u.is_empty()) {
            self.aliases
                .entry(newhash.clone())
                .or_default()
                .push(big.clone());
            if let Some(old) = self.aliases.get(&oldhash).cloned() {
                for a in old {
                    self.redirections.insert(a.term.to_nt(), small.clone());
                    self.aliases.entry(newhash.clone()).or_default().push(a);
                }
            }
        }
        self.class_actions.move_key(&oldhash, &newhash);
        self.property_actions.move_key(&oldhash, &newhash);
    }

    fn index(&self, p: Part) -> &OrderedIndex {
        match p {
            Part::Subject => &self.subject_index,
            Part::Predicate => &self.predicate_index,
            Part::Object => &self.object_index,
        }
    }

    /// The node of statement `st` at `p`.
    pub fn part(&self, st: usize, p: Part) -> &Node {
        let s = &self.statements[st];
        match p {
            Part::Subject => &s.subject,
            Part::Predicate => &s.predicate,
            Part::Object => &s.object,
        }
    }

    /// `statementsMatching(subj, pred, obj, undefined, justOne)`
    /// (identity.js:350-411): the indices of the matching statements, in the
    /// order of the shortest index list (insertion order within it).
    pub fn statements_matching(
        &self,
        subject: Option<&Node>,
        predicate: Option<&Node>,
        object: Option<&Node>,
        just_one: bool,
    ) -> Vec<usize> {
        let pat = [
            (Part::Subject, subject),
            (Part::Predicate, predicate),
            (Part::Object, object),
        ];
        let given: Vec<(Part, Node, String)> = pat
            .iter()
            .filter_map(|(p, n)| {
                n.map(|n| {
                    let c = self.canon(n);
                    let h = c.term.to_nt();
                    (*p, c, h)
                })
            })
            .collect();
        if given.is_empty() {
            return (0..self.statements.len()).collect();
        }
        if given.len() == 1 {
            let (p, _, h) = &given[0];
            let Some(list) = self.index(*p).get(h) else {
                return Vec::new();
            };
            if just_one && list.len() > 1 {
                return list[..1].to_vec();
            }
            return list.clone();
        }
        let mut best = usize::MAX;
        let mut best_i = 0;
        for (i, (p, _, h)) in given.iter().enumerate() {
            let Some(list) = self.index(*p).get(h) else {
                return Vec::new();
            };
            if list.len() < best {
                best = list.len();
                best_i = i;
            }
        }
        let (bp, _, bh) = &given[best_i];
        let possibles = self.index(*bp).get(bh).cloned().unwrap_or_default();
        let mut results = Vec::new();
        for st in possibles {
            let ok = given.iter().enumerate().all(|(i, (p, pattern, _))| {
                i == best_i || self.canon(self.part(st, *p)).term.same_term(&pattern.term)
            });
            if ok {
                results.push(st);
                if just_one {
                    break;
                }
            }
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_as_smushes_to_the_smaller_uri_and_keeps_the_statement() {
        let mut s = Store::new();
        let a = s.sym("http://a");
        let p = s.sym("http://p");
        let lit = s.literal("x", None, None);
        s.add(a.clone(), p.clone(), lit);
        let b = s.sym("http://b");
        let same = s.sym(format!("{OWL_NS}sameAs"));
        s.add(b.clone(), same, a.clone());
        // b is now a: its statements are found under a.
        let m = s.statements_matching(Some(&b), Some(&p), None, false);
        assert_eq!(m, vec![0]);
        assert_eq!(s.statements.len(), 2);
        let keys: Vec<&str> = s.subject_index.entries().map(|(k, _)| k).collect();
        assert_eq!(keys, ["<http://a>"]);
    }

    #[test]
    fn matching_picks_the_shortest_list() {
        let mut s = Store::new();
        let x = s.sym("x");
        let p = s.sym("p");
        let q = s.sym("q");
        let o1 = s.literal("1", None, None);
        let o2 = s.literal("2", None, None);
        s.add(x.clone(), p.clone(), o1);
        s.add(x.clone(), q.clone(), o2);
        assert_eq!(
            s.statements_matching(Some(&x), Some(&q), None, false),
            vec![1]
        );
        assert_eq!(s.statements_matching(Some(&x), None, None, true), vec![0]);
    }
}
