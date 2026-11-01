//! A network of [`HemPipe`]s joined by 0-D junctions (gh:#1071, RELAP-7
//! parity epic gh:#1056). AI draft, clean room: written from the RELAP-7
//! Theory Manual's description of its junction components (INL/EXT-14-31366,
//! §6.5; cited, not quoted), never from RELAP-7 source.
//!
//! # The junctions
//!
//! - [`HemJunction::Flow`]: two pipe ends meeting with no volume, and a form
//!   loss `K`. The RELAP-7 counterpart imposes mass, momentum (with `K`) and
//!   total-enthalpy constraints by Lagrange multipliers (§6.5.1). Here, in
//!   finite volume, the junction is a face: the KNP flux between the two end
//!   cells, through the smaller of the two end areas. Only two ends: an n-way
//!   meeting is a [`HemJunction::Volume`].
//! - [`HemJunction::Volume`]: a 0-D volume storing mass and total energy, with
//!   any number of pipe ends attached, each with its own `K` (RELAP-7's
//!   VolumeBranch, §6.5.2, Eqs 346–348). Its kinetic energy is neglected: its
//!   state is `(ρ = M/V, e = U/M)`, closed by the network's equation of state.
//!
//! # How a step works
//!
//! 1. One common `dt`: the Courant target over every pipe, and over every
//!    volume `Σ (|u| + c)·A/V` of its ends.
//! 2. For each junction face, the KNP flux between the end cell (velocity
//!    along the end's outward normal) and the state across the junction.
//!    The form loss `Δp = K·½ρu²` (upstream cell's density and normal
//!    velocity) enters as RELAP5 puts it, in the junction momentum balance:
//!    the flux is computed with the **downstream** state's pressure raised by
//!    `Δp`, the upstream pipe receives that flux, and the downstream pipe the
//!    same flux with its momentum lowered by `Δp`. At steady state the two
//!    end pressures then differ by exactly `Δp`. (Adding `Δp` to one side
//!    after the flux, the first version, carried only about 40 % of it,
//!    because the KNP flux averages the two pressures.)
//! 3. Each end gets that flux as a [`HemBoundary::Flux`], held for the step.
//!    Every pipe is advanced by `dt` ([`HemPipe::step_by`]).
//! 4. Each volume integrates the mass and energy rates its ends carried,
//!    `dM = Σ ṁ dt`, `dU = Σ Ė dt`.
//!
//! **Mass and total energy are conserved by construction**: the two sides of
//! every junction face integrate the same rate over the same `dt` (to
//! round-off). Momentum is not, at a loss or an area change, which is
//! physical (the wall and loss forces).
//!
//! **Accuracy.** The junction face is first order and frozen over the
//! Runge–Kutta stages of the step, so a junction is not invisible: see the
//! transparency test, which measures the difference against one continuous
//! pipe.

use petir::mathf::RealMath;
use outram_foam_basic_lib::fv_operators::fvc::central_upwind::{
    central_flux_speed_max, knp_face_flux, KnpFaceFlux, KnpFaceState,
};
use outram_foam_basic_lib::primitives::vector::Vector3;
use uom::si::area::square_meter;
use uom::si::length::meter;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::energy::joule;
use uom::si::f64::{
    Area, AvailableEnergy, Energy, Length, HeatFluxDensity, Mass, MassDensity, MassFlux, Pressure,
    ThermodynamicTemperature, Time, Volume,
};
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::heat_flux_density::watt_per_square_meter;
use uom::si::mass::kilogram;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::mass_flux::kilogram_per_square_meter_second;
use uom::si::pressure::pascal;
use uom::si::time::second;
use uom::si::volume::cubic_meter;

use super::eos::{HemEquationOfState, HemThermoState};
use super::{HemBoundary, HemCellState, HemError, HemPipe, PipeEnd};

/// One end of one pipe in a [`HemNetwork`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipeEndRef {
    /// Index returned by [`HemNetwork::add_pipe`].
    pub pipe: usize,
    /// Which end.
    pub end: PipeEnd,
}

/// A 0-D volume in a [`HemNetwork`] (RELAP-7 VolumeBranch).
#[derive(Debug, Clone, PartialEq)]
pub struct VolumeBranch {
    volume_m3: f64,
    mass_kg: f64,
    energy_j: f64,
    /// Attached ends and their form-loss coefficients.
    connections: Vec<(PipeEndRef, f64)>,
    /// How each end draws from the volume (all `Mixture` for a plain
    /// volume branch).
    ports: Vec<VolumePort>,
    /// Cross-section for the liquid level, m² (`None` for a plain volume).
    cross_section_m2: Option<f64>,
}

/// How a port of a volume draws from it (gh:#1073; RELAP-7 §6.8–6.9, cited,
/// not quoted). With a two-phase volume and a closure that supplies
/// [`HemEquationOfState::saturated_phases`], the phases are taken as
/// separated: liquid below the level, vapour above. With a single-phase
/// volume, or a closure without a phase split, every port draws the mixture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VolumePort {
    /// The homogeneous mixture (a plain volume branch).
    Mixture,
    /// Saturated vapour: an ideal separator's steam outlet (Eqs 370–372).
    Vapour,
    /// Saturated liquid: an ideal separator's liquid outlet.
    Liquid,
    /// A port at this height above the volume's bottom, m: below the level it
    /// draws liquid at `p + ρ_f g (L − z)`, above it vapour at `p` (the
    /// downcomer's uncovered-port switch, Eq 377).
    AtHeight(f64),
}

/// What a port drew on the last evaluation (see [`HemNetwork::port_draw`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortDraw {
    /// The mixture.
    Mixture,
    /// Saturated liquid.
    Liquid,
    /// Saturated vapour.
    Vapour,
}

impl VolumeBranch {
    /// Volume, m³.
    pub fn volume(&self) -> Volume {
        Volume::new::<cubic_meter>(self.volume_m3)
    }

    /// Mass held.
    pub fn mass(&self) -> Mass {
        Mass::new::<kilogram>(self.mass_kg)
    }

    /// Total (internal) energy held; kinetic energy is neglected.
    pub fn energy(&self) -> Energy {
        Energy::new::<joule>(self.energy_j)
    }
}

