// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full notice.

//! Tests for the `nuclearSteadyStatePebble` port, on the GeN-Foam `3D_gFHR`
//! tutorial's own data.

use super::*;

/// The gFHR tutorial's pebble, from
/// `Tutorials/reactorCases/3D_gFHR/rootCase/constant/fluidRegion/phaseProperties`
/// at upstream commit `652b3da`.
fn gfhr() -> PebbleGeometry {
    PebbleGeometry {
        core_radius: 1.38e-2,
        matrix_radius: 1.8e-2,
        shell_radius: 2.0e-2,
        fuel_radius: 212.5e-6,
        buffer_radius: 312.5e-6,
        inner_pyc_radius: 352.5e-6,
        silicon_carbide_radius: 387.5e-6,
        outer_pyc_radius: 427.5e-6,
        triso_count: 9022.0,
    }
}

/// The gFHR conductivities. Upstream states each as a polynomial whose only
/// non-zero coefficient is the constant term, so these are exact for that case.
fn gfhr_k() -> PebbleConductivities {
    PebbleConductivities {
        fuel: 3.3073,
        buffer: 0.50,
        pyrolytic_carbon: 4.00,
        silicon_carbide: 90.3,
        graphite: 41.68,
    }
}

/// `powerDensityNeutronics` (W/m^3) and `volumeFraction` from the same block.
const GFHR_POWER_DENSITY: f64 = 19.96567386e6;
const GFHR_SOLID_FRACTION: f64 = 0.61;

/// Upstream's `Alltest` reference peak fuel temperatures, K.
const TFMAX_AVG: f64 = 981.808;
const TFMAX_MIN: f64 = 900.664;
const TFMAX_MAX: f64 = 1062.87;
/// Upstream's own coolant inlet boundary condition, `0/fluidRegion/T`.
const T_COOLANT_INLET: f64 = 823.15;

/// The chain is monotone inward and every step is strictly positive — the
/// cheapest check that no term has a flipped sign.
#[test]
fn the_temperature_chain_increases_monotonically_inward() {
    let t = gfhr().steady_temperatures(900.0, GFHR_POWER_DENSITY, GFHR_SOLID_FRACTION, &gfhr_k());
    assert!(t.surface < t.matrix_outer, "shell");
    assert!(t.matrix_outer < t.matrix_average, "annulus");
    assert!(t.matrix_average < t.particle_surface, "coatings");
    assert!(t.particle_surface < t.fuel_average, "kernel average");
    assert!(
        t.fuel_average < t.fuel_max,
        "kernel centre above its average"
    );
    // The kernel centre rise is 15/6 = 2.5x the average rise, exactly.
    let avg_rise = t.fuel_average - t.particle_surface;
    let max_rise = t.fuel_max - t.particle_surface;
    assert!(
        (max_rise / avg_rise - 15.0 / 6.0).abs() < 1e-12,
        "centre/average rise ratio is {}, expected 2.5",
        max_rise / avg_rise
    );
}

/// The Maxwell mixture rule sits between the two phases it mixes, and reduces to
/// the matrix conductivity when there are no particles.
#[test]
fn the_maxwell_effective_conductivity_is_bounded_by_its_phases() {
    let g = gfhr();
    let k = gfhr_k();
    let k_eff = g.effective_matrix_conductivity(&k);
    let k_triso = (1.0 / g.fuel_radius - 1.0 / g.outer_pyc_radius) / g.coating_resistance(&k);
    println!(
        "gFHR matrix: k_graphite = {:.3}, k_triso = {:.4}, PF = {:.4}, k_eff = {:.4} W/(m K)",
        k.graphite,
        k_triso,
        g.packing_fraction(),
        k_eff
    );
    // The particles are far less conductive than graphite here, so mixing them
    // in must lower the effective value, and it cannot go below the dispersed
    // phase.
    assert!(k_triso < k.graphite, "gFHR TRISO are the poorer conductor");
    assert!(
        k_eff < k.graphite && k_eff > k_triso,
        "k_eff = {k_eff} is outside [{k_triso}, {}]",
        k.graphite
    );

    // No particles -> the bare matrix value.
    let mut empty = g;
    empty.triso_count = 1e-12;
    assert!(
        (empty.effective_matrix_conductivity(&k) - k.graphite).abs() < 1e-6,
        "an unloaded matrix must be plain graphite"
    );
}

/// The chain is linear in power, so doubling the power density doubles every
/// temperature rise — the property that makes a uniformly-powered bed's `Tfmax`
/// spread equal to its surface-temperature spread.
#[test]
fn every_rise_is_linear_in_the_power_density() {
    let g = gfhr();
    let k = gfhr_k();
    let a = g.steady_temperatures(900.0, GFHR_POWER_DENSITY, GFHR_SOLID_FRACTION, &k);
    let b = g.steady_temperatures(900.0, 2.0 * GFHR_POWER_DENSITY, GFHR_SOLID_FRACTION, &k);
    assert!(
        (b.total_rise() / a.total_rise() - 2.0).abs() < 1e-12,
        "rise ratio {} for a doubled power density",
        b.total_rise() / a.total_rise()
    );
    // And independent of the surface temperature it is anchored on.
    let c = g.steady_temperatures(1200.0, GFHR_POWER_DENSITY, GFHR_SOLID_FRACTION, &k);
    assert!(
        (c.total_rise() - a.total_rise()).abs() < 1e-9,
        "the rise must not depend on the surface temperature"
    );
}

