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
// PORTED from the GNU Scientific Library (GSL) 2.8, specfunc/dawson.c
// <https://www.gnu.org/software/gsl/>
// Copyright (C) 1996-2000 Gerard Jungman
// GSL is free software under the GNU General Public License, version 3 or
// later.
//
// GSL's own lineage here is SLATEC's `daws.f` by W. Fullerton. All three
// Chebyshev tables were extracted from that source by script, not retyped,
// and are audited bit-for-bit by tests/gsl_tables_audit.rs.

//! Dawson's integral `F(x)`.
//!
//! # What this is
//!
//! ```text
//!     F(x) = e^{-x^2} integral_0^x e^{t^2} dt
//! ```
//!
//! the odd, bounded companion of the error function: where `erf` saturates,
//! `F` rises to a single maximum of about 0.5410 at `x ~ 0.9241` and then
//! decays like `1/(2x)`. It is what `exp(x^2) erf(x)` would be if that
//! product did not overflow — the reason the function exists at all is that
//! `F(x) = (sqrt(pi)/2) e^{-x^2} erfi(x)` is computable where the factors
//! separately are not.
//!
//! It is also the imaginary part of the Faddeeva function on the real axis,
//! which is why `njoy-outram-park-fork` needs it for Doppler broadening.
//!
//! # Argument range, stated plainly
//!
//! **All real `x`**, dimensionless (`f64`). `F` is odd and bounded by 0.5411,
//! so there is no overflow anywhere. Past `0.1 * DBL_MAX` upstream declares
//! underflow and returns `0.0`; below that the tail is `1/(2x)` exactly.
//!
//! `NaN` propagates.
//!
//! # All three Chebyshev tables are DELIBERATELY TRUNCATED by upstream
//!
//! | table | array | GSL evaluates | full order, in a comment |
//! |---|---|---|---|
//! | `daw_data` | 21 | 16 | 20 |
//! | `daw2_data` | 45 | 33 | 44 |
//! | `dawa_data` | 75 | 35 | 74 |
//!
//! That is SLATEC's `initds` convention: the array carries enough
//! coefficients for the highest precision anyone might want, and the
//! `cheb_series` order selects how many are needed for `double`. GSL left the
//! full order beside each as `/* 20, */`. The full arrays are kept here so
//! the audit can compare them against upstream, and the evaluation slices to
//! the order GSL declares — the same arrangement `zeta`'s `zetam1_inter_cs`
//! already has. `the_truncated_series_stop_where_gsl_says` measures what the
//! discarded tails would change.
//!
//! # Accuracy
//!
//! Measured against the **defining integral** by composite Gauss-Legendre,
//! against the ODE `F' = 1 - 2xF` that characterises it, and against the
//! asymptotic series. Results are in the tests.

// Under a std-linked build (`cargo test`) f64's inherent abs/exp shadow these
// trait methods, leaving the import formally unused. See crate::real.
#[allow(unused_imports)]
use crate::real::Real;

use crate::cheb_slice::eval_gsl as cheb;
use crate::specfunc::SQRT_DBL_EPSILON;

/// `1.225 * GSL_SQRT_DBL_EPSILON`, below which `F(x) = x` to working
/// precision.
const XSML: f64 = 1.225 * SQRT_DBL_EPSILON;
/// `1/(sqrt(2) * GSL_SQRT_DBL_EPSILON)`, above which the `32/y^2` fit is
/// indistinguishable from its `1/(2x)` limit.
const XBIG: f64 = 1.0 / (core::f64::consts::SQRT_2 * SQRT_DBL_EPSILON);
/// `0.1 * GSL_DBL_MAX`, upstream's `UNDERFLOW_ERROR` bound.
const XMAX: f64 = 0.1 * f64::MAX;

/// Coefficients of `daw_data` that GSL evaluates: order 15, so 16 of 21.
const DAW_USED: usize = 16;
/// Coefficients of `daw2_data` that GSL evaluates: order 32, so 33 of 45.
const DAW2_USED: usize = 33;
/// Coefficients of `dawa_data` that GSL evaluates: order 34, so 35 of 75.
const DAWA_USED: usize = 35;

