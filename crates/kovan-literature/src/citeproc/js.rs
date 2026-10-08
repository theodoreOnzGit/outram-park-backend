// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      none: JavaScript language semantics the port relies on
//              (truthiness, string coercion, UTF-16 string indexing,
//              parseInt), written for the port. No upstream code.
// Version:     2.4.63, commit 73bc1b44bc7d54d0bfec4e070fd27f5efe024ff9
// Licence:     AGPL-3.0 (this crate's licence; see NOTICE).
// Modified:    2026-10-08, by the OUTRAM PARK contributors.
// No warranty: this program is distributed in the hope that it will be
//              useful, but WITHOUT ANY WARRANTY; without even the implied
//              warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
//              PURPOSE. See the GNU Affero General Public License.

//! JavaScript semantics for the citeproc-js port (`PORTING.md` §3, §5).
//!
//! citeproc-js leans on JS's loose typing: truthiness, `"" + x`, `parseInt`,
//! and UTF-16 string indexing. These helpers reproduce exactly that, so the
//! translated code can say `js::truthy(v)` where the JS says `if (v)`, and get
//! the same answer on every input.

use std::cmp::Ordering;

use serde_json::{Map, Value};

/// A plain JS object of data (`{}`): `state.opt`, `token.strings`, an item.
pub type Obj = Map<String, Value>;

/// The characters JS's `\s` matches, as the inside of a regex character
/// class. JS `\s` = Unicode `White_Space` plus U+FEFF; Rust's `\s` lacks
/// U+FEFF. Use `format!("[{}]", js::WS)`.
pub const WS: &str = r"\s\x{feff}";

/// JS truthiness of a JSON value (`if (v)`). `""`, `0`, `-0`, `NaN`, `null`,
/// `false` are false; every string with content, every non-zero number, and
/// **every array or object, even empty**, is true.
pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0 && !f.is_nan()).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// JS truthiness of an optional value: `undefined` (absent) is false.
pub fn truthy_opt(v: Option<&Value>) -> bool {
    v.map(truthy).unwrap_or(false)
}

