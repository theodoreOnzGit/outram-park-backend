//! **V&V: under grid refinement, NJOY2016's own unresolved output converges
//! onto this crate's directly-evaluated values.**
//!
//! # The claim being tested, and why it needed a test
//!
//! `reconr_urr_kernel_vs_njoy2016.rs` establishes that at 8 of the 34 grid
//! points NJOY writes inside U-234's unresolved range, NJOY's value is an
//! interpolation of its own MF=2/MT=152 table rather than an evaluation.
//!
//! **That alone does not say which code is closer to the truth.** An
//! implementation correct at the table nodes and wrong between them would
//! produce identical evidence. Asserting "we are more accurate than NJOY" on
//! the strength of node agreement would be an overclaim, and this file exists
//! so the stronger statement is *measured* rather than asserted.
//!
//! # Methodology — refinement with the physics held fixed
//!
//! NJOY forces the unresolved-parameter interpolation law to lin-lin
//! (`unresr.f90:1057-1058` reads the file's `INT` and overwrites it with 2), so
//! **inserting lin-lin-interpolated parameter points is an exact operation**:
//! `D(E)`, `GX(E)`, `GN0(E)`, `GG(E)` and `GF(E)` are pointwise unchanged. What
//! changes is `eunr`, built from those energies — so RECONR evaluates its
//! kernel at more energies and interpolates at fewer.
//!
//! U-234's parameter grid (10 energies) was refined 16-fold to 145, and NJOY
//! re-run on the modified evaluation. Both tapes are committed:
//! `reference-data/reconr/u234-ENDF8.0-paramgrid-x16.endf` (input) and
//! `…-0K-err0.001-paramgrid-x16.pendf` (NJOY's output). The generator and the
//! full study are in
//! `verification_and_validation/urr_interpolation_study/`.
//!
//! # Results (2026-09-14), MT=18, relative deviation from this crate's kernel
//!
//! | E (eV) | NJOY baseline | NJOY x16 |
//! |---|---|---|
//! | 7.00000e3 | -9.00e-4 | -3.33e-5 |
//! | 4.368748e4 | 5.80e-3 | 1.90e-5 |
//! | 4.51800e4 | 6.25e-3 | 4.73e-5 |
//! | 5.25000e4 | 3.70e-3 | 0.00e+00 |
//! | 7.00000e4 | -4.62e-3 | 0.00e+00 |
//! | 8.00000e4 | -5.69e-3 | 0.00e+00 |
//! | 9.00000e4 | -4.71e-3 | -5.33e-7 |
//!
//! Three reach exact agreement; `9.0e4` reaches the 7-significant-figure floor
//! of NJOY's printed output. **NJOY's own output moves onto ours**, which is
//! what licenses the accuracy statement.
//!
//! Convergence is **not monotone everywhere** — at an intermediate x2
//! refinement, `7.0e3` and `9.0e4` get worse before improving, because
//! inserting points changes which neighbours bracket the target energy. The
//! full series is tabulated in the study.
//!
//! # Scope of the claim
//!
//! One material, one reaction, `LSSF = 0`, infinite dilution, 0 K. This says
//! nothing about NJOY's resolved-range reconstruction or its self-shielded URR
//! treatment (UNRESR/PURR), and it is verification, not validation — no
//! measurement is involved.

use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reference_data::{reference_file, reference_file_or_skip};
use njoy_outram_park_fork::unresr::mf2::parse_lru2_ranges;
use njoy_outram_park_fork::unresr::unresolved_cross_sections;
use njoy_outram_park_fork::unresr::wfun::WTable;

const MAT: i32 = 9225;
const SIG0: f64 = 1.0e10;

/// The energies at which baseline NJOY interpolates rather than evaluates.
const INTERPOLATED: [f64; 7] = [
    7.00000e3, 4.368748e4, 4.51800e4, 5.25000e4, 7.00000e4, 8.00000e4, 9.00000e4,
];

/// Baseline deviations, recorded 2026-09-14 — the "before" column.
const BASELINE: [f64; 7] = [-9.00e-4, 5.80e-3, 6.25e-3, 3.70e-3, -4.62e-3, -5.69e-3, -4.71e-3];

/// After 16-fold parameter-grid refinement every deviation must be at or below
/// this. The measured worst is 4.73e-5; the gate allows an order of margin so
/// it flags a real regression rather than last-digit drift.
const REFINED_GATE: f64 = 5.0e-4;

fn mf3_mt(tape: &Tape, mt: i32) -> Option<Vec<(f64, f64)>> {
    let sec = tape.section(MAT, 3, mt)?;
    let mut c = SectionCursor::new(&sec.rows);
    c.read_cont().ok()?;
    Some(c.read_tab1().ok()?.pairs)
}

fn at(pairs: &[(f64, f64)], e: f64) -> Option<f64> {
    pairs
        .iter()
        .find(|(x, _)| (x - e).abs() <= 1e-6 * e)
        .map(|&(_, y)| y)
}

