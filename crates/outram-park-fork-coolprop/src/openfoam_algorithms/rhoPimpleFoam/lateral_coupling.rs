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
use crate::openfoam_algorithms::openfoam_source::{PatchField, Vector3};

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
    /// **Accuracy caveat.** Unlike the inlet value, this is the *predictor*
    /// flux `φ_HbyA` at the boundary: [`super::OPCPFluidArray::step`] applies
    /// the pressure correction `−ρ_f·rAU_f·snGrad(p)·|Sf|` to internal faces
    /// only, so a fixed-pressure outlet's boundary flux carries the
    /// pre-correction value. Compared against
    /// [`Self::get_inlet_mass_flowrate_actual`] it is therefore a *global mass
    /// balance* indicator, not an exact conservation identity — the difference
    /// is genuine solver imbalance plus this boundary-correction gap.
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

    /// Prescribes a fixed inlet specific-enthalpy boundary condition on
    /// the `"left"` patch (x = 0) -- pairs with [`Self::set_inlet_velocity`]
    /// (or [`Self::set_inlet_mass_flowrate`]) to fully specify the inlet
    /// thermodynamic state.
    ///
    /// The boundary condition **persists across steps**: it enters the energy
    /// equation both through the convective inflow `∇·(φh)` and through the
    /// enthalpy diffusion term, and [`super::OPCPFluidArray::step`] re-stamps
    /// the BC template after the energy solve (which, like every linear solve
    /// here, returns a zero-gradient-bounded field). Before that re-stamp was
    /// added it was a **one-shot**: measured 2026-08-12 over 10 000 steps of
    /// 2×10⁻⁴ s (Nitrogen, 8 cells, 1 m, 0.5 m/s, seed 300 K = 311.20 kJ/kg,
    /// inlet BC 520.45 kJ/kg), no cell moved more than 0.4 kJ/kg from the seed
    /// unless the caller re-issued this call every step. See the module tests
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
    /// ## Results (2026-08-12)
    /// Passes. `he[0] = 556.52 kJ/kg` (gap closed 117.2 %; the overshoot past
    /// the BC value is dispersive ringing, see the note below), field mean
    /// `364.98 kJ/kg` versus the 311.20 kJ/kg seed.
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
    /// ## Results (2026-08-12)
    /// Passes: `he[0] = 296.13 kJ/kg` (gap closed 107.2 %, again with dispersive
    /// undershoot past the BC), field mean `444.68 kJ/kg` versus the
    /// 520.45 kJ/kg seed. Fails on the pre-fix code for the same reason as the
    /// heating case (measured pre-fix at 2000 steps: `he[0] = 520.16 kJ/kg`,
    /// 0.14 % of the gap).
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
    /// Passes. (a) `9.999122e-4 kg/s`, −0.0088 %; (b) `2.000597e-3 kg/s`,
    /// **+100.06 %** — the assumed-density error transfers one-for-one into the
    /// imposed mass flow; (c) `1.000000000000e-3 kg/s`, relative error exactly
    /// 0.0 (by construction: the velocity is re-derived each corrector from the
    /// very face density that multiplies it).
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
}
