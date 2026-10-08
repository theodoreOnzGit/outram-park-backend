// Part of the kovan port of citeproc-js (GitHub #790).
//
// Upstream:    citeproc-js, https://github.com/juris-m/citeproc-js
// Source:      src/util_dateparser.js
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

//! Port of `src/util_dateparser.js`: `CSL.DateParser`, the free-text date
//! parser behind `raw` dates.
//!
//! citeproc-js has ONE parser object per process (`CSL.DateParser = new
//! CSL.DateParser()`); the port has one [`DateParser`] per engine
//! (`state.fun.dateparser`). `addDateParserMonths` is idempotent, which makes
//! the two equivalent (checked in `scripts/csl-intermediate-reference.cjs`).
//!
//! A parsed date is a JS object, here a [`js::Obj`]: `year`, `month`, `day`,
//! `season`, `year_end`, ..., `circa`, `literal`. Numeric parts are JSON
//! numbers once parsed (as `parseInt` leaves them in JS), strings otherwise.
//!
//! The parser's tokenising regexps use the lookahead `(?![0-9])`, which the
//! `regex` crate lacks. [`rex_split`] is a hand-written matcher for exactly
//! those patterns (alternation, greedy quantifiers and backtracking as in V8).

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use super::js::{self, Obj};

// DUP-CHECK: load.js CSL.DATE_PARTS_ALL
const DATE_PARTS_ALL: [&str; 4] = ["year", "month", "day", "season"];

/// The Japanese imperial epochs and their year offsets (`epochPairs`).
const EPOCH_PAIRS: [(&str, i64); 4] = [
    ("\u{660E}\u{6CBB}", 1867),
    ("\u{5927}\u{6B63}", 1911),
    ("\u{662D}\u{548C}", 1925),
    ("\u{5E73}\u{6210}", 1988),
];

/// `epochMatcher` / `epochSplitter` (the pattern without the `g` flag is the
/// same): `(?:明治|大正|昭和|平成)(?:[0-9]+)`.
static EPOCH_RE: LazyLock<Regex> = LazyLock::new(|| {
    let names: Vec<&str> = EPOCH_PAIRS.iter().map(|p| p.0).collect();
    Regex::new(&format!("(?:{})(?:[0-9]+)", names.join("|"))).expect("static regex")
});

/// `/([^0-9]+)([0-9]+)/` applied to an epoch match.
static EPOCH_PARTS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("([^0-9]+)([0-9]+)").expect("static regex"));

/// `kanjiMonthDay`: `/(月|年)/g`.
static KANJI_MONTH_DAY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("(\u{6708}|\u{5E74})").expect("static regex"));

/// JS `.`: anything but `\n`, `\r`, U+2028, U+2029.
const DOT: &str = "[^\\n\\r\\u{2028}\\u{2029}]";

/// `/^(.*[0-9])T[0-9].*/`.
static ISO_TIME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("^({DOT}*[0-9])T[0-9]{DOT}*")).expect("static regex"));

/// `/^[0-9]{1,3}$/`.
static SHORT_YEAR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^[0-9]{1,3}$").expect("static regex"));

/// `/\s*[0-9]{2}:[0-9]{2}(?::[0-9]+)/` (the seconds group is mandatory).
static CLOCK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        "[{ws}]*[0-9]{{2}}:[0-9]{{2}}(?::[0-9]+)",
        ws = js::WS
    ))
    .expect("static regex")
});

/// `/\s+/g`.
static WS_RUN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("[{}]+", js::WS)).expect("static regex"));

/// `/\s*-\s*$/`.
static TRAILING_DASH_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("[{ws}]*-[{ws}]*$", ws = js::WS)).expect("static regex"));

/// `/\s*-\s*\//`.
static DASH_SLASH_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("[{ws}]*-[{ws}]*/", ws = js::WS)).expect("static regex"));

/// `/\.\s*$/`.
static TRAILING_DOT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("\\.[{ws}]*$", ws = js::WS)).expect("static regex"));

/// `/([A-Za-z])\./g`.
static ABBREV_DOT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("([A-Za-z])\\.").expect("static regex"));

/// `/^\s*([\-\/]|[^\-\/\~\?0-9]+|[\-~?0-9]+)\s*$/`.
static ELEMENT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        "^[{ws}]*(-|/|[^-/~?0-9]+|[-~?0-9]+)[{ws}]*$",
        ws = js::WS
    ))
    .expect("static regex")
});

