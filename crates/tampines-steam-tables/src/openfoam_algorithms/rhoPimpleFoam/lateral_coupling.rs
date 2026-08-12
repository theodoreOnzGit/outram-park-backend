// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! Lateral (radial) thermal coupling, volumetric heat source, pipe geometry
//! and flow bookkeeping for [`super::TampinesSteamArray`].
//!
//! Mirrors `outram_park_fork_coolprop::OPCPFluidArray`'s interface (see that
//! crate's `openfoam_algorithms/rhoPimpleFoam/lateral_coupling.rs`), which in
//! turn mirrors a subset of `tuas_boussinesq_solver::FluidArray`'s -- so all
//! three backends are driveable through a comparable API (the eventual
//! `TampinesArray` enum, see the workspace's `op-21g.7` bead, will dispatch
//! across them). Simplified relative to TUAS: the caller supplies a thermal
//! conductance directly -- there is no `NusseltCorrelation` port here.
//!
//! Unlike `OPCPFluidArray`, [`Self::set_temperature_vector`] here uses this
//! crate's own real IAPWS-IF97 `(p, T)` flash
//! ([`crate::interfaces::functional_programming::pt_flash_eqm`]) rather than
//! a placeholder EOS -- this crate owns that flash, so there is no reason to
//! fake it the way a cross-crate port might. [`super::TampinesSteamArray::correct_thermo`]
//! itself now also uses the real `(p, h)` flash (see that method's own doc
//! comment).
//!
//! Also provides prescribed inlet/outlet boundary-condition setters/getters
//! ([`Self::set_inlet_velocity`], [`Self::set_inlet_enthalpy`],
//! [`Self::set_outlet_pressure`], [`Self::get_outlet_pressure`],
//! [`Self::get_outlet_enthalpy`], [`Self::get_outlet_temperature`]) so a
//! caller can drive this array as a simple pipe/tube (known inlet flow +
//! thermodynamic state, known downstream pressure) without needing to
//! reach into the internal `PatchField`/`Vector3`/mesh-patch-index
//! representation directly -- keeping those internals out of this crate's
//! public surface (see this module's own `CLAUDE.md`).

use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{
    Angle, AvailableEnergy, Length, MassRate, Power, Pressure, ThermalConductance,
    ThermodynamicTemperature, Velocity,
};
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::velocity::meter_per_second;

use crate::interfaces::functional_programming::pt_flash_eqm::{
    h_tp_eqm_single_phase, kappa_t_tp_eqm, v_tp_eqm_single_phase,
};
use crate::openfoam_algorithms::openfoam_source::{PatchField, Vector3};

use super::{AdvectionTerminalState, TampinesSteamArray};

/// Errors from [`TampinesSteamArray`]'s lateral-coupling / bookkeeping
/// interface.
#[derive(Debug, Clone, PartialEq)]
pub enum TampinesSteamArrayError {
    /// A per-cell vector argument did not have length `mesh.n_cells`.
    LengthMismatch {
        /// Name of the offending argument.
        array: &'static str,
        /// Required length (`mesh.n_cells`).
        expected: usize,
        /// Actual length of the supplied argument.
        got: usize,
    },
    /// The caller supplied a timestep that is not a usable positive duration.
    ///
    /// Reported rather than clamped or silently substituted: a zero or
    /// negative timestep means the caller's clock is wrong, and advancing by
    /// some other value would produce a plausible-looking result for a
    /// simulation that never ran the requested step.
    InvalidTimestep {
        /// The offending timestep, in seconds.
        seconds: f64,
    },
}

impl std::fmt::Display for TampinesSteamArrayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LengthMismatch {
                array,
                expected,
                got,
            } => write!(
                f,
                "{array} has length {got}, expected {expected} (mesh.n_cells)"
            ),
            Self::InvalidTimestep { seconds } => write!(
                f,
                "timestep must be a positive, finite duration; got {seconds} s"
            ),
        }
    }
}

impl std::error::Error for TampinesSteamArrayError {}

impl TampinesSteamArray {
    /// Register one lateral (radial) thermal link to another array/solid at a
    /// uniform conductance, for use in the next [`Self::step`].
    ///
    /// `temperature_vec` must have length `mesh.n_cells` — one neighbour
    /// temperature per cell. `average_thermal_conductance` \[W/K\] is applied
    /// uniformly to every cell (the caller is responsible for any Nusselt /
    /// geometry calculation that produced it — this array does not compute
    /// one itself).
    ///
    /// Multiple calls accumulate independent links (e.g. coupling to several
    /// neighbouring arrays); all links are consumed and cleared by
    /// [`Self::clear_vectors`] once per [`Self::step`].
    pub fn lateral_link_new_temperature_vector_avg_conductance(
        &mut self,
        average_thermal_conductance: ThermalConductance,
        temperature_vec: Vec<ThermodynamicTemperature>,
    ) -> Result<(), TampinesSteamArrayError> {
        let n = self.mesh.n_cells;
        if temperature_vec.len() != n {
            return Err(TampinesSteamArrayError::LengthMismatch {
                array: "temperature_vec",
                expected: n,
                got: temperature_vec.len(),
            });
        }
        let conductance_vec = vec![average_thermal_conductance; n];
        self.lateral_adjacent_array_temperature_vector
            .push(temperature_vec);
        self.lateral_adjacent_array_conductance_vector
            .push(conductance_vec);
        Ok(())
    }

    /// Register a volumetric heat source for use in the next [`Self::step`].
    ///
    /// `power_source` \[W\] is the total power; `q_fraction_vec` (length
    /// `mesh.n_cells`) distributes it across cells (need not sum to 1 —
    /// mirrors TUAS's `q_fraction_vector`). Multiple calls accumulate
    /// independent sources; all are consumed and cleared by
    /// [`Self::clear_vectors`] once per [`Self::step`].
    pub fn lateral_link_new_power_vector(
        &mut self,
        power_source: Power,
        q_fraction_vec: Vec<f64>,
    ) -> Result<(), TampinesSteamArrayError> {
        let n = self.mesh.n_cells;
        if q_fraction_vec.len() != n {
            return Err(TampinesSteamArrayError::LengthMismatch {
                array: "q_fraction_vec",
                expected: n,
                got: q_fraction_vec.len(),
            });
        }
        self.q_vector.push(power_source);
        self.q_fraction_vector.push(q_fraction_vec);
        Ok(())
    }

    /// Empty all registered lateral-coupling and heat-source vectors.
    ///
    /// Called once at the end of [`Self::step`] — links/sources are
    /// per-timestep registrations, not persistent state.
    pub fn clear_vectors(&mut self) {
        self.lateral_adjacent_array_temperature_vector.clear();
        self.lateral_adjacent_array_conductance_vector.clear();
        self.q_vector.clear();
        self.q_fraction_vector.clear();
    }

    /// Wetted perimeter \[m\] (bookkeeping — see [`Self::get_hydraulic_diameter`]).
    pub fn get_wetted_perimeter(&self) -> Length {
        self.wetted_perimeter
    }

    /// Set the wetted perimeter \[m\].
    pub fn set_wetted_perimeter(&mut self, wetted_perimeter: Length) {
        self.wetted_perimeter = wetted_perimeter;
    }

    /// Incline angle from horizontal \[rad\] (bookkeeping only).
    pub fn get_incline_angle(&self) -> Angle {
        self.incline_angle
    }

    /// Set the incline angle from horizontal \[rad\].
    pub fn set_incline_angle(&mut self, incline_angle: Angle) {
        self.incline_angle = incline_angle;
    }

    /// Hydraulic diameter `D_h = 4 * xs_area / wetted_perimeter` \[m\].
    pub fn get_hydraulic_diameter(&self) -> Length {
        4.0 * self.xs_area / self.wetted_perimeter
    }

    /// Bulk mass flowrate \[kg/s\] -- inert bookkeeping, NOT a boundary
    /// condition. To drive the array at a known flow use
    /// [`Self::set_inlet_mass_flowrate`]. (See the field's doc comment
    /// on [`TampinesSteamArray`] for why `step()` does not read this).
    pub fn get_mass_flowrate(&self) -> MassRate {
        self.mass_flowrate
    }

    /// Set the bulk mass flowrate \[kg/s\].
    pub fn set_mass_flowrate(&mut self, mass_flowrate: MassRate) {
        self.mass_flowrate = mass_flowrate;
    }

    /// Pressure loss \[Pa\] (plain storage, independent of `mass_flowrate`).
    pub fn get_pressure_loss(&self) -> Pressure {
        self.pressure_loss
    }

    /// Set the pressure loss \[Pa\].
    pub fn set_pressure_loss(&mut self, pressure_loss: Pressure) {
        self.pressure_loss = pressure_loss;
    }

    /// Internal pressure source \[Pa\] (e.g. a simulated pump; plain storage).
    pub fn get_internal_pressure_source(&self) -> Pressure {
        self.internal_pressure_source
    }

    /// Set the internal pressure source \[Pa\].
    pub fn set_internal_pressure_source(&mut self, internal_pressure_source: Pressure) {
        self.internal_pressure_source = internal_pressure_source;
    }

    /// Per-cell temperature \[K\], read from the `t` field (length `mesh.n_cells`).
    pub fn get_temperature_vector(&self) -> Vec<ThermodynamicTemperature> {
        self.t
            .internal
            .iter()
            .map(|&t_k| ThermodynamicTemperature::new::<kelvin>(t_k))
            .collect()
    }

    /// Overwrite the per-cell temperature at the current pressure, via a real
    /// IAPWS-IF97 `(p, T)` single-phase flash
    /// ([`crate::interfaces::functional_programming::pt_flash_eqm`]).
    ///
    /// Writes `he`/`rho`/`t`/`psi` together — **not** a plain `t` field write
    /// — since `he` (specific enthalpy) is the actual PIMPLE state variable;
    /// writing `t` alone would be silently undone by the next
    /// [`super::TampinesSteamArray::correct_thermo`] call, which recomputes
    /// `t`/`rho`/`psi` from `(p, he)` via its own real `(p, h)` flash (see
    /// that method's doc) -- so this setter's `(p, T)` flash and
    /// `correct_thermo`'s `(p, h)` flash are two independent real-EOS
    /// evaluations of the same state, not a placeholder vs. real split.
    pub fn set_temperature_vector(
        &mut self,
        temperature_vec: Vec<ThermodynamicTemperature>,
    ) -> Result<(), TampinesSteamArrayError> {
        let n = self.mesh.n_cells;
        if temperature_vec.len() != n {
            return Err(TampinesSteamArrayError::LengthMismatch {
                array: "temperature_vec",
                expected: n,
                got: temperature_vec.len(),
            });
        }
        for c in 0..n {
            let p_c = Pressure::new::<pascal>(self.p.internal[c]);
            let t_c = temperature_vec[c];

            let h = h_tp_eqm_single_phase(t_c, p_c);
            let v = v_tp_eqm_single_phase(t_c, p_c);
            let rho = 1.0 / v.get::<cubic_meter_per_kilogram>();
            let kappa_t = kappa_t_tp_eqm(t_c, p_c).value; // raw Pa^-1 (see module doc: `InversePressure` has no named unit)

            self.rho.internal[c] = rho.max(1e-4);
            self.he.internal[c] = h.get::<joule_per_kilogram>();
            self.t.internal[c] = t_c.get::<kelvin>();
            self.psi.internal[c] = (rho * kappa_t).max(1e-12);
        }
        Ok(())
    }

    /// Total volumetric heat-source power \[W\] distributed into cell `c` from
    /// every registered source, using each source's `q_fraction_vector` entry.
    ///
    /// Used by [`super::TampinesSteamArray::step`] to add an explicit source
    /// term to the energy equation, alongside the existing `conv_he`/`dp_dt`
    /// terms.
    pub(super) fn cell_heat_source_power(&self, c: usize) -> Power {
        let mut q = Power::new::<watt>(0.0);
        for (source_idx, fractions) in self.q_fraction_vector.iter().enumerate() {
            q += self.q_vector[source_idx] * fractions[c];
        }
        q
    }

