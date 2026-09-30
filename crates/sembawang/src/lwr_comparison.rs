// SPDX-License-Identifier: GPL-3.0

//! # HTR-10 bounding air ingress against an equivalent-power LWR (GitHub #450, #452, #453)
//!
//! > **Research, education and V&V only** (`RESPONSIBLE_USE.md`). Nothing here
//! > is a source term, a dose or a siting argument for HTR-10, NuScale or any
//! > plant. The HTR-10 arm is a **bounding case, not a transient** (#420); the
//! > LWR arms are **published design / risk-study source terms with different
//! > accident physics** (#450). The comparison puts them through the **same**
//! > dispersion and dose arithmetic so the difference is the source term alone.
//!
//! One library function per arm, so the `lwr_nureg1465_counterpart` example
//! and `htgr_sim_v1`'s map read the same numbers (maintainer direction,
//! #452/#453):
//!
//! | Arm | Function | Boundary | What it is |
//! |---|---|---|---|
//! | HTR-10 bounding air ingress | [`htr10_air_ingress_bound`] | to the environment (no building credit, #409) | 1400 °C / 140 h failure fractions, KORA f_ox, TRISO-ATOPS release over the dose window, + Liu & Cao circulating at 100 % |
//! | LWR, NUREG-1465 | [`nureg1465_pwr_into_containment`] | **into containment** | Table 3.13 (PWR), all four phases or gap + early in-vessel |
//! | LWR, RG 1.183 Rev. 1 | [`rg1183_pwr_into_containment`], [`rg1183_containment_leakage`] | into containment; then **to the environment** at the TS leak rate `L_a` | Table 2 (MHA LOCA), Table 5 timing, Appendix A-2.7 leakage |
//! | LWR, WASH-1400 PWR 8 | [`wash1400_pwr8_to_atmosphere`] | **to the atmosphere** | Table 5-1: gap release, containment not isolated, no core melt -- the closest analogue to the HTR-10 bound (#451) |
//!
//! The LWR inventory is NuScale's Table B-5 (one module) scaled by thermal
//! power, [`pwr_inventory_scaled`]; the 160 MWt module power is the
//! maintainer's attribution (see the CSV header).
//!
//! **`L_a` is plant-specific and not in RG 1.183.** [`rg1183_containment_leakage`]
//! takes it as an input and also returns the release per unit `L_a`
//! (the small-leak limit), so a caller without a sourced `L_a` reports
//! "per 1 %/day" rather than inventing one.
//!
//! **Removal credit taken in containment: none.** RG 1.183 Appendix A-2.2
//! to A-2.6 *allow* natural deposition, sprays, filters and scrubbing, each
//! with its own model; none is credited here (conservative), and the iodine
//! species split (A-1.1: 95 % CsI, 4.85 % elemental, 0.15 % organic) is
//! therefore not needed for transport. Decay during hold-up is applied.
//!
//! Dose: [`max_dose`], the same `buangkok` single-plume Gaussian, FGR-15
//! submersion and groundshine, FGR-11 inhalation, adult, worst stability class
//! at 1 m/s, as `examples/htr10_air_ingress_kora_bound.rs`, whose chain
//! [`htr10_air_ingress_bound`] reproduces.

use boon_lay::fuel_failure::htr10 as panama_htr10;
use boon_lay::triso_atops_fork::accident::AccidentFractions;
use boon_lay::triso_atops_fork::nuclide_model::nuclide_database::find_nuclide;
use buangkok::coefficients::{
    external_coefficient, fgr11_inhalation, fgr11_inhalation_max_over_classes,
    fgr15_air_submersion, fgr15_ground_surface, fgr15_short_lived_progeny,
};
use buangkok::pydoseia::dcf::AgeBracket;
use buangkok::pydoseia::dispersion::{
    dilution_single_plume_no_met, MeanSpeedScaling, PlumeGeometry, Receptor, StabilityClass,
};
use buangkok::pydoseia::dose::{deposition_velocity_m_per_s, submersion_dose, Release};
use changi::activity::inventory::htr10_equilibrium_core;
use changi::activity::primary_helium::htr10_primary_helium_end_of_life;
use uom::si::f64::{Length, Radioactivity, ThermodynamicTemperature, Time};
use uom::si::length::meter;
use uom::si::radioactivity::becquerel;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::hour;

