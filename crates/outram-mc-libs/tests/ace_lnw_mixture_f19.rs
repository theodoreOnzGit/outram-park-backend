// SPDX-License-Identifier: GPL-3.0

//! **An ACE `LNW` chain of correlated laws is a mixture selected by its
//! applicability — GitHub #365.**
//!
//! # What was wrong
//!
//! `Nuclide::from_ace` refused any DLW `LNW` chain that was not made only of
//! MF=5-style laws (4/7/9/11). F-19 (ENDF/B-VIII.0) writes MT=16 (n,2n) as a
//! chain of **two law-61** (tabulated-cosine, correlated) distributions, both
//! applicable 10.987-20 MeV with `p = 0.5` each, so the whole nuclide was
//! refused on both the NJOY2016 and the Rust-NJOY ACE tables, and routes 3/5 of
//! the five-route ICSBEP study could not run HEU-SOL-THERM-009 at all.
//!
//! # The representation
//!
//! Each link becomes one `ContinuumBranch` (the type the ENDF route already
//! uses for F-19's two MF=6 subsections), carrying the link's applicability
//! `p_k(E)` in the new `applicability` field; `ContinuumEmission::branch_for`
//! selects on `p_k(E)` when present and on the ENDF yield otherwise. That is
//! OpenMC's `ReactionProduct { distribution_, applicability_ }`
//! (`src/reaction_product.cpp:110-129`). One type and one sampler serve both
//! routes. Yield and applicability are kept as **separate fields**: ACER
//! derives `p_k = y_k / sum y` (`acefc.f90` `acelf6`), so they select the same
//! law, but only one of them is a multiplicity.
//!
//! # Methodology
//!
//! 1. **Selection statistics (synthetic, no data needed).** A two-branch
//!    emission with a known `p_1(E)`; `branch_for` is called `N = 200 000`
//!    times with independent uniforms and the fraction choosing branch 1 is
//!    compared with `p_1(E)`. Tolerance **5 sigma binomial**,
//!    `sigma = sqrt(p(1-p)/N)` (`~1.07e-3` at `p = 0.35`). Three cases pin the
//!    three parts of OpenMC's `Tabulated1D` evaluation: lin-lin interpolation
//!    inside the range, a histogram (`INT = 1`) region honoured rather than
//!    linearised, and **clamping** to the end value outside the range (the
//!    workspace's `eval_tab1` returns 0 there, which would hand every draw to
//!    the first law).
//! 2. **F-19 loads** from both ACE libraries and MT=16 carries two
//!    applicability-selected branches.
//! 3. **Code-to-code against OpenMC.** Reference: the exact CDF of `E'` that
//!    OpenMC's C++ sampler produces from the same `.ace` file, computed from
//!    OpenMC's *own Python ACE reader* by
//!    `verification_and_validation/ace_route_physics/openmc_inputs/f19_mt16_reference_cdf.py`
//!    (OpenMC 0.16.1.dev25, `d7d3284a1`) into
//!    `verification_and_validation/ace_route_physics/data/f19_mt16_openmc_cdf_<lib>.csv`.
//!    outram-mc samples `N = 200 000` outgoing energies at 14, 16.25 and
//!    18 MeV through the transport kernel's own entry point
//!    (`continuum_inelastic_scatter_evaluated_with`), and the one-sample
//!    **Kolmogorov-Smirnov** distance to the reference CDF must be below the
//!    `alpha = 0.001` critical value `1.95 / sqrt(N) = 4.36e-3`. 14 and 18 MeV
//!    sit on the incident grid (`r = 0`); 16.25 MeV sits mid-bin (`r = 0.5`),
//!    so it exercises the statistical interpolation and envelope scaling too.
//! 4. **The check can fail.** The same KS statistic, computed for draws from
//!    **branch 0 only** (a mixture that ignores the second law), must *exceed*
//!    the critical value by a wide margin at every energy — otherwise the
//!    comparison would not detect the defect the change fixes.
//! 5. **ACE route vs ENDF route.** The ENDF route's MT=16
//!    (`ContinuumEmission::from_endf_mf6` on
//!    `reference-data/endf/n-009_F_019-ENDF8.0.endf`, the exact call
//!    `Nuclide::from_tape` makes) is sampled the same way; two-sample KS at
//!    `alpha = 0.001`, critical `1.95 sqrt(2/N) = 6.17e-3`.
//!
//! The ACE files are built by the five-route study
//! (`verification_and_validation/icsbep/five_route_keff/`) under
//! `target/five_route_keff/{njoy,rust}/293.6K/F19.ace`; they are regenerable
//! scratch, so the data-dependent tests print and skip when absent.
//!
//! # Results (2026-09-29, both libraries)
//!
//! | check | measured | bound |
//! |---|---|---|
//! | synthetic, lin-lin `p_1(1.5 MeV) = 0.35` | 0.34964 (z = -0.34) | `5 sigma` |
//! | synthetic, histogram `p_1(1.5 MeV) = 0.10` | 0.10100 (z = +1.48) | `5 sigma` |
//! | synthetic, clamped `p_1(0.5 MeV) = 0.20`, `p_1(4 MeV) = 0.80` | 0.19946 / 0.80035 | `5 sigma` |
//! | F-19 MT=16 KS vs OpenMC, 14 / 16.25 / 18 MeV | 1.82e-3 / 2.14e-3 / 1.32e-3 | `< 4.36e-3` |
//! | `<E'>` outram-mc vs OpenMC reference | 1.14162 / 1.94127 / 2.59565 vs 1.141511 / 1.943615 / 2.595567 MeV | (KS is the gate) |
//! | branch-0-only KS (must fail) | 6.92e-2 / 1.33e-1 / 1.88e-1 | `> 5 x 4.36e-3` |
//! | ACE vs ENDF route two-sample KS | 2.22e-3 / 2.76e-3 / 2.96e-3 | `< 6.17e-3` |
//!
//! Identical on the NJOY2016 and Rust-NJOY tables (their MT=16 data agree line
//! for line). Mutations run and reverted: ignoring the applicability fails all
//! three synthetic selection tests; zeroing it below its range fails the
//! clamping test. Full record:
//! `verification_and_validation/ace_route_physics/f19_lnw_mixture_2026-09-29.md`.

