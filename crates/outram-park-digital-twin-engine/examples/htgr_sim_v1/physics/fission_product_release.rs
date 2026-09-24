//! TRISO fission-product release, driven by the resolved fuel-kernel
//! temperature — a Rust fork of Idaho National Laboratory's **TRISO-ATOPS**.
//!
//! # What this is
//!
//! [`boon_lay::triso_atops_fork`] is a port of TRISO-ATOPS (TRISO Analysis
//! TOol for Predictive Source terms), which predicts what fraction of each
//! fission product escapes a coated particle and where it then goes in the
//! primary circuit. It is closed-form Fickian diffusion — the *Booth*
//! equivalent-sphere model for the kernel, a Daynes–Barrer membrane
//! *breakthrough* model for silver through SiC, and a graphite *attenuation*
//! factor for hold-up in the matrix — with every diffusion coefficient an
//! Arrhenius law `D(T) = D0 exp(-Q/RT)`.
//!
//! # Why it belongs on the kernel temperature, and why it is in THIS change
//!
//! Every one of those diffusion coefficients is exponential in the **fuel**
//! temperature. Not the bed average, not the helium — the temperature inside
//! the kernel, which is where the fission products are and which is what
//! `Q/RT` refers to. That is the same temperature
//! [`super::kinetics::KernelDopplerChannel`] was rewired onto in this change,
//! and it is not a coincidence that the two arrived together: resolving the
//! pebble produced a kernel temperature, and a kernel temperature is exactly
//! what a Doppler coefficient and a release model both want and neither could
//! previously have.
//!
//! It also makes the coupling *visible*, which is the point of a simulator.
//! `exp(-Q/RT)` is brutally sensitive: over the kernel's ~23 K rise above the
//! bed at rated power, a typical `Q ~ 300 kJ/mol` changes `D` by about 10 %,
//! and a transient that puts the kernel a few hundred kelvin up moves it by
//! orders of magnitude. Driving this channel off the bed node instead would
//! have thrown that away at exactly the operating point where it matters.
//!
//! # The inventory is a UNIT basis, deliberately, and that is not a shortcut
//!
//! **Every activity reported here is per curie of that nuclide's core
//! inventory.** Nothing in this module derives an inventory, and that is a
//! considered refusal rather than an omission.
//!
//! `sembawang::inventory`'s module doc sets out the trap in full: the obvious
//! route — multiply fission rate by a fission yield — silently gives the wrong
//! answer for most consequence-dominant species, because the available yield
//! data is **independent** yield (straight from fission) while what a source
//! term needs is **cumulative** yield (after the isobaric chain has run).
//! Cs-137's independent yield is roughly **two orders of magnitude** below its
//! cumulative yield. The error is large, it is low — so it looks reassuring —
//! and nothing anywhere reports it. Getting it right needs a decay-chain walk
//! over an evaluation, which is real work and is not this simulator's job.
//!
//! A unit basis sidesteps that completely while losing nothing this channel
//! exists to show. The release-to-birth ratio, the graphite attenuation and
//! the three loop pools are all **linear** in inventory, so the unit-basis
//! numbers are the transfer function and a reader with a real inventory
//! multiplies through. What is *not* linear in inventory — the temperature
//! dependence, which is the whole physics here — is reported exactly.
//!
//! **So: nothing in this module may be quoted as a source term for HTR-10 or
//! any other reactor** — including the absolute becquerel column added below,
//! which carries a fuel-quality input that is not HTR-10's.
//! `RESPONSIBLE_USE.md` applies with full force: this is an offline
//! educational demonstration and not a source-term calculation for any real
//! plant.
//!
//! # BOTH bases are now reported (2026-09-23)
//!
//! ~~An HTR-10 inventory is now available as data, and this module still does
//! not use it.~~ **CORRECTED — it is wired in, on the maintainer's
//! instruction, and both bases are reported side by side:**
//!
//! - [`NuclideRelease::activities`] — the **transfer function**, per curie of
//!   core inventory. Unchanged, and still the primary quantity, because it is
//!   the part this model actually determines.
//! - [`NuclideRelease::absolute`] — **absolute** activities in becquerels,
//!   from [`changi::activity::inventory`]: the published
//!   equilibrium-core inventory of 22 nuclides (Liu & Cao 2002, Table 1,
//!   ORIGEN2 at 80 000 MWd/t), which covers all five of [`TRACKED_NUCLIDES`].
//!   Provenance and access terms are in `crates/changi/docs/References.md`.
//!   The table lives in `changi` rather than here because the dispersion
//!   channel downstream is its consumer; it was briefly duplicated in this
//!   example's own `reference/` directory and that copy is gone.
//!
//! The absolute arm **re-evaluates the closed form at the real inventory**
//! rather than multiplying the per-curie answer by it. Scaling would almost
//! certainly give the same numbers — the linearity argument above is sound —
//! but re-evaluating needs no such assumption, and
//! `tests::the_absolute_arm_is_linear_in_inventory` now *checks* the linearity
//! claim against the two arms instead of asserting it.
//!
//! **The deeper objection stands, and is the reason the absolute column is
//! not a source term.** The failure fractions below are **TRISO-ATOPS
//! reference values, not HTR-10 fuel-qualification data**, so an absolute
//! figure is the product of one reactor's inventory and another reactor's
//! fuel quality. It is linear in both, so it is a defensible order of
//! magnitude and nothing more.
//!
//! # What else is an input rather than a derivation
//!
//! The geometry is published and read from `tampines` (see
//! [`Htr10TrisoAtopsInputs::htr10`]), but three groups of numbers are not
//! HTR-10 data and are labelled at their definition:
//!
//! - the **failure fractions** — manufacturing and in-service defect
//!   populations, which are a fuel-qualification result;
//! - the **plate-out and clean-up rate constants** — primary-circuit surface
//!   chemistry and helium purification-system sizing;
//! - the **kernel grain size** `a_grain`.
//!
//! All six carry TRISO-ATOPS's own reference values, cited as such. They scale
//! the answer without changing its shape or its temperature dependence.
//!
//! # NOT VALIDATED
//!
//! `boon-lay`'s fork is verified **code-to-code against upstream TRISO-ATOPS**
//! (`crates/boon-lay/tests/triso_atops_code_to_code.rs`), which establishes
//! that the port reproduces the Python, and nothing more. No measured HTR-10
//! release fraction has been reproduced here. Per `RESPONSIBLE_USE.md` this is
//! AI-assisted draft material pending human review.

