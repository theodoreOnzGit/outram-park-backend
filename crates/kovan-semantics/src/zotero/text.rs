// Part of the kovan Zotero port (GitHub #747, #751).
//
// Upstream: Zotero utilities, https://github.com/zotero/utilities (commit
// 4051881d59c6), `utilities.js`: `cleanISBN` (532), `toISBN13` (576),
// `removeDiacritics` (1153); Zotero, https://github.com/zotero/zotero (commit
// 9cbba8c4d281), `chrome/content/zotero/xpcom/duplicates.js`
// `normalizeString` (110).
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0. See this crate's NOTICE, "Upstream: Zotero".

//! The string helpers the duplicate finder and the merge use, with
//! JavaScript's semantics where they differ from Rust's: `String#trim`'s
//! whitespace set, `<` on strings (UTF-16 code units), `\s`, `\b` and `\d`
//! in a non-Unicode regex.

use std::cmp::Ordering;

use super::diacritics_generated::{LOWERCASE, UPPERCASE};

/// Whether `c` is JavaScript whitespace (`WhiteSpace` + `LineTerminator`,
/// ECMA-262 12.2/12.3): what `String#trim` strips and `\s` matches.
/// Unlike [`char::is_whitespace`] it includes U+FEFF and excludes U+0085.
pub fn is_js_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{9}' | '\u{A}' | '\u{B}' | '\u{C}' | '\u{D}' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

/// JavaScript `String#trim`.
pub fn js_trim(s: &str) -> &str {
    s.trim_matches(is_js_whitespace)
}

/// JavaScript `a < b` on two strings: UTF-16 code-unit order (which differs
/// from Rust's `str` order for supplementary-plane characters).
pub fn cmp_utf16(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// The length of `s` in UTF-16 code units (JavaScript `String#length`).
pub fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// `Zotero.Utilities.removeDiacritics(str)` (utilities.js:1153), both passes
/// (lowercase, then uppercase).
///
/// Upstream runs one global regex replacement per map entry, in order. Every
/// base is plain ASCII and maps to itself (tested), so that sequence equals
/// a per-character lookup: the first lowercase entry containing the
/// character, else the first uppercase one, else the character itself.
pub fn remove_diacritics(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match base_of(c) {
            Some(b) => out.push_str(b),
            None => out.push(c),
        }
    }
    out
}

fn base_of(c: char) -> Option<&'static str> {
    LOWERCASE
        .iter()
        .chain(UPPERCASE.iter())
        .find(|(_, letters)| letters.contains(&c))
        .map(|(b, _)| *b)
}

