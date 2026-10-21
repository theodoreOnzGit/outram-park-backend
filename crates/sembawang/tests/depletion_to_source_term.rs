// SPDX-License-Identifier: GPL-3.0

//! **The depletion path into the release chain** (GitHub #951): an outram-mc
//! burnup run, converted to a `RadionuclideInventory`, restricted to what
//! TRISO-ATOPS models, released.
//!
//! # Methodology
//!
//! Deplete OpenMC's `chain_simple` U-235/U-238 pin (Predictor, 2 x 5 d at
//! 1 MW in 1000 cm³), convert the end-of-life step, state that the one
//! depleted region is one radial ring, restrict to TRISO-ATOPS's table, and
//! run `accident_release` on the NP-MHTGR reference plant over a 600 to
//! 1600 °C ramp. Pass criteria:
//! - the chain nuclides TRISO-ATOPS does not model are **reported**, by name
//!   (none silently dropped), and the kept ones keep the chain's order;
//! - the source term releases exactly the kept nuclides;
//! - the release is **linear in the inventory**: doubling the depleted volume
//!   doubles every nuclide's release to round-off (the property that makes a
//!   depletion-sized inventory interchangeable with a prescribed one).
//!
//! This is a plumbing check, not a source term for any reactor: a one-group
//! LWR pin chain is not HTGR fuel.
//!
//! # Results
//!
//! 2026-10-17: reported (dropped) Xe136, Gd157, Gd156, U234, U235, U238;
//! kept and released I-135, Xe-135, Cs-135, none screened out. I-135 releases
//! 5.49e10 Bq and Xe-135 7.30e9 Bq; Cs-135 is stable in `chain_simple`, so it
//! carries zero activity and releases exactly zero. The doubled-volume ratio
//! is 2 to within 1e-12 on both non-zero releases.

use outram_mc_libs::depletion::chain::DepletionChain;
use outram_mc_libs::depletion::integrators::Integrator;
use outram_mc_libs::depletion::operator::{deplete_with, BurnupSettings};
use sembawang::accident::release::{accident_release, PlantParameters};
use sembawang::inventory::{self, RadionuclideInventory, Regions};
use sembawang::scenario::TemperatureTransient;
use uom::si::f64::{ThermodynamicTemperature, Time, Volume};
use uom::si::radioactivity::becquerel;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::second;
use uom::si::volume::cubic_centimeter;

fn depleted(volume_cm3: f64) -> (RadionuclideInventory, Vec<String>) {
    let chain = DepletionChain::simple();
    let settings = BurnupSettings {
        power_watts: 1.0e6,
        fuel_volume_cm3: 1000.0,
        step_days: 5.0,
        n_steps: 2,
        ..BurnupSettings::default()
    };
    let n_u = 0.0232_f64;
    let initial = vec![
        ("U235".to_string(), 0.0425 * n_u),
        ("U238".to_string(), 0.9575 * n_u),
    ];
    let result = deplete_with(Integrator::Predictor, &chain, &initial, &settings);
    let eol = RadionuclideInventory::from_burnup_step(
        result.eol(),
        &chain,
        Volume::new::<cubic_centimeter>(volume_cm3),
        "pin",
        "chain_simple, Predictor, 2 x 5 d at 1 MW",
    )
    .unwrap()
    .with_regions(Regions::RadialRings(1))
    .unwrap();
    let (kept, dropped) = inventory::restrict_to_triso_atops(&eol).unwrap();
    (kept, dropped.iter().map(|i| i.name()).collect())
}

#[test]
fn a_depleted_inventory_runs_through_the_release_chain() {
    let transient = TemperatureTransient::from_ramp(
        ThermodynamicTemperature::new::<degree_celsius>(600.0),
        ThermodynamicTemperature::new::<degree_celsius>(1600.0),
        Time::new::<second>(1.0e4),
        Time::new::<second>(1.0e5),
        41,
        1,
        1,
    )
    .unwrap();
    let plant = PlantParameters::np_mhtgr_reference(1.0e-4, 1.0e-4, 0.0);

    let (inv, dropped) = depleted(1000.0);
    assert_eq!(
        dropped,
        vec!["Xe136", "Gd157", "Gd156", "U234", "U235", "U238"]
    );
    assert_eq!(inventory::names(&inv), vec!["I-135", "Xe-135", "Cs-135"]);

    let one = accident_release(&inv, &transient, &plant).unwrap();
    let two = accident_release(&depleted(2000.0).0, &transient, &plant).unwrap();
    let labels: Vec<&str> = one
        .source_term
        .nuclides
        .iter()
        .map(|n| n.label.as_str())
        .collect();
    for l in &labels {
        assert!(["I-135", "Xe-135", "Cs-135"].contains(l), "{l}");
    }
    assert!(
        labels.contains(&"I-135") && labels.contains(&"Xe-135"),
        "{labels:?}"
    );
    for (a, b) in one
        .source_term
        .nuclides
        .iter()
        .zip(&two.source_term.nuclides)
    {
        let (qa, qb) = (
            a.total_released().get::<becquerel>(),
            b.total_released().get::<becquerel>(),
        );
        assert!(qa.is_finite() && qa >= 0.0, "{}: {qa}", a.label);
        if qa > 0.0 {
            assert!(((qb / qa) - 2.0).abs() < 1e-12, "{}: {qb} / {qa}", a.label);
        }
    }
    // Recorded 2026-10-17: all three released, none screened; Cs-135 is
    // stable in `chain_simple`, so zero activity in, zero out.
    assert_eq!(labels, vec!["I-135", "Xe-135", "Cs-135"]);
    assert!(one.screened_out.is_empty());
    assert_eq!(
        one.source_term.nuclides[2]
            .total_released()
            .get::<becquerel>(),
        0.0
    );
    let i135 = one
        .source_term
        .nuclides
        .iter()
        .find(|n| n.label == "I-135")
        .unwrap();
    assert!(i135.total_released().get::<becquerel>() > 0.0);
}
