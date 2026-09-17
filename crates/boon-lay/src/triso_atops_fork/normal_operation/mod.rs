// SPDX-License-Identifier: GPL-3.0
//
// TRISO-ATOPS fork — provenance
// -----------------------------
// Upstream project : TRISO-ATOPS (INL) — https://github.com/IdahoLabResearch/TRISO-ATOPS
// Upstream commit  : de374c8
// Upstream source  : trisoatops/trisoatops.py (`normal_operation`, `higher_activities`
//                    routing, the `× λ / 3.7e10` Ci conversion)
//                    trisoatops/utility_functions/calculation_functions.py (`higher_activities`)
//                    trisoatops/utility_functions/run_functions.py (run-file I/O — see the
//                    JSON-driver note below; still scaffolded, bead op-b4a.2.3)
// Original license : MIT — Copyright (c) 2026 Battelle Energy Alliance, LLC
// This port is distributed under GPL-3.0 as part of the combined boon-lay work;
// the MIT notice above is retained (see LICENSE.triso-atops / NOTICE.triso-atops).

//! # Nodal orchestration — the per-node normal-operation source term
//!
//! This module composes the whole normal-operation chain for one nuclide at one
//! reactor node, tying together the calculation core and the activity layer:
//!
//! 1. kernel & graphite diffusion coefficients ([`crate::triso_atops_fork::diffusion`]),
//! 2. the release-to-birth ratio `<R/B>_fail` ([`crate::triso_atops_fork::release_models::rb_fail`]),
//! 3. the release rate `R` ([`crate::triso_atops_fork::activities::release_rate`]),
//! 4. the coolant source rate `S` and graphite hold-up `G`
//!    ([`crate::triso_atops_fork::activities::base_activities`]),
//! 5. the three primary-loop pools — circulating `C`, plate-out `P`, clean-up
//!    `HPS` ([`crate::triso_atops_fork::activities::coolant_activity`]) — with the
//!    upstream group-dependent `k_plate` / `k_clean` routing and HPS toggle.
//!
//! It ports the body of `trisoatops.py::normal_operation` (the per-nuclide,
//! per-node loop) and `calculation_functions.py::higher_activities`. The
//! whole-reactor sweep over many nuclides and nodes is a thin loop over
//! [`normal_operation_node`]; parent → daughter chaining is threaded through
//! [`ParentPools`] (compute a parent nuclide first, feed its
//! [`NodalActivities::parent_pools`] into the daughter call).
//!
//! ## Units
//!
//! Geometry, rate constants, and times are `uom`-typed ([`PlantConstants`]); the
//! inventory is an [`Activity`] (Bq). The intermediate pools in [`NodalActivities`]
//! are effective-unit `f64` (see [`crate::triso_atops_fork::activities`] for why);
//! [`NodalActivities::to_curies`] performs the single, documented
//! `× λ / 3.7e10` conversion to the reportable [`NodalActivitiesCurie`] (all in
//! curies, or curies/second for the two rates).
//!
//! ## Still scaffolded — the JSON run-file driver (bead op-b4a.2.3)
//!
//! The TRISO-ATOPS GUI writes a `.json` run file (User Manual §2.4) that
//! `run_functions.py` parses (`process_run_file`, `check_run_file`,
//! `convert_time`, `nuclide_sort`, `inventory_processing`) before
//! `trisoatops.py::main` drives `normal_operation` / `accident_case`. That
//! file-I/O + argparse layer, and the transient `accident_case`, are **not yet
//! ported** — they are bead **op-b4a.2.3**, which depends on this nodal
//! orchestration. The Rust entry point there will take a typed `RunConfig` (built
//! directly from [`PlantConstants`] + a nuclide/inventory list) and may add a
//! thin `serde` reader for existing GUI run files. The physics it will call —
//! [`normal_operation_node`] — is complete and verified here.

use uom::si::f64::{Frequency, Length, ThermodynamicTemperature, Time};
use uom::si::frequency::hertz;

use crate::triso_atops_fork::activities::{
    base_activities, circulating, clean_up, curies_from_becquerels, plate_out, release_rate,
    FailureFractions,
};
use crate::triso_atops_fork::diffusion::diffusion_coefficient;
use crate::triso_atops_fork::nuclide_model::ElementGroup;
use crate::triso_atops_fork::release_models::rb_fail;
use crate::triso_atops_fork::{Activity, DecayConstant, TrisoAtopsNuclide};

