// Part of the kovan Zotero port (GitHub #747, #748).
//
// Upstream: Zotero utilities, https://github.com/zotero/utilities
//   (commit 4051881d59c6), date.js (Zotero.Date) and utilities.js
//   (trimInternal, lpad); month names from Zotero
//   https://github.com/zotero/zotero (commit 9cbba8c4d281)
//   resource/schema/dateFormats.json (en-US), day suffixes from
//   chrome/locale/en-US/zotero/zotero.properties:802.
// Copyright (c) 2009 Center for History and New Media, George Mason
// University, Fairfax, Virginia, USA; (c) Corporation for Digital Scholarship.
// Licence: AGPL-3.0 (upstream: AGPL-3.0-or-later).

//! Zotero's date handling (`Zotero.Date`), the parts that the CSL-JSON
//! conversion and the Zotero item model need.
//!
//! Ported: `strToDate` (date.js:272), `formatDate` (long form, :567),
//! `strToISO` (:600), `sqlToISO8601` (:617), `parseEraYear` (:744),
//! `looksLikeEDTF` (:766), `parseEDTF` (:799), `strToMultipart` (:902),
//! `multipartToSQL` (:966), `isMultipart` (:954) and `multipartToStr` (:983)
//! (added 2026-10-07, #750), `isSQLDate`/`isSQLDateTime`/
//! `isSQLDateTimeWithoutSeconds` (:1019-1034), `isISODate` (:235), and the
//! ISO -> SQL and UTC -> local conversions of `isoToDate`/`dateToSQL`/
//! `sqlToDate`.
//!
//! **Deliberate differences, all stated:**
//!
//! * **Locale.** Month names are en-US only (Zotero adds the UI locale's
//!   months to English ones; date.js:83-97). A Chinese or French month name is
//!   not recognised. Two-digit-year windowing and US/European day-month order
//!   come from [`DateOptions`] instead of the clock and `Zotero.locale`.
//! * **Time zone.** JavaScript's `Date` uses the machine's zone; here "local
//!   time" is UTC shifted by [`DateOptions::utc_offset_minutes`] (default 0,
//!   i.e. local = UTC). No daylight-saving rules.
//! * **`undefined` parts.** Upstream stores a non-participating regex group
//!   as `undefined` and later concatenates it into `part`, which yields the
//!   literal text `"undefined"` (e.g. `strToDate("2021 May").part`). This port
//!   reproduces that, because upstream is the specification; it is marked
//!   where it happens.

use regex::Regex;
use std::sync::OnceLock;

/// en-US month names, `dateFormats.json` `en-US.short` then `.long`
/// (zotero resource/schema/dateFormats.json).
const MONTHS_SHORT: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const MONTHS_LONG: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// What upstream reads from the environment: the clock, the locale and the
/// time zone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateOptions {
    /// The current year, for two-digit years (date.js:351-364: a two-digit
    /// year up to this year's last two digits is this century, otherwise the
    /// previous one).
    pub current_year: i64,
    /// Read an ambiguous `a/b/c` as month/day (US, FM, PW, PH locales,
    /// date.js:310-314) rather than day/month.
    pub month_first: bool,
    /// Day suffixes accepted after a day number ("26th"). The Zotero client
    /// uses the locale string `date.daySuffixes` ("st, nd, rd, th" for
    /// en-US); the standalone utilities (no client) use none (date.js:434).
    pub day_suffixes: Vec<String>,
    /// Minutes east of UTC of "local time" (`Date#getTimezoneOffset` with
    /// the sign flipped). Used only for the access-date conversion in
    /// `itemToCSLJSON` (utilities_item.js:306-315).
    pub utc_offset_minutes: i32,
}

impl Default for DateOptions {
    /// The Zotero client in en-US, UTC, with `current_year` from the system
    /// clock (2026 on a target without one, e.g. `wasm32-unknown-unknown`).
    fn default() -> Self {
        DateOptions {
            current_year: system_year(),
            month_first: true,
            day_suffixes: vec!["st".into(), "nd".into(), "rd".into(), "th".into()],
            utc_offset_minutes: 0,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn system_year() -> i64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => civil_from_days((d.as_secs() / 86_400) as i64).0,
        Err(_) => 2026,
    }
}

#[cfg(target_arch = "wasm32")]
fn system_year() -> i64 {
    // `SystemTime::now` panics on wasm32-unknown-unknown.
    2026
}

/// The year of a [`StrDate`], which upstream keeps as a string ("2010",
/// "200 BCE", "circa 1995").
pub type YearString = String;

/// The result of [`str_to_date`] (upstream's plain object).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StrDate {
    /// The year as a string, possibly with an era marker or circa prefix.
    pub year: Option<YearString>,
    /// The month, **0-indexed** as in JavaScript (January = 0).
    pub month: Option<u32>,
    /// The day of the month (1-31).
    pub day: Option<u32>,
    /// Anything left over (e.g. a season "Summer"), cleaned of punctuation
    /// at both ends.
    pub part: Option<String>,
    /// The order the parts appeared in: a string over `y`, `m`, `d`.
    pub order: String,
}

/// Year, month (0-indexed) and day of one end of a [`EdtfDate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdtfParts {
    /// Signed year (1 BCE = 0, as in EDTF).
    pub year: i64,
    /// Month, 0-indexed.
    pub month: Option<u32>,
    /// Day of the month.
    pub day: Option<u32>,
}

/// The result of [`parse_edtf`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdtfDate {
    /// The date, or the start of the interval.
    pub begin: EdtfParts,
    /// The end of a closed interval.
    pub end: Option<EdtfParts>,
    /// Whether any part is uncertain or approximate (`~`, `?`, `%`).
    pub circa: bool,
}

fn re(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).expect("static regex compiles"))
}

const BCE_MARKER: &str = r"B\.?\s?C\.?(?:\s?E\.?)?";
const CE_MARKER: &str = r"C\.?\s?E\.?|A\.?\s?D\.?";

