// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_dates.js
// Version:     2.4.63, commit 73bc1b44bc7d54d0bfec4e070fd27f5efe024ff9
// Copyright:   (c) 2009-2019 Frank Bennett
// Licence:     AGPL-3.0, taken from upstream's "CPAL-1.0 or AGPL-3.0-or-later"
//              (LICENSE at the commit above; see this crate's NOTICE).
// Modified:    2026-10-08, by the OUTRAM PARK contributors. This file is a
//              Rust translation (port) of the file(s) named above, modified
//              from the original.
// No warranty: this program is distributed in the hope that it will be
//              useful, but WITHOUT ANY WARRANTY; without even the implied
//              warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR
//              PURPOSE. See the GNU Affero General Public License.

//! Port of `src/util_dates.js`: `CSL.Util.Dates`, the year / month / day
//! formatters used when rendering dates and building date sort keys.
//!
//! The JS functions take `(state, num, ...)` and return a string (or a number
//! for [`month_numeric`]). Here `num` is a [`serde_json::Value`] (a JS number
//! or string; `Value::Null` stands for `undefined`/`null`).
//!
//! # Integration points
//!
//! * [`month_long`] / [`month_short`] call `state.getTerm(...)` through
//!   [`input_get_term`](super::util_number::input_get_term), a thin wrapper
//!   over `State::get_term`.
//! * [`year_imperial`] needs `state.tmp.date_object` (set by `node_date.js`)
//!   and the abbreviation transform (`util_transform.js`
//!   `loadAbbreviation`); both are passed in, see its docs.
//! * [`day_ordinal`] is `state.fun.ordinalizer.format(num, gender)`.

use serde_json::Value;

use super::js::{self, Obj};
use super::state::State;
use super::util_number::{input_get_term, TermQuery};
use super::{CslResult, EngineError};

/// `CSL.Util.Dates.year["long"]`: the number as a string. A falsy `num`
/// becomes `"0"`, except a boolean, which becomes `""`.
pub fn year_long(num: &Value) -> String {
    if !js::truthy(num) {
        if num.is_boolean() {
            return String::new();
        }
        return "0".to_string();
    }
    js::to_js_string(num)
}

/// JS `Number(v)` for a JSON value (strings are parsed as JS does for the
/// plain decimal forms the engine sees; anything else is NaN).
fn to_number(v: &Value) -> f64 {
    match v {
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        Value::String(s) => {
            let t = js::trim(s);
            if t.is_empty() {
                0.0
            } else {
                t.parse::<f64>().unwrap_or(f64::NAN)
            }
        }
        Value::Bool(b) => f64::from(u8::from(*b)),
        Value::Null => 0.0,
        _ => f64::NAN,
    }
}

/// The Japanese era containing `date` (YYYYMMDD as an integer): its kanji
/// label and year offset, or `None` before 1868-09-08.
pub fn imperial_era(date: Option<i64>) -> Option<(&'static str, i64)> {
    let d = date?;
    if (18680908..19120730).contains(&d) {
        Some(("\u{660e}\u{6cbb}", 1867))
    } else if (19120730..19261225).contains(&d) {
        Some(("\u{5927}\u{6b63}", 1911))
    } else if (19261225..19890108).contains(&d) {
        Some(("\u{662d}\u{548c}", 1925))
    } else if d >= 19890108 {
        Some(("\u{5e73}\u{6210}", 1988))
    } else {
        None
    }
}

/// `CSL.Util.Dates.year.imperial(state, num, end)`: crude conversion to a
/// Japanese imperial year, `""` when `num` is before the Meiji era.
///
/// * `date_object` is `state.tmp.date_object` (the date being rendered),
///   read for `month`/`day` (or `month_end`/`day_end` when `end`).
/// * `abbreviate` is the abbreviation step: given the era label it returns
///   the replacement from the `default`/`number` abbreviation list (after
///   `sys.normalizeAbbrevsKey("number", label)` and
///   `transform.loadAbbreviation`), or `None` to keep the label.
pub fn year_imperial<F: FnMut(&str) -> Option<String>>(
    date_object: &Obj,
    num: &Value,
    end: bool,
    mut abbreviate: F,
) -> String {
    let num_str = if !js::truthy(num) {
        if num.is_boolean() {
            String::new()
        } else {
            "0".to_string()
        }
    } else {
        js::to_js_string(num)
    };
    let suffix = if end { "_end" } else { "" };
    let pad2 = |v: Option<&Value>| -> String {
        let mut s = match v {
            Some(x) if js::truthy(x) => js::to_js_string(x),
            _ => "1".to_string(),
        };
        while js::len(&s) < 2 {
            s = format!("0{s}");
        }
        s
    };
    let month = pad2(date_object.get(&format!("month{suffix}")));
    let day = pad2(date_object.get(&format!("day{suffix}")));
    let date = js::parse_int(&format!("{num_str}{month}{day}"));
    match imperial_era(date) {
        Some((label, offset)) => {
            let label = abbreviate(label).unwrap_or_else(|| label.to_string());
            let n = if !js::truthy(num) {
                if num.is_boolean() {
                    0.0
                } else {
                    0.0
                }
            } else {
                to_number(num)
            };
            format!("{label}{}", js::number_to_js_string(n - offset as f64))
        }
        None => String::new(),
    }
}

