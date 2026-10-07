// Part of the kovan Zotero port (GitHub #747, #749).
//
// Upstream: Zotero translate, https://github.com/zotero/translate (commit
//   dd524aea9a55): src/rdf/uri.js `$rdf.Util.uri.join` :21-70 (from
//   http://www.w3.org/2005/10/ajaw/uri.js, "2005 W3C open source licence";
//   see NOTICE).
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA;
//   (c) 2005 World Wide Web Consortium (MIT, ERCIM, Keio).
// Licence: AGPL-3.0 (upstream translate: AGPL-3.0-or-later; uri.js: W3C
//   Software Notice and License, GPL-compatible).

//! `$rdf.Util.uri.join`: resolve a URI against a base the way the RDF parser
//! does (not RFC 3986: the AJAW library's own rules, ported as they are).
//! Indices are byte offsets; every character the function searches for is
//! ASCII, so they agree with upstream's UTF-16 offsets on the same text.

use regex::Regex;
use std::sync::OnceLock;

/// `join(given, base)`.
pub fn join(given: &str, base: &str) -> String {
    let base = match base.find('#') {
        Some(h) if h > 0 => &base[..h],
        _ => base,
    };
    if given.is_empty() {
        return base.to_owned();
    }
    if given.starts_with('#') {
        return format!("{base}{given}");
    }
    if given.contains(':') {
        return given.to_owned();
    }
    if base.is_empty() {
        return given.to_owned();
    }
    let Some(base_colon) = base.find(':') else {
        // "Invalid base" (logged upstream).
        return given.to_owned();
    };
    let base_scheme = &base[..=base_colon];
    if given.starts_with("//") {
        return format!("{base_scheme}{given}");
    }
    let base_single;
    if base[base_colon..].find("//") == Some(1) {
        // `base.indexOf('//', baseColon) == baseColon + 1`: a host part.
        match find_from(base, "/", base_colon + 3) {
            Some(i) => base_single = i,
            None => {
                return if base.len() as isize - base_colon as isize - 3 > 0 {
                    format!("{base}/{given}")
                } else {
                    format!("{base_scheme}{given}")
                };
            }
        }
    } else {
        match find_from(base, "/", base_colon + 1) {
            Some(i) => base_single = i,
            None => {
                return if base.len() - base_colon - 1 > 0 {
                    format!("{base}/{given}")
                } else {
                    format!("{base_scheme}{given}")
                };
            }
        }
    }
    if given.starts_with('/') {
        return format!("{}{given}", &base[..base_single]);
    }
    let mut path = base[base_single..].to_owned();
    let Some(last_slash) = path.rfind('/') else {
        return format!("{base_scheme}{given}");
    };
    if last_slash < path.len() - 1 {
        path.truncate(last_slash + 1);
    }
    path.push_str(given);
    static UP: OnceLock<Regex> = OnceLock::new();
    let up = UP.get_or_init(|| Regex::new(r"[^/]*/\.\./").unwrap());
    while up.is_match(&path) {
        path = up.replace(&path, "").into_owned();
    }
    path = path.replace("./", "");
    if path.ends_with("/.") {
        path.truncate(path.len() - 1);
    }
    format!("{}{path}", &base[..base_single])
}

/// `s.indexOf(pat, from)` (byte offsets; `from` past the end finds nothing
/// unless `pat` is empty).
fn find_from(s: &str, pat: &str, from: usize) -> Option<usize> {
    if from > s.len() || !s.is_char_boundary(from) {
        return None;
    }
    s[from..].find(pat).map(|i| i + from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_cases() {
        assert_eq!(join("#x", ""), "#x");
        assert_eq!(join("x", ""), "x");
        assert_eq!(join("http://a/b", "http://c/"), "http://a/b");
        assert_eq!(join("c", "http://a/b"), "http://a/c");
        assert_eq!(join("../c", "http://a/b/d"), "http://a/c");
        assert_eq!(join("/c", "http://a/b/d"), "http://a/c");
        assert_eq!(join("#f", "http://a/b#g"), "http://a/b#f");
        assert_eq!(join("c", "http://a"), "http://a/c");
    }
}
