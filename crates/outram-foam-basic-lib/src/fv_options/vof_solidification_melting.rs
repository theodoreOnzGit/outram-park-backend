// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com / openfoam.org)
// Copyright (C) 2017-2026 OpenFOAM Foundation
// Upstream project: OpenFOAM-dev (OpenFOAM Foundation), vendored read-only at
//   crates/outram-foam-turbulence-lib/upstream_source/OpenFOAM
//   (README.org dated 14th July 2026; etc/bashrc WM_PROJECT_VERSION=dev).
// Upstream source files:
//   applications/modules/compressibleVoF/fvModels/VoFSolidificationMelting/
//     VoFSolidificationMelting.H
//     VoFSolidificationMelting.C
// Upstream licence: GPL-3.0-or-later.
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Solidification and melting **inside a VoF phase** — upstream
//! `fv::VoFSolidificationMelting`.
//!
//! # What belongs here, and what does not
//!
//! Only the source-term model. This module holds no volume-of-fluid machinery:
//! it does not advect an interface, does not compress it, and does not know how
//! `alpha_vof` was obtained. It *consumes* a VoF phase fraction supplied by
//! whatever solver owns the interface, which is exactly upstream's position —
//! upstream looks the field up from a `compressibleTwoPhaseVoFMixture` it does
//! not own either.
//!
//! Latent-heat and Darcy-drag physics that does **not** involve a VoF phase
//! belongs in [`solidification_melting`](super::solidification_melting)
//! instead.

use super::{CellSelection, TemperatureTable};
use crate::fields::VolScalarField;
use crate::ldu_matrix::{FvMatrix, FvVectorMatrix};

/// Solidification and melting of the condensed phase of a VoF simulation.
///
/// # How this differs from the enthalpy-porosity model
///
/// [`SolidificationMelting`](super::SolidificationMelting) *derives* its liquid
/// fraction by integrating an under-relaxed enthalpy update, so latent heat
/// feeds back on the temperature and holds a melting cell at its melting point.
/// This model does not integrate anything: the **solid fraction is read
/// straight off a table** `alphaSolid(T)`, capped by however much of the cell
/// the VoF phase occupies. It is the cruder of the two, and the right one only
/// when the interface tracking — not the phase-change thermodynamics — is what
/// the calculation is about.
///
/// It also carries **no buoyancy term**. The enthalpy-porosity model supplies
/// its own Boussinesq force; here the host VoF solver already carries gravity
/// through its `p_rgh` formulation, so adding one would double-count it.
///
/// # The two meanings of `relax` — an upstream trap
///
/// Both models take a `relax` coefficient defaulting to `0.9`, and they mean
/// **different things**:
///
/// - Enthalpy-porosity: `relax` scales the *increment*,
///   `α₁ ← α₁ + relax·Cp·(T − T_liq_eff)/L`. Raising it makes the update
///   larger.
/// - Here: `relax` is a *convex blend* between the new and old value,
///   `αₛ ← min(relax·α_vof·αₛ(T) + (1 − relax)·αₛ_old, α_vof)`. Raising it
///   makes the update *approach the table value faster*, and `relax = 1`
///   discards the history entirely.
///
/// Transplanting a tuned `relax` from one model to the other is therefore not
/// meaningful. This port keeps both upstream behaviours exactly as written
/// rather than unifying them.
///
/// # State
///
/// Stateful, like the enthalpy-porosity model: it holds the solid fraction and
/// its previous-timestep value, because the latent-heat source is a rate.
#[derive(Debug, Clone, PartialEq)]
pub struct VofSolidificationMelting {
    name: String,
    velocity_name: String,
    energy_name: String,
    selection: CellSelection,

    solid_fraction_table: TemperatureTable,
    latent_heat: f64,
    relaxation: f64,
    darcy_coefficient: f64,
    darcy_regularisation: f64,

    /// Solid fraction per selected cell \[-\], in `[0, α_vof]`.
    solid_fraction: Vec<f64>,
    /// Solid fraction at the previous timestep, for the latent-heat rate.
    solid_fraction_old: Vec<f64>,
}

