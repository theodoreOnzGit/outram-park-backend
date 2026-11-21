// SPDX-License-Identifier: AGPL-3.0-only
//
// Attribution (RESEARCH_INTEGRITY_AND_PROVENANCE.md): port of EGSnrc.
//   Upstream project: EGSnrc, https://github.com/nrc-cnrc/EGSnrc
//   Upstream file:    HEN_HOUSE/src/nrcaux.mortran
//   Upstream commit:  bd49a47aae3938f49f4100b8e018091861408b19
//   Copyright:        National Research Council of Canada (and SLAC for the
//                     EGS4-derived parts, distributed by NRC under AGPL-3.0)
//   Upstream licence: AGPL-3.0-or-later; this port is AGPL-3.0-only.
//! `HEN_HOUSE/src/nrcaux.mortran` (EGSnrc).
//!
//! Upstream: <https://github.com/nrc-cnrc/EGSnrc>, commit `bd49a47aae3938f49f4100b8e018091861408b19`
//! (read 2026-11-20, a shallow clone, never committed here). Licence: AGPL-3.0
//! (the file headers read version 3 or, at the licensee's option, later;
//! the SLAC-derived parts carry their own notice in the header).
//!
//! NRC auxiliary routines the core calls (599 lines). Description from the file's header and size; not yet read line by line.
//!
//! **Status: first port of `SIGMA`, `prepare_alias_sampling` and `alias_sample`,
//! not differentially tested (gh:#1680).** `WATCH` (event-by-event trace
//! printing for graphics output) is I/O only and is not ported.
//!
//! | upstream | here |
//! |---|---|
//! | `SUBROUTINE SIGMA` (`nrcaux.mortran:422-495`) | [`sigma`] |
//! | `subroutine prepare_alias_sampling` (`:507-557`) | [`prepare_alias_sampling`] |
//! | `function alias_sample` (`:566-590`) | [`alias_sample`] |

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::egsnrc_macros::UniformSource;
use alloc::vec::Vec;
use petir::real::Real;

/// What `sigma` returns in its by-reference arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SigmaStatus {
    /// `IERR`: 0 normal, 1 the mode was out of range (and reset to 2), 10 only
    /// one batch, 11 a datum with no non-zero entry, -1 fatal input error.
    pub ierr: i32,
    /// `MODE` on return (reset to 2 if it was out of range).
    pub mode: i32,
}

/// `SIGMA` (`nrcaux.mortran:422-495`): mean and relative statistical
/// uncertainty (in percent) over batches. `data[n - 1][i - 1]` is the
/// quantity `n` in batch `i` (`DATA(N,I)`); on return `data[n-1][0]` holds the
/// mean (the sum when `mode == 2`) and `data[n-1][1]` its uncertainty, capped
/// at 99.9 %. `mode`: 0 use only the non-zero batches, 1 treat zeros as
/// data, 2 as 1 and return the sum. `ndata` and `istat` are the number of
/// quantities and batches in use.
///
/// The dimensions `$MXDATA` and `$STAT` are the shape of `data`.
pub fn sigma(data: &mut [Vec<f64>], ndata: usize, istat: usize, mode: i32) -> SigmaStatus {
    const EMAX: f64 = 99.9;
    let mut mode = mode;
    let mut ierr = 0;
    if !(0..=2).contains(&mode) {
        mode = 2;
        ierr = 1;
    }
    let mxdata = data.len();
    let stat_dim = data.first().map_or(0, Vec::len);
    if ndata == 0 || ndata > mxdata || istat == 0 || istat > stat_dim {
        return SigmaStatus { ierr: -1, mode };
    }
    if istat == 1 {
        for n in 0..ndata {
            data[n][1.min(stat_dim - 1)] = EMAX;
        }
        return SigmaStatus { ierr: 10, mode };
    }
    let mut stat = 0.0_f64;
    let mut sdenom = 0.0_f64;
    if mode != 0 {
        stat = istat as f64;
        sdenom = stat * (stat - 1.0);
    }
    for n in 0..ndata {
        let mut non0 = 0;
        let mut avg = 0.0_f64;
        let mut error = 0.0_f64;
        for i in 0..istat {
            let datum = data[n][i];
            if datum != 0.0 {
                non0 += 1;
                avg += datum;
                error += datum * datum;
            }
        }
        let transfer;
        if non0 == 0 {
            ierr = 11;
            error = EMAX;
            transfer = true;
        } else if non0 == 1 && mode == 0 {
            error = EMAX;
            transfer = true;
        } else {
            if mode == 0 {
                stat = non0 as f64;
                sdenom = stat * (stat - 1.0);
            }
            transfer = false;
        }
        if !transfer {
            avg /= stat;
            let mut argmnt = error - stat * avg * avg;
            if argmnt < 0.0 {
                argmnt = 0.0; // round-off only
            }
            error = Real::sqrt(argmnt / sdenom);
            error = if avg == 0.0 { EMAX } else { 100.0 * error / avg.abs() };
            if mode == 2 {
                avg *= stat;
            }
        }
        data[n][0] = avg;
        data[n][1] = EMAX.min(error);
    }
    SigmaStatus { ierr, mode }
}