/// `/[0-9]{4}/`.
static FOUR_DIGITS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("[0-9]{4}").expect("static regex"));

/// `/^cir/`.
static CIR_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new("^cir").expect("static regex"));

/// `/^[0-9]+$/`.
static ALL_DIGITS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^[0-9]+$").expect("static regex"));

/// `/^bc/`.
static BC_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new("^bc").expect("static regex"));

/// `/^ad/`.
static AD_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new("^ad").expect("static regex"));

/// `/(?:mic|tri|hil|eas)/`.
static CRUFT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("(?:mic|tri|hil|eas)").expect("static regex"));

/// One entry of `monthRexes`: the pattern source and its compiled form. A
/// month string that is not a valid regular expression never matches (JS
/// would have thrown while building it).
#[derive(Debug, Clone)]
pub struct MonthRex {
    /// The JS pattern source, e.g. `^(?:jan)`.
    pub source: String,
    /// Compiled `source`; `None` when it does not compile.
    pub re: Option<Regex>,
}

impl MonthRex {
    fn new(source: String) -> MonthRex {
        let re = Regex::new(&source).ok();
        MonthRex { source, re }
    }

    fn is_match(&self, text: &str) -> bool {
        self.re.as_ref().map(|r| r.is_match(text)).unwrap_or(false)
    }
}

impl PartialEq for MonthRex {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
    }
}

/// `CSL.DateParser` (one per engine). Mutable parts of the JS object:
/// `monthStrings`, `monthSets`, `monthAbbrevs`, `monthRexes`, `monthGuess`,
/// `dayGuess`.
#[derive(Debug, Clone, PartialEq)]
pub struct DateParser {
    /// `monthStrings`: the 18 default English month/season names.
    pub month_strings: Vec<String>,
    /// `monthSets`: per month, the strings known for it.
    pub month_sets: Vec<Vec<String>>,
    /// `monthAbbrevs`: per month, the (possibly extended) abbreviations.
    pub month_abbrevs: Vec<Vec<String>>,
    /// `monthRexes`: per month, the matcher built from `month_abbrevs`.
    pub month_rexes: Vec<MonthRex>,
    /// `monthGuess`: index of the month in a two-number date.
    pub month_guess: usize,
    /// `dayGuess`: index of the day in a two-number date.
    pub day_guess: usize,
}

impl Default for DateParser {
    fn default() -> Self {
        DateParser::new()
    }
}

