// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/neutronics/pointKinetics/
//                    (pointKineticNeutronics.C — the reactivity-feedback
//                     assembly: Doppler / fuel- and coolant-temperature /
//                     density / expansion coefficients integrated over the
//                     feedback fields) and neutronics/include/
//                     updateReactivity.H
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal authors: Carlo Fiorina, Konstantin Mikityuk (EPFL)
//   Upstream license: GPL-3.0
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
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! # Reactivity-feedback layer (temperature / density → reactivity)
//!
//! Rust port of the reactivity-feedback assembly GeN-Foam's `pointKinetics`
//! neutronics performs each step: it turns the thermal-hydraulic / mechanical
//! **feedback fields** on the mesh (fuel temperature, structure temperature,
//! coolant density, axial expansion) into a single scalar reactivity `Δρ` that
//! perturbs the neutronics. This is the layer that closes the
//! neutronics ↔ thermal-hydraulics loop.
//!
//! It generalises the 0-D Doppler feedback already demonstrated in
//! [`super::outer_iteration`] (`ρ = ρ_ext − α (T_fuel − T_ref)`, a single
//! lumped temperature) to **spatial mesh fields**: each feedback term
//! contributes a *local* reactivity density per cell, and the total `Δρ` is the
//! importance-weighted volume average of those local contributions (first-order
//! perturbation theory: the reactivity worth of a local perturbation is weighted
//! by the local neutron importance).
//!
//! ## The feedback sum
//!
//! For a set of feedback terms `t` (Doppler, expansion, coolant density), with a
//! per-cell importance weight `w_c` (cell volume, optionally times a supplied
//! worth field such as `phi^2`):
//!
//! ```text
//!   Δρ = ( Σ_c w_c Σ_t contribution_t(field_t[c]) ) / ( Σ_c w_c )
//! ```
//!
//! Each term's contribution is written so a **positive** coefficient is the
//! physically stabilising (negative-feedback) case, matching
//! [`super::outer_iteration::LumpedNeutronics`]:
//!
//! - [`FeedbackTerm::Doppler`] — `−α (T − T_ref)` (linear) or
//!   `−α T_ref ln(T/T_ref)` (logarithmic, the classic Doppler broadening law;
//!   equal to the linear form to first order in `ΔT`).
//! - [`FeedbackTerm::Expansion`] — `−α (T_struct − T_ref)` (axial fuel / clad
//!   expansion, temperature-driven).
//! - [`FeedbackTerm::Density`] — `−α (ρ − ρ_ref)/ρ_ref` (coolant density / void;
//!   coefficient is reactivity per unit *fractional* density change).
//!
//! ## Reduction to the lumped case (V&V anchor)
//!
//! For a **uniform** field and a volume weight, the importance-weighted average
//! collapses to the single-term lumped expression, so a mesh with a spatially
//! constant `T_fuel` reproduces `Δρ = −α (T_fuel − T_ref)` exactly — the bridge
//! between this layer and the verified 0-D path.
//!
//! ## Units
//!
//! Coefficients and references cross the API as named `uom` aliases
//! ([`FeedbackCoefficient`] = 1/K, [`ThermodynamicTemperature`],
//! [`MassDensity`]); the returned [`Reactivity`] is `uom`'s dimensionless
//! [`Ratio`]. The per-cell reduction is done in raw `f64` internally (never at
//! the API boundary).

use uom::si::f64::{MassDensity, Ratio, ThermodynamicTemperature};
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::ratio::ratio;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

use super::coupling_fields::CouplingRegion;
use super::outer_iteration::{CouplingLoopError, FeedbackCoefficient};
use crate::genfoam::neutronics::point_kinetics::Reactivity;

/// The temperature dependence of the Doppler reactivity term.
///
/// Both laws use the same [`FeedbackCoefficient`] `α` (1/K) and agree to first
/// order in `ΔT`; they differ only in how the term grows for large excursions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DopplerLaw {
    /// `−α (T − T_ref)` — linear in the temperature rise.
    Linear,
    /// `−α T_ref ln(T/T_ref)` — the logarithmic Doppler-broadening law (the
    /// resonance-integral `A + B ln T` form), scaled by `T_ref` so `α` keeps its
    /// 1/K units and the two laws coincide as `T → T_ref`.
    Logarithmic,
}

