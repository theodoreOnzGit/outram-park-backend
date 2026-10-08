// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_names.js
// Version:     2.4.63, commit 73bc1b44bc7d54d0bfec4e070fd27f5efe024ff9
// Copyright:   (c) 2009-2019 Frank Bennett
// Licence:     AGPL-3.0, taken from upstream's "CPAL-1.0 or AGPL-3.0-or-later"
//              (LICENSE at the commit above; see this crate's NOTICE).
// Modified:    2026-10-08, by the OUTRAM PARK contributors. This file is a
//              Rust translation (port) of the file named above, modified
//              from the original.
// No warranty: this program is distributed in the hope that it will be
//              useful, but WITHOUT ANY WARRANTY; without even the implied
//              warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
//              PURPOSE. See the GNU Affero General Public License.


//! Port of `src/util_names.js`: `CSL.Util.Names`, the pure string helpers
//! that initialise, un-initialise and compare personal names.
//!
//! Differential tests against citeproc-js 2.4.63: see the `tests` module
//! (reference `tests/data/csl/units/names_util.json`, generator
//! `scripts/csl-units/names_util.cjs`).

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::formats::js_replace_first;
use super::formatters::name_doppel;
use super::js;
use super::load::{
    to_locale_lower_case, NAME_INITIAL_REGEXP, ROMANESQUE_REGEXP, STARTSWITH_ROMANESQUE_REGEXP,
};
use super::obj_blob::JS_WS_CLASS;
use super::state::State;

/// JS `.`: anything but `\n`, `\r`, U+2028, U+2029.
const DOT: &str = "[^\\n\\r\\u{2028}\\u{2029}]";

fn rx(src: &str) -> Regex {
    #[allow(clippy::expect_used)]
    Regex::new(src).expect("constant regex")
}

static RE_LEADING_TAGS: LazyLock<Regex> = LazyLock::new(|| rx("^(?:<[^>]+>)*"));
static RE_WS_HYPHEN_WS: LazyLock<Regex> = LazyLock::new(|| rx(&format!("[{JS_WS_CLASS}]*-[{JS_WS_CLASS}]*")));
static RE_WS_RUN: LazyLock<Regex> = LazyLock::new(|| rx(&format!("[{JS_WS_CLASS}]+")));
static RE_HYPHEN_LOWER: LazyLock<Regex> = LazyLock::new(|| rx("-([a-z])"));
static RE_ENDASH_LOWER: LazyLock<Regex> = LazyLock::new(|| rx("\u{2013}([a-z])"));
static RE_NOT_DOT_TAIL: LazyLock<Regex> = LazyLock::new(|| rx("[^.]+$"));
static RE_TAG_RUN: LazyLock<Regex> = LazyLock::new(|| rx("(?:-*<[^>]+>-*)"));
static RE_TAGS_ONLY: LazyLock<Regex> = LazyLock::new(|| rx("(?:<[^>]+>)+"));
static RE_TRAILING_SPACE: LazyLock<Regex> =
    LazyLock::new(|| rx(&format!("^({DOT}*[^{JS_WS_CLASS}])*([{JS_WS_CLASS}]+)$")));
static RE_LAST_DOT: LazyLock<Regex> = LazyLock::new(|| rx(&format!("^({DOT}*)\\.({DOT}*)$")));
static RE_TRAIL_WS_A: LazyLock<Regex> =
    LazyLock::new(|| rx("[\u{9}\u{a}\u{b}\u{c}\u{d}\u{20}\u{feff}\u{a0}]+$"));
static RE_WS_SIMPLE: LazyLock<Regex> = LazyLock::new(|| rx("[\u{9}\u{a}\u{b}\u{c}\u{d}\u{20}]+"));
static RE_ANY_CHAR_REST: LazyLock<Regex> = LazyLock::new(|| rx(&format!("^({DOT})({DOT}*)")));
static RE_TRAIL_FEFF: LazyLock<Regex> = LazyLock::new(|| rx("\u{feff}$"));

/// `CSL.Util.Names.compareNamesets`, which is
/// `CSL.NameOutput.prototype._compareNamesets`: see
/// [`super::util_names_common::compare_namesets`].
#[allow(unused_imports)]
pub use super::util_names_common::compare_namesets;

/// `CSL.Util.Names.unInitialize(state, name)`: upstream splits `name` at
/// hyphens and white space and re-joins the pieces with the separators it
/// found, i.e. it returns `name` unchanged (the commented-out code that once
/// lower-cased the tails of the pieces is gone). `""` for an empty name.
pub fn un_initialize(_state: &State, name: &str) -> String {
    if name.is_empty() {
        return String::new();
    }
    // namelist = name.split(/(?:\-|\s+)/); punctlist = name.match(/(\-|\s+)/g);
    // ret = namelist[0] + punctlist[0] + namelist[1] + ... : the identity.
    name.to_string()
}

