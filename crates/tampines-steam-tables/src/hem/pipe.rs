// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.org)
// Copyright (C) 2011-2022 OpenFOAM Foundation
// Upstream: OpenFOAM-10 (commit 89f925dc), applications/solvers/compressible/
//           rhoCentralFoam/rhoCentralFoam.C, centralCourantNo.H, setDeltaT.H
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

//! The HEM pipe solver, [`HemPipe`], and the types that configure it.
//!
//! # The scheme, and where each piece comes from
//!
//! Density-based, explicit, finite volume on a uniform 1-D mesh, conserving
//! `W = [ρ, ρu, ρE]` with `E = e + ½u²` (maintainer decision D1, gh #896):
//!
//! | piece | source |
//! |---|---|
//! | face flux | KNP central-upwind, the shared kernel [`outram_foam_basic_lib::fv_operators::fvc::central_upwind`] |
//! | reconstruction | `rhoCentralFoam.C`: `ρ`, `ρU`, `rPsi = p/ρ`, `e` and `c` each reconstructed with the limiter, then `U = ρU/ρ`, `p = ρ·rPsi` |
//! | limiter | [`outram_foam_basic_lib::limiters::FluxLimiter`], in the MUSCL form of [`outram_foam_basic_lib::fv_operators::fvc::reconstruct_pos_neg`] specialised to a uniform 1-D mesh (cross-checked against it in the unit tests) |
//! | Courant number | `centralCourantNo.H`: `Co = ½·max_cells(Σ_faces amaxSf / V)·Δt`, `amaxSf` from [`central_flux_speed_max`] |
//! | adaptive Δt | `setDeltaT.H`: reduce immediately, grow by at most `min(Δt_old + 0.1Δt*, 1.2Δt_old)` |
//! | time integration | forward Euler (upstream) or a `petir::ssprk` SSP Runge–Kutta (decision D3) |
//! | boundary faces | first order, the face value from the boundary condition (upstream's `interpolate(_, pos/neg)` at patches) |
//!
//! # Deviations from upstream, stated
//!
//! - **The Courant number is not lagged.** Upstream sets `Δt` from the
//!   *previous* step's `CoNum`; here it is computed from the current state
//!   before each step, then `setDeltaT`'s growth damping is applied against the
//!   previous `Δt`. A wave that speeds up within one step (a break opening) is
//!   therefore seen by the step it affects.
//! - **`rPsi` is `p/ρ`, not `1/ψ`.** For a perfect gas they are equal
//!   (`RT`); for a general EOS `ψ` is not defined but `p/ρ` is, so
//!   `p_face = ρ_face·rPsi_face` keeps upstream's arithmetic.
//! - **`c` is the closure's equilibrium speed**, reconstructed like `e`;
//!   upstream forms `√(γ·rPsi)` from reconstructed `Cp/Cv`, which is perfect-gas
//!   only.
//! - **Inviscid and adiabatic.** Upstream's `!inviscid` viscous and heat-flux
//!   corrections are not ported; neither is wall friction or gravity. Recorded
//!   as an omission: a pipe with friction needs a source term this solver
//!   does not have.
//! - **Errors instead of bounds.** No density or pressure is ever clamped: a
//!   non-physical state is an [`HemError::Unphysical`] and the step is undone.
//!
//! # Conservation bookkeeping
//!
//! The state vector carries four extra integrals beside the cells: the mass
//! and total energy that crossed each end, integrated by the same time
//! integrator as the cells. So `total_mass() + Σ cumulative_mass_outflow()`
//! is constant to round-off whatever the boundaries do.

use outram_foam_basic_lib::fv_operators::fvc::central_upwind::{
    central_flux_speed_max, knp_face_flux, physical_face_flux, KnpFaceFlux, KnpFaceState,
};
use outram_foam_basic_lib::limiters::FluxLimiter;
use outram_foam_basic_lib::primitives::vector::Vector3;
use petir::mathf::RealMath;
use petir::ssprk::{ssp_step, SspMethod, SspWorkspace};
use uom::si::area::square_meter;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::energy::joule;
use uom::si::f64::{
    Area, AvailableEnergy, Energy, Frequency, HeatFluxDensity, Length, Mass, MassDensity,
    MassFlux, Pressure, Ratio, ThermodynamicTemperature, Time, Velocity,
};
use uom::si::heat_flux_density::watt_per_square_meter;
use uom::si::angle::radian;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{Angle, DynamicViscosity, HeatTransfer, Power, VolumetricPowerDensity};
use uom::si::power::watt;
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::volumetric_power_density::watt_per_cubic_meter;
use super::wall_friction::wall_friction_acceleration;
use uom::si::frequency::hertz;
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::mass_flux::kilogram_per_square_meter_second;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

use super::eos::{HemEquationOfState, HemThermoState};
use super::{HemError, Iapws97Hem};

/// The largest central Courant number (upstream's measure, see
/// [`HemError::CourantViolation`]) a [`TimeStepControl::Fixed`] step may run
/// at: `0.5`, which is a classical `c·Δt/Δx = 1` for a fluid at rest. Above it
/// no explicit scheme here is stable, so the step is refused rather than run.
pub const FIXED_STEP_COURANT_LIMIT: f64 = 0.5;

/// How the flow area varies along the pipe (gh:#1058, RELAP-7 §2.1.1). A
/// `Copy` enum of analytic profiles, so [`HemPipeGeometry`] stays `Copy`; a
/// tabulated profile would need heap storage and is not offered.
///
/// Every profile is evaluated at the cell faces; a cell's area is the mean of
/// its two faces, and its volume `Δx` times that.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AreaProfile {
    /// The same area everywhere, m².
    Uniform(f64),
    /// Linear from `inlet_m2` at `x = 0` to `outlet_m2` at `x = L`.
    Linear {
        /// Area at the inlet, m².
        inlet_m2: f64,
        /// Area at the outlet, m².
        outlet_m2: f64,
    },
    /// A converging–diverging nozzle: quadratic from `inlet_m2` down to
    /// `throat_m2` at `throat_fraction·L`, then quadratic up to `outlet_m2`,
    /// with zero slope at the throat (so the area is C¹).
    ConvergingDiverging {
        /// Area at the inlet, m².
        inlet_m2: f64,
        /// Area at the throat, m², the smallest.
        throat_m2: f64,
        /// Area at the outlet, m².
        outlet_m2: f64,
        /// Throat position over the length, in `(0, 1)`.
        throat_fraction: f64,
    },
}

impl AreaProfile {
    /// Area at `x` along a pipe of length `length_m`, m².
    fn at(&self, x: f64, length_m: f64) -> f64 {
        match *self {
            AreaProfile::Uniform(a) => a,
            AreaProfile::Linear {
                inlet_m2,
                outlet_m2,
            } => inlet_m2 + (outlet_m2 - inlet_m2) * x / length_m,
            AreaProfile::ConvergingDiverging {
                inlet_m2,
                throat_m2,
                outlet_m2,
                throat_fraction,
            } => {
                let xt = throat_fraction * length_m;
                if x <= xt {
                    let s = 1.0 - x / xt;
                    throat_m2 + (inlet_m2 - throat_m2) * s * s
                } else {
                    let s = (x - xt) / (length_m - xt);
                    throat_m2 + (outlet_m2 - throat_m2) * s * s
                }
            }
        }
    }

    /// `Err` unless every area is finite and `> 0` and a throat lies inside.
    fn validate(&self) -> Result<(), HemError> {
        let (areas, ok_shape): (Vec<f64>, bool) = match *self {
            AreaProfile::Uniform(a) => (vec![a], true),
            AreaProfile::Linear {
                inlet_m2,
                outlet_m2,
            } => (vec![inlet_m2, outlet_m2], true),
            AreaProfile::ConvergingDiverging {
                inlet_m2,
                throat_m2,
                outlet_m2,
                throat_fraction,
            } => (
                vec![inlet_m2, throat_m2, outlet_m2],
                throat_fraction > 0.0 && throat_fraction < 1.0,
            ),
        };
        if areas.iter().all(|a| a.is_finite() && *a > 0.0) && ok_shape {
            Ok(())
        } else {
            Err(HemError::InvalidInput(format!(
                "area profile {self:?} needs every area finite and > 0 and a throat fraction in (0, 1)"
            )))
        }
    }
}

