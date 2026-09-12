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
// The oracle tables themselves live in the library, at
// `outram_mc_libs::vv::njoy_golden`, so that the `examples/` programs which
// regenerate them assert against the *same* recorded numbers these tests do.
// Two copies of an oracle drift, and the copy nothing checks drifts first.
use outram_mc_libs::vv::njoy_golden::{
    GRAPHITE_KERNEL as GRAPHITE_KERNEL_NJOY, GRAPHITE_KERNEL_CONVERGED_ABOVE_EV,
    GRAPHITE_KERNEL_CONVERGED_TOL, GRAPHITE_KERNEL_TOL, GRAPHITE_XS_COHERENT,
    GRAPHITE_XS_INELASTIC, GRAPHITE_XS_TOL, H2O_KERNEL as H2O_KERNEL_NJOY, H2O_KERNEL_CROSSOVER_EV,
    H2O_KERNEL_TOL, H2O_XS as H2O_XS_NJOY, H2O_XS_TOL,
};

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
        worst.0.abs() < H2O_XS_TOL,
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

/// **This crate's H-in-H₂O *kernel* — the outgoing-energy distribution transport
/// samples — is too narrow: it moves neutrons 2–5.5 % less than NJOY2016's.**
///
/// # Methodology
///
/// 200 000 samples of `ThermalScattering::sample` per incident energy, reduced
/// to `⟨E′⟩/E`, against a quadrature on NJOY's MF=6/MT=222 matrix (no sampling
/// on that side). Same tape, same temperature.
///
/// **Sampling uncertainty, measured rather than assumed.** An earlier version of
/// this comment said "binomial noise is well under 0.2 %". That is wrong at the
/// bottom of the range: `⟨E′⟩/E` is a mean of a *wide* distribution where
/// up-scatter dominates, not a binomial proportion. Measured standard error at
/// 400 000 samples (`examples/h2o_kernel_vs_njoy_thermr.rs`, which prints it per
/// row): **0.28 % at 1.5 meV**, 0.21 % at 5 meV, and 0.06–0.09 % from 0.05 eV up.
/// Scaled to the 200 000 samples used here, ~0.40 % at the worst point.
///
/// The −5.54 % finding is therefore ~14 σ at 1.5 meV and larger still in the
/// middle of the range — real by a wide margin, but *not* by the factor the old
/// comment implied.
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
        if e < 0.9 * H2O_KERNEL_CROSSOVER_EV {
            assert!(
                rel < 0.005,
                "below the ~0.11 eV crossover the kernel should under-GAIN energy \
                 (negative rel); at {e} eV it is {:+.2} %",
                100.0 * rel
            );
        } else if e > 1.3 * H2O_KERNEL_CROSSOVER_EV {
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
        worst < H2O_KERNEL_TOL,
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
        if e > GRAPHITE_KERNEL_CONVERGED_ABOVE_EV {
            assert!(
                rel.abs() < GRAPHITE_KERNEL_CONVERGED_TOL,
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
        worst < GRAPHITE_KERNEL_TOL,
        "the graphite kernel is {:.2} % from NJOY at {:.4e} eV — worse than the 1.53 % \
         recorded on 2026-09-11",
        100.0 * worst,
        worst_e
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Graphite — cross section. The control for the water cross section, and the
// half of the graphite comparison that had no test at all until 2026-09-11.
// ─────────────────────────────────────────────────────────────────────────────

/// **This crate's graphite S(α,β) cross sections reproduce NJOY2016's THERMR to
/// 0.14 % on the inelastic channel and 0.09 % on the coherent-elastic one.**
///
/// # Why this exists
///
/// `graphite_sab_kernel_against_njoy_thermr` covers the *kernel*. The *cross
/// section* — the magnitude, which sets how often a thermal neutron interacts
/// with graphite at all — was compared ad hoc while chasing #186/#188 and never
/// committed, so the figure quoted in this workspace's V&V documents ("±0.05 %")
/// had nothing checking it. It is now measured, recorded, and asserted, and the
/// honest number is 0.14 %, not 0.05 %.
///
/// It is also the control that makes the water result mean something: same
/// code path, same THERMR comparison, a different law. Water is +1.47 %
/// (`h2o_sab_cross_section_against_njoy_thermr`); graphite is −0.14 %. Ten times
/// better, on the same machinery — so the water offset is water's, not the
/// method's.
///
/// # Methodology
///
/// `ThermalScattering::inelastic_xs` against THERMR MT=229 and
/// `elastic_xs` against MT=230, on `tsl-crystalline-graphite` (MAT 30,
/// with C-12 MAT 625) at **600 K — a tabulated temperature on that tape**, so no
/// temperature interpolation is involved and any difference is in the law
/// itself. The NJOY deck is in the module docs; the oracle values and their
/// provenance are in [`outram_mc_libs::vv::njoy_golden`].
///
/// # Results (2026-09-11, NJOY2016 2016.79, ENDF/B-VIII.0)
///
/// ```text
///   E [eV]     MT=229 NJOY     ours        rel       MT=230 NJOY   ours        rel
///   1e-4        3.341434       3.342667   +0.04 %     0            0          exact
///   1e-3        1.137236       1.135755   −0.13 %     0            0          exact
///   5e-3        0.7150516      0.7149814  −0.01 %     4.616808     4.616808   −0.00 %
///   0.0253      1.110868       1.110623   −0.02 %     4.041842     4.041842   −0.00 %
///   0.1         2.588940       2.588316   −0.02 %     2.243431     2.243431   −0.00 %
///   1.0         4.485358       4.483433   −0.04 %     0.2594442    0.2594442  −0.00 %
///   3.0         4.654246       4.649273   −0.11 %     0.08655500   0.08648140 −0.09 %
///   3.9         4.673901       4.667192   −0.14 %     0.06653488   0.06652415 −0.02 %
/// ```
///
/// Worst: **−0.14 %** (inelastic, 3.9 eV) and **−0.09 %** (coherent, 3.0 eV).
///
/// # What is asserted
///
/// Both channels inside 0.5 % — about 3.5x the worst observed, which leaves room
/// for grid noise without admitting a real regression.
///
/// Separately, and more sharply: the coherent-elastic cross section must be
/// **exactly zero** below the first Bragg edge. That is a structural property,
/// not a numerical one — a code that puts the cutoff in the wrong place, or that
/// smears it, cannot produce an exact zero there, and no percentage envelope
/// would notice.
#[test]
fn graphite_sab_cross_sections_against_njoy_thermr() {
    let Some(law) = law_or_skip("tsl-crystalline-graphite.endf", 30, 600.0, "c_Graphite") else {
        return;
    };

    let mut worst_inel: (f64, f64) = (0.0, 0.0);
    for &(e, njoy) in GRAPHITE_XS_INELASTIC {
        let ours = law.inelastic_xs(e);
        let rel = ours / njoy - 1.0;
        println!(
            "  MT=229  {e:>9.4e}  NJOY {njoy:>11.6}  ours {ours:>11.6}  {:>+6.2} %",
            100.0 * rel
        );
        if rel.abs() > worst_inel.0.abs() {
            worst_inel = (rel, e);
        }
    }
    assert!(
        worst_inel.0.abs() < GRAPHITE_XS_TOL,
        "graphite's incoherent-inelastic cross section is {:+.3} % from NJOY at \
         {:.4e} eV — worse than the −0.14 % recorded on 2026-09-11. Graphite is the \
         control that makes the H(H2O) result (#188) attributable to water rather \
         than to this comparison method; if graphite drifts, that argument goes with it",
        100.0 * worst_inel.0,
        worst_inel.1
    );

    let mut worst_coh: (f64, f64) = (0.0, 0.0);
    for &(e, njoy) in GRAPHITE_XS_COHERENT {
        let ours = law.elastic_xs(e);
        if njoy == 0.0 {
            assert_eq!(
                ours, 0.0,
                "graphite's coherent-elastic cross section is {ours} b at {e} eV, below \
                 the first Bragg edge where it must be exactly zero. The Bragg cutoff \
                 has moved or is being smeared — a structural defect no percentage \
                 envelope would catch"
            );
            println!("  MT=230  {e:>9.4e}  below first Bragg edge: exactly zero, as NJOY");
            continue;
        }
        let rel = ours / njoy - 1.0;
        println!(
            "  MT=230  {e:>9.4e}  NJOY {njoy:>11.6}  ours {ours:>11.6}  {:>+6.2} %",
            100.0 * rel
        );
        if rel.abs() > worst_coh.0.abs() {
            worst_coh = (rel, e);
        }
    }
    assert!(
        worst_coh.0.abs() < GRAPHITE_XS_TOL,
        "graphite's coherent-elastic cross section is {:+.3} % from NJOY at {:.4e} eV \
         — worse than the −0.09 % recorded on 2026-09-11",
        100.0 * worst_coh.0,
        worst_coh.1
    );

    println!(
        "  worst: inelastic {:+.3} % at {:.4e} eV, coherent {:+.3} % at {:.4e} eV",
        100.0 * worst_inel.0,
        worst_inel.1,
        100.0 * worst_coh.0,
        worst_coh.1
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Graphite — scattering ANGLE
// ─────────────────────────────────────────────────────────────────────────────

/// **This crate's graphite S(α,β) scattering ANGLE matches NJOY2016's THERMR to
/// ≤ 0.009 absolute on μ̄, which excludes the thermal angle as the cause of the
/// FHR ring-RPT +4004 pcm residual.**
///
/// # Why this test exists
///
/// Every other thermal test in this file is an **energy-domain** test: cross
/// section (how often a neutron collides) and `⟨E′⟩/E` (how much energy it
/// loses). Neither constrains μ. Until 2026-09-12 the only assertion on a
/// thermal cosine anywhere in this crate was `(-1.0..=1.0).contains(&mu)` —
/// "is it a valid cosine". The angle sets σ_tr = σ_s(1 − μ̄), hence the
/// diffusion coefficient, hence the thermal flux shape in a heterogeneous cell,
/// which is the axis the residual hunt (`op-mzvp.2.12`) had narrowed to.
///
/// # Methodology
///
/// 200 000 samples of `ThermalScattering::sample` at each of the thirteen
/// energies of [`GRAPHITE_MUBAR`], on `tsl-crystalline-graphite` (MAT 30) at
/// **600 K — a tabulated temperature on that tape**, so no interpolation is
/// involved. The two thermal channels are separated by the one unambiguous
/// signature: coherent elastic is the only one that leaves `E′` exactly equal
/// to `E`.
///
/// The oracle is NJOY2016 THERMR, and its two halves are independent of each
/// other:
///
/// - **inelastic** — the 16 equally-probable laboratory cosines MF=6/MT=229
///   carries per outgoing-energy bin, folded with that record's own `f(E′)`;
/// - **coherent elastic** — the Bragg edge table `(E_i, f_i)` recovered from
///   the risers of the `E·σ_coh(E)` staircase in MF=3/MT=230, with
///   `μ_i = 1 − 2E_i/E`. This never reads this crate's edge table.
///
/// Deck and the full measured table: `examples/graphite_mubar_vs_njoy_thermr.rs`
/// and [`GRAPHITE_MUBAR`]'s own doc comment.
///
/// # Results (2026-09-12, NJOY2016 2016.79, ENDF/B-VIII.0)
///
/// Worst **+0.0085 absolute on μ̄_inelastic** at 0.0253 eV — a real 5σ
/// deviation on the 400 000-sample run the golden table was taken from, not
/// noise — and **+0.0030 on μ̄_elastic** at 0.2 eV.
///
/// # The bound is ABSOLUTE, and that is deliberate
///
/// μ̄_inelastic passes through zero near 0.9 eV. A *relative* envelope blows up
/// there for arithmetic reasons and means nothing — the same trap
/// [`GRAPHITE_KERNEL`] records for ξ. The envelopes are 0.02 (inelastic) and
/// 0.01 (elastic) absolute, sized to catch the measured deviation roughly
/// doubling.
///
/// # What a passing result here means for the residual
///
/// μ̄_total is ≈ 0.05 across the whole thermal range, so an error of 0.005 moves
/// σ_tr = σ_s(1 − μ̄) by **0.05 %**. That is the number to weigh against
/// +4004 pcm: the thermal scattering angle is excluded.
#[test]
fn graphite_sab_mean_cosine_against_njoy_thermr() {
    use outram_mc_libs::vv::njoy_golden::{
        GRAPHITE_MUBAR, GRAPHITE_MUBAR_ELASTIC_TOL, GRAPHITE_MUBAR_TOL,
    };
    const N: usize = 200_000;

    let Some(law) = law_or_skip("tsl-crystalline-graphite.endf", 30, 600.0, "c_Graphite") else {
        return;
    };
    let mut seed = 20_260_912_u64;
    let (mut worst_in, mut worst_el): ((f64, f64), (f64, f64)) = ((0.0, 0.0), (0.0, 0.0));
    for &(e, mu_in_njoy, mu_el_njoy, _mu_tot_njoy) in GRAPHITE_MUBAR {
        let (mut su_in, mut n_in, mut su_el, mut n_el) = (0.0, 0usize, 0.0, 0usize);
        for _ in 0..N {
            let Some((ep, mu)) = law.sample(e, &mut seed) else {
                continue;
            };
            assert!(
                (-1.0..=1.0).contains(&mu),
                "graphite returned an invalid cosine {mu} at {e} eV"
            );
            if (ep - e).abs() <= 1.0e-12 * e {
                su_el += mu;
                n_el += 1;
            } else {
                su_in += mu;
                n_in += 1;
            }
        }
        assert!(
            n_in > N / 100,
            "graphite produced almost no inelastic scatters at {e} eV"
        );
        let mu_in = su_in / n_in as f64;
        let d_in = mu_in - mu_in_njoy;
        if d_in.abs() > worst_in.0.abs() {
            worst_in = (d_in, e);
        }
        if n_el > N / 100 {
            let mu_el = su_el / n_el as f64;
            let d_el = mu_el - mu_el_njoy;
            if d_el.abs() > worst_el.0.abs() {
                worst_el = (d_el, e);
            }
            println!(
                "  {e:>9.4e}  inel NJOY {mu_in_njoy:>+8.5} ours {mu_in:>+8.5} ({d_in:>+8.5})  \
                 elas NJOY {mu_el_njoy:>+8.5} ours {mu_el:>+8.5} ({d_el:>+8.5})"
            );
        } else {
            println!(
                "  {e:>9.4e}  inel NJOY {mu_in_njoy:>+8.5} ours {mu_in:>+8.5} ({d_in:>+8.5})  \
                 elas: below the first Bragg edge, no channel"
            );
        }
    }
    assert!(
        worst_in.0.abs() < GRAPHITE_MUBAR_TOL,
        "graphite's inelastic mean cosine is {:+.5} from NJOY at {:.4e} eV — worse than \
         the +0.0085 recorded on 2026-09-12. Do NOT widen this envelope: it is the only \
         oracle on a thermal scattering angle in this crate",
        worst_in.0,
        worst_in.1
    );
    assert!(
        worst_el.0.abs() < GRAPHITE_MUBAR_ELASTIC_TOL,
        "graphite's coherent-elastic mean cosine is {:+.5} from the Bragg staircase at \
         {:.4e} eV — worse than the +0.0030 recorded on 2026-09-12",
        worst_el.0,
        worst_el.1
    );
    println!(
        "  worst: inelastic {:+.5} at {:.4e} eV, coherent-elastic {:+.5} at {:.4e} eV",
        worst_in.0, worst_in.1, worst_el.0, worst_el.1
    );
}

/// **This crate's graphite S(α,β) kernel has the right MEAN outgoing energy and
/// the wrong WIDTH: −2.7 % to −11.1 % narrow below 0.2 eV, and up to +39 % broad
/// at 2 eV. Measured 2026-09-12; worth −63 pcm, which is not the ring-RPT
/// residual.**
///
/// # Why a second moment
///
/// [`GRAPHITE_KERNEL`] is `⟨E′⟩/E`, the **first** moment, and it agrees with
/// NJOY to 0.1 % above 0.2 eV. A kernel can have exactly the right mean and the
/// wrong spread — and the spread is what decides how many neutrons cross the
/// 0.625 eV group boundary per collision, i.e. where the 1/E slowing-down
/// spectrum joins the Maxwellian. In a pebble whose `p` is 0.48, half the
/// neutron economy turns on that crossing. Nothing here had ever measured it.
///
/// # Methodology
///
/// 200 000 samples of `ThermalScattering::sample` at each of the thirteen
/// energies of [`GRAPHITE_KERNEL_WIDTH`], reduced to `sqrt(var(E′))/⟨E′⟩` with
/// the coherent-elastic channel removed by the `E′ == E` signature (it would
/// otherwise contribute a spurious zero-width spike). `tsl-crystalline-graphite`
/// (MAT 30) at **600 K — a tabulated temperature**. The oracle is a quadrature
/// on THERMR's MF=6/MT=229 matrix: no sampling on that side.
///
/// # The defect, and what it is
///
/// The sign flips at 0.39 eV, which is the tell. Below it, what is measured is
/// the 16-bin equiprobable representation's own truncation of the tails
/// (one-signed narrow). Above it, the emission tables sit on a 48-point log grid
/// over 1e-5 … 4 eV — **adjacent incident energies are 31.6 % apart** — and
/// `select_table` picks between the two bracketing tables by ACE statistical
/// interpolation. Mixing two tables whose means are 31.6 % apart adds a variance
/// `r(1−r)(m₂−m₁)²` the true kernel has not got, and above 0.4 eV, where the
/// intrinsic spread has fallen to `w ≈ 0.12`, that added variance dominates.
///
/// Raising `N_EMIT_GRID` from 48 to 192 collapses +39.0 % at 2 eV to −2.3 %,
/// which is the confirmation that the grid is the cause and not something in the
/// S(α,β) integration.
///
/// # It was priced, not assumed
///
/// `examples/fhr_ring_rpt_endf.rs` with `OUTRAM_RINGRPT_ONLY=csg`, same deck,
/// same seed, 2026-09-12: `N_EMIT_GRID = 48` gives k = 1.40745 ± 0.00214 and
/// `N_EMIT_GRID = 192` gives k = 1.40682 ± 0.00221 — **−63 pcm**, inside its own
/// sampling error, against the +4004 pcm residual under investigation. A real
/// defect, and not that one.
///
/// # The envelopes
///
/// 50 % across the table is a **characterisation** bound on the open defect, the
/// same contract [`H2O_KERNEL`] carries — sized to fail if it grows, not to
/// bless it. 15 % and one-signed below 0.2 eV is the genuine quality bound on
/// the equiprobable representation. **Tighten the first to ~15 % when the grid
/// is refined; never widen either.**
#[test]
fn graphite_sab_kernel_width_against_njoy_thermr() {
    use outram_mc_libs::vv::njoy_golden::{
        GRAPHITE_KERNEL_WIDTH, GRAPHITE_KERNEL_WIDTH_INTRINSIC_BELOW_EV,
        GRAPHITE_KERNEL_WIDTH_NARROW_TOL, GRAPHITE_KERNEL_WIDTH_TOL,
    };
    const N: usize = 200_000;

    let Some(law) = law_or_skip("tsl-crystalline-graphite.endf", 30, 600.0, "c_Graphite") else {
        return;
    };
    let mut seed = 20_260_912_u64;
    let (mut worst, mut worst_e) = (0.0_f64, 0.0_f64);
    for &(e, w_njoy) in GRAPHITE_KERNEL_WIDTH {
        let (mut s1, mut s2, mut k) = (0.0, 0.0, 0usize);
        for _ in 0..N {
            let Some((ep, _mu)) = law.sample(e, &mut seed) else {
                continue;
            };
            if (ep - e).abs() <= 1.0e-12 * e {
                continue;
            }
            s1 += ep;
            s2 += ep * ep;
            k += 1;
        }
        assert!(k > N / 100, "no inelastic scatters at {e} eV");
        let (m1, m2) = (s1 / k as f64, s2 / k as f64);
        let w = (m2 - m1 * m1).max(0.0).sqrt() / m1;
        let rel = w / w_njoy - 1.0;
        println!(
            "  {e:>9.4e}  NJOY {w_njoy:>8.5}  ours {w:>8.5}  {:>+7.2} %",
            100.0 * rel
        );
        if rel.abs() > worst.abs() {
            worst = rel;
            worst_e = e;
        }
        if e < GRAPHITE_KERNEL_WIDTH_INTRINSIC_BELOW_EV {
            assert!(
                rel < 0.0 && rel.abs() < GRAPHITE_KERNEL_WIDTH_NARROW_TOL,
                "graphite's kernel width is {:+.2} % from NJOY at {e:.4e} eV — below \
                 {GRAPHITE_KERNEL_WIDTH_INTRINSIC_BELOW_EV} eV it was one-signed narrow \
                 and inside {GRAPHITE_KERNEL_WIDTH_NARROW_TOL} on 2026-09-12",
                100.0 * rel
            );
        }
    }
    assert!(
        worst.abs() < GRAPHITE_KERNEL_WIDTH_TOL,
        "graphite's kernel width is {:+.2} % from NJOY at {worst_e:.4e} eV — worse than \
         the +39.0 % recorded on 2026-09-12. This is a CHARACTERISATION bound on a known \
         defect: do not widen it",
        100.0 * worst
    );
    println!("  worst {:+.2} % at {worst_e:.4e} eV", 100.0 * worst);
}
