// SPDX-License-Identifier: GPL-3.0-only
//! How high does the plume really fly, and what does a building do to it?
//! pyDOSEIA's plume-rise and building-wake formulas, which upstream defines
//! but never calls, applied to the steady plume of rung 1.
//!
//! > **Research, education and V&V only** (`RESPONSIBLE_USE.md`). Every input
//! > below is **illustrative** (upstream's own defaults, or a round number
//! > stated as such); none describes a real stack or building.
//!
//! Written for the dispersion lesson track (gh:#530, rung 3).
//!
//! ```text
//! cargo run --release -p buangkok --example plume_rise_and_wake
//! ```
//!
//! # Methodology
//!
//! - **Plume rise.** `buangkok::pydoseia::plume_rise`, ported from pyDOSEIA
//!   `metfunc.py`. `plume_rise_neutral_unstable` is ported faithfully;
//!   `plume_rise_stable_upstream` reproduces upstream defect D6 (always class
//!   F, the windy formula); `plume_rise_stable_both_formulas` is the port's
//!   labelled divergence. **None of these formulas has been checked against
//!   the references upstream cites** (the AERB guide, IAEA-TECDOC-379), which
//!   are not in the open corpus (gh:#530 gap issue).
//! - **Inputs** are upstream's defaults: exit velocity `W0 = 10` m/s, inner
//!   diameter `D_i = 5` m, outer diameter `D_e = 8` m, wind `U = 2` m/s.
//! - **Effect on the plume.** The ground-level centreline `chi/Q` of rung 1
//!   (`dilution_single_plume_no_met`, class D, 2 m/s at 10 m) at 1 km, with the
//!   release height `H = 30` m and with the effective height `H + dh(1 km)`.
//!   The wind-speed height correction is evaluated at whatever height is
//!   passed, as upstream does.
//! - **Building wake.** `building_wake_gifford`, the port's corrected
//!   version of upstream's unusable function (D5): Gifford's
//!   `chi/Q = 1 / ((c A + pi sigma_y sigma_z) U)`, `c = 0.5`, floored at a
//!   third of the unwaked value. Ground-level release (pyDOSEIA raises it to
//!   10 m in the wind correction only), class F, 1 m/s at 10 m, an
//!   **illustrative** building cross-section of 1000 m².
//!
//! # Results
//!
//! Run 2026-10-04 on `develop` `d4428668be` plus this file (`--release`, one
//! core):
//!
//! - Neutral rise, upstream defaults: `dh = 75.0 m` (the cap `3 D_i W0 / U`)
//!   at x = 50, 100, 200, 500 and 1000 m, because the downwash term adds
//!   `-3 (1.5 - 5) 8 = +84 m`.
//! - Stable rise: upstream as it runs 29.32 m; divergence E calm 116.45 m,
//!   windy 32.94 m; F calm 97.78 m, windy 29.32 m.
//! - Class D, 2 m/s at 10 m, 1 km, ground-level centreline: `chi/Q` 3.643e-5
//!   s/m^3 at H = 30 m, 1.863e-7 at H + dh = 105 m (ratio 0.005).
//! - Wake, ground release, class F, 1 m/s, A = 1000 m^2: ratio to unwaked
//!   0.333 (the floor) at 100 and 200 m, 0.412 at 400 m, 0.764 at 1000 m.
//!
//! Interpretation: with upstream's defaults the downwash sign question decides
//! the near-field rise, and rise alone lowers this ground-level `chi/Q` about
//! 200-fold; none of it is in any dose pathway (gh:#542).

use buangkok::pydoseia::dispersion::{
    dilution_single_plume_no_met, height_correction_factor, sigma_y, sigma_z, MeanSpeedScaling,
    PlumeGeometry, Receptor, StabilityClass,
};
use buangkok::pydoseia::plume_rise::{
    building_wake_gifford, plume_rise_neutral_unstable, plume_rise_stable_both_formulas,
    plume_rise_stable_upstream, StableClass,
};
use uom::si::f64::{Length, Velocity};
use uom::si::length::meter;
use uom::si::velocity::meter_per_second;

/// Upstream's default stack exit velocity, m/s.
const W0: f64 = 10.0;
/// Upstream's default inner stack diameter, m.
const D_I: f64 = 5.0;
/// Upstream's default outer stack diameter, m.
const D_E: f64 = 8.0;
/// Upstream's default wind speed for the rise formulas, m/s.
const U: f64 = 2.0;
/// Release height for the effect-on-the-plume part, m (rung 1's stack).
const H_STACK_M: f64 = 30.0;
/// Illustrative building cross-section for the wake, m^2. Not a real building.
const BUILDING_AREA_M2: f64 = 1000.0;

