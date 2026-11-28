// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2011-2017 OpenFOAM Foundation
// Copyright (C) 2016-2024 OpenCFD Ltd.
//
// Port of the time-name precision of OpenFOAM-ESI v2506 `Time`:
//   src/OpenFOAM/db/Time/Time.C:83          `maxPrecision_ = 3 - log10(SMALL)`
//   src/OpenFOAM/db/Time/Time.C:193-255     `setControls`: the start directory's name decides a larger precision ("Increasing the timePrecision")
//   src/OpenFOAM/db/Time/Time.C:1058-1068   `operator++`: `oldTimeValue`, `oldTimeName`, `setTime(value() + deltaT_, timeIndex_ + 1)`
//   src/OpenFOAM/db/Time/Time.C:1189-1225   `operator++`: at a write time, `precision_++` until the name reads back as the old value plus deltaT
//   src/OpenFOAM/db/Time/TimeIO.C           `readDict`: `timePrecision` (default 6)
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full notice.

//! # The name of the time directory, as `Time` builds it (gh:#1938, README round 84)
//!
//! `Time::timeName()` is `%g` of the time at `precision_` digits (`timeFormat general`). `precision_` starts at `controlDict`'s
//! `timePrecision` (default 6) and GROWS during a run: at every write time `Time::operator++` reads the new name back and, while it differs from
//! `oldTimeValue + deltaT` by more than `max(min(10^-precision, 0.1 deltaT), SMALL)`, adds a digit (up to `maxPrecision_ = 18`); the real binary
//! prints "Increased the timePrecision from 8 to 9 to distinguish between timeNames at time 100.000001" (2D_fullCoupling, `writeControl timeStep`,
//! README round 84). The precision is never lowered again. [`TimeName`] carries it from step to step.
//!
//! `no_std` + `alloc`; no file I/O.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use crate::io::foam_ascii::{time_name, MAX_TIME_PRECISION};

/// OpenFOAM `SMALL` (double precision), the floor of the name tolerance.
const SMALL: f64 = 1e-15;

/// `Time::precision_` and the name it gives the current time.
#[derive(Debug, Clone, PartialEq)]
pub struct TimeName {
    precision: usize,
    name: String,
}

/// `readScalar(name, value)`: the name read back as a number (`None`: not a number).
fn read_back(name: &str) -> Option<f64> {
    match name.parse::<f64>() {
        Ok(v) => Some(v),
        Err(_) => None,
    }
}

impl TimeName {
    /// `precision_ = timePrecision` and the name of `value` at it (`setTime(startTime_, 0)`).
    #[must_use]
    pub fn new(value: f64, precision: usize) -> Self {
        Self { precision, name: time_name(value, precision) }
    }

    /// `Time::setControls` (`Time.C:193-255`) for a start directory named `dir_name`: when no directory carries the name at `precision_`, the
    /// precision is lowered from `maxPrecision_` one digit at a time, stopping as soon as the name no longer changes, and the last precision
    /// whose name is an existing directory is kept (so a name that is the same at several precisions keeps the LARGEST of them); when none
    /// matches, the precision stays. `exists` says whether a directory of that name exists.
    #[must_use]
    pub fn for_start_directory(value: f64, precision: usize, exists: impl Fn(&str) -> bool) -> Self {
        let mut me = Self::new(value, precision);
        if exists(&me.name) {
            return me;
        }
        let mut required: Option<usize> = None;
        let mut old = me.name.clone();
        let mut p = MAX_TIME_PRECISION;
        while p > precision {
            let new = time_name(value, p);
            if new == old {
                break;
            }
            old = new.clone();
            if exists(&new) {
                required = Some(p);
            }
            p -= 1;
        }
        if let Some(r) = required {
            me = Self::new(value, r);
        }
        me
    }

    /// `Time::precision_`.
    #[must_use]
    pub fn precision(&self) -> usize {
        self.precision
    }

    /// `Time::timeName()`.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// `Time::operator++` after the clock advanced from `old_value` to `value` with `delta_t`: the new name at the current precision, and, at a write
    /// time, the precision increase of `Time.C:1189-1225`. Returns `Some((old, new))` when the precision grew ("Increased the timePrecision from old
    /// to new").
    pub fn advance(&mut self, old_value: f64, value: f64, delta_t: f64, write_time: bool) -> Option<(usize, usize)> {
        self.name = time_name(value, self.precision);
        if !write_time {
            return None;
        }
        // `timeToUserTime` is the identity (no user time in these cases)
        let user_delta_t = value - (value - delta_t);
        // `pow(10, -precision_)`: the correctly rounded decimal `1e-p` (what the platform `pow` returns for these powers)
        let ten_pow = |p: usize| -> f64 {
            match format!("1e-{p}").parse::<f64>() {
                Ok(x) => x,
                Err(_) => SMALL,
            }
        };
        let tol = |p: usize| (ten_pow(p).min(0.1 * user_delta_t)).max(SMALL);
        let differs = |name: &str, p: usize| match read_back(name) {
            Some(v) => (v - old_value - user_delta_t).abs() > tol(p),
            None => false,
        };
        // `timeTol` is fixed by the precision BEFORE the loop (`Time.C:1196-1197`)
        let tol0 = self.precision;
        if !differs(&self.name, tol0) {
            return None;
        }
        let old_precision = self.precision;
        while self.precision < MAX_TIME_PRECISION && differs(&self.name, tol0) {
            self.precision += 1;
            self.name = time_name(value, self.precision);
        }
        if self.precision == old_precision {
            None
        } else {
            Some((old_precision, self.precision))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The precision steps the real binary printed on 2D_fullCoupling (README round 84, `sh_ts9`): `timePrecision 8`, start `100`, first step
    /// `deltaT 1e-6`: "Increased the timePrecision from 8 to 9 ... at time 100.000001".
    #[test]
    fn the_first_write_after_a_one_microsecond_step_grows_the_precision_from_8_to_9() {
        let mut n = TimeName::new(100.0, 8);
        assert_eq!(n.name(), "100");
        let t1 = 100.0 + 1e-6;
        assert_eq!(n.advance(100.0, t1, 1e-6, true), Some((8, 9)));
        assert_eq!(n.name(), "100.000001");
        // not a write time: the name follows the precision, nothing grows
        let mut m = TimeName::new(100.0, 8);
        assert_eq!(m.advance(100.0, t1, 1e-6, false), None);
        assert_eq!(m.name(), "100");
    }

    #[test]
    fn a_start_directory_written_at_a_larger_precision_is_found() {
        let v = 100.000_025_959;
        let n = TimeName::for_start_directory(v, 8, |d| d == "100.000025959");
        // upstream walks DOWN from `maxPrecision_` and stops when the name no longer changes (`Time.C:211-222`): the name is the same at
        // 12 .. 17 digits, so the loop stops at 16 with 17 recorded, not the smallest precision that would do
        assert_eq!((n.precision(), n.name()), (17, "100.000025959"));
        let same = TimeName::for_start_directory(100.0, 8, |d| d == "100");
        assert_eq!(same.precision(), 8);
        let none = TimeName::for_start_directory(v, 8, |_| false);
        assert_eq!(none.precision(), 8);
    }
}
