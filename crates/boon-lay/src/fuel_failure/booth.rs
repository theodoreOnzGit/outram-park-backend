// SPDX-License-Identifier: GPL-3.0
//
// PANAMA-I reimplementation — provenance
// --------------------------------------
// Reference : Verfondern, K. & Nabielek, H., "The Mathematical Basis of the
//             PANAMA-I Code for Modeling Pressure Vessel Failure of TRISO
//             Coated Particles under Accident Conditions",
//             Forschungszentrum Jülich, HTA-IB-03/90, 1 August 1990.
//             Reprinted as Appendix C, printed pages -479- to -511-.
// Status    : the report is restricted literature with no reuse licence. Only
//             the governing EQUATIONS and their constants are reproduced here,
//             with citation, as scientific facts. No prose, figure or page of
//             that document is copied into this repository, and the PDF is not
//             tracked here. See DATA_POLICY.md.
// Nature    : an independent Rust implementation of the published model, not a
//             port of the PANAMA Fortran (which is closed-source and was never
//             consulted).

//! **Fission-gas release** — the Booth function `f(τ)` (unnumbered, page
//! -485-), Eq (4) for `F_d` (page -485-, Allelein 1983), and the
//! dimensionless times `τ_i`, `τ_a` (page -486-).
//!
//! ```text
//! f(τ) = 1 − (6/τ)·Σ_{n=1}^∞ (1 − exp(−n²π²τ)) / (n⁴π⁴)
//!
//! F_d  = [ (τ_i + τ_a)·f(τ_i + τ_a) − τ_a·f(τ_a) ] / τ_i        (4)
//!
//! τ_i  = D_S(T_B)·t_B      τ_a = D_S(T)·t
//! ```
//!
//! `F_d` is the fraction of the stable fission gas that has escaped the kernel
//! into the buffer void, and it is what multiplies the fission-gas yield in
//! Eq (3). `f` itself is the classical Booth release integral for a sphere
//! with a **constant production rate**; Eq (4) differences the
//! irradiation-plus-accident release against the accident-only part so that
//! what is left is the release attributable to the inventory built up during
//! irradiation.
//!
//! # The printed series has a grouping ambiguity, and only one reading works
//!
//! On page -485- the fraction bar in the summand spans **only**
//! `exp(−n²π²τ) / (n⁴π⁴)`, with the `(1 −` opening outside it. Read
//! literally, the summand is `1 − exp(−n²π²τ)/(n⁴π⁴)`, which tends to `1` as
//! `n → ∞` — the series **diverges**, and a 1000-term partial sum gives
//! `f(0.1) = −6.0·10⁴` instead of a number in `[0, 1]`.
//!
//! The consistent reading puts the whole `1 − exp(…)` in the numerator, as
//! implemented. It is the reading verified below, and it is also the standard
//! Booth form.
//!
//! | reading | `f(0.1)` | `f(0.5)` | `f(1.9)` |
//! |---|---|---|---|
//! | literal, bar over the exponential only | −5.99997·10⁴ | −1.19990·10⁴ | −3.1569·10³ |
//! | **whole `1 − exp(…)` in the numerator** | **0.56365** | **0.86755** | **0.96491** |
//!
//! ## Verification — methodology
//!
//! Fig. 1 (page -486-) plots `f(τ)` over `τ ∈ [0, 2]` and is therefore a
//! direct check on the reading. Digitised by the maintainer 2026-09-24:
//! **78 points**, `τ` from 0.0313 to 1.9077, read off the printed curve.
//! (The digitiser's y-axis calibration labels the upper gridline `500`; a
//! least-squares fit of the digitised ordinate against this implementation,
//! forced through the origin and restricted to `τ ≥ 0.15` where the curve is
//! not near-vertical, gives a scale of **501.29**, i.e. that gridline is
//! `f = 1` to within 0.26 %. The comparison below uses `f = y/500`.)
//!
//! Pass criterion: the consistent reading within digitisation noise over the
//! whole figure; the literal reading excluded by orders of magnitude.
//!
//! ## Verification — results, 2026-09-24
//!
//! | sample | n | mean \|Δf\| | median | worst |
//! |---|---|---|---|---|
//! | all digitised points | 78 | **0.0066** | 0.0020 | 0.047 |
//! | `τ ≥ 0.15` | 68 | **0.0028** | — | 0.018 |
//!
//! The worst point is the first one, `τ = 0.0313`, where the curve is nearly
//! vertical (`df/dτ ≈ 5`): a 0.009 error in reading `τ` off the page accounts
//! for the whole 0.047. On an ordinate running 0 to 1 the `τ ≥ 0.15` figure
//! of 0.0028 is digitisation noise. Pinned by
//! [`tests::figure_1_is_reproduced`] and
//! [`tests::the_literal_grouping_is_excluded`].
//!
//! ## Two analytic checks the figure cannot give
//!
//! Because `Σ 1/(n⁴π⁴) = ζ(4)/π⁴ = 1/90` exactly, the series rearranges to
//! `f = 1 − 1/(15τ) + (6/τ)·Σ exp(−n²π²τ)/(n⁴π⁴)`, giving two limits that are
//! independent of the digitisation:
//!
//! | limit | closed form | agreement |
//! |---|---|---|
//! | `τ → ∞` | `1 − 1/(15τ)` | 4·10⁻¹² at `τ = 10` |
//! | `τ → 0` | `4√(τ/π) − 3τ/2` | 2·10⁻⁷ at `τ = 10⁻⁴` |
//!
//! Both are pinned by [`tests::the_analytic_limits_are_recovered`]. They also
//! fix the *normalisation*, which Fig. 1 alone cannot: a factor-of-two error
//! in the `6/τ` pre-factor would still plot as a plausible rising curve.
//!
//! # Summation cut-off: the report's 1000-term cap is what binds
//!
//! Page -485- states the "infinite" sum is terminated after **1000 summands
//! (caution!)**, or when two consecutive summands differ by no more than
//! **10⁻²⁰**. Both are implemented, but the second never fires first: the
//! summand tends to `1/(n⁴π⁴)`, whose consecutive differences reach 10⁻²⁰ only
//! near `n ≈ 5.3·10³`. The report's own "caution!" is well placed, and the
//! measured cost of the cap is:
//!
//! | `τ` | 1000-term sum | error against the rearranged form |
//! |---|---|---|
//! | 10⁻² … 10¹ | 0.2107 … 0.9933 | ≤ 2·10⁻⁹ |
//! | 10⁻⁴ | 0.02242 | 3·10⁻⁷ |
//! | 10⁻⁶ | 0.002276 | ≤ 2·10⁻⁵ (bound `2/(N³π⁴τ)`) |
//! | 10⁻⁸ | 6.4·10⁻⁴ | ~4·10⁻⁴ — **larger than the answer** |
//!
//! So this function is trustworthy for `τ ≳ 10⁻⁵` and degrades below it.
//! That limit is the report's algorithm, not an implementation shortcut, and
//! it is left in place rather than silently replaced by the rearranged form —
//! which is in any case *worse* for small `τ`, since it cancels
//! `1/(15τ) ≈ 6.7·10⁶` against itself to produce a number of order 10⁻⁴.
//! [`tests::the_thousand_term_cap_is_the_binding_one`] records both facts.