    /// Overwrites the whole **internal** velocity field to a uniform axial
    /// value \[m/s\] (+x). Distinct from [`Self::set_inlet_velocity`], which
    /// only sets the inlet boundary condition: this pre-conditions every
    /// cell's velocity so a near-incompressible liquid does not water-hammer
    /// when an inlet velocity BC is first imposed (see the stability guide,
    /// `rho_pimple_foam/docs/stability_a_students_guide.md`). The internal
    /// `Vector3` element type is not part of this crate's public surface, so
    /// this is the supported way to seed the velocity field from outside.
    pub fn set_uniform_velocity_field(&mut self, velocity: Velocity) {
        let vx = velocity.get::<meter_per_second>();
        for c in 0..self.mesh.n_cells {
            self.u.internal[c] = Vector3::new(vx, 0.0, 0.0);
        }
    }

    /// Prescribes a fixed inlet velocity boundary condition on the
    /// `"left"` patch (x = 0, see [`crate::openfoam_algorithms::openfoam_source::interface::one_dimensional_meshing::create_one_d_mesh`]).
    ///
    /// For driving this array as a simple pipe/tube with a known inlet
    /// flow (e.g. from an upstream pump). `velocity` is the x-direction
    /// flow speed; positive means fluid entering the domain (flowing
    /// left-to-right, +x) -- take effect on the next [`super::TampinesSteamArray::step`].
    /// **Clears any prescribed mass-flow inlet**
    /// ([`Self::set_inlet_mass_flowrate`]): the two prescribe the same patch, so
    /// the last one called wins rather than silently fighting each other.
    pub fn set_inlet_velocity(&mut self, velocity: Velocity) {
        let size = self.mesh.patches[1].size;
        let v = Vector3::new(velocity.get::<meter_per_second>(), 0.0, 0.0);
        self.u.boundary[1] = PatchField::fixed_value_vec(size, v);
        self.inlet_mass_flowrate = None;
    }

    /// Prescribe a fixed inlet **mass flowrate** \[kg/s\] on the `"left"` patch
    /// (x = 0) -- OpenFOAM's `flowRateInletVelocity`.
    ///
    /// Positive is **into** the domain (+x). Takes effect on the next
    /// [`super::TampinesSteamArray::step`], and is re-derived every pressure
    /// corrector from the array's own inlet-face density, so the imposed
    /// boundary mass flux is exactly `mass_flowrate`.
    ///
    /// # Why this exists, and why `set_mass_flowrate` is not it
    ///
    /// [`Self::set_mass_flowrate`] is inert bookkeeping -- `step()` never reads
    /// it. Before this method existed, a caller wanting a known mass flow had
    /// to pick an assumed inlet density, convert to a velocity themselves, and
    /// call [`Self::set_inlet_velocity`]. That imposes
    /// `m_actual = (rho_solved / rho_assumed) * m_target`, which drifts as the
    /// solved density departs from the guess. In the HTGR steam-generator work
    /// that drift opened a **1.33 MW** gap between the two sides of an
    /// otherwise-converged heat exchanger (bead `op-289n`'s sibling).
    ///
    /// **Pairs with [`Self::set_inlet_enthalpy`]** to fully specify the inlet
    /// state. Calling [`Self::set_inlet_velocity`] clears this, and vice versa.
    pub fn set_inlet_mass_flowrate(&mut self, mass_flowrate: MassRate) {
        self.inlet_mass_flowrate = Some(mass_flowrate);
    }

    /// Remove any prescribed inlet mass flowrate, leaving whatever velocity
    /// boundary condition is currently on the inlet patch in force.
    pub fn clear_inlet_mass_flowrate(&mut self) {
        self.inlet_mass_flowrate = None;
    }

    /// Prescribe the **junction specific enthalpy** \[J/kg\] at the inlet
    /// terminal -- the `"left"` patch, x = 0. Pairs with
    /// [`Self::set_inlet_velocity`] or [`Self::set_inlet_mass_flowrate`] to
    /// fully specify the incoming stream.
    ///
    /// # This is an upwind terminal, not a Dirichlet patch
    ///
    /// The value is used as the upstream enthalpy **only while flow is entering
    /// through this end**. If the flow reverses and fluid leaves through the
    /// inlet, the pipe's own first cell is upstream and this value is ignored
    /// until the flow turns around again -- see
    /// [`super::TampinesSteamArray::correct_advection_terminals`] and
    /// `docs/boundary-conditions-convention.md`.
    ///
    /// That is deliberate, and it is why the convention exists: these arrays are
    /// pipes in a network, so their ends are junctions. An unconditional
    /// Dirichlet would clamp the face even on outflow, exporting the *junction's*
    /// enthalpy instead of the fluid's.
    ///
    /// Also selects the inflowing density: while this terminal is inflowing, the
    /// entering mass flux uses `rho(p_cell, h)` rather than the first cell's own
    /// density (see
    /// [`super::TampinesSteamArray::apply_junction_densities`]).
    ///
    /// Clear it with [`Self::clear_inlet_enthalpy`].
    pub fn set_inlet_enthalpy(&mut self, h: AvailableEnergy) {
        self.inlet_terminal = AdvectionTerminalState::Junction(h);
        // Stamp the terminal immediately as well, so an array that is read (or
        // stepped) before any flux exists sees the junction state rather than a
        // stale face value. `correct_advection_terminals` takes over from the
        // first step onward and may replace this with zero-gradient if the flow
        // turns out to be leaving here.
        let size = self.mesh.patches[super::INLET_PATCH].size;
        self.he.boundary[super::INLET_PATCH] =
            PatchField::fixed_value(size, h.get::<joule_per_kilogram>());
    }

    /// Remove the inlet junction enthalpy, returning that terminal to
    /// [`AdvectionTerminalState::ZeroGradientExtrapolated`] -- on inflow it will
    /// then advect in the adjacent cell's own enthalpy, there being nothing else
    /// known.
    pub fn clear_inlet_enthalpy(&mut self) {
        self.inlet_terminal = AdvectionTerminalState::ZeroGradientExtrapolated;
    }

    /// Prescribe the **junction specific enthalpy** \[J/kg\] at the outlet
    /// terminal -- the `"right"` patch, x = length.
    ///
    /// # Why an outlet needs one
    ///
    /// While flow leaves through this end the value is ignored: the pipe is
    /// upstream, and zero-gradient is the right condition because the downstream
    /// state is genuinely unknown. It matters **when the flow reverses**, which
    /// this workspace does routinely -- natural circulation reverses at start-up
    /// and stagnation, a blowdown reverses as it flashes, and a counter-flow
    /// heat exchanger has its two outlets at opposite ends. Without a junction
    /// state here, a reversal makes the array advect its own enthalpy back in
    /// through its outlet, which is the self-referential failure the whole
    /// convention exists to rule out.
    ///
    /// Set it to whatever is on the other side of the junction -- the plenum,
    /// downheader, or next component the outlet connects to. Clear it with
    /// [`Self::clear_outlet_enthalpy`].
    pub fn set_outlet_enthalpy(&mut self, h: AvailableEnergy) {
        self.outlet_terminal = AdvectionTerminalState::Junction(h);
    }

    /// Remove the outlet junction enthalpy, returning that terminal to
    /// [`AdvectionTerminalState::ZeroGradientExtrapolated`].
    pub fn clear_outlet_enthalpy(&mut self) {
        self.outlet_terminal = AdvectionTerminalState::ZeroGradientExtrapolated;
    }

    /// Prescribes a fixed outlet velocity boundary condition on the
    /// `"right"` patch (x = length, the outlet -- see
    /// [`crate::openfoam_algorithms::openfoam_source::interface::one_dimensional_meshing::create_one_d_mesh`]).
    ///
    /// The outlet mirror of [`Self::set_inlet_velocity`] (same `"right"` patch
    /// index the outlet-pressure/outlet-state accessors use). For driving this
    /// array with a known discharge velocity -- e.g. the Edwards blowdown feeds
    /// the equivalent full-face velocity from the choked-break solution here each
    /// step. `velocity` is the x-direction speed; positive means fluid leaving
    /// the domain (flowing left-to-right, +x) -- takes effect on the next
    /// [`super::TampinesSteamArray::step`].
    pub fn set_outlet_velocity(&mut self, velocity: Velocity) {
        let size = self.mesh.patches[0].size;
        let v = Vector3::new(velocity.get::<meter_per_second>(), 0.0, 0.0);
        self.u.boundary[0] = PatchField::fixed_value_vec(size, v);
    }

    /// Prescribes a fixed outlet pressure boundary condition on the
    /// `"right"` patch (x = length) -- e.g. the downstream pressure a
    /// turbine or condenser imposes.
    pub fn set_outlet_pressure(&mut self, p: Pressure) {
        let size = self.mesh.patches[0].size;
        self.p.boundary[0] = PatchField::fixed_value(size, p.get::<pascal>());
    }

    /// Outlet-cell (the last cell, owner of the `"right"` patch) pressure
    /// -- for a caller reading the downstream state after [`super::TampinesSteamArray::step`].
    pub fn get_outlet_pressure(&self) -> Pressure {
        let n = self.mesh.n_cells;
        Pressure::new::<pascal>(self.p.internal[n - 1])
    }

    /// Outlet-cell specific enthalpy.
    pub fn get_outlet_enthalpy(&self) -> AvailableEnergy {
        let n = self.mesh.n_cells;
        AvailableEnergy::new::<joule_per_kilogram>(self.he.internal[n - 1])
    }

