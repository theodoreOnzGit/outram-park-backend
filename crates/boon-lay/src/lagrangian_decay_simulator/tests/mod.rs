use fission_yields_data::prelude::Nuclide;
use uom::si::{f64::Time, time::{day, hour, minute, second, year}};

use crate::prelude::{decay_library::DecayLibrary, SingleNuclideSimulatorMC};

/// basically shows you how to get half life for a single nuclide
#[test]
fn half_life_test_tritium(){
    let mut decay_library: DecayLibrary = DecayLibrary::new();

    // begin tests
    let current_nuclide = Nuclide::H3;
    let sim = SingleNuclideSimulatorMC::new_decay_chain_simulation(
        current_nuclide, 
        &mut decay_library);

    let half_life: Time = Time::new::<year>(12.3);
    let test_half_life = sim.get_current_half_life();

    approx::assert_relative_eq!(
        half_life.get::<second>(),
        test_half_life.get::<second>(),
        max_relative=9e-3
    );



}

// now, these are vibe coded 


// Helper used by all tests
fn assert_half_life_for(nuclide: Nuclide, expected: Time, max_rel: f64) {
    let mut decay_library: DecayLibrary = DecayLibrary::new();
    let sim = SingleNuclideSimulatorMC::new_decay_chain_simulation(nuclide, &mut decay_library);
    let test_half_life = sim.get_current_half_life();

    approx::assert_relative_eq!(
        expected.get::<second>(),
        test_half_life.get::<second>(),
        max_relative = max_rel
    );
}

// Tritium (H-3) – matches your example ~12.32 y
#[test]
fn half_life_h3() {
    assert_half_life_for(Nuclide::H3, Time::new::<year>(12.32), 1e-2);
}

// Iodine-131 ~8.02 days
#[test]
fn half_life_i131() {
    assert_half_life_for(Nuclide::I131, Time::new::<day>(8.02), 1e-2);
}

// Caesium-137 ~30.17 years
#[test]
fn half_life_cs137() {
    assert_half_life_for(Nuclide::Cs137, Time::new::<year>(30.17), 9e-3);
}

// Strontium-90 ~28.8 years
#[test]
fn half_life_sr90() {
    assert_half_life_for(Nuclide::Sr90, Time::new::<year>(28.8), 1e-3);
}

// Cobalt-60 ~5.271 years
#[test]
fn half_life_co60() {
    assert_half_life_for(Nuclide::Co60, Time::new::<year>(5.271), 1e-3);
}

// Carbon-14 ~5730 years
#[test]
fn half_life_c14() {
    assert_half_life_for(Nuclide::C14, Time::new::<year>(5730.0), 9e-3);
}

// Potassium-40 ~1.248e9 years
#[test]
fn half_life_k40() {
    assert_half_life_for(Nuclide::K40, Time::new::<year>(1.248e9), 5e-3);
}

// Uranium-238 ~4.468e9 years
#[test]
fn half_life_u238() {
    assert_half_life_for(Nuclide::U238, Time::new::<year>(4.468e9), 5e-3);
}

// Uranium-235 ~7.038e8 years
#[test]
fn half_life_u235() {
    assert_half_life_for(Nuclide::U235, Time::new::<year>(7.038e8), 5e-3);
}

// Thorium-232 ~1.405e10 years
#[test]
fn half_life_th232() {
    assert_half_life_for(Nuclide::Th232, Time::new::<year>(1.405e10), 1e-2);
}

// Plutonium-239 ~2.41e4 years
#[test]
fn half_life_pu239() {
    assert_half_life_for(Nuclide::Pu239, Time::new::<year>(2.41e4), 1e-2);
}

// Radium-226 ~1600 years
#[test]
fn half_life_ra226() {
    assert_half_life_for(Nuclide::Ra226, Time::new::<year>(1600.0), 1e-3);
}

// Radon-222 ~3.8235 days
#[test]
fn half_life_rn222() {
    assert_half_life_for(Nuclide::Rn222, Time::new::<day>(3.8235), 1e-3);
}

// Polonium-210 ~138.376 days
#[test]
fn half_life_po210() {
    assert_half_life_for(Nuclide::Po210, Time::new::<day>(138.376), 1e-3);
}

// If your enum includes Ba-137m (metastable), ~2.552 minutes
// (Your earlier example used stable Ba-137; if Ba-137m is available, include this.)
#[test]
fn half_life_ba137m() {
    assert_half_life_for(Nuclide::Ba137m, Time::new::<minute>(2.552), 1e-3);
}

// Sodium-22 ~2.601 years
#[test]
fn half_life_na22() {
    assert_half_life_for(Nuclide::Na22, Time::new::<year>(2.601), 9e-3);
}

// Xenon-135 ~9.14 hours
#[test]
fn half_life_xe135() {
    assert_half_life_for(Nuclide::Xe135, Time::new::<hour>(9.14), 1e-3);
}

// Krypton-85 ~10.756 years
#[test]
fn half_life_kr85() {
    assert_half_life_for(Nuclide::Kr85, Time::new::<year>(10.756), 1e-3);
}