use uom::si::f64::{Frequency, Ratio, Time};
use uom::si::ratio::ratio;

/// The report's cap on the number of summands (page -485-).
pub const MAX_SUMMANDS: usize = 1000;

/// The report's convergence criterion on consecutive summands (page -485-).
///
/// Never the binding one — see the module docs.
pub const SUMMAND_CONVERGENCE: f64 = 1.0e-20;

/// The Booth release function `f(τ)` (unnumbered, page -485-).
///
/// ```text
/// f(τ) = 1 − (6/τ)·Σ_{n=1}^∞ (1 − exp(−n²π²τ)) / (n⁴π⁴)
/// ```
///
/// Dimensionless in and out. `f(0) = 0` and `f → 1` as `τ → ∞`; the return is
/// clamped to `[0, 1]` only at `τ ≤ 0`, where the expression is undefined —
/// everywhere else the series is left to speak for itself, so that a
/// mis-transcription shows up as an out-of-range value rather than being
/// hidden by a clamp.
///
/// Accurate for `τ ≳ 10⁻⁵`; see the module docs for the measured behaviour of
/// the report's 1000-term cap below that.
pub fn booth_release_function(tau: Ratio) -> Ratio {
    let t = tau.get::<ratio>();
    if t <= 0.0 {
        return Ratio::new::<ratio>(0.0);
    }
    let pi2 = std::f64::consts::PI * std::f64::consts::PI;
    let pi4 = pi2 * pi2;
    let mut sum = 0.0f64;
    let mut previous: Option<f64> = None;
    for n in 1..=MAX_SUMMANDS {
        let n = n as f64;
        let n2 = n * n;
        let summand = (1.0 - (-n2 * pi2 * t).exp()) / (n2 * n2 * pi4);
        sum += summand;
        if let Some(p) = previous {
            if (summand - p).abs() <= SUMMAND_CONVERGENCE {
                break;
            }
        }
        previous = Some(summand);
    }
    Ratio::new::<ratio>(1.0 - 6.0 / t * sum)
}

