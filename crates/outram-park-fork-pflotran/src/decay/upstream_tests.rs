// Copyright (C) 2026 Theodore Ong and the outram-park contributors. GPL-3.0-only.

//! Tests of the ported upstream PFLOTRAN decay algorithms (gh:#1000):
//! code-to-code against the compiled upstream routines, and analytic checks
//! against CRAM48. Each test states its methodology, its prediction and
//! tolerance (fixed before the first run) and its measured result.

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
    let order = (errs[k - 2] / errs[k - 1]).log2();
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
    let decayed = 1000.0 * (1.0 - 2f64.powf(-2.5));
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