/// Per-cell importance weight used to average local reactivity contributions.
///
/// First-order perturbation theory weights a local perturbation's reactivity
/// worth by the local neutron importance (`phi ϕ†`); with the fundamental-mode
/// approximation `ϕ† ∝ ϕ` that is `phi^2`. Absent an adjoint solve, a plain
/// volume weight is the honest default (every cell equally important).
#[derive(Debug, Clone)]
pub enum ImportanceWeight {
    /// Weight each cell by its volume `V_c` only (uniform importance).
    Volume,
    /// Weight each cell by `V_c · f_c`, where `f_c` is the named scalar field
    /// (e.g. a one-group flux or `phi^2` worth field) read from the region.
    Field(String),
}

/// One additive feedback contribution (enum dispatch, no `dyn`).
///
/// Each variant names the region scalar field it reads, its coefficient, and its
/// reference point. `contribution` returns the **local** reactivity `Δρ_c` (a
/// dimensionless `f64`) for the field value in one cell; a positive coefficient
/// gives negative (stabilising) feedback under a positive deviation.
#[derive(Debug, Clone)]
pub enum FeedbackTerm {
    /// Fuel-temperature (Doppler) feedback, `−α (T − T_ref)` or the log law.
    Doppler {
        /// Name of the fuel-temperature field (K) in the region.
        field: String,
        /// Feedback coefficient `α` (1/K); positive ⇒ stabilising.
        coeff: FeedbackCoefficient,
        /// Reference temperature `T_ref` (K).
        t_ref: ThermodynamicTemperature,
        /// Temperature dependence.
        law: DopplerLaw,
    },
    /// Structure-temperature / thermal-expansion feedback, `−α (T − T_ref)`.
    Expansion {
        /// Name of the structure-temperature field (K) in the region.
        field: String,
        /// Feedback coefficient `α` (1/K); positive ⇒ stabilising.
        coeff: FeedbackCoefficient,
        /// Reference temperature `T_ref` (K).
        t_ref: ThermodynamicTemperature,
    },
    /// Coolant-density / void feedback, `−α (ρ − ρ_ref)/ρ_ref` (coefficient is
    /// reactivity per unit fractional density change).
    Density {
        /// Name of the coolant-density field (kg/m³) in the region.
        field: String,
        /// Reactivity per unit fractional density change (dimensionless
        /// `Ratio`). Positive ⇒ losing coolant (density drop) adds positive
        /// reactivity is the *destabilising* case, so a positive coefficient
        /// here means density *rise* subtracts reactivity.
        coeff: Ratio,
        /// Reference density `ρ_ref` (kg/m³).
        rho_ref: MassDensity,
    },
}

impl FeedbackTerm {
    /// The name of the region field this term reads.
    #[must_use]
    pub fn field_name(&self) -> &str {
        match self {
            FeedbackTerm::Doppler { field, .. }
            | FeedbackTerm::Expansion { field, .. }
            | FeedbackTerm::Density { field, .. } => field,
        }
    }

    /// Local reactivity contribution `Δρ_c` for the field value `x` in one cell.
    fn contribution(&self, x: f64) -> f64 {
        match self {
            FeedbackTerm::Doppler {
                coeff, t_ref, law, ..
            } => {
                let a = coeff.get::<per_kelvin>();
                let tr = t_ref.get::<kelvin>();
                match law {
                    DopplerLaw::Linear => -a * (x - tr),
                    // −α T_ref ln(T/T_ref); guard non-positive T (extrapolation).
                    DopplerLaw::Logarithmic => {
                        if x > 0.0 {
                            -a * tr * (x / tr).ln()
                        } else {
                            -a * (x - tr)
                        }
                    }
                }
            }
            FeedbackTerm::Expansion { coeff, t_ref, .. } => {
                -coeff.get::<per_kelvin>() * (x - t_ref.get::<kelvin>())
            }
            FeedbackTerm::Density { coeff, rho_ref, .. } => {
                let rr = rho_ref.get::<kilogram_per_cubic_meter>();
                -coeff.get::<ratio>() * (x - rr) / rr
            }
        }
    }
}

