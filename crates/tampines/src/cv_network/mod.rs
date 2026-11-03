// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! The control-volume network: [`ControlVolume`]s joined by
//! [`FlowPath`]s, stepped implicitly in the volume pressures (gh:#1095,
//! with gh:#1094's paths; MELCOR-style control-volume epic gh:#1091;
//! MELCOR 2.2 CVH/FL Reference Manual §6–7, cited, not quoted).
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! # The step
//!
//! ~~A step of `Δt` is a Picard (fixed-point) iteration on the end-of-step
//! state:~~ **CORRECTED 2026-11-02 (gh:#1119):** a step of `Δt` is an
//! iteration on the end-of-step state whose end pressures (step 1) come by
//! default from a Newton step on the path flows (the section below). The
//! Picard (fixed-point) iteration with under-relaxed pressures, described
//! here, is the ablation [`CvNetwork::without_newton_path_flows`] (bit for
//! bit) and the fallback:
//!
//! 1. Each path advances its two velocities from their start-of-step values
//!    with the end pressures of the **latest iterate** (the start state on the
//!    first pass), so at convergence the momentum equation is backward Euler
//!    in the pressures, which is what lets `Δt` exceed the acoustic limit by
//!    orders of magnitude.
//! 2. The path's phase rates give each end volume its outflow (the forward
//!    total from `from`, the backward total from `to`).
//! 3. Every volume is stepped from its start state with those outflows and,
//!    as inflow, **what the donor actually sent** on the previous iterate
//!    (its [`PortTransfer`]): water at its enthalpy, each non-condensable at
//!    the donor's gas temperature.
//! 4. Repeat until the largest relative change of any path's mass rate is
//!    below the tolerance.
//!
//! With `max_iterations = 1` this is loose (operator-split) coupling.
//!
//! # The Newton step on the path flows (gh:#1119)
//!
//! Picard with under-relaxation converges only linearly on a stiff path (a
//! Mark I downcomer), and once the equalising limit binds, a path's rate no
//! longer depends on the relaxed pressures. So the rate residual can be met
//! while those pressures are still far from the end state: a **false
//! convergence**, measured in `cv_network.md`. The Newton step:
//!
//! - **The volumes are linearised by their measured response.** Per
//!   flowing path, its end pressures' response to its flow, by finite
//!   differences of the volumes themselves (`PathResponse`): the donor
//!   stepped again with its outflow raised, and the receiver with its inflow
//!   raised by what the donor then sent more. So condensing at a submerged
//!   exit, bubbling, flashing and the level's head are in it. It is taken
//!   when a path first flows within the step, and re-taken when the rates it
//!   was taken about move by more than 10 %.
//! - **The paths are not linearised.** Each iterate solves the flows
//!   `x = Φ(x)` with the paths' own steps, and the limit, at the linearised
//!   pressures `P̃ + D (x − x_{k−1})`. `P̃` is the latest iterate's pressures
//!   with each lagged receiver's response added, because a receiver is
//!   stepped with what its donor sent on the iterate before. A path's
//!   dead band (a downcomer below its submergence) and the limit's kink are
//!   therefore exact. `Φᵢ` falls as `xᵢ` rises, so each path's root is
//!   bracketed at once and found by regula falsi (`monotone_root`), path by
//!   path (Gauss–Seidel).
//! - **The equalising limit and a breaker's add-back use the same measured
//!   response** for their slopes. The converged limit is unchanged (the
//!   driving difference meets its target); only the convergence rate
//!   changes.
//! - Everything else is the Picard step: the volumes' step, the delivery,
//!   the in-transit inventory, the breakers' hysteresis, the convergence
//!   test (**CORRECTED 2026-11-03, gh:#1124:** but for the floor below).
//! - **A round-off floor on Newton's convergence test.** A path's rate
//!   change counts as converged when the mass it moves in the step is at or
//!   below the f64 resolution of the smaller inventory it joins,
//!   `|Δṁ| Δt ≤ ε_f64 · min(M_from, M_to)` (`M` a volume's water and every
//!   gas, a boundary infinite; `roundoff_floor`). Without it, a path whose
//!   flow toggles at round-off while nothing else flows keeps the residual
//!   (each change over the largest rate, then that tiny flow itself) O(1):
//!   `sembawang`'s Unit 1 building opening, one ulp of pressure from the
//!   environment, made Newton fall back on every step (measured in
//!   `cv_network.md`). Picard's test is unchanged, bit for bit.
//! - **Fallback:** a Newton step that does not converge in 30 iterates, or
//!   meets any error, is re-run by Picard from the start state
//!   ([`NetworkStepReport::newton_fell_back`]; the iterates it spent first
//!   are [`NetworkStepReport::newton_iterations`], gh:#1124).
//! - **Not used** with [`CvNetwork::without_iterate_switch_decisions`] (the
//!   b4c99e210 pin) or with one iteration (loose coupling).
//!
//! On a Mark I stand-in (`tests.rs`, `mark_i_proxy`): mean 4.82 iterations
//! a step against Picard's 18.62, 18.2–20.5 ms against 39.9–45.9 ms, no
//! halving or fallback. **Its results differ from Picard's by up to 3.7 Pa
//! a step**, beyond Picard's tolerance spread (0.025 Pa). On those steps
//! Picard's limited answer does not satisfy the momentum equation at its
//! own end state (it is off by up to 0.41 of the largest rate), and Newton's
//! does (2.1e-5).
//!
//! **Exact conservation.** At the last iterate a receiver's inflow is the
//! donor's outflow of the iterate before, which differs by the remaining
//! residual. That difference (water, each gas, energy) is held per path end
//! as **in-transit** inventory and delivered on the next step, and the
//! network totals count it, so mass, every gas and energy are conserved to
//! round-off whatever the tolerance.
//!
//! ~~**Atmosphere reactions** (`CvNetwork::set_atmosphere_reactions`,
//! gh:#1100) are internal sources: the network's water, gases and energy
//! close as `now = start + boundary_inflow + reaction_totals`
//! (`CvNetwork::reaction_totals`).~~ **CORRECTED 2026-11-01** (gh:#1103:
//! the chemistry moved to `sembawang`): **atmosphere species sources and
//! sinks** ([`CvNetwork::set_atmosphere_species`]) cross the boundary, so
//! the network closes as `now = start + boundary_inflow`.
//!
//! # Limits
//!
//! - ~~The donor volume decides which phase leaves by its level at the port
//!   (a point draw); the path's partial-submergence split sets only the rate.~~
//!   **CORRECTED 2026-11-02 (gh:#1117):** the donor draws both phases in the
//!   path's own split (liquid through the submerged part of the opening, gas
//!   through the rest; MELCOR's FL junction opening fractions), so a junction
//!   straddling the surface moves both. The point draw is the ablation
//!   [`CvNetwork::without_two_phase_junction_draw`].
//! - Boundaries are fixed-state reservoirs ([`NetworkBoundary`]).
//! - ~~A vacuum breaker is opened or shut for a whole step by the
//!   start-of-step pressures, and the equalising limit is taken from the
//!   start-of-step state (junction pressures less the gas column, gh:#1097):
//!   both lag by one step a source that arrives within it (measured in
//!   `cv_network.md` test 7 and in `sembawang`'s containment comparison).~~
//!   **CORRECTED 2026-11-02 (gh:#1116):** both are re-decided on every
//!   Picard iterate from the latest one. The equalising limit is a
//!   linearised end-of-step condition: the gas that brings the driving
//!   difference to its target, with this path's own flow added back as the
//!   latest iterate carries it. That is, the donor's loss from the previous
//!   iterate and the receiver's gain from the one before, each at its
//!   adiabatic slope `γ p/n` (the ablation keeps the ported isothermal
//!   `p/n`). A path fed by a boundary is limited by its receiver alone. A
//!   breaker opens on any iterate where that difference exceeds its set
//!   point, and stays open for the step (hysteresis: at most one change a
//!   step, so no chatter; open is safe because the limit then gives zero
//!   flow below the set point). The old start-of-step decisions are the
//!   ablation [`CvNetwork::without_iterate_switch_decisions`].
//! - The switches are still switches: choking and the limit make a
//!   transient that crosses them non-smooth in `Δt`. On a smooth case the
//!   error at these steps is second-order dominated (`cv_network.md`,
//!   gh:#1116).
//! - The Newton step's volume response is a finite-difference linearisation
//!   (a chord, re-taken on a 10 % move); a path's cross-coupling through a
//!   shared volume enters only through it. A step whose start is far from
//!   linear in the volumes falls back to Picard (none did on the stand-in).
//!   `sembawang`'s case B has **not** been run with it (gh:#1119).
//! - The round-off floor catches a toggle whose mass in a step is below
//!   one f64 resolution of the inventory, not one of a few: on a 1 m²
//!   opening from a 60 m tall N2 vessel at one ulp from its boundary the
//!   toggle moved about 1.5 times the floor, and Newton still fell back on
//!   every step (`cv_network.md`, gh:#1124).

use tampines_steam_tables::gas_mixture::{NonCondensable, R_GAS};
use uom::si::available_energy::joule_per_kilogram;
use uom::si::energy::joule;
use uom::si::f64::{AvailableEnergy, Energy, Mass, MassDensity, MassRate, Pressure, ThermodynamicTemperature, Time};
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use crate::control_volume::{AtmosphereSpeciesRates, ControlVolume, ControlVolumeInputs, ControlVolumeStepReport, DonorState, ElevatedPoolHeat, Source};
use crate::flow_path::{FlowPath, FlowTransfer, PathEnd};
use crate::TampinesError;

/// Gravity, m/s² (as `flow_path`'s).
const G: f64 = 9.80665;

