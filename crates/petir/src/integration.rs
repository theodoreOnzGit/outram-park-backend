// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of PETIR, a component of OUTRAM PARK.
//
// PETIR is free software: you can redistribute it and/or modify it under the
// terms of the GNU General Public License as published by the Free Software
// Foundation, version 3 of the License.
//
// PETIR is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along
// with PETIR.  If not, see <https://www.gnu.org/licenses/>.
//
// PORTED from the GNU Scientific Library (GSL) 2.8, commit
// cf180cd7fbd06039a577f9c9ff0b428784765ac1, read 2026-09-14.
// Copyright (C) 1996-2000 Brian Gough
// GPL-3.0-or-later (verified from the per-file headers; see NOTICE).
//
// GSL's integration layer is itself a translation of QUADPACK (Piessens,
// de Doncker-Kapenga, Uberhuber and Kahaner, 1983), which is public domain.
//
//   integration/qk.c            gsl_integration_qk  -> kronrod
//   integration/qk{15,21,31,41,51,61}.c  the node and weight tables
//   integration/err.c           rescale_error       -> rescale_error
//   integration/qag.c           qag                 -> qag

//! Numerical quadrature — GSL's `integration/`, itself QUADPACK.
//!
//! # What is here
//!
//! - [`kronrod`] — one Gauss-Kronrod rule over one interval, returning the
//!   estimate and a genuine error bound. The building block.
//! - [`qag`] — **adaptive** quadrature: apply the rule, find the sub-interval
//!   with the worst error, bisect it, repeat. The routine to reach for.
//!
//! # How Gauss-Kronrod gets an error estimate for free
//!
//! An `n`-point Gauss rule is exact for polynomials of degree `2n - 1`. The
//! trick Kronrod added is to *extend* it with `n + 1` more points chosen so the
//! combined `2n + 1`-point rule is exact to degree `3n + 1` — **and the
//! original `n` Gauss points are reused**. So evaluating the Kronrod rule gives
//! you the Gauss result at no extra cost, and the difference between the two is
//! an estimate of the error.
//!
//! That is why these rules are named in pairs (`15` means 15 Kronrod points
//! wrapping a 7-point Gauss rule) and why quadrature libraries almost
//! universally use them: an error estimate that costs nothing is what makes
//! adaptivity possible.
//!
//! # Choosing a rule
//!
//! | Rule | Points | Use when |
//! |---|---|---|
//! | [`QkRule::Qk15`] | 15 | smooth, well-behaved `f`; cheapest |
//! | [`QkRule::Qk21`] | 21 | **the default**; GSL's own recommendation for general use |
//! | [`QkRule::Qk31`], [`QkRule::Qk41`] | 31, 41 | moderately difficult integrands |
//! | [`QkRule::Qk51`], [`QkRule::Qk61`] | 51, 61 | **oscillatory** integrands — a high-order rule resolves wiggles a low-order one averages away |
//!
//! Higher is not automatically better: a high-order rule spends many
//! evaluations per sub-interval, which is wasted on a function that is smooth
//! apart from one awkward point. There, a low-order rule plus more bisection
//! wins.
//!
//! # What is NOT ported
//!
//! **QAGS** — the extrapolating variant — is not here. QAGS applies the epsilon
//! algorithm to the sequence of sub-interval results, which lets it handle
//! *integrable singularities* at the endpoints (`1/sqrt(x)` on `[0, 1]`).
//! [`qag`] will grind through its iteration limit on such an integrand and
//! report [`PetirError::MaxIterations`] rather than silently returning
//! something wrong — but it will not solve it. Nor are the infinite-range
//! transforms (`QAGI`), the weighted rules (`QAWO`, `QAWS`, `QAWC`), or the
//! non-adaptive `QNG`. All are tracked as beads.
//!
//! This is the honest boundary of the port, and worth reading before assuming
//! a hard integral will work.
//!
//! # Units
//!
//! Bare dimensionless `f64`.

/// Gauss-Legendre and Newton-Cotes quadrature, ported from the `peroxide`
/// crate. Complements the QUADPACK rules in this module rather than replacing
/// them — they are non-adaptive and give no error estimate, in exchange for
/// exactness on polynomials up to degree `2n - 1`.
pub mod gauss_legendre;
/// The Gauss-Legendre node and weight tables for orders 2 to 30, ported
/// verbatim from the `peroxide` crate. Data, not algorithm.
pub mod gauss_legendre_tables;

use alloc::vec;
use alloc::vec::Vec;