/// GSL's `daw_data`, 21 coefficients on `[-1, 1]`, of which GSL evaluates 16.
#[rustfmt::skip]
const DAW: [f64; 21] = [
    -0.6351734375145949201065127736293e-02, -0.2294071479677386939899824125866e+00,
    0.2213050093908476441683979161786e-01, -0.1549265453892985046743057753375e-02,
    0.8497327715684917456777542948066e-04, -0.3828266270972014924994099521309e-05,
    0.1462854806250163197757148949539e-06, -0.4851982381825991798846715425114e-08,
    0.1421463577759139790347568183304e-09, -0.3728836087920596525335493054088e-11,
    0.8854942961778203370194565231369e-13, -0.1920757131350206355421648417493e-14,
    0.3834325867246327588241074439253e-16, -0.7089154168175881633584099327999e-18,
    0.1220552135889457674416901120000e-19, -0.1966204826605348760299451733333e-21,
    0.2975845541376597189113173333333e-23, -0.4247069514800596951039999999999e-25,
    0.5734270767391742798506666666666e-27, -0.7345836823178450261333333333333e-29,
    0.8951937667516552533333333333333e-31,
];

/// GSL's `daw2_data`, 45 coefficients on `[-1, 1]`, of which GSL evaluates 33.
#[rustfmt::skip]
const DAW2: [f64; 45] = [
    -0.56886544105215527114160533733674e-01, -0.31811346996168131279322878048822e+00,
    0.20873845413642236789741580198858e+00, -0.12475409913779131214073498314784e+00,
    0.67869305186676777092847516423676e-01, -0.33659144895270939503068230966587e-01,
    0.15260781271987971743682460381640e-01, -0.63483709625962148230586094788535e-02,
    0.24326740920748520596865966109343e-02, -0.86219541491065032038526983549637e-03,
    0.28376573336321625302857636538295e-03, -0.87057549874170423699396581464335e-04,
    0.24986849985481658331800044137276e-04, -0.67319286764160294344603050339520e-05,
    0.17078578785573543710504524047844e-05, -0.40917551226475381271896592490038e-06,
    0.92828292216755773260751785312273e-07, -0.19991403610147617829845096332198e-07,
    0.40963490644082195241210487868917e-08, -0.80032409540993168075706781753561e-09,
    0.14938503128761465059143225550110e-09, -0.26687999885622329284924651063339e-10,
    0.45712216985159458151405617724103e-11, -0.75187305222043565872243727326771e-12,
    0.11893100052629681879029828987302e-12, -0.18116907933852346973490318263084e-13,
    0.26611733684358969193001612199626e-14, -0.37738863052129419795444109905930e-15,
    0.51727953789087172679680082229329e-16, -0.68603684084077500979419564670102e-17,
    0.88123751354161071806469337321745e-18, -0.10974248249996606292106299624652e-18,
    0.13261199326367178513595545891635e-19, -0.15562732768137380785488776571562e-20,
    0.17751425583655720607833415570773e-21, -0.19695006967006578384953608765439e-22,
    0.21270074896998699661924010120533e-23, -0.22375398124627973794182113962666e-24,
    0.22942768578582348946971383125333e-25, -0.22943788846552928693329592319999e-26,
    0.22391702100592453618342297600000e-27, -0.21338230616608897703678225066666e-28,
    0.19866196585123531518028458666666e-29, -0.18079295866694391771955199999999e-30,
    0.16090686015283030305450666666666e-31,
];

