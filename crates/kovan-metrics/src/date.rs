//! Calendar dates, without a date crate.
//!
//! This crate needs exactly three things from a calendar: parse the workspace's
//! `DDMMYY` window notation, format a date back out, and know what today is for
//! the default report window. That is not enough to justify pulling `chrono` /
//! `time` / `jiff` into `[workspace.dependencies]` — none of which is currently
//! there — so the civil-date conversion is done here with Howard Hinnant's
//! `days_from_civil` / `civil_from_days` algorithms (public domain, from
//! <http://howardhinnant.github.io/date_algorithms.html>).
//!
//! **Timezone caveat.** [`today`] is **UTC**, because `std` exposes no local
//! timezone. The Python it replaces used `datetime.date.today()`, which is
//! *local*. The two differ only for the hours either side of midnight, and only
//! for the default `--to` bound and the generated filename tag — never for the
//! token figures themselves, which come from commit trailers. Pass `--to`
//! explicitly if the boundary matters.

use std::time::{SystemTime, UNIX_EPOCH};

/// A proleptic-Gregorian calendar date: year, month (1-12), day (1-31).
///
/// Ordering is chronological, so `from <= commit_date <= to` window tests are
/// just comparisons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    /// Full year, e.g. `2026`.
    pub year: i32,
    /// Month of year, `1..=12`.
    pub month: u32,
    /// Day of month, `1..=31`.
    pub day: u32,
}

/// Why a `DDMMYY` string could not be read as a date.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DateError {
    /// The input was not exactly six ASCII digits.
    #[error("expected DDMMYY (6 digits), got {0:?}")]
    NotSixDigits(String),
    /// The digits parsed but do not name a real calendar day (month 13, 31
    /// February, 29 February in a common year, …).
    #[error("{0:02}/{1:02}/{2} is not a real calendar date")]
    NotACalendarDate(u32, u32, i32),
}

/// Days in `month` of `year`, honouring the Gregorian leap rule.
fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

impl Date {
    /// Build a date, rejecting anything that is not a real calendar day.
    pub fn new(year: i32, month: u32, day: u32) -> Result<Self, DateError> {
        if month < 1 || month > 12 || day < 1 || day > days_in_month(year, month) {
            return Err(DateError::NotACalendarDate(day, month, year));
        }
        Ok(Self { year, month, day })
    }

    /// Parse the workspace's `DDMMYY` window notation — day, month, then a
    /// **2-digit year taken as `2000 + yy`**, matching the Python it replaces.
    ///
    /// ```
    /// # use kovan_metrics::date::Date;
    /// let d = Date::parse_ddmmyy("130826").unwrap();
    /// assert_eq!((d.year, d.month, d.day), (2026, 8, 13));
    /// ```
    pub fn parse_ddmmyy(s: &str) -> Result<Self, DateError> {
        let s = s.trim();
        if s.len() != 6 || !s.bytes().all(|b| b.is_ascii_digit()) {
            return Err(DateError::NotSixDigits(s.to_string()));
        }
        let num = |a: usize, b: usize| s[a..b].parse::<u32>().unwrap_or(0);
        Date::new(2000 + num(4, 6) as i32, num(2, 4), num(0, 2))
    }

    /// `YYYY-MM-DD`, the form git accepts for `--since` / `--until`.
    pub fn iso(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    /// `DDMMYY`, the form used in generated report filenames.
    pub fn ddmmyy(&self) -> String {
        format!(
            "{:02}{:02}{:02}",
            self.day,
            self.month,
            (self.year - 2000).rem_euclid(100)
        )
    }

    /// `13 Aug 2026` — the human-facing form used in report headings.
    pub fn human(&self) -> String {
        const MONTHS: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        let name = MONTHS
            .get((self.month as usize).saturating_sub(1))
            .copied()
            .unwrap_or("???");
        format!("{:02} {} {}", self.day, name, self.year)
    }

    /// Days since the Unix epoch (1970-01-01), negative before it. Hinnant's
    /// `days_from_civil`.
    pub fn to_epoch_days(self) -> i64 {
        let y = if self.month <= 2 {
            self.year - 1
        } else {
            self.year
        } as i64;
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = y - era * 400;
        let m = self.month as i64;
        let d = self.day as i64;
        let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    /// Inverse of [`to_epoch_days`](Date::to_epoch_days). Hinnant's
    /// `civil_from_days`.
    pub fn from_epoch_days(days: i64) -> Self {
        let z = days + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = mp + if mp < 10 { 3 } else { -9 };
        Self {
            year: (y + i64::from(m <= 2)) as i32,
            month: m as u32,
            day: d as u32,
        }
    }

    /// Today's date **in UTC** — see the module note on the timezone caveat.
    /// Falls back to the epoch if the system clock predates 1970.
    pub fn today() -> Self {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        Self::from_epoch_days(secs.div_euclid(86_400))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_workspace_ddmmyy_notation() {
        let d = Date::parse_ddmmyy("130826").unwrap();
        assert_eq!((d.year, d.month, d.day), (2026, 8, 13));
        assert_eq!(d.iso(), "2026-08-13");
        assert_eq!(d.ddmmyy(), "130826");
        assert_eq!(d.human(), "13 Aug 2026");
    }

    #[test]
    fn rejects_non_six_digit_input() {
        for bad in ["", "1308", "13082026", "13/08/26", "abcdef"] {
            assert!(matches!(
                Date::parse_ddmmyy(bad),
                Err(DateError::NotSixDigits(_))
            ));
        }
    }

    #[test]
    fn rejects_impossible_calendar_days() {
        assert!(Date::parse_ddmmyy("320826").is_err()); // 32 August
        assert!(Date::parse_ddmmyy("011326").is_err()); // month 13
        assert!(Date::parse_ddmmyy("000826").is_err()); // day 0
    }

    #[test]
    fn honours_the_gregorian_leap_rule() {
        // 2024 is a leap year, 2026 is not.
        assert!(Date::parse_ddmmyy("290224").is_ok());
        assert!(Date::parse_ddmmyy("290226").is_err());
        // 2000 was a leap year (divisible by 400); 1900 was not — the century
        // rule the naive `% 4` check gets wrong.
        assert!(Date::new(2000, 2, 29).is_ok());
        assert!(Date::new(1900, 2, 29).is_err());
    }

    #[test]
    fn epoch_day_conversion_round_trips() {
        for (y, m, d) in [
            (1970, 1, 1),
            (2000, 2, 29),
            (2026, 8, 13),
            (2026, 12, 31),
            (1999, 12, 31),
        ] {
            let date = Date::new(y, m, d).unwrap();
            assert_eq!(Date::from_epoch_days(date.to_epoch_days()), date);
        }
        assert_eq!(Date::new(1970, 1, 1).unwrap().to_epoch_days(), 0);
    }

    #[test]
    fn dates_order_chronologically() {
        let a = Date::new(2026, 8, 13).unwrap();
        let b = Date::new(2026, 9, 1).unwrap();
        let c = Date::new(2027, 1, 1).unwrap();
        assert!(a < b && b < c);
    }
}
