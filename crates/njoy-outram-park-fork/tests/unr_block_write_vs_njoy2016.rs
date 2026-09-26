// SPDX-License-Identifier: GPL-3.0

//! **The ACE UNR block writer, checked against NJOY2016's own blocks** —
//! GitHub #325.
//!
//! # Methodology
//!
//! Two gates, each against NJOY2016 and neither against this crate's reader
//! alone.
//!
//! 1. **The writer, isolated from PURR.** PURR samples resonance ladders.
//!    ~~so a table *generated* here will not match NJOY's band for band~~
//!    (**CORRECTED 2026-09-26**: it does now, gate 3), but a generated table
//!    tests the writer and the generator together. So: read NJOY's own UNR block out of the
//!    reference U-234/235/238 tables in `reference-data/ace` (made by
//!    `RECONR+BROADR+PURR+ACER`), hand those tables to
//!    [`unr_words`], and require **every word** — value and integer typing —
//!    to equal NJOY's. Any difference is the writer's: the layout, the
//!    column order, the cumulative rounding, the pinned last band, or the
//!    `LSSF`-dependent heating unit.
//! 2. **The competition flags, isolated from the bands.** `ILF`/`IOA` are
//!    derived from MF=3 thresholds, not from ladders, so the ported rule
//!    (`purr.f90:1110-1192`) must reproduce NJOY's flags exactly from the same
//!    evaluation. PURR's statistical controls are set minimal because the flags
//!    do not depend on them.
//!
//! 3. **PURR itself, from the evaluation.** `UrrProbabilityTables::from_endf`
//!    at NJOY's deck (293.6 K, 20 bins, 64 ladders, 10 000 samples) must give
//!    NJOY's UNR block **word for word**. `rann` and its seed are ported, so
//!    the ladders are the same ladders and there is no statistical excuse:
//!    every band value is deterministic.
//!
//! # Results (2026-09-26)
//!
//! Printed per nuclide by the tests. Recorded in
//! `verification_and_validation/unr_block_write/unr_block_write_2026_09_26.md`.

use njoy_outram_park_fork::acer::jxs;
use njoy_outram_park_fork::acer::read;
use njoy_outram_park_fork::acer::unr::unr_words;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::purr::UrrProbabilityTables;
use njoy_outram_park_fork::reference_data::{ace_reference_file_or_skip, reference_endf};

const K_BOLTZMANN_MEV: f64 = 8.617_333_262e-11;

fn reference(name: &str) -> Option<read::RawAceTable> {
    let rel = format!("reference-njoy/endf-b-viii.0/293.6K/{name}.ace.gz");
    let p = ace_reference_file_or_skip(&rel, &format!("unr-write/{name}"))?;
    Some(read::read(&p).unwrap_or_else(|e| panic!("read {name}: {e}")))
}

/// NJOY's UNR block, word for word, with the integer typing its Type-1
/// writer uses: six integers, then reals (`acefc.f90:13450-13457`).
fn njoy_block(t: &read::RawAceTable) -> Vec<(f64, bool)> {
    let loc = t.jxs[jxs::LUNR];
    assert!(loc > 0, "reference table has no UNR block");
    let b = (loc - 1) as usize;
    let n = t.xss[b] as usize;
    let m = t.xss[b + 1] as usize;
    let len = 6 + n * (1 + 6 * m);
    t.xss[b..b + len]
        .iter()
        .enumerate()
        .map(|(i, &v)| (v, i < 6))
        .collect()
}

