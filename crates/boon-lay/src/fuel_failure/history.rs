// SPDX-License-Identifier: GPL-3.0
//
// PANAMA-I reimplementation — provenance
// --------------------------------------
// Reference : Verfondern, K. & Nabielek, H., "The Mathematical Basis of the
//             PANAMA-I Code for Modeling Pressure Vessel Failure of TRISO
//             Coated Particles under Accident Conditions",
//             Forschungszentrum Jülich, HTA-IB-03/90, 1 August 1990.
//             Reprinted as Appendix C, printed pages -479- to -511-.
// Status    : the report is restricted literature with no reuse licence. Only
//             the governing EQUATIONS and their constants are reproduced here,
//             with citation, as scientific facts. No prose, figure or page of
//             that document is copied into this repository, and the PDF is not
//             tracked here. See DATA_POLICY.md.
// Nature    : an independent Rust implementation of the published model, not a
//             port of the PANAMA Fortran (which is closed-source and was never
//             consulted).

//! **The time-stepping driver** — §3.1, pages -482- and -483-.
//!
//! Everything else in [`super`] is one equation. This is the assembly: it
//! walks a temperature history interval by interval, carries the three
//! history variables forward, and reports `φ₁`, `φ₂`, `φ_total` and the
//! three rates `φ̇` (page -511-, all `s⁻¹`) at every step.
//!
//! ```text
//! per interval [t₁, t₂] at mean temperature T_m:
//!
//!   FKOR(t₂) = FKOR(t₁) + v̇(T_m)·Δt/d_o                       (p-492)
//!   ζ(t₂)    = ζ(t₁)    + k(T_m)·Δt                            (11)
//!
//!   φ₁ ← φ₁ + max(0, φ₁(t₂,T_m) − φ₁(t₁,T_m))                  (p-482)
//!   φ₂  = 1 − exp(−α·ζ(t₂)^β)                                  (13)
//!   φ_total = 1 − (1−φ_o)(1−φ₁)(1−φ₂)                          (p-480)
//! ```
//!
//! # The two mechanisms are stepped differently, and that is the report's
//!
//! `φ₁` is **accumulated from positive increments**: page -482- forms
//! `φ₁(t₂,T_m) − φ₁(t₁,T_m)` — both ends evaluated at *this* interval's mean
//! temperature — and adds it to the running total only if it is positive. So
//! `φ₁` is monotone by construction: a particle that has burst does not
//! un-burst when the transient cools.
//!
//! `φ₂` is **not** accumulated. Page -483- is explicit: `ζ` carries the whole
//! history, and `φ₂(t₂)` is read straight off Eq (13) at that `ζ`. Only the
//! *rate* `φ̇₂` is formed by differencing.
//!
//! Getting this backwards is invisible on a monotone heat-up — the two agree
//! exactly there — and wrong on anything that cools, which is every reactor
//! transient the report goes on to compute (Figs. 10–13).
//! [`tests::the_two_mechanisms_are_stepped_differently`] pins the
//! distinction on a history that cools.
//!
//! # Verification — methodology and results
//!
//! ## 1. Step-size independence at constant temperature (the report's own claim)
//!
//! Page -482-: "At a constant temperature, the length of the time interval
//! does not influence the computed result." That is a falsifiable statement
//! about *this algorithm*, independent of any figure, and it is the sharpest
//! check available on the stepping itself — a driver that recomputed `FKOR`
//! or `ζ` from total elapsed time, or that accumulated `φ₂`, would fail it.
//!
//! **Result, 2026-09-24:** 300 h at 1600 °C taken in 1, 12, 300 and 3000
//! steps gives `φ_total` agreeing to **2·10⁻¹² relative** (`FKOR` and `ζ` to
//! better than 10⁻¹²; the extra digit is the `f(τ)` sum). Pinned by
//! [`tests::the_step_length_does_not_matter_at_constant_temperature`].
//!
//! ## 2. Fig. 6 — the first end-to-end check on the whole chain
//!
//! Fig. 6 (page -500-) is PANAMA's own output for eight SiC varieties at
//! 1600 °C over ~250 h, and it is the only figure in the report that exercises
//! Eqs (1), (8a) and (9a) across a *family* of particles. Page -498- states
//! its basis explicitly: the Table 1 "after irradiation" values, computed at
//! `T_B = 1000 °C` and `Γ = 1·10²⁵ m⁻² EDN`. Its caption states the kernel
//! ((Th,U)O₂) and the temperature and nothing else — no geometry, no `V_k`,
//! no `V_f`, no `F_b`, no `t_B` — so the **absolute** failure fraction cannot
//! be reproduced without inventing four inputs, and inventing them is exactly
//! the tuning this workspace forbids.
//!
//! What the figure *can* verify, with no invented input at all, is the
//! consequence of `σ_t(t)` being **common to all eight curves**. Only `σ_o`
//! and `m` differ between varieties, so inverting Eq (1) on each digitised
//! curve,
//!
//! ```text
//! σ_t^(i)(t) = σ_o,i · ( −ln(1 − φ_i(t)) / ln2 )^(1/m_i)
//! ```
//!
//! must return the **same** `σ_t(t)` from all eight. That is an eight-fold
//! over-determined test of Eq (1) together with Eqs (8a)/(9a).
//!
//! **Results, 2026-09-24** (467 digitised points, maintainer; 121 of them lie
//! below the figure's plotted 10⁻⁶ floor and are excluded from the inversion):
//!
//! | quantity | measured |
//! |---|---|
//! | rank order of the eight curves | **8/8 reproduced** |
//! | spread top-to-bottom at 248 h | 4.79 decades |
//! | recovered common `σ_t` | 132 MPa at 130 h → 163 MPa at 248 h |
//! | relative s.d. of `σ_t` across varieties | **9.0 % (130 h) … 11.2 % (248 h)** |
//! | per-curve residual in `log₁₀ φ` at one common `σ_t` | **−0.37 … +0.39**, mean \|·\| **0.23** |
//!
//! Reproducing the ordering and the 4.8-decade spread of eight curves from
//! one common stress, to ±0.4 decades, is a real success for Eq (1) and the
//! Table 1 degradation. But the residual is **systematic, not random**: it
//! runs monotonically with `m_oo`, from −0.37 decades at `m_oo = 5.0` to
//! +0.39 at `m_oo = 8.5`. Two hypotheses were tested and neither removes it:
//!
//! - **Fluence.** Removing the Eq (8a)/(9a) degradation entirely (`Γ → 0`)
//!   halves the scatter, from 10.1 % to 5.4 % relative s.d. But page -498-
//!   states Fig. 6's basis is `Γ = 1·10²⁵`, so this is a *disagreement with
//!   the figure*, not a licence to change the input — and `Γ` was not
//!   changed.
//! - **The plotted floor.** Restricting to points inside the figure's own
//!   10⁻⁶ … 10 axis leaves the trend intact (9.0 → 11.2 % against
//!   9.3 → 11.3 % unrestricted).
//!
//! Recorded with numbers in `docs/panama-i-units-and-open-questions.md`.
//! Pinned by [`tests::figure_6_recovers_one_common_stress_history`].
//!
//! ## 3. Fig. 7 — the staged history, and the first VALIDATION case
//!
//! Fig. 7 (page -501-) is the FRJ2-K11/03 heating experiment: **measured**
//! ⁸⁵Kr release alongside two PANAMA curves. Unlike every check above it has
//! a complete stated input set — `σ_oo = 600 MPa`, `m_oo = 6`,
//! `T_B = 1160 °C`, `t_B = 260 FPD`, `F_B = 0.09 FIMA`,
//! `Γ = 0.05·10²⁵ m⁻² EDN`, `η̇(T) ≡ 0` — and page -498- states the staging
//! outright: **100 h at 1400 °C, then 100 h at 1500 °C, then 1600 °C** to
//! 1000 h. Nothing here was inferred from the curve's slope.
//!
//! ### The load-bearing assumption being inherited
//!
//! PANAMA computes a particle **failure** fraction; Fig. 7 plots a ⁸⁵Kr
//! **release** fraction, and the report puts them on one axis. That equates
//! the two — a failed particle releases its whole krypton inventory. **This
//! is an assumption inherited from the report, not derived here.** If a
//! comparison matches in shape but sits at a constant offset, it is the first
//! suspect.
//!
//! ### Code-to-code: does this implementation reproduce PANAMA's own curve?
//!
//! The absolute level still needs the unstated geometry, so the comparison is
//! made on `σ_t` recovered from the `Without Grain Boundary Corrosion` curve
//! (the right comparator, since `η̇ ≡ 0`) against the chain's own `σ_t`, with
//! **one** free scale — the geometry aggregate `r/(2·d_o·(V_f/V_k))`. No
//! physics constant is adjusted.
//!
//! **Results, 2026-09-24** (71 digitised points):
//!
//! | window | relative s.d. of `σ_t^PANAMA / σ_t^chain` | max/min |
//! |---|---|---|
//! | **0–300 h, all three stages** | **4.9 %** | 1.20 |
//! | 300–1000 h | 15.4 % | 1.77 |
//! | whole run | 21.9 % | 2.17 |
//!
//! Per stage the ratio is 0.1561 (1400 °C), 0.1473 (1500 °C), 0.1608
//! (1600 °C, first 100 h) — within ±4.5 % of each other. **The staging is
//! reproduced**: the temperature dependence entering through Eq (5c)'s `OPF`,
//! `D_S(T)`, `v̇(T)` and Eq (3)'s explicit `T` all land together across two
//! step changes.
//!
//! **Then it drifts, and that is the finding.** By 977 h PANAMA's implied
//! `σ_t` is 282 MPa; with the single scale fixed over 0–300 h the chain
//! predicts 148 MPa — a factor **1.90**.
//! Diagnosis: PANAMA's curve follows `φ ∝ t^3.21` at late times, i.e.
//! `σ_t ∝ t^0.54`, whereas in the chain `F_d` has saturated (0.980 at 296 h,
//! 0.9999 at 977 h) and `OPF` is constant at fixed temperature, leaving only
//! `FKOR` — which rises **4 %** over the last 700 h. Something in PANAMA
//! keeps the pressure climbing as `√t` after the Booth release is over, and
//! the printed equations do not say what. Recorded as an open item with these
//! numbers in `docs/panama-i-units-and-open-questions.md`;
//! [`tests::figure_7_reproduces_the_staged_history_then_drifts`] pins both
//! halves so neither can be lost.
//!
//! ### Code-to-data: does PANAMA reproduce the experiment?
//!
//! This part needs no geometry — it compares the nine measured points against
//! the report's own two curves.
//!
//! | comparator | mean residual, `log₁₀` | mean \|·\| | worst |
//! |---|---|---|---|
//! | `Without Grain Boundary Corrosion` (`η̇ ≡ 0`, the caption's case) | **+0.51** | 0.52 | +1.14 |
//! | `With Grain Boundary Corrosion` | −1.49 | 1.49 | −1.85 |
//!
//! So at 1400–1600 °C PANAMA **under**-predicts FRJ2-K11/03 by half a decade
//! with grain-boundary corrosion off, and over-predicts by 1.5 decades with
//! it on; the measurement lies between the two, nearer the "without" curve.
//! That reproduces page -499-'s own reading — that the with-corrosion model
//! "covers the measured values in a conservative approximation" — and it is a
//! statement about **PANAMA**, not about this implementation.
//! [`tests::figure_7_brackets_the_measurement`] records it.
//!
//! One digitisation label reads `90% FIMA` where the caption says
//! `9.0 % FIMA` and `F_B = 0.09`; 0.09 is used, and the slip is noted.

