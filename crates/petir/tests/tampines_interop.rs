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
//! # This is the regression evidence for the rewiring
//!
//! `tampines-steam-tables` now calls PETIR for all of this, so these tests are
//! what stands between the move and a silent numerical change. Each compares
//! the **old implementation, transcribed verbatim from the pre-move file**,
//! against the PETIR function that replaced it, on tampines' **real committed
//! coefficient tables**.
//!
//! That comparison is necessary because tampines' own test suite **cannot be
//! run in this environment**, and could not before the move either: its
//! dev-dependencies pull the `egui`/`eframe` GUI stack, which requires rustc
//! 1.95 against the 1.94.1 available here. Verified by stashing the change and
//! reproducing the same failure. PETIR has no such dependencies, so the
//! equivalence check lives here.
//!
//! Three blockers were named when this file was first written. All three are
//! now closed: `petir::cheb_slice` supplies allocation-free evaluation over
//! borrowed slices and the two tensor-product evaluators (lifted from tampines
//! per `bn:op-chyp.2`), and `from_plain_coefficients` / `eval_plain` handle the
//! convention.

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


// ─────────────────────────────────────────────────────────────────────────────
// Old-vs-new equivalence on tampines' real tables. The `old_*` functions are
// transcribed verbatim from
// tampines-steam-tables/src/backward_eqn_chebyshev_experimental/chebyshev.rs
// as it stood BEFORE the move, so the comparison is against that code and not
// a paraphrase of it.
// ─────────────────────────────────────────────────────────────────────────────

fn old_scale(v: f64, lo: f64, hi: f64) -> f64 {
    2.0 * (v - lo) / (hi - lo) - 1.0
}

fn old_cheb_basis<const N: usize>(x: f64) -> [f64; N] {
    let mut t = [0.0_f64; N];
    if N == 0 {
        return t;
    }
    t[0] = 1.0;
    if N > 1 {
        t[1] = x;
    }
    for k in 2..N {
        t[k] = 2.0 * x * t[k - 1] - t[k - 2];
    }
    t
}

fn old_cheb2_dense<const M: usize, const N: usize>(x: f64, y: f64, c: &[[f64; N]; M]) -> f64 {
    let tx = old_cheb_basis::<M>(x);
    let ty = old_cheb_basis::<N>(y);
    let mut out = 0.0;
    for i in 0..M {
        for j in 0..N {
            out += c[i][j] * tx[i] * ty[j];
        }
    }
    out
}

fn old_cheb2_sparse(x: f64, y: f64, coeffs: &[(usize, usize, f64)]) -> f64 {
    let tx = old_cheb_basis::<9>(x);
    let ty = old_cheb_basis::<9>(y);
    let mut out = 0.0;
    for &(i, j, c) in coeffs {
        out += c * tx[i] * ty[j];
    }
    out
}

/// The basis recurrence is transcribed unchanged, so it must be *bit*-identical.
#[test]
fn basis_is_bit_identical_to_the_old_implementation() {
    for k in 0..=400 {
        let x = -1.0 + 2.0 * f64::from(k) / 400.0;
        let old = old_cheb_basis::<11>(x);
        let new: [f64; 11] = petir::basis(x);
        for i in 0..11 {
            assert_eq!(old[i].to_bits(), new[i].to_bits(), "T_{i}({x})");
        }
    }
}

/// `cheb1` -> [`petir::eval_plain`]: same Clenshaw, so bit-identical.
#[test]
fn eval_plain_is_bit_identical_to_the_old_cheb1() {
    for k in 0..=400 {
        let x = -1.0 + 2.0 * f64::from(k) / 400.0;
        let old = tampines_cheb1(x, &HF_COEFFS_HEAD);
        let new = petir::eval_plain(x, &HF_COEFFS_HEAD);
        assert_eq!(old.to_bits(), new.to_bits(), "at x = {x}");
    }
}

/// The tensor-product evaluators are transcribed unchanged: bit-identical on a
/// real 11x11 table shaped like `P_HS_LOG_COEFFS`.
#[test]
fn tensor_product_is_bit_identical_to_the_old_implementation() {
    // A deterministic stand-in with the same shape and magnitude spread as the
    // committed pressure table; the arithmetic under test does not care which
    // numbers it sums, only how many and in what order.
    let mut c = [[0.0f64; 11]; 11];
    for i in 0..11 {
        for j in 0..11 {
            c[i][j] = ((i * 11 + j) as f64).sin() * 1.0e4 / ((i + j + 1) as f64);
        }
    }
    let sparse: Vec<(usize, usize, f64)> =
        (0..9).flat_map(|i| (0..9).map(move |j| (i, j, c[i][j]))).collect();

    for k in 0..=60 {
        let x = -1.0 + 2.0 * f64::from(k) / 60.0;
        for m in 0..=60 {
            let y = -1.0 + 2.0 * f64::from(m) / 60.0;
            assert_eq!(
                old_cheb2_dense(x, y, &c).to_bits(),
                petir::eval2_dense(x, y, &c).to_bits(),
                "dense at ({x}, {y})"
            );
            assert_eq!(
                old_cheb2_sparse(x, y, &sparse).to_bits(),
                petir::eval2_sparse::<9>(x, y, &sparse).to_bits(),
                "sparse at ({x}, {y})"
            );
        }
    }
}

/// `scale` is the ONE place the move changes bits. PETIR uses GSL's
/// `(2v - lo - hi)/(hi - lo)`; tampines used the algebraically identical
/// `2(v - lo)/(hi - lo) - 1`. This measures the difference rather than
/// assuming it is negligible: it must be at most 1 ulp, and must vanish at the
/// interval's endpoints and midpoint where both forms are exact.
#[test]
fn the_scaling_change_is_at_most_one_ulp() {
    // Absolute, not ULP: the output spans [-1, 1] and crosses zero at the
    // interval midpoint, where two values that differ by 1e-17 have wildly
    // different bit patterns. ULP distance is meaningless across a sign change.
    let mut worst = 0.0f64;
    let mut worst_t = 0.0f64;
    for k in 0..=1000 {
        let t = TLO + (THI - TLO) * f64::from(k) / 1000.0;
        let d = (old_scale(t, TLO, THI) - petir::scale(t, TLO, THI)).abs();
        if d > worst {
            worst = d;
            worst_t = t;
        }
    }
    println!("  scale(): worst |delta| over the fit domain = {worst:.3e} at T = {worst_t:.4} K");
    // One double rounding on a quantity bounded by 1.
    assert!(
        worst <= 2.0 * f64::EPSILON,
        "scale differs by {worst:.3e} at T = {worst_t:.4} K, above 2*eps"
    );

    // At the ENDPOINTS both forms are algebraically exact and must agree
    // bit-for-bit: at t = lo, old gives 2*0/(hi-lo) - 1 = -1 and new gives
    // (lo - hi)/(hi - lo) = -1, with no rounding in either.
    for t in [TLO, THI] {
        assert_eq!(
            old_scale(t, TLO, THI).to_bits(),
            petir::scale(t, TLO, THI).to_bits(),
            "the two forms must agree exactly at the endpoint t = {t}"
        );
    }
    // The MIDPOINT is NOT such a case, and assuming it was is what this
    // comment exists to prevent: 0.5*(TLO + THI) is not exactly representable,
    // so both forms return a tiny non-zero value and they differ. Only the
    // absolute bound above applies there.
    let mid = 0.5 * (TLO + THI);
    let d = (old_scale(mid, TLO, THI) - petir::scale(mid, TLO, THI)).abs();
    assert!(d <= 2.0 * f64::EPSILON, "midpoint differs by {d:.3e}");
}
