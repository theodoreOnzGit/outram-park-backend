// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! **V&V — `linalg::expm` (Padé + `DGEBAL` balancing, Octave's `expm.m`)
//! against `depletion::cram48` (OpenMC's IPF CRAM48) on a stiff decay chain**
//! (gh:#999).
//!
//! # Methodology
//!
//! The uranium series, U-238 to Pb-206, as a 15-member linear chain (the
//! 0.021 % Bi-214 -> Tl-210 alpha branch and the 0.00019 % Pb-210 -> Hg-206
//! branch are folded into the main line: they change the matrix's entries,
//! not its stiffness, which is what this test is about). Half-lives span
//! **Po-214's 164.3 us (1.6e-4 s) to U-238's 4.468e9 a (1.4e17 s)**, 21
//! orders of magnitude. Half-lives are rounded literature values (ENDF/B-VIII.0
//! decay sublibrary values to 3-4 figures); they are inputs to a numerical
//! comparison, not data anyone should quote.
//!
//! Start: one unit of pure U-238. Steps `dt` from 1 s to 1e9 a, each solved
//! directly from `t = 0` (no step chaining), by:
//!
//! - `cram48(A, n0, dt)` (one substep, upstream's default);
//! - `expm(A dt) n0`;
//! - the **exact** answer for the first three members (U-238, Th-234,
//!   Pa-234m): linear-chain Bateman, well conditioned because their decay
//!   constants differ by 10^9 and 10^4 (no near-cancellation in
//!   `lambda_j - lambda_i`). Downstream members have no well-conditioned
//!   closed form here (fifteen-term alternating sums), so they are compared
//!   method against method only.
//!
//! Every error is **normwise**, `|x_i - y_i| / sum(n0)`: an absolute error in
//! units of the inventory. Short-lived members sit at relative abundance
//! `lambda_U238 / lambda_i` (Po-214: ~1e-21), far below any normwise
//! resolution, so per-member relative errors on them would measure nothing.
//!
//! # Expectations, written before the first run (2026-10-20)
//!
//! - **CRAM48**: its rational approximation error on the negative real axis
//!   is below round-off (Halphen's constant, `9.289^-48 ~ 1e-46`), and the
//!   shifted solves `(A dt - theta I)` are triangular here, so the error is
//!   round-off independent of stiffness: **~1e-15 normwise at every `dt`**.
//!   Gate: CRAM48 vs exact (first three members) and CRAM48's atom balance,
//!   `1e-13` normwise at every `dt`.
//! - **expm**: Padé [8/8] after `s` squarings, `s ~ log2 ||A dt||_inf`. Here
//!   `||A dt||` reaches ~1e20 (`s = 67`) at `dt = 1e9 a`. The classical
//!   forward-error bound for scaling and squaring grows like `2^s u`, which
//!   is **vacuous** (> 1) for `s > 53`; so the a-priori expectation is that
//!   expm *may* lose all accuracy at the stiffest steps, and nothing better
//!   can be promised. In practice decay matrices are triangular with a
//!   non-negative exponential, and the squarings of such matrices are often
//!   far better behaved than the bound. Gate, fixed from the bound, not from
//!   the result: `|expm - cram48| <= max(1e-12, min(1, 10 * 2^s * u))`
//!   normwise. **The measured numbers are the finding, not the gate.**
//!
//! # Results (2026-10-20, `--release`)
//!
//! Normwise, in units of the initial inventory; `s` is Octave's squaring
//! count for `||A dt||_inf`.
//!
//! | dt | s | CRAM48 vs exact | CRAM48 balance | expm vs CRAM48 | expm balance | Octave 11.3.0 `expm` balance |
//! |---|---|---|---|---|---|---|
//! | 1 s | 13 | 8.88e-16 | 8.88e-16 | 8.88e-16 | 0 | — |
//! | 1 h | 24 | 1.11e-15 | 1.11e-15 | 1.65e-14 | 1.78e-14 | — |
//! | 1 d | 29 | 2.22e-16 | 3.33e-16 | 4.25e-13 | 4.25e-13 | — |
//! | 1 a | 37 | 4.44e-16 | 4.44e-16 | 1.55e-10 | 1.55e-10 | 1.551e-10 |
//! | 1e3 a | 47 | 3.33e-16 | 4.44e-16 | 1.55e-7 | 1.55e-7 | 1.554e-7 |
//! | 1e6 a | 57 | 1.89e-15 | 2.00e-15 | 5.57e-4 | 1.02e-3 | 1.023e-3 |
//! | **1e9 a** | **67** | 2.22e-16 | 1.11e-16 | **6.58e5** | **6.58e5** | **6.579e5** |
//!
//! The Octave column is stock `expm.m` run on the same matrix
//! (`octave-cli`, 11.3.0, 2026-10-20; the script is reproduced in the test
//! `expm_vs_cram48_on_the_uranium_series`'s docs). PETIR's port matches it
//! to the printed digits at every step, so **the failure is the algorithm's,
//! not the port's**.
//!
//! **Interpretation.** CRAM48 is round-off accurate (<= 2e-15) at every
//! step, 1 s to 1e9 a, as expected. `expm`'s error grows steadily with the
//! squaring count, roughly a factor 10^3 per 10 squarings (`~2^s u`, as the
//! classical bound says), from round-off at `s = 13` to 5.6e-4 at `s = 57`;
//! at `s = 67` (1e9 a) **it fails outright: the "inventory" comes out
//! 6.6e5 times the atoms put in**, a non-physical answer with no error
//! raised. That breaches even the vacuous `min(1, ...)` gate fixed before
//! running. **The gate was not moved**: the row is pinned as a measured
//! FAILURE (the test asserts `expm` misses its bound there and that the
//! answer is non-physical), so the record cannot silently go green if
//! PETIR's `expm` changes. Use CRAM for decay chains; `expm` is a
//! cross-check only for steps of modest stiffness (`s` up to ~30 keeps it
//! within ~1e-12 here).

