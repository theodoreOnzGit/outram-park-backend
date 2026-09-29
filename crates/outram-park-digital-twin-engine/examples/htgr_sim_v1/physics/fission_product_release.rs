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
//! `KernelDopplerChannel` (removed 2026-09-28, gh:#360) was rewired onto in this change,
//! and it is not a coincidence that the two arrived together: resolving the
//! pebble produced a kernel temperature, and a kernel temperature is exactly
//! what a Doppler coefficient and a release model both want and neither could
//! previously have.
//!
//! **CORRECTED 2026-09-28 (gh:#360) -- "the same temperature" holds for the
//! kernel-above-node offset only.** This channel receives
//! `PebbleBedPorousMediaNode::peak_kernel_temperature` = bed node + offset
//! (hottest kernel of a core-average pebble, fluence 0, start-of-step bed),
//! with the bed volume average as the graphite temperature. The Doppler
//! channel adds the offset to the kinetics node `T_f` instead, which at power
//! was measured 235 K above the bed (an energy-accounting defect in that node,
//! gh:#360). The two absolute kernel temperatures therefore disagree.
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
//! ~~**Every activity reported here is per curie of that nuclide's core
//! inventory.**~~ **SUPERSEDED 2026-09-23 and 2026-09-29** -- see "BOTH bases"
//! and "The live primary pools" below: the published Table 1 inventory drives
//! an absolute arm, which is the simulator's primary basis since gh:#399.
//! What follows remains true of the reasoning: nothing in this module
//! *derives* an inventory from yields, and that is a considered refusal
//! rather than an omission.
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
//! any other reactor** — including the absolute becquerel column added below
//! (~~which carries a fuel-quality input that is not HTR-10's~~; its inputs are
//! HTR-10's since 2026-09-29, and it is still indicative only).
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
//! ~~**The deeper objection stands, and is the reason the absolute column is
//! not a source term.** The failure fractions below are **TRISO-ATOPS
//! reference values, not HTR-10 fuel-qualification data** ...~~
//! **CHANGED 2026-09-29 (gh:#399, source-term stage 1):** the absolute arm
//! is now HTR-10's end to end -- Table 1 inventory, HTR-10's measured free
//! uranium (Tang et al. 2002), a live in-service failure from boon-lay fuel
//! failure at the plant's kernel temperature, HTR-10's fuel residence, and
//! HTR-10's primary-circuit constants (Liu & Cao 2002, Yao et al. 2002) in
//! **live** pools stepped through the transient. It is the simulator's
//! primary basis now; the per-curie arm is kept as the transfer function
//! (pool / inventory). Its uncalibrated comparison with Liu & Cao's Tables 2
//! and 3 is `tests::the_release_is_compared_uncalibrated_with_liu_cao_tables_2_and_3`.
//! It is still **indicative, research/education only** (`RESPONSIBLE_USE.md`),
//! never a licensing source term.
//!
//! # The live primary pools (gh:#399)
//!
//! ~~The loop pools are TRISO-ATOPS's closed forms at `run_time = 1 year +
//! sim_t` -- effectively saturated~~. They are now **carried**: each tracked
//! nuclide's circulating, plate-out and clean-up pools and its cumulative
//! leak are stepped exactly ([`boon_lay::triso_atops_fork::activities::live_pools`])
//! over the time since the last evaluation, with the fuel-side source at the
//! current kernel temperature held over the step, and opened at the exact
//! 20-full-power-year history ([`POOL_OPENING_HISTORY_S`], Liu & Cao's
//! Table 3 basis). Rate constants: [`htr10_pool_rates`]. The leak
//! ([`NodalActivitiesBq::leak_rate`]) is what leaves the circuit.
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

use boon_lay::triso_atops_fork::activities::live_pools::{self, PoolRates, PrimaryPools};
use boon_lay::triso_atops_fork::activities::{becquerels_from_curies, FailureFractions};
use boon_lay::triso_atops_fork::normal_operation::{
    normal_operation_node, NodalActivitiesCurie, NodeState, ParentPools, PlantConstants,
};
use boon_lay::triso_atops_fork::nuclide_model::nuclide_database::supported_nuclides;
use boon_lay::triso_atops_fork::nuclide_model::{ElementGroup, TrisoAtopsNuclide};

use super::pebble_bed::FuelStackTemperatures;

use uom::si::f64::{Frequency, Length, MassRate, ThermodynamicTemperature, Time};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::frequency::hertz;
use uom::si::length::meter;
use uom::si::time::second;

/// The unit-inventory basis every activity in this module is reported
/// against: **one curie** of the nuclide in the core. See the module doc for
/// why the basis is a unit rather than a real inventory.
pub const UNIT_INVENTORY_CURIES: f64 = 1.0;

/// Becquerels per curie -- the tests' conversion (the channel converts
/// through boon-lay's `becquerels_from_curies` and the decay constant).
#[cfg(test)]
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
    /// Primary-circuit leak rate `k_leak C` \[Bq/s\] (gh:#399) -- what leaves
    /// the circuit into the reactor building.
    pub leak_rate: f64,
    /// Release rate **up the stack** \[Bq/s\] (gh:#400): what leaves the
    /// reactor building (`bishan::building`, HTR-10 vented confinement) at the
    /// end of the latest step. This, not the circuit leak, is the source the
    /// atmospheric dispersion receives.
    pub stack_release_rate: f64,
    /// Activity airborne in the reactor building \[Bq\] (gh:#400).
    pub building_activity: f64,
}

// ---------------------------------------------------------------------------
// HTR-10 PRIMARY-CIRCUIT POOL CONSTANTS -- PUBLISHED (gh:#399, 2026-09-29)
//
// Liu Yuanzhong & Cao Jianzhu (2002), Nucl. Eng. Des. 218, 81-90, section
// 2.4.1 (proprietary; cited, not redistributed), give the removal parameters
// they used for HTR-10's primary helium; Yao et al. (2002), Nucl. Eng. Des.
// 218, 163-167, give the helium purification throughput. These replace
// TRISO-ATOPS's reference `k_plate` and `k_clean`, which were for a different
// plant (and whose provenance was mis-stated; see the struck-through note at
// the top of `impl Htr10TrisoAtopsInputs`).
// ---------------------------------------------------------------------------

/// HTR-10 helium purification throughput as a fraction of the primary helium
/// inventory per hour: **"about 5 %"** (10.5 kg/h; Yao et al. 2002, section 1
/// and abstract, "corresponding with a 5% gas change of the helium inventory
/// in primary circuit").
pub const HTR10_PURIFICATION_INVENTORY_FRACTION_PER_H: f64 = 0.05;

/// HTR-10 helium purification flow \[kg/h\]: **10.5** (Yao et al. 2002).
pub const HTR10_PURIFICATION_FLOW_KG_PER_H: f64 = 10.5;

/// HTR-10 primary helium inventory \[kg\], **derived** from Yao's two figures:
/// `10.5 kg/h / (0.05 /h)` = **210 kg**. Sets the loop's cycle time
/// `M / m_dot`, which turns Liu & Cao's per-cycle deposition into a rate
/// constant, **and** sizes the primary loop's cold-return CV
/// (`primary_loop::cold_return_volume`, gh:#403), so the thermal-hydraulic
/// loop and the source term hold one inventory. ~~(It is about ten times the
/// stage (a) primary-loop model's own inventory, which rests on an invented
/// 6 m^3 allowance.)~~ Corrected 2026-09-29 (gh:#403): the loop is sized from
/// this figure now.
pub fn htr10_primary_helium_inventory_kg() -> f64 {
    HTR10_PURIFICATION_FLOW_KG_PER_H / HTR10_PURIFICATION_INVENTORY_FRACTION_PER_H
}