/// A junction in a [`HemNetwork`].
#[derive(Debug, Clone, PartialEq)]
pub enum HemJunction {
    /// Two pipe ends, no volume, form loss `K` (dimensionless, `≥ 0`).
    Flow {
        /// First end.
        a: PipeEndRef,
        /// Second end.
        b: PipeEndRef,
        /// Form-loss coefficient `K`.
        form_loss: f64,
    },
    /// A 0-D volume with any number of ends.
    Volume(VolumeBranch),
    /// A pump from suction end `a` to discharge end `b` (RELAP-7 §6.6, gh:#1072).
    Pump {
        /// Suction end.
        a: PipeEndRef,
        /// Discharge end.
        b: PipeEndRef,
        /// How the head is set.
        head: PumpHead,
        /// Reverse-flow form losses `(K1, K2)` (Eqs 357-358).
        reverse_loss: (f64, f64),
    },
    /// A valve between two ends: area `opening(t)·min(A)`, loss
    /// `K/opening²`, closed (both ends walls) at `opening ≤ 0` (§6.10).
    Valve {
        /// First end.
        a: PipeEndRef,
        /// Second end.
        b: PipeEndRef,
        /// Opening fraction against time, clamped to `[0, 1]`.
        opening: TimeTable,
        /// Form loss when fully open.
        form_loss: f64,
    },
    /// A relief/safety valve from `a` (protected side) to `b` (discharge
    /// side) (RELAP-7 §6.11, gh:#1074). Opens when `a`'s end-cell pressure
    /// reaches `set_pressure_pa`, recloses when it falls to
    /// `reseat_pressure_pa`; while open it passes the equation of state's
    /// break flow ([`HemEquationOfState::break_flow`]: choked or
    /// subcritical) from `a`'s stagnation state to `b`'s pressure, through
    /// `area_fraction·min(A)`. Only closures with a break-flow model (IAPWS-IF97)
    /// can open it; the manual's ideal-gas choking relations are not used.
    ReliefValve {
        /// Protected end.
        a: PipeEndRef,
        /// Discharge end.
        b: PipeEndRef,
        /// Opening pressure, Pa.
        set_pressure_pa: f64,
        /// Reclosing pressure, Pa, `≤ set_pressure_pa`.
        reseat_pressure_pa: f64,
        /// Flow area over the smaller end area, in `(0, 1]`.
        area_fraction: f64,
    },
    /// A turbine from inlet end `a` to outlet end `b` (§6.7).
    Turbine {
        /// Inlet end.
        a: PipeEndRef,
        /// Outlet end.
        b: PipeEndRef,
        /// The characteristic and efficiency.
        params: TurbineParams,
    },
}

impl HemJunction {
    /// The two ends of a two-end junction.
    fn ends(&self) -> Option<(PipeEndRef, PipeEndRef)> {
        match self {
            HemJunction::Flow { a, b, .. }
            | HemJunction::Pump { a, b, .. }
            | HemJunction::Valve { a, b, .. }
            | HemJunction::ReliefValve { a, b, .. }
            | HemJunction::Turbine { a, b, .. } => Some((*a, *b)),
            HemJunction::Volume(_) => None,
        }
    }
}

/// How a pump's head is set (RELAP-7 §6.6).
#[derive(Debug, Clone, PartialEq)]
pub enum PumpHead {
    /// Head `H(t)`, m (a coastdown is a falling table).
    Head(TimeTable),
    /// Shaft power `W(t)`, W, giving `H = W/(ṁ g)` (Eq 355) with `ṁ` from
    /// the previous step, capped at the pump's shut-off head. Eq 355 is
    /// singular at `ṁ = 0`; without the cap a pump started from rest never
    /// develops head (found 2026-11-01: the first version gave zero head
    /// from rest, a flat 0 Pa rise).
    ShaftPower {
        /// Shaft power, W.
        power: TimeTable,
        /// Shut-off head (the head at zero flow), m, `> 0`.
        shutoff_head_m: f64,
    },
}

/// A turbine's characteristic (RELAP-7 §6.7, Eqs 359-364).
///
/// Mass flow `ṁ = ṁ_max (p01/p01r) √(T01r/T01) tanh(β (p01/p02 − 1))`, with
/// `β` from the design point, `tanh(β (r_d − 1)) = ṁ_r/ṁ_max` (Eq 362).
/// Shaft power `ṁ (h01 − h02)` with `h01 − h02 = η (h01 − h02s)`. The manual's
/// isentropic relations are ideal-gas forms; here the isentropic enthalpy
/// drop is `γ/(γ−1) (p01/ρ01) (1 − (p02/p01)^((γ−1)/γ))` with a user `γ`, and
/// the stagnation pressure `p0 = p + ½ρu²`. Closed when `p01 ≤ p02`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TurbineParams {
    /// `ṁ_max`, kg/s.
    pub max_mass_flow_kg_s: f64,
    /// Design inlet stagnation pressure `p01r`, Pa.
    pub design_stagnation_pressure_pa: f64,
    /// Design inlet stagnation temperature `T01r`, K.
    pub design_stagnation_temperature_k: f64,
    /// Design pressure ratio `p01r/p02r`, `> 1`.
    pub design_pressure_ratio: f64,
    /// Design mass flow over `ṁ_max`, in `(0, 1)`.
    pub design_mass_flow_ratio: f64,
    /// Thermal efficiency `η`, in `(0, 1]`.
    pub efficiency: f64,
    /// Isentropic exponent `γ`, `> 1`.
    pub isentropic_exponent: f64,
}

impl TurbineParams {
    /// `β` of Eq 362.
    pub fn beta(&self) -> f64 {
        let x = self.design_mass_flow_ratio;
        0.5 * ((1.0 + x) / (1.0 - x)).r_ln() / (self.design_pressure_ratio - 1.0)
    }
}

/// A piecewise-linear table of one quantity against time (SI), clamped at
/// both ends, as RELAP-7's time-dependent components take their inputs
/// (§6.13–6.14) and GeN-Foam's `TimeProfile::table` interpolates.
#[derive(Debug, Clone, PartialEq)]
pub struct TimeTable {
    times_s: Vec<f64>,
    values: Vec<f64>,
}

impl TimeTable {
    /// Build from `(time, value)` points with strictly increasing, finite times
    /// and finite values.
    ///
    /// # Errors
    ///
    /// [`HemError::InvalidInput`] for no points, non-increasing or non-finite
    /// times, or a non-finite value.
    pub fn new(points: &[(f64, f64)]) -> Result<Self, HemError> {
        let ok = !points.is_empty()
            && points.iter().all(|(t, v)| t.is_finite() && v.is_finite())
            && points.windows(2).all(|w| w[1].0 > w[0].0);
        if !ok {
            return Err(HemError::InvalidInput(format!(
                "a time table needs finite values at strictly increasing times; got {points:?}"
            )));
        }
        Ok(Self {
            times_s: points.iter().map(|p| p.0).collect(),
            values: points.iter().map(|p| p.1).collect(),
        })
    }

    /// A constant.
    pub fn constant(value: f64) -> Self {
        Self {
            times_s: vec![0.0],
            values: vec![value],
        }
    }

    /// The value at time `t_s`: linear between points, held beyond the ends.
    pub fn at(&self, t_s: f64) -> f64 {
        let n = self.times_s.len();
        if t_s <= self.times_s[0] {
            return self.values[0];
        }
        if t_s >= self.times_s[n - 1] {
            return self.values[n - 1];
        }
        let k = self.times_s.partition_point(|&x| x <= t_s);
        let (t0, t1) = (self.times_s[k - 1], self.times_s[k]);
        let (v0, v1) = (self.values[k - 1], self.values[k]);
        v0 + (v1 - v0) * (t_s - t0) / (t1 - t0)
    }
}

