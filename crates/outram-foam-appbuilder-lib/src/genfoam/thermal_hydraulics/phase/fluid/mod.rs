// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/phaseModels/fluid/fluid.{C,H}
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `fluid` — the fluid-phase field-state container
//!
//! Port of GeN-Foam's `Foam::fluid`. Upstream describes this class as one that
//! *"mainly acts as a variable placeholder, as little functionality is deferred
//! to the class itself"* — it is the **per-cell field-state bag** the porous
//! momentum / energy / pressure solvers and every closure correlation read from
//! and write to. This Rust port keeps exactly that role: [`Fluid`] owns the
//! phase fields and exposes them; the physics (thermo, turbulence, drag, heat
//! transfer) lives in the `thermophysical`, `turbulence`, and `closures`
//! modules and updates these fields by reference.
//!
//! ## What it owns
//!
//! - **Kinematics** — velocity `U` and its magnitude `magU`; the superficial
//!   volumetric flux `phi`, the phase volumetric flux `alphaPhi`, the phase mass
//!   flux `alphaRhoPhi`, and the cell-centred mass flux `alphaRhoMagU`; the
//!   dilation term `dgdt`; and the solver-normalised volume fraction
//!   `normalized`.
//! - **Thermophysics placeholders** — density `rho`, temperature `T`, specific
//!   enthalpy `h`, thermal conductivity `kappa`, specific heat `Cp`, dynamic
//!   viscosity `mu`, Prandtl number `Pr`. As upstream, these are *placeholders*
//!   the thermo package recomputes and writes into each step (see the field-unit
//!   table below).
//! - **Geometry** — the characteristic (hydraulic) diameter `Dh`.
//! - **Two-phase quantities** — flow quality `X`, Lockhart–Martinelli parameter
//!   `XLM`, and the dispersion marker; trivial (single-phase) unless a two-phase
//!   solver drives them.
//! - **Coupling / diagnostics** — the neutronics-to-fluid power density, the
//!   continuity error and its time-integral, and the `thermoResidualAlpha`
//!   markers that switch the energy equation off in near-empty cells.
//!
//! ## Field units (basic-lib fields are bare `f64`, unitful by convention)
//!
//! [`outram_foam_basic_lib`]'s [`VolScalarField`] / [`VolVectorField`] /
//! [`SurfaceScalarField`] store plain `f64` per cell (or face), so each field's
//! physical unit is a documented convention, matching the precedent set by
//! `genfoam::neutronics`'s state. Scalar summaries that cross the public API
//! ([`Fluid::thermo_residual_alpha`], [`Fluid::cumulative_continuity_error`])
//! are returned as named [`uom`] types.
//!
//! | Field | Symbol | Unit |
//! |---|---|---|
//! | [`Fluid::alpha`] | `alpha` | dimensionless (`0..=1`) |
//! | [`Fluid::velocity`] | `U` | `m/s` (vector) |
//! | [`Fluid::mag_velocity`] | `\|U\|` | `m/s` |
//! | [`Fluid::phi`] | `phi` | `m^3/s` (superficial volumetric face flux) |
//! | [`Fluid::alpha_phi`] | `alphaPhi` | `m^3/s` (phase volumetric face flux) |
//! | [`Fluid::alpha_rho_phi`] | `alphaRhoPhi` | `kg/s` (phase mass face flux) |
//! | [`Fluid::alpha_rho_mag_u`] | `alphaRhoMagU` | `kg/(m^2 s)` (cell mass flux) |
//! | [`Fluid::dgdt`] | `dgdt` | `1/s` (dilation) |
//! | [`Fluid::normalized`] | `alpha_norm` | dimensionless |
//! | [`Fluid::density`] | `rho` | `kg/m^3` |
//! | [`Fluid::temperature`] | `T` | `K` |
//! | [`Fluid::enthalpy`] | `h` | `J/kg` (specific enthalpy) |
//! | [`Fluid::kappa`] | `kappa` | `W/(m K)` (thermal conductivity) |
//! | [`Fluid::cp`] | `Cp` | `J/(kg K)` (specific heat) |
//! | [`Fluid::mu`] | `mu` | `Pa s` (dynamic viscosity) |
//! | [`Fluid::prandtl`] | `Pr` | dimensionless |
//! | [`Fluid::hydraulic_diameter`] | `Dh` | `m` |
//! | [`Fluid::power_density`] | `q'''` | `W/m^3` |
//! | [`Fluid::continuity_error`] | `contErr` | `kg/(m^3 s)` |
//! | [`Fluid::flow_quality`] | `X` | dimensionless |
//! | [`Fluid::lockhart_martinelli`] | `XLM` | dimensionless |
//! | [`Fluid::dispersion`] | — | dimensionless (`0` continuous … `1` dispersed) |

