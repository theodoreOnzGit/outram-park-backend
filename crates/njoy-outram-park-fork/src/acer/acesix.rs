//! `aceth.f90`'s `acesix` — turning a THERMR MF=6 emission matrix into the
//! equally-probable outgoing-energy bins the ACE `ITXE` block stores
//! (`IFENG = 0`), and which a Monte-Carlo code samples directly.
//!
//! # Why this exists
//!
//! This is the **only** faithful way to get equally-probable emission bins out
//! of a thermal kernel, and it replaces a home-grown routine
//! (`IncoherentInelastic::equiprobable_emission`) that was measured 1.584 %
//! narrow against NJOY's own matrix — the defect behind GitHub #188. See
//! `tests/thermr_kernel_vs_njoy2016_matrix.rs` for that measurement.
//!
//! # The algorithm (`aceth.f90:294-503`)
//!
//! For one incident energy, the MF=6 record holds `nep` rows of
//! `[E', f(E->E'), mu_1 .. mu_nang]`. `acesix` walks that table once,
//! accumulating the trapezoidal area `sum` and the first moment `grall` of the
//! *piecewise-linear* `f`, and closes a bin whenever `sum` reaches that bin's
//! target area `fract`:
//!
//! - inside a panel, the bin edge `xn` solves `∫f = fract - sum` exactly. `f`
//!   is linear there, so that is a quadratic in `xn`
//!   (`disc = (yl/f)^2 + 2(fract-sum)/f`, `aceth.f90:409-418`), degenerating
//!   to the linear `xn = xl + (fract-sum)/yl` when the slope is negligible.
//! - the bin's stored energy is **`xbar = grall / sum`** — the area-weighted
//!   *mean* `E'` over the bin, not its midpoint and not a CDF-inversion point.
//! - the stored cosines are interpolated **at `xbar`** between the two rows
//!   bracketing it (`aceth.f90:463-468`), then clamped to `[-1, 1]` exactly as
//!   upstream does (`aceth.f90:478-490`, which also warns; we clamp silently).
//! - `sum` and `grall` reset to zero at each bin (`aceth.f90:501-502`), so
//!   `fract` is a per-bin area, not a running cumulative target.
//!
//! Before any of that, `acesix` **overwrites the first and last rows' cosines
//! with their neighbours'** (`aceth.f90:344-352`). Those rows sit at the edges
//! of the kinematic range where `f` is zero and the tabulated angles carry no
//! information; using them unmodified would drag the first and last bins'
//! cosines toward meaningless values.
//!
//! # Bin weighting
//!
//! `iwt` selects the target areas ([`BinWeights`], `aceth.f90:296-313`):
//! `Variable` is NJOY's `1 4 10 ... 10 4 1` pattern (the default, which spends
//! more bins on the distribution's tails), `Constant` is a flat `1/nbin`. The
//! tabulated form (`iwt > 1`, `IFENG = 2`) writes `(xn, yn, cdf)` triples
//! instead of `xbar` and is **not** ported here.

use crate::thermr::calcem::types::EqualProbableRow;

/// One equally-probable outgoing-energy bin.
#[derive(Debug, Clone, PartialEq)]
pub struct AcesixBin {
    /// `xbar` — the area-weighted mean `E'` over the bin \[eV\].
    pub e_out_ev: f64,
    /// The `nang` equally-probable cosines, interpolated at `xbar`.
    pub cosines: Vec<f64>,
}

/// `iwt`: the pattern of per-bin target areas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinWeights {
    /// `iwt = 0` — NJOY's default `1 4 10 ... 10 4 1` relative pattern.
    /// Needs `nbin > 3`.
    Variable,
    /// `iwt = 1` — a flat `1/nbin`.
    Constant,
}