/// The reactivity-feedback assembly: a set of [`FeedbackTerm`]s and the
/// importance weighting used to reduce their per-cell contributions to one
/// scalar `Δρ`.
///
/// Build with [`Self::new`] (empty, volume-weighted) and add terms with the
/// `with_*` builders, then call [`Self::reactivity`] against a
/// [`CouplingRegion`] carrying the feedback fields.
#[derive(Debug, Clone)]
pub struct ReactivityFeedback {
    terms: Vec<FeedbackTerm>,
    weight: ImportanceWeight,
}

impl Default for ReactivityFeedback {
    fn default() -> Self {
        Self::new()
    }
}

impl ReactivityFeedback {
    /// A feedback assembly with no terms and a volume importance weight.
    #[must_use]
    pub fn new() -> Self {
        Self {
            terms: Vec::new(),
            weight: ImportanceWeight::Volume,
        }
    }

    /// Set the per-cell importance weight (default [`ImportanceWeight::Volume`]).
    #[must_use]
    pub fn with_weight(mut self, weight: ImportanceWeight) -> Self {
        self.weight = weight;
        self
    }

    /// Add a Doppler (fuel-temperature) term.
    #[must_use]
    pub fn with_doppler(
        mut self,
        field: &str,
        coeff: FeedbackCoefficient,
        t_ref: ThermodynamicTemperature,
        law: DopplerLaw,
    ) -> Self {
        self.terms.push(FeedbackTerm::Doppler {
            field: field.to_string(),
            coeff,
            t_ref,
            law,
        });
        self
    }

    /// Add a structure-temperature / expansion term.
    #[must_use]
    pub fn with_expansion(
        mut self,
        field: &str,
        coeff: FeedbackCoefficient,
        t_ref: ThermodynamicTemperature,
    ) -> Self {
        self.terms.push(FeedbackTerm::Expansion {
            field: field.to_string(),
            coeff,
            t_ref,
        });
        self
    }

    /// Add a coolant-density / void term.
    #[must_use]
    pub fn with_density(mut self, field: &str, coeff: Ratio, rho_ref: MassDensity) -> Self {
        self.terms.push(FeedbackTerm::Density {
            field: field.to_string(),
            coeff,
            rho_ref,
        });
        self
    }

    /// Borrow the configured terms (for diagnostics / dispatch checks).
    #[must_use]
    pub fn terms(&self) -> &[FeedbackTerm] {
        &self.terms
    }

    /// The total feedback reactivity `Δρ` for the feedback fields on `region`.
    ///
    /// Reads each term's field from the region, forms the per-cell local
    /// reactivity, and returns the importance-weighted volume average (see the
    /// module docs). With no terms it returns zero reactivity.
    ///
    /// # Errors
    ///
    /// [`CouplingLoopError::MissingField`] if any term's field, or the weight
    /// field, is absent from the region.
    pub fn reactivity(&self, region: &CouplingRegion) -> Result<Reactivity, CouplingLoopError> {
        if self.terms.is_empty() {
            return Ok(Reactivity::new::<ratio>(0.0));
        }
        let n = region.mesh().n_cells;
        let vols = &region.mesh().cell_volumes;

        // Resolve the per-cell weight field (V_c or V_c·f_c) once.
        let weight_field: Option<Vec<f64>> = match &self.weight {
            ImportanceWeight::Volume => None,
            ImportanceWeight::Field(name) => {
                let f = region
                    .scalar(name)
                    .ok_or_else(|| CouplingLoopError::MissingField {
                        region: region.name().to_string(),
                        field: name.clone(),
                    })?;
                Some(f.internal.as_slice().to_vec())
            }
        };

        // Gather each term's field slice up front (one lookup per term).
        let mut term_fields: Vec<&[f64]> = Vec::with_capacity(self.terms.len());
        for t in &self.terms {
            let f =
                region
                    .scalar(t.field_name())
                    .ok_or_else(|| CouplingLoopError::MissingField {
                        region: region.name().to_string(),
                        field: t.field_name().to_string(),
                    })?;
            term_fields.push(f.internal.as_slice());
        }

        let mut num = 0.0_f64;
        let mut den = 0.0_f64;
        for c in 0..n {
            let w = match &weight_field {
                None => vols[c],
                Some(wf) => vols[c] * wf[c],
            };
            let mut local = 0.0_f64;
            for (t, fld) in self.terms.iter().zip(term_fields.iter()) {
                local += t.contribution(fld[c]);
            }
            num += w * local;
            den += w;
        }
        let rho = if den.abs() > 0.0 { num / den } else { 0.0 };
        Ok(Reactivity::new::<ratio>(rho))
    }