/// How a time-dependent volume fixes its thermodynamic state.
#[derive(Debug, Clone, PartialEq)]
pub enum TdvThermo {
    /// Temperature, K (single phase).
    Temperature(TimeTable),
    /// Specific enthalpy, J/kg (valid in the two-phase dome, where it fixes
    /// the void fraction RELAP-7's TDV takes directly).
    Enthalpy(TimeTable),
}

/// A time-dependent boundary on one pipe end (RELAP-7 §6.13–6.14).
#[derive(Debug, Clone, PartialEq)]
pub enum TimeDependentBoundary {
    /// Time-dependent volume: a stagnation reservoir at `pressure(t)` (Pa)
    /// and the given temperature or enthalpy, applied each step as a
    /// [`HemBoundary::Reservoir`].
    Volume {
        /// Stagnation pressure, Pa.
        pressure: TimeTable,
        /// Temperature or enthalpy.
        thermo: TdvThermo,
    },
    /// Time-dependent junction: a mass flow `mass_flow(t)` (kg/s, positive
    /// INTO the pipe) entering at `temperature(t)` (K) and the end cell's
    /// pressure; for outflow the end cell's state is carried out.
    Junction {
        /// Mass flow into the pipe, kg/s.
        mass_flow: TimeTable,
        /// Inflow temperature, K.
        temperature: TimeTable,
    },
}

/// Pipes and junctions advanced together with one time step.
#[derive(Debug, Clone)]
pub struct HemNetwork<E: HemEquationOfState> {
    eos: E,
    pipes: Vec<HemPipe<E>>,
    junctions: Vec<HemJunction>,
    time_dependent: Vec<(PipeEndRef, TimeDependentBoundary)>,
    /// Mass flow through each two-end junction at the last step, kg/s (a →
    /// b positive).
    junction_mass_flow: Vec<f64>,
    /// Power each pump put into, or each turbine took out of, the fluid at
    /// the last step, W.
    junction_power: Vec<f64>,
    /// Whether each relief valve is open (and whether it was choked).
    relief_state: Vec<(bool, bool)>,
    courant: f64,
    time_s: f64,
}

/// What a two-end junction gives each side for one step.
struct TwoEnd {
    bc_a: HemBoundary,
    bc_b: HemBoundary,
    rate: f64,
    mdot: f64,
    pump_power: f64,
}

/// Standard gravity, m/s².
const GRAVITY: f64 = 9.80665;

/// A face state for the KNP flux along an end's outward normal.
fn knp_state(rho: f64, u_n: f64, p: f64, e: f64, c: f64) -> KnpFaceState {
    KnpFaceState::total_energy(rho, Vector3::new(u_n, 0.0, 0.0), p, c, e)
}

/// The same face state with its pressure raised by `dp`.
fn raised(rho: f64, u_n: f64, p: f64, e: f64, c: f64, dp: f64) -> KnpFaceState {
    knp_state(rho, u_n, p + dp, e, c)
}

/// Outward normal of an end along the pipe's `+x`.
fn outward(end: PipeEnd) -> f64 {
    match end {
        PipeEnd::Inlet => -1.0,
        PipeEnd::Outlet => 1.0,
    }
}

/// A face flux along an end's outward normal, as the end's `+x` flux.
fn to_pipe_frame(end: PipeEnd, mass_n: f64, mom_n: f64, energy_n: f64) -> HemBoundary {
    let n = outward(end);
    HemBoundary::Flux {
        mass_flux: MassFlux::new::<kilogram_per_square_meter_second>(n * mass_n),
        momentum_flux: Pressure::new::<pascal>(mom_n),
        energy_flux: HeatFluxDensity::new::<watt_per_square_meter>(n * energy_n),
    }
}

impl<E: HemEquationOfState> HemNetwork<E> {
    /// An empty network closed by `eos` (used for every volume), with a target
    /// central Courant number `courant` (upstream's measure; `0.2` is
    /// upstream's shock-tube value).
    ///
    /// # Errors
    ///
    /// [`HemError::InvalidInput`] unless `0 < courant ≤ 0.5`.
    pub fn new(eos: E, courant: f64) -> Result<Self, HemError> {
        if !(courant.is_finite() && courant > 0.0 && courant <= super::FIXED_STEP_COURANT_LIMIT) {
            return Err(HemError::InvalidInput(format!(
                "network Courant number {courant} must be in (0, 0.5]"
            )));
        }
        Ok(Self {
            eos,
            pipes: Vec::new(),
            junctions: Vec::new(),
            time_dependent: Vec::new(),
            junction_mass_flow: Vec::new(),
            junction_power: Vec::new(),
            relief_state: Vec::new(),
            courant,
            time_s: 0.0,
        })
    }

    /// Add a pipe; returns its index. Its ends keep their own boundaries
    /// unless a junction claims them.
    pub fn add_pipe(&mut self, pipe: HemPipe<E>) -> usize {
        self.pipes.push(pipe);
        self.pipes.len() - 1
    }

    fn check_end(&self, end: PipeEndRef) -> Result<(), HemError> {
        if end.pipe >= self.pipes.len() {
            return Err(HemError::InvalidInput(format!("no pipe {}", end.pipe)));
        }
        let taken = self.junctions.iter().any(|j| match j {
            HemJunction::Volume(v) => v.connections.iter().any(|(e, _)| *e == end),
            other => other.ends().is_some_and(|(a, b)| a == end || b == end),
        });
        let driven = self.time_dependent.iter().any(|(e, _)| *e == end);
        if taken || driven {
            return Err(HemError::InvalidInput(format!(
                "{end:?} already belongs to a junction"
            )));
        }
        Ok(())
    }

    /// Drive one pipe end with a time-dependent volume or junction
    /// (gh:#1076); returns its index among the time-dependent boundaries.
    ///
    /// # Errors
    ///
    /// [`HemError::InvalidInput`] for an unknown or already joined end.
    pub fn add_time_dependent(
        &mut self,
        end: PipeEndRef,
        boundary: TimeDependentBoundary,
    ) -> Result<usize, HemError> {
        self.check_end(end)?;
        self.time_dependent.push((end, boundary));
        Ok(self.time_dependent.len() - 1)
    }