/// A fixed-state reservoir at the end of a path: the environment, a stack.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NetworkBoundary {
    /// Pressure.
    pub pressure: Pressure,
    /// Temperature of its gas.
    pub temperature: ThermodynamicTemperature,
    /// Its gas: steam mass fraction and each non-condensable's mass
    /// fraction (summing to 1 with the steam).
    pub steam_mass_fraction: f64,
    /// Non-condensable mass fractions, [`NonCondensable::ALL`] order.
    pub ncg_mass_fractions: [f64; 6],
}

impl NetworkBoundary {
    /// Dry air-like gas (79 % N2, 21 % O2 by mole) at `p` and `t`.
    pub fn dry_air(pressure: Pressure, temperature: ThermodynamicTemperature) -> Self {
        let (mn, mo) = (0.79 * NonCondensable::N2.molar_mass(), 0.21 * NonCondensable::O2.molar_mass());
        let mut f = [0.0; 6];
        f[NonCondensable::N2.index()] = mn / (mn + mo);
        f[NonCondensable::O2.index()] = mo / (mn + mo);
        Self { pressure, temperature, steam_mass_fraction: 0.0, ncg_mass_fractions: f }
    }

    fn molar_mass(&self) -> f64 {
        let inv: f64 = self.steam_mass_fraction / tampines_steam_tables::gas_mixture::M_H2O
            + NonCondensable::ALL.iter().map(|g| self.ncg_mass_fractions[g.index()] / g.molar_mass()).sum::<f64>();
        1.0 / inv
    }
}

/// One end of a network path.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NetworkEnd {
    /// Port `port` of volume `volume`.
    Volume {
        /// Volume index.
        volume: usize,
        /// Port index in that volume.
        port: usize,
    },
    /// A fixed-state reservoir.
    Boundary(NetworkBoundary),
}

/// A path between two ends.
#[derive(Debug, Clone, PartialEq)]
pub struct NetworkPath {
    /// The path.
    pub path: FlowPath,
    /// Its `from` end.
    pub from: NetworkEnd,
    /// Its `to` end.
    pub to: NetworkEnd,
}

/// Iteration controls: Picard's, and the Newton step's (gh:#1119), whose
/// iterates are further capped at 30 before it falls back to Picard.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NetworkControl {
    /// Iterations per step (1 = loose coupling).
    pub max_iterations: usize,
    /// Largest relative change of a path mass rate at convergence.
    pub tolerance: f64,
}

impl Default for NetworkControl {
    /// 100 iterations to 1e-6 of the largest path rate. (A first default of
    /// 1e-8 stalled at 6e-8 on a two-tank gas network at 0.1 s, 2026-11-01;
    /// it is a solver control, and conservation is exact regardless through
    /// the in-transit inventory.)
    fn default() -> Self {
        Self { max_iterations: 100, tolerance: 1e-6 }
    }
}

/// The report of a network step.
#[derive(Debug, Clone, PartialEq)]
pub struct NetworkStepReport {
    /// Iterations taken: the Newton step's, or Picard's when Newton is
    /// ablated, not used, or fell back (then Picard's alone; the failed
    /// Newton iterates are in [`Self::newton_iterations`]).
    pub iterations: usize,
    /// Newton iterates this step (gh:#1124): equal to `iterations` when
    /// Newton converged, the iterates it spent before giving up when it fell
    /// back (so the step cost `newton_iterations + iterations` passes), zero
    /// when Newton was not used.
    pub newton_iterations: usize,
    /// Last residual (largest relative change of a path rate).
    pub residual: f64,
    /// Each path's transfer at the last iterate.
    pub transfers: Vec<FlowTransfer>,
    /// Each volume's own report at the last iterate, in the order the
    /// volumes were added (gh:#1100): among others its
    /// `pool_heat_to_atmosphere`, the pool heat that went into the
    /// atmosphere because the pool was dry or boiled dry within the step,
    /// which a caller cannot otherwise see.
    pub volumes: Vec<ControlVolumeStepReport>,
    /// Per path, whether the equalising limit cut its gas flow at the last
    /// iterate (gh:#1116).
    pub equalising_limited: Vec<bool>,
    /// Whether the Newton step on the path flows (gh:#1119) failed to
    /// converge and this step was re-run with the under-relaxed Picard
    /// iteration from its start state. `false` when Newton converged or was
    /// not used.
    pub newton_fell_back: bool,
}

/// Inventory carried between steps by a path end: mass that left a donor on
/// the last iterate but was not yet delivered (see the module docs).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct InTransit {
    water_kg: f64,
    ncg_kg: [f64; 6],
    energy_j: f64,
}

/// A network of control volumes and flow paths.
#[derive(Debug, Clone)]
pub struct CvNetwork {
    volumes: Vec<ControlVolume>,
    paths: Vec<NetworkPath>,
    /// Per path: in transit toward its `to` end, toward its `from` end.
    transit: Vec<[InTransit; 2]>,
    /// External heat into each volume's pool, W (held until changed).
    pool_heat_w: Vec<f64>,
    /// External heat into each volume's pool at elevations (held until
    /// changed; gh:#1100).
    pool_heat_at: Vec<Vec<ElevatedPoolHeat>>,
    /// External heat into each volume's atmosphere, W, and steam condensed
    /// out of it onto the caller's structures, kg/s (held; gh:#1097).
    atm_heat_w: Vec<f64>,
    atm_sink_kg_s: Vec<f64>,
    /// Species sources and sinks on each volume's atmosphere (held;
    /// gh:#1103).
    atm_species: Vec<AtmosphereSpeciesRates>,
    /// External sources, per volume per port (held; gh:#1097).
    sources: Vec<Vec<Source>>,
    /// Net mass and energy that crossed boundaries into the network.
    boundary_in: InTransit,
    time_s: f64,
    control: NetworkControl,
    /// Switch decisions (equalising limit, vacuum breakers) from the latest
    /// Picard iterate (default, gh:#1116); else from the start of the step.
    iterate_switches: bool,
    /// A donor port moves both phases in the path's own split (default,
    /// gh:#1117); else one phase by its level (the point draw).
    two_phase_draw: bool,
    /// A Newton step on the path flows (default, gh:#1119); else the
    /// under-relaxed Picard iteration.
    newton: bool,
}

fn mr(x: f64) -> MassRate {
    MassRate::new::<kilogram_per_second>(x)
}

/// Newton iterates a step may take before it falls back to Picard
/// (gh:#1119). Newton converging needs a handful; 30 is far past that and
/// still well short of Picard's 100.
const NEWTON_MAX_ITERATIONS: usize = 30;

/// A path's signed mass rate, both phases, positive `from → to`, kg/s.
fn signed_rate(t: &FlowTransfer) -> f64 {
    t.liquid_mass_rate.get::<kilogram_per_second>() + t.gas_mass_rate.get::<kilogram_per_second>()
}

/// The largest change of path `np`'s mass rate, kg/s, that the end state
/// cannot resolve over a step of `dts` s (gh:#1124): the mass it moves in
/// the step, `|Δṁ| Δt`, at or below the f64 resolution of the smaller
/// inventory it joins, `f64::EPSILON · min(M_from, M_to)`, with `M` a
/// volume's water and every gas at the start of the step and a boundary
/// infinite. A change that small moves the volume's pressure by less than
/// about one ulp, so the iterates can toggle on it forever (Unit 1's
/// building opening at one ulp below the environment's pressure, measured
/// in `cv_network.md`). Newton's convergence test treats it as converged.
fn roundoff_floor(volumes: &[ControlVolume], np: &NetworkPath, dts: f64) -> f64 {
    let inventory = |e: &NetworkEnd| match e {
        NetworkEnd::Volume { volume, .. } => {
            let v = &volumes[*volume];
            v.water_mass().get::<kilogram>() + v.ncg_masses().iter().map(|m| m.get::<kilogram>()).sum::<f64>()
        }
        NetworkEnd::Boundary(_) => f64::INFINITY,
    };
    f64::EPSILON * inventory(&np.from).min(inventory(&np.to)) / dts
}

/// One path's linearised effect on the network's end pressures for the
/// Newton step (gh:#1119): per path `j`, `[from end, to end]`, the change of
/// that end's pressure per kg/s of this path's signed flow, Pa s/kg, by
/// finite differences of the volumes themselves (so condensing at a
/// submerged exit, bubbling, flashing and the level's head are in it).
#[derive(Debug, Clone, PartialEq)]
struct PathResponse {
    /// Through the donor: its outflow raised.
    donor: Vec<[f64; 2]>,
    /// Through the receiver: its inflow raised by what the donor then sent.
    receiver: Vec<[f64; 2]>,
    /// Whether the receiver sees this path's flow one iterate late (a volume
    /// donor: the receiver is stepped with what it sent on the iterate
    /// before); a boundary sends at the current rate.
    receiver_lagged: bool,
    /// The signed rates it was taken about: the donor's outflow, and what
    /// the receiver was given (the iterate before's, when lagged).
    base: (f64, f64),
}

impl PathResponse {
    /// Whether it was taken about rates within 10 % of `(outflow, inflow)`
    /// (gh:#1119: a pool's response to bubbling taken at zero inflow, on
    /// the first iterate the path flows, made the next iterates oscillate).
    fn holds_at(&self, outflow: f64, inflow: f64) -> bool {
        let near = |a: f64, b: f64| (a - b).abs() <= 0.1 * a.abs().max(b.abs());
        near(self.base.0, outflow) && near(self.base.1, inflow)
    }
}

