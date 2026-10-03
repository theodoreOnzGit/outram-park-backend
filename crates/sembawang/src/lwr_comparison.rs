// SPDX-License-Identifier: GPL-3.0

//! # HTR-10 against an equivalent-power LWR (GitHub #450, #452, #453, #464)
//!
//! ~~**Framing (maintainer decisions, 2026-09-30, #450).** PRIMARY:
//! **design basis against design basis** ... SECONDARY: the
//! **beyond-design-basis bounding** pair, the KORA bound against WASH-1400
//! PWR 8.~~ **CHANGED 2026-09-30 (maintainer decision, #464): paired by
//! INITIATING EVENT and severity**, DLOFC (HTR) against LOCA (LWR), in two
//! tiers ([`Tier`]):
//!
//! - **Design basis: DLOFC vs LOCA.** HTR-10 depressurisation
//!   ([`htr10_dba_release`], Liu & Cao Table 8) against the LWR MHA LOCA
//!   ([`nuscale_mha_loca`], RG 1.183 Rev. 1).
//! - **Beyond design basis: DLOFC + air ingress (bounding) vs LOCA + core
//!   melt.** The KORA bound ([`htr10_air_ingress_bound`]) against the LWR
//!   LOCA with ECCS failure ([`nuscale_severe_loca`], NUREG-1465 Table 3.13,
//!   all phases, into an **intact** containment leaking at `L_a`).
//! - **Context:** WASH-1400 PWR 8, no-melt, uncontained; paired by
//!   containment state, not initiator.
//!
//! [`bounding_comparison`] returns every arm, in tier order ([`ARM_COLUMNS`]).
//!
//! **Crediting basis (maintainer decision, 2026-09-30):** "The comparison
//! neglects the pools because we are comparing technology at the reactor
//! level, not what is surrounding the reactor. If NuScale gives pool credit,
//! then HTGR can be submerged in a pool as well." Each side is credited
//! **only with its reactor-level inherent barrier**: for the LWR, the
//! containment vessel leaking at `L_a`; for the HTR, the TRISO particles.
//! Nothing surrounding the reactor is credited on either side (NuScale's
//! reactor pool, reactor building, sprays, filters; the HTR confinement or
//! building, #409, or a hypothetical pool). Pool scrubbing is excluded **by
//! design**, not pending literature. In-containment natural deposition is part
//! of the containment barrier, so that arm stays (pending literature).
//! [`REACTOR_LEVEL_BASIS`] carries this wherever the comparison is printed.
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
//! | **DB: HTR-10 DLOFC** | [`htr10_dba_release`] | to the environment | Liu & Cao (2002) Table 8, published (depressurisation; water ingress); cross-checked with [`htr10_dba_vs_table9`] |
//! | **DB: LWR LOCA** | [`nuscale_mha_loca`] | to the environment via containment leakage | RG 1.183 Rev. 1 MHA LOCA, NuScale inventory, `L_a` = 0.20 %/day ([`NUSCALE_LA_PERCENT_PER_DAY`]); no removal, and natural deposition ([`NaturalDeposition`], pending literature) |
//! | **BDB: HTR-10 DLOFC + air ingress** | [`htr10_air_ingress_bound`] | to the environment (no building credit, #409) | 1400 °C / 140 h failure fractions, KORA f_ox, TRISO-ATOPS release over the dose window, vented full flow-through ([`AIR_INGRESS_VENTING`], #469), + Liu & Cao circulating at 100 % |
//! | **BDB: LWR LOCA + core melt** | [`nuscale_severe_loca`] | to the environment via containment leakage | NUREG-1465 Table 3.13 PWR, all four phases, Table 3.6 timing ([`nureg1465_pwr_phases`]); intact containment at `L_a`; no removal, and natural deposition (pending literature). [`CONTAINED_CORE_MELT_ASSUMPTION`] |
//! | Context: WASH-1400 PWR 8 | [`wash1400_pwr8_to_atmosphere`] | **to the atmosphere** | Table 5-1: gap release, containment not isolated, no core melt (#451) |
//! | LWR, NUREG-1465 | [`nureg1465_pwr_into_containment`] | **into containment** | Table 3.13 (PWR), all four phases or gap + early in-vessel |
//! | LWR, RG 1.183 Rev. 1 | [`rg1183_pwr_into_containment`], [`rg1183_containment_leakage`] | into containment; then **to the environment** at the TS leak rate `L_a` | Table 2 (MHA LOCA), Table 5 timing, Appendix A-2.7 leakage |
//!
//! Both LWR LOCA arms leak through one integrator, [`containment_leak`], and
//! differ only in the phased source they feed it.
//!
//! The LWR inventory is NuScale's Table B-5 (one module) scaled by thermal
//! power, [`pwr_inventory_scaled`]; the 160 MWt module power is the
//! maintainer's attribution (see the CSV header).
//!
//! **`L_a` is plant-specific and not in RG 1.183.** [`rg1183_containment_leakage`]
//! takes it as an input and also returns the release per unit `L_a`
//! (the small-leak limit), so a caller without a sourced `L_a` reports
//! "per 1 %/day" rather than inventing one. **Since 2026-09-30** NuScale's is
//! sourced: 0.20 wt%/day (NRC Phase 4 SER Ch. 6, PDF p. 91), used by
//! [`nuscale_mha_loca`].
//!
//! ~~**Removal credit taken in containment: none.** ... the iodine species
//! split (A-1.1: 95 % CsI, 4.85 % elemental, 0.15 % organic) is therefore
//! not needed for transport.~~ **CHANGED 2026-09-30:** two arms. (i) No
//! removal credit. (ii) **Natural deposition only**, as App. A-2.2 allows
//! (model: SRP 6.5.2, or NUREG/CR-6189 case by case), through
//! [`rg1183_leak`] with explicit rates and the A-1.1 species split. The rates
//! are **pending literature** ([`NATURAL_DEPOSITION_PENDING`]) and never
//! defaulted. Sprays, filters and scrubbing are not credited. Decay during
//! hold-up is applied. The containment is scaled down with power
//! ([`scaled_containment`], maintainer decision; an assumption).
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
pub use changi::activity::accident_airborne_release::AccidentCase;
use buangkok::published::accident_dose_by_distance::htr10_accident_dose_by_distance;
use changi::activity::accident_airborne_release::htr10_accident_release;
use changi::activity::inventory::htr10_equilibrium_core;
use changi::activity::primary_helium::htr10_primary_helium_end_of_life;
use uom::si::f64::{Length, Radioactivity, ThermodynamicTemperature, Time};
use uom::si::length::meter;
use uom::si::radioactivity::becquerel;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::hour;

use crate::accident::release::{accident_release_with_venting, Venting};
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
    /// f_hm, Liu & Cao 2002 s.2.1 (HTR-10 design free uranium); read from
    /// [`crate::htr10::LIU_CAO_DESIGN_FREE_URANIUM`], one copy (#469).
    pub const F_HM: f64 = crate::htr10::LIU_CAO_DESIGN_FREE_URANIUM;
    /// f_inc, Liu & Cao 2002 s.2.1 (design irradiation failure); read from
    /// [`crate::htr10::LIU_CAO_DESIGN_IRRADIATION_FAILURE`], one copy (#469).
    pub const F_INC: f64 = crate::htr10::LIU_CAO_DESIGN_IRRADIATION_FAILURE;
    /// f_sic, **stand-in** (NP-MHTGR reference; no HTR-10 value).
    pub const F_SIC_STAND_IN: f64 = 1.0e-4;
    /// f_inc_sic, **stand-in** (NP-MHTGR).
    pub const F_INC_SIC_STAND_IN: f64 = 3.6e-5;
    /// KORA AVR 92/22: about 20 of 16 400 particles failed in air at 1400 °C
    /// for 140 h (IAEA-TECDOC-978 Table 5-7 = Kugeler 2017 Table 9). Pinned
    /// to the committed Table 5-7 row by
    /// `tests::f_ox_kora_is_the_table_5_7_sphere_test` (#453).
    pub const F_OX_KORA: f64 = 1.2e-3;
    /// boon-lay fuel-failure integration steps over the hold.
    pub const BL_STEPS: usize = 200;
}

/// IAEA-TECDOC-978 (IAEA, Vienna, 1997) air-oxidation fuel data, the
/// maintainer's kovan digitisations, committed under `reference/tecdoc978/`
/// with their provenance (#453). Proprietary tier: the values are cited, the
/// PDF is not redistributed.
pub mod kora {
    const TABLE_5_7: &str = include_str!("../reference/tecdoc978/table5_7_kora_heating_in_air.csv");
    const FIG_5_23: &str =
        include_str!("../reference/tecdoc978/fig5_23_failure_fraction_air_ingress_constant_t.csv");

