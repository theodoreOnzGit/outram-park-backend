//! **V&V: LRF=7 (R-matrix limited) reconstruction with a background R-matrix,
//! against NJOY2016's own RECONR.**
//!
//! # Methodology
//!
//! **What is computed.** The crate's [`reconr`] reconstruction of Sr-88
//! (ENDF/B-VIII.1, MAT 3837 — ~~the only LRF=7 evaluation held in
//! `reference-data/endf/`~~ **CORRECTED 2026-10-01**: one of seven held there,
//! with Cl-35, Fe-54, Fe-57, Cu-63, Cu-65 and Mo-95), MF=3 MT=1/2/102, over its whole resolved-resonance
//! range `1e-5 .. 9.5e5 eV`.
//!
//! **Reference.** NJOY2016 upstream `ac5adf5f` (2016.79), built in-session with
//! gfortran 13.3.0, run on the same evaluation with the deck committed beside
//! the tape as `reference-data/reconr/sr88-ENDF8.1-0K-err0.001.njoy-input`:
//!
//! ```text
//! reconr
//! 20 22
//! 'sr88 lrf7 pendf'/
//! 3837 0/
//! .001/
//! 0/
//! stop
//! ```
//!
//! The comparison is made **on NJOY's own grid** — every one of the 44,326
//! points its PENDF carries inside the resolved range — so the crate is judged
//! where the reference chose to place points, not where it placed its own.
//! Duplicate energies (NJOY writes them at MF=3 discontinuities) keep the first.
//!
//! **Pass criterion.** ~~Worst relative deviation `\<= 2e-2` per MT at the deck's
//! own `err = 0.001`. That gate is deliberately loose, and the reason is in the
//! results below: what remains is linearisation, not physics, and this test
//! exists to catch the *physics* regressing.~~ **CORRECTED 2026-11-14 (gh:#1366
//! §2 item 3):** every `(E, sigma)` pair NJOY writes inside the window is
//! **bit-identical** to the port's, after the port's result has been through
//! PENDF text ([`ReconrResult::through_pendf_text`], NJOY's `a11`), with the same
//! pair count, duplicates included. No tolerance. The September residual was a
//! grid difference that the 2026-09-26 RECONR fixes (#340) removed;
//! `pendf_stages_vs_njoy2016` has been word-identical on this deck since then,
//! but this gate was never tightened.
//!
//! [`ReconrResult::through_pendf_text`]: njoy_outram_park_fork::reconr::ReconrResult::through_pendf_text
//!
//! # Why this test exists — the defect it pins
//!
//! Before 2026-09-14 this case was catastrophically wrong, and the crate did not
//! know it: `reconr`'s LRF=7 path returned the hard-sphere/potential term with
//! **no R-matrix resonance contribution reaching the elastic channel**
//! (gh:#202, `bn:op-hb9l`). Elastic came back as a flat `4 pi a^2 = 4.969327 b`
//! across the bottom of the resolved range where NJOY varies (8.843210 b at
//! 1e-5 eV) — worst relative error MT=1 **1.03e1** and MT=2 **1.04e1**, both at
//! 7.4368e5 eV, MT=102 **1.38e0**.
//!
//! The cause was **not** the one the issue's "where to look" note inferred.
//! `samm::run` returning `NotPorted` is vestigial and nothing calls it; the
//! reconstruction runs. What was missing is the **background R-matrix**
//! (`KBK > 0`, `samm.f90:1199-1256` reads it, `:3265-3295` applies it): the
//! parser advanced the cursor past those records and discarded them.
//!
//! Every one of Sr-88's 7 spin groups carries `KBK = 1` with `LCH = 2`,
//! `LBK = 2` — a SAMMY-parametrised background on the **elastic** channel. All
//! 443 of its resonances sit at or above 12.41 keV, so at thermal the resonance
//! sum has nothing to say and that background term carries essentially the
//! whole elastic cross section beyond hard-sphere scattering. Discarding it is
//! why the error was elastic-specific: capture, the eliminated channel, was
//! only mildly wrong (1.38e0) because its thermal value is a distant-resonance
//! `1/v` tail that *was* being computed.
//!
//! # Results (2026-09-14, this evaluation, NJOY2016 `ac5adf5f`)
//!
//! Worst relative deviation over all 44,326 NJOY grid points in the resolved
//! range:
//!
//! | MT | before the fix | after, `err = 0.001` | after, `err = 0.0001` |
//! |---|---|---|---|
//! | 1 (total) | 1.03e1 | 9.80e-3 | 2.39e-3 |
//! | 2 (elastic) | 1.04e1 | 1.00e-2 | 9.97e-4 |
//! | 102 (capture) | 1.38e0 | 9.98e-3 | 1.00e-3 |
//!
//! # Results (2026-11-14, develop `7ab655456`, same NJOY tape)
//!
//! | MT | old instrument: unrounded `eval_mt` vs NJOY text, worst relative | new instrument: pairs through `a11`, differing |
//! |---|---|---|
//! | 1 (total) | 6.88e-8 | **0 of 44 326** |
//! | 2 (elastic) | 9.30e-8 | **0 of 44 326** |
//! | 102 (capture) | 1.70e-7 | **0 of 44 326** |
//!
//! The old instrument's 1e-7 residual was NJOY's own 7-figure print rounding
//! (at most 5e-7 relative); predicted on gh:#1366 before the change, together
//! with the 0-differing result, and both held.
//!
//! ~~**The residual is linearisation, not physics, and that was measured rather
//! than assumed.**~~ (2026-09-14, superseded: no residual is left.) Tightening the reconstruction tolerance tenfold shrank the
//! worst deviation by very close to tenfold on MT=2 and MT=102 — the signature
//! of a grid/linearisation envelope. A physics error does not scale with `err`.
//!
//! Independently, at the three energies gh:#202 recorded NJOY values for, the
//! reconstruction now reproduces them at the 7-figure floor of NJOY's own
//! printed output:
//!
//! | E (eV) | NJOY | ours, before | ours, after |
//! |---|---|---|---|
//! | 1.00000e-5 | 8.843210 | 4.969327 | 8.843210 |
//! | 1.03125e-5 | 8.838604 | 4.969327 | 8.838613 |
//! | 1.06250e-5 | 8.834137 | 4.969327 | 8.834151 |
//!
//! # Interpretation, and what this does NOT establish
//!
//! LRF=7 reconstruction is now verified against NJOY on the one LRF=7
//! evaluation held, including the `LBK = 2` background form. It does **not**
//! verify: `LBK = 1` (tabulated background) or `LBK = 3` (Fröhner), neither of
//! which any held evaluation exercises; multi-channel LRF=7 groups (all seven
//! of Sr-88's groups have a single explicit channel); `KRM != 3`; `IFG = 1`;
//! or the eliminated channel appearing anywhere but first. Those remain open on
//! `bn:op-cjw.2`, and a second LRF=7 tape is what would close them.
//!
//! ~~**One divergence is left standing deliberately, and is not this test's
//! gate.** NJOY's MT=1 is the sum of all partials (`reconr.f90:73`), whereas
//! this crate builds MT=1 as its own MF=3 background plus the resonance total.
//! At 1e-5 eV NJOY's total (9.047157 b) equals *this crate's own*
//! MT=2 + MT=102 (8.843210 + 0.203947) exactly, while the crate reports
//! 9.025528 b — short by 2.1629e-2 b, which is precisely the MF=3 MT=102
//! background it leaves out of the total. That is a separate, smaller defect
//! with its own bead; it is visible in the MT=1 column above as the one entry
//! that does not scale cleanly with `err`.~~ **CORRECTED 2026-11-14:** gone. The
//! port's MT=1 is NJOY's in every one of the 44 326 pairs (table above), so it
//! is the sum of the partials as `reconr.f90` builds it.

