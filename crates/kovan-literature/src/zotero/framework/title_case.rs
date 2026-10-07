// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero utilities, https://github.com/zotero/utilities (commit
//   4051881d59c6): utilities.js `capitalizeTitle` :1067-1138; Zotero
//   translate (commit dd524aea9a55) src/utilities_translate.js
//   `Zotero.Utilities.Translate.prototype.capitalizeTitle` :66-78.
// Copyright (c) 2009 Center for History and New Media, George Mason
//   University, Fairfax, Virginia, USA; Corporation for Digital Scholarship.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! `ZU.capitalizeTitle(string, force)`.
//!
//! In a translator, `ZU.capitalizeTitle` without `force` reads the
//! `capitalizeTitles` preference, which the translation-server does not set
//! (its `Zotero.Prefs.get` reads the server config): so there it only
//! normalises whitespace and `" : "`. [`capitalize_title`] takes `force`
//! as the caller passes it, with `false` meaning that server behaviour.
//!
//! Lengths and offsets count `char`s where upstream counts UTF-16 code
//! units; the two differ only on astral-plane characters.

use super::js;
use super::utilities::trim_internal;
use regex::Regex;
use std::sync::OnceLock;

/// `skipWords` (:1068-1070).
const SKIP_WORDS: [&str; 22] = [
    "but", "or", "yet", "so", "for", "and", "nor", "a", "an", "the", "at", "by", "from", "in",
    "into", "of", "on", "to", "with", "up", "down", "as",
];

/// `delimiterRegexp` (:1073): one character.
fn is_delimiter(c: char) -> bool {
    matches!(
        c,
        ' ' | '/' | '\u{2D}' | '\u{AD}' | '\u{2010}'
            ..='\u{2015}' | '\u{2212}' | '\u{2E3A}' | '\u{2E3B}'
    )
}

/// JavaScript `str.toUpperCase()`.
fn upper(s: &str) -> String {
    s.to_uppercase()
}

/// JavaScript `str.toLowerCase()`.
fn lower(s: &str) -> String {
    s.to_lowercase()
}

/// `Zotero.Utilities.capitalizeTitle(string, force)` (:1067-1138) as a
/// translator on the translation-server runs it (module docs).
pub fn capitalize_title(string: &str, force: bool) -> String {
    let string = trim_internal(string).replace(" : ", ": ");
    if !force {
        return string;
    }
    if string.is_empty() {
        return String::new();
    }

    // Remove HTML tags, remembering their positions in the original string.
    static TAG: OnceLock<Regex> = OnceLock::new();
    let tag = TAG.get_or_init(|| Regex::new("<[^>]+>").unwrap());
    let mut html_tags: Vec<(String, usize)> = Vec::new();
    let mut cleaned = String::new();
    let mut last = 0;
    for m in tag.find_iter(&string) {
        cleaned.push_str(&string[last..m.start()]);
        html_tags.push((m.as_str().to_owned(), string[..m.start()].chars().count()));
        last = m.end();
    }
    cleaned.push_str(&string[last..]);

    // `cleanedString.split(delimiterRegexp)` with the capturing group: the
    // pieces and the delimiters between them.
    let mut words: Vec<String> = vec![String::new()];
    for c in cleaned.chars() {
        if is_delimiter(c) {
            words.push(c.to_string());
            words.push(String::new());
        } else {
            words.last_mut().unwrap().push(c);
        }
    }
    let is_upper_case = upper(&cleaned) == cleaned;

    static PUNCT: OnceLock<Regex> = OnceLock::new();
    let punct_re =
        PUNCT.get_or_init(|| Regex::new(&format!("^(?:['\"¡¿“‘„«]|{})+", js::WS)).unwrap());
    static NON_ALPHA: OnceLock<Regex> = OnceLock::new();
    let non_alpha = NON_ALPHA.get_or_init(|| Regex::new("[^a-zA-Z]+").unwrap());

    let mut new_string = String::new();
    let last_word_index = words.len() - 1;
    let mut previous_word_index: Option<usize> = None;
    for i in 0..=last_word_index {
        let w = words[i].clone();
        let n = w.chars().count();
        let single_delim = n == 1 && is_delimiter(w.chars().next().unwrap());
        if n != 0 && !single_delim {
            let lower_variant = lower(&w);
            if is_upper_case || w == lower_variant {
                let stripped = non_alpha.replace(&lower_variant, "");
                let after_colon = previous_word_index.is_some_and(|p| {
                    words[p]
                        .chars()
                        .last()
                        .is_some_and(|c| matches!(c, ':' | '?' | '!'))
                });
                if SKIP_WORDS.contains(&stripped.as_ref())
                    && i != 0
                    && i != last_word_index
                    && !after_colon
                {
                    words[i] = lower_variant;
                } else {
                    let punct = punct_re
                        .find(&w)
                        .map_or(1, |m| m.as_str().chars().count() + 1);
                    let head: String = w.chars().take(punct).collect();
                    let tail: String = w.chars().skip(punct).collect();
                    words[i] = upper(&head) + &lower(&tail);
                }
            }
            previous_word_index = Some(i);
        }
        new_string.push_str(&words[i]);
    }

    // Reinsert the HTML tags at their original positions.
    for (m, offset) in html_tags {
        let chars: Vec<char> = new_string.chars().collect();
        let at = offset.min(chars.len());
        let mut s: String = chars[..at].iter().collect();
        s.push_str(&m);
        s.extend(&chars[at..]);
        new_string = s;
    }
    new_string
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_case() {
        assert_eq!(
            capitalize_title("the art of war: a new translation", true),
            "The Art of War: A New Translation"
        );
        assert_eq!(capitalize_title("  a  :  b  ", false), "a: b");
        assert_eq!(
            capitalize_title("an <i>example</i> of the title", true),
            "An <i>Example</i> of the Title"
        );
    }
}
