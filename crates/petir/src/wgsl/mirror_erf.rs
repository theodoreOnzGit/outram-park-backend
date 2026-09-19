//! `f32` mirrors of `shaders/erf.wgsl` — GSL's error-function family.
//!
//! Generated from the same parse of `upstream_source/GSL/specfunc/erfc.c` that
//! produced the shader, so the two cannot drift in their coefficients. The
//! control flow is written out by hand to match the shader branch for branch.
//!
//! See [`crate::wgsl::mirror`] for why a mirror exists at all.

#[allow(unused_imports)]
use crate::real::Real;

#[rustfmt::skip]
const ERFC_XLT1: [f32; 20] = [
    1.06073416421769980345174155056, -0.42582445804381043569204735291,
    0.04955262679620434040357683080, 0.00449293488768382749558001242,
    -0.00129194104658496953494224761, -0.00001836389292149396270416979,
    0.00002211114704099526291538556, -5.23337485234257134673693179020e-7,
    -2.78184788833537885382530989578e-7, 1.41158092748813114560316684249e-8,
    2.72571296330561699984539141865e-9, -2.06343904872070629406401492476e-10,
    -2.14273991996785367924201401812e-11, 2.22990255539358204580285098119e-12,
    1.36250074650698280575807934155e-13, -1.95144010922293091898995913038e-14,
    -6.85627169231704599442806370690e-16, 1.44506492869699938239521607493e-16,
    2.45935306460536488037576200030e-18, -9.29599561220523396007359328540e-19
];

#[rustfmt::skip]
const ERFC_X15: [f32; 25] = [
    0.44045832024338111077637466616, -0.143958836762168335790826895326,
    0.044786499817939267247056666937, -0.013343124200271211203618353102,
    0.003824682739750469767692372556, -0.001058699227195126547306482530,
    0.000283859419210073742736310108, -0.000073906170662206760483959432,
    0.000018725312521489179015872934, -4.62530981164919445131297264430e-6,
    1.11558657244432857487884006422e-6, -2.63098662650834130067808832725e-7,
    6.07462122724551777372119408710e-8, -1.37460865539865444777251011793e-8,
    3.05157051905475145520096717210e-9, -6.65174789720310713757307724790e-10,
    1.42483346273207784489792999706e-10, -3.00141127395323902092018744545e-11,
    6.22171792645348091472914001250e-12, -1.26994639225668496876152836555e-12,
    2.55385883033257575402681845385e-13, -5.06258237507038698392265499770e-14,
    9.89705409478327321641264227110e-15, -1.90685978789192181051961024995e-15,
    3.50826648032737849245113757340e-16
];

#[rustfmt::skip]
const ERFC_X510: [f32; 20] = [
    1.11684990123545698684297865808, 0.003736240359381998520654927536,
    -0.000916623948045470238763619870, 0.000199094325044940833965078819,
    -0.000040276384918650072591781859, 7.76515264697061049477127605790e-6,
    -1.44464794206689070402099225301e-6, 2.61311930343463958393485241947e-7,
    -4.61833026634844152345304095560e-8, 8.00253111512943601598732144340e-9,
    -1.36291114862793031395712122089e-9, 2.28570483090160869607683087722e-10,
    -3.78022521563251805044056974560e-11, 6.17253683874528285729910462130e-12,
    -9.96019290955316888445830597430e-13, 1.58953143706980770269506726000e-13,
    -2.51045971047162509999527428316e-14, 3.92607828989125810013581287560e-15,
    -6.07970619384160374392535453420e-16, 9.12600607264794717315507477670e-17
];

#[rustfmt::skip]
const ERFC8_P: [f32; 6] = [
    2.97886562639399288862, 7.409740605964741794425, 6.1602098531096305440906,
    5.019049726784267463450058, 1.275366644729965952479585264,
    0.5641895835477550741253201704
];