/// The equalising limit's linearisation from path `i`'s own measured
/// response `d` (gh:#1119), in place of the adiabatic slopes `γ p/n`:
/// `(stiffness, own)`, the fall of the forward driving difference per kg/s
/// of forward flow, Pa s/kg, and the forward difference this path's own
/// flow already took out of the latest iterate, Pa: the donor's at `g1`
/// (the previous iterate's gas rate) and a lagged receiver's at `g2` (the
/// one before). `None` if the stiffness is not positive and finite (then
/// the adiabatic slopes stand).
fn limit_response(d: &PathResponse, i: usize, g1: f64, g2: f64) -> Option<(f64, f64)> {
    let fall = |r: &[[f64; 2]]| r[i][1] - r[i][0];
    let stiffness = fall(&d.donor) + fall(&d.receiver);
    let g_rec = if d.receiver_lagged { g2 } else { g1 };
    let own = fall(&d.donor) * g1 + fall(&d.receiver) * g_rec;
    (stiffness.is_finite() && stiffness > 0.0 && own.is_finite()).then_some((stiffness, own))
}

/// The equalising limit's cap on a path's gas rate, kg/s, from its
/// measured response (gh:#1119): the gas that brings the end-of-step driving
/// difference to its target. `drive` is the latest iterate's `(forward,
/// backward)` driving difference, `(stiffness, own)` its [`limit_response`],
/// `breaker_dp` a breaker's opening difference (zero otherwise), `mg` the
/// gas rate (its sign picks the direction).
fn measured_cap(drive: (f64, f64), response: (f64, f64), breaker_dp: f64, mg: f64) -> f64 {
    let (stiffness, own) = response;
    let dp_eq = if mg > 0.0 { drive.0 + own - breaker_dp } else { drive.1 - own };
    if dp_eq > 0.0 {
        dp_eq / stiffness
    } else {
        0.0
    }
}

/// The root of `h(x) = x − φ(x)` for a non-increasing `φ` (so `h` rises
/// with slope at least 1), from `x0`, to `|h| ≤ max(eps, rel |x|)`
/// (gh:#1119). The slope
/// brackets the root at once: in `[x0 − h(x0), x0]` when `h(x0) > 0`, else
/// in `[x0, x0 − h(x0)]`. Then the Illinois variant of regula falsi (the
/// bracket never grows, so it cannot cycle as an unguarded Newton step
/// does across a kink: a path's dead band, the limit).
///
/// # Errors
///
/// `φ`'s errors; [`TampinesError::Numerical`] if the bracket does not hold
/// (`φ` rose somewhere) or 80 evaluations do not meet the tolerance.
fn monotone_root<F>(x0: f64, eps: f64, rel: f64, mut phi: F) -> Result<f64, TampinesError>
where
    F: FnMut(f64) -> Result<f64, TampinesError>,
{
    let tol = |x: f64| eps.max(rel * x.abs());
    let h0 = x0 - phi(x0)?;
    if h0.abs() <= tol(x0) {
        return Ok(x0);
    }
    let x1 = x0 - h0;
    let h1 = x1 - phi(x1)?;
    if h1.abs() <= tol(x1) {
        return Ok(x1);
    }
    if h0.signum() == h1.signum() {
        return Err(TampinesError::Numerical(format!("network Newton step: no bracket ({x0:e}, {h0:e}), ({x1:e}, {h1:e})")));
    }
    let ((mut a, mut ha), (mut b, mut hb)) = ((x0, h0), (x1, h1));
    let mut side = 0_i8;
    for _ in 0..80 {
        let x = (a * hb - b * ha) / (hb - ha);
        let hx = x - phi(x)?;
        if hx.abs() <= tol(x) || (b - a).abs() <= tol(x) {
            return Ok(x);
        }
        if hx.signum() == ha.signum() {
            (a, ha) = (x, hx);
            if side == -1 {
                hb *= 0.5;
            }
            side = -1;
        } else {
            (b, hb) = (x, hx);
            if side == 1 {
                ha *= 0.5;
            }
            side = 1;
        }
    }
    Err(TampinesError::Numerical("network Newton step: the path solve did not converge".into()))
}

/// The end pressures each path sees with the network's flows at `x`, by the
/// volumes' measured responses about the latest iterate (gh:#1119): `pt`
/// the latest pressures (each lagged receiver corrected), plus each
/// responding path's `D (x − x1)`, `x1` the rates the latest iterate was
/// stepped with.
fn linear_pressures(pt: &[[f64; 2]], d: &[Option<PathResponse>], x: &[f64], x1: &[f64]) -> Vec<[f64; 2]> {
    let mut u = pt.to_vec();
    for (i, di) in d.iter().enumerate() {
        if let Some(di) = di {
            let dx = x[i] - x1[i];
            for (uj, (dj, rj)) in u.iter_mut().zip(di.donor.iter().zip(&di.receiver)) {
                uj[0] += (dj[0] + rj[0]) * dx;
                uj[1] += (dj[1] + rj[1]) * dx;
            }
        }
    }
    u
}

/// The latest pressures with each lagged receiver's response to `x1 − x2`
/// added (gh:#1119): the latest iterate's receivers were stepped with what
/// their donors sent on the iterate before, so `P̃` is the state as if each
/// had received `x1`.
fn lag_corrected(p_raw: &[[f64; 2]], d: &[Option<PathResponse>], x1: &[f64], x2: &[f64]) -> Vec<[f64; 2]> {
    let mut pt = p_raw.to_vec();
    for (i, di) in d.iter().enumerate() {
        if let Some(di) = di.as_ref().filter(|di| di.receiver_lagged) {
            let dx = x1[i] - x2[i];
            for (ptj, rj) in pt.iter_mut().zip(&di.receiver) {
                ptj[0] += rj[0] * dx;
                ptj[1] += rj[1] * dx;
            }
        }
    }
    pt
}

impl CvNetwork {
    /// An empty network.
    pub fn new(control: NetworkControl) -> Self {
        Self {
            volumes: Vec::new(),
            paths: Vec::new(),
            transit: Vec::new(),
            pool_heat_w: Vec::new(),
            pool_heat_at: Vec::new(),
            atm_heat_w: Vec::new(),
            atm_sink_kg_s: Vec::new(),
            atm_species: Vec::new(),
            sources: Vec::new(),
            boundary_in: InTransit::default(),
            time_s: 0.0,
            control,
            iterate_switches: true,
            two_phase_draw: true,
            newton: true,
        }
    }

    /// Ablation (gh:#1119): solve each step by the under-relaxed Picard
    /// iteration on the end pressures, as before 2026-11-02 (bit for bit),
    /// instead of the Newton step on the path flows. Picard converges on a
    /// stiff path (Mark I downcomers) only linearly.
    pub fn without_newton_path_flows(mut self) -> Self {
        self.newton = false;
        self
    }

    /// Whether a step is solved by the Newton step on the path flows (the
    /// default; see the module docs). It is used only with the switch
    /// decisions from the latest iterate and more than one iteration.
    pub fn newton_path_flows(&self) -> bool {
        self.newton
    }

    /// Ablation (gh:#1116): take the equalising limit and the vacuum
    /// breakers' open/shut decisions from the start-of-step state for the
    /// whole step, as before 2026-11-02 (bit for bit). They then lag by one
    /// step a source that arrives within it.
    pub fn without_iterate_switch_decisions(mut self) -> Self {
        self.iterate_switches = false;
        self
    }

    /// Ablation (gh:#1117): a donor volume draws one phase through a path's
    /// port by its level at the port's elevation (the point draw), whatever
    /// the path's split, as before 2026-11-02 (bit for bit). A junction
    /// straddling the surface then moves pure liquid at the path's total
    /// rate while the level is above the opening's centre, and pure gas
    /// below it.
    pub fn without_two_phase_junction_draw(mut self) -> Self {
        self.two_phase_draw = false;
        self
    }

    /// Whether a donor port moves both phases in the path's split (the
    /// default).
    pub fn two_phase_junction_draw(&self) -> bool {
        self.two_phase_draw
    }

    /// Whether the switch decisions come from the latest Picard iterate (the
    /// default) rather than the start of the step.
    pub fn iterate_switch_decisions(&self) -> bool {
        self.iterate_switches
    }

    /// Add a volume; returns its index.
    pub fn add_volume(&mut self, v: ControlVolume) -> usize {
        self.sources.push(vec![Source::none(); v.ports().len()]);
        self.volumes.push(v);
        self.pool_heat_w.push(0.0);
        self.pool_heat_at.push(Vec::new());
        self.atm_heat_w.push(0.0);
        self.atm_sink_kg_s.push(0.0);
        self.atm_species.push(AtmosphereSpeciesRates::none());
        self.volumes.len() - 1
    }

    /// Add a path; returns its index.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for a missing volume or port, or a
    /// volume port used by two paths.
    pub fn add_path(&mut self, path: NetworkPath) -> Result<usize, TampinesError> {
        for end in [path.from, path.to] {
            if let NetworkEnd::Volume { volume, port } = end {
                let v = self.volumes.get(volume).ok_or_else(|| TampinesError::InvalidInput(format!("no volume {volume}")))?;
                if port >= v.ports().len() {
                    return Err(TampinesError::InvalidInput(format!("volume {volume} has no port {port}")));
                }
                let used = self.paths.iter().any(|p| p.from == end || p.to == end);
                if used {
                    return Err(TampinesError::InvalidInput(format!("volume {volume} port {port} already has a path")));
                }
                if self.sources[volume][port] != Source::none() {
                    return Err(TampinesError::InvalidInput(format!("volume {volume} port {port} carries a source")));
                }
            }
        }
        self.paths.push(path);
        self.transit.push([InTransit::default(); 2]);
        Ok(self.paths.len() - 1)
    }

    /// Set the external heat into volume `i`'s pool (held until changed).
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for no such volume.
    pub fn set_pool_heat(&mut self, i: usize, q: uom::si::f64::Power) -> Result<(), TampinesError> {
        let h = self.pool_heat_w.get_mut(i).ok_or_else(|| TampinesError::InvalidInput(format!("no volume {i}")))?;
        *h = q.get::<uom::si::power::watt>();
        Ok(())
    }