#[test]
fn reading_then_writing_reproduces_njoys_unr_block_word_for_word() {
    for name in ["U234", "U235", "U238"] {
        let Some(t) = reference(name) else { return };
        let kt_k = t.header.kt_mev / K_BOLTZMANN_MEV;
        let tables = UrrProbabilityTables::from_ace(&t, kt_k)
            .unwrap_or_else(|e| panic!("{name}: from_ace: {e}"))
            .unwrap_or_else(|| panic!("{name}: the reference table has a UNR block"));

        let want = njoy_block(&t);
        let got = unr_words(&tables);
        assert_eq!(
            got.len(),
            want.len(),
            "{name}: the written block has {} words, NJOY's has {}",
            got.len(),
            want.len()
        );

        let mut n_value = 0usize;
        let mut n_type = 0usize;
        let mut first_bad: Option<(usize, f64, f64)> = None;
        // Compared as a Type-1 file prints them (`1pE20.11`, 12 digits): since
        // 2026-09-26 the writer rounds with upstream's own `sigfig`, whose
        // x1.0000000000001 bias sits below that precision -- and the file, not
        // the in-memory double, is what is being reproduced.
        let printed = |x: f64| format!("{x:.11e}");
        for (i, ((gv, gi), (wv, wi))) in got.iter().zip(want.iter()).enumerate() {
            if printed(*gv) != printed(*wv) {
                n_value += 1;
                first_bad.get_or_insert((i, *gv, *wv));
            }
            if gi != wi {
                n_type += 1;
            }
        }
        println!(
            "{name}: {} words, header {:?}, value mismatches {n_value}, integer-typing \
             mismatches {n_type}",
            want.len(),
            &want[..6].iter().map(|w| w.0).collect::<Vec<_>>()
        );
        assert_eq!(
            (n_value, n_type),
            (0, 0),
            "{name}: first differing word {first_bad:?} (index, written, NJOY)"
        );
    }
}

#[test]
fn the_competition_flags_and_energy_grid_are_derived_exactly_as_njoy_derives_them() {
    // (name, tape, MAT)
    for (name, tape_file, mat) in [
        ("U234", "n-092_U_234-ENDF8.0.endf", 9225),
        ("U235", "n-092_U_235-ENDF8.0.endf", 9228),
        ("U238", "n-092_U_238.endf", 9237),
    ] {
        let Some(ref_table) = reference(name) else { return };
        let Some(path) = reference_endf(tape_file) else {
            println!("SKIP {name}: tape {tape_file} absent");
            return;
        };
        let loc = (ref_table.jxs[jxs::LUNR] - 1) as usize;
        let njoy = (ref_table.xss[loc + 3] as i32, ref_table.xss[loc + 4] as i32);

        let tape = Tape::read_file(&path).expect("parse tape");
        // The flags come from MF=3 thresholds, not from the ladders, so PURR's
        // statistics are set minimal (15 bins is PURR's own floor, `nbin should be
        // 15 or more`): this measures the rule, not the bands.
        let ours = UrrProbabilityTables::from_endf(&tape, mat, 293.6, 15, 1, 20)
            .unwrap_or_else(|e| panic!("{name}: PURR: {e}"))
            .unwrap_or_else(|| panic!("{name} has an unresolved range"));
        let got = (ours.inelastic_competition, ours.absorption_competition);
        println!("{name}: competition flags (ILF, IOA) ours {got:?}, NJOY {njoy:?}");
        let njoy_n = ref_table.xss[loc] as usize;
        let njoy_e: Vec<f64> = ref_table.xss[loc + 6..loc + 6 + njoy_n]
            .iter()
            .map(|e| e * 1.0e6)
            .collect();
        println!(
            "{name}: unresolved energy grid ours {} points [{:.4e} .. {:.4e}], NJOY {} points \
             [{:.4e} .. {:.4e}]",
            ours.len(),
            ours.energies().first().unwrap(),
            ours.energies().last().unwrap(),
            njoy_n,
            njoy_e.first().unwrap(),
            njoy_e.last().unwrap()
        );
        // The grid is NJOY's `rdunf2` node logic, so it must reproduce NJOY's
        // grid point for point AND at the 7th figure -- the endpoints are
        // shaded one unit in the 7th figure, which a 4-figure comparison
        // cannot see (and did not, before 2026-09-26).
        assert_eq!(
            ours.len(),
            njoy_n,
            "{name}: our unresolved grid has {} points, NJOY's {njoy_n}",
            ours.len()
        );
        let worst = ours
            .energies()
            .iter()
            .zip(&njoy_e)
            .map(|(a, b)| (a / b - 1.0).abs())
            .fold(0.0f64, f64::max);
        println!("   worst relative energy difference against NJOY: {worst:.2e}");
        assert!(
            worst < 1.0e-9,
            "{name}: grid differs from NJOY's by {worst:.2e} relative -- more than the \
             7-figure rounding both sides apply"
        );
        assert_eq!(
            got, njoy,
            "{name}: the ported purr.f90:1110-1192 rule disagrees with NJOY's own table"
        );
        assert_eq!(ours.interpolation, 2, "ACER always writes interpolation 2");
    }
}

