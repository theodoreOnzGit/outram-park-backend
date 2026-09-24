// SPDX-License-Identifier: GPL-3.0
//
// HTR-10 DLOFC case — provenance
// ------------------------------
// Transient shape : Kugeler, K., Nabielek, H. & Buckthorpe, D. (2017), "The High
//                   Temperature Gas-cooled Reactor: Safety considerations of the
//                   (V)HTR-Modul", EUR 28712 EN, JRC107642, doi:10.2760/270321.
//                   Sections 7.2 and 9.9.2, and Table 44. OPEN literature; reuse
//                   authorised with acknowledgement (EC Decision 2011/833/EU).
//                   In-repo: crates/kovan-literature/generated/markdown/open/
//                   vhtr-modul-safety-jrc.md (catalogue entry `kugeler2017vhtr`).
// Failure model   : PANAMA-I via `boon_lay::fuel_failure` (HTA-IB-03/90).
// Release model   : TRISO-ATOPS via `boon_lay::triso_atops_fork` (INL, MIT).
// Geometry        : NOT stated here. Supplied by the caller, so that the
//                   published HTR-10 pebble and particle dimensions live in
//                   exactly one place in this workspace
//                   (`tampines::pebble_bed`) and cannot drift.

//! # HTR-10 depressurised loss of forced cooling — the PANAMA ↔ TRISO-ATOPS seam
//!
//! This module joins the two halves of `boon-lay` that had never been run
//! together on one reactor:
//!
//! ```text
//!   fuel_failure (PANAMA-I)          triso_atops_fork (TRISO-ATOPS)
//!   phi_1, phi_2 over a transient -> f_inc_acc -> release fractions -> Ci
//! ```
//!
//! and drives the result with the **published HTR-10 equilibrium-core
//! inventory** (`changi::activity::inventory`, Liu & Cao 2002 Table 1). It
//! owns the transient *shape* and the seam; it owns no geometry and no
//! inventory, both of which already exist elsewhere in the workspace.
//!
//! **RESEARCH, EDUCATION AND V&V ONLY.** Not a source term for HTR-10 or any
//! other plant, not for emergency planning, licensing or safety analysis. See
//! the workspace `RESPONSIBLE_USE.md`. Per `AI_USAGE.md` this is AI-assisted
//! draft material pending human review.
//!
//! # Which seam is worth having, and why it is not the normal-operation one
//!
//! `boon_lay::fuel_failure::htr10` established (2026-09-24) that PANAMA's
//! in-service failure fraction under **normal operation** is `10⁻¹⁵`–`10⁻⁶`
//! across HTR-10's plausible fuel-temperature band, against the `3·10⁻⁵`
//! as-manufactured placeholder that release calculations actually use.
//! Substituting one for the other would divide every reported activity by
//! about `10⁷` on the strength of a model answering a different question.
//!
//! So this module wires PANAMA in where it belongs: as
//! [`AccidentFractions::incremental_accident`], the **accident-added**
//! in-service failure fraction. The four normal-operation fractions stay the
//! caller's, because PANAMA models none of them — its own equivalent, `φ_o`,
//! is an input to it too (HTA-IB-03/90 page -480-).
//!
//! # The transient: HTR-Module's DLOFC history, used as a STAND-IN for HTR-10's
//!
//! HTR-10's own depressurised-loss-of-forced-cooling fuel temperature history
//! is **not published in this workspace's literature.** What is available, in
//! the open JRC (V)HTR-Modul volume already used for this fuel line's
//! strength data, is HTR-Module's:
//!
//! | Quantity | Value | Where |
//! |---|---|---|
//! | peak fuel temperature | **1500 °C** | §9.9.2 / §7.2.2: "After 30 hours the maximum fuel temperature reaches 1 500 °C. After reaching the maximum the temperature decreases." |
//! | time to peak | **30 h** | same, and §7.2.2 "around 1 500 °C in the hotspot region of the HTR Module after 30 hours" |
//! | duration above 1500 °C | ~30 h for < 5 % of elements, upper edge ~1550 °C | §7.2.2 |
//! | nominal / maximum peak | **1450 °C / 1615 °C** | Table 44 (uncertainty analysis, 200 MW HTR-Module) |
//! | fuel temperature limit | 1600 °C | stated throughout |
//!
//! **Using HTR-Module's history for HTR-10 is a stand-in, and it is the
//! conservative direction.** HTR-10 is 10 MW thermal against HTR-Module's
//! 200 MW, at a lower mean power density and with a far shorter heat-transport
//! path out of the core, so its own DLOFC peak is expected *below* 1500 °C.
//! The choice is also *consistent*: `boon_lay::fuel_failure::htr10` already
//! takes its `σ_oo`/`m_oo` and `Γ` stand-ins from this same HTR-Module column
//! of this same report, so the transient and the fuel strength come from one
//! reactor rather than two.
//!
//! # Two parameters of the shape are ESTIMATES, and they are flagged as such
//!
//! The report states the *rise* (1500 °C at 30 h) and that the temperature
//! then decreases, but its cooldown leg is a **figure** (Figures 15, 74, 158)
//! and is not digitised in this workspace. So:
//!
//! - [`ESTIMATED_COOLDOWN_TIME_CONSTANT_HOURS`] — the post-peak relaxation
//!   time constant. **Estimated, not cited.**
//! - [`ESTIMATED_LATE_TIME_CELSIUS`] — the temperature the core relaxes
//!   towards. **Estimated, not cited.**
//!
//! Both are swept in the example rather than asserted, and GitHub #296 asks
//! the maintainer for the digitised figure that would replace them. Their
//! influence is bounded and stated: TRISO-ATOPS's own venting model releases
//! only while the core is **heating** (`coolant_release` selects
//! `dT/dt >= 0`), so the cooldown leg contributes **nothing** to the vented
//! source term and these two estimates move the reported release by zero. They
//! affect PANAMA's failure accumulation only, where they act on a leg whose
//! temperature is falling and whose contribution is correspondingly small.
//!
//! # The time grid, and the 0.39 % it costs
//!
//! PANAMA steps between samples at the interval's midpoint temperature, so the
//! answer depends on the grid and the dependence is **measured** rather than
//! assumed away. Everything here and in the example runs at **201 samples**
//! over 200 h, a 1 h step. The convergence study in
//! [`tests::the_time_grid_converges_at_second_order_and_the_error_is_stated`]
//! shows second-order convergence from the 2 h step down and puts the
//! 201-sample answer **0.39 % below** the extrapolated limit of
//! `1.071920·10⁻⁷`. The 4 h step is 16 % low and is not in the asymptotic
//! range. Nothing here is quoted to better than that.
//!
//! # What the seam cannot yet do: the failure fraction is a SCALAR
//!
//! [`crate::accident::release::accident_release`] takes one
//! [`AccidentFractions`] for the whole run, so a single number has to stand
//! for `f_inc_acc(t)`. Using the end-of-transient value applies the final
//! failure fraction from `t = 0` and therefore **over-predicts the early
//! release**; using the value at the peak under-predicts the late release,
//! which the venting model discards anyway. Both are reported. A
//! time-dependent failure fraction threaded through the release chain is the
//! obvious next step and is **not done here**.