/// JS `"" + v` / `String(v)` for a JSON value. Numbers print as JS prints them
/// (`1` not `1.0`, `0.5`, `1e21`); arrays join their elements' strings with
/// `,`; objects become `[object Object]`; `null` is `"null"`.
pub fn to_js_string(v: &Value) -> String {
    match v {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.to_string()
            } else if let Some(u) = n.as_u64() {
                u.to_string()
            } else {
                number_to_js_string(n.as_f64().unwrap_or(f64::NAN))
            }
        }
        Value::String(s) => s.clone(),
        Value::Array(a) => a
            .iter()
            .map(|x| match x {
                // JS: [null].join() and [undefined].join() give "".
                Value::Null => String::new(),
                other => to_js_string(other),
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

/// JS `"" + n` for a number (`Number.prototype.toString()`): integers without
/// a fraction, `NaN`, `Infinity`, and the shortest round-trip decimal
/// otherwise (Rust's `{}` for `f64` is also shortest round-trip).
pub fn number_to_js_string(f: f64) -> String {
    if f.is_nan() {
        "NaN".to_string()
    } else if f.is_infinite() {
        if f > 0.0 { "Infinity" } else { "-Infinity" }.to_string()
    } else if f == f.trunc() && f.abs() < 1e21 {
        format!("{}", f as i64)
    } else {
        format!("{f}")
    }
}

/// JS `parseInt(s, 10)`: skip leading whitespace, an optional sign, then the
/// longest run of ASCII digits. `None` is JS's `NaN` (no digits).
pub fn parse_int(s: &str) -> Option<i64> {
    let t = s.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
    let (neg, rest) = match t.chars().next() {
        Some('-') => (true, &t[1..]),
        Some('+') => (false, &t[1..]),
        _ => (false, t),
    };
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    // Saturate rather than overflow: JS would give a float; inputs in the
    // suite never come near i64's range.
    let mut n: i64 = 0;
    for d in digits.bytes() {
        n = n.saturating_mul(10).saturating_add((d - b'0') as i64);
    }
    Some(if neg { -n } else { n })
}

/// JS `parseInt(v, 10)` on a JSON value (`parseInt` stringifies first).
pub fn parse_int_value(v: &Value) -> Option<i64> {
    parse_int(&to_js_string(v))
}

/// The UTF-16 code units of `s` (what JS indexes).
fn units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

/// Back from UTF-16 code units. A lone surrogate (only possible when a slice
/// splits a pair) becomes U+FFFD, where JS would keep the lone unit.
fn from_units(u: &[u16]) -> String {
    String::from_utf16_lossy(u)
}

/// JS `s.length`: UTF-16 code units.
pub fn len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Resolve a JS index argument (negative counts from the end) to `0..=len`.
fn resolve(i: i64, len: usize) -> usize {
    if i < 0 {
        (len as i64 + i).max(0) as usize
    } else {
        (i as usize).min(len)
    }
}

/// JS `s.slice(start, end)` (`end = None` is "to the end"). Negative indices
/// count from the end, as in JS.
pub fn slice(s: &str, start: i64, end: Option<i64>) -> String {
    if s.is_ascii() {
        let l = s.len();
        let a = resolve(start, l);
        let b = end.map(|e| resolve(e, l)).unwrap_or(l);
        return if a < b {
            s[a..b].to_string()
        } else {
            String::new()
        };
    }
    let u = units(s);
    let l = u.len();
    let a = resolve(start, l);
    let b = end.map(|e| resolve(e, l)).unwrap_or(l);
    if a < b {
        from_units(&u[a..b])
    } else {
        String::new()
    }
}

/// JS `s.substring(start, end)`: negative and NaN clamp to 0, and the
/// arguments swap when `start > end`.
pub fn substring(s: &str, start: i64, end: Option<i64>) -> String {
    let u = units(s);
    let l = u.len() as i64;
    let a = start.clamp(0, l) as usize;
    let b = end.unwrap_or(l).clamp(0, l) as usize;
    let (a, b) = if a > b { (b, a) } else { (a, b) };
    from_units(&u[a..b])
}

/// JS `s.substr(start, length)` (`length = None` is "to the end").
pub fn substr(s: &str, start: i64, length: Option<i64>) -> String {
    let u = units(s);
    let l = u.len();
    let a = resolve(start, l);
    let n = length.unwrap_or(l as i64).max(0) as usize;
    let b = (a + n).min(l);
    from_units(&u[a..b])
}

/// JS `s.charAt(i)` (empty when out of range).
pub fn char_at(s: &str, i: i64) -> String {
    if i < 0 {
        return String::new();
    }
    let u = units(s);
    match u.get(i as usize) {
        Some(&c) => from_units(&[c]),
        None => String::new(),
    }
}

/// JS `s.indexOf(needle, from)`, in UTF-16 code units; `-1` when absent.
pub fn index_of(s: &str, needle: &str, from: usize) -> i64 {
    let hay = units(s);
    let nee = units(needle);
    if nee.is_empty() {
        return from.min(hay.len()) as i64;
    }
    if nee.len() > hay.len() {
        return -1;
    }
    for i in from..=(hay.len() - nee.len()) {
        if hay[i..i + nee.len()] == nee[..] {
            return i as i64;
        }
    }
    -1
}

/// JS `s.lastIndexOf(needle)`; `-1` when absent.
pub fn last_index_of(s: &str, needle: &str) -> i64 {
    let hay = units(s);
    let nee = units(needle);
    if nee.len() > hay.len() {
        return -1;
    }
    for i in (0..=(hay.len() - nee.len())).rev() {
        if hay[i..i + nee.len()] == nee[..] {
            return i as i64;
        }
    }
    -1
}

/// JS `s.split(re)` where `re` has capture groups: JS puts each match's
/// captures (undefined ones as `None`) between the pieces. With no captures
/// this is a plain split. An empty-string input gives `[""]`, as in JS.
pub fn split_with_captures(re: &regex::Regex, s: &str) -> Vec<Option<String>> {
    // ECMA-262 String.prototype.split with a RegExp separator: matches are
    // tried at positions q < size only, and a match whose end equals the last
    // split point p (an empty match there) is skipped.
    if s.is_empty() {
        return if re.is_match(s) {
            Vec::new()
        } else {
            vec![Some(String::new())]
        };
    }
    let mut out = Vec::new();
    let mut last = 0;
    for caps in re.captures_iter(s) {
        let Some(m) = caps.get(0) else { continue };
        if m.start() >= s.len() {
            break;
        }
        if m.end() == last {
            continue;
        }
        out.push(Some(s[last..m.start()].to_string()));
        for g in 1..caps.len() {
            out.push(caps.get(g).map(|x| x.as_str().to_string()));
        }
        last = m.end();
    }
    out.push(Some(s[last..].to_string()));
    out
}

/// JS `s.split(re)` for a pattern without captures: the pieces.
pub fn split(re: &regex::Regex, s: &str) -> Vec<String> {
    split_with_captures(re, s)
        .into_iter()
        .map(|x| x.unwrap_or_default())
        .collect()
}

/// JS `s.trim()`: strips JS whitespace (Unicode `White_Space` + U+FEFF).
pub fn trim(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
}

/// JS `a.localeCompare(b, lang)` as node's ICU computes it.
///
/// **Provisional** (PORTING.md §8): the #795 owner replaces this with a real
/// ICU-equivalent collator. Until then: compare case-insensitively on the
/// NFD form with diacritics stripped, then by diacritics, then by case
/// (lower before upper, as ICU's default tertiary order).
pub fn locale_compare(a: &str, b: &str, _lang: &str) -> Ordering {
    use unicode_normalization::UnicodeNormalization;
    let base = |s: &str| -> String {
        s.nfd()
            .filter(|c| !('\u{300}'..='\u{36f}').contains(c))
            .flat_map(char::to_lowercase)
            .collect()
    };
    let p = base(a).cmp(&base(b));
    if p != Ordering::Equal {
        return p;
    }
    let s = |x: &str| -> String { x.nfd().flat_map(char::to_lowercase).collect() };
    let q = s(a).cmp(&s(b));
    if q != Ordering::Equal {
        return q;
    }
    // Tertiary: lowercase first.
    for (x, y) in a.chars().zip(b.chars()) {
        if x != y {
            return match (x.is_lowercase(), y.is_lowercase()) {
                (true, false) => Ordering::Less,
                (false, true) => Ordering::Greater,
                _ => x.cmp(&y),
            };
        }
    }
    a.len().cmp(&b.len())
}

/// `-1`, `0` or `1`, the integer a JS comparator returns.
pub fn ordering_to_i32(o: Ordering) -> i32 {
    match o {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

/// A string field of an object, `None` when absent or not a string.
pub fn get_str<'v>(o: &'v Obj, key: &str) -> Option<&'v str> {
    o.get(key).and_then(Value::as_str)
}

/// A field of an object as JS `"" + o[key]` would give it, `None` when absent
/// (`undefined`).
pub fn get_string(o: &Obj, key: &str) -> Option<String> {
    o.get(key).map(to_js_string)
}

/// JS truthiness of `o[key]`.
pub fn get_truthy(o: &Obj, key: &str) -> bool {
    truthy_opt(o.get(key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn truthiness_follows_javascript() {
        for v in [json!(""), json!(0), json!(0.0), json!(null), json!(false)] {
            assert!(!truthy(&v), "{v} should be falsy");
        }
        for v in [
            json!("0"),
            json!(" "),
            json!(1),
            json!(-1.5),
            json!([]),
            json!({}),
            json!(true),
        ] {
            assert!(truthy(&v), "{v} should be truthy");
        }
        assert!(!truthy_opt(None));
    }

    #[test]
    fn string_coercion_follows_javascript() {
        assert_eq!(to_js_string(&json!(1)), "1");
        assert_eq!(to_js_string(&json!(1.0)), "1");
        assert_eq!(to_js_string(&json!(0.5)), "0.5");
        assert_eq!(to_js_string(&json!([1, "a", null])), "1,a,");
        assert_eq!(to_js_string(&json!({})), "[object Object]");
        assert_eq!(to_js_string(&json!(null)), "null");
        assert_eq!(number_to_js_string(f64::NAN), "NaN");
    }

    #[test]
    fn parse_int_follows_javascript() {
        assert_eq!(parse_int("  42abc"), Some(42));
        assert_eq!(parse_int("-7"), Some(-7));
        assert_eq!(parse_int("x1"), None);
        assert_eq!(parse_int(""), None);
        assert_eq!(parse_int_value(&json!(3.9)), Some(3));
    }

    #[test]
    fn utf16_indexing_follows_javascript() {
        let s = "a\u{1F600}b"; // the emoji is two UTF-16 units
        assert_eq!(len(s), 4);
        assert_eq!(slice(s, -1, None), "b");
        assert_eq!(slice(s, 0, Some(1)), "a");
        assert_eq!(slice("héllo", 1, Some(3)), "él");
        assert_eq!(slice("abc", 2, Some(1)), "");
        assert_eq!(substring("abc", 2, Some(0)), "ab");
        assert_eq!(substr("abcdef", -3, Some(2)), "de");
        assert_eq!(char_at("abc", 5), "");
        assert_eq!(index_of("abcabc", "c", 3), 5);
        assert_eq!(index_of("abc", "", 1), 1);
        assert_eq!(last_index_of("abcabc", "b"), 4);
        assert_eq!(index_of("abc", "z", 0), -1);
    }

    #[test]
    fn split_keeps_captures_like_javascript() {
        let re = regex::Regex::new("(-)|,").unwrap();
        // "a-b,c".split(/(-)|,/) is ["a", "-", "b", undefined, "c"] in JS.
        assert_eq!(
            split_with_captures(&re, "a-b,c"),
            vec![
                Some("a".into()),
                Some("-".into()),
                Some("b".into()),
                None,
                Some("c".into())
            ]
        );
        assert_eq!(
            split(&regex::Regex::new(",").unwrap(), ""),
            vec![String::new()]
        );
        // "abc".split(/(?:)/) is ["a", "b", "c"] in JS.
        assert_eq!(
            split(&regex::Regex::new("").unwrap(), "abc"),
            vec!["a", "b", "c"]
        );
    }
}