/// `normalizeString` inside `Zotero.Duplicates._findDuplicates`
/// (duplicates.js:110): remove diacritics, turn runs of ASCII space and
/// punctuation (`[ !-/:-@[-`{-~]`) into one space, trim, lower-case.
pub fn normalize_string(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    let stripped = remove_diacritics(s);
    let mut out = String::with_capacity(stripped.len());
    let mut in_run = false;
    for c in stripped.chars() {
        let punct = matches!(c, ' '..='/' | ':'..='@' | '['..='`' | '{'..='~');
        if punct {
            if !in_run {
                out.push(' ');
            }
            in_run = true;
        } else {
            out.push(c);
            in_run = false;
        }
    }
    js_trim(&out).to_lowercase()
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Try `97[89]\s*(?:\d\s*){9}\d` (`thirteen`) or `(?:\d\s*){9}[\dX]` at
/// `chars[p..]`; the end index of the match, before the trailing `\b` check.
fn match_isbn_at(chars: &[char], p: usize, thirteen: bool) -> Option<usize> {
    let mut i = p;
    let skip_ws = |i: &mut usize| {
        while *i < chars.len() && is_js_whitespace(chars[*i]) {
            *i += 1;
        }
    };
    if thirteen {
        if chars.get(i) != Some(&'9') || chars.get(i + 1) != Some(&'7') {
            return None;
        }
        if !matches!(chars.get(i + 2), Some('8' | '9')) {
            return None;
        }
        i += 3;
        skip_ws(&mut i);
    }
    for _ in 0..9 {
        if !chars.get(i).is_some_and(|c| c.is_ascii_digit()) {
            return None;
        }
        i += 1;
        skip_ws(&mut i);
    }
    match chars.get(i) {
        Some(c) if c.is_ascii_digit() => Some(i + 1),
        Some('X') if !thirteen => Some(i + 1),
        _ => None,
    }
}

fn boundary(chars: &[char], i: usize) -> bool {
    let before = i > 0 && is_word(chars[i - 1]);
    let after = i < chars.len() && is_word(chars[i]);
    before != after
}

fn digit(c: u8) -> u32 {
    (c - b'0') as u32
}

/// `Zotero.Utilities.cleanISBN(isbnStr, dontValidate)` (utilities.js:532):
/// the first ISBN-10/13 in the string (dashes ignored, whitespace inside
/// it removed) whose check digit is valid, or any first match when
/// `dont_validate`. `None` is upstream's `false`.
pub fn clean_isbn(isbn_str: &str, dont_validate: bool) -> Option<String> {
    let upper: String = isbn_str
        .to_uppercase()
        .chars()
        .filter(|c| {
            !matches!(
                c,
                '\u{2D}' | '\u{AD}' | '\u{2010}'..='\u{2015}' | '\u{2043}' | '\u{2212}'
            )
        })
        .collect();
    let chars: Vec<char> = upper.chars().collect();
    // `isbnRE.exec` from `lastIndex`; on a failed checksum upstream retries
    // from the match index + 1, which is the same as scanning every start.
    for p in 0..chars.len() {
        if !boundary(&chars, p) {
            continue;
        }
        let end = [true, false]
            .into_iter()
            .find_map(|t| match_isbn_at(&chars, p, t).filter(|&e| boundary(&chars, e)));
        let Some(end) = end else { continue };
        let isbn: String = chars[p..end]
            .iter()
            .filter(|c| !is_js_whitespace(**c))
            .collect();
        if dont_validate {
            return Some(isbn);
        }
        let b = isbn.as_bytes();
        if b.len() == 10 {
            let mut sum: u32 = (0..9).map(|i| digit(b[i]) * (10 - i as u32)).sum();
            sum += if b[9] == b'X' { 10 } else { digit(b[9]) };
            if sum.is_multiple_of(11) {
                return Some(isbn);
            }
        } else {
            let mut sum: u32 = (0..12)
                .map(|i| digit(b[i]) * if i % 2 == 1 { 3 } else { 1 })
                .sum();
            sum += digit(b[12]);
            if sum.is_multiple_of(10) {
                return Some(isbn);
            }
        }
    }
    None
}

/// `Zotero.Utilities.toISBN13(isbnStr)` (utilities.js:576): the ISBN-13 of
/// the first ISBN in the string (check digit recomputed, not validated).
/// `None` where upstream throws ("ISBN not found").
pub fn to_isbn13(isbn_str: &str) -> Option<String> {
    let isbn = clean_isbn(isbn_str, true)?;
    let mut head = if isbn.len() == 13 {
        isbn[..12].to_owned()
    } else {
        format!("978{}", &isbn[..9])
    };
    let sum: u32 = head
        .bytes()
        .enumerate()
        .map(|(i, c)| digit(c) * if i % 2 == 1 { 3 } else { 1 })
        .sum();
    let check = (10 - sum % 10) % 10;
    head.push(char::from_digit(check, 10).unwrap_or('0'));
    Some(head)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_base_is_a_fixed_point_so_per_char_lookup_equals_upstreams_sequence() {
        for (base, _) in LOWERCASE.iter().chain(UPPERCASE.iter()) {
            assert_eq!(remove_diacritics(base), *base, "{base}");
        }
    }

    #[test]
    fn diacritics_removed() {
        assert_eq!(
            remove_diacritics("Ångström Æsir œuvre"),
            "Angstrom AEsir oeuvre"
        );
    }

    #[test]
    fn normalize_string_matches_upstream_rules() {
        assert_eq!(
            normalize_string("  Effective--Java: 3rd ed.! "),
            "effective java 3rd ed"
        );
        assert_eq!(normalize_string("Über"), "uber");
        assert_eq!(normalize_string("!!!"), "");
    }

    // utilities test/tests/utilitiesTest.js has the cleanISBN cases this
    // port is checked against in tests/zotero_upstream.rs; these are the
    // branch checks.
    #[test]
    fn isbn_checksums() {
        assert_eq!(
            clean_isbn("0134685997", false).as_deref(),
            Some("0134685997")
        );
        assert_eq!(
            clean_isbn("978-0-13-468599-1", false).as_deref(),
            Some("9780134685991")
        );
        assert_eq!(clean_isbn("0134685998", false), None);
        assert_eq!(
            clean_isbn("0134685998", true).as_deref(),
            Some("0134685998")
        );
        assert_eq!(to_isbn13("0134685997").as_deref(), Some("9780134685991"));
        assert_eq!(to_isbn13("9780134685991").as_deref(), Some("9780134685991"));
        assert_eq!(to_isbn13("no isbn"), None);
    }

    #[test]
    fn utf16_order_differs_from_utf8_for_astral() {
        // U+FF61 (BMP, high) vs U+1F600 (astral, surrogate D83D): UTF-8
        // says FF61 < 1F600, UTF-16 says the opposite.
        assert_eq!(cmp_utf16("\u{1F600}", "\u{FF61}"), Ordering::Less);
        assert!("\u{1F600}" > "\u{FF61}");
    }
}
