// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! Non-panicking element access for the GSL linear-algebra ports (gh:#993).
//!
//! **Neither ported nor lifted — crate-local plumbing**, like
//! [`crate::zip`]: no arithmetic. GSL's loops read `x[i]`; a Rust subscript
//! can panic, which `tests/no_panic_gate.rs` forbids, so the ports read
//! [`rd`] and write [`wr`] instead. Out of range reads `NaN` and writes are
//! discarded, the same contract as [`Matrix::get`](crate::linalg::Matrix::get)
//! and [`Matrix::set`](crate::linalg::Matrix::set). Every port sizes its
//! vectors before the loop, so neither fallback is reached by a correct call.

/// `v[i]`, or `NaN` if out of range.
#[inline]
pub(crate) fn rd(v: &[f64], i: usize) -> f64 {
    match v.get(i) {
        Some(&x) => x,
        None => f64::NAN,
    }
}

/// `v[i] = x`, discarded if out of range.
#[inline]
pub(crate) fn wr(v: &mut [f64], i: usize, x: f64) {
    if let Some(s) = v.get_mut(i) {
        *s = x;
    }
}

/// `v[i] += inc` (C's compound assignment, the same single rounding),
/// discarded if out of range.
#[inline]
pub(crate) fn acc(v: &mut [f64], i: usize, inc: f64) {
    if let Some(s) = v.get_mut(i) {
        *s += inc;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn out_of_range_reads_nan_and_writes_are_discarded() {
        let mut v = [1.0, 2.0];
        assert_eq!(rd(&v, 1), 2.0);
        assert!(rd(&v, 2).is_nan());
        wr(&mut v, 0, 5.0);
        wr(&mut v, 9, 7.0);
        acc(&mut v, 1, 0.5);
        acc(&mut v, 9, 0.5);
        assert_eq!(v, [5.0, 2.5]);
    }
}
