// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! Point kinetics with fuel / moderator / reflector temperature feedback,
//! Xe-135 and decay heat, for the HTR-10 transients (gh:#1151).
//!
//! Everything numerical is `teh-o-prke`'s: [`PointReactor`] (N-group PRKE by
//! backward Euler + ANS decay heat, RELAP-7 Eqs 412-429). This module only
//! supplies the HTR-10 data and the reactivity.
//!
//! # Kinetics data (input sheet `htr10-th-benchmark.md` Sec 9.1)
//!
//! | Quantity | Base | Source | Sensitivity |
//! |---|---|---|---|
//! | `beta_eff` | 0.00726 | [T1] Sec 2.3.1.1; [J09] Tab 3; [C09] Tab 1 | none (all agree) |
//! | `Lambda` | 1.68e-3 s | [T1], [J09], [C09] (D4: the as-run sources agree) | `KIN-LAMBDA-H06`: 1.68e-4 s |
//! | `alpha_f / alpha_m / alpha_r` | -2.13 / -16.2 / +7.71 pcm/K | [T1], [J09] Tab 3 | `KIN-ALPHA-G02-IC`, `-G02-EQ` |
//! | six delayed groups | **Unknown in every source** | — | see below |
//!
//! **The six-group constants are a named assumption**: Keepin's U-235
//! thermal-fission set (G. R. Keepin, T. F. Wimett, R. K. Zeigler, "Delayed
//! neutrons from fissionable isotopes of uranium, plutonium and thorium",
//! Phys. Rev. 107 (1957) 1044, Table; reproduced in G. R. Keepin, *Physics
//! of Nuclear Kinetics*, Addison-Wesley 1965, Table 4-7): half-lives
//! 55.72 / 22.72 / 6.22 / 2.30 / 0.610 / 0.230 s, relative abundances
//! 0.033 / 0.219 / 0.196 / 0.395 / 0.115 / 0.042. Chosen because HTR-10 fuel
//! is 17 % enriched UO2 in a thermal spectrum, so almost every fission is
//! U-235 thermal fission; only the relative abundances are used, rescaled to
//! the benchmark's `beta_eff`. Sensitivities: `teh-o-prke`'s own tabulated
//! U-235 set (~~rounded half-lives and fractions~~ **since 2026-11-10,
//! gh:#1319: ENDF/B-VIII.0 U-235 thermal, MAT 9228 MT=455; mean precursor
//! life 11.06 s against Keepin's 13.02 s**, also rescaled), and the
//! one-group equivalent (same `sum beta_i / lambda_i`).
//!
//! # Feedback
//!
//! `rho = alpha_f (T_f - T_f0) + alpha_m (T_m - T_m0) + alpha_r (T_r - T_r0)
//! + (rho_Xe - rho_Xe0)`, the overall coefficients applied to core-average
//! temperatures; [T1] Sec 2.1.1.6 splits them over zones by power fraction,
//! which is what the core model's power-weighted averages do.
//!
//! # Xenon (default ON; `XE-OFF` is the explicit ablation)
//!
//! I-135 / Xe-135 with burnout, the yields and decay constants of
//! `teh-o-prke`'s [`Xenon135Poisoning`] (Lamarsh 1975 Tabs 7.5-7.6). The
//! reactivity of an inventory `X` (atoms in the core) is, one-group,
//!
//! ```text
//! rho_Xe = -(sigma_Xe / sigma_f) (X / N_235) (Sigma_f / Sigma_a),
//! Sigma_f / Sigma_a = k_inf / nu,       sigma_Xe phi = (sigma_Xe / sigma_f) F / N_235
//! ```
//!
//! with `F` the fission rate. Inputs, each cited and each an assumption:
//! `sigma_Xe = 2.65e6 b`, `sigma_f(U-235) = 585 b` (2200 m/s values;
//! S. F. Mughabghab, *Atlas of Neutron Resonances*, 2006), so the ratio is
//! taken at 2200 m/s, not spectrum-averaged; `nu = 2.43` (U-235 thermal,
//! Lamarsh Tab 6.5); `k_inf = 1.77`, the MCNP5 value for the 57:43
//! fuel/dummy BCC lattice recorded in `docs/reactor-scoping/htr10-neutronics.md`
//! (Wang 2014, config b); `N_235` from the as-run 13,651 fuel pebbles
//! ([J09] Tab 2) x 5.0 g U x 17 % ([T1] Tab 2.2). Only the CHANGE from the
//! equilibrium inventory enters the reactivity.
//!
//! > Research, education and V&V only. AI draft, unreviewed.

