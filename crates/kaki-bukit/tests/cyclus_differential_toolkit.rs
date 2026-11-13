// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! Toolkit-kernel differential against CYCLUS 1.6.0 (gh:#1431).
//!
//! # Methodology
//!
//! `verification_and_validation/cyclus_differential_gh1431/harness.cc` calls
//! CYCLUS's own routines (enrichment toolkit, `KahanSum`, `compmath`,
//! `Composition::Decay`) from the conda `libcyclus.so` and prints every result
//! at 17 significant digits to `cyclus_output/toolkit.txt`. The tests here
//! call the port with the same inputs and compare. Predictions were committed
//! first (`PREDICTIONS.md`, rows C3 and C4). Pass criteria are the predicted
//! bounds and are not relaxed.
//!
//! # Results (2026-11-13)
//!
//! See `README.md` beside the data.

use kaki_bukit::arithmetic::kahan_sum;
use kaki_bukit::comp_math::{self, CompMap};
use kaki_bukit::composition::{AtomicMasses, Composition};
use kaki_bukit::nuclide::Nuc;
use kaki_bukit::decay::{decay_composition, decay_composition_with_tol, DecayChain, DecayData, DEFAULT_TOL};
use kaki_bukit::toolkit::enrichment::{feed_qty, swu_required, tails_qty, value_func, Assays};

const TOOLKIT: &str = include_str!(
    "../verification_and_validation/cyclus_differential_gh1431/cyclus_output/toolkit.txt"
);

/// Lines of the harness output whose first word is `tag`, split on spaces
/// with the tag removed.
fn lines(tag: &str) -> Vec<Vec<&'static str>> {
    TOOLKIT
        .lines()
        .filter(|l| l.split(' ').next() == Some(tag))
        .map(|l| l.split(' ').skip(1).collect())
        .collect()
}

fn rel(a: f64, b: f64) -> f64 {
    if a == b {
        0.0
    } else {
        ((a - b) / b).abs()
    }
}

/// C4. Enrichment toolkit at ten product assays (feed 0.00711, tails 0.0025)
/// and product quantities 1 and 37.5 kg: `swu`, `feed`, `tails`, plus the
/// value function. Prediction: <= 1e-14 relative.
#[test]
fn c4_enrichment_matches_cyclus() {
    let a = |p: f64| Assays::new(0.00711, p, 0.0025);
    let mut n = 0;
    let mut identical = 0;
    let mut worst = 0.0_f64;
    for (tag, which) in [("swu", 0), ("feed", 1), ("tails", 2)] {
        for l in lines(tag) {
            let p: f64 = l[0].parse().unwrap();
            let q: f64 = l[1].parse().unwrap();
            let want: f64 = l[2].parse().unwrap();
            let got = match which {
                0 => swu_required(q, &a(p)).unwrap(),
                1 => feed_qty(q, &a(p)),
                _ => tails_qty(q, &a(p)),
            };
            n += 1;
            identical += usize::from(got.to_bits() == want.to_bits());
            worst = worst.max(rel(got, want));
        }
    }
    for l in lines("value") {
        let p: f64 = l[0].parse().unwrap();
        let want: f64 = l[1].parse().unwrap();
        let got = value_func(p).unwrap();
        n += 1;
        identical += usize::from(got.to_bits() == want.to_bits());
        worst = worst.max(rel(got, want));
    }
    std::eprintln!("enrichment: n={n} bit-identical={identical} worst_rel={worst:e}");
    assert_eq!(n, 70);
    assert!(worst <= 1e-14, "worst relative difference {worst:e}");
}

/// C3a. `KahanSum` of a badly conditioned list; bit-identical predicted.
#[test]
fn c3a_kahan_sum_matches_cyclus() {
    let mut v = vec![1.0];
    for i in 0..1000 {
        v.push(1e-16 * f64::from(1 + i % 7));
    }
    v.extend([-1.0, 0.1, 0.2, 0.3]);
    let want: f64 = lines("kahan")[0][0].parse().unwrap();
    assert_eq!(kahan_sum(&v).to_bits(), want.to_bits());
}

fn comp(pairs: &[(i32, f64)]) -> CompMap {
    pairs.iter().map(|(n, v)| (Nuc(*n), *v)).collect()
}

/// C3b. `comp_math` add, sub, sum and normalise; bit-identical predicted.
#[test]
fn c3b_comp_math_matches_cyclus() {
    let a = comp(&[(922350000, 0.0355), (922380000, 0.9645), (942390000, 1e-3)]);
    let b = comp(&[(922350000, 0.1), (922380000, 0.3), (942400000, 0.7)]);
    let s = comp_math::add(&a, &b);
    let d = comp_math::sub(&s, &b);
    let mut n = s.clone();
    comp_math::normalize(&mut n, 1.0);
    let want_sum: f64 = lines("sum_a")[0][0].parse().unwrap();
    assert_eq!(comp_math::sum(&a).to_bits(), want_sum.to_bits());
    for (tag, m) in [("add", &s), ("sub", &d), ("norm", &n)] {
        let rows = lines(tag);
        assert_eq!(rows.len(), m.len(), "{tag}: nuclide count");
        for r in rows {
            let nuc: i32 = r[0].parse().unwrap();
            let want: f64 = r[1].parse().unwrap();
            assert_eq!(m[&Nuc(nuc)].to_bits(), want.to_bits(), "{tag} {nuc}");
        }
    }
}

