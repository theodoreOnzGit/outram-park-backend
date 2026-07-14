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

    /// Prescribes a fixed inlet velocity boundary condition on the
    /// `"left"` patch (x = 0, see [`crate::openfoam_algorithms::openfoam_source::interface::one_dimensional_meshing::create_one_d_mesh`]).
    ///
    /// For driving this array as a simple pipe/tube with a known inlet
    /// flow (e.g. from an upstream pump). `velocity` is the x-direction
    /// flow speed; positive means fluid entering the domain (flowing
    /// left-to-right, +x) -- take effect on the next [`super::TampinesSteamArray::step`].
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
        let inlet_enthalpy = crate::interfaces::functional_programming::pt_flash_eqm::h_tp_eqm_single_phase(
            preset_temp, outlet_pressure,
        );
        arr.set_inlet_velocity(inlet_velocity);
        arr.set_inlet_enthalpy(inlet_enthalpy);
        arr.set_outlet_pressure(outlet_pressure);

        arr.run(200);

        let all_finite = arr.u.internal.as_slice().iter().all(|v| v.mag().is_finite())
            && arr.p.internal.as_slice().iter().all(|x| x.is_finite())
            && arr.he.internal.as_slice().iter().all(|x| x.is_finite());
        assert!(all_finite, "fields must stay finite when driven by prescribed inlet/outlet BCs");

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
        let mean_u_x: f64 = arr.u.internal.as_slice().iter().map(|v| v.x).sum::<f64>()
            / arr.mesh.n_cells as f64;
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
        let inlet_enthalpy = crate::interfaces::functional_programming::pt_flash_eqm::h_tp_eqm_single_phase(
            preset_temp, outlet_pressure,
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
