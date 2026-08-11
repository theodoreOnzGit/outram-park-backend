//! # Helium thermophysical-property adapter for the HTR-10 primary circuit
//!
//! A thin, `uom`-typed façade over [`outram_park_fork_coolprop`]'s helium
//! equation of state and transport correlations, evaluated at a
//! `(temperature, pressure)` state point.
//!
//! ## What belongs here
//!
//! Pure, stateless property lookups for **gaseous helium**: density,
//! dynamic viscosity, thermal conductivity, isobaric specific heat,
//! specific enthalpy, speed of sound, and the derived Prandtl number. Each
//! is a single call with no hidden state, so a component model can evaluate
//! properties at whatever mean state it decides is appropriate.
//!
//! ## What does NOT belong here
//!
//! - **Property *correlations* themselves.** The Helmholtz EOS and the
//!   transport fits live in `outram-park-fork-coolprop`; this module only
//!   adapts them. If a correlation is wrong, fix it there, not here.
//! - **Component physics** (pressure drop, heat transfer). Those live in
//!   the sibling modules [`crate::gas_phase::pipe`] and
//!   [`crate::gas_phase::kta_bed`].
//! - **Other gases.** The adapter is deliberately helium-only so the
//!   documented validity ranges below mean something. `coolprop`'s
//!   [`Fluid`] enum covers ~137 fluids if a caller needs another one.
//!
//! ## Provenance of the underlying correlations
//!
//! Read out of `outram-park-fork-coolprop` on 2026-08-11:
//!
//! - **Equation of state** — `crates/outram-park-fork-coolprop/src/fluids/helium.rs`,
//!   a reduced-Helmholtz (Span-Wagner form) EOS with Power + Gaussian
//!   residual terms only (no non-analytic critical term). That crate's
//!   `tests/helium_reference.rs` reproduces CoolProp's own tabulated
//!   triple-liquid, triple-vapour and **critical-point** pressures to
//!   better than 1e-3 relative.
//! - **Dynamic viscosity** — Arp, McCarty & Friend, *NIST Technical Note
//!   1334* (1998), as implemented by CoolProp's
//!   `viscosity_helium_hardcoded` and ported in
//!   `outram-park-fork-coolprop/src/transport.rs` (`helium_viscosity`).
//! - **Thermal conductivity** — Hands & Arp, as implemented by CoolProp's
//!   `conductivity_hardcoded_helium` and ported in the same file
//!   (`helium_conductivity`). The near-critical enhancement term `lambda_c`
//!   (only active over 3.5-12 K) is omitted upstream, which is irrelevant
//!   at HTGR temperatures.
//!
//! ## Why this module exists (a known defect it replaces)
//!
//! The `htgr_sim_v1` example in the **read-only** downstream crate
//! `outram-park-digital-twin-engine` hard-codes a *constant* helium dynamic
//! viscosity even though the Arp-McCarty-Friend correlation is available in
//! `outram-park-fork-coolprop`. That remains a defect in that crate and is
//! **not** fixed by this module; this module is the correct replacement
//! path a future fix should call. Tracked in the workspace bead tracker
//! (see `op-wqk.9.1` and the follow-up filed on 2026-08-11).
//!
//! ## Validity range
//!
//! The public functions guard for `T > 0`, `p > 0` and finite inputs, and
//! reject anything outside **2.2 K to 1500 K** and **1 Pa to 100 MPa** with
//! [`TampinesError::InvalidInput`]. That envelope comfortably contains the
//! HTR-10 primary circuit (3.0 MPa, 523.15 K core inlet to 973.15 K core
//! outlet). The bounds are a *usage* guard on the adapter, not a claim that
//! the upstream fits are equally accurate everywhere inside them — the
//! viscosity correlation in particular switches branch at 100 K and freezes
//! its `ln(T)` argument above 300 K.
//!
//! ## Status
//!
//! **NOT VALIDATED against HTR-10 measurements.** The tests below are
//! code-to-code and self-consistency checks against the upstream CoolProp
//! port plus ideal-gas limits. AI-assisted draft pending human review per
//! `RESPONSIBLE_USE.md`.

