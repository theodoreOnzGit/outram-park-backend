//! RELAP5-style single-phase wall heat-transfer selection for hydraulic
//! geometries 101 (internal pipe flow) and 110 (vertical bundle of in-line
//! rods, parallel flow only). GitHub issue #1063, part of #1056.
//!
//! > Research, education and V&V only. AI draft, unreviewed. Not for
//! > facility operation, licensing or safety decisions.
//!
//! # Specification
//!
//! RELAP-7 Theory Manual (INL), §2.2.2.1–2.2.2.2, Eqs 45–51, which state
//! RELAP5's single-phase model for these two geometries. The manual is not
//! redistributable and is cited by section and equation only. Every
//! correlation it uses is a standard published one, cited at its function:
//!
//! | Leg | Correlation | Original source | Manual |
//! |---|---|---|---|
//! | forced laminar | `Nu = 4.36` | fully developed laminar tube flow, uniform wall flux (exact `48/11 = 4.3636`) | Eq. 45 |
//! | forced turbulent | Dittus–Boelter `Nu = C Re^0.8 Pr^n`, `C = 0.023`, `n = 0.4` heated / `0.3` cooled | Dittus & Boelter (1930) | Eq. 46 |
//! | free convection | Churchill–Chu vertical plate | Churchill & Chu (1975) | Eqs 47–48 |
//! | liquid metal, pipe | `Nu = 5.0 + 0.025 Pe^0.8` | Seban & Shimazaki (1951) | Eq. 49 |
//! | bundle multiplier | `C = 0.023 P/D`, `P/D` in `[1.1, 1.6]` | Inayatov (1975) | Eq. 50 |
//! | liquid metal, bundle | `Nu = 4.0 + 0.33 (P/D)^3.8 (Pe/100)^0.86 + 0.16 (P/D)^5`, `P/D` in `[1.1, 1.4]` | Kazimi & Carelli (1976), "Westinghouse correlation" | Eq. 51 |
//!
//! # Selection logic
//!
//! - **`Pr ≥ 0.1` (ordinary fluids):** the Nusselt number is the maximum of
//!   the forced-laminar, forced-turbulent and free-convection values, to
//!   avoid discontinuities between regimes. The turbulent leg is evaluated
//!   at **every** Reynolds number (no `Re > 2300` gate), as the manual
//!   states it. Geometry 110 differs only in the Inayatov multiplier on the
//!   turbulent leg.
//! - **`Pr < 0.1` (liquid metals):** one correlation for all regimes — Eq.
//!   49 for geometry 101, Eq. 51 for geometry 110. No max-selection.
//!
//! # Consequences of the specification worth knowing (found by the V&V)
//!
//! - **Dittus–Boelter overtakes the laminar value deep in laminar flow.**
//!   With the turbulent leg ungated, `0.023 Re^0.8 Pr^0.4 = 4.36` at
//!   `Re = (4.36/0.023/Pr^0.4)^1.25`: `Re ≈ 703` at `Pr = 1`, `Re ≈ 266` at
//!   `Pr = 7`. This is what the stated model does; it is not a defect here.
//! - **The `Pr = 0.1` switch is discontinuous.** At `Re = 10⁴`,
//!   Dittus–Boelter at `Pr = 0.1` gives `14.51`; Seban–Shimazaki just below
//!   gives `≈ 11.27`, a 22 % drop.
//!
//! # Decisions not fixed by the manual (stated, not hidden)
//!
//! - The heating direction is taken as "fluid heated" when
//!   `T_wall ≥ T_bulk` (a zero difference picks `n = 0.4`; the free
//!   convection leg is then at its conduction limit anyway).
//! - The Grashof number uses `|T_wall − T_bulk|`.
//! - An absent `P/D` defaults to 1.1 for the liquid-metal bundle form as
//!   well as for the Inayatov multiplier (the manual states the default for
//!   Eq. 50 and the clip range for Eq. 51).
//! - The selection compares Nusselt numbers on the hydraulic diameter. When
//!   the free-convection length `L` differs from `D_h`, the Churchill–Chu
//!   value (on `L`) is rescaled by `D_h/L` so the three legs compare the same
//!   coefficient `h`.
//! - Published validity ranges (Dittus–Boelter `Re ≳ 10⁴`; Churchill–Chu
//!   `Ra < 10¹²`; Kazimi–Carelli `10 ≤ Pe ≤ 5000`) are documented and not
//!   enforced, because the manual evaluates each correlation everywhere.
//!
//! # Reuse (searched 2026-11-01)
//!
//! Dittus–Boelter and Churchill–Chu come from `tuas_boussinesq_solver`
//! (`NusseltCorrelation::DittusBoelter`,
//! `NusseltCorrelation::ChurchillChuVerticalPlate`, both added for this
//! issue). The laminar constant is **not** taken from
//! `NusseltCorrelation::PipeConstantHeatFluxFullyDeveloped`, which returns
//! `4.354`, nor from `pipe_correlations::laminar_nusselt_uniform_heat_flux_fully_developed`,
//! which returns `4.36` but panics above `Re = 2300` and so cannot sit under
//! a max-selection; [`RELAP5_LAMINAR_NUSSELT`] is the manual's `4.36`.
//! GeN-Foam's generic `A + B Re^C Pr^D` HTC (`outram-foam-appbuilder-lib`,
//! `fs_htc.rs`) was checked and not used: tampines does not depend on that
//! crate, and it has no max-selection or bundle forms. No Seban–Shimazaki,
//! Inayatov or Kazimi–Carelli implementation existed in the workspace.
//!
//! V&V record: `crates/tampines/docs/relap5-htc-dispatcher-vv.md`.