    /// The boundary a time-dependent component sets on its end at time `t`.
    fn time_dependent_boundary(
        &self,
        end: PipeEndRef,
        tdb: &TimeDependentBoundary,
        t: f64,
    ) -> Result<HemBoundary, HemError> {
        match tdb {
            TimeDependentBoundary::Volume { pressure, thermo } => {
                let p = Pressure::new::<pascal>(pressure.at(t));
                let h = match thermo {
                    TdvThermo::Enthalpy(h) => AvailableEnergy::new::<joule_per_kilogram>(h.at(t)),
                    TdvThermo::Temperature(temp) => {
                        self.eos
                            .state_p_t(p, ThermodynamicTemperature::new::<kelvin>(temp.at(t)))?
                            .specific_enthalpy
                    }
                };
                Ok(HemBoundary::Reservoir {
                    stagnation_pressure: p,
                    stagnation_enthalpy: h,
                })
            }
            TimeDependentBoundary::Junction {
                mass_flow,
                temperature,
            } => {
                let (c, area) = self.end_cell(end)?;
                let g_in = mass_flow.at(t) / area;
                let p_c = c.pressure.get::<pascal>();
                let (rho, h) = if g_in >= 0.0 {
                    let st = self.eos.state_p_t(
                        c.pressure,
                        ThermodynamicTemperature::new::<kelvin>(temperature.at(t)),
                    )?;
                    (
                        st.density.get::<kilogram_per_cubic_meter>(),
                        st.specific_enthalpy.get::<joule_per_kilogram>(),
                    )
                } else {
                    (
                        c.density.get::<kilogram_per_cubic_meter>(),
                        c.specific_enthalpy.get::<joule_per_kilogram>(),
                    )
                };
                // Outward-normal frame: inflow is a negative normal mass flux.
                let u = g_in / rho;
                Ok(to_pipe_frame(
                    end.end,
                    -g_in,
                    p_c + g_in * u,
                    -g_in * (h + 0.5 * u * u),
                ))
            }
        }
    }

    fn check_loss(k: f64) -> Result<(), HemError> {
        if k.is_finite() && k >= 0.0 {
            Ok(())
        } else {
            Err(HemError::InvalidInput(format!(
                "form loss K = {k} must be >= 0"
            )))
        }
    }

    /// Add a pump, valve or turbine (gh:#1072); returns the junction's index.
    ///
    /// # Errors
    ///
    /// [`HemError::InvalidInput`] for an unknown or already joined end, the
    /// same end twice, a negative loss, or turbine parameters out of range.
    pub fn add_component(&mut self, junction: HemJunction) -> Result<usize, HemError> {
        let Some((a, b)) = junction.ends() else {
            return Err(HemError::InvalidInput(
                "use add_volume_branch for a volume".into(),
            ));
        };
        self.check_end(a)?;
        self.check_end(b)?;
        if a == b {
            return Err(HemError::InvalidInput(
                "a junction needs two different ends".into(),
            ));
        }
        match &junction {
            HemJunction::Flow { form_loss, .. } | HemJunction::Valve { form_loss, .. } => {
                Self::check_loss(*form_loss)?
            }
            HemJunction::Pump {
                reverse_loss, head, ..
            } => {
                Self::check_loss(reverse_loss.0)?;
                Self::check_loss(reverse_loss.1)?;
                if let PumpHead::ShaftPower { shutoff_head_m, .. } = head {
                    if !(shutoff_head_m.is_finite() && *shutoff_head_m > 0.0) {
                        return Err(HemError::InvalidInput(format!(
                            "shut-off head {shutoff_head_m} m must be > 0"
                        )));
                    }
                }
            }
            HemJunction::Turbine { params: t, .. } => {
                let ok = t.max_mass_flow_kg_s > 0.0
                    && t.design_stagnation_pressure_pa > 0.0
                    && t.design_stagnation_temperature_k > 0.0
                    && t.design_pressure_ratio > 1.0
                    && t.design_mass_flow_ratio > 0.0
                    && t.design_mass_flow_ratio < 1.0
                    && t.efficiency > 0.0
                    && t.efficiency <= 1.0
                    && t.isentropic_exponent > 1.0;
                if !ok {
                    return Err(HemError::InvalidInput(format!(
                        "turbine parameters out of range: {t:?}"
                    )));
                }
            }
            HemJunction::ReliefValve {
                set_pressure_pa,
                reseat_pressure_pa,
                area_fraction,
                ..
            } => {
                let ok = *set_pressure_pa > 0.0
                    && *reseat_pressure_pa > 0.0
                    && reseat_pressure_pa <= set_pressure_pa
                    && *area_fraction > 0.0
                    && *area_fraction <= 1.0;
                if !ok {
                    return Err(HemError::InvalidInput(format!(
                        "relief valve needs 0 < reseat <= set and area fraction in (0, 1]; got set {set_pressure_pa}, reseat {reseat_pressure_pa}, fraction {area_fraction}"
                    )));
                }
            }
            HemJunction::Volume(_) => {}
        }
        self.junctions.push(junction);
        Ok(self.junctions.len() - 1)
    }

    /// Whether relief valve `index` is open, and whether its last flow was
    /// choked.
    pub fn relief_valve_state(&self, index: usize) -> (bool, bool) {
        self.relief_state
            .get(index)
            .copied()
            .unwrap_or((false, false))
    }

    /// Mass flow through two-end junction `index` at the last step, kg/s,
    /// `a → b` positive.
    pub fn junction_mass_flow(&self, index: usize) -> f64 {
        self.junction_mass_flow.get(index).copied().unwrap_or(0.0)
    }

    /// Power a pump put into (positive) or a turbine took out of (positive)
    /// the fluid at the last step, W.
    pub fn junction_power(&self, index: usize) -> f64 {
        self.junction_power.get(index).copied().unwrap_or(0.0)
    }

    /// The face fluxes of a two-end junction: KNP between the end cells
    /// through `scale·min(A)`, with a form loss `K` and a pump pressure rise
    /// `pump_dp` from `a` to `b`. Both enter the junction momentum balance
    /// the RELAP5 way: the loss raises the downstream state's pressure, the
    /// pump raises `a`'s, and each side receives the flux less its jump.
    fn two_end(
        &self,
        a: PipeEndRef,
        b: PipeEndRef,
        scale: f64,
        form_loss: f64,
        pump_dp: f64,
    ) -> Result<TwoEnd, HemError> {
        let nx = Vector3::new(1.0, 0.0, 0.0);
        let (ca, area_a) = self.end_cell(a)?;
        let (cb, area_b) = self.end_cell(b)?;
        // In a's outward frame, b's velocity is along −(b's outward normal).
        let pa = Self::prim(&ca, outward(a.end));
        let pb = Self::prim(&cb, -outward(b.end));
        // Upstream by the mean normal velocity; the loss from its cell.
        let a_is_up = pa.1 + pb.1 >= 0.0;
        let up = if a_is_up { pa } else { pb };
        let loss = form_loss * 0.5 * up.0 * up.1 * up.1;
        let (dpa, dpb) = if a_is_up { (0.0, loss) } else { (loss, 0.0) };
        let dpa = if pump_dp != 0.0 { dpa + pump_dp } else { dpa };
        let sa = raised(pa.0, pa.1, pa.2, pa.3, pa.4, dpa);
        let sb = raised(pb.0, pb.1, pb.2, pb.3, pb.4, dpb);
        let f: KnpFaceFlux = knp_face_flux(&sa, &sb, nx);
        let amax = central_flux_speed_max(&sa, &sb, nx);
        let aj = area_a.min(area_b) * scale;
        let mdot = f.mass * aj;
        let edot = f.energy * aj;
        let rho_up = up.0;
        let (mom_a, mom_b) = if area_a == area_b && scale == 1.0 {
            (f.momentum.x, f.momentum.x)
        } else {
            let u_j = f.mass / rho_up;
            let p_j = f.momentum.x - f.mass * u_j;
            let side = |area: f64| p_j + (mdot / area) * (mdot / area) / rho_up;
            (side(area_a), side(area_b))
        };
        let (mom_a, mom_b) = if a_is_up {
            (mom_a, mom_b - loss)
        } else {
            (mom_a - loss, mom_b)
        };
        // The pump work enters b only: a loses its jump in momentum and the
        // pressure work ṁ·Δp/ρ in energy.
        let (mom_a, edot_a, pump_power) = if pump_dp != 0.0 {
            let w = mdot * pump_dp / pa.0;
            (mom_a - pump_dp, edot - w, w)
        } else {
            (mom_a, edot, 0.0)
        };
        let cell = self.pipes[a.pipe]
            .geometry()
            .cell_width()
            .get::<uom::si::length::meter>();
        Ok(TwoEnd {
            // a: flux out of a; b: the same rate into b, so out of b is negative.
            bc_a: to_pipe_frame(a.end, mdot / area_a, mom_a, edot_a / area_a),
            bc_b: to_pipe_frame(b.end, -mdot / area_b, mom_b, -edot / area_b),
            rate: amax / cell,
            mdot,
            pump_power,
        })
    }

