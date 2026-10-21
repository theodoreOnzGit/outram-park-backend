//! **Syntax colours for source text** (GitHub #857), with no language
//! server: the maintainer does not want a second rust-analyzer running
//! ("too memory intensive", 2026-10-13).
//!
//! ```text
//!  source lines ──> kopitiam_syntax::Highlighter (a line lexer that carries
//!                   "inside a block comment / raw string" from line to line)
//!       │
//!       v
//!  runs(line, spans) ──> [("pub", Keyword), (" ", none), ("fn", Keyword), ..]
//!       │                 every byte of the line exactly once, in order
//!       v
//!  ui: colour(kind) ──> one coloured piece of text per run
//! ```
//!
//! The lexer is lexical only: it knows a keyword, a string, a comment, a
//! number, a macro call and a capitalised name. It does not know what a
//! name refers to; that comes from kovan's call graph, drawn on top by the
//! callers of this module.
//!
//! Reused, not written here: `kopitiam-syntax` is the lexer kvim itself
//! highlights with.

pub use kopitiam_syntax::{HighlightKind, HighlightSpan, Language};

/// The language of `file`, by its extension; `None` when the lexer has none
/// for it (the text is then drawn plain).
pub fn language_of(file: &str) -> Option<Language> {
    Language::from_extension(file.rsplit_once('.')?.1)
}

/// The spans of every line of `source`, in order. Lines are lexed from the
/// top so a block comment or raw string opened on one line colours the
/// next; `source` should therefore start outside one (a function's own
/// lines, doc comment included, do).
pub fn highlight(source: &str, language: Language) -> Vec<Vec<HighlightSpan>> {
    let mut h = kopitiam_syntax::Highlighter::new(language);
    source.lines().map(|l| h.highlight_line(l)).collect()
}

/// `line` cut into runs, each with the kind the lexer gave it (`None`: plain
/// text). The runs cover the line exactly once and in order, whatever the
/// spans are: a span that overlaps an earlier one, runs past the end or
/// does not sit on character boundaries is dropped, never guessed at.
pub fn runs<'a>(line: &'a str, spans: &[HighlightSpan]) -> Vec<(&'a str, Option<HighlightKind>)> {
    let mut out = Vec::new();
    let mut at = 0;
    for s in spans {
        let ok = s.start >= at && s.end > s.start && s.end <= line.len() && line.is_char_boundary(s.start) && line.is_char_boundary(s.end);
        if !ok {
            continue;
        }
        if s.start > at {
            out.push((&line[at..s.start], None));
        }
        out.push((&line[s.start..s.end], Some(s.kind)));
        at = s.end;
    }
    if at < line.len() {
        out.push((&line[at..], None));
    }
    out
}

/// The kind of the run holding byte `byte` of the line, if any.
pub fn kind_at(spans: &[HighlightSpan], byte: usize) -> Option<HighlightKind> {
    spans.iter().find(|s| s.start <= byte && byte < s.end).map(|s| s.kind)
}