use crate::accident::release::accident_release;
use crate::htr10::{self, Htr10Geometry};
use crate::inventory::{CoreInventory, NuclideInventory};
use crate::scenario::TemperatureTransient;
use crate::Error as SembawangError;

/// Activity released per nuclide \[Bq\].
pub type Releases = Vec<(String, f64)>;

// ----------------------------------------------------------------------------
// HTR-10 bounding air ingress
// ----------------------------------------------------------------------------

/// The bounding case's constants, with the sources of
/// `examples/htr10_air_ingress_kora_bound.rs` (the maintainer's bounding case
/// B, 2026-09-30): whole core 1400 °C for 140 h, every particle exposed.
pub mod bound {
    /// Hold temperature \[°C\].
    pub const HOLD_CELSIUS: f64 = 1400.0;
    /// Hold duration the failure fractions are taken at \[h\].
    pub const HOLD_HOURS: f64 = 140.0;
    /// f_hm, Liu & Cao 2002 s.2.1 (HTR-10 design free uranium).
    pub const F_HM: f64 = 3.0e-4;
    /// f_inc, Liu & Cao 2002 s.2.1 (design irradiation failure).
    pub const F_INC: f64 = 5.0e-4;
    /// f_sic, **stand-in** (NP-MHTGR reference; no HTR-10 value).
    pub const F_SIC_STAND_IN: f64 = 1.0e-4;
    /// f_inc_sic, **stand-in** (NP-MHTGR).
    pub const F_INC_SIC_STAND_IN: f64 = 3.6e-5;
    /// KORA AVR 92/22: about 20 of 16 400 particles failed in air at 1400 °C
    /// for 140 h (IAEA-TECDOC-978 Table 5-7 = Kugeler 2017 Table 9).
    pub const F_OX_KORA: f64 = 1.2e-3;
    /// boon-lay fuel-failure integration steps over the hold.
    pub const BL_STEPS: usize = 200;
}

/// The HTR-10 bounding air-ingress release over `window` \[Bq per nuclide\]:
/// boon-lay fuel failure at 1400 °C/140 h plus KORA f_ox as the accident
/// increment, TRISO-ATOPS release (real normal-operation pools, #448) of the
/// Liu & Cao Table 1 inventory under a flat 1400 °C hold over the window,
/// **plus** Liu & Cao Table 3's circulating activity released at 100 %
/// (conservative; it double-counts the model's own circuit term, as the
/// example states). The chain of `examples/htr10_air_ingress_kora_bound.rs`.
///
/// `geometry` is HTR-10's (the caller reads it from `tampines::pebble_bed`).
///
/// # Errors
/// If the release chain rejects its inputs.
pub fn htr10_air_ingress_bound(
    geometry: Htr10Geometry,
    window: Time,
) -> Result<Releases, SembawangError> {
    use bound::*;
    let t_b = htr10::stand_in_irradiation_temperature();
    let hold_t = ThermodynamicTemperature::new::<degree_celsius>(HOLD_CELSIUS);
    let (_, _, f_end) =
        htr10::isothermal_failure(t_b, hold_t, Time::new::<hour>(HOLD_HOURS), BL_STEPS);
    let d_phi_bl = f_end - panama_htr10::end_of_irradiation_failure(t_b).get::<ratio>();
    let fractions = AccidentFractions {
        heavy_metal: F_HM,
        sic: F_SIC_STAND_IN,
        incremental: F_INC,
        incremental_sic: F_INC_SIC_STAND_IN,
        incremental_accident: d_phi_bl + F_OX_KORA,
        incremental_sic_accident: 0.0,
    };
    let kept: Vec<NuclideInventory> = htr10_equilibrium_core()
        .iter()
        .filter(|e| find_nuclide(e.nuclide).is_some())
        .map(|e| NuclideInventory::uniform(e.nuclide, e.activity, 1))
        .collect();
    let inventory = CoreInventory::new(kept, 1, 1);
    let samples = 97;
    let window_h = window.get::<hour>();
    let times: Vec<Time> = (0..samples)
        .map(|i| Time::new::<hour>(window_h * i as f64 / (samples - 1) as f64))
        .collect();
    let transient =
        TemperatureTransient::from_nodes(times, vec![vec![vec![hold_t; 1]; samples]; 1])?;
    let plant = htr10::plant_parameters(geometry, fractions);
    let out = accident_release(&inventory, &transient, &plant)?;
    let circulating = htr10_primary_helium_end_of_life();
    Ok(out
        .source_term
        .nuclides
        .iter()
        .map(|r| {
            let c = circulating
                .iter()
                .find(|e| e.nuclide == r.label)
                .map_or(0.0, |e| e.activity.get::<becquerel>());
            (r.label.clone(), r.total_released().get::<becquerel>() + c)
        })
        .collect())
}

