//! # ANS-5.1 decay-heat machinery: standards, isotope mix, `F_gamma` (gh:#1080)
//!
//! RELAP-7 Theory Manual (INL/EXT-14-31366), §7.2, Eqs (417)-(420). For each
//! fissioning isotope `alpha` and exponential group `j`:
//!
//! ```text
//! d gamma_aj/dt = F_gamma a_aj / lambda_aj  F_a psi(t) - lambda_aj gamma_aj     (417)
//! P'_gamma(t)   = sum_a sum_j lambda_aj gamma_aj                                (418)
//! P_gamma(t)    = G(t) P'_gamma(t)                                              (420)
//! ```
//!
//! `psi` is the total fission rate, `F_a` the fraction of fissions in isotope
//! `a` (summing to 1), `F_gamma` a conservatism factor (1 for best estimate).
//! This module integrates `H_aj = lambda_aj gamma_aj` (units of power) with
//! the same analytic per-step update as [`crate::decay_heat::DecayHeat`],
//! exact for a fission rate held constant over the step; the two share
//! [`crate::decay_heat::advance_group_power`].
//!
//! ## Data: what is built in and what is not, and why
//!
//! | [`AnsStandard`] | isotopes | coefficients |
//! |---|---|---|
//! | `Draft1978` (default) | U-235, U-238, Pu-239 | **built in**: Tobias Table 16 (England *et al.* 1978), the set already in [`crate::decay_heat`] |
//! | `Ans1979` | U-235, U-238, Pu-239 | **user supplied** |
//! | `Ans1994` | U-235, U-238, Pu-239, Pu-241 | **user supplied** |
//! | `Ans2005` | U-235, U-238, Pu-239, Pu-241 | **user supplied** |
//!
//! The 1979, 1994 and 2005 23-group coefficient tables are published in the
//! ANSI/ANS-5.1 standards, which are copyrighted documents sold by the
//! American Nuclear Society. `DATA_POLICY.md` admits only open-source,
//! public-literature or properly licensed data. On 2026-11-01 the workspace
//! was searched for an open reproduction of those tables (`reference-data/`,
//! the `reactor-literature` open corpus, every source file): none was found.
//! The RELAP-7 manual, which says its code carries the tables, is held
//! privately and does not print them. Typing the coefficients from memory
//! would be fabrication. So the enum, the isotope slots (including Pu-241)
//! and the machinery are here, and the three standards' coefficients are
//! entered by the user through [`DecayHeatGroupSet::from_tabulated`] or
//! [`DecayHeatGroupSet::new`], from a copy of the standard they hold.
//!
//! The 1979 standard is said (MELCOR DCH documentation, as cited in
//! `sembawang::bwr_mark1::decay`) to rest on the same England *et al.*
//! evaluation as the 1978 draft, but whether its printed coefficients equal
//! Tobias Table 16 digit for digit is **not verified here**, so `Ans1979` is
//! not silently aliased to the draft set.
//!
//! **Default kept: the 1978 draft fit** (gh:#1080 asks for it to stay the
//! default unless the maintainer decides otherwise; recorded 2026-11-01).
//!
//! > Research, education and V&V only. AI draft, unreviewed.

use petir::mathf::RealMath;
use uom::si::energy::megaelectronvolt;
use uom::si::f64::*;
use uom::si::frequency::hertz;
use uom::si::power::watt;
use uom::si::ratio::ratio;
use uom::si::time::second;
use uom::ConstZero;

use crate::decay_heat::{advance_group_power, FissioningNuclide, DECAY_HEAT_GROUPS, JOULE_PER_MEV};
use crate::teh_o_prke_error::TehOPrkeError;

/// Which ANS decay-heat standard's group structure is in use.
///
/// Enum, not trait object: the set is closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AnsStandard {
    /// The 1978 draft ANS standard fit (England *et al.* 1978, as tabulated by
    /// Tobias): three isotopes, coefficients built in. The default.
    #[default]
    Draft1978,
    /// ANSI/ANS-5.1-1979: three isotopes, coefficients user supplied.
    Ans1979,
    /// ANSI/ANS-5.1-1994: four isotopes (adds Pu-241), user supplied.
    Ans1994,
    /// ANSI/ANS-5.1-2005: four isotopes (adds Pu-241), user supplied.
    Ans2005,
}

