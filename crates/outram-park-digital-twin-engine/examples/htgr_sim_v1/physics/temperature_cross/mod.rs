//! Remedies for a temperature cross in the nodalised counter-flow steam
//! generator.
//!
//! # Why this module exists
//!
//! The exchanger is **three separate matrix systems** — hot fluid array, tube
//! metal [`SolidColumn`](tuas_boussinesq_solver::pre_built_components::one_d_solid_structure),
//! cold fluid array — coupled through conductance terms evaluated at the
//! previous iterate. That inter-array coupling is **explicit (Lie-split)**
//! however implicit each array's own convection is.
//!
//! At a coarse enough coupling step the counter-flow arrangement therefore
//! cannot resolve, and the cold stream overtakes the hot: a **temperature
//! cross**, which violates the second law. Measured 2026-08-13 on `htgr_sim_v1`
//! with the helium side already implicit, it appears at 1 sub-step per 0.1 s
//! plant step (`Co_hot` = 1.776) and not at 2 (`Co_hot` = 0.886).
//!
//! A fully implicit solve spanning all three arrays would remove the problem at
//! the root, at the cost of replacing three cheap tridiagonal solves with one
//! large coupled system. The maintainer has explicitly rejected that trade.
//!
//! # The governing idea
//!
//! **Adopt a thermodynamically admissible steady profile when a cross is
//! observed.** Thermodynamics is then not violated, even though the transient
//! differs from a fully-resolved one. Degrade to a physically valid state
//! rather than crash or emit nonsense.
//!
//! This is a **heuristic and a deliberate fidelity trade**, not a solver
//! improvement. Anything computed while a remedy is engaged is *not* a resolved
//! transient and must never be reported as one. See
//! `docs/heat-exchanger-temperature-cross-fallback.md` for the full design
//! discussion, and [`TemperatureCrossRemedy`] for what each method does.
//!
//! # Default
//!
//! [`TemperatureCrossRemedy::None`] — detect, count and report, repair nothing.
//! The shipped simulator runs 2 sub-steps per plant step, where no cross
//! occurs, so no remedy is engaged in normal operation. The remedies exist for
//! coarser coupling than that.

// DELETE THIS WHEN DISPATCH IS WIRED INTO `steam_generator.rs`.
//
// Nothing outside the tests calls `TemperatureCrossRemedy::apply` yet, so every
// item in this module and its three children reads as dead to the compiler. The
// three remedy modules carry the same allow for the same reason and with the
// same instruction. Once the exchanger consults a remedy on detecting a cross,
// all four should come off and any warning that survives is then a genuine
// unused item worth deleting.
#![allow(dead_code)]

use uom::si::f64::*;

pub mod bedok_enthalpy_march;
pub mod eliminate_metal;
pub mod lmtd_profile;

