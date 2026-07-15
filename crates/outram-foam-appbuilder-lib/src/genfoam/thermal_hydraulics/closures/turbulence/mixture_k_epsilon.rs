// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/physicsModels/turbulenceModels/
//             mixtureKEpsilon/{mixtureKEpsilon.C,mixtureKEpsilon.H}
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `mixture_k_epsilon` — mixture-model two-phase turbulence
//!
//! Port of GeN-Foam's `mixtureKEpsilon`: a shared-`k`/`epsilon` mixture
//! turbulence model for dispersed gas-liquid systems, based on Behzadi, Issa
//! & Rusche (2004) with an effective gas density (virtual-mass-corrected) and
//! a Lahey-style bubble-induced production term.
//!
//! References:
//! - Behzadi, A., Issa, R. I., & Rusche, H. (2004). *Modelling of dispersed
//!   bubble and droplet flow at high phase fractions.* Chemical Engineering
//!   Science, 59(4), 759-770.
//! - Lahey Jr., R. T. (2005). *The simulation of multidimensional multiphase
//!   flows.* Nuclear Engineering and Design, 235(10), 1043-1060 (bubble term).
//!
//! ## What this module ports
//!
//! [`MixtureKEpsilonClosure`] carries `Cmu`, `Cp`, `C3` and exposes:
//!
//! - [`MixtureKEpsilonClosure::dispersion_coefficient`] — `Ct2()`, the
//!   dispersed-phase turbulent-response coefficient (how closely the gas
//!   phase's turbulence tracks the liquid's).
//! - [`MixtureKEpsilonClosure::effective_gas_density`] — `rhogEff()`.
//! - [`MixtureKEpsilonClosure::mixture_density`] — `rhom()`.
//! - [`MixtureKEpsilonClosure::mix_k`] / [`MixtureKEpsilonClosure::mix_epsilon`]
//!   — the two instantiations of `mix()`.
//! - [`MixtureKEpsilonClosure::bubble_induced_production`] — `bubbleG()`
//!   (dimensionally distinct from the Lahey model's — see
//!   [`super::units::MixtureBubbleProduction`]).
//! - [`MixtureKEpsilonClosure::k_production_rate`] /
//!   [`MixtureKEpsilonClosure::epsilon_production_rate`] — `kSource()`/
//!   `epsilonSource()`, which upstream are already plain explicit sources
//!   (`fvm::Su`), so these need no further decomposition.
//!
//! `Cd()` is shared with `LaheyKEpsilon` and lives at
//! [`super::drag_coefficient_from_kd`]. `rholEff()` upstream is a trivial
//! passthrough (`return liquid().rho();`) — not ported as its own function;
//! callers pass the liquid density directly wherever `rholEff` would appear.
//! `mixFlux`/`mixU`/`Cc2` and the `correct()` phase-averaging orchestration
//! operate on `surfaceScalarField`s / look up sibling phase objects from the
//! mesh registry — deferred (see the parent module doc).

use super::drag_coefficient_from_kd;
use super::units::{
    dissipation_rate_of_change, mixture_bubble_production, DragCoefficient, HydraulicDiameter,
    MixtureBubbleProduction, RelativeVelocity, TurbulentDissipationRate,
    TurbulentDissipationRateOfChange, TurbulentKineticEnergy, VoidFraction,
    VolumetricRelaxationCoefficient,
};
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{MassDensity, Ratio};
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::ratio::ratio;
use uom::si::specific_power::watt_per_kilogram;
use uom::si::velocity::meter_per_second;

/// Mixture k-epsilon closure coefficients (Behzadi/Issa/Rusche + Lahey bubble
/// term).
///
/// Upstream defaults (`mixtureKEpsilonCoeffs`): `cmu = 0.09`, `cp = 0.25`
/// (from the base `LaheyKEpsilonCoeffs`-style default, reused here), `c3`
/// defaults to `C2` (typically `1.92`) unless overridden. `C1`/`C2`/
/// `sigmak`/`sigmaEps` belong to the reused generic k-epsilon transport
/// equation and are not duplicated here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MixtureKEpsilonClosure {
    /// Standard k-epsilon eddy-viscosity coefficient `Cmu` (dimensionless),
    /// used here only inside the dispersion-coefficient formula.
    pub cmu: f64,
    /// Bubble-induced-production coefficient `Cp` (dimensionless).
    pub cp: f64,
    /// Epsilon-production coefficient `C3` for the bubble-induced source
    /// (dimensionless).
    pub c3: f64,
}

impl MixtureKEpsilonClosure {
    /// Upstream default coefficients (`mixtureKEpsilonCoeffs` defaults, with
    /// `C3 = C2 = 1.92`).
    #[must_use]
    pub const fn new_default() -> Self {
        Self {
            cmu: 0.09,
            cp: 0.25,
            c3: 1.92,
        }
    }

