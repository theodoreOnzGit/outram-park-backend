//! GASPR cross-code verification: gas-production cross sections (MT=203–207)
//! against **NJOY2016's own output**, on five light nuclides.
//!
//! # Methodology
//!
//! **Oracle.** NJOY2016 at commit `ac5adf5f` (`/home/user/njoy2016-src`, built
//! from source and executed for this comparison — not a stored number), run as
//!
//! ```text
//! moder  20 -21 /
//! reconr -21 -22 /  'pendf' /  <mat> 0 /  0.001 /  0 /
//! gaspr  -21 -22 -23 /
//! moder  -23 24 /
//! ```
//!
//! on the committed ENDF/B-VIII.0 evaluations in `reference-data/endf/`. The
//! resulting PENDF tapes, trimmed to MF=1 plus MF=3/MT=203–207, are committed
//! as `reference-data/gaspr/<case>-ENDF8.0-0K-err0.001.gaspr.pendf` with the
//! deck that produced each one beside it, so the reference regenerates rather
//! than being trusted.
//!
//! **Our side.** `reconr` at the same 0.001 tolerance, then
//! [`GasProduction::from_reconr`]. Both codes therefore start from the same
//! evaluation and the same reconstruction tolerance; what is being compared is
//! the gas *bookkeeping*, not the reconstruction.
//!
//! **Comparison.** The two sides tabulate on different grids by construction —
//! NJOY writes gas production on the MT=1 union grid above its gas threshold
//! `thrg`, this port on the union of the contributing sections' own grids — so
//! both are lin-lin interpolated at 600 log-spaced energies across the
//! overlap, and the worst relative difference per MT is asserted.
//!
//! Two classes of point are excluded from the assertion, and both are stated
//! rather than tuned away:
//!
//! - **Below `1e-6` of the reference section's own peak.** Immediately above a
//!   threshold each code interpolates along its own first chord, so the two
//!   disagree by 100 % relative while differing by ~1e-9 b in absolute terms.
//!   Asserting there measures grid placement, not bookkeeping. The thresholds
//!   themselves are *not* waived — they are asserted separately, to 1 %.
//! - Points where both sides are below `1e-12` b.
//!
//! **Pass criterion.** 1 % per MT. Both sides interpolate linearly between
//! *different* node sets, so a curved σ(E) shows a chord difference that is
//! bounded by RECONR's own `errmax = 10·err = 1 %` convergence, exactly as the
//! BROADR golden test documents.
//!
//! # The five cases, and what each one isolates
//!
//! | case | what it tests |
//! |---|---|
//! | **Li-6** | MT=105 residual = ⁴He, and `LR=32` breakup on 30 inelastic levels |
//! | **Be-9** | the `izr = 4008` rule — ⁸Be counts as **two** alphas |
//! | **B-10** | `LR=22/28/35` breakup, and a non-gas residual (⁷Li) as control |
//! | **H-2** | MT=102 radiative capture producing ³H, from the residual alone |
//! | **C-12** | `LR=23` breakup (3α) on the inelastic levels |
//!
//! Between them these exercise every one of the six residual rules that fires
//! on a real evaluation, plus the `LR` half of the ejectile table. Neither is
//! reachable from a lookup keyed on MT alone.
//!
//! # Results (2026-09-17, ENDF/B-VIII.0, NJOY2016 `ac5adf5f`)
//!
//! All 17 gas-production sections the two codes both produce agree, with the
//! **worst relative difference over any of them 4.9e-3** and the typical one
//! between 1e-4 and 8e-4:
//!
//! | case | MT=203 (p) | MT=204 (d) | MT=205 (t) | MT=207 (α) |
//! |---|---|---|---|---|
//! | Li-6 | 4.5e-5 | 3.7e-3 | **7.5e-4** | **7.5e-4** |
//! | Be-9 | 4.4e-5 | 2.1e-5 | 6.1e-5 | **5.5e-5** |
//! | B-10 | 4.3e-4 | 4.9e-3 | **6.9e-4** | **8.7e-4** |
//! | H-2  | 3.8e-3 | — | **8.3e-4** | — |
//! | C-12 | **3.1e-5** | **1.9e-7** | — | 1.4e-4 |
//!
//! (Bold = a value that depends on the residual-nucleus rule, the `LR`
//! breakup table, or the MF=6/MT=5 yields — i.e. on something a lookup keyed
//! on MT alone cannot produce.) Neither code produces a section the other
//! does not. Every thresholds pair agrees to better than 1 %.
//!
//! **Three real defects were found by this comparison and fixed**, which is
//! the reason it exists:
//!
//! 1. **The residual nucleus was not credited at all.** ⁶Li(n,t) lost its
//!    alpha, ⁹Be(n,2n) produced no helium, ²H(n,γ) produced no tritium.
//! 2. **RECONR did not synthesise the lumped MT=103–107** for an evaluation
//!    carrying only the discrete MT=600–849 levels. B-10 has MT=700 but no
//!    MT=105, so its tritium production came out 3.19e-5 b against NJOY's
//!    3.01e-2 b — a factor of 940 — and its alpha production 0.568 b against
//!    1.004 b. Fixed in `reconr::synthesise_lumped_particle_channels`.
//! 3. **MT=5's energy-dependent MF=6 yields were not read.** C-12's proton
//!    and deuteron production above 14.5 MeV was identically zero against
//!    NJOY's 3.5e-2 b and 1.3e-2 b. Fixed by
//!    [`GasProduction::from_reconr_and_tape`].
//!
//! The write-up is
//! `verification_and_validation/gaspr_light_nuclides_vs_njoy2016.md`.

