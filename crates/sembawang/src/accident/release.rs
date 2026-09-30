// SPDX-License-Identifier: GPL-3.0

//! The driver: a prescribed transient plus an inventory, out to a source term.
//!
//! # What this does and does not own
//!
//! Every piece of release physics here is a call into
//! `boon_lay::triso_atops_fork` — diffusion integrals, release fractions,
//! breakthrough, the atoms-to-curies conversion. **None of it is reimplemented,
//! and none of it should be**: that layer is ported, `uom`-typed and verified
//! code-to-code against upstream TRISO-ATOPS, and a second copy here would
//! drift from it silently.
//!
//! What this module owns is the orchestration: the node loops, the venting
//! pairing (through [`super::venting::VentingWindow`], never a prefix), the
//! cumulative-to-incremental conversion, and the assembly into a
//! [`SourceTerm`].
//!
//! # The 6-to-7 field bridge
//!
//! `normal_operation_node` returns a `NodalActivities` with **six** channels.
//! `release_activity` wants a `NormalOperationNode` with **seven**. The extra
//! one is `kernel_inventory_atoms`, which is not an output of the normal-
//! operation solve at all — it is the node's own inventory expressed as an atom
//! count, `atom_count_from_activity(inventory, lambda)`. [`bridge_node`] is that
//! conversion, in one place, so the two shapes cannot be lined up wrongly at a
//! call site.
//!
//! # Cumulative curies to per-window activity
//!
//! `accident_release_curies` returns a **cumulative** release at each venting
//! sample. [`SourceTerm`] wants the activity released **during** each window.
//! The conversion is a first difference, with the first window carrying
//! everything up to the second venting sample so nothing is dropped:
//!
//! ```text
//! release[0] = cumulative[1]
//! release[i] = cumulative[i+1] - cumulative[i]      for i >= 1
//! ```
//!
//! so the windows sum to `cumulative[last]` exactly.
//!
//! **Windows are contiguous by construction even when the venting mask is
//! gappy**, because window `i` spans venting sample `i` to venting sample
//! `i+1` — a cooling period simply becomes one long window rather than a hole.
//! That matters because `changi`'s `SourceTerm` rejects gaps.
//!
//! A negative increment is possible, because upstream's `release_activity`
//! deliberately does not clamp. It is **not clamped here either** — it is
//! reported through [`crate::error::Caveats::negative_atom_count_seen`] and
//! floored to zero only at the [`SourceTerm`] boundary, which rejects negative
//! activities. Silently clamping and silently passing it on are both worse than
//! saying so.

use boon_lay::triso_atops_fork::accident::{
    accident_release_curies, atoms_to_curies, coolant_release, mean_temperature_rate,
    AccidentFractions, NormalOperationNode, ReleaseMaterial as AccidentMaterial,
};
use boon_lay::triso_atops_fork::activities::atom_count_from_activity;
use boon_lay::triso_atops_fork::diffusion::{integrate_diffusion_over_time, DiffusionMaterial};
use boon_lay::triso_atops_fork::nuclide_model::ElementGroup;
use boon_lay::triso_atops_fork::release_models::release_fraction_transient;
use boon_lay::triso_atops_fork::activities::FailureFractions;
use boon_lay::triso_atops_fork::normal_operation::{
    normal_operation_node, NodalActivities, NodeState, ParentPools, PlantConstants,
};
use boon_lay::triso_atops_fork::run_selection::{
    normalise_nuclide_name, select_nuclides, select_nuclides_accident, ParentDecayPolicy,
};
use std::collections::HashMap;
use boon_lay::triso_atops_fork::TrisoAtopsNuclide;
use changi::activity::deposition::DepositionGroup;
use changi::activity::source::{NuclideRelease, ReleaseWindow, SourceTerm};
use uom::si::area::square_meter;
use uom::si::f64::{Area, Frequency, Length, Pressure, Ratio, ThermodynamicTemperature, Time};
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::second;

use crate::error::{Caveats, Error, Result};
use crate::inventory::CoreInventory;
use crate::scenario::TemperatureTransient;
use crate::units;

use super::venting::VentingWindow;

/// Lower edge of the Arrhenius diffusion correlation's fitted range, degrees
/// Celsius. Crossing it sets [`Caveats::diffusion_coefficient_clamped`].
/// ~~Outside it `boon-lay` clamps rather than extrapolating~~ **CORRECTED 2026-09-30 (#449):**
/// `boon-lay` clamps only group-specific lower limits and extrapolates
/// everything else; see that caveat's docs.
pub const DIFFUSION_FIT_MIN_CELSIUS: f64 = 700.0;

