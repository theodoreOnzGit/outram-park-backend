//! **Code-to-code — PETIR against `tampines-steam-tables`' own Chebyshev
//! evaluator, on its own committed coefficients.**
//!
//! # Why
//!
//! `bn:op-chyp.2` names that crate's
//! `backward_eqn_chebyshev_experimental/chebyshev.rs` as "existing evaluation
//! code to lift rather than rewrite". Before anything is lifted or swapped,
//! the two evaluators have to be shown to agree on real data — and the first
//! thing that check turns up is that **they use different conventions**.
//!
//! tampines closes Clenshaw with `c[0] + x*b1 - b2`; GSL closes with
//! `0.5 * c[0]` because `gsl_cheb_init` doubles the zeroth coefficient. Feed
//! one convention's table to the other and every value is wrong by `c[0]/2` —
//! silently, with no error and no obvious symptom. For `HF_COEFFS` below that
//! offset would be ~904 kJ/kg of saturated-liquid enthalpy.
//!
//! [`ChebSeries::from_plain_coefficients`] is the bridge, and this test is the
//! evidence that it is the *right* bridge: with it, PETIR reproduces
//! tampines' evaluator on tampines' real committed table.
//!
//! # What this does NOT claim
//!
//! It does not show PETIR is ready to replace that module. Three things still
//! stand in the way, recorded here so they are not rediscovered:
//!
//! 1. **2-D.** Two of the three consumers use tensor-product evaluators
//!    (`cheb2_dense`, `cheb2_sparse`). GSL's `cheb` module is 1-D only, so
//!    PETIR has nothing to offer them yet.
//! 2. **Allocation.** [`ChebSeries`] owns a `Vec`; tampines' tables are
//!    `const [f64; N]` evaluated in steam-table lookups on solver hot paths.
//!    A per-call allocation there would be a regression.
//! 3. **Convention.** The tables would have to be migrated, or every call site
//!    routed through the adapter.
//!
//! Those are the actual content of "lift rather than rewrite", and none of
//! them is resolved by this test.

use petir::ChebSeries;

/// `HF_COEFFS` from
/// `tampines-steam-tables/src/backward_eqn_chebyshev_experimental/region_4_near_critical_hs.rs:247`
/// — saturated-liquid enthalpy near the critical point, first 8 terms of 19.
/// A slice is enough: the convention question lives entirely in `c[0]`, and a
/// truncated series is still a valid series to compare two evaluators on.
const HF_COEFFS_HEAD: [f64; 8] = [
    1.80825398923045304e+03,
    1.60086291751135349e+02,
    3.29796694038905116e+01,
    1.61377380873189011e+01,
    9.66662493733826089e+00,
    6.36006374111844419e+00,
    4.43947932182388083e+00,
    3.21089148762468568e+00,
];

/// `TLO`/`THI` from that file (:99-100) — the fit domain in kelvin.
const TLO: f64 = 623.15;
const THI: f64 = 647.04;

/// tampines' evaluator, transcribed verbatim from its `chebyshev.rs` so the
/// comparison is against that code and not against a paraphrase of it.
fn tampines_cheb1(x: f64, c: &[f64]) -> f64 {
    let (mut b1, mut b2) = (0.0, 0.0);
    for &ck in c[1..].iter().rev() {
        let b = 2.0 * x * b1 - b2 + ck;
        b2 = b1;
        b1 = b;
    }
    c[0] + x * b1 - b2
}

/// tampines' `scale` (its `chebyshev.rs:18`).
fn tampines_scale(v: f64, lo: f64, hi: f64) -> f64 {
    2.0 * (v - lo) / (hi - lo) - 1.0
}

#[test]
fn petir_reproduces_tampines_on_its_own_committed_coefficients() {
    let cs = ChebSeries::from_plain_coefficients(HF_COEFFS_HEAD.to_vec(), TLO, THI)
        .expect("plain-convention table");

    let mut worst = 0.0f64;
    let mut worst_t = 0.0f64;
    for k in 0..=200 {
        let t = TLO + (THI - TLO) * f64::from(k) / 200.0;
        let theirs = tampines_cheb1(tampines_scale(t, TLO, THI), &HF_COEFFS_HEAD);
        // PETIR maps [a,b] -> [-1,1] itself, so it takes the raw temperature.
        let ours = cs.eval(t);
        let d = (ours - theirs).abs();
        if d > worst {
            worst = d;
            worst_t = t;
        }
    }
    println!("  vs tampines cheb1 on HF_COEFFS: worst |delta| = {worst:.3e} at T = {worst_t:.3} K");
    // Values are ~1.8e3 kJ/kg, so 1e-9 is ~1e-13 relative — round-off only.
    assert!(
        worst < 1.0e-9,
        "PETIR and tampines disagree by {worst:.3e} at T = {worst_t:.3} K on the same \
         committed coefficients"
    );
}

/// The failure mode the adapter exists to prevent, pinned so nobody "simplifies"
/// `from_plain_coefficients` into `from_coefficients` later.
#[test]
fn mixing_the_conventions_is_wrong_by_exactly_half_the_constant_term() {
    let wrong = ChebSeries::from_coefficients(HF_COEFFS_HEAD.to_vec(), TLO, THI).unwrap();
    let right = ChebSeries::from_plain_coefficients(HF_COEFFS_HEAD.to_vec(), TLO, THI).unwrap();
    let t = 0.5 * (TLO + THI);
    let offset = right.eval(t) - wrong.eval(t);
    let expected = 0.5 * HF_COEFFS_HEAD[0];
    assert!(
        (offset - expected).abs() < 1.0e-9,
        "the convention offset should be exactly c[0]/2 = {expected:.6}, measured {offset:.6}"
    );
    // ~904 kJ/kg on a saturated-liquid enthalpy: not a subtle error, but a
    // silent one -- both evaluators return a plausible number.
    assert!(offset > 900.0, "offset {offset:.1} — sanity check on the magnitude");
}