/// A 1-D pipe: length, flow area (constant or varying, [`AreaProfile`]) and
/// number of cells.
///
/// Cell `i` spans `[i·Δx, (i+1)·Δx]` with `Δx = L/n`; `x = 0` is the
/// [`PipeEnd::Inlet`], `x = L` the [`PipeEnd::Outlet`]. Horizontal and
/// frictionless (see the module doc).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HemPipeGeometry {
    length_m: f64,
    profile: AreaProfile,
    n_cells: usize,
    /// Incline from horizontal, rad; `+` means `+x` points upward (gh:#1061).
    incline_rad: f64,
    /// Wall roughness `ε`, m (gh:#1061).
    roughness_m: f64,
    /// Laminar shape factor `Φ_S`, 1 for a circular pipe (gh:#1061).
    shape_factor: f64,
}

impl HemPipeGeometry {
    /// Build from total length `L` (m, `> 0`), flow area `A` (m², `> 0`) and
    /// the number of uniform cells (`≥ 1`).
    ///
    /// # Errors
    ///
    /// [`HemError::InvalidInput`] naming the offending dimension.
    pub fn new(length: Length, flow_area: Area, n_cells: usize) -> Result<Self, HemError> {
        let length_m = length.get::<meter>();
        let flow_area_m2 = flow_area.get::<square_meter>();
        if !(length_m.is_finite() && length_m > 0.0) {
            return Err(HemError::InvalidInput(format!(
                "pipe length {length_m} m must be > 0"
            )));
        }
        if !(flow_area_m2.is_finite() && flow_area_m2 > 0.0) {
            return Err(HemError::InvalidInput(format!(
                "flow area {flow_area_m2} m2 must be > 0"
            )));
        }
        Self::with_area_profile(length, AreaProfile::Uniform(flow_area_m2), n_cells)
    }

    /// Build with a varying flow area (gh:#1058): the quasi-1-D equations of
    /// RELAP-7 §2.1.1, `∂(ρA)/∂t + ∂(ρuA)/∂x = 0`, momentum with the wall
    /// pressure force `p ∂A/∂x`, energy with `ρuHA`.
    ///
    /// # Errors
    ///
    /// [`HemError::InvalidInput`] for a non-positive length, an invalid
    /// profile, or zero cells.
    pub fn with_area_profile(
        length: Length,
        profile: AreaProfile,
        n_cells: usize,
    ) -> Result<Self, HemError> {
        let length_m = length.get::<meter>();
        if !(length_m.is_finite() && length_m > 0.0) {
            return Err(HemError::InvalidInput(format!(
                "pipe length {length_m} m must be > 0"
            )));
        }
        profile.validate()?;
        if n_cells == 0 {
            return Err(HemError::InvalidInput(
                "a pipe needs at least one cell".into(),
            ));
        }
        Ok(Self {
            length_m,
            profile,
            n_cells,
            incline_rad: 0.0,
            roughness_m: 0.0,
            shape_factor: 1.0,
        })
    }

    /// The same pipe inclined by `incline` from horizontal (`+` for `+x`
    /// upward): gravity enters momentum and energy (gh:#1061). Default 0.
    pub fn with_incline(self, incline: Angle) -> Self {
        Self { incline_rad: incline.get::<radian>(), ..self }
    }

    /// The same pipe with wall roughness `ε` (default 0, smooth).
    ///
    /// # Errors
    ///
    /// [`HemError::InvalidInput`] for a negative or non-finite roughness.
    pub fn with_roughness(self, roughness: Length) -> Result<Self, HemError> {
        let r = roughness.get::<meter>();
        if !(r.is_finite() && r >= 0.0) {
            return Err(HemError::InvalidInput(format!("roughness {r} m must be >= 0")));
        }
        Ok(Self { roughness_m: r, ..self })
    }

    /// The same pipe with laminar shape factor `Φ_S` (default 1, circular).
    ///
    /// # Errors
    ///
    /// [`HemError::InvalidInput`] unless `Φ_S > 0`.
    pub fn with_shape_factor(self, shape_factor: f64) -> Result<Self, HemError> {
        if !(shape_factor.is_finite() && shape_factor > 0.0) {
            return Err(HemError::InvalidInput(format!("shape factor {shape_factor} must be > 0")));
        }
        Ok(Self { shape_factor, ..self })
    }

    /// Incline from horizontal.
    pub fn incline(&self) -> Angle {
        Angle::new::<radian>(self.incline_rad)
    }

    /// Wall roughness `ε`.
    pub fn roughness(&self) -> Length {
        Length::new::<meter>(self.roughness_m)
    }

    /// Total length `L`.
    pub fn length(&self) -> Length {
        Length::new::<meter>(self.length_m)
    }

    /// Flow area at the inlet face, `A(0)`: the pipe's area when it is
    /// uniform. For a varying area use [`Self::flow_area_at`].
    pub fn flow_area(&self) -> Area {
        Area::new::<square_meter>(self.face_area_m2(0))
    }

    /// The area profile.
    pub fn area_profile(&self) -> AreaProfile {
        self.profile
    }

    /// Flow area at `x` from the inlet.
    pub fn flow_area_at(&self, x: Length) -> Area {
        Area::new::<square_meter>(self.profile.at(x.get::<meter>(), self.length_m))
    }

    /// Hydraulic diameter at `x` for a circular cross-section, `4A/P =
    /// √(4A/π)`.
    pub fn hydraulic_diameter_at(&self, x: Length) -> Length {
        let a = self.profile.at(x.get::<meter>(), self.length_m);
        Length::new::<meter>((4.0 * a / std::f64::consts::PI).sqrt())
    }

    /// Wall (heated) area per unit length at `x` for a circular cross-section
    /// whose radius varies along the pipe: `2πR√(1 + (dR/dx)²)`, the lateral
    /// surface of a frustum per unit length (RELAP-7 Eqs 38–44 for the
    /// circular case). `dR/dx` is a central difference over one cell.
    pub fn wall_area_per_length_at(&self, x: Length) -> Length {
        let xm = x.get::<meter>().clamp(0.0, self.length_m);
        let r = |x: f64| (self.profile.at(x, self.length_m) / std::f64::consts::PI).sqrt();
        let h = 0.5 * self.dx();
        let (x0, x1) = ((xm - h).max(0.0), (xm + h).min(self.length_m));
        let slope = (r(x1) - r(x0)) / (x1 - x0);
        Length::new::<meter>(2.0 * std::f64::consts::PI * r(xm) * (1.0 + slope * slope).sqrt())
    }

    /// `true` when the area is the same everywhere.
    pub fn is_uniform(&self) -> bool {
        matches!(self.profile, AreaProfile::Uniform(_))
    }

    /// Area of face `f` (`0` is the inlet, `n` the outlet), m².
    fn face_area_m2(&self, face: usize) -> f64 {
        self.profile.at(face as f64 * self.dx(), self.length_m)
    }

    /// Area of cell `i`, the mean of its faces, m².
    fn cell_area_m2(&self, cell: usize) -> f64 {
        0.5 * (self.face_area_m2(cell) + self.face_area_m2(cell + 1))
    }

    /// Number of cells `n`.
    pub fn n_cells(&self) -> usize {
        self.n_cells
    }

    /// Cell width `Δx = L/n`.
    pub fn cell_width(&self) -> Length {
        Length::new::<meter>(self.dx())
    }

    /// Centre of cell `i`, measured from the inlet: `(i + ½)·Δx`.
    pub fn cell_centre(&self, cell: usize) -> Length {
        Length::new::<meter>((cell as f64 + 0.5) * self.dx())
    }

    fn dx(&self) -> f64 {
        self.length_m / self.n_cells as f64
    }
}

/// A primitive state, for initial conditions. Velocity is along `+x`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PrimitiveState {
    /// Single-phase `(p, T)`. Inside the dome `(p, T)` does not fix the
    /// state; use [`Self::PressureEnthalpy`].
    PressureTemperature {
        /// Absolute pressure.
        pressure: Pressure,
        /// Temperature.
        temperature: ThermodynamicTemperature,
        /// Velocity along the pipe (`+` towards the outlet).
        velocity: Velocity,
    },
    /// `(p, h)`, valid in the dome.
    PressureEnthalpy {
        /// Absolute pressure.
        pressure: Pressure,
        /// Specific enthalpy, J/kg.
        specific_enthalpy: AvailableEnergy,
        /// Velocity along the pipe.
        velocity: Velocity,
    },
    /// `(p, ρ)`: the natural input of the ideal-gas Sod problem. Not every
    /// equation of state supports it (IF97 does not).
    PressureDensity {
        /// Absolute pressure.
        pressure: Pressure,
        /// Mass density.
        density: MassDensity,
        /// Velocity along the pipe.
        velocity: Velocity,
    },
}

