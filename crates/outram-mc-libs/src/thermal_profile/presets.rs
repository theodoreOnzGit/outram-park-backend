// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! Ballpark heat-removal presets (maintainer decision on gh:#945, 2026-10-17).
//!
//! **Ballpark by design.** A beginner is not expected to know
//! thermal-hydraulics, so heat removal is a named preset. Each preset's `h`
//! and coolant temperature is a named constant below, with its source or an
//! explicit `UNCITED` marker. **The literature corpus holds no textbook table
//! of typical heat-transfer coefficients** (searched 2026-10-17: no Lienhard,
//! Incropera or Todreas & Kazimi), so no constant here is "picked from a
//! textbook range"; the cited ones are single values from one source each.

use uom::si::f64::{HeatTransfer, ThermodynamicTemperature};
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

/// Air-cooled film coefficient, W/(m^2 K).
///
/// **Source:** IAEA-TECDOC-1694 (2013), *Evaluation of HTGR Performance:
/// Benchmark Analysis Related to the PBMR-400, PBMM, GT-MHR, HTR-10 and
/// ASTRA*, Table 5.33 (printed p. 430), "Convective heat transfer coefficient
/// h [W/m2K] ... Outside (o) 10": the PBMM pressure vessel's insulated
/// outer surface to ambient air, radiation excluded by that table's text.
/// One model input for one facility, not a range.
pub const AIR_COOLED_HTC_W_PER_M2_K: f64 = 10.0;

/// Ambient air temperature for [`HeatRemoval::AirCooled`], K.
///
/// **UNCITED — maintainer to supply source.** Room temperature, chosen as a
/// round value; no corpus source states an ambient air temperature.
pub const AIR_AMBIENT_TEMPERATURE_K: f64 = 300.0;

/// Water-cooled film coefficient, W/(m^2 K).
///
/// **UNCITED — maintainer to supply source.** A round value for forced
/// convection of liquid water, not taken from the corpus (no source in it
/// gives a water heat-transfer coefficient or range).
pub const WATER_COOLED_HTC_W_PER_M2_K: f64 = 5000.0;

/// Coolant temperature for [`HeatRemoval::WaterCooled`], K (304.5 degC).
///
/// **Source:** Marguet, *The History of the Pressurized Water Reactor*
/// (Springer, doi:10.1007/978-3-030-86638-9), section 1.12 on Tihange: "The
/// average temperature in the core is 304.5 degC". So "water-cooled" here
/// means pressurised-water-reactor coolant, not a low-temperature pool.
pub const WATER_COOLANT_TEMPERATURE_K: f64 = 304.5 + 273.15;

/// How the heat leaves the body: a ballpark preset (see the module docs).
///
/// At steady state everything generated leaves, so the input is *how* it
/// leaves, never how much.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HeatRemoval {
    /// Convection to still-ish air: [`AIR_COOLED_HTC_W_PER_M2_K`] to
    /// [`AIR_AMBIENT_TEMPERATURE_K`]. Radiation is not modelled.
    AirCooled,
    /// Convection to pressurised water: [`WATER_COOLED_HTC_W_PER_M2_K`] to
    /// [`WATER_COOLANT_TEMPERATURE_K`].
    WaterCooled,
    /// The whole outer surface is held at this temperature.
    FixedSurfaceTemperature(ThermodynamicTemperature),
}

impl HeatRemoval {
    /// The film coefficient the preset uses (`None` for a fixed surface).
    #[must_use]
    pub fn heat_transfer_coefficient(self) -> Option<HeatTransfer> {
        let h = match self {
            Self::AirCooled => AIR_COOLED_HTC_W_PER_M2_K,
            Self::WaterCooled => WATER_COOLED_HTC_W_PER_M2_K,
            Self::FixedSurfaceTemperature(_) => return None,
        };
        Some(HeatTransfer::new::<watt_per_square_meter_kelvin>(h))
    }

    /// The coolant (ambient) temperature, or the fixed surface temperature.
    #[must_use]
    pub fn reference_temperature(self) -> ThermodynamicTemperature {
        match self {
            Self::AirCooled => ThermodynamicTemperature::new::<kelvin>(AIR_AMBIENT_TEMPERATURE_K),
            Self::WaterCooled => {
                ThermodynamicTemperature::new::<kelvin>(WATER_COOLANT_TEMPERATURE_K)
            }
            Self::FixedSurfaceTemperature(t) => t,
        }
    }

    /// Whether the preset's values carry a literature citation for both `h`
    /// and the temperature (`false` when either is marked UNCITED).
    #[must_use]
    pub fn fully_cited(self) -> bool {
        match self {
            // h cited (TECDOC-1694), ambient uncited.
            Self::AirCooled => false,
            // temperature cited (Marguet), h uncited.
            Self::WaterCooled => false,
            // The user states the temperature.
            Self::FixedSurfaceTemperature(_) => true,
        }
    }
}