use crate::TampinesError;
use outram_park_fork_coolprop::{conductivity, state_ph, state_pt, viscosity, Fluid};
use uom::si::available_energy::joule_per_kilogram;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{
    AvailableEnergy, DynamicViscosity, MassDensity, Pressure, Ratio, SpecificHeatCapacity,
    ThermalConductivity, ThermodynamicTemperature, Velocity,
};
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::velocity::meter_per_second;

/// Specific enthalpy (energy per unit mass), J/kg.
///
/// A readable alias for `uom`'s `AvailableEnergy`, whose name does not
/// suggest "specific enthalpy" to a thermal-hydraulics reader.
pub type SpecificEnthalpy = AvailableEnergy;

/// Lowest temperature the adapter accepts, kelvin. Helium's triple point is
/// 2.1768 K; below that the single-phase EOS call is meaningless.
const T_MIN_K: f64 = 2.2;
/// Highest temperature the adapter accepts, kelvin. Well above the HTR-10
/// core outlet (973.15 K) and above graphite-core accident temperatures.
const T_MAX_K: f64 = 1500.0;
/// Lowest pressure the adapter accepts, pascal.
const P_MIN_PA: f64 = 1.0;
/// Highest pressure the adapter accepts, pascal.
const P_MAX_PA: f64 = 1.0e8;

/// Every helium property this module can report, evaluated at one
/// `(temperature, pressure)` state point.
///
/// Returned as a bundle by [`helium_state`] because the underlying EOS
/// flash is the expensive step and yields all of these at once — asking for
/// density and then cp separately solves the same Newton iteration twice.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeliumState {
    /// Temperature the state was evaluated at, kelvin.
    pub temperature: ThermodynamicTemperature,
    /// Pressure the state was evaluated at, pascal.
    pub pressure: Pressure,
    /// Mass density, kg/m^3. At the HTR-10 design point (3.0 MPa) helium
    /// runs roughly 1.2-2.8 kg/m^3 across the 250-700 C core rise.
    pub density: MassDensity,
    /// Dynamic viscosity, Pa s. Order 3-4 x 10^-5 Pa s at HTGR
    /// temperatures — it rises with temperature, unlike a liquid's.
    pub dynamic_viscosity: DynamicViscosity,
    /// Thermal conductivity, W/(m K). Helium conducts unusually well for a
    /// gas (order 0.3 W/(m K) at HTGR temperatures, ~10x air).
    pub thermal_conductivity: ThermalConductivity,
    /// Isobaric specific heat `c_p`, J/(kg K). Near-constant at about
    /// 5193 J/(kg K) for a monatomic ideal gas (`5R/2M`).
    pub specific_heat_cp: SpecificHeatCapacity,
    /// Specific enthalpy, J/kg, on the upstream EOS's own reference-state
    /// convention. **Only enthalpy *differences* are meaningful** — never
    /// compare an absolute value here against another property library.
    pub specific_enthalpy: SpecificEnthalpy,
    /// Speed of sound, m/s. Used to form the Mach number that justifies the
    /// low-Mach treatment in [`crate::gas_phase::pipe`].
    pub speed_of_sound: Velocity,
}

impl HeliumState {
    /// Prandtl number `Pr = c_p mu / lambda`, dimensionless.
    ///
    /// Monatomic gases sit near 0.66-0.68; this is the group both the
    /// Dittus-Boelter and Gnielinski heat-transfer correlations take, and
    /// helium's low `Pr` is why those correlations must be used inside
    /// their stated `Pr` validity ranges rather than extrapolated.
    pub fn prandtl(&self) -> Ratio {
        let cp = self.specific_heat_cp.get::<joule_per_kilogram_kelvin>();
        let mu = self.dynamic_viscosity.get::<pascal_second>();
        let k = self.thermal_conductivity.get::<watt_per_meter_kelvin>();
        Ratio::new::<ratio>(cp * mu / k)
    }

