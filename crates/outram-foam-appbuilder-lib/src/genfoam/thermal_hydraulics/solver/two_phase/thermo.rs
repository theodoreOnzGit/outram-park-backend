// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! Per-phase thermophysical packages the two-phase tutorials select.
//!
//! GeN-Foam's `fluid` owns an OpenFOAM `rhoThermo` (`fluid.C`, `thermo_`).
//! Every two-phase feature case of `652b3da` (1D_boiling, 1D_PSBT_SC, 1D_CHF,
//! 2D_KNS37-L22, 2D_voidMotionNoPhaseChange) selects `heRhoThermo` with
//! `energy sensibleEnthalpy` and one of two stacks, both already ported in
//! `outram-foam-basic-lib::thermophysics` and reused here, not re-written:
//!
//! - liquid: `icoPolynomial` + `hPolynomial` + `polynomial` transport
//!   ([`PhaseThermo::Polynomial`]);
//! - vapour: `perfectGas` + `hConst` + `const` transport
//!   ([`PhaseThermo::PerfectGasConst`]).
//!
//! `he` is the sensible enthalpy `hs(p, T)`; `T` is recovered from `he` by
//! basic-lib's Newton `t_from_hs` (OpenFOAM `species::thermo::THs`), and the
//! energy diffusivity is `alphahe = kappa/Cp` (laminar; `alphat = 0`).

use outram_foam_basic_lib::prelude::*;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{AvailableEnergy, DynamicViscosity, MolarMass, Pressure, Ratio, SpecificHeatCapacity, ThermodynamicTemperature};
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::molar_mass::gram_per_mole;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

/// The liquid stack: `icoPolynomial` EOS, `hPolynomial` thermo,
/// `polynomial` transport, eight coefficients each (the tutorials' `<8>`).
pub type PolynomialStack = PolynomialTransport<HPolynomialThermo<IcoPolynomial<8>, 8>, 8>;

/// The vapour stack: `perfectGas` EOS, `hConst` thermo, `const` transport.
pub type PerfectGasConstStack = ConstTransport<HConstThermo<PerfectGas>>;

/// One phase's thermophysical package (closed enum, no `dyn`).
#[derive(Debug, Clone)]
pub enum PhaseThermo {
    /// `heRhoThermo<icoPolynomial, hPolynomial, polynomial>`.
    Polynomial(PolynomialStack),
    /// `heRhoThermo<perfectGas, hConst, const>`.
    PerfectGasConst(PerfectGasConstStack),
}

/// Point properties at one `(p, T)`, SI base units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointProps {
    /// Density [kg/m^3].
    pub rho: f64,
    /// Compressibility `psi = d rho/d p` at constant T [s^2/m^2].
    pub psi: f64,
    /// Specific heat [J/(kg K)].
    pub cp: f64,
    /// Dynamic viscosity [Pa s].
    pub mu: f64,
    /// Thermal conductivity [W/(m K)].
    pub kappa: f64,
}

impl PhaseThermo {
    /// The liquid stack from the dictionary coefficients: molar weight
    /// [g/mol], `rhoCoeffs`, `CpCoeffs`, `Hf` [J/kg], `Sf` [J/(kg K)],
    /// `muCoeffs`, `kappaCoeffs` (OpenFOAM `thermophysicalProperties`).
    #[must_use]
    pub fn polynomial(
        mol_weight_g_per_mol: f64,
        rho_coeffs: [f64; 8],
        cp_coeffs: [f64; 8],
        hf: f64,
        sf: f64,
        mu_coeffs: [f64; 8],
        kappa_coeffs: [f64; 8],
    ) -> Self {
        let eos = IcoPolynomial::new(
            MolarMass::new::<gram_per_mole>(mol_weight_g_per_mol),
            Polynomial::new(rho_coeffs),
        );
        let thermo = HPolynomialThermo::new(
            eos,
            Polynomial::new(cp_coeffs),
            AvailableEnergy::new::<joule_per_kilogram>(hf),
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(sf),
        );
        Self::Polynomial(PolynomialTransport::new(
            thermo,
            Polynomial::new(mu_coeffs),
            Polynomial::new(kappa_coeffs),
        ))
    }

