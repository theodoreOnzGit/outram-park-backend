//! PCICE: a pressure-corrected, semi-implicit solver for 1-D compressible flow
//! on a general equation of state, finite volume (gh:#1068).
//!
//! > Research, education and V&V only. AI draft, unreviewed. Not for nuclear
//! > facility operation, reactor control, licensing or safety-critical
//! > decisions. See `RESPONSIBLE_USE.md`.
//!
//! # What this module answers
//!
//! *How do we march the compressible Euler equations with a time step set by
//! the flow speed `|u|` instead of the sound speed `|u| + c`?* An explicit
//! density-based scheme such as [`crate::hem::HemPipe`] (KNP) must resolve the
//! acoustic waves, so at low Mach number it takes `c/|u|` times more steps than
//! the material motion needs. PCICE treats the pressure implicitly, so the
//! step is limited by the material Courant number alone.
//!
//! # References
//!
//! - R. C. Martineau and R. A. Berry, "The pressure-corrected ICE finite element
//!   method for compressible flows on unstructured meshes", *J. Comput. Phys.*
//!   198 (2004) 659–685. The algorithm (explicit predictor without the pressure
//!   terms, a Crank–Nicolson pressure target reached through a pressure
//!   Poisson / Helmholtz equation closed by the EOS, then the corrected mass,
//!   momentum and total energy) is theirs; their discretisation is finite
//!   element, this one is a cell-centred finite volume.
//! - RELAP-7 Theory Manual, §5.3 (Eqs 257–274), which describes PCICE for the
//!   RELAP-7 single-phase equations. The manual is not public: it is cited by
//!   section and equation only, nothing is transcribed from it. This module was
//!   written from the open paper above and from first principles (clean room;
//!   no RELAP source was read).
//!
//! # The equations
//!
//! The quasi-1-D Euler equations in a duct of area `A(x)`:
//!
//! - `∂(ρA)/∂t + ∂(ρuA)/∂x = 0`
//! - `∂(ρuA)/∂t + ∂((ρu² + p)A)/∂x = p ∂A/∂x`
//! - `∂(ρEA)/∂t + ∂((ρE + p)uA)/∂x = 0`, `E = e + ½u²`
//!
//! closed by `p = p(ρ, e)` from any [`HemEquationOfState`] (ideal gas,
//! IAPWS-IF97, CoolProp through [`crate::hem::HemEos`], the reduced
//! verification closures).
//!
//! # The algorithm, per step `Δt` (cell `i`, face `f`, volume `V_i = A_i Δx`)
//!
//! 1. **Explicit predictor, two-stage Runge–Kutta (Heun / SSP22).** Mass,
//!    momentum and total energy are advanced with their *advective* fluxes
//!    only, `ρu`, `ρu²`, `ρEu`: no pressure in the momentum flux, no pressure
//!    work in the energy flux. Face values are MUSCL-reconstructed with
//!    `outram_foam_basic_lib`'s [`FluxLimiter`] (the uniform-mesh form of
//!    `fvc::reconstruct_pos_neg`, as in `HemPipe`), and the face flux is a
//!    central flux with a local Lax–Friedrichs dissipation at the **material**
//!    speed `a = max(|u_L|, |u_R|)`, never `|u| + c`. The stage-averaged face
//!    fluxes `G*`, `F_m*`, `F_E*` are kept.
//! 2. **Crank–Nicolson pressure target.** The pressure that acts over the step
//!    is `p^θ = (1 − θ) pⁿ + θ pⁿ⁺¹`, `θ = ½` by default
//!    ([`PciceNumerics::implicitness`]).
//! 3. **Intermediate momentum and the corrected mass flux.** The face mass
//!    flux is the time average of `(ρu)ⁿ` and `(ρu)ⁿ⁺¹`, which is the predictor
//!    flux plus the pressure impulse over half the step:
//!    `G_f = G*_f − ½Δt (p^θ_R − p^θ_L)/d_f` (`d_f = Δx`, `Δx/2` at a pressure
//!    boundary). The compact face gradient is the Rhie–Chow-like coupling that
//!    keeps a collocated grid free of odd–even pressure modes. Momentum
//!    and energy fluxes follow: `F_m = F_m* + (G − G*) u*_f + p^θ_f`,
//!    `F_E = F_E* + (G − G*) E*_f + p^θ_f G/ρ*_f`, and the cell update is
//!    conservative, `V_i Δq_i = −Δt Σ_f ±A_f F_f` (+ `Δt p^θ_i (A_{i+½} − A_{i−½})`
//!    in momentum).
//! 4. **Pressure-correction (Helmholtz) equation.** Linearise
//!    `pⁿ⁺¹ = p(ρ, ρE, ρu)` about the current iterate `p^k`:
//!    `δp_i − A_i δρ_i − B_i δ(ρE)_i − C_i δ(ρu)_i = p_EOS,i − p^k_i`, with
//!    `A = (∂p/∂ρ)_e − (∂p/∂e)_ρ (e − ½u²)/ρ`, `B = (∂p/∂e)_ρ/ρ`,
//!    `C = −(∂p/∂e)_ρ u/ρ`, and every `δq_i` a linear function of
//!    `δp_{i−1}, δp_i, δp_{i+1}` through step 3. That is a tridiagonal system,
//!    solved by [`crate::multiphase_1d::thomas_solve`]. For an ideal gas at
//!    rest it reduces to `δp − (cΔt/2Δx)² Δ²δp = r`, the Crank–Nicolson wave
//!    operator. **The EOS derivatives are finite differences** of
//!    [`HemEquationOfState::state_rho_e`] (forward, relative step `1e-6`),
//!    because the trait supplies no derivatives. They only shape the
//!    correction; the residual uses the EOS itself.
//! 5. **Two correction passes** ([`PciceNumerics::correction_passes`]): the
//!    EOS, its derivatives and the lagged face coefficients are re-evaluated
//!    at the new pressure and step 4 is solved again.
//! 6. **Energy update and closure.** With the final iterate, mass, momentum and
//!    total energy are updated once more (step 3) with the corrected mass flux
//!    and the time-centred pressure, and the new pressure is taken from the EOS
//!    at the new `(ρ, e)`, so the carried state is thermodynamically
//!    consistent. What is left of `p_EOS − p^k` is reported as
//!    [`PciceStepReport::pressure_residual`].
//!
//! Every update is in flux form, so with two walls total mass and total energy
//! are conserved to round-off.
//!
//! # Time step
//!
//! `Δt = C_mat Δx / max|u|` ([`PciceNumerics::material_courant`]), capped by
//! [`PciceNumerics::max_dt`]. An optional acoustic cap
//! ([`PciceNumerics::acoustic_courant_cap`]) is for problems where the
//! acoustic waves themselves are the answer (shock tubes); without it the
//! acoustic Courant number can be far above one, which is the method's point.
//!
//! **Limit (measured, see the V&V record):** a fluid that starts at rest
//! against a strong pressure jump has no material speed to set the first step,
//! so `max_dt` sets it; on Sod with the default 1 ms (acoustic Courant ≈ 3.7)
//! the first step drives a density negative and is refused. Use the acoustic
//! cap, or a small `max_dt`, for such starts.
//!
//! # Boundaries ([`PciceBoundary`])
//!
//! - `Wall`: zero mass and energy flux, the cell pressure on the wall.
//! - `FixedPressure(p)`: the face pressure is `p`; the reconstruction ghost is
//!   the end cell (zero gradient), as `HemPipe` does.
//! - `Reservoir { p0, h0 }`: for inflow the face takes `p = p0 − ½ρu²` and
//!   `h = h0 − ½u²` with the end cell's velocity, density from the EOS
//!   (`state_p_h`); for outflow `p = p0` with the end cell's state.
//!
//! # What is not here
//!
//! Friction, gravity, heat sources, choked breaks and junctions (all in
//! `HemPipe`). Variable area is implemented (quasi-1-D, with the wall
//! pressure force) and verified only for a duct at rest.
//!
//! # V&V
//!
//! `crates/tampines/tests/pcice.rs`, recorded with methodology and results in
//! `crates/tampines/verification_and_validation/pcice.md`. The case setups are
//! public in [`cases`] so gh:#1069 (the finite-element comparison) can reuse
//! them.