use teh_o_prke::prelude::{
    AnsDecayHeat, FissionPowerConversion, FissioningNuclide, FissioningNuclideType, PointKinetics,
    PointReactor, Xenon135Poisoning, NOMINAL_ENERGY_PER_FISSION_MEV,
};
use uom::si::energy::megaelectronvolt;
use uom::si::f64::{
    Energy, Frequency, Power, Ratio, ReciprocalLength, TemperatureCoefficient,
    ThermodynamicTemperature, Time, Velocity, Volume, VolumetricNumberRate,
};
use uom::si::frequency::hertz;
use uom::si::power::watt;
use uom::si::ratio::ratio;
use uom::si::reciprocal_length::reciprocal_meter;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;
use uom::si::volume::cubic_meter;

pub use tampines::htr10_primary_loop::FeedbackTemperatures;
use tampines::TampinesError;

const AVOGADRO: f64 = 6.022_140_76e23;
/// Keepin U-235 thermal half-lives, s.
const KEEPIN_HALF_LIVES_S: [f64; 6] = [55.72, 22.72, 6.22, 2.30, 0.610, 0.230];
/// Keepin U-235 thermal relative abundances `a_i = beta_i / beta`.
const KEEPIN_ABUNDANCES: [f64; 6] = [0.033, 0.219, 0.196, 0.395, 0.115, 0.042];

fn teho_err(e: impl std::fmt::Display) -> TampinesError {
    TampinesError::Numerical(format!("teh-o-prke: {e}"))
}

/// Which delayed-neutron group constants to use (none is published for
/// HTR-10; see the module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DelayedNeutronSet {
    /// Keepin (1957) U-235 thermal, six groups. The named base assumption.
    #[default]
    KeepinU235Thermal,
    /// `teh-o-prke`'s tabulated U-235 set (~~rounded Keepin-like
    /// half-lives and fractions~~ **ENDF/B-VIII.0 U-235 thermal since
    /// 2026-11-10, gh:#1319**), group shares rescaled to `beta_eff`.
    /// Sensitivity `KIN-DN-TEHO`.
    TehOPrkeU235,
    /// One group with the Keepin set's `sum a_i / lambda_i`. Sensitivity
    /// `KIN-DN-1G`.
    OneGroupKeepin,
}

impl DelayedNeutronSet {
    /// `(beta_i, lambda_i)` rescaled so `sum beta_i = beta_eff`.
    pub fn groups(&self, beta_eff: Ratio) -> (Vec<Ratio>, Vec<Frequency>) {
        let b = beta_eff.get::<ratio>();
        let ln2 = core::f64::consts::LN_2;
        match self {
            Self::KeepinU235Thermal => (
                KEEPIN_ABUNDANCES.iter().map(|a| Ratio::new::<ratio>(a * b)).collect(),
                KEEPIN_HALF_LIVES_S
                    .iter()
                    .map(|t| Frequency::new::<hertz>(ln2 / t))
                    .collect(),
            ),
            Self::TehOPrkeU235 => {
                let f = FissioningNuclideType::U235.get_delayed_fraction_array();
                let total: f64 = f.iter().map(|x| x.get::<ratio>()).sum();
                (
                    f.iter().map(|x| Ratio::new::<ratio>(x.get::<ratio>() / total * b)).collect(),
                    FissioningNuclideType::U235.get_decay_constant_array().to_vec(),
                )
            }
            Self::OneGroupKeepin => {
                let mean_life: f64 = KEEPIN_ABUNDANCES
                    .iter()
                    .zip(KEEPIN_HALF_LIVES_S.iter())
                    .map(|(a, t)| a * t / ln2)
                    .sum();
                (vec![beta_eff], vec![Frequency::new::<hertz>(1.0 / mean_life)])
            }
        }
    }
}

