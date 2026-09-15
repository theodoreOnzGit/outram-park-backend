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

/// **V&V — gFHR: reproducing upstream's `Tfmax_min` from upstream's own case
/// data, at upstream's own tolerance.**
///
/// ## Why the minimum, and only the minimum, is reachable
///
/// The peak-fuel temperature of a pebble is
///
/// ```text
///   Tfmax = T_fluid + q_surf/h + dT_pebble
/// ```
///
/// `dT_pebble` is the ported [`PebbleGeometry::steady_temperatures`] chain and is
/// a constant across this bed (uniform power, uniform `alpha`). `q_surf/h` is the
/// convective film drop. The only unknown is `T_fluid`, and computing it through
/// the bed needs the three-dimensional porous flow solve this crate does not
/// have.
///
/// **But at one place `T_fluid` is known exactly without any solve**: the inlet
/// plane, where upstream's own `0/fluidRegion/T` fixes it at 823.15 K. The
/// coldest pebble in the bed is the one sitting there, so `Tfmax_min` — and
/// nothing else in upstream's triple — can be reconstructed from first
/// principles.
///
/// The bed-average and maximum cannot: upstream's `Tfmax` spread is 162.206 K
/// against a mean coolant rise of only `P/(m_dot c_p) = 105.2 K`, so the bed is
/// meaningfully flow-maldistributed and no one-dimensional reduction recovers
/// those two.
///
/// ## Methodology
///
/// Every input is upstream's own, from the `3D_gFHR` case at commit `652b3da`:
///
/// | quantity | value | source |
/// |---|---|---|
/// | `m_dot` | 1173 kg/s | the arithmetic in `0/fluidRegion/U`'s own comment |
/// | `rho(T)` | `2413.03 − 0.4884 T` | `thermophysicalProperties`, `rhoCoeffs` |
/// | `c_p` | 2265.75 J/(kg K) | `CpCoeffs` |
/// | `mu` | 1e-3 Pa s | `muCoeffs` |
/// | `kappa` | 1.1 W/(m K) | `kappaCoeffs` |
/// | `D_h` | 0.0255737704918 m | `structureProperties/Core` |
/// | core radius | 1.2 m | `cylinderMesh.m4`, `define(r, 120)` |
/// | `Nu` | `2 + 1.1 Re^0.6 Pr^(1/3)` | `heatTransferModels/Core`, Wakao |
/// | `a_v` | `3 alpha / r_shell` | upstream's own comment in `phaseProperties` |
/// | `T_inlet` | 823.15 K | `0/fluidRegion/T`, `bottom` |
///
/// The Nusselt correlation is evaluated through this crate's **ported** closure
/// [`FsForcedConvectionHtc::Nusselt`], not a hand-written formula, so the
/// correlation itself is part of what is under test.
///
/// Upstream reference: `expectedTfmaxMin = 900.664 K`, at upstream's own
/// `Alltest` tolerance of `error = 0.01` (1 % relative).
///
/// ## Results (measured 2026-09-15)
///
/// | quantity | value |
/// |---|---|
/// | `Re` | 1.087e4 |
/// | `Pr` | 2.060 |
/// | `Nu` (Wakao) | 372.0 |
/// | `h` | 1.600e4 W/(m² K) |
/// | film drop `q_surf/h` | 13.64 K |
/// | pebble rise `dT_pebble` | 61.67 K |
/// | **`Tfmax_min` reconstructed** | **898.46 K** |
/// | upstream `expectedTfmaxMin` | 900.664 K |
/// | difference | **−2.20 K = −0.24 %** |
///
/// Inside upstream's own 1 % tolerance (9.01 K), with no fitted parameter.
///
/// **Where the 2.2 K sits, and why it has the sign it does.** The reconstruction
/// evaluates at the inlet *plane*; upstream's minimum is over *cell centres*, and
/// the first cell centre is half a cell above the inlet. With 75 cells over
/// 3.0947 m and a 105.2 K mean rise, half a cell is about 0.70 K of fluid
/// heating. The remainder is flow maldistribution near the inlet, which also
/// perturbs the local `Re` and hence `h`. Both effects push upstream's value
/// *above* the inlet-plane reconstruction, which is the sign observed.
#[test]
fn the_gfhr_minimum_peak_fuel_temperature_reproduces_upstream() {
    use crate::genfoam::thermal_hydraulics::closures::heat_transfer::fs_htc::FsForcedConvectionHtc;
    use crate::genfoam::thermal_hydraulics::closures::heat_transfer::PrandtlNumber;
    use crate::genfoam::thermal_hydraulics::units::ReynoldsNumber;
    use uom::si::f64::{Length, ThermalConductivity};
    use uom::si::length::meter;
    use uom::si::ratio::ratio;
    use uom::si::thermal_conductivity::watt_per_meter_kelvin;

    // Upstream's fluid and flow data.
    const MASS_FLOW: f64 = 1173.0; // kg/s
    const CP: f64 = 2265.75; // J/(kg K)
    const MU: f64 = 1.0e-3; // Pa s
    const KAPPA: f64 = 1.1; // W/(m K)
    const D_H: f64 = 0.0255737704918; // m
    const CORE_RADIUS: f64 = 1.2; // m

    let g = gfhr();
    let k = gfhr_k();

    // Density from upstream's own polynomial at the inlet temperature.
    let rho = 2413.03 - 0.4884 * T_COOLANT_INLET;
    // Velocity exactly as upstream's own U boundary condition computes it:
    // mdot/(rho * pi r^2 * volumeFraction).
    let area = std::f64::consts::PI * CORE_RADIUS * CORE_RADIUS * GFHR_SOLID_FRACTION;
    let velocity = MASS_FLOW / (rho * area);

    let re = rho * velocity * D_H / MU;
    let pr = MU * CP / KAPPA;

    // Wakao, through the ported closure rather than a hand-written formula.
    let wakao = FsForcedConvectionHtc::Nusselt {
        a: 2.0,
        b: 1.1,
        c: 0.6,
        d: 1.0 / 3.0,
        e: 0.0,
    };
    let h = wakao
        .heat_transfer_coefficient(
            ReynoldsNumber::new::<ratio>(re),
            PrandtlNumber::new::<ratio>(pr),
            ThermalConductivity::new::<watt_per_meter_kelvin>(KAPPA),
            Length::new::<meter>(D_H),
            None,
        )
        .value;
    let nu = h * D_H / KAPPA;

    // Interfacial area density, upstream's own identity a_v = 3 alpha / r_shell.
    let a_v = 3.0 * GFHR_SOLID_FRACTION / g.shell_radius;
    let q_surf = GFHR_POWER_DENSITY / a_v;
    let film = q_surf / h;

    let t = g.steady_temperatures(
        T_COOLANT_INLET + film,
        GFHR_POWER_DENSITY,
        GFHR_SOLID_FRACTION,
        &k,
    );
    let reconstructed = t.fuel_max;
    let error = reconstructed - TFMAX_MIN;
    let tolerance = 0.01 * TFMAX_MIN; // upstream's own Alltest tolerance

    println!(
        "\n=== gFHR Tfmax_min reconstructed from upstream's own case data ===\n\
         \trho(823.15 K)      {rho:9.2} kg/m3\n\
         \tvelocity           {velocity:9.6} m/s   (mdot/(rho A alpha))\n\
         \tRe                 {re:9.4e}\n\
         \tPr                 {pr:9.4}\n\
         \tNu (Wakao, ported) {nu:9.2}\n\
         \th                  {h:9.4e} W/(m2 K)\n\
         \ta_v                {a_v:9.2} 1/m\n\
         \tq_surf             {q_surf:9.4e} W/m2\n\
         \t--\n\
         \tfilm drop          {film:9.3} K\n\
         \tpebble rise        {:9.3} K\n\
         \tTfmax_min (recon)  {reconstructed:9.3} K\n\
         \tTfmax_min upstream {TFMAX_MIN:9.3} K\n\
         \tdifference         {error:+9.3} K  ({:+.3} %), tolerance +/-{tolerance:.2} K",
        t.total_rise(),
        100.0 * error / TFMAX_MIN,
    );

    assert!(
        error.abs() < tolerance,
        "the reconstructed Tfmax_min is {reconstructed:.3} K against upstream's \
         {TFMAX_MIN} K, a difference of {error:+.3} K, outside upstream's own \
         1 % tolerance of {tolerance:.2} K"
    );
    // The sign is predicted: upstream's minimum is at the first cell CENTRE,
    // half a cell above the inlet plane this reconstructs, so upstream must be
    // the warmer of the two.
    assert!(
        error < 0.0,
        "the reconstruction ({reconstructed:.3} K) is ABOVE upstream's minimum \
         ({TFMAX_MIN} K). The inlet plane is colder than any cell centre, so the \
         reconstruction must come out below — this sign inversion means the film \
         drop or the pebble rise is overstated."
    );
}
