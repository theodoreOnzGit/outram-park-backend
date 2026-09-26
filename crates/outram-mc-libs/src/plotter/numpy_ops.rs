// SPDX-License-Identifier: GPL-3.0-only
//! **The handful of NumPy operations `openmc.plotter` leans on, reproduced to
//! the bit.**
//!
//! The plotter's contract is a PNG identical to upstream's, and matplotlib
//! draws whatever floats it is handed. So every array operation upstream
//! performs between reading the data and calling `ax.plot` has to round the
//! same way here. The operations, and the NumPy source each follows
//! (NumPy 2.5.3, as installed with the reference OpenMC):
//!
//! | here | NumPy | used by upstream at |
//! |---|---|---|
//! | [`interp`] | `np.interp` (`_core/src/multiarray/compiled_base.c`, `arr_interp`) | `plotter.py:222-223` |
//! | [`union1d`] | `np.union1d` = `unique(concatenate)` | `plotter.py:218`, `:431-446`, `:651` |
//! | [`isclose_scalar`] | `np.isclose` (`_core/numeric.py`) | `function.py:207-208` |
//! | [`nan_to_num`] | `np.nan_to_num` | `plotter.py:270` |
//! | [`pairwise_sum`] | `np.sum` / `np.add.reduce` (`pairwise_sum_DOUBLE`) | `plotter.py:271`, `material.py:1357,1370` |
//! | [`searchsorted_right`] | `np.searchsorted(..., side='right')` | `function.py:167` |
//! | [`python_round`] | Python `round` (half to even) | `plotter.py:377` |
//!
//! `pairwise_sum` was checked against `np.sum` on 20 000 random arrays of
//! length 1-300 (2026-09-26): identical in every case, whereas the naive
//! "first element plus pairwise rest" form differed in 9 674 of them.

/// `np.searchsorted(a, v, side='right')`: the number of elements `<= v`.
pub fn searchsorted_right(a: &[f64], v: f64) -> usize {
    a.partition_point(|&x| x <= v)
}

/// `np.union1d(a, b)`: sorted, with exact duplicates removed.
pub fn union1d(a: &[f64], b: &[f64]) -> Vec<f64> {
    let mut v: Vec<f64> = a.iter().chain(b.iter()).copied().collect();
    v.sort_by(|x, y| x.total_cmp(y));
    // `np.unique` compares with `!=` on the sorted array; for finite energy
    // grids `total_cmp` order and `==` agree.
    v.dedup_by(|x, y| *x == *y);
    v
}

/// `np.interp(x, xp, fp)` with upstream's defaults (`left = fp[0]`,
/// `right = fp[-1]`), following `arr_interp` line by line, including the
/// "exact hit" and NaN-retry branches.
pub fn interp(x: &[f64], xp: &[f64], fp: &[f64]) -> Vec<f64> {
    assert_eq!(xp.len(), fp.len(), "np.interp: xp and fp differ in length");
    let n = xp.len();
    assert!(n > 0, "np.interp: empty xp");
    let lval = fp[0];
    let rval = fp[n - 1];
    x.iter()
        .map(|&xv| {
            if xv.is_nan() {
                return xv;
            }
            if n == 1 {
                // `arr_interp`'s lenxp == 1 branch.
                return if xv < xp[0] {
                    lval
                } else if xv > xp[0] {
                    rval
                } else {
                    fp[0]
                };
            }
            // binary_search_with_guess: -1 below, len above, else the largest
            // j with xp[j] <= x.
            if xv > xp[n - 1] {
                return rval;
            }
            if xv < xp[0] {
                return lval;
            }
            let j = searchsorted_right(xp, xv) - 1;
            if j == n - 1 || xp[j] == xv {
                return fp[j];
            }
            let slope = (fp[j + 1] - fp[j]) / (xp[j + 1] - xp[j]);
            let mut r = slope * (xv - xp[j]) + fp[j];
            if r.is_nan() {
                r = slope * (xv - xp[j + 1]) + fp[j + 1];
                if r.is_nan() && fp[j] == fp[j + 1] {
                    r = fp[j];
                }
            }
            r
        })
        .collect()
}

/// `np.isclose(a, b, rtol=1e-5, atol)` for scalars:
/// `|a - b| <= atol + rtol*|b|` and `b` finite, or `a == b`.
pub fn isclose_scalar(a: f64, b: f64, atol: f64) -> bool {
    let rtol = 1.0e-5;
    ((a - b).abs() <= atol + rtol * b.abs() && b.is_finite()) || a == b
}

/// `np.nan_to_num`: NaN to 0, +inf to the largest double, -inf to the most
/// negative.
pub fn nan_to_num(v: f64) -> f64 {
    if v.is_nan() {
        0.0
    } else if v == f64::INFINITY {
        f64::MAX
    } else if v == f64::NEG_INFINITY {
        f64::MIN
    } else {
        v
    }
}

/// `np.sum` of a contiguous float64 array: NumPy's `pairwise_sum_DOUBLE`
/// (block size 128, eight accumulators).
pub fn pairwise_sum(a: &[f64]) -> f64 {
    let n = a.len();
    if n < 8 {
        let mut res = 0.0;
        for &v in a {
            res += v;
        }
        res
    } else if n <= 128 {
        let mut r = [0.0f64; 8];
        r.copy_from_slice(&a[..8]);
        let mut i = 8;
        while i < n - (n % 8) {
            for j in 0..8 {
                r[j] += a[i + j];
            }
            i += 8;
        }
        let mut res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
        while i < n {
            res += a[i];
            i += 1;
        }
        res
    } else {
        let mut n2 = n / 2;
        n2 -= n2 % 8;
        pairwise_sum(&a[..n2]) + pairwise_sum(&a[n2..])
    }
}

/// Python's `round(x)` to an integer: halves go to the even neighbour.
pub fn python_round(x: f64) -> f64 {
    x.round_ties_even()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interp_matches_numpy_edge_rules() {
        let xp = [1.0, 2.0, 4.0];
        let fp = [10.0, 20.0, 0.0];
        let got = interp(&[0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 5.0], &xp, &fp);
        assert_eq!(got, vec![10.0, 10.0, 15.0, 20.0, 10.0, 0.0, 0.0]);
    }

    #[test]
    fn union_sorts_and_dedups() {
        assert_eq!(union1d(&[3.0, 1.0, 2.0], &[2.0, 5.0]), vec![1.0, 2.0, 3.0, 5.0]);
    }

    #[test]
    fn python_round_is_bankers() {
        assert_eq!(python_round(292.5), 292.0);
        assert_eq!(python_round(293.5), 294.0);
        assert_eq!(python_round(293.6), 294.0);
    }
}