/// A dimensionless diffusion time `τ = D_S·t` (page -486-).
///
/// Used for both of the report's arguments: `τ_i = D_S(T_B)·t_B` over the
/// irradiation and `τ_a = D_S(T)·t` over the accident. `D_S = D_eff/r_o²` is
/// a [`Frequency`] (the report prints it in `s⁻¹`) so the product is
/// dimensionless by construction and cannot be assembled from the wrong pair
/// of quantities.
pub fn dimensionless_time(reduced_diffusion: Frequency, elapsed: Time) -> Ratio {
    reduced_diffusion * elapsed
}

/// **Eq (4)** — the released fraction `F_d` of the stable fission gas
/// (page -485-, Allelein 1983).
///
/// ```text
/// F_d = [ (τ_i + τ_a)·f(τ_i + τ_a) − τ_a·f(τ_a) ] / τ_i
/// ```
///
/// - `tau_irradiation` — `τ_i = D_S(T_B)·t_B`.
/// - `tau_accident` — `τ_a = D_S(T)·t`.
///
/// Applies to Xe and Kr. The argument structure is as printed: `τ_i` appears
/// only inside the sum and in the denominator, so `F_d` is a *time-average*
/// over the irradiation rather than a release evaluated at its end.
///
/// ## `τ_i = 0` is not defined by the report
///
/// Eq (4) is singular there and the report does not say what to do. This
/// returns **zero**, on the grounds that no irradiation means no fission-gas
/// inventory to release — and in Eq (3) the same limit carries `F_b = 0`
/// alongside, so the pressure is zero either way. That is this
/// implementation's convention and not the report's; it is recorded in
/// `docs/panama-i-units-and-open-questions.md`.
pub fn released_gas_fraction(tau_irradiation: Ratio, tau_accident: Ratio) -> Ratio {
    let ti = tau_irradiation.get::<ratio>();
    let ta = tau_accident.get::<ratio>().max(0.0);
    if ti <= 0.0 {
        return Ratio::new::<ratio>(0.0);
    }
    let total = ti + ta;
    let f_total = booth_release_function(Ratio::new::<ratio>(total)).get::<ratio>();
    let f_accident = booth_release_function(Ratio::new::<ratio>(ta)).get::<ratio>();
    Ratio::new::<ratio>((total * f_total - ta * f_accident) / ti)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::frequency::hertz;
    use uom::si::time::second;

    fn f(tau: f64) -> f64 {
        booth_release_function(Ratio::new::<ratio>(tau)).get::<ratio>()
    }

    fn f_d(ti: f64, ta: f64) -> f64 {
        released_gas_fraction(Ratio::new::<ratio>(ti), Ratio::new::<ratio>(ta)).get::<ratio>()
    }

    /// The literal reading of the printed summand, kept so the rejected
    /// grouping stays reproducible instead of only described.
    fn f_literal(tau: f64) -> f64 {
        let pi2 = std::f64::consts::PI * std::f64::consts::PI;
        let pi4 = pi2 * pi2;
        let mut sum = 0.0;
        for n in 1..=MAX_SUMMANDS {
            let n = n as f64;
            let n2 = n * n;
            sum += 1.0 - (-n2 * pi2 * tau).exp() / (n2 * n2 * pi4);
        }
        1.0 - 6.0 / tau * sum
    }

    /// **Fig. 1 (page -486-) is reproduced.**
    ///
    /// Methodology: Fig. 1 plots `f(τ)` over `τ ∈ [0, 2]`. Digitised by the
    /// maintainer 2026-09-24 — 78 points, `τ` 0.0313 to 1.9077. The
    /// digitiser's upper gridline is calibrated `500`; a through-origin fit
    /// of the digitised ordinate against this implementation over
    /// `τ ≥ 0.15` gives 501.29, so `f = y/500` to within 0.26 %.
    ///
    /// Results: mean `|Δf|` **0.0066** over all 78 points (median 0.0020,
    /// worst 0.047 at the first point, where the curve is near-vertical);
    /// **0.0028** mean and 0.018 worst over the 68 points with `τ ≥ 0.15`.
    /// On an ordinate running 0 to 1 that is digitisation noise.
    ///
    /// The samples below are taken from that digitisation at ten-point
    /// intervals plus the two endpoints; the tolerance is the measured worst
    /// case for each region, not a value chosen to pass.
    #[test]
    fn figure_1_is_reproduced() {
        // (tau, digitised f = y/500)
        let steep = [(0.031_344_3, 0.399_521), (0.102_150_7, 0.591_603)];
        let body = [
            (0.153_893_9, 0.672_197),
            (0.374_483_1, 0.829_354),
            (0.638_645_5, 0.896_516),
            (0.905_531_1, 0.926_067),
            (1.175_140_2, 0.944_872),
            (1.447_472_5, 0.954_275),
            (1.719_804_9, 0.962_334),
            (1.907_714_2, 0.967_707),
        ];
        for (tau, want) in steep {
            let got = f(tau);
            assert!(
                (got - want).abs() < 0.05,
                "tau={tau}: series gives {got:.4}, Fig. 1 shows {want:.4}"
            );
        }
        for (tau, want) in body {
            let got = f(tau);
            assert!(
                (got - want).abs() < 0.02,
                "tau={tau}: series gives {got:.4}, Fig. 1 shows {want:.4}"
            );
        }
    }

    /// **The literal grouping of the printed summand is excluded by Fig. 1.**
    ///
    /// Pins the rejected reading so the settlement cannot be quietly undone.
    /// It is not a near miss: the series diverges, and its 1000-term partial
    /// sum is four orders of magnitude outside `[0, 1]`.
    #[test]
    fn the_literal_grouping_is_excluded() {
        for tau in [0.1, 0.5, 1.9] {
            let lit = f_literal(tau);
            assert!(
                lit < -1.0e3,
                "the literal reading should be wildly negative at tau={tau}, got {lit}"
            );
            assert!(
                (0.0..=1.0).contains(&f(tau)),
                "the implemented reading must stay a fraction"
            );
        }
        // And it diverges: doubling the term count doubles the damage.
        let pi2 = std::f64::consts::PI * std::f64::consts::PI;
        let pi4 = pi2 * pi2;
        let partial = |n_max: usize| -> f64 {
            (1..=n_max)
                .map(|n| {
                    let n2 = (n * n) as f64;
                    1.0 - (-n2 * pi2 * 0.5).exp() / (n2 * n2 * pi4)
                })
                .sum::<f64>()
        };
        assert!(partial(2000) > 1.9 * partial(1000));
    }

    /// **Two analytic limits the figure cannot supply.**
    ///
    /// `Σ 1/(n⁴π⁴) = 1/90` exactly, so `f → 1 − 1/(15τ)` for large `τ`; and
    /// the classical constant-production Booth expansion gives
    /// `f → 4√(τ/π) − 3τ/2` for small `τ`. Agreement measured 2026-09-24:
    /// 4·10⁻¹² at `τ = 10`, 2·10⁻⁷ at `τ = 10⁻⁴`.
    ///
    /// These fix the normalisation, which Fig. 1 alone cannot: a wrong `6/τ`
    /// pre-factor would still plot as a plausible rising curve.
    #[test]
    fn the_analytic_limits_are_recovered() {
        for tau in [5.0, 10.0, 100.0] {
            let want = 1.0 - 1.0 / (15.0 * tau);
            assert!(
                (f(tau) - want).abs() < 1e-10,
                "tau={tau}: {} vs 1 - 1/(15 tau) = {want}",
                f(tau)
            );
        }
        for tau in [1.0e-4, 1.0e-3, 1.0e-2] {
            let want = 4.0 * (tau / std::f64::consts::PI).sqrt() - 1.5 * tau;
            assert!(
                (f(tau) - want).abs() < 3.0e-7,
                "tau={tau}: {} vs 4 sqrt(tau/pi) - 3 tau/2 = {want}",
                f(tau)
            );
        }
        assert_eq!(f(0.0), 0.0);
        assert_eq!(f(-1.0), 0.0);
    }

    /// The 1000-term cap, not the 10⁻²⁰ summand criterion, is what stops the
    /// sum — and it is what limits the accuracy at small `τ`.
    #[test]
    fn the_thousand_term_cap_is_the_binding_one() {
        // Consecutive summands only close to 1e-20 near n ~ 5.3e3, well past
        // the cap: check the gap is still far above tolerance at n = 1000.
        let pi2 = std::f64::consts::PI * std::f64::consts::PI;
        let pi4 = pi2 * pi2;
        let term = |n: f64| (1.0 - (-n * n * pi2).exp()) / (n * n * n * n * pi4);
        let gap = (term(1000.0) - term(999.0)).abs();
        assert!(
            gap > SUMMAND_CONVERGENCE,
            "the summand criterion must NOT fire before the cap; gap {gap:e}"
        );

        // The rearranged form, valid where it does not cancel catastrophically.
        let rearranged = |tau: f64| {
            let tail: f64 = (1..80)
                .map(|n| {
                    let n2 = (n * n) as f64;
                    (-n2 * pi2 * tau).exp() / (n2 * n2 * pi4)
                })
                .sum();
            1.0 - 1.0 / (15.0 * tau) + 6.0 / tau * tail
        };
        for tau in [1.0e-2, 1.0e-1, 1.0, 10.0] {
            assert!(
                (f(tau) - rearranged(tau)).abs() < 1e-8,
                "tau={tau}: {} vs {}",
                f(tau),
                rearranged(tau)
            );
        }
        // And it degrades at 1e-8, as the report's "caution!" implies: the
        // error there is of the same size as the answer.
        let truth = 4.0 * (1.0e-8 / std::f64::consts::PI).sqrt();
        assert!(
            (f(1.0e-8) - truth).abs() > 0.5 * truth,
            "the cap's limitation at tau=1e-8 is a documented fact, not a bug \
             to be silently patched"
        );
    }

    /// **Eq (4) reduces to `f(τ_i)` when `τ_a = 0`** — an exact identity in
    /// the printed equation, and the cheapest check that the argument
    /// structure was transcribed correctly.
    #[test]
    fn eq_4_reduces_to_f_at_zero_accident_time() {
        for ti in [1.0e-3, 0.01, 0.1, 1.0, 10.0] {
            assert!(
                (f_d(ti, 0.0) - f(ti)).abs() < 1e-12,
                "F_d(tau_i, 0) must be f(tau_i); {} vs {}",
                f_d(ti, 0.0),
                f(ti)
            );
        }
    }

    /// `F_d` is a fraction, rises with accident time, and saturates at 1.
    ///
    /// Measured over `τ_i ∈ [10⁻⁴, 10²] × τ_a ∈ [0, 10²]`: the whole surface
    /// lies in `[0.0224, 1.0]` and is monotone in `τ_a`.
    ///
    /// The monotonicity tolerance is `10⁻⁹`, not `0`: Eq (4) differences two
    /// numbers of order `τ_a` and divides by `τ_i`, so at `τ_i = 10⁻⁴` with
    /// `F_d` already at 1 it cancels about four digits. That is a property of
    /// the printed equation, not of this implementation — the direct Booth
    /// sum itself has no cancellation (see the module docs).
    #[test]
    fn eq_4_is_a_monotone_fraction() {
        for ti in [1.0e-4, 1.0e-2, 0.1, 1.0, 10.0, 100.0] {
            let mut previous = -1.0;
            for k in 0..60 {
                let ta = k as f64 * 0.05;
                let v = f_d(ti, ta);
                assert!(
                    (0.0..=1.0 + 1e-9).contains(&v),
                    "F_d out of range at ({ti}, {ta}): {v}"
                );
                assert!(
                    v >= previous - 1e-9,
                    "F_d must not fall with accident time at ({ti}, {ta})"
                );
                previous = v;
            }
            assert!(f_d(ti, 1.0e3) > 0.999);
        }
        assert_eq!(f_d(0.0, 1.0), 0.0, "tau_i = 0 is this crate's convention");
    }

    /// `τ = D_S·t` is dimensionless and assembled from the report's own
    /// quantities.
    #[test]
    fn dimensionless_time_is_the_product() {
        let tau = dimensionless_time(Frequency::new::<hertz>(2.0e-7), Time::new::<second>(5.0e6));
        assert!((tau.get::<ratio>() - 1.0).abs() < 1e-12);
    }
}