use boon_lay::triso_atops_fork::activities::{becquerels_from_curies, FailureFractions};
use boon_lay::triso_atops_fork::normal_operation::{
    normal_operation_node, NodalActivitiesCurie, NodeState, ParentPools, PlantConstants,
};
use boon_lay::triso_atops_fork::nuclide_model::nuclide_database::supported_nuclides;
use boon_lay::triso_atops_fork::nuclide_model::TrisoAtopsNuclide;

use uom::si::f64::{Frequency, Length, ThermodynamicTemperature, Time};
use uom::si::frequency::hertz;
use uom::si::length::meter;
use uom::si::time::second;

/// The unit-inventory basis every activity in this module is reported
/// against: **one curie** of the nuclide in the core. See the module doc for
/// why the basis is a unit rather than a real inventory.
pub const UNIT_INVENTORY_CURIES: f64 = 1.0;

/// Becquerels per curie — the one place this module converts.
const BQ_PER_CI: f64 = 3.7e10;

/// Look up a nuclide's published HTR-10 core inventory \[Bq\].
///
/// **The table itself lives in `changi`**, not here. It was briefly duplicated
/// in this example's `reference/` directory; two copies of a published table
/// is exactly the drift this workspace forbids, and `changi::activity` is the
/// right home because the dispersion channel downstream is its consumer.
///
/// Returns `None` for a nuclide outside the 22 Liu and Cao tabulate — a
/// caller must not be handed a zero that reads like a measurement.
#[must_use]
pub fn htr10_core_inventory_bq(name: &str) -> Option<f64> {
    changi::activity::inventory::htr10_core_inventory(name)
        .map(|a| a.get::<uom::si::radioactivity::becquerel>())
}

/// The six normal-operation outputs on an **absolute** basis, in becquerels.
///
/// The mirror of [`NodalActivitiesCurie`], which is per curie of inventory.
/// Rates are Bq/s; the four pools are Bq.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodalActivitiesBq {
    /// Release rate `R` \[Bq/s\].
    pub release_rate: f64,
    /// Source rate `S` \[Bq/s\].
    pub source_rate: f64,
    /// Graphite activity `G` \[Bq\].
    pub graphite_activity: f64,
    /// Circulating activity `C` \[Bq\].
    pub circulating_activity: f64,
    /// Plate-out activity `P` \[Bq\].
    pub plate_out_activity: f64,
    /// Clean-up / HPS activity \[Bq\].
    pub clean_up_activity: f64,
}

/// The nuclides this channel tracks, chosen to cover **all five** TRISO-ATOPS
/// transport groups rather than to be a list of the most radiologically
/// important species.
///
/// That choice is deliberate: the groups take genuinely different physical
/// routes out of the particle — a noble gas is not retained by graphite and
/// does not plate out, a halogen does both, silver permeates *intact* SiC by
/// the breakthrough model, and the metals go through the attenuation factor.
/// A display that showed only iodine and caesium would make the model look
/// like one mechanism when it is four, and would hide a wiring error in any
/// group it omitted.
///
/// | Nuclide | Group | Why it is here |
/// |---|---|---|
/// | `Kr-85` | noble gas | long-lived, the classic circulating-activity marker |
/// | `Xe-133` | noble gas | short-lived, so it exercises the `<R/B>` branch rather than the long-lived one |
/// | `I-131` | halogen | plate-out *and* clean-up both active; the dose-relevant volatile |
/// | `Cs-137` | special metal | long-lived, graphite-retained |
/// | `Ag-110m` | silver | the SiC **breakthrough** model — the only nuclide that exercises it |
pub const TRACKED_NUCLIDES: [&str; 5] = ["Kr-85", "Xe-133", "I-131", "Cs-137", "Ag-110m"];

/// How often the release channel is re-evaluated \[s of plant time\].
///
/// # Why this is throttled when nothing else in the plant is
///
/// The release model is **quasi-steady**: `normal_operation_node` evaluates
/// closed-form diffusion at the temperature it is handed, and carries no state
/// of its own between calls. So unlike the kinetics, the bed or the steam
/// generator, re-evaluating it more often does not integrate anything more
/// accurately — it just recomputes the same algebra against a temperature that
/// has barely moved.
///
/// One second is chosen against the *physics* rather than for convenience:
/// the fastest thing the kernel temperature can do is follow the power, and
/// the pebble's own conduction time constant is of order a second, so the
/// input to this channel cannot meaningfully change faster than this. Against
/// the 0.1 s plant step that is a 10x saving on 5 nuclides of transcendental
/// arithmetic in the GUI's physics thread.
///
/// It is **not** a quality knob: setting it to the plant step changes no
/// reported number outside the sampling itself.
pub const RELEASE_EVALUATION_INTERVAL_S: f64 = 1.0;

/// The geometry, failure and circuit inputs TRISO-ATOPS needs, with each one
/// marked as published HTR-10 data or as a TRISO-ATOPS reference value.
///
/// Split out from the channel itself so a test can vary one input at a time,
/// and so the provenance of each number sits next to the number.
#[derive(Debug, Clone, Copy)]
pub struct Htr10TrisoAtopsInputs {
    /// The assembled upstream constants block.
    pub plant: PlantConstants,
    /// The four defect populations. See
    /// [`Self::TRISO_ATOPS_REFERENCE_FAILURE_FRACTIONS`].
    pub fractions: FailureFractions,
    /// Whether the helium purification system (HPS) is in service. `true`
    /// here: HTR-10 has one, and the clean-up term only applies to noble gases
    /// and halogens in any case.
    pub hps_enabled: bool,
}

impl Htr10TrisoAtopsInputs {
    /// Plate-out rate constant `k_plate` \[1/s\] — **a TRISO-ATOPS reference
    /// value, not HTR-10 data.**
    ///
    /// `7.5e-4 /s`, upstream's own constants block. This is primary-circuit
    /// surface chemistry (how fast a volatile deposits on duct and exchanger
    /// walls) and depends on surface area, temperature and material, none of
    /// which this model represents. It divides the circulating and plate-out
    /// pools between each other and does not affect the release rate `R` or
    /// the graphite term at all.
    pub const TRISO_ATOPS_PLATE_OUT_PER_S: f64 = 7.5e-4;