/// A fissioning isotope slot in a decay-heat standard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecayHeatIsotope {
    /// U-235 (thermal fission)
    U235,
    /// U-238 (fast fission)
    U238,
    /// Pu-239 (thermal fission)
    Pu239,
    /// Pu-241 (thermal fission); only in the 1994 and 2005 standards.
    Pu241,
}

impl DecayHeatIsotope {
    fn index(self) -> usize {
        match self {
            Self::U235 => 0,
            Self::U238 => 1,
            Self::Pu239 => 2,
            Self::Pu241 => 3,
        }
    }
}

const THREE: [DecayHeatIsotope; 3] = [
    DecayHeatIsotope::U235,
    DecayHeatIsotope::U238,
    DecayHeatIsotope::Pu239,
];
const FOUR: [DecayHeatIsotope; 4] = [
    DecayHeatIsotope::U235,
    DecayHeatIsotope::U238,
    DecayHeatIsotope::Pu239,
    DecayHeatIsotope::Pu241,
];

impl AnsStandard {
    /// The isotopes the standard tabulates (`N_I` = 3 or 4, RELAP-7 §7.2).
    pub fn isotopes(&self) -> &'static [DecayHeatIsotope] {
        match self {
            Self::Draft1978 | Self::Ans1979 => &THREE,
            Self::Ans1994 | Self::Ans2005 => &FOUR,
        }
    }

    /// The built-in coefficient set for `isotope`, if this crate carries one.
    /// Only `Draft1978` has built-in data; see the module docs for why.
    pub fn built_in_group_set(&self, isotope: DecayHeatIsotope) -> Option<DecayHeatGroupSet> {
        let nuclide = match (self, isotope) {
            (Self::Draft1978, DecayHeatIsotope::U235) => FissioningNuclide::U235Thermal,
            (Self::Draft1978, DecayHeatIsotope::U238) => FissioningNuclide::U238Fast,
            (Self::Draft1978, DecayHeatIsotope::Pu239) => FissioningNuclide::Pu239Thermal,
            _ => return None,
        };
        Some(DecayHeatGroupSet {
            alpha_lambda: *nuclide.parameters(),
        })
    }

    fn includes(&self, isotope: DecayHeatIsotope) -> bool {
        self.isotopes().contains(&isotope)
    }
}

/// One isotope's 23 `(a_j, lambda_j)` pairs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecayHeatGroupSet {
    /// `a` in MeV/(fission s), `lambda` in 1/s.
    alpha_lambda: [(f64, f64); DECAY_HEAT_GROUPS],
}

impl DecayHeatGroupSet {
    /// From dimensioned coefficients: `alpha[j]` is the power per fission of
    /// group `j` (the standard's `a_j`, printed in MeV/(fission s)) and
    /// `lambda[j]` its decay constant. Every value must be finite and > 0.
    pub fn new(
        alpha: [Power; DECAY_HEAT_GROUPS],
        lambda: [Frequency; DECAY_HEAT_GROUPS],
    ) -> Result<Self, TehOPrkeError> {
        let mut table = [(0.0, 0.0); DECAY_HEAT_GROUPS];
        for j in 0..DECAY_HEAT_GROUPS {
            table[j] = (
                alpha[j].get::<watt>() / JOULE_PER_MEV,
                lambda[j].get::<hertz>(),
            );
        }
        Self::from_tabulated(table)
    }

    /// From the standard's printed table as numbers: `(a_j` in MeV/(fission
    /// s), `lambda_j` in 1/s`)`, 23 rows. Every value must be finite and > 0.
    pub fn from_tabulated(
        alpha_lambda: [(f64, f64); DECAY_HEAT_GROUPS],
    ) -> Result<Self, TehOPrkeError> {
        for (j, (a, l)) in alpha_lambda.iter().enumerate() {
            if !(a.is_finite() && *a > 0.0 && l.is_finite() && *l > 0.0) {
                return Err(TehOPrkeError::GenericStringError(format!(
                    "decay-heat group {j}: a = {a}, lambda = {l}; both must be finite and > 0"
                )));
            }
        }
        Ok(Self { alpha_lambda })
    }

    /// The `(a_j, lambda_j)` pairs, MeV/(fission s) and 1/s.
    pub fn tabulated(&self) -> &[(f64, f64); DECAY_HEAT_GROUPS] {
        &self.alpha_lambda
    }

