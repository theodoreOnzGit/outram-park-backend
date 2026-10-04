//! HTR-10 air ingress, **bounding case B**: a 1400 °C hold, with the KORA
//! measured SiC-oxidation failure added to boon-lay fuel failure, then released
//! through TRISO-ATOPS (GitHub #434, #435, #438).
//!
//! > **Research, education and V&V only** (`RESPONSIBLE_USE.md`). This is not a
//! > source term for HTR-10 or for any facility~~, and it computes no dose~~.
//! > **CORRECTED 2026-10-04:** it does compute a research-grade dose at
//! > distance, through `buangkok`'s Gaussian plume and FGR coefficients (see
//! > "Maximum dose" under Results); that dose is a property of this bounding
//! > model, never a dose to a real person.
//!
//! Written for the maintainer to **inspect by hand and re-code**. Every input
//! is a named constant with its source beside it, and the arithmetic is kept
//! in `main` in the order it is done.
//!
//! # Methodology
//!
//! **What is computed.** The activity released from the HTR-10 core by an air
//! ingress, in the **bounding case** the maintainer chose on 2026-09-30:
//! - the whole core held at **1400 °C for 140 h**;
//! - the matrix burned to bare SiC, so every particle is exposed;
//! - the pressure 1 atm (depressurised).
//!
//! **The failure fractions, TRISO-ATOPS classes** (`AccidentFractions`):
//!
//! | Field | Class | Value | Source |
//! |---|---|---|---|
//! | `heavy_metal` (f_hm) | full | 3e-4 | Liu & Cao 2002 §2.1, HTR-10 design free uranium |
//! | `incremental` (f_inc) | full | 5e-4 | Liu & Cao 2002 §2.1, design irradiation failure |
//! | `sic` | SiC-only | 1e-4 | **stand-in**, the NP-MHTGR reference; no HTR-10 value exists |
//! | `incremental_sic` | SiC-only | 3.6e-5 | **stand-in**, NP-MHTGR |
//! | `incremental_accident` | full | Δφ_BL + **1.2e-3** | Δφ_BL: boon-lay fuel failure (the PANAMA-I report's equations) at 1400 °C for 140 h, increment only. 1.2e-3: **KORA AVR 92/22**, an irradiated sphere in air at 1400 °C for 140 h, about 20 of 16 400 failed (IAEA-TECDOC-978 Table 5-7 = Kugeler 2017 Table 9; experimental) |
//! | `incremental_sic_accident` | SiC-only | 0 | as `sembawang::htr10::with_panama_accident_increment` sets it |
//!
//! The KORA failures were detected by ⁸⁵Kr release, so they are **full**
//! failures, and they add to boon-lay's increment in the same field.
//!
//! **The chain.**
//! - Inventory: Liu & Cao Table 1 (`changi::activity::inventory`), filtered to
//!   the nuclides TRISO-ATOPS supports.
//! - Release: `sembawang::accident::release::accident_release`, which runs
//!   TRISO-ATOPS diffusion release on those fractions.
//! - Geometry: HTR-10's, from `tampines::pebble_bed` (TECDOC-1382 Table 4-17).
//!
//! **Known behaviours of the chain, reported, not corrected:**
//! 1. ~~**The primary-circuit pools start empty** (`zero_pools`) … under-predicts~~
//!    **CORRECTED 2026-09-30 (#448):** the release now starts from **real
//!    normal-operation pools** (the new default, code-to-code verified against
//!    upstream at 3.8e-12). HTR-10 has no sourced `k_plate`, `k_clean`,
//!    `a_grain`, `x_liftoff` or normal-operation temperature, so those are
//!    stand-ins (see `htr10::plant_parameters`). Liu & Cao's Table 3
//!    circulating activity is still printed **beside** the release. It now
//!    double-counts the model's own circuit term, which is conservative and
//!    negligible here.
//! 2. **A half-life screen drops nuclides with t½ < 4 % of the 140 h
//!    transient** (≈ 5.6 h). The dropped ones are printed.
//! 3. **TRISO-ATOPS releases by diffusion, not 100 %.** This is a realistic
//!    release at bounding failure fractions, and it should sit **below**
//!    #435's 100 %-release bound, except silver, where #435 also takes 100 %.
//!    #438 compares the two.
//! 4. **The irradiation temperature is a stand-in**
//!    (`stand_in_irradiation_temperature`, #297), and Δφ_BL depends on it.
//!
//! **Pass criterion.** None. This is an estimate, not a gate. The check that
//! applies is #438's ordering: this release ≤ #435's hand-coded bound.
//!
//! # Results
//!
//! `cargo run --release -p sembawang --example htr10_air_ingress_kora_bound`,
//! 2026-09-30, on branch `claude/htr10-geometry-verification-gsg8jx`. **Not
//! reviewed by a human.**
//!
//! | Quantity | Value |
//! |---|---|
//! **Re-measured 2026-10-04** (dispersion lesson track, gh:#530) on `develop`
//! `d4428668be`, one core: every number below reproduced at its printed
//! precision (release 1.689e13 Bq, both venting modes 1.6888e13 Bq; 400 m
//! dose 66.137 mSv; the distance table; the 62.87 mSv cavity-ventilation arm).
//! No value is superseded.
//!
//! **Updated 2026-09-30 (third run):** real normal-operation pools (#448)
//! and groundshine added. The release window is the **96 h dose period**; the
//! failure fractions stay at their 140 h values. Earlier results are struck
//! through below.
//!
//! | Quantity | Value |
//! |---|---|
//! | boon-lay increment Δφ_BL, 1400 °C for 140 h (T_B 776 °C stand-in) | **3.768e-7** |
//! | full-failure fraction | **2.0004e-3** = 3e-4 + 5e-4 + 3.768e-7 + 1.2e-3 (KORA, 60 %) |
//! | SiC-only fraction (stand-ins) | 1.36e-4 |
//! | TRISO-ATOPS release over 96 h | **1.689e13 Bq**. Upstream venting and `FullFlowThrough` agree exactly |
//! | released / core | noble gases and I **1.91e-4** (Kr-85 1.76e-4); Cs-134 1.49e-3; **Cs-137 1.02e-3** (~~1.68e-3~~ with empty pools); Sr 1.9e-5; **Ag-110m 2.49e-2** |
//! | Screened out (t½ < 3.84 h) | Kr-83m, Kr-87, Kr-88, I-132, I-134 |
//! | Not in TRISO-ATOPS's table | H-3, Xe-135m, Rb-88 |
//! | Caveats | `negative_atom_count_seen` = true: silver and, since #448, Cs-137 (the path is not yet separated; see `htr10` tests) |
//!
//! **Maximum dose at 400 m, first 96 h** (adult; `buangkok`'s pyDOSEIA
//! single-plume Gaussian, ground release, ground-level centreline, 1 m/s,
//! class F, the largest χ/Q; the whole release blows over one point):
//!
//! | Pathway | Dose | Notes |
//! |---|---|---|
//! | Submersion (FGR-15) | **1.35 mSv** | I-135 0.70, I-133 0.32 |
//! | Inhalation (FGR-11, B = 3.33e-4 m³/s) | **41.4 mSv** | I-131 15.8, I-133 6.06, Cs-137 5.81, Cs-134 5.52, Sr-90 3.45, Sr-89 2.66 |
//! | **Groundshine (FGR-15, 96 h, new)** | **23.4 mSv** | I-133 5.90, Cs-134 5.22, I-131 4.38, I-135 3.65, Cs-137 2.97 |
//! | **Total** | **66.1 mSv** | ~~47.2~~ (empty pools, no groundshine); ~~27.9~~ (with 3 FGR-11 nuclides) |
//!
//! Groundshine method: deposit `A = Ψ · v_d`, with pyDOSEIA's SRS-19
//! velocities (noble gases 0, iodine and particulates 1000 m/d); present from
//! t = 0; no weathering; decaying over 96 h,
//! `E = A · DCF_gs · (1 − e^{−λT})/λ`. The plume is **not** depleted by the
//! deposition. That errs high on both air and ground, and is stated.
//!
//! **Maximum dose vs distance** (same assumptions; class F everywhere):
//!
//! | Distance | 400 m | 600 m | 800 m | 1 km | 1.5 km | **2 km** | 3 km | 5 km | 10 km |
//! |---|---|---|---|---|---|---|---|---|---|
//! | Dose (mSv) | 66.1 | 33.5 | 20.8 | 14.3 | 7.38 | **4.78** | 2.67 | 1.33 | 0.54 |
//!
//! **The dose falls to 10 mSv at ≈ 1238 m** (~~1011 m~~ without groundshine
//! and pools; ~~738 m~~ with 3 FGR-11 nuclides).
//!
//! **Transport arm (#447, 2026-09-30).** The bound vents fully (upstream's
//! isothermal branch, the conservative limit). With the core gas exchanged
//! instead at HTR-10's cavity ventilation rate, 100 %/day for 72 h then
//! sealed (Gao & Shi 2002 §5.3.2, `Venting::gao_shi_htr10_cavity_ventilation`),
//! the release is **0.950** of full venting (1 − e⁻³) and the maximum 400 m
//! dose is **62.9 mSv** against 66.1. Full venting adds only ≈ 5 %, because
//! three air changes already exchange 95 % of the core gas. That arm also
//! errs high, by construction: `frac(t)` multiplies the cumulative release.
//!
//! **Under-counted, stated:**
//! - ~~Inhalation for only Ag-110m, I-131, Cs-137~~ **fixed 2026-09-30**: every
//!   released nuclide with an FGR-11 entry is now counted. The noble gases have
//!   none by design (their dose is submersion).
//! - ~~Cs and Ag over-stated by empty pools~~: fixed by #448; see above.
//! - Not computed: ingestion,
//!   the screened-out short-lived nuclides (Kr-88, I-132, I-134 among them),
//!   and H-3, Xe-135m, Rb-88.
//!
//! **Conservative, stated:**
//! - class F at 1 m/s for 96 h with no direction change;
//! - ground release, no building wake, no depletion, no decay in transit;
//! - a 1400 °C whole-core hold; the failure fractions at their 140 h values.
//!
//! **Context only, not validation:** Liu & Cao 2002 Table 9 gives whole-body
//! doses of 0.077 mSv (depressurisation) and 0.20 mSv (water ingress) at
//! 0.25 km, for **different** accidents, with AIRDOS-EPA and measured site
//! meteorology. They are not a comparator for this case.
//!
//! ~~**FINDING: TRISO-ATOPS releases nothing from an isothermal hold, by
//! construction** … TRISO-ATOPS has no transport path for air ingress.~~
//! **CORRECTED 2026-09-30 (#446):** the first run of this example released
//! **0 Bq**, and it was read as an upstream limitation. **Reading upstream
//! showed it was a port defect.** `trisoatops.py::accident_case` (commit
//! `de374c8`) has
//! `if not np.all(accident_temp == accident_temp[0,0,0]): … coolant_release …
//! else: frac = np.ones(np.size(times))`,
//! so a uniform constant hold vents fully at every sample. `sembawang`'s port
//! always called `coolant_release`. The branch is restored in
//! `sembawang::accident::release::Venting::Upstream`; the numbers above are
//! after the fix.
//!
//! **What remains true:** for a **non-uniform** transient, upstream (and so
//! this chain, by default) still transports activity only while the core
//! heats. That is a depressurisation model, with no ingress flow. For ingress
//! transients with gradients, use `Venting::FullFlowThrough` (conservative,
//! everything released leaves the core) or `Venting::Prescribed` (a
//! caller-supplied exchanged fraction, e.g. from the cavity ventilation once
//! #420 is sourced).