/// Reactor-level constants shared by every node in a normal-operation run.
///
/// Ports the `constants` array unpacked at the top of
/// `trisoatops.py::normal_operation` (User Manual §2.4 keys). All fields are
/// `uom`-typed so an accidental unit slip is a compile error.
#[derive(Debug, Clone, Copy)]
pub struct PlantConstants {
    /// Plate-out rate constant `k_plate` ([`Frequency`], `s^-1`).
    pub k_plate: Frequency,
    /// Clean-up (HPS) rate constant `k_clean` ([`Frequency`], `s^-1`).
    pub k_clean: Frequency,
    /// Graphite layer thickness `a_graph` ([`Length`], `m`).
    pub graphite_thickness: Length,
    /// UCO fuel grain size `a_grain` ([`Length`], `m`).
    pub grain_size: Length,
    /// SiC layer thickness `a_SiC` ([`Length`], `m`).
    pub sic_thickness: Length,
    /// Fuel kernel radius `r` ([`Length`], `m`).
    pub kernel_radius: Length,
    /// Reactor run time `t` used for the coolant-pool balances ([`Time`], `s`).
    pub run_time: Time,
    /// Fuel irradiation time `t_irad` used for release/birth and diffusion
    /// ([`Time`], `s`).
    pub irradiation_time: Time,
}

/// The local temperature state of a single reactor node.
///
/// Ports the per-node entries of the `core_temp` (fuel) and `graph_temp`
/// (graphite) profiles.
#[derive(Debug, Clone, Copy)]
pub struct NodeState {
    /// Fuel/kernel temperature at the node ([`ThermodynamicTemperature`]).
    pub core_temperature: ThermodynamicTemperature,
    /// Graphite temperature at the node ([`ThermodynamicTemperature`]).
    pub graphite_temperature: ThermodynamicTemperature,
}

/// Parent-nuclide activity pools fed into a daughter's node calculation.
///
/// Ports the `C_Parent` / `P_Parent` / `HPS_Parent` arrays threaded through
/// `higher_activities`. All three are **effective-unit `f64`** (same convention
/// as [`NodalActivities`]); pass [`ParentPools::none`] when the nuclide has no
/// tracked parent, or a parent's [`NodalActivities::parent_pools`] otherwise.
#[derive(Debug, Clone, Copy, Default)]
pub struct ParentPools {
    /// Parent circulating pool `C_parent` (effective `f64`).
    pub circulating: f64,
    /// Parent plate-out pool `P_parent` (effective `f64`).
    pub plate_out: f64,
    /// Parent clean-up pool `HPS_parent` (effective `f64`).
    pub clean_up: f64,
}

impl ParentPools {
    /// No parent contribution (all pools zero).
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }
}

/// The six normal-operation activity outputs for one nuclide at one node,
/// in **effective units** (before the `× λ / 3.7e10` curie conversion).
///
/// Convert to reportable curies with [`to_curies`](Self::to_curies); chain into a
/// daughter nuclide with [`parent_pools`](Self::parent_pools).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodalActivities {
    /// Release rate `R` from the fuel (effective `f64`, Bq = `atoms/s`).
    pub release_rate: f64,
    /// Coolant source rate `S` after graphite attenuation (effective `f64`, Bq).
    pub source_rate: f64,
    /// Graphite hold-up activity `G` (effective `f64`, atom count).
    pub graphite_activity: f64,
    /// Circulating coolant pool `C` (effective `f64`, atom count).
    pub circulating_activity: f64,
    /// Plate-out pool `P` (effective `f64`, atom count).
    pub plate_out_activity: f64,
    /// Clean-up / HPS pool `HPS` (effective `f64`, atom count); `0` when the HPS
    /// is disabled or the nuclide is not a volatile.
    pub clean_up_activity: f64,
}

impl NodalActivities {
    /// The parent-chaining pools ([`ParentPools`]) to feed into a daughter
    /// nuclide whose parent is this one.
    #[must_use]
    pub fn parent_pools(&self) -> ParentPools {
        ParentPools {
            circulating: self.circulating_activity,
            plate_out: self.plate_out_activity,
            clean_up: self.clean_up_activity,
        }
    }

