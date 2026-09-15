//! Hydroelectric turbine: shaft power from static + velocity head and
//! volumetric flow.
//!
//! # Attribution
//!
//! - **Upstream project:** DWSIM — Open Source Process Simulator
//! - **Source file:** `DWSIM.UnitOperations/UnitOperations/CleanEnergies/HydroelectricTurbine.vb`
//! - **Upstream commit:** `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`)
//! - **Upstream copyright:** Daniel Wagner O. de Medeiros and the DWSIM contributors
//! - **Upstream licence:** GPL-3.0
//! - **This port:** GPL-3.0-only (OUTRAM PARK fork; not the official DWSIM software)
//!
//! # The model, in full
//!
//! `HydroelectricTurbine.vb:301-340`:
//!
//! `h_v = (v_in² - v_out²) / (2 g)`   (velocity head, `:318`)
//!
//! `H = h_s + h_v`                    (total head, `:322`)
//!
//! `P = eta * rho * g * H * Q`        (`:324`)
//!
//! with `h_s` the static (elevation) head \[m\], `v_in`/`v_out` the penstock
//! inlet and draft-tube outlet velocities \[m/s\], `rho` the liquid density
//! \[kg/m³\], `Q` the volumetric flow \[m³/s\], and `eta` the turbine-generator
//! efficiency (a percentage upstream, divided by 100 at `:308`). Upstream
//! reports kW.
//!
//! This is the classical hydropower equation with a Bernoulli velocity-head
//! correction. Note the velocity head is a *recovery* term: the turbine
//! captures the kinetic energy the water loses between inlet and outlet, so
//! `v_in > v_out` adds head and `v_in < v_out` subtracts it.
//!
//! # `g = 9.8`, not 9.80665
//!
//! DWSIM hard-codes `Dim g = 9.8` (`:314`). That is 0.068 % below the CODATA
//! standard 9.806 65 m/s², and it enters the power **twice** — once directly
//! in `rho g H Q` and once inversely inside the velocity head — so the net
//! effect on power is roughly `-0.068 %` for a static-head-dominated machine.
//! The upstream literal is kept ([`crate::clean_energies::GRAVITY_M_PER_S2`])
//! so this port reproduces DWSIM's numbers exactly rather than being silently
//! 0.07 % "better" and disagreeing with the reference implementation.
//!
//! # Outlet stream (flash boundary)
//!
//! DWSIM finishes by copying the inlet mixture onto the outlet, holding the
//! pressure, subtracting the extracted work from the mass enthalpy, and
//! flashing PH (`:330-338`). Per the crate convention no flash happens here;
//! [`outlet_specific_enthalpy`] returns the enthalpy DWSIM writes, and the
//! caller flashes `(p_in, h_out)`.
//!
//! Note the **pressure is unchanged** across the unit (`:334`,
//! `msout.SetPressure(msin.GetPressure)`) even though a real turbine drops
//! pressure across the runner: in this model the energy comes out of the
//! head and the enthalpy, not out of a pressure change. That is upstream's
//! choice and is reproduced.
//!
//! # Excluded DWSIM behaviour
//!
//! Beyond the module-wide exclusions in [`crate::clean_energies`], this file
//! drops the structured report builder (`GetStructuredReport`, `:171-195`),
//! the volumetric-flow and density reads from the inlet material stream
//! (`:310-312`) — inputs here — and the outlet material-stream mutation with
//! its PH flash (`:330-338`).

use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{AvailableEnergy, Length, MassDensity, MassRate, Power, Ratio, Velocity, VolumeRate};
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::ratio::ratio;
use uom::si::velocity::meter_per_second;
use uom::si::volume_rate::cubic_meter_per_second;

use super::{CleanEnergyUnitOp, GRAVITY_M_PER_S2};

