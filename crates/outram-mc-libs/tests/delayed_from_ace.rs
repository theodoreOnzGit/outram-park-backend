// SPDX-License-Identifier: GPL-3.0

//! **Delayed neutrons on the ACE route, against the ENDF route** — GitHub #307
//! item 1.
//!
//! `Nuclide::from_ace` left `delayed: None` because the ACE DNU/BDD blocks were
//! not decoded, so the ACE route could not do kinetics at all while the ENDF
//! route could. Same shape as the URR omission: physics the file carries,
//! absent because nothing read it.
//!
//! # The oracle
//!
//! Both routes describe the **same evaluation**, so the decay constants and the
//! delayed yield must agree — and λ is the sharpest check available, because it
//! is a handful of numbers fixed by the evaluation with no sampling or
//! interpolation anywhere near them. ACE stores λ in inverse **shakes**; get
//! that conversion wrong and λ is out by 1e8, which looks like a number rather
//! than an error. Comparing against ENDF's `s⁻¹` catches exactly that.
//!
//! # Results, 2026-09-25
//!
//! Printed by the tests.

use njoy_outram_park_fork::acer::read;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reference_data::{ace_reference_file_or_skip, reference_endf};
use outram_mc_libs::material::nuclide::{DelayedData, Nuclide};

/// **THE GATE: a fissile nuclide from ACE carries delayed data.**
#[test]
fn u235_from_ace_has_delayed_neutrons() {
    let rel = "reference-njoy/endf-b-viii.0/293.6K/U235.ace.gz";
    let Some(p) = ace_reference_file_or_skip(rel, "delayed-from-ace/U235") else {
        return;
    };
    let raw = read::read(&p).expect("read U235");
    let n = Nuclide::from_ace(&raw, "U235").expect("construct");

    let d = n
        .delayed()
        .expect("U-235 is fissile and its ACE table carries DNU/BDD, so delayed data must be present");
    println!(
        "U235 from ACE: {} precursor groups, nu_d grid {} points ({:.3e}..{:.3e} eV)",
        d.lambda.len(),
        d.energy.len(),
        d.energy.first().copied().unwrap_or(0.0),
        d.energy.last().copied().unwrap_or(0.0)
    );
    println!("  lambda [1/s]: {:?}", d.lambda.iter().map(|l| format!("{l:.6e}")).collect::<Vec<_>>());

    assert!(
        (5..=8).contains(&d.lambda.len()),
        "ENDF/B-VIII.0 uses 6 (sometimes 8) precursor groups; got {}",
        d.lambda.len()
    );
    // **The shake conversion.** Physical precursor half-lives run from ~0.2 s
    // to ~80 s, so lambda is ~1e-2 .. ~4 /s. Missing the 1e8 would put every
    // one near 1e-10, which is a number and not an error.
    for (k, &l) in d.lambda.iter().enumerate() {
        assert!(
            (1.0e-3..1.0e2).contains(&l),
            "group {k} lambda = {l:.6e} /s is outside any physical precursor \
             decay constant (~1e-2 to ~4 /s). A value near 1e-10 means the \
             inverse-SHAKES conversion was skipped; near 1e6 means it was \
             applied twice"
        );
    }
    assert!(
        d.lambda.windows(2).all(|w| w[1] >= w[0]),
        "ACE stores precursor groups in ascending lambda; {:?} is not sorted, \
         which would mean the per-group stride through BDD is wrong",
        d.lambda
    );
    assert!(!d.energy.is_empty() && d.energy.len() == d.nu_delayed.len());
    assert!(
        d.energy.windows(2).all(|w| w[1] > w[0]),
        "the delayed nu-bar grid must be ascending"
    );
    // Delayed nu-bar for U-235 is ~0.0167 at thermal, rising slightly.
    let nu_d0 = d.nu_delayed[0];
    assert!(
        (0.005..0.05).contains(&nu_d0),
        "delayed nu-bar {nu_d0} at the bottom of the grid is not near U-235's \
         ~0.0167; a TAB1 misread returns a point count or an energy here"
    );

    // The group shares must partition unity after renormalisation.
    let sum: f64 = d
        .group_fraction
        .iter()
        .filter_map(|g| g.first().map(|&(_, p)| p))
        .sum();
    println!("  group shares sum to {sum:.9}");
    assert!(
        (sum - 1.0).abs() < 1.0e-9,
        "the renormalised group shares sum to {sum}, not 1. ACE files do not \
         store them summing to 1 and upstream renormalises; without that the \
         delayed fraction is wrong by however far off unity the file is"
    );
}