/// **The pipeline, end to end**: `build_full_with_purr` on a real evaluation,
/// serialised to Type-1 **text** and parsed back by the production reader.
///
/// The two gates above check the writer against NJOY's words and the flag rule
/// against NJOY's flags. This one checks that the pieces are actually joined:
/// the block lands where NJOY puts it (immediately after DLW), survives a text
/// round trip with its integer typing intact, and decodes to the tables that
/// were generated — and that `build_full`, the deck without PURR, still writes
/// none. U-234 because it has the smallest unresolved range of the three
/// (26 energies); PURR's statistics are set low because this measures wiring,
/// not band shapes.
#[test]
fn build_full_with_purr_writes_a_block_that_reads_back_as_generated() {
    use njoy_outram_park_fork::acer::read::AceFileType;
    use njoy_outram_park_fork::acer::{build_full, build_full_with_purr};
    use njoy_outram_park_fork::broadr::broaden_result;
    use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};

    let Some(path) = reference_endf("n-092_U_234-ENDF8.0.endf") else {
        println!("SKIP: U-234 tape absent");
        return;
    };
    const MAT: i32 = 9225;
    const T_K: f64 = 293.6;
    let kt_mev = K_BOLTZMANN_MEV * T_K;
    let tape = Tape::read_file(&path).expect("parse tape");
    let recon0 = reconr(
        &tape,
        &ReconrConfig {
            mat: MAT,
            tolerance: 1.0e-3,
            temperature: 0.0,
        },
    )
    .expect("RECONR");
    let recon = broaden_result(&recon0, T_K);

    let (nbin, nladr, nsamp) = (15, 2, 100);
    let generated = UrrProbabilityTables::from_endf(&tape, MAT, T_K, nbin, nladr, nsamp)
        .expect("PURR")
        .expect("U-234 has an unresolved range");
    let with = build_full_with_purr(&tape, MAT, &recon, kt_mev, 0, nbin, nladr, nsamp)
        .expect("build with PURR");
    let without = build_full(&tape, MAT, &recon, kt_mev, 0).expect("build without PURR");

    // Through Type-1 TEXT and the production parser, so the integer typing of
    // the six header words is exercised, not just the in-memory values.
    let text = with
        .to_raw(AceFileType::Type1Ascii)
        .expect("to raw")
        .to_type1_string();
    let back = read::parse_type1(&text).expect("parse the written table");

    let lunr = back.jxs[jxs::LUNR];
    assert!(lunr > 0, "build_full_with_purr must set JXS(23)");
    // Immediately after DLW: NJOY's own ordering, DLW -> LUNR -> DNU -> BDD
    // -> DNEDL -> DNED -> GPD -> MTRP. The block after UNR is whichever of
    // those is present first. (~~Assumed MTRP~~: stale since the delayed
    // blocks landed between them, 2026-09-26.)
    let dlw_end = back
        .jxs
        .iter()
        .enumerate()
        .filter(|&(k, &l)| k != jxs::END && l > lunr)
        .map(|(_, &l)| l)
        .min()
        .unwrap_or(back.jxs[jxs::END] + 1);
    let len = 6 + generated.len() * (1 + 6 * generated.n_bands());
    assert_eq!(
        lunr as usize + len,
        dlw_end as usize,
        "the UNR block must end exactly where the next block (or the table) begins"
    );
    assert!(
        lunr > back.jxs[jxs::DLW],
        "the UNR block goes after DLW, as NJOY writes it"
    );

    let read_back = UrrProbabilityTables::from_ace(&back, T_K)
        .expect("decode UNR")
        .expect("the written table has a UNR block");
    assert_eq!(read_back.len(), generated.len());
    assert_eq!(read_back.n_bands(), generated.n_bands());
    assert_eq!(read_back.lssf, generated.lssf);
    assert_eq!(
        (read_back.inelastic_competition, read_back.absorption_competition),
        (generated.inelastic_competition, generated.absorption_competition)
    );
    // Energies go through eV -> MeV -> 7 figures -> eV, so they agree to the
    // 7th figure, not bit for bit.
    for (a, b) in read_back.energies().iter().zip(generated.energies()) {
        assert!(((a - b) / b).abs() < 1.0e-6, "energy {a} vs {b}");
    }
    // And the sampled physics agrees at every tabulated energy, to the same
    // 7-figure rounding the writer applies.
    //
    // Sampled at the READ-BACK grid's energies, not the generated one's: the
    // generated grid carries NJOY's `sigfig` bias (x 1.0000000000001, as
    // upstream's in-memory `eunr` does), so its last point sits 1e-13 above the
    // read-back table's rounded upper edge, where `sample` rightly says "outside
    // the tabulated range". The read-back energies lie inside both.
    let mut worst = 0.0f64;
    for &e in read_back.energies() {
        for xi in [0.05, 0.5, 0.95] {
            let (a, b) = match (read_back.sample(e, xi), generated.sample(e, xi)) {
                (Some(a), Some(b)) => (a, b),
                other => panic!("sample at {e} eV: {other:?}"),
            };
            let (va, vb) = match (a, b) {
                (
                    njoy_outram_park_fork::purr::UrrSample::SelfShieldingFactors(x),
                    njoy_outram_park_fork::purr::UrrSample::SelfShieldingFactors(y),
                )
                | (
                    njoy_outram_park_fork::purr::UrrSample::CrossSections(x),
                    njoy_outram_park_fork::purr::UrrSample::CrossSections(y),
                ) => (x, y),
                _ => panic!("LSSF changed across the round trip"),
            };
            for k in 0..4 {
                if vb[k] != 0.0 {
                    worst = worst.max(((va[k] - vb[k]) / vb[k]).abs());
                }
            }
        }
    }
    assert!(worst < 1.0e-6, "sampled bands differ by {worst:.2e} after the round trip");

    // The deck without PURR still writes none.
    let raw_without = without.to_raw(AceFileType::Type1Ascii).expect("to raw");
    assert_eq!(
        raw_without.jxs[jxs::LUNR],
        0,
        "build_full is the deck without PURR and must not grow a UNR block"
    );
    println!(
        "U-234 build_full_with_purr: UNR at JXS(23) = {lunr}, {} energies x {} bands, \
         flags {:?}, LSSF {}, worst sampled-band difference after the text round trip \
         {worst:.2e}; build_full JXS(23) = 0",
        generated.len(),
        generated.n_bands(),
        (generated.inelastic_competition, generated.absorption_competition),
        generated.lssf
    );
}