/// `CSL.Util.Dates.year["short"]`: the last two digits of a four-digit year;
/// `None` (JS `undefined`) for anything else. `null` is a TypeError
/// (`num.toString()`), as upstream.
pub fn year_short(num: &Value) -> CslResult<Option<String>> {
    if num.is_null() {
        return Err(EngineError::BadInput(
            "Cannot read properties of undefined (reading 'toString')".into(),
        ));
    }
    let s = js::to_js_string(num);
    if !s.is_empty() && js::len(&s) == 4 {
        Ok(Some(js::substr(&s, 2, None)))
    } else {
        Ok(None)
    }
}

/// `CSL.Util.Dates.year.numeric`: left-pad the trailing digits to four. Only
/// the trailing run of digits is kept; the text before it is prefixed back
/// (so `-5` gives `-0005`), but when there are no trailing digits the
/// prefix is dropped (upstream's `slice(0, -0)` quirk: `"abc"` gives
/// `"0000"`).
pub fn year_numeric(num: &Value) -> String {
    let s = js::to_js_string(num);
    let digits_len = s.chars().rev().take_while(|c| c.is_ascii_digit()).count();
    let digits: String = s.chars().skip(s.chars().count() - digits_len).collect();
    let pre = js::slice(&s, 0, Some(-(digits_len as i64)));
    let mut n = digits;
    while js::len(&n) < 4 {
        n = format!("0{n}");
    }
    format!("{pre}{n}")
}

/// The result of `normalizeMonth(num, true)`: a month or season number and
/// the term stub (`"month-"` or `"season-"`) it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonthSeason {
    /// `"month-"` or `"season-"`.
    pub stub: &'static str,
    /// 1..=12 (months) or 1..=4 (seasons); 0 when out of range.
    pub num: i64,
}

fn month_input_int(num: &Value) -> i64 {
    // num = ""+num; if (!/^[0-9]+$/) num = 0; parseInt(num, 10)
    let s = if js::truthy(num) {
        js::to_js_string(num)
    } else {
        "0".to_string()
    };
    if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) {
        js::parse_int(&s).unwrap_or(0)
    } else {
        0
    }
}

/// `CSL.Util.Dates.normalizeMonth(num)` without `useSeason`: the month 1 to
/// 12, or 0.
pub fn normalize_month(num: &Value) -> i64 {
    let n = month_input_int(num);
    if !(1..=12).contains(&n) {
        0
    } else {
        n
    }
}

/// `CSL.Util.Dates.normalizeMonth(num, true)`: months 1-12, seasons 13-16
/// (and 17-24 folded back by 4s) as `season-01` to `season-04`.
pub fn normalize_month_season(num: &Value) -> MonthSeason {
    let n = month_input_int(num);
    let mut res = MonthSeason {
        stub: "month-",
        num: n,
    };
    if res.num < 1 || res.num > 24 {
        res.num = 0;
    } else {
        while res.num > 16 {
            res.num -= 4;
        }
        if res.num > 12 {
            res.stub = "season-";
            res.num -= 12;
        }
    }
    res
}

/// `CSL.Util.Dates.month.numeric`: the month number (a JSON number) or `""`.
pub fn month_numeric(num: &Value) -> Value {
    let n = normalize_month(num);
    if n == 0 {
        Value::String(String::new())
    } else {
        Value::from(n)
    }
}

/// `CSL.Util.Dates.month["numeric-leading-zeros"]`: `"05"`, or `""`.
pub fn month_numeric_leading_zeros(num: &Value) -> String {
    let n = normalize_month(num);
    if n == 0 {
        return String::new();
    }
    let mut s = n.to_string();
    while js::len(&s) < 2 {
        s = format!("0{s}");
    }
    s
}