    /// `sum_j a_j / lambda_j`: total decay energy per fission.
    pub fn total_decay_energy_per_fission(&self) -> Energy {
        let mev: f64 = self.alpha_lambda.iter().map(|(a, l)| a / l).sum();
        Energy::new::<megaelectronvolt>(mev)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct IsotopeSlot {
    set: DecayHeatGroupSet,
    fission_fraction: Ratio,
    group_power: [Power; DECAY_HEAT_GROUPS],
}

/// Mixed-isotope fission-product decay heat under one [`AnsStandard`]
/// (RELAP-7 Eqs 417-418).
///
/// ```
/// use teh_o_prke::prelude::*;
/// use uom::si::f64::*;
/// use uom::si::frequency::hertz;
/// use uom::si::ratio::ratio;
/// use uom::si::time::second;
///
/// let mut dh = AnsDecayHeat::new(AnsStandard::Draft1978)
///     .with_built_in_isotope(DecayHeatIsotope::U235, Ratio::new::<ratio>(0.7))?
///     .with_built_in_isotope(DecayHeatIsotope::Pu239, Ratio::new::<ratio>(0.3))?;
/// dh.advance(Frequency::new::<hertz>(3.1e16), Time::new::<second>(1.0))?;
/// assert!(dh.total_decay_power().value > 0.0);
/// # Ok::<(), TehOPrkeError>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnsDecayHeat {
    standard: AnsStandard,
    slots: [Option<IsotopeSlot>; 4],
    conservatism_factor: Ratio,
}

impl Default for AnsDecayHeat {
    /// `Draft1978`, all fissions in U-235, `F_gamma = 1`, cold.
    fn default() -> Self {
        Self::new(AnsStandard::Draft1978)
            .with_built_in_isotope(DecayHeatIsotope::U235, Ratio::new::<ratio>(1.0))
            .expect("Draft1978 carries U-235")
    }
}

impl AnsDecayHeat {
    /// An empty mix (no isotopes yet), `F_gamma = 1`. Add isotopes with
    /// [`Self::with_built_in_isotope`] or [`Self::with_user_coefficients`].
    pub fn new(standard: AnsStandard) -> Self {
        Self {
            standard,
            slots: [None; 4],
            conservatism_factor: Ratio::new::<ratio>(1.0),
        }
    }

    /// Add `isotope` with fission fraction `F_a`, using the built-in set.
    /// Errors if the standard has no built-in data for it (everything except
    /// `Draft1978`'s three isotopes) or does not include it.
    pub fn with_built_in_isotope(
        self,
        isotope: DecayHeatIsotope,
        fission_fraction: Ratio,
    ) -> Result<Self, TehOPrkeError> {
        let set = self.standard.built_in_group_set(isotope).ok_or_else(|| {
            TehOPrkeError::GenericStringError(format!(
                "{:?} has no built-in coefficients for {isotope:?}: the ANS-5.1 \
                 1979/1994/2005 tables are copyrighted and not in this crate; \
                 supply them with with_user_coefficients",
                self.standard
            ))
        })?;
        self.with_user_coefficients(isotope, fission_fraction, set)
    }

    /// Add `isotope` with fission fraction `F_a` and user-supplied
    /// coefficients. Errors if the standard does not include the isotope
    /// (Pu-241 under `Draft1978`/`Ans1979`) or the fraction is outside [0, 1].
    pub fn with_user_coefficients(
        mut self,
        isotope: DecayHeatIsotope,
        fission_fraction: Ratio,
        set: DecayHeatGroupSet,
    ) -> Result<Self, TehOPrkeError> {
        if !self.standard.includes(isotope) {
            return Err(TehOPrkeError::GenericStringError(format!(
                "{:?} does not tabulate {isotope:?}",
                self.standard
            )));
        }
        let f = fission_fraction.get::<ratio>();
        if !(0.0..=1.0).contains(&f) {
            return Err(TehOPrkeError::GenericStringError(format!(
                "fission fraction {f} outside [0, 1]"
            )));
        }
        self.slots[isotope.index()] = Some(IsotopeSlot {
            set,
            fission_fraction,
            group_power: [Power::ZERO; DECAY_HEAT_GROUPS],
        });
        Ok(self)
    }

    /// Set the conservatism factor `F_gamma` (1 for best estimate; RELAP-7
    /// Eq 417). It multiplies every group source, so it scales the decay
    /// power of a cold-started state linearly.
    pub fn with_conservatism_factor(mut self, f_gamma: Ratio) -> Self {
        self.conservatism_factor = f_gamma;
        self
    }

    /// The standard in use.
    pub fn standard(&self) -> AnsStandard {
        self.standard
    }

    /// `F_gamma`.
    pub fn conservatism_factor(&self) -> Ratio {
        self.conservatism_factor
    }

