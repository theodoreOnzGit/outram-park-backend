// SPDX-License-Identifier: GPL-3.0

//! **The ACE photon-production blocks read back — GitHub #307 item 5.**
//!
//! # Methodology
//!
//! `acer::photon_blocks` writes MTRP/LSIGP/SIGP/LANDP/ANDP/LDLWP/DLWP; nothing
//! read them. `acer::photon_read::decode_photon_production` does, ported from
//! `openmc/data/reaction.py::_get_photon_products_ace` at OpenMC `afa7a14`.
//!
//! The oracle for every number below is **OpenMC's own Python reader** walking
//! the same bytes (`openmc.data.ace.get_table`, then the same seven `JXS`
//! blocks), so a disagreement is a disagreement between two readers rather than
//! a self-consistency check.
//!
//! U-235 comes from the committed `reference-data/ace` submodule, so the main
//! case runs wherever the submodule is initialised. Na-23, C-12 and H-2 are
//! regenerable tables gated on `OUTRAM_ACE_LAW_DIR` (the deck is in
//! `ace_dlw_law_family_vs_njoy2016.rs`); they are here because they cover what
//! U-235 does not — an `MFTYPE = 13` cross-section entry, a non-isotropic ANDP
//! entry, and a single-line table.
//!
//! # Results (2026-09-25)
//!
//! | table | `NTRP` | `MFTYPE` census | DLWP law census | LANDP = 0 |
//! |---|---|---|---|---|
//! | U-235 293.6 K | 583 | `{12: 577, 16: 6}` | `{2: 576, 4: 7}` | 583/583 |
//! | Na-23 0 K | 247 | `{12: 246, 13: 1}` | `{2: 246, 4: 1}` | 247/247 |
//! | C-12 0 K | 5 | `{12: 4, 16: 1}` | `{2: 4, 4: 1}` | **4**/5 |
//! | H-2 0 K | 1 | `{12: 1}` | `{2: 1}` | 1/1 |
//!
//! Every count matches OpenMC's. Two physical spot checks: U-235's first entry
//! is `MTRP = 18001` — fission's own photon subsection — carrying an MF=12 yield
//! on MT=18 and a LAW=4 continuum spectrum; and H-2's single entry is a
//! **discrete line at 6.251 MeV**, which is the `n + d -> t + γ` capture gamma.

use njoy_outram_park_fork::acer::ce_laws::AceEnergyLaw;
use njoy_outram_park_fork::acer::photon_read::{
    decode_photon_production, AcePhotonEntry, AcePhotonRate,
};
use njoy_outram_park_fork::acer::read;
use njoy_outram_park_fork::reference_data::ace_reference_file_or_skip;
use std::collections::BTreeMap;

const MEV: f64 = 1.0e6;

fn reference_u235() -> Option<Vec<AcePhotonEntry>> {
    let rel = "reference-njoy/endf-b-viii.0/293.6K/U235.ace.gz";
    let p = ace_reference_file_or_skip(rel, "ace-photon-read/U235")?;
    let t = read::read(&p).unwrap_or_else(|e| panic!("read U235: {e}"));
    Some(decode_photon_production(&t).unwrap_or_else(|e| panic!("U235 photon blocks: {e}")))
}

fn regenerated(name: &str) -> Option<Vec<AcePhotonEntry>> {
    let dir = match std::env::var("OUTRAM_ACE_LAW_DIR") {
        Ok(d) if !d.is_empty() => std::path::PathBuf::from(d),
        _ => {
            println!(
                "OUTRAM_ACE_LAW_DIR unset: skipping {name}. The deck is in \
                 ace_dlw_law_family_vs_njoy2016.rs's module doc."
            );
            return None;
        }
    };
    let t = read::read(dir.join(name).join("tape24"))
        .unwrap_or_else(|e| panic!("read {name}: {e}"));
    Some(decode_photon_production(&t).unwrap_or_else(|e| panic!("{name} photon blocks: {e}")))
}

fn census(entries: &[AcePhotonEntry]) -> (BTreeMap<i32, usize>, BTreeMap<String, usize>, usize) {
    let mut rate: BTreeMap<i32, usize> = BTreeMap::new();
    let mut laws: BTreeMap<String, usize> = BTreeMap::new();
    let mut isotropic = 0usize;
    for e in entries {
        let key = match &e.rate {
            AcePhotonRate::Yield { mftype, .. } => *mftype,
            AcePhotonRate::Xs { .. } => 13,
        };
        *rate.entry(key).or_insert(0) += 1;
        *laws.entry(e.law.code()).or_insert(0) += 1;
        if e.angular.is_none() {
            isotropic += 1;
        }
    }
    (rate, laws, isotropic)
}