/// `CSL.Util.Names.initializeWith(state, name, terminator, normalizeOnly)`:
/// reduce the given-name string `name` to initials, each followed by
/// `terminator` (`"%s"` in the terminator is replaced by the initial);
/// with `normalize_only` only the existing initials are normalised.
pub fn initialize_with(state: &State, name: &str, terminator: &str, normalize_only: bool) -> String {
    if name.is_empty() {
        return String::new();
    }
    let has_percent_s = terminator.contains("%s");
    if name == "Lord"
        || name == "Lady"
        || (!STARTSWITH_ROMANESQUE_REGEXP.is_match(&RE_LEADING_TAGS.replace(name, ""))
            && !has_percent_s)
    {
        return name.to_string();
    }

    let mut name = name.to_string();
    if state.opt.get("initialize-with-hyphen") == Some(&Value::Bool(false)) {
        name = name.replace('-', " ");
    }

    // We need to suss out what is a set of initials or abbreviation,
    // so that they can be selectively normalized.
    name = RE_WS_HYPHEN_WS.replace_all(&name, "-").into_owned();
    name = RE_WS_RUN.replace_all(&name, " ").into_owned();
    name = RE_HYPHEN_LOWER.replace_all(&name, "\u{2013}${1}").into_owned();

    // for (i = name.length-2; i > -1; i--) if "." not followed by " ": insert one
    let mut chars: Vec<char> = name.chars().collect();
    let mut i = chars.len() as i64 - 2;
    while i > -1 {
        let u = i as usize;
        if chars[u] == '.' && chars.get(u + 1) != Some(&' ') {
            chars.insert(u + 1, ' ');
        }
        i -= 1;
    }
    name = chars.into_iter().collect();

    // (1) Split the string
    let splits = name_doppel().split(&name);
    let mut namelist: Vec<String> = vec![splits.strings.first().cloned().unwrap_or_default()];

    if splits.tags.is_empty() {
        if let Some(m) = RE_NOT_DOT_TAIL.find(&namelist[0]) {
            let m = m.as_str();
            if js::len(m) == 1 && m != m.to_lowercase() {
                namelist[0].push('.');
            }
        }
    }

    for i in 1..splits.strings.len() {
        namelist.push(splits.tags.get(i - 1).cloned().unwrap_or_default());
        namelist.push(splits.strings[i].clone());
    }

    // Use doInitializeName or doNormalizeName, depending on requirements.
    let ret = if normalize_only {
        do_normalize(state, &mut namelist, terminator)
    } else {
        do_initialize(state, &mut namelist, terminator)
    };
    RE_ENDASH_LOWER.replace_all(&ret, "-${1}").into_owned()
}

/// `CSL.Util.Names.notag(str)`: `str` without leading tags.
pub fn notag(s: &str) -> String {
    RE_LEADING_TAGS.replace(s, "").into_owned()
}

/// `CSL.Util.Names.mergetag(state, tagstr, newstr)`: put the tags found in
/// `tagstr` between the text of `newstr` and its trailing white space.
pub fn mergetag(_state: &State, tagstr: &str, newstr: &str) -> String {
    let tags: Vec<&str> = RE_TAG_RUN.find_iter(tagstr).map(|m| m.as_str()).collect();
    if tags.is_empty() {
        return newstr.to_string();
    }
    let tagstr = tags.concat();
    match RE_TRAILING_SPACE.captures(newstr) {
        Some(m) => {
            let m1 = m.get(1).map(|x| x.as_str()).unwrap_or("");
            let m2 = m.get(2).map(|x| x.as_str()).unwrap_or("");
            format!("{m1}{tagstr}{m2}")
        }
        None => format!("{newstr}{tagstr}"),
    }
}

/// `CSL.Util.Names.tagonly(state, str)`: the first run of tags in `str`,
/// or `str` itself when there is none.
pub fn tagonly(_state: &State, s: &str) -> String {
    match RE_TAGS_ONLY.find(s) {
        Some(m) => m.as_str().to_string(),
        None => s.to_string(),
    }
}