/// How the exchanger should respond when a temperature cross is detected.
///
/// Enum dispatch, not trait objects, per the workspace design rules: the set of
/// remedies is closed and known at compile time, and adding one should force
/// every match site to be revisited.
///
/// The variants are ordered by increasing intervention. Each does strictly more
/// to the state than the one before it, and correspondingly departs further
/// from the transient the arrays were computing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TemperatureCrossRemedy {
    /// **Do nothing but observe.** The default.
    ///
    /// Detect the cross, count it, and report it through
    /// [`CrossRepair::events`]. The state is handed to the next timestep
    /// unmodified, so a cross will propagate and any second-law assertion
    /// downstream will fire.
    ///
    /// This is the right choice whenever the coupling step is fine enough that
    /// a cross indicates a **bug rather than coarseness** — which is the case
    /// for the shipped 2 sub-steps per plant step. A remedy engaging there
    /// would convert a loud, informative failure into a quiet wrong answer.
    #[default]
    None,

    /// **Enthalpy march** — the method translated from the BEDOK reference.
    ///
    /// Marches mixture enthalpy along each stream from a pure energy balance
    /// and rebuilds both temperature profiles from the result. Because it works
    /// in enthalpy rather than temperature, it passes through the boiling
    /// transition natively: there are no zone boundaries to locate and no
    /// per-zone heat transfer coefficient to choose.
    ///
    /// # Attribution
    ///
    /// The enthalpy-march formulation follows `singleflow1devap.m` by
    /// **Than Yan Ren** (Singapore Nuclear Research and Safety Institute),
    /// translated into this workspace as
    /// [`bedok::reference::th::single_flow_evap`] and used here with the
    /// author's permission, consistent with the rest of the BEDOK port. The
    /// IAPWS calls go through this workspace's own IF97 implementation rather
    /// than a third-party `IAPWS_IF97.m`.
    ///
    /// Implemented in [`bedok_enthalpy_march`].
    Bedok,

    /// **Impose an LMTD-consistent steady profile.**
    ///
    /// Computes steady-state outlet temperatures from the log-mean temperature
    /// difference and adjusts both arrays' profiles to match.
    ///
    /// # Read the zoning caveat before using this
    ///
    /// LMTD assumes constant specific heats, constant `U` and **no phase
    /// change**. A once-through steam generator violates all three across the
    /// boiling boundary. A single-zone LMTD applied across that boundary will
    /// produce a confidently wrong profile — worse than a crash, because it
    /// looks reasonable. Correct use requires **zoning** the exchanger
    /// (subcooled / two-phase / superheat) with a per-zone `U`.
    ///
    /// Note the compensating nuance: *within* a two-phase zone one stream is
    /// isothermal, which makes LMTD **exact** there for constant `U`, with the
    /// multipass correction `F = 1` regardless of flow arrangement. LMTD is not
    /// wrong in two-phase; it is wrong *across* the zone boundaries.
    ///
    /// Implemented in [`lmtd_profile`].
    Lmtd,

    /// **Eliminate the tube metal from the coupling loop.**
    ///
    /// The metal has no advection — it is a per-node capacitance with two
    /// conductances — so it can be removed analytically (a Schur complement),
    /// coupling hot and cold nodes directly through the series thermal
    /// resistance. The metal temperature is then *set* to the value implied at
    /// that steady state rather than integrated.
    ///
    /// This takes one full lag out of the inter-array coupling loop, which in a
    /// counter-flow arrangement is where the cross originates. It is the least
    /// invasive of the three repairs: it does not impose a steady profile on
    /// the fluid streams, only on the metal.
    ///
    /// Implemented in [`eliminate_metal`].
    EliminateMetal,
}

/// Everything a remedy needs to repair a crossed profile, and nothing else.
///
/// Node ordering follows the exchanger's own convention: index 0 is the **hot
/// inlet** end. The cold stream flows the other way, so the cold inlet is the
/// **last** index — this is a counter-flow exchanger and getting that backwards
/// silently inverts the profile.
///
/// All temperatures are `uom`-typed. Conductances are per node, already divided
/// by the node count, in watts per kelvin.
#[derive(Clone, Debug)]
pub struct CrossRepairInputs {
    /// Hot-stream node temperatures, hot inlet first.
    pub hot_temperatures: Vec<ThermodynamicTemperature>,
    /// Tube-metal node temperatures, same ordering.
    pub metal_temperatures: Vec<ThermodynamicTemperature>,
    /// Cold-stream node temperatures, same ordering — so the cold *inlet* is
    /// the last element.
    pub cold_temperatures: Vec<ThermodynamicTemperature>,
    /// Hot-side film conductance per node.
    pub hot_node_conductance: ThermalConductance,
    /// Cold-side film conductance per node.
    pub cold_node_conductance: ThermalConductance,
    /// Hot-stream mass flow.
    pub hot_mass_flow: MassRate,
    /// Cold-stream mass flow.
    pub cold_mass_flow: MassRate,
    /// Hot-stream inlet temperature (the boundary condition, not node 0's
    /// current value).
    pub hot_inlet_temperature: ThermodynamicTemperature,
    /// Cold-stream inlet specific enthalpy (the boundary condition). Enthalpy
    /// rather than temperature because the cold side may be two-phase, where
    /// temperature does not determine state.
    pub cold_inlet_enthalpy: AvailableEnergy,
    /// Hot-side operating pressure.
    pub hot_pressure: Pressure,
    /// Cold-side operating pressure.
    pub cold_pressure: Pressure,
}