fn bce_marker_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, &format!("(?i)^(?:{BCE_MARKER})$"))
}
fn era_year_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(
        &R,
        &format!(r"(?i)^(?:({CE_MARKER})\s*)?([0-9]{{1,4}})(?:\s*({BCE_MARKER}|{CE_MARKER}))?$"),
    )
}
fn era_range_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(
        &R,
        &format!(
            r"(?i)^(?:({CE_MARKER})\s*)?([0-9]{{1,4}})(?:\s*({BCE_MARKER}|{CE_MARKER}))?\s*-\s*([0-9]{{1,4}})(?:\s*({BCE_MARKER}|{CE_MARKER}))?$"
        ),
    )
}
fn era_marker_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(
        &R,
        &format!(r"(?i)[0-9]\s*(?:{BCE_MARKER}|{CE_MARKER})|(?:{CE_MARKER})\s*[0-9]"),
    )
}
fn era_markers_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, &format!("(?i){BCE_MARKER}|{CE_MARKER}"))
}
fn negative_year_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, r"(?:^|/)-[0-9]")
}
fn circa_prefix_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, r"(?i)^(?:~|circa |ca\.? ?|c\. ?|about |around )\s*")
}
fn edtf_chars_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, r"(?i)^[-0-9T:+Z~?%/\s.]*$")
}
fn edtf_shape_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, r"^-?[0-9]{4}[-0-9~?%]*(/-?[0-9]{4}[-0-9~?%]*)?$")
}

/// `_normalizeDashes` (date.js:713): en dash, em dash and minus sign become
/// hyphens, and runs of hyphens become one.
fn normalize_dashes(s: &str) -> String {
    let s: String = s
        .chars()
        .map(|c| match c {
            '\u{2013}' | '\u{2014}' | '\u{2212}' => '-',
            c => c,
        })
        .collect();
    let mut out = String::with_capacity(s.len());
    let mut prev_dash = false;
    for c in s.chars() {
        if c == '-' {
            if !prev_dash {
                out.push('-');
            }
            prev_dash = true;
        } else {
            out.push(c);
            prev_dash = false;
        }
    }
    out
}

/// JavaScript `String.prototype.trim`.
fn js_trim(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
}

/// `_toEDTFYear` (date.js:659).
fn to_edtf_year(year: &str, marker: &str) -> String {
    let sign = if bce_marker_re().is_match(marker) {
        "-"
    } else {
        ""
    };
    format!("{sign}{}", lpad(year, '0', 4))
}

/// `_eraToEDTF` (date.js:674).
fn era_to_edtf(s: &str) -> Option<String> {
    let caps = era_range_re()
        .captures(s)
        .or_else(|| era_year_re().captures(s))?;
    let get = |i: usize| caps.get(i).map(|m| m.as_str()).filter(|s| !s.is_empty());
    let prefix = get(1);
    let begin_year = get(2)?;
    let begin_marker = get(3);
    // The single-year regex has no groups 4/5.
    let end_year = if caps.len() > 4 { get(4) } else { None };
    let end_marker = if caps.len() > 5 { get(5) } else { None };
    if prefix.is_some() && begin_marker.is_some() {
        return None;
    }
    let begin_marker = prefix.or(begin_marker);
    if begin_marker.is_none() && end_marker.is_none() {
        return None;
    }
    let all_zero = |y: &str| y.chars().all(|c| c == '0');
    if all_zero(begin_year) || end_year.is_some_and(all_zero) {
        return None;
    }
    let mut edtf = to_edtf_year(begin_year, begin_marker.or(end_marker).unwrap_or(""));
    if let Some(end_year) = end_year {
        edtf.push('/');
        edtf.push_str(&to_edtf_year(
            end_year,
            end_marker.or(begin_marker).unwrap_or(""),
        ));
    }
    Some(edtf)
}

/// `_hasEraNotation` (date.js:729).
fn has_era_notation(s: &str) -> bool {
    negative_year_re().is_match(s)
        || (era_marker_re().is_match(s)
            && edtf_chars_re().is_match(&era_markers_re().replace_all(s, "")))
}

/// `Zotero.Date.parseEraYear` (date.js:744): a single year with an era
/// marker ("200 BCE", "AD 200") as a signed year; `None` otherwise.
pub fn parse_era_year(s: &str) -> Option<i64> {
    let edtf = era_to_edtf(&normalize_dashes(js_trim(s)))?;
    if edtf.contains('/') {
        return None;
    }
    edtf.parse().ok()
}

/// `Zotero.Date.looksLikeEDTF` (date.js:766): an EDTF-shaped value, a
/// negative year or an era marker.
pub fn looks_like_edtf(s: &str) -> bool {
    let s = normalize_dashes(js_trim(s));
    let s = circa_prefix_re().replace(&s, "");
    has_era_notation(&s) || edtf_shape_re().is_match(&s)
}

/// Days in a 1-indexed month, proleptic Gregorian (`_daysInMonth`, date.js:718).
fn days_in_month(year: i64, month: u32) -> u32 {
    if month == 2 {
        return if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
            29
        } else {
            28
        };
    }
    if [4, 6, 9, 11].contains(&month) {
        30
    } else {
        31
    }
}