    /// The face fluxes of a turbine (RELAP-7 §6.7): mass flow from the
    /// characteristic, energy out of `a` at `h01`, into `b` at `h02`; each side
    /// keeps its own pressure in momentum (the turbine sets `p1` for the inlet
    /// and `p2` for the outlet). `pump_power` carries the shaft power taken.
    fn turbine(&self, a: PipeEndRef, b: PipeEndRef, t: &TurbineParams) -> Result<TwoEnd, HemError> {
        let (ca, area_a) = self.end_cell(a)?;
        let (cb, area_b) = self.end_cell(b)?;
        let v = |c: &HemCellState| c.velocity.get::<uom::si::velocity::meter_per_second>();
        let (rho1, p1, h1, u1) = (
            ca.density.get::<kilogram_per_cubic_meter>(),
            ca.pressure.get::<pascal>(),
            ca.specific_enthalpy.get::<joule_per_kilogram>(),
            v(&ca),
        );
        let (rho2, p2, u2) = (
            cb.density.get::<kilogram_per_cubic_meter>(),
            cb.pressure.get::<pascal>(),
            v(&cb),
        );
        let (p01, h01) = (p1 + 0.5 * rho1 * u1 * u1, h1 + 0.5 * u1 * u1);
        let p02 = p2 + 0.5 * rho2 * u2 * u2;
        let cell = self.pipes[a.pipe]
            .geometry()
            .cell_width()
            .get::<uom::si::length::meter>();
        let c_max = ca
            .equilibrium_sound_speed
            .get::<uom::si::velocity::meter_per_second>()
            .max(
                cb.equilibrium_sound_speed
                    .get::<uom::si::velocity::meter_per_second>(),
            );
        if p01 <= p02 {
            // Closed valve: both ends walls.
            return Ok(TwoEnd {
                bc_a: to_pipe_frame(a.end, 0.0, p1, 0.0),
                bc_b: to_pipe_frame(b.end, 0.0, p2, 0.0),
                rate: c_max / cell,
                mdot: 0.0,
                pump_power: 0.0,
            });
        }
        let st01 = self.eos.state_p_h(
            Pressure::new::<pascal>(p01),
            AvailableEnergy::new::<joule_per_kilogram>(h01),
        )?;
        let t01 = st01.temperature.get::<kelvin>();
        let rho01 = st01.density.get::<kilogram_per_cubic_meter>();
        let mdot = t.max_mass_flow_kg_s
            * (p01 / t.design_stagnation_pressure_pa)
            * (t.design_stagnation_temperature_k / t01).sqrt()
            * (t.beta() * (p01 / p02 - 1.0)).r_tanh();
        let g = t.isentropic_exponent;
        let dh_s = g / (g - 1.0) * (p01 / rho01) * (1.0 - (p02 / p01).r_powf((g - 1.0) / g));
        let h02 = h01 - t.efficiency * dh_s;
        let w = mdot * (h01 - h02);
        let (ga, gb) = (mdot / area_a, mdot / area_b);
        let rate = (c_max + (ga / rho1).abs().max((gb / rho2).abs())) / cell;
        Ok(TwoEnd {
            bc_a: to_pipe_frame(a.end, ga, p1 + ga * ga / rho1, ga * h01),
            bc_b: to_pipe_frame(b.end, -gb, p2 + gb * gb / rho2, -gb * h02),
            rate,
            mdot,
            pump_power: w,
        })
    }

    /// Join two pipe ends with no volume and form loss `K`; returns the
    /// junction's index.
    ///
    /// # Errors
    ///
    /// [`HemError::InvalidInput`] for an unknown or already joined end, the
    /// same end twice, or `K < 0`.
    pub fn add_flow_junction(
        &mut self,
        a: PipeEndRef,
        b: PipeEndRef,
        form_loss: f64,
    ) -> Result<usize, HemError> {
        self.check_end(a)?;
        self.check_end(b)?;
        Self::check_loss(form_loss)?;
        if a == b {
            return Err(HemError::InvalidInput(
                "a flow junction needs two different ends".into(),
            ));
        }
        self.junctions.push(HemJunction::Flow { a, b, form_loss });
        Ok(self.junctions.len() - 1)
    }

    /// Add a 0-D volume `volume` holding `initial` (closed by the network's
    /// equation of state), with the given ends and form losses attached;
    /// returns its index.
    ///
    /// # Errors
    ///
    /// [`HemError::InvalidInput`] for a non-positive volume, no ends, an
    /// unknown or already joined end, an end listed twice, or `K < 0`.
    pub fn add_volume_branch(
        &mut self,
        volume: Volume,
        initial: HemThermoState,
        connections: Vec<(PipeEndRef, f64)>,
    ) -> Result<usize, HemError> {
        let v = volume.get::<cubic_meter>();
        if !(v.is_finite() && v > 0.0) || connections.is_empty() {
            return Err(HemError::InvalidInput(format!(
                "a volume branch needs V > 0 (got {v} m3) and at least one end"
            )));
        }
        for (i, (e, k)) in connections.iter().enumerate() {
            self.check_end(*e)?;
            Self::check_loss(*k)?;
            if connections[..i].iter().any(|(o, _)| o == e) {
                return Err(HemError::InvalidInput(format!("{e:?} listed twice")));
            }
        }
        let rho = initial.density.get::<kilogram_per_cubic_meter>();
        let mass_kg = rho * v;
        let energy_j = mass_kg * initial.specific_internal_energy.get::<joule_per_kilogram>();
        let ports = vec![VolumePort::Mixture; connections.len()];
        self.junctions.push(HemJunction::Volume(VolumeBranch {
            volume_m3: v,
            mass_kg,
            energy_j,
            connections,
            ports,
            cross_section_m2: None,
        }));
        Ok(self.junctions.len() - 1)
    }

