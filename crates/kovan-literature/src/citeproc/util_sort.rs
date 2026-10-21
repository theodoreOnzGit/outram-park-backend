// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_sort.js
// Version:     2.4.63, commit 73bc1b44bc7d54d0bfec4e070fd27f5efe024ff9
// Copyright:   (c) 2009-2019 Frank Bennett
// Licence:     AGPL-3.0, taken from upstream's "CPAL-1.0 or AGPL-3.0-or-later"
//              (LICENSE at the commit above; see this crate's NOTICE).
// Modified:    2026-10-08, by the OUTRAM PARK contributors. This file is a
//              Rust translation (port) of the files named above, modified
//              from the original.
// No warranty: this program is distributed in the hope that it will be
//              useful, but WITHOUT ANY WARRANTY; without even the implied
//              warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
//              PURPOSE. See the GNU Affero General Public License.

//! `CSL.Util.Sort.strip_prepositions` (util_sort.js).

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::js;

/// `/^(([aA]|[aA][nN]|[tT][hH][eE])\s+)/`.
static LEADING_ARTICLE: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(&format!("^(?:[aA]|[aA][nN]|[tT][hH][eE])[{}]+", js::WS)).expect("static regex")
});

/// `CSL.Util.Sort.strip_prepositions(str)`: drop a leading `a`, `an` or `the`
/// and the white space after it. Anything that is not a string is returned
/// as it came (upstream guards with `"string" === typeof str`).
pub fn strip_prepositions(value: &Value) -> Value {
    match value {
        Value::String(s) => match LEADING_ARTICLE.find(s) {
            // `str.substr(m[1].length)`: the match is the whole group, in UTF-16 units.
            Some(m) => Value::String(js::substr(s, js::len(m.as_str()) as i64, None)),
            None => value.clone(),
        },
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn strips_a_an_the_only_when_followed_by_space() {
        assert_eq!(strip_prepositions(&json!("The Title")), json!("Title"));
        assert_eq!(strip_prepositions(&json!("an  apple")), json!("apple"));
        assert_eq!(strip_prepositions(&json!("A\u{a0}b")), json!("b"));
        assert_eq!(strip_prepositions(&json!("Another")), json!("Another"));
        assert_eq!(strip_prepositions(&json!("theory")), json!("theory"));
        assert_eq!(strip_prepositions(&json!("the")), json!("the"));
        assert_eq!(strip_prepositions(&Value::Null), Value::Null);
        assert_eq!(strip_prepositions(&json!(7)), json!(7));
    }
}