/// Pad unpadded negative years at the start of each interval end
/// (date.js:819-822, `/(^|\/)-([0-9]{1,3})(?=[-~?%/]|$)/g`, written out
/// because the `regex` crate has no look-ahead).
fn pad_negative_years(s: &str) -> String {
    s.split('/')
        .map(|seg| {
            let Some(rest) = seg.strip_prefix('-') else {
                return seg.to_owned();
            };
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            let after = &rest[digits.len()..];
            let ok_after = after.is_empty() || after.starts_with(['-', '~', '?', '%']);
            if (1..=3).contains(&digits.len()) && ok_after {
                format!("-{}{after}", lpad(&digits, '0', 4))
            } else {
                seg.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// `Zotero.Date.parseEDTF` (date.js:799): the subset of EDTF (ISO 8601-2)
/// that Zotero reads, plus its normalisations (dash ranges, condensed ranges,
/// circa prefixes, era markers). `None` where upstream returns `false`.
pub fn parse_edtf(s: &str) -> Option<EdtfDate> {
    let mut s = normalize_dashes(js_trim(s));
    let circa = circa_prefix_re().is_match(&s);
    if circa {
        s = circa_prefix_re().replace(&s, "").into_owned();
    }
    if let Some(era) = era_to_edtf(&s) {
        s = era;
    }
    // "EDTF contains no spaces and begins with a digit or minus sign"
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c == '-' || c.is_ascii_digit() => {}
        _ => return None,
    }
    if s.chars().any(char::is_whitespace) {
        return None;
    }
    s = pad_negative_years(&s);
    // Dash-separated year ranges, including condensed ones.
    {
        static R: OnceLock<Regex> = OnceLock::new();
        let range = re(&R, r"^([0-9]{4})-([0-9]{2}|[0-9]{4})$");
        if let Some(c) = range.captures(&s) {
            let begin: i64 = c[1].parse().unwrap_or(0);
            let mut end: i64 = c[2].parse().unwrap_or(0);
            if c[2].len() == 2 {
                end = if end > 12 {
                    (begin / 100) * 100 + end
                } else {
                    0
                };
            }
            if end > begin {
                s = format!("{}/{}", &c[1], lpad(&end.to_string(), '0', 4));
            }
        }
    }
    if circa && !s.ends_with(['~', '?', '%']) {
        s.push('~');
    }
    if !edtf_shape_re().is_match(&s) {
        return None;
    }

    fn parse_date(d: &str) -> Option<(EdtfParts, bool)> {
        static R: OnceLock<Regex> = OnceLock::new();
        let r = re(
            &R,
            r"^(-?[0-9]{4})(?:-([0-9]{2})(?:-([0-9]{2}))?)?([~?%])?$",
        );
        let m = r.captures(d)?;
        let year: i64 = m[1].parse().ok()?;
        let mut parts = EdtfParts {
            year,
            month: None,
            day: None,
        };
        if let Some(mm) = m.get(2) {
            let month: u32 = mm.as_str().parse().ok()?;
            if !(1..=12).contains(&month) {
                return None;
            }
            parts.month = Some(month - 1);
            if let Some(dd) = m.get(3) {
                let day: u32 = dd.as_str().parse().ok()?;
                if day < 1 || day > days_in_month(year, month) {
                    return None;
                }
                parts.day = Some(day);
            }
        }
        Some((parts, m.get(4).is_some()))
    }

    let mut split = s.split('/');
    let begin_str = split.next().unwrap_or("");
    let end_str = split.next();
    let (begin, begin_circa) = parse_date(begin_str)?;
    let mut result = EdtfDate {
        begin,
        end: None,
        circa: begin_circa,
    };
    if let Some(end_str) = end_str {
        let (end, end_circa) = parse_date(end_str)?;
        result.end = Some(end);
        result.circa |= end_circa;
        let comparable = |d: &EdtfParts| {
            d.year * 10000
                + d.month.map(|m| (m as i64 + 1) * 100).unwrap_or(0)
                + d.day.unwrap_or(0) as i64
        };
        if comparable(&end) <= comparable(&result.begin) {
            return None;
        }
    }
    Some(result)
}

/// `Zotero.Utilities.lpad` (utilities.js:975).
pub fn lpad(s: &str, pad: char, length: usize) -> String {
    let n = s.chars().count();
    if n >= length {
        return s.to_owned();
    }
    let mut out: String = std::iter::repeat_n(pad, length - n).collect();
    out.push_str(s);
    out
}

/// `Zotero.Utilities.trimInternal` (utilities.js:310): collapse whitespace
/// runs to one space and trim.
pub fn trim_internal(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_space = false;
    for c in s.chars() {
        if c.is_whitespace() || c == '\u{a0}' {
            if !in_space {
                out.push(' ');
            }
            in_space = true;
        } else {
            out.push(c);
            in_space = false;
        }
    }
    js_trim(&out).to_owned()
}

/// A `strToDate` work-in-progress part: `None` text is JavaScript `undefined`.
struct Part {
    text: Option<String>,
    pos: PartPos,
}

#[derive(Clone, Copy)]
enum PartPos {
    None,
    BeforeAll,
    BeforeM,
    AfterM,
}

/// `_insertDateOrderPart` (date.js:530).
fn insert_order_part(order: &str, part: char, pos: PartPos) -> String {
    if order.is_empty() {
        return part.to_string();
    }
    match pos {
        PartPos::BeforeAll => format!("{part}{order}"),
        PartPos::BeforeM => match order.find('m') {
            None => order.to_owned(),
            Some(i) => format!("{}{part}{}", &order[..i], &order[i..]),
        },
        PartPos::AfterM => match order.find('m') {
            None => format!("{order}{part}"),
            Some(i) => format!("{}{part}{}", &order[..=i], &order[i + 1..]),
        },
        PartPos::None => format!("{order}{part}"),
    }
}

/// The year while `strToDate` runs: upstream holds a string or a number and
/// tests its truthiness.
#[derive(Clone)]
enum Year {
    Str(String),
    Num(i64),
}

impl Year {
    fn truthy(&self) -> bool {
        match self {
            Year::Str(s) => !s.is_empty(),
            Year::Num(n) => *n != 0,
        }
    }
}

/// `Zotero.Date.strToDate` (date.js:272): read year, month, day and a
/// leftover part out of free text. See the module docs for the locale and
/// `undefined` notes.
pub fn str_to_date(s: &str, opts: &DateOptions) -> StrDate {
    let string = trim_internal(s);
    let mut year: Option<Year> = None;
    let mut month: Option<i64> = None;
    let mut day: Option<i64> = None;
    let mut order = String::new();
    if string.is_empty() {
        return StrDate::default();
    }
    let mut parts: Vec<Part> = Vec::new();

    static SLASH: OnceLock<Regex> = OnceLock::new();
    let slash = re(
        &SLASH,
        r"^(.*?)(?-u:\b)([0-9]{1,4})(?:([\-/\.\x{5e74}])([0-9]{1,2}))?(?:([\-/\.\x{6708}])([0-9]{1,4}))?((?:(?-u:\b)|[^0-9]).*?)$",
    );
    let m = slash.captures(&string);
    let accepted = m.as_ref().is_some_and(|m| {
        let g = |i: usize| m.get(i).map(|x| x.as_str()).filter(|s| !s.is_empty());
        let (m3, m5) = (g(3), g(5));
        let sane_sep = m5.is_none()
            || m3.is_none()
            || m3 == m5
            || (m3 == Some("\u{5e74}") && m5 == Some("\u{6708}"));
        let complete = (g(2).is_some() && g(4).is_some() && g(6).is_some())
            || (g(1).is_none() && g(7).is_none());
        sane_sep && complete
    });
    if let (true, Some(m)) = (accepted, m.as_ref()) {
        let g = |i: usize| m.get(i).map(|x| x.as_str().to_owned());
        let (m2, m3, m4, m6) = (g(2).unwrap_or_default(), g(3), g(4), g(6));
        let truthy = |s: &Option<String>| s.as_deref().is_some_and(|s| !s.is_empty());
        let (y_s, mo_s, d_s): (Option<String>, Option<String>, Option<String>);
        if m2.len() == 3 || m2.len() == 4 || m3.as_deref() == Some("\u{5e74}") {
            y_s = Some(m2.clone());
            mo_s = m4.clone();
            d_s = m6.clone();
            if !m2.is_empty() {
                order.push('y');
            }
            if truthy(&m4) {
                order.push('m');
            }
            if truthy(&m6) {
                order.push('d');
            }
        } else if !m2.is_empty() && !truthy(&m4) && truthy(&m6) {
            mo_s = Some(m2.clone());
            y_s = m6.clone();
            d_s = None;
            order.push('m');
            order.push('y');
        } else {
            if opts.month_first {
                mo_s = Some(m2.clone());
                d_s = m4.clone();
                if !m2.is_empty() {
                    order.push('m');
                }
                if truthy(&m4) {
                    order.push('d');
                }
            } else {
                mo_s = m4.clone();
                d_s = Some(m2.clone());
                if !m2.is_empty() {
                    order.push('d');
                }
                if truthy(&m4) {
                    order.push('m');
                }
            }
            y_s = m6.clone();
            if m6.is_some() {
                order.push('y');
            }
        }
        let long_year = y_s
            .as_deref()
            .is_some_and(|y| !y.is_empty() && y.chars().count() > 2);
        year = y_s.map(|y| {
            if y.is_empty() {
                Year::Str(y)
            } else {
                Year::Num(y.parse().unwrap_or(0))
            }
        });
        day = d_s
            .filter(|d| !d.is_empty())
            .map(|d| d.parse().unwrap_or(0));
        month = mo_s
            .filter(|d| !d.is_empty())
            .map(|d| d.parse().unwrap_or(0));
        if month.is_some_and(|mo| mo > 12) {
            std::mem::swap(&mut month, &mut day);
            order = order
                .replacen('m', "D", 1)
                .replacen('d', "M", 1)
                .replacen('D', "d", 1)
                .replacen('M', "m", 1);
        }
        let month_ok = month.is_none_or(|mo| mo == 0 || mo <= 12);
        let day_ok = day.is_none_or(|d| d == 0 || d <= 31);
        if month_ok && day_ok {
            if let Some(Year::Num(y)) = year {
                if y != 0 && y < 100 && !long_year {
                    let two_digit = opts.current_year.rem_euclid(100);
                    let century = opts.current_year - two_digit;
                    year = Some(Year::Num(if y <= two_digit {
                        century + y
                    } else {
                        century - 100 + y
                    }));
                }
            }
            // `if (date.month) date.month--; else delete date.month;`
            month = month.filter(|mo| *mo != 0).map(|mo| mo - 1);
            parts.push(Part {
                text: g(1),
                pos: PartPos::BeforeAll,
            });
            parts.push(Part {
                text: g(7),
                pos: PartPos::None,
            });
        } else {
            year = None;
            month = None;
            day = None;
            order.clear();
            parts.push(Part {
                text: Some(string.clone()),
                pos: PartPos::None,
            });
        }
    } else {
        parts.push(Part {
            text: Some(string.clone()),
            pos: PartPos::None,
        });
    }

    // JavaScript coerces `undefined` to the string "undefined" in exec() and
    // in concatenation; reproduce that (see module docs).
    let js_text = |p: &Part| p.text.clone().unwrap_or_else(|| "undefined".to_owned());

    // YEAR
    if !year.as_ref().is_some_and(Year::truthy) {
        static YEAR: OnceLock<Regex> = OnceLock::new();
        let year_re = re(
            &YEAR,
            r"(?i)^(.*?)(?-u:\b)((?:circa |around |about |c\.? ?)?[0-9]{1,4}(?: ?B\.? ?C\.?(?: ?E\.?)?| ?C\.? ?E\.?| ?A\.? ?D\.?)|[0-9]{3,4})(?-u:\b)(.*?)$",
        );
        for i in 0..parts.len() {
            let text = js_text(&parts[i]);
            if let Some(m) = year_re.captures(&text) {
                year = Some(Year::Str(m[2].to_owned()));
                order = insert_order_part(&order, 'y', parts[i].pos);
                let before = Part {
                    text: Some(m[1].to_owned()),
                    pos: PartPos::BeforeAll,
                };
                let after = Part {
                    text: Some(m.get(3).map(|x| x.as_str()).unwrap_or("").to_owned()),
                    pos: PartPos::None,
                };
                parts.splice(i..=i, [before, after]);
                break;
            }
        }
    }

    // MONTH
    if month.is_none() {
        static MONTH: OnceLock<Regex> = OnceLock::new();
        let names: Vec<String> = MONTHS_SHORT
            .iter()
            .chain(MONTHS_LONG.iter())
            .map(|m| m.to_lowercase())
            .collect();
        let month_re = re(
            &MONTH,
            &format!(
                r"(?i)(.*)(?:^|[^\p{{L}}])({})[^ ]*(?: (.*)$|$)",
                names.join("|")
            ),
        );
        for i in 0..parts.len() {
            let text = js_text(&parts[i]);
            if let Some(m) = month_re.captures(&text) {
                let idx = names
                    .iter()
                    .position(|n| *n == m[2].to_lowercase())
                    .unwrap_or(0);
                month = Some((idx % 12) as i64);
                order = insert_order_part(&order, 'm', parts[i].pos);
                let before = Part {
                    text: Some(m[1].to_owned()),
                    pos: PartPos::BeforeM,
                };
                // m[3] is undefined when the month ends the string.
                let after = Part {
                    text: m.get(3).map(|x| x.as_str().to_owned()),
                    pos: PartPos::AfterM,
                };
                parts.splice(i..=i, [before, after]);
                break;
            }
        }
    }

    // DAY
    if !day.is_some_and(|d| d != 0) {
        let suffixes = opts
            .day_suffixes
            .iter()
            .map(|s| regex::escape(s))
            .collect::<Vec<_>>()
            .join("|");
        let day_re = Regex::new(&format!(
            r"(?i)(?-u:\b)([0-9]{{1,2}})(?:{suffixes})?(?-u:\b)(.*)"
        ))
        .expect("day regex compiles");
        for i in 0..parts.len() {
            let text = js_text(&parts[i]);
            if let Some(m) = day_re.captures(&text) {
                let d: i64 = m[1].parse().unwrap_or(99);
                if d <= 31 {
                    day = Some(d);
                    order = insert_order_part(&order, 'd', parts[i].pos);
                    let start = m.get(0).map(|x| x.start()).unwrap_or(0);
                    let rest = m.get(2).map(|x| x.as_str()).unwrap_or("");
                    let part = if start > 0 {
                        let mut p = text[..start].to_owned();
                        if !rest.is_empty() {
                            p.push(' ');
                            p.push_str(rest);
                        }
                        p
                    } else {
                        rest.to_owned()
                    };
                    parts.splice(
                        i..=i,
                        [Part {
                            text: Some(part),
                            pos: PartPos::None,
                        }],
                    );
                    break;
                }
            }
        }
    }

    // Concatenate and clean the leftover part.
    let mut part = String::new();
    for p in &parts {
        part.push_str(&js_text(p));
        part.push(' ');
    }
    let part = part
        .trim_start_matches(|c: char| !c.is_ascii_alphanumeric())
        .trim_end_matches(|c: char| !c.is_ascii_alphanumeric())
        .to_owned();

    StrDate {
        year: year.and_then(|y| match y {
            Year::Str(s) if s.is_empty() => None,
            Year::Str(s) => Some(s),
            Year::Num(n) => Some(n.to_string()),
        }),
        month: month.map(|m| m as u32),
        day: day.filter(|d| *d != 0).map(|d| d as u32),
        part: (!part.is_empty()).then_some(part),
        order,
    }
}

/// `Zotero.Date.formatDate` without `shortFormat` (date.js:567): e.g.
/// "Summer December 31, 1999". The month is 0-indexed.
pub fn format_date(
    part: Option<&str>,
    year: Option<&str>,
    month: Option<u32>,
    day: Option<u32>,
) -> String {
    let mut s = String::new();
    if let Some(p) = part.filter(|p| !p.is_empty()) {
        s.push_str(p);
        s.push(' ');
    }
    if let Some(name) = month.and_then(|m| MONTHS_LONG.get(m as usize)) {
        s.push_str(name);
        match day.filter(|d| *d != 0) {
            Some(d) => s.push_str(&format!(" {d}, ")),
            None => s.push(' '),
        }
    }
    // Upstream `if(date.year)` (date.js:592): any non-empty string is
    // truthy in JavaScript, "0" included. (~~`&& *y != "0"`~~ CORRECTED
    // 2026-10-07: that dropped year 0, unlike upstream; found by the
    // translator port running upstream's date.js, #749.)
    if let Some(y) = year.filter(|y| !y.is_empty()) {
        s.push_str(y);
    }
    s
}

/// `Zotero.Date.strToISO` (date.js:600): `YYYY[-MM[-DD]]`, or `None`.
pub fn str_to_iso(s: &str, opts: &DateOptions) -> Option<String> {
    let d = str_to_date(s, opts);
    // Upstream `if(date.year)` (date.js:603): any non-empty string is truthy,
    // so `strToISO("0000")` is "0000" (run upstream, 2026-10-07, #749).
    // ~~`filter(|y| y != "0")`~~ CORRECTED 2026-10-07.
    let year = d.year.filter(|y| !y.is_empty())?;
    let mut out = lpad(&year, '0', 4);
    if let Some(m) = d.month {
        out.push('-');
        out.push_str(&lpad(&(m + 1).to_string(), '0', 2));
        if let Some(day) = d.day {
            out.push('-');
            out.push_str(&lpad(&day.to_string(), '0', 2));
        }
    }
    Some(out)
}

fn sql_date_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(
        &R,
        r"^-?[0-9]{4}-(0[1-9]|10|11|12)-(0[1-9]|[1-2][0-9]|30|31)$",
    )
}

/// `Zotero.Date.isSQLDate` (date.js:1019), `YYYY-MM-DD`.
pub fn is_sql_date(s: &str) -> bool {
    sql_date_re().is_match(s)
}

/// `Zotero.Date.isSQLDateTime` (date.js:1027), `YYYY-MM-DD hh:mm:ss`.
pub fn is_sql_date_time(s: &str) -> bool {
    static R: OnceLock<Regex> = OnceLock::new();
    re(
        &R,
        r"^-?[0-9]{4}-(0[1-9]|10|11|12)-(0[1-9]|[1-2][0-9]|30|31) ([0-1][0-9]|[2][0-3]):([0-5][0-9]):([0-5][0-9])$",
    )
    .is_match(s)
}

/// `Zotero.Date.isSQLDateTimeWithoutSeconds` (date.js:1032).
pub fn is_sql_date_time_without_seconds(s: &str) -> bool {
    static R: OnceLock<Regex> = OnceLock::new();
    re(
        &R,
        r"^-?[0-9]{4}-(0[1-9]|10|11|12)-(0[1-9]|[1-2][0-9]|30|31) ([0-1][0-9]|[2][0-3]):([0-5][0-9])$",
    )
    .is_match(s)
}

fn iso8601_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(
        &R,
        r"^([0-9]{4})(-([0-9]{2})(-([0-9]{2})(T([0-9]{2}):([0-9]{2})(:([0-9]{2})(\.([0-9]+))?)?(Z|(([-+])([0-9]{2}):([0-9]{2})))?)?)?)?$",
    )
}

/// `Zotero.Date.isISODate` (date.js:235).
pub fn is_iso_date(s: &str) -> bool {
    iso8601_re().is_match(s)
}

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's
/// `days_from_civil`).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The inverse of [`days_from_civil`]: `(year, month 1-12, day)`.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Seconds since the epoch -> `YYYY-MM-DD hh:mm:ss` (`dateToSQL`, date.js:167).
fn secs_to_sql(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{}-{:02}-{:02} {:02}:{:02}:{:02}",
        lpad(&y.to_string(), '0', 4),
        m,
        d,
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// `Zotero.Date.sqlToDate(sql, isUTC=true)` as seconds since the epoch, for
/// `YYYY-MM-DD`, `YYYY-MM-DD hh:mm` and `YYYY-MM-DD hh:mm:ss` (date.js:121).
pub fn sql_to_epoch_secs(sql: &str) -> Option<i64> {
    if !is_sql_date(sql) && !is_sql_date_time(sql) && !is_sql_date_time_without_seconds(sql) {
        return None;
    }
    let (date, time) = match sql.split_once(' ') {
        Some((d, t)) => (d, Some(t)),
        None => (sql, None),
    };
    let neg = date.starts_with('-');
    let mut it = date.trim_start_matches('-').split('-');
    let y: i64 = it.next()?.parse().ok()?;
    let y = if neg { -y } else { y };
    let mo: i64 = it.next()?.parse().ok()?;
    let d: i64 = it.next()?.parse().ok()?;
    let (mut hh, mut mm, mut ss) = (0, 0, 0);
    if let Some(t) = time {
        let mut tp = t.split(':');
        hh = tp.next()?.parse().ok()?;
        mm = tp.next()?.parse().ok()?;
        ss = tp.next().map(|s| s.parse().unwrap_or(0)).unwrap_or(0);
    }
    Some(days_from_civil(y, mo, d) * 86_400 + hh * 3600 + mm * 60 + ss)
}

/// `Zotero.Date.isoToDate` then `dateToSQL(d, true)` (date.js:245, 167):
/// an ISO 8601 date/time as a UTC SQL date-time. A date-only or
/// zone-less value is read as UTC (JavaScript reads a zone-less *date-time*
/// as local time; with local = UTC, as here, the two agree).
pub fn iso_to_sql(iso: &str) -> Option<String> {
    let c = iso8601_re().captures(iso)?;
    let num = |i: usize| c.get(i).map(|m| m.as_str().parse::<i64>().unwrap_or(0));
    let y = num(1)?;
    let mo = num(3).unwrap_or(1);
    let d = num(5).unwrap_or(1);
    let hh = num(7).unwrap_or(0);
    let mi = num(8).unwrap_or(0);
    let ss = num(10).unwrap_or(0);
    if !(1..=12).contains(&mo)
        || d < 1
        || d > days_in_month(y, mo as u32) as i64
        || hh > 24
        || mi > 59
        || ss > 59
    {
        return None;
    }
    let mut secs = days_from_civil(y, mo, d) * 86_400 + hh * 3600 + mi * 60 + ss;
    if c.get(14).is_some() {
        let sign = if &c[15] == "-" { -1 } else { 1 };
        let off = num(16).unwrap_or(0) * 3600 + num(17).unwrap_or(0) * 60;
        secs -= sign * off;
    }
    Some(secs_to_sql(secs))
}

/// UTC SQL date-time -> "local" SQL date-time, shifted by
/// [`DateOptions::utc_offset_minutes`] (`sqlToDate(date, true)` then
/// `dateToSQL(localDate)`, utilities_item.js:313-314). Invalid input gives
/// `""`, as upstream's `dateToSQL(false)` does.
pub fn utc_sql_to_local_sql(sql: &str, opts: &DateOptions) -> String {
    match sql_to_epoch_secs(sql) {
        Some(secs) => secs_to_sql(secs + opts.utc_offset_minutes as i64 * 60),
        None => String::new(),
    }
}

/// `Zotero.Date.sqlToISO8601` (date.js:617): drop `-00` parts, `T...Z`.
pub fn sql_to_iso8601(sql: &str) -> Option<String> {
    static R: OnceLock<Regex> = OnceLock::new();
    let r = re(&R, r"^([0-9]{4})-([0-9]{2})-([0-9]{2})");
    let head: String = sql.chars().take(10).collect();
    let m = r.captures(&head)?;
    let mut date = m[1].to_owned();
    if &m[2] != "00" {
        date.push('-');
        date.push_str(&m[2]);
        if &m[3] != "00" {
            date.push('-');
            date.push_str(&m[3]);
        }
    }
    let time: String = sql.chars().skip(11).collect();
    if !time.is_empty() {
        date.push('T');
        date.push_str(&time);
        date.push('Z');
    }
    Some(date)
}

/// `Zotero.Date.strToMultipart` (date.js:902): the sortable
/// `YYYY-MM-DD <original>` form Zotero stores for date fields.
pub fn str_to_multipart(s: &str, opts: &DateOptions) -> String {
    if s.is_empty() {
        return String::new();
    }
    let (mut year, month, day): (Option<String>, Option<u32>, Option<u32>);
    if let Some(e) = parse_edtf(s) {
        year = Some(e.begin.year.to_string());
        month = e.begin.month;
        day = e.begin.day;
    } else if has_era_notation(&circa_prefix_re().replace(&normalize_dashes(js_trim(s)), "")) {
        year = None;
        month = None;
        day = None;
    } else {
        let d = str_to_date(s, opts);
        year = d.year;
        month = d.month;
        day = d.day;
    }
    // "remove year value if not between 1 and 9999"
    if let Some(y) = &year {
        let ok = !y.is_empty() && y.len() <= 4 && y.chars().all(|c| c.is_ascii_digit());
        if !ok || y == "0" {
            year = None;
        }
    }
    let month_s = month.map(|m| (m + 1).to_string()).unwrap_or_default();
    format!(
        "{}-{}-{} {s}",
        year.map(|y| lpad(&y, '0', 4))
            .unwrap_or_else(|| "0000".into()),
        lpad(&month_s, '0', 2),
        day.map(|d| lpad(&d.to_string(), '0', 2))
            .unwrap_or_else(|| "00".into())
    )
}

/// `Zotero.Date.multipartToSQL` (date.js:966): the `YYYY-MM-DD` head of a
/// multipart date, `0000-00-00` for a non-multipart string.
pub fn multipart_to_sql(multi: &str) -> String {
    if multi.is_empty() {
        return String::new();
    }
    static R: OnceLock<Regex> = OnceLock::new();
    let r = re(
        &R,
        r"^[0-9]{4}-(0[0-9]|10|11|12)-(0[0-9]|[1-2][0-9]|30|31) ",
    );
    if is_sql_date_time(multi) || is_sql_date_time_without_seconds(multi) || !r.is_match(multi) {
        return "0000-00-00".into();
    }
    multi.chars().take(10).collect()
}

/// `Zotero.Date.isMultipart` (date.js:954): whether `s` is a multipart date
/// (`YYYY-MM-DD <original>`), which an SQL date-time is not. Added
/// 2026-10-07 for the Zotero database reader (GitHub #750).
pub fn is_multipart(s: &str) -> bool {
    if is_sql_date_time(s) || is_sql_date_time_without_seconds(s) {
        return false;
    }
    // `_multipartRE` (date.js:944).
    static R: OnceLock<Regex> = OnceLock::new();
    re(
        &R,
        r"^[0-9]{4}-(0[0-9]|10|11|12)-(0[0-9]|[1-2][0-9]|30|31) ",
    )
    .is_match(s)
}

/// `Zotero.Date.multipartToStr` (date.js:983): the user part of a multipart
/// date (`2006-11-03 November 3rd, 2006` -> `November 3rd, 2006`); any other
/// string unchanged. This is how `Item#getField` turns a stored date field
/// back into what the user typed (item.js:303). Added 2026-10-07 (#750).
pub fn multipart_to_str(multi: &str) -> String {
    if multi.is_empty() {
        return String::new();
    }
    if !is_multipart(multi) {
        return multi.to_owned();
    }
    // `multi.substr(11)`: the first 11 characters matched an ASCII regex, so
    // the byte offset is the character offset.
    multi[11..].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `isMultipart`/`multipartToStr` on the examples of date.js's own doc
    /// comments (:963, :981) and the SQL date-time exclusion (:955).
    #[test]
    fn multipart_round_trip_and_sql_exclusion() {
        assert!(is_multipart("2006-11-03 November 3rd, 2006"));
        assert_eq!(
            multipart_to_str("2006-11-03 November 3rd, 2006"),
            "November 3rd, 2006"
        );
        assert!(!is_multipart("2006-11-03 12:34:56"));
        assert_eq!(
            multipart_to_str("2006-11-03 12:34:56"),
            "2006-11-03 12:34:56"
        );
        assert_eq!(multipart_to_str("November 2006"), "November 2006");
        assert_eq!(multipart_to_str(""), "");
        let o = DateOptions::default();
        for s in ["1999-12-31", "March 2010", "ca. 1850", "2004-06~"] {
            assert_eq!(multipart_to_str(&str_to_multipart(s, &o)), s, "{s}");
        }
    }

    fn opts() -> DateOptions {
        DateOptions {
            current_year: 2026,
            ..DateOptions::default()
        }
    }

    fn p(year: i64, month: Option<u32>, day: Option<u32>) -> EdtfParts {
        EdtfParts { year, month, day }
    }

    /// Every case of utilities test/tests/dateTest.js:2-133 ("parseEDTF").
    #[test]
    fn parse_edtf_upstream_cases() {
        let o = parse_edtf("2004-06~").unwrap();
        assert_eq!(o.begin, p(2004, Some(5), None));
        assert!(o.circa);
        let o = parse_edtf("2021/2026").unwrap();
        assert_eq!(
            (o.begin, o.end, o.circa),
            (p(2021, None, None), Some(p(2026, None, None)), false)
        );
        let o = parse_edtf("-429?").unwrap();
        assert_eq!(o.begin, p(-429, None, None));
        assert!(o.circa);
        for (s, b, e) in [
            ("1995-1996", 1995, Some(1996)),
            ("2021-22", 2021, Some(2022)),
            ("1995\u{2013}1996", 1995, Some(1996)),
            ("1995--96", 1995, Some(1996)),
        ] {
            let o = parse_edtf(s).unwrap();
            assert_eq!(o.begin, p(b, None, None), "{s}");
            assert_eq!(o.end, e.map(|e| p(e, None, None)), "{s}");
        }
        let o = parse_edtf("1995-05").unwrap();
        assert_eq!((o.begin, o.end), (p(1995, Some(4), None), None));
        for s in ["~1995", "ca. 1995"] {
            let o = parse_edtf(s).unwrap();
            assert_eq!(o.begin, p(1995, None, None));
            assert!(o.circa, "{s}");
        }
        let o = parse_edtf("429 BCE").unwrap();
        assert_eq!((o.begin, o.circa), (p(-429, None, None), false));
        let o = parse_edtf("~429 BCE").unwrap();
        assert_eq!((o.begin, o.circa), (p(-429, None, None), true));
        assert_eq!(parse_edtf("429 CE").unwrap().begin, p(429, None, None));
        assert_eq!(parse_edtf("AD 429").unwrap().begin, p(429, None, None));
        for (s, b, e) in [
            ("1000 BCE-900 BCE", -1000, -900),
            ("1000\u{2013}900 BCE", -1000, -900),
            ("AD 429-500", 429, 500),
            ("100 BCE-50 CE", -100, 50),
        ] {
            let o = parse_edtf(s).unwrap();
            assert_eq!(
                (o.begin, o.end),
                (p(b, None, None), Some(p(e, None, None))),
                "{s}"
            );
        }
        let o = parse_edtf("ca. 480 B.C. - 323 B.C.").unwrap();
        assert_eq!(
            (o.begin, o.end, o.circa),
            (p(-480, None, None), Some(p(-323, None, None)), true)
        );
        assert_eq!(
            parse_edtf("2024-02-29").unwrap().begin,
            p(2024, Some(1), Some(29))
        );
        for s in [
            "2023-02-29",
            "2021-02-30",
            "2021-13",
            "429",
            "196X",
            "2021/..",
            "1996-1995",
            "2021-21",
            "2026/2021",
            "2021/2021",
            "2021-05/2021-04",
            "900-1000 BCE",
            "50 CE-100 BCE",
            "AD 429 BCE",
            "0 CE",
            "AD 0",
            "May 13, 2021",
        ] {
            assert_eq!(parse_edtf(s), None, "{s} should be rejected");
        }
    }

    /// utilities test/tests/dateTest.js:135-165 ("strToMultipart").
    #[test]
    fn str_to_multipart_upstream_cases() {
        let o = opts();
        for (s, want) in [
            ("2021-05-13", "2021-05-13 2021-05-13"),
            ("2021/2026", "2021-00-00 2021/2026"),
            ("2021-05/2021-06", "2021-05-00 2021-05/2021-06"),
            ("2004-06~", "2004-06-00 2004-06~"),
            ("ca. 1900", "1900-00-00 ca. 1900"),
            ("429 CE", "0429-00-00 429 CE"),
            ("-500/-750", "0000-00-00 -500/-750"),
            ("500-750 BCE", "0000-00-00 500-750 BCE"),
            ("1996-1995", "1996-00-00 1996-1995"),
            ("-0429", "0000-00-00 -0429"),
            ("429 BCE", "0000-00-00 429 BCE"),
        ] {
            assert_eq!(str_to_multipart(s, &o), want, "{s}");
        }
    }

    /// zotero test/tests/dateTest.js:166-283 ("#strToDate()"), en-US cases;
    /// the zh-CN month case is out of scope (en-US months only).
    #[test]
    fn str_to_date_upstream_cases() {
        let o = opts();
        for s in ["", " "] {
            assert_eq!(str_to_date(s, &o).year, None);
        }
        for s in ["June 26, 2010", "26 June 2010"] {
            let d = str_to_date(s, &o);
            assert_eq!(
                (d.month, d.day, d.year.as_deref()),
                (Some(5), Some(26), Some("2010")),
                "{s}"
            );
        }
        for (s, y) in [
            ("001", "1"),
            ("0001", "1"),
            ("012", "12"),
            ("0012", "12"),
            ("0123", "123"),
            ("01/01/08", "2008"),
            ("1/1/68", "1968"),
            ("1/1/19", "2019"),
        ] {
            assert_eq!(str_to_date(s, &o).year.as_deref(), Some(y), "{s}");
        }
        for s in ["8/2020", "08/2020"] {
            let d = str_to_date(s, &o);
            assert_eq!(
                (d.month, d.day, d.year.as_deref()),
                (Some(7), None, Some("2020")),
                "{s}"
            );
        }
        let d = str_to_date("1", &o);
        assert_eq!((d.month, d.year, d.order.as_str()), (Some(0), None, "m"));
        let d = str_to_date("25", &o);
        assert_eq!(
            (d.day, d.month, d.year, d.order.as_str()),
            (Some(25), None, None, "d")
        );
    }

    #[test]
    fn str_to_date_keeps_era_and_season() {
        let o = opts();
        let d = str_to_date("January 10, 200 BCE", &o);
        assert_eq!(
            (d.year.as_deref(), d.month, d.day),
            (Some("200 BCE"), Some(0), Some(10))
        );
        assert_eq!(parse_era_year(d.year.as_deref().unwrap()), Some(-200));
        let d = str_to_date("Summer 429 BCE", &o);
        assert_eq!(
            (d.year.as_deref(), d.month, d.part.as_deref()),
            (Some("429 BCE"), None, Some("Summer"))
        );
        let d = str_to_date("1999-12-31", &o);
        assert_eq!(
            (d.year.as_deref(), d.month, d.day),
            (Some("1999"), Some(11), Some(31))
        );
        // The `undefined` concatenation upstream performs (module docs).
        // Confirmed by running upstream (2026-10-07, #752): RIS export of an
        // item dated "2021 May" (RIS.js writes `year/month/day/part` for DA)
        // from a Zotero translation-server (utilities 1dd38e27edf8, whose
        // strToDate matches 4051881d59c6 here; re-run with 4051881d59c6
        // itself the same day, same result) gives `DA  - 2021/05//undefined`
        // (kovan-literature tests/data/zotero/reference/export/ris/
        // kovanProbes.json, item KPROBE22).
        assert_eq!(
            str_to_date("2021 May", &o).part.as_deref(),
            Some("undefined")
        );
    }

    #[test]
    fn era_year_and_looks_like_edtf() {
        assert_eq!(parse_era_year("200 BCE"), Some(-200));
        assert_eq!(parse_era_year("AD 200"), Some(200));
        assert_eq!(parse_era_year("200"), None);
        assert!(looks_like_edtf("-500/-750"));
        assert!(looks_like_edtf("500-750 BCE"));
        assert!(looks_like_edtf("1996-1995"));
        assert!(!looks_like_edtf("May 13, 2021"));
        assert!(!looks_like_edtf("January 10, 200 BCE"));
    }

    #[test]
    fn iso_sql_conversions() {
        assert_eq!(
            iso_to_sql("1997-06-13T23:59:58Z").as_deref(),
            Some("1997-06-13 23:59:58")
        );
        assert_eq!(
            iso_to_sql("1997-06-13T23:59:58+01:00").as_deref(),
            Some("1997-06-13 22:59:58")
        );
        assert_eq!(iso_to_sql("2008").as_deref(), Some("2008-01-01 00:00:00"));
        assert_eq!(iso_to_sql("nope"), None);
        assert_eq!(
            sql_to_iso8601("2015-04-12 09:00:22").as_deref(),
            Some("2015-04-12T09:00:22Z")
        );
        assert_eq!(sql_to_iso8601("2015-00-00").as_deref(), Some("2015"));
        let sgt = DateOptions {
            utc_offset_minutes: 480,
            ..opts()
        };
        assert_eq!(
            utc_sql_to_local_sql("2019-01-08 16:00:00", &sgt),
            "2019-01-09 00:00:00"
        );
        assert_eq!(utc_sql_to_local_sql("garbage", &sgt), "");
        assert!(is_sql_date("2001-02-03"));
        assert!(!is_sql_date("2001-02-03 12:13:14"));
        assert!(is_sql_date_time("2001-02-03 12:13:14"));
        assert_eq!(
            str_to_iso("May 13, 2021", &opts()).as_deref(),
            Some("2021-05-13")
        );
        assert_eq!(multipart_to_sql("2021-05-13 May 13, 2021"), "2021-05-13");
    }

    #[test]
    fn civil_round_trip() {
        for days in [-800_000i64, -1, 0, 1, 10_957, 20_000, 2_000_000] {
            let (y, m, d) = civil_from_days(days);
            assert_eq!(days_from_civil(y, m, d), days);
        }
    }

    #[test]
    fn format_date_long_form() {
        assert_eq!(
            format_date(None, Some("1999"), Some(11), Some(31)),
            "December 31, 1999"
        );
        assert_eq!(
            format_date(Some("Spring"), Some("2021"), None, None),
            "Spring 2021"
        );
        assert_eq!(
            format_date(None, Some("1999"), Some(0), None),
            "January 1999"
        );
    }

    /// Year 0 is a year (upstream `if(date.year)` is true for the string
    /// "0"). Expected value from running upstream's date.js on "0000"
    /// (2026-10-07, #749): `strToISO("0000") === "0000"`. Before the fix the
    /// port returned `None`.
    #[test]
    fn year_zero_is_kept_as_upstream_does() {
        let o = DateOptions::default();
        assert_eq!(str_to_iso("0000", &o).as_deref(), Some("0000"));
        assert_eq!(format_date(None, Some("0"), None, None), "0");
    }
}
