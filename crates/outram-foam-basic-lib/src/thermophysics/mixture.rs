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

//! Multicomponent mixture rules of OpenFOAM-v2506 (ESI), gh:#1378 family 3:
//! `specie::operator*`/`+=` (mass-fraction `Y` and molecular weight `W`
//! bookkeeping), each model's `operator+=` (mass-fraction-weighted parameters),
//! and the two mixing loops of `multiComponentMixture`: `cellMixture`
//! (mass-fraction weights), `cellVolMixture` (volume-fraction weights), plus
//! `correctMassFractions`.
//!
//! # Upstream structure reproduced
//! In v2506 a species model is a stack `transport<thermo<eos<specie>>>`; the
//! mixture is `mixture = Y0 * s0; mixture += Y1 * s1; ...`. Every layer's
//! `operator+=` saves `Y1 = Y()` BEFORE calling the layer below, the bottom
//! `specie::operator+=` updates `Y` and `W`, and every layer then mixes its own
//! parameters with `Y1/Y_new` and `Y_other/Y_new`. All layers therefore use the
//! same two weights, which is why the port can keep `Y` and `W` in one
//! [`MixtureComponent`] wrapper and give each model only [`Mixable`]'s two
//! parameter-level methods.
//!
//! # Scope
//! [`Mixable`] is implemented for `PerfectGas`, `RhoConst`, `HConstThermo`,
//! `ConstTransport` and `SutherlandTransport` (each next to its type). The other
//! models are deferred (gh:#1378 children): their `operator+=` bodies are
//! in the same `*I.H` files and follow the same pattern; `janafThermo` also
//! intersects the temperature ranges and checks `Tcommon`.
//!
//! The `ReactionTable`, the `Y` fields and the mesh are not ported here.

use crate::primitives::{ROOT_VSMALL, SMALL};
use crate::thermophysics::imports::*;

/// Parameter-level mixing of one model layer, the part of upstream's
/// `operator+=` that follows the `Y`/`W` update.
pub trait Mixable: Clone {
    /// Set the molecular weight of the innermost equation of state
    /// (upstream `specie::molWeight_`, updated by `specie::operator+=`).
    fn set_mol_weight(&mut self, w: MolarMass);

    /// `self = y1 * self + y2 * other` on this model's own parameters, with the
    /// weights upstream computes: `y1 = Y_old/Y_new`, `y2 = Y_other/Y_new`.
    fn mix_params(&mut self, y1: f64, y2: f64, other: &Self);
}

/// `kg/mol` that, multiplied by `1e3`, gives exactly `w_kg_per_kmol`
/// (OpenFOAM's `W` is in kg/kmol; the port stores kg/mol and forms
/// `R = RR/(W*1e3)`, so the unit round trip must be exact for bit-identity,
/// as in gh:#1362). Falls back to `w * 1e-3` if no neighbour round-trips.
pub fn molar_mass_from_kg_per_kmol(w_kg_per_kmol: f64) -> MolarMass {
    let w0 = w_kg_per_kmol * 1.0e-3;
    for d in 0..8i64 {
        for sgn in [1i64, -1] {
            let cand = f64::from_bits((w0.to_bits() as i64 + sgn * d) as u64);
            if cand * 1.0e3 == w_kg_per_kmol {
                return MolarMass::new::<kilogram_per_mole>(cand);
            }
        }
    }
    MolarMass::new::<kilogram_per_mole>(w0)
}

/// A species model with its mass fraction `Y` and molecular weight `W`
/// (kg/kmol): upstream's `specie` base carrying `Y_` and `molWeight_`.
#[derive(Debug, Clone)]
pub struct MixtureComponent<M: Mixable> {
    model: M,
    y: f64,
    w_kg_per_kmol: f64,
}

impl<M: Mixable> MixtureComponent<M> {
    /// A pure species: `Y = 1`. `w_kg_per_kmol` must be the molecular weight
    /// `model` was built with.
    pub fn new(model: M, w_kg_per_kmol: f64) -> Self {
        Self { model, y: 1.0, w_kg_per_kmol }
    }

    /// Upstream `operator*(s, st)`: `Y = s * Y`, `W` unchanged.
    pub fn scaled(&self, s: f64) -> Self {
        Self { model: self.model.clone(), y: s * self.y, w_kg_per_kmol: self.w_kg_per_kmol }
    }

