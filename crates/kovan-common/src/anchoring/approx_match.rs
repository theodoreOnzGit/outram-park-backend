// Ported from approx-string-match (JavaScript/TypeScript), `src/index.ts`.
//   Upstream:  https://github.com/robertknight/approx-string-match-js
//   Version:   2.0.0, commit fe814eba4d6b6daf88d38179331a14d156a0b5a0
//   Copyright: (c) 2020 Robert Knight
//   Licence:   MIT (LICENSE file and package.json checked first-hand,
//              2026-10-07). The MIT notice is reproduced in this crate's
//              NOTICE. This Rust port is distributed under AGPL-3.0-only as
//              part of kovan-common.
//
//! Myers' bit-parallel approximate string matching (GitHub #754).
//!
//! A line-by-line port of `approx-string-match`, the matcher Hypothesis's
//! `match-quote.ts` calls. It finds every end position in `text` where
//! `pattern` matches with the fewest edits (insertions, deletions or
//! substitutions), up to `max_errors`, and then the start of each match.
//!
//! References, as upstream cites them:
//! 1. G. Myers, "A Fast Bit-Vector Algorithm for Approximate String Matching
//!    Based on Dynamic Programming", J. ACM 46(3), 395-415, 1999.
//! 2. M. Šošić, "An SIMD dynamic programming C/C++ library", doctoral
//!    dissertation, University of Zagreb, 2014.
//!
//! **One deliberate difference from upstream: the unit of text.** Upstream
//! counts UTF-16 code units, so a character outside the Basic Multilingual
//! Plane (an emoji) counts as two. This port counts Unicode scalar values
//! (`char`), which is what the W3C Web Annotation model specifies for
//! `TextPositionSelector` and what upstream's own test calls the behaviour
//! "we probably want". Upstream's `unicode` fixtures are ported with the
//! expectations adjusted accordingly (see `tests/upstream_approx.rs`).
//!
//! The word size stays 32 bits, as upstream, so the block arithmetic (and
//! its edge cases at pattern lengths of 32 and 64) is exercised exactly as
//! upstream's tests exercise it.

use std::collections::BTreeMap;

/// One approximate match of a pattern in a text, in `char` offsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Match {
    /// Start offset of the match in the text.
    pub start: usize,
    /// End offset (exclusive) of the match in the text.
    pub end: usize,
    /// Edits (insertions, deletions, substitutions) between the pattern and
    /// the matched text.
    pub errors: usize,
}

/// Word size of the bit-parallel blocks, as upstream (JavaScript has only
/// 32-bit bitwise operators).
const W: usize = 32;

/// Block calculation step (upstream `advanceBlock`; Fig. 8 of Myers, with
/// the branch-free updates of Šošić §4.2.3). `h_in` and the return value are
/// horizontal deltas in {-1, 0, 1}.
fn advance_block(
    p: &mut [u32],
    m: &mut [u32],
    last_row_mask: &[u32],
    peq: &[u32],
    b: usize,
    h_in: i32,
) -> i32 {
    let mut pv = p[b];
    let mut mv = m[b];
    let h_in_is_negative: u32 = u32::from(h_in < 0);
    let eq = peq[b] | h_in_is_negative;

    // Step 1: horizontal deltas.
    let xv = eq | mv;
    let xh = ((eq & pv).wrapping_add(pv) ^ pv) | eq;

    let mut ph = mv | !(xh | pv);
    let mut mh = pv & xh;

    // Step 2: score of the last row of this block.
    let h_out = i32::from(ph & last_row_mask[b] != 0) - i32::from(mh & last_row_mask[b] != 0);

    // Step 3: vertical deltas for the next character.
    ph <<= 1;
    mh <<= 1;
    mh |= h_in_is_negative;
    ph |= u32::from(h_in > 0);

    pv = mh | !(xv | ph);
    mv = ph & xv;
    p[b] = pv;
    m[b] = mv;
    h_out
}

