//! **V&V: MF=3 MT=1 is the sum of its parts, as NJOY2016 builds it.**
//!
//! # Methodology
//!
//! **What is computed.** The crate's [`reconr`] MT=1 (total) cross section,
//! checked two ways against NJOY2016's own RECONR output:
//!
//! 1. against NJOY's MT=1 directly, at energies chosen at the bottom of the
//!    resolved range where the defect below is visible and linearisation is not;
//! 2. against **this crate's own** reconstructed partials, summed — which is
//!    the invariant `reconr.f90:72-73` states and the property that was broken.
//!
//! **Inputs.** Both materials in `reference-data/reconr/` with a committed
//! 0 K RECONR oracle and an evaluation to reconstruct from:
//!
//! | material | MAT | `err` | why it is here |
//! |---|---|---|---|
//! | Si-30 (ENDF/B-VIII.0) | 1431 | 0.001 | the **control** — its MT=1 background already contains the partials', so a correct fix changes nothing |
//! | Sr-88 (ENDF/B-VIII.1) | 3837 | 0.001 | the **case** — MF=3 MT=1 and MT=2 backgrounds are identically zero while MT=102 carries a `1/v` remainder |
//!
//! **Reference.** NJOY2016 upstream `ac5adf5f` (2016.79), gfortran 13.3.0; the
//! decks are committed beside each tape.
//!
//! **Pass criterion.** MT=1 within 1e-5 relative of NJOY at the probe energies,
//! and within 1e-9 relative of the crate's own summed partials everywhere they
//! are compared.
//!
//! # The defect this pins (`bn:op-u9jp`)
//!
//! RECONR's own header says *"Redundant reactions are reconstructed to be the
//! sum of their parts"* (`reconr.f90:72-73`), and `emerge` implements it by
//! walking every MF=3 section and accumulating it into each redundant target
//! (`reconr.f90:4840-4893`; for `mtr = 1` it falls straight through to label
//! 400 with no per-MT test of its own).
//!
//! This crate instead built MT=1 as *its own MF=3 background plus the resonance
//! total*. Those agree only when the evaluation's MT=1 background already
//! contains every partial's background. Sr-88 is the case where it does not:
//! at 1e-5 eV NJOY's total is 9.047157 b, which is exactly this crate's own
//! MT=2 + MT=102 (8.843210 + 0.203947), while the crate reported **9.025528 b**
//! — short by 2.1629e-2 b, precisely the MF=3 MT=102 background it omitted.
//! The partials were right; only their sum was wrong.
//!
//! It survived because it is invisible on the common case. Si-30 agreed to
//! every digit both before and after.
//!
//! # Results (2026-09-14)
//!
//! | material | E | NJOY | before | after |
//! |---|---|---|---|---|
//! | Sr-88 | 1e-5 eV | 9.047157 | 9.025528 | 9.047157 |
//! | Sr-88 | 1e-3 eV | 8.187682 | 8.185519 | 8.187682 |
//! | Sr-88 | 1.0 eV | 7.204380 | 7.204312 | 7.204380 |
//! | Si-30 | 1e-5 eV | 7.843335 | 7.843335 | 7.843335 |
//! | Si-30 | 1.0 eV | 2.474378 | 2.474379 | 2.474379 |
//!
//! # The fission rule, and how it was nearly got backwards
//!
//! Which sections count as "parts" is not simply ENDF's redundancy table — it
//! depends on where *this crate* puts the resonance contribution.
//!
//! Upstream `anlyzd` builds MT=18 **from** MT=19/20/21/38 when those are
//! present (`reconr.f90:553-556`, setting `mtr18`). Copying that rule here is
//! wrong, and measurably so: `assemble`'s `targets` list reconstructs
//! `Mt18Fission` and leaves MT=19..21/38 as pure background. On U-234
//! (ENDF/B-VIII.0, MAT 9225), which carries all five sections, at 1e-5 eV
//! MT=19, 20, 21 and 38 are each exactly `0.0` while **MT=18 is
//! 3.448068842 b**. Summing the parts instead of MT=18 dropped precisely that
//! 3.448 b and moved the total from NJOY's 5.206555e3 — matched to 3.2e-8 — out
//! to 5.203107e3, a **6.6e-4** relative error.
//!
//! That is a silent 0.066 % hole in the total cross section of a **Godiva
//! nuclide**, introduced by a fix. It was caught by running the change against
//! NJOY on U-234 rather than assuming the redundancy table transfers, and the
//! rule is now spelled out in `rebuild_total_as_sum_of_parts` rather than left
//! to inference.
//!
//! # No effect on the transport nuclides — measured, not assumed
//!
//! Because MT=1 feeds the transport total, a paired A/B was run on every
//! nuclide behind the Godiva and LCT008 cases before this change was kept:
//! U-235, U-238, U-234, O-16, F-19 and C-12, at 1e-5 / 1 / 1e3 / 1e6 eV. All
//! are **bit-identical** between the old and new constructions, bar a
//! last-digit 2.2e-10 wobble on U-235 at 1 eV from summation order. No k-eff
//! baseline moves.
//!
//! # What this does NOT establish
//!
//! Only MT=1 is rebuilt. The other redundant reactions RECONR names — MT=3
//! (nonelastic), MT=4 (sum of MT=51..91), MT=18 where MT=19..21/38 are present,
//! and MT=103..107 where the discrete MT=600..849 levels are present — are
//! **not** reconstructed as sums by this crate, and this test does not check
//! them. Whether any evaluation held here exercises that gap is unmeasured.
//!
//! The worst-deviation figures over the full grid are unchanged by this fix
//! (Si-30 5.17e-3, Sr-88 9.80e-3) because those points sit at high energy where
//! linearisation dominates, not where the omitted background does. This test
//! therefore probes at the bottom of the range deliberately; the full-grid
//! sweep lives in `reconr_sr88_lrf7_kbk_njoy_golden.rs`.