    /// The vapour stack: molar weight [g/mol], `Cp` [J/(kg K)], `Hf` [J/kg],
    /// `mu` [Pa s], `Pr`. `Tref`/`Hsref` take OpenFOAM's defaults (`Tstd`,
    /// 0), which no tutorial overrides.
    #[must_use]
    pub fn perfect_gas_const(mol_weight_g_per_mol: f64, cp: f64, hf: f64, mu: f64, pr: f64) -> Self {
        let eos = PerfectGas::new(MolarMass::new::<gram_per_mole>(mol_weight_g_per_mol));
        let thermo = HConstThermo::new(
            eos,
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(cp),
            AvailableEnergy::new::<joule_per_kilogram>(hf),
            ThermodynamicTemperature::new::<kelvin>(298.15),
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
        );
        Self::PerfectGasConst(ConstTransport::new(
            thermo,
            DynamicViscosity::new::<pascal_second>(mu),
            Ratio::new::<ratio>(pr),
        ))
    }

    /// Sensible enthalpy `hs(p, T)` [J/kg] (`he` for `sensibleEnthalpy`).
    #[must_use]
    pub fn he(&self, p: f64, t: f64) -> f64 {
        let (p, t) = (Pressure::new::<pascal>(p), ThermodynamicTemperature::new::<kelvin>(t));
        match self {
            Self::Polynomial(s) => s.hs(p, t),
            Self::PerfectGasConst(s) => s.hs(p, t),
        }
        .get::<joule_per_kilogram>()
    }

    /// Temperature from sensible enthalpy by Newton from `t0` (OpenFOAM
    /// `THs`). Falls back to `t0` if Newton does not converge (upstream
    /// aborts; the solver reports the cell instead, see the driver).
    #[must_use]
    pub fn t_from_he(&self, he: f64, p: f64, t0: f64) -> Option<f64> {
        let (h, p, t0) = (
            AvailableEnergy::new::<joule_per_kilogram>(he),
            Pressure::new::<pascal>(p),
            ThermodynamicTemperature::new::<kelvin>(t0),
        );
        let r = match self {
            Self::Polynomial(s) => s.t_from_hs(h, p, t0),
            Self::PerfectGasConst(s) => s.t_from_hs(h, p, t0),
        };
        r.ok().map(|t| t.get::<kelvin>())
    }

    /// Density, compressibility, `Cp`, `mu`, `kappa` at `(p, T)`.
    #[must_use]
    pub fn props(&self, p: f64, t: f64) -> PointProps {
        let (p, t) = (Pressure::new::<pascal>(p), ThermodynamicTemperature::new::<kelvin>(t));
        match self {
            Self::Polynomial(s) => PointProps {
                rho: s.rho(p, t).get::<kilogram_per_cubic_meter>(),
                psi: (s.psi(p, t) * Pressure::new::<pascal>(1.0))
                    .get::<kilogram_per_cubic_meter>(),
                cp: s.cp(p, t).get::<joule_per_kilogram_kelvin>(),
                mu: s.mu(p, t).get::<pascal_second>(),
                kappa: s.kappa(p, t).get::<watt_per_meter_kelvin>(),
            },
            Self::PerfectGasConst(s) => PointProps {
                rho: s.rho(p, t).get::<kilogram_per_cubic_meter>(),
                psi: (s.psi(p, t) * Pressure::new::<pascal>(1.0))
                    .get::<kilogram_per_cubic_meter>(),
                cp: s.cp(p, t).get::<joule_per_kilogram_kelvin>(),
                mu: s.mu(p, t).get::<pascal_second>(),
                kappa: s.kappa(p, t).get::<watt_per_meter_kelvin>(),
            },
        }
    }
}
