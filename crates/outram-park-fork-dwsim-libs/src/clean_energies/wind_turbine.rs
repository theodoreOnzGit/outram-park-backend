//! Wind turbine: Betz-limited rotor power from air density, wind speed and
//! swept area.
//!
//! # Attribution
//!
//! - **Upstream project:** DWSIM — Open Source Process Simulator
//! - **Source file:** `DWSIM.UnitOperations/UnitOperations/CleanEnergies/WindTurbine.vb`
//! - **Upstream commit:** `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`)
//! - **Upstream copyright:** Daniel Wagner O. de Medeiros and the DWSIM contributors
//! - **Upstream licence:** GPL-3.0
//! - **This port:** GPL-3.0-only (OUTRAM PARK fork; not the official DWSIM software)
//!
//! # The model, in full
//!
//! Two lines of arithmetic (`WindTurbine.vb:404-406`):
//!
//! `P_max = N * (8/27) * rho * v^3 * A`
//!
//! `P = P_max * eta / 100`
//!
//! with `N` turbines, air density `rho` \[kg/m³\], wind speed `v` \[m/s\],
//! swept disk area `A` \[m²\] and efficiency `eta` as a percentage. Upstream
//! divides by 1000 to report kW.
//!
//! The `8/27` factor is the **Betz limit**: the maximum extractable fraction
//! of the wind's kinetic-energy flux is `16/27`, and the flux itself is
//! `(1/2) rho A v^3`, so `(16/27)(1/2) = 8/27`. DWSIM's
//! `MaximumTheoreticalPower` is therefore genuinely the Betz power, not the
//! raw wind power — and `Efficiency` is efficiency *relative to Betz*, not
//! an absolute power coefficient `C_p`. A DWSIM `Efficiency` of 80 %
//! corresponds to `C_p = 0.8 * 16/27 = 0.474`, which is a realistic modern
//! large turbine.
//!
//! # What this model does NOT have
//!
//! **There is no cut-in speed, no rated power / pitch-regulated plateau, and
//! no cut-out speed.** The output is a pure cubic in wind speed for all
//! `v >= 0`, so at storm wind speeds it predicts unbounded power that no
//! real turbine would produce, and at 1 m/s it predicts a small positive
//! power where a real turbine would be parked. Anyone using this for an
//! annual-energy estimate must impose the turbine's own power curve
//! externally. This is stated plainly because it is the single most
//! important limitation of the ported model, and it would be easy to assume
//! otherwise from the unit's name.
//!
//! # Air density (flash boundary)
//!
//! DWSIM computes the humid-air density by building a two-component
//! `Air`/`Water` material stream, flashing it with a Raoult property package
//! to get the saturated water mole fraction, scaling that by the relative
//! humidity, rebuilding the stream at the humid composition, and reading the
//! vapour-phase density (`WindTurbine.vb:362-396`). Following the crate
//! convention, **none of that flash work happens here**: `air_density` is an
//! input to [`generated_power`]. The one piece of that sequence that is pure
//! arithmetic — scaling the saturated mole fraction by relative humidity
//! (`:381`) — is ported as [`humid_air_water_mole_fraction`], so a caller
//! driving [`crate::thermo`] can follow DWSIM's recipe exactly.
//!
//! # Excluded DWSIM behaviour
//!
//! Beyond the module-wide exclusions in [`crate::clean_energies`], this file
//! drops the weather-service lookup (`:348-351`), the `Calculator` /
//! `RaoultPropertyPackage` stream construction and disposal (`:362-396`) —
//! pushed to the caller as described above — and the outlet energy-stream
//! write at `:408`.

use uom::si::area::square_meter;
use uom::si::f64::{Area, Length, MassDensity, Power, Ratio, Velocity};
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::power::watt;
use uom::si::ratio::ratio;
use uom::si::velocity::meter_per_second;

use super::{CleanEnergyUnitOp, WeatherSource};

/// Betz coefficient `16/27` — the maximum fraction of the wind's
/// kinetic-energy flux any actuator disk can extract (Betz 1919).
///
/// DWSIM never spells this out; it folds `(16/27) * (1/2) = 8/27` into the
/// single literal `8.0 / 27.0` at `WindTurbine.vb:404`. It is named here
/// because the folded constant is otherwise unrecognisable.
pub const BETZ_LIMIT: f64 = 16.0 / 27.0;

