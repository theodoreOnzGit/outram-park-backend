use std::sync::Arc;

use crate::fields::field::Field;
use crate::fields::vol_field::VolScalarField;
use crate::mesh::fv_mesh::FvMesh;
use crate::thermophysics::imports::*;
use crate::thermophysics::transport::TransportModel;
use super::traits::FluidThermo;

/// Compressible thermo using ψ-based density: `ρ = ψ · p`.
///
/// This is the `psiThermo` closure used by **sonicFoam** and the transonic
/// branch of **rhoPimpleFoam**.  Storing ψ rather than recomputing it each
/// step lets the pressure equation access ψ directly without a thermo call.
///
/// `M` is any `TransportModel` (which supers `ThermoModel` and `EquationOfState`).
pub struct PsiThermo<M: TransportModel> {
    /// Per-species transport/thermo/EOS kernel (mesh-independent).
    pub species: M,
    pub p:   VolScalarField,
    pub t:   VolScalarField,
    /// Sensible enthalpy `hs` [J/kg].
    pub he:  VolScalarField,
    pub rho: VolScalarField,
    pub psi: VolScalarField,
}

impl<M: TransportModel> PsiThermo<M> {
    /// Construct a thermodynamically consistent initial state.
    ///
    /// All fields are uniform; `correct()` should be called whenever `he` or
    /// `p` are updated to keep `T`, `ρ`, and `ψ` in sync.
    pub fn new(species: M, mesh: Arc<FvMesh>, p_init: f64, t_init: f64) -> Self {
        let p_val = Pressure::new::<pascal>(p_init);
        let t_val = ThermodynamicTemperature::new::<kelvin>(t_init);

        let hs = species.hs(p_val, t_val).get::<joule_per_kilogram>();
        let rho = species.rho(p_val, t_val).get::<kilogram_per_cubic_meter>();
        let psi = rho / p_init;  // ψ = ρ/p  [s²/m²]

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
            let p_c  = Pressure::new::<pascal>(self.p.internal[c]);
            let t_old = ThermodynamicTemperature::new::<kelvin>(self.t.internal[c]);
            let he_c = AvailableEnergy::new::<joule_per_kilogram>(self.he.internal[c]);

            let t_c = self.species.t_from_hs(he_c, p_c, t_old);
            self.t.internal[c] = t_c.get::<kelvin>();

            let rho_c = self.species.rho(p_c, t_c).get::<kilogram_per_cubic_meter>();
            self.rho.internal[c] = rho_c;
            self.psi.internal[c] = rho_c / self.p.internal[c];
        }
    }

    fn correct_boundaries(&mut self) {
        let mesh = self.p.mesh.clone();
        for (pi, patch) in mesh.patches.iter().enumerate() {
            for fi in 0..patch.size {
                let owner = mesh.owner[patch.start + fi];
                // Zero-gradient propagation: boundary ≈ owner cell
                self.t.boundary[pi].values[fi]   = self.t.internal[owner];
                self.rho.boundary[pi].values[fi]  = self.rho.internal[owner];
                self.psi.boundary[pi].values[fi]  = self.psi.internal[owner];
                // Update he boundary from T boundary
                let p_f = self.p.boundary[pi].values[fi];
                let t_f = self.t.boundary[pi].values[fi];
                let p_v = Pressure::new::<pascal>(p_f);
                let t_v = ThermodynamicTemperature::new::<kelvin>(t_f);
                self.he.boundary[pi].values[fi] =
                    self.species.hs(p_v, t_v).get::<joule_per_kilogram>();
            }
        }
    }
}

impl<M: TransportModel> FluidThermo for PsiThermo<M> {
    fn mesh(&self) -> &Arc<FvMesh> { &self.p.mesh }
    fn p(&self)   -> &VolScalarField { &self.p }
    fn p_mut(&mut self) -> &mut VolScalarField { &mut self.p }
    fn t(&self)   -> &VolScalarField { &self.t }
    fn rho(&self) -> &VolScalarField { &self.rho }
    fn he(&self)  -> &VolScalarField { &self.he }
    fn he_mut(&mut self) -> &mut VolScalarField { &mut self.he }
    fn psi(&self) -> &VolScalarField { &self.psi }

    fn mu(&self) -> VolScalarField {
        let mesh = self.p.mesh.clone();
        let n = mesh.n_cells;
        let internal = Field::from_fn(n, |c| {
            let p_c = Pressure::new::<pascal>(self.p.internal[c]);
            let t_c = ThermodynamicTemperature::new::<kelvin>(self.t.internal[c]);
            self.species.mu(p_c, t_c).get::<pascal_second>()
        });
        let boundary = mesh.patches.iter().enumerate().map(|(pi, patch)| {
            let values = Field::from_fn(patch.size, |fi| {
                let p_f = Pressure::new::<pascal>(self.p.boundary[pi].values[fi]);
                let t_f = ThermodynamicTemperature::new::<kelvin>(self.t.boundary[pi].values[fi]);
                self.species.mu(p_f, t_f).get::<pascal_second>()
            });
            crate::fields::boundary::bc::PatchField {
                bc: crate::fields::boundary::bc::BoundaryCondition::ZeroGradient,
                values,
            }
        }).collect();
        VolScalarField::new("mu", mesh, internal, boundary)
    }