impl VofSolidificationMelting {
    /// Build the model with upstream's numerical defaults.
    ///
    /// - `name` — identifier for diagnostics, upstream's dictionary key.
    /// - `velocity_name` / `energy_name` — the equations this contributes to,
    ///   upstream's `addSupFields` returning `{"U", thermo1().he().name()}`.
    /// - `selection` — the cells acted on, upstream's `cellZone`.
    /// - `solid_fraction_table` — upstream's `alphaSolidT`, solid fraction
    ///   \[-\] against temperature \[K\]. It **descends**: 1 when cold, 0 when
    ///   hot. That is the opposite sense to the drag table
    ///   [`SolidificationPorosity`](super::SolidificationPorosity) gives the
    ///   same [`TemperatureTable`] type; the type is shared because the
    ///   interpolation and end-clamping are identical, not because the
    ///   quantity is, and it carries no unit of its own.
    /// - `latent_heat` — `L` \[J/kg\].
    ///
    /// Defaults applied: `relax = 0.9`, `Cu = 1e5`, `q = 1e-3`, matching
    /// upstream's documented defaults.
    ///
    /// `n_cells` is the mesh cell count, needed to size the internal state.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        velocity_name: impl Into<String>,
        energy_name: impl Into<String>,
        selection: CellSelection,
        solid_fraction_table: TemperatureTable,
        latent_heat: f64,
        n_cells: usize,
    ) -> Self {
        let n = selection.len(n_cells);
        Self {
            name: name.into(),
            velocity_name: velocity_name.into(),
            energy_name: energy_name.into(),
            selection,
            solid_fraction_table,
            latent_heat,
            relaxation: 0.9,
            darcy_coefficient: 1.0e5,
            darcy_regularisation: 1.0e-3,
            solid_fraction: vec![0.0; n],
            solid_fraction_old: vec![0.0; n],
        }
    }

    /// Override the numerical coefficients.
    ///
    /// `relaxation` \[-\] in `[0, 1]` — the convex blend described on the type,
    /// **not** the enthalpy-porosity model's increment scale.
    /// `darcy_coefficient` `Cu` and `darcy_regularisation` `q` have the same
    /// meaning as in
    /// [`SolidificationMeltingCoefficients`](super::SolidificationMeltingCoefficients):
    /// `Cu` sets the drag magnitude, `q` bounds it in a fully solid cell.
    #[must_use]
    pub fn with_coefficients(
        mut self,
        relaxation: f64,
        darcy_coefficient: f64,
        darcy_regularisation: f64,
    ) -> Self {
        self.relaxation = relaxation;
        self.darcy_coefficient = darcy_coefficient;
        self.darcy_regularisation = darcy_regularisation;
        self
    }

    /// The model's name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The velocity field this adds a momentum sink to.
    #[must_use]
    pub fn velocity_name(&self) -> &str {
        &self.velocity_name
    }

    /// The energy field this adds latent heat to.
    #[must_use]
    pub fn energy_name(&self) -> &str {
        &self.energy_name
    }

    /// The cells acted on.
    #[must_use]
    pub fn selection(&self) -> &CellSelection {
        &self.selection
    }

    /// The current solid fraction \[-\], indexed by position within the
    /// selection (not by mesh cell index).
    #[must_use]
    pub fn solid_fraction(&self) -> &[f64] {
        &self.solid_fraction
    }

    /// Advance the solid fraction from the current temperature and VoF phase
    /// fraction — upstream's `correct()`.
    ///
    /// `αₛ ← min(relax·α_vof·αₛ(T) + (1 − relax)·αₛ_old, α_vof)`
    ///
    /// The `min` against `α_vof` is what stops the model claiming more solid
    /// than there is condensed phase in the cell: with `relax < 1` the blend
    /// can otherwise carry a stale value forward past a receding interface.
    ///
    /// # No once-per-timestep guard
    ///
    /// Unlike [`SolidificationMelting::update`](super::SolidificationMelting::update),
    /// this has none, because upstream's `correct()` is called by the solver at
    /// a defined point in the timestep rather than lazily from `addSup`. Call
    /// it exactly once per timestep, before the equations that use it. Calling
    /// it twice applies the relaxation twice, exactly as it would upstream.
    pub fn correct(&mut self, temperature: &VolScalarField, vof_phase_fraction: &VolScalarField) {
        let cells = self.selection.cells(temperature.internal.len());
        for (i, &cell) in cells.iter().enumerate() {
            let alpha_vof = vof_phase_fraction.internal[cell];
            let table = self.solid_fraction_table.value(temperature.internal[cell]);
            let blended = self.relaxation * alpha_vof * table
                + (1.0 - self.relaxation) * self.solid_fraction[i];
            self.solid_fraction[i] = blended.min(alpha_vof);
        }
    }

    /// Roll the solid-fraction history forward.
    ///
    /// Upstream's `alphaSolid_.oldTime()` at the top of `correct()`. Call once
    /// per completed timestep; the latent-heat source is proportional to
    /// `∂(ρα_s)/∂t` and is silently wrong without it.
    pub fn advance_time(&mut self) {
        self.solid_fraction_old
            .copy_from_slice(&self.solid_fraction);
    }

    /// The Darcy drag coefficient at a given **fluid** fraction \[1/s\] per
    /// unit density.
    ///
    /// `Cu (1 − α_fluid)² / (α_fluid³ + q)`, where `α_fluid = 1 − αₛ`.
    /// Positive here, because upstream writes the sink as `Sp -= Vc·ρ·S` with
    /// a positive `S`; see [`add_momentum_source`](Self::add_momentum_source)
    /// for why that ends up the same drag as the enthalpy-porosity model's
    /// negative one.
    #[must_use]
    pub fn darcy_coefficient(&self, fluid_fraction: f64) -> f64 {
        let solid = 1.0 - fluid_fraction;
        self.darcy_coefficient * solid * solid
            / (fluid_fraction * fluid_fraction * fluid_fraction + self.darcy_regularisation)
    }

    /// Add the momentum sink to a velocity equation.
    ///
    /// Upstream's `addSup(rho, U, eqn)`:
    ///
    /// ```text
    /// const scalar alphaFluid = 1 - alphaSolid_[celli];
    /// const scalar S = Cu_*sqr(1 - alphaFluid)/(pow3(alphaFluid) + q_);
    /// Sp[celli] -= Vc*rho[celli]*S;
    /// ```
    ///
    /// # Why the sign looks opposite to the enthalpy-porosity model
    ///
    /// It is not. Upstream writes the base fvModel as `Sp += Vc·S` with
    /// `S = −Cu(1−α₁)²/(α₁³+q)` and this one as `Sp −= Vc·ρ·S` with a
    /// *positive* `S`; the two products are the same negative number, modulo
    /// the density weighting. Both then pass through the
    /// `solve(UEqn == fvModels.source(...))` negation described on
    /// [`SolidificationMelting::add_momentum_source`](super::SolidificationMelting::add_momentum_source),
    /// reaching the solved system as a positive diagonal contribution — a
    /// stabilising sink. This port therefore places it the same way:
    /// `diag += V·ρ·Cu(1−α_fluid)²/(α_fluid³+q)`.
    pub fn add_momentum_source(&self, rho: &VolScalarField, eqn: &mut FvVectorMatrix) {
        let mesh = eqn.mesh.clone();
        let cells = self.selection.cells(mesh.n_cells);
        for (i, &cell) in cells.iter().enumerate() {
            let v = mesh.cell_volumes[cell];
            let fluid = 1.0 - self.solid_fraction[i];
            eqn.ldu.diag[cell] += v * rho.internal[cell] * self.darcy_coefficient(fluid);
        }
    }

    /// Add the latent-heat source to an **enthalpy** equation.
    ///
    /// Upstream's `addSup(alpha, rho, he, eqn)`, which is the single line
    /// `eqn += L*(fvc::ddt(rho, alphaSolid_))`. Following it through the
    /// `solve(hEqn == fvModels.source(...))` negation, the term reaching the
    /// solved right-hand side is `−L·∂(ρα_s)/∂t`: freezing (`∂α_s/∂t > 0`)
    /// *releases* energy, so it appears as a negative contribution to an
    /// equation whose sources are subtracted.
    ///
    /// # Enthalpy only — an upstream limitation kept, not corrected
    ///
    /// [`SolidificationMelting::add_energy_source`](super::SolidificationMelting::add_energy_source)
    /// branches on whether the equation is solved in temperature or enthalpy
    /// and divides by `Cp` in the temperature form. **This model has no such
    /// branch upstream** — it is only ever attached to `thermo1().he()`, an
    /// enthalpy — so none is added here. Attaching it to a temperature equation
    /// would over-source it by a factor of `Cp`, typically several hundred.
    /// That restriction is upstream's and is reproduced rather than papered
    /// over.
    ///
    /// `dt` is the timestep \[s\]; a non-positive `dt` contributes nothing.
    pub fn add_enthalpy_source(&self, rho: &VolScalarField, dt: f64, eqn: &mut FvMatrix) {
        if dt <= 0.0 {
            return;
        }
        let mesh = eqn.mesh.clone();
        let cells = self.selection.cells(mesh.n_cells);
        for (i, &cell) in cells.iter().enumerate() {
            let v = mesh.cell_volumes[cell];
            let rate =
                rho.internal[cell] * (self.solid_fraction[i] - self.solid_fraction_old[i]) / dt;
            eqn.source[cell] -= v * self.latent_heat * rate;
        }
    }
}

#[cfg(test)]
mod tests;