/// `CSL.Util.Names.doNormalize(state, namelist, terminator)`: `namelist` is
/// a flat list of given-name elements and the separators between them.
pub fn do_normalize(state: &State, namelist: &mut [String], terminator: &str) -> String {
    // Flag elements that look like abbreviations
    let mut is_abbrev: Vec<bool> = Vec::with_capacity(namelist.len());
    for el in namelist.iter_mut() {
        let nt = notag(el);
        if js::len(&nt) > 1 && nt.ends_with('.') {
            *el = RE_LAST_DOT.replace(el, "${1}${2}").into_owned();
            is_abbrev.push(true);
        } else if js::len(el) == 1 && el.to_uppercase() == *el {
            is_abbrev.push(true);
        } else {
            is_abbrev.push(false);
        }
    }
    // Step through the elements of the givenname array
    let n = namelist.len();
    let mut i = 0;
    while i < n {
        // If the element is not an abbreviation, leave it and its trailing spaces alone
        if is_abbrev[i] {
            // For all elements but the last
            if i + 2 < n {
                // Start from scratch on space-like things following an abbreviation
                namelist[i + 1] = tagonly(state, &namelist[i + 1]);
                if !is_abbrev.get(i + 2).copied().unwrap_or(false) {
                    namelist[i + 1] = format!("{} ", tagonly(state, &namelist[i + 1]));
                }
                // Add the terminator to the element
                // If the following element is not a single-character abbreviation, remove a trailing zero-width non-break space, if present
                if js::len(&namelist[i + 2]) > 1 {
                    namelist[i + 1] = format!(
                        "{}{}",
                        RE_TRAIL_FEFF.replace(terminator, ""),
                        namelist[i + 1]
                    );
                } else {
                    namelist[i + 1] = mergetag(state, &namelist[i + 1], terminator);
                }
            }
            // For the last element (if it is an abbreviation), just append the terminator
            if i + 1 == n {
                namelist[i] = format!("{}{}", namelist[i], terminator);
            }
        }
        i += 2;
    }
    // Remove trailing cruft and duplicate spaces, and return
    let joined = namelist.concat();
    let a = RE_TRAIL_WS_A.replace(&joined, "").into_owned();
    let b = RE_WS_HYPHEN_WS.replace_all(&a, "-").into_owned();
    RE_WS_SIMPLE.replace_all(&b, " ").into_owned()
}

/// `CSL.Util.Names.doInitialize(state, namelist, terminator)`.
pub fn do_initialize(state: &State, namelist: &mut Vec<String>, terminator: &str) -> String {
    let has_percent_s = terminator.contains("%s");
    let ilen = namelist.len();
    let mut i = 0;
    while i < ilen {
        let n = namelist[i].clone();
        if n.is_empty() {
            i += 2;
            continue;
        }
        // m: [full, m1, m2, m3] where each group may be undefined
        let mut m: Option<(String, Option<String>, Option<String>)> =
            NAME_INITIAL_REGEXP.captures(&n).map(|c| {
                (
                    c.get(1).map(|x| x.as_str()).unwrap_or("").to_string(),
                    c.get(2).map(|x| x.as_str().to_string()),
                    c.get(3).map(|x| x.as_str().to_string()),
                )
            });
        if m.is_none()
            && !STARTSWITH_ROMANESQUE_REGEXP.is_match(&n)
            && js::len(&n) > 1
            && has_percent_s
        {
            m = RE_ANY_CHAR_REST.captures(&n).map(|c| {
                (
                    c.get(1).map(|x| x.as_str()).unwrap_or("").to_string(),
                    c.get(2).map(|x| x.as_str().to_string()),
                    None,
                )
            });
        }
        if let Some((m1, m2, m3)) = m.as_mut() {
            if m2.as_deref().map(|x| !x.is_empty()).unwrap_or(false)
                && m3.as_deref().map(|x| !x.is_empty()).unwrap_or(false)
            {
                *m1 = format!("{}{}", m1, m2.as_deref().unwrap_or(""));
                *m2 = Some(String::new());
            }
        }
        let first_upper = m.as_ref().map(|(m1, _, _)| {
            let f = js::slice(m1, 0, Some(1));
            f == f.to_uppercase()
        });
        if first_upper == Some(true) {
            let (m1, m2, _m3) = m.clone().unwrap_or_default();
            let mut extra = String::new();
            if let Some(m2) = m2.as_deref().filter(|x| !x.is_empty()) {
                let mut s = String::new();
                for c in m2.chars() {
                    let cs = c.to_string();
                    if cs == cs.to_uppercase() {
                        s.push(c);
                    } else {
                        break;
                    }
                }
                if js::len(&s) < js::len(m2) {
                    extra = to_locale_lower_case(state, &s);
                }
            }
            namelist[i] = format!("{m1}{extra}");
            if i + 1 < ilen {
                if has_percent_s {
                    namelist[i] = js_replace_first(terminator, "%s", &namelist[i]);
                } else if namelist[i + 1].contains('-') {
                    let stripped = js_replace_first(&namelist[i + 1], "-", "");
                    namelist[i + 1] = format!("{}-", mergetag(state, &stripped, terminator));
                } else {
                    namelist[i + 1] = mergetag(state, &namelist[i + 1], terminator);
                }
            } else if has_percent_s {
                namelist[i] = js_replace_first(terminator, "%s", &namelist[i]);
            } else {
                namelist.push(terminator.to_string());
            }
        } else if ROMANESQUE_REGEXP.is_match(&n)
            && m.as_ref()
                .map(|(_, _, m3)| m3.as_deref().map(str::is_empty).unwrap_or(true))
                .unwrap_or(true)
        {
            namelist[i] = format!(" {n}");
        }
        i += 2;
    }
    let ret = namelist.concat();
    let a = RE_TRAIL_WS_A.replace(&ret, "").into_owned();
    let b = RE_WS_HYPHEN_WS.replace_all(&a, "-").into_owned();
    RE_WS_SIMPLE.replace_all(&b, " ").into_owned()
}