/// The matcher for the splitting regexps `rexDash`, `rexDashSlash` and
/// `rexSlashDash`, which differ only in `numd` (`%%NUMD%%`, the delimiter
/// inside a numeric date) and `dated` (`%%DATED%%`, the range separator).
///
/// JS source: `(yearFirst|yearLast|numberVal|rangeSeparator|fuzzyChar|chars)`
/// with
/// `yearLast = (?:[?0-9]{1,2}NUMD){0,2}[?0-9]{4}(?![0-9])`,
/// `yearFirst = [?0-9]{4}(?:NUMD[?0-9]{1,2}){0,2}(?![0-9])`,
/// `numberVal = [?0-9]{1,3}`, `rangeSeparator = [DATED]`,
/// `fuzzyChar = [?~]`, `chars = [^-/~?0-9]+`.
/// Returns the end (exclusive, in `chars`) of the leftmost-first match
/// starting at `q`, or `None`.
fn rex_match_at(s: &[char], q: usize, numd: char, dated: char) -> Option<usize> {
    let qm_digit = |p: usize| p < s.len() && (s[p] == '?' || s[p].is_ascii_digit());
    let not_digit_at = |p: usize| !(p < s.len() && s[p].is_ascii_digit());
    let four = |p: usize| (0..4).all(|k| qm_digit(p + k));
    // (?:NUMD[?0-9]{1,2}) alternatives from `p`, greedy: 2 characters first.
    let group_first = |p: usize| -> Vec<usize> {
        let mut v = Vec::new();
        if p < s.len() && s[p] == numd {
            if qm_digit(p + 1) && qm_digit(p + 2) {
                v.push(p + 3);
            }
            if qm_digit(p + 1) {
                v.push(p + 2);
            }
        }
        v
    };
    // (?:[?0-9]{1,2}NUMD) alternatives from `p`.
    let group_last = |p: usize| -> Vec<usize> {
        let mut v = Vec::new();
        if qm_digit(p) && qm_digit(p + 1) && p + 2 < s.len() && s[p + 2] == numd {
            v.push(p + 3);
        }
        if qm_digit(p) && p + 1 < s.len() && s[p + 1] == numd {
            v.push(p + 2);
        }
        v
    };
    // {0,2} greedy over a group function: end positions in priority order.
    fn reps(p: usize, max: u8, group: &dyn Fn(usize) -> Vec<usize>, out: &mut Vec<usize>) {
        if max > 0 {
            for e in group(p) {
                reps(e, max - 1, group, out);
            }
        }
        out.push(p);
    }
    // 1. yearFirst
    if four(q) {
        let mut ends = Vec::new();
        reps(q + 4, 2, &group_first, &mut ends);
        for e in ends {
            if not_digit_at(e) {
                return Some(e);
            }
        }
    }
    // 2. yearLast
    {
        let mut starts = Vec::new();
        reps(q, 2, &group_last, &mut starts);
        for p in starts {
            if four(p) && not_digit_at(p + 4) {
                return Some(p + 4);
            }
        }
    }
    // 3. numberVal
    if qm_digit(q) {
        let mut e = q + 1;
        while e < q + 3 && qm_digit(e) {
            e += 1;
        }
        return Some(e);
    }
    // 4. rangeSeparator
    if q < s.len() && s[q] == dated {
        return Some(q + 1);
    }
    // 5. fuzzyChar
    if q < s.len() && (s[q] == '?' || s[q] == '~') {
        return Some(q + 1);
    }
    // 6. chars
    let is_char = |c: char| !matches!(c, '-' | '/' | '~' | '?') && !c.is_ascii_digit();
    if q < s.len() && is_char(s[q]) {
        let mut e = q + 1;
        while e < s.len() && is_char(s[e]) {
            e += 1;
        }
        return Some(e);
    }
    None
}

/// `txt.split(rex)` for the regexps above: the pieces between matches, with
/// each match (the single capture group) inserted after the piece before it.
fn rex_split(txt: &str, numd: char, dated: char) -> Vec<String> {
    let s: Vec<char> = txt.chars().collect();
    let mut out = Vec::new();
    let mut p = 0usize;
    let mut q = 0usize;
    while q < s.len() {
        match rex_match_at(&s, q, numd, dated) {
            Some(e) if e > p => {
                out.push(s[p..q].iter().collect());
                out.push(s[q..e].iter().collect());
                p = e;
                q = p;
            }
            _ => q += 1,
        }
    }
    out.push(s[p..].iter().collect());
    out
}

/// JS `str.replace(/^0*/, "")`: strip leading zeros.
fn strip_leading_zeros(s: &str) -> String {
    s.trim_start_matches('0').to_string()
}

/// JS truthiness of an optional string-or-number date part.
fn part_truthy(o: &Obj, key: &str) -> bool {
    js::get_truthy(o, key)
}

/// JS `Number(string)` for the digit strings the parser produces
/// (optionally signed); `NaN` otherwise.
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

/// A JS number (as `Value`): an integer when it is one.
fn number_value(f: f64) -> Value {
    if f.is_finite() && f == f.trunc() && f.abs() < 9.0e15 {
        Value::from(f as i64)
    } else {
        serde_json::Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }
}

impl DateParser {
    /// `new CSL.DateParser()`: `setOrderMonthDay()` then
    /// `resetDateParserMonths()`.
    pub fn new() -> DateParser {
        let month_string = "january february march april may june july august september october november december spring summer fall winter spring summer";
        let mut p = DateParser {
            month_strings: month_string.split(' ').map(str::to_string).collect(),
            month_sets: Vec::new(),
            month_abbrevs: Vec::new(),
            month_rexes: Vec::new(),
            month_guess: 0,
            day_guess: 1,
        };
        p.set_order_month_day();
        p.reset_date_parser_months();
        p
    }

    /// `setOrderDayMonth`: numeric dates read day first.
    pub fn set_order_day_month(&mut self) {
        self.month_guess = 1;
        self.day_guess = 0;
    }

    /// `setOrderMonthDay`: numeric dates read month first (the default).
    pub fn set_order_month_day(&mut self) {
        self.month_guess = 0;
        self.day_guess = 1;
    }

