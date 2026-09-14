//! Diagnostics for the `(rho,h)` inversion — run these by name when a gate in
//! [`super`] moves, to separate *solver* error from *conditioning*.
//!
//! These follow the same convention as `diagnose_p_rho_h_error` in the
//! Chebyshev module: they are `#[test]`s so they compile and run with the
//! suite, but they assert only the thing they are diagnosing, and print the
//! breakdown a human needs to interpret a moved tolerance.

use uom::si::available_energy::kilojoule_per_kilogram;
use uom::si::f64::*;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::{bar, pascal};
use uom::si::specific_volume::cubic_meter_per_kilogram;

use crate::interfaces::functional_programming::ph_flash_eqm::v_ph_eqm;
use crate::interfaces::functional_programming::rho_h_flash_eqm::p_rho_h_eqm;

/// Is a large pressure error the solver's fault, or the problem's?
///
/// For a handful of states known to be hard — the compressed liquid, the
/// saturated-liquid boundary, a dense supercritical state — this prints, side
/// by side:
///
/// - the **pressure** error `|dp/p|`, which is what a caller sees;
/// - the **volume residual** `|v(p_found, h) - v_target| / v_target`, which is
///   what the root find actually minimises;
/// - the local amplification `A = |d ln p / d ln v|_h`.
///
/// The interpretation is the whole point: if the volume residual is at
/// floating-point noise while the pressure error is large, the root find has
/// done everything it can and the pressure is simply **not determined** by the
/// density to better than that — `dp/p ~ A * (dv/v)`. Only a volume residual
/// far above noise would indict the solver.
///
/// It asserts just one thing: that the volume residual is small everywhere. A
/// failure there is a real solver defect.
#[test]
fn diagnose_whether_pressure_error_is_solver_or_conditioning() {
    // (p_bar, h_kj_per_kg, label)
    let probes = [
        (
            8.0_f64,
            721.018_f64,
            "saturated liquid at 8 bar (worst round-trip node)",
        ),
        (300.0, 1454.55, "dense supercritical liquid-like, 30 MPa"),
        (990.0, 500.0, "heavily compressed liquid, 99 MPa"),
        (10.0, 3000.0, "superheated vapour, 10 bar"),
        (1.0, 2700.0, "wet-ish vapour near saturation, 1 bar"),
        (
            0.1,
            75.5555,
            "subcooled liquid at 0.1 bar, 18 degC (worst node)",
        ),
        (0.5, 75.5, "subcooled liquid at 0.5 bar"),
    ];

    println!(
        "{:<52} {:>12} {:>14} {:>12}",
        "state", "|dp/p|", "|dv/v| resid", "amplif. A"
    );

    let mut worst_volume_residual = 0.0_f64;

    for (p_bar_val, h_kj, label) in probes {
        let p_ref = Pressure::new::<bar>(p_bar_val);
        let h = AvailableEnergy::new::<kilojoule_per_kilogram>(h_kj);

        let v_target = v_ph_eqm(p_ref, h).get::<cubic_meter_per_kilogram>();
        let rho = MassDensity::new::<kilogram_per_cubic_meter>(1.0 / v_target);

        let p_found = p_rho_h_eqm(rho, h);
        let p_err =
            ((p_found.get::<pascal>() - p_ref.get::<pascal>()) / p_ref.get::<pascal>()).abs();

        let v_found = v_ph_eqm(p_found, h).get::<cubic_meter_per_kilogram>();
        let v_residual = ((v_found - v_target) / v_target).abs();

        // Local amplification, by a ONE-SIDED difference upward in ln p.
        //
        // A central difference is wrong here: several of these probes sit ON
        // the saturation line, where v has a kink. Straddling it averages the
        // two-phase side (where v responds strongly to p) with the liquid side
        // (where it barely responds), and reports a sensitivity the liquid
        // side does not have -- which is exactly how a real non-uniqueness
        // gets hidden behind a healthy-looking number.
        let step = 1.0e-4;
        let p_pa = p_ref.get::<pascal>();
        let ln_v_hi = v_ph_eqm(Pressure::new::<pascal>(p_pa * (1.0 + step)), h)
            .get::<cubic_meter_per_kilogram>()
            .ln();
        let ln_v_at = v_ph_eqm(p_ref, h).get::<cubic_meter_per_kilogram>().ln();
        let d_ln_v_d_ln_p = (ln_v_hi - ln_v_at) / step;
        let amplification = if d_ln_v_d_ln_p == 0.0 {
            f64::INFINITY
        } else {
            1.0 / d_ln_v_d_ln_p.abs()
        };

        println!("{label:<52} {p_err:>12.3e} {v_residual:>14.3e} {amplification:>12.3e}");

        worst_volume_residual = worst_volume_residual.max(v_residual);
    }

    println!("\nworst volume residual over the probes: {worst_volume_residual:.3e}");
    println!(
        "A volume residual at ~1e-15 with a large |dp/p| means the pressure is \
         not recoverable from the density to better than A * (dv/v); it is not \
         a solver defect."
    );

    assert!(
        worst_volume_residual < 1.0e-9,
        "the root find is not converging on its own residual: worst |dv/v| = \
         {worst_volume_residual:.3e}"
    );
}

