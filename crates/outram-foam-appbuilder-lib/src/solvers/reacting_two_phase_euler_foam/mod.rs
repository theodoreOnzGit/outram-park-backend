// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com / openfoam.org)
// Copyright (C) 2013-2026 OpenFOAM Foundation
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

//! # reactingTwoPhaseEulerFoam — reacting two-phase Euler-Euler solver (app layer)
//!
//! Application-layer translation of OpenFOAM's reacting two-phase Euler-Euler
//! solver. In the upstream we track (OpenFOAM-dev, the Foundation), the old
//! `reactingTwoPhaseEulerFoam` executable has been folded into the
//! runtime-selectable **`multiphaseEuler`** solver module
//! (`applications/modules/multiphaseEuler`, run via `foamRun -solver
//! multiphaseEuler`). This module ports that solver's **loop structure** and
//! its reacting-two-phase physics onto the pure-Rust finite-volume stack.
//!
//! ## What it composes vs. what it adds
//!
//! Per the workspace **Layer-5 rule** (solver-loop logic lives in the
//! application crate; the finite-volume/physics building blocks live in the
//! libraries), this solver:
//!
//! - **composes** the hydrodynamic core from [`outram_foam_multiphase`]:
//!   [`TwoFluidPimple`] already provides the two phase-momentum predictors, the
//!   shared-pressure PISO corrector (Rhie-Chow coupled), the interphase
//!   [`DragModel`], and dispersed volume-fraction transport
//!   (`α_d + α_c = 1`); and
//! - **adds** the reacting-Euler application layer that `multiphaseEuler` layers
//!   on top of the flow solve: per-phase **energy equations** with
//!   **interfacial heat transfer** ([`InterfacialHeatTransfer`]), **interfacial
//!   mass transfer / phase change** ([`InterfacialMassTransfer`]) with
//!   latent-heat coupling, and a **reaction heat source**
//!   ([`ReactionSource`]).
//!
//! ## Solver loop (mirrors `multiphaseEuler.C`)
//!
//! Each [`solve_timestep`](ReactingTwoPhaseEulerFoam::solve_timestep) runs, in
//! order:
//!
//! 1. **pre-predictor + momentum-predictor + pressure-corrector** — delegated to
//!    [`TwoFluidPimple::solve_timestep`] (`α`-transport, both `UEqns`, the shared
//!    `pEqn`, flux/velocity correction).
//! 2. **thermophysical-predictor** — a `nEnergyCorrectors` loop
//!    ([`thermophysical_predictor`](ReactingTwoPhaseEulerFoam::thermophysical_predictor))
//!    that assembles and solves both phase enthalpy equations with the
//!    one-resistance interfacial heat exchange and any reaction/latent sources,
//!    exactly the role of `multiphaseEuler`'s `energyPredictor()` /
//!    `thermophysicalPredictor()`.
//! 3. **phase-change update** — an operator-split application of the interfacial
//!    mass-transfer rate `ṁ` to the volume fractions and the phase enthalpies
//!    (latent heat).
//!
//! ## Energy equation (per phase `k`, conservative enthalpy form)
//!
//! Solving specific enthalpy `he_k = Cp_k · T_k` `[J/kg]`, mirroring
//! `rhoPimpleFoam`'s `heEqn` extended with the `α_k` weighting and interfacial
//! source (`multiphaseEuler` `thermophysicalPredictor.C:88`
//! `phase.heEqn() == heatTransfer[phase.name()]`):
//!
//! $$ \frac{\partial(\alpha_k \rho_k he_k)}{\partial t} + \nabla\cdot(\alpha_k \rho_k \phi_k he_k) - \nabla\cdot\left(\frac{\alpha_k \kappa_k}{Cp_k}\nabla he_k\right) = K\,(T_o - T_k) + \dot q_{rxn,k} + \dot q_{lat,k} $$
//!
//! with `K` `[W/(m³·K)]` the interfacial volumetric heat-transfer coefficient,
//! `T_o` the *other* phase's temperature, `q̇_rxn` the reaction heat, and
//! `q̇_lat` the latent-heat coupling. The interfacial term is split
//! **implicit in the own phase** (`K/Cp_k` on the diagonal) and **explicit in
//! the other phase** (`K·T_o` in the source), so at convergence of the corrector
//! loop the exchange is symmetric (`+K(T_o−T_k)` and `+K(T_k−T_o)` on the two
//! phases) and total thermal energy is conserved.
//!
//! > **⚠️ Unverified until validated — foundation.** Verification-tested only
//! > (formula exactness, thermal-equilibration limit, energy conservation,
//! > source-term response). **No benchmark/experimental validation** — that is
//! > the human V&V step (workspace `RESPONSIBLE_USE.md`). Not for nuclear
//! > facility operation, reactor control, safety-critical, or licensing
//! > decisions. Independent OUTRAM PARK fork, not the official OpenFOAM software
//! > (see `TRADEMARKS.md`).
//!
//! ### Honest scope (what this foundation does *not* yet do)
//!
//! - **Constant per-phase `Cp`, `κ`** (perfect-caloric closure `he = Cp·T`).
//!   Composition-resolved chemistry is available but reduced: an optional
//!   single-phase multicomponent [`PhaseSpecies`] transported with the phase
//!   mass flux + Fickian diffusion, plus a single global first-order Arrhenius
//!   [`ReactionMechanism`] (`fuel → product`, heat-release into that phase). A
//!   prescribed [`ReactionSource`] heat term is also kept for
//!   composition-free cases. Multi-step mechanisms, per-species properties /
//!   diffusivities, reversible/multi-phase reactions, and the full
//!   `phaseSystem` kinetics remain future work.
//! - **One-resistance** interfacial heat transfer (no interface-temperature
//!   two-resistance solve), **operator-split** phase change (the `ṁ` source is
//!   not folded into the implicit `α`-transport matrix), and no population
//!   balance, MRF, LTS, or buoyant `p_rgh` split.
//! - Enum dispatch (no `dyn`), no lifetime parameters, `uom` at the public
//!   boundary, documented raw `f64` (SI) in the inner assembly loops — per the
//!   workspace `CLAUDE.md`.

use crate::error::AppBuilderError;
use crate::io::control_dict::{ControlDict, StartControl, StopControl};
use crate::io::fv_schemes::FvSchemes;
use crate::io::fv_solution::FvSolution;
use outram_foam_basic_lib::prelude::*;
use outram_foam_multiphase::two_fluid::{DragModel, TwoFluidSystem};
use outram_foam_multiphase::two_fluid_pimple::TwoFluidPimple;
use outram_foam_multiphase::MultiphaseError;
use std::sync::Arc;
use uom::si::f64::{SpecificHeatCapacity, ThermalConductivity, ThermodynamicTemperature};
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

// ─────────────────────────────────────────────────────────────────────────────
// Per-phase thermal state
// ─────────────────────────────────────────────────────────────────────────────

/// Constant-property thermal state carried for one phase: its specific enthalpy
/// field `he_k = Cp_k·T_k` `[J/kg]` plus the constant caloric properties needed
/// to close the energy equation.
///
/// The temperature is recovered as `T_k = he_k / Cp_k` (reference `0 K`); only
/// enthalpy *differences* are physically meaningful in this foundation, which is
/// all the interfacial exchange, latent heat, and reaction terms depend on.
pub struct PhaseThermo {
    /// Specific enthalpy `he_k` `[J/kg]`, the solved energy variable.
    pub he: VolScalarField,
    /// Old-time specific enthalpy for the `∂/∂t` term.
    pub he_old: VolScalarField,
    /// Isobaric specific heat capacity `Cp_k` `[J/(kg·K)]` (constant, > 0).
    pub cp: f64,
    /// Thermal conductivity `κ_k` `[W/(m·K)]` (constant, ≥ 0).
    pub kappa: f64,
}

