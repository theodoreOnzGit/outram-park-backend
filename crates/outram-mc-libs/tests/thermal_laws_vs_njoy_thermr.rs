//! **The S(α,β) thermal laws, against NJOY2016's own THERMR — as committed
//! golden data, so the comparison runs everywhere and cannot drift.**
//!
//! Every number in this file was measured on 2026-09-11 by running NJOY2016
//! (release **2016.79**, `18Mar25`, built from `/home/user/njoy/njoy2016`) on the
//! same ENDF tapes this crate reads, and is reproduced in the tables below and
//! asserted against. The NJOY output tapes are 24–28 MB and are **not** checked
//! in; the handful of numbers taken off them are, which is what lets these run
//! without NJOY installed.
//!
//! # Why this file exists
//!
//! These comparisons were run ad-hoc while chasing the FHR ring-RPT /
//! LEU-COMP-THERM-008 residual (#186, #188). Run ad-hoc, they are anecdotes;
//! the numbers then get quoted in documents and drift from what the code does.
//! **That actually happened here** — see `graphite_sab_kernel_against_njoy_thermr`
//! below, which corrects a claim this workspace had been repeating.
//!
//! # The NJOY decks
//!
//! **H in H₂O** (`tsl-HinH2O`, MAT 1, with H-1 MAT 125), 293.6 K. The THERMR
//! card is `matde matdp nbin ntemp iinc icoh iform natom mtref iprint`, taken
//! from NJOY2016's own test 68 — `natom = 2` for the two hydrogens per molecule,
//! `icoh = 0` because water has no coherent-elastic channel:
//!
//! ```text
//! reconr
//!  20 21/
//!  'pendf for h1'/
//!  125 0/
//!  0.001/
//!  0/
//! broadr
//!  20 21 22/
//!  125 1/
//!  0.001/
//!  293.6/
//!  0/
//! thermr
//!  30 22 23/
//!  1 125 20 1 2 0 0 2 222 1/
//!  293.6/
//!  0.001 10.0/
//! stop
//! ```
//!
//! **Graphite** (`tsl-crystalline-graphite`, MAT 30, with C-12 MAT 625), 600 K —
//! a *tabulated* temperature on that tape, so no interpolation is involved:
//! identical deck with `625` for the material, `30 625 16 1 2 1 0 1 229 1/`, and
//! `0.001 4.0/`. `tape23` then carries MT=229 (incoherent inelastic) and MT=230
//! (coherent elastic).
//!
//! Regenerate with `examples/h2o_vs_njoy_thermr.rs`,
//! `examples/h2o_kernel_vs_njoy_thermr.rs` and
//! `examples/graphite_kernel_vs_njoy_thermr.rs`, which read the tapes directly
//! and print these tables in full.

use outram_mc_libs::material::thermal::ThermalScattering;

/// `Some(law)` when the tape is present, else `None` after printing a skip note.
/// Data-gated tests pass rather than fail when the repo-only
/// `reference-data/endf/` is absent — same contract as
/// `tests/thermal_graphite_elastic.rs`.
fn law_or_skip(file: &str, mat: i32, temp_k: f64, name: &str) -> Option<ThermalScattering> {
    let Some(path) = njoy_outram_park_fork::reference_data::reference_endf(file) else {
        println!("[{name}] SKIP: {file} not in reference-data/endf/");
        return None;
    };
    match ThermalScattering::from_endf_file(path.to_str().expect("path"), mat, temp_k, name) {
        Ok(l) => Some(l),
        Err(e) => {
            println!("[{name}] SKIP: {e:?}");
            None
        }
    }
}

/// `(⟨E′⟩/E, ξ)` from `n` samples of the law at incident energy `e`, with the
/// coherent-elastic channel (the only one that leaves `E′` exactly equal to `E`)
/// removed so the moment is the *inelastic* one NJOY's MF=6 matrix describes.
fn sampled_moments(law: &ThermalScattering, e: f64, n: usize, seed: &mut u64) -> (f64, f64) {
    let (mut sum_ratio, mut sum_xi, mut k) = (0.0, 0.0, 0usize);
    for _ in 0..n {
        let Some((ep, _mu)) = law.sample(e, seed) else {
            continue;
        };
        if (ep - e).abs() <= 1.0e-12 * e || ep <= 0.0 {
            continue;
        }
        sum_ratio += ep / e;
        sum_xi += (e / ep).ln();
        k += 1;
    }
    assert!(
        k > n / 100,
        "law produced almost no inelastic scatters at {e} eV"
    );
    (sum_ratio / k as f64, sum_xi / k as f64)
}

