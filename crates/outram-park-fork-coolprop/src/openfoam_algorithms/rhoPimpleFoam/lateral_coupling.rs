// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! Lateral (radial) thermal coupling, volumetric heat source, pipe geometry
//! and flow bookkeeping for [`super::OPCPFluidArray`].
//!
//! Mirrors a subset of `tuas_boussinesq_solver`'s `FluidArray` interface (see
//! `one_d_fluid_array_with_lateral_coupling/lateral_connection.rs`) so a
//! caller (e.g. a future TAMPINES `Pipe` component) can drive either backend
//! through a comparable API. Simplified relative to TUAS: the caller supplies
//! a thermal conductance / pressure loss directly — there is no
//! `NusseltCorrelation` or `DimensionlessDarcyLossCorrelations` port here.
//!
//! Also provides prescribed inlet/outlet boundary-condition setters/getters
//! ([`Self::set_inlet_velocity`], [`Self::set_inlet_enthalpy`],
//! [`Self::set_outlet_pressure`], [`Self::get_outlet_pressure`],
//! [`Self::get_outlet_enthalpy`], [`Self::get_outlet_temperature`]) so a
//! caller can drive this array as a simple pipe/tube (known inlet flow +
//! thermodynamic state, known downstream pressure) without reaching into the
//! internal `PatchField`/`Vector3`/mesh-patch-index representation directly.
//! Kept byte-for-byte parallel with `tampines-steam-tables`'s
//! `TampinesSteamArray` equivalents, since both drive the same rhoPimpleFoam
//! port.

use uom::si::f64::{
    Angle, AvailableEnergy, Length, MassRate, Power, Pressure, ThermalConductance,
    ThermodynamicTemperature, Velocity,
};
use uom::si::available_energy::joule_per_kilogram;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::velocity::meter_per_second;

use crate::flash;
use crate::openfoam_algorithms::openfoam_source::{BoundaryCondition, PatchField, Vector3};

use super::OPCPFluidArray;