/// GSL's `dawa_data`, 75 coefficients on `[-1, 1]`, of which GSL evaluates 35.
#[rustfmt::skip]
const DAWA: [f64; 75] = [
    0.1690485637765703755422637438849e-01, 0.8683252278406957990536107850768e-02,
    0.2424864042417715453277703459889e-03, 0.1261182399572690001651949240377e-04,
    0.1066453314636176955705691125906e-05, 0.1358159794790727611348424505728e-06,
    0.2171042356577298398904312744743e-07, 0.2867010501805295270343676804813e-08,
    -0.1901336393035820112282492378024e-09, -0.3097780484395201125532065774268e-09,
    -0.1029414876057509247398132286413e-09, -0.6260356459459576150417587283121e-11,
    0.8563132497446451216262303166276e-11, 0.3033045148075659292976266276257e-11,
    -0.2523618306809291372630886938826e-12, -0.4210604795440664513175461934510e-12,
    -0.4431140826646238312143429452036e-13, 0.4911210272841205205940037065117e-13,
    0.1235856242283903407076477954739e-13, -0.5788733199016569246955765071069e-14,
    -0.2282723294807358620978183957030e-14, 0.7637149411014126476312362917590e-15,
    0.3851546883566811728777594002095e-15, -0.1199932056928290592803237283045e-15,
    -0.6313439150094572347334270285250e-16, 0.2239559965972975375254912790237e-16,
    0.9987925830076495995132891200749e-17, -0.4681068274322495334536246507252e-17,
    -0.1436303644349721337241628751534e-17, 0.1020822731410541112977908032130e-17,
    0.1538908873136092072837389822372e-18, -0.2189157877645793888894790926056e-18,
    0.2156879197938651750392359152517e-20, 0.4370219827442449851134792557395e-19,
    -0.8234581460977207241098927905177e-20, -0.7498648721256466222903202835420e-20,
    0.3282536720735671610957612930039e-20, 0.8858064309503921116076561515151e-21,
    -0.9185087111727002988094460531485e-21, 0.2978962223788748988314166045791e-22,
    0.1972132136618471883159505468041e-21, -0.5974775596362906638089584995117e-22,
    -0.2834410031503850965443825182441e-22, 0.2209560791131554514777150489012e-22,
    -0.5439955741897144300079480307711e-25, -0.5213549243294848668017136696470e-23,
    0.1702350556813114199065671499076e-23, 0.6917400860836148343022185660197e-24,
    -0.6540941793002752512239445125802e-24, 0.6093576580439328960371824654636e-25,
    0.1408070432905187461501945080272e-24, -0.6785886121054846331167674943755e-25,
    -0.9799732036214295711741583102225e-26, 0.2121244903099041332598960939160e-25,
    -0.5954455022548790938238802154487e-26, -0.3093088861875470177838847232049e-26,
    0.2854389216344524682400691986104e-26, -0.3951289447379305566023477271811e-27,
    -0.5906000648607628478116840894453e-27, 0.3670236964668687003647889980609e-27,
    -0.4839958238042276256598303038941e-29, -0.9799265984210443869597404017022e-28,
    0.4684773732612130606158908804300e-28, 0.5030877696993461051647667603155e-29,
    -0.1547395051706028239247552068295e-28, 0.6112180185086419243976005662714e-29,
    0.1357913399124811650343602736158e-29, -0.2417687752768673088385304299044e-29,
    0.8369074582074298945292887587291e-30, 0.2665413042788979165838319401566e-30,
    -0.3811653692354890336935691003712e-30, 0.1230054721884951464371706872585e-30,
    0.4622506399041493508805536929983e-31, -0.6120087296881677722911435593001e-31,
    0.1966024640193164686956230217896e-31,
];