use petir::depletion::{cram48, DepletionMatrix};
use petir::linalg::{expm, Matrix};
use petir::real::{exp, ln};

const YEAR: f64 = 365.25 * 86_400.0;
const DAY: f64 = 86_400.0;
const MIN: f64 = 60.0;

/// The uranium series, U-238 -> ... -> Pb-206, half-lives in seconds
/// (`f64::INFINITY` = stable).
fn uranium_series() -> Vec<(&'static str, f64)> {
    vec![
        ("U-238", 4.468e9 * YEAR),
        ("Th-234", 24.10 * DAY),
        ("Pa-234m", 1.159 * MIN),
        ("U-234", 2.455e5 * YEAR),
        ("Th-230", 7.538e4 * YEAR),
        ("Ra-226", 1600.0 * YEAR),
        ("Rn-222", 3.8235 * DAY),
        ("Po-218", 3.098 * MIN),
        ("Pb-214", 26.8 * MIN),
        ("Bi-214", 19.9 * MIN),
        ("Po-214", 164.3e-6),
        ("Pb-210", 22.2 * YEAR),
        ("Bi-210", 5.012 * DAY),
        ("Po-210", 138.376 * DAY),
        ("Pb-206", f64::INFINITY),
    ]
}

fn lambdas() -> Vec<f64> {
    uranium_series()
        .iter()
        .map(|&(_, h)| if h.is_infinite() { 0.0 } else { ln(2.0) / h })
        .collect()
}

/// Decay matrix, parent by column, daughter by row, per second.
fn decay_matrix(l: &[f64]) -> DepletionMatrix {
    let n = l.len();
    let mut a = DepletionMatrix::zeros(n);
    for j in 0..n {
        a.set(j, j, -l[j]).unwrap();
        if j + 1 < n {
            a.set(j + 1, j, l[j]).unwrap();
        }
    }
    a
}

/// `expm(A dt) n0`, and the number of squarings Octave's rule picks:
/// `[~, e] = log2(norm(A dt, Inf))`, `s = min(max(0, e), 1023)`.
fn expm_apply(a: &DepletionMatrix, n0: &[f64], dt: f64) -> (Vec<f64>, i32) {
    let n = a.order();
    let data: Vec<f64> = a.as_slice().iter().map(|v| v * dt).collect();
    let mut norm_inf = 0.0_f64;
    for i in 0..n {
        let row: f64 = (0..n).map(|j| data[i * n + j].abs()).sum();
        norm_inf = norm_inf.max(row);
    }
    // frexp exponent: norm = f * 2^e with f in [0.5, 1).
    let e = if norm_inf > 0.0 {
        (norm_inf.log2().floor() as i32) + 1
    } else {
        0
    };
    let s = e.clamp(0, 1023);
    let m = Matrix::from_row_major(n, n, data).unwrap();
    let r = expm(&m).unwrap();
    let x = (0..n)
        .map(|i| (0..n).map(|j| r.get(i, j) * n0[j]).sum())
        .collect();
    (x, s)
}