pub mod cases;

use tampines_steam_tables::hem::{
    FluxLimiter, HemCellState, HemEquationOfState, HemError, HemInitialCondition,
    HemPipeGeometry, HemSnapshot, HemThermoState, PipeEnd, PrimitiveState,
};
use uom::si::available_energy::joule_per_kilogram;
use uom::si::energy::joule;
use uom::si::f64::{
    AvailableEnergy, Energy, Length, Mass, MassDensity, Pressure, Ratio, Time, Velocity,
};
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

use crate::multiphase_1d::thomas_solve;
use crate::TampinesError;

/// Relative step of the finite-difference EOS derivatives.
const FD_REL_STEP: f64 = 1.0e-6;

/// A boundary condition at one end of a [`PcicePipe`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PciceBoundary {
    /// Closed rigid end: no mass or energy flux; the end cell's pressure acts
    /// on the wall.
    Wall,
    /// Fixed static pressure at the face; inflow brings the end cell's
    /// density and energy (zero gradient).
    FixedPressure(Pressure),
    /// Stagnation reservoir `(p0, h0)`: inflow at `p = p0 − ½ρu²`,
    /// `h = h0 − ½u²`; outflow at `p = p0`.
    Reservoir {
        /// Stagnation pressure `p0`.
        stagnation_pressure: Pressure,
        /// Stagnation specific enthalpy `h0`, J/kg.
        stagnation_enthalpy: AvailableEnergy,
    },
}

/// Numerical settings of a [`PcicePipe`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PciceNumerics {
    /// Target material Courant number `max|u| Δt/Δx`. Default `0.5`.
    pub material_courant: Ratio,
    /// Optional cap on the acoustic Courant number `max(|u| + c) Δt/Δx`.
    /// Default `None`: the step is set by the material speed only.
    pub acoustic_courant_cap: Option<Ratio>,
    /// Largest step allowed (also the step of a fluid at rest). Default 1 ms.
    pub max_dt: Time,
    /// Reconstruction limiter of the predictor. Default van Leer.
    pub limiter: FluxLimiter,
    /// Pressure-correction passes per step, `≥ 1`. Default `2`.
    pub correction_passes: usize,
    /// Time weight `θ` of the new pressure, in `[½, 1]`: `½` is Crank–Nicolson
    /// (the default, PCICE's choice), `1` backward Euler.
    pub implicitness: Ratio,
}

impl Default for PciceNumerics {
    fn default() -> Self {
        Self {
            material_courant: Ratio::new::<ratio>(0.5),
            acoustic_courant_cap: None,
            max_dt: Time::new::<second>(1.0e-3),
            limiter: FluxLimiter::VanLeer,
            correction_passes: 2,
            implicitness: Ratio::new::<ratio>(0.5),
        }
    }
}