/// Our kernel's MT=18 at `e`, at infinite dilution.
fn ours_at(e: f64) -> Option<f64> {
    let ep = reference_file("endf", "n-092_U_234-ENDF8.0.endf")?;
    let tape = Tape::read_file(&ep).ok()?;
    let sec = tape.section(MAT, 2, 151)?;
    let ranges = parse_lru2_ranges(&sec.rows[1..]).ok()?;
    let range = ranges.first()?;
    let table = WTable::new();
    let out = unresolved_cross_sections(
        std::slice::from_ref(range),
        e,
        0.0,
        &[SIG0],
        [0.0; 4],
        &table,
    )
    .ok()?;
    Some(out.first()?[2])
}

#[test]
fn refining_njoys_parameter_grid_moves_its_output_onto_ours() {
    let Some(refined) = reference_file_or_skip(
        "reconr",
        "u234-ENDF8.0-0K-err0.001-paramgrid-x16.pendf",
        "U-234 x16 refined RECONR golden",
    ) else {
        return;
    };
    let Some(base_p) = reference_file("reconr", "u234-ENDF8.0-0K-err0.001.pendf") else {
        eprintln!("skipping: baseline RECONR golden absent");
        return;
    };
    let base = mf3_mt(&Tape::read_file(&base_p).expect("baseline parses"), 18)
        .expect("baseline MF=3 MT=18");
    let x16 = mf3_mt(&Tape::read_file(&refined).expect("refined parses"), 18)
        .expect("refined MF=3 MT=18");

    let mut improved = 0usize;
    for (i, &e) in INTERPOLATED.iter().enumerate() {
        let (Some(b), Some(r), Some(o)) = (at(&base, e), at(&x16, e), ours_at(e)) else {
            eprintln!("  E={e:e}: missing data, skipped");
            continue;
        };
        let db = (b - o) / o;
        let dr = (r - o) / o;
        println!("  E={e:12.6e}  baseline {db:+.2e} (recorded {:+.2e})  x16 {dr:+.2e}", BASELINE[i]);

        assert!(
            dr.abs() <= REFINED_GATE,
            "E={e:e}: after 16x parameter-grid refinement NJOY still differs from \
             this crate by {dr:.2e}, above the {REFINED_GATE:.0e} gate. Either the \
             refinement no longer converges, or this crate's between-node \
             evaluation has regressed -- the accuracy claim in the module doc \
             rests on this."
        );
        assert!(
            dr.abs() < db.abs() || dr.abs() < 1.0e-6,
            "E={e:e}: refinement did not move NJOY closer to us \
             (baseline {db:.2e}, refined {dr:.2e}). Without that, the claim that \
             our evaluated value is the converged one is unsupported."
        );
        improved += 1;
    }
    assert!(
        improved >= 5,
        "only {improved} of {} energies could be compared",
        INTERPOLATED.len()
    );
}

/// **The control.** Refinement must not change the physics — otherwise the
/// convergence above would be meaningless.
///
/// At every energy where the *original* parameter grid has a node, NJOY
/// evaluates in both runs, so the two outputs must be identical. Measured
/// 2026-09-14: bit-identical at all of them, and across all 45,954 shared grid
/// points the median and 95th-percentile difference are both exactly zero.
#[test]
fn refinement_leaves_the_physics_unchanged_at_the_original_parameter_energies() {
    let (Some(base_p), Some(ref_p)) = (
        reference_file("reconr", "u234-ENDF8.0-0K-err0.001.pendf"),
        reference_file("reconr", "u234-ENDF8.0-0K-err0.001-paramgrid-x16.pendf"),
    ) else {
        eprintln!("skipping: RECONR goldens absent");
        return;
    };
    let base = mf3_mt(&Tape::read_file(&base_p).expect("baseline parses"), 18)
        .expect("baseline MF=3 MT=18");
    let x16 = mf3_mt(&Tape::read_file(&ref_p).expect("refined parses"), 18)
        .expect("refined MF=3 MT=18");

    // The evaluation's own unresolved parameter energies.
    const PARAM_ENERGIES: [f64; 8] = [2.5e3, 3.5e3, 5.0e3, 8.0e3, 1.5e4, 2.5e4, 4.0e4, 6.0e4];

    let mut checked = 0usize;
    for &e in &PARAM_ENERGIES {
        let (Some(b), Some(r)) = (at(&base, e), at(&x16, e)) else {
            continue;
        };
        let dev = (r - b).abs() / b.abs();
        println!("  E={e:10.4e}: baseline {b:.9e}  refined {r:.9e}  ({dev:.1e})");
        assert!(
            dev == 0.0,
            "E={e:e}: refinement changed NJOY's value at an ORIGINAL parameter \
             energy ({b:.9e} -> {r:.9e}, {dev:.2e}). Lin-lin resampling of a \
             lin-lin-interpolated function must be exact, so a non-zero change \
             means the densified tape altered the physics -- and the convergence \
             study built on it is invalid."
        );
        checked += 1;
    }
    assert!(checked >= 6, "only {checked} parameter energies present in both tapes");
}
