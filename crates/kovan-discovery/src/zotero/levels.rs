// Part of the kovan Zotero port (GitHub #747, #751).
//
// Upstream: Zotero, https://github.com/zotero/zotero (commit 9cbba8c4d281):
// chrome/content/zotero/xpcom/data/search.js:2111-2543
// (combineConditions, _isAncestorLevel, _levelDistance,
// _closestRelatedLevel, mapPredicate, _matchingItemsSQL, _rollUpAnyToLevel).
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA
// (search.js: (c) 2009 Center for History and New Media, George Mason
// University). Licence: AGPL-3.0. See this crate's NOTICE, "Upstream: Zotero".

//! Cross-level mapping and the condition-group tree, over row sets.
//!
//! Upstream rewrites SQL: each predicate is `itemID [NOT] IN (...)` and the
//! mapping wraps it in sub-selects over `itemAttachments`, `itemNotes` and
//! `itemAnnotations`. Here a predicate is the set of rows it is true for, and
//! each sub-select is the same set operation on the rows. A predicate
//! reduced to the constant `'0'` is the empty set.

use super::conditions::{Level, Operator};
use super::library::{RowSet, SearchLibrary};

/// The rows of the table that holds `level` (`_levelChildTable`,
/// search.js:2239).
fn table_rows(lib: &SearchLibrary, level: Level) -> Vec<usize> {
    match level {
        Level::Attachment => lib.attachments().collect(),
        Level::Note => lib.item_notes().collect(),
        Level::Annotation => lib.annotations().collect(),
        Level::Item | Level::Any => Vec::new(),
    }
}

/// `parentItemID` of row `i` in the table of `level`.
fn parent_in_table(lib: &SearchLibrary, level: Level, i: usize) -> Option<usize> {
    let r = &lib.rows[i];
    match level {
        Level::Note => r.note_parent,
        _ => r.parent,
    }
}

/// `_isAncestorLevel` (search.js:2256).
pub fn is_ancestor_level(anc: Level, desc: Level) -> bool {
    let mut l = desc;
    while l != Level::Item {
        match l.parent() {
            None => return false,
            Some(p) => {
                l = p;
                if l == anc {
                    return true;
                }
            }
        }
    }
    false
}

/// `_levelDistance` (search.js:2274).
fn level_distance(a: Level, b: Level) -> Option<usize> {
    let walk = |from: Level, to: Level| {
        let mut l = Some(from);
        let mut d = 0;
        while let Some(x) = l {
            if x == to {
                return Some(d);
            }
            l = x.parent();
            d += 1;
        }
        None
    };
    walk(a, b).or_else(|| walk(b, a))
}

/// `_closestRelatedLevel` (search.js:2297).
fn closest_related_level(levels: &[Level], to: Level) -> Option<Level> {
    let mut best = None;
    let mut best_d = usize::MAX;
    for &l in levels {
        if l == Level::Any {
            continue;
        }
        if let Some(d) = level_distance(l, to) {
            if d < best_d {
                best = Some(l);
                best_d = d;
            }
        }
    }
    best
}

/// `_matchingItemsSQL` (search.js:2492): the rows of a predicate, without
/// trashed items when `exclude_deleted`.
fn matching(lib: &SearchLibrary, set: &RowSet, exclude_deleted: bool) -> RowSet {
    if exclude_deleted {
        let d = lib.deleted_items();
        set.difference(&d).copied().collect()
    } else {
        set.clone()
    }
}

/// `_rollUpAnyToLevel` (search.js:2512).
fn roll_up_any(lib: &SearchLibrary, set: &RowSet, to: Level, exclude_deleted: bool) -> RowSet {
    let m = matching(lib, set, exclude_deleted);
    match to {
        Level::Annotation => m
            .into_iter()
            .filter(|&i| lib.rows[i].is_annotation())
            .collect(),
        Level::Note => m
            .into_iter()
            .filter(|&i| lib.rows[i].in_item_notes)
            .collect(),
        Level::Attachment => m
            .into_iter()
            .filter_map(|i| {
                let r = &lib.rows[i];
                if r.is_attachment() {
                    Some(i)
                } else if r.is_annotation() {
                    r.parent
                } else {
                    None
                }
            })
            .collect(),
        Level::Item | Level::Any => m
            .into_iter()
            .map(|i| {
                let r = &lib.rows[i];
                // COALESCE(aAtt.parentItemID, annot.parentItemID,
                //          att.parentItemID, note.parentItemID, m.itemID)
                if r.is_annotation() {
                    if let Some(a) = r.parent {
                        let ar = &lib.rows[a];
                        if ar.is_attachment() {
                            if let Some(g) = ar.parent {
                                return g;
                            }
                        }
                        return a;
                    }
                }
                if r.is_attachment() {
                    if let Some(p) = r.parent {
                        return p;
                    }
                }
                if r.in_item_notes {
                    if let Some(p) = r.note_parent {
                        return p;
                    }
                }
                i
            })
            .collect(),
    }
}

