//! Notebook: **`search`** (criticality search) — verification tests (op-6tz.6).
//!
//! Notebook provenance: openmc-notebooks `search.ipynb`, commit
//! `cf1e5db2cd77d53a4fa76ffd9af7ab638f468713` (MIT).
//!
//! # Methodology
//!
//! The OpenMC notebook drives `openmc.search_for_keff` (bisection) over boron
//! concentration for a PWR pin cell, finding the critical ~1926 ppm. The search
//! itself is a **transport** capability (repeated k-eigenvalue solves) that
//! belongs to `outram-mc-libs`, not this data crate. njoy's only role is to
//! *supply the cross sections* the transport solve consumes. So the full
//! notebook is scaffolded `#[ignore]` (transport, cross-track to op-6tz's
//! transport slice), and the one njoy-side precondition — that the fissile
//! nuclide's cross sections reconstruct — is a live check.
//!
//! # Results (2026-07-15)
//!
//! - U-235 (the pin-cell fuel nuclide) reconstructs and gives a physical thermal
//!   fission cross section (~585 b), i.e. the data layer can feed a search.

use std::path::PathBuf;

use njoy_outram_park_fork::interface::NuclearDataLibrary;
use uom::si::{area::barn, energy::electronvolt, f64::Energy};

const U235_MAT: i32 = 9228;

fn u235_path() -> PathBuf {
    let p = njoy_outram_park_fork::reference_data::reference_endf_dir()
        .join("n-092_U_235-ENDF8.0.endf");
    p
}

/// njoy-side precondition for a criticality search: the fuel nuclide's cross
/// sections are available and physical. (The search loop itself is transport.)
#[test]
fn fuel_cross_sections_available_for_search() {
    let lib = NuclearDataLibrary::from_file(u235_path(), U235_MAT)
        .expect("load U-235 ENDF")
        .reconstruct(0.001)
        .expect("RECONR U-235");
    let fis = lib
        .fission_xs(Energy::new::<electronvolt>(0.0253))
        .get::<barn>();
    println!("U-235 thermal σ_f available for search = {fis:.1} b");
    assert!((500.0..650.0).contains(&fis), "σ_f(thermal) = {fis:.1} b");
}

/// Notebook op: `openmc.search_for_keff(model_builder, ...)` bisection to the
/// critical boron concentration.
///
/// Requires a **transport / k-eigenvalue solver** and a criticality-search
/// driver — an `outram-mc-libs` capability (op-6tz transport slice), not this
/// data crate. njoy supplies only the cross sections (verified above).
#[test]
#[ignore = "requires transport k-eff search (outram-mc-libs / op-6tz transport slice)"]
fn search_for_critical_boron_concentration() {
    panic!("search_for_keff needs a transport k-eigenvalue solver — outram-mc-libs owns this");
}
