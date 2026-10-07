// Part of the kovan Zotero port (GitHub #747, #756).
//
// Upstream: Zotero utilities, https://github.com/zotero/utilities (commit
//   4051881d59c6, the translation-server's pin): utilities.js
//   `extractIdentifiers` :383-470; Zotero translate (commit dd524aea9a55)
//   src/translation/translate.js `Zotero.Translate.Search#setIdentifier`
//   :2746-2782; the translation-server (commit 3a9d17614896)
//   src/searchEndpoint.js :40-47 (the PMID-only rule). Tests: zotero
//   (desktop) test/tests/utilities_internalTest.js `#extractIdentifiers()`.
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! Finding identifiers in text, as Zotero's "Add Item by Identifier" box
//! does: DOIs first, then ISBNs, then arXiv IDs, then ADS bibcodes, then
//! PubMed IDs (each tried only when the earlier kinds found nothing).
//!
//! The upstream regular expressions use lookaheads, which the `regex`
//! crate does not have, so each is a small hand-written matcher that
//! reproduces the expression's backtracking on ASCII-class input
//! (JavaScript's `\d`, `\b`, `[A-Za-z]` are ASCII; `\s` is JavaScript's
//! whitespace, [`js::is_space`]). Quirks kept: ADS bibcodes and PMIDs are
//! not de-duplicated (upstream tests a match object, not the string,
//! against its `Set`).

use super::super::framework::identifiers::clean_isbn;
use super::super::framework::item::JsObject;
use super::super::framework::js;
use super::super::framework::utilities::clean_doi;
use serde_json::Value;

/// One identifier `extractIdentifiers` found.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Identifier {
    /// `{DOI}`.
    Doi(String),
    /// `{ISBN}` (cleaned: digits and X, no hyphens).
    Isbn(String),
    /// `{arXiv}` (without its version).
    ArXiv(String),
    /// `{adsBibcode}`.
    AdsBibcode(String),
    /// `{PMID}`.
    Pmid(String),
}

impl Identifier {
    /// The identifier's value.
    pub fn value(&self) -> &str {
        match self {
            Identifier::Doi(s)
            | Identifier::Isbn(s)
            | Identifier::ArXiv(s)
            | Identifier::AdsBibcode(s)
            | Identifier::Pmid(s) => s,
        }
    }

    /// The property name upstream uses (`DOI`, `ISBN`, `arXiv`,
    /// `adsBibcode`, `PMID`).
    pub fn kind(&self) -> &'static str {
        match self {
            Identifier::Doi(_) => "DOI",
            Identifier::Isbn(_) => "ISBN",
            Identifier::ArXiv(_) => "arXiv",
            Identifier::AdsBibcode(_) => "adsBibcode",
            Identifier::Pmid(_) => "PMID",
        }
    }

    /// As upstream's object (`{"DOI": "10..."}`).
    pub fn to_value(&self) -> Value {
        let mut m = serde_json::Map::new();
        m.insert(self.kind().to_owned(), Value::String(self.value().to_owned()));
        Value::Object(m)
    }

    /// The search item `Zotero.Translate.Search#setIdentifier` builds
    /// (translate.js:2746-2782).
    pub fn search_item(&self) -> JsObject {
        let mut o = JsObject::new();
        match self {
            Identifier::Doi(d) => {
                o.set("itemType", "journalArticle");
                o.set("DOI", d.as_str());
            }
            Identifier::Isbn(i) => {
                o.set("itemType", "book");
                o.set("ISBN", i.as_str());
            }
            Identifier::Pmid(p) => {
                o.set("itemType", "journalArticle");
                o.set("contextObject", format!("rft_id=info:pmid/{p}"));
            }
            Identifier::ArXiv(a) => {
                o.set("itemType", "journalArticle");
                o.set("arXiv", a.as_str());
            }
            Identifier::AdsBibcode(b) => {
                o.set("itemType", "journalArticle");
                o.set("adsBibcode", b.as_str());
            }
        }
        o
    }
}

