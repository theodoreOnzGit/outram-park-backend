// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! # KTA 3102.2 heat transfer and KTA 3102.3 pressure drop in a pebble bed
//!
//! Both as IAEA-TECDOC-1694 Table 2.10 gives them for the HTR-10 benchmark
//! (proprietary tier; formulas and coefficients only, per D1). The KTA rules
//! themselves (KTA-GS, German) are not in either literature repository.
//!
//! ## KTA 3102.2 — pebble-surface heat transfer
//!
//! ```text
//! Nu = 1.27 Pr^(1/3) Re^0.36 / eps^1.18 + 0.033 Pr^(1/2) Re^0.86 / eps^1.07
//! alpha = Nu lambda / d,     Re = (mdot/A) d / eta,     Q = alpha A_K (T_K - T_G)
//! ```
//!
//! `A` is the empty-bed flow area, so `mdot/A` is the **superficial** mass
//! flux and `Re` the superficial Reynolds number (not divided by `1 - eps`).
//! `A_K` is the surface of one pebble. Validity as printed: `100 <= Re <=
//! 1e5`, `0.36 <= eps <= 0.42`, bed-to-pebble diameter ratio `D/d >= 20`, bed
//! height `H >= 4d`; deviation +/-20 % at 95 % confidence.
//! [`kta_3102_2_nusselt`] evaluates the formula outside that window too (a
//! natural-circulation transient passes through low `Re`) and
//! [`kta_3102_2_in_validity_range`] tells the caller whether it is inside.
//!
//! ## KTA 3102.3 — friction pressure drop
//!
//! ```text
//! dP/dH = psi (1 - eps)/eps^3 (1/d) (1/(2 rho)) (mdot/A)^2
//! psi   = 320/(Re/(1 - eps)) + 6/(Re/(1 - eps))^0.1
//! ```
//!
//! Validity as printed: `1 < Re/(1 - eps) <= 1e5`, `0.36 <= eps <= 0.42`,
//! `H > 5d`. **This is the one copy of the friction factor in the
//! `tampines` lineage.** It moved here from `tampines::gas_phase::kta_bed` on
//! 2026-11-03 (gh:#1148); `tampines::gas_phase::KtaBed` now calls it. The
//! `outram-park-digital-twin-engine` crate's `htr10::kta` still carries its
//! own copy (not touched here; it is a separate follow-up). Re-read against
//! the rendered Table 2.10 on 2026-11-03: the coefficients `320`, `6` and
//! the `0.1` exponent are those the VTB generic pebble-bed tutorial uses,
//! so the earlier "not checked against the standard" note in `kta_bed` is
//! now answered for the benchmark's printing of it.

use petir::mathf::RealMath;
use uom::si::f64::{
    DynamicViscosity, HeatTransfer, Length, MassDensity, Ratio, ThermalConductivity,
};
use uom::si::ratio::ratio;
use uom::si::{Quantity, ISQ, SI};
use uom::typenum::{N2, P1, Z0};

use super::HtgrClosureError;

/// Mass flux `G = mdot / A`, kg/(m^2 s): `uom`'s own quantity, re-exported.
pub use uom::si::f64::MassFlux;

/// Pressure gradient, Pa/m. `uom` has no named alias for it.
pub type PressureGradient = Quantity<ISQ<N2, P1, N2, Z0, Z0, Z0, Z0>, SI<f64>, f64>;

fn check_porosity(eps: f64, what: &str) -> Result<(), HtgrClosureError> {
    if !(eps > 0.0 && eps < 1.0) {
        return Err(HtgrClosureError::InvalidInput(format!(
            "{what}: porosity {eps} must lie strictly in (0, 1)"
        )));
    }
    Ok(())
}

fn check_positive(v: f64, what: &str) -> Result<(), HtgrClosureError> {
    if !(v.is_finite() && v > 0.0) {
        return Err(HtgrClosureError::InvalidInput(format!(
            "{what} must be finite and positive, got {v}"
        )));
    }
    Ok(())
}