use njoy_outram_park_fork::endf::records::{Cont, Tab1};
use njoy_outram_park_fork::nuclear_data::secondary::{
    ChiEout, ChiTabular, ContinuumAngular, ContinuumBranch, ContinuumEmission,
};
use outram_mc_libs::geometry::position::Direction;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::scatter::{
    continuum_inelastic_scatter_evaluated_with, ContinuumAngularMode,
};
use outram_mc_libs::rng::lcg::prn;
use std::path::PathBuf;

const N: usize = 200_000;
/// `alpha = 0.001` one-sample KS critical value, asymptotic form.
fn ks_crit_one(n: usize) -> f64 {
    1.95 / (n as f64).sqrt()
}
/// `alpha = 0.001` two-sample KS critical value for equal sizes.
fn ks_crit_two(n: usize) -> f64 {
    1.95 * (2.0 / n as f64).sqrt()
}

fn tab1(pairs: &[(f64, f64)], interp: &[(u32, u32)]) -> Tab1 {
    Tab1 {
        head: Cont {
            c1: 0.0,
            c2: 0.0,
            l1: 0,
            l2: 0,
            n1: interp.len() as i32,
            n2: pairs.len() as i32,
        },
        interp: interp.to_vec(),
        pairs: pairs.to_vec(),
    }
}

/// A flat one-table spectrum; only its identity matters for selection tests.
fn flat_branch(p: Tab1) -> ContinuumBranch {
    ContinuumBranch {
        spectrum: ChiTabular {
            incident: vec![1.0e5, 1.0e7],
            tables: vec![
                ChiEout {
                    e_out: vec![0.0, 1.0],
                    pdf: vec![1.0, 1.0],
                    cdf: vec![0.0, 1.0],
                    linlin: false,
                    n_discrete: 0,
                };
                2
            ],
            incident_interp: Vec::new(),
        },
        yield_pairs: Vec::new(),
        applicability: Some(p),
        angular: ContinuumAngular::EvaluatedIsotropic,
    }
}