/// Fuel, moderator and reflector temperature coefficients.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeedbackCoefficients {
    /// Fuel, 1/K.
    pub fuel: TemperatureCoefficient,
    /// Moderator (pebble graphite), 1/K.
    pub moderator: TemperatureCoefficient,
    /// Reflector, 1/K.
    pub reflector: TemperatureCoefficient,
}

fn pcm_per_k(v: f64) -> TemperatureCoefficient {
    TemperatureCoefficient::new::<per_kelvin>(v * 1.0e-5)
}

impl FeedbackCoefficients {
    /// [T1] Sec 2.3.1.1 / [J09] Tab 3: -2.13 / -16.2 / +7.71 pcm/K. Base.
    pub fn tecdoc1694() -> Self {
        Self { fuel: pcm_per_k(-2.13), moderator: pcm_per_k(-16.2), reflector: pcm_per_k(7.71) }
    }
    /// [G02] Sec 4.3.2, initial core: -1.42 / -10.8 / +5.14 pcm/K.
    pub fn g02_initial_core() -> Self {
        Self { fuel: pcm_per_k(-1.42), moderator: pcm_per_k(-10.8), reflector: pcm_per_k(5.14) }
    }
    /// [G02] Sec 4.3.2, equilibrium core: -2.55 / -5.62 / +4.29 pcm/K.
    pub fn g02_equilibrium_core() -> Self {
        Self { fuel: pcm_per_k(-2.55), moderator: pcm_per_k(-5.62), reflector: pcm_per_k(4.29) }
    }
}

/// The Xe-135 channel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum XenonModel {
    /// No xenon reactivity (ablation `XE-OFF`).
    Off,
    /// One-group I/Xe with burnout (see the module docs).
    OneGroupBurnout {
        /// `sigma_Xe / sigma_f(U-235)`.
        xe_to_fission_cross_section_ratio: f64,
        /// `Sigma_f / Sigma_a = k_inf / nu`.
        fission_to_absorption_ratio: f64,
        /// U-235 atoms in the core.
        u235_atoms: f64,
    },
}

impl XenonModel {
    /// The HTR-10 values of the module docs.
    pub fn htr10() -> Self {
        let u235_atoms = 13_651.0 * 5.0 * 0.17 / 235.044 * AVOGADRO;
        Self::OneGroupBurnout {
            xe_to_fission_cross_section_ratio: 2.65e6 / 585.0,
            fission_to_absorption_ratio: 1.77 / 2.43,
            u235_atoms,
        }
    }
}

/// Kinetics parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KineticsParameters {
    /// `beta_eff`.
    pub beta_eff: Ratio,
    /// Prompt generation time `Lambda`.
    pub generation_time: Time,
    /// Delayed group constants.
    pub delayed_set: DelayedNeutronSet,
    /// Temperature coefficients.
    pub coefficients: FeedbackCoefficients,
    /// Xenon.
    pub xenon: XenonModel,
    /// Multiplier on the ANS decay power (the standard's `F_gamma`); 1 is
    /// best estimate. `DH-HALF` (0.5) mirrors [C14] Tab 4 case 3.
    pub decay_heat_scale: f64,
    /// External neutron source (gh:#1151 part B; see [`NeutronSource`]).
    pub neutron_source: NeutronSource,
}