fn is_digit(c: char) -> bool {
    c.is_ascii_digit()
}

fn is_letter(c: char) -> bool {
    c.is_ascii_alphabetic()
}

/// `Zotero.Utilities.extractIdentifiers(text)` (utilities.js:383-470).
pub fn extract_identifiers(text: &str) -> Vec<Identifier> {
    let mut ids: Vec<Identifier> = Vec::new();
    let mut found: Vec<String> = Vec::new();

    // DOIs: split on whitespace (the regex's [\s ]+; \s has U+00A0).
    for word in split_js_whitespace(text) {
        if let Some(doi) = clean_doi(word) {
            if !found.contains(&doi) {
                found.push(doi.clone());
                ids.push(Identifier::Doi(doi));
            }
        }
    }

    // ISBNs.
    if ids.is_empty() {
        let dashes = |c: char| matches!(c, '\u{2D}' | '\u{AD}' | '\u{2010}'..='\u{2015}' | '\u{2212}');
        let s: String = text.chars().filter(|&c| !dashes(c)).collect::<String>().to_uppercase();
        let chars: Vec<char> = s.chars().collect();
        for m in isbn_matches(&chars) {
            if let Some(isbn) = clean_isbn(&m, false) {
                if !found.contains(&isbn) {
                    found.push(isbn.clone());
                    ids.push(Identifier::Isbn(isbn));
                }
            }
        }
        if ids.is_empty() {
            let chars: Vec<char> = chars.into_iter().filter(|&c| c != ' ' && c != '\u{A0}').collect();
            for m in isbn_matches(&chars) {
                if let Some(isbn) = clean_isbn(&m, false) {
                    if !found.contains(&isbn) {
                        found.push(isbn.clone());
                        ids.push(Identifier::Isbn(isbn));
                    }
                }
            }
        }
    }

    let chars: Vec<char> = text.chars().collect();

    // arXiv.
    if ids.is_empty() {
        let mut last = 0;
        while let Some((end, id)) = next_arxiv(&chars, last) {
            if !found.contains(&id) {
                found.push(id.clone());
                ids.push(Identifier::ArXiv(id));
            }
            last = end;
        }
    }

    // ADS bibcodes (no de-duplication upstream).
    if ids.is_empty() {
        let mut last = 0;
        while let Some((end, b)) = next_bibcode(&chars, last) {
            ids.push(Identifier::AdsBibcode(b));
            last = end;
        }
    }

    // PMIDs (no de-duplication upstream).
    if ids.is_empty() {
        let mut last = 0;
        while let Some((end, p)) = next_pmid(&chars, last) {
            ids.push(Identifier::Pmid(p));
            last = end;
        }
    }
    ids
}

/// `text.split(/[\s ]+/)`.
fn split_js_whitespace(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut in_ws = false;
    for (i, c) in text.char_indices() {
        if js::is_space(c) {
            if !in_ws {
                out.push(&text[start..i]);
                in_ws = true;
            }
        } else if in_ws {
            start = i;
            in_ws = false;
        }
    }
    if in_ws {
        out.push("");
    } else {
        out.push(&text[start..]);
    }
    out
}