fn ground_chi_over_q(class: StabilityClass, h_m: f64, x_m: f64, u10: f64) -> f64 {
    let geometry = PlumeGeometry {
        release_height: Length::new::<meter>(h_m),
        measurement_height: Length::new::<meter>(10.0),
        receptor: Receptor::GroundLevelCentreline,
    };
    let scaling = MeanSpeedScaling::PerClass([Velocity::new::<meter_per_second>(u10); 6]);
    dilution_single_plume_no_met(Length::new::<meter>(x_m), geometry, scaling)[class.index()]
        .seconds_per_cubic_meter()
}

fn main() {
    println!("Part 1. Plume rise, upstream defaults W0 = {W0} m/s, D_i = {D_I} m, D_e = {D_E} m, U = {U} m/s");
    println!("  neutral/unstable (A-D), faithful port, by downwind distance x:");
    for x in [50.0, 100.0, 200.0, 500.0, 1000.0] {
        let dh = plume_rise_neutral_unstable(W0, x, U, D_I, D_E);
        let cap = 3.0 * D_I * W0 / U;
        println!("    x = {x:>6} m   dh = {dh:>7.2} m   (the cap 3 D_i W0/U = {cap:.1} m)");
    }
    let up = plume_rise_stable_upstream(W0, U, D_I);
    let e = plume_rise_stable_both_formulas(W0, U, D_I, StableClass::E);
    let f = plume_rise_stable_both_formulas(W0, U, D_I, StableClass::F);
    println!("  stable, upstream as it runs (D6: always class F, windy formula): dh = {up:.2} m");
    println!(
        "  stable, the port's divergence:  E calm {:.2} m, windy {:.2} m;  F calm {:.2} m, windy {:.2} m",
        e.calm_formula, e.windy_formula, f.calm_formula, f.windy_formula
    );

    println!("\nPart 2. What the rise does to the ground-level centreline chi/Q at 1 km (class D, 2 m/s at 10 m)");
    let dh = plume_rise_neutral_unstable(W0, 1000.0, U, D_I, D_E);
    let at_h = ground_chi_over_q(StabilityClass::D, H_STACK_M, 1000.0, 2.0);
    let at_he = ground_chi_over_q(StabilityClass::D, H_STACK_M + dh, 1000.0, 2.0);
    println!("    H = {H_STACK_M} m:            chi/Q = {at_h:.3e} s/m^3");
    println!(
        "    H + dh = {:.1} m:       chi/Q = {at_he:.3e} s/m^3   (ratio {:.3})",
        H_STACK_M + dh,
        at_he / at_h
    );

    println!("\nPart 3. Building wake (Gifford, the port's corrected D5), ground release, class F, 1 m/s at 10 m, A = {BUILDING_AREA_M2} m^2 (illustrative)");
    for x in [100.0, 200.0, 400.0, 1000.0] {
        let unwaked = ground_chi_over_q(StabilityClass::F, 0.0, x, 1.0);
        let xl = Length::new::<meter>(x);
        let sy = sigma_y(StabilityClass::F, xl).get::<meter>();
        let sz = sigma_z(StabilityClass::F, xl).get::<meter>();
        // The wind the plume formula used: 1 m/s at 10 m, release raised to 10 m.
        let u = height_correction_factor(
            StabilityClass::F,
            Length::new::<meter>(0.0),
            Length::new::<meter>(10.0),
        );
        let waked = building_wake_gifford(unwaked, BUILDING_AREA_M2, u, sy, sz);
        println!(
            "    x = {x:>6} m   unwaked {unwaked:.3e}   wake {waked:.3e}   ratio {:.3}{}",
            waked / unwaked,
            if (waked - unwaked / 3.0).abs() < 1e-300_f64.max(1e-12 * unwaked) {
                "  (at the 1/3 floor)"
            } else {
                ""
            }
        );
    }
    println!(
        "\nReading notes:\n\
         - pyDOSEIA's own dose path uses NONE of these: its release height is the effective height as given.\n\
         - The formulas have not been checked against the references upstream cites (not in the open corpus).\n\
         - The neutral formula applies its downwash term 3 (1.5 - W0/U) D_e at every W0/U; when W0/U > 1.5\n\
           that term is negative and ADDS height. Whether the reference restricts it to W0 < 1.5 U is not\n\
           checked here (see the lesson, rung 3)."
    );
}