/// Scans `v(p, h)` across the Region 1 / Region 4 seam at the worst
/// round-trip node, to show *why* the pressure there is not recoverable.
///
/// The node is `p = 8 bar, h = 721.018 kJ/kg`, which is the **saturated
/// liquid** at 8 bar — it sits exactly on the saturation line. This prints
/// `v` and the IF97 region on a fine pressure sweep through the seam, so a
/// reader can see the shape of the surface the root find is inverting instead
/// of taking a claim about it on trust.
///
/// Diagnostic only: it asserts nothing about accuracy.
#[test]
fn diagnose_the_region_1_to_4_seam_at_the_worst_node() {
    use crate::interfaces::functional_programming::ph_flash_eqm::ph_flash_region;

    let h = AvailableEnergy::new::<kilojoule_per_kilogram>(721.018);
    let v_at_8_bar = v_ph_eqm(Pressure::new::<bar>(8.0), h).get::<cubic_meter_per_kilogram>();

    println!("target v at 8 bar = {v_at_8_bar:.17e} m3/kg");
    println!(
        "{:>10} {:>24} {:>16} {:>10}",
        "p (bar)", "v (m3/kg)", "(v-v8)/v8", "region"
    );

    for i in 0..=24 {
        let p_bar_val = 7.90 + 0.02 * (i as f64);
        let p = Pressure::new::<bar>(p_bar_val);
        let v = v_ph_eqm(p, h).get::<cubic_meter_per_kilogram>();
        let rel = (v - v_at_8_bar) / v_at_8_bar;
        let region = ph_flash_region(p, h);
        println!("{p_bar_val:>10.3} {v:>24.17e} {rel:>16.3e} {region:>10?}");
    }
}

/// Scans `v(p, h)` and the region label across the full pressure domain at
/// `h = 2902.88 kJ/kg` — the enthalpy of the 40 bar / 280 degC superheated
/// vapour node, which was the last state whose volume residual would not
/// converge.
///
/// Diagnostic only; asserts nothing.
#[test]
fn diagnose_the_full_pressure_sweep_at_the_40_bar_node() {
    use crate::interfaces::functional_programming::ph_flash_eqm::ph_flash_region;
    use uom::si::pressure::pascal;

    let h = AvailableEnergy::new::<kilojoule_per_kilogram>(2902.88);
    let v_target = v_ph_eqm(Pressure::new::<bar>(40.0), h).get::<cubic_meter_per_kilogram>();
    println!("target v at 40 bar = {v_target:.12e} m3/kg");
    println!(
        "{:>14} {:>20} {:>14} {:>10}",
        "p (Pa)", "v (m3/kg)", "ln v - ln vt", "region"
    );

    // Stay strictly inside the domain: the exact endpoints p_sat(273.15 K) and
    // 100 MPa are the boundary itself, and the validity check rejects a probe
    // that lands a rounding step outside.
    let p_lo = 612.0_f64;
    let p_hi = 99.9e6_f64;
    for i in 0..=28 {
        let ln_p = p_lo.ln() + (p_hi.ln() - p_lo.ln()) * (i as f64) / 28.0;
        let p_pa = ln_p.exp();
        let p = Pressure::new::<pascal>(p_pa);
        let v = v_ph_eqm(p, h).get::<cubic_meter_per_kilogram>();
        let f = v.ln() - v_target.ln();
        let region = ph_flash_region(p, h);
        println!("{p_pa:>14.4e} {v:>20.10e} {f:>14.4e} {region:>10?}");
    }
}