impl PrimitiveState {
    fn close<E: HemEquationOfState>(&self, eos: &E) -> Result<(HemThermoState, f64), HemError> {
        match *self {
            PrimitiveState::PressureTemperature {
                pressure,
                temperature,
                velocity,
            } => Ok((
                eos.state_p_t(pressure, temperature)?,
                velocity.get::<meter_per_second>(),
            )),
            PrimitiveState::PressureEnthalpy {
                pressure,
                specific_enthalpy,
                velocity,
            } => Ok((
                eos.state_p_h(pressure, specific_enthalpy)?,
                velocity.get::<meter_per_second>(),
            )),
            PrimitiveState::PressureDensity {
                pressure,
                density,
                velocity,
            } => Ok((
                eos.state_p_rho(pressure, density)?,
                velocity.get::<meter_per_second>(),
            )),
        }
    }
}

/// The initial condition of a [`HemPipe`].
#[derive(Debug, Clone, PartialEq)]
pub enum HemInitialCondition {
    /// The same state in every cell.
    Uniform(PrimitiveState),
    /// A Riemann problem: `left` in cells whose centre is below `diaphragm`
    /// (measured from the inlet), `right` in the rest.
    Riemann {
        /// Diaphragm position, measured from the inlet (`x = 0`).
        diaphragm: Length,
        /// State for `x < diaphragm`.
        left: PrimitiveState,
        /// State for `x ≥ diaphragm`.
        right: PrimitiveState,
    },
    /// One state per cell, inlet first; length must equal the cell count.
    PerCell(Vec<PrimitiveState>),
}

/// Which end of the pipe a boundary condition applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipeEnd {
    /// `x = 0`.
    Inlet,
    /// `x = L`.
    Outlet,
}

/// A boundary condition at one pipe end. Every one is applied as a
/// first-order boundary face state (upstream's treatment of patches).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HemBoundary {
    /// Closed rigid end: face velocity `0`, pressure, density and energy
    /// zero-gradient. The face mass and energy fluxes are exactly zero, so
    /// two walls conserve mass and total energy to round-off.
    Wall,
    /// Zero-gradient (transmissive) open end, every variable extrapolated
    /// from the end cell. Sod's open ends.
    Transmissive,
    /// Stagnation reservoir `(p0, h0)`, upstream's `totalPressure` with a
    /// total enthalpy: for inflow the face takes `p = p0 − ½ρu²` and
    /// `h = h0 − ½u²` (velocity zero-gradient); for outflow `p = p0` with the
    /// end cell's enthalpy. Density and sound speed from the EOS at the face.
    Reservoir {
        /// Stagnation pressure `p0`.
        stagnation_pressure: Pressure,
        /// Stagnation specific enthalpy `h0`, J/kg.
        stagnation_enthalpy: AvailableEnergy,
    },
    /// Fixed static pressure; velocity zero-gradient, enthalpy taken from the
    /// end cell (upstream fixes `p` and extrapolates `T`; inside the dome `T`
    /// does not fix the state, so `h` stands in for it).
    FixedPressure(Pressure),
    /// A break of area `area_fraction × A` discharging to `back_pressure`.
    /// The mass flux is the EOS's [`HemEquationOfState::break_flow`] at the
    /// end cell's stagnation state `(p, h + ½u²)`: critical if choked,
    /// isentropic-subcritical otherwise, zero below the back pressure. The
    /// face carries that mass flux out of the pipe with the end cell's
    /// pressure, density and internal energy. Change `area_fraction` between
    /// steps to ramp a rupture disc. Only [`Iapws97Hem`] supplies the flow
    /// model; other closures return [`HemError::Unsupported`].
    ChokedBreak {
        /// Break area over pipe flow area, in `[0, 1]`.
        area_fraction: Ratio,
        /// Pressure the break discharges to.
        back_pressure: Pressure,
    },
    /// A prescribed face flux per unit area, in the `+x` direction like every
    /// face flux here (so at the inlet a positive mass flux enters the pipe,
    /// at the outlet it leaves). The junction framework (gh:#1071) computes
    /// it from the state across the junction and holds it for one step. The
    /// end cell's state is the reconstruction ghost; the Courant rate uses the
    /// end cell's `|u| + c`.
    Flux {
        /// Mass flux `ρu`, kg/(m²·s).
        mass_flux: MassFlux,
        /// Momentum flux `ρu² + p`, Pa.
        momentum_flux: Pressure,
        /// Total-energy flux `ρuH`, W/m².
        energy_flux: HeatFluxDensity,
    },
}

/// Heat through the pipe wall into the fluid (gh:#1062, RELAP-7 Eqs 22,
/// 35–37). Applied over the wall area per unit length,
/// [`HemPipeGeometry::wall_area_per_length_at`] (circular cross-section),
/// divided by the flow area, as a source in the total-energy equation.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum WallHeat {
    /// No heat through the wall (the default: a boundary input, not physics
    /// a closure supplies).
    #[default]
    Adiabatic,
    /// A prescribed heat flux into the fluid, W/m² of wall.
    HeatFlux(HeatFluxDensity),
    /// A wall at `temperature` with heat-transfer coefficient `h`: `q'' =
    /// h (T_wall − T)`, `T` the cell temperature (RELAP-7 Eq 35).
    WallTemperature {
        /// Wall temperature.
        temperature: ThermodynamicTemperature,
        /// Heat-transfer coefficient `H_w`.
        heat_transfer_coefficient: HeatTransfer,
    },
}

/// How the time step is chosen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimeStepControl {
    /// Upstream `setDeltaT.H` at a target central Courant number (upstream's
    /// measure, half the classical number in 1-D), capped at `max_dt`.
    AdaptiveCourant {
        /// Target central Courant number, `> 0`. Upstream's shock-tube
        /// tutorial uses `0.2`.
        courant: Ratio,
        /// Largest step allowed.
        max_dt: Time,
    },
    /// A fixed step. A step whose central Courant number would exceed
    /// [`FIXED_STEP_COURANT_LIMIT`] returns [`HemError::CourantViolation`]
    /// instead of running unstable.
    Fixed(Time),
}

/// The time integrator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeIntegrator {
    /// One forward-Euler stage, as upstream `rhoCentralFoam` (`fvm::ddt`,
    /// Euler) does.
    ForwardEuler,
    /// Shu–Osher SSP Runge–Kutta, 2 stages, 2nd order (`petir::ssprk`, a port
    /// of PyClaw's `SSP22`).
    Ssp22,
    /// Shu–Osher SSP Runge–Kutta, 3 stages, 3rd order (PyClaw `SSP33`).
    Ssp33,
    /// Ketcheson's SSP Runge–Kutta, 10 stages, 4th order (PyClaw `SSP104`).
    Ssp104,
}

/// Numerical settings. [`Default`] is the configuration the V&V record
/// (`verification_and_validation/hem_pipe_vs_exact_riemann_and_edwards.md`)
/// was measured with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HemNumerics {
    /// Time-step control. Default: adaptive at central Courant `0.2`
    /// (upstream's shock-tube tutorial value), `max_dt = 1 ms`.
    pub time_step: TimeStepControl,
    /// Reconstruction limiter. Default: van Leer (upstream's
    /// `reconstruct(...) vanLeer`).
    pub limiter: FluxLimiter,
    /// Time integrator. Default: [`TimeIntegrator::Ssp22`] (decision D3), the
    /// cheapest scheme that keeps the forward-Euler TVD property at second
    /// order in time.
    pub time_integrator: TimeIntegrator,
}

impl Default for HemNumerics {
    fn default() -> Self {
        Self {
            time_step: TimeStepControl::AdaptiveCourant {
                courant: Ratio::new::<ratio>(0.2),
                max_dt: Time::new::<second>(1.0e-3),
            },
            limiter: FluxLimiter::VanLeer,
            time_integrator: TimeIntegrator::Ssp22,
        }
    }
}