/// Fraction of `N` draws that pick branch 0, and its binomial sigma at `p0`.
fn fraction_first(law: &ContinuumEmission, e: f64, p0: f64, seed: u64) -> (f64, f64) {
    let mut s = seed;
    let first = &law.branches[0];
    let hits = (0..N)
        .filter(|_| std::ptr::eq(law.branch_for(e, prn(&mut s)), first))
        .count();
    (hits as f64 / N as f64, (p0 * (1.0 - p0) / N as f64).sqrt())
}

#[test]
fn lnw_selection_follows_linlin_applicability() {
    // p1 rises 0.2 -> 0.8 over 1-3 MeV; p2 = 1 - p1. At 1.5 MeV p1 = 0.35.
    let p1 = tab1(&[(1.0e6, 0.2), (3.0e6, 0.8)], &[(2, 2)]);
    let p2 = tab1(&[(1.0e6, 0.8), (3.0e6, 0.2)], &[(2, 2)]);
    let law = ContinuumEmission {
        branches: vec![flat_branch(p1), flat_branch(p2)],
        cm_frame: false,
    };
    assert!(law.is_applicability_mixture());
    let (f, sigma) = fraction_first(&law, 1.5e6, 0.35, 0x5EED_0001);
    println!(
        "lin-lin: f = {f:.5}, expect 0.35000, sigma = {sigma:.2e}, z = {:.2}",
        (f - 0.35) / sigma
    );
    assert!(
        (f - 0.35).abs() < 5.0 * sigma,
        "f = {f}, expected 0.35 +/- 5 sigma ({sigma})"
    );
    // The applicability mixture is ONE particle: its expected multiplicity is
    // sum p_k * 1 = 1, not the number of branches.
    assert!((law.total_yield_at(1.5e6) - 1.0).abs() < 1e-12);
}

#[test]
fn lnw_selection_honours_a_histogram_region() {
    // INT = 1 over the whole table: p1 = 0.1 on [1, 2) MeV, not the lin-lin 0.5.
    let p1 = tab1(&[(1.0e6, 0.1), (2.0e6, 0.9), (3.0e6, 0.9)], &[(3, 1)]);
    let p2 = tab1(&[(1.0e6, 0.9), (2.0e6, 0.1), (3.0e6, 0.1)], &[(3, 1)]);
    let law = ContinuumEmission {
        branches: vec![flat_branch(p1), flat_branch(p2)],
        cm_frame: false,
    };
    let (f, sigma) = fraction_first(&law, 1.5e6, 0.10, 0x5EED_0002);
    println!(
        "histogram: f = {f:.5}, expect 0.10000, sigma = {sigma:.2e}, z = {:.2}",
        (f - 0.10) / sigma
    );
    assert!(
        (f - 0.10).abs() < 5.0 * sigma,
        "f = {f}: the INT=1 region was not honoured"
    );
}

#[test]
fn lnw_selection_clamps_outside_the_applicability_range() {
    // OpenMC's Tabulated1D returns the end value outside the range
    // (src/endf.cpp:249-252); a zeroing evaluator would pick branch 0 always.
    let p1 = tab1(&[(1.0e6, 0.2), (3.0e6, 0.8)], &[(2, 2)]);
    let p2 = tab1(&[(1.0e6, 0.8), (3.0e6, 0.2)], &[(2, 2)]);
    let law = ContinuumEmission {
        branches: vec![flat_branch(p1), flat_branch(p2)],
        cm_frame: false,
    };
    for (e, p0, seed) in [(0.5e6, 0.2, 0x5EED_0003_u64), (4.0e6, 0.8, 0x5EED_0004)] {
        let (f, sigma) = fraction_first(&law, e, p0, seed);
        println!(
            "clamped E = {e:.1e}: f = {f:.5}, expect {p0}, z = {:.2}",
            (f - p0) / sigma
        );
        assert!(
            (f - p0).abs() < 5.0 * sigma,
            "E = {e}: f = {f}, expected {p0}"
        );
    }
}

