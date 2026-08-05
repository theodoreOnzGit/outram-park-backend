// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
// Upstream source: src/finiteVolume/cfdTools/general/fvModels/ (`fvModel`,
//   `fvModels`) and src/fvModels/general/ (the derived sources).
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

//! Optional source terms added to finite-volume equations — OpenFOAM's
//! `fvOptions` mechanism.
//!
//! # What this is for
//!
//! A solver assembles a fixed equation — momentum, energy, a transported
//! scalar. Real cases then need *extra* terms in it that the solver itself
//! knows nothing about: a heat source in one region, a porous drag in another,
//! a phase-change latent heat, a momentum sink representing a fan. Editing the
//! solver for each is unworkable, so OpenFOAM lets a case attach source terms
//! to named equations from the outside. That is `fvOptions`.
//!
//! The pattern is the same everywhere it appears:
//!
//! ```text
//! solve( ddt(rho, U) + div(phi, U) - laplacian(mu, U) == fvModels.source(rho, U) );
//! ```
//!
//! The solver names the equation; the case decides what, if anything, is added
//! to it.
//!
//! # A note on the name
//!
//! What ESI OpenFOAM (openfoam.com) calls **`fvOptions`**, the OpenFOAM
//! Foundation (openfoam.org) split into **`fvModels`** — terms that add
//! *sources* to an equation — and **`fvConstraints`** — terms that *constrain*
//! a solution, such as fixing a value in a cell set. This port follows the
//! Foundation split, because the vendored reference tree is the Foundation
//! one, but the module is named `fv_options` because that is the name most
//! users will search for. [`FvModel`] is the source half; constraints are not
//! yet ported.
//!
//! # Why this lives in `outram-foam-basic-lib`
//!
//! It operates directly on [`FvMatrix`](crate::ldu_matrix::FvMatrix), and every
//! solver crate needs it — the multiphase, turbulence and application layers
//! all assemble equations that a case may want to add to. Putting it in a
//! solver crate would make it unavailable to the others. This mirrors
//! OpenFOAM's own dependency position, where `fvOptions`/`fvModels` sits
//! directly on `finiteVolume`.
//!
//! # Sign convention, and the trap in it
//!
//! Sources are added to the **right-hand side**, i.e. the equation reads
//! `ddt(...) + div(...) == source`. A positive scalar source therefore
//! *increases* the solved quantity.
//!
//! Internally an `FvMatrix` stores the system as `A·φ = b`, so a right-hand
//! side contribution goes into `source`, while an implicit contribution
//! proportional to `φ` goes onto the **diagonal with the opposite sign**. That
//! asymmetry is the classic way to get a source term backwards, so
//! [`FvModel`] never asks a caller to place terms by hand:
//! [`add_source_scalar`](FvModels::add_source_scalar) and
//! [`add_source_vector`](FvModels::add_source_vector) do the placement, and the
//! individual models express themselves as an explicit part and an implicit
//! coefficient.
//!
//! # Cell selection
//!
//! Every model applies over a [`CellSelection`] — the whole mesh, or a named
//! subset. This is upstream's `cellSetOption`/`fvCellZone`. Selections hold
//! their cell list behind an `Arc`, per the workspace rule against lifetime
//! parameters, so sharing one selection between several models is free.

mod selection;
mod semi_implicit;
mod solidification_melting;
mod solidification_porosity;
mod temperature_table;
mod vof_solidification_melting;

pub use selection::CellSelection;
pub use semi_implicit::SemiImplicitSource;
pub use solidification_melting::{SolidificationMelting, SolidificationMeltingCoefficients};
pub use solidification_porosity::{MomentumEquationForm, SolidificationPorosity};
pub use temperature_table::TemperatureTable;
pub use vof_solidification_melting::VofSolidificationMelting;

use crate::fields::{VolScalarField, VolVectorField};
use crate::ldu_matrix::{FvMatrix, FvVectorMatrix};

/// One optional source term.
///
/// Enum dispatch rather than trait objects, per the workspace rule: the set of
/// source models is closed and known at compile time, adding one forces every
/// `match` to be revisited, and rust-analyzer can navigate to each variant —
/// none of which is true of upstream's runtime-selection table.
///
/// # Which equations a model contributes to
///
/// Not every model contributes to every equation.
/// [`contributes_to`](FvModel::contributes_to) reports what a model acts on,
/// mirroring upstream's `addSupFields`, so applying a whole collection to an
/// equation only invokes the models that have something to say about it.
#[derive(Debug, Clone)]
pub enum FvModel {
    /// A general explicit/implicit source, upstream `semiImplicitSource`.
    SemiImplicit(SemiImplicitSource),
    /// Solidification and melting by the enthalpy-porosity method, upstream
    /// `solidificationMelting`. The physically complete phase-change model:
    /// tracks a liquid fraction, absorbs latent heat, carries its own
    /// Boussinesq buoyancy.
    SolidificationMelting(SolidificationMelting),
    /// Solidification as a bare porous blockage, upstream
    /// `porosityModels::solidification`. **No latent heat and no buoyancy** —
    /// it only freezes the momentum out of cold cells.
    ///
    /// Strictly, upstream files this under `porosityModel` rather than
    /// `fvModel`; it is folded into this enum because from a solver's point of
    /// view it is the same thing — a term added to the momentum equation over
    /// a cell zone — and keeping it in a parallel mechanism would double the
    /// wiring for no gain.
    SolidificationPorosity(SolidificationPorosity),
    /// Solidification and melting of a VoF phase, upstream
    /// `VoFSolidificationMelting`. Needs a VoF phase fraction supplied from
    /// outside; see [`FvModels::correct`].
    VofSolidificationMelting(VofSolidificationMelting),
}