#[test]
fn u235s_583_photon_subsections_decode_with_openmcs_census() {
    let Some(entries) = reference_u235() else { return };
    assert_eq!(entries.len(), 583, "OpenMC reads NXS(7) = 583 on this table");
    let (rate, laws, isotropic) = census(&entries);
    assert_eq!(rate, BTreeMap::from([(12, 577), (16, 6)]));
    assert_eq!(
        laws,
        BTreeMap::from([("2".to_string(), 576), ("4".to_string(), 7)])
    );
    assert_eq!(isotropic, 583, "every U-235 photon entry has LANDP = 0");

    // An entry is one SUBSECTION, not one reaction: 583 entries over 81 MTs.
    let mts: std::collections::BTreeSet<i32> = entries.iter().map(|e| e.neutron_mt).collect();
    assert_eq!(mts.len(), 81);

    // The first entry is fission's own photon subsection.
    let first = &entries[0];
    assert_eq!(first.mtrp, 18_001);
    assert_eq!((first.neutron_mt, first.subsection), (18, 1));
    let AcePhotonRate::Yield { mftype, mtmult, energy, y } = &first.rate else {
        panic!("MT=18's photon rate is an MF=12 yield, got {:?}", first.rate)
    };
    assert_eq!((*mftype, *mtmult), (12, 18));
    assert!(
        energy.windows(2).all(|w| w[1] >= w[0]),
        "the yield's incident grid must be ascending"
    );
    assert!(y.iter().all(|v| *v >= 0.0), "a photon yield cannot be negative");
    assert!(
        matches!(first.law, AceEnergyLaw::Tabulated { law: 4, .. }),
        "fission photons are a continuum spectrum (LAW=4), got {}",
        first.law.code()
    );
    // 576 of the 583 are discrete lines; check one has a physical energy.
    let lines: Vec<f64> = entries
        .iter()
        .filter_map(|e| match e.law {
            AceEnergyLaw::DiscretePhoton { eg, .. } => Some(eg),
            _ => None,
        })
        .collect();
    assert_eq!(lines.len(), 576);
    assert!(
        lines.iter().all(|&eg| eg > 0.0 && eg < 20.0 * MEV),
        "a discrete gamma line outside (0, 20) MeV means the MeV scaling is wrong"
    );
    println!(
        "U-235 293.6 K: {} photon entries over {} MTs, rate {rate:?}, laws {laws:?}, \
         {} discrete lines from {:.4} to {:.4} MeV",
        entries.len(),
        mts.len(),
        lines.len(),
        lines.iter().cloned().fold(f64::MAX, f64::min) / MEV,
        lines.iter().cloned().fold(0.0, f64::max) / MEV
    );
}

#[test]
fn na23_carries_the_mftype_13_form_and_c12_a_non_isotropic_entry() {
    let Some(na23) = regenerated("Na23") else { return };
    let (rate, laws, isotropic) = census(&na23);
    assert_eq!(na23.len(), 247);
    assert_eq!(rate, BTreeMap::from([(12, 246), (13, 1)]));
    assert_eq!(
        laws,
        BTreeMap::from([("2".to_string(), 246), ("4".to_string(), 1)])
    );
    assert_eq!(isotropic, 247);

    // The one MFTYPE = 13 entry: a production CROSS SECTION on the table's own
    // grid, not a yield. Reading it as a yield would understate production by
    // the size of the cross section and look like a small number.
    let xs_entry = na23
        .iter()
        .find(|e| matches!(e.rate, AcePhotonRate::Xs { .. }))
        .expect("Na-23 has exactly one MFTYPE = 13 entry");
    let AcePhotonRate::Xs { threshold_index, xs } = &xs_entry.rate else {
        unreachable!()
    };
    assert!(!xs.is_empty());
    assert!(xs.iter().all(|v| *v >= 0.0), "a cross section cannot be negative");
    println!(
        "Na-23: MFTYPE=13 entry MTRP={} from grid index {} with {} points, max {:.4e} b",
        xs_entry.mtrp,
        threshold_index,
        xs.len(),
        xs.iter().cloned().fold(0.0, f64::max)
    );

    let Some(c12) = regenerated("C12") else { return };
    let (rate, _, isotropic) = census(&c12);
    assert_eq!(c12.len(), 5);
    assert_eq!(rate, BTreeMap::from([(12, 4), (16, 1)]));
    assert_eq!(
        isotropic, 4,
        "C-12 has one entry with a real ANDP distribution, which is what makes \
         the LANDP/ANDP path exercised at all"
    );
    let anisotropic = c12
        .iter()
        .find(|e| e.angular.is_some())
        .expect("one C-12 entry is anisotropic");
    let ang = anisotropic.angular.as_ref().unwrap();
    assert_eq!(ang.lct, 1, "a photon distribution is laboratory-frame");
    assert!(!ang.energies.is_empty());
    println!(
        "C-12: MTRP={} carries an ANDP distribution at {} incident energies",
        anisotropic.mtrp,
        ang.energies.len()
    );
}

#[test]
fn h2s_single_entry_is_the_6_25_mev_capture_line() {
    let Some(h2) = regenerated("H2") else { return };
    assert_eq!(h2.len(), 1);
    let e = &h2[0];
    assert_eq!(e.mtrp, 102_001, "H-2's only photon source is capture");
    assert_eq!(e.neutron_mt, 102);
    let AceEnergyLaw::DiscretePhoton { lp, eg } = e.law else {
        panic!("expected a discrete line, got law {}", e.law.code())
    };
    // n + d -> t + gamma releases 6.257 MeV; the table stores 6.251.
    assert!(
        (eg - 6.251 * MEV).abs() < 1.0e3,
        "the capture line is at {eg:.6e} eV, not 6.251 MeV — a MeV/eV slip would \
         be a factor of a million and a wrong LP would read the energy as a flag"
    );
    assert!(matches!(lp, 0 | 1 | 2), "LP = {lp} is not a legal primary flag");
    println!("H-2: one entry, MTRP=102001, discrete line LP={lp} at {:.4} MeV", eg / MEV);
}
