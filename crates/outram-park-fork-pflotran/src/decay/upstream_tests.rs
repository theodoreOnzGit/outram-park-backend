// Copyright (C) 2026 Theodore Ong and the outram-park contributors. GPL-3.0-only.

//! Tests of the ported upstream PFLOTRAN decay algorithms (gh:#1000):
//! code-to-code against the compiled upstream routines, and analytic checks
//! against CRAM48. Each test states its methodology, its prediction and
//! tolerance (fixed before the first run) and its measured result.

use super::*;
use crate::decay::{DecayChain, DecaySolver, Nuclide};

const MINUTE: f64 = 60.0;
const HOUR: f64 = 3600.0;
const DAY: f64 = 86_400.0;
const YEAR: f64 = 365.25 * DAY;

/// The compiled upstream routines' output (`run.sh`, gfortran 16.2.1).
const UPSTREAM: &str =
    include_str!("../../verification_and_validation/upstream_decay/upstream_output.txt");

fn nuc(name: &str, hl: f64, daughters: Vec<(usize, f64)>) -> Nuclide {
    Nuclide {
        name: name.to_string(),
        half_life_seconds: hl,
        daughters,
    }
}

/// The driver's `radon` / `radon0` chain.
fn radon() -> DecayChain {
    DecayChain::new(vec![
        nuc("Rn-222", 3.8235 * DAY, vec![(1, 1.0)]),
        nuc("Po-218", 3.098 * MINUTE, vec![(2, 1.0)]),
        nuc("Pb-214", 26.8 * MINUTE, vec![(3, 1.0)]),
        nuc("Bi-214", 19.9 * MINUTE, vec![(4, 1.0)]),
        nuc("Pb-210", 22.2 * YEAR, vec![]),
    ])
    .unwrap()
}

/// The driver's `thc` chain (thorium C branch).
fn thorium_c() -> DecayChain {
    DecayChain::new(vec![
        nuc("Pb-212", 10.64 * HOUR, vec![(1, 1.0)]),
        nuc("Bi-212", 60.55 * MINUTE, vec![(2, 0.6406), (3, 0.3594)]),
        nuc("Po-212", 0.299e-6, vec![(4, 1.0)]),
        nuc("Tl-208", 3.053 * MINUTE, vec![(4, 1.0)]),
        nuc("Pb-208", f64::INFINITY, vec![]),
    ])
    .unwrap()
}

/// The driver's `conv` chain (two parents, one daughter).
fn converging() -> DecayChain {
    DecayChain::new(vec![
        nuc("P1", 4.0, vec![(2, 1.0)]),
        nuc("P2", 9.0, vec![(2, 1.0)]),
        nuc("D", f64::INFINITY, vec![]),
    ])
    .unwrap()
}

/// `(name, chain, initial inventory, dt)` exactly as `driver.F90`.
fn driver_cases() -> Vec<(&'static str, DecayChain, Vec<f64>, f64)> {
    vec![
        ("radon", radon(), vec![1000.0, 1.0, 1.0, 1.0, 1.0], HOUR),
        ("radon0", radon(), vec![1000.0, 0.0, 0.0, 0.0, 0.0], HOUR),
        ("thc", thorium_c(), vec![1000.0, 1.0, 1.0, 1.0, 1.0], 600.0),
        ("conv", converging(), vec![10.0, 20.0, 1.0], 1.0),
    ]
}

/// Upstream's masses for `(case, tag)`, `tag` `E` or `I`, as bit patterns:
/// `out[step - 1][isotope - 1]`.
fn upstream_masses(case: &str, tag: &str) -> Vec<Vec<u64>> {
    let mut out: Vec<Vec<u64>> = Vec::new();
    for line in UPSTREAM.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() == 5 && f[0] == case && f[1] == tag {
            let step: usize = f[2].parse().unwrap();
            if out.len() < step {
                out.resize(step, Vec::new());
            }
            out[step - 1].push(u64::from_str_radix(f[4], 16).unwrap());
        }
    }
    out
}

/// Upstream's Newton iteration count per step (`N` lines, decimal).
fn upstream_iterations(case: &str) -> Vec<usize> {
    UPSTREAM
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>())
        .filter(|f| f.len() == 5 && f[0] == case && f[1] == "N")
        .map(|f| f[4].parse().unwrap())
        .collect()
}