    /// `resetDateParserMonths`: back to the English defaults. Note the
    /// default matchers are NOT anchored (`(?:jan)`), unlike those built by
    /// [`DateParser::add_date_parser_months`] (`^(?:jan)`); upstream quirk.
    pub fn reset_date_parser_months(&mut self) {
        self.month_sets = self.month_strings.iter().map(|m| vec![m.clone()]).collect();
        self.month_abbrevs = self
            .month_sets
            .iter()
            .map(|set| set.iter().map(|m| js::slice(m, 0, Some(3))).collect())
            .collect();
        self.month_rexes = self
            .month_abbrevs
            .iter()
            .map(|a| MonthRex::new(format!("(?:{})", a.join("|"))))
            .collect();
    }

    /// `addDateParserMonths(lst)`: add a list of 12 months (or 16 with
    /// seasons), extending abbreviations to resolve ambiguities. A list of
    /// any other length is ignored. Idempotent.
    pub fn add_date_parser_months(&mut self, lst: &[String]) {
        if lst.len() != 12 && lst.len() != 16 {
            return; // CSL.debug(...) upstream
        }
        for i in 0..lst.len() {
            let mut abbrev_length: usize;
            let mut skip = false;
            let mut insert: usize = 3;
            let mut extended_sets: std::collections::BTreeMap<
                usize,
                std::collections::BTreeMap<usize, usize>,
            > = std::collections::BTreeMap::new();
            for j in 0..self.month_abbrevs.len() {
                extended_sets.insert(j, std::collections::BTreeMap::new());
                if j == i {
                    for k in 0..self.month_abbrevs[i].len() {
                        let a = &self.month_abbrevs[i][k];
                        if *a == js::slice(&lst[i], 0, Some(js::len(a) as i64)) {
                            skip = true;
                            break;
                        }
                    }
                } else {
                    for k in 0..self.month_abbrevs[j].len() {
                        abbrev_length = js::len(&self.month_abbrevs[j][k]);
                        if self.month_abbrevs[j][k]
                            == js::slice(&lst[i], 0, Some(abbrev_length as i64))
                        {
                            while js::slice(&self.month_sets[j][k], 0, Some(abbrev_length as i64))
                                == js::slice(&lst[i], 0, Some(abbrev_length as i64))
                            {
                                if abbrev_length > js::len(&lst[i])
                                    || abbrev_length > js::len(&self.month_sets[j][k])
                                {
                                    // unable to disambiguate month string
                                    break;
                                } else {
                                    abbrev_length += 1;
                                }
                            }
                            insert = abbrev_length;
                            if let Some(m) = extended_sets.get_mut(&j) {
                                m.insert(k, abbrev_length);
                            }
                        }
                    }
                }
                for (jk, inner) in extended_sets.clone() {
                    for (kk, al) in inner {
                        self.month_abbrevs[jk][kk] =
                            js::slice(&self.month_sets[jk][kk], 0, Some(al as i64));
                    }
                }
            }
            if !skip {
                self.month_sets[i].push(lst[i].clone());
                self.month_abbrevs[i].push(js::slice(&lst[i], 0, Some(insert as i64)));
            }
        }
        self.month_rexes = self
            .month_abbrevs
            .iter()
            .map(|a| MonthRex::new(format!("^(?:{})", a.join("|"))))
            .collect();
        if self.month_abbrevs.len() == 18 {
            for i in 12..14 {
                self.month_rexes[i + 4] =
                    MonthRex::new(format!("^(?:{})", self.month_abbrevs[i].join("|")));
            }
        }
    }

    /// `convertDateObjectToArray(thedate)`: converts the object in place
    /// (`year`/`month`/`day` and `_end` parts become `date-parts`) and
    /// returns it.
    pub fn convert_date_object_to_array(&self, mut thedate: Obj) -> Obj {
        let mut first: Vec<Value> = Vec::new();
        let mut slicelen = 0usize;
        for part in ["year", "month", "day"] {
            if !part_truthy(&thedate, part) {
                break;
            }
            slicelen += 1;
            if let Some(v) = thedate.remove(part) {
                first.push(v);
            }
        }
        let mut second: Vec<Value> = Vec::new();
        for part in ["year_end", "month_end", "day_end"].iter().take(slicelen) {
            if !part_truthy(&thedate, part) {
                break;
            }
            if let Some(v) = thedate.remove(*part) {
                second.push(v);
            }
        }
        let mut parts = vec![Value::Array(first.clone())];
        if first.len() == second.len() {
            parts.push(Value::Array(second));
        }
        thedate.insert("date-parts".to_string(), Value::Array(parts));
        thedate
    }