use boon_lay::fuel_failure::htr10 as panama_htr10;
use buangkok::coefficients::{
    external_coefficient, fgr11_inhalation, fgr11_inhalation_max_over_classes,
    fgr15_air_submersion, fgr15_ground_surface, fgr15_short_lived_progeny,
};
use buangkok::pydoseia::dcf::AgeBracket;
use buangkok::pydoseia::dispersion::{
    dilution_single_plume_no_met, MeanSpeedScaling, PlumeGeometry, Receptor, StabilityClass,
};
use buangkok::pydoseia::dose::{deposition_velocity_m_per_s, submersion_dose, Release};
use buangkok::published::accident_dose_by_distance::htr10_accident_dose_by_distance;
use boon_lay::triso_atops_fork::accident::AccidentFractions;
use boon_lay::triso_atops_fork::nuclide_model::nuclide_database::find_nuclide;
use changi::activity::inventory::htr10_equilibrium_core;
use changi::activity::primary_helium::htr10_primary_helium_end_of_life;
use sembawang::accident::release::{accident_release, accident_release_with_venting, Venting};
use sembawang::htr10::{self, Htr10Geometry};
use sembawang::inventory::{CoreInventory, NuclideInventory};
use sembawang::scenario::TemperatureTransient;
use uom::si::f64::{Length, Radioactivity, ThermodynamicTemperature, Time};
use uom::si::length::{kilometer, meter};
use uom::si::radioactivity::becquerel;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::hour;