fn month_term(
    state: &mut State,
    num: &Value,
    form: &str,
    force_default_locale: bool,
) -> Option<String> {
    let res = normalize_month_season(num);
    if res.num == 0 {
        return Some(String::new());
    }
    let mut n = res.num.to_string();
    while js::len(&n) < 2 {
        n = format!("0{n}");
    }
    let mut q = TermQuery::new(&format!("{}{}", res.stub, n));
    q.form = Some(form.to_string());
    q.plural = 0;
    q.force_default_locale = force_default_locale;
    input_get_term(state, &q)
}

/// `CSL.Util.Dates.month["long"]`: the locale's long month (or season) name;
/// `Some("")` for an out-of-range month, `None` if the term is missing
/// (JS `undefined`). `gender` is unused upstream.
pub fn month_long(
    state: &mut State,
    num: &Value,
    _gender: Option<&str>,
    force_default_locale: bool,
) -> Option<String> {
    month_term(state, num, "long", force_default_locale)
}

/// `CSL.Util.Dates.month["short"]`: as [`month_long`], short form.
pub fn month_short(
    state: &mut State,
    num: &Value,
    _gender: Option<&str>,
    force_default_locale: bool,
) -> Option<String> {
    month_term(state, num, "short", force_default_locale)
}

/// `CSL.Util.Dates.day.numeric` and `.day["long"]` (the same function): the
/// day as a string. Unlike the year formatters it does not guard a falsy
/// `num`: `null` is a TypeError, as upstream.
pub fn day_numeric(num: &Value) -> CslResult<String> {
    if num.is_null() {
        return Err(EngineError::BadInput(
            "Cannot read properties of undefined (reading 'toString')".into(),
        ));
    }
    Ok(js::to_js_string(num))
}

/// `CSL.Util.Dates.day["numeric-leading-zeros"]`: two digits, `"00"` for a
/// falsy day.
pub fn day_numeric_leading_zeros(num: &Value) -> String {
    let mut s = if js::truthy(num) {
        js::to_js_string(num)
    } else {
        "0".to_string()
    };
    while js::len(&s) < 2 {
        s = format!("0{s}");
    }
    s
}

/// `CSL.Util.Dates.day.ordinal`: `state.fun.ordinalizer.format(num, gender)`.
pub fn day_ordinal(
    state: &mut State,
    num: &Value,
    gender: Option<&str>,
) -> super::CslResult<String> {
    state.fun.ordinalizer.clone().format(state, num, gender)
}

impl State {
    /// `CSL.Util.Dates.year.imperial(state, num, end)` with the engine's own
    /// `state.tmp.date_object` ([`year_imperial`] takes it as a parameter).
    ///
    /// PORT-LATER(w2-render): the abbreviation step (`sys.normalizeAbbrevsKey`,
    /// `state.transform.abbrevs['default']['number']` and
    /// `state.transform.loadAbbreviation`, util_transform.js) is the render
    /// agent's; until it exists the era label is never replaced, which is what
    /// upstream does when the host supplies no abbreviations.
    pub fn year_imperial(&mut self, num: &Value, end: bool) -> CslResult<String> {
        let date_object: Obj = match &self.tmp.date_object {
            Value::Object(o) => o.clone(),
            _ => Obj::new(),
        };
        Ok(year_imperial(&date_object, num, end, |_label| None))
    }
}

#[cfg(test)]
mod tests {
    //! Differential tests against citeproc-js 2.4.63: reference
    //! `tests/data/csl/units/numbers.json` (generator
    //! `scripts/csl-units/numbers.cjs`), sections `year`, `month`, `day`,
    //! `imperial` and, per locale, `ord_engines[lang].dates` (the long and
    //! short month names with the locale terms citeproc-js read). Inputs are
    //! numbers, digit strings, text, `null`, `undefined`, booleans.
    use super::*;

    const REF: &str = include_str!("../../tests/data/csl/units/numbers.json");

    /// The reference encodes JS `undefined` as `{"undef": true}`.
    fn input(v: &Value) -> Value {
        if v.get("undef").is_some() {
            Value::Null
        } else {
            v.clone()
        }
    }

    fn out(v: &Value, key: &str) -> Value {
        v.get(key).cloned().unwrap_or(Value::Null)
    }

