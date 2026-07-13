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
//! fake it the way a cross-crate port might. This does **not** change
//! [`super::TampinesSteamArray::correct_thermo`]'s existing placeholder
//! `ρ = ψ·p` EOS used by [`super::TampinesSteamArray::step`] -- wiring the
//! real steam tables into the PIMPLE loop's own thermodynamic closure is a
//! separate, larger task (see that method's own doc comment).

use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{
    Angle, Length, MassRate, Power, Pressure, ThermalConductance, ThermodynamicTemperature,
};
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermodynamic_temperature::kelvin;

use crate::interfaces::functional_programming::pt_flash_eqm::{
    h_tp_eqm_single_phase, kappa_t_tp_eqm, v_tp_eqm_single_phase,
};

use super::TampinesSteamArray;

/// Errors from [`TampinesSteamArray`]'s lateral-coupling / bookkeeping
/// interface.
#[derive(Debug, Clone, PartialEq)]
pub enum TampinesSteamArrayError {
    /// A per-cell vector argument did not have length `mesh.n_cells`.
    LengthMismatch {
        /// Name of the offending argument.
        array: &'static str,
        expected: usize,
        got: usize,
    },
}

impl std::fmt::Display for TampinesSteamArrayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LengthMismatch { array, expected, got } => write!(
                f,
                "{array} has length {got}, expected {expected} (mesh.n_cells)"
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

    /// Bulk mass flowrate \[kg/s\] (plain storage — see the field's doc comment
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
    /// [`super::TampinesSteamArray::correct_thermo`] call (which still uses
    /// the placeholder `ρ = ψ·p` EOS, not this flash -- see this module's
    /// doc). `psi` here is set from the *real* isothermal compressibility
    /// `ψ = ρ·κ_T`, so a caller reading it back after this call sees better
    /// physics than `correct_thermo` alone provides.
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
            Err(TampinesSteamArrayError::LengthMismatch { expected: 5, got: 3, .. })
        ));
    }

    #[test]
    fn lateral_link_rejects_wrong_length_q_fraction_vec() {
        let mut arr = test_array(5);
        let bad = vec![0.2; 4];
        let result = arr.lateral_link_new_power_vector(Power::new::<watt>(100.0), bad);
        assert!(matches!(
            result,
            Err(TampinesSteamArrayError::LengthMismatch { expected: 5, got: 4, .. })
        ));
    }

    #[test]
    fn clear_vectors_empties_after_step() {
        let mut arr = test_array(5);
        let fractions = vec![0.2; 5];
        arr.lateral_link_new_power_vector(Power::new::<watt>(100.0), fractions).unwrap();
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
        arr.set_mass_flowrate(MassRate::new::<uom::si::mass_rate::kilogram_per_second>(2.5));
        arr.set_pressure_loss(Pressure::new::<pascal>(500.0));
        arr.set_internal_pressure_source(Pressure::new::<pascal>(1000.0));
        arr.set_incline_angle(Angle::new::<uom::si::angle::radian>(0.1));

        assert!((arr.get_mass_flowrate().get::<uom::si::mass_rate::kilogram_per_second>() - 2.5).abs() < 1e-9);
        assert!((arr.get_pressure_loss().get::<pascal>() - 500.0).abs() < 1e-9);
        assert!((arr.get_internal_pressure_source().get::<pascal>() - 1000.0).abs() < 1e-9);
        assert!((arr.get_incline_angle().get::<uom::si::angle::radian>() - 0.1).abs() < 1e-9);
    }
}