use njoy_outram_park_fork::endf::mt::MtReaction;
use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use njoy_outram_park_fork::reference_data::{reference_file, reference_file_or_skip};

const MAT: i32 = 3837;
const EL: f64 = 1.0e-5;
const EH: f64 = 9.5e5;

/// Worst relative deviations recorded on 2026-09-14 (see the module doc),
/// printed beside today's count for the record. ~~Gated at `2e-2`~~ **CORRECTED
/// 2026-11-14**: the gate is now bit-identity of every pair.
const RECORDED: [(i32, f64); 3] = [(1, 9.80e-3), (2, 1.00e-2), (102, 9.98e-3)];

fn njoy_pairs(tape: &Tape, mt: i32) -> Vec<(f64, f64)> {
    let sec = tape
        .section(MAT, 3, mt)
        .unwrap_or_else(|| panic!("NJOY PENDF has MF=3 MT={mt}"));
    let mut cur = SectionCursor::new(&sec.rows);
    cur.read_cont().expect("MF=3 CONT header");
    cur.read_tab1().expect("MF=3 TAB1").pairs
}

#[test]
fn sr88_lrf7_background_rmatrix_reproduces_njoy2016() {
    let Some(pendf) = reference_file_or_skip(
        "reconr",
        "sr88-ENDF8.1-0K-err0.001.pendf",
        "Sr-88 LRF=7 RECONR golden",
    ) else {
        return;
    };
    let Some(eval) = reference_file("endf", "n-038_Sr_088-ENDF8.1.endf") else {
        eprintln!("skipping: reference-data/endf/n-038_Sr_088-ENDF8.1.endf absent");
        return;
    };

    let tape = Tape::read_file(&eval).expect("Sr-88 evaluation parses");
    let ours = reconr(
        &tape,
        &ReconrConfig {
            mat: MAT,
            tolerance: 0.001,
            temperature: 0.0,
        },
    )
    .expect("LRF=7 reconstruction runs");
    let njoy = Tape::read_file(&pendf).expect("NJOY PENDF parses");

    // Through PENDF text: NJOY's modules talk through tapes, and every value
    // NJOY writes passes through `a11` (7 significant figures). Comparing the
    // unrounded in-memory result against NJOY's text measured that rounding
    // (worst 1.70e-7 on 2026-11-14), not the reconstruction.
    let ours = ours.through_pendf_text();
    for (mt, recorded) in RECORDED {
        let rx = MtReaction::from_any(mt);
        let in_window = |p: &&(f64, f64)| (EL..=EH).contains(&p.0);
        let theirs: Vec<(f64, f64)> = njoy_pairs(&njoy, mt).iter().filter(in_window).copied().collect();
        let sec = ours
            .sections
            .iter()
            .find(|s| s.mt == rx)
            .unwrap_or_else(|| panic!("MT={mt} missing from the port's result"));
        let mine: Vec<(f64, f64)> = sec.pairs.iter().filter(in_window).copied().collect();

        assert!(
            theirs.len() > 40_000,
            "MT={mt}: only {} NJOY grid points inside the resolved range -- \
             the oracle or the range is wrong",
            theirs.len()
        );
        let differing: Vec<usize> = (0..theirs.len().min(mine.len()))
            .filter(|&i| theirs[i] != mine[i])
            .collect();
        println!(
            "  MT={mt:3}: NJOY {} pairs, ours {}; {} differing (E or sigma, bitwise); \
             recorded {recorded:.2e} worst relative on 2026-09-14",
            theirs.len(),
            mine.len(),
            differing.len()
        );
        assert_eq!(
            mine.len(),
            theirs.len(),
            "MT={mt}: ours carries {} pairs in the resolved range, NJOY {}",
            mine.len(),
            theirs.len()
        );
        if let Some(&i) = differing.first() {
            panic!(
                "MT={mt}: {} of {} pairs differ from NJOY2016's; first at index {i}: \
                 ours {:?} vs NJOY {:?}. A deviation at O(1) means the LRF=7 \
                 background R-matrix (KBK) has stopped being applied -- see this \
                 file's module doc, gh:#202, bn:op-hb9l.",
                differing.len(),
                theirs.len(),
                mine[i],
                theirs[i]
            );
        }
    }
}