// ─────────────────────────────────────────────────────────────────────────────
// H in H₂O — cross section
// ─────────────────────────────────────────────────────────────────────────────

/// NJOY2016 THERMR MT=222 for `tsl-HinH2O` at 293.6 K: `(E [eV], σ [b])`.
const H2O_XS_NJOY: &[(f64, f64)] = &[
    (1.000000e-03, 1.168588e2), // ours 1.184556e2, +1.37 %
    (5.000000e-03, 8.185424e1), // ours 8.253468e1, +0.83 %
    (1.000000e-02, 6.932242e1), // ours 6.992186e1, +0.86 %
    (2.530000e-02, 5.168752e1), // ours 5.214197e1, +0.88 %
    (5.000000e-02, 3.951121e1), // ours 3.990964e1, +1.01 %
    (1.000000e-01, 3.255061e1), // ours 3.289730e1, +1.07 %
    (2.000000e-01, 2.745337e1), // ours 2.776834e1, +1.15 %
    (4.000000e-01, 2.363236e1), // ours 2.391182e1, +1.18 %
    (6.250000e-01, 2.215801e1), // ours 2.230188e1, +0.65 %
    (1.000000e+00, 2.150120e1), // ours 2.181613e1, +1.46 %
    (2.000000e+00, 2.094862e1), // ours 2.125675e1, +1.47 %
];

/// **This crate's H-in-H₂O S(α,β) cross section sits a consistent ~+1 % above
/// NJOY2016's THERMR, and its law ends near 2 eV where NJOY's runs to 10.**
///
/// # Methodology
///
/// `ThermalScattering::total_xs` against NJOY's MT=222, same tape
/// (`tsl-HinH2O`, MAT 1), same temperature (293.6 K, a tabulated one), same
/// evaluation. NJOY deck in the module docs.
///
/// # Results (2026-09-11, NJOY2016 2016.79, ENDF/B-VIII.0)
///
/// ```text
///   E [eV]     NJOY [b]      ours [b]    rel
///    0.001     116.8588      118.4556   +1.37 %
///    0.005      81.85424      82.53468  +0.83 %
///    0.01       69.32242      69.92186  +0.86 %
///    0.0253     51.68752      52.14197  +0.88 %
///    0.05       39.51121      39.90964  +1.01 %
///    0.1        32.55061      32.89730  +1.07 %
///    0.2        27.45337      27.76834  +1.15 %
///    0.4        23.63236      23.91182  +1.18 %
///    0.625      22.15801      22.30188  +0.65 %
///    1.0        21.50120      21.81613  +1.46 %
///    2.0        20.94862      21.25675  +1.47 %
///    4.0        20.68854       0        (our law has ended; free gas takes over)
///    8.0        20.56327       0
/// ```
///
/// # What is asserted, and why the bound is loose
///
/// The bound is **2 %**, which *permits* the observed +1 %. That is deliberate:
/// this is a **characterisation** test for a known defect (#188), pinning the
/// current state so a further regression fails while the fix is pending. When
/// #188 lands, tighten it to ~0.3 % — the graphite cross section's level.
///
/// The sign is also asserted, because "within 2 %" would pass if the error
/// flipped, and a sign flip would mean something entirely different happened.
#[test]
fn h2o_sab_cross_section_against_njoy_thermr() {
    let Some(law) = law_or_skip("tsl-HinH2O.endf", 1, 293.6, "c_H_in_H2O") else {
        return;
    };
    let mut worst: (f64, f64) = (0.0, 0.0);
    for &(e, njoy) in H2O_XS_NJOY {
        let ours = law.total_xs(e);
        assert!(
            ours > 0.0,
            "our H(H2O) law has no cross section at {e} eV, where NJOY gives {njoy} b — \
             the law's upper limit has moved down"
        );
        let rel = ours / njoy - 1.0;
        println!(
            "  {e:>9.4e}  NJOY {njoy:>10.5}  ours {ours:>10.5}  {:>+6.2} %",
            100.0 * rel
        );
        if rel.abs() > worst.0.abs() {
            worst = (rel, e);
        }
    }
    println!("  worst {:+.2} % at {:.4e} eV", 100.0 * worst.0, worst.1);
    assert!(
        worst.0.abs() < 0.02,
        "H(H2O) cross section is {:+.2} % from NJOY at {:.4e} eV — worse than the \
         +1.5 % recorded on 2026-09-11 (GitHub #188)",
        100.0 * worst.0,
        worst.1
    );
    assert!(
        worst.0 > 0.0,
        "the H(H2O) cross-section error has changed SIGN (now {:+.2} %). The recorded \
         defect is a consistent EXCESS of ~+1 %; a deficit is a different bug and this \
         test's premise no longer holds",
        100.0 * worst.0
    );
}