/// What one [`PcicePipe::step`] did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PciceStepReport {
    /// Time at the end of the step.
    pub time: Time,
    /// Step taken.
    pub dt: Time,
    /// `max|u| Δt/Δx` at the start of the step.
    pub material_courant: Ratio,
    /// `max(|u| + c) Δt/Δx` at the start of the step.
    pub acoustic_courant: Ratio,
    /// `max_i |p_EOS,i − p^k_i| / p_EOS,i` after the last correction pass:
    /// how far the linearised pressure was from the EOS at the new state.
    pub pressure_residual: Ratio,
    /// Lowest cell pressure at the end of the step.
    pub min_pressure: Pressure,
}

/// What [`PcicePipe::run_until`] did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PciceRunSummary {
    /// Steps taken.
    pub steps: usize,
    /// Time reached (exactly the requested end time).
    pub time: Time,
    /// Largest material Courant number of any step.
    pub max_material_courant: Ratio,
    /// Largest acoustic Courant number of any step.
    pub max_acoustic_courant: Ratio,
    /// Largest pressure residual of any step.
    pub max_pressure_residual: Ratio,
    /// Lowest cell pressure at the end of any step.
    pub min_pressure: Pressure,
}

/// The PCICE pipe solver, generic over the equation of state.
///
/// Built from the same geometry and initial-condition types as
/// [`crate::hem::HemPipe`] ([`HemPipeGeometry`], [`HemInitialCondition`]) and
/// reporting the same [`HemSnapshot`], so the two solvers run the same case
/// and compare cell by cell. Both ends start as walls.
#[derive(Debug, Clone)]
pub struct PcicePipe<E: HemEquationOfState> {
    geometry: HemPipeGeometry,
    eos: E,
    numerics: PciceNumerics,
    dx: f64,
    /// Face areas, `n + 1`, m².
    area_f: Vec<f64>,
    /// Cell volumes, m³.
    vol: Vec<f64>,
    /// Conserved `[ρ, ρu, ρE]` per cell.
    q: Vec<[f64; 3]>,
    /// Closed state per cell, consistent with `q`.
    thermo: Vec<HemThermoState>,
    inlet: PciceBoundary,
    outlet: PciceBoundary,
    time_s: f64,
    /// Net mass and energy that entered through the boundaries, kg and J.
    net_mass_in: f64,
    net_energy_in: f64,
}

/// A boundary face as one step sees it.
#[derive(Debug, Clone, Copy)]
enum BFace {
    Wall,
    /// Face pressure `p_b`; `ghost` the reconstruction state outside, or `None`
    /// for zero gradient (the end cell of whatever state is reconstructed).
    Pressure { p_b: f64, ghost: Option<[f64; 3]> },
}

/// One face's advective flux and its upwind face state.
#[derive(Debug, Clone, Copy, Default)]
struct FaceFlux {
    g: f64,
    fm: f64,
    fe: f64,
    rho: f64,
    mom: f64,
    rhoe: f64,
}

/// The cell update produced by one pressure iterate.
struct Evaluated {
    q: Vec<[f64; 3]>,
    /// Face mass and energy fluxes per unit area (for the boundary bookkeeping).
    g: Vec<f64>,
    fe: Vec<f64>,
    /// Linearisation per face: `(dG/dp_L, dG/dp_R)`, the upwind velocity, the
    /// face total enthalpy and `G/ρ_f`, and the weights of `δp_L, δp_R` in
    /// `δp^θ_f`.
    dg: Vec<(f64, f64)>,
    dpf: Vec<(f64, f64)>,
    u_up: Vec<f64>,
    h_f: Vec<f64>,
    u_f: Vec<f64>,
}

fn in_cell(e: HemError, cell: usize) -> TampinesError {
    match e {
        HemError::EquationOfState { cell: None, detail } => {
            TampinesError::Hem(HemError::EquationOfState {
                cell: Some(cell),
                detail,
            })
        }
        other => TampinesError::Hem(other),
    }
}

fn unphysical(cell: usize, quantity: &'static str, value: f64) -> TampinesError {
    TampinesError::Hem(HemError::Unphysical {
        cell,
        quantity,
        value,
    })
}