/// Wind-turbine siting class — DWSIM's `EquipmentTypes` list
/// (`WindTurbine.vb:29-33`: `{"", "Onshore", "Offshore"}`).
///
/// **Descriptive only.** DWSIM's `Calculate` never branches on it. Carried so
/// the flowsheet metadata survives the port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindTurbineSiting {
    /// DWSIM's empty first entry — no siting chosen.
    #[default]
    Unspecified,
    /// Land-based installation.
    Onshore,
    /// Sea-based installation.
    Offshore,
}

/// Swept disk area from rotor diameter: `A = pi D² / 4`
/// (`WindTurbine.vb:399`).
///
/// DWSIM keeps `DiskArea` and `RotorDiameter` mutually consistent: whichever
/// the user sets last, the other is recomputed (`:398-402`, `:518-523`).
/// `rotor_diameter` is the full tip-to-tip diameter in metres.
pub fn disk_area_from_rotor_diameter(rotor_diameter: Length) -> Area {
    let d = rotor_diameter.get::<meter>();
    Area::new::<square_meter>(core::f64::consts::PI * d * d / 4.0)
}

/// Rotor diameter from swept disk area: `D = sqrt(4 A / pi)`
/// (`WindTurbine.vb:401`) — the inverse of
/// [`disk_area_from_rotor_diameter`].
///
/// `disk_area` must be `>= 0`; a negative area yields `NaN`.
pub fn rotor_diameter_from_disk_area(disk_area: Area) -> Length {
    let a = disk_area.get::<square_meter>();
    Length::new::<meter>((a * 4.0 / core::f64::consts::PI).sqrt())
}

/// Water mole fraction of humid air, from the saturated value and the
/// relative humidity — `WindTurbine.vb:381` (`wc = wc * rh / 100.0`).
///
/// `saturated_water_mole_fraction` is the water mole fraction in air
/// saturated at the ambient temperature and pressure (DWSIM obtains it by
/// flashing an `Air`/`Water` stream, `:368-378`); `relative_humidity` is the
/// dimensionless fraction in `[0, 1]` — note DWSIM stores it as a
/// **percentage** (default 30, `:58`) and divides by 100 here, so pass `0.30`
/// where DWSIM's editor shows `30`.
///
/// Returns the humid-air water mole fraction, which the caller feeds back
/// into its property package to obtain the density DWSIM then reads
/// (`:383-393`). This function is the *only* part of DWSIM's air-density
/// sequence that is not a flash, and it is exposed so the recipe can be
/// followed exactly.
pub fn humid_air_water_mole_fraction(
    saturated_water_mole_fraction: Ratio,
    relative_humidity: Ratio,
) -> Ratio {
    saturated_water_mole_fraction * relative_humidity.get::<ratio>()
}

/// Betz-limit power of a wind farm \[W\] — `WindTurbine.vb:404`.
///
/// `P_max = N * (8/27) * rho * v^3 * A`
///
/// This is the theoretical maximum an ideal actuator disk could extract, not
/// what any real turbine delivers; use [`generated_power`] for that.
///
/// # Inputs
///
/// - `air_density` — `rho` \[kg/m³\]. About 1.225 at sea level and 15 °C,
///   falling with altitude and temperature; DWSIM computes it from a humid-air
///   flash (see the module note).
/// - `wind_speed` — `v` \[m/s\] at hub height, `>= 0`. The cubic dependence
///   makes this by far the most sensitive input: a 10 % error in `v` is a
///   33 % error in power.
/// - `disk_area` — swept area `A` \[m²\], from
///   [`disk_area_from_rotor_diameter`].
/// - `number_of_turbines` — `N` identical turbines (`:74`, DWSIM default 1).
///
/// See the module docs: **no cut-in, rated or cut-out behaviour is applied**,
/// because DWSIM implements none.
pub fn maximum_theoretical_power(
    air_density: MassDensity,
    wind_speed: Velocity,
    disk_area: Area,
    number_of_turbines: u32,
) -> Power {
    let rho = air_density.get::<kilogram_per_cubic_meter>();
    let v = wind_speed.get::<meter_per_second>();
    let a = disk_area.get::<square_meter>();
    let n = f64::from(number_of_turbines);
    Power::new::<watt>(n * (BETZ_LIMIT / 2.0) * rho * v * v * v * a)
}

