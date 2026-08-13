//! # KTA packed-bed pressure-drop correlation
//!
//! The German Kerntechnischer Ausschuss (KTA) pressure-drop correlation for
//! flow through a pebble bed, as stated (with a fully worked example) in the
//! Virtual Test Bed generic pebble-bed tutorial, step 2 (Open tier;
//! `reference-data/virtual_test_bed/doc/content/htgr/generic-pbr-tutorial/step2.md`).
//! The correlation set is:
//!
//! - `-dp/dx = psi * ((1-eps)/eps^3) * (1/(2*D_h*rho)) * (mdot/A)^2`
//! - `psi = 320/(Re/(1-eps)) + 6/(Re/(1-eps))^0.1`
//! - `Re = (mdot/A) * D_h / mu`
//!
//! where `eps` is the bed porosity, `D_h` the hydraulic diameter (for pebble
//! beds, the pebble diameter), `rho` the fluid density, `mu` the dynamic
//! viscosity, and `mdot/A` the superficial mass flux through the bed
//! cross-section `A`.
//!
//! - **Belongs here:** the KTA correlation functions and the type aliases
//!   ([`MassFlux`], [`PressureGradient`]) their signatures need, plus the
//!   tests reproducing the VTB worked example.
//! - **Does NOT belong here:** design constants ([`super::design`]), bed
//!   conductivity ([`super::zbs`]), or any solver loop.
//!
//! **Validity.** The KTA correlation is stated for packed beds of spheres at
//! porosities near the random-packing range (the HTR-10 bed has eps = 0.39)
//! and modified Reynolds numbers `Re/(1-eps)` from 1 to about 1e5; the VTB
//! worked example sits at `Re/(1-eps)` = 6.6e4. Outside that range the
//! friction factor is an extrapolation.
//!
//! **Status:** the tests below check the *correlation implementation* against
//! a published worked example. That is code verification, not validation of
//! any HTR-10 simulator.

use uom::si::f64::{Area, DynamicViscosity, Length, MassDensity, MassRate, Pressure, Ratio};
use uom::si::ratio::ratio;
use uom::si::{ISQ, Quantity, SI};
use uom::typenum::{N1, N2, P1, Z0};

/// Superficial mass flux through the bed cross-section, `mdot/A`, in
/// kilograms per square metre per second (kg m^-2 s^-1). This is the flux
/// over the *whole* bed cross-section, not the interstitial (pore) flux.
pub type MassFlux = Quantity<ISQ<N2, P1, N1, Z0, Z0, Z0, Z0>, SI<f64>, f64>;

/// Magnitude of a pressure gradient, in pascals per metre (Pa/m =
/// kg m^-2 s^-2). [`kta_pressure_gradient`] returns the magnitude of the
/// streamwise pressure *drop* per unit length, i.e. `-dp/dx > 0` for flow in
/// the `+x` direction.
pub type PressureGradient = Quantity<ISQ<N2, P1, N2, Z0, Z0, Z0, Z0>, SI<f64>, f64>;

/// Superficial mass flux `G = mdot / A` (kg m^-2 s^-1) from the bed mass flow
/// rate `mdot` (kg/s) and the bed cross-sectional area `A` (m^2). `A` is the
/// full (empty-tube) cross-section of the cylindrical flow channel, not the
/// pore area.
pub fn superficial_mass_flux(mass_flow: MassRate, flow_area: Area) -> MassFlux {
    mass_flow / flow_area
}

/// Packed-bed Reynolds number `Re = G * D_h / mu` (dimensionless), with `G`
/// the superficial mass flux (kg m^-2 s^-1), `D_h` the hydraulic diameter
/// (m; the pebble diameter for pebble beds), and `mu` the fluid dynamic
/// viscosity (Pa s). This is the plain superficial-velocity Reynolds number;
/// the KTA friction factor internally uses the modified form `Re/(1-eps)`.
pub fn packed_bed_reynolds(
    mass_flux: MassFlux,
    hydraulic_diameter: Length,
    dynamic_viscosity: DynamicViscosity,
) -> Ratio {
    mass_flux * hydraulic_diameter / dynamic_viscosity
}

/// KTA friction factor `psi = 320/(Re/(1-eps)) + 6/(Re/(1-eps))^0.1`
/// (dimensionless), with `Re` the superficial-velocity packed-bed Reynolds
/// number and `eps` the bed porosity (dimensionless, 0 < eps < 1; HTR-10
/// bed: 0.39). Source: VTB generic pebble-bed tutorial, step 2 (Open tier).
pub fn kta_friction_factor(reynolds: Ratio, porosity: Ratio) -> Ratio {
    let re_modified = reynolds.get::<ratio>() / (1.0 - porosity.get::<ratio>());
    Ratio::new::<ratio>(320.0 / re_modified + 6.0 / re_modified.powf(0.1))
}