/// `mapPredicate` (search.js:2366): the rows at `to` for a predicate whose
/// rows are at `from`.
pub fn map_predicate(
    lib: &SearchLibrary,
    set: &RowSet,
    from: &[Level],
    to: Level,
    negated: bool,
    exclude_deleted: bool,
) -> RowSet {
    if to == Level::Any {
        return set.clone();
    }
    if from == [Level::Any] {
        return if negated {
            set.clone()
        } else {
            roll_up_any(lib, set, to, exclude_deleted)
        };
    }
    if from.contains(&to) {
        return set.clone();
    }
    let Some(f) = closest_related_level(from, to) else {
        return RowSet::new();
    };
    let matches = matching(lib, set, exclude_deleted);

    let ancestors: Vec<Level> = from
        .iter()
        .copied()
        .filter(|&l| l != Level::Any && is_ancestor_level(l, to))
        .collect();
    if !negated && ancestors.len() > 1 {
        // UNION over each ancestor of (to row, ancestor row) pairs
        // (search.js:2450-2471).
        let mut out = RowSet::new();
        for anc in ancestors {
            let mut levels = Vec::new();
            let mut l = to;
            while l != anc {
                levels.push(l);
                l = l.parent().expect("ancestor reachable");
            }
            for t0 in table_rows(lib, levels[0]) {
                // Join up the chain.
                let mut cur = t0;
                let mut ok = true;
                for (i, lv) in levels.iter().enumerate().skip(1) {
                    let p = parent_in_table(lib, levels[i - 1], cur);
                    match p {
                        Some(p) if table_rows_contains(lib, *lv, p) => cur = p,
                        _ => {
                            ok = false;
                            break;
                        }
                    }
                }
                if !ok {
                    continue;
                }
                let last = *levels.last().expect("non-empty");
                let anc_row = if last.can_be_standalone() {
                    Some(parent_in_table(lib, last, cur).unwrap_or(cur))
                } else {
                    parent_in_table(lib, last, cur)
                };
                if anc_row.is_some_and(|a| matches.contains(&a)) {
                    out.insert(t0);
                }
            }
        }
        return out;
    }

    if is_ancestor_level(to, f) {
        // mapUp
        let mut l = f;
        let mut s = matches;
        while l != to {
            let standalone = l.can_be_standalone();
            s = table_rows(lib, l)
                .into_iter()
                .filter(|i| s.contains(i))
                .filter_map(|i| {
                    let p = parent_in_table(lib, l, i);
                    if standalone {
                        Some(p.unwrap_or(i))
                    } else {
                        p
                    }
                })
                .collect();
            match l.parent() {
                Some(p) => l = p,
                None => return RowSet::new(),
            }
        }
        s
    } else {
        // mapDown
        let mut path = Vec::new();
        let mut l = to;
        while l != f {
            path.push(l);
            match l.parent() {
                Some(p) => l = p,
                None => return RowSet::new(),
            }
        }
        let mut s = matches;
        for &lv in path.iter().rev() {
            s = table_rows(lib, lv)
                .into_iter()
                .filter(|&i| parent_in_table(lib, lv, i).is_some_and(|p| s.contains(&p)))
                .collect();
        }
        s
    }
}

fn table_rows_contains(lib: &SearchLibrary, level: Level, i: usize) -> bool {
    let r = &lib.rows[i];
    match level {
        Level::Attachment => r.is_attachment(),
        Level::Note => r.in_item_notes,
        Level::Annotation => r.is_annotation(),
        Level::Item | Level::Any => false,
    }
}

