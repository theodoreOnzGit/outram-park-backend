// Part of the kovan Zotero port (GitHub #747, #749).
//
// Not a port of a Zotero file: the JavaScript built-ins that Zotero's
// translators and framework rely on (ECMA-262 `String.prototype.trim`, the
// `\s` class, `ToString`, truthiness), written so the ported code can keep
// upstream's semantics where Rust's differ.
// Copyright (c) 2026 the kovan authors. Licence: AGPL-3.0-only.

//! JavaScript semantics the translator port depends on.
//!
//! Rust and JavaScript disagree on small things that change output:
//!
//! * **Whitespace.** JavaScript's `\s` and `String.prototype.trim` use
//!   ECMA-262 WhiteSpace + LineTerminator, which includes U+FEFF and excludes
//!   U+0085; Rust's `char::is_whitespace`, `str::trim` and the `regex` crate's
//!   Unicode `\s` use Unicode `White_Space`, which is the other way round.
//!   Use [`is_space`], [`trim`] and [`WS`] (a regex class) instead.
//! * **`\w` and `\b`** are ASCII in JavaScript and Unicode in `regex`. Write
//!   `[A-Za-z0-9_]` ([`WORD`]) for `\w` and `(?-u:\b)` for `\b`.
//! * **String units.** JavaScript indexes UTF-16 code units; this port
//!   indexes Unicode scalar values (`char`s). The two agree on every
//!   character in the Basic Multilingual Plane, i.e. everything except
//!   astral-plane characters (emoji, some CJK extensions), where a JavaScript
//!   `length`, `substr` or `read(n)` counts 2 and this port counts 1.

use serde_json::Value;

/// A regex character class equal to JavaScript's `\s` (ECMA-262 WhiteSpace
/// and LineTerminator).
pub const WS: &str = r"[\t\n\x0B\x0C\r \x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}\x{FEFF}]";

/// A regex character class equal to JavaScript's `\S`.
pub const NOT_WS: &str = r"[^\t\n\x0B\x0C\r \x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}\x{FEFF}]";

/// A regex character class equal to JavaScript's `\w`.
pub const WORD: &str = "[A-Za-z0-9_]";

/// Whether `c` matches JavaScript's `\s`.
pub fn is_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

/// Whether `c` matches JavaScript's `\w`.
pub fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// `String.prototype.trim`.
pub fn trim(s: &str) -> &str {
    s.trim_matches(is_space)
}

/// `String.prototype.trimStart`.
pub fn trim_start(s: &str) -> &str {
    s.trim_start_matches(is_space)
}

/// `String.prototype.trimEnd`.
pub fn trim_end(s: &str) -> &str {
    s.trim_end_matches(is_space)
}

/// JavaScript truthiness of a JSON value (`undefined` is `None`).
pub fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(_)) | Some(Value::Object(_)) => true,
    }
}

/// JavaScript `ToString` of a JSON value: what `val.toString()` or `"" + val`
/// gives (arrays join with ",", objects are `[object Object]`).
pub fn to_js_string(v: &Value) -> String {
    match v {
        Value::Null => "null".to_owned(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => number_to_string(n),
        Value::String(s) => s.clone(),
        Value::Array(a) => a
            .iter()
            .map(|x| match x {
                // Array.prototype.join writes null and undefined as "".
                Value::Null => String::new(),
                other => to_js_string(other),
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_owned(),
    }
}

/// `Number.prototype.toString` for the numbers JSON carries: integers print
/// without a decimal point, as in JavaScript.
pub fn number_to_string(n: &serde_json::Number) -> String {
    if let Some(i) = n.as_i64() {
        return i.to_string();
    }
    if let Some(u) = n.as_u64() {
        return u.to_string();
    }
    let f = n.as_f64().unwrap_or(f64::NAN);
    if f.fract() == 0.0 && f.abs() < 1e21 {
        format!("{f:.0}")
    } else {
        f.to_string()
    }
}

/// `str.substr(start, len)` over chars, with JavaScript's clamping.
pub fn substr(s: &str, start: usize, len: usize) -> String {
    s.chars().skip(start).take(len).collect()
}

/// The number of units JavaScript's `length` would report, counted in chars
/// (see the module docs on string units).
pub fn len(s: &str) -> usize {
    s.chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn trim_uses_ecmascript_whitespace() {
        assert_eq!(trim("\u{FEFF} a \u{A0}"), "a");
        // U+0085 is Unicode White_Space but not ECMAScript whitespace.
        assert_eq!(trim("\u{85}a"), "\u{85}a");
        assert_eq!(regex::Regex::new(WS).unwrap().is_match("\u{FEFF}"), true);
        assert_eq!(regex::Regex::new(WS).unwrap().is_match("\u{85}"), false);
        // An ASCII word boundary, as JavaScript's \b: "é" is not \w, so there is a
        // boundary before "t" (node: /\bt/.test("ét") === true), none in "at".
        let b = regex::Regex::new(r"(?-u:\b)t").unwrap();
        assert!(b.is_match("ét"));
        assert!(!b.is_match("at"));
        assert!(b.is_match("é t"));
    }

    #[test]
    fn to_string_and_truthiness() {
        assert_eq!(to_js_string(&json!([1, "a", null])), "1,a,");
        assert_eq!(to_js_string(&json!({"a": 1})), "[object Object]");
        assert_eq!(to_js_string(&json!(2.0)), "2");
        assert!(!truthy(Some(&json!(0))));
        assert!(truthy(Some(&json!([]))));
        assert!(!truthy(Some(&json!(""))));
        assert!(!truthy(None));
    }
}
