// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
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

/// Universal gas constant in J/(mol·K).
/// Using this value with `MolarMass` in kg/mol gives `r = R_UNIVERSAL / W` in J/(kg·K).
pub const R_UNIVERSAL: f64 = 8.314_462_618_153_24;

/// The gas constant OpenFOAM-v2506 (ESI) actually uses, in J/(mol K):
/// `NA * k` from the case-independent `etc/controlDict` values
/// `NA = 6.0221417930e23`, `k = 1.38065e-23` (`DimensionedConstants`). It is
/// **9.0e-7 relative above** [`R_UNIVERSAL`] (CODATA 2019). Upstream then forms
/// `RR = 1e3 * R` in J/(kmol K) and `R_specific = RR / W` with `W` in kg/kmol.
pub const R_UNIVERSAL_ESI_V2506: f64 = 6.022_141_793_0e23 * 1.380_65e-23;

/// Which universal gas constant an equation of state uses.
///
/// `Codata2019` (the default, `R_UNIVERSAL`) is the physically current value.
/// `EsiV2506` reproduces OpenFOAM-v2506 bit for bit, including its
/// `RR/W` kmol arithmetic (gh:#1362 row 4: upstream's constant differs by
/// 9.0e-7 relative, which is the whole of the rho, psi and `CpMCv` mismatch
/// of the default). Select it with the equation of state's `with_gas_constant`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GasConstant {
    /// CODATA 2019, `R_UNIVERSAL / W` with `W` in kg/mol.
    #[default]
    Codata2019,
    /// OpenFOAM-v2506 (ESI): `(1e3 * R_UNIVERSAL_ESI_V2506) / (W * 1e3)`.
    EsiV2506,
}

impl GasConstant {
    /// `RR = 1e3 * R` in J/(kmol K) as OpenFOAM forms it.
    pub fn universal_j_per_kmol_k(self) -> f64 {
        match self {
            GasConstant::Codata2019 => 1.0e3 * R_UNIVERSAL,
            GasConstant::EsiV2506 => 1.0e3 * R_UNIVERSAL_ESI_V2506,
        }
    }

    /// Specific gas constant `R / W` in J/(kg K) for a molar mass in kg/mol.
    pub fn specific(self, w_kg_per_mol: f64) -> f64 {
        match self {
            GasConstant::Codata2019 => R_UNIVERSAL / w_kg_per_mol,
            GasConstant::EsiV2506 => (1.0e3 * R_UNIVERSAL_ESI_V2506) / (w_kg_per_mol * 1.0e3),
        }
    }
}

/// Standard thermodynamic temperature (used as entropy reference in S = Cp·ln(T/Tstd)).
pub const T_STD: f64 = 298.15; // K

/// Minimum temperature floor used in Newton T-iteration to prevent log(0).
pub const T_MIN: f64 = 100.0; // K

/// Upper JANAF coefficient range limit.
pub const T_MAX: f64 = 6000.0; // K

/// Standard-state reference pressure for entropy calculations.
///
/// ~~`101_325.0` Pa~~ **CORRECTED 2026-11-13 (gh:#1362 row 4):** `1e5` Pa. Both
/// OpenFOAM lines define `Pstd = 1e5` Pa (v2506 `etc/controlDict`,
/// `DimensionedConstants/SICoeffs/standard`), and `perfectGas::S` is
/// `-R log(p/Pstd)`. The old value shifted every perfect-gas and
/// Peng-Robinson absolute entropy by `R ln(1.01325)` (3.8 J/(kg K) for air).
pub const P_REF: f64 = 1.0e5; // Pa

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn esi_gas_constant_is_nine_e_minus_7_above_codata() {
        let rel = (R_UNIVERSAL_ESI_V2506 - R_UNIVERSAL) / R_UNIVERSAL;
        assert!(rel > 8.9e-7 && rel < 9.1e-7, "{rel}");
        // RR/W with W in kg/kmol, against the per-mole form
        let w = 0.028_96;
        let a = GasConstant::Codata2019.specific(w);
        let b = GasConstant::EsiV2506.specific(w);
        assert!((b / a - 1.0 - rel).abs() < 1e-12);
        assert_eq!(GasConstant::default(), GasConstant::Codata2019);
        assert_eq!(GasConstant::Codata2019.universal_j_per_kmol_k(), 1.0e3 * R_UNIVERSAL);
        assert_eq!(P_REF, 1.0e5);
    }
}
