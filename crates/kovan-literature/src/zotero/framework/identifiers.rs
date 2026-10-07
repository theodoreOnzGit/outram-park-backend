// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero utilities, https://github.com/zotero/utilities (commit
//   4051881d59c6, the translation-server's submodule for the references):
//   utilities.js `cleanISBN` :532-566, `cleanISSN` :603-627.
// Copyright (c) 2009 Center for History and New Media, George Mason
//   University, Fairfax, Virginia, USA; Corporation for Digital Scholarship.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! `ZU.cleanISBN` and `ZU.cleanISSN`.
//!
//! Upstream matches with JavaScript regular expressions whose `\b` is an
//! ASCII word boundary and whose `\s` is JavaScript whitespace. The port
//! runs the same patterns over an ASCII stand-in of the text with one
//! character per character (JavaScript whitespace becomes a space, any
//! other non-ASCII character `!`, neither of which is a word character),
//! so `\b`, `\d` and `\s` mean what they mean upstream, and reads the match
//! back from the original text by position.

use super::js;
use regex::Regex;
use std::sync::OnceLock;

/// The dashes both functions ignore: `[\x2D\xAD‐-―⁃−]+`.
fn strip_dashes(s: &str) -> String {
    s.chars()
        .filter(|&c| {
            !matches!(
                c,
                '\u{2D}' | '\u{AD}' | '\u{2010}'..='\u{2015}' | '\u{2043}' | '\u{2212}'
            )
        })
        .collect()
}

/// The ASCII stand-in described in the module docs.
fn ascii_stand_in(chars: &[char]) -> String {
    chars
        .iter()
        .map(|&c| {
            if js::is_space(c) {
                ' '
            } else if c.is_ascii() {
                c
            } else {
                '!'
            }
        })
        .collect()
}

/// `exec` in a loop with `lastIndex` reset to `match.index + 1` whenever
/// `accept` rejects the match (the retry both functions do); returns what
/// `accept` returned for the first accepted match.
fn scan(s: &str, re: &Regex, mut accept: impl FnMut(&str) -> Option<String>) -> Option<String> {
    let chars: Vec<char> = s.chars().collect();
    let ascii = ascii_stand_in(&chars);
    let mut last = 0;
    while last <= ascii.len() {
        let m = re.find_at(&ascii, last)?;
        // `isbnMatch[0].replace(/\s+/g, '')`; ASCII positions are char
        // positions.
        let matched: String = chars[m.start()..m.end()]
            .iter()
            .filter(|c| !js::is_space(**c))
            .collect();
        if let Some(r) = accept(&matched) {
            return Some(r);
        }
        last = m.start() + 1;
    }
    None
}

fn digit(c: u8) -> u32 {
    u32::from(c - b'0')
}

/// `Zotero.Utilities.cleanISBN(isbnStr, dontValidate)` (:532-566): the first
/// ISBN-10 or ISBN-13 in the text whose check digit is valid (any, with
/// `dont_validate`), without spaces and dashes; `None` (upstream `false`)
/// when there is none.
pub fn clean_isbn(isbn_str: &str, dont_validate: bool) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"\b(?:97[89] *(?:[0-9] *){9}[0-9]|(?:[0-9] *){9}[0-9X])\b").unwrap()
    });
    let s = strip_dashes(&isbn_str.to_uppercase());
    scan(&s, re, |isbn| {
        if dont_validate {
            return Some(isbn.to_owned());
        }
        let b = isbn.as_bytes();
        if b.len() == 10 {
            let mut sum = 0;
            for (i, &c) in b.iter().take(9).enumerate() {
                sum += digit(c) * (10 - i as u32);
            }
            sum += if b[9] == b'X' { 10 } else { digit(b[9]) };
            (sum % 11 == 0).then(|| isbn.to_owned())
        } else {
            let mut sum = 0;
            for i in (0..12).step_by(2) {
                sum += digit(b[i]);
            }
            for i in (1..12).step_by(2) {
                sum += digit(b[i]) * 3;
            }
            sum += digit(b[12]);
            (sum % 10 == 0).then(|| isbn.to_owned())
        }
    })
}

/// `Zotero.Utilities.cleanISSN(issnStr)` (:603-627): the first ISSN in the
/// text with a valid check digit, as `NNNN-NNNN`; `None` (upstream `false`)
/// when there is none.
pub fn clean_issn(issn_str: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"\b(?:[0-9] *){7}[0-9X]\b").unwrap());
    let s = strip_dashes(&issn_str.to_uppercase());
    scan(&s, re, |issn| {
        let b = issn.as_bytes();
        let mut sum = 0;
        for (i, &c) in b.iter().take(7).enumerate() {
            sum += digit(c) * (8 - i as u32);
        }
        sum += if b[7] == b'X' { 10 } else { digit(b[7]) };
        (sum % 11 == 0).then(|| format!("{}-{}", &issn[..4], &issn[4..]))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isbn_and_issn() {
        assert_eq!(
            clean_isbn("978-3-16-148410-0", false).as_deref(),
            Some("9783161484100")
        );
        assert_eq!(clean_isbn("0-306-40615-3", false), None);
        assert_eq!(
            clean_isbn("0-306-40615-3", true).as_deref(),
            Some("0306406153")
        );
        assert_eq!(clean_issn("issn 0378-5955").as_deref(), Some("0378-5955"));
        assert_eq!(clean_issn("1234-5678"), None);
    }
}