/// Upper edge of the fitted range, degrees Celsius. See
/// [`DIFFUSION_FIT_MIN_CELSIUS`].
pub const DIFFUSION_FIT_MAX_CELSIUS: f64 = 2400.0;

/// The geometry, failure fractions and plant constants a release needs.
///
/// Every field is prescribed by the caller. There are no defaults, deliberately
/// — a default geometry would be a specific reactor's, wearing no label.
#[derive(Debug, Clone, PartialEq)]
pub struct PlantParameters {
    /// Failure fractions, normal-operation and accident-phase.
    pub fractions: AccidentFractions,
    /// Matrix graphite diffusion thickness.
    pub graphite_thickness: Length,
    /// Fuel kernel radius.
    pub kernel_radius: Length,
    /// SiC layer thickness.
    pub sic_thickness: Length,
    /// Coolant pressure, for the venting calculation.
    pub coolant_pressure: Pressure,
    /// Fraction of plated-out activity lifted off during the accident.
    pub x_liftoff: f64,
    /// Whether a helium purification system is fitted.
    pub clean_up_fitted: bool,
    /// The primary-circuit state the accident starts from (GitHub #448). The
    /// constructors set [`PrimaryCircuitPools::FromNormalOperation`]; the empty
    /// state is an explicit ablation.
    pub pools: PrimaryCircuitPools,
}

/// The primary-circuit and fuel-matrix pools an accident starts from.
///
/// Upstream (`trisoatops.py::main`, commit `de374c8`) runs `normal_operation`
/// and feeds its per-node pools into `accident_case`. That does three things
/// the empty state omits (#448):
/// 1. `release_activity` subtracts the graphite, circulating, plate-out and HPS
///    inventory from what the kernel can still release;
/// 2. the graphite pool is released through `RF_graph`;
/// 3. the circuit term `C + x_liftoff · P` is added.
///
/// **Default ON** (CLAUDE.md "correct physics is the default"): every
/// constructor in this crate sets [`Self::FromNormalOperation`].
#[derive(Debug, Clone, PartialEq)]
pub enum PrimaryCircuitPools {
    /// Run TRISO-ATOPS normal operation per node first, as upstream does.
    FromNormalOperation(NormalOperation),
    /// **Ablation only.** Every pool empty ([`zero_pools`]). Measured in
    /// upstream on a heat-up case (#448), this **over**-predicts the metals
    /// (Ag-110m ×2.0, Cs-137 ×1.6) and slightly **under**-predicts
    /// un-scrubbed noble gases (Kr-85 ×0.87). The direction is
    /// nuclide-dependent.
    EmptyAblation,
}

/// The normal-operation inputs upstream's `normal_operation` needs, beyond
/// the inventory, fractions and geometry already in [`PlantParameters`].
#[derive(Debug, Clone, PartialEq)]
pub struct NormalOperation {
    /// Plate-out rate constant `k_plate`.
    pub k_plate: Frequency,
    /// Helium-purification clean-up rate constant `k_clean`. Applied only when
    /// [`PlantParameters::clean_up_fitted`].
    pub k_clean: Frequency,
    /// Kernel grain size `a_grain`.
    pub grain_size: Length,
    /// Reactor run time, for the coolant-pool balances.
    pub run_time: Time,
    /// Fuel irradiation time, for release-to-birth and the short-lived flag.
    pub irradiation_time: Time,
    /// Normal-operation fuel and graphite temperatures.
    pub temperatures: NodeTemperatures,
}

/// Normal-operation temperatures over the core nodes.
#[derive(Debug, Clone, PartialEq)]
pub enum NodeTemperatures {
    /// One fuel and one graphite temperature for every node.
    Uniform {
        /// Fuel (kernel) temperature.
        core: ThermodynamicTemperature,
        /// Matrix graphite temperature.
        graphite: ThermodynamicTemperature,
    },
    /// Per node, `[ring][axial]`, matching the transient's node layout.
    PerNode {
        /// Fuel (kernel) temperatures.
        core: Vec<Vec<ThermodynamicTemperature>>,
        /// Matrix graphite temperatures.
        graphite: Vec<Vec<ThermodynamicTemperature>>,
    },
}

impl NodeTemperatures {
    fn at(&self, ring: usize, axial: usize) -> NodeState {
        match self {
            Self::Uniform { core, graphite } => NodeState {
                core_temperature: *core,
                graphite_temperature: *graphite,
            },
            Self::PerNode { core, graphite } => NodeState {
                core_temperature: core[ring][axial],
                graphite_temperature: graphite[ring][axial],
            },
        }
    }
}

