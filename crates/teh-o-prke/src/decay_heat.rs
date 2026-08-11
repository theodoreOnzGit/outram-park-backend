//! # Fission-product decay heat (23-group, 1978 draft ANS Standard)
//!
//! Decay heat is the power released by the radioactive decay of fission
//! products after fission itself has stopped. It is what makes a reactor
//! impossible to simply switch off, and it is the entire subject of a passive
//! decay-heat-removal safety case.
//!
//! ## What belongs in this module
//!
//! The group-fit decay-heat model and its tabulated parameters, plus the
//! time-integration of those groups against a fission-power history. What does
//! **not** belong here: neutron kinetics (see [`crate::zero_power_prke`] and
//! friends), heat transfer, or any reactor-specific geometry.
//!
//! ## Data source
//!
//! Parameters are Table 16 of
//!
//! > Tobias, A., "Decay heat", *Progress in Nuclear Energy*, table titled
//! > "Parameters for fission product decay heat functions of 1978 draft ANS
//! > Standard (England *et al.*, 1978)", p. 78.
//!
//! Tobias reproduces the fit of England *et al.* (1978) that forms the basis of
//! the 1978 proposed ANS Standard. Tobias records that the burst function is
//! reproduced by this 23-exponential sum "to within a few tenths of a percent,
//! for cooling times of up to 1e9 sec".
//!
//! Three fissioning nuclides are tabulated — see [`FissioningNuclide`]. The
//! numerical parameters are physical measurements and are used here as facts;
//! the source publication itself is catalogued separately and is not
//! redistributed.
//!
//! ## Model
//!
//! For a single fissile nuclide the decay-heat *burst* function, the power
//! released at time `t` seconds after a single fission, is Tobias eq. (32):
//!
//! ```text
//! m(t) = sum_{i=1..23} alpha_i * exp(-lambda_i * t)      [MeV / (fission . s)]
//! ```
//!
//! and the integral decay heat after an irradiation of `I` seconds at constant
//! fission rate is eq. (33):
//!
//! ```text
//! M(I,t) = sum_{i=1..23} (alpha_i / lambda_i) * exp(-lambda_i * t)
//!                        * (1 - exp(-lambda_i * I))      [MeV / fission]
//! ```
//!
//! Tobias notes that an infinite irradiation is represented in eq. (33) by
//! `I = 1e13 s`.
//!
//! This module integrates the equivalent per-group differential form, which is
//! what a transient simulation needs because the fission power is not constant:
//!
//! ```text
//! dH_i/dt = alpha_i * F(t) - lambda_i * H_i        H_i has units of power
//! P_decay(t) = sum_i H_i(t)
//! ```
//!
//! where `F(t)` is the fission rate. A single fission at `t = 0` sets
//! `H_i = alpha_i` and then decays as `alpha_i * exp(-lambda_i t)`, recovering
//! eq. (32) exactly; holding `F` constant for `I` seconds and then decaying
//! recovers eq. (33) exactly. The two published forms are therefore special
//! cases of what is integrated here, which is the property the unit tests check.
//!
//! ## Why the update is analytic and not an explicit Euler step
//!
//! **This matters, and it is why the previous placeholder could not have
//! worked.** The decay constants span
//! `lambda = 2.2138e+01` down to `1.5699e-14` per second — **fifteen orders of
//! magnitude**. The fastest group has a time constant of about 45 ms. An
//! explicit update would need a timestep below that for stability, so at the
//! 0.1-1 s timesteps these simulators actually run, the fast groups would blow
//! up or oscillate.
//!
//! [`DecayHeat::advance_timestep`] therefore integrates each group
//! **analytically** over the step, treating the fission power as constant
//! across it:
//!
//! ```text
//! H_i(t+dt) = H_i(t) * exp(-lambda_i * dt)
//!           + (alpha_i * F / lambda_i) * (1 - exp(-lambda_i * dt))
//! ```
//!
//! This is exact for a piecewise-constant fission power, unconditionally
//! stable at any timestep, and costs one `exp` per group per step.
//!
//! ## Status
//!
//! **AI-assisted implementation, not yet human-reviewed** — see
//! `RESPONSIBLE_USE.md` and `VERIFICATION_AND_VALIDATION.md`. The unit tests in
//! this file verify the implementation against the source's own published
//! equations and against the total decay energy per fission; they do **not**
//! validate the model against a measured decay-heat transient.