/// Errors from [`OPCPFluidArray`]'s lateral-coupling / bookkeeping interface.
#[derive(Debug, Clone, PartialEq)]
pub enum OPCPFluidArrayError {
    /// A per-cell vector argument did not have length `mesh.n_cells`.
    LengthMismatch {
        /// Name of the offending argument.
        array: &'static str,
        /// Length the argument was required to have (`mesh.n_cells`).
        expected: usize,
        /// Length the argument actually had.
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

impl std::fmt::Display for OPCPFluidArrayError {
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

impl std::error::Error for OPCPFluidArrayError {}

impl OPCPFluidArray {
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
    ) -> Result<(), OPCPFluidArrayError> {
        let n = self.mesh.n_cells;
        if temperature_vec.len() != n {
            return Err(OPCPFluidArrayError::LengthMismatch {
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
    ) -> Result<(), OPCPFluidArrayError> {
        let n = self.mesh.n_cells;
        if q_fraction_vec.len() != n {
            return Err(OPCPFluidArrayError::LengthMismatch {
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
    /// Called once at the end of [`Self::step`] (matches TUAS's
    /// `advance_timestep_with_mass_flowrate`, which calls its own
    /// `clear_vectors` at the very end) — links/sources are per-timestep
    /// registrations, not persistent state.
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

    /// Bulk mass flowrate \[kg/s\] (plain storage — see the field's doc comment
    /// on [`OPCPFluidArray`] for why `step()` does not read this).
    pub fn get_mass_flowrate(&self) -> MassRate {
        self.mass_flowrate
    }

    /// Set the bulk mass flowrate \[kg/s\] — **bookkeeping only; this does not
    /// impose any flow.**
    ///
    /// The value is recorded in [`OPCPFluidArray::mass_flowrate`] for a
    /// caller-driven pipe-network layer and is never read by
    /// [`super::OPCPFluidArray::step`], which solves for its own mass flux
    /// `phi` from the momentum and pressure equations.
    ///
    /// To actually **impose** a mass flow at the inlet, call
    /// [`Self::set_inlet_mass_flowrate`] instead; to read back the flow the
    /// solver produced, call [`Self::get_inlet_mass_flowrate_actual`] /
    /// [`Self::get_outlet_mass_flowrate_actual`].
    pub fn set_mass_flowrate(&mut self, mass_flowrate: MassRate) {
        self.mass_flowrate = mass_flowrate;
    }

    /// Prescribes a **mass-flow inlet** on the `"left"` patch (x = 0): a real
    /// boundary condition, in contrast to the bookkeeping-only
    /// [`Self::set_mass_flowrate`].
    ///
    /// Each pressure corrector of [`super::OPCPFluidArray::step`] re-derives
    /// the fixed inlet velocity `u_in = ṁ / (ρ_inlet · A_inlet)` from the
    /// **current** inlet-face density, so the mass flux actually crossing the
    /// inlet stays equal to `mass_flowrate` as the solution's density evolves.
    /// This is OpenFOAM's `flowRateInletVelocity`
    /// (`src/finiteVolume/fields/fvPatchFields/derived/flowRateInletVelocity/`)
    /// applied to this 1-D array. Doing the conversion once outside the solver
    /// — pick an assumed density, divide, and call [`Self::set_inlet_velocity`]
    /// — instead imposes `ṁ_actual = (ρ_solved/ρ_assumed)·ṁ_target`, which
    /// drifts away from the target by exactly the density error.
    ///
    /// ## Units and sign
    /// `mass_flowrate` is in kg/s; **positive is into the domain** (+x). A
    /// negative value prescribes outflow through the inlet patch, in which case
    /// a fixed inlet enthalpy ([`Self::set_inlet_enthalpy`]) is no longer a
    /// meaningful boundary condition (a Dirichlet value on an outflow face) —
    /// nothing here stops you, but do not rely on it.
    ///
    /// ## Interaction with [`Self::set_inlet_velocity`]
    /// The two prescribe the same patch and are mutually exclusive: this
    /// installs the mass-flow inlet (overriding any fixed inlet velocity), and
    /// [`Self::set_inlet_velocity`] clears it again.
    ///
    /// ## Caveat
    /// Imposing a mass flow does **not** by itself close the system: pair it
    /// with a pressure boundary condition downstream
    /// ([`Self::set_outlet_pressure`]), exactly as for a prescribed inlet
    /// velocity.
    pub fn set_inlet_mass_flowrate(&mut self, mass_flowrate: MassRate) {
        self.inlet_mass_flowrate = Some(mass_flowrate);
    }

    /// The prescribed inlet mass flowrate \[kg/s\], or `None` if no mass-flow
    /// inlet is imposed. See [`Self::set_inlet_mass_flowrate`].
    pub fn get_inlet_mass_flowrate(&self) -> Option<MassRate> {
        self.inlet_mass_flowrate
    }

    /// Removes any prescribed mass-flow inlet, leaving the inlet velocity
    /// boundary condition at whatever value the last corrector derived. Pair
    /// with [`Self::set_inlet_velocity`] if a specific velocity is wanted
    /// afterwards (that setter clears the mass-flow inlet by itself).
    pub fn clear_inlet_mass_flowrate(&mut self) {
        self.inlet_mass_flowrate = None;
    }

    /// The mass flowrate \[kg/s\] **actually** crossing the inlet (`"left"`)
    /// patch in the most recent [`super::OPCPFluidArray::step`], read off the
    /// solved face mass flux `phi`. Positive means flow **into** the domain.
    ///
    /// This is the diagnostic to check a prescribed flow against: with a
    /// mass-flow inlet ([`Self::set_inlet_mass_flowrate`]) it should reproduce
    /// the prescribed value to round-off, and with a velocity inlet derived
    /// from an assumed density it shows the drift that assumption introduces.
    /// Zero before the first step (`phi` starts at zero).
    pub fn get_inlet_mass_flowrate_actual(&self) -> MassRate {
        let patch = &self.mesh.patches[super::INLET_PATCH];
        let sum: f64 = (0..patch.size)
            .map(|fi| self.phi.boundary[super::INLET_PATCH].values[fi])
            .sum();
        // `phi` is positive *out of* the domain, so an inflow is negative.
        MassRate::new::<kilogram_per_second>(-sum)
    }

    /// The mass flowrate \[kg/s\] leaving the outlet (`"right"`) patch in the
    /// most recent [`super::OPCPFluidArray::step`]. Positive means flow **out
    /// of** the domain.
    ///
    /// Compared against [`Self::get_inlet_mass_flowrate_actual`] this is the
    /// array's **global mass balance**, and at steady state the two agree to
    /// round-off: measured 2026-08-12 on the 8-cell heated fixture,
    /// `ṁ_out = 1.000000000×10⁻³ kg/s` against an exactly-imposed
    /// `1.0×10⁻³ kg/s` inlet (−0.0000 %), and likewise at 16 and 32 cells.
    ///
    /// **History.** Until 2026-08-12 this returned the *predictor* flux
    /// `φ_HbyA`, because [`super::OPCPFluidArray::step`] applied the pressure
    /// correction `−ρ_f·rAU_f·snGrad(p)·|Sf|` to internal faces only; a
    /// fixed-pressure outlet therefore reported a pre-correction value and the
    /// steady mass balance was out by −0.7035 % at 8 cells, −0.3150 % at 16 and
    /// −0.1482 % at 32. Applying that correction to boundary faces as well —
    /// which is what OpenFOAM's `phi = phiHbyA - pEqn.flux()` does — removed it
    /// (bead op-nnqi). The correction is self-selecting: `snGrad` is zero on a
    /// zero-gradient pressure patch, so a prescribed-velocity or mass-flow
    /// inlet is untouched.
    ///
    /// **Remaining caveat, transient only.** A tight *boundary* balance does not
    /// make the array's stored mass conserved: under strong volumetric heating
    /// `Σ ρ·V` falls by more than the boundaries account for, because the
    /// pressure equation's compressibility term carries `∂ρ/∂p|_h` but no
    /// `∂ρ/∂h|_p`. See
    /// `tests::transient_energy_imbalance_is_the_enthalpy_of_the_unconserved_mass`
    /// for the measurement and the ruled-out alternatives.
    pub fn get_outlet_mass_flowrate_actual(&self) -> MassRate {
        let patch = &self.mesh.patches[super::OUTLET_PATCH];
        let sum: f64 = (0..patch.size)
            .map(|fi| self.phi.boundary[super::OUTLET_PATCH].values[fi])
            .sum();
        MassRate::new::<kilogram_per_second>(sum)
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

    /// Overwrite the per-cell temperature at the current pressure.
    ///
    /// Does a per-cell `(p, T)` flash and writes `he`/`rho`/`t`/`psi` together
    /// — **not** a plain `t` field write. `he` (specific enthalpy) is the
    /// actual PIMPLE state variable; writing `t` alone would be silently
    /// undone by the next [`Self::correct_thermo`] `(p, h)` flash. If a
    /// cell's `(p, T)` does not converge to a single-phase state, that
    /// cell's fields are left untouched (mirrors `correct_thermo`'s own
    /// error handling — never a wrong number).
    pub fn set_temperature_vector(
        &mut self,
        temperature_vec: Vec<ThermodynamicTemperature>,
    ) -> Result<(), OPCPFluidArrayError> {
        let n = self.mesh.n_cells;
        if temperature_vec.len() != n {
            return Err(OPCPFluidArrayError::LengthMismatch {
                array: "temperature_vec",
                expected: n,
                got: temperature_vec.len(),
            });
        }
        for c in 0..n {
            let p_c = self.p.internal[c];
            let t_c = temperature_vec[c].get::<kelvin>();
            if let Ok(state) = flash::state_pt(self.fluid, t_c, p_c) {
                self.rho.internal[c] = state.density.max(1e-4);
                self.he.internal[c] = state.enthalpy;
                self.t.internal[c] = state.temperature;
                self.psi.internal[c] =
                    flash::drho_dp_t(self.fluid, state.temperature, state.density).max(1e-12);
            }
        }
        Ok(())
    }

    /// Total volumetric heat-source power \[W\] distributed into cell `c` from
    /// every registered source, using each source's `q_fraction_vector` entry.
    ///
    /// Used by [`super::OPCPFluidArray::step`] to add an explicit source term
    /// to the energy equation, alongside the existing `conv_he`/`dp_dt` terms.
    pub(super) fn cell_heat_source_power(&self, c: usize) -> Power {
        let mut q = Power::new::<watt>(0.0);
        for (source_idx, fractions) in self.q_fraction_vector.iter().enumerate() {
            q += self.q_vector[source_idx] * fractions[c];
        }
        q
    }

    /// Prescribes a fixed inlet velocity boundary condition on the
    /// `"left"` patch (x = 0, see [`crate::openfoam_algorithms::openfoam_source::interface::one_dimensional_meshing::create_one_d_mesh`]).
    ///
    /// For driving this array as a simple pipe/tube with a known inlet
    /// flow (e.g. from an upstream pump). `velocity` is the x-direction
    /// flow speed; positive means fluid entering the domain (flowing
    /// left-to-right, +x) -- takes effect on the next [`super::OPCPFluidArray::step`]
    /// and persists across steps (the BC template is re-stamped inside
    /// `step`).
    ///
    /// **Clears any prescribed mass-flow inlet** ([`Self::set_inlet_mass_flowrate`]):
    /// the two prescribe the same patch, and a velocity fixed here would
    /// otherwise be silently overwritten by the flow-rate inlet on the next
    /// corrector. If what you want is a mass flow, prescribe it directly rather
    /// than converting it to a velocity with an assumed density — see that
    /// method for why the conversion drifts.
    pub fn set_inlet_velocity(&mut self, velocity: Velocity) {
        let size = self.mesh.patches[super::INLET_PATCH].size;
        let v = Vector3::new(velocity.get::<meter_per_second>(), 0.0, 0.0);
        self.u.boundary[super::INLET_PATCH] = PatchField::fixed_value_vec(size, v);
        self.inlet_mass_flowrate = None;
    }

    /// Prescribes the specific enthalpy carried by fluid **entering** through
    /// the `"left"` end (x = 0) -- the upstream state of that end when it acts
    /// as an inlet. Pairs with [`Self::set_inlet_velocity`] (or
    /// [`Self::set_inlet_mass_flowrate`]) to fully specify the inlet
    /// thermodynamic state.
    ///
    /// ## This is an advection terminal, not a Dirichlet patch
    /// These arrays are **pipes**: their ends are junctions with a flow network,
    /// not patches on a standalone domain. The value set here is therefore used
    /// **only while fluid is flowing in through this end**, and is ignored when
    /// the flow reverses and the end becomes an outlet — the same
    /// direction-switched upwind rule as `tuas_boussinesq_solver`'s
    /// `single_control_vol/boundary_condition_interactions/advection_to_bcs.rs`
    /// (and OpenFOAM's `inletOutlet`). See
    /// `openfoam_source::fv_operators::fvc::div_limited`'s boundary section for
    /// the full statement. Use [`Self::set_outlet_enthalpy`] to give the other
    /// end its own upstream state for the reversed case.
    ///
    /// The boundary condition **persists across steps**: it enters the energy
    /// equation through the convective inflow `∇·(φh)` and through the enthalpy
    /// diffusion term, and [`super::OPCPFluidArray::step`] re-stamps the BC
    /// template after the energy solve (which, like every linear solve here,
    /// returns a zero-gradient-bounded field). Before that re-stamp was added it
    /// was a **one-shot**: measured 2026-08-12 over 10 000 steps of 2×10⁻⁴ s
    /// (Nitrogen, 8 cells, 1 m, 0.5 m/s, seed 300 K = 311.20 kJ/kg, inlet BC
    /// 520.45 kJ/kg), no cell moved more than 0.4 kJ/kg from the seed unless the
    /// caller re-issued this call every step. See the module tests
    /// `inlet_enthalpy_bc_*_without_reapplying_each_step`.
    ///
    /// ## Units
    /// `h` is a specific enthalpy \[J/kg\] on the same reference scale as the
    /// [`crate::flash`] EOS -- take it from a flash of the intended inlet state
    /// rather than assuming a reference point.
    pub fn set_inlet_enthalpy(&mut self, h: AvailableEnergy) {
        let size = self.mesh.patches[super::INLET_PATCH].size;
        self.he.boundary[super::INLET_PATCH] =
            PatchField::fixed_value(size, h.get::<joule_per_kilogram>());
    }

    /// Prescribes the specific enthalpy carried by fluid **entering** through
    /// the `"right"` end (x = length) — i.e. the upstream state to use *if the
    /// flow reverses* and that end becomes an inlet. The mirror of
    /// [`Self::set_inlet_enthalpy`].
    ///
    /// While flow leaves through this end (the normal case) the value set here
    /// is **ignored**, and the exported enthalpy is the interior cell's own —
    /// the domain chooses what it exports. Only when the boundary mass flux
    /// turns negative does this value enter the energy equation. Same
    /// direction-switched upwind rule as
    /// `tuas_boussinesq_solver`'s `advection_to_bcs.rs`; see
    /// `openfoam_source::fv_operators::fvc::div_limited`.
    ///
    /// If it is never called, the right end keeps its default zero-gradient
    /// terminal: on reversal it would advect the domain's own last-cell
    /// enthalpy back in (TUAS's "non-set-temperature" advection, a legitimate
    /// but self-referential mode). **Set it whenever a reversal is physically
    /// possible and you know the upstream state** — for a pipe in a loop, that
    /// is the enthalpy of whatever component sits downstream.
    ///
    /// ## Units
    /// `h` is a specific enthalpy \[J/kg\] on the [`crate::flash`] EOS's
    /// reference scale, as for [`Self::set_inlet_enthalpy`].
    pub fn set_outlet_enthalpy(&mut self, h: AvailableEnergy) {
        let size = self.mesh.patches[super::OUTLET_PATCH].size;
        self.he.boundary[super::OUTLET_PATCH] =
            PatchField::fixed_value(size, h.get::<joule_per_kilogram>());
    }

    /// The specific enthalpy the fluid actually carries **across the `"left"`
    /// end** in the state left by the most recent [`super::OPCPFluidArray::step`]
    /// — i.e. the upstream value the direction-switched advection terminal
    /// selected, whichever side that turned out to be.
    ///
    /// On inflow (`φ < 0`) this is the value given to
    /// [`Self::set_inlet_enthalpy`] (or the extrapolated cell value if none was
    /// given); on outflow it is cell 0's own enthalpy. Together with
    /// [`Self::get_inlet_mass_flowrate_actual`] it gives the enthalpy flow
    /// through that end, which is what a network-level energy balance needs.
    pub fn get_inlet_advected_enthalpy(&self) -> AvailableEnergy {
        AvailableEnergy::new::<joule_per_kilogram>(self.advected_enthalpy(super::INLET_PATCH, 0))
    }

    /// The specific enthalpy the fluid actually carries **across the `"right"`
    /// end** in the state left by the most recent
    /// [`super::OPCPFluidArray::step`] — the mirror of
    /// [`Self::get_inlet_advected_enthalpy`]. On the normal outflow direction
    /// this is the last cell's own enthalpy.
    pub fn get_outlet_advected_enthalpy(&self) -> AvailableEnergy {
        let n = self.mesh.n_cells;
        AvailableEnergy::new::<joule_per_kilogram>(
            self.advected_enthalpy(super::OUTLET_PATCH, n - 1),
        )
    }

    /// Shared implementation of [`Self::get_inlet_advected_enthalpy`] /
    /// [`Self::get_outlet_advected_enthalpy`]: reproduce the upwind advection
    /// terminal's choice for patch `patch_idx`, whose owner cell is `owner`.
    /// `phi` is positive out of the domain, so a negative boundary flux is
    /// inflow and takes the boundary-side state.
    fn advected_enthalpy(&self, patch_idx: usize, owner: usize) -> f64 {
        let patch = &self.mesh.patches[patch_idx];
        if patch.size == 0 {
            return self.he.internal[owner];
        }
        let phi_b: f64 = (0..patch.size)
            .map(|fi| self.phi.boundary[patch_idx].values[fi])
            .sum();
        if phi_b >= 0.0 {
            self.he.internal[owner]
        } else {
            match self.he.boundary[patch_idx].bc {
                BoundaryCondition::FixedValue(v) => v,
                _ => self.he.internal[owner],
            }
        }
    }

    /// Prescribes a fixed outlet pressure boundary condition on the
    /// `"right"` patch (x = length) -- e.g. the downstream pressure a
    /// turbine or condenser imposes.
    pub fn set_outlet_pressure(&mut self, p: Pressure) {
        let size = self.mesh.patches[super::OUTLET_PATCH].size;
        self.p.boundary[super::OUTLET_PATCH] = PatchField::fixed_value(size, p.get::<pascal>());
    }

    /// Outlet-cell (the last cell, owner of the `"right"` patch) pressure
    /// -- for a caller reading the downstream state after [`super::OPCPFluidArray::step`].
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
    use crate::fluid::Fluid;
    use super::super::{EnergyBalanceMode, EnergyConvectionScheme};
    use uom::si::area::square_meter;
    use uom::si::length::meter;
    use uom::si::mass_rate::kilogram_per_second;
    use uom::si::pressure::pascal;
    use uom::si::thermal_conductance::watt_per_kelvin;
    use uom::si::time::second;

    fn test_array(n: i64) -> OPCPFluidArray {
        OPCPFluidArray::new(
            Fluid::Nitrogen,
            Length::new::<meter>(1.0),
            uom::si::f64::Area::new::<square_meter>(1.0e-4),
            n,
            uom::si::f64::Time::new::<second>(0.01),
        )
        .unwrap()
    }

    /// ## Methodology
    /// Drives the array as a simple gas pipe with a prescribed inlet
    /// velocity + enthalpy and a prescribed outlet pressure -- the
    /// `OPCPFluidArray` counterpart of `tampines-steam-tables`'s
    /// `TampinesSteamArray` BC-driven regression test, exercising the same
    /// (shared-design) rhoPimpleFoam pressure-velocity coupling under a
    /// real Dirichlet velocity BC. Working fluid is **Nitrogen at
    /// 300 K / 1 bar (a gas)**, deliberately -- a compressible gas has a
    /// far larger compressibility ψ = (∂ρ/∂p)_T than liquid water, so it
    /// tolerates a brisk 0.5 m/s impulsive inlet start (a near-
    /// incompressible liquid would water-hammer; that is why the
    /// `TampinesSteamArray` sibling test uses a gentle 0.02 m/s inlet).
    /// Transient PISO at the gas acoustic-CFL timestep.
    ///
    /// ## Result (2026-07-14)
    /// Passes: fields stay finite over 500 steps, the outlet cell pressure
    /// settles near the imposed 1 bar BC, and the flow moves in +x. This
    /// depends on the pressure-boundary-source fix in `step()` (the
    /// `p_eqn.source[c] += source_p[c]` note there): before it, overwriting
    /// the pressure equation's source dropped the FixedValue-outlet
    /// Dirichlet term while keeping its diagonal, silently imposing
    /// `p_outlet = 0` and blowing the field up within ~10 steps even for
    /// this well-conditioned gas case and even from a uniform equilibrium
    /// field. See the workspace beads tracker (`op-21g.12`).
    #[test]
    fn inlet_outlet_bcs_drive_gas_flow_and_outlet_pressure_settles() {
        use uom::si::velocity::meter_per_second;

        let mut arr = OPCPFluidArray::new(
            Fluid::Nitrogen,
            Length::new::<meter>(1.0),
            uom::si::f64::Area::new::<square_meter>(1.0e-4),
            10,
            uom::si::f64::Time::new::<second>(2.0e-4), // acoustic CFL: dx/c ~ 0.1/350 ~ 2.9e-4 s for N2 gas
        )
        .unwrap();
        arr.set_piso_algorithm(2);

        // Pre-initialize near the operating point (300 K, 1 bar gas).
        let outlet_pressure = Pressure::new::<pascal>(1.0e5);
        for c in 0..10 {
            arr.p.internal[c] = outlet_pressure.get::<pascal>();
        }
        let preset_temp = ThermodynamicTemperature::new::<kelvin>(300.0);
        arr.set_temperature_vector(vec![preset_temp; 10]).unwrap();

        let inlet_state = flash::state_pt(Fluid::Nitrogen, 300.0, 1.0e5).unwrap();
        arr.set_inlet_velocity(Velocity::new::<meter_per_second>(0.5));
        arr.set_inlet_enthalpy(AvailableEnergy::new::<joule_per_kilogram>(
            inlet_state.enthalpy,
        ));
        arr.set_outlet_pressure(outlet_pressure);

        arr.run(500);

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

        let outlet_p = arr.get_outlet_pressure();
        assert!(
            (outlet_p.get::<pascal>() - outlet_pressure.get::<pascal>()).abs() < 5.0e3,
            "outlet pressure {} Pa should be close to the imposed BC {} Pa",
            outlet_p.get::<pascal>(),
            outlet_pressure.get::<pascal>()
        );

        let mean_u_x: f64 =
            arr.u.internal.as_slice().iter().map(|v| v.x).sum::<f64>() / arr.mesh.n_cells as f64;
        assert!(
            mean_u_x > 0.0,
            "flow should move in +x, driven by the inlet velocity BC; got mean u_x = {mean_u_x}"
        );
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
            Err(OPCPFluidArrayError::LengthMismatch {
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
            Err(OPCPFluidArrayError::LengthMismatch {
                expected: 5,
                got: 4,
                ..
            })
        ));
    }

    #[test]
    fn lateral_heat_source_raises_temperature_over_a_step() {
        let mut arr = test_array(5);
        arr.correct_transport();
        let t_before: f64 = arr
            .get_temperature_vector()
            .iter()
            .map(|t| t.get::<kelvin>())
            .sum();

        let fractions = vec![0.2; 5];
        arr.lateral_link_new_power_vector(Power::new::<watt>(500.0), fractions)
            .unwrap();
        arr.step();
        // `t`/`rho` lag `he` by one outer-corrector iteration (they're refreshed
        // from the *previous* `he` before the energy equation solves for the new
        // one) — sync them from the fresh `he` before reading the temperature.
        arr.correct_thermo();

        let t_after: f64 = arr
            .get_temperature_vector()
            .iter()
            .map(|t| t.get::<kelvin>())
            .sum();
        assert!(
            t_after > t_before,
            "t_after={t_after} should exceed t_before={t_before}"
        );
    }

    #[test]
    fn lateral_conductance_cools_toward_colder_neighbour() {
        let mut arr = test_array(5);
        arr.correct_transport();
        let t_before: f64 = arr
            .get_temperature_vector()
            .iter()
            .map(|t| t.get::<kelvin>())
            .sum();

        let colder = vec![ThermodynamicTemperature::new::<kelvin>(250.0); 5];
        arr.lateral_link_new_temperature_vector_avg_conductance(
            ThermalConductance::new::<watt_per_kelvin>(50.0),
            colder,
        )
        .unwrap();
        arr.step();
        arr.correct_thermo(); // sync `t` from the fresh `he` -- see the comment above

        let t_after: f64 = arr
            .get_temperature_vector()
            .iter()
            .map(|t| t.get::<kelvin>())
            .sum();
        assert!(
            t_after < t_before,
            "t_after={t_after} should be below t_before={t_before}"
        );
    }

    #[test]
    fn clear_vectors_empties_after_step() {
        let mut arr = test_array(5);
        arr.correct_transport();
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
    fn set_temperature_vector_updates_he_consistently() {
        let mut arr = test_array(3);
        let target = vec![ThermodynamicTemperature::new::<kelvin>(310.0); 3];
        arr.set_temperature_vector(target).unwrap();
        for c in 0..3 {
            assert!((arr.t.internal[c] - 310.0).abs() < 1e-6);
        }
        let he_before = arr.he.internal.as_slice().to_vec();
        arr.correct_thermo();
        for c in 0..3 {
            assert!(
                (arr.he.internal[c] - he_before[c]).abs() < 1e-6,
                "correct_thermo should reproduce the same he from the (p, he) pair set_temperature_vector wrote"
            );
        }
    }

    #[test]
    fn get_hydraulic_diameter_matches_4a_over_p() {
        let mut arr = test_array(3);
        arr.set_wetted_perimeter(Length::new::<meter>(0.1));
        let d_h = arr.get_hydraulic_diameter();
        let expected = 4.0 * arr.xs_area.get::<square_meter>() / 0.1;
        assert!((d_h.get::<meter>() - expected).abs() < 1e-9);
    }

    /// Verifies the OpenFOAM-style pressure bounding added to
    /// [`OPCPFluidArray::step`] (mirroring `pressureControl::limit`'s
    /// `pMin`/`pMax` clamp — see that method's doc comment and the V&V log
    /// `pressure_bounding_vs_openfoam_pressurecontrol.md`). Sets tight
    /// bounds via [`OPCPFluidArray::set_pressure_bounds`] and drives a brisk
    /// impulsive inlet velocity; asserts every cell pressure stays inside
    /// the bounds and finite across the run — i.e. the clamp is applied each
    /// step. Result (2026-07-14): passes.
    #[test]
    fn pressure_bounding_clamps_transient_within_set_bounds() {
        use uom::si::pressure::pascal;
        use uom::si::velocity::meter_per_second;

        let mut arr = OPCPFluidArray::new(
            Fluid::Nitrogen,
            Length::new::<meter>(1.0),
            uom::si::f64::Area::new::<square_meter>(1.0e-4),
            10,
            uom::si::f64::Time::new::<second>(2.0e-4),
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
        let preset_temp = ThermodynamicTemperature::new::<kelvin>(300.0);
        arr.set_temperature_vector(vec![preset_temp; 10]).unwrap();
        let inlet_state = flash::state_pt(Fluid::Nitrogen, 300.0, 1.0e5).unwrap();
        arr.set_inlet_velocity(Velocity::new::<meter_per_second>(2.0));
        arr.set_inlet_enthalpy(AvailableEnergy::new::<joule_per_kilogram>(
            inlet_state.enthalpy,
        ));
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

    // ── Boundary-condition regression fixtures ───────────────────────────────
    //
    // Nitrogen at 1 bar, 300-500 K: comfortably single-phase everywhere in the
    // range these tests visit, so the `(p, T)` and `(p, h)` flashes are always
    // well-posed (water is not usable here -- a single-phase flash panics
    // inside the dome).

    /// Number of cells in the BC regression fixtures.
    const BC_N: usize = 8;
    /// Working pressure \[Pa\] of the BC regression fixtures.
    const BC_P: f64 = 1.0e5;
    /// Cross-sectional area \[m²\] of the BC regression fixtures.
    const BC_A: f64 = 1.0e-4;
    /// Timestep \[s\] of the BC regression fixtures -- at/below the acoustic CFL
    /// limit `dx/c ≈ 0.125/353 ≈ 3.5e-4 s` for nitrogen gas at 300 K.
    const BC_DT: f64 = 2.0e-4;

    /// A 1 m, 8-cell nitrogen pipe at `BC_P`, uniformly seeded at `t_seed`, with
    /// the outlet pressure fixed at `BC_P`. No inlet BC is applied -- each test
    /// installs the one it exercises.
    fn bc_fixture(t_seed: f64) -> OPCPFluidArray {
        let mut arr = OPCPFluidArray::new(
            Fluid::Nitrogen,
            Length::new::<meter>(1.0),
            uom::si::f64::Area::new::<square_meter>(BC_A),
            BC_N as i64,
            uom::si::f64::Time::new::<second>(BC_DT),
        )
        .unwrap();
        arr.set_piso_algorithm(2);
        for c in 0..BC_N {
            arr.p.internal[c] = BC_P;
        }
        arr.set_temperature_vector(vec![ThermodynamicTemperature::new::<kelvin>(t_seed); BC_N])
            .unwrap();
        arr.correct_transport();
        arr.set_outlet_pressure(Pressure::new::<pascal>(BC_P));
        arr
    }

    /// Specific enthalpy \[J/kg\] of nitrogen at `BC_P` and `t`.
    fn bc_enthalpy(t: f64) -> f64 {
        flash::state_pt(Fluid::Nitrogen, t, BC_P).unwrap().enthalpy
    }

    /// Drives `bc_fixture(t_seed)` with a fixed 0.5 m/s inlet velocity and an
    /// inlet enthalpy taken at `t_inlet`, both applied **exactly once** before
    /// the run, and returns the enthalpy field after `n_steps`.
    fn run_inlet_enthalpy_case(t_seed: f64, t_inlet: f64, n_steps: usize) -> Vec<f64> {
        let mut arr = bc_fixture(t_seed);
        arr.set_inlet_velocity(Velocity::new::<meter_per_second>(0.5));
        arr.set_inlet_enthalpy(AvailableEnergy::new::<joule_per_kilogram>(bc_enthalpy(
            t_inlet,
        )));
        arr.run(n_steps);
        arr.he.internal.as_slice().to_vec()
    }

    /// ## Methodology
    /// The inlet-enthalpy regression the `TampinesSteamArray` defect
    /// (bead `op-289n`) hid behind: start from a **uniform** field that differs
    /// from the inlet BC, so the only thing that can move the solution is
    /// advection **from the boundary** (a uniform field has zero internal
    /// convective change), and apply
    /// [`OPCPFluidArray::set_inlet_enthalpy`] **once**, not once per step.
    ///
    /// 8 cells, 1 m, 10⁻⁴ m², nitrogen at 1 bar (single-phase throughout),
    /// transient PISO with 2 pressure correctors, `dt = 2e-4 s` (below the
    /// acoustic CFL limit `dx/c ≈ 3.5e-4 s`), 3000 steps = 0.6 s simulated
    /// ≈ 0.3 pipe transits at the imposed 0.5 m/s. Seed 300 K
    /// (`h = 311.20 kJ/kg`); inlet BC 500 K (`h = 520.45 kJ/kg`). Pass
    /// criterion: the inlet cell closes at least half the 209.25 kJ/kg gap to
    /// the BC, the field mean rises, and every cell stays finite.
    ///
    /// ## Results (2026-08-12, bounded van Leer default)
    /// Passes. `he[0] = 519.48 kJ/kg` — gap closed **99.5 %**, i.e. the inlet
    /// cell sits just under the 520.45 kJ/kg BC without overshooting it — and
    /// field mean `364.73 kJ/kg` versus the 311.20 kJ/kg seed.
    ///
    /// **Moved by the 2026-08-12 scheme fix (bead op-1fyp).** Under the previous
    /// unlimited central differencing the same run gave `he[0] = 556.52 kJ/kg`
    /// (117.2 % of the gap — 36 kJ/kg *past* the boundary value it was
    /// approaching) and mean `364.98 kJ/kg`. The mean barely moved (0.07 %,
    /// as it should: the same energy is being advected in either way) while the
    /// unphysical overshoot disappeared, which is the change being made.
    ///
    /// **This test fails on the pre-fix code**: `FvMatrix::solve` returns a
    /// zero-gradient-bounded field, so before `step` re-stamped the captured
    /// `he` BC template the inlet enthalpy was a one-shot. Measured on the
    /// pre-fix code at 2000 steps of this same setup, `he[0]` reached only
    /// 311.48 kJ/kg — 0.13 % of the gap — and after 10 000 steps no cell in the
    /// whole field was more than 0.4 kJ/kg from its seed.
    ///
    /// **Known limitation (not asserted here).** `fvc::div` interpolates the
    /// convected enthalpy linearly, i.e. pure central differencing, and the
    /// energy equation is advection-dominated (cell Péclet ≫ 1). The advancing
    /// thermal front therefore carries dispersive over/undershoots of order
    /// 10-15 % of the imposed step on this 8-cell mesh — the inlet cell
    /// overshoots the 520.45 kJ/kg BC. That is a discretisation-scheme
    /// question (an upwind/limited convection scheme for `∇·(φh)`), separate
    /// from the boundary condition being honoured, and is not fixed here.
    #[test]
    fn inlet_enthalpy_bc_heats_uniform_field_without_reapplying_each_step() {
        let h_seed = bc_enthalpy(300.0);
        let h_bc = bc_enthalpy(500.0);
        let he = run_inlet_enthalpy_case(300.0, 500.0, 3000);

        assert!(
            he.iter().all(|h| h.is_finite()),
            "enthalpy field went non-finite: {he:?}"
        );
        let closed = (he[0] - h_seed) / (h_bc - h_seed);
        assert!(
            closed > 0.5,
            "inlet cell should close most of the gap to the inlet BC; \
             he[0] = {:.2} kJ/kg closed only {:.1} % of the {:.2} kJ/kg gap \
             from the {:.2} kJ/kg seed (field {he:?})",
            he[0] / 1e3,
            100.0 * closed,
            (h_bc - h_seed) / 1e3,
            h_seed / 1e3,
        );
        let mean = he.iter().sum::<f64>() / he.len() as f64;
        assert!(
            mean > h_seed + 0.05 * (h_bc - h_seed),
            "field mean {:.2} kJ/kg should have risen well above the {:.2} kJ/kg seed",
            mean / 1e3,
            h_seed / 1e3,
        );
    }

    /// The cooling direction of
    /// [`inlet_enthalpy_bc_heats_uniform_field_without_reapplying_each_step`]
    /// — same fixture and pass criterion, with the seed and the inlet BC
    /// exchanged (seed 500 K, `h = 520.45 kJ/kg`; inlet BC 300 K,
    /// `h = 311.20 kJ/kg`). Both directions are exercised because a boundary
    /// term that contributes an unset/zero enthalpy rather than the prescribed
    /// one still *looks* right in the cooling direction — that was one of the
    /// symptoms recorded in bead `op-289n`.
    ///
    /// ## Results (2026-08-12, bounded van Leer default)
    /// Passes: `he[0] = 312.40 kJ/kg` — gap closed **99.4 %**, just short of the
    /// 311.20 kJ/kg BC rather than past it — and field mean `444.97 kJ/kg`
    /// versus the 520.45 kJ/kg seed. Fails on the pre-BC-fix code for the same
    /// reason as the heating case (measured then at 2000 steps:
    /// `he[0] = 520.16 kJ/kg`, 0.14 % of the gap).
    ///
    /// **Moved by the 2026-08-12 scheme fix (bead op-1fyp):** unlimited central
    /// differencing gave `he[0] = 296.13 kJ/kg` (107.2 % of the gap, undershooting
    /// the BC by 15 kJ/kg) and mean `444.68 kJ/kg`.
    #[test]
    fn inlet_enthalpy_bc_cools_uniform_field_without_reapplying_each_step() {
        let h_seed = bc_enthalpy(500.0);
        let h_bc = bc_enthalpy(300.0);
        let he = run_inlet_enthalpy_case(500.0, 300.0, 3000);

        assert!(
            he.iter().all(|h| h.is_finite()),
            "enthalpy field went non-finite: {he:?}"
        );
        let closed = (he[0] - h_seed) / (h_bc - h_seed);
        assert!(
            closed > 0.5,
            "inlet cell should close most of the gap to the inlet BC; \
             he[0] = {:.2} kJ/kg closed only {:.1} % of the {:.2} kJ/kg gap \
             from the {:.2} kJ/kg seed (field {he:?})",
            he[0] / 1e3,
            100.0 * closed,
            (h_bc - h_seed) / 1e3,
            h_seed / 1e3,
        );
        let mean = he.iter().sum::<f64>() / he.len() as f64;
        assert!(
            mean < h_seed + 0.05 * (h_bc - h_seed),
            "field mean {:.2} kJ/kg should have fallen well below the {:.2} kJ/kg seed",
            mean / 1e3,
            h_seed / 1e3,
        );
    }

    /// ## Methodology
    /// Verifies that [`OPCPFluidArray::set_inlet_mass_flowrate`] actually
    /// **imposes** the mass flow it is given, and contrasts it with the
    /// workaround it replaces (convert to a velocity with an assumed density
    /// and call [`OPCPFluidArray::set_inlet_velocity`]).
    ///
    /// Same 8-cell nitrogen fixture as the inlet-enthalpy tests, seeded and fed
    /// at 300 K, target `ṁ = 1.0 g/s`, 2000 steps of 2×10⁻⁴ s. Three arrays are
    /// run: (a) a velocity derived from the *correct* density
    /// `ρ(300 K, 1 bar) = 1.12328 kg/m³`; (b) a velocity derived from a *wrong*
    /// (design-point) density `ρ(600 K, 1 bar) = 0.56130 kg/m³`, i.e. a factor-2
    /// error, which is the failure mode this fix exists for; (c) the prescribed
    /// mass-flow inlet. The imposed flow is read back with
    /// [`OPCPFluidArray::get_inlet_mass_flowrate_actual`], which sums the solved
    /// boundary face flux `phi` and so is independent of how the BC was set.
    ///
    /// Pass criterion: (c) reproduces the target to better than 1×10⁻⁶ relative,
    /// and (b) is out by more than 50 % — i.e. the test would not pass merely
    /// because the two routes happen to agree.
    ///
    /// ## Results (2026-08-12)
    /// Passes. (a) `1.000000022e-3 kg/s`, +0.0000 %; (b) `2.001207e-3 kg/s`,
    /// **+100.12 %** — the assumed-density error transfers one-for-one into the
    /// imposed mass flow; (c) `1.000000000000e-3 kg/s`, relative error exactly
    /// 0.0 (by construction: the velocity is re-derived each corrector from the
    /// very face density that multiplies it).
    ///
    /// **Moved by the 2026-08-12 scheme + boundary-flux fixes:** case (a) was
    /// `9.999122e-4 kg/s` (−0.0088 %) and case (b) `2.000597e-3` (+100.06 %)
    /// beforehand. Case (a) tightening to round-off is the boundary pressure
    /// correction (op-nnqi) making the whole flow field consistent; case (c) was
    /// exact before and after, since it is exact by construction.
    #[test]
    fn mass_flow_inlet_imposes_the_prescribed_flowrate() {
        let target = 1.0e-3_f64;
        let h_in = bc_enthalpy(300.0);

        let mut with_right_rho = bc_fixture(300.0);
        let rho_right = flash::state_pt(Fluid::Nitrogen, 300.0, BC_P)
            .unwrap()
            .density;
        with_right_rho.set_inlet_enthalpy(AvailableEnergy::new::<joule_per_kilogram>(h_in));
        with_right_rho.set_inlet_velocity(Velocity::new::<meter_per_second>(
            target / (rho_right * BC_A),
        ));
        with_right_rho.run(2000);
        let m_right = with_right_rho
            .get_inlet_mass_flowrate_actual()
            .get::<kilogram_per_second>();

        let mut with_wrong_rho = bc_fixture(300.0);
        let rho_wrong = flash::state_pt(Fluid::Nitrogen, 600.0, BC_P)
            .unwrap()
            .density;
        with_wrong_rho.set_inlet_enthalpy(AvailableEnergy::new::<joule_per_kilogram>(h_in));
        with_wrong_rho.set_inlet_velocity(Velocity::new::<meter_per_second>(
            target / (rho_wrong * BC_A),
        ));
        with_wrong_rho.run(2000);
        let m_wrong = with_wrong_rho
            .get_inlet_mass_flowrate_actual()
            .get::<kilogram_per_second>();

        let mut prescribed = bc_fixture(300.0);
        prescribed.set_inlet_enthalpy(AvailableEnergy::new::<joule_per_kilogram>(h_in));
        prescribed.set_inlet_mass_flowrate(MassRate::new::<kilogram_per_second>(target));
        prescribed.run(2000);
        let m_prescribed = prescribed
            .get_inlet_mass_flowrate_actual()
            .get::<kilogram_per_second>();

        assert!(
            ((m_prescribed - target) / target).abs() < 1.0e-6,
            "prescribed mass-flow inlet should impose the target exactly; \
             got {m_prescribed:.9e} kg/s against a {target:.9e} kg/s target"
        );
        assert!(
            ((m_wrong - target) / target).abs() > 0.5,
            "the assumed-density workaround should visibly mis-impose the flow \
             (that is the defect being fixed); got {m_wrong:.9e} kg/s against a \
             {target:.9e} kg/s target, with the right-density case at {m_right:.9e} kg/s"
        );
    }

    /// ## Methodology
    /// The steady-state energy check a counter-flow heat exchanger needs and
    /// the assumed-density velocity workaround cannot deliver: with a
    /// prescribed inlet mass flow, a fixed inlet enthalpy and a uniformly
    /// distributed 200 W volumetric heat source, the converged array must
    /// satisfy `ṁ·(h_out − h_in) = Q`.
    ///
    /// Same 8-cell nitrogen fixture, seeded and fed at 300 K, `ṁ = 1.0 g/s`,
    /// 4000 steps of 2×10⁻⁴ s = 0.8 s simulated (about 7 transits at the
    /// resulting ≈ 9 m/s, and well past the thermal transient). Pass criterion:
    /// the enthalpy rise closes the 200 W balance to within 1 %.
    ///
    /// ## Results (2026-08-12)
    /// Passes: `ṁ·(h_out − h_in) = 200.06 W` against `Q = 200 W` (+0.03 %), at
    /// `T_out = 491.29 K`. Refining the mesh tightens it — measured at the same
    /// physical setup, `n = 16` gives 200.0002 W and `n = 32` gives 200.0000 W.
    ///
    /// **Residual, deliberately not asserted here.** The *mass* balance does
    /// not close as tightly as the energy balance:
    /// [`OPCPFluidArray::get_outlet_mass_flowrate_actual`] reads
    /// 9.9296×10⁻⁴ kg/s against the exactly-imposed 1.0×10⁻³ kg/s inlet, a
    /// −0.70 % deficit at `n = 8`. That is a first-order boundary truncation
    /// error, not a broken BC: it halves under each mesh refinement
    /// (−0.7035 % at `n = 8`, −0.3150 % at `n = 16`, −0.1482 % at `n = 32`,
    /// measured 2026-08-12), because the outlet face velocity is taken by
    /// zero-gradient extrapolation from the last cell centre while the flow is
    /// still accelerating through that half cell. See that method's accuracy
    /// caveat.
    #[test]
    fn mass_flow_inlet_closes_the_steady_energy_balance() {
        let target = 1.0e-3_f64;
        let q_watt = 200.0_f64;
        let h_in = bc_enthalpy(300.0);

        let mut arr = bc_fixture(300.0);
        arr.set_inlet_enthalpy(AvailableEnergy::new::<joule_per_kilogram>(h_in));
        arr.set_inlet_mass_flowrate(MassRate::new::<kilogram_per_second>(target));
        for _ in 0..4000 {
            arr.lateral_link_new_power_vector(
                Power::new::<watt>(q_watt),
                vec![1.0 / BC_N as f64; BC_N],
            )
            .unwrap();
            arr.step();
        }

        let m_in = arr
            .get_inlet_mass_flowrate_actual()
            .get::<kilogram_per_second>();
        let h_out = arr.get_outlet_enthalpy().get::<joule_per_kilogram>();
        let q_transported = m_in * (h_out - h_in);
        assert!(
            ((q_transported - q_watt) / q_watt).abs() < 0.01,
            "steady energy balance should close: mdot*(h_out - h_in) = {q_transported:.4} W \
             against Q = {q_watt} W (mdot = {m_in:.9e} kg/s, h_out = {:.2} kJ/kg)",
            h_out / 1e3,
        );
    }

    /// [`OPCPFluidArray::set_inlet_velocity`] and
    /// [`OPCPFluidArray::set_inlet_mass_flowrate`] prescribe the same patch, so
    /// each must clear the other rather than leaving a silently-overridden
    /// boundary condition behind. Verifies the flag bookkeeping directly (no
    /// solve involved). Result (2026-08-12): passes.
    #[test]
    fn inlet_velocity_and_mass_flow_bcs_are_mutually_exclusive() {
        let mut arr = bc_fixture(300.0);
        assert!(arr.get_inlet_mass_flowrate().is_none());

        let target = MassRate::new::<kilogram_per_second>(1.0e-3);
        arr.set_inlet_mass_flowrate(target);
        assert_eq!(arr.get_inlet_mass_flowrate(), Some(target));

        arr.set_inlet_velocity(Velocity::new::<meter_per_second>(1.0));
        assert!(
            arr.get_inlet_mass_flowrate().is_none(),
            "set_inlet_velocity must clear a prescribed mass-flow inlet"
        );

        arr.set_inlet_mass_flowrate(target);
        arr.clear_inlet_mass_flowrate();
        assert!(arr.get_inlet_mass_flowrate().is_none());
    }

    /// ## Methodology
    /// The **boundedness** regression for the energy equation's convection term
    /// (bead op-1fyp). Advects a step change in enthalpy from the inlet into a
    /// uniform field and asserts the solution never leaves the range spanned by
    /// its own initial state and its inlet BC — the defining property of a TVD
    /// scheme, and the property unlimited central differencing does not have.
    ///
    /// Both directions are run (seed 300 K → BC 500 K, and the reverse), at 3000
    /// and 10 000 steps of 2×10⁻⁴ s on the 8-cell nitrogen fixture at 0.5 m/s.
    /// The test also runs [`EnergyConvectionScheme::Linear`] and asserts it
    /// **does** violate the bound, so it cannot pass merely because the case is
    /// too gentle to distinguish the schemes.
    ///
    /// ## Results (2026-08-12), as a fraction of the 209.26 kJ/kg imposed step
    ///
    /// | scheme | 3000 steps, heating | 3000, cooling | 10 000, heating | 10 000, cooling |
    /// |---|---|---|---|---|
    /// | `Linear` (old) | **+17.24 % over** | **+7.20 % under** | **+36.36 % over** | **+12.71 % under** |
    /// | `VanLeer` (new default) | none (−0.46 %) | none (−0.58 %) | none (−0.00 %) | none (−0.00 %) |
    /// | `Minmod` | none (−1.47 %) | none (−1.52 %) | none (−0.00 %) | none (−0.00 %) |
    /// | `Upwind` | none (−9.06 %) | none (−9.05 %) | none (−0.03 %) | none (−0.00 %) |
    ///
    /// A negative figure is the margin by which the field stayed *inside* the
    /// bound. In temperature terms the `Linear` violations are the ones that
    /// matter physically: 534.1 K against a 500 K inlet BC at 3000 steps
    /// heating, 571.6 K at 10 000 steps, and 274.5 K against a 300 K inlet and a
    /// 500 K seed in the cooling direction — an undershoot below *everything*
    /// imposed, which for a stream near saturation can hand `correct_thermo`'s
    /// `(p, h)` flash a state outside the EOS's range.
    ///
    /// `VanLeer` keeps the front markedly sharper than `Upwind` while staying
    /// bounded: at 3000 steps heating its profile is 499.1/458.0/349.7/303.1 K
    /// over the first four cells against `Upwind`'s 482.0/429.7/366.7/325.7 K.
    #[test]
    fn energy_convection_is_bounded_at_an_advected_front() {
        for &(t_seed, t_bc) in &[(300.0_f64, 500.0_f64), (500.0, 300.0)] {
            let h_seed = bc_enthalpy(t_seed);
            let h_bc = bc_enthalpy(t_bc);
            let lo = h_seed.min(h_bc);
            let hi = h_seed.max(h_bc);
            let span = hi - lo;

            for &steps in &[3000usize, 10_000] {
                // The bounded default must not leave [lo, hi].
                let mut arr = bc_fixture(t_seed);
                arr.set_inlet_velocity(Velocity::new::<meter_per_second>(0.5));
                arr.set_inlet_enthalpy(AvailableEnergy::new::<joule_per_kilogram>(h_bc));
                arr.run(steps);
                let he = arr.he.internal.as_slice().to_vec();
                let over = he.iter().cloned().fold(f64::NEG_INFINITY, f64::max) - hi;
                let under = lo - he.iter().cloned().fold(f64::INFINITY, f64::min);
                assert!(
                    over < 0.01 * span && under < 0.01 * span,
                    "the default (TVD) scheme must stay inside the {:.2}..{:.2} kJ/kg \
                     range set by the seed and the inlet BC ({t_seed} K -> {t_bc} K, \
                     {steps} steps): overshoot {:+.2} kJ/kg, undershoot {:+.2} kJ/kg, \
                     field {he:?}",
                    lo / 1e3,
                    hi / 1e3,
                    over / 1e3,
                    under / 1e3,
                );
            }

            // ...and unlimited central differencing must visibly fail to, so
            // this case genuinely discriminates between the schemes.
            let mut arr = bc_fixture(t_seed);
            arr.set_he_convection_scheme(EnergyConvectionScheme::Linear);
            arr.set_inlet_velocity(Velocity::new::<meter_per_second>(0.5));
            arr.set_inlet_enthalpy(AvailableEnergy::new::<joule_per_kilogram>(h_bc));
            arr.run(10_000);
            let he = arr.he.internal.as_slice().to_vec();
            let over = he.iter().cloned().fold(f64::NEG_INFINITY, f64::max) - hi;
            let under = lo - he.iter().cloned().fold(f64::INFINITY, f64::min);
            assert!(
                over.max(under) > 0.05 * span,
                "EnergyConvectionScheme::Linear is expected to ring on this case \
                 ({t_seed} K -> {t_bc} K); if it no longer does, the case has become \
                 too gentle to discriminate the schemes and the bounded assertion \
                 above proves nothing. overshoot {:+.2} kJ/kg, undershoot {:+.2} kJ/kg",
                over / 1e3,
                under / 1e3,
            );
        }
    }

    // ── Global balance + reversal: the tests that gate the whole formulation ──
    //
    // The per-setter tests above check that each boundary condition is wired up.
    // These two check the things a *pipe in a network* has to satisfy no matter
    // how the setters are wired: energy in equals energy out plus storage, and
    // nothing blows up when the flow turns round. A forward-flow-only test
    // passes identically with a wrong outlet terminal and proves nothing.

    /// Stored enthalpy `Σ ρ·V·h` \[J\] over the whole array.
    fn stored_enthalpy(arr: &OPCPFluidArray) -> f64 {
        (0..arr.mesh.n_cells)
            .map(|c| arr.rho.internal[c] * arr.mesh.cell_volumes[c] * arr.he.internal[c])
            .sum()
    }

    /// Fluid mass `Σ ρ·V` \[kg\] currently held in the array.
    fn stored_mass(arr: &OPCPFluidArray) -> f64 {
        (0..arr.mesh.n_cells)
            .map(|c| arr.rho.internal[c] * arr.mesh.cell_volumes[c])
            .sum()
    }

    /// Net advected enthalpy flow **into** the array across both ends \[W\],
    /// built from exactly what the upwind advection terminals selected.
    fn net_enthalpy_in(arr: &OPCPFluidArray) -> f64 {
        arr.get_inlet_mass_flowrate_actual()
            .get::<kilogram_per_second>()
            * arr
                .get_inlet_advected_enthalpy()
                .get::<joule_per_kilogram>()
            - arr
                .get_outlet_mass_flowrate_actual()
                .get::<kilogram_per_second>()
                * arr
                    .get_outlet_advected_enthalpy()
                    .get::<joule_per_kilogram>()
    }

    /// ## Methodology
    /// The **steady global energy balance** — the statement a heat exchanger is
    /// built on, and the one that cannot even be *posed* with zero-gradient
    /// terminals at both ends (no boundary for energy to cross). Prescribed
    /// 1.0 g/s inlet mass flow, prescribed 300 K inlet enthalpy, 200 W spread
    /// uniformly over the cells, run to steady state; then check two forms:
    ///
    /// 1. the component form `ṁ·(h_out − h_in) = Q`, and
    /// 2. the boundary form `(ṁ_in·h_in − ṁ_out·h_out) + Q = 0`, built from the
    ///    solved boundary mass fluxes and the enthalpies the direction-switched
    ///    terminals actually selected ([`OPCPFluidArray::get_inlet_advected_enthalpy`]).
    ///
    /// Form 2 is the strict one: it uses the outlet mass flux rather than
    /// assuming it equals the inlet's, so it exposes any boundary-flux error.
    /// 8/16/32 cells, `dt` scaled with the mesh, 4000 steps at `n = 8`.
    ///
    /// ## Results (2026-08-12)
    /// Passes. Form 1: `199.9878 W` against 200 W (−0.006 %) at `n = 8`.
    /// Form 2: residual `+0.0122 W` = **+0.006 % of Q** at `n = 8`
    /// (`+0.0239 W` at `n = 16`, `+0.0472 W` at `n = 32`).
    ///
    /// **Form 2 is where the boundary pressure-correction fix shows up.** Before
    /// `step` applied `−ρ_f·rAU_f·snGrad(p)·|Sf|` to boundary faces as well as
    /// internal ones, the outlet carried the uncorrected predictor flux and the
    /// same residual was `+1.3834 W` (+0.69 % of Q) at `n = 8`, `+0.6347 W` at
    /// `n = 16`, `+0.3376 W` at `n = 32` — a first-order error, now essentially
    /// eliminated (113× smaller at `n = 8`). Bead op-nnqi.
    #[test]
    fn steady_energy_balance_closes_at_the_boundaries() {
        let target = 1.0e-3_f64;
        let q_watt = 200.0_f64;
        let h_in = bc_enthalpy(300.0);

        let mut arr = bc_fixture(300.0);
        arr.set_inlet_enthalpy(AvailableEnergy::new::<joule_per_kilogram>(h_in));
        arr.set_outlet_enthalpy(AvailableEnergy::new::<joule_per_kilogram>(h_in));
        arr.set_inlet_mass_flowrate(MassRate::new::<kilogram_per_second>(target));
        for _ in 0..4000 {
            arr.lateral_link_new_power_vector(
                Power::new::<watt>(q_watt),
                vec![1.0 / BC_N as f64; BC_N],
            )
            .unwrap();
            arr.step();
        }

        // Form 1 — component form.
        let m_in = arr
            .get_inlet_mass_flowrate_actual()
            .get::<kilogram_per_second>();
        let h_out = arr.get_outlet_enthalpy().get::<joule_per_kilogram>();
        let q_component = m_in * (h_out - h_in);
        assert!(
            ((q_component - q_watt) / q_watt).abs() < 0.01,
            "component energy balance: mdot*(h_out - h_in) = {q_component:.4} W against Q = {q_watt} W"
        );

        // Form 2 — strict boundary form.
        let residual = net_enthalpy_in(&arr) + q_watt;
        assert!(
            (residual / q_watt).abs() < 0.001,
            "boundary energy balance should close to better than 0.1 % of Q; \
             residual = {residual:.4} W against Q = {q_watt} W \
             (m_in = {m_in:.6e}, m_out = {:.6e} kg/s)",
            arr.get_outlet_mass_flowrate_actual()
                .get::<kilogram_per_second>(),
        );
    }

    /// ## Methodology
    /// The **transient** global balance, over a 300 K → steady heat-up under a
    /// 200 W source at a prescribed 1.0 g/s. Accumulates
    /// `∫(ṁ_in·h_in − ṁ_out·h_out + Q) dt` step by step and compares it with the
    /// change in stored enthalpy `Δ(Σ ρ·V·h)`; in parallel it accumulates
    /// `∫(ṁ_in − ṁ_out) dt` and compares with the change in stored mass
    /// `Δ(Σ ρ·V)`. 8 cells, `dt = 2×10⁻⁴ s`, 4000 steps = 0.8 s (≈ 7 transits),
    /// after a 200-step flow-establishment warm-up.
    ///
    /// ## Results (2026-08-12) — a KNOWN, QUANTIFIED, UNFIXED IMBALANCE
    /// The transient balance does **not** close, and this test pins down exactly
    /// why rather than hiding it behind a tolerance.
    ///
    /// - Energy: `∫(…)dt = 9.5696 J` against `ΔE = 0.1125 J` — a residual of
    ///   **9.4571 J**, 5.9 % of the 160 J the source delivered over the window.
    /// - Mass: `∫(ṁ_in − ṁ_out)dt = 2.761×10⁻⁷ kg` against
    ///   `Δ(Σ ρV) = −2.556×10⁻⁵ kg` — a residual of **2.584×10⁻⁵ kg**. The
    ///   array's EOS mass falls by 26 mg that never crosses either boundary.
    /// - The two residuals are the same event: `9.4571 J / 2.584×10⁻⁵ kg =
    ///   366.1 kJ/kg`, which lies inside the 311.20–511.2 kJ/kg range the
    ///   enthalpy field spans over the run. **The energy that fails to balance is
    ///   precisely the enthalpy carried by the mass that fails to balance**, and
    ///   that is what this test asserts.
    ///
    /// ## Cause, and what it is not
    /// `step` computes a continuity density `ρ_cont = ρ_old − dt·∇·φ`, then
    /// `correct_thermo` overwrites `self.rho` with the EOS density `ρ(p, h)`.
    /// The pressure equation reconciles the two only through its compressibility
    /// diagonal `ψ·V/dt` with `ψ = ∂ρ/∂p|_h` — it carries **no `∂ρ/∂h|_p` term**,
    /// so the density drop caused by *heating* is never fed back as mass the
    /// domain must expel. Under strong volumetric heating the two densities
    /// therefore drift apart and the array's EOS mass is not conserved.
    ///
    /// Ruled out by measurement (2026-08-12), so the remaining explanation is
    /// the one above:
    /// - **Not** the outlet boundary flux (op-nnqi): the residual is unchanged
    ///   (9.4571 J vs 9.4571 J) by the boundary pressure-correction fix that cut
    ///   the *steady* residual 113-fold.
    /// - **Not** spatial truncation: 9.37 J at `n = 8`, 9.43 J at `n = 16`,
    ///   8.65 J at `n = 32` — flat under refinement.
    /// - **Not** an unconverged PIMPLE loop: identical at 1, 2, 4 and 8 outer
    ///   correctors (9.4571 → 9.4796 → 9.4795 → 9.4795 J; converged by 2). This
    ///   one was a stated prediction that the measurement falsified.
    ///
    /// Tracked as its own bead; fixing it means adding the enthalpy-driven
    /// density change to the pressure equation, which is a change to the
    /// `pEqn` formulation and not something to slip in alongside a
    /// boundary-condition fix.
    #[test]
    fn transient_energy_imbalance_is_the_enthalpy_of_the_unconserved_mass() {
        let target = 1.0e-3_f64;
        let q_watt = 200.0_f64;
        let dt = BC_DT;
        let h_in = bc_enthalpy(300.0);

        let mut arr = bc_fixture(300.0);
        arr.set_inlet_enthalpy(AvailableEnergy::new::<joule_per_kilogram>(h_in));
        arr.set_outlet_enthalpy(AvailableEnergy::new::<joule_per_kilogram>(h_in));
        arr.set_inlet_mass_flowrate(MassRate::new::<kilogram_per_second>(target));
        for _ in 0..200 {
            arr.step();
        }

        let e0 = stored_enthalpy(&arr);
        let m0 = stored_mass(&arr);
        let mut energy_integral = 0.0;
        let mut mass_integral = 0.0;
        for _ in 0..4000 {
            arr.lateral_link_new_power_vector(
                Power::new::<watt>(q_watt),
                vec![1.0 / BC_N as f64; BC_N],
            )
            .unwrap();
            arr.step();
            energy_integral += (net_enthalpy_in(&arr) + q_watt) * dt;
            mass_integral += (arr
                .get_inlet_mass_flowrate_actual()
                .get::<kilogram_per_second>()
                - arr
                    .get_outlet_mass_flowrate_actual()
                    .get::<kilogram_per_second>())
                * dt;
        }
        let energy_residual = energy_integral - (stored_enthalpy(&arr) - e0);
        let mass_residual = mass_integral - (stored_mass(&arr) - m0);

        // The unbalanced mass is real and non-trivial -- if it ever vanishes,
        // the pEqn has gained the missing dρ/dh term and this test should be
        // revisited (that is a fix, not a regression).
        assert!(
            mass_residual.abs() > 1.0e-6,
            "expected the known EOS/continuity mass divergence; got only \
             {mass_residual:.6e} kg — if this is now zero the pressure equation \
             has been fixed and this test needs rewriting"
        );

        // The energy that fails to balance is the enthalpy of the mass that
        // fails to balance: their ratio must be an enthalpy the field actually
        // holds.
        let implied_h = energy_residual / mass_residual;
        let h_lo = bc_enthalpy(300.0);
        let h_hi = arr
            .he
            .internal
            .as_slice()
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(
            implied_h > h_lo && implied_h < h_hi,
            "the energy residual {energy_residual:.4} J and the mass residual \
             {mass_residual:.6e} kg should be the same event: their ratio \
             {:.2} kJ/kg must lie inside the {:.2}..{:.2} kJ/kg range the field \
             spans. If it does not, something OTHER than the known EOS/continuity \
             mass divergence is now breaking the energy balance.",
            implied_h / 1e3,
            h_lo / 1e3,
            h_hi / 1e3,
        );
    }

    /// ## Methodology
    /// **Flow reversal.** The test that a forward-flow-only suite cannot
    /// replace: with a fixed-value terminal at each end, drive the flow forward,
    /// reverse it, and reverse it again, asserting throughout that the enthalpy
    /// field never leaves the range spanned by the two end states and the seed.
    ///
    /// This is what the direction-switched upwind advection terminal buys (see
    /// `fvc::div_limited`, ported from `tuas_boussinesq_solver`'s
    /// `advection_to_bcs.rs`). Under the previous patch-type-driven treatment
    /// the left end would have kept *forcing* its prescribed enthalpy onto an
    /// outflow face while the right end advected the domain's own enthalpy back
    /// inwards — both ends wrong at once, in the direction that matters.
    ///
    /// 8 cells, nitrogen at 1 bar, seeded uniformly at 400 K (415.45 kJ/kg);
    /// left-end state 300 K (311.20 kJ/kg), right-end state 500 K
    /// (520.45 kJ/kg); ±0.5 m/s, switching every 2500 steps of 2×10⁻⁴ s, four
    /// phases (two full reversals).
    ///
    /// ## Results (2026-08-12)
    /// Passes with **zero** excursion: worst overshoot and worst undershoot are
    /// both `+0.00 kJ/kg` across all four phases, i.e. the field stays inside
    /// 311.20–520.45 kJ/kg to round-off at every one of the 10 000 steps.
    /// The physics comes out right end-to-end: flowing forward the inlet cell
    /// settles to 301.7 K (approaching the 300 K left-end state), and after
    /// reversal the far cell settles to 499.9 K (approaching the 500 K
    /// right-end state) — each end supplies its own prescribed state exactly
    /// when it is the upstream one.
    #[test]
    fn flow_reversal_keeps_enthalpy_bounded_by_both_end_states() {
        let h_left = bc_enthalpy(300.0);
        let h_right = bc_enthalpy(500.0);
        let h_seed = bc_enthalpy(400.0);
        let lo = h_left.min(h_right).min(h_seed);
        let hi = h_left.max(h_right).max(h_seed);
        let tol = 1.0e-3 * (hi - lo); // 0.1 % of the span

        let mut arr = bc_fixture(400.0);
        arr.set_inlet_enthalpy(AvailableEnergy::new::<joule_per_kilogram>(h_left));
        arr.set_outlet_enthalpy(AvailableEnergy::new::<joule_per_kilogram>(h_right));

        for phase in 0..4 {
            let u = if phase % 2 == 0 { 0.5 } else { -0.5 };
            arr.set_inlet_velocity(Velocity::new::<meter_per_second>(u));
            for step in 0..2500 {
                arr.step();
                for (c, &h) in arr.he.internal.as_slice().iter().enumerate() {
                    assert!(
                        h > lo - tol && h < hi + tol,
                        "phase {phase} (u = {u} m/s), step {step}, cell {c}: enthalpy \
                         {:.2} kJ/kg left the {:.2}..{:.2} kJ/kg range spanned by the \
                         two end states and the seed",
                        h / 1e3,
                        lo / 1e3,
                        hi / 1e3,
                    );
                }
            }
        }

        // Reversal actually happened, and the right-hand end supplied its own
        // state when it became the inlet.
        assert!(
            arr.get_inlet_mass_flowrate_actual()
                .get::<kilogram_per_second>()
                < 0.0,
            "the final phase should be running in reverse"
        );
        arr.correct_thermo();
        let t_far = arr.t.internal[BC_N - 1];
        assert!(
            t_far > 490.0,
            "with flow entering through the right-hand end, the far cell should \
             approach that end's prescribed 500 K state; got {t_far:.2} K"
        );
    }

    // ── EnergyBalanceMode: explicit vs implicit energy convection ────────────
    //
    // These are the V&V cases for `EnergyBalanceMode` (bead op-j2oq). The
    // `bc_fixture` above runs at `dt = 2e-4 s` on a 0.125 m cell at 0.5 m/s, i.e.
    // cell Courant number `Co = |u|·dt/dx = 8e-4` — three orders of magnitude
    // below the explicit term's stability limit. A test at that step passes under
    // BOTH modes and therefore proves nothing about the mode switch, so the
    // headline case below deliberately runs the *same pipe* at `Co = 2`.

    /// The velocity used by the Courant-number fixtures \[m/s\].
    const CO_U: f64 = 0.5;
    /// Cell size of the 8-cell, 1 m Courant fixtures \[m\].
    const CO_DX: f64 = 1.0 / BC_N as f64;

    /// An 8-cell / 1 m / 10⁻⁴ m² nitrogen pipe at `BC_P`, seeded uniformly at
    /// `t_seed`, fed at [`CO_U`] with a `t_bc` inlet enthalpy, stepping at `dt`
    /// — i.e. at cell Courant number `Co = CO_U·dt/CO_DX` (dimensionless).
    ///
    /// Unlike [`bc_fixture`] the **velocity field is pre-seeded** at [`CO_U`]
    /// rather than started from rest. That is deliberate: an impulsive start at a
    /// half-second timestep would superimpose a violent startup acoustic
    /// transient on the question being asked, and the question here is only about
    /// the energy equation's convection term. Starting from the flow the inlet BC
    /// imposes keeps every mode comparison a like-for-like one.
    fn courant_fixture(
        dt: f64,
        t_seed: f64,
        t_bc: f64,
        mode: EnergyBalanceMode,
        scheme: EnergyConvectionScheme,
    ) -> OPCPFluidArray {
        let mut arr = OPCPFluidArray::new(
            Fluid::Nitrogen,
            Length::new::<meter>(1.0),
            uom::si::f64::Area::new::<square_meter>(BC_A),
            BC_N as i64,
            uom::si::f64::Time::new::<second>(dt),
        )
        .unwrap();
        arr.set_piso_algorithm(2);
        for c in 0..BC_N {
            arr.p.internal[c] = BC_P;
            arr.u.internal[c].x = CO_U;
        }
        arr.set_temperature_vector(vec![ThermodynamicTemperature::new::<kelvin>(t_seed); BC_N])
            .unwrap();
        arr.correct_transport();
        arr.set_outlet_pressure(Pressure::new::<pascal>(BC_P));
        arr.set_inlet_velocity(Velocity::new::<meter_per_second>(CO_U));
        arr.set_inlet_enthalpy(AvailableEnergy::new::<joule_per_kilogram>(bc_enthalpy(
            t_bc,
        )));
        arr.set_he_balance_mode(mode);
        arr.set_he_convection_scheme(scheme);
        arr
    }

    /// Worst overshoot above `hi` and worst undershoot below `lo`, each as a
    /// **fraction of the `hi − lo` span** (dimensionless, negative = inside the
    /// range with margin). Returns `None` if any cell went non-finite.
    fn excursion(arr: &OPCPFluidArray, lo: f64, hi: f64) -> Option<(f64, f64)> {
        let he = arr.he.internal.as_slice();
        if !he.iter().all(|h| h.is_finite()) {
            return None;
        }
        let span = hi - lo;
        let mx = he.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let mn = he.iter().cloned().fold(f64::INFINITY, f64::min);
        Some(((mx - hi) / span, (lo - mn) / span))
    }

    /// ## Methodology
    /// **The acceptance case for [`EnergyBalanceMode::Implicit`]**: the same
    /// 8-cell nitrogen pipe driven past the explicit term's Courant limit, so the
    /// two modes are genuinely discriminated rather than merely both passing.
    ///
    /// Geometry/flow: 1 m, 8 cells (`Δx = 0.125 m`), 10⁻⁴ m², nitrogen at 1 bar,
    /// 0.5 m/s, seeded 300 K (`h = 311.20 kJ/kg`) with a 500 K
    /// (`h = 520.45 kJ/kg`) inlet-enthalpy BC and a 1 bar outlet — i.e. an
    /// advected thermal front, the case bead `op-1fyp` was measured on. Every run
    /// covers the **same 4 s of physical time** (≈ 2 pipe transits), with the step
    /// and the step count traded off against each other, and every run is
    /// otherwise byte-identical apart from
    /// [`OPCPFluidArray::set_he_balance_mode`]. Three Courant numbers:
    ///
    /// | `Δt` | steps | `Co = uΔt/Δx` |
    /// |---|---|---|
    /// | 0.25 s | 16 | 1.00 |
    /// | 0.3125 s | 13 | 1.25 |
    /// | 0.5 s | 8 | 2.00 |
    ///
    /// Pass criterion, measured as an excursion outside the 311.20–520.45 kJ/kg
    /// range spanned by the run's own initial and boundary data, as a fraction of
    /// that 209.26 kJ/kg span: **explicit must fail** (excursion > 100 % of span,
    /// or non-finite) at `Co ≥ 1.25`, **implicit must hold** (excursion < 5 % of
    /// span) at every `Co`, and both must agree that `Co = 1.00` is still fine —
    /// the last one is what pins the failure to the Courant condition rather than
    /// to something incidental about a large timestep.
    ///
    /// ## Results (2026-08-13)
    /// Passes, and reproduces the theoretical `Co = 1` threshold almost exactly.
    /// Overshoot above the 520.45 kJ/kg boundary value, in % of span:
    ///
    /// | `Co` | Explicit / van Leer | Implicit / van Leer |
    /// |---|---|---|
    /// | 1.00 | +0.00 % | +0.01 % |
    /// | 1.25 | **+135.14 %** | +0.10 % |
    /// | 2.00 | **+47539 %** | +1.19 % |
    ///
    /// At `Co = 2` the explicit field reads
    /// `[512.90, 18244.62, −10000.00, 47964.83, 100000.00, −10000.00, 1116.70,
    /// 918.75] kJ/kg` — four cells pinned on the `(p, h)` admissibility guard
    /// rails (`h_clamp_events = 4`), which is the guard doing its job on a solve
    /// that has already diverged, not a solution. The implicit field at the same
    /// step reads `[520.46, 520.40, 520.33, 520.58, 521.95, 522.94, 516.43,
    /// 487.19] kJ/kg`: a physically sensible, nearly flushed pipe approaching the
    /// 520.45 kJ/kg inlet state.
    ///
    /// **The divergence is the energy equation, not the pressure–velocity
    /// coupling.** In every diverged explicit run the pressure and velocity fields
    /// stay finite (asserted below), and the momentum equation is already implicit
    /// (`fvm::div_vec`). The only difference between the passing and the failing
    /// run is where `∇·(φh)` is evaluated.
    ///
    /// **Also measured, and consistent with theory:** explicit *upwind*
    /// (`EnergyConvectionScheme::Upwind`, `EnergyBalanceMode::Explicit`) diverges
    /// at `Co = 4` too — `[100000.00, −10000.00, −10000.00, 89719.16, …] kJ/kg`.
    /// The limiter is not what fails; the explicit treatment is. That is the whole
    /// reason the two knobs are separate enums.
    #[test]
    fn implicit_energy_balance_survives_courant_above_one() {
        let (t_seed, t_bc) = (300.0_f64, 500.0_f64);
        let lo = bc_enthalpy(t_seed).min(bc_enthalpy(t_bc));
        let hi = bc_enthalpy(t_seed).max(bc_enthalpy(t_bc));

        for &(dt, steps, explicit_must_fail) in &[
            (0.25_f64, 16usize, false),
            (0.3125, 13, true),
            (0.5, 8, true),
        ] {
            let co = CO_U * dt / CO_DX;

            // ── Implicit: must stay bounded at every Courant number ──────────
            let mut imp = courant_fixture(
                dt,
                t_seed,
                t_bc,
                EnergyBalanceMode::Implicit,
                EnergyConvectionScheme::VanLeer,
            );
            imp.run(steps);
            let (over, under) = excursion(&imp, lo, hi).unwrap_or_else(|| {
                panic!(
                    "Co = {co:.2}: the implicit mode went non-finite: {:?}",
                    imp.he.internal.as_slice()
                )
            });
            assert!(
                over < 0.05 && under < 0.05,
                "Co = {co:.2}: the implicit mode must stay inside the \
                 {:.2}..{:.2} kJ/kg range set by its own seed and inlet BC; \
                 overshoot {:+.2} %, undershoot {:+.2} % of span; field {:?}",
                lo / 1e3,
                hi / 1e3,
                100.0 * over,
                100.0 * under,
                imp.he.internal.as_slice(),
            );

            // ── Explicit: fine at Co = 1, diverges above it ──────────────────
            let mut exp = courant_fixture(
                dt,
                t_seed,
                t_bc,
                EnergyBalanceMode::Explicit,
                EnergyConvectionScheme::VanLeer,
            );
            exp.run(steps);
            let exp_excursion = excursion(&exp, lo, hi);
            // Whatever the energy equation did, the pressure-velocity coupling
            // must have survived — otherwise this case would not isolate the
            // convection treatment.
            assert!(
                exp.p.internal.as_slice().iter().all(|x| x.is_finite())
                    && exp.u.internal.as_slice().iter().all(|v| v.x.is_finite()),
                "Co = {co:.2}: the explicit run's pressure/velocity fields must \
                 stay finite, so that any enthalpy divergence is attributable to \
                 the energy equation alone"
            );
            match exp_excursion {
                None => assert!(
                    explicit_must_fail,
                    "Co = {co:.2}: the explicit mode should still be stable here, \
                     but the enthalpy field went non-finite"
                ),
                Some((over, under)) => {
                    if explicit_must_fail {
                        assert!(
                            over.max(under) > 1.0,
                            "Co = {co:.2} is above the explicit term's stability \
                             limit, so the explicit mode is expected to diverge; \
                             if it no longer does, this case no longer \
                             discriminates the two modes and the implicit \
                             assertion above proves nothing. overshoot {:+.2} %, \
                             undershoot {:+.2} % of span; field {:?}",
                            100.0 * over,
                            100.0 * under,
                            exp.he.internal.as_slice(),
                        );
                    } else {
                        assert!(
                            over < 0.05 && under < 0.05,
                            "Co = {co:.2} is at (not above) the explicit stability \
                             limit, so the explicit mode should still be bounded; \
                             overshoot {:+.2} %, undershoot {:+.2} % of span",
                            100.0 * over,
                            100.0 * under,
                        );
                    }
                }
            }
        }
    }

    /// ## Methodology
    /// The **price** of [`EnergyBalanceMode::Implicit`], measured where the
    /// explicit mode is perfectly happy — because a mode that were strictly better
    /// would not need to be selectable, and the honest reading of this pair is
    /// that each wins in a different regime.
    ///
    /// Same fixture and transient as
    /// [`inlet_enthalpy_bc_heats_uniform_field_without_reapplying_each_step`]:
    /// 8 cells, 1 m, 10⁻⁴ m², nitrogen at 1 bar, 0.5 m/s, `dt = 2×10⁻⁴ s`
    /// (`Co = 8×10⁻⁴`), 3000 steps = 0.6 s, both temperature directions
    /// (300 K → 500 K inlet, and 500 K → 300 K). The two modes are run with the
    /// same default van Leer limiter and compared cell by cell. Pass criterion:
    /// the largest per-cell enthalpy difference is under 0.5 % of the 209.26 kJ/kg
    /// imposed step — i.e. the deferred correction really does recover the
    /// limiter, rather than the implicit mode quietly degrading to upwind.
    ///
    /// ## Results (2026-08-13)
    /// Passes. Largest per-cell difference **0.068 kJ/kg (0.033 % of span)**
    /// heating and **0.135 kJ/kg (0.064 % of span)** cooling. Profiles, first four
    /// cells, heating direction:
    ///
    /// - explicit / van Leer: `519.48, 476.19, 362.94, 314.38 kJ/kg`
    /// - implicit / van Leer: `519.47, 476.13, 362.99, 314.42 kJ/kg`
    /// - implicit / upwind (deferred correction identically zero, for scale):
    ///   `501.46, 446.46, 380.74, 338.02 kJ/kg`
    ///
    /// The implicit/upwind row is the point: dropping the limiter costs
    /// **18 kJ/kg (8.6 % of span)** in the inlet cell, 300× the cost of the
    /// implicit *time* treatment at this step. So the deferred correction is
    /// carrying essentially all of the limiter's value.
    ///
    /// ## Where implicit IS worse — the honest part
    /// This near-agreement is a small-Courant result and does not generalise. The
    /// numerical diffusion of an upwind scheme scales as `(u·Δx/2)·(1 − Co)`
    /// explicit and `(u·Δx/2)·(1 + Co)` implicit, so the modes coincide only as
    /// `Co → 0` and diverge as `Co` grows — the explicit front *sharpens* toward
    /// `Co = 1` while the implicit one smears. Measured on this same pipe after
    /// **2 s** (one pipe transit at 0.5 m/s), outlet-cell enthalpy against a
    /// 311.20 kJ/kg seed:
    ///
    /// | `Co` | `Δt` × steps | Explicit | Implicit |
    /// |---|---|---|---|
    /// | 8×10⁻⁴ | 2×10⁻⁴ s × 10 000 | 325.78 kJ/kg | 325.85 kJ/kg |
    /// | 0.25 | 6.25×10⁻² s × 32 | 312.68 kJ/kg | 330.03 kJ/kg |
    /// | 0.50 | 0.125 s × 16 | 311.22 kJ/kg | 337.27 kJ/kg |
    ///
    /// At `Co = 0.5` the implicit front has smeared roughly two cells further
    /// downstream than the explicit one. **Below `Co ≈ 1` the explicit mode is the
    /// more accurate choice, and that is why it remains the default.**
    ///
    /// Raising the outer-corrector count does **not** recover it: measured at
    /// `Co = 0.5` after **4 s** (0.125 s × 32), implicit outlet-cell enthalpy is
    /// 517.71 / 517.50 / 517.64 / 517.65 kJ/kg at
    /// 1 / 2 / 4 / 8 outer correctors against the explicit 520.26 kJ/kg. The
    /// residual gap is the *temporal* implicitness, not the deferred correction's
    /// Picard lag, so no number of correctors closes it.
    #[test]
    fn energy_balance_modes_agree_at_small_courant() {
        for &(t_seed, t_bc) in &[(300.0_f64, 500.0_f64), (500.0, 300.0)] {
            let span = (bc_enthalpy(t_bc) - bc_enthalpy(t_seed)).abs();

            let mut exp = courant_fixture(
                BC_DT,
                t_seed,
                t_bc,
                EnergyBalanceMode::Explicit,
                EnergyConvectionScheme::VanLeer,
            );
            let mut imp = courant_fixture(
                BC_DT,
                t_seed,
                t_bc,
                EnergyBalanceMode::Implicit,
                EnergyConvectionScheme::VanLeer,
            );
            exp.run(3000);
            imp.run(3000);

            let worst = (0..BC_N)
                .map(|c| (exp.he.internal[c] - imp.he.internal[c]).abs())
                .fold(0.0_f64, f64::max);
            assert!(
                worst < 0.005 * span,
                "at Co = {:.1e} the deferred correction should reproduce the \
                 explicit limited scheme to well under 0.5 % of the {:.2} kJ/kg \
                 span ({t_seed} K -> {t_bc} K); worst per-cell difference \
                 {:.3} kJ/kg = {:.3} % of span\n  explicit {:?}\n  implicit {:?}",
                CO_U * BC_DT / CO_DX,
                span / 1e3,
                worst / 1e3,
                100.0 * worst / span,
                exp.he.internal.as_slice(),
                imp.he.internal.as_slice(),
            );
        }
    }

    /// ## Methodology
    /// [`EnergyBalanceMode::Implicit`] must not cost the boundedness that bead
    /// `op-1fyp` was opened for. A nonlinear flux limiter cannot be written into a
    /// linear matrix, so a naive `fvc::div_limited → fvm::div` swap would silently
    /// demote the default van Leer scheme to first-order upwind; the deferred
    /// correction exists to prevent that, and this test is what checks it did.
    ///
    /// The implicit-mode twin of
    /// [`energy_convection_is_bounded_at_an_advected_front`], with the identical
    /// case and the identical pass criterion: 8 cells, 1 m, nitrogen at 1 bar,
    /// 0.5 m/s, `dt = 2×10⁻⁴ s`, both directions (300 K ⇄ 500 K), at 3000 and
    /// 10 000 steps. The enthalpy field must stay inside the range spanned by its
    /// own seed and inlet BC to within 1 % of that span.
    ///
    /// ## Results (2026-08-13)
    /// Passes in both directions and at both step counts. At 3000 steps the
    /// heating case peaks at **0.47 % of span *below*** the 520.45 kJ/kg boundary
    /// value (no overshoot at all) and the cooling case at 0.58 % of span above
    /// the 311.20 kJ/kg one. Contrast the same case under
    /// [`EnergyConvectionScheme::Linear`] in the explicit sibling test, which
    /// overshoots by 10-15 % of span — the limiter is intact through the deferred
    /// correction, not bypassed by it.
    #[test]
    fn implicit_mode_preserves_boundedness_at_an_advected_front() {
        for &(t_seed, t_bc) in &[(300.0_f64, 500.0_f64), (500.0, 300.0)] {
            let lo = bc_enthalpy(t_seed).min(bc_enthalpy(t_bc));
            let hi = bc_enthalpy(t_seed).max(bc_enthalpy(t_bc));
            for &steps in &[3000usize, 10_000] {
                let mut arr = courant_fixture(
                    BC_DT,
                    t_seed,
                    t_bc,
                    EnergyBalanceMode::Implicit,
                    EnergyConvectionScheme::VanLeer,
                );
                arr.run(steps);
                let (over, under) = excursion(&arr, lo, hi).expect("field went non-finite");
                assert!(
                    over < 0.01 && under < 0.01,
                    "implicit mode with the default van Leer limiter must stay \
                     inside the {:.2}..{:.2} kJ/kg range set by the seed and the \
                     inlet BC ({t_seed} K -> {t_bc} K, {steps} steps): overshoot \
                     {:+.2} %, undershoot {:+.2} % of span; field {:?}",
                    lo / 1e3,
                    hi / 1e3,
                    100.0 * over,
                    100.0 * under,
                    arr.he.internal.as_slice(),
                );
            }
        }
    }

    /// ## Methodology
    /// The implicit-mode twin of
    /// [`steady_energy_balance_closes_at_the_boundaries`], because moving a term
    /// from the source vector into the matrix is exactly the kind of change that
    /// can lose conservation without losing stability — and a heat exchanger that
    /// is stable but does not conserve energy is worse than one that visibly
    /// blows up.
    ///
    /// Identical case to that test: 8 cells, prescribed 1.0 g/s inlet mass flow,
    /// prescribed 300 K inlet enthalpy, 200 W spread uniformly over the cells,
    /// 4000 steps of 2×10⁻⁴ s to steady state, with
    /// [`EnergyBalanceMode::Implicit`] selected. Two forms are checked: the
    /// component form `ṁ·(h_out − h_in) = Q` to 1 %, and the strict boundary form
    /// `(ṁ_in·h_in − ṁ_out·h_out) + Q = 0` to 0.1 % of `Q`, both built from the
    /// solved boundary mass fluxes.
    ///
    /// ## Results (2026-08-13)
    /// Passes, and is indistinguishable from the explicit path: component form
    /// `199.9878 W` against 200 W (−0.006 %); boundary-form residual
    /// `+0.0122 W` = **+0.006 % of Q**, the same figures the explicit sibling
    /// records. Conservation is preserved because `fvm::div` is assembled in flux
    /// form — every internal face contributes `+φ_f·h_f` to its owner and
    /// `−φ_f·h_f` to its neighbour — and the deferred correction is a difference
    /// of two flux-form divergences, so it telescopes to zero over the domain as
    /// well.
    ///
    /// This test does **not** rehabilitate the known *transient* imbalance
    /// documented in
    /// [`transient_energy_imbalance_is_the_enthalpy_of_the_unconserved_mass`];
    /// that residual is a missing `∂ρ/∂h|_p` term in the pressure equation and is
    /// untouched by the convection treatment.
    #[test]
    fn implicit_mode_closes_the_steady_energy_balance() {
        let target = 1.0e-3_f64;
        let q_watt = 200.0_f64;
        let h_in = bc_enthalpy(300.0);

        let mut arr = bc_fixture(300.0);
        arr.set_he_balance_mode(EnergyBalanceMode::Implicit);
        arr.set_inlet_enthalpy(AvailableEnergy::new::<joule_per_kilogram>(h_in));
        arr.set_outlet_enthalpy(AvailableEnergy::new::<joule_per_kilogram>(h_in));
        arr.set_inlet_mass_flowrate(MassRate::new::<kilogram_per_second>(target));
        for _ in 0..4000 {
            arr.lateral_link_new_power_vector(
                Power::new::<watt>(q_watt),
                vec![1.0 / BC_N as f64; BC_N],
            )
            .unwrap();
            arr.step();
        }

        let m_in = arr
            .get_inlet_mass_flowrate_actual()
            .get::<kilogram_per_second>();
        let h_out = arr.get_outlet_enthalpy().get::<joule_per_kilogram>();
        let q_component = m_in * (h_out - h_in);
        assert!(
            ((q_component - q_watt) / q_watt).abs() < 0.01,
            "implicit-mode component energy balance: mdot*(h_out - h_in) = \
             {q_component:.4} W against Q = {q_watt} W"
        );

        let residual = net_enthalpy_in(&arr) + q_watt;
        assert!(
            (residual / q_watt).abs() < 0.001,
            "implicit-mode boundary energy balance should close to better than \
             0.1 % of Q; residual = {residual:.4} W against Q = {q_watt} W"
        );
    }
}