    /// `convertDateObjectToString(thedate)`: `year-month-day`, stopping at the
    /// first absent part. Cannot represent ranges (upstream says so).
    pub fn convert_date_object_to_string(&self, thedate: &Obj) -> String {
        let mut ret: Vec<String> = Vec::new();
        for part in DATE_PARTS_ALL.iter().take(3) {
            match thedate.get(*part) {
                Some(v) if js::truthy(v) => ret.push(js::to_js_string(v)),
                _ => break,
            }
        }
        ret.join("-")
    }

    /// `_parseNumericDate(ret, delim, suff, txt)`: fill `ret` from a numeric
    /// piece such as `2012-05-03` or `05/03`.
    fn parse_numeric_date(&self, ret: &mut Obj, delim: char, suff: &str, txt: &str) {
        let mut lst: Vec<String> = txt.split(delim).map(str::to_string).collect();
        for i in 0..lst.len() {
            if js::len(&lst[i]) == 4 {
                ret.insert(
                    format!("year{suff}"),
                    Value::String(strip_leading_zeros(&lst[i])),
                );
                if i == 0 {
                    lst = lst[1..].to_vec();
                } else {
                    lst.truncate(i);
                }
                break;
            }
        }
        // parseInt: None is NaN, and 0 is falsy as well.
        let nums: Vec<Option<i64>> = lst.iter().map(|x| js::parse_int(x)).collect();
        let truthy = |x: Option<i64>| matches!(x, Some(n) if n != 0);
        let show = |x: Option<i64>| Value::String(x.map(|n| n.to_string()).unwrap_or_default());
        if nums.len() == 1 || (nums.len() == 2 && !truthy(nums[1])) {
            let month = nums[0];
            if truthy(month) {
                ret.insert(format!("month{suff}"), show(nums[0]));
            }
        } else if nums.len() == 2 {
            let mg = nums[self.month_guess];
            let dg = nums[self.day_guess];
            let (month, day) = if matches!(mg, Some(n) if n > 12) {
                (dg, mg)
            } else {
                (mg, dg)
            };
            if truthy(month) {
                ret.insert(format!("month{suff}"), show(month));
                if truthy(day) {
                    ret.insert(format!("day{suff}"), show(day));
                }
            }
        }
    }