// ---------------------------------------------------------------- the inputs

/// Bounding hold temperature \[°C\]: the KORA and JAERI test temperature.
const HOLD_CELSIUS: f64 = 1400.0;
/// Hold duration \[h\]: the KORA AVR 92/22 test's 140 h. It covers Gao &
/// Shi's 72 h air supply twice over.
const HOLD_HOURS: f64 = 140.0;

/// f_hm: Liu & Cao 2002 §2.1, HTR-10 design as-manufactured free uranium.
const F_HM: f64 = 3.0e-4;
/// f_inc: Liu & Cao 2002 §2.1, HTR-10 design irradiation failure.
const F_INC: f64 = 5.0e-4;
/// f_sic: **stand-in**, the NP-MHTGR reference (`PlantParameters::np_mhtgr_reference`).
const F_SIC_STAND_IN: f64 = 1.0e-4;
/// f_inc_sic: **stand-in**, NP-MHTGR.
const F_INC_SIC_STAND_IN: f64 = 3.6e-5;
/// f_ox: KORA AVR 92/22, 1400 °C, 140 h, about 20 of 16 400 failed
/// (IAEA-TECDOC-978 Table 5-7; Kugeler 2017 Table 9). Experimental.
const F_OX_KORA: f64 = 1.2e-3;