/// Primary helium leakage: **1 % of the total volume per day** (Liu & Cao
/// 2002 section 2.4.1; "specified to be below 1 % per day" in section 2.4.2)
/// \[1/s\]. The same constant the dispersion channel's leak uses.
pub const HTR10_PRIMARY_LEAK_PER_S: f64 = 0.01 / 86_400.0;

/// Liu & Cao's purification **efficiency** by element: **99 %** for I, Kr, Xe
/// (and C, tritium), **90 %** for Sr, Ag, Cs, Rb (section 2.4.1,
/// "conservatively set"). `None` for an element they do not name.
pub fn htr10_purification_efficiency(z: u32) -> Option<f64> {
    match z {
        53 | 36 | 54 | 6 | 1 => Some(0.99),
        38 | 47 | 55 | 37 => Some(0.90),
        _ => None,
    }
}

/// Liu & Cao's deposition **per cycle** of the primary circuit by element:
/// **30 %** for Rb and Sr, **50 %** for Ag and Cs, **20 %** for iodine
/// ("conservatively taken", section 2.4.1, against AVR's 50-90 %); noble gases
/// do not deposit. `None` for an element they do not name.
pub fn htr10_deposition_per_cycle(z: u32) -> Option<f64> {
    match z {
        37 | 38 => Some(0.30),
        47 | 55 => Some(0.50),
        53 => Some(0.20),
        36 | 54 => Some(0.0),
        _ => None,
    }
}

/// The HTR-10 pool rate constants \[1/s\] for a nuclide of element `z` and
/// decay constant `decay_per_s`, at loop flow `mass_flow`:
///
/// - clean-up `k_clean = eta x 0.05 /h` (Yao's throughput x Liu & Cao's
///   efficiency);
/// - plate-out `k_plate = -ln(1 - d) m_dot / M` -- a per-cycle deposition `d`
///   over a cycle of `M / m_dot` (Liu & Cao's `d`, Yao-derived `M`), so it
///   scales with the flow and vanishes with it;
/// - leak `k_leak` = 1 %/day.
///
/// # Panics
///
/// For an element Liu & Cao give no efficiency or deposition for -- a
/// tracked nuclide outside their list is a wiring error, not a zero.
pub fn htr10_pool_rates(z: u32, decay_per_s: f64, mass_flow: MassRate) -> PoolRates {
    let eta = htr10_purification_efficiency(z)
        .unwrap_or_else(|| panic!("no HTR-10 purification efficiency for Z = {z}"));
    let d = htr10_deposition_per_cycle(z)
        .unwrap_or_else(|| panic!("no HTR-10 deposition per cycle for Z = {z}"));
    let cycles_per_s =
        mass_flow.get::<kilogram_per_second>().abs() / htr10_primary_helium_inventory_kg();
    PoolRates {
        decay: decay_per_s,
        plate_out: -(1.0 - d).ln() * cycles_per_s,
        clean_up: eta * HTR10_PURIFICATION_INVENTORY_FRACTION_PER_H / 3600.0,
        leak: HTR10_PRIMARY_LEAK_PER_S,
    }
}

/// How long the pools have been accumulating when the plant opens \[s\]:
/// **20 full-power years**, the basis of Liu & Cao's primary-helium activities
/// (Table 3, "at the end of 20a lifetime of full power operation"). The pools
/// are opened at the exact solution for that history at the opening source,
/// so the simulator starts from an operating circuit and the comparison with
/// Table 3 is like-for-like.
pub const POOL_OPENING_HISTORY_S: f64 = 20.0 * 3.155_76e7;

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
/// The **fuel side** is **quasi-steady**: `normal_operation_node` evaluates
/// closed-form diffusion at the temperature it is handed, and carries no state
/// of its own between calls. (The primary pools downstream of it are live
/// since 2026-09-29 and are stepped **exactly** over whatever interval
/// elapsed, so the throttle does not degrade them.) So unlike the kinetics, the bed or the steam
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
    // ~~`fractions: FailureFractions`~~ -- removed 2026-09-29: the four
    // defect populations are now computed per evaluation at the live kernel
    // temperature ([`Self::fractions_at`]), not stored.
    /// The **closed-form** pools' HPS switch in `normal_operation_node`.
    /// ~~`true` here: HTR-10 has one~~ **CHANGED 2026-09-29 (gh:#399)**:
    /// `false`, because the closed-form pools are no longer used -- HTR-10's
    /// helium purification acts in the live pools ([`htr10_pool_rates`]),
    /// on every element Liu & Cao give an efficiency for, not only the
    /// volatiles TRISO-ATOPS's closed form scrubs.
    pub hps_enabled: bool,
}

impl Htr10TrisoAtopsInputs {
    // ~~`TRISO_ATOPS_PLATE_OUT_PER_S = 7.5e-4` and `TRISO_ATOPS_CLEAN_UP_PER_S
    // = 8.77e-5` -- "upstream's own constants block"~~ -- DELETED 2026-09-29
    // (gh:#399). The provenance was wrong: upstream TRISO-ATOPS's code has
    // `k_plate` 7.5e-4 and `k_clean` 1e-4 as GUI defaults
    // (`trisoatops_gui.py:300, :382`); 8.77e-5 is from the TRISO-ATOPS paper
    // (Stoyer, Raichart & Petti, Nucl. Technol. 2026, Table 3: "35% of
    // inventory per hour with 90% efficiency"), which also gives k_plate =
    // 7.5e-5, ten times below the GUI's. Neither is HTR-10's. The pools now
    // carry HTR-10's own published constants: see [`htr10_pool_rates`].

    /// Kernel grain size `a_grain` \[m\] — **a TRISO-ATOPS reference value,
    /// not HTR-10 data.**
    ///
    /// `1e-5 m`, upstream's GUI default (`trisoatops_gui.py:299`) and the
    /// TRISO-ATOPS paper's Table 3. It enters only the special-metal
    /// equivalent-sphere radius `a_booth = sqrt(2 a_grain r)`, so of the
    /// tracked nuclides it reaches Cs-137 alone. No grain size for HTR-10's
    /// UO2 kernels is available in this workspace; upstream's value is for UCO
    /// fuel and is carried unchanged rather than adjusted towards a number
    /// nobody has published.
    pub const TRISO_ATOPS_GRAIN_SIZE_M: f64 = 1.0e-5;

    /// HTR-10 first-loading **free uranium fraction**, `U_free / U_total` =
    /// **5.0e-5**: the average over 25 spherical-fuel-element lots, burn-leach
    /// method -- Tang et al. (2002), *Nucl. Eng. Des.* 218, 91-102, abstract,
    /// Table 2 and section 4.2 (proprietary; cited, not redistributed). F1-F10
    /// averaged 1.1e-4, F11-F25 1.4e-5; the specification limit is 3e-4.
    ///
    /// **What it measures.** Burn-leach dissolves every uranium atom not
    /// protected by an intact SiC layer once the carbon (including the PyC) is
    /// burned off, so it is "the uncoated uranium in the particles with
    /// defective SiC layer and the contaminated uranium" together (Tang et
    /// al., section 4.2) -- TRISO-ATOPS's `f_hm + f_sic`. The split between
    /// the two is **not published**. It matters only for the noble gases and
    /// halogens, which a defective-SiC particle's intact PyC retains
    /// (TRISO-ATOPS's `release_rate` counts `f_hm` and not `f_sic` for them).
    /// This channel takes the **bounding** assignment -- all of it as exposed
    /// heavy metal (`f_hm`) -- which over-states the gas and iodine release by
    /// at most the (unknown) defective-SiC share; the other bound (all `f_sic`)
    /// is reported by `tests::the_free_uranium_split_bounds_the_gas_release`.
    pub const HTR10_FREE_URANIUM_FRACTION: f64 = 5.0e-5;

