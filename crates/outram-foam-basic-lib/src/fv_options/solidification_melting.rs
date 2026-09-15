// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
// Upstream source: src/fvModels/general/solidificationMelting/
//   (`solidificationMelting.C` / `.H`; ESI OpenFOAM calls the same model
//    `solidificationMeltingSource`).
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Solidification and melting by the enthalpy-porosity method.

use super::CellSelection;
use crate::fields::VolScalarField;
use crate::ldu_matrix::{FvMatrix, FvVectorMatrix};
use crate::primitives::Vector3;

/// Material and numerical coefficients of the melting model.
///
/// # Units
///
/// Strict SI throughout: temperatures in kelvin, latent heat in J/kg, the
/// expansion coefficient in 1/K, gravity in m/s², the reference density in
/// kg/m³.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolidificationMeltingCoefficients {
    /// Solidus temperature `T_sol` \[K\] — below this the material is fully
    /// solid.
    pub solidus: f64,
    /// Liquidus temperature `T_liq` \[K\] — above this it is fully liquid.
    ///
    /// Must exceed [`solidus`](Self::solidus). A pure substance melts at a
    /// single temperature, which would make the two equal and the liquid
    /// fraction a step function; the model needs a finite mushy range, so set
    /// them a degree or two apart even for a pure metal.
    pub liquidus: f64,
    /// Latent heat of fusion `L` \[J/kg\].
    pub latent_heat: f64,
    /// Liquid fraction at which the mushy zone is considered eutectic,
    /// `alpha1e` \[-\].
    ///
    /// Upstream's `alpha1e`. Shifts where the effective liquidus sits within
    /// the mushy range; zero recovers a linear solidus-to-liquidus ramp.
    pub eutectic_fraction: f64,
    /// Darcy coefficient `Cu` \[kg/(m³·s)\] for the momentum sink.
    ///
    /// Sets how strongly the mushy zone resists flow. Large values effectively
    /// freeze the solid; upstream's default is 100000.
    pub darcy_coefficient: f64,
    /// Small constant `q` \[-\] preventing division by zero in the Darcy term
    /// when the liquid fraction vanishes. Upstream's default is 0.001.
    pub darcy_regularisation: f64,
    /// Thermal expansion coefficient `beta` \[1/K\] for the buoyancy source.
    pub thermal_expansion: f64,
    /// Reference density `rho_ref` \[kg/m³\] used by the Boussinesq buoyancy
    /// term.
    pub reference_density: f64,
    /// Under-relaxation applied to the liquid-fraction update \[-\].
    ///
    /// The update is explicit and can overshoot, so upstream relaxes it;
    /// the default is 0.9. Values above 1 are not meaningful.
    pub relaxation: f64,
    /// Specific heat capacity `Cp` \[J/(kg·K)\].
    ///
    /// Upstream can look this up from a thermophysical model; this port takes
    /// it as a constant, which is upstream's `CpRef` mode. A caller needing a
    /// temperature-dependent `Cp` should update this between timesteps.
    pub specific_heat: f64,
}

impl SolidificationMeltingCoefficients {
    /// Coefficients with upstream's defaults for the numerical parameters,
    /// requiring only the material properties.
    ///
    /// `relaxation = 0.9`, `darcy_coefficient = 1e5`,
    /// `darcy_regularisation = 1e-3`, `eutectic_fraction = 0`.
    #[must_use]
    pub fn new(
        solidus: f64,
        liquidus: f64,
        latent_heat: f64,
        specific_heat: f64,
        reference_density: f64,
        thermal_expansion: f64,
    ) -> Self {
        Self {
            solidus,
            liquidus,
            latent_heat,
            eutectic_fraction: 0.0,
            darcy_coefficient: 1.0e5,
            darcy_regularisation: 1.0e-3,
            thermal_expansion,
            reference_density,
            relaxation: 0.9,
            specific_heat,
        }
    }
}