use petir::mathf::RealMath;
use uom::si::acceleration::meter_per_second_squared;
use uom::si::f64::{
    Acceleration, DynamicViscosity, HeatTransfer, Length, MassDensity, Ratio,
    SpecificHeatCapacity, TemperatureCoefficient, TemperatureInterval, ThermalConductivity,
    ThermodynamicTemperature, Velocity,
};
use uom::si::length::meter;
use uom::si::ratio::ratio;
use uom::si::temperature_interval::kelvin as kelvin_interval;
use uom::si::thermodynamic_temperature::kelvin;

use tuas_boussinesq_solver::heat_transfer_correlations::nusselt_number_correlations::{
    enums::NusseltCorrelation, grashof_number, ChurchillChuData, DittusBoelterData,
    FluidHeatingDirection,
};

use crate::error::TampinesError;

/// Laminar Nusselt number for fully developed flow in a circular tube with
/// uniform wall heat flux, as RELAP5 / RELAP-7 use it (Theory Manual
/// §2.2.2.1, Eq. 45). The exact value is `48/11 = 4.3636…` (e.g. Incropera
/// & DeWitt, Eq. 8.53).
pub const RELAP5_LAMINAR_NUSSELT: f64 = 4.36;

/// Prandtl number below which a fluid is treated as a liquid metal
/// (Theory Manual §2.2.2.1–2.2.2.2). `Pr < 0.1` is liquid metal;
/// `Pr = 0.1` is an ordinary fluid.
pub const LIQUID_METAL_PRANDTL_THRESHOLD: f64 = 0.1;

/// RELAP5 hydraulic heat-transfer geometry. An enum, not a trait object:
/// the set is closed (only these two are in RELAP-7).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Relap5HeatTransferGeometry {
    /// Geometry **101**, the default: internal pipe flow.
    InternalPipe,
    /// Geometry **110**: vertical bundle of in-line rods (square pitch, or
    /// staggered on an equilateral triangle pitch), parallel flow only.
    VerticalRodBundleParallelFlow {
        /// Rod pitch over rod diameter `P/D`; `None` means "not provided"
        /// and selects the default 1.1. Clipped per correlation (see
        /// [`inayatov_pitch_to_diameter`] and
        /// [`kazimi_carelli_pitch_to_diameter`]).
        pitch_to_diameter: Option<Ratio>,
    },
}