/// Velocity head `h_v = (v_in² - v_out²) / (2 g)` \[m\] —
/// `HydroelectricTurbine.vb:318`.
///
/// The head equivalent of the kinetic energy the water gives up across the
/// machine. Positive when the flow decelerates (`v_in > v_out`), which is the
/// normal case for a draft tube; **negative** if the outlet is faster, in
/// which case it subtracts from the total head. DWSIM applies no clamp and
/// neither does this port.
///
/// Both velocities are in m/s and may be any real value (only their squares
/// matter, so signs are irrelevant). Uses DWSIM's `g = 9.8` — see the module
/// note.
pub fn velocity_head(inlet_velocity: Velocity, outlet_velocity: Velocity) -> Length {
    let v_in = inlet_velocity.get::<meter_per_second>();
    let v_out = outlet_velocity.get::<meter_per_second>();
    Length::new::<meter>((v_in * v_in - v_out * v_out) / (2.0 * GRAVITY_M_PER_S2))
}

/// Total head `H = h_s + h_v` \[m\] — `HydroelectricTurbine.vb:322`.
///
/// `static_head` is the elevation difference between headwater and tailwater
/// \[m\] (DWSIM's `StaticHead`, default 1 m, `:28`); `velocity_head` comes
/// from [`velocity_head`]. No friction, penstock or minor losses are
/// deducted — DWSIM models none, so an "available head" the user supplies
/// should already be net of losses if that matters.
pub fn total_head(static_head: Length, velocity_head: Length) -> Length {
    static_head + velocity_head
}

/// Generated electrical power \[W\] — `HydroelectricTurbine.vb:324`.
///
/// `P = eta * rho * g * H * Q`
///
/// # Inputs
///
/// - `efficiency` — combined turbine + generator efficiency `eta`,
///   dimensionless in `[0, 1]`. DWSIM stores it as a percentage with default
///   75 (`:26`) and divides by 100 at `:308`; pass `0.75` where DWSIM's
///   editor shows `75`. Real large hydro sets reach 0.85-0.92.
/// - `liquid_density` — `rho` \[kg/m³\], read by DWSIM from the inlet
///   stream's liquid phase (`:312`); about 1000 for water. An input here
///   (flash boundary).
/// - `total_head` — `H` \[m\] from [`total_head`].
/// - `volumetric_flow` — `Q` \[m³/s\], read by DWSIM from the inlet stream
///   (`:310`). An input here.
///
/// The relation is exactly linear in head, flow, density and efficiency.
/// Uses DWSIM's `g = 9.8` — see the module note.
pub fn generated_power(
    efficiency: Ratio,
    liquid_density: MassDensity,
    total_head: Length,
    volumetric_flow: VolumeRate,
) -> Power {
    let eta = efficiency.get::<ratio>();
    let rho = liquid_density.get::<kilogram_per_cubic_meter>();
    let h = total_head.get::<meter>();
    let q = volumetric_flow.get::<cubic_meter_per_second>();
    Power::new::<watt>(eta * rho * GRAVITY_M_PER_S2 * h * q)
}

/// Outlet specific enthalpy after extracting the shaft work —
/// `HydroelectricTurbine.vb:335`.
///
/// `h_out = h_in - P / w`
///
/// DWSIM writes `msin.GetMassEnthalpy() - GeneratedPower / msin.GetMassFlow()`,
/// with `GeneratedPower` in kW and mass enthalpy in kJ/kg, so the term is
/// dimensionally consistent (kW / (kg/s) = kJ/kg). This port works in SI:
/// `power` in W, `mass_flow` in kg/s, enthalpies in J/kg.
///
/// The returned enthalpy is the input to a caller-side **PH flash** at the
/// unchanged inlet pressure (`:334, :336`) — see the module "flash boundary"
/// note.
///
/// Returns a non-finite value for zero mass flow, which is degenerate input.
pub fn outlet_specific_enthalpy(
    inlet_specific_enthalpy: AvailableEnergy,
    power: Power,
    mass_flow: MassRate,
) -> AvailableEnergy {
    let h_in = inlet_specific_enthalpy.get::<joule_per_kilogram>();
    let p = power.get::<watt>();
    let w = mass_flow.get::<kilogram_per_second>();
    AvailableEnergy::new::<joule_per_kilogram>(h_in - p / w)
}