    /// `sum_a F_a`; must be 1 (to 1e-12) before advancing.
    pub fn total_fission_fraction(&self) -> Ratio {
        self.slots
            .iter()
            .flatten()
            .fold(Ratio::ZERO, |acc, s| acc + s.fission_fraction)
    }

    fn check_fractions(&self) -> Result<(), TehOPrkeError> {
        let total = self.total_fission_fraction().get::<ratio>();
        if (total - 1.0).abs() > 1.0e-12 {
            return Err(TehOPrkeError::GenericStringError(format!(
                "fission fractions sum to {total}, not 1 (RELAP-7 Eq 417)"
            )));
        }
        Ok(())
    }

    /// Advance every group by `timestep` at constant total fission rate `psi`
    /// (fissions per second), analytically (exact for constant `psi`).
    pub fn advance(
        &mut self,
        fission_rate: Frequency,
        timestep: Time,
    ) -> Result<(), TehOPrkeError> {
        self.check_fractions()?;
        let rate = fission_rate.get::<hertz>();
        let dt = timestep.get::<second>();
        let fg = self.conservatism_factor.get::<ratio>();
        for slot in self.slots.iter_mut().flatten() {
            let factor = fg * slot.fission_fraction.get::<ratio>();
            for (j, (a, l)) in slot.set.alpha_lambda.iter().enumerate() {
                slot.group_power[j] =
                    advance_group_power(slot.group_power[j], factor * a, rate, *l, dt);
            }
        }
        Ok(())
    }

    /// Put every group at equilibrium with an infinitely long irradiation at
    /// `psi`: `H_aj = F_gamma F_a a_aj psi / lambda_aj`.
    pub fn saturate(&mut self, fission_rate: Frequency) -> Result<(), TehOPrkeError> {
        self.check_fractions()?;
        let rate = fission_rate.get::<hertz>();
        let fg = self.conservatism_factor.get::<ratio>();
        for slot in self.slots.iter_mut().flatten() {
            let factor = fg * slot.fission_fraction.get::<ratio>();
            for (j, (a, l)) in slot.set.alpha_lambda.iter().enumerate() {
                slot.group_power[j] = Power::new::<watt>(factor * a * rate / l * JOULE_PER_MEV);
            }
        }
        Ok(())
    }

    /// Uncorrected decay power `P'_gamma` (Eq 418). Multiply by
    /// [`neutron_capture_correction`] for Eq 420.
    pub fn total_decay_power(&self) -> Power {
        self.slots.iter().flatten().fold(Power::ZERO, |acc, s| {
            s.group_power.iter().fold(acc, |a, g| a + *g)
        })
    }

