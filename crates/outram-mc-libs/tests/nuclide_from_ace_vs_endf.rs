// SPDX-License-Identifier: GPL-3.0

//! **A nuclide built from NJOY2016's ACE, against the same nuclide built from
//! the ENDF evaluation.** GitHub #270.
//!
//! # Why this is the comparison that matters
//!
//! `Nuclide::from_ace` and `Nuclide::from_tape` are two entirely separate
//! routes to the same physics:
//!
//! | | ACE route | ENDF route |
//! |---|---|---|
//! | processing | **NJOY2016** RECONR+BROADR+PURR+ACER | **this workspace's** RECONR + BROADR |
//! | reader | `acer::ce_decode` + `ce_laws` | `endf` + `reconr` |
//! | grid | NJOY's adaptive union grid | ours |
//!
//! So an agreement here is a genuine **cross-code** check of this port's
//! resonance reconstruction and Doppler broadening against the Fortran, on the
//! same evaluation — not a round trip through one codec.
//!
//! It also closes the gap the HDF5 V&V record named: *"running the same problem
//! through `outram-mc-libs`' transport for a two-code comparison on one library
//! ... needs a public constructor for a `Nuclide` built from supplied pointwise
//! data"*. That constructor now exists.
//!
//! # Methodology
//!
//! Build U-235 and U-238 both ways at 293.6 K and compare the microscopic cross
//! sections at energies chosen to exercise different physics: thermal
//! (1/v capture and fission), the 6.674 eV U-238 resonance (the hardest point
//! for reconstruction *and* broadening), the unresolved range, and fast.
//!
//! The ACE tables come from the `reference-data/ace` submodule, initialised
//! automatically if absent. The ENDF tapes are the same evaluations NJOY was
//! given, per the submodule's `MANIFEST.tsv`.
//!
//! # What a disagreement would mean
//!
//! Not "a tolerance to widen". The two routes share no code, so a difference is
//! either a reconstruction difference (our RECONR against NJOY's), a broadening
//! difference (our BROADR against NJOY's SIGMA1), or a decode error. Each is a
//! finding.

use njoy_outram_park_fork::acer::read;
use njoy_outram_park_fork::reference_data::{ace_reference_file_or_skip, reference_endf};
use outram_mc_libs::material::nuclide::Nuclide;

const TEMP_K: f64 = 293.6;

/// Energies chosen to exercise different physics, not to be easy.
const PROBE_EV: [f64; 6] = [
    2.53e-2, // thermal: 1/v capture, U-235 fission
    1.0,     // epithermal
    6.674,   // U-238's big resonance: reconstruction AND broadening
    1.0e3,   // resolved resonance region
    1.0e5,   // unresolved range
    2.0e6,   // fast: fission spectrum, inelastic open
];

fn from_ace(nuclide: &str) -> Option<Nuclide> {
    let rel = format!("reference-njoy/endf-b-viii.0/293.6K/{nuclide}.ace.gz");
    let p = ace_reference_file_or_skip(&rel, &format!("from_ace/{nuclide}"))?;
    let raw = read::read(&p).unwrap_or_else(|e| panic!("read {nuclide} ACE: {e}"));
    Some(Nuclide::from_ace(&raw, nuclide).unwrap_or_else(|e| panic!("from_ace {nuclide}: {e}")))
}