    /// One row of TECDOC-978 Table 5-7 (KORA heating tests in air, Kr-85
    /// release).
    #[derive(Debug, Clone, PartialEq)]
    pub struct HeatingTest {
        /// Fuel sample (e.g. `AVR 92/22`).
        pub sample: String,
        /// Particles in the sample.
        pub particles: f64,
        /// Maximum temperature \[°C\].
        pub max_celsius: f64,
        /// Time at temperature \[h\].
        pub hours: f64,
        /// Failed particles.
        pub failed: f64,
        /// Printed fraction of failed particles.
        pub failed_fraction: f64,
    }

    /// Split one CSV line, honouring double quotes (`"16,400"`).
    fn fields(line: &str) -> Vec<String> {
        let (mut out, mut cur, mut quoted) = (Vec::new(), String::new(), false);
        for c in line.chars() {
            match c {
                '"' => quoted = !quoted,
                ',' if !quoted => out.push(std::mem::take(&mut cur)),
                _ => cur.push(c),
            }
        }
        out.push(cur);
        out
    }

    fn data_lines(csv: &str) -> impl Iterator<Item = Vec<String>> + '_ {
        csv.lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .skip(1)
            .map(fields)
    }

    /// Table 5-7, every row.
    ///
    /// # Panics
    /// Never for the shipped CSV (a test parses it).
    pub fn table_5_7() -> Vec<HeatingTest> {
        data_lines(TABLE_5_7)
            .map(|f| {
                let num = |i: usize| f[i].replace(',', "").trim().parse::<f64>().unwrap();
                HeatingTest {
                    sample: f[0].clone(),
                    particles: num(1),
                    max_celsius: num(4),
                    hours: num(5),
                    failed: num(7),
                    failed_fraction: num(8),
                }
            })
            .collect()
    }

    /// The Table 5-7 **whole-sphere** test (16 400 particles; the 10-particle
    /// rows are loose particles, not fuel in a sphere) at `celsius` for
    /// `hours`, if Table 5-7 has one.
    pub fn sphere_test(celsius: f64, hours: f64) -> Option<HeatingTest> {
        table_5_7()
            .into_iter()
            .find(|t| t.particles > 1000.0 && t.max_celsius == celsius && t.hours == hours)
    }

    /// One Fig. 5-23 series, by its legend name: `(hours, failure fraction)`,
    /// in digitised order.
    pub fn fig_5_23(series: &str) -> Vec<(f64, f64)> {
        data_lines(FIG_5_23)
            .filter(|f| f[0] == series)
            .map(|f| (f[1].parse().unwrap(), f[2].parse().unwrap()))
            .collect()
    }

    /// Fig. 5-23's **1400 °C Nabielek prediction** (dashed line) at `hours`,
    /// interpolated log-linearly between the digitised points (the figure's
    /// y axis is logarithmic). `None` outside the digitised range. A
    /// **model prediction** shown for context, not a measurement, and not
    /// used in the bound.
    pub fn nabielek_1400c_prediction(hours: f64) -> Option<f64> {
        let pts = fig_5_23("1400C prediction Nabielek (dashed lines)");
        pts.windows(2).find_map(|w| {
            let ((t0, f0), (t1, f1)) = (w[0], w[1]);
            (t0..=t1).contains(&hours).then(|| {
                let a = (hours - t0) / (t1 - t0);
                (f0.ln() + a * (f1.ln() - f0.ln())).exp()
            })
        })
    }
}

/// The HTR-10 bounding air-ingress release over `window` \[Bq per nuclide\]:
/// boon-lay fuel failure at 1400 °C/140 h plus KORA f_ox as the accident
/// increment, TRISO-ATOPS release (real normal-operation pools, #448) of the
/// Liu & Cao Table 1 inventory under a flat 1400 °C hold over the window,
/// vented by [`AIR_INGRESS_VENTING`] (full flow-through, stated since #469),
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
    let hold_t = ThermodynamicTemperature::new::<degree_celsius>(bound::HOLD_CELSIUS);
    let fractions = air_ingress_bound_fractions();
    let samples = 97;
    let window_h = window.get::<hour>();
    let times: Vec<Time> = (0..samples)
        .map(|i| Time::new::<hour>(window_h * i as f64 / (samples - 1) as f64))
        .collect();
    let transient =
        TemperatureTransient::from_nodes(times, vec![vec![vec![hold_t; 1]; samples]; 1])?;
    air_ingress_release_over(geometry, fractions, &transient)
}

/// How the bounding air ingress carries the release out of the core:
/// **[`Venting::FullFlowThrough`]**, `frac = 1` at every sample — the
/// conservative limit of [`Venting::Prescribed`], and the assumption of the
/// #435 bound ("every particle exposed", no primary-circuit retention).
///
/// **Why not [`Venting::Upstream`] (#469 item 4).** Upstream TRISO-ATOPS is a
/// depressurisation model: its only transport is gas expansion while the core
/// heats. It gives `frac = 1` on an *exactly* uniform, constant hold (#446),
/// but a hold drifting by 0.1 K takes its `coolant_release` branch and vents
/// `≈ 1 − T0/T`, so the bound fell from 1.69e13 Bq to 4.94e10 Bq
/// (`tests::a_near_isothermal_air_ingress_still_vents`, measured 2026-09-30).
/// Air ingress convects the release out whatever the temperature does, so the
/// transport is stated explicitly. On the exact hold the two agree, so the
/// recorded bounding numbers do not move.
pub const AIR_INGRESS_VENTING: Venting = Venting::FullFlowThrough;

/// The bounding case's six failure fractions: the four normal-operation
/// classes (tramp uranium `f_hm`, SiC defects `f_sic`, in-service `f_inc`,
/// SiC-only in-service `f_inc_sic`) plus the accident increment, boon-lay fuel
/// failure over the 1400 °C/140 h hold and KORA `f_ox`.
fn air_ingress_bound_fractions() -> AccidentFractions {
    use bound::*;
    let t_b = htr10::stand_in_irradiation_temperature();
    let hold_t = ThermodynamicTemperature::new::<degree_celsius>(HOLD_CELSIUS);
    let (_, _, f_end) =
        htr10::isothermal_failure(t_b, hold_t, Time::new::<hour>(HOLD_HOURS), BL_STEPS);
    let d_phi_bl = f_end - panama_htr10::end_of_irradiation_failure(t_b).get::<ratio>();
    AccidentFractions {
        heavy_metal: F_HM,
        sic: F_SIC_STAND_IN,
        incremental: F_INC,
        incremental_sic: F_INC_SIC_STAND_IN,
        incremental_accident: d_phi_bl + F_OX_KORA,
        incremental_sic_accident: 0.0,
    }
}

/// The release half of [`htr10_air_ingress_bound`] over any one-node
/// `transient`: TRISO-ATOPS from real normal-operation pools, vented by
/// [`AIR_INGRESS_VENTING`], plus Liu & Cao Table 3's circulating activity at
/// 100 %. Split out so a test can hand it a near-isothermal history (#469).
fn air_ingress_release_over(
    geometry: Htr10Geometry,
    fractions: AccidentFractions,
    transient: &TemperatureTransient,
) -> Result<Releases, SembawangError> {
    let kept: Vec<NuclideInventory> = htr10_equilibrium_core()
        .iter()
        .filter(|e| find_nuclide(e.nuclide).is_some())
        .map(|e| NuclideInventory::uniform(e.nuclide, e.activity, 1))
        .collect();
    let inventory = CoreInventory::new(kept, 1, 1);
    let plant = htr10::plant_parameters(geometry, fractions);
    let out = accident_release_with_venting(&inventory, transient, &plant, &AIR_INGRESS_VENTING)?;
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
    // small-leak limit: derivative at L -> 0, by a tiny L and division
    let per_percent_per_day: Releases = rg1183_leak(inventory, 1e-6, window, None)
        .into_iter()
        .map(|(n, v)| (n, v / 1e-6))
        .collect();
    let at_la = leak_rate_percent_per_day.map(|la| rg1183_leak(inventory, la, window, None));
    Leakage {
        at_la,
        per_percent_per_day,
    }
}