// ----------------------------------------------------------------------------
// LWR inventory and published fractions
// ----------------------------------------------------------------------------

const NUSCALE_B5: &str = include_str!("../reference/lwr/nuscale_dca_part3_rev4_table_b5_bq.csv");
const N1465_T313: &str =
    include_str!("../reference/lwr/nureg1465_table3_13_pwr_into_containment.csv");
const RG1183_T2: &str = include_str!("../reference/lwr/rg1183r1_table2_pwr_into_containment.csv");
const WASH1400_PWR8: &str =
    include_str!("../reference/lwr/wash1400_table5_1_pwr8_to_atmosphere.csv");

/// NuScale module thermal power the Table B-5 inventory is attributed to
/// \[MWth\] -- **the maintainer's attribution**, not stated in the document
/// (see the CSV header).
pub const NUSCALE_MODULE_MWTH: f64 = 160.0;

fn rows(text: &str) -> impl Iterator<Item = Vec<&str>> {
    text.lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split(',').map(str::trim).collect())
}

fn element(nuclide: &str) -> &str {
    nuclide.split('-').next().unwrap_or("")
}

/// NuScale Table B-5 (one module) scaled to `thermal_power_mwth` \[Bq\].
pub fn pwr_inventory_scaled(thermal_power_mwth: f64) -> Releases {
    rows(NUSCALE_B5)
        .map(|r| {
            (
                r[0].to_string(),
                r[1].parse::<f64>().unwrap() * thermal_power_mwth / NUSCALE_MODULE_MWTH,
            )
        })
        .collect()
}

/// NUREG-1465 Table 3.8 group of an element (printed p. 10).
fn n1465_group(el: &str) -> Option<&'static str> {
    Some(match el {
        "Xe" | "Kr" => "noble_gases",
        "I" | "Br" => "halogens",
        "Cs" | "Rb" => "alkali_metals",
        "Te" | "Sb" | "Se" => "tellurium",
        "Ba" | "Sr" => "barium_strontium",
        "Ru" | "Rh" | "Pd" | "Mo" | "Tc" | "Co" => "noble_metals",
        "La" | "Zr" | "Nd" | "Eu" | "Nb" | "Pm" | "Pr" | "Sm" | "Y" | "Cm" | "Am" => "lanthanides",
        "Ce" | "Pu" | "Np" => "cerium",
        _ => return None,
    })
}

