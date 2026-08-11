//! # KTA 3102.3 packed-bed (pebble-bed) pressure drop
//!
//! The friction side of a pebble bed: the pressure gradient a gas coolant
//! sees flowing through a randomly packed bed of monosized spheres.
//!
//! ## What belongs here / what does not
//!
//! - **Belongs:** the KTA-form packed-bed friction factor and the pressure
//!   gradient / total drop built from it, plus the bed geometry those need.
//! - **Does NOT belong:** the bed's *conduction* physics — the
//!   Zehner-Bauer-Schlunder effective conductivity lives in
//!   [`crate::pebble_bed::zbs`]. Nor particle-to-fluid heat transfer
//!   (Wakao), which is still unimplemented anywhere in the workspace.
//!
//! ## Formulation
//!
//! With `eps` the bed porosity, `G = mdot/A` the **superficial** mass flux
//! (referred to the empty bed cross-section), `D_h` the pebble diameter,
//! `rho` the gas density and `mu` its dynamic viscosity:
//!
//! ```text
//! Re      = G * D_h / mu                      (superficial Reynolds number)
//! Re_mod  = Re / (1 - eps)                    (modified Reynolds number)
//! psi     = 320 / Re_mod + 6 / Re_mod^0.1     (KTA friction factor)
//! -dp/dx  = psi * (1 - eps)/eps^3 * G^2 / (2 * D_h * rho)
//! dp      = (-dp/dx) * L
//! ```
//!
//! The `320` coefficient sits on the **linear (viscous)** term and the `6`
//! on the weakly Reynolds-dependent **inertial** term. Note carefully that
//! [`packed_bed_reynolds`] returns the *plain superficial* `Re`; the
//! `1/(1-eps)` modification happens **inside** [`kta_friction_factor`].
//! Passing an already-modified Reynolds number in is the obvious way to get
//! this wrong.
//!
//! The returned gradient is a **positive magnitude** (the drop), not a
//! signed derivative.
//!
//! ## Validity
//!
//! Randomly packed monosized spheres near random-packing porosity (the
//! HTR-10 bed is `eps = 0.39`), for modified Reynolds numbers `Re/(1-eps)`
//! from about 1 to 1e5. The VTB worked example below sits at
//! `Re/(1-eps) = 6.6e4`, near the top of that band. The correlation is a
//! **steady, incompressible-within-a-slice** friction closure: for a long
//! bed with significant gas expansion, march it in slices with properties
//! re-evaluated per slice rather than applying it once with a single mean
//! density.
//!
//! ## Provenance, and why this is a reimplementation
//!
//! The workspace already carries a verified implementation of this
//! correlation in **`crates/outram-park-digital-twin-engine/src/htr10/kta.rs`**
//! (read on 2026-08-11). TAMPINES **cannot depend on that crate** — it is a
//! downstream GUI/digital-twin crate, so the edge would invert the
//! dependency graph. This module is therefore a deliberate, faithful
//! **reimplementation** of the same formulation, verified against the same
//! gold values, rather than a shared type. The two are independent code
//! paths that must agree.
//!
//! The upstream implementation's stated source is the **Virtual Test Bed
//! generic pebble-bed tutorial, step 2** (Open tier;
//! `reference-data/virtual_test_bed/doc/content/htgr/generic-pbr-tutorial/step2.md`,
//! with the porosity taken from `step2.i`). That is the equation set
//! transcribed above. **Honesty note:** neither that module nor this one
//! was written with page-level access to the KTA 3102.3 standard itself —
//! the "KTA" name is carried over from how the VTB tutorial and the
//! pebble-bed literature label this `320/6` form. A human should confirm
//! the coefficients against the standard before this module is promoted
//! past Prototype in the V&V pipeline.
//!
//! ## Verification & Validation
//!
//! **Methodology.** Reproduce the VTB generic pebble-bed tutorial step-2
//! worked example, whose published inputs are `Re = 40125`, `eps = 0.39`,
//! `D_h = 0.06 m`, `mu = 1.991242e-5 Pa s`, `rho = 8.628204 kg/m^3`, over a
//! `10 m` bed, and whose published outputs are the friction factor
//! `psi = 1.983` and the pressure gradient `3493 Pa/m` (Pronghorn itself
//! reports a drop of `3.4933e4 Pa`). Pass criterion: friction factor within
//! `0.001`, gradient within `1 Pa/m`, drop within `0.01 kPa` — the
//! resolution of the published digits.
//!
//! **Results.** See the `#[test]` doc comments in this module's `tests`
//! submodule for the numbers this implementation actually produced on
//! 2026-08-11, measured by running the tests.
//!
//! ## Status
//!
//! **Verified against the VTB gold values.** Not validated against HTR-10
//! measurements — no comparison against plant or experimental data has been
//! made, and none is claimed. AI-assisted draft pending human review per
//! `RESPONSIBLE_USE.md`.