/// The containment removal channels a nuclide sees under RG 1.183 Rev. 1:
/// `(weight, removal rate [1/s])` pairs summing to weight 1. Noble gases are
/// not removed. **Iodine** is split by the RG's species, 95 % CsI (aerosol),
/// 4.85 % elemental, 0.15 % organic (not removed) (RG 1.183 Rev. 1 App.
/// A-1.1, which applies when the sump pH is kept at 7 or above; assumed).
/// Every other group, Br included, is particulate (A-1.1: "fission products
/// should be assumed to be in particulate form").
fn removal_channels(n: &str, removal: Option<(f64, f64)>) -> Vec<(f64, f64)> {
    let (aerosol, elemental) = removal.unwrap_or((0.0, 0.0));
    match element(n) {
        "Xe" | "Kr" => vec![(1.0, 0.0)],
        "I" => vec![(0.95, aerosol), (0.0485, elemental), (0.0015, 0.0)],
        _ => vec![(1.0, aerosol)],
    }
}

/// One phase of a release **into containment**: `fraction` of the core
/// inventory, released **linearly** between `onset_s` and `end_s` \[s after
/// the initiating event\].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReleasePhase {
    /// Fraction of core inventory released in this phase.
    pub fraction: f64,
    /// Phase onset \[s\].
    pub onset_s: f64,
    /// Phase end \[s\].
    pub end_s: f64,
}

/// RG 1.183 Rev. 1 Table 2 / Table 5 phases of a nuclide (gap, early
/// in-vessel), or `None` if its element has no Table 6 group.
fn rg1183_phases(n: &str) -> Option<Vec<ReleasePhase>> {
    let g = rg1183_group(element(n))?;
    let r = rows(RG1183_T2).find(|r| r[0] == g)?;
    let f: Vec<f64> = r[1..].iter().map(|x| x.parse().unwrap()).collect();
    let (t0, t1, t2) = (f[2] * 3600.0, f[3] * 3600.0, f[4] * 3600.0);
    Some(vec![
        ReleasePhase {
            fraction: f[0],
            onset_s: t0,
            end_s: t1,
        },
        ReleasePhase {
            fraction: f[1],
            onset_s: t1,
            end_s: t2,
        },
    ])
}

/// NUREG-1465 PWR release-phase timing, **Table 3.6** "Release Phase
/// Durations for PWRs and BWRs" (printed p. 9, PDF p. 18) and s.3.3 (printed
/// pp. 7-9), in hours.
pub mod n1465_timing {
    /// End of the coolant-activity phase = gap onset \[h\]: Table 3.6 gives
    /// "10 to 30 seconds" for PWRs without leak-before-break approval; the
    /// **30 s** end of that range is taken (it is also RG 1.183's 0.5 min).
    pub const GAP_ONSET_H: f64 = 30.0 / 3600.0;
    /// Gap-activity phase duration \[h\] (Table 3.6; = Table 3.13 header).
    pub const GAP_H: f64 = 0.5;
    /// Early in-vessel duration \[h\] (Table 3.6; = Table 3.13 header). It
    /// ends at **vessel breach** (s.3.3, p. 8).
    pub const EARLY_IN_VESSEL_H: f64 = 1.3;
    /// Ex-vessel duration \[h\], from vessel breach (Table 3.6, s.3.3 p. 9).
    pub const EX_VESSEL_H: f64 = 2.0;
    /// Late in-vessel duration \[h\]. It "commences at vessel breach and
    /// proceeds simultaneously with the occurrence of the ex-vessel phase"
    /// (s.3.3, p. 9), so it starts with the ex-vessel phase, not after it.
    pub const LATE_IN_VESSEL_H: f64 = 10.0;
    /// Vessel breach \[h\]: gap onset + gap + early in-vessel = 1.808 h.
    pub const VESSEL_BREACH_H: f64 = GAP_ONSET_H + GAP_H + EARLY_IN_VESSEL_H;
}

/// **NUREG-1465 Table 3.13 PWR, all four phases**, timed by Table 3.6
/// ([`n1465_timing`]): gap (30 s -> 0.508 h), early in-vessel (-> vessel
/// breach at 1.808 h), ex-vessel (1.808 -> 3.808 h) and late in-vessel
/// (1.808 -> 11.808 h, concurrent with ex-vessel). Release is **linear
/// within each phase**, an assumption: NUREG-1465 gives durations only, and
/// linear is RG 1.183 Rev. 1's stated default for its own phases. `None` if
/// the element has no Table 3.8 group (Table 3.8 grouping, as
/// [`nureg1465_pwr_into_containment`]).
pub fn nureg1465_pwr_phases(nuclide: &str) -> Option<[ReleasePhase; 4]> {
    use n1465_timing::*;
    let g = n1465_group(element(nuclide))?;
    let r = rows(N1465_T313).find(|r| r[0] == g)?;
    let f: Vec<f64> = r[1..].iter().map(|x| x.parse().unwrap()).collect();
    let h = 3600.0;
    let gap_end = GAP_ONSET_H + GAP_H;
    Some([
        ReleasePhase {
            fraction: f[0],
            onset_s: GAP_ONSET_H * h,
            end_s: gap_end * h,
        },
        ReleasePhase {
            fraction: f[1],
            onset_s: gap_end * h,
            end_s: VESSEL_BREACH_H * h,
        },
        ReleasePhase {
            fraction: f[2],
            onset_s: VESSEL_BREACH_H * h,
            end_s: (VESSEL_BREACH_H + EX_VESSEL_H) * h,
        },
        ReleasePhase {
            fraction: f[3],
            onset_s: VESSEL_BREACH_H * h,
            end_s: (VESSEL_BREACH_H + LATE_IN_VESSEL_H) * h,
        },
    ])
}

/// RG 1.183 containment -> environment at leak rate `l_percent` \[%/day\],
/// with an optional first-order removal `(aerosol, elemental iodine)`
/// \[1/s\] inside the containment (natural deposition). `None` is no
/// removal credit. Source: Table 2 fractions, linear over the Table 5
/// phases, terminating at the end of early in-vessel (App. A-2.1); leak
/// `L_a` for 24 h, then `L_a/2` (PWR, App. A-2.7); decay applied.
///
/// The integrator is [`containment_leak`] (shared with the severe-LOCA arm,
/// [`nuscale_severe_loca`]).
pub fn rg1183_leak(
    inventory: &Releases,
    l_percent: f64,
    window: Time,
    removal: Option<(f64, f64)>,
) -> Releases {
    containment_leak(inventory, rg1183_phases, l_percent, window, removal)
}

