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
//! A step of `Δt` is a Picard (fixed-point) iteration on the end-of-step
//! state:
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
//! - The donor volume decides which phase leaves by its level at the port
//!   (a point draw); the path's partial-submergence split sets only the rate.
//! - Boundaries are fixed-state reservoirs ([`NetworkBoundary`]).
//! - A vacuum breaker is opened or shut for a whole step by the
//!   start-of-step pressures, and the equalising limit is taken from the
//!   start-of-step state (junction pressures less the gas column, gh:#1097):
//!   both lag by one step a source that arrives within it (measured in
//!   `cv_network.md` test 7 and in `sembawang`'s containment comparison).

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

/// Picard controls.
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
    /// Picard iterations taken.
    pub iterations: usize,
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
}

fn mr(x: f64) -> MassRate {
    MassRate::new::<kilogram_per_second>(x)
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
        }
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
        let dts = dt.get::<second>();
        let start_vols = self.volumes.clone();
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
        let drive: Vec<(f64, f64)> = self
            .paths
            .iter()
            .map(|np| {
                let g = *np.path.geometry();
                let (zf, zt) = (g.from_elevation.get::<meter>(), g.to_elevation.get::<meter>());
                let a = Self::path_end(&self.volumes, &np.from, zf);
                let b = Self::path_end(&self.volumes, &np.to, zt);
                let (pa, pb) = (a.pressure.get::<pascal>(), b.pressure.get::<pascal>());
                let (ra, rb) = (a.gas_density.get::<kilogram_per_cubic_meter>(), b.gas_density.get::<kilogram_per_cubic_meter>());
                (pa - pb - ra * G * (zt - zf), pb - pa - rb * G * (zf - zt))
            })
            .collect();
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
                    p.hold_open(Some(a.pressure.get::<pascal>() - b.pressure.get::<pascal>() > dp_open));
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
        let (mut omega, mut last_residual) = (1.0_f64, f64::INFINITY);
        let (mut residual, mut iterations): (f64, usize);
        let mut transfers: Vec<FlowTransfer> = Vec::new();
        let (mut new_vols, mut new_paths, mut boundary_step);
        for k in 1..=self.control.max_iterations {
            iterations = k;
            // 1. Paths from the latest iterate.
            new_paths = start_paths.clone();
            transfers.clear();
            for (pi, np) in self.paths.iter().enumerate() {
                let g = *np.path.geometry();
                let mut a = Self::path_end(&latest, &np.from, g.from_elevation.get::<meter>());
                let mut b = Self::path_end(&latest, &np.to, g.to_elevation.get::<meter>());
                // Under-relaxed end pressures: the plain fixed point on the
                // pressures oscillated (found 2026-11-01: a stalled residual
                // of 1e-5 in a two-tank gas network at 0.1 s). `omega`
                // halves whenever the residual grows.
                if let Some((pa, pb)) = used_p[pi] {
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
                if mg != 0.0 {
                    let (donor, receiver) = if mg > 0.0 { (&np.from, &np.to) } else { (&np.to, &np.from) };
                    let side = |e: &NetworkEnd| -> Option<(f64, f64, f64)> {
                        match e {
                            NetworkEnd::Volume { volume, .. } => {
                                let v = &start_vols[*volume];
                                let a = v.atmosphere();
                                let n = a.steam_kg / tampines_steam_tables::gas_mixture::M_H2O + a.ncg_total_moles();
                                Some((v.atmosphere_pressure().get::<pascal>(), n, a.total_mass().get::<kilogram>() / n))
                            }
                            NetworkEnd::Boundary(_) => None,
                        }
                    };
                    if let Some((p_d, n_d, m_d)) = side(donor) {
                        let stiff = p_d / n_d + side(receiver).map_or(0.0, |(p, n, _)| p / n);
                        let dp_eq = if mg > 0.0 { drive[pi].0 - breaker_dp[pi] } else { drive[pi].1 };
                        let m_eq = if dp_eq > 0.0 { dp_eq / stiff * m_d } else { 0.0 };
                        if mg.abs() * dts > m_eq {
                            let f = m_eq / (mg.abs() * dts);
                            new_paths[pi].scale_gas_velocity(f);
                            t.gas_mass_rate = t.gas_mass_rate * f;
                            t.gas_enthalpy_rate = t.gas_enthalpy_rate * f;
                        }
                    }
                }
                transfers.push(t);
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
            boundary_step = InTransit::default();
            for (pi, np) in self.paths.iter().enumerate() {
                let t = &transfers[pi];
                let (ml, mg) = (t.liquid_mass_rate.get::<kilogram_per_second>(), t.gas_mass_rate.get::<kilogram_per_second>());
                let fwd = ml.max(0.0) + mg.max(0.0);
                let bwd = (-ml).max(0.0) + (-mg).max(0.0);
                for (dir, (donor, receiver, rate)) in [(np.from, np.to, fwd), (np.to, np.from, bwd)].into_iter().enumerate() {
                    if let NetworkEnd::Volume { volume, port } = donor {
                        inputs[volume].ports[port].outflow = mr(rate);
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
                let d = if prev_rates[pi].is_nan() { f64::INFINITY } else { (r - prev_rates[pi]).abs() / scale };
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
                return Ok(NetworkStepReport { iterations, residual, transfers, volumes: reports });
            }
            if k == self.control.max_iterations {
                if self.control.max_iterations == 1 {
                    self.commit(new_vols, new_paths, transit, boundary_step, dts);
                    return Ok(NetworkStepReport { iterations, residual: 0.0, transfers, volumes: reports });
                }
                return Err(TampinesError::Numerical(format!("network step: no convergence in {iterations} iterations (residual {residual:e})")));
            }
        }
        Err(TampinesError::Numerical("network step: no iterations".into()))
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