use boon_lay::fuel_failure::history::{AccidentHistory, AccidentStep};
use boon_lay::fuel_failure::htr10 as panama_htr10;
use boon_lay::triso_atops_fork::accident::AccidentFractions;
use uom::si::f64::{Length, Pressure, ThermodynamicTemperature, Time};
use uom::si::pressure::kilopascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::{hour, second};

use crate::accident::release::PlantParameters;
use crate::error::Result;
use crate::scenario::TemperatureTransient;

/// Peak fuel temperature of the HTR-Module DLOFC history, **1500 °C**
/// (JRC EUR 28712 EN §9.9.2: "After 30 hours the maximum fuel temperature
/// reaches 1 500 °C").
pub const DLOFC_PEAK_CELSIUS: f64 = 1500.0;

/// Time to that peak, **30 h**, from the same sentence.
pub const DLOFC_TIME_TO_PEAK_HOURS: f64 = 30.0;

/// Table 44's **nominal** calculated peak fuel temperature for the same
/// accident, 1450 °C — the low arm of the report's own uncertainty analysis.
pub const DLOFC_TABLE_44_NOMINAL_CELSIUS: f64 = 1450.0;

/// Table 44's **maximum** calculated peak fuel temperature, 1615 °C — the high
/// arm, and the only one of the three that exceeds the 1600 °C limit.
pub const DLOFC_TABLE_44_MAXIMUM_CELSIUS: f64 = 1615.0;

/// The fuel temperature limit the whole design argument is built around,
/// 1600 °C (stated throughout the JRC volume).
pub const FUEL_TEMPERATURE_LIMIT_CELSIUS: f64 = 1600.0;

/// How long the transient is carried, **200 h** — the window HTA-IB-03/90
/// itself reports HTR-Module depressurised failure at (page -504-), so PANAMA
/// results here are directly comparable with that statement.
pub const REPORTING_WINDOW_HOURS: f64 = 200.0;

/// `T_B`, the pre-accident average fuel temperature, **776 °C** — HTR-Module's
/// published average (HTA-IB-03/90 Table 2). **A stand-in, not HTR-10 data:**
/// HTR-10 publishes a *maximum* fuel temperature and PANAMA's `T_B` is an
/// average. Same stand-in `boon_lay::fuel_failure::htr10` uses.
pub const STAND_IN_IRRADIATION_CELSIUS: f64 = 776.0;

/// **ESTIMATE, not cited.** Post-peak relaxation time constant, 60 h.
///
/// The JRC volume states the peak and that the temperature falls after it, but
/// gives the decay only as a figure. 60 h is chosen so the core is back near
/// its late-time level by the 200 h reporting window, which is the shape the
/// report's figures show; nothing in the text fixes it.
///
/// It cannot move the reported release (see the module docs: the venting model
/// discards every cooling sample), so it is a sensitivity on PANAMA's failure
/// accumulation and nothing else.
pub const ESTIMATED_COOLDOWN_TIME_CONSTANT_HOURS: f64 = 60.0;

/// **ESTIMATE, not cited.** The temperature the core relaxes towards after the
/// peak, 900 °C.
///
/// Chosen above the 700 °C lower edge of TRISO-ATOPS's fitted Arrhenius range
/// (`crate::accident::release::DIFFUSION_FIT_MIN_CELSIUS`) so the late leg is
/// computed by the correlation rather than by its clamp — which means the
/// choice is visible in the caveats if it is changed downwards, rather than
/// silently altering the physics.
pub const ESTIMATED_LATE_TIME_CELSIUS: f64 = 900.0;

/// A DLOFC fuel-temperature history: linear rise to a peak, then exponential
/// relaxation towards a late-time level.
///
/// **An analytic shape, not a thermal-hydraulic solution.** Nothing in this
/// workspace computes HTR-10's DLOFC transient; `htgr_sim_v1`'s LOFC scenario
/// is a *pressurised* circulator trip arrested by Doppler feedback within the
/// hour and does not reach this regime.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DlofcShape {
    /// Pre-accident fuel temperature, `T_B`.
    pub initial: ThermodynamicTemperature,
    /// Peak fuel temperature.
    pub peak: ThermodynamicTemperature,
    /// Time from accident start to the peak.
    pub time_to_peak: Time,
    /// Temperature the core relaxes towards after the peak.
    pub late_time: ThermodynamicTemperature,
    /// Relaxation time constant of the cooling leg.
    pub cooldown_time_constant: Time,
    /// Total duration carried.
    pub total: Time,
}

impl DlofcShape {
    /// The HTR-Module DLOFC history as the JRC volume states it, with the two
    /// estimated cooldown parameters.
    ///
    /// Peak [`DLOFC_PEAK_CELSIUS`] at [`DLOFC_TIME_TO_PEAK_HOURS`], from
    /// [`STAND_IN_IRRADIATION_CELSIUS`], carried to
    /// [`REPORTING_WINDOW_HOURS`].
    #[must_use]
    pub fn htr_module_jrc() -> Self {
        Self::with_peak(ThermodynamicTemperature::new::<degree_celsius>(
            DLOFC_PEAK_CELSIUS,
        ))
    }

    /// The same shape at a different peak — for the Table 44 arms, or for a
    /// sensitivity on the one input the report gives three values for.
    #[must_use]
    pub fn with_peak(peak: ThermodynamicTemperature) -> Self {
        Self {
            initial: ThermodynamicTemperature::new::<degree_celsius>(
                STAND_IN_IRRADIATION_CELSIUS,
            ),
            peak,
            time_to_peak: Time::new::<hour>(DLOFC_TIME_TO_PEAK_HOURS),
            late_time: ThermodynamicTemperature::new::<degree_celsius>(
                ESTIMATED_LATE_TIME_CELSIUS,
            ),
            cooldown_time_constant: Time::new::<hour>(ESTIMATED_COOLDOWN_TIME_CONSTANT_HOURS),
            total: Time::new::<hour>(REPORTING_WINDOW_HOURS),
        }
    }