impl Relap5HeatTransferGeometry {
    /// The RELAP5 geometry number: 101 or 110.
    pub fn relap5_geometry_number(&self) -> u16 {
        match self {
            Self::InternalPipe => 101,
            Self::VerticalRodBundleParallelFlow { .. } => 110,
        }
    }
}

/// Which correlation set the returned Nusselt number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relap5HeatTransferRegime {
    /// Forced laminar, `Nu = 4.36`, was the maximum.
    ForcedLaminar,
    /// Forced turbulent (Dittus–Boelter, with the Inayatov multiplier in
    /// geometry 110) was the maximum.
    ForcedTurbulent,
    /// Free convection (Churchill–Chu) was the maximum.
    FreeConvection,
    /// `Pr < 0.1`: the liquid-metal correlation (no max-selection).
    LiquidMetal,
}

/// Dimensionless inputs to [`relap5_nusselt`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Relap5DimensionlessGroups {
    /// Reynolds number on the hydraulic diameter, `≥ 0`.
    pub reynolds: Ratio,
    /// Bulk Prandtl number, `> 0`.
    pub prandtl: Ratio,
    /// Grashof number on the free-convection length `L`, `≥ 0`.
    pub grashof: Ratio,
    /// `L / D_h`, the free-convection length over the hydraulic diameter
    /// (1 for RELAP5's default `L = D_h`), `> 0`.
    pub free_convection_length_over_hydraulic_diameter: Ratio,
    /// Whether the wall heats or cools the fluid (Dittus–Boelter exponent).
    pub heating_direction: FluidHeatingDirection,
}

/// The selected Nusselt number and the regime that produced it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Relap5NusseltSelection {
    /// Nusselt number on the hydraulic diameter, `h D_h / k`.
    pub nusselt: Ratio,
    /// Which correlation won.
    pub regime: Relap5HeatTransferRegime,
}

/// Seban–Shimazaki liquid-metal correlation for turbulent flow in a pipe
/// with uniform wall temperature, `Nu = 5.0 + 0.025 Pe^0.8`.
///
/// Source: R. A. Seban and T. T. Shimazaki (1951), "Heat transfer to a
/// fluid flowing turbulently in a smooth pipe with walls at constant
/// temperature", *Transactions of the ASME* **73**, 803–809. (Lyon's 1951
/// uniform-heat-flux form has 7.0 in place of 5.0; the manual's Eq. 49 has
/// 5.0.) Used for all liquid-metal regimes in geometry 101 (RELAP-7 Theory
/// Manual §2.2.2.1, Eq. 49).
///
/// Hand check: `Pe = 100` gives `100^0.8 = 39.8107`, `Nu = 5.99527`.
pub fn seban_shimazaki_liquid_metal_nusselt(peclet_number: f64) -> f64 {
    5.0 + 0.025 * peclet_number.r_powf(0.8)
}

/// `P/D` used by the Inayatov multiplier: an absent value or one below
/// 1.1 becomes 1.1; above 1.6 becomes 1.6 (RELAP-7 Theory Manual §2.2.2.2,
/// following RELAP5).
pub fn inayatov_pitch_to_diameter(pitch_to_diameter: Option<f64>) -> f64 {
    match pitch_to_diameter {
        None => 1.1,
        Some(x) => x.clamp(1.1, 1.6),
    }
}

/// Inayatov's bundle coefficient replacing Dittus–Boelter's 0.023 for
/// parallel flow along in-line (square) or staggered (equilateral
/// triangle) rod bundles: `C = 0.023 P/D`, `P/D` clipped by
/// [`inayatov_pitch_to_diameter`].
///
/// Source: A. Ya. Inayatov (1975), "Correlation of data on heat transfer
/// flow parallel to tube bundles at relative tube pitches of
/// 1.1 < s/d < 1.6", *Heat Transfer — Soviet Research* **7**(3), 84–88.
/// As used in RELAP5 / RELAP-7 (Theory Manual §2.2.2.2, Eq. 50).
pub fn inayatov_bundle_coefficient(pitch_to_diameter: Option<f64>) -> f64 {
    0.023 * inayatov_pitch_to_diameter(pitch_to_diameter)
}