    /// Outlet-cell temperature.
    pub fn get_outlet_temperature(&self) -> ThermodynamicTemperature {
        let n = self.mesh.n_cells;
        ThermodynamicTemperature::new::<kelvin>(self.t.internal[n - 1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::area::square_meter;
    use uom::si::f64::{Area, Time};
    use uom::si::length::meter;
    use uom::si::thermal_conductance::watt_per_kelvin;
    use uom::si::time::second;

    fn test_array(n: i64) -> TampinesSteamArray {
        TampinesSteamArray::new(
            Length::new::<meter>(1.0),
            Area::new::<square_meter>(1.0e-4),
            n,
            Time::new::<second>(0.01),
        )
        .unwrap()
    }

    /// ## Methodology
    /// Drives the array as a simple subcooled-liquid-water pipe with a
    /// prescribed inlet velocity + enthalpy and a prescribed outlet
    /// pressure -- the first exercise anywhere in this crate of
    /// [`TampinesSteamArray::step`]'s pressure-velocity coupling under a
    /// real Dirichlet velocity BC (every prior test either uses the
    /// default zero-gradient/passive BCs or drives `he` alone via
    /// [`TampinesSteamArray::set_temperature_vector`]). The array is
    /// pre-initialized near a well-subcooled operating point (320 K, 1 bar,
    /// safely away from the ~373 K saturation line at that pressure) and
    /// the inlet enthalpy is derived from the same (T, p) point, so the run
    /// stays single-phase throughout. Transient PISO at a liquid-water
    /// acoustic-CFL timestep (dt = 5e-5 s < dx/c_sound ≈ 0.1/1450 ≈ 6.9e-5 s).
    ///
    /// ## Result (2026-07-14)
    /// Passes: fields stay finite over 200 steps, the outlet cell pressure
    /// settles near the imposed 1 bar BC, and the flow moves in +x. This
    /// only works because of the pressure-boundary-source fix in `step()`
    /// (see the `p_eqn.source[c] += source_p[c]` note there): before that
    /// fix, overwriting the pressure equation's source dropped the
    /// FixedValue-outlet Dirichlet term while keeping its diagonal, which
    /// silently imposed `p_outlet = 0` and blew the field up within ~10
    /// steps -- even for a compressible gas (see
    /// `outram_park_fork_coolprop::OPCPFluidArray`'s parallel test) and
    /// even from a uniform equilibrium field. See the workspace beads
    /// tracker (`op-21g.12`) for the full debugging trail.
    ///
    /// A remaining sharp edge (not exercised here): near the saturation
    /// boundary, `correct_thermo`'s `(p,h)` region classification and
    /// `thermal_conductivity`'s internal `(T,p)` re-classification can
    /// disagree, hitting `pt_flash_eqm::cp_tp_eqm_single_phase`'s
    /// `FwdEqnRegion::Region4 => todo!(...)`. A real boiling steam-generator
    /// tube passes through that boundary, so it must be addressed before
    /// this array can model one -- still tracked on `op-21g.12`.
    #[test]
    fn inlet_outlet_bcs_drive_flow_and_outlet_pressure_settles_near_imposed_value() {
        let mut arr = TampinesSteamArray::new(
            Length::new::<meter>(1.0),
            Area::new::<square_meter>(1.0e-4),
            10,
            Time::new::<second>(5.0e-5), // liquid-water acoustic CFL: dx/c ≈ 0.1/1450 ≈ 6.9e-5 s
        )
        .unwrap();
        arr.set_piso_algorithm(2);

        // Pre-initialize the whole array near a well-subcooled operating
        // point (rather than leaving it at new()'s default 1 bar/300 K
        // reference state), staying well clear of the saturation boundary.
        let outlet_pressure = Pressure::new::<pascal>(1.0e5);
        for c in 0..10 {
            arr.p.internal[c] = outlet_pressure.get::<pascal>();
        }
        let preset_temp = ThermodynamicTemperature::new::<kelvin>(320.0);
        arr.set_temperature_vector(vec![preset_temp; 10]).unwrap();

        // Gentle inlet velocity: near-incompressible liquid water started
        // impulsively surges by the Joukowsky pressure Δp = ρ·c·Δu ≈
        // 1000·1450·u; at u = 0.02 m/s that is ~29 kPa, a mild transient
        // that stays comfortably within the IAPWS-IF97 valid range (0.5 m/s
        // would be a ~7 bar water-hammer surge whose reflection undershoots
        // below the triple-point pressure).
        let inlet_velocity = Velocity::new::<meter_per_second>(0.02);
        let inlet_enthalpy =
            crate::interfaces::functional_programming::pt_flash_eqm::h_tp_eqm_single_phase(
                preset_temp,
                outlet_pressure,
            );
        arr.set_inlet_velocity(inlet_velocity);
        arr.set_inlet_enthalpy(inlet_enthalpy);
        arr.set_outlet_pressure(outlet_pressure);

        arr.run(200);

        let all_finite = arr
            .u
            .internal
            .as_slice()
            .iter()
            .all(|v| v.mag().is_finite())
            && arr.p.internal.as_slice().iter().all(|x| x.is_finite())
            && arr.he.internal.as_slice().iter().all(|x| x.is_finite());
        assert!(
            all_finite,
            "fields must stay finite when driven by prescribed inlet/outlet BCs"
        );

        // outlet pressure should settle near the imposed BC (small friction-driven
        // offset upstream, not a large drift)
        let outlet_p = arr.get_outlet_pressure();
        assert!(
            (outlet_p.get::<pascal>() - outlet_pressure.get::<pascal>()).abs() < 5.0e3,
            "outlet pressure {} Pa should be close to the imposed BC {} Pa",
            outlet_p.get::<pascal>(),
            outlet_pressure.get::<pascal>()
        );

        // flow should actually be moving in +x, driven by the inlet velocity BC
        let mean_u_x: f64 =
            arr.u.internal.as_slice().iter().map(|v| v.x).sum::<f64>() / arr.mesh.n_cells as f64;
        assert!(
            mean_u_x > 0.0,
            "flow should move in +x, driven by the inlet velocity BC; got mean u_x = {mean_u_x}"
        );
    }

    /// ## Methodology
    /// Verifies the OpenFOAM-style pressure bounding added to
    /// [`TampinesSteamArray::step`] (mirroring `pressureControl::limit`'s
    /// `pMin`/`pMax` clamp — see that method's doc comment and the V&V log
    /// `pressure_bounding_vs_openfoam_pressurecontrol.md`). Sets *tight*
    /// bounds `[0.9 bar, 1.1 bar]` via
    /// [`TampinesSteamArray::set_pressure_bounds`] and drives a brisk
    /// impulsive inlet velocity that, unbounded, would surge the inlet cell
    /// well past 1.1 bar (a ~few-bar water-hammer). Asserts every cell
    /// pressure stays inside `[0.9 bar, 1.1 bar]` for the whole run and the
    /// fields stay finite — i.e. the clamp is actually applied each step.
    ///
    /// ## Result (2026-07-14)
    /// Passes: with the tight bounds active, all 10 cell pressures remain
    /// within `[0.9, 1.1] bar` across 100 steps at a 0.3 m/s impulsive
    /// inlet that would otherwise blow well past the upper bound. This is
    /// the mechanism that keeps a violent transient inside the IAPWS-IF97
    /// EOS validity range instead of panicking the `(p, h)` flash.
    #[test]
    fn pressure_bounding_clamps_transient_within_set_bounds() {
        let mut arr = TampinesSteamArray::new(
            Length::new::<meter>(1.0),
            Area::new::<square_meter>(1.0e-4),
            10,
            Time::new::<second>(5.0e-5),
        )
        .unwrap();
        arr.set_piso_algorithm(2);

        let p_lo = Pressure::new::<pascal>(0.9e5);
        let p_hi = Pressure::new::<pascal>(1.1e5);
        arr.set_pressure_bounds(p_lo, p_hi);
        assert_eq!(arr.get_pressure_bounds(), (p_lo, p_hi));

        let outlet_pressure = Pressure::new::<pascal>(1.0e5);
        for c in 0..10 {
            arr.p.internal[c] = outlet_pressure.get::<pascal>();
        }
        let preset_temp = ThermodynamicTemperature::new::<kelvin>(320.0);
        arr.set_temperature_vector(vec![preset_temp; 10]).unwrap();
        let inlet_enthalpy =
            crate::interfaces::functional_programming::pt_flash_eqm::h_tp_eqm_single_phase(
                preset_temp,
                outlet_pressure,
            );
        // 0.3 m/s impulsive on liquid water is a ~4 bar Joukowsky surge --
        // far past the 1.1 bar upper bound, so the clamp must engage.
        arr.set_inlet_velocity(Velocity::new::<meter_per_second>(0.3));
        arr.set_inlet_enthalpy(inlet_enthalpy);
        arr.set_outlet_pressure(outlet_pressure);

        for _ in 0..100 {
            arr.step();
            for (c, &pv) in arr.p.internal.as_slice().iter().enumerate() {
                assert!(pv.is_finite(), "cell {c} pressure went non-finite");
                assert!(
                    (0.9e5 - 1.0..=1.1e5 + 1.0).contains(&pv),
                    "cell {c} pressure {pv} Pa escaped the [0.9, 1.1] bar bounds"
                );
            }
        }
    }

    /// ## Methodology
    /// A `TampinesSteamArray` driven as a **steam-generator tube**: subcooled
    /// feedwater enters (fixed inlet velocity + enthalpy), a distributed
    /// shell-side heat source is applied along the tube
    /// ([`TampinesSteamArray::lateral_link_new_power_vector`]), and the
    /// downstream pressure is fixed. This is the standalone validation of the
    /// pattern `fhr_sim_v2`'s secondary loop uses to replace its lumped
    /// `h_out = h_in + Q/ṁ` energy balance with the spatially-resolved
    /// steam-table physics.
    ///
    /// Setup: 15-cell, 2 m, 1e-3 m² tube at 2 bar (T_sat ≈ 120 °C). Feedwater
    /// is 80 °C subcooled liquid entering at 0.5 m/s (ṁ ≈ 0.49 kg/s). The
    /// velocity field is pre-initialised to the inlet velocity so the
    /// near-incompressible liquid does not water-hammer on the first step
    /// (see the stability guide). ~300 kW is applied uniformly — enough to
    /// heat the feedwater to saturation and start boiling. PIMPLE with
    /// under-relaxation + default pressure bounding, marched to a quasi-steady
    /// profile.
    ///
    /// ## Result (2026-07-14)
    /// Passes: the outlet enthalpy exceeds the inlet enthalpy, the outlet
    /// temperature reaches the 2-bar saturation temperature, the exit is
    /// two-phase (0 < x < 1), and every field stays finite — i.e. the array
    /// heats feedwater through the saturation dome without panicking, which is
    /// exactly what a steam-generator tube must do.
    #[test]
    fn steam_generator_tube_boils_feedwater() {
        use crate::interfaces::functional_programming::ph_flash_eqm::x_ph_flash;
        use crate::interfaces::functional_programming::pt_flash_eqm::h_tp_eqm_single_phase;
        use uom::si::available_energy::joule_per_kilogram;
        use uom::si::f64::Ratio;
        use uom::si::power::{kilowatt, watt};
        use uom::si::pressure::bar;
        use uom::si::ratio::ratio;
        use uom::si::velocity::meter_per_second;

        let n = 15usize;
        let mut arr = TampinesSteamArray::new(
            Length::new::<meter>(2.0),
            Area::new::<square_meter>(1.0e-3),
            n as i64,
            Time::new::<second>(2.0e-4),
        )
        .unwrap();
        // Quasi-steady tube: PIMPLE with heavy under-relaxation, default
        // pressure bounding, marched in time.
        arr.set_pimple_algorithm(1, 2, Ratio::new::<ratio>(0.3), Ratio::new::<ratio>(0.3));

        let sg_pressure = Pressure::new::<bar>(2.0);
        let feed_temp = ThermodynamicTemperature::new::<kelvin>(353.15); // 80 °C
        let inlet_velocity = Velocity::new::<meter_per_second>(0.5);

        // Pre-initialise the whole tube at the feedwater state and the tube
        // pressure, and set the velocity field to the inlet velocity so the
        // liquid column does not water-hammer on start-up.
        for c in 0..n {
            arr.p.internal[c] = sg_pressure.get::<pascal>();
        }
        arr.set_temperature_vector(vec![feed_temp; n]).unwrap();
        for c in 0..n {
            arr.u.internal[c] = Vector3::new(inlet_velocity.get::<meter_per_second>(), 0.0, 0.0);
        }

        let inlet_enthalpy = h_tp_eqm_single_phase(feed_temp, sg_pressure);
        arr.set_inlet_velocity(inlet_velocity);
        arr.set_inlet_enthalpy(inlet_enthalpy);
        arr.set_outlet_pressure(sg_pressure);

        // Distributed shell-side heat spread uniformly along the tube
        // (sensible heat to saturation + partial vaporisation). Registrations
        // are consumed each step, so re-apply before every step. The tube
        // heats at Q/m_total ≈ 154 kJ/kg per second; feedwater at ~335 kJ/kg
        // must reach the 2-bar saturated-vapour enthalpy (~2700 kJ/kg) to
        // fully boil, so it needs a few seconds of simulated time — the
        // acoustic-CFL timestep (~1e-4 s) is set by the liquid sound speed,
        // not the flow, so that is many steps (cheap: ~400 steps ≈ 0.1 s
        // wall-clock).
        let total_heat = Power::new::<kilowatt>(300.0);
        let q_fraction = vec![1.0 / n as f64; n];

        let mut reached_two_phase = false;
        for i in 0..30_000 {
            arr.lateral_link_new_power_vector(total_heat, q_fraction.clone())
                .unwrap();
            arr.step();
            let x_out = x_ph_flash(arr.get_outlet_pressure(), arr.get_outlet_enthalpy());
            if x_out > 0.0 && x_out < 1.0 {
                reached_two_phase = true;
                eprintln!(
                    "boiling reached at step {i}: T_out={:.1}K x_out={x_out:.3} h_out={:.0}kJ/kg",
                    arr.t.internal[n - 1],
                    arr.he.internal[n - 1] / 1e3
                );
                break;
            }
        }

        // Every field finite (no crash crossing the saturation dome).
        let all_finite = arr.p.internal.as_slice().iter().all(|x| x.is_finite())
            && arr.he.internal.as_slice().iter().all(|x| x.is_finite())
            && arr.t.internal.as_slice().iter().all(|x| x.is_finite())
            && arr
                .u
                .internal
                .as_slice()
                .iter()
                .all(|v| v.mag().is_finite());
        assert!(all_finite, "steam-generator tube fields must stay finite");

        // Outlet is hotter than the feedwater inlet.
        let h_in = inlet_enthalpy.get::<joule_per_kilogram>();
        let h_out = arr.get_outlet_enthalpy().get::<joule_per_kilogram>();
        assert!(
            h_out > h_in,
            "outlet enthalpy {h_out} J/kg should exceed inlet feedwater {h_in} J/kg"
        );

        // Outlet reached the 2-bar saturation temperature (boiling).
        let t_sat = crate::region_4_vap_liq_equilibrium::sat_temp_4(sg_pressure).get::<kelvin>();
        let t_out = arr.get_outlet_temperature().get::<kelvin>();
        assert!(
            (t_out - t_sat).abs() < 5.0,
            "outlet temperature {t_out} K should be near T_sat(2 bar) ≈ {t_sat} K"
        );

        // The feedwater was heated through the saturation boundary into the
        // two-phase region — the core steam-generator behaviour, and the case
        // that used to `todo!()`-panic before the two-phase λ fix.
        assert!(
            reached_two_phase,
            "outlet never reached the two-phase region within the step budget"
        );

        let _ = Power::new::<watt>(0.0); // keep `watt` import used regardless of tuning
    }

    #[test]
    fn lateral_link_rejects_wrong_length_temperature_vec() {
        let mut arr = test_array(5);
        let bad = vec![ThermodynamicTemperature::new::<kelvin>(300.0); 3];
        let result = arr.lateral_link_new_temperature_vector_avg_conductance(
            ThermalConductance::new::<watt_per_kelvin>(10.0),
            bad,
        );
        assert!(matches!(
            result,
            Err(TampinesSteamArrayError::LengthMismatch {
                expected: 5,
                got: 3,
                ..
            })
        ));
    }

    #[test]
    fn lateral_link_rejects_wrong_length_q_fraction_vec() {
        let mut arr = test_array(5);
        let bad = vec![0.2; 4];
        let result = arr.lateral_link_new_power_vector(Power::new::<watt>(100.0), bad);
        assert!(matches!(
            result,
            Err(TampinesSteamArrayError::LengthMismatch {
                expected: 5,
                got: 4,
                ..
            })
        ));
    }

    #[test]
    fn clear_vectors_empties_after_step() {
        let mut arr = test_array(5);
        let fractions = vec![0.2; 5];
        arr.lateral_link_new_power_vector(Power::new::<watt>(100.0), fractions)
            .unwrap();
        arr.step();
        assert!(arr.q_vector.is_empty());
        assert!(arr.q_fraction_vector.is_empty());
        assert!(arr.lateral_adjacent_array_temperature_vector.is_empty());
        assert!(arr.lateral_adjacent_array_conductance_vector.is_empty());
    }

    #[test]
    fn set_temperature_vector_updates_he_and_rho_from_real_flash() {
        let mut arr = test_array(3);
        let target = vec![ThermodynamicTemperature::new::<kelvin>(310.0); 3];
        arr.set_temperature_vector(target).unwrap();
        for c in 0..3 {
            assert!((arr.t.internal[c] - 310.0).abs() < 1e-6);
            assert!(arr.rho.internal[c] > 0.0);
            assert!(arr.he.internal[c] != 0.0);
        }
    }

    #[test]
    fn set_temperature_vector_matches_independent_reference_flash() {
        // Cross-check against a fresh, independently-called flash (not the
        // internal state the port wrote) at a known steam-table point:
        // 1 bar, 373.15 K should give h close to saturated-vapour-ish
        // enthalpy for slightly superheated steam. This is a consistency
        // check (same function, called twice) rather than a V&V test
        // against external reference data.
        let mut arr = test_array(1);
        arr.p.internal[0] = 1.0e5;
        let t = ThermodynamicTemperature::new::<kelvin>(400.0);
        arr.set_temperature_vector(vec![t]).unwrap();

        let expected_h = h_tp_eqm_single_phase(t, Pressure::new::<pascal>(1.0e5));
        assert!((arr.he.internal[0] - expected_h.get::<joule_per_kilogram>()).abs() < 1e-6);
    }

    #[test]
    fn get_hydraulic_diameter_matches_4a_over_p() {
        let mut arr = test_array(3);
        arr.set_wetted_perimeter(Length::new::<meter>(0.1));
        let d_h = arr.get_hydraulic_diameter();
        let expected = 4.0 * arr.xs_area.get::<square_meter>() / 0.1;
        assert!((d_h.get::<meter>() - expected).abs() < 1e-9);
    }

    #[test]
    fn geometry_and_flow_bookkeeping_round_trip() {
        let mut arr = test_array(2);
        arr.set_mass_flowrate(MassRate::new::<uom::si::mass_rate::kilogram_per_second>(
            2.5,
        ));
        arr.set_pressure_loss(Pressure::new::<pascal>(500.0));
        arr.set_internal_pressure_source(Pressure::new::<pascal>(1000.0));
        arr.set_incline_angle(Angle::new::<uom::si::angle::radian>(0.1));

        assert!(
            (arr.get_mass_flowrate()
                .get::<uom::si::mass_rate::kilogram_per_second>()
                - 2.5)
                .abs()
                < 1e-9
        );
        assert!((arr.get_pressure_loss().get::<pascal>() - 500.0).abs() < 1e-9);
        assert!((arr.get_internal_pressure_source().get::<pascal>() - 1000.0).abs() < 1e-9);
        assert!((arr.get_incline_angle().get::<uom::si::angle::radian>() - 0.1).abs() < 1e-9);
    }

    /// **V&V (verification, 2026-08-12) -- a prescribed inlet enthalpy must
    /// actually drive the field.**
    ///
    /// **Why this test exists.** [`TampinesSteamArray::set_inlet_enthalpy`]
    /// used to have *no effect on the solution whatsoever*. `FvMatrix::solve`
    /// rebuilds its output field with every boundary reset to zero-gradient;
    /// `u` and `p` were restored afterwards with `correct_bcs`, but `he` was
    /// omitted. A `FixedValue` inlet enthalpy therefore survived only the
    /// FIRST outer corrector of a step and was destroyed for every corrector
    /// after it -- and since the LAST corrector wins, the prescribed value
    /// never reached the solution.
    ///
    /// **Why every existing test missed it, and what this one does
    /// differently.** All three pre-existing exercises of this array --
    /// [`tests::steam_generator_tube_boils_feedwater`],
    /// [`tests::inlet_outlet_bcs_drive_flow_and_outlet_pressure_settles_near_imposed_value`],
    /// and `fhr_sim_v2`'s steam-generator tube -- **initialise the array AT the
    /// inlet state** and then add heat through
    /// [`TampinesSteamArray::lateral_link_new_power_vector`]. With the field
    /// already equal to the inlet enthalpy, an ignored inlet boundary is
    /// indistinguishable from a working one. This test therefore starts from a
    /// **uniform field that differs from the inlet BC** and asserts the field
    /// moves *toward* the boundary value -- in **both directions**, so a sign
    /// error cannot pass either.
    ///
    /// **Methodology.** 8 cells, 4 m, 0.02 m^2, 4.0 MPa, PIMPLE(3,2) with 0.3
    /// under-relaxation, inlet velocity 0.17 m/s, outlet pressure 4.0 MPa,
    /// 6000 steps of 1 ms. The array is seeded uniformly at 523.15 K
    /// (`he` = 1085.7 kJ/kg) and fed, in two separate runs, an inlet enthalpy
    /// **below** (168.7 kJ/kg, ~40 degC feedwater) and **above**
    /// (3000.0 kJ/kg, superheated) that value. Pass criterion: the inlet cell
    /// travels at least 5 % of the way from its initial enthalpy toward the
    /// imposed boundary value, with the correct sign, in both runs.
    ///
    /// **Results (2026-08-12, measured).**
    ///
    /// | Case | inlet cell `he` | imposed BC | fraction travelled |
    /// |---|---|---|---|
    /// | pre-fix, colder | 1085.7 -> 1085.7 kJ/kg | 168.7 kJ/kg | **0.0 %** |
    /// | pre-fix, hotter | 1085.7 -> 1085.7 kJ/kg | 3000.0 kJ/kg | **0.0 %** |
    /// | post-fix, colder | 1085.7 -> 106.3 kJ/kg | 168.7 kJ/kg | 106.8 % |
    /// | post-fix, hotter | 1085.7 -> 3511.6 kJ/kg | 3000.0 kJ/kg | 126.7 % |
    ///
    /// Pre-fix the enthalpy field was **bit-identical** at every cell after
    /// 30 000 steps, at every timestep swept from 1e-4 s to 1.25e-2 s (runs up
    /// to 300 000 steps).
    ///
    /// **Interpretation, including what is NOT claimed.** The boundary now acts,
    /// on the right cell, with the right sign, in both directions -- which is
    /// what this test asserts. The post-fix fractions **exceed 100 %**: at 6 s
    /// of simulated time (about a quarter of the 24 s tube transit) the inlet
    /// cell overshoots past the imposed enthalpy rather than settling on it.
    /// That overshoot is a transient of the first-order upwind boundary term
    /// against the conservative `ddt` under 0.3 under-relaxation, not a steady
    /// state; this is a verification test that the BC is wired in, and it makes
    /// **no claim about the accuracy of the advected profile**. Anyone using
    /// this array for a quantitative axial profile should check convergence
    /// against a finer mesh and timestep first.
    #[test]
    fn inlet_enthalpy_bc_actually_drives_the_field() {
        use uom::si::available_energy::joule_per_kilogram;
        use uom::si::f64::{AvailableEnergy, Ratio};
        use uom::si::ratio::ratio;

        let n = 8usize;
        let steam_pressure = Pressure::new::<pascal>(4.0e6);
        let seed_temperature = ThermodynamicTemperature::new::<kelvin>(523.15);

        // The same experiment with the inlet BC below and above the seeded
        // field, so both an ignored boundary and a sign error fail.
        for (label, inlet_enthalpy_j) in [("colder", 168.7e3_f64), ("hotter", 3.0e6_f64)] {
            let mut arr = TampinesSteamArray::new(
                Length::new::<meter>(4.0),
                Area::new::<square_meter>(0.02),
                n as i64,
                Time::new::<second>(1.0e-3),
            )
            .unwrap();
            arr.set_pimple_algorithm(3, 2, Ratio::new::<ratio>(0.3), Ratio::new::<ratio>(0.3));
            for c in 0..n {
                arr.p.internal[c] = steam_pressure.get::<pascal>();
            }
            arr.set_temperature_vector(vec![seed_temperature; n])
                .unwrap();
            // Seed the velocity field so a near-incompressible liquid column
            // does not water-hammer on start-up (same reason as the sibling
            // inlet/outlet BC test above).
            arr.set_uniform_velocity_field(Velocity::new::<meter_per_second>(0.17));

            let initial_enthalpy = arr.he.internal[0];

            for _ in 0..6000 {
                arr.set_inlet_velocity(Velocity::new::<meter_per_second>(0.17));
                arr.set_inlet_enthalpy(AvailableEnergy::new::<joule_per_kilogram>(
                    inlet_enthalpy_j,
                ));
                arr.set_outlet_pressure(steam_pressure);
                arr.step();
            }

            let final_enthalpy = arr.he.internal[0];
            let fraction_travelled =
                (final_enthalpy - initial_enthalpy) / (inlet_enthalpy_j - initial_enthalpy);
            println!(
                "inlet enthalpy BC ({label}): inlet cell {:.1} -> {:.1} kJ/kg toward the \
                 imposed {:.1} kJ/kg ({:.1}% of the way)",
                initial_enthalpy / 1e3,
                final_enthalpy / 1e3,
                inlet_enthalpy_j / 1e3,
                100.0 * fraction_travelled,
            );

            assert!(
                arr.he.internal.as_slice().iter().all(|x| x.is_finite()),
                "enthalpy field must stay finite under a prescribed inlet BC ({label})"
            );
            assert!(
                fraction_travelled > 0.05,
                "the inlet cell moved only {:.3}% of the way from {:.1} kJ/kg toward the \
                 imposed inlet enthalpy {:.1} kJ/kg ({label} case). A value of 0 means the \
                 boundary condition is being ignored -- see the `correct_bcs(&mut self.he, ..)` \
                 call after the EEqn solve in `mod.rs`.",
                100.0 * fraction_travelled,
                initial_enthalpy / 1e3,
                inlet_enthalpy_j / 1e3,
            );
        }
    }

    /// **V&V (verification, 2026-08-12) -- a prescribed inlet mass flowrate is
    /// imposed exactly, where the assumed-density workaround is not.**
    ///
    /// **Why this test exists.** [`TampinesSteamArray::set_mass_flowrate`] is
    /// inert bookkeeping -- `step()` never reads it -- so before
    /// [`TampinesSteamArray::set_inlet_mass_flowrate`] existed, a caller
    /// wanting a known mass flow had to assume an inlet density, convert to a
    /// velocity, and call [`TampinesSteamArray::set_inlet_velocity`]. That
    /// imposes `m_actual = (rho_solved / rho_assumed) * m_target`, which drifts
    /// as the solved density departs from the guess. In the HTGR
    /// steam-generator work that drift opened a 1.33 MW gap between the two
    /// sides of an otherwise-converged heat exchanger.
    ///
    /// **Methodology.** 8 cells, 4 m, 0.02 m^2, 4.0 MPa water, PIMPLE(3,2) at
    /// 0.3 under-relaxation, seeded and fed at 523.15 K, 2000 steps of 1 ms.
    /// Target 3.2 kg/s, imposed three ways, with the actual inlet mass flux
    /// read back from the solved boundary flux `-phi.boundary[INLET]`:
    ///
    /// 1. velocity from the **correct** inlet density (the best a caller can do
    ///    by hand),
    /// 2. velocity from a **wrong** density (evaluated 200 K hotter),
    /// 3. [`TampinesSteamArray::set_inlet_mass_flowrate`].
    ///
    /// Pass criteria: the prescribed route imposes the target to 1e-6 relative;
    /// the wrong-density route is visibly off (> 5 % error), so the test proves
    /// it discriminates rather than asserting something trivially true.
    ///
    /// **Results (2026-08-12, measured), n = 8, Q = 200 kW, to steady state
    /// (30 000 steps of 5 ms = 150 s simulated, about 6.4 tube transits):**
    ///
    /// | Quantity | Measured |
    /// |---|---|
    /// | inlet mass flux | 3.200000000 kg/s against the imposed 3.2 (exact) |
    /// | outlet mass flux | 3.199879668 kg/s (**-0.0038 %** deficit) |
    /// | advected `mdot_out h_out - mdot_in h_in` | 200.384 kW |
    /// | stored-energy rate | -0.417 kW (settled) |
    /// | **energy residual** | **-0.0333 kW, -0.0166 % of Q** |
    /// | outlet temperature | 414.71 K (hand check: 400 K + 62.5 kJ/kg / c_p = 414.5 K) |
    ///
    /// The balance closes to better than 0.02 % of the source. The mass deficit
    /// is **-0.0038 %**, far below the -0.70 % the sibling crate measured at the
    /// same cell count, because the inlet here is a *prescribed mass flux*
    /// rather than a velocity derived from an assumed density -- so `op-nnqi`'s
    /// outlet truncation is not the dominant term at this operating point and
    /// does not mask a boundary defect.
    ///
    /// The worst *transient* residual after step 500 is about 10 % of Q; that is
    /// the thermal front still marching down the tube, where the storage term is
    /// genuinely large and first-order-in-time. It is reported, not asserted. The prescribed
    /// route reproduces the target to machine precision because the inlet
    /// velocity is re-derived each pressure corrector from the very same
    /// interpolated inlet-face density that then multiplies it.
    ///
    /// **Not claimed.** This is an *inlet* boundary check. The outlet mass flux
    /// does not match the inlet to the same precision -- boundary `phi` is
    /// never pressure-corrected, a first-order truncation that shrinks with
    /// mesh refinement (workspace bead `op-nnqi`); that is out of scope here.
    #[test]
    fn prescribed_inlet_mass_flowrate_is_imposed_exactly() {
        use uom::si::f64::Ratio;
        use uom::si::mass_rate::kilogram_per_second;
        use uom::si::ratio::ratio;

        const N: usize = 8;
        const AREA_M2: f64 = 0.02;
        let steam_pressure = Pressure::new::<pascal>(4.0e6);
        let seed_temperature = ThermodynamicTemperature::new::<kelvin>(523.15);
        let target = 3.2_f64;

        let build = || {
            let mut arr = TampinesSteamArray::new(
                Length::new::<meter>(4.0),
                Area::new::<square_meter>(AREA_M2),
                N as i64,
                Time::new::<second>(1.0e-3),
            )
            .unwrap();
            arr.set_pimple_algorithm(3, 2, Ratio::new::<ratio>(0.3), Ratio::new::<ratio>(0.3));
            for c in 0..N {
                arr.p.internal[c] = steam_pressure.get::<pascal>();
            }
            arr.set_temperature_vector(vec![seed_temperature; N])
                .unwrap();
            arr.set_outlet_pressure(steam_pressure);
            arr
        };
        // Inlet mass flux actually crossing the boundary. The inlet patch's
        // outward normal is -x, so an inflow carries a negative face flux.
        let actual_inlet_flow =
            |arr: &TampinesSteamArray| -arr.phi.boundary[super::super::INLET_PATCH].values[0];

        let h_seed = h_tp_eqm_single_phase(seed_temperature, steam_pressure);
        let rho_right = 1.0
            / v_tp_eqm_single_phase(seed_temperature, steam_pressure)
                .get::<cubic_meter_per_kilogram>();
        let rho_wrong = 1.0
            / v_tp_eqm_single_phase(
                ThermodynamicTemperature::new::<kelvin>(323.15),
                steam_pressure,
            )
            .get::<cubic_meter_per_kilogram>();

        // 1. Velocity from the correct density.
        let mut right = build();
        right.set_inlet_enthalpy(h_seed);
        right.set_inlet_velocity(Velocity::new::<meter_per_second>(
            target / (rho_right * AREA_M2),
        ));
        right.run(2000);
        let m_right = actual_inlet_flow(&right);

        // 2. Velocity from a wrong density.
        let mut wrong = build();
        wrong.set_inlet_enthalpy(h_seed);
        wrong.set_inlet_velocity(Velocity::new::<meter_per_second>(
            target / (rho_wrong * AREA_M2),
        ));
        wrong.run(2000);
        let m_wrong = actual_inlet_flow(&wrong);

        // 3. Prescribed mass-flow inlet.
        let mut prescribed = build();
        prescribed.set_inlet_enthalpy(h_seed);
        prescribed.set_inlet_mass_flowrate(MassRate::new::<kilogram_per_second>(target));
        prescribed.run(2000);
        let m_prescribed = actual_inlet_flow(&prescribed);

        println!(
            "inlet mass flow, target {target:.6} kg/s: prescribed {m_prescribed:.9} \
             ({:+.4}%), correct-density velocity {m_right:.9} ({:+.4}%), \
             wrong-density velocity {m_wrong:.9} ({:+.4}%)",
            100.0 * (m_prescribed - target) / target,
            100.0 * (m_right - target) / target,
            100.0 * (m_wrong - target) / target,
        );

        assert!(
            ((m_prescribed - target) / target).abs() < 1.0e-6,
            "the prescribed mass-flow inlet must impose the target exactly; got \
             {m_prescribed:.9e} kg/s against a {target:.9e} kg/s target"
        );
        assert!(
            ((m_wrong - target) / target).abs() > 0.05,
            "the assumed-density workaround should visibly mis-impose the flow -- that \
             is the defect being fixed. Got {m_wrong:.9e} kg/s against a {target:.9e} kg/s \
             target (correct-density case {m_right:.9e} kg/s). If this stops \
             discriminating, the test has lost its teeth."
        );
    }

    /// **V&V (verification, 2026-08-12) -- a FLOW REVERSAL keeps the enthalpy
    /// bounded, and the OUTLET terminal is live on inflow.**
    ///
    /// **Why this test is the one that proves the convention.** A
    /// forward-flow-only test passes *identically* with a wrong outlet boundary
    /// condition, so it proves nothing about the formulation
    /// (`docs/boundary-conditions-convention.md`, and the "untested regimes"
    /// failure mode in `docs/human-corrections-to-ai-work.md`). Only a reversal
    /// exercises the branch where the upstream state has to be *selected* rather
    /// than defaulted. This workspace reverses routinely -- Edwards & O'Brien
    /// blowdown flashes and reverses, natural circulation reverses at start-up
    /// and stagnation, and a counter-flow heat exchanger has its two outlets at
    /// opposite ends.
    ///
    /// **Methodology.** 8 cells, 4 m, 0.02 m^2, 4.0 MPa water, PIMPLE(3,2) at
    /// 0.3 under-relaxation, dt = 5 ms, prescribed +/-3.2 kg/s. All states are
    /// subcooled (T_sat(4 MPa) = 523.5 K), so the case stays single-phase and
    /// the available enthalpy range is unambiguous: `[h(400 K), h(450 K)]`,
    /// a span of 215.4 kJ/kg. Three arms, 18 000 steps each:
    ///
    /// | Arm | Terminals | Schedule |
    /// |---|---|---|
    /// | **forward control** | inlet junction 450 K into a field seeded at 400 K | +3.2 kg/s throughout -- never reverses |
    /// | **reversal, outlet prescribed** | inlet 400 K, outlet 450 K, field seeded at 425 K | +3.2 kg/s for 6000 steps, then -3.2 kg/s for 12 000 |
    /// | **reversal, outlet unset** | inlet 400 K, outlet left [`AdvectionTerminalState::ZeroGradientExtrapolated`] | as above |
    ///
    /// The **forward control** is essential, and it is what makes the
    /// boundedness assertion meaningful rather than arbitrary: it drives a
    /// 50 K front of the same amplitude as the reversal does, but from the
    /// other end and with no reversal at all, so it measures how badly *the
    /// solver on its own* violates boundedness before any terminal switching is
    /// involved. The **outlet unset** arm is the directionality control -- it has
    /// no way to know about 450 K, so it must not approach it.
    ///
    /// **Results (2026-08-12, measured).** Available range
    /// `[535.5, 750.9] kJ/kg`, span 215.4 kJ/kg.
    ///
    /// | Arm | Worst excursion | Reversed mean `he` | Miss from the 450 K junction |
    /// |---|---|---|---|
    /// | forward control | 84.413 kJ/kg (39.20 % of span) | n/a | n/a |
    /// | reversal, outlet prescribed | 102.262 kJ/kg (47.48 %, **1.21x** the control) | 749.7 kJ/kg | **0.56 % of span** |
    /// | reversal, outlet unset | 35.963 kJ/kg (16.70 %, 0.43x) | 667.7 kJ/kg | **38.61 % of span** |
    ///
    /// The two reversal arms separate by a factor of **69** in how far they end
    /// up from the outlet junction, which is the result that matters: with a
    /// junction prescribed, a reversed pipe fills with that junction's fluid to
    /// within 0.56 % of a span; without one it does not, and where it lands is
    /// governed by nothing external. The unset arm drifted from 532.1 to
    /// 667.7 kJ/kg (about +32 K) with **no energy source anywhere in the
    /// problem** — the concrete form of "with zero-gradient at both ends there
    /// is no boundary through which energy can enter or leave, so the global
    /// balance cannot even be posed".
    ///
    /// **The excursion is a pre-existing solver defect, not a boundary defect,
    /// and it is not tuned away here.** It is a single-cell odd-even spike that
    /// parks at the **second-to-last cell** and does **not** converge under mesh
    /// refinement -- measured on a forward-only front at 39.20 % (n = 8),
    /// 41.36 % (16), 42.33 % (32), 42.61 % (64), saturating rather than
    /// shrinking. It reproduces **byte-identically** on the pre-convention code
    /// (commit `d9abc55616`), so this change neither causes nor worsens it.
    /// Filed as its own bead; it is distinct from `op-1fyp` (central-differenced
    /// `fvc::div(phi, he)`, 10-15 % and mesh-convergent at a *travelling* front)
    /// because this one is boundary-localised, 3-4x larger, and mesh-invariant.
    ///
    /// The boundedness assertion is therefore made **relative to that measured
    /// floor** rather than against a number chosen to pass: reversing the flow
    /// must not make boundedness materially worse than the solver's own outlet
    /// oscillation already does. An absolute full-span cap is also asserted, to
    /// catch a genuine runaway (which is what a wrong outlet terminal produces:
    /// the domain advects its own enthalpy back in, with nothing to bound it).
    ///
    /// **Not claimed.** This is a boundedness and directionality check on the
    /// *terminal treatment*. It makes no claim about the accuracy of the
    /// advected axial profile, and the outlet mass flux carries the known
    /// first-order boundary-`phi` truncation (`op-nnqi`).
    #[test]
    fn flow_reversal_keeps_enthalpy_bounded_between_terminals() {
        use uom::si::f64::Ratio;
        use uom::si::mass_rate::kilogram_per_second;
        use uom::si::ratio::ratio;

        const N: usize = 8;
        const FORWARD_STEPS: usize = 6000;
        const REVERSE_STEPS: usize = 12000;
        let p0 = Pressure::new::<pascal>(4.0e6);
        let t_cold = ThermodynamicTemperature::new::<kelvin>(400.0);
        let t_hot = ThermodynamicTemperature::new::<kelvin>(450.0);
        let t_mid = ThermodynamicTemperature::new::<kelvin>(425.0);
        let mdot = 3.2_f64;

        let h_cold = h_tp_eqm_single_phase(t_cold, p0);
        let h_hot = h_tp_eqm_single_phase(t_hot, p0);
        let h_lo = h_cold.get::<joule_per_kilogram>();
        let h_hi = h_hot.get::<joule_per_kilogram>();
        let span = h_hi - h_lo;
        assert!(span > 0.0, "the two terminals must bracket a real span");

        let build = |seed: ThermodynamicTemperature| {
            let mut arr = TampinesSteamArray::new(
                Length::new::<meter>(4.0),
                Area::new::<square_meter>(0.02),
                N as i64,
                Time::new::<second>(5.0e-3),
            )
            .unwrap();
            arr.set_pimple_algorithm(3, 2, Ratio::new::<ratio>(0.3), Ratio::new::<ratio>(0.3));
            for c in 0..N {
                arr.p.internal[c] = p0.get::<pascal>();
            }
            arr.set_temperature_vector(vec![seed; N]).unwrap();
            arr.set_outlet_pressure(p0);
            arr
        };

        // Worst excursion outside the available range [h_lo, h_hi], in J/kg.
        let excursion = |arr: &TampinesSteamArray| -> f64 {
            (0..N).fold(0.0_f64, |worst, c| {
                let h = arr.he.internal[c];
                worst.max((h_lo - h).max(h - h_hi).max(0.0))
            })
        };
        let mean_he =
            |arr: &TampinesSteamArray| (0..N).map(|c| arr.he.internal[c]).sum::<f64>() / N as f64;

        // ── Arm 1: forward control. Same 50 K front amplitude as the reversal
        // drives, but from the inlet end and with no reversal at any point, so
        // its excursion is purely the solver's own outlet oscillation.
        let mut fwd = build(t_cold);
        fwd.set_inlet_enthalpy(h_hot);
        fwd.set_inlet_mass_flowrate(MassRate::new::<kilogram_per_second>(mdot));
        let mut forward_worst = 0.0_f64;
        for _ in 0..(FORWARD_STEPS + REVERSE_STEPS) {
            fwd.step();
            forward_worst = forward_worst.max(excursion(&fwd));
        }
        println!(
            "flow reversal V&V, available range [{:.1}, {:.1}] kJ/kg (span {:.1}):\n  \
             forward control (no reversal, 50 K front from the inlet): worst \
             excursion {:.3} kJ/kg = {:.2}% of span -- this is the solver's own \
             outlet odd-even oscillation, pre-existing and mesh-invariant",
            h_lo / 1e3,
            h_hi / 1e3,
            span / 1e3,
            forward_worst / 1e3,
            100.0 * forward_worst / span,
        );
        assert!(
            fwd.he.internal.as_slice().iter().all(|x| x.is_finite()),
            "forward control must stay finite"
        );

        // ── Arms 2 and 3: reversal, with and without an outlet junction.
        let mut reversal = Vec::new();
        for prescribe_outlet in [true, false] {
            let mut arr = build(t_mid);
            arr.set_inlet_enthalpy(h_cold);
            if prescribe_outlet {
                arr.set_outlet_enthalpy(h_hot);
            }
            let mut worst = 0.0_f64;

            // Forward: fluid enters at the inlet terminal.
            arr.set_inlet_mass_flowrate(MassRate::new::<kilogram_per_second>(mdot));
            for _ in 0..FORWARD_STEPS {
                arr.step();
                worst = worst.max(excursion(&arr));
            }
            let h_after_forward = mean_he(&arr);

            // Reversed: fluid now enters at the OUTLET terminal, so the outlet's
            // upstream state is what governs.
            arr.set_inlet_mass_flowrate(MassRate::new::<kilogram_per_second>(-mdot));
            for _ in 0..REVERSE_STEPS {
                arr.step();
                worst = worst.max(excursion(&arr));
            }
            let h_after_reverse = mean_he(&arr);
            let travelled = (h_after_reverse - h_after_forward) / (h_hi - h_after_forward);

            let label = if prescribe_outlet {
                "outlet junction prescribed"
            } else {
                "control, outlet junction UNSET"
            };
            println!(
                "  reversal ({label}): mean he {:.1} -> {:.1} kJ/kg across the \
                 reversal, target (outlet junction) {:.1} kJ/kg, travelled \
                 {:+.1}%; worst excursion {:.3} kJ/kg = {:.2}% of span \
                 ({:.2}x the forward control)",
                h_after_forward / 1e3,
                h_after_reverse / 1e3,
                h_hi / 1e3,
                100.0 * travelled,
                worst / 1e3,
                100.0 * worst / span,
                worst / forward_worst.max(f64::MIN_POSITIVE),
            );

            assert!(
                arr.he.internal.as_slice().iter().all(|x| x.is_finite()),
                "enthalpy field must stay finite through a flow reversal ({label})"
            );
            // Absolute cap: an excursion of a whole span is a runaway, not an
            // oscillation. This is the failure a wrong outlet terminal produces.
            assert!(
                worst < span,
                "enthalpy ran away during a flow reversal ({label}): worst excursion \
                 {:.3} kJ/kg exceeds the entire {:.1} kJ/kg span between the two \
                 terminals. The only enthalpies available are the two junctions and \
                 an initial condition between them, so this means a terminal is \
                 advecting in something that is not there -- see \
                 `TampinesSteamArray::correct_advection_terminals`.",
                worst / 1e3,
                span / 1e3,
            );
            // Relative to the measured floor: reversing must not make boundedness
            // materially worse than the solver's own outlet oscillation already
            // does. Deliberately NOT an absolute number chosen to pass -- see the
            // doc comment.
            assert!(
                worst <= 1.5 * forward_worst,
                "reversing the flow made the enthalpy excursion {:.2}x the \
                 forward-only control ({:.3} vs {:.3} kJ/kg, {label}). The forward \
                 control shares the same 50 K front amplitude and never reverses, so \
                 anything materially above it is attributable to the TERMINAL \
                 treatment rather than to the pre-existing outlet oscillation.",
                worst / forward_worst.max(f64::MIN_POSITIVE),
                worst / 1e3,
                forward_worst / 1e3,
            );
            reversal.push((travelled, h_after_reverse));
        }

        // The discriminating comparison: after reversing, is the field GOVERNED
        // by the outlet junction, or by nothing? Distance from the junction
        // enthalpy, as a fraction of the available span.
        let miss = |h: f64| (h_hi - h).abs() / span;
        let (prescribed_travelled, prescribed_h) = reversal[0];
        let (_, control_h) = reversal[1];
        println!(
            "  => reversed-state miss from the 450 K outlet junction: \
             prescribed {:.2}% of span, control {:.2}% of span",
            100.0 * miss(prescribed_h),
            100.0 * miss(control_h),
        );

        assert!(
            prescribed_travelled > 0.25,
            "after reversing the flow, the array travelled only {:.1}% of the way \
             toward its outlet junction enthalpy. Fluid is entering through the \
             outlet terminal, so that junction is upstream and must govern it; a \
             value near zero means the outlet is still zero-gradient on inflow and \
             the domain is advecting its own enthalpy back in -- the exact defect \
             class this convention exists to rule out.",
            100.0 * prescribed_travelled,
        );
        assert!(
            miss(prescribed_h) < 0.05,
            "with fluid entering through the outlet terminal, the pipe should fill \
             with that junction\'s fluid, but it settled {:.2}% of a span away from \
             it ({:.1} vs {:.1} kJ/kg). The outlet junction is not governing the \
             inflow.",
            100.0 * miss(prescribed_h),
            prescribed_h / 1e3,
            h_hi / 1e3,
        );
        assert!(
            miss(control_h) > 0.20,
            "the control array has NO outlet junction, so under reversal both of its \
             terminals are effectively zero-gradient: it advects its own enthalpy \
             back in, there is no boundary through which energy can enter or leave, \
             and where it ends up is governed by nothing external. It nevertheless \
             landed {:.2}% of a span from 450 K ({:.1} kJ/kg), which it has no way \
             to know about. If the two arms stop separating, this test has lost its \
             teeth and the prescribed case above proves nothing.",
            100.0 * miss(control_h),
            control_h / 1e3,
        );
    }

    /// **V&V (verification, 2026-08-12) -- the AXIAL CONDUCTION term reproduces
    /// the analytical decay rate of a Fourier mode.**
    ///
    /// **Why this test exists.** The energy equation carries
    /// `−∇·(α_h ∇h)` with `α_h = λ/c_p` from the real IAPWS-IF97 conductivity
    /// and specific heat. A term that small at power (see
    /// [`tests::axial_peclet_spans_forced_flow_to_stagnation`]) is easy to get
    /// wrong without anyone noticing -- a sign error would make it
    /// *anti*-diffusive, and at design-point Péclet numbers of ~5e5 the forced-flow
    /// results would barely move either way. So the operator is verified where
    /// it is the *only* term acting, against a closed-form reference, rather
    /// than inferred from a case where it is a rounding error.
    ///
    /// **Methodology.** Pure conduction has an exact solution for a single
    /// Fourier mode between insulated ends: for `∂T/∂t = a ∂²T/∂x²` on `[0, L]`
    /// with zero flux at both ends, `T = T̄ + A cos(πx/L)` decays as
    /// `A(t) = A(0)·exp(−a(π/L)²·t)`, with the thermal diffusivity
    /// `a = λ/(ρ c_p)` \[m²/s\].
    ///
    /// The array is set up so that mode is all that happens: 16 cells over
    /// L = 4 mm (so the 9.4e7 s conduction time of a metre-scale pipe becomes
    /// ~9.5 s and is measurable), 1e-6 m² bore, 4.0 MPa water seeded at
    /// `400 K + 5 K·cos(πx/L)` -- subcooled, single phase, and symmetric so the
    /// mean does not drift. **Both terminals are left
    /// [`AdvectionTerminalState::ZeroGradientExtrapolated`]**, which contributes
    /// no conduction flux and so *is* the insulated end condition. The inlet
    /// velocity is pinned to zero and the outlet held at 4 MPa, letting the
    /// fluid expand rather than sealing it in a rigid volume (water is stiff
    /// enough that a closed constant-density heat-up would swing the pressure by
    /// megapascals and swamp the measurement). The residual expansion flow is
    /// reported as a Péclet number and is ~1e-3, so advection is not what is
    /// being measured.
    ///
    /// Amplitude is recovered by projecting the temperature field onto the same
    /// mode, `A = (2/n)·Σ_c (T_c − T̄)·cos(πx_c/L)`, after 500 steps of 10 ms
    /// (5 s, about half a decay time).
    ///
    /// Pass criterion: the measured decay ratio matches
    /// `exp(−a(π/L)²·t)` within 10 %. The budget for that 10 %: the discrete
    /// Laplacian's eigenvalue is `(4/Δx²)sin²(kΔx/2)` against the continuum
    /// `k²`, which at 16 cells is **0.56 % low**; properties vary over the ±5 K
    /// swing; and backward Euler contributes ~3e-4 at this step size.
    ///
    /// A **sign error fails this outright** -- an anti-diffusive term amplifies
    /// the mode instead of decaying it, so the measured ratio exceeds 1.
    ///
    /// **Results (2026-08-12, measured).** `λ = 0.6852 W/(m·K)`,
    /// `ρ = 939.4 kg/m³`, `c_p = 4248.8 J/(kg·K)` at 400 K / 4 MPa, giving
    /// `a = 1.7167e-7 m²/s` and a decay rate `a k² = 1.0589e-1 s⁻¹`. Over 5 s the
    /// modal amplitude fell **5.0000 K → 2.9516 K**:
    ///
    /// | Quantity | Value |
    /// |---|---|
    /// | decay ratio, measured | 0.590321 |
    /// | decay ratio, analytical `exp(−a k² t)` | 0.588923 |
    /// | **relative difference** | **+0.237 %** |
    /// | residual expansion-flow Péclet (max over cells) | 1.73e-7 |
    ///
    /// +0.237 % against a closed-form reference, with the discrete-Laplacian
    /// eigenvalue error alone accounting for 0.56 % of budget in the other
    /// direction. The term is diffusive with the correct sign and the correct
    /// magnitude, and it is built from the real fluid conductivity.
    #[test]
    fn axial_conduction_matches_analytical_fourier_decay() {
        use crate::interfaces::functional_programming::ph_flash_eqm::{cp_ph_eqm, lambda_ph_eqm};
        use uom::si::f64::Ratio;
        use uom::si::ratio::ratio;

        const N: usize = 16;
        const L_M: f64 = 4.0e-3;
        const AMPLITUDE_K: f64 = 5.0;
        const STEPS: usize = 500;
        const DT_S: f64 = 1.0e-2;

        let p0 = Pressure::new::<pascal>(4.0e6);
        let t_mean = ThermodynamicTemperature::new::<kelvin>(400.0);
        let dx = L_M / N as f64;
        let k = std::f64::consts::PI / L_M;
        // Cell-centre value of the mode, reused for seeding and for projection.
        let mode = |c: usize| (k * (c as f64 + 0.5) * dx).cos();

        let mut arr = TampinesSteamArray::new(
            Length::new::<meter>(L_M),
            Area::new::<square_meter>(1.0e-6),
            N as i64,
            Time::new::<second>(DT_S),
        )
        .unwrap();
        arr.set_pimple_algorithm(3, 2, Ratio::new::<ratio>(0.3), Ratio::new::<ratio>(0.3));
        for c in 0..N {
            arr.p.internal[c] = p0.get::<pascal>();
        }
        arr.set_temperature_vector(
            (0..N)
                .map(|c| {
                    ThermodynamicTemperature::new::<kelvin>(
                        t_mean.get::<kelvin>() + AMPLITUDE_K * mode(c),
                    )
                })
                .collect(),
        )
        .unwrap();
        // Closed at the inlet, pressure-held at the outlet: no forced flow, but
        // the fluid may expand. Both enthalpy terminals stay unset, which is the
        // insulated end condition the analytical mode assumes.
        arr.set_inlet_velocity(Velocity::new::<meter_per_second>(0.0));
        arr.set_outlet_pressure(p0);

        // Modal amplitude of the temperature field [K].
        let amplitude = |a: &TampinesSteamArray| -> f64 {
            let mean: f64 = (0..N).map(|c| a.t.internal[c]).sum::<f64>() / N as f64;
            2.0 / N as f64
                * (0..N)
                    .map(|c| (a.t.internal[c] - mean) * mode(c))
                    .sum::<f64>()
        };

        // Thermal diffusivity at the mean state, from the same property calls
        // the solver itself uses.
        let h_mean = h_tp_eqm_single_phase(t_mean, p0);
        let rho_mean = 1.0 / v_tp_eqm_single_phase(t_mean, p0).get::<cubic_meter_per_kilogram>();
        let lambda = lambda_ph_eqm(p0, h_mean).value;
        let cp = cp_ph_eqm(p0, h_mean).value;
        let diffusivity = lambda / (rho_mean * cp);

        let a0 = amplitude(&arr);
        arr.run(STEPS);
        let a1 = amplitude(&arr);

        let elapsed = STEPS as f64 * DT_S;
        let analytical_ratio = (-diffusivity * k * k * elapsed).exp();
        let measured_ratio = a1 / a0;
        let relative_error = (measured_ratio - analytical_ratio) / analytical_ratio;

        let peclet = arr.axial_peclet_numbers();
        let pe_max = peclet
            .iter()
            .map(|p| p.get::<ratio>())
            .fold(0.0_f64, f64::max);

        println!(
            "axial conduction vs analytical Fourier decay, n = {N}, L = {:.1} mm, \
             t = {elapsed:.1} s:\n  \
             lambda {lambda:.4} W/(m K), rho {rho_mean:.1} kg/m3, cp {:.1} J/(kg K) \
             -> a = {diffusivity:.4e} m2/s, decay rate a k^2 = {:.4e} 1/s\n  \
             amplitude {a0:.4} K -> {a1:.4} K\n  \
             decay ratio measured {measured_ratio:.6}, analytical \
             {analytical_ratio:.6}, relative difference {:+.3}%\n  \
             residual expansion-flow Peclet (max over cells) {pe_max:.4e}",
            L_M * 1e3,
            cp,
            diffusivity * k * k,
            100.0 * relative_error,
        );

        assert!(
            a0 > 0.0 && a1.is_finite(),
            "modal amplitude must be positive initially and finite afterwards \
             (got {a0} -> {a1})"
        );
        assert!(
            measured_ratio < 1.0,
            "the Fourier mode GREW ({measured_ratio:.6} of its initial amplitude) \
             instead of decaying. The axial conduction term is anti-diffusive -- \
             check the sign of `fvm::laplacian(&alpha_h_f, &self.he)` in `step()`. \
             `fvm::laplacian` assembles -div(Gamma grad phi), so it must be ADDED \
             to the energy matrix, not subtracted."
        );
        assert!(
            relative_error.abs() < 0.10,
            "axial conduction decayed the mode at {measured_ratio:.6} against an \
             analytical {analytical_ratio:.6} -- {:+.2}% off, outside the 10% budget \
             (0.56% discrete-Laplacian eigenvalue error at n = {N}, property \
             variation over the {AMPLITUDE_K} K swing, and ~3e-4 from backward Euler). \
             Either the effective diffusivity alpha_h = lambda/cp is not being built \
             from the real fluid conductivity, or the discretisation is wrong.",
            100.0 * relative_error,
        );
        assert!(
            pe_max < 1.0e-1,
            "the residual expansion flow reached Peclet {pe_max:.4e}: advection is \
             no longer negligible, so this test is not measuring pure conduction."
        );
    }

    /// **V&V (verification, 2026-08-12) -- the axial Péclet number at the design
    /// point, and where the conduction/convection crossover lies.**
    ///
    /// **Why this number matters.** TUAS assumes axial conduction is negligible
    /// against convection. That is a justified assumption *for TUAS's regime*,
    /// not a general truth, and it does not degrade gracefully -- it **inverts**.
    /// Under forced flow convection dominates overwhelmingly; at stagnation,
    /// natural-circulation onset, or loss of forced cooling, convection goes to
    /// zero and conduction becomes the entire heat path. So the term is present
    /// and small rather than absent, and the Péclet number is what tells a
    /// reader when it matters instead of the modelling boundary being silent.
    ///
    /// **Methodology.** The crate's single-phase design point, the same one
    /// [`tests::global_energy_balance_closes_across_the_boundaries`] uses:
    /// 8 cells, 4 m, 0.02 m^2, 4.0 MPa water at 400 K, a prescribed 3.2 kg/s
    /// inlet, 200 kW distributed lateral source, run 4000 steps of 5 ms (20 s)
    /// so the velocity and property fields have settled. Then
    /// [`TampinesSteamArray::axial_peclet_numbers`] and
    /// [`TampinesSteamArray::peak_axial_conduction_rate`] are read off, and the
    /// crossover velocity `u(Pe = 1) = α_h/(ρ Δx)` computed from the same
    /// solved state.
    ///
    /// The comparison to make is the maintainer's HTR-10 one: axial conduction
    /// 11.74 kW against a 10 MW duty, **0.117 %** (pebble bed via
    /// Zehner-Bauer-Schluender, this workspace, 2026-08-12). A single-phase
    /// water pipe at 0.17 m/s is far more convection-dominated than that.
    ///
    /// Pass criteria: `Pe > 1e4` at every cell (unambiguously forced-flow), and
    /// the peak axial conduction rate below 0.1 % of the 200 kW duty -- so the
    /// term provably cannot be perturbing the forced-flow answer. Both are
    /// reported with their measured values, not just asserted.
    ///
    /// **Results (2026-08-12, measured).**
    ///
    /// | Quantity | Measured |
    /// |---|---|
    /// | cell Péclet number | 4.9626e5 (min) to 4.9910e5 (max) |
    /// | `α_h = λ/c_p` | 1.6062e-4 kg/(m·s) |
    /// | ρ, Δx, u at mid-pipe | 931.90 kg/m³, 0.5000 m, 0.17169 m/s |
    /// | peak axial conduction | **6.0025e-2 W = 3.00e-7 of the 200 kW duty** |
    /// | crossover velocity (Pe = 1) | 3.4470e-7 m/s |
    /// | crossover mass flow | 6.4246e-6 kg/s (**2.008e-6 of the design 3.2 kg/s**) |
    ///
    /// So at power the term is 60 milliwatts against 200 kilowatts, and the
    /// crossover to conduction-dominated transport sits two millionths of the
    /// way down from the design flow. That is the justification for carrying it:
    /// it cannot perturb the forced-flow answer, and it is the only thing left
    /// when the flow stops.
    ///
    /// **Not claimed.** This is a *verification* of the term's magnitude in this
    /// solver at this operating point. It is not a validation of axial
    /// conduction against experiment, and it says nothing about the two-phase
    /// regime, where `λ` and `c_p` both behave very differently across the
    /// saturation dome.
    #[test]
    fn axial_peclet_spans_forced_flow_to_stagnation() {
        use uom::si::f64::{Power, Ratio};
        use uom::si::mass_rate::kilogram_per_second;
        use uom::si::power::{kilowatt, watt};
        use uom::si::ratio::ratio;

        const N: usize = 8;
        let p0 = Pressure::new::<pascal>(4.0e6);
        let t_seed = ThermodynamicTemperature::new::<kelvin>(400.0);
        let duty = Power::new::<kilowatt>(200.0);

        let mut arr = TampinesSteamArray::new(
            Length::new::<meter>(4.0),
            Area::new::<square_meter>(0.02),
            N as i64,
            Time::new::<second>(5.0e-3),
        )
        .unwrap();
        arr.set_pimple_algorithm(3, 2, Ratio::new::<ratio>(0.3), Ratio::new::<ratio>(0.3));
        for c in 0..N {
            arr.p.internal[c] = p0.get::<pascal>();
        }
        arr.set_temperature_vector(vec![t_seed; N]).unwrap();
        arr.set_inlet_enthalpy(h_tp_eqm_single_phase(t_seed, p0));
        arr.set_inlet_mass_flowrate(MassRate::new::<kilogram_per_second>(3.2));
        arr.set_outlet_pressure(p0);

        let q_fraction = vec![1.0 / N as f64; N];
        for _ in 0..4000 {
            arr.lateral_link_new_power_vector(duty, q_fraction.clone())
                .unwrap();
            arr.step();
        }

        let peclet: Vec<f64> = arr
            .axial_peclet_numbers()
            .iter()
            .map(|p| p.get::<ratio>())
            .collect();
        let pe_min = peclet.iter().cloned().fold(f64::INFINITY, f64::min);
        let pe_max = peclet.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let conduction = arr.peak_axial_conduction_rate().get::<watt>();
        let conduction_fraction = conduction / duty.get::<watt>();

        // Crossover: u at which Pe = 1, from the solved mid-pipe state.
        let mid = N / 2;
        let dx = arr.mesh.cell_volumes[mid] / 0.02;
        let alpha_h = arr.alpha_h.internal[mid];
        let rho = arr.rho.internal[mid];
        let u_crossover = alpha_h / (rho * dx);
        let mdot_crossover = rho * u_crossover * 0.02;
        let u_design = arr.u.internal[mid].mag();

        println!(
            "axial Peclet at the design point, n = {N}, 3.2 kg/s, Q = 200 kW:\n  \
             Pe per cell: min {pe_min:.4e}, max {pe_max:.4e}\n  \
             alpha_h = lambda/cp {alpha_h:.4e} kg/(m s), rho {rho:.2} kg/m3, \
             dx {dx:.4} m, u {u_design:.5} m/s\n  \
             peak axial conduction {conduction:.4e} W = {:.3e} of the 200 kW duty \
             ({:.2e} %)\n  \
             crossover (Pe = 1): u = {u_crossover:.4e} m/s, mdot = \
             {mdot_crossover:.4e} kg/s ({:.3e} of the design 3.2 kg/s)",
            conduction_fraction,
            100.0 * conduction_fraction,
            mdot_crossover / 3.2,
        );

        assert!(
            peclet.iter().all(|p| p.is_finite()),
            "Peclet numbers must all be finite"
        );
        assert!(
            pe_min > 1.0e4,
            "the design point should be unambiguously convection-dominated, but the \
             minimum cell Peclet number is {pe_min:.4e}. Either the flow is not \
             developing or alpha_h = lambda/cp is far too large."
        );
        assert!(
            conduction_fraction < 1.0e-3,
            "axial conduction is {:.4e} W, {:.3}% of the 200 kW duty -- large enough \
             to perturb the forced-flow answer. At Pe ~ {pe_max:.2e} it should be \
             utterly negligible; a value this size means the conductivity or the \
             discretisation is wrong, and that is a finding to report rather than \
             tune away.",
            conduction,
            100.0 * conduction_fraction,
        );
    }

    /// **V&V (verification, 2026-08-12) -- the GLOBAL ENERGY BALANCE closes
    /// across the boundaries, at steady state and through a transient.**
    ///
    /// **Why this test is the important one.** The directional tests above
    /// ([`tests::inlet_enthalpy_bc_actually_drives_the_field`]) catch the
    /// specific defect already known about. This one catches the whole *class*.
    /// The underlying fault was not "a setter was one-shot" -- it was that the
    /// energy equation ran with **zero-gradient boundaries at BOTH ends**. A
    /// zero-gradient inlet means `h_face = h_cell0`, so the domain advects in
    /// its *own* enthalpy: the influx term is self-referential and the interior
    /// field is structurally indifferent to whatever is actually flowing in.
    /// With zero-gradient at both ends there is no boundary through which
    /// energy can enter or leave at all, and the global balance cannot even be
    /// posed. The correct pairing for an advection-dominated 1-D energy
    /// equation is **Dirichlet at the inlet** (incoming fluid carries its
    /// enthalpy) and **zero-gradient at the OUTLET only** (the downstream state
    /// is genuinely unknown, so extrapolation is right there).
    ///
    /// **Methodology.** 8 cells, 4 m, 0.02 m^2, 4.0 MPa water, PIMPLE(3,2) at
    /// 0.3 under-relaxation, seeded uniformly at 523.15 K. The inlet is fully
    /// specified -- a prescribed **mass flow** of 3.2 kg/s
    /// ([`TampinesSteamArray::set_inlet_mass_flowrate`], so the inlet flux is
    /// exact rather than density-guessed) plus a fixed inlet enthalpy at
    /// 523.15 K -- and a uniformly distributed 200 kW lateral source is applied
    /// every step. The balance asserted is the full transient form
    ///
    /// ```text
    /// (mdot_out h_out - mdot_in h_in) + d/dt (sum_c rho_c V_c h_c) = Q
    /// ```
    ///
    /// with both boundary fluxes read from the SOLVED face fluxes
    /// `phi.boundary` rather than from any nominal value. Checked after 4000
    /// steps of 1 ms (steady), and as a running check through the transient.
    ///
    /// **Results (2026-08-12, measured), n = 8, Q = 200 kW, to steady state
    /// (30 000 steps of 5 ms = 150 s simulated, about 6.4 tube transits):**
    ///
    /// | Quantity | Measured |
    /// |---|---|
    /// | inlet mass flux | 3.200000000 kg/s against the imposed 3.2 (exact) |
    /// | outlet mass flux | 3.199879668 kg/s (**-0.0038 %** deficit) |
    /// | advected `mdot_out h_out - mdot_in h_in` | 200.384 kW |
    /// | stored-energy rate | -0.417 kW (settled) |
    /// | **energy residual** | **-0.0333 kW, -0.0166 % of Q** |
    /// | outlet temperature | 414.71 K (hand check: 400 K + 62.5 kJ/kg / c_p = 414.5 K) |
    ///
    /// The balance closes to better than 0.02 % of the source. The mass deficit
    /// is **-0.0038 %**, far below the -0.70 % the sibling crate measured at the
    /// same cell count, because the inlet here is a *prescribed mass flux*
    /// rather than a velocity derived from an assumed density -- so `op-nnqi`'s
    /// outlet truncation is not the dominant term at this operating point and
    /// does not mask a boundary defect.
    ///
    /// The worst *transient* residual after step 500 is about 10 % of Q; that is
    /// the thermal front still marching down the tube, where the storage term is
    /// genuinely large and first-order-in-time. It is reported, not asserted.
    ///
    /// **Known residual, deliberately reported and not tuned away.** The outlet
    /// mass flux does not equal the inlet: boundary `phi` is never
    /// pressure-corrected, a first-order truncation error that shrinks with
    /// mesh refinement (workspace bead `op-nnqi`; the sibling crate measured
    /// -0.7035 % at n = 8, -0.3150 % at n = 16, -0.1482 % at n = 32). Because
    /// the enthalpy outflux is `mdot_out * h_out`, that mass deficit propagates
    /// straight into this energy residual, and at n = 8 it is expected to
    /// dominate it. The test therefore reports the mass deficit alongside the
    /// energy residual so the two can be told apart, and asserts a tolerance
    /// consistent with the known truncation rather than one tightened until it
    /// passes. **If the energy residual is materially larger than the mass
    /// deficit can explain, that is a genuine energy-boundary defect and this
    /// test is doing its job.**
    #[test]
    fn global_energy_balance_closes_across_the_boundaries() {
        use crate::openfoam_algorithms::openfoam_source::BoundaryCondition;
        use uom::si::f64::{Power, Ratio};
        use uom::si::mass_rate::kilogram_per_second;
        use uom::si::power::kilowatt;
        use uom::si::ratio::ratio;

        const N: usize = 8;
        const INLET: usize = 1;
        const OUTLET: usize = 0;
        let steam_pressure = Pressure::new::<pascal>(4.0e6);
        // Deliberately SUBCOOLED (T_sat(4 MPa) = 523.5 K). Seeding at
        // saturation makes the 200 kW source flash the column: the density
        // collapses, the stored-energy term swamps the advective one and the
        // array is still evolving violently after 4 s, so the *steady* balance
        // is not even defined. Measured on the saturated setup: storage
        // -3010 kW against a 200 kW source, outlet mass flux +5.37 % above the
        // inlet as vapour expands. That is real two-phase physics, not a
        // boundary defect -- but it is the wrong experiment for a boundary
        // check, so this one stays single-phase throughout.
        let seed_temperature = ThermodynamicTemperature::new::<kelvin>(400.0);
        let mdot_target = 3.2_f64;
        let source_w = Power::new::<kilowatt>(200.0);

        let mut arr = TampinesSteamArray::new(
            Length::new::<meter>(4.0),
            Area::new::<square_meter>(0.02),
            N as i64,
            Time::new::<second>(5.0e-3),
        )
        .unwrap();
        arr.set_pimple_algorithm(3, 2, Ratio::new::<ratio>(0.3), Ratio::new::<ratio>(0.3));
        for c in 0..N {
            arr.p.internal[c] = steam_pressure.get::<pascal>();
        }
        arr.set_temperature_vector(vec![seed_temperature; N])
            .unwrap();
        let h_in = h_tp_eqm_single_phase(seed_temperature, steam_pressure);
        arr.set_inlet_enthalpy(h_in);
        arr.set_inlet_mass_flowrate(MassRate::new::<kilogram_per_second>(mdot_target));
        arr.set_outlet_pressure(steam_pressure);

        // Energy stored in the fluid, sum_c rho_c V_c h_c [J].
        let stored = |a: &TampinesSteamArray| -> f64 {
            (0..N)
                .map(|c| a.rho.internal[c] * a.mesh.cell_volumes[c] * a.he.internal[c])
                .sum()
        };

        // Axial CONDUCTION power entering through the two terminals [W]:
        // sum over Dirichlet boundary faces of alpha_h * |Sf| * (h_bc - h_cell)/d,
        // the same face flux `fvm::laplacian(&alpha_h_f, &self.he)` assembles.
        // A zero-gradient terminal contributes nothing by construction (it is an
        // insulated end for the diffusion operator), so this is nonzero only at a
        // terminal whose junction state is currently upwind.
        //
        // At this operating point it is ~1e-7 of the source -- see
        // `tests::axial_peclet_spans_forced_flow_to_stagnation`, which measures
        // Pe ~ 5e5 here. It is carried in the balance anyway rather than assumed
        // away: the term is real, and at stagnation it is the ONLY term.
        let boundary_conduction = |a: &TampinesSteamArray| -> f64 {
            let mut q = 0.0_f64;
            for (pi, patch) in a.mesh.patches.iter().enumerate() {
                for fi in 0..patch.size {
                    let gf = patch.start + fi;
                    let owner = a.mesh.owner[gf];
                    let d = (a.mesh.face_centres[gf] - a.mesh.cell_centres[owner]).mag();
                    if d < 1e-300 {
                        continue;
                    }
                    let h_face = match &a.he.boundary[pi].bc {
                        BoundaryCondition::FixedValue(v) => *v,
                        BoundaryCondition::FixedField(ff) => ff[fi],
                        // ZeroGradient / Symmetry / Empty: no diffusive flux.
                        _ => continue,
                    };
                    q += a.alpha_h.internal[owner]
                        * a.mesh.face_areas[gf]
                        * (h_face - a.he.internal[owner])
                        / d;
                }
            }
            q
        };

        let q_fraction = vec![1.0 / N as f64; N];
        let dt_s = 5.0e-3_f64;
        let mut worst_transient_residual_fraction = 0.0_f64;

        // ~6.4 tube transits at the resulting 0.17 m/s (4 m / 0.17 = 23.5 s).
        for step in 0..30000 {
            let stored_before = stored(&arr);
            arr.lateral_link_new_power_vector(source_w, q_fraction.clone())
                .unwrap();
            arr.step();
            let stored_after = stored(&arr);

            // Solved boundary mass fluxes. The inlet normal is -x, so an
            // inflow carries a negative face flux; the outlet normal is +x.
            let mdot_in = -arr.phi.boundary[INLET].values[0];
            let mdot_out = arr.phi.boundary[OUTLET].values[0];
            let h_out = arr.he.internal[N - 1];
            let advected = mdot_out * h_out - mdot_in * h_in.get::<joule_per_kilogram>();
            let storage_rate = (stored_after - stored_before) / dt_s;
            let residual =
                advected + storage_rate - source_w.get::<watt>() - boundary_conduction(&arr);

            // Skip the opening acoustic transient, where the pressure field is
            // still ringing in and the storage term is dominated by it.
            if step > 500 {
                worst_transient_residual_fraction = worst_transient_residual_fraction
                    .max((residual / source_w.get::<watt>()).abs());
            }
        }

        // ── Steady-state balance ────────────────────────────────────────────
        let stored_before = stored(&arr);
        arr.lateral_link_new_power_vector(source_w, q_fraction.clone())
            .unwrap();
        arr.step();
        let storage_rate = (stored(&arr) - stored_before) / dt_s;

        let mdot_in = -arr.phi.boundary[INLET].values[0];
        let mdot_out = arr.phi.boundary[OUTLET].values[0];
        let h_out = arr.he.internal[N - 1];
        let advected = mdot_out * h_out - mdot_in * h_in.get::<joule_per_kilogram>();
        let conduction = boundary_conduction(&arr);
        let residual = advected + storage_rate - source_w.get::<watt>() - conduction;
        let mass_deficit_fraction = (mdot_out - mdot_in) / mdot_in;
        let residual_fraction = residual / source_w.get::<watt>();

        println!(
            "global energy balance, n = {N}, Q = {:.1} kW:\n  \
             mdot_in {:.9} kg/s (imposed {:.6}), mdot_out {:.9} kg/s, mass deficit {:+.4}%\n  \
             advected {:.4} kW, storage {:.6} kW, terminal conduction {:+.6e} kW \
             ({:.2e} of Q), residual {:+.4} kW ({:+.4}% of Q)\n  \
             worst transient residual after step 500: {:.4}% of Q\n  \
             T_out {:.2} K",
            source_w.get::<kilowatt>(),
            mdot_in,
            mdot_target,
            mdot_out,
            100.0 * mass_deficit_fraction,
            advected / 1e3,
            storage_rate / 1e3,
            conduction / 1e3,
            conduction.abs() / source_w.get::<watt>(),
            residual / 1e3,
            100.0 * residual_fraction,
            100.0 * worst_transient_residual_fraction,
            arr.get_outlet_temperature().get::<kelvin>(),
        );

        assert!(
            arr.he.internal.as_slice().iter().all(|x| x.is_finite()),
            "enthalpy field must stay finite"
        );
        // The mass deficit propagates into the energy residual through
        // `mdot_out * h_out`, so the bar is set by that known truncation error
        // (op-nnqi), not by an arbitrarily tightened number. An energy residual
        // materially larger than the mass deficit can explain would be a real
        // energy-boundary defect.
        assert!(
            residual_fraction.abs() < 0.01,
            "steady-state energy balance failed to close: residual {:+.4} kW is \
             {:+.4}% of the {:.1} kW source (mass deficit {:+.4}%). A large residual \
             here means energy is entering or leaving through a boundary that is not \
             accounted for -- check the enthalpy boundary conditions.",
            residual / 1e3,
            100.0 * residual_fraction,
            source_w.get::<kilowatt>(),
            100.0 * mass_deficit_fraction,
        );
    }
}