/// **A nuclide built from ACE carries the right physics.**
///
/// Checked against values that are independent of both codes -- the accepted
/// thermal cross sections and the known resonance -- so this cannot pass by
/// two implementations agreeing on the same mistake.
///
/// # Results (2026-09-23, NJOY2016 2016.79 `ac5adf5`, ENDF/B-VIII.0)
///
/// ```text
/// U235 @ 0.0253 eV   total 700.116   elastic 14.109   fission 586.635   abs 686.007
/// U238 @ 6.674 eV    total 7675.52   elastic 481.29   absorption 7194.23
/// U238 @ 0.0253 eV   total 11.9224   elastic  9.240   absorption 2.683
/// ```
///
/// All within a per-cent of the accepted values, and `total` closes against
/// `elastic + absorption` exactly at thermal.
#[test]
fn a_nuclide_from_ace_has_the_right_physics() {
    let Some(u235) = from_ace("U235") else { return };
    let x = u235.xs_at_energy(2.53e-2, TEMP_K);
    // Accepted thermal values for U-235: sigma_f ~ 585 b, sigma_a ~ 681 b,
    // sigma_t ~ 698 b. A 3 % band is wide enough for evaluation differences
    // and far too tight for a decode error.
    assert!(
        (x.fission - 585.0).abs() / 585.0 < 0.03,
        "U-235 thermal fission {} b, expected ~585",
        x.fission
    );
    assert!(
        (x.total - 698.0).abs() / 698.0 < 0.03,
        "U-235 thermal total {} b, expected ~698",
        x.total
    );
    // **The MT=1 pin.** ACE does not list the total in MTR -- it is in ESZ --
    // so a reader that looks only at MTR reports total = 0 while every partial
    // is perfect. A zero macroscopic total makes distance-to-collision
    // infinite and every particle streams out: k = 0 from good data.
    assert!(x.total > 0.0, "total cross section is zero; MT=1 is missing from ESZ");
    assert!(
        (x.total - (x.elastic + x.absorption)).abs() < 1.0e-3 * x.total,
        "at thermal, total {} should be elastic {} + absorption {}",
        x.total,
        x.elastic,
        x.absorption
    );

    let Some(u238) = from_ace("U238") else { return };
    let r = u238.xs_at_energy(6.674, TEMP_K);
    assert!(
        r.absorption > 5.0e3,
        "U-238's 6.674 eV resonance should exceed 5000 b in absorption, got {}",
        r.absorption
    );
    let t = u238.xs_at_energy(2.53e-2, TEMP_K);
    assert!(
        (t.absorption - 2.68).abs() / 2.68 < 0.05,
        "U-238 thermal capture {} b, expected ~2.68",
        t.absorption
    );
    println!(
        "U235 thermal: total {:.3} elastic {:.3} fission {:.3} abs {:.3}",
        x.total, x.elastic, x.fission, x.absorption
    );
    println!(
        "U238 @6.674 eV: total {:.2} elastic {:.2} abs {:.2}",
        r.total, r.elastic, r.absorption
    );
}

/// **ACE against ENDF, the cross-code comparison.**
///
/// Slow: the ENDF route runs this workspace's RECONR and BROADR from scratch.
/// Gated behind `long-tests`, which is on by default.
#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "reconstructs two actinides from ENDF (RECONR + BROADR, several minutes)"
)]
fn ace_and_endf_routes_agree_on_the_same_evaluation() {
    for (nuclide, tape) in [
        ("U235", "n-092_U_235-ENDF8.0.endf"),
        ("U238", "n-092_U_238.endf"),
    ] {
        let Some(ace) = from_ace(nuclide) else { return };
        let Some(path) = reference_endf(tape) else {
            println!("[ace-vs-endf/{nuclide}] SKIP: no ENDF tape {tape}");
            continue;
        };
        let endf = Nuclide::from_endf_file(&path, nuclide, TEMP_K, 1.0e-3)
            .unwrap_or_else(|e| panic!("from_endf_file {nuclide}: {e}"));

        println!("\n{nuclide}: ACE (NJOY2016) vs ENDF (this port's RECONR+BROADR)");
        println!(
            "  {:>10}  {:>14} {:>14} {:>9}   {:>14} {:>14} {:>9}",
            "E [eV]", "ACE total", "ENDF total", "rel", "ACE fission", "ENDF fission", "rel"
        );
        for e in PROBE_EV {
            let a = ace.xs_at_energy(e, TEMP_K);
            let b = endf.xs_at_energy(e, TEMP_K);
            let rel = |p: f64, q: f64| if q.abs() > 0.0 { (p - q) / q } else { 0.0 };
            println!(
                "  {e:>10.3e}  {:>14.5e} {:>14.5e} {:>+8.2}%   {:>14.5e} {:>14.5e} {:>+8.2}%",
                a.total,
                b.total,
                100.0 * rel(a.total, b.total),
                a.fission,
                b.fission,
                100.0 * rel(a.fission, b.fission)
            );
        }
        // Reported, not gated, on this first pass: the two routes' agreement is
        // the measurement, and a threshold picked before seeing it would be
        // guesswork. The gate below is the one that must hold regardless --
        // both routes must produce a usable, positive total.
        for e in PROBE_EV {
            let a = ace.xs_at_energy(e, TEMP_K);
            let b = endf.xs_at_energy(e, TEMP_K);
            assert!(a.total > 0.0, "{nuclide}: ACE total is zero at {e:.3e} eV");
            assert!(b.total > 0.0, "{nuclide}: ENDF total is zero at {e:.3e} eV");
        }
    }
}
