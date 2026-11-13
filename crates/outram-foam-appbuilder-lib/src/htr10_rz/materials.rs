// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! Homogenised region properties of the HTR-10 R-Z model, built from the
//! gh:#1148 closures ([`crate::htgr_closures`]) and tabulated for the solid
//! conduction field.
//!
//! ## Homogenisation (TECDOC-1694 Sec 2.2.1.2, Tab 2.4)
//!
//! Tab 2.4 gives each region's **solid mass fraction** `f` and says heat
//! capacity is weighted by void fraction. This module weights **both**
//! properties the same way, as the brief for gh:#1149 asks:
//!
//! ```text
//!   lambda = f lambda_solid(T) + (1 - f) lambda_He(T)
//!   rho c  = f (rho c)_solid(T) + (1 - f) (rho c)_He(T)
//! ```
//!
//! For conductivity this is the parallel-path (arithmetic) mean, exact for
//! conduction along straight channels and an **upper bound** across them; a
//! Maxwell-type mean would give `lambda_s (1 - eps)/(1 + eps)` across, 17 %
//! lower in the control-rod ring (`eps = 0.207`). No radiation inside the
//! channels is added.
//!
//! ## Gas cavities (THERMIX-style homogenised media)
//!
//! A helium cavity of width `delta` between grey walls of emissivity `eps`
//! (TECDOC-1694 Tab 2.13) is a medium of conductivity
//!
//! ```text
//!   lambda = lambda_He(T) + 4 sigma T^3 delta / (2/eps - 1)
//! ```
//!
//! The radiative term is the parallel-plate exchange written as a
//! conductivity: integrated across the cavity, `int lambda_rad dT =
//! sigma (T1^4 - T2^4) delta / (2/eps - 1)`, so a resolved cavity carries
//! exactly the grey two-plate exchange `sigma (T1^4 - T2^4)/(2/eps - 1)` as
//! its cells refine. `delta` is the width across which the cavity exchanges
//! (radial for the annular gaps, axial for the plena); the medium is
//! isotropic, so the other direction gets the same conductivity, an
//! approximation for the two large cavities (18, 21). Natural convection in
//! the cavities is **not** modelled (THERMIX includes it); that omission
//! under-states heat transfer.
//!
//! ## Properties outside a closure's range
//!
//! The solver needs a value at any temperature, and the closures refuse
//! temperatures outside their stated range. Each property is therefore
//! **tabulated** every 5 K from 273.15 to 2273.15 K, and outside a closure's
//! range the value at the nearest end of the range is **held**. Every hold is
//! listed in [`RegionMaterial::valid_range_kelvin`], and the model reports
//! after each solve which regions reached a held temperature
//! (`Htr10RzResult::range_flags`). The holds:
//!
//! - **Side reflector below 300 deg C** (the gh:#1148 flag): the Tab 2.11 fit
//!   is refused there. TECDOC-1694 gives no other guidance (its stated range
//!   is 0-2000 deg C), so the default **holds the value at 300 deg C**
//!   ([`SideReflectorBelow300C::HoldAt300C`]), a named assumption; the
//!   sensitivity case uses the unirradiated Tab 2.11 top/bottom-reflector fit
//!   instead ([`SideReflectorBelow300C::TopBottomFit`]). At zero dose the two
//!   fits agree at 300 deg C to 0.5 % (84.2 vs 84.6 W/(m K)).
//! - Helium (Tab 2.8): 293-1773 K. Carbon brick (Tab 2.12): up to 1000 deg C.
//!   Top/bottom graphite (Tab 2.11): up to 1700 deg C. RPV steel (Fig 2.8,
//!   digitised): 290-700 K. Barrel steel: 293-1070 K. Fuel element
//!   (Tab 2.9, used by ZBS only): up to 1300 deg C.
//!
//! **Fluence.** Zero for every graphite (the initial core; the DOSIS unit
//! is Unknown in the input sheet, so a non-zero value could not be cited).
//! **Steel for regions 10, 11, 13** (bottom supports): no material is given;
//! the Fig 2.8 barrel curve is used (Assumed).

use uom::si::areal_number_density::per_square_centimeter;
use uom::si::f64::{ArealNumberDensity, Pressure, ThermodynamicTemperature};
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::bar;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

use super::Htr10RzError;
use crate::genfoam::thermal_hydraulics::solid_conduction::{
    ConductivityModel, HeatCapacityModel, PropertyModel, PropertyTable,
};
use crate::htgr_closures::{
    BedConductivityModel, BedState, KtaHelium, SteelComponent, StructuralMaterial,
};

/// Stefan-Boltzmann constant, W m^-2 K^-4 (CODATA 2018 exact).
pub const SIGMA: f64 = 5.670_374_419e-8;