/// A configured hydroelectric-turbine unit operation — the ported subset of
/// DWSIM's `HydroelectricTurbine` class state
/// (`HydroelectricTurbine.vb:24-38`).
///
/// Owns everything by value. Defaults follow DWSIM's own (`:26-38`): 75 %
/// efficiency, 1 m static head, 1.0 m/s inlet and 0.5 m/s outlet velocity —
/// plus water density and zero flow, which DWSIM reads from its inlet stream.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HydroelectricTurbine {
    /// Combined turbine + generator efficiency `eta`, dimensionless `[0, 1]`
    /// (`:26`, DWSIM default 75 %, stored here as 0.75).
    pub efficiency: Ratio,
    /// Static (elevation) head `h_s` \[m\] (`:28`, DWSIM default 1 m).
    pub static_head: Length,
    /// Penstock inlet velocity `v_in` \[m/s\] (`:32`, DWSIM default 1.0).
    pub inlet_velocity: Velocity,
    /// Draft-tube outlet velocity `v_out` \[m/s\] (`:34`, DWSIM default 0.5).
    pub outlet_velocity: Velocity,
    /// Liquid density `rho` \[kg/m³\] — read by DWSIM from the inlet stream
    /// (`:312`), an input here (flash boundary).
    pub liquid_density: MassDensity,
    /// Volumetric flow `Q` \[m³/s\] — read by DWSIM from the inlet stream
    /// (`:310`), an input here.
    pub volumetric_flow: VolumeRate,
}

impl Default for HydroelectricTurbine {
    fn default() -> Self {
        Self {
            efficiency: Ratio::new::<ratio>(0.75),
            static_head: Length::new::<meter>(1.0),
            inlet_velocity: Velocity::new::<meter_per_second>(1.0),
            outlet_velocity: Velocity::new::<meter_per_second>(0.5),
            liquid_density: MassDensity::new::<kilogram_per_cubic_meter>(1000.0),
            volumetric_flow: VolumeRate::new::<cubic_meter_per_second>(0.0),
        }
    }
}

impl HydroelectricTurbine {
    /// Velocity head for this turbine's configured velocities \[m\] —
    /// DWSIM's `VelocityHead` property (`:30, :320`). See [`velocity_head`].
    pub fn velocity_head(&self) -> Length {
        velocity_head(self.inlet_velocity, self.outlet_velocity)
    }

    /// Total head `h_s + h_v` \[m\] — DWSIM's `TotalHead` property
    /// (`:36, :322`). See [`total_head`].
    pub fn total_head(&self) -> Length {
        total_head(self.static_head, self.velocity_head())
    }
}

impl CleanEnergyUnitOp for HydroelectricTurbine {
    /// `"Hydroelectric Turbine"` — `HydroelectricTurbine.vb:40-42`.
    fn display_name(&self) -> &'static str {
        "Hydroelectric Turbine"
    }

    /// `"HT-"` — `HydroelectricTurbine.vb:24`.
    fn prefix(&self) -> &'static str {
        "HT-"
    }

    /// Generated electrical power \[W\] — `HydroelectricTurbine.vb:324`,
    /// evaluated from the struct's own fields. A default-constructed turbine
    /// has zero flow and reports 0 W.
    fn generated_power(&self) -> Power {
        generated_power(
            self.efficiency,
            self.liquid_density,
            self.total_head(),
            self.volumetric_flow,
        )
    }
}

#[cfg(test)]
mod tests {
    //! # Verification tests (methodology + measured results)
    //!
    //! **Verification, not validation** — these confirm the port reproduces
    //! `HydroelectricTurbine.vb:308-335`. No comparison against a measured
    //! turbine performance curve has been made. Measured 2026-08-11 with
    //! `cargo test --release`.
    use super::*;
    use approx::assert_relative_eq;