// Under a std-linked build (`cargo test`) f64's inherent abs shadows this
// trait, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

use crate::scalar::{DBL_EPSILON, DBL_MIN};
use crate::zip::zip_flat;
use crate::{PetirError, Result};

/// Which Gauss-Kronrod rule to apply.
///
/// The number is the count of Kronrod points; each wraps a Gauss rule of half
/// that (rounded down). See the module docs for how to choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QkRule {
    /// 15-point Kronrod over a 7-point Gauss rule. GSL `GSL_INTEG_GAUSS15`.
    Qk15,
    /// 21-point Kronrod over a 10-point Gauss rule. GSL `GSL_INTEG_GAUSS21`.
    /// The general-purpose default.
    Qk21,
    /// 31-point. GSL `GSL_INTEG_GAUSS31`.
    Qk31,
    /// 41-point. GSL `GSL_INTEG_GAUSS41`.
    Qk41,
    /// 51-point. GSL `GSL_INTEG_GAUSS51`.
    Qk51,
    /// 61-point. GSL `GSL_INTEG_GAUSS61`. Best for oscillatory integrands.
    Qk61,
}

impl QkRule {
    /// The `(xgk, wgk, wg)` tables for this rule.
    fn tables(self) -> (&'static [f64], &'static [f64], &'static [f64]) {
        match self {
            QkRule::Qk15 => (&XGK_15, &WGK_15, &WG_15),
            QkRule::Qk21 => (&XGK_21, &WGK_21, &WG_21),
            QkRule::Qk31 => (&XGK_31, &WGK_31, &WG_31),
            QkRule::Qk41 => (&XGK_41, &WGK_41, &WG_41),
            QkRule::Qk51 => (&XGK_51, &WGK_51, &WG_51),
            QkRule::Qk61 => (&XGK_61, &WGK_61, &WG_61),
        }
    }

    /// Number of Kronrod points the rule uses.
    pub fn points(self) -> usize {
        match self {
            QkRule::Qk15 => 15,
            QkRule::Qk21 => 21,
            QkRule::Qk31 => 31,
            QkRule::Qk41 => 41,
            QkRule::Qk51 => 51,
            QkRule::Qk61 => 61,
        }
    }
}

/// The result of one Gauss-Kronrod application.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QkResult {
    /// The Kronrod estimate of the integral.
    pub result: f64,
    /// Estimated absolute error, after QUADPACK's rescaling.
    pub abserr: f64,
    /// Integral of `|f|` over the interval — the scale against which
    /// round-off is judged.
    pub resabs: f64,
    /// Integral of `|f - mean(f)|` over the interval. Zero for a constant `f`,
    /// and QUADPACK uses `abserr == resasc` as the signal that the error
    /// estimate is not meaningful.
    pub resasc: f64,
}

/// QUADPACK's error rescaling — `integration/err.c`, `rescale_error`.
///
/// The raw `|kronrod - gauss|` difference is optimistic, so QUADPACK inflates
/// it by a power law in `resasc` and then floors it at the round-off level that
/// `resabs` implies. Both halves matter: without the first, adaptive
/// subdivision stops too early; without the second, it chases an error smaller
/// than the arithmetic can represent and never terminates.
fn rescale_error(mut err: f64, result_abs: f64, result_asc: f64) -> f64 {
    err = err.abs();

    if result_asc != 0.0 && err != 0.0 {
        let scale = (200.0 * err / result_asc).powf(1.5);
        if scale < 1.0 {
            err = result_asc * scale;
        } else {
            err = result_asc;
        }
    }

    if result_abs > DBL_MIN / (50.0 * DBL_EPSILON) {
        let min_err = 50.0 * DBL_EPSILON * result_abs;
        if min_err > err {
            err = min_err;
        }
    }

    err
}