impl KineticsParameters {
    /// The base case of the module docs, with the start-up source
    /// ([`NeutronSource::htr10_am_be_startup`]) ON.
    pub fn htr10_base() -> Self {
        Self {
            beta_eff: Ratio::new::<ratio>(0.00726),
            generation_time: Time::new::<second>(1.68e-3),
            delayed_set: DelayedNeutronSet::default(),
            coefficients: FeedbackCoefficients::tecdoc1694(),
            xenon: XenonModel::htr10(),
            decay_heat_scale: 1.0,
            neutron_source: NeutronSource::htr10_am_be_startup(),
        }
    }

    /// The ablation `SRC-OFF`: no external source (part A's kinetics).
    #[must_use]
    pub fn without_neutron_source(mut self) -> Self {
        self.neutron_source = NeutronSource::None;
        self
    }
}

/// An external (non-fission) neutron source in the point kinetics
/// (gh:#1151 part B).
///
/// **Why it matters.** Deep subcritical, the fission power of a
/// source-free PRKE decays with the longest precursor period for as long
/// as the reactivity stays negative (part A: hours, to ~1e-20 of the
/// initial power), and the return to power then has to climb those decades
/// back; a source holds the fission power at its subcritical
/// multiplication `S / |rho|` instead.
///
/// **Magnitude, cited.** HTR-10 was started with a **20 Ci Am-Be source
/// placed in the side reflector, emitting 4.4e7 n/s** (IAEA-TECDOC-1382
/// (2003) Sec 4.2.1.5, "Initial criticality"; the input sheet
/// `htr10-neutronics.md` records the 20 Ci). **Whether it was still in the
/// reactor on 15 Oct 2003 is Unknown**: no source in either corpus says it
/// was removed, and a graphite core at this burnup has no other source
/// strong enough for a source-range start-up. Its importance relative to a
/// fission neutron is Unknown; 1 is used, an upper bound for a reflector
/// source. The intrinsic source of the fuel (U-238 spontaneous fission) is
/// the floor that is certainly present ([`Self::htr10_intrinsic`]).
///
/// **Units.** For a source `S` (n/s) of importance `w`, the PRKE term is
/// `q = w S / (Lambda nu v Sigma_f V)` in the conversion's units, so that
/// the subcritical fission-neutron rate is `w S / |rho|` (derivation in
/// [`Htr10Kinetics::source_rate`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NeutronSource {
    /// No source (ablation `SRC-OFF`).
    None,
    /// A constant source.
    Constant {
        /// Emission rate, n/s.
        neutrons_per_second: f64,
        /// Importance relative to a fission neutron.
        importance: f64,
    },
}

/// Neutrons per U-235 thermal fission, 2.43 (Lamarsh, as for xenon above).
const NU_U235: f64 = 2.43;

impl NeutronSource {
    /// The 20 Ci Am-Be start-up source, 4.4e7 n/s (TECDOC-1382
    /// Sec 4.2.1.5), importance 1 (Assumed). **The base**: see the type docs
    /// for why, and for what is Unknown.
    pub fn htr10_am_be_startup() -> Self {
        Self::Constant { neutrons_per_second: 4.4e7, importance: 1.0 }
    }

    /// `SRC-INTRINSIC`: U-238 spontaneous fission in the as-run fuel only.
    /// 13,651 pebbles x 5.0 g U x 0.83 U-238 ([J09] Tab 2, [T1] Tab 2.2) =
    /// 56.65 kg; `T_1/2` 4.468e9 y and SF branch 5.5e-5 % (Tuli, *Nuclear
    /// Wallet Cards*, 2011) give 6.84e-3 fissions/(s g); `nu_SF = 2.0` is
    /// **Assumed** (not in the corpus): 775 n/s. (alpha, n) on oxygen is
    /// left out, so this is a lower bound.
    pub fn htr10_intrinsic() -> Self {
        let per_g = core::f64::consts::LN_2 / (4.468e9 * 3.155_76e7) * AVOGADRO / 238.050_8 * 5.5e-7 * 2.0;
        Self::Constant { neutrons_per_second: 13_651.0 * 5.0 * 0.83 * per_g, importance: 1.0 }
    }