    /// Kinematic viscosity `nu = mu / rho`, m^2/s, as a plain `f64` in SI
    /// units.
    ///
    /// Returned unwrapped rather than as a `uom` `KinematicViscosity`
    /// because it exists only to be fed straight into a dimensionless
    /// group; prefer [`reynolds_number`] over forming it by hand.
    pub fn kinematic_viscosity_m2_per_s(&self) -> f64 {
        self.dynamic_viscosity.get::<pascal_second>()
            / self.density.get::<kilogram_per_cubic_meter>()
    }
}

/// Validate a `(T, p)` state point against the adapter's accepted envelope.
fn check_state(t_k: f64, p_pa: f64) -> Result<(), TampinesError> {
    if !t_k.is_finite() || !p_pa.is_finite() {
        return Err(TampinesError::InvalidInput(format!(
            "helium properties: non-finite state (T = {t_k} K, p = {p_pa} Pa)"
        )));
    }
    if !(T_MIN_K..=T_MAX_K).contains(&t_k) {
        return Err(TampinesError::InvalidInput(format!(
            "helium properties: temperature {t_k} K outside the supported \
             {T_MIN_K}-{T_MAX_K} K range"
        )));
    }
    if !(P_MIN_PA..=P_MAX_PA).contains(&p_pa) {
        return Err(TampinesError::InvalidInput(format!(
            "helium properties: pressure {p_pa} Pa outside the supported \
             {P_MIN_PA}-{P_MAX_PA} Pa range"
        )));
    }
    Ok(())
}

/// Full helium thermophysical state at temperature `t` and pressure `p`.
///
/// This is the primary entry point — the single-property helpers below all
/// delegate to it. Inputs must lie in 2.2-1500 K and 1 Pa-100 MPa (see the
/// module docs); anything else returns [`TampinesError::InvalidInput`].
///
/// Errors with [`TampinesError::Numerical`] if the upstream `(p, T)` flash
/// fails to converge, and with [`TampinesError::Unphysical`] if the
/// transport correlations decline to return a value (which for helium means
/// the state landed outside their supported region).
pub fn helium_state(
    t: ThermodynamicTemperature,
    p: Pressure,
) -> Result<HeliumState, TampinesError> {
    let t_k = t.get::<kelvin>();
    let p_pa = p.get::<pascal>();
    check_state(t_k, p_pa)?;

    let s = state_pt(Fluid::Helium, t_k, p_pa).map_err(|e| {
        TampinesError::Numerical(format!(
            "helium (p,T) flash failed at T = {t_k} K, p = {p_pa} Pa: {e:?}"
        ))
    })?;

    let mu = viscosity(Fluid::Helium, t_k, s.density).ok_or_else(|| {
        TampinesError::Unphysical(format!(
            "helium viscosity correlation returned no value at T = {t_k} K, \
             rho = {} kg/m^3",
            s.density
        ))
    })?;
    let lambda = conductivity(Fluid::Helium, t_k, s.density).ok_or_else(|| {
        TampinesError::Unphysical(format!(
            "helium conductivity correlation returned no value at T = {t_k} K, \
             rho = {} kg/m^3",
            s.density
        ))
    })?;

    Ok(HeliumState {
        temperature: t,
        pressure: p,
        density: MassDensity::new::<kilogram_per_cubic_meter>(s.density),
        dynamic_viscosity: DynamicViscosity::new::<pascal_second>(mu),
        thermal_conductivity: ThermalConductivity::new::<watt_per_meter_kelvin>(lambda),
        specific_heat_cp: SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(s.cp),
        specific_enthalpy: SpecificEnthalpy::new::<joule_per_kilogram>(s.enthalpy),
        speed_of_sound: Velocity::new::<meter_per_second>(s.speed_of_sound),
    })
}

