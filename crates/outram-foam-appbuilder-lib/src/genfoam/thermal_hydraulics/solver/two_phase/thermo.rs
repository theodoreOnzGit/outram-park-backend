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

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
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

/// The gas stack of `3D_HTR-10`'s helium (gh:#1876, README round 87): `perfectGas` EOS, `hPolynomial` thermo, `polynomial`
/// transport, eight coefficients each; the universal gas constant is ESI v2506's (as [`PhaseThermo::perfect_gas_const`]).
pub type PerfectGasPolynomialStack = PolynomialTransport<HPolynomialThermo<PerfectGas, 8>, 8>;

/// One phase's thermophysical package (closed enum, no `dyn`).
#[derive(Debug, Clone)]
pub enum PhaseThermo {
    /// `heRhoThermo<icoPolynomial, hPolynomial, polynomial>`.
    Polynomial(PolynomialStack),
    /// `heRhoThermo<perfectGas, hConst, const>`.
    PerfectGasConst(PerfectGasConstStack),
    /// `heRhoThermo<pureMixture<const<hConst<Boussinesq<specie>>>>>` (gh:#1878 item 3): `rho = rho0 (1 - beta (T - T0))`,
    /// `psi = 0`, `Hs = Cp (T - Tref) + Hsref`. Upstream's `fluid::rho()` is the CONSTANT `rho0` for it
    /// ([`PhaseThermo::boussinesq_rho0`]); this thermo's `rho` is `fluid::rho(true)`, used only in the buoyancy flux.
    BoussinesqHConst(super::one_phase_cc::BoussinesqHConst),
    /// `heRhoThermo<pureMixture<polynomial<hPolynomial<perfectGas<specie>>>>>` (gh:#1876, README round 87; `3D_HTR-10`'s helium):
    /// `rho = p/(R T)`, `psi = 1/(R T)`, `Cp` and `Hs` from the `CpCoeffs` polynomial (perfectGas adds no departure terms).
    PerfectGasPolynomial(PerfectGasPolynomialStack),
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

    /// The `perfectGas` + `hPolynomial` + `polynomial` stack from the dictionary coefficients: molar weight [g/mol], `CpCoeffs`,
    /// `Hf` [J/kg], `Sf` [J/(kg K)], `muCoeffs`, `kappaCoeffs` (gh:#1876). The gas constant is OpenFOAM-v2506's
    /// (`GasConstant::EsiV2506`), the binary this port is compared with.
    #[must_use]
    pub fn perfect_gas_polynomial(
        mol_weight_g_per_mol: f64,
        cp_coeffs: [f64; 8],
        hf: f64,
        sf: f64,
        mu_coeffs: [f64; 8],
        kappa_coeffs: [f64; 8],
    ) -> Self {
        let eos = PerfectGas::new(MolarMass::new::<gram_per_mole>(mol_weight_g_per_mol)).with_gas_constant(GasConstant::EsiV2506);
        let thermo = HPolynomialThermo::new(
            eos,
            Polynomial::new(cp_coeffs),
            AvailableEnergy::new::<joule_per_kilogram>(hf),
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(sf),
        );
        Self::PerfectGasPolynomial(PolynomialTransport::new(thermo, Polynomial::new(mu_coeffs), Polynomial::new(kappa_coeffs)))
    }

    /// The vapour stack: molar weight [g/mol], `Cp` [J/(kg K)], `Hf` [J/kg],
    /// `mu` [Pa s], `Pr`. `Tref`/`Hsref` take OpenFOAM's defaults (`Tstd`,
    /// 0), which no tutorial overrides.
    ///
    /// The universal gas constant is OpenFOAM-v2506's (`GasConstant::EsiV2506`, `RR = 1e3 NA k` with the
    /// `etc/controlDict` constants), because the binary this port is compared with is ESI v2506. ~~CODATA 2019~~
    /// **CORRECTED 2026-11-26** (gh:#1361 round 13): the CODATA value made the vapour density 8.96e-7 relative
    /// larger than upstream's, the seed of the 1e-9 to 1e-5 differences at the first boiling evaluation of the
    /// TRACE closures (the Basu onset temperature reads the vapour density). The earlier value stays callable as
    /// [`Self::perfect_gas_const_codata2019`] and [`Self::with_gas_constant`].
    #[must_use]
    pub fn perfect_gas_const(mol_weight_g_per_mol: f64, cp: f64, hf: f64, mu: f64, pr: f64) -> Self {
        Self::perfect_gas_const_codata2019(mol_weight_g_per_mol, cp, hf, mu, pr).with_gas_constant(GasConstant::EsiV2506)
    }