    /// Add a volume whose phases separate (gh:#1073): a vertical vessel of
    /// `volume` and `cross_section` (height `V/A`), each end drawing as its
    /// [`VolumePort`] says. An ideal **separator-dryer** is an inlet with
    /// `Mixture`, a steam outlet with `Vapour` and a liquid outlet with
    /// `Liquid`; a **downcomer** has `AtHeight` ports. Mass and energy are
    /// stored as in a volume branch, and conserved by construction.
    ///
    /// # Errors
    ///
    /// [`HemError::InvalidInput`] as [`Self::add_volume_branch`], for a
    /// non-positive cross-section, or a port height outside `[0, V/A]`.
    pub fn add_separated_volume(
        &mut self,
        volume: Volume,
        cross_section: Area,
        initial: HemThermoState,
        connections: Vec<(PipeEndRef, f64, VolumePort)>,
    ) -> Result<usize, HemError> {
        let a = cross_section.get::<square_meter>();
        let height = volume.get::<cubic_meter>() / a;
        if !(a.is_finite() && a > 0.0) {
            return Err(HemError::InvalidInput(format!("cross-section {a} m2")));
        }
        for (_, _, port) in &connections {
            if let VolumePort::AtHeight(z) = port {
                if !(z.is_finite() && *z >= 0.0 && *z <= height) {
                    return Err(HemError::InvalidInput(format!("port height {z} m outside [0, {height}] m")));
                }
            }
        }
        let ports: Vec<VolumePort> = connections.iter().map(|c| c.2).collect();
        let ji = self.add_volume_branch(volume, initial, connections.into_iter().map(|c| (c.0, c.1)).collect())?;
        if let HemJunction::Volume(v) = &mut self.junctions[ji] {
            v.ports = ports;
            v.cross_section_m2 = Some(a);
        }
        Ok(ji)
    }

    /// The collapsed liquid level of separated volume `index`, `M(1 − x)/(ρ_f A)`
    /// clamped to the vessel height; the full height when single-phase liquid
    /// or without a phase split, zero when single-phase vapour. `None` for a
    /// plain volume branch or another junction.
    ///
    /// # Errors
    ///
    /// When the volume's state cannot be closed.
    pub fn volume_level(&self, index: usize) -> Result<Option<Length>, HemError> {
        match self.junctions.get(index) {
            Some(HemJunction::Volume(v)) if v.cross_section_m2.is_some() => {
                let st = self.close_volume(v)?;
                Ok(Some(Length::new::<meter>(self.level_of(v, &st))))
            }
            _ => Ok(None),
        }
    }

    /// What port `port` of volume `index` draws now. `None` if there is no such
    /// volume or port.
    ///
    /// # Errors
    ///
    /// When the volume's state cannot be closed.
    pub fn port_draw(&self, index: usize, port: usize) -> Result<Option<PortDraw>, HemError> {
        match self.junctions.get(index) {
            Some(HemJunction::Volume(v)) if port < v.ports.len() => {
                let st = self.close_volume(v)?;
                Ok(Some(self.port_state(v, &st, port).2))
            }
            _ => Ok(None),
        }
    }

    fn level_of(&self, v: &VolumeBranch, st: &HemThermoState) -> f64 {
        let a = v.cross_section_m2.unwrap_or(f64::INFINITY);
        let height = v.volume_m3 / a;
        match self.eos.saturated_phases(st) {
            Some((liq, _)) => {
                let x = st.quality.map_or(0.0, |q| q.get::<uom::si::ratio::ratio>()).clamp(0.0, 1.0);
                let l = v.mass_kg * (1.0 - x) / (liq.density.get::<kilogram_per_cubic_meter>() * a);
                l.min(height)
            }
            None => {
                if st.void_fraction.get::<uom::si::ratio::ratio>() >= 0.5 {
                    0.0
                } else {
                    height
                }
            }
        }
    }

    /// The state a port presents, its extra (hydrostatic) pressure, and what
    /// it draws.
    fn port_state(&self, v: &VolumeBranch, st: &HemThermoState, k: usize) -> (HemThermoState, f64, PortDraw) {
        let port = v.ports.get(k).copied().unwrap_or(VolumePort::Mixture);
        if port == VolumePort::Mixture {
            return (*st, 0.0, PortDraw::Mixture);
        }
        let Some((liq, vap)) = self.eos.saturated_phases(st) else {
            return (*st, 0.0, PortDraw::Mixture);
        };
        match port {
            VolumePort::Vapour => (vap, 0.0, PortDraw::Vapour),
            VolumePort::Liquid => (liq, 0.0, PortDraw::Liquid),
            VolumePort::AtHeight(z) => {
                let l = self.level_of(v, st);
                if z < l {
                    let dp = liq.density.get::<kilogram_per_cubic_meter>() * GRAVITY * (l - z);
                    (liq, dp, PortDraw::Liquid)
                } else {
                    (vap, 0.0, PortDraw::Vapour)
                }
            }
            VolumePort::Mixture => (*st, 0.0, PortDraw::Mixture),
        }
    }

    /// The pipes, by index.
    pub fn pipe(&self, index: usize) -> Option<&HemPipe<E>> {
        self.pipes.get(index)
    }

    /// A pipe, mutably, to change its source terms between steps (a reactor
    /// power source sets cell heat inputs this way, gh:#1079). Changing its
    /// boundaries at a junction-driven end is overwritten on the next step.
    pub fn pipe_mut(&mut self, index: usize) -> Option<&mut HemPipe<E>> {
        self.pipes.get_mut(index)
    }

    /// The junctions, by index.
    pub fn junction(&self, index: usize) -> Option<&HemJunction> {
        self.junctions.get(index)
    }

    /// Simulation time.
    pub fn time(&self) -> Time {
        Time::new::<second>(self.time_s)
    }

    /// The thermodynamic state of volume junction `index`.
    ///
    /// # Errors
    ///
    /// [`HemError::InvalidInput`] if `index` is not a volume; the equation of
    /// state's error if it cannot close the state.
    pub fn volume_state(&self, index: usize) -> Result<HemThermoState, HemError> {
        match self.junctions.get(index) {
            Some(HemJunction::Volume(v)) => self.close_volume(v),
            _ => Err(HemError::InvalidInput(format!(
                "junction {index} is not a volume"
            ))),
        }
    }

    fn close_volume(&self, v: &VolumeBranch) -> Result<HemThermoState, HemError> {
        self.eos.state_rho_e(
            MassDensity::new::<kilogram_per_cubic_meter>(v.mass_kg / v.volume_m3),
            AvailableEnergy::new::<joule_per_kilogram>(v.energy_j / v.mass_kg),
            None,
        )
    }

    /// Mass in every pipe and volume.
    pub fn total_mass(&self) -> Mass {
        let pipes: f64 = self
            .pipes
            .iter()
            .map(|p| p.total_mass().get::<kilogram>())
            .sum();
        let vols: f64 = self
            .junctions
            .iter()
            .map(|j| match j {
                HemJunction::Volume(v) => v.mass_kg,
                _ => 0.0,
            })
            .sum();
        Mass::new::<kilogram>(pipes + vols)
    }