    /// ~~`TRISO_ATOPS_REFERENCE_FAILURE_FRACTIONS` -- `f_hm = 1e-5, f_sic =
    /// 2e-5, f_inc = 3e-5, f_inc_sic = 4e-5`, "from upstream's own constants
    /// block"~~ -- **DELETED 2026-09-29 (gh:#399)**, and its provenance was
    /// false: those four numbers are nowhere in upstream TRISO-ATOPS (whose
    /// GUI defaults are 1e-4, 1e-4, 2.3e-5, 3.6e-5, `trisoatops_gui.py:231-234`)
    /// nor in the TRISO-ATOPS paper (2e-5, 1e-4, 2.3e-5, 3.6e-5, Table 2). They
    /// are the synthetic inputs of boon-lay's own reference generator
    /// (`crates/boon-lay/dev/gen_triso_atops_reference.py:802`). Replaced by
    /// HTR-10's measured free uranium ([`Self::HTR10_FREE_URANIUM_FRACTION`])
    /// and a live in-service failure fraction ([`Self::fractions_at`]).
    ///
    /// The failure fractions at kernel temperature `kernel` with the
    /// chemical-attack failure `chemical_attack` added:
    ///
    /// | TRISO-ATOPS | Value | Source |
    /// |---|---|---|
    /// | `f_hm` | 5.0e-5 | [`Self::HTR10_FREE_URANIUM_FRACTION`] (bounding split) |
    /// | `f_sic` | 0 | (the other bound; see above) |
    /// | `f_inc` | `phi_1(T_B) + chemical_attack` | boon-lay fuel failure, end of irradiation, `T_B = T_kernel - 75 K` |
    /// | `f_inc_sic` | 0 | SiC decomposition is negligible below ~2000 degC |
    ///
    /// `phi_1` is **boon-lay fuel failure** (boon-lay's implementation of the
    /// PANAMA-I pressure-vessel formulas -- not the PANAMA code), at the end of
    /// irradiation for an HTR-10 particle irradiated at `T_B`
    /// ([`boon_lay::fuel_failure::htr10::end_of_irradiation_failure`]); `T_B`
    /// is the report's irradiation (surface) temperature, the kernel less the
    /// report's own 75 K kernel-versus-surface correction (Eq. (5c); see that
    /// function's doc). It is an extrapolation with stand-in strength data,
    /// quasi-steady (an equilibrium core irradiated at the current
    /// temperature), and negligible at design temperatures -- 1.2e-12 at 776
    /// degC -- but it is the in-service failure the plant temperature drives,
    /// so it is live. ~~`2.4e-15` at 700 degC, `9.3e-13` at 776 degC and
    /// `5.9e-7` at 1000 degC~~ **CORRECTED 2026-09-29**: the current
    /// implementation gives 2.8e-15, 1.2e-12 and 1.6e-6 (pinned by
    /// `boon_lay::fuel_failure::htr10::tests::normal_operation_failure_is_negligible_against_the_placeholder`).
    ///
    /// `chemical_attack` is the **hook** for stages 3-4 (steam hydrolysis of
    /// exposed kernels, oxidation-driven failure): the channel passes zero
    /// until an ingress model supplies it (gh:#401, #402).
    pub fn fractions_at(
        kernel: ThermodynamicTemperature,
        chemical_attack: f64,
    ) -> FailureFractions {
        use uom::si::ratio::ratio;
        use uom::si::thermodynamic_temperature::kelvin;
        let t_b = ThermodynamicTemperature::new::<kelvin>(kernel.get::<kelvin>() - 75.0);
        let phi_1 = boon_lay::fuel_failure::htr10::end_of_irradiation_failure(t_b).get::<ratio>();
        FailureFractions {
            heavy_metal: Self::HTR10_FREE_URANIUM_FRACTION,
            sic: 0.0,
            incremental: phi_1 + chemical_attack,
            incremental_sic: 0.0,
        }
    }

    /// Irradiation time \[s\]: HTR-10's mean fuel residence at full power,
    /// **1080 FPD** ([`boon_lay::fuel_failure::htr10::RESIDENCE_FULL_POWER_DAYS`],
    /// derived there from the published 80 000 MWd/t, 27 000 x 5 g and
    /// 10 MW). ~~One year, upstream's reference `t_irad`~~ until 2026-09-29.
    ///
    /// Sets how long the fuel has been accumulating and releasing, and appears
    /// in the long-lived Booth release fraction and in the birth-rate
    /// normalisation. This simulator has **no burnup**, so the fuel neither
    /// ages nor changes composition as it runs; a fixed irradiation time is
    /// the consistent choice.
    pub const IRRADIATION_TIME_S: f64 =
        boon_lay::fuel_failure::htr10::RESIDENCE_FULL_POWER_DAYS * 86_400.0;

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
                // The closed-form pools `normal_operation_node` also returns
                // are NOT used: the live pools carry HTR-10's constants (see
                // `htr10_pool_rates`). Zero here makes that visible -- the
                // release rate, source rate and graphite hold-up this channel
                // does use do not depend on them.
                k_plate: Frequency::new::<hertz>(0.0),
                k_clean: Frequency::new::<hertz>(0.0),
                graphite_thickness: unfuelled_shell,
                grain_size: Length::new::<meter>(Self::TRISO_ATOPS_GRAIN_SIZE_M),
                sic_thickness: particle.silicon_carbide_outer_radius
                    - particle.inner_pyc_outer_radius,
                kernel_radius: particle.kernel_radius,
                // Overwritten per evaluation from the plant clock.
                run_time: Time::new::<second>(0.0),
                irradiation_time: Time::new::<second>(Self::IRRADIATION_TIME_S),
            },
            // The CLOSED-FORM pools' HPS switch. Off, because those pools are
            // not used (and with the zeroed k_plate/k_clean above the ported
            // `clean_up` is 0/0 when it is on). HTR-10's helium purification
            // is in the LIVE pools, at Yao/Liu & Cao's rate: see
            // `htr10_pool_rates`.
            hps_enabled: false,
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
    /// ~~**NOT A SOURCE TERM.** See the module docs: these are one reactor's
    /// inventory driven through another reactor's fuel-quality data.~~
    /// **CHANGED 2026-09-29 (gh:#399):** HTR-10's own inventory, HTR-10's own
    /// measured free uranium and HTR-10's own circuit constants now -- but
    /// still an indicative, research/education figure (`RESPONSIBLE_USE.md`),
    /// not a licensing source term. The three pools are the **live** pools.
    pub absolute: Option<NodalActivitiesBq>,
    /// Coolant source rate `S` \[atoms/s\] -- the live pools' driver.
    pub source_atoms_per_s: f64,
    /// The HTR-10 pool rate constants this nuclide was stepped with.
    pub pool_rates: PoolRates,
    /// The pools after [`POOL_OPENING_HISTORY_S`] at this source, from empty
    /// -- what the channel opens with, and what a pure evaluation reports.
    pub opening_pools: PrimaryPools,
}