/// KTA pressure-drop magnitude per unit bed length (Pa/m):
/// `-dp/dx = psi * ((1-eps)/eps^3) * (1/(2*D_h*rho)) * G^2`, with `G` the
/// superficial mass flux (kg m^-2 s^-1), `D_h` the hydraulic diameter (m),
/// `eps` the bed porosity (dimensionless), `rho` the fluid density (kg/m^3),
/// and `mu` the dynamic viscosity (Pa s) entering through the Reynolds
/// number. Returns the *positive* drop per length; the pressure falls in the
/// flow direction. Source: VTB generic pebble-bed tutorial, step 2 (Open
/// tier).
pub fn kta_pressure_gradient(
    mass_flux: MassFlux,
    hydraulic_diameter: Length,
    porosity: Ratio,
    density: MassDensity,
    dynamic_viscosity: DynamicViscosity,
) -> PressureGradient {
    let re = packed_bed_reynolds(mass_flux, hydraulic_diameter, dynamic_viscosity);
    let psi = kta_friction_factor(re, porosity);
    let eps = porosity.get::<ratio>();
    let geometry_factor = Ratio::new::<ratio>((1.0 - eps) / (eps * eps * eps));
    // psi * (1-eps)/eps^3 * G^2 / (2 * D_h * rho); all uom-typed, the result
    // has the dimension Pa/m by construction.
    psi * geometry_factor * mass_flux * mass_flux
        / (Ratio::new::<ratio>(2.0) * hydraulic_diameter * density)
}

