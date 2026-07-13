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

use std::sync::Arc;

use crate::fields::field::Field;
use crate::fields::vol_field::VolScalarField;
use crate::mesh::fv_mesh::FvMesh;
use crate::thermophysics::imports::*;
use crate::thermophysics::transport::TransportModel;
use super::traits::FluidThermo;

/// Compressible thermo using explicit EOS density: `ρ = ρ(p, T)`.
///
/// This is the `rhoThermo` closure used by the subsonic branch of
/// **rhoPimpleFoam**.  Density is computed directly from the equation of
/// state, not from ψ·p, so it works for non-ideal gas models (e.g. real-gas
/// EOS or incompressible `RhoConst`).
///
/// `M` is any `TransportModel` (which supers `ThermoModel` and `EquationOfState`).
pub struct RhoThermo<M: TransportModel> {
    /// Per-species transport/thermo/EOS kernel (mesh-independent).
    pub species: M,
    pub p:   VolScalarField,
    pub t:   VolScalarField,
    /// Sensible enthalpy `hs` [J/kg].
    pub he:  VolScalarField,
    pub rho: VolScalarField,
    /// Compressibility ψ = ∂ρ/∂p|_T [s²/m²] — stored for the pressure eqn.
    pub psi: VolScalarField,
}

impl<M: TransportModel> RhoThermo<M> {
    /// Construct a thermodynamically consistent initial state.
    pub fn new(species: M, mesh: Arc<FvMesh>, p_init: f64, t_init: f64) -> Self {
        let p_val = Pressure::new::<pascal>(p_init);
        let t_val = ThermodynamicTemperature::new::<kelvin>(t_init);

        let hs  = species.hs(p_val, t_val).get::<joule_per_kilogram>();
        let rho = species.rho(p_val, t_val).get::<kilogram_per_cubic_meter>();
        let psi = rho / p_init;

        Self {
            p:   VolScalarField::uniform("p",   mesh.clone(), p_init),
            t:   VolScalarField::uniform("T",   mesh.clone(), t_init),
            he:  VolScalarField::uniform("he",  mesh.clone(), hs),
            rho: VolScalarField::uniform("rho", mesh.clone(), rho),
            psi: VolScalarField::uniform("psi", mesh.clone(), psi),
            species,
        }
    }

    fn correct_internal(&mut self) {
        let n = self.p.mesh.n_cells;
        for c in 0..n {
            let p_c   = Pressure::new::<pascal>(self.p.internal[c]);
            let t_old = ThermodynamicTemperature::new::<kelvin>(self.t.internal[c]);
            let he_c  = AvailableEnergy::new::<joule_per_kilogram>(self.he.internal[c]);

            let t_c   = self.species.t_from_hs(he_c, p_c, t_old).unwrap_or(t_old);
            self.t.internal[c] = t_c.get::<kelvin>();

            // Explicit EOS density — not ψ·p
            let rho_c = self.species.rho(p_c, t_c).get::<kilogram_per_cubic_meter>();
            self.rho.internal[c] = rho_c;
            // Still keep ψ for the pressure equation
            self.psi.internal[c] = rho_c / self.p.internal[c];
        }
    }

    fn correct_boundaries(&mut self) {
        let mesh = self.p.mesh.clone();
        for (pi, patch) in mesh.patches.iter().enumerate() {
            for fi in 0..patch.size {
                let owner = mesh.owner[patch.start + fi];
                self.t.boundary[pi].values[fi]  = self.t.internal[owner];
                self.rho.boundary[pi].values[fi] = self.rho.internal[owner];
                self.psi.boundary[pi].values[fi] = self.psi.internal[owner];
                let p_f = Pressure::new::<pascal>(self.p.boundary[pi].values[fi]);
                let t_f = ThermodynamicTemperature::new::<kelvin>(self.t.boundary[pi].values[fi]);
                self.he.boundary[pi].values[fi] =
                    self.species.hs(p_f, t_f).get::<joule_per_kilogram>();
            }
        }
    }

    fn transport_field(&self, name: &str, f: impl Fn(Pressure, ThermodynamicTemperature) -> f64) -> VolScalarField {
        let mesh = self.p.mesh.clone();
        let internal = Field::from_fn(mesh.n_cells, |c| {
            f(
                Pressure::new::<pascal>(self.p.internal[c]),
                ThermodynamicTemperature::new::<kelvin>(self.t.internal[c]),
            )
        });
        let boundary = mesh.patches.iter().enumerate().map(|(pi, patch)| {
            let values = Field::from_fn(patch.size, |fi| {
                f(
                    Pressure::new::<pascal>(self.p.boundary[pi].values[fi]),
                    ThermodynamicTemperature::new::<kelvin>(self.t.boundary[pi].values[fi]),
                )
            });
            crate::fields::boundary::bc::PatchField {
                bc: crate::fields::boundary::bc::BoundaryCondition::ZeroGradient,
                values,
            }
        }).collect();
        VolScalarField::new(name, mesh, internal, boundary)
    }
}