/// Apply one Gauss-Kronrod rule over `[a, b]`.
///
/// This is a **single** application — no subdivision, no adaptivity. It is
/// exact for polynomials up to the rule's degree and merely approximate
/// otherwise; [`QkResult::abserr`] says how approximate.
///
/// Translates GSL's `gsl_integration_qk`.
///
/// # Example
///
/// ```
/// use petir::integration::{kronrod, QkRule};
/// // A 21-point rule integrates a cubic exactly.
/// let r = kronrod(QkRule::Qk21, |x: f64| x * x * x, 0.0, 2.0);
/// assert!((r.result - 4.0).abs() < 1e-13);
/// ```
pub fn kronrod<F>(rule: QkRule, f: F, a: f64, b: f64) -> QkResult
where
    F: Fn(f64) -> f64,
{
    let (xgk, wgk, wg) = rule.tables();
    let n = xgk.len();

    let center = 0.5 * (a + b);
    let half_length = 0.5 * (b - a);
    let abs_half_length = half_length.abs();
    let f_center = f(center);

    let mut fv1 = vec![0.0_f64; n];
    let mut fv2 = vec![0.0_f64; n];

    // Upstream's `wgk[n-1]` is the weight at the centre, and its `for (j = 0;
    // j < n - 1; j++)` loop walks everything BEFORE it -- so one `split_last`
    // supplies both, with no subscript and no separate bounds check.
    //
    // Every built-in rule has a non-empty table (pinned by
    // `the_tables_have_the_lengths_the_algorithm_assumes` below), so the
    // `None` arm is unreachable. It returns the integral of a rule with no
    // points, which is zero -- the right answer for that input rather than a
    // disguised failure.
    let Some((&wgk_centre, wgk_head)) = wgk.split_last() else {
        return QkResult {
            result: 0.0,
            abserr: 0.0,
            resabs: 0.0,
            resasc: 0.0,
        };
    };

    let mut result_gauss = 0.0_f64;
    let mut result_kronrod = f_center * wgk_centre;
    let mut result_abs = result_kronrod.abs();

    if n % 2 == 0 {
        // `wg[n/2 - 1]`. `split_last` above established `n >= 1`, and `n` is
        // even here, so `n >= 2` and the subtraction cannot wrap.
        if let Some(&w) = wg.get(n / 2 - 1) {
            result_gauss = f_center * w;
        }
    }

    // Points shared with the underlying Gauss rule: upstream's odd Kronrod
    // indices `jtw = 2j + 1`, which is what `skip(1).step_by(2)` walks. The
    // `take` reproduces upstream's `j < (n - 1) / 2` bound, which for the
    // even-length tables stops one short of the strided sequence.
    for (&x_j, &wgk_j, &wg_j, f1, f2) in zip_flat!(
        xgk.iter().skip(1).step_by(2),
        wgk.iter().skip(1).step_by(2),
        wg.iter(),
        fv1.iter_mut().skip(1).step_by(2),
        fv2.iter_mut().skip(1).step_by(2)
    )
    .take((n - 1) / 2)
    {
        let abscissa = half_length * x_j;
        let fval1 = f(center - abscissa);
        let fval2 = f(center + abscissa);
        let fsum = fval1 + fval2;
        *f1 = fval1;
        *f2 = fval2;
        result_gauss += wg_j * fsum;
        result_kronrod += wgk_j * fsum;
        result_abs += wgk_j * (fval1.abs() + fval2.abs());
    }

    // The points Kronrod adds: upstream's even indices `jtwm1 = 2j`.
    for (&x_j, &wgk_j, f1, f2) in zip_flat!(
        xgk.iter().step_by(2),
        wgk.iter().step_by(2),
        fv1.iter_mut().step_by(2),
        fv2.iter_mut().step_by(2)
    )
    .take(n / 2)
    {
        let abscissa = half_length * x_j;
        let fval1 = f(center - abscissa);
        let fval2 = f(center + abscissa);
        *f1 = fval1;
        *f2 = fval2;
        result_kronrod += wgk_j * (fval1 + fval2);
        result_abs += wgk_j * (fval1.abs() + fval2.abs());
    }

    let mean = result_kronrod * 0.5;
    let mut result_asc = wgk_centre * (f_center - mean).abs();
    // `wgk_head` is `wgk[0 .. n-1]`, so the zip runs exactly upstream's
    // `n - 1` times; `fv1`/`fv2` are longer and their last entry is never
    // written, exactly as upstream leaves it.
    for (&w, &a_j, &b_j) in zip_flat!(wgk_head.iter(), fv1.iter(), fv2.iter()) {
        result_asc += w * ((a_j - mean).abs() + (b_j - mean).abs());
    }

    // The Gauss/Kronrod difference IS the error estimate.
    let err = (result_kronrod - result_gauss) * half_length;

    let result_kronrod = result_kronrod * half_length;
    let result_abs = result_abs * abs_half_length;
    let result_asc = result_asc * abs_half_length;

    QkResult {
        result: result_kronrod,
        abserr: rescale_error(err, result_abs, result_asc),
        resabs: result_abs,
        resasc: result_asc,
    }
}