    /// Upstream `operator+=`: `W = sumY/(Y/W + Y2/W2)`, `Y = sumY`, then every
    /// layer mixes its parameters with `Y1/Y` and `Y2/Y`.
    pub fn add_assign(&mut self, other: &Self) {
        let y1_old = self.y;
        let sum_y = self.y + other.y;
        if sum_y.abs() > SMALL {
            self.w_kg_per_kmol =
                sum_y / (self.y / self.w_kg_per_kmol + other.y / other.w_kg_per_kmol);
        }
        self.y = sum_y;
        self.model.set_mol_weight(molar_mass_from_kg_per_kmol(self.w_kg_per_kmol));
        if self.y.abs() > SMALL {
            let y1 = y1_old / self.y;
            let y2 = other.y / self.y;
            self.model.mix_params(y1, y2, &other.model);
        }
    }

    /// The mixed model.
    pub fn model(&self) -> &M {
        &self.model
    }

    /// The accumulated mass fraction `Y`.
    pub fn y(&self) -> f64 {
        self.y
    }

    /// The molecular weight `W` in kg/kmol.
    pub fn w_kg_per_kmol(&self) -> f64 {
        self.w_kg_per_kmol
    }
}

/// `multiComponentMixture::cellMixture`: `mixture = Y0*s0; mixture += Yn*sn`.
/// `species[i]` carries the species model and `y[i]` its mass fraction in the cell.
pub fn cell_mixture<M: Mixable>(species: &[MixtureComponent<M>], y: &[f64]) -> MixtureComponent<M> {
    let mut mixture = species[0].scaled(y[0]);
    for n in 1..y.len() {
        mixture.add_assign(&species[n].scaled(y[n]));
    }
    mixture
}

/// `multiComponentMixture::cellVolMixture`: weights are the volume fractions
/// `Y_i/rho_i/rhoInv`, with `rho_i = rho_i(p, T)` of the pure species and
/// `rhoInv = sum_i Y_i/rho_i` summed in order.
pub fn cell_vol_mixture<M: Mixable>(
    species: &[MixtureComponent<M>],
    y: &[f64],
    rho: &[f64],
) -> MixtureComponent<M> {
    let mut rho_inv = 0.0;
    for i in 0..y.len() {
        rho_inv += y[i] / rho[i];
    }
    let mut mixture = species[0].scaled(y[0] / rho[0] / rho_inv);
    for n in 1..y.len() {
        mixture.add_assign(&species[n].scaled(y[n] / rho[n] / rho_inv));
    }
    mixture
}

/// Outcome of [`correct_mass_fractions`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MassFractionCheck {
    /// The sum `Yt` of the input fractions, summed in order from `1.0*Y0`.
    pub sum: f64,
    /// `|Yt| - 1`; upstream warns when it exceeds `ROOTVSMALL`.
    pub excess: f64,
    /// Whether upstream would print its "different from one" warning.
    pub warns: bool,
}