/// `CSL.Util.Names.getRawName(name)`: the literal, or the given and family
/// names, joined with a space.
pub fn get_raw_name(name: &Value) -> String {
    let mut ret: Vec<String> = Vec::new();
    let get = |k: &str| name.get(k).filter(|v| js::truthy(v));
    if let Some(l) = get("literal") {
        ret.push(js::to_js_string(l));
    } else {
        if let Some(g) = get("given") {
            ret.push(js::to_js_string(g));
        }
        if let Some(f) = get("family") {
            ret.push(js::to_js_string(f));
        }
    }
    ret.join(" ")
}

#[cfg(test)]
mod tests {
    //! Differential tests against citeproc-js 2.4.63 for `CSL.Util.Names`:
    //! reference `tests/data/csl/units/names_output.json`, section `util`
    //! (generator `scripts/csl-units/names_output.cjs`).
    //!
    //! * `initializeWith` over ~480 given names and suffixes (every
    //!   `given`/`suffix` of a sample of the fixtures' names, plus edge cases:
    //!   hyphens, particles, tags, non-Latin scripts, non-breaking spaces),
    //!   20 terminators (`.`, `%s`, `%s.`, `﻿`, `$&`, ...), both values of
    //!   `normalizeOnly`, and `initialize-with-hyphen` unset / false / true
    //!   (the last two over the edge cases only): about 14,500 cases;
    //! * `unInitialize`, `mergetag`, `tagonly`, `notag`, `getRawName`.
    //!
    //! Pass criterion: every result equal to citeproc-js's.
    use serde_json::Value;

    use super::*;
    use crate::citeproc::util_names_output::testing::REFERENCE;

    fn state(hyphen: &Value) -> State {
        let mut st = State::default();
        if !hyphen.is_null() {
            st.opt.insert("initialize-with-hyphen".into(), hyphen.clone());
        }
        st.tmp.lang_array = vec!["en".to_string()];
        st
    }

    #[test]
    fn initialize_with_matches_citeproc_js() {
        let mut n = 0;
        for set in REFERENCE["util"]["names"].as_array().expect("names") {
            let st = state(&set["hyphen"]);
            for row in set["rows"].as_array().expect("rows") {
                let g = row[0].as_str().expect("g");
                let t = row[1].as_str().expect("t");
                let normalize = row[2].as_bool().expect("n");
                let got = initialize_with(&st, g, t, normalize);
                assert_eq!(
                    Some(got.as_str()),
                    row[3].as_str(),
                    "initializeWith({g:?}, {t:?}, {normalize}) hyphen={}",
                    set["hyphen"]
                );
                n += 1;
            }
        }
        assert!(n > 14000, "{n}");
    }

    #[test]
    fn small_helpers_match_citeproc_js() {
        let st = state(&Value::Null);
        for r in REFERENCE["util"]["unInit"].as_array().expect("unInit") {
            assert_eq!(
                Some(un_initialize(&st, r["g"].as_str().expect("g")).as_str()),
                r["v"].as_str()
            );
        }
        for r in REFERENCE["util"]["mergetag"].as_array().expect("mergetag") {
            let (a, b) = (r["a"].as_str().expect("a"), r["b"].as_str().expect("b"));
            assert_eq!(Some(mergetag(&st, a, b).as_str()), r["v"].as_str(), "mergetag({a:?}, {b:?})");
        }
        for r in REFERENCE["util"]["tagonly"].as_array().expect("tagonly") {
            let s = r["s"].as_str().expect("s");
            assert_eq!(Some(tagonly(&st, s).as_str()), r["v"].as_str(), "tagonly({s:?})");
        }
        for r in REFERENCE["util"]["notag"].as_array().expect("notag") {
            let s = r["s"].as_str().expect("s");
            assert_eq!(Some(notag(s).as_str()), r["v"].as_str(), "notag({s:?})");
        }
        for r in REFERENCE["util"]["getRaw"].as_array().expect("getRaw") {
            assert_eq!(Some(get_raw_name(&r["n"]).as_str()), r["v"].as_str(), "{}", r["n"]);
        }
    }
}
