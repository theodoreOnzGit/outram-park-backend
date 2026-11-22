// SPDX-License-Identifier: AGPL-3.0-only

//! **The reader unpacks the tabulated laws, URR tables and the second
//! temperature of a real OpenMC library** (GitHub #303).
//!
//! The file is the 11-nuclide LCT-008 library built for gh:#550 (NJOY2016 ACER
//! at 0 K and 293.6 K, then `IncidentNeutron.from_njoy`), generated and not
//! committed, so the test skips with a printed reason unless
//! `OUTRAM_OPENMC_LCT008_H5` names its directory.
//!
//! # Methodology
//!
//! The expected shapes are read off the files with h5py (OpenMC 0.15.3
//! container, 2026-11-18), not off the Rust: U-235 has `correlated` on MT 5,
//! 16, 17 and 91, a `continuous` prompt fission spectrum on MT 18 and six
//! `delayed` groups beside it; U-234 has `kalbach-mann` on MT 16, 17, 37 and 91
//! and four partial fissions; U-238 carries an 83-energy, 20-band URR table at
//! `294K`; every file has `0K` and `294K`, and the `0K` energy grid is the
//! longer one (U-235: 233 515 against 76 027 points).
//!
//! # Results (2026-11-18)
//!
//! All pass on that library: the decoded counts below equal the h5py census,
//! and the cosine/energy table shapes are consistent (`validate`). Their
//! bit-level consequence, through `Nuclide::from_hdf5`, is recorded in
//! `outram-mc-libs/verification_and_validation/hdf5_library_303/`.

use njoy_outram_park_fork::hdf5::nuclide_read::{
    read_nuclide, read_nuclide_selected, ReadDistribution,
};

fn dir() -> Option<std::path::PathBuf> {
    let d = std::env::var_os("OUTRAM_OPENMC_LCT008_H5");
    if d.is_none() {
        println!("SKIP: set OUTRAM_OPENMC_LCT008_H5 to the LCT-008 library directory");
    }
    d.map(Into::into)
}

#[test]
fn u235_laws_urr_and_both_temperatures_are_read() {
    let Some(d) = dir() else { return };
    let r = read_nuclide_selected(d.join("U235.h5"), Some("294K"), &["0K"]).expect("read");
    assert_eq!(r.primary_temperature, "294K");
    // cross sections are the 294K ones (76027 points), not the alphabetically
    // first label's
    assert_eq!(r.any_energy().unwrap().len(), 76_027);
    let el = &r.reactions[&2];
    let (xs0, thr0) = &el.other_temperature_xs["0K"];
    assert_eq!(thr0 + xs0.len(), r.energy["0K"].len());
    assert_eq!(r.energy["0K"].len(), 233_515);

    for mt in [5, 16, 17, 91] {
        let p = &r.reactions[&mt].products[0];
        assert!(
            matches!(p.distributions[0], ReadDistribution::Correlated(_)),
            "MT={mt}: {:?}",
            p.distributions[0]
        );
        let ReadDistribution::Correlated(t) = &p.distributions[0] else { unreachable!() };
        t.validate().expect("consistent table");
        assert!(t.rows.iter().all(|row| row.cosines.len() == row.e_out.len()));
    }
    let fission = &r.reactions[&18];
    let prompt: Vec<_> = fission.products.iter().filter(|p| p.emission_mode == "prompt").collect();
    let delayed: Vec<_> = fission.products.iter().filter(|p| p.emission_mode == "delayed").collect();
    assert_eq!(delayed.len(), 6);
    assert!(prompt.iter().any(|p| p.particle == "neutron"));
    let n = prompt.iter().find(|p| p.particle == "neutron").unwrap();
    assert!(matches!(
        &n.distributions[0],
        ReadDistribution::Uncorrelated {
            energy: Some(njoy_outram_park_fork::hdf5::nuclide_laws::EnergyDist::Continuous(_)),
            ..
        }
    ));
    let urr = &r.urr["294K"];
    assert_eq!(urr.inelastic_flag, 4);
    assert!(urr.multiply_smooth);
    assert_eq!(urr.table.len(), urr.energy.len() * 6 * urr.n_bands);

    // an absent label is an error naming the labels present
    let err = read_nuclide_selected(d.join("U235.h5"), Some("900K"), &[]).unwrap_err().to_string();
    assert!(err.contains("900K") && err.contains("294K"), "{err}");
}

#[test]
fn u234_kalbach_mann_and_partial_fissions_are_read() {
    let Some(d) = dir() else { return };
    let r = read_nuclide_selected(d.join("U234.h5"), Some("294K"), &[]).expect("read");
    for mt in [16, 17, 37, 91] {
        assert!(
            matches!(r.reactions[&mt].products[0].distributions[0], ReadDistribution::KalbachMann(_)),
            "MT={mt}"
        );
    }
    for mt in [19, 20, 21, 38] {
        let n_delayed = r.reactions[&mt]
            .products
            .iter()
            .filter(|p| p.emission_mode == "delayed")
            .count();
        assert_eq!(n_delayed, 6, "MT={mt}");
    }
}

/// `read_nuclide` (no selection) on a two-temperature file still reads one
/// consistent temperature: the first label, with its own grid checked.
#[test]
fn the_unselected_read_is_consistent_on_a_two_temperature_file() {
    let Some(d) = dir() else { return };
    let r = read_nuclide(d.join("H1.h5")).expect("read");
    assert_eq!(r.primary_temperature, "0K");
    let n = r.any_energy().unwrap().len();
    let rx = &r.reactions[&2];
    assert_eq!(rx.threshold_idx + rx.xs.len(), n);
}