/// `prepare_alias_sampling` (`nrcaux.mortran:507-557`): an alias table for a
/// histogram of `nsbin` bins. `fs` holds the bin probabilities **1-based**
/// (`fs[0]` unused; values below 1e-30 are raised to it, as upstream does in
/// place). Returns `(ws, ibin)`, 1-based like the input.
pub fn prepare_alias_sampling(fs: &mut [f64]) -> (Vec<f64>, Vec<i32>) {
    let nsbin = fs.len() - 1;
    let mut ws = alloc::vec![0.0_f64; nsbin + 1];
    let mut ibin = alloc::vec![0_i32; nsbin + 1];
    let mut sum = 0.0;
    for i in 1..=nsbin {
        if fs[i] < 1e-30 {
            fs[i] = 1e-30;
        }
        ws[i] = -fs[i];
        ibin[i] = 1;
        sum += fs[i];
    }
    sum /= nsbin as f64;
    for i in 1..nsbin {
        // the Mortran DO loops leave the index at nsbin if no bin qualifies
        let mut j_h = nsbin;
        for jh in 1..=nsbin {
            if ws[jh] < 0.0 && ws[jh].abs() > sum {
                j_h = jh;
                break;
            }
        }
        let mut j_l = nsbin;
        for jl in 1..=nsbin {
            if ws[jl] < 0.0 && ws[jl].abs() < sum {
                j_l = jl;
                break;
            }
        }
        let aux = sum - ws[j_l].abs();
        ws[j_h] += aux;
        ws[j_l] = -ws[j_l] / sum;
        ibin[j_l] = j_h as i32;
        if i == nsbin - 1 {
            ws[j_h] = 1.0;
        }
    }
    (ws, ibin)
}

/// `alias_sample` (`nrcaux.mortran:566-590`): a value from the histogram with
/// bin limits `xs[0..=nsbin]` (0-based, `nsbin + 1` values) and an alias table
/// from [`prepare_alias_sampling`] (`ws`, `ibin` 1-based).
pub fn alias_sample<R: UniformSource>(xs: &[f64], ws: &[f64], ibin: &[i32], rng: &mut R) -> f64 {
    let nsbin = ws.len() - 1;
    let v1 = rng.uniform();
    let v2 = rng.uniform();
    let mut aj = 1.0 + v1 * nsbin as f64;
    let mut j = aj as usize;
    if j > nsbin {
        j = nsbin; // only if the generator can return 1
    }
    aj -= j as f64;
    if aj > ws[j] {
        j = ibin[j] as usize;
    }
    (1.0 - v2) * xs[j - 1] + v2 * xs[j]
}

#[cfg(test)]
mod tests {
    use super::*;
    use petir::rng::ranmar::Ranmar;

    #[test]
    fn sigma_mean_and_relative_error() {
        // one quantity over four batches 1, 2, 3, 4
        let mut d = alloc::vec![alloc::vec![1.0, 2.0, 3.0, 4.0]];
        let st = sigma(&mut d, 1, 4, 1);
        assert_eq!(st, SigmaStatus { ierr: 0, mode: 1 });
        assert!((d[0][0] - 2.5).abs() < 1e-12);
        // sqrt((30 - 4*2.5^2) / (4*3)) / 2.5 * 100
        let want = ((30.0 - 25.0) / 12.0f64).sqrt() / 2.5 * 100.0;
        assert!((d[0][1] - want).abs() < 1e-12, "{} vs {want}", d[0][1]);
        // mode 2 returns the sum
        let mut d = alloc::vec![alloc::vec![1.0, 2.0, 3.0, 4.0]];
        sigma(&mut d, 1, 4, 2);
        assert!((d[0][0] - 10.0).abs() < 1e-12);
        // mode 0 ignores the zero batches
        let mut d = alloc::vec![alloc::vec![2.0, 0.0, 4.0, 0.0]];
        sigma(&mut d, 1, 4, 0);
        assert!((d[0][0] - 3.0).abs() < 1e-12);
        let want = ((20.0 - 2.0 * 9.0) / 2.0f64).sqrt() / 3.0 * 100.0;
        assert!((d[0][1] - want).abs() < 1e-12);
    }

