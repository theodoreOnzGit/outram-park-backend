// Ported from the Hypothesis client (TypeScript):
//   `src/annotator/util/normalize.ts` (`translateOffsets`, `advance`,
//   `countChars`) and `src/annotator/anchoring/pdf.ts` (`isSpace`,
//   `stripSpaces`).
//   Upstream:  https://github.com/hypothesis/client
//   Commit:    b4d085a2f893aa6de3b61d8b8bc3ae4d0f24fc1a (shallow clone, 2026-10-07)
//   Copyright: (c) 2013-2019 Hypothes.is Project and contributors
//   Licence:   BSD-2-Clause (checked first-hand 2026-10-07; see NOTICE).
//              This Rust port is distributed under AGPL-3.0-only.
//
//! Whitespace-insensitive offset translation, used by the page anchoring to
//! compare a quote with re-extracted PDF text that differs in its spaces.
//!
//! **Left out:** upstream's optional Unicode NFKD normalisation inside
//! `translateOffsets` (which relates an "fi" ligature to "f" + "i"). It
//! needs a Unicode normalisation table, and no wasm-clean crate for one is
//! in the root `[workspace.dependencies]`; upstream itself switches it off
//! on its quote-matching path (`normalize: false`) and uses it only to
//! relate PDF.js's text API to its rendered text layer, a step kovan does
//! not have. A ligature difference therefore costs edit errors here instead.

/// Upstream's `isSpace`: the ASCII spaces PDF.js produces, plus NBSP. Not
/// every Unicode space, deliberately (upstream's comment).
pub fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\u{c}' | '\n' | '\r' | '\t' | '\u{b}' | '\u{a0}')
}

/// `!is_space(c)`.
pub fn is_not_space(c: char) -> bool {
    !is_space(c)
}

/// The characters of `s` with [`is_space`] ones removed (upstream
/// `stripSpaces`).
pub fn strip_spaces(s: &[char]) -> Vec<char> {
    s.iter().copied().filter(|c| !is_space(*c)).collect()
}

/// Smallest offset in `s` from `start_pos` with at least `count` characters
/// passing `filter` before it (upstream `advance`).
fn advance<F: Fn(char) -> bool>(
    s: &[char],
    mut count: usize,
    filter: &F,
    start_pos: usize,
) -> usize {
    let mut pos = start_pos;
    while pos < s.len() && count > 0 {
        if filter(s[pos]) {
            count -= 1;
        }
        pos += 1;
    }
    pos
}

/// Characters in `s[start..end]` passing `filter` (upstream `countChars`).
fn count_chars<F: Fn(char) -> bool>(s: &[char], filter: &F, start: usize, end: usize) -> usize {
    s[start..end].iter().filter(|c| filter(**c)).count()
}

/// Translate a `(start, end)` pair of offsets in `input` into the matching
/// offsets in `output`, two strings that hold the same "important"
/// characters (those passing `filter`) with different ignored ones between
/// them. Of several equivalent output offsets, the largest start and the
/// smallest end are chosen, so leading and trailing ignored characters are
/// trimmed. Out-of-range inputs are clamped, as upstream. Upstream
/// `translateOffsets` without its `normalize` option (module doc).
pub fn translate_offsets<F: Fn(char) -> bool>(
    input: &[char],
    output: &[char],
    start: i64,
    end: i64,
    filter: F,
) -> (usize, usize) {
    let n = input.len() as i64;
    let start = start.min(n).max(0) as usize;
    let end = (end.min(n)).max(start as i64) as usize;

    let before_start = count_chars(input, &filter, 0, start);
    let start_to_end = count_chars(input, &filter, start, end);

    let mut out_start = advance(output, before_start, &filter, 0);
    while out_start < output.len() && !filter(output[out_start]) {
        out_start += 1;
    }
    let out_end = advance(output, start_to_end, &filter, out_start);
    (out_start, out_end)
}