/// Reference pressure drop over a bed of length `L`: `dp = (-dp/dx) * L`
/// (Pa), with the gradient magnitude from [`kta_pressure_gradient`]. Valid
/// only when the gradient is uniform over the bed (constant properties and
/// flux), as in the VTB worked example.
pub fn pressure_drop_over_bed(gradient: PressureGradient, bed_length: Length) -> Pressure {
    gradient * bed_length
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::area::square_meter;
    use uom::si::dynamic_viscosity::pascal_second;
    use uom::si::length::meter;
    use uom::si::mass_density::kilogram_per_cubic_meter;
    use uom::si::mass_rate::kilogram_per_second;
    use uom::si::pressure::kilopascal;

    /// V&V: KTA friction factor reproduces the VTB worked example.
    ///
    /// **Methodology.** The VTB generic pebble-bed tutorial, step 2 (Open
    /// tier;
    /// `reference-data/virtual_test_bed/doc/content/htgr/generic-pbr-tutorial/step2.md`)
    /// states that for Re = 40125 and porosity eps = 0.39 (from `step2.i`),
    /// psi = 1.983. The check evaluates
    /// psi = 320/(Re/(1-eps)) + 6/(Re/(1-eps))^0.1 at exactly those inputs.
    /// Pass criterion: |psi - 1.983| < 0.001 (the source gives 4 significant
    /// figures).
    ///
    /// **Results (recorded 2026-08-11).** psi = 1.98340, matching the
    /// published 1.983 to all quoted digits (residual +0.0004, +0.02%).
    #[test]
    fn kta_friction_factor_reproduces_vtb_worked_example() {
        let psi = kta_friction_factor(Ratio::new::<ratio>(40125.0), Ratio::new::<ratio>(0.39));
        println!("psi = {:.5}", psi.get::<ratio>());
        assert!(
            (psi.get::<ratio>() - 1.983).abs() < 0.001,
            "psi = {} vs published 1.983",
            psi.get::<ratio>()
        );
    }

    /// V&V: KTA pressure gradient reproduces the VTB worked example gold
    /// answer.
    ///
    /// **Methodology.** The VTB generic pebble-bed tutorial, step 2 (Open
    /// tier) works the KTA correlation at D_h = 0.06 m, eps = 0.39,
    /// rho = 8.628204 kg/m^3, mu = 1.991242e-5 Pa s, Re = 40125, and states
    /// the gold answers dp/dx = -3493 Pa/m and 34.93 kPa over the 10 m bed
    /// (Pronghorn itself computes 3.4933e4 Pa). The check takes the
    /// published Re as the flow input (G = Re * mu / D_h, the source's own
    /// definition inverted) and evaluates the full correlation chain. Pass
    /// criteria: |dp/dx| within 1 Pa/m of 3493 (source precision, 4
    /// significant figures); 10 m drop within 0.01 kPa of 34.93 kPa.
    ///
    /// **Results (recorded 2026-08-11).** G = 13.31643 kg m^-2 s^-1,
    /// |dp/dx| = 3493.17 Pa/m, drop over 10 m = 34.9317 kPa — the published
    /// gold answers (3493 Pa/m; 34.93 kPa, Pronghorn 3.4933e4 Pa) are
    /// reproduced to all quoted digits. This is the exact checked-in
    /// reference number this V&V record is anchored on.
    #[test]
    fn kta_pressure_gradient_reproduces_vtb_worked_example() {
        let d_h = Length::new::<meter>(0.06);
        let mu = DynamicViscosity::new::<pascal_second>(1.991242e-5);
        let rho = MassDensity::new::<kilogram_per_cubic_meter>(8.628204);
        let eps = Ratio::new::<ratio>(0.39);

        // G from the published Re: G = Re * mu / D_h (the source's Reynolds
        // definition, inverted).
        let g: MassFlux = Ratio::new::<ratio>(40125.0) * mu / d_h;

        // The Reynolds number recomputed from that flux must round-trip.
        let re = packed_bed_reynolds(g, d_h, mu);
        assert!((re.get::<ratio>() - 40125.0).abs() < 1e-6);

        let gradient = kta_pressure_gradient(g, d_h, eps, rho, mu);
        let drop = pressure_drop_over_bed(gradient, Length::new::<meter>(10.0));
        println!(
            "G = {:.5} kg/(m^2 s); |dp/dx| = {:.2} Pa/m; drop over 10 m = {:.4} kPa",
            g.value,
            gradient.value,
            drop.get::<kilopascal>()
        );
        assert!(
            (gradient.value - 3493.0).abs() < 1.0,
            "|dp/dx| = {} Pa/m vs published 3493 Pa/m",
            gradient.value
        );
        assert!(
            (drop.get::<kilopascal>() - 34.93).abs() < 0.01,
            "drop = {} kPa vs published 34.93 kPa",
            drop.get::<kilopascal>()
        );
        // The pressure must FALL along the flow: the returned magnitude is
        // positive by construction.
        assert!(gradient.value > 0.0);
    }

    /// V&V (documented source inconsistency): Reynolds number from the VTB
    /// tutorial's stated mass flow.
    ///
    /// **Methodology.** The VTB tutorial's input file `step2.i` (Open tier;
    /// `reference-data/virtual_test_bed/htgr/generic-pbr-tutorial/step2.i`)
    /// sets `mass_flow_rate = 60.0` kg/s, and the tutorial text gives
    /// A = 4.523893 m^2, D_h = 0.06 m, mu = 1.991242e-5 Pa s, and states
    /// Re = 40125. The check computes Re = (mdot/A) * D_h / mu from those
    /// inputs. Pass criterion: within 0.5% of the published 40125 — NOT
    /// exact, see below.
    ///
    /// **Results (recorded 2026-08-11).** Re = 39963.7, which is 0.40%
    /// BELOW the published 40125. The tutorial's own chain is internally
    /// inconsistent at the 0.4% level: its rho-bar and mu-bar are
    /// postprocessor averages over the converged simulation, and its inlet
    /// velocity boundary condition is set with a separate constant density,
    /// so the simulation's actual mass flux differs slightly from
    /// 60.0/4.523893. Driving the correlation from the published Re instead
    /// (see `kta_pressure_gradient_reproduces_vtb_worked_example`)
    /// reproduces every downstream gold number exactly, so the correlation
    /// implementation is verified; this test documents the 0.4% source-side
    /// discrepancy honestly rather than hiding it in a loose tolerance.
    #[test]
    fn reynolds_from_tutorial_mass_flow_documents_source_inconsistency() {
        let g = superficial_mass_flux(
            MassRate::new::<kilogram_per_second>(60.0),
            Area::new::<square_meter>(4.523893),
        );
        let re = packed_bed_reynolds(
            g,
            Length::new::<meter>(0.06),
            DynamicViscosity::new::<pascal_second>(1.991242e-5),
        );
        let rel = (re.get::<ratio>() - 40125.0) / 40125.0;
        println!(
            "Re = {:.1}; relative to published 40125: {:+.3}%",
            re.get::<ratio>(),
            rel * 100.0
        );
        // Documented discrepancy: -0.40%. Bound it at 0.5% so a transcription
        // error (wrong area, wrong viscosity) still fails loudly.
        assert!(rel.abs() < 0.005);
        // And assert the discrepancy IS there — if a future source revision
        // makes the chain exact, this test should be revisited, not silently
        // pass with a stale doc comment.
        assert!(rel < -0.001);
    }
}