    /// `w S`, n/s.
    pub fn effective_strength(&self) -> f64 {
        match self {
            Self::None => 0.0,
            Self::Constant { neutrons_per_second, importance } => neutrons_per_second * importance,
        }
    }
}

/// Reactivity split into its parts (dimensionless `dk/k`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReactivityBreakdown {
    /// Fuel temperature term.
    pub fuel: f64,
    /// Moderator temperature term.
    pub moderator: f64,
    /// Reflector temperature term.
    pub reflector: f64,
    /// Xenon change from equilibrium.
    pub xenon: f64,
    /// Externally inserted reactivity (rod withdrawal; zero in the LOFC).
    pub external: f64,
    /// The constant offset that makes the initial state with the source a
    /// steady state, `-q Lambda / n0` (the plant was held critical with the
    /// source in; about -1.8e-10 for the Am-Be source at 3.3 MW).
    pub source_compensation: f64,
}

impl ReactivityBreakdown {
    /// Sum of the parts.
    pub fn total(&self) -> f64 {
        self.fuel + self.moderator + self.reflector + self.xenon + self.external + self.source_compensation
    }
}

/// Point kinetics + decay heat + xenon + temperature feedback.
#[derive(Debug, Clone, PartialEq)]
pub struct Htr10Kinetics {
    reactor: PointReactor,
    params: KineticsParameters,
    reference: FeedbackTemperatures,
    iodine: f64,
    xenon: f64,
    xenon_equilibrium: f64,
    source_compensation: f64,
}

impl Htr10Kinetics {
    /// Critical at `total_power` (immediate + saturated decay heat) with the
    /// feedback referenced to `reference` and xenon at equilibrium.
    ///
    /// The decay heat is saturated (infinite irradiation at this power),
    /// an approximation of the ~3000 MWh history ([T1] Sec 2.3.1.1) whose
    /// detail is Unknown. The immediate/decay split is solved so that their
    /// sum is `total_power`.
    ///
    /// # Errors
    /// [`TampinesError::Numerical`] from `teh-o-prke`.
    pub fn new(
        params: KineticsParameters,
        total_power: Power,
        reference: FeedbackTemperatures,
    ) -> Result<Self, TampinesError> {
        let (betas, lambdas) = params.delayed_set.groups(params.beta_eff);
        let kinetics = PointKinetics::new(betas, lambdas).map_err(teho_err)?;
        // Scaling constants of RELAP-7 Eqs 414-416. Only Q_f is physical here
        // (it sets the fission rate that drives xenon and decay heat); v, V
        // and Sigma_f only scale n, and the PRKE is linear in n.
        let q_f = Energy::new::<megaelectronvolt>(NOMINAL_ENERGY_PER_FISSION_MEV)
            - FissioningNuclide::U235Thermal.total_decay_energy_per_fission();
        let conversion = FissionPowerConversion {
            neutron_speed: Velocity::new::<meter_per_second>(2200.0),
            core_volume: Volume::new::<cubic_meter>(5.0),
            macroscopic_fission_cross_section: ReciprocalLength::new::<reciprocal_meter>(1.0),
            immediate_energy_per_fission: q_f,
        };
        let decay = AnsDecayHeat::default().with_conservatism_factor(Ratio::new::<ratio>(params.decay_heat_scale));
        let mut reactor = PointReactor::new(kinetics, conversion, decay);
        let p0 = total_power.get::<watt>();
        reactor.set_critical_equilibrium(total_power, params.generation_time);
        reactor.saturate_decay_heat().map_err(teho_err)?;
        let decay_over_immediate = reactor.decay_power().get::<watt>() / p0;
        let p_imm = Power::new::<watt>(p0 / (1.0 + decay_over_immediate));
        reactor.set_critical_equilibrium(p_imm, params.generation_time);
        reactor.saturate_decay_heat().map_err(teho_err)?;
        let mut me = Self {
            reactor,
            params,
            reference,
            iodine: 0.0,
            xenon: 0.0,
            xenon_equilibrium: 0.0,
            source_compensation: 0.0,
        };
        let f = me.reactor.fission_rate().get::<hertz>();
        let (iodine, xenon) = me.xenon_equilibrium_inventory(f);
        me.iodine = iodine;
        me.xenon = xenon;
        me.xenon_equilibrium = xenon;
        let n0 = me.reactor.kinetics().neutron_density().value;
        me.source_compensation = -me.source_rate().value * me.params.generation_time.get::<second>() / n0;
        Ok(me)
    }