/// The three energies gh:#202 recorded NJOY values for, checked directly.
///
/// This is deliberately separate from the sweep above: these are the numbers a
/// human wrote into the issue, so reproducing them is a check against the
/// written record rather than against a tape this session generated.
#[test]
fn sr88_elastic_matches_the_values_recorded_in_the_defect_report() {
    let Some(eval) = reference_file("endf", "n-038_Sr_088-ENDF8.1.endf") else {
        eprintln!("skipping: Sr-88 evaluation absent");
        return;
    };
    let tape = Tape::read_file(&eval).expect("tape");
    let ours = reconr(
        &tape,
        &ReconrConfig {
            mat: MAT,
            tolerance: 0.001,
            temperature: 0.0,
        },
    )
    .expect("reconstruction runs");

    // (E, NJOY elastic) as recorded in gh:#202. The pre-fix port returned a
    // flat 4.969327 b -- the potential term -- at all three.
    const REPORTED: [(f64, f64); 3] = [
        (1.00000e-5, 8.843210),
        (1.03125e-5, 8.838604),
        (1.06250e-5, 8.834137),
    ];
    for (e, reference) in REPORTED {
        let mine = ours.eval_mt(MtReaction::Mt2Elastic, e);
        let dev = (mine - reference).abs() / reference;
        println!("  E={e:.6e}  ours {mine:.6} vs NJOY {reference:.6}  rel {dev:.2e}");
        assert!(
            dev < 1.0e-5,
            "elastic at {e:e}: {mine:.6} vs the {reference:.6} recorded in gh:#202 \
             ({dev:.2e} relative). 4.969327 means the KBK background is gone again."
        );
    }
}