/// A predicate collected for the tree: its rows, the level(s) they are at
/// and whether it is a negation.
#[derive(Debug, Clone)]
pub struct Predicate {
    /// Rows the predicate is true for.
    pub rows: RowSet,
    /// The level(s) the condition matches at.
    pub levels: Vec<Level>,
    /// `isNot`/`doesNotContain`/`isEmpty`.
    pub negate: bool,
}

/// An entry of `builtConditions` (search.js:1348): a predicate or a marker.
#[derive(Debug, Clone)]
pub enum Built {
    /// `groupStart`
    GroupStart,
    /// `groupEnd`
    GroupEnd,
    /// `joinMode`
    JoinMode(Operator),
    /// `resultLevel`
    ResultLevel(Level),
    /// A predicate.
    Pred(Predicate),
}

#[derive(Debug, Default)]
struct Node {
    children: Vec<Child>,
    join_mode: Option<Operator>,
    level: Option<Level>,
}

#[derive(Debug)]
enum Child {
    Pred(usize),
    Group(usize),
}

/// `Zotero.Search.combineConditions` (search.js:2111): the rows of the
/// combined predicate, or `None` when there is nothing to combine (upstream
/// returns `sql: ''`).
pub fn combine_conditions(
    lib: &SearchLibrary,
    built: &[Built],
    root_level: Level,
    exclude_deleted: bool,
) -> Option<RowSet> {
    let mut nodes: Vec<Node> = vec![Node::default()];
    let mut preds: Vec<&Predicate> = Vec::new();
    let mut stack = vec![0usize];
    for b in built {
        let g = *stack.last().expect("root");
        match b {
            Built::GroupStart => {
                nodes.push(Node::default());
                let id = nodes.len() - 1;
                nodes[g].children.push(Child::Group(id));
                stack.push(id);
            }
            Built::GroupEnd => {
                if stack.len() > 1 {
                    stack.pop();
                }
            }
            Built::JoinMode(op) => {
                if nodes[g].join_mode.is_none() {
                    nodes[g].join_mode = Some(*op);
                }
            }
            Built::ResultLevel(l) => nodes[g].level = Some(*l),
            Built::Pred(p) => {
                preds.push(p);
                nodes[g].children.push(Child::Pred(preds.len() - 1));
            }
        }
    }
    combine_group(lib, &nodes, &preds, 0, root_level, exclude_deleted)
}

fn combine_group(
    lib: &SearchLibrary,
    nodes: &[Node],
    preds: &[&Predicate],
    n: usize,
    parent_level: Level,
    exclude_deleted: bool,
) -> Option<RowSet> {
    let node = &nodes[n];
    let level = node.level.unwrap_or(parent_level);
    let mut required: Vec<RowSet> = Vec::new();
    let mut optional: Vec<RowSet> = Vec::new();
    for c in &node.children {
        let set = match c {
            Child::Group(g) => match combine_group(lib, nodes, preds, *g, level, exclude_deleted) {
                Some(s) => s,
                None => continue,
            },
            Child::Pred(p) => {
                let p = preds[*p];
                map_predicate(lib, &p.rows, &p.levels, level, p.negate, exclude_deleted)
            }
        };
        if node.join_mode != Some(Operator::Any) {
            required.push(set);
        } else {
            optional.push(set);
        }
    }
    let mut parts: Vec<RowSet> = Vec::new();
    if !required.is_empty() {
        let mut it = required.into_iter();
        let first = it.next().expect("non-empty");
        parts.push(it.fold(first, |a, b| a.intersection(&b).copied().collect()));
    }
    if !optional.is_empty() {
        parts.push(
            optional
                .into_iter()
                .fold(RowSet::new(), |a, b| a.union(&b).copied().collect()),
        );
    }
    let mut it = parts.into_iter();
    let first = it.next()?;
    let mut set: RowSet = it.fold(first, |a, b| a.intersection(&b).copied().collect());
    let target = if parent_level == Level::Any {
        Level::Item
    } else {
        parent_level
    };
    if let Some(l) = node.level {
        if l != Level::Any && l != target {
            set = map_predicate(lib, &set, &[l], target, false, exclude_deleted);
        }
    }
    Some(set)
}