    /// Set the external heat into volume `i`'s pool at elevations (held
    /// until changed; replaces the previous list; see
    /// [`ControlVolumeInputs::pool_heat_at`]), beside
    /// [`Self::set_pool_heat`]'s heat shared by mass (gh:#1100).
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for no such volume, or a band with a
    /// non-finite elevation or power or its top below its bottom.
    pub fn set_pool_heat_at(&mut self, i: usize, heats: &[ElevatedPoolHeat]) -> Result<(), TampinesError> {
        for h in heats {
            let (z0, z1, w) = (h.bottom.get::<meter>(), h.top.get::<meter>(), h.power.get::<uom::si::power::watt>());
            if !(z0.is_finite() && z1.is_finite() && z1 >= z0 && w.is_finite()) {
                return Err(TampinesError::InvalidInput(format!("pool heat at an elevation {h:?}")));
            }
        }
        let h = self.pool_heat_at.get_mut(i).ok_or_else(|| TampinesError::InvalidInput(format!("no volume {i}")))?;
        *h = heats.to_vec();
        Ok(())
    }

    /// Set the external heat into volume `i`'s atmosphere (held until
    /// changed): the caller's heat structures (gh:#1097).
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for no such volume.
    pub fn set_atmosphere_heat(&mut self, i: usize, q: uom::si::f64::Power) -> Result<(), TampinesError> {
        let h = self.atm_heat_w.get_mut(i).ok_or_else(|| TampinesError::InvalidInput(format!("no volume {i}")))?;
        *h = q.get::<uom::si::power::watt>();
        Ok(())
    }

    /// Set the steam condensed out of volume `i`'s atmosphere onto the
    /// caller's structures (held until changed; see
    /// [`ControlVolumeInputs::atmosphere_steam_sink`]).
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for no such volume or a negative or
    /// non-finite rate.
    pub fn set_atmosphere_steam_sink(&mut self, i: usize, rate: MassRate) -> Result<(), TampinesError> {
        let x = rate.get::<kilogram_per_second>();
        if !(x.is_finite() && x >= 0.0) {
            return Err(TampinesError::InvalidInput(format!("steam sink {x} kg/s")));
        }
        let h = self.atm_sink_kg_s.get_mut(i).ok_or_else(|| TampinesError::InvalidInput(format!("no volume {i}")))?;
        *h = x;
        Ok(())
    }

    /// Set the species sources and sinks on volume `i`'s atmosphere (held
    /// until changed; see [`ControlVolumeInputs::atmosphere_species`]): kg/s
    /// per species, negative removing, each at its code molar enthalpy at
    /// the start-of-step atmosphere (gh:#1103; a caller's chemistry, e.g.
    /// `sembawang`'s burns, sets them with the matching heat by
    /// [`Self::set_atmosphere_heat`]). They cross the boundary account
    /// ([`Self::boundary_inflow`]).
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for no such volume or a non-finite
    /// rate.
    pub fn set_atmosphere_species(&mut self, i: usize, rates: AtmosphereSpeciesRates) -> Result<(), TampinesError> {
        let all = std::iter::once(rates.steam).chain(rates.ncg);
        if all.map(|m| m.get::<kilogram_per_second>()).any(|x| !x.is_finite()) {
            return Err(TampinesError::InvalidInput(format!("species rates {rates:?}")));
        }
        let r = self.atm_species.get_mut(i).ok_or_else(|| TampinesError::InvalidInput(format!("no volume {i}")))?;
        *r = rates;
        Ok(())
    }

    /// Set an external source on port `port` of volume `volume` (held until
    /// changed): a relief valve's discharge, a vessel leak, a core's
    /// hydrogen (gh:#1097). It enters through the volume's own port model
    /// (bubbles below the level), and the network counts it as boundary
    /// inflow.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for no such volume or port, or a port
    /// a path already uses.
    pub fn set_source(&mut self, volume: usize, port: usize, source: Source) -> Result<(), TampinesError> {
        let end = NetworkEnd::Volume { volume, port };
        if self.paths.iter().any(|p| p.from == end || p.to == end) {
            return Err(TampinesError::InvalidInput(format!("volume {volume} port {port} has a path")));
        }
        let s = self
            .sources
            .get_mut(volume)
            .and_then(|v| v.get_mut(port))
            .ok_or_else(|| TampinesError::InvalidInput(format!("no volume {volume} port {port}")))?;
        *s = source;
        Ok(())
    }

    /// Set path `i`'s valve open fraction ([`FlowPath::set_open_fraction`]).
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for no such path.
    pub fn set_path_open_fraction(&mut self, i: usize, fraction: f64) -> Result<(), TampinesError> {
        let p = self.paths.get_mut(i).ok_or_else(|| TampinesError::InvalidInput(format!("no path {i}")))?;
        p.path.set_open_fraction(fraction);
        Ok(())
    }

    /// The volumes.
    pub fn volume(&self, i: usize) -> Option<&ControlVolume> {
        self.volumes.get(i)
    }

    /// The paths.
    pub fn path(&self, i: usize) -> Option<&NetworkPath> {
        self.paths.get(i)
    }

    /// Network time.
    pub fn time(&self) -> Time {
        Time::new::<second>(self.time_s)
    }

    /// Water in the volumes and in transit, kg.
    pub fn water_mass(&self) -> Mass {
        let v: f64 = self.volumes.iter().map(|c| c.water_mass().get::<kilogram>()).sum();
        let t: f64 = self.transit.iter().flatten().map(|x| x.water_kg).sum();
        Mass::new::<kilogram>(v + t)
    }

    /// Each non-condensable in the volumes and in transit, kg.
    pub fn ncg_masses(&self) -> [f64; 6] {
        let mut m = [0.0; 6];
        for c in &self.volumes {
            for (k, x) in c.ncg_masses().iter().enumerate() {
                m[k] += x.get::<kilogram>();
            }
        }
        for x in self.transit.iter().flatten() {
            for k in 0..6 {
                m[k] += x.ncg_kg[k];
            }
        }
        m
    }

    /// Energy in the volumes and in transit, J.
    pub fn total_energy(&self) -> Energy {
        let v: f64 = self.volumes.iter().map(|c| c.total_energy().get::<joule>()).sum();
        let t: f64 = self.transit.iter().flatten().map(|x| x.energy_j).sum();
        Energy::new::<joule>(v + t)
    }

    /// Net water, each gas and energy that entered from boundaries since
    /// construction (negative if out): `(water kg, gases kg, energy J)`.
    /// Since gh:#1097 it also counts the external sources, the atmosphere
    /// steam sinks and the external heats (pool and atmosphere); ~~pool heat
    /// was not counted~~ (before 2026-11-01 the pool heat was left out,
    /// which no test exercised).
    pub fn boundary_inflow(&self) -> (f64, [f64; 6], f64) {
        (self.boundary_in.water_kg, self.boundary_in.ncg_kg, self.boundary_in.energy_j)
    }

    /// The `PathEnd` an end presents, from `volumes` (the latest iterate).
    fn path_end(volumes: &[ControlVolume], end: &NetworkEnd, elevation: f64) -> PathEnd {
        match end {
            NetworkEnd::Volume { volume, .. } => {
                let v = &volumes[*volume];
                let (liq, gas): (DonorState, DonorState) = v.phase_states_at(uom::si::f64::Length::new::<meter>(elevation));
                let x_steam_mol = {
                    let a = v.atmosphere();
                    let n_s = a.steam_kg / tampines_steam_tables::gas_mixture::M_H2O;
                    let n = n_s + a.ncg_total_moles();
                    if n > 0.0 { n_s / n } else { 0.0 }
                };
                let molar = {
                    let a = v.atmosphere();
                    let n = a.steam_kg / tampines_steam_tables::gas_mixture::M_H2O + a.ncg_total_moles();
                    a.total_mass().get::<kilogram>() / n
                };
                PathEnd {
                    pressure: v.pressure_at_elevation(uom::si::f64::Length::new::<meter>(elevation)),
                    pool_level: v.swollen_level(),
                    liquid_density: liq.density,
                    liquid_enthalpy: liq.specific_enthalpy,
                    gas_density: gas.density,
                    gas_enthalpy: gas.specific_enthalpy,
                    gas_temperature: gas.temperature,
                    gas_molar_mass: molar,
                    gas_steam_mole_fraction: x_steam_mol,
                }
            }
            NetworkEnd::Boundary(b) => {
                let t = b.temperature.get::<kelvin>();
                let m = b.molar_mass();
                let rho = b.pressure.get::<pascal>() * m / (R_GAS * t);
                PathEnd {
                    pressure: b.pressure,
                    pool_level: uom::si::f64::Length::new::<meter>(-1.0e9),
                    liquid_density: MassDensity::new::<kilogram_per_cubic_meter>(1000.0),
                    liquid_enthalpy: AvailableEnergy::new::<joule_per_kilogram>(0.0),
                    gas_density: MassDensity::new::<kilogram_per_cubic_meter>(rho),
                    gas_enthalpy: AvailableEnergy::new::<joule_per_kilogram>(0.0),
                    gas_temperature: b.temperature,
                    gas_molar_mass: m,
                    gas_steam_mole_fraction: 0.0,
                }
            }
        }
    }