impl<M: TransportModel> FluidThermo for RhoThermo<M> {
    fn mesh(&self) -> &Arc<FvMesh> { &self.p.mesh }
    fn p(&self)   -> &VolScalarField { &self.p }
    fn p_mut(&mut self) -> &mut VolScalarField { &mut self.p }
    fn t(&self)   -> &VolScalarField { &self.t }
    fn rho(&self) -> &VolScalarField { &self.rho }
    fn he(&self)  -> &VolScalarField { &self.he }
    fn he_mut(&mut self) -> &mut VolScalarField { &mut self.he }
    fn psi(&self) -> &VolScalarField { &self.psi }

    fn mu(&self) -> VolScalarField {
        self.transport_field("mu", |p, t| self.species.mu(p, t).get::<pascal_second>())
    }

    fn kappa(&self) -> VolScalarField {
        self.transport_field("kappa", |p, t| self.species.kappa(p, t).get::<watt_per_meter_kelvin>())
    }

    fn alpha_h(&self) -> VolScalarField {
        self.transport_field("alpha", |p, t| self.species.alpha_h(p, t).get::<pascal_second>())
    }

    fn correct(&mut self) {
        self.correct_internal();
        self.correct_boundaries();
    }

    fn correct_rho(&mut self, delta_rho: &VolScalarField, rho_min: f64, rho_max: f64) {
        for c in 0..self.rho.mesh.n_cells {
            self.rho.internal[c] =
                (self.rho.internal[c] + delta_rho.internal[c]).clamp(rho_min, rho_max);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::Vector3;
    use crate::mesh::fv_mesh::{FvMeshBuilder, BoundaryPatch, PatchKind};
    use crate::thermophysics::eos::PerfectGas;
    use crate::thermophysics::thermo::HConstThermo;
    use crate::thermophysics::transport::ConstTransport;
    use approx::assert_relative_eq;

    fn air_thermo() -> impl TransportModel {
        let eos = PerfectGas::new(MolarMass::new::<gram_per_mole>(28.97));
        let thermo = HConstThermo::new(
            eos,
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(1004.5),
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
            ThermodynamicTemperature::new::<kelvin>(298.15),
            AvailableEnergy::new::<joule_per_kilogram>(0.0),
        );
        ConstTransport::new(thermo, DynamicViscosity::new::<pascal_second>(1.81e-5), Ratio::new::<ratio>(0.71))
    }

    fn tiny_mesh() -> Arc<FvMesh> {
        Arc::new(FvMeshBuilder::new()
            .n_cells(2).n_internal_faces(1)
            .owner(vec![0, 1, 0]).neighbour(vec![1])
            .patches(vec![
                BoundaryPatch::new("right", 1, 1, PatchKind::Wall),
                BoundaryPatch::new("left",  2, 1, PatchKind::Wall),
            ])
            .cell_volumes(vec![1.0, 1.0])
            .cell_centres(vec![Vector3::new(0.25, 0.0, 0.0), Vector3::new(0.75, 0.0, 0.0)])
            .face_area_vectors(vec![
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(-1.0, 0.0, 0.0),
            ])
            .face_centres(vec![
                Vector3::new(0.5, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(0.0, 0.0, 0.0),
            ])
            .build().unwrap())
    }

    #[test]
    fn initial_state_consistent() {
        let m = tiny_mesh();
        let thermo = RhoThermo::new(air_thermo(), m, 101325.0, 300.0);
        let expected_rho = 101325.0 / (287.05 * 300.0);
        assert_relative_eq!(thermo.rho.internal[0], expected_rho, epsilon = 0.5);
    }

    #[test]
    fn correct_updates_temperature() {
        let m = tiny_mesh();
        let mut thermo = RhoThermo::new(air_thermo(), m, 101325.0, 300.0);
        // HConstThermo: hs(p,T) = Cp*(T - Tref); Tref=298.15
        let he_500 = 1004.5 * (500.0 - 298.15);
        thermo.he.internal[0] = he_500;
        thermo.he.internal[1] = he_500;
        thermo.correct();
        assert_relative_eq!(thermo.t.internal[0], 500.0, epsilon = 1.0);
        // ρ should decrease at higher T (ideal gas: ρ = p/(R·T))
        let rho_cold = 101325.0 / (287.05 * 300.0);
        let rho_hot  = 101325.0 / (287.05 * 500.0);
        assert!(thermo.rho.internal[0] < rho_cold);
        assert_relative_eq!(thermo.rho.internal[0], rho_hot, epsilon = 0.1);
    }

    #[test]
    fn psi_vs_rho_thermo_match() {
        // For a perfect gas, PsiThermo and RhoThermo should give the same ρ
        use crate::fluid_thermo::PsiThermo;
        let m = tiny_mesh();
        let mut psi = PsiThermo::new(air_thermo(), m.clone(), 101325.0, 300.0);
        let mut rho = RhoThermo::new(air_thermo(), m, 101325.0, 300.0);
        psi.correct();
        rho.correct();
        assert_relative_eq!(
            psi.rho.internal[0],
            rho.rho.internal[0],
            epsilon = 1e-8
        );
    }
}