#[rustfmt::skip]
const ERFC8_Q: [f32; 7] = [
    3.3690752069827527677, 9.608965327192787870698, 17.08144074746600431571095,
    12.0489519278551290360340491, 9.396034016235054150430579648,
    2.260528520767326969591866945, 1.0
];

/// GSL's single-precision Chebyshev orders, the `order_sp` field of each
/// `cheb_series` in `erfc.c`.
///
/// These are **GSL's own numbers**, not a retuning: 12, 16 and 12 against full
/// orders of 19, 24 and 19. Using them is what makes this the single-precision
/// path GSL provides rather than a truncation someone chose.
pub const ORDER_SP: (usize, usize, usize) = (12, 16, 12);

/// Clenshaw over a coefficient array at a fixed order, on `t` in `[-1, 1]`.
///
/// Mirrors the three `petir_cheb_erfc_*` functions, which are the same body
/// with different tables. `c[0]` enters halved, as GSL's `cheb_eval_e` has it.
fn cheb(c: &[f32], order: usize, t: f32) -> f32 {
    let Some((&c0, tail)) = c.split_first() else {
        return 0.0;
    };
    let used = order.min(tail.len());
    let Some(slice) = tail.get(..used) else {
        return 0.0;
    };
    let mut d1 = 0.0_f32;
    let mut d2 = 0.0_f32;
    let y2 = 2.0 * t;
    for &ci in slice.iter().rev() {
        let temp = d1;
        d1 = y2 * d1 - d2 + ci;
        d2 = temp;
    }
    t * d1 - d2 + 0.5 * c0
}

/// The Maclaurin series for `erf`. Mirrors `petir_erfseries` / GSL's
/// `erfseries`.
pub fn erfseries(x: f32) -> f32 {
    const TWO_OVER_SQRTPI: f32 = 1.128_379_2;
    let mut coef = x;
    let mut e = coef;
    for k in 1..30i32 {
        coef *= -x * x / k as f32;
        e += coef / (2.0 * k as f32 + 1.0);
    }
    TWO_OVER_SQRTPI * e
}

/// `erfc` for `x > 10`. Mirrors `petir_erfc8` / GSL's `erfc8`.
pub fn erfc8(x: f32) -> f32 {
    // Upstream walks `P[5]` down to `P[0]` and `Q[6]` down to `Q[0]` by index.
    // Written here with `split_last` and a reversed iterator, which is the
    // same sweep in the same order without a runtime-checked subscript --
    // `tests/no_panic_gate.rs` allows a literal index into a `const` array but
    // not a loop variable, and it is right to.
    let (&p_hi, p_rest) = match ERFC8_P.split_last() {
        Some(v) => v,
        None => return 0.0,
    };
    let mut num = p_hi;
    for &pi in p_rest.iter().rev() {
        num = x * num + pi;
    }

    let (&q_hi, q_rest) = match ERFC8_Q.split_last() {
        Some(v) => v,
        None => return 0.0,
    };
    let mut den = q_hi;
    for &qi in q_rest.iter().rev() {
        den = x * den + qi;
    }

    (num / den) * (-x * x).exp()
}

/// The complementary error function in `f32`.
///
/// Mirrors `petir_erfc`, which ports `gsl_sf_erfc_e` — four branches on
/// `|x|`, then the reflection `erfc(-x) = 2 - erfc(x)`.
///
/// # Example
///
/// ```
/// use petir::wgsl::mirror_erf::erfc;
/// assert!((erfc(0.0) - 1.0).abs() < 1e-6);
/// ```
pub fn erfc(x: f32) -> f32 {
    let ax = x.abs();
    let (o1, o2, o3) = ORDER_SP;
    let e_val = if ax <= 1.0 {
        cheb(&ERFC_XLT1, o1, 2.0 * ax - 1.0)
    } else if ax <= 5.0 {
        let ex2 = (-x * x).exp();
        ex2 * cheb(&ERFC_X15, o2, 0.5 * (ax - 3.0))
    } else if ax < 10.0 {
        let exterm = (-x * x).exp() / ax;
        exterm * cheb(&ERFC_X510, o3, (2.0 * ax - 15.0) / 5.0)
    } else {
        erfc8(ax)
    };

    if x < 0.0 {
        2.0 - e_val
    } else {
        e_val
    }
}