use super::{MassFlux, PressureGradient};
use crate::gas_phase::properties::helium_state;
use crate::TampinesError;
use uom::si::area::square_meter;
use uom::si::f64::{
    Area, DynamicViscosity, Length, MassDensity, MassRate, Pressure, Ratio,
    ThermodynamicTemperature,
};
use uom::si::length::meter;
use uom::si::ratio::ratio;

/// Superficial mass flux `G = mdot / A`, kg/(m^2 s), referred to the
/// **empty** bed cross-section (not the free-flow area between pebbles).
///
/// The KTA correlation is defined on the superficial flux; dividing by the
/// porous free area instead inflates `G` by `1/eps` and the gradient by
/// `1/eps^2`.
pub fn superficial_mass_flux(mass_flow: MassRate, bed_cross_section: Area) -> MassFlux {
    mass_flow / bed_cross_section
}

/// Superficial packed-bed Reynolds number `Re = G D_h / mu`, dimensionless.
///
/// This is the **plain** superficial `Re`. [`kta_friction_factor`] applies
/// the `1/(1-eps)` modification itself — do not pre-divide.
pub fn packed_bed_reynolds(
    mass_flux: MassFlux,
    pebble_diameter: Length,
    dynamic_viscosity: DynamicViscosity,
) -> Ratio {
    mass_flux * pebble_diameter / dynamic_viscosity
}

/// KTA packed-bed friction factor
/// `psi = 320/Re_mod + 6/Re_mod^0.1`, dimensionless, with
/// `Re_mod = Re/(1 - eps)`.
///
/// `reynolds` is the **superficial** Reynolds number from
/// [`packed_bed_reynolds`]; `porosity` is the bed void fraction, strictly
/// between 0 and 1.
///
/// Errors with [`TampinesError::InvalidInput`] for a porosity outside
/// `(0, 1)` or a non-positive / non-finite Reynolds number (the `320/Re`
/// term diverges at `Re = 0`; a stagnant bed has no friction gradient to
/// report, so the caller must handle that case rather than receive an
/// infinity).
pub fn kta_friction_factor(reynolds: Ratio, porosity: Ratio) -> Result<Ratio, TampinesError> {
    let eps = porosity.get::<ratio>();
    let re = reynolds.get::<ratio>();
    if !(0.0..1.0).contains(&eps) || eps <= 0.0 {
        return Err(TampinesError::InvalidInput(format!(
            "KTA friction factor: porosity {eps} must lie strictly in (0, 1)"
        )));
    }
    if !re.is_finite() || re <= 0.0 {
        return Err(TampinesError::InvalidInput(format!(
            "KTA friction factor: Reynolds number {re} must be finite and positive"
        )));
    }
    let re_modified = re / (1.0 - eps);
    Ok(Ratio::new::<ratio>(
        320.0 / re_modified + 6.0 / re_modified.powf(0.1),
    ))
}

/// A packed bed of monosized spheres, described by the three parameters the
/// KTA pressure-drop correlation needs. Plain data; the physics lives in
/// [`KtaBed::pressure_gradient`] and [`KtaBed::pressure_drop`].
///
/// Construct with [`KtaBed::new`], or [`KtaBed::htr10`] for the cited
/// HTR-10 core.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KtaBed {
    /// Bed porosity (void fraction), dimensionless, strictly in `(0, 1)`.
    /// Random close-packed sphere beds sit near 0.36-0.42; the HTR-10 bed
    /// is 0.39 (filling fraction 0.61, IAEA HTR-10 benchmark description,
    /// Open tier).
    pub porosity: Ratio,
    /// Pebble (sphere) diameter, metres — the correlation's `D_h`.
    /// HTR-10: 0.06 m.
    pub pebble_diameter: Length,
    /// Bed cross-sectional area, m^2, referred to the **empty** bed (the
    /// full core barrel bore, not the free area between pebbles). HTR-10's
    /// 1.8 m core diameter gives about 2.545 m^2.
    pub cross_section: Area,
}