/// **Intact containment -> environment**, for any phased source: `phases`
/// gives each nuclide's releases into a well-mixed containment ([`None`]
/// drops the nuclide). Leak rate `l_percent` \[%/day\] for the first 24 h,
/// then half (RG 1.183 Rev. 1 App. A-2.7, PWR); optional first-order removal
/// `(aerosol, elemental iodine)` \[1/s\], split by the RG 1.183 App. A-1.1
/// iodine species; radioactive decay applied. The containment is assumed to
/// stay **intact** for the whole window (no failure, no bypass): the only
/// path out is the leak.
///
/// Integrated by exact exponential steps of 60 s (`dA/dt = S - (lambda + L
/// + lambda_removal) A`, released `= integral L A dt`). The source over a
/// step is the phase's rate times its overlap with the step, so every phase
/// delivers exactly its fraction. ~~Source evaluated at the step midpoint~~
/// **CHANGED 2026-09-30 (#464):** the midpoint rule gave RG 1.183's gap phase
/// (0.5 min -> 0.23 h = 798 s) 14 steps = 840 s, over-delivering it by 5.3 %
/// and early in-vessel short by 0.08 %; found while adding the severe-LOCA
/// arm; the DBA dose moved as recorded on #464.
pub fn containment_leak<P, V>(
    inventory: &Releases,
    phases: P,
    l_percent: f64,
    window: Time,
    removal: Option<(f64, f64)>,
) -> Releases
where
    P: Fn(&str) -> Option<V>,
    V: AsRef<[ReleasePhase]>,
{
    let window_s = window.get::<uom::si::time::second>();
    let dt = 60.0;
    let steps = (window_s / dt).ceil() as usize;
    let run = |n: &str, bq: f64, removal_per_s: f64| -> Option<f64> {
        let ph = phases(n)?;
        let ph = ph.as_ref();
        let lam = find_nuclide(n).map_or_else(
            || lambda_fallback(n),
            |x| x.decay_constant().get::<uom::si::frequency::hertz>(),
        );
        let l_full = l_percent / 100.0 / 86_400.0;
        let (mut a, mut released) = (0.0_f64, 0.0_f64);
        for k in 0..steps {
            let t = k as f64 * dt;
            let mid = t + 0.5 * dt;
            // Step-averaged source: each phase contributes in proportion to
            // its overlap with [t, t + dt], so every phase delivers exactly
            // its fraction whatever its alignment with the 60 s grid.
            let mut src = 0.0;
            for p in ph {
                let overlap = (t + dt).min(p.end_s) - t.max(p.onset_s);
                if overlap > 0.0 {
                    src += p.fraction * bq * overlap / ((p.end_s - p.onset_s) * dt);
                }
            }
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
            let x = (lam + l + removal_per_s) * dt;
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
    inventory
        .iter()
        .filter_map(|(n, bq)| {
            let mut total = 0.0;
            for (w, k) in removal_channels(n, removal) {
                total += run(n, w * bq, k)?;
            }
            Some((n.clone(), total))
        })
        .collect()
}

/// NuScale's maximum allowable containment (CNV) leak rate `L_a` \[%/day of
/// the containment air mass\]: **0.20 wt%/day at P_a**. NRC, *Phase 4 SER,
/// Chapter 6* (NuScale DCA), PDF p. 91: "The NuScale maximum allowable CNV
/// leak rate, La, is 0.20 wt% of the containment air mass per day at the
/// calculated Pa" (proprietary-filed here; cited, not redistributed). As a
/// fraction per day it is **unchanged by the power scaling** (maintainer
/// decision, 2026-09-30).
pub const NUSCALE_LA_PERCENT_PER_DAY: f64 = 0.20;

/// NuScale's minimum containment free volume \[ft^3\]: **6,000 ft^3**. Same
/// SER, PDF pp. 19-20 ("the minimum containment free volume is 6,000 ft3",
/// ADAMS ML18304A128).
pub const NUSCALE_CNV_FREE_VOLUME_FT3: f64 = 6_000.0;

/// **Containment scaled DOWN with power** (maintainer decision, 2026-09-30),
/// an ASSUMPTION stated as such: geometric similarity, free volume `V ∝ P`,
/// surface `S ∝ V^(2/3)`, so `S/V` grows by `(P_ref/P)^(1/3)`. Returns `(V
/// [m^3], S/V factor relative to the NuScale module)`. At 10 MWth: 375 ft^3
/// = 10.62 m^3 and `(160/10)^(1/3) = 2.520`. `L_a` (a fraction per day) is
/// unchanged; natural deposition (`∝ S/V`) is multiplied by the factor.
pub fn scaled_containment(thermal_power_mwth: f64) -> (f64, f64) {
    let v_ref_m3 = NUSCALE_CNV_FREE_VOLUME_FT3 * 0.028_316_846_592;
    (
        v_ref_m3 * thermal_power_mwth / NUSCALE_MODULE_MWTH,
        (NUSCALE_MODULE_MWTH / thermal_power_mwth).cbrt(),
    )
}

/// What the natural-deposition arm still needs, printed wherever it would be
/// (RG 1.183 Rev. 1 App. A-2.2: SRP 6.5.2 is the acceptable model;
/// NUREG/CR-6189 only case by case, adjusted to the Rev. 1 source term, at
/// 10th-percentile values).
pub const NATURAL_DEPOSITION_PENDING: &str = "pending literature: RG 1.183 Rev. 1 App. A-2.2 \
     accepts the natural-deposition model of NUREG-0800 (SRP) Section 6.5.2 (or NUREG/CR-6189, \
     ML100130305, case by case, adjusted, 10th percentile); neither is held, nor is NuScale's CNV \
     internal surface area (DCA Part 2 Tier 2 Ch. 6/15, or SER Section 15.0.3)";

/// Natural-deposition removal rates in the containment **at the NuScale
/// module** (160 MWt) \[1/s\]: an explicit input, never defaulted. `None`
/// means pending literature ([`NATURAL_DEPOSITION_PENDING`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NaturalDeposition {
    /// Aerosol removal rate at the reference containment \[1/s\].
    pub aerosol_per_s: Option<f64>,
    /// Elemental-iodine removal rate at the reference containment \[1/s\].
    pub elemental_iodine_per_s: Option<f64>,
}

impl NaturalDeposition {
    /// No rates: the arm reports [`NATURAL_DEPOSITION_PENDING`].
    pub const PENDING_LITERATURE: Self = Self {
        aerosol_per_s: None,
        elemental_iodine_per_s: None,
    };

    /// The rates at `thermal_power_mwth`, scaled by the `S/V` factor of
    /// [`scaled_containment`]; `None` unless both are given.
    pub fn at_power(&self, thermal_power_mwth: f64) -> Option<(f64, f64)> {
        let f = scaled_containment(thermal_power_mwth).1;
        Some((self.aerosol_per_s? * f, self.elemental_iodine_per_s? * f))
    }
}

/// An LWR arm released through the intact, leaking containment: the no-removal
/// result and, when the rates are given, the natural-deposition result
/// \[Bq per nuclide, to the environment\]. Used by both LWR LOCA arms,
/// [`nuscale_mha_loca`] (design basis) and [`nuscale_severe_loca`] (beyond
/// design basis, core melt).
#[derive(Debug, Clone, PartialEq)]
pub struct LwrContainedRelease {
    /// No removal credit.
    pub no_removal: Releases,
    /// Natural deposition only; `None` while pending literature.
    pub natural_deposition: Option<Releases>,
}

/// The design-basis arm's name for [`LwrContainedRelease`] (kept for callers).
pub type LwrDba = LwrContainedRelease;

/// The **LWR design-basis arm** (maintainer decision, 2026-09-30, #450):
/// RG 1.183 Rev. 1 MHA LOCA with the NuScale inventory scaled to
/// `thermal_power_mwth`, leaking at NuScale's `L_a` = 0.20 %/day (24 h, then
/// half), released to the environment over `window`.
pub fn nuscale_mha_loca(
    thermal_power_mwth: f64,
    window: Time,
    deposition: &NaturalDeposition,
) -> LwrContainedRelease {
    let inv = pwr_inventory_scaled(thermal_power_mwth);
    LwrContainedRelease {
        no_removal: rg1183_leak(&inv, NUSCALE_LA_PERCENT_PER_DAY, window, None),
        natural_deposition: deposition
            .at_power(thermal_power_mwth)
            .map(|r| rg1183_leak(&inv, NUSCALE_LA_PERCENT_PER_DAY, window, Some(r))),
    }
}

/// What the severe-LOCA arm assumes about the containment, printed wherever
/// the arm is shown (maintainer decision, 2026-09-30, #464).
pub const CONTAINED_CORE_MELT_ASSUMPTION: &str = "contained core melt: the containment is \
     ASSUMED to stay intact (no early failure, no bypass) and to leak only at L_a; WASH-1400 \
     PWR 1-3 are the containment-failure categories, for context";

/// The comparison's crediting basis (maintainer decision, 2026-09-30,
/// #450/#464), printed wherever the comparison is shown: "The comparison
/// neglects the pools because we are comparing technology at the reactor
/// level, not what is surrounding the reactor. If NuScale gives pool credit,
/// then HTGR can be submerged in a pool as well." **Each side is credited only
/// with its reactor-level inherent barrier**: the containment vessel leaking
/// at `L_a` (LWR), the TRISO particles (HTR). Nothing surrounding the reactor
/// is credited on either side. Pool scrubbing is excluded **by design**, not
/// pending literature; in-containment natural deposition is part of the
/// containment barrier and stays (pending literature).
pub const REACTOR_LEVEL_BASIS: &str = "reactor-level comparison (maintainer decision, \
     2026-09-30): each side is credited only with its reactor-level inherent barrier -- the \
     containment vessel leaking at L_a (LWR), the TRISO particles (HTR). Nothing surrounding the \
     reactor is credited: not NuScale's reactor pool, reactor building, sprays or filters; not \
     the HTR confinement/building (#409) or a hypothetical pool. Pool scrubbing is excluded by \
     design, not pending literature. \"If NuScale gives pool credit, then HTGR can be submerged \
     in a pool as well.\"";

/// The **LWR beyond-design-basis arm: LOCA with ECCS failure -> core melt**
/// (maintainer decision, 2026-09-30, #464). The NuScale inventory scaled to
/// `thermal_power_mwth` (as [`nuscale_mha_loca`]) is released into the
/// containment by **NUREG-1465 Table 3.13 PWR, all four phases**
/// ([`nureg1465_pwr_phases`]: gap, early in-vessel, ex-vessel, late
/// in-vessel, Table 3.6 timing). The containment is **intact** and leaks at
/// NuScale's `L_a` = 0.20 %/day for 24 h, then half, with the RG 1.183 App.
/// A-1.1 iodine species (95 % CsI, 4.85 % elemental, 0.15 % organic) for the
/// removal arm; decay applied; to the environment over `window`. The
/// integrator is the design-basis arm's ([`containment_leak`]).
///
/// **Assumption, stated** ([`CONTAINED_CORE_MELT_ASSUMPTION`]): this is a
/// *contained* core melt. No early containment failure, no bypass, no
/// basemat melt-through inside the window. NUREG-1465's ex-vessel phase
/// (core-concrete interaction) is taken as releasing into the same intact
/// atmosphere. WASH-1400 PWR 1-3 are the containment-failure categories.
pub fn nuscale_severe_loca(
    thermal_power_mwth: f64,
    window: Time,
    deposition: &NaturalDeposition,
) -> LwrContainedRelease {
    let inv = pwr_inventory_scaled(thermal_power_mwth);
    let leak = |r| {
        containment_leak(
            &inv,
            nureg1465_pwr_phases,
            NUSCALE_LA_PERCENT_PER_DAY,
            window,
            r,
        )
    };
    LwrContainedRelease {
        no_removal: leak(None),
        natural_deposition: deposition
            .at_power(thermal_power_mwth)
            .map(|r| leak(Some(r))),
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
    /// Nuclides lacking a coefficient on some pathway. ~~(NOT counted as
    /// zero)~~ **CORRECTED 2026-09-30:** a missing pathway contributes
    /// nothing to `total_sv` or `by_group`, so those are LOWER BOUNDS; this
    /// list says which nuclides make them so (#456).
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

/// **HTR-10 design-basis releases** to the environment \[Bq per nuclide\]:
/// Liu & Cao (2002, NED 218:81-90) **Table 8**, either the depressurisation
/// (DN65 charging-tube rupture, s.4.1.1) or the water ingress (two SG tubes,
/// relief failed, s.4.1.2), as `changi::activity::accident_airborne_release`
/// holds it. Published, not computed here. H-3 and C-14 are included; their
/// missing FGR coefficients are reported by [`max_dose`], not zeroed silently
/// (#452, 2026-09-30).
pub fn htr10_dba_release(case: AccidentCase) -> Releases {
    htr10_accident_release()
        .iter()
        .map(|e| (e.nuclide.to_string(), e.release(case).get::<becquerel>()))
        .collect()
}

/// One distance of the Table 9 cross-check of [`htr10_dba_release`] through
/// the shared dose chain.
#[derive(Debug, Clone, PartialEq)]
pub struct Table9Check {
    /// Receptor distance \[m\].
    pub distance_m: f64,
    /// Our maximum dose \[mSv\]: [`max_dose`] with
    /// [`DoseAssumptions::bounding_example`] (worst class, 1 m/s, ground
    /// release, 96 h, submersion + groundshine + inhalation).
    pub ours_msv: f64,
    /// Liu & Cao Table 9 "whole-body" \[mSv\] (their 40 m stack and their
    /// unpublished weather; STOERNEU).
    pub published_whole_body_msv: f64,
    /// `ours / published`.
    pub ratio: f64,
}

/// Cross-check [`htr10_dba_release`] through [`max_dose`] against Liu & Cao
/// Table 9 at its own distances. **Different conditions, stated:** the
/// shared chain is a ground-level release at the worst class and 1 m/s (the
/// #452 conditions), while Table 9 comes from a 40 m stack under weather the
/// paper does not give. The ratio is a finding, not a gate, and nothing is
/// tuned to it. Liu & Cao's own conditions are swept (external pathways only)
/// in `buangkok/tests/liu_cao_external_dose_cross_check.rs` (#379).
///
/// Measured 2026-09-30: ours/Table 9 = 0.12 (depressurisation) and 0.08
/// (water ingress) at 250 m, 0.01-0.04 from 0.75 to 15 km, and 1.46 / 1.13
/// at 75 km. That is the gap #379 found (Table 9's unstated integration
/// period and weather), made larger here by the 96 h groundshine window.
pub fn htr10_dba_vs_table9(case: AccidentCase) -> Vec<Table9Check> {
    let rel = htr10_dba_release(case);
    let a = DoseAssumptions::bounding_example();
    htr10_accident_dose_by_distance()
        .iter()
        .map(|row| {
            let x = row.distance.get::<meter>();
            let ours = 1e3 * max_dose(&rel, x, a).total_sv;
            let published = row.doses(case).whole_body_msv;
            Table9Check {
                distance_m: x,
                ours_msv: ours,
                published_whole_body_msv: published,
                ratio: ours / published,
            }
        })
        .collect()
}

/// The comparison's tiers (maintainer decision, 2026-09-30, #450, #464):
/// **paired by initiating event and severity**, DLOFC (HTR) against LOCA
/// (LWR).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// HTR-10 DLOFC (Liu & Cao Table 8) vs LWR design-basis LOCA (RG 1.183).
    DesignBasis,
    /// HTR-10 DLOFC + air ingress (KORA bound) vs LWR LOCA with ECCS failure
    /// and core melt, in an intact containment.
    BeyondDesignBasis,
    /// WASH-1400 PWR 8: context only.
    Context,
}

impl Tier {
    /// The tier's heading, used by the #452 example, the map table and the
    /// headless CSV.
    pub fn label(self) -> &'static str {
        match self {
            Tier::DesignBasis => "Design basis: DLOFC vs LOCA",
            Tier::BeyondDesignBasis => {
                "Beyond design basis: DLOFC + air ingress (bounding) vs LOCA + core melt"
            }
            Tier::Context => {
                "Context: WASH-1400 PWR 8 -- no-melt, uncontained; paired by containment \
                 state, not initiator"
            }
        }
    }
}

/// One dose column of the comparison: its tier, a heading and a CSV key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArmColumn {
    /// Which tier the column belongs to.
    pub tier: Tier,
    /// Short heading (the unit, mSv, is the caller's to add).
    pub heading: &'static str,
    /// Column name for CSV output, in mSv.
    pub csv_key: &'static str,
}