    /// Convert to reportable curies via the single `× λ / 3.7e10` step.
    ///
    /// The two *rates* (release, source) become `Ci/s`; the four *amounts*
    /// (graphite, circulating, plate-out, HPS) become `Ci`. Ports the
    /// `np.sum(...) × λ / 3.7e10` output step of `trisoatops.py::normal_operation`.
    ///
    /// # Arguments
    /// - `decay_constant` — the nuclide `λ` ([`DecayConstant`], `s^-1`).
    #[must_use]
    pub fn to_curies(&self, decay_constant: DecayConstant) -> NodalActivitiesCurie {
        // value [effective] × λ  → becquerels (rate) / becquerels (amount·λ),
        // then curies_from_becquerels applies the /3.7e10.
        let lam = decay_constant.get::<hertz>();
        let ci = |v: f64| curies_from_becquerels(Activity::new::<hertz>(v * lam));
        NodalActivitiesCurie {
            release_rate: ci(self.release_rate),
            source_rate: ci(self.source_rate),
            graphite_activity: ci(self.graphite_activity),
            circulating_activity: ci(self.circulating_activity),
            plate_out_activity: ci(self.plate_out_activity),
            clean_up_activity: ci(self.clean_up_activity),
        }
    }
}

/// The six normal-operation outputs in reportable **curies** (rates in `Ci/s`).
///
/// Field-for-field the curie form of [`NodalActivities`]; the two rate fields
/// (`release_rate`, `source_rate`) are in `Ci/s`, the four pool fields in `Ci`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodalActivitiesCurie {
    /// Release rate `R` (`Ci/s`).
    pub release_rate: f64,
    /// Source rate `S` (`Ci/s`).
    pub source_rate: f64,
    /// Graphite activity `G` (`Ci`).
    pub graphite_activity: f64,
    /// Circulating activity `C` (`Ci`).
    pub circulating_activity: f64,
    /// Plate-out activity `P` (`Ci`).
    pub plate_out_activity: f64,
    /// Clean-up / HPS activity (`Ci`).
    pub clean_up_activity: f64,
}