/// Solidification and melting of a pure or eutectic material.
///
/// # The enthalpy-porosity method
///
/// The difficulty with melting is that the solid/liquid interface moves, and
/// tracking it explicitly on a fixed mesh is painful. The enthalpy-porosity
/// method avoids tracking it at all. Every cell carries a **liquid fraction**
/// `α₁` between 0 and 1, and two things follow from it:
///
/// - **Latent heat** enters the energy equation as a source proportional to
///   `∂(ρα₁)/∂t`. A cell that is melting absorbs energy without rising in
///   temperature, which is exactly what latent heat means.
/// - **The solid is immobilised** by treating the mushy zone as a porous
///   medium: a Darcy drag `-Cu(1-α₁)²/(α₁³+q)` is applied to the momentum
///   equation. In a fully liquid cell `α₁ = 1` and the drag vanishes; as `α₁`
///   falls the drag grows without bound, pinning the velocity to zero. No
///   moving boundary is ever needed.
///
/// The Carman-Kozeny form of that drag is not arbitrary — it is the
/// permeability of a porous bed of fixed solid fraction, which is what a mushy
/// zone physically is.
///
/// Buoyancy is included in the Boussinesq form, `ρ_ref g β ΔT`, since natural
/// convection in the melt is usually what drives the process.
///
/// # State
///
/// Unlike most of this crate, the model is **stateful**: it stores the liquid
/// fraction per cell and advances it each time it is applied. Upstream hides
/// this behind C++ `mutable`; here the apply methods take `&mut self`, which
/// makes the state visible in the signature rather than hidden.
///
/// # Not covered
///
/// Upstream also handles a thermophysical-model lookup for `Cp` and mesh
/// topology changes. This port takes `Cp` as a coefficient (upstream's `CpRef`
/// mode) and does not track topology changes, since the mesh here is fixed at
/// construction.
#[derive(Debug, Clone, PartialEq)]
pub struct SolidificationMelting {
    name: String,
    velocity_name: String,
    energy_name: String,
    energy_is_temperature: bool,
    selection: CellSelection,
    coefficients: SolidificationMeltingCoefficients,
    gravity: Vector3,

    /// Liquid fraction per selected cell \[-\], in `[0, 1]`.
    liquid_fraction: Vec<f64>,
    /// Liquid fraction at the previous timestep, for the latent-heat rate.
    liquid_fraction_old: Vec<f64>,
    /// Temperature excess over the effective liquidus per selected cell \[K\],
    /// driving buoyancy.
    delta_temperature: Vec<f64>,
    /// Whether [`update`](SolidificationMelting::update) has already run in the
    /// current timestep. Upstream's `curTimeIndex_` guard, see there.
    updated_this_step: bool,
}