/// Seconds in upstream's year (`convert_time`: 365 days).
const UPSTREAM_YEAR_S: f64 = 365.0 * 24.0 * 3600.0;

impl NormalOperation {
    /// The NP-MHTGR reference normal operation, Stoyer et al. 2026 Case A,
    /// Table 3 (`boon-lay/verification_and_validation/mhtgr_stoyer/constants.csv`):
    /// `k_plate` 7.5e-5 /s, `k_clean` 8.77e-5 /s, `a_grain` 1e-5 m,
    /// run time 40 y, irradiation time 3 y.
    ///
    /// **Temperatures are a stand-in:** a uniform **885.24 K** (core and
    /// graphite), the arithmetic mean of the paper's Table 7 per-node profile
    /// (the paper sets graphite = fuel). A caller with per-node temperatures
    /// should use [`NodeTemperatures::PerNode`].
    #[must_use]
    pub fn np_mhtgr_reference() -> Self {
        let t = ThermodynamicTemperature::new::<uom::si::thermodynamic_temperature::kelvin>(885.24);
        Self {
            k_plate: Frequency::new::<uom::si::frequency::hertz>(7.5e-5),
            k_clean: Frequency::new::<uom::si::frequency::hertz>(8.77e-5),
            grain_size: Length::new::<uom::si::length::meter>(1.0e-5),
            run_time: Time::new::<uom::si::time::second>(40.0 * UPSTREAM_YEAR_S),
            irradiation_time: Time::new::<uom::si::time::second>(3.0 * UPSTREAM_YEAR_S),
            temperatures: NodeTemperatures::Uniform { core: t, graphite: t },
        }
    }
}

impl PlantParameters {
    /// The NP-MHTGR reference geometry and normal-operation failure fractions,
    /// with the **accident-phase** parameters required as arguments.
    ///
    /// # Which numbers are cited and which are not
    ///
    /// The geometry and the two normal-operation failure fractions come from
    /// the NP-MHTGR New Production Reactor Program case that upstream
    /// TRISO-ATOPS ships as its reference, and are already carried in this
    /// workspace with provenance — see
    /// `boon-lay/tests/triso_atops_fork_verification.rs`, which pins them
    /// against upstream at commit `de374c8`:
    ///
    /// | parameter | value |
    /// |---|---|
    /// | kernel radius | 213 um |
    /// | SiC thickness | 35 um |
    /// | matrix graphite thickness | 4.5 mm |
    /// | heavy-metal contamination fraction | 1e-4 |
    /// | as-fabricated SiC defect fraction | 1e-4 |
    /// | incremental failure fraction | 2.3e-5 |
    /// | incremental SiC failure fraction | 3.6e-5 |
    ///
    /// **The three accident-phase parameters are NOT in that case**, so they
    /// are arguments rather than defaults. Supplying a default for them would
    /// put an uncited number behind a constructor named after a specific
    /// reactor, which is precisely how a guess acquires a citation it never
    /// earned. The caller states them and owns them.
    ///
    /// Coolant pressure defaults to one atmosphere (101.325 kPa), which is
    /// a **depressurised** condition — appropriate for a depressurised
    /// conduction cooldown and wrong for anything else.
    ///
    /// # Arguments
    /// - `incremental_accident` — accident-phase incremental failure fraction.
    /// - `incremental_sic_accident` — accident-phase incremental SiC failure
    ///   fraction.
    /// - `x_liftoff` — fraction of plated-out activity lifted off.
    ///   ~~It does nothing, because the crate starts from empty pools~~
    ///   **CORRECTED 2026-09-30 (#448):** the plant starts from real
    ///   normal-operation pools ([`NormalOperation::np_mhtgr_reference`]), so
    ///   the lift-off term is live.
    #[must_use]
    pub fn np_mhtgr_reference(
        incremental_accident: f64,
        incremental_sic_accident: f64,
        x_liftoff: f64,
    ) -> Self {
        Self {
            fractions: AccidentFractions {
                heavy_metal: 1.0e-4,
                sic: 1.0e-4,
                incremental: 2.3e-5,
                incremental_sic: 3.6e-5,
                incremental_accident,
                incremental_sic_accident,
            },
            graphite_thickness: Length::new::<uom::si::length::meter>(0.0045),
            kernel_radius: Length::new::<uom::si::length::meter>(0.000_213),
            sic_thickness: Length::new::<uom::si::length::meter>(3.5e-5),
            coolant_pressure: Pressure::new::<uom::si::pressure::kilopascal>(101.325),
            x_liftoff,
            clean_up_fitted: true,
            pools: PrimaryCircuitPools::FromNormalOperation(NormalOperation::np_mhtgr_reference()),
        }
    }