use uom::si::energy::megaelectronvolt;
use uom::si::f64::*;
use uom::si::power::watt;
use uom::si::time::second;
use uom::ConstZero;

/// Number of exponential groups in the 1978 draft ANS Standard fit.
///
/// Fixed at 23 by the published fit (Tobias Table 16); it is not a tuning
/// parameter.
pub const DECAY_HEAT_GROUPS: usize = 23;

/// Recoverable energy released per fission, used to convert a fission **power**
/// into a fission **rate** so the group parameters (which are per fission) can
/// be applied.
///
/// 200 MeV is the conventional round figure for thermal fission of U-235; the
/// true value depends on nuclide and on how much escaping neutrino energy is
/// excluded. Treating it as exactly 200 MeV introduces a systematic error of a
/// couple of percent in the absolute decay-heat level. Callers who need better
/// should use [`DecayHeat::with_energy_per_fission`].
pub const NOMINAL_ENERGY_PER_FISSION_MEV: f64 = 200.0;

/// The fissioning nuclide whose decay-heat parameters are in use.
///
/// The three cases tabulated by the 1978 draft ANS Standard. Real fuel is a
/// mixture, and Tobias eq. (34) sums over nuclides weighted by their fractional
/// fission rates; this enum selects one at a time, so a mixture must be handled
/// by summing several [`DecayHeat`] instances (see
/// [`DecayHeat::total_decay_heat_power`] and the module tests).
///
/// An enum rather than a trait object, per the workspace design rules: the set
/// of tabulated nuclides is closed and known at compile time, so adding one is
/// a compile error at every match site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FissioningNuclide {
    /// Thermal fission of U-235. Derived from decay-heat functions in
    /// England *et al.* (1978). Total decay energy 13.18 MeV/fission.
    U235Thermal,
    /// Fast fission of U-238. Tobias notes that for U-238 **only summation
    /// results were used**, rather than the fitted-to-measurement route used
    /// for the other two, so this column carries a different pedigree.
    /// Total decay energy 16.24 MeV/fission.
    U238Fast,
    /// Thermal fission of Pu-239. Total decay energy 10.93 MeV/fission.
    Pu239Thermal,
}

/// Thermal fission of U-235: `(alpha_i, lambda_i)`.
///
/// `alpha` in MeV/(fission.s), `lambda` in 1/s. Tobias Table 16, column 1.
const U235_THERMAL: [(f64, f64); DECAY_HEAT_GROUPS] = [
    (6.5057e-01, 2.2138e+01),
    (5.1264e-01, 5.1587e-01),
    (2.4384e-01, 1.9594e-01),
    (1.3850e-01, 1.0314e-01),
    (5.5440e-02, 3.3656e-02),
    (2.2225e-02, 1.1681e-02),
    (3.3088e-03, 3.5870e-03),
    (9.3015e-04, 1.3930e-03),
    (8.0943e-04, 6.2630e-04),
    (1.9567e-04, 1.8906e-04),
    (3.2535e-05, 5.4988e-05),
    (7.5595e-06, 2.0958e-05),
    (2.5232e-06, 1.0010e-05),
    (4.9948e-07, 2.5438e-06),
    (1.8531e-07, 6.6361e-07),
    (2.6608e-08, 1.2290e-07),
    (2.2398e-09, 2.7213e-08),
    (8.1641e-12, 4.3714e-09),
    (8.7797e-11, 7.5780e-10),
    (2.5131e-14, 2.4786e-10),
    (3.2176e-16, 2.2384e-13),
    (4.5038e-17, 2.4600e-14),
    (7.4791e-17, 1.5699e-14),
];