/// RG 1.183 Rev. 1 Table 6 group of an element (p. 24).
fn rg1183_group(el: &str) -> Option<&'static str> {
    Some(match el {
        "Xe" | "Kr" => "noble_gases",
        "I" | "Br" => "halogens",
        "Cs" | "Rb" => "alkali_metals",
        "Te" | "Sb" | "Se" => "tellurium",
        "Ba" | "Sr" => "barium_strontium",
        "Ru" | "Rh" | "Pd" | "Co" => "noble_metals",
        "La" | "Nd" | "Eu" | "Pm" | "Pr" | "Sm" | "Y" | "Cm" | "Am" => "lanthanides",
        "Ce" | "Pu" | "Np" | "Zr" => "cerium",
        "Mo" | "Tc" | "Nb" => "molybdenum",
        _ => return None,
    })
}

/// Which NUREG-1465 phases to include.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum N1465Phases {
    /// Gap + early in-vessel (the part a design-basis LOCA conventionally uses).
    GapAndEarlyInVessel,
    /// All four phases (gap, early in-vessel, ex-vessel, late in-vessel).
    All,
}

/// NUREG-1465 Table 3.13 PWR release **into containment** \[Bq per nuclide\].
/// Nuclides whose element has no Table 3.8 group are omitted.
pub fn nureg1465_pwr_into_containment(inventory: &Releases, phases: N1465Phases) -> Releases {
    let table: Vec<Vec<&str>> = rows(N1465_T313).collect();
    inventory
        .iter()
        .filter_map(|(n, bq)| {
            let g = n1465_group(element(n))?;
            let r = table.iter().find(|r| r[0] == g)?;
            let v: Vec<f64> = r[1..].iter().map(|x| x.parse().unwrap()).collect();
            let f = match phases {
                N1465Phases::GapAndEarlyInVessel => v[0] + v[1],
                N1465Phases::All => v.iter().sum(),
            };
            Some((n.clone(), f * bq))
        })
        .collect()
}

/// RG 1.183 Rev. 1 Table 2 PWR release **into containment** (gap + early
/// in-vessel) \[Bq per nuclide\], no decay.
pub fn rg1183_pwr_into_containment(inventory: &Releases) -> Releases {
    let table: Vec<Vec<&str>> = rows(RG1183_T2).collect();
    inventory
        .iter()
        .filter_map(|(n, bq)| {
            let g = rg1183_group(element(n))?;
            let r = table.iter().find(|r| r[0] == g)?;
            let f: f64 = r[1].parse::<f64>().unwrap() + r[2].parse::<f64>().unwrap();
            Some((n.clone(), f * bq))
        })
        .collect()
}

/// RG 1.183 containment -> environment for one leak rate.
#[derive(Debug, Clone)]
pub struct Leakage {
    /// Released to the environment at the given `L_a` \[Bq per nuclide\];
    /// `None` when no `L_a` was supplied.
    pub at_la: Option<Releases>,
    /// Released per unit `L_a` in the small-leak limit \[Bq per nuclide per
    /// (1 %/day)\] -- what a caller without a sourced `L_a` reports.
    pub per_percent_per_day: Releases,
}