/// Full helium thermophysical state from **pressure and specific
/// enthalpy**, the natural pair for a steady-flow energy balance.
///
/// A duct that adds `Q` watts to a flow of `mdot` kg/s raises the specific
/// enthalpy by exactly `Q/mdot`; recovering the temperature from `(p, h)`
/// keeps that balance exact as `c_p` varies, where a `Q = mdot c_p dT`
/// shortcut would not.
///
/// `enthalpy` must be on the **same reference-state convention** as
/// [`HeliumState::specific_enthalpy`] — i.e. it must have come from this
/// module (or from `outram-park-fork-coolprop` directly). Absolute
/// enthalpies from another property library will silently give a wrong
/// temperature.
///
/// Errors with [`TampinesError::Numerical`] if the `(p, h)` flash does not
/// converge (which for helium means the requested enthalpy is outside the
/// EOS's reach), and otherwise as [`helium_state`].
pub fn helium_state_ph(
    p: Pressure,
    enthalpy: SpecificEnthalpy,
) -> Result<HeliumState, TampinesError> {
    let p_pa = p.get::<pascal>();
    let h = enthalpy.get::<joule_per_kilogram>();
    if !p_pa.is_finite() || !h.is_finite() {
        return Err(TampinesError::InvalidInput(format!(
            "helium properties: non-finite (p, h) state (p = {p_pa} Pa, h = {h} J/kg)"
        )));
    }
    let s = state_ph(Fluid::Helium, p_pa, h).map_err(|e| {
        TampinesError::Numerical(format!(
            "helium (p,h) flash failed at p = {p_pa} Pa, h = {h} J/kg: {e:?}"
        ))
    })?;
    // Re-enter through the (T, p) path so the same envelope guard and the
    // same transport-correlation error handling apply.
    helium_state(ThermodynamicTemperature::new::<kelvin>(s.temperature), p)
}

/// Helium mass density, kg/m^3, at `(t, p)`. See [`helium_state`] for the
/// accepted input envelope and the error cases.
pub fn helium_density(
    t: ThermodynamicTemperature,
    p: Pressure,
) -> Result<MassDensity, TampinesError> {
    Ok(helium_state(t, p)?.density)
}

/// Helium dynamic viscosity, Pa s, at `(t, p)` (Arp, McCarty & Friend,
/// NIST TN-1334). See [`helium_state`] for the accepted input envelope.
///
/// This is the correct replacement for any hard-coded constant helium
/// viscosity — see the module-level note on `htgr_sim_v1`.
pub fn helium_viscosity(
    t: ThermodynamicTemperature,
    p: Pressure,
) -> Result<DynamicViscosity, TampinesError> {
    Ok(helium_state(t, p)?.dynamic_viscosity)
}

/// Helium thermal conductivity, W/(m K), at `(t, p)` (Hands & Arp). See
/// [`helium_state`] for the accepted input envelope.
pub fn helium_thermal_conductivity(
    t: ThermodynamicTemperature,
    p: Pressure,
) -> Result<ThermalConductivity, TampinesError> {
    Ok(helium_state(t, p)?.thermal_conductivity)
}

/// Helium isobaric specific heat `c_p`, J/(kg K), at `(t, p)`. See
/// [`helium_state`] for the accepted input envelope.
pub fn helium_cp(
    t: ThermodynamicTemperature,
    p: Pressure,
) -> Result<SpecificHeatCapacity, TampinesError> {
    Ok(helium_state(t, p)?.specific_heat_cp)
}

/// Helium Prandtl number, dimensionless, at `(t, p)`. See [`helium_state`]
/// for the accepted input envelope.
pub fn helium_prandtl(t: ThermodynamicTemperature, p: Pressure) -> Result<Ratio, TampinesError> {
    Ok(helium_state(t, p)?.prandtl())
}

/// Reynolds number `Re = G D / mu` for a duct, dimensionless, from the mass
/// flux `G` \[kg/(m^2 s)\], the hydraulic diameter `d` and the state's
/// dynamic viscosity.
///
/// Written in the mass-flux form deliberately: `G = mdot/A` is constant
/// along a duct of fixed area even when the gas expands, so `Re` computed
/// this way does not silently depend on which density the caller picked.
pub fn reynolds_number(
    mass_flux_kg_per_m2_s: f64,
    hydraulic_diameter: uom::si::f64::Length,
    dynamic_viscosity: DynamicViscosity,
) -> Ratio {
    let d = hydraulic_diameter.get::<uom::si::length::meter>();
    let mu = dynamic_viscosity.get::<pascal_second>();
    Ratio::new::<ratio>(mass_flux_kg_per_m2_s * d / mu)
}