/// Fast fission of U-238: `(alpha_i, lambda_i)`.
///
/// `alpha` in MeV/(fission.s), `lambda` in 1/s. Tobias Table 16, column 2.
const U238_FAST: [(f64, f64); DECAY_HEAT_GROUPS] = [
    (1.2311e+00, 3.2881e+00),
    (1.1486e+00, 9.3805e-01),
    (7.0701e-01, 3.7073e-01),
    (2.5209e-01, 1.1118e-01),
    (7.1870e-02, 3.6143e-02),
    (2.8291e-02, 1.3272e-02),
    (6.8382e-03, 5.0133e-03),
    (1.2322e-03, 1.3655e-03),
    (6.8409e-04, 5.5158e-04),
    (1.6975e-04, 1.7873e-04),
    (2.4182e-05, 4.9032e-05),
    (6.6356e-06, 1.7058e-05),
    (1.0075e-06, 7.0465e-06),
    (4.9894e-07, 2.3190e-06),
    (1.6352e-07, 6.4480e-07),
    (2.3355e-08, 1.2649e-07),
    (2.8094e-09, 2.5548e-08),
    (3.6236e-11, 8.4782e-09),
    (6.4577e-11, 7.5130e-10),
    (4.4963e-14, 2.4188e-10),
    (3.6654e-16, 2.2739e-13),
    (5.6293e-17, 9.0536e-14),
    (7.1602e-17, 5.6098e-15),
];

/// Thermal fission of Pu-239: `(alpha_i, lambda_i)`.
///
/// `alpha` in MeV/(fission.s), `lambda` in 1/s. Tobias Table 16, column 3.
const PU239_THERMAL: [(f64, f64); DECAY_HEAT_GROUPS] = [
    (2.0830e-01, 1.0020e+01),
    (3.8530e-01, 6.4330e-01),
    (2.2130e-01, 2.1860e-01),
    (9.4600e-02, 1.0040e-01),
    (3.5310e-02, 3.7280e-02),
    (2.2920e-02, 1.4350e-02),
    (3.9460e-03, 4.5490e-03),
    (1.3170e-03, 1.3280e-03),
    (7.0520e-04, 5.3560e-04),
    (1.4320e-04, 1.7300e-04),
    (1.7650e-05, 4.8810e-05),
    (7.3470e-06, 2.0060e-05),
    (1.7470e-06, 8.3190e-06),
    (5.4810e-07, 2.3580e-06),
    (1.6710e-07, 6.4500e-07),
    (2.1120e-08, 1.2780e-07),
    (2.9960e-09, 2.4660e-08),
    (5.1070e-11, 9.3780e-09),
    (5.7300e-11, 7.4500e-10),
    (4.1380e-14, 2.4260e-10),
    (1.0880e-15, 2.2100e-13),
    (2.4540e-17, 2.6400e-14),
    (7.5570e-17, 1.3800e-14),
];

impl FissioningNuclide {
    /// The 23 `(alpha_i, lambda_i)` pairs for this nuclide.
    ///
    /// `alpha` is in MeV per fission per second; `lambda` is a decay constant
    /// in reciprocal seconds. Groups are ordered by decreasing `lambda`
    /// (fastest-decaying group first), as printed in the source table.
    pub fn parameters(&self) -> &'static [(f64, f64); DECAY_HEAT_GROUPS] {
        match self {
            Self::U235Thermal => &U235_THERMAL,
            Self::U238Fast => &U238_FAST,
            Self::Pu239Thermal => &PU239_THERMAL,
        }
    }

    /// Total decay energy released per fission over infinite time following a
    /// single fission, `sum_i alpha_i / lambda_i`, in MeV per fission.
    ///
    /// This is the `t = 0`, `I -> infinity` limit of Tobias eq. (33) and is a
    /// useful physical sanity check on the tabulated data: it should land near
    /// the ~13 MeV of beta and gamma energy released per fission (neutrino
    /// energy is excluded, being unrecoverable).
    ///
    /// Measured from the tabulated data on 2026-08-11:
    /// U-235 thermal 13.183, U-238 fast 16.244, Pu-239 thermal 10.932
    /// MeV/fission.
    pub fn total_decay_energy_per_fission(&self) -> Energy {
        let mev: f64 = self.parameters().iter().map(|(a, l)| a / l).sum();
        Energy::new::<megaelectronvolt>(mev)
    }
}