/// **RG 1.183 Rev. 1 containment leakage to the environment** over `window`.
///
/// - Source into the well-mixed containment: Table 2 fractions, released
///   **linearly** over each Table 5 phase (gap 0.5 min -> 0.23 h, early
///   in-vessel 0.23 -> 4.5 h; the RG's stated default), release terminating
///   at the end of early in-vessel (Appendix A-2.1).
/// - Leakage (Appendix A-2.7): `L_a` for the first 24 h, `L_a / 2` after (PWR).
/// - Removal: **none credited** (module doc). Radioactive decay applied.
/// - `leak_rate_percent_per_day`: the plant's TS `L_a`. **Plant-specific, not
///   in RG 1.183; never defaulted here.** `None` -> only the per-unit result.
///
/// Integrated by explicit exponential steps of 60 s (`dA/dt = S - (lambda +
/// L) A`, released `= integral L A dt`).
pub fn rg1183_containment_leakage(
    inventory: &Releases,
    leak_rate_percent_per_day: Option<f64>,
    window: Time,
) -> Leakage {
    let table: Vec<Vec<&str>> = rows(RG1183_T2).collect();
    let window_s = window.get::<uom::si::time::second>();
    let dt = 60.0;
    let steps = (window_s / dt).ceil() as usize;
    let run = |n: &str, bq: f64, l_percent: f64| -> Option<f64> {
        let g = rg1183_group(element(n))?;
        let r = table.iter().find(|r| r[0] == g)?;
        let f: Vec<f64> = r[1..].iter().map(|x| x.parse().unwrap()).collect();
        let (gap, early, t0, t1, t2) = (f[0], f[1], f[2] * 3600.0, f[3] * 3600.0, f[4] * 3600.0);
        let lam = find_nuclide(n).map_or_else(
            || lambda_fallback(n),
            |x| x.decay_constant().get::<uom::si::frequency::hertz>(),
        );
        let l_full = l_percent / 100.0 / 86_400.0;
        let (mut a, mut released) = (0.0_f64, 0.0_f64);
        for k in 0..steps {
            let t = k as f64 * dt;
            let mid = t + 0.5 * dt;
            let src = if (t0..t1).contains(&mid) {
                gap * bq / (t1 - t0)
            } else if (t1..t2).contains(&mid) {
                early * bq / (t2 - t1)
            } else {
                0.0
            };
            let l = if mid < 24.0 * 3600.0 {
                l_full
            } else {
                0.5 * l_full
            };
            // Exact step of dA/dt = src - k A over dt, written with
            // phi1 = (1 - e^-x)/x and phi2 = (x - 1 + e^-x)/x^2 (x = k dt),
            // series for small x. The naive `(a - src/k)(1 - e)/k + src/k dt`
            // cancels catastrophically when k dt << 1 (long-lived nuclides in
            // the small-leak limit) and returned NEGATIVE releases; found on
            // the first run of the #452 example, 2026-09-30.
            let x = (lam + l) * dt;
            let (phi1, phi2) = if x < 1e-3 {
                (
                    1.0 - x / 2.0 + x * x / 6.0 - x * x * x / 24.0,
                    0.5 - x / 6.0 + x * x / 24.0 - x * x * x / 120.0,
                )
            } else {
                let om = -(-x).exp_m1();
                (om / x, (x - om) / (x * x))
            };
            let int_a = a * dt * phi1 + src * dt * dt * phi2;
            released += l * int_a;
            a = a * (-x).exp() + src * dt * phi1;
        }
        Some(released)
    };
    let per_unit = |n: &str, bq: f64| {
        // small-leak limit: derivative at L -> 0, by a tiny L and division
        run(n, bq, 1e-6).map(|x| x / 1e-6)
    };
    let per_percent_per_day: Releases = inventory
        .iter()
        .filter_map(|(n, bq)| per_unit(n, *bq).map(|v| (n.clone(), v)))
        .collect();
    let at_la = leak_rate_percent_per_day.map(|la| {
        inventory
            .iter()
            .filter_map(|(n, bq)| run(n, *bq, la).map(|v| (n.clone(), v)))
            .collect()
    });
    Leakage {
        at_la,
        per_percent_per_day,
    }
}

/// Decay constants for inventory nuclides TRISO-ATOPS's table lacks \[1/s\]
/// (ENDF/B-VIII.0 half-lives via boon-lay's decay library would be the source;
/// for the containment hold-up the few absent nuclides are taken as **stable**
/// -- conservative, and stated).
fn lambda_fallback(_nuclide: &str) -> f64 {
    0.0
}