    /// `parseDateToObject(txt)`: parse a date string into a date object
    /// (`{year, month, day, ..._end, season, circa}`), or `{literal: txt}`
    /// when no year can be found.
    pub fn parse_date_to_object(&self, txt_in: &str) -> Obj {
        let orig = txt_in.to_string();
        let mut txt = txt_in.to_string();
        let mut slash_pos: i64 = -1;
        let mut dash_pos: i64 = -1;
        let mut year_is_negative = false;
        if !txt.is_empty() {
            txt = ISO_TIME_RE.replace(&txt, "${1}").into_owned();
            if js::slice(&txt, 0, Some(1)) == "-" {
                year_is_negative = true;
                txt = js::slice(&txt, 1, None);
            }
            if SHORT_YEAR_RE.is_match(&txt) {
                while js::len(&txt) < 4 {
                    txt = format!("0{txt}");
                }
            }
            txt = CLOCK_RE.replace(&txt, "").into_owned();
            if KANJI_MONTH_DAY_RE.is_match(&txt) {
                txt = WS_RUN_RE.replace_all(&txt, "").into_owned();
                txt = txt.replace('\u{65E5}', ""); // kanjiYear (sic)
                txt = KANJI_MONTH_DAY_RE.replace_all(&txt, "-").into_owned();
                txt = txt.replace('\u{301c}', "/");
                txt = txt.replace("-/", "/");
                if txt.ends_with('-') {
                    txt.pop();
                }
                // Era names: replace each "<era><digits>" by the Gregorian year.
                if EPOCH_RE.is_match(&txt) {
                    txt = EPOCH_RE
                        .replace_all(&txt, |c: &regex::Captures| {
                            let m = c.get(0).map(|m| m.as_str()).unwrap_or("");
                            match EPOCH_PARTS_RE.captures(m) {
                                Some(pp) => {
                                    let era = pp.get(1).map(|x| x.as_str()).unwrap_or("");
                                    let digits = pp.get(2).map(|x| x.as_str()).unwrap_or("");
                                    let base = EPOCH_PAIRS
                                        .iter()
                                        .find(|e| e.0 == era)
                                        .map(|e| e.1)
                                        .unwrap_or(0);
                                    (base + js::parse_int(digits).unwrap_or(0)).to_string()
                                }
                                None => String::new(),
                            }
                        })
                        .into_owned();
                }
                txt = TRAILING_DASH_RE.replace(&txt, "").into_owned();
                txt = DASH_SLASH_RE.replace(&txt, "/").into_owned();
                txt = TRAILING_DOT_RE.replace(&txt, "").into_owned();
                // txt.replace(/\.(?! )/, ""): the first "." not followed by a space.
                if let Some(pos) = txt
                    .char_indices()
                    .find(|(i, c)| *c == '.' && !txt[i + 1..].starts_with(' '))
                    .map(|(i, _)| i)
                {
                    txt.replace_range(pos..pos + 1, "");
                }
                slash_pos = js::index_of(&txt, "/", 0);
                dash_pos = js::index_of(&txt, "-", 0);
            }
        }
        // drop punctuation from a.d., b.c.
        txt = ABBREV_DOT_RE.replace_all(&txt, "${1}").into_owned();

        let mut number = String::new();
        let mut note = String::new();
        let mut thedate = Obj::new();
        if js::slice(&txt, 0, Some(1)) == "\"" && js::slice(&txt, -1, None) == "\"" {
            thedate.insert(
                "literal".to_string(),
                Value::String(js::slice(&txt, 1, Some(-1))),
            );
            return thedate;
        }
        let (range_delim, date_delim, lst): (char, char, Vec<String>);
        if slash_pos > -1 && dash_pos > -1 {
            let slash_count = txt.split('/').count();
            if slash_count > 3 {
                range_delim = '-';
                txt = txt.replace('_', "-");
                date_delim = '/';
                lst = rex_split(&txt, '/', '-');
            } else {
                range_delim = '/';
                txt = txt.replace('_', "/");
                date_delim = '-';
                lst = rex_split(&txt, '-', '/');
            }
        } else {
            txt = txt.replace('/', "-");
            txt = txt.replace('_', "-");
            range_delim = '-';
            date_delim = '-';
            lst = rex_split(&txt, '-', '-');
        }
        let mut ret: Vec<String> = Vec::new();
        for piece in &lst {
            if let Some(m) = ELEMENT_RE.captures(piece) {
                if let Some(g) = m.get(1) {
                    ret.push(g.as_str().to_string());
                }
            }
        }
        // Phase 2
        let range_delim_s = range_delim.to_string();
        let delim_pos = ret.iter().position(|x| *x == range_delim_s);
        let mut delims: Vec<(usize, usize)> = Vec::new();
        let mut is_range = false;
        if let Some(dp) = delim_pos {
            delims.push((0, dp));
            delims.push((dp + 1, ret.len()));
            is_range = true;
        } else {
            delims.push((0, ret.len()));
        }
        let mut suff = String::new();
        for (a, b) in &delims {
            let date: Vec<String> = ret[*a..*b].to_vec();
            'outer: for element in &date {
                if element.contains(date_delim) {
                    self.parse_numeric_date(&mut thedate, date_delim, &suff, element);
                    continue;
                }
                if FOUR_DIGITS_RE.is_match(element) {
                    thedate.insert(
                        format!("year{suff}"),
                        Value::String(strip_leading_zeros(element)),
                    );
                    continue;
                }
                if element == "~" || element == "?" || element == "c" || CIR_RE.is_match(element) {
                    thedate.insert("circa".to_string(), Value::Bool(true));
                }
                let lower = element.to_lowercase();
                for (k, mrex) in self.month_rexes.iter().enumerate() {
                    if mrex.is_match(&lower) {
                        thedate.insert(format!("month{suff}"), Value::String((k + 1).to_string()));
                        continue 'outer;
                    }
                }
                if ALL_DIGITS_RE.is_match(element) {
                    number = element.clone();
                }
                if BC_RE.is_match(&lower) && !number.is_empty() {
                    let n = number.parse::<f64>().unwrap_or(f64::NAN) * -1.0;
                    thedate.insert(
                        format!("year{suff}"),
                        Value::String(js::number_to_js_string(n)),
                    );
                    number = String::new();
                    continue;
                }
                if AD_RE.is_match(&lower) && !number.is_empty() {
                    thedate.insert(format!("year{suff}"), Value::String(number.clone()));
                    number = String::new();
                    continue;
                }
                if CRUFT_RE.is_match(&lower) && !part_truthy(&thedate, &format!("season{suff}")) {
                    note = element.clone();
                    continue;
                }
            }
            if !number.is_empty() {
                thedate.insert(format!("day{suff}"), Value::String(number.clone()));
                number = String::new();
            }
            if !note.is_empty() && !part_truthy(&thedate, &format!("season{suff}")) {
                thedate.insert(
                    format!("season{suff}"),
                    Value::String(js::trim(&note).to_string()),
                );
                note = String::new();
            }
            suff = "_end".to_string();
        }
        if is_range {
            for item in DATE_PARTS_ALL {
                let end = format!("{item}_end");
                if part_truthy(&thedate, item) && !part_truthy(&thedate, &end) {
                    if let Some(v) = thedate.get(item).cloned() {
                        thedate.insert(end, v);
                    }
                } else if !part_truthy(&thedate, item) && part_truthy(&thedate, &end) {
                    if let Some(v) = thedate.get(&end).cloned() {
                        thedate.insert(item.to_string(), v);
                    }
                }
            }
        }
        if !part_truthy(&thedate, "year")
            || (part_truthy(&thedate, "year")
                && part_truthy(&thedate, "day")
                && !part_truthy(&thedate, "month"))
        {
            thedate = Obj::new();
            thedate.insert("literal".to_string(), Value::String(orig));
        }
        for part in ["year", "month", "day", "year_end", "month_end", "day_end"] {
            if let Some(Value::String(s)) = thedate.get(part) {
                if ALL_DIGITS_RE.is_match(s) {
                    let n = js::parse_int(s).unwrap_or(0);
                    thedate.insert(part.to_string(), Value::from(n));
                }
            }
        }
        if year_is_negative && thedate.contains_key("year") {
            let y = thedate.get("year").map(to_number).unwrap_or(f64::NAN) * -1.0;
            thedate.insert("year".to_string(), number_value(y));
        }
        thedate
    }

    /// `parseDateToArray(txt)`: [`parse_date_to_object`] then
    /// [`convert_date_object_to_array`].
    ///
    /// [`parse_date_to_object`]: DateParser::parse_date_to_object
    /// [`convert_date_object_to_array`]: DateParser::convert_date_object_to_array
    pub fn parse_date_to_array(&self, txt: &str) -> Obj {
        self.convert_date_object_to_array(self.parse_date_to_object(txt))
    }

    /// `parseDateToString(txt)`: [`parse_date_to_object`] then
    /// [`convert_date_object_to_string`].
    ///
    /// [`parse_date_to_object`]: DateParser::parse_date_to_object
    /// [`convert_date_object_to_string`]: DateParser::convert_date_object_to_string
    pub fn parse_date_to_string(&self, txt: &str) -> String {
        self.convert_date_object_to_string(&self.parse_date_to_object(txt))
    }

    /// `parse(txt)`: alias of [`parse_date_to_object`](DateParser::parse_date_to_object).
    pub fn parse(&self, txt: &str) -> Obj {
        self.parse_date_to_object(txt)
    }
}

