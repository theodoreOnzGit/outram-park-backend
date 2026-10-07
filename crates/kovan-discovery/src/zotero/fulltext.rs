// Part of the kovan Zotero port (GitHub #747, #751).
//
// Upstream: Zotero, https://github.com/zotero/zotero (commit 9cbba8c4d281):
// chrome/content/zotero/xpcom/fulltext.js:2132-2189 (findTextInString),
// 2208-2246 (CJK bigrams, word tokens), 2399-2507 (getSubstringMatchClause,
// getWordMatchClause, canSearchContent, canSearchNotes), 2632-2739
// (findItemsWithContent, getContentSearchSQL, findTextInItems).
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0. See this crate's NOTICE, "Upstream: Zotero".

//! Full-text content matching, against text the caller supplies.
//!
//! Upstream keeps an SQLite FTS5 word index (normalised attachment text,
//! `unicode61` tokens), a CJK 2-gram index, and the cached text files it
//! scans to verify a phrase or run a regular expression. None of that
//! exists for a `ZoteroLibrary`; [`super::SearchLibrary::set_full_text`]
//! supplies an attachment's extracted text, and this module reproduces the
//! *matching semantics* over it:
//!
//! - the index path: the term's word tokens as an adjacent phrase with the
//!   last token a prefix (`"t1 t2"*`), or a pure-CJK term's 2-grams as a
//!   contiguous phrase, then (multi-token or punctuated terms) the
//!   separator-collapsed substring verification of `findTextInString`;
//! - the scan path (`regexp`/`regexpCS` modes, and terms the index cannot
//!   answer): `findTextInString`.
//!
//! **Differs from upstream:** the `unicode61` tokenizer is approximated by
//! runs of `\p{L}\p{N}` (upstream's own `_wordTokenRE`, fulltext.js:2214,
//! which it documents as matching the tokenizer); regular expressions are
//! Rust `regex` syntax, not JavaScript (look-around and back-references are
//! unsupported and match nothing); upstream's `textMaxLength` truncation and
//! the "Binary" modes (which read non-text files) do not apply.

use std::sync::OnceLock;

use regex::{Regex, RegexBuilder};

use super::library::{RowSet, SearchLibrary};
use super::normalize::{js_len, normalize_for_search};

fn cjk_char_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"[\p{Han}\p{Hiragana}\p{Katakana}\p{Hangul}]").expect("re"))
}

fn cjk_run_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"[\p{Han}\p{Hiragana}\p{Katakana}\p{Hangul}]+").expect("re"))
}

fn word_token_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"[\p{L}\p{N}]+").expect("re"))
}

/// `getCJKBigrams` (fulltext.js:2224): overlapping 2-grams of each CJK run.
fn cjk_bigrams(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for m in cjk_run_re().find_iter(text) {
        let chars: Vec<char> = m.as_str().chars().collect();
        for w in chars.windows(2) {
            out.push(w.iter().collect());
        }
    }
    out
}

/// `hasNonCJKWordChars` (fulltext.js:2244).
fn has_non_cjk_word_chars(normalized: &str) -> bool {
    let stripped = cjk_run_re().replace_all(normalized, "");
    word_token_re().is_match(&stripped)
}

/// The index clause of a content term (`getWordMatchClause`,
/// fulltext.js:2441).
#[derive(Debug, Clone, PartialEq, Eq)]
enum WordClause {
    /// Pure CJK: these 2-grams as a contiguous phrase.
    Cjk(Vec<String>),
    /// Word tokens as an adjacent phrase, last one a prefix; `verify` asks
    /// for the substring check.
    Words { tokens: Vec<String>, verify: bool },
}

fn word_match_clause(term: &str) -> Option<WordClause> {
    let n = normalize_for_search(term);
    if n.is_empty() {
        return None;
    }
    let has_cjk = cjk_char_re().is_match(&n);
    let has_non_cjk = has_non_cjk_word_chars(&n);
    if has_cjk && !has_non_cjk {
        let b = cjk_bigrams(&n);
        return if b.is_empty() {
            None
        } else {
            Some(WordClause::Cjk(b))
        };
    }
    if has_cjk {
        return None;
    }
    let tokens: Vec<String> = word_token_re()
        .find_iter(&n)
        .map(|m| m.as_str().to_owned())
        .collect();
    if tokens.is_empty() {
        return None;
    }
    static PUNCT: OnceLock<Regex> = OnceLock::new();
    let punct = PUNCT
        .get_or_init(|| Regex::new(r"[^\p{L}\p{N}\s-]").expect("re"))
        .is_match(&n);
    let verify = tokens.len() > 1 || punct;
    Some(WordClause::Words { tokens, verify })
}

/// `getSubstringMatchClause(term) !== null` (fulltext.js:2399), i.e.
/// `Zotero.FullText.canSearchNotes` (fulltext.js:2505).
pub fn can_search_notes(term: &str) -> bool {
    let n = normalize_for_search(term);
    if n.is_empty() {
        return false;
    }
    let has_cjk = cjk_char_re().is_match(&n);
    let has_non_cjk = has_non_cjk_word_chars(&n);
    if has_cjk && !has_non_cjk {
        return !cjk_bigrams(&n).is_empty();
    }
    !has_cjk && js_len(&n) >= 3
}

/// `Zotero.FullText.canSearchContent` (fulltext.js:2488).
pub fn can_search_content(term: &str) -> bool {
    match word_match_clause(term) {
        None => false,
        Some(WordClause::Cjk(_)) => true,
        Some(WordClause::Words { tokens, .. }) => tokens.len() > 1 || js_len(&tokens[0]) >= 3,
    }
}