impl NuclideRelease {
    /// Report the reactor-building inventory `b` \[atoms\] as activity and
    /// its stack release rate (exhaust x (1 - filter capture)).
    fn set_building(
        &mut self,
        b: bishan::building::BuildingInventory,
        p: bishan::building::BuildingParameters,
    ) {
        let lam = self.decay_constant.get::<hertz>();
        if let Some(a) = self.absolute.as_mut() {
            a.building_activity = b.airborne * lam;
            a.stack_release_rate =
                (1.0 - p.filter_capture) * p.exhaust_turnover.get::<hertz>() * b.airborne * lam;
        }
    }

    /// Report `pools` (atoms) as this nuclide's circulating, plate-out and
    /// clean-up activities, on both bases, and the leak rate.
    fn set_pools(&mut self, pools: PrimaryPools) {
        let lam = self.decay_constant.get::<hertz>();
        if let Some(a) = self.absolute.as_mut() {
            a.circulating_activity = pools.circulating * lam;
            a.plate_out_activity = pools.plate_out * lam;
            a.clean_up_activity = pools.clean_up * lam;
            a.leak_rate = self.pool_rates.leak * pools.circulating * lam;
        }
        // Per curie of core inventory: pool activity over inventory activity.
        if let Some(inventory) = self.core_inventory_bq {
            self.activities.circulating_activity = pools.circulating * lam / inventory;
            self.activities.plate_out_activity = pools.plate_out * lam / inventory;
            self.activities.clean_up_activity = pools.clean_up * lam / inventory;
        }
    }
}