impl PhaseThermo {
    /// Build a phase thermal state from constant properties and a uniform
    /// initial temperature.
    ///
    /// # Parameters
    /// - `mesh` — the shared finite-volume mesh.
    /// - `name` — field label (e.g. `"he.water"`).
    /// - `cp` — isobaric specific heat capacity (`uom`), must be `> 0`.
    /// - `kappa` — thermal conductivity (`uom`), must be `≥ 0`.
    /// - `t0` — uniform initial temperature (`uom`), must be `> 0 K`.
    ///
    /// # Errors
    /// [`AppBuilderError::MissingKey`] is *not* used here; invalid inputs return
    /// [`MultiphaseError::InvalidInput`] (re-exported through this crate's
    /// solver error path) — `cp ≤ 0`, `kappa < 0`, or `t0 ≤ 0`.
    pub fn new(
        mesh: Arc<FvMesh>,
        name: impl Into<String>,
        cp: SpecificHeatCapacity,
        kappa: ThermalConductivity,
        t0: ThermodynamicTemperature,
    ) -> Result<Self, MultiphaseError> {
        let cp = cp.get::<joule_per_kilogram_kelvin>();
        let kappa = kappa.get::<watt_per_meter_kelvin>();
        let t0 = t0.get::<kelvin>();
        if cp <= 0.0 {
            return Err(MultiphaseError::InvalidInput(format!(
                "phase Cp must be > 0 (got {cp})"
            )));
        }
        if kappa < 0.0 {
            return Err(MultiphaseError::InvalidInput(format!(
                "phase kappa must be >= 0 (got {kappa})"
            )));
        }
        if t0 <= 0.0 {
            return Err(MultiphaseError::InvalidInput(format!(
                "initial temperature must be > 0 K (got {t0})"
            )));
        }
        let he = VolScalarField::uniform(name, mesh.clone(), cp * t0);
        let he_old = he.clone();
        Ok(Self {
            he,
            he_old,
            cp,
            kappa,
        })
    }

    /// Temperature field `T_k = he_k / Cp_k` `[K]` (zero-gradient boundaries).
    pub fn temperature(&self) -> VolScalarField {
        let mesh = self.he.mesh.clone();
        let n = mesh.n_cells;
        let inv_cp = 1.0 / self.cp;
        let vals: Vec<f64> = (0..n).map(|c| self.he.internal[c] * inv_cp).collect();
        scalar_field("T", mesh, vals)
    }