use njoy_outram_park_fork::endf::interp::eval_tab1;
use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::gaspr::{GasProduction, GasSpecies};
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use njoy_outram_park_fork::reference_data::{reference_endf_or_skip, reference_file_or_skip};

const TOL: f64 = 1.0e-2;
const ABS_FLOOR: f64 = 1.0e-12;
/// Fraction of the reference section's own peak below which a point is a
/// threshold-chord artefact rather than a bookkeeping difference.
const PEAK_FRACTION: f64 = 1.0e-6;
/// Tolerance on the two codes' first-nonzero energy for a given MT.
const THRESHOLD_TOL: f64 = 1.0e-2;
const N_SAMPLES: usize = 600;

struct Case {
    label: &'static str,
    endf: &'static str,
    mat: i32,
    pendf: &'static str,
}

const CASES: &[Case] = &[
    Case {
        label: "li6",
        endf: "n-003_Li_006-ENDF8.0.endf",
        mat: 325,
        pendf: "li6-ENDF8.0-0K-err0.001.gaspr.pendf",
    },
    Case {
        label: "be9",
        endf: "n-004_Be_009-ENDF8.0.endf",
        mat: 425,
        pendf: "be9-ENDF8.0-0K-err0.001.gaspr.pendf",
    },
    Case {
        label: "b10",
        endf: "n-005_B_010-ENDF8.0.endf",
        mat: 525,
        pendf: "b10-ENDF8.0-0K-err0.001.gaspr.pendf",
    },
    Case {
        label: "h2",
        endf: "n-001_H_002-ENDF8.0.endf",
        mat: 128,
        pendf: "h2-ENDF8.0-0K-err0.001.gaspr.pendf",
    },
    Case {
        label: "c12",
        endf: "n-006_C_012-ENDF8.0.endf",
        mat: 625,
        pendf: "c12-ENDF8.0-0K-err0.001.gaspr.pendf",
    },
];

const SPECIES: [(GasSpecies, i32); 5] = [
    (GasSpecies::H1, 203),
    (GasSpecies::H2, 204),
    (GasSpecies::H3, 205),
    (GasSpecies::He3, 206),
    (GasSpecies::He4, 207),
];

/// The reaction threshold implied by a tabulation: the last energy carrying a
/// zero cross section immediately below the first non-zero one, which is how
/// ENDF (and both codes' grids) mark a threshold. `None` when the tabulation
/// is non-zero from its first point, i.e. the channel has no threshold.
fn threshold_of(pairs: &[(f64, f64)]) -> Option<f64> {
    let first_nz = pairs.iter().position(|&(_, y)| y > 0.0)?;
    if first_nz == 0 {
        return None;
    }
    Some(pairs[first_nz - 1].0)
}