    /// The fuel temperature at one instant.
    ///
    /// Linear on `[0, t_peak]`; on the cooling leg
    /// `T = T_late + (T_peak − T_late)·exp(−(t − t_peak)/τ)`.
    #[must_use]
    pub fn temperature_at(&self, t: Time) -> ThermodynamicTemperature {
        let t_s = t.get::<second>();
        let peak_s = self.time_to_peak.get::<second>();
        let t0 = self.initial.get::<degree_celsius>();
        let t1 = self.peak.get::<degree_celsius>();
        let t_late = self.late_time.get::<degree_celsius>();
        let c = if t_s <= peak_s {
            t0 + (t1 - t0) * (t_s / peak_s)
        } else {
            let tau = self.cooldown_time_constant.get::<second>();
            t_late + (t1 - t_late) * (-(t_s - peak_s) / tau).exp()
        };
        ThermodynamicTemperature::new::<degree_celsius>(c)
    }

    /// Uniformly spaced sample times over `[0, total]`.
    ///
    /// # Panics
    /// Panics if `samples < 2`.
    #[must_use]
    pub fn sample_times(&self, samples: usize) -> Vec<Time> {
        assert!(samples >= 2, "a transient needs at least two samples");
        let total = self.total.get::<second>();
        let dt = total / (samples - 1) as f64;
        (0..samples)
            .map(|i| Time::new::<second>(i as f64 * dt))
            .collect()
    }

    /// The temperature history at those sample times.
    #[must_use]
    pub fn history(&self, samples: usize) -> Vec<ThermodynamicTemperature> {
        self.sample_times(samples)
            .into_iter()
            .map(|t| self.temperature_at(t))
            .collect()
    }

    /// The shape as a [`TemperatureTransient`], every node on the same history.
    ///
    /// **One history for every node means there is no hot node**, which is the
    /// thing a real calculation turns on: the JRC figures are explicit that
    /// under 5 % of elements reach the peak. So this over-states the release by
    /// applying the hot-node history to the whole core, and that direction is
    /// the reason it is safe to report as a bound rather than an estimate.
    ///
    /// # Errors
    /// Propagates [`TemperatureTransient::from_nodes`].
    pub fn transient(
        &self,
        samples: usize,
        n_radial: usize,
        n_axial: usize,
    ) -> Result<TemperatureTransient> {
        let times = self.sample_times(samples);
        let profile = self.history(samples);
        let temperatures = vec![
            profile
                .iter()
                .map(|t| vec![*t; n_axial])
                .collect::<Vec<_>>();
            n_radial
        ];
        TemperatureTransient::from_nodes(times, temperatures)
    }
}

/// PANAMA-I walked over a DLOFC history: the failure trail and the two scalars
/// the release chain can consume.
#[derive(Debug, Clone, PartialEq)]
pub struct PanamaOverTransient {
    /// Sample times, matching [`DlofcShape::sample_times`].
    pub times: Vec<Time>,
    /// Fuel temperature at each sample, °C.
    pub temperature_celsius: Vec<f64>,
    /// `φ₁`, pressure-vessel overstress, at each sample.
    pub phi_1: Vec<f64>,
    /// `φ₂`, SiC thermal decomposition, at each sample.
    pub phi_2: Vec<f64>,
    /// `f_inc = 1 − (1−φ₁)(1−φ₂)` at each sample — PANAMA's in-service failure
    /// fraction, which is what TRISO-ATOPS calls `incremental`.
    pub in_service: Vec<f64>,
    /// `φ₁` at the end of irradiation, i.e. `f_inc` at `t = 0`. PANAMA sets
    /// `φ₁(t=0)` to this rather than to zero (page -482-).
    pub end_of_irradiation_phi_1: f64,
    /// `f_inc(end) − f_inc(0)` — the accident-added failure over the whole
    /// transient. The **conservative** arm for `f_inc_acc`.
    pub accident_increment_final: f64,
    /// `f_inc(t_peak) − f_inc(0)` — the accident-added failure up to the peak,
    /// which is where the venting window ends. The **consistent** arm.
    pub accident_increment_at_peak: f64,
}

/// Walk PANAMA-I over a DLOFC history for the HTR-10 particle.
///
/// The particle is `boon_lay::fuel_failure::htr10::particle`: HTR-10's
/// published geometry, kernel compound, derived burnup (`F_b = 0.0851` FIMA)
/// and derived residence (`t_B = 1080` FPD), with the two named HTR-Module
/// stand-ins for `σ_oo`/`m_oo` and `Γ`. Read that module before quoting any
/// number this returns.
///
/// `φ_o` is **zero** in the particle, deliberately: the as-manufactured
/// population is carried by TRISO-ATOPS's own `f_hm` / `f_sic`, and counting it
/// in both places would double it.
///
/// # Arguments
/// - `shape` — the temperature history.
/// - `irradiation_temperature` — `T_B`. PANAMA's oxygen and diffusion terms
///   need it and it is **not** the accident temperature.
/// - `samples` — number of history samples; the accident is stepped between
///   consecutive samples at their midpoint temperature, which is PANAMA's own
///   `T_m` (page -482-).
///
/// # Panics
/// Panics if `samples < 2`.
#[must_use]
pub fn panama_over_transient(
    shape: &DlofcShape,
    irradiation_temperature: ThermodynamicTemperature,
    samples: usize,
) -> PanamaOverTransient {
    let times = shape.sample_times(samples);
    let history = shape.history(samples);

    let particle = panama_htr10::particle(irradiation_temperature);
    let phi_1_0 = panama_htr10::end_of_irradiation_failure(irradiation_temperature);
    let mut run = AccidentHistory::new(particle, phi_1_0);

    let mut phi_1 = Vec::with_capacity(samples);
    let mut phi_2 = Vec::with_capacity(samples);
    let mut in_service = Vec::with_capacity(samples);
    let mut temperature_celsius = Vec::with_capacity(samples);

    let record = |p: &boon_lay::fuel_failure::history::FailureProgress,
                  phi_1: &mut Vec<f64>,
                  phi_2: &mut Vec<f64>,
                  in_service: &mut Vec<f64>| {
        phi_1.push(p.pressure_vessel.get::<ratio>());
        phi_2.push(p.thermal_decomposition.get::<ratio>());
        in_service.push(p.in_service_failure_fraction().get::<ratio>());
    };

    record(&run.progress(), &mut phi_1, &mut phi_2, &mut in_service);
    temperature_celsius.push(history[0].get::<degree_celsius>());

    for i in 1..samples {
        // T_m, the mean temperature over the interval (page -482-).
        let mean_c = 0.5
            * (history[i - 1].get::<degree_celsius>() + history[i].get::<degree_celsius>());
        let progress = run.step(AccidentStep {
            duration: times[i] - times[i - 1],
            mean_temperature: ThermodynamicTemperature::new::<degree_celsius>(mean_c),
        });
        record(&progress, &mut phi_1, &mut phi_2, &mut in_service);
        temperature_celsius.push(history[i].get::<degree_celsius>());
    }

    let f_inc_0 = in_service[0];
    let accident_increment_final = (in_service[samples - 1] - f_inc_0).max(0.0);

    // The sample at or just past the peak — the venting window's last sample.
    let peak_s = shape.time_to_peak.get::<second>();
    let peak_index = times
        .iter()
        .position(|t| t.get::<second>() >= peak_s)
        .unwrap_or(samples - 1);
    let accident_increment_at_peak = (in_service[peak_index] - f_inc_0).max(0.0);

    PanamaOverTransient {
        times,
        temperature_celsius,
        phi_1,
        phi_2,
        in_service,
        end_of_irradiation_phi_1: phi_1_0.get::<ratio>(),
        accident_increment_final,
        accident_increment_at_peak,
    }
}