#[cfg(test)]
mod tests {
    //! Differential tests against citeproc-js 2.4.63. The reference is
    //! `tests/data/csl/units/dateparser.json`, generated by
    //! `scripts/csl-units/dateparser.cjs` (node, run by hand): for ~455 date
    //! strings (every `raw` date in the CSL test suite's fixtures plus
    //! generated ranges, seasons, circa, BC/AD, kanji eras, months in several
    //! locales, slashes, dashes, partial dates) the results of
    //! `parseDateToObject`, `parseDateToArray` and `parseDateToString`, replayed
    //! after each of 13 `addDateParserMonths` stages applied cumulatively to
    //! one parser. Pass criterion: every output equal to citeproc-js's.
    use super::*;

    const REF: &str = include_str!("../../tests/data/csl/units/dateparser.json");

    fn strings_of(v: &Value) -> Vec<String> {
        v.as_array()
            .map(|a| a.iter().map(|x| js::to_js_string(x)).collect())
            .unwrap_or_default()
    }

    #[test]
    fn parser_matches_citeproc_js_at_every_month_stage() {
        let reference: Value = serde_json::from_str(REF).expect("reference json");
        let lists = reference["month_lists"].as_object().expect("lists");
        let mut parser = DateParser::new();
        let mut checked = 0usize;
        let mut mismatches: Vec<String> = Vec::new();
        for stage in reference["stages"].as_array().expect("stages") {
            let label = stage["label"].as_str().unwrap_or("");
            if let Some(added) = stage["added"].as_str() {
                if added == "reset" {
                    parser.set_order_month_day();
                    parser.reset_date_parser_months();
                } else if added == "string" {
                    let l: Vec<String> =
                        "janu febr marc apri mayy june july augu sept octo nove dece"
                            .split(' ')
                            .map(str::to_string)
                            .collect();
                    parser.add_date_parser_months(&l);
                } else {
                    parser.add_date_parser_months(&strings_of(&lists[added]));
                }
            }
            let want_rexes = strings_of(&stage["month_rexes"]);
            let got_rexes: Vec<String> = parser
                .month_rexes
                .iter()
                .map(|r| r.source.clone())
                .collect();
            assert_eq!(got_rexes, want_rexes, "month regexps at stage {label}");
            let want_abbrevs: Vec<Vec<String>> = stage["month_abbrevs"]
                .as_array()
                .expect("abbrevs")
                .iter()
                .map(strings_of)
                .collect();
            assert_eq!(
                parser.month_abbrevs, want_abbrevs,
                "abbrevs at stage {label}"
            );
            for c in stage["cases"].as_array().expect("cases") {
                let input = c["in"].as_str().expect("input");
                let obj = parser.parse_date_to_object(input);
                let arr = parser.parse_date_to_array(input);
                let s = parser.parse_date_to_string(input);
                let pairs = [
                    ("obj", Value::Object(obj)),
                    ("arr", Value::Object(arr)),
                    ("str", Value::String(s)),
                ];
                for (k, got) in pairs {
                    checked += 1;
                    if got != c[k] {
                        mismatches
                            .push(format!("[{label}] {k}({input:?}): want {} got {got}", c[k]));
                    }
                }
            }
        }
        assert!(
            mismatches.is_empty(),
            "{} of {checked} differ, first: {:#?}",
            mismatches.len(),
            &mismatches[..mismatches.len().min(15)]
        );
        assert!(checked > 15000, "checked {checked}");
    }