impl CrossRepairInputs {
    /// Number of nodes. All three profiles are the same length by construction.
    pub fn node_count(&self) -> usize {
        self.hot_temperatures.len()
    }

    /// Worst temperature cross in the supplied profiles \[K\]: the largest
    /// amount by which the cold stream exceeds the hot at any node. Zero or
    /// negative means no cross.
    pub fn worst_cross_kelvin(&self) -> f64 {
        use uom::si::thermodynamic_temperature::kelvin;
        self.hot_temperatures
            .iter()
            .zip(self.cold_temperatures.iter())
            .fold(0.0_f64, |worst, (h, c)| {
                worst.max(c.get::<kelvin>() - h.get::<kelvin>())
            })
    }
}

/// A repaired set of profiles, plus the bookkeeping needed to keep the repair
/// honest.
#[derive(Clone, Debug)]
pub struct CrossRepairOutcome {
    /// Repaired hot-stream node temperatures.
    pub hot_temperatures: Vec<ThermodynamicTemperature>,
    /// Repaired tube-metal node temperatures.
    pub metal_temperatures: Vec<ThermodynamicTemperature>,
    /// Repaired cold-stream node temperatures.
    pub cold_temperatures: Vec<ThermodynamicTemperature>,
    /// **First-law bookkeeping.** Overwriting a profile changes the energy
    /// stored in the arrays discontinuously. The second law is respected
    /// pointwise by construction, but energy is not conserved across the jump.
    ///
    /// This records the discrepancy so it is auditable rather than invisible: a
    /// running total that grows steadily means the remedy is doing real
    /// violence to the energy balance, whatever the temperature profiles look
    /// like. Positive means the repair *added* energy to the exchanger.
    pub energy_discrepancy: Energy,
}

/// Why a remedy could not be applied.
///
/// A remedy that cannot run must say so rather than silently returning the
/// input unchanged — a silent no-op would look identical to
/// [`TemperatureCrossRemedy::None`] while claiming to be a repair.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CrossRepairError {
    /// The remedy is selected but not yet implemented.
    NotImplemented(&'static str),
    /// The inputs were not usable — mismatched profile lengths, a non-positive
    /// flow, a non-finite temperature.
    BadInputs(String),
    /// The remedy ran but could not produce a cross-free profile. The caller
    /// should escalate or fail loudly; it must **not** treat this as success.
    DidNotConverge(String),
}

impl core::fmt::Display for CrossRepairError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotImplemented(what) => {
                write!(f, "temperature-cross remedy not implemented: {what}")
            }
            Self::BadInputs(why) => write!(f, "temperature-cross remedy got bad inputs: {why}"),
            Self::DidNotConverge(why) => {
                write!(f, "temperature-cross remedy did not converge: {why}")
            }
        }
    }
}

impl TemperatureCrossRemedy {
    /// Apply this remedy to a crossed profile.
    ///
    /// Returns `Ok(None)` for [`Self::None`], which observes but does not
    /// repair. Returns `Ok(Some(outcome))` when a repair was produced, and the
    /// caller is responsible for writing it back into the arrays and adding
    /// [`CrossRepairOutcome::energy_discrepancy`] to its running total.
    ///
    /// # Errors
    ///
    /// [`CrossRepairError`] if the remedy is unimplemented, the inputs are
    /// unusable, or it ran and could not produce a cross-free profile. **A
    /// caller must not treat an error as "no repair needed".**
    pub fn apply(
        &self,
        inputs: &CrossRepairInputs,
    ) -> Result<Option<CrossRepairOutcome>, CrossRepairError> {
        match self {
            Self::None => Ok(None),
            Self::Bedok => bedok_enthalpy_march::repair(inputs).map(Some),
            Self::Lmtd => lmtd_profile::repair(inputs).map(Some),
            Self::EliminateMetal => eliminate_metal::repair(inputs).map(Some),
        }
    }