/// Receptor distance \[m\] (#209's order-of-magnitude question).
const RECEPTOR_M: f64 = 400.0;
/// Release height \[m\]: ground level, no plume rise, no stack credit (#436).
const RELEASE_HEIGHT_M: f64 = 0.0;
/// Wind measurement height \[m\]. pyDOSEIA raises a release below 10 m to
/// 10 m in its height correction, so with 10 m here the wind is exactly 1 m/s.
const MEASUREMENT_HEIGHT_M: f64 = 10.0;
/// Breathing rate \[m³/s\]: FGR-11's "normal breathing rate" 0.020 m³/min,
/// the highest in the corpus (#437;
/// `crates/kovan-literature/derived/epa-fgr11-fgr13-breathing-rates.md`).
/// pyDOSEIA's own is 8400 m³/y (2.66e-4 m³/s), 20 % lower.
const BREATHING_M3_PER_S: f64 = 0.020 / 60.0;

/// Dose integration period \[h\]: the 4-day early phase (maintainer,
/// 2026-09-30: "max dose, 96 h"). The RELEASE transient runs to this time, so
/// the dose counts everything released in the first 96 h. The failure
/// fractions stay at their 140 h values (KORA's measurement and the 140 h
/// boon-lay hold), which errs high, since failure only grows with time.
const DOSE_PERIOD_HOURS: f64 = 96.0;

/// Time samples in the flat release transient.
const SAMPLES: usize = 97;
/// Integration steps for the boon-lay isothermal hold.
const BL_STEPS: usize = 200;
/// A single node: a uniform hold has no gradient to resolve.
const N_RADIAL: usize = 1;
const N_AXIAL: usize = 1;

/// Nuclides to show first, the five `htgr_sim_v1` tracks.
const HEADLINE: [&str; 5] = ["Kr-85", "Xe-133", "I-131", "Cs-137", "Ag-110m"];

/// Distances for the dose sweep \[m\].
const SWEEP_M: [f64; 9] = [400.0, 600.0, 800.0, 1000.0, 1500.0, 2000.0, 3000.0, 5000.0, 10000.0];
/// Dose level whose distance is reported \[Sv\] (maintainer's question: 10 mSv).
const DOSE_LEVEL_SV: f64 = 10.0e-3;

/// The largest single-plume χ/Q over classes A-F at `x_m`, and its class.
fn max_chi_over_q(x_m: f64, geometry: PlumeGeometry) -> (StabilityClass, f64) {
    let per_class = dilution_single_plume_no_met(
        Length::new::<meter>(x_m),
        geometry,
        MeanSpeedScaling::UnitSpeed,
    );
    StabilityClass::ALL
        .iter()
        .zip(per_class.iter())
        .map(|(c, d)| (*c, d.seconds_per_cubic_meter()))
        .fold((StabilityClass::A, 0.0), |a, b| if b.1 > a.1 { b } else { a })
}

/// HTR-10 geometry from `tampines` (TECDOC-1382 part 2 Table 4-17), as in the
/// `htr10_dlofc_panama_source_term` example.
fn htr10_geometry() -> Htr10Geometry {
    let particle = tampines::pebble_bed::triso::TrisoParticle::htr10();
    let pebble = tampines::pebble_bed::pebble::Pebble::htr10();
    Htr10Geometry {
        kernel_radius: particle.kernel_radius,
        sic_thickness: particle.silicon_carbide_outer_radius - particle.inner_pyc_outer_radius,
        graphite_thickness: pebble.outer_radius - pebble.fuelled_zone_radius,
    }
}

/// Liu & Cao Table 1, kept where TRISO-ATOPS supports the nuclide. Returns the
/// inventory and the (nuclide, Bq) pairs dropped.
fn htr10_inventory() -> (CoreInventory, Vec<(String, f64)>) {
    let mut kept = Vec::new();
    let mut dropped = Vec::new();
    for entry in htr10_equilibrium_core() {
        if find_nuclide(entry.nuclide).is_some() {
            kept.push(NuclideInventory::uniform(entry.nuclide, entry.activity, N_RADIAL));
        } else {
            dropped.push((entry.nuclide.to_string(), entry.activity.get::<becquerel>()));
        }
    }
    (CoreInventory::new(kept, N_RADIAL, N_AXIAL), dropped)
}