/// One cell's full state, in `uom` units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HemCellState {
    /// Absolute static pressure.
    pub pressure: Pressure,
    /// Temperature (the saturation temperature inside the dome).
    pub temperature: ThermodynamicTemperature,
    /// Mixture density.
    pub density: MassDensity,
    /// Velocity along the pipe (`+` towards the outlet).
    pub velocity: Velocity,
    /// Specific internal energy, J/kg.
    pub specific_internal_energy: AvailableEnergy,
    /// Specific static enthalpy, J/kg.
    pub specific_enthalpy: AvailableEnergy,
    /// Homogeneous-equilibrium sound speed.
    pub equilibrium_sound_speed: Velocity,
    /// Equilibrium quality inside the dome, `None` outside it.
    pub quality: Option<Ratio>,
    /// Homogeneous (no-slip) void fraction.
    pub void_fraction: Ratio,
    /// Dynamic viscosity, when the closure supplies one (gh:#1061).
    pub dynamic_viscosity: Option<DynamicViscosity>,
}

impl HemCellState {
    fn new(t: HemThermoState, u: f64, mu: Option<DynamicViscosity>) -> Self {
        Self {
            pressure: t.pressure,
            temperature: t.temperature,
            density: t.density,
            velocity: Velocity::new::<meter_per_second>(u),
            specific_internal_energy: t.specific_internal_energy,
            specific_enthalpy: t.specific_enthalpy,
            equilibrium_sound_speed: t.equilibrium_sound_speed,
            quality: t.quality,
            void_fraction: t.void_fraction,
            dynamic_viscosity: mu,
        }
    }
}

/// Every cell's state at one time.
#[derive(Debug, Clone, PartialEq)]
pub struct HemSnapshot {
    /// Simulation time.
    pub time: Time,
    /// Cell centres, measured from the inlet.
    pub cell_centres: Vec<Length>,
    /// Cell states, inlet first.
    pub cells: Vec<HemCellState>,
}

/// What one [`HemPipe::step`] did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HemStepReport {
    /// Time at the end of the step.
    pub time: Time,
    /// Step taken.
    pub dt: Time,
    /// Central Courant number of the step (upstream's measure).
    pub courant: Ratio,
    /// Lowest cell pressure at the end of the step.
    pub min_pressure: Pressure,
    /// `true` if a [`HemBoundary::ChokedBreak`] end was choked at the start
    /// of the step.
    pub break_choked: bool,
}

/// What [`HemPipe::run_until`] did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HemRunSummary {
    /// Steps taken.
    pub steps: usize,
    /// Time reached (exactly the requested end time).
    pub time: Time,
    /// Largest central Courant number of any step.
    pub max_courant: Ratio,
    /// Lowest cell pressure seen at the end of any step.
    pub min_pressure: Pressure,
}

/// One cell, decoded, raw SI.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Prim {
    rho: f64,
    u: f64,
    p: f64,
    e: f64,
    c: f64,
    /// Dynamic viscosity, Pa·s; 0 when the closure supplies none.
    mu: f64,
    /// Temperature, K.
    t: f64,
}

impl Prim {
    fn of(cell: &HemCellState) -> Self {
        Self {
            rho: cell.density.get::<kilogram_per_cubic_meter>(),
            u: cell.velocity.get::<meter_per_second>(),
            p: cell.pressure.get::<pascal>(),
            e: cell.specific_internal_energy.get::<joule_per_kilogram>(),
            c: cell.equilibrium_sound_speed.get::<meter_per_second>(),
            mu: cell.dynamic_viscosity.map_or(0.0, |m| m.get::<pascal_second>()),
            t: cell.temperature.get::<kelvin>(),
        }
    }

    fn knp(&self) -> KnpFaceState {
        KnpFaceState::total_energy(
            self.rho,
            Vector3::new(self.u, 0.0, 0.0),
            self.p,
            self.c,
            self.e,
        )
    }
}

/// Face fluxes (per unit area, `+x`), face speeds and break status of one
/// state.
#[derive(Debug, Clone)]
struct Assembly {
    flux: Vec<KnpFaceFlux>,
    /// Cell pressures, Pa, for the wall-pressure force of a varying area.
    cell_pressure: Vec<f64>,
    /// Cell momentum and energy sources per unit volume from wall friction
    /// and gravity (gh:#1061), N/m³ and W/m³.
    cell_source: Vec<(f64, f64)>,
    courant_rate: f64,
    break_choked: bool,
}

/// Number of bookkeeping entries after the cells in the state vector:
/// mass in at the inlet, energy in at the inlet, mass out at the outlet,
/// energy out at the outlet (kg and J).
const N_BOOKKEEPING: usize = 4;

/// MUSCL face values of one variable on a uniform 1-D mesh with one ghost
/// value at each end (`ext.len() = n + 2`, `ext[0]` and `ext[n+1]` the
/// ghosts). Returns `(pos, neg)` for the `n − 1` internal faces.
///
/// The uniform-mesh specialisation of
/// [`outram_foam_basic_lib::fv_operators::fvc::reconstruct_pos_neg`]: there
/// `r_pos = 2·(d·∇φ_O)/(φ_N − φ_O) − 1` with the Gauss gradient
/// `∇φ_O = (φ_{O+1} − φ_{O−1})/(2Δx)`, which reduces to
/// `r_pos = (φ_O − φ_{O−1})/(φ_N − φ_O)`, and likewise for `r_neg`; the face
/// value is `φ_O + ψ(r)·(φ_lin − φ_O)`. The unit test
/// `reconstruction_matches_basic_lib_reconstruct_pos_neg` holds the two equal.
pub(crate) fn muscl_1d(ext: &[f64], limiter: FluxLimiter) -> (Vec<f64>, Vec<f64>) {
    const TINY: f64 = 1e-30; // as reconstruct_pos_neg
    let n = ext.len() - 2;
    let mut pos = Vec::with_capacity(n.saturating_sub(1));
    let mut neg = Vec::with_capacity(n.saturating_sub(1));
    for f in 0..n.saturating_sub(1) {
        // owner ext[f+1], neighbour ext[f+2]
        let (pm, po, pn, pnn) = (ext[f], ext[f + 1], ext[f + 2], ext[f + 3]);
        let gradf = pn - po;
        if gradf.abs() < TINY {
            pos.push(po);
            neg.push(pn);
            continue;
        }
        let lin = 0.5 * (po + pn);
        let r_pos = (po - pm) / gradf;
        let r_neg = (pnn - pn) / gradf;
        pos.push(po + limiter.psi(r_pos) * (lin - po));
        neg.push(pn + limiter.psi(r_neg) * (lin - pn));
    }
    (pos, neg)
}

/// One reconstructed variable `f` over the cells, with a ghost
/// `2·f(b) − f(end cell)` at each end so that the linear face value at a
/// boundary equals the boundary value (the Gauss gradient upstream forms from
/// the patch value).
fn with_ghosts<F: Fn(&Prim) -> f64>(cells: &[Prim], b_in: &Prim, b_out: &Prim, f: F) -> Vec<f64> {
    let n = cells.len();
    let mut ext = Vec::with_capacity(n + 2);
    ext.push(2.0 * f(b_in) - f(&cells[0]));
    ext.extend(cells.iter().map(&f));
    ext.push(2.0 * f(b_out) - f(&cells[n - 1]));
    ext
}

/// A transient HEM pipe: conserved `[ρ, ρu, ρE]` per cell, closed by `E`.
///
/// ```no_run
/// use tampines_steam_tables::hem::*;
/// use uom::si::f64::*;
/// use uom::si::{area::square_meter, length::meter, pressure::megapascal,
///               thermodynamic_temperature::kelvin, time::second, velocity::meter_per_second};
///
/// let geometry = HemPipeGeometry::new(
///     Length::new::<meter>(4.096), Area::new::<square_meter>(4.185e-3), 24).unwrap();
/// let initial = HemInitialCondition::Uniform(PrimitiveState::PressureTemperature {
///     pressure: Pressure::new::<megapascal>(7.0),
///     temperature: ThermodynamicTemperature::new::<kelvin>(500.0),
///     velocity: Velocity::new::<meter_per_second>(0.0),
/// });
/// // IF97 water is the default closure.
/// let mut pipe: HemPipe = HemPipe::new(geometry, Iapws97Hem, initial, HemNumerics::default()).unwrap();
/// pipe.set_boundary(PipeEnd::Outlet, HemBoundary::FixedPressure(
///     Pressure::new::<megapascal>(6.0))).unwrap();
/// pipe.run_until(Time::new::<second>(1.0e-3)).unwrap();
/// let snapshot = pipe.snapshot();
/// ```
///
/// Both ends start as [`HemBoundary::Wall`].
#[derive(Debug, Clone)]
pub struct HemPipe<E: HemEquationOfState = Iapws97Hem> {
    geometry: HemPipeGeometry,
    eos: E,
    numerics: HemNumerics,
    /// `[ρ_0, ρu_0, ρE_0, ρ_1, …, M_in, E_in, M_out, E_out]`.
    q: Vec<f64>,
    /// Decoded state of `q`, kept in step with it.
    cells: Vec<HemCellState>,
    inlet: HemBoundary,
    outlet: HemBoundary,
    time_s: f64,
    last_dt: Option<f64>,
    work: SspWorkspace,
    /// Wall friction applied (default; gh:#1061). Off only by
    /// [`HemPipe::without_wall_friction`].
    wall_friction: bool,
    /// Heat through the wall (gh:#1062); adiabatic by default.
    wall_heat: WallHeat,
    /// Volumetric heat source `r` (RELAP-7 Eq 22), W/m³; zero by default.
    volumetric_heating_w_m3: f64,
    /// Per-cell heat input as a power density \[W/m³\]; empty when unset.
    cell_heating_w_m3: Vec<f64>,
}