/// Prints exactly what `p_rho_h_eqm` returns for the 40 bar / 280 degC node,
/// and what the pressure equation says about it, so the residual can be
/// attributed. Diagnostic only.
#[test]
fn diagnose_the_40_bar_node_solution() {
    use crate::interfaces::functional_programming::rho_h_flash_eqm::p_rho_h_eqm;
    use uom::si::pressure::pascal;

    let h = AvailableEnergy::new::<kilojoule_per_kilogram>(2902.88);
    let p_ref = Pressure::new::<bar>(40.0);
    let v_target = v_ph_eqm(p_ref, h).get::<cubic_meter_per_kilogram>();
    let rho = MassDensity::new::<kilogram_per_cubic_meter>(1.0 / v_target);

    let p_back = p_rho_h_eqm(rho, h);
    let v_back = v_ph_eqm(p_back, h).get::<cubic_meter_per_kilogram>();

    println!("p_ref    = {:.12e} Pa", p_ref.get::<pascal>());
    println!("p_back   = {:.12e} Pa", p_back.get::<pascal>());
    println!("v_target = {v_target:.15e}");
    println!("v_back   = {v_back:.15e}");
    println!(
        "dp/p     = {:.3e}",
        (p_back.get::<pascal>() - p_ref.get::<pascal>()) / p_ref.get::<pascal>()
    );
    println!("dv/v     = {:.3e}", (v_back - v_target) / v_target);

    // Probe v either side of the returned pressure to see whether the surface
    // is actually flat there, or whether the solve stopped short.
    for delta in [-1.0e-3_f64, -1.0e-5, 0.0, 1.0e-5, 1.0e-3] {
        let p = Pressure::new::<pascal>(p_back.get::<pascal>() * (1.0 + delta));
        let v = v_ph_eqm(p, h).get::<cubic_meter_per_kilogram>();
        println!(
            "  p_back*(1{delta:+.0e}) -> v = {v:.15e}, (v-vt)/vt = {:.3e}",
            (v - v_target) / v_target
        );
    }
}

/// Tabulates [`p_rho_h_conditioning`] across states spanning the regimes, so
/// the numbers quoted in its doc comment can be re-measured rather than
/// trusted.
///
/// Diagnostic only; asserts just the ordering the API promises — vapour and
/// two-phase states are well conditioned, the low-pressure subcooled liquid is
/// not.
#[test]
fn diagnose_the_conditioning_measure_across_regimes() {
    use crate::interfaces::functional_programming::rho_h_flash_eqm::p_rho_h_conditioning;

    let probes = [
        (10.0_f64, 3000.0_f64, "superheated vapour, 10 bar"),
        (1.0, 2000.0, "two-phase, 1 bar"),
        (8.0, 721.018, "saturated liquid, 8 bar"),
        (990.0, 500.0, "compressed liquid, 99 MPa"),
        (0.5, 75.5, "subcooled liquid, 0.5 bar"),
        (0.1, 75.5555, "subcooled liquid, 0.1 bar"),
    ];

    println!("{:<36} {:>14}", "state", "conditioning A");
    let mut vapour = f64::NAN;
    let mut worst_liquid: f64 = 0.0;

    for (p_bar_val, h_kj, label) in probes {
        let p = Pressure::new::<bar>(p_bar_val);
        let h = AvailableEnergy::new::<kilojoule_per_kilogram>(h_kj);
        let v = v_ph_eqm(p, h).get::<cubic_meter_per_kilogram>();
        let rho = MassDensity::new::<kilogram_per_cubic_meter>(1.0 / v);

        let a = p_rho_h_conditioning(rho, h);
        println!("{label:<36} {a:>14.3e}");

        if label.starts_with("superheated") {
            vapour = a;
        }
        if label == "subcooled liquid, 0.1 bar" {
            worst_liquid = a;
        }
    }

    assert!(
        vapour < 10.0,
        "vapour should be well conditioned: {vapour:.3e}"
    );
    assert!(
        worst_liquid > vapour * 100.0,
        "the low-pressure subcooled liquid should be flagged far worse than \
         vapour: {worst_liquid:.3e} vs {vapour:.3e}"
    );
}