/// The seven dose columns, **ordered by tier**, in the order of
/// [`ComparisonRow::arm_doses_sv`]. The map, the headless CSV and the #452
/// example all read this, so they cannot disagree on order or labels.
pub const ARM_COLUMNS: [ArmColumn; 7] = [
    ArmColumn {
        tier: Tier::DesignBasis,
        heading: "DB: HTR-10 DLOFC (depressurisation, Liu & Cao T8)",
        csv_key: "db_htr10_dlofc_msv",
    },
    ArmColumn {
        tier: Tier::DesignBasis,
        heading: "DB: LWR LOCA (RG 1.183), no removal",
        csv_key: "db_lwr_loca_no_removal_msv",
    },
    ArmColumn {
        tier: Tier::DesignBasis,
        heading: "DB: LWR LOCA (RG 1.183), natural deposition",
        csv_key: "db_lwr_loca_natural_deposition_msv",
    },
    ArmColumn {
        tier: Tier::BeyondDesignBasis,
        heading: "BDB: HTR-10 DLOFC + air ingress (KORA bound)",
        csv_key: "bdb_htr10_dlofc_air_ingress_kora_msv",
    },
    ArmColumn {
        tier: Tier::BeyondDesignBasis,
        heading: "BDB: LWR LOCA + core melt (NUREG-1465 all phases), no removal",
        csv_key: "bdb_lwr_loca_core_melt_no_removal_msv",
    },
    ArmColumn {
        tier: Tier::BeyondDesignBasis,
        heading: "BDB: LWR LOCA + core melt, natural deposition",
        csv_key: "bdb_lwr_loca_core_melt_natural_deposition_msv",
    },
    ArmColumn {
        tier: Tier::Context,
        heading: "Context: WASH-1400 PWR 8 (no melt, uncontained)",
        csv_key: "context_wash1400_pwr8_msv",
    },
];

