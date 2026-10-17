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

//! Thermal conductivity of the solids a beginner model is made of (gh:#949).
//!
//! Each variant names its correlation and its source. A correlation is
//! **evaluated as written outside its stated range** (extrapolated), and the
//! profile result records every material that was evaluated outside its range
//! ([`super::ThermalProfile::conductivity_outside_range`]); nothing is clamped
//! silently. **Copies elsewhere in the workspace** are named so the two cannot
//! drift unnoticed; this crate cannot depend on those crates.

use uom::si::f64::{ThermalConductivity, ThermodynamicTemperature};
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

/// A solid's thermal conductivity model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThermalMaterial {
    /// Fresh UO2 at 95 % of theoretical density.
    ///
    /// **Source:** Geelhood et al. (2020) model as given in PNNL-31427
    /// (Wells, Phillips & Geelhood, *TRISO Fuel: Properties and Failure
    /// Modes*, 2021), Eq. (3.2)-(3.3), with burnup `Bu = 0` and no gadolinia,
    /// which removes `f(Bu)` and `g(Bu) h(T)`:
    /// `k95 = 1/(A + B T) + (C/T^2) exp(-D/T)`, `A = 0.0452 m K/W`,
    /// `B = 2.46e-4 m/W`, `C = 3.5e9 W K/m`, `D = 16361 K`, T in K. Eq. (3.4)
    /// at `d = 0.95` returns `k95` itself (factor 0.99995). No validity range
    /// is stated in the pages read; none is enforced. Same form as
    /// `outram-park-fork-offbeat`'s `ConductivityModel::MatproUo2`, which uses
    /// `D = 16360 K` where PNNL-31427 prints 16361 K.
    UraniumDioxide,
    /// Unirradiated graphite matrix (A3-27, 1800 degC heat treatment), no
    /// coated particles, at the reference density 1.7 g/cm^3.
    ///
    /// **Source:** PNNL-31427 Eq. (3.43) with Table 3.3 (Hales et al. 2020):
    /// `k = k100 [1 - alpha (T - 100) exp(delta T)]`, T in degC,
    /// `k100 = 47.4 W/(m K)`, `alpha = 9.7556e-4`, `delta = -6.0360e-4`.
    /// The fluence, packing-fraction and density factors (Eq. 3.44-3.47) are 1
    /// at zero fluence, zero packing fraction and reference density. Same row
    /// as `tuas_boussinesq_solver`'s
    /// `nuclear_graphite_matrix_a3_thermal_conductivity_zero_fluence`. A
    /// fuel-element matrix graphite, used here as the beginner's "graphite".
    GraphiteMatrixA3,
    /// AISI 304L stainless steel.
    ///
    /// **Source:** Graves et al. (1991), Int. J. Thermophys. 12:409-415, as
    /// Eq. (2.27) of Ong (2024), PhD dissertation (open corpus):
    /// `k = 7.9318 + 0.023051 T - 6.4166e-6 T^2` W/(m K), T in K; range
    /// 300-700 K per `tuas_boussinesq_solver`'s
    /// `steel_ss_304_l_ornl_thermal_conductivity`, which is the same fit.
    StainlessSteel304L,
    /// Aluminium, constant.
    ///
    /// **Source:** OpenStax *University Physics* Vol. 2 (2016), Table 1.5,
    /// "Aluminum 220", a value near room temperature; no temperature
    /// dependence is modelled.
    Aluminium,
    /// Zirconium alloy (Zircaloy).
    ///
    /// **UNCITED — maintainer to supply source.** The RELAP Zircaloy fit
    /// `k = 7.51 + 2.09e-2 T - 1.45e-5 T^2 + 7.67e-9 T^3` (T in K, 290-1800 K)
    /// as ported in `outram-park-fork-offbeat`'s
    /// `ConductivityModel::RelapZircaloy` from OFFBEAT's `conductivityRelapZy.C`;
    /// no literature source for it is in the corpus.
    Zircaloy,
    /// Uranium metal, constant.
    ///
    /// **UNCITED — maintainer to supply source.** 27.5 W/(m K), a
    /// room-temperature value given from general knowledge, not from the
    /// corpus (no uranium-metal conductivity was found there).
    UraniumMetal,
    /// A conductivity the user states, constant in temperature.
    Constant(ThermalConductivity),
}

impl ThermalMaterial {
    /// `k` at temperature `t`, evaluated as written (see the module docs).
    #[must_use]
    pub fn conductivity(self, t: ThermodynamicTemperature) -> ThermalConductivity {
        let tk = t.get::<kelvin>();
        let k = match self {
            Self::UraniumDioxide => {
                let (a, b, c, d) = (0.0452, 2.46e-4, 3.5e9, 16361.0);
                1.0 / (a + b * tk) + c / (tk * tk) * (-d / tk).exp()
            }
            Self::GraphiteMatrixA3 => {
                let tc = tk - 273.15;
                let (k100, alpha, delta) = (47.4, 9.7556e-4, -6.0360e-4);
                k100 * (1.0 - alpha * (tc - 100.0) * (delta * tc).exp())
            }
            Self::StainlessSteel304L => 7.9318 + 0.023051 * tk - 6.4166e-6 * tk * tk,
            Self::Aluminium => 220.0,
            Self::Zircaloy => 7.51 + 2.09e-2 * tk - 1.45e-5 * tk * tk + 7.67e-9 * tk * tk * tk,
            Self::UraniumMetal => 27.5,
            Self::Constant(k) => return k,
        };
        ThermalConductivity::new::<watt_per_meter_kelvin>(k)
    }

    /// The stated validity range in K, where the source gives one.
    #[must_use]
    pub fn validity_kelvin(self) -> Option<(f64, f64)> {
        match self {
            Self::StainlessSteel304L => Some((300.0, 700.0)),
            Self::Zircaloy => Some((290.0, 1800.0)),
            _ => None,
        }
    }

    /// Whether the conductivity carries a literature citation.
    #[must_use]
    pub fn is_cited(self) -> bool {
        !matches!(self, Self::Zircaloy | Self::UraniumMetal)
    }
}