    /// The source a receiver gets from a delivery `(water kg, gas kg, energy
    /// J)` over `dt`, closing the energy on the receiver's model. Negative
    /// components (an over-delivery last step) are not delivered: they, and
    /// any energy the gas alone cannot carry, are returned as what stays in
    /// transit.
    fn source_from(receiver: &ControlVolume, water: f64, ncg: &[f64; 6], energy: f64, t_gas: f64, dt: f64) -> (Source, InTransit) {
        let model = receiver.gas_model();
        let mut left = InTransit::default();
        let mut gas = [0.0; 6];
        for q in 0..6 {
            if ncg[q] >= 0.0 {
                gas[q] = ncg[q];
            } else {
                left.ncg_kg[q] = ncg[q];
            }
        }
        let h_ncg: f64 = NonCondensable::ALL
            .iter()
            .map(|g| gas[g.index()] / g.molar_mass() * (model.ncg_u(*g, t_gas) + R_GAS * t_gas))
            .sum();
        let mut s = Source::none();
        s.ncg_temperature = ThermodynamicTemperature::new::<kelvin>(t_gas);
        for g in NonCondensable::ALL {
            s.ncg_mass_rates[g.index()] = mr(gas[g.index()] / dt);
        }
        // Water carries the rest of the energy when there is enough of it
        // with a sane enthalpy; otherwise the gas's own enthalpy arrives and
        // the rest stays in transit.
        let h_w = if water > 1e-12 { (energy - h_ncg) / water } else { f64::NAN };
        if water > 1e-12 && h_w.is_finite() && h_w > 0.0 && h_w < 5.0e6 {
            s.water_mass_rate = mr(water / dt);
            s.water_specific_enthalpy = AvailableEnergy::new::<joule_per_kilogram>(h_w);
        } else {
            left.water_kg = water;
            left.energy_j = energy - h_ncg;
        }
        (s, left)
    }

    /// One network step of `dt` (see the module docs).
    ///
    /// # Errors
    ///
    /// The volumes' and paths' errors; [`TampinesError::Numerical`] if more
    /// than one iteration was allowed and the tolerance was not met.
    pub fn step(&mut self, dt: Time) -> Result<NetworkStepReport, TampinesError> {
        // gh:#1119: Newton on the path flows, with the switch decisions of
        // the latest iterate only (the lagged ablation is the b4c99e210 pin)
        // and never for loose coupling (one pass has nothing to solve).
        if self.newton && self.iterate_switches && self.control.max_iterations > 1 {
            // Nothing is committed before convergence, so a failed Newton
            // step leaves the start state for Picard. Any error falls back:
            // a far Newton iterate can take a volume outside its tables
            // where the Picard iterates would not; if Picard fails too, its
            // error is the one returned.
            let mut tried = 0;
            match self.step_solve(dt, true, &mut tried) {
                Ok(r) => return Ok(r),
                Err(_) => {
                    let mut r = self.step_solve(dt, false, &mut 0)?;
                    r.newton_fell_back = true;
                    r.newton_iterations = tried;
                    return Ok(r);
                }
            }
        }
        self.step_solve(dt, false, &mut 0)
    }