/// WASH-1400 Table 5-1 PWR 8 release **to the atmosphere** \[Bq per nuclide\].
/// Iodine = the I column + the organic-I column.
pub fn wash1400_pwr8_to_atmosphere(inventory: &Releases) -> Releases {
    let t: Vec<Vec<&str>> = rows(WASH1400_PWR8).collect();
    let col = |name: &str| -> f64 {
        t.iter()
            .find(|r| r[0] == name)
            .map(|r| r[1].parse().unwrap())
            .unwrap()
    };
    inventory
        .iter()
        .filter_map(|(n, bq)| {
            let f = match element(n) {
                "Xe" | "Kr" => col("xe_kr"),
                "I" => col("i") + col("organic_i"),
                "Cs" | "Rb" => col("cs_rb"),
                "Te" | "Sb" => col("te_sb"),
                "Ba" | "Sr" => col("ba_sr"),
                "Ru" | "Mo" | "Rh" | "Tc" | "Co" => col("ru"),
                "La" | "Nd" | "Y" | "Ce" | "Pr" | "Nb" | "Am" | "Cm" | "Pu" | "Np" | "Zr" => {
                    col("la")
                }
                _ => return None,
            };
            Some((n.clone(), f * bq))
        })
        .collect()
}

// ----------------------------------------------------------------------------
// Dose
// ----------------------------------------------------------------------------

/// The comparison's reporting groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    /// Xe, Kr.
    NobleGases,
    /// I, Br.
    Halogens,
    /// Cs, Rb.
    AlkaliMetals,
    /// Te, Sb, Se.
    Tellurium,
    /// Ba, Sr.
    BariumStrontium,
    /// Ag, Pd (HTR-10's silver; not a NUREG-1465 group).
    Silver,
    /// Everything else (noble metals, lanthanides, cerium, Mo).
    Other,
}

impl Group {
    /// All groups, in report order.
    pub const ALL: [Group; 7] = [
        Group::NobleGases,
        Group::Halogens,
        Group::AlkaliMetals,
        Group::Tellurium,
        Group::BariumStrontium,
        Group::Silver,
        Group::Other,
    ];
    /// The group of a nuclide label.
    pub fn of(nuclide: &str) -> Group {
        match element(nuclide) {
            "Xe" | "Kr" => Group::NobleGases,
            "I" | "Br" => Group::Halogens,
            "Cs" | "Rb" => Group::AlkaliMetals,
            "Te" | "Sb" | "Se" => Group::Tellurium,
            "Ba" | "Sr" => Group::BariumStrontium,
            "Ag" | "Pd" => Group::Silver,
            _ => Group::Other,
        }
    }
    /// A short label.
    pub fn label(self) -> &'static str {
        match self {
            Group::NobleGases => "noble gases",
            Group::Halogens => "halogens",
            Group::AlkaliMetals => "alkali metals",
            Group::Tellurium => "Te group",
            Group::BariumStrontium => "Ba/Sr",
            Group::Silver => "Ag",
            Group::Other => "other",
        }
    }
}

/// The site and dose assumptions every arm shares -- those of
/// `examples/htr10_air_ingress_kora_bound.rs`.
#[derive(Debug, Clone, Copy)]
pub struct DoseAssumptions {
    /// Release height \[m\]: **0, ground level**, for every arm (the HTR-10
    /// example's; an LWR containment leak is also conventionally a ground-level
    /// release). Kept identical so the comparison is like-for-like.
    pub release_height_m: f64,
    /// Wind measurement height \[m\].
    pub measurement_height_m: f64,
    /// Adult breathing rate \[m^3/s\] (FGR-11 convention, 20 L/min).
    pub breathing_m3_per_s: f64,
    /// Groundshine exposure period \[s\].
    pub exposure_s: f64,
}

impl DoseAssumptions {
    /// The HTR-10 bounding example's: ground release, 10 m wind, 20 L/min,
    /// 96 h.
    pub fn bounding_example() -> Self {
        Self {
            release_height_m: 0.0,
            measurement_height_m: 10.0,
            breathing_m3_per_s: 0.020 / 60.0,
            exposure_s: 96.0 * 3600.0,
        }
    }
}