/// Electrical power actually generated \[W\] — `WindTurbine.vb:406`.
///
/// `P = P_max * eta`
///
/// `efficiency` is the fraction of the **Betz** power captured, dimensionless
/// in `[0, 1]`. DWSIM stores it as a percentage with default 80 (`:72`) and
/// divides by 100; this port takes the fraction, so pass `0.80` where DWSIM's
/// editor shows `80`. Because the reference is Betz and not the raw wind
/// power, `eta = 1` is physically attainable in principle (it means a
/// perfect actuator disk), unlike an absolute power coefficient which cannot
/// exceed `16/27`.
///
/// The remaining arguments are as for [`maximum_theoretical_power`].
pub fn generated_power(
    air_density: MassDensity,
    wind_speed: Velocity,
    disk_area: Area,
    number_of_turbines: u32,
    efficiency: Ratio,
) -> Power {
    maximum_theoretical_power(air_density, wind_speed, disk_area, number_of_turbines)
        * efficiency.get::<ratio>()
}

/// A configured wind-turbine unit operation — the ported subset of DWSIM's
/// `WindTurbine` class state (`WindTurbine.vb:29-80`).
///
/// Owns everything by value. Defaults follow DWSIM's own (`:52-78`): 10 m/s
/// wind, 10 m² disk, 80 % efficiency, one turbine — except air density, which
/// this port defaults to sea-level 1.225 kg/m³ because DWSIM computes it from
/// a flash this crate does not perform (see the module note).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindTurbine {
    /// Onshore/offshore siting (descriptive only — see
    /// [`WindTurbineSiting`]).
    pub siting: WindTurbineSiting,
    /// Where the caller says the ambient conditions came from
    /// (`CleanEnergyUnitOpBase.vb:12`). Changes no arithmetic.
    pub weather_source: WeatherSource,
    /// Hub-height wind speed `v` \[m/s\] (`:52, :60`, DWSIM default 10 m/s).
    pub wind_speed: Velocity,
    /// Humid-air density `rho` \[kg/m³\] — DWSIM's computed `AirDensity`
    /// (`:76, :393`), an input here (flash boundary).
    pub air_density: MassDensity,
    /// Swept disk area `A` \[m²\] (`:68`, DWSIM default 10 m²). Kept
    /// consistent with the rotor diameter by
    /// [`disk_area_from_rotor_diameter`].
    pub disk_area: Area,
    /// Number of identical turbines `N` (`:74`, DWSIM default 1).
    pub number_of_turbines: u32,
    /// Fraction of Betz power captured `eta`, dimensionless `[0, 1]`
    /// (`:72`, DWSIM default 80 %, stored here as 0.80).
    pub efficiency: Ratio,
}

impl Default for WindTurbine {
    fn default() -> Self {
        Self {
            siting: WindTurbineSiting::Unspecified,
            weather_source: WeatherSource::Global,
            wind_speed: Velocity::new::<meter_per_second>(0.0),
            air_density: MassDensity::new::<kilogram_per_cubic_meter>(1.225),
            disk_area: Area::new::<square_meter>(10.0),
            number_of_turbines: 1,
            efficiency: Ratio::new::<ratio>(0.80),
        }
    }
}

impl WindTurbine {
    /// Betz-limit power for this turbine's current configuration \[W\] —
    /// DWSIM's `MaximumTheoreticalPower` property (`:80, :404`). See
    /// [`maximum_theoretical_power`].
    pub fn maximum_theoretical_power(&self) -> Power {
        maximum_theoretical_power(
            self.air_density,
            self.wind_speed,
            self.disk_area,
            self.number_of_turbines,
        )
    }

    /// Rotor diameter implied by this turbine's swept area \[m\] — DWSIM's
    /// `RotorDiameter` property (`:70, :401`). See
    /// [`rotor_diameter_from_disk_area`].
    pub fn rotor_diameter(&self) -> Length {
        rotor_diameter_from_disk_area(self.disk_area)
    }
}

impl CleanEnergyUnitOp for WindTurbine {
    /// `"Wind Turbine"` — `WindTurbine.vb:82-84`.
    fn display_name(&self) -> &'static str {
        "Wind Turbine"
    }

    /// `"WT-"` — `WindTurbine.vb:50`.
    fn prefix(&self) -> &'static str {
        "WT-"
    }

    /// Generated electrical power \[W\] — `WindTurbine.vb:406`, evaluated
    /// from the struct's own fields via [`generated_power`]. A
    /// default-constructed turbine sits in still air and reports 0 W.
    fn generated_power(&self) -> Power {
        generated_power(
            self.air_density,
            self.wind_speed,
            self.disk_area,
            self.number_of_turbines,
            self.efficiency,
        )
    }
}