/// One sub-interval of the adaptive subdivision.
#[derive(Debug, Clone, Copy)]
struct Segment {
    a: f64,
    b: f64,
    result: f64,
    abserr: f64,
}

/// The outcome of an adaptive integration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Integral {
    /// The estimated integral.
    pub value: f64,
    /// Estimated absolute error.
    pub abserr: f64,
    /// How many sub-intervals the subdivision used.
    pub subintervals: usize,
}

/// Adaptive Gauss-Kronrod quadrature — GSL's `QAG`.
///
/// Applies `rule` over `[a, b]`, then repeatedly bisects whichever sub-interval
/// currently carries the largest error estimate, until the total error meets
/// `max(epsabs, epsrel * |result|)` or `limit` sub-intervals are used.
///
/// # Why bisect the WORST interval
///
/// Because that is where the error is. A function that is smooth everywhere
/// except near one point gets almost all of the subdivision spent near that
/// point, and almost none wasted on the smooth stretch — which is the whole
/// reason adaptive quadrature beats a fine uniform grid.
///
/// # Errors
///
/// - [`PetirError::Domain`] if `a` or `b` is not finite. Infinite ranges need
///   the `QAGI` transforms, which are not ported.
/// - [`PetirError::Tolerance`] if both tolerances are effectively zero — GSL
///   refuses `epsabs <= 0` together with `epsrel < 50 * eps`, because no
///   subdivision can meet it.
/// - [`PetirError::RoundOff`] if the first application's error is already at
///   the round-off floor yet still above the requested tolerance. Subdividing
///   would not help; the tolerance is unattainable in double precision.
/// - [`PetirError::MaxIterations`] if `limit` sub-intervals are exhausted.
///   **This is the usual symptom of a singularity** that QAGS would handle and
///   this routine cannot — see the module docs.
///
/// # Example
///
/// ```
/// use petir::integration::{qag, QkRule};
/// // integral of exp(x) from 0 to 1 is e - 1.
/// let r = qag(QkRule::Qk21, |x: f64| x.exp(), 0.0, 1.0, 0.0, 1e-10, 50).unwrap();
/// assert!((r.value - (core::f64::consts::E - 1.0)).abs() < 1e-12);
/// ```
pub fn qag<F>(
    rule: QkRule,
    f: F,
    a: f64,
    b: f64,
    epsabs: f64,
    epsrel: f64,
    limit: usize,
) -> Result<Integral>
where
    F: Fn(f64) -> f64,
{
    if !a.is_finite() || !b.is_finite() {
        return Err(PetirError::Domain);
    }
    if limit == 0 {
        return Err(PetirError::Invalid);
    }
    // GSL: "tolerance cannot be achieved with given epsabs and epsrel".
    if epsabs <= 0.0 && (epsrel < 50.0 * DBL_EPSILON || epsrel < 0.5e-28) {
        return Err(PetirError::Tolerance);
    }

    let first = kronrod(rule, &f, a, b);
    let mut tolerance = epsabs.max(epsrel * first.result.abs());
    let round_off = 50.0 * DBL_EPSILON * first.resabs;

    if first.abserr <= round_off && first.abserr > tolerance {
        // Subdividing cannot help: the error is already round-off.
        return Err(PetirError::RoundOff);
    }
    if (first.abserr <= tolerance && first.abserr != first.resasc) || first.abserr == 0.0 {
        return Ok(Integral {
            value: first.result,
            abserr: first.abserr,
            subintervals: 1,
        });
    }
    if limit == 1 {
        return Err(PetirError::MaxIterations);
    }

    let mut segments: Vec<Segment> = vec![Segment {
        a,
        b,
        result: first.result,
        abserr: first.abserr,
    }];
    let mut area = first.result;
    let mut errsum = first.abserr;

    while segments.len() < limit {
        // Bisect the sub-interval carrying the largest error.
        let mut worst = 0usize;
        let mut worst_abserr = f64::NEG_INFINITY;
        for (i, s) in segments.iter().enumerate() {
            if s.abserr > worst_abserr {
                worst_abserr = s.abserr;
                worst = i;
            }
        }
        // `segments` is non-empty on every iteration (it is seeded with one
        // segment and only ever grows), so this always resolves.
        let Some(&seg) = segments.get(worst) else {
            return Err(PetirError::Invalid);
        };

        let mid = 0.5 * (seg.a + seg.b);
        // A sub-interval too small to bisect meaningfully: further subdivision
        // would only accumulate round-off.
        if mid <= seg.a || mid >= seg.b {
            return Err(PetirError::RoundOff);
        }

        let left = kronrod(rule, &f, seg.a, mid);
        let right = kronrod(rule, &f, mid, seg.b);

        area += (left.result + right.result) - seg.result;
        errsum += (left.abserr + right.abserr) - seg.abserr;

        let Some(slot) = segments.get_mut(worst) else {
            return Err(PetirError::Invalid);
        };
        *slot = Segment {
            a: seg.a,
            b: mid,
            result: left.result,
            abserr: left.abserr,
        };
        segments.push(Segment {
            a: mid,
            b: seg.b,
            result: right.result,
            abserr: right.abserr,
        });

        tolerance = epsabs.max(epsrel * area.abs());
        if errsum <= tolerance {
            return Ok(Integral {
                value: area,
                abserr: errsum,
                subintervals: segments.len(),
            });
        }
    }

    Err(PetirError::MaxIterations)
}