use uom::si::f64::{Frequency, Pressure, Ratio, ThermodynamicTemperature, Time, Volume};
use uom::si::frequency::hertz;
use uom::si::ratio::ratio;
use uom::si::time::second;

use super::booth::{dimensionless_time, released_gas_fraction};
use super::corrosion::advance_thinning_factor;
use super::decomposition::{
    advance_action_integral, thermal_decomposition_failure_fraction, DecompositionCalibration,
};
use super::diffusion::{reduced_diffusion_coefficient, KernelKind};
use super::geometry::SicLayer;
use super::grain_boundary::{
    advance_grain_boundary_exposure, corroded_weibull_modulus, GrainBoundaryCorrosion,
};
use super::molar_volume::{molar_volume, KernelCompound};
use super::oxygen::{
    oxygen_per_fission_thoria, oxygen_per_fission_uco, oxygen_per_fission_uo2, HeatingRegime,
};
use super::pressure::internal_gas_pressure;
use super::stress::induced_stress_with_thinning_factor;
use super::weibull::weibull_failure_fraction;
use super::{total_failure_fraction, FailureFraction};

/// Where the step's `OPF` comes from — Eqs (5a)–(5e).
///
/// An enum rather than a callback, per the workspace's no-trait-objects rule,
/// and because the report's own set of sources is closed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OxygenSource {
    /// **Eq (5d)** — `UCO`: no oxygen is produced.
    Uco,
    /// **Eq (5a)** — `(Th,U)O₂`, from the accident temperature, the
    /// thorium/²³⁵U ratio `N` and the burnup.
    ThoriumUraniumOxide {
        /// `N`, the thorium/uranium-235 ratio (5 for AVR, 10 for THTR).
        thorium_to_u235: f64,
        /// `F_b`, heavy-metal burnup in FIMA as a fraction.
        burnup: Ratio,
    },
    /// **Eq (5c)** — `UO₂` during heating, from the irradiation history.
    UraniumOxide {
        /// `T_B`, the particle surface temperature during irradiation.
        irradiation_temperature: ThermodynamicTemperature,
        /// `t_B`, the irradiation time. Enters Eq (5b) in **seconds**.
        irradiation_time: Time,
    },
    /// A fixed `OPF` supplied by the caller.
    ///
    /// For the cases where the report states the value rather than the
    /// correlation, and for ablating the oxygen term to see what it carries.
    Fixed(Ratio),
}

impl OxygenSource {
    /// The `OPF` for this step's accident temperature.
    pub fn oxygen_per_fission(self, accident_temperature: ThermodynamicTemperature) -> Ratio {
        match self {
            OxygenSource::Uco => oxygen_per_fission_uco(),
            OxygenSource::ThoriumUraniumOxide {
                thorium_to_u235,
                burnup,
            } => oxygen_per_fission_thoria(accident_temperature, thorium_to_u235, burnup),
            OxygenSource::UraniumOxide {
                irradiation_temperature,
                irradiation_time,
            } => oxygen_per_fission_uo2(
                irradiation_temperature,
                irradiation_time,
                HeatingRegime::DuringHeating {
                    temperature: accident_temperature,
                },
            ),
            OxygenSource::Fixed(opf) => opf,
        }
    }
}

/// Everything about the particle that does not change during the accident.
///
/// All of it is an **input**. The report's figures state some of these and
/// not others; where a figure does not state one, this crate takes it from
/// the caller rather than inventing a value — see the module docs on Fig. 6.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParticleState {
    /// The SiC layer geometry — `r`, `d_o`.
    pub layer: SicLayer,
    /// Which compound the kernel is, for Eqs (6a)–(6c).
    pub compound: KernelCompound,
    /// Which `D_S` correlation applies (page -487- uses the `UO₂` relation
    /// for `UCO` as well, which is why this is a separate choice from
    /// [`ParticleState::compound`]).
    pub diffusion_kernel: KernelKind,
    /// `V_k`, the kernel volume.
    pub kernel_volume: Volume,
    /// `V_f`, the void volume in the buffer used as free volume (the report
    /// takes 50 % of the buffer volume).
    pub free_volume: Volume,
    /// `F_b`, heavy-metal burnup in FIMA as a fraction. Feeds both Eq (3) and
    /// the `(Th,U)O₂` `D_S` correlation.
    pub burnup: Ratio,
    /// `F_f`, the stable fission-gas yield. The report's value is
    /// [`super::STABLE_FISSION_GAS_YIELD`] = 0.31.
    pub stable_gas_yield: Ratio,
    /// `τ_i = D_S(T_B)·t_B`, the dimensionless irradiation time (page -486-).
    ///
    /// Supplied rather than derived, because it needs `T_B` *and* `t_B` and
    /// the report's figures generally give neither. [`irradiation_tau`] builds
    /// it when they are known.
    pub dimensionless_irradiation_time: Ratio,
    /// `σ_o`, the SiC median strength at the end of irradiation — Eq (8a),
    /// [`super::irradiated_strength`].
    pub median_strength: Pressure,
    /// `m_o`, the Weibull modulus at the end of irradiation — Eq (9a),
    /// [`super::irradiated_weibull_modulus`].
    pub weibull_modulus: f64,
    /// Where `OPF` comes from.
    pub oxygen: OxygenSource,
    /// Which fit of Eq (13) applies — Eq (14a) or (14b).
    pub decomposition: DecompositionCalibration,
    /// Whether Eqs (10b)/(10c) are applied. **Off by default**, as in the
    /// report (page -495-).
    pub grain_boundary: GrainBoundaryCorrosion,
    /// `φ_o`, the as-manufactured defective fraction. The report's own runs
    /// use zero; see [`super::AS_MANUFACTURED_TARGET`].
    pub as_manufactured: FailureFraction,
}

/// `τ_i = D_S(T_B)·t_B` (page -486-), for the cases where `T_B` and `t_B` are
/// both known.
pub fn irradiation_tau(
    kernel: KernelKind,
    irradiation_temperature: ThermodynamicTemperature,
    irradiation_time: Time,
    burnup: Ratio,
) -> Ratio {
    dimensionless_time(
        reduced_diffusion_coefficient(kernel, irradiation_temperature, burnup),
        irradiation_time,
    )
}

/// One interval of the accident history.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AccidentStep {
    /// `t₂ − t₁`.
    pub duration: Time,
    /// `T_m`, the mean temperature prevailing over the interval (page -482-).
    pub mean_temperature: ThermodynamicTemperature,
}

/// The state carried across intervals, plus the reported quantities.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FailureProgress {
    /// Accident time elapsed.
    pub elapsed: Time,
    /// `FKOR`, the SiC thinning factor (page -492-). Starts at 1.
    pub thinning_factor: Ratio,
    /// `ζ`, the action integral (Eq 11). Starts at 0.
    pub action_integral: Ratio,
    /// `∫η̇ dt`, the grain-boundary exposure. Starts at 0, and stays there
    /// unless [`GrainBoundaryCorrosion::Enabled`].
    pub grain_boundary_exposure: Ratio,
    /// `φ₁`, accumulated from positive increments (page -482-).
    pub pressure_vessel: FailureFraction,
    /// `φ₂`, read directly off Eq (13) at the current `ζ` (page -483-).
    pub thermal_decomposition: FailureFraction,
    /// `φ_total` (page -480-).
    pub total: FailureFraction,
    /// `φ̇₁ = Δφ₁/Δt` over the interval just taken \[s⁻¹\] (page -511-).
    pub pressure_vessel_rate: Frequency,
    /// `φ̇₂ = Δφ₂/Δt` \[s⁻¹\].
    pub thermal_decomposition_rate: Frequency,
    /// `φ̇_gesamt = Δφ_total/Δt` \[s⁻¹\].
    pub total_rate: Frequency,
}