fn check(case: &Case) {
    let tag = format!("gaspr-njoy {}", case.label);
    let Some(endf_path) = reference_endf_or_skip(case.endf, &tag) else {
        return;
    };
    let Some(pendf_path) = reference_file_or_skip("gaspr", case.pendf, &tag) else {
        return;
    };
    let endf = Tape::read_file(&endf_path).expect("ENDF parses");
    let njoy = Tape::read_file(&pendf_path).expect("NJOY gas PENDF parses");
    let recon = reconr(
        &endf,
        &ReconrConfig {
            mat: case.mat,
            tolerance: 0.001,
            temperature: 0.0,
        },
    )
    .expect("RECONR");
    let gas = GasProduction::from_reconr_and_tape(&recon, &endf, case.mat);

    let mut failures = Vec::new();
    let mut compared = 0usize;
    for (species, mt) in SPECIES {
        let njoy_sec = njoy.section(case.mat, 3, mt);
        let ours_present = gas.energy.iter().any(|&e| gas.eval(species, e) > 0.0);
        let Some(njoy_sec) = njoy_sec else {
            assert!(
                !ours_present,
                "[{tag}] MT={mt}: NJOY produced no section but this port does \
                 — a gas channel is being invented"
            );
            continue;
        };
        let mut cur = SectionCursor::new(&njoy_sec.rows);
        let _ = cur.read_cont().unwrap();
        let tab = cur.read_tab1().unwrap();

        let e_lo = tab.pairs[0].0.max(gas.energy[0]);
        let e_hi = tab.pairs.last().unwrap().0.min(*gas.energy.last().unwrap());
        assert!(
            e_hi > e_lo,
            "[{tag}] MT={mt}: no overlap between the two grids \
             (njoy {:.3e}..{:.3e} eV, ours {:.3e}..{:.3e} eV)",
            tab.pairs[0].0,
            tab.pairs.last().unwrap().0,
            gas.energy[0],
            gas.energy.last().unwrap()
        );

        // The two codes must agree on where the channel opens. ENDF writes a
        // threshold as an explicit node with sigma = 0, so the threshold is
        // the last zero *before* the first non-zero point -- not the first
        // non-zero point itself, which only says where each code's grid
        // happened to put its next node.
        let njoy_threshold = threshold_of(&tab.pairs);
        let ours: Vec<(f64, f64)> = gas
            .energy
            .iter()
            .map(|&e| (e, gas.eval(species, e)))
            .collect();
        let our_threshold = threshold_of(&ours);
        let thr_rel = match (our_threshold, njoy_threshold) {
            (Some(a), Some(b)) => (a - b).abs() / b.abs().max(1.0e-30),
            _ => 0.0,
        };
        if thr_rel >= THRESHOLD_TOL {
            failures.push(format!(
                "MT={mt} threshold {:.6e} eV against njoy {:.6e} eV ({thr_rel:.3e} relative)",
                our_threshold.unwrap_or(f64::NAN),
                njoy_threshold.unwrap_or(f64::NAN),
            ));
        }

        let peak = tab.pairs.iter().map(|&(_, y)| y).fold(0.0f64, f64::max);
        let floor = (PEAK_FRACTION * peak).max(ABS_FLOOR);
        let (mut worst, mut at, mut got_w, mut want_w) = (0.0f64, 0.0, 0.0, 0.0);
        // Start 1 % above the overlap: at the threshold node itself the
        // reference is identically zero by construction, so a relative
        // comparison there measures nothing. Threshold agreement is asserted
        // above, on the same 1 %.
        let e_from = e_lo * (1.0 + THRESHOLD_TOL);
        if e_from >= e_hi {
            continue;
        }
        for i in 0..=N_SAMPLES {
            let e = e_from * (e_hi / e_from).powf(i as f64 / N_SAMPLES as f64);
            let want = eval_tab1(e, &tab.interp, &tab.pairs).unwrap();
            let got = gas.eval(species, e);
            let scale = got.abs().max(want.abs());
            if scale <= floor {
                continue;
            }
            let rel = (got - want).abs() / scale;
            if rel > worst {
                worst = rel;
                at = e;
                got_w = got;
                want_w = want;
            }
        }
        compared += 1;
        println!(
            "[{tag}] MT={mt} njoy {:5} pts over {:.3e}..{:.3e} eV: worst {:.3e} at {:.4e} eV \
             (crate {:.6e} b, njoy {:.6e} b)",
            tab.pairs.len(),
            tab.pairs[0].0,
            tab.pairs.last().unwrap().0,
            worst,
            at,
            got_w,
            want_w
        );
        if worst >= TOL {
            failures.push(format!(
                "MT={mt} deviates {worst:.3e} at {at:.4e} eV \
                 (crate {got_w:.6e} b, njoy {want_w:.6e} b), tol {TOL:e}"
            ));
        }
    }
    assert!(compared > 0, "[{tag}] no MT=203–207 section compared");
    assert!(failures.is_empty(), "{tag}: {}", failures.join("; "));
}

/// ⁶Li: MT=105 `(n,t)` whose residual *is* an alpha, plus `LR=32` deuteron
/// breakup on 30 inelastic levels. Both halves of the yield are exercised, and
/// the alpha leg is the one an MT-keyed lookup loses entirely.
#[test]
fn li6_gas_production_matches_njoy() {
    check(&CASES[0]);
}

/// ⁹Be: the only residual rule with multiplicity two. `(n,2n)` leaves ⁸Be,
/// which is particle-unbound and counts as two alphas — without the rule
/// beryllium shows zero helium production.
#[test]
fn be9_gas_production_matches_njoy() {
    check(&CASES[1]);
}

/// ¹⁰B: `LR=22/28/35` breakup on the inelastic levels, and `(n,α)` leaving
/// ⁷Li — a residual that is *not* a gas, so the rule must stay silent.
#[test]
fn b10_gas_production_matches_njoy() {
    check(&CASES[2]);
}

/// ²H: MT=102 radiative capture. The evaluation names no ejectile at all, so
/// every barn of the tritium production comes from the residual rule.
#[test]
fn h2_gas_production_matches_njoy() {
    check(&CASES[3]);
}

/// ¹²C: `LR=23` (3α) breakup on the inelastic levels — the largest `LR`
/// multiplicity that appears in `reference-data/endf/`.
#[test]
fn c12_gas_production_matches_njoy() {
    check(&CASES[4]);
}