impl<E: HemEquationOfState> HemPipe<E> {
    /// Build the pipe and close its initial state with `eos`.
    ///
    /// # Errors
    ///
    /// [`HemError::InvalidInput`] for a per-cell condition of the wrong length
    /// or invalid numerics; any error of the EOS for a state it cannot close.
    pub fn new(
        geometry: HemPipeGeometry,
        eos: E,
        initial: HemInitialCondition,
        numerics: HemNumerics,
    ) -> Result<Self, HemError> {
        validate_numerics(&numerics)?;
        let n = geometry.n_cells();
        let states: Vec<PrimitiveState> = match initial {
            HemInitialCondition::Uniform(s) => vec![s; n],
            HemInitialCondition::Riemann {
                diaphragm,
                left,
                right,
            } => {
                let xd = diaphragm.get::<meter>();
                (0..n)
                    .map(|i| {
                        if geometry.cell_centre(i).get::<meter>() < xd {
                            left
                        } else {
                            right
                        }
                    })
                    .collect()
            }
            HemInitialCondition::PerCell(v) => {
                if v.len() != n {
                    return Err(HemError::InvalidInput(format!(
                        "per-cell initial condition has {} states for {n} cells",
                        v.len()
                    )));
                }
                v
            }
        };
        let mut q = vec![0.0; 3 * n + N_BOOKKEEPING];
        let mut cells = Vec::with_capacity(n);
        for (i, s) in states.iter().enumerate() {
            let (t, u) = s.close(&eos).map_err(|e| e.in_cell(i))?;
            let rho = t.density.get::<kilogram_per_cubic_meter>();
            let e = t.specific_internal_energy.get::<joule_per_kilogram>();
            q[3 * i] = rho;
            q[3 * i + 1] = rho * u;
            q[3 * i + 2] = rho * (e + 0.5 * u * u);
            cells.push(HemCellState::new(t, u, eos.dynamic_viscosity(&t)));
        }
        let len = q.len();
        let mut pipe = Self {
            geometry,
            eos,
            numerics,
            q,
            cells,
            inlet: HemBoundary::Wall,
            outlet: HemBoundary::Wall,
            time_s: 0.0,
            last_dt: None,
            work: SspWorkspace::new(len),
            wall_friction: true,
            wall_heat: WallHeat::Adiabatic,
            volumetric_heating_w_m3: 0.0,
            cell_heating_w_m3: Vec::new(),
        };
        // Close the conserved state once through (ρ, e), so what the solver
        // carries is exactly what it will decode on the first step.
        let hints: Vec<f64> = pipe
            .cells
            .iter()
            .map(|c| c.pressure.get::<pascal>())
            .collect();
        pipe.cells = pipe.decode(&pipe.q, &hints)?;
        Ok(pipe)
    }

    /// Set the boundary condition at one end (effective from the next step).
    ///
    /// # Errors
    ///
    /// [`HemError::InvalidInput`] for a non-positive or non-finite pressure,
    /// or a break area fraction outside `[0, 1]`.
    pub fn set_boundary(&mut self, end: PipeEnd, boundary: HemBoundary) -> Result<(), HemError> {
        let positive = |name: &str, p: Pressure| -> Result<(), HemError> {
            let v = p.get::<pascal>();
            if v.is_finite() && v > 0.0 {
                Ok(())
            } else {
                Err(HemError::InvalidInput(format!("{name} {v} Pa must be > 0")))
            }
        };
        match boundary {
            HemBoundary::Wall | HemBoundary::Transmissive => {}
            HemBoundary::Flux {
                mass_flux,
                momentum_flux,
                energy_flux,
            } => {
                let v = [
                    mass_flux.get::<kilogram_per_square_meter_second>(),
                    momentum_flux.get::<pascal>(),
                    energy_flux.get::<watt_per_square_meter>(),
                ];
                if !v.iter().all(|x| x.is_finite()) {
                    return Err(HemError::InvalidInput(format!(
                        "prescribed face flux {v:?} must be finite"
                    )));
                }
            }
            HemBoundary::Reservoir {
                stagnation_pressure,
                ..
            } => positive("reservoir stagnation pressure", stagnation_pressure)?,
            HemBoundary::FixedPressure(p) => positive("fixed pressure", p)?,
            HemBoundary::ChokedBreak {
                area_fraction,
                back_pressure,
            } => {
                positive("break back pressure", back_pressure)?;
                let f = area_fraction.get::<ratio>();
                if !(0.0..=1.0).contains(&f) {
                    return Err(HemError::InvalidInput(format!(
                        "break area fraction {f} must be in [0, 1]"
                    )));
                }
            }
        }
        match end {
            PipeEnd::Inlet => self.inlet = boundary,
            PipeEnd::Outlet => self.outlet = boundary,
        }
        Ok(())
    }

    /// The boundary condition at one end.
    pub fn boundary(&self, end: PipeEnd) -> HemBoundary {
        match end {
            PipeEnd::Inlet => self.inlet,
            PipeEnd::Outlet => self.outlet,
        }
    }

    /// The pipe geometry.
    pub fn geometry(&self) -> &HemPipeGeometry {
        &self.geometry
    }

    /// The closure.
    pub fn equation_of_state(&self) -> &E {
        &self.eos
    }

    /// The numerical settings.
    pub fn numerics(&self) -> &HemNumerics {
        &self.numerics
    }

    /// Simulation time.
    pub fn time(&self) -> Time {
        Time::new::<second>(self.time_s)
    }

    /// State of one cell (`0` at the inlet).
    ///
    /// # Errors
    ///
    /// [`HemError::InvalidInput`] for an index past the last cell.
    pub fn cell_state(&self, cell: usize) -> Result<HemCellState, HemError> {
        self.cells.get(cell).copied().ok_or_else(|| {
            HemError::InvalidInput(format!(
                "cell {cell} out of range (n = {})",
                self.cells.len()
            ))
        })
    }

    /// Every cell's state now.
    pub fn snapshot(&self) -> HemSnapshot {
        HemSnapshot {
            time: self.time(),
            cell_centres: (0..self.geometry.n_cells())
                .map(|i| self.geometry.cell_centre(i))
                .collect(),
            cells: self.cells.clone(),
        }
    }

    /// Mass in the pipe, `Σ ρ_i·Δx·A`.
    pub fn total_mass(&self) -> Mass {
        Mass::new::<kilogram>(self.volume_sum(0))
    }

    /// Total (internal plus kinetic) energy in the pipe, `Σ ρE_i·Δx·A`.
    pub fn total_energy(&self) -> Energy {
        Energy::new::<joule>(self.volume_sum(2))
    }

    /// Mass that has left the pipe through `end` since `t = 0` (negative if
    /// net inflow), integrated by the same scheme as the cells.
    pub fn cumulative_mass_outflow(&self, end: PipeEnd) -> Mass {
        let n = 3 * self.geometry.n_cells();
        Mass::new::<kilogram>(match end {
            PipeEnd::Inlet => -self.q[n],
            PipeEnd::Outlet => self.q[n + 2],
        })
    }

    /// Total energy that has left the pipe through `end` since `t = 0`
    /// (negative if net inflow).
    pub fn cumulative_energy_outflow(&self, end: PipeEnd) -> Energy {
        let n = 3 * self.geometry.n_cells();
        Energy::new::<joule>(match end {
            PipeEnd::Inlet => -self.q[n + 1],
            PipeEnd::Outlet => self.q[n + 3],
        })
    }