/// Compute the normal-operation activity source term for one nuclide at one node.
///
/// Ports the per-nuclide, per-node body of `trisoatops.py::normal_operation`
/// together with `calculation_functions.py::higher_activities`. The group-
/// dependent removal-constant routing follows the upstream exactly:
///
/// - **Noble gas** → no plate-out (`k_plate = 0`); plate-out pool forced to `0`;
///   clean-up applies (`k_clean`) only when `hps_enabled`.
/// - **Halogen** → plate-out and (when `hps_enabled`) clean-up both apply.
/// - **Special metal / silver / other** → plate-out applies; clean-up never
///   applies (metals are not scrubbed by the HPS).
///
/// When `hps_enabled` is `false`, the clean-up rate constant is treated as `0`
/// for every group and the clean-up pool is `0`, matching the upstream
/// `clean is False` branch.
///
/// # Arguments
/// - `nuclide` — the species record ([`TrisoAtopsNuclide`]); supplies `z`, `λ`,
///   and transport group.
/// - `short_lived` — the run-dependent short-lived flag `sl` (upstream:
///   `t½ / t_irad < 0.2`).
/// - `inventory` — the node's radionuclide inventory as an [`Activity`] (Bq);
///   from [`crate::triso_atops_fork::activities::becquerels_from_curies`].
/// - `fractions` — the fuel [`FailureFractions`].
/// - `plant` — reactor-level [`PlantConstants`].
/// - `node` — the node [`NodeState`] (core + graphite temperatures).
/// - `hps_enabled` — whether the helium-purification system is modelled.
/// - `parent` — parent-nuclide [`ParentPools`]; [`ParentPools::none`] if none.
///
/// # Returns
/// The effective-unit [`NodalActivities`]; call
/// [`NodalActivities::to_curies`] for the reportable curie values.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn normal_operation_node(
    nuclide: &TrisoAtopsNuclide,
    short_lived: bool,
    inventory: Activity,
    fractions: FailureFractions,
    plant: PlantConstants,
    node: NodeState,
    hps_enabled: bool,
    parent: ParentPools,
) -> NodalActivities {
    let z = nuclide.z;
    let lam = nuclide.decay_constant();
    let group = nuclide.element_group();

    // 1. Diffusion coefficients at this node's temperatures.
    let diff = diffusion_coefficient(z, node.core_temperature, node.graphite_temperature);

    // 2. Release-to-birth-at-failure ratio.
    let rb = rb_fail(
        z,
        short_lived,
        lam,
        node.core_temperature,
        plant.irradiation_time,
        plant.grain_size,
        plant.sic_thickness,
        plant.kernel_radius,
        diff.kernel,
    );

    // 3. Release rate R from the fuel.
    let r = release_rate(
        rb,
        group,
        fractions,
        inventory,
        short_lived,
        plant.irradiation_time,
        lam,
    );

    // 4. Coolant source rate S and graphite hold-up G.
    let sg = base_activities(
        group,
        lam,
        plant.irradiation_time,
        plant.graphite_thickness,
        diff.graphite,
        r,
    );

    // 5. Group-dependent removal-constant routing (ports higher_activities).
    let zero = Frequency::new::<hertz>(0.0);
    let (k_plate, k_clean_group) = match group {
        ElementGroup::NobleGas => (zero, plant.k_clean),
        ElementGroup::Halogen => (plant.k_plate, plant.k_clean),
        _ => (plant.k_plate, zero),
    };
    let k_clean = if hps_enabled { k_clean_group } else { zero };

    let c = circulating(
        sg.source_rate,
        k_plate,
        lam,
        plant.run_time,
        k_clean,
        parent.circulating,
    );

    let p = match group {
        ElementGroup::NobleGas => 0.0, // noble gases do not plate out
        _ => plate_out(
            k_plate,
            sg.source_rate,
            lam,
            plant.run_time,
            c,
            k_clean,
            parent.plate_out,
        ),
    };

    let hps = if hps_enabled && matches!(group, ElementGroup::NobleGas | ElementGroup::Halogen) {
        clean_up(
            k_plate,
            sg.source_rate,
            lam,
            plant.run_time,
            c,
            k_clean,
            parent.clean_up,
        )
    } else {
        0.0
    };

    NodalActivities {
        release_rate: r,
        source_rate: sg.source_rate,
        graphite_activity: sg.graphite_activity,
        circulating_activity: c,
        plate_out_activity: p,
        clean_up_activity: hps,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::triso_atops_fork::activities::becquerels_from_curies;
    use crate::triso_atops_fork::nuclide_model::nuclide_database::find_nuclide;
    use approx::assert_relative_eq;
    use uom::si::length::meter;
    use uom::si::thermodynamic_temperature::degree_celsius;
    use uom::si::time::second;

    const FRACTIONS: FailureFractions = FailureFractions {
        heavy_metal: 1e-4,
        sic: 1e-4,
        incremental: 2.3e-5,
        incremental_sic: 3.6e-5,
    };

    fn plant() -> PlantConstants {
        let yr = 3.155_76e7;
        PlantConstants {
            k_plate: Frequency::new::<hertz>(7.5e-4),
            k_clean: Frequency::new::<hertz>(8.77e-5),
            graphite_thickness: Length::new::<meter>(0.0045),
            grain_size: Length::new::<meter>(1e-5),
            sic_thickness: Length::new::<meter>(3.5e-5),
            kernel_radius: Length::new::<meter>(0.000213),
            run_time: Time::new::<second>(40.0 * yr),
            irradiation_time: Time::new::<second>(3.0 * yr),
        }
    }

    fn node_900c() -> NodeState {
        NodeState {
            core_temperature: ThermodynamicTemperature::new::<degree_celsius>(900.0),
            graphite_temperature: ThermodynamicTemperature::new::<degree_celsius>(900.0),
        }
    }

    /// End-to-end single node, Cs-137 (special metal, HPS off), matches upstream.
    ///
    /// Methodology: 900 °C node, 44.5 Ci inventory, geometry per the upstream
    /// driver. Runs the full chain (diffusion → `<R/B>` → release → source/graphite
    /// → circulating/plate-out) and converts to curies. Reference values (upstream
    /// `trisoatops.py::normal_operation` composition, run 2026-07-15, commit
    /// de374c8): release 1.258227631e-10 Ci/s, source 1.241364332e-10 Ci/s,
    /// graphite 1.542561347e-4 Ci, circulating 1.655150832e-7 Ci, plate-out
    /// 0.1023699795 Ci, HPS 0. Pass tol 1e-6 rel.
    #[test]
    fn cs137_node_matches_upstream() {
        let cs = find_nuclide("Cs-137").unwrap();
        let out = normal_operation_node(
            &cs,
            false,
            becquerels_from_curies(44.5),
            FRACTIONS,
            plant(),
            node_900c(),
            false,
            ParentPools::none(),
        )
        .to_curies(cs.decay_constant());
        assert_relative_eq!(
            out.release_rate,
            1.258_227_631_073_319e-10,
            max_relative = 1e-6
        );
        assert_relative_eq!(
            out.source_rate,
            1.241_364_332_344_98e-10,
            max_relative = 1e-6
        );
        assert_relative_eq!(
            out.graphite_activity,
            1.542_561_346_936_662e-4,
            max_relative = 1e-6
        );
        assert_relative_eq!(
            out.circulating_activity,
            1.655_150_831_630_567e-7,
            max_relative = 1e-6
        );
        assert_relative_eq!(
            out.plate_out_activity,
            0.102_369_979_450_406_4,
            max_relative = 1e-6
        );
        assert_eq!(out.clean_up_activity, 0.0);
    }

    /// End-to-end single node, Kr-88 (noble gas, HPS on), matches upstream.
    ///
    /// Noble-gas path: no plate-out, no graphite hold-up, clean-up active.
    /// References (upstream, 2026-07-15): release = source = 1.381059264e-9 Ci/s,
    /// graphite 0, circulating 8.861119809e-6 Ci, plate-out 0, HPS 1.140206976e-5
    /// Ci. Pass tol 1e-6 rel.
    #[test]
    fn kr88_node_matches_upstream() {
        let kr = find_nuclide("Kr-88").unwrap();
        let out = normal_operation_node(
            &kr,
            true,
            becquerels_from_curies(44.5),
            FRACTIONS,
            plant(),
            node_900c(),
            true,
            ParentPools::none(),
        )
        .to_curies(kr.decay_constant());
        assert_relative_eq!(
            out.release_rate,
            1.381_059_264_467_175e-9,
            max_relative = 1e-6
        );
        assert_relative_eq!(
            out.source_rate,
            1.381_059_264_467_175e-9,
            max_relative = 1e-6
        );
        assert_eq!(out.graphite_activity, 0.0);
        assert_relative_eq!(
            out.circulating_activity,
            8.861_119_808_806_668e-6,
            max_relative = 1e-6
        );
        assert_eq!(out.plate_out_activity, 0.0);
        assert_relative_eq!(
            out.clean_up_activity,
            1.140_206_976_124_235e-5,
            max_relative = 1e-6
        );
    }

    /// End-to-end single node, I-131 (halogen, HPS on), matches upstream.
    ///
    /// Halogen path: plate-out **and** clean-up both active, no graphite hold-up.
    /// References (upstream, 2026-07-15): release = source = 3.017238319e-11 Ci/s,
    /// circulating 3.597519388e-8 Ci, plate-out 2.699034135e-5 Ci, HPS
    /// 3.156070581e-6 Ci. Pass tol 1e-6 rel.
    #[test]
    fn i131_node_matches_upstream() {
        let i131 = find_nuclide("I-131").unwrap();
        let out = normal_operation_node(
            &i131,
            true,
            becquerels_from_curies(44.5),
            FRACTIONS,
            plant(),
            node_900c(),
            true,
            ParentPools::none(),
        )
        .to_curies(i131.decay_constant());
        assert_relative_eq!(
            out.release_rate,
            3.017_238_318_535_933e-11,
            max_relative = 1e-6
        );
        assert_eq!(out.graphite_activity, 0.0);
        assert_relative_eq!(
            out.circulating_activity,
            3.597_519_388_257_818e-8,
            max_relative = 1e-6
        );
        assert_relative_eq!(
            out.plate_out_activity,
            2.699_034_134_630_28e-5,
            max_relative = 1e-6
        );
        assert_relative_eq!(
            out.clean_up_activity,
            3.156_070_581_427_675e-6,
            max_relative = 1e-6
        );
    }
}