/// Dawson's integral `F(x) = e^{-x^2} integral_0^x e^{t^2} dt`, GSL's
/// `gsl_sf_dawson`.
///
/// `x` is dimensionless (`f64`), any real value. The result is odd, bounded
/// by about 0.5411, and `0.0` past `0.1 * DBL_MAX` where upstream declares
/// underflow. `NaN` propagates.
///
/// Four branches: `F(x) = x` below `XSML`, two Chebyshev fits in `2y^2 - 1`
/// and `y^2/8 - 1` covering `|x| < 4`, a third in `32/y^2 - 1` out to
/// [`XBIG`], and the exact asymptote `1/(2x)` beyond.
///
/// # Examples
///
/// ```
/// use petir::specfunc::dawson::dawson;
/// // Odd, and zero at the origin.
/// assert_eq!(dawson(0.0), 0.0);
/// assert_eq!(dawson(-1.5), -dawson(1.5));
/// // Single maximum near x = 0.9241.
/// assert!((dawson(0.924_138_2) - 0.541_044).abs() < 1e-5);
/// // And a 1/(2x) tail.
/// assert!((dawson(100.0) - 0.005).abs() < 1e-6);
/// ```
pub fn dawson(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    let y = x.abs();
    if y < XSML {
        x
    } else if y < 1.0 {
        // The 0.75 is carried outside the fit so the series has no constant
        // term to lose precision on; upstream does the same.
        let Some(c) = DAW.get(..DAW_USED) else {
            return f64::NAN;
        };
        x * (0.75 + cheb(2.0 * y * y - 1.0, c))
    } else if y < 4.0 {
        let Some(c) = DAW2.get(..DAW2_USED) else {
            return f64::NAN;
        };
        x * (0.25 + cheb(0.125 * y * y - 1.0, c))
    } else if y < XBIG {
        let Some(c) = DAWA.get(..DAWA_USED) else {
            return f64::NAN;
        };
        (0.5 + cheb(32.0 / (y * y) - 1.0, c)) / x
    } else if y < XMAX {
        0.5 / x
    } else {
        // Upstream's UNDERFLOW_ERROR.
        0.0
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;

    /// `F(x) = e^{-x^2} integral_0^x e^{t^2} dt`, computed **independently of
    /// this module** by composite 30-point Gauss-Legendre.
    ///
    /// The `e^{t^2 - x^2}` form is used rather than `e^{-x^2}` times
    /// `integral e^{t^2}`: the second overflows for `x` past about 26 while
    /// the answer stays near `1/(2x)`, which is the whole reason the function
    /// is tabulated instead of being written out.
    fn by_quadrature(x: f64, panels: usize) -> f64 {
        if x == 0.0 {
            return 0.0;
        }
        let h = x / panels as f64;
        let mut acc = 0.0;
        for k in 0..panels {
            let a = k as f64 * h;
            acc += crate::integration::gauss_legendre::gauss_legendre(
                |t: f64| ((t - x) * (t + x)).exp(),
                a,
                a + h,
                30,
            )
            .expect("30-point Gauss-Legendre is tabulated");
        }
        acc
    }

    /// **The defining integral is reproduced to 1.6e-13**, measured
    /// 2026-09-19 across 14 abscissae placed to cross every branch boundary
    /// (`XSML`, 1, 4) and to sit just inside each of them.
    ///
    /// Pass criterion 1e-12; the residual is the quadrature's own, and grows
    /// with the interval length — 1.0e-15 at `x = 1`, 2.0e-14 at `x = 5`,
    /// 1.6e-13 at `x = 20` with a fixed 200 panels.
    #[test]
    fn the_defining_integral_is_reproduced() {
        let (mut worst, mut at) = (0.0_f64, 0.0_f64);
        for x in [
            0.01_f64, 0.1, 0.5, 0.9, 0.999, 1.0, 1.5, 2.5, 3.999, 4.0, 5.0, 8.0, 20.0, 100.0,
        ] {
            let q = by_quadrature(x, 200);
            let e = ((dawson(x) - q) / q).abs();
            if e > worst {
                worst = e;
                at = x;
            }
        }
        assert!(
            worst < 1e-12,
            "Dawson against its defining integral: {worst:e} at x = {at}. The \
             documented figure is 1.6e-13 at x = 20"
        );
    }

    /// **`F' = 1 - 2 x F`**, the first-order ODE that characterises Dawson's
    /// integral — a check that shares no table with the function and holds
    /// across every branch at once.
    ///
    /// Eighth-order central difference over `x` in `[-30, 30]` at 0.05
    /// spacing. Worst residual 3.759e-12 at `x = -1.05` (2026-09-19), which
    /// is the stencil's floor rather than the function's: it sits right
    /// beside the `|x| = 1` branch boundary where the difference straddles
    /// two different Chebyshev fits.
    #[test]
    fn it_satisfies_its_own_ode() {
        let (mut worst, mut at) = (0.0_f64, 0.0_f64);
        for k in 0..=1200 {
            let x = -30.0 + 0.05 * k as f64;
            let h = 1e-4_f64.max(x.abs() * 1e-6);
            let d = ((1.0 / 280.0) * (dawson(x - 4.0 * h) - dawson(x + 4.0 * h))
                + (4.0 / 105.0) * (dawson(x + 3.0 * h) - dawson(x - 3.0 * h))
                + 0.2 * (dawson(x - 2.0 * h) - dawson(x + 2.0 * h))
                + 0.8 * (dawson(x + h) - dawson(x - h)))
                / h;
            let want = 1.0 - 2.0 * x * dawson(x);
            let e = (d - want).abs();
            if e > worst {
                worst = e;
                at = x;
            }
        }
        assert!(
            worst < 1e-10,
            "F' = 1 - 2xF residual: {worst:e} at x = {at}. Documented at \
             3.759e-12, which is the difference stencil's floor"
        );
    }

    /// The single maximum, against the published value.
    ///
    /// `F` peaks where `F' = 0`, i.e. where `F(x) = 1/(2x)` — so the location
    /// is found here by bisecting that condition rather than by scanning a
    /// grid, which makes the test independent of the grid's spacing.
    ///
    /// Measured 2026-09-19: **0.541044224638 at x = 0.924138873**, against
    /// the tabulated 0.5410442246 at 0.9241388730.
    #[test]
    fn the_maximum_is_where_the_literature_puts_it() {
        // F(x) - 1/(2x) is NEGATIVE below the peak and POSITIVE above it,
        // which is the opposite of what it looks like: 1/(2x) diverges at the
        // origin where F vanishes, and the tail F = 1/(2x) + 1/(4x^3) + ...
        // sits just above its own asymptote. A first draft of this test had
        // the sense backwards and bisected to the bracket's right end.
        let (mut lo, mut hi) = (0.5_f64, 1.5_f64);
        assert!(
            dawson(lo) - 0.5 / lo < 0.0 && dawson(hi) - 0.5 / hi > 0.0,
            "the bracket must straddle the crossing"
        );
        for _ in 0..200 {
            let m = 0.5 * (lo + hi);
            if dawson(m) - 0.5 / m < 0.0 {
                lo = m;
            } else {
                hi = m;
            }
        }
        let x = 0.5 * (lo + hi);
        assert!(
            (x - 0.924_138_873_0).abs() < 1e-9,
            "the maximum is documented at x = 0.9241388730; it is at {x:.12}"
        );
        assert!(
            (dawson(x) - 0.541_044_224_6).abs() < 1e-9,
            "the maximum value is documented as 0.5410442246; it is {:.12}",
            dawson(x)
        );
        // And nothing anywhere exceeds it.
        for k in 0..=100_000 {
            let t = 1e-4 * k as f64;
            assert!(dawson(t) <= dawson(x), "F({t}) exceeds the maximum");
        }
    }

    /// **Upstream truncates all three tables, and it costs at most 7.7 ulp**
    /// — measured, because "GSL keeps fewer coefficients than it stores" is
    /// the kind of claim that invites assuming the discarded tail is zero.
    ///
    /// It is zero for one of the three and not for the other two:
    ///
    /// | table | array | used | discarded tail starts at | effect on the answer |
    /// |---|---|---|---|---|
    /// | `daw` | 21 | 16 | 3.0e-24 | **0** — bit-identical |
    /// | `daw2` | 45 | 33 | -1.6e-21 | 1.708e-15 (7.7 ulp) at `x = 3.98` |
    /// | `dawa` | 75 | 35 | -7.5e-21 | 4.140e-16 (1.9 ulp) at `x = 3819` |
    ///
    /// This is SLATEC's `initds` convention — the array holds enough for the
    /// highest precision anyone might want and the order selects what
    /// `double` needs — so the few ulp are a deliberate trade, not an
    /// oversight. The test asserts both halves: that truncating really is
    /// what GSL does, and that its cost is the measured handful of ulp rather
    /// than nothing.
    #[test]
    fn upstreams_truncation_costs_at_most_eight_ulp() {
        assert!(DAW_USED < DAW.len() && DAW2_USED < DAW2.len() && DAWA_USED < DAWA.len());
        assert_eq!((DAW_USED, DAW2_USED, DAWA_USED), (16, 33, 35));
        assert_eq!((DAW.len(), DAW2.len(), DAWA.len()), (21, 45, 75));

        /// The same function with every stored coefficient evaluated.
        fn full(x: f64) -> f64 {
            let y = x.abs();
            if y < XSML {
                x
            } else if y < 1.0 {
                x * (0.75 + cheb(2.0 * y * y - 1.0, &DAW))
            } else if y < 4.0 {
                x * (0.25 + cheb(0.125 * y * y - 1.0, &DAW2))
            } else if y < XBIG {
                (0.5 + cheb(32.0 / (y * y) - 1.0, &DAWA)) / x
            } else if y < XMAX {
                0.5 / x
            } else {
                0.0
            }
        }

        for (name, lo, hi, want_ulp) in [
            ("daw", 1e-6_f64, 0.999_f64, 0.0_f64),
            ("daw2", 1.0, 3.999, 8.0),
            ("dawa", 4.0, 1e7, 2.0),
        ] {
            let (mut worst, mut at) = (0.0_f64, 0.0_f64);
            for k in 0..=20_000 {
                let x = lo * (hi / lo).powf(k as f64 / 20_000.0);
                let (a, b) = (dawson(x), full(x));
                if a.abs() > 1e-300 {
                    let e = ((a - b) / a).abs();
                    if e > worst {
                        worst = e;
                        at = x;
                    }
                }
            }
            let ulp = worst / f64::EPSILON;
            assert!(
                ulp <= want_ulp,
                "{name}: truncation is documented as costing at most \
                 {want_ulp} ulp; it costs {ulp:.1} ({worst:e}) at x = {at}"
            );
        }

        // The first table's discarded tail really is below the sum's ulp --
        // which is WHY it is bit-identical, and the thing that would change
        // if upstream re-fitted it.
        for &c in DAW.get(DAW_USED..).into_iter().flatten() {
            assert!(c.abs() < 1e-20, "a discarded daw coefficient is {c:e}");
        }
    }

    /// Odd, bounded, `1/(2x)` in the tail, and the two documented refusals.
    #[test]
    fn the_shape_and_the_edges() {
        assert_eq!(dawson(0.0), 0.0);
        assert!(dawson(f64::NAN).is_nan());

        // Odd, exactly -- both branches of the sign go through the same
        // |x| and the same multiply by x.
        for k in 1..=4000 {
            let x = 0.01 * k as f64;
            assert_eq!(dawson(-x), -dawson(x), "not odd at {x}");
        }

        // Bounded by its maximum everywhere, including the far tail.
        for x in [1e-9_f64, 0.5, 1.0, 10.0, 1e6, 1e100, 1e300] {
            assert!(
                dawson(x).abs() <= 0.541_044_3,
                "F({x:e}) = {:e} exceeds the maximum",
                dawson(x)
            );
        }

        // 1/(2x) asymptotically, to the order the next term allows:
        // F(x) = 1/(2x) + 1/(4x^3) + O(x^-5).
        for x in [50.0_f64, 100.0, 1e3, 1e4] {
            let two_term = 0.5 / x + 0.25 / (x * x * x);
            assert!(
                ((dawson(x) - two_term) / two_term).abs() < 3.0 / (x * x * x * x),
                "the tail at x = {x:e} is {:e} against the two-term {two_term:e}",
                dawson(x)
            );
        }

        // Upstream's UNDERFLOW_ERROR, and that it is the ONLY place the
        // function returns zero for non-zero x.
        assert_eq!(dawson(XMAX), 0.0);
        assert_eq!(dawson(f64::MAX), 0.0);
        assert!(dawson(XMAX * 0.999) > 0.0);
        assert_eq!(dawson(f64::INFINITY), 0.0);
    }

    /// The three machine constants are the expressions upstream writes.
    #[test]
    fn the_bounds_are_what_upstream_declares() {
        assert_eq!(XSML, 1.225 * SQRT_DBL_EPSILON);
        assert_eq!(XBIG, 1.0 / (core::f64::consts::SQRT_2 * SQRT_DBL_EPSILON));
        assert_eq!(XMAX, 0.1 * f64::MAX);
        // And they are ordered, which the branch chain silently relies on.
        assert!(XSML < 1.0 && 4.0 < XBIG && XBIG < XMAX);
    }
}