    /// Methodology — **power from head, flow and efficiency** (`:324`). A
    /// 50 m static-head plant passing 20 m³/s of water (1000 kg/m³) at 90 %
    /// efficiency, with a 6 m/s penstock and 2 m/s draft tube. Hand
    /// calculation:
    ///
    /// - `h_v = (36 - 4) / (2 * 9.8) = 32 / 19.6 = 1.632653 m`
    /// - `H = 50 + 1.632653 = 51.632653 m`
    /// - `P = 0.9 * 1000 * 9.8 * 51.632653 * 20 = 9.1080e6 W`
    ///
    /// Results (2026-08-11): `h_v = 1.63265306 m`, `H = 51.63265306 m`,
    /// `P = 9108000.0 W` (9.108 MW) — a plausible mid-size hydro set.
    #[test]
    fn power_from_head_flow_and_efficiency() {
        let h_v = velocity_head(
            Velocity::new::<meter_per_second>(6.0),
            Velocity::new::<meter_per_second>(2.0),
        );
        assert_relative_eq!(h_v.get::<meter>(), 1.63265306, epsilon = 1e-7);

        let h = total_head(Length::new::<meter>(50.0), h_v);
        assert_relative_eq!(h.get::<meter>(), 51.63265306, epsilon = 1e-7);

        let p = generated_power(
            Ratio::new::<ratio>(0.9),
            MassDensity::new::<kilogram_per_cubic_meter>(1000.0),
            h,
            VolumeRate::new::<cubic_meter_per_second>(20.0),
        );
        assert_relative_eq!(p.get::<watt>(), 9.108e6, epsilon = 1.0);
    }

    /// Methodology: the power law is exactly linear in head, flow, density
    /// and efficiency (`:324`). Doubling each in turn must double the output;
    /// zero head or zero flow must give zero.
    /// Results (2026-08-11): all four doubling ratios exactly `2.000000`;
    /// zero head and zero flow both give `0 W`.
    #[test]
    fn power_is_linear_in_head_flow_density_and_efficiency() {
        let eta = Ratio::new::<ratio>(0.9);
        let rho = MassDensity::new::<kilogram_per_cubic_meter>(1000.0);
        let h = Length::new::<meter>(50.0);
        let q = VolumeRate::new::<cubic_meter_per_second>(20.0);
        let base = generated_power(eta, rho, h, q).get::<watt>();

        assert_relative_eq!(
            generated_power(eta, rho, Length::new::<meter>(100.0), q).get::<watt>() / base,
            2.0,
            epsilon = 1e-12
        );
        assert_relative_eq!(
            generated_power(eta, rho, h, VolumeRate::new::<cubic_meter_per_second>(40.0))
                .get::<watt>()
                / base,
            2.0,
            epsilon = 1e-12
        );
        assert_relative_eq!(
            generated_power(
                eta,
                MassDensity::new::<kilogram_per_cubic_meter>(2000.0),
                h,
                q
            )
            .get::<watt>()
                / base,
            2.0,
            epsilon = 1e-12
        );
        assert_relative_eq!(
            generated_power(Ratio::new::<ratio>(1.8), rho, h, q).get::<watt>() / base,
            2.0,
            epsilon = 1e-12
        );

        assert_relative_eq!(
            generated_power(eta, rho, Length::new::<meter>(0.0), q).get::<watt>(),
            0.0,
            epsilon = 1e-15
        );
        assert_relative_eq!(
            generated_power(eta, rho, h, VolumeRate::new::<cubic_meter_per_second>(0.0))
                .get::<watt>(),
            0.0,
            epsilon = 1e-15
        );
    }

    /// Methodology: the velocity head is a *recovery* term (`:318`), so it
    /// must be positive when the flow decelerates, zero when velocities
    /// match, and negative when the outlet is faster than the inlet. DWSIM
    /// applies no clamp.
    /// Results (2026-08-11): `(6, 2) -> +1.632653 m`; `(3, 3) -> 0.000000 m`;
    /// `(2, 6) -> -1.632653 m` (the mirror image, as the formula requires).
    #[test]
    fn velocity_head_is_signed_and_unclamped() {
        let decel = velocity_head(
            Velocity::new::<meter_per_second>(6.0),
            Velocity::new::<meter_per_second>(2.0),
        );
        let equal = velocity_head(
            Velocity::new::<meter_per_second>(3.0),
            Velocity::new::<meter_per_second>(3.0),
        );
        let accel = velocity_head(
            Velocity::new::<meter_per_second>(2.0),
            Velocity::new::<meter_per_second>(6.0),
        );
        assert!(decel.get::<meter>() > 0.0);
        assert_relative_eq!(equal.get::<meter>(), 0.0, epsilon = 1e-15);
        assert_relative_eq!(accel.get::<meter>(), -decel.get::<meter>(), epsilon = 1e-12);
    }