/// GSL `qk15.c` `xgk` (8 values) — extracted mechanically from the
/// vendored source, never retyped.
const XGK_15: [f64; 8] = [
    0.991455371120812639206854697526329,
    0.949107912342758524526189684047851,
    0.864864423359769072789712788640926,
    0.741531185599394439863864773280788,
    0.586087235467691130294144838258730,
    0.405845151377397166906606412076961,
    0.207784955007898467600689403773245,
    0.000000000000000000000000000000000,
];

/// GSL `qk15.c` `wgk` (8 values) — extracted mechanically from the
/// vendored source, never retyped.
const WGK_15: [f64; 8] = [
    0.022935322010529224963732008058970,
    0.063092092629978553290700663189204,
    0.104790010322250183839876322541518,
    0.140653259715525918745189590510238,
    0.169004726639267902826583426598550,
    0.190350578064785409913256402421014,
    0.204432940075298892414161999234649,
    0.209482141084727828012999174891714,
];

/// GSL `qk15.c` `wg` (4 values) — extracted mechanically from the
/// vendored source, never retyped.
const WG_15: [f64; 4] = [
    0.129484966168869693270611432679082,
    0.279705391489276667901467771423780,
    0.381830050505118944950369775488975,
    0.417959183673469387755102040816327,
];

/// GSL `qk21.c` `xgk` (11 values) — extracted mechanically from the
/// vendored source, never retyped.
const XGK_21: [f64; 11] = [
    0.995657163025808080735527280689003,
    0.973906528517171720077964012084452,
    0.930157491355708226001207180059508,
    0.865063366688984510732096688423493,
    0.780817726586416897063717578345042,
    0.679409568299024406234327365114874,
    0.562757134668604683339000099272694,
    0.433395394129247190799265943165784,
    0.294392862701460198131126603103866,
    0.148874338981631210884826001129720,
    0.000000000000000000000000000000000,
];

/// GSL `qk21.c` `wgk` (11 values) — extracted mechanically from the
/// vendored source, never retyped.
const WGK_21: [f64; 11] = [
    0.011694638867371874278064396062192,
    0.032558162307964727478818972459390,
    0.054755896574351996031381300244580,
    0.075039674810919952767043140916190,
    0.093125454583697605535065465083366,
    0.109387158802297641899210590325805,
    0.123491976262065851077958109831074,
    0.134709217311473325928054001771707,
    0.142775938577060080797094273138717,
    0.147739104901338491374841515972068,
    0.149445554002916905664936468389821,
];

/// GSL `qk21.c` `wg` (5 values) — extracted mechanically from the
/// vendored source, never retyped.
const WG_21: [f64; 5] = [
    0.066671344308688137593568809893332,
    0.149451349150580593145776339657697,
    0.219086362515982043995534934228163,
    0.269266719309996355091226921569469,
    0.295524224714752870173892994651338,
];

/// GSL `qk31.c` `xgk` (16 values) — extracted mechanically from the
/// vendored source, never retyped.
const XGK_31: [f64; 16] = [
    0.998002298693397060285172840152271,
    0.987992518020485428489565718586613,
    0.967739075679139134257347978784337,
    0.937273392400705904307758947710209,
    0.897264532344081900882509656454496,
    0.848206583410427216200648320774217,
    0.790418501442465932967649294817947,
    0.724417731360170047416186054613938,
    0.650996741297416970533735895313275,
    0.570972172608538847537226737253911,
    0.485081863640239680693655740232351,
    0.394151347077563369897207370981045,
    0.299180007153168812166780024266389,
    0.201194093997434522300628303394596,
    0.101142066918717499027074231447392,
    0.000000000000000000000000000000000,
];