/// Which equation a source term is being applied to.
///
/// Models are attached to equations *by the name of the solved field*, exactly
/// as upstream does. This is stringly-typed for the same reason upstream is:
/// the solver that assembles an equation and the case that adds a source to it
/// do not share a type, and the field name is the only stable identifier they
/// both know.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EquationField<'n>(pub &'n str);

impl FvModel {
    /// The model's name, for diagnostics.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::SemiImplicit(m) => m.name(),
            Self::SolidificationMelting(m) => m.name(),
            Self::SolidificationPorosity(m) => m.name(),
            Self::VofSolidificationMelting(m) => m.name(),
        }
    }

    /// Whether this model contributes to the equation for `field`.
    ///
    /// Upstream's `addsSupToField`. A model asked for a field it does not act
    /// on contributes nothing, rather than erroring — that is how a single
    /// collection can be handed to every equation a solver assembles.
    #[must_use]
    pub fn contributes_to(&self, field: &str) -> bool {
        match self {
            Self::SemiImplicit(m) => m.field_name() == field,
            Self::SolidificationMelting(m) => {
                field == m.velocity_name() || field == m.energy_name()
            }
            Self::SolidificationPorosity(m) => field == m.velocity_name(),
            Self::VofSolidificationMelting(m) => {
                field == m.velocity_name() || field == m.energy_name()
            }
        }
    }
}

/// A collection of source terms, applied together to an equation.
///
/// Upstream's `fvModels`. Held by a solver and handed each equation in turn.
#[derive(Debug, Clone, Default)]
pub struct FvModels {
    models: Vec<FvModel>,
}

impl FvModels {
    /// An empty collection — a solver with no optional sources.
    #[must_use]
    pub fn new() -> Self {
        Self { models: Vec::new() }
    }

    /// Attach a model.
    pub fn push(&mut self, model: FvModel) {
        self.models.push(model);
    }

    /// The attached models.
    #[must_use]
    pub fn models(&self) -> &[FvModel] {
        &self.models
    }

    /// Mutable access, needed because stateful models advance their internal
    /// state when applied.
    pub fn models_mut(&mut self) -> &mut [FvModel] {
        &mut self.models
    }

    /// Whether any attached model contributes to `field`.
    ///
    /// Lets a solver skip assembling a source it would then discard.
    #[must_use]
    pub fn contributes_to(&self, field: &str) -> bool {
        self.models.iter().any(|m| m.contributes_to(field))
    }
}

/// The explicit and implicit halves of a source term, per cell.
///
/// Upstream splits a source into `Su` (explicit, independent of the solution)
/// and `Sp` (implicit, the coefficient of `φ`). Keeping them apart is not
/// bookkeeping: putting a stabilising negative coefficient on the diagonal
/// rather than in the right-hand side is what keeps the matrix diagonally
/// dominant and the solve stable, and it is why a Darcy drag is written
/// implicitly.
///
/// # Units
///
/// `explicit` is in the units of the equation's residual per unit volume;
/// `implicit` in those units divided by the solved variable. Both are
/// **per unit volume** — multiplication by the cell volume happens when the
/// contribution is placed into the matrix, matching upstream, which writes
/// `Sp[celli] += Vc*S`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SourceContribution {
    /// The part independent of the solved variable, per unit volume.
    pub explicit: f64,
    /// The coefficient of the solved variable, per unit volume.
    ///
    /// **Negative values stabilise.** A sink proportional to the solution has a
    /// negative coefficient here, and it lands on the matrix diagonal with the
    /// sign flipped, increasing diagonal dominance.
    pub implicit: f64,
}