/// One distance of the HTR-10 / LWR comparison: the maximum 96 h dose of
/// each arm \[Sv\], same site, weather, height and receptor (#452, #453).
///
/// ~~**Framing (maintainer decision, 2026-09-30, #450): design basis against
/// design basis is the PRIMARY comparison** ... The KORA bound against
/// WASH-1400 PWR 8 is the SECONDARY, beyond-design-basis bounding
/// comparison.~~ **CHANGED 2026-09-30 (maintainer decision, #464): paired by
/// initiating event and severity**, DLOFC against LOCA, in two tiers
/// ([`Tier`]): design basis (HTR-10 DLOFC vs RG 1.183 LOCA) and beyond design
/// basis (HTR-10 DLOFC + air-ingress bound vs LWR LOCA + core melt).
/// WASH-1400 PWR 8 is context only: no melt, uncontained, paired by
/// containment state rather than initiator. Fields are in tier order.
#[derive(Debug, Clone, PartialEq)]
pub struct ComparisonRow {
    /// Receptor distance \[m\].
    pub distance_m: f64,
    /// Worst stability class at 1 m/s. It depends on distance only, so it is
    /// the same for every arm.
    pub class: StabilityClass,
    /// Design basis: HTR-10 DLOFC (depressurisation), Liu & Cao Table 8
    /// ([`htr10_dba_release`]).
    pub htr10_dba_depressurisation_sv: f64,
    /// Design basis: LWR LOCA, RG 1.183 MHA ([`nuscale_mha_loca`]), `L_a`
    /// 0.20 %/day, **no removal credit**.
    pub lwr_dba_no_removal_sv: f64,
    /// Design basis: the same with **natural deposition only**; `None` while
    /// pending literature ([`NATURAL_DEPOSITION_PENDING`]).
    pub lwr_dba_natural_deposition_sv: Option<f64>,
    /// Beyond design basis: HTR-10 DLOFC + air ingress, KORA bound
    /// ([`htr10_air_ingress_bound`]).
    pub htr10_bound_sv: f64,
    /// Beyond design basis: LWR LOCA + core melt, NUREG-1465 Table 3.13 all
    /// phases, intact containment at `L_a` ([`nuscale_severe_loca`]), **no
    /// removal credit**.
    pub lwr_severe_loca_no_removal_sv: f64,
    /// Beyond design basis: the same with natural deposition only; `None`
    /// while pending literature.
    pub lwr_severe_loca_natural_deposition_sv: Option<f64>,
    /// Context: WASH-1400 PWR 8, to the atmosphere.
    pub wash1400_pwr8_sv: f64,
}

impl ComparisonRow {
    /// The seven doses \[Sv\] in the order of [`ARM_COLUMNS`]; `None` is a
    /// pending arm (print "pending literature", never 0).
    pub fn arm_doses_sv(&self) -> [Option<f64>; 7] {
        [
            Some(self.htr10_dba_depressurisation_sv),
            Some(self.lwr_dba_no_removal_sv),
            self.lwr_dba_natural_deposition_sv,
            Some(self.htr10_bound_sv),
            Some(self.lwr_severe_loca_no_removal_sv),
            self.lwr_severe_loca_natural_deposition_sv,
            Some(self.wash1400_pwr8_sv),
        ]
    }
}

/// Share of each arm's released Bq whose nuclide lacks an FGR coefficient on
/// some pathway. Those pathways count zero, so a non-zero share marks a
/// LOWER-BOUND dose (#456).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IncompleteShares {
    /// HTR-10 depressurisation DBA (H-3, C-14).
    pub htr10_dba: f64,
    /// LWR MHA LOCA, no removal.
    pub lwr_dba: f64,
    /// HTR-10 KORA bound.
    pub htr10_bound: f64,
    /// LWR LOCA + core melt, no removal.
    pub lwr_severe_loca: f64,
    /// WASH-1400 PWR 8.
    pub wash1400: f64,
}

/// The comparison over `distances_m` (#452, and `htgr_sim_v1`'s map, #453),
/// from one call so that the two cannot drift apart.
#[derive(Debug, Clone, PartialEq)]
pub struct BoundingComparison {
    /// One row per distance, in the order given.
    pub rows: Vec<ComparisonRow>,
    /// Coverage of the FGR tables per arm.
    pub incomplete: IncompleteShares,
}

/// Build the [`BoundingComparison`]: the design-basis tier (HTR-10 DLOFC,
/// [`htr10_dba_release`]; LWR LOCA, [`nuscale_mha_loca`] with `deposition`),
/// the beyond-design-basis tier (HTR-10 DLOFC + air-ingress bound,
/// [`htr10_air_ingress_bound`]; LWR LOCA + core melt, [`nuscale_severe_loca`]
/// with `deposition`) and the WASH-1400 PWR 8 context arm, with the LWR
/// inventory and containment scaled to `mwth`, through [`max_dose`] with
/// [`DoseAssumptions::bounding_example`].
///
/// # Errors
/// If the HTR-10 release chain rejects its inputs.
pub fn bounding_comparison(
    geometry: Htr10Geometry,
    window: Time,
    mwth: f64,
    distances_m: &[f64],
    deposition: &NaturalDeposition,
) -> Result<BoundingComparison, SembawangError> {
    let dba = htr10_dba_release(AccidentCase::Depressurization);
    let lwr = nuscale_mha_loca(mwth, window, deposition);
    let htr = htr10_air_ingress_bound(geometry, window)?;
    let severe = nuscale_severe_loca(mwth, window, deposition);
    let wash = wash1400_pwr8_to_atmosphere(&pwr_inventory_scaled(mwth));
    let a = DoseAssumptions::bounding_example();
    let opt = |r: &Option<Releases>, x: f64| r.as_ref().map(|r| max_dose(r, x, a).total_sv);
    let rows: Vec<ComparisonRow> = distances_m
        .iter()
        .map(|&x| {
            let h = max_dose(&htr, x, a);
            ComparisonRow {
                distance_m: x,
                class: h.class,
                htr10_dba_depressurisation_sv: max_dose(&dba, x, a).total_sv,
                lwr_dba_no_removal_sv: max_dose(&lwr.no_removal, x, a).total_sv,
                lwr_dba_natural_deposition_sv: opt(&lwr.natural_deposition, x),
                htr10_bound_sv: h.total_sv,
                lwr_severe_loca_no_removal_sv: max_dose(&severe.no_removal, x, a).total_sv,
                lwr_severe_loca_natural_deposition_sv: opt(&severe.natural_deposition, x),
                wash1400_pwr8_sv: max_dose(&wash, x, a).total_sv,
            }
        })
        .collect();
    Ok(BoundingComparison {
        rows,
        incomplete: IncompleteShares {
            htr10_dba: incomplete_share(&dba),
            lwr_dba: incomplete_share(&lwr.no_removal),
            htr10_bound: incomplete_share(&htr),
            lwr_severe_loca: incomplete_share(&severe.no_removal),
            wash1400: incomplete_share(&wash),
        },
    })
}