    fn xenon_burnout_rate(&self, fission_rate: f64) -> f64 {
        match self.params.xenon {
            XenonModel::Off => 0.0,
            XenonModel::OneGroupBurnout { xe_to_fission_cross_section_ratio, u235_atoms, .. } => {
                xe_to_fission_cross_section_ratio * fission_rate / u235_atoms
            }
        }
    }

    fn xenon_equilibrium_inventory(&self, fission_rate: f64) -> (f64, f64) {
        let gi = Xenon135Poisoning::fp_yield_iodine_135_from_u235_thermal_fission().get::<ratio>();
        let gx = Xenon135Poisoning::fp_yield_xe_135_from_u235_thermal_fission().get::<ratio>();
        let li = Xenon135Poisoning::iodine_135_decay_const().get::<hertz>();
        let lx = Xenon135Poisoning::xe_135_decay_const().get::<hertz>();
        let i = gi * fission_rate / li;
        let x = (gx + gi) * fission_rate / (lx + self.xenon_burnout_rate(fission_rate));
        (i, x)
    }

    fn xenon_reactivity(&self, inventory: f64) -> f64 {
        match self.params.xenon {
            XenonModel::Off => 0.0,
            XenonModel::OneGroupBurnout {
                xe_to_fission_cross_section_ratio,
                fission_to_absorption_ratio,
                u235_atoms,
            } => -xe_to_fission_cross_section_ratio * inventory / u235_atoms * fission_to_absorption_ratio,
        }
    }

    /// Equilibrium xenon worth at the initial state, `dk/k` (negative).
    pub fn equilibrium_xenon_worth(&self) -> f64 {
        self.xenon_reactivity(self.xenon_equilibrium)
    }

    /// The reactivity at `temps` and the current xenon.
    pub fn reactivity(&self, temps: &FeedbackTemperatures) -> ReactivityBreakdown {
        let c = &self.params.coefficients;
        let d = |a: ThermodynamicTemperature, b: ThermodynamicTemperature| {
            a.get::<kelvin>() - b.get::<kelvin>()
        };
        ReactivityBreakdown {
            fuel: c.fuel.get::<per_kelvin>() * d(temps.fuel, self.reference.fuel),
            moderator: c.moderator.get::<per_kelvin>() * d(temps.moderator, self.reference.moderator),
            reflector: c.reflector.get::<per_kelvin>() * d(temps.reflector, self.reference.reflector),
            xenon: self.xenon_reactivity(self.xenon) - self.xenon_reactivity(self.xenon_equilibrium),
            external: 0.0,
            source_compensation: self.source_compensation,
        }
    }

    /// The PRKE source term of [`KineticsParameters::neutron_source`] in the
    /// conversion's units: `q = w S / (Lambda nu v Sigma_f V)`.
    ///
    /// Derivation: the PRKE's `n` is the conversion's density, with fission
    /// rate `F = n v Sigma_f V`. The fission-neutron production is
    /// `n V / Lambda` per second in PRKE terms and `nu F` physically; the
    /// equation is linear in `n` and `q`, so the physical source `w S / V`
    /// maps to `q = (w S / V) n / n_phys` with `n_phys = Lambda nu F / V`,
    /// i.e. the expression above. At a steady subcritical `rho`,
    /// `n = -q Lambda / rho` and the fission-neutron rate is `w S / |rho|`.
    pub fn source_rate(&self) -> VolumetricNumberRate {
        let c = self.reactor.conversion();
        let denom = self.params.generation_time.get::<second>()
            * NU_U235
            * c.neutron_speed.get::<meter_per_second>()
            * c.macroscopic_fission_cross_section.get::<reciprocal_meter>()
            * c.core_volume.get::<cubic_meter>();
        VolumetricNumberRate::new::<uom::si::volumetric_number_rate::per_cubic_meter_second>(
            self.params.neutron_source.effective_strength() / denom,
        )
    }