/// **V&V — gFHR: the ported model's pebble rise is consistent with upstream's
/// own reported peak fuel temperatures and coolant inlet.**
///
/// ## Methodology
///
/// The gFHR tutorial's `powerDensityNeutronics` is a **uniform scalar**
/// (`19.96567386e6` W/m^3), not a field, and `alpha` is uniform at `0.61`. The
/// chain is linear in power and carries no other spatial dependence, so the
/// surface-to-peak-kernel rise `dT` is **the same constant in every cell of the
/// bed**. That turns upstream's three reported statistics into a constraint that
/// can be checked without running the coupled thermal-hydraulics:
///
/// ```text
///   Tfmax_min = T_surf,min + dT,      T_surf,min >= T_coolant,inlet
///   =>  dT <= Tfmax_min - T_coolant,inlet = 900.664 - 823.15 = 77.514 K
/// ```
///
/// The inequality is strict in practice: the coldest pebble surface sits *above*
/// the inlet coolant temperature by the convective film drop, so the slack
/// between the computed `dT` and 77.514 K **is** that film drop, and must be
/// small and positive. A `dT` above 77.514 K would mean the ported model
/// produces more pebble-internal rise than upstream's own numbers leave room
/// for — a genuine disagreement — and a `dT` far below it would leave an
/// implausibly large film drop.
///
/// Uniform power also predicts that the spread of `Tfmax` is purely the spread
/// of the surface temperature: `Tfmax_max - Tfmax_min = 162.206 K` is a
/// statement about the coolant and the film, not about the pebble.
///
/// ## Pass criterion
///
/// `0 < dT < 77.514 K`, with the implied film drop
/// `77.514 - dT` reported. This uses only upstream's own published numbers and
/// upstream's own case data; nothing is fitted.
///
/// ## Results (measured 2026-09-15)
///
/// See the printed report.
#[test]
fn the_gfhr_pebble_rise_fits_inside_upstreams_reported_temperatures() {
    let g = gfhr();
    let k = gfhr_k();
    // The surface temperature is an input the port cannot compute; the rise is
    // independent of it, so any value in range serves.
    let t = g.steady_temperatures(900.0, GFHR_POWER_DENSITY, GFHR_SOLID_FRACTION, &k);
    let dt = t.total_rise();
    let headroom = TFMAX_MIN - T_COOLANT_INLET;
    let film = headroom - dt;

    println!(
        "\n=== gFHR pebble, upstream's own case data through the ported model ===\n\
         \tpebble power              {:.1} W  (q = {GFHR_POWER_DENSITY:.4e} W/m3, alpha = {GFHR_SOLID_FRACTION})\n\
         \tk_eff matrix (Maxwell)    {:.4} W/(m K)   (bare graphite {:.2})\n\
         \t--\n\
         \tsurface -> matrix outer   {:7.3} K\n\
         \tmatrix outer -> average   {:7.3} K\n\
         \tTRISO coatings            {:7.3} K\n\
         \tkernel surface -> centre  {:7.3} K\n\
         \t          total rise      {dt:7.3} K\n\
         \t--\n\
         \tupstream Tfmax min/avg/max  {TFMAX_MIN} / {TFMAX_AVG} / {TFMAX_MAX} K\n\
         \tcoolant inlet               {T_COOLANT_INLET} K\n\
         \theadroom Tfmax_min - inlet  {headroom:.3} K\n\
         \timplied convective film drop at the coldest pebble: {film:.3} K\n\
         \timplied bed-average pebble surface: {:.2} K\n\
         \tTfmax spread {:.3} K is surface-temperature spread only (uniform power)",
        t.pebble_power,
        t.matrix_effective_conductivity,
        k.graphite,
        t.matrix_outer - t.surface,
        t.matrix_average - t.matrix_outer,
        t.particle_surface - t.matrix_average,
        t.fuel_max - t.particle_surface,
        TFMAX_AVG - dt,
        TFMAX_MAX - TFMAX_MIN,
    );

    assert!(dt > 0.0, "a heated pebble must be hotter than its surface");
    assert!(
        dt < headroom,
        "the ported model's pebble rise is {dt:.3} K, but upstream's own \
         Tfmax_min ({TFMAX_MIN} K) and coolant inlet ({T_COOLANT_INLET} K) leave \
         only {headroom:.3} K for it. The model disagrees with upstream's \
         reported temperatures."
    );
    assert!(
        film > 0.0 && film < 40.0,
        "the implied convective film drop at the coldest pebble is {film:.3} K, \
         which is not a physical pebble-bed film drop for this flow"
    );
}