    /// Invert `Cd` from the interfacial friction coefficient `Kd`. Thin
    /// forwarder to the formula shared with `LaheyKEpsilon`; see
    /// [`drag_coefficient_from_kd`] for the derivation.
    #[must_use]
    pub fn drag_coefficient(
        kd: VolumetricRelaxationCoefficient,
        dh_gas: HydraulicDiameter,
        rho_liquid: MassDensity,
        alpha_liquid: VoidFraction,
        alpha_gas: VoidFraction,
        relative_velocity: RelativeVelocity,
    ) -> DragCoefficient {
        drag_coefficient_from_kd(
            kd,
            dh_gas,
            rho_liquid,
            alpha_liquid,
            alpha_gas,
            relative_velocity,
        )
    }

    /// Dispersed-phase turbulent-response coefficient `Ct2`.
    ///
    /// Upstream `Ct2()`:
    /// ```text
    /// beta   = (6*Cmu / (4*sqrt(3/2))) * (Kd/rho_l) * (k_l/epsilon_l)
    /// Ct0    = (3 + beta) / (1 + beta + 2*rho_g/rho_l)
    /// fAlpha = (180 + (-4710 + 42600*alpha_g)*alpha_g) * alpha_g
    /// Ct2    = (1 + (Ct0 - 1)*exp(-fAlpha))^2
    /// ```
    /// `Ct2 -> 1` as `alpha_g -> 0` (`fAlpha -> 0`, `exp(0) = 1`, giving
    /// `(1 + (Ct0-1))^2 = Ct0^2`... note this is upstream's own formula, not
    /// simplified further here) and the gas phase tracks the liquid's `k`
    /// increasingly closely as bubbles get smaller / drag increases (`beta`
    /// grows).
    #[must_use]
    pub fn dispersion_coefficient(
        &self,
        kd: VolumetricRelaxationCoefficient,
        rho_liquid: MassDensity,
        rho_gas: MassDensity,
        liquid_k: TurbulentKineticEnergy,
        liquid_epsilon: TurbulentDissipationRate,
        alpha_gas: VoidFraction,
    ) -> Ratio {
        let rho_l = rho_liquid.get::<kilogram_per_cubic_meter>();
        let rho_g = rho_gas.get::<kilogram_per_cubic_meter>();
        let k_l = liquid_k.get::<joule_per_kilogram>();
        let eps_l = liquid_epsilon.get::<watt_per_kilogram>();
        let ag = alpha_gas.get::<ratio>();

        let beta = (6.0 * self.cmu / (4.0 * (1.5_f64).sqrt())) * (kd.value / rho_l) * (k_l / eps_l);
        let ct0 = (3.0 + beta) / (1.0 + beta + 2.0 * rho_g / rho_l);
        let f_alphad = (180.0 + (-4.71e3 + 4.26e4 * ag) * ag) * ag;
        let ct2 = (1.0 + (ct0 - 1.0) * (-f_alphad).exp()).powi(2);

        Ratio::new::<ratio>(ct2)
    }

    /// Virtual-mass-corrected effective gas density `rhogEff = rho_g +
    /// Vm*rho_l`.
    ///
    /// Upstream `rhogEff() = gas().rho() + pair().Vm()*liquid().rho()`.
    /// `virtual_mass_coefficient` is the (dimensionless) added-mass
    /// coefficient `Vm`, produced by the interfacial virtual-mass closures
    /// (out of scope here — see `closures::interfacial`, bead op-p6p.7.8).
    #[must_use]
    pub fn effective_gas_density(
        rho_gas: MassDensity,
        virtual_mass_coefficient: Ratio,
        rho_liquid: MassDensity,
    ) -> MassDensity {
        rho_gas + virtual_mass_coefficient * rho_liquid
    }

    /// Mixture density `rhom = alpha_l*rholEff + alpha_g*rhogEff`.
    ///
    /// Upstream `rhom() = liquid()*rholEff() + gas()*rhogEff()`.
    #[must_use]
    pub fn mixture_density(
        alpha_liquid: VoidFraction,
        rho_liquid_eff: MassDensity,
        alpha_gas: VoidFraction,
        rho_gas_eff: MassDensity,
    ) -> MassDensity {
        alpha_liquid * rho_liquid_eff + alpha_gas * rho_gas_eff
    }