/// The TRISO fission-product release channel for the HTGR plant.
///
/// Holds the fixed inputs, the most recent evaluation and -- ~~no integrated
/// state~~ since 2026-09-29 (gh:#399) -- the **live primary pools**, which
/// are integrated state. The plant steps this channel once per plant step,
/// after its outer-corrector loop, so it is never rewound.
#[derive(Debug, Clone)]
pub struct TrisoAtopsReleaseChannel {
    inputs: Htr10TrisoAtopsInputs,
    nuclides: Vec<TrisoAtopsNuclide>,
    latest: Vec<NuclideRelease>,
    /// The kernel temperature the latest evaluation was taken at, for display
    /// and so a reader can see which temperature produced these numbers.
    evaluated_at_kernel: Option<ThermodynamicTemperature>,
    /// The whole fuel stack of the most recent evaluation.
    evaluated_at: Option<FuelStackTemperatures>,
    /// Plant time of the most recent evaluation \[s\], for the throttle.
    last_evaluated_s: Option<f64>,
    /// The **live** primary-circuit pools, one per tracked nuclide, in atoms
    /// (gh:#399). `None` until the first evaluation opens them at the
    /// [`POOL_OPENING_HISTORY_S`] state.
    pools: Option<Vec<PrimaryPools>>,
    /// The **reactor building** inventory per tracked nuclide \[atoms\]
    /// (gh:#400), fed by the pools' leak. `None` until the first evaluation
    /// opens it at the building's steady state for the opening leak.
    building: Option<Vec<bishan::building::BuildingInventory>>,
    /// Primary loop mass flow the plate-out rate is formed at. Set by the
    /// plant every step ([`Self::set_primary_flow`]); the rated 4.3 kg/s until
    /// then.
    primary_flow: MassRate,
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
            evaluated_at: None,
            last_evaluated_s: None,
            pools: None,
            building: None,
            primary_flow: super::pebble_bed::nominal_helium_flow(),
        }
    }

    /// Set the primary loop mass flow the live plate-out rate is formed at
    /// (plate-out is per cycle of the loop, so it scales with the flow).
    pub fn set_primary_flow(&mut self, mass_flow: MassRate) {
        self.primary_flow = mass_flow;
    }

    /// Re-evaluate the release channel if the throttle allows, at the fuel
    /// stack temperatures the core currently reports.
    ///
    /// ~~`kernel_temperature` is `PebbleBedPorousMediaNode::peak_kernel_temperature`~~
    /// **CHANGED 2026-09-28 (gh:#360, maintainer direction).** The channel is
    /// handed the whole fuel stack ([`FuelStackTemperatures`]) and uses the
    /// physically matching temperature for each term:
    ///
    /// | TRISO-ATOPS input | Before | Now |
    /// |---|---|---|
    /// | kernel diffusion (`core_temp`) | peak kernel centre of a core-average pebble (bed + offset, start-of-step) | **inventory-averaged kernel** = the kinetics fuel node |
    /// | silver breakthrough through SiC | the kernel temperature (as upstream) | **average particle's SiC-layer mean** |
    /// | graphite hold-up (`graph_temp`) | bed volume average (whole ball) | **fuelled-zone matrix mean** |
    ///
    /// Why each: upstream TRISO-ATOPS takes a per-node fuel temperature for the
    /// node's whole inventory (`trisoatops.py:36-106`), i.e. a representative
    /// temperature, not a peak -- the inventory-weighted kernel is that, where
    /// the peak centre over-stated `D` by a factor ~1.2-2 at Arrhenius
    /// `Q = 126-488 kJ/mol` (gh:#360). Silver breakthrough is diffusion through
    /// the **SiC**, so its temperature is the SiC's (a deliberate, documented
    /// departure from upstream, which passes the kernel temperature). The
    /// graphite hold-up is diffusion through the matrix around the particles,
    /// which is the fuelled zone, not the cooler unfuelled shell.
    ///
    /// `None` -- the stack could not be formed -- means **this channel does not
    /// evaluate**; it still refuses to substitute the bed temperature (`D(T)`
    /// is exponential, so a wrong temperature gives a confidently wrong answer).
    /// Since 2026-09-28 the stack exists on every tier and up to the 3000 K
    /// window, so `None` is now reached only past it.
    ///
    /// Returns `true` when an evaluation actually ran.
    pub fn update(&mut self, sim_time_s: f64, temperatures: Option<FuelStackTemperatures>) -> bool {
        let Some(stack) = temperatures else {
            return false;
        };
        let due = match self.last_evaluated_s {
            None => true,
            Some(last) => (sim_time_s - last).abs() >= RELEASE_EVALUATION_INTERVAL_S,
        };
        if !due {
            return false;
        }

        // The live pools (gh:#399): stepped exactly over the time since the
        // last evaluation with this evaluation's source held constant; opened
        // at the 20-year history on the first one.
        let dt = self
            .last_evaluated_s
            .map_or(0.0, |last| (sim_time_s - last).max(0.0));
        let mut releases = self.evaluate_stack(sim_time_s, stack);
        // The cumulative leak each pool stood at before this step, so the
        // building receives exactly what left the circuit during it.
        let leaked_before: Vec<f64> = self
            .pools
            .as_ref()
            .map_or_else(Vec::new, |p| p.iter().map(|x| x.leaked).collect());
        let pools = match self.pools.take() {
            None => releases.iter().map(|r| r.opening_pools).collect::<Vec<_>>(),
            Some(previous) => previous
                .iter()
                .zip(releases.iter())
                .map(|(pool, r)| live_pools::step(*pool, r.source_atoms_per_s, r.pool_rates, dt).0)
                .collect(),
        };
        // The reactor building (gh:#400), fed by what the circuit leaked
        // over the step (its exact integral, as a mean rate, so atoms carry
        // over exactly); opened at its own steady state for the opening leak.
        let building_parameters = bishan::building::BuildingParameters::htr10();
        let building = match self.building.take() {
            None => pools
                .iter()
                .zip(releases.iter())
                .map(|(pool, r)| {
                    let inflow = r.pool_rates.leak * pool.circulating;
                    bishan::building::step(
                        bishan::building::BuildingInventory::default(),
                        inflow,
                        r.decay_constant,
                        building_parameters,
                        Time::new::<second>(POOL_OPENING_HISTORY_S),
                    )
                    .0
                })
                .collect::<Vec<_>>(),
            Some(previous) => previous
                .iter()
                .zip(pools.iter())
                .zip(leaked_before.iter())
                .zip(releases.iter())
                .map(|(((b, pool), leaked_before), r)| {
                    let inflow = if dt > 0.0 {
                        (pool.leaked - leaked_before) / dt
                    } else {
                        0.0
                    };
                    bishan::building::step(
                        *b,
                        inflow,
                        r.decay_constant,
                        building_parameters,
                        Time::new::<second>(dt),
                    )
                    .0
                })
                .collect(),
        };
        for ((release, pool), b) in releases.iter_mut().zip(pools.iter()).zip(building.iter()) {
            release.set_pools(*pool);
            release.set_building(*b, building_parameters);
        }
        self.pools = Some(pools);
        self.building = Some(building);
        self.latest = releases;
        self.evaluated_at_kernel = Some(stack.kernel);
        self.evaluated_at = Some(stack);
        self.last_evaluated_s = Some(sim_time_s);
        true
    }

    /// The live pools \[atoms\], one per tracked nuclide, `None` before the
    /// first evaluation.
    #[cfg(test)] // read by the conservation tests
    pub fn pools(&self) -> Option<&[PrimaryPools]> {
        self.pools.as_deref()
    }

    /// [`Self::evaluate`] with the whole fuel stack: kernel diffusion at the
    /// kernel temperature, **silver breakthrough at the SiC temperature**,
    /// graphite hold-up at the fuelled-zone matrix. See [`Self::update`].
    pub fn evaluate_stack(
        &self,
        sim_time_s: f64,
        stack: FuelStackTemperatures,
    ) -> Vec<NuclideRelease> {
        self.evaluate_with_sic(
            sim_time_s,
            stack.kernel,
            stack.silicon_carbide,
            stack.fuelled_zone_matrix,
        )
    }

    /// A fuel stack with one kernel temperature and one graphite temperature,
    /// the SiC at the kernel's (upstream TRISO-ATOPS's own convention) -- for
    /// tests and sweeps that vary a single fuel temperature.
    #[cfg(test)] // test and sweep helper
    pub fn kernel_and_graphite(
        kernel: ThermodynamicTemperature,
        graphite: ThermodynamicTemperature,
    ) -> FuelStackTemperatures {
        FuelStackTemperatures {
            kernel,
            silicon_carbide: kernel,
            fuelled_zone_matrix: graphite,
            peak_kernel: kernel,
        }
    }

    /// The fuel stack the most recent evaluation was taken at.
    #[cfg(test)] // test and sweep helper
    pub fn evaluated_at(&self) -> Option<FuelStackTemperatures> {
        self.evaluated_at
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
    ///
    /// This form passes the **kernel** temperature to silver's SiC
    /// breakthrough, exactly as upstream does; [`Self::evaluate_with_sic`] is
    /// the form the plant uses.
    #[cfg(test)] // test and sweep helper
    pub fn evaluate(
        &self,
        sim_time_s: f64,
        kernel_temperature: ThermodynamicTemperature,
        graphite_temperature: ThermodynamicTemperature,
    ) -> Vec<NuclideRelease> {
        self.evaluate_with_sic(
            sim_time_s,
            kernel_temperature,
            kernel_temperature,
            graphite_temperature,
        )
    }

    /// [`Self::evaluate`] with a separate SiC temperature for the silver
    /// group. For silver, TRISO-ATOPS's `rb_fail` uses its temperature argument
    /// **only** in the SiC diffusion coefficient `D_SiC,Ag(T)`
    /// (`boon_lay::triso_atops_fork::release_models::rb_fail`, Silver arm), and
    /// the kernel diffusion coefficient it computes for silver is not used by
    /// that arm; so handing the silver group the SiC temperature as its
    /// "core" temperature changes the breakthrough term and nothing else. Every
    /// other group gets the kernel temperature.
    pub fn evaluate_with_sic(
        &self,
        sim_time_s: f64,
        kernel_temperature: ThermodynamicTemperature,
        silicon_carbide_temperature: ThermodynamicTemperature,
        graphite_temperature: ThermodynamicTemperature,
    ) -> Vec<NuclideRelease> {
        let mut plant = self.inputs.plant;
        // `run_time` only feeds the closed-form pools, which this channel no
        // longer uses (the live pools replace them); the history is stated
        // for completeness.
        plant.run_time = Time::new::<second>(POOL_OPENING_HISTORY_S + sim_time_s.max(0.0));
        // HTR-10's measured free uranium plus the live in-service failure at
        // this kernel temperature (gh:#399). The chemical-attack hook is zero
        // until an ingress stage supplies it.
        let fractions = Htr10TrisoAtopsInputs::fractions_at(kernel_temperature, 0.0);

        let kernel_node = NodeState {
            core_temperature: kernel_temperature,
            graphite_temperature,
        };
        let silver_node = NodeState {
            core_temperature: silicon_carbide_temperature,
            graphite_temperature,
        };
        // One curie of this nuclide in the core: the unit basis of the
        // per-curie transfer function (`becquerels_from_curies` is boon-lay's
        // own Ci->Bq boundary).
        let unit_inventory = becquerels_from_curies(UNIT_INVENTORY_CURIES);

        self.nuclides
            .iter()
            .map(|nuclide| {
                let node = if nuclide.element_group() == ElementGroup::Silver {
                    silver_node
                } else {
                    kernel_node
                };
                let lam = nuclide.decay_constant();
                let lam_per_s = lam.get::<hertz>();
                // Upstream's short-lived flag: `hl / irad_time < 0.2`
                // (`calculation_functions.py:263`, `nuclide_import`'s
                // default `short_lived_ratio`). ~~`half_life <
                // IRRADIATION_TIME_S`, described as "upstream's own `sl`
                // flag"~~ -- CORRECTED 2026-09-29: that was not upstream's
                // criterion and put Ag-110m (250 d) on the short-lived branch.
                let short_lived = nuclide.half_life.get::<second>()
                    / Htr10TrisoAtopsInputs::IRRADIATION_TIME_S
                    < 0.2;
                let activities = normal_operation_node(
                    nuclide,
                    short_lived,
                    unit_inventory,
                    fractions,
                    plant,
                    node,
                    self.inputs.hps_enabled,
                    ParentPools::none(),
                )
                .to_curies(lam);

                // The fuel side at the published inventory, in atoms: release
                // rate, coolant source and graphite hold-up. Re-evaluated at
                // the real inventory rather than scaled from the per-curie
                // answer; `tests::the_absolute_arm_is_linear_in_inventory`
                // checks the two agree.
                let core_inventory_bq = htr10_core_inventory_bq(nuclide.name);
                let raw = core_inventory_bq.map(|bq| {
                    normal_operation_node(
                        nuclide,
                        short_lived,
                        Frequency::new::<hertz>(bq),
                        fractions,
                        plant,
                        node,
                        self.inputs.hps_enabled,
                        ParentPools::none(),
                    )
                });
                let source_atoms_per_s = raw.map_or(0.0, |r| r.source_rate);
                let pool_rates = htr10_pool_rates(nuclide.z, lam_per_s, self.primary_flow);
                let opening_pools = live_pools::step(
                    PrimaryPools::default(),
                    source_atoms_per_s,
                    pool_rates,
                    POOL_OPENING_HISTORY_S,
                )
                .0;
                let absolute = raw.map(|a| NodalActivitiesBq {
                    release_rate: a.release_rate * lam_per_s,
                    source_rate: a.source_rate * lam_per_s,
                    graphite_activity: a.graphite_activity * lam_per_s,
                    // Filled from the pools by `set_pools` below.
                    circulating_activity: 0.0,
                    plate_out_activity: 0.0,
                    clean_up_activity: 0.0,
                    leak_rate: 0.0,
                    stack_release_rate: 0.0,
                    building_activity: 0.0,
                });

                let mut release = NuclideRelease {
                    name: nuclide.name,
                    z: nuclide.z,
                    decay_constant: lam,
                    activities,
                    core_inventory_bq,
                    absolute,
                    source_atoms_per_s,
                    pool_rates,
                    opening_pools,
                };
                // A pure evaluation reports the opening pools; `update`
                // replaces them with the live ones.
                release.set_pools(opening_pools);
                release
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
    #[cfg(test)] // test and sweep helper
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
    #[cfg(test)] // test and sweep helper
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
                nuclide.half_life.get::<second>() / Htr10TrisoAtopsInputs::IRRADIATION_TIME_S < 0.2;
            let a = normal_operation_node(
                nuclide,
                short_lived,
                becquerels_from_curies(curies),
                Htr10TrisoAtopsInputs::fractions_at(node.core_temperature, 0.0),
                plant,
                node,
                inputs.hps_enabled,
                ParentPools::none(),
            )
            .to_curies(nuclide.decay_constant());
            [a.release_rate, a.source_rate, a.graphite_activity]
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
            ch.update(
                0.0,
                kernel.map(|k| TrisoAtopsReleaseChannel::kernel_and_graphite(k, graphite))
            ),
            "the first call must evaluate"
        );
        assert_eq!(ch.latest().len(), TRACKED_NUCLIDES.len());
        assert!(
            !ch.update(
                0.5,
                kernel.map(|k| TrisoAtopsReleaseChannel::kernel_and_graphite(k, graphite))
            ),
            "half an interval later must NOT re-evaluate"
        );
        assert!(
            ch.update(
                1.0,
                kernel.map(|k| TrisoAtopsReleaseChannel::kernel_and_graphite(k, graphite))
            ),
            "a full interval later must re-evaluate"
        );

        // No kernel: no evaluation, and the previous one survives with its own
        // timestamp so the display cannot present it as current.
        let before = ch.last_evaluated_s();
        assert!(!ch.update(99.0, None), "a missing kernel must not evaluate");
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

    /// V&V (gh:#399): **the release is compared, uncalibrated, with Liu & Cao
    /// (2002) Tables 2 and 3.**
    ///
    /// # Methodology
    ///
    /// Pre-registered instrument: kernel, SiC and fuelled-zone matrix all at
    /// **864 degC**, Liu & Cao's stated maximum normal-operation fuel-centre
    /// temperature (section 2.3) -- a bounding-high temperature for a
    /// core-average model -- at the rated 4.3 kg/s. Everything else is the
    /// channel as built: Table 1 inventory, Tang's free uranium 5.0e-5 as
    /// exposed heavy metal, boon-lay fuel failure's `phi_1`, HTR-10's residence
    /// time and HTR-10's pool constants. Nothing is fitted.
    ///
    /// 1. **Table 2** (release rate from the fuel elements, Bq h^-1 MWt^-1):
    ///    the model's coolant source `S` (after graphite hold-up, i.e. out of
    ///    the fuel element) x 3600 / 10 MWt.
    /// 2. **Table 3** (primary-helium activity after 20 full-power years, Bq):
    ///    the live pools' opening state, which is exactly that history.
    ///
    /// Reported as model / published per nuclide. No band is asserted -- the
    /// finding is the disagreement; only finiteness and positivity are.
    ///
    /// # Results (2026-09-29)
    ///
    /// | Nuclide | `S` model | Table 2 | model/pub | circulating model (20 a) | Table 3 | model/pub |
    /// |---|---|---|---|---|---|---|
    /// | Kr-85 | 1.607e3 | 1.5e4 | **0.107** | 3.219e5 Bq | 3.0e6 | **0.107** |
    /// | Xe-133 | 2.217e6 | 1.2e7 | **0.185** | 4.001e8 | 2.2e9 | **0.182** |
    /// | I-131 | 7.856e5 | 4.9e6 | **0.160** | 4.760e5 | 2.1e6 | **0.227** |
    /// | Cs-137 | 1.140e5 | 8.9e3 | **12.8** | 2.228e4 | 1.6e3 | **13.9** |
    /// | Ag-110m | 3.081e2 | 1.5e2 | **2.05** | 6.024e1 | 2.6e1 | **2.32** |
    ///
    /// (`S` in Bq h^-1 MWt^-1.) **Interpretation.** The circulating ratios
    /// track the source ratios, so HTR-10's own pool constants reproduce Liu &
    /// Cao's pool arithmetic; the disagreement is on the fuel side. The noble
    /// gases and iodine come out **5-10x low** and caesium **13x high**, with
    /// no fitting. Candidate causes, not tested here: Liu & Cao's release is a
    /// core-integrated calculation over a temperature distribution (this is
    /// one node at their stated maximum); their free-uranium treatment for the
    /// gases differs from TRISO-ATOPS's empirical `<R/B>`; and TRISO-ATOPS's
    /// caesium diffusivities and grain size are for UCO fuel, not HTR-10's UO2.
    #[test]
    fn the_release_is_compared_uncalibrated_with_liu_cao_tables_2_and_3() {
        use uom::si::thermodynamic_temperature::degree_celsius;
        let t = ThermodynamicTemperature::new::<degree_celsius>(864.0);
        let releases = channel().evaluate_with_sic(0.0, t, t, t);
        println!("nuclide | S model | Table 2 | ratio | circulating model | Table 3 | ratio  (Bq/(h MWt); Bq)");
        for r in &releases {
            let a = r
                .absolute
                .expect("every tracked nuclide has a Table 1 inventory");
            let s_model = a.source_rate * 3600.0 / 10.0;
            let s_pub =
                changi::activity::fuel_release::htr10_fuel_element_release_rate_bq_per_h_per_mwt(
                    r.name,
                )
                .expect("Table 2 lists every tracked nuclide");
            let c_pub = changi::activity::primary_helium::htr10_primary_helium_activity(r.name)
                .expect("Table 3 lists every tracked nuclide")
                .get::<uom::si::radioactivity::becquerel>();
            println!(
                "{} | {:.3e} | {:.2e} | {:.3e} | {:.3e} | {:.2e} | {:.3e}",
                r.name,
                s_model,
                s_pub,
                s_model / s_pub,
                a.circulating_activity,
                c_pub,
                a.circulating_activity / c_pub
            );
            assert!(s_model.is_finite() && s_model > 0.0);
            assert!(a.circulating_activity.is_finite() && a.circulating_activity > 0.0);
        }
    }

    /// **The unpublished free-uranium split bounds the gas and iodine
    /// release** (gh:#399). Tang's 5.0e-5 is `f_hm + f_sic` together; the
    /// channel takes it all as `f_hm`. Methodology: evaluate Kr-85, Xe-133 and
    /// I-131 at 864 degC with the whole fraction as `f_hm` and then as `f_sic`
    /// (with the same in-service `phi_1`); print the ratio. Results
    /// (2026-09-29): **1.587e7** for all three -- with the whole fraction as
    /// defective SiC the intact PyC retains the gases, and the release falls to
    /// the in-service `phi_1` share alone. The unpublished split is therefore
    /// the dominant uncertainty of the gas release; the channel's assignment is
    /// the upper bound.
    #[test]
    fn the_free_uranium_split_bounds_the_gas_release() {
        use boon_lay::triso_atops_fork::normal_operation::normal_operation_node;
        use uom::si::thermodynamic_temperature::degree_celsius;
        let t = ThermodynamicTemperature::new::<degree_celsius>(864.0);
        let ch = channel();
        let as_hm = Htr10TrisoAtopsInputs::fractions_at(t, 0.0);
        let as_sic = FailureFractions {
            heavy_metal: 0.0,
            sic: Htr10TrisoAtopsInputs::HTR10_FREE_URANIUM_FRACTION,
            ..as_hm
        };
        let node = NodeState {
            core_temperature: t,
            graphite_temperature: t,
        };
        for name in ["Kr-85", "Xe-133", "I-131"] {
            let n = ch.nuclides.iter().find(|n| n.name == name).unwrap();
            let sl = n.half_life.get::<second>() / Htr10TrisoAtopsInputs::IRRADIATION_TIME_S < 0.2;
            let r = |f| {
                normal_operation_node(
                    n,
                    sl,
                    becquerels_from_curies(1.0),
                    f,
                    ch.inputs.plant,
                    node,
                    true,
                    ParentPools::none(),
                )
                .release_rate
            };
            let (hm, sic) = (r(as_hm), r(as_sic));
            println!(
                "{name}: release with all free U as f_hm / as f_sic = {:.4e}",
                hm / sic
            );
            assert!(
                hm >= sic,
                "the exposed-kernel assignment must be the bounding one"
            );
        }
    }

    /// **The live pools open at the 20-year state, relax toward the source's
    /// equilibrium, carry the leak, and follow a hotter kernel up** (gh:#399).
    ///
    /// Methodology: update once at 900 K / 850 K (the opening), then every
    /// 1 s for 600 s; then raise the kernel to 1300 K for 600 s. Require: the
    /// first evaluation's pools equal the opening pools exactly; `leak_rate =
    /// k_leak x circulating` to 1e-12; circulating Xe-133 higher after the
    /// heat-up than before. Results (2026-09-29): Xe-133 circulating
    /// 6.5752e7 -> 7.3845e7 Bq, leak 7.61 -> 8.55 Bq/s.
    #[test]
    fn the_live_pools_open_carry_the_leak_and_follow_the_kernel() {
        let mut ch = channel();
        let k = |v| ThermodynamicTemperature::new::<kelvin>(v);
        let stack = |kernel| TrisoAtopsReleaseChannel::kernel_and_graphite(k(kernel), k(850.0));
        assert!(ch.update(0.0, Some(stack(900.0))));
        for r in ch.latest() {
            let lam = r.decay_constant.get::<hertz>();
            let a = r.absolute.unwrap();
            assert_eq!(a.circulating_activity, r.opening_pools.circulating * lam);
        }
        for i in 1..=600 {
            ch.update(i as f64, Some(stack(900.0)));
        }
        let before = ch
            .latest()
            .iter()
            .find(|r| r.name == "Xe-133")
            .unwrap()
            .absolute
            .unwrap();
        for i in 601..=1200 {
            ch.update(i as f64, Some(stack(1300.0)));
        }
        let after = ch
            .latest()
            .iter()
            .find(|r| r.name == "Xe-133")
            .unwrap()
            .absolute
            .unwrap();
        for r in ch.latest() {
            let a = r.absolute.unwrap();
            assert!(
                (a.leak_rate - r.pool_rates.leak * a.circulating_activity).abs()
                    <= 1e-12 * a.leak_rate.abs().max(1e-300)
            );
        }
        println!(
            "Xe-133 circulating: {:.4e} Bq at 900 K kernel, {:.4e} Bq after 600 s at 1300 K; leak {:.4e} -> {:.4e} Bq/s",
            before.circulating_activity, after.circulating_activity, before.leak_rate, after.leak_rate
        );
        assert!(after.circulating_activity > before.circulating_activity);
        assert!(ch.pools().unwrap().iter().all(|p| p.leaked > 0.0));
    }

    /// **The chemical-attack hook adds to the in-service failure** and to
    /// nothing else (gh:#399; consumers gh:#401, #402).
    #[test]
    fn the_chemical_attack_hook_adds_to_the_in_service_failure() {
        let t = ThermodynamicTemperature::new::<kelvin>(1100.0);
        let base = Htr10TrisoAtopsInputs::fractions_at(t, 0.0);
        let hit = Htr10TrisoAtopsInputs::fractions_at(t, 1.0e-3);
        assert!((hit.incremental - base.incremental - 1.0e-3).abs() < 1e-18);
        assert_eq!(hit.heavy_metal, base.heavy_metal);
        assert_eq!(hit.sic, base.sic);
        assert_eq!(hit.incremental_sic, base.incremental_sic);
        assert_eq!(base.heavy_metal, 5.0e-5);
    }

    /// The design-point fuel stack the plant opens at: the bed at its seed
    /// temperature, plus `bed_offset_k`, and the fuel node at the bed plus
    /// `R P_rated`. This mirrors `KineticsChannel::new_htr10_published`, so
    /// the comparison is taken at the same state the kinetics is seeded at.
    fn design_point_stack(bed_offset_k: f64) -> FuelStackTemperatures {
        use super::super::pebble_bed::{FuelBedCoupling, PebbleBedPorousMediaNode};
        use uom::si::power::megawatt;
        let bed = PebbleBedPorousMediaNode::new()
            .pebble_temperature()
            .get::<kelvin>()
            + bed_offset_k;
        let coupling = FuelBedCoupling::at_design_point();
        let rise = (coupling.resistance * uom::si::f64::Power::new::<megawatt>(10.0))
            .get::<uom::si::temperature_interval::kelvin>();
        coupling.stack(
            ThermodynamicTemperature::new::<kelvin>(bed),
            ThermodynamicTemperature::new::<kelvin>(bed + rise),
        )
    }

    /// **V&V (gh:#378): the absolute circulating activity against Liu and Cao
    /// (2002) Table 3, the published HTR-10 primary-helium activity.**
    ///
    /// # Methodology
    ///
    /// - **Computed.** `NuclideRelease::absolute.circulating_activity` \[Bq\] for
    ///   the five [`TRACKED_NUCLIDES`], evaluated by the plant's own channel
    ///   ([`TrisoAtopsReleaseChannel::evaluate_stack`]) at the **design-point
    ///   fuel stack** the simulator is seeded at: bed 950 K (the seed), and the
    ///   fuel node at bed + `R P` for 10 MW through
    ///   [`FuelBedCoupling::at_design_point`](super::super::pebble_bed::FuelBedCoupling::at_design_point).
    ///   This is the state `KineticsChannel::new_htr10_published` opens at.
    /// - **Reference.** `changi::activity::primary_helium`, Liu and Cao (2002)
    ///   Table 3: primary-helium activity at the **end of a 20-year full-power
    ///   life** \[Bq\].
    /// - **Adjustment column, labelled, not a model run.** #370: the absolute arm
    ///   converts the equilibrium-core inventory to a birth rate by dividing by
    ///   `1 - exp(-lambda t_irr)` with `t_irr` = 1 y, which inflates long-lived
    ///   nuclides. `ratio x (1 - exp(-lambda t_irr))` removes that one factor
    ///   analytically, for long-lived nuclides only. Ag-110m is on the
    ///   short-lived branch today (#369), so it carries no inflation and needs
    ///   no adjustment. Fixing #369 would add exactly this factor to it and
    ///   #370 would then remove it, so the two defects cancel for Ag-110m.
    /// - **Sensitivity.** The bed is moved -100 K and +100 K from the seed,
    ///   because #372 shows the one-node bed is outlet-referenced, not
    ///   core-average.
    /// - **Pass criterion: a sanity check only, NOT a physics gate.** Each ratio
    ///   is finite and positive (silver: non-negative, see below). **Nothing is
    ///   tuned to Table 3**; the numbers are recorded, not matched.
    ///   *History, stated because it would otherwise look like goal-post
    ///   moving:* a 1e-4..1e4 "wiring band" was set before the first
    ///   measurement. Cs-137 (2.8e-6) and then Ag-110m (2.5e-18) fell outside
    ///   it for physical reasons, not wiring faults, so the band was dropped
    ///   rather than widened a second time. The spread IS the finding.
    ///
    /// # Results (2026-09-29, branch `claude/htgr-sim-v1-source-term-u7qwe0`)
    ///
    /// Run with `cargo test --release -p outram-park-digital-twin-engine
    /// --example htgr_sim_v1 -- circulating_activity_against_liu_and_cao
    /// --nocapture`. `adj.` is sim/T3 x (1 - exp(-lambda t_irr)) (#370, long-lived only).
    ///
    /// **Design stack (bed 954.9 K matrix, 955.1 K SiC, 960.3 K kernel):**
    ///
    /// | Nuclide | sim \[Bq\] | Table 3 \[Bq\] | sim/T3 | adj. |
    /// |---|---|---|---|---|
    /// | Kr-85 | 2.8227e4 | 3.0e6 | 9.409e-3 | 5.881e-4 |
    /// | Xe-133 | 1.5625e7 | 2.2e9 | 7.102e-3 | (short) |
    /// | I-131 | 5.8893e5 | 2.1e6 | 2.804e-1 | (short) |
    /// | Cs-137 | 8.4268e-3 | 1.6e3 | 5.267e-6 | 1.200e-7 |
    /// | Ag-110m | 0 | 26 | 0 | (short) |
    ///
    /// **Sensitivity, sim/T3 at bed -100 K / seed / +100 K:**
    ///
    /// | Nuclide | -100 K | seed | +100 K |
    /// |---|---|---|---|
    /// | Kr-85 | 3.333e-3 | 9.409e-3 | 2.184e-2 |
    /// | Xe-133 | 2.765e-3 | 7.102e-3 | 1.527e-2 |
    /// | I-131 | 1.092e-1 | 2.804e-1 | 6.029e-1 |
    /// | Cs-137 | 2.776e-6 | 5.267e-6 | **2.708e1** |
    /// | Ag-110m | 2.5e-18 | 0 | 4.4e-11 |
    ///
    /// # Interpretation
    ///
    /// 1. **The simulator is LOW for every tracked nuclide at its design
    ///    stack**, by 100-140x for the noble gases and 3.6x for I-131. #359's
    ///    expectation, and #370's direction for the long-lived arm, was that
    ///    the absolute arm might read high. Removing the #370 inflation makes
    ///    Kr-85 and Cs-137 **lower still**, so that inflation is not what sets
    ///    the gap.
    /// 2. **For the noble gases, the helium-purification rate constant alone
    ///    accounts for most of it.** Without plate-out, `C = R/(lambda +
    ///    k_clean)`. For Xe-133, `(lambda + k_clean)/lambda` = (1.52e-6 +
    ///    8.77e-5)/1.52e-6 ~ 59, and `k_clean` is TRISO-ATOPS's NP-MHTGR
    ///    reference value, not HTR-10's (see
    ///    [`Htr10TrisoAtopsInputs::TRISO_ATOPS_CLEAN_UP_PER_S`]). That is
    ///    arithmetic on the model's own form, not a separate run. The remainder
    ///    (~2x) sits in the failure fractions (TRISO-ATOPS reference, not
    ///    HTR-10) and the noble-gas `R/B` at 960 K.
    /// 3. **Cs-137 is dominated by graphite hold-up, and it is a cliff:** x5e6
    ///    between the seed and +100 K. So the temperature the graphite term is
    ///    evaluated at (#371) and the outlet-referenced bed (#372) are not
    ///    second-order for caesium. They decide the answer.
    /// 4. **Ag-110m is ~0**: TRISO-ATOPS releases silver only by breakthrough
    ///    of *intact* SiC, and failed particles add none on that path. At
    ///    955 K the SiC diffusion lag `a^2/6D` is of order 10^3 years. The
    ///    nonzero values at 855 K and 1055 K are f64 round-off in an
    ///    ill-conditioned series (see `boon-lay`'s code-to-code doc).
    ///    Table 3's 26 Bq therefore needs a mechanism this model does not have.
    /// 5. **Bases differ, stated rather than corrected:** Table 3 is the end of
    ///    a 20-year life; the simulator uses a 1-year irradiation and the
    ///    Table 1 equilibrium-core inventory.
    ///
    /// Nothing here is a validation of anything. It is the first measured
    /// comparison of this chain against published HTR-10 primary-circuit data,
    /// and it says the placeholder circuit constants, not the release physics,
    /// dominate the noble-gas and iodine answer.
    #[test]
    fn circulating_activity_against_liu_and_cao_table_3() {
        use changi::activity::primary_helium::htr10_primary_helium_activity;
        use uom::si::radioactivity::becquerel;

        let channel = TrisoAtopsReleaseChannel::new_htr10();
        let t_irr = Htr10TrisoAtopsInputs::IRRADIATION_TIME_S;

        for offset in [-100.0, 0.0, 100.0] {
            let stack = design_point_stack(offset);
            println!(
                "bed offset {offset:+.0} K: kernel {:.2} K, SiC {:.2} K, fuelled-zone matrix {:.2} K",
                stack.kernel.get::<kelvin>(),
                stack.silicon_carbide.get::<kelvin>(),
                stack.fuelled_zone_matrix.get::<kelvin>(),
            );
            println!(
                "{:>8} {:>7} {:>12} {:>12} {:>10} {:>12} {:>14}",
                "nuclide",
                "branch",
                "sim [Bq]",
                "Table 3 [Bq]",
                "sim/T3",
                "#370 factor",
                "sim/T3 adj."
            );
            for r in channel.evaluate_stack(0.0, stack) {
                let sim = r
                    .absolute
                    .expect("tracked nuclides all have an inventory")
                    .circulating_activity;
                let table3 = htr10_primary_helium_activity(r.name)
                    .unwrap_or_else(|| panic!("{} is not in Table 3", r.name))
                    .get::<becquerel>();
                let lambda = r.decay_constant.get::<hertz>();
                let long_lived = std::f64::consts::LN_2 / lambda >= t_irr;
                let inflation_removed = if long_lived {
                    1.0 - (-lambda * t_irr).exp()
                } else {
                    1.0
                };
                let ratio = sim / table3;
                println!(
                    "{:>8} {:>7} {:>12.4e} {:>12.4e} {:>10.4e} {:>12.4e} {:>14.4e}",
                    r.name,
                    if long_lived { "long" } else { "short" },
                    sim,
                    table3,
                    ratio,
                    inflation_removed,
                    ratio * inflation_removed,
                );
                // Silver's intact-SiC breakthrough is ~0 at these temperatures and
                // sits at f64 round-off (see the doc), so it may legitimately be 0.
                let floor_ok = if r.name == "Ag-110m" {
                    ratio >= 0.0
                } else {
                    ratio > 0.0
                };
                assert!(
                    ratio.is_finite() && floor_ok,
                    "{}: sim/Table 3 = {ratio:e} is not finite and in range",
                    r.name
                );
            }
        }
    }
}