    fn kappa(&self) -> VolScalarField {
        let mesh = self.p.mesh.clone();
        let n = mesh.n_cells;
        let internal = Field::from_fn(n, |c| {
            let p_c = Pressure::new::<pascal>(self.p.internal[c]);
            let t_c = ThermodynamicTemperature::new::<kelvin>(self.t.internal[c]);
            self.species.kappa(p_c, t_c).get::<watt_per_meter_kelvin>()
        });
        let boundary = mesh.patches.iter().enumerate().map(|(pi, patch)| {
            let values = Field::from_fn(patch.size, |fi| {
                let p_f = Pressure::new::<pascal>(self.p.boundary[pi].values[fi]);
                let t_f = ThermodynamicTemperature::new::<kelvin>(self.t.boundary[pi].values[fi]);
                self.species.kappa(p_f, t_f).get::<watt_per_meter_kelvin>()
            });
            crate::fields::boundary::bc::PatchField {
                bc: crate::fields::boundary::bc::BoundaryCondition::ZeroGradient,
                values,
            }
        }).collect();
        VolScalarField::new("kappa", mesh, internal, boundary)
    }

    fn alpha_h(&self) -> VolScalarField {
        let mesh = self.p.mesh.clone();
        let n = mesh.n_cells;
        let internal = Field::from_fn(n, |c| {
            let p_c = Pressure::new::<pascal>(self.p.internal[c]);
            let t_c = ThermodynamicTemperature::new::<kelvin>(self.t.internal[c]);
            self.species.alpha_h(p_c, t_c).get::<pascal_second>()
        });
        let boundary = mesh.patches.iter().enumerate().map(|(pi, patch)| {
            let values = Field::from_fn(patch.size, |fi| {
                let p_f = Pressure::new::<pascal>(self.p.boundary[pi].values[fi]);
                let t_f = ThermodynamicTemperature::new::<kelvin>(self.t.boundary[pi].values[fi]);
                self.species.alpha_h(p_f, t_f).get::<pascal_second>()
            });
            crate::fields::boundary::bc::PatchField {
                bc: crate::fields::boundary::bc::BoundaryCondition::ZeroGradient,
                values,
            }
        }).collect();
        VolScalarField::new("alpha", mesh, internal, boundary)
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
        let thermo = PsiThermo::new(air_thermo(), m, 101325.0, 300.0);
        // ρ ≈ 1.176 kg/m³, psi = rho/p
        assert_relative_eq!(thermo.rho.internal[0], 101325.0 / (287.05 * 300.0), epsilon = 1.0);
        let psi_expected = thermo.rho.internal[0] / 101325.0;
        assert_relative_eq!(thermo.psi.internal[0], psi_expected, epsilon = 1e-10);
    }

    #[test]
    fn correct_recovers_temperature_from_he() {
        let m = tiny_mesh();
        let mut thermo = PsiThermo::new(air_thermo(), m, 101325.0, 300.0);
        // Manually set he to correspond to T=400 K.
        // HConstThermo: hs(p,T) = Cp*(T - Tref) + Hsref; Tref=298.15, Hsref=0
        let he_400 = 1004.5 * (400.0 - 298.15);
        thermo.he.internal[0] = he_400;
        thermo.he.internal[1] = he_400;
        thermo.correct();
        assert_relative_eq!(thermo.t.internal[0], 400.0, epsilon = 0.5);
        assert_relative_eq!(thermo.t.internal[1], 400.0, epsilon = 0.5);
    }

    #[test]
    fn mu_and_kappa_fields_populated() {
        let m = tiny_mesh();
        let thermo = PsiThermo::new(air_thermo(), m, 101325.0, 300.0);
        let mu = thermo.mu();
        let kappa = thermo.kappa();
        assert_relative_eq!(mu.internal[0], 1.81e-5, epsilon = 1e-8);
        assert!(kappa.internal[0] > 0.0);
    }

    #[test]
    fn correct_rho_clamps() {
        let m = tiny_mesh();
        let mut thermo = PsiThermo::new(air_thermo(), m.clone(), 101325.0, 300.0);
        let delta = VolScalarField::uniform("drho", m, -5.0);
        let rho_before = thermo.rho.internal[0];
        thermo.correct_rho(&delta, rho_before - 1.0, rho_before + 1.0);
        // After clamp: rho should not go below (rho_before - 1.0)
        assert!(thermo.rho.internal[0] >= rho_before - 1.0 - 1e-12);
    }
}