/// Lowest and highest tabulated temperature, K, and the step.
const T_LO: f64 = 273.15;
const T_HI: f64 = 2273.15;
const T_STEP: f64 = 5.0;

/// What a region is made of and how it is homogenised.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RegionMaterial {
    /// The pebble bed: the bed effective conductivity of the chosen
    /// [`BedConductivityModel`] (already an effective, void-inclusive
    /// value), heat capacity `0.61 x` the Tab 2.9 graphite value.
    PebbleBed,
    /// Side-reflector graphite, Tab 2.11 (irradiation-dependent fit, zero
    /// dose), with helium in the rest.
    SideReflector {
        /// Tab 2.4 solid mass fraction.
        solid_fraction: f64,
    },
    /// Top/bottom-reflector graphite, Tab 2.11 (unirradiated fit).
    TopBottomReflector {
        /// Tab 2.4 solid mass fraction.
        solid_fraction: f64,
    },
    /// Boronated carbon brick, Tab 2.12.
    CarbonBrick,
    /// RPV steel, the Fig 2.8 "SA508, SA516-70, RPV" curve (digitised).
    RpvSteel {
        /// Tab 2.4 solid mass fraction.
        solid_fraction: f64,
    },
    /// Core-vessel (barrel) steel, the Fig 2.8 "SA240, SS316, core barrel"
    /// curve (Tab 2.3 says SA387-11, which Fig 2.8 does not show: input
    /// sheet N16).
    BarrelSteel {
        /// Tab 2.4 solid mass fraction.
        solid_fraction: f64,
    },
    /// A helium cavity: gas conduction plus grey two-plate radiation across
    /// `width_m`.
    HeliumCavity {
        /// Wall emissivity, Tab 2.13 (or assumed, see `geometry.rs`).
        emissivity: f64,
        /// Width across which the cavity exchanges, m.
        width_m: f64,
    },
}

/// What the side-reflector conductivity is below 300 deg C, where the
/// Tab 2.11 fit is refused (gh:#1148 flag).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SideReflectorBelow300C {
    /// Hold the 300 deg C value (named assumption `SR-HOLD300`). Default.
    #[default]
    HoldAt300C,
    /// Use the unirradiated Tab 2.11 top/bottom-reflector fit (sensitivity
    /// case `SR-TOPBOTTOM`).
    TopBottomFit,
}

/// The carbon-brick conductivity (gh:#1150 ablation).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum CarbonBrickConductivity {
    /// TECDOC-1694 Tab 2.12, `0.05 + 0.03e-3 T` W/(cm K) (5-8 W/(m K)).
    /// **The default**: the only cited value.
    #[default]
    Tab212,
    /// A constant value. **Ablation only**: the 30 W/(m K) that
    /// `htgr_sim_v1`'s passive decay-heat chain assumes has no source; this
    /// variant exists to measure, inside this model, how much of the
    /// gh:#1149 gap (102.5 vs 254.0 kW) that one value carries.
    Constant(uom::si::f64::ThermalConductivity),
}

/// Options that change material properties.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialOptions {
    /// Bed effective conductivity. Default Breitbach (D3).
    pub bed: BedConductivityModel,
    /// Side reflector below 300 deg C.
    pub side_reflector_below_300c: SideReflectorBelow300C,
    /// Helium pressure for the Tab 2.8 fits, bar. Default 30 bar (the
    /// FPIC specification, Tab 2.1).
    pub helium_pressure_bar: f64,
    /// Carbon brick conductivity. Default Tab 2.12.
    pub carbon_brick: CarbonBrickConductivity,
}

impl Default for MaterialOptions {
    fn default() -> Self {
        Self {
            bed: BedConductivityModel::default(),
            side_reflector_below_300c: SideReflectorBelow300C::default(),
            helium_pressure_bar: 30.0,
            carbon_brick: CarbonBrickConductivity::default(),
        }
    }
}

fn kel(t: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(t)
}

fn zero_dose() -> ArealNumberDensity {
    ArealNumberDensity::new::<per_square_centimeter>(0.0)
}

fn closure_err(what: &str, e: impl std::fmt::Display) -> Htr10RzError {
    Htr10RzError::Property(format!("{what}: {e}"))
}

/// Helium conductivity, W/(m K), Tab 2.8 (bar-fixed), temperature held to
/// 293-1773 K.
fn helium_k(t: f64, opts: &MaterialOptions) -> Result<f64, Htr10RzError> {
    let he = KtaHelium::new(
        Pressure::new::<bar>(opts.helium_pressure_bar),
        kel(t.clamp(KtaHelium::MIN_TEMPERATURE_K, KtaHelium::MAX_TEMPERATURE_K)),
    )
    .map_err(|e| closure_err("helium", e))?;
    Ok(he.thermal_conductivity().get::<watt_per_meter_kelvin>())
}