/// A maximum dose at one distance.
#[derive(Debug, Clone)]
pub struct Dose {
    /// Worst stability class at 1 m/s (largest chi/Q).
    pub class: StabilityClass,
    /// chi/Q used \[s/m^3\].
    pub chi_over_q: f64,
    /// Total over nuclides with coefficients \[Sv\].
    pub total_sv: f64,
    /// By group \[Sv\].
    pub by_group: Vec<(Group, f64)>,
    /// Nuclides lacking a coefficient on some pathway (NOT counted as zero).
    pub missing: Vec<String>,
}

/// Maximum dose at `x_m` from `releases`, the whole release passing the
/// receptor at the worst class, 1 m/s: submersion (FGR-15) + inhalation
/// (FGR-11, max over classes) + groundshine (FGR-15, deposited at buangkok's
/// velocity, decaying over the exposure period). The arithmetic of
/// `examples/htr10_air_ingress_kora_bound.rs` section 5.
pub fn max_dose(releases: &Releases, x_m: f64, a: DoseAssumptions) -> Dose {
    let geometry = PlumeGeometry {
        release_height: Length::new::<meter>(a.release_height_m),
        measurement_height: Length::new::<meter>(a.measurement_height_m),
        receptor: Receptor::GroundLevelCentreline,
    };
    let per_class = dilution_single_plume_no_met(
        Length::new::<meter>(x_m),
        geometry,
        MeanSpeedScaling::UnitSpeed,
    );
    let (ci, _) = per_class.iter().enumerate().fold((0, 0.0), |acc, (i, d)| {
        let v = d.seconds_per_cubic_meter();
        if v > acc.1 {
            (i, v)
        } else {
            acc
        }
    });
    let chi = per_class[ci];
    let sub_table = fgr15_air_submersion();
    let gs_table = fgr15_ground_surface();
    let chains = fgr15_short_lived_progeny();
    let inh_table = fgr11_inhalation();
    let mut by_group: Vec<(Group, f64)> = Group::ALL.iter().map(|g| (*g, 0.0)).collect();
    let mut missing = Vec::new();
    let has_cs137 = releases.iter().any(|(n, q)| n == "Cs-137" && *q > 0.0);
    for (n, q) in releases {
        if *q <= 0.0 {
            continue;
        }
        // buangkok's FGR-15 lookups already add Ba-137m to Cs-137 in secular
        // equilibrium (0.944 Bq/Bq, `buangkok::coefficients::PROGENY`). An
        // inventory that also lists Ba-137m (NuScale Table B-5 does) would
        // count its external dose twice, so it is skipped when Cs-137 is in
        // the release. (Its FGR-11 inhalation contribution is nil: no entry,
        // and a 2.55 min daughter delivers its dose as Cs-137's.)
        if n == "Ba-137m" && has_cs137 {
            continue;
        }
        let psi = chi.seconds_per_cubic_meter() * q;
        let release = Release::Instantaneous(Radioactivity::new::<becquerel>(*q));
        let e_sub = external_coefficient(&sub_table, &chains, n, AgeBracket::Adult)
            .map(|dcf| submersion_dose(chi, release, dcf).sieverts());
        let e_inh = fgr11_inhalation_max_over_classes(&inh_table, n, AgeBracket::Adult)
            .map(|dcf| psi * dcf * a.breathing_m3_per_s);
        let v_d = deposition_velocity_m_per_s(element(n));
        let lam = find_nuclide(n)
            .map(|x| x.decay_constant().get::<uom::si::frequency::hertz>())
            .unwrap_or(0.0);
        let decay_integral = if lam > 0.0 {
            -(-lam * a.exposure_s).exp_m1() / lam
        } else {
            a.exposure_s
        };
        let e_gs = if v_d == 0.0 {
            Some(0.0)
        } else {
            external_coefficient(&gs_table, &chains, n, AgeBracket::Adult)
                .map(|dcf| psi * v_d * dcf * decay_integral)
        };
        let noble = matches!(Group::of(n), Group::NobleGases);
        if e_sub.is_none() || e_gs.is_none() || (e_inh.is_none() && !noble) {
            missing.push(n.clone());
        }
        let e = e_sub.unwrap_or(0.0) + e_inh.unwrap_or(0.0) + e_gs.unwrap_or(0.0);
        let g = Group::of(n);
        by_group.iter_mut().find(|(k, _)| *k == g).unwrap().1 += e;
    }
    Dose {
        class: StabilityClass::ALL[ci],
        chi_over_q: chi.seconds_per_cubic_meter(),
        total_sv: by_group.iter().map(|(_, v)| v).sum(),
        by_group,
        missing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The scaled inventory is Table B-5 x 10/160, and NUREG-1465 all-phase
    /// noble gases are the whole noble-gas inventory.
    #[test]
    fn scaling_and_fractions_are_the_tables() {
        let inv = pwr_inventory_scaled(10.0);
        assert_eq!(inv.len(), 69);
        let i131 = inv.iter().find(|(n, _)| n == "I-131").unwrap().1;
        assert!((i131 - 1.58e17 * 10.0 / 160.0).abs() < 1e3);
        let all = nureg1465_pwr_into_containment(&inv, N1465Phases::All);
        let xe = all.iter().find(|(n, _)| n == "Xe-133").unwrap().1;
        assert!((xe - 3.31e17 / 16.0).abs() / xe < 1e-12);
        let i = all.iter().find(|(n, _)| n == "I-131").unwrap().1;
        assert!((i - 0.75 * i131).abs() / i < 1e-12);
        let w = wash1400_pwr8_to_atmosphere(&inv);
        let wi = w.iter().find(|(n, _)| n == "I-131").unwrap().1;
        assert!((wi - (1e-4 + 5e-6) * i131).abs() / wi < 1e-12);
    }

    /// Containment leakage: a stable nuclide released instantly into
    /// containment leaks `L T` in the small-leak limit, and the per-unit
    /// figure equals the at-L figure / L for small L.
    #[test]
    fn leakage_is_linear_in_small_leak_rates() {
        let inv: Releases = vec![("Kr-85".into(), 1.0e15)];
        let out = rg1183_containment_leakage(&inv, Some(0.2), Time::new::<hour>(96.0));
        let at = out.at_la.unwrap()[0].1;
        let per = out.per_percent_per_day[0].1;
        // At 0.2 %/day the leak itself depletes the containment by about
        // L x 2.4 d ~ 0.5 % over the window, so the linear extrapolation
        // over-states by about half that; the tolerance is that bound.
        assert!((at / 0.2 - per).abs() / per < 5e-3, "{at} {per}");
        // Kr-85 (10.7 y) into containment ~0.962 x inventory by 4.5 h; leak
        // ~24 h at L then 72 h at L/2 (minus the ramp): ~ 0.962 (1 + 1.5 - 0.1) / 100 per %/day.
        let expect = 0.962 * 1.0e15 * (1.0 + 1.5) / 100.0;
        assert!((per - expect).abs() / expect < 0.08, "{per} vs ~{expect}");
    }

    /// Pins the 2026-09-30 defect: the small-leak limit returned negative Bq
    /// for long-lived nuclides (Cs-137 etc.) through catastrophic
    /// cancellation. Every nuclide of the scaled inventory must leak a
    /// non-negative amount, and never more than its inventory per %/day x
    /// 4 days (an upper bound for any Table 2 fraction <= 1).
    #[test]
    fn leakage_is_non_negative_and_bounded_for_the_whole_inventory() {
        let inv = pwr_inventory_scaled(10.0);
        let out = rg1183_containment_leakage(&inv, Some(0.2), Time::new::<hour>(96.0));
        for (n, v) in out
            .per_percent_per_day
            .iter()
            .chain(out.at_la.unwrap().iter())
        {
            let bq = inv.iter().find(|(m, _)| m == n).unwrap().1;
            assert!(*v >= 0.0, "{n}: {v}");
            assert!(*v <= bq * 4.0 / 100.0, "{n}: {v} > bound for {bq}");
        }
    }
}
