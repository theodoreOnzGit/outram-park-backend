// Part of the kovan Zotero port (GitHub #747, #751).
//
// Upstream: Zotero, https://github.com/zotero/zotero (commit 9cbba8c4d281):
// chrome/content/zotero/xpcom/utilities_internal.js (normalizeForSearch,
// normalizeForSearchStorage); the SQLite semantics the search SQL relies on
// (LIKE, COLLATE NOCASE, CAST AS INT, SUBSTR) are re-implemented from the
// SQLite documentation, since search.js hands them to SQLite.
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA
// (older files: (c) 2006-2016 Center for History and New Media, George Mason
// University). Licence: AGPL-3.0. See this crate's NOTICE, "Upstream: Zotero".

//! Text normalisation and the SQLite primitives Zotero's search SQL uses.
//!
//! Zotero builds SQL and lets SQLite evaluate it; this port evaluates in
//! memory, so the few SQLite behaviours the generated SQL depends on are
//! reproduced here exactly:
//!
//! - [`sql_like`]: `LIKE` with `%`/`_` wildcards, case-insensitive for ASCII
//!   letters only (SQLite's default, no ICU). Zotero does **not** escape `%`
//!   or `_` in a search term (search.js:1922, 1973), so they stay wildcards
//!   here too.
//! - [`nocase_eq`]: `= ? COLLATE NOCASE` (ASCII-only case folding).
//! - [`sqlite_cast_int`] and [`is_canonical_int`]: `CAST(x AS INT)` and the
//!   "is it numeric" guard search.js:1899-1903 appends to `<`/`>`.
//! - [`substr`]: `SUBSTR(x, start, len)`, 1-based, counting characters.

use unicode_normalization::UnicodeNormalization;

/// `Zotero.Utilities.Internal._searchNormalizeMap` (utilities_internal.js:44):
/// letters NFKD leaves non-ASCII.
fn search_normalize_map(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{00f8}' => "o",
        '\u{0153}' => "oe",
        '\u{00e6}' => "ae",
        '\u{0142}' => "l",
        '\u{0111}' => "d",
        '\u{00f0}' => "d",
        '\u{00fe}' => "th",
        '\u{00df}' => "ss",
        '\u{0131}' => "i",
        '\u{2044}' => "/",
        _ => return None,
    })
}

/// The rich-text formatting tags `_searchFormattingTagRE` strips
/// (utilities_internal.js:62).
const FORMATTING_TAGS: [&str; 11] = [
    "<i>",
    "</i>",
    "<b>",
    "</b>",
    "<sub>",
    "</sub>",
    "<sup>",
    "</sup>",
    "<span style=\"font-variant:small-caps;\">",
    "<span class=\"nocase\">",
    "</span>",
];

fn strip_formatting_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    'outer: while !rest.is_empty() {
        if rest.starts_with('<') {
            for t in FORMATTING_TAGS {
                if rest.starts_with(t) {
                    rest = &rest[t.len()..];
                    continue 'outer;
                }
            }
        }
        let c = rest.chars().next().expect("non-empty");
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// `Zotero.Utilities.Internal.normalizeForSearch` (utilities_internal.js:83):
/// strip formatting tags, NFKD, drop U+0300-U+036F, lower-case, map the
/// non-decomposing letters, fold typographic quotes and dashes, NFC.
pub fn normalize_for_search(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    let s = if s.contains('<') {
        strip_formatting_tags(s)
    } else {
        s.to_owned()
    };
    let decomposed: String = s
        .nfkd()
        .filter(|c| !('\u{0300}'..='\u{036f}').contains(c))
        .collect();
    let lower = decomposed.to_lowercase();
    let mut out = String::with_capacity(lower.len());
    for c in lower.chars() {
        if let Some(m) = search_normalize_map(c) {
            out.push_str(m);
            continue;
        }
        match c {
            '\u{2018}' | '\u{2019}' | '\u{201a}' | '\u{201b}' | '\u{2032}' => out.push('\''),
            '\u{201c}' | '\u{201d}' | '\u{201e}' | '\u{201f}' | '\u{2033}' => out.push('"'),
            '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2015}'
            | '\u{2212}' => out.push('-'),
            _ => out.push(c),
        }
    }
    out.nfc().collect()
}

/// JavaScript `String#length` (UTF-16 code units), which upstream's length
/// thresholds count.
pub fn js_len(s: &str) -> usize {
    s.encode_utf16().count()
}

fn ascii_fold(c: char) -> char {
    c.to_ascii_lowercase()
}

