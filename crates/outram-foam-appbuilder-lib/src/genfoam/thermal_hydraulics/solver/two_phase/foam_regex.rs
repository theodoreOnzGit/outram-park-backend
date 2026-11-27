// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! A small regular-expression matcher for OpenFOAM dictionary keywords (gh:#1870, gh:#1361 round 62).
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! OpenFOAM stores a quoted keyword (`"(h|k|epsilon)Final"`, `"div\(alphaRhoPhi.*,k.*\)"`) as a POSIX extended regular expression
//! (`keyType::REGEX`, matched with `regExp::match`, i.e. the WHOLE name must match). The shipped `2D_FFTF` `fvSolution` uses
//! `"(h|k|epsilon)"`: the older [`super::case::key_matches`] knew literals and `.*` only. This matcher supports what the shipped
//! tutorials use: literals, `\x` escapes, `.`, `[...]` classes (ranges, leading `^`), groups `( ... )`, alternation `|`, and the
//! quantifiers `*`, `+`, `?` on an atom or a group. It is a set-of-positions simulation (no backtracking blow-up). An
//! unparsable pattern matches nothing.
//!
//! Upstream: `src/OpenFOAM/db/dictionary/dictionary.C` (`lookupEntryPtr` with pattern matching) and `regExp` (POSIX ERE).
//! Not ported: `{m,n}` repetition, back-references, anchors (`^`, `$`, `{` make the pattern unparsable here: no match).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use alloc::vec::Vec;

#[derive(Debug, Clone)]
enum Node {
    Lit(char),
    Any,
    Class { neg: bool, items: Vec<(char, char)> },
    Group(Vec<Vec<Piece>>),
}

#[derive(Debug, Clone)]
struct Piece {
    node: Node,
    /// 0 for `*` and `?`, else 1
    min: usize,
    /// `*` and `+`
    unbounded: bool,
}

fn parse_alt(c: &[char], i: &mut usize, depth: usize) -> Option<Vec<Vec<Piece>>> {
    let mut alts: Vec<Vec<Piece>> = Vec::new();
    let mut seq: Vec<Piece> = Vec::new();
    while *i < c.len() {
        let ch = c[*i];
        match ch {
            ')' => {
                if depth == 0 {
                    return None;
                }
                alts.push(seq);
                return Some(alts);
            }
            '|' => {
                alts.push(core::mem::take(&mut seq));
                *i += 1;
            }
            '(' => {
                *i += 1;
                let g = parse_alt(c, i, depth + 1)?;
                if *i >= c.len() || c[*i] != ')' {
                    return None;
                }
                *i += 1;
                seq.push(Piece { node: Node::Group(g), min: 1, unbounded: false });
            }
            '[' => {
                *i += 1;
                let mut neg = false;
                if *i < c.len() && c[*i] == '^' {
                    neg = true;
                    *i += 1;
                }
                let mut items = Vec::new();
                let mut first = true;
                while *i < c.len() && (c[*i] != ']' || first) {
                    first = false;
                    let lo = c[*i];
                    if *i + 2 < c.len() && c[*i + 1] == '-' && c[*i + 2] != ']' {
                        items.push((lo, c[*i + 2]));
                        *i += 3;
                    } else {
                        items.push((lo, lo));
                        *i += 1;
                    }
                }
                if *i >= c.len() {
                    return None;
                }
                *i += 1;
                seq.push(Piece { node: Node::Class { neg, items }, min: 1, unbounded: false });
            }
            '.' => {
                *i += 1;
                seq.push(Piece { node: Node::Any, min: 1, unbounded: false });
            }
            '\\' => {
                if *i + 1 >= c.len() {
                    return None;
                }
                seq.push(Piece { node: Node::Lit(c[*i + 1]), min: 1, unbounded: false });
                *i += 2;
            }
            '*' | '+' | '?' => {
                let last = seq.last_mut()?;
                if last.min != 1 || last.unbounded {
                    return None; // a quantifier on a quantified atom
                }
                match ch {
                    '*' => {
                        last.min = 0;
                        last.unbounded = true;
                    }
                    '+' => last.unbounded = true,
                    _ => last.min = 0, // `?`: zero or one; `unbounded` stays false (one step)
                }
                *i += 1;
            }
            '^' | '$' | '{' => return None,
            _ => {
                seq.push(Piece { node: Node::Lit(ch), min: 1, unbounded: false });
                *i += 1;
            }
        }
    }
    if depth != 0 {
        return None;
    }
    alts.push(seq);
    Some(alts)
}