/// `P/D` used by the liquid-metal bundle correlation: an absent value
/// defaults to 1.1 (assumption, see the module docs), then clipped to
/// `[1.1, 1.4]` (RELAP-7 Theory Manual §2.2.2.2, below Eq. 51).
pub fn kazimi_carelli_pitch_to_diameter(pitch_to_diameter: Option<f64>) -> f64 {
    match pitch_to_diameter {
        None => 1.1,
        Some(x) => x.clamp(1.1, 1.4),
    }
}

/// Liquid-metal Nusselt number for parallel flow in rod bundles,
/// `Nu = 4.0 + 0.33 (P/D)^3.8 (Pe/100)^0.86 + 0.16 (P/D)^5.0`. The
/// argument is used as given; clip it first with
/// [`kazimi_carelli_pitch_to_diameter`].
///
/// Source: M. S. Kazimi and M. D. Carelli (1976), "Heat transfer
/// correlation for analysis of CRBRP assemblies", Westinghouse report
/// CRBRP-ARD-0034, known as the Westinghouse correlation and reproduced in
/// N. E. Todreas and M. S. Kazimi, *Nuclear Systems I* (§10, liquid-metal
/// heat transfer in rod bundles) with validity `1.1 ≤ P/D ≤ 1.4`,
/// `10 ≤ Pe ≤ 5000`. The form, coefficients and `P/D` range match RELAP-7
/// Theory Manual §2.2.2.2, Eq. 51 (which does not name its source); the
/// identification is from the coefficients and range and should be
/// confirmed against the report by a reviewer.
///
/// Hand check: `P/D = 1.2, Pe = 100`: `1.2^3.8 = 1.99938`,
/// `1.2^5 = 2.48832`, `Nu = 4 + 0.65980 + 0.39813 = 5.05793`.
pub fn kazimi_carelli_rod_bundle_liquid_metal_nusselt(pitch_to_diameter: f64, peclet_number: f64) -> f64 {
    4.0 + 0.33 * pitch_to_diameter.r_powf(3.8) * (peclet_number / 100.0).r_powf(0.86)
        + 0.16 * pitch_to_diameter.r_powf(5.0)
}

fn require_finite(name: &str, value: f64, lower: f64, strict: bool) -> Result<(), TampinesError> {
    let below = if strict { value <= lower } else { value < lower };
    if !value.is_finite() || below {
        let relation = if strict { ">" } else { ">=" };
        return Err(TampinesError::InvalidInput(format!(
            "{name} = {value} must be finite and {relation} {lower}"
        )));
    }
    Ok(())
}