use njoy_outram_park_fork::endf::mt::MtReaction;
use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use njoy_outram_park_fork::reference_data::reference_file;

struct Case {
    evaluation: &'static str,
    oracle: &'static str,
    mat: i32,
    /// Probe energies, chosen low enough that linearisation is negligible.
    probes: &'static [f64],
}

const CASES: [Case; 2] = [
    Case {
        evaluation: "n-014_Si_030-ENDF8.0.endf",
        oracle: "si30-ENDF8.0-0K.pendf",
        mat: 1431,
        probes: &[1.0e-5, 1.0e-3, 1.0],
    },
    Case {
        evaluation: "n-038_Sr_088-ENDF8.1.endf",
        oracle: "sr88-ENDF8.1-0K-err0.001.pendf",
        mat: 3837,
        probes: &[1.0e-5, 1.0e-3, 1.0],
    },
];

fn njoy_mt1(tape: &Tape, mat: i32) -> Vec<(f64, f64)> {
    let sec = tape.section(mat, 3, 1).expect("NJOY PENDF has MF=3 MT=1");
    let mut cur = SectionCursor::new(&sec.rows);
    cur.read_cont().expect("CONT");
    cur.read_tab1().expect("TAB1").pairs
}

/// Value of NJOY's MT=1 at exactly `e`, or `None` if its grid has no such point.
fn njoy_at(pairs: &[(f64, f64)], e: f64) -> Option<f64> {
    pairs
        .iter()
        .find(|(x, _)| (x - e).abs() <= 1e-9 * e.abs().max(1.0))
        .map(|&(_, y)| y)
}

#[test]
fn mt1_reproduces_njoy2016_where_the_partials_carry_background() {
    let mut ran = 0;
    for case in &CASES {
        let (Some(ep), Some(op)) = (
            reference_file("endf", case.evaluation),
            reference_file("reconr", case.oracle),
        ) else {
            eprintln!("skipping {}: reference data absent", case.evaluation);
            continue;
        };
        ran += 1;

        let tape = Tape::read_file(&ep).expect("evaluation parses");
        let ours = reconr(
            &tape,
            &ReconrConfig {
                mat: case.mat,
                tolerance: 0.001,
                temperature: 0.0,
            },
        )
        .expect("reconstruction runs");
        let njoy = njoy_mt1(&Tape::read_file(&op).expect("oracle parses"), case.mat);

        for &e in case.probes {
            let Some(reference) = njoy_at(&njoy, e) else {
                continue;
            };
            let mine = ours.eval_mt(MtReaction::Mt1Total, e);
            let dev = (mine - reference).abs() / reference.abs();
            println!(
                "  {} MT=1 at E={e:.3e}: ours {mine:.6e} vs NJOY {reference:.6e} ({dev:.2e})",
                case.evaluation
            );
            assert!(
                dev < 1.0e-5,
                "{} MT=1 at {e:e}: {mine:.6e} vs NJOY {reference:.6e} ({dev:.2e}). \
                 A shortfall here means MT=1 has stopped being the sum of its parts \
                 (reconr.f90:72-73) -- see bn:op-u9jp.",
                case.evaluation
            );
        }
    }
    assert!(ran > 0, "no reference data available for any case");
}