#[test]
fn endf_yield_selection_is_unchanged() {
    // An ENDF-built emission (no applicability) still selects by yield: yields
    // 1 and 3 give branch 0 a quarter of the draws.
    let mut b0 = flat_branch(tab1(&[(1.0, 1.0)], &[]));
    let mut b1 = b0.clone();
    b0.applicability = None;
    b1.applicability = None;
    b0.yield_pairs = vec![(1.0e5, 1.0), (1.0e7, 1.0)];
    b1.yield_pairs = vec![(1.0e5, 3.0), (1.0e7, 3.0)];
    let law = ContinuumEmission {
        branches: vec![b0, b1],
        cm_frame: false,
    };
    assert!(!law.is_applicability_mixture());
    assert!(
        (law.total_yield_at(1.0e6) - 4.0).abs() < 1e-12,
        "ENDF yields still sum"
    );
    let (f, sigma) = fraction_first(&law, 1.0e6, 0.25, 0x5EED_0005);
    assert!((f - 0.25).abs() < 5.0 * sigma, "f = {f}");
}

// ── F-19 on real tables ────────────────────────────────────────────────────

fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn f19_ace(lib: &str) -> Option<PathBuf> {
    let p = workspace().join(format!("target/five_route_keff/{lib}/293.6K/F19.ace"));
    if p.is_file() {
        Some(p)
    } else {
        println!(
            "{} absent: skipping (built by verification_and_validation/icsbep/five_route_keff/)",
            p.display()
        );
        None
    }
}

/// `(e_in, [(x, F(x))])` blocks from the OpenMC reference CSV.
fn reference_cdf(lib: &str) -> Vec<(f64, Vec<(f64, f64)>)> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "verification_and_validation/ace_route_physics/data/f19_mt16_openmc_cdf_{lib}.csv"
    ));
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    let mut out: Vec<(f64, Vec<(f64, f64)>)> = Vec::new();
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("e_in"))
    {
        let v: Vec<f64> = line.split(',').map(|x| x.trim().parse().unwrap()).collect();
        match out.last_mut() {
            Some((e, pts)) if *e == v[0] => pts.push((v[1], v[2])),
            _ => out.push((v[0], vec![(v[1], v[2])])),
        }
    }
    out
}

fn cdf_at(pts: &[(f64, f64)], x: f64) -> f64 {
    if x <= pts[0].0 {
        return 0.0;
    }
    if x >= pts[pts.len() - 1].0 {
        return pts[pts.len() - 1].1;
    }
    let k = pts.partition_point(|p| p.0 <= x);
    let (x0, y0) = pts[k - 1];
    let (x1, y1) = pts[k];
    y0 + (y1 - y0) * (x - x0) / (x1 - x0)
}

/// One-sample KS distance of sorted `xs` to a piecewise-linear CDF.
fn ks_one(xs: &mut [f64], pts: &[(f64, f64)]) -> f64 {
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = xs.len() as f64;
    xs.iter()
        .enumerate()
        .map(|(i, &x)| {
            let f = cdf_at(pts, x);
            (f - i as f64 / n).abs().max(((i + 1) as f64 / n - f).abs())
        })
        .fold(0.0, f64::max)
}

fn ks_two(a: &mut [f64], b: &mut [f64]) -> f64 {
    a.sort_by(|x, y| x.partial_cmp(y).unwrap());
    b.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let (na, nb) = (a.len() as f64, b.len() as f64);
    let (mut i, mut j, mut d) = (0usize, 0usize, 0.0f64);
    while i < a.len() && j < b.len() {
        let x = a[i].min(b[j]);
        while i < a.len() && a[i] <= x {
            i += 1;
        }
        while j < b.len() && b[j] <= x {
            j += 1;
        }
        d = d.max((i as f64 / na - j as f64 / nb).abs());
    }
    d
}

fn sample_eout(law: &ContinuumEmission, awr: f64, q: f64, e: f64, seed: u64) -> Vec<f64> {
    let mut s = seed;
    let u = Direction::new(0.0, 0.0, 1.0);
    (0..N)
        .map(|_| {
            continuum_inelastic_scatter_evaluated_with(
                e,
                u,
                awr,
                q,
                Some(law),
                ContinuumAngularMode::Evaluated,
                &mut s,
            )
            .0
        })
        .collect()
}

