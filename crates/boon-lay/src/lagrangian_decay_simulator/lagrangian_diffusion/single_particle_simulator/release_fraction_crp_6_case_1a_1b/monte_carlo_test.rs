//! CRP-6 Case 1 verification: Walk-on-Spheres kernel release vs the Crank
//! analytical (continuum) solution.
//!
//! Both use the same kernel radius, temperature-dependent Jiang diffusion
//! coefficient, uniform initial concentration, and perfect-sink surface, so the
//! Monte-Carlo (Lagrangian) release fraction must agree with the closed-form
//! Crank series (Eulerian) to within Monte-Carlo statistics. The reference
//! literature ranges are those cited in the sibling analytical verification
//! (Hales et al. 2021, Table 4).

use fission_yields_data::prelude::Nuclide;
use uom::si::f64::*;
use uom::si::length::micrometer;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::hour;

use crate::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::release_fraction_analytical_solution::calculate_analytical_fraction_released;
use crate::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::release_fraction_crp_6_case_1a_1b::simulation_code::{
    kernel_diffusion_coefficient, mc_kernel_release_fraction,
};

/// Common CRP-6 kernel radius (425 µm diameter).
fn kernel_radius() -> Length {
    Length::new::<micrometer>(425.0) / 2.0
}

/// Runs the MC vs analytical comparison for a temperature and prints both, so a
/// `--nocapture` run supplies the measured numbers for the V&V record.
fn compare(label: &str, temp_c: f64, expected_lo: f64, expected_hi: f64) {
    let radius = kernel_radius();
    let temperature = ThermodynamicTemperature::new::<degree_celsius>(temp_c);
    let time = Time::new::<hour>(200.0);
    let nuclide = Nuclide::Cs137;

    let d = kernel_diffusion_coefficient(nuclide, temperature);
    let analytical = calculate_analytical_fraction_released(d, radius, time, 200);
    let mc = mc_kernel_release_fraction(nuclide, radius, temperature, time, 40_000, 0x0C1A_5EED);

    println!("CRP-6 Case 1 — {label} ({temp_c}\u{b0}C, 200 h)");
    println!("  D               = {:e} m^2/s", d.get::<uom::si::diffusion_coefficient::square_meter_per_second>());
    println!("  MC release      = {mc:.4}");
    println!("  Crank analytic  = {analytical:.4}");
    println!("  |MC - Crank|    = {:.4}", (mc - analytical).abs());

    // Agreement with the continuum solution to within MC statistics.
    assert!(
        (mc - analytical).abs() < 0.02,
        "{label}: MC {mc:.4} disagrees with Crank {analytical:.4}"
    );
    // Sanity against the cited literature range for the analytical value.
    assert!(
        analytical >= expected_lo && analytical <= expected_hi,
        "{label}: analytical {analytical:.4} outside expected [{expected_lo}, {expected_hi}]"
    );
}

#[test]
fn crp6_case1a_cs_release_1200c_matches_crank() {
    // Analytical value for this case is ~0.53 (see the sibling analytical test).
    compare("Case 1a", 1200.0, 0.45, 0.55);
}

#[test]
fn crp6_case1b_cs_release_1600c_matches_crank() {
    // Analytical value for this case is ~0.97-1.00.
    compare("Case 1b", 1600.0, 0.97, 1.00);
}
