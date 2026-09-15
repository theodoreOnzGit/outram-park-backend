// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com / openfoam.org)
// Copyright (C) 2017-2026 OpenFOAM Foundation
// Upstream project: OpenFOAM-dev (OpenFOAM Foundation), vendored read-only at
//   crates/outram-foam-turbulence-lib/upstream_source/OpenFOAM
//   (README.org dated 14th July 2026; etc/bashrc WM_PROJECT_VERSION=dev).
// Upstream source files:
//   src/finiteVolume/cfdTools/general/porosityModel/solidification/solidification.H
//   src/finiteVolume/cfdTools/general/porosityModel/solidification/solidification.C
//   src/finiteVolume/cfdTools/general/porosityModel/solidification/solidificationTemplates.C
// Upstream licence: GPL-3.0-or-later.
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! The **porosity-model** form of solidification — a temperature-tabulated
//! Darcy blockage with no latent heat and no buoyancy.
//!
//! # What belongs here, and what does not
//!
//! This module holds only upstream's `porosityModels::solidification`. It is a
//! deliberately cruder model than the enthalpy-porosity one in
//! [`solidification_melting`](super::solidification_melting), and the two are
//! not interchangeable — see [`SolidificationPorosity`] for the comparison.
//!
//! Latent heat, liquid-fraction tracking, mushy-zone thermodynamics and
//! Boussinesq buoyancy all belong in the enthalpy-porosity model, **not** here;
//! upstream is explicit that this model changes neither the temperature nor the
//! energy balance.

use super::{CellSelection, TemperatureTable};
use crate::fields::VolScalarField;
use crate::ldu_matrix::FvVectorMatrix;

/// Which momentum equation the drag is being added to.
///
/// Upstream branches on `UEqn.dimensions() == dimensions::force`, looking the
/// density field up only in that branch. A runtime dimension check has no Rust
/// equivalent here, so the choice is an explicit enum on the model: the caller
/// that assembled the equation is the one that knows which form it is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MomentumEquationForm {
    /// Kinematic momentum equation — pressure is `p/ρ` and density does not
    /// appear. The drag added is `V·D(T)`, so `D` is a rate \[1/s\].
    Kinematic,
    /// Density-weighted (force) momentum equation. The drag added is
    /// `V·ρ·D(T)`, upstream's `dimensions::force` branch.
    DensityWeighted,
}

/// Simple solidification as a porous blockage — upstream
/// `porosityModels::solidification`.
///
/// # The model
///
/// The solid phase is represented purely as a drag on the momentum equation,
///
/// `S = -α ρ D(T) U`,
///
/// with `D(T)` a user-supplied table of drag against temperature and `α` an
/// optional phase fraction of the solidifying phase. Nothing else happens:
/// upstream states plainly that **the latent heat of solidification is not
/// included and the temperature is unchanged by the modelled change of phase.**
///
/// # When to use this instead of the enthalpy-porosity model
///
/// [`SolidificationMelting`](super::SolidificationMelting) is the physically
/// complete model — it tracks a liquid fraction, absorbs latent heat, and
/// carries its own Boussinesq buoyancy. This one does none of that. It exists
/// because it is cheap and because it needs no thermophysical model at all: it
/// only ever reads a temperature field. Use it to freeze flow out of a region
/// whose thermal history you do not care about; use the enthalpy-porosity model
/// when the phase change itself is the physics of interest.
///
/// A gallium melting calculation, for instance, cannot be run with this model —
/// without latent heat the melt front advances at entirely the wrong rate.
///
/// # State
///
/// Stateless, unlike [`SolidificationMelting`](super::SolidificationMelting).
/// The drag depends only on the current temperature, so there is no history to
/// carry and no per-timestep update to guard.
#[derive(Debug, Clone, PartialEq)]
pub struct SolidificationPorosity {
    name: String,
    velocity_name: String,
    selection: CellSelection,
    drag: TemperatureTable,
    form: MomentumEquationForm,
    phase_fraction_name: Option<String>,
}

