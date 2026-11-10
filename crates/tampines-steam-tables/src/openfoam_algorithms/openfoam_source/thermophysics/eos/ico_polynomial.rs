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

use crate::openfoam_algorithms::openfoam_source::imports::*;
use crate::openfoam_algorithms::openfoam_source::constants::R_UNIVERSAL;
use crate::openfoam_algorithms::openfoam_source::Polynomial;
use super::traits::EquationOfState;

/// Incompressible polynomial EOS: `rho(T) = poly(T)`.
///
/// Port of `Foam::icoPolynomial<Specie, PolySize>`, OpenFOAM v2506 (ESI)
/// `src/thermophysicalModels/specie/equationOfState/icoPolynomial/icoPolynomialI.H`:
/// `rho` returns `rhoCoeffs_.value(T)` (line 83), and `H`, `Cp`, `E`, `Cv`,
/// `S`, `psi`, `Z` and `CpMCv` all return `0` (lines 94-171).
///
/// ~~The polynomial gives specific volume as a function of T, `ρ = 1/poly(T)`;
/// ψ = 0; h_eos = p·v = p/ρ.~~ **CORRECTED 2026-11-10 (gh:#1238):** that was
/// not what upstream does; upstream reads `rhoCoeffs<N>` as the **density**,
/// and its enthalpy and compressibility-factor departures are zero. The same
/// fix as `outram-foam-basic-lib::thermophysics::eos::IcoPolynomial`
/// (commit `66605b3dc`, gh:#1218), ported here unchanged. This copy has no
/// caller in the workspace (checked 2026-11-10), so no recorded number moves.
#[derive(Debug, Clone, Copy)]
pub struct IcoPolynomial<const N: usize> {
    mol_weight: MolarMass,
    poly: Polynomial<N>,
}

impl<const N: usize> IcoPolynomial<N> {
    /// `poly` coefficients give the density `[kg/m³]` as a polynomial in T `[K]`
    /// (upstream's `rhoCoeffs<N>`).
    pub fn new(mol_weight: MolarMass, poly: Polynomial<N>) -> Self {
        Self { mol_weight, poly }
    }
}

impl<const N: usize> EquationOfState for IcoPolynomial<N> {
    fn mol_weight(&self) -> MolarMass {
        self.mol_weight
    }

    fn r(&self) -> SpecificHeatCapacity {
        let w = self.mol_weight.get::<kilogram_per_mole>();
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(R_UNIVERSAL / w)
    }

    fn rho(&self, _p: Pressure, t: ThermodynamicTemperature) -> MassDensity {
        MassDensity::new::<kilogram_per_cubic_meter>(self.poly.value(t.get::<kelvin>()))
    }

    fn psi(&self, _p: Pressure, _t: ThermodynamicTemperature) -> Compressibility {
        MassDensity::new::<kilogram_per_cubic_meter>(0.0) / Pressure::new::<pascal>(1.0)
    }

    fn z(&self, _p: Pressure, _t: ThermodynamicTemperature) -> Ratio {
        // Upstream `icoPolynomial::Z` returns 0 (icoPolynomialI.H).
        use uom::si::ratio::ratio;
        Ratio::new::<ratio>(0.0)
    }

    fn cp_m_cv(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(0.0)
    }

