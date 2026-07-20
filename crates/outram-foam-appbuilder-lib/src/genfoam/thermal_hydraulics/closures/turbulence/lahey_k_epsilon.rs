// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/physicsModels/turbulenceModels/
//             LaheyKEpsilon/{LaheyKEpsilon.C,LaheyKEpsilon.H}
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `lahey_k_epsilon` — bubble-induced turbulence (Lahey, 2005)
//!
//! Port of GeN-Foam's `LaheyKEpsilon`: a continuous-liquid-phase k-epsilon
//! model with an added bubble-induced-turbulence source, valid for dispersed
//! gas / continuous liquid systems (per the upstream class doc: applying it to
//! continuous-gas systems runs but is not physically validated, since the
//! source terms hard-code the gas `Dh` and liquid `rho`).
//!
//! Reference: Lahey Jr., R. T. (2005). *The simulation of multidimensional
//! multiphase flows.* Nuclear Engineering and Design, 235(10), 1043-1060.
//!
//! ## What this module ports
//!
//! [`LaheyBubbleClosure`] carries the four upstream model coefficients
//! (`Cp`, `Cmub`, `C3`, `alphaInversion`; `Cmu`/`C1`/`C2`/`sigmak`/`sigmaEps`
//! belong to the reused generic k-epsilon and are not duplicated here) and
//! exposes:
//!
//! - [`LaheyBubbleClosure::bubble_induced_eddy_viscosity`] — the additive
//!   `nut` term from `correctNut()` (the generic `Cmu*k^2/epsilon` term is
//!   reused from `outram_foam_turbulence_lib`, not ported here).
//! - [`LaheyBubbleClosure::bubble_induced_production`] — `bubbleG()`.
//! - [`LaheyBubbleClosure::phase_transfer_rate`] — `phaseTransferCoeff()`,
//!   with the `rho` factor stripped out (see its doc for why).
//! - [`LaheyBubbleClosure::k_production_rate`] /
//!   [`LaheyBubbleClosure::epsilon_production_rate`] — the explicit parts of
//!   `kSource()`/`epsilonSource()`, recast as **specific** (per-unit-mass)
//!   rates so they compose with [`bubble_induced_production`]. The `alpha*rho`
//!   volumetric weighting and the `fvm::Sp`/`fvm::Su` matrix assembly are
//!   solver-integration concerns (see the parent module doc).
//!
//! `Cd()` is shared with `mixtureKEpsilon` and lives at
//! [`super::drag_coefficient_from_kd`].

use super::drag_coefficient_from_kd;
use super::units::{
    dissipation_rate_of_change, DragCoefficient, EddyViscosity, HydraulicDiameter,
    RelativeVelocity, TurbulentDissipationRate, TurbulentDissipationRateOfChange,
    TurbulentKineticEnergy, VoidFraction, VolumetricRelaxationCoefficient,
};
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{Frequency, MassDensity, Time};
use uom::si::frequency::hertz;
use uom::si::length::meter;
use uom::si::ratio::ratio;
use uom::si::specific_power::watt_per_kilogram;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

/// Bubble-induced-turbulence closure coefficients (Lahey, 2005).
///
/// Upstream defaults (from `LaheyKEpsilonCoeffs`): `cp = 0.25`,
/// `cmub = 0.6`, `c3 = -0.33` (upstream default; the constructor falls back to
/// `C2` — usually `1.92` — if `C3` is not set in the dictionary, but `-0.33`
/// is Lahey's own published value and the one documented in the class header),
/// `alpha_inversion = 0.3`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LaheyBubbleClosure {
    /// Bubble-induced-production coefficient `Cp` (dimensionless).
    pub cp: f64,
    /// Bubble-induced-viscosity coefficient `Cmub` (dimensionless).
    pub cmub: f64,
    /// Epsilon-production coefficient `C3` for the bubble-induced source
    /// (dimensionless; Lahey's value is `-0.33`, note the sign).
    pub c3: f64,
    /// Void-fraction threshold `alphaInversion` above which the gas phase is
    /// treated as continuous (no phase-transfer relaxation applied above it;
    /// dimensionless, `0..=1`).
    pub alpha_inversion: f64,
}

impl LaheyBubbleClosure {
    /// Upstream default coefficients (`LaheyKEpsilonCoeffs` defaults).
    #[must_use]
    pub const fn new_default() -> Self {
        Self {
            cp: 0.25,
            cmub: 0.6,
            c3: -0.33,
            alpha_inversion: 0.3,
        }
    }

    /// Invert `Cd` from the interfacial friction coefficient `Kd`. Thin
    /// forwarder to the formula shared with `mixtureKEpsilon`; see
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

    /// The bubble-induced addend to the turbulent (eddy) viscosity.
    ///
    /// Upstream: `nut += Cmub * gas().Dh() * gas() * mag(pair().magUr())`
    /// (`correctNut()`). The generic `Cmu*k^2/epsilon` term that this is
    /// *added to* is the reused generic k-epsilon closure and is not part of
    /// this function.
    #[must_use]
    pub fn bubble_induced_eddy_viscosity(
        &self,
        dh_gas: HydraulicDiameter,
        alpha_gas: VoidFraction,
        relative_velocity: RelativeVelocity,
    ) -> EddyViscosity {
        let nu_t_bi = self.cmub
            * dh_gas.get::<meter>()
            * alpha_gas.get::<ratio>()
            * relative_velocity.get::<meter_per_second>();

        EddyViscosity::new::<uom::si::kinematic_viscosity::square_meter_per_second>(nu_t_bi)
    }