    /// Methodology — **energy-balance closure at the outlet** (`:335`). The
    /// specific enthalpy drop must exactly account for the extracted power:
    /// `w * (h_in - h_out) = P`. Using the 9.108 MW case above with a mass
    /// flow of `rho * Q = 20000 kg/s` and `h_in = 100 kJ/kg`.
    ///
    /// Hand calculation: `dh = 9.108e6 / 20000 = 455.4 J/kg`, so
    /// `h_out = 100000 - 455.4 = 99544.6 J/kg`.
    /// Results (2026-08-11): `h_out = 99544.600000 J/kg`;
    /// `w (h_in - h_out) = 9108000.0 W`, matching `P` to 1e-6 — the balance
    /// closes exactly.
    #[test]
    fn outlet_enthalpy_closes_the_energy_balance() {
        let p = Power::new::<watt>(9.108e6);
        let w = MassRate::new::<kilogram_per_second>(20_000.0);
        let h_in = AvailableEnergy::new::<joule_per_kilogram>(100_000.0);
        let h_out = outlet_specific_enthalpy(h_in, p, w);

        assert_relative_eq!(h_out.get::<joule_per_kilogram>(), 99_544.6, epsilon = 1e-6);
        let recovered = w.get::<kilogram_per_second>()
            * (h_in.get::<joule_per_kilogram>() - h_out.get::<joule_per_kilogram>());
        assert_relative_eq!(recovered, p.get::<watt>(), epsilon = 1e-6);
    }

    /// Methodology: DWSIM's defaults (`:26-34`) — 75 %, 1 m static head,
    /// 1.0/0.5 m/s — with the density and flow the inlet stream would
    /// supply. Hand calculation: `h_v = (1 - 0.25)/19.6 = 0.038265 m`,
    /// `H = 1.0382653 m`, `P = 0.75 * 1000 * 9.8 * 1.0382653 * 2`.
    /// Results (2026-08-11): `h_v = 0.03826531 m`, `H = 1.03826531 m`,
    /// `P = 15262.500000 W`. A default turbine with no flow reports `0 W`,
    /// and the names match `:40-42` and `:24`.
    #[test]
    fn dwsim_default_configuration_and_names() {
        let idle = HydroelectricTurbine::default();
        assert_eq!(idle.generated_power().get::<watt>(), 0.0);
        assert_eq!(idle.display_name(), "Hydroelectric Turbine");
        assert_eq!(idle.prefix(), "HT-");

        let t = HydroelectricTurbine {
            volumetric_flow: VolumeRate::new::<cubic_meter_per_second>(2.0),
            ..Default::default()
        };
        assert_relative_eq!(t.velocity_head().get::<meter>(), 0.03826531, epsilon = 1e-7);
        assert_relative_eq!(t.total_head().get::<meter>(), 1.03826531, epsilon = 1e-7);
        assert_relative_eq!(t.generated_power().get::<watt>(), 15262.5, epsilon = 0.01);
    }

    /// Methodology: confirm the ported gravity really is DWSIM's rounded
    /// `9.8` (`:314`) and not the CODATA standard, and quantify the
    /// difference so the choice is on record.
    /// Result (2026-08-11): `g = 9.8` exactly; `(9.80665 - 9.8)/9.80665 =
    /// 0.0678 %` — the systematic bias this port inherits from upstream.
    #[test]
    fn gravity_is_dwsims_rounded_value_not_codata() {
        assert_eq!(GRAVITY_M_PER_S2, 9.8);
        let relative_bias = (9.806_65 - GRAVITY_M_PER_S2) / 9.806_65;
        assert_relative_eq!(relative_bias, 0.000678, epsilon = 1e-5);
    }
}