    fn cp_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(0.0)
    }

    fn h_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> AvailableEnergy {
        // Upstream `icoPolynomial::H` returns 0 (icoPolynomialI.H:94).
        AvailableEnergy::new::<joule_per_kilogram>(0.0)
    }

    fn e_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> AvailableEnergy {
        AvailableEnergy::new::<joule_per_kilogram>(0.0)
    }

    fn s_eos(&self, _p: Pressure, _t: ThermodynamicTemperature) -> SpecificHeatCapacity {
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::molar_mass::gram_per_mole;
    use uom::si::thermodynamic_temperature::kelvin;
    use uom::si::pressure::pascal;
    use uom::si::mass_density::kilogram_per_cubic_meter;
    use uom::si::available_energy::joule_per_kilogram;
    use approx::assert_relative_eq;

    /// Two-term specific-volume polynomial for water, valid near 300 K.
    /// Fit: v(T) ≈ 1/998.0 + 4e-7·(T−298.15)·(1/998.0)²  (simplified)
    /// Here we use a constant polynomial (single term) as a sanity check.
    fn water_const() -> IcoPolynomial<1> {
        // rho = 998.0 kg/m³  (constant density ≈ RhoConst)
        IcoPolynomial::new(
            MolarMass::new::<gram_per_mole>(18.015),
            Polynomial::new([998.0_f64]),
        )
    }

    fn water_linear() -> IcoPolynomial<2> {
        // rho(T) = 1100 - 0.34·T  (≈ 998 kg/m³ at 300 K, falling with T)
        IcoPolynomial::new(
            MolarMass::new::<gram_per_mole>(18.015),
            Polynomial::new([1100.0_f64, -0.34]),
        )
    }

    #[test]
    fn constant_poly_density() {
        let w = water_const();
        let p = Pressure::new::<pascal>(101_325.0);
        let t = ThermodynamicTemperature::new::<kelvin>(300.0);
        let rho = w.rho(p, t).get::<kilogram_per_cubic_meter>();
        assert_relative_eq!(rho, 998.0, epsilon = 0.01);
    }

    #[test]
    fn psi_is_zero() {
        let w = water_linear();
        let p = Pressure::new::<pascal>(101_325.0);
        let t = ThermodynamicTemperature::new::<kelvin>(300.0);
        let psi_times_p = (w.psi(p, t) * p).get::<kilogram_per_cubic_meter>();
        assert_relative_eq!(psi_times_p, 0.0, epsilon = 1e-10);
    }

    /// Upstream `icoPolynomial::H` and `Z` return 0 (OpenFOAM v2506
    /// `icoPolynomialI.H`); the earlier port returned `p/rho` (gh:#1238).
    #[test]
    fn h_eos_and_z_are_zero_as_upstream() {
        let w = water_linear();
        let p = Pressure::new::<pascal>(1e6);
        let t = ThermodynamicTemperature::new::<kelvin>(300.0);
        assert_eq!(w.h_eos(p, t).get::<joule_per_kilogram>(), 0.0);
        assert_eq!(w.z(p, t).get::<uom::si::ratio::ratio>(), 0.0);
    }

    /// GeN-Foam 1D_boiling sodium `rhoCoeffs<8>` evaluate to a liquid-sodium
    /// density, which only the density reading gives (gh:#1218, gh:#1238).
    /// Same check as basic-lib's `sodium_rho_coeffs_are_a_density`.
    #[test]
    fn sodium_rho_coeffs_are_a_density() {
        let na = IcoPolynomial::new(
            MolarMass::new::<gram_per_mole>(22.989769),
            Polynomial::new([1006.81, -2.17e-1, -2.83e-6, -6.54e-9, 0.0, 0.0, 0.0, 0.0]),
        );
        let p = Pressure::new::<pascal>(1e5);
        let t = ThermodynamicTemperature::new::<kelvin>(652.15);
        let rho = na.rho(p, t).get::<kilogram_per_cubic_meter>();
        let expected = 1006.81 - 0.217 * 652.15 - 2.83e-6 * 652.15 * 652.15
            - 6.54e-9 * 652.15 * 652.15 * 652.15;
        assert_relative_eq!(rho, expected, epsilon = 1e-9);
        assert!((850.0..870.0).contains(&rho));
    }

    #[test]
    fn density_decreases_with_temperature() {
        let w = water_linear();
        let p = Pressure::new::<pascal>(101_325.0);
        let t1 = ThermodynamicTemperature::new::<kelvin>(300.0);
        let t2 = ThermodynamicTemperature::new::<kelvin>(310.0);
        let rho1 = w.rho(p, t1).get::<kilogram_per_cubic_meter>();
        let rho2 = w.rho(p, t2).get::<kilogram_per_cubic_meter>();
        assert!(
            rho1 > rho2,
            "rho should decrease with T for positive slope poly"
        );
    }
}
