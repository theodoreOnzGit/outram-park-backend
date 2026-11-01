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

use outram_foam_basic_lib::fv_operators::fvc::central_upwind::{
    central_flux_speed_max, knp_face_flux, KnpFaceFlux, KnpFaceState,
};
use outram_foam_basic_lib::primitives::vector::Vector3;
use uom::si::area::square_meter;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::energy::joule;
use uom::si::f64::{
    AvailableEnergy, Energy, HeatFluxDensity, Mass, MassDensity, MassFlux, Pressure, Time, Volume,
};
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
}

/// Pipes and junctions advanced together with one time step.
#[derive(Debug, Clone)]
pub struct HemNetwork<E: HemEquationOfState> {
    eos: E,
    pipes: Vec<HemPipe<E>>,
    junctions: Vec<HemJunction>,
    courant: f64,
    time_s: f64,
}

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
            HemJunction::Flow { a, b, .. } => *a == end || *b == end,
            HemJunction::Volume(v) => v.connections.iter().any(|(e, _)| *e == end),
        });
        if taken {
            return Err(HemError::InvalidInput(format!(
                "{end:?} already belongs to a junction"
            )));
        }
        Ok(())
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
        self.junctions.push(HemJunction::Volume(VolumeBranch {
            volume_m3: v,
            mass_kg,
            energy_j,
            connections,
        }));
        Ok(self.junctions.len() - 1)
    }

    /// The pipes, by index.
    pub fn pipe(&self, index: usize) -> Option<&HemPipe<E>> {
        self.pipes.get(index)
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
                HemJunction::Flow { .. } => 0.0,
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
                HemJunction::Flow { .. } => 0.0,
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
        let mut volume_rates: Vec<(usize, f64, f64)> = Vec::new();
        let nx = Vector3::new(1.0, 0.0, 0.0);
        for (ji, j) in self.junctions.iter().enumerate() {
            match j {
                HemJunction::Flow { a, b, form_loss } => {
                    let (ca, area_a) = self.end_cell(*a)?;
                    let (cb, area_b) = self.end_cell(*b)?;
                    // In a's outward frame, b's velocity is along −(b's outward normal).
                    let pa = Self::prim(&ca, outward(a.end));
                    let pb = Self::prim(&cb, -outward(b.end));
                    // Upstream by the mean normal velocity; the loss from its cell.
                    let a_is_up = pa.1 + pb.1 >= 0.0;
                    let up = if a_is_up { pa } else { pb };
                    let loss = form_loss * 0.5 * up.0 * up.1 * up.1;
                    let (dpa, dpb) = if a_is_up { (0.0, loss) } else { (loss, 0.0) };
                    let sa = raised(pa.0, pa.1, pa.2, pa.3, pa.4, dpa);
                    let sb = raised(pb.0, pb.1, pb.2, pb.3, pb.4, dpb);
                    let f: KnpFaceFlux = knp_face_flux(&sa, &sb, nx);
                    let amax = central_flux_speed_max(&sa, &sb, nx);
                    let aj = area_a.min(area_b);
                    let mdot = f.mass * aj;
                    let edot = f.energy * aj;
                    let rho_up = up.0;
                    let (mom_a, mom_b) = if area_a == area_b {
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
                    // a: flux out of a; b: the same rate into b, so out of b is negative.
                    boundaries.push((
                        *a,
                        to_pipe_frame(a.end, mdot / area_a, mom_a, edot / area_a),
                    ));
                    boundaries.push((
                        *b,
                        to_pipe_frame(b.end, -mdot / area_b, mom_b, -edot / area_b),
                    ));
                    rate = rate.max(
                        amax / self.pipes[a.pipe]
                            .geometry()
                            .cell_width()
                            .get::<uom::si::length::meter>(),
                    );
                }
                HemJunction::Volume(v) => {
                    let st = self.close_volume(v)?;
                    let rho_v = st.density.get::<kilogram_per_cubic_meter>();
                    let (mut mdot_in, mut edot_in, mut vol_rate) = (0.0, 0.0, 0.0);
                    for (end, k) in &v.connections {
                        let (c, area) = self.end_cell(*end)?;
                        let pc = Self::prim(&c, outward(end.end));
                        // Upstream by the end cell's normal velocity.
                        let pipe_up = pc.1 >= 0.0;
                        let rho_up = if pipe_up { pc.0 } else { rho_v };
                        let loss = k * 0.5 * rho_up * pc.1 * pc.1;
                        let sc = raised(
                            pc.0,
                            pc.1,
                            pc.2,
                            pc.3,
                            pc.4,
                            if pipe_up { 0.0 } else { loss },
                        );
                        let sv_k = raised(
                            rho_v,
                            0.0,
                            st.pressure.get::<pascal>(),
                            st.specific_internal_energy.get::<joule_per_kilogram>(),
                            st.equilibrium_sound_speed
                                .get::<uom::si::velocity::meter_per_second>(),
                            if pipe_up { loss } else { 0.0 },
                        );
                        let f = knp_face_flux(&sc, &sv_k, nx);
                        let amax = central_flux_speed_max(&sc, &sv_k, nx);
                        // The pipe is downstream when the volume feeds it.
                        let mom = if pipe_up {
                            f.momentum.x
                        } else {
                            f.momentum.x - loss
                        };
                        boundaries.push((*end, to_pipe_frame(end.end, f.mass, mom, f.energy)));
                        mdot_in += f.mass * area;
                        edot_in += f.energy * area;
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
        // 3. Apply the junction fluxes and advance every pipe.
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