    /// The same plant, with the primary-circuit pools **emptied**: an explicit
    /// ablation of [`PrimaryCircuitPools::FromNormalOperation`] (#448).
    #[must_use]
    pub fn without_normal_operation_pools(mut self) -> Self {
        self.pools = PrimaryCircuitPools::EmptyAblation;
        self
    }
}

/// A completed release calculation.
#[derive(Debug, Clone, PartialEq)]
pub struct AccidentRelease {
    /// The source term, ready for `changi`.
    pub source_term: SourceTerm,
    /// Known upstream behaviours that affected this result. **Report these
    /// alongside any number taken from `source_term`** — see [`Caveats`].
    pub caveats: Caveats,
    /// Which samples vented, and how much each released.
    pub venting: VentingWindow,
    /// Nuclides dropped by the **half-life screen** (t½ below 4 % of the
    /// transient), as supplied. ~~"with the reason"~~: no reason is stored,
    /// and since #449 unknown names are an error, never listed here.
    pub screened_out: Vec<String>,
    /// Per released nuclide, the **unfloored cumulative release at the last
    /// venting sample** \[Bq\]: upstream `accident_case`'s last total. The
    /// source term's window sum equals this unless a window was floored
    /// ([`Caveats::negative_atom_count_seen`]). This is the quantity to compare
    /// with upstream.
    pub cumulative_final: Vec<(String, f64)>,
}

/// A normal-operation state with every pool empty: the
/// [`PrimaryCircuitPools::EmptyAblation`] state.
///
/// ~~Starting from zero therefore **under-predicts** the early release~~
/// **CORRECTED 2026-09-30 (#448):** the direction is **nuclide-dependent**.
/// Measured in upstream TRISO-ATOPS (3-year irradiation, 900 °C normal
/// operation, heat to 1600 °C then cool), the ratio empty ÷ real pools is:
/// Ag-110m ×2.00 and Cs-137 ×1.59 (**over**-predicted: the kernel term is not
/// reduced by what already left it); Kr-85 ×0.87 without HPS (**under**: the
/// circuit term is lost); Sr-90 0.99; I-131 1.00. Since #448 the default is
/// [`PrimaryCircuitPools::FromNormalOperation`], and this is an ablation.
#[must_use]
pub fn zero_pools() -> boon_lay::triso_atops_fork::normal_operation::NodalActivities {
    boon_lay::triso_atops_fork::normal_operation::NodalActivities {
        release_rate: 0.0,
        source_rate: 0.0,
        graphite_activity: 0.0,
        circulating_activity: 0.0,
        plate_out_activity: 0.0,
        clean_up_activity: 0.0,
    }
}

/// Upstream's `normal_operation`, per node, for every inventory nuclide the
/// normal-operation selection keeps: the pools [`PrimaryCircuitPools::FromNormalOperation`]
/// starts the accident from. Keyed by nuclide name; each vector is indexed
/// `ring * n_axial + axial`.
///
/// It follows `boon-lay`'s `tests/mhtgr_stoyer_workflow.rs::port_case_nodal`,
/// which is verified against upstream at 1e-9:
/// - `select_nuclides` with [`ParentDecayPolicy::UpstreamTableDefault`];
/// - nuclides in **supplied order** (upstream's `nuclide_sort` is a no-op), so
///   a parent must precede its daughter for the daughter to see its pools;
/// - the HPS applied iff [`PlantParameters::clean_up_fitted`].
fn normal_operation_pools(
    inventory: &CoreInventory,
    transient: &TemperatureTransient,
    plant: &PlantParameters,
    op: &NormalOperation,
) -> HashMap<String, Vec<NodalActivities>> {
    let names = inventory.names();
    let (selected, _unknown) =
        select_nuclides(&names, op.irradiation_time, None, ParentDecayPolicy::UpstreamTableDefault);
    let constants = PlantConstants {
        k_plate: op.k_plate,
        k_clean: op.k_clean,
        graphite_thickness: plant.graphite_thickness,
        grain_size: op.grain_size,
        sic_thickness: plant.sic_thickness,
        kernel_radius: plant.kernel_radius,
        run_time: op.run_time,
        irradiation_time: op.irradiation_time,
    };
    let f = plant.fractions;
    let fractions = FailureFractions {
        heavy_metal: f.heavy_metal,
        sic: f.sic,
        incremental: f.incremental,
        incremental_sic: f.incremental_sic,
    };
    let mut out: HashMap<String, Vec<NodalActivities>> = HashMap::new();
    for s in &selected {
        let Some(inv) = inventory
            .nuclides
            .iter()
            .find(|n| normalise_nuclide_name(&n.name).is_ok_and(|c| c == s.nuclide.name))
        else {
            continue;
        };
        let parent = s
            .parent_decay
            .then(|| out.get(s.nuclide.parents[0]))
            .flatten()
            .cloned();
        let mut nodes = Vec::with_capacity(transient.n_radial * transient.n_axial);
        for r in 0..transient.n_radial {
            let axial = inv.axial_curies(r, transient.n_axial);
            for (k, curies) in axial.iter().enumerate().take(transient.n_axial) {
                let pools = parent
                    .as_ref()
                    .map_or(ParentPools::none(), |p| p[r * transient.n_axial + k].parent_pools());
                nodes.push(normal_operation_node(
                    &s.nuclide,
                    s.short_lived,
                    units::to_boon_lay_activity(units::from_curies(*curies)),
                    fractions,
                    constants,
                    op.temperatures.at(r, k),
                    plant.clean_up_fitted,
                    pools,
                ));
            }
        }
        out.insert(s.nuclide.name.to_string(), nodes);
    }
    out
}