/// `/(?:\D|^)(97[89]\d{10}|\d{9}[\dX])(?!\d)/g` run with `exec` to the end:
/// group 1 of each match.
fn isbn_matches(c: &[char]) -> Vec<String> {
    let mut out = Vec::new();
    let mut last = 0;
    'outer: while last <= c.len() {
        for p in last..=c.len() {
            // Prefix: \D consumes a non-digit, else ^ at 0.
            let mut starts = Vec::new();
            if p < c.len() && !is_digit(c[p]) {
                starts.push(p + 1);
            }
            if p == 0 {
                starts.push(0);
            }
            for g in starts {
                let digits_at = |i: usize| i < c.len() && is_digit(c[i]);
                let not_digit_after = |i: usize| !digits_at(i);
                // 97[89]\d{10}
                if g + 13 <= c.len()
                    && c[g] == '9'
                    && c[g + 1] == '7'
                    && (c[g + 2] == '8' || c[g + 2] == '9')
                    && (g + 3..g + 13).all(digits_at)
                    && not_digit_after(g + 13)
                {
                    out.push(c[g..g + 13].iter().collect());
                    last = g + 13;
                    continue 'outer;
                }
                // \d{9}[\dX]
                if g + 10 <= c.len()
                    && (g..g + 9).all(digits_at)
                    && (is_digit(c[g + 9]) || c[g + 9] == 'X')
                    && not_digit_after(g + 10)
                {
                    out.push(c[g..g + 10].iter().collect());
                    last = g + 10;
                    continue 'outer;
                }
            }
        }
        break;
    }
    out
}

/// The next match of upstream's arXiv expression at or after `from`:
/// `((?:[^A-Za-z]|^)([\-A-Za-z\.]+\/\d{7})(?:(v[0-9]+)|)(?!\d))|
/// ((?:\D|^)(\d{4}\.\d{4,5})(?:(v[0-9]+)|)(?!\d))`; returns the match end
/// and `m[2] || m[5]`.
fn next_arxiv(c: &[char], from: usize) -> Option<(usize, String)> {
    let digit_at = |i: usize| i < c.len() && is_digit(c[i]);
    // After the identifier at `i`: `(?:(v[0-9]+)|)(?!\d)`; the end, or None.
    let tail = |i: usize| -> Option<usize> {
        if i < c.len() && c[i] == 'v' && digit_at(i + 1) {
            let mut j = i + 1;
            while digit_at(j) {
                j += 1;
            }
            return Some(j);
        }
        (!digit_at(i)).then_some(i)
    };
    for p in from..=c.len() {
        // Alternative 1: old style.
        let mut starts = Vec::new();
        if p < c.len() && !is_letter(c[p]) {
            starts.push(p + 1);
        }
        if p == 0 {
            starts.push(0);
        }
        for s in starts {
            let in_class = |ch: char| ch == '-' || ch == '.' || is_letter(ch);
            let mut e = s;
            while e < c.len() && in_class(c[e]) {
                e += 1;
            }
            if e > s && e < c.len() && c[e] == '/' && (e + 1..e + 8).all(digit_at) {
                if let Some(end) = tail(e + 8) {
                    return Some((end, c[s..e + 8].iter().collect()));
                }
            }
        }
        // Alternative 2: new style.
        let mut starts = Vec::new();
        if p < c.len() && !is_digit(c[p]) {
            starts.push(p + 1);
        }
        if p == 0 {
            starts.push(0);
        }
        for s in starts {
            if (s..s + 4).all(digit_at) && s + 4 < c.len() && c[s + 4] == '.' {
                for n in [5usize, 4] {
                    if (s + 5..s + 5 + n).all(digit_at) {
                        if let Some(end) = tail(s + 5 + n) {
                            return Some((end, c[s..s + 5 + n].iter().collect()));
                        }
                    }
                }
            }
        }
    }
    None
}

/// `/\b(\d{4}\D\S{13}[A-Z.:])\b/g`: the next match at or after `from`.
fn next_bibcode(c: &[char], from: usize) -> Option<(usize, String)> {
    let word = |i: usize| i < c.len() && js::is_word_char(c[i]);
    let boundary = |i: usize| (i > 0 && word(i - 1)) != word(i);
    for p in from..c.len() {
        if p + 19 > c.len() || !boundary(p) {
            continue;
        }
        let ok = (p..p + 4).all(|i| is_digit(c[i]))
            && !is_digit(c[p + 4])
            && (p + 5..p + 18).all(|i| !js::is_space(c[i]))
            && (c[p + 18].is_ascii_uppercase() || c[p + 18] == '.' || c[p + 18] == ':')
            && boundary(p + 19);
        if ok {
            return Some((p + 19, c[p..p + 19].iter().collect()));
        }
    }
    None
}