/// An `(r, g, b)` colour for `kind` on a dark or a light background. Kinds
/// that only occur in prose languages share the nearest code colour.
pub fn rgb(kind: HighlightKind, dark: bool) -> (u8, u8, u8) {
    use HighlightKind as K;
    match (kind, dark) {
        (K::Keyword, true) => (198, 146, 233),
        (K::Keyword, false) => (140, 60, 170),
        (K::Type, true) => (110, 200, 200),
        (K::Type, false) => (20, 120, 130),
        (K::String | K::CodeBlock, true) => (170, 210, 130),
        (K::String | K::CodeBlock, false) => (60, 120, 30),
        (K::Escape, true) => (230, 180, 110),
        (K::Escape, false) => (160, 100, 20),
        (K::Comment, true) => (130, 140, 150),
        (K::Comment, false) => (110, 115, 120),
        (K::Number, true) => (240, 160, 110),
        (K::Number, false) => (170, 80, 20),
        (K::Function | K::Heading, true) => (130, 180, 250),
        (K::Function | K::Heading, false) => (30, 90, 190),
        (K::Macro | K::Link, true) => (120, 190, 240),
        (K::Macro | K::Link, false) => (20, 110, 160),
        (K::Attribute | K::Emphasis, true) => (220, 200, 120),
        (K::Attribute | K::Emphasis, false) => (130, 110, 20),
        (K::Operator | K::Punctuation, true) => (190, 195, 200),
        (K::Operator | K::Punctuation, false) => (70, 75, 80),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Rust line is cut into runs that put the line back together, with
    /// the keyword, the string and the comment each given their kind.
    #[test]
    fn a_rust_line_is_cut_into_kinds_that_rebuild_it() {
        let src = "pub fn area(r: f64) -> f64 { 3.14 * r } // \"circle\"\nlet s = \"a // b\";";
        let spans = highlight(src, Language::Rust);
        assert_eq!(spans.len(), 2);
        for (line, sp) in src.lines().zip(&spans) {
            let r = runs(line, sp);
            assert_eq!(r.iter().map(|(t, _)| *t).collect::<String>(), line, "runs rebuild the line");
        }
        let first = runs(src.lines().next().unwrap(), &spans[0]);
        let kind = |t: &str| first.iter().find(|(x, _)| *x == t).and_then(|(_, k)| *k);
        assert_eq!(kind("pub"), Some(HighlightKind::Keyword));
        assert_eq!(kind("fn"), Some(HighlightKind::Keyword));
        assert_eq!(kind("3.14"), Some(HighlightKind::Number));
        assert!(first.last().is_some_and(|(t, k)| t.starts_with("//") && *k == Some(HighlightKind::Comment)));
        // `//` inside a string is string, not comment.
        let second = runs(src.lines().nth(1).unwrap(), &spans[1]);
        assert!(second.iter().any(|(t, k)| t.contains("a // b") && *k == Some(HighlightKind::String)), "{second:?}");
        assert!(!second.iter().any(|(_, k)| *k == Some(HighlightKind::Comment)));
    }

    /// A block comment opened on one line colours the next line too.
    #[test]
    fn a_block_comment_carries_across_lines() {
        let spans = highlight("let a = 1; /* note\nstill note */ let b = 2;", Language::Rust);
        assert_eq!(kind_at(&spans[1], 0), Some(HighlightKind::Comment));
        assert_eq!(kind_at(&spans[1], 14), Some(HighlightKind::Keyword), "`let` after the comment closes");
    }

    /// Spans that cannot be trusted are dropped and the line still comes
    /// back whole, including a line with characters wider than one byte.
    #[test]
    fn bad_spans_never_lose_or_split_text() {
        let line = "let σ = r·p; // v̇·t";
        let k = HighlightKind::Keyword;
        let bad = [
            HighlightSpan { start: 0, end: 3, kind: k },
            HighlightSpan { start: 2, end: 5, kind: k },   // overlaps the first
            HighlightSpan { start: 5, end: 5, kind: k },   // empty
            HighlightSpan { start: 4, end: 5, kind: k },   // ends inside the two-byte sigma
            HighlightSpan { start: 10, end: 999, kind: k }, // past the end
        ];
        let r = runs(line, &bad);
        assert_eq!(r.iter().map(|(t, _)| *t).collect::<String>(), line);
        assert_eq!(r[0], ("let", Some(k)));
        assert_eq!(r.len(), 2);
        // And the lexer's own spans on it rebuild it too.
        let sp = highlight(line, Language::Rust);
        assert_eq!(runs(line, &sp[0]).iter().map(|(t, _)| *t).collect::<String>(), line);
    }

    #[test]
    fn the_language_comes_from_the_extension() {
        assert_eq!(language_of("crates/a/src/lib.rs"), Some(Language::Rust));
        assert_eq!(language_of("Cargo.toml"), Some(Language::Toml));
        assert_eq!(language_of("LICENSE"), None);
        assert_eq!(language_of("notes.txt"), None);
    }

    /// Every kind has a colour on both backgrounds, and comments are the
    /// dimmest thing on a dark one.
    #[test]
    fn every_kind_has_a_colour() {
        use HighlightKind as K;
        let all = [K::Keyword, K::Type, K::String, K::Escape, K::Comment, K::Number, K::Function, K::Operator,
            K::Punctuation, K::Macro, K::Attribute, K::Heading, K::Emphasis, K::Link, K::CodeBlock];
        let lum = |(r, g, b): (u8, u8, u8)| r as u32 + g as u32 + b as u32;
        for k in all {
            assert!(lum(rgb(k, true)) > lum(rgb(k, false)), "{k:?}: brighter on dark than on light");
            assert!(lum(rgb(K::Comment, true)) <= lum(rgb(k, true)), "{k:?}");
        }
    }
}