/// Ends and error counts of the best matches of `pattern` in `text`
/// (upstream `findMatchEnds`, Fig. 9 of Myers). Only the matches with the
/// lowest error count are kept.
///
/// `max_errors` is an `f64` because upstream's caller `matchQuote` passes
/// `quote.length / 2`, which is fractional for an odd-length quote; the
/// comparisons below reproduce JavaScript's number semantics exactly.
pub(crate) fn find_match_ends(text: &[char], pattern: &[char], max_errors: f64) -> Vec<Match> {
    if pattern.is_empty() {
        return Vec::new();
    }
    let len = pattern.len();
    // Clamp so that the `max_errors` and `len` rows are in the same block.
    let mut max_errors = max_errors.min(len as f64);
    let mut matches = Vec::new();

    let b_max = len.div_ceil(W) - 1;
    let mut p = vec![0u32; b_max + 1];
    let mut m = vec![0u32; b_max + 1];
    let mut last_row_mask = vec![1u32 << 31; b_max + 1];
    last_row_mask[b_max] = 1u32 << ((len - 1) % W);

    // Bit vectors marking where each pattern character occurs. A BTreeMap
    // (not a HashMap) so nothing here depends on a random hash seed.
    let empty_peq = vec![0u32; b_max + 1];
    let mut peq: BTreeMap<char, Vec<u32>> = BTreeMap::new();
    for &c in pattern {
        if peq.contains_key(&c) {
            continue;
        }
        let mut char_peq = vec![0u32; b_max + 1];
        for (b, word) in char_peq.iter_mut().enumerate() {
            for r in 0..W {
                let idx = b * W + r;
                if idx < len && pattern[idx] == c {
                    *word |= 1u32 << r;
                }
            }
        }
        peq.insert(c, char_peq);
    }

    // Index of the last active block in the column.
    let mut y = ((max_errors / W as f64).ceil() as i64 - 1).max(0) as usize;

    // Maximum error count at the bottom of each block.
    let mut score = vec![0i64; b_max + 1];
    for (b, s) in score.iter_mut().enumerate().take(y + 1) {
        *s = ((b + 1) * W) as i64;
    }
    score[b_max] = len as i64;

    for b in 0..=y {
        p[b] = !0;
        m[b] = 0;
    }

    for (j, c) in text.iter().enumerate() {
        let char_peq = peq.get(c).unwrap_or(&empty_peq);

        let mut carry = 0i32;
        for (b, s) in score.iter_mut().enumerate().take(y + 1) {
            carry = advance_block(&mut p, &mut m, &last_row_mask, char_peq, b, carry);
            *s += i64::from(carry);
        }

        if (score[y] - i64::from(carry)) as f64 <= max_errors
            && y < b_max
            && ((char_peq[y + 1] & 1) != 0 || carry < 0)
        {
            // Bottom block under threshold: process one more block.
            y += 1;
            p[y] = !0;
            m[y] = 0;
            let max_block_score = if y == b_max {
                let remainder = len % W;
                if remainder == 0 {
                    W
                } else {
                    remainder
                }
            } else {
                W
            } as i64;
            let h = advance_block(&mut p, &mut m, &last_row_mask, char_peq, y, carry);
            score[y] = score[y - 1] + max_block_score - i64::from(carry) + i64::from(h);
        } else {
            // Bottom block over threshold: process fewer blocks next column.
            while y > 0 && score[y] as f64 >= max_errors + W as f64 {
                y -= 1;
            }
        }

        if y == b_max && score[y] as f64 <= max_errors {
            if (score[y] as f64) < max_errors {
                // Discard earlier, worse matches.
                matches.clear();
            }
            matches.push(Match {
                start: 0,
                end: j + 1,
                errors: score[y] as usize,
            });
            // Only the best matches are reported, so ratchet the threshold.
            max_errors = score[y] as f64;
        }
    }
    matches
}

/// Starts of the matches whose ends `find_match_ends` found (upstream
/// `findMatchStarts`): match the reversed pattern against the reversed text
/// with the same error count, and of several possible starts take the one
/// that makes the match longest.
fn find_match_starts(text: &[char], pattern: &[char], matches: Vec<Match>) -> Vec<Match> {
    let pat_rev: Vec<char> = pattern.iter().rev().copied().collect();
    matches
        .into_iter()
        .map(|mt| {
            let min_start = mt.end.saturating_sub(pattern.len() + mt.errors);
            let text_rev: Vec<char> = text[min_start..mt.end].iter().rev().copied().collect();
            let start = find_match_ends(&text_rev, &pat_rev, mt.errors as f64)
                .iter()
                .fold(mt.end, |min, rm| {
                    if mt.end - rm.end < min {
                        mt.end - rm.end
                    } else {
                        min
                    }
                });
            Match {
                start,
                end: mt.end,
                errors: mt.errors,
            }
        })
        .collect()
}

/// The closest matches of `pattern` in `text`: every match with the lowest
/// error count, or none if none has `max_errors` or fewer. Upstream's
/// default export `search`. Offsets are `char` indices.
pub fn search(text: &[char], pattern: &[char], max_errors: usize) -> Vec<Match> {
    search_f64(text, pattern, max_errors as f64)
}

/// [`search`] on string slices; offsets are still `char` indices.
pub fn search_str(text: &str, pattern: &str, max_errors: usize) -> Vec<Match> {
    let t: Vec<char> = text.chars().collect();
    let p: Vec<char> = pattern.chars().collect();
    search(&t, &p, max_errors)
}

/// [`search`] with upstream's JavaScript-number threshold (see
/// [`find_match_ends`]).
pub(crate) fn search_f64(text: &[char], pattern: &[char], max_errors: f64) -> Vec<Match> {
    let ends = find_match_ends(text, pattern, max_errors);
    find_match_starts(text, pattern, ends)
}