    /// One step solved by Newton on the path flows (`newton`) or by the
    /// under-relaxed Picard iteration (see the module docs). `tried` is set
    /// to the iterates taken, whether or not the step converges.
    fn step_solve(&mut self, dt: Time, newton: bool, tried: &mut usize) -> Result<NetworkStepReport, TampinesError> {
        let dts = dt.get::<second>();
        let start_vols = self.volumes.clone();
        // gh:#1124, Newton's convergence test only: per path, the largest
        // change of its rate that the end state cannot resolve, kg/s (see
        // `roundoff_floor`).
        let floor: Vec<f64> = if newton { self.paths.iter().map(|np| roundoff_floor(&start_vols, np, dts)).collect() } else { Vec::new() };
        // A vacuum breaker opens or stays shut for the whole step by the
        // start-of-step pressures (gh:#1097: decided on each Picard iterate,
        // the first pass equalised the two volumes, the second found the
        // breaker shut, and the step converged with no flow, the breaker
        // stuck shut at any pressure difference; found on the Mark I
        // containment, 2026-11-01). Its equalising limit then stops at the
        // opening difference, not at equal pressures.
        let mut breaker_dp: Vec<f64> = vec![0.0; self.paths.len()];
        // Each path's start-of-step driving pressure difference for its gas
        // phase, forward and backward: the junction pressures (with any
        // liquid head over a submerged junction) less the donor gas column
        // `ρ_g g Δz`. The equalising limit stops where it vanishes (gh:#1097:
        // taken from the atmospheres' pressures it let the downcomers
        // equalise the drywell with the torus gas, ignoring the 11.8 kPa of
        // submergence, so the next iterate drove the gas back and the step
        // did not converge; found on the Mark I containment, 2026-11-01).
        let paths = &self.paths;
        let drive_from = |vols: &[ControlVolume]| -> Vec<(f64, f64)> {
            paths
                .iter()
                .map(|np| {
                    let g = *np.path.geometry();
                    let (zf, zt) = (g.from_elevation.get::<meter>(), g.to_elevation.get::<meter>());
                    let a = Self::path_end(vols, &np.from, zf);
                    let b = Self::path_end(vols, &np.to, zt);
                    let (pa, pb) = (a.pressure.get::<pascal>(), b.pressure.get::<pascal>());
                    let (ra, rb) = (a.gas_density.get::<kilogram_per_cubic_meter>(), b.gas_density.get::<kilogram_per_cubic_meter>());
                    (pa - pb - ra * G * (zt - zf), pb - pa - rb * G * (zf - zt))
                })
                .collect()
        };
        let drive: Vec<(f64, f64)> = drive_from(&self.volumes);
        // gh:#1116: with `iterate_switches` (the default) the limit and the
        // breakers are re-decided on every iterate from the latest one; else
        // (the ablation) from the start-of-step state above, for the step.
        let lagged = !self.iterate_switches;
        let mut breaker_open: Vec<bool> = vec![false; self.paths.len()];
        let start_paths: Vec<FlowPath> = self
            .paths
            .iter()
            .enumerate()
            .map(|(pi, np)| {
                let mut p = np.path.clone();
                if let Some(dp_open) = p.opening_pressure_difference() {
                    let g = *p.geometry();
                    let a = Self::path_end(&self.volumes, &np.from, g.from_elevation.get::<meter>());
                    let b = Self::path_end(&self.volumes, &np.to, g.to_elevation.get::<meter>());
                    let open = a.pressure.get::<pascal>() - b.pressure.get::<pascal>() > dp_open;
                    p.hold_open(Some(open));
                    breaker_open[pi] = open;
                    breaker_dp[pi] = dp_open;
                }
                p
            })
            .collect();
        let mut latest = start_vols.clone();
        // Per path, what the donor actually sent on the previous iterate:
        // [toward `to`, toward `from`] as (water, gases, energy, gas T).
        let mut sent: Vec<[(f64, [f64; 6], f64, f64); 2]> = vec![[(0.0, [0.0; 6], 0.0, 300.0); 2]; self.paths.len()];
        let mut prev_rates: Vec<f64> = vec![f64::NAN; self.paths.len()];
        let mut used_p: Vec<Option<(f64, f64)>> = vec![None; self.paths.len()];
        // Per path, the gas moles it moved on the previous iterate and on the
        // one before, positive `from → to` (gh:#1116). In the latest iterate
        // the donor has lost the first and the receiver has gained the
        // second, because a receiver is stepped with what the donor sent on
        // the iterate before (step 3 of the module docs). Both are zero on the
        // first iterate, whose latest state is the start state (its limit
        // differs from the ablation's only in the adiabatic slope).
        let mut prev_mol: Vec<f64> = vec![0.0; self.paths.len()];
        let mut prev2_mol: Vec<f64> = vec![0.0; self.paths.len()];
        let mut limited: Vec<bool> = vec![false; self.paths.len()];
        let (mut omega, mut last_residual) = (1.0_f64, f64::INFINITY);
        // gh:#1119, the Newton step's state. Per path: its end-pressure
        // response (`D`, taken once a step) and its signed rate on the last
        // two iterates.
        let n_paths = self.paths.len();
        let mut jac_d: Vec<Option<PathResponse>> = vec![None; n_paths];
        let (mut x1, mut x2): (Vec<f64>, Vec<f64>) = (vec![0.0; n_paths], vec![0.0; n_paths]);
        // And each path's gas rate on the last two iterates, for the limit.
        let (mut g1, mut g2): (Vec<f64>, Vec<f64>) = (vec![0.0; n_paths], vec![0.0; n_paths]);
        let max_iterations = if newton { self.control.max_iterations.min(NEWTON_MAX_ITERATIONS) } else { self.control.max_iterations };
        let (mut residual, mut iterations): (f64, usize);
        let mut transfers: Vec<FlowTransfer> = Vec::new();
        let (mut new_vols, mut new_paths, mut boundary_step);
        for k in 1..=max_iterations {
            iterations = k;
            *tried = k;
            // 1. Paths from the latest iterate.
            new_paths = start_paths.clone();
            transfers.clear();
            // The state the switches are decided on (gh:#1116): the latest
            // iterate, or the start of the step for the ablation.
            let iterate_drive = if lagged || k == 1 { None } else { Some(drive_from(&latest)) };
            let (drive_k, vols_k): (&[(f64, f64)], &[ControlVolume]) = match &iterate_drive {
                Some(d) => (d, &latest),
                None => (&drive, &start_vols),
            };
            // `p / n` and the molar mass of a volume end (a boundary: none).
            let side = |e: &NetworkEnd| -> Option<(f64, f64, f64)> {
                match e {
                    NetworkEnd::Volume { volume, .. } => {
                        let v = &vols_k[*volume];
                        let a = v.atmosphere();
                        let n = a.steam_kg / tampines_steam_tables::gas_mixture::M_H2O + a.ncg_total_moles();
                        Some((v.atmosphere_pressure().get::<pascal>(), n, a.total_mass().get::<kilogram>() / n))
                    }
                    NetworkEnd::Boundary(_) => None,
                }
            };
            // An end's pressure response to a mole leaving or arriving
            // within the step: adiabatic, `γ p / n` with `γ = 1 + n R / C_v`
            // from the volume's own gas model (gas leaving carries its
            // enthalpy, so the rest cools; gas arriving brings its own). The
            // ablation's limit (below) keeps the ported isothermal `p / n`.
            // The converged limit does not depend on this slope (it ends
            // where the driving difference meets its target); the slope
            // sets how fast the iterates get there: with `p / n` they
            // oscillated by a factor of about −0.4 per iterate, `1 − γ`
            // (sembawang's building openings, 2 → 36 iterations, found
            // 2026-11-02).
            let slope = |e: &NetworkEnd| -> f64 {
                match e {
                    NetworkEnd::Volume { volume, .. } => {
                        let v = &vols_k[*volume];
                        let a = v.atmosphere();
                        let n = a.steam_kg / tampines_steam_tables::gas_mixture::M_H2O + a.ncg_total_moles();
                        let p = v.atmosphere_pressure().get::<pascal>();
                        let gamma = v
                            .gas_model()
                            .heat_capacity(&a, v.atmosphere_volume(), v.atmosphere_temperature())
                            .map_or(1.0, |c| 1.0 + n * R_GAS / c.get::<uom::si::heat_capacity::joule_per_kelvin>());
                        gamma * p / n
                    }
                    NetworkEnd::Boundary(_) => 0.0,
                }
            };
            let stiffness = |np: &NetworkPath| -> f64 { slope(&np.from) + slope(&np.to) };
            // This path's own flow as the latest iterate carries it, as a
            // forward pressure difference: the donor's loss (previous
            // iterate) and the receiver's gain (the iterate before), each at
            // its own slope.
            let own_dp = |np: &NetworkPath, n_donor: f64, n_receiver: f64| -> f64 { n_donor * slope(&np.from) + n_receiver * slope(&np.to) };
            if !lagged {
                // A breaker also opens on this iterate if the latest
                // difference with its own flow added back exceeds its set
                // point; once open it stays open for the step (hysteresis: a
                // decision changes at most once a step). Open is safe: the
                // limit below then gives zero flow below the set point.
                for (pi, np) in self.paths.iter().enumerate() {
                    if let Some(dp_open) = np.path.opening_pressure_difference() {
                        if !breaker_open[pi] && k > 1 {
                            let g = *np.path.geometry();
                            let a = Self::path_end(&latest, &np.from, g.from_elevation.get::<meter>());
                            let b = Self::path_end(&latest, &np.to, g.to_elevation.get::<meter>());
                            // gh:#1119: Newton adds its own flow back at its
                            // measured response, where it has one.
                            let own = match jac_d[pi].as_ref().filter(|_| newton).and_then(|d| limit_response(d, pi, g1[pi], g2[pi])) {
                                Some((_, own)) => own,
                                None => own_dp(np, prev_mol[pi], prev2_mol[pi]),
                            };
                            let d0 = a.pressure.get::<pascal>() - b.pressure.get::<pascal>() + own;
                            breaker_open[pi] = d0 > dp_open;
                        }
                        new_paths[pi].hold_open(Some(breaker_open[pi]));
                    }
                }
            }
            // gh:#1119: the Newton iterate (from the second iterate; the
            // first is Picard's, unrelaxed). The volumes are linearised by
            // their measured responses `D` about the latest iterate; the paths
            // are not: the flows `x = Φ(x)` are solved with the paths' own
            // steps at the linearised pressures (and the limit), so a path's
            // dead band (a downcomer below its submergence) and the limit's
            // kink are honoured exactly. Each path in turn (Gauss–Seidel),
            // each a bracketed scalar root (`Φᵢ` falls as `xᵢ` rises).
            let u_newton: Option<Vec<[f64; 2]>> = if newton && k > 1 {
                let p_raw: Vec<[f64; 2]> = self
                    .paths
                    .iter()
                    .map(|np| {
                        let g = np.path.geometry();
                        let pa = Self::path_end(&latest, &np.from, g.from_elevation.get::<meter>()).pressure.get::<pascal>();
                        let pb = Self::path_end(&latest, &np.to, g.to_elevation.get::<meter>()).pressure.get::<pascal>();
                        [pa, pb]
                    })
                    .collect();
                let pt = lag_corrected(&p_raw, &jac_d, &x1, &x2);
                let mut x = x1.clone();
                // Each path's flow to 1e-3 of the tolerance, relative to the
                // larger of its own and the latest largest rate.
                let x_scale = x1.iter().fold(1e-9_f64, |m, v| m.max(v.abs()));
                let rel = 1e-3 * self.control.tolerance;
                let eps = rel * x_scale;
                // The flow of path `i` at the flows `x`.
                let phi = |i: usize, x: &[f64]| -> Result<f64, TampinesError> {
                    let np = &self.paths[i];
                    let g = np.path.geometry();
                    let u = linear_pressures(&pt, &jac_d, x, &x1);
                    let mut a = Self::path_end(&latest, &np.from, g.from_elevation.get::<meter>());
                    let mut b = Self::path_end(&latest, &np.to, g.to_elevation.get::<meter>());
                    a.pressure = Pressure::new::<pascal>(u[i][0]);
                    b.pressure = Pressure::new::<pascal>(u[i][1]);
                    let t = new_paths[i].clone().step(dt, &a, &b)?;
                    let (ml, mut mg) = (t.liquid_mass_rate.get::<kilogram_per_second>(), t.gas_mass_rate.get::<kilogram_per_second>());
                    let has_volume = matches!(np.from, NetworkEnd::Volume { .. }) || matches!(np.to, NetworkEnd::Volume { .. });
                    if let Some(resp) = jac_d[i].as_ref().and_then(|d| limit_response(d, i, g1[i], g2[i])).filter(|_| has_volume && mg != 0.0) {
                        let cap = measured_cap(drive_k[i], resp, breaker_dp[i], mg);
                        if mg.abs() > cap {
                            mg = cap.copysign(mg);
                        }
                    }
                    Ok(ml + mg)
                };
                let responding: Vec<usize> = (0..n_paths).filter(|&i| jac_d[i].is_some()).collect();
                for _sweep in 0..20 {
                    let mut moved = 0.0_f64;
                    for &i in &responding {
                        let xi = monotone_root(x[i], eps, rel, |v| {
                            let mut xv = x.clone();
                            xv[i] = v;
                            phi(i, &xv)
                        })?;
                        moved = moved.max((xi - x[i]).abs());
                        x[i] = xi;
                    }
                    if moved <= eps || responding.len() == 1 {
                        break;
                    }
                }
                Some(linear_pressures(&pt, &jac_d, &x, &x1))
            } else {
                None
            };
            for (pi, np) in self.paths.iter().enumerate() {
                let g = *np.path.geometry();
                let mut a = Self::path_end(&latest, &np.from, g.from_elevation.get::<meter>());
                let mut b = Self::path_end(&latest, &np.to, g.to_elevation.get::<meter>());
                // Under-relaxed end pressures: the plain fixed point on the
                // pressures oscillated (found 2026-11-01: a stalled residual
                // of 1e-5 in a two-tank gas network at 0.1 s). `omega`
                // halves whenever the residual grows.
                if let Some(u) = &u_newton {
                    a.pressure = Pressure::new::<pascal>(u[pi][0]);
                    b.pressure = Pressure::new::<pascal>(u[pi][1]);
                } else if let Some((pa, pb)) = used_p[pi] {
                    a.pressure = Pressure::new::<pascal>(omega * a.pressure.get::<pascal>() + (1.0 - omega) * pa);
                    b.pressure = Pressure::new::<pascal>(omega * b.pressure.get::<pascal>() + (1.0 - omega) * pb);
                }
                used_p[pi] = Some((a.pressure.get::<pascal>(), b.pressure.get::<pascal>()));
                let mut t = new_paths[pi].step(dt, &a, &b)?;
                // The equalising limit (ported from sembawang's
                // `two_sided_equalising` downcomer cap,
                // `bwr_mark1::containment`): a stiff gas path never moves, in
                // one step, more than the moles that bring the two
                // atmospheres to one pressure (isothermal, both sides
                // counted; a boundary does not move). Without it the first
                // Picard pass at Δt = 1 s took half a tank's gas at the
                // start-of-step choked rate (found 2026-11-01).
                let mg = t.gas_mass_rate.get::<kilogram_per_second>();
                limited[pi] = false;
                if mg != 0.0 {
                    let (donor, receiver) = if mg > 0.0 { (&np.from, &np.to) } else { (&np.to, &np.from) };
                    if lagged {
                        if let Some((p_d, n_d, m_d)) = side(donor) {
                            let stiff = p_d / n_d + side(receiver).map_or(0.0, |(p, n, _)| p / n);
                            let dp_eq = if mg > 0.0 { drive[pi].0 - breaker_dp[pi] } else { drive[pi].1 };
                            let m_eq = if dp_eq > 0.0 { dp_eq / stiff * m_d } else { 0.0 };
                            if mg.abs() * dts > m_eq {
                                let f = m_eq / (mg.abs() * dts);
                                new_paths[pi].scale_gas_velocity(f);
                                t.gas_mass_rate = t.gas_mass_rate * f;
                                t.gas_enthalpy_rate = t.gas_enthalpy_rate * f;
                                limited[pi] = true;
                            }
                        }
                    } else {
                        // The limit as an end-of-step condition, linearised
                        // about the latest iterate: the moles that bring the
                        // driving difference, with this path's own previous
                        // flow added back, to the target (gh:#1116). A path
                        // fed by a boundary is limited by its receiver alone
                        // (a boundary does not move); without that, inflow
                        // from a boundary ran past the end-of-step balance
                        // and the limit on the return flow ping-ponged
                        // (sembawang's building openings, 2 → 36 iterations,
                        // found 2026-11-02).
                        let donor_molar = match donor {
                            NetworkEnd::Volume { .. } => side(donor).map(|(_, _, m)| m),
                            NetworkEnd::Boundary(b) if side(receiver).is_some() => Some(b.molar_mass()),
                            NetworkEnd::Boundary(_) => None,
                        };
                        let measured = jac_d[pi].as_ref().filter(|_| newton).and_then(|d| limit_response(d, pi, g1[pi], g2[pi]));
                        if let (Some(_), Some(resp)) = (donor_molar, measured) {
                            // gh:#1119: the same end-of-step condition with
                            // the path's measured response (condensing,
                            // bubbling, the level) for the slopes; the
                            // converged limit is the same, only the rate of
                            // getting there changes.
                            let cap = measured_cap(drive_k[pi], resp, breaker_dp[pi], mg);
                            if mg.abs() > cap {
                                let f = cap / mg.abs();
                                new_paths[pi].scale_gas_velocity(f);
                                t.gas_mass_rate = t.gas_mass_rate * f;
                                t.gas_enthalpy_rate = t.gas_enthalpy_rate * f;
                                limited[pi] = true;
                            }
                        } else if let Some(m_d) = donor_molar {
                            let stiff = stiffness(np);
                            let own = own_dp(np, prev_mol[pi], prev2_mol[pi]);
                            let dp_eq = if mg > 0.0 { drive_k[pi].0 + own - breaker_dp[pi] } else { drive_k[pi].1 - own };
                            let m_eq = if dp_eq > 0.0 { dp_eq / stiff * m_d } else { 0.0 };
                            if mg.abs() * dts > m_eq {
                                let f = m_eq / (mg.abs() * dts);
                                new_paths[pi].scale_gas_velocity(f);
                                t.gas_mass_rate = t.gas_mass_rate * f;
                                t.gas_enthalpy_rate = t.gas_enthalpy_rate * f;
                                limited[pi] = true;
                            }
                        }
                    }
                }
                // The moles moved this iterate, for the next one's limit.
                let mg = t.gas_mass_rate.get::<kilogram_per_second>();
                prev2_mol[pi] = prev_mol[pi];
                prev_mol[pi] = if mg == 0.0 {
                    0.0
                } else {
                    let donor = if mg > 0.0 { &np.from } else { &np.to };
                    let m_d = match donor {
                        NetworkEnd::Volume { .. } => side(donor).map_or(f64::NAN, |(_, _, m)| m),
                        NetworkEnd::Boundary(b) => b.molar_mass(),
                    };
                    mg * dts / m_d
                };
                transfers.push(t);
            }
            if newton {
                x2 = std::mem::replace(&mut x1, transfers.iter().map(signed_rate).collect());
                g2 = std::mem::replace(&mut g1, transfers.iter().map(|t| t.gas_mass_rate.get::<kilogram_per_second>()).collect());
            }
            // 2–3. Volumes from the start with these outflows and the
            // donors' previous sendings as inflow.
            new_vols = start_vols.clone();
            let mut inputs: Vec<ControlVolumeInputs> = new_vols
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    let mut c = ControlVolumeInputs::closed(v.ports().len());
                    c.pool_heat = uom::si::f64::Power::new::<uom::si::power::watt>(self.pool_heat_w[i]);
                    c.pool_heat_at.clone_from(&self.pool_heat_at[i]);
                    c.atmosphere_heat = uom::si::f64::Power::new::<uom::si::power::watt>(self.atm_heat_w[i]);
                    c.atmosphere_steam_sink = mr(self.atm_sink_kg_s[i]);
                    c.atmosphere_species = self.atm_species[i];
                    for (port, src) in self.sources[i].iter().enumerate() {
                        c.ports[port].inflow = *src;
                    }
                    c
                })
                .collect();
            let mut leftover: Vec<[InTransit; 2]> = vec![[InTransit::default(); 2]; self.paths.len()];
            // What each path end delivered this iterate (gh:#1119's response).
            let mut delivery: Vec<[(f64, [f64; 6], f64, f64); 2]> = vec![[(0.0, [0.0; 6], 0.0, 300.0); 2]; self.paths.len()];
            boundary_step = InTransit::default();
            for (pi, np) in self.paths.iter().enumerate() {
                let t = &transfers[pi];
                let (ml, mg) = (t.liquid_mass_rate.get::<kilogram_per_second>(), t.gas_mass_rate.get::<kilogram_per_second>());
                let fwd = ml.max(0.0) + mg.max(0.0);
                let bwd = (-ml).max(0.0) + (-mg).max(0.0);
                let (liq_fwd, liq_bwd) = (ml.max(0.0), (-ml).max(0.0));
                for (dir, (donor, receiver, rate)) in [(np.from, np.to, fwd), (np.to, np.from, bwd)].into_iter().enumerate() {
                    if let NetworkEnd::Volume { volume, port } = donor {
                        inputs[volume].ports[port].outflow = mr(rate);
                        // The path's own split of this direction's rate
                        // between the phases (gh:#1117), for the donor to
                        // draw; the ablation leaves the donor's point draw.
                        if self.two_phase_draw && rate > 0.0 {
                            let liq = if dir == 0 { liq_fwd } else { liq_bwd };
                            inputs[volume].ports[port].liquid_fraction = Some((liq / rate).clamp(0.0, 1.0));
                        }
                    }
                    // Delivery: last iterate's sending plus this path end's
                    // in-transit inventory from the previous step.
                    let (w, n, e, tg) = sent[pi][dir];
                    let tr = self.transit[pi][dir];
                    let (w, e) = (w + tr.water_kg, e + tr.energy_j);
                    let mut nn = n;
                    for q in 0..6 {
                        nn[q] += tr.ncg_kg[q];
                    }
                    delivery[pi][dir] = (w, nn, e, tg);
                    match receiver {
                        NetworkEnd::Volume { volume, port } => {
                            let (src, left) = Self::source_from(&new_vols[volume], w, &nn, e, tg, dts);
                            inputs[volume].ports[port].inflow = src;
                            leftover[pi][dir] = left;
                        }
                        NetworkEnd::Boundary(_) => {}
                    }
                    // A boundary donor sends its own gas at the path rate.
                    if let NetworkEnd::Boundary(b) = donor {
                        if rate > 0.0 {
                            if let NetworkEnd::Volume { volume, port } = receiver {
                                let m = rate * dts;
                                let tb = b.temperature.get::<kelvin>();
                                let mut gas = [0.0; 6];
                                for q in 0..6 {
                                    gas[q] = m * b.ncg_mass_fractions[q];
                                }
                                let model = new_vols[volume].gas_model();
                                let h: f64 = NonCondensable::ALL
                                    .iter()
                                    .map(|g| gas[g.index()] / g.molar_mass() * (model.ncg_u(*g, tb) + R_GAS * tb))
                                    .sum();
                                let src = inputs[volume].ports[port].inflow.with_gas(
                                    &NonCondensable::ALL.map(|g| (g, mr(gas[g.index()] / dts))),
                                    b.temperature,
                                );
                                inputs[volume].ports[port].inflow = src;
                                boundary_step.energy_j += h;
                                for q in 0..6 {
                                    boundary_step.ncg_kg[q] += gas[q];
                                }
                            }
                        }
                    }
                }
            }
            // Step every volume; record what each donor sent.
            let mut reports = Vec::with_capacity(new_vols.len());
            for (i, v) in new_vols.iter_mut().enumerate() {
                let r = v.step(dt, &inputs[i])?;
                reports.push(r);
            }
            // gh:#1119: each path that flows gets its response, re-taken when
            // the rates it was taken about have moved by more than 10 % (a
            // chord Newton otherwise).
            if newton {
                let scale = transfers.iter().map(|t| signed_rate(t).abs()).fold(1e-3, f64::max);
                for pi in 0..n_paths {
                    // `x1` is now this iterate's rate and `x2` the one the
                    // receiver was given.
                    let x = signed_rate(&transfers[pi]);
                    let stale = match &jac_d[pi] {
                        None => x != 0.0,
                        Some(d) => x != 0.0 && !d.holds_at(x, if d.receiver_lagged { x2[pi] } else { x }),
                    };
                    if stale {
                        jac_d[pi] = Some(self.path_response(pi, &transfers[pi], x2[pi], scale, &start_vols, &new_vols, &inputs, &reports, &delivery[pi], dt)?);
                    }
                }
            }
            // External sources, sinks and heats cross the boundary
            // (gh:#1097): what each volume reports it took in or gave up.
            for (i, r) in reports.iter().enumerate() {
                for (port, src) in self.sources[i].iter().enumerate() {
                    if *src != Source::none() {
                        let pt = &r.ports[port];
                        boundary_step.water_kg += pt.water_in.get::<kilogram>();
                        boundary_step.energy_j += pt.energy_in.get::<joule>();
                        for q in 0..6 {
                            boundary_step.ncg_kg[q] += pt.ncg_in[q].get::<kilogram>();
                        }
                    }
                }
                boundary_step.water_kg -= r.steam_sink.get::<kilogram>();
                // The species sources and sinks (gh:#1103).
                boundary_step.water_kg += r.species_water.get::<kilogram>();
                boundary_step.energy_j += r.species_energy.get::<joule>();
                for q in 0..6 {
                    boundary_step.ncg_kg[q] += r.species_ncg[q].get::<kilogram>();
                }
                boundary_step.energy_j += (self.pool_heat_w[i] + self.atm_heat_w[i]) * dts - r.steam_sink_energy.get::<joule>();
                for h in &self.pool_heat_at[i] {
                    boundary_step.energy_j += h.power.get::<uom::si::power::watt>() * dts;
                }
            }
            let mut new_sent = sent.clone();
            for (pi, np) in self.paths.iter().enumerate() {
                for (dir, donor) in [np.from, np.to].into_iter().enumerate() {
                    if let NetworkEnd::Volume { volume, port } = donor {
                        let pt = &reports[volume].ports[port];
                        let (_, gas_state) = latest[volume].phase_states_at(new_vols[volume].ports()[port].elevation);
                        new_sent[pi][dir] = (
                            pt.water_out.get::<kilogram>(),
                            pt.ncg_out.map(|m| m.get::<kilogram>()),
                            pt.energy_out.get::<joule>(),
                            gas_state.temperature.get::<kelvin>(),
                        );
                    }
                }
            }
            // Residual on the path rates.
            residual = 0.0;
            // Normalised by the network's largest rate (a path's own rate
            // goes to zero as its ends equalise, and a relative change of a
            // vanishing rate stalled the iteration: found 2026-11-01).
            let scale = transfers
                .iter()
                .map(|t| t.liquid_mass_rate.get::<kilogram_per_second>().abs() + t.gas_mass_rate.get::<kilogram_per_second>().abs())
                .fold(1e-12, f64::max);
            for (pi, t) in transfers.iter().enumerate() {
                let r = t.liquid_mass_rate.get::<kilogram_per_second>().abs() + t.gas_mass_rate.get::<kilogram_per_second>().abs();
                let d = if prev_rates[pi].is_nan() {
                    f64::INFINITY
                } else if newton && (r - prev_rates[pi]).abs() <= floor[pi] {
                    // gh:#1124: a change the end state cannot resolve (a
                    // round-off toggle) is converged; Newton only, so Picard
                    // stays bit for bit.
                    0.0
                } else {
                    (r - prev_rates[pi]).abs() / scale
                };
                residual = residual.max(d);
                prev_rates[pi] = r;
            }
            // In transit at the end of this iterate: sent now minus delivered.
            let mut transit = vec![[InTransit::default(); 2]; self.paths.len()];
            for (pi, np) in self.paths.iter().enumerate() {
                for (dir, (donor, receiver)) in [(np.from, np.to), (np.to, np.from)].into_iter().enumerate() {
                    let lo = leftover[pi][dir];
                    let delivered_w = sent[pi][dir].0 + self.transit[pi][dir].water_kg - lo.water_kg;
                    let delivered_e = sent[pi][dir].2 + self.transit[pi][dir].energy_j - lo.energy_j;
                    let mut delivered_n = sent[pi][dir].1;
                    for q in 0..6 {
                        delivered_n[q] += self.transit[pi][dir].ncg_kg[q] - lo.ncg_kg[q];
                    }
                    if matches!(donor, NetworkEnd::Volume { .. }) && matches!(receiver, NetworkEnd::Volume { .. }) {
                        // Conservation: in transit after = before + sent now
                        // − delivered now. (A first version omitted the
                        // "before" term, so the network lost the previous
                        // step's in-transit inventory, about the Picard
                        // residual per step: found 2026-11-01 when the
                        // tolerance was relaxed to 1e-6.)
                        let old = self.transit[pi][dir];
                        let mut tr = InTransit {
                            water_kg: old.water_kg + new_sent[pi][dir].0 - delivered_w,
                            energy_j: old.energy_j + new_sent[pi][dir].2 - delivered_e,
                            ..InTransit::default()
                        };
                        for q in 0..6 {
                            tr.ncg_kg[q] = old.ncg_kg[q] + new_sent[pi][dir].1[q] - delivered_n[q];
                        }
                        transit[pi][dir] = tr;
                    } else if matches!(donor, NetworkEnd::Volume { .. }) {
                        // Into a boundary: what the donor actually sent on
                        // this iterate leaves the network now (no lag).
                        boundary_step.water_kg -= new_sent[pi][dir].0;
                        boundary_step.energy_j -= new_sent[pi][dir].2;
                        for q in 0..6 {
                            boundary_step.ncg_kg[q] -= new_sent[pi][dir].1[q];
                        }
                    }
                }
            }
            sent = new_sent;
            latest = new_vols.clone();
            if k > 2 && residual > last_residual {
                omega = (0.5 * omega).max(0.05);
            }
            last_residual = residual;
            if k > 1 && residual < self.control.tolerance {
                self.commit(new_vols, new_paths, transit, boundary_step, dts);
                return Ok(NetworkStepReport { iterations, newton_iterations: if newton { iterations } else { 0 }, residual, transfers, volumes: reports, equalising_limited: limited, newton_fell_back: false });
            }
            if k == max_iterations {
                if self.control.max_iterations == 1 {
                    self.commit(new_vols, new_paths, transit, boundary_step, dts);
                    return Ok(NetworkStepReport { iterations, newton_iterations: 0, residual: 0.0, transfers, volumes: reports, equalising_limited: limited, newton_fell_back: false });
                }
                return Err(TampinesError::Numerical(format!("network step: no convergence in {iterations} iterations (residual {residual:e})")));
            }
        }
        Err(TampinesError::Numerical("network step: no iterations".into()))
    }

    /// Path `pi`'s [`PathResponse`] about this iterate (gh:#1119): its
    /// donor stepped again from the start state (`start`) with its outflow
    /// raised by `δ`, and its receiver with its inflow raised by what the
    /// donor then sent more, each against the iterate's own step (`stepped`
    /// from `inputs`, with `reports`). `t` is the path's transfer this
    /// iterate (its direction picks the donor; at zero, forward), `inflow`
    /// the rate the receiver was given this iterate, `scale` the
    /// network's largest rate (`δ` = 1e-6 of the larger of the two), and
    /// `delivery` what this iterate delivered per direction, `(water kg,
    /// gases kg, energy J, gas T)`.
    ///
    /// # Errors
    ///
    /// The volumes' errors.
    #[allow(clippy::too_many_arguments)]
    fn path_response(
        &self,
        pi: usize,
        t: &FlowTransfer,
        inflow: f64,
        scale: f64,
        start: &[ControlVolume],
        stepped: &[ControlVolume],
        inputs: &[ControlVolumeInputs],
        reports: &[ControlVolumeStepReport],
        delivery: &[(f64, [f64; 6], f64, f64); 2],
        dt: Time,
    ) -> Result<PathResponse, TampinesError> {
        let dts = dt.get::<second>();
        let np = &self.paths[pi];
        let x = signed_rate(t);
        let forward = x >= 0.0;
        let delta = 1e-6 * x.abs().max(scale);
        // A raised flow in the current direction is a change `s δ` of the
        // signed rate.
        let ds = if forward { delta } else { -delta };
        let (donor, receiver, dir) = if forward { (np.from, np.to, 0) } else { (np.to, np.from, 1) };
        let n = self.paths.len();
        let lagged = matches!(donor, NetworkEnd::Volume { .. });
        let mut out = PathResponse { donor: vec![[0.0; 2]; n], receiver: vec![[0.0; 2]; n], receiver_lagged: lagged, base: (x, if lagged { inflow } else { x }) };
        // What the donor sends more: water kg, gases kg, energy J.
        let (mut dw, mut dn, mut de) = (0.0, [0.0; 6], 0.0);
        match donor {
            NetworkEnd::Volume { volume, port } => {
                let mut inp = inputs[volume].clone();
                inp.ports[port].outflow += mr(delta);
                let mut v = start[volume].clone();
                let r = v.step(dt, &inp)?;
                let (base, more) = (&reports[volume].ports[port], &r.ports[port]);
                dw = (more.water_out - base.water_out).get::<kilogram>();
                de = (more.energy_out - base.energy_out).get::<joule>();
                for q in 0..6 {
                    dn[q] = (more.ncg_out[q] - base.ncg_out[q]).get::<kilogram>();
                }
                self.add_end_response(volume, &v, &stepped[volume], ds, &mut out.donor);
            }
            NetworkEnd::Boundary(b) => {
                for q in 0..6 {
                    dn[q] = delta * dts * b.ncg_mass_fractions[q];
                }
            }
        }
        if let NetworkEnd::Volume { volume, port } = receiver {
            let mut inp = inputs[volume].clone();
            inp.ports[port].inflow = match donor {
                NetworkEnd::Volume { .. } => {
                    let (w, nn, e, tg) = delivery[dir];
                    let mut n2 = nn;
                    for q in 0..6 {
                        n2[q] += dn[q];
                    }
                    Self::source_from(&start[volume], w + dw, &n2, e + de, tg, dts).0
                }
                NetworkEnd::Boundary(b) => inp.ports[port].inflow.with_gas(&NonCondensable::ALL.map(|g| (g, mr(dn[g.index()] / dts))), b.temperature),
            };
            let mut v = start[volume].clone();
            v.step(dt, &inp)?;
            self.add_end_response(volume, &v, &stepped[volume], ds, &mut out.receiver);
        }
        Ok(out)
    }

    /// Add to `out[j][e]` the change of every path end on volume `volume`
    /// from `base` to `moved`, per `dx` of flow (gh:#1119).
    fn add_end_response(&self, volume: usize, moved: &ControlVolume, base: &ControlVolume, dx: f64, out: &mut [[f64; 2]]) {
        for (np, oj) in self.paths.iter().zip(out.iter_mut()) {
            let g = np.path.geometry();
            for (e, (end, z)) in [(np.from, g.from_elevation), (np.to, g.to_elevation)].into_iter().enumerate() {
                if matches!(end, NetworkEnd::Volume { volume: v, .. } if v == volume) {
                    oj[e] += (moved.pressure_at_elevation(z) - base.pressure_at_elevation(z)).get::<pascal>() / dx;
                }
            }
        }
    }

    fn commit(&mut self, vols: Vec<ControlVolume>, paths: Vec<FlowPath>, transit: Vec<[InTransit; 2]>, b: InTransit, dt: f64) {
        self.volumes = vols;
        for (np, mut p) in self.paths.iter_mut().zip(paths) {
            p.hold_open(None);
            np.path = p;
        }
        self.transit = transit;
        self.boundary_in.water_kg += b.water_kg;
        self.boundary_in.energy_j += b.energy_j;
        for q in 0..6 {
            self.boundary_in.ncg_kg[q] += b.ncg_kg[q];
        }
        self.time_s += dt;
    }
}

#[cfg(test)]
mod tests;
