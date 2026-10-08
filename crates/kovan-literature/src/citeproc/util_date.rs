// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_date.js
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

//! Port of `src/util_date.js`: `CSL.dateMacroAsSortKey`, `CSL.dateAsSortKey`
//! and `CSL.Engine.prototype.dateParseArray`.
//!
//! `dateAsSortKey` appends strings to the output queue (`state.output`). It is
//! split into [`date_sort_key_parts`], which computes exactly the strings
//! upstream appends and in what order, and [`date_as_sort_key`], which is the
//! JS function: it appends them.

use serde_json::Value;

use super::js::{self, Obj};
use super::obj_token::Token;
use super::queue::{self, QueueId};
use super::state::State;
use super::load::{DATE_PARTS, DATE_PARTS_INTERNAL};
use super::util_dates;
use super::{CslResult, EngineError};


/// The strings `CSL.dateAsSortKey` appends to the output queue, in order,
/// and the flag they are appended with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateSortKey {
    /// `"empty"`, or `"macro-with-date"` for a macro when
    /// `state.tmp.extension` is set.
    pub macro_flag: &'static str,
    /// One string per `state.output.append(str, macroFlag)` call.
    pub parts: Vec<String>,
}

/// The core of `CSL.dateAsSortKey`: what it would append for variable
/// `variable` of `item` with the date parts `dateparts` (`this.dateparts`).
///
/// `date_parse_array` is `state.dateParseArray`; the parser is
/// `state.fun.dateparser`.
pub fn date_sort_key_parts(
    state: &State,
    item: &Value,
    variable: &str,
    dateparts: &[String],
) -> CslResult<Vec<String>> {
    let mut dp: Obj = match item.get(variable) {
        None => {
            let mut o = Obj::new();
            o.insert("date-parts".into(), serde_json::json!([[0]]));
            o
        }
        Some(Value::Object(o)) => o.clone(),
        // A non-object date: JS property reads on a string/number yield undefined.
        Some(_) => Obj::new(),
    };
    if js::get_truthy(&dp, "raw") {
        let raw = js::to_js_string(dp.get("raw").unwrap_or(&Value::Null));
        dp = state.fun.dateparser.parse_date_to_array(&raw);
    } else if js::get_truthy(&dp, "date-parts") {
        dp = date_parse_array(&dp)?;
    }
    let mut parts = Vec::new();
    if js::get_truthy(&dp, "year") {
        for elem in DATE_PARTS_INTERNAL {
            let mut value = Value::from(0);
            let e = elem.strip_suffix("_end").unwrap_or(elem);
            if js::get_truthy(&dp, elem) && dateparts.iter().any(|d| d == e) {
                value = dp.get(*elem).cloned().unwrap_or(Value::Null);
            }
            if js::slice(elem, 0, Some(4)) == "year" {
                let mut yr = util_dates::year_numeric(&value);
                let mut prefix = "1";
                if yr.starts_with('-') {
                    prefix = "0";
                    yr = js::slice(&yr, 1, None);
                    let n = 9999 - js::parse_int(&yr).unwrap_or(0);
                    yr = n.to_string();
                }
                parts.push(util_dates::year_numeric(&Value::String(format!(
                    "{prefix}{yr}"
                ))));
            } else {
                let mut v = match e {
                    "month" => util_dates::month_numeric_leading_zeros(&value),
                    _ => util_dates::day_numeric_leading_zeros(&value),
                };
                if v.is_empty() {
                    v = "00".to_string();
                }
                parts.push(v);
            }
        }
    }
    Ok(parts)
}

/// `CSL.dateAsSortKey.call(token, state, Item, isMacro)`: append the sort key
/// of the token's date variable to `state.output`, one
/// `state.output.append(part, macroFlag)` per part (util_date.js:49,56).
///
/// Sets `token.dateparts` to `["year","month","day"]` when unset (as the JS
/// does on `this`). Returns what was appended, for inspection.
pub fn date_as_sort_key(
    state: &mut State,
    token: &mut Token,
    item: &Value,
    is_macro: bool,
) -> CslResult<DateSortKey> {
    let variable = token
        .variables
        .first()
        .cloned()
        .ok_or_else(|| EngineError::BadInput("dateAsSortKey: token has no variable".into()))?;
    let macro_flag = if is_macro && !state.tmp.extension.is_empty() {
        "macro-with-date"
    } else {
        "empty"
    };
    if !token.extra.contains_key("dateparts") {
        token.extra.insert(
            "dateparts".into(),
            serde_json::json!(["year", "month", "day"]),
        );
    }
    let dateparts: Vec<String> = token
        .extra
        .get("dateparts")
        .and_then(Value::as_array)
        .map(|a| a.iter().map(js::to_js_string).collect())
        .unwrap_or_default();
    let parts = date_sort_key_parts(state, item, &variable, &dateparts)?;
    for part in &parts {
        queue::append_simple(state, QueueId::Output, part.as_str(), macro_flag)?;
    }
    Ok(DateSortKey { macro_flag, parts })
}

/// `CSL.dateMacroAsSortKey.call(token, state, Item)`: [`date_as_sort_key`]
/// with `isMacro = true`.
pub fn date_macro_as_sort_key(
    state: &mut State,
    token: &mut Token,
    item: &Value,
) -> CslResult<DateSortKey> {
    date_as_sort_key(state, token, item, true)
}