/// Whether the term can be answered from the index without a verification
/// scan (`getContentSearchSQL(term) !== null`, fulltext.js:2671).
pub fn index_only(term: &str) -> bool {
    matches!(
        word_match_clause(term),
        Some(WordClause::Cjk(_)) | Some(WordClause::Words { verify: false, .. })
    )
}

fn phrase_match(content_tokens: &[String], phrase: &[String], last_prefix: bool) -> bool {
    if phrase.is_empty() || content_tokens.len() < phrase.len() {
        return false;
    }
    let k = phrase.len();
    content_tokens.windows(k).any(|w| {
        w.iter().zip(phrase).enumerate().all(|(i, (c, p))| {
            if last_prefix && i == k - 1 {
                c.starts_with(p.as_str())
            } else {
                c == p
            }
        })
    })
}

fn index_match(content: &str, clause: &WordClause) -> bool {
    let n = normalize_for_search(content);
    match clause {
        WordClause::Cjk(b) => phrase_match(&cjk_bigrams(&n), b, false),
        WordClause::Words { tokens, .. } => {
            let ct: Vec<String> = word_token_re()
                .find_iter(&n)
                .map(|m| m.as_str().to_owned())
                .collect();
            phrase_match(&ct, tokens, true)
        }
    }
}

/// `findTextInString` (fulltext.js:2132).
pub fn find_text_in_string(content: &str, term: &str, mode: Option<&str>) -> bool {
    match mode {
        Some(m @ ("regexp" | "regexpCS" | "regexpBinary" | "regexpCSBinary")) => {
            let mut pat = term;
            // "Slashes in regex are optional"; user flags are ignored.
            let stripped;
            if let Some(rest) = term.strip_prefix('/') {
                if let Some(end) = rest.rfind('/') {
                    if !rest[end + 1..].contains('/') {
                        stripped = rest[..end].to_owned();
                        pat = &stripped;
                    }
                }
            }
            match RegexBuilder::new(pat)
                .multi_line(true)
                .case_insensitive(!m.contains("regexpCS"))
                .build()
            {
                Ok(re) => re.is_match(content),
                Err(_) => false,
            }
        }
        _ => {
            let collapse = |s: &str| {
                let mut out = String::with_capacity(s.len());
                let mut in_sep = false;
                for c in s.chars() {
                    if c.is_whitespace() || c == '-' {
                        if !in_sep {
                            out.push(' ');
                        }
                        in_sep = true;
                    } else {
                        out.push(c);
                        in_sep = false;
                    }
                }
                out
            };
            let t = collapse(&normalize_for_search(term));
            if t.trim().is_empty() {
                return false;
            }
            collapse(&normalize_for_search(content)).contains(&t)
        }
    }
}

/// `Zotero.FullText.findTextInItems` (fulltext.js:2683): the attachments of
/// `rows` whose supplied text matches.
pub fn find_text_in_items(
    lib: &SearchLibrary,
    rows: &RowSet,
    term: &str,
    mode: Option<&str>,
) -> RowSet {
    if term.is_empty() {
        return RowSet::new();
    }
    rows.iter()
        .copied()
        .filter(|&i| {
            let r = &lib.rows[i];
            r.is_attachment()
                && lib
                    .full_text
                    .get(&r.key)
                    .is_some_and(|c| find_text_in_string(c, term, mode))
        })
        .collect()
}

/// `Zotero.FullText.findItemsWithContent` (fulltext.js:2632): the indexed
/// attachments (restricted to `scope` when given) matching `term`, or
/// `None` when the index cannot answer it.
pub fn find_items_with_content(
    lib: &SearchLibrary,
    term: &str,
    scope: Option<&RowSet>,
) -> Option<RowSet> {
    let clause = word_match_clause(term)?;
    let mut ids: RowSet = lib
        .attachments()
        .filter(|&i| {
            lib.full_text
                .get(&lib.rows[i].key)
                .is_some_and(|c| index_match(c, &clause))
        })
        .collect();
    if let Some(s) = scope {
        ids = ids.intersection(s).copied().collect();
    }
    let verify = matches!(clause, WordClause::Words { verify: true, .. });
    if verify && !ids.is_empty() {
        ids = find_text_in_items(lib, &ids, term, None);
    }
    Some(ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clauses() {
        assert!(can_search_content("electro"));
        assert!(!can_search_content("el"));
        assert!(can_search_content("clim chang"));
        assert!(!can_search_notes("re"));
        assert!(can_search_notes("zqsnote"));
        assert!(index_only("foo"));
        assert!(!index_only("foo bar"));
        assert!(!index_only("c++"));
    }

    #[test]
    fn string_matching() {
        assert!(find_text_in_string("hello\nfoo bar", "foo bar", None));
        assert!(!find_text_in_string("hello\nfoo", "foo bar", None));
        assert!(find_text_in_string(
            "decision-making",
            "decision making",
            None
        ));
        assert!(find_text_in_string(
            "hello\nfoo bar",
            "foo.+bar",
            Some("regexp")
        ));
        assert!(!find_text_in_string("foo\nbar", "foo.+bar", Some("regexp")));
        assert!(find_text_in_string(
            "FOO bar",
            "/foo.+bar/g",
            Some("regexp")
        ));
        assert!(!find_text_in_string(
            "FOO bar",
            "foo.+bar",
            Some("regexpCS")
        ));
    }

    #[test]
    fn phrase_prefix() {
        let t = |s: &str| -> Vec<String> { s.split(' ').map(str::to_owned).collect() };
        assert!(phrase_match(&t("the climate of"), &t("clim"), true));
        assert!(!phrase_match(&t("condition"), &t("ion"), true));
        assert!(phrase_match(&t("foo bar"), &t("foo ba"), true));
        assert!(!phrase_match(&t("foo x bar"), &t("foo ba"), true));
    }
}