/// Helium `rho c_p`, J/(m^3 K), Tab 2.8.
fn helium_c(t: f64, opts: &MaterialOptions) -> Result<f64, Htr10RzError> {
    let he = KtaHelium::new(
        Pressure::new::<bar>(opts.helium_pressure_bar),
        kel(t.clamp(KtaHelium::MIN_TEMPERATURE_K, KtaHelium::MAX_TEMPERATURE_K)),
    )
    .map_err(|e| closure_err("helium", e))?;
    Ok(he.density().get::<kilogram_per_cubic_meter>() * 5195.0)
}

fn structural_k(m: StructuralMaterial, t: f64) -> Result<f64, Htr10RzError> {
    m.thermal_conductivity(kel(t))
        .map(|k| k.get::<watt_per_meter_kelvin>())
        .map_err(|e| closure_err("structural material", e))
}

/// TECDOC-1694 Tab 2.9 graphite volumetric heat capacity polynomial with the
/// given prefactor (1.75 graphite, 1.55 carbon brick, Tab 2.12), J/(m^3 K),
/// `T` held at 1200 deg C above.
fn graphite_c(prefactor: f64, t_k: f64) -> f64 {
    let t = (t_k - 273.15).clamp(0.0, 1200.0);
    prefactor * (0.645 + 3.14e-3 * t - 2.809e-6 * t * t + 0.959e-9 * t * t * t) * 1.0e6
}

/// Fig 2.9 steel `rho c_p`, read by eye in the input sheet Sec 4.6
/// (Uncertain), linear between the read points and held outside.
fn steel_c(rpv: bool, t: f64) -> f64 {
    let ((t0, c0), (t1, c1)) = if rpv {
        ((300.0, 3.5e6), (700.0, 4.8e6))
    } else {
        ((300.0, 3.9e6), (1070.0, 5.05e6))
    };
    let t = t.clamp(t0, t1);
    c0 + (c1 - c0) * (t - t0) / (t1 - t0)
}

impl RegionMaterial {
    /// Conductivity at `t` K, W/(m K), with the holds of the module docs.
    ///
    /// # Errors
    /// [`Htr10RzError::Property`] if a closure fails inside its range.
    pub fn conductivity_si(&self, t: f64, opts: &MaterialOptions) -> Result<f64, Htr10RzError> {
        let mix = |f: f64, ks: f64| -> Result<f64, Htr10RzError> {
            Ok(f * ks + (1.0 - f) * helium_k(t, opts)?)
        };
        match *self {
            Self::PebbleBed => {
                let ks = structural_k(
                    StructuralMaterial::FuelElement {
                        fast_fluence: zero_dose(),
                    },
                    t.min(1573.15),
                )?;
                let kf = helium_k(t, opts)?;
                let state = BedState::new(
                    kel(t),
                    uom::si::f64::ThermalConductivity::new::<watt_per_meter_kelvin>(ks),
                    uom::si::f64::ThermalConductivity::new::<watt_per_meter_kelvin>(kf),
                );
                opts.bed
                    .effective_conductivity(&state)
                    .map(|k| k.get::<watt_per_meter_kelvin>())
                    .map_err(|e| closure_err("bed", e))
            }
            Self::SideReflector { solid_fraction } => {
                let side = StructuralMaterial::SideReflector {
                    fast_fluence: zero_dose(),
                };
                let ks = if t >= 573.15 {
                    structural_k(side, t.min(2273.15))?
                } else {
                    match opts.side_reflector_below_300c {
                        SideReflectorBelow300C::HoldAt300C => structural_k(side, 573.15)?,
                        SideReflectorBelow300C::TopBottomFit => {
                            structural_k(StructuralMaterial::TopBottomReflector, t.max(273.15))?
                        }
                    }
                };
                mix(solid_fraction, ks)
            }
            Self::TopBottomReflector { solid_fraction } => mix(
                solid_fraction,
                structural_k(StructuralMaterial::TopBottomReflector, t.clamp(273.15, 1973.15))?,
            ),
            Self::CarbonBrick => match opts.carbon_brick {
                CarbonBrickConductivity::Tab212 => {
                    structural_k(StructuralMaterial::CarbonBrick, t.clamp(273.15, 1273.15))
                }
                CarbonBrickConductivity::Constant(k) => Ok(k.get::<watt_per_meter_kelvin>()),
            },
            Self::RpvSteel { solid_fraction } => mix(
                solid_fraction,
                structural_k(
                    StructuralMaterial::Steel(SteelComponent::RpvSa508Sa516_70),
                    t.clamp(290.0, 700.0),
                )?,
            ),
            Self::BarrelSteel { solid_fraction } => mix(
                solid_fraction,
                structural_k(
                    StructuralMaterial::Steel(SteelComponent::CoreBarrelSa240Ss316),
                    t.clamp(293.0, 1070.0),
                )?,
            ),
            Self::HeliumCavity {
                emissivity,
                width_m,
            } => Ok(helium_k(t, opts)?
                + 4.0 * SIGMA * t * t * t * width_m / (2.0 / emissivity - 1.0)),
        }
    }