#[test]
fn f19_mt16_from_ace_matches_openmc_sampling() {
    for lib in ["njoy", "rust"] {
        let Some(path) = f19_ace(lib) else { continue };
        let nuc = Nuclide::from_ace_file(&path, "F19")
            .unwrap_or_else(|e| panic!("{lib}: F-19 must load from ACE now (#365): {e}"));
        let law = nuc
            .continuum_law(16)
            .expect("F-19 MT=16 is placed as a continuum law");
        assert_eq!(law.branches.len(), 2, "{lib}: two law-61 links");
        assert!(law.is_applicability_mixture());
        assert!(
            !law.cm_frame,
            "{lib}: F-19 MT=16 is laboratory-frame (TY > 0)"
        );
        for b in &law.branches {
            assert!(
                matches!(b.angular, ContinuumAngular::LabTabulated(_)),
                "law 61"
            );
            assert_eq!(b.applicability_at(14.0e6), Some(0.5));
        }
        let awr = nuc.awr;
        let q = -10.431e6;
        let crit = ks_crit_one(N);
        for (k, (e, pts)) in reference_cdf(lib).into_iter().enumerate() {
            let mut xs = sample_eout(law, awr, q, e, 0xF19_0000 + k as u64);
            let mean = xs.iter().sum::<f64>() / N as f64;
            let d = ks_one(&mut xs, &pts);
            // Mutation: the first law alone, which is what ignoring the
            // applicability of the second law would sample.
            let only0 = ContinuumEmission {
                branches: vec![law.branches[0].clone()],
                cm_frame: false,
            };
            let mut ys = sample_eout(&only0, awr, q, e, 0xF19_1000 + k as u64);
            let d0 = ks_one(&mut ys, &pts);
            println!(
                "{lib} E = {:6.3} MeV: <E'> = {:.5} MeV, KS D = {d:.3e} (crit {crit:.3e}); \
                 branch-0-only D = {d0:.3e}",
                e / 1e6,
                mean / 1e6
            );
            assert!(d < crit, "{lib} E = {e}: KS D = {d} >= {crit}");
            assert!(
                d0 > 5.0 * crit,
                "{lib} E = {e}: the check cannot see a missing law (D0 = {d0})"
            );
        }
    }
}

#[test]
fn f19_mt16_ace_route_matches_endf_route() {
    let tape_path = workspace().join("reference-data/endf/n-009_F_019-ENDF8.0.endf");
    let tape = njoy_outram_park_fork::endf::tape::Tape::read_file(&tape_path).expect("F-19 tape");
    let mat = tape.materials()[0];
    let endf = ContinuumEmission::from_endf_mf6(&tape, mat, 16)
        .expect("parse")
        .expect("F-19 MT=16 is MF=6");
    assert_eq!(endf.branches.len(), 2, "two MF=6 neutron subsections");
    assert!(!endf.is_applicability_mixture());
    for lib in ["njoy", "rust"] {
        let Some(path) = f19_ace(lib) else { continue };
        let nuc = Nuclide::from_ace_file(&path, "F19").expect("loads");
        let ace = nuc.continuum_law(16).expect("MT=16");
        let crit = ks_crit_two(N);
        for (k, e) in [14.0e6, 16.25e6, 18.0e6].into_iter().enumerate() {
            let mut a = sample_eout(ace, nuc.awr, -10.431e6, e, 0xACE_0000 + k as u64);
            let mut b = sample_eout(&endf, nuc.awr, -10.431e6, e, 0xE0F_0000 + k as u64);
            let (ma, mb) = (
                a.iter().sum::<f64>() / N as f64,
                b.iter().sum::<f64>() / N as f64,
            );
            let d = ks_two(&mut a, &mut b);
            println!(
                "{lib} E = {:6.3} MeV: <E'> ACE {:.5} / ENDF {:.5} MeV, two-sample KS D = {d:.3e} \
                 (crit {crit:.3e})",
                e / 1e6,
                ma / 1e6,
                mb / 1e6
            );
            assert!(d < crit, "{lib} E = {e}: ACE vs ENDF KS D = {d} >= {crit}");
        }
    }
}