/// Share of `rel`'s Bq whose nuclide lacks an FGR coefficient on some pathway
/// (see [`Dose::missing`]).
pub fn incomplete_share(rel: &Releases) -> f64 {
    let missing = max_dose(rel, 1000.0, DoseAssumptions::bounding_example()).missing;
    let tot: f64 = rel.iter().map(|(_, b)| b).sum();
    let miss: f64 = rel
        .iter()
        .filter(|(n, _)| missing.contains(n))
        .map(|(_, b)| b)
        .sum();
    // `+ 0.0` turns the empty sum's -0.0 into 0.0 for printing.
    if tot > 0.0 {
        miss / tot + 0.0
    } else {
        0.0
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

    /// The HTR-10 DBA arm is Liu & Cao Table 8 as `changi` holds it (18
    /// nuclides; Xe-133 2.2e10 / 6.5e8 Bq, I-131 2.5e7 / 2.2e8 Bq), and its
    /// Table 9 cross-check covers all 13 published distances with finite,
    /// positive doses. The ratios are recorded on #452, not asserted.
    #[test]
    fn htr10_dba_arm_is_table_8_and_meets_table_9_distances() {
        let get = |r: &Releases, n: &str| r.iter().find(|(m, _)| m == n).unwrap().1;
        let d = htr10_dba_release(AccidentCase::Depressurization);
        let w = htr10_dba_release(AccidentCase::WaterIngress);
        assert_eq!((d.len(), w.len()), (18, 18));
        assert_eq!((get(&d, "Xe-133"), get(&w, "Xe-133")), (2.2e10, 6.5e8));
        assert_eq!((get(&d, "I-131"), get(&w, "I-131")), (2.5e7, 2.2e8));
        for case in [AccidentCase::Depressurization, AccidentCase::WaterIngress] {
            let rows = htr10_dba_vs_table9(case);
            assert_eq!(rows.len(), 13);
            assert_eq!(rows[0].distance_m, 250.0);
            assert!(rows
                .iter()
                .all(|r| r.ours_msv.is_finite() && r.ours_msv > 0.0));
        }
    }

    /// `bound::F_OX_KORA` is the committed TECDOC-978 Table 5-7 row it
    /// cites (AVR 92/22, 16 400 particles, 1400 °C, 140 h, 20 failed), and
    /// the printed fraction is failed/particles to its printed precision.
    /// Fig. 5-23's 1400 °C Nabielek prediction at 140 h is recorded for
    /// context (2026-09-30: 1.59e-2, about 13x the measurement; not used).
    #[test]
    fn f_ox_kora_is_the_table_5_7_sphere_test() {
        assert_eq!(kora::table_5_7().len(), 7);
        let t = kora::sphere_test(bound::HOLD_CELSIUS, bound::HOLD_HOURS).unwrap();
        assert_eq!(t.sample, "AVR 92/22");
        assert_eq!((t.particles, t.failed), (16_400.0, 20.0));
        assert_eq!(t.failed_fraction, bound::F_OX_KORA);
        assert!((t.failed / t.particles - t.failed_fraction).abs() < 0.05e-3);
        let p = kora::nabielek_1400c_prediction(140.0).unwrap();
        assert!((p - 1.59e-2).abs() < 0.01e-2, "{p}");
        assert!(kora::nabielek_1400c_prediction(1000.0).is_none());
    }

    /// The LWR DBA arm: `L_a` = 0.20 %/day as cited; with zero removal the
    /// natural-deposition path equals no-removal exactly; a removal rate
    /// lowers every non-noble release and leaves noble gases untouched; the
    /// iodine species split leaves 0.15 % organic unremovable; the default is
    /// pending literature (None). The rates used here are **test inputs, not
    /// data**. The containment scaling at 10 MWth: 10.62 m^3 and 2.520x S/V.
    #[test]
    fn lwr_dba_arm_removal_and_scaling() {
        let w = Time::new::<hour>(96.0);
        let pending = nuscale_mha_loca(10.0, w, &NaturalDeposition::PENDING_LITERATURE);
        assert!(pending.natural_deposition.is_none());
        let zero = NaturalDeposition {
            aerosol_per_s: Some(0.0),
            elemental_iodine_per_s: Some(0.0),
        };
        let z = nuscale_mha_loca(10.0, w, &zero);
        assert_eq!(z.natural_deposition.as_ref().unwrap(), &z.no_removal);
        let some = NaturalDeposition {
            aerosol_per_s: Some(1e-4),
            elemental_iodine_per_s: Some(1e-3),
        };
        let r = nuscale_mha_loca(10.0, w, &some);
        let dep = r.natural_deposition.unwrap();
        for ((n, a), (_, b)) in r.no_removal.iter().zip(&dep) {
            if matches!(element(n), "Xe" | "Kr") {
                assert_eq!(a, b, "{n}");
            } else if *a > 0.0 {
                assert!(b < a, "{n}");
            }
        }
        let i131 = |rel: &Releases| rel.iter().find(|(n, _)| n == "I-131").unwrap().1;
        assert!(i131(&dep) > 0.0015 * 0.5 * i131(&r.no_removal));
        let (v, f) = scaled_containment(10.0);
        assert!(
            (v - 10.6188).abs() < 1e-3 && (f - 2.5198).abs() < 1e-3,
            "{v} {f}"
        );
        assert_eq!(NUSCALE_LA_PERCENT_PER_DAY, 0.20);
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

    /// The severe-LOCA source is NUREG-1465 Table 3.13 (all four phases) on
    /// Table 3.6 timing: for every nuclide of the scaled inventory the four
    /// phase fractions sum to [`nureg1465_pwr_into_containment`]'s all-phase
    /// fraction; the printed all-phase totals per group (noble gases 1.0,
    /// halogens 0.75, alkali metals 0.75, Te 0.305, Ba/Sr 0.12, noble metals
    /// 0.005, Ce 0.0055, La 0.0052); vessel breach at 30 s + 0.5 h + 1.3 h;
    /// late in-vessel concurrent with ex-vessel; every phase a whole number of
    /// 60 s steps long.
    #[test]
    fn severe_loca_phases_are_table_3_13_on_table_3_6_timing() {
        let inv = pwr_inventory_scaled(10.0);
        let all = nureg1465_pwr_into_containment(&inv, N1465Phases::All);
        for ((n, bq), (m, rel)) in inv
            .iter()
            .filter(|(n, _)| n1465_group(element(n)).is_some())
            .zip(&all)
        {
            assert_eq!(n, m);
            let f: f64 = nureg1465_pwr_phases(n)
                .unwrap()
                .iter()
                .map(|p| p.fraction)
                .sum();
            assert!((f * bq - rel).abs() <= 1e-12 * rel.max(1.0), "{n}");
        }
        let total = |n: &str| -> f64 {
            nureg1465_pwr_phases(n)
                .unwrap()
                .iter()
                .map(|p| p.fraction)
                .sum()
        };
        for (n, t) in [
            ("Xe-133", 1.0),
            ("I-131", 0.75),
            ("Cs-137", 0.75),
            ("Te-132", 0.305),
            ("Sr-90", 0.12),
            ("Ru-106", 0.005),
            ("Ce-144", 0.0055),
            ("La-140", 0.0052),
        ] {
            assert!((total(n) - t).abs() < 1e-12, "{n}: {}", total(n));
        }
        let p = nureg1465_pwr_phases("I-131").unwrap();
        let h = 3600.0;
        assert!((p[0].onset_s - 30.0).abs() < 1e-9);
        assert!((p[1].end_s - (30.0 + 1.8 * h)).abs() < 1e-9);
        assert_eq!(p[2].onset_s, p[1].end_s);
        assert_eq!(
            p[3].onset_s, p[2].onset_s,
            "late in-vessel starts at breach"
        );
        assert!((p[2].end_s - p[2].onset_s - 2.0 * h).abs() < 1e-9);
        assert!((p[3].end_s - p[3].onset_s - 10.0 * h).abs() < 1e-9);
        for q in p {
            let steps = (q.end_s - q.onset_s) / 60.0;
            assert!((steps - steps.round()).abs() < 1e-9, "{steps}");
        }
    }

    /// Conservation of the phased source through [`containment_leak`]: for a
    /// STABLE tracer of each group (a label with no decay data, so lambda =
    /// 0) and a leak so fast that nothing stays in the containment, the
    /// release to the environment equals the Table 3.13 all-phase total times
    /// the inventory, to 1e-9. At `L_a` it is never more than the delivered
    /// amount, and never negative.
    #[test]
    fn severe_loca_source_is_conserved_through_the_leak() {
        let w = Time::new::<hour>(96.0);
        let tracers: Releases = [
            "Xe-999", "I-999", "Cs-999", "Te-999", "Sr-999", "Ru-999", "Ce-999", "La-999",
        ]
        .iter()
        .map(|n| (n.to_string(), 1.0e15))
        .collect();
        let fast = containment_leak(&tracers, nureg1465_pwr_phases, 1.0e7, w, None);
        let slow = containment_leak(
            &tracers,
            nureg1465_pwr_phases,
            NUSCALE_LA_PERCENT_PER_DAY,
            w,
            None,
        );
        assert_eq!(fast.len(), tracers.len());
        for ((n, f), (_, s)) in fast.iter().zip(&slow) {
            let delivered: f64 = nureg1465_pwr_phases(n)
                .unwrap()
                .iter()
                .map(|p| p.fraction)
                .sum::<f64>()
                * 1.0e15;
            assert!(
                (f - delivered).abs() <= 1e-9 * delivered,
                "{n}: {f} vs {delivered}"
            );
            assert!(*s >= 0.0 && *s <= delivered, "{n}: {s}");
        }
    }

    /// The same conservation ledger for the DESIGN-BASIS arm's source: stable
    /// tracers of every RG 1.183 Table 6 group, leaked at once, release exactly
    /// the Table 2 gap + early in-vessel fraction. Pins the 2026-09-30 defect
    /// (#464): the midpoint rule gave the 798 s gap phase 840 s of source and
    /// delivered 1.053x its fraction (noble gases 0.9632 for 0.962).
    #[test]
    fn dba_source_is_conserved_through_the_leak() {
        let w = Time::new::<hour>(96.0);
        let tracers: Releases = [
            "Xe-999", "I-999", "Cs-999", "Te-999", "Sr-999", "Ru-999", "Ce-999", "La-999", "Mo-999",
        ]
        .iter()
        .map(|n| (n.to_string(), 1.0e15))
        .collect();
        let fast = containment_leak(&tracers, rg1183_phases, 1.0e7, w, None);
        let into = rg1183_pwr_into_containment(&tracers);
        assert_eq!(fast.len(), into.len());
        for ((n, f), (_, c)) in fast.iter().zip(&into) {
            assert!((f - c).abs() <= 1e-9 * c.max(1.0), "{n}: {f} vs {c}");
        }
    }

    /// Every nuclide of the severe-LOCA arm leaks a non-negative amount, and
    /// never more than its all-phase release x (L_a x 4 days) -- the bound the
    /// DBA arm is held to, with the Table 3.13 fraction in place of 1.
    #[test]
    fn severe_loca_leakage_is_non_negative_and_bounded() {
        let inv = pwr_inventory_scaled(10.0);
        let arm = nuscale_severe_loca(
            10.0,
            Time::new::<hour>(96.0),
            &NaturalDeposition::PENDING_LITERATURE,
        );
        assert!(arm.natural_deposition.is_none(), "pending literature");
        let into = nureg1465_pwr_into_containment(&inv, N1465Phases::All);
        assert_eq!(arm.no_removal.len(), into.len());
        for (n, v) in &arm.no_removal {
            let c = into.iter().find(|(m, _)| m == n).unwrap().1;
            assert!(*v >= 0.0, "{n}: {v}");
            assert!(
                *v <= c * NUSCALE_LA_PERCENT_PER_DAY * 4.0 / 100.0,
                "{n}: {v} > {c}"
            );
        }
    }

    /// The prediction stated before the first run (#464): a LOCA with core
    /// melt releases more into the same leaking containment than the RG 1.183
    /// design-basis LOCA (Table 3.13 all phases vs Table 2 gap + early
    /// in-vessel: halogens 0.75 vs 0.377, alkali metals 0.75 vs 0.235, Ba/Sr
    /// 0.12 vs 0.0054), so its dose must be at least the DBA's at every
    /// distance, and so must those three groups. Also: a zero removal rate
    /// reproduces no-removal exactly.
    #[test]
    fn severe_loca_dose_is_at_least_the_rg1183_dba_dose() {
        let w = Time::new::<hour>(96.0);
        let a = DoseAssumptions::bounding_example();
        let dba = nuscale_mha_loca(10.0, w, &NaturalDeposition::PENDING_LITERATURE);
        let sev = nuscale_severe_loca(10.0, w, &NaturalDeposition::PENDING_LITERATURE);
        for x in [400.0, 1000.0, 10_000.0] {
            let (d, s) = (
                max_dose(&dba.no_removal, x, a),
                max_dose(&sev.no_removal, x, a),
            );
            assert!(
                s.total_sv >= d.total_sv,
                "{x}: {} < {}",
                s.total_sv,
                d.total_sv
            );
            for g in [Group::Halogens, Group::AlkaliMetals, Group::BariumStrontium] {
                let get = |v: &Dose| v.by_group.iter().find(|(k, _)| *k == g).unwrap().1;
                assert!(get(&s) >= get(&d), "{x} {g:?}");
            }
        }
        let zero = NaturalDeposition {
            aerosol_per_s: Some(0.0),
            elemental_iodine_per_s: Some(0.0),
        };
        let z = nuscale_severe_loca(10.0, w, &zero);
        assert_eq!(z.natural_deposition.as_ref().unwrap(), &z.no_removal);
    }

    /// [`ARM_COLUMNS`] is in tier order (design basis, beyond design basis,
    /// context), carries the maintainer's tier labels, and matches
    /// [`ComparisonRow::arm_doses_sv`] position for position.
    #[test]
    fn arm_columns_are_tier_ordered_and_match_the_row() {
        let rank = |t: Tier| match t {
            Tier::DesignBasis => 0,
            Tier::BeyondDesignBasis => 1,
            Tier::Context => 2,
        };
        assert!(ARM_COLUMNS
            .windows(2)
            .all(|w| rank(w[0].tier) <= rank(w[1].tier)));
        assert_eq!(Tier::DesignBasis.label(), "Design basis: DLOFC vs LOCA");
        assert_eq!(
            Tier::BeyondDesignBasis.label(),
            "Beyond design basis: DLOFC + air ingress (bounding) vs LOCA + core melt"
        );
        assert!(Tier::Context.label().contains("not initiator"));
        let row = ComparisonRow {
            distance_m: 1.0,
            class: StabilityClass::ALL[0],
            htr10_dba_depressurisation_sv: 1.0,
            lwr_dba_no_removal_sv: 2.0,
            lwr_dba_natural_deposition_sv: None,
            htr10_bound_sv: 4.0,
            lwr_severe_loca_no_removal_sv: 5.0,
            lwr_severe_loca_natural_deposition_sv: None,
            wash1400_pwr8_sv: 7.0,
        };
        assert_eq!(
            row.arm_doses_sv(),
            [
                Some(1.0),
                Some(2.0),
                None,
                Some(4.0),
                Some(5.0),
                None,
                Some(7.0)
            ]
        );
    }

    /// [`bounding_comparison`] (what the map and the #452 example read) gives,
    /// for the severe-LOCA column, exactly [`max_dose`] of
    /// [`nuscale_severe_loca`], at or above the DBA LOCA column at every
    /// distance, with the coverage share reported.
    #[test]
    fn bounding_comparison_carries_the_severe_loca_arm() {
        let particle = tampines::pebble_bed::triso::TrisoParticle::htr10();
        let pebble = tampines::pebble_bed::pebble::Pebble::htr10();
        let geometry = Htr10Geometry {
            kernel_radius: particle.kernel_radius,
            sic_thickness: particle.silicon_carbide_outer_radius - particle.inner_pyc_outer_radius,
            graphite_thickness: pebble.outer_radius - pebble.fuelled_zone_radius,
        };
        let w = Time::new::<hour>(96.0);
        let dep = NaturalDeposition::PENDING_LITERATURE;
        let xs = [400.0, 1000.0];
        let c = bounding_comparison(geometry, w, 10.0, &xs, &dep).unwrap();
        let sev = nuscale_severe_loca(10.0, w, &dep);
        let a = DoseAssumptions::bounding_example();
        for (row, x) in c.rows.iter().zip(xs) {
            assert_eq!(
                row.lwr_severe_loca_no_removal_sv,
                max_dose(&sev.no_removal, x, a).total_sv
            );
            assert_eq!(row.lwr_severe_loca_natural_deposition_sv, None);
            assert!(row.lwr_severe_loca_no_removal_sv >= row.lwr_dba_no_removal_sv);
        }
        assert!(c.incomplete.lwr_severe_loca > 0.0 && c.incomplete.lwr_severe_loca < 1.0);
    }

    /// **A near-isothermal air ingress still vents (#469 item 4).** Air
    /// ingress carries the release out by gas exchange whatever the
    /// temperature does, so the bound must not depend on heat-up expansion.
    ///
    /// Methodology: the bounding release over the exact 1400 °C hold, and over
    /// the same hold drifting up by 0.1 K across the 96 h window (97 samples,
    /// same fractions, same geometry). Upstream's heat-up venting
    /// (`coolant_release`) vents `≈ 1 − T0/T ≈ 6e-5` of the drifting case, so a
    /// bound built on it collapses to the circulating activity; a transport
    /// fraction (here `Venting::FullFlowThrough`) is indifferent to the 0.1 K.
    /// Pass: the summed release of the drifting case is within 1 % of the
    /// exact hold's. Results (2026-09-30): printed; fails with
    /// `Venting::Upstream`, passes with `FullFlowThrough`.
    #[test]
    fn a_near_isothermal_air_ingress_still_vents() {
        let particle = tampines::pebble_bed::triso::TrisoParticle::htr10();
        let pebble = tampines::pebble_bed::pebble::Pebble::htr10();
        let geometry = Htr10Geometry {
            kernel_radius: particle.kernel_radius,
            sic_thickness: particle.silicon_carbide_outer_radius - particle.inner_pyc_outer_radius,
            graphite_thickness: pebble.outer_radius - pebble.fuelled_zone_radius,
        };
        let samples = 97;
        let times: Vec<Time> = (0..samples)
            .map(|i| Time::new::<hour>(96.0 * i as f64 / (samples - 1) as f64))
            .collect();
        let at = |drift_k: f64| {
            let temps = (0..samples)
                .map(|i| {
                    vec![ThermodynamicTemperature::new::<degree_celsius>(
                        bound::HOLD_CELSIUS + drift_k * i as f64 / (samples - 1) as f64,
                    )]
                })
                .collect();
            let tr = TemperatureTransient::from_nodes(times.clone(), vec![temps]).unwrap();
            let r = air_ingress_release_over(geometry, air_ingress_bound_fractions(), &tr)
                .expect("release chain runs");
            r.iter().map(|(_, bq)| bq).sum::<f64>()
        };
        let (flat, drifting) = (at(0.0), at(0.1));
        println!("bound release: isothermal {flat:.4e} Bq, +0.1 K drift {drifting:.4e} Bq");
        assert!(flat > 0.0);
        assert!(
            (drifting / flat - 1.0).abs() < 0.01,
            "a 0.1 K drift must not change the vented release: {drifting:e} vs {flat:e}"
        );
    }
}