/// Gate 3: PURR's bands, generated from the ENDF evaluation, equal NJOY's.
///
/// **Result, 2026-09-26:** every word identical, U-234 (3 152 words), U-235
/// (2 305) and U-238 (10 049). Before this held, six divergences from
/// `unrest`/`ladr2`/`rdf3un` were found and fixed, recorded in
/// `verification_and_validation/ace_block_parity/`:
/// - the resonance window's inclusive lower sample (`fsrch`);
/// - a sample equal to a bin edge going to the bin above;
/// - MT=153's 7-figure text hand-off before ACER sums the probabilities;
/// - the temperature (the card's 293.6 K, not a kT read back from a header);
/// - the LSSF=1 competition remainder (`tol = 1e-6` and the `ecomp` test);
/// - the ladder's crossing resonance, sampled but not used (`nr=ir-1`).
#[test]
fn purr_generates_njoys_bands_word_for_word() {
    for (name, tape_file, mat) in [
        ("U234", "n-092_U_234-ENDF8.0.endf", 9225),
        ("U235", "n-092_U_235-ENDF8.0.endf", 9228),
        ("U238", "n-092_U_238.endf", 9237),
    ] {
        let Some(t) = reference(name) else { return };
        let Some(path) = reference_endf(tape_file) else {
            println!("SKIP: {tape_file} absent");
            return;
        };
        let tape = Tape::read_file(&path).expect("parse tape");
        let tables = UrrProbabilityTables::from_endf(&tape, mat, 293.6, 20, 64, 10_000)
            .unwrap_or_else(|e| panic!("{name}: PURR: {e}"))
            .unwrap_or_else(|| panic!("{name}: the evaluation has an unresolved range"));
        let want = njoy_block(&t);
        let got = unr_words(&tables);
        assert_eq!(got.len(), want.len(), "{name}: block length");
        let printed = |x: f64| format!("{x:.11e}");
        let bad: Vec<usize> = (0..want.len())
            .filter(|&i| printed(got[i].0) != printed(want[i].0) || got[i].1 != want[i].1)
            .collect();
        println!("{name}: {} of {} UNR words differ from NJOY2016's", bad.len(), want.len());
        assert!(
            bad.is_empty(),
            "{name}: first differing word {} (ours {:e}, NJOY {:e})",
            bad[0],
            got[bad[0]].0,
            want[bad[0]].0
        );
    }
}
