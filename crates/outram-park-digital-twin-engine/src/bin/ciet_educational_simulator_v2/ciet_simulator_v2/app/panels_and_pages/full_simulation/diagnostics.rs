//! Temperature diagnostics printed to the command line: bulk and wall
//! temperatures either side of the heater, the DHX shell and the DHX tube.
//!
//! ## Provenance
//!
//! Ported from the CIET Educational Simulator **v1**
//! (`crates/tuas_boussinesq_solver/examples/ciet_educational_simulator/ciet_simulator_v1/app/panels_and_pages/full_simulation/diagnostics.rs`),
//! GPL-3.0, same licence. The functions and their `dbg!` output are unchanged.
//!
//! **What v2 changed here: nothing but this header -- but the CALLER quieted it.**
//! v1 passed `print_debug_results = true` unconditionally, so every timestep
//! dumped the whole loop to stderr. v2's physics loop passes the
//! `--verbose-temperatures` flag instead, defaulting to `false`, because in
//! headless mode the dump buries the one-line status output. The functions
//! themselves still print exactly what v1's printed when asked to.
//!
//! `egui`-free, so this builds on Android/Termux.

use tuas_boussinesq_solver::pre_built_components::insulated_pipes_and_fluid_components::InsulatedFluidComponent;
use tuas_boussinesq_solver::prelude::beta_testing::InsulatedPorousMediaFluidComponent;
use uom::si::f64::*;
use uom::si::thermodynamic_temperature::degree_celsius;
/// these are temperature diagnostic
/// functions to check bulk and wall temperature before
/// and after the heater
///
/// before heater: BT-11, WT-10
/// after heater and MX-10: BT-12, WT-13
///
/// so can take heater bottom head temperature (1b) at wall
/// and at bulk
///
/// I'm also using the bulk fluid temperature inside the static mixer
/// and its wall temperature
/// as a proxy for BT-12 and WT-12
pub fn pri_loop_heater_temperature_diagnostics_ver_4(
    heater_bottom_head_1b: &mut InsulatedPorousMediaFluidComponent,
    static_mixer_10_label_2: &mut InsulatedFluidComponent,
    print_debug_results: bool,
) -> (
    (ThermodynamicTemperature, ThermodynamicTemperature),
    (ThermodynamicTemperature, ThermodynamicTemperature),
) {
    // bulk and wall temperatures before entering heater
    let bt_11 = heater_bottom_head_1b
        .pipe_fluid_array
        .try_get_bulk_temperature()
        .unwrap();
    let wt_10 = heater_bottom_head_1b
        .pipe_shell
        .try_get_bulk_temperature()
        .unwrap();

    // bulk and wall temperatures after entering heater
    let bt_12 = static_mixer_10_label_2
        .pipe_fluid_array
        .try_get_bulk_temperature()
        .unwrap();
    let wt_13 = static_mixer_10_label_2
        .pipe_shell
        .try_get_bulk_temperature()
        .unwrap();

    // debug
    if print_debug_results {
        dbg!(&(
            "bulk and wall temp degC, before and after heater respectively",
            bt_11.get::<degree_celsius>(),
            wt_10.get::<degree_celsius>(),
            bt_12.get::<degree_celsius>(),
            wt_13.get::<degree_celsius>(),
        ));
    }

    return ((bt_11, wt_10), (bt_12, wt_13));
}
