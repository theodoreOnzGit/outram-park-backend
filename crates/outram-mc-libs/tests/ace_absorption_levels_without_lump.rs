// SPDX-License-Identifier: GPL-3.0

//! **Absorption levels (MT=600..849) count even when the table has no lump**
//! — GitHub #365 audit.
//!
//! # What was wrong
//!
//! The pointwise tier sums (n,p)...(n,alpha) absorption through the lumps
//! MT=103..107 only. A legal ACE table carrying the levels but not the lump
//! (NJOY writes the lumps; other processors need not) lost that absorption
//! into the elastic remainder. OpenMC builds the lump from its components
//! when it is missing (`openmc/data/neutron.py:617-630`). `Nuclide::from_ace`
//! now does the same.
//!
//! # Methodology
//!
//! ENDF/B-VIII.0 carries levels without the lump in 202 cases (Mn-55, Fe-54/56
//! and others; scanned 2026-09-29 over the 557 neutron evaluations). NJOY adds
//! the lump, so no held ACE table has the form. The test **constructs** one:
//! NJOY2016's Mn-55 table (`target/five_route_keff/njoy/293.6K/Mn55.ace`,
//! skipped when absent) with MT=103 and MT=107 relabelled to MT=150/151,
//! unassigned MTs nothing reads. That leaves 34 (n,p) and 32 (n,alpha)
//! levels with no lump.
//!
//! Absorption at 12 energies from 1 MeV to 20 MeV must equal the unmodified
//! table's to **1e-6 relative**. The lump and the sum of its levels are
//! rounded to 7 figures independently by ACER, so they agree to that and no
//! better. Relabelling alone, without the fix, loses up to the whole (n,p) +
//! (n,alpha) share; the test asserts that share is visible (> 1e-3 relative
//! somewhere), so the check can fail.
//!
//! # Results (2026-09-29)
//!
//! Printed by the test.

use outram_mc_libs::material::nuclide::Nuclide;
use std::path::PathBuf;

#[test]
fn absorption_levels_count_without_their_lump() {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/five_route_keff/njoy/293.6K/Mn55.ace");
    if !p.is_file() {
        println!("{} absent: skipping", p.display());
        return;
    }
    let raw = njoy_outram_park_fork::acer::read::read(&p).expect("read");
    let orig = Nuclide::from_ace(&raw, "Mn55").expect("from_ace");
    let mut t = raw.clone();
    let mtr = (t.jxs[njoy_outram_park_fork::acer::jxs::MTR] - 1) as usize;
    let nmt = t.nxs[njoy_outram_park_fork::acer::nxs::NTR] as usize;
    let mut relabelled = 0;
    for k in 0..nmt {
        match t.xss[mtr + k] as i32 {
            103 => {
                t.xss[mtr + k] = 150.0;
                relabelled += 1;
            }
            107 => {
                t.xss[mtr + k] = 151.0;
                relabelled += 1;
            }
            _ => {}
        }
    }
    assert_eq!(relabelled, 2, "Mn-55 carries MT=103 and MT=107");
    let lumpless = Nuclide::from_ace(&t, "Mn55").expect("from_ace, lumps removed");
    let (mut worst, mut share) = (0.0f64, 0.0f64);
    for k in 0..12 {
        let e = 1.0e6 * (20.0f64).powf(k as f64 / 11.0);
        let a = orig.xs_at_energy(e, 293.6).absorption;
        let b = lumpless.xs_at_energy(e, 293.6).absorption;
        worst = worst.max(((b - a) / a).abs());
        println!(
            "E = {:.2} MeV: absorption {a:.6e} (with lumps) vs {b:.6e} (levels only)",
            e / 1e6
        );
        // How much of the absorption the (n,p)+(n,alpha) lumps carry here.
        share = share.max(1.0 - orig_capture_fraction(&orig, e));
    }
    println!("worst relative difference {worst:.2e}; largest (n,p)+(n,alpha) share {share:.3}");
    assert!(
        worst < 1.0e-6,
        "levels-only absorption differs by {worst:.2e}"
    );
    assert!(
        share > 1.0e-3,
        "the lumps must matter somewhere, or the test cannot fail"
    );
}

/// Capture (MT=102) as a fraction of absorption.
fn orig_capture_fraction(n: &Nuclide, e: f64) -> f64 {
    let x = n.xs_at_energy(e, 293.6);
    let cap = n.reaction_xs(102, e).unwrap_or(0.0);
    if x.absorption > 0.0 {
        cap / x.absorption
    } else {
        1.0
    }
}