/// Bridge `normal_operation_node`'s six channels to `release_activity`'s seven.
///
/// The seventh, `kernel_inventory_atoms`, is the node's own inventory as an
/// atom count — not an output of the normal-operation solve. See the module
/// docs.
#[must_use]
pub fn bridge_node(
    activities: &boon_lay::triso_atops_fork::normal_operation::NodalActivities,
    inventory_curies: f64,
    decay_constant: uom::si::f64::Frequency,
) -> NormalOperationNode {
    let inventory = units::to_boon_lay_activity(units::from_curies(inventory_curies));
    NormalOperationNode {
        kernel_inventory_atoms: atom_count_from_activity(inventory, decay_constant),
        release_rate: activities.release_rate,
        source_rate: activities.source_rate,
        graphite_activity: activities.graphite_activity,
        circulating_activity: activities.circulating_activity,
        plate_out_activity: activities.plate_out_activity,
        clean_up_activity: activities.clean_up_activity,
    }
}

/// Which `changi` deposition group a TRISO-ATOPS nuclide belongs to.
///
/// **Goes through the atomic number, not through `boon-lay`'s
/// [`ElementGroup`].** The two groupings genuinely differ: Se and Te travel
/// through TRISO layers like halogens and are grouped with them for *release*,
/// but deposit as condensed aerosols. Translating one enum into the other would
/// be wrong for exactly those two, so the classification is re-derived from `Z`.
#[must_use]
pub fn deposition_group_of(nuclide: &TrisoAtopsNuclide) -> DepositionGroup {
    DepositionGroup::from_atomic_number(nuclide.z)
}

/// Run the accident release and assemble a source term.
///
/// # Arguments
/// - `inventory` — the prescribed core inventory. Its `n_radial`/`n_axial` must
///   match `transient`'s.
/// - `transient` — the prescribed temperature history.
/// - `plant` — geometry, failure fractions and plant constants.
///
/// # The half-life screen sets the nuclide list
///
/// `select_nuclides_accident` drops any nuclide whose half-life is below 4 % of
/// the accident duration, on the grounds that it decays away before it matters.
/// **That threshold is relative, so lengthening the transient screens out
/// more.** Whichever nuclides it drops are returned in
/// [`AccidentRelease::screened_out`] rather than vanishing.
///
/// # Errors
/// [`Error::UnknownNuclide`] if **any** inventory name does not parse or is not
/// in the supported table. Non-canonical spellings (`cs137`) are normalised, as
/// upstream does. ~~(only when no nuclide survives)~~ **CORRECTED 2026-09-30
/// (#449)**: unknown names were previously listed in `screened_out`;
/// [`Error::TransientTooShort`] if the venting calculation has too little to
/// work with; [`Error::VentingTimeNotOnAxis`] if the venting selection cannot
/// be reconciled with the time axis.
///
/// # Panics
/// Panics if `inventory` and `transient` disagree on the node counts.
pub fn accident_release(
    inventory: &CoreInventory,
    transient: &TemperatureTransient,
    plant: &PlantParameters,
) -> Result<AccidentRelease> {
    accident_release_with_venting(inventory, transient, plant, &Venting::Upstream)
}