fn node_ends(n: &Node, s: &[char], pos: usize, out: &mut Vec<usize>) {
    match n {
        Node::Lit(c) => {
            if pos < s.len() && s[pos] == *c {
                out.push(pos + 1);
            }
        }
        Node::Any => {
            if pos < s.len() {
                out.push(pos + 1);
            }
        }
        Node::Class { neg, items } => {
            if pos < s.len() {
                let ch = s[pos];
                let inside = items.iter().any(|&(lo, hi)| lo <= ch && ch <= hi);
                if inside != *neg {
                    out.push(pos + 1);
                }
            }
        }
        Node::Group(alts) => {
            for e in run_alts(alts, s, &[pos]) {
                out.push(e);
            }
        }
    }
}

fn dedup(mut v: Vec<usize>) -> Vec<usize> {
    v.sort_unstable();
    v.dedup();
    v
}

fn run_piece(p: &Piece, s: &[char], starts: &[usize]) -> Vec<usize> {
    let mut cur: Vec<usize> = starts.to_vec();
    let mut result: Vec<usize> = Vec::new();
    if p.min == 0 {
        result.extend_from_slice(&cur);
    }
    let mut seen = alloc::vec![false; s.len() + 2];
    let mut guard = 0usize;
    loop {
        let mut next = Vec::new();
        for &pos in &cur {
            node_ends(&p.node, s, pos, &mut next);
        }
        let next = dedup(next);
        if next.is_empty() {
            break;
        }
        result.extend_from_slice(&next);
        if !p.unbounded {
            break;
        }
        let fresh: Vec<usize> = next.into_iter().filter(|&e| !seen[e]).collect();
        for &e in &fresh {
            seen[e] = true;
        }
        if fresh.is_empty() {
            break;
        }
        cur = fresh;
        guard += 1;
        if guard > s.len() + 2 {
            break;
        }
    }
    dedup(result)
}

fn run_alts(alts: &[Vec<Piece>], s: &[char], starts: &[usize]) -> Vec<usize> {
    let mut all = Vec::new();
    for seq in alts {
        let mut cur: Vec<usize> = starts.to_vec();
        for p in seq {
            cur = run_piece(p, s, &cur);
            if cur.is_empty() {
                break;
            }
        }
        all.extend(cur);
    }
    dedup(all)
}

/// Does the WHOLE of `name` match the POSIX-ERE-subset `pattern`? An unparsable pattern matches nothing.
pub(crate) fn foam_regex_full_match(pattern: &str, name: &str) -> bool {
    let c: Vec<char> = pattern.chars().collect();
    let s: Vec<char> = name.chars().collect();
    let mut i = 0usize;
    let Some(alts) = parse_alt(&c, &mut i, 0) else { return false };
    if i != c.len() {
        return false;
    }
    run_alts(&alts, &s, &[0]).contains(&s.len())
}

/// Does `pattern` use a regex feature beyond literals, escapes and `.*` (an unescaped `(`, `)`, `|`, `[`, `+`, `?`)?
pub(crate) fn needs_full_regex(pattern: &str) -> bool {
    let mut esc = false;
    for ch in pattern.chars() {
        if esc {
            esc = false;
            continue;
        }
        match ch {
            '\\' => esc = true,
            '(' | ')' | '|' | '[' | '+' | '?' => return true,
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_fftf_keywords() {
        assert!(foam_regex_full_match("(h|k|epsilon)", "h"));
        assert!(foam_regex_full_match("(h|k|epsilon)", "epsilon"));
        assert!(!foam_regex_full_match("(h|k|epsilon)", "hFinal"));
        assert!(foam_regex_full_match("(h|k|epsilon)Final", "kFinal"));
        assert!(!foam_regex_full_match("(h|k|epsilon)Final", "k"));
        assert!(foam_regex_full_match(".*", "anything"));
        assert!(foam_regex_full_match("epsilon.*", "epsilonFinal"));
    }

    #[test]
    fn quantifiers_classes_and_groups() {
        assert!(foam_regex_full_match("a+b?c", "aaac"));
        assert!(foam_regex_full_match("a+b?c", "abc"));
        assert!(!foam_regex_full_match("a+b?c", "bc"));
        assert!(foam_regex_full_match("[a-c]x*", "bxxx"));
        assert!(!foam_regex_full_match("[^a-c]x*", "bxxx"));
        assert!(foam_regex_full_match("(ab)*c", "ababc"));
        assert!(!foam_regex_full_match("(ab)*c", "abac"));
        assert!(foam_regex_full_match("(h|e).*", "hFinal"));
        assert!(foam_regex_full_match("div\\(alphaRhoPhi.*,(h|e).*\\)", "div(alphaRhoPhi,h)"));
        assert!(!foam_regex_full_match("(unclosed", "x"));
        assert!(needs_full_regex("(h|k)") && !needs_full_regex("div\\(a.*\\)") && !needs_full_regex("alpha.*"));
    }
}