impl BinWeights {
    /// The normalized per-bin target areas (`wt(1..nbin)`, `aceth.f90:296-313`).
    fn weights(self, nbin: usize) -> Vec<f64> {
        match self {
            BinWeights::Constant => vec![1.0 / nbin as f64; nbin],
            BinWeights::Variable => {
                // wtt = 1/10/(nbin-3); interior 10*wtt, ends wtt, next-to-ends
                // 4*wtt. Sums to 1 exactly for nbin > 3.
                let wtt = 1.0 / 10.0 / (nbin as f64 - 3.0);
                (0..nbin)
                    .map(|i| {
                        if i == 0 || i == nbin - 1 {
                            wtt
                        } else if i == 1 || i == nbin - 2 {
                            4.0 * wtt
                        } else {
                            10.0 * wtt
                        }
                    })
                    .collect()
            }
        }
    }
}

/// Run `acesix`'s equally-probable solve over one incident energy's MF=6 rows.
///
/// `rows` are the emission table for a single incident energy, ascending in
/// `E'`, with `pdf` the **unnormalized** `σ(E→E')` that `calcem` produces — it
/// is normalized here, since `acesix` reads an ENDF MF=6 record whose law is
/// already normalized to unit integral.
///
/// Returns `nbin` bins, or an empty vector when the kernel carries no area
/// (`σ_inel = 0`, i.e. no scatter) or the inputs are degenerate.
pub fn acesix_equiprobable(rows: &[EqualProbableRow], nbin: usize, iwt: BinWeights) -> Vec<AcesixBin> {
    let nep = rows.len();
    if nep < 2 || nbin == 0 {
        return Vec::new();
    }
    if iwt == BinWeights::Variable && nbin <= 3 {
        return Vec::new();
    }
    let nang = rows[0].cosines.len();

    // NOTE: the law is used EXACTLY as handed in — `acesix` does not normalize,
    // and neither may we. `fract` is an absolute area target drawn from `wt`
    // (which sums to 1), so scaling the ordinates even slightly walks every
    // subsequent bin boundary. Measured: normalizing by the law's own integral
    // (which a 7-significant-figure MF=6 tape carries to within ~7e-7 of 1)
    // left bins 0-37 exact and then drifted monotonically to 2.6e-5 by bin 63 —
    // cumulative, because each bin resumes at the previous bin's edge. Callers
    // holding an unnormalized law (`calcem`'s rows are sigma(E->E') in barns)
    // must normalize BEFORE calling, which is what THERMR's own MF=6 writer
    // does. [`normalized_rows`] is that step.
    let total: f64 = rows
        .windows(2)
        .map(|w| (w[1].pdf + w[0].pdf) * (w[1].ep_ev - w[0].ep_ev) / 2.0)
        .sum();
    if !(total > 0.0) {
        return Vec::new();
    }

    // `aceth.f90:344-352` — the first and last rows' cosines are replaced by
    // their neighbours' before the solve.
    let mut mu: Vec<Vec<f64>> = rows.iter().map(|r| r.cosines.clone()).collect();
    if nep >= 2 {
        mu[0] = mu[1].clone();
        mu[nep - 1] = mu[nep - 2].clone();
    }
    let ep: Vec<f64> = rows.iter().map(|r| r.ep_ev).collect();
    let pdf: Vec<f64> = rows.iter().map(|r| r.pdf).collect();

    let wt = iwt.weights(nbin);
    let mut out: Vec<AcesixBin> = Vec::with_capacity(nbin);

    let (mut sum, mut grall) = (0.0f64, 0.0f64);
    let (mut xl, mut yl) = (ep[0], pdf[0]);
    let mut fract = wt[0];
    let mut j = 0usize; // bins closed so far
    let mut i = 0usize; // rows consumed (1-based in Fortran)

    // `240` — walk the rows.
    while i < nep {
        let (x, y) = (ep[i], pdf[i]);
        i += 1;
        // `250` — (re)process this panel, possibly closing several bins in it.
        loop {
            let add = (y + yl) * (x - xl) / 2.0;
            if x == xl {
                break;
            }
            let last_bin_takes_remainder = i == nep && j + 1 == nbin;
            let closes = last_bin_takes_remainder || sum + add >= fract - fract / 10_000.0;
            if !closes {
                // Whole panel is inside the current bin.
                sum += add;
                grall += moment(xl, yl, x, y, x);
                break;
            }
            // `260` — the bin edge falls inside this panel (or the last bin
            // swallows what is left).
            j += 1;
            let xn = if last_bin_takes_remainder || sum + add < fract + fract / 10_000.0 {
                x
            } else if (y - yl).abs() > (y + yl) / 100_000.0 {
                let f = (y - yl) / (x - xl);
                // `aceth.f90:411-415` takes |disc| and continues rather than
                // failing; reproduced, since a negative disc here is a rounding
                // artefact of a nearly-exhausted panel.
                let disc = ((yl / f).powi(2) + 2.0 * (fract - sum) / f).abs();
                let sign = if f < 0.0 { -1.0 } else { 1.0 };
                (xl - yl / f + sign * disc.sqrt()).clamp(xl, x)
            } else {
                (xl + (fract - sum) / yl).min(x)
            };

            // `270` — close the bin on [xl, xn].
            let yn = yl + (y - yl) * (xn - xl) / (x - xl);
            sum += (yn + yl) * (xn - xl) / 2.0;
            grall += moment(xl, yl, x, y, xn);
            let xbar = if sum > 0.0 { grall / sum } else { xn };

            // Bracket `xbar` and interpolate the cosines there
            // (`aceth.f90:438-468`).
            let mut l = 1usize; // Fortran l=2 -> rows (l-2, l-1) = (0, 1)
            while l + 1 < nep && ep[l] < xbar {
                l += 1;
            }
            let (xlo, xhi) = (ep[l - 1], ep[l]);
            let t = if xhi > xlo {
                (xbar - xlo) / (xhi - xlo)
            } else {
                0.0
            };
            let cosines = (0..nang)
                .map(|k| (mu[l - 1][k] + (mu[l][k] - mu[l - 1][k]) * t).clamp(-1.0, 1.0))
                .collect();
            out.push(AcesixBin {
                e_out_ev: xbar,
                cosines,
            });

            xl = xn;
            yl = yn;
            sum = 0.0;
            grall = 0.0;
            if j >= nbin {
                return out;
            }
            fract = wt[j];
            if xl >= x {
                break;
            }
            // else: `go to 250` — same panel, next bin.
        }
        // `280`
        xl = x;
        yl = y;
    }
    out
}