/// Exact U-238, Th-234, Pa-234m from one unit of U-238 (linear Bateman).
fn exact_first_three(l: &[f64], t: f64) -> [f64; 3] {
    let (l1, l2, l3) = (l[0], l[1], l[2]);
    let (e1, e2, e3) = (exp(-l1 * t), exp(-l2 * t), exp(-l3 * t));
    [
        e1,
        l1 / (l2 - l1) * (e1 - e2),
        l1 * l2
            * (e1 / ((l2 - l1) * (l3 - l1))
                + e2 / ((l1 - l2) * (l3 - l2))
                + e3 / ((l1 - l3) * (l2 - l3))),
    ]
}

/// **The comparison**: CRAM48 against the exact answer and its own atom
/// balance (gated at 1e-13), and expm against CRAM48 (gated by the a-priori
/// scaling-and-squaring bound). See the module docs for the methodology,
/// the expectations written before running, and the measured table.
///
/// **The 1e9 a row failed its gate on the first run (2026-10-20)**; the
/// assertion there was then changed, after seeing the result, from "meets
/// the bound" to "misses the bound and is non-physical", so that the
/// failure stays recorded rather than being hidden by a looser bound. The
/// gate for every other row is as fixed before running.
///
/// Octave cross-run of the failing rows (stock `expm.m`, 11.3.0):
///
/// ```text
/// Y=365.25*86400; D=86400; M=60;
/// h=[4.468e9*Y, 24.10*D, 1.159*M, 2.455e5*Y, 7.538e4*Y, 1600*Y, 3.8235*D, ...
///    3.098*M, 26.8*M, 19.9*M, 164.3e-6, 22.2*Y, 5.012*D, 138.376*D, Inf];
/// l=log(2)./h; l(isinf(h))=0; n=numel(l);
/// A=diag(-l); for j=1:n-1, A(j+1,j)=l(j); end
/// n0=zeros(n,1); n0(1)=1;
/// for dt=[1*Y, 1e3*Y, 1e6*Y, 1e9*Y]
///   x=expm(A*dt)*n0; printf("%g a: |sum-1|=%.3e\n", dt/Y, abs(sum(x)-1));
/// end
/// % 1 a: 1.551e-10   1000 a: 1.554e-07   1e+06 a: 1.023e-03   1e+09 a: 6.579e+05
/// ```
#[test]
fn expm_vs_cram48_on_the_uranium_series() {
    let l = lambdas();
    let a = decay_matrix(&l);
    let n = a.order();
    let mut n0 = vec![0.0; n];
    n0[0] = 1.0;
    let u = f64::EPSILON / 2.0;

    let steps = [
        ("1 s", 1.0),
        ("1 h", 3600.0),
        ("1 d", DAY),
        ("1 a", YEAR),
        ("1e3 a", 1e3 * YEAR),
        ("1e6 a", 1e6 * YEAR),
        ("1e9 a", 1e9 * YEAR),
    ];
    println!("| dt | s | CRAM48 vs exact | CRAM48 balance | expm vs CRAM48 | expm balance | expm min entry |");
    println!("|---|---|---|---|---|---|---|");
    for (name, dt) in steps {
        let c = cram48(&a, &n0, dt).unwrap();
        let (x, s) = expm_apply(&a, &n0, dt);
        let ex = exact_first_three(&l, dt);

        let c_exact = (0..3).map(|i| (c[i] - ex[i]).abs()).fold(0.0_f64, f64::max);
        let c_bal = (c.iter().sum::<f64>() - 1.0).abs();
        let x_c = (0..n).map(|i| (x[i] - c[i]).abs()).fold(0.0_f64, f64::max);
        let x_bal = (x.iter().sum::<f64>() - 1.0).abs();
        let x_min = x.iter().cloned().fold(f64::INFINITY, f64::min);
        println!(
            "| {name} | {s} | {c_exact:.2e} | {c_bal:.2e} | {x_c:.2e} | {x_bal:.2e} | {x_min:.2e} |"
        );

        assert!(c_exact < 1e-13, "{name}: CRAM48 vs exact {c_exact}");
        assert!(c_bal < 1e-13, "{name}: CRAM48 balance {c_bal}");
        let bound = (10.0 * 2f64.powi(s) * u).min(1.0).max(1e-12);
        assert!(
            x.iter().all(|v| v.is_finite()),
            "{name}: expm produced a non-finite entry"
        );
        if name == "1e9 a" {
            // Measured FAILURE of the a-priori gate, pinned (module docs):
            // expm (and Octave's own expm.m) returns ~6.6e5 x the inventory.
            assert!(
                x_c > bound && x_bal > 1.0,
                "1e9 a: expm now meets its gate ({x_c} <= {bound}); re-measure and update the docs"
            );
        } else {
            assert!(x_c <= bound, "{name}: expm vs CRAM48 {x_c} > bound {bound}");
        }
    }
}