    #[test]
    fn sigma_status_codes() {
        let mut d = alloc::vec![alloc::vec![1.0, 2.0, 3.0]];
        // mode out of range is reset to 2 with ierr = 1
        let st = sigma(&mut d, 1, 3, 7);
        assert_eq!(st, SigmaStatus { ierr: 1, mode: 2 });
        // bad dimensions are fatal
        assert_eq!(sigma(&mut d, 2, 3, 1).ierr, -1);
        assert_eq!(sigma(&mut d, 1, 4, 1).ierr, -1);
        assert_eq!(sigma(&mut d, 0, 3, 1).ierr, -1);
        assert_eq!(sigma(&mut d, 1, 0, 1).ierr, -1);
        // a single batch: uncertainty is the cap
        let mut one = alloc::vec![alloc::vec![5.0, 0.0]];
        assert_eq!(sigma(&mut one, 1, 1, 1).ierr, 10);
        assert_eq!(one[0][1], 99.9);
        // all zero: ierr 11, cap, mean zero
        let mut z = alloc::vec![alloc::vec![0.0, 0.0, 0.0]];
        assert_eq!(sigma(&mut z, 1, 3, 1).ierr, 11);
        assert_eq!((z[0][0], z[0][1]), (0.0, 99.9));
        // mode 0 with a single non-zero datum
        let mut s1 = alloc::vec![alloc::vec![0.0, 3.0, 0.0]];
        assert_eq!(sigma(&mut s1, 1, 3, 0).ierr, 0);
        assert_eq!((s1[0][0], s1[0][1]), (3.0, 99.9));
        // a zero mean with non-zero data is capped
        let mut m = alloc::vec![alloc::vec![1.0, -1.0, 0.0]];
        sigma(&mut m, 1, 3, 1);
        assert_eq!(m[0][1], 99.9);
    }


    /// **Upstream finding.** `prepare_alias_sampling` finalises a bin only when
    /// its weight is strictly above or below the average, so a bin that is
    /// *exactly* average (a uniform histogram, or a tie such as 0.4 - 0.15 =
    /// 0.25) is left with a negative weight and is always redirected through
    /// its initial alias, bin 1. `init_compton` avoids this with a final loop
    /// this routine does not have. Pinned here as upstream behaves.
    #[test]
    fn exactly_average_bins_stay_unfinalised_as_upstream() {
        let mut fs = alloc::vec![0.0, 0.25, 0.25, 0.25, 0.25];
        let (ws, ibin) = prepare_alias_sampling(&mut fs);
        assert!(ws[1] < 0.0 && ws[2] < 0.0 && ws[3] < 0.0, "bins 1-3 never finalised: {ws:?}");
        assert_eq!(ws[4], 1.0);
        assert_eq!((ibin[1], ibin[2], ibin[3]), (1, 1, 1));
        let xs = [0.0, 1.0, 2.0, 3.0, 4.0];
        let mut rng = Ranmar::new(3, 4);
        let n = 20_000;
        let in_first = (0..n).filter(|_| alias_sample(&xs, &ws, &ibin, &mut rng) < 1.0).count();
        assert!(in_first as f64 / n as f64 > 0.7, "three quarters of the samples land in bin 1");
    }

    #[test]
    fn alias_table_reproduces_bin_probabilities_and_samples_them() {
        let mut fs = alloc::vec![0.0, 0.1, 0.35, 0.22, 0.33];
        let (ws, ibin) = prepare_alias_sampling(&mut fs);
        let nsbin = 4;
        // exact reconstruction: P(bin s) = (1/n) [ws(s) + sum over bins aliased to s of (1 - ws(i))]
        for s in 1..=nsbin {
            let mut p = ws[s].clamp(0.0, 1.0);
            for i in 1..=nsbin {
                if ibin[i] as usize == s && i != s {
                    p += 1.0 - ws[i].clamp(0.0, 1.0);
                }
            }
            p /= nsbin as f64;
            assert!((p - fs[s]).abs() < 1e-12, "bin {s}: {p} vs {}", fs[s]);
        }
        // sampling reproduces the histogram
        let xs = [0.0, 1.0, 2.0, 3.0, 4.0];
        let mut rng = Ranmar::new(8, 9);
        let mut counts = [0usize; 4];
        let n = 100_000;
        for _ in 0..n {
            let x = alias_sample(&xs, &ws, &ibin, &mut rng);
            counts[x as usize] += 1;
        }
        for b in 0..4 {
            assert!((counts[b] as f64 / n as f64 - fs[b + 1]).abs() < 0.01, "bin {b}");
        }
        // zero-probability bins are raised to 1e-30 in place
        let mut z = alloc::vec![0.0, 0.0, 1.0];
        prepare_alias_sampling(&mut z);
        assert_eq!(z[1], 1e-30);
    }
}