    /// Decay power of one isotope; `None` if it is not in the mix.
    pub fn isotope_decay_power(&self, isotope: DecayHeatIsotope) -> Option<Power> {
        self.slots[isotope.index()].map(|s| s.group_power.iter().fold(Power::ZERO, |a, g| a + *g))
    }
}

/// The neutron-absorption correction `G(t)` of the 1979/1994/2005 standards
/// as RELAP-7 Eq (419) gives it:
///
/// ```text
/// G(t) = 1 + (3.24e-6 + 5.23e-10 t) T^0.4 psi_g
/// ```
///
/// `t` time since shutdown (s), `T` operating time including shutdowns (s),
/// `psi_g` fissions per initial fissile atom. RELAP-7 states the limits
/// `1 <= psi_g <= 3`, `T < 1.2614e8 s`, `t < 1e4 s`; outside them this
/// returns an error rather than extrapolating. (The standard's tabulated
/// `G_max(t)` used beyond 1e4 s is not reproduced here; `sembawang` carries
/// MELCOR's copy.)
pub fn neutron_capture_correction(
    time_since_shutdown: Time,
    operating_time: Time,
    fissions_per_initial_fissile_atom: Ratio,
) -> Result<Ratio, TehOPrkeError> {
    let t = time_since_shutdown.get::<second>();
    let big_t = operating_time.get::<second>();
    let psi = fissions_per_initial_fissile_atom.get::<ratio>();
    if !(0.0..1.0e4).contains(&t)
        || !(0.0..1.2614e8).contains(&big_t)
        || !(1.0..=3.0).contains(&psi)
    {
        return Err(TehOPrkeError::GenericStringError(format!(
            "G(t) (RELAP-7 Eq 419) outside its stated limits: t = {t} s (< 1e4), \
             T = {big_t} s (< 1.2614e8), psi_g = {psi} (1..3)"
        )));
    }
    Ok(Ratio::new::<ratio>(
        1.0 + (3.24e-6 + 5.23e-10 * t) * big_t.r_powf(0.4) * psi,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decay_heat::{DecayHeat, NOMINAL_ENERGY_PER_FISSION_MEV};
    use uom::si::power::megawatt;

    fn r(x: f64) -> Ratio {
        Ratio::new::<ratio>(x)
    }

    /// Isotope slots per standard: 3 for Draft1978/1979, 4 (with Pu-241) for
    /// 1994/2005; only Draft1978 has built-in data; Pu-241 is refused under
    /// the 1979 standard.
    #[test]
    fn standards_have_the_right_isotope_slots_and_data() {
        assert_eq!(AnsStandard::default(), AnsStandard::Draft1978);
        assert_eq!(AnsStandard::Ans1979.isotopes().len(), 3);
        assert_eq!(AnsStandard::Ans1994.isotopes().len(), 4);
        assert!(AnsStandard::Ans2005
            .isotopes()
            .contains(&DecayHeatIsotope::Pu241));
        for std in [
            AnsStandard::Ans1979,
            AnsStandard::Ans1994,
            AnsStandard::Ans2005,
        ] {
            for iso in std.isotopes() {
                assert!(std.built_in_group_set(*iso).is_none());
            }
            assert!(AnsDecayHeat::new(std)
                .with_built_in_isotope(DecayHeatIsotope::U235, r(1.0))
                .is_err());
        }
        let set = AnsStandard::Draft1978
            .built_in_group_set(DecayHeatIsotope::U235)
            .unwrap();
        assert!(AnsDecayHeat::new(AnsStandard::Ans1979)
            .with_user_coefficients(DecayHeatIsotope::Pu241, r(1.0), set)
            .is_err());
        assert!(AnsDecayHeat::new(AnsStandard::Ans1994)
            .with_user_coefficients(DecayHeatIsotope::Pu241, r(1.0), set)
            .is_ok());
        assert!(AnsDecayHeat::new(AnsStandard::Draft1978)
            .with_built_in_isotope(DecayHeatIsotope::U235, r(1.5))
            .is_err());
    }

    /// User-supplied coefficients are validated; the uom constructor and the
    /// tabulated constructor agree; total decay energy matches the
    /// `FissioningNuclide` figure (13.183 MeV for U-235, 2026-08-11 record).
    #[test]
    fn user_coefficients_round_trip() {
        let mut bad = *FissioningNuclide::U235Thermal.parameters();
        bad[3].1 = 0.0;
        assert!(DecayHeatGroupSet::from_tabulated(bad).is_err());
        let p = FissioningNuclide::U235Thermal.parameters();
        let alpha = p.map(|(a, _)| Power::new::<watt>(a * JOULE_PER_MEV));
        let lambda = p.map(|(_, l)| Frequency::new::<hertz>(l));
        let set = DecayHeatGroupSet::new(alpha, lambda).unwrap();
        for j in 0..DECAY_HEAT_GROUPS {
            assert!((set.tabulated()[j].0 / p[j].0 - 1.0).abs() < 1e-15);
        }
        let e = set
            .total_decay_energy_per_fission()
            .get::<megaelectronvolt>();
        assert!((e - 13.183).abs() < 1e-3, "{e}");
    }

    /// **V&V: Draft1978 single isotope reproduces `DecayHeat` bit for bit.**
    ///
    /// *Methodology.* U-235 only, `F_a = 1`, `F_gamma = 1`; irradiate at 10 MW
    /// (rate converted with the same 200 MeV and the same expression as
    /// `DecayHeat`) for 1000 s, then shut down for 1000 s, both in 1 s steps.
    /// *Prediction.* zero bits differ, since both call the same group update
    /// and multiplying by 1.0 is exact. *Result (2026-11-01):* identical at
    /// every step.
    #[test]
    fn draft1978_single_isotope_matches_decay_heat_bitwise() {
        let mut old = DecayHeat::new(FissioningNuclide::U235Thermal);
        let mut new = AnsDecayHeat::default();
        let p = Power::new::<megawatt>(10.0);
        let rate = p.get::<watt>() / (NOMINAL_ENERGY_PER_FISSION_MEV * JOULE_PER_MEV);
        let dt = Time::new::<second>(1.0);
        for k in 0..2000 {
            let (pw, rt) = if k < 1000 {
                (p, rate)
            } else {
                (Power::ZERO, 0.0)
            };
            old.advance_timestep(pw, dt);
            new.advance(Frequency::new::<hertz>(rt), dt).unwrap();
            assert_eq!(
                old.total_decay_heat_power().value.to_bits(),
                new.total_decay_power().value.to_bits(),
                "step {k}"
            );
        }
    }

    /// **V&V: `F_gamma` and fission fractions, hand evaluation.**
    ///
    /// *Methodology.* Saturated state, so `P' = F_gamma psi sum_a F_a
    /// E_a` with `E_a = sum_j a_j/lambda_j`. Mix 0.6 U-235 / 0.25 Pu-239 /
    /// 0.15 U-238, `F_gamma = 1.2`, `psi = 1e17 /s`. Hand value from the
    /// recorded totals (13.183, 10.932, 16.244 MeV): 1.2 x 1e17 x (0.6 x
    /// 13.183 + 0.25 x 10.932 + 0.15 x 16.244) MeV/s = 1.2 x 1e17 x 13.0794
    /// MeV/s = 2.51466e5 W. *Prediction:* agreement to the 3-decimal rounding
    /// of the totals, ~1e-4 relative; exactly 1.2x the F_gamma = 1 value.
    /// *Result (2026-11-01):* passes; ratio exact to 1e-15.
    #[test]
    fn conservatism_factor_and_mix_hand_evaluation() {
        let mk = |fg: f64| {
            AnsDecayHeat::new(AnsStandard::Draft1978)
                .with_built_in_isotope(DecayHeatIsotope::U235, r(0.6))
                .unwrap()
                .with_built_in_isotope(DecayHeatIsotope::Pu239, r(0.25))
                .unwrap()
                .with_built_in_isotope(DecayHeatIsotope::U238, r(0.15))
                .unwrap()
                .with_conservatism_factor(r(fg))
        };
        let psi = Frequency::new::<hertz>(1.0e17);
        let mut a = mk(1.2);
        a.saturate(psi).unwrap();
        let mut b = mk(1.0);
        b.saturate(psi).unwrap();
        let hand = 1.2 * 1.0e17 * (0.6 * 13.183 + 0.25 * 10.932 + 0.15 * 16.244) * JOULE_PER_MEV;
        let got = a.total_decay_power().get::<watt>();
        assert!((got / hand - 1.0).abs() < 1e-4, "{got} vs {hand}");
        assert!((got / b.total_decay_power().get::<watt>() - 1.2).abs() < 1e-14);
        assert_eq!(a.conservatism_factor(), r(1.2));
        assert_eq!(a.standard(), AnsStandard::Draft1978);
        assert!(a.isotope_decay_power(DecayHeatIsotope::Pu241).is_none());
        assert!(a.isotope_decay_power(DecayHeatIsotope::U235).unwrap().value > 0.0);
        // fractions not summing to 1 are refused
        let mut c = AnsDecayHeat::new(AnsStandard::Draft1978)
            .with_built_in_isotope(DecayHeatIsotope::U235, r(0.5))
            .unwrap();
        assert!(c.advance(psi, Time::new::<second>(1.0)).is_err());
        assert!(c.saturate(psi).is_err());
    }

    /// `G(t)`, RELAP-7 Eq 419, by hand: t = 100 s, T = 3e7 s, psi_g = 2:
    /// T^0.4 = exp(0.4 ln 3e7) = 979.148; G = 1 + (3.24e-6 + 5.23e-8) x
    /// 979.148 x 2 = 1.006447. Out-of-range inputs are refused.
    /// *Result (2026-11-01):* 1.006447 to 1e-6.
    #[test]
    fn neutron_capture_correction_hand_evaluation() {
        let g = neutron_capture_correction(
            Time::new::<second>(100.0),
            Time::new::<second>(3.0e7),
            r(2.0),
        )
        .unwrap()
        .get::<ratio>();
        let hand = 1.0 + (3.24e-6 + 5.23e-10 * 100.0) * (3.0e7f64).r_powf(0.4) * 2.0;
        assert!((g - hand).abs() < 1e-15);
        assert!((g - 1.006447).abs() < 1e-6, "{g}");
        assert!(neutron_capture_correction(
            Time::new::<second>(2.0e4),
            Time::new::<second>(1.0),
            r(1.0)
        )
        .is_err());
        assert!(neutron_capture_correction(
            Time::new::<second>(1.0),
            Time::new::<second>(1.0),
            r(0.5)
        )
        .is_err());
    }
}