/// Secular equilibrium at 1e6 a, a physical check on both methods that
/// needs no closed form: each member `i` between U-234 and Po-210 carries
/// activity `lambda_i N_i` equal to U-238's, to the extent `t` is long
/// against every daughter except U-234 (2.455e5 a; at 1e6 a it is
/// `1 - exp(-lambda_U234 t) ~ 94 %` of the way there, so it is excluded) and
/// Th-230 (fed by U-234, likewise excluded). Members from Ra-226 down reach
/// equilibrium with Th-230, not U-238, so the check is the **chain ratio**
/// `lambda_i N_i / (lambda_Th230 N_Th230)` for Ra-226..Po-210, which is 1 to
/// `O(lambda_Th230 / lambda_Ra226) ~ 2e-2` (transient-equilibrium
/// correction). Gate fixed in advance: within 3e-2 for CRAM48. Reported for
/// expm. Th-234 and Pa-234m against U-238: within `lambda_U238/lambda_Th234
/// ~ 1e-11`, gated at 1e-9 for CRAM48.
///
/// Measured 2026-10-20: CRAM48 Th-234/Pa-234m vs U-238 **1.48e-11**
/// (prediction ~1e-11); Ra-226..Po-210 vs Th-230 **CRAM48 6.22e-4**
/// (inside the 3e-2 gate; the 2e-2 estimate was an upper bound), `expm`
/// **3.76e-2** (reported, not gated: outside the CRAM gate, consistent with
/// its 5.6e-4 normwise error at this step landing on members whose share is
/// ~1e-10).
#[test]
fn both_methods_reach_secular_equilibrium() {
    let l = lambdas();
    let a = decay_matrix(&l);
    let n = a.order();
    let mut n0 = vec![0.0; n];
    n0[0] = 1.0;
    let t = 1e6 * YEAR;
    let c = cram48(&a, &n0, t).unwrap();
    let (x, _) = expm_apply(&a, &n0, t);

    let mut worst_c_top = 0.0_f64;
    for i in 1..=2 {
        worst_c_top = worst_c_top.max((l[i] * c[i] / (l[0] * c[0]) - 1.0).abs());
    }
    let mut worst_c = 0.0_f64;
    let mut worst_x = 0.0_f64;
    for i in 5..=13 {
        worst_c = worst_c.max((l[i] * c[i] / (l[4] * c[4]) - 1.0).abs());
        worst_x = worst_x.max((l[i] * x[i] / (l[4] * x[4]) - 1.0).abs());
    }
    println!(
        "secular: CRAM48 Th-234/Pa-234m vs U-238 {worst_c_top:.2e}; Ra-226..Po-210 vs Th-230: \
         CRAM48 {worst_c:.2e}, expm {worst_x:.2e}"
    );
    assert!(
        worst_c_top < 1e-9,
        "CRAM48 top-of-chain equilibrium {worst_c_top}"
    );
    assert!(worst_c < 3e-2, "CRAM48 Ra..Po equilibrium {worst_c}");
}