/// GSL `qk31.c` `wgk` (16 values) — extracted mechanically from the
/// vendored source, never retyped.
const WGK_31: [f64; 16] = [
    0.005377479872923348987792051430128,
    0.015007947329316122538374763075807,
    0.025460847326715320186874001019653,
    0.035346360791375846222037948478360,
    0.044589751324764876608227299373280,
    0.053481524690928087265343147239430,
    0.062009567800670640285139230960803,
    0.069854121318728258709520077099147,
    0.076849680757720378894432777482659,
    0.083080502823133021038289247286104,
    0.088564443056211770647275443693774,
    0.093126598170825321225486872747346,
    0.096642726983623678505179907627589,
    0.099173598721791959332393173484603,
    0.100769845523875595044946662617570,
    0.101330007014791549017374792767493,
];

/// GSL `qk31.c` `wg` (8 values) — extracted mechanically from the
/// vendored source, never retyped.
const WG_31: [f64; 8] = [
    0.030753241996117268354628393577204,
    0.070366047488108124709267416450667,
    0.107159220467171935011869546685869,
    0.139570677926154314447804794511028,
    0.166269205816993933553200860481209,
    0.186161000015562211026800561866423,
    0.198431485327111576456118326443839,
    0.202578241925561272880620199967519,
];

/// GSL `qk41.c` `xgk` (21 values) — extracted mechanically from the
/// vendored source, never retyped.
const XGK_41: [f64; 21] = [
    0.998859031588277663838315576545863,
    0.993128599185094924786122388471320,
    0.981507877450250259193342994720217,
    0.963971927277913791267666131197277,
    0.940822633831754753519982722212443,
    0.912234428251325905867752441203298,
    0.878276811252281976077442995113078,
    0.839116971822218823394529061701521,
    0.795041428837551198350638833272788,
    0.746331906460150792614305070355642,
    0.693237656334751384805490711845932,
    0.636053680726515025452836696226286,
    0.575140446819710315342946036586425,
    0.510867001950827098004364050955251,
    0.443593175238725103199992213492640,
    0.373706088715419560672548177024927,
    0.301627868114913004320555356858592,
    0.227785851141645078080496195368575,
    0.152605465240922675505220241022678,
    0.076526521133497333754640409398838,
    0.000000000000000000000000000000000,
];

/// GSL `qk41.c` `wgk` (21 values) — extracted mechanically from the
/// vendored source, never retyped.
const WGK_41: [f64; 21] = [
    0.003073583718520531501218293246031,
    0.008600269855642942198661787950102,
    0.014626169256971252983787960308868,
    0.020388373461266523598010231432755,
    0.025882133604951158834505067096153,
    0.031287306777032798958543119323801,
    0.036600169758200798030557240707211,
    0.041668873327973686263788305936895,
    0.046434821867497674720231880926108,
    0.050944573923728691932707670050345,
    0.055195105348285994744832372419777,
    0.059111400880639572374967220648594,
    0.062653237554781168025870122174255,
    0.065834597133618422111563556969398,
    0.068648672928521619345623411885368,
    0.071054423553444068305790361723210,
    0.073030690332786667495189417658913,
    0.074582875400499188986581418362488,
    0.075704497684556674659542775376617,
    0.076377867672080736705502835038061,
    0.076600711917999656445049901530102,
];

/// GSL `qk41.c` `wg` (10 values) — extracted mechanically from the
/// vendored source, never retyped.
const WG_41: [f64; 10] = [
    0.017614007139152118311861962351853,
    0.040601429800386941331039952274932,
    0.062672048334109063569506535187042,
    0.083276741576704748724758143222046,
    0.101930119817240435036750135480350,
    0.118194531961518417312377377711382,
    0.131688638449176626898494499748163,
    0.142096109318382051329298325067165,
    0.149172986472603746787828737001969,
    0.152753387130725850698084331955098,
];