/// TRISO-ATOPS's own NP-MHTGR reference **normal-operation** failure
/// fractions, with both accident fields zero.
///
/// `f_hm = 1·10⁻⁴`, `f_sic = 1·10⁻⁴`, `f_inc = 2.3·10⁻⁵`,
/// `f_inc_sic = 3.6·10⁻⁵` — read out of
/// [`PlantParameters::np_mhtgr_reference`] rather than retyped, so there is one
/// copy of them in this workspace.
///
/// **These are NOT HTR-10 fuel-qualification data.** They are a manufacturing
/// and irradiation result for the NP-MHTGR reference fuel. HTR-10's fuel is
/// German-lineage, and the measured German record —
/// `boon_lay::fuel_failure::htr10::qualification::FREE_URANIUM_FRACTIONS`,
/// `7.8·10⁻⁶` to `50.7·10⁻⁶` — is roughly an order of magnitude *better* than
/// the `1·10⁻⁴` here. Release from noble gases and halogens is **linear** in
/// `f_hm`, so that is a factor ~2–13 on every volatile number computed with
/// this set. Both arms are reported in the example rather than one being
/// chosen.
#[must_use]
pub fn np_mhtgr_normal_operation_fractions() -> AccidentFractions {
    PlantParameters::np_mhtgr_reference(0.0, 0.0, 0.0).fractions
}

/// Set the accident-phase incremental fraction from PANAMA, leaving the four
/// normal-operation fractions untouched.
///
/// `f_inc_sic_acc` is set to **zero**, and that is a considered refusal rather
/// than an omission: PANAMA's `φ₂` is an in-service *loss* of the SiC layer to
/// thermal decomposition, while TRISO-ATOPS's `f_inc_sic` is a distinct
/// as-manufactured-population field. `φ₂` is already carried inside
/// `f_inc_acc` through
/// `boon_lay::fuel_failure::history::FailureProgress::in_service_failure_fraction`,
/// so routing it to `incremental_sic_accident` as well would both double-count
/// it and invent a correspondence neither code states.
#[must_use]
pub fn with_panama_accident_increment(
    base: AccidentFractions,
    increment: f64,
) -> AccidentFractions {
    AccidentFractions {
        incremental_accident: increment,
        incremental_sic_accident: 0.0,
        ..base
    }
}

/// The HTR-10 particle and pebble dimensions a release needs.
///
/// **Deliberately not a constructor with values in it.** The published HTR-10
/// dimensions live in `tampines::pebble_bed` (`TrisoParticle::htr10`,
/// `Pebble::htr10`), sourced to IAEA-TECDOC-1382 part 2 Table 4-17, and this
/// crate does not depend on `tampines`. A second copy of them here is exactly
/// the drift the workspace forbids, so the caller reads them from `tampines`
/// and passes them in. The example and this module's tests both do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Htr10Geometry {
    /// Fuel kernel radius `r` — HTR-10: 250 µm.
    pub kernel_radius: Length,
    /// SiC layer thickness `a_SiC` — HTR-10: 35 µm (380 → 415 µm).
    pub sic_thickness: Length,
    /// Matrix graphite diffusion thickness `a_graph` — HTR-10: the 5 mm
    /// unfuelled pebble shell.
    pub graphite_thickness: Length,
}

/// Assemble [`PlantParameters`] for an HTR-10 DLOFC.
///
/// - `coolant_pressure` is one atmosphere: this is a **depressurised**
///   accident, which is what the transient shape describes.
/// - `x_liftoff` is zero. `crate::accident::release::zero_pools` means the
///   accident starts with nothing plated out, so there is nothing to lift off
///   and any non-zero value here would be arithmetic on an empty pool. This
///   **under-predicts** the early release by whatever a real operating cycle
///   would have left in the circuit.
/// - `clean_up_fitted` is `true`: HTR-10 has a helium purification system.
#[must_use]
pub fn plant_parameters(
    geometry: Htr10Geometry,
    fractions: AccidentFractions,
) -> PlantParameters {
    PlantParameters {
        fractions,
        graphite_thickness: geometry.graphite_thickness,
        kernel_radius: geometry.kernel_radius,
        sic_thickness: geometry.sic_thickness,
        coolant_pressure: Pressure::new::<kilopascal>(101.325),
        x_liftoff: 0.0,
        clean_up_fitted: true,
    }
}

/// `T_B` as a `uom` temperature, for callers that do not want to reach for the
/// unit themselves.
#[must_use]
pub fn stand_in_irradiation_temperature() -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(STAND_IN_IRRADIATION_CELSIUS)
}