/// RELAP5 single-phase Nusselt number on the hydraulic diameter, selected
/// as described in the module docs (RELAP-7 Theory Manual §2.2.2.1–2.2.2.2).
///
/// Errors on non-finite inputs, `Re < 0`, `Pr ≤ 0`, `Gr < 0`,
/// `L/D_h ≤ 0`, or a provided `P/D` that is not finite.
pub fn relap5_nusselt(
    geometry: Relap5HeatTransferGeometry,
    groups: Relap5DimensionlessGroups,
) -> Result<Relap5NusseltSelection, TampinesError> {
    let re = groups.reynolds.get::<ratio>();
    let pr = groups.prandtl.get::<ratio>();
    let gr = groups.grashof.get::<ratio>();
    let l_over_d = groups
        .free_convection_length_over_hydraulic_diameter
        .get::<ratio>();
    require_finite("Reynolds number", re, 0.0, false)?;
    require_finite("Prandtl number", pr, 0.0, true)?;
    require_finite("Grashof number", gr, 0.0, false)?;
    require_finite("free-convection length / hydraulic diameter", l_over_d, 0.0, true)?;
    let pitch_to_diameter = match geometry {
        Relap5HeatTransferGeometry::InternalPipe => None,
        Relap5HeatTransferGeometry::VerticalRodBundleParallelFlow { pitch_to_diameter } => {
            match pitch_to_diameter {
                Some(x) => {
                    let x = x.get::<ratio>();
                    if !x.is_finite() {
                        return Err(TampinesError::InvalidInput(format!(
                            "pitch-to-diameter ratio = {x} must be finite"
                        )));
                    }
                    Some(x)
                }
                None => None,
            }
        }
    };

    if pr < LIQUID_METAL_PRANDTL_THRESHOLD {
        let pe = re * pr;
        let nu = match geometry {
            Relap5HeatTransferGeometry::InternalPipe => seban_shimazaki_liquid_metal_nusselt(pe),
            Relap5HeatTransferGeometry::VerticalRodBundleParallelFlow { .. } => {
                kazimi_carelli_rod_bundle_liquid_metal_nusselt(
                    kazimi_carelli_pitch_to_diameter(pitch_to_diameter),
                    pe,
                )
            }
        };
        return Ok(Relap5NusseltSelection {
            nusselt: Ratio::new::<ratio>(nu),
            regime: Relap5HeatTransferRegime::LiquidMetal,
        });
    }

    let to_tampines = |e: tuas_boussinesq_solver::tuas_lib_error::TuasLibError| {
        TampinesError::InvalidInput(e.to_string())
    };
    let dittus_boelter = NusseltCorrelation::DittusBoelter(DittusBoelterData {
        reynolds: groups.reynolds,
        prandtl_bulk: groups.prandtl,
        heating_direction: groups.heating_direction,
    })
    .try_get_nusselt()
    .map_err(to_tampines)?
    .get::<ratio>();
    // Inayatov: C = 0.023 P/D, i.e. the Dittus-Boelter value times P/D.
    let turbulent = match geometry {
        Relap5HeatTransferGeometry::InternalPipe => dittus_boelter,
        Relap5HeatTransferGeometry::VerticalRodBundleParallelFlow { .. } => {
            dittus_boelter * inayatov_pitch_to_diameter(pitch_to_diameter)
        }
    };
    let natural_on_length = NusseltCorrelation::ChurchillChuVerticalPlate(ChurchillChuData {
        grashof: groups.grashof,
        prandtl: groups.prandtl,
    })
    .try_get_nusselt()
    .map_err(to_tampines)?
    .get::<ratio>();
    // Nu_L k / L = Nu_Dh k / D_h  =>  Nu_Dh = Nu_L D_h / L.
    let natural = natural_on_length / l_over_d;

    let mut nu = RELAP5_LAMINAR_NUSSELT;
    let mut regime = Relap5HeatTransferRegime::ForcedLaminar;
    if turbulent > nu {
        nu = turbulent;
        regime = Relap5HeatTransferRegime::ForcedTurbulent;
    }
    if natural > nu {
        nu = natural;
        regime = Relap5HeatTransferRegime::FreeConvection;
    }
    Ok(Relap5NusseltSelection {
        nusselt: Ratio::new::<ratio>(nu),
        regime,
    })
}