/// The decay chain the harness read out of pyne, and the starting
/// composition: Sr-90 1.0, Y-90 0.2, Cs-137 0.7, Ba-137 0.05, Co-60 0.4.
fn decay_setup() -> (DecayChain, Composition) {
    let mut chain = DecayChain::new();
    for l in lines("dc") {
        let nuc: i32 = l[0].parse().unwrap();
        let dc: f64 = l[1].parse().unwrap();
        if dc > 0.0 {
            let branches: Vec<(Nuc, f64)> = lines("br")
                .iter()
                .filter(|b| b[0] == l[0])
                .map(|b| (Nuc(b[1].parse().unwrap()), b[2].parse().unwrap()))
                .collect();
            chain.insert(Nuc(nuc), DecayData::new(dc, branches));
        }
    }
    let c0 = comp(&[
        (380900000, 1.0),
        (390900000, 0.2),
        (551370000, 0.7),
        (561370000, 0.05),
        (270600000, 0.4),
    ]);
    let comp0 = Composition::from_atom(c0, &AtomicMasses::MassNumber).unwrap();
    (chain, comp0)
}

/// Compares `out` with the harness rows tagged `tag` for `steps`; returns
/// (compared, bit-identical, worst relative difference). A nuclide the port
/// holds but CYCLUS omits must be below `tol_absent`.
fn compare_decay(
    out: &Composition,
    tag: &str,
    steps: u32,
    tol_absent: f64,
) -> (usize, usize, f64) {
    let rows: Vec<(i32, f64)> = lines(tag)
        .iter()
        .filter(|r| r[0] == steps.to_string())
        .map(|r| (r[1].parse().unwrap(), r[2].parse().unwrap()))
        .collect();
    let (mut n, mut same, mut worst) = (0, 0, 0.0_f64);
    for (nuc, want) in &rows {
        let got = out.atom().get(&Nuc(*nuc)).copied().unwrap_or(0.0);
        n += 1;
        same += usize::from(got.to_bits() == want.to_bits());
        worst = worst.max(rel(got, *want));
        std::eprintln!("  {tag} steps={steps} nuc={nuc} port={got:.17e} cyclus={want:.17e} rel={:e}", rel(got, *want));
    }
    for (nuc, got) in out.atom() {
        if !rows.iter().any(|(n, _)| Nuc(*n) == *nuc) {
            assert!(
                *got < tol_absent,
                "{tag} {steps}: port holds {nuc:?} = {got:e}, CYCLUS omits it"
            );
        }
    }
    (n, same, worst)
}

/// C3c. Decay by CYCLUS's **live** path, `Composition::Decay`, which in the
/// pinned source calls pyne's CRAM14 on the full pyne decay matrix
/// (`composition.cc:148-175`), against this port's default solver
/// (`decay_composition`, CRAM48). The five-nuclide composition is decayed for
/// 1, 400 and 4000 steps of 30 days. Decay constants and branching ratios
/// are read out of pyne by the harness and loaded into a `DecayChain`, so the
/// data are identical and only the method differs.
///
/// Prediction (committed first): relative difference < 1e-10 for every
/// nuclide. **Measured: met for 19 of 20 values; missed on Y-90 at 4000
/// steps (9.3e-8 atoms, 2.8e-8 relative).** The miss is explained and
/// checked here against the analytic Bateman solution for the two-member
/// Sr-90 -> Y-90 chain (Zr-90 is stable, and `exp(-lambda_Y t)` underflows):
/// the port is within 1e-12 of it, CYCLUS's CRAM14 is not. CRAM14's error is
/// absolute (~1e-15 of the unit input), so it shows only on a trace nuclide.
/// The bound is not relaxed: the one exceedance is asserted to be exactly
/// that case.
#[test]
fn c3c_decay_cram_matches_cyclus_composition_decay() {
    let (chain, comp0) = decay_setup();
    let masses = AtomicMasses::MassNumber;
    let (mut n, mut same, mut worst) = (0, 0, 0.0_f64);
    let mut exceed = Vec::new();
    for steps in [1_u32, 400, 4000] {
        let secs = f64::from(steps) * 2_592_000.0;
        let out = decay_composition(&comp0, &chain, secs, &masses).unwrap();
        let (a, b, c) = compare_decay(&out, "decay", steps, 1e-15);
        n += a;
        same += b;
        worst = worst.max(c);
        for l in lines("decay").iter().filter(|r| r[0] == steps.to_string()) {
            let nuc: i32 = l[1].parse().unwrap();
            let want: f64 = l[2].parse().unwrap();
            let got = out.atom().get(&Nuc(nuc)).copied().unwrap_or(0.0);
            if rel(got, want) >= 1e-10 {
                exceed.push((steps, nuc, got, want));
            }
        }
    }
    std::eprintln!(
        "decay CRAM48 vs CYCLUS CRAM14: compared={n} bit-identical={same} worst_rel={worst:e} exceed={exceed:?}"
    );
    assert_eq!(exceed.len(), 1, "exactly one value exceeds 1e-10: {exceed:?}");
    let (steps, nuc, got, want) = exceed[0];
    assert_eq!((steps, nuc), (4000, 390900000), "the exceedance is Y-90 at 4000 steps");
    // Analytic Sr-90 -> Y-90 with Sr-90 = 1.0, Y-90 = 0.2 at t = 4000 * 30 d.
    let ls = chain.decay_constant(Nuc(380900000));
    let ly = chain.decay_constant(Nuc(390900000));
    let t = 4000.0 * 2_592_000.0;
    let exact = ls / (ly - ls) * (-ls * t).exp() + 0.2 * (-ly * t).exp();
    assert!(rel(got, exact) < 1e-12, "port vs analytic {:e}", rel(got, exact));
    assert!(rel(want, exact) > 1e-8, "CYCLUS vs analytic {:e}", rel(want, exact));
}