use std::sync::Arc;

use outram_foam_basic_lib::fields::surface_field::SurfaceScalarField;
use outram_foam_basic_lib::fields::vol_field::{VolScalarField, VolVectorField};
use outram_foam_basic_lib::mesh::fv_mesh::FvMesh;
use uom::si::f64::Mass;
use uom::si::mass::kilogram;

use super::phase_base::{PhaseBase, VolumeFraction};

#[cfg(test)]
mod tests;

/// Generate paired `field()` / `field_mut()` accessors with shared doc text.
///
/// Each entry `get, get_mut: Ty;` expands to an immutable `get(&self) -> &Ty`
/// borrowing `self.get` and a mutable `get_mut(&mut self) -> &mut Ty` borrowing
/// the same field. The macro keeps the ~24 placeholder-field accessors terse
/// and identical in form without repeating the boilerplate two dozen times.
macro_rules! accessors {
    ($($(#[$m:meta])* $get:ident, $get_mut:ident : $ty:ty;)+) => {
        $(
            $(#[$m])*
            #[must_use]
            pub fn $get(&self) -> &$ty {
                &self.$get
            }
            $(#[$m])*
            ///
            /// Mutable accessor (the physics packages write through it).
            pub fn $get_mut(&mut self) -> &mut $ty {
                &mut self.$get
            }
        )+
    };
}

/// The physical state of matter of a [`Fluid`] phase.
///
/// Closed-enum port of GeN-Foam's `fluid::stateOfMatter`. Dispatched by value
/// (no trait objects) via [`Self::is_liquid`] / [`Self::is_gas`]; several
/// closures branch on it (e.g. boiling models only fire for a `Gas` companion
/// to a `Liquid`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateOfMatter {
    /// Not classified — the single-phase default (`onePhase` runs).
    #[default]
    Undetermined,
    /// Liquid phase.
    Liquid,
    /// Gas / vapour phase.
    Gas,
}

impl StateOfMatter {
    /// `true` if this phase is explicitly the [`StateOfMatter::Liquid`].
    #[must_use]
    pub fn is_liquid(self) -> bool {
        self == StateOfMatter::Liquid
    }

    /// `true` if this phase is explicitly the [`StateOfMatter::Gas`].
    #[must_use]
    pub fn is_gas(self) -> bool {
        self == StateOfMatter::Gas
    }

    /// The upstream dictionary keyword for this state
    /// (`"undetermined"` / `"liquid"` / `"gas"`).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            StateOfMatter::Undetermined => "undetermined",
            StateOfMatter::Liquid => "liquid",
            StateOfMatter::Gas => "gas",
        }
    }
}

/// A generic fluid phase: the per-cell field-state container the porous
/// thermal-hydraulics solvers and closures operate on.
///
/// Construct with [`Fluid::new`], which allocates every field on the shared
/// mesh with GeN-Foam's initial values (`alpha = 0`, `U = 0`, `Dh = SMALL`,
/// `XLM = 1e4`, markers = 0). A solver or thermo package then overwrites the
/// fields as it iterates; the `*_mut` accessors expose them for that.
///
/// All fields are defined on one shared [`FvMesh`] (`Arc`), reachable through
/// [`Fluid::mesh`]. The phase volume fraction and name live in the embedded
/// [`PhaseBase`].
#[derive(Debug, Clone)]
pub struct Fluid {
    /// Shared per-phase base (volume fraction `alpha`, name, residual alpha).
    base: PhaseBase,
    /// Liquid / gas / undetermined classification.
    state_of_matter: StateOfMatter,

    // ── Kinematics ───────────────────────────────────────────────────────
    velocity: VolVectorField,
    mag_velocity: VolScalarField,
    phi: SurfaceScalarField,
    alpha_phi: SurfaceScalarField,
    alpha_rho_phi: SurfaceScalarField,
    alpha_rho_mag_u: VolScalarField,
    dgdt: VolScalarField,
    normalized: VolScalarField,

    // ── Thermophysics placeholders (written by the thermo package) ───────
    density: VolScalarField,
    temperature: VolScalarField,
    enthalpy: VolScalarField,
    kappa: VolScalarField,
    cp: VolScalarField,
    mu: VolScalarField,
    prandtl: VolScalarField,

    // ── Geometry ─────────────────────────────────────────────────────────
    hydraulic_diameter: VolScalarField,

    // ── Two-phase quantities ─────────────────────────────────────────────
    flow_quality: VolScalarField,
    lockhart_martinelli: VolScalarField,
    dispersion: VolScalarField,
    min_xlm: f64,
    max_xlm: f64,

    // ── Coupling / diagnostics ───────────────────────────────────────────
    power_density: VolScalarField,
    continuity_error: VolScalarField,
    cumulative_continuity_error: f64,

    // ── Thermo-residual switching ────────────────────────────────────────
    thermo_residual_alpha: VolumeFraction,
    above_thermo_residual_alpha: VolScalarField,
    below_thermo_residual_alpha: VolScalarField,

    // ── Equation of state ────────────────────────────────────────────────
    boussinesq: bool,
    reference_density: VolScalarField,
}

impl Fluid {
    /// Small positive floor for the hydraulic diameter, matching GeN-Foam's
    /// initialisation `Dh = SMALL`.
    const SMALL: f64 = 1.0e-15;

    /// Allocate a fluid phase named `name` on `mesh` with the given
    /// [`StateOfMatter`] and GeN-Foam's default initial field values.
    ///
    /// All scalar/vector cell fields start at zero except the hydraulic
    /// diameter (`Dh = SMALL`) and the Lockhart–Martinelli parameter
    /// (`XLM = 1e4`, upstream's large-value seed); `minXLM = 1e-4`,
    /// `maxXLM = 1/minXLM`; `thermoResidualAlpha = 0`; `Boussinesq = false`.
    /// Set the initial volume fraction with [`Fluid::alpha_mut`].
    #[must_use]
    pub fn new(mesh: Arc<FvMesh>, name: impl Into<String>, state_of_matter: StateOfMatter) -> Self {
        let base = PhaseBase::new(mesh.clone(), name);
        let n = base.name().to_string();
        let min_xlm = 1.0e-4;
        Self {
            velocity: VolVectorField::zero(format!("U.{n}"), mesh.clone()),
            mag_velocity: VolScalarField::zeros(format!("magU.{n}"), mesh.clone()),
            phi: SurfaceScalarField::zeros(format!("phi.{n}"), mesh.clone()),
            alpha_phi: SurfaceScalarField::zeros(format!("alphaPhi.{n}"), mesh.clone()),
            alpha_rho_phi: SurfaceScalarField::zeros(format!("alphaRhoPhi.{n}"), mesh.clone()),
            alpha_rho_mag_u: VolScalarField::zeros(format!("alphaRhoMagU.{n}"), mesh.clone()),
            dgdt: VolScalarField::zeros(format!("dgdt.{n}"), mesh.clone()),
            normalized: VolScalarField::zeros(format!("normalized.alpha.{n}"), mesh.clone()),
            density: VolScalarField::zeros(format!("rho.{n}"), mesh.clone()),
            temperature: VolScalarField::zeros(format!("T.{n}"), mesh.clone()),
            enthalpy: VolScalarField::zeros(format!("h.{n}"), mesh.clone()),
            kappa: VolScalarField::zeros(format!("kappa.{n}"), mesh.clone()),
            cp: VolScalarField::zeros(format!("Cp.{n}"), mesh.clone()),
            mu: VolScalarField::zeros(format!("mu.{n}"), mesh.clone()),
            prandtl: VolScalarField::zeros(format!("Pr.{n}"), mesh.clone()),
            hydraulic_diameter: VolScalarField::uniform(
                format!("Dh.{n}"),
                mesh.clone(),
                Self::SMALL,
            ),
            flow_quality: VolScalarField::zeros(format!("flowQuality.{n}"), mesh.clone()),
            lockhart_martinelli: VolScalarField::uniform(format!("XLM.{n}"), mesh.clone(), 1.0e4),
            dispersion: VolScalarField::zeros(format!("dispersion.{n}"), mesh.clone()),
            min_xlm,
            max_xlm: 1.0 / min_xlm,
            power_density: VolScalarField::zeros(format!("powerDensity.{n}"), mesh.clone()),
            continuity_error: VolScalarField::zeros(format!("contErr.{n}"), mesh.clone()),
            cumulative_continuity_error: 0.0,
            thermo_residual_alpha: super::phase_base::volume_fraction(0.0),
            above_thermo_residual_alpha: VolScalarField::zeros(
                format!("aboveThermoResidualAlpha.{n}"),
                mesh.clone(),
            ),
            below_thermo_residual_alpha: VolScalarField::uniform(
                format!("belowThermoResidualAlpha.{n}"),
                mesh.clone(),
                1.0,
            ),
            boussinesq: false,
            reference_density: VolScalarField::zeros(format!("rho0.{n}"), mesh),
            base,
            state_of_matter,
        }
    }

    // ── Base forwarding ──────────────────────────────────────────────────

    /// The phase name.
    #[must_use]
    pub fn name(&self) -> &str {
        self.base.name()
    }

    /// The mesh every field of this phase is defined on.
    #[must_use]
    pub fn mesh(&self) -> &Arc<FvMesh> {
        self.base.mesh()
    }

    /// The embedded [`PhaseBase`] (volume fraction, name, residual alpha).
    #[must_use]
    pub fn base(&self) -> &PhaseBase {
        &self.base
    }

    /// Mutable access to the embedded [`PhaseBase`].
    pub fn base_mut(&mut self) -> &mut PhaseBase {
        &mut self.base
    }

    /// The phase volume-fraction field `alpha` (dimensionless, `0..=1`).
    #[must_use]
    pub fn alpha(&self) -> &VolScalarField {
        self.base.alpha()
    }

    /// Mutable access to the phase volume-fraction field `alpha`.
    pub fn alpha_mut(&mut self) -> &mut VolScalarField {
        self.base.alpha_mut()
    }

    /// This phase's state of matter (liquid / gas / undetermined).
    #[must_use]
    pub fn state_of_matter(&self) -> StateOfMatter {
        self.state_of_matter
    }

    /// `true` if this phase is the liquid; forwards to [`StateOfMatter::is_liquid`].
    #[must_use]
    pub fn is_liquid(&self) -> bool {
        self.state_of_matter.is_liquid()
    }

    /// `true` if this phase is the gas; forwards to [`StateOfMatter::is_gas`].
    #[must_use]
    pub fn is_gas(&self) -> bool {
        self.state_of_matter.is_gas()
    }

    // ── Field accessors (const + mut) ────────────────────────────────────

    accessors! {
        /// Velocity `U` (`m/s`, vector).
        velocity, velocity_mut: VolVectorField;
        /// Velocity magnitude `|U|` (`m/s`).
        mag_velocity, mag_velocity_mut: VolScalarField;
        /// Superficial volumetric face flux `phi` (`m^3/s`).
        phi, phi_mut: SurfaceScalarField;
        /// Phase volumetric face flux `alphaPhi` (`m^3/s`).
        alpha_phi, alpha_phi_mut: SurfaceScalarField;
        /// Phase mass face flux `alphaRhoPhi` (`kg/s`).
        alpha_rho_phi, alpha_rho_phi_mut: SurfaceScalarField;
        /// Cell-centred mass flux `alphaRhoMagU` (`kg/(m^2 s)`); regime-map placeholder.
        alpha_rho_mag_u, alpha_rho_mag_u_mut: VolScalarField;
        /// Dilation term `dgdt` (`1/s`) — the compressible part of continuity.
        dgdt, dgdt_mut: VolScalarField;
        /// Solver-normalised volume fraction (dimensionless).
        normalized, normalized_mut: VolScalarField;
        /// Density `rho` (`kg/m^3`). Placeholder updated by the thermo package.
        density, density_mut: VolScalarField;
        /// Temperature `T` (`K`). Placeholder updated by the thermo package.
        temperature, temperature_mut: VolScalarField;
        /// Specific enthalpy `h` (`J/kg`). Placeholder updated by the thermo package.
        enthalpy, enthalpy_mut: VolScalarField;
        /// Thermal conductivity `kappa` (`W/(m K)`). Placeholder updated by thermo.
        kappa, kappa_mut: VolScalarField;
        /// Specific heat `Cp` (`J/(kg K)`). Placeholder updated by thermo.
        cp, cp_mut: VolScalarField;
        /// Dynamic viscosity `mu` (`Pa s`). Placeholder updated by thermo.
        mu, mu_mut: VolScalarField;
        /// Prandtl number `Pr` (dimensionless). Placeholder updated by thermo.
        prandtl, prandtl_mut: VolScalarField;
        /// Characteristic (hydraulic) diameter `Dh` (`m`).
        hydraulic_diameter, hydraulic_diameter_mut: VolScalarField;
        /// Flow quality `X` (dimensionless); non-trivial only in two-phase.
        flow_quality, flow_quality_mut: VolScalarField;
        /// Lockhart–Martinelli parameter `XLM` (dimensionless).
        lockhart_martinelli, lockhart_martinelli_mut: VolScalarField;
        /// Dispersion marker (`0` continuous … `1` dispersed), dimensionless.
        dispersion, dispersion_mut: VolScalarField;
        /// Neutronics-projected power density `q'''` (`W/m^3`) deposited in the fluid.
        power_density, power_density_mut: VolScalarField;
        /// Continuity error `contErr` (`kg/(m^3 s)`).
        continuity_error, continuity_error_mut: VolScalarField;
        /// Marker: `1` where `alpha_norm > thermoResidualAlpha`, else `0`.
        above_thermo_residual_alpha, above_thermo_residual_alpha_mut: VolScalarField;
        /// Marker: `1` where `alpha_norm <= thermoResidualAlpha`, else `0`.
        below_thermo_residual_alpha, below_thermo_residual_alpha_mut: VolScalarField;
        /// Reference density `rho0` (`kg/m^3`), used only under the Boussinesq EOS.
        reference_density, reference_density_mut: VolScalarField;
    }

    /// Minimum allowable Lockhart–Martinelli parameter (dimensionless).
    #[must_use]
    pub fn min_xlm(&self) -> f64 {
        self.min_xlm
    }

    /// Maximum allowable Lockhart–Martinelli parameter (dimensionless).
    #[must_use]
    pub fn max_xlm(&self) -> f64 {
        self.max_xlm
    }

    /// The `thermoResidualAlpha` threshold (dimensionless). Below it the fluid
    /// energy equation is skipped and the phase temperature is pinned to the
    /// interfacial temperature.
    #[must_use]
    pub fn thermo_residual_alpha(&self) -> VolumeFraction {
        self.thermo_residual_alpha
    }

    /// Overwrite the `thermoResidualAlpha` threshold.
    pub fn set_thermo_residual_alpha(&mut self, value: VolumeFraction) {
        self.thermo_residual_alpha = value;
    }

    /// Whether this phase uses the Boussinesq equation of state.
    #[must_use]
    pub fn is_boussinesq(&self) -> bool {
        self.boussinesq
    }

    /// Set the Boussinesq flag (set by the thermo package after reading the EOS).
    pub fn set_boussinesq(&mut self, value: bool) {
        self.boussinesq = value;
    }

    /// The Boussinesq-sensitive density.
    ///
    /// Mirrors GeN-Foam's `fluid::rho(isVariableIfBoussinesq)`: under the
    /// Boussinesq EOS the momentum/pressure equations use the constant
    /// [`reference_density`](Self::reference_density) unless the *variable*
    /// density is explicitly requested; otherwise the thermodynamic
    /// [`density`](Self::density) is used.
    #[must_use]
    pub fn rho(&self, variable_if_boussinesq: bool) -> &VolScalarField {
        if self.boussinesq && !variable_if_boussinesq {
            &self.reference_density
        } else {
            &self.density
        }
    }

    /// Domain-integrated continuity error `cumulContErr` (`kg`).
    #[must_use]
    pub fn cumulative_continuity_error(&self) -> Mass {
        Mass::new::<kilogram>(self.cumulative_continuity_error)
    }

    /// Overwrite the integrated continuity error.
    pub fn set_cumulative_continuity_error(&mut self, value: Mass) {
        self.cumulative_continuity_error = value.get::<kilogram>();
    }

    // ── `correct*` updates (pure per-cell field arithmetic) ──────────────

    /// Recompute the velocity magnitude field: `magU = |U|` per cell.
    ///
    /// Port of GeN-Foam's `magU_ = mag(U_)`.
    pub fn correct_mag_velocity(&mut self) {
        let u = self.velocity.internal.as_slice();
        let m = self.mag_velocity.internal.as_mut_slice();
        for (mi, ui) in m.iter_mut().zip(u) {
            *mi = ui.mag();
        }
    }

    /// Recompute the cell-centred mass flux `alphaRhoMagU = alpha * rho * magU`.
    ///
    /// Port of GeN-Foam's `fluid::correctAlphaRhoMagU`. Uses the thermodynamic
    /// (variable) density, matching upstream's `this->rho()` default there.
    pub fn correct_alpha_rho_mag_u(&mut self) {
        let alpha = self.base.alpha().internal.as_slice();
        let rho = self.density.internal.as_slice();
        let mag_u = self.mag_velocity.internal.as_slice();
        let out = self.alpha_rho_mag_u.internal.as_mut_slice();
        for (i, o) in out.iter_mut().enumerate() {
            *o = alpha[i] * rho[i] * mag_u[i];
        }
    }

    /// Recompute the thermo-residual marker fields from `normalized` and the
    /// `thermoResidualAlpha` threshold.
    ///
    /// Port of GeN-Foam's `fluid::correctThermoResidualMarkers`:
    /// `above = pos(alpha_norm - thermoResidualAlpha)` (1 where strictly above,
    /// else 0) and `below = neg0(alpha_norm - thermoResidualAlpha)` (1 where at
    /// or below, else 0). The two markers partition every cell.
    pub fn correct_thermo_residual_markers(&mut self) {
        let tra = self.thermo_residual_alpha.get::<uom::si::ratio::ratio>();
        let norm = self.normalized.internal.as_slice();
        let above = self.above_thermo_residual_alpha.internal.as_mut_slice();
        let below = self.below_thermo_residual_alpha.internal.as_mut_slice();
        for i in 0..norm.len() {
            let d = norm[i] - tra;
            above[i] = if d > 0.0 { 1.0 } else { 0.0 };
            below[i] = if d <= 0.0 { 1.0 } else { 0.0 };
        }
    }

    /// Kinematic viscosity `nu = mu / rho` (`m^2/s`) as a fresh field.
    ///
    /// Convenience for the phase-pair Reynolds numbers (see
    /// [`super::phase_pair`]); GeN-Foam reads this from `thermo().nu()`. Uses
    /// the thermodynamic (variable) density.
    #[must_use]
    pub fn nu(&self) -> VolScalarField {
        let mu = self.mu.internal.as_slice();
        let rho = self.density.internal.as_slice();
        let mut out = VolScalarField::zeros(format!("nu.{}", self.name()), self.mesh().clone());
        let o = out.internal.as_mut_slice();
        for i in 0..o.len() {
            o[i] = mu[i] / rho[i];
        }
        out
    }
}