    /// Total energy in every pipe and volume.
    pub fn total_energy(&self) -> Energy {
        let pipes: f64 = self
            .pipes
            .iter()
            .map(|p| p.total_energy().get::<joule>())
            .sum();
        let vols: f64 = self
            .junctions
            .iter()
            .map(|j| match j {
                HemJunction::Volume(v) => v.energy_j,
                _ => 0.0,
            })
            .sum();
        Energy::new::<joule>(pipes + vols)
    }

    /// The end cell of `end` and that end's face area, m².
    fn end_cell(&self, end: PipeEndRef) -> Result<(HemCellState, f64), HemError> {
        let pipe = &self.pipes[end.pipe];
        let g = pipe.geometry();
        let (cell, x) = match end.end {
            PipeEnd::Inlet => (0, 0.0),
            PipeEnd::Outlet => (g.n_cells() - 1, g.length().get::<uom::si::length::meter>()),
        };
        let area = g
            .flow_area_at(uom::si::f64::Length::new::<uom::si::length::meter>(x))
            .get::<square_meter>();
        Ok((pipe.cell_state(cell)?, area))
    }

    /// `(ρ, u_n, p, e, c)` of a cell, its velocity along `normal`.
    fn prim(c: &HemCellState, normal: f64) -> (f64, f64, f64, f64, f64) {
        (
            c.density.get::<kilogram_per_cubic_meter>(),
            normal * c.velocity.get::<uom::si::velocity::meter_per_second>(),
            c.pressure.get::<pascal>(),
            c.specific_internal_energy.get::<joule_per_kilogram>(),
            c.equilibrium_sound_speed
                .get::<uom::si::velocity::meter_per_second>(),
        )
    }