/// `CSL.Engine.prototype.dateParseArray(date_obj)`: turn a CSL-JSON date
/// (`date-parts` arrays) into the flat internal form (`year`, `month`, `day`,
/// `year_end`, ... as integers), copying every other field. A `date-parts`
/// whose two halves differ in length is a `CSL.error`.
pub fn date_parse_array(date_obj: &Obj) -> CslResult<Obj> {
    let mut ret = Obj::new();
    for (field, val) in date_obj {
        if field == "date-parts" {
            let dp: &[Value] = val.as_array().map(Vec::as_slice).unwrap_or(&[]);
            if dp.len() > 1 {
                let l0 = dp[0].as_array().map(Vec::len).unwrap_or(0);
                let l1 = dp[1].as_array().map(Vec::len).unwrap_or(0);
                if l0 != l1 {
                    // CSL.error(...): upstream throws the string "citeproc-js error: ..."
                    return Err(EngineError::Csl(
                        "CSL data error: element mismatch in date range input.".to_string(),
                    ));
                }
            }
            let exts = ["", "_end"];
            for (i, half) in dp.iter().enumerate() {
                for (j, part) in DATE_PARTS.iter().enumerate() {
                    let key = format!("{}{}", part, exts.get(i).copied().unwrap_or("undefined"));
                    let cell = half.as_array().and_then(|a| a.get(j));
                    match cell.and_then(js::parse_int_value) {
                        Some(n) => {
                            ret.insert(key, Value::from(n));
                        }
                        None => {
                            // ret[key] = undefined: an absent key in this model.
                            ret.remove(&key);
                        }
                    }
                }
            }
        } else if field == "literal"
            && val.is_object()
            && val.get("part").map(Value::is_string).unwrap_or(false)
        {
            // XXXX: temporary workaround (upstream)
            ret.insert(
                "literal".to_string(),
                val.get("part").cloned().unwrap_or(Value::Null),
            );
        } else {
            ret.insert(field.clone(), val.clone());
        }
    }
    Ok(ret)
}

impl State {
    /// `CSL.Engine.prototype.dateParseArray`: see [`date_parse_array`].
    pub fn date_parse_array(&self, date_obj: &Obj) -> CslResult<Obj> {
        date_parse_array(date_obj)
    }
}

#[cfg(test)]
mod tests {
    //! Differential tests against citeproc-js 2.4.63: reference
    //! `tests/data/csl/units/datekey.json` (generator
    //! `scripts/csl-units/datekey.cjs`): `dateAsSortKey` and
    //! `dateMacroAsSortKey` over 26 date shapes (date-parts, raw, literal,
    //! flat year/month/day, BC years, ranges, out-of-range parts), 7 `dateparts`
    //! settings and both `tmp.extension` values: the strings appended to the
    //! output queue and the flags they carry. `dateParseArray` is tested with
    //! the date parser (`dateparser.json`).
    use super::*;
    use crate::citeproc::obj_token::TokenType;

    const REF: &str = include_str!("../../tests/data/csl/units/datekey.json");

    #[test]
    fn date_parse_array_matches_citeproc_js() {
        let r: Value =
            serde_json::from_str(include_str!("../../tests/data/csl/units/dateparser.json"))
                .expect("json");
        let mut n = 0;
        for c in r["date_parse_array"].as_array().expect("cases") {
            let Value::Object(input) = &c["in"] else {
                continue;
            };
            n += 1;
            match (date_parse_array(input), c.get("error")) {
                (Ok(o), None) => assert_eq!(Value::Object(o), c["out"], "{c}"),
                (Err(e), Some(w)) => assert_eq!(Some(e.to_string().as_str()), w.as_str(), "{c}"),
                (g, w) => panic!("{c}: {g:?} vs {w:?}"),
            }
        }
        assert!(n > 100, "{n}");
    }

    #[test]
    fn date_sort_keys_match_citeproc_js() {
        let r: Value = serde_json::from_str(REF).expect("json");
        let mut n = 0;
        for c in r["cases"].as_array().expect("cases") {
            let mut state = State::default();
            let mut token = Token::new("key", TokenType::Singleton);
            token.variables = vec!["issued".to_string()];
            if let Some(dp) = c["dateparts"].as_array() {
                token
                    .extra
                    .insert("dateparts".into(), Value::Array(dp.clone()));
            }
            let is_macro = c["isMacro"].as_bool().unwrap_or(false);
            let ext = c["ext"].as_bool().unwrap_or(false);
            if ext {
                state.tmp.extension = "_sort".to_string();
            }
            // As inside the macro whose date this is (`doing-macro-with-date`, set
            // by expandMacro's closure): the queue then accepts the
            // "macro-with-date" token name.
            state.tmp.doing_macro_with_date = is_macro && ext;
            let got = date_as_sort_key(&mut state, &mut token, &c["item"], is_macro);
            n += 1;
            match (got, c.get("error")) {
                (Ok(k), None) => {
                    let want: Vec<(String, String)> = c["out"]
                        .as_array()
                        .expect("out")
                        .iter()
                        .map(|p| {
                            (
                                p[0].as_str().unwrap_or("").to_string(),
                                p[1].as_str().unwrap_or("").to_string(),
                            )
                        })
                        .collect();
                    let have: Vec<(String, String)> = k
                        .parts
                        .iter()
                        .map(|p| (p.clone(), k.macro_flag.to_string()))
                        .collect();
                    assert_eq!(have, want, "case {c}");
                    assert_eq!(
                        token.extra.get("dateparts").cloned().unwrap_or(Value::Null),
                        c["token_dateparts"],
                        "token.dateparts {c}"
                    );
                }
                (Err(e), Some(w)) => assert_eq!(Some(e.to_string().as_str()), w.as_str(), "{c}"),
                (g, w) => panic!("{c}: {g:?} vs {w:?}"),
            }
        }
        assert!(n > 1000);
    }
}
