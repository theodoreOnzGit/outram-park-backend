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
use boon_lay::triso_atops_fork::run_selection::select_nuclides_accident;
use boon_lay::triso_atops_fork::TrisoAtopsNuclide;
use changi::activity::deposition::DepositionGroup;
use changi::activity::source::{NuclideRelease, ReleaseWindow, SourceTerm};
use uom::si::area::square_meter;
use uom::si::f64::{Area, Length, Pressure, Ratio, ThermodynamicTemperature, Time};
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::second;

use crate::error::{Caveats, Error, Result};
use crate::inventory::CoreInventory;
use crate::scenario::TemperatureTransient;
use crate::units;

use super::venting::VentingWindow;

/// Lower edge of the Arrhenius diffusion correlation's fitted range, degrees
/// Celsius. Outside it `boon-lay` clamps rather than extrapolating; crossing it
/// sets [`Caveats::diffusion_coefficient_clamped`].
pub const DIFFUSION_FIT_MIN_CELSIUS: f64 = 700.0;

/// Upper edge of the fitted range, degrees Celsius. See
/// [`DIFFUSION_FIT_MIN_CELSIUS`].
pub const DIFFUSION_FIT_MAX_CELSIUS: f64 = 2400.0;

/// The geometry, failure fractions and plant constants a release needs.
///
/// Every field is prescribed by the caller. There are no defaults, deliberately
/// — a default geometry would be a specific reactor's, wearing no label.
#[derive(Debug, Clone, Copy, PartialEq)]
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
    /// - `x_liftoff` — fraction of plated-out activity lifted off. Note this
    ///   crate currently starts from empty pools (see [`zero_pools`]), so it
    ///   has nothing to lift off and this parameter does nothing until a
    ///   normal-operation history is wired in.
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
        }
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
    /// Nuclides dropped by the half-life screen, with the reason.
    pub screened_out: Vec<String>,
}

/// A normal-operation state with every pool empty.
///
/// Used when no normal-operation history has been run, which is this crate's
/// current position: nothing has accumulated in the coolant, on surfaces or in
/// the clean-up system before the accident starts.
///
/// **That is a modelling choice with a direction.** A real plant carries a
/// circulating and plated-out inventory built up over the operating cycle, and
/// an accident lifts some of it off. Starting from zero therefore
/// **under-predicts** the early release, by however much that pre-existing
/// inventory would have contributed. Wiring a normal-operation history in is
/// the obvious next step and is not done here.
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
/// [`Error::UnknownNuclide`] if a name is not in the supported table;
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
    let (selected, _errors) = select_nuclides_accident(&names, transient.end(), None);
    let kept: Vec<String> = selected.iter().map(|n| n.name.to_string()).collect();
    let screened_out: Vec<String> = names
        .iter()
        .filter(|n| !kept.iter().any(|k| k == *n))
        .map(|n| (*n).to_string())
        .collect();
    if selected.is_empty() {
        return Err(Error::UnknownNuclide(format!(
            "no nuclide survived selection out of {names:?}"
        )));
    }

    // -- venting
    let rate = mean_temperature_rate(&transient.times, &transient.all_node_histories());
    let hot = transient.hot_node_history();
    let (vent_fraction, vent_times) = coolant_release(
        &transient.times,
        &rate,
        &hot,
        plant.coolant_pressure,
    );
    let venting =
        VentingWindow::from_coolant_release(&transient.times, &vent_times, vent_fraction.clone())?;
    caveats.first_sample_forced_fully_vented = !venting.is_empty();
    caveats.venting_mask_was_gappy = !venting.is_contiguous();
    if venting.len() < 2 {
        return Err(Error::TransientTooShort(venting.len()));
    }

    let n_keep = venting.len();
    let kept_times = venting.times(&transient.times);

    // -- per-nuclide, per-node release
    let mut releases = Vec::with_capacity(selected.len());
    for nuclide in &selected {
        let group = nuclide.element_group();
        let volatile = matches!(group, ElementGroup::NobleGas | ElementGroup::Halogen);
        let lambda = nuclide.decay_constant();
        let lam_hz = lambda.get::<uom::si::frequency::hertz>();

        let inv = inventory
            .nuclides
            .iter()
            .find(|n| n.name == nuclide.name)
            .ok_or_else(|| Error::UnknownNuclide(nuclide.name.to_string()))?;

        let mut kernel_curies = vec![0.0_f64; n_keep];
        let mut graphite_curies = vec![0.0_f64; n_keep];

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
                let node = bridge_node(&zero_pools(), axial[k], lambda);

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
            0.0,
            0.0,
            plant.x_liftoff,
        );

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
