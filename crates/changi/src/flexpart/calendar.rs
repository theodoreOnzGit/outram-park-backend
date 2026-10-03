// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/juldate.f90, src/caldate.f90 (behaviour only — see below)
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! FLEXPART's Julian-date calendar: `juldate` (calendar → Julian date) and
//! `caldate` (Julian date → calendar).
//!
//! # What is ported and what is not
//!
//! `caldate.f90`'s header says it is *"adapted from Numerical Recipes"*, and
//! `juldate.f90` uses Numerical Recipes' `julday` constants (`igreg =
//! 15+31*(10+12*1582)`). The maintainer's ruling on FLEXPART's Numerical
//! Recipes code (GitHub issue #410, 2026-10-02, for `random_mod.f90`) is that it
//! is not ported and not re-implemented clean-room. This module follows that
//! ruling for the **date arithmetic**: the integer day-number conversion uses
//! **Howard Hinnant's `days_from_civil` / `civil_from_days`**, which are public
//! domain (<http://howardhinnant.github.io/date_algorithms.html>) and already the
//! workspace's reference implementation in `crates/kovan-metrics/src/date.rs`.
//! That crate cannot be a dependency here (it is outside the wasm gate), so the
//! two short functions are repeated with the citation.
//!
//! The **time-of-day arithmetic** — hours, minutes and rounded seconds from the
//! fractional day, and the `ss == 60` / `mi == 60` roll-overs — is FLEXPART's own
//! and is ported line for line.
//!
//! # Day-number convention
//!
//! FLEXPART's integer day is the Julian Day Number with the day starting at
//! **midnight**: 1970-01-01 is day 2 440 588, and `12:00` on that day is
//! `2440588.5`. (Astronomical Julian dates start at noon; FLEXPART's do not.)
//!
//! # Valid range
//!
//! Gregorian dates from **1582-10-15** onwards. Upstream switches to the
//! Julian calendar before that date; Hinnant's algorithm is proleptic
//! Gregorian. The two agree on every date the fixture covers, which starts at
//! 1582-10-15 itself. Upstream also refuses year 0 and shifts negative years by
//! one; neither is reachable for a meteorological date and neither is ported.

/// Julian Day Number of 1970-01-01 in FLEXPART's (midnight-based) convention.
const JDN_UNIX_EPOCH: i64 = 2_440_588;

/// Days since 1970-01-01 of a proleptic-Gregorian civil date (Hinnant).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Proleptic-Gregorian civil date of a day count since 1970-01-01 (Hinnant).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// Calendar date and time → FLEXPART Julian date (`juldate.f90`).
///
/// # Arguments
/// - `yyyymmdd` — date as the integer `YYYYMMDD`, e.g. `20210615`.
/// - `hhmmss` — time of day as the integer `HHMMSS`, e.g. `81530` for 08:15:30.
///
/// # Returns
/// The Julian date in days, midnight-based (see the module docs), with the
/// time of day added as `hh/24 + mi/1440 + ss/86400` in that order, exactly as
/// upstream sums it.
#[must_use]
pub fn juldate(yyyymmdd: i64, hhmmss: i64) -> f64 {
    let yyyy = yyyymmdd / 10_000;
    let mm = (yyyymmdd - 10_000 * yyyy) / 100;
    let dd = yyyymmdd - 10_000 * yyyy - 100 * mm;
    let hh = hhmmss / 10_000;
    let mi = (hhmmss - 10_000 * hh) / 100;
    let ss = hhmmss - 10_000 * hh - 100 * mi;
    let julday = days_from_civil(yyyy, mm, dd) + JDN_UNIX_EPOCH;
    julday as f64 + hh as f64 / 24.0 + mi as f64 / 1440.0 + ss as f64 / 86_400.0
}

/// FLEXPART Julian date → calendar date and time (`caldate.f90`).
///
/// # Arguments
/// - `jul` — Julian date in days, midnight-based.
///
/// # Returns
/// `(yyyymmdd, hhmmss)` as integers.
///
/// # Rounding, ported from upstream
///
/// Hours and minutes are **truncated**, seconds are **rounded** (`nint`, half
/// away from zero). A second count that rounds up to 60 rolls into the minute,
/// and a minute count of 60 rolls into the hour. Upstream does **not** roll an
/// hour count of 24 into the next day, and neither does this port: a Julian
/// date within half a second of midnight returns `hhmmss = 240000` on the
/// earlier date.
#[must_use]
pub fn caldate(jul: f64) -> (i64, i64) {
    let julday = jul.trunc() as i64;
    let (yyyy, mm, dd) = civil_from_days(julday - JDN_UNIX_EPOCH);
    let yyyymmdd = 10_000 * yyyy + 100 * mm + dd;
    let frac = jul - julday as f64;
    let mut hh = (24.0 * frac).trunc() as i64;
    let mut mi = (1440.0 * frac - 60.0 * hh as f64).trunc() as i64;
    let mut ss = (86_400.0 * frac - 3600.0 * hh as f64 - 60.0 * mi as f64).round() as i64;
    if ss == 60 {
        ss = 0;
        mi += 1;
    }
    if mi == 60 {
        mi = 0;
        hh += 1;
    }
    (yyyymmdd, 10_000 * hh + 100 * mi + ss)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_epoch_is_jdn_2440588_at_midnight() {
        assert_eq!(juldate(19_700_101, 0), 2_440_588.0);
        assert_eq!(caldate(2_440_588.5), (19_700_101, 120_000));
    }

    #[test]
    fn round_trips_a_leap_day() {
        let j = juldate(20_000_229, 81_530);
        assert_eq!(caldate(j), (20_000_229, 81_530));
    }
}