#[cfg(test)]
mod tests {
    //! # Verification tests (methodology + measured results)
    //!
    //! **Verification, not validation** — these confirm the port reproduces
    //! `WindTurbine.vb:399-406`, including the *absence* of any cut-in /
    //! rated / cut-out behaviour. No comparison against a manufacturer power
    //! curve has been made; the model could not pass one. Measured
    //! 2026-08-11 with `cargo test --release`.
    use super::*;
    use approx::assert_relative_eq;

    /// Methodology: sea-level air (1.225 kg/m³), 12 m/s wind, a 90 m rotor,
    /// one turbine, 80 % of Betz. Hand calculation:
    /// `A = pi * 90² / 4 = 6361.725124 m²`;
    /// `P_max = (8/27) * 1.225 * 12³ * 6361.725124`;
    /// `P = 0.8 * P_max`.
    ///
    /// Results (2026-08-11): `A = 6361.725124 m²`,
    /// `P_max = 3990073.997 W` (3.990 MW), `P = 3192059.198 W`
    /// (3.192 MW) — a plausible rating for a 90 m machine at 12 m/s.
    #[test]
    fn power_at_a_realistic_operating_point() {
        let area = disk_area_from_rotor_diameter(Length::new::<meter>(90.0));
        assert_relative_eq!(area.get::<square_meter>(), 6361.725124, epsilon = 1e-5);

        let rho = MassDensity::new::<kilogram_per_cubic_meter>(1.225);
        let v = Velocity::new::<meter_per_second>(12.0);
        let p_max = maximum_theoretical_power(rho, v, area, 1).get::<watt>();
        let p = generated_power(rho, v, area, 1, Ratio::new::<ratio>(0.80)).get::<watt>();

        assert_relative_eq!(p_max, 3_990_073.997, epsilon = 1.0);
        assert_relative_eq!(p, 0.8 * p_max, epsilon = 1e-6);
        assert_relative_eq!(p, 3_192_059.198, epsilon = 1.0);
    }

    /// Methodology — **power versus wind speed**, the cubic law of
    /// `WindTurbine.vb:404`. Sweeping 0, 3, 5, 12, 25 and 40 m/s at fixed
    /// density, area and efficiency, the output must be (a) zero at zero
    /// wind, (b) strictly increasing, and (c) exactly cubic:
    /// `P(2v) / P(v) = 8` for any `v > 0`.
    ///
    /// **This test also documents what upstream does NOT do.** A real
    /// turbine has a cut-in speed (~3 m/s, below which it produces nothing),
    /// a rated speed (~12 m/s, above which output is held flat by pitch
    /// control), and a cut-out speed (~25 m/s, above which it shuts down).
    /// DWSIM implements none of the three, so the port must not either — the
    /// assertions below deliberately confirm the *unclamped* cubic all the
    /// way to 40 m/s.
    ///
    /// Results (2026-08-11), 1.225 kg/m³, 100 m², eta = 0.8, giving
    /// `P = 29.03704 v³` W: `v = 0 -> 0 W`; `3 -> 784.0 W`;
    /// `5 -> 3629.6 W`; `12 -> 50176.0 W`; `25 -> 453703.7 W`;
    /// `40 -> 1858370.4 W`. Monotonic throughout; no plateau above 12 m/s
    /// and no shutdown above 25 m/s — the model keeps rising, reaching
    /// 1.86 MW at gale force from a 100 m² disk, which no real turbine would
    /// deliver. `P(24)/P(12) = 8.000000` and `P(10)/P(5) = 8.000000`,
    /// exactly cubic.
    #[test]
    fn power_follows_an_unclamped_cubic_in_wind_speed() {
        let rho = MassDensity::new::<kilogram_per_cubic_meter>(1.225);
        let area = Area::new::<square_meter>(100.0);
        let eta = Ratio::new::<ratio>(0.80);
        let p = |v: f64| {
            generated_power(rho, Velocity::new::<meter_per_second>(v), area, 1, eta).get::<watt>()
        };

        // (a) still air produces nothing.
        assert_relative_eq!(p(0.0), 0.0, epsilon = 1e-15);

        // (b) strictly increasing across the whole range, including past the
        // speeds where a real turbine would plateau (12 m/s) or cut out
        // (25 m/s) -- upstream implements neither.
        let speeds = [3.0, 5.0, 12.0, 25.0, 40.0];
        for w in speeds.windows(2) {
            assert!(
                p(w[1]) > p(w[0]),
                "power must keep rising: P({}) = {} is not > P({}) = {}",
                w[1],
                p(w[1]),
                w[0],
                p(w[0])
            );
        }

        // (c) exactly cubic: doubling the wind speed multiplies power by 8.
        assert_relative_eq!(p(24.0) / p(12.0), 8.0, epsilon = 1e-12);
        assert_relative_eq!(p(10.0) / p(5.0), 8.0, epsilon = 1e-12);

        // Spot values: P = 29.03704 v^3 W for this configuration.
        assert_relative_eq!(p(3.0), 784.0, epsilon = 0.01);
        assert_relative_eq!(p(12.0), 50_176.0, epsilon = 0.1);
        assert_relative_eq!(p(40.0), 1_858_370.4, epsilon = 1.0);
    }