    /// Central Courant number per second of step at the current state
    /// (upstream `centralCourantNo.H` divided by `Δt`), in 1/s. The adaptive
    /// step is `courant / this`.
    ///
    /// # Errors
    ///
    /// As [`Self::step`], from evaluating the boundary faces.
    pub fn courant_rate(&self) -> Result<Frequency, HemError> {
        Ok(Frequency::new::<hertz>(self.assemble(&self.prims())?.courant_rate))
    }

    /// Advance one step, chosen by [`HemNumerics::time_step`].
    ///
    /// On any error the state is left exactly as it was before the call.
    ///
    /// # Errors
    ///
    /// [`HemError::CourantViolation`] under a fixed step that is too large;
    /// [`HemError::Unphysical`] / [`HemError::EquationOfState`] if a stage
    /// produces a state that cannot be closed.
    pub fn step(&mut self) -> Result<HemStepReport, HemError> {
        self.step_until_at_most(f64::INFINITY)
    }

    /// The same pipe without wall friction: the ablation of gh:#1061. Wall
    /// friction is on by default and acts whenever the closure supplies a
    /// viscosity ([`HemEquationOfState::dynamic_viscosity`]).
    pub fn without_wall_friction(self) -> Self {
        Self { wall_friction: false, ..self }
    }

    /// Set the heat through the wall (gh:#1062).
    ///
    /// # Errors
    ///
    /// [`HemError::InvalidInput`] for a non-finite flux, a non-positive wall
    /// temperature, or a negative or non-finite coefficient.
    pub fn set_wall_heat(&mut self, wall_heat: WallHeat) -> Result<(), HemError> {
        let ok = match wall_heat {
            WallHeat::Adiabatic => true,
            WallHeat::HeatFlux(q) => q.get::<watt_per_square_meter>().is_finite(),
            WallHeat::WallTemperature { temperature, heat_transfer_coefficient } => {
                let t = temperature.get::<kelvin>();
                let h = heat_transfer_coefficient.get::<watt_per_square_meter_kelvin>();
                t.is_finite() && t > 0.0 && h.is_finite() && h >= 0.0
            }
        };
        if !ok {
            return Err(HemError::InvalidInput(format!("invalid wall heat {wall_heat:?}")));
        }
        self.wall_heat = wall_heat;
        Ok(())
    }

    /// Set a uniform volumetric heat source `r` (RELAP-7 Eq 22).
    ///
    /// # Errors
    ///
    /// [`HemError::InvalidInput`] if it is not finite.
    pub fn set_volumetric_heating(&mut self, r: VolumetricPowerDensity) -> Result<(), HemError> {
        let v = r.get::<watt_per_cubic_meter>();
        if !v.is_finite() {
            return Err(HemError::InvalidInput(format!("volumetric heating {v} W/m3")));
        }
        self.volumetric_heating_w_m3 = v;
        Ok(())
    }

    /// Set a heat input per cell, as a power deposited in each cell's fluid
    /// (gh:#1079: a reactor power source distributes core power this way). It
    /// adds to any uniform [`Self::set_volumetric_heating`]. An empty slice
    /// clears it.
    ///
    /// # Errors
    ///
    /// [`HemError::InvalidInput`] if the length is not the cell count or a
    /// value is not finite.
    pub fn set_cell_heat_input(&mut self, powers: &[Power]) -> Result<(), HemError> {
        if powers.is_empty() {
            self.cell_heating_w_m3.clear();
            return Ok(());
        }
        let n = self.geometry.n_cells();
        if powers.len() != n {
            return Err(HemError::InvalidInput(format!("{} cell powers for {n} cells", powers.len())));
        }
        let mut v = Vec::with_capacity(n);
        for (i, q) in powers.iter().enumerate() {
            let w = q.get::<watt>();
            if !w.is_finite() {
                return Err(HemError::InvalidInput(format!("cell {i} heat input {w} W")));
            }
            v.push(w / self.cell_volume(i));
        }
        self.cell_heating_w_m3 = v;
        Ok(())
    }

    /// The per-cell heat input set by [`Self::set_cell_heat_input`] (empty
    /// when unset).
    pub fn cell_heat_input(&self) -> Vec<Power> {
        self.cell_heating_w_m3
            .iter()
            .enumerate()
            .map(|(i, r)| Power::new::<watt>(r * self.cell_volume(i)))
            .collect()
    }

    /// The heat through the wall.
    pub fn wall_heat(&self) -> WallHeat {
        self.wall_heat
    }

    /// Whether wall friction is applied.
    pub fn has_wall_friction(&self) -> bool {
        self.wall_friction
    }

    /// Advance exactly `dt`, whatever [`HemNumerics::time_step`] says (the
    /// junction framework, gh:#1071, steps every pipe of a network by one
    /// common `dt`). A step whose central Courant number would exceed
    /// [`FIXED_STEP_COURANT_LIMIT`] is refused.
    ///
    /// # Errors
    ///
    /// [`HemError::InvalidInput`] for a non-positive or non-finite `dt`,
    /// [`HemError::CourantViolation`], and as [`Self::step`].
    pub fn step_by(&mut self, dt: Time) -> Result<HemStepReport, HemError> {
        let d = dt.get::<second>();
        if !(d.is_finite() && d > 0.0) {
            return Err(HemError::InvalidInput(format!("step {d} s must be > 0")));
        }
        let saved = self.numerics.time_step;
        self.numerics.time_step = TimeStepControl::Fixed(dt);
        let out = self.step_until_at_most(self.time_s + d);
        self.numerics.time_step = saved;
        out
    }

    /// Step until `t_end`, shortening the last step to land on it exactly.
    ///
    /// # Errors
    ///
    /// As [`Self::step`]; the state is that after the last successful step.
    /// [`HemError::InvalidInput`] if `t_end` is before the current time.
    pub fn run_until(&mut self, t_end: Time) -> Result<HemRunSummary, HemError> {
        let te = t_end.get::<second>();
        if !(te.is_finite() && te >= self.time_s) {
            return Err(HemError::InvalidInput(format!(
                "run_until({te} s) is before the current time {} s",
                self.time_s
            )));
        }
        let mut steps = 0;
        let mut max_co: f64 = 0.0;
        let mut min_p = f64::INFINITY;
        while self.time_s < te {
            let r = self.step_until_at_most(te)?;
            steps += 1;
            max_co = max_co.max(r.courant.get::<ratio>());
            min_p = min_p.min(r.min_pressure.get::<pascal>());
        }
        Ok(HemRunSummary {
            steps,
            time: self.time(),
            max_courant: Ratio::new::<ratio>(max_co),
            min_pressure: Pressure::new::<pascal>(min_p),
        })
    }

    /// Volume of cell `i`, `Δx·A_i`.
    fn cell_volume(&self, cell: usize) -> f64 {
        self.geometry.dx() * self.geometry.cell_area_m2(cell)
    }

    /// `Σ q[3i + k]·V_i`; for a uniform pipe the original `(Σ q)·V`, so its
    /// results stay byte-identical to the constant-area solver.
    fn volume_sum(&self, k: usize) -> f64 {
        let n = self.geometry.n_cells();
        if self.geometry.is_uniform() {
            (0..n).map(|i| self.q[3 * i + k]).sum::<f64>() * self.cell_volume(0)
        } else {
            (0..n).map(|i| self.q[3 * i + k] * self.cell_volume(i)).sum()
        }
    }

    fn prims(&self) -> Vec<Prim> {
        self.cells.iter().map(Prim::of).collect()
    }