/// A flat `HOLD_CELSIUS` history over `DOSE_PERIOD_HOURS` (the release
/// window), uniform over the nodes.
fn flat_hold() -> TemperatureTransient {
    let times: Vec<Time> = (0..SAMPLES)
        .map(|i| Time::new::<hour>(DOSE_PERIOD_HOURS * i as f64 / (SAMPLES - 1) as f64))
        .collect();
    let t = ThermodynamicTemperature::new::<degree_celsius>(HOLD_CELSIUS);
    let temperatures = vec![vec![vec![t; N_AXIAL]; SAMPLES]; N_RADIAL];
    TemperatureTransient::from_nodes(times, temperatures).expect("a flat hold has enough samples")
}

fn main() {
    println!("================================================================");
    println!(" HTR-10 air ingress, BOUNDING CASE B: 1400 C hold, 140 h");
    println!(" boon-lay fuel failure + KORA f_ox -> TRISO-ATOPS (sembawang)");
    // CORRECTED 2026-10-04: this banner said "No dose.", but sections 5-7 print one.
    println!(" RESEARCH, EDUCATION AND V&V ONLY. Not a source term. The dose below is a bounding model's, never a dose to a person.");
    println!("================================================================\n");

    // ------------------------------------------------ 1. boon-lay increment
    let t_b = htr10::stand_in_irradiation_temperature();
    let hold_t = ThermodynamicTemperature::new::<degree_celsius>(HOLD_CELSIUS);
    let (phi_1, phi_2, f_end) =
        htr10::isothermal_failure(t_b, hold_t, Time::new::<hour>(HOLD_HOURS), BL_STEPS);
    let f_eoi = panama_htr10::end_of_irradiation_failure(t_b).get::<ratio>();
    let d_phi_bl = f_end - f_eoi;
    println!("-- 1. boon-lay fuel failure, isothermal {HOLD_CELSIUS} C for {HOLD_HOURS} h");
    println!(
        "   T_B (STAND-IN, #297)            {:.0} C",
        t_b.get::<degree_celsius>()
    );
    println!("   phi_1 (pressure vessel)        {phi_1:.3e}");
    println!("   phi_2 (SiC decomposition)      {phi_2:.3e}");
    println!("   f_inc at end of hold           {f_end:.3e}");
    println!("   minus end-of-irradiation phi_1 {f_eoi:.3e}");
    println!("   = accident increment d_phi_BL  {d_phi_bl:.3e}\n");

    // ------------------------------------------------ 2. failure fractions
    let fractions = AccidentFractions {
        heavy_metal: F_HM,
        sic: F_SIC_STAND_IN,
        incremental: F_INC,
        incremental_sic: F_INC_SIC_STAND_IN,
        incremental_accident: d_phi_bl + F_OX_KORA,
        incremental_sic_accident: 0.0,
    };
    let full = F_HM + F_INC + d_phi_bl + F_OX_KORA;
    let sic_only = F_SIC_STAND_IN + F_INC_SIC_STAND_IN;
    println!("-- 2. failure fractions (TRISO-ATOPS classes)");
    println!("   full:     f_hm {F_HM:.1e} + f_inc {F_INC:.1e} + d_phi_BL {d_phi_bl:.3e} + f_ox(KORA) {F_OX_KORA:.1e}");
    println!("             = {full:.4e}");
    println!("   SiC-only: f_sic {F_SIC_STAND_IN:.1e} (stand-in) + f_inc_sic {F_INC_SIC_STAND_IN:.1e} (stand-in) = {sic_only:.3e}");
    println!(
        "   share of full failure carried by KORA f_ox: {:.1} %\n",
        100.0 * F_OX_KORA / full
    );

    // ------------------------------------------------ 3. TRISO-ATOPS release
    let (inventory, dropped) = htr10_inventory();
    let plant = htr10::plant_parameters(htr10_geometry(), fractions);
    // Upstream venting: a uniform constant hold takes upstream's frac = 1 branch
    // (restored 2026-09-30, #446). FullFlowThrough is this workspace's explicit
    // conservative mode; on an isothermal hold the two must agree.
    let out = accident_release(&inventory, &flat_hold(), &plant).expect("the release chain runs");
    let out_ff =
        accident_release_with_venting(&inventory, &flat_hold(), &plant, &Venting::FullFlowThrough)
            .expect("the release chain runs");
    let ff_total: f64 = out_ff
        .source_term
        .nuclides
        .iter()
        .map(|r| r.total_released().get::<becquerel>())
        .sum();

    let atops: Vec<(String, f64)> = out
        .source_term
        .nuclides
        .iter()
        .map(|r| (r.label.clone(), r.total_released().get::<becquerel>()))
        .collect();
    let circulating = htr10_primary_helium_end_of_life();
    let circ_bq = |n: &str| {
        circulating
            .iter()
            .find(|e| e.nuclide == n)
            .map_or(0.0, |e| e.activity.get::<becquerel>())
    };
    let core_bq = |n: &str| {
        htr10_equilibrium_core()
            .iter()
            .find(|e| e.nuclide == n)
            .map_or(f64::NAN, |e| e.activity.get::<becquerel>())
    };

    println!("-- 3. release [Bq] over the first {DOSE_PERIOD_HOURS} h: TRISO-ATOPS (real normal-operation pools, #448) + Liu & Cao circulating (100 %)");
    println!("   (the Liu & Cao column DOUBLE-COUNTS the model's own circuit term: conservative, and negligible here)");
    println!("   nuclide     core inventory   TRISO-ATOPS    + circulating   = total     total/core");
    let mut order: Vec<&(String, f64)> = atops.iter().collect();
    order.sort_by_key(|(n, _)| HEADLINE.iter().position(|h| h == n).unwrap_or(HEADLINE.len()));
    for (n, bq) in order {
        let c = circ_bq(n);
        let core = core_bq(n);
        println!(
            "   {n:<10} {core:>14.3e}   {bq:>11.3e}   {c:>13.3e}   {:>10.3e}   {:>9.2e}",
            bq + c,
            (bq + c) / core
        );
    }
    let total_atops: f64 = atops.iter().map(|(_, b)| b).sum();
    let total_circ: f64 = atops.iter().map(|(n, _)| circ_bq(n)).sum();
    println!(
        "   TOTAL (released nuclides)      {total_atops:>11.3e}   {total_circ:>13.3e}   {:>10.3e}\n",
        total_atops + total_circ
    );

    // ------------------------------------------------ 4. what the chain dropped
    println!("-- 4. reported, not corrected");
    println!("   screened out (t1/2 < 4 % of {DOSE_PERIOD_HOURS} h): {:?}", out.screened_out);
    println!(
        "   not in TRISO-ATOPS's nuclide table: {:?}",
        dropped.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>()
    );
    let c = &out.caveats;
    println!(
        "   caveats: first-sample-vented {}, negative-atoms {}, D-clamped {}, gappy-venting {}",
        c.first_sample_forced_fully_vented,
        c.negative_atom_count_seen,
        c.diffusion_coefficient_clamped,
        c.venting_mask_was_gappy
    );
    println!("   pools: real normal-operation pools (#448; HTR-10 stand-ins, see htr10::plant_parameters), x_liftoff 0.05 (stand-in).");
    println!(
        "   venting: Upstream (uniform-constant branch, frac = 1) total {total_atops:.4e} Bq; \
         FullFlowThrough total {ff_total:.4e} Bq (must agree)"
    );

    // ------------------------------------------------ 5. dose at 400 m (buangkok)
    // Gaussian plume: buangkok's pyDOSEIA port, single (instantaneous) plume,
    // no met data, 1 m/s, ground-level release, ground-level centreline
    // receptor, total reflection. Every class A-F is evaluated and the LARGEST
    // chi/Q is used (#436: the worst class is tested, not assumed).
    let geometry = PlumeGeometry {
        release_height: Length::new::<meter>(RELEASE_HEIGHT_M),
        measurement_height: Length::new::<meter>(MEASUREMENT_HEIGHT_M),
        receptor: Receptor::GroundLevelCentreline,
    };
    let per_class = dilution_single_plume_no_met(
        Length::new::<meter>(RECEPTOR_M),
        geometry,
        MeanSpeedScaling::UnitSpeed,
    );
    println!("\n-- 5. MAXIMUM dose at {RECEPTOR_M} m, first {DOSE_PERIOD_HOURS} h (buangkok Gaussian plume, FGR, adult)");
    println!("   every release in the {DOSE_PERIOD_HOURS} h window is taken to pass the receptor at the worst class, 1 m/s,");
    println!("   with no wind-direction change: the whole release blows over one point.");
    println!("   chi/Q [s/m3] by class, single plume, 1 m/s, ground release, centreline:");
    let mut worst = (StabilityClass::A, 0.0_f64);
    for (class, d) in StabilityClass::ALL.iter().zip(per_class.iter()) {
        let v = d.seconds_per_cubic_meter();
        println!("     {class:?}: {v:.4e}");
        if v > worst.1 {
            worst = (*class, v);
        }
    }
    let chi = per_class[StabilityClass::ALL.iter().position(|c| *c == worst.0).unwrap()];
    println!("   used: class {:?}, chi/Q = {:.4e} s/m3 (the largest)\n", worst.0, worst.1);

    let sub_table = fgr15_air_submersion();
    let chains = fgr15_short_lived_progeny();
    let inh_table = fgr11_inhalation();
    let gs_table = fgr15_ground_surface();
    let exposure_s = DOSE_PERIOD_HOURS * 3600.0;
    println!("   nuclide     Q [Bq]       Psi [Bq s/m3]   submersion [Sv]   inhalation [Sv]   groundshine [Sv]   total [Sv]");
    let (mut e_sub_sum, mut e_inh_sum, mut e_gs_sum) = (0.0, 0.0, 0.0);
    let mut dose_by_nuclide: Vec<(String, f64)> = Vec::new();
    let mut missing_gs = Vec::new();
    let mut missing_sub = Vec::new();
    let mut missing_inh = Vec::new();
    for (n, bq) in &atops {
        let q = bq + circ_bq(n);
        let psi = chi.seconds_per_cubic_meter() * q;
        let release = Release::Instantaneous(Radioactivity::new::<becquerel>(q));
        let e_sub = external_coefficient(&sub_table, &chains, n, AgeBracket::Adult)
            .map(|dcf| submersion_dose(chi, release, dcf).sieverts());
        // Inhalation by hand (Psi * DCF * B), so the #437 breathing rate is
        // used; buangkok's inhalation_dose hard-codes pyDOSEIA's rate.
        let e_inh = fgr11_inhalation_max_over_classes(&inh_table, n, AgeBracket::Adult)
            .map(|dcf| psi * dcf * BREATHING_M3_PER_S);
        // Groundshine: deposit A = Psi * v_d [Bq/m2] (pyDOSEIA's SRS-19
        // velocities; the plume is NOT depleted by it, which errs high), present
        // from t = 0, no weathering, decaying over the dose period:
        // E = A * DCF_gs * (1 - exp(-lambda T)) / lambda.
        let element = n.split('-').next().unwrap_or("");
        let v_d = deposition_velocity_m_per_s(element);
        let lam = find_nuclide(n)
            .map(|x| x.decay_constant().get::<uom::si::frequency::hertz>())
            .unwrap_or(0.0);
        let decay_integral = if lam > 0.0 { -(-lam * exposure_s).exp_m1() / lam } else { exposure_s };
        let e_gs = if v_d == 0.0 {
            Some(0.0)
        } else {
            external_coefficient(&gs_table, &chains, n, AgeBracket::Adult)
                .map(|dcf| psi * v_d * dcf * decay_integral)
        };
        let fmt = |e: Option<f64>| e.map_or_else(|| "MISSING".to_string(), |v| format!("{v:.3e}"));
        println!(
            "   {n:<10} {q:>11.3e}   {psi:>13.3e}   {:>15}   {:>15}   {:>16}   {:>10.3e}",
            fmt(e_sub),
            fmt(e_inh),
            fmt(e_gs),
            e_sub.unwrap_or(0.0) + e_inh.unwrap_or(0.0) + e_gs.unwrap_or(0.0)
        );
        match e_gs {
            Some(v) => e_gs_sum += v,
            None => missing_gs.push(n.as_str()),
        }
        dose_by_nuclide.push((
            n.clone(),
            e_sub.unwrap_or(0.0) + e_inh.unwrap_or(0.0) + e_gs.unwrap_or(0.0),
        ));
        match e_sub {
            Some(v) => e_sub_sum += v,
            None => missing_sub.push(n.as_str()),
        }
        match e_inh {
            Some(v) => e_inh_sum += v,
            None => missing_inh.push(n.as_str()),
        }
    }
    println!(
        "   TOTAL over nuclides WITH a coefficient: submersion {e_sub_sum:.3e} Sv, inhalation {e_inh_sum:.3e} Sv, \
         groundshine ({DOSE_PERIOD_HOURS} h) {e_gs_sum:.3e} Sv, sum {:.3e} Sv = {:.3e} mSv",
        e_sub_sum + e_inh_sum + e_gs_sum,
        1e3 * (e_sub_sum + e_inh_sum + e_gs_sum)
    );
    println!("   no groundshine coefficient (NOT zero, missing): {missing_gs:?}");
    println!("   no submersion coefficient (NOT zero, missing): {missing_sub:?}");
    println!("   no inhalation coefficient (FGR-11 adult; noble gases have none by design): {missing_inh:?}");
    println!("   NOT computed: ingestion, the screened-out");
    println!("   short-lived nuclides, and H-3 / Xe-135m / Rb-88 (not in TRISO-ATOPS).");

    // Calculated reference, not validation: Liu & Cao Table 9 (AIRDOS-EPA, measured
    // site met, their depressurisation and water-ingress releases, not this case).
    println!("\n   reference (Liu & Cao 2002 Table 9, a DIFFERENT scenario and model; context only):");
    for row in htr10_accident_dose_by_distance().iter().take(2) {
        println!(
            "     {:.2} km: depressurisation whole-body {:.1e} mSv, water ingress whole-body {:.1e} mSv",
            row.distance.get::<kilometer>(),
            row.depressurization.whole_body_msv,
            row.water_ingress.whole_body_msv
        );
    }

    // ------------------------------------------------ 6. dose vs distance
    // The release is fixed, so every pathway above is linear in chi/Q: the dose
    // at x is (dose at 400 m) * maxchi(x) / maxchi(400 m), with the worst class
    // re-chosen at each x. Same under-count as section 5 (missing coefficients).
    let dose_per_chi = (e_sub_sum + e_inh_sum + e_gs_sum) / worst.1;
    println!("\n-- 6. MAXIMUM dose vs distance, first {DOSE_PERIOD_HOURS} h (same under-count as section 5)");
    println!("   distance [m]   class   chi/Q [s/m3]    dose [mSv]");
    for x in SWEEP_M {
        let (c, v) = max_chi_over_q(x, geometry);
        println!("   {x:>12.0}   {c:?}       {v:>11.4e}   {:>10.3}", 1e3 * dose_per_chi * v);
    }
    // Bisection for the distance where the dose falls to DOSE_LEVEL_SV
    // (chi/Q is strictly decreasing in x for every class).
    let dose_at = |x: f64| dose_per_chi * max_chi_over_q(x, geometry).1;
    if dose_at(SWEEP_M[0]) > DOSE_LEVEL_SV {
        let (mut lo, mut hi) = (SWEEP_M[0], 100_000.0_f64);
        if dose_at(hi) < DOSE_LEVEL_SV {
            for _ in 0..60 {
                let mid = 0.5 * (lo + hi);
                if dose_at(mid) > DOSE_LEVEL_SV { lo = mid } else { hi = mid }
            }
            println!(
                "   dose falls to {:.0} mSv at x = {:.0} m (class {:?})",
                1e3 * DOSE_LEVEL_SV,
                hi,
                max_chi_over_q(hi, geometry).0
            );
        } else {
            println!("   dose is still above {:.0} mSv at 100 km", 1e3 * DOSE_LEVEL_SV);
        }
    }

    // ------------------------------------------------ 7. transport arm: Gao & Shi ventilation
    // The bound above vents fully (upstream's isothermal branch). Here the
    // core gas instead exchanges at the HTR-10 cavity ventilation rate, Gao &
    // Shi 2002 §5.3.2: 100 %/day, well mixed, cut off at 72 h. Every pathway
    // is linear in the released activity, so each nuclide's dose scales by
    // its release ratio.
    let out_vent = accident_release_with_venting(
        &inventory,
        &flat_hold(),
        &plant,
        &Venting::gao_shi_htr10_cavity_ventilation(),
    )
    .expect("the release chain runs");
    let full: std::collections::HashMap<&str, f64> =
        out.cumulative_final.iter().map(|(n, b)| (n.as_str(), *b)).collect();
    let mut vent_dose = 0.0;
    for (n, bq) in &out_vent.cumulative_final {
        let scale = if full[n.as_str()] > 0.0 { bq / full[n.as_str()] } else { 0.0 };
        vent_dose += dose_by_nuclide.iter().find(|(m, _)| m == n).map_or(0.0, |(_, d)| *d) * scale;
    }
    let vent_total: f64 = out_vent.cumulative_final.iter().map(|(_, b)| b).sum();
    let full_total: f64 = out.cumulative_final.iter().map(|(_, b)| b).sum();
    println!("\n-- 7. transport arm: Gao & Shi cavity ventilation (100 %/day, cut off at 72 h) instead of full venting");
    println!(
        "   release {vent_total:.4e} Bq (full venting {full_total:.4e}; ratio {:.3}); max dose at {RECEPTOR_M} m {:.2} mSv (bound {:.2} mSv)",
        vent_total / full_total,
        1e3 * vent_dose,
        1e3 * (e_sub_sum + e_inh_sum + e_gs_sum)
    );
}