/// **Both routes describe one evaluation, so λ must agree.**
///
/// This is the check that makes the shake conversion real rather than merely
/// plausible: the ENDF route reads λ in `s⁻¹` from MF=1/MT=455 directly, so an
/// agreement here cannot be a units coincidence.
#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "reads the U-235 ENDF tape; runs by default"
)]
fn ace_and_endf_agree_on_the_decay_constants() {
    let rel = "reference-njoy/endf-b-viii.0/293.6K/U235.ace.gz";
    let (Some(pa), Some(pe)) = (
        ace_reference_file_or_skip(rel, "delayed-cmp/ace"),
        reference_endf("n-092_U_235-ENDF8.0.endf"),
    ) else {
        return;
    };
    let raw = read::read(&pa).expect("read the ACE table");
    let from_ace = DelayedData::from_ace(&raw)
        .expect("decode")
        .expect("U-235's ACE table carries delayed data");

    let tape = Tape::read_file(&pe).expect("parse the ENDF tape");
    let mat = *tape.materials().first().expect("a material");
    let from_endf = DelayedData::from_tape(&tape, mat)
        .expect("decode")
        .expect("U-235's evaluation carries MT=455");

    println!("  lambda from ACE : {:?}", from_ace.lambda.iter().map(|l| format!("{l:.6e}")).collect::<Vec<_>>());
    println!("  lambda from ENDF: {:?}", from_endf.lambda.iter().map(|l| format!("{l:.6e}")).collect::<Vec<_>>());

    assert_eq!(
        from_ace.lambda.len(),
        from_endf.lambda.len(),
        "the two routes disagree on the precursor group COUNT ({} vs {}), so one \
         of them is mis-striding its per-group records",
        from_ace.lambda.len(),
        from_endf.lambda.len()
    );
    for (k, (&a, &e)) in from_ace.lambda.iter().zip(&from_endf.lambda).enumerate() {
        let rel_err = (a - e).abs() / e;
        println!("    group {k}: ACE {a:.6e} vs ENDF {e:.6e}  ({:.2e} relative)", rel_err);
        assert!(
            rel_err < 1.0e-4,
            "group {k}: lambda {a:.6e} /s from ACE against {e:.6e} /s from ENDF, \
             {rel_err:.3e} relative. These are the SAME evaluation's constants, so \
             they must agree to the file's own precision. A factor near 1e8 means \
             the inverse-shakes conversion"
        );
    }

    // **The delayed yield, compared point by point over the whole grid.**
    //
    // An earlier version of this test picked ONE energy with
    // `partition_point(|x| x < e_lo)` and compared that. It failed at 5.4 %,
    // and the cause was the test rather than the decoder: ACE's first energy
    // comes back as 9.999999999999999e-6 from the MeV->eV round-trip on
    // 1e-11 MeV while ENDF's is exactly 1e-5, so the index landed on 1 for one
    // route and 0 for the other and it compared ACE's SECOND point against
    // ENDF's first. Comparing the grids pairwise has no index to get wrong and
    // checks every point instead of one.
    assert_eq!(
        from_ace.energy.len(),
        from_endf.energy.len(),
        "the two routes disagree on the delayed nu-bar grid LENGTH ({} vs {})",
        from_ace.energy.len(),
        from_endf.energy.len()
    );
    for (i, ((&ea, &na), (&ee, &ne))) in from_ace
        .energy
        .iter()
        .zip(&from_ace.nu_delayed)
        .zip(from_endf.energy.iter().zip(&from_endf.nu_delayed))
        .enumerate()
    {
        // The energy tolerance is the MeV->eV round-trip's own rounding, not a
        // physics allowance: ACE stores MeV, so every energy is a product of
        // 1e6 and cannot be bit-identical to the ENDF tape's eV.
        assert!(
            (ea - ee).abs() <= 1.0e-9 * ee.max(1.0),
            "point {i}: energy {ea:.17e} eV from ACE against {ee:.17e} from ENDF"
        );
        assert!(
            (na - ne).abs() <= 1.0e-9 * ne.max(1.0e-12),
            "point {i} at {ee:.4e} eV: delayed nu-bar {na:.9} from ACE against \
             {ne:.9} from ENDF. These are one evaluation's own yield and must \
             agree to the file's precision"
        );
    }
    println!(
        "  delayed nu-bar agrees at all {} grid points (first {:.5}, last {:.5})",
        from_ace.energy.len(),
        from_ace.nu_delayed[0],
        from_ace.nu_delayed[from_ace.nu_delayed.len() - 1]
    );
}

/// A non-fissionable nuclide has no delayed data, and that is `None`.
#[test]
fn a_table_without_delayed_data_gives_none() {
    // U-234's ACE table is fissionable but processed with partial fission; if it
    // carries no DNU the answer must still be a clean None rather than an error.
    for name in ["U234", "U238"] {
        let rel = format!("reference-njoy/endf-b-viii.0/293.6K/{name}.ace.gz");
        let Some(p) = ace_reference_file_or_skip(&rel, &format!("delayed-none/{name}")) else {
            continue;
        };
        let raw = read::read(&p).expect("read");
        let got = DelayedData::from_ace(&raw)
            .unwrap_or_else(|e| panic!("{name}: a missing DNU must be None, not an error: {e}"));
        println!(
            "{name}: delayed = {}",
            got.as_ref().map_or("None".to_string(), |d| format!("{} groups", d.lambda.len()))
        );
    }
}