    /// Advance by `dt` with the feedback evaluated at `temps` (the core's
    /// start-of-step state: explicit coupling, implicit kinetics).
    ///
    /// # Errors
    /// [`TampinesError::Numerical`] from `teh-o-prke`.
    pub fn step(&mut self, dt: Time, temps: &FeedbackTemperatures) -> Result<ReactivityBreakdown, TampinesError> {
        self.step_with_external(dt, temps, 0.0)
    }

    /// As [`Self::step`], with an externally inserted reactivity `external`
    /// (`dk/k`, e.g. a rod withdrawal) added to the feedback.
    ///
    /// # Errors
    /// [`TampinesError::Numerical`] from `teh-o-prke`.
    pub fn step_with_external(
        &mut self,
        dt: Time,
        temps: &FeedbackTemperatures,
        external: f64,
    ) -> Result<ReactivityBreakdown, TampinesError> {
        let mut rho = self.reactivity(temps);
        rho.external = external;
        let q = self.source_rate();
        self.reactor
            .step(dt, Ratio::new::<ratio>(rho.total()), self.params.generation_time, q)
            .map_err(teho_err)?;
        // I-135 / Xe-135 by backward Euler at the end-of-step fission rate.
        let f = self.reactor.fission_rate().get::<hertz>();
        let h = dt.get::<second>();
        let gi = Xenon135Poisoning::fp_yield_iodine_135_from_u235_thermal_fission().get::<ratio>();
        let gx = Xenon135Poisoning::fp_yield_xe_135_from_u235_thermal_fission().get::<ratio>();
        let li = Xenon135Poisoning::iodine_135_decay_const().get::<hertz>();
        let lx = Xenon135Poisoning::xe_135_decay_const().get::<hertz>();
        self.iodine = (self.iodine + h * gi * f) / (1.0 + h * li);
        let burn = self.xenon_burnout_rate(f);
        self.xenon = (self.xenon + h * (gx * f + li * self.iodine)) / (1.0 + h * (lx + burn));
        Ok(rho)
    }

    /// Immediate fission power.
    pub fn fission_power(&self) -> Power {
        self.reactor.immediate_fission_power()
    }

    /// Fission rate, 1/s (the conversion's `F = n v Sigma_f V`).
    pub fn fission_rate(&self) -> Frequency {
        self.reactor.fission_rate()
    }

    /// Fission-product decay power.
    pub fn decay_power(&self) -> Power {
        self.reactor.decay_power()
    }

    /// Total thermal power deposited in the core.
    pub fn total_power(&self) -> Power {
        self.reactor.total_power()
    }

    /// The parameters.
    pub fn parameters(&self) -> &KineticsParameters {
        &self.params
    }

    /// Xenon inventory relative to its initial equilibrium.
    pub fn relative_xenon(&self) -> f64 {
        if self.xenon_equilibrium > 0.0 {
            self.xenon / self.xenon_equilibrium
        } else {
            1.0
        }
    }

    /// `sum beta_i / lambda_i` of the group set in use, s (the delayed
    /// neutrons' mean-life-weighted fraction; equal for `KeepinU235Thermal`
    /// and `OneGroupKeepin` by construction).
    pub fn delayed_mean_life_weight(&self) -> f64 {
        let (b, l) = self.params.delayed_set.groups(self.params.beta_eff);
        b.iter().zip(l.iter()).map(|(b, l)| b.get::<ratio>() / l.get::<hertz>()).sum()
    }
}