    /// Volumetric heat capacity at `t` K, J/(m^3 K) (only a transient solve
    /// reads it).
    ///
    /// # Errors
    /// As [`Self::conductivity_si`].
    pub fn heat_capacity_si(&self, t: f64, opts: &MaterialOptions) -> Result<f64, Htr10RzError> {
        let he = helium_c(t, opts)?;
        Ok(match *self {
            Self::PebbleBed => 0.61 * graphite_c(1.75, t) + 0.39 * he,
            Self::SideReflector { solid_fraction } | Self::TopBottomReflector { solid_fraction } => {
                solid_fraction * graphite_c(1.75, t) + (1.0 - solid_fraction) * he
            }
            Self::CarbonBrick => graphite_c(1.55, t),
            Self::RpvSteel { solid_fraction } => solid_fraction * steel_c(true, t) + (1.0 - solid_fraction) * he,
            Self::BarrelSteel { solid_fraction } => {
                solid_fraction * steel_c(false, t) + (1.0 - solid_fraction) * he
            }
            Self::HeliumCavity { .. } => he,
        })
    }

    /// The temperature range, K, inside which every closure this region uses
    /// is evaluated without a hold. Outside it a held value is used.
    #[must_use]
    pub fn valid_range_kelvin(&self, opts: &MaterialOptions) -> (f64, f64) {
        let he = (KtaHelium::MIN_TEMPERATURE_K, KtaHelium::MAX_TEMPERATURE_K);
        match *self {
            Self::PebbleBed => match opts.bed {
                // Tab 2.9 prints no range for the bed fit, only its own hold
                // below 250 deg C, which is part of the fit as printed.
                BedConductivityModel::Breitbach => (T_LO, f64::INFINITY),
                BedConductivityModel::Kugeler2017 => (T_LO, 2773.15),
                // ZBS reads the fuel-element and helium fits.
                BedConductivityModel::Zbs(_) => (he.0, 1573.15),
            },
            Self::SideReflector { solid_fraction } => {
                (573.15, if solid_fraction < 1.0 { he.1 } else { 2273.15 })
            }
            Self::TopBottomReflector { solid_fraction } => {
                if solid_fraction < 1.0 {
                    (he.0, he.1)
                } else {
                    (T_LO, 1973.15)
                }
            }
            Self::CarbonBrick => (T_LO, 1273.15),
            Self::RpvSteel { solid_fraction } => {
                if solid_fraction < 1.0 {
                    (he.0, 700.0)
                } else {
                    (290.0, 700.0)
                }
            }
            Self::BarrelSteel { .. } => (he.0, 1070.0),
            Self::HeliumCavity { .. } => he,
        }
    }

    /// The tabulated conductivity model the solver uses (every 5 K,
    /// 273.15-2273.15 K, clamped beyond).
    ///
    /// # Errors
    /// As [`Self::conductivity_si`].
    pub fn conductivity_model(&self, opts: &MaterialOptions) -> Result<ConductivityModel, Htr10RzError> {
        Ok(ConductivityModel(PropertyModel::Tabulated(self.table(|t| {
            self.conductivity_si(t, opts)
        })?)))
    }

    /// The tabulated heat-capacity model.
    ///
    /// # Errors
    /// As [`Self::conductivity_si`].
    pub fn heat_capacity_model(&self, opts: &MaterialOptions) -> Result<HeatCapacityModel, Htr10RzError> {
        Ok(HeatCapacityModel(PropertyModel::Tabulated(self.table(|t| {
            self.heat_capacity_si(t, opts)
        })?)))
    }

    fn table(
        &self,
        f: impl Fn(f64) -> Result<f64, Htr10RzError>,
    ) -> Result<PropertyTable, Htr10RzError> {
        let n = ((T_HI - T_LO) / T_STEP).round() as usize;
        let pts = (0..=n)
            .map(|i| {
                let t = T_LO + T_STEP * i as f64;
                f(t).map(|v| (t, v))
            })
            .collect::<Result<Vec<_>, _>>()?;
        PropertyTable::new(&pts).map_err(|e| Htr10RzError::Property(e.to_string()))
    }
}