impl FailureProgress {
    /// The **in-service** failure fraction: `φ₁` and `φ₂` combined, with the
    /// as-manufactured population excluded.
    ///
    /// ```text
    /// f_inc = 1 − (1 − φ₁)·(1 − φ₂)
    /// ```
    ///
    /// This is the quantity PANAMA actually computes, and it is what
    /// [`crate::triso_atops_fork::activities::source_terms::FailureFractions`]
    /// calls `incremental`. [`FailureProgress::total`] differs from it only by
    /// `φ_o`, which PANAMA takes as an input and does not model.
    ///
    /// Both mechanisms are included: a particle whose SiC has thermally
    /// decomposed is no longer a barrier, so it releases its fission gas for
    /// the same reason a burst one does. `φ₂` is **not** routed to
    /// `incremental_sic` — that field is a distinct as-manufactured
    /// population upstream, not an in-service SiC loss, and mapping one onto
    /// the other would be inventing a correspondence neither code states.
    pub fn in_service_failure_fraction(&self) -> FailureFraction {
        total_failure_fraction(
            Ratio::new::<ratio>(0.0),
            self.pressure_vessel,
            self.thermal_decomposition,
        )
    }

    /// The state at `t = 0`: an uncorroded layer, no action integral, and
    /// `φ₁` at its end-of-irradiation value.
    ///
    /// The report (page -482-) states that `φ₁(t = 0)` **is** the value at the
    /// end of irradiation, so it is an input here rather than zero.
    pub fn at_start(
        end_of_irradiation_phi_1: FailureFraction,
        as_manufactured: FailureFraction,
    ) -> Self {
        FailureProgress {
            elapsed: Time::new::<second>(0.0),
            thinning_factor: Ratio::new::<ratio>(1.0),
            action_integral: Ratio::new::<ratio>(0.0),
            grain_boundary_exposure: Ratio::new::<ratio>(0.0),
            pressure_vessel: end_of_irradiation_phi_1,
            thermal_decomposition: Ratio::new::<ratio>(0.0),
            total: total_failure_fraction(
                as_manufactured,
                end_of_irradiation_phi_1,
                Ratio::new::<ratio>(0.0),
            ),
            pressure_vessel_rate: Frequency::new::<hertz>(0.0),
            thermal_decomposition_rate: Frequency::new::<hertz>(0.0),
            total_rate: Frequency::new::<hertz>(0.0),
        }
    }
}

/// The accident driver: a [`ParticleState`] plus the running
/// [`FailureProgress`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AccidentHistory {
    particle: ParticleState,
    progress: FailureProgress,
}

impl AccidentHistory {
    /// Start an accident from the end-of-irradiation state.
    pub fn new(particle: ParticleState, end_of_irradiation_phi_1: FailureFraction) -> Self {
        let progress =
            FailureProgress::at_start(end_of_irradiation_phi_1, particle.as_manufactured);
        AccidentHistory { particle, progress }
    }

    /// The particle this history is running.
    pub fn particle(&self) -> ParticleState {
        self.particle
    }

    /// The current state.
    pub fn progress(&self) -> FailureProgress {
        self.progress
    }

    /// `φ₁(t, T_m)` for a given elapsed accident time and thinning factor —
    /// the full pressure-vessel chain evaluated at one instant.
    ///
    /// Public because it is what Fig. 6's inversion compares against, and
    /// because it is the only way to see the chain's intermediate values
    /// without stepping.
    pub fn pressure_vessel_failure_at(
        &self,
        elapsed: Time,
        thinning_factor: Ratio,
        temperature: ThermodynamicTemperature,
    ) -> FailureFraction {
        let p = self.pressure_at(elapsed, temperature);
        let sigma_t = induced_stress_with_thinning_factor(&self.particle.layer, p, thinning_factor);
        let m = match self.particle.grain_boundary {
            GrainBoundaryCorrosion::Disabled => self.particle.weibull_modulus,
            GrainBoundaryCorrosion::Enabled => corroded_weibull_modulus(
                self.particle.weibull_modulus,
                self.progress.grain_boundary_exposure,
            ),
        };
        weibull_failure_fraction(sigma_t, self.particle.median_strength, m)
    }

    /// **Eq (3)** — the internal gas pressure at an instant, with `F_d` from
    /// Eq (4) at this elapsed accident time and `OPF` at this temperature.
    pub fn pressure_at(&self, elapsed: Time, temperature: ThermodynamicTemperature) -> Pressure {
        let ds = reduced_diffusion_coefficient(
            self.particle.diffusion_kernel,
            temperature,
            self.particle.burnup,
        );
        let tau_a = dimensionless_time(ds, elapsed);
        let f_d = released_gas_fraction(self.particle.dimensionless_irradiation_time, tau_a);
        let opf = self.particle.oxygen.oxygen_per_fission(temperature);
        internal_gas_pressure(
            f_d,
            self.particle.stable_gas_yield,
            opf,
            self.particle.burnup,
            self.particle.free_volume,
            self.particle.kernel_volume,
            molar_volume(self.particle.compound),
            temperature,
        )
    }

    /// Advance one interval (pages -482-, -483-, -492-, -496-) and return the
    /// new state.
    pub fn step(&mut self, step: AccidentStep) -> FailureProgress {
        let dt = step.duration;
        let t_m = step.mean_temperature;
        let d_o = self.particle.layer.initial_thickness();

        let t1 = self.progress.elapsed;
        let t2 = t1 + dt;
        let fkor1 = self.progress.thinning_factor;
        let fkor2 = advance_thinning_factor(fkor1, d_o, t_m, dt);

        // phi_1: BOTH ends at this interval's mean temperature (page -482-).
        let phi1_start = self.pressure_vessel_failure_at(t1, fkor1, t_m);
        let phi1_end = self.pressure_vessel_failure_at(t2, fkor2, t_m);
        let increment = (phi1_end - phi1_start).get::<ratio>().max(0.0);
        let phi1_before = self.progress.pressure_vessel;
        let phi1 = Ratio::new::<ratio>((phi1_before.get::<ratio>() + increment).min(1.0));

        // phi_2: zeta carries the history; Eq (13) is read directly (p-483-).
        let zeta = advance_action_integral(self.progress.action_integral, d_o, t_m, dt);
        let phi2_before = self.progress.thermal_decomposition;
        let phi2 = thermal_decomposition_failure_fraction(zeta, self.particle.decomposition);

        let exposure = match self.particle.grain_boundary {
            GrainBoundaryCorrosion::Disabled => self.progress.grain_boundary_exposure,
            GrainBoundaryCorrosion::Enabled => {
                advance_grain_boundary_exposure(self.progress.grain_boundary_exposure, t_m, dt)
            }
        };

        let total_before = self.progress.total;
        let total = total_failure_fraction(self.particle.as_manufactured, phi1, phi2);

        let seconds = dt.get::<second>();
        let rate = |a: Ratio, b: Ratio| -> Frequency {
            if seconds > 0.0 {
                Frequency::new::<hertz>((b.get::<ratio>() - a.get::<ratio>()) / seconds)
            } else {
                Frequency::new::<hertz>(0.0)
            }
        };

        self.progress = FailureProgress {
            elapsed: t2,
            thinning_factor: fkor2,
            action_integral: zeta,
            grain_boundary_exposure: exposure,
            pressure_vessel: phi1,
            thermal_decomposition: phi2,
            total,
            pressure_vessel_rate: rate(phi1_before, phi1),
            thermal_decomposition_rate: rate(phi2_before, phi2),
            total_rate: rate(total_before, total),
        };
        self.progress
    }

    /// Walk a whole temperature history and return the final state.
    pub fn run(&mut self, steps: &[AccidentStep]) -> FailureProgress {
        for s in steps {
            self.step(*s);
        }
        self.progress
    }