impl SolidificationPorosity {
    /// Build the model.
    ///
    /// - `name` — identifier for diagnostics, upstream's dictionary key.
    /// - `velocity_name` — the velocity field whose equation gains the drag.
    /// - `selection` — the cells acted on, upstream's `cellZone`.
    /// - `drag` — the `D(T)` table.
    /// - `form` — whether the momentum equation is kinematic or
    ///   density-weighted; see [`MomentumEquationForm`].
    ///
    /// The optional phase fraction (upstream's `alpha`, default `none`) is not
    /// set here; use
    /// [`with_phase_fraction`](Self::with_phase_fraction) if the drag should
    /// act only on a fraction of each cell.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        velocity_name: impl Into<String>,
        selection: CellSelection,
        drag: TemperatureTable,
        form: MomentumEquationForm,
    ) -> Self {
        Self {
            name: name.into(),
            velocity_name: velocity_name.into(),
            selection,
            drag,
            form,
            phase_fraction_name: None,
        }
    }

    /// Name the optional phase fraction field of the solidifying phase,
    /// upstream's `alpha` keyword.
    ///
    /// The drag is scaled by this field, so a cell only half occupied by the
    /// solidifying phase feels half the blockage. Without it upstream
    /// substitutes `geometricOneField()`, i.e. `α = 1` everywhere.
    #[must_use]
    pub fn with_phase_fraction(mut self, alpha_name: impl Into<String>) -> Self {
        self.phase_fraction_name = Some(alpha_name.into());
        self
    }

    /// The model's name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The velocity field this adds drag to.
    #[must_use]
    pub fn velocity_name(&self) -> &str {
        &self.velocity_name
    }

    /// The name of the optional solidifying-phase fraction field, if one was
    /// set.
    #[must_use]
    pub fn phase_fraction_name(&self) -> Option<&str> {
        self.phase_fraction_name.as_deref()
    }

    /// The cells acted on.
    #[must_use]
    pub fn selection(&self) -> &CellSelection {
        &self.selection
    }

    /// The `D(T)` table.
    #[must_use]
    pub fn drag_table(&self) -> &TemperatureTable {
        &self.drag
    }

    /// Which momentum-equation form this model was built for.
    #[must_use]
    pub fn form(&self) -> MomentumEquationForm {
        self.form
    }

    /// Add the porous drag to a velocity equation.
    ///
    /// Upstream's `correct(fvVectorMatrix& UEqn)`, which does
    ///
    /// ```text
    /// Udiag[celli] += V[celli]*alpha[celli]*rho[celli]*D_->value(T[celli]);
    /// ```
    ///
    /// `rho` is used only in the [`DensityWeighted`](MomentumEquationForm::DensityWeighted)
    /// form and ignored otherwise; pass any field in the kinematic case.
    /// `phase_fraction` is `Some` only if the model was built with
    /// [`with_phase_fraction`](Self::with_phase_fraction) — passing `None` when
    /// one was named silently drops the scaling, so callers should keep the two
    /// together.
    ///
    /// # Sign convention — this one is *not* negated, unlike the fvModel
    ///
    /// This is the trap when reading the two solidification models side by
    /// side. Upstream's **fvModel** `solidificationMelting` writes into an
    /// intermediate matrix that `solve(UEqn == fvModels.source(...))` then
    /// **subtracts**, so its `Sp[celli] += Vc*S` with a *negative* `S` reaches
    /// the solved system as a positive diagonal contribution. Upstream's
    /// **porosityModel**, by contrast, is handed the solver's own `UEqn` and
    /// mutates `UEqn.diag()` in place — nothing negates it afterwards, which is
    /// exactly why upstream writes `+=` here with a *positive* `D`.
    ///
    /// The two upstream files therefore look like they disagree about the sign
    /// of a Darcy drag, and they do not. This port writes
    /// `diag += V·α·ρ·D(T)` — upstream's expression unchanged — because
    /// `eqn` here, like upstream's `UEqn`, *is* the system being solved.
    /// Increasing the diagonal damps the velocity, which is the intended sink.
    pub fn add_momentum_source(
        &self,
        temperature: &VolScalarField,
        rho: &VolScalarField,
        phase_fraction: Option<&VolScalarField>,
        eqn: &mut FvVectorMatrix,
    ) {
        let mesh = eqn.mesh.clone();
        let cells = self.selection.cells(mesh.n_cells);

        for &cell in &cells {
            let v = mesh.cell_volumes[cell];
            let alpha = phase_fraction.map_or(1.0, |a| a.internal[cell]);
            let density = match self.form {
                MomentumEquationForm::Kinematic => 1.0,
                MomentumEquationForm::DensityWeighted => rho.internal[cell],
            };
            let d = self.drag.value(temperature.internal[cell]);
            eqn.ldu.diag[cell] += v * alpha * density * d;
        }
    }

    /// The diagonal drag coefficient per cell, upstream's `Udiag` inside
    /// `calcForce`.
    ///
    /// This is `V·α·ρ·D(T)` — the *same* quantity
    /// [`add_momentum_source`](Self::add_momentum_source) adds to the diagonal,
    /// exposed on its own so a caller can form upstream's diagnostic
    /// `force = Udiag·U` (units \[N\] in the density-weighted form, \[m⁴/s²\]
    /// in the kinematic one) without re-deriving it. The cell volume **is**
    /// included, because upstream's `calcForce` passes `mesh_.V()` into the
    /// same `apply` the momentum correction uses.
    ///
    /// It is a diagnostic only: no equation gains a term from calling this.
    ///
    /// Returns one value per **selected** cell, in selection order, not one per
    /// mesh cell.
    #[must_use]
    pub fn force_diagonal(
        &self,
        temperature: &VolScalarField,
        rho: &VolScalarField,
        phase_fraction: Option<&VolScalarField>,
        cell_volumes: &[f64],
    ) -> Vec<f64> {
        let n_cells = temperature.internal.len();
        self.selection
            .cells(n_cells)
            .iter()
            .map(|&cell| {
                let alpha = phase_fraction.map_or(1.0, |a| a.internal[cell]);
                let density = match self.form {
                    MomentumEquationForm::Kinematic => 1.0,
                    MomentumEquationForm::DensityWeighted => rho.internal[cell],
                };
                cell_volumes[cell] * alpha * density * self.drag.value(temperature.internal[cell])
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