/// Close a primitive initial state with `eos`. `HemPipe`'s own closure is
/// private to `tampines-steam-tables`, so the three-way match is repeated here.
fn close_primitive<E: HemEquationOfState>(
    eos: &E,
    s: &PrimitiveState,
) -> Result<(HemThermoState, f64), HemError> {
    match *s {
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

/// Central flux with local Lax–Friedrichs dissipation at the material speed.
fn face_flux(ql: [f64; 3], qr: [f64; 3]) -> FaceFlux {
    let ul = ql[1] / ql[0];
    let ur = qr[1] / qr[0];
    let a = ul.abs().max(ur.abs());
    let ub = 0.5 * (ul + ur);
    let up = |l: f64, r: f64| {
        if ub > 0.0 {
            l
        } else if ub < 0.0 {
            r
        } else {
            0.5 * (l + r)
        }
    };
    FaceFlux {
        g: 0.5 * (ql[1] + qr[1]) - 0.5 * a * (qr[0] - ql[0]),
        fm: 0.5 * (ql[1] * ul + qr[1] * ur) - 0.5 * a * (qr[1] - ql[1]),
        fe: 0.5 * (ql[2] * ul + qr[2] * ur) - 0.5 * a * (qr[2] - ql[2]),
        rho: up(ql[0], qr[0]),
        mom: up(ql[1], qr[1]),
        rhoe: up(ql[2], qr[2]),
    }
}

/// MUSCL face values `(owner side, neighbour side)` of one variable at the
/// face between `po` and `pn`, with `pm` behind the owner and `pnn` beyond
/// the neighbour. The uniform-mesh form of
/// `outram_foam_basic_lib::fv_operators::fvc::reconstruct_pos_neg`, the same
/// as `tampines_steam_tables`' (crate-private) `muscl_1d`.
fn muscl_face(pm: f64, po: f64, pn: f64, pnn: f64, limiter: FluxLimiter) -> (f64, f64) {
    const TINY: f64 = 1e-30;
    let gradf = pn - po;
    if gradf.abs() < TINY {
        return (po, pn);
    }
    let lin = 0.5 * (po + pn);
    let r_pos = (po - pm) / gradf;
    let r_neg = (pnn - pn) / gradf;
    (
        po + limiter.psi(r_pos) * (lin - po),
        pn + limiter.psi(r_neg) * (lin - pn),
    )
}

impl<E: HemEquationOfState> PcicePipe<E> {
    /// Build the pipe and close its initial state with `eos`. Both ends start
    /// as [`PciceBoundary::Wall`].
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for invalid numerics or a per-cell
    /// condition of the wrong length; the EOS's error for a state it cannot
    /// close.
    pub fn new(
        geometry: HemPipeGeometry,
        eos: E,
        initial: HemInitialCondition,
        numerics: PciceNumerics,
    ) -> Result<Self, TampinesError> {
        validate_numerics(&numerics)?;
        let n = geometry.n_cells();
        let states: Vec<PrimitiveState> = match initial {
            HemInitialCondition::Uniform(s) => vec![s; n],
            HemInitialCondition::Riemann {
                diaphragm,
                left,
                right,
            } => (0..n)
                .map(|i| {
                    if geometry.cell_centre(i) < diaphragm {
                        left
                    } else {
                        right
                    }
                })
                .collect(),
            HemInitialCondition::PerCell(v) => {
                if v.len() != n {
                    return Err(TampinesError::InvalidInput(format!(
                        "per-cell initial condition has {} states for {n} cells",
                        v.len()
                    )));
                }
                v
            }
        };
        let dx = geometry.cell_width().get::<meter>();
        let area_f: Vec<f64> = (0..=n)
            .map(|f| {
                geometry
                    .flow_area_at(Length::new::<meter>(f as f64 * dx))
                    .get::<uom::si::area::square_meter>()
            })
            .collect();
        let vol: Vec<f64> = (0..n).map(|i| 0.5 * (area_f[i] + area_f[i + 1]) * dx).collect();
        let mut q = Vec::with_capacity(n);
        let mut thermo = Vec::with_capacity(n);
        for (i, s) in states.iter().enumerate() {
            let (t, u) = close_primitive(&eos, s).map_err(|e| in_cell(e, i))?;
            let rho = t.density.get::<kilogram_per_cubic_meter>();
            let e = t.specific_internal_energy.get::<joule_per_kilogram>();
            q.push([rho, rho * u, rho * (e + 0.5 * u * u)]);
            thermo.push(t);
        }
        let mut pipe = Self {
            geometry,
            eos,
            numerics,
            dx,
            area_f,
            vol,
            q,
            thermo,
            inlet: PciceBoundary::Wall,
            outlet: PciceBoundary::Wall,
            time_s: 0.0,
            net_mass_in: 0.0,
            net_energy_in: 0.0,
        };
        // Close the carried state once through (ρ, e), so the solver starts
        // from exactly what it will decode.
        let hints: Vec<f64> = pipe.thermo.iter().map(|t| t.pressure.get::<pascal>()).collect();
        pipe.thermo = pipe.close_all(&pipe.q, &hints)?;
        Ok(pipe)
    }

    /// Set the boundary condition at one end (effective from the next step).
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for a non-positive or non-finite
    /// pressure.
    pub fn set_boundary(
        &mut self,
        end: PipeEnd,
        boundary: PciceBoundary,
    ) -> Result<(), TampinesError> {
        let p = match boundary {
            PciceBoundary::Wall => None,
            PciceBoundary::FixedPressure(p) => Some(p),
            PciceBoundary::Reservoir {
                stagnation_pressure,
                ..
            } => Some(stagnation_pressure),
        };
        if let Some(p) = p {
            let v = p.get::<pascal>();
            if !(v.is_finite() && v > 0.0) {
                return Err(TampinesError::InvalidInput(format!(
                    "boundary pressure {v} Pa must be > 0"
                )));
            }
        }
        match end {
            PipeEnd::Inlet => self.inlet = boundary,
            PipeEnd::Outlet => self.outlet = boundary,
        }
        Ok(())
    }

    /// The boundary condition at one end.
    pub fn boundary(&self, end: PipeEnd) -> PciceBoundary {
        match end {
            PipeEnd::Inlet => self.inlet,
            PipeEnd::Outlet => self.outlet,
        }
    }

    /// The geometry.
    pub fn geometry(&self) -> &HemPipeGeometry {
        &self.geometry
    }

    /// The equation of state.
    pub fn equation_of_state(&self) -> &E {
        &self.eos
    }

    /// The numerical settings.
    pub fn numerics(&self) -> &PciceNumerics {
        &self.numerics
    }

    /// Simulation time.
    pub fn time(&self) -> Time {
        Time::new::<second>(self.time_s)
    }

    /// Every cell's state, in the same form as [`crate::hem::HemPipe`]'s.
    pub fn snapshot(&self) -> HemSnapshot {
        let cells = self
            .q
            .iter()
            .zip(&self.thermo)
            .map(|(q, t)| HemCellState {
                pressure: t.pressure,
                temperature: t.temperature,
                density: t.density,
                velocity: Velocity::new::<meter_per_second>(q[1] / q[0]),
                specific_internal_energy: t.specific_internal_energy,
                specific_enthalpy: t.specific_enthalpy,
                equilibrium_sound_speed: t.equilibrium_sound_speed,
                quality: t.quality,
                void_fraction: t.void_fraction,
                dynamic_viscosity: self.eos.dynamic_viscosity(t),
            })
            .collect();
        HemSnapshot {
            time: self.time(),
            cell_centres: (0..self.q.len()).map(|i| self.geometry.cell_centre(i)).collect(),
            cells,
        }
    }

    /// Total mass in the pipe, `Σ ρ_i V_i`.
    pub fn total_mass(&self) -> Mass {
        Mass::new::<kilogram>(self.q.iter().zip(&self.vol).map(|(q, v)| q[0] * v).sum())
    }

    /// Total (internal + kinetic) energy in the pipe, `Σ ρE_i V_i`.
    pub fn total_energy(&self) -> Energy {
        Energy::new::<joule>(self.q.iter().zip(&self.vol).map(|(q, v)| q[2] * v).sum())
    }

    /// Net mass that has entered through both ends since construction
    /// (inlet in minus outlet out).
    pub fn net_mass_inflow(&self) -> Mass {
        Mass::new::<kilogram>(self.net_mass_in)
    }

    /// Net total energy (`ρuH` flux) that has entered through both ends.
    pub fn net_energy_inflow(&self) -> Energy {
        Energy::new::<joule>(self.net_energy_in)
    }

    /// The step the controller would take now: material Courant, capped by
    /// `max_dt` and by the acoustic cap when one is set.
    pub fn stable_time_step(&self) -> Time {
        let (umax, wmax) = self.speeds();
        let nm = &self.numerics;
        let mut dt = nm.max_dt.get::<second>();
        if umax > 0.0 {
            dt = dt.min(nm.material_courant.get::<ratio>() * self.dx / umax);
        }
        if let Some(cap) = nm.acoustic_courant_cap {
            dt = dt.min(cap.get::<ratio>() * self.dx / wmax);
        }
        Time::new::<second>(dt)
    }

    /// Take one step of [`Self::stable_time_step`].
    ///
    /// # Errors
    ///
    /// As [`Self::step_by`].
    pub fn step(&mut self) -> Result<PciceStepReport, TampinesError> {
        let dt = self.stable_time_step();
        self.step_by(dt)
    }

    /// Take one step of `dt`. The material Courant number is not checked: a
    /// step above the stable one is the caller's choice. On error the state is
    /// unchanged.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for a non-positive `dt`;
    /// [`HemError::Unphysical`] (in [`TampinesError::Hem`]) for a non-positive
    /// density or pressure; the EOS's errors; a singular pressure matrix as
    /// [`TampinesError::Numerical`].
    pub fn step_by(&mut self, dt: Time) -> Result<PciceStepReport, TampinesError> {
        let dt = dt.get::<second>();
        if !(dt.is_finite() && dt > 0.0) {
            return Err(TampinesError::InvalidInput(format!(
                "time step {dt} s must be > 0"
            )));
        }
        let n = self.q.len();
        let theta = self.numerics.implicitness.get::<ratio>();
        let (umax, wmax) = self.speeds();
        let bfaces = [self.boundary_face(PipeEnd::Inlet)?, self.boundary_face(PipeEnd::Outlet)?];

        // 1. Predictor.
        let pred = self.predict(dt, &bfaces)?;

        // 2–5. Pressure correction.
        let p_n: Vec<f64> = self.thermo.iter().map(|t| t.pressure.get::<pascal>()).collect();
        let mut p_k = p_n.clone();
        for _pass in 0..self.numerics.correction_passes {
            let ev = self.evaluate(dt, theta, &pred, &bfaces, &p_n, &p_k)?;
            let mut sub = vec![0.0; n];
            let mut diag = vec![1.0; n];
            let mut sup = vec![0.0; n];
            let mut rhs = vec![0.0; n];
            for i in 0..n {
                let [rho, mom, rhoe] = ev.q[i];
                let u = mom / rho;
                let e = rhoe / rho - 0.5 * u * u;
                if !(rho > 0.0 && rho.is_finite()) {
                    return Err(unphysical(i, "density", rho));
                }
                let (p_eos, p_rho, p_e) = self.eos_with_derivatives(rho, e, p_k[i], i)?;
                rhs[i] = p_eos - p_k[i];
                let wa = p_rho - p_e * (e - 0.5 * u * u) / rho;
                let wb = p_e / rho;
                let wc = -p_e * u / rho;
                // δq_i = −Δt/V_i [A_{i+1} δF_{i+1} − A_i δF_i] (+ source in m).
                let s = dt / self.vol[i];
                let (al, ar) = (self.area_f[i], self.area_f[i + 1]);
                let fl = i;
                let fr = i + 1;
                // Face derivative pairs (dF/dp_left-cell, dF/dp_right-cell)
                // for mass, momentum, energy.
                let dfaces = |f: usize| -> [(f64, f64); 3] {
                    let (gl, gr) = ev.dg[f];
                    let (pl, pr) = ev.dpf[f];
                    [
                        (gl, gr),
                        (gl * ev.u_up[f] + pl, gr * ev.u_up[f] + pr),
                        (gl * ev.h_f[f] + ev.u_f[f] * pl, gr * ev.h_f[f] + ev.u_f[f] * pr),
                    ]
                };
                let dl = dfaces(fl);
                let dr = dfaces(fr);
                let w = [wa, wc, wb];
                let mut c_im1 = 0.0;
                let mut c_i = 0.0;
                let mut c_ip1 = 0.0;
                for k in 0..3 {
                    c_im1 += w[k] * s * al * dl[k].0;
                    c_i += w[k] * (-s) * (ar * dr[k].0 - al * dl[k].1);
                    c_ip1 += w[k] * (-s) * ar * dr[k].1;
                }
                c_i += wc * s * (ar - al) * theta;
                sub[i] = -c_im1;
                diag[i] = 1.0 - c_i;
                sup[i] = -c_ip1;
            }
            let dp = thomas_solve(&sub, &diag, &sup, &rhs)?;
            for i in 0..n {
                p_k[i] += dp[i];
                if !(p_k[i] > 0.0 && p_k[i].is_finite()) {
                    return Err(unphysical(i, "pressure", p_k[i]));
                }
            }
        }

        // 6. Final update with the converged iterate, and the EOS closure.
        let ev = self.evaluate(dt, theta, &pred, &bfaces, &p_n, &p_k)?;
        for (i, q) in ev.q.iter().enumerate() {
            if !(q[0] > 0.0 && q[0].is_finite()) {
                return Err(unphysical(i, "density", q[0]));
            }
        }
        let thermo = self.close_all(&ev.q, &p_k)?;
        let mut residual = 0.0_f64;
        let mut p_min = f64::INFINITY;
        for (i, t) in thermo.iter().enumerate() {
            let p = t.pressure.get::<pascal>();
            if !(p > 0.0 && p.is_finite()) {
                return Err(unphysical(i, "pressure", p));
            }
            residual = residual.max((p - p_k[i]).abs() / p);
            p_min = p_min.min(p);
        }
        // Commit.
        self.net_mass_in += dt * (self.area_f[0] * ev.g[0] - self.area_f[n] * ev.g[n]);
        self.net_energy_in += dt * (self.area_f[0] * ev.fe[0] - self.area_f[n] * ev.fe[n]);
        self.q = ev.q;
        self.thermo = thermo;
        self.time_s += dt;
        Ok(PciceStepReport {
            time: self.time(),
            dt: Time::new::<second>(dt),
            material_courant: Ratio::new::<ratio>(umax * dt / self.dx),
            acoustic_courant: Ratio::new::<ratio>(wmax * dt / self.dx),
            pressure_residual: Ratio::new::<ratio>(residual),
            min_pressure: Pressure::new::<pascal>(p_min),
        })
    }

    /// Step until `t_end` (the last step is shortened to land on it).
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] if `t_end` is before the current time;
    /// otherwise as [`Self::step_by`].
    pub fn run_until(&mut self, t_end: Time) -> Result<PciceRunSummary, TampinesError> {
        let t_end_s = t_end.get::<second>();
        if !(t_end_s.is_finite() && t_end_s >= self.time_s) {
            return Err(TampinesError::InvalidInput(format!(
                "end time {t_end_s} s is before the current time {} s",
                self.time_s
            )));
        }
        let mut summary = PciceRunSummary {
            steps: 0,
            time: self.time(),
            max_material_courant: Ratio::new::<ratio>(0.0),
            max_acoustic_courant: Ratio::new::<ratio>(0.0),
            max_pressure_residual: Ratio::new::<ratio>(0.0),
            min_pressure: Pressure::new::<pascal>(f64::INFINITY),
        };
        let tol = 1e-12 * t_end_s.max(1e-300);
        while t_end_s - self.time_s > tol {
            let dt = self.stable_time_step().get::<second>().min(t_end_s - self.time_s);
            let r = self.step_by(Time::new::<second>(dt))?;
            summary.steps += 1;
            summary.max_material_courant = summary.max_material_courant.max(r.material_courant);
            summary.max_acoustic_courant = summary.max_acoustic_courant.max(r.acoustic_courant);
            summary.max_pressure_residual = summary.max_pressure_residual.max(r.pressure_residual);
            summary.min_pressure = summary.min_pressure.min(r.min_pressure);
        }
        self.time_s = t_end_s.max(self.time_s);
        summary.time = self.time();
        Ok(summary)
    }

    // ───────────────────────── internals ─────────────────────────

    /// `(max|u|, max(|u| + c))` over the cells.
    fn speeds(&self) -> (f64, f64) {
        let mut umax = 0.0_f64;
        let mut wmax = 0.0_f64;
        for (q, t) in self.q.iter().zip(&self.thermo) {
            let u = (q[1] / q[0]).abs();
            umax = umax.max(u);
            wmax = wmax.max(u + t.equilibrium_sound_speed.get::<meter_per_second>());
        }
        (umax, wmax)
    }

    /// Close every cell's `(ρ, e)`.
    fn close_all(&self, q: &[[f64; 3]], hints: &[f64]) -> Result<Vec<HemThermoState>, TampinesError> {
        q.iter()
            .enumerate()
            .map(|(i, q)| {
                let u = q[1] / q[0];
                let e = q[2] / q[0] - 0.5 * u * u;
                self.eos
                    .state_rho_e(
                        MassDensity::new::<kilogram_per_cubic_meter>(q[0]),
                        AvailableEnergy::new::<joule_per_kilogram>(e),
                        Some(Pressure::new::<pascal>(hints[i])),
                    )
                    .map_err(|err| in_cell(err, i))
            })
            .collect()
    }

    /// `p(ρ, e)` and the forward-difference `(∂p/∂ρ)_e`, `(∂p/∂e)_ρ`.
    fn eos_with_derivatives(
        &self,
        rho: f64,
        e: f64,
        hint: f64,
        cell: usize,
    ) -> Result<(f64, f64, f64), TampinesError> {
        let p_of = |r: f64, en: f64| -> Result<f64, TampinesError> {
            self.eos
                .state_rho_e(
                    MassDensity::new::<kilogram_per_cubic_meter>(r),
                    AvailableEnergy::new::<joule_per_kilogram>(en),
                    Some(Pressure::new::<pascal>(hint)),
                )
                .map(|t| t.pressure.get::<pascal>())
                .map_err(|err| in_cell(err, cell))
        };
        let p = p_of(rho, e)?;
        let hr = FD_REL_STEP * rho;
        let he = FD_REL_STEP * (e.abs() + p / rho);
        let p_rho = (p_of(rho + hr, e)? - p) / hr;
        let p_e = (p_of(rho, e + he)? - p) / he;
        Ok((p, p_rho, p_e))
    }

    /// The boundary face of one end for this step, from the current state.
    fn boundary_face(&self, end: PipeEnd) -> Result<BFace, TampinesError> {
        let n = self.q.len();
        let (bc, cell, inward) = match end {
            PipeEnd::Inlet => (self.inlet, 0, 1.0),
            PipeEnd::Outlet => (self.outlet, n - 1, -1.0),
        };
        match bc {
            PciceBoundary::Wall => Ok(BFace::Wall),
            PciceBoundary::FixedPressure(p) => Ok(BFace::Pressure {
                p_b: p.get::<pascal>(),
                ghost: None,
            }),
            PciceBoundary::Reservoir {
                stagnation_pressure,
                stagnation_enthalpy,
            } => {
                let p0 = stagnation_pressure.get::<pascal>();
                let h0 = stagnation_enthalpy.get::<joule_per_kilogram>();
                let u = self.q[cell][1] / self.q[cell][0];
                if u * inward < 0.0 {
                    // Outflow: the reservoir pressure, end-cell state.
                    return Ok(BFace::Pressure { p_b: p0, ghost: None });
                }
                let h = h0 - 0.5 * u * u;
                let mut rho_b = self.q[cell][0];
                let mut state = self.thermo[cell];
                for _ in 0..4 {
                    let p_b = p0 - 0.5 * rho_b * u * u;
                    state = self
                        .eos
                        .state_p_h(
                            Pressure::new::<pascal>(p_b),
                            AvailableEnergy::new::<joule_per_kilogram>(h),
                        )
                        .map_err(|err| in_cell(err, cell))?;
                    rho_b = state.density.get::<kilogram_per_cubic_meter>();
                }
                let e_b = state.specific_internal_energy.get::<joule_per_kilogram>();
                Ok(BFace::Pressure {
                    p_b: p0 - 0.5 * rho_b * u * u,
                    ghost: Some([rho_b, rho_b * u, rho_b * (e_b + 0.5 * u * u)]),
                })
            }
        }
    }

    /// Advective face fluxes of a conserved state `q`.
    fn advective_fluxes(&self, q: &[[f64; 3]], bfaces: &[BFace; 2]) -> Vec<FaceFlux> {
        let n = q.len();
        let ghost = |bf: &BFace, end_cell: [f64; 3]| -> [f64; 3] {
            match *bf {
                BFace::Wall => [end_cell[0], -end_cell[1], end_cell[2]],
                BFace::Pressure { ghost: Some(g), .. } => g,
                BFace::Pressure { ghost: None, .. } => end_cell,
            }
        };
        let g_in = ghost(&bfaces[0], q[0]);
        let g_out = ghost(&bfaces[1], q[n - 1]);
        let ext = |j: usize| -> [f64; 3] {
            // j indexes [ghost_in, cell 0, …, cell n−1, ghost_out]
            if j == 0 {
                g_in
            } else if j == n + 1 {
                g_out
            } else {
                q[j - 1]
            }
        };
        let lim = self.numerics.limiter;
        let mut faces = vec![FaceFlux::default(); n + 1];
        for (f, face) in faces.iter_mut().enumerate().take(n).skip(1) {
            // Owner cell f−1 = ext(f), neighbour cell f = ext(f+1).
            let (pm, po, pn, pnn) = (ext(f - 1), ext(f), ext(f + 1), ext(f + 2));
            let mut ql = [0.0; 3];
            let mut qr = [0.0; 3];
            for k in 0..3 {
                let (l, r) = muscl_face(pm[k], po[k], pn[k], pnn[k], lim);
                ql[k] = l;
                qr[k] = r;
            }
            *face = face_flux(ql, qr);
        }
        let wall = |c: [f64; 3]| FaceFlux {
            g: 0.0,
            fm: 0.0,
            fe: 0.0,
            rho: c[0],
            mom: 0.0,
            rhoe: c[2],
        };
        faces[0] = match bfaces[0] {
            BFace::Wall => wall(q[0]),
            BFace::Pressure { .. } => face_flux(g_in, q[0]),
        };
        faces[n] = match bfaces[1] {
            BFace::Wall => wall(q[n - 1]),
            BFace::Pressure { .. } => face_flux(q[n - 1], g_out),
        };
        faces
    }

    /// Step 1: the two-stage predictor; returns the stage-averaged face fluxes.
    fn predict(&self, dt: f64, bfaces: &[BFace; 2]) -> Result<Vec<FaceFlux>, TampinesError> {
        let n = self.q.len();
        let f0 = self.advective_fluxes(&self.q, bfaces);
        let mut q1 = self.q.clone();
        for i in 0..n {
            let s = dt / self.vol[i];
            let (al, ar) = (self.area_f[i], self.area_f[i + 1]);
            let (l, r) = (f0[i], f0[i + 1]);
            q1[i][0] -= s * (ar * r.g - al * l.g);
            q1[i][1] -= s * (ar * r.fm - al * l.fm);
            q1[i][2] -= s * (ar * r.fe - al * l.fe);
            if !(q1[i][0] > 0.0 && q1[i][0].is_finite()) {
                return Err(unphysical(i, "predictor density", q1[i][0]));
            }
        }
        let f1 = self.advective_fluxes(&q1, bfaces);
        Ok(f0
            .iter()
            .zip(&f1)
            .map(|(a, b)| FaceFlux {
                g: 0.5 * (a.g + b.g),
                fm: 0.5 * (a.fm + b.fm),
                fe: 0.5 * (a.fe + b.fe),
                rho: 0.5 * (a.rho + b.rho),
                mom: 0.5 * (a.mom + b.mom),
                rhoe: 0.5 * (a.rhoe + b.rhoe),
            })
            .collect())
    }

    /// Steps 3 and 6: the conservative update for the pressure iterate `p_k`,
    /// with the face linearisation of step 4.
    #[allow(clippy::too_many_arguments)]
    fn evaluate(
        &self,
        dt: f64,
        theta: f64,
        pred: &[FaceFlux],
        bfaces: &[BFace; 2],
        p_n: &[f64],
        p_k: &[f64],
    ) -> Result<Evaluated, TampinesError> {
        let n = self.q.len();
        let p_th: Vec<f64> = p_n
            .iter()
            .zip(p_k)
            .map(|(a, b)| (1.0 - theta) * a + theta * b)
            .collect();
        let mut g = vec![0.0; n + 1];
        let mut fm = vec![0.0; n + 1];
        let mut fe = vec![0.0; n + 1];
        let mut dg = vec![(0.0, 0.0); n + 1];
        let mut dpf = vec![(0.0, 0.0); n + 1];
        let mut u_up = vec![0.0; n + 1];
        let mut h_f = vec![0.0; n + 1];
        let mut u_f = vec![0.0; n + 1];
        for f in 0..=n {
            let pf = pred[f];
            // Face pressure p^θ_f, the compact gradient and their δp weights.
            let face = if f == 0 {
                match bfaces[0] {
                    BFace::Wall => None,
                    BFace::Pressure { p_b, .. } => {
                        let d = 0.5 * self.dx;
                        Some((p_b, (p_th[0] - p_b) / d, (0.0, 0.0), (0.0, theta / d)))
                    }
                }
            } else if f == n {
                match bfaces[1] {
                    BFace::Wall => None,
                    BFace::Pressure { p_b, .. } => {
                        let d = 0.5 * self.dx;
                        Some((p_b, (p_b - p_th[n - 1]) / d, (0.0, 0.0), (-theta / d, 0.0)))
                    }
                }
            } else {
                let d = self.dx;
                Some((
                    0.5 * (p_th[f - 1] + p_th[f]),
                    (p_th[f] - p_th[f - 1]) / d,
                    (0.5 * theta, 0.5 * theta),
                    (-theta / d, theta / d),
                ))
            };
            match face {
                None => {
                    // Wall: only the pressure force, from the adjacent cell.
                    let (c, w) = if f == 0 { (0, (0.0, theta)) } else { (n - 1, (theta, 0.0)) };
                    fm[f] = p_th[c];
                    dpf[f] = w;
                }
                Some((p_face, grad, w_pf, w_grad)) => {
                    let gf = pf.g - 0.5 * dt * grad;
                    let up = pf.mom / pf.rho;
                    let e_f = pf.rhoe / pf.rho;
                    g[f] = gf;
                    fm[f] = pf.fm + (gf - pf.g) * up + p_face;
                    fe[f] = pf.fe + (gf - pf.g) * e_f + p_face * gf / pf.rho;
                    dg[f] = (-0.5 * dt * w_grad.0, -0.5 * dt * w_grad.1);
                    dpf[f] = w_pf;
                    u_up[f] = up;
                    h_f[f] = e_f + p_face / pf.rho;
                    u_f[f] = gf / pf.rho;
                }
            }
        }
        let mut q = self.q.clone();
        for i in 0..n {
            let s = dt / self.vol[i];
            let (al, ar) = (self.area_f[i], self.area_f[i + 1]);
            q[i][0] -= s * (ar * g[i + 1] - al * g[i]);
            q[i][1] -= s * (ar * fm[i + 1] - al * fm[i]) - s * p_th[i] * (ar - al);
            q[i][2] -= s * (ar * fe[i + 1] - al * fe[i]);
        }
        Ok(Evaluated {
            q,
            g,
            fe,
            dg,
            dpf,
            u_up,
            h_f,
            u_f,
        })
    }
}

fn validate_numerics(nm: &PciceNumerics) -> Result<(), TampinesError> {
    let c = nm.material_courant.get::<ratio>();
    let dt = nm.max_dt.get::<second>();
    let th = nm.implicitness.get::<ratio>();
    let cap_ok = nm
        .acoustic_courant_cap
        .map_or(true, |a| a.get::<ratio>().is_finite() && a.get::<ratio>() > 0.0);
    if !(c.is_finite() && c > 0.0 && c <= 1.0) {
        return Err(TampinesError::InvalidInput(format!(
            "material Courant {c} must be in (0, 1]"
        )));
    }
    if !(dt.is_finite() && dt > 0.0) || !cap_ok || nm.correction_passes == 0 {
        return Err(TampinesError::InvalidInput(format!(
            "PCICE numerics invalid: max_dt {dt} s must be > 0, the acoustic cap > 0, \
             and at least one correction pass ({})",
            nm.correction_passes
        )));
    }
    if !(0.5..=1.0).contains(&th) {
        return Err(TampinesError::InvalidInput(format!(
            "implicitness {th} must be in [0.5, 1]"
        )));
    }
    Ok(())
}