impl SolidificationMelting {
    /// Build the model.
    ///
    /// `energy_is_temperature` selects which form the energy equation is
    /// solved in: `true` for temperature, `false` for enthalpy. Upstream makes
    /// the same distinction by inspecting the solved field's dimensions, and it
    /// matters — the latent-heat source is divided by `Cp` in the temperature
    /// form and not in the enthalpy form. Getting it wrong scales the latent
    /// heat by `Cp`, typically a factor of several hundred.
    ///
    /// `n_cells` is the mesh cell count, needed to size the internal state.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        velocity_name: impl Into<String>,
        energy_name: impl Into<String>,
        energy_is_temperature: bool,
        selection: CellSelection,
        coefficients: SolidificationMeltingCoefficients,
        gravity: Vector3,
        n_cells: usize,
    ) -> Self {
        let n = selection.len(n_cells);
        Self {
            name: name.into(),
            velocity_name: velocity_name.into(),
            energy_name: energy_name.into(),
            energy_is_temperature,
            selection,
            coefficients,
            gravity,
            liquid_fraction: vec![0.0; n],
            liquid_fraction_old: vec![0.0; n],
            delta_temperature: vec![0.0; n],
            updated_this_step: false,
        }
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

    /// The current liquid fraction, indexed by position within the selection
    /// (not by mesh cell index).
    #[must_use]
    pub fn liquid_fraction(&self) -> &[f64] {
        &self.liquid_fraction
    }

    /// The effective liquidus temperature \[K\] at a given liquid fraction.
    ///
    /// `max(T_sol, T_sol + (T_liq - T_sol)(α₁ - α₁ₑ)/(1 - α₁ₑ))` — upstream's
    /// expression. The `max` matters: below the eutectic fraction the linear
    /// term would fall *below* the solidus, and clamping there is what keeps
    /// the eutectic plateau flat.
    #[must_use]
    pub fn effective_liquidus(&self, liquid_fraction: f64) -> f64 {
        let c = &self.coefficients;
        let linear = c.solidus
            + (c.liquidus - c.solidus) * (liquid_fraction - c.eutectic_fraction)
                / (1.0 - c.eutectic_fraction);
        c.solidus.max(linear)
    }

    /// Advance the liquid fraction from the current temperature field, **at
    /// most once per timestep**.
    ///
    /// Upstream's `update()`. Called automatically by the apply methods; public
    /// because a caller driving the model directly may want it.
    ///
    /// The update is explicit and under-relaxed:
    ///
    /// `α₁ ← clamp(α₁ + relax · Cp · (T - T_liq_eff(α₁)) / L, 0, 1)`
    ///
    /// then `ΔT` is recomputed against the **updated** fraction — which is what
    /// upstream does, and is not the same as using the old one.
    ///
    /// # The once-per-timestep guard
    ///
    /// A solver applies this model to *two* equations per timestep — momentum
    /// and energy — and each apply needs a current liquid fraction. Letting the
    /// update run on both would advance the under-relaxed explicit iteration
    /// twice per step, so the effective relaxation would be a function of how
    /// many equations happen to reference the model. Upstream guards against
    /// that with `curTimeIndex_`; this port uses a flag cleared by
    /// [`advance_time`](Self::advance_time). Calls after the first in a step
    /// return without doing anything.
    pub fn update(&mut self, temperature: &VolScalarField) {
        if self.updated_this_step {
            return;
        }
        self.updated_this_step = true;

        let c = self.coefficients;
        let cells = self.selection.cells(temperature.internal.len());

        for (i, &cell) in cells.iter().enumerate() {
            let t = temperature.internal[cell];
            let alpha = self.liquid_fraction[i];
            let new = alpha
                + c.relaxation * c.specific_heat * (t - self.effective_liquidus(alpha))
                    / c.latent_heat;
            self.liquid_fraction[i] = new.clamp(0.0, 1.0);
            self.delta_temperature[i] = t - self.effective_liquidus(self.liquid_fraction[i]);
        }
    }

    /// Roll the liquid-fraction history forward and re-arm
    /// [`update`](Self::update).
    ///
    /// Call once per completed timestep. It does two things, both required:
    ///
    /// - The latent-heat source is proportional to `∂(ρα₁)/∂t`, so it needs the
    ///   previous step's fraction; without this call that rate stays measured
    ///   against an ever-staler state and the latent heat is silently wrong.
    /// - It clears the once-per-timestep guard documented on
    ///   [`update`](Self::update). Omitting the call therefore freezes the
    ///   liquid fraction entirely, rather than merely staling it.
    pub fn advance_time(&mut self) {
        self.liquid_fraction_old
            .copy_from_slice(&self.liquid_fraction);
        self.updated_this_step = false;
    }

    /// The Darcy drag coefficient at a given liquid fraction \[kg/(m³·s)\].
    ///
    /// `-Cu (1-α₁)² / (α₁³ + q)`. Negative, since it is a sink.
    #[must_use]
    pub fn darcy_coefficient(&self, liquid_fraction: f64) -> f64 {
        let c = &self.coefficients;
        let solid = 1.0 - liquid_fraction;
        -c.darcy_coefficient * solid * solid
            / (liquid_fraction * liquid_fraction * liquid_fraction + c.darcy_regularisation)
    }

    /// Add the momentum sink and buoyancy to a velocity equation.
    ///
    /// Upstream's `addSup(U, eqn)`. The Darcy drag is the implicit coefficient
    /// and goes onto the diagonal (stabilising); the Boussinesq buoyancy is
    /// explicit and goes into the source. Both are scaled by the cell volume.
    ///
    /// # Why this is not a literal transcription of upstream
    ///
    /// Upstream writes
    ///
    /// ```text
    /// Sp[celli] += Vc*S;                      // S  = -Cu(1-a)^2/(a^3+q)
    /// Su[celli] += Vc*Sb;                     // Sb = rhoRef*g*beta*deltaT
    /// ```
    ///
    /// directly into `eqn.diag()` and `eqn.source()` — but that `eqn` is not
    /// the solver's equation. It is a *separate* `fvMatrix` that
    /// `fvModels::sourceTerm` builds and that the solver then combines with
    /// `solve(UEqn == fvModels.source(rho, U))`, where `operator==(A, B)`
    /// expands to `A - B`. Every coefficient an `addSup` writes is therefore
    /// **negated** before it reaches the system actually solved.
    ///
    /// Transcribing the two lines literally into a matrix that *is* the solved
    /// system inverts both terms: the Darcy drag becomes a source of momentum
    /// inside the solid instead of a sink, and it lands on the diagonal with
    /// the wrong sign, destroying diagonal dominance exactly where the
    /// coefficient is largest. This port therefore places the terms per the
    /// convention documented on [`SourceContribution`](super::SourceContribution)
    /// — `source += V·explicit`, `diag -= V·implicit` — which reproduces
    /// upstream's *solved* equation rather than its intermediate matrix.
    ///
    /// The resulting explicit buoyancy is `-ρ_ref β ΔT g`, which is the
    /// standard Boussinesq force: a cell hotter than its effective liquidus
    /// (`ΔT > 0`) is pushed *against* gravity.
    pub fn add_momentum_source(&mut self, temperature: &VolScalarField, eqn: &mut FvVectorMatrix) {
        self.update(temperature);

        let c = self.coefficients;
        let mesh = eqn.mesh.clone();
        let cells = self.selection.cells(mesh.n_cells);

        for (i, &cell) in cells.iter().enumerate() {
            let v = mesh.cell_volumes[cell];
            let drag = self.darcy_coefficient(self.liquid_fraction[i]);
            let buoyancy = self.gravity
                * (-c.reference_density * c.thermal_expansion * self.delta_temperature[i]);

            eqn.ldu.diag[cell] -= v * drag;
            eqn.source[cell] += buoyancy * v;
        }
    }

    /// Add the latent-heat source to an energy equation.
    ///
    /// Upstream's `addSup(rho, he, eqn)` via `apply()`, which writes
    /// `eqn -= L·∂(ρα₁)/∂t` (or `eqn -= (L/Cp)·∂(ρα₁)/∂t` in the temperature
    /// form) into the intermediate `fvModels` matrix. Following that through
    /// the `solve(hEqn == fvModels.source(...))` negation described on
    /// [`add_momentum_source`](Self::add_momentum_source), the term reaching
    /// the solved system's right-hand side is
    ///
    /// `-L · ∂(ρα₁)/∂t`,  or `-(L/Cp) · ∂(ρα₁)/∂t` when the equation is solved
    /// in temperature rather than enthalpy.
    ///
    /// The time derivative is discretised as first-order Euler against the
    /// previous step's liquid fraction, so
    /// [`advance_time`](Self::advance_time) must be called once per completed
    /// timestep.
    ///
    /// The sign is a sink: melting *absorbs* energy. That is why the
    /// contribution is subtracted from the equation, and it is the term that
    /// holds a melting cell at its melting point instead of letting it heat
    /// through.
    pub fn add_energy_source(
        &mut self,
        rho: &VolScalarField,
        temperature: &VolScalarField,
        dt: f64,
        eqn: &mut FvMatrix,
    ) {
        if dt <= 0.0 {
            return;
        }
        self.update(temperature);

        let c = self.coefficients;
        let scale = if self.energy_is_temperature {
            c.latent_heat / c.specific_heat
        } else {
            c.latent_heat
        };

        let mesh = eqn.mesh.clone();
        let cells = self.selection.cells(mesh.n_cells);

        for (i, &cell) in cells.iter().enumerate() {
            let v = mesh.cell_volumes[cell];
            let rate =
                rho.internal[cell] * (self.liquid_fraction[i] - self.liquid_fraction_old[i]) / dt;
            eqn.source[cell] -= v * scale * rate;
        }
    }
}

#[cfg(test)]
mod tests;