    /// One step that ends no later than `t_end` (and exactly on it when the
    /// chosen step would overshoot).
    fn step_until_at_most(&mut self, t_end: f64) -> Result<HemStepReport, HemError> {
        let cap = t_end - self.time_s;
        let prims0 = self.prims();
        let asm0 = self.assemble(&prims0)?;
        let rate = asm0.courant_rate;

        let (dt, courant) = match self.numerics.time_step {
            TimeStepControl::Fixed(dt) => {
                let dt = dt.get::<second>();
                let co = rate * dt;
                if co > FIXED_STEP_COURANT_LIMIT {
                    return Err(HemError::CourantViolation {
                        courant: co,
                        limit: FIXED_STEP_COURANT_LIMIT,
                    });
                }
                let dt = dt.min(cap);
                (dt, rate * dt)
            }
            TimeStepControl::AdaptiveCourant { courant, max_dt } => {
                let max_co = courant.get::<ratio>();
                let target = if rate > 0.0 {
                    max_co / rate
                } else {
                    f64::INFINITY
                };
                // setDeltaT.H: reduce immediately, grow by at most
                // min(dt_old + 0.1·target, 1.2·dt_old).
                let dt = match self.last_dt {
                    Some(prev) => target.min(prev + 0.1 * target).min(1.2 * prev),
                    None => target,
                };
                let dt = dt.min(max_dt.get::<second>()).min(cap);
                (dt, rate * dt)
            }
        };

        let n = self.geometry.n_cells();
        let q_old = self.q.clone();
        let mut q = self.q.clone();
        let mut work = std::mem::replace(&mut self.work, SspWorkspace::new(0));
        let mut hints: Vec<f64> = prims0.iter().map(|c| c.p).collect();
        let mut err: Option<HemError> = None;
        let mut first = Some(asm0);
        let mut break_choked = false;
        let t0 = self.time_s;
        let integrated = {
            let this = &*self;
            let mut rhs = |_t: f64, qs: &[f64], dq: &mut [f64]| {
                if err.is_some() {
                    dq.fill(0.0);
                    return;
                }
                let asm = match first.take() {
                    Some(a) => {
                        break_choked = a.break_choked;
                        Ok(a)
                    }
                    None => this.decode(qs, &hints).and_then(|cells| {
                        for (h, c) in hints.iter_mut().zip(&cells) {
                            *h = c.pressure.get::<pascal>();
                        }
                        let p: Vec<Prim> = cells.iter().map(Prim::of).collect();
                        this.assemble(&p)
                    }),
                };
                match asm {
                    Ok(a) => this.rhs_from(&a, n, dq),
                    Err(e) => {
                        err = Some(e);
                        dq.fill(0.0);
                    }
                }
            };
            let method = match self.numerics.time_integrator {
                TimeIntegrator::ForwardEuler => None,
                TimeIntegrator::Ssp22 => Some(SspMethod::Ssp22),
                TimeIntegrator::Ssp33 => Some(SspMethod::Ssp33),
                TimeIntegrator::Ssp104 => Some(SspMethod::Ssp104),
            };
            match method {
                None => {
                    let mut k = vec![0.0; q.len()];
                    rhs(t0, &q, &mut k);
                    for (qi, ki) in q.iter_mut().zip(&k) {
                        *qi += dt * ki;
                    }
                    Ok(())
                }
                // Sizes always match (the workspace is built for this state),
                // so petir's only errors cannot occur; they are mapped anyway.
                Some(m) => ssp_step(m, &mut rhs, t0, &mut q, dt, &mut work).map_err(|e| {
                    HemError::InvalidInput(format!("SSP step rejected the state: {e:?}"))
                }),
            }
        };
        self.work = work;
        if let Some(e) = err.or(integrated.err()) {
            self.q = q_old;
            return Err(e);
        }
        // Close the new state now, so an unphysical result fails THIS step
        // (and is rolled back) rather than the next one.
        match self.decode(&q, &hints) {
            Ok(cells) => {
                self.q = q;
                self.cells = cells;
            }
            Err(e) => {
                self.q = q_old;
                return Err(e);
            }
        }
        self.time_s = if dt == cap { t_end } else { t0 + dt };
        self.last_dt = Some(dt);
        let min_p = self
            .cells
            .iter()
            .map(|c| c.pressure.get::<pascal>())
            .fold(f64::INFINITY, f64::min);
        Ok(HemStepReport {
            time: self.time(),
            dt: Time::new::<second>(dt),
            courant: Ratio::new::<ratio>(courant),
            min_pressure: Pressure::new::<pascal>(min_p),
            break_choked,
        })
    }

    /// Close every cell of the conserved vector `q`.
    fn decode(&self, q: &[f64], hints: &[f64]) -> Result<Vec<HemCellState>, HemError> {
        (0..self.geometry.n_cells())
            .map(|i| {
                let rho = q[3 * i];
                if !(rho.is_finite() && rho > 0.0) {
                    return Err(HemError::Unphysical {
                        cell: i,
                        quantity: "density",
                        value: rho,
                    });
                }
                let u = q[3 * i + 1] / rho;
                let e = q[3 * i + 2] / rho - 0.5 * u * u;
                if !e.is_finite() {
                    return Err(HemError::Unphysical {
                        cell: i,
                        quantity: "specific internal energy",
                        value: e,
                    });
                }
                let t = self
                    .eos
                    .state_rho_e(
                        MassDensity::new::<kilogram_per_cubic_meter>(rho),
                        AvailableEnergy::new::<joule_per_kilogram>(e),
                        Some(Pressure::new::<pascal>(hints[i])),
                    )
                    .map_err(|err| err.in_cell(i))?;
                let p = t.pressure.get::<pascal>();
                if !(p.is_finite() && p > 0.0) {
                    return Err(HemError::Unphysical {
                        cell: i,
                        quantity: "pressure",
                        value: p,
                    });
                }
                Ok(HemCellState::new(t, u, self.eos.dynamic_viscosity(&t)))
            })
            .collect()
    }

    /// Boundary face state at `end`, given its end cell `o`. Returns the face
    /// state and whether a break there is choked.
    fn boundary_face(&self, end: PipeEnd, o: &Prim) -> Result<(Prim, bool), HemError> {
        let bc = self.boundary(end);
        // Outward unit normal along x: −1 at the inlet, +1 at the outlet.
        let out = match end {
            PipeEnd::Inlet => -1.0,
            PipeEnd::Outlet => 1.0,
        };
        let cell = match end {
            PipeEnd::Inlet => 0,
            PipeEnd::Outlet => self.geometry.n_cells() - 1,
        };
        let h_o = o.e + o.p / o.rho;
        let from_ph = |p: f64, h: f64, u: f64| -> Result<Prim, HemError> {
            let t = self
                .eos
                .state_p_h(
                    Pressure::new::<pascal>(p),
                    AvailableEnergy::new::<joule_per_kilogram>(h),
                )
                .map_err(|e| e.in_cell(cell))?;
            Ok(Prim {
                rho: t.density.get::<kilogram_per_cubic_meter>(),
                u,
                p,
                e: t.specific_internal_energy.get::<joule_per_kilogram>(),
                c: t.equilibrium_sound_speed.get::<meter_per_second>(),
                mu: o.mu,
                t: t.temperature.get::<kelvin>(),
            })
        };
        Ok(match bc {
            HemBoundary::Wall => (Prim { u: 0.0, ..*o }, false),
            HemBoundary::Transmissive | HemBoundary::Flux { .. } => (*o, false),
            HemBoundary::FixedPressure(p) => (from_ph(p.get::<pascal>(), h_o, o.u)?, false),
            HemBoundary::Reservoir {
                stagnation_pressure,
                stagnation_enthalpy,
            } => {
                let p0 = stagnation_pressure.get::<pascal>();
                let h0 = stagnation_enthalpy.get::<joule_per_kilogram>();
                let inflow = o.u * out < 0.0;
                if inflow {
                    let ke = 0.5 * o.u * o.u;
                    (from_ph(p0 - o.rho * ke, h0 - ke, o.u)?, false)
                } else {
                    (from_ph(p0, h_o, o.u)?, false)
                }
            }
            HemBoundary::ChokedBreak {
                area_fraction,
                back_pressure,
            } => {
                let flow = self
                    .eos
                    .break_flow(
                        Pressure::new::<pascal>(o.p),
                        AvailableEnergy::new::<joule_per_kilogram>(h_o + 0.5 * o.u * o.u),
                        back_pressure,
                    )
                    .map_err(|e| e.in_cell(cell))?;
                let g = flow.mass_flux.get::<kilogram_per_square_meter_second>()
                    * area_fraction.get::<ratio>();
                (
                    Prim {
                        u: out * g / o.rho,
                        ..*o
                    },
                    flow.choked,
                )
            }
        })
    }