    /// Mass-weighted mixture turbulent kinetic energy.
    ///
    /// Upstream `mix(kl, kg) = (liquid()*rholEff()*kl + gas()*rhogEff()*kg) /
    /// rhom()`, instantiated for `k`.
    #[must_use]
    pub fn mix_k(
        alpha_liquid: VoidFraction,
        rho_liquid_eff: MassDensity,
        liquid_k: TurbulentKineticEnergy,
        alpha_gas: VoidFraction,
        rho_gas_eff: MassDensity,
        gas_k: TurbulentKineticEnergy,
    ) -> TurbulentKineticEnergy {
        let rhom = Self::mixture_density(alpha_liquid, rho_liquid_eff, alpha_gas, rho_gas_eff);
        let al = alpha_liquid.get::<ratio>();
        let rl = rho_liquid_eff.get::<kilogram_per_cubic_meter>();
        let kl = liquid_k.get::<joule_per_kilogram>();
        let ag = alpha_gas.get::<ratio>();
        let rg = rho_gas_eff.get::<kilogram_per_cubic_meter>();
        let kg = gas_k.get::<joule_per_kilogram>();
        let rm = rhom.get::<kilogram_per_cubic_meter>();

        TurbulentKineticEnergy::new::<joule_per_kilogram>((al * rl * kl + ag * rg * kg) / rm)
    }

    /// Mass-weighted mixture turbulent dissipation rate.
    ///
    /// Upstream `mix(epsilonl, epsilong)`, instantiated for `epsilon`.
    #[must_use]
    pub fn mix_epsilon(
        alpha_liquid: VoidFraction,
        rho_liquid_eff: MassDensity,
        liquid_epsilon: TurbulentDissipationRate,
        alpha_gas: VoidFraction,
        rho_gas_eff: MassDensity,
        gas_epsilon: TurbulentDissipationRate,
    ) -> TurbulentDissipationRate {
        let rhom = Self::mixture_density(alpha_liquid, rho_liquid_eff, alpha_gas, rho_gas_eff);
        let al = alpha_liquid.get::<ratio>();
        let rl = rho_liquid_eff.get::<kilogram_per_cubic_meter>();
        let el = liquid_epsilon.get::<watt_per_kilogram>();
        let ag = alpha_gas.get::<ratio>();
        let rg = rho_gas_eff.get::<kilogram_per_cubic_meter>();
        let eg = gas_epsilon.get::<watt_per_kilogram>();
        let rm = rhom.get::<kilogram_per_cubic_meter>();

        TurbulentDissipationRate::new::<watt_per_kilogram>((al * rl * el + ag * rg * eg) / rm)
    }

    /// Mixture-model bubble-induced production term `bubbleG`.
    ///
    /// Upstream: `Cp * liquid()*liquid().rho() * (1 + Cd^(4/3)) * gas() *
    /// pow3(magUr) / max(gas().Dh(), 1e-3 m)`. Carries an extra `alpha_l*rho_l`
    /// factor relative to the Lahey model's `bubbleG` — upstream's own
    /// comment notes this makes the two dimensionally different; see
    /// [`MixtureBubbleProduction`].
    #[must_use]
    pub fn bubble_induced_production(
        &self,
        alpha_liquid: VoidFraction,
        rho_liquid: MassDensity,
        drag_coefficient: DragCoefficient,
        alpha_gas: VoidFraction,
        relative_velocity: RelativeVelocity,
        dh_gas: HydraulicDiameter,
    ) -> MixtureBubbleProduction {
        let al = alpha_liquid.get::<ratio>();
        let rl = rho_liquid.get::<kilogram_per_cubic_meter>();
        let cd = drag_coefficient.get::<ratio>();
        let ag = alpha_gas.get::<ratio>();
        let ur = relative_velocity.get::<meter_per_second>();
        let dh = dh_gas.get::<meter>().max(1e-3);

        let bubble_g = self.cp * al * rl * (1.0 + cd.powf(4.0 / 3.0)) * ag * ur.powi(3) / dh;

        mixture_bubble_production(bubble_g)
    }

    /// Specific mixture-`k` production rate — upstream `kSource() =
    /// fvm::Su(bubbleG()/rhom(), km())`, i.e. already an explicit source; this
    /// is just `bubbleG/rhom`.
    #[must_use]
    pub fn k_production_rate(
        bubble_induced_production: MixtureBubbleProduction,
        rho_mixture: MassDensity,
    ) -> TurbulentDissipationRate {
        TurbulentDissipationRate::new::<watt_per_kilogram>(
            bubble_induced_production.value / rho_mixture.get::<kilogram_per_cubic_meter>(),
        )
    }

    /// Specific mixture-`epsilon` production rate — upstream `epsilonSource()
    /// = fvm::Su(C3*epsilonm()*bubbleG()/(rhom()*km()), epsilonm())`.
    #[must_use]
    pub fn epsilon_production_rate(
        &self,
        epsilon_mixture: TurbulentDissipationRate,
        k_mixture: TurbulentKineticEnergy,
        bubble_induced_production: MixtureBubbleProduction,
        rho_mixture: MassDensity,
    ) -> TurbulentDissipationRateOfChange {
        let eps = epsilon_mixture.get::<watt_per_kilogram>();
        let k = k_mixture.get::<joule_per_kilogram>();
        let g = bubble_induced_production.value;
        let rm = rho_mixture.get::<kilogram_per_cubic_meter>();

        dissipation_rate_of_change(self.c3 * eps * g / (rm * k))
    }
}