/// Everything [`KtaBed::pressure_gradient_detailed`] computed, so a caller
/// can inspect the intermediate dimensionless groups rather than trust a
/// bare pressure number.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KtaBedResult {
    /// Superficial mass flux `G = mdot/A`, kg/(m^2 s).
    pub mass_flux: MassFlux,
    /// Superficial Reynolds number `Re = G D_h / mu`, dimensionless.
    pub reynolds: Ratio,
    /// Modified Reynolds number `Re/(1 - eps)`, dimensionless — the group
    /// the correlation's stated 1 to 1e5 validity band is quoted on.
    pub modified_reynolds: Ratio,
    /// KTA friction factor `psi`, dimensionless.
    pub friction_factor: Ratio,
    /// Pressure-drop magnitude per unit bed length, Pa/m (positive).
    pub pressure_gradient: PressureGradient,
}

impl KtaBed {
    /// A bed of the given porosity (dimensionless, in `(0,1)`), pebble
    /// diameter (metres) and empty-bed cross-sectional area (m^2).
    pub fn new(porosity: Ratio, pebble_diameter: Length, cross_section: Area) -> Self {
        Self {
            porosity,
            pebble_diameter,
            cross_section,
        }
    }

    /// The HTR-10 pebble-bed core: porosity 0.39 (filling fraction 0.61)
    /// and pebble diameter 6.0 cm from the IAEA HTR-10 benchmark
    /// description (Open tier), with the cross-section taken as the full
    /// bore of the 1.8 m core diameter, `pi/4 * 1.8^2`.
    ///
    /// Note this is the *geometric* core bore; it makes no allowance for
    /// the ~10 % control-rod/reflector bypass flow the HTR-10 design point
    /// carries. A caller modelling bypass should scale the mass flow, not
    /// this area.
    pub fn htr10() -> Self {
        let d_core = 1.8_f64;
        Self::new(
            Ratio::new::<ratio>(0.39),
            Length::new::<meter>(0.06),
            Area::new::<square_meter>(std::f64::consts::FRAC_PI_4 * d_core * d_core),
        )
    }

    /// Free (open) flow area between the pebbles, `eps * A`, m^2.
    ///
    /// Not used by the friction correlation — which is defined on the
    /// superficial flux — but it is what an interstitial *velocity* (and
    /// hence a Mach number) must be formed from.
    pub fn free_flow_area(&self) -> Area {
        self.cross_section * self.porosity
    }

    /// Full KTA evaluation at a given mass flow and gas state, returning
    /// every intermediate group (see [`KtaBedResult`]).
    ///
    /// `mass_flow` is the total flow through the bed, kg/s; `density` and
    /// `dynamic_viscosity` are the gas properties at whatever mean state
    /// the caller judges appropriate (for a long bed with strong heating,
    /// march in slices instead — see the module docs).
    ///
    /// Errors with [`TampinesError::InvalidInput`] for a non-positive
    /// geometry or gas property, or via [`kta_friction_factor`] for an
    /// out-of-range porosity or Reynolds number.
    pub fn pressure_gradient_detailed(
        &self,
        mass_flow: MassRate,
        density: MassDensity,
        dynamic_viscosity: DynamicViscosity,
    ) -> Result<KtaBedResult, TampinesError> {
        use uom::si::dynamic_viscosity::pascal_second;
        use uom::si::mass_density::kilogram_per_cubic_meter;

        let d_h = self.pebble_diameter.get::<meter>();
        let a = self.cross_section.get::<square_meter>();
        let rho = density.get::<kilogram_per_cubic_meter>();
        let mu = dynamic_viscosity.get::<pascal_second>();
        for (name, v) in [
            ("pebble diameter", d_h),
            ("cross-section", a),
            ("density", rho),
            ("dynamic viscosity", mu),
        ] {
            if !v.is_finite() || v <= 0.0 {
                return Err(TampinesError::InvalidInput(format!(
                    "KTA bed: {name} must be finite and positive, got {v}"
                )));
            }
        }

        let g = superficial_mass_flux(mass_flow, self.cross_section);
        let re = packed_bed_reynolds(g, self.pebble_diameter, dynamic_viscosity);
        let psi = kta_friction_factor(re, self.porosity)?;
        let eps = self.porosity.get::<ratio>();
        let geometry_factor = Ratio::new::<ratio>((1.0 - eps) / (eps * eps * eps));

        // psi * (1-eps)/eps^3 * G^2 / (2 D_h rho); uom carries the algebra,
        // so the result is Pa/m by construction.
        let gradient: PressureGradient = psi * geometry_factor * g * g
            / (Ratio::new::<ratio>(2.0) * self.pebble_diameter * density);

        Ok(KtaBedResult {
            mass_flux: g,
            reynolds: re,
            modified_reynolds: Ratio::new::<ratio>(re.get::<ratio>() / (1.0 - eps)),
            friction_factor: psi,
            pressure_gradient: gradient,
        })
    }