    #[test]
    fn year_month_day_formatters_match_citeproc_js() {
        let r: Value = serde_json::from_str(REF).expect("json");
        for c in r["year"].as_array().expect("year") {
            let n = input(&c["num"]);
            assert_eq!(Value::String(year_long(&n)), c["long"], "year.long {n}");
            match (year_short(&n), c.get("short_error")) {
                (Ok(v), None) => assert_eq!(
                    v.map(Value::String)
                        .unwrap_or(serde_json::json!({"undef": true})),
                    c["short"],
                    "year.short {n}"
                ),
                (Err(_), Some(_)) => {}
                (g, w) => panic!("year.short {n}: {g:?} vs {w:?}"),
            }
            assert_eq!(
                Value::String(year_numeric(&n)),
                c["numeric"],
                "year.numeric {n}"
            );
        }
        for c in r["month"].as_array().expect("month") {
            let n = input(&c["num"]);
            assert_eq!(month_numeric(&n), c["numeric"], "month.numeric {n}");
            assert_eq!(
                Value::String(month_numeric_leading_zeros(&n)),
                c["numeric-leading-zeros"],
                "month.numeric-leading-zeros {n}"
            );
            assert_eq!(
                Value::from(normalize_month(&n)),
                c["norm"],
                "normalizeMonth {n}"
            );
            let ms = normalize_month_season(&n);
            assert_eq!(
                serde_json::json!({"stub": ms.stub, "num": ms.num}),
                c["norm_season"],
                "normalizeMonth(.., true) {n}"
            );
        }
        for c in r["day"].as_array().expect("day") {
            let n = input(&c["num"]);
            match (day_numeric(&n), c.get("numeric_error")) {
                (Ok(v), None) => assert_eq!(Value::String(v), c["numeric"], "day.numeric {n}"),
                (Err(_), Some(_)) => {}
                (g, w) => panic!("day.numeric {n}: {g:?} vs {w:?}"),
            }
            assert_eq!(
                Value::String(day_numeric_leading_zeros(&n)),
                c["numeric-leading-zeros"],
                "day.numeric-leading-zeros {n}"
            );
        }
    }

    #[test]
    fn month_names_match_citeproc_js_in_seven_locales() {
        let r: Value = serde_json::from_str(REF).expect("json");
        let mut n = 0;
        for (lang, e) in r["ord_engines"].as_object().expect("engines") {
            let mut st = State::default();
            st.opt.insert("lang".into(), Value::String(lang.clone()));
            st.opt.insert(
                "default-locale".into(),
                Value::Array(vec![Value::String(lang.clone())]),
            );
            super::super::test_support::install_locale(
                &mut st,
                super::super::test_support::logged_locale(&e["log"], None, None),
            );
            for c in e["dates"].as_array().expect("dates") {
                let num = input(&c["num"]);
                let force = c["force"].as_bool().unwrap_or(false);
                let got = if c["fn"] == "long" {
                    month_long(&mut st, &num, None, force)
                } else {
                    month_short(&mut st, &num, None, force)
                };
                n += 1;
                if c.get("error").is_some() {
                    continue;
                }
                match (got, c.get("out")) {
                    (Some(s), Some(w)) => {
                        assert_eq!(Value::String(s), *w, "{lang} {} {num}", c["fn"])
                    }
                    (None, None) => assert!(
                        c["undef"].as_bool() == Some(true),
                        "{lang} {} {num}",
                        c["fn"]
                    ),
                    (g, w) => panic!("{lang} {} {num}: {g:?} vs {w:?}", c["fn"]),
                }
            }
        }
        assert!(n > 500);
    }

    #[test]
    fn imperial_years_match_citeproc_js() {
        let r: Value = serde_json::from_str(REF).expect("json");
        for c in r["imperial"].as_array().expect("imperial") {
            let (mo, d) = (c["month"].clone(), c["day"].clone());
            let mut date_object = Obj::new();
            date_object.insert("month".into(), mo.clone());
            date_object.insert("day".into(), d.clone());
            let bump = |v: &Value| match v.as_i64() {
                Some(n) if n != 0 => Value::from(n + 1),
                _ => Value::from(0),
            };
            date_object.insert("month_end".into(), bump(&mo));
            date_object.insert("day_end".into(), bump(&d));
            let abbr = c["abbr"].as_str().map(str::to_string);
            let got = year_imperial(
                &date_object,
                &c["year"],
                c["end"].as_bool().unwrap_or(false),
                |label| {
                    if label == "\u{5e73}\u{6210}" {
                        abbr.clone()
                    } else {
                        None
                    }
                },
            );
            assert_eq!(Value::String(got), c["out"], "imperial {c}");
        }
    }
}