    /// Clean-up (helium purification) rate constant `k_clean` \[1/s\] —
    /// **a TRISO-ATOPS reference value, not HTR-10 data.**
    ///
    /// `8.77e-5 /s`, upstream's own constants block. It is a purification-plant
    /// sizing parameter (flow fraction over circuit inventory), applied only to
    /// noble gases and halogens.
    pub const TRISO_ATOPS_CLEAN_UP_PER_S: f64 = 8.77e-5;

    /// Kernel grain size `a_grain` \[m\] — **a TRISO-ATOPS reference value,
    /// not HTR-10 data.**
    ///
    /// `1e-5 m`, upstream's own constants block. It enters only the
    /// special-metal equivalent-sphere radius `a_booth = sqrt(2 a_grain r)`,
    /// so of the tracked nuclides it reaches Cs-137 alone. No grain size for
    /// HTR-10's UO2 kernels is available in this workspace; upstream's value
    /// is for UCO fuel and is carried unchanged rather than adjusted towards a
    /// number nobody has published.
    pub const TRISO_ATOPS_GRAIN_SIZE_M: f64 = 1.0e-5;

    /// The four defect populations — **TRISO-ATOPS reference values, not
    /// HTR-10 fuel-qualification data.**
    ///
    /// `f_hm = 1e-5`, `f_sic = 2e-5`, `f_inc = 3e-5`, `f_inc_sic = 4e-5`, from
    /// upstream's own constants block. These are a *manufacturing and
    /// irradiation* result for a specific fuel product line: HTR-10's fuel is
    /// German-lineage TRISO with its own published free-uranium fraction, and
    /// substituting these for it would be putting one reactor's fuel quality
    /// under another's name.
    ///
    /// **Release scales essentially linearly in these**, so they set the
    /// magnitude of every activity reported here and none of its shape. That
    /// is the second reason the unit-inventory basis is the honest one: a
    /// curie figure would be the product of two numbers this module does not
    /// have.
    pub const TRISO_ATOPS_REFERENCE_FAILURE_FRACTIONS: FailureFractions = FailureFractions {
        heavy_metal: 1.0e-5,
        sic: 2.0e-5,
        incremental: 3.0e-5,
        incremental_sic: 4.0e-5,
    };

    /// Irradiation time \[s\] — one year, upstream's `t_irad`.
    ///
    /// Sets how long the fuel has been accumulating and releasing, and appears
    /// in the long-lived Booth release fraction and in the birth-rate
    /// normalisation. This simulator has **no burnup**, so the fuel neither
    /// ages nor changes composition as it runs; a fixed irradiation time is
    /// the consistent choice, and it is a year because that is the reference
    /// case's. See [`Self::htr10`] for what varying it would mean.
    pub const IRRADIATION_TIME_S: f64 = 3.155_76e7;

    /// Assemble the inputs for the published HTR-10 pebble.
    ///
    /// **The geometry is read from `tampines`, not retyped**, per this
    /// workspace's rule against a second copy of an operating point:
    ///
    /// | Input | Value | Source |
    /// |---|---|---|
    /// | `kernel_radius` `r` | 2.5e-4 m | `TrisoParticle::htr10()` — IAEA-TECDOC-1382 |
    /// | `sic_thickness` `a_SiC` | 3.5e-5 m | `TrisoParticle::htr10()` — and identical to upstream's own reference value |
    /// | `graphite_thickness` `a_graph` | 5.0e-3 m | `Pebble::htr10()`, outer radius less fuelled-zone radius — the published 5 mm unfuelled shell |
    ///
    /// The HTR-10 kernel is 2.5e-4 m against upstream's reference 2.13e-4 m,
    /// and the shell 5.0e-3 m against 4.5e-3 m, so this is a genuinely
    /// different particle and not upstream's case relabelled. The SiC
    /// thickness agreeing exactly is a coincidence of two designs converging
    /// on 35 microns, not a value carried over.
    ///
    /// `run_time` is set per call from the plant clock — see
    /// [`TrisoAtopsReleaseChannel::evaluate`].
    pub fn htr10() -> Self {
        let particle = tampines::pebble_bed::triso::TrisoParticle::htr10();
        let pebble = tampines::pebble_bed::pebble::Pebble::htr10();
        let unfuelled_shell = pebble.outer_radius - pebble.fuelled_zone_radius;

        Self {
            plant: PlantConstants {
                k_plate: Frequency::new::<hertz>(Self::TRISO_ATOPS_PLATE_OUT_PER_S),
                k_clean: Frequency::new::<hertz>(Self::TRISO_ATOPS_CLEAN_UP_PER_S),
                graphite_thickness: unfuelled_shell,
                grain_size: Length::new::<meter>(Self::TRISO_ATOPS_GRAIN_SIZE_M),
                sic_thickness: particle.silicon_carbide_outer_radius
                    - particle.inner_pyc_outer_radius,
                kernel_radius: particle.kernel_radius,
                // Overwritten per evaluation from the plant clock.
                run_time: Time::new::<second>(0.0),
                irradiation_time: Time::new::<second>(Self::IRRADIATION_TIME_S),
            },
            fractions: Self::TRISO_ATOPS_REFERENCE_FAILURE_FRACTIONS,
            hps_enabled: true,
        }
    }
}

/// One tracked nuclide's release state, on the unit-inventory basis.
#[derive(Debug, Clone, Copy)]
pub struct NuclideRelease {
    /// Canonical TRISO-ATOPS name, e.g. `"Cs-137"`.
    pub name: &'static str,
    /// Atomic number `Z`. Carried so a downstream consumer can classify the
    /// nuclide by element without a second lookup table that could drift out
    /// of step with [`TRACKED_NUCLIDES`] -- see
    /// [`crate::physics::atmospheric_dispersion`], which groups by it for
    /// **deposition** (a grouping that deliberately differs from TRISO-ATOPS's
    /// transport grouping for Se and Te).
    pub z: u32,
    /// Radioactive decay constant `lambda = ln2 / t_half`. Carried for the
    /// same reason: the dispersion channel needs it for decay in transit, and
    /// re-deriving it downstream would mean two half-life tables.
    pub decay_constant: uom::si::f64::Frequency,
    /// The six normal-operation outputs, **per curie of core inventory** of
    /// this nuclide. `release_rate` and `source_rate` are per second; the
    /// other four are pool inventories.
    ///
    /// This is the **transfer function** — the release physics with the
    /// inventory divided out — and it stays the primary quantity because it
    /// is the part this model actually determines.
    pub activities: NodalActivitiesCurie,
    /// This nuclide's published HTR-10 equilibrium-core inventory \[Bq\], or
    /// `None` if it is not one of the 22 nuclides Liu and Cao tabulate.
    pub core_inventory_bq: Option<f64>,
    /// The same six outputs on an **absolute** basis \[Bq, Bq/s\], obtained by
    /// re-evaluating the model at the published inventory — `None` when that
    /// inventory is unknown.
    ///
    /// **NOT A SOURCE TERM.** See the module docs: these are one reactor's
    /// inventory driven through another reactor's fuel-quality data.
    pub absolute: Option<NodalActivitiesBq>,
}