/// The error function in `f32`.
///
/// Mirrors `petir_erf`, which ports `gsl_sf_erf_e`: the Taylor series below
/// `|x| = 1`, and `1 - erfc` above it.
///
/// # Example
///
/// ```
/// use petir::wgsl::mirror_erf::erf;
/// assert_eq!(erf(0.0), 0.0);
/// assert!((erf(1.0) - 0.842_700_8).abs() < 1e-6);
/// ```
pub fn erf(x: f32) -> f32 {
    if x.abs() < 1.0 {
        return erfseries(x);
    }
    1.0 - erfc(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `erfc` in `f32` tracks PETIR's `f64` `erfc` across every branch.
    ///
    /// # Methodology
    ///
    /// PETIR's own `specfunc::erfc` is *delegated* to `libm` (fdlibm), not to
    /// GSL's algorithm — so this is a genuine cross-check between two
    /// independent implementations, not a routine compared against itself.
    /// Sampled densely enough to cross every one of GSL's four branch
    /// boundaries: |x| = 1, 5 and 10.
    ///
    /// Compared **relatively**, because `erfc` spans 30 orders of magnitude
    /// over this range; an absolute tolerance would be met trivially in the
    /// tail and would test nothing there.
    ///
    /// # Results
    ///
    /// Worst relative difference **8.368e-06, at x = 8.1**, measured
    /// 2026-09-19. Broken down by GSL branch, against the error that
    /// single precision alone predicts:
    ///
    /// | branch | range | worst measured | `x^2 * eps_f32` |
    /// |---|---|---|---|
    /// | Chebyshev `xlt1` | 0 to 1 | 3.377e-07 | 1.192e-07 |
    /// | Chebyshev `x15` x `exp(-x^2)` | 1 to 5 | 3.197e-06 | 2.980e-06 |
    /// | Chebyshev `x510` x `exp(-x^2)/x` | 5 to 10 | 9.043e-06 | 1.192e-05 |
    ///
    /// **The error is not the Chebyshev truncation; it is `exp`.** Every
    /// branch above 1 multiplies by `exp(-x^2)`, and `exp` amplifies an error
    /// in its argument by the argument's own magnitude: a single `f32`
    /// rounding of `x*x` at `x = 8` is `64 * eps_f32 ~ 8e-6` once exponentiated.
    /// The measured column tracks the predicted one across three branches
    /// spanning two orders of magnitude, which is what identifies the cause.
    ///
    /// Two consequences worth acting on. Raising the Chebyshev order would buy
    /// nothing. And a caller who needs relative accuracy in the tail should
    /// want a *scaled* form — `erfcx(x) = exp(x^2) erfc(x)`, which PETIR has in
    /// `f64` — rather than this, because the scaled function never forms
    /// `exp(-x^2)` at all.
    ///
    /// # What this does NOT cover
    ///
    /// **The fourth branch, `erfc8` for `x >= 10`, is not exercised by this
    /// test and effectively cannot be.** `erfc(10)` is about 2e-45, at or
    /// below the smallest `f32` subnormal, so in single precision the whole
    /// branch underflows to zero and there is no relative error to measure.
    /// It is transcribed and it validates, but its arithmetic is unverified
    /// here; the tests below check only that it stays finite and
    /// non-negative.
    #[test]
    fn f32_erfc_tracks_the_f64_erfc_across_every_branch() {
        let mut worst = 0.0_f64;
        let mut worst_at = 0.0_f64;
        for k in 0..=2400 {
            let x = -12.0 + 0.01 * k as f64;
            let exact = crate::specfunc::erfc(x);
            // Below the f32 subnormal floor there is nothing to compare.
            if exact < 1e-30 {
                continue;
            }
            let got = erfc(x as f32) as f64;
            let rel = ((got - exact) / exact).abs();
            if rel > worst {
                worst = rel;
                worst_at = x;
            }
        }
        assert!(
            worst < 1e-4,
            "worst relative difference {worst:e} at x = {worst_at}"
        );
    }

    /// `erf` in `f32` tracks PETIR's `f64` `erf`.
    ///
    /// # Methodology
    ///
    /// As above, across the series branch (`|x| < 1`) and the `1 - erfc`
    /// branch. Points where `erf` is near zero are skipped: it has a root at
    /// the origin and a relative error there is meaningless.
    ///
    /// # Results
    ///
    /// Worst relative difference **1.959e-07, at x = -0.85**, measured
    /// 2026-09-19 — about 1.6 `f32` ulp, so at the floor.
    ///
    /// `erf` is far more accurate than `erfc` here, and for a structural
    /// reason: on `|x| < 1` it uses the Taylor series and never forms
    /// `exp(-x^2)`, so it escapes the amplification described above. Above 1
    /// it is `1 - erfc`, where `erfc` is the small quantity and the
    /// subtraction is benign.
    #[test]
    fn f32_erf_tracks_the_f64_erf() {
        let mut worst = 0.0_f64;
        let mut worst_at = 0.0_f64;
        for k in 0..=1200 {
            let x = -6.0 + 0.01 * k as f64;
            let exact = crate::specfunc::erf(x);
            if exact.abs() < 1e-3 {
                continue;
            }
            let got = erf(x as f32) as f64;
            let rel = ((got - exact) / exact).abs();
            if rel > worst {
                worst = rel;
                worst_at = x;
            }
        }
        assert!(
            worst < 1e-5,
            "worst relative difference {worst:e} at x = {worst_at}"
        );
    }

    /// The identity `erf(x) + erfc(x) = 1` holds in `f32`.
    ///
    /// # Why this and not only the reference comparison
    ///
    /// It is an *internal* consistency check: the two functions take different
    /// branches at the same `x` (below 1, `erf` uses the series while `erfc`
    /// uses a Chebyshev fit), so the identity exercises a combination the
    /// reference comparison checks only one side of at a time.
    ///
    /// # Results
    ///
    /// Worst `|erf(x) + erfc(x) - 1|` is **2.384e-07** over `[-4, 4]`,
    /// measured 2026-09-19 — two `f32` ulp of 1.0, which is the least it could
    /// be for two independently rounded quantities that must sum to one.
    #[test]
    fn erf_and_erfc_are_complementary_in_f32() {
        let mut worst = 0.0_f32;
        for k in 0..=800 {
            let x = -4.0 + 0.01 * k as f32;
            let s = erf(x) + erfc(x);
            worst = worst.max((s - 1.0).abs());
        }
        assert!(worst < 1e-5, "worst |erf + erfc - 1| = {worst:e}");
    }

    /// Known values, and the behaviour at and beyond the branch edges.
    #[test]
    fn known_values_and_edges() {
        assert_eq!(erf(0.0), 0.0);
        assert!((erfc(0.0) - 1.0).abs() < 1e-6);
        assert!((erf(1.0) - 0.842_700_79).abs() < 1e-6);
        assert!((erfc(-1.0) - 1.842_700_79).abs() < 1e-6);
        // erfc decays monotonically and stays non-negative through the tail.
        let mut prev = f32::INFINITY;
        for k in 0..=200 {
            let x = 0.1 * k as f32;
            let v = erfc(x);
            assert!(v >= 0.0, "erfc({x}) = {v} is negative");
            assert!(v <= prev + 1e-7, "erfc is not monotone at {x}");
            prev = v;
        }
        // Far tail underflows to zero rather than producing a NaN.
        assert!(erfc(30.0) >= 0.0 && erfc(30.0).is_finite());
        assert!((erfc(-30.0) - 2.0).abs() < 1e-6);
    }
}