/// `multiComponentMixture::correctMassFractions` for one cell: `Y_n /= Yt`.
/// Returns an error string where upstream is fatal (`|Yt| < ROOTVSMALL`).
pub fn correct_mass_fractions(y: &mut [f64]) -> Result<MassFractionCheck, String> {
    let mut yt = 1.0 * y[0];
    for v in y.iter().skip(1) {
        yt += *v;
    }
    if yt.abs() < ROOT_VSMALL {
        return Err(format!("Sum of mass fractions is zero ({yt})"));
    }
    let diff = yt.abs() - 1.0;
    for v in y.iter_mut() {
        *v /= yt;
    }
    Ok(MassFractionCheck { sum: yt, excess: diff, warns: diff > ROOT_VSMALL })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thermophysics::eos::{EquationOfState, PerfectGas, RhoConst};
    use crate::thermophysics::thermo::{HConstThermo, ThermoModel};
    use crate::thermophysics::transport::{ConstTransport, SutherlandTransport, TransportModel};

    type Gas = ConstTransport<HConstThermo<PerfectGas>>;

    fn gas(w_kmol: f64, cp: f64, hf: f64, mu: f64, pr: f64) -> MixtureComponent<Gas> {
        let eos = PerfectGas::new(molar_mass_from_kg_per_kmol(w_kmol));
        let th = HConstThermo::new(
            eos,
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(cp),
            AvailableEnergy::new::<joule_per_kilogram>(hf),
            ThermodynamicTemperature::new::<kelvin>(298.15),
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
        );
        MixtureComponent::new(
            ConstTransport::new(th, DynamicViscosity::new::<pascal_second>(mu), Ratio::new::<ratio>(pr)),
            w_kmol,
        )
    }

    #[test]
    fn harmonic_molecular_weight_and_linear_cp() {
        let a = gas(28.0, 1040.0, 0.0, 1.8e-5, 0.7);
        let b = gas(44.0, 850.0, -9.0e6, 1.5e-5, 0.8);
        let m = cell_mixture(&[a, b], &[0.25, 0.75]);
        // 1/W = sum Y_i/W_i
        let w = 1.0 / (0.25 / 28.0 + 0.75 / 44.0);
        assert!((m.w_kg_per_kmol() - w).abs() < 1e-12);
        assert!((m.y() - 1.0).abs() < 1e-15);
        let p = Pressure::new::<pascal>(1.0e5);
        let t = ThermodynamicTemperature::new::<kelvin>(500.0);
        let mm = m.model();
        let cp = mm.cp(p, t).get::<joule_per_kilogram_kelvin>();
        assert!((cp - (0.25 * 1040.0 + 0.75 * 850.0)).abs() < 1e-9);
        assert!((mm.hc().get::<joule_per_kilogram>() - 0.75 * -9.0e6).abs() < 1e-6);
        // the mixture gas constant uses the mixture W
        let r = mm.r().get::<joule_per_kilogram_kelvin>();
        assert!((r - 8314.462618153240 / w).abs() / r < 1e-12);
        // 1/Pr mixes harmonically: kappa = Cp mu rPr with mixed mu and rPr
        let mu = mm.mu(p, t).get::<pascal_second>();
        assert!((mu - (0.25 * 1.8e-5 + 0.75 * 1.5e-5)).abs() < 1e-18);
        assert!(mm.kappa(p, t).get::<watt_per_meter_kelvin>() > 0.0);
        let _ = mm.alpha_h(p, t);
    }

    #[test]
    fn mass_fraction_normalisation_and_volume_weights() {
        // upstream warns only when the sum EXCEEDS one (diff = |Yt| - 1 > ROOTVSMALL)
        let mut y = [0.2, 0.2, 0.4];
        let c = correct_mass_fractions(&mut y).unwrap();
        assert!((c.sum - 0.8).abs() < 1e-15 && !c.warns);
        let mut hi = [0.5, 0.5, 0.2];
        assert!(correct_mass_fractions(&mut hi).unwrap().warns);
        assert!((y.iter().sum::<f64>() - 1.0).abs() < 1e-15);
        assert!(correct_mass_fractions(&mut [0.0, 0.0]).is_err());
        let mut ok = [0.5, 0.5];
        assert!(!correct_mass_fractions(&mut ok).unwrap().warns);

        let a = gas(28.0, 1040.0, 0.0, 1.8e-5, 0.7);
        let b = gas(44.0, 850.0, 0.0, 1.5e-5, 0.8);
        let rho = [1.0, 3.0];
        let v = cell_vol_mixture(&[a.clone(), b.clone()], &[0.5, 0.5], &rho);
        // volume fractions: (0.5/1)/(0.5/1+0.5/3) = 0.75 and 0.25
        let cp = v.model().cp(Pressure::new::<pascal>(1e5), ThermodynamicTemperature::new::<kelvin>(300.0));
        assert!((cp.get::<joule_per_kilogram_kelvin>() - (0.75 * 1040.0 + 0.25 * 850.0)).abs() < 1e-9);
        assert!((v.y() - 1.0).abs() < 1e-14);
    }

    #[test]
    fn other_mixable_layers_are_reached() {
        let mk = |w: f64, rho: f64, a_s: f64, ts: f64| {
            let eos = RhoConst::new(molar_mass_from_kg_per_kmol(w), MassDensity::new::<kilogram_per_cubic_meter>(rho));
            let th = HConstThermo::new(
                eos,
                SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(4000.0),
                AvailableEnergy::new::<joule_per_kilogram>(0.0),
                ThermodynamicTemperature::new::<kelvin>(298.15),
                AvailableEnergy::new::<joule_per_kilogram>(0.0),
            );
            MixtureComponent::new(SutherlandTransport::new(th, a_s, ts), w)
        };
        let m = cell_mixture(&[mk(18.0, 1000.0, 1.0e-6, 100.0), mk(60.0, 800.0, 3.0e-6, 140.0)], &[0.5, 0.5]);
        let rho = m.model().rho(Pressure::new::<pascal>(1e5), ThermodynamicTemperature::new::<kelvin>(300.0));
        assert!((rho.get::<kilogram_per_cubic_meter>() - 900.0).abs() < 1e-9);
        let mu = m.model().mu(Pressure::new::<pascal>(1e5), ThermodynamicTemperature::new::<kelvin>(300.0));
        let want = 2.0e-6 * 300f64.sqrt() / (1.0 + 120.0 / 300.0);
        assert!((mu.get::<pascal_second>() - want).abs() / want < 1e-12);
    }
}