fn normwise(x: &[f64], y: &[f64], total: f64) -> f64 {
    x.iter()
        .zip(y)
        .map(|(a, b)| (a - b).abs() / total)
        .fold(0.0, f64::max)
}

/// Run `steps` steps of `solver` from `c0`.
fn march(chain: &DecayChain, solver: DecaySolver, c0: &[f64], dt: f64, steps: usize) -> Vec<f64> {
    let mut c = c0.to_vec();
    for _ in 0..steps {
        chain.decay_with(solver, &mut c, dt).unwrap();
    }
    c
}

/// The decay constants agree bit for bit with upstream's HALF_LIFE card
/// (`-1.d0*log(0.5d0)/half_life`, `pm_ufd_decay.F90` line 480): the premise
/// of every bit-identity test below. Measured 2026-10-20: 18 of 18 bit-equal.
#[test]
fn decay_constants_match_upstreams_half_life_card() {
    let mut checked = 0;
    for (name, chain, _, _) in driver_cases() {
        for line in UPSTREAM.lines().filter(|l| l.starts_with("L ")) {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f[1] != name {
                continue;
            }
            let i: usize = f[2].parse().unwrap();
            let bits = u64::from_str_radix(f[3], 16).unwrap();
            assert_eq!(
                chain.decay_constant(i - 1).to_bits(),
                bits,
                "{name} isotope {i}"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 18);
}

/// **Code-to-code, UFD_DECAY default (3-generation explicit).**
///
/// Methodology: every case of `driver.F90` (radon chain from a positive and
/// from a pure-parent inventory, 10 steps of 1 h; thorium C, 10 steps of
/// 600 s; two converging parents, 5 steps of 1 s) is marched with
/// [`DecaySolver::UfdThreeGeneration`], and every mass after every step is
/// compared with the compiled upstream routine's, as bit patterns.
///
/// Prediction, written before the first run: **bit-identical, 165 of 165**.
/// The expressions are evaluated in upstream's order, neither side contracts
/// to FMA, and both `exp`s are ARM optimized-routines' (glibc's since 2.28,
/// and PETIR's port). Tolerance: none, bit equality.
///
/// Measured 2026-10-20: **165 of 165 bit-identical**, against
/// both Fortran builds (`run.sh`; this path's `exp` calls stay scalar in
/// both).
#[test]
fn ufd_three_generation_matches_compiled_upstream_bit_for_bit() {
    let mut equal = 0;
    for (name, chain, c0, dt) in driver_cases() {
        let up = upstream_masses(name, "E");
        assert_eq!(up.len(), if name == "conv" { 5 } else { 10 });
        let mut c = c0.clone();
        for (step, want) in up.iter().enumerate() {
            chain
                .decay_with(DecaySolver::UfdThreeGeneration, &mut c, dt)
                .unwrap();
            for (i, (x, w)) in c.iter().zip(want).enumerate() {
                assert_eq!(
                    x.to_bits(),
                    *w,
                    "{name} step {} isotope {}: {x:e} vs upstream {:e}",
                    step + 1,
                    i + 1,
                    f64::from_bits(*w)
                );
                equal += 1;
            }
        }
    }
    assert_eq!(equal, 165);
}

/// **Code-to-code, UFD_DECAY `IMPLICIT_SOLUTION`.**
///
/// Methodology: the `driver.F90` cases with a positive inventory (radon,
/// thorium C, converging; upstream cannot start from a zero, see the module
/// docs) marched with [`ufd_implicit`]; every mass after every step compared
/// bit for bit, and every step's Newton iteration count compared with
/// upstream's `it`.
///
/// Prediction, written before the first run: **bit-identical, 115 of 115
/// masses and 25 of 25 iteration counts.** Same reasoning as the 3-generation
/// test, plus: the LU is ported line for line, so its pivoting and summation
/// order are upstream's. Tolerance: none.
///
/// Measured 2026-10-20: **115 of 115 masses and 25 of 25 iteration
/// counts bit-identical** (counts 4 to 25; 25 at thorium C's first step).
/// The first run compared against the plain `-O2` build instead and failed
/// by 1 ulp at radon step 5, isotope 4: see the next test.
#[test]
fn ufd_implicit_matches_compiled_upstream_bit_for_bit() {
    let (mut equal, mut counts) = (0, 0);
    for (name, chain, c0, dt) in driver_cases() {
        if name == "radon0" {
            continue;
        }
        let up = upstream_masses(name, "I");
        let its = upstream_iterations(name);
        assert_eq!(its.len(), up.len());
        let daughters = chain.daughter_lists();
        let mut c = c0.clone();
        for (step, want) in up.iter().enumerate() {
            let (next, it) = ufd_implicit(&chain.lambda, &daughters, &c, dt).unwrap();
            c = next;
            assert_eq!(it, its[step], "{name} step {} iterations", step + 1);
            counts += 1;
            for (i, (x, w)) in c.iter().zip(want).enumerate() {
                assert_eq!(
                    x.to_bits(),
                    *w,
                    "{name} step {} isotope {}: {x:e} vs upstream {:e}",
                    step + 1,
                    i + 1,
                    f64::from_bits(*w)
                );
                equal += 1;
            }
        }
        // `decay_with` is the same routine.
        assert_eq!(
            march(&chain, DecaySolver::UfdImplicit, &c0, dt, up.len()),
            c
        );
    }
    assert_eq!((equal, counts), (115, 25));
}

/// The plain `-O2` build of the same Fortran (`run.sh`).
const UPSTREAM_O2_VECTOR: &str =
    include_str!("../../verification_and_validation/upstream_decay/upstream_output_O2_vector.txt");

/// **Code-to-code, IMPLICIT_SOLUTION, against the vectorised `-O2` build.**
///
/// Methodology: as the test above, against `upstream_output_O2_vector.txt`,
/// where gfortran turned the array statement `solution =
/// solution*exp(-rhs)` (line 1989) into calls to glibc libmvec's
/// `_ZGVbN2v_exp`, which is not bit-identical to scalar `exp` (found while
/// debugging the first run of the test above, which compared against this
/// build and failed by 1 ulp at radon step 5; `run.sh` now builds both). The
/// two Fortran builds already differ from each other in 19 of 115 masses and
/// in no iteration count, so that much was known before this test ran.
///
/// Prediction and tolerance, fixed before running: 96 of 115 masses
/// bit-equal, the same 25 iteration counts, worst relative difference
/// `< 1e-13` (libmvec `exp` is within 4 ulp; the Newton's fixed point damps
/// rather than amplifies a perturbation). Measured 2026-10-20: 96 of 115 bit-equal, worst relative
/// difference 2.36e-16 (1 ulp), iteration counts identical.
#[test]
fn ufd_implicit_against_the_vectorised_o2_build() {
    let (mut equal, mut total, mut worst) = (0, 0, 0.0_f64);
    for (name, chain, c0, dt) in driver_cases() {
        if name == "radon0" {
            continue;
        }
        let mut want_its = Vec::new();
        let mut want: Vec<Vec<u64>> = Vec::new();
        for f in UPSTREAM_O2_VECTOR
            .lines()
            .map(|l| l.split_whitespace().collect::<Vec<_>>())
        {
            if f.len() == 5 && f[0] == name && f[1] == "N" {
                want_its.push(f[4].parse::<usize>().unwrap());
            }
            if f.len() == 5 && f[0] == name && f[1] == "I" {
                let step: usize = f[2].parse().unwrap();
                if want.len() < step {
                    want.resize(step, Vec::new());
                }
                want[step - 1].push(u64::from_str_radix(f[4], 16).unwrap());
            }
        }
        let mut c = c0.clone();
        for (step, w) in want.iter().enumerate() {
            let (next, it) = ufd_implicit(&chain.lambda, &chain.daughter_lists(), &c, dt).unwrap();
            c = next;
            assert_eq!(it, want_its[step], "{name} step {}", step + 1);
            for (x, wb) in c.iter().zip(w) {
                let y = f64::from_bits(*wb);
                total += 1;
                if x.to_bits() == *wb {
                    equal += 1;
                }
                worst = worst.max((x - y).abs() / y.abs());
            }
        }
    }
    println!("vs -O2 vectorised build: {equal} of {total} bit-equal, worst relative {worst:.2e}");
    assert_eq!(total, 115);
    assert!(worst < 1e-13, "{worst}");
}

/// Where upstream hangs or aborts, this port returns an error (module docs
/// item 4): a zero daughter makes the Newton cycle (the extracted Fortran ran
/// for over 1.9e6 iterations without stopping, `run.sh`), and a zero
/// parentless nuclide gives a zero Jacobian row (upstream: `MPI_Abort`).
#[test]
fn ufd_implicit_reports_where_upstream_hangs_or_aborts() {
    let pair =
        DecayChain::new(vec![nuc("A", 50.0, vec![(1, 1.0)]), nuc("B", 8.0, vec![])]).unwrap();
    let mut c = vec![1000.0, 0.0];
    let err = pair
        .decay_with(DecaySolver::UfdImplicit, &mut c, 1.0)
        .unwrap_err();
    assert!(
        matches!(&err, PflotranError::Convergence(m) if m.contains("did not converge in 1000")),
        "{err:?}"
    );
    assert_eq!(c, vec![1000.0, 0.0], "inventory untouched on error");
    let lone = DecayChain::new(vec![nuc("A", 50.0, vec![])]).unwrap();
    let err = lone
        .decay_with(DecaySolver::UfdImplicit, &mut [0.0], 1.0)
        .unwrap_err();
    assert!(matches!(&err, PflotranError::Convergence(m) if m.contains("row 0 is zero")));
}

/// Module docs item 5, upstream's quirk carried over: `prev_solution` starts
/// at 1, so an inventory of ones is "converged" before the first iteration
/// and comes back undecayed, after 0 iterations.
#[test]
fn ufd_implicit_skips_an_inventory_of_ones() {
    let chain =
        DecayChain::new(vec![nuc("A", 1.0, vec![(1, 1.0)]), nuc("B", 2.0, vec![])]).unwrap();
    let (out, it) =
        ufd_implicit(&chain.lambda, &chain.daughter_lists(), &[1.0, 1.0], 10.0).unwrap();
    assert_eq!((out, it), (vec![1.0, 1.0], 0));
}

/// Upstream's setup guards (lines 1243-1262) and this port's equal-rate guard
/// (module docs item 3) refuse before computing.
#[test]
fn ufd_guards_refuse_self_ancestry_and_equal_rates() {
    let selfish = DecayChain::new(vec![nuc("A", 5.0, vec![(0, 1.0)])]).unwrap();
    for solver in [DecaySolver::UfdThreeGeneration, DecaySolver::UfdImplicit] {
        let err = selfish.decay_with(solver, &mut [1.0], 1.0).unwrap_err();
        assert!(matches!(&err, PflotranError::InvalidInput(m) if m.contains("its parent")));
    }
    let cycle = DecayChain::new(vec![
        nuc("A", 5.0, vec![(1, 1.0)]),
        nuc("B", 7.0, vec![(0, 1.0)]),
    ])
    .unwrap();
    let err = cycle
        .decay_with(DecaySolver::UfdImplicit, &mut [1.0, 2.0], 1.0)
        .unwrap_err();
    assert!(matches!(&err, PflotranError::InvalidInput(m) if m.contains("grandparent")));
    let twins =
        DecayChain::new(vec![nuc("A", 5.0, vec![(1, 1.0)]), nuc("B", 5.0, vec![])]).unwrap();
    let err = twins
        .decay_with(DecaySolver::UfdThreeGeneration, &mut [1.0, 0.0], 1.0)
        .unwrap_err();
    assert!(matches!(&err, PflotranError::InvalidInput(m) if m.contains("share the decay")));
    // The implicit path divides by no difference of rates, and runs.
    twins
        .decay_with(DecaySolver::UfdImplicit, &mut [1.0, 0.5], 1.0)
        .unwrap();
}

/// **The 3-generation form is exact for depth <= 3.**
///
/// Methodology: A (30 s) -> B (7 s) -> C (90 s) -> out of the network, unit
/// branching, from (1000, 50, 20); one step of 40 s, and 20 steps of 2 s,
/// against one CRAM48 step of 40 s (round-off-exact on this matrix, gh:#999).
///
/// Prediction and tolerance, fixed before running: normwise
/// (`|diff| / 1070`) `< 1e-13`, round-off only, since the closed form has no
/// truncation at depth 3.
///
/// Measured 2026-10-20: one step 3.72e-16, 20 steps 7.44e-16.
#[test]
fn ufd_three_generation_is_exact_for_depth_three() {
    let chain = DecayChain::new(vec![
        nuc("A", 30.0, vec![(1, 1.0)]),
        nuc("B", 7.0, vec![(2, 1.0)]),
        nuc("C", 90.0, vec![]),
    ])
    .unwrap();
    let c0 = [1000.0, 50.0, 20.0];
    for (dt, steps) in [(40.0, 1), (2.0, 20)] {
        let ufd = march(&chain, DecaySolver::UfdThreeGeneration, &c0, dt, steps);
        let exact = march(&chain, DecaySolver::Cram48, &c0, 40.0, 1);
        let e = normwise(&ufd, &exact, 1070.0);
        println!("depth 3, {steps} x {dt} s: UFD vs CRAM48 {e:.2e}");
        assert!(e < 1e-13, "{e}");
    }
}

/// **The 3-generation truncation on a 5-member chain, against CRAM48.**
///
/// Methodology: the radon chain from 1000 units of Rn-222, marched to
/// `T = 2 h` in `N` equal steps, `N = 12, 24, 48, 96, 192, 384`, against one
/// CRAM48 step of `T`. Within a step, ingrowth through a great-grandparent is
/// dropped: Bi-214 misses Rn-222's contribution, and Pb-210 misses Po-218's
/// and Rn-222's. The leading dropped term is
/// `lambda_Rn lambda_Po lambda_Pb N_Rn dt^3 / 6` per step, so the local error
/// is `O(dt^3)` and the error at fixed `T` is `O(dt^2)`.
///
/// Prediction, written before running: the error at `T` falls 4x per halving
/// of `dt` once `lambda_Po dt` is small; **observed order between the two
/// finest step sizes in [1.8, 2.2]** (the tolerance). Magnitude at `N = 12`
/// (`dt` = 10 min): ~1e-4 of the inventory.
///
/// Measured 2026-10-20: (normwise, `|diff| / 1000`)
///
/// | `N` | `dt` | error |
/// |---|---|---|
/// | 12 | 600 s | 7.34e-4 |
/// | 24 | 300 s | 2.42e-4 |
/// | 48 | 150 s | 7.04e-5 |
/// | 96 | 75 s | 1.91e-5 |
/// | 192 | 37.5 s | 4.96e-6 |
/// | 384 | 18.75 s | 1.27e-6 |
///
/// Observed order, finest pair: **1.970**, inside [1.8, 2.2]. The
/// magnitude prediction at `N = 12` was 7x low (7.34e-4, not ~1e-4): it
/// used the leading `dt^3` term at `lambda_Po dt = 2.2`, outside that
/// term's range.
#[test]
fn ufd_three_generation_truncation_is_second_order_at_fixed_time() {
    let chain = radon();
    let c0 = [1000.0, 0.0, 0.0, 0.0, 0.0];
    let t = 2.0 * HOUR;
    let exact = march(&chain, DecaySolver::Cram48, &c0, t, 1);
    let mut errs = Vec::new();
    for n in [12_usize, 24, 48, 96, 192, 384] {
        let ufd = march(
            &chain,
            DecaySolver::UfdThreeGeneration,
            &c0,
            t / n as f64,
            n,
        );
        let e = normwise(&ufd, &exact, 1000.0);
        println!("radon UFD 3-gen, N = {n}: {e:.3e}");
        errs.push(e);
    }
    let k = errs.len();
    let order = (errs[k - 2] / errs[k - 1]).r_log2();
    println!("observed order (finest pair): {order:.3}");
    assert!((1.8..=2.2).contains(&order), "order {order}");
}

/// Module docs item 1, upstream's quirk carried over: the 3-generation form
/// never reads the branching fractions.
///
/// Methodology: P (10 s) -> {D1 0.6, D2 0.4}, both stable, from 1000 P, one
/// step of 25 s, against CRAM48. Prediction (exact algebra; tolerance 1e-12
/// relative): D1 = D2 = the whole decayed amount `1000 (1 - 2^-2.5)`, where
/// the true answer is 0.6 and 0.4 of it, so the network gains exactly the
/// decayed amount. Measured 2026-10-20: as predicted: D1 = D2 = 823.2233,
/// and the network gains 823.2233 atoms, 82 % of the initial inventory.
#[test]
fn ufd_three_generation_ignores_branching_like_upstream() {
    let chain = DecayChain::new(vec![
        nuc("P", 10.0, vec![(1, 0.6), (2, 0.4)]),
        nuc("D1", f64::INFINITY, vec![]),
        nuc("D2", f64::INFINITY, vec![]),
    ])
    .unwrap();
    let c0 = [1000.0, 0.0, 0.0];
    let ufd = march(&chain, DecaySolver::UfdThreeGeneration, &c0, 25.0, 1);
    let cram = march(&chain, DecaySolver::Cram48, &c0, 25.0, 1);
    let decayed = 1000.0 * (1.0 - 2f64.r_powf(-2.5));
    let rel = |a: f64, b: f64| (a - b).abs() / b;
    assert!(
        rel(ufd[1], decayed) < 1e-12 && rel(ufd[2], decayed) < 1e-12,
        "{ufd:?}"
    );
    assert!(rel(cram[1], 0.6 * decayed) < 1e-12 && rel(cram[2], 0.4 * decayed) < 1e-12);
    let gain = ufd.iter().sum::<f64>() - 1000.0;
    println!("UFD 3-gen branching gain: {gain:.4} atoms");
    assert!(rel(gain, decayed) < 1e-12);
}

/// **Backward Euler's per-step error is `(lambda dt)^2 / 2`.**
///
/// Methodology: one nuclide, one step, `x = lambda dt` in {1e-1, 1e-2, 1e-3}.
/// Backward Euler gives `1 / (1 + x)`, the exact answer is `e^-x`, and their
/// difference is `x^2/2 - 5x^3/6 + O(x^4)`. Prediction and tolerance, fixed
/// before running: `|err / (x^2/2) - 1| < 2x` (the next term is `5x/3`
/// relative). Measured 2026-10-20: `err / (x^2/2) - 1` = -1.493e-1, -1.648e-2,
/// -1.665e-3, i.e. `-5x/3` to two digits, as the series says.
#[test]
fn backward_euler_local_error_is_half_lambda_dt_squared() {
    let chain = DecayChain::new(vec![nuc("A", 100.0, vec![])]).unwrap();
    let lam = chain.decay_constant(0);
    for x in [1e-1, 1e-2, 1e-3] {
        let dt = x / lam;
        let be = march(&chain, DecaySolver::BackwardEuler, &[1.0], dt, 1)[0];
        let err = be - (-x).r_exp();
        let ratio = err / (x * x / 2.0) - 1.0;
        println!("BE x = {x:e}: err {err:.4e}, err / (x^2/2) - 1 = {ratio:.4e}");
        assert!(ratio.abs() < 2.0 * x, "x = {x}: {ratio}");
    }
}

/// **Backward Euler converges at first order, and conserves atoms.**
///
/// Methodology: the radon chain from 1000 Rn-222 to `T = 2 h` in `N` steps,
/// `N = 12 ... 384`, against one CRAM48 step; and thorium C (all branching
/// stays in the network) over 10 steps of 600 s for the atom balance.
///
/// Prediction and tolerances, fixed before running: observed order between
/// the two finest step sizes in [0.9, 1.1]; atom balance `< 1e-13` of the
/// inventory (the columns of `I - dt A` sum to 1, so `sum N1 = sum N0` up to
/// round-off). Magnitude at `N = 12`: ~1e-2 of the inventory, about 100x worse
/// than the 3-generation form at the same step.
///
/// Measured 2026-10-20: (normwise, `|diff| / 1000`)
///
/// | `N` | `dt` | backward Euler | 3-generation (test above) |
/// |---|---|---|---|
/// | 12 | 600 s | 2.32e-4 | 7.34e-4 |
/// | 24 | 300 s | 1.18e-4 | 2.42e-4 |
/// | 48 | 150 s | 5.94e-5 | 7.04e-5 |
/// | 96 | 75 s | 2.98e-5 | 1.91e-5 |
/// | 192 | 37.5 s | 1.49e-5 | 4.96e-6 |
/// | 384 | 18.75 s | 7.48e-6 | 1.27e-6 |
///
/// Observed order, finest pair: **0.999**, inside [0.9, 1.1]. Atom balance
/// 2.26e-16. **The magnitude prediction was wrong, in direction too:**
/// backward Euler is 2.32e-4 at `N = 12`, 43x below the ~1e-2 predicted
/// and 3.2x *better* than the 3-generation form at the same step, not
/// 100x worse. On this 5-member chain the 3-generation form drops
/// great-grandparent ingrowth outright, while backward Euler only damps
/// it; the 3-generation form wins only below `dt` ~ 100 s (crossover
/// between `N = 48` and `N = 96`).
#[test]
fn backward_euler_is_first_order_and_conservative() {
    let chain = radon();
    let c0 = [1000.0, 0.0, 0.0, 0.0, 0.0];
    let t = 2.0 * HOUR;
    let exact = march(&chain, DecaySolver::Cram48, &c0, t, 1);
    let mut errs = Vec::new();
    for n in [12_usize, 24, 48, 96, 192, 384] {
        let be = march(&chain, DecaySolver::BackwardEuler, &c0, t / n as f64, n);
        let e = normwise(&be, &exact, 1000.0);
        println!("radon BE, N = {n}: {e:.3e}");
        errs.push(e);
    }
    let k = errs.len();
    let order = (errs[k - 2] / errs[k - 1]).r_log2();
    println!("observed order (finest pair): {order:.3}");
    assert!((0.9..=1.1).contains(&order), "order {order}");

    let thc = thorium_c();
    let start = [1000.0, 1.0, 1.0, 1.0, 1.0];
    let be = march(&thc, DecaySolver::BackwardEuler, &start, 600.0, 10);
    let bal = (be.iter().sum::<f64>() - 1004.0).abs() / 1004.0;
    println!("thorium C BE atom balance: {bal:.2e}");
    assert!(bal < 1e-13, "{bal}");
}

/// **UFD `IMPLICIT_SOLUTION` is backward Euler, up to its Newton stop.**
///
/// Methodology: the three positive-inventory driver cases, marched with
/// [`DecaySolver::UfdImplicit`] and with [`DecaySolver::BackwardEuler`] (the
/// direct solve of the same equations).
///
/// Prediction and tolerance, fixed before running: per-entry relative
/// difference `< 1e-9`. The Newton stops once the relative change is below
/// `1e-6` and converges quadratically, so the remaining error should be about
/// 1e-12. Measured 2026-10-20: worst relative difference
/// 1.69e-14 (radon), 9.02e-14 (thorium C), 2.40e-13 (converging): inside
/// the tolerance, near the ~1e-12 predicted.
#[test]
fn ufd_implicit_is_backward_euler_to_its_newton_tolerance() {
    for (name, chain, c0, dt) in driver_cases() {
        if name == "radon0" {
            continue;
        }
        let steps = if name == "conv" { 5 } else { 10 };
        let ufd = march(&chain, DecaySolver::UfdImplicit, &c0, dt, steps);
        let be = march(&chain, DecaySolver::BackwardEuler, &c0, dt, steps);
        let worst = ufd
            .iter()
            .zip(&be)
            .map(|(a, b)| (a - b).abs() / b.abs())
            .fold(0.0, f64::max);
        println!("{name}: UFD implicit vs BE, worst relative {worst:.2e}");
        assert!(worst < 1e-9, "{name}: {worst}");
    }
}

/// The ported LU solves a system that needs pivoting, and refuses a zero row
/// (upstream aborts there).
#[test]
fn ported_lu_solves_and_refuses_a_zero_row() {
    // [[1, 2, 3], [4, 5, 6], [7, 8, 10]] x = [6, 15, 25] -> x = (1, 1, 1).
    let mut a = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 10.0];
    let mut indx = vec![0; 3];
    lu_decomposition(&mut a, 3, &mut indx).unwrap();
    assert_ne!(indx, vec![0, 1, 2], "pivoting exercised");
    let mut b = vec![6.0, 15.0, 25.0];
    lu_back_substitution(&a, 3, &indx, &mut b);
    for x in b {
        assert!((x - 1.0).abs() < 1e-14, "{x}");
    }
    let mut z = vec![1.0, 2.0, 0.0, 0.0];
    assert!(lu_decomposition(&mut z, 2, &mut [0, 0]).is_err());
}