/// Fission-product decay-heat state: one stored power per exponential group.
///
/// Construct with [`DecayHeat::new`] (all groups cold, as at first startup of
/// fresh fuel) or [`DecayHeat::new_at_equilibrium`] (groups saturated to an
/// infinite prior irradiation, which is the realistic starting point for a
/// shutdown transient). Advance with [`DecayHeat::advance_timestep`] and read
/// with [`DecayHeat::total_decay_heat_power`].
///
/// Owns its data by value; no lifetimes, no heap allocation.
#[derive(Clone, Copy, Debug)]
pub struct DecayHeat {
    /// Which tabulated nuclide's parameters are in use.
    nuclide: FissioningNuclide,
    /// Per-group stored decay power `H_i`. Summing these gives the total
    /// decay-heat power. Never negative for non-negative fission power.
    group_power: [Power; DECAY_HEAT_GROUPS],
    /// Recoverable energy per fission, used to turn fission power into fission
    /// rate. Defaults to [`NOMINAL_ENERGY_PER_FISSION_MEV`].
    energy_per_fission: Energy,
}

impl Default for DecayHeat {
    /// Thermal U-235, all groups cold.
    ///
    /// Cold rather than saturated because a caller that never irradiates should
    /// see zero decay heat rather than a spurious source. For a shutdown
    /// transient use [`DecayHeat::new_at_equilibrium`] instead.
    fn default() -> Self {
        Self::new(FissioningNuclide::U235Thermal)
    }
}

impl DecayHeat {
    /// A cold decay-heat state for `nuclide`: no fission products present, so
    /// the initial decay-heat power is exactly zero.
    pub fn new(nuclide: FissioningNuclide) -> Self {
        Self {
            nuclide,
            group_power: [Power::ZERO; DECAY_HEAT_GROUPS],
            energy_per_fission: Energy::new::<megaelectronvolt>(
                NOMINAL_ENERGY_PER_FISSION_MEV,
            ),
        }
    }

    /// A decay-heat state saturated to an infinite prior irradiation at
    /// `fission_power`.
    ///
    /// Every group is placed at its equilibrium value
    /// `H_i = alpha_i * F / lambda_i`, the `I -> infinity` limit of Tobias
    /// eq. (33). This is the correct starting point for a shutdown or
    /// loss-of-cooling transient in a reactor that has been running a long
    /// time; starting cold would understate decay heat drastically and
    /// flatter any passive-cooling result.
    ///
    /// Note the longest-lived groups have `lambda ~ 1.6e-14 /s`, a half-life of
    /// order a million years, so "infinite irradiation" is a real idealisation:
    /// those groups never saturate in any physical reactor. Tobias represents
    /// infinite irradiation as `I = 1e13 s`.
    pub fn new_at_equilibrium(nuclide: FissioningNuclide, fission_power: Power) -> Self {
        let mut decay_heat = Self::new(nuclide);
        let fission_rate_per_second = decay_heat.fission_rate_per_second(fission_power);
        for (index, (alpha, lambda)) in nuclide.parameters().iter().enumerate() {
            let equilibrium_mev_per_second = alpha * fission_rate_per_second / lambda;
            decay_heat.group_power[index] =
                Power::new::<watt>(equilibrium_mev_per_second * JOULE_PER_MEV);
        }
        decay_heat
    }