    /// Methodology: the disk-area / rotor-diameter pair must round-trip
    /// (`:399`, `:401`), matching DWSIM's mutual-consistency rule at
    /// `:398-402`. Start from `D = 90 m`, convert to area and back.
    /// Result (2026-08-11): `D -> A = 6361.725124 m² -> D = 90.000000 m`,
    /// round-trip exact to 1e-12.
    #[test]
    fn disk_area_and_rotor_diameter_round_trip() {
        let d0 = Length::new::<meter>(90.0);
        let a = disk_area_from_rotor_diameter(d0);
        let d1 = rotor_diameter_from_disk_area(a);
        assert_relative_eq!(d1.get::<meter>(), 90.0, epsilon = 1e-12);
    }

    /// Methodology: the Betz constant folded into DWSIM's `8.0 / 27.0`
    /// (`:404`) must equal `(16/27)/2`, confirming the ported factorisation
    /// is arithmetically identical to upstream's literal.
    /// Result (2026-08-11): `BETZ_LIMIT / 2 = 0.296296…` and
    /// `8/27 = 0.296296…`, equal to 1e-15.
    #[test]
    fn folded_betz_constant_matches_upstream_literal() {
        assert_relative_eq!(BETZ_LIMIT / 2.0, 8.0 / 27.0, epsilon = 1e-15);
    }

    /// Methodology: humidity scaling (`:381`). A saturated water mole
    /// fraction of 0.031 at 30 % relative humidity gives `0.0093`. Also
    /// checks that `N` turbines scale the farm output linearly (`:404`).
    /// Results (2026-08-11): mole fraction `0.009300`; 5 turbines give
    /// exactly `5.000000` times one turbine's output.
    #[test]
    fn humidity_scaling_and_turbine_count_scaling() {
        let wc =
            humid_air_water_mole_fraction(Ratio::new::<ratio>(0.031), Ratio::new::<ratio>(0.30));
        assert_relative_eq!(wc.get::<ratio>(), 0.0093, epsilon = 1e-12);

        let rho = MassDensity::new::<kilogram_per_cubic_meter>(1.225);
        let v = Velocity::new::<meter_per_second>(10.0);
        let a = Area::new::<square_meter>(100.0);
        let eta = Ratio::new::<ratio>(0.8);
        let one = generated_power(rho, v, a, 1, eta).get::<watt>();
        let five = generated_power(rho, v, a, 5, eta).get::<watt>();
        assert_relative_eq!(five / one, 5.0, epsilon = 1e-12);
    }

    /// Methodology: a default turbine sits in still air, so it must report
    /// 0 W; names must match `WindTurbine.vb:82-84` and `:50`.
    /// Result (2026-08-11): `0 W`, `"Wind Turbine"`, `"WT-"`; default
    /// 10 m² disk gives a rotor diameter of `3.568248 m`.
    #[test]
    fn default_turbine_is_becalmed_and_names_match_upstream() {
        let t = WindTurbine::default();
        assert_eq!(t.generated_power().get::<watt>(), 0.0);
        assert_eq!(t.maximum_theoretical_power().get::<watt>(), 0.0);
        assert_eq!(t.display_name(), "Wind Turbine");
        assert_eq!(t.prefix(), "WT-");
        assert_relative_eq!(t.rotor_diameter().get::<meter>(), 3.5682482, epsilon = 1e-6);
    }
}