/// `/(^|\s|,|:)(\d{1,9})(?=\s|,|$)/g`: the next match at or after `from`.
fn next_pmid(c: &[char], from: usize) -> Option<(usize, String)> {
    let digit_run = |s: usize| {
        let mut e = s;
        while e < c.len() && is_digit(c[e]) {
            e += 1;
        }
        e
    };
    let after_ok = |i: usize| i == c.len() || js::is_space(c[i]) || c[i] == ',';
    for p in from..=c.len() {
        let mut starts = Vec::new();
        if p == 0 {
            starts.push(0);
        }
        if p < c.len() && (js::is_space(c[p]) || c[p] == ',' || c[p] == ':') {
            starts.push(p + 1);
        }
        for s in starts {
            let e = digit_run(s);
            if e > s && e - s <= 9 && after_ok(e) {
                return Some((e, c[s..e].iter().collect()));
            }
        }
    }
    None
}

/// What the translation-server's `/search` takes from the text
/// (searchEndpoint.js:40-47): the first identifier, except that a PMID
/// counts only when it is all the text (optionally `pmid:`-prefixed).
pub fn identifier_for_search(text: &str) -> Option<Identifier> {
    let ids = extract_identifiers(text);
    let first = ids.into_iter().next()?;
    if let Identifier::Pmid(p) = &first {
        if *p != pmid_only(text) {
            return None;
        }
    }
    Some(first)
}

/// `data.replace(/^\s*(?:pmid:)?([0-9]+)\s*$/, '$1')`.
fn pmid_only(text: &str) -> String {
    let t = js::trim(text);
    let core = t.strip_prefix("pmid:").unwrap_or(t);
    if !core.is_empty() && core.chars().all(|c| c.is_ascii_digit()) {
        core.to_owned()
    } else {
        text.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // zotero test/tests/utilities_internalTest.js #extractIdentifiers().
    #[test]
    fn upstream_tests() {
        assert_eq!(extract_identifiers("0838985890"), vec![Identifier::Isbn("0838985890".into())]);
        assert_eq!(
            extract_identifiers("978-0838985892"),
            vec![Identifier::Isbn("9780838985892".into())]
        );
        assert_eq!(
            extract_identifiers("978-0838985892 9781479347711 "),
            vec![
                Identifier::Isbn("9780838985892".into()),
                Identifier::Isbn("9781479347711".into())
            ]
        );
        assert_eq!(
            extract_identifiers("10.4103/0976-500X.85940"),
            vec![Identifier::Doi("10.4103/0976-500X.85940".into())]
        );
        assert_eq!(
            extract_identifiers("1 PMID:24297125,222 3-4 1234567890, 123456789"),
            ["1", "24297125", "222", "123456789"]
                .map(|s| Identifier::Pmid(s.into()))
                .to_vec()
        );
        assert_eq!(
            extract_identifiers(
                "0706.0044 arXiv:0706.00441v1,12345678,hep-ex/9809001v1, math.GT/0309135."
            ),
            ["0706.0044", "0706.00441", "hep-ex/9809001", "math.GT/0309135"]
                .map(|s| Identifier::ArXiv(s.into()))
                .to_vec()
        );
        assert_eq!(
            extract_identifiers("9 2021wfc..rept....8D, 2022MSSP..16208010Y."),
            ["2021wfc..rept....8D", "2022MSSP..16208010Y"]
                .map(|s| Identifier::AdsBibcode(s.into()))
                .to_vec()
        );
    }

    #[test]
    fn pmid_counts_only_alone() {
        assert_eq!(identifier_for_search(" pmid:123 "), Some(Identifier::Pmid("123".into())));
        assert_eq!(identifier_for_search("123 456"), None);
        assert_eq!(
            identifier_for_search("arXiv:1706.03762"),
            Some(Identifier::ArXiv("1706.03762".into()))
        );
    }
}
