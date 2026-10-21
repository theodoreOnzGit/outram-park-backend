// SPDX-License-Identifier: GPL-3.0

//! **`CeNeutronAce::reconstructed_total` does not double count the (n,d),
//! (n,t), (n,He-3) and (n,2n) level components** — GitHub #365 audit.
//!
//! # What was wrong
//!
//! `channel_mts` dropped a lump's components for MT=4, 103, 107 and 18, but
//! passed MT=650–799 through beside MT=104–106, and MT=875–891 beside MT=16.
//! OpenMC's `SUM_RULES` names all of them. A table carrying both a lump and
//! its levels, as NJOY writes O-16 and Al-27 (MT=104 with 20 and 21 levels,
//! MT=105 with 11 and 12), then summed those channels twice.
//!
//! # Methodology
//!
//! On every light-nuclide table of the five-route study
//! (`target/five_route_keff/njoy/293.6K/`, NJOY2016, skipped when absent),
//! `reconstructed_total()` must equal ESZ's total at every grid point to
//! **1e-6 relative**. That is ACER's 7-significant-figure rounding: the total
//! and each partial are written rounded independently. It is a check of the
//! partition, not of physics. The largest relative miss is printed.
//!
//! # Results (2026-09-29)
//!
//! Printed per table. With the fix reverted, O-16 and Al-27 fail.

use njoy_outram_park_fork::acer::ce_decode::decode_ce;
use njoy_outram_park_fork::acer::read::read;
use std::path::PathBuf;

#[test]
fn light_nuclide_totals_reconstruct_without_double_counting() {
    let dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/five_route_keff/njoy/293.6K");
    if !dir.is_dir() {
        println!("{} absent: skipping", dir.display());
        return;
    }
    for name in ["H1", "O16", "F19", "Al27", "Si28", "Si29", "Si30", "Mn55"] {
        let p = dir.join(format!("{name}.ace"));
        if !p.is_file() {
            continue;
        }
        let ace = decode_ce(&read(&p).expect("read")).expect("decode");
        let rec = ace.reconstructed_total();
        let worst = rec
            .iter()
            .zip(&ace.total)
            .filter(|(_, t)| **t > 0.0)
            .map(|(r, t)| ((r - t) / t).abs())
            .fold(0.0f64, f64::max);
        println!("{name}: worst relative |reconstructed - total| = {worst:.2e}");
        assert!(
            worst < 1.0e-6,
            "{name}: reconstructed total misses ESZ by {worst:.2e}"
        );
    }
}