    #[test]
    fn day_month_order_matches_citeproc_js() {
        let reference: Value = serde_json::from_str(REF).expect("reference json");
        let lists = reference["month_lists"].as_object().expect("lists");
        let mut parser = DateParser::new();
        for name in [
            "turkish",
            "turkish",
            "french",
            "french_short",
            "german",
            "spanish",
            "english_again",
            "short12",
            "wrong_len",
            "weird",
        ] {
            parser.add_date_parser_months(&strings_of(&lists[name]));
        }
        let l: Vec<String> = "janu febr marc apri mayy june july augu sept octo nove dece"
            .split(' ')
            .map(str::to_string)
            .collect();
        parser.add_date_parser_months(&l);
        parser.set_order_day_month();
        let mut n = 0;
        for set in reference["numeric_order"].as_array().expect("order") {
            for c in set["cases"].as_array().expect("cases") {
                let input = c["in"].as_str().expect("input");
                assert_eq!(
                    Value::Object(parser.parse_date_to_object(input)),
                    c["obj"],
                    "day-month order, input {input:?}"
                );
                n += 1;
            }
        }
        assert!(n > 400);
    }

    #[test]
    fn rex_split_handles_the_lookahead_cases() {
        // yearFirst must not end before a further digit: "20123" is not a year.
        assert_eq!(rex_split("20123", '-', '-'), vec!["", "201", "", "23", ""]);
        assert_eq!(
            rex_split("2012-05-03", '-', '-'),
            vec!["", "2012-05-03", ""]
        );
        assert_eq!(
            rex_split("May 2012", '-', '-'),
            vec!["", "May ", "", "2012", ""]
        );
    }
}