/// The in-service failure fraction PANAMA gives for an isothermal hold — the
/// form the report's own heating experiments have, and the form the
/// 1600 °C/200 h comparison in HTA-IB-03/90 page -504- is stated in.
///
/// Returns `(φ₁, φ₂, f_inc)` at the end of the hold.
#[must_use]
pub fn isothermal_failure(
    irradiation_temperature: ThermodynamicTemperature,
    accident: ThermodynamicTemperature,
    hold: Time,
    steps: usize,
) -> (f64, f64, f64) {
    let particle = panama_htr10::particle(irradiation_temperature);
    let phi_1_0 = panama_htr10::end_of_irradiation_failure(irradiation_temperature);
    let mut run = AccidentHistory::new(particle, phi_1_0);
    let end = run.run_isothermal(accident, hold, steps);
    (
        end.pressure_vessel.get::<ratio>(),
        end.thermal_decomposition.get::<ratio>(),
        end.in_service_failure_fraction().get::<ratio>(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accident::release::accident_release;
    use crate::inventory::{CoreInventory, NuclideInventory};
    use boon_lay::triso_atops_fork::nuclide_model::nuclide_database::find_nuclide;
    use changi::activity::inventory::htr10_equilibrium_core;
    use uom::si::radioactivity::becquerel;

    /// The sample count the example and every number below are taken at.
    const SAMPLES: usize = 201;

    fn t_b() -> ThermodynamicTemperature {
        stand_in_irradiation_temperature()
    }

    /// The published HTR-10 geometry, read from `tampines` rather than retyped.
    fn geometry() -> Htr10Geometry {
        let particle = tampines::pebble_bed::triso::TrisoParticle::htr10();
        let pebble = tampines::pebble_bed::pebble::Pebble::htr10();
        Htr10Geometry {
            kernel_radius: particle.kernel_radius,
            sic_thickness: particle.silicon_carbide_outer_radius
                - particle.inner_pyc_outer_radius,
            graphite_thickness: pebble.outer_radius - pebble.fuelled_zone_radius,
        }
    }

    /// The published inventory, restricted to what TRISO-ATOPS models.
    fn inventory() -> CoreInventory {
        let kept: Vec<NuclideInventory> = htr10_equilibrium_core()
            .into_iter()
            .filter(|e| find_nuclide(e.nuclide).is_some())
            .map(|e| NuclideInventory::uniform(e.nuclide, e.activity, 1))
            .collect();
        CoreInventory::new(kept, 1, 1)
    }

    /// **The geometry this case runs on is HTR-10's published geometry.**
    ///
    /// Methodology: read the three dimensions the release chain uses out of
    /// `tampines::pebble_bed` and compare them against the values
    /// IAEA-TECDOC-1382 part 2 Table 4-17 publishes, which
    /// `boon_lay::fuel_failure::htr10` independently carries as its own
    /// constants. Passing means the two crates agree and neither has drifted.
    ///
    /// Results (2026-09-24): kernel radius 250.0 µm, SiC thickness 35.0 µm
    /// (415 − 380), unfuelled graphite shell 5.00 mm (30 − 25 mm). All three
    /// match `boon_lay::fuel_failure::htr10`'s constants exactly.
    #[test]
    fn the_geometry_is_htr10s_and_the_two_crates_agree() {
        use boon_lay::fuel_failure::htr10 as panama;
        use uom::si::length::{micrometer, millimeter};
        let g = geometry();
        assert!(
            (g.kernel_radius.get::<micrometer>() - panama::KERNEL_RADIUS_UM).abs() < 1e-6,
            "kernel radius {} um against boon-lay's {}",
            g.kernel_radius.get::<micrometer>(),
            panama::KERNEL_RADIUS_UM
        );
        let sic_um = panama::SIC_OUTER_RADIUS_UM - panama::SIC_INNER_RADIUS_UM;
        assert!(
            (g.sic_thickness.get::<micrometer>() - sic_um).abs() < 1e-6,
            "SiC thickness {} um against boon-lay's {sic_um}",
            g.sic_thickness.get::<micrometer>()
        );
        assert!(
            (g.graphite_thickness.get::<millimeter>() - 5.0).abs() < 1e-9,
            "the unfuelled shell is the published 5 mm; got {} mm",
            g.graphite_thickness.get::<millimeter>()
        );
    }

    /// **The DLOFC shape reproduces the two figures the JRC volume states.**
    ///
    /// Methodology: evaluate [`DlofcShape::temperature_at`] at `t = 0`,
    /// `t = 30 h` and on the cooling leg, and check the rise against
    /// §9.9.2's "after 30 hours the maximum fuel temperature reaches 1 500 °C.
    /// After reaching the maximum the temperature decreases."
    ///
    /// Results (2026-09-24): 776.0 °C at `t = 0`, 1500.0 °C at 30 h,
    /// 1263.9 °C at 60 h and 935.3 °C at 200 h — monotone rising to the peak
    /// and monotone falling after it, as the source states.
    #[test]
    fn the_dlofc_shape_matches_the_published_rise() {
        let s = DlofcShape::htr_module_jrc();
        let c = |h: f64| s.temperature_at(Time::new::<hour>(h)).get::<degree_celsius>();
        assert!((c(0.0) - STAND_IN_IRRADIATION_CELSIUS).abs() < 1e-9);
        assert!(
            (c(DLOFC_TIME_TO_PEAK_HOURS) - DLOFC_PEAK_CELSIUS).abs() < 1e-9,
            "the peak must be the published 1500 C at 30 h; got {}",
            c(DLOFC_TIME_TO_PEAK_HOURS)
        );
        assert!(c(60.0) < c(30.0), "the temperature must fall after the peak");
        assert!(c(200.0) < c(60.0), "and keep falling");
        assert!(
            c(200.0) > crate::accident::release::DIFFUSION_FIT_MIN_CELSIUS,
            "the late leg must stay inside the fitted Arrhenius range, or the caveat \
             changes meaning; got {} C",
            c(200.0)
        );
    }

    /// **PANAMA over this transient agrees with HTA-IB-03/90's own published
    /// bound, and the flat 1600 °C hold does not — which is the point.**
    ///
    /// Methodology: HTA-IB-03/90 page -504- states that HTR-Module
    /// depressurised stays below `10⁻⁶` at 200 h. Run PANAMA over the DLOFC
    /// transient for 200 h and compare; then run a *flat* 200 h hold at the
    /// 1600 °C limit and compare that too. The two arms answer different
    /// questions and the test asserts both answers.
    ///
    /// Results (2026-09-24), HTR-10 particle, `T_B = 776 °C`:
    ///
    /// | arm | `f_inc` at 200 h | against the `10⁻⁶` bound |
    /// |---|---|---|
    /// | DLOFC transient, peak 1500 °C at 30 h | **1.068·10⁻⁷** | consistent, 9.4× below |
    /// | flat 200 h at 1600 °C | **3.103·10⁻⁵** | 31× above |
    ///
    /// The flat hold exceeding the bound is not a disagreement: it holds the
    /// fuel at its accident limit for the entire window, which no transient
    /// does. The transient arm is the one the report's sentence is about.
    ///
    /// **This is the nearest thing to a check the literature supports and it is
    /// NOT a validation** — it is HTR-Module's bound against HTR-Module's
    /// transient, evaluated with HTR-10's geometry and burnup, and PANAMA was
    /// validated on German TRISO over 1600–2500 °C so both arms are
    /// extrapolations at or below 1600 °C.
    #[test]
    fn the_transient_is_consistent_with_the_reports_own_bound() {
        let p = panama_over_transient(&DlofcShape::htr_module_jrc(), t_b(), SAMPLES);
        let f_200 = p.in_service[SAMPLES - 1];
        assert!(
            f_200 < 1.0e-6,
            "HTA-IB-03/90 p-504- puts HTR-Module depressurised below 1e-6 at 200 h; \
             this transient gives {f_200:.4e}"
        );
        assert!(
            (f_200 - 1.068e-7).abs() / 1.068e-7 < 0.02,
            "the recorded value is 1.068e-7; got {f_200:.4e}"
        );

        let (_, _, flat) = isothermal_failure(
            t_b(),
            ThermodynamicTemperature::new::<degree_celsius>(1600.0),
            Time::new::<hour>(REPORTING_WINDOW_HOURS),
            200,
        );
        assert!(
            flat > 1.0e-6,
            "the flat 1600 C hold must NOT be mistaken for the transient: it is expected \
             to exceed the bound, and got {flat:.4e}"
        );
        assert!(
            (flat - 3.103e-5).abs() / 3.103e-5 < 0.02,
            "the recorded flat-hold value is 3.103e-5; got {flat:.4e}"
        );
    }

    /// **A convergence study on the time grid, with the grid error stated
    /// rather than assumed away.**
    ///
    /// Methodology: PANAMA's driver steps between samples at the interval's
    /// *midpoint* temperature, so the answer is grid-dependent and the only
    /// honest thing to do is measure the dependence. Evaluate `f_inc` at 200 h
    /// on a doubling sequence and check (i) that successive differences fall by
    /// the factor ~4 that second-order convergence requires, and (ii) that the
    /// reported grid sits within a stated distance of the extrapolated limit.
    ///
    /// **Results (2026-09-24), HTR-Module DLOFC shape, `T_B = 776 °C`:**
    ///
    /// | samples | step | `f_inc` at 200 h | Δ from previous |
    /// |---|---|---|---|
    /// | samples | step | `f_inc` at 200 h | Δ from previous | Δ ratio |
    /// |---|---|---|---|---|
    /// | 51 | 4 h | 9.005579·10⁻⁸ | — | — |
    /// | 101 | 2 h | 1.0553258·10⁻⁷ | +1.5477·10⁻⁸ | — |
    /// | **201** | **1 h** | **1.0676990·10⁻⁷** | +1.2373·10⁻⁹ | **12.51** |
    /// | 401 | 30 min | 1.0708605·10⁻⁷ | +3.1615·10⁻¹⁰ | 3.914 |
    /// | 801 | 15 min | 1.0716552·10⁻⁷ | +7.947·10⁻¹¹ | 3.979 |
    /// | 1601 | 7.5 min | 1.0718541·10⁻⁷ | +1.990·10⁻¹¹ | 3.994 |
    ///
    /// **The asymptotic second-order regime starts at the 2 h step, not before.**
    /// The last three ratios are 3.914, 3.979 and 3.994, converging on the 4
    /// that second order in Δt requires, while the 4 h → 2 h step gives
    /// **12.51** and is plainly outside it. That is in the table rather than
    /// trimmed out of it: a convergence study that shows only the well-behaved
    /// end of its own sequence is not a study.
    ///
    /// Richardson extrapolation on the finest pair, for a second-order scheme,
    /// gives a limit of **1.071920·10⁻⁷**.
    ///
    /// **So the 201-sample grid used everywhere in this module and its example
    /// under-states `f_inc` by 0.39 %, and that is stated rather than hidden.**
    /// It changes no conclusion: the quantity is `10⁻⁷` against an
    /// as-manufactured `1.23·10⁻⁴`, so a 0.4 % grid error is three orders of
    /// magnitude below the term it would have to move. The 51-sample grid is
    /// **16 % low** and would be a different matter; 201 was chosen because
    /// this study says it is enough, not the other way round.
    #[test]
    fn the_time_grid_converges_at_second_order_and_the_error_is_stated() {
        let shape = DlofcShape::htr_module_jrc();
        let f = |n: usize| {
            let p = panama_over_transient(&shape, t_b(), n);
            p.in_service[n - 1]
        };
        let series: Vec<f64> = [51usize, 101, 201, 401, 801, 1601].iter().map(|n| f(*n)).collect();
        let deltas: Vec<f64> = series.windows(2).map(|w| w[1] - w[0]).collect();
        for d in &deltas {
            assert!(*d > 0.0, "the sequence must be monotone; deltas {deltas:?}");
        }
        let ratios: Vec<f64> = deltas.windows(2).map(|w| w[0] / w[1]).collect();
        // The COARSEST step is NOT in the asymptotic range, and the study says so
        // rather than starting where the answer is convenient.
        assert!(
            ratios[0] > 6.0,
            "the 4 h -> 2 h step is recorded as outside the asymptotic range (ratio 12.51); got {:.3}, which would mean the table is wrong",
            ratios[0]
        );
        // Second order means each further halving of the step quarters the error.
        for (i, r) in ratios.iter().enumerate().skip(1) {
            assert!(
                (3.5..4.5).contains(r),
                "step {i}: successive differences fall by {r:.3}, not the ~4 that second-order convergence requires; series {series:?}"
            );
        }
        assert!(
            ratios[ratios.len() - 1] > ratios[1],
            "the ratios must be converging ON 4, not wandering; {ratios:?}"
        );
        // Richardson on the finest pair, for a second-order scheme.
        let n = series.len();
        let limit = series[n - 1] + (series[n - 1] - series[n - 2]) / 3.0;
        assert!(
            (limit - 1.071920e-7).abs() / 1.071920e-7 < 1.0e-3,
            "the recorded extrapolated limit is 1.071920e-7; got {limit:.7e}"
        );
        // And the grid this module reports on, against that limit.
        let reported = series[2]; // 201 samples
        let grid_error = (limit - reported) / limit;
        assert!(
            (0.0035..0.0045).contains(&grid_error),
            "the 201-sample grid error is recorded as 0.39 %; got {:.3} %",
            100.0 * grid_error
        );
        let coarse_error = (limit - series[0]) / limit;
        assert!(
            (0.15..0.17).contains(&coarse_error),
            "the 51-sample grid error is recorded as 16 %; got {:.1} %",
            100.0 * coarse_error
        );
    }

    /// **The seam is wired and it is small — the ablation is the control.**
    ///
    /// Methodology: run the whole release chain twice on the HTR-10 DLOFC
    /// case, identical except that `f_inc_acc` is PANAMA's computed value in
    /// one arm and **zero** in the other. If the two totals were equal the
    /// seam would be doing nothing, and the assertion catches that.
    ///
    /// Results (2026-09-24), 19 of the 22 published nuclides (H-3, Xe-135m and
    /// Rb-88 are not in TRISO-ATOPS's table), NP-MHTGR normal-operation
    /// fractions, one node, 200 h:
    ///
    /// | arm | total released |
    /// |---|---|
    /// | PANAMA on (`f_inc_acc = 1.068·10⁻⁷`) | **1.8762·10¹¹ Bq** |
    /// | PANAMA ablated (`f_inc_acc = 0`) | **1.8747·10¹¹ Bq** |
    ///
    /// Ratio **1.0008** — the accident-added failure raises the source term by
    /// **0.08 %** at a 1500 °C peak, because it is `10⁻⁷` against an
    /// as-manufactured `f_hm + f_inc` of `1.23·10⁻⁴`. **The finding is that the
    /// seam is negligible here, not that it is broken**: PANAMA only overtakes
    /// the as-manufactured population above about 1800 °C, which is beyond the
    /// design limit.
    ///
    /// **NOT a source term for HTR-10.** See the module docs for the three
    /// separate reasons.
    #[test]
    fn the_panama_seam_changes_the_release_and_the_ablation_proves_it() {
        let shape = DlofcShape::htr_module_jrc();
        let p = panama_over_transient(&shape, t_b(), SAMPLES);
        let transient = shape.transient(SAMPLES, 1, 1).expect("enough samples");
        let inv = inventory();

        let total = |increment: f64| {
            let fractions = with_panama_accident_increment(
                np_mhtgr_normal_operation_fractions(),
                increment,
            );
            let plant = plant_parameters(geometry(), fractions);
            let out = accident_release(&inv, &transient, &plant).expect("the chain runs");
            out.source_term
                .nuclides
                .iter()
                .map(|n| n.total_released().get::<becquerel>())
                .sum::<f64>()
        };

        let on = total(p.accident_increment_final);
        let off = total(0.0);
        assert!(
            on > off,
            "the seam must move the answer; got {on:.5e} with PANAMA and {off:.5e} without"
        );
        let ablation_ratio = on / off;
        assert!(
            (ablation_ratio - 1.0008).abs() < 5.0e-4,
            "the recorded ablation ratio is 1.0008; got {ablation_ratio:.6}"
        );
        assert!(
            (on - 1.8762e11).abs() / 1.8762e11 < 0.02,
            "the recorded total is 1.8762e11 Bq; got {on:.5e}"
        );
    }

    /// **PANAMA's number must NOT be routed to `incremental`, and the seam does
    /// not touch the three as-manufactured fractions.**
    ///
    /// Methodology: apply [`with_panama_accident_increment`] to the NP-MHTGR
    /// set and check field by field that only `incremental_accident` moved.
    /// This is the structural guarantee behind the module's claim that the
    /// as-manufactured populations stay the caller's.
    ///
    /// Result (2026-09-24): `f_hm`, `f_sic`, `f_inc` and `f_inc_sic` are
    /// unchanged; `f_inc_acc` carries the PANAMA value; `f_inc_sic_acc` is
    /// zero, deliberately (see the function docs).
    #[test]
    fn the_seam_sets_exactly_one_field() {
        let base = np_mhtgr_normal_operation_fractions();
        let wired = with_panama_accident_increment(base, 1.234e-6);
        assert_eq!(wired.heavy_metal, base.heavy_metal);
        assert_eq!(wired.sic, base.sic);
        assert_eq!(wired.incremental, base.incremental);
        assert_eq!(wired.incremental_sic, base.incremental_sic);
        assert_eq!(wired.incremental_accident, 1.234e-6);
        assert_eq!(
            wired.incremental_sic_accident, 0.0,
            "phi_2 is already inside f_inc_acc; routing it here would double-count it"
        );
    }

    /// **The two ESTIMATEd cooldown parameters cannot move the released
    /// activity**, which is what makes it honest to report a release at all
    /// while they are uncited.
    ///
    /// Methodology: TRISO-ATOPS's `coolant_release` selects only samples with
    /// `dT/dt >= 0`, so the cooling leg never vents. Run the release with the
    /// cooldown time constant at 30 h, 60 h and 120 h and with the late-time
    /// temperature at 800 °C and 1000 °C, and compare the totals.
    ///
    /// Results (2026-09-24): every arm gives **1.8762·10¹¹ Bq**, identical to
    /// the last printed digit. The estimates move PANAMA's failure
    /// accumulation only, and that term is itself 0.08 % of the answer.
    #[test]
    fn the_estimated_cooldown_parameters_cannot_move_the_release() {
        let inv = inventory();
        let mut totals = Vec::new();
        for (tau_h, late_c) in [(30.0, 900.0), (60.0, 900.0), (120.0, 900.0), (60.0, 800.0), (60.0, 1000.0)] {
            let mut shape = DlofcShape::htr_module_jrc();
            shape.cooldown_time_constant = Time::new::<hour>(tau_h);
            shape.late_time = ThermodynamicTemperature::new::<degree_celsius>(late_c);
            let p = panama_over_transient(&shape, t_b(), SAMPLES);
            let fractions = with_panama_accident_increment(
                np_mhtgr_normal_operation_fractions(),
                p.accident_increment_final,
            );
            let plant = plant_parameters(geometry(), fractions);
            let transient = shape.transient(SAMPLES, 1, 1).expect("enough samples");
            let out = accident_release(&inv, &transient, &plant).expect("the chain runs");
            totals.push(
                out.source_term
                    .nuclides
                    .iter()
                    .map(|n| n.total_released().get::<becquerel>())
                    .sum::<f64>(),
            );
        }
        let first = totals[0];
        for (i, t) in totals.iter().enumerate() {
            assert!(
                (t - first).abs() / first < 1.0e-3,
                "arm {i} gives {t:.6e} against arm 0's {first:.6e}; the cooldown estimates \
                 are supposed to be unable to change the vented release"
            );
        }
    }

    /// **`φ₂` contributes nothing at any temperature this case reaches**, and
    /// takes over only where the report says it does.
    ///
    /// Methodology: evaluate the isothermal 200 h hold across 1200–2200 °C and
    /// find where `φ₂` overtakes `φ₁`. HTA-IB-03/90 page -508- states that
    /// thermal decomposition governs above about 2000 °C.
    ///
    /// Results (2026-09-24): `φ₂` is `3.4·10⁻¹⁵` at 1600 °C and `2.78·10⁻⁴` at
    /// 2000 °C against `φ₁ = 4.79·10⁻³`, and **overtakes `φ₁` between 2000 and
    /// 2200 °C** (0.977 against 0.0607) — matching the report's statement.
    #[test]
    fn thermal_decomposition_takes_over_where_the_report_says_it_does() {
        let hold = Time::new::<hour>(REPORTING_WINDOW_HOURS);
        let at = |c: f64| {
            isothermal_failure(
                t_b(),
                ThermodynamicTemperature::new::<degree_celsius>(c),
                hold,
                200,
            )
        };
        let (phi1_1600, phi2_1600, _) = at(1600.0);
        assert!(
            phi2_1600 < 1.0e-12 && phi2_1600 < phi1_1600,
            "at the 1600 C limit decomposition must be negligible; phi_2 = {phi2_1600:.3e}"
        );
        let (phi1_2000, phi2_2000, _) = at(2000.0);
        assert!(
            phi2_2000 < phi1_2000,
            "at 2000 C the pressure vessel must still lead; {phi2_2000:.3e} vs {phi1_2000:.3e}"
        );
        let (phi1_2200, phi2_2200, _) = at(2200.0);
        assert!(
            phi2_2200 > phi1_2200,
            "by 2200 C decomposition must lead (page -508-); {phi2_2200:.3e} vs {phi1_2200:.3e}"
        );
    }

    /// **Which nuclides carry floored negative release windows — measured per
    /// nuclide, not inferred from one flag over the whole run.**
    ///
    /// Methodology: `accident_release` returns a single
    /// [`crate::error::Caveats::negative_atom_count_seen`] for the entire run,
    /// so "which nuclide caused it" cannot be read off a combined result. Run
    /// the chain **once per nuclide**, on a one-nuclide inventory, and record
    /// the flag for each. With
    /// [`crate::accident::release::zero_pools`] the atom-count path cannot fire
    /// (every subtracted term is zero), so a flag here is necessarily the
    /// per-window first difference going negative — i.e. a non-monotonic
    /// cumulative release, which is **floored** and therefore **over-states**
    /// that nuclide's total.
    ///
    /// **Result (2026-09-24): exactly one of the 12 reported nuclides —
    /// `Ag-110m`.** Kr-85, Xe-131m, Xe-133, Xe-133m, Xe-135, I-131, I-133,
    /// Sr-89, Sr-90, Cs-134 and Cs-137 are all clean.
    ///
    /// That is the model's structure rather than an accident of this
    /// transient. Silver is the only nuclide routed through
    /// `breakthrough_model_transient`, whose `−a/(2r)` time-lag term drives the
    /// release fraction negative — where it is clamped to zero — until
    /// breakthrough, so only silver's cumulative curie series can fall. The
    /// measured size of the over-statement is a factor **1.0011** (13 of 30
    /// windows; windows sum `2.503345e-2 Ci` against a cumulative endpoint of
    /// `2.500569e-2 Ci`).
    ///
    /// **This test exists because the claim was originally made after checking
    /// four nuclides and generalising.** Four is not twelve, and a structural
    /// argument that has never been able to fail is not evidence. GitHub #300.
    #[test]
    fn only_silver_carries_floored_negative_windows() {
        let shape = DlofcShape::htr_module_jrc();
        let transient = shape.transient(SAMPLES, 1, 1).expect("enough samples");
        let p = panama_over_transient(&shape, t_b(), SAMPLES);
        let plant = plant_parameters(
            geometry(),
            with_panama_accident_increment(
                np_mhtgr_normal_operation_fractions(),
                p.accident_increment_final,
            ),
        );

        let mut flagged = Vec::new();
        let mut clean = Vec::new();
        for entry in htr10_equilibrium_core() {
            if find_nuclide(entry.nuclide).is_none() {
                continue;
            }
            let one = CoreInventory::new(
                vec![NuclideInventory::uniform(entry.nuclide, entry.activity, 1)],
                1,
                1,
            );
            // A nuclide screened out by the half-life test yields no release at
            // all, so it is not evidence either way and is skipped.
            let Ok(out) = accident_release(&one, &transient, &plant) else {
                continue;
            };
            if !out.screened_out.is_empty() {
                continue;
            }
            if out.caveats.negative_atom_count_seen {
                flagged.push(entry.nuclide);
            } else {
                clean.push(entry.nuclide);
            }
        }

        assert_eq!(
            flagged,
            vec!["Ag-110m"],
            "recorded: silver alone goes negative, because it is the only nuclide routed \
             through the breakthrough model. Clean: {clean:?}"
        );
        assert!(
            clean.len() >= 10,
            "the control must actually cover the other nuclides, not skip them; only \
             {} were exercised: {clean:?}",
            clean.len()
        );
    }

    /// **Three of the 22 published nuclides are not modelled, and they are not
    /// silently zero.**
    ///
    /// Methodology: intersect `changi`'s published HTR-10 inventory with
    /// TRISO-ATOPS's 84-nuclide table.
    ///
    /// Result (2026-09-24): **19 of 22** survive. H-3 (`3.81·10¹²` Bq),
    /// Xe-135m (`2.64·10¹⁵` Bq) and Rb-88 (`1.03·10¹⁶` Bq) are absent from
    /// TRISO-ATOPS. Rb-88 and Xe-135m are among the larger entries in the
    /// table, so their absence is a real gap in coverage rather than a rounding
    /// matter, and any total computed here is a total over the 19 — never over
    /// the core.
    #[test]
    fn the_unmodelled_nuclides_are_named_rather_than_dropped_silently() {
        let published = htr10_equilibrium_core();
        assert_eq!(published.len(), 22, "Liu and Cao Table 1");
        let missing: Vec<&str> = published
            .iter()
            .filter(|e| find_nuclide(e.nuclide).is_none())
            .map(|e| e.nuclide)
            .collect();
        assert_eq!(
            missing,
            vec!["H-3", "Xe-135m", "Rb-88"],
            "the unmodelled set is recorded so a change to either table is visible"
        );
        assert_eq!(inventory().nuclides.len(), 19);
    }
}