    /// Bubble-induced turbulent-kinetic-energy production `bubbleG`.
    ///
    /// Upstream: `Cp*(1 + Cd^(4/3)) * gas() * pow3(magUr) /
    /// max(gas().Dh(), 1e-3 m)` (`bubbleG()`). A genuine specific (per-unit-
    /// mass) production rate, `dk/dt`-shaped — see [`TurbulentDissipationRate`].
    #[must_use]
    pub fn bubble_induced_production(
        &self,
        drag_coefficient: DragCoefficient,
        alpha_gas: VoidFraction,
        relative_velocity: RelativeVelocity,
        dh_gas: HydraulicDiameter,
    ) -> TurbulentDissipationRate {
        let cd = drag_coefficient.get::<ratio>();
        let ag = alpha_gas.get::<ratio>();
        let ur = relative_velocity.get::<meter_per_second>();
        let dh = dh_gas.get::<meter>().max(1e-3);

        let bubble_g = self.cp * (1.0 + cd.powf(4.0 / 3.0)) * ag * ur.powi(3) / dh;

        TurbulentDissipationRate::new::<watt_per_kilogram>(bubble_g)
    }

    /// Gas-to-liquid turbulence phase-transfer relaxation rate.
    ///
    /// Upstream: `phaseTransferCoeff() = max(alphaInversion - alpha, 0) * rho
    /// * min(epsilon_g/k_g, 1/dt)`. This port **omits the `rho` factor** and
    /// returns a pure relaxation [`Frequency`] (`1/s`) instead: `rho` (and the
    /// implicit cell-volume factor baked into `fvm::Sp`) are solver-level
    /// volumetric bookkeeping, not part of this closure's physical content —
    /// see [`k_production_rate`](Self::k_production_rate) for how the caller
    /// recombines it. Multiply the result by the local liquid density to
    /// reproduce the upstream `pTC` exactly.
    #[must_use]
    pub fn phase_transfer_rate(
        &self,
        alpha_gas: VoidFraction,
        gas_k: TurbulentKineticEnergy,
        gas_epsilon: TurbulentDissipationRate,
        timestep: Time,
    ) -> Frequency {
        let gap = (self.alpha_inversion - alpha_gas.get::<ratio>()).max(0.0);
        let eps_over_k = gas_epsilon.get::<watt_per_kilogram>() / gas_k.get::<joule_per_kilogram>();
        let inv_dt = 1.0 / timestep.get::<second>();
        let rate = gap * eps_over_k.min(inv_dt);

        Frequency::new::<hertz>(rate)
    }

    /// Net specific (per-unit-mass) rate of change of `k` from the bubble and
    /// phase-transfer sources — `dk/dt` from the explicit part of `kSource()`,
    /// divided through by `alpha*rho`:
    ///
    /// ```text
    /// dk/dt|_bubble = alpha_gas * bubbleG + phase_transfer_rate * (k_gas - k)
    /// ```
    ///
    /// (upstream: `alpha*rho*bubbleG() + pTC*k_gas - fvm::Sp(pTC, k)`, which
    /// at the current field value equals `alpha*rho*bubbleG() + pTC*(k_gas -
    /// k)`; dividing by `rho` and using [`phase_transfer_rate`] in place of
    /// `pTC/rho` gives the expression above). The solver multiplies this by
    /// `alpha*rho` (and applies the cell-volume factor) to assemble the actual
    /// matrix source.
    #[must_use]
    pub fn k_production_rate(
        &self,
        alpha_gas: VoidFraction,
        bubble_induced_production: TurbulentDissipationRate,
        phase_transfer_rate: Frequency,
        gas_k: TurbulentKineticEnergy,
        local_k: TurbulentKineticEnergy,
    ) -> TurbulentDissipationRate {
        let ag = alpha_gas.get::<ratio>();
        let g = bubble_induced_production.get::<watt_per_kilogram>();
        let rate = phase_transfer_rate.get::<hertz>();
        let dk = gas_k.get::<joule_per_kilogram>() - local_k.get::<joule_per_kilogram>();

        TurbulentDissipationRate::new::<watt_per_kilogram>(ag * g + rate * dk)
    }

    /// Net rate of change of `epsilon` from the bubble and phase-transfer
    /// sources — `d(epsilon)/dt` from the explicit part of `epsilonSource()`,
    /// divided through by `alpha*rho`:
    ///
    /// ```text
    /// d(epsilon)/dt|_bubble =
    ///     alpha_gas * C3 * (epsilon/k) * bubbleG
    ///   + phase_transfer_rate * (epsilon_gas - epsilon)
    /// ```
    ///
    /// (upstream: `alpha*rho*C3*epsilon*bubbleG()/k + pTC*epsilon_gas -
    /// fvm::Sp(pTC, epsilon)`, divided by `rho` as in
    /// [`k_production_rate`](Self::k_production_rate)).
    #[must_use]
    pub fn epsilon_production_rate(
        &self,
        alpha_gas: VoidFraction,
        epsilon: TurbulentDissipationRate,
        k: TurbulentKineticEnergy,
        bubble_induced_production: TurbulentDissipationRate,
        phase_transfer_rate: Frequency,
        gas_epsilon: TurbulentDissipationRate,
    ) -> TurbulentDissipationRateOfChange {
        let ag = alpha_gas.get::<ratio>();
        let eps = epsilon.get::<watt_per_kilogram>();
        let k_v = k.get::<joule_per_kilogram>();
        let g = bubble_induced_production.get::<watt_per_kilogram>();
        let rate = phase_transfer_rate.get::<hertz>();
        let d_eps = gas_epsilon.get::<watt_per_kilogram>() - eps;

        let d_eps_dt = ag * self.c3 * (eps / k_v) * g + rate * d_eps;

        dissipation_rate_of_change(d_eps_dt)
    }
}