/// **This crate's H-in-H₂O law ends between 2 and 4 eV, where NJOY's THERMR run
/// extends to 10 eV — above it the free-gas kernel takes over.**
///
/// # Why pin this
///
/// It is a real difference in the modelled range, and it is invisible in any
/// cross-section comparison that stops at 2 eV. Above the handover the two
/// treatments happen to agree closely — NJOY's bound value is 20.689 b at 4 eV
/// against this crate's free-gas 20.50 b, 0.9 % apart — so the *consequence* is
/// small, but the discontinuity is real and should not move silently.
///
/// # Results (2026-09-11)
///
/// `total_xs` is positive at 2.0 eV (21.257 b) and exactly zero at 4.0 eV.
#[test]
fn h2o_sab_law_ends_between_2_and_4_ev() {
    let Some(law) = law_or_skip("tsl-HinH2O.endf", 1, 293.6, "c_H_in_H2O") else {
        return;
    };
    assert!(
        law.total_xs(2.0) > 0.0,
        "the H(H2O) law no longer covers 2.0 eV; it did on 2026-09-11 (21.257 b)"
    );
    assert_eq!(
        law.total_xs(4.0),
        0.0,
        "the H(H2O) law now extends past 4.0 eV. That may be an improvement — NJOY's \
         own run reaches 10 eV — but the handover to free gas has moved and the \
         transport's thermal/epithermal seam moved with it"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Kernels — the outgoing-energy distribution transport actually samples
// ─────────────────────────────────────────────────────────────────────────────

/// NJOY2016 THERMR MF=6/MT=222 for `tsl-HinH2O` at 293.6 K, reduced to its first
/// moment: `(E [eV], ⟨E′⟩/E, ξ = ⟨ln(E/E′)⟩)`.
const H2O_KERNEL_NJOY: &[(f64, f64, f64)] = &[
    (1.500000e-03, 7.304950, -0.893430), // ours 6.899970 (−5.54 %)
    (5.000000e-03, 2.510210, -0.426060), // ours 2.414360 (−3.82 %)
    (1.000000e-02, 1.666430, -0.235610), // ours 1.618340 (−2.89 %)
    (2.530000e-02, 1.193860, -0.055270), // ours 1.176320 (−1.47 %)
    (5.000000e-02, 1.010440, 0.086990),  // ours 1.002250 (−0.81 %)
    (1.115700e-01, 0.800170, 0.366900),  // ours 0.800170 (+0.00 %)
    (2.000000e-01, 0.713830, 0.489700),  // ours 0.718120 (+0.60 %)
    (4.170400e-01, 0.642280, 0.620940),  // ours 0.646770 (+0.70 %)
    (6.250000e-01, 0.605110, 0.710540),  // ours 0.612900 (+1.29 %)
    (1.050000e+00, 0.565930, 0.801740),  // ours 0.572550 (+1.17 %)
    (1.855000e+00, 0.539260, 0.869670),  // ours 0.547230 (+1.48 %)
];

/// NJOY2016 THERMR MF=6/MT=229 for `tsl-crystalline-graphite` at 600 K:
/// `(E [eV], ⟨E′⟩/E)`, coherent elastic excluded from the moment.
const GRAPHITE_KERNEL_NJOY: &[(f64, f64)] = &[
    (1.000000e-02, 4.907990), // ours 4.897740 (−0.21 %), 83 % coherent elastic
    (2.530000e-02, 1.903930), // ours 1.874890 (−1.53 %), 78 %
    (5.000000e-02, 1.264140), // ours 1.246550 (−1.39 %), 66 %
    (1.115700e-01, 1.011580), // ours 1.004850 (−0.67 %), 43 %
    (2.000000e-01, 0.936480), // ours 0.936760 (+0.03 %), 27 %
    (4.170400e-01, 0.890410), // ours 0.890280 (−0.01 %), 13 %
    (6.250000e-01, 0.878570), // ours 0.879470 (+0.10 %), 9 %
    (1.050000e+00, 0.869530), // ours 0.870310 (+0.09 %), 5 %
    (1.855000e+00, 0.863980), // ours 0.864200 (+0.03 %), 3 %
    (3.750000e+00, 0.860400), // ours 0.860980 (+0.07 %), 2 %
];

/// **This crate's H-in-H₂O *kernel* — the outgoing-energy distribution transport
/// samples — is too narrow: it moves neutrons 2–5.5 % less than NJOY2016's.**
///
/// # Methodology
///
/// 200 000 samples of `ThermalScattering::sample` per incident energy, reduced
/// to `⟨E′⟩/E`, against a quadrature on NJOY's MF=6/MT=222 matrix (no sampling
/// on that side). Same tape, same temperature. Binomial noise at 200 000 samples
/// is well under 0.2 %, so every deviation below is far outside it.
///
/// # Results (2026-09-11, NJOY2016 2016.79)
///
/// ```text
///   E [eV]    ⟨E′⟩/E NJOY      ours       rel       ξ NJOY    ξ ours
///   0.0015        7.30495    6.89997   −5.54 %    −0.89343  −0.88430
///   0.005         2.51021    2.41436   −3.82 %    −0.42606  −0.42661
///   0.01          1.66643    1.61834   −2.89 %    −0.23561  −0.23775
///   0.0253        1.19386    1.17632   −1.47 %    −0.05527  −0.05558
///   0.05          1.01044    1.00225   −0.81 %    +0.08699  +0.07530
///   0.1116        0.80017    0.80017   +0.00 %    +0.36690  +0.34828
///   0.2           0.71383    0.71812   +0.60 %    +0.48970  +0.46416
///   0.625         0.60511    0.61290   +1.29 %    +0.71054  +0.67885
///   1.855         0.53926    0.54723   +1.48 %    +0.86967  +0.84120
/// ```
///
/// **Both halves err the same way.** Below the crossover at ~0.11 eV the neutron
/// should *gain* energy and ours gains too little; above it the neutron should
/// *lose* energy and ours loses too little. The distribution is clustered too
/// close to the incident energy. `ξ` runs 3–5 % low above 0.05 eV: this crate's
/// water moderates about 4 % less per collision than NJOY's.
///
/// # What is asserted
///
/// A **6 %** envelope, which permits the measured −5.54 %: a characterisation
/// test for the open defect #188, pinning the state so it cannot get worse
/// unnoticed. **It also asserts the shape** — negative below the crossover,
/// positive above it — because that sign structure is the evidence for the
/// suspected mechanism (a too-narrow short-collision-time tail truncating the
/// distribution symmetrically), and a change in it would mean a different bug.
///
/// When #188 is fixed, replace the envelope with ~1 % and delete the shape
/// assertions.
#[test]
fn h2o_sab_kernel_against_njoy_thermr() {
    let Some(law) = law_or_skip("tsl-HinH2O.endf", 1, 293.6, "c_H_in_H2O") else {
        return;
    };
    let mut seed = 20_260_911_u64;
    let (mut worst, mut worst_e) = (0.0_f64, 0.0_f64);
    for &(e, njoy_m1, njoy_xi) in H2O_KERNEL_NJOY {
        let (m1, xi) = sampled_moments(&law, e, 200_000, &mut seed);
        let rel = m1 / njoy_m1 - 1.0;
        println!(
            "  {e:>9.4e}  <E'>/E NJOY {njoy_m1:>9.5} ours {m1:>9.5} ({:>+6.2} %)   \
             xi NJOY {njoy_xi:>+9.5} ours {xi:>+9.5}",
            100.0 * rel
        );
        if rel.abs() > worst {
            worst = rel.abs();
            worst_e = e;
        }
        // The sign structure IS the finding — see the doc comment.
        if e < 0.10 {
            assert!(
                rel < 0.005,
                "below the ~0.11 eV crossover the kernel should under-GAIN energy \
                 (negative rel); at {e} eV it is {:+.2} %",
                100.0 * rel
            );
        } else if e > 0.15 {
            assert!(
                rel > -0.005,
                "above the ~0.11 eV crossover the kernel should under-LOSE energy \
                 (positive rel); at {e} eV it is {:+.2} %",
                100.0 * rel
            );
        }
    }
    println!(
        "  worst |Δ⟨E′⟩/E| = {:.2} % at {:.4e} eV",
        100.0 * worst,
        worst_e
    );
    assert!(
        worst < 0.06,
        "the H(H2O) kernel is {:.2} % from NJOY at {:.4e} eV — worse than the 5.54 % \
         recorded on 2026-09-11 (GitHub #188)",
        100.0 * worst,
        worst_e
    );
}

/// **The graphite kernel, on the identical check — and a correction to a claim
/// this workspace had been repeating.**
///
/// # Why this is here
///
/// It is the control for `h2o_sab_kernel_against_njoy_thermr`: same code, same
/// sampler, same comparison method, a different thermal law. Without it, the
/// water result could be an artefact of the method.
///
/// # The correction
///
/// `verification_and_validation/ring_rpt/ring_rpt_vs_openmc.md` carried
/// *"graphite S(α,β) **outgoing energy** | NJOY THERMR MF=6 scattering matrix |
/// **≤0.5 %**"*, and that figure was used to argue water was uniquely bad. The
/// ≤0.5 % is real but was **scoped to 0.1–4 eV**. Measured over the same range
/// as water, graphite is **−1.53 % at 0.0253 eV** — essentially the same as
/// water's −1.47 % there. The honest statement is narrower than the one that was
/// being made:
///
/// | E [eV] | graphite | water |
/// |---|---|---|
/// | 0.0015 | (below range) | **−5.54 %** |
/// | 0.01 | −0.21 % | −2.89 % |
/// | 0.0253 | **−1.53 %** | **−1.47 %** |
/// | 0.05 | −1.39 % | −0.81 % |
/// | 0.2 | +0.03 % | +0.60 % |
/// | 0.625 | +0.10 % | +1.29 % |
/// | 1.855 | +0.03 % | +1.48 % |
///
/// So: **at the thermal peak both laws are ~1.5 % low — a general deviation of
/// this crate's S(α,β) kernel, not a water-specific one.** What *is*
/// water-specific is the two ends: graphite converges to ≤0.1 % above 0.2 eV
/// while water stays at +0.6 to +1.5 %, and water degrades to −5.5 % at
/// 1.5 meV where graphite's worst anywhere in 0.01–4 eV is −1.69 %.
///
/// # What is asserted
///
/// A 2 % envelope (permitting the −1.53 %), **and** convergence to ≤0.4 % above
/// 0.2 eV — which is the property water lacks, and therefore the one that makes
/// this a control rather than a second anecdote.
#[test]
fn graphite_sab_kernel_against_njoy_thermr() {
    let Some(law) = law_or_skip("tsl-crystalline-graphite.endf", 30, 600.0, "c_Graphite") else {
        return;
    };
    let mut seed = 777_000_111_u64;
    let (mut worst, mut worst_e) = (0.0_f64, 0.0_f64);
    for &(e, njoy_m1) in GRAPHITE_KERNEL_NJOY {
        let (m1, _xi) = sampled_moments(&law, e, 200_000, &mut seed);
        let rel = m1 / njoy_m1 - 1.0;
        println!(
            "  {e:>9.4e}  <E'>/E NJOY {njoy_m1:>9.5} ours {m1:>9.5} ({:>+6.2} %)",
            100.0 * rel
        );
        if rel.abs() > worst {
            worst = rel.abs();
            worst_e = e;
        }
        if e > 0.2 {
            assert!(
                rel.abs() < 0.004,
                "graphite's kernel no longer converges on NJOY above 0.2 eV: {:+.2} % at \
                 {e} eV. That convergence is what distinguishes it from water (GitHub \
                 #188) and makes this test a control",
                100.0 * rel
            );
        }
    }
    println!(
        "  worst |Δ⟨E′⟩/E| = {:.2} % at {:.4e} eV",
        100.0 * worst,
        worst_e
    );
    assert!(
        worst < 0.02,
        "the graphite kernel is {:.2} % from NJOY at {:.4e} eV — worse than the 1.53 % \
         recorded on 2026-09-11",
        100.0 * worst,
        worst_e
    );
}