    /// [`Self::perfect_gas_const`] with the CODATA 2019 universal gas constant (the port's form before gh:#1361
    /// round 13; 8.96e-7 relative away from upstream's vapour density). Deprecated: kept to reproduce earlier numbers.
    #[must_use]
    pub fn perfect_gas_const_codata2019(mol_weight_g_per_mol: f64, cp: f64, hf: f64, mu: f64, pr: f64) -> Self {
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

    /// The same package with the universal gas constant of a `perfectGas` vapour set to `gas_constant`
    /// (a liquid `icoPolynomial` package has none and is returned unchanged).
    #[must_use]
    pub fn with_gas_constant(self, gas_constant: GasConstant) -> Self {
        match self {
            Self::PerfectGasConst(s) => Self::PerfectGasConst(s.with_gas_constant(gas_constant)),
            other => other,
        }
    }

    /// Sensible enthalpy `hs(p, T)` [J/kg] (`he` for `sensibleEnthalpy`).
    #[must_use]
    pub fn he(&self, p: f64, t: f64) -> f64 {
        let (p, t) = (Pressure::new::<pascal>(p), ThermodynamicTemperature::new::<kelvin>(t));
        match self {
            Self::Polynomial(s) => s.hs(p, t),
            Self::PerfectGasConst(s) => s.hs(p, t),
            Self::PerfectGasPolynomial(s) => s.hs(p, t),
            Self::BoussinesqHConst(b) => return b.hs(t.get::<kelvin>()),
        }
        .get::<joule_per_kilogram>()
    }

    /// Temperature from sensible enthalpy as `species::thermo::T` does it (gh:#1361 round 13): Newton from `t0` on
    /// `hs(p, T) - he` with the derivative `Cp(p, T)`, in a `do { ... } while (mag(Tnew - Test) > Ttol)` loop with
    /// `Ttol = T0 * 1e-4` (`thermo::tol_`) and at most 100 iterations. The loop stops after the first step smaller
    /// than 1e-4 of `T0` (about 0.06 K), so the returned `T` is that Newton iterate, NOT the converged root: it
    /// differs from the root by the quadratic remainder (3e-6 K for a 0.02 K step in the sub-cooled liquid, where
    /// `dln(Cp)/dT` is 1.5e-2 /K). Returns `None` past 100 iterations (upstream aborts).
    #[must_use]
    pub fn t_from_he(&self, he: f64, p: f64, t0: f64) -> Option<f64> {
        let ttol = t0 * 1.0e-4;
        let mut tnew = t0;
        let mut iter = 0;
        loop {
            let test = tnew;
            tnew = test - (self.he(p, test) - he) / self.props(p, test).cp;
            if iter > 100 {
                return None;
            }
            iter += 1;
            if (tnew - test).abs() <= ttol {
                return Some(tnew);
            }
        }
    }

    /// DEPRECATED (gh:#1361 round 13): temperature from sensible enthalpy by Newton to `|dT/T| < 1e-6` (the basic-lib
    /// `t_from_hs`), the form the port used before round 13. It returns a more converged root than upstream's
    /// [`Self::t_from_he`], so `T`, `Cp`, `rho`, `kappa` differ from the real binary's by up to 1e-8 relative.
    #[must_use]
    pub fn t_from_he_converged(&self, he: f64, p: f64, t0: f64) -> Option<f64> {
        let (h, p, t0) = (
            AvailableEnergy::new::<joule_per_kilogram>(he),
            Pressure::new::<pascal>(p),
            ThermodynamicTemperature::new::<kelvin>(t0),
        );
        let r = match self {
            Self::Polynomial(s) => s.t_from_hs(h, p, t0),
            Self::PerfectGasConst(s) => s.t_from_hs(h, p, t0),
            Self::PerfectGasPolynomial(s) => s.t_from_hs(h, p, t0),
            Self::BoussinesqHConst(b) => return Some(b.t_from_hs(he, t0.get::<kelvin>())),
        };
        r.ok().map(|t| t.get::<kelvin>())
    }

    /// `fluid::rho()` of a `Boussinesq` equation of state: the constant `rho0` (`fluid.C:795`); `None` for every other package.
    #[must_use]
    pub fn boussinesq_rho0(&self) -> Option<f64> {
        match self {
            Self::BoussinesqHConst(b) => Some(b.rho0),
            _ => None,
        }
    }

    /// `constTransport::alphah = mu / Pr`, which `heRhoThermo::calculate` stores as the `alpha` field and
    /// `alphaEff` returns for a laminar flow; `None` for the packages whose `alphah` is `kappa / Cp` (what the solver computes).
    #[must_use]
    pub fn alpha_h_const(&self) -> Option<f64> {
        match self {
            Self::BoussinesqHConst(b) => Some(b.alpha_h()),
            _ => None,
        }
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
            Self::BoussinesqHConst(b) => PointProps {
                rho: b.rho(t.get::<kelvin>()),
                psi: 0.0,
                cp: b.cp,
                mu: b.mu,
                kappa: b.cp * b.alpha_h(),
            },
            Self::PerfectGasPolynomial(s) => PointProps {
                rho: s.rho(p, t).get::<kilogram_per_cubic_meter>(),
                psi: (s.psi(p, t) * Pressure::new::<pascal>(1.0)).get::<kilogram_per_cubic_meter>(),
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