/// How released activity leaves the core, i.e. the vented fraction `frac(t)`
/// that multiplies the cumulative fuel release (upstream
/// `accident_totals = frac * (kernel + graphite) + …`).
///
/// **TRISO-ATOPS is a depressurisation model.** Its user manual: *"releases
/// are due to a breach in the reactor resulting in a venting of the core"*.
/// Its only transport is [`Venting::Upstream`]: thermal expansion of the
/// coolant while the core heats. **It has no air- or water-ingress transport**,
/// in which gas flows through the core and carries the release out whatever
/// the temperature does. The other two variants exist for that (GitHub #446),
/// and they are **this workspace's additions, not upstream behaviour**.
///
/// `frac(t)` is the fraction of the fuel's cumulative release up to `t` that
/// has left the core by `t`. It is dimensionless, in `[0, 1]`.
#[derive(Debug, Clone, PartialEq)]
pub enum Venting {
    /// Upstream TRISO-ATOPS (`trisoatops.py::accident_case`, commit `de374c8`),
    /// both branches:
    /// - **uniform and constant temperature** (every node at every time equal
    ///   to the first, by exact comparison as upstream's
    ///   `np.all(accident_temp == accident_temp[0, 0, 0])`): **`frac = 1` at
    ///   every sample**, and no `coolant_release` call;
    /// - **otherwise:** `coolant_release`. The ideal-gas expansion fraction at
    ///   the heating samples (`dT/dt ≥ 0`), with upstream's forced
    ///   `frac[0] = 1`.
    ///
    /// ~~Before 2026-09-30 the port always took the second branch~~, so an
    /// isothermal hold vented nothing after `t = 0` and released **0 Bq**
    /// (#446). The first branch was missing from the port and is restored
    /// here.
    ///
    /// **Where upstream has no answer (#447, #449):** if *every* sample
    /// vents, as in a monotonic heat-up or any `from_ramp` ramp-and-hold,
    /// upstream's `accident_temp[:, :-0, :]` is empty and it raises
    /// `IndexError`. This port returns the ideal-gas fraction `≈ 1 − T0/T`
    /// instead: a defined answer, but **not an upstream-verified one**. A
    /// spatially non-uniform field that is constant in time also reaches
    /// that path, and there every `frac` after the first is 0 (#447 item 1,
    /// open).
    Upstream,
    /// Everything released from the fuel leaves the core at once: `frac = 1`
    /// at every sample. The conservative choice for an ingress with no
    /// primary-circuit retention (the #435 bound's assumption). Not upstream.
    FullFlowThrough,
    /// A caller-supplied `frac(t)`, **one entry per transient sample**, each
    /// finite and in `[0, 1]`: e.g. the cumulative fraction of the core's gas
    /// exchanged by an ingress flow. The caller owns the number and its
    /// source. Not upstream.
    Prescribed(Vec<f64>),
}

impl TemperatureTransient {
    /// Whether every node at every time has exactly the first node's first
    /// temperature: upstream's test for skipping `coolant_release`.
    fn is_uniform_and_constant(&self) -> bool {
        let first = self.temperatures[0][0][0];
        self.temperatures
            .iter()
            .flatten()
            .flatten()
            .all(|t| *t == first)
    }
}