    /// Walk an isothermal hold split into `n` equal intervals — the shape
    /// every heating experiment in the report has.
    pub fn run_isothermal(
        &mut self,
        temperature: ThermodynamicTemperature,
        total: Time,
        n: usize,
    ) -> FailureProgress {
        let n = n.max(1);
        let dt = total / (n as f64);
        for _ in 0..n {
            self.step(AccidentStep {
                duration: dt,
                mean_temperature: temperature,
            });
        }
        self.progress
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::length::micrometer;
    use uom::si::pressure::{megapascal, pascal};
    use uom::si::f64::Length;
    use uom::si::thermodynamic_temperature::degree_celsius;
    use uom::si::time::{day, hour};
    use uom::si::volume::cubic_meter;

    use crate::fuel_failure::grain_boundary::grain_boundary_corrosion_rate;
    use crate::fuel_failure::strength::{irradiated_strength, irradiated_weibull_modulus};

    /// Table 1 (page -500-), "before irradiation" columns — the eight SiC
    /// varieties Fig. 6 plots, in the figure's own top-to-bottom order.
    /// Already verified 16/16 by `strength::tests::table_1_is_reproduced_exactly`.
    const TABLE_1: [(&str, f64, f64); 8] = [
        ("EO 249-251", 453.0, 5.0),
        ("HT 150-167", 600.0, 6.0),
        ("EO 1674", 722.0, 7.0),
        ("ECO 1541", 1080.0, 6.4),
        ("EO 1607", 850.0, 8.0),
        ("EC 1338/1339", 998.0, 7.4),
        ("EO 403-405", 867.0, 8.4),
        ("EUO 1551", 1060.0, 8.5),
    ];

    /// A representative (Th,U)O2 TRISO. Dimensions only — nothing here is
    /// calibrated to any figure, and Fig. 6's caption states none of it.
    fn particle() -> ParticleState {
        let t_b = ThermodynamicTemperature::new::<degree_celsius>(1000.0);
        ParticleState {
            layer: SicLayer {
                inner_radius: Length::new::<micrometer>(250.0),
                outer_radius: Length::new::<micrometer>(285.0),
            },
            compound: KernelCompound::ThoriumUraniumOxide,
            diffusion_kernel: KernelKind::ThoriumUraniumOxide,
            kernel_volume: Volume::new::<cubic_meter>(9.0e-13),
            free_volume: Volume::new::<cubic_meter>(1.8e-13),
            burnup: Ratio::new::<ratio>(0.10),
            stable_gas_yield: Ratio::new::<ratio>(super::super::STABLE_FISSION_GAS_YIELD),
            dimensionless_irradiation_time: Ratio::new::<ratio>(0.09),
            median_strength: irradiated_strength(Pressure::new::<megapascal>(600.0), 1.0, t_b),
            weibull_modulus: irradiated_weibull_modulus(6.0, 1.0, t_b),
            oxygen: OxygenSource::ThoriumUraniumOxide {
                thorium_to_u235: 5.0,
                burnup: Ratio::new::<ratio>(0.10),
            },
            decomposition: DecompositionCalibration::ParticlesInSphere,
            grain_boundary: GrainBoundaryCorrosion::Disabled,
            as_manufactured: Ratio::new::<ratio>(0.0),
        }
    }

    fn zero() -> FailureFraction {
        Ratio::new::<ratio>(0.0)
    }

    /// FRJ2-K11/03 as Fig. 7's caption states it (page -501-):
    /// `σ_oo = 600 MPa`, `m_oo = 6`, `T_B = 1160 degC`, `t_B = 260 FPD`,
    /// `F_B = 0.09 FIMA`, `Γ = 0.05·10²⁵ m⁻² EDN`, `η̇(T) ≡ 0`.
    ///
    /// Two things the caption does **not** state:
    ///
    /// - **The kernel compound.** `UO₂` is used, with Eq (5c) for `OPF`. It
    ///   is the tightest of the four candidates against the report's own
    ///   curve over 0–300 h (relative s.d. 5.0 % against 5.7 % for
    ///   `(Th,U)O₂` at `N = 10`, 6.1 % at `N = 5`, and 7.7 % for `UCO`, i.e.
    ///   `OPF = 0`), but that is a weak preference and is recorded as such.
    /// - **The geometry.** `r`, `d_o`, `V_k` and `V_f` set the absolute
    ///   stress and nothing in the report gives them. The values here are a
    ///   representative TRISO and are **not** fitted: every Fig. 7 assertion
    ///   is on a ratio, in which they cancel.
    fn frj2_k11_particle() -> ParticleState {
        let t_b = ThermodynamicTemperature::new::<degree_celsius>(1160.0);
        let t_b_time = Time::new::<day>(260.0);
        let gamma = 0.05;
        ParticleState {
            layer: SicLayer {
                inner_radius: Length::new::<micrometer>(250.0),
                outer_radius: Length::new::<micrometer>(285.0),
            },
            compound: KernelCompound::UraniumOxide,
            diffusion_kernel: KernelKind::UraniumOxide,
            kernel_volume: Volume::new::<cubic_meter>(9.0e-13),
            free_volume: Volume::new::<cubic_meter>(1.8e-13),
            burnup: Ratio::new::<ratio>(0.09),
            stable_gas_yield: Ratio::new::<ratio>(super::super::STABLE_FISSION_GAS_YIELD),
            dimensionless_irradiation_time: irradiation_tau(
                KernelKind::UraniumOxide,
                t_b,
                t_b_time,
                Ratio::new::<ratio>(0.09),
            ),
            median_strength: irradiated_strength(Pressure::new::<megapascal>(600.0), gamma, t_b),
            weibull_modulus: irradiated_weibull_modulus(6.0, gamma, t_b),
            oxygen: OxygenSource::UraniumOxide {
                irradiation_temperature: t_b,
                irradiation_time: t_b_time,
            },
            decomposition: DecompositionCalibration::ParticlesInSphere,
            grain_boundary: GrainBoundaryCorrosion::Disabled,
            as_manufactured: zero(),
        }
    }

    /// Fig. 7's staging, stated on page -498-: 100 h at 1400 degC, 100 h at
    /// 1500 degC, then 1600 degC to the end.
    fn frj2_k11_temperature(t_h: f64) -> ThermodynamicTemperature {
        let c = if t_h <= 100.0 {
            1400.0
        } else if t_h <= 200.0 {
            1500.0
        } else {
            1600.0
        };
        ThermodynamicTemperature::new::<degree_celsius>(c)
    }

    /// Step the driver through Fig. 7's staged history to `t_h` and return
    /// the state. One-hour steps: the report's page -482- claim of step-size
    /// independence is checked separately.
    fn frj2_k11_run_to(t_h: f64) -> (AccidentHistory, FailureProgress) {
        let mut h = AccidentHistory::new(frj2_k11_particle(), zero());
        let mut done = 0.0f64;
        let mut progress = h.progress();
        while done < t_h - 1e-9 {
            let dt = (t_h - done).min(1.0);
            progress = h.step(AccidentStep {
                duration: Time::new::<hour>(dt),
                mean_temperature: frj2_k11_temperature(done + 0.5 * dt),
            });
            done += dt;
        }
        (h, progress)
    }

    fn frj2_k11_thinning_factor(t_h: f64) -> Ratio {
        frj2_k11_run_to(t_h).1.thinning_factor
    }

    /// `σ_t` from the chain at `t_h` on Fig. 7's staged history.
    fn frj2_k11_stress_at(t_h: f64) -> Pressure {
        let (h, p) = frj2_k11_run_to(t_h);
        let t = frj2_k11_temperature(t_h);
        induced_stress_with_thinning_factor(
            &h.particle().layer,
            h.pressure_at(Time::new::<hour>(t_h), t),
            p.thinning_factor,
        )
    }

    /// **The report's own claim, page -482-: "At a constant temperature, the
    /// length of the time interval does not influence the computed result."**
    ///
    /// Methodology: run 300 h at 1600 °C in 1, 12, 300 and 3000 equal
    /// intervals and compare `φ_total`, `FKOR` and `ζ`. This is falsifiable
    /// and independent of every figure: a driver that recomputed `FKOR` or
    /// `ζ` from total elapsed time, or that accumulated `φ₂` by increments
    /// instead of reading Eq (13), would fail it.
    ///
    /// Result, 2026-09-24: `φ_total` agrees to 2·10⁻¹² relative, `FKOR` and
    /// `ζ` to better than 10⁻¹².
    #[test]
    fn the_step_length_does_not_matter_at_constant_temperature() {
        let t = ThermodynamicTemperature::new::<degree_celsius>(1600.0);
        let total = Time::new::<hour>(300.0);
        let mut reference = None;
        for n in [1usize, 12, 300, 3000] {
            let mut h = AccidentHistory::new(particle(), zero());
            let p = h.run_isothermal(t, total, n);
            match reference {
                None => reference = Some(p),
                Some(r) => {
                    let rel = |a: f64, b: f64| (a - b).abs() / b.abs().max(1e-300);
                    assert!(
                        rel(p.total.get::<ratio>(), r.total.get::<ratio>()) < 1e-10,
                        "n={n}: phi_total {} vs {}",
                        p.total.get::<ratio>(),
                        r.total.get::<ratio>()
                    );
                    assert!(
                        rel(
                            p.thinning_factor.get::<ratio>(),
                            r.thinning_factor.get::<ratio>()
                        ) < 1e-12
                    );
                    assert!(
                        rel(
                            p.action_integral.get::<ratio>(),
                            r.action_integral.get::<ratio>()
                        ) < 1e-12
                    );
                }
            }
        }
    }

    /// **`φ₁` accumulates; `φ₂` is read directly.** The distinction is
    /// invisible on a monotone heat-up and decisive on anything that cools —
    /// which is every reactor transient in Figs. 10–13.
    ///
    /// Run 2000 °C then 1200 °C: `ζ` keeps rising (slowly), so `φ₂` does not
    /// fall; and `φ₁` must not fall even though the cooler step's own
    /// increment is negative.
    #[test]
    fn the_two_mechanisms_are_stepped_differently() {
        let mut h = AccidentHistory::new(particle(), zero());
        let hot = h.step(AccidentStep {
            duration: Time::new::<hour>(100.0),
            mean_temperature: ThermodynamicTemperature::new::<degree_celsius>(2000.0),
        });
        let cold = h.step(AccidentStep {
            duration: Time::new::<hour>(100.0),
            mean_temperature: ThermodynamicTemperature::new::<degree_celsius>(1200.0),
        });

        assert!(
            cold.pressure_vessel >= hot.pressure_vessel,
            "phi_1 must be monotone: {:?} then {:?}",
            hot.pressure_vessel,
            cold.pressure_vessel
        );
        // The cooling step's own phi_1 increment is negative -- check that
        // the discard is what kept it monotone, not luck.
        let fkor = hot.thinning_factor;
        let cooler = ThermodynamicTemperature::new::<degree_celsius>(1200.0);
        let a = h.pressure_vessel_failure_at(Time::new::<hour>(100.0), fkor, cooler);
        let b =
            h.pressure_vessel_failure_at(Time::new::<hour>(200.0), cold.thinning_factor, cooler);
        assert!(
            b.get::<ratio>() < hot.pressure_vessel.get::<ratio>(),
            "the cool step really is below the hot one's level ({} vs {})",
            b.get::<ratio>(),
            hot.pressure_vessel.get::<ratio>()
        );
        assert!(a.get::<ratio>() < hot.pressure_vessel.get::<ratio>());

        assert!(
            cold.action_integral > hot.action_integral,
            "zeta is monotone by construction"
        );
        assert!(cold.thermal_decomposition >= hot.thermal_decomposition);
    }

    /// **Fig. 6 (page -500-) recovers ONE common stress history from all
    /// eight varieties — and the residual is systematic.**
    ///
    /// Methodology: `σ_t(t)` is common to all eight curves, so inverting
    /// Eq (1) on each, `σ_t = σ_o·(−ln(1−φ)/ln2)^(1/m)`, must return the same
    /// value eight times. `σ_o` and `m` come from Eqs (8a)/(9a) applied to
    /// Table 1's before-irradiation columns at the basis page -498- states
    /// for Fig. 6: `T_B = 1000 °C`, `Γ = 1·10²⁵ m⁻² EDN`. Nothing is fitted —
    /// the figure's unstated inputs only set `σ_t`, which is the unknown
    /// being solved for.
    ///
    /// Results, 2026-09-24 (467 digitised points, maintainer). At 210 h, the
    /// seven varieties lying inside the figure's plotted 10⁻⁶ floor recover
    /// `σ_t = 152.8 MPa` with a relative s.d. of **10.4 %**; feeding that one
    /// stress back through Eq (1) reproduces the figure's rank order **8/8**
    /// across a 4.8-decade spread, with per-curve residuals running
    /// **−0.370 … +0.388** in `log₁₀ φ` (mean \|·\| 0.232).
    ///
    /// **That residual is a digitisation artefact, and this test now says
    /// so.** Deflating the log ordinate by `6/7` — seven decades entered
    /// where six are plotted — collapses the eight varieties onto one `σ_t`
    /// at **1.4 % relative s.d.**, with per-variety residuals of ±2 % in
    /// place of +23 %/−10 %. The same `6/7` is picked out independently by
    /// the two-curve identity on Figs. 7 and 8
    /// ([`figure_8_two_curves_expose_a_log_axis_calibration_error`]), which
    /// uses no model at all. Both readings are asserted here so the evidence
    /// survives: the trend as digitised, and its removal.
    #[test]
    fn figure_6_recovers_one_common_stress_history() {
        // Digitised Fig. 6 at t = 210 h, in the figure's top-to-bottom order.
        let at_210h: [f64; 8] = [
            2.057e-2, 1.283e-3, 8.180e-5, 1.592e-5, 4.696e-6, 3.740e-6, 1.837e-6, 3.040e-7,
        ];
        let t_b = ThermodynamicTemperature::new::<degree_celsius>(1000.0);
        let props: Vec<(f64, f64)> = TABLE_1
            .iter()
            .map(|(_, s_oo, m_oo)| {
                (
                    irradiated_strength(Pressure::new::<megapascal>(*s_oo), 1.0, t_b)
                        .get::<megapascal>(),
                    irradiated_weibull_modulus(*m_oo, 1.0, t_b),
                )
            })
            .collect();

        // 1. Invert Eq (1) on every curve that lies inside the plotted axis.
        let recovered: Vec<f64> = at_210h
            .iter()
            .zip(props.iter())
            .filter(|(phi, _)| **phi >= 1.0e-6)
            .map(|(phi, (s_o, m))| s_o * (-(1.0 - phi).ln() / std::f64::consts::LN_2).powf(1.0 / m))
            .collect();
        assert_eq!(
            recovered.len(),
            7,
            "one curve is below the plotted 1e-6 floor"
        );
        let mean = recovered.iter().sum::<f64>() / recovered.len() as f64;
        let rel_sd = (recovered.iter().map(|x| (x - mean).powi(2)).sum::<f64>()
            / recovered.len() as f64)
            .sqrt()
            / mean;
        assert!(
            (145.0..160.0).contains(&mean),
            "recovered common sigma_t should be ~152.8 MPa, got {mean:.1} ({recovered:?})"
        );
        assert!(
            rel_sd < 0.12,
            "the eight varieties must agree on one sigma_t to ~10 %; got {rel_sd:.4}"
        );

        // 2. That one stress must reproduce the figure's rank order, 8/8.
        let sigma_t = Pressure::new::<megapascal>(mean);
        let predicted: Vec<f64> = props
            .iter()
            .map(|(s_o, m)| {
                weibull_failure_fraction(sigma_t, Pressure::new::<megapascal>(*s_o), *m)
                    .get::<ratio>()
            })
            .collect();
        for i in 1..8 {
            assert!(
                predicted[i] < predicted[i - 1],
                "Fig. 6's order must come out of Eq (1) at one sigma_t: \
                 {} then {} at index {i}",
                predicted[i - 1],
                predicted[i]
            );
        }
        let spread = (at_210h[0] / at_210h[7]).log10();
        assert!(
            spread > 4.5,
            "Fig. 6 spans {spread:.2} decades -- this is a wide family, not a narrow one"
        );

        // 3. The residual is systematic in m_oo -- and it is a DIGITISATION
        //    artefact, not physics. Both halves are asserted so neither can
        //    be lost: the trend as digitised, and its removal under the 6/7
        //    log-axis reading that Figs. 7 and 8 independently pick out.
        let resid: Vec<f64> = predicted
            .iter()
            .zip(at_210h.iter())
            .map(|(p, f)| (p / f).log10())
            .collect();
        assert!(
            resid[0] < -0.3 && resid[7] > 0.3,
            "as digitised, the residual runs monotonically with m_oo: {resid:?}"
        );

        // Deflate the log ordinate by 6/7 -- seven decades entered where six
        // are plotted -- and the eight varieties collapse onto one sigma_t.
        let deflate = |phi: f64| 10f64.powf(-6.0 + (6.0 / 7.0) * (phi.log10() + 6.0));
        let corrected: Vec<f64> = at_210h
            .iter()
            .zip(props.iter())
            .map(|(phi, (s_o, m))| {
                s_o * (-(1.0 - deflate(*phi)).ln() / std::f64::consts::LN_2).powf(1.0 / m)
            })
            .collect();
        let cmean = corrected.iter().sum::<f64>() / 8.0;
        let crel =
            (corrected.iter().map(|x| (x - cmean).powi(2)).sum::<f64>() / 8.0).sqrt() / cmean;
        assert!(
            crel < 0.03,
            "under the 6/7 reading the eight varieties must agree to ~1.4 %; \
             got {crel:.4} from {corrected:?}"
        );
        assert!(
            crel < rel_sd / 5.0,
            "the corrected reading must be decisively better ({crel:.4} against {rel_sd:.4}), \
             not marginally -- that is what makes it a calibration finding rather than a fit"
        );
    }

    /// **Fig. 7 (page -501-): the staged history is reproduced for 300 h,
    /// then drifts to a factor 1.9 — and the drift is the finding.**
    ///
    /// Methodology: the FRJ2-K11/03 heating experiment, with the caption's
    /// complete input set (`σ_oo = 600 MPa`, `m_oo = 6`, `T_B = 1160 °C`,
    /// `t_B = 260 FPD`, `F_B = 0.09`, `Γ = 0.05·10²⁵`, `η̇ ≡ 0`) and the
    /// staging page -498- states outright: 100 h at 1400 °C, 100 h at
    /// 1500 °C, then 1600 °C. The comparator is the report's own `Without
    /// Grain Boundary Corrosion` curve, which is the one `η̇ ≡ 0` selects.
    ///
    /// The absolute level needs the geometry the caption omits, so the test
    /// is on the *ratio* `σ_t^PANAMA / σ_t^chain`, where `σ_t^PANAMA` comes
    /// from inverting Eq (1) on the digitised curve. One free scale — the
    /// geometry aggregate — and no physics constant touched.
    ///
    /// Results, 2026-09-24 (71 digitised points): the ratio is constant to
    /// **4.9 % relative s.d. over 0–300 h across all three stages** (per
    /// stage 0.1561 / 0.1473 / 0.1608), then rises to 0.295 by 977 h. At
    /// 977 h PANAMA implies `σ_t = 282 MPa` where the chain gives 148 MPa, a
    /// factor **1.91**. PANAMA's curve goes as `φ ∝ t^3.21`, i.e.
    /// `σ_t ∝ t^0.54`, while in the chain `F_d` has saturated (0.980 at
    /// 296 h → 0.9999 at 977 h) and only `FKOR` is left, worth 4 % over the
    /// last 700 h.
    ///
    /// **⁸⁵Kr release is equated with particle failure throughout.** That is
    /// the report's own identification, inherited here, not derived.
    #[test]
    fn figure_7_reproduces_the_staged_history_then_drifts() {
        // Digitised "Without Grain Boundary Corrosion", (t h, release fraction).
        let panama: [(f64, f64); 7] = [
            (22.5, 5.412e-6),
            (81.1, 1.039e-5),
            (174.0, 3.233e-5),
            (296.1, 1.739e-4),
            (475.5, 7.903e-4),
            (726.2, 2.676e-3),
            (976.8, 7.986e-3),
        ];
        let p = frj2_k11_particle();
        let sigma_o = p.median_strength.get::<megapascal>();
        let m = p.weibull_modulus;

        let mut ratios = Vec::new();
        for (t_h, phi) in panama {
            let chain = frj2_k11_stress_at(t_h).get::<megapascal>();
            let from_figure = sigma_o * (-(1.0 - phi).ln() / std::f64::consts::LN_2).powf(1.0 / m);
            ratios.push((t_h, from_figure / chain, from_figure, chain));
        }

        // 1. Through all three stages, out to 300 h, ONE scale works.
        let early: Vec<f64> = ratios
            .iter()
            .filter(|(t, ..)| *t <= 300.0)
            .map(|(_, r, ..)| *r)
            .collect();
        let mean = early.iter().sum::<f64>() / early.len() as f64;
        let rel_sd = (early.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / early.len() as f64)
            .sqrt()
            / mean;
        assert!(
            rel_sd < 0.08,
            "0-300 h spans 1400, 1500 and 1600 degC; one scale should hold to \
             ~5 %, got {rel_sd:.4} from {early:?}"
        );

        // 2. And then it does not. Recorded, not tuned away.
        let (t_last, r_last, fig_last, chain_last) = *ratios.last().unwrap();
        let drift = r_last / mean;
        assert!(
            (1.6..2.3).contains(&drift),
            "the late-time drift measured 1.9x at {t_last} h; got {drift:.2} \
             (figure implies {fig_last:.0} MPa, chain gives {chain_last:.0} MPa)"
        );
        assert!(
            (270.0..295.0).contains(&fig_last),
            "PANAMA implies sigma_t = 282 MPa at 977 h, got {fig_last:.1}"
        );

        // 3. The cause: F_d has saturated, so the chain has nothing left to
        //    grow with. Pinned so the diagnosis is checkable, not asserted.
        let history = AccidentHistory::new(p, zero());
        let hot = ThermodynamicTemperature::new::<degree_celsius>(1600.0);
        let ds = reduced_diffusion_coefficient(p.diffusion_kernel, hot, p.burnup);
        let f_d = |t_h: f64| {
            released_gas_fraction(
                p.dimensionless_irradiation_time,
                dimensionless_time(ds, Time::new::<hour>(t_h)),
            )
            .get::<ratio>()
        };
        assert!(f_d(296.1) > 0.97 && f_d(976.8) > 0.9998, "F_d saturates");
        let _ = history;
        let fkor_ratio = frj2_k11_thinning_factor(976.8).get::<ratio>()
            / frj2_k11_thinning_factor(296.1).get::<ratio>();
        assert!(
            fkor_ratio < 1.06,
            "FKOR is all the chain has left after 300 h, and it is worth {:.1} %",
            100.0 * (fkor_ratio - 1.0)
        );
    }

    /// **Fig. 7, code-to-data: the measurement sits BETWEEN PANAMA's two
    /// curves.**
    ///
    /// Methodology: the nine measured ⁸⁵Kr points against the report's own
    /// two curves, interpolated in `log₁₀`. No geometry and no part of this
    /// implementation enter — this is a statement about PANAMA.
    ///
    /// Results, 2026-09-24: against `Without Grain Boundary Corrosion`
    /// (`η̇ ≡ 0`, the caption's case) the mean residual is **+0.51 decades**
    /// (mean \|·\| 0.52, worst +1.14 at 339 h) — PANAMA **under**-predicts.
    /// Against `With Grain Boundary Corrosion` it is **−1.49 decades** (worst
    /// −1.85) — a large over-prediction. So the data lies between them,
    /// nearer the "without" curve, which reproduces page -499-'s own reading
    /// that the with-corrosion model covers the measurements conservatively.
    ///
    /// One digitisation label reads `90% FIMA` where the caption says
    /// `9.0 % FIMA`; 0.09 is the value used throughout.
    #[test]
    fn figure_7_brackets_the_measurement() {
        // (t h, measured 85Kr, PANAMA without gb, PANAMA with gb) -- the
        // PANAMA values interpolated from the digitised curves at the
        // measurement times.
        let rows: [(f64, f64, f64, f64); 9] = [
            (99.8, 4.690e-5, 1.276e-5, 1.021e-4),
            (202.2, 4.757e-5, 4.876e-5, 3.091e-3),
            (223.4, 9.397e-5, 6.642e-5, 6.703e-3),
            (250.1, 3.084e-4, 9.809e-5, 1.360e-2),
            (274.8, 3.848e-4, 1.373e-4, 2.204e-2),
            (287.7, 4.720e-4, 1.600e-4, 2.676e-2),
            (300.4, 6.292e-4, 1.823e-4, 3.217e-2),
            (318.4, 1.612e-3, 2.361e-4, 4.040e-2),
            (339.3, 3.685e-3, 2.684e-4, 4.892e-2),
        ];
        let mut without = Vec::new();
        let mut with = Vec::new();
        for (_, meas, no_gb, gb) in rows {
            without.push((meas / no_gb).log10());
            with.push((meas / gb).log10());
        }
        let mean = |v: &Vec<f64>| v.iter().sum::<f64>() / v.len() as f64;
        assert!(
            (0.40..0.65).contains(&mean(&without)),
            "PANAMA without grain-boundary corrosion under-predicts FRJ2-K11/03 \
             by ~+0.51 decades; got {:.3}",
            mean(&without)
        );
        assert!(
            (-1.60..-1.35).contains(&mean(&with)),
            "PANAMA with grain-boundary corrosion over-predicts by ~1.49 \
             decades; got {:.3}",
            mean(&with)
        );
        // The measurement is bracketed at every single point.
        for (i, (t, ..)) in rows.iter().enumerate() {
            assert!(
                without[i] > 0.0 || with[i] < 0.0,
                "at {t} h the measurement must lie between the two curves"
            );
        }
        // And the "without" curve is the closer one, which is what makes the
        // with-corrosion variant "conservative" in the report's sense.
        assert!(mean(&without).abs() < mean(&with).abs());
    }

    /// AVR GO 2 as Fig. 8's caption states it (page -501-): `σ_oo = 600 MPa`,
    /// `m_oo = 6`, `T_B = 950 degC`, `t_B = 500 FPD`, `F_B = 0.082 FIMA`,
    /// `Γ = 0.6·10²⁵ m⁻² EDN`, `η̇(T) ≡ 0`, isothermal at 1600 degC.
    ///
    /// **The kernel is `(Th,U)O₂` with `N = 5`**, and unlike Fig. 7 that is
    /// not a weak preference — three independent reasons agree:
    /// the report's own Eq (5a) note gives `N = 5` for **AVR**; GO 2 is an
    /// AVR fuel element; and the `UO₂` correlation is **excluded** by the
    /// growth bound in
    /// [`figure_8_excludes_the_uranium_oxide_oxygen_correlation`].
    ///
    /// Geometry is again not stated and again cancels: every Fig. 8
    /// assertion is on a ratio.
    fn avr_go2_particle() -> ParticleState {
        let t_b = ThermodynamicTemperature::new::<degree_celsius>(950.0);
        let t_b_time = Time::new::<day>(500.0);
        let gamma = 0.6;
        let burnup = Ratio::new::<ratio>(0.082);
        ParticleState {
            layer: SicLayer {
                inner_radius: Length::new::<micrometer>(250.0),
                outer_radius: Length::new::<micrometer>(285.0),
            },
            compound: KernelCompound::ThoriumUraniumOxide,
            diffusion_kernel: KernelKind::ThoriumUraniumOxide,
            kernel_volume: Volume::new::<cubic_meter>(9.0e-13),
            free_volume: Volume::new::<cubic_meter>(1.8e-13),
            burnup,
            stable_gas_yield: Ratio::new::<ratio>(super::super::STABLE_FISSION_GAS_YIELD),
            dimensionless_irradiation_time: irradiation_tau(
                KernelKind::ThoriumUraniumOxide,
                t_b,
                t_b_time,
                burnup,
            ),
            median_strength: irradiated_strength(Pressure::new::<megapascal>(600.0), gamma, t_b),
            weibull_modulus: irradiated_weibull_modulus(6.0, gamma, t_b),
            oxygen: OxygenSource::ThoriumUraniumOxide {
                thorium_to_u235: 5.0,
                burnup,
            },
            decomposition: DecompositionCalibration::ParticlesInSphere,
            grain_boundary: GrainBoundaryCorrosion::Disabled,
            as_manufactured: zero(),
        }
    }

    fn avr_go2_stress_at(t_h: f64) -> Pressure {
        let mut h = AccidentHistory::new(avr_go2_particle(), zero());
        let t = ThermodynamicTemperature::new::<degree_celsius>(1600.0);
        let p = h.run_isothermal(t, Time::new::<hour>(t_h), (t_h.ceil() as usize).max(1));
        induced_stress_with_thinning_factor(
            &h.particle().layer,
            h.pressure_at(Time::new::<hour>(t_h), t),
            p.thinning_factor,
        )
    }

    /// Invert Eq (1): the `σ_t` a digitised failure fraction implies.
    fn stress_implied_by(phi: f64, sigma_o: Pressure, m: f64) -> f64 {
        sigma_o.get::<megapascal>() * (-(1.0 - phi).ln() / std::f64::consts::LN_2).powf(1.0 / m)
    }

    /// **Fig. 8 (page -501-): the Fig. 7 drift REAPPEARS under a purely
    /// isothermal history — so the driver's staging is exonerated.**
    ///
    /// Methodology: AVR GO 2, isothermal 1600 degC to 1000 h, caption inputs,
    /// compared against the report's own `without Grain Boundary Corrosion`
    /// curve on the ratio `σ_t^PANAMA / σ_t^chain` with one free geometry
    /// scale. This is the discriminating run for gh:#295's second
    /// disagreement: Fig. 7 was staged 1400/1500/1600 degC, so a drift there
    /// could have been the driver mishandling stage changes. Fig. 8 has no
    /// stages.
    ///
    /// Results, 2026-09-24 (74 digitised points): the ratio rises
    /// **monotonically from the first point** — 0.189 at 14.6 h to 0.447 at
    /// 967 h, relative s.d. **19.0 %** over the whole run, max/min 2.37.
    /// There is **no flat window at all**, where Fig. 7 held to 4.9 % for its
    /// first 300 h. The drift is therefore not a staging artefact; if
    /// anything Fig. 7's flat early window now looks like the rising
    /// temperature masking the same shortfall.
    ///
    /// Both PANAMA curves go as `σ_t ∝ t^0.52` (Fig. 8) and `t^0.54`
    /// (Fig. 7) at late times, while the chain has only `FKOR` left once
    /// `F_d` saturates — 4.2 % from 244 h to 967 h here.
    ///
    /// **Digitisation caveat.** Fig. 8's y-axis calibration is very likely
    /// stretched by 7/6 (see
    /// [`figure_8_two_curves_expose_a_log_axis_calibration_error`]).
    /// Deflating the log ordinate by that factor reduces the disagreement
    /// from 19.0 % to **14.3 %** and max/min from 2.37 to 1.91 — it does
    /// **not** remove it. No correction is applied to the data here; the
    /// reduced figure is quoted so the reader knows how much of the
    /// disagreement the calibration could at most account for.
    #[test]
    fn figure_8_isothermal_drift_reappears() {
        // Digitised "without Grain Boundary Corrosion", (t h, release fraction).
        let panama: [(f64, f64); 9] = [
            (14.6, 2.126e-6),
            (46.5, 9.056e-6),
            (91.2, 2.466e-5),
            (148.6, 6.298e-5),
            (244.4, 1.752e-4),
            (291.7, 2.683e-4),
            (490.9, 1.072e-3),
            (700.3, 3.046e-3),
            (954.4, 8.654e-3),
        ];
        let p = avr_go2_particle();
        let ratios: Vec<f64> = panama
            .iter()
            .map(|(t, phi)| {
                stress_implied_by(*phi, p.median_strength, p.weibull_modulus)
                    / avr_go2_stress_at(*t).get::<megapascal>()
            })
            .collect();

        // Monotone from the very first point -- no plateau, unlike Fig. 7.
        for i in 1..ratios.len() {
            assert!(
                ratios[i] > ratios[i - 1],
                "the ratio must rise throughout: {ratios:?}"
            );
        }
        let drift = ratios[ratios.len() - 1] / ratios[0];
        assert!(
            (2.0..2.8).contains(&drift),
            "isothermal Fig. 8 drifts by ~2.4x over 14.6-954 h; got {drift:.2}"
        );

        // And there is no 300 h window that holds the way Fig. 7's did.
        let early: Vec<f64> = panama
            .iter()
            .zip(ratios.iter())
            .filter(|((t, _), _)| *t <= 302.0)
            .map(|(_, r)| *r)
            .collect();
        let mean = early.iter().sum::<f64>() / early.len() as f64;
        let rel_sd = (early.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / early.len() as f64)
            .sqrt()
            / mean;
        assert!(
            rel_sd > 0.10,
            "Fig. 7's first 300 h held to 4.9 %; Fig. 8's does not ({rel_sd:.4}). \
             That asymmetry is the finding -- do not tune it away."
        );

        // The cause is the same as Fig. 7's: FKOR is all that is left.
        let fkor_ratio = {
            let mut h = AccidentHistory::new(p, zero());
            let t = ThermodynamicTemperature::new::<degree_celsius>(1600.0);
            let late = h
                .run_isothermal(t, Time::new::<hour>(967.0), 967)
                .thinning_factor
                .get::<ratio>();
            let mut h2 = AccidentHistory::new(p, zero());
            let early = h2
                .run_isothermal(t, Time::new::<hour>(244.0), 244)
                .thinning_factor
                .get::<ratio>();
            late / early
        };
        assert!(
            fkor_ratio < 1.05,
            "FKOR is worth {:.1} % over the last 720 h",
            100.0 * (fkor_ratio - 1.0)
        );
    }

    /// **Fig. 8's two PANAMA curves expose a log-axis calibration error —
    /// and, once it is accounted for, VERIFY Eqs (10b)/(10c).**
    ///
    /// Methodology: the `with` and `without Grain Boundary Corrosion` curves
    /// are the same calculation with and without Eq (10b), so inverting
    /// Eq (1) on each — using `m_o` for one and `m_o·(0.44 + 0.56·e^(−η̇t))`
    /// with `η̇` from Eq (10c) for the other — **must return one `σ_t(t)`**.
    /// No geometry, no free scale, no part of the chain: this uses only the
    /// digitised curves and Eqs (1)/(10b)/(10c).
    ///
    /// Result, 2026-09-24: taken as digitised, the two disagree badly —
    /// `σ_t^with / σ_t^without` runs 1.24 → 1.81, mean **1.505**. But the
    /// disagreement is exactly what a **stretched log ordinate** produces,
    /// and solving for the stretch gives `k = 0.855` on Fig. 8 and `0.875` on
    /// Fig. 7, against `6/7 = 0.857`. At `k = 6/7` the two curves agree to a
    /// mean ratio of **1.014** (Fig. 8) and **1.000** (Fig. 7).
    ///
    /// `6/7` is the signature of an axis calibrated as **seven decades where
    /// six are plotted**: both figures label `10⁰` at the top gridline, and
    /// both digitisations carry an upper calibration point entered as `10`.
    ///
    /// Two things follow, and they matter in opposite directions:
    ///
    /// - **Eqs (10b)/(10c) are verified** — the first external check this
    ///   crate has on them, and previously recorded as impossible. The
    ///   grain-boundary law connects the report's own two curves.
    /// - **Figs. 7 and 8's digitised ordinates are ~17 % too wide in the
    ///   log**, so every residual quoted against them is an upper bound.
    ///
    /// **Nothing is corrected here.** Deflating the data would erase the
    /// evidence; the right fix is to re-digitise with the axis calibrated on
    /// the plotted decades. What this test pins is the *size* of the effect,
    /// so later work knows how much precision the digitisation supports.
    #[test]
    fn figure_8_two_curves_expose_a_log_axis_calibration_error() {
        // (t h, without corrosion, with corrosion), interpolated from the
        // two digitised curves at common times.
        let pairs: [(f64, f64, f64); 8] = [
            (30.0, 4.791e-6, 9.953e-5),
            (60.0, 1.301e-5, 7.914e-4),
            (100.0, 2.857e-5, 3.717e-3),
            (200.0, 1.106e-4, 2.686e-2),
            (300.0, 2.767e-4, 6.207e-2),
            (500.0, 1.122e-3, 1.286e-1),
            (700.0, 3.039e-3, 2.105e-1),
            (900.0, 7.025e-3, 3.250e-1),
        ];
        let p = avr_go2_particle();
        let hot = ThermodynamicTemperature::new::<degree_celsius>(1600.0);

        let ratio_at = |k: f64| -> f64 {
            let deflate = |phi: f64| 10f64.powf(-6.0 + k * (phi.log10() + 6.0));
            let mut acc = 0.0;
            for (t_h, no_gb, gb) in pairs {
                let exposure: Ratio = grain_boundary_corrosion_rate(hot) * Time::new::<hour>(t_h);
                let m_gb = corroded_weibull_modulus(p.weibull_modulus, exposure);
                let a = stress_implied_by(deflate(no_gb), p.median_strength, p.weibull_modulus);
                let b = stress_implied_by(deflate(gb), p.median_strength, m_gb);
                acc += b / a;
            }
            acc / pairs.len() as f64
        };

        // As digitised: a 50 % disagreement where the identity demands 0 %.
        let raw = ratio_at(1.0);
        assert!(
            (1.4..1.65).contains(&raw),
            "taken as digitised the two curves disagree by ~1.5x, got {raw:.3}"
        );

        // Deflating by 6/7 -- seven decades entered for six plotted -- closes it.
        let corrected = ratio_at(6.0 / 7.0);
        assert!(
            (corrected - 1.0).abs() < 0.05,
            "at k = 6/7 Eqs (10b)/(10c) must connect the two curves; got {corrected:.4}"
        );
        assert!(
            (corrected - 1.0).abs() < (raw - 1.0).abs() / 5.0,
            "the calibration reading must be decisively better, not marginally"
        );
    }

    /// **Fig. 8 EXCLUDES the `UO₂` oxygen correlation for AVR GO 2 — by a
    /// bound that no value of `D_S` can escape.**
    ///
    /// Methodology: at fixed temperature, `σ_t ∝ (F_d·F_f + OPF)·FKOR`, and
    /// `F_d` is a fraction. So the **largest** growth in `σ_t` the chain can
    /// produce between any two times is `(F_f + OPF)/OPF` times the `FKOR`
    /// ratio, attained only if `F_d` runs the whole way from 0 to 1. That
    /// ceiling depends on `OPF` alone — not on `D_S`, not on `τ_i`, not on
    /// the geometry.
    ///
    /// Result, 2026-09-24: Fig. 8's curve requires `σ_t` to grow **4.50×**
    /// between 14.6 h and 967 h. With Eq (5c)'s `UO₂` value
    /// (`OPF = 0.157` at 1600 degC for `T_B = 950 degC`, `t_B = 500 FPD`) the
    /// ceiling is **3.14×** — violated. With Eq (5a) for `(Th,U)O₂` at the
    /// report's own AVR value `N = 5` (`OPF = 0.036`) the ceiling is 10.2×,
    /// which is not.
    ///
    /// The exclusion survives the calibration caveat: under the `k = 6/7`
    /// reading the required growth falls to 3.63×, still above 3.14×.
    ///
    /// This is a **positive identification of the kernel from the figure's
    /// own output**, and it agrees with AVR GO 2 being a thorium fuel
    /// element. It is also a reminder that the ceiling is low: PANAMA's
    /// pressure cannot grow without bound once the oxygen term is present.
    #[test]
    fn figure_8_excludes_the_uranium_oxide_oxygen_correlation() {
        let p = avr_go2_particle();
        let first = stress_implied_by(2.126e-6, p.median_strength, p.weibull_modulus);
        let last = stress_implied_by(9.23e-3, p.median_strength, p.weibull_modulus);
        let required = last / first;
        assert!(
            (4.3..4.7).contains(&required),
            "Fig. 8 requires sigma_t to grow ~4.5x, got {required:.2}"
        );

        let hot = ThermodynamicTemperature::new::<degree_celsius>(1600.0);
        let fkor_ratio = {
            let mut a = AccidentHistory::new(p, zero());
            let mut b = AccidentHistory::new(p, zero());
            a.run_isothermal(hot, Time::new::<hour>(967.0), 967)
                .thinning_factor
                .get::<ratio>()
                / b.run_isothermal(hot, Time::new::<hour>(14.6), 15)
                    .thinning_factor
                    .get::<ratio>()
        };
        let ceiling = |opf: Ratio| {
            let o = opf.get::<ratio>();
            (super::super::STABLE_FISSION_GAS_YIELD + o) / o * fkor_ratio
        };

        let uo2 = OxygenSource::UraniumOxide {
            irradiation_temperature: ThermodynamicTemperature::new::<degree_celsius>(950.0),
            irradiation_time: Time::new::<day>(500.0),
        }
        .oxygen_per_fission(hot);
        assert!(
            ceiling(uo2) < required,
            "the UO2 correlation caps sigma_t growth at {:.2}x, below the {required:.2}x \
             Fig. 8 needs -- no D_S can rescue it",
            ceiling(uo2)
        );
        // Even on the k = 6/7 calibration reading, which shrinks the demand.
        assert!(ceiling(uo2) < required.powf(6.0 / 7.0));

        let thoria = p.oxygen.oxygen_per_fission(hot);
        assert!(
            ceiling(thoria) > required * 2.0,
            "the (Th,U)O2 correlation leaves ample headroom ({:.1}x)",
            ceiling(thoria)
        );
    }

    /// **Fig. 8, code-to-data: PANAMA again under-predicts with `η̇ ≡ 0` and
    /// over-predicts with it on — inside its own claimed valid range.**
    ///
    /// Methodology: the measured AVR GO 2 points against the report's own two
    /// curves, in `log₁₀`. No geometry and no part of this implementation
    /// enter. The caption's case is **70/26 at 8.2 % FIMA**, which is the
    /// burnup the PANAMA curve is drawn for; 70/7 (7.2 %) and 70/15 (7.1 %)
    /// are the same experiment at other burnups and are reported as spread.
    ///
    /// Results, 2026-09-24:
    ///
    /// | comparator | n | mean | mean \|·\| | worst |
    /// |---|---|---|---|---|
    /// | `without` corrosion, 70/26 only | 4 | **+0.24** | 0.43 | +0.47 |
    /// | `without` corrosion, all burnups | 9 | **+0.45** | 0.53 | +1.15 |
    /// | `with` corrosion, all burnups | 9 | −1.52 | 1.52 | −2.74 |
    ///
    /// This matters more than Fig. 7's equivalent: page -479- claims good
    /// agreement **1600–2500 degC** and concedes over-conservatism below it,
    /// so Fig. 7's 1400/1500 degC stages had an excuse and Fig. 8, isothermal
    /// at 1600 degC, has none. The `η̇ ≡ 0` case is **not** conservative
    /// here — it sits below the data by a quarter of a decade on the
    /// caption's own burnup, and the 70/26 residual changes sign at 302 h.
    ///
    /// Under the `k = 6/7` calibration reading the 70/26 mean becomes +0.20
    /// and the mean \|·\| 0.37; the conclusion does not change.
    #[test]
    fn figure_8_brackets_the_measurement() {
        // (t h, measured, PANAMA without gb, PANAMA with gb)
        let caption_case: [(f64, f64, f64, f64); 4] = [
            (22.5, 1.011e-5, 3.399e-6, 4.386e-5),
            (53.6, 3.109e-5, 1.104e-5, 5.631e-4),
            (101.9, 7.633e-5, 2.951e-5, 3.927e-3),
            (301.9, 1.126e-4, 2.786e-4, 6.250e-2),
        ];
        let other_burnups: [(f64, f64, f64, f64); 5] = [
            (87.0, 8.459e-5, 2.264e-5, 2.467e-3),
            (156.8, 1.449e-4, 6.915e-5, 1.413e-2),
            (200.0, 2.258e-4, 1.107e-4, 2.687e-2),
            (35.8, 3.127e-5, 6.178e-6, 1.615e-4),
            (140.5, 7.917e-4, 5.584e-5, 1.045e-2),
        ];
        let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;

        let caption_without: Vec<f64> = caption_case
            .iter()
            .map(|(_, m, no_gb, _)| (m / no_gb).log10())
            .collect();
        assert!(
            (0.15..0.35).contains(&mean(&caption_without)),
            "70/26 against the eta_dot = 0 curve measured +0.24 decades, got {:.3}",
            mean(&caption_without)
        );
        // The residual changes sign inside the measured window -- the model
        // crosses the data rather than tracking it.
        assert!(
            caption_without[0] > 0.0 && caption_without[3] < 0.0,
            "the 70/26 residual changes sign by 302 h: {caption_without:?}"
        );

        let all: Vec<f64> = caption_case
            .iter()
            .chain(other_burnups.iter())
            .map(|(_, m, no_gb, _)| (m / no_gb).log10())
            .collect();
        let all_with: Vec<f64> = caption_case
            .iter()
            .chain(other_burnups.iter())
            .map(|(_, m, _, gb)| (m / gb).log10())
            .collect();
        assert!(
            (0.35..0.55).contains(&mean(&all)),
            "all nine points: +0.45 decades expected, got {:.3}",
            mean(&all)
        );
        assert!(
            (-1.65..-1.40).contains(&mean(&all_with)),
            "with corrosion the over-prediction measured -1.52 decades, got {:.3}",
            mean(&all_with)
        );
        // Same picture as Fig. 7: the data lies between, nearer "without".
        assert!(mean(&all).abs() < mean(&all_with).abs());
    }

    /// The chain composes and the reported rates are the differences the
    /// symbol list (page -511-) defines them to be.
    #[test]
    fn the_reported_rates_are_delta_phi_over_delta_t() {
        let mut h = AccidentHistory::new(particle(), zero());
        let before = h.progress();
        let step = AccidentStep {
            duration: Time::new::<hour>(50.0),
            mean_temperature: ThermodynamicTemperature::new::<degree_celsius>(1800.0),
        };
        let after = h.step(step);
        let dt = step.duration.get::<second>();
        let want = (after.total.get::<ratio>() - before.total.get::<ratio>()) / dt;
        assert!(
            (after.total_rate.get::<hertz>() - want).abs() < 1e-18,
            "{} vs {want}",
            after.total_rate.get::<hertz>()
        );
        assert!(after.total_rate.get::<hertz>() >= 0.0);
        assert!(
            h.pressure_at(step.duration, step.mean_temperature)
                .get::<pascal>()
                > 0.0
        );
    }

    /// Grain-boundary corrosion is off by default and only acts when asked —
    /// and when it acts, it worsens the answer (Figs. 7/8).
    #[test]
    fn grain_boundary_corrosion_is_opt_in() {
        let t = ThermodynamicTemperature::new::<degree_celsius>(1600.0);
        let total = Time::new::<hour>(1000.0);

        let mut off = AccidentHistory::new(particle(), zero());
        let a = off.run_isothermal(t, total, 100);
        assert_eq!(a.grain_boundary_exposure.get::<ratio>(), 0.0);

        let mut p = particle();
        p.grain_boundary = GrainBoundaryCorrosion::Enabled;
        let mut on = AccidentHistory::new(p, zero());
        let b = on.run_isothermal(t, total, 100);
        assert!(b.grain_boundary_exposure.get::<ratio>() > 0.0);
        assert!(
            b.pressure_vessel > a.pressure_vessel,
            "with-corrosion must sit above without-corrosion, as Figs. 7/8 show"
        );
        assert_eq!(
            particle().grain_boundary,
            GrainBoundaryCorrosion::Disabled,
            "the report's own default is off (page -495-)"
        );
    }

    /// `φ₁` starts at its end-of-irradiation value, not at zero (page -482-).
    #[test]
    fn phi_1_starts_from_the_end_of_irradiation_value() {
        let start = Ratio::new::<ratio>(1.0e-4);
        let h = AccidentHistory::new(particle(), start);
        assert_eq!(h.progress().pressure_vessel, start);
        assert!(
            (h.progress().total.get::<ratio>() - start.get::<ratio>()).abs() / start.get::<ratio>()
                < 1e-12,
            "with phi_o = phi_2 = 0 the total IS phi_1"
        );
    }
}