/// C3d. The legacy uniformized-Taylor solver: cyclus's `Decayer`
/// (`decayer.cc:159`, `uniform_taylor.cc`, tolerance hard-coded to 1e-3)
/// against `decay_composition_with_tol(.., DEFAULT_TOL)` on a composition of
/// slow nuclides only (Cs-137 0.7, Ba-137 0.05, Co-60 0.4, Ni-60 0.1,
/// Kr-85 0.3), for 1, 40 and 4000 steps of 30 days. Same data, same
/// algorithm. See the README for why the fast Sr-90 / Y-90 pair is excluded
/// (next test). Prediction: < 1e-10; the bit-identical count is printed.
#[test]
fn c3d_decay_taylor_matches_cyclus_decayer() {
    let (chain, _) = decay_setup();
    let masses = AtomicMasses::MassNumber;
    let c1 = comp(&[
        (551370000, 0.7),
        (561370000, 0.05),
        (270600000, 0.4),
        (280600000, 0.1),
        (360850000, 0.3),
    ]);
    let comp1 = Composition::from_atom(c1, &masses).unwrap();
    let (mut n, mut same, mut worst) = (0, 0, 0.0_f64);
    for steps in [1_u32, 40, 4000] {
        let secs = f64::from(steps) * 2_592_000.0;
        let out = decay_composition_with_tol(&comp1, &chain, secs, &masses, DEFAULT_TOL).unwrap();
        let (a, b, c) = compare_decay(&out, "taylor", steps, 1e-15);
        n += a;
        same += b;
        worst = worst.max(c);
    }
    std::eprintln!(
        "decay Taylor vs CYCLUS Decayer: compared={n} bit-identical={same} worst_rel={worst:e}"
    );
    assert!(worst < 1e-10, "worst relative difference {worst:e}");
}

/// C3e. A defect in CYCLUS's legacy `Decayer`, found by this differential and
/// proven from its source: `BuildDecayMatrix` (`decayer.cc`) sets a nuclide's
/// decay constant to zero when `exp(-2903040000 * lambda) == 0` (comment:
/// "mostly stable nuclides ... 100 years"). The `exp` is the **double**
/// overload, which underflows to 0 for lambda > 745 / 2.9e9 = 2.57e-7 /s, so
/// the heuristic freezes every nuclide with half-life under ~31 days, the
/// opposite of its comment. Y-90 (lambda 3.0e-6 /s) is frozen: after one
/// 30-day step CYCLUS's `Decayer` reports Y-90 = 0.2019 (initial 0.2 plus
/// ingrowth, never decaying) where the physical value is 3.35e-4. The port
/// documents the same deviation (`decay.rs`, `build_decay_matrix`) and does
/// not port the heuristic. Asserted here as measured on the CYCLUS side.
#[test]
fn c3e_cyclus_decayer_freezes_fast_nuclides() {
    let (chain, comp0) = decay_setup();
    let masses = AtomicMasses::MassNumber;
    let out = decay_composition_with_tol(&comp0, &chain, 2_592_000.0, &masses, DEFAULT_TOL).unwrap();
    let cy_y90: f64 = lines("taylor_sr")
        .iter()
        .find(|r| r[1] == "390900000")
        .map(|r| r[2].parse().unwrap())
        .unwrap();
    let port_y90 = out.atom()[&Nuc(390900000)];
    std::eprintln!("Y-90 after 30 d: port {port_y90:e}, CYCLUS Decayer {cy_y90:e}");
    assert!(cy_y90 > 0.2, "CYCLUS Decayer's Y-90 grew past its initial 0.2");
    assert!(port_y90 < 1e-3, "port Y-90 decays to equilibrium");
}
