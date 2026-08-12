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
    /// left-to-right, +x) -- takes effect on the next [`super::OPCPFluidArray::step`].
    pub fn set_inlet_velocity(&mut self, velocity: Velocity) {
        let size = self.mesh.patches[1].size;
        let v = Vector3::new(velocity.get::<meter_per_second>(), 0.0, 0.0);
        self.u.boundary[1] = PatchField::fixed_value_vec(size, v);
    }

    /// Prescribes a fixed inlet specific-enthalpy boundary condition on
    /// the `"left"` patch (x = 0) -- pairs with [`Self::set_inlet_velocity`]
    /// to fully specify the inlet thermodynamic state.
    pub fn set_inlet_enthalpy(&mut self, h: AvailableEnergy) {
        let size = self.mesh.patches[1].size;
        self.he.boundary[1] = PatchField::fixed_value(size, h.get::<joule_per_kilogram>());
    }

    /// Prescribes a fixed outlet pressure boundary condition on the
    /// `"right"` patch (x = length) -- e.g. the downstream pressure a
    /// turbine or condenser imposes.
    pub fn set_outlet_pressure(&mut self, p: Pressure) {
        let size = self.mesh.patches[0].size;
        self.p.boundary[0] = PatchField::fixed_value(size, p.get::<pascal>());
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
}