    /// Wall friction `−ρ f u|u|/(2D_h)` (RELAP-7 Eq 24, the RELAP5 four-region
    /// factor) and gravity `−ρ g sinθ` in momentum, gravity work `−ρ u g
    /// sinθ` in energy (wall friction does no work on the total energy: the
    /// wall does not move). Per unit volume.
    fn cell_sources(&self, cells: &[Prim]) -> Vec<(f64, f64)> {
        const G: f64 = 9.80665;
        let geo = &self.geometry;
        let g_sin = G * geo.incline_rad.r_sin();
        cells
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let friction = if self.wall_friction && c.mu > 0.0 {
                    let x = geo.cell_centre(i);
                    let d_h = geo.hydraulic_diameter_at(x).get::<meter>();
                    c.rho
                        * wall_friction_acceleration(c.u, c.rho, c.mu, d_h, geo.roughness_m / d_h, geo.shape_factor)
                } else {
                    0.0
                };
                let x = geo.cell_centre(i);
                let wall_flux = match self.wall_heat {
                    WallHeat::Adiabatic => 0.0,
                    WallHeat::HeatFlux(q) => q.get::<watt_per_square_meter>(),
                    WallHeat::WallTemperature { temperature, heat_transfer_coefficient } => {
                        heat_transfer_coefficient.get::<watt_per_square_meter_kelvin>()
                            * (temperature.get::<kelvin>() - c.t)
                    }
                };
                let heat = if wall_flux != 0.0 {
                    wall_flux * geo.wall_area_per_length_at(x).get::<meter>()
                        / geo.flow_area_at(x).get::<square_meter>()
                } else {
                    0.0
                };
                let cell_heat = self.cell_heating_w_m3.get(i).copied().unwrap_or(0.0);
                (-friction - c.rho * g_sin, -c.rho * c.u * g_sin + heat + self.volumetric_heating_w_m3 + cell_heat)
            })
            .collect()
    }

    /// Face fluxes and Courant rate of a decoded state.
    fn assemble(&self, cells: &[Prim]) -> Result<Assembly, HemError> {
        let n = cells.len();
        let (b_in, ch_in) = self.boundary_face(PipeEnd::Inlet, &cells[0])?;
        let (b_out, ch_out) = self.boundary_face(PipeEnd::Outlet, &cells[n - 1])?;

        // Reconstruct as rhoCentralFoam.C does: ρ, ρU, rPsi = p/ρ, e and c.
        let lim = self.numerics.limiter;
        let (rho_p, rho_n) = muscl_1d(&with_ghosts(cells, &b_in, &b_out, |c| c.rho), lim);
        let (m_p, m_n) = muscl_1d(&with_ghosts(cells, &b_in, &b_out, |c| c.rho * c.u), lim);
        let (rpsi_p, rpsi_n) = muscl_1d(&with_ghosts(cells, &b_in, &b_out, |c| c.p / c.rho), lim);
        let (e_p, e_n) = muscl_1d(&with_ghosts(cells, &b_in, &b_out, |c| c.e), lim);
        let (c_p, c_n) = muscl_1d(&with_ghosts(cells, &b_in, &b_out, |c| c.c), lim);

        let nx = Vector3::new(1.0, 0.0, 0.0);
        let mut flux = Vec::with_capacity(n + 1);
        let mut amax = Vec::with_capacity(n + 1);

        let b_in_s = b_in.knp();
        flux.push(boundary_flux(self.inlet, &b_in_s, nx));
        amax.push(central_flux_speed_max(&b_in_s, &b_in_s, nx));
        for f in 0..n - 1 {
            let side = |rho: f64, m: f64, rpsi: f64, e: f64, c: f64| {
                KnpFaceState::total_energy(rho, Vector3::new(m / rho, 0.0, 0.0), rho * rpsi, c, e)
            };
            let l = side(rho_p[f], m_p[f], rpsi_p[f], e_p[f], c_p[f]);
            let r = side(rho_n[f], m_n[f], rpsi_n[f], e_n[f], c_n[f]);
            flux.push(knp_face_flux(&l, &r, nx));
            amax.push(central_flux_speed_max(&l, &r, nx));
        }
        let b_out_s = b_out.knp();
        flux.push(boundary_flux(self.outlet, &b_out_s, nx));
        amax.push(central_flux_speed_max(&b_out_s, &b_out_s, nx));

        // centralCourantNo.H: ½·max_cells(Σ_faces amaxSf / V); per unit Δt.
        // With a varying area each face is weighted by A_f/A_i (exactly 1 for
        // a uniform pipe, so that case is unchanged bit for bit).
        let g = &self.geometry;
        let dx = g.dx();
        let courant_rate = (0..n)
            .map(|i| {
                let ai = g.cell_area_m2(i);
                0.5 * (amax[i] * (g.face_area_m2(i) / ai) + amax[i + 1] * (g.face_area_m2(i + 1) / ai)) / dx
            })
            .fold(0.0, f64::max);
        let choked =
            |bc: HemBoundary, ch: bool| matches!(bc, HemBoundary::ChokedBreak { .. }) && ch;
        Ok(Assembly {
            flux,
            cell_pressure: cells.iter().map(|c| c.p).collect(),
            cell_source: self.cell_sources(cells),
            courant_rate,
            break_choked: choked(self.inlet, ch_in) || choked(self.outlet, ch_out),
        })
    }

    /// `V_i dW_i/dt = −(F A)_{i+½} + (F A)_{i−½} + [0, p_i (A_{i+½} −
    /// A_{i−½}), 0]`, the quasi-1-D finite-volume form (gh:#1058), plus the
    /// boundary bookkeeping. The wall-pressure force uses the cell pressure, so
    /// a fluid at rest at uniform pressure stays at rest exactly. For a uniform
    /// pipe the ratios `A_f/A_i` are exactly 1 and the force exactly 0, so the
    /// result is the constant-area `−(F_{i+½} − F_{i−½})/Δx` bit for bit.
    fn rhs_from(&self, a: &Assembly, n: usize, dq: &mut [f64]) {
        let g = &self.geometry;
        let dx = g.dx();
        for i in 0..n {
            let (fw, fe) = (&a.flux[i], &a.flux[i + 1]);
            let ai = g.cell_area_m2(i);
            let (aw, ae) = (g.face_area_m2(i), g.face_area_m2(i + 1));
            let (rw, re) = (aw / ai, ae / ai);
            dq[3 * i] = -(fe.mass * re - fw.mass * rw) / dx;
            dq[3 * i + 1] =
                -(fe.momentum.x * re - fw.momentum.x * rw) / dx + a.cell_pressure[i] * (re - rw) / dx;
            dq[3 * i + 2] = -(fe.energy * re - fw.energy * rw) / dx;
            // Added only when non-zero, so a frictionless horizontal pipe is
            // the source-free solver bit for bit.
            let (sm, se) = a.cell_source[i];
            if sm != 0.0 {
                dq[3 * i + 1] += sm;
            }
            if se != 0.0 {
                dq[3 * i + 2] += se;
            }
        }
        dq[3 * n] = a.flux[0].mass * g.face_area_m2(0);
        dq[3 * n + 1] = a.flux[0].energy * g.face_area_m2(0);
        dq[3 * n + 2] = a.flux[n].mass * g.face_area_m2(n);
        dq[3 * n + 3] = a.flux[n].energy * g.face_area_m2(n);
    }
}

/// The flux through a boundary face: the prescribed one for
/// [`HemBoundary::Flux`], otherwise the physical flux of the face state.
fn boundary_flux(bc: HemBoundary, face: &KnpFaceState, nx: Vector3) -> KnpFaceFlux {
    match bc {
        HemBoundary::Flux {
            mass_flux,
            momentum_flux,
            energy_flux,
        } => KnpFaceFlux {
            mass: mass_flux.get::<kilogram_per_square_meter_second>(),
            momentum: Vector3::new(momentum_flux.get::<pascal>(), 0.0, 0.0),
            energy: energy_flux.get::<watt_per_square_meter>(),
        },
        _ => physical_face_flux(face, nx),
    }
}

fn validate_numerics(nm: &HemNumerics) -> Result<(), HemError> {
    match nm.time_step {
        TimeStepControl::Fixed(dt) => {
            let v = dt.get::<second>();
            if !(v.is_finite() && v > 0.0) {
                return Err(HemError::InvalidInput(format!(
                    "fixed time step {v} s must be > 0"
                )));
            }
        }
        TimeStepControl::AdaptiveCourant { courant, max_dt } => {
            let c = courant.get::<ratio>();
            let m = max_dt.get::<second>();
            if !(c.is_finite() && c > 0.0) {
                return Err(HemError::InvalidInput(format!(
                    "target Courant number {c} must be > 0"
                )));
            }
            if !(m > 0.0) {
                return Err(HemError::InvalidInput(format!("max_dt {m} s must be > 0")));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod test_access {
    //! Crate-internal hooks for the unit tests.
    pub(crate) use super::muscl_1d;
}