impl FvModels {
    /// Advance every stateful model, once per timestep — upstream's
    /// `fvModels::correct()`.
    ///
    /// Only [`VofSolidificationMelting`] needs this: its solid fraction is
    /// driven by a VoF phase fraction that no equation assembly ever sees, so
    /// upstream updates it from a dedicated solver call rather than lazily from
    /// `addSup`. [`SolidificationMelting`] updates itself lazily instead (it
    /// reads only the temperature, which every `addSup` already has), and
    /// guards against doing so twice in one step; calling this does not disturb
    /// that.
    ///
    /// `vof_phase_fraction` is the VoF condensed-phase fraction \[-\]. Passing
    /// `None` while a [`VofSolidificationMelting`] model is attached leaves its
    /// solid fraction frozen — legal, but almost certainly a wiring mistake, so
    /// it is worth asserting against in a case setup.
    pub fn correct(
        &mut self,
        temperature: &VolScalarField,
        vof_phase_fraction: Option<&VolScalarField>,
    ) {
        for model in &mut self.models {
            if let FvModel::VofSolidificationMelting(m) = model {
                if let Some(alpha) = vof_phase_fraction {
                    m.correct(temperature, alpha);
                }
            }
        }
    }

    /// Roll every stateful model's history forward and re-arm its
    /// once-per-timestep guards.
    ///
    /// Call **once per completed timestep**, after the equations have been
    /// solved. Both phase-change models discretise their latent-heat source as
    /// a first-order Euler rate against the previous step's phase fraction, so
    /// omitting this does not merely stale the source — for
    /// [`SolidificationMelting`] it also leaves the once-per-step guard set,
    /// freezing the liquid fraction for the rest of the run.
    pub fn advance_time(&mut self) {
        for model in &mut self.models {
            match model {
                FvModel::SolidificationMelting(m) => m.advance_time(),
                FvModel::VofSolidificationMelting(m) => m.advance_time(),
                FvModel::SemiImplicit(_) | FvModel::SolidificationPorosity(_) => {
                    // Stateless — nothing to roll forward.
                }
            }
        }
    }

    /// Add every applicable model's contribution to a scalar equation.
    ///
    /// `field` names the solved field, `rho` is the density used by models that
    /// need it (pass a uniform field of ones for an incompressible equation,
    /// which is upstream's `geometricOneField`), and `dt` the timestep in
    /// seconds.
    ///
    /// Placement into the matrix follows the convention documented at module
    /// level: the explicit part goes to `source`, the implicit coefficient to
    /// the diagonal with its sign flipped, both scaled by the cell volume.
    pub fn add_source_scalar(
        &mut self,
        field: &str,
        rho: &VolScalarField,
        temperature: &VolScalarField,
        dt: f64,
        eqn: &mut FvMatrix,
    ) {
        let mesh = eqn.mesh.clone();
        for model in &mut self.models {
            if !model.contributes_to(field) {
                continue;
            }
            match model {
                FvModel::SemiImplicit(m) => {
                    for cell in m.selection().cells(mesh.n_cells) {
                        let v = mesh.cell_volumes[cell];
                        let c = m.contribution();
                        eqn.source[cell] += v * c.explicit;
                        eqn.ldu.diag[cell] -= v * c.implicit;
                    }
                }
                FvModel::SolidificationMelting(m) => {
                    m.add_energy_source(rho, temperature, dt, eqn);
                }
                FvModel::VofSolidificationMelting(m) => {
                    m.add_enthalpy_source(rho, dt, eqn);
                }
                FvModel::SolidificationPorosity(_) => {
                    // Momentum only — upstream's porosity model touches no
                    // energy equation at all, and `contributes_to` already
                    // filters it out for any field but its velocity.
                }
            }
        }
    }

    /// Add every applicable model's contribution to a vector equation.
    ///
    /// As [`add_source_scalar`](Self::add_source_scalar), for a momentum
    /// equation. `phase_fraction` is the optional solidifying-phase fraction
    /// \[-\] used by [`SolidificationPorosity`] models built with
    /// [`with_phase_fraction`](SolidificationPorosity::with_phase_fraction);
    /// pass `None` when no model needs one, which is upstream's default
    /// `alpha none`.
    pub fn add_source_vector(
        &mut self,
        field: &str,
        rho: &VolScalarField,
        temperature: &VolScalarField,
        velocity: &VolVectorField,
        phase_fraction: Option<&VolScalarField>,
        dt: f64,
        eqn: &mut FvVectorMatrix,
    ) {
        let _ = (velocity, dt);
        for model in &mut self.models {
            if !model.contributes_to(field) {
                continue;
            }
            match model {
                FvModel::SemiImplicit(_) => {
                    // The scalar semi-implicit source does not act on vector
                    // equations; upstream declares its field explicitly and it
                    // is filtered out by `contributes_to` above for any field
                    // other than its own.
                }
                FvModel::SolidificationMelting(m) => {
                    m.add_momentum_source(temperature, eqn);
                }
                FvModel::SolidificationPorosity(m) => {
                    m.add_momentum_source(temperature, rho, phase_fraction, eqn);
                }
                FvModel::VofSolidificationMelting(m) => {
                    m.add_momentum_source(rho, eqn);
                }
            }
        }
    }
}
