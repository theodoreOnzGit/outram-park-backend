// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translators, https://github.com/zotero/translators
//   (commit 3d1c78530f42): BibTeX.js — `splitUnprotected` :517-570,
//   `parseFilePathRecord` :572-609, `unescapeBibTeX` :665-718,
//   `xcase`/`sup`/`sub`/`mapTeXmarkup` :1115-1135,
//   `decodeFilePathComponent` :1206-1209.
// Copyright (C) 2019 CHNM, Simon Kornblith, Richard Karnesky and Emiliano
//   heyns.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! BibTeX import's text helpers: splitting outside braces, LaTeX to
//! Unicode/HTML (`unescapeBibTeX`), JabRef/Mendeley file records.

use super::reverse_mapping_table::TABLE as REVERSE_MAPPING;
use crate::zotero::framework::html::unescape_html;
use crate::zotero::framework::js;
use crate::zotero::framework::JsObject;
use regex::{Captures, Regex};
use std::sync::OnceLock;

fn re(cell: &'static OnceLock<Regex>, pattern: impl FnOnce() -> String) -> &'static Regex {
    cell.get_or_init(|| Regex::new(&pattern()).expect("static regex compiles"))
}

/// `splitUnprotected(str, delim)` (:517-570): split on `delim` outside
/// braces, a backslash protecting the next character.
pub(super) fn split_unprotected(s: &str, delim: &Regex) -> Vec<String> {
    let c: Vec<char> = s.chars().collect();
    // delim.exec in sequence: the global matches, as (char index, char length).
    let byte_to_char = |b: usize| s[..b].chars().count();
    let matches: Vec<(usize, usize)> = delim
        .find_iter(s)
        .map(|m| (byte_to_char(m.start()), m.as_str().chars().count()))
        .collect();
    let mut next = 0usize; // index into matches of nextPossibleSplit
    if matches.is_empty() {
        return vec![s.to_owned()];
    }
    let mut parts = Vec::new();
    let mut open = 0i32;
    let mut part_start = 0usize;
    let mut i = 0usize;
    let substr = |from: usize| -> String { c[from.min(c.len())..].iter().collect() };
    while i < c.len() {
        if i > matches[next].0 {
            next += 1;
            if next >= matches.len() {
                parts.push(substr(part_start));
                return parts;
            }
        }
        if c[i] == '\\' {
            i += 2;
            continue;
        }
        if c[i] == '{' {
            open += 1;
            i += 1;
            continue;
        }
        if c[i] == '}' {
            open -= 1;
            if open < 0 {
                open = 0;
            }
            i += 1;
            continue;
        }
        if open != 0 {
            i += 1;
            continue;
        }
        if i == matches[next].0 {
            parts.push(c[part_start..i].iter().collect());
            i += matches[next].1 - 1;
            part_start = i + 1;
            next += 1;
            if next >= matches.len() {
                parts.push(substr(part_start));
                return parts;
            }
        }
        i += 1;
    }
    let last = substr(part_start);
    let last = js::trim(&last);
    if !last.is_empty() {
        parts.push(last.to_owned());
    }
    parts
}

/// `decodeFilePathComponent` (:1206-1209).
pub(super) fn decode_file_path_component(value: &str) -> String {
    static R: OnceLock<Regex> = OnceLock::new();
    if value.is_empty() {
        return String::new();
    }
    re(&R, || r"\\([^A-Za-z0-9.])".into())
        .replace_all(value, "$1")
        .into_owned()
}

/// `parseFilePathRecord` (:572-609).
pub(super) fn parse_file_path_record(record: &[char]) -> Option<JsObject> {
    let mut start = 0;
    let mut fields: Vec<String> = Vec::new();
    let mut i = 0;
    while i < record.len() {
        if record[i] == '\\' {
            i += 2;
            continue;
        }
        if record[i] == ':' {
            fields.push(decode_file_path_component(
                &record[start..i].iter().collect::<String>(),
            ));
            start = i + 1;
        }
        i += 1;
    }
    fields.push(decode_file_path_component(
        &record[start.min(record.len())..].iter().collect::<String>(),
    ));
    if fields.len() != 3 && fields.len() != 1 {
        return None;
    }
    let mut a = JsObject::new();
    let path;
    if fields.len() == 3 {
        let t = js::trim(&fields[0]);
        a.set("title", if t.is_empty() { "Attachment" } else { t });
        path = fields[1].clone();
        let mut mime = fields[2].clone();
        if mime.to_lowercase().contains("pdf") {
            mime = "application/pdf".into();
        }
        a.set("path", path.clone());
        a.set("mimeType", mime);
    } else {
        a.set("title", "Attachment");
        path = fields[0].clone();
        a.set("path", path.clone());
    }
    let p = js::trim(&path).to_owned();
    if p.is_empty() {
        return None;
    }
    a.set("path", p);
    Some(a)
}

/// `xcase` (:1115-1117), for `sup`/`sub`.
pub(super) fn xcase(prefix: &str, cased: &str, tag: &str, tex: &str) -> String {
    let mut s = if prefix.is_empty() {
        String::new()
    } else {
        format!("${prefix}$")
    };
    let key = format!("${tex}{{{cased}}}$");
    match REVERSE_MAPPING
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| *v)
        .filter(|v| !v.is_empty())
    {
        Some(v) => s.push_str(v),
        None => s.push_str(&format!("<{tag}>{cased}</{tag}>")),
    }
    s
}