/// MT=1 equals the sum of the crate's own reconstructed partials.
///
/// Independent of the oracle: this is RECONR's stated invariant checked against
/// the crate's own output, so it still holds if the reference tapes are absent.
#[test]
fn mt1_equals_the_sum_of_our_own_partials() {
    let mut ran = 0;
    for case in &CASES {
        let Some(ep) = reference_file("endf", case.evaluation) else {
            continue;
        };
        ran += 1;
        let tape = Tape::read_file(&ep).expect("evaluation parses");
        let ours = reconr(
            &tape,
            &ReconrConfig {
                mat: case.mat,
                tolerance: 0.001,
                temperature: 0.0,
            },
        )
        .expect("reconstruction runs");

        // The MF=3 sections the evaluation carries, and which of them are parts.
        let present: Vec<i32> = (1..900)
            .filter(|&m| tape.section(case.mat, 3, m).is_some())
            .collect();
        let has = |lo: i32, hi: i32| present.iter().any(|&m| (lo..=hi).contains(&m));
        let has_fission_parts = present
            .iter()
            .any(|&m| (19..=21).contains(&m) || m == 38);
        let is_part = |mt: i32| -> bool {
            if mt == 1 || mt == 3 || mt == 4 || mt == 10 || (46..=49).contains(&mt) {
                return false;
            }
            if mt > 200 && mt < 600 {
                return false;
            }
            if mt == 18 && has_fission_parts {
                return false;
            }
            match mt {
                103 => !has(600, 649),
                104 => !has(650, 699),
                105 => !has(700, 749),
                106 => !has(750, 799),
                107 => !has(800, 849),
                _ => true,
            }
        };

        for &e in case.probes {
            let sum: f64 = present
                .iter()
                .copied()
                .filter(|&m| is_part(m))
                .map(|m| ours.eval_mt(MtReaction::from_any(m), e))
                .sum();
            let total = ours.eval_mt(MtReaction::Mt1Total, e);
            let dev = (total - sum).abs() / sum.abs().max(1e-30);
            println!(
                "  {} at E={e:.3e}: MT=1 {total:.9e} vs summed parts {sum:.9e} ({dev:.2e})",
                case.evaluation
            );
            assert!(
                dev < 1.0e-9,
                "{} at {e:e}: MT=1 {total:.9e} != sum of parts {sum:.9e} ({dev:.2e})",
                case.evaluation
            );
        }
    }
    assert!(ran > 0, "no evaluations available");
}

/// U-234 carries MT=18 *and* MT=19/20/21/38, and is the case that decides the
/// fission rule. Guards it without needing the 9 MB PENDF committed.
///
/// The NJOY value below was measured on 2026-09-14 with the deck committed at
/// `reference-data/reconr/u234-ENDF8.0-0K-err0.001.njoy-input`, NJOY2016
/// `ac5adf5f`, gfortran 13.3.0 — regenerate it with that deck to re-derive.
#[test]
fn u234_total_carries_fission_from_mt18_not_its_zeroed_sub_parts() {
    let Some(ep) = reference_file("endf", "n-092_U_234-ENDF8.0.endf") else {
        eprintln!("skipping: U-234 evaluation absent");
        return;
    };
    let tape = Tape::read_file(&ep).expect("evaluation parses");
    let ours = reconr(
        &tape,
        &ReconrConfig {
            mat: 9225,
            tolerance: 0.001,
            temperature: 0.0,
        },
    )
    .expect("reconstruction runs");

    const E: f64 = 1.0e-5;
    /// NJOY2016 MF=3 MT=1 for U-234 at 1e-5 eV (see this test's doc comment).
    const NJOY_TOTAL: f64 = 5.206555e3;

    let mt18 = ours.eval_mt(MtReaction::from_any(18), E);
    let subs: f64 = [19, 20, 21, 38]
        .iter()
        .map(|&m| ours.eval_mt(MtReaction::from_any(m), E))
        .sum();
    let total = ours.eval_mt(MtReaction::Mt1Total, E);

    println!("  U-234 at {E:e}: MT=18 {mt18:.9e}, MT=19+20+21+38 {subs:.9e}, MT=1 {total:.9e}");

    // The premise: the sub-parts are empty here and MT=18 carries the fission.
    assert!(
        mt18 > 1.0,
        "premise broken: MT=18 is {mt18:.3e}, expected the reconstructed \
         resonance fission (3.448068842 b on 2026-09-14). If the crate has \
         moved the resonance contribution onto MT=19..21, this test's rule \
         -- and rebuild_total_as_sum_of_parts -- must be revisited."
    );
    assert!(
        subs.abs() < 1.0e-12,
        "premise broken: MT=19+20+21+38 sum to {subs:.3e}, expected 0.0"
    );

    // The consequence: the total must contain MT=18's fission.
    let dev = (total - NJOY_TOTAL).abs() / NJOY_TOTAL;
    assert!(
        dev < 1.0e-5,
        "U-234 MT=1 at {E:e} is {total:.9e} against NJOY's {NJOY_TOTAL:.6e} \
         ({dev:.2e}). A shortfall of about {mt18:.3e} b means MT=1 is summing \
         the zeroed MT=19..21/38 instead of MT=18 -- see bn:op-u9jp."
    );
}