/// [`accident_release`] with the core-venting (transport) mode chosen
/// explicitly; see [`Venting`]. [`accident_release`] is this with
/// [`Venting::Upstream`].
///
/// # Errors
/// As [`accident_release`], plus [`Error::VentingLengthMismatch`] and
/// [`Error::VentingFractionOutOfRange`] for a bad [`Venting::Prescribed`].
///
/// # Panics
/// As [`accident_release`].
pub fn accident_release_with_venting(
    inventory: &CoreInventory,
    transient: &TemperatureTransient,
    plant: &PlantParameters,
    venting_mode: &Venting,
) -> Result<AccidentRelease> {
    assert_eq!(
        inventory.n_radial, transient.n_radial,
        "inventory and transient disagree on the radial node count"
    );
    assert_eq!(
        inventory.n_axial, transient.n_axial,
        "inventory and transient disagree on the axial node count"
    );
    if transient.len() < 2 {
        return Err(Error::TransientTooShort(transient.len()));
    }

    let mut caveats = Caveats::default();

    // The transient's own range against the correlation's fitted range.
    if transient.min_celsius() < DIFFUSION_FIT_MIN_CELSIUS
        || transient.peak_celsius() > DIFFUSION_FIT_MAX_CELSIUS
    {
        caveats.diffusion_coefficient_clamped = true;
    }

    // -- which nuclides survive the half-life screen
    let names = inventory.names();
    // Names are normalised as upstream's `nuclide_import_accident` does
    // ("cs137" -> "Cs-137"). A name that is NOT in the TRISO-ATOPS table, or
    // does not parse, is an ERROR (#449). Upstream raises `KeyError` for the
    // first and only warns for the second; returning an error for both is
    // stricter, deliberately, because a nuclide silently left out of a source
    // term is the kind of silent zero #446 was. Before 2026-09-30 both were
    // listed as "screened out".
    let (selected, errors) = select_nuclides_accident(&names, transient.end(), None);
    if !errors.is_empty() {
        return Err(Error::UnknownNuclide(format!("{errors:?}")));
    }
    let kept: Vec<String> = selected.iter().map(|n| n.name.to_string()).collect();
    let screened_out: Vec<String> = names
        .iter()
        .filter(|n| {
            let canonical = normalise_nuclide_name(n).unwrap_or_default();
            !kept.iter().any(|k| *k == canonical)
        })
        .map(|n| (*n).to_string())
        .collect();
    if selected.is_empty() {
        return Err(Error::UnknownNuclide(format!(
            "no nuclide survived selection out of {names:?}"
        )));
    }

    // -- venting (see `Venting`)
    let n_samples = transient.times.len();
    let venting = match venting_mode {
        Venting::Upstream if transient.is_uniform_and_constant() => {
            // Upstream's `else: frac = np.ones(np.size(times))`.
            VentingWindow::all_samples(vec![1.0; n_samples])
        }
        Venting::Upstream => {
            let rate = mean_temperature_rate(&transient.times, &transient.all_node_histories());
            let hot = transient.hot_node_history();
            let (vent_fraction, vent_times) =
                coolant_release(&transient.times, &rate, &hot, plant.coolant_pressure);
            let window =
                VentingWindow::from_coolant_release(&transient.times, &vent_times, vent_fraction)?;
            caveats.first_sample_forced_fully_vented = !window.is_empty();
            window
        }
        Venting::FullFlowThrough => VentingWindow::all_samples(vec![1.0; n_samples]),
        Venting::Prescribed(fractions) => {
            if fractions.len() != n_samples {
                return Err(Error::VentingLengthMismatch {
                    expected: n_samples,
                    got: fractions.len(),
                });
            }
            if let Some((index, &value)) = fractions
                .iter()
                .enumerate()
                .find(|(_, f)| !(f.is_finite() && (0.0..=1.0).contains(*f)))
            {
                return Err(Error::VentingFractionOutOfRange { index, value });
            }
            VentingWindow::all_samples(fractions.clone())
        }
    };
    caveats.venting_mask_was_gappy = !venting.is_contiguous();
    if venting.len() < 2 {
        return Err(Error::TransientTooShort(venting.len()));
    }

    let n_keep = venting.len();
    let kept_times = venting.times(&transient.times);

    // -- the normal-operation pools the accident starts from (#448)
    let normal_pools: Option<HashMap<String, Vec<NodalActivities>>> = match &plant.pools {
        PrimaryCircuitPools::FromNormalOperation(op) => {
            Some(normal_operation_pools(inventory, transient, plant, op))
        }
        PrimaryCircuitPools::EmptyAblation => None,
    };

    // -- per-nuclide, per-node release
    let mut releases = Vec::with_capacity(selected.len());
    let mut cumulative_final = Vec::with_capacity(selected.len());
    for nuclide in &selected {
        let group = nuclide.element_group();
        let volatile = matches!(group, ElementGroup::NobleGas | ElementGroup::Halogen);
        let lambda = nuclide.decay_constant();
        let lam_hz = lambda.get::<uom::si::frequency::hertz>();

        let inv = inventory
            .nuclides
            .iter()
            .find(|n| normalise_nuclide_name(&n.name).is_ok_and(|c| c == nuclide.name))
            .ok_or_else(|| Error::UnknownNuclide(nuclide.name.to_string()))?;

        let mut kernel_curies = vec![0.0_f64; n_keep];
        let mut graphite_curies = vec![0.0_f64; n_keep];
        // Circuit pools summed over nodes, in effective atoms (upstream sums
        // `nodal_normop[nuclide][4 or 5]` before the `lam / 3.7e10`).
        let (mut circulating_atoms, mut plate_out_atoms) = (0.0_f64, 0.0_f64);
        let node_pools = normal_pools.as_ref().and_then(|m| m.get(nuclide.name));

        for r in 0..transient.n_radial {
            let axial = inv.axial_curies(r, transient.n_axial);
            for k in 0..transient.n_axial {
                // The node's temperature history, GATHERED at the venting
                // samples -- never a prefix. This is the whole point of
                // VentingWindow.
                let full = transient.node_history(r, k);
                let history = venting.gather(&full);

                let int_kernel = integrate_diffusion_over_time(
                    nuclide.z,
                    &kept_times,
                    &history,
                    DiffusionMaterial::Kernel,
                );
                let int_graphite = if volatile {
                    vec![Area::new::<square_meter>(0.0); n_keep]
                } else {
                    integrate_diffusion_over_time(
                        nuclide.z,
                        &kept_times,
                        &history,
                        DiffusionMaterial::Graphite,
                    )
                };

                // The normal-operation state this node starts the accident in.
                // Prescribed as zero pools: this crate does not run a
                // normal-operation history, so nothing has accumulated in the
                // coolant, on surfaces or in the clean-up system beforehand.
                let pools = node_pools.map_or_else(zero_pools, |v| v[r * transient.n_axial + k]);
                circulating_atoms += pools.circulating_activity;
                plate_out_atoms += pools.plate_out_activity;
                let node = bridge_node(&pools, axial[k], lambda);

                for t in 0..n_keep {
                    let rf_k: Ratio = release_fraction_transient(
                        nuclide.z,
                        group,
                        int_kernel[t],
                        plant.kernel_radius,
                        Some(plant.sic_thickness),
                        boon_lay::triso_atops_fork::release_models::ReleaseMaterial::Kernel,
                    );
                    let rf_g: Ratio = release_fraction_transient(
                        nuclide.z,
                        group,
                        int_graphite[t],
                        plant.graphite_thickness,
                        None,
                        boon_lay::triso_atops_fork::release_models::ReleaseMaterial::Graphite,
                    );
                    let atoms_k = boon_lay::triso_atops_fork::accident::release_activity(
                        group,
                        plant.fractions,
                        node,
                        rf_k.get::<ratio>(),
                        plant.clean_up_fitted,
                        AccidentMaterial::Kernel,
                        true,
                        nuclide.z,
                    );
                    let atoms_g = boon_lay::triso_atops_fork::accident::release_activity(
                        group,
                        plant.fractions,
                        node,
                        rf_g.get::<ratio>(),
                        plant.clean_up_fitted,
                        AccidentMaterial::Graphite,
                        true,
                        nuclide.z,
                    );
                    if atoms_k < 0.0 || atoms_g < 0.0 {
                        caveats.negative_atom_count_seen = true;
                    }
                    kernel_curies[t] += atoms_to_curies(atoms_k, lam_hz);
                    graphite_curies[t] += atoms_to_curies(atoms_g, lam_hz);
                }
            }
        }

        let cumulative = accident_release_curies(
            &kernel_curies,
            &graphite_curies,
            venting.fractions(),
            atoms_to_curies(circulating_atoms, lam_hz),
            atoms_to_curies(plate_out_atoms, lam_hz),
            plant.x_liftoff,
        );

        cumulative_final.push((
            nuclide.name.to_string(),
            units::from_curies(*cumulative.last().unwrap_or(&0.0))
                .get::<uom::si::radioactivity::becquerel>(),
        ));

        // Cumulative -> per-window. See the module docs.
        let mut per_window = Vec::with_capacity(n_keep - 1);
        for i in 0..n_keep - 1 {
            let inc = if i == 0 {
                cumulative[1]
            } else {
                cumulative[i + 1] - cumulative[i]
            };
            if inc < 0.0 {
                caveats.negative_atom_count_seen = true;
            }
            // SourceTerm rejects a negative activity, so the floor happens
            // here and is recorded above rather than hidden.
            per_window.push(units::from_curies(inc.max(0.0)));
        }

        releases.push(NuclideRelease {
            label: nuclide.name.to_string(),
            decay_constant: lambda,
            deposition_group: deposition_group_of(nuclide),
            released: per_window,
        });
    }

    // Windows span venting sample i to i+1, contiguous by construction even
    // when the venting mask is gappy.
    let windows: Vec<ReleaseWindow> = (0..n_keep - 1)
        .map(|i| ReleaseWindow::new(kept_times[i], kept_times[i + 1]))
        .collect();

    Ok(AccidentRelease {
        source_term: SourceTerm::new(windows, releases),
        caveats,
        venting,
        screened_out,
        cumulative_final,
    })
}

/// A transient's peak against the diffusion correlation's fitted range, for
/// reporting.
#[must_use]
pub fn temperature_is_inside_the_fitted_range(t: ThermodynamicTemperature) -> bool {
    let c = t.get::<degree_celsius>();
    (DIFFUSION_FIT_MIN_CELSIUS..=DIFFUSION_FIT_MAX_CELSIUS).contains(&c)
}

/// The total duration a source term spans, for sizing a dispersion run.
#[must_use]
pub fn source_term_duration(term: &SourceTerm) -> Time {
    let start = term.windows[0].start.get::<second>();
    Time::new::<second>(term.end().get::<second>() - start)
}