/// The HTR-10 primary-circuit design point, as the state inputs this module
/// takes.
///
/// Values from the IAEA HTR-10 benchmark description (Open tier): helium at
/// **3.0 MPa**, **4.3 kg/s** total core flow, core inlet **250 C**
/// (523.15 K), core outlet **700 C** (973.15 K), **10 MW** thermal.
pub mod htr10_design_point {
    use super::*;

    /// Primary-circuit helium pressure, 3.0 MPa.
    pub fn pressure() -> Pressure {
        Pressure::new::<pascal>(3.0e6)
    }

    /// Core inlet temperature, 250 C = 523.15 K.
    pub fn core_inlet_temperature() -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(523.15)
    }

    /// Core outlet temperature, 700 C = 973.15 K.
    pub fn core_outlet_temperature() -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(973.15)
    }

    /// Total core helium mass flow rate, 4.3 kg/s.
    pub fn mass_flow_rate() -> uom::si::f64::MassRate {
        uom::si::f64::MassRate::new::<uom::si::mass_rate::kilogram_per_second>(4.3)
    }

    /// Core thermal power, 10 MW.
    pub fn thermal_power() -> uom::si::f64::Power {
        uom::si::f64::Power::new::<uom::si::power::watt>(10.0e6)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(k: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(k)
    }
    fn p(pa: f64) -> Pressure {
        Pressure::new::<pascal>(pa)
    }

    /// V&V — density against `outram-park-fork-coolprop`'s own recorded
    /// HTR-10 helium anchor states.
    ///
    /// **Methodology.** The upstream crate records four helium densities
    /// measured from its EOS on 2026-08-11 (crate v0.1.1), in
    /// `src/openfoam_algorithms/rhoPimpleFoam/mod.rs`, test
    /// `helium_htr10_default_taper_zeroes_hybrid`: 2.739989 kg/m^3 at
    /// (523.15 K, 3.0 MPa), 1.478781 at (973.15 K, 3.0 MPa), 1.227576 at
    /// (1173.15 K, 3.0 MPa), and 0.160391 at (300 K, 1 bar). This adapter
    /// must reproduce them exactly — it is a pass-through, so any
    /// difference means the adapter is transforming the value. Pass
    /// criterion: relative difference below 1e-5 — the upstream anchors
    /// are quoted to six decimal places, so 1e-5 is the tightest bound the
    /// *quoted digits* can support, not a statement about the EOS's own
    /// precision.
    ///
    /// **Results (2026-08-11, this adapter).** All four reproduce the
    /// upstream anchors to every quoted digit. Measured relative
    /// differences: **2.052e-8** at (523.15 K, 3.0 MPa,
    /// rho = 2.739988944), **2.337e-7** at (973.15 K, 3.0 MPa,
    /// rho = 1.478781346), **5.869e-8** at (1173.15 K, 3.0 MPa,
    /// rho = 1.227576072), and **2.533e-6** at (300 K, 1 bar,
    /// rho = 0.160391406) — the last being purely the rounding of the
    /// anchor's own sixth decimal place. All PASS.
    ///
    /// **Interpretation.** The adapter adds no error to the upstream EOS.
    /// This is a code-to-code check within one workspace, **not** an
    /// independent validation against NIST or CoolProp upstream.
    #[test]
    fn density_reproduces_coolprop_htr10_anchor_states() {
        let anchors = [
            (523.15, 3.0e6, 2.739989_f64, "HTR-10 core inlet"),
            (973.15, 3.0e6, 1.478781, "HTR-10 design core outlet"),
            (1173.15, 3.0e6, 1.227576, "high-temperature test"),
            (300.0, 1.0e5, 0.160391, "cold depressurised"),
        ];
        for (t_k, p_pa, expected, label) in anchors {
            let rho = helium_density(t(t_k), p(p_pa))
                .unwrap()
                .get::<kilogram_per_cubic_meter>();
            let rel = (rho - expected).abs() / expected;
            println!(
                "{label:28} T = {t_k:8.2} K  p = {p_pa:9.0} Pa  \
                 rho = {rho:.9}  (anchor {expected})  rel = {rel:.3e}"
            );
            assert!(rel < 1e-5, "{label}: rho = {rho} vs anchor {expected}");
        }
    }

    /// V&V — transport properties against the literature values
    /// `outram-park-fork-coolprop` cites in
    /// `tests/transport_vle.rs::transport_helium_hardcoded_vs_literature`.
    ///
    /// **Methodology.** That test asserts, against NIST literature values,
    /// `mu(300 K, 0.16035 kg/m^3) ~ 19.9e-6 Pa s` (2 % tolerance),
    /// `lambda(300 K, 0.16035) ~ 0.152 W/(m K)` (5 %), and
    /// `mu(400 K, 0.1203) ~ 24.3e-6 Pa s` (2 %). This adapter is entered
    /// through `(T, p)` rather than `(T, rho)`, so the states are
    /// reproduced at 1 bar — where the adapter's own density is 0.160391
    /// and 0.120288 kg/m^3 respectively, within 0.03 % of the quoted
    /// densities, far inside the tolerance bands. Pass criteria are the
    /// upstream tolerances.
    ///
    /// **Results (2026-08-11, this adapter).** Measured
    /// `mu(300 K, 1 bar) = 1.992967e-5 Pa s` (`+0.149 %` vs 19.9e-6, at
    /// this adapter's rho = 0.160391 kg/m^3),
    /// `mu(400 K, 1 bar) = 2.429223e-5 Pa s` (`-0.032 %` vs 24.3e-6, at
    /// rho = 0.120309), and `lambda(300 K, 1 bar) = 0.155973 W/(m K)`
    /// (`+2.614 %` vs 0.152). All PASS against the upstream 2 % / 5 %
    /// tolerances.
    ///
    /// **Interpretation.** The Arp-McCarty-Friend viscosity and Hands-Arp
    /// conductivity correlations reach the adapter intact and agree with
    /// the cited NIST values at ambient conditions. The band the
    /// correlations are quoted at (2 % / 5 %) is the accuracy limit, not
    /// the agreement shown here — note the conductivity sits +2.6 % off
    /// the quoted literature value, over half its allowed band. No check
    /// at HTGR temperatures against an independent source has been made.
    #[test]
    fn transport_properties_match_the_cited_nist_literature_values() {
        let cases = [
            (300.0_f64, 19.9e-6_f64, 0.02_f64, "viscosity at 300 K"),
            (400.0, 24.3e-6, 0.02, "viscosity at 400 K"),
        ];
        for (t_k, expected, tol, label) in cases {
            let s = helium_state(t(t_k), p(1.0e5)).unwrap();
            let mu = s.dynamic_viscosity.get::<pascal_second>();
            let rel = (mu - expected) / expected;
            println!(
                "{label:22} rho = {:.6} kg/m^3  mu = {mu:.6e} Pa s  \
                 (literature {expected:.3e})  rel = {:+.4} %",
                s.density.get::<kilogram_per_cubic_meter>(),
                100.0 * rel
            );
            assert!(rel.abs() < tol, "{label}: {mu} vs {expected}, rel {rel}");
        }

        let s = helium_state(t(300.0), p(1.0e5)).unwrap();
        let k = s.thermal_conductivity.get::<watt_per_meter_kelvin>();
        let rel = (k - 0.152) / 0.152;
        println!(
            "conductivity at 300 K  k = {k:.6} W/(m K)  (literature 0.152)  rel = {:+.4} %",
            100.0 * rel
        );
        assert!(rel.abs() < 0.05, "conductivity {k} vs 0.152, rel {rel}");
    }

    /// V&V — isobaric specific heat against the anchor recorded in
    /// `outram-park-digital-twin-engine/src/htr10/design.rs`.
    ///
    /// **Methodology.** That crate's
    /// `operating_point_energy_balance_closes_against_coolprop_helium_cp`
    /// records `c_p(748.15 K, 3.0 MPa) = 5191.5 J/(kg K)` and uses it to
    /// close the HTR-10 energy balance to +0.455 % of the published 10 MW.
    /// Reproduce that `c_p` here (pass criterion 0.1 %), and separately
    /// check it against the monatomic ideal-gas value
    /// `5R/(2M) = 5 x 8.3144598 / (2 x 0.004002602) = 5193.1592 J/(kg K)`
    /// (pass criterion 1 %, since helium at 3 MPa is nearly but not
    /// exactly ideal).
    ///
    /// **Results (2026-08-11, this adapter).** Measured
    /// `c_p(748.15 K, 3.0 MPa) = 5191.451091 J/(kg K)`, `-0.00094 %`
    /// against the 5191.5 anchor and `-0.0329 %` against the ideal-gas
    /// 5193.1592. Both PASS. Across the whole 250-700 C core range `c_p`
    /// varies only between 5191.441 and 5191.615 J/(kg K) (see
    /// [`measure_htr10_states`]) — a 0.0034 % spread, which is why a
    /// constant-`c_p` energy balance is a defensible approximation for
    /// helium even though a constant *viscosity* is not.
    #[test]
    fn cp_matches_the_htr10_anchor_and_the_ideal_monatomic_value() {
        let cp = helium_cp(t(748.15), p(3.0e6))
            .unwrap()
            .get::<joule_per_kilogram_kelvin>();
        let ideal = 5.0 * 8.3144598 / (2.0 * 0.004002602);
        println!(
            "cp(748.15 K, 3 MPa) = {cp:.6} J/(kg K)  \
             (anchor 5191.5, rel {:+.5} %)  (ideal 5R/2M = {ideal:.4}, rel {:+.5} %)",
            100.0 * (cp - 5191.5) / 5191.5,
            100.0 * (cp - ideal) / ideal
        );
        assert!(
            (cp - 5191.5).abs() / 5191.5 < 1e-3,
            "cp = {cp} vs anchor 5191.5"
        );
        assert!(
            (cp - ideal).abs() / ideal < 0.01,
            "cp = {cp} vs ideal {ideal}"
        );
    }

    /// V&V — helium's density varies by nearly a factor of two across the
    /// HTR-10 core, which is the measurement behind the ruling that
    /// Boussinesq (TUAS) cannot model it.
    ///
    /// **Methodology.** Compare the density at the 523.15 K core inlet and
    /// the 973.15 K core outlet, both at 3.0 MPa. The Boussinesq
    /// approximation requires `|drho/rho| << 1`; a ratio near 2 fails that
    /// outright. Pass criterion: the ratio exceeds 1.5.
    ///
    /// **Result (2026-08-11).** `rho(523.15 K) = 2.739989 kg/m^3`,
    /// `rho(973.15 K) = 1.478781 kg/m^3`, ratio **1.85287**, i.e. the
    /// density falls by **46.0 %** across the core. PASSES.
    ///
    /// **Interpretation.** Boussinesq is inapplicable to HTR-10 helium by
    /// a wide margin. This is the quantitative basis for the module-level
    /// scope ruling in [`crate::gas_phase`].
    #[test]
    fn density_ratio_across_the_core_rules_out_boussinesq() {
        let cold = helium_density(t(523.15), p(3.0e6))
            .unwrap()
            .get::<kilogram_per_cubic_meter>();
        let hot = helium_density(t(973.15), p(3.0e6))
            .unwrap()
            .get::<kilogram_per_cubic_meter>();
        let ratio_v = cold / hot;
        println!(
            "rho(523.15 K) = {cold:.6}, rho(973.15 K) = {hot:.6}, ratio = {ratio_v:.5} \
             ({:.1} % density fall across the core)",
            100.0 * (cold - hot) / cold
        );
        assert!(
            ratio_v > 1.5,
            "density ratio {ratio_v} is too small to rule out Boussinesq"
        );
    }

    /// V&V — Prandtl number stays in the narrow band monatomic gases
    /// occupy, and inside Gnielinski's stated `0.5 <= Pr <= 2000` validity
    /// range across the whole HTR-10 core.
    ///
    /// **Methodology.** Evaluate `Pr = c_p mu / lambda` at 3.0 MPa over
    /// 523.15-973.15 K. Kinetic theory puts a monatomic gas near
    /// `Pr = 2/3`. Pass criterion: `0.60 <= Pr <= 0.72` throughout.
    ///
    /// **Result (2026-08-11).** `Pr` rises monotonically from **0.658469**
    /// at 250 C to **0.661835** at 700 C — a 0.5 % spread, comfortably
    /// inside the band and close to the kinetic-theory 2/3. PASSES.
    ///
    /// **Interpretation.** Gnielinski is valid across the whole core;
    /// Dittus-Boelter's stated lower bound of `Pr = 0.6` is met but with
    /// under 10 % margin, which is why Gnielinski is this workspace's
    /// default gas heat-transfer correlation.
    #[test]
    fn prandtl_stays_in_the_monatomic_band_across_the_core() {
        let mut previous = 0.0_f64;
        for t_c in [250.0_f64, 350.0, 450.0, 550.0, 650.0, 700.0] {
            let pr = helium_prandtl(t(t_c + 273.15), p(3.0e6))
                .unwrap()
                .get::<ratio>();
            println!("T = {t_c:6.1} C  Pr = {pr:.6}");
            assert!(
                (0.60..=0.72).contains(&pr),
                "Pr = {pr} at {t_c} C is outside the monatomic band"
            );
            assert!(pr > previous, "Pr is not monotonically increasing");
            previous = pr;
        }
    }

    /// The `(p, h)` entry point must invert the `(T, p)` one exactly.
    #[test]
    fn state_ph_inverts_state_pt() {
        for t_k in [300.0_f64, 523.15, 748.15, 973.15, 1173.15] {
            let a = helium_state(t(t_k), p(3.0e6)).unwrap();
            let b = helium_state_ph(p(3.0e6), a.specific_enthalpy).unwrap();
            let d = (b.temperature.get::<kelvin>() - t_k).abs();
            println!("T = {t_k:8.2} K  round-trip residual = {d:.3e} K");
            assert!(d < 1e-6, "(p,h) round trip at {t_k} K left {d} K");
        }
    }

    /// Guard behaviour: states outside the accepted envelope are rejected
    /// rather than silently extrapolated.
    #[test]
    fn out_of_envelope_states_are_rejected() {
        assert!(
            helium_state(t(1.0), p(3.0e6)).is_err(),
            "below the triple point"
        );
        assert!(helium_state(t(5000.0), p(3.0e6)).is_err(), "above 1500 K");
        assert!(helium_state(t(500.0), p(0.0)).is_err(), "zero pressure");
        assert!(helium_state(t(500.0), p(1.0e12)).is_err(), "above 100 MPa");
        assert!(
            helium_state(t(f64::NAN), p(3.0e6)).is_err(),
            "NaN temperature"
        );
        assert!(
            helium_state(t(500.0), p(f64::INFINITY)).is_err(),
            "infinite pressure"
        );
    }

    /// Print the helium state across the HTR-10 core temperature rise.
    /// Not an assertion — this is the harness the measured numbers in the
    /// V&V tests below were taken from (2026-08-11).
    #[test]
    fn measure_htr10_states() {
        for t_c in [250.0_f64, 350.0, 450.0, 550.0, 650.0, 700.0] {
            let s = helium_state(t(t_c + 273.15), htr10_design_point::pressure()).unwrap();
            println!(
                "T={:6.1} C  rho={:.6} kg/m3  mu={:.6e} Pa.s  k={:.6} W/mK  \
                 cp={:.3} J/kgK  Pr={:.6}  a={:.2} m/s",
                t_c,
                s.density.get::<kilogram_per_cubic_meter>(),
                s.dynamic_viscosity.get::<pascal_second>(),
                s.thermal_conductivity.get::<watt_per_meter_kelvin>(),
                s.specific_heat_cp.get::<joule_per_kilogram_kelvin>(),
                s.prandtl().get::<ratio>(),
                s.speed_of_sound.get::<meter_per_second>(),
            );
        }
    }
}