    /// Short human-readable name, for GUI readouts and log lines.
    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "none (observe only)",
            Self::Bedok => "BEDOK enthalpy march (Than Yan Ren)",
            Self::Lmtd => "LMTD steady profile",
            Self::EliminateMetal => "eliminate tube metal",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::available_energy::joule_per_kilogram;
    use uom::si::mass_rate::kilogram_per_second;
    use uom::si::power::watt;
    use uom::si::pressure::pascal;
    use uom::si::thermodynamic_temperature::kelvin;

    /// A three-node exchanger whose middle node is crossed by 5 K.
    fn crossed_inputs() -> CrossRepairInputs {
        let k = |t: f64| ThermodynamicTemperature::new::<kelvin>(t);
        CrossRepairInputs {
            hot_temperatures: vec![k(900.0), k(700.0), k(600.0)],
            metal_temperatures: vec![k(800.0), k(690.0), k(580.0)],
            cold_temperatures: vec![k(690.0), k(705.0), k(500.0)],
            hot_node_conductance: ThermalConductance::new::<
                uom::si::thermal_conductance::watt_per_kelvin,
            >(1.0e4),
            cold_node_conductance: ThermalConductance::new::<
                uom::si::thermal_conductance::watt_per_kelvin,
            >(3.0e4),
            hot_mass_flow: MassRate::new::<kilogram_per_second>(4.3),
            cold_mass_flow: MassRate::new::<kilogram_per_second>(3.2),
            hot_inlet_temperature: k(973.15),
            cold_inlet_enthalpy: AvailableEnergy::new::<joule_per_kilogram>(4.0e5),
            hot_pressure: Pressure::new::<pascal>(3.0e6),
            cold_pressure: Pressure::new::<pascal>(4.0e6),
        }
    }

    /// Methodology: build a three-node profile whose middle node has the cold
    /// stream 5 K above the hot, and confirm the detector reports exactly that.
    /// Pass criterion: `worst_cross_kelvin()` within 1e-12 of 5.0.
    ///
    /// # Results (2026-08-13)
    ///
    /// Reported 5.0 K exactly. Interpretation: the detector the remedies key
    /// off measures what it claims to, so a remedy firing or not firing is
    /// attributable.
    #[test]
    fn the_detector_reports_the_worst_cross() {
        let inputs = crossed_inputs();
        assert!((inputs.worst_cross_kelvin() - 5.0).abs() < 1e-12);
        assert_eq!(inputs.node_count(), 3);
    }

    /// Methodology: apply [`TemperatureCrossRemedy::None`] to a crossed profile
    /// and confirm it returns no repair. Pass criterion: `Ok(None)`.
    ///
    /// # Results (2026-08-13)
    ///
    /// `Ok(None)`, profiles untouched. Interpretation: the default observes
    /// without intervening, so a cross at the shipped sub-step count still
    /// surfaces as a loud failure rather than being quietly absorbed.
    #[test]
    fn the_default_remedy_repairs_nothing() {
        let inputs = crossed_inputs();
        assert_eq!(
            TemperatureCrossRemedy::default(),
            TemperatureCrossRemedy::None
        );
        assert!(TemperatureCrossRemedy::None
            .apply(&inputs)
            .unwrap()
            .is_none());
    }