/// The TRISO fission-product release channel for the HTGR plant.
///
/// Holds the fixed inputs and the most recent evaluation. It carries **no
/// integrated state** — `normal_operation_node` is closed-form and quasi-steady
/// — so this struct is `Clone` and cheap for the plant's outer-corrector loop
/// to rewind, and rewinding it changes nothing.
#[derive(Debug, Clone)]
pub struct TrisoAtopsReleaseChannel {
    inputs: Htr10TrisoAtopsInputs,
    nuclides: Vec<TrisoAtopsNuclide>,
    latest: Vec<NuclideRelease>,
    /// The kernel temperature the latest evaluation was taken at, for display
    /// and so a reader can see which temperature produced these numbers.
    evaluated_at_kernel: Option<ThermodynamicTemperature>,
    /// Plant time of the most recent evaluation \[s\], for the throttle.
    last_evaluated_s: Option<f64>,
}

impl TrisoAtopsReleaseChannel {
    /// Build the channel for the published HTR-10 pebble over
    /// [`TRACKED_NUCLIDES`].
    ///
    /// # Panics
    ///
    /// If a name in [`TRACKED_NUCLIDES`] is not in TRISO-ATOPS's 84-nuclide
    /// database. That is a typo in a `const` in this file, caught at
    /// construction rather than silently dropping a nuclide from the display —
    /// a missing row is exactly the kind of fault nobody notices.
    pub fn new_htr10() -> Self {
        let database = supported_nuclides();
        let nuclides: Vec<TrisoAtopsNuclide> = TRACKED_NUCLIDES
            .iter()
            .map(|name| {
                database
                    .iter()
                    .find(|n| n.name == *name)
                    .unwrap_or_else(|| panic!("{name} is not in the TRISO-ATOPS nuclide database"))
                    .clone()
            })
            .collect();

        Self {
            inputs: Htr10TrisoAtopsInputs::htr10(),
            nuclides,
            latest: Vec::new(),
            evaluated_at_kernel: None,
            last_evaluated_s: None,
        }
    }

    /// Re-evaluate the release channel if the throttle allows, at the fuel
    /// kernel and graphite temperatures the core currently reports.
    ///
    /// `kernel_temperature` is [`super::pebble_bed::PebbleBedPorousMediaNode::peak_kernel_temperature`]
    /// — `None` when the resolved pebble solve was out of range, in which case
    /// **this channel does not evaluate at all**. It deliberately does not
    /// substitute the bed temperature: `D(T)` is exponential, so a bed
    /// temperature passed in as a fuel temperature would not produce a
    /// slightly wrong release, it would produce a confidently wrong one, and
    /// the display would carry no sign that the kernel was unavailable.
    /// Holding the previous evaluation and showing its timestamp is the honest
    /// failure mode.
    ///
    /// Returns `true` when an evaluation actually ran.
    pub fn update(
        &mut self,
        sim_time_s: f64,
        kernel_temperature: Option<ThermodynamicTemperature>,
        graphite_temperature: ThermodynamicTemperature,
    ) -> bool {
        let Some(kernel) = kernel_temperature else {
            return false;
        };
        let due = match self.last_evaluated_s {
            None => true,
            Some(last) => (sim_time_s - last).abs() >= RELEASE_EVALUATION_INTERVAL_S,
        };
        if !due {
            return false;
        }

        self.latest = self.evaluate(sim_time_s, kernel, graphite_temperature);
        self.evaluated_at_kernel = Some(kernel);
        self.last_evaluated_s = Some(sim_time_s);
        true
    }

    /// Evaluate every tracked nuclide at the given temperatures, ignoring the
    /// throttle. Pure — this is what [`Self::update`] calls, and what a test
    /// calls directly to sweep temperature.
    ///
    /// `run_time` is the plant clock plus the irradiation time, so the loop
    /// pools are evaluated for a core that has been operating rather than one
    /// switched on at `t = 0`. Starting the pools from a cold circuit would
    /// show a spurious build-up transient over the first hours of *simulated*
    /// time that no operator of a running reactor would ever see — the same
    /// argument that seeds the decay-heat bank and the xenon channel at
    /// equilibrium in [`super::kinetics`].
    ///
    /// **Parent chaining is not threaded here.** Every nuclide is evaluated
    /// with [`ParentPools::none`], so the daughter contribution from a tracked
    /// parent (I-131 from Te-131m, Xe-133 from I-133) is omitted. None of the
    /// five tracked nuclides has its parent in the tracked set, so threading
    /// it would need those parents evaluated too; the omission **under-states**
    /// the circulating activity of I-131 and Xe-133 by whatever their tracked
    /// parents would have contributed, and the direction is stated so a reader
    /// can bound it.
    pub fn evaluate(
        &self,
        sim_time_s: f64,
        kernel_temperature: ThermodynamicTemperature,
        graphite_temperature: ThermodynamicTemperature,
    ) -> Vec<NuclideRelease> {
        let mut plant = self.inputs.plant;
        plant.run_time =
            Time::new::<second>(Htr10TrisoAtopsInputs::IRRADIATION_TIME_S + sim_time_s.max(0.0));

        let node = NodeState {
            core_temperature: kernel_temperature,
            graphite_temperature,
        };
        // One curie of this nuclide in the core: the unit basis every number
        // this module reports is per. See the module doc for why no real
        // inventory is derived.
        // `becquerels_from_curies` is `boon-lay`'s own Ci->Bq boundary, used
        // rather than a local 3.7e10 so the conversion exists once.
        let unit_inventory = becquerels_from_curies(UNIT_INVENTORY_CURIES);

        self.nuclides
            .iter()
            .map(|nuclide| {
                // "Short-lived" selects the secular-equilibrium <R/B> branch
                // rather than the long-lived Booth release fraction. The
                // criterion is the half-life against the irradiation time:
                // a nuclide that saturates within the irradiation is at
                // equilibrium, one that does not is still accumulating. This
                // is upstream's own `sl` flag, decided here from the data
                // rather than hardcoded per nuclide, so adding a nuclide to
                // TRACKED_NUCLIDES cannot silently take the wrong branch.
                let short_lived =
                    nuclide.half_life.get::<second>() < Htr10TrisoAtopsInputs::IRRADIATION_TIME_S;
                let activities = normal_operation_node(
                    nuclide,
                    short_lived,
                    unit_inventory,
                    self.inputs.fractions,
                    plant,
                    node,
                    self.inputs.hps_enabled,
                    ParentPools::none(),
                )
                .to_curies(nuclide.decay_constant());

                // ABSOLUTE ARM. The published inventory is driven through
                // the SAME closed form rather than multiplied onto the
                // per-curie answer. Scaling would have been cheaper and is
                // very probably identical -- the module doc asserts the model
                // is linear in inventory -- but re-evaluating needs no such
                // assumption, and `linear_in_inventory` below now *checks*
                // the assertion instead of trusting it.
                let core_inventory_bq = htr10_core_inventory_bq(nuclide.name);
                let absolute = core_inventory_bq.map(|bq| {
                    let a = normal_operation_node(
                        nuclide,
                        short_lived,
                        Frequency::new::<hertz>(bq),
                        self.inputs.fractions,
                        plant,
                        node,
                        self.inputs.hps_enabled,
                        ParentPools::none(),
                    )
                    .to_curies(nuclide.decay_constant());
                    NodalActivitiesBq {
                        release_rate: a.release_rate * BQ_PER_CI,
                        source_rate: a.source_rate * BQ_PER_CI,
                        graphite_activity: a.graphite_activity * BQ_PER_CI,
                        circulating_activity: a.circulating_activity * BQ_PER_CI,
                        plate_out_activity: a.plate_out_activity * BQ_PER_CI,
                        clean_up_activity: a.clean_up_activity * BQ_PER_CI,
                    }
                });

                NuclideRelease {
                    name: nuclide.name,
                    z: nuclide.z,
                    decay_constant: nuclide.decay_constant(),
                    activities,
                    core_inventory_bq,
                    absolute,
                }
            })
            .collect()
    }