/// KTA 3102.2 Nusselt number of a pebble surface (TECDOC-1694 Tab. 2.10),
/// from the superficial Reynolds number `reynolds`, the gas Prandtl number
/// and the bed porosity.
///
/// Errors with [`HtgrClosureError::InvalidInput`] for a non-positive `Re` or
/// `Pr`, or a porosity outside (0, 1). It does **not** error outside the
/// printed validity window; check [`kta_3102_2_in_validity_range`].
pub fn kta_3102_2_nusselt(
    reynolds: Ratio,
    prandtl: Ratio,
    porosity: Ratio,
) -> Result<Ratio, HtgrClosureError> {
    let (re, pr, eps) = (
        reynolds.get::<ratio>(),
        prandtl.get::<ratio>(),
        porosity.get::<ratio>(),
    );
    check_positive(re, "KTA 3102.2 Reynolds number")?;
    check_positive(pr, "KTA 3102.2 Prandtl number")?;
    check_porosity(eps, "KTA 3102.2")?;
    let nu = 1.27 * pr.r_powf(1.0 / 3.0) / eps.r_powf(1.18) * re.r_powf(0.36)
        + 0.033 * pr.sqrt() / eps.r_powf(1.07) * re.r_powf(0.86);
    Ok(Ratio::new::<ratio>(nu))
}

/// Whether `(Re, eps)` lies inside the KTA 3102.2 window printed in
/// TECDOC-1694 Tab. 2.10: `100 <= Re <= 1e5` and `0.36 <= eps <= 0.42`.
/// (The geometric conditions `D/d >= 20`, `H >= 4d` are the caller's.)
pub fn kta_3102_2_in_validity_range(reynolds: Ratio, porosity: Ratio) -> bool {
    let (re, eps) = (reynolds.get::<ratio>(), porosity.get::<ratio>());
    (100.0..=1.0e5).contains(&re) && (0.36..=0.42).contains(&eps)
}

/// Pebble-surface heat-transfer coefficient `alpha = Nu lambda / d` from
/// KTA 3102.2, with `Re = G d / eta` formed from the superficial mass flux.
///
/// The gas properties are the caller's: [`super::KtaHelium`] for the
/// benchmark's own fits, or CoolProp. Errors as [`kta_3102_2_nusselt`], and
/// for non-positive diameter, viscosity or conductivity.
pub fn kta_3102_2_heat_transfer_coefficient(
    mass_flux: MassFlux,
    pebble_diameter: Length,
    porosity: Ratio,
    viscosity: DynamicViscosity,
    conductivity: ThermalConductivity,
    prandtl: Ratio,
) -> Result<HeatTransfer, HtgrClosureError> {
    check_positive(pebble_diameter.value, "pebble diameter")?;
    check_positive(viscosity.value, "gas viscosity")?;
    check_positive(conductivity.value, "gas conductivity")?;
    let re: Ratio = mass_flux * pebble_diameter / viscosity;
    let nu = kta_3102_2_nusselt(re, prandtl, porosity)?;
    Ok(nu * conductivity / pebble_diameter)
}

/// KTA 3102.3 friction factor `psi = 320/Re_m + 6/Re_m^0.1`,
/// `Re_m = Re/(1 - eps)`, from the **superficial** Reynolds number (the
/// `1/(1 - eps)` is applied here; do not pre-divide).
///
/// Errors with [`HtgrClosureError::InvalidInput`] for a porosity outside
/// (0, 1) or a non-positive or non-finite Reynolds number.
pub fn kta_3102_3_friction_factor(
    reynolds: Ratio,
    porosity: Ratio,
) -> Result<Ratio, HtgrClosureError> {
    let (re, eps) = (reynolds.get::<ratio>(), porosity.get::<ratio>());
    check_porosity(eps, "KTA 3102.3 friction factor")?;
    check_positive(re, "KTA 3102.3 Reynolds number")?;
    let re_m = re / (1.0 - eps);
    Ok(Ratio::new::<ratio>(320.0 / re_m + 6.0 / re_m.r_powf(0.1)))
}