    /// Advance every pipe and volume by one common step.
    ///
    /// # Errors
    ///
    /// Any pipe's step error (the network is then left part-stepped, so treat
    /// the error as fatal for the run), or a volume state the equation of
    /// state cannot close.
    pub fn step(&mut self) -> Result<Time, HemError> {
        // 1. The common time step.
        let mut rate: f64 = 0.0;
        for p in &self.pipes {
            rate = rate.max(p.courant_rate()?.get::<uom::si::frequency::hertz>());
        }
        // 2. Junction fluxes, and the volume rates they imply.
        let mut boundaries: Vec<(PipeEndRef, HemBoundary)> = Vec::new();
        let mut outputs: Vec<(usize, f64, f64)> = Vec::new();
        let mut relief: Vec<(usize, bool, bool)> = Vec::new();
        let mut volume_rates: Vec<(usize, f64, f64)> = Vec::new();
        for (ji, j) in self.junctions.iter().enumerate() {
            match j {
                HemJunction::Flow { a, b, form_loss } => {
                    let r = self.two_end(*a, *b, 1.0, *form_loss, 0.0)?;
                    boundaries.push((*a, r.bc_a));
                    boundaries.push((*b, r.bc_b));
                    rate = rate.max(r.rate);
                    outputs.push((ji, r.mdot, 0.0));
                }
                HemJunction::Pump {
                    a,
                    b,
                    head,
                    reverse_loss,
                } => {
                    let (ca, _) = self.end_cell(*a)?;
                    let rho_a = ca.density.get::<kilogram_per_cubic_meter>();
                    let h = match head {
                        PumpHead::Head(t) => t.at(self.time_s),
                        PumpHead::ShaftPower {
                            power,
                            shutoff_head_m,
                        } => {
                            let m = self.junction_mass_flow.get(ji).copied().unwrap_or(0.0);
                            let w = power.at(self.time_s);
                            if m * GRAVITY * shutoff_head_m > w {
                                w / (m * GRAVITY)
                            } else {
                                *shutoff_head_m
                            }
                        }
                    };
                    // Normal flow (a → b) gets the head; reverse flow is a
                    // resistance K1 + K2 (RELAP-7 Eqs 357-358).
                    let ua =
                        outward(a.end) * ca.velocity.get::<uom::si::velocity::meter_per_second>();
                    let (dp, k) = if ua >= 0.0 {
                        (rho_a * GRAVITY * h, 0.0)
                    } else {
                        (0.0, reverse_loss.0 + reverse_loss.1)
                    };
                    let r = self.two_end(*a, *b, 1.0, k, dp)?;
                    boundaries.push((*a, r.bc_a));
                    boundaries.push((*b, r.bc_b));
                    rate = rate.max(r.rate);
                    outputs.push((ji, r.mdot, r.pump_power));
                }
                HemJunction::Valve {
                    a,
                    b,
                    opening,
                    form_loss,
                } => {
                    let open = opening.at(self.time_s).clamp(0.0, 1.0);
                    if open <= 0.0 {
                        for e in [a, b] {
                            let (c, _) = self.end_cell(*e)?;
                            boundaries.push((
                                *e,
                                to_pipe_frame(e.end, 0.0, c.pressure.get::<pascal>(), 0.0),
                            ));
                        }
                        outputs.push((ji, 0.0, 0.0));
                    } else {
                        let r = self.two_end(*a, *b, open, form_loss / (open * open), 0.0)?;
                        boundaries.push((*a, r.bc_a));
                        boundaries.push((*b, r.bc_b));
                        rate = rate.max(r.rate);
                        outputs.push((ji, r.mdot, 0.0));
                    }
                }
                HemJunction::ReliefValve {
                    a,
                    b,
                    set_pressure_pa,
                    reseat_pressure_pa,
                    area_fraction,
                } => {
                    let (ca, area_a) = self.end_cell(*a)?;
                    let (cb, area_b) = self.end_cell(*b)?;
                    let p_a = ca.pressure.get::<pascal>();
                    let was_open = self.relief_state.get(ji).map_or(false, |s| s.0);
                    let open = if was_open {
                        p_a > *reseat_pressure_pa
                    } else {
                        p_a >= *set_pressure_pa
                    };
                    let p_b = cb.pressure.get::<pascal>();
                    if open {
                        let u_a = ca.velocity.get::<uom::si::velocity::meter_per_second>();
                        let h0 = ca.specific_enthalpy.get::<joule_per_kilogram>() + 0.5 * u_a * u_a;
                        let flow = self.eos.break_flow(
                            ca.pressure,
                            AvailableEnergy::new::<joule_per_kilogram>(h0),
                            cb.pressure,
                        )?;
                        let g = flow.mass_flux.get::<kilogram_per_square_meter_second>();
                        let mdot = g * area_a.min(area_b) * area_fraction;
                        let (ga, gb) = (mdot / area_a, mdot / area_b);
                        let (rho_a, rho_b) = (
                            ca.density.get::<kilogram_per_cubic_meter>(),
                            cb.density.get::<kilogram_per_cubic_meter>(),
                        );
                        boundaries
                            .push((*a, to_pipe_frame(a.end, ga, p_a + ga * ga / rho_a, ga * h0)));
                        boundaries.push((
                            *b,
                            to_pipe_frame(b.end, -gb, p_b + gb * gb / rho_b, -gb * h0),
                        ));
                        let c = ca
                            .equilibrium_sound_speed
                            .get::<uom::si::velocity::meter_per_second>()
                            .max(
                                cb.equilibrium_sound_speed
                                    .get::<uom::si::velocity::meter_per_second>(),
                            );
                        let cell = self.pipes[a.pipe]
                            .geometry()
                            .cell_width()
                            .get::<uom::si::length::meter>();
                        rate = rate.max((c + (ga / rho_a).abs().max((gb / rho_b).abs())) / cell);
                        outputs.push((ji, mdot, 0.0));
                        relief.push((ji, true, flow.choked));
                    } else {
                        boundaries.push((*a, to_pipe_frame(a.end, 0.0, p_a, 0.0)));
                        boundaries.push((*b, to_pipe_frame(b.end, 0.0, p_b, 0.0)));
                        outputs.push((ji, 0.0, 0.0));
                        relief.push((ji, false, false));
                    }
                }
                HemJunction::Turbine { a, b, params } => {
                    let r = self.turbine(*a, *b, params)?;
                    boundaries.push((*a, r.bc_a));
                    boundaries.push((*b, r.bc_b));
                    rate = rate.max(r.rate);
                    outputs.push((ji, r.mdot, r.pump_power));
                }
                HemJunction::Volume(v) => {
                    let st_mix = self.close_volume(v)?;
                    let (mut mdot_in, mut edot_in, mut vol_rate) = (0.0, 0.0, 0.0);
                    for (port_k, (end, k)) in v.connections.iter().enumerate() {
                        // The state this port presents (the mixture for a
                        // plain volume branch).
                        let (st, dp_port, _) = self.port_state(v, &st_mix, port_k);
                        let rho_v = st.density.get::<kilogram_per_cubic_meter>();
                        let (c, area) = self.end_cell(*end)?;
                        let pc = Self::prim(&c, outward(end.end));
                        // Mass, momentum (along the pipe's outward normal) and
                        // energy flux, positive from the pipe into the volume.
                        let (mass, mom_raw, energy, amax) = {
                            // Every port is a characteristic boundary of its pipe
                            // (CORRECTED 2026-11-01, gh:#1073). It was a KNP flux
                            // against the volume at rest, which at steady flow
                            // forces `ρ_L − ρ_R = ρu/a`, i.e. an artificial port
                            // loss `Δp ≈ ρcu` (≈ 10 kPa at 0.38 m/s of 7 MPa
                            // two-phase water), and which at a separated port
                            // moved mass at rest across the unlike states (a
                            // liquid port drained a 0.3 kg volume in four steps).
                            // A linearised acoustic flux has the same `ρcu`. The
                            // port is instead a characteristic
                            // boundary of the pipe, as `HemBoundary::Reservoir`:
                            // the face velocity follows the pipe's outgoing
                            // acoustic invariant, `u = u_L + (p_L − p_f)/Z_L`;
                            // outflow from the volume obeys the stagnation
                            // relation `p_port = p_f + (1 + K)·½ρ_d u²`, inflow
                            // the sink relation `p_f = p_port + K·½ρ_L u²`; the
                            // donor phase carries `ρ`, `ρu` and `ρh₀`.
                            let z_l = pc.0 * pc.4;
                            let p_port = st.pressure.get::<pascal>() + dp_port;
                            let c0 = pc.2 + z_l * pc.1 - p_port;
                            let (u_f, rho_d, h0_d) = if c0 >= 0.0 {
                                let a2 = 0.5 * pc.0 * k;
                                let u = 2.0 * c0 / (z_l + (z_l * z_l + 4.0 * a2 * c0).sqrt());
                                (u, pc.0, pc.3 + pc.2 / pc.0 + 0.5 * pc.1 * pc.1)
                            } else {
                                let a2 = 0.5 * rho_v * (1.0 + k);
                                let u = 2.0 * c0 / (z_l + (z_l * z_l - 4.0 * a2 * c0).sqrt());
                                (u, rho_v, st.specific_enthalpy.get::<joule_per_kilogram>())
                            };
                            let p_f = pc.2 - z_l * (u_f - pc.1);
                            let m = rho_d * u_f;
                            let c_r = st.equilibrium_sound_speed.get::<uom::si::velocity::meter_per_second>();
                            // The face pressure carries the loss.
                            (m, m * u_f + p_f, m * h0_d, (pc.4 + pc.1.abs()).max(c_r + u_f.abs()))
                        };
                        let mom = mom_raw;
                        boundaries.push((*end, to_pipe_frame(end.end, mass, mom, energy)));
                        mdot_in += mass * area;
                        edot_in += energy * area;
                        vol_rate += amax * area / v.volume_m3;
                        rate = rate.max(
                            amax / self.pipes[end.pipe]
                                .geometry()
                                .cell_width()
                                .get::<uom::si::length::meter>(),
                        );
                    }
                    rate = rate.max(vol_rate);
                    volume_rates.push((ji, mdot_in, edot_in));
                }
            }
        }
        let dt = if rate > 0.0 {
            self.courant / rate
        } else {
            f64::INFINITY
        };
        if !dt.is_finite() {
            return Err(HemError::InvalidInput(
                "the network has no wave speed to set a step".into(),
            ));
        }
        // 3. Apply the junction fluxes and time-dependent boundaries, and
        //    advance every pipe.
        for (end, tdb) in &self.time_dependent {
            let bc = self.time_dependent_boundary(*end, tdb, self.time_s)?;
            boundaries.push((*end, bc));
        }
        for (end, bc) in boundaries {
            self.pipes[end.pipe].set_boundary(end.end, bc)?;
        }
        let step = Time::new::<second>(dt);
        for p in &mut self.pipes {
            p.step_by(step)?;
        }
        // 4. The volumes.
        for (ji, mdot, edot) in volume_rates {
            if let HemJunction::Volume(v) = &mut self.junctions[ji] {
                v.mass_kg += mdot * dt;
                v.energy_j += edot * dt;
                if !(v.mass_kg > 0.0) {
                    return Err(HemError::Unphysical {
                        cell: ji,
                        quantity: "volume-branch mass",
                        value: v.mass_kg,
                    });
                }
            }
        }
        self.junction_mass_flow.resize(self.junctions.len(), 0.0);
        self.junction_power.resize(self.junctions.len(), 0.0);
        self.relief_state
            .resize(self.junctions.len(), (false, false));
        for (ji, open, choked) in relief {
            self.relief_state[ji] = (open, choked);
        }
        for (ji, m, w) in outputs {
            self.junction_mass_flow[ji] = m;
            self.junction_power[ji] = w;
        }
        self.time_s += dt;
        Ok(step)
    }

    /// Step until the network time reaches at least `t_end`; returns the
    /// number of steps.
    ///
    /// # Errors
    ///
    /// As [`Self::step`].
    pub fn run_until(&mut self, t_end: Time) -> Result<usize, HemError> {
        let te = t_end.get::<second>();
        let mut n = 0;
        while self.time_s < te {
            self.step()?;
            n += 1;
        }
        Ok(n)
    }
}