    /// Importance-weighted mean of a named scalar field over the region — the
    /// aggregate `\bar{x} = (Σ_c w_c x_c) / (Σ_c w_c)`.
    ///
    /// This is the scalar feedback point a *mesh* neutronics model collapses a
    /// spatial temperature field to when it materialises its cross sections at a
    /// single feedback value (see [`super::mesh_region::MeshNeutronics`]); it
    /// uses the same importance weighting as [`Self::reactivity`], so both stay
    /// consistent.
    ///
    /// # Errors
    ///
    /// [`CouplingLoopError::MissingField`] if `field` or the weight field is
    /// absent from the region.
    pub fn weighted_mean(
        &self,
        region: &CouplingRegion,
        field: &str,
    ) -> Result<f64, CouplingLoopError> {
        let n = region.mesh().n_cells;
        let vols = &region.mesh().cell_volumes;
        let x = region
            .scalar(field)
            .ok_or_else(|| CouplingLoopError::MissingField {
                region: region.name().to_string(),
                field: field.to_string(),
            })?
            .internal
            .as_slice()
            .to_vec();
        let weight_field: Option<Vec<f64>> = match &self.weight {
            ImportanceWeight::Volume => None,
            ImportanceWeight::Field(name) => Some(
                region
                    .scalar(name)
                    .ok_or_else(|| CouplingLoopError::MissingField {
                        region: region.name().to_string(),
                        field: name.clone(),
                    })?
                    .internal
                    .as_slice()
                    .to_vec(),
            ),
        };
        let mut num = 0.0_f64;
        let mut den = 0.0_f64;
        for c in 0..n {
            let w = match &weight_field {
                None => vols[c],
                Some(wf) => vols[c] * wf[c],
            };
            num += w * x[c];
            den += w;
        }
        Ok(if den.abs() > 0.0 { num / den } else { 0.0 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use outram_foam_basic_lib::fields::vol_field::VolScalarField;
    use outram_foam_basic_lib::interface::one_dimensional_meshing::create_one_d_mesh;
    use outram_foam_basic_lib::mesh::fv_mesh::FvMesh;
    use uom::si::area::square_meter;
    use uom::si::f64::{Area, Length};
    use uom::si::length::meter;

    fn slab(n: i64) -> Arc<FvMesh> {
        Arc::new(
            create_one_d_mesh(Length::new::<meter>(1.0), Area::new::<square_meter>(1.0), n)
                .unwrap(),
        )
    }

    fn region_with_tfuel(mesh: Arc<FvMesh>, values: &[f64]) -> CouplingRegion {
        let mut r = CouplingRegion::new("core", mesh.clone());
        let f = VolScalarField::uniform("TFuel", mesh, 0.0).with_internal(values.to_vec());
        r.insert_scalar(f);
        r
    }

    // Small helper mirroring the diffusion `WithInternal` trait locally.
    trait WithInternal {
        fn with_internal(self, v: Vec<f64>) -> Self;
    }
    impl WithInternal for VolScalarField {
        fn with_internal(mut self, v: Vec<f64>) -> Self {
            self.internal = outram_foam_basic_lib::prelude::Field::new(v);
            self
        }
    }

    /// V&V — the mesh feedback reduces to the lumped `−α (T − T_ref)` for a
    /// uniform field.
    ///
    /// Methodology: a 5-cell slab at a spatially uniform `T_fuel = 1000 K`, a
    /// single linear Doppler term with `α = 3 pcm/K` and `T_ref = 900 K`. The
    /// importance-weighted average must equal the lumped value
    /// `−3e-5 · (1000 − 900) = −3.0e-3` (−300 pcm) regardless of the (uniform)
    /// weighting.
    ///
    /// Result (2026-07-15): Δρ = −3.000000e-3 (−300.0 pcm), |err| < 1e-15. Pass —
    /// the mesh layer is a strict generalisation of the 0-D Doppler feedback.
    #[test]
    fn uniform_field_reduces_to_lumped() {
        let mesh = slab(5);
        let r = region_with_tfuel(mesh, &[1000.0; 5]);
        let fb = ReactivityFeedback::new().with_doppler(
            "TFuel",
            FeedbackCoefficient::new::<per_kelvin>(3.0e-5),
            ThermodynamicTemperature::new::<kelvin>(900.0),
            DopplerLaw::Linear,
        );
        let rho = fb.reactivity(&r).unwrap().get::<ratio>();
        assert!((rho - (-3.0e-3)).abs() < 1e-15, "Δρ = {rho}");
    }

    /// V&V — negative (stabilising) Doppler feedback under heating.
    ///
    /// Methodology: a linearly-varying `T_fuel` profile from 950 K to 1150 K
    /// (mean 1050 K on a uniform mesh) with `α = 2 pcm/K`, `T_ref = 900 K`. The
    /// volume-averaged rise is `150 K`, so `Δρ = −2e-5·150 = −3.0e-3`.
    ///
    /// Result (2026-07-15): Δρ = −3.000000e-3, and Δρ < 0 (opposes the heating).
    /// Pass.
    #[test]
    fn spatial_profile_negative_feedback() {
        let mesh = slab(5);
        // cell-centre temperatures, mean = 1050 K
        let t = [950.0, 1000.0, 1050.0, 1100.0, 1150.0];
        let r = region_with_tfuel(mesh, &t);
        let fb = ReactivityFeedback::new().with_doppler(
            "TFuel",
            FeedbackCoefficient::new::<per_kelvin>(2.0e-5),
            ThermodynamicTemperature::new::<kelvin>(900.0),
            DopplerLaw::Linear,
        );
        let rho = fb.reactivity(&r).unwrap().get::<ratio>();
        assert!(rho < 0.0, "heating must subtract reactivity, got {rho}");
        assert!((rho - (-3.0e-3)).abs() < 1e-12, "Δρ = {rho}");
    }

    /// V&V — multiple terms (Doppler + coolant density) superpose linearly and
    /// the log Doppler law matches the linear law to first order.
    ///
    /// Methodology: uniform `T_fuel = 950 K` (ΔT = 50 K), `α_D = 2 pcm/K` and a
    /// coolant density dropping from `ρ_ref = 700` to `630 kg/m³` (a −10 %
    /// fractional change) with density coefficient `0.01` (reactivity per unit
    /// fractional density). Expected: Doppler −1.0e-3, density
    /// `−0.01·(−0.10) = +1.0e-3`, net ≈ 0. The log-law Doppler at ΔT/T ≈ 5 %
    /// must sit within 3 % of the linear term.
    ///
    /// Result (2026-07-15): linear net Δρ = 0.0 exactly (−1.0e-3 + 1.0e-3);
    /// log-law Doppler = −9.732e-4 vs linear −1.0e-3 (2.7 % low, as expected for
    /// `T_ref ln(T/T_ref)` vs `ΔT` at ΔT/T ≈ 5.6 %). Pass.
    #[test]
    fn multiterm_superposition_and_log_law() {
        let mesh = slab(4);
        let mut r = CouplingRegion::new("core", mesh.clone());
        r.insert_scalar(VolScalarField::uniform("TFuel", mesh.clone(), 950.0));
        r.insert_scalar(VolScalarField::uniform("rhoCool", mesh, 630.0));

        let fb = ReactivityFeedback::new()
            .with_doppler(
                "TFuel",
                FeedbackCoefficient::new::<per_kelvin>(2.0e-5),
                ThermodynamicTemperature::new::<kelvin>(900.0),
                DopplerLaw::Linear,
            )
            .with_density(
                "rhoCool",
                Ratio::new::<ratio>(0.01),
                MassDensity::new::<kilogram_per_cubic_meter>(700.0),
            );
        let rho = fb.reactivity(&r).unwrap().get::<ratio>();
        assert!(rho.abs() < 1e-12, "net Δρ should cancel, got {rho}");

        // Log law within a few % of linear for this small excursion.
        let fb_log = ReactivityFeedback::new().with_doppler(
            "TFuel",
            FeedbackCoefficient::new::<per_kelvin>(2.0e-5),
            ThermodynamicTemperature::new::<kelvin>(900.0),
            DopplerLaw::Logarithmic,
        );
        let rho_log = fb_log.reactivity(&r).unwrap().get::<ratio>();
        let rel = (rho_log - (-1.0e-3)).abs() / 1.0e-3;
        assert!(rel < 0.03, "log law {rho_log} too far from linear −1e-3");
    }
}