/// KTA 3102.3 pressure-drop magnitude per unit bed height, Pa/m (positive),
/// `psi (1 - eps)/eps^3 G^2 / (2 d rho)`, with `Re = G d / eta`.
///
/// Errors as [`kta_3102_3_friction_factor`], and for a non-positive
/// diameter, density or viscosity.
pub fn kta_3102_3_pressure_gradient(
    mass_flux: MassFlux,
    pebble_diameter: Length,
    porosity: Ratio,
    density: MassDensity,
    viscosity: DynamicViscosity,
) -> Result<PressureGradient, HtgrClosureError> {
    check_positive(pebble_diameter.value, "pebble diameter")?;
    check_positive(density.value, "gas density")?;
    check_positive(viscosity.value, "gas viscosity")?;
    let re: Ratio = mass_flux * pebble_diameter / viscosity;
    let psi = kta_3102_3_friction_factor(re, porosity)?;
    let eps = porosity.get::<ratio>();
    let geometry = Ratio::new::<ratio>((1.0 - eps) / (eps * eps * eps));
    Ok(psi * geometry * mass_flux * mass_flux
        / (Ratio::new::<ratio>(2.0) * pebble_diameter * density))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::htgr_closures::helium::KtaHelium;
    use uom::si::dynamic_viscosity::pascal_second;
    use uom::si::heat_transfer::watt_per_square_meter_kelvin;
    use uom::si::length::meter;
    use uom::si::mass_density::kilogram_per_cubic_meter;
    use uom::si::pressure::bar;
    use uom::si::thermodynamic_temperature::kelvin;
    use uom::si::f64::{Pressure, ThermodynamicTemperature};

    fn r(v: f64) -> Ratio {
        Ratio::new::<ratio>(v)
    }

    /// V&V — KTA 3102.2 against a hand evaluation of TECDOC-1694 Tab. 2.10.
    ///
    /// **Methodology.** (a) At `Re = 1e4`, `Pr = 0.66`, `eps = 0.39`, the
    /// printed formula evaluated independently in Python gives
    /// `Nu = 294.74200175`. (b) The HTR-10 design point, as a worked example:
    /// 4.3 kg/s through the empty 1.8 m bore (`A = 2.54469 m^2`,
    /// `G = 1.689793 kg/(m^2 s)`), 6 cm pebbles, `eps = 0.39`, helium at
    /// 30 bar and the 748.15 K core-mean temperature from the Tab. 2.8 fits
    /// (bar reading): Python gives `eta = 3.77512e-5 Pa s`,
    /// `lambda = 0.295895 W/(m K)`, `Pr = 0.662795`, `Re = 2685.68`,
    /// `Nu = 123.138`, `alpha = 607.27 W/(m^2 K)`. Tolerance 1e-9 relative
    /// (both are the same formula; this checks the transcription and the
    /// unit handling). (c) Cross-check against Wakao-Funazkri,
    /// `Nu = 2 + 1.1 Pr^(1/3) Re^0.6`, the closure `tampines` uses: at the
    /// design point it gives 111.47, so KTA is 10.5 % higher, inside KTA's
    /// own +/-20 % band. Reported, not gated tighter than 20 %.
    ///
    /// **Results (2026-11-03).** (a) and (b) agree to < 1e-9; (b) is inside
    /// the validity window (`Re = 2686`). (c) KTA/Wakao = 1.1047.
    #[test]
    fn kta_3102_2_matches_hand_values_and_wakao() {
        let nu = kta_3102_2_nusselt(r(1.0e4), r(0.66), r(0.39)).unwrap().get::<ratio>();
        assert!((nu / 294.7420017540218 - 1.0).abs() < 1e-9, "{nu}");

        let area = std::f64::consts::FRAC_PI_4 * 1.8 * 1.8;
        let g = MassFlux::new::<uom::si::mass_flux::kilogram_per_square_meter_second>(4.3 / area);
        let he = KtaHelium::new(
            Pressure::new::<bar>(30.0),
            ThermodynamicTemperature::new::<kelvin>(748.15),
        )
        .unwrap();
        let d = Length::new::<meter>(0.06);
        let re: Ratio = g * d / he.viscosity();
        let alpha = kta_3102_2_heat_transfer_coefficient(
            g,
            d,
            r(0.39),
            he.viscosity(),
            he.thermal_conductivity(),
            he.prandtl(),
        )
        .unwrap()
        .get::<watt_per_square_meter_kelvin>();
        let nu = kta_3102_2_nusselt(re, he.prandtl(), r(0.39)).unwrap().get::<ratio>();
        let pr = he.prandtl().get::<ratio>();
        let re_v = re.get::<ratio>();
        let wakao = 2.0 + 1.1 * pr.powf(1.0 / 3.0) * re_v.powf(0.6);
        println!("HTR-10 design point: Re {re_v:.3} Pr {pr:.6} Nu {nu:.4} alpha {alpha:.3} W/m2K; Wakao Nu {wakao:.4}; KTA/Wakao {:.4}", nu / wakao);
        assert!((re_v / 2685.675802303499 - 1.0).abs() < 1e-9);
        assert!((nu / 123.13803645701289 - 1.0).abs() < 1e-9);
        assert!((alpha / 607.2658141761322 - 1.0).abs() < 1e-9);
        assert!(kta_3102_2_in_validity_range(re, r(0.39)));
        assert!(!kta_3102_2_in_validity_range(r(50.0), r(0.39)));
        assert!(!kta_3102_2_in_validity_range(r(1e3), r(0.45)));
        assert!((nu / wakao - 1.0).abs() < 0.20);
        assert!(kta_3102_2_nusselt(r(0.0), r(0.66), r(0.39)).is_err());
        assert!(kta_3102_2_nusselt(r(1e3), r(0.66), r(1.0)).is_err());
    }

    /// V&V — KTA 3102.3 against the VTB generic pebble-bed tutorial step 2
    /// gold values (Open tier), the same gold values the `tampines` copy was
    /// verified against before it moved here.
    ///
    /// **Methodology.** `Re = 40125`, `eps = 0.39`, `d = 0.06 m`,
    /// `mu = 1.991242e-5 Pa s`, `rho = 8.628204 kg/m^3`; published
    /// `psi = 1.983` and `3493 Pa/m`. Pass: `psi` within 0.001, gradient
    /// within 1 Pa/m (the published digits).
    ///
    /// **Results (2026-11-03).** `psi = 1.983395`, gradient
    /// `3493.167 Pa/m` — the same digits the `tampines` copy recorded on
    /// 2026-08-11, as it must be, since it is now this code.
    #[test]
    fn kta_3102_3_reproduces_vtb_gold_values() {
        let psi = kta_3102_3_friction_factor(r(40125.0), r(0.39)).unwrap().get::<ratio>();
        let mu = DynamicViscosity::new::<pascal_second>(1.991242e-5);
        let d = Length::new::<meter>(0.06);
        let g: MassFlux = r(40125.0) * mu / d;
        let grad = kta_3102_3_pressure_gradient(
            g,
            d,
            r(0.39),
            MassDensity::new::<kilogram_per_cubic_meter>(8.628204),
            mu,
        )
        .unwrap()
        .value;
        println!("psi {psi:.6}, gradient {grad:.3} Pa/m");
        assert!((psi - 1.983).abs() < 1e-3);
        assert!((psi - 1.983395).abs() < 1e-6);
        assert!((grad - 3493.0).abs() < 1.0);
        assert!(kta_3102_3_friction_factor(r(1e3), r(0.0)).is_err());
        assert!(kta_3102_3_friction_factor(r(f64::NAN), r(0.39)).is_err());
    }
}