    /// Pressure-drop magnitude per unit bed length, Pa/m (positive). Thin
    /// wrapper over [`Self::pressure_gradient_detailed`].
    pub fn pressure_gradient(
        &self,
        mass_flow: MassRate,
        density: MassDensity,
        dynamic_viscosity: DynamicViscosity,
    ) -> Result<PressureGradient, TampinesError> {
        Ok(self
            .pressure_gradient_detailed(mass_flow, density, dynamic_viscosity)?
            .pressure_gradient)
    }

    /// Total pressure drop across a bed of height `bed_height`, Pa
    /// (positive magnitude).
    ///
    /// Applies the gradient uniformly over the height, so it assumes the
    /// gas properties supplied are representative of the whole bed. For the
    /// HTR-10 core, where the gas roughly halves in density top to bottom,
    /// prefer [`Self::pressure_drop_helium_marched`].
    pub fn pressure_drop(
        &self,
        mass_flow: MassRate,
        density: MassDensity,
        dynamic_viscosity: DynamicViscosity,
        bed_height: Length,
    ) -> Result<Pressure, TampinesError> {
        Ok(self.pressure_gradient(mass_flow, density, dynamic_viscosity)? * bed_height)
    }

    /// Total helium pressure drop across a heated bed, marched in
    /// `n_slices` equal-height slices with properties re-evaluated from
    /// [`helium_state`] at each slice's mean temperature.
    ///
    /// The gas temperature is assumed to rise **linearly** with height from
    /// `inlet_temperature` to `outlet_temperature` — a first-cut axial
    /// power shape, not a solved energy balance. Pressure is held at
    /// `pressure` for the property evaluation, which is accurate to the
    /// extent that the total drop is small against the system pressure
    /// (for HTR-10 it is: tens of kPa against 3 MPa).
    ///
    /// This is the recommended entry point for the HTR-10 core, because a
    /// single-mean-density evaluation misrepresents a bed whose gas density
    /// changes by roughly a factor of two.
    ///
    /// Errors with [`TampinesError::InvalidInput`] for `n_slices == 0` or a
    /// non-positive bed height, and propagates [`helium_state`]'s errors.
    pub fn pressure_drop_helium_marched(
        &self,
        mass_flow: MassRate,
        pressure: Pressure,
        inlet_temperature: ThermodynamicTemperature,
        outlet_temperature: ThermodynamicTemperature,
        bed_height: Length,
        n_slices: usize,
    ) -> Result<Pressure, TampinesError> {
        use uom::si::pressure::pascal;
        use uom::si::thermodynamic_temperature::kelvin;

        if n_slices == 0 {
            return Err(TampinesError::InvalidInput(
                "KTA marched bed: n_slices must be at least 1".to_string(),
            ));
        }
        if !bed_height.get::<meter>().is_finite() || bed_height.get::<meter>() <= 0.0 {
            return Err(TampinesError::InvalidInput(format!(
                "KTA marched bed: bed height must be finite and positive, got {} m",
                bed_height.get::<meter>()
            )));
        }

        let t_in = inlet_temperature.get::<kelvin>();
        let t_out = outlet_temperature.get::<kelvin>();
        let dz = bed_height / (n_slices as f64);
        let mut total = Pressure::new::<pascal>(0.0);

        for i in 0..n_slices {
            // Mid-slice temperature on the assumed linear axial profile.
            let frac = (i as f64 + 0.5) / (n_slices as f64);
            let t_mid = ThermodynamicTemperature::new::<kelvin>(t_in + (t_out - t_in) * frac);
            let state = helium_state(t_mid, pressure)?;
            total += self.pressure_drop(mass_flow, state.density, state.dynamic_viscosity, dz)?;
        }
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gas_phase::properties::htr10_design_point;
    use uom::si::dynamic_viscosity::pascal_second;
    use uom::si::mass_density::kilogram_per_cubic_meter;
    use uom::si::mass_rate::kilogram_per_second;
    use uom::si::pressure::{kilopascal, pascal};
    use uom::si::thermodynamic_temperature::kelvin;

    /// VTB worked-example inputs, transcribed on 2026-08-11 from the
    /// verified implementation in
    /// `crates/outram-park-digital-twin-engine/src/htr10/kta.rs`, whose
    /// stated source is the Virtual Test Bed generic pebble-bed tutorial
    /// step 2 (Open tier).
    const VTB_REYNOLDS: f64 = 40125.0;
    /// VTB worked-example bed porosity (from the tutorial's `step2.i`).
    const VTB_POROSITY: f64 = 0.39;
    /// VTB worked-example pebble diameter, m.
    const VTB_D_H_M: f64 = 0.06;
    /// VTB worked-example helium dynamic viscosity, Pa s.
    const VTB_MU_PA_S: f64 = 1.991242e-5;
    /// VTB worked-example helium density, kg/m^3.
    const VTB_RHO_KG_M3: f64 = 8.628204;
    /// VTB worked-example bed height, m.
    const VTB_BED_LENGTH_M: f64 = 10.0;
    /// VTB published friction factor, dimensionless.
    const VTB_PSI: f64 = 1.983;
    /// VTB published pressure gradient, Pa/m.
    const VTB_GRADIENT_PA_PER_M: f64 = 3493.0;
    /// VTB published total drop over the 10 m bed, kPa
    /// (Pronghorn reports 3.4933e4 Pa).
    const VTB_DROP_KPA: f64 = 34.93;

    /// V&V — KTA friction factor against the VTB gold value.
    ///
    /// **Methodology.** Evaluate `psi` at the published `Re = 40125`,
    /// `eps = 0.39`. Pass criterion `|psi - 1.983| < 0.001`, the resolution
    /// of the published digits.
    ///
    /// **Result (2026-08-11, this implementation):** `psi = 1.983395`,
    /// residual `+0.000395` (`+0.0199 %`). PASSES — the residual is
    /// entirely the rounding of the published four-digit `1.983`.
    #[test]
    fn kta_friction_factor_reproduces_vtb_gold_value() {
        let psi = kta_friction_factor(
            Ratio::new::<ratio>(VTB_REYNOLDS),
            Ratio::new::<ratio>(VTB_POROSITY),
        )
        .unwrap()
        .get::<ratio>();
        println!(
            "psi = {psi:.6}  (VTB {VTB_PSI})  residual {:+.6} ({:+.4} %)",
            psi - VTB_PSI,
            100.0 * (psi - VTB_PSI) / VTB_PSI
        );
        assert!(
            (psi - VTB_PSI).abs() < 0.001,
            "friction factor {psi} vs VTB {VTB_PSI}"
        );
    }

    /// V&V — KTA pressure gradient and total drop against the VTB gold
    /// values.
    ///
    /// **Methodology.** Back out the mass flux from the published Reynolds
    /// number (`G = Re mu / D_h`) so the comparison is driven by the
    /// tutorial's own published `Re` rather than by its mass-flow input,
    /// which is known to be mutually inconsistent with it at the 0.4 %
    /// level (see [`vtb_mass_flow_reynolds_inconsistency_is_reproduced`]).
    /// Then evaluate the gradient at `eps = 0.39`, `D_h = 0.06 m`,
    /// `rho = 8.628204 kg/m^3`, `mu = 1.991242e-5 Pa s`, and the drop over
    /// a 10 m bed. Pass criteria: gradient within `1 Pa/m` of `3493 Pa/m`,
    /// drop within `0.01 kPa` of `34.93 kPa`.
    ///
    /// **Result (2026-08-11, this implementation):**
    /// `G = 13.316431 kg/(m^2 s)`, `Re = 40125.000` (round-trip exact),
    /// `Re_mod = 65778.7`, `psi = 1.983395`,
    /// `|dp/dx| = 3493.167 Pa/m` (residual `+0.167 Pa/m`, `+0.0048 %`),
    /// drop `= 34.93167 kPa` (residual `+0.00167 kPa`). Both PASS.
    ///
    /// **Agreement with the reference implementation.** The read-only
    /// `outram-park-digital-twin-engine/src/htr10/kta.rs` records
    /// `G = 13.31643`, `|dp/dx| = 3493.17 Pa/m`, drop `= 34.9317 kPa` for
    /// the same case. This reimplementation reproduces every digit that
    /// module records, and agrees with the VTB published values to
    /// `+0.0048 %` on the gradient — i.e. to the resolution of the
    /// published four-digit figure.
    #[test]
    fn kta_pressure_gradient_reproduces_vtb_gold_values() {
        let d_h = Length::new::<meter>(VTB_D_H_M);
        let mu = DynamicViscosity::new::<pascal_second>(VTB_MU_PA_S);
        let rho = MassDensity::new::<kilogram_per_cubic_meter>(VTB_RHO_KG_M3);

        // G = Re * mu / D_h, from the published Reynolds number.
        let g: MassFlux = Ratio::new::<ratio>(VTB_REYNOLDS) * mu / d_h;

        // Use a unit cross-section so mass_flow numerically equals G.
        let bed = KtaBed::new(
            Ratio::new::<ratio>(VTB_POROSITY),
            d_h,
            Area::new::<square_meter>(1.0),
        );
        let mass_flow = MassRate::new::<kilogram_per_second>(
            g.value, // kg/(m^2 s) through 1 m^2 == kg/s
        );

        let out = bed.pressure_gradient_detailed(mass_flow, rho, mu).unwrap();

        // Round-trip: the Reynolds number must come back out.
        let re = out.reynolds.get::<ratio>();
        assert!(
            (re - VTB_REYNOLDS).abs() < 1e-6,
            "Re round-trip {re} vs {VTB_REYNOLDS}"
        );

        let grad = out.pressure_gradient.value; // Pa/m
        let drop =
            (out.pressure_gradient * Length::new::<meter>(VTB_BED_LENGTH_M)).get::<kilopascal>();
        println!(
            "G = {:.6} kg/(m^2 s)  Re = {:.3}  Re_mod = {:.1}  psi = {:.6}\n\
             |dp/dx| = {grad:.3} Pa/m  (VTB {VTB_GRADIENT_PA_PER_M}) residual {:+.3}\n\
             drop = {drop:.5} kPa  (VTB {VTB_DROP_KPA}) residual {:+.5}",
            g.value,
            re,
            out.modified_reynolds.get::<ratio>(),
            out.friction_factor.get::<ratio>(),
            grad - VTB_GRADIENT_PA_PER_M,
            drop - VTB_DROP_KPA,
        );

        assert!(grad > 0.0, "gradient must be a positive magnitude");
        assert!(
            (grad - VTB_GRADIENT_PA_PER_M).abs() < 1.0,
            "gradient {grad} Pa/m vs VTB {VTB_GRADIENT_PA_PER_M} Pa/m"
        );
        assert!(
            (drop - VTB_DROP_KPA).abs() < 0.01,
            "drop {drop} kPa vs VTB {VTB_DROP_KPA} kPa"
        );
    }

    /// V&V — documents a known internal inconsistency in the VTB source,
    /// mirroring the same recorded finding in
    /// `outram-park-digital-twin-engine/src/htr10/kta.rs`.
    ///
    /// **Methodology.** The tutorial's `step2.i` gives
    /// `mass_flow_rate = 60.0 kg/s` over a bed area of `4.523893 m^2`.
    /// Forming `Re` from *those* inputs rather than from the published
    /// `Re = 40125` should disagree slightly — the tutorial's `rho`/`mu`
    /// are postprocessor averages over the converged run while its inlet
    /// velocity boundary condition uses a separate constant density. This
    /// test **asserts the discrepancy exists** (it is a negative test): the
    /// relative difference must be within 0.5 % but strictly below
    /// -0.1 %.
    ///
    /// **Result (2026-08-11, this implementation):** `Re = 39963.74`,
    /// relative difference `-0.4019 %` against the published `40125`.
    /// PASSES. Consequence: drive this correlation from the published
    /// Reynolds number, not from `60.0 / 4.523893`.
    #[test]
    fn vtb_mass_flow_reynolds_inconsistency_is_reproduced() {
        let mdot = MassRate::new::<kilogram_per_second>(60.0);
        let area = Area::new::<square_meter>(4.523893);
        let g = superficial_mass_flux(mdot, area);
        let re = packed_bed_reynolds(
            g,
            Length::new::<meter>(VTB_D_H_M),
            DynamicViscosity::new::<pascal_second>(VTB_MU_PA_S),
        )
        .get::<ratio>();
        let rel = (re - VTB_REYNOLDS) / VTB_REYNOLDS;
        println!(
            "Re from tutorial mass flow = {re:.2}  rel = {:+.4} %",
            100.0 * rel
        );
        assert!(rel.abs() < 0.005, "discrepancy larger than 0.5 %: {rel}");
        assert!(
            rel < -0.001,
            "expected the documented ~-0.4 % shortfall, got {rel}"
        );
    }

    /// Analytic limit — at large modified Reynolds number the friction
    /// factor must approach the pure inertial term `6 Re_mod^-0.1`, and at
    /// small `Re_mod` the viscous term `320/Re_mod` must dominate.
    #[test]
    fn friction_factor_has_the_right_asymptotes() {
        let eps = Ratio::new::<ratio>(0.39);
        // Viscous end: Re_mod = 1 -> psi ~ 320 + 6 = 326.
        let low = kta_friction_factor(Ratio::new::<ratio>(1.0 - 0.39), eps)
            .unwrap()
            .get::<ratio>();
        assert!((low - 326.0).abs() < 1e-9, "low-Re psi = {low}");
        // Inertial end: at Re_mod = 1e5 the viscous term is 3.2e-3, tiny
        // against 6 * (1e5)^-0.1 = 6 * 0.316... .
        let re_mod = 1.0e5;
        let high = kta_friction_factor(Ratio::new::<ratio>(re_mod * (1.0 - 0.39)), eps)
            .unwrap()
            .get::<ratio>();
        let inertial_only = 6.0 / re_mod.powf(0.1);
        assert!(
            (high - inertial_only).abs() / inertial_only < 2e-3,
            "high-Re psi = {high}, inertial-only = {inertial_only}"
        );
    }

    /// Guard behaviour: out-of-range porosity and Reynolds number are
    /// rejected rather than returning an infinity or a NaN.
    #[test]
    fn out_of_range_inputs_are_rejected() {
        let re = Ratio::new::<ratio>(1000.0);
        assert!(kta_friction_factor(re, Ratio::new::<ratio>(0.0)).is_err());
        assert!(kta_friction_factor(re, Ratio::new::<ratio>(1.0)).is_err());
        assert!(kta_friction_factor(re, Ratio::new::<ratio>(-0.1)).is_err());
        assert!(kta_friction_factor(Ratio::new::<ratio>(0.0), Ratio::new::<ratio>(0.39)).is_err());
        assert!(
            kta_friction_factor(Ratio::new::<ratio>(f64::NAN), Ratio::new::<ratio>(0.39)).is_err()
        );
    }

    /// Print the HTR-10 core pressure drop at the design point, both with a
    /// single mean-temperature property evaluation and marched in slices.
    /// Measurement harness for the V&V numbers recorded in
    /// [`htr10_core_pressure_drop_is_a_small_fraction_of_system_pressure`].
    #[test]
    fn measure_htr10_core_pressure_drop() {
        let bed = KtaBed::htr10();
        let p = htr10_design_point::pressure();
        let mdot = htr10_design_point::mass_flow_rate();
        let t_in = htr10_design_point::core_inlet_temperature();
        let t_out = htr10_design_point::core_outlet_temperature();
        let height = Length::new::<meter>(1.97); // IAEA HTR-10: 197 cm average core height

        let t_mean = ThermodynamicTemperature::new::<kelvin>(
            0.5 * (t_in.get::<kelvin>() + t_out.get::<kelvin>()),
        );
        let s = helium_state(t_mean, p).unwrap();
        let single = bed
            .pressure_drop(mdot, s.density, s.dynamic_viscosity, height)
            .unwrap()
            .get::<pascal>();
        let detail = bed
            .pressure_gradient_detailed(mdot, s.density, s.dynamic_viscosity)
            .unwrap();

        println!(
            "HTR-10 core, single mean state (T = {:.2} K):\n  \
             A = {:.6} m^2  G = {:.6} kg/(m^2 s)  Re = {:.1}  Re_mod = {:.1}  psi = {:.6}\n  \
             |dp/dx| = {:.4} Pa/m  drop over {:.2} m = {single:.3} Pa",
            t_mean.get::<kelvin>(),
            bed.cross_section.get::<square_meter>(),
            detail.mass_flux.value,
            detail.reynolds.get::<ratio>(),
            detail.modified_reynolds.get::<ratio>(),
            detail.friction_factor.get::<ratio>(),
            detail.pressure_gradient.value,
            height.get::<meter>(),
        );

        for n in [1usize, 2, 4, 16, 64, 256] {
            let marched = bed
                .pressure_drop_helium_marched(mdot, p, t_in, t_out, height, n)
                .unwrap()
                .get::<pascal>();
            println!("  marched n = {n:4}: drop = {marched:.4} Pa");
        }
    }

    /// V&V — HTR-10 core pressure drop is physically plausible and the
    /// marched integration converges.
    ///
    /// **Methodology.** Evaluate the KTA drop across the HTR-10 core
    /// (`eps = 0.39`, `d_p = 6 cm`, bore `pi/4 * 1.8^2 m^2`, average height
    /// 197 cm — IAEA HTR-10 benchmark description, Open tier) at the design
    /// point (helium, 3.0 MPa, 4.3 kg/s, 523.15 K to 973.15 K), with
    /// helium properties from [`helium_state`]. Checks: (a) the modified
    /// Reynolds number lands inside the correlation's stated 1 to 1e5
    /// validity band; (b) the drop is a small fraction of the 3.0 MPa
    /// system pressure, so the constant-pressure property evaluation used
    /// inside the march is self-consistent; (c) the slice march converges
    /// — the 64-slice and 256-slice answers agree to better than 0.1 %.
    ///
    /// **Results (2026-08-11, this implementation).** Cross-section
    /// `2.544690 m^2`, `G = 1.689793 kg/(m^2 s)`; at the 748.15 K mean
    /// state `Re = 2692.7`, `Re_mod = 4414.2` (inside the 1-1e5 validity
    /// band), `psi = 2.664680`, `|dp/dx| = 339.4330 Pa/m`, single-state
    /// drop `668.683 Pa` over the 1.97 m bed. Marched: `n = 1` gives
    /// `668.6830 Pa`, `n = 2` `669.4981`, `n = 4` `669.7046`, `n = 16`
    /// `669.7694`, `n = 64` `669.7735`, `n = 256` `669.7737 Pa` —
    /// converged to `3.0e-5 %` between the last two. The converged drop is
    /// `0.0223 %` of the 3.0 MPa system pressure. All three criteria PASS.
    ///
    /// **Interpretation.** The HTR-10 core is a low-resistance bed at the
    /// design flow — under 700 Pa across the whole core, so the circulator
    /// head is set by the rest of the loop, not by the bed. The
    /// single-mean-state estimate understates the converged drop by only
    /// `0.16 %`, much less than one might expect from a 46 % density
    /// change, because the KTA gradient's `G^2/rho` and its `psi(Re(mu))`
    /// dependence partly cancel as the gas heats. Note the flow sits at
    /// `Re_mod ~ 4.4e3`, an order of magnitude below the VTB example's
    /// `6.6e4`, so the *viscous* `320/Re_mod` term contributes far more
    /// here than in the gold-value case — a regime the gold value does not
    /// exercise. No comparison against HTR-10 measurements has been made
    /// and none is claimed.
    #[test]
    fn htr10_core_pressure_drop_is_a_small_fraction_of_system_pressure() {
        let bed = KtaBed::htr10();
        let p = htr10_design_point::pressure();
        let mdot = htr10_design_point::mass_flow_rate();
        let t_in = htr10_design_point::core_inlet_temperature();
        let t_out = htr10_design_point::core_outlet_temperature();
        let height = Length::new::<meter>(1.97);

        let t_mean = ThermodynamicTemperature::new::<kelvin>(
            0.5 * (t_in.get::<kelvin>() + t_out.get::<kelvin>()),
        );
        let s = helium_state(t_mean, p).unwrap();
        let detail = bed
            .pressure_gradient_detailed(mdot, s.density, s.dynamic_viscosity)
            .unwrap();

        // (a) inside the stated validity band.
        let re_mod = detail.modified_reynolds.get::<ratio>();
        assert!(
            (1.0..=1.0e5).contains(&re_mod),
            "Re_mod = {re_mod} outside the correlation's 1..1e5 validity band"
        );

        // (c) march convergence.
        let d64 = bed
            .pressure_drop_helium_marched(mdot, p, t_in, t_out, height, 64)
            .unwrap()
            .get::<pascal>();
        let d256 = bed
            .pressure_drop_helium_marched(mdot, p, t_in, t_out, height, 256)
            .unwrap()
            .get::<pascal>();
        let rel = (d256 - d64).abs() / d256;
        assert!(
            rel < 1e-3,
            "march not converged: n=64 {d64}, n=256 {d256}, rel {rel}"
        );

        // (b) small against system pressure.
        let frac = d256 / p.get::<pascal>();
        assert!(
            frac < 0.01,
            "core drop {d256} Pa is {frac} of system pressure -- the \
             constant-pressure property march would no longer be self-consistent"
        );
        assert!(d256 > 0.0);
    }
}