/// SQLite `value LIKE pattern` without `ESCAPE`: `%` matches any run of
/// characters, `_` exactly one, other characters match themselves with ASCII
/// case folding.
pub fn sql_like(value: &str, pattern: &str) -> bool {
    let v: Vec<char> = value.chars().map(ascii_fold).collect();
    let p: Vec<char> = pattern.chars().map(ascii_fold).collect();
    // Iterative wildcard match with back-tracking to the last '%'.
    let (mut vi, mut pi) = (0usize, 0usize);
    let mut star: Option<(usize, usize)> = None;
    while vi < v.len() {
        if pi < p.len() && p[pi] == '%' {
            star = Some((pi, vi));
            pi += 1;
        } else if pi < p.len() && (p[pi] == '_' || p[pi] == v[vi]) {
            pi += 1;
            vi += 1;
        } else if let Some((sp, sv)) = star {
            pi = sp + 1;
            vi = sv + 1;
            star = Some((sp, sv + 1));
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '%' {
        pi += 1;
    }
    pi == p.len()
}

/// SQLite `a = b COLLATE NOCASE` (ASCII-only folding).
pub fn nocase_eq(a: &str, b: &str) -> bool {
    a.chars().count() == b.chars().count()
        && a.chars()
            .zip(b.chars())
            .all(|(x, y)| ascii_fold(x) == ascii_fold(y))
}

/// SQLite `CAST(text AS INT)`: the longest leading integer prefix after
/// leading whitespace, 0 when there is none.
pub fn sqlite_cast_int(s: &str) -> i64 {
    let t = s.trim_start();
    let mut end = 0;
    for (i, c) in t.char_indices() {
        if c.is_ascii_digit() || (i == 0 && (c == '-' || c == '+')) {
            end = i + c.len_utf8();
        } else {
            break;
        }
    }
    t[..end].parse::<i64>().unwrap_or(0)
}

/// `CAST(CAST(x AS INT) AS STRING) = x` for a text value (search.js:1899):
/// whether the text is the canonical spelling of an integer.
pub fn is_canonical_int(s: &str) -> bool {
    sqlite_cast_int(s).to_string() == s
}

/// SQLite `SUBSTR(x, start, len)` with a 1-based, positive `start`.
pub fn substr(s: &str, start: usize, len: usize) -> String {
    s.chars().skip(start.saturating_sub(1)).take(len).collect()
}

/// A note's searchable plain text: the note HTML with markup removed and
/// common entities decoded, then [`normalize_for_search`].
///
/// Upstream (`_normalizeNoteText`, fulltext.js:2350) uses Mozilla's
/// `nsIParserUtils.convertToPlainText(note, OutputRaw)`, which cannot run
/// here. This approximation drops every tag, puts a newline at block-level
/// tags (`p`, `div`, `br`, `li`, `h1`-`h6`, `tr`, `blockquote`, `pre`) and
/// decodes `&amp; &lt; &gt; &quot; &apos; &#39; &nbsp;` and numeric
/// references. It differs from upstream on other entities and on scripts or
/// styles, whose text upstream drops.
pub fn note_plain_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(c) = rest.chars().next() {
        if c == '<' {
            if let Some(end) = rest.find('>') {
                let tag = rest[1..end]
                    .trim_start_matches('/')
                    .split(|c: char| c.is_whitespace() || c == '/')
                    .next()
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if matches!(
                    tag.as_str(),
                    "p" | "div"
                        | "br"
                        | "li"
                        | "h1"
                        | "h2"
                        | "h3"
                        | "h4"
                        | "h5"
                        | "h6"
                        | "tr"
                        | "blockquote"
                        | "pre"
                ) && !out.is_empty()
                    && !out.ends_with('\n')
                {
                    out.push('\n');
                }
                rest = &rest[end + 1..];
                continue;
            }
        }
        if c == '&' {
            if let Some(end) = rest[..rest.len().min(12)].find(';') {
                let ent = &rest[1..end];
                let decoded = match ent {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    "nbsp" => Some('\u{a0}'),
                    _ => ent.strip_prefix('#').and_then(|n| {
                        let v = match n.strip_prefix(['x', 'X']) {
                            Some(h) => u32::from_str_radix(h, 16).ok(),
                            None => n.parse().ok(),
                        };
                        v.and_then(char::from_u32)
                    }),
                };
                if let Some(d) = decoded {
                    out.push(d);
                    rest = &rest[end + 1..];
                    continue;
                }
            }
        }
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    normalize_for_search(out.trim_end_matches('\n'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn like_wildcards_and_ascii_case() {
        assert!(sql_like("Review of Finance", "review of finance"));
        assert!(sql_like("abc", "%b%"));
        assert!(sql_like("abc", "a_c"));
        assert!(!sql_like("abc", "a_"));
        assert!(sql_like("2019-06-08", "2019-__-__"));
        assert!(!sql_like("\u{0398}", "\u{03b8}")); // non-ASCII case not folded
        assert!(sql_like("", "%"));
    }

    #[test]
    fn normalize_examples() {
        assert_eq!(normalize_for_search("Séance"), "seance");
        assert_eq!(
            normalize_for_search("søren œuvre ﬁle x² ½ straße"),
            "soren oeuvre file x2 1/2 strasse"
        );
        assert_eq!(normalize_for_search("“pp. 10–12”"), "\"pp. 10-12\"");
        assert_eq!(normalize_for_search("がん"), "がん");
        assert_eq!(
            normalize_for_search("The <span style=\"font-variant:small-caps;\">x</span> y"),
            "the x y"
        );
    }

    #[test]
    fn cast_int() {
        assert_eq!(sqlite_cast_int("12abc"), 12);
        assert_eq!(sqlite_cast_int("abc"), 0);
        assert!(is_canonical_int("12"));
        assert!(!is_canonical_int("012"));
        assert!(!is_canonical_int("12-15"));
    }

    #[test]
    fn note_text() {
        assert_eq!(
            note_plain_text("<p>zqsnote re content</p>"),
            "zqsnote re content"
        );
        assert_eq!(note_plain_text("<p>a &amp; b</p><p>c</p>"), "a & b\nc");
    }
}