    /// Methodology: dispatch every variant against a genuinely crossed profile
    /// and assert each behaves as its documentation claims -- no variant may
    /// silently return the input unchanged while presenting as a repair. Pass
    /// criterion: each outcome matches the variant's documented contract, and
    /// any variant that returns a repaired profile has actually removed the
    /// cross.
    ///
    /// # Results (2026-08-13, all three remedies implemented)
    ///
    /// | variant | outcome |
    /// |---|---|
    /// | `None` | `Ok(None)` -- observes, repairs nothing |
    /// | `Bedok` | `Ok(Some(_))`, cross removed |
    /// | `Lmtd` | `Ok(Some(_))`, cross removed |
    /// | `EliminateMetal` | `Err(DidNotConverge)` -- **always, by construction** |
    ///
    /// **`EliminateMetal` failing here is correct, not a defect.** It rewrites
    /// only the metal temperature, and it is invoked only on an already-crossed
    /// state, so it can never clear a cross *between the two fluid streams*.
    /// The deeper reason is a limitation of this contract rather than of the
    /// method: eliminating the metal is a change to **how the next step is
    /// integrated** (replacing the four metal links with one direct hot-cold
    /// link at the series conductance), whereas [`CrossRepairInputs`] and
    /// [`CrossRepairOutcome`] express a **state repair after the fact**, and a
    /// state repair cannot undo a lag already taken. Getting its real benefit
    /// needs an integration-time hook in `steam_generator.rs`, not a bigger
    /// repair. Recorded rather than hidden, and tracked in `op-3lay`.
    ///
    /// Interpretation: the dispatch is honest -- a selected remedy either
    /// repairs the cross or says plainly that it did not.
    #[test]
    fn every_remedy_behaves_as_its_documentation_claims() {
        let inputs = crossed_inputs();
        assert!(
            inputs.worst_cross_kelvin() > 0.0,
            "the fixture must actually be crossed or this test proves nothing"
        );

        assert!(
            TemperatureCrossRemedy::None
                .apply(&inputs)
                .unwrap()
                .is_none(),
            "None must observe without repairing"
        );

        for remedy in [TemperatureCrossRemedy::Bedok, TemperatureCrossRemedy::Lmtd] {
            let outcome = remedy
                .apply(&inputs)
                .unwrap_or_else(|e| panic!("{} failed: {e}", remedy.label()))
                .unwrap_or_else(|| panic!("{} returned no repair", remedy.label()));
            let repaired = CrossRepairInputs {
                hot_temperatures: outcome.hot_temperatures.clone(),
                cold_temperatures: outcome.cold_temperatures.clone(),
                metal_temperatures: outcome.metal_temperatures.clone(),
                ..inputs.clone()
            };
            assert!(
                repaired.worst_cross_kelvin() <= 1e-9,
                "{} returned a profile still crossed by {} K",
                remedy.label(),
                repaired.worst_cross_kelvin()
            );
        }

        match TemperatureCrossRemedy::EliminateMetal.apply(&inputs) {
            Err(CrossRepairError::DidNotConverge(_)) => {}
            other => panic!(
                "EliminateMetal returned {other:?}; it cannot clear a fluid-stream cross \
                 through a post-hoc state repair and must say so"
            ),
        }
    }

    /// Guards the counter-flow node ordering, which is the single easiest thing
    /// for a remedy implementation to get backwards.
    ///
    /// # Results (2026-08-13)
    ///
    /// Hot node 0 (900 K) is the hot inlet end; cold node 2 (500 K) is the cold
    /// inlet end. Interpretation: index 0 is the hot inlet and the cold stream
    /// runs the other way, as the struct documents.
    #[test]
    fn node_zero_is_the_hot_inlet_end_and_the_cold_stream_runs_the_other_way() {
        let i = crossed_inputs();
        let hot: Vec<f64> = i
            .hot_temperatures
            .iter()
            .map(|t| t.get::<kelvin>())
            .collect();
        let cold: Vec<f64> = i
            .cold_temperatures
            .iter()
            .map(|t| t.get::<kelvin>())
            .collect();
        assert!(hot[0] > hot[2], "hot must fall from node 0");
        assert!(cold[2] < cold[0], "cold must be coldest at the last node");
        let _ = Power::new::<watt>(0.0);
    }
}
