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

use uom::si::f64::{Angle, Length, MassRate, Power, Pressure, ThermalConductance, ThermodynamicTemperature};
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::power::watt;

use crate::flash;

use super::OPCPFluidArray;

/// Errors from [`OPCPFluidArray`]'s lateral-coupling / bookkeeping interface.
#[derive(Debug, Clone, PartialEq)]
pub enum OPCPFluidArrayError {
    /// A per-cell vector argument did not have length `mesh.n_cells`.
    LengthMismatch {
        /// Name of the offending argument.
        array: &'static str,
        expected: usize,
        got: usize,
    },
}

impl std::fmt::Display for OPCPFluidArrayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LengthMismatch { array, expected, got } => write!(
                f,
                "{array} has length {got}, expected {expected} (mesh.n_cells)"
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
        self.lateral_adjacent_array_temperature_vector.push(temperature_vec);
        self.lateral_adjacent_array_conductance_vector.push(conductance_vec);
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
            Err(OPCPFluidArrayError::LengthMismatch { expected: 5, got: 3, .. })
        ));
    }

    #[test]
    fn lateral_link_rejects_wrong_length_q_fraction_vec() {
        let mut arr = test_array(5);
        let bad = vec![0.2; 4];
        let result = arr.lateral_link_new_power_vector(Power::new::<watt>(100.0), bad);
        assert!(matches!(
            result,
            Err(OPCPFluidArrayError::LengthMismatch { expected: 5, got: 4, .. })
        ));
    }

    #[test]
    fn lateral_heat_source_raises_temperature_over_a_step() {
        let mut arr = test_array(5);
        arr.correct_transport();
        let t_before: f64 = arr.get_temperature_vector().iter().map(|t| t.get::<kelvin>()).sum();

        let fractions = vec![0.2; 5];
        arr.lateral_link_new_power_vector(Power::new::<watt>(500.0), fractions).unwrap();
        arr.step();
        // `t`/`rho` lag `he` by one outer-corrector iteration (they're refreshed
        // from the *previous* `he` before the energy equation solves for the new
        // one) — sync them from the fresh `he` before reading the temperature.
        arr.correct_thermo();

        let t_after: f64 = arr.get_temperature_vector().iter().map(|t| t.get::<kelvin>()).sum();
        assert!(t_after > t_before, "t_after={t_after} should exceed t_before={t_before}");
    }

    #[test]
    fn lateral_conductance_cools_toward_colder_neighbour() {
        let mut arr = test_array(5);
        arr.correct_transport();
        let t_before: f64 = arr.get_temperature_vector().iter().map(|t| t.get::<kelvin>()).sum();

        let colder = vec![ThermodynamicTemperature::new::<kelvin>(250.0); 5];
        arr.lateral_link_new_temperature_vector_avg_conductance(
            ThermalConductance::new::<watt_per_kelvin>(50.0),
            colder,
        )
        .unwrap();
        arr.step();
        arr.correct_thermo(); // sync `t` from the fresh `he` -- see the comment above

        let t_after: f64 = arr.get_temperature_vector().iter().map(|t| t.get::<kelvin>()).sum();
        assert!(t_after < t_before, "t_after={t_after} should be below t_before={t_before}");
    }

    #[test]
    fn clear_vectors_empties_after_step() {
        let mut arr = test_array(5);
        arr.correct_transport();
        let fractions = vec![0.2; 5];
        arr.lateral_link_new_power_vector(Power::new::<watt>(100.0), fractions).unwrap();
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
}
