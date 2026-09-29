// SPDX-License-Identifier: GPL-3.0
//! **A heated steam-generator tube must never cool its water below the
//! feedwater** -- the minimal reproduction of gh:#319 / #343, with no steam
//! generator, no tube metal, no helium and no plant.
//!
//! # The invariant
//!
//! A tube whose every wall node is at or above the feedwater temperature can
//! only add heat to the water. With advection and heating as the only energy
//! paths, no cell's specific enthalpy can fall below the inlet's -- a discrete
//! maximum principle that any bounded convection scheme satisfies. (The one
//! reversible exception is pressure work, `dh = dp / rho`: a 1 MPa swing in
//! liquid moves `h` by about 1 kJ/kg, which sets the tolerance below.)
//!
//! # What it caught (2026-09-29)
//!
//! `TampinesSteamArray` convected enthalpy with `fvc::div`, a plain **linear**
//! (central) face interpolation. At steady state the inlet cell's balance is
//! `m_dot (h_in - (h_0 + h_1)/2) + Q_0 = 0`, i.e.
//! `h_0 = 2 h_in - h_1 + 2 Q_0 / m_dot`: when the next cell boils while the
//! inlet cell is barely heated, `h_0` is driven below the feedwater without
//! bound. In `htgr_sim_v1`'s counter-flow steam generator the inlet cell is the
//! cold end, where the tube metal sits near the feedwater temperature, so any
//! transient that moves the boiling front toward the inlet walks that cell down
//! to the 273.15 K IF97 floor and the flash panics.
//!
//! # Methodology
//!
//! HTR-10-illustrative tube bundle as `htgr_sim_v1` builds it: 90 tubes of
//! 14 mm bore, 34 m, 8 cells, 4 MPa outlet, feedwater 313.15 K (forward
//! region-1 enthalpy, 171.08 kJ/kg) at 3 kg/s, the plant's cold-side
//! conductance `4.26e4 / 0.25` W/K split over the cells, 0.0125 s step, 2 outer
//! and 2 inner correctors at 0.3 relaxation. The wall is prescribed: the inlet
//! cell's wall **at** the feedwater temperature (no heating), 700 K elsewhere,
//! which boils the tube from cell 1 onward. 60 s of simulated time.
//!
//! # Results (measured 2026-09-29, `cargo test --release -j2 -p
//! tampines-steam-tables --test heated_tube_never_undercuts_its_feedwater`)
//!
//! | Scheme | Worst `min(h) - h_in` | Inlet cell |
//! |---|---|---|
//! | **van Leer (default)** | **-97.5 J/kg** (-0.02 K, start-up pressure work) | 171.1 kJ/kg = `h_in`, 313.15 K at 59 s |
//! | Linear (pre-2026-09-29, ablation) | **-100.0 kJ/kg at 10.74 s**, still falling ~1.5 kJ/kg per 0.125 s | 291.3 K at 10 s and falling |
//!
//! Interpretation: the walk is a property of the convection scheme alone, and
//! the bounded scheme removes it. Harness check of a discrete invariant, not a
//! physics validation.

use tampines_steam_tables::region_1_subcooled_liquid::h_tp_1;
use tampines_steam_tables::{EnergyConvectionScheme, TampinesSteamArray};
use uom::si::area::square_meter;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{
    Area, Length, MassRate, Pressure, Ratio, ThermalConductance, ThermodynamicTemperature, Time,
};
use uom::si::length::meter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::pressure::{megapascal, pascal};
use uom::si::ratio::ratio;
use uom::si::thermal_conductance::watt_per_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

/// Pressure-work allowance \[J/kg\]: `dp / rho` for a 1 MPa swing in liquid.
const PRESSURE_WORK_ALLOWANCE_J_PER_KG: f64 = 1.0e3;

/// Run the tube for `seconds` and return the worst `min_cell(h) - h_in`
/// \[J/kg\] and the time at which the undershoot first exceeded 100 kJ/kg, if
/// it did (the run stops there, before the flash reaches the IF97 floor).
fn worst_undershoot(scheme: EnergyConvectionScheme, seconds: f64) -> (f64, Option<f64>) {
    let n = 8usize;
    let dt = 0.0125;
    let p = Pressure::new::<megapascal>(4.0);
    let t_feed = ThermodynamicTemperature::new::<kelvin>(313.15);
    let h_in = h_tp_1(t_feed, p);
    let area = 90.0 * std::f64::consts::PI / 4.0 * 0.014_f64.powi(2);
    let mut arr = TampinesSteamArray::new(
        Length::new::<meter>(34.0),
        Area::new::<square_meter>(area),
        n as i64,
        Time::new::<second>(dt),
    )
    .unwrap();
    for c in 0..n {
        arr.p.internal[c] = p.get::<pascal>();
    }
    arr.set_temperature_vector(vec![t_feed; n]).unwrap();
    arr.set_pimple_algorithm(2, 2, Ratio::new::<ratio>(0.3), Ratio::new::<ratio>(0.3));
    arr.set_outlet_pressure(p);
    arr.set_he_convection_scheme(scheme);

    let ua = ThermalConductance::new::<watt_per_kelvin>(4.26e4 / 0.25 / n as f64);
    let mut wall = vec![ThermodynamicTemperature::new::<kelvin>(700.0); n];
    wall[0] = t_feed;

    let hin = h_in.get::<joule_per_kilogram>();
    let mut worst = f64::INFINITY;
    let steps = (seconds / dt).round() as usize;
    for i in 0..steps {
        arr.set_inlet_mass_flowrate(MassRate::new::<kilogram_per_second>(3.0));
        arr.set_inlet_enthalpy(h_in);
        arr.set_outlet_pressure(p);
        arr.lateral_link_new_temperature_vector_avg_conductance(ua, wall.clone())
            .unwrap();
        arr.advance_timestep(Time::new::<second>(dt)).unwrap();
        let hmin = arr
            .he
            .internal
            .as_slice()
            .iter()
            .cloned()
            .fold(f64::INFINITY, f64::min);
        worst = worst.min(hmin - hin);
        if hmin - hin < -100.0e3 {
            return (worst, Some((i + 1) as f64 * dt));
        }
    }
    (worst, None)
}

/// The default (van Leer) scheme keeps every cell at or above the feedwater.
#[test]
fn the_default_scheme_never_undercuts_the_feedwater() {
    let (worst, walked_at) = worst_undershoot(EnergyConvectionScheme::default(), 60.0);
    println!("van Leer: worst min(h) - h_in = {worst:.3} J/kg");
    assert_eq!(
        walked_at, None,
        "the inlet cell walked 100 kJ/kg below the feedwater"
    );
    assert!(
        worst > -PRESSURE_WORK_ALLOWANCE_J_PER_KG,
        "a cell fell {worst:.1} J/kg below the feedwater enthalpy in a tube that is only heated"
    );
}

/// ABLATION: the pre-2026-09-29 linear face value reproduces the walk. Pins
/// the mechanism, so the test above cannot pass for an unrelated reason.
#[test]
fn the_linear_ablation_reproduces_the_walk_toward_the_floor() {
    let (worst, walked_at) = worst_undershoot(EnergyConvectionScheme::Linear, 60.0);
    println!(
        "Linear: worst min(h) - h_in = {worst:.3} J/kg, 100 kJ/kg undercut at {walked_at:?} s"
    );
    assert!(
        walked_at.is_some(),
        "the linear scheme no longer walks the inlet cell down (worst {worst:.1} J/kg): \
         re-examine what this ablation is pinning"
    );
}