/// GSL `qk51.c` `xgk` (26 values) — extracted mechanically from the
/// vendored source, never retyped.
const XGK_51: [f64; 26] = [
    0.999262104992609834193457486540341,
    0.995556969790498097908784946893902,
    0.988035794534077247637331014577406,
    0.976663921459517511498315386479594,
    0.961614986425842512418130033660167,
    0.942974571228974339414011169658471,
    0.920747115281701561746346084546331,
    0.894991997878275368851042006782805,
    0.865847065293275595448996969588340,
    0.833442628760834001421021108693570,
    0.797873797998500059410410904994307,
    0.759259263037357630577282865204361,
    0.717766406813084388186654079773298,
    0.673566368473468364485120633247622,
    0.626810099010317412788122681624518,
    0.577662930241222967723689841612654,
    0.526325284334719182599623778158010,
    0.473002731445714960522182115009192,
    0.417885382193037748851814394594572,
    0.361172305809387837735821730127641,
    0.303089538931107830167478909980339,
    0.243866883720988432045190362797452,
    0.183718939421048892015969888759528,
    0.122864692610710396387359818808037,
    0.061544483005685078886546392366797,
    0.000000000000000000000000000000000,
];

/// GSL `qk51.c` `wgk` (26 values) — extracted mechanically from the
/// vendored source, never retyped.
const WGK_51: [f64; 26] = [
    0.001987383892330315926507851882843,
    0.005561932135356713758040236901066,
    0.009473973386174151607207710523655,
    0.013236229195571674813656405846976,
    0.016847817709128298231516667536336,
    0.020435371145882835456568292235939,
    0.024009945606953216220092489164881,
    0.027475317587851737802948455517811,
    0.030792300167387488891109020215229,
    0.034002130274329337836748795229551,
    0.037116271483415543560330625367620,
    0.040083825504032382074839284467076,
    0.042872845020170049476895792439495,
    0.045502913049921788909870584752660,
    0.047982537138836713906392255756915,
    0.050277679080715671963325259433440,
    0.052362885806407475864366712137873,
    0.054251129888545490144543370459876,
    0.055950811220412317308240686382747,
    0.057437116361567832853582693939506,
    0.058689680022394207961974175856788,
    0.059720340324174059979099291932562,
    0.060539455376045862945360267517565,
    0.061128509717053048305859030416293,
    0.061471189871425316661544131965264,
    0.061580818067832935078759824240066,
];

/// GSL `qk51.c` `wg` (13 values) — extracted mechanically from the
/// vendored source, never retyped.
const WG_51: [f64; 13] = [
    0.011393798501026287947902964113235,
    0.026354986615032137261901815295299,
    0.040939156701306312655623487711646,
    0.054904695975835191925936891540473,
    0.068038333812356917207187185656708,
    0.080140700335001018013234959669111,
    0.091028261982963649811497220702892,
    0.100535949067050644202206890392686,
    0.108519624474263653116093957050117,
    0.114858259145711648339325545869556,
    0.119455763535784772228178126512901,
    0.122242442990310041688959518945852,
    0.123176053726715451203902873079050,
];

/// GSL `qk61.c` `xgk` (31 values) — extracted mechanically from the
/// vendored source, never retyped.
const XGK_61: [f64; 31] = [
    0.999484410050490637571325895705811,
    0.996893484074649540271630050918695,
    0.991630996870404594858628366109486,
    0.983668123279747209970032581605663,
    0.973116322501126268374693868423707,
    0.960021864968307512216871025581798,
    0.944374444748559979415831324037439,
    0.926200047429274325879324277080474,
    0.905573307699907798546522558925958,
    0.882560535792052681543116462530226,
    0.857205233546061098958658510658944,
    0.829565762382768397442898119732502,
    0.799727835821839083013668942322683,
    0.767777432104826194917977340974503,
    0.733790062453226804726171131369528,
    0.697850494793315796932292388026640,
    0.660061064126626961370053668149271,
    0.620526182989242861140477556431189,
    0.579345235826361691756024932172540,
    0.536624148142019899264169793311073,
    0.492480467861778574993693061207709,
    0.447033769538089176780609900322854,
    0.400401254830394392535476211542661,
    0.352704725530878113471037207089374,
    0.304073202273625077372677107199257,
    0.254636926167889846439805129817805,
    0.204525116682309891438957671002025,
    0.153869913608583546963794672743256,
    0.102806937966737030147096751318001,
    0.051471842555317695833025213166723,
    0.000000000000000000000000000000000,
];