/// `∫ E' f(E') dE'` from `xl` to `upto`, with `f` linear through
/// `(xl, yl)-(x, y)` (`aceth.f90:401-402, 435-436`).
fn moment(xl: f64, yl: f64, x: f64, y: f64, upto: f64) -> f64 {
    let slope = (y - yl) / (x - xl);
    let intercept = yl - slope * xl;
    intercept * (upto * upto - xl * xl) / 2.0 + slope * (upto.powi(3) - xl.powi(3)) / 3.0
}

/// Scale a law to unit integral, the way THERMR's MF=6 writer does before the
/// record reaches `acesix`.
///
/// [`acesix_equiprobable`] deliberately does **not** do this itself (see the
/// note in its body): `fract` is an absolute area target, so rescaling the
/// ordinates walks every later bin boundary. `calcem` produces
/// `sigma(E -> E')` in barns, whose integral is `sigma_inel` rather than 1, so
/// that path runs through here first.
///
/// Returns an empty vector if the law carries no area.
pub fn normalized_rows(rows: &[EqualProbableRow]) -> Vec<EqualProbableRow> {
    let total: f64 = rows
        .windows(2)
        .map(|w| (w[1].pdf + w[0].pdf) * (w[1].ep_ev - w[0].ep_ev) / 2.0)
        .sum();
    if !(total > 0.0) {
        return Vec::new();
    }
    rows.iter()
        .map(|r| EqualProbableRow {
            ep_ev: r.ep_ev,
            pdf: r.pdf / total,
            cosines: r.cosines.clone(),
        })
        .collect()
}