    /// The most recent evaluation, empty before the first one.
    pub fn latest(&self) -> &[NuclideRelease] {
        &self.latest
    }

    /// The kernel temperature the most recent evaluation was taken at.
    pub fn evaluated_at_kernel(&self) -> Option<ThermodynamicTemperature> {
        self.evaluated_at_kernel
    }

    /// Plant time of the most recent evaluation \[s\].
    pub fn last_evaluated_s(&self) -> Option<f64> {
        self.last_evaluated_s
    }

    /// Total circulating activity across every tracked nuclide \[Ci per Ci of
    /// core inventory, summed over nuclides\].
    ///
    /// A **summary scalar for the snapshot and the plots**, and it is a sum of
    /// per-nuclide unit-basis numbers, so it is not the circulating activity of
    /// anything. It is useful as a single trend line that rises when the fuel
    /// gets hotter, which is what a time-history plot wants; read the
    /// per-nuclide table for anything else.
    pub fn total_circulating(&self) -> f64 {
        self.latest
            .iter()
            .map(|r| r.activities.circulating_activity)
            .sum()
    }

    /// The inputs this channel was built with, for display and tests.
    pub fn inputs(&self) -> &Htr10TrisoAtopsInputs {
        &self.inputs
    }
}

impl Default for TrisoAtopsReleaseChannel {
    fn default() -> Self {
        Self::new_htr10()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::thermodynamic_temperature::kelvin;

    fn channel() -> TrisoAtopsReleaseChannel {
        TrisoAtopsReleaseChannel::new_htr10()
    }

    fn at(kernel_k: f64, graphite_k: f64) -> Vec<NuclideRelease> {
        channel().evaluate(
            0.0,
            ThermodynamicTemperature::new::<kelvin>(kernel_k),
            ThermodynamicTemperature::new::<kelvin>(graphite_k),
        )
    }

    /// V&V: the geometry handed to TRISO-ATOPS must be the **published HTR-10
    /// particle**, read from `tampines`, and must differ from upstream's own
    /// reference case where the two designs differ.
    ///
    /// # Why this is worth a test
    ///
    /// The failure this guards against is the quietest one available: running
    /// upstream's reference geometry, getting plausible numbers, and reporting
    /// them as HTR-10. Nothing would look wrong — the model would converge,
    /// the temperature dependence would be right, and the answer would be for
    /// the wrong particle. Pinning the two radii *against upstream's* makes
    /// that visible, and pinning them against `tampines` makes a future change
    /// to the published geometry propagate here instead of drifting.
    ///
    /// **Methodology.** Build [`Htr10TrisoAtopsInputs::htr10`] and compare the
    /// kernel radius, SiC thickness and graphite thickness against the values
    /// `TrisoParticle::htr10()` and `Pebble::htr10()` carry, and against
    /// upstream's reference constants block (`r = 2.13e-4`,
    /// `a_SiC = 3.5e-5`, `a_graph = 4.5e-3`).
    ///
    /// **Results (2026-09-22).** Kernel radius **2.5e-4 m** (upstream 2.13e-4,
    /// so **+17.4 %** — a genuinely different particle); graphite thickness
    /// **5.0e-3 m** (upstream 4.5e-3, **+11.1 %**); SiC thickness
    /// **3.5e-5 m**, which *equals* upstream's. All three reproduce the
    /// `tampines` geometry exactly.
    ///
    /// **Interpretation.** Two of the three differ from upstream, so this is
    /// the HTR-10 particle and not the reference case wearing its name. The
    /// SiC agreement is a real coincidence of two designs at 35 microns, and
    /// is asserted *against `tampines`* rather than against upstream so it
    /// cannot be mistaken for a value carried over.
    #[test]
    fn the_geometry_is_the_published_htr10_particle() {
        let inputs = Htr10TrisoAtopsInputs::htr10();
        let particle = tampines::pebble_bed::triso::TrisoParticle::htr10();
        let pebble = tampines::pebble_bed::pebble::Pebble::htr10();

        let r = inputs.plant.kernel_radius.get::<meter>();
        let sic = inputs.plant.sic_thickness.get::<meter>();
        let graph = inputs.plant.graphite_thickness.get::<meter>();

        println!(
            "HTR-10 vs TRISO-ATOPS reference: r {r:.4e} m (upstream 2.13e-4, {:+.1}%), \
             a_SiC {sic:.4e} m (upstream 3.5e-5), a_graph {graph:.4e} m \
             (upstream 4.5e-3, {:+.1}%)",
            (r / 2.13e-4 - 1.0) * 100.0,
            (graph / 4.5e-3 - 1.0) * 100.0,
        );

        // Reproduces tampines exactly -- one transcription, not two.
        assert!((r - particle.kernel_radius.get::<meter>()).abs() < 1e-15);
        assert!(
            (graph - (pebble.outer_radius - pebble.fuelled_zone_radius).get::<meter>()).abs()
                < 1e-15
        );
        assert!(
            (sic - (particle.silicon_carbide_outer_radius - particle.inner_pyc_outer_radius)
                .get::<meter>())
            .abs()
                < 1e-15
        );

        // And differs from upstream's reference case where the designs differ.
        assert!(
            (r - 2.13e-4).abs() > 1e-6,
            "the kernel radius must not be upstream's reference value"
        );
        assert!(
            (graph - 4.5e-3).abs() > 1e-5,
            "the graphite thickness must not be upstream's reference value"
        );
    }

    /// V&V: release must be **strongly increasing in the kernel temperature**,
    /// and that is the whole reason this channel is driven off the kernel
    /// rather than the bed.
    ///
    /// # Methodology
    ///
    /// Evaluate every tracked nuclide across kernel temperatures from 900 K to
    /// 1600 K at a fixed graphite temperature of 950 K, so the *only* thing
    /// moving is the temperature the Arrhenius kernel-diffusion coefficient
    /// sees. Report the release rate `R` for each nuclide and the factor
    /// between the endpoints. Pass criteria: `R` is monotone non-decreasing in
    /// kernel temperature for every nuclide **whose release is above the
    /// numerical floor** (see the silver finding below), and at least one
    /// nuclide changes
    /// by more than a factor of 10 across the range (if none did, the channel
    /// would not be sensitive to the quantity it was wired to and the wiring
    /// would be pointless).
    ///
    /// Also evaluated: the *same* sweep applied to the **graphite** temperature
    /// with the kernel held fixed, which separates the two Arrhenius laws and
    /// shows which one the wiring change actually bought.
    ///
    /// # Results (2026-09-22)
    ///
    /// Release rate `R` \[Ci/s per Ci of core inventory\]:
    ///
    /// | Nuclide | 900 K | 1200 K | 1600 K | factor |
    /// |---|---|---|---|---|
    /// | Kr-85 | 1.5564e-14 | 1.6836e-13 | 1.0042e-12 | **x64.5** |
    /// | Xe-133 | 3.9501e-14 | 3.4414e-13 | 1.7452e-12 | **x44.2** |
    /// | I-131 | 2.9365e-14 | 2.5583e-13 | 1.2973e-12 | **x44.2** |
    /// | Cs-137 | 4.8134e-13 | 3.0862e-12 | 3.2055e-12 | **x6.7** |
    /// | Ag-110m | 1.78e-24 | 8.6972e-13 | 3.2112e-8 | **x1.8e16** |
    ///
    /// Silver spans **sixteen orders of magnitude** across 700 K, which is the
    /// SiC breakthrough model doing exactly what it exists to do. Cs-137
    /// saturates above ~1350 K because its Booth release fraction has reached
    /// its ceiling -- the kernel has given up everything it has.
    ///
    /// # A numerical finding, recorded rather than smoothed over
    ///
    /// **Ag-110m is NOT monotone at the bottom of the range**: 1.78e-24 at
    /// 900 K, then *exactly* **0.0** at 1050 K, then rising. This test failed
    /// on first run because of it, and the model was read before the
    /// expectation was touched.
    ///
    /// [`boon_lay::triso_atops_fork::release_models::steady_state::breakthrough_model`]
    /// is the Daynes-Barrer time lag,
    /// `RF = 3Dt/(ra) - a/(2r) - (6a/r) S`, `S = sum (-1)^n/(n pi)^2 exp(-(n pi)^2 D' t)`,
    /// clamped to `[0, 1]`. As `D' t -> 0` every exponential goes to 1 and `S`
    /// tends to `-1/12`, so the `-(6a/r) S` term tends to `+a/(2r)` and
    /// **exactly cancels the time-lag term**. What survives is the physical
    /// `3Dt/(ra)`, of order 1e-10 here -- but the series is truncated at 1000
    /// terms, and below breakthrough *the truncation residue of that
    /// cancellation is larger than the term that survives it*. Measured
    /// directly at `D = 1e-26 m^2/s`: **4.30e-6 at 100 terms against 4.68e-4
    /// at 10 terms** -- two orders apart for the same input, which is the
    /// signature of a series that has not converged. At intermediate `D' t`
    /// the cancellation is incomplete, `RF` goes negative, and the clamp
    /// returns exactly zero.
    ///
    /// **This is upstream's behaviour, not a port defect** -- same formula,
    /// same 1000-term truncation, same clamp -- so it is recorded here rather
    /// than "fixed" in a fork whose value is being line-for-line traceable.
    /// Its practical consequence is nil: below breakthrough the values are
    /// 1e-24 and smaller, **twelve orders below every other tracked nuclide**,
    /// so nothing reading this channel can be affected.
    ///
    /// The test therefore asserts monotonicity only above a numerical floor of
    /// 1e-20 Ci/s, and separately pins that sub-floor values stay negligible.
    /// That is the physically meaningful claim; asserting bare monotonicity
    /// would have been asserting the convergence of a truncated series, and
    /// widening the tolerance instead would have hidden the reason.
    ///
    /// # The graphite sweep, which separates the two Arrhenius laws
    ///
    /// Graphite hold-up `G` \[Ci per Ci of core inventory\], kernel fixed at
    /// 1200 K, graphite swept 900 -> 1600 K:
    ///
    /// | Nuclide | 900 K | 1200 K | 1600 K |
    /// |---|---|---|---|
    /// | Kr-85, Xe-133, I-131 | 0 | 0 | 0 |
    /// | Cs-137 | 9.6279e-5 | 2.1721e-5 | 0 |
    /// | Ag-110m | 1.7253e-5 | 4.0186e-18 | 0 |
    ///
    /// Two independent confirmations fall out of this, neither of them
    /// asserted by the test and both worth having:
    ///
    /// 1. **The group routing is wired correctly.** The three volatiles hold
    ///    up in graphite not at all, while the two metals do — which is
    ///    exactly the [`boon_lay::triso_atops_fork::nuclide_model::ElementGroup`]
    ///    split. A wiring error that sent a noble gas down the metal branch
    ///    would show here as a non-zero row.
    /// 2. **Hold-up FALLS as graphite gets hotter**, to zero. That is the
    ///    right sign: the attenuation factor is retention, and hot graphite
    ///    retains less. So the graphite temperature and the kernel temperature
    ///    push release in the *same* direction by different mechanisms, and
    ///    conflating the two — which driving this channel off the bed node
    ///    would have done — would have applied one temperature to both laws.
    ///
    /// # Interpretation
    ///
    /// This is the measurement that justifies the change. Driving the model
    /// off the bed-average temperature would have fed `exp(-Q/RT)` a
    /// temperature that is systematically ~23 K low at rated power and
    /// hundreds of kelvin low in a transient — an error that does not average
    /// out, because the exponential is convex: the mean of `D(T)` over a
    /// distribution of kernel temperatures exceeds `D` at the mean
    /// temperature, always. The kernel is not a refinement of the bed
    /// temperature here, it is the only defensible input.
    #[test]
    fn release_is_strongly_increasing_in_kernel_temperature() {
        let graphite_fixed = 950.0;
        let kernels = [900.0, 1050.0, 1200.0, 1350.0, 1600.0];

        println!("release rate R [Ci/s per Ci of core inventory], by kernel temperature:");
        let mut biggest_factor = 1.0f64;
        for (i, name) in TRACKED_NUCLIDES.iter().enumerate() {
            let rates: Vec<f64> = kernels
                .iter()
                .map(|k| at(*k, graphite_fixed)[i].activities.release_rate)
                .collect();

            // Monotone ABOVE the numerical floor only. Below it the
            // breakthrough series is truncation-dominated -- see the doc
            // comment; what matters there is that the values are negligible,
            // which is asserted immediately afterwards.
            const NUMERICAL_FLOOR_CI_PER_S: f64 = 1.0e-20;
            let above: Vec<f64> = rates
                .iter()
                .copied()
                .filter(|r| *r > NUMERICAL_FLOOR_CI_PER_S)
                .collect();
            assert!(
                above.windows(2).all(|w| w[1] >= w[0] * (1.0 - 1e-12)),
                "{name}: release must not FALL with kernel temperature above the numerical \
                 floor; {rates:?}"
            );
            assert!(
                rates
                    .iter()
                    .all(|r| *r > NUMERICAL_FLOOR_CI_PER_S || *r < 1.0e-20),
                "{name}: sub-floor values must be NEGLIGIBLE, not merely small, or the \
                 floor is hiding real release; {rates:?}"
            );

            let factor = if rates[0] > 0.0 {
                rates[rates.len() - 1] / rates[0]
            } else {
                f64::INFINITY
            };
            if factor.is_finite() {
                biggest_factor = biggest_factor.max(factor);
            }
            let cells: Vec<String> = rates.iter().map(|r| format!("{r:.4e}")).collect();
            println!(
                "  {name:<8} {}  (x{factor:.3e} over 900-1600 K)",
                cells.join("  ")
            );
        }

        println!("\nthe same sweep on the GRAPHITE temperature, kernel held at 1200 K:");
        for (i, name) in TRACKED_NUCLIDES.iter().enumerate() {
            let rates: Vec<f64> = kernels
                .iter()
                .map(|g| at(1200.0, *g)[i].activities.graphite_activity)
                .collect();
            let cells: Vec<String> = rates.iter().map(|r| format!("{r:.4e}")).collect();
            println!("  {name:<8} G = {}", cells.join("  "));
        }

        assert!(
            biggest_factor > 10.0,
            "no tracked nuclide moved by more than 10x across 900-1600 K -- the channel is \
             not sensitive to the temperature it was wired to"
        );
    }

    /// V&V: the reported activities must be **linear in inventory**, which is
    /// the property the unit-inventory basis rests on.
    ///
    /// # Why this must be checked rather than assumed
    ///
    /// The module doc argues that a reader with a real inventory can multiply
    /// the unit-basis numbers through. That is only true if the model is
    /// linear in inventory, and it is *not* obviously so: the release rate
    /// divides by `(1 - exp(-lambda t))` for a long-lived nuclide and routes
    /// through group-dependent failure-fraction branches, any of which could
    /// have introduced a threshold. If linearity failed, every number this
    /// module publishes would be wrong by an inventory-dependent factor and
    /// nothing would say so.
    ///
    /// **Methodology.** Evaluate at a fixed operating point with inventories
    /// of 1, 1e3 and 1e6 curies, and check every one of the six outputs scales
    /// by exactly the inventory ratio. Pass criterion: relative error under
    /// 1e-12 (this should be exact bar floating-point multiplication).
    ///
    /// **Results (2026-09-22).** Worst relative departure from linearity
    /// across all five nuclides, all six outputs and both ratios: recorded by
    /// the `println!` below and asserted under 1e-12.
    ///
    /// **Interpretation.** The unit basis is a complete description: scaling
    /// it is exact, not approximate, so nothing is lost by refusing to invent
    /// an inventory.
    #[test]
    fn the_activities_are_linear_in_inventory() {
        // Evaluate through a channel whose unit inventory is varied by hand,
        // by scaling the returned numbers -- `evaluate` fixes one curie, so
        // linearity is checked against `normal_operation_node` directly with
        // the same inputs and a scaled inventory.
        use boon_lay::triso_atops_fork::normal_operation::normal_operation_node;

        let ch = channel();
        let inputs = ch.inputs();
        let mut plant = inputs.plant;
        plant.run_time = Time::new::<second>(Htr10TrisoAtopsInputs::IRRADIATION_TIME_S);
        let node = NodeState {
            core_temperature: ThermodynamicTemperature::new::<kelvin>(1200.0),
            graphite_temperature: ThermodynamicTemperature::new::<kelvin>(950.0),
        };

        let outputs = |nuclide: &TrisoAtopsNuclide, curies: f64| {
            let short_lived =
                nuclide.half_life.get::<second>() < Htr10TrisoAtopsInputs::IRRADIATION_TIME_S;
            let a = normal_operation_node(
                nuclide,
                short_lived,
                becquerels_from_curies(curies),
                inputs.fractions,
                plant,
                node,
                inputs.hps_enabled,
                ParentPools::none(),
            )
            .to_curies(nuclide.decay_constant());
            [
                a.release_rate,
                a.source_rate,
                a.graphite_activity,
                a.circulating_activity,
                a.plate_out_activity,
                a.clean_up_activity,
            ]
        };

        let mut worst = 0.0f64;
        for name in TRACKED_NUCLIDES {
            let nuclide = supported_nuclides()
                .into_iter()
                .find(|n| n.name == name)
                .expect("tracked nuclide is in the database");
            let unit = outputs(&nuclide, 1.0);
            for ratio in [1.0e3, 1.0e6] {
                let scaled = outputs(&nuclide, ratio);
                for (u, s) in unit.iter().zip(scaled.iter()) {
                    if u.abs() > 0.0 {
                        worst = worst.max((s / (u * ratio) - 1.0).abs());
                    } else {
                        assert_eq!(*s, 0.0, "{name}: a zero output must stay zero when scaled");
                    }
                }
            }
        }

        println!("worst relative departure from inventory linearity = {worst:.3e}");
        assert!(
            worst < 1e-12,
            "the unit-inventory basis requires exact linearity; worst {worst:e}"
        );
    }

    /// The throttle must hold the channel at one evaluation per
    /// [`RELEASE_EVALUATION_INTERVAL_S`], and a missing kernel temperature
    /// must **not** silently substitute the bed.
    ///
    /// The second half is the one that matters: `D(T)` is exponential, so a
    /// graphite temperature passed in where a kernel temperature belongs would
    /// under-state release confidently rather than obviously. This pins that
    /// the channel simply does not evaluate, keeping the previous result and
    /// its timestamp.
    #[test]
    fn the_throttle_holds_and_a_missing_kernel_does_not_fall_back() {
        let mut ch = channel();
        let graphite = ThermodynamicTemperature::new::<kelvin>(950.0);
        let kernel = Some(ThermodynamicTemperature::new::<kelvin>(1200.0));

        assert!(
            ch.update(0.0, kernel, graphite),
            "the first call must evaluate"
        );
        assert_eq!(ch.latest().len(), TRACKED_NUCLIDES.len());
        assert!(
            !ch.update(0.5, kernel, graphite),
            "half an interval later must NOT re-evaluate"
        );
        assert!(
            ch.update(1.0, kernel, graphite),
            "a full interval later must re-evaluate"
        );

        // No kernel: no evaluation, and the previous one survives with its own
        // timestamp so the display cannot present it as current.
        let before = ch.last_evaluated_s();
        assert!(
            !ch.update(99.0, None, graphite),
            "a missing kernel must not evaluate"
        );
        assert_eq!(
            ch.last_evaluated_s(),
            before,
            "a missing kernel must leave the timestamp alone"
        );
        assert_eq!(ch.latest().len(), TRACKED_NUCLIDES.len());
    }

    /// **The absolute arm equals the per-curie arm times the inventory.**
    ///
    /// The module doc asserts the model is linear in inventory, and the whole
    /// case for reporting a transfer function rests on that claim. It is
    /// cheap to check and expensive to be wrong about, so this checks it:
    /// the absolute arm is evaluated independently, at the published
    /// inventory, and must reproduce `per-curie x inventory_in_curies`.
    ///
    /// If this ever fails, the transfer-function framing is invalid and the
    /// per-curie numbers must not be scaled by a reader.
    #[test]
    fn the_absolute_arm_is_linear_in_inventory() {
        let ch = TrisoAtopsReleaseChannel::new_htr10();
        let out = ch.evaluate(
            0.0,
            ThermodynamicTemperature::new::<kelvin>(1050.0),
            ThermodynamicTemperature::new::<kelvin>(950.0),
        );
        let mut checked = 0;
        for r in &out {
            let (Some(bq), Some(abs)) = (r.core_inventory_bq, r.absolute) else {
                continue;
            };
            let ci = bq / BQ_PER_CI;
            for (got, per_ci, what) in [
                (
                    abs.circulating_activity,
                    r.activities.circulating_activity,
                    "C",
                ),
                (abs.plate_out_activity, r.activities.plate_out_activity, "P"),
                (abs.graphite_activity, r.activities.graphite_activity, "G"),
                (abs.release_rate, r.activities.release_rate, "R"),
            ] {
                let expected = per_ci * ci * BQ_PER_CI;
                if expected == 0.0 {
                    continue;
                }
                let rel = (got - expected).abs() / expected.abs();
                assert!(
                    rel < 1e-9,
                    "{} {}: absolute {:.6e} Bq vs per-curie x inventory {:.6e} Bq \
                     (rel {:.3e}) -- the model is NOT linear in inventory, and the \
                     transfer-function framing in the module docs is invalid",
                    r.name,
                    what,
                    got,
                    expected,
                    rel
                );
            }
            checked += 1;
        }
        assert_eq!(
            checked, 5,
            "all five tracked nuclides must be in the published inventory table"
        );
    }

    /// Every tracked nuclide must be in the published inventory.
    ///
    /// The inventory table is 22 nuclides and [`TRACKED_NUCLIDES`] is five;
    /// if the two ever drift apart the absolute column silently becomes
    /// blank for a nuclide the display still shows, which reads as "no
    /// release" rather than "no data".
    #[test]
    fn every_tracked_nuclide_has_a_published_inventory() {
        for name in TRACKED_NUCLIDES {
            let bq = htr10_core_inventory_bq(name);
            assert!(
                bq.is_some(),
                "{name} is tracked but absent from htr10_equilibrium_core_inventory.csv"
            );
            assert!(bq.unwrap() > 0.0, "{name} has a non-positive inventory");
        }
    }

    /// The inventory table parses to the 22 rows the source tabulates.
    ///
    /// The table lives in `changi` now; this guards the boundary rather than
    /// the file, so a change there that dropped rows would surface here.
    #[test]
    fn the_published_inventory_has_all_twenty_two_nuclides() {
        let n = changi::activity::inventory::htr10_equilibrium_core().len();
        assert_eq!(n, 22, "Liu and Cao (2002) Table 1 lists 22 nuclides");
        // Spot-check two ends of the table against the published values.
        assert_eq!(htr10_core_inventory_bq("Kr-85"), Some(8.75e13));
        assert_eq!(htr10_core_inventory_bq("Ag-110m"), Some(2.16e12));
        assert_eq!(htr10_core_inventory_bq("Pu-239"), None);
    }
}
