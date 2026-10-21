// Part of the kovan Zotero port (GitHub #747, #751).
//
// Upstream: Zotero, https://github.com/zotero/zotero (commit 9cbba8c4d281):
// chrome/content/zotero/xpcom/data/search.js:1721-1875 (the date-condition
// SQL). The SQLite date functions that SQL calls (DATE(x, 'localtime'),
// DATE(x, 'unixepoch', 'localtime'), DATE('NOW', 'localtime', '-N days'))
// are re-implemented from the SQLite documentation.
// Copyright (c) Corporation for Digital Scholarship, Vienna, Virginia, USA.
// Licence: AGPL-3.0. See this crate's NOTICE, "Upstream: Zotero".

//! Calendar arithmetic for the date conditions.
//!
//! `days_from_civil`/`civil_from_days` are Howard Hinnant's public-domain
//! algorithms; kovan-common's `zotero::date` has private copies of the same
//! two functions (not exported), so they are repeated here rather than
//! widening that crate's API.

use kovan_common::zotero::date::sql_to_epoch_secs;

use super::library::SearchClock;

/// Days since 1970-01-01 of a proleptic Gregorian date.
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The proleptic Gregorian (year, month, day) of a day number.
pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn fmt_date(days: i64) -> String {
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// `DATE(secs, 'unixepoch', 'localtime')`.
pub fn date_of_unix_local(secs: i64, clock: &SearchClock) -> String {
    fmt_date((secs + clock.utc_offset_minutes as i64 * 60).div_euclid(86_400))
}

/// `DATE(sql, 'localtime')` for a UTC SQL date or date-time; `None` (SQL
/// NULL) when the text is not one.
pub fn date_of_sql_local(sql: &str, clock: &SearchClock) -> Option<String> {
    sql_to_epoch_secs(sql).map(|s| date_of_unix_local(s, clock))
}

/// The local date `offset_days` from now (`dateToSQL(new Date(...))`, the
/// `today`/`yesterday`/`tomorrow` handling of search.js:1797-1806).
pub fn local_today(clock: &SearchClock, offset_days: i64) -> String {
    date_of_unix_local(clock.now_unix + offset_days * 86_400, clock)
}

/// `DATE('NOW', 'localtime', modifier)` with `modifier` = `-` + the
/// condition value (search.js:1870), e.g. `-10 days`, `-1 months`.
///
/// SQLite's `NNN days|hours|minutes|seconds|months|years` modifiers
/// (singular or plural, any case) are supported; any other modifier makes
/// SQLite return NULL, so `None` here.
pub fn date_now_minus(modifier: &str, clock: &SearchClock) -> Option<String> {
    let local = clock.now_unix + clock.utc_offset_minutes as i64 * 60;
    let m = modifier.trim();
    let split = m.find(|c: char| c.is_whitespace()).unwrap_or(m.len());
    let (num, unit) = (m[..split].trim(), m[split..].trim().to_ascii_lowercase());
    let n: f64 = num.parse().ok()?;
    let unit = unit.strip_suffix('s').unwrap_or(&unit).to_owned();
    let secs = match unit.as_str() {
        "day" => Some(n * 86_400.0),
        "hour" => Some(n * 3600.0),
        "minute" => Some(n * 60.0),
        "second" => Some(n),
        _ => None,
    };
    if let Some(s) = secs {
        return Some(fmt_date((local + s as i64).div_euclid(86_400)));
    }
    let days = local.div_euclid(86_400);
    let (y, mo, d) = civil_from_days(days);
    let (y2, mo2) = match unit.as_str() {
        "month" => {
            let total = y * 12 + (mo - 1) + n as i64;
            (total.div_euclid(12), total.rem_euclid(12) + 1)
        }
        "year" => (y + n as i64, mo),
        _ => return None,
    };
    // SQLite normalises an overflowing day (03-31 - 1 month = 02-31 = 03-03).
    Some(fmt_date(days_from_civil(y2, mo2, 1) + d - 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers() {
        // 2026-10-07T12:00:00Z
        let c = SearchClock::utc(days_from_civil(2026, 10, 7) * 86_400 + 43_200);
        assert_eq!(local_today(&c, 0), "2026-10-07");
        assert_eq!(local_today(&c, -1), "2026-10-06");
        assert_eq!(date_now_minus("-10 days", &c).unwrap(), "2026-09-27");
        assert_eq!(date_now_minus("-1 DAYS", &c).unwrap(), "2026-10-06");
        assert_eq!(date_now_minus("-1 months", &c).unwrap(), "2026-09-07");
        assert_eq!(date_now_minus("-2 years", &c).unwrap(), "2024-10-07");
        assert!(date_now_minus("-x", &c).is_none());
        let mar31 = SearchClock::utc(days_from_civil(2026, 3, 31) * 86_400);
        assert_eq!(date_now_minus("-1 months", &mar31).unwrap(), "2026-03-03");
    }
}