/// GSL `qk61.c` `wgk` (31 values) — extracted mechanically from the
/// vendored source, never retyped.
const WGK_61: [f64; 31] = [
    0.001389013698677007624551591226760,
    0.003890461127099884051267201844516,
    0.006630703915931292173319826369750,
    0.009273279659517763428441146892024,
    0.011823015253496341742232898853251,
    0.014369729507045804812451432443580,
    0.016920889189053272627572289420322,
    0.019414141193942381173408951050128,
    0.021828035821609192297167485738339,
    0.024191162078080601365686370725232,
    0.026509954882333101610601709335075,
    0.028754048765041292843978785354334,
    0.030907257562387762472884252943092,
    0.032981447057483726031814191016854,
    0.034979338028060024137499670731468,
    0.036882364651821229223911065617136,
    0.038678945624727592950348651532281,
    0.040374538951535959111995279752468,
    0.041969810215164246147147541285970,
    0.043452539701356069316831728117073,
    0.044814800133162663192355551616723,
    0.046059238271006988116271735559374,
    0.047185546569299153945261478181099,
    0.048185861757087129140779492298305,
    0.049055434555029778887528165367238,
    0.049795683427074206357811569379942,
    0.050405921402782346840893085653585,
    0.050881795898749606492297473049805,
    0.051221547849258772170656282604944,
    0.051426128537459025933862879215781,
    0.051494729429451567558340433647099,
];

/// GSL `qk61.c` `wg` (15 values) — extracted mechanically from the
/// vendored source, never retyped.
const WG_61: [f64; 15] = [
    0.007968192496166605615465883474674,
    0.018466468311090959142302131912047,
    0.028784707883323369349719179611292,
    0.038799192569627049596801936446348,
    0.048402672830594052902938140422808,
    0.057493156217619066481721689402056,
    0.065974229882180495128128515115962,
    0.073755974737705206268243850022191,
    0.080755895229420215354694938460530,
    0.086899787201082979802387530715126,
    0.092122522237786128717632707087619,
    0.096368737174644259639468626351810,
    0.099593420586795267062780282103569,
    0.101762389748405504596428952168554,
    0.102852652893558840341285636705415,
];

#[cfg(test)]
mod tests {
    use super::*;

    /// The three tables of each rule must stand in the relationship
    /// [`kronrod`] assumes, because two of that routine's branches are
    /// unreachable only if they do.
    ///
    /// # Methodology
    ///
    /// For each of the six rules, check that
    ///
    /// - `wgk` has the same length `n` as `xgk` — `wgk_centre` is taken by
    ///   `split_last` on `wgk` and paired with `xgk`-strided values;
    /// - `wgk` is **non-empty**, which is what makes the "rule with no points"
    ///   arm of `kronrod` unreachable;
    /// - `wg` is long enough to hold `n / 2` entries, so the
    ///   `wg.get(n / 2 - 1)` read for an even-length table always resolves;
    /// - `wg` has exactly `n / 2` entries (integer division), the count
    ///   QUADPACK's Gauss half-rule carries. This is also what makes the
    ///   shared-point loop's `take((n - 1) / 2)` meaningful: for odd `n` the
    ///   two agree and `wg` is the binding input, while for even `n` the
    ///   strided `xgk` walk runs one step longer than upstream's bound and the
    ///   `take` is what stops it.
    ///
    /// # Results
    ///
    /// Passes for all six rules as of 2026-09-15, on the tables extracted from
    /// GSL 2.8 `qk15.c` … `qk61.c`:
    /// `(n, wg.len())` = (8, 4), (11, 5), (16, 8), (21, 10), (26, 13), (31, 15)
    /// — `wg.len() == n / 2` in every case.
    /// Interpretation: the two `else`/`if let` arms in [`kronrod`] that exist
    /// to avoid a subscript are dead code for every rule the enum can name,
    /// which is what lets the routine stay infallible.
    #[test]
    fn the_tables_have_the_lengths_the_algorithm_assumes() {
        for rule in [
            QkRule::Qk15,
            QkRule::Qk21,
            QkRule::Qk31,
            QkRule::Qk41,
            QkRule::Qk51,
            QkRule::Qk61,
        ] {
            let (xgk, wgk, wg) = rule.tables();
            let n = xgk.len();
            assert!(!wgk.is_empty(), "{rule:?}: wgk must be non-empty");
            assert_eq!(wgk.len(), n, "{rule:?}: wgk and xgk must agree in length");
            assert!(
                wg.len() >= n / 2,
                "{rule:?}: wg must hold at least n/2 = {} entries, has {}",
                n / 2,
                wg.len()
            );
            assert_eq!(wg.len(), n / 2, "{rule:?}: wg must hold n/2 entries");
            // The rule's advertised point count is 2n - 1: n Kronrod abscissae
            // mirrored about the centre, which is shared.
            assert_eq!(rule.points(), 2 * n - 1, "{rule:?}: point count");
        }
    }
}