/// `mapTeXmarkup` (:1124-1135).
pub(super) fn map_tex_markup(tex: &str) -> String {
    static IT: OnceLock<Regex> = OnceLock::new();
    static BF: OnceLock<Regex> = OnceLock::new();
    static SUB1: OnceLock<Regex> = OnceLock::new();
    static SUB2: OnceLock<Regex> = OnceLock::new();
    static SUP1: OnceLock<Regex> = OnceLock::new();
    static SUP2: OnceLock<Regex> = OnceLock::new();
    static SC: OnceLock<Regex> = OnceLock::new();
    let t = re(&IT, || r"\\textit\{([^\}]+\})".into()).replace_all(tex, "<i>${1}</i>");
    let t = re(&BF, || r"\\textbf\{([^\}]+\})".into()).replace_all(&t, "<b>${1}</b>");
    let sub = |c: &Captures| xcase(&c[1], &c[2], "sub", "_");
    let sup = |c: &Captures| xcase(&c[1], &c[2], "sup", "^");
    let t = re(&SUB1, || r"\$([^\{\$]*)_\{([^\}]+)\}\$".into()).replace_all(&t, sub);
    let t = re(&SUB2, || {
        r"\$([^\{\$]*)_\{\\textrm\{([^\}\$]+)\}\}\$".into()
    })
    .replace_all(&t, sub);
    let t = re(&SUP1, || r"\$([^\{\$]*)\^\{([^\}]+)\}\$".into()).replace_all(&t, sup);
    let t = re(&SUP2, || r"\$([^\{\$]*)\^\{\\textrm\{([^\}]+)\}\}\$".into()).replace_all(&t, sup);
    let t = re(&SC, || r"\\textsc\{([^\}]+)".into())
        .replace_all(&t, "<span style=\"small-caps\">${1}</span>");
    t.into_owned()
}

/// `unescapeBibTeX(value)` (:665-718).
pub(super) fn unescape_bibtex(value: &str, in_child_translator: bool) -> String {
    static ACC: OnceLock<Regex> = OnceLock::new();
    static CARON: OnceLock<Regex> = OnceLock::new();
    static MATH: OnceLock<Regex> = OnceLock::new();
    static BRACES: OnceLock<Regex> = OnceLock::new();
    static BS: OnceLock<Regex> = OnceLock::new();
    static DBS: OnceLock<Regex> = OnceLock::new();
    static SPACE: OnceLock<Regex> = OnceLock::new();
    static ENT: OnceLock<Regex> = OnceLock::new();
    if js::len(value) < 2 {
        return value.to_owned();
    }
    let v =
        re(&ACC, || r#"\{?(\\[`"'^~=])\{?\\?([A-Za-z])\}"#.into()).replace_all(value, "{${1}${2}}");
    let v = re(&CARON, || r"(\\[a-z])\{(\\?[A-Za-z])\}".into()).replace_all(&v, "{${1} ${2}}");
    let mut v = map_tex_markup(&v);
    for (mapped, unicode) in REVERSE_MAPPING {
        while v.contains(mapped) {
            v = v.replacen(mapped, unicode, 1);
        }
        let mapped2: String = mapped.chars().filter(|c| *c != '{' && *c != '}').collect();
        while v.contains(&mapped2) {
            v = v.replacen(&mapped2, unicode, 1);
        }
    }
    let v = re(&MATH, || r"\$([^$]+)\$".into()).replace_all(&v, "${1}");
    // kill braces
    let v = re(&BRACES, || r"([^\\])[{}]+".into()).replace_all(&v, "${1}");
    let mut v = v.into_owned();
    if v.starts_with('{') {
        v.remove(0);
    }
    // chop off backslashes
    let bs = re(&BS, || r"([^\\])\\([#$%&~_^\\{}])".into());
    v = bs.replace_all(&v, "${1}${2}").into_owned();
    v = bs.replace_all(&v, "${1}${2}").into_owned();
    const SPECIAL: &str = "#$%&~_^\\{}";
    let chars: Vec<char> = v.chars().collect();
    if chars.first() == Some(&'\\') && chars.get(1).is_some_and(|c| SPECIAL.contains(*c)) {
        v = chars[1..].iter().collect();
    }
    let chars: Vec<char> = v.chars().collect();
    let n = chars.len();
    if n >= 2 && chars[n - 1] == '\\' && SPECIAL.contains(chars[n - 2]) {
        v = chars[..n - 1].iter().collect();
    }
    let v = re(&DBS, || r"\\\\".into()).replace_all(&v, r"\");
    let v = re(&SPACE, || format!("{}+", js::WS)).replace_all(&v, " ");
    let mut v = v.into_owned();
    if in_child_translator && v.contains('&') {
        v = re(&ENT, || r"&#?[A-Za-z0-9_]+;".into())
            .replace_all(&v, |c: &Captures| {
                let entity = &c[0];
                let mut ch = unescape_html(entity);
                if ch == entity {
                    ch = unescape_html(&entity.to_lowercase());
                }
                ch
            })
            .into_owned();
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_unprotected_respects_braces() {
        let and = Regex::new(&format!("(?i){ws}+and{ws}+", ws = js::WS)).unwrap();
        assert_eq!(
            split_unprotected("A and {B and C} AND D", &and),
            vec!["A", "{B and C}", "D"]
        );
        assert_eq!(split_unprotected("solo", &and), vec!["solo"]);
    }

    #[test]
    fn unescape_accents_and_braces() {
        assert_eq!(unescape_bibtex(r#"M{\"u}ller"#, false), "Müller");
        // Expected values from running upstream's unescapeBibTeX (node, 2026-10-07).
        assert_eq!(unescape_bibtex(r"{T}he \emph{x}", false), "The \\emphx");
        assert_eq!(unescape_bibtex(r"50\% \& more", false), "50% & more");
    }
}