/// Physical (`uom`) inputs for [`relap5_wall_heat_transfer`]. Properties
/// are bulk-fluid values (the manual evaluates `k` at the bulk
/// temperature).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Relap5WallConvectionInputs {
    /// Fluid density `ρ`.
    pub density: MassDensity,
    /// Dynamic viscosity `μ`.
    pub dynamic_viscosity: DynamicViscosity,
    /// Thermal conductivity `k`.
    pub thermal_conductivity: ThermalConductivity,
    /// Isobaric specific heat capacity `c_p` (for `Pr = μ c_p / k`).
    pub specific_heat_capacity: SpecificHeatCapacity,
    /// Volumetric thermal expansion coefficient `β`.
    pub thermal_expansion_coefficient: TemperatureCoefficient,
    /// Area-averaged bulk fluid temperature `T`.
    pub bulk_temperature: ThermodynamicTemperature,
    /// Wall temperature `T_w`.
    pub wall_temperature: ThermodynamicTemperature,
    /// Mean flow speed `|u|` (its sign does not matter).
    pub flow_speed: Velocity,
    /// Heat-transfer hydraulic diameter `D_h`.
    pub hydraulic_diameter: Length,
    /// Free-convection length scale `L`; `None` uses `D_h` (RELAP5's
    /// default).
    pub free_convection_length: Option<Length>,
    /// Gravitational acceleration `g`.
    pub gravity: Acceleration,
}

/// Result of [`relap5_wall_heat_transfer`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Relap5WallHeatTransfer {
    /// The dimensionless groups formed from the inputs.
    pub groups: Relap5DimensionlessGroups,
    /// The selected Nusselt number and regime.
    pub selection: Relap5NusseltSelection,
    /// Wall heat-transfer coefficient `h = Nu k / D_h`.
    pub heat_transfer_coefficient: HeatTransfer,
}

impl Relap5WallConvectionInputs {
    /// Forms `Re = ρ |u| D_h / μ`, `Pr = μ c_p / k`,
    /// `Gr = ρ² g β |T_w − T| L³ / μ²` and the heating direction
    /// (`T_w ≥ T` is "fluid heated").
    pub fn dimensionless_groups(&self) -> Result<Relap5DimensionlessGroups, TampinesError> {
        let d_h = self.hydraulic_diameter.get::<meter>();
        require_finite("hydraulic diameter [m]", d_h, 0.0, true)?;
        let length = self.free_convection_length.unwrap_or(self.hydraulic_diameter);
        require_finite("free-convection length [m]", length.get::<meter>(), 0.0, true)?;
        require_finite(
            "gravity [m/s^2]",
            self.gravity.get::<meter_per_second_squared>(),
            0.0,
            false,
        )?;
        let reynolds: Ratio = self.density * self.flow_speed.abs() * self.hydraulic_diameter
            / self.dynamic_viscosity;
        let prandtl: Ratio =
            self.dynamic_viscosity * self.specific_heat_capacity / self.thermal_conductivity;
        let delta_t =
            self.wall_temperature.get::<kelvin>() - self.bulk_temperature.get::<kelvin>();
        let grashof = grashof_number(
            self.density,
            self.gravity,
            self.thermal_expansion_coefficient,
            TemperatureInterval::new::<kelvin_interval>(delta_t),
            length,
            self.dynamic_viscosity,
        );
        let heating_direction = if delta_t >= 0.0 {
            FluidHeatingDirection::FluidHeated
        } else {
            FluidHeatingDirection::FluidCooled
        };
        Ok(Relap5DimensionlessGroups {
            reynolds,
            prandtl,
            grashof,
            free_convection_length_over_hydraulic_diameter: length / self.hydraulic_diameter,
            heating_direction,
        })
    }
}

/// RELAP5 single-phase wall heat-transfer coefficient for geometry 101 or
/// 110: forms the dimensionless groups, selects the Nusselt number with
/// [`relap5_nusselt`] and returns `h = Nu k / D_h`.
pub fn relap5_wall_heat_transfer(
    geometry: Relap5HeatTransferGeometry,
    inputs: &Relap5WallConvectionInputs,
) -> Result<Relap5WallHeatTransfer, TampinesError> {
    let groups = inputs.dimensionless_groups()?;
    let selection = relap5_nusselt(geometry, groups)?;
    let heat_transfer_coefficient: HeatTransfer =
        selection.nusselt * inputs.thermal_conductivity / inputs.hydraulic_diameter;
    Ok(Relap5WallHeatTransfer {
        groups,
        selection,
        heat_transfer_coefficient,
    })
}

#[cfg(test)]
mod tests;