    /// Override the recoverable energy per fission (default
    /// [`NOMINAL_ENERGY_PER_FISSION_MEV`]).
    ///
    /// Only affects the fission-power to fission-rate conversion, so it scales
    /// the decay-heat power inversely and linearly.
    pub fn with_energy_per_fission(mut self, energy_per_fission: Energy) -> Self {
        self.energy_per_fission = energy_per_fission;
        self
    }

    /// The nuclide whose parameters this state uses.
    pub fn nuclide(&self) -> FissioningNuclide {
        self.nuclide
    }

    /// Fission rate in fissions per second implied by a fission power.
    fn fission_rate_per_second(&self, fission_power: Power) -> f64 {
        let watts = fission_power.get::<watt>();
        let joules_per_fission = self.energy_per_fission.get::<megaelectronvolt>() * JOULE_PER_MEV;
        watts / joules_per_fission
    }

    /// Advance every decay-heat group by `timestep`, holding `fission_power`
    /// constant across the step.
    ///
    /// Integrates each group **analytically**:
    ///
    /// ```text
    /// H_i(t+dt) = H_i(t) * exp(-lambda_i dt)
    ///           + (alpha_i F / lambda_i) * (1 - exp(-lambda_i dt))
    /// ```
    ///
    /// which is exact for a piecewise-constant fission power and
    /// unconditionally stable. See the module docs for why an explicit update
    /// is unusable here: the decay constants span fifteen orders of magnitude
    /// and the fastest group has a ~45 ms time constant.
    ///
    /// `fission_power` is the **prompt fission power only**. Do not add the
    /// decay heat back in here — that would be a feedback loop counting the
    /// same energy twice.
    pub fn advance_timestep(&mut self, fission_power: Power, timestep: Time) {
        let dt = timestep.get::<second>();
        let fission_rate_per_second = self.fission_rate_per_second(fission_power);

        for (index, (alpha, lambda)) in self.nuclide.parameters().iter().enumerate() {
            let decay_factor = (-lambda * dt).exp();
            let source_mev_per_second = alpha * fission_rate_per_second / lambda;
            let source = Power::new::<watt>(source_mev_per_second * JOULE_PER_MEV);

            self.group_power[index] =
                self.group_power[index] * decay_factor + source * (1.0 - decay_factor);
        }
    }

    /// Total fission-product decay-heat power, the sum over all 23 groups.
    ///
    /// Always non-negative for non-negative fission power, so callers must not
    /// need `.abs()` on the result — if a caller is taking an absolute value
    /// here, that is masking a sign error elsewhere rather than fixing one.
    pub fn total_decay_heat_power(&self) -> Power {
        self.group_power
            .iter()
            .fold(Power::ZERO, |total, group| total + *group)
    }

    /// The fraction of fission power released *promptly*, i.e. everything
    /// except the energy that emerges later as fission-product decay heat.
    ///
    /// Equals `1 - (total decay energy per fission) / (energy per fission)`.
    /// A caller that adds [`DecayHeat::total_decay_heat_power`] on top of the
    /// prompt fission power must scale the prompt term by this, or it counts
    /// the decay energy twice and overstates total thermal power.
    ///
    /// At equilibrium the two terms sum back to the full fission power by
    /// construction, which is the property that makes this self-consistent
    /// rather than a tuned constant. For U-235 thermal at the nominal 200
    /// MeV/fission this is 1 - 13.183/200 = 0.9341.
    pub fn prompt_power_fraction(&self) -> Ratio {
        let decay = self.nuclide.total_decay_energy_per_fission();
        Ratio::new::<uom::si::ratio::ratio>(1.0)
            - decay / self.energy_per_fission
    }

    /// The stored power of a single group, for inspection and testing.
    ///
    /// Returns `None` when `group_index >= DECAY_HEAT_GROUPS`. Groups are
    /// ordered fastest-decaying first, matching the source table.
    pub fn group_power(&self, group_index: usize) -> Option<Power> {
        self.group_power.get(group_index).copied()
    }
}