    /// Volume-averaged temperature `[K]` — a diagnostic used by the tests and by
    /// callers checking bulk thermal state.
    pub fn mean_temperature(&self) -> f64 {
        let mesh = self.he.mesh.clone();
        let mut sum_vt = 0.0;
        let mut sum_v = 0.0;
        for c in 0..mesh.n_cells {
            let v = mesh.cell_volumes[c];
            sum_vt += v * self.he.internal[c] / self.cp;
            sum_v += v;
        }
        sum_vt / sum_v
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Interfacial heat transfer (one-resistance, dispersed-sphere Nusselt models)
// ─────────────────────────────────────────────────────────────────────────────

/// Interfacial **heat-transfer** closure supplying the volumetric coefficient
/// `K` `[W/(m³·K)]` in the per-phase energy source `K·(T_o − T_k)`.
///
/// Ports the dispersed-sphere family of `heatTransferModels`
/// (`multiphaseEuler/phaseSystem/interfacialModels/heatTransferModels/`). The
/// coefficient follows the ported form
/// (`RanzMarshall.C`: `K = 6·max(α_d,α_res)·κ_c·Nu/d²`), with the Nusselt
/// number set by the variant:
///
/// - [`Spherical`](Self::Spherical) — `Nu = 2` (pure conduction limit of a
///   sphere; `sphericalHeatTransfer`).
/// - [`RanzMarshall`](Self::RanzMarshall) — `Nu = 2 + 0.6·√Re·∛Pr`
///   (`RanzMarshall.C:56`), the standard forced-convection correlation.
/// - [`ConstantNu`](Self::ConstantNu) — a prescribed constant Nusselt number.
///
/// Enum dispatch (not `dyn`) per the workspace design rules.
#[derive(Debug, Clone, Copy)]
pub enum InterfacialHeatTransfer {
    /// Conduction-limit sphere, `Nu = 2`.
    Spherical,
    /// Ranz-Marshall forced-convection correlation `Nu = 2 + 0.6·√Re·∛Pr`.
    RanzMarshall,
    /// Prescribed constant Nusselt number `Nu`.
    ConstantNu(f64),
}

impl InterfacialHeatTransfer {
    /// Nusselt number for a single cell given the local slip Reynolds number
    /// `re` `[-]` and continuous-phase Prandtl number `pr` `[-]`.
    ///
    /// `Nu = 2` (spherical), `2 + 0.6·√Re·∛Pr` (Ranz-Marshall), or the constant.
    pub fn nusselt(&self, re: f64, pr: f64) -> f64 {
        match self {
            Self::Spherical => 2.0,
            Self::RanzMarshall => 2.0 + 0.6 * re.max(0.0).sqrt() * pr.max(0.0).cbrt(),
            Self::ConstantNu(nu) => *nu,
        }
    }

    /// `true` if this model needs the slip Reynolds-number field (only
    /// Ranz-Marshall does) — lets the solver skip the `Re` evaluation (which
    /// requires `ν_c > 0`) for the constant-Nusselt variants.
    fn needs_reynolds(&self) -> bool {
        matches!(self, Self::RanzMarshall)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Interfacial mass transfer (phase change), operator-split
// ─────────────────────────────────────────────────────────────────────────────

/// Interfacial **mass-transfer** (phase-change) closure supplying the
/// volumetric rate `ṁ` `[kg/(m³·s)]` transferred **from the dispersed phase to
/// the continuous phase** (positive `ṁ` = dispersed evaporating/dissolving into
/// the continuous carrier; negative = condensation onto the dispersed phase).
///
/// Applied **operator-split** after the energy predictor: the volume fractions
/// are updated by `Δα_d = −ṁ·Δt/ρ_d` and the released/absorbed latent heat
/// `L·ṁ` is added to the continuous phase enthalpy (see
/// [`ReactingTwoPhaseEulerFoam::latent_heat`]). This is a foundation stand-in
/// for the phaseSystem's implicit mass-transfer coupling.
///
/// Enum dispatch (not `dyn`).
#[derive(Debug, Clone, Copy)]
pub enum InterfacialMassTransfer {
    /// No phase change (`ṁ = 0`).
    None,
    /// A prescribed spatially-uniform rate `ṁ` `[kg/(m³·s)]` (dispersed →
    /// continuous). For controlled tests and cases where the rate is supplied
    /// externally.
    ConstantRate(f64),
}

impl InterfacialMassTransfer {
    /// The uniform volumetric transfer rate `ṁ` `[kg/(m³·s)]` (0 for
    /// [`None`](Self::None)).
    fn rate(&self) -> f64 {
        match self {
            Self::None => 0.0,
            Self::ConstantRate(m) => *m,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Reaction heat source
// ─────────────────────────────────────────────────────────────────────────────

/// Which phase a volumetric source is deposited into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseSelector {
    /// The dispersed phase `d`.
    Dispersed,
    /// The continuous phase `c`.
    Continuous,
}

/// **Reaction heat source** — a prescribed volumetric heat-release rate added to
/// one phase's energy equation. Stands in for composition-resolved finite-rate
/// chemistry (not modelled at this foundation stage; see the module "Honest
/// scope").
///
/// Enum dispatch (not `dyn`).
#[derive(Debug, Clone, Copy)]
pub enum ReactionSource {
    /// No reaction.
    None,
    /// Uniform volumetric heat release `q̇` `[W/m³]` deposited into the selected
    /// phase.
    VolumetricHeat {
        /// Phase the heat is released into.
        phase: PhaseSelector,
        /// Volumetric heat-release rate `q̇` `[W/m³]` (positive = heating).
        q: f64,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// Composition-resolved species transport + finite-rate reaction
// ─────────────────────────────────────────────────────────────────────────────

/// Universal gas constant `R` `[J/(mol·K)]` (CODATA).
const R_GAS: f64 = 8.314_462_618;

/// Multicomponent **composition** carried inside one phase: a set of species
/// mass fractions `Y_i` `[-]` transported with that phase's mass flux plus a
/// single Fickian diffusivity. This is the composition the finite-rate
/// [`ReactionMechanism`] acts on — the "reacting" content the prescribed
/// [`ReactionSource`] heat term stands in for when no composition is modelled.
///
/// Mirrors `multiphaseEuler`'s per-phase `Y` fields (`thermophysicalPredictor.C`
/// `compositionPredictor()` — `phase.YiEqn(Y[i]) == …`), reduced to a single
/// phase with a shared constant diffusivity.
pub struct PhaseSpecies {
    /// Which phase carries this composition.
    pub phase: PhaseSelector,
    /// Species names, one per transported mass fraction.
    pub names: Vec<String>,
    /// Species mass-fraction fields `Y_i` `[-]`, `Σ_i Y_i = 1`.
    pub y: Vec<VolScalarField>,
    /// Old-time mass fractions for the `∂/∂t` term.
    pub y_old: Vec<VolScalarField>,
    /// Fickian mass diffusivity `D` `[m²/s]` (constant, ≥ 0), shared by all
    /// species.
    pub diffusivity: f64,
}

impl PhaseSpecies {
    /// Build a composition from species names and uniform initial mass
    /// fractions.
    ///
    /// # Parameters
    /// - `mesh` — shared finite-volume mesh.
    /// - `phase` — which phase carries the composition.
    /// - `names` — species labels (length `N ≥ 1`).
    /// - `y0` — uniform initial mass fractions `[-]`, length `N`, each in
    ///   `[0, 1]` and summing to `1` (to `1e-6`).
    /// - `diffusivity` — Fickian diffusivity `D` `[m²/s]`, must be `≥ 0`.
    ///
    /// # Errors
    /// [`MultiphaseError::InvalidInput`] on length mismatch, out-of-range `y0`,
    /// a non-unit sum, or negative diffusivity.
    pub fn new(
        mesh: Arc<FvMesh>,
        phase: PhaseSelector,
        names: Vec<String>,
        y0: &[f64],
        diffusivity: f64,
    ) -> Result<Self, MultiphaseError> {
        if names.is_empty() || names.len() != y0.len() {
            return Err(MultiphaseError::InvalidInput(format!(
                "species names ({}) and y0 ({}) must be non-empty and equal length",
                names.len(),
                y0.len()
            )));
        }
        if diffusivity < 0.0 {
            return Err(MultiphaseError::InvalidInput(format!(
                "diffusivity must be >= 0 (got {diffusivity})"
            )));
        }
        let mut sum = 0.0;
        for &y in y0 {
            if !(0.0..=1.0).contains(&y) {
                return Err(MultiphaseError::InvalidInput(format!(
                    "initial mass fraction must be in [0,1] (got {y})"
                )));
            }
            sum += y;
        }
        if (sum - 1.0).abs() > 1.0e-6 {
            return Err(MultiphaseError::InvalidInput(format!(
                "initial mass fractions must sum to 1 (got {sum})"
            )));
        }
        let y: Vec<VolScalarField> = names
            .iter()
            .zip(y0)
            .map(|(name, &y0)| VolScalarField::uniform(format!("Y.{name}"), mesh.clone(), y0))
            .collect();
        let y_old = y.clone();
        Ok(Self {
            phase,
            names,
            y,
            y_old,
            diffusivity,
        })
    }

    /// Mass-fraction field of species `i` `[-]`.
    pub fn mass_fraction(&self, i: usize) -> &VolScalarField {
        &self.y[i]
    }

    /// Volume-averaged mass fraction of species `i` `[-]` (diagnostic).
    pub fn mean_mass_fraction(&self, i: usize) -> f64 {
        let mesh = self.y[i].mesh.clone();
        let mut sum_vy = 0.0;
        let mut sum_v = 0.0;
        for c in 0..mesh.n_cells {
            let v = mesh.cell_volumes[c];
            sum_vy += v * self.y[i].internal[c];
            sum_v += v;
        }
        sum_vy / sum_v
    }
}

/// Finite-rate homogeneous **reaction mechanism** acting on a [`PhaseSpecies`]
/// composition and releasing heat into that phase's energy equation.
///
/// Enum dispatch (not `dyn`).
#[derive(Debug, Clone, Copy)]
pub enum ReactionMechanism {
    /// No reaction (species are transported inertly).
    None,
    /// A single global irreversible reaction `fuel → product` with first-order
    /// Arrhenius kinetics. The volumetric fuel consumption rate is
    ///
    /// ```text
    /// ω = A · exp(−Ea/(R·T)) · ρ · Y_fuel     [kg/(m³·s)]
    /// ```
    ///
    /// (`A` pre-exponential `[1/s]`, `Ea` activation energy `[J/mol]`). Species
    /// sources are `−ω` (fuel) and `+ω` (product); the heat release is
    /// `q̇ = ΔH · ω` `[W/m³]` deposited into the reacting phase's enthalpy.
    Arrhenius {
        /// Index of the fuel species in [`PhaseSpecies::y`].
        fuel: usize,
        /// Index of the product species in [`PhaseSpecies::y`].
        product: usize,
        /// Pre-exponential factor `A` `[1/s]`.
        a_pre: f64,
        /// Activation energy `Ea` `[J/mol]`.
        e_act: f64,
        /// Heat of reaction `ΔH` per unit fuel mass `[J/kg]` (positive =
        /// exothermic).
        delta_h: f64,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// The solver
// ─────────────────────────────────────────────────────────────────────────────

/// Reacting two-phase Euler-Euler solver (application layer) — see the module
/// documentation for the algorithm, provenance, and honest scope.
pub struct ReactingTwoPhaseEulerFoam {
    /// Hydrodynamic core (two phase momenta + shared pressure + `α`-transport +
    /// drag), composed from [`outram_foam_multiphase`].
    pub hydro: TwoFluidPimple,
    /// Thermal state of the dispersed phase.
    pub dispersed_thermo: PhaseThermo,
    /// Thermal state of the continuous phase.
    pub continuous_thermo: PhaseThermo,
    /// Interfacial heat-transfer closure.
    pub heat_transfer: InterfacialHeatTransfer,
    /// Interfacial mass-transfer (phase-change) closure.
    pub mass_transfer: InterfacialMassTransfer,
    /// Prescribed reaction heat source (composition-free stand-in).
    pub reaction: ReactionSource,
    /// Optional composition-resolved species transported in one phase. When set
    /// (with a non-[`None`](ReactionMechanism::None)
    /// [`reaction_mechanism`](Self::reaction_mechanism)) the solver runs a
    /// composition predictor before the energy predictor, exactly as
    /// `multiphaseEuler` does.
    pub species: Option<PhaseSpecies>,
    /// Finite-rate reaction mechanism acting on [`species`](Self::species);
    /// releases heat into that phase's enthalpy equation.
    pub reaction_mechanism: ReactionMechanism,
    /// Cached finite-rate reaction heat `[W/m³]` produced by the composition
    /// predictor and consumed by the energy predictor within the same corrector.
    reaction_heat: Option<VolScalarField>,
    /// Latent heat of the dispersed→continuous phase change `L` `[J/kg]`
    /// (absorbed on evaporation `ṁ > 0`). Only used when
    /// [`mass_transfer`](Self::mass_transfer) is active.
    pub latent_heat: f64,
    /// Residual volume-fraction floor `α_res` `[-]` guarding the `max(α,α_res)`
    /// in the interfacial coefficient and the per-phase energy denominators
    /// (mirrors OpenFOAM's `residualAlpha`). Must be `> 0`.
    pub residual_alpha: f64,
    /// Number of energy correctors per time step (`nEnergyCorrectors`, ≥ 1).
    pub n_energy_correctors: usize,
    /// Time-control dictionary.
    pub control: ControlDict,
    /// Discretisation schemes.
    pub schemes: FvSchemes,
    /// Linear-solver / PIMPLE control.
    pub solution: FvSolution,
    /// Current simulation time `[s]`.
    pub time: f64,
    /// Linear-solver settings for the energy solves.
    settings: SolverSettings,
}

impl ReactingTwoPhaseEulerFoam {
    /// Assemble a reacting two-phase Euler solver from a hydrodynamic core and
    /// the two phases' thermal states.
    ///
    /// Model defaults: [`InterfacialHeatTransfer::Spherical`], no mass transfer,
    /// no reaction, zero latent heat, `α_res = 1e-6`, `nEnergyCorrectors = 1`,
    /// default linear-solver settings. Set the model fields directly (they are
    /// `pub`) before stepping.
    ///
    /// Both thermal states and the hydrodynamic phases must be built on the same
    /// mesh; this is checked by `Arc` pointer identity.
    ///
    /// # Errors
    /// [`AppBuilderError::MissingKey`] is not used; a mesh mismatch returns
    /// [`MultiphaseError::InvalidInput`].
    pub fn new(
        hydro: TwoFluidPimple,
        dispersed_thermo: PhaseThermo,
        continuous_thermo: PhaseThermo,
        control: ControlDict,
        schemes: FvSchemes,
        solution: FvSolution,
    ) -> Result<Self, MultiphaseError> {
        if !Arc::ptr_eq(&hydro.mesh, &dispersed_thermo.he.mesh)
            || !Arc::ptr_eq(&hydro.mesh, &continuous_thermo.he.mesh)
        {
            return Err(MultiphaseError::InvalidInput(
                "hydro core and both phase thermal states must share the same mesh".into(),
            ));
        }
        Ok(Self {
            hydro,
            dispersed_thermo,
            continuous_thermo,
            heat_transfer: InterfacialHeatTransfer::Spherical,
            mass_transfer: InterfacialMassTransfer::None,
            reaction: ReactionSource::None,
            species: None,
            reaction_mechanism: ReactionMechanism::None,
            reaction_heat: None,
            latent_heat: 0.0,
            residual_alpha: 1.0e-6,
            n_energy_correctors: 1,
            control,
            schemes,
            solution,
            time: 0.0,
            settings: SolverSettings::default(),
        })
    }

    /// Dispersed-phase temperature field `[K]`.
    pub fn dispersed_temperature(&self) -> VolScalarField {
        self.dispersed_thermo.temperature()
    }

    /// Continuous-phase temperature field `[K]`.
    pub fn continuous_temperature(&self) -> VolScalarField {
        self.continuous_thermo.temperature()
    }

    /// Advance the coupled flow, energy, and phase-change state by one time step
    /// `dt` `[s]`, mirroring `multiphaseEuler.C`'s solve sequence.
    ///
    /// 1. hydrodynamics ([`TwoFluidPimple::solve_timestep`]);
    /// 2. energy ([`thermophysical_predictor`](Self::thermophysical_predictor),
    ///    `nEnergyCorrectors` sweeps);
    /// 3. operator-split phase change ([`apply_mass_transfer`](Self::apply_mass_transfer)).
    ///
    /// On return, the phase velocities/fluxes/pressure, the volume fractions,
    /// and both phase enthalpies hold the new-time state, and [`time`](Self::time)
    /// is advanced by `dt`.
    ///
    /// # Errors
    /// - [`MultiphaseError::InvalidInput`] if `dt ≤ 0`, `nEnergyCorrectors == 0`,
    ///   or `α_res ≤ 0`.
    /// - Propagates hydrodynamic, drag, and linear-solve failures from the
    ///   composed [`TwoFluidPimple`] and the energy solves.
    pub fn solve_timestep(&mut self, dt: f64) -> Result<(), MultiphaseError> {
        if dt <= 0.0 {
            return Err(MultiphaseError::InvalidInput(format!(
                "dt must be > 0 (got {dt})"
            )));
        }
        if self.n_energy_correctors == 0 {
            return Err(MultiphaseError::InvalidInput(
                "nEnergyCorrectors must be >= 1".into(),
            ));
        }
        if self.residual_alpha <= 0.0 {
            return Err(MultiphaseError::InvalidInput(format!(
                "residual_alpha must be > 0 (got {})",
                self.residual_alpha
            )));
        }

        // Freeze old-time enthalpy for the ddt term (state entering the step).
        self.dispersed_thermo.he_old = self.dispersed_thermo.he.clone();
        self.continuous_thermo.he_old = self.continuous_thermo.he.clone();
        // Freeze old-time species mass fractions.
        if let Some(species) = self.species.as_mut() {
            species.y_old = species.y.clone();
        }

        // (1) Flow: momentum predictors + shared-pressure PISO + α transport.
        self.hydro.solve_timestep(dt)?;

        // (2) Energy: interfacial heat transfer + reaction sources.
        self.thermophysical_predictor(dt)?;

        // (3) Phase change (operator-split mass transfer + latent heat).
        self.apply_mass_transfer(dt)?;

        self.time += dt;
        Ok(())
    }

    /// Energy predictor loop — assemble and solve both phase enthalpy equations
    /// `nEnergyCorrectors` times with the one-resistance interfacial exchange.
    ///
    /// Both phases use the *same* start-of-corrector temperature snapshot for
    /// the explicit other-phase term (Jacobi coupling), so the exchange stays
    /// symmetric and the corrector loop converges the implicit/explicit split.
    ///
    /// # Errors
    /// Propagates [`MultiphaseError`] from the interfacial-coefficient
    /// evaluation (e.g. undefined slip Reynolds number when `ν_c ≤ 0` under
    /// Ranz-Marshall).
    pub fn thermophysical_predictor(&mut self, dt: f64) -> Result<(), MultiphaseError> {
        for _ecorr in 0..self.n_energy_correctors {
            // Interfacial volumetric coefficient K [W/(m³·K)] from current state.
            let k_ht = self.interfacial_coefficient()?;

            // Composition predictor (species transport + finite-rate reaction),
            // ahead of energy so its heat release feeds the enthalpy equations —
            // the order `multiphaseEuler` uses (compositionPredictor →
            // energyPredictor).
            self.composition_predictor(dt)?;

            // Snapshot both temperatures for the explicit other-phase source.
            let t_d_snap = self.dispersed_thermo.temperature();
            let t_c_snap = self.continuous_thermo.temperature();

            // Dispersed phase enthalpy equation (other phase = continuous).
            self.solve_phase_energy(dt, true, &k_ht, &t_c_snap)?;
            // Continuous phase enthalpy equation (other phase = dispersed).
            self.solve_phase_energy(dt, false, &k_ht, &t_d_snap)?;
        }
        Ok(())
    }

    /// Composition predictor — advance the [`species`](Self::species) mass
    /// fractions by one implicit step of their per-phase transport equation and
    /// apply the finite-rate [`reaction_mechanism`](Self::reaction_mechanism),
    /// caching the reaction heat for the energy predictor.
    ///
    /// For species `i` in the reacting phase (density `ρ`, fraction `α`, mass
    /// flux `α ρ φ`, diffusivity `D`):
    ///
    /// ```text
    /// ∂(α ρ Y_i)/∂t + ∇·(α ρ φ Y_i) − ∇·(α ρ D ∇Y_i) = ν_i · ω
    /// ```
    ///
    /// with `ν_fuel = −1`, `ν_product = +1`, else `0`, and the Arrhenius rate
    /// `ω = A·exp(−Ea/(R·T))·ρ·Y_fuel` `[kg/(m³·s)]`. After the solves each `Y_i`
    /// is clamped to `[0,1]` and the set is renormalised so `Σ_i Y_i = 1`
    /// (a bounded stand-in for OpenFOAM's `Yt`-normalisation). A no-op when no
    /// composition is configured.
    ///
    /// # Errors
    /// [`MultiphaseError::Solver`] if a species solve returns a non-finite value.
    pub fn composition_predictor(&mut self, dt: f64) -> Result<(), MultiphaseError> {
        self.reaction_heat = None;
        // Pull everything needed out of `self.species` up front so the later
        // write-back does not overlap this immutable borrow.
        let (phase_sel, mut y, y_old, diffusivity, n_species) = match &self.species {
            None => return Ok(()),
            Some(s) => (
                s.phase,
                s.y.clone(),
                s.y_old.clone(),
                s.diffusivity,
                s.names.len(),
            ),
        };
        let dispersed = phase_sel == PhaseSelector::Dispersed;
        let mesh = self.hydro.mesh.clone();
        let n = mesh.n_cells;

        // Reacting-phase handles (α, U, ρ) and its temperature.
        let (alpha, u_phase, rho, t_field) = if dispersed {
            (
                self.hydro.system.dispersed.alpha().clone(),
                self.hydro.system.dispersed.u().clone(),
                self.hydro
                    .system
                    .dispersed
                    .rho()
                    .get::<uom::si::mass_density::kilogram_per_cubic_meter>(),
                self.dispersed_thermo.temperature(),
            )
        } else {
            (
                self.hydro.system.continuous.alpha().clone(),
                self.hydro.system.continuous.u().clone(),
                self.hydro
                    .system
                    .continuous
                    .rho()
                    .get::<uom::si::mass_density::kilogram_per_cubic_meter>(),
                self.continuous_thermo.temperature(),
            )
        };

        // Arrhenius rate ω [kg/(m³·s)] and heat q̇ = ΔH·ω [W/m³].
        let (omega, fuel_i, product_i) = match self.reaction_mechanism {
            ReactionMechanism::None => (vec![0.0_f64; n], None, None),
            ReactionMechanism::Arrhenius {
                fuel,
                product,
                a_pre,
                e_act,
                delta_h: _,
            } => {
                let yf = &y[fuel];
                let w: Vec<f64> = (0..n)
                    .map(|c| {
                        let t = t_field.internal[c].max(1.0);
                        (a_pre * (-e_act / (R_GAS * t)).exp() * rho * yf.internal[c]).max(0.0)
                    })
                    .collect();
                (w, Some(fuel), Some(product))
            }
        };
        if let ReactionMechanism::Arrhenius { delta_h, .. } = self.reaction_mechanism {
            let q: Vec<f64> = omega.iter().map(|&w| delta_h * w).collect();
            self.reaction_heat = Some(scalar_field("qRxn", mesh.clone(), q));
        }

        // Shared conservative-transport coefficients (as in the energy solve).
        let alpha_rho: VolScalarField = {
            let vals: Vec<f64> = (0..n).map(|c| alpha.internal[c].max(0.0) * rho).collect();
            scalar_field("alphaRho", mesh.clone(), vals)
        };
        let alpha_rho_phi = fvc::interpolate(&alpha_rho) * fvc::flux(&u_phase);
        let alpha_rho_d = {
            let vals: Vec<f64> = (0..n)
                .map(|c| alpha.internal[c].max(0.0) * rho * diffusivity)
                .collect();
            fvc::interpolate(&scalar_field("alphaRhoD", mesh.clone(), vals))
        };

        // Solve each species transport with its reaction source ν_i·ω.
        for i in 0..n_species {
            let nu = if Some(i) == fuel_i {
                -1.0
            } else if Some(i) == product_i {
                1.0
            } else {
                0.0
            };
            let yi = y[i].clone();
            let conv = fvc::div(&alpha_rho_phi, &yi);
            let mut eqn =
                fvm::ddt_coeff(&alpha_rho, &yi, &y_old[i], dt) + fvm::laplacian(&alpha_rho_d, &yi);
            for c in 0..n {
                let v = mesh.cell_volumes[c];
                eqn.source[c] -= v * conv.internal[c];
                eqn.source[c] += v * nu * omega[c];
            }
            let (yi_new, _perf) = eqn.solve(format!("Y{i}"), self.settings);
            for c in 0..n {
                if !yi_new.internal[c].is_finite() {
                    return Err(MultiphaseError::Solver(format!(
                        "species {i} became non-finite in cell {c}"
                    )));
                }
            }
            y[i] = yi_new;
        }

        // Clamp to [0,1] and renormalise so Σ_i Y_i = 1 (bounded stand-in).
        for c in 0..n {
            let mut sum = 0.0;
            for yi in y.iter_mut() {
                let v = yi.internal[c].clamp(0.0, 1.0);
                yi.internal[c] = v;
                sum += v;
            }
            if sum > 0.0 {
                for yi in y.iter_mut() {
                    yi.internal[c] /= sum;
                }
            }
        }

        // Write the updated composition back.
        if let Some(species) = self.species.as_mut() {
            species.y = y;
        }
        Ok(())
    }

    /// Assemble and solve one phase's conservative enthalpy equation.
    ///
    /// `dispersed == true` selects the dispersed phase; `k_ht` is the interfacial
    /// coefficient field `[W/(m³·K)]`; `t_other` is the *other* phase's
    /// (snapshot) temperature `[K]` for the explicit interfacial source.
    fn solve_phase_energy(
        &mut self,
        dt: f64,
        dispersed: bool,
        k_ht: &VolScalarField,
        t_other: &VolScalarField,
    ) -> Result<(), MultiphaseError> {
        let mesh = self.hydro.mesh.clone();
        let n = mesh.n_cells;

        // Phase-dependent handles (immutable reads from the hydro system).
        let (alpha, u_phase, rho, cp, kappa) = if dispersed {
            (
                self.hydro.system.dispersed.alpha().clone(),
                self.hydro.system.dispersed.u().clone(),
                self.hydro
                    .system
                    .dispersed
                    .rho()
                    .get::<uom::si::mass_density::kilogram_per_cubic_meter>(),
                self.dispersed_thermo.cp,
                self.dispersed_thermo.kappa,
            )
        } else {
            (
                self.hydro.system.continuous.alpha().clone(),
                self.hydro.system.continuous.u().clone(),
                self.hydro
                    .system
                    .continuous
                    .rho()
                    .get::<uom::si::mass_density::kilogram_per_cubic_meter>(),
                self.continuous_thermo.cp,
                self.continuous_thermo.kappa,
            )
        };

        // ── Conservative-transport coefficients ──────────────────────────────
        // ddt/convection mass weight  α_k ρ_k  [kg/m³]  (cellwise, floored).
        let alpha_rho: VolScalarField = {
            let vals: Vec<f64> = (0..n).map(|c| alpha.internal[c].max(0.0) * rho).collect();
            scalar_field("alphaRho", mesh.clone(), vals)
        };
        // Phase mass flux  α_k ρ_k (U_k·S_f)  [kg/s]  = interp(α_k ρ_k)·flux(U_k).
        let alpha_rho_phi = fvc::interpolate(&alpha_rho) * fvc::flux(&u_phase);
        // Enthalpy diffusivity  α_k κ_k / Cp_k  [kg/(m·s)]: build the cell field
        // then interpolate to faces (no field×scalar operator on VolScalarField).
        let alpha_diff = {
            let vals: Vec<f64> = (0..n)
                .map(|c| alpha.internal[c].max(0.0) * kappa / cp)
                .collect();
            fvc::interpolate(&scalar_field("alphaKappaByCp", mesh.clone(), vals))
        };

        // Explicit convection ∇·(alphaRhoPhi · he)  [W/m³] (per volume).
        let he_field = if dispersed {
            self.dispersed_thermo.he.clone()
        } else {
            self.continuous_thermo.he.clone()
        };
        let conv = fvc::div(&alpha_rho_phi, &he_field);

        // ── Assemble  ∂(αρ he)/∂t − ∇·(αρφ he) [explicit] + ∇·(ακ/Cp ∇he) ────
        let he_old = if dispersed {
            self.dispersed_thermo.he_old.clone()
        } else {
            self.continuous_thermo.he_old.clone()
        };
        let mut eqn = fvm::ddt_coeff(&alpha_rho, &he_field, &he_old, dt)
            + fvm::laplacian(&alpha_diff, &he_field);

        // Interfacial one-resistance exchange  K·(T_o − T_k):
        //   implicit sink  (K/Cp) he_k  → diagonal;
        //   explicit source  K·T_o      → source.
        // Reaction + latent heat: explicit volumetric sources [W/m³].
        let (q_rxn, latent_phase, m_dot) = self.energy_sources(dispersed);
        {
            let sink: Vec<f64> = (0..n).map(|c| k_ht.internal[c] / cp).collect();
            let sink_field = scalar_field("htSink", mesh.clone(), sink);
            // Implicit interfacial sink on he.
            eqn = eqn + fvm::sp(&sink_field, &he_field);

            for c in 0..n {
                let v = mesh.cell_volumes[c];
                // explicit convection (moved to RHS): source -= V·∇·(φh).
                eqn.source[c] -= v * conv.internal[c];
                // explicit interfacial source  K·T_o.
                eqn.source[c] += v * k_ht.internal[c] * t_other.internal[c];
                // reaction heat q̇ [W/m³].
                eqn.source[c] += v * q_rxn[c];
                // latent-heat coupling  ∓L·ṁ  [W/m³] (see energy_sources).
                eqn.source[c] += v * latent_phase[c];
            }
            let _ = m_dot; // rate is applied to α in apply_mass_transfer.
        }

        let (he_new, perf) = eqn.solve(
            if dispersed {
                "he.dispersed"
            } else {
                "he.continuous"
            },
            self.settings,
        );
        if !perf.final_residual.is_finite() {
            return Err(MultiphaseError::Solver(
                "phase enthalpy solve returned a non-finite residual".into(),
            ));
        }
        for c in 0..n {
            if !he_new.internal[c].is_finite() {
                return Err(MultiphaseError::Solver(format!(
                    "phase enthalpy became non-finite in cell {c}"
                )));
            }
        }
        if dispersed {
            self.dispersed_thermo.he = he_new;
        } else {
            self.continuous_thermo.he = he_new;
        }
        Ok(())
    }

    /// Per-cell reaction heat `q̇` `[W/m³]` and latent-heat coupling `[W/m³]` for
    /// the selected phase, plus the (uniform) mass-transfer rate `ṁ`.
    ///
    /// Latent-heat convention: evaporation `ṁ > 0` (dispersed → continuous)
    /// **absorbs** `L·ṁ` — a heat sink on the continuous phase (`−L·ṁ`) — during
    /// the energy solve, consistent with the operator-split `α` update in
    /// [`apply_mass_transfer`](Self::apply_mass_transfer).
    fn energy_sources(&self, dispersed: bool) -> (Vec<f64>, Vec<f64>, f64) {
        let n = self.hydro.mesh.n_cells;
        let mut q_rxn = vec![0.0_f64; n];
        if let ReactionSource::VolumetricHeat { phase, q } = self.reaction {
            let hit = matches!(
                (phase, dispersed),
                (PhaseSelector::Dispersed, true) | (PhaseSelector::Continuous, false)
            );
            if hit {
                q_rxn.iter_mut().for_each(|x| *x = q);
            }
        }
        // Finite-rate reaction heat (cached by the composition predictor) is
        // deposited into the phase that carries the reacting composition.
        if let (Some(species), Some(heat)) = (&self.species, &self.reaction_heat) {
            let hit = matches!(
                (species.phase, dispersed),
                (PhaseSelector::Dispersed, true) | (PhaseSelector::Continuous, false)
            );
            if hit {
                for c in 0..n {
                    q_rxn[c] += heat.internal[c];
                }
            }
        }
        let m_dot = self.mass_transfer.rate();
        let mut latent = vec![0.0_f64; n];
        // Latent heat is drawn from the continuous phase on evaporation.
        if !dispersed && m_dot != 0.0 && self.latent_heat != 0.0 {
            let sink = -self.latent_heat * m_dot; // [W/m³]
            latent.iter_mut().for_each(|x| *x = sink);
        }
        (q_rxn, latent, m_dot)
    }

    /// Interfacial volumetric heat-transfer coefficient field `K`
    /// `[W/(m³·K)]` = `6·max(α_d,α_res)·κ_c·Nu/d²` (ported `RanzMarshall.C`).
    ///
    /// The Nusselt number uses the configured [`InterfacialHeatTransfer`] model
    /// with the local slip Reynolds number (Ranz-Marshall only) and the
    /// continuous-phase Prandtl number `Pr = μ_c·Cp_c/κ_c`.
    fn interfacial_coefficient(&self) -> Result<VolScalarField, MultiphaseError> {
        let mesh = self.hydro.mesh.clone();
        let n = mesh.n_cells;
        let sys: &TwoFluidSystem = &self.hydro.system;

        let d = sys.dispersed.diameter().get::<uom::si::length::meter>();
        let kappa_c = self.continuous_thermo.kappa;
        let cp_c = self.continuous_thermo.cp;
        let mu_c = sys
            .continuous
            .mu()
            .get::<uom::si::dynamic_viscosity::pascal_second>();
        // Continuous-phase Prandtl number  Pr = μ_c Cp_c / κ_c  [-].
        let pr = if kappa_c > 0.0 {
            mu_c * cp_c / kappa_c
        } else {
            0.0
        };

        // Slip Reynolds-number field (only needed by Ranz-Marshall).
        let re_field = if self.heat_transfer.needs_reynolds() {
            Some(sys.reynolds_number()?)
        } else {
            None
        };

        let alpha_d = sys.dispersed.alpha();
        let d2 = d * d;
        let a_res = self.residual_alpha;
        let vals: Vec<f64> = (0..n)
            .map(|c| {
                let re = re_field.as_ref().map(|f| f.internal[c]).unwrap_or(0.0);
                let nu = self.heat_transfer.nusselt(re, pr);
                6.0 * alpha_d.internal[c].max(a_res) * kappa_c * nu / d2
            })
            .collect();
        Ok(scalar_field("Kht", mesh, vals))
    }

    /// Operator-split interfacial mass transfer: update the volume fractions by
    /// `Δα_d = −ṁ·Δt/ρ_d` (dispersed shrinks on evaporation), clamp to `[0,1]`,
    /// and re-impose `α_c = 1 − α_d`.
    ///
    /// The latent-heat coupling is applied inside the energy predictor (see
    /// [`energy_sources`](Self::energy_sources)); this step moves only the
    /// volume fractions. A no-op when [`mass_transfer`](Self::mass_transfer) is
    /// [`InterfacialMassTransfer::None`].
    ///
    /// # Errors
    /// [`MultiphaseError::InvalidInput`] if `dt ≤ 0`.
    pub fn apply_mass_transfer(&mut self, dt: f64) -> Result<(), MultiphaseError> {
        let m_dot = self.mass_transfer.rate();
        if m_dot == 0.0 {
            return Ok(());
        }
        if dt <= 0.0 {
            return Err(MultiphaseError::InvalidInput(format!(
                "dt must be > 0 (got {dt})"
            )));
        }
        let rho_d = self
            .hydro
            .system
            .dispersed
            .rho()
            .get::<uom::si::mass_density::kilogram_per_cubic_meter>();
        let n = self.hydro.mesh.n_cells;
        let d_alpha = -m_dot * dt / rho_d;
        {
            let alpha_d = self.hydro.system.dispersed.alpha_mut();
            let internal = alpha_d.internal.as_mut_slice();
            for c in 0..n {
                internal[c] = (internal[c] + d_alpha).clamp(0.0, 1.0);
            }
        }
        self.hydro.system.enforce_alpha_constraint();
        Ok(())
    }

    /// Run the transient loop from the control dictionary's start time to its end
    /// time at the fixed `deltaT`, mirroring the other applib solvers' `run`.
    ///
    /// # Errors
    /// - [`AppBuilderError::TimeLimit`] if the stop control is not an end time.
    /// - Wraps any per-step [`MultiphaseError`] as
    ///   [`AppBuilderError::Diverged`].
    pub fn run(&mut self) -> Result<(), AppBuilderError> {
        let start = match self.control.start {
            StartControl::StartTime(t) => t,
            _ => 0.0,
        };
        let end = match self.control.stop {
            StopControl::EndTime(t) => t,
            _ => return Err(AppBuilderError::TimeLimitReached { t: self.time }),
        };
        let dt = self.control.delta_t;
        self.time = start;
        while self.time < end - 1.0e-12 {
            // Map any physical/solver failure to the diverged channel; the
            // MultiphaseError message is preserved via its Display in logs.
            self.solve_timestep(dt)
                .map_err(|_e| AppBuilderError::Diverged {
                    iter: 0,
                    residual: f64::NAN,
                })?;
        }
        Ok(())
    }
}

/// Build a `VolScalarField` from per-cell values with zero-gradient boundaries
/// (matches the multiphase crate's internal helper of the same shape).
fn scalar_field(name: &str, mesh: Arc<FvMesh>, vals: Vec<f64>) -> VolScalarField {
    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField::zero_gradient(p.size))
        .collect();
    VolScalarField::new(name, mesh, Field::new(vals), boundary)
}

/// Convenience constructor: a [`DragModel`]-coupled hydrodynamic core plus the
/// two thermal states, assembled into a ready-to-step solver with default
/// controls. Kept out of `new` so the primary constructor stays I/O-driven.
pub fn build_default(
    system: TwoFluidSystem,
    drag: DragModel,
    dispersed_thermo: PhaseThermo,
    continuous_thermo: PhaseThermo,
) -> Result<ReactingTwoPhaseEulerFoam, MultiphaseError> {
    let hydro = TwoFluidPimple::new(system, drag);
    let control = ControlDict::default();
    let schemes = FvSchemes::default();
    let solution = FvSolution::default();
    ReactingTwoPhaseEulerFoam::new(
        hydro,
        dispersed_thermo,
        continuous_thermo,
        control,
        schemes,
        solution,
    )
}

// ═════════════════════════════════════════════════════════════════════════════
// Tests — verification only (formula exactness, thermal-equilibration limit,
// energy conservation, source-term response). NOT validation: no benchmark or
// experimental comparison here (human V&V step).
// ═════════════════════════════════════════════════════════════════════════════
#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::prelude::{BoundaryPatch, FvMeshBuilder, PatchKind, Vector3};
    use outram_foam_multiphase::two_fluid::Phase;
    use uom::si::dynamic_viscosity::pascal_second;
    use uom::si::f64::{DynamicViscosity, Length, MassDensity};
    use uom::si::length::meter;
    use uom::si::mass_density::kilogram_per_cubic_meter;

    fn vx(x: f64) -> Vector3 {
        Vector3::new(x, 0.0, 0.0)
    }

    /// Four-cell 1-D line mesh (mirrors the multiphase crate's `line_mesh_4`).
    fn mesh() -> Arc<FvMesh> {
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(4)
                .n_internal_faces(3)
                .owner(vec![0, 1, 2, 0, 3])
                .neighbour(vec![1, 2, 3])
                .patches(vec![
                    BoundaryPatch::new("inlet", 3, 1, PatchKind::Patch),
                    BoundaryPatch::new("outlet", 4, 1, PatchKind::Patch),
                ])
                .cell_volumes(vec![1.0, 1.0, 1.0, 1.0])
                .cell_centres(vec![vx(0.5), vx(1.5), vx(2.5), vx(3.5)])
                .face_area_vectors(vec![vx(1.0), vx(1.0), vx(1.0), vx(-1.0), vx(1.0)])
                .face_centres(vec![vx(1.0), vx(2.0), vx(3.0), vx(0.0), vx(4.0)])
                .build()
                .unwrap(),
        )
    }

    fn phase(m: &Arc<FvMesh>, name: &str, rho: f64, mu: f64, d: f64, alpha0: f64) -> Phase {
        Phase::new(
            m.clone(),
            name,
            MassDensity::new::<kilogram_per_cubic_meter>(rho),
            DynamicViscosity::new::<pascal_second>(mu),
            Length::new::<meter>(d),
            alpha0,
        )
        .unwrap()
    }

    fn thermo(m: &Arc<FvMesh>, name: &str, cp: f64, kappa: f64, t0: f64) -> PhaseThermo {
        PhaseThermo::new(
            m.clone(),
            name,
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(cp),
            ThermalConductivity::new::<watt_per_meter_kelvin>(kappa),
            ThermodynamicTemperature::new::<kelvin>(t0),
        )
        .unwrap()
    }

    /// Ranz-Marshall Nusselt number matches the hand-computed value
    /// `Nu = 2 + 0.6·√Re·∛Pr`. Re=100, Pr=0.7 → 2 + 0.6·10·0.8879 = 7.3275.
    #[test]
    fn ranz_marshall_nusselt_hand_value() {
        let m = InterfacialHeatTransfer::RanzMarshall;
        let nu = m.nusselt(100.0, 0.7);
        let expect = 2.0 + 0.6 * 10.0 * 0.7_f64.cbrt();
        assert!((nu - expect).abs() < 1e-12, "nu={nu} expect={expect}");
        // Spherical is the Re→0 limit.
        assert_eq!(InterfacialHeatTransfer::Spherical.nusselt(0.0, 0.7), 2.0);
        assert_eq!(
            InterfacialHeatTransfer::ConstantNu(6.0).nusselt(50.0, 1.0),
            6.0
        );
    }

    /// Constructor rejects a mesh mismatch between hydro and thermal states.
    #[test]
    fn rejects_mesh_mismatch() {
        let m1 = mesh();
        let m2 = mesh();
        let sys = TwoFluidSystem::new(
            phase(&m1, "air", 1.2, 1.8e-5, 1e-3, 0.3),
            phase(&m1, "water", 1000.0, 1e-3, 1e-3, 0.7),
        )
        .unwrap();
        let hydro = TwoFluidPimple::new(sys, DragModel::SchillerNaumann);
        let td = thermo(&m1, "he.air", 1005.0, 0.025, 320.0);
        let tc = thermo(&m2, "he.water", 4182.0, 0.6, 300.0); // wrong mesh
        let err = ReactingTwoPhaseEulerFoam::new(
            hydro,
            td,
            tc,
            ControlDict::default(),
            FvSchemes::default(),
            FvSolution::default(),
        );
        assert!(err.is_err());
    }

    /// With no flow, the two phases exchange heat only through the interfacial
    /// coefficient and must relax toward the shared mass-heat-weighted mean
    /// temperature, conserving total thermal energy.
    ///
    /// Methodology: air (α=0.5, hot 400 K) + water (α=0.5, cold 300 K), U=0, no
    /// reaction / phase change, Spherical Nu, many small steps. Pass criteria:
    /// (a) |T_d − T_c| shrinks below 1 K; (b) both approach the analytic
    /// equilibrium `T_eq = Σ(mᵢCpᵢTᵢ)/Σ(mᵢCpᵢ)`; (c) total thermal energy
    /// `Σ mᵢCpᵢTᵢ` conserved to < 1 %.
    #[test]
    fn thermal_equilibration_conserves_energy() {
        let m = mesh();
        let rho_d = 1.2;
        let rho_c = 1000.0;
        let cp_d = 1005.0;
        let cp_c = 4182.0;
        let (t_d0, t_c0) = (400.0, 300.0);
        let alpha_d = 0.5;

        let sys = TwoFluidSystem::new(
            phase(&m, "air", rho_d, 1.8e-5, 1e-3, alpha_d),
            phase(&m, "water", rho_c, 1e-3, 1e-3, 1.0 - alpha_d),
        )
        .unwrap();
        let mut solver = build_default(
            sys,
            DragModel::SchillerNaumann,
            thermo(&m, "he.air", cp_d, 0.025, t_d0),
            thermo(&m, "he.water", cp_c, 0.6, t_c0),
        )
        .unwrap();
        solver.heat_transfer = InterfacialHeatTransfer::Spherical;
        solver.n_energy_correctors = 2;

        // Mass-heat weights (α constant here since U=0, no phase change).
        let w_d = alpha_d * rho_d * cp_d;
        let w_c = (1.0 - alpha_d) * rho_c * cp_c;
        let t_eq = (w_d * t_d0 + w_c * t_c0) / (w_d + w_c);
        let e0 = w_d * t_d0 + w_c * t_c0;

        for _ in 0..400 {
            solver.solve_timestep(1.0e-3).unwrap();
        }

        let td = solver.dispersed_thermo.mean_temperature();
        let tc = solver.continuous_thermo.mean_temperature();
        let e1 = w_d * td + w_c * tc;

        assert!(
            (td - tc).abs() < 1.0,
            "phases not equilibrated: Td={td} Tc={tc}"
        );
        assert!(
            (td - t_eq).abs() < 2.0 && (tc - t_eq).abs() < 2.0,
            "not at equilibrium Teq={t_eq}: Td={td} Tc={tc}"
        );
        let rel_e = (e1 - e0).abs() / e0.abs();
        assert!(rel_e < 1.0e-2, "energy not conserved: rel err {rel_e:.3e}");
        // Water (huge heat capacity) barely moves; equilibrium sits near 300 K.
        assert!(t_eq < 301.0, "weighted equilibrium should sit near water T");
    }

    /// A reaction heat source deposited in the dispersed phase raises its
    /// temperature; with no interfacial transfer it is a closed heat-up whose
    /// rise matches `ΔT = q̇·Δt·N / (α_d ρ_d Cp_d)`.
    #[test]
    fn reaction_heat_raises_phase_temperature() {
        let m = mesh();
        let (rho_d, cp_d, alpha_d) = (2.0, 500.0, 0.4);
        let sys = TwoFluidSystem::new(
            phase(&m, "fuel", rho_d, 2e-5, 1e-3, alpha_d),
            phase(&m, "gas", 1.0, 2e-5, 1e-3, 1.0 - alpha_d),
        )
        .unwrap();
        let mut solver = build_default(
            sys,
            DragModel::SchillerNaumann,
            thermo(&m, "he.fuel", cp_d, 0.02, 300.0),
            thermo(&m, "he.gas", 1000.0, 0.02, 300.0),
        )
        .unwrap();
        // Kill interfacial exchange so the heat-up is closed and analytic:
        // ConstantNu(0) → K = 0.
        solver.heat_transfer = InterfacialHeatTransfer::ConstantNu(0.0);
        let q = 1.0e5; // W/m³
        solver.reaction = ReactionSource::VolumetricHeat {
            phase: PhaseSelector::Dispersed,
            q,
        };

        let t0 = solver.dispersed_thermo.mean_temperature();
        let dt = 1.0e-3;
        let steps = 50;
        for _ in 0..steps {
            solver.solve_timestep(dt).unwrap();
        }
        let t1 = solver.dispersed_thermo.mean_temperature();
        let expect = q * dt * steps as f64 / (alpha_d * rho_d * cp_d);
        let got = t1 - t0;
        assert!(
            (got - expect).abs() / expect < 5.0e-2,
            "reaction heat-up off: got {got:.4} expect {expect:.4}"
        );
        // Continuous phase (no source, K=0) is untouched to within the linear
        // solver's tolerance (~1e-7 relative on he ⇒ ~1e-4 K), i.e. ≪ the ~12.5 K
        // dispersed rise — a clean separation, not physical heating.
        assert!(
            (solver.continuous_thermo.mean_temperature() - 300.0).abs() < 1e-2,
            "continuous phase should not heat (got {})",
            solver.continuous_thermo.mean_temperature()
        );
    }

    /// A constant-rate phase change (dispersed → continuous) reduces the
    /// dispersed volume fraction at the analytic rate `Δα_d = −ṁ·Δt/ρ_d` and
    /// keeps `α_d + α_c = 1`.
    #[test]
    fn mass_transfer_moves_volume_fraction() {
        let m = mesh();
        let (rho_d, alpha_d0) = (2.0, 0.6);
        let sys = TwoFluidSystem::new(
            phase(&m, "drops", rho_d, 2e-5, 1e-3, alpha_d0),
            phase(&m, "vapour", 1.0, 2e-5, 1e-3, 1.0 - alpha_d0),
        )
        .unwrap();
        let mut solver = build_default(
            sys,
            DragModel::SchillerNaumann,
            thermo(&m, "he.drops", 4000.0, 0.6, 350.0),
            thermo(&m, "he.vapour", 2000.0, 0.02, 350.0),
        )
        .unwrap();
        let m_dot = 4.0; // kg/(m³·s), dispersed → continuous
        solver.mass_transfer = InterfacialMassTransfer::ConstantRate(m_dot);
        solver.latent_heat = 2.26e6; // J/kg (order of water); exercises the sink

        let dt = 1.0e-3;
        let steps = 20;
        for _ in 0..steps {
            solver.solve_timestep(dt).unwrap();
        }
        // Mean α_d over the domain.
        let ad = {
            let a = solver.hydro.system.dispersed.alpha();
            let n = m.n_cells;
            (0..n).map(|c| a.internal[c]).sum::<f64>() / n as f64
        };
        let ac = {
            let a = solver.hydro.system.continuous.alpha();
            let n = m.n_cells;
            (0..n).map(|c| a.internal[c]).sum::<f64>() / n as f64
        };
        let expect = alpha_d0 - m_dot * dt * steps as f64 / rho_d;
        assert!((ad - expect).abs() < 1e-9, "alpha_d off: {ad} vs {expect}");
        assert!(
            (ad + ac - 1.0).abs() < 1e-12,
            "saturation broken: {}",
            ad + ac
        );
    }

    /// Helper: a single-phase continuous gas carrying an N-species composition
    /// on the shared mesh, with a matching two-fluid system for the hydro core.
    fn reacting_gas_solver(
        m: &Arc<FvMesh>,
        names: &[&str],
        y0: &[f64],
        t0: f64,
    ) -> ReactingTwoPhaseEulerFoam {
        let sys = TwoFluidSystem::new(
            phase(m, "particles", 2.0, 2e-5, 1e-3, 0.05),
            phase(m, "gas", 1.0, 2e-5, 1e-3, 0.95),
        )
        .unwrap();
        let mut solver = build_default(
            sys,
            DragModel::SchillerNaumann,
            thermo(m, "he.particles", 800.0, 0.02, t0),
            thermo(m, "he.gas", 1200.0, 0.025, t0),
        )
        .unwrap();
        // No interfacial heat exchange, so a gas-phase reaction heats only the gas.
        solver.heat_transfer = InterfacialHeatTransfer::ConstantNu(0.0);
        solver.species = Some(
            PhaseSpecies::new(
                m.clone(),
                PhaseSelector::Continuous,
                names.iter().map(|s| s.to_string()).collect(),
                y0,
                1.0e-5,
            )
            .unwrap(),
        );
        solver
    }

    /// Inert species (no reaction mechanism) are transported conservatively:
    /// with `U = 0` and uniform initial fields, every `Y_i` is unchanged and
    /// `Σ_i Y_i = 1` is preserved exactly.
    #[test]
    fn inert_species_conserve_sum() {
        let m = mesh();
        let mut solver = reacting_gas_solver(&m, &["A", "B", "C"], &[0.5, 0.3, 0.2], 320.0);
        // ReactionMechanism::None by default.
        for _ in 0..25 {
            solver.solve_timestep(1.0e-3).unwrap();
        }
        let sp = solver.species.as_ref().unwrap();
        for c in 0..m.n_cells {
            let sum: f64 = sp.y.iter().map(|y| y.internal[c]).sum();
            assert!((sum - 1.0).abs() < 1e-9, "sum Y != 1 in cell {c}: {sum}");
        }
        assert!((sp.mean_mass_fraction(0) - 0.5).abs() < 1e-9);
        assert!((sp.mean_mass_fraction(1) - 0.3).abs() < 1e-9);
    }

    /// A single global Arrhenius reaction `fuel → product` in the gas phase
    /// consumes fuel, produces product (conserving `ΣY = 1`), and the heat
    /// release raises the gas temperature.
    ///
    /// Methodology: gas at 1500 K (hot enough that `exp(−Ea/RT)` is
    /// appreciable), `Y = [fuel 0.8, product 0.2]`, `A = 1e4 /s`, `Ea = 5e4
    /// J/mol`, `ΔH = 3e6 J/kg`. Pass criteria: fuel monotonically down, product
    /// up by the same amount, `ΣY = 1` held, gas `T` strictly increases, and the
    /// inert particulate phase (no source, `K = 0`) is untouched.
    #[test]
    fn arrhenius_reaction_burns_fuel_and_heats_gas() {
        let m = mesh();
        let t0 = 1500.0;
        let mut solver = reacting_gas_solver(&m, &["fuel", "product"], &[0.8, 0.2], t0);
        solver.reaction_mechanism = ReactionMechanism::Arrhenius {
            fuel: 0,
            product: 1,
            a_pre: 1.0e4,
            e_act: 5.0e4,
            delta_h: 3.0e6,
        };

        let fuel0 = solver.species.as_ref().unwrap().mean_mass_fraction(0);
        let gas_t0 = solver.continuous_thermo.mean_temperature();
        let part_t0 = solver.dispersed_thermo.mean_temperature();

        for _ in 0..50 {
            solver.solve_timestep(1.0e-3).unwrap();
        }

        let sp = solver.species.as_ref().unwrap();
        let fuel1 = sp.mean_mass_fraction(0);
        let prod1 = sp.mean_mass_fraction(1);
        let gas_t1 = solver.continuous_thermo.mean_temperature();
        let part_t1 = solver.dispersed_thermo.mean_temperature();

        assert!(
            fuel1 < fuel0 - 1e-4,
            "fuel not consumed: {fuel0} -> {fuel1}"
        );
        assert!(prod1 > 0.2 + 1e-4, "product not formed: {prod1}");
        for c in 0..m.n_cells {
            let sum: f64 = sp.y.iter().map(|y| y.internal[c]).sum();
            assert!((sum - 1.0).abs() < 1e-9, "sum Y != 1: {sum}");
        }
        assert!(
            gas_t1 > gas_t0 + 1.0,
            "gas did not heat: {gas_t0} -> {gas_t1}"
        );
        // Particulate phase carries no composition and K=0 → untouched.
        assert!(
            (part_t1 - part_t0).abs() < 1e-2,
            "inert phase heated: {part_t0} -> {part_t1}"
        );
    }
}