/// Joules per megaelectronvolt.
///
/// The CODATA elementary charge times 1e6. Used to convert the tabulated
/// MeV-based parameters into SI power.
const JOULE_PER_MEV: f64 = 1.602_176_634e-13;

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::power::megawatt;
    use uom::si::time::{day, hour};

    /// **Methodology.** Sum `alpha_i / lambda_i` over all 23 groups for each
    /// tabulated nuclide. This is the `t = 0`, infinite-irradiation limit of
    /// Tobias eq. (33) and equals the total beta plus gamma energy released per
    /// fission. Pass criterion: each nuclide lands in 10-17 MeV/fission, the
    /// physically expected band, and the ordering is
    /// Pu-239 < U-235 < U-238-fast.
    ///
    /// **Results (2026-08-11).** U-235 thermal 13.183, U-238 fast 16.244,
    /// Pu-239 thermal 10.932 MeV/fission. All within band and correctly
    /// ordered. This is a transcription check on Table 16 as much as a physics
    /// check: a single mistyped mantissa in a low-index group would move these
    /// sums well outside the band.
    #[test]
    fn total_decay_energy_per_fission_is_physical() {
        let u235 = FissioningNuclide::U235Thermal
            .total_decay_energy_per_fission()
            .get::<megaelectronvolt>();
        let u238 = FissioningNuclide::U238Fast
            .total_decay_energy_per_fission()
            .get::<megaelectronvolt>();
        let pu239 = FissioningNuclide::Pu239Thermal
            .total_decay_energy_per_fission()
            .get::<megaelectronvolt>();

        for (name, value) in [("U235", u235), ("U238", u238), ("Pu239", pu239)] {
            assert!(
                (10.0..=17.0).contains(&value),
                "{name} total decay energy {value} MeV/fission outside the physical band"
            );
        }
        assert!(pu239 < u235, "Pu-239 should release less than U-235");
        assert!(u235 < u238, "U-238 fast should release more than U-235");
    }

    /// **Methodology.** Every tabulated column must have 23 groups, strictly
    /// decreasing `lambda`, and strictly positive `alpha` and `lambda`. The
    /// monotonicity check is a transcription guard: the source prints the
    /// groups ordered by decreasing decay constant, so an out-of-order value
    /// indicates a mis-read row.
    ///
    /// **Results (2026-08-11).** All three columns pass: 23 groups each,
    /// `lambda` strictly decreasing from 2.2138e+01 to 1.5699e-14 (U-235),
    /// 3.2881e+00 to 5.6098e-15 (U-238), 1.0020e+01 to 1.3800e-14 (Pu-239).
    #[test]
    fn tabulated_parameters_are_well_formed() {
        for nuclide in [
            FissioningNuclide::U235Thermal,
            FissioningNuclide::U238Fast,
            FissioningNuclide::Pu239Thermal,
        ] {
            let parameters = nuclide.parameters();
            assert_eq!(parameters.len(), DECAY_HEAT_GROUPS);
            for (alpha, lambda) in parameters.iter() {
                assert!(*alpha > 0.0, "{nuclide:?} has a non-positive alpha");
                assert!(*lambda > 0.0, "{nuclide:?} has a non-positive lambda");
            }
            for pair in parameters.windows(2) {
                assert!(
                    pair[0].1 > pair[1].1,
                    "{nuclide:?} decay constants are not strictly decreasing"
                );
            }
        }
    }

    /// **Methodology.** A cold state advanced with zero fission power must stay
    /// at exactly zero decay heat, and a cold state must report zero before any
    /// irradiation. Guards against a spurious source term.
    ///
    /// **Results (2026-08-11).** Zero in both cases, exactly.
    #[test]
    fn cold_state_produces_no_decay_heat() {
        let mut decay_heat = DecayHeat::new(FissioningNuclide::U235Thermal);
        assert_eq!(decay_heat.total_decay_heat_power().get::<watt>(), 0.0);
        decay_heat.advance_timestep(Power::ZERO, Time::new::<second>(1.0));
        assert_eq!(decay_heat.total_decay_heat_power().get::<watt>(), 0.0);
    }

    /// **Methodology.** Integrate a cold state forward at constant fission
    /// power and compare against the closed form of Tobias eq. (33), which for
    /// an irradiation of `I` seconds at constant fission rate `F` and `t = 0`
    /// gives a decay-heat power of
    /// `sum_i (alpha_i F / lambda_i) (1 - exp(-lambda_i I))`.
    /// Irradiate 1 MW of fission power for 1 hour in 0.1 s steps. Pass
    /// criterion: agreement within 0.1 % relative.
    ///
    /// This is the central correctness test: it demonstrates the per-group
    /// differential form integrated by [`DecayHeat::advance_timestep`] is
    /// equivalent to the published integral equation.
    ///
    /// **Results (2026-08-11).** Closed-form eq. (33) after 1 h at 1 MW gives
    /// **5.27890e+04 W**; the 36 000-step integration agrees with it to well
    /// inside the 0.1 % criterion. The agreement is exact rather than merely
    /// close because the per-step update is the analytic solution for a
    /// piecewise-constant source, so the only error is floating-point
    /// accumulation.
    #[test]
    fn constant_irradiation_matches_published_integral_equation() {
        let nuclide = FissioningNuclide::U235Thermal;
        let fission_power = Power::new::<megawatt>(1.0);
        let irradiation = Time::new::<hour>(1.0);
        let timestep = Time::new::<second>(0.1);

        let mut decay_heat = DecayHeat::new(nuclide);
        let steps = (irradiation.get::<second>() / timestep.get::<second>()).round() as u32;
        for _ in 0..steps {
            decay_heat.advance_timestep(fission_power, timestep);
        }
        let integrated = decay_heat.total_decay_heat_power().get::<watt>();

        // Closed form of Tobias eq. (33) at t = 0 after irradiation I.
        let joules_per_fission = NOMINAL_ENERGY_PER_FISSION_MEV * JOULE_PER_MEV;
        let fission_rate = fission_power.get::<watt>() / joules_per_fission;
        let irradiation_seconds = irradiation.get::<second>();
        let analytic: f64 = nuclide
            .parameters()
            .iter()
            .map(|(alpha, lambda)| {
                alpha * fission_rate / lambda * (1.0 - (-lambda * irradiation_seconds).exp())
            })
            .sum::<f64>()
            * JOULE_PER_MEV;

        let relative_error = (integrated - analytic).abs() / analytic;
        assert!(
            relative_error < 1.0e-3,
            "integrated {integrated:e} W vs analytic {analytic:e} W, \
             relative error {relative_error:e}"
        );
    }

    /// **Methodology.** A state saturated by [`DecayHeat::new_at_equilibrium`]
    /// and then advanced at the same fission power must not drift: equilibrium
    /// is by definition the fixed point of the group update. Advance 1000 steps
    /// of 1 s at 10 MW and require the total to be unchanged within 1e-9
    /// relative.
    ///
    /// This is the analogue of the steady-state property test every transient
    /// solver should have, and it directly exercises the stiff fast groups:
    /// with an explicit update these would diverge immediately at a 1 s step.
    ///
    /// **Results (2026-08-11).** Passes: the total held within the 1e-9
    /// relative criterion over all 1000 steps, with no visible drift. The exact
    /// residual is floating-point accumulation and is not asserted to a tighter
    /// figure here, because that would be a claim about round-off behaviour
    /// rather than about the model.
    #[test]
    fn equilibrium_state_does_not_drift() {
        let fission_power = Power::new::<megawatt>(10.0);
        let mut decay_heat =
            DecayHeat::new_at_equilibrium(FissioningNuclide::U235Thermal, fission_power);
        let initial = decay_heat.total_decay_heat_power().get::<watt>();

        for _ in 0..1000 {
            decay_heat.advance_timestep(fission_power, Time::new::<second>(1.0));
        }
        let final_power = decay_heat.total_decay_heat_power().get::<watt>();

        let relative_drift = (final_power - initial).abs() / initial;
        assert!(
            relative_drift < 1.0e-9,
            "equilibrium drifted: {initial:e} -> {final_power:e} W ({relative_drift:e})"
        );
    }

    /// **Methodology.** Decay heat immediately after shutdown from a long
    /// irradiation should be a few percent of the pre-shutdown fission power,
    /// and should fall monotonically thereafter. Saturate at 10 MW, then set
    /// fission power to zero and sample the decay-heat power at 1 s, 1 hour and
    /// 1 day. Pass criterion: the 1 s value lies between 2 % and 12 % of full
    /// power, and the sequence is strictly decreasing.
    ///
    /// The 2-12 % window is deliberately loose. It is a shape check, not a
    /// validation against a measured decay-heat curve — no such comparison is
    /// made anywhere in this crate yet, so **this model must not be described
    /// as validated**.
    ///
    /// **Results (2026-08-11).** Saturated decay-heat power at 10 MW is
    /// 6.59129e+05 W, i.e. **6.591 % of full power**. After shutdown, at this
    /// test's cumulative sample times: **6.159 %** at 1 s, **1.312 %** at
    /// 1 s + 1 h, and **0.501 %** at a further 1 day (t = 90 001 s). Strictly
    /// decreasing, and the near-shutdown figure sits in the expected
    /// few-percent band for infinite irradiation.
    #[test]
    fn shutdown_decay_heat_has_the_expected_magnitude_and_falls() {
        let full_power = Power::new::<megawatt>(10.0);
        let mut decay_heat =
            DecayHeat::new_at_equilibrium(FissioningNuclide::U235Thermal, full_power);

        let sample_after = |decay_heat: &mut DecayHeat, duration: Time, steps: u32| {
            let dt = duration / steps as f64;
            for _ in 0..steps {
                decay_heat.advance_timestep(Power::ZERO, dt);
            }
            decay_heat.total_decay_heat_power() / full_power
        };

        let at_one_second = sample_after(&mut decay_heat, Time::new::<second>(1.0), 10);
        let at_one_hour = sample_after(&mut decay_heat, Time::new::<hour>(1.0), 360);
        let at_one_day = sample_after(&mut decay_heat, Time::new::<day>(1.0), 240);

        let fraction = |ratio: Ratio| ratio.get::<uom::si::ratio::ratio>();
        assert!(
            (0.02..=0.12).contains(&fraction(at_one_second)),
            "decay heat 1 s after shutdown was {} of full power",
            fraction(at_one_second)
        );
        assert!(
            fraction(at_one_hour) < fraction(at_one_second),
            "decay heat should fall between 1 s and 1 hour"
        );
        assert!(
            fraction(at_one_day) < fraction(at_one_hour),
            "decay heat should fall between 1 hour and 1 day"
        );
    }

    /// **Methodology.** Reproduce Tobias eq. (32), the single-fission burst
    /// function `m(t) = sum_i alpha_i exp(-lambda_i t)`, by construction and
    /// check it decreases monotonically over 0 to 1e6 s. Confirms the group
    /// ordering and signs are consistent.
    ///
    /// **Results (2026-08-11).** Monotonically decreasing at every sampled
    /// decade from 1e-3 s to 1e6 s for all three nuclides.
    #[test]
    fn burst_function_decreases_monotonically() {
        for nuclide in [
            FissioningNuclide::U235Thermal,
            FissioningNuclide::U238Fast,
            FissioningNuclide::Pu239Thermal,
        ] {
            let burst = |t: f64| -> f64 {
                nuclide
                    .parameters()
                    .iter()
                    .map(|(alpha, lambda)| alpha * (-lambda * t).exp())
                    .sum()
            };
            let mut previous = burst(1.0e-3);
            for decade in -2..=6 {
                let value = burst(10f64.powi(decade));
                assert!(
                    value < previous,
                    "{nuclide:?} burst function not decreasing at 1e{decade} s"
                );
                previous = value;
            }
        }
    }
}
